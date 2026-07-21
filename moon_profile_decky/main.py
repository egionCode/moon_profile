import os
import socket
import stat
import json
import shutil
import urllib.request
import urllib.error

import decky
from moonprofile_core import RUNNER_PORT, build_magic_packet, detect_context


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

    def health(self) -> dict:
        # Short 2s timeout, deliberately shorter than the other methods'
        # 10-15s: polling this from Quick Access every few seconds
        # shouldn't hang waiting on a host that might already be off.
        req = urllib.request.Request(f"{self.base_url}/health")
        with urllib.request.urlopen(req, timeout=2) as resp:
            return json.loads(resp.read())

    def get_mac(self) -> dict:
        req = urllib.request.Request(f"{self.base_url}/system/mac")
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read())

    def shutdown(self) -> dict:
        req = urllib.request.Request(f"{self.base_url}/system/shutdown", data=b"", method="POST")
        with urllib.request.urlopen(req, timeout=10) as resp:
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
                "steamgriddb_api_key": "",
                "mac_address": "",
            }
        with open(path) as f:
            config = json.load(f)
        # setdefault: configs saved before these features existed don't have this field.
        config.setdefault("runner_port", RUNNER_PORT)
        config.setdefault("steamgriddb_api_key", "")
        config.setdefault("mac_address", "")
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

    async def log_frontend_error(self, message: str) -> None:
        # Bridge for frontend-side failures (e.g. gameArtwork.ts's
        # best-effort CDN/SteamGridDB fetches) to land in the SAME log the
        # "Logs" tab reads - console.error in the frontend only reaches the
        # Steam WebHelper's own devtools, decky.logger writes to
        # DECKY_PLUGIN_LOG (this process, the Python backend), a different
        # place entirely.
        decky.logger.error(f"[frontend] {message}")

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

    async def get_host_status(self) -> str:
        # Polled by QuickAccessContent.tsx to show a status indicator and
        # gate the shutdown/wake buttons (only one of them makes sense at
        # a time). GET /health is a cheap probe with a short timeout
        # (RunnerClient.health), it doesn't fire a sync notification like
        # /games would.
        config = await self.get_config()
        if not config.get("host"):
            return "unconfigured"

        try:
            client = RunnerClient(config["host"], config.get("runner_port", RUNNER_PORT))
            client.health()
            return "online"
        except (urllib.error.URLError, OSError, json.JSONDecodeError):
            return "offline"

    async def fetch_host_mac(self) -> dict:
        # Called from the "Detect MAC from host" button in
        # ApolloConfigSection.tsx - requires the host to already be
        # reachable (which is why it lives in the config screen, not in
        # Quick Access), persists the result so wake_host can use it later
        # once the host is off and can no longer be asked directly.
        config = await self.get_config()
        host = config.get("host")
        if not host:
            return {"ok": False, "error": "Configure the Apollo host first (Apollo Config tab)"}

        try:
            client = RunnerClient(host, config.get("runner_port", RUNNER_PORT))
            result = client.get_mac()
            mac = result.get("mac")
            if not mac:
                return {"ok": False, "error": "The Runner could not detect a MAC address on the host"}
            config["mac_address"] = mac
            await self.save_config(config)
            return {"ok": True, "mac": mac}
        except (urllib.error.URLError, OSError, json.JSONDecodeError) as e:
            decky.logger.error(f"Failed to fetch the host MAC from the Runner: {e}")
            return {"ok": False, "error": f"Could not talk to the MoonProfile Runner: {e}"}

    async def shutdown_host(self) -> dict:
        # "Turn off host" button in QuickAccessContent.tsx, gated behind a
        # ConfirmModal on the frontend (destructive and hard to reverse
        # without Wake-on-LAN already configured).
        config = await self.get_config()
        host = config.get("host")
        if not host:
            return {"ok": False, "error": "Configure the Apollo host first (Apollo Config tab)"}

        try:
            client = RunnerClient(host, config.get("runner_port", RUNNER_PORT))
            return client.shutdown()
        except (urllib.error.URLError, OSError, json.JSONDecodeError) as e:
            decky.logger.error(f"Failed to shut down the host via the Runner: {e}")
            return {"ok": False, "error": f"Could not talk to the MoonProfile Runner: {e}"}

    async def wake_host(self) -> dict:
        # "Wake host" button in QuickAccessContent.tsx. Only works if the
        # host's NIC/BIOS actually has Wake-on-LAN enabled, which is
        # outside this code's control - out of scope to verify from here
        # (see docs/prd.md Phase 6).
        config = await self.get_config()
        mac = config.get("mac_address")
        if not mac:
            return {
                "ok": False,
                "error": "No MAC address saved yet - detect it from the Apollo Config tab while the host is on",
            }

        try:
            packet = build_magic_packet(mac)
        except ValueError as e:
            return {"ok": False, "error": str(e)}

        try:
            with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
                sock.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
                sock.sendto(packet, ("255.255.255.255", 9))
            return {"ok": True}
        except OSError as e:
            decky.logger.error(f"Failed to send the Wake-on-LAN packet: {e}")
            return {"ok": False, "error": f"Failed to send the Wake-on-LAN packet: {e}"}

    async def _main(self):
        decky.logger.info("MoonProfile loaded")

    async def _unload(self):
        decky.logger.info("MoonProfile unloaded")

    async def _uninstall(self):
        decky.logger.info("MoonProfile uninstalled")
