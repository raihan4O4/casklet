import { expect, test, vi } from "vitest";
import { motionDuration } from "./motion";

test("motion durations accept minified seconds and milliseconds and respect live reduced motion", () => {
  const original = Object.getOwnPropertyDescriptor(
    Element.prototype,
    "animate",
  );
  const media = vi.fn(() => ({ matches: false }));
  vi.stubGlobal("matchMedia", media);
  Object.defineProperty(Element.prototype, "animate", {
    configurable: true,
    value: vi.fn(),
  });
  try {
    document.documentElement.style.setProperty("--motion-enter", "200ms");
    expect(motionDuration("enter")).toBe(200);
    document.documentElement.style.setProperty("--motion-enter", ".2s");
    expect(motionDuration("enter")).toBe(200);
    media.mockReturnValue({ matches: true });
    expect(motionDuration("enter")).toBe(0);
  } finally {
    document.documentElement.style.removeProperty("--motion-enter");
    if (original) Object.defineProperty(Element.prototype, "animate", original);
    else Reflect.deleteProperty(Element.prototype, "animate");
    vi.unstubAllGlobals();
  }
});
