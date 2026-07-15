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
    Minimal client for the MoonProfile Runner (Tauri/Rust daemon running on
    the host, see moon_profile_runner/): lists the Steam games installed on
    the host (for per-game shortcuts, see gameShortcuts.ts/gameSync.ts) and
    requests the active session to be closed (the Runner detects session
    end on its own via the real OS process, PRD Phase 5: stream_game's
    auto-detach falls into "placebo" mode on Apollo after 5s, current_app
    never reflects reality again, but manually closing also goes through
    here, reusing the same session registered by runner.py). No
    authentication: server open on the local network (explicit decision:
    on an already-trusted home LAN, the friction of pasting a token into
    the config isn't worth the security gain).
    """

    def __init__(self, host: str, port: int):
        self.base_url = f"http://{host}:{port}"

    def list_games(self) -> list:
        req = urllib.request.Request(f"{self.base_url}/games")
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read())

    def list_displays(self) -> list:
        # Host display outputs (via kscreen-doctor -j, see displays.rs):
        # feeds ProfileEditor.tsx with real options instead of the user
        # having to type the output name by hand.
        req = urllib.request.Request(f"{self.base_url}/displays")
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read())

    def close_session(self) -> dict:
        # No body: the Runner already has host_app_id/credentials kept in
        # memory since runner.py registered the session at launch (see
        # session.rs). If no session is registered, the Runner responds
        # with {"ok": False, "error": "..."} (it doesn't raise).
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
        # setdefault: configs saved before these features existed don't have this field.
        config.setdefault("runner_port", RUNNER_PORT)
        return config

    async def save_config(self, config: dict) -> None:
        path = _config_path()
        with open(path, "w") as f:
            json.dump(config, f)
        os.chmod(path, stat.S_IRUSR | stat.S_IWUSR)  # 0600: holds the Apollo credential

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
        # Map of host_app_id -> {deck_app_id, name, is_steam} for the
        # per-game shortcuts already created (see gameShortcuts.ts/gameSync.ts):
        # a real file instead of just localStorage, to give real control and
        # visibility over what was created (and feed the "Games" tab in the UI).
        path = _game_shortcuts_path()
        if not os.path.exists(path):
            return {}
        with open(path) as f:
            return json.load(f)

    async def save_game_shortcuts(self, shortcuts: dict) -> None:
        with open(_game_shortcuts_path(), "w") as f:
            json.dump(shortcuts, f, indent=2)

    async def get_streaming_collection_id(self):
        # Persisted id of the "Streaming" collection (see gameCollection.ts):
        # doesn't rely solely on the tag lookup on every sync, since the tag
        # is derived from the displayed name and breaks if the user renames
        # the collection manually in Steam; the id survives that.
        path = _streaming_collection_path()
        if not os.path.exists(path):
            return None
        with open(path) as f:
            return json.load(f).get("collection_id")

    async def save_streaming_collection_id(self, collection_id: str) -> None:
        with open(_streaming_collection_path(), "w") as f:
            json.dump({"collection_id": collection_id}, f)

    async def get_logs(self, lines: int) -> str:
        # decky.DECKY_PLUGIN_LOG is the CURRENT session's file (name with a
        # timestamp, computed once on import of the "decky" module): the
        # loader keeps only the 4 most recent in DECKY_PLUGIN_LOG_DIR,
        # deleting the rest on its own.
        try:
            with open(decky.DECKY_PLUGIN_LOG) as f:
                all_lines = f.readlines()
            return "".join(all_lines[-lines:]) if all_lines else "(empty log)"
        except OSError as e:
            return f"Could not read the log: {e}"

    async def detect_context(self) -> str:
        return detect_context()

    async def stop_stream(self) -> dict:
        # The Runner (host) is the one that actually closes things: it
        # already has the session registered by runner.py at launch
        # (app_id + credentials in memory), kills the game if it's still
        # alive, and restores the display BEFORE notifying Apollo. The
        # Runner is NOT optional anymore: Apollo has no prep-cmd at all
        # (neither do nor undo), calling it directly without the Runner
        # would just drop the connection without restoring anything, so
        # an error here is reported as a real error, not a silent fallback.
        config = await self.get_config()
        if not config.get("host"):
            return {"ok": False, "error": "Configure the Apollo host first"}

        host = config["host"]

        try:
            client = RunnerClient(host, config.get("runner_port", RUNNER_PORT))
            result = client.close_session()
            if not result.get("ok"):
                decky.logger.error(f"Runner failed to close the session: {result.get('error')}")
            return result
        except (urllib.error.URLError, OSError, json.JSONDecodeError) as e:
            decky.logger.error(f"Failed to talk to the MoonProfile Runner to close the session: {e}")
            return {"ok": False, "error": f"Could not talk to the MoonProfile Runner: {e}"}

    async def list_host_games(self) -> dict:
        # Called by the frontend (gameSync.ts) for the "Sync host games"
        # button: lists the Steam games installed on the host via the
        # Runner, to create a per-game shortcut on the Deck (see gameShortcuts.ts).
        config = await self.get_config()
        host = config.get("host")
        if not host:
            return {"ok": False, "error": "Configure the Apollo host first (Apollo Config tab)", "games": []}

        try:
            client = RunnerClient(host, config.get("runner_port", RUNNER_PORT))
            games = client.list_games()
            return {
                "ok": True,
                "games": games,
                "runner_path": os.path.join(decky.DECKY_PLUGIN_DIR, "runner", "runner.py"),
            }
        except (urllib.error.URLError, OSError, json.JSONDecodeError) as e:
            decky.logger.error(f"Failed to list games from the MoonProfile Runner: {e}")
            return {"ok": False, "error": f"Could not talk to the MoonProfile Runner: {e}", "games": []}

    async def list_host_displays(self) -> dict:
        # Called by ProfileEditor.tsx to populate the "Target output"
        # select and the list of outputs to disable with the host's real
        # monitors, instead of the user typing the name by hand.
        config = await self.get_config()
        host = config.get("host")
        if not host:
            return {"ok": False, "error": "Configure the Apollo host first (Apollo Config tab)", "displays": []}

        try:
            client = RunnerClient(host, config.get("runner_port", RUNNER_PORT))
            displays = client.list_displays()
            return {"ok": True, "displays": displays}
        except (urllib.error.URLError, OSError, json.JSONDecodeError) as e:
            decky.logger.error(f"Failed to list displays from the MoonProfile Runner: {e}")
            return {"ok": False, "error": f"Could not talk to the MoonProfile Runner: {e}", "displays": []}

    async def _main(self):
        decky.logger.info("MoonProfile loaded")

    async def _unload(self):
        decky.logger.info("MoonProfile unloaded")

    async def _uninstall(self):
        decky.logger.info("MoonProfile uninstalled")
