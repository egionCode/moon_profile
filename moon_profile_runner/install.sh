#!/usr/bin/env bash
# Installs MoonProfile Runner as a KDE autostart entry: builds the
# release binary, copies it to ~/.local/bin/, and registers the
# autostart entry in ~/.config/autostart/. Session autostart (not a
# systemd service) because the app has a tray/GUI and needs an active
# graphical session to show up.
#
# Usage: ./install.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_NAME="moon_profile_runner"
INSTALL_BIN="${HOME}/.local/bin/${BIN_NAME}"
AUTOSTART_DIR="${HOME}/.config/autostart"

log() { printf '[install] %s\n' "$1"; }

log "building release..."
cargo build --release --manifest-path "${SCRIPT_DIR}/src-tauri/Cargo.toml"

log "copying binary to ${INSTALL_BIN}..."
mkdir -p "${HOME}/.local/bin"
cp "${SCRIPT_DIR}/src-tauri/target/release/${BIN_NAME}" "${INSTALL_BIN}"

log "registering autostart in ${AUTOSTART_DIR}..."
mkdir -p "${AUTOSTART_DIR}"
sed "s|__EXEC_PATH__|${INSTALL_BIN}|" "${SCRIPT_DIR}/packaging/moon-profile-runner.desktop" \
    > "${AUTOSTART_DIR}/moon-profile-runner.desktop"

log "done - starts on its own at next login, or run it now with: ${INSTALL_BIN}"
