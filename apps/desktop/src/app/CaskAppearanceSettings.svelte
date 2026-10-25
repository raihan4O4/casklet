<script lang="ts">
  import CaskBadge from "./CaskBadge.svelte";
  import CaskAppearancePicker from "./CaskAppearancePicker.svelte";
  import {
    appearanceForCask,
    saveCaskAppearance,
    DEFAULT_APPEARANCE,
  } from "./caskAppearance";
  let { caskId, name }: { caskId: string; name: string } = $props();
  let editing = $state(false);
  let draft = $state({ ...DEFAULT_APPEARANCE });
  let status = $state("");
  let trigger = $state<HTMLButtonElement>();
  function close() {
    editing = false;
    queueMicrotask(() => trigger?.focus());
  }
</script>

<section
  class="settings-card cask-appearance-settings"
  aria-label="Cask appearance"
>
  <div class="settings-heading">
    <CaskBadge {caskId} size={38} />
    <div>
      <h2>Appearance</h2>
      <p>An icon and color for {name}.</p>
    </div>
    {#if !editing}<button
        type="button"
        bind:this={trigger}
        onclick={() => {
          draft = appearanceForCask(caskId);
          status = "";
          editing = true;
        }}>Change appearance</button
      >{/if}
  </div>
  {#if editing}<div class="appearance-settings-editor">
      <CaskAppearancePicker bind:value={draft} {caskId} initiallyOpen />
      <div class="appearance-settings-actions">
        <button type="button" onclick={close}>Cancel appearance</button><button
          type="button"
          class="primary-button"
          onclick={() => {
            const persisted = saveCaskAppearance(caskId, draft);
            status = persisted
              ? "Appearance saved."
              : "Appearance updated for this session only.";
            close();
          }}>Save appearance</button
        >
      </div>
    </div>{/if}
  {#if status}<p class="appearance-save-status" role="status">{status}</p>{/if}
</section>
