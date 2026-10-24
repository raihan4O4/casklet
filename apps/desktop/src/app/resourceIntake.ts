import { invoke } from "@tauri-apps/api/core";
import type { ResourceView } from "../contracts/generated/casklet";
import { CONTRACT_VERSION } from "../contracts/generated/casklet";

export async function addResourceBatch(options: {
  paths: string[];
  caskId: string;
  owned: boolean;
  categoryKey: string;
  existingIds: string[];
  onProgress: (done: number, total: number) => void;
}) {
  const known = new Set(options.existingIds);
  const seen = new Set<string>();
  const result = {
    added: 0,
    skipped: 0,
    failures: [] as { path: string; reason: string }[],
  };
  for (const [index, path] of options.paths.entries()) {
    options.onProgress(index + 1, options.paths.length);
    const normalized = path
      .replaceAll("/", "\\")
      .replace(/^\\\\\?\\/, "")
      .toLocaleLowerCase();
    if (seen.has(normalized)) {
      result.skipped++;
      continue;
    }
    seen.add(normalized);
    if (!path.trim() || path.includes("\0")) {
      result.failures.push({
        path,
        reason: "Choose a valid file or folder path.",
      });
      continue;
    }
    try {
      const resource = await invoke<ResourceView>(
        options.owned ? "adopt_resource" : "add_resource",
        {
          request: {
            version: CONTRACT_VERSION,
            caskId: options.caskId,
            sourcePath: path,
            categoryKey: options.categoryKey,
            ...(options.owned
              ? { confirmOwnership: true }
              : { mode: "reference" }),
          },
        },
      );
      if (known.has(resource.id)) result.skipped++;
      else {
        result.added++;
        known.add(resource.id);
      }
    } catch (cause) {
      const reason =
        typeof cause === "object" && cause && "message" in cause
          ? String(cause.message)
          : String(cause);
      result.failures.push({ path, reason });
    }
  }
  return result;
}
