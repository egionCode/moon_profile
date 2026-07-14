#!/usr/bin/env bash
# Deploy do MoonProfile pro Steam Deck: rsync + restart do plugin_loader.
#
# Requisitos ja configurados (ver historico da Fase 1):
#   - chave SSH sem senha pra deck@DECK_HOST
#   - regras sudoers NOPASSWD em /etc/sudoers.d/zzz-decky-plugin-loader:
#       - "systemctl stop/start/restart plugin_loader.service" (servico de
#         SISTEMA nesse Deck, nao --user)
#       - "chown -R deck.deck" so nesta pasta (usa ".", nao ":" - o
#         caractere ":" quebra a gramatica do sudoers)
#     O plugin_loader.service (roda como root) reafirma posse de root na
#     pasta raiz do plugin em poucos segundos, MESMO sem restart (medido:
#     ainda "deck" 1s depois do chown, ja' "root" de novo 4s depois) -
#     por isso a pasta so fica estavel com o servico PARADO durante o
#     rsync. plugin.json fica de fora do sync tambem (ver --exclude
#     abaixo) porque o reclaim nele e' ainda mais agressivo; so editar/
#     subir esse arquivo manualmente se precisar mudar plugin.json.
#
# Uso: ./deploy.sh [build]
#   ./deploy.sh          -> so sincroniza o que ja foi buildado e reinicia
#   ./deploy.sh build    -> roda "pnpm build" antes de sincronizar

set -euo pipefail

DECK_HOST="${DECK_HOST:-192.168.1.67}"
DECK_USER="${DECK_USER:-deck}"
PLUGIN_DIR="/home/deck/homebrew/plugins/moonprofile"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

log() { printf '[deploy] %s\n' "$1"; }

if [[ "${1:-}" == "build" ]]; then
    log "buildando..."
    (cd "${SCRIPT_DIR}" && npx --yes pnpm@latest build)
fi

log "parando plugin_loader (senao ele reafirma posse de root na pasta durante o sync)..."
ssh "${DECK_USER}@${DECK_HOST}" "sudo systemctl stop plugin_loader.service && sudo chown -R deck.deck '${PLUGIN_DIR}'"

log "sincronizando pro Deck (${DECK_USER}@${DECK_HOST}:${PLUGIN_DIR})..."
# --no-owner --no-group: maquinas diferentes, sem usuarios/grupos em comum -
# "-a" tentando preservar dono/grupo local trava em chgrp (Operation not
# permitted) porque o usuario "deck" nao pertence ao grupo do usuario local.
rsync -az --no-owner --no-group --delete \
    --exclude node_modules \
    --exclude .git \
    --exclude plugin.json \
    --exclude docs \
    "${SCRIPT_DIR}/" "${DECK_USER}@${DECK_HOST}:${PLUGIN_DIR}/"

log "reiniciando plugin_loader..."
ssh "${DECK_USER}@${DECK_HOST}" "sudo systemctl start plugin_loader.service"

log "pronto."
