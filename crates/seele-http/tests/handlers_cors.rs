//! CORS allowlist E2E tests.
//!
//! `cors_origins` is an exact-match allowlist, not a boolean: a listed
//! origin gets `Access-Control-Allow-Origin` back, anything else gets
//! nothing, and an empty list keeps CORS off entirely. Both the preflight
//! (`OPTIONS` + `Access-Control-Request-Method`) and the simple request
//! path are checked, because the Observatory needs both.

use std::sync::Arc;

use reqwest::Method;
use seele_embedder::{Embedder, FakeEmbedder};
use seele_http::{SeeleService, Server, ServerConfig};
use seele_storage::init_db;
use tempfile::TempDir;

const ALLOWED: &str = "http://localhost:8765";
const OUTSIDER: &str = "http://evil.example";
const ACAO: &str = "access-control-allow-origin";

async fn spawn(cors_origins: Vec<String>) -> (TempDir, String) {
    let td = TempDir::new().unwrap();
    let pool = init_db(td.path().join("seele.db")).unwrap();
    let embedder: Arc<dyn Embedder> = Arc::new(FakeEmbedder);
    let service = SeeleService::new(pool, embedder);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Server::new(
        service,
        ServerConfig {
            addr,
            cors_origins,
            auth_bearer: None,
            legacy_engram_paths: false,
            chat: None,
        },
    )
    .router();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (td, format!("http://{addr}"))
}

/// Preflight for a JSON POST, the shape a browser sends before
/// `POST /memories`.
async fn preflight(base: &str, origin: &str) -> reqwest::Response {
    reqwest::Client::new()
        .request(Method::OPTIONS, format!("{base}/memories"))
        .header("Origin", origin)
        .header("Access-Control-Request-Method", "POST")
        .header("Access-Control-Request-Headers", "content-type")
        .send()
        .await
        .unwrap()
}

/// Simple (non-preflighted) request: the browser still needs the header
/// on the actual response to hand the body to the page.
async fn simple_get(base: &str, origin: &str) -> reqwest::Response {
    reqwest::Client::new()
        .get(format!("{base}/health"))
        .header("Origin", origin)
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn listed_origin_is_echoed_on_preflight_and_simple_request() {
    let (_td, base) = spawn(vec![ALLOWED.to_string()]).await;

    let pre = preflight(&base, ALLOWED).await;
    assert_eq!(
        pre.headers().get(ACAO).map(|v| v.to_str().unwrap()),
        Some(ALLOWED),
        "preflight must allow the listed origin"
    );

    let get = simple_get(&base, ALLOWED).await;
    assert_eq!(get.status(), 200);
    assert_eq!(
        get.headers().get(ACAO).map(|v| v.to_str().unwrap()),
        Some(ALLOWED),
        "simple request must allow the listed origin"
    );
}

#[tokio::test]
async fn unlisted_origin_gets_no_allow_origin_header() {
    let (_td, base) = spawn(vec![ALLOWED.to_string()]).await;

    let pre = preflight(&base, OUTSIDER).await;
    assert!(
        pre.headers().get(ACAO).is_none(),
        "preflight leaked {ACAO} to an unlisted origin: {:?}",
        pre.headers().get(ACAO)
    );

    let get = simple_get(&base, OUTSIDER).await;
    assert!(
        get.headers().get(ACAO).is_none(),
        "simple request leaked {ACAO} to an unlisted origin: {:?}",
        get.headers().get(ACAO)
    );
}

#[tokio::test]
async fn empty_allowlist_disables_cors_for_every_origin() {
    let (_td, base) = spawn(vec![]).await;

    for origin in [ALLOWED, OUTSIDER] {
        let pre = preflight(&base, origin).await;
        assert!(
            pre.headers().get(ACAO).is_none(),
            "preflight from {origin} got {ACAO} with CORS disabled"
        );

        let get = simple_get(&base, origin).await;
        assert!(
            get.headers().get(ACAO).is_none(),
            "simple request from {origin} got {ACAO} with CORS disabled"
        );
    }
}

/// The regression this allowlist exists to prevent: a wildcard entry must
/// not reopen `Access-Control-Allow-Origin: *` (and must not panic the
/// server while being rejected — `AllowOrigin::list` panics on `*`).
///
/// `seele serve --cors-allow "*"` never reaches this far: the CLI refuses
/// to boot on it (`serve.rs::validate_cors_origins`). This covers the
/// layer underneath, for callers that build the router directly.
#[tokio::test]
async fn wildcard_entry_is_rejected_not_honoured() {
    let (_td, base) = spawn(vec!["*".to_string()]).await;

    let get = simple_get(&base, OUTSIDER).await;
    assert_eq!(get.status(), 200);
    assert!(
        get.headers().get(ACAO).is_none(),
        "`*` as an allowlist entry reopened permissive CORS"
    );
}
