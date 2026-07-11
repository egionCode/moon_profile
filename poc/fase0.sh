#!/usr/bin/env bash
# Fase 0 do MoonProfile: PoC de linha de comando.
# Reproduz o fluxo completo (Apollo + Moonlight CLI) sem escrever o plugin,
# pra validar que HDR, resolucao e troca de AppID funcionam ponta a ponta.
#
# Uso:
#   1. Preenche as variaveis na secao CONFIG abaixo.
#   2. ./fase0.sh apply    -> atualiza o app "SteamGame" no Apollo e lanca via Moonlight
#   3. ./fase0.sh undo     -> roda manualmente o prep-cmd undo (pra testar sem esperar o timeout)
#   4. ./fase0.sh apps     -> lista os apps atuais no Apollo (debug)
#
# Requisitos: curl, jq, moonlight (flatpak com.moonlight_stream.Moonlight), kscreen-doctor no host.

set -euo pipefail

# ============ CONFIG (preenche antes de rodar) ============

APOLLO_HOST="192.168.1.6"          # IP do host com Apollo
APOLLO_PORT="47990"
APOLLO_USER="admin"                # credencial admin do Apollo
APOLLO_PASS="troque-me"

APPID="2050650"                    # AppID do Steam a testar (ex: RE4 Remake)
APP_NAME="SteamGame"               # nome fixo do app no Apollo, reaproveitado a cada troca de jogo

TARGET_OUTPUT="HDMI-A-1"           # saida a ativar (TV dockada)
RESTORE_OUTPUT="DP-3"              # saida a restaurar no undo (ex: eDP interno via dock)
RESOLUTION="3840x2160"
FPS="60"
HDR="true"                         # "true" ou "false"

# ============================================================

BASE_URL="https://${APOLLO_HOST}:${APOLLO_PORT}"
CURL_AUTH=(-u "${APOLLO_USER}:${APOLLO_PASS}" -k -s)

log() { printf '[fase0] %s\n' "$1" >&2; }

# Corpo (sem o wrapper "bash -c '...'") do prep-cmd "do": ativa output alvo,
# aplica modo, HDR, desliga o resto. Usa aspas DUPLAS pros literais porque o
# corpo inteiro e embrulhado em aspas simples mais abaixo (build_do_cmd) -
# aspas simples aninhadas aqui quebrariam esse wrapper.
build_do_body() {
    local cmd="kscreen-doctor output.${TARGET_OUTPUT}.enable"
    cmd+=" ; kscreen-doctor output.${TARGET_OUTPUT}.mode.${RESOLUTION}@${FPS}"
    if [[ "${HDR}" == "true" ]]; then
        cmd+=" ; kscreen-doctor output.${TARGET_OUTPUT}.hdr.enable"
        cmd+=" ; kscreen-doctor output.${TARGET_OUTPUT}.wcg.enable"
    fi
    cmd+=" ; kscreen-doctor output.${RESTORE_OUTPUT}.disable"
    printf '%s' "${cmd}"
}

# Corpo do prep-cmd "undo": mata o jogo pelo AppId (soft depois hard kill),
# fecha o Big Picture e restaura os outputs originais. AppId= usa aspas
# DUPLAS pelo mesmo motivo do build_do_body.
# ";" no lugar de "&&" e intencional: se o pkill falhar (jogo ja fechou),
# a cadeia continua e os outputs sao restaurados de qualquer forma.
build_undo_body() {
    local cmd="pkill -TERM -f \"AppId=${APPID}\""
    cmd+=" ; sleep 5"
    cmd+=" ; pkill -KILL -f \"AppId=${APPID}\" 2>/dev/null"
    cmd+=" ; setsid steam steam://close/bigpicture"
    cmd+=" ; sleep 2"
    cmd+=" ; kscreen-doctor output.${RESTORE_OUTPUT}.enable"
    cmd+=" ; sleep 1"
    cmd+=" ; kscreen-doctor output.${TARGET_OUTPUT}.disable"
    printf '%s' "${cmd}"
}

build_do_cmd() { printf "bash -c '%s'" "$(build_do_body)"; }
build_undo_cmd() { printf "bash -c '%s'" "$(build_undo_body)"; }

# Busca o index do app existente com nome $APP_NAME, ou -1 se ainda nao existe.
# Isso e o que permite trocar de AppID sem reiniciar o Apollo: sempre atualiza
# o mesmo app em vez de criar um novo a cada chamada.
find_app_index() {
    curl "${CURL_AUTH[@]}" "${BASE_URL}/api/apps" \
        | jq -r --arg name "${APP_NAME}" \
            '.apps | to_entries[] | select(.value.name == $name) | .key' \
        | head -n1
}

cmd_apps() {
    curl "${CURL_AUTH[@]}" "${BASE_URL}/api/apps" | jq .
}

cmd_apply() {
    local index
    index=$(find_app_index || true)
    index="${index:--1}"
    log "atualizando app '${APP_NAME}' (index=${index}) com AppID=${APPID}"

    local do_cmd undo_cmd payload
    do_cmd=$(build_do_cmd)
    undo_cmd=$(build_undo_cmd)

    payload=$(jq -n \
        --arg name "${APP_NAME}" \
        --arg cmd "steam steam://rungameid/${APPID}" \
        --argjson index "${index}" \
        --arg do "${do_cmd}" \
        --arg undo "${undo_cmd}" \
        --arg output "/tmp/apollo-steamgame-${APPID}.log" \
        '{
            name: $name,
            cmd: $cmd,
            index: $index,
            "auto-detach": true,
            "wait-all": false,
            "exit-timeout": 5,
            "exclude-global-prep-cmd": false,
            elevated: false,
            "prep-cmd": [{ do: $do, undo: $undo }],
            output: $output
        }')

    curl "${CURL_AUTH[@]}" -X POST "${BASE_URL}/api/apps" \
        -H "Content-Type: application/json" \
        -d "${payload}"
    log "app atualizado. Lancando via Moonlight CLI..."

    local hdr_flag=(--no-hdr)
    [[ "${HDR}" == "true" ]] && hdr_flag=(--hdr)

    flatpak run com.moonlight_stream.Moonlight stream "${APOLLO_HOST}" "${APP_NAME}" \
        --resolution "${RESOLUTION}" \
        --fps "${FPS}" \
        --video-codec HEVC \
        "${hdr_flag[@]}"
}

cmd_undo() {
    log "rodando o prep-cmd undo manualmente (sem esperar o Apollo)"
    bash -c "$(build_undo_body)"
}

case "${1:-}" in
    apply) cmd_apply ;;
    undo) cmd_undo ;;
    apps) cmd_apps ;;
    *)
        echo "Uso: $0 {apply|undo|apps}" >&2
        exit 1
        ;;
esac
