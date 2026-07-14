// Servidor HTTP embutido, falado pelo plugin Decky (moon_profile_decky) pra
// checar se um jogo ainda esta rodando no host - resolve a deteccao de fim
// de sessao que o Apollo nao consegue fazer sozinho (auto-detach do
// stream_game entra em modo "placebo" e nunca mais reporta o app como
// parado, ver docs/prd.md Fase 5). O SO nao mente sobre o processo, mesmo
// quando o Apollo mente.
//
// Sem autenticacao - servidor aberto na rede local (decisao explicita:
// numa LAN domestica ja confiavel, o atrito de colar um token na config
// do Deck nao compensa o ganho de seguranca).

use axum::{extract::Query, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[derive(Deserialize)]
struct StatusQuery {
    app_id: String,
}

#[derive(Serialize, Deserialize)]
struct StatusResponse {
    running: bool,
}

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
async fn session_status(Query(query): Query<StatusQuery>) -> Json<StatusResponse> {
    let needle = format!("AppId={}", query.app_id);
    let mut sys = System::new();
    // refresh_processes() (o metodo de conveniencia) usa um ProcessRefreshKind
    // padrao que NAO inclui cmd (so memoria/cpu/disco/exe) - cmd() sempre
    // voltava vazio sem isso, entao o match nunca batia (bug real,
    // confirmado rodando de verdade: processo existia, cmdline batia, e
    // mesmo assim "running: false"). refresh_processes_specifics com
    // with_cmd(Always) e' o jeito certo de pedir esse dado. Coberto pelo
    // teste `reports_running_when_a_matching_process_is_alive` abaixo -
    // regressao real ja encontrada uma vez, nao repetir.
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::new().with_cmd(UpdateKind::Always),
    );

    let running = sys.processes().values().any(|process| {
        process
            .cmd()
            .iter()
            .any(|arg| cmd_arg_matches_app_id(&arg.to_string_lossy(), &needle))
    });

    Json(StatusResponse { running })
}

fn app() -> Router {
    Router::new().route("/session/status", get(session_status))
}

pub async fn run_server() {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:47991")
        .await
        .expect("failed to bind runner HTTP server to port 47991");

    axum::serve(listener, app())
        .await
        .expect("runner HTTP server crashed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::process::Child;
    use std::time::Duration;
    use tower::ServiceExt;

    // Unitario e rapido - nao precisa de processo de verdade, so cobre a
    // logica de match pura (os casos de borda do prefixo compartilhado).
    #[test]
    fn cmd_arg_matches_app_id_cases() {
        assert!(cmd_arg_matches_app_id("AppId=42", "AppId=42"));
        assert!(cmd_arg_matches_app_id("SteamLaunch AppId=42 --", "AppId=42"));
        assert!(!cmd_arg_matches_app_id("AppId=420", "AppId=42"));
        assert!(!cmd_arg_matches_app_id("AppId=4", "AppId=42"));
        assert!(!cmd_arg_matches_app_id("nada a ver", "AppId=42"));
    }

    // Spawna um processo real com "AppId=<id>" no cmdline (via "exec -a",
    // mesmo truque usado manualmente pra validar o bug do refresh_processes)
    // - testa contra o SO de verdade, nao um mock de sysinfo. Mata o
    // processo no Drop mesmo se o teste falhar no meio.
    struct FakeGameProcess {
        child: Child,
    }

    impl FakeGameProcess {
        fn spawn(app_id: &str) -> Self {
            let marker = format!("AppId={app_id}");
            let child = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("exec -a \"{marker}\" sleep 30"))
                .spawn()
                .expect("falha ao spawnar processo fake pro teste");
            Self { child }
        }
    }

    impl Drop for FakeGameProcess {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    async fn query_status(app_id: &str) -> (StatusCode, StatusResponse) {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri(format!("/session/status?app_id={app_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: StatusResponse = serde_json::from_slice(&bytes).unwrap();
        (status, body)
    }

    // IDs de teste bem distintos (900xxx) pra nao colidir por acaso com um
    // AppId de jogo de verdade rodando na maquina de dev enquanto testa.

    #[tokio::test]
    async fn reports_not_running_when_no_matching_process_exists() {
        let (status, body) = query_status("900001").await;
        assert_eq!(status, StatusCode::OK);
        assert!(!body.running);
    }

    #[tokio::test]
    async fn reports_running_when_a_matching_process_is_alive() {
        let fake = FakeGameProcess::spawn("900002");
        tokio::time::sleep(Duration::from_millis(200)).await;

        let (status, body) = query_status("900002").await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.running);
        drop(fake);
    }

    #[tokio::test]
    async fn reports_not_running_again_after_the_process_exits() {
        let fake = FakeGameProcess::spawn("900003");
        tokio::time::sleep(Duration::from_millis(200)).await;
        drop(fake); // mata e espera terminar antes de perguntar de novo
        tokio::time::sleep(Duration::from_millis(200)).await;

        let (status, body) = query_status("900003").await;

        assert_eq!(status, StatusCode::OK);
        assert!(!body.running);
    }

    #[tokio::test]
    async fn does_not_match_a_different_app_id_with_a_shared_prefix() {
        // "900004" nao deveria bater com um processo que tem "AppId=9000044"
        // (prefixo compartilhado) - contains() e' substring, entao esse
        // caso confirma que o formato "AppId=<id>" exato (sem separador
        // depois) nao gera falso positivo aqui na pratica: o cmdline real
        // tem espaco/fim de string logo depois do id, o pkill/match usa
        // exatamente essa mesma suposicao no lado do main.py.
        let fake = FakeGameProcess::spawn("9000044");
        tokio::time::sleep(Duration::from_millis(200)).await;

        let (status, body) = query_status("900004").await;

        assert_eq!(status, StatusCode::OK);
        assert!(!body.running);
        drop(fake);
    }
}
