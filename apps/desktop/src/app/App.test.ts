import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/svelte";
import { afterEach, expect, test, vi } from "vitest";
import App from "./App.svelte";

const calls = vi.hoisted(() => ({
  invoke: vi.fn(),
  drop: null as
    | null
    | ((event: { payload: { type: string; paths: string[] } }) => void),
}));
vi.mock("@tauri-apps/api/core", () => calls);
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onDragDropEvent: async (handler: typeof calls.drop) => {
      calls.drop = handler;
      return () => {
        calls.drop = null;
      };
    },
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

afterEach(() => {
  cleanup();
  calls.invoke.mockReset();
  calls.drop = null;
});

test("a dropped file waits for an explicit intake mode and Add action", async () => {
  const cask = {
    version: 1,
    id: "cask-2",
    name: "Work",
    state: "ACTIVE",
    managedStorageRoot: "C:/Casklets/cask-2",
    createdAtMs: 1,
    updatedAtMs: 1,
    revision: 0,
  };
  const resource = {
    version: 1,
    id: "resource-1",
    caskId: cask.id,
    name: "notes.txt",
    path: "C:/notes.txt",
    kind: "FILE",
    management: "REFERENCED",
    containment: "EXTERNAL",
    availability: "PRESENT",
    categoryKey: "other",
    sizeBytes: 5,
    sizeMeasurement: "EXACT",
    unknownEntries: 0,
  };
  calls.invoke.mockImplementation(async (command: string) => {
    if (command === "list_casks") return [cask];
    if (command === "get_cask") return cask;
    if (command === "list_resources") return [];
    if (command === "list_categories")
      return [{ version: 1, key: "other", name: "Other", isBuiltin: true }];
    if (command === "storage_summary")
      return {
        managedLogicalBytes: 0,
        referencedLogicalBytes: 0,
        physicalBytes: null,
        reclaimableMinBytes: 0,
        reclaimableMaxBytes: 0,
        unknownEntries: 0,
      };
    if (command === "add_resource") return resource;
    throw new Error(`Unexpected command: ${command}`);
  });
  render(App);
  await fireEvent.click(await screen.findByRole("button", { name: "Work" }));
  await waitFor(() => expect(calls.drop).not.toBeNull());
  calls.drop?.({ payload: { type: "drop", paths: ["C:/notes.txt"] } });
  const add = await screen.findByRole("button", { name: "Add resource" });
  expect((add as HTMLButtonElement).disabled).toBe(true);
  expect(
    calls.invoke.mock.calls.some(([command]) => command === "add_resource"),
  ).toBe(false);
  await fireEvent.click(screen.getByLabelText("Reference at current location"));
  await fireEvent.click(add);
  await waitFor(() =>
    expect(calls.invoke).toHaveBeenCalledWith("add_resource", {
      request: {
        version: 1,
        caskId: "cask-2",
        sourcePath: "C:/notes.txt",
        mode: "reference",
        categoryKey: "other",
      },
    }),
  );
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
    if (command === "list_resources" || command === "list_categories")
      return [];
    if (command === "storage_summary")
      return {
        managedLogicalBytes: 0,
        referencedLogicalBytes: 0,
        physicalBytes: null,
        reclaimableMinBytes: 0,
        reclaimableMaxBytes: 0,
        unknownEntries: 0,
      };
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
