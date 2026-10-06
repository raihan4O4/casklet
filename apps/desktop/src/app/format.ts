export function bytes(value: number | null | undefined): string {
  if (value == null) return "Not measured";
  if (value === 0) return "0 B";
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), 4);
  return `${(value / 1024 ** index).toLocaleString(undefined, { maximumFractionDigits: index === 0 ? 0 : 1 })} ${["B", "KB", "MB", "GB", "TB"][index]}`;
}
export function relativeTime(timestamp: number): string {
  const days = Math.max(0, Math.floor((Date.now() - timestamp) / 86400000));
  if (days === 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days < 7) return `${days} days ago`;
  if (days < 30)
    return `${Math.floor(days / 7)} ${days < 14 ? "week" : "weeks"} ago`;
  return new Date(timestamp).toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}
export function label(value: string): string {
  return value
    .toLowerCase()
    .replaceAll("_", " ")
    .replace(/^./, (letter) => letter.toUpperCase());
}

/** Stable identity accents: never interpreted as ownership or health. */
export function caskColor(id: string): string {
  const colors = [
    "#5b8b6b",
    "#5d7d9e",
    "#85709c",
    "#ae8154",
    "#ae6e70",
    "#5c8d91",
  ];
  let hash = 0;
  for (const character of id)
    hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
  return colors[hash % colors.length];
}

export function activityDay(timestamp: number): string {
  const date = new Date(timestamp);
  const now = new Date();
  if (date.toDateString() === now.toDateString()) return "Today";
  now.setDate(now.getDate() - 1);
  if (date.toDateString() === now.toDateString()) return "Yesterday";
  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}
export function signatureLabel(status: string): string {
  if (status === "VALID") return "Signed";
  if (status === "UNSIGNED") return "Unsigned";
  return "Signature not verified";
}
