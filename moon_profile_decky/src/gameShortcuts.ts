// Creates a Steam shortcut PER host GAME (one for each host_app_id coming
// from the MoonProfile Runner, see gameSync.ts), visible in the library
// (unlike the shared, hidden "MoonProfile Launcher" from steamShortcut.ts,
// which still exists for the old game-page button).
//
// Key difference that simplifies things a lot: the Launch Options are set
// ONCE at creation time (just "MOONPROFILE_HOST_APP_ID=<id>"), not on every
// launch. runner.py now auto-configures itself when it runs (reads
// config/profiles from disk, talks to Apollo), so it no longer needs JS
// running BEFORE the click (which wouldn't exist anyway if the user clicks
// Steam's native "Play" on an already-existing shortcut).
//
// The host_app_id -> shortcut map is persisted in game_shortcuts.json (via
// main.py, see api.ts) instead of localStorage: gives real
// control/visibility (feeds the "Games" tab) and survives clearing the
// embedded browser's data. The CALLER (gameSync.ts) is the one that
// reads/saves the whole map (once for N games, not a roundtrip per game),
// the functions here only read/mutate the in-memory object they receive.

import { GameShortcuts } from "./types";

function wait(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitForAppOverview(appId: number, tries = 20): Promise<boolean> {
  for (let i = 0; i < tries; i++) {
    if (window.appStore.GetAppOverviewByAppID(appId) !== null) {
      return true;
    }
    await wait(250);
  }
  return false;
}

// Ensures a shortcut exists for this host game, creating one if needed.
// Mutates "shortcuts" (adds/updates the entry), the caller is responsible
// for persisting the map afterward (saveGameShortcuts). Returns the appId
// (of the shortcut on the Deck) or null if creation fails. Does NOT hide
// the shortcut, it is the visible entry point now.
export async function ensureGameShortcut(
  shortcuts: GameShortcuts,
  hostAppId: string,
  name: string,
  execPath: string,
  isSteam: boolean,
): Promise<number | null> {
  const existing = shortcuts[hostAppId];
  if (existing !== undefined && window.appStore.GetAppOverviewByAppID(existing.deck_app_id) !== null) {
    return existing.deck_app_id;
  }

  const appId = await SteamClient.Apps.AddShortcut(name, execPath, "", "");
  if (typeof appId !== "number") {
    return null;
  }
  if (!(await waitForAppOverview(appId))) {
    return null;
  }

  SteamClient.Apps.SetShortcutName(appId, name);
  // "%command%": Steam's placeholder for "run the shortcut's executable
  // here", fixed forever, only set at this creation (not before every
  // launch, unlike the old shared shortcut).
  SteamClient.Apps.SetAppLaunchOptions(appId, `MOONPROFILE_HOST_APP_ID=${hostAppId} %command%`);

  shortcuts[hostAppId] = { deck_app_id: appId, name, is_steam: isSteam };
  return appId;
}

// Removes from Steam every shortcut tracked in the map, used by the
// "Clear synced games" button (GamesGridSection.tsx). The caller still
// needs to persist the empty map afterward (saveGameShortcuts({})).
export function removeAllGameShortcuts(shortcuts: GameShortcuts): void {
  for (const entry of Object.values(shortcuts)) {
    SteamClient.Apps.RemoveShortcut(entry.deck_app_id);
  }
}
