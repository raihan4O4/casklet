<script lang="ts">
  import Icon from "./Icon.svelte";
  import { caskColors, colorForCask, CASK_PALETTE } from "./caskColors";
  import {
    caskAppearances,
    CASK_ICONS,
    DEFAULT_APPEARANCE,
    type CaskAppearance,
  } from "./caskAppearance";
  let {
    caskId = "",
    appearance,
    size = 44,
    title,
    muted = false,
  }: {
    caskId?: string;
    appearance?: CaskAppearance;
    size?: number;
    title?: string;
    muted?: boolean;
  } = $props();
  const choice = $derived(
    appearance ?? $caskAppearances[caskId] ?? DEFAULT_APPEARANCE,
  );
  const icon = $derived(
    CASK_ICONS.find((item) => `icon:${item.value}` === choice.symbol)?.value ??
      "casks",
  );
  const color = $derived(
    muted
      ? "#999999"
      : (choice.color ??
          $caskColors[caskId] ??
          (caskId ? colorForCask(caskId) : CASK_PALETTE[0])),
  );
</script>

<span
  class="collection-mark cask-badge"
  data-cask-symbol={choice.symbol}
  style:--cask-color={color}
  style:width={`${size}px`}
  style:height={`${size}px`}
  {title}
  aria-hidden="true"
>
  {#if choice.symbol.startsWith("emoji:")}<span
      class="cask-emoji"
      style:font-size={`${Math.round(size * 0.56)}px`}
      >{choice.symbol.slice(6)}</span
    >{:else}<Icon name={icon} size={Math.round(size * 0.5)} />{/if}
</span>
