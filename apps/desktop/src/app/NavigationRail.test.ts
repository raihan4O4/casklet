import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import NavigationRail from "./NavigationRail.svelte";

afterEach(cleanup);
const items = [
  { key: "casks", title: "Casks", icon: "casks" as const },
  { key: "activity", title: "Activity", icon: "activity" as const },
];

test("navigation names the current destination and routes click and keyboard to the same action", async () => {
  const onselect = vi.fn();
  render(NavigationRail, {
    items,
    current: "casks",
    label: "Main sections",
    onselect,
  });
  expect(
    screen.getByRole("button", { name: "Casks" }).getAttribute("aria-current"),
  ).toBe("page");
  await fireEvent.click(screen.getByRole("button", { name: "Activity" }));
  await fireEvent.keyDown(window, { key: "1", altKey: true });
  expect(onselect.mock.calls).toEqual([["activity"], ["casks"]]);
});

test("navigation shortcuts cannot dismiss or navigate behind an open modal", async () => {
  const onselect = vi.fn();
  render(NavigationRail, {
    items,
    current: "casks",
    label: "Main sections",
    onselect,
  });
  const dialog = document.createElement("dialog");
  dialog.setAttribute("open", "");
  document.body.append(dialog);
  try {
    await fireEvent.keyDown(window, { key: "2", altKey: true });
    expect(onselect).not.toHaveBeenCalled();
  } finally {
    dialog.remove();
  }
  await fireEvent.keyDown(window, { key: "2" });
  await fireEvent.keyDown(window, { key: "8", altKey: true });
  expect(onselect).not.toHaveBeenCalled();
});
