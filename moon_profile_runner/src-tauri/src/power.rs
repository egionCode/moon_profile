// Host power control: MAC detection (for Wake-on-LAN, triggered from the
// Deck side, see moonprofile_core.py:build_magic_packet) and shutdown.
// Both entry points here are pure/parameterized (detect_primary_mac takes
// the sysfs path, shutdown_command returns data instead of executing
// anything) so the logic is actually unit-testable without depending on
// this machine's real network interfaces or really running systemctl -
// see the accepted test gap in docs/prd.md Phase 6 (an end-to-end test
// here would genuinely power off whatever machine runs `cargo test`).

use std::fs;

const IGNORED_PREFIXES: &[&str] = &["lo", "docker", "veth", "virbr", "br-", "wg", "tun"];

fn is_ignored_interface(name: &str) -> bool {
    IGNORED_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}

// Picks the MAC of the primary physical NIC, for the Deck to save and
// later use to build a Wake-on-LAN magic packet. net_path is
// parameterized (the real caller passes "/sys/class/net") so this can be
// tested against a fixture directory instead of the real network
// hardware of the machine running the test - same pattern as
// detect_context(drm_path=...) in moonprofile_core.py. Prefers an
// interface that's actually "up" (a MAC from a disconnected NIC still
// works for Wake-on-LAN in practice, but an "up" one is the safer guess
// when there's more than one candidate); falls back to any interface
// with a real MAC if none is up.
pub fn detect_primary_mac(net_path: &str) -> Option<String> {
    let entries = fs::read_dir(net_path).ok()?;
    let mut candidates: Vec<(bool, String)> = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if is_ignored_interface(&name) {
            continue;
        }

        let iface_path = entry.path();
        let Ok(raw_mac) = fs::read_to_string(iface_path.join("address")) else {
            continue;
        };
        let mac = raw_mac.trim().to_string();
        if mac.is_empty() || mac == "00:00:00:00:00:00" {
            continue;
        }

        let is_up = fs::read_to_string(iface_path.join("operstate"))
            .map(|state| state.trim() == "up")
            .unwrap_or(false);
        candidates.push((is_up, mac));
    }

    candidates.sort_by_key(|(is_up, _)| !is_up);
    candidates.into_iter().next().map(|(_, mac)| mac)
}

// Pure - returns the command as data instead of running it, so it has an
// actual unit test (see tests/power.rs) without ever touching the real
// systemctl.
pub fn shutdown_command() -> (&'static str, &'static [&'static str]) {
    ("systemctl", &["poweroff"])
}

// Only called from the real POST /system/shutdown handler (server.rs),
// after the response has already been sent - see the ordering note
// there. Best-effort: nothing left to notify if this fails, the machine
// is either shutting down or it isn't.
pub fn run_shutdown() {
    let (cmd, args) = shutdown_command();
    match std::process::Command::new(cmd).args(args).output() {
        Ok(output) if !output.status.success() => {
            println!(
                "[{}] [power] shutdown command exited with {}: {}",
                crate::server::timestamp(),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Err(error) => {
            println!("[{}] [power] failed to run the shutdown command: {error}", crate::server::timestamp());
        }
        Ok(_) => {}
    }
}

#[cfg(test)]
#[path = "tests/power.rs"]
mod tests;
