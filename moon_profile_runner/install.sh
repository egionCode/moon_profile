#!/usr/bin/env bash
# Installs MoonProfile Runner for the current user: builds the release
# binary, copies it to ~/.local/bin/, adds it to the applications menu,
# and registers + enables a systemd --user unit for autostart
# (WantedBy=graphical-session.target - the app has a tray/GUI and needs
# an active graphical session to show up, verified this works on a real
# KDE Plasma 6 Wayland session).
#
# Usage: ./install.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_NAME="moon_profile_runner"
INSTALL_BIN="${HOME}/.local/bin/${BIN_NAME}"
APPLICATIONS_DIR="${HOME}/.local/share/applications"
SYSTEMD_USER_DIR="${HOME}/.config/systemd/user"
OLD_AUTOSTART_FILE="${HOME}/.config/autostart/moon-profile-runner.desktop"

log() { printf '[install] %s\n' "$1"; }

log "building release..."
cargo build --release --manifest-path "${SCRIPT_DIR}/src-tauri/Cargo.toml"

log "copying binary to ${INSTALL_BIN}..."
mkdir -p "${HOME}/.local/bin"
# Copy to a temp file in the same directory, then rename over the
# destination - a plain `cp` onto the existing path fails with ETXTBSY
# ("Text file busy") if a previous install is still running that exact
# binary (e.g. re-running this script to upgrade while the systemd unit
# is active), since it truncates+writes the running inode in place.
# rename() only swaps the directory entry, so the already-running
# process keeps its old (now-unlinked) inode undisturbed while new
# invocations get the fresh binary.
INSTALL_BIN_TMP="$(mktemp "${HOME}/.local/bin/.${BIN_NAME}.XXXXXX")"
cp "${SCRIPT_DIR}/src-tauri/target/release/${BIN_NAME}" "${INSTALL_BIN_TMP}"
chmod +x "${INSTALL_BIN_TMP}"
mv -f "${INSTALL_BIN_TMP}" "${INSTALL_BIN}"

log "adding it to the applications menu in ${APPLICATIONS_DIR}..."
mkdir -p "${APPLICATIONS_DIR}"
sed "s|__EXEC_PATH__|${INSTALL_BIN}|" "${SCRIPT_DIR}/packaging/moon-profile-runner.desktop" \
    > "${APPLICATIONS_DIR}/moon-profile-runner.desktop"

log "registering the systemd --user unit in ${SYSTEMD_USER_DIR}..."
mkdir -p "${SYSTEMD_USER_DIR}"
sed "s|/usr/bin/moon-profile-runner|${INSTALL_BIN}|" "${SCRIPT_DIR}/packaging/moon-profile-runner.service" \
    > "${SYSTEMD_USER_DIR}/moon-profile-runner.service"

# Pre-migration installs of this same script registered autostart via a
# plain XDG .desktop entry instead of systemd - leaving it behind would
# make both fire on the next login (two Runner processes racing for the
# same HTTP port, the second one panicking on bind). Safe to always
# attempt: a no-op if it was never there.
if [ -f "${OLD_AUTOSTART_FILE}" ]; then
    log "removing stale pre-systemd autostart entry (${OLD_AUTOSTART_FILE})..."
    rm -f "${OLD_AUTOSTART_FILE}"
fi

log "enabling and (re)starting it now..."
systemctl --user daemon-reload
systemctl --user enable moon-profile-runner.service
systemctl --user restart moon-profile-runner.service

log "done - running now (with the freshly built binary), starts on its own at next login, and is in the applications menu."
