import { invoke } from "@tauri-apps/api/core";
import { tick } from "svelte";

/** Reveal the native window only after data, CSS, fonts and layout have settled. */
export async function revealWindow() {
  await tick();
  await document.fonts?.ready;
  // Hidden WebViews may throttle animation frames. Flush layout without waiting
  // for visibility; native and document backgrounds are already explicitly white.
  document.documentElement.getBoundingClientRect();
  await invoke("window_ready");
}
