//! HTTP server skeleton: axum App + Router + AppState + middleware base.
//!
//! Real handlers live in `handlers/` modules and are wired here. The skeleton
//! exposes `/health` and `/version`; everything else lands in subsequent
//! Sprint-03 blocks (B/C/D).

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::FromRef;
use axum::http::HeaderValue;
use axum::middleware;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde_json::json;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::auth::require_bearer;
use crate::handlers;
use crate::openapi;
use crate::service::SeeleService;

/// Shared state every handler sees via `axum::extract::State<AppState>`.
/// Existing handlers use `State<Arc<SeeleService>>` (via the FromRef impl
/// below); the chat handler pulls the whole `AppState`.
#[derive(Clone)]
pub struct AppState {
    pub service: Arc<SeeleService>,
    pub chat: Option<Arc<ChatProviderConfig>>,
}

impl FromRef<AppState> for Arc<SeeleService> {
    fn from_ref(input: &AppState) -> Self {
        input.service.clone()
    }
}

impl FromRef<AppState> for Option<Arc<ChatProviderConfig>> {
    fn from_ref(input: &AppState) -> Self {
        input.chat.clone()
    }
}

#[derive(Clone)]
pub struct ServerConfig {
    pub addr: SocketAddr,
    /// Exact-match CORS allowlist. Empty = CORS disabled. Each entry is one
    /// full origin (scheme + host + port) spelled exactly as the browser
    /// sends it in the `Origin` header — `http://localhost:8765` and
    /// `http://127.0.0.1:8765` are two different origins.
    pub cors_origins: Vec<String>,
    /// When set, every route except `/health`, `/version`, `/docs/*`,
    /// `/openapi.json` requires `Authorization: Bearer <token>`.
    pub auth_bearer: Option<String>,
    /// Expose ENGRAM-compatible aliases (`POST /save`, `GET /show/{id}`)
    /// alongside the canonical SEELE routes. See ADR-13.
    pub legacy_engram_paths: bool,
    /// AI chat provider config (POST /chat). None = endpoint disabled.
    pub chat: Option<ChatProviderConfig>,
}

/// Chat-with-DB provider config. None of these fields ever leave the
/// machine — the API key stays on the box running `seele serve`.
#[derive(Clone, Debug)]
pub struct ChatProviderConfig {
    /// Provider family. Supported: `"minimax"`, `"openai"`, `"openrouter"`,
    /// `"together"`, `"groq"`, `"deepseek"`, `"anthropic"`, or any other
    /// label — anything that's not `"anthropic"` is treated as
    /// OpenAI-compatible and uses `endpoint` as `/v1/chat/completions`.
    pub provider: String,
    pub api_key: String,
    pub model: String,
    /// OpenAI-compatible providers: the full `/v1/chat/completions` URL.
    /// Ignored for `"anthropic"`.
    pub endpoint: Option<String>,
}

impl ServerConfig {
    pub fn loopback(port: u16) -> Self {
        Self {
            addr: SocketAddr::from(([127, 0, 0, 1], port)),
            cors_origins: Vec::new(),
            auth_bearer: None,
            legacy_engram_paths: false,
            chat: None,
        }
    }
}

pub struct Server {
    pub service: SeeleService,
    pub config: ServerConfig,
}

impl Server {
    pub fn new(service: SeeleService, config: ServerConfig) -> Self {
        Self { service, config }
    }

    /// Build the `axum::Router`. Public so tests can mount it against an
    /// in-process listener.
    pub fn router(&self) -> Router {
        let state = AppState {
            service: Arc::new(self.service.clone()),
            chat: self.config.chat.clone().map(Arc::new),
        };

        let cors = if self.config.cors_origins.is_empty() {
            CorsLayer::new()
        } else {
            // Exact-match allowlist: a browser gets an
            // `Access-Control-Allow-Origin` back only when its `Origin`
            // header is byte-identical to one of the configured entries,
            // so every host/port form in use has to be listed
            // (`http://localhost:8765` does not cover
            // `http://127.0.0.1:8765`). Credentials stay off on purpose —
            // callers authenticate with a bearer header, not cookies.
            // Methods/headers stay `Any` so POST + Content-Type clears
            // preflight; without that, a JSON POST from a browser fails
            // with "failed to fetch".
            CorsLayer::new()
                .allow_origin(AllowOrigin::list(allowed_origins(
                    &self.config.cors_origins,
                )))
                .allow_methods(Any)
                .allow_headers(Any)
        };

        // Routes that require auth (when enabled). Built first so the
        // auth middleware wraps only these; `/health`, `/version`, docs
        // stay public.
        let mut protected = Router::new()
            .route(
                "/memories",
                post(handlers::save_memory).get(handlers::list_memories),
            )
            .route(
                "/memories/{id}",
                get(handlers::get_memory)
                    .delete(handlers::soft_delete_memory)
                    .patch(handlers::patch_memory_metadata),
            )
            .route("/memories/{id}/restore", post(handlers::restore_memory))
            .route("/memories/{id}/links", get(handlers::list_links_for_memory))
            .route("/search", post(handlers::search_memories))
            .route("/projects", get(handlers::list_projects))
            .route(
                "/sessions",
                post(handlers::start_session).get(handlers::list_sessions),
            )
            .route("/sessions/{id}", get(handlers::get_session))
            .route("/sessions/{id}/end", put(handlers::end_session))
            .route("/sessions/{id}/abort", put(handlers::abort_session))
            .route("/links", post(handlers::create_link))
            .route("/links/{id}", delete(handlers::delete_link))
            .route(
                "/relations",
                post(handlers::create_relation).get(handlers::list_relations),
            )
            .route("/relations/{id}/judge", put(handlers::judge_relation))
            .route("/conflicts", get(handlers::list_pending_conflicts))
            .route("/stats", get(handlers::get_stats))
            .route("/embedder", get(handlers::get_embedder_info))
            .route("/chat", post(handlers::chat))
            .route("/chat/info", get(handlers::chat_info));

        if self.config.legacy_engram_paths {
            // ADR-13: ENGRAM-compatible aliases. Same handlers, alternate
            // paths. New SEELE endpoints (sessions, relations, etc.) are
            // NOT exposed under legacy paths — they don't exist in ENGRAM.
            protected = protected
                .route("/save", post(handlers::save_memory))
                .route("/show/{id}", get(handlers::get_memory));
        }

        let protected = protected.with_state(state);

        let protected = if let Some(token) = self.config.auth_bearer.clone() {
            protected.layer(middleware::from_fn(move |req, next| {
                let token = token.clone();
                async move { require_bearer(token, req, next).await }
            }))
        } else {
            protected
        };

        Router::new()
            .route("/health", get(health))
            .route("/version", get(version))
            .merge(protected)
            .merge(openapi::routes())
            .layer(TraceLayer::new_for_http())
            .layer(cors)
            .layer(CompressionLayer::new())
    }

    /// Bind + serve. Blocks until the server stops.
    ///
    /// Writes a single grepeable line to stderr once the listener is up:
    /// `seele http listening on http://<addr>`. This lets `--port 0`
    /// callers (tests, scripts) discover the OS-chosen port without
    /// the listener-then-drop race.
    pub async fn run(self) -> anyhow::Result<()> {
        let app = self.router();
        let listener = tokio::net::TcpListener::bind(&self.config.addr).await?;
        let addr = listener.local_addr()?;
        eprintln!("seele http listening on http://{addr}");
        tracing::info!(%addr, "seele http listening");
        axum::serve(listener, app).await?;
        Ok(())
    }
}

/// Check one configured CORS origin: `Ok` with the `HeaderValue` the
/// allowlist compares against, or `Err` with the message a human needs.
///
/// Three ways to fail. `*` is rejected because it is not an allowlist
/// entry at all — `--cors-allow "*"` used to mean "allow everything",
/// honouring it here would reopen the hole this allowlist closes, and
/// handing it to `AllowOrigin::list` panics outright. Then the value has
/// to *look* like an origin, because matching is byte-exact against what
/// the browser puts in `Origin`, and a browser sends `scheme://host[:port]`
/// and nothing else: `localhost:8765` or `http://localhost:8765/` are
/// accepted nowhere, they just never match, which is the silent failure
/// this whole check exists to prevent. Last, it has to be a legal header
/// value.
///
/// `seele serve` calls this at startup and refuses to boot on the first
/// rejection, so the flag fails fast on exactly the set of values the
/// router would otherwise drop.
pub fn parse_cors_origin(origin: &str) -> Result<HeaderValue, String> {
    if origin.trim() == "*" {
        return Err(format!(
            "invalid CORS origin: {origin} (the allowlist takes exact origins, not wildcards — \
             pass one --cors-allow per origin, e.g. --cors-allow http://localhost:8765 \
             --cors-allow http://127.0.0.1:8765)"
        ));
    }
    if let Some(detail) = origin_shape_problem(origin) {
        return Err(format!(
            "invalid CORS origin: {origin} (expected scheme://host[:port], \
             e.g. http://127.0.0.1:8765 — {detail})"
        ));
    }
    HeaderValue::from_str(origin).map_err(|_| {
        format!(
            "invalid CORS origin: {origin} (expected scheme://host[:port], \
             e.g. http://127.0.0.1:8765 — not a legal header value)"
        )
    })
}

/// What is wrong with the shape of `origin`, or `None` when it is exactly
/// `scheme://host[:port]`.
///
/// Checked in the order a human makes the mistake: the scheme first (a
/// bare `localhost:8765` is the most common one), then an empty host,
/// then whitespace — with `--cors-allow` now repeatable, cramming two
/// origins into one value is an easy slip, and `a/b` inside it would
/// otherwise be reported as a stray path — and finally anything trailing
/// the port. Nothing here inspects the host itself: `[::1]` and a
/// punycode name are both fine, and the byte-exact match against the
/// browser's `Origin` is the real arbiter.
fn origin_shape_problem(origin: &str) -> Option<&'static str> {
    let host = match origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    {
        Some(host) => host,
        None => return Some("the scheme is required, lowercase, and has to be http or https"),
    };
    if host.is_empty() {
        return Some("the host is missing");
    }
    if host.contains(char::is_whitespace) {
        return Some("no whitespace in an origin; pass one --cors-allow per origin");
    }
    if host.contains(['/', '?', '#']) {
        return Some("an origin stops at the port: no trailing slash, path, query or fragment");
    }
    None
}

/// Turn the configured origin strings into the `HeaderValue`s the CORS
/// allowlist compares against.
///
/// A bad entry is dropped rather than taking the process down: `seele
/// serve` already refuses to start on one, so reaching this point means a
/// caller built the router directly. If every entry is dropped the layer
/// ends up with an empty list, which matches no origin at all (same
/// effective result as CORS disabled).
///
/// The complaints go to stderr through `eprintln!`, deliberately, not
/// through `tracing::warn!`: nothing in SEELE installs a tracing
/// subscriber, so a `warn!` here is swallowed whole and the operator gets
/// "listening" with an allowlist that silently matches nothing.
fn allowed_origins(origins: &[String]) -> Vec<HeaderValue> {
    let mut allowed = Vec::with_capacity(origins.len());
    for origin in origins {
        match parse_cors_origin(origin) {
            Ok(value) => allowed.push(value),
            Err(reason) => eprintln!("seele: {reason} — origin skipped"),
        }
    }

    if allowed.is_empty() {
        eprintln!(
            "seele: all {} configured CORS origins were rejected; no origin is allowed \
             (CORS is effectively off)",
            origins.len()
        );
    }

    allowed
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({"status": "ok"}))
}

async fn version() -> Json<serde_json::Value> {
    Json(json!({
        "name": "seele",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
