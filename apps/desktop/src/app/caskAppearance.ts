import { get, writable } from "svelte/store";
import { emit, listen } from "@tauri-apps/api/event";
import emojiData from "./emoji-data.json";
import { VALID_CASK_COLORS } from "./caskColors";
import { notify } from "./notifications";

export const CASK_APPEARANCE_KEY = "casklet.cask-appearance.v1";
export const CASK_ICONS = [
  { value: "casks", name: "Collection" },
  { value: "folder", name: "Folder" },
  { value: "briefcase", name: "Work" },
  { value: "book", name: "Reading" },
  { value: "graduation", name: "Study" },
  { value: "code", name: "Code" },
  { value: "terminal", name: "Terminal" },
  { value: "camera", name: "Photography" },
  { value: "image", name: "Images" },
  { value: "video", name: "Film" },
  { value: "audio", name: "Music" },
  { value: "gamepad", name: "Games" },
  { value: "flask", name: "Research" },
  { value: "heart", name: "Personal" },
  { value: "plane", name: "Travel" },
  { value: "leaf", name: "Nature" },
  { value: "document", name: "Notes" },
  { value: "archive", name: "Archive" },
  { value: "package", name: "Applications" },
  { value: "storage", name: "Storage" },
  { value: "database", name: "Data" },
  { value: "activity", name: "Activity" },
  { value: "clock", name: "Planning" },
  { value: "link", name: "Links" },
] as const;
const LEGACY_EMOJIS = [
  { value: "📁", name: "Folder" },
  { value: "💼", name: "Work" },
  { value: "📚", name: "Books" },
  { value: "🎓", name: "Study" },
  { value: "💻", name: "Code" },
  { value: "📷", name: "Photography" },
  { value: "🎨", name: "Art" },
  { value: "🎬", name: "Film" },
  { value: "🎵", name: "Music" },
  { value: "🎮", name: "Games" },
  { value: "🔬", name: "Research" },
  { value: "🚀", name: "Launch" },
  { value: "✈️", name: "Travel" },
  { value: "🌿", name: "Nature" },
  { value: "🏠", name: "Home" },
  { value: "🛠️", name: "Tools" },
  { value: "📝", name: "Notes" },
  { value: "📦", name: "Archive" },
  { value: "💡", name: "Ideas" },
  { value: "⭐", name: "Favorites" },
  { value: "☕", name: "Coffee" },
  { value: "🏋️", name: "Fitness" },
  { value: "🧭", name: "Explore" },
  { value: "🎯", name: "Goals" },
] as const;
export const CASK_EMOJIS = emojiData.map(([value, label, group]) => ({
  value,
  name: LEGACY_EMOJIS.find((item) => item.value === value)?.name ?? label,
  keywords: `${label} ${group}`,
  group,
}));
CASK_EMOJIS.sort((a, b) => {
  const rank = (value: string) => {
    const index = LEGACY_EMOJIS.findIndex((item) => item.value === value);
    return index < 0 ? LEGACY_EMOJIS.length : index;
  };
  return rank(a.value) - rank(b.value);
});
export const EMOJI_GROUPS = [...new Set(CASK_EMOJIS.map((item) => item.group))];
const emojiValues = new Set(CASK_EMOJIS.map((item) => `emoji:${item.value}`));
export type CaskSymbol =
  | `icon:${(typeof CASK_ICONS)[number]["value"]}`
  | `emoji:${(typeof CASK_EMOJIS)[number]["value"]}`;
export type CaskAppearance = { symbol: CaskSymbol; color: string | null };
export const DEFAULT_APPEARANCE: CaskAppearance = {
  symbol: "icon:casks",
  color: null,
};
type Appearances = Record<string, CaskAppearance>;
export function validAppearance(value: unknown): value is CaskAppearance {
  if (
    !value ||
    typeof value !== "object" ||
    !("symbol" in value) ||
    !("color" in value)
  )
    return false;
  return (
    (CASK_ICONS.some((item) => `icon:${item.value}` === value.symbol) ||
      emojiValues.has(String(value.symbol))) &&
    (value.color === null ||
      VALID_CASK_COLORS.some((color) => color === value.color))
  );
}
function readAppearances(): Appearances {
  try {
    const value: unknown = JSON.parse(
      localStorage.getItem(CASK_APPEARANCE_KEY) ?? "{}",
    );
    if (!value || typeof value !== "object" || Array.isArray(value)) return {};
    return Object.fromEntries(
      Object.entries(value).filter(
        ([id, appearance]) => id.length > 0 && validAppearance(appearance),
      ),
    );
  } catch {
    return {};
  }
}
export const caskAppearances = writable<Appearances>(readAppearances());
export function appearanceForCask(id: string): CaskAppearance {
  return { ...(get(caskAppearances)[id] ?? DEFAULT_APPEARANCE) };
}
/** One local appearance record, separate from operational Cask metadata. */
export function saveCaskAppearance(
  id: string,
  appearance: CaskAppearance,
): boolean {
  if (!id || !validAppearance(appearance)) return false;
  const choice = { ...appearance };
  const appearances = {
    ...get(caskAppearances),
    ...readAppearances(),
    [id]: choice,
  };
  caskAppearances.set(appearances);
  let persisted = true;
  try {
    localStorage.setItem(CASK_APPEARANCE_KEY, JSON.stringify(appearances));
  } catch {
    persisted = false;
    notify(
      "Cask appearance changed for this session. It could not be saved.",
      "warning",
    );
  }
  void emit("cask-appearance-changed", { id, appearance: choice }).catch(
    () => {},
  );
  return persisted;
}
export function startCaskAppearanceSync() {
  let disposed = false;
  let unlisten: (() => void) | undefined;
  const storage = (event: StorageEvent) => {
    if (event.key === CASK_APPEARANCE_KEY)
      caskAppearances.set(readAppearances());
  };
  window.addEventListener("storage", storage);
  void listen<{ id: string; appearance: unknown }>(
    "cask-appearance-changed",
    (event) => {
      if (
        !disposed &&
        typeof event.payload?.id === "string" &&
        validAppearance(event.payload.appearance)
      ) {
        const { id, appearance } = event.payload;
        caskAppearances.update((items) => ({
          ...items,
          [id]: { ...appearance },
        }));
      }
    },
  )
    .then((fn) => {
      if (disposed) fn();
      else {
        unlisten = fn;
        caskAppearances.update((items) => ({ ...items, ...readAppearances() }));
      }
    })
    .catch(() => {});
  return () => {
    disposed = true;
    unlisten?.();
    window.removeEventListener("storage", storage);
  };
}
