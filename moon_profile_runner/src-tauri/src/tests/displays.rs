use super::*;

// Reduced fixture, actually captured by running "kscreen-doctor -j"
// on the device (only the fields that matter, "modes" trimmed down) -
// avoids depending on kscreen-doctor being installed to test parsing.
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
            HostDisplay { name: "HDMI-A-1".to_string(), connected: true, enabled: false, modes: Vec::new() },
            HostDisplay { name: "DP-3".to_string(), connected: true, enabled: true, modes: Vec::new() },
            HostDisplay { name: "DP-4".to_string(), connected: false, enabled: false, modes: Vec::new() },
        ]
    );
}

#[test]
fn returns_empty_on_malformed_json() {
    assert_eq!(parse_kscreen_json("this is not json"), Vec::new());
}

#[test]
fn returns_empty_when_outputs_field_is_missing() {
    assert_eq!(parse_kscreen_json(r#"{"features": 255}"#), Vec::new());
}

// Real fixture (trimmed), actually captured from a real monitor: EDID
// repeats "3840x2160@60" four times with slightly different
// refreshRate floats, plus a lower/smaller mode.
const FIXTURE_WITH_MODES: &str = r#"{
    "features": 255,
    "outputs": [
        {
            "id": 1,
            "name": "HDMI-A-1",
            "connected": true,
            "enabled": true,
            "modes": [
                {"id": "1", "name": "3840x2160@60", "refreshRate": 59.981998443603516, "size": {"width": 3840, "height": 2160}},
                {"id": "10", "name": "3840x2160@60", "refreshRate": 60.0, "size": {"width": 3840, "height": 2160}},
                {"id": "11", "name": "3840x2160@60", "refreshRate": 60.0, "size": {"width": 3840, "height": 2160}},
                {"id": "12", "name": "3840x2160@60", "refreshRate": 59.939998626708984, "size": {"width": 3840, "height": 2160}},
                {"id": "20", "name": "1920x1080@120", "refreshRate": 120.0, "size": {"width": 1920, "height": 1080}}
            ]
        }
    ]
}"#;

#[test]
fn dedups_modes_that_only_differ_by_a_fractional_refresh_rate() {
    let displays = parse_kscreen_json(FIXTURE_WITH_MODES);

    let modes = &displays[0].modes;
    assert_eq!(modes.iter().filter(|m| m.resolution == "3840x2160" && m.fps == 60).count(), 1);
}

#[test]
fn sorts_modes_by_largest_resolution_first_then_highest_fps() {
    let displays = parse_kscreen_json(FIXTURE_WITH_MODES);

    assert_eq!(
        displays[0].modes,
        vec![
            HostDisplayMode { resolution: "3840x2160".to_string(), fps: 60 },
            HostDisplayMode { resolution: "1920x1080".to_string(), fps: 120 },
        ]
    );
}
