// Gerencia o atalho Steam compartilhado ("MoonProfile Launcher") que serve
// so' de veiculo pro Gamescope focar o Moonlight - ver runner/runner.py e
// main.py:stream_game() pra entender o resto do mecanismo.
//
// Isso e' o mesmo truque que o MoonDeck usa (src/steam-utils/addShortcut.ts
// deles): o Gamescope so foca/mostra janelas lancadas atraves do mecanismo
// real da Steam. Um atalho non-Steam apontando pro runner.py resolve isso.
//
// O atalho e' criado UMA vez e cacheado (localStorage, sobrevive a reload
// do plugin); nunca precisa recriar dele em uso normal, so' se o cache
// apontar pra um appid que nao existe mais (steam client resetado, etc).

const STORAGE_KEY = "moonprofile-launcher-appid";
const SHORTCUT_NAME = "MoonProfile Launcher";

// Valores confirmados olhando o codigo-fonte do MoonDeck (src/steam-utils/
// launchApp.ts): "SteamClient.Apps.RunGame(gameId, "", -1, 100)". O "100"
// e' ELaunchSource._2ftLibraryDetails (nao _10ft=200, que eu tinha chutado
// errado antes) - usamos o mesmo valor deles por seguranca, ja que e' o
// que esta' comprovadamente funcionando no MoonDeck.
const LAUNCH_SOURCE = 100;

function wait(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// window.collectionStore nao e' tipado no @decky/ui - mesmo padrao que o
// MoonDeck usa (src/steam-utils/getCollectionStore.ts): cast manual, so'
// com os dois metodos que a gente realmente precisa.
interface CollectionStore {
  BIsHidden: (appId: number) => boolean;
  SetAppsAsHidden: (appIds: number[], hide: boolean) => void;
}

function getCollectionStore(): CollectionStore | null {
  return (window as unknown as { collectionStore?: CollectionStore }).collectionStore ?? null;
}

// Esconde o atalho da biblioteca - e' so' um detalhe de implementacao, nao
// algo que o usuario deveria ver/clicar direto. "hide the app will remove
// it from other collections" (comentario do proprio MoonDeck) - inofensivo
// pro nosso caso, o atalho nao pertence a nenhuma colecao mesmo.
function hideShortcut(appId: number): void {
  const collectionStore = getCollectionStore();
  if (collectionStore === null) {
    console.error("MoonProfile: collectionStore nao disponivel, nao consegui esconder o atalho");
    return;
  }
  if (!collectionStore.BIsHidden(appId)) {
    collectionStore.SetAppsAsHidden([appId], true);
  }
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

// Garante que o atalho existe, criando se necessario. Retorna o appId ou
// null se a criacao falhar (nesse caso o chamador deve avisar o usuario -
// segundo o proprio MoonDeck, falhas aqui costumam significar "cliente
// Steam em estado ruim, precisa reiniciar").
export async function ensureLauncherShortcut(execPath: string): Promise<number | null> {
  const cached = localStorage.getItem(STORAGE_KEY);
  if (cached) {
    const appId = Number(cached);
    if (!Number.isNaN(appId) && window.appStore.GetAppOverviewByAppID(appId) !== null) {
      hideShortcut(appId); // defensivo - garante que continua escondido
      return appId;
    }
    localStorage.removeItem(STORAGE_KEY);
  }

  const appId = await SteamClient.Apps.AddShortcut(SHORTCUT_NAME, execPath, "", "");
  if (typeof appId !== "number") {
    return null;
  }
  if (!(await waitForAppOverview(appId))) {
    return null;
  }

  SteamClient.Apps.SetShortcutName(appId, SHORTCUT_NAME);
  hideShortcut(appId);
  localStorage.setItem(STORAGE_KEY, String(appId));
  return appId;
}

function buildLaunchOptions(env: Record<string, string>): string {
  const pairs = Object.entries(env).map(([key, value]) => `${key}=${value}`);
  // "%command%" e' o placeholder da Steam pra "roda o executavel do atalho
  // aqui" - sem isso a Steam nao sabe onde encaixar o exec path na string
  // de launch options (mesmo padrao usado pelo MoonDeck).
  pairs.push("%command%");
  return pairs.join(" ");
}

// Seta as launch options com as variaveis do lancamento atual e dispara o
// atalho. SetAppLaunchOptions nao retorna Promise - nao temos confirmacao
// de que a mudanca esta' persistida antes do RunGame ler ela, por isso o
// delay de seguranca. Ainda precisa validar isso rodando de verdade no Deck.
//
// BUG JA ENCONTRADO E CORRIGIDO: RunGame nao aceita o appId numerico como
// string - precisa do "gameid" do overview do app (um id interno diferente
// pra atalhos non-Steam). Passar String(appId) direto faz o RunGame
// silenciosamente nao fazer nada (confirmado rodando no device: nenhum
// erro, nenhum processo novo, nenhuma conexao no Apollo). Ver getGameId.ts
// do MoonDeck - o mesmo padrao.
export async function launchViaSteam(appId: number, env: Record<string, string>): Promise<void> {
  const overview = window.appStore.GetAppOverviewByAppID(appId);
  const gameId = overview?.gameid;
  if (!gameId) {
    throw new Error(`Nao consegui achar o gameid do atalho (appId=${appId})`);
  }

  SteamClient.Apps.SetAppLaunchOptions(appId, buildLaunchOptions(env));
  await wait(300);
  SteamClient.Apps.RunGame(gameId, "", -1, LAUNCH_SOURCE);
}
