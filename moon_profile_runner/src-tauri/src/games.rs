// Enumerates the Steam games installed on the host, so the Deck can
// automatically create a shortcut per game (see docs/prd.md, per-game
// shortcuts section - Stage A: only real Steam games, non-Steam is
// Stage B).
//
// File format: Valve's text VDF/KeyValues format (same format as
// `libraryfolders.vdf` and `appmanifest_<id>.acf`) - we use
// keyvalues-serde (Serde on top of keyvalues-parser) instead of writing
// a parser by hand.

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

// The top-level "libraryfolders" in the VDF wraps an object whose keys
// are just indices ("0", "1", ...) - HashMap<String, _> covers this
// without needing to know how many libraries exist upfront.
#[derive(Deserialize)]
struct LibraryFolders(HashMap<String, LibraryFolderEntry>);

#[derive(Deserialize)]
struct LibraryFolderEntry {
    path: String,
}

// The top-level "AppState" in the .acf wraps only the fields of the app
// itself - the extra fields in the real file (StateFlags, LastUpdated,
// etc) are ignored by serde automatically, no need to list them all.
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

// appmanifest_*.acf also exists for the tools/runtimes Valve installs
// alongside games (Proton, Steam Linux Runtime, redistributables) -
// they aren't games, creating a shortcut for them would produce a
// "game" called "Proton 9.0" in the library. There's no reliable local
// field to tell them apart (you'd have to query the Steam Web API) - a
// practical name-based filter, which Valve uses consistently for these
// entries.
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

// Takes the host's Steam install root path (e.g. ~/.local/share/Steam)
// so it can be tested with a fixture, instead of always depending on a
// real Steam installation on the machine.
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

// Non-Steam shortcuts (Stage B): live in a separate binary-VDF file per
// Steam user profile, not under steamapps/ at all -
// userdata/<user_id>/config/shortcuts.vdf. Enumerates every profile
// found (a host normally only has one, but nothing stops there being
// old/extra ones) - fail-open per file, same philosophy as
// list_steam_games (a missing/corrupt shortcuts.vdf just means "no
// non-Steam games from this profile", not an error).
fn shortcuts_vdf_paths(steam_root: &Path) -> Vec<PathBuf> {
    let userdata_dir = steam_root.join("userdata");
    let Ok(entries) = fs::read_dir(&userdata_dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path().join("config").join("shortcuts.vdf"))
        .filter(|path| path.is_file())
        .collect()
}

// Pure - parses the shortcuts.vdf bytes already read from disk, kept
// separate from the file I/O so it can be tested against a hand-built
// fixture without needing a real Steam profile on the test machine.
//
// "appid" is stored as a SIGNED 32-bit int (steam-vdf-parser's I32),
// but Steam treats it as unsigned everywhere else it matters (the
// compatdata/<id> folder name) - confirmed against a real shortcuts.vdf
// on this machine: an entry with appid=I32(-408863985) has its game's
// compatdata folder named "3886103311", which is exactly
// `(-408863985i32) as u32`.
//
// That unsigned appid alone is NOT enough to launch the shortcut via
// steam://rungameid/<id>, though: real bug found running this end to
// end (Deck -> Apollo -> `steam steam://rungameid/<appid>` on the host)
// - Steam's console_log.txt showed `ExecuteSteamURL` receiving the
// call, but no subsequent `GameAction ... LaunchApp` for that AppID
// (contrast with a manual "Play" click on the same shortcut, which does
// log the full LaunchApp task sequence). The URL was silently ignored:
// Steam disambiguates a real game's AppID from a shortcut's via the
// 64-bit GameID's upper bits, not the raw 32-bit appid alone. For real
// Steam games GameID == appid (fits in 32 bits, type "App" = 0), but a
// shortcut's GameID is `(appid as u64) << 32 | 0x02000000` (type
// "Shortcut" = 0x02) - the same encoding used by community tools that
// generate non-Steam shortcuts (e.g. Boilr, Steam ROM Manager). Baking
// this into host_app_id here (instead of threading is_steam down to
// runner.py) keeps runner.py's cmd-building code identical for both
// cases, see moonprofile_core.py.
//
// Only "AppName"/"appid"/"IsHidden" are read - Exe/StartDir/icon/etc
// aren't needed here.
fn parse_shortcuts_vdf(bytes: &[u8]) -> Vec<HostGame> {
    let Ok(vdf) = steam_vdf_parser::parse_shortcuts(bytes) else {
        return Vec::new();
    };
    let Some(shortcuts) = vdf.get_obj(&["shortcuts"]) else {
        return Vec::new();
    };

    shortcuts
        .values()
        .filter_map(|entry| {
            let is_hidden = entry.get_i32(&["IsHidden"]).unwrap_or(0) != 0;
            if is_hidden {
                return None;
            }
            let name = entry.get_str(&["AppName"])?.to_string();
            let appid = entry.get_i32(&["appid"])? as u32;
            let game_id = ((appid as u64) << 32) | 0x0200_0000;
            Some(HostGame { name, host_app_id: game_id.to_string(), is_steam: false })
        })
        .collect()
}

pub fn list_non_steam_games(steam_root: &Path) -> Vec<HostGame> {
    shortcuts_vdf_paths(steam_root)
        .into_iter()
        .filter_map(|path| fs::read(&path).ok())
        .flat_map(|bytes| parse_shortcuts_vdf(&bytes))
        .collect()
}

// appmanifest_*.acf has no category field - "Aseprite", "Blender" etc
// (real software sold on Steam, not an internal Valve tool) pass through
// is_valve_tooling's name filter without issue, because they aren't
// Proton/redistributables. Only Steam's public API (appdetails) has the
// real data that's missing - but it's NOT the "type" field: validated
// directly against the real API that Steam classifies both Aseprite and
// Blender as type=="game" too (the Store has no dedicated type for
// "software", only Game vs DLC vs demo etc - "type" doesn't distinguish
// what we want). The signal that actually works is "categories": every
// real game has at least one game mode (Single-player/Multi-player/
// Co-op/PvP/...), tools never do (confirmed against the real API:
// Aseprite/Blender have no "categories" at all, SteamVR has categories
// but none of them gameplay, versus Dota 2/No Man's Sky/Resident Evil 4
// which always have one).
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

// Fixed IDs from Steam's category catalog (stable for years, confirmed
// against the real API): 1=Multi-player, 2=Single-player, 9=Co-op,
// 20=MMO, 24=Shared/Split Screen, 27=Cross-Platform Multiplayer,
// 36=Online PvP, 38=Online Co-op, 47=LAN PvP, 48=LAN Co-op, 49=PvP. Any
// of these being present is the signal that "this is played", not just
// used.
const GAMEPLAY_CATEGORY_IDS: &[u32] = &[1, 2, 9, 20, 24, 27, 36, 38, 47, 48, 49];

fn has_gameplay_category(categories: &Option<Vec<Category>>) -> bool {
    categories
        .as_ref()
        .map(|cats| cats.iter().any(|c| GAMEPLAY_CATEGORY_IDS.contains(&c.id)))
        .unwrap_or(false)
}

// None = couldn't tell (network error, unexpected response, app not
// found) - the caller decides what to do (fail-open: keeps the game
// when in doubt, see filter_to_games_only). Some(bool) = the query
// succeeded, real classification result.
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

// Queries each candidate concurrently (not one at a time - that would be
// slow for a library with dozens of games) and keeps only the ones with
// a gameplay category. Fail-open: if the query fails (network down,
// timeout, unexpected response) the game stays in the list anyway -
// better to occasionally show a piece of software by mistake than to
// hide a real game because of a transient network issue.
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
    let steam_root = default_steam_root();
    let candidates = list_steam_games(&steam_root);
    let mut games = filter_to_games_only(candidates, STEAM_STORE_BASE_URL).await;
    // Non-Steam shortcuts don't have a real Store appid to query
    // (filter_to_games_only's categories check would just 404 against
    // Steam's API for these), and the user already chose to add each one
    // as a shortcut deliberately - no gameplay-category filtering needed.
    games.extend(list_non_steam_games(&steam_root));
    games
}

#[cfg(test)]
#[path = "tests/games.rs"]
mod tests;
