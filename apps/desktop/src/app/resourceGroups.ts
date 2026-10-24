import type {
  CategoryView,
  ResourceView,
} from "../contracts/generated/casklet";
import { bytes, label } from "./format";

const categoryIcons = {
  images: "image",
  videos: "video",
  documents: "document",
  audio: "audio",
  archives: "archive",
  applications: "package",
} as const;

export function categoryIcon(categoryKey: string, kind?: string) {
  return Object.hasOwn(categoryIcons, categoryKey)
    ? categoryIcons[categoryKey as keyof typeof categoryIcons]
    : kind === "DIRECTORY"
      ? "folder"
      : "file";
}

export type ResourceGroup = {
  key: string;
  name: string;
  items: ResourceView[];
  sizeBytes: number;
  unknown: number;
  included: number;
};
const normalizedPath = (value: string) =>
  value
    .replace(/^\\\\\?\\/, "")
    .replaceAll("\\", "/")
    .replace(/\/+$/, "")
    .toLocaleLowerCase();

/** Top-level resource snapshots only; folder descendants are never listed or scanned here. */
export function resourceGroups(
  resources: ResourceView[],
  categories: CategoryView[],
): ResourceGroup[] {
  const groups = new Map<string, ResourceGroup>();
  const paths = resources.map((item) => ({
    item,
    path: normalizedPath(item.path),
  }));
  for (const { item, path } of paths) {
    const key = item.categoryKey || "other";
    let group = groups.get(key);
    if (!group) {
      group = {
        key,
        name:
          categories.find((category) => category.key === key)?.name ??
          label(key),
        items: [],
        sizeBytes: 0,
        unknown: 0,
        included: 0,
      };
      groups.set(key, group);
    }
    group.items.push(item);
    // A separately referenced child remains visible, but its bytes belong to its containing folder.
    const covered = paths.some(
      (parent) =>
        parent.item.id !== item.id &&
        parent.item.kind === "DIRECTORY" &&
        path.startsWith(parent.path + "/"),
    );
    if (covered) {
      group.included++;
      continue;
    }
    if (item.availability !== "PRESENT" || item.sizeBytes === null)
      group.unknown++;
    else {
      group.sizeBytes += item.sizeBytes;
      group.unknown += item.unknownEntries;
    }
  }
  return [...groups.values()].sort((a, b) => a.name.localeCompare(b.name));
}

export function groupSize(group: ResourceGroup): string {
  if (group.included === group.items.length) return "Included in folder";
  if (group.unknown && !group.sizeBytes) return "Not measured";
  return `${bytes(group.sizeBytes)}${group.unknown ? " · partial" : ""}`;
}
