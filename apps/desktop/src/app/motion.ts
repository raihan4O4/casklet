import { flip } from "svelte/animate";
import { cubicOut } from "svelte/easing";
import { fade } from "svelte/transition";

/** Read at interaction time so changing the OS preference takes effect immediately. */
export function motionDuration(
  kind: "fast" | "normal" | "enter" | "exit" = "normal",
) {
  if (
    typeof matchMedia !== "undefined" &&
    matchMedia("(prefers-reduced-motion: reduce)").matches
  )
    return 0;
  const fallback = { fast: 120, normal: 180, enter: 200, exit: 120 };
  if (typeof document === "undefined") return fallback[kind];
  if (typeof Element.prototype.animate !== "function") return 0;
  const token = getComputedStyle(document.documentElement)
    .getPropertyValue(`--motion-${kind}`)
    .trim();
  const value =
    parseFloat(token) *
    (token.endsWith("ms") ? 1 : token.endsWith("s") ? 1000 : 1);
  return Number.isFinite(value) ? value : fallback[kind];
}

export function surfaceFade(
  node: Element,
  _params: unknown,
  options: { direction: "in" | "out" | "both" },
) {
  return fade(node, {
    duration: motionDuration(options.direction === "out" ? "exit" : "enter"),
    easing: cubicOut,
  });
}

export function floatingMotion(kind: "enter" | "exit") {
  return {
    y: motionDuration(kind) ? (kind === "enter" ? -3 : -2) : 0,
    duration: motionDuration(kind),
    easing: cubicOut,
  };
}

/** Keep identities continuous during sorting; never scale text during a view change. */
export function reorderMotion(
  node: Element,
  rectangles: { from: DOMRect; to: DOMRect },
) {
  const sameSize =
    Math.abs(rectangles.from.width - rectangles.to.width) < 1 &&
    Math.abs(rectangles.from.height - rectangles.to.height) < 1;
  return flip(node, rectangles, {
    duration: sameSize ? motionDuration("normal") : 0,
    easing: cubicOut,
  });
}
