// Best-effort autostart enablement for the systemd --user unit
// (packaging/moon-profile-runner.service). Pacman/yay run the package
// install as root, which can't safely touch a specific user's systemd
// --user instance (wrong session, wrong D-Bus, wrong XDG_RUNTIME_DIR) -
// see moon-profile-runner-git.install, which only prints a reminder.
// Running the enable from INSIDE the app instead works fine, because by
// the time this runs we're already inside the right user session - so
// the first time someone launches the Runner by hand (e.g. from the
// applications menu, right after installing), it quietly registers
// itself for autostart on every future login. Idempotent and harmless
// if it's already enabled, or if systemd/systemctl aren't available at
// all (e.g. `cargo run` during development, or a non-systemd host).

const UNIT_NAME: &str = "moon-profile-runner.service";

pub fn ensure_enabled() {
    ensure_enabled_with("systemctl", UNIT_NAME);
}

fn ensure_enabled_with(systemctl_bin: &str, unit: &str) {
    let already_enabled = std::process::Command::new(systemctl_bin)
        .args(["--user", "is-enabled", unit])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    if already_enabled {
        return;
    }

    let _ = std::process::Command::new(systemctl_bin)
        .args(["--user", "enable", unit])
        .output();
}

#[cfg(test)]
#[path = "tests/autostart.rs"]
mod tests;
