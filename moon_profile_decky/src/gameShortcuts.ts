// Cria um atalho Steam POR JOGO do host (um pra cada host_app_id vindo do
// MoonProfile Runner - ver gameSync.ts), visivel na biblioteca (ao
// contrario do "MoonProfile Launcher" compartilhado e escondido de
// steamShortcut.ts, que ainda existe pro botao antigo da tela do jogo).
//
// Diferenca chave que simplifica bastante: as Launch Options sao setadas
// UMA VEZ na criacao (so' "MOONPROFILE_HOST_APP_ID=<id>"), nao a cada
// lancamento - o runner.py agora se auto-configura sozinho na hora de
// rodar (le config/perfis do disco, fala com o Apollo), entao nao precisa
// mais de JS rodando ANTES do clique (o que nao existiria mesmo, se o
// usuario clicar "Jogar" nativo da Steam num atalho ja existente).
//
// O mapa host_app_id -> atalho e' persistido em game_shortcuts.json (via
// main.py, ver api.ts) em vez de localStorage - da controle/visibilidade
// real (alimenta a aba "Jogos") e sobrevive a limpeza de dados do
// navegador embutido. O CHAMADOR (gameSync.ts) e' quem le/salva o mapa
// inteiro (uma vez so' pra N jogos, nao um roundtrip por jogo) - as
// funcoes aqui so' leem/mutam o objeto em memoria que recebem.

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

// Garante que existe um atalho pra este jogo do host, criando se
// necessario. Muta "shortcuts" (adiciona/atualiza a entrada) - quem chama
// e' responsavel por persistir o mapa depois (saveGameShortcuts). Retorna
// o appId (do atalho no Deck) ou null se a criacao falhar. NAO esconde o
// atalho - ele e' o ponto de entrada visivel agora.
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
  // "%command%": placeholder da Steam pra "roda o executavel do atalho
  // aqui" - fixo pra sempre, so' setado nesta criacao (nao antes de cada
  // lancamento, diferente do atalho compartilhado antigo).
  SteamClient.Apps.SetAppLaunchOptions(appId, `MOONPROFILE_HOST_APP_ID=${hostAppId} %command%`);

  shortcuts[hostAppId] = { deck_app_id: appId, name, is_steam: isSteam };
  return appId;
}

// Remove da Steam todos os atalhos rastreados no mapa - usado pelo botao
// "Limpar jogos sincronizados" (GamesGridSection.tsx). Quem chama ainda
// precisa persistir o mapa vazio depois (saveGameShortcuts({})).
export function removeAllGameShortcuts(shortcuts: GameShortcuts): void {
  for (const entry of Object.values(shortcuts)) {
    SteamClient.Apps.RemoveShortcut(entry.deck_app_id);
  }
}
