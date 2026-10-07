import { invoke } from "@tauri-apps/api/core";
import { syncNativeTheme } from "./theme";
import { tick } from "svelte";

/** Reveal the native window only after data, CSS, fonts and layout have settled. */
export async function revealWindow() {
  await tick();
  await document.fonts?.ready;
  await Promise.all(
    [...document.images].map((image) => image.decode?.().catch(() => {})),
  );
  await syncNativeTheme();
  // Hidden WebViews may throttle animation frames. Flush layout without waiting
  // for visibility; native and document backgrounds already match the selected theme.
  document.documentElement.getBoundingClientRect();
  await invoke("window_ready");
}
