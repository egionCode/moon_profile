use super::*;

// Creates a fake "Steam install" in a temporary folder - 2 libraries,
// one valid app in each, one junk file that isn't an appmanifest
// (shouldn't crash nor show up in the result).
fn write_fixture(root: &Path) {
    let steamapps = root.join("steamapps");
    fs::create_dir_all(&steamapps).unwrap();

    let lib2 = root.join("lib2");
    fs::create_dir_all(lib2.join("steamapps")).unwrap();

    let library_folders = format!(
        r#""libraryfolders"
{{
    "0"
    {{
        "path"		"{}"
    }}
    "1"
    {{
        "path"		"{}"
    }}
}}
"#,
        root.display(),
        lib2.display(),
    );
    fs::write(steamapps.join("libraryfolders.vdf"), library_folders).unwrap();

    fs::write(
        steamapps.join("appmanifest_2050650.acf"),
        r#""AppState"
{
    "appid"		"2050650"
    "universe"		"1"
    "name"		"Resident Evil 4"
    "StateFlags"		"4"
}
"#,
    )
    .unwrap();

    fs::write(
        lib2.join("steamapps").join("appmanifest_570.acf"),
        r#""AppState"
{
    "appid"		"570"
    "name"		"Dota 2"
}
"#,
    )
    .unwrap();

    // file that isn't an appmanifest - shouldn't be read nor break anything
    fs::write(steamapps.join("libraryfolder.vdf.bak"), "some junk").unwrap();

    // Valve tooling (not a game) - shouldn't show up in the result
    fs::write(
        steamapps.join("appmanifest_2805730.acf"),
        r#""AppState"
{
    "appid"		"2805730"
    "name"		"Proton 9.0"
}
"#,
    )
    .unwrap();
}

#[test]
fn lists_games_from_all_library_folders() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixture(tmp.path());

    let mut games = list_steam_games(tmp.path());
    // by name, not by host_app_id - "570" > "2050650" as a string
    // (lexicographic comparison, not numeric).
    games.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(
        games,
        vec![
            HostGame { name: "Dota 2".into(), host_app_id: "570".into(), is_steam: true },
            HostGame { name: "Resident Evil 4".into(), host_app_id: "2050650".into(), is_steam: true },
        ]
    );
}

#[test]
fn returns_empty_when_libraryfolders_vdf_is_missing() {
    let tmp = tempfile::tempdir().unwrap();
    // no fixture written - steam_root doesn't even have steamapps/
    assert_eq!(list_steam_games(tmp.path()), Vec::new());
}

#[test]
fn ignores_files_that_are_not_app_manifests() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixture(tmp.path());

    let games = list_steam_games(tmp.path());
    assert!(games.iter().all(|g| g.name != "some junk"));
}

#[test]
fn excludes_valve_tooling_entries() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixture(tmp.path());

    // fixture includes "Proton 9.0" (appid 2805730) - a real finding
    // from running against this machine's real Steam install: without
    // this filter, "Proton 9.0"/"Steamworks Common Redistributables"/etc
    // showed up in the list as if they were games.
    let games = list_steam_games(tmp.path());
    assert!(games.iter().all(|g| g.host_app_id != "2805730"));
    assert!(games.iter().all(|g| !g.name.starts_with("Proton")));
}

#[test]
fn is_valve_tooling_cases() {
    assert!(is_valve_tooling("Proton 9.0"));
    assert!(is_valve_tooling("Proton Experimental"));
    assert!(is_valve_tooling("Steamworks Common Redistributables"));
    assert!(is_valve_tooling("Steam Linux Runtime - Sniper"));
    assert!(!is_valve_tooling("Resident Evil 4"));
    assert!(!is_valve_tooling("Protontricks Helper")); // doesn't start with "Proton " (with a space)
}

#[test]
fn has_gameplay_category_cases() {
    assert!(has_gameplay_category(&Some(vec![Category { id: 2 }]))); // Single-player
    assert!(has_gameplay_category(&Some(vec![Category { id: 22 }, Category { id: 1 }]))); // mixed, but has Multi-player
    assert!(!has_gameplay_category(&Some(vec![Category { id: 22 }, Category { id: 30 }]))); // only achievements/workshop
    assert!(!has_gameplay_category(&Some(Vec::new())));
    assert!(!has_gameplay_category(&None)); // Aseprite/Blender: no categories at all
}

// wiremock instead of the real Steam API - tests can't depend on a real
// network (slow, unstable, subject to rate limiting).
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn filter_to_games_only_excludes_entries_without_a_gameplay_category() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/appdetails"))
        .and(query_param("appids", "111"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "111": {"success": true, "data": {"categories": [{"id": 2, "description": "Single-player"}]}}
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/appdetails"))
        .and(query_param("appids", "222"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "222": {"success": true, "data": {"categories": null}}
        })))
        .mount(&mock_server)
        .await;

    let candidates = vec![
        HostGame { name: "Real Game".into(), host_app_id: "111".into(), is_steam: true },
        HostGame { name: "Some Program".into(), host_app_id: "222".into(), is_steam: true },
    ];

    let result = filter_to_games_only(candidates, &mock_server.uri()).await;

    assert_eq!(result, vec![HostGame { name: "Real Game".into(), host_app_id: "111".into(), is_steam: true }]);
}

#[tokio::test]
async fn filter_to_games_only_fails_open_when_api_is_unreachable() {
    // port 0 never accepts a connection - simulates the API being
    // unreachable without depending on a real network nor a long timeout.
    let candidates = vec![HostGame { name: "No Network".into(), host_app_id: "333".into(), is_steam: true }];

    let result = filter_to_games_only(candidates, "http://127.0.0.1:0").await;

    assert_eq!(result, vec![HostGame { name: "No Network".into(), host_app_id: "333".into(), is_steam: true }]);
}

// Hand-builds the exact byte layout confirmed against a real
// shortcuts.vdf captured on a real machine (see games.rs's
// parse_shortcuts_vdf doc comment): a top-level "shortcuts" object
// containing one nested object per shortcut, keyed by index ("0", "1",
// ...), each holding [type byte]["field name"\0][value] triples and
// terminated by a single 0x08. Type 0x02 = signed i32 (4 bytes LE),
// 0x01 = null-terminated string. Only the fields parse_shortcuts_vdf
// actually reads are included - Steam's real files have more (Exe,
// StartDir, icon, ...) but the parser doesn't require them.
fn shortcut_entry_bytes(index: &str, appid: i32, name: &str, is_hidden: i32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(0x00);
    bytes.extend_from_slice(index.as_bytes());
    bytes.push(0x00);
    bytes.push(0x02);
    bytes.extend_from_slice(b"appid");
    bytes.push(0x00);
    bytes.extend_from_slice(&appid.to_le_bytes());
    bytes.push(0x01);
    bytes.extend_from_slice(b"AppName");
    bytes.push(0x00);
    bytes.extend_from_slice(name.as_bytes());
    bytes.push(0x00);
    bytes.push(0x02);
    bytes.extend_from_slice(b"IsHidden");
    bytes.push(0x00);
    bytes.extend_from_slice(&is_hidden.to_le_bytes());
    bytes.push(0x08); // end of this shortcut's object
    bytes
}

fn shortcuts_vdf_bytes(entries: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(0x00);
    bytes.extend_from_slice(b"shortcuts");
    bytes.push(0x00);
    for entry in entries {
        bytes.extend_from_slice(entry);
    }
    bytes.push(0x08); // end of the "shortcuts" container
    bytes.push(0x08); // end of the implicit root object
    bytes
}

#[test]
fn parse_shortcuts_vdf_reads_appid_as_unsigned_and_the_app_name() {
    // appid=-12345 as i32, reinterpreted as u32, is 4294954951 - same
    // cast parse_shortcuts_vdf itself performs, and the same
    // relationship confirmed against a real file's compatdata folder
    // name (see the doc comment on parse_shortcuts_vdf).
    let bytes = shortcuts_vdf_bytes(&[shortcut_entry_bytes("0", -12345, "Test Game", 0)]);

    let games = parse_shortcuts_vdf(&bytes);

    assert_eq!(games, vec![HostGame { name: "Test Game".into(), host_app_id: "4294954951".into(), is_steam: false }]);
}

#[test]
fn parse_shortcuts_vdf_skips_hidden_entries() {
    let bytes = shortcuts_vdf_bytes(&[
        shortcut_entry_bytes("0", -1, "Visible Game", 0),
        shortcut_entry_bytes("1", -2, "Hidden Game", 1),
    ]);

    let games = parse_shortcuts_vdf(&bytes);

    assert_eq!(games, vec![HostGame { name: "Visible Game".into(), host_app_id: "4294967295".into(), is_steam: false }]);
}

#[test]
fn parse_shortcuts_vdf_returns_empty_on_malformed_bytes() {
    assert_eq!(parse_shortcuts_vdf(b"not a valid shortcuts.vdf"), Vec::new());
}

#[test]
fn list_non_steam_games_returns_empty_when_userdata_is_missing() {
    let tmp = tempfile::tempdir().unwrap();
    // no userdata/ folder at all - simulates a host with no non-Steam
    // shortcuts (or Steam never having created a profile yet).
    assert_eq!(list_non_steam_games(tmp.path()), Vec::new());
}

#[test]
fn list_non_steam_games_reads_shortcuts_from_every_user_profile() {
    let tmp = tempfile::tempdir().unwrap();
    let profile_a = tmp.path().join("userdata").join("111").join("config");
    let profile_b = tmp.path().join("userdata").join("222").join("config");
    fs::create_dir_all(&profile_a).unwrap();
    fs::create_dir_all(&profile_b).unwrap();
    fs::write(
        profile_a.join("shortcuts.vdf"),
        shortcuts_vdf_bytes(&[shortcut_entry_bytes("0", -1, "Game From Profile A", 0)]),
    )
    .unwrap();
    fs::write(
        profile_b.join("shortcuts.vdf"),
        shortcuts_vdf_bytes(&[shortcut_entry_bytes("0", -2, "Game From Profile B", 0)]),
    )
    .unwrap();

    let mut games = list_non_steam_games(tmp.path());
    games.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(
        games,
        vec![
            HostGame { name: "Game From Profile A".into(), host_app_id: "4294967295".into(), is_steam: false },
            HostGame { name: "Game From Profile B".into(), host_app_id: "4294967294".into(), is_steam: false },
        ]
    );
}
