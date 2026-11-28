import { writable } from "svelte/store";

export type NoticeKind = "success" | "error" | "warning" | "info";
export type Notice = {
  id: number;
  message: string;
  kind: NoticeKind;
  key?: string;
};
export const notifications = writable<Notice[]>([]);
let sequence = 0;
const timers = new Map<number, ReturnType<typeof setTimeout>>();

export function dismissNotice(id: number) {
  clearTimeout(timers.get(id));
  timers.delete(id);
  notifications.update((items) => items.filter((item) => item.id !== id));
}

export function notify(
  message: string,
  kind: NoticeKind = "success",
  key?: string,
) {
  let id = ++sequence;
  notifications.update((items) => {
    const existing = key ? items.find((item) => item.key === key) : undefined;
    if (existing) {
      id = existing.id;
      clearTimeout(timers.get(id));
    }
    const next = [
      ...items.filter((item) => item.id !== id),
      { id, message, kind, key },
    ];
    const removed = next.length > 2 ? next.shift() : undefined;
    if (removed) {
      clearTimeout(timers.get(removed.id));
      timers.delete(removed.id);
    }
    return next;
  });
  timers.set(
    id,
    setTimeout(() => dismissNotice(id), kind === "error" ? 8000 : 4500),
  );
  return id;
}

export function clearNotices() {
  for (const timer of timers.values()) clearTimeout(timer);
  timers.clear();
  notifications.set([]);
}
