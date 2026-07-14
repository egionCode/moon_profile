"""
Logica compartilhada entre main.py (roda dentro do Decky Loader) e
runner.py (roda como processo solto, exec'ado por um atalho Steam - ver
docs/prd.md, secao dos atalhos por jogo). Os dois precisam falar com o
Apollo e detectar contexto do mesmo jeito, entao isso mora aqui em vez de
duplicado nos dois lugares.

runner.py NAO roda dentro do ambiente do Decky Loader - importa este
modulo inserindo o caminho de py_modules/ manualmente no sys.path (ver
runner.py), nao pelo mecanismo normal do loader.
"""

import json
import ssl
import http.cookiejar
import urllib.request
import urllib.error

APOLLO_PORT = 47990
RUNNER_PORT = 47991

# Mapeamento pro flag --video-codec do moonlight CLI: nosso modelo de dados
# usa "H264" (sem ponto) mas o CLI espera "H.264" literal.
CODEC_FLAGS = {"HEVC": "HEVC", "AV1": "AV1", "H264": "H.264"}


def detect_context(drm_path: str = "/sys/class/drm") -> str:
    """Retorna 'docked' se algum display externo estiver conectado, senao 'handheld'.

    drm_path e' configuravel pra permitir testar contra uma fixture (uma
    pasta fake com a mesma estrutura de /sys/class/drm) em vez de depender
    do hardware de verdade da maquina rodando o teste.
    """
    import os

    for entry in os.listdir(drm_path):
        if not entry.startswith("card"):
            continue
        # eDP = tela interna do proprio Deck, sempre presente e sempre
        # "connected" - e "eDP-1" contem "DP" como substring, entao sem essa
        # exclusao a funcao SEMPRE retornava "docked", dockado ou nao
        # (bug real, encontrado testando no device de verdade).
        if "eDP" in entry:
            continue
        if not ("HDMI" in entry or "DP" in entry):
            continue
        status_file = os.path.join(drm_path, entry, "status")
        if os.path.exists(status_file):
            with open(status_file) as f:
                if f.read().strip() == "connected":
                    return "docked"
    return "handheld"


def build_prep_cmd(host_cfg: dict, app_id) -> list:
    """
    Monta o array "prep-cmd" em passos simples (sem "bash -c '...'", sem
    aspas). O Apollo spawna comandos via Boost.Process com uma string
    unica, dividida SO por espaco em branco - aspas nao viram agrupamento
    (bug conhecido da lib: github.com/klemens-morgenstern/boost-process/
    issues/128). Um "bash -c '<script com ; >'" vira argv quebrado e o
    bash morre com erro de sintaxe (exit code 2). Validado empiricamente
    na Fase 0 (poc/fase0.sh) contra o Apollo de verdade.

    O Apollo executa os "do" na ordem do array e os "undo" na ordem
    REVERSA (proc_t::terminate() em process.cpp) - os passos de undo
    abaixo sao posicionados de tras pra frente por isso.
    """
    target = host_cfg["target_output"]
    resolution = host_cfg["resolution"]
    fps = host_cfg["fps"]
    hdr_state = "enable" if host_cfg.get("hdr") else "disable"
    disable_outputs = host_cfg.get("disable_outputs", [])
    n = len(disable_outputs)
    length = 7 + n

    do_actions = [
        f"kscreen-doctor output.{target}.enable",
        f"kscreen-doctor output.{target}.mode.{resolution}@{fps}",
        f"kscreen-doctor output.{target}.hdr.{hdr_state} output.{target}.wcg.{hdr_state}",
    ] + [f"kscreen-doctor output.{o}.disable" for o in disable_outputs]

    steps = [{"do": "", "undo": ""} for _ in range(length)]
    for i, action in enumerate(do_actions):
        steps[i]["do"] = action

    # 20s de graca (nao 5s): o "reaper" do Steam e' quem derruba a arvore
    # inteira do jogo (proton/wine/exe) quando recebe o SIGTERM. Se o jogo
    # demorar mais que a janela de graca pra responder, o SIGKILL seguinte
    # mata o reaper ANTES dele terminar de derrubar os filhos, deixando
    # processos orfaos rodando pra sempre (confirmado no device: 3 arvores
    # de RESIDENT EVIL 4 orfas acumuladas de testes anteriores, competindo
    # pela GPU - por isso nada aparecia na tela). PRD ja antecipava esse
    # risco ("sleep 5 pode nao ser suficiente pra jogos com autosave raro").
    steps[length - 1]["undo"] = f"pkill -TERM -f AppId={app_id}"
    steps[length - 2]["undo"] = "sleep 20"
    steps[length - 3]["undo"] = f"pkill -KILL -f AppId={app_id}"
    steps[length - 4]["undo"] = "setsid steam steam://close/bigpicture"
    steps[length - 5]["undo"] = "sleep 2"
    for k, o in enumerate(disable_outputs):
        steps[length - 6 - k]["undo"] = f"kscreen-doctor output.{o}.enable"
    steps[length - 6 - n]["undo"] = "sleep 1"
    # So desliga o target_output no undo se tiver outro output pra restaurar
    # no lugar (disable_outputs nao vazio) - senao a tela fica preta sem
    # nada pra mostrar (confirmado no device: target_output = monitor
    # principal de verdade, disable_outputs=[] pra debug, desligar ele sem
    # ligar nada de volta apagou a tela toda).
    if disable_outputs:
        steps[0]["undo"] = f"kscreen-doctor output.{target}.disable"

    return steps


def build_restore_commands(host_cfg: dict) -> list:
    """
    So' a parte de RESTAURAR TELA do undo de build_prep_cmd (fecha o Big
    Picture, religa os outputs desativados, desliga o target) - SEM o
    pkill/sleep-20/pkill de matar o jogo. Em ordem de EXECUCAO (nao e'
    array do Apollo, e' uma lista simples pro Runner rodar direto via
    shell, um comando por vez, na ordem que aparece).

    Usado so' pelo fechamento AUTONOMO (watchdog do Runner - session.rs):
    quando o watchdog chama isso, o processo do jogo JA foi confirmado
    morto (is_app_id_running() == False) - o periodo de graca de 20s do
    build_prep_cmd existe pra um jogo que TALVEZ ainda esteja vivo (ver
    comentario la'), o que nao e' o caso aqui. Rodar esses 20s de espera
    de qualquer jeito so' atrasa a resposta do usuario sem nenhum
    beneficio (nada pra matar). O fechamento MANUAL ("Fechar conexao")
    continua usando o array cheio de build_prep_cmd via Apollo, porque
    nesse caso o jogo pode genuinamente ainda estar rodando.
    """
    target = host_cfg["target_output"]
    disable_outputs = host_cfg.get("disable_outputs", [])

    commands = ["setsid steam steam://close/bigpicture", "sleep 2"]
    commands.extend(f"kscreen-doctor output.{o}.enable" for o in disable_outputs)
    commands.append("sleep 1")
    if disable_outputs:
        commands.append(f"kscreen-doctor output.{target}.disable")
    return commands


class ApolloClient:
    """
    Cliente minimo (so stdlib, sem 'requests') pra API REST do Apollo.

    Este fork (ClassicOldSong/Apollo) NAO usa HTTP Basic Auth, apesar do
    que diz docs/api.md - authenticate() em confighttp.cpp so checa um
    cookie de sessao "auth", obtido via POST /api/login. Validado na
    Fase 0 contra o Apollo de verdade.
    """

    def __init__(self, host: str, username: str, password: str, port: int = APOLLO_PORT):
        self.base_url = f"https://{host}:{port}"
        self.username = username
        self.password = password

        ctx = ssl.create_default_context()
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE  # certificado auto-assinado do Apollo
        self.opener = urllib.request.build_opener(
            urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()),
            urllib.request.HTTPSHandler(context=ctx),
        )

    def _request(self, method: str, path: str, data: dict | None = None) -> dict:
        body = json.dumps(data).encode() if data is not None else None
        req = urllib.request.Request(f"{self.base_url}{path}", data=body, method=method)
        if body is not None:
            req.add_header("Content-Type", "application/json")
        with self.opener.open(req, timeout=10) as resp:
            raw = resp.read()
            return json.loads(raw) if raw else {}

    def login(self) -> None:
        self._request("POST", "/api/login", {"username": self.username, "password": self.password})

    def find_app_uuid(self, name: str) -> str:
        # Apps sao identificados por uuid, nao por indice de array - o
        # campo "index" do exemplo antigo de doc do Sunshine e' descartado
        # pelo servidor (migrate_apps() em process.cpp).
        apps = self._request("GET", "/api/apps").get("apps", [])
        for app in apps:
            if app.get("name") == name:
                return app.get("uuid", "")
        return ""

    def save_app(self, payload: dict) -> dict:
        return self._request("POST", "/api/apps", payload)

    def close_app(self) -> dict:
        # Termina a sessao/app rodando no momento no Apollo (proc::terminate()
        # em process.cpp), o que dispara o "undo" do prep-cmd em ordem
        # reversa - mata o jogo pelo AppId e restaura os displays.
        return self._request("POST", "/api/apps/close", {})


def classify_apollo_error(host: str, error: Exception) -> str:
    """
    Traduz uma excecao de rede/HTTP falando com o Apollo numa mensagem clara,
    em vez do texto cru da excecao Python.

    Credenciais erradas: confirmado no codigo real do Apollo
    (confighttp.cpp:login()) que ele responde 401 nesse caso (nao 403/400
    generico) - e' um sinal confiavel pra diferenciar de "host offline".
    """
    if isinstance(error, urllib.error.HTTPError):
        if error.code == 401:
            return "Usuario ou senha do Apollo incorretos"
        return f"Apollo respondeu com erro inesperado (HTTP {error.code})"
    if isinstance(error, json.JSONDecodeError):
        return f"Apollo respondeu algo que nao e' JSON - confira se {host}:{APOLLO_PORT} e' mesmo o Apollo"
    return f"Nao consegui alcancar o Apollo em {host}:{APOLLO_PORT} - confira se o host esta ligado e na mesma rede"
