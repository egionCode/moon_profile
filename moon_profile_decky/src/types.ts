export interface MoonlightConfig {
  bitrate: number; // in kbps, ex: 150000
  codec: "HEVC" | "AV1" | "H264";

  // Advanced video: raw moonlight-qt CLI flags (see build_moonlight_flags
  // in moonprofile_core.py), passthrough only, no interaction with the
  // host's own display switch. All optional: undefined falls back to
  // moonlight-qt's own default (documented per field, from
  // streamingpreferences.cpp), so a profile saved before these fields
  // existed keeps streaming unchanged until someone opens the editor.
  // Plain bool in moonlight-qt, no multi-mode option - but only takes
  // effect when vsync is also true (session.cpp gates it as
  // "enableVsync && framePacing" before the decoder ever sees it).
  frame_pacing?: boolean; // default: false
  vsync?: boolean; // default: true
  display_mode?: "fullscreen" | "windowed" | "borderless"; // default: "fullscreen"
  video_decoder?: "auto" | "software" | "hardware"; // default: "auto"
  yuv444?: boolean; // default: false
  performance_overlay?: boolean; // default: false
  keep_awake?: boolean; // default: true
  game_optimization?: boolean; // default: true

  // Input
  absolute_mouse?: boolean; // default: false
  mouse_buttons_swap?: boolean; // default: false
  touchscreen_trackpad?: boolean; // default: false
  multi_controller?: boolean; // default: true
  background_gamepad?: boolean; // default: false
  reverse_scroll_direction?: boolean; // default: false
  swap_gamepad_buttons?: boolean; // default: false
  capture_system_keys?: "never" | "fullscreen" | "always"; // default: "never"

  // Audio
  audio_config?: "stereo" | "5.1-surround" | "7.1-surround"; // default: "stereo"
  audio_on_host?: boolean; // default: false
  mute_on_focus_loss?: boolean; // default: false
}

export interface HostConfig {
  target_output: string; // ex: "HDMI-A-1"
  wcg: boolean; // Wide Color Gamut
  disable_outputs: string[]; // ex: ["DP-3"]
  // Sends the cursor to the bottom-right corner of the target output on
  // launch (via ydotool, see build_display_commands in
  // moonprofile_core.py), some games (ex: FIFA) trap the cursor in the
  // middle of the host screen even while playing with a controller only.
  move_cursor_to_corner?: boolean;
  // Enters Big Picture on the host when launching, exits before changing
  // the resolution on close (see build_display_commands/
  // build_restore_commands), useful for those who use the host itself as
  // an HTPC/TV.
  enter_bigpicture?: boolean;
}

export interface Profile {
  id: string; // ex: "1" (auto-generated sequential int, as a string)
  name: string; // ex: "Docked TV 4K HDR"
  trigger: "docked" | "handheld" | "manual";
  // Single source of truth for resolution/fps/HDR - drives BOTH the
  // host's kscreen-doctor mode switch AND the Moonlight stream's
  // --resolution/--fps/--hdr, there's no legitimate reason for the two
  // sides to ever differ (used to be duplicated under moonlight/host,
  // see git history).
  resolution: string; // ex: "3840x2160"
  fps: number; // ex: 60
  hdr: boolean;
  moonlight: MoonlightConfig;
  host: HostConfig;
}

export interface Config {
  host: string; // ex: "192.168.1.6"
  username: string; // Apollo admin credential
  password: string; // Apollo admin credential
  // MoonProfile Runner (Tauri/Rust daemon on the host, Phase 5),
  // supplements the end-of-session detection that Apollo can't do on its
  // own. No authentication (server open on the LAN, explicit decision).
  // Runs on the SAME machine as Apollo, only the port changes, the host is
  // "host" above.
  runner_port: number;
  // Host's primary NIC MAC address, detected via GET /system/mac on the
  // Runner (see ApolloConfigSection.tsx's "Detect MAC from host" button)
  // and used to build the Wake-on-LAN magic packet in wake_host once the
  // host is off and can't be asked directly anymore. Empty until detected.
  mac_address: string;
}

// "unconfigured": no Apollo host set yet. "online"/"offline": whether
// GET /health on the Runner answered - polled by QuickAccessContent.tsx
// to show a status indicator and gate the shutdown/wake buttons.
export type HostStatus = "unconfigured" | "online" | "offline";

// Generic ok/error shape for actions that don't need to return anything
// else (shutdown_host, wake_host in main.py).
export interface OkResult {
  ok: boolean;
  error?: string;
}

export interface FetchHostMacResult extends OkResult {
  mac?: string;
}

export interface StreamResult {
  ok: boolean;
  profile?: string;
  context?: string;
  error?: string;
  runner_path?: string;
  launch_env?: Record<string, string>;
}

// A game listed by the MoonProfile Runner (see moon_profile_runner/src-tauri/
// src/games.rs) - either a real Steam catalog game or a non-Steam shortcut
// already added to the host's Steam library (is_steam distinguishes them,
// since only real Steam games have official CDN artwork available).
export interface HostGame {
  name: string;
  host_app_id: string;
  is_steam: boolean;
}

export interface ListGamesResult {
  ok: boolean;
  games: HostGame[];
  runner_path?: string;
  error?: string;
}

// One resolution/fps combo a display actually supports (from
// kscreen-doctor's EDID-derived "modes" list, already deduped/sorted by
// the Runner - see displays.rs's modes_from_kscreen). Drives the single
// Profile.resolution/fps in ProfileEditor.tsx - there's no reason for
// host and stream to differ, it should always match what the host
// output is actually being switched to.
export interface HostDisplayMode {
  resolution: string; // ex: "3840x2160"
  fps: number;
}

// A host display/output (via kscreen-doctor -j, see
// moon_profile_runner/src-tauri/src/displays.rs), feeds the "Monitor"/
// "Resolution" selects and the list of outputs to disable in
// ProfileEditor.tsx.
export interface HostDisplay {
  name: string; // ex: "HDMI-A-1"
  connected: boolean;
  enabled: boolean;
  modes: HostDisplayMode[];
}

export interface ListDisplaysResult {
  ok: boolean;
  displays: HostDisplay[];
  error?: string;
}

// A per-game shortcut already created on the Deck, persisted in
// game_shortcuts.json (see main.py), keyed by host_app_id. Feeds the
// "Games" tab (grid) besides being used by gameShortcuts.ts to avoid
// recreating a shortcut that already exists.
export interface GameShortcutEntry {
  deck_app_id: number;
  name: string;
  is_steam: boolean;
}

export type GameShortcuts = Record<string, GameShortcutEntry>;

// Read-only view of the Runner's ActiveSession (GET /session/status),
// with the friendly name resolved from game_shortcuts.json on the Deck
// side (the Runner only knows the app_id) - polled by
// QuickAccessContent.tsx to show "Game X is running".
export interface SessionStatus {
  running: boolean;
  app_id: string | null;
  name: string | null;
}
