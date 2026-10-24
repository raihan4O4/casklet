import { invoke } from "@tauri-apps/api/core";

export type ApplicationIconSource =
  | { kind: "registry"; key: string }
  | { kind: "portable"; id: string };

// Batch mounted rows, coalesce duplicate requests, and keep extraction off list loading.
const cache = new Map<
  string,
  { expires: number; result: Promise<string | null> }
>();
const pending: {
  source: ApplicationIconSource;
  resolve: (icon: string | null) => void;
}[] = [];
let scheduled = false;
let running = false;

async function flush() {
  scheduled = false;
  if (running) return;
  running = true;
  try {
    while (pending.length) {
      const batch = pending.splice(0, 16);
      try {
        const icons = await invoke<(string | null)[]>("application_icons", {
          sources: batch.map((item) => item.source),
        });
        batch.forEach((item, index) => item.resolve(icons?.[index] ?? null));
      } catch {
        batch.forEach((item) => item.resolve(null));
      }
    }
  } finally {
    running = false;
  }
}

export function applicationIcon(
  source: ApplicationIconSource,
): Promise<string | null> {
  const key = JSON.stringify(source);
  const cached = cache.get(key);
  if (cached && cached.expires > Date.now()) return cached.result;
  const result = new Promise<string | null>((resolve) =>
    pending.push({ source, resolve }),
  );
  if (cache.size >= 1000) cache.delete(cache.keys().next().value!);
  cache.set(key, { expires: Date.now() + 5 * 60_000, result });
  if (!scheduled && !running) {
    scheduled = true;
    queueMicrotask(() => void flush());
  }
  return result;
}
