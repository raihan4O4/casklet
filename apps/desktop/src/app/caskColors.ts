import { get, writable } from "svelte/store";
import { emit, listen } from "@tauri-apps/api/event";
import { notify } from "./notifications";

export const CASK_COLORS_KEY = "casklet.cask-colors.v1";
const LEGACY_CASK_PALETTE = [
  "#5b8b6b",
  "#5d7d9e",
  "#85709c",
  "#ae8154",
  "#ae6e70",
  "#5c8d91",
  "#9c785c",
  "#8b8954",
  "#8875ad",
  "#537f8d",
  "#a46889",
  "#6b8560",
] as const;
export const CASK_PALETTE = [
  "#84cc16",
  "#3b82f6",
  "#a78bfa",
  "#fbbf24",
  "#fb4b85",
  "#14b8a6",
  "#ff7043",
  "#ff9f1c",
  "#8b5cf6",
  "#06b6d4",
  "#e040fb",
  "#22c55e",
] as const;
export const VALID_CASK_COLORS = [...CASK_PALETTE, ...LEGACY_CASK_PALETTE];
type Colors = Record<string, string>;
function savedColors(): Colors {
  try {
    const value: unknown = JSON.parse(
      localStorage.getItem(CASK_COLORS_KEY) ?? "{}",
    );
    if (!value || typeof value !== "object" || Array.isArray(value)) return {};
    return Object.fromEntries(
      Object.entries(value).filter(
        ([id, color]) =>
          id.length > 0 &&
          typeof color === "string" &&
          VALID_CASK_COLORS.some((item) => item === color),
      ),
    );
  } catch {
    return {};
  }
}
export const caskColors = writable<Colors>(savedColors());
/** Stable fallback when preferences are unavailable; identity is already durable. */
export function fallbackCaskColor(id: string): string {
  let hash = 0;
  for (const character of id)
    hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
  return CASK_PALETTE[hash % CASK_PALETTE.length];
}
export function colorForCask(id: string): string {
  return get(caskColors)[id] ?? fallbackCaskColor(id);
}
let warned = false;
/** Appearance only. Never modifies Cask metadata, ownership or export authority. */
export function assignCaskColors(casks: { id: string; createdAtMs: number }[]) {
  const colors = { ...get(caskColors), ...savedColors() };
  let changed = false;
  for (const cask of [...casks].sort(
    (a, b) => a.createdAtMs - b.createdAtMs || a.id.localeCompare(b.id),
  )) {
    if (Object.hasOwn(colors, cask.id)) continue;
    const counts = CASK_PALETTE.map(
      (color) => Object.values(colors).filter((item) => item === color).length,
    );
    const least = Math.min(...counts);
    const preferred = CASK_PALETTE.indexOf(
      fallbackCaskColor(cask.id) as (typeof CASK_PALETTE)[number],
    );
    let index = preferred;
    while (counts[index] !== least) index = (index + 1) % CASK_PALETTE.length;
    colors[cask.id] = CASK_PALETTE[index];
    changed = true;
  }
  caskColors.set(colors);
  if (!changed) return;
  try {
    localStorage.setItem(CASK_COLORS_KEY, JSON.stringify(colors));
  } catch {
    if (!warned)
      notify(
        "Cask colors apply for this session. Appearance preferences could not be saved.",
        "warning",
      );
    warned = true;
  }
  void emit("cask-colors-changed").catch(() => {});
}
export function startCaskColorSync() {
  let disposed = false;
  let unlisten: (() => void) | undefined;
  const reload = () =>
    caskColors.update((colors) => ({ ...colors, ...savedColors() }));
  const storage = (event: StorageEvent) => {
    if (event.key === CASK_COLORS_KEY) reload();
  };
  window.addEventListener("storage", storage);
  void listen("cask-colors-changed", reload)
    .then((fn) => {
      if (disposed) fn();
      else {
        unlisten = fn;
        reload();
      }
    })
    .catch(() => {});
  return () => {
    disposed = true;
    unlisten?.();
    window.removeEventListener("storage", storage);
  };
}
