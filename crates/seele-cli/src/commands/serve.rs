use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Args as ClapArgs;
use seele_http::server::{parse_cors_origin, ChatProviderConfig};
use seele_http::{Server, ServerConfig};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Listen port. `0` lets the OS pick (binding announced via stderr).
    #[arg(long, default_value_t = 7777)]
    pub port: u16,
    /// Bind address.
    #[arg(long, default_value = "127.0.0.1")]
    pub bind: String,
    /// Expose ENGRAM-compatible path aliases per ADR-13.
    #[arg(long)]
    pub legacy_engram_paths: bool,
    /// Require `Authorization: Bearer <token>` for non-public routes.
    /// Reads from the env var named here if the value starts with `$`,
    /// otherwise used literally. E.g. `--auth-bearer $SEELE_TOKEN`.
    #[arg(long)]
    pub auth_bearer: Option<String>,
    /// Add one origin to the CORS allowlist. Repeatable; matching is
    /// exact, so give the full origin (scheme, host and port) exactly as
    /// the browser sends it: `http://127.0.0.1:8765` and
    /// `http://localhost:8765` are different origins and both need
    /// listing if both are used. Only listed origins get an
    /// `Access-Control-Allow-Origin` back. An unusable value — `*`
    /// included, since the allowlist takes no wildcards — refuses to
    /// start the server rather than being dropped silently. Empty = CORS
    /// disabled (default; safe for local-only use).
    #[arg(long = "cors-allow", value_name = "ORIGIN")]
    pub cors_allow: Vec<String>,
    /// AI provider for the chat-with-DB endpoint (POST /chat).
    /// Supported: `minimax`, `openai`, `openrouter`, `together`, `groq`,
    /// `deepseek`, `anthropic`, or any other label (treated as
    /// OpenAI-compatible, needs --chat-endpoint).
    #[arg(long = "chat-provider", value_name = "NAME")]
    pub chat_provider: Option<String>,
    /// API key for the chat provider. Reads from the env var named here if
    /// the value starts with `$`, otherwise used literally. E.g.
    /// `--chat-key $MINIMAX_API_KEY`.
    #[arg(long = "chat-key", value_name = "KEY_OR_ENVVAR")]
    pub chat_key: Option<String>,
    /// Model name to use for chat completions. Defaults vary per provider.
    #[arg(long = "chat-model", value_name = "MODEL")]
    pub chat_model: Option<String>,
    /// Override the OpenAI-compatible chat completions URL. Ignored for
    /// `anthropic`. If omitted, a sensible default per --chat-provider is
    /// used.
    #[arg(long = "chat-endpoint", value_name = "URL")]
    pub chat_endpoint: Option<String>,
}

pub async fn run(args: Args, db: &Option<PathBuf>, fake_embedder: bool) -> anyhow::Result<()> {
    // First thing, before opening the DB or loading an embedder: a typo in
    // an origin should cost one line of stderr and a non-zero exit, not a
    // server that comes up announcing "listening" with an allowlist that
    // silently matches nothing.
    validate_cors_origins(&args.cors_allow)?;

    let svc = crate::app::build_service(db, fake_embedder)?;
    let addr: SocketAddr = format!("{}:{}", args.bind, args.port).parse()?;

    let chat = match (args.chat_provider, args.chat_key) {
        (Some(provider), Some(key_or_env)) => {
            let api_key = resolve_secret(&key_or_env, "chat key")?;
            let model = args
                .chat_model
                .unwrap_or_else(|| default_model_for(&provider));
            Some(ChatProviderConfig {
                provider,
                api_key,
                model,
                endpoint: args.chat_endpoint,
            })
        }
        (None, None) => None,
        _ => {
            anyhow::bail!("--chat-provider and --chat-key must be set together (or both omitted)");
        }
    };

    let auth_bearer = args
        .auth_bearer
        .as_deref()
        .map(|raw| resolve_secret(raw, "auth bearer"))
        .transpose()?;

    Server::new(
        svc,
        ServerConfig {
            addr,
            cors_origins: args.cors_allow,
            auth_bearer,
            legacy_engram_paths: args.legacy_engram_paths,
            chat,
        },
    )
    .run()
    .await
}

/// Reject the whole run on the first unusable `--cors-allow` value. The
/// check is `seele_http`'s own, so the flag refuses exactly what the CORS
/// layer would drop — the CLI and the router can never disagree about
/// which origins count.
fn validate_cors_origins(origins: &[String]) -> anyhow::Result<()> {
    for origin in origins {
        if let Err(reason) = parse_cors_origin(origin) {
            anyhow::bail!("{reason}");
        }
    }
    Ok(())
}

fn resolve_secret(raw: &str, label: &str) -> anyhow::Result<String> {
    if let Some(envvar) = raw.strip_prefix('$') {
        std::env::var(envvar)
            .map_err(|_| anyhow::anyhow!("{label} references env var ${envvar} but it is not set"))
    } else {
        Ok(raw.to_string())
    }
}

fn default_model_for(provider: &str) -> String {
    match provider.to_ascii_lowercase().as_str() {
        "minimax" => "MiniMax-M2".to_string(),
        "openai" => "gpt-4o-mini".to_string(),
        "openrouter" => "openai/gpt-4o-mini".to_string(),
        "together" => "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo".to_string(),
        "groq" => "llama-3.3-70b-versatile".to_string(),
        "deepseek" => "deepseek-chat".to_string(),
        "anthropic" => "claude-haiku-4-5-20251001".to_string(),
        _ => "gpt-4o-mini".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serve_auth_bearer_env_var_indirection_resolves() {
        std::env::set_var("SEELE_TEST_AUTH_BEARER", "tok-from-env");
        let resolved = resolve_secret("$SEELE_TEST_AUTH_BEARER", "auth bearer").unwrap();
        assert_eq!(resolved, "tok-from-env");
        std::env::remove_var("SEELE_TEST_AUTH_BEARER");
    }

    #[test]
    fn serve_auth_bearer_literal_value_passes_through() {
        let resolved = resolve_secret("literal-token", "auth bearer").unwrap();
        assert_eq!(resolved, "literal-token");
    }

    #[test]
    fn serve_auth_bearer_unset_env_var_errors() {
        let err = resolve_secret("$SEELE_TEST_AUTH_BEARER_UNSET", "auth bearer").unwrap_err();
        assert!(
            err.to_string().contains("$SEELE_TEST_AUTH_BEARER_UNSET"),
            "unexpected error: {err}"
        );
    }

    // -------- --cors-allow validation --------

    /// Every value here is a shape a browser really sends in `Origin`.
    #[test]
    fn serve_accepts_well_formed_origins() {
        for good in [
            // the four KAIROS passes today
            "http://localhost:8765",
            "http://127.0.0.1:8765",
            "http://localhost:8766",
            "http://127.0.0.1:8766",
            // https, default port omitted, IPv6 literal
            "https://seele.example",
            "https://seele.example:8443",
            "http://[::1]:8765",
        ] {
            validate_cors_origins(&[good.to_string()])
                .unwrap_or_else(|e| panic!("{good} must be accepted: {e}"));
        }
    }

    #[test]
    fn serve_empty_cors_allow_is_valid_cors_stays_off() {
        validate_cors_origins(&[]).expect("no --cors-allow means CORS disabled, not an error");
    }

    /// Assert the run is refused and hand back the message, so each case
    /// below can check what the operator is actually told.
    fn cors_rejection(origin: &str) -> String {
        let err = validate_cors_origins(&[origin.to_string()])
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("invalid CORS origin") && err.contains("scheme://host[:port]"),
            "every rejection must show the expected shape: {err}"
        );
        err
    }

    #[test]
    fn serve_rejects_wildcard_cors_origin() {
        let err = validate_cors_origins(&["*".to_string()])
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("invalid CORS origin") && err.contains("wildcards"),
            "unexpected error: {err}"
        );
    }

    /// The silent failure this shape check exists to kill: a browser never
    /// sends a scheme-less `Origin`, so this used to be accepted and then
    /// match nothing, forever, without a word.
    #[test]
    fn serve_rejects_cors_origin_without_a_scheme() {
        let err = cors_rejection("localhost:8765");
        assert!(err.contains("scheme"), "unexpected error: {err}");
    }

    #[test]
    fn serve_rejects_cors_origin_with_a_path() {
        for bad in [
            "http://localhost:8765/",
            "http://localhost:8765/index.html",
            "http://localhost:8765/api/wagers",
        ] {
            let err = cors_rejection(bad);
            assert!(err.contains("path"), "unexpected error for {bad}: {err}");
        }
    }

    #[test]
    fn serve_rejects_cors_origin_with_a_query_or_fragment() {
        for bad in ["http://localhost:8765?x=1", "http://localhost:8765#top"] {
            let err = cors_rejection(bad);
            assert!(err.contains("query"), "unexpected error for {bad}: {err}");
        }
    }

    #[test]
    fn serve_rejects_cors_origin_with_no_host() {
        let err = cors_rejection("http://");
        assert!(err.contains("host"), "unexpected error: {err}");
    }

    /// `--cors-allow` is repeatable, so the tempting slip is to cram the
    /// whole list into one value.
    #[test]
    fn serve_rejects_several_origins_crammed_into_one_value() {
        let err = cors_rejection("http://localhost:8765 http://localhost:8766");
        assert!(
            err.contains("one --cors-allow per origin"),
            "unexpected error: {err}"
        );
    }

    /// Header injection attempt: refused, not quietly dropped.
    #[test]
    fn serve_rejects_cors_origin_carrying_a_newline() {
        cors_rejection("http://localhost:8765\nX-Injected: 1");
    }

    /// Right shape, still not a legal header value: DEL is neither
    /// whitespace nor a delimiter, so only `HeaderValue` catches it. This
    /// is the third and last rejection path.
    #[test]
    fn serve_rejects_cors_origin_that_is_not_a_header_value() {
        let err = cors_rejection("http://bad\u{7f}origin");
        assert!(err.contains("header value"), "unexpected error: {err}");
    }

    #[test]
    fn serve_reports_the_offending_origin_not_just_that_one_is_bad() {
        let origins = [
            "http://localhost:8765".to_string(),
            "http://bad\u{7f}origin".to_string(),
        ];
        let err = validate_cors_origins(&origins).unwrap_err().to_string();
        assert!(err.contains("bad"), "error must name the value: {err}");
    }
}
