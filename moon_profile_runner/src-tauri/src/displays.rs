// Lists the host's monitors/screen outputs (via kscreen-doctor -j) to
// give the Deck real options instead of the user having to type the
// output name ("HDMI-A-1", "DP-3", etc) or its resolution/fps by hand -
// the UI uses this to populate the "Monitor" and "Resolution" selects in
// ProfileEditor.tsx (which drive host AND Moonlight resolution/fps
// together now, see docs/prd.md) and the dynamic list for
// disable_outputs.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct HostDisplayMode {
    pub resolution: String, // ex: "3840x2160"
    pub fps: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct HostDisplay {
    pub name: String,
    pub connected: bool,
    pub enabled: bool,
    pub modes: Vec<HostDisplayMode>,
}

#[derive(Deserialize)]
struct KscreenSize {
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
struct KscreenMode {
    #[serde(rename = "refreshRate")]
    refresh_rate: f64,
    size: KscreenSize,
}

// Only the fields we care about from kscreen-doctor's full JSON - serde
// ignores the rest (id, currentModeId, icc profile, etc) automatically.
#[derive(Deserialize)]
struct KscreenOutput {
    name: String,
    connected: bool,
    enabled: bool,
    #[serde(default)]
    modes: Vec<KscreenMode>,
}

#[derive(Deserialize)]
struct KscreenConfig {
    outputs: Vec<KscreenOutput>,
}

// Real EDID data repeats the same (width, height, rounded fps) several
// times with slightly different float refreshRate values (confirmed
// against a real monitor: "3840x2160@60" showed up 4 times, refreshRate
// 59.98/60.0/60.0/59.94) - dedup by the rounded fps, since that's the
// granularity a profile actually stores (MoonlightConfig.fps/
// HostConfig.fps are both plain integers). Sorted biggest resolution
// first, then highest fps, so the UI's dropdown shows the best option
// first instead of whatever order kscreen-doctor happened to list.
fn modes_from_kscreen(raw_modes: &[KscreenMode]) -> Vec<HostDisplayMode> {
    let mut seen = std::collections::HashSet::new();
    let mut entries: Vec<(u32, u32, u32)> = Vec::new(); // (width, height, fps)
    for m in raw_modes {
        let key = (m.size.width, m.size.height, m.refresh_rate.round() as u32);
        if seen.insert(key) {
            entries.push(key);
        }
    }

    entries.sort_by(|a, b| {
        let area_a = u64::from(a.0) * u64::from(a.1);
        let area_b = u64::from(b.0) * u64::from(b.1);
        area_b.cmp(&area_a).then(b.2.cmp(&a.2))
    });

    entries.into_iter().map(|(width, height, fps)| HostDisplayMode { resolution: format!("{width}x{height}"), fps }).collect()
}

// Pure - parses the JSON that kscreen-doctor -j prints. Kept separate
// from the actual process call so it can be tested against real
// fixtures (captured from the device) without depending on
// kscreen-doctor being installed on the machine running the test.
fn parse_kscreen_json(raw: &str) -> Vec<HostDisplay> {
    let Ok(config) = serde_json::from_str::<KscreenConfig>(raw) else {
        return Vec::new();
    };
    config
        .outputs
        .into_iter()
        .map(|o| HostDisplay { name: o.name, connected: o.connected, enabled: o.enabled, modes: modes_from_kscreen(&o.modes) })
        .collect()
}

// Fail-open (empty list) if kscreen-doctor doesn't exist, fails, or
// returns something unexpected - same philosophy as the rest of the
// project (e.g. filter_to_games_only in games.rs): better for the UI to
// show an empty list (the user can still type manually as a fallback)
// than to hang.
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
