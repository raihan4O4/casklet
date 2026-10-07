import { writable, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { notify } from "./notifications";

export type Theme = "light" | "dark";
export const THEME_KEY = "casklet.appearance";
export const theme = writable<Theme>("light");
const validTheme = (value: unknown): value is Theme =>
  value === "light" || value === "dark";

function applyTheme(value: Theme) {
  document.documentElement.classList.add("theme-switching");
  document.documentElement.dataset.theme = value;
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", value === "dark" ? "#202226" : "#ffffff");
  theme.set(value);
  // Flush the new palette with transitions disabled, then restore normal hover
  // motion. Opposing foreground/background fades must not reduce readability.
  document.documentElement.getBoundingClientRect();
  document.documentElement.classList.remove("theme-switching");
}

/** Presentation preference only: Cask data and domain policy are untouched. */
export function initializeTheme() {
  let value: Theme = "light";
  try {
    const saved = localStorage.getItem(THEME_KEY);
    if (validTheme(saved)) value = saved;
  } catch {
    /* Storage may be unavailable; light remains the default. */
  }
  applyTheme(value);
}

let nativeUpdate: Promise<unknown> = Promise.resolve();
export function syncNativeTheme() {
  // Serialize native changes and read the latest choice when each update runs.
  // Rapid keyboard toggles cannot leave a stale native background behind.
  nativeUpdate = nativeUpdate
    .catch(() => {})
    .then(() => invoke("set_window_theme", { theme: get(theme) }));
  return nativeUpdate;
}

export async function setThemePreference(value: Theme) {
  if (!validTheme(value)) return;
  applyTheme(value);
  try {
    localStorage.setItem(THEME_KEY, value);
  } catch {
    notify(
      "Appearance changed for this session. The preference could not be saved.",
      "warning",
      "appearance",
    );
  }
  try {
    await syncNativeTheme();
    if (get(theme) === value) await emit("appearance-changed", value);
  } catch (cause) {
    const detail =
      typeof cause === "string"
        ? cause
        : cause instanceof Error
          ? cause.message
          : "message" in Object(cause)
            ? String((cause as { message: unknown }).message)
            : "Native window update failed";
    notify(
      `Unable to synchronize appearance: ${detail}`,
      "error",
      "appearance",
    );
  }
}

/** Tauri events update open WebViews; shared storage also covers reconnects. */
export function startThemeSync() {
  let disposed = false;
  let unlisten: (() => void) | undefined;
  const accept = (value: unknown) => {
    if (!validTheme(value) || value === get(theme) || disposed) return;
    applyTheme(value);
    void syncNativeTheme().catch(() =>
      notify(
        "Unable to update the native window appearance.",
        "error",
        "appearance",
      ),
    );
  };
  const storage = (event: StorageEvent) => {
    if (event.key === THEME_KEY) accept(event.newValue ?? "light");
    else if (event.key === null) accept("light");
  };
  window.addEventListener("storage", storage);
  void listen<Theme>("appearance-changed", (event) => accept(event.payload))
    .then((fn) => {
      if (disposed) fn();
      else {
        unlisten = fn;
        // Reconcile a preference changed while the native listener was attaching.
        try {
          accept(localStorage.getItem(THEME_KEY));
        } catch {
          /* Keep the current session theme. */
        }
      }
    })
    .catch(() =>
      notify(
        "Appearance synchronization is unavailable.",
        "warning",
        "appearance",
      ),
    );
  return () => {
    disposed = true;
    window.removeEventListener("storage", storage);
    unlisten?.();
  };
}
