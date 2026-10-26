<script lang="ts">
  import Icon from "./Icon.svelte";
  import CaskBadge from "./CaskBadge.svelte";
  import CaskAppearancePicker from "./CaskAppearancePicker.svelte";
  import { DEFAULT_APPEARANCE, type CaskAppearance } from "./caskAppearance";
  let {
    caskId,
    name,
    value = $bindable({ ...DEFAULT_APPEARANCE }),
    editing = $bindable(false),
    disabled = false,
  }: {
    caskId: string;
    name: string;
    value?: CaskAppearance;
    editing?: boolean;
    disabled?: boolean;
  } = $props();
</script>

<section
  class="settings-card cask-appearance-settings"
  aria-label="Cask appearance"
>
  <div class="settings-heading">
    <CaskBadge {caskId} appearance={value} size={48} />
    <div>
      <h2>Appearance</h2>
      <p>An icon and color for {name}.</p>
    </div>
    {#if !editing}<button
        type="button"
        {disabled}
        onclick={() => (editing = true)}
        ><Icon name="edit" size={17} />Change appearance</button
      >{/if}
  </div>
  {#if editing}<div class="appearance-settings-editor">
      <CaskAppearancePicker bind:value {caskId} {disabled} initiallyOpen />
    </div>{/if}
</section>
