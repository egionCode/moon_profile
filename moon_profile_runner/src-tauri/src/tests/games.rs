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
