# MoonProfile

Decky Loader plugin for Steam Deck that manages Moonlight streaming
profiles with automatic context detection (docked vs handheld) and
dynamic configuration of the Apollo host via REST API.

## Motivation

The current Moonlight streaming flow suffers from:

- Moonlight doesn't know the usage context (docked/handheld), triggers
  wrong resolutions (e.g. 800p instead of 4K when docked)
- Apollo's fixed prep-cmd doesn't adapt to different scenarios (HDR TV
  vs SDR handheld)
- MoonDeck solves part of the problem, but requires an extra daemon on
  the host (Buddy) and has no contextual profiles
- Manually configuring every session (bitrate, codec, HDR, target
  output) doesn't scale

The plugin centralizes the decisions that today are scattered across
Moonlight, Apollo, KDE, Steam, and the user.

## Difference from MoonDeck

- ~~Zero additional component on the host~~ - valid until Phase 5:
  detecting the end of a session via Apollo's `current_app` doesn't
  actually work (auto-detach enters "placebo" mode, see Phase 5), so we
  deliberately gave up that differentiator in exchange for real
  robustness (MoonProfile Runner, a Tauri/Rust daemon on the host). Still
  without MoonDeck Buddy-style certificate/TLS pairing, just a simple
  token.
- Streaming profiles editable in-place on the Deck
- Automatic context detection (docked/handheld)
- Each profile simultaneously controls Moonlight client configuration
  and host display configuration

## Stack

- **Frontend**: TypeScript, React, `@decky/ui`, `@decky/api`
- **Backend**: Python 3.11+ (embedded in Decky Loader)
- **Bundler**: Rollup
- **Host requirements**: Apollo 0.4.8+, KDE Plasma 6 Wayland, AMD RDNA 4
  GPU or equivalent (via VAAPI)
- **Client**: Moonlight Flatpak (`com.moonlight_stream.Moonlight`)

## Architecture

```
[Deck: Steam library]
    ↓
[Quick Access or button on the game screen]
    ↓
[Plugin's Python backend]
    ├─→ Detects context (docked/handheld) via /sys/class/drm
    ├─→ Selects the matching profile
    ├─→ POST to Apollo's API: updates the "SteamGame" app with prep-cmd + cmd
    └─→ subprocess: Moonlight CLI with the profile's args
         ↓
[Apollo runs the prep-cmd DO with the profile's args]
    ├─→ Activates the target output (HDMI-A-1, DP-3, etc)
    ├─→ Sets resolution and refresh rate
    ├─→ Enables HDR and WCG if applicable
    ├─→ Disables other outputs
    └─→ Runs steam://rungameid/APPID
         ↓
    [Stream running]
         ↓
[On closing Moonlight or losing connection]
    ↓
[Apollo runs the prep-cmd UNDO]
    ├─→ pkill the game process by AppID
    ├─→ Restores original outputs
    └─→ Disables the virtual output
```

## Data model

### Profile

```typescript
interface Profile {
    id: string;                    // e.g. "docked-tv-4k-hdr"
    name: string;                  // e.g. "Docked TV 4K HDR"
    trigger: "docked" | "handheld" | "manual";
    moonlight: MoonlightConfig;
    host: HostConfig;
}

interface MoonlightConfig {
    resolution: string;            // e.g. "3840x2160"
    fps: number;                   // e.g. 60
    bitrate: number;                // in kbps, e.g. 150000
    codec: "HEVC" | "AV1" | "H264";
    hdr: boolean;
}

interface HostConfig {
    target_output: string;         // e.g. "HDMI-A-1"
    resolution: string;            // e.g. "3840x2160"
    fps: number;                   // e.g. 60
    hdr: boolean;
    wcg: boolean;                  // Wide Color Gamut
    disable_outputs: string[];     // e.g. ["DP-3"]
}
```

### Global config

```typescript
interface Config {
    host: string;                  // e.g. "192.168.1.6"
    username: string;              // Apollo admin credential
    password: string;              // Apollo admin credential
}
```

Persistence:
- `$DECKY_PLUGIN_SETTINGS_DIR/profiles.json`
- `$DECKY_PLUGIN_SETTINGS_DIR/config.json` (0600 permissions)

## Repository structure

```
moonprofile/
├── plugin.json                   # Decky metadata
├── package.json                  # frontend deps
├── rollup.config.js              # bundler
├── tsconfig.json
├── main.py                       # Python backend
├── src/
│   ├── index.tsx                 # entry point + patch registration
│   ├── types.ts                  # shared interfaces
│   ├── api.ts                    # callable() bindings to the backend
│   ├── QuickAccessContent.tsx    # main UI
│   ├── ProfileEditor.tsx         # profile CRUD editor
│   └── ConfigEditor.tsx          # global config (host, credentials)
├── defaults/                     # first-run default files
│   └── profiles.json             # example profiles
└── PROJECT.md                    # this file
```

## Execution phases

### Phase 0: CLI proof of concept (target: 1h)

Validates the architecture without writing a plugin.

Goals:
- Via curl, update the "SteamGame" app on Apollo with a custom prep-cmd
- Via the Moonlight CLI, connect to the updated app
- Confirm that HDR, resolution, and dynamic AppID work end to end

Deliverable: reference bash script that reproduces the complete flow.

Success criteria: can launch RE4 with HDR on via command line, switch to
a different AppID without restarting Apollo.

### Phase 1: Python backend + minimal Quick Access (target: 3h)

Functional plugin with config and one hardcoded profile.

Goals:
- Clone of the Decky template
- Complete `main.py` with methods: `get_config`, `save_config`,
  `get_profiles`, `save_profiles`, `detect_context`, `stream_game`
- Quick Access UI with: editable global config + profile list + "Stream
  currently selected game" button
- Get the AppID of the focused game via
  `SteamClient.Router.MainRunningApp` or similar
- Hardcoded profiles in `defaults/profiles.json`

Deliverable: installable plugin on the Deck that replaces MoonDeck in
the docked/handheld flow.

Success criteria: select a game in the library, open Quick Access, click
"Stream", context correctly detected, game launches on the host with the
profile applied.

### Phase 2: Profile UI (target: 3h)

CRUD profile editor inside Quick Access.

Goals:
- Create, edit, duplicate, delete profiles
- All fields editable via TextField, DropdownItem, SliderField,
  ToggleField
- Basic validation (unique name, resolution in the correct format)
- Visual feedback (toaster.toast) on every operation

Deliverable: complete profile management without manually editing JSON.

Success criteria: create a new profile from scratch, save it, apply it
in a stream, without touching a file.

### Phase 3: Button on the game screen (target: 2-6h, unpredictable)

Injection via a React patch on the game details page.

Goals:
- `routerHook.addPatch("/library/app/:appid", ...)`
- `afterPatch` and `findInReactTree` to locate the actions container
- Injects a `StreamButton` that calls `streamGame(appId, gameName)`
- Dropdown to manually choose a profile (optional)

Deliverable: "Stream via Moonlight" button appears on each game's
screen, next to the standard buttons.

Success criteria: click the button directly without going through Quick
Access, stream starts.

Risk: the most fragile part, breaks between Steam client versions.
Studying MoonDeck's current code is mandatory.

### Phase 4: Polish

Goals (no particular order, pick based on real usage):
- ~~Persistent notifications during an active stream / end-of-session
  detection~~ - moved to Phase 5 (needs the host daemon, see below). The
  original idea of polling Apollo's `current_app` **doesn't work**,
  reason documented in Phase 5.
- ✅ Error handling (host offline, wrong credentials, Apollo not
  responding) - `main.py:_apollo_error_response`, distinguishes the 3
  cases (confirmed 401 = wrong credential by reading Apollo's
  `confighttp.cpp`).
- ✅ Custom icon in the Decky menu (`FaSatelliteDish`, already done)
- ✅ Internal logs accessible from the UI - "Logs" tab in the Settings
  sidenav, reads `decky.DECKY_PLUGIN_LOG` on demand.
- ❌ Dropped: detecting OLED vs LCD Deck models, no concrete use case
  that justifies it (would only change FPS/HDR defaults in the handheld
  profile; the user already configures this manually without issue).
- ❌ Dropped for now: multi-host support, the user only uses one Apollo
  host today, no real need for it. Reconsider if that changes.

Phase 4 closed with what made sense to implement now.

### Phase 5: MoonProfile Runner (host daemon, Tauri/Rust)

Deliberate architecture change, gives up the "zero additional component"
differentiator (see Motivation/Difference at the top of this document)
in exchange for real robustness. Since it isn't a Decky plugin, it has
none of the Decky Plugin Store's restrictions (including the "most of
the code can't have been written by AI" one, a mandatory checkbox on the
`decky-plugin-database` PR template), so the stack is free, chosen
without that constraint: **Tauri v2 (Rust)**, with a tray icon + on-demand
window.

Single phase (absorbed the old "Phase 4.5", these were separate items
only because they weren't technically dependent on each other, but
they're part of the same effort to mature the host/game-management side
of the project).

**Why the daemon became necessary** (a technical finding, not repeating
the investigation): we tried solving end-of-session detection via
*polling* `GET /api/apps` (the `current_app` field), the solution the
original Phase 4 anticipated. It doesn't work. Reading Apollo's code
(`ClassicOldSong/Apollo`, `src/process.cpp`, function `proc_t::running()`):

```cpp
} else if (_app.auto_detach && std::chrono::steady_clock::now() - _app_launch_time < 5s) {
  // "App exited within 5 seconds of launch. Treating the app as a detached command."
  placebo = true;
  return _app_id;  // "running" forever from here on
}
```

Our `stream_game()` uses `"auto-detach": true` precisely because
`cmd: "steam steam://rungameid/{app_id}"` returns almost immediately
(it's just a relay to the Steam client, the actual game runs
separately, detached). That's exactly the `placebo = true` trigger: once
in that mode, `running()` **never goes back to zero on its own**, so
`current_app` stays stuck "running" until someone calls `close_app`
manually (our "Close connection"). There's no polling workaround for
this, the data we'd be reading simply doesn't reflect reality.

**First slice - ✅ implemented and validated on-device (end-of-session detection):**
- `moon_profile_runner/` (a complete Tauri v2 project, sibling monorepo
  to `moon_profile_decky/`): tray icon (`TrayIconBuilder`) + on-demand
  window (`tauri.conf.json` with `windows: []`, window created on tray
  click).
- Embedded HTTP server (`axum`, on its own thread + `tokio` runtime,
  separate from Tauri's event loop) on port `47991`. No authentication,
  server open on the local network (deliberate decision: on an
  already-trusted home LAN, the friction of a token isn't worth the
  security gain).
- **Later architecture change, bigger than the original detection
  feature:** Apollo stopped having any prep-cmd at all (neither "do" nor
  "undo"), the Runner (Rust) took over 100% control of the host display
  (`kscreen-doctor`) and the session lifecycle, both at launch
  (`POST /session/register`, runs the display-on commands synchronously
  before responding) and at close (`POST /session/close` manual, or
  autonomous via a background watchdog that detects on its own when the
  game closed, via `sysinfo`). See `session.rs`/`apollo.rs`/`displays.rs`
  and the "Runner controls everything that touches the host" rule in
  `AGENTS.md`. This made the Runner **mandatory** (no longer optional)
  and eliminated the client-side polling that used to exist in
  `stream.ts` (file removed, see Stage C).
- Automated tests (`cargo test`), already caught real bugs running
  against the real OS/kscreen-doctor: `refresh_processes()` without
  `cmd()` populated, a substring match colliding with a shared numeric
  prefix, and an initialization race (watchdog closing a game that was
  still just loading, fixed with the `confirmed_running` field).
- New "Runner" tab in the Settings sidenav (`RunnerConfigSection.tsx`),
  only the port is configurable, the host is the same one from the
  "Apollo config" tab (Runner and Apollo always run on the same
  machine).
- Autostart via a systemd `--user` unit
  (`packaging/moon-profile-runner.service`, `WantedBy=graphical-session.target`),
  installed by `moon_profile_runner/install.sh` or the AUR package.
  Verified on a real KDE Plasma 6 Wayland session that
  `graphical-session.target` correctly imports `WAYLAND_DISPLAY` and
  `DBUS_SESSION_BUS_ADDRESS`, so the tray/GUI shows up fine, plus gets
  restart-on-failure and `journalctl --user` logs for free. Also a
  regular applications menu entry (`packaging/moon-profile-runner.desktop`,
  no longer tied to autostart). Packaged for the AUR (`packaging/PKGBUILD`,
  `-git` package). Pacman/yay run as root, so they can't safely enable a
  systemd `--user` unit for the logged-in user (wrong session, wrong
  D-Bus, wrong `XDG_RUNTIME_DIR`) - `autostart.rs` has the app enable
  itself the first time it's launched by hand instead (idempotent, and
  it's running inside the right session by then), the `.install`
  scriptlet's message is just a manual fallback.

### Per-game shortcuts generated from the Runner (replaced the game-screen button)

Instead of a button injected via a React patch (fragile, only worked for
games that were already real Steam catalog entries), the Runner reads
the games on the host and the Deck creates a visible shortcut per game
(with cover/hero art), the user clicks "Play" natively, no button
needed.

**Important finding from planning:** the shortcut, once it becomes a
normal library item, can be clicked without any of our JS running
beforehand, that's why `runner.py` stopped being a dumb launcher and now
self-configures (reads config/profiles from disk, detects context, talks
to Apollo) before exec'ing Moonlight.

**Stage A - ✅ implemented and validated on-device (real Steam games):**
- `py_modules/moonprofile_core.py`: `ApolloClient`, `detect_context`,
  `build_display_commands`/`build_restore_commands`, `classify_apollo_error`,
  extracted from `main.py` so it can also be imported by `runner.py`
  (which runs outside Decky Loader, without `py_modules` automatically
  on `sys.path`, `runner.py` inserts it manually).
- `moon_profile_runner/src-tauri/src/games.rs`: parsing of
  `libraryfolders.vdf` + `appmanifest_*.acf`, `GET /games` endpoint.
  Filters out Valve tools by name AND real software (Aseprite, Blender,
  etc) via the Steam public API's gameplay categories (finding: Steam's
  `type` field doesn't distinguish game from tool, `categories` does).
- `gameShortcuts.ts`: **visible** shortcut (not hidden), fixed launch
  options (`MOONPROFILE_HOST_APP_ID=<id>`) set once at creation.
- `gameArtwork.ts`: `SteamClient.Apps.SetCustomArtworkForApp` with
  cover/hero art from the official Steam CDN (only for real Steam
  AppIDs).
- `gameSync.ts` + a "Sync games from host" button in Quick Access, with
  a real progress bar (game by game), manual sync for now.
- `gameCollection.ts`: groups the synced shortcuts into a "Streaming"
  collection (`window.collectionStore`, persisted id to survive manual
  renaming).

**Stage B - ✅ implemented: non-Steam games.**
- `moon_profile_runner/src-tauri/src/games.rs`: parses each Steam user
  profile's `userdata/<user_id>/config/shortcuts.vdf` (Valve's binary VDF
  format, via the `steam-vdf-parser` crate, which handles both the text
  and binary variants), skips hidden shortcuts (`IsHidden`). The stored
  `appid` is a signed 32-bit int, but Steam treats it as unsigned
  everywhere it's actually used (the `steam://rungameid/<id>` URL, the
  `compatdata/<id>` folder name) - confirmed against a real file on a
  real machine, casting to `u32` before formatting it as `host_app_id` is
  required. Merged into `list_host_games()` after the Steam-only
  category filter (non-Steam appids aren't real Store entries, querying
  them there would just 404).
- New `steamgriddb_api_key` config field, its own panel in
  `GamesGridSection.tsx` (rather than `ApolloConfigSection.tsx`, since
  it's specifically about non-Steam artwork, not Apollo credentials) -
  free key from https://www.steamgriddb.com/profile/preferences/api.
  **Superseded** (see below): removed in favor of a build-time constant.
- `gameArtwork.ts`: `applySteamGridDbArtwork` (search by name, then
  grids/heroes/logos/icons endpoints) alongside the existing
  `applySteamCdnArtwork` for real Steam games - both now cover all 5
  `SetCustomArtworkForApp` asset types (vertical grid, wide grid, hero,
  logo, icon), except the CDN path's icon (no reliable fixed URL for
  it without an extra API call, left unset for real Steam games).
- Every artwork/sync step is best-effort and never throws - failures
  (missing API key, no SteamGridDB match, network error,
  `SetCustomArtworkForApp` itself rejecting) are logged via a new
  `log_frontend_error` bridge (`main.py`, since frontend `console.error`
  never reaches `DECKY_PLUGIN_LOG`) instead of stopping the sync loop
  over the remaining games.

**Later change: SteamGridDB key moved from per-install config to a
build-time constant.** There is exactly one SteamGridDB account behind
this plugin (the maintainer's own) - a per-install Settings field made
sense for something like Apollo credentials (different per host) but
not for this, so it was removed: `config.steamgriddb_api_key` (and its
TextField in `GamesGridSection.tsx`) are gone, replaced by
`src/env.ts`'s `STEAMGRIDDB_API_KEY`, inlined into `dist/index.js` at
build time by a `replace()` rollup plugin (`rollup.config.js`) that
reads a local `.env` (gitignored - this repo is public on GitHub, see
`.env.example`). `gameSync.ts`/`GamesGridSection.tsx` no longer need
`getConfig()`/`Config` at all for this.

**Stage C - ✅ done: old button removed.** Deleted
`LibraryAppPatch.tsx`, `GameActionButton.tsx`, `stream.ts`,
`steamShortcut.ts`, `ButtonPositionSection.tsx` (the "Button position"
tab, only made sense for the button that no longer exists), and the
`patchLibraryApp()` registration in `index.tsx`. `main.py:stream_game()`
and the `button_position`/`ButtonPosition` field/type (config and type)
also went away, orphaned after the removal. `main.py:stop_stream()`
still exists (used by Quick Access's "Close connection", which talks to
the Runner, not the same mechanism as the old button).

**Out of scope (not yet decided if it's worth it):**
- List of connected clients / connection stability status in the
  Runner's window.
- Host readiness check before starting a stream (GPU/encoder, active
  Plasma session).
- Certificate/TLS pairing, if it's ever genuinely needed (what MoonDeck
  Buddy does, much more complex, a conscious decision not to do this
  now).

Explicit decision (recorded so as not to repeat the discussion later):
**not forking MoonDeck or Buddy.** Their architecture assumes the two
things this project existed to avoid (an extra daemon on the host,
absence of contextual profiles, see Motivation), except Phase 5 already
gave up the first item, deliberately. Even so, forking would inherit an
unfamiliar C++/Qt architecture and non-contextual profiles, more work,
not less. The strategy remains: read their code as a point-in-time
reference (as already done for the game-screen button, the `gameid`
fix, and the Tauri tray/menu API), implement directly on the
already-validated stack.

### Phase 6: Host power control (shutdown, Wake-on-LAN, status)

Quick Access only closed the connection and synced games so far - nothing
told the user whether the host was even reachable, and there was no way
to turn it off or back on from the Deck. This phase adds a status
indicator plus power buttons to `QuickAccessContent.tsx`, split across the
same three layers as everything else host-related (see "The Runner
controls everything that touches the host" in `AGENTS.md`):

- **Runner (Rust)**, new `src-tauri/src/power.rs`: `detect_primary_mac`
  reads `/sys/class/net/*/address` + `.../operstate` (parameterized by
  path, same testability pattern as `detect_context(drm_path=...)` in
  `moonprofile_core.py`), skipping `lo` and virtual interfaces
  (`docker`/`veth`/`virbr`/`br-`/`wg`/`tun`) and preferring one that's
  actually up. `shutdown_command()` is a pure function returning
  `("systemctl", ["poweroff"])`, kept separate from execution so it has an
  actual unit test. `server.rs` gained three routes: `GET /health` (cheap
  liveness probe, deliberately not reusing `/games` which fires a sync
  notification and touches kscreen-doctor indirectly), `GET /system/mac`,
  and `POST /system/shutdown` (responds `{"ok": true}` first, then, after
  a short delay so the HTTP response actually makes it out over the
  network, runs `shutdown_command()` in the background - same
  respond-before-side-effect ordering already used by
  `close_session_now`/the watchdog in `session.rs`).
- **Decky backend (Python)**: `moonprofile_core.py` gained
  `build_magic_packet(mac)` (parses `:`- or `-`-separated MAC, raises
  `ValueError` on bad input, returns the 6xFF + 16x-MAC magic packet).
  `main.py`'s `RunnerClient` gained `health()` (short 2s timeout,
  deliberately shorter than the other methods' 10-15s so polling from
  Quick Access doesn't hang waiting on a host that's already off),
  `get_mac()`, and `shutdown()`. New `Plugin` methods: `get_host_status()`
  (`"unconfigured"` / `"online"` / `"offline"`), `fetch_host_mac()` (asks
  the Runner, persists into `config["mac_address"]`), `shutdown_host()`,
  and `wake_host()` (builds the magic packet from the saved MAC and sends
  it via a broadcast UDP socket to port 9 - only works if the host's
  NIC/BIOS actually has Wake-on-LAN enabled, which is outside this code's
  control).
- **Decky frontend (TS)**: `QuickAccessContent.tsx` polls
  `getHostStatus()` on an interval and shows the translated status, plus a
  "Turn off host" button (gated behind `ConfirmModal` from `@decky/ui`,
  since this is destructive and hard to reverse without WoL already
  configured, only enabled when online) and a "Wake host" button (only
  enabled when offline). `ApolloConfigSection.tsx` gained a read-only MAC
  field and a "Detect MAC from host" button (calls `fetch_host_mac`,
  requires the host to already be reachable, which is why it lives in the
  config screen and not in Quick Access).

**Known, accepted test gap:** there is no integration test that actually
invokes `POST /system/shutdown` end-to-end, nor one that exercises the
real `GET /system/mac` handler - the first would genuinely power off
whatever machine runs `cargo test`, the second depends on the real
`/sys/class/net` of that machine (not deterministic in CI). Both are
covered only by unit tests of the pure functions underneath
(`shutdown_command`, `detect_primary_mac` against fixtures), documented
inline where the gap is and in `AGENTS.md`.

**Out of scope for this phase:** verifying, from this codebase, that
`systemctl poweroff` is actually passwordless for the active session
(polkit `allow_active`) and that Wake-on-LAN is enabled in the host's
BIOS/NIC - both need to be confirmed by hand on the real machine.

## Technical references

### Apollo API (inherited from Sunshine)

Endpoint: `https://HOST:47990/api/apps`

Authentication: Basic auth (admin/password configured on Apollo).

Self-signed certificate, client needs to disable SSL verification.

Non-browser clients are exempt from CSRF (confirmed in the official
docs).

Methods used:
- `GET /api/apps` → lists current apps
- `POST /api/apps` → creates or updates (use `index: -1` to create, or
  an existing index to update)

POST body:

```json
{
  "name": "SteamGame",
  "cmd": "steam steam://rungameid/2050650",
  "index": -1,
  "auto-detach": true,
  "wait-all": false,
  "exit-timeout": 5,
  "exclude-global-prep-cmd": false,
  "elevated": false,
  "prep-cmd": [{
    "do": "bash -c '...inline command...'",
    "undo": "bash -c '...inline command...'"
  }],
  "output": "/tmp/apollo-steamgame-2050650.log"
}
```

Known limitation: the `env` field is only editable via the `apps.json`
file directly, not via the API. That's why we pass everything through
inline `prep-cmd`.

### Undo command with a clean game kill

```bash
# generated dynamically by the plugin, embedding the known AppID
pkill -TERM -f "AppId=2050650" ; sleep 5 ; pkill -KILL -f "AppId=2050650" 2>/dev/null ; setsid steam steam://close/bigpicture ; sleep 2 ; kscreen-doctor output.DP-3.enable ; sleep 1 ; kscreen-doctor output.HDMI-A-1.disable
```

Using `;` instead of `&&` is intentional: if pkill returns an error (the
game already closed), the chain continues and restores the displays.

### Context detection

```python
def detect_context() -> str:
    """Returns 'docked' if any external display is connected, otherwise 'handheld'."""
    drm_path = "/sys/class/drm"
    for entry in os.listdir(drm_path):
        if not entry.startswith("card"):
            continue
        if not ("HDMI" in entry or "DP" in entry):
            continue
        status_file = os.path.join(drm_path, entry, "status")
        if os.path.exists(status_file):
            with open(status_file) as f:
                if f.read().strip() == "connected":
                    return "docked"
    return "handheld"
```

### Steam Browser Protocol

Existing and used:
- `steam://rungameid/<appid>` → launches the game
- `steam://open/bigpicture` → opens Big Picture
- `steam://close/bigpicture` → closes Big Picture

Does NOT exist:
- `steam://exit/<appid>` → not a valid URL scheme, why we use `pkill`
  instead

## Development workflow

### Initial setup

```bash
git clone https://github.com/SteamDeckHomebrew/decky-plugin-template moonprofile
cd moonprofile
rm -rf .git && git init
pnpm install
```

Edit `plugin.json` with the name, author, description.

### Build

```bash
pnpm build
```

Generates `dist/index.js`, which Decky Loader loads.

### Deploy to the Deck

rsync method:
```bash
rsync -avz --delete \
    ./ deck@STEAMDECK_IP:/home/deck/homebrew/plugins/moonprofile/ \
    --exclude node_modules --exclude .git

ssh deck@STEAMDECK_IP "systemctl --user restart plugin_loader"
```

VS Code method: Remote-SSH directly into the Deck, edit in place, reload
via the Decky UI.

### Logs

On the Deck:
```bash
journalctl --user -f | grep -i decky
```

Plugin-specific logs:
```bash
tail -f /home/deck/homebrew/logs/moonprofile/plugin.log
```

Frontend logs go to the Steam WebHelper devtools (enable via Decky
Settings → Developer Options).

## Known risks and limitations

1. **The library patch is fragile**: Gaming Mode's React class names
   change between Steam client versions. Requires ongoing maintenance.
   Mitigation: start without the patch (Quick Access only), add it later
   if truly needed.

2. **String escaping in prep-cmd**: if a path or profile name has single
   quotes, it breaks. Mitigation: sanitize inputs in the editor.

3. **No save sync beyond Steam Cloud**: acceptable for the personal
   flow.

4. **No automatic session resume**: if the connection drops, reopen
   manually.

5. **`sleep 5` in the undo might not be enough for games with rare
   autosaves**: accept the loss or increase it. Configurable per profile
   in Phase 4.

6. **The `docked` trigger alone doesn't distinguish good vs bad
   network**: if you play docked at home and docked at a friend's place,
   you need to select the profile manually. Expanding to a composite
   trigger (docked + SSID) is possible in Phase 4.

7. **Matching by `AppId=` in pkill is fragile if two games run
   simultaneously**: rare scenario.

## External resources

- Sunshine/Apollo API: https://docs.lizardbyte.dev/projects/sunshine/latest/md_docs_2api.html
- MoonDeck (case study): https://github.com/FrogTheFrog/moondeck
- Decky Loader wiki: https://wiki.deckbrew.xyz/en/plugin-dev/getting-started
- Decky plugin template: https://github.com/SteamDeckHomebrew/decky-plugin-template
- HLTB plugin (simple patch reference): https://github.com/OMGDuke/HLTB-For-Deck

## Scope restrictions (important)

**Hard stop at Phase 1.** Two weeks of real use before deciding on Phase
2 or 3.

Reasons:
- Historical pattern of accumulating partial projects
- Ares deadline in August has priority over this project
- Ongoing Oraculo rewrite can't be slowed down
- Phase 1 already solves the personal problem (docked/handheld with
  profiles)
- Phases 2 and 3 are polish, not essential features

If after two weeks of real use there's genuine pain (not abstract
desire) for profile CRUD or the game-screen button, then invest more
time. Before that, it's a sign of over-engineering or disguised
procrastination.
