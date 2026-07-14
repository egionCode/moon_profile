#!/usr/bin/env bash
# Instala o MoonProfile Runner como autostart do KDE: builda o binario de
# release, copia pra ~/.local/bin/ e registra o autostart em
# ~/.config/autostart/. Autostart de sessao (nao um servico systemd) porque
# o app tem tray/GUI e precisa de sessao grafica ativa pra aparecer.
#
# Uso: ./install.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_NAME="moon_profile_runner"
INSTALL_BIN="${HOME}/.local/bin/${BIN_NAME}"
AUTOSTART_DIR="${HOME}/.config/autostart"

log() { printf '[install] %s\n' "$1"; }

log "buildando release..."
cargo build --release --manifest-path "${SCRIPT_DIR}/src-tauri/Cargo.toml"

log "copiando binario pra ${INSTALL_BIN}..."
mkdir -p "${HOME}/.local/bin"
cp "${SCRIPT_DIR}/src-tauri/target/release/${BIN_NAME}" "${INSTALL_BIN}"

log "registrando autostart em ${AUTOSTART_DIR}..."
mkdir -p "${AUTOSTART_DIR}"
sed "s|__EXEC_PATH__|${INSTALL_BIN}|" "${SCRIPT_DIR}/packaging/moon-profile-runner.desktop" \
    > "${AUTOSTART_DIR}/moon-profile-runner.desktop"

log "pronto - inicia sozinho no proximo login, ou roda agora com: ${INSTALL_BIN}"
