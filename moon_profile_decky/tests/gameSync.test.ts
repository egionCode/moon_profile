import { describe, it, expect, vi, beforeEach } from "vitest";

const listHostGames = vi.fn();
const getGameShortcuts = vi.fn();
const saveGameShortcuts = vi.fn();
const ensureGameShortcut = vi.fn();
const applySteamCdnArtwork = vi.fn();
const addShortcutsToStreamingCollection = vi.fn();
const toast = vi.fn();

vi.mock("../src/api", () => ({
  listHostGames: (...args: unknown[]) => listHostGames(...args),
  getGameShortcuts: (...args: unknown[]) => getGameShortcuts(...args),
  saveGameShortcuts: (...args: unknown[]) => saveGameShortcuts(...args),
}));
vi.mock("../src/gameShortcuts", () => ({
  ensureGameShortcut: (...args: unknown[]) => ensureGameShortcut(...args),
}));
vi.mock("../src/gameArtwork", () => ({
  applySteamCdnArtwork: (...args: unknown[]) => applySteamCdnArtwork(...args),
}));
vi.mock("../src/gameCollection", () => ({
  addShortcutsToStreamingCollection: (...args: unknown[]) => addShortcutsToStreamingCollection(...args),
}));
vi.mock("@decky/api", () => ({
  toaster: { toast: (...args: unknown[]) => toast(...args) },
}));

// Dynamic import after the vi.mock calls (hoisted), same pattern already
// used in gameCollection.test.ts.
const { syncHostGames } = await import("../src/gameSync");

const GAMES = [
  { name: "Game A", host_app_id: "111", is_steam: true },
  { name: "Game B", host_app_id: "222", is_steam: true },
  { name: "Game C", host_app_id: "333", is_steam: false },
];

describe("syncHostGames progress callback", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listHostGames.mockResolvedValue({ ok: true, runner_path: "/runner/runner.py", games: GAMES });
    getGameShortcuts.mockResolvedValue({});
    saveGameShortcuts.mockResolvedValue(undefined);
    ensureGameShortcut.mockImplementation(async (_shortcuts, hostAppId) => Number(hostAppId));
    applySteamCdnArtwork.mockResolvedValue(undefined);
    addShortcutsToStreamingCollection.mockResolvedValue(true);
  });

  it("reports progress once per game, in order, with 1-based current and the game name", async () => {
    const onProgress = vi.fn();

    await syncHostGames(onProgress);

    expect(onProgress).toHaveBeenCalledTimes(3);
    expect(onProgress).toHaveBeenNthCalledWith(1, 1, 3, "Game A");
    expect(onProgress).toHaveBeenNthCalledWith(2, 2, 3, "Game B");
    expect(onProgress).toHaveBeenNthCalledWith(3, 3, 3, "Game C");
  });

  it("still advances progress for a game whose shortcut creation fails", async () => {
    ensureGameShortcut.mockImplementationOnce(async () => null); // "Game A" fails
    const onProgress = vi.fn();

    await syncHostGames(onProgress);

    expect(onProgress).toHaveBeenCalledTimes(3);
    expect(onProgress).toHaveBeenNthCalledWith(1, 1, 3, "Game A");
  });

  it("works fine without an onProgress callback", async () => {
    await expect(syncHostGames()).resolves.toBeUndefined();
  });
});
