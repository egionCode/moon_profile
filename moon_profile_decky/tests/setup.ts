// O ambiente de teste roda em Node puro (sem jsdom - nao renderizamos
// nenhum DOM de verdade, so' mockamos a superficie das APIs globais que a
// Steam injeta em window, ex: SteamClient/appStore/collectionStore). Sem
// isso, "window" nao existe em Node e o codigo sob teste quebra na
// primeira referencia.
if (typeof (globalThis as unknown as { window?: unknown }).window === "undefined") {
  (globalThis as unknown as { window: unknown }).window = globalThis;
}
