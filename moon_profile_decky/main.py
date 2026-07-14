import os
import stat
import json
import shutil
import urllib.request
import urllib.error

import decky
from moonprofile_core import ApolloClient, classify_apollo_error, detect_context

APP_NAME = "SteamGame"
RUNNER_PORT = 47991

# Mesma posicao que ja estava hardcoded no GameActionButton.tsx (canto
# inferior esquerdo, com um respiro da borda) - agora e' so o default
# inicial, configuravel pela UI (ConfigEditor.tsx).
DEFAULT_BUTTON_POSITION = {"top": "", "bottom": "2.8vw", "left": "32px", "right": ""}


def _config_path() -> str:
    return os.path.join(decky.DECKY_PLUGIN_SETTINGS_DIR, "config.json")


def _profiles_path() -> str:
    return os.path.join(decky.DECKY_PLUGIN_SETTINGS_DIR, "profiles.json")


def _default_profiles_path() -> str:
    return os.path.join(decky.DECKY_PLUGIN_DIR, "defaults", "profiles.json")


def _game_shortcuts_path() -> str:
    return os.path.join(decky.DECKY_PLUGIN_SETTINGS_DIR, "game_shortcuts.json")


class RunnerClient:
    """
    Cliente minimo pro MoonProfile Runner (daemon Tauri/Rust rodando no
    host, ver moon_profile_runner/) - checa se o processo do jogo ainda
    esta rodando (suprindo a deteccao de fim de sessao que o Apollo nao
    consegue fazer sozinho - Fase 5 do PRD: auto-detach do stream_game
    entra em modo "placebo" no Apollo depois de 5s, current_app nunca mais
    reflete a realidade) e lista os jogos Steam instalados no host (pros
    atalhos por jogo - ver gameShortcuts.ts/gameSync.ts). Sem autenticacao -
    servidor aberto na rede local (decisao explicita: numa LAN domestica ja
    confiavel, o atrito de colar um token na config nao compensa o ganho
    de seguranca).
    """

    def __init__(self, host: str, port: int):
        self.base_url = f"http://{host}:{port}"

    def session_running(self, app_id) -> bool:
        req = urllib.request.Request(f"{self.base_url}/session/status?app_id={app_id}")
        with urllib.request.urlopen(req, timeout=5) as resp:
            data = json.loads(resp.read())
            return bool(data.get("running", True))

    def list_games(self) -> list:
        req = urllib.request.Request(f"{self.base_url}/games")
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read())


def _apollo_error_response(host: str, error: Exception) -> dict:
    # Wrapper fino - classify_apollo_error (moonprofile_core, compartilhado
    # com runner.py) devolve so' a mensagem; aqui empacota no formato de
    # resposta RPC que o frontend espera ({"ok": False, "error": ...}).
    return {"ok": False, "error": classify_apollo_error(host, error)}


class Plugin:
    async def get_config(self) -> dict:
        path = _config_path()
        if not os.path.exists(path):
            return {
                "host": "",
                "username": "",
                "password": "",
                "button_position": dict(DEFAULT_BUTTON_POSITION),
                "runner_port": RUNNER_PORT,
            }
        with open(path) as f:
            config = json.load(f)
        # setdefault: configs salvos antes dessas features nao tem esses campos.
        config.setdefault("button_position", dict(DEFAULT_BUTTON_POSITION))
        config.setdefault("runner_port", RUNNER_PORT)
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

    async def get_game_shortcuts(self) -> dict:
        # Mapa host_app_id -> {deck_app_id, name, is_steam} dos atalhos por
        # jogo ja criados (ver gameShortcuts.ts/gameSync.ts) - arquivo de
        # verdade em vez de so' localStorage, pra dar controle/visibilidade
        # real sobre o que foi criado (e alimentar a aba "Jogos" da UI).
        path = _game_shortcuts_path()
        if not os.path.exists(path):
            return {}
        with open(path) as f:
            return json.load(f)

    async def save_game_shortcuts(self, shortcuts: dict) -> None:
        with open(_game_shortcuts_path(), "w") as f:
            json.dump(shortcuts, f, indent=2)

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
        return detect_context()

    async def stream_game(self, app_id: int) -> dict:
        # NAO fala com o Apollo aqui mais (login/prep-cmd/save_app) - quem
        # faz isso agora e' o proprio runner.py, sozinho, na hora do
        # lancamento (necessario pros atalhos por jogo, que o usuario pode
        # clicar "Jogar" nativo sem passar por nenhum JS nosso antes - ver
        # runner.py pro porque). Aqui so' valida ANTES de tentar lancar
        # (host configurado, tem perfil pro contexto atual) pra dar um erro
        # claro via toast em vez de deixar o runner.py falhar silencioso
        # so' com log.
        config = await self.get_config()
        if not config.get("host"):
            return {"ok": False, "error": "Configure o host do Apollo primeiro"}

        context = detect_context()
        profiles = await self.get_profiles()
        profile = next((p for p in profiles if p.get("trigger") == context), None)
        if profile is None:
            return {"ok": False, "error": f"Nenhum perfil configurado pro contexto '{context}'"}

        # NAO lancamos o moonlight aqui (subprocess.Popen direto) - o
        # Gamescope (compositor do Modo Jogo) so foca/mostra janelas
        # lancadas atraves do mecanismo real da Steam (confirmado no
        # device: um subprocess solto abre em fullscreen mas sem foco
        # nenhum, "escondido" atras da UI). A solucao (igual o MoonDeck
        # faz) e' a Steam lancar um atalho non-Steam que aponta pro
        # runner.py estatico.
        return {
            "ok": True,
            "profile": profile["name"],
            "context": context,
            "runner_path": os.path.join(decky.DECKY_PLUGIN_DIR, "runner", "runner.py"),
            "launch_env": {"MOONPROFILE_HOST_APP_ID": str(app_id)},
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

    async def check_session_status(self, app_id: int) -> dict:
        # Chamado pelo frontend em polling enquanto uma sessao esta ativa
        # (ver stream.ts), pra detectar quando o jogo fecha por dentro sem
        # passar por "Fechar conexao". Se o host do Apollo nao estiver
        # configurado ou o Runner ficar inalcancavel, assume "ainda
        # rodando" (running=True) - nao queremos fechar a sessao por
        # engano so' porque o daemon caiu ou nunca foi instalado (fallback
        # seguro, equivalente a nao ter essa feature ainda).
        #
        # Mesmo host do Apollo (config["host"]) - Runner e Apollo sempre
        # rodam na mesma maquina, nao faz sentido pedir o IP duas vezes.
        config = await self.get_config()
        host = config.get("host")
        if not host:
            return {"ok": False, "running": True}

        try:
            client = RunnerClient(host, config.get("runner_port", RUNNER_PORT))
            running = client.session_running(app_id)
            return {"ok": True, "running": running}
        except (urllib.error.URLError, OSError, json.JSONDecodeError) as e:
            decky.logger.error(f"Falha ao falar com o MoonProfile Runner: {e}")
            return {"ok": False, "running": True}

    async def list_host_games(self) -> dict:
        # Chamado pelo frontend (gameSync.ts) pro botao "Sincronizar jogos
        # do host" - lista os jogos Steam instalados no host via Runner,
        # pra criar um atalho por jogo no Deck (ver gameShortcuts.ts).
        config = await self.get_config()
        host = config.get("host")
        if not host:
            return {"ok": False, "error": "Configure o host do Apollo primeiro (aba Config do Apollo)", "games": []}

        try:
            client = RunnerClient(host, config.get("runner_port", RUNNER_PORT))
            games = client.list_games()
            return {
                "ok": True,
                "games": games,
                "runner_path": os.path.join(decky.DECKY_PLUGIN_DIR, "runner", "runner.py"),
            }
        except (urllib.error.URLError, OSError, json.JSONDecodeError) as e:
            decky.logger.error(f"Falha ao listar jogos do MoonProfile Runner: {e}")
            return {"ok": False, "error": f"Nao consegui falar com o MoonProfile Runner: {e}", "games": []}

    async def _main(self):
        decky.logger.info("MoonProfile carregado")

    async def _unload(self):
        decky.logger.info("MoonProfile descarregado")

    async def _uninstall(self):
        decky.logger.info("MoonProfile desinstalado")
