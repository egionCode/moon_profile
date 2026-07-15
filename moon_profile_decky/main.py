import os
import stat
import json
import shutil
import urllib.request
import urllib.error

import decky
from moonprofile_core import RUNNER_PORT, detect_context


def _config_path() -> str:
    return os.path.join(decky.DECKY_PLUGIN_SETTINGS_DIR, "config.json")


def _profiles_path() -> str:
    return os.path.join(decky.DECKY_PLUGIN_SETTINGS_DIR, "profiles.json")


def _default_profiles_path() -> str:
    return os.path.join(decky.DECKY_PLUGIN_DIR, "defaults", "profiles.json")


def _game_shortcuts_path() -> str:
    return os.path.join(decky.DECKY_PLUGIN_SETTINGS_DIR, "game_shortcuts.json")


def _streaming_collection_path() -> str:
    return os.path.join(decky.DECKY_PLUGIN_SETTINGS_DIR, "streaming_collection.json")


class RunnerClient:
    """
    Cliente minimo pro MoonProfile Runner (daemon Tauri/Rust rodando no
    host, ver moon_profile_runner/) - lista os jogos Steam instalados no
    host (pros atalhos por jogo - ver gameShortcuts.ts/gameSync.ts) e pede
    o fechamento da sessao ativa (o Runner sozinho detecta fim de sessao
    via processo real do SO - Fase 5 do PRD: auto-detach do stream_game
    entra em modo "placebo" no Apollo depois de 5s, current_app nunca mais
    reflete a realidade - mas fechar manualmente tambem passa por aqui,
    reaproveitando a mesma sessao registrada por runner.py). Sem
    autenticacao - servidor aberto na rede local (decisao explicita: numa
    LAN domestica ja confiavel, o atrito de colar um token na config nao
    compensa o ganho de seguranca).
    """

    def __init__(self, host: str, port: int):
        self.base_url = f"http://{host}:{port}"

    def list_games(self) -> list:
        req = urllib.request.Request(f"{self.base_url}/games")
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read())

    def list_displays(self) -> list:
        # Monitores/outputs de tela do host (via kscreen-doctor -j, ver
        # displays.rs) - alimenta o ProfileEditor.tsx com opcoes de
        # verdade em vez do usuario digitar o nome do output na mao.
        req = urllib.request.Request(f"{self.base_url}/displays")
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read())

    def close_session(self) -> dict:
        # Sem corpo - o Runner ja tem host_app_id/credenciais guardados em
        # memoria desde que runner.py registrou a sessao no lancamento
        # (ver session.rs). Se nao houver sessao registrada, o Runner
        # responde {"ok": False, "error": "..."} (nao levanta excecao).
        req = urllib.request.Request(f"{self.base_url}/session/close", data=b"", method="POST")
        with urllib.request.urlopen(req, timeout=15) as resp:
            return json.loads(resp.read())


class Plugin:
    async def get_config(self) -> dict:
        path = _config_path()
        if not os.path.exists(path):
            return {
                "host": "",
                "username": "",
                "password": "",
                "runner_port": RUNNER_PORT,
            }
        with open(path) as f:
            config = json.load(f)
        # setdefault: configs salvos antes dessas features nao tem esse campo.
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

    async def get_streaming_collection_id(self):
        # Id persistido da colecao "Streaming" (ver gameCollection.ts) -
        # nao depende so' da busca por tag a cada sincronizacao: o tag e'
        # derivado do nome exibido, quebra se o usuario renomear a colecao
        # manualmente na Steam; o id sobrevive a isso.
        path = _streaming_collection_path()
        if not os.path.exists(path):
            return None
        with open(path) as f:
            return json.load(f).get("collection_id")

    async def save_streaming_collection_id(self, collection_id: str) -> None:
        with open(_streaming_collection_path(), "w") as f:
            json.dump({"collection_id": collection_id}, f)

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

    async def stop_stream(self) -> dict:
        # O Runner (host) e' quem fecha de verdade - ele ja tem a sessao
        # registrada por runner.py no lancamento (app_id + credenciais em
        # memoria), mata o jogo se ainda estiver vivo e restaura a tela
        # ANTES de avisar o Apollo. O Runner NAO e' mais opcional: o
        # Apollo nao tem prep-cmd nenhum (nem do, nem undo) - chamar ele
        # direto sem o Runner so' derrubaria a conexao sem restaurar nada,
        # entao um erro aqui e' reportado como erro de verdade, nao um
        # fallback silencioso.
        config = await self.get_config()
        if not config.get("host"):
            return {"ok": False, "error": "Configure o host do Apollo primeiro"}

        host = config["host"]

        try:
            client = RunnerClient(host, config.get("runner_port", RUNNER_PORT))
            result = client.close_session()
            if not result.get("ok"):
                decky.logger.error(f"Runner nao conseguiu fechar a sessao: {result.get('error')}")
            return result
        except (urllib.error.URLError, OSError, json.JSONDecodeError) as e:
            decky.logger.error(f"Falha ao falar com o MoonProfile Runner pra fechar a sessao: {e}")
            return {"ok": False, "error": f"Nao consegui falar com o MoonProfile Runner: {e}"}

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

    async def list_host_displays(self) -> dict:
        # Chamado pelo ProfileEditor.tsx pra popular o select de
        # "Output alvo" e a lista de outputs a desabilitar com os
        # monitores de verdade do host, em vez do usuario digitar o
        # nome na mao.
        config = await self.get_config()
        host = config.get("host")
        if not host:
            return {"ok": False, "error": "Configure o host do Apollo primeiro (aba Config do Apollo)", "displays": []}

        try:
            client = RunnerClient(host, config.get("runner_port", RUNNER_PORT))
            displays = client.list_displays()
            return {"ok": True, "displays": displays}
        except (urllib.error.URLError, OSError, json.JSONDecodeError) as e:
            decky.logger.error(f"Falha ao listar monitores do MoonProfile Runner: {e}")
            return {"ok": False, "error": f"Nao consegui falar com o MoonProfile Runner: {e}", "displays": []}

    async def _main(self):
        decky.logger.info("MoonProfile carregado")

    async def _unload(self):
        decky.logger.info("MoonProfile descarregado")

    async def _uninstall(self):
        decky.logger.info("MoonProfile desinstalado")
