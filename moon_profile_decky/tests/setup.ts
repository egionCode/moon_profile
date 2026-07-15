// The test environment runs in plain Node (no jsdom, we don't render any
// real DOM, we just mock the surface of the global APIs that Steam injects
// into window, ex: SteamClient/appStore/collectionStore). Without this,
// "window" doesn't exist in Node and the code under test breaks on the
// first reference.
if (typeof (globalThis as unknown as { window?: unknown }).window === "undefined") {
  (globalThis as unknown as { window: unknown }).window = globalThis;
}
