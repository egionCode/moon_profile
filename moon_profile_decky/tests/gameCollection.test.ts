import { describe, it, expect, vi, beforeEach } from "vitest";

const getStreamingCollectionId = vi.fn();
const saveStreamingCollectionId = vi.fn();

vi.mock("../src/api", () => ({
  getStreamingCollectionId: (...args: unknown[]) => getStreamingCollectionId(...args),
  saveStreamingCollectionId: (...args: unknown[]) => saveStreamingCollectionId(...args),
}));

// Dynamic import after vi.mock (hoisted), same pattern recommended by
// vitest for mocking a module imported by the module under test.
const { addShortcutsToStreamingCollection } = await import("../src/gameCollection");

function overviewOf(appId: number): unknown {
  return { appid: appId };
}

describe("addShortcutsToStreamingCollection", () => {
  let addApps: ReturnType<typeof vi.fn>;
  let save: ReturnType<typeof vi.fn>;
  let collection: any;

  beforeEach(() => {
    vi.clearAllMocks();
    getStreamingCollectionId.mockResolvedValue(null);
    saveStreamingCollectionId.mockResolvedValue(undefined);

    addApps = vi.fn();
    save = vi.fn().mockResolvedValue(undefined);
    collection = {
      apps: { has: vi.fn().mockReturnValue(false) },
      AsDragDropCollection: () => ({ AddApps: addApps }),
      Save: save,
    };
    (window as any).collectionStore = {
      GetCollectionIDByUserTag: vi.fn().mockReturnValue(null),
      GetCollection: vi.fn().mockReturnValue(undefined),
      NewUnsavedCollection: vi.fn().mockReturnValue(collection),
    };
    (window as any).appStore = {
      GetAppOverviewByAppID: vi.fn((id: number) => overviewOf(id)),
    };
  });

  it("creates the collection with the initial apps when none exists yet", async () => {
    (window as any).collectionStore.GetCollectionIDByUserTag
      .mockReturnValueOnce(null) // resolveStreamingCollection: doesn't exist yet
      .mockReturnValue("collection-id-1"); // after Save(), to find out the real id

    const ok = await addShortcutsToStreamingCollection([111, 222]);

    expect(ok).toBe(true);
    expect((window as any).collectionStore.NewUnsavedCollection).toHaveBeenCalledWith(
      "Streaming",
      undefined,
      [overviewOf(111), overviewOf(222)],
    );
    expect(save).toHaveBeenCalled();
    expect(saveStreamingCollectionId).toHaveBeenCalledWith("collection-id-1");
  });

  it("reuses the persisted collection id instead of creating a duplicate", async () => {
    getStreamingCollectionId.mockResolvedValue("collection-id-1");
    (window as any).collectionStore.GetCollection.mockReturnValue(collection);

    const ok = await addShortcutsToStreamingCollection([111]);

    expect(ok).toBe(true);
    expect((window as any).collectionStore.GetCollection).toHaveBeenCalledWith("collection-id-1");
    expect((window as any).collectionStore.NewUnsavedCollection).not.toHaveBeenCalled();
    expect(saveStreamingCollectionId).not.toHaveBeenCalled(); // id didn't change, no need to rewrite
  });

  it("falls back to tag lookup when the persisted id points at a deleted collection", async () => {
    getStreamingCollectionId.mockResolvedValue("stale-id");
    (window as any).collectionStore.GetCollection.mockImplementation((id: string) =>
      id === "stale-id" ? undefined : collection,
    );
    (window as any).collectionStore.GetCollectionIDByUserTag.mockReturnValue("collection-id-2");

    const ok = await addShortcutsToStreamingCollection([111]);

    expect(ok).toBe(true);
    expect((window as any).collectionStore.NewUnsavedCollection).not.toHaveBeenCalled();
    expect(saveStreamingCollectionId).toHaveBeenCalledWith("collection-id-2"); // id changed, rewrite
  });

  it("skips apps already in the collection and does not call Save again", async () => {
    getStreamingCollectionId.mockResolvedValue("collection-id-1");
    (window as any).collectionStore.GetCollection.mockReturnValue(collection);
    collection.apps.has.mockReturnValue(true); // everything is already there

    const ok = await addShortcutsToStreamingCollection([111, 222]);

    expect(ok).toBe(true);
    expect(addApps).not.toHaveBeenCalled();
    expect(save).not.toHaveBeenCalled();
  });

  it("only adds the apps that are missing (dedup)", async () => {
    getStreamingCollectionId.mockResolvedValue("collection-id-1");
    (window as any).collectionStore.GetCollection.mockReturnValue(collection);
    collection.apps.has.mockImplementation((id: number) => id === 111); // 111 already there, 222 not

    const ok = await addShortcutsToStreamingCollection([111, 222]);

    expect(ok).toBe(true);
    expect(addApps).toHaveBeenCalledWith([overviewOf(222)]);
  });

  it("returns false without throwing when the collection cannot be created", async () => {
    (window as any).collectionStore.NewUnsavedCollection.mockReturnValue(undefined);

    const ok = await addShortcutsToStreamingCollection([111]);

    expect(ok).toBe(false);
    expect(saveStreamingCollectionId).not.toHaveBeenCalled();
  });
});
