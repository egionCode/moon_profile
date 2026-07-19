export interface MoonlightConfig {
  resolution: string; // ex: "3840x2160"
  fps: number; // ex: 60
  bitrate: number; // in kbps, ex: 150000
  codec: "HEVC" | "AV1" | "H264";
  hdr: boolean;
}

export interface HostConfig {
  target_output: string; // ex: "HDMI-A-1"
  resolution: string; // ex: "3840x2160"
  fps: number; // ex: 60
  hdr: boolean;
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
  id: string; // ex: "docked-tv-4k-hdr"
  name: string; // ex: "Docked TV 4K HDR"
  trigger: "docked" | "handheld" | "manual";
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
  // SteamGridDB API key (free, https://www.steamgriddb.com/profile/preferences/api),
  // used to fetch cover/hero art for non-Steam games synced from the host
  // (see gameArtwork.ts's applySteamGridDbArtwork) - real Steam games use
  // the official CDN instead and don't need this. Empty string means
  // non-Steam games sync without artwork.
  steamgriddb_api_key: string;
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

// A host display/output (via kscreen-doctor -j, see
// moon_profile_runner/src-tauri/src/displays.rs), feeds the "Target
// output" select and the list of outputs to disable in ProfileEditor.tsx.
export interface HostDisplay {
  name: string; // ex: "HDMI-A-1"
  connected: boolean;
  enabled: boolean;
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
