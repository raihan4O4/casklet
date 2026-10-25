<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";
  import BrandLogo from "./BrandLogo.svelte";

  export type NavigationItem = {
    key: string;
    title: string;
    icon:
      | "casks"
      | "installations"
      | "activity"
      | "settings"
      | "folder"
      | "storage";
    badge?: number;
  };
  let {
    items,
    current,
    label,
    onselect,
    footer,
    compact = false,
  }: {
    items: NavigationItem[];
    current: string;
    label: string;
    onselect: (key: string) => void;
    footer?: Snippet;
    compact?: boolean;
  } = $props();
  function shortcut(event: KeyboardEvent) {
    if (
      !event.altKey ||
      event.ctrlKey ||
      event.metaKey ||
      event.shiftKey ||
      event.repeat ||
      document.querySelector("dialog[open]")
    )
      return;
    const index = Number(event.key) - 1;
    if (!Number.isInteger(index) || index < 0 || index >= items.length) return;
    event.preventDefault();
    onselect(items[index].key);
  }
</script>

<svelte:window onkeydown={shortcut} />

<header class="app-navigation" class:compact aria-label={label}>
  {#if !compact}<div class="rail-brand" title="Casklet">
      <BrandLogo /><span>Casklet</span>
    </div>{/if}
  <nav aria-label={label}>
    {#each items as item (item.key)}
      <button
        type="button"
        class:active={current === item.key}
        aria-current={current === item.key ? "page" : undefined}
        title={`${item.title} (Alt+${items.indexOf(item) + 1})`}
        aria-keyshortcuts={`Alt+${items.indexOf(item) + 1}`}
        onclick={() => onselect(item.key)}
      >
        <span class="rail-icon"
          ><Icon name={item.icon} size={21} />
          {#if item.badge}<span
              class="rail-badge"
              title="Recovery needs attention">{item.badge}</span
            >{/if}
        </span>
        <span>{item.title}</span>
      </button>
    {/each}
  </nav>
  {#if footer}<div class="rail-footer">{@render footer()}</div>{/if}
</header>

<style>
  .app-navigation {
    display: flex;
    align-items: center;
    gap: 32px;
    padding: 12px 32px;
    min-width: 0;
    background: var(--surface-panel);
    border-bottom: 1px solid var(--border-subtle);
  }
  .rail-brand {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-heading);
    font-size: 16px;
    font-weight: 650;
    letter-spacing: -0.04em;
  }
  .rail-brand :global(img) {
    width: 25px;
    height: 25px;
    object-fit: contain;
  }
  nav {
    display: flex;
    gap: 4px;
    min-width: 0;
  }
  nav button {
    position: relative;
    gap: 8px;
    min-height: 40px;
    padding: 9px 14px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-secondary);
    font-size: 12px;
    border-radius: var(--radius-control);
  }
  nav button:hover {
    background: var(--surface-hover);
    color: var(--text-heading);
    box-shadow: none;
  }
  nav button.active {
    background: var(--accent-surface);
    color: var(--accent);
    font-weight: 650;
  }
  nav button.active::after {
    content: "";
    position: absolute;
    bottom: -13px;
    height: 2px;
    left: 14px;
    right: 14px;
    background: var(--accent);
  }
  .rail-icon {
    position: relative;
    display: flex;
  }
  .rail-icon :global(svg) {
    width: 17px;
    height: 17px;
  }
  .rail-badge {
    position: absolute;
    top: -10px;
    right: -10px;
    padding: 1px 4px;
    min-width: 16px;
    background: var(--warning-surface);
    color: var(--warning-text);
    border: 1px solid var(--warning-border);
    border-radius: 4px;
    font-size: 10px;
  }
  .rail-footer {
    margin-left: auto;
  }
  .rail-footer :global(button) {
    font-size: 11px;
    gap: 7px;
    padding-inline: 8px;
  }
  .compact {
    padding: 0 24px;
    gap: 0;
    background: var(--surface-panel);
  }
  .compact nav {
    width: 100%;
    gap: 14px;
  }
  .compact nav button {
    flex: 1;
    padding: 12px 4px;
    min-height: 48px;
    border-radius: 0;
    font-size: 12px;
    background: transparent;
  }
  .compact nav button.active::after {
    bottom: -1px;
    left: 0;
    right: 0;
  }
  @media (max-width: 760px) {
    .app-navigation:not(.compact) {
      flex-wrap: wrap;
      gap: 8px 20px;
      padding: 12px 20px 0;
    }
    .app-navigation:not(.compact) nav {
      order: 3;
      width: 100%;
    }
    .app-navigation:not(.compact) nav button {
      flex: 1;
    }
    nav button.active::after {
      bottom: -1px;
    }
    .rail-footer {
      margin-left: auto;
    }
  }
  @media (max-width: 420px) {
    .app-navigation:not(.compact) {
      padding-inline: 16px;
    }
    nav button {
      padding: 10px 7px;
      gap: 6px;
      font-size: 11px;
    }
    .compact {
      padding-inline: 12px;
    }
    .compact nav {
      gap: 7px;
    }
    .compact nav button {
      font-size: 11px;
      gap: 5px;
    }
    .rail-icon :global(svg) {
      width: 15px;
      height: 15px;
    }
  }
</style>
