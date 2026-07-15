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

    assert_eq!(result, Err("Usuario ou senha do Apollo incorretos".to_string()));
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

    assert_eq!(result, Err("Apollo respondeu com erro inesperado (HTTP 500)".to_string()));
}

#[tokio::test]
async fn reports_unreachable_host_when_connection_fails() {
    // porta 0 nunca aceita conexao - simula o Apollo desligado/
    // inalcancavel sem depender de timeout longo.
    let result = close_session_at("http://127.0.0.1:0", "user", "pass").await;

    assert_eq!(
        result,
        Err("Nao consegui alcancar o Apollo em http://127.0.0.1:0 - confira se o host esta ligado".to_string())
    );
}
