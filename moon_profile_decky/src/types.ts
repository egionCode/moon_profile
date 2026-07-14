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
  // Sem autenticacao (servidor aberto na LAN, decisao explicita).
  runner_host: string;
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

export interface SessionStatus {
  ok: boolean;
  running: boolean;
}
