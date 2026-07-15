use super::*;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn succeeds_when_login_and_close_both_return_2xx() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/login"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/apps/close"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let result = close_session_at(&mock_server.uri(), "user", "pass").await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn reports_wrong_credentials_on_401_from_login() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/login"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&mock_server)
        .await;

    let result = close_session_at(&mock_server.uri(), "user", "wrong").await;

    assert_eq!(result, Err("Incorrect Apollo username or password".to_string()));
}

#[tokio::test]
async fn reports_unexpected_status_from_close() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/login"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/apps/close"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let result = close_session_at(&mock_server.uri(), "user", "pass").await;

    assert_eq!(result, Err("Apollo responded with an unexpected error (HTTP 500)".to_string()));
}

#[tokio::test]
async fn reports_unreachable_host_when_connection_fails() {
    // port 0 never accepts a connection - simulates Apollo being
    // powered off/unreachable without relying on a long timeout.
    let result = close_session_at("http://127.0.0.1:0", "user", "pass").await;

    assert_eq!(
        result,
        Err("Could not reach Apollo at http://127.0.0.1:0 - check whether the host is powered on".to_string())
    );
}
