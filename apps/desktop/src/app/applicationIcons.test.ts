import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/svelte";
import { invoke } from "@tauri-apps/api/core";
import ApplicationIcon from "./ApplicationIcon.svelte";
import { applicationIcon } from "./applicationIcons";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
const png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB";

describe("application icons", () => {
  it("batches requests, coalesces duplicates, and caches results", async () => {
    vi.mocked(invoke).mockResolvedValue([png, null]);
    const source = { kind: "registry" as const, key: "batch-test" };
    const first = applicationIcon(source);
    expect(applicationIcon(source)).toBe(first);
    const missing = applicationIcon({ kind: "portable", id: "missing-test" });
    expect(await first).toBe(png);
    expect(await missing).toBeNull();
    expect(await applicationIcon(source)).toBe(png);
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("application_icons", {
      sources: [source, { kind: "portable", id: "missing-test" }],
    });
  });

  it("shows the icon and restores the fallback when decoding fails", async () => {
    vi.mocked(invoke).mockResolvedValue([png]);
    const { container } = render(ApplicationIcon, {
      source: { kind: "portable", id: "render-test" },
    });
    await waitFor(() =>
      expect(container.querySelector("img")?.getAttribute("src")).toBe(png),
    );
    expect(container.querySelector("img")?.getAttribute("alt")).toBe("");
    await fireEvent.error(container.querySelector("img")!);
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("svg")).not.toBeNull();
  });

  it("keeps the fallback after extraction errors without rejecting list loading", async () => {
    vi.mocked(invoke).mockRejectedValue(new Error("Icon helper unavailable"));
    expect(
      await applicationIcon({ kind: "registry", key: "failure-test" }),
    ).toBeNull();
  });

  it("ignores a stale icon when a row changes source", async () => {
    let finish!: (value: (string | null)[]) => void;
    vi.mocked(invoke).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const view = render(ApplicationIcon, {
      source: { kind: "portable", id: "stale-test" },
    });
    await waitFor(() => expect(finish).toBeTypeOf("function"));
    await view.rerender({ source: undefined });
    finish([png]);
    await waitFor(() => expect(view.container.querySelector("img")).toBeNull());
  });
});
