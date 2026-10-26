<script lang="ts">
  import { tick, untrack } from "svelte";
  import Icon from "./Icon.svelte";
  import CaskBadge from "./CaskBadge.svelte";
  import {
    CASK_ICONS,
    CASK_EMOJIS,
    EMOJI_GROUPS,
    DEFAULT_APPEARANCE,
    type CaskAppearance,
  } from "./caskAppearance";
  import { CASK_PALETTE } from "./caskColors";
  let {
    value = $bindable({ ...DEFAULT_APPEARANCE }),
    caskId = "",
    disabled = false,
    initiallyOpen = false,
  }: {
    value?: CaskAppearance;
    caskId?: string;
    disabled?: boolean;
    initiallyOpen?: boolean;
  } = $props();
  let open = $state(untrack(() => initiallyOpen));
  let mode: "icon" | "emoji" = $state(
    untrack(() => (value.symbol.startsWith("emoji:") ? "emoji" : "icon")),
  );
  let query = $state("");
  let category = $state("");
  let page = $state(0);
  const pageSize = 96;
  let root = $state<HTMLDivElement>();
  let trigger = $state<HTMLButtonElement>();
  async function escapePicker(event: KeyboardEvent) {
    if (
      event.key !== "Escape" ||
      !open ||
      !root?.contains(document.activeElement)
    )
      return;
    event.preventDefault();
    event.stopPropagation();
    open = false;
    await tick();
    trigger?.focus();
  }
  const visibleIcons = $derived(
    CASK_ICONS.filter((item) =>
      item.name.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()),
    ),
  );
  const matchingEmojis = $derived(
    CASK_EMOJIS.filter(
      (item) =>
        (!category || item.group === category) &&
        query
          .trim()
          .toLocaleLowerCase()
          .split(/\s+/)
          .every((term) =>
            `${item.name} ${item.keywords} ${item.value}`
              .toLocaleLowerCase()
              .includes(term),
          ),
    ),
  );
  const visibleEmojis = $derived(
    matchingEmojis.slice(page * pageSize, (page + 1) * pageSize),
  );
  const chosenName = $derived(
    (value.symbol.startsWith("icon:")
      ? CASK_ICONS.find((item) => `icon:${item.value}` === value.symbol)?.name
      : CASK_EMOJIS.find((item) => `emoji:${item.value}` === value.symbol)
          ?.name) ??
      (value.symbol.startsWith("emoji:") ? "Saved emoji" : "Collection"),
  );
  const colors = [
    "Lime",
    "Blue",
    "Lavender",
    "Amber",
    "Rose",
    "Teal",
    "Coral",
    "Orange",
    "Violet",
    "Ocean",
    "White",
    "Leaf",
  ];
</script>

<svelte:window onkeydown={escapePicker} />

<div class="cask-appearance-picker" bind:this={root}>
  <button
    type="button"
    bind:this={trigger}
    class="appearance-trigger"
    aria-label="Choose Cask appearance"
    aria-expanded={open}
    {disabled}
    onclick={() => (open = !open)}
  >
    <CaskBadge {caskId} appearance={value} size={38} /><span
      ><strong>{chosenName}</strong><small>Icon, emoji and color</small></span
    ><Icon name={open ? "up" : "down"} size={16} />
  </button>
  {#if open}<div class="appearance-palette">
      <div class="appearance-picker-toolbar">
        <div role="group" aria-label="Symbol type">
          {#each ["icon", "emoji"] as tab (tab)}<button
              type="button"
              aria-pressed={mode === tab}
              {disabled}
              onclick={() => {
                mode = tab as typeof mode;
                query = "";
                page = 0;
              }}>{tab === "icon" ? "Icons" : "Emoji"}</button
            >{/each}
        </div>
        <button
          type="button"
          class="text-button"
          {disabled}
          onclick={() => {
            value = { ...DEFAULT_APPEARANCE };
            category = "";
            query = "";
            page = 0;
          }}>Clear</button
        >
      </div>
      <label class="search-field appearance-search"
        ><Icon name="search" size={15} /><input
          aria-label={mode === "icon" ? "Search icons" : "Search emojis"}
          placeholder={mode === "icon" ? "Search icons" : "Search emojis"}
          bind:value={query}
          oninput={() => (page = 0)}
          onkeydown={(event) => {
            if (event.key === "Enter") event.preventDefault();
          }}
          {disabled}
        /></label
      >
      <div
        class="appearance-color-options"
        role="group"
        aria-label="Cask color"
      >
        <button
          type="button"
          class="appearance-color automatic-color"
          aria-label="Automatic color"
          title="Automatic color"
          aria-pressed={value.color === null}
          {disabled}
          onclick={() => (value = { ...value, color: null })}
          ><Icon name="casks" size={14} /></button
        >
        {#if value.color && !CASK_PALETTE.some((color) => color === value.color)}
          <button
            type="button"
            class="appearance-color"
            aria-label="Saved color"
            title="Saved color"
            aria-pressed="true"
            style:--swatch={value.color}
            {disabled}><span></span></button
          >
        {/if}
        {#each CASK_PALETTE as color, index (color)}<button
            type="button"
            class="appearance-color"
            class:white-color={color === "#ffffff"}
            aria-label={`${colors[index]} color`}
            title={colors[index]}
            aria-pressed={value.color === color}
            style:--swatch={color}
            {disabled}
            onclick={() => (value = { ...value, color })}><span></span></button
          >{/each}
      </div>
      {#if mode === "emoji"}
        <label class="emoji-category"
          >Category
          <select
            aria-label="Emoji category"
            bind:value={category}
            onchange={() => (page = 0)}
            {disabled}
          >
            <option value="">All emoji</option>
            {#each EMOJI_GROUPS as group (group)}<option value={group}
                >{group}</option
              >{/each}
          </select>
        </label>
      {/if}
      <div
        class="appearance-symbol-options"
        role="group"
        aria-label={mode === "icon" ? "Cask icons" : "Cask emojis"}
      >
        {#if mode === "icon"}{#each visibleIcons as item (item.value)}<button
              type="button"
              aria-label={`${item.name} icon`}
              title={item.name}
              aria-pressed={value.symbol === `icon:${item.value}`}
              {disabled}
              onclick={() =>
                (value = { ...value, symbol: `icon:${item.value}` })}
              ><Icon name={item.value} size={20} /></button
            >{:else}<p>No matching icons.</p>{/each}
        {:else}{#each visibleEmojis as item (item.value)}<button
              type="button"
              aria-label={`${item.name} emoji`}
              title={item.name}
              aria-pressed={value.symbol === `emoji:${item.value}`}
              {disabled}
              onclick={() =>
                (value = { ...value, symbol: `emoji:${item.value}` })}
              ><span aria-hidden="true">{item.value}</span></button
            >{:else}<p>No matching emojis.</p>{/each}{/if}
      </div>
      {#if mode === "emoji"}
        <div class="emoji-pagination">
          <button
            type="button"
            aria-label="Previous emoji page"
            disabled={disabled || page === 0}
            onclick={() => page--}>Previous</button
          >
          <span role="status"
            >{matchingEmojis.length.toLocaleString()} emoji · {page + 1} / {Math.max(
              1,
              Math.ceil(matchingEmojis.length / pageSize),
            )}</span
          >
          <button
            type="button"
            aria-label="Next emoji page"
            disabled={disabled ||
              (page + 1) * pageSize >= matchingEmojis.length}
            onclick={() => page++}>Next</button
          >
        </div>
      {/if}
      <p class="appearance-picker-note" role="status">
        Preview: {chosenName}{value.color
          ? ", custom color"
          : ", automatic color"}
      </p>
    </div>{/if}
</div>
