// Coleção "Streaming" do Deck - agrupa os atalhos sincronizados numa pasta
// visivel na biblioteca, separada do resto da colecao de jogos. Igual
// SteamClient.Apps.SetCustomArtworkForApp em gameArtwork.ts,
// window.collectionStore nao e' tipado por @decky/ui - shape confirmado
// lendo o codigo-fonte real do MoonDeck (FrogTheFrog/moondeck,
// src/steam-utils/), que resolve o mesmo problema (colecao non-Steam com
// dedup) pro streaming via Moonlight.

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

// O id persistido (config, ver api.ts) e' a fonte de verdade primeiro -
// sobrevive a renomeacao manual da colecao (o "tag" usado por
// GetCollectionIDByUserTag e' derivado do nome exibido, quebra se o
// usuario renomear na Steam). Busca por tag e' so' o fallback pra achar
// uma colecao ja existente da primeira vez (antes de termos um id
// persistido) ou se o id salvo ficou orfao (colecao apagada por fora).
// So' cria uma nova de verdade se nenhuma das duas achar nada.
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

  // Cria ja' com os apps iniciais (3o argumento de NewUnsavedCollection),
  // em vez de criar vazia e so' depois chamar AddApps - um unico Save().
  const created = window.collectionStore.NewUnsavedCollection(COLLECTION_TAG, undefined, initialOverviews);
  if (created === undefined) {
    return null;
  }
  await created.Save();

  // NewUnsavedCollection nao devolve o id (so' o objeto da colecao) - o
  // unico jeito de descobrir o id de verdade e' perguntar de novo pelo
  // tag, agora que ja foi salva.
  const newId = window.collectionStore.GetCollectionIDByUserTag(COLLECTION_TAG);
  if (newId === null) {
    return null;
  }
  return { collection: created, id: newId };
}

// Adiciona os atalhos sincronizados (deck_app_id) na colecao "Streaming",
// criando-a se ainda nao existir, e persiste o id resolvido pra
// sincronizacoes futuras nao dependerem so' da busca por tag. Dedup
// manual via collection.apps.has - AddApps nao deduplica sozinho
// (confirmado no codigo do MoonDeck) - so' persiste (Save) se realmente
// faltava alguem. Retorna false (sem lancar excecao) se algo deu errado,
// pro chamador poder avisar o usuario em vez de falhar silencioso.
export async function addShortcutsToStreamingCollection(deckAppIds: number[]): Promise<boolean> {
  const overviews = deckAppIds
    .map((appId) => window.appStore.GetAppOverviewByAppID(appId))
    .filter((overview): overview is NonNullable<typeof overview> => overview !== null);

  const persistedId = await getStreamingCollectionId();
  const resolved = await resolveStreamingCollection(persistedId, overviews);
  if (resolved === null) {
    console.error('MoonProfile: falha ao criar/obter a colecao "Streaming"');
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
