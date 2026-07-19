// Applies capsule/hero/logo/icon images to a shortcut created by
// gameShortcuts.ts, using SteamClient.Apps.SetCustomArtworkForApp (real
// API confirmed by reading the source of SteamGridDB/decky-steamgriddb,
// src/hooks/useSGDB.tsx: it's all client-side, fetch the image, base64 it,
// call this function).
//
// "ELibraryAssetType" is a real enum in @decky/ui but isn't reachable via
// public import (only the "Apps" TYPE is re-exported, not the enum).
// decky-steamgriddb itself uses the literal numbers directly for the same
// reason (see their constants.ts: grid_p=0, hero=1, logo=2, grid_l=3,
// icon=4), so we follow the same pattern here.
//
// Every apply* function here is best-effort and NEVER throws: a missing
// image, a network error, or SetCustomArtworkForApp itself rejecting just
// skips that one asset type and logs it (see logFrontendError) - one
// failing asset (or one failing game, in gameSync.ts) must never stop the
// rest of the sync.
import { fetchNoCors } from "@decky/api";
import { logFrontendError } from "./api";

const ASSET_TYPE_CAPSULE = 0; // vertical grid (library)
const ASSET_TYPE_HERO = 1; // background shown behind the game page
const ASSET_TYPE_LOGO = 2;
const ASSET_TYPE_WIDE_CAPSULE = 3; // wide grid (recently played, home)
const ASSET_TYPE_ICON = 4;

// Also exported for GamesGridSection.tsx to reuse (capsule preview in our
// own UI, not Steam's) - same logic, avoid duplicating it.
export async function getImageAsB64(url: string): Promise<string | null> {
  try {
    const response = await fetchNoCors(url);
    if (!response.ok) {
      return null;
    }
    const buffer = await response.arrayBuffer();
    const bytes = new Uint8Array(buffer);
    let binary = "";
    for (let i = 0; i < bytes.byteLength; i++) {
      binary += String.fromCharCode(bytes[i]);
    }
    return btoa(binary);
  } catch (e) {
    void logFrontendError(`failed to fetch artwork image (${url}): ${e}`);
    return null;
  }
}

async function applyArtwork(appId: number, url: string, assetType: number): Promise<void> {
  const data = await getImageAsB64(url);
  if (!data) {
    return; // fetch already logged its own failure, leave this artwork unset
  }
  try {
    await SteamClient.Apps.SetCustomArtworkForApp(appId, data, "jpg", assetType);
  } catch (e) {
    void logFrontendError(`SetCustomArtworkForApp failed (appId=${appId}, assetType=${assetType}): ${e}`);
  }
}

// Applies every given (url, assetType) pair independently - Promise.all
// on the individual applyArtwork calls would be fine too (they already
// never throw), but allSettled makes the "one failure can't affect the
// others" guarantee explicit rather than implicit.
async function applyAll(appId: number, assets: { url: string | undefined; assetType: number }[]): Promise<void> {
  await Promise.allSettled(
    assets.filter((a): a is { url: string; assetType: number } => a.url !== undefined).map((a) => applyArtwork(appId, a.url, a.assetType)),
  );
}

// Official, free Steam CDN, only works for real Steam AppIDs.
export function getSteamCapsuleUrl(steamAppId: string): string {
  return `https://cdn.cloudflare.steamstatic.com/steam/apps/${steamAppId}/library_600x900.jpg`;
}

// Only works for games that are actually in the Steam catalog (needs the
// real Valve AppID, not a non-Steam shortcut). No official CDN "icon" -
// unlike grid/hero/logo/wide-grid, Steam's small square icon is keyed by a
// per-app hash that isn't derivable from the AppID alone, so it's left
// unset here (SteamGridDB's non-Steam path below does have a real
// /icons/ endpoint, so it gets all 5 asset types).
export async function applySteamCdnArtwork(shortcutAppId: number, steamAppId: string): Promise<void> {
  const base = `https://cdn.cloudflare.steamstatic.com/steam/apps/${steamAppId}`;
  await applyAll(shortcutAppId, [
    { url: getSteamCapsuleUrl(steamAppId), assetType: ASSET_TYPE_CAPSULE },
    { url: `${base}/library_hero.jpg`, assetType: ASSET_TYPE_HERO },
    { url: `${base}/logo.png`, assetType: ASSET_TYPE_LOGO },
    { url: `${base}/header.jpg`, assetType: ASSET_TYPE_WIDE_CAPSULE },
  ]);
}

// Non-Steam games have no official Store page, so no CDN art -
// SteamGridDB (https://www.steamgriddb.com/api/v2) is a free community
// database keyed by game NAME (autocomplete search) instead of a Steam
// AppID, and covers all 5 asset types. Requires the user's own API key
// (config.steamgriddb_api_key, free from their SteamGridDB account
// preferences) - see ApolloConfigSection.
const STEAMGRIDDB_BASE_URL = "https://www.steamgriddb.com/api/v2";

interface SteamGridDbSearchResult {
  success: boolean;
  data?: { id: number; name: string }[];
}

interface SteamGridDbImageResult {
  success: boolean;
  data?: { url: string }[];
}

async function steamGridDbFetch<T>(path: string, apiKey: string): Promise<T | null> {
  try {
    const response = await fetchNoCors(`${STEAMGRIDDB_BASE_URL}${path}`, {
      headers: { Authorization: `Bearer ${apiKey}` },
    });
    if (!response.ok) {
      void logFrontendError(`SteamGridDB request failed with HTTP ${response.status} (${path})`);
      return null;
    }
    return (await response.json()) as T;
  } catch (e) {
    void logFrontendError(`SteamGridDB request failed (${path}): ${e}`);
    return null;
  }
}

async function firstImageUrl(path: string, apiKey: string): Promise<string | undefined> {
  const result = await steamGridDbFetch<SteamGridDbImageResult>(path, apiKey);
  return result?.data?.[0]?.url;
}

async function findSteamGridDbGameId(gameName: string, apiKey: string): Promise<number | undefined> {
  const search = await steamGridDbFetch<SteamGridDbSearchResult>(
    `/search/autocomplete/${encodeURIComponent(gameName)}`,
    apiKey,
  );
  return search?.data?.[0]?.id;
}

// Best-effort, same as applySteamCdnArtwork: any step failing (no API key
// configured upstream, game not found on SteamGridDB, network error) just
// leaves that shortcut's artwork unset, it never throws or blocks the
// rest of the sync (see gameSync.ts).
export async function applySteamGridDbArtwork(shortcutAppId: number, gameName: string, apiKey: string): Promise<void> {
  const gameId = await findSteamGridDbGameId(gameName, apiKey);
  if (gameId === undefined) {
    void logFrontendError(`SteamGridDB has no match for "${gameName}", skipping artwork`);
    return;
  }

  const [capsuleUrl, wideCapsuleUrl, heroUrl, logoUrl, iconUrl] = await Promise.all([
    firstImageUrl(`/grids/game/${gameId}?dimensions=600x900`, apiKey),
    firstImageUrl(`/grids/game/${gameId}?dimensions=460x215,920x430`, apiKey),
    firstImageUrl(`/heroes/game/${gameId}`, apiKey),
    firstImageUrl(`/logos/game/${gameId}`, apiKey),
    firstImageUrl(`/icons/game/${gameId}`, apiKey),
  ]);

  await applyAll(shortcutAppId, [
    { url: capsuleUrl, assetType: ASSET_TYPE_CAPSULE },
    { url: wideCapsuleUrl, assetType: ASSET_TYPE_WIDE_CAPSULE },
    { url: heroUrl, assetType: ASSET_TYPE_HERO },
    { url: logoUrl, assetType: ASSET_TYPE_LOGO },
    { url: iconUrl, assetType: ASSET_TYPE_ICON },
  ]);
}

// Exported for GamesGridSection.tsx's own capsule preview (not Steam's
// UI) - same lookup applySteamGridDbArtwork uses for the vertical grid,
// just returning the URL instead of applying it.
export async function getSteamGridDbCapsuleUrl(gameName: string, apiKey: string): Promise<string | null> {
  const gameId = await findSteamGridDbGameId(gameName, apiKey);
  if (gameId === undefined) {
    return null;
  }
  return (await firstImageUrl(`/grids/game/${gameId}?dimensions=600x900`, apiKey)) ?? null;
}
