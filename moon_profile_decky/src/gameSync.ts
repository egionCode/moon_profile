// Orquestra a sincronizacao "um atalho por jogo do host" - Estagio A dos
// atalhos por jogo (ver docs/prd.md): lista os jogos via MoonProfile
// Runner, garante um atalho visivel pra cada um (gameShortcuts.ts), aplica
// capa/hero (gameArtwork.ts, so' pra jogos Steam reais por enquanto) e
// agrupa tudo na colecao "Streaming" (gameCollection.ts). O mapa de
// atalhos e' lido e salvo UMA vez so' aqui (nao um roundtrip por jogo) e
// persiste em game_shortcuts.json (ver main.py) - alimenta a aba "Jogos".
//
// Sincronizacao manual (botao), nao automatica em background - mesmo
// espirito incremental do resto do projeto.
import { toaster } from "@decky/api";
import { getGameShortcuts, listHostGames, saveGameShortcuts } from "./api";
import { ensureGameShortcut } from "./gameShortcuts";
import { applySteamCdnArtwork } from "./gameArtwork";
import { addShortcutsToStreamingCollection } from "./gameCollection";

// onProgress (opcional) e' chamado depois de CADA jogo processado (sucesso
// ou falha - o contador sempre avanca) - alimenta a barra de progresso em
// QuickAccessContent.tsx. current e' 1-based (1o jogo = current=1).
export async function syncHostGames(onProgress?: (current: number, total: number, gameName: string) => void): Promise<void> {
  const result = await listHostGames();
  if (!result.ok || !result.runner_path) {
    toaster.toast({ title: "MoonProfile - erro", body: result.error ?? "Falha desconhecida" });
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
      console.error(`MoonProfile: falha ao criar atalho pra "${game.name}" (${game.host_app_id})`);
      continue;
    }
    deckAppIds.push(shortcutAppId);
    if (game.is_steam) {
      await applySteamCdnArtwork(shortcutAppId, game.host_app_id);
    }
    created++;
  }

  await saveGameShortcuts(shortcuts);
  // uma chamada so' com todos os appids - o dedup contra quem ja' esta' na
  // colecao acontece dentro de addShortcutsToStreamingCollection.
  const collectionOk = await addShortcutsToStreamingCollection(deckAppIds);
  if (!collectionOk) {
    toaster.toast({
      title: "MoonProfile - aviso",
      body: 'Jogos sincronizados, mas falhou ao organizar na colecao "Streaming" (ver logs)',
    });
  }

  toaster.toast({
    title: "MoonProfile",
    body: `${created} de ${result.games.length} jogos sincronizados`,
  });
}
