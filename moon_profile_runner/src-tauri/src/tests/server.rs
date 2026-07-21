use super::*;
use crate::clients::ClientsState;
use crate::test_support::FakeGameProcess;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tower::ServiceExt;

fn empty_clients() -> (ClientsState, ClientsFilePath) {
    (ClientsState(Arc::new(Mutex::new(HashMap::new()))), ClientsFilePath(std::env::temp_dir().join("moon_profile_runner_tests_unused_clients.json")))
}

// Unit test, fast - doesn't need a real process, just covers the pure
// matching logic (the shared-prefix edge cases).
#[test]
fn cmd_arg_matches_app_id_cases() {
    assert!(cmd_arg_matches_app_id("AppId=42", "AppId=42"));
    assert!(cmd_arg_matches_app_id("SteamLaunch AppId=42 --", "AppId=42"));
    assert!(!cmd_arg_matches_app_id("AppId=420", "AppId=42"));
    assert!(!cmd_arg_matches_app_id("AppId=4", "AppId=42"));
    assert!(!cmd_arg_matches_app_id("nothing relevant", "AppId=42"));
}

// Test IDs quite distinct (900xxx) to avoid accidentally colliding with
// a real game's AppId running on the dev machine while testing.

#[tokio::test]
async fn is_app_id_running_false_when_no_matching_process_exists() {
    assert!(!is_app_id_running("900001"));
}

#[tokio::test]
async fn is_app_id_running_true_when_a_matching_process_is_alive() {
    let fake = FakeGameProcess::spawn("900002");
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(is_app_id_running("900002"));
    drop(fake);
}

#[tokio::test]
async fn is_app_id_running_false_again_after_the_process_exits() {
    let fake = FakeGameProcess::spawn("900003");
    tokio::time::sleep(Duration::from_millis(200)).await;
    drop(fake); // kills and waits for it to exit before checking again
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(!is_app_id_running("900003"));
}

#[tokio::test]
async fn is_app_id_running_does_not_match_a_different_app_id_with_a_shared_prefix() {
    // "900004" shouldn't match a process that has "AppId=9000044"
    // (shared prefix) - contains() is substring matching, so this case
    // confirms that the exact "AppId=<id>" format (with no separator
    // after) doesn't produce a false positive here in practice.
    let fake = FakeGameProcess::spawn("9000044");
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(!is_app_id_running("900004"));
    drop(fake);
}

fn test_app() -> Router {
    let (tx, _rx) = mpsc::unbounded_channel();
    let session_state: SessionState = Arc::new(Mutex::new(None));
    let (clients_state, clients_file_path) = empty_clients();
    app(tx, session_state, ApolloBaseUrl("http://127.0.0.1:0".to_string()), clients_state, clients_file_path)
}

// Only confirms the route is registered and returns valid JSON - the
// actual game-parsing logic (the VDF fixtures, the "missing
// libraryfolders.vdf" case, etc) is already covered in depth in
// games.rs; duplicating it here would just test the same thing twice.
#[tokio::test]
async fn games_route_returns_a_json_array() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/games")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let _games: Vec<crate::games::HostGame> = serde_json::from_slice(&bytes).unwrap();
}

// Only confirms the route is registered and returns valid JSON - the
// kscreen-doctor -j parsing logic is already covered in depth in
// displays.rs (real fixtures, malformed JSON, etc).
#[tokio::test]
async fn displays_route_returns_a_json_array() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/displays")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let _displays: Vec<crate::displays::HostDisplay> = serde_json::from_slice(&bytes).unwrap();
}

#[tokio::test]
async fn health_route_returns_ok_true() {
    let response = test_app()
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["ok"], true);
}

// Doesn't assert on a specific MAC - that depends on the real network
// interfaces of whatever machine runs this test (detect_primary_mac
// itself is already covered against fixtures in tests/power.rs). Only
// confirms the route is wired up and returns the expected shape.
//
// There is deliberately no test here for POST /system/shutdown - see the
// comment on the `shutdown` handler in server.rs and the accepted test
// gap in docs/prd.md Phase 6.
#[tokio::test]
async fn system_mac_route_returns_valid_json() {
    let response = test_app()
        .oneshot(Request::builder().uri("/system/mac").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(body.get("mac").is_some());
}

// "when a sync call is received" - the handler sends an event to the
// channel on every GET /games, which lib.rs listens to in order to fire
// the real desktop notification (with a real AppHandle, out of scope
// for this test - testing just the signal, not the notification
// itself, avoids needing tauri::test::mock_builder generic over the
// Runtime).
#[tokio::test]
async fn games_route_sends_a_games_synced_event() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let session_state: SessionState = Arc::new(Mutex::new(None));
    let (clients_state, clients_file_path) = empty_clients();

    let _ = app(tx, session_state, ApolloBaseUrl("http://127.0.0.1:0".to_string()), clients_state, clients_file_path)
        .oneshot(
            Request::builder()
                .uri("/games")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(rx.try_recv(), Ok(RunnerEvent::GamesSynced));
}
