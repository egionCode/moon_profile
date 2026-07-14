import { callable } from "@decky/api";
import { Config, GameShortcuts, ListGamesResult, Profile, StreamResult } from "./types";

export const getConfig = callable<[], Config>("get_config");
export const saveConfig = callable<[config: Config], void>("save_config");

export const getProfiles = callable<[], Profile[]>("get_profiles");
export const saveProfiles = callable<[profiles: Profile[]], void>("save_profiles");

export const detectContext = callable<[], "docked" | "handheld">("detect_context");

export const streamGame = callable<[app_id: number], StreamResult>("stream_game");
export const stopStream = callable<[], StreamResult>("stop_stream");

export const getLogs = callable<[lines: number], string>("get_logs");

export const listHostGames = callable<[], ListGamesResult>("list_host_games");

export const getGameShortcuts = callable<[], GameShortcuts>("get_game_shortcuts");
export const saveGameShortcuts = callable<[shortcuts: GameShortcuts], void>("save_game_shortcuts");

export const getStreamingCollectionId = callable<[], string | null>("get_streaming_collection_id");
export const saveStreamingCollectionId = callable<[collection_id: string], void>("save_streaming_collection_id");
