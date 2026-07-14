import os
import stat
import json
import shutil
import ssl
import http.cookiejar
import urllib.request
import urllib.error

import decky

APP_NAME = "SteamGame"
APOLLO_PORT = 47990

# Mesma posicao que ja estava hardcoded no GameActionButton.tsx (canto
# inferior esquerdo, com um respiro da borda) - agora e' so o default
# inicial, configuravel pela UI (ConfigEditor.tsx).
DEFAULT_BUTTON_POSITION = {"top": "", "bottom": "2.8vw", "left": "32px", "right": ""}

# Mapeamento pro flag --video-codec do moonlight CLI: nosso modelo de dados
# usa "H264" (sem ponto) mas o CLI espera "H.264" literal.
CODEC_FLAGS = {"HEVC": "HEVC", "AV1": "AV1", "H264": "H.264"}


def _config_path() -> str:
    return os.path.join(decky.DECKY_PLUGIN_SETTINGS_DIR, "config.json")


def _profiles_path() -> str:
    return os.path.join(decky.DECKY_PLUGIN_SETTINGS_DIR, "profiles.json")


def _default_profiles_path() -> str:
    return os.path.join(decky.DECKY_PLUGIN_DIR, "defaults", "profiles.json")


def _detect_context() -> str:
    """Retorna 'docked' se algum display externo estiver conectado, senao 'handheld'."""
    drm_path = "/sys/class/drm"
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


def _build_prep_cmd(host_cfg: dict, app_id) -> list:
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


def _apollo_error_response(host: str, error: Exception) -> dict:
    """
    Traduz uma excecao de rede/HTTP falando com o Apollo numa mensagem clara
    pro usuario, em vez do texto cru da excecao Python. Usado tanto em
    stream_game quanto em stop_stream - o mesmo catch (URLError, OSError)
    ja cobre tudo aqui, porque HTTPError e' subclasse de URLError.

    Credenciais erradas: confirmado no codigo real do Apollo
    (confighttp.cpp:login()) que ele responde 401 nesse caso (nao 403/400
    generico) - e' um sinal confiavel pra diferenciar de "host offline".
    """
    if isinstance(error, urllib.error.HTTPError):
        if error.code == 401:
            return {"ok": False, "error": "Usuario ou senha do Apollo incorretos"}
        return {"ok": False, "error": f"Apollo respondeu com erro inesperado (HTTP {error.code})"}
    if isinstance(error, json.JSONDecodeError):
        return {
            "ok": False,
            "error": f"Apollo respondeu algo que nao e' JSON - confira se {host}:{APOLLO_PORT} e' mesmo o Apollo",
        }
    return {
        "ok": False,
        "error": f"Nao consegui alcancar o Apollo em {host}:{APOLLO_PORT} - confira se o host esta ligado e na mesma rede",
    }


class Plugin:
    async def get_config(self) -> dict:
        path = _config_path()
        if not os.path.exists(path):
            return {"host": "", "username": "", "password": "", "button_position": dict(DEFAULT_BUTTON_POSITION)}
        with open(path) as f:
            config = json.load(f)
        # setdefault: configs salvos antes dessa feature nao tem esse campo.
        config.setdefault("button_position", dict(DEFAULT_BUTTON_POSITION))
        return config

    async def save_config(self, config: dict) -> None:
        path = _config_path()
        with open(path, "w") as f:
            json.dump(config, f)
        os.chmod(path, stat.S_IRUSR | stat.S_IWUSR)  # 0600 - guarda credencial do Apollo

    async def get_profiles(self) -> list:
        path = _profiles_path()
        if not os.path.exists(path):
            default_path = _default_profiles_path()
            if os.path.exists(default_path):
                shutil.copyfile(default_path, path)
            else:
                with open(path, "w") as f:
                    json.dump([], f)
        with open(path) as f:
            return json.load(f)

    async def save_profiles(self, profiles: list) -> None:
        with open(_profiles_path(), "w") as f:
            json.dump(profiles, f, indent=2)

    async def get_logs(self, lines: int) -> str:
        # decky.DECKY_PLUGIN_LOG e' o arquivo da sessao ATUAL (nome com
        # timestamp, calculado uma vez no import do modulo "decky") - o
        # loader mantem so os 4 mais recentes em DECKY_PLUGIN_LOG_DIR,
        # apagando o resto sozinho.
        try:
            with open(decky.DECKY_PLUGIN_LOG) as f:
                all_lines = f.readlines()
            return "".join(all_lines[-lines:]) if all_lines else "(log vazio)"
        except OSError as e:
            return f"Nao consegui ler o log: {e}"

    async def detect_context(self) -> str:
        return _detect_context()

    async def stream_game(self, app_id: int) -> dict:
        config = await self.get_config()
        if not config.get("host"):
            return {"ok": False, "error": "Configure o host do Apollo primeiro"}

        context = _detect_context()
        profiles = await self.get_profiles()
        profile = next((p for p in profiles if p.get("trigger") == context), None)
        if profile is None:
            return {"ok": False, "error": f"Nenhum perfil configurado pro contexto '{context}'"}

        try:
            client = ApolloClient(config["host"], config["username"], config["password"])
            client.login()
            uuid = client.find_app_uuid(APP_NAME)
            prep_cmd = _build_prep_cmd(profile["host"], app_id)
            client.save_app({
                "name": APP_NAME,
                "cmd": f"steam steam://rungameid/{app_id}",
                "uuid": uuid,
                "auto-detach": True,
                "wait-all": False,
                "exit-timeout": 5,
                "exclude-global-prep-cmd": False,
                "elevated": False,
                "prep-cmd": prep_cmd,
                "output": f"/tmp/apollo-steamgame-{app_id}.log",
            })
        except (urllib.error.URLError, OSError, json.JSONDecodeError) as e:
            decky.logger.error(f"Falha ao falar com Apollo: {e}")
            return _apollo_error_response(config["host"], e)

        moonlight_cfg = profile["moonlight"]
        codec_flag = CODEC_FLAGS.get(moonlight_cfg["codec"], "auto")

        # NAO lancamos o moonlight aqui (subprocess.Popen direto) - o
        # Gamescope (compositor do Modo Jogo) so foca/mostra janelas
        # lancadas atraves do mecanismo real da Steam (confirmado no
        # device: um subprocess solto abre em fullscreen mas sem foco
        # nenhum, "escondido" atras da UI). A solucao (igual o MoonDeck
        # faz) e' a Steam lancar um atalho ("MoonProfile Launcher") que
        # aponta pro runner.py estatico; o runner le essas variaveis do
        # ambiente (injetadas pela propria Steam via launch options) e
        # executa o flatpak/moonlight de verdade. Quem monta o atalho e
        # dispara o lancamento e' o FRONTEND (SteamClient so existe la).
        return {
            "ok": True,
            "profile": profile["name"],
            "context": context,
            "runner_path": os.path.join(decky.DECKY_PLUGIN_DIR, "runner", "runner.py"),
            "launch_env": {
                "MOONPROFILE_HOST": config["host"],
                "MOONPROFILE_APP_NAME": APP_NAME,
                "MOONPROFILE_RESOLUTION": moonlight_cfg["resolution"],
                "MOONPROFILE_FPS": str(moonlight_cfg["fps"]),
                "MOONPROFILE_BITRATE": str(moonlight_cfg["bitrate"]),
                "MOONPROFILE_CODEC": codec_flag,
                "MOONPROFILE_HDR": "1" if moonlight_cfg.get("hdr") else "0",
                "MOONPROFILE_LOG_PATH": os.path.join(decky.DECKY_PLUGIN_RUNTIME_DIR, "moonlight.log"),
            },
        }

    async def stop_stream(self) -> dict:
        config = await self.get_config()
        if not config.get("host"):
            return {"ok": False, "error": "Configure o host do Apollo primeiro"}
        try:
            client = ApolloClient(config["host"], config["username"], config["password"])
            client.login()
            client.close_app()
        except (urllib.error.URLError, OSError, json.JSONDecodeError) as e:
            decky.logger.error(f"Falha ao fechar sessao no Apollo: {e}")
            return _apollo_error_response(config["host"], e)
        return {"ok": True}

    async def _main(self):
        decky.logger.info("MoonProfile carregado")

    async def _unload(self):
        decky.logger.info("MoonProfile descarregado")

    async def _uninstall(self):
        decky.logger.info("MoonProfile desinstalado")
