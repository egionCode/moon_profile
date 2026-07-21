"""
Logic shared between main.py (runs inside the Decky Loader) and runner.py
(runs as a standalone process, exec'd by a Steam shortcut, see docs/prd.md,
per-game shortcuts section). Both need to talk to Apollo and detect
context the same way, so it lives here instead of being duplicated in
both places.

runner.py does NOT run inside the Decky Loader environment: it imports
this module by manually inserting the py_modules/ path into sys.path
(see runner.py), not through the loader's normal mechanism.
"""

import json
import ssl
import http.cookiejar
import urllib.request
import urllib.error

APOLLO_PORT = 47990
RUNNER_PORT = 47991

# Mapping for the moonlight CLI's --video-codec flag: our data model uses
# "H264" (no dot) but the CLI expects a literal "H.264".
CODEC_FLAGS = {"HEVC": "HEVC", "AV1": "AV1", "H264": "H.264"}


def detect_context(drm_path: str = "/sys/class/drm") -> str:
    """Returns 'docked' if any external display is connected, otherwise 'handheld'.

    drm_path is configurable to allow testing against a fixture (a fake
    folder with the same structure as /sys/class/drm) instead of depending
    on the real hardware of the machine running the test.
    """
    import os

    for entry in os.listdir(drm_path):
        if not entry.startswith("card"):
            continue
        # eDP = the Deck's own internal display, always present and always
        # "connected", and "eDP-1" contains "DP" as a substring, so without
        # this exclusion the function ALWAYS returned "docked", docked or
        # not (real bug, found while testing on the actual device).
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
    Commands to TURN ON/CONFIGURE the host display (kscreen-doctor: enables
    the target_output, sets mode/resolution, HDR, disables the other
    outputs), in simple EXECUTION order (no longer an Apollo array, it's a
    list of strings that the Runner (Rust) runs directly via shell, BEFORE
    exec'ing into Moonlight, see session.rs/register_session).

    Apollo no longer has any prep-cmd at all (neither do nor undo): an
    explicit decision to take this responsibility away from it (more
    "plug and play": Apollo only needs to know how to connect and run the
    "cmd", the one who actually controls the display and the session
    lifecycle is the Runner, which already needs to know how to turn
    displays on/off for closing anyway, see build_restore_commands). The
    Runner stopped being optional because of this: without it, the display
    switch simply doesn't happen.
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

    if host_cfg.get("enter_bigpicture"):
        # Enters Big Picture on the host at launch: useful for anyone using
        # the host itself as an HTPC/TV who wants the Big Picture UI
        # instead of the desktop showing up behind the stream. Symmetric
        # with the (now conditional) "close Big Picture" in build_restore_commands.
        commands.append("setsid steam steam://open/bigpicture")

    if host_cfg.get("move_cursor_to_corner"):
        # Some games (real finding: FIFA) lock the cursor in the middle of
        # the screen even while playing with a controller only: send it to
        # the bottom-right corner of the target output (ydotool, the only
        # way to move the cursor on Wayland without compositor support,
        # KWin won't let you write workspace.cursorPos on this Plasma
        # version, confirmed by running it for real). Runs through the
        # Runner (Rust) just like the rest of display_commands, "Rust
        # controls everything that touches the host", see AGENTS.md.
        width, height = resolution.split("x")
        commands.append(f"ydotool mousemove -a {int(width) - 1} {int(height) - 1}")

    return commands


def build_restore_commands(host_cfg: dict) -> list:
    """
    Commands to RESTORE the host display (closes Big Picture, re-enables
    the disabled outputs, turns off the target), in EXECUTION order. Used
    by the Runner both on AUTONOMOUS closing (watchdog detects the game
    closed on its own) and on MANUAL closing ("Close connection"), after
    making sure (session.rs) the game process has really ended already,
    see kill_game_process on the Rust side, which takes care of killing
    the game BEFORE running this when closing is manual (it could still
    genuinely be alive).
    """
    target = host_cfg["target_output"]
    disable_outputs = host_cfg.get("disable_outputs", [])

    commands = []
    if host_cfg.get("enter_bigpicture"):
        # Only closes Big Picture if the profile actually opened it at
        # launch (build_display_commands): always FIRST on closing, before
        # touching any kscreen-doctor (leave Big Picture before changing
        # the resolution, not after).
        commands.append("setsid steam steam://close/bigpicture")
        commands.append("sleep 2")
    commands.extend(f"kscreen-doctor output.{o}.enable" for o in disable_outputs)
    commands.append("sleep 1")
    if disable_outputs:
        commands.append(f"kscreen-doctor output.{target}.disable")
    return commands


def build_magic_packet(mac: str) -> bytes:
    """
    Builds a standard Wake-on-LAN magic packet for the given MAC address:
    6 bytes of 0xFF followed by the target MAC repeated 16 times, sent as
    a broadcast UDP datagram (main.py:wake_host). Accepts both ':'- and
    '-'-separated MAC notation (depends on where the address was copied
    from - the Runner's /system/mac reports ':'-separated).
    """
    separator = ":" if ":" in mac else "-"
    parts = mac.split(separator)
    if len(parts) != 6:
        raise ValueError(f"'{mac}' is not a valid MAC address")
    try:
        mac_bytes = bytes(int(part, 16) for part in parts)
    except ValueError:
        raise ValueError(f"'{mac}' is not a valid MAC address")
    return b"\xff" * 6 + mac_bytes * 16


class ApolloClient:
    """
    Minimal client (stdlib only, no 'requests') for the Apollo REST API.

    This fork (ClassicOldSong/Apollo) does NOT use HTTP Basic Auth, despite
    what docs/api.md says: authenticate() in confighttp.cpp only checks an
    "auth" session cookie, obtained via POST /api/login. Validated in
    Phase 0 against the real Apollo.
    """

    def __init__(self, host: str, username: str, password: str, port: int = APOLLO_PORT):
        self.base_url = f"https://{host}:{port}"
        self.username = username
        self.password = password

        ctx = ssl.create_default_context()
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE  # Apollo's self-signed certificate
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
        # Apps are identified by uuid, not array index: the "index" field
        # from the old Sunshine doc example is discarded by the server
        # (migrate_apps() in process.cpp).
        apps = self._request("GET", "/api/apps").get("apps", [])
        for app in apps:
            if app.get("name") == name:
                return app.get("uuid", "")
        return ""

    def save_app(self, payload: dict) -> dict:
        return self._request("POST", "/api/apps", payload)


def classify_apollo_error(host: str, error: Exception) -> str:
    """
    Translates a network/HTTP exception from talking to Apollo into a clear
    message, instead of the raw Python exception text.

    Wrong credentials: confirmed in the real Apollo code
    (confighttp.cpp:login()) that it responds with 401 in that case (not a
    generic 403/400), a reliable signal to tell apart from "host offline".
    """
    if isinstance(error, urllib.error.HTTPError):
        if error.code == 401:
            return "Wrong Apollo username or password"
        return f"Apollo responded with an unexpected error (HTTP {error.code})"
    if isinstance(error, json.JSONDecodeError):
        return f"Apollo responded with something that isn't JSON, check whether {host}:{APOLLO_PORT} is really Apollo"
    return f"Could not reach Apollo at {host}:{APOLLO_PORT}, check whether the host is on and on the same network"
