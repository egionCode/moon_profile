// Orchestrates the "one shortcut per host game" sync, per-game shortcuts
// (see docs/prd.md): lists the games via the MoonProfile Runner, ensures a
// visible shortcut for each one (gameShortcuts.ts), applies cover/hero/
// logo/icon art (gameArtwork.ts - the official Steam CDN for real Steam
// games, SteamGridDB for non-Steam ones), and groups everything into the
// "Streaming" collection (gameCollection.ts). The shortcut map is read and
// saved ONCE here (not a roundtrip per game) and persists in
// game_shortcuts.json (see main.py), feeding the "Games" tab.
//
// Manual sync (button), not an automatic background one, same incremental
// spirit as the rest of the project.
import { toaster } from "@decky/api";
import { getGameShortcuts, listHostGames, logFrontendError, saveGameShortcuts } from "./api";
import { STEAMGRIDDB_API_KEY } from "./env";
import { ensureGameShortcut } from "./gameShortcuts";
import { applySteamCdnArtwork, applySteamGridDbArtwork } from "./gameArtwork";
import { addShortcutsToStreamingCollection } from "./gameCollection";

// Artwork itself never throws (see gameArtwork.ts), but this wraps the
// call anyway: a single unexpected exception here must never abort the
// whole sync loop over the remaining games.
async function applyArtworkSafely(shortcutAppId: number, game: { name: string; host_app_id: string; is_steam: boolean }, steamgriddbApiKey: string): Promise<void> {
  try {
    if (game.is_steam) {
      await applySteamCdnArtwork(shortcutAppId, game.host_app_id);
    } else if (steamgriddbApiKey) {
      await applySteamGridDbArtwork(shortcutAppId, game.name, steamgriddbApiKey);
    }
  } catch (e) {
    void logFrontendError(`unexpected error applying artwork for "${game.name}": ${e}`);
  }
}

// onProgress (optional) is called after EACH game processed (success or
// failure, the counter always advances), it feeds the progress bar in
// QuickAccessContent.tsx. current is 1-based (1st game = current=1).
export async function syncHostGames(onProgress?: (current: number, total: number, gameName: string) => void): Promise<void> {
  const result = await listHostGames();
  if (!result.ok || !result.runner_path) {
    toaster.toast({ title: "MoonProfile - error", body: result.error ?? "Unknown failure" });
    return;
  }

  const shortcuts = await getGameShortcuts();

  let created = 0;
  const deckAppIds: number[] = [];
  const total = result.games.length;
  for (const [index, game] of result.games.entries()) {
    onProgress?.(index + 1, total, game.name);

    const shortcutAppId = await ensureGameShortcut(
      shortcuts,
      game.host_app_id,
      game.name,
      result.runner_path,
      game.is_steam,
    );
    if (shortcutAppId === null) {
      void logFrontendError(`failed to create shortcut for "${game.name}" (${game.host_app_id})`);
      continue;
    }
    deckAppIds.push(shortcutAppId);
    await applyArtworkSafely(shortcutAppId, game, STEAMGRIDDB_API_KEY);
    created++;
  }

  await saveGameShortcuts(shortcuts);
  // A single call with all the appids, the dedup against what's already
  // in the collection happens inside addShortcutsToStreamingCollection.
  const collectionOk = await addShortcutsToStreamingCollection(deckAppIds);
  if (!collectionOk) {
    toaster.toast({
      title: "MoonProfile - warning",
      body: 'Games synced, but failed to organize into the "Streaming" collection (see logs)',
    });
  }

  toaster.toast({
    title: "MoonProfile",
    body: `${created} of ${result.games.length} games synced`,
  });
}
