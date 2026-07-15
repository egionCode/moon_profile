// Minimal client for Apollo's REST API, used only by session closing
// (see session.rs) - the Runner runs on the SAME host as Apollo, so it
// talks to it via loopback, without needing the LAN address the Deck
// uses.
//
// Same behavior as moonprofile_core.py:ApolloClient (Python, used
// by runner.py/main.py): login via session cookie (POST /api/login,
// not HTTP Basic Auth - this fork doesn't follow what the old Sunshine
// docs describe). Apollo does NOT have prep-cmd configured (explicit
// decision, see session.rs/moonprofile_core.py:build_display_commands)
// - POST /api/apps/close only drops the connection/stream itself, killing
// the game and switching the screen is 100% the Runner's responsibility.
//
// Apollo's self-signed certificate: accepts an invalid certificate on
// purpose (danger_accept_invalid_certs), just like the Python code does
// with ssl.CERT_NONE.

use reqwest::{Client, StatusCode};
use serde_json::json;

pub const DEFAULT_APOLLO_BASE_URL: &str = "https://127.0.0.1:47990";

fn build_client() -> Result<Client, String> {
    Client::builder()
        .cookie_store(true) // stores the "auth" cookie returned by /api/login
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("Failed to build HTTP client for Apollo: {e}"))
}

fn classify_status(status: StatusCode) -> Result<(), String> {
    if status.is_success() {
        return Ok(());
    }
    if status == StatusCode::UNAUTHORIZED {
        return Err("Incorrect Apollo username or password".to_string());
    }
    Err(format!("Apollo responded with an unexpected error (HTTP {})", status.as_u16()))
}

fn unreachable_message(base_url: &str) -> String {
    format!("Could not reach Apollo at {base_url} - check whether the host is powered on")
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

// base_url is a parameter (instead of always DEFAULT_APOLLO_BASE_URL) to
// allow pointing at a wiremock::MockServer in tests, same pattern
// already used in games.rs (STEAM_STORE_BASE_URL).
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

// Tests physically separated (src/tests/apollo.rs) - still part of the
// `apollo` module (the #[path] attribute only changes WHERE the file
// lives, not its position in the module tree), so `super::*` there
// still sees everything private here, same as before.
#[cfg(test)]
#[path = "tests/apollo.rs"]
mod tests;
