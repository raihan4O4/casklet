import { expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type { ResourceView } from "../contracts/generated/casklet";
import { resourceGroups, groupSize, categoryIcon } from "./resourceGroups";
import { addResourceBatch } from "./resourceIntake";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
test("category glyphs follow assigned keys with neutral custom and folder fallbacks", () => {
  expect(
    ["images", "videos", "documents", "audio", "archives", "applications"].map(
      (key) => categoryIcon(key),
    ),
  ).toEqual(["image", "video", "document", "audio", "archive", "package"]);
  expect(categoryIcon("images", "DIRECTORY")).toBe("image");
  expect(categoryIcon("custom-images", "FILE")).toBe("file");
  expect(categoryIcon("other", "DIRECTORY")).toBe("folder");
  expect(categoryIcon("toString")).toBe("file");
});
const resource = (
  id: string,
  categoryKey: string,
  sizeBytes: number | null,
  path = `C:/data/${id}`,
): ResourceView => ({
  version: 1,
  id,
  caskId: "cask",
  name: id,
  path,
  categoryKey,
  sizeBytes,
  kind: "FILE",
  management: "REFERENCED",
  shared: false,
  containment: "EXTERNAL",
  availability: "PRESENT",
  sizeMeasurement: "LOGICAL",
  unknownEntries: 0,
  ownership: "REFERENCED",
  provenance: "USER_REFERENCE",
});

test("category groups contain only used categories and react to recategorization/removal", () => {
  const items = [
    resource("photo", "images", 100),
    resource("video", "videos", 200),
    resource("second", "images", 50),
  ];
  const groups = resourceGroups(items, []);
  expect(
    groups.map((group) => [group.key, group.items.length, group.sizeBytes]),
  ).toEqual([
    ["images", 2, 150],
    ["videos", 1, 200],
  ]);
  items[0].categoryKey = "videos";
  expect(resourceGroups(items, []).map((group) => group.sizeBytes)).toEqual([
    50, 300,
  ]);
  expect(
    resourceGroups(
      items.filter((item) => item.categoryKey !== "images"),
      [],
    ),
  ).toHaveLength(1);
});
test("nested resources remain visible without counting folder bytes twice, unknown is not zero", () => {
  const folder = {
    ...resource("folder", "projects", 1000, "C:/data/project"),
    kind: "DIRECTORY",
  };
  const groups = resourceGroups(
    [
      folder,
      resource("child", "images", 100, "C:/data/project/photo.png"),
      resource("unknown", "", null),
    ],
    [],
  );
  expect(groups.reduce((sum, group) => sum + group.sizeBytes, 0)).toBe(1000);
  expect(groupSize(groups.find((group) => group.key === "images")!)).toBe(
    "Included in folder",
  );
  expect(groupSize(groups.find((group) => group.key === "other")!)).toBe(
    "Not measured",
  );
});
test("multi-item intake continues after failure, skips duplicate identities and preserves per-item auto category", async () => {
  vi.mocked(invoke).mockImplementation(async (_command, args) => {
    const path = (args as { request: { sourcePath: string } }).request
      .sourcePath;
    if (path.includes("missing")) throw { message: "File is unavailable" };
    return resource(path.endsWith("old.txt") ? "existing" : path, "images", 4);
  });
  const progress = vi.fn();
  const result = await addResourceBatch({
    paths: [
      "C:/image.png",
      "C:/missing.txt",
      "C:/old.txt",
      "C:/Folder",
      "C:/image.png",
    ],
    caskId: "cask",
    owned: false,
    categoryKey: "auto",
    existingIds: ["existing"],
    onProgress: progress,
  });
  expect(result).toEqual({
    added: 2,
    skipped: 2,
    failures: [{ path: "C:/missing.txt", reason: "File is unavailable" }],
  });
  expect(progress).toHaveBeenLastCalledWith(5, 5);
  expect(
    vi
      .mocked(invoke)
      .mock.calls.every(
        ([command, args]) =>
          command === "add_resource" &&
          (args as { request: { categoryKey: string; mode: string } }).request
            .categoryKey === "auto" &&
          (args as { request: { mode: string } }).request.mode === "reference",
      ),
  ).toBe(true);
});
