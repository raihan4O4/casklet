import { afterEach, expect, test, vi } from "vitest";
import { get } from "svelte/store";
import { CASK_PALETTE, assignCaskColors, colorForCask } from "./caskColors";
import {
  appearanceForCask,
  caskAppearances,
  CASK_APPEARANCE_KEY,
  DEFAULT_APPEARANCE,
  saveCaskAppearance,
  validAppearance,
} from "./caskAppearance";
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(async () => {}),
  listen: vi.fn(async () => () => {}),
}));
afterEach(() => {
  localStorage.clear();
  caskAppearances.set({});
  vi.restoreAllMocks();
});
test("symbols and optional colors survive reload without changing automatic colors", async () => {
  assignCaskColors([
    { id: "cask-a", createdAtMs: 1 },
    { id: "cask-b", createdAtMs: 2 },
  ]);
  const automatic = colorForCask("cask-a");
  expect(
    saveCaskAppearance("cask-a", {
      symbol: "icon:camera",
      color: CASK_PALETTE[4],
    }),
  ).toBe(true);
  saveCaskAppearance("cask-b", { symbol: "emoji:🎨", color: null });
  expect(colorForCask("cask-a")).toBe(automatic);
  vi.resetModules();
  const restarted = await import("./caskAppearance");
  expect(restarted.appearanceForCask("cask-a")).toEqual({
    symbol: "icon:camera",
    color: CASK_PALETTE[4],
  });
  expect(restarted.appearanceForCask("cask-b")).toEqual({
    symbol: "emoji:🎨",
    color: null,
  });
});
test("legacy defaults, invalid choices and Clear keep safe appearances", () => {
  expect(appearanceForCask("existing")).toEqual(DEFAULT_APPEARANCE);
  for (const value of [
    null,
    {},
    { symbol: "icon:unknown", color: null },
    { symbol: "emoji:<script>", color: null },
    { symbol: "icon:camera", color: "url(x)" },
  ])
    expect(validAppearance(value)).toBe(false);
  saveCaskAppearance("existing", {
    symbol: "emoji:🎮",
    color: CASK_PALETTE[2],
  });
  saveCaskAppearance("existing", { ...DEFAULT_APPEARANCE });
  expect(appearanceForCask("existing")).toEqual(DEFAULT_APPEARANCE);
  expect(
    JSON.parse(localStorage.getItem(CASK_APPEARANCE_KEY)!).existing,
  ).toEqual(DEFAULT_APPEARANCE);
});
test("failed storage retains the session choice and reports persistence failure", () => {
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
    throw Error("Unavailable");
  });
  expect(
    saveCaskAppearance("session", { symbol: "icon:book", color: null }),
  ).toBe(false);
  expect(get(caskAppearances).session.symbol).toBe("icon:book");
});

test("full emoji sequences and legacy colors remain valid and durable", async () => {
  const { CASK_EMOJIS, EMOJI_GROUPS } = await import("./caskAppearance");
  expect(CASK_EMOJIS.length).toBe(3963);
  expect(new Set(CASK_EMOJIS.map((item) => item.value)).size).toBe(3963);
  expect(EMOJI_GROUPS.length).toBe(9);
  for (const value of ["🇮🇳", "👩🏽‍💻", "👨‍👩‍👧‍👦"]) {
    expect(
      saveCaskAppearance(value, { symbol: `emoji:${value}`, color: "#5c8d91" }),
    ).toBe(true);
  }
  vi.resetModules();
  const restarted = await import("./caskAppearance");
  expect(restarted.appearanceForCask("👩🏽‍💻")).toEqual({
    symbol: "emoji:👩🏽‍💻",
    color: "#5c8d91",
  });
});
