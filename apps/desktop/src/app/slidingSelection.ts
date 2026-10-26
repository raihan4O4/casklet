import { tick } from "svelte";
import { motionDuration } from "./motion";

type Selection = { key: string; selector: string };

/** One shared surface; rebase interrupted motion from its current visual position. */
export function slidingSelection(node: HTMLElement, initial: Selection) {
  let selection = initial;
  let animation: Animation | undefined;
  let positioned = false;
  let destroyed = false;
  let queued = false;
  const indicator = document.createElement("span");
  indicator.className = "selection-indicator";
  indicator.setAttribute("aria-hidden", "true");
  node.prepend(indicator);
  node.classList.add("sliding-selection");
  const preference =
    typeof matchMedia === "function"
      ? matchMedia("(prefers-reduced-motion: reduce)")
      : null;

  function place(animate: boolean) {
    const target = node.querySelector<HTMLElement>(selection.selector);
    if (!target || destroyed) return;
    const previous = positioned ? indicator.getBoundingClientRect() : null;
    const parent = node.getBoundingClientRect();
    const next = target.getBoundingClientRect();
    const left = `${next.left - parent.left - node.clientLeft + node.scrollLeft}px`;
    const top = `${next.top - parent.top - node.clientTop + node.scrollTop}px`;
    if (
      !animate &&
      positioned &&
      Math.abs(parseFloat(indicator.style.left) - parseFloat(left)) < 0.1 &&
      Math.abs(parseFloat(indicator.style.top) - parseFloat(top)) < 0.1 &&
      Math.abs(parseFloat(indicator.style.width) - next.width) < 0.1 &&
      Math.abs(parseFloat(indicator.style.height) - next.height) < 0.1
    )
      return;
    animation?.cancel();
    Object.assign(indicator.style, {
      left: `${next.left - parent.left - node.clientLeft + node.scrollLeft}px`,
      top: `${next.top - parent.top - node.clientTop + node.scrollTop}px`,
      width: `${next.width}px`,
      height: `${next.height}px`,
      opacity: "1",
    });
    const duration = animate ? motionDuration("enter") : 0;
    if (
      previous &&
      next.width &&
      next.height &&
      duration &&
      typeof indicator.animate === "function"
    ) {
      animation = indicator.animate(
        [
          {
            transform: `translate(${previous.left - next.left}px, ${previous.top - next.top}px) scale(${previous.width / next.width}, ${previous.height / next.height})`,
          },
          { transform: "none" },
        ],
        { duration, easing: "cubic-bezier(.215,.61,.355,1)" },
      );
    }
    if (animation) animation.onfinish = () => place(false);
    positioned = true;
  }
  async function schedule() {
    if (queued) return;
    queued = true;
    await tick();
    queued = false;
    if (!destroyed) place(true);
  }
  // Resize/layout changes snap to the current target, rather than replaying selection.
  const resize =
    typeof ResizeObserver === "function"
      ? new ResizeObserver(() => {
          if (animation?.playState !== "running") place(false);
        })
      : null;
  resize?.observe(node);
  node.querySelectorAll("button").forEach((button) => resize?.observe(button));
  const reset = () => place(false);
  preference?.addEventListener?.("change", reset);
  void schedule();
  void document.fonts?.ready.then(() => {
    if (!destroyed) place(false);
  });
  return {
    update(next: Selection) {
      selection = next;
      void schedule();
    },
    destroy() {
      destroyed = true;
      animation?.cancel();
      resize?.disconnect();
      preference?.removeEventListener?.("change", reset);
      indicator.remove();
      node.classList.remove("sliding-selection");
    },
  };
}
