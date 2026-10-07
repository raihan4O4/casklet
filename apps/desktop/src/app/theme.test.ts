import { afterEach, beforeEach, expect, test, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/svelte";
import { get } from "svelte/store";
import css from "../styles/global.css?raw";
import ThemeSettings from "./ThemeSettings.svelte";
import BrandLogo from "./BrandLogo.svelte";
import {
  initializeTheme,
  setThemePreference,
  startThemeSync,
  theme,
  THEME_KEY,
} from "./theme";
import { clearNotices, notifications } from "./notifications";
import { revealWindow } from "./windowReady";

const native = vi.hoisted(() => ({
  invoke: vi.fn(async () => {}),
  emit: vi.fn(async (name: string) => {
    if (!/^[\w:/-]+$/.test(name)) throw new Error("Invalid native event name");
  }),
  listener: null as null | ((event: { payload: unknown }) => void),
  unlisten: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
vi.mock("@tauri-apps/api/event", () => ({
  emit: native.emit,
  listen: vi.fn(async (name: string, listener: typeof native.listener) => {
    if (!/^[\w:/-]+$/.test(name)) throw new Error("Invalid native event name");
    native.listener = listener;
    return native.unlisten;
  }),
}));
let dispose: (() => void) | undefined;
beforeEach(() => {
  localStorage.clear();
  initializeTheme();
  native.invoke.mockClear();
  native.emit.mockClear();
  native.unlisten.mockClear();
});
afterEach(() => {
  dispose?.();
  dispose = undefined;
  cleanup();
  clearNotices();
  vi.restoreAllMocks();
  localStorage.clear();
  initializeTheme();
});

test("Settings saves theme, updates both brand treatments and native appearance", async () => {
  render(ThemeSettings);
  render(BrandLogo);
  expect(screen.getByRole("img").getAttribute("src")).toContain(
    "casklet-logo-black",
  );
  await fireEvent.click(screen.getByRole("radio", { name: "Dark" }));
  await waitFor(() =>
    expect(native.emit).toHaveBeenCalledWith("appearance-changed", "dark"),
  );
  expect(localStorage.getItem(THEME_KEY)).toBe("dark");
  expect(document.documentElement.dataset.theme).toBe("dark");
  expect(screen.getByRole("img").getAttribute("src")).toContain(
    "casklet-logo-white",
  );
  expect(native.invoke).toHaveBeenCalledWith("set_window_theme", {
    theme: "dark",
  });
  await fireEvent.click(screen.getByRole("radio", { name: "Light" }));
  await waitFor(() => expect(get(theme)).toBe("light"));
  expect(screen.getByRole("img").getAttribute("src")).toContain(
    "casklet-logo-black",
  );
});

test("saved theme restores before rendering and invalid values fall back to light", () => {
  localStorage.setItem(THEME_KEY, "dark");
  initializeTheme();
  expect(get(theme)).toBe("dark");
  localStorage.setItem(THEME_KEY, "invalid");
  initializeTheme();
  expect(get(theme)).toBe("light");
});

test("open windows accept valid theme events and storage changes without echoing", async () => {
  dispose = startThemeSync();
  await Promise.resolve();
  native.listener?.({ payload: "dark" });
  await waitFor(() =>
    expect(native.invoke).toHaveBeenCalledWith("set_window_theme", {
      theme: "dark",
    }),
  );
  native.listener?.({ payload: "unknown" });
  expect(get(theme)).toBe("dark");
  window.dispatchEvent(
    new StorageEvent("storage", { key: THEME_KEY, newValue: "light" }),
  );
  await waitFor(() => expect(get(theme)).toBe("light"));
  expect(native.emit).not.toHaveBeenCalled();
  dispose();
  dispose = undefined;
  expect(native.unlisten).toHaveBeenCalled();
  window.dispatchEvent(
    new StorageEvent("storage", { key: THEME_KEY, newValue: "dark" }),
  );
  expect(get(theme)).toBe("light");
});

test("rapid changes keep the latest theme and cannot broadcast a stale choice", async () => {
  const dark = setThemePreference("dark");
  const light = setThemePreference("light");
  await Promise.all([dark, light]);
  expect(get(theme)).toBe("light");
  expect(localStorage.getItem(THEME_KEY)).toBe("light");
  expect(native.emit).not.toHaveBeenCalledWith("appearance-changed", "dark");
  expect(native.invoke).toHaveBeenLastCalledWith("set_window_theme", {
    theme: "light",
  });
});

test("a storage failure preserves session appearance and reports that it was not saved", async () => {
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
    throw new Error("Unavailable");
  });
  await setThemePreference("dark");
  expect(get(theme)).toBe("dark");
  expect(get(notifications)[0].kind).toBe("warning");
  expect(get(notifications)[0].message).toContain("could not be saved");
});

test("native theme is applied before a saved-dark window is revealed", async () => {
  localStorage.setItem(THEME_KEY, "dark");
  initializeTheme();
  await revealWindow();
  const commands = native.invoke.mock.calls as unknown as [string, unknown][];
  expect(commands.map((call) => call[0])).toEqual([
    "set_window_theme",
    "window_ready",
  ]);
  expect(commands[0][1]).toEqual({ theme: "dark" });
});

test("dark typography, state colors, focus and control borders meet contrast targets", () => {
  const dark = css.match(/:root\[data-theme="dark"\] \{([^}]+)\}/)![1];
  const palette = Object.fromEntries(
    [...dark.matchAll(/--([\w-]+): (#[\da-f]{6});/g)].map((match) => [
      match[1],
      match[2],
    ]),
  );
  const luminance = (hex: string) => {
    const channels = [1, 3, 5]
      .map((index) => parseInt(hex.slice(index, index + 2), 16) / 255)
      .map((value) =>
        value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4,
      );
    return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
  };
  const contrast = (a: string, b: string) => {
    const values = [luminance(a), luminance(b)].sort((x, y) => y - x);
    return (values[0] + 0.05) / (values[1] + 0.05);
  };
  for (const surface of [
    "app-background",
    "surface-panel",
    "surface-secondary",
    "surface-floating",
    "surface-selected",
  ]) {
    for (const text of [
      "text-heading",
      "text-primary",
      "text-secondary",
      "text-muted",
    ])
      expect(
        contrast(palette[text], palette[surface]),
        `${text} on ${surface}`,
      ).toBeGreaterThanOrEqual(4.5);
  }
  for (const state of ["danger", "warning", "success"])
    expect(
      contrast(palette[`${state}-text`], palette[`${state}-surface`]),
      state,
    ).toBeGreaterThanOrEqual(4.5);
  expect(
    contrast(palette["disabled-text"], palette["disabled-surface"]),
  ).toBeGreaterThanOrEqual(4.5);
  expect(
    contrast(palette["text-placeholder"], palette["surface-input"]),
  ).toBeGreaterThanOrEqual(4.5);
  expect(
    contrast(palette["action-text"], palette["action-background"]),
  ).toBeGreaterThanOrEqual(4.5);
  expect(
    contrast(palette["focus-border"], palette["surface-input"]),
  ).toBeGreaterThanOrEqual(3);
  expect(
    contrast(palette["border-control"], palette["surface-input"]),
  ).toBeGreaterThanOrEqual(3);
});
