/** sRGB / WCAG 2 contrast. Keep raw fills separate from readable accent ink. */
export function normalizeAccent(value: unknown): string | null {
  return typeof value === "string" && /^#[\da-f]{6}$/i.test(value)
    ? value.toLowerCase()
    : null;
}
const channels = (hex: string) =>
  [1, 3, 5].map((index) => parseInt(hex.slice(index, index + 2), 16));
export function contrast(a: string, b: string): number {
  const luminance = (hex: string) => {
    const linear = channels(hex).map((channel) => {
      const v = channel / 255;
      return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
    });
    return linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722;
  };
  const x = luminance(a),
    y = luminance(b);
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}
function mix(a: string, b: string, weight: number): string {
  const other = channels(b);
  return (
    "#" +
    channels(a)
      .map((v, i) =>
        Math.round(v + (other[i] - v) * weight)
          .toString(16)
          .padStart(2, "0"),
      )
      .join("")
  );
}
export function accentPalette(color: string, mode: "light" | "dark") {
  const dark = mode === "dark";
  const endpoint = dark ? "#ffffff" : "#000000";
  const surface = mix(dark ? "#282b30" : "#ffffff", color, dark ? 0.09 : 0.07);
  // Include every shared neutral surface on which accent text/focus appears.
  const surfaces = dark
    ? [
        "#202226",
        "#24262a",
        "#282b30",
        "#2d3035",
        "#30343a",
        "#24272c",
        "#34383e",
        "#373c44",
        surface,
      ]
    : [
        "#ffffff",
        "#f4f5f3",
        "#ffffff",
        "#f0f2ef",
        "#e7ebe6",
        "#e1e8e3",
        surface,
      ];
  let ink = color;
  for (let step = 0; step <= 100; step++) {
    ink = mix(color, endpoint, step / 100);
    if (surfaces.every((background) => contrast(ink, background) >= 4.5)) break;
  }
  const foreground =
    contrast(color, "#000000") >= contrast(color, "#ffffff")
      ? "#000000"
      : "#ffffff";
  // Move hover away from its foreground so the same label remains readable.
  const hover = mix(
    color,
    foreground === "#ffffff" ? "#000000" : "#ffffff",
    0.12,
  );
  return {
    "--action-background": color,
    "--action-hover": hover,
    "--action-text": foreground,
    "--action-border": ink,
    "--accent": ink,
    "--accent-surface": surface,
    "--surface-selected": surface,
    "--control-accent": ink,
    "--focus-border": ink,
    "--focus-ring": ink + "30",
    "--storage-files": ink,
    "--storage-apps": mix(ink, dark ? "#282b30" : "#ffffff", 0.3),
    "--storage-refs": mix(ink, dark ? "#282b30" : "#ffffff", 0.6),
  };
}
