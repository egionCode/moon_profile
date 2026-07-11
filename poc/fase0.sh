#!/usr/bin/env bash
# Fase 0 do MoonProfile: PoC de linha de comando.
# Reproduz o fluxo completo (Apollo + Moonlight CLI) sem escrever o plugin,
# pra validar que HDR, resolucao e troca de AppID funcionam ponta a ponta.
#
# Uso (rodar no Deck: e' onde o curl e o cliente Moonlight vivem):
#   1. Preenche as variaveis na secao CONFIG abaixo.
#   2. ./fase0.sh apply    -> atualiza o app "SteamGame" no Apollo e lanca via Moonlight
#   3. ./fase0.sh undo     -> IMPRIME o prep-cmd undo (nao executa!). Cola via SSH no HOST
#                             pra testar sem esperar o Apollo detectar fim de sessao.
#   4. ./fase0.sh apps     -> lista os apps atuais no Apollo (debug)
#
# Onde cada coisa roda:
#   - curl (POST /api/apps) e o moonlight CLI: no DECK.
#   - kscreen-doctor / steam / pkill do prep-cmd: no HOST, executados PELO APOLLO
#     (nunca localmente por este script - ver cmd_undo).
#
# Requisitos no Deck: curl, jq, moonlight (flatpak com.moonlight_stream.Moonlight).
# Requisitos no host: Apollo rodando, kscreen-doctor, KDE Plasma 6 Wayland.

set -euo pipefail

# ============ CONFIG (preenche antes de rodar) ============

APOLLO_HOST="192.168.1.6"          # IP do host com Apollo
APOLLO_PORT="47990"
APOLLO_USER='admin'                # credencial admin do Apollo
APOLLO_PASS='troque-me'            # aspas SIMPLES aqui de proposito: com aspas
                                    # duplas, "$", "`" e "\" na senha seriam
                                    # interpretados pelo bash antes de virar o
                                    # valor da variavel, corrompendo a senha
                                    # silenciosamente

APPID="2050650"                    # AppID do Steam a testar (ex: RE4 Remake)
APP_NAME="SteamGame"               # nome fixo do app no Apollo, reaproveitado a cada troca de jogo

TARGET_OUTPUT="HDMI-A-1"           # saida a ativar (TV dockada)
RESTORE_OUTPUT="DP-3"              # saida a restaurar no undo (ex: eDP interno via dock)
RESOLUTION="3840x2160"
FPS="60"
HDR="true"                         # "true" ou "false"

# ============================================================

BASE_URL="https://${APOLLO_HOST}:${APOLLO_PORT}"

# Este fork do Apollo (ClassicOldSong/Apollo) NAO usa HTTP Basic Auth na API,
# apesar do que diz docs/api.md. authenticate() em src/confighttp.cpp so
# checa um cookie de sessao "auth", obtido via POST /api/login. Por isso
# usamos cookie jar em vez de "-u user:pass".
COOKIE_JAR="$(mktemp)"
trap 'rm -f "${COOKIE_JAR}"' EXIT
CURL_AUTH=(-k -s -b "${COOKIE_JAR}" -c "${COOKIE_JAR}")

log() { printf '[fase0] %s\n' "$1" >&2; }

# Faz o POST /api/login e guarda o cookie "auth" no cookie jar pras
# proximas chamadas. Tem que rodar antes de qualquer outra chamada na API.
login() {
    local payload raw status
    payload=$(jq -n --arg u "${APOLLO_USER}" --arg p "${APOLLO_PASS}" \
        '{username: $u, password: $p}')
    raw=$(curl "${CURL_AUTH[@]}" -w '\n%{http_code}' -X POST "${BASE_URL}/api/login" \
        -H "Content-Type: application/json" -d "${payload}")
    status="${raw##*$'\n'}"
    if [[ ! "${status}" =~ ^2 ]]; then
        log "ERRO HTTP ${status} em POST /api/login - usuario/senha errados em Apollo?"
        exit 1
    fi
    log "login ok, cookie de sessao obtido"
}

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

# Faz a chamada e VERIFICA o status HTTP. curl so falha (exit != 0) em erro de
# rede/TLS, nao em 401/403/500 - por isso um 401 passava batido e o script
# seguia como se tivesse dado certo. Aqui aborta com o corpo da resposta.
api_request() {
    local method="$1" path="$2" data="${3:-}"
    local raw status body
    if [[ -n "${data}" ]]; then
        raw=$(curl "${CURL_AUTH[@]}" -w '\n%{http_code}' -X "${method}" "${BASE_URL}${path}" \
            -H "Content-Type: application/json" -d "${data}")
    else
        raw=$(curl "${CURL_AUTH[@]}" -w '\n%{http_code}' -X "${method}" "${BASE_URL}${path}")
    fi
    status="${raw##*$'\n'}"
    body="${raw%$'\n'*}"
    if [[ ! "${status}" =~ ^2 ]]; then
        log "ERRO HTTP ${status} em ${method} ${path}"
        log "resposta do Apollo: ${body}"
        exit 1
    fi
    printf '%s' "${body}"
}

# Busca o index do app existente com nome $APP_NAME, ou -1 se ainda nao existe.
# Isso e o que permite trocar de AppID sem reiniciar o Apollo: sempre atualiza
# o mesmo app em vez de criar um novo a cada chamada.
find_app_index() {
    api_request GET /api/apps \
        | jq -r --arg name "${APP_NAME}" \
            '.apps | to_entries[] | select(.value.name == $name) | .key' \
        | head -n1
}

cmd_apps() {
    login
    api_request GET /api/apps | jq .
}

cmd_apply() {
    login
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

    api_request POST /api/apps "${payload}"
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
    # NAO executa localmente: o prep-cmd undo roda no HOST (e' o Apollo, no
    # host, quem chama isso), nunca no Deck. kscreen-doctor referencia as
    # saidas de video da GPU do host, e o pkill -f "AppId=..." tem que mirar
    # o processo do Steam do host, nao o do Deck. Executar aqui por engano
    # mataria o processo errado ou simplesmente falharia silenciosamente.
    log "ATENCAO: cole o comando abaixo via SSH no HOST, nao rode no Deck"
    build_undo_body
    echo
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
