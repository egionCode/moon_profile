#!/usr/bin/env python3
"""
Runner that the Steam shortcuts (one per game, see src/gameShortcuts.ts) execute.

Why it exists: Gamescope (the Game Mode compositor) only focuses/shows
windows launched through Steam's real mechanism. A standalone subprocess
(what Phase 1 did) opens in fullscreen but stays "hidden" behind the UI,
with no focus at all (confirmed by running it on the device). The
solution (same as MoonDeck does) is to register this script as a
non-Steam shortcut; Steam then actually executes it (Gamescope treats it
as a game, focuses it normally).

IMPORTANT CHANGE (per-game shortcuts, visible in the library): previously,
this script only exec'd into Moonlight; whoever configured Apollo (login,
prep-cmd, cmd) was always the plugin's JS, called BEFORE launch via
stream_game(). That worked because it was always OUR button that
triggered the click. Now that the shortcuts are normal library items (the
user clicks Steam's native "Play", without going through our code), our JS
NEVER runs before launch, so this script needs to self-configure: read
config/profiles from disk, detect context, talk to Apollo, and ONLY THEN
exec into Moonlight. That's why it imports moonprofile_core (the same
logic main.py uses) instead of just receiving ready-made environment
variables.

How it receives the parameter that matters: MOONPROFILE_HOST_APP_ID is set
in the shortcut's Launch Options ONCE, at creation time (see
ensureGameShortcut in src/gameShortcuts.ts): it's the game's real AppID on
the HOST's Steam. Everything else (which profile to use, Apollo config) is
resolved here, at launch time, reading the same files main.py reads.

IMPORTANT CHANGE #2 (Apollo without prep-cmd, Runner mandatory): Apollo no
longer turns the host display on/off by itself, that's now 100% the
responsibility of the MoonProfile Runner (Rust, see moon_profile_runner/),
both at launch (register_with_runner sends the display_commands, which the
Runner runs BEFORE responding) and at closing (restore_commands, autonomous
or manual). Apollo is left with only the "cmd" (connect + run the game),
simpler, "plug and play", and gives the Deck full control over the session
lifecycle. That's why the Runner stopped being optional: without it, the
display simply doesn't switch (see main(), which aborts the launch if
register_with_runner fails, the same way it already did if
configure_apollo failed).
"""
import os
import sys
import json
import urllib.error
import urllib.request

# runner.py lives at <PLUGIN_DIR>/runner/runner.py, py_modules is a sibling
# of runner/. It doesn't run via the Decky Loader (Steam executes it
# directly), so DECKY_PLUGIN_DIR/py_modules is not on sys.path by default
# the way it would be for main.py (see decky-loader's sandboxed_plugin.py),
# it needs to be inserted manually.
_PLUGIN_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(_PLUGIN_DIR, "py_modules"))

from moonprofile_core import (  # noqa: E402 (import after sys.path.insert is intentional)
    RUNNER_PORT,
    ApolloClient,
    CODEC_FLAGS,
    build_display_commands,
    build_restore_commands,
    classify_apollo_error,
    detect_context,
)

APP_NAME = "SteamGame"


def _decky_home() -> str:
    # <PLUGIN_DIR> = <DECKY_HOME>/plugins/moonprofile, the same convention
    # decky-loader itself uses for plugins/settings/data (all siblings
    # under DECKY_HOME), except here it's discovered at runtime instead of
    # coming from the loader's environment variables (which don't exist
    # for this process).
    return os.path.dirname(os.path.dirname(_PLUGIN_DIR))


def _settings_dir() -> str:
    return os.path.join(_decky_home(), "settings", "moonprofile")


def _runtime_dir() -> str:
    return os.path.join(_decky_home(), "data", "moonprofile")


def _load_json(path: str) -> dict | list:
    with open(path) as f:
        return json.load(f)


def _pick_profile(profiles: list, context: str) -> dict | None:
    return next((p for p in profiles if p.get("trigger") == context), None)


def configure_apollo(host_app_id: str) -> dict:
    """
    Replicates the part of main.py's stream_game() that talks to Apollo:
    login, save the "SteamGame" app with this game's AppID. Apollo no
    longer gets any prep-cmd at all (neither do nor undo), whoever turns
    the display on/off now is always the Runner (Rust), both at launch and
    at closing (see register_with_runner/build_display_commands). This
    keeps Apollo simpler ("plug and play", it only needs to know how to
    connect and run the cmd) and gives the Deck full control over the
    session lifecycle. Raises an exception if something fails (the caller
    decides what to do).
    """
    config = _load_json(os.path.join(_settings_dir(), "config.json"))
    profiles = _load_json(os.path.join(_settings_dir(), "profiles.json"))

    context = detect_context()
    profile = _pick_profile(profiles, context)
    if profile is None:
        raise RuntimeError(f"No profile configured for context '{context}'")

    client = ApolloClient(config["host"], config["username"], config["password"])
    client.login()
    uuid = client.find_app_uuid(APP_NAME)
    client.save_app({
        "name": APP_NAME,
        "cmd": f"steam steam://rungameid/{host_app_id}",
        "uuid": uuid,
        "auto-detach": True,
        "wait-all": False,
        "exit-timeout": 5,
        "exclude-global-prep-cmd": False,
        "elevated": False,
        "prep-cmd": [],
        "output": f"/tmp/apollo-steamgame-{host_app_id}.log",
    })

    return {"config": config, "profile": profile}


def register_with_runner(config: dict, host_app_id: str, profile: dict) -> None:
    """
    Registers the session with the MoonProfile Runner (daemon on the
    host): app_id + Apollo credentials IN MEMORY (never written to disk on
    the host, see session.rs), plus the commands to TURN ON the display
    (build_display_commands) and to RESTORE it (build_restore_commands).
    The Runner runs the display-on commands RIGHT NOW, synchronously (this
    call only returns after that), which is why it needs to happen BEFORE
    the exec into Moonlight, otherwise the stream would start before the
    display is in the right state.

    The Runner stopped being OPTIONAL because of this: since Apollo no
    longer has any prep-cmd, without the Runner the display simply
    doesn't switch, which is why this function raises instead of just
    logging and moving on (see main() below, which aborts the launch if
    this fails, the same way it already aborts if configure_apollo fails).
    """
    body = json.dumps({
        "app_id": host_app_id,
        "username": config["username"],
        "password": config["password"],
        "display_commands": build_display_commands(profile["host"]),
        "restore_commands": build_restore_commands(profile["host"]),
    }).encode()
    req = urllib.request.Request(
        f"http://{config['host']}:{config.get('runner_port', RUNNER_PORT)}/session/register",
        data=body,
        method="POST",
    )
    req.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(req, timeout=30):  # 30s: gives the display_commands time to run on the Runner
        pass
    print(f"Session registered with the Runner ({config['host']}:{config.get('runner_port', RUNNER_PORT)}) for app_id={host_app_id}", file=sys.stderr)


def main() -> None:
    host_app_id = os.environ.get("MOONPROFILE_HOST_APP_ID")
    if not host_app_id:
        print("MOONPROFILE_HOST_APP_ID not set, aborting", file=sys.stderr)
        sys.exit(1)

    log_path = os.path.join(_runtime_dir(), "moonlight.log")
    os.makedirs(_runtime_dir(), exist_ok=True)
    # Redirects stdout/stderr to the log BEFORE the exec (fds are inherited
    # across execvp, flatpak/moonlight itself writes to them directly; the
    # Apollo configuration errors below also land here).
    log_fd = os.open(log_path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o644)
    os.dup2(log_fd, 1)
    os.dup2(log_fd, 2)
    os.close(log_fd)

    try:
        result = configure_apollo(host_app_id)
    except (urllib.error.URLError, OSError, json.JSONDecodeError) as e:
        host = ""
        try:
            host = _load_json(os.path.join(_settings_dir(), "config.json")).get("host", "")
        except OSError:
            pass
        # Aborts instead of trying to stream anyway: if Apollo wasn't
        # configured correctly, the host display is likely not in the
        # right config (wrong resolution/output), streaming anyway would
        # just give a broken screen instead of a clear error.
        print(f"Failed to configure Apollo: {classify_apollo_error(host, e)}", file=sys.stderr)
        sys.exit(1)
    except RuntimeError as e:
        print(f"Failed to configure Apollo: {e}", file=sys.stderr)
        sys.exit(1)

    config = result["config"]

    try:
        register_with_runner(config, host_app_id, result["profile"])
    except (urllib.error.URLError, OSError, json.JSONDecodeError) as e:
        # The Runner is NOT optional anymore: Apollo has no prep-cmd at
        # all, so without the Runner the host display never switches to
        # the right target_output/resolution. Aborting here (instead of
        # streaming anyway) gives the same error handling configure_apollo
        # already has, clear, in the log, instead of a silently broken screen.
        print(f"Failed to register with the MoonProfile Runner (mandatory): {e}", file=sys.stderr)
        sys.exit(1)

    moonlight_cfg = result["profile"]["moonlight"]
    codec_flag = CODEC_FLAGS.get(moonlight_cfg["codec"], "auto")
    hdr_flag = "--hdr" if moonlight_cfg.get("hdr") else "--no-hdr"

    args = [
        "flatpak", "run", "com.moonlight_stream.Moonlight", "stream",
        config["host"], APP_NAME,
        "--resolution", moonlight_cfg["resolution"],
        "--fps", str(moonlight_cfg["fps"]),
        "--bitrate", str(moonlight_cfg["bitrate"]),
        "--video-codec", codec_flag,
        hdr_flag,
    ]

    # execvp REPLACES this process with flatpak (same PID): important so
    # Steam/Gamescope track the game's real process, not a Python wrapper
    # left hanging on top of it.
    os.execvp("flatpak", args)


if __name__ == "__main__":
    main()
