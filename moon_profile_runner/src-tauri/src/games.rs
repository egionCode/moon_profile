// Enumera os jogos Steam instalados no host, pro Deck criar um atalho por
// jogo automaticamente (ver docs/prd.md, secao dos atalhos por jogo -
// Estagio A: so' jogos Steam reais, non-Steam fica pro Estagio B).
//
// Formato dos arquivos: VDF/KeyValues texto da Valve (mesmo formato de
// `libraryfolders.vdf` e `appmanifest_<id>.acf`) - usamos keyvalues-serde
// (Serde por cima do keyvalues-parser) em vez de escrever um parser na mao.

use keyvalues_serde::from_str;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const STEAM_STORE_BASE_URL: &str = "https://store.steampowered.com";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct HostGame {
    pub name: String,
    pub host_app_id: String,
    pub is_steam: bool,
}

// "libraryfolders" no topo do VDF envolve um objeto cujas chaves sao so'
// indices ("0", "1", ...) - HashMap<String, _> cobre isso sem precisar
// saber quantas bibliotecas existem de antemao.
#[derive(Deserialize)]
struct LibraryFolders(HashMap<String, LibraryFolderEntry>);

#[derive(Deserialize)]
struct LibraryFolderEntry {
    path: String,
}

// "AppState" no topo do .acf envolve soh os campos do proprio app - os
// campos extras do arquivo real (StateFlags, LastUpdated, etc) sao
// ignorados pelo serde automaticamente, nao precisa listar todos.
#[derive(Deserialize)]
struct AppManifest {
    appid: String,
    name: String,
}

fn default_steam_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".local/share/Steam")
}

fn parse_library_paths(vdf_content: &str) -> Vec<PathBuf> {
    match from_str::<LibraryFolders>(vdf_content) {
        Ok(folders) => folders.0.into_values().map(|entry| PathBuf::from(entry.path)).collect(),
        Err(_) => Vec::new(),
    }
}

// appmanifest_*.acf tambem existe pras ferramentas/runtimes que a Valve
// instala junto (Proton, Steam Linux Runtime, redistributables) - nao sao
// jogos, criar atalho pra eles seria um "jogo" chamado "Proton 9.0" na
// biblioteca. Nao ha' campo local confiavel que distinga isso (teria que
// consultar a Steam Web API) - filtro pratico por nome, que a Valve usa de
// forma consistente pra essas entradas.
fn is_valve_tooling(name: &str) -> bool {
    name.starts_with("Proton ")
        || name == "Steamworks Common Redistributables"
        || name.starts_with("Steam Linux Runtime")
}

fn parse_app_manifest(content: &str) -> Option<HostGame> {
    let manifest = from_str::<AppManifest>(content).ok()?;
    if is_valve_tooling(&manifest.name) {
        return None;
    }
    Some(HostGame {
        name: manifest.name,
        host_app_id: manifest.appid,
        is_steam: true,
    })
}

fn is_app_manifest_filename(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with("appmanifest_") && name.ends_with(".acf"))
        .unwrap_or(false)
}

// Recebe o caminho raiz da instalacao Steam do host (ex:
// ~/.local/share/Steam) pra poder ser testado com uma fixture, em vez de
// depender sempre do Steam de verdade instalado na maquina.
pub fn list_steam_games(steam_root: &Path) -> Vec<HostGame> {
    let library_folders_path = steam_root.join("steamapps").join("libraryfolders.vdf");
    let Ok(content) = fs::read_to_string(&library_folders_path) else {
        return Vec::new();
    };

    let mut games = Vec::new();
    for library_path in parse_library_paths(&content) {
        let steamapps_dir = library_path.join("steamapps");
        let Ok(entries) = fs::read_dir(&steamapps_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !is_app_manifest_filename(&path) {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&path) {
                if let Some(game) = parse_app_manifest(&content) {
                    games.push(game);
                }
            }
        }
    }
    games
}

// appmanifest_*.acf nao tem campo de categoria - "Aseprite", "Blender" etc
// (software real vendido na Steam, nao ferramenta interna da Valve) passam
// pelo filtro de nome do is_valve_tooling sem problema, porque nao sao
// Proton/redistributable. So' a API publica da Steam (appdetails) tem o
// dado real que falta - mas NAO e' o campo "type": validado direto contra
// a API de verdade que a Steam classifica Aseprite E Blender como
// type=="game" tambem (a Store nao tem um tipo dedicado pra "software",
// so' Jogos vs DLC vs demo etc - "type" nao distingue o que queremos).
// O sinal que de fato funciona e' "categories": todo jogo de verdade tem
// pelo menos um modo de jogo (Single-player/Multi-player/Co-op/PvP/...),
// ferramentas nunca tem (confirmado contra a API real: Aseprite/Blender
// sem "categories" nenhuma, SteamVR com categorias mas nenhuma de
// gameplay, contra Dota 2/No Man's Sky/Resident Evil 4 que sempre tem).
#[derive(Deserialize)]
struct AppDetailsEntry {
    success: bool,
    data: Option<AppDetailsData>,
}

#[derive(Deserialize)]
struct AppDetailsData {
    categories: Option<Vec<Category>>,
}

#[derive(Deserialize)]
struct Category {
    id: u32,
}

// IDs fixos do catalogo de categorias da Steam (estaveis ha' anos,
// confirmados contra a API real): 1=Multi-player, 2=Single-player,
// 9=Co-op, 20=MMO, 24=Shared/Split Screen, 27=Cross-Platform
// Multiplayer, 36=Online PvP, 38=Online Co-op, 47=LAN PvP, 48=LAN Co-op,
// 49=PvP. Qualquer um desses presente e' o sinal de "isso e' jogado", nao
// so' usado.
const GAMEPLAY_CATEGORY_IDS: &[u32] = &[1, 2, 9, 20, 24, 27, 36, 38, 47, 48, 49];

fn has_gameplay_category(categories: &Option<Vec<Category>>) -> bool {
    categories
        .as_ref()
        .map(|cats| cats.iter().any(|c| GAMEPLAY_CATEGORY_IDS.contains(&c.id)))
        .unwrap_or(false)
}

// None = nao deu pra descobrir (erro de rede, resposta inesperada, app
// nao encontrado) - o chamador decide o que fazer (fail-open: mantem o
// jogo na duvida, ver filter_to_games_only). Some(bool) = consulta deu
// certo, resultado da classificacao de verdade.
async fn is_actual_game(client: &reqwest::Client, base_url: &str, app_id: &str) -> Option<bool> {
    let url = format!("{base_url}/api/appdetails?appids={app_id}&filters=basic,categories");
    let response = client.get(&url).send().await.ok()?;
    let body: HashMap<String, AppDetailsEntry> = response.json().await.ok()?;
    let entry = body.get(app_id)?;
    if !entry.success {
        return None;
    }
    Some(has_gameplay_category(&entry.data.as_ref()?.categories))
}

// Consulta cada candidato concorrentemente (nao um por vez - seria lento
// pra uma biblioteca com dezenas de jogos) e mantem so' os que tem
// categoria de gameplay. Fail-open: se a consulta falhar (rede fora,
// timeout, resposta inesperada) o jogo fica na lista mesmo assim - melhor
// mostrar um software errado ocasional do que esconder um jogo de verdade
// por causa de um problema de rede passageiro.
async fn filter_to_games_only(candidates: Vec<HostGame>, base_url: &str) -> Vec<HostGame> {
    let client = reqwest::Client::new();
    let mut tasks = tokio::task::JoinSet::new();
    for game in candidates {
        let client = client.clone();
        let base_url = base_url.to_string();
        tasks.spawn(async move {
            let keep = is_actual_game(&client, &base_url, &game.host_app_id).await.unwrap_or(true);
            (game, keep)
        });
    }

    let mut result = Vec::new();
    while let Some(joined) = tasks.join_next().await {
        if let Ok((game, keep)) = joined {
            if keep {
                result.push(game);
            }
        }
    }
    result
}

pub async fn list_host_games() -> Vec<HostGame> {
    let candidates = list_steam_games(&default_steam_root());
    filter_to_games_only(candidates, STEAM_STORE_BASE_URL).await
}

#[cfg(test)]
#[path = "tests/games.rs"]
mod tests;
