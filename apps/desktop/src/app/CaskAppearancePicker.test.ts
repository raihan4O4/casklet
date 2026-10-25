import { afterEach, expect, test, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/svelte";
import CreateCaskForm from "./CreateCaskForm.svelte";
import CaskAppearanceSettings from "./CaskAppearanceSettings.svelte";
import CaskBadge from "./CaskBadge.svelte";
import {
  caskAppearances,
  appearanceForCask,
  DEFAULT_APPEARANCE,
  CASK_APPEARANCE_KEY,
} from "./caskAppearance";
import { CASK_PALETTE } from "./caskColors";
const native = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(async () => {}),
  listen: vi.fn(async () => () => {}),
}));
afterEach(() => {
  cleanup();
  localStorage.clear();
  caskAppearances.set({});
  native.invoke.mockReset();
});
const cask = { id: "created", name: "Photography", createdAtMs: 1 };
test("creation saves its preview by returned stable ID without changing the core request", async () => {
  native.invoke.mockResolvedValue(cask);
  const onCreated = vi.fn(async () => {});
  render(CreateCaskForm, { onCreated, onCancel: vi.fn() });
  await fireEvent.input(screen.getByRole("textbox", { name: "Cask name" }), {
    target: { value: "Photography" },
  });
  await fireEvent.click(
    screen.getByRole("button", { name: "Choose Cask appearance" }),
  );
  await fireEvent.click(
    screen.getByRole("button", { name: "Photography icon" }),
  );
  await fireEvent.click(screen.getByRole("button", { name: "Rose color" }));
  expect(
    document.querySelector(
      '.appearance-trigger [data-cask-symbol="icon:camera"]',
    ),
  ).toBeTruthy();
  await fireEvent.click(screen.getByRole("button", { name: "Create Cask" }));
  await waitFor(() => expect(onCreated).toHaveBeenCalledWith(cask));
  expect(native.invoke).toHaveBeenCalledWith("create_cask", {
    request: { version: 1, name: "Photography" },
  });
  expect(
    JSON.parse(localStorage.getItem(CASK_APPEARANCE_KEY)!).created,
  ).toEqual({ symbol: "icon:camera", color: CASK_PALETTE[4] });
});
test("emoji search and Clear update the preview without creating a Cask", async () => {
  render(CreateCaskForm, {
    onCreated: vi.fn(async () => {}),
    onCancel: vi.fn(),
  });
  await fireEvent.click(
    screen.getByRole("button", { name: "Choose Cask appearance" }),
  );
  await fireEvent.click(screen.getByRole("button", { name: "Emoji" }));
  const search = screen.getByRole("textbox", { name: "Search emojis" });
  await fireEvent.input(search, { target: { value: "art" } });
  await fireEvent.click(screen.getByRole("button", { name: "Art emoji" }));
  expect(
    Array.from(
      document.querySelectorAll(".appearance-trigger [data-cask-symbol]"),
    ).find((item) => item.getAttribute("data-cask-symbol") === "emoji:🎨"),
  ).toBeTruthy();
  await fireEvent.input(search, { target: { value: "unmatched" } });
  expect(screen.getByText("No matching emojis.")).toBeTruthy();
  await fireEvent.keyDown(search, { key: "Enter" });
  expect(native.invoke).not.toHaveBeenCalled();
  await fireEvent.click(screen.getByRole("button", { name: "Clear" }));
  expect(
    document.querySelector(
      '.appearance-trigger [data-cask-symbol="icon:casks"]',
    ),
  ).toBeTruthy();
});
test("settings update shared badges only on Save and discard cancelled edits", async () => {
  render(CaskBadge, { caskId: "existing" });
  render(CaskAppearanceSettings, { caskId: "existing", name: "Existing Cask" });
  await fireEvent.click(
    screen.getByRole("button", { name: "Change appearance" }),
  );
  await fireEvent.click(screen.getByRole("button", { name: "Emoji" }));
  await fireEvent.click(screen.getByRole("button", { name: "Games emoji" }));
  await fireEvent.click(screen.getByRole("button", { name: "Teal color" }));
  expect(appearanceForCask("existing")).toEqual(DEFAULT_APPEARANCE);
  await fireEvent.click(
    screen.getByRole("button", { name: "Save appearance" }),
  );
  expect(appearanceForCask("existing")).toEqual({
    symbol: "emoji:🎮",
    color: CASK_PALETTE[5],
  });
  expect(
    Array.from(document.querySelectorAll("[data-cask-symbol]")).filter(
      (item) => item.getAttribute("data-cask-symbol") === "emoji:🎮",
    ).length,
  ).toBe(2);
  await fireEvent.click(
    screen.getByRole("button", { name: "Change appearance" }),
  );
  await fireEvent.click(screen.getByRole("button", { name: "Clear" }));
  await fireEvent.click(
    screen.getByRole("button", { name: "Cancel appearance" }),
  );
  expect(appearanceForCask("existing").symbol).toBe("emoji:🎮");
  await waitFor(() =>
    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: "Change appearance" }),
    ),
  );
});

test("full emoji catalog supports category browsing, bounded pages and exact glyph search", async () => {
  render(CreateCaskForm, {
    onCreated: vi.fn(async () => {}),
    onCancel: vi.fn(),
  });
  await fireEvent.click(
    screen.getByRole("button", { name: "Choose Cask appearance" }),
  );
  await fireEvent.click(screen.getByRole("button", { name: "Emoji" }));
  const grid = screen.getByRole("group", { name: "Cask emojis" });
  expect(grid.querySelectorAll("button").length).toBe(96);
  await fireEvent.click(
    screen.getByRole("button", { name: "Next emoji page" }),
  );
  expect(grid.querySelectorAll("button").length).toBe(96);
  await fireEvent.change(
    screen.getByRole("combobox", { name: "Emoji category" }),
    { target: { value: "Flags" } },
  );
  expect(
    (
      screen.getByRole("button", {
        name: "Previous emoji page",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  await fireEvent.input(
    screen.getByRole("textbox", { name: "Search emojis" }),
    { target: { value: "🇮🇳" } },
  );
  expect(grid.querySelectorAll("button").length).toBe(1);
  await fireEvent.click(
    screen.getByRole("button", { name: "flag: India emoji" }),
  );
  expect(document.querySelector(".appearance-trigger")?.textContent).toContain(
    "🇮🇳",
  );
});
