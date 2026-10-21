import { expect, test } from "vitest";
import { accentPalette, normalizeAccent, contrast } from "./accent";
import globalCss from "../styles/global.css?raw";
import workspaceCss from "../styles/workspace.css?raw";

// Independent WCAG oracle; exercise the actual shared surface declarations.
function ratio(a: string, b: string) {
  const luminance = (hex: string) => {
    const rgb = hex
      .slice(1)
      .match(/../g)!
      .map((v) => parseInt(v, 16) / 255)
      .map((v) =>
        v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4),
      );
    return rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
  };
  const x = luminance(a),
    y = luminance(b);
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}
test("strict color validation and known WCAG reference values", () => {
  expect(normalizeAccent("#ABCDEF")).toBe("#abcdef");
  for (const value of ["#fff", "#12345678", "red", "#123456;", null, 17])
    expect(normalizeAccent(value)).toBeNull();
  expect(contrast("#000000", "#ffffff")).toBe(21);
  expect(contrast("#777777", "#ffffff")).toBeCloseTo(4.478, 3);
});
test.each(["light", "dark"] as const)(
  "%s accent palette stays readable across RGB space and every shared surface",
  (mode) => {
    const block = (source: string, dark: boolean) =>
      source.match(
        dark ? /:root\[data-theme="dark"\] \{([^}]+)\}/ : /:root \{([^}]+)\}/,
      )?.[1] ?? "";
    const tokens = Object.fromEntries(
      [
        ...[
          block(globalCss, false),
          block(workspaceCss, false),
          ...(mode === "dark"
            ? [block(globalCss, true), block(workspaceCss, true)]
            : []),
        ]
          .join("\n")
          .matchAll(/--([\w-]+): (#[\da-f]{6});/g),
      ].map((match) => [match[1], match[2]]),
    );
    for (let r = 0; r <= 255; r += 51)
      for (let g = 0; g <= 255; g += 51)
        for (let b = 0; b <= 255; b += 51) {
          const color =
            "#" +
            [r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("");
          const palette = accentPalette(color, mode);
          expect(palette["--action-background"]).toBe(color);
          for (const background of [
            palette["--action-background"],
            palette["--action-hover"],
          ])
            expect(
              ratio(palette["--action-text"], background),
              color,
            ).toBeGreaterThanOrEqual(4.5);
          for (const surface of [
            "app-background",
            "surface-navigation",
            "surface-panel",
            "surface-secondary",
            "surface-input",
            "surface-hover",
            "surface-selected",
            "surface-floating",
          ]) {
            expect(
              ratio(palette["--accent"], tokens[surface]),
              `${color} on ${surface}`,
            ).toBeGreaterThanOrEqual(4.5);
            expect(
              ratio(palette["--focus-border"], tokens[surface]),
            ).toBeGreaterThanOrEqual(3);
          }
          expect(
            ratio(palette["--accent"], palette["--accent-surface"]),
          ).toBeGreaterThanOrEqual(4.5);
          expect(
            ratio(tokens["text-primary"], palette["--accent-surface"]),
          ).toBeGreaterThanOrEqual(4.5);
        }
  },
);
