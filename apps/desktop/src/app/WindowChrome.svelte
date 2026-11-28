<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen } from "@tauri-apps/api/event";
  import BrandLogo from "./BrandLogo.svelte";
  let title = $state("Casklet");
  let error = $state("");
  async function action(kind: "close" | "minimize") {
    try {
      await getCurrentWindow()[kind]();
    } catch {
      error = `Unable to ${kind} window.`;
    }
  }
  onMount(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const refresh = () =>
      void getCurrentWindow()
        .title()
        .then((value) => {
          if (!disposed) title = value;
        })
        .catch(() => {});
    refresh();
    void listen("casks-changed", refresh).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  });
</script>

<div class="window-chrome" data-tauri-drag-region>
  <div class="window-caption" data-tauri-drag-region>
    <BrandLogo alt="" /><span data-tauri-drag-region>{title}</span>
  </div>
  <div class="window-controls">
    <button
      type="button"
      aria-label="Minimize window"
      onclick={() => void action("minimize")}
      ><svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"
        ><path d="M1 6h10" stroke="currentColor" /></svg
      ></button
    >
    <button
      type="button"
      class="window-close"
      aria-label="Close window"
      onclick={() => void action("close")}
      ><svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"
        ><path d="m1 1 10 10M11 1 1 11" stroke="currentColor" /></svg
      ></button
    >
  </div>
</div>
{#if error}<div class="window-error" role="alert">{error}</div>{/if}
