// Tracks which Decks have connected to this Runner (any HTTP request -
// /games, /session/register, /health, etc), for the "Clients" section of
// the Runner's own window (see moon_profile_runner/src/main.js). Persisted
// to disk (clients.json in the app's data dir, resolved by lib.rs, which
// has the AppHandle needed for that - this module only receives a plain
// Path, same decoupling as the rest of server.rs/session.rs not knowing
// about Tauri) so previously-seen Decks still show up (as unreachable)
// even after the Runner restarts and they haven't reconnected yet.

use axum::extract::{ConnectInfo, Extension, Request};
use axum::middleware::Next;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct KnownClient {
    pub ip: String,
    pub last_seen: String, // RFC3339, chrono::Local::now().to_rfc3339()
}

#[derive(Clone)]
pub struct ClientsState(pub Arc<Mutex<HashMap<String, KnownClient>>>);

// Newtype so it can be distinguished from other Extension<PathBuf> on the
// same Router - axum identifies extensions by TYPE, not by name (same
// reason ApolloBaseUrl wraps a plain String in session.rs).
#[derive(Clone)]
pub struct ClientsFilePath(pub PathBuf);

// Pure - the actual insert/update logic, kept separate from the
// clock/IO around it so it has a real unit test (tests/clients.rs).
pub fn upsert_client(clients: &mut HashMap<String, KnownClient>, ip: String, seen_at: String) {
    clients.insert(ip.clone(), KnownClient { ip, last_seen: seen_at });
}

// Fail-open (empty map) if the file doesn't exist yet (first run) or is
// malformed - same philosophy as list_displays()/list_host_games()
// elsewhere in this project: better to start with an empty known-clients
// list than to crash the Runner over a corrupted file.
pub fn load_known_clients(path: &Path) -> HashMap<String, KnownClient> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };
    let Ok(list) = serde_json::from_str::<Vec<KnownClient>>(&raw) else {
        return HashMap::new();
    };
    list.into_iter().map(|c| (c.ip.clone(), c)).collect()
}

pub fn save_known_clients(path: &Path, clients: &HashMap<String, KnownClient>) {
    let list: Vec<&KnownClient> = clients.values().collect();
    if let Ok(json) = serde_json::to_string_pretty(&list) {
        if let Err(error) = std::fs::write(path, json) {
            println!("[{}] [clients] failed to persist known clients to {}: {error}", crate::server::timestamp(), path.display());
        }
    }
}

async fn record_client_seen(state: &ClientsState, path: &Path, ip: String) {
    let mut guard = state.0.lock().await;
    upsert_client(&mut guard, ip, chrono::Local::now().to_rfc3339());
    save_known_clients(path, &guard);
}

// Runs on every request (see the layer order in server.rs::app) - uses
// Option<ConnectInfo<...>> (infallible) rather than ConnectInfo directly
// so requests built without it (every existing test in tests/server.rs,
// which call the Router directly via `oneshot` instead of a real
// accepted TCP connection) don't get rejected; in production,
// run_server's into_make_service_with_connect_info always provides it.
pub async fn record_client_middleware(
    Extension(state): Extension<ClientsState>,
    Extension(ClientsFilePath(path)): Extension<ClientsFilePath>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    request: Request,
    next: Next,
) -> Response {
    if let Some(ConnectInfo(addr)) = connect_info {
        record_client_seen(&state, &path, addr.ip().to_string()).await;
    }
    next.run(request).await
}

#[cfg(test)]
#[path = "tests/clients.rs"]
mod tests;
