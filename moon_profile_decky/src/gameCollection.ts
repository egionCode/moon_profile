// The Deck's "Streaming" collection: groups the synced shortcuts into a
// folder visible in the library, separate from the rest of the game
// collection. Same as SteamClient.Apps.SetCustomArtworkForApp in
// gameArtwork.ts, window.collectionStore isn't typed by @decky/ui, its
// shape was confirmed by reading the real source of MoonDeck
// (FrogTheFrog/moondeck, src/steam-utils/), which solves the same problem
// (non-Steam collection with dedup) for streaming via Moonlight.

import { getStreamingCollectionId, saveStreamingCollectionId } from "./api";

const COLLECTION_TAG = "Streaming";

interface SteamCollection {
  apps: {
    has: (appId: number) => boolean;
  };
  AsDragDropCollection: () => {
    AddApps: (overviews: unknown[]) => void;
  };
  Save: () => Promise<void>;
}

interface CollectionStore {
  GetCollectionIDByUserTag: (tag: string) => string | null;
  GetCollection: (collectionId: string) => SteamCollection | undefined;
  NewUnsavedCollection: (tag: string, filter: unknown, overviews: unknown[]) => SteamCollection | undefined;
}

declare global {
  interface Window {
    collectionStore: CollectionStore;
  }
}

interface ResolvedCollection {
  collection: SteamCollection;
  id: string;
}

// The persisted id (config, see api.ts) is the source of truth first: it
// survives manual renaming of the collection (the "tag" used by
// GetCollectionIDByUserTag is derived from the displayed name, so it
// breaks if the user renames it in Steam). Tag lookup is just the fallback
// to find an already-existing collection the first time (before we have a
// persisted id) or if the saved id became orphaned (collection deleted
// externally). Only actually creates a new one if neither finds anything.
async function resolveStreamingCollection(
  persistedId: string | null,
  initialOverviews: unknown[],
): Promise<ResolvedCollection | null> {
  if (persistedId !== null) {
    const byId = window.collectionStore.GetCollection(persistedId);
    if (byId !== undefined) {
      return { collection: byId, id: persistedId };
    }
  }

  const idByTag = window.collectionStore.GetCollectionIDByUserTag(COLLECTION_TAG);
  if (idByTag !== null) {
    const byTag = window.collectionStore.GetCollection(idByTag);
    if (byTag !== undefined) {
      return { collection: byTag, id: idByTag };
    }
  }

  // Create it already with the initial apps (3rd argument of
  // NewUnsavedCollection), instead of creating it empty and calling
  // AddApps afterward: a single Save().
  const created = window.collectionStore.NewUnsavedCollection(COLLECTION_TAG, undefined, initialOverviews);
  if (created === undefined) {
    return null;
  }
  await created.Save();

  // NewUnsavedCollection doesn't return the id (only the collection
  // object), the only way to find out the real id is to ask again by tag,
  // now that it has been saved.
  const newId = window.collectionStore.GetCollectionIDByUserTag(COLLECTION_TAG);
  if (newId === null) {
    return null;
  }
  return { collection: created, id: newId };
}

// Adds the synced shortcuts (deck_app_id) to the "Streaming" collection,
// creating it if it doesn't exist yet, and persists the resolved id so
// future syncs don't have to depend solely on the tag lookup. Manual dedup
// via collection.apps.has, AddApps doesn't deduplicate on its own
// (confirmed in MoonDeck's code), only persists (Save) if something was
// actually missing. Returns false (without throwing) if something went
// wrong, so the caller can warn the user instead of failing silently.
export async function addShortcutsToStreamingCollection(deckAppIds: number[]): Promise<boolean> {
  const overviews = deckAppIds
    .map((appId) => window.appStore.GetAppOverviewByAppID(appId))
    .filter((overview): overview is NonNullable<typeof overview> => overview !== null);

  const persistedId = await getStreamingCollectionId();
  const resolved = await resolveStreamingCollection(persistedId, overviews);
  if (resolved === null) {
    console.error('MoonProfile: failed to create/get the "Streaming" collection');
    return false;
  }

  if (resolved.id !== persistedId) {
    await saveStreamingCollectionId(resolved.id);
  }

  const missingAppIds = deckAppIds.filter((appId) => !resolved.collection.apps.has(appId));
  if (missingAppIds.length === 0) {
    return true;
  }

  const missingOverviews = missingAppIds
    .map((appId) => window.appStore.GetAppOverviewByAppID(appId))
    .filter((overview): overview is NonNullable<typeof overview> => overview !== null);

  resolved.collection.AsDragDropCollection().AddApps(missingOverviews);
  await resolved.collection.Save();
  return true;
}
