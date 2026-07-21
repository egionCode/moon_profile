// Best-effort round-trip latency to a known client (a Deck that has
// connected to this Runner before, see clients.rs) - shells out to the
// system `ping` binary (same "shell out to a system tool" pattern as
// kscreen-doctor/systemctl elsewhere in this project) instead of opening
// a raw ICMP socket ourselves, which would need CAP_NET_RAW the Runner
// doesn't have; the system `ping` binary is already
// setuid/capabilities-enabled on essentially every Linux distro.
//
// Only ever invoked from the Runner's own window (main.js), on a timer
// that runs while that window is actually open - not from the always-on
// HTTP server side, so an unreachable Deck can never slow down anything
// the Deck itself is waiting on.

use std::time::Duration;

// Pure - parses `ping -c 1`'s stdout for the round-trip time ("...
// time=12.3 ms ..."), kept separate from actually running `ping` so it
// has a real unit test (tests/ping.rs) against captured output instead
// of depending on the local network.
fn parse_ping_output(raw: &str) -> Option<f64> {
    let marker = "time=";
    let start = raw.find(marker)? + marker.len();
    let rest = &raw[start..];
    let end = rest.find(' ')?;
    rest[..end].parse::<f64>().ok()
}

// ping_bin is parameterized (real callers always pass "ping") so the
// "binary doesn't exist/can't be spawned" failure path can be tested
// with a bogus path instead of mutating the process-wide PATH env var,
// which would race with every other test in this binary that shells out
// to a command (cargo test runs them in parallel by default).
fn ping_once_with(ping_bin: &str, ip: &str) -> Option<f64> {
    let output = std::process::Command::new(ping_bin).args(["-c", "1", "-W", "1", ip]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_ping_output(&String::from_utf8_lossy(&output.stdout))
}

// One ping, ~1s timeout. None on any failure (host down, ping missing,
// unparseable output) - fail-open, same as list_displays()/
// detect_primary_mac() elsewhere: the caller (the window's polling loop)
// just shows "could not connect" either way.
pub fn ping_once(ip: &str) -> Option<f64> {
    ping_once_with("ping", ip)
}

// Timeout for the whole ping_client Tauri command (lib.rs), on top of
// `ping`'s own -W 1: guards against ping hanging indefinitely for some
// other reason (DNS resolution on a hostname instead of a plain IP, a
// stuck subprocess, etc) - the window's 3s polling loop must never stall
// waiting on a single client.
pub const PING_COMMAND_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(test)]
#[path = "tests/ping.rs"]
mod tests;
