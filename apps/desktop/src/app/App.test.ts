import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/svelte";
import { afterEach, expect, test, vi } from "vitest";
import App from "./App.svelte";

const calls = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => calls);

afterEach(() => {
  cleanup();
  calls.invoke.mockReset();
});

test("creates and inspects a Cask through typed Tauri commands", async () => {
  const cask = {
    version: 1,
    id: "cask-1",
    name: "Research",
    state: "ACTIVE",
    managedStorageRoot: "C:/Casklets/cask-1",
    createdAtMs: 1,
    updatedAtMs: 1,
    revision: 0,
  };
  let items: (typeof cask)[] = [];
  calls.invoke.mockImplementation(async (command: string) => {
    if (command === "list_casks") return items;
    if (command === "create_cask") {
      items = [cask];
      return cask;
    }
    if (command === "get_cask") return cask;
    throw new Error(`Unexpected command: ${command}`);
  });
  render(App);
  await screen.findByText("No Casks yet.");
  await fireEvent.input(screen.getByLabelText("New Cask name"), {
    target: { value: "Research" },
  });
  await fireEvent.click(screen.getByRole("button", { name: "Create Cask" }));
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Research" })).toBeTruthy(),
  );
  await fireEvent.click(screen.getByRole("button", { name: "Research" }));
  await waitFor(() =>
    expect(calls.invoke).toHaveBeenCalledWith("get_cask", {
      request: { version: 1, id: "cask-1" },
    }),
  );
  expect(screen.getByText("C:/Casklets/cask-1")).toBeTruthy();
});
