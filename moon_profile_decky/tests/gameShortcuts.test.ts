import { describe, it, expect, vi, beforeEach } from "vitest";
import { ensureGameShortcut, removeAllGameShortcuts } from "../src/gameShortcuts";
import type { GameShortcuts } from "../src/types";

function overviewOf(appId: number): unknown {
  return { appid: appId };
}

describe("ensureGameShortcut", () => {
  beforeEach(() => {
    (window as any).SteamClient = {
      Apps: {
        AddShortcut: vi.fn(),
        SetShortcutName: vi.fn(),
        SetAppLaunchOptions: vi.fn(),
        RemoveShortcut: vi.fn(),
      },
    };
    (window as any).appStore = {
      GetAppOverviewByAppID: vi.fn().mockReturnValue(null),
    };
  });

  it("creates a new shortcut when none is tracked yet", async () => {
    const shortcuts: GameShortcuts = {};
    (window as any).SteamClient.Apps.AddShortcut.mockResolvedValue(999);
    (window as any).appStore.GetAppOverviewByAppID.mockReturnValue(overviewOf(999));

    const appId = await ensureGameShortcut(shortcuts, "123", "My Game", "/path/runner.py", true);

    expect(appId).toBe(999);
    expect((window as any).SteamClient.Apps.AddShortcut).toHaveBeenCalledWith("My Game", "/path/runner.py", "", "");
    expect((window as any).SteamClient.Apps.SetShortcutName).toHaveBeenCalledWith(999, "My Game");
    expect((window as any).SteamClient.Apps.SetAppLaunchOptions).toHaveBeenCalledWith(
      999,
      "MOONPROFILE_HOST_APP_ID=123 %command%",
    );
    expect(shortcuts["123"]).toEqual({ deck_app_id: 999, name: "My Game", is_steam: true });
  });

  it("reuses the tracked shortcut when it still exists in the library", async () => {
    const shortcuts: GameShortcuts = { "123": { deck_app_id: 999, name: "My Game", is_steam: true } };
    (window as any).appStore.GetAppOverviewByAppID.mockReturnValue(overviewOf(999));

    const appId = await ensureGameShortcut(shortcuts, "123", "My Game", "/path/runner.py", true);

    expect(appId).toBe(999);
    expect((window as any).SteamClient.Apps.AddShortcut).not.toHaveBeenCalled();
  });

  it("recreates the shortcut if the tracked one was removed from the library", async () => {
    const shortcuts: GameShortcuts = { "123": { deck_app_id: 999, name: "My Game", is_steam: true } };
    (window as any).appStore.GetAppOverviewByAppID
      .mockReturnValueOnce(null) // check of the existing entry: it disappeared
      .mockReturnValue(overviewOf(1000)); // after recreating, it shows up with the new id
    (window as any).SteamClient.Apps.AddShortcut.mockResolvedValue(1000);

    const appId = await ensureGameShortcut(shortcuts, "123", "My Game", "/path/runner.py", true);

    expect(appId).toBe(1000);
    expect((window as any).SteamClient.Apps.AddShortcut).toHaveBeenCalledTimes(1);
    expect(shortcuts["123"].deck_app_id).toBe(1000);
  });

  it("returns null when AddShortcut fails to return a numeric appid", async () => {
    const shortcuts: GameShortcuts = {};
    (window as any).SteamClient.Apps.AddShortcut.mockResolvedValue(undefined);

    const appId = await ensureGameShortcut(shortcuts, "123", "My Game", "/path/runner.py", true);

    expect(appId).toBeNull();
    expect(shortcuts["123"]).toBeUndefined();
  });

  it("returns null when the overview never appears after creation", async () => {
    vi.useFakeTimers();
    try {
      const shortcuts: GameShortcuts = {};
      (window as any).SteamClient.Apps.AddShortcut.mockResolvedValue(999);
      (window as any).appStore.GetAppOverviewByAppID.mockReturnValue(null);

      const resultPromise = ensureGameShortcut(shortcuts, "123", "My Game", "/path/runner.py", true);
      await vi.advanceTimersByTimeAsync(20 * 250);
      const appId = await resultPromise;

      expect(appId).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("removeAllGameShortcuts", () => {
  beforeEach(() => {
    (window as any).SteamClient = { Apps: { RemoveShortcut: vi.fn() } };
  });

  it("removes every tracked shortcut from Steam", () => {
    const shortcuts: GameShortcuts = {
      "123": { deck_app_id: 999, name: "A", is_steam: true },
      "456": { deck_app_id: 1000, name: "B", is_steam: false },
    };

    removeAllGameShortcuts(shortcuts);

    expect((window as any).SteamClient.Apps.RemoveShortcut).toHaveBeenCalledWith(999);
    expect((window as any).SteamClient.Apps.RemoveShortcut).toHaveBeenCalledWith(1000);
    expect((window as any).SteamClient.Apps.RemoveShortcut).toHaveBeenCalledTimes(2);
  });
});
