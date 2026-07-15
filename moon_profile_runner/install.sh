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

log() { printf '[install] %s\n' "$1"; }

log "building release..."
cargo build --release --manifest-path "${SCRIPT_DIR}/src-tauri/Cargo.toml"

log "copying binary to ${INSTALL_BIN}..."
mkdir -p "${HOME}/.local/bin"
cp "${SCRIPT_DIR}/src-tauri/target/release/${BIN_NAME}" "${INSTALL_BIN}"

log "adding it to the applications menu in ${APPLICATIONS_DIR}..."
mkdir -p "${APPLICATIONS_DIR}"
sed "s|__EXEC_PATH__|${INSTALL_BIN}|" "${SCRIPT_DIR}/packaging/moon-profile-runner.desktop" \
    > "${APPLICATIONS_DIR}/moon-profile-runner.desktop"

log "registering the systemd --user unit in ${SYSTEMD_USER_DIR}..."
mkdir -p "${SYSTEMD_USER_DIR}"
sed "s|/usr/bin/moon-profile-runner|${INSTALL_BIN}|" "${SCRIPT_DIR}/packaging/moon-profile-runner.service" \
    > "${SYSTEMD_USER_DIR}/moon-profile-runner.service"

log "enabling and starting it now..."
systemctl --user daemon-reload
systemctl --user enable --now moon-profile-runner.service

log "done - running now, starts on its own at next login, and is in the applications menu."
