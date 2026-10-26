<script lang="ts" module>
  import { writable } from "svelte/store";
  const activeMenu = writable<string | null>(null);
</script>

<script lang="ts">
  import { onDestroy, type Snippet } from "svelte";
  import { get } from "svelte/store";
  import { floatingMotion } from "./motion";
  import { fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  let {
    id,
    label,
    children,
  }: {
    id: string;
    label: string;
    children: Snippet;
  } = $props();
  let root: HTMLDivElement;
  let trigger: HTMLButtonElement;
  let menu = $state<HTMLDivElement>(null!);
  const isOpen = $derived($activeMenu === id);

  onDestroy(() => {
    if (get(activeMenu) === id) activeMenu.set(null);
  });
  function close(focus = false) {
    activeMenu.set(null);
    if (focus) trigger?.focus();
  }
  $effect(() => {
    if (!isOpen) return;
    const outside = (event: Event) => {
      if (event.target instanceof Node && !root.contains(event.target)) close();
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        close(true);
      }
    };
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("focusin", outside);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("focusin", outside);
      document.removeEventListener("keydown", escape);
    };
  });
  function navigate(event: KeyboardEvent) {
    const items = [
      ...menu.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"),
    ];
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      const next =
        event.key === "Home"
          ? 0
          : event.key === "End"
            ? items.length - 1
            : (current + (event.key === "ArrowDown" ? 1 : -1) + items.length) %
              items.length;
      items[next]?.focus();
    }
  }
</script>

<div bind:this={root} class="popup-root">
  <button
    bind:this={trigger}
    class="icon-button popup-trigger"
    type="button"
    aria-label={label}
    aria-expanded={isOpen}
    aria-haspopup="menu"
    aria-controls={`${id}-menu`}
    onclick={() => activeMenu.set(isOpen ? null : id)}
    onkeydown={(event) => {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        activeMenu.set(id);
        queueMicrotask(() =>
          menu?.querySelector<HTMLButtonElement>("button")?.focus(),
        );
      }
    }}><Icon name="more" size={18} /></button
  >
  {#if isOpen}
    <div
      bind:this={menu}
      id={`${id}-menu`}
      class="menu popup-menu"
      role="menu"
      aria-hidden={!isOpen}
      inert={!isOpen}
      tabindex="-1"
      aria-label={label}
      onkeydown={navigate}
      onclick={(event) => {
        if (event.target instanceof Element && event.target.closest("button"))
          close(true);
      }}
      in:fly={floatingMotion("enter")}
      out:fly={floatingMotion("exit")}
    >
      {@render children()}
    </div>
  {/if}
</div>
