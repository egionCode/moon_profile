import { callable } from "@decky/api";
import { Config, Profile, SessionStatus, StreamResult } from "./types";

export const getConfig = callable<[], Config>("get_config");
export const saveConfig = callable<[config: Config], void>("save_config");

export const getProfiles = callable<[], Profile[]>("get_profiles");
export const saveProfiles = callable<[profiles: Profile[]], void>("save_profiles");

export const detectContext = callable<[], "docked" | "handheld">("detect_context");

export const streamGame = callable<[app_id: number], StreamResult>("stream_game");
export const stopStream = callable<[], StreamResult>("stop_stream");

export const getLogs = callable<[lines: number], string>("get_logs");

export const checkSessionStatus = callable<[app_id: number], SessionStatus>("check_session_status");
