// Coleção "Streaming" do Deck - agrupa os atalhos sincronizados numa pasta
// visivel na biblioteca, separada do resto da colecao de jogos. Igual
// SteamClient.Apps.SetCustomArtworkForApp em gameArtwork.ts,
// window.collectionStore nao e' tipado por @decky/ui - shape confirmado
// lendo o codigo-fonte real do MoonDeck (FrogTheFrog/moondeck,
// src/steam-utils/), que resolve o mesmo problema (colecao non-Steam com
// dedup) pro streaming via Moonlight.

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

// Busca por tag (nome) em vez de criar sempre - evita duplicar a colecao
// a cada sincronizacao (GetCollectionIDByUserTag/NewUnsavedCollection e'
// o mesmo padrao que o MoonDeck usa pra colecao propria dele).
async function getOrCreateStreamingCollection(): Promise<SteamCollection | null> {
  const existingId = window.collectionStore.GetCollectionIDByUserTag(COLLECTION_TAG);
  if (existingId !== null) {
    const existing = window.collectionStore.GetCollection(existingId);
    if (existing !== undefined) {
      return existing;
    }
  }

  const created = window.collectionStore.NewUnsavedCollection(COLLECTION_TAG, undefined, []);
  if (created === undefined) {
    return null;
  }
  await created.Save(); // sem Save() a colecao nao persiste (so' fica em memoria)
  return created;
}

// Adiciona os atalhos sincronizados (deck_app_id) na colecao "Streaming",
// criando-a se ainda nao existir. Dedup manual via collection.apps.has -
// AddApps nao deduplica sozinho (confirmado no codigo do MoonDeck) - so'
// persiste (Save) se realmente faltava alguem, overwrite/reuso correto
// entre sincronizacoes sem duplicar entradas.
export async function addShortcutsToStreamingCollection(deckAppIds: number[]): Promise<void> {
  const collection = await getOrCreateStreamingCollection();
  if (collection === null) {
    console.error('MoonProfile: falha ao criar/obter a colecao "Streaming"');
    return;
  }

  const missingOverviews = deckAppIds
    .filter((appId) => !collection.apps.has(appId))
    .map((appId) => window.appStore.GetAppOverviewByAppID(appId))
    .filter((overview): overview is NonNullable<typeof overview> => overview !== null);

  if (missingOverviews.length === 0) {
    return;
  }

  collection.AsDragDropCollection().AddApps(missingOverviews);
  await collection.Save();
}
