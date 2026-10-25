<script lang="ts">
  import {
    theme,
    accent,
    setAccentPreference,
    setThemePreference,
    type Theme,
  } from "./theme";
  import { contrast, normalizeAccent } from "./accent";
  const presets = [
    { name: "Lagoon", color: "#235d65" },
    { name: "Iris", color: "#7357d8" },
    { name: "Blue", color: "#2563eb" },
    { name: "Rose", color: "#c43d72" },
    { name: "Amber", color: "#e5a521" },
    { name: "Leaf", color: "#448348" },
  ];
  let hex = $state("");
  let invalid = $state(false);
  $effect(() => {
    hex = $accent ?? ($theme === "dark" ? "#b6cbff" : "#315bb5");
    invalid = false;
  });
  function updateHex() {
    const value = normalizeAccent(
      hex.trim().startsWith("#") ? hex.trim() : "#" + hex.trim(),
    );
    invalid = value === null;
    if (value) void setAccentPreference(value);
  }
  import Icon from "./Icon.svelte";
</script>

<section class="settings-card" aria-labelledby="appearance-title">
  <div class="settings-heading">
    <span class="setting-symbol"><Icon name="settings" size={20} /></span>
    <div>
      <h2 id="appearance-title">Appearance</h2>
      <p>Make every Casklet window feel right for you.</p>
    </div>
  </div>
  <div class="appearance-setting">
    <div>
      <h3>Color theme</h3>
      <p>Choose the appearance for all Casklet windows.</p>
    </div>
    <div class="theme-control" role="radiogroup" aria-label="Color theme">
      {#each ["light", "dark"] as value (value)}
        <label
          ><input
            type="radio"
            name="color-theme"
            {value}
            checked={$theme === value}
            onchange={() => void setThemePreference(value as Theme)}
          /><span
            class="theme-preview"
            class:dark-preview={value === "dark"}
            aria-hidden="true"
            ><span></span><span><i></i><i></i><i></i></span></span
          ><span>{value === "light" ? "Light" : "Dark"}</span></label
        >
      {/each}
    </div>
  </div>
  <div class="accent-setting">
    <div class="accent-heading">
      <div>
        <h3>Accent color</h3>
        <p>Your color, throughout Casklet. Changes are saved instantly.</p>
      </div>
      <button
        class="text-button"
        disabled={$accent === null}
        onclick={() => void setAccentPreference(null)}>Reset to default</button
      >
    </div>
    <div class="accent-editor">
      <div class="accent-choices">
        <div class="accent-presets" role="group" aria-label="Accent presets">
          {#each presets as preset (preset.color)}
            <button
              class="accent-swatch"
              style:--swatch={preset.color}
              style:--swatch-text={contrast(preset.color, "#000000") >=
              contrast(preset.color, "#ffffff")
                ? "#000000"
                : "#ffffff"}
              aria-label={preset.name}
              aria-pressed={$accent === preset.color}
              onclick={() => void setAccentPreference(preset.color)}
            >
              <span aria-hidden="true"
                >{$accent === preset.color ? "✓" : ""}</span
              >
            </button>
          {/each}
        </div>
        <div class="accent-custom">
          <label
            class="color-picker"
            style:--swatch={$accent ??
              ($theme === "dark" ? "#b6cbff" : "#315bb5")}
          >
            <span aria-hidden="true"></span><span>Custom color</span>
            <input
              type="color"
              aria-label="Custom accent color"
              value={$accent ?? ($theme === "dark" ? "#b6cbff" : "#315bb5")}
              oninput={(event) =>
                void setAccentPreference(event.currentTarget.value)}
            />
          </label>
          <label class="accent-hex"
            >HEX
            <input
              aria-label="Accent hex color"
              aria-invalid={invalid}
              aria-describedby="accent-help"
              spellcheck="false"
              maxlength="7"
              bind:value={hex}
              oninput={() => {
                invalid = false;
                if (normalizeAccent(hex)) void setAccentPreference(hex);
              }}
              onchange={updateHex}
              onkeydown={(event) => {
                if (event.key === "Enter") updateHex();
              }}
            />
          </label>
        </div>
        <p id="accent-help" class:field-error={invalid}>
          {invalid
            ? "Enter six hex digits, such as #7357D8."
            : "Choose any color. Text and icons adjust for contrast."}
        </p>
      </div>
      <div class="accent-live-preview" aria-label="Accent preview">
        <span class="eyebrow">LIVE PREVIEW</span>
        <div>
          <span class="accent-preview-icon"
            ><Icon name="folder" size={19} /></span
          ><strong>A little more you.</strong>
        </div>
        <span class="accent-preview-action"
          ><Icon name="check" size={16} /> Ready to go</span
        >
        <span class="accent-preview-link">Your workspace, your color</span>
      </div>
    </div>
  </div>
</section>
