// Applies a capsule/hero image to a shortcut created by gameShortcuts.ts,
// using SteamClient.Apps.SetCustomArtworkForApp (real API confirmed by
// reading the source of SteamGridDB/decky-steamgriddb,
// src/hooks/useSGDB.tsx: it's all client-side, fetch the image, base64 it,
// call this function).
//
// "ELibraryAssetType" is a real enum in @decky/ui but isn't reachable via
// public import (only the "Apps" TYPE is re-exported, not the enum).
// decky-steamgriddb itself uses the literal numbers directly for the same
// reason (see their constants.ts: grid_p=0, hero=1, logo=2, grid_l=3,
// icon=4), so we follow the same pattern here.
import { fetchNoCors } from "@decky/api";

const ASSET_TYPE_CAPSULE = 0; // vertical capsule (grid_p)
const ASSET_TYPE_HERO = 1;

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
    console.error(`MoonProfile: failed to fetch artwork image (${url})`, e);
    return null;
  }
}

async function applyArtwork(appId: number, url: string, assetType: number): Promise<void> {
  const data = await getImageAsB64(url);
  if (!data) {
    return; // fetch failed, leave this specific artwork unset, don't block the rest
  }
  await SteamClient.Apps.SetCustomArtworkForApp(appId, data, "jpg", assetType);
}

// Official, free Steam CDN, only works for real Steam AppIDs.
export function getSteamCapsuleUrl(steamAppId: string): string {
  return `https://cdn.cloudflare.steamstatic.com/steam/apps/${steamAppId}/library_600x900.jpg`;
}

function getSteamHeroUrl(steamAppId: string): string {
  return `https://cdn.cloudflare.steamstatic.com/steam/apps/${steamAppId}/library_hero.jpg`;
}

// Only works for games that are actually in the Steam catalog (needs the
// real Valve AppID, not a non-Steam shortcut). Non-Steam is left for Stage B
// (SteamGridDB).
export async function applySteamCdnArtwork(shortcutAppId: number, steamAppId: string): Promise<void> {
  await Promise.all([
    applyArtwork(shortcutAppId, getSteamCapsuleUrl(steamAppId), ASSET_TYPE_CAPSULE),
    applyArtwork(shortcutAppId, getSteamHeroUrl(steamAppId), ASSET_TYPE_HERO),
  ]);
}
