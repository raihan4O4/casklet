import { afterEach, expect, test, vi } from "vitest";
import { get } from "svelte/store";
import {
  assignCaskColors,
  caskColors,
  CASK_COLORS_KEY,
  CASK_PALETTE,
  colorForCask,
} from "./caskColors";
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(async () => {}),
  listen: vi.fn(async () => () => {}),
}));
afterEach(() => {
  localStorage.clear();
  caskColors.set({});
  vi.restoreAllMocks();
});

test("automatic colors are varied, saved and balanced after the palette repeats", async () => {
  const casks = Array.from({ length: CASK_PALETTE.length * 2 }, (_, i) => ({
    id: `cask-${i}`,
    createdAtMs: i,
  }));
  assignCaskColors(casks.slice(0, CASK_PALETTE.length));
  expect(new Set(Object.values(get(caskColors))).size).toBe(
    CASK_PALETTE.length,
  );
  const original = JSON.parse(localStorage.getItem(CASK_COLORS_KEY)!);
  assignCaskColors(casks);
  for (const cask of casks.slice(0, CASK_PALETTE.length))
    expect(colorForCask(cask.id)).toBe(original[cask.id]);
  for (const color of CASK_PALETTE)
    expect(
      Object.values(get(caskColors)).filter((value) => value === color),
    ).toHaveLength(2);
  vi.resetModules();
  const restarted = await import("./caskColors");
  expect(get(restarted.caskColors)).toEqual(
    JSON.parse(localStorage.getItem(CASK_COLORS_KEY)!),
  );
});

test("existing Casks get defaults and corrupt preferences do not prevent opening", () => {
  localStorage.setItem(CASK_COLORS_KEY, "not json");
  assignCaskColors([{ id: "old-cask", createdAtMs: 0 }]);
  expect(CASK_PALETTE).toContain(colorForCask("old-cask"));
  expect(JSON.parse(localStorage.getItem(CASK_COLORS_KEY)!)["old-cask"]).toBe(
    colorForCask("old-cask"),
  );
});

test("unavailable preference storage keeps session colors without failing library loading", () => {
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
    throw Error("Unavailable");
  });
  expect(() =>
    assignCaskColors([{ id: "session", createdAtMs: 0 }]),
  ).not.toThrow();
  expect(CASK_PALETTE).toContain(colorForCask("session"));
});
