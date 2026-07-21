import { callable } from "@decky/api";
import {
  Config,
  FetchHostMacResult,
  GameShortcuts,
  HostStatus,
  ListDisplaysResult,
  ListGamesResult,
  OkResult,
  Profile,
  StreamResult,
} from "./types";

export const getConfig = callable<[], Config>("get_config");
export const saveConfig = callable<[config: Config], void>("save_config");

export const getProfiles = callable<[], Profile[]>("get_profiles");
export const saveProfiles = callable<[profiles: Profile[]], void>("save_profiles");

export const detectContext = callable<[], "docked" | "handheld">("detect_context");

export const stopStream = callable<[], StreamResult>("stop_stream");

export const getLogs = callable<[lines: number], string>("get_logs");
export const logFrontendError = callable<[message: string], void>("log_frontend_error");

export const listHostGames = callable<[], ListGamesResult>("list_host_games");
export const listHostDisplays = callable<[], ListDisplaysResult>("list_host_displays");

export const getGameShortcuts = callable<[], GameShortcuts>("get_game_shortcuts");
export const saveGameShortcuts = callable<[shortcuts: GameShortcuts], void>("save_game_shortcuts");

export const getStreamingCollectionId = callable<[], string | null>("get_streaming_collection_id");
export const saveStreamingCollectionId = callable<[collection_id: string], void>("save_streaming_collection_id");

export const getHostStatus = callable<[], HostStatus>("get_host_status");
export const fetchHostMac = callable<[], FetchHostMacResult>("fetch_host_mac");
export const shutdownHost = callable<[], OkResult>("shutdown_host");
export const wakeHost = callable<[], OkResult>("wake_host");
