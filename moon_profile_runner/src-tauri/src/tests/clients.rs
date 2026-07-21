use super::*;
use axum::body::Body;
use axum::http::Request as HttpRequest;
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;

#[test]
fn upsert_client_inserts_a_new_entry() {
    let mut clients = HashMap::new();

    upsert_client(&mut clients, "192.168.1.50".to_string(), "2026-01-01T00:00:00-03:00".to_string());

    assert_eq!(
        clients.get("192.168.1.50"),
        Some(&KnownClient { ip: "192.168.1.50".to_string(), last_seen: "2026-01-01T00:00:00-03:00".to_string() })
    );
}

#[test]
fn upsert_client_updates_last_seen_for_an_existing_ip_instead_of_duplicating() {
    let mut clients = HashMap::new();
    upsert_client(&mut clients, "192.168.1.50".to_string(), "2026-01-01T00:00:00-03:00".to_string());

    upsert_client(&mut clients, "192.168.1.50".to_string(), "2026-01-01T00:05:00-03:00".to_string());

    assert_eq!(clients.len(), 1);
    assert_eq!(clients["192.168.1.50"].last_seen, "2026-01-01T00:05:00-03:00");
}

#[test]
fn save_and_load_known_clients_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clients.json");
    let mut clients = HashMap::new();
    upsert_client(&mut clients, "192.168.1.50".to_string(), "2026-01-01T00:00:00-03:00".to_string());
    upsert_client(&mut clients, "192.168.1.51".to_string(), "2026-01-01T00:01:00-03:00".to_string());

    save_known_clients(&path, &clients);
    let loaded = load_known_clients(&path);

    assert_eq!(loaded, clients);
}

#[test]
fn load_known_clients_returns_empty_when_the_file_does_not_exist_yet() {
    assert_eq!(load_known_clients(std::path::Path::new("/nonexistent/clients.json")), HashMap::new());
}

#[test]
fn load_known_clients_returns_empty_on_malformed_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clients.json");
    std::fs::write(&path, "not json").unwrap();

    assert_eq!(load_known_clients(&path), HashMap::new());
}

async fn dummy_handler() -> &'static str {
    "ok"
}

fn test_router(state: ClientsState, path: ClientsFilePath) -> Router {
    Router::new()
        .route("/dummy", get(dummy_handler))
        .layer(axum::middleware::from_fn(record_client_middleware))
        .layer(Extension(state))
        .layer(Extension(path))
}

#[tokio::test]
async fn records_the_caller_ip_from_connect_info() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("clients.json");
    let state = ClientsState(Arc::new(Mutex::new(HashMap::new())));

    let addr: SocketAddr = "192.168.1.50:12345".parse().unwrap();
    let request = HttpRequest::builder()
        .uri("/dummy")
        .extension(ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();

    let response = test_router(state.clone(), ClientsFilePath(file_path.clone())).oneshot(request).await.unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let clients = state.0.lock().await;
    assert!(clients.contains_key("192.168.1.50"));
    drop(clients);
    // Also confirms it was persisted, not just kept in memory.
    assert!(load_known_clients(&file_path).contains_key("192.168.1.50"));
}

#[tokio::test]
async fn does_not_record_anything_when_connect_info_is_missing() {
    // Every existing test elsewhere in this crate builds requests via
    // `oneshot` without a real accepted TCP connection, so they have no
    // ConnectInfo extension at all - confirms that case doesn't panic or
    // reject the request (Option<ConnectInfo<..>> is infallible).
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("clients.json");
    let state = ClientsState(Arc::new(Mutex::new(HashMap::new())));

    let request = HttpRequest::builder().uri("/dummy").body(Body::empty()).unwrap();

    let response = test_router(state.clone(), ClientsFilePath(file_path)).oneshot(request).await.unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert!(state.0.lock().await.is_empty());
}
