<script lang="ts">
  import { theme, setThemePreference, type Theme } from "./theme";
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
</section>
