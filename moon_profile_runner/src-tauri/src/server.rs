// Servidor HTTP embutido, falado pelo plugin Decky (moon_profile_decky) e
// por runner.py (Deck) - registra/lista jogos do host e cuida do fim de
// sessao (ver session.rs) sem depender do Apollo, que entra em modo
// "placebo" depois do auto-detach do stream_game e nunca mais reporta o
// app como parado (ver docs/prd.md Fase 5). O SO nao mente sobre o
// processo, mesmo quando o Apollo mente.
//
// Sem autenticacao - servidor aberto na rede local (decisao explicita:
// numa LAN domestica ja confiavel, o atrito de colar um token na config
// do Deck nao compensa o ganho de seguranca).

use axum::{extract::Extension, routing::get, routing::post, Json, Router};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tokio::sync::mpsc;

use crate::displays::{list_displays, HostDisplay};
use crate::games::{list_host_games, HostGame};
use crate::session::{close_session_now, register_session, ApolloBaseUrl, SessionState};

// So' pros prints de diagnostico (register/watchdog/close) - sem isso os
// logs no console do "tauri dev" nao davam pra saber EM QUE SEGUNDO cada
// coisa aconteceu (relevante pro watchdog, que roda a cada poucos
// segundos).
pub(crate) fn timestamp() -> String {
    chrono::Local::now().format("%H:%M:%S%.3f").to_string()
}

// Eventos que server.rs/session.rs sinalizam pra lib.rs (que TEM o
// AppHandle de verdade) disparar notificacoes de desktop - os handlers
// axum em si nao conhecem Tauri/AppHandle nenhum. Decoupled assim de
// proposito: usar AppHandle direto aqui exigiria testes com
// tauri::test::mock_builder (generico sobre o Runtime, muito mais
// complexo pra testar um pulso) - um canal simples e' trivial de testar
// (so' cria um par e ignora o receiver).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerEvent {
    GamesSynced,
    SessionClosed,
}
pub type EventNotifier = mpsc::UnboundedSender<RunnerEvent>;

// "AppId=<id>" tem que ser seguido de um NAO-digito (ou fim de string) -
// um contains() puro faria "AppId=900004" bater como prefixo de
// "AppId=9000044" (falso positivo com outro AppId que so' compartilha o
// prefixo numerico). Coberto por
// `does_not_match_a_different_app_id_with_a_shared_prefix` abaixo.
fn cmd_arg_matches_app_id(arg: &str, needle: &str) -> bool {
    let Some(pos) = arg.find(needle) else {
        return false;
    };
    match arg[pos + needle.len()..].chars().next() {
        Some(c) => !c.is_ascii_digit(),
        None => true,
    }
}

// Mesma convencao de identificar o processo certo que main.py:_build_prep_cmd
// ja usa no "pkill -f AppId=<id>" do undo - so que aqui e' LEITURA, nao kill.
// pub(crate) porque session.rs (o watchdog de fechamento autonomo) usa a
// mesma checagem, sem precisar passar por HTTP.
pub(crate) fn is_app_id_running(app_id: &str) -> bool {
    let needle = format!("AppId={app_id}");
    let mut sys = System::new();
    // refresh_processes() (o metodo de conveniencia) usa um ProcessRefreshKind
    // padrao que NAO inclui cmd (so memoria/cpu/disco/exe) - cmd() sempre
    // voltava vazio sem isso, entao o match nunca batia (bug real,
    // confirmado rodando de verdade: processo existia, cmdline batia, e
    // mesmo assim "running: false"). refresh_processes_specifics com
    // with_cmd(Always) e' o jeito certo de pedir esse dado. Regressao real
    // ja encontrada uma vez, nao repetir.
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::new().with_cmd(UpdateKind::Always),
    );

    sys.processes().values().any(|process| {
        process
            .cmd()
            .iter()
            .any(|arg| cmd_arg_matches_app_id(&arg.to_string_lossy(), &needle))
    })
}

// "quando receber uma chamada de sincronia" - dispara o pulso ANTES de
// montar a resposta, no exato momento em que o Deck de fato pediu a lista.
async fn games(Extension(notify): Extension<EventNotifier>) -> Json<Vec<HostGame>> {
    println!("[{}] [server] GET /games (sincronizacao pedida pelo Deck)", timestamp());
    let _ = notify.send(RunnerEvent::GamesSynced);
    Json(list_host_games().await)
}

// GET /displays - lista os monitores/outputs do host (via kscreen-doctor)
// pra UI do Deck popular um select em vez do usuario digitar o nome do
// output na mao (ver moon_profile_decky/src/ProfileEditor.tsx).
async fn displays() -> Json<Vec<HostDisplay>> {
    Json(list_displays())
}

pub fn app(notify: EventNotifier, session_state: SessionState, apollo_base_url: ApolloBaseUrl) -> Router {
    Router::new()
        .route("/games", get(games))
        .route("/displays", get(displays))
        .route("/session/register", post(register_session))
        .route("/session/close", post(close_session_now))
        .layer(Extension(notify))
        .layer(Extension(session_state))
        .layer(Extension(apollo_base_url))
}

pub async fn run_server(notify: EventNotifier, session_state: SessionState, apollo_base_url: ApolloBaseUrl) {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:47991")
        .await
        .expect("failed to bind runner HTTP server to port 47991");
    println!("[{}] [server] MoonProfile Runner escutando em 0.0.0.0:47991", timestamp());

    axum::serve(listener, app(notify, session_state, apollo_base_url))
        .await
        .expect("runner HTTP server crashed");
}

#[cfg(test)]
#[path = "tests/server.rs"]
mod tests;
