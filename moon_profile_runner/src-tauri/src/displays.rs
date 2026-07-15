// Lists the host's monitors/screen outputs (via kscreen-doctor -j) to
// give the Deck real options instead of the user having to type the
// output name ("HDMI-A-1", "DP-3", etc) by hand - the UI uses this to
// populate a <select> for target_output and a dynamic list for
// disable_outputs (see src/api.ts/ProfileEditor on the Decky side).

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct HostDisplay {
    pub name: String,
    pub connected: bool,
    pub enabled: bool,
}

// Only the fields we care about from kscreen-doctor's full JSON - serde
// ignores the rest (modes, icc profile, etc) automatically.
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
        .map(|o| HostDisplay { name: o.name, connected: o.connected, enabled: o.enabled })
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
