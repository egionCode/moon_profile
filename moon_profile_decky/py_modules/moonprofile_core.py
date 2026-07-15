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


def build_display_commands(host_cfg: dict) -> list:
    """
    Comandos de LIGAR/CONFIGURAR a tela do host (kscreen-doctor: ativa o
    target_output, seta modo/resolucao, HDR, desativa os outros
    outputs) - em ordem de EXECUCAO simples (nao e' mais array do Apollo,
    e' uma lista de strings que o Runner (Rust) roda direto via shell,
    ANTES de dar exec no Moonlight - ver session.rs/register_session).

    O Apollo NAO tem mais prep-cmd nenhum (nem do, nem undo) - decisao
    explicita de tirar essa responsabilidade dele (mais "plug and play":
    o Apollo so' precisa saber conectar e rodar o "cmd", quem manda de
    verdade na tela e no ciclo de vida da sessao e' o Runner, que ja
    precisa saber ligar/desligar telas pro fechamento mesmo - ver
    build_restore_commands). O Runner deixou de ser opcional por causa
    disso: sem ele, a troca de tela simplesmente nao acontece.
    """
    target = host_cfg["target_output"]
    resolution = host_cfg["resolution"]
    fps = host_cfg["fps"]
    hdr_state = "enable" if host_cfg.get("hdr") else "disable"
    disable_outputs = host_cfg.get("disable_outputs", [])

    commands = [
        f"kscreen-doctor output.{target}.enable",
        f"kscreen-doctor output.{target}.mode.{resolution}@{fps}",
        f"kscreen-doctor output.{target}.hdr.{hdr_state} output.{target}.wcg.{hdr_state}",
    ]
    commands.extend(f"kscreen-doctor output.{o}.disable" for o in disable_outputs)
    return commands


def build_restore_commands(host_cfg: dict) -> list:
    """
    Comandos de RESTAURAR a tela do host (fecha o Big Picture, religa os
    outputs desativados, desliga o target) - em ordem de EXECUCAO. Usado
    pelo Runner tanto no fechamento AUTONOMO (watchdog detecta que o jogo
    fechou sozinho) quanto no MANUAL ("Fechar conexao"), depois de
    garantir (session.rs) que o processo do jogo ja acabou de verdade -
    ver kill_game_process no lado Rust, que cuida de matar o jogo ANTES
    de rodar isso quando o fechamento e' manual (pode estar genuinamente
    vivo ainda).
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
        # Termina a conexao/stream ativa no Apollo (proc::terminate() em
        # process.cpp) - o Apollo NAO tem prep-cmd configurado (ver
        # build_display_commands/build_restore_commands), entao isso so'
        # derruba a conexao em si; matar o jogo e restaurar a tela e'
        # responsabilidade do Runner (Rust), que roda ANTES de chamar isso.
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
