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
import workspaceCss from "../styles/workspace.css?raw";
import ThemeSettings from "./ThemeSettings.svelte";
import BrandLogo from "./BrandLogo.svelte";
import {
  initializeTheme,
  setThemePreference,
  startThemeSync,
  theme,
  THEME_KEY,
  ACCENT_KEY,
  accent,
  setAccentPreference,
} from "./theme";
import { clearNotices, notifications } from "./notifications";
import { revealWindow } from "./windowReady";

const native = vi.hoisted(() => ({
  invoke: vi.fn(async () => {}),
  emit: vi.fn(async (name: string) => {
    if (!/^[\w:/-]+$/.test(name)) throw new Error("Invalid native event name");
  }),
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  listener: null as null | ((event: { payload: unknown }) => void),
  unlisten: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
vi.mock("@tauri-apps/api/event", () => ({
  emit: native.emit,
  listen: vi.fn(async (name: string, listener: typeof native.listener) => {
    if (!/^[\w:/-]+$/.test(name)) throw new Error("Invalid native event name");
    if (listener) native.listeners.set(name, listener);
    if (name === "appearance-changed") native.listener = listener;
    return native.unlisten;
  }),
}));
let dispose: (() => void) | undefined;
beforeEach(() => {
  localStorage.clear();
  initializeTheme();
  native.listeners.clear();
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
  const dark = [css, workspaceCss]
    .map((source) => source.match(/:root\[data-theme="dark"\] \{([^}]+)\}/)![1])
    .join("\n");
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

test("accent picker previews, persists and validates HEX without a reset action", async () => {
  render(ThemeSettings);
  await fireEvent.click(screen.getByRole("button", { name: "Iris" }));
  expect(get(accent)).toBe("#7357d8");
  expect(localStorage.getItem(ACCENT_KEY)).toBe("#7357d8");
  expect(
    document.documentElement.style.getPropertyValue("--action-background"),
  ).toBe("#7357d8");
  await fireEvent.input(screen.getByLabelText("Custom accent color"), {
    target: { value: "#ffffff" },
  });
  expect(document.documentElement.style.getPropertyValue("--action-text")).toBe(
    "#000000",
  );
  const hex = screen.getByRole("textbox", { name: "Accent hex color" });
  await fireEvent.input(hex, { target: { value: "invalid" } });
  await fireEvent.change(hex);
  expect(hex.getAttribute("aria-invalid")).toBe("true");
  expect(get(accent)).toBe("#ffffff");
  await fireEvent.input(hex, { target: { value: "#000000" } });
  expect(get(accent)).toBe("#000000");
  await setThemePreference("dark");
  expect(
    document.documentElement.style.getPropertyValue("--action-background"),
  ).toBe("#000000");
  expect(document.documentElement.style.getPropertyValue("--action-text")).toBe(
    "#ffffff",
  );
  initializeTheme();
  expect(get(accent)).toBe("#000000");
  expect(screen.queryByRole("button", { name: "Reset to default" })).toBeNull();
  expect(localStorage.getItem(ACCENT_KEY)).toBe("#000000");
});

test("accent sync accepts valid native/storage changes, resets and disposes without echoes", async () => {
  dispose = startThemeSync();
  await Promise.resolve();
  native.listeners.get("accent-changed")?.({ payload: "#FFAA00" });
  expect(get(accent)).toBe("#ffaa00");
  native.listeners.get("accent-changed")?.({ payload: "red;display:none" });
  expect(get(accent)).toBe("#ffaa00");
  window.dispatchEvent(
    new StorageEvent("storage", { key: ACCENT_KEY, newValue: "#0000ff" }),
  );
  expect(get(accent)).toBe("#0000ff");
  window.dispatchEvent(new StorageEvent("storage", { key: null }));
  expect(get(accent)).toBeNull();
  expect(native.emit).not.toHaveBeenCalled();
  dispose();
  dispose = undefined;
  native.listeners.get("accent-changed")?.({ payload: "#ffaa00" });
  expect(get(accent)).toBeNull();
});

test("invalid saved accents cannot inject CSS and storage failure keeps the session color", async () => {
  localStorage.setItem(ACCENT_KEY, "#fff; color:red");
  initializeTheme();
  expect(get(accent)).toBeNull();
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
    throw Error("Unavailable");
  });
  await setAccentPreference("#123456");
  expect(get(accent)).toBe("#123456");
  expect(get(notifications)[0].message).toContain("could not be saved");
  await setAccentPreference("bad");
  expect(get(accent)).toBe("#123456");
});

test("rapid accent changes broadcast the latest choice and reset restores theme defaults", async () => {
  await Promise.all([
    setAccentPreference("#ffffff"),
    setAccentPreference("#000000"),
    setAccentPreference("#7357d8"),
  ]);
  expect(native.emit).toHaveBeenCalledWith("accent-changed", "#7357d8");
  expect(native.emit).not.toHaveBeenCalledWith("accent-changed", "#ffffff");
  expect(native.emit).not.toHaveBeenCalledWith("accent-changed", "#000000");
  expect(get(accent)).toBe("#7357d8");
  await setAccentPreference(null);
  expect(native.emit).toHaveBeenLastCalledWith("accent-changed", null);
  await setThemePreference("dark");
  expect(
    document.documentElement.style.getPropertyValue("--action-background"),
  ).toBe("");
});
