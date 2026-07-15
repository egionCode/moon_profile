// Lista os monitores/outputs de tela do host (via kscreen-doctor -j) pra
// alimentar o Deck com opcoes de verdade em vez do usuario ter que
// digitar o nome do output ("HDMI-A-1", "DP-3", etc) na mao - a UI usa
// isso pra popular um <select> pro target_output e uma lista dinamica
// pro disable_outputs (ver src/api.ts/ProfileEditor no lado Decky).

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct HostDisplay {
    pub name: String,
    pub connected: bool,
    pub enabled: bool,
}

// So' os campos que nos interessam do JSON completo do kscreen-doctor -
// serde ignora o resto (modes, icc profile, etc) automaticamente.
#[derive(Deserialize)]
struct KscreenOutput {
    name: String,
    connected: bool,
    enabled: bool,
}

#[derive(Deserialize)]
struct KscreenConfig {
    outputs: Vec<KscreenOutput>,
}

// Pura - parseia o JSON que kscreen-doctor -j imprime. Separada da
// chamada de processo de verdade pra poder testar contra fixtures reais
// (capturadas do device) sem depender do kscreen-doctor estar instalado
// na maquina rodando o teste.
fn parse_kscreen_json(raw: &str) -> Vec<HostDisplay> {
    let Ok(config) = serde_json::from_str::<KscreenConfig>(raw) else {
        return Vec::new();
    };
    config
        .outputs
        .into_iter()
        .map(|o| HostDisplay { name: o.name, connected: o.connected, enabled: o.enabled })
        .collect()
}

// Fail-open (lista vazia) se o kscreen-doctor nao existir, falhar, ou
// devolver algo inesperado - mesma filosofia do resto do projeto (ex:
// filter_to_games_only em games.rs): melhor a UI mostrar uma lista vazia
// (usuario ainda pode digitar manualmente como fallback) do que travar.
pub fn list_displays() -> Vec<HostDisplay> {
    let Ok(output) = std::process::Command::new("kscreen-doctor").arg("-j").output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let Ok(raw) = String::from_utf8(output.stdout) else {
        return Vec::new();
    };
    parse_kscreen_json(&raw)
}

#[cfg(test)]
#[path = "tests/displays.rs"]
mod tests;
