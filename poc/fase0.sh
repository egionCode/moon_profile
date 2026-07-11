#!/usr/bin/env bash
# Fase 0 do MoonProfile: PoC de linha de comando.
# Reproduz o fluxo completo (Apollo + Moonlight CLI) sem escrever o plugin,
# pra validar que HDR, resolucao e troca de AppID funcionam ponta a ponta.
#
# Uso (rodar no Deck: e' onde o curl e o cliente Moonlight vivem):
#   1. Preenche as variaveis na secao CONFIG abaixo.
#   2. ./fase0.sh apply    -> atualiza (por uuid) ou cria o app "SteamGame" no
#                             Apollo e lanca via Moonlight
#   3. ./fase0.sh undo     -> IMPRIME o prep-cmd undo (nao executa!). Cola via SSH no HOST
#                             pra testar sem esperar o Apollo detectar fim de sessao.
#   4. ./fase0.sh apps     -> lista os apps atuais no Apollo (debug)
#   5. ./fase0.sh delete   -> remove o app "SteamGame" (debug/limpeza)
#
# Onde cada coisa roda:
#   - curl (POST /api/apps) e o moonlight CLI: no DECK.
#   - kscreen-doctor / steam / pkill do prep-cmd: no HOST, executados PELO APOLLO
#     (nunca localmente por este script - ver cmd_undo).
#
# Requisitos no Deck: curl, jq, moonlight (flatpak com.moonlight_stream.Moonlight).
# Requisitos no host: Apollo rodando, kscreen-doctor, KDE Plasma 6 Wayland.

set -euo pipefail

# Log em arquivo ao lado do script, com tudo que o script imprime (stdout e
# stderr) durante a execucao, inclusive as respostas cruas do Apollo. "tee -a"
# mantem a saida no terminal alem de gravar no arquivo.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_FILE="${SCRIPT_DIR}/fase0.log"
exec > >(tee -a "${LOG_FILE}") 2>&1

# ============ CONFIG (preenche antes de rodar) ============

APOLLO_HOST="192.168.1.6"          # IP do host com Apollo
APOLLO_PORT="47990"
APOLLO_USER='apollo'                # credencial admin do Apollo
APOLLO_PASS='2003'                 # aspas SIMPLES aqui de proposito: com aspas
                                    # duplas, "$", "`" e "\" na senha seriam
                                    # interpretados pelo bash antes de virar o
                                    # valor da variavel, corrompendo a senha
                                    # silenciosamente

APPID="2050650"                    # AppID do Steam a testar (ex: RE4 Remake)
APP_NAME="SteamGame"               # nome fixo do app no Apollo, reaproveitado a cada troca de jogo

TARGET_OUTPUT="HDMI-A-1"           # saida a ativar (TV dockada)
RESTORE_OUTPUT="DP-3"              # saida a restaurar no undo (ex: eDP interno via dock)
RESOLUTION="1920x1080"
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

log() { printf '[fase0][%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$1" >&2; }

# Faz o POST /api/login e guarda o cookie "auth" no cookie jar pras
# proximas chamadas. Tem que rodar antes de qualquer outra chamada na API.
login() {
    local payload raw status
    payload=$(jq -n --arg u "${APOLLO_USER}" --arg p "${APOLLO_PASS}" \
        '{username: $u, password: $p}')
    raw=$(curl "${CURL_AUTH[@]}" -w '\n%{http_code}' -X POST "${BASE_URL}/api/login" \
        -H "Content-Type: application/json" -d "${payload}")
    status="${raw##*$'\n'}"
    log "POST /api/login -> HTTP ${status}"
    if [[ ! "${status}" =~ ^2 ]]; then
        log "ERRO HTTP ${status} em POST /api/login - usuario/senha errados em Apollo?"
        exit 1
    fi
    log "login ok, cookie de sessao obtido"
}

# Monta o array "prep-cmd" inteiro como passos SEPARADOS (sem bash -c, sem
# aspas). O Apollo executa cada "do" na ordem do array e cada "undo" na ORDEM
# REVERSA (confirmado em proc_t::terminate() no process.cpp: percorre de
# tras pra frente). Isso permite ordenar do/undo de forma independente sem
# nenhum comando composto.
#
# Motivo de nao usar "bash -c '...; ...; ...'" como antes: o Apollo spawna
# comandos via Boost.Process com uma string unica, que e' dividida SO por
# espaco em branco - aspas nao viram agrupamento (bug conhecido da lib,
# https://github.com/klemens-morgenstern/boost-process/issues/128). Isso
# quebrava "bash -c '<script>'" em varios argv soltos; bash -c so pega o
# primeiro pedaco (uma aspa aberta sem fechar) e morre com erro de sintaxe
# (exit code 2) - era esse o erro real, nao permissao nem ambiente.
#
# Cada comando abaixo e' uma unica invocacao com args separados por espaco
# (sem aspas, sem ";"), que e' exatamente como Boost.Process consegue
# executar sem ambiguidade - o mesmo padrao ja comprovado nos scripts
# apollo-deck-start.sh/apollo-desktop-start.sh deste host.
#
# pkill sem match so retorna exit 1 sem imprimir nada (nao precisa mais do
# "2>/dev/null" que exigia shell); o loop de undo do Apollo so loga um
# warning em cmd com erro e CONTINUA pro proximo passo (confirmado em
# terminate(): "if (ret != 0) { BOOST_LOG(warning)... }", sem abortar) -
# preserva o mesmo comportamento tolerante a falha que o ";" garantia antes.
build_prep_cmd_json() {
    local hdr_state="disable"
    [[ "${HDR}" == "true" ]] && hdr_state="enable"

    jq -n \
        --arg target "${TARGET_OUTPUT}" \
        --arg restore "${RESTORE_OUTPUT}" \
        --arg resolution "${RESOLUTION}" \
        --arg fps "${FPS}" \
        --arg hdr_state "${hdr_state}" \
        --arg appid "${APPID}" \
        '[
            { do: "kscreen-doctor output.\($target).enable",
              undo: "kscreen-doctor output.\($target).disable" },
            { do: "kscreen-doctor output.\($target).mode.\($resolution)@\($fps)",
              undo: "sleep 1" },
            { do: "kscreen-doctor output.\($target).hdr.\($hdr_state) output.\($target).wcg.\($hdr_state)",
              undo: "kscreen-doctor output.\($restore).enable" },
            { do: "kscreen-doctor output.\($restore).disable",
              undo: "sleep 2" },
            { do: "", undo: "setsid steam steam://close/bigpicture" },
            { do: "", undo: "pkill -KILL -f AppId=\($appid)" },
            { do: "", undo: "sleep 5" },
            { do: "", undo: "pkill -TERM -f AppId=\($appid)" }
        ]'
}

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
    log "${method} ${path} -> HTTP ${status}: ${body}"
    if [[ ! "${status}" =~ ^2 ]]; then
        log "ERRO HTTP ${status} em ${method} ${path}"
        exit 1
    fi
    printf '%s' "${body}"
}

# Busca o UUID do app existente com nome $APP_NAME, ou string vazia se ainda
# nao existe. Este fork do Apollo NAO identifica apps por indice de array -
# o campo "index" do payload e simplesmente descartado pelo servidor
# (inputTree_p->erase("index") em src/process.cpp). Quem faz update-in-place
# e o "uuid": POST /api/apps com uuid de um app existente SUBSTITUI aquele
# app; com uuid vazio, CRIA um novo (ver saveApp()/migrate_apps() no
# confighttp.cpp/process.cpp do repo). Por isso nao precisa deletar+recriar:
# so precisa reenviar o mesmo uuid a cada troca de AppID.
find_app_uuid() {
    api_request GET /api/apps \
        | jq -r --arg name "${APP_NAME}" \
            '.apps[]? | select(.name == $name) | .uuid' \
        | head -n1
}

cmd_apps() {
    login
    api_request GET /api/apps | jq .
}

# Utilitario de debug: remove o app "SteamGame" via POST /api/apps/delete
# (a doc lista "DELETE /api/apps/{index}", mas o codigo real registra
# "^/api/apps/delete$" em POST com {"uuid": "..."} no corpo - mais um
# descompasso doc/codigo nesse fork).
cmd_delete() {
    login
    local uuid
    uuid=$(find_app_uuid || true)
    if [[ -z "${uuid}" ]]; then
        log "app '${APP_NAME}' nao encontrado, nada pra deletar"
        return 0
    fi
    log "deletando app '${APP_NAME}' (uuid=${uuid})"
    api_request POST /api/apps/delete "$(jq -n --arg u "${uuid}" '{uuid: $u}')"
}

cmd_apply() {
    login
    local uuid
    uuid=$(find_app_uuid || true)
    log "atualizando app '${APP_NAME}' (uuid=${uuid:-<novo>}) com AppID=${APPID}"

    local payload
    payload=$(jq -n \
        --arg name "${APP_NAME}" \
        --arg cmd "steam steam://rungameid/${APPID}" \
        --arg uuid "${uuid}" \
        --argjson prepcmd "$(build_prep_cmd_json)" \
        --arg output "/tmp/apollo-steamgame-${APPID}.log" \
        '{
            name: $name,
            cmd: $cmd,
            uuid: $uuid,
            "auto-detach": true,
            "wait-all": false,
            "exit-timeout": 5,
            "exclude-global-prep-cmd": false,
            elevated: false,
            "prep-cmd": $prepcmd,
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
    log "ATENCAO: cole os comandos abaixo via SSH no HOST, na ordem, nao rode no Deck"
    log "(e' a ordem REVERSA do array prep-cmd - assim que o Apollo executa o undo)"
    build_prep_cmd_json | jq -r '[.[] | select(.undo != "")] | reverse | .[] | .undo'
}

case "${1:-}" in
    apply) cmd_apply ;;
    undo) cmd_undo ;;
    apps) cmd_apps ;;
    delete) cmd_delete ;;
    *)
        echo "Uso: $0 {apply|undo|apps|delete}" >&2
        exit 1
        ;;
esac
