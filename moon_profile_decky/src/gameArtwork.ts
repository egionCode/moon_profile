// Aplica capa/hero num atalho criado por gameShortcuts.ts, usando
// SteamClient.Apps.SetCustomArtworkForApp (API real confirmada lendo o
// codigo-fonte do SteamGridDB/decky-steamgriddb, src/hooks/useSGDB.tsx -
// e' tudo client-side: busca a imagem, base64, chama essa funcao).
//
// "ELibraryAssetType" e' um enum real do @decky/ui mas nao fica alcancavel
// via import publico (so' o "Apps" TYPE e' re-exportado, nao o enum) - o
// proprio decky-steamgriddb usa os numeros literais direto pelo mesmo
// motivo (ver constants.ts deles: grid_p=0, hero=1, logo=2, grid_l=3,
// icon=4), entao seguimos o mesmo padrao aqui.
import { fetchNoCors } from "@decky/api";

const ASSET_TYPE_CAPSULE = 0; // capa vertical (grid_p)
const ASSET_TYPE_HERO = 1;

// Exportada tambem pra GamesGridSection.tsx reusar (preview da capa na
// nossa propria UI, nao a da Steam) - mesma logica, nao duplicar.
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
    console.error(`MoonProfile: falha ao buscar imagem de artwork (${url})`, e);
    return null;
  }
}

async function applyArtwork(appId: number, url: string, assetType: number): Promise<void> {
  const data = await getImageAsB64(url);
  if (!data) {
    return; // falha ao buscar - deixa sem essa arte especifica, nao trava o resto
  }
  await SteamClient.Apps.SetCustomArtworkForApp(appId, data, "jpg", assetType);
}

// CDN oficial e gratuita da Steam - so' funciona pra AppIDs Steam reais.
export function getSteamCapsuleUrl(steamAppId: string): string {
  return `https://cdn.cloudflare.steamstatic.com/steam/apps/${steamAppId}/library_600x900.jpg`;
}

function getSteamHeroUrl(steamAppId: string): string {
  return `https://cdn.cloudflare.steamstatic.com/steam/apps/${steamAppId}/library_hero.jpg`;
}

// So' funciona pra jogos que sao catalogo Steam real (precisa do AppID
// real da Valve, nao de um atalho non-Steam). Non-Steam fica pro Estagio B
// (SteamGridDB).
export async function applySteamCdnArtwork(shortcutAppId: number, steamAppId: string): Promise<void> {
  await Promise.all([
    applyArtwork(shortcutAppId, getSteamCapsuleUrl(steamAppId), ASSET_TYPE_CAPSULE),
    applyArtwork(shortcutAppId, getSteamHeroUrl(steamAppId), ASSET_TYPE_HERO),
  ]);
}
