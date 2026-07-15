use super::*;

// Fixture reduzida, capturada de verdade rodando "kscreen-doctor -j"
// no device (so' os campos que import, "modes" comprimido) - evita
// depender do kscreen-doctor estar instalado pra testar o parsing.
const FIXTURE: &str = r#"{
    "features": 255,
    "outputs": [
        {"id": 1, "name": "HDMI-A-1", "connected": true, "enabled": false, "modes": []},
        {"id": 2, "name": "DP-3", "connected": true, "enabled": true, "modes": []},
        {"id": 3, "name": "DP-4", "connected": false, "enabled": false, "modes": []}
    ]
}"#;

#[test]
fn parses_name_connected_and_enabled_for_each_output() {
    let displays = parse_kscreen_json(FIXTURE);

    assert_eq!(
        displays,
        vec![
            HostDisplay { name: "HDMI-A-1".to_string(), connected: true, enabled: false },
            HostDisplay { name: "DP-3".to_string(), connected: true, enabled: true },
            HostDisplay { name: "DP-4".to_string(), connected: false, enabled: false },
        ]
    );
}

#[test]
fn returns_empty_on_malformed_json() {
    assert_eq!(parse_kscreen_json("nao e' json"), Vec::new());
}

#[test]
fn returns_empty_when_outputs_field_is_missing() {
    assert_eq!(parse_kscreen_json(r#"{"features": 255}"#), Vec::new());
}
