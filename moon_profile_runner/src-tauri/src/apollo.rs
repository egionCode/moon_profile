// Cliente minimo pra API REST do Apollo, usado so' pelo fechamento de
// sessao (ver session.rs) - o Runner roda no MESMO host que o Apollo,
// entao fala com ele via loopback, sem precisar do endereco LAN que o
// Deck usa.
//
// Mesmo comportamento que moonprofile_core.py:ApolloClient (Python, usado
// por runner.py/main.py): login por cookie de sessao (POST /api/login,
// nao HTTP Basic Auth - este fork nao segue o que a doc antiga do
// Sunshine descreve). O Apollo NAO tem prep-cmd configurado (decisao
// explicita - ver session.rs/moonprofile_core.py:build_display_commands)
// - POST /api/apps/close so' derruba a conexao/stream em si, matar o
// jogo e trocar a tela e' 100% responsabilidade do Runner.
//
// Certificado autoassinado do Apollo - aceita certificado invalido de
// proposito (danger_accept_invalid_certs), igual o Python faz com
// ssl.CERT_NONE.

use reqwest::{Client, StatusCode};
use serde_json::json;

pub const DEFAULT_APOLLO_BASE_URL: &str = "https://127.0.0.1:47990";

fn build_client() -> Result<Client, String> {
    Client::builder()
        .cookie_store(true) // guarda o cookie "auth" devolvido pelo /api/login
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("Falha ao construir cliente HTTP pro Apollo: {e}"))
}

fn classify_status(status: StatusCode) -> Result<(), String> {
    if status.is_success() {
        return Ok(());
    }
    if status == StatusCode::UNAUTHORIZED {
        return Err("Usuario ou senha do Apollo incorretos".to_string());
    }
    Err(format!("Apollo respondeu com erro inesperado (HTTP {})", status.as_u16()))
}

fn unreachable_message(base_url: &str) -> String {
    format!("Nao consegui alcancar o Apollo em {base_url} - confira se o host esta ligado")
}

async fn login(client: &Client, base_url: &str, username: &str, password: &str) -> Result<(), String> {
    let resp = client
        .post(format!("{base_url}/api/login"))
        .json(&json!({"username": username, "password": password}))
        .send()
        .await
        .map_err(|_| unreachable_message(base_url))?;
    classify_status(resp.status())
}

// base_url e' parametro (em vez de sempre DEFAULT_APOLLO_BASE_URL) pra
// permitir apontar pra um wiremock::MockServer nos testes, mesmo padrao
// ja usado em games.rs (STEAM_STORE_BASE_URL).
pub async fn close_session_at(base_url: &str, username: &str, password: &str) -> Result<(), String> {
    let client = build_client()?;
    login(&client, base_url, username, password).await?;

    let close_resp = client
        .post(format!("{base_url}/api/apps/close"))
        .json(&json!({}))
        .send()
        .await
        .map_err(|_| unreachable_message(base_url))?;
    classify_status(close_resp.status())?;

    Ok(())
}

#[cfg(test)]
mod tests {
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
}
