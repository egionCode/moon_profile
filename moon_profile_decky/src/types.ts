export interface MoonlightConfig {
  resolution: string; // ex: "3840x2160"
  fps: number; // ex: 60
  bitrate: number; // em kbps, ex: 150000
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
}

export interface Profile {
  id: string; // ex: "docked-tv-4k-hdr"
  name: string; // ex: "Docked TV 4K HDR"
  trigger: "docked" | "handheld" | "manual";
  moonlight: MoonlightConfig;
  host: HostConfig;
}

// Valores CSS crus (ex: "32px", "2.8vw") aplicados direto no "position:
// absolute" do botao na tela do jogo (ver GameActionButton.tsx) - string
// vazia significa "nao setar essa propriedade".
export interface ButtonPosition {
  top: string;
  bottom: string;
  left: string;
  right: string;
}

export interface Config {
  host: string; // ex: "192.168.1.6"
  username: string; // credencial admin do Apollo
  password: string; // credencial admin do Apollo
  button_position: ButtonPosition;
  // MoonProfile Runner (daemon Tauri/Rust no host, Fase 5) - suplementa a
  // deteccao de fim de sessao que o Apollo nao consegue fazer sozinho.
  // Sem autenticacao (servidor aberto na LAN, decisao explicita). Roda na
  // MESMA maquina que o Apollo - so' a porta muda, o host e' "host" acima.
  runner_port: number;
}

export interface StreamResult {
  ok: boolean;
  profile?: string;
  context?: string;
  error?: string;
  runner_path?: string;
  launch_env?: Record<string, string>;
}

// Um jogo listado pelo MoonProfile Runner (ver moon_profile_runner/src-tauri/
// src/games.rs) - Estagio A: so' jogos Steam reais (is_steam sempre true por
// enquanto, non-Steam fica pro Estagio B).
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

// Um monitor/output de tela do host (via kscreen-doctor -j, ver
// moon_profile_runner/src-tauri/src/displays.rs) - alimenta o select de
// "Output alvo" e a lista de outputs a desabilitar no ProfileEditor.tsx.
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

// Um atalho por jogo ja criado no Deck - persistido em
// game_shortcuts.json (ver main.py), chave e' o host_app_id. Alimenta a
// aba "Jogos" (grid) alem de servir pra gameShortcuts.ts nao recriar
// atalho que ja existe.
export interface GameShortcutEntry {
  deck_app_id: number;
  name: string;
  is_steam: boolean;
}

export type GameShortcuts = Record<string, GameShortcutEntry>;
