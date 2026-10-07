<script lang="ts">
  import { theme, setThemePreference, type Theme } from "./theme";
</script>

<section aria-labelledby="appearance-title">
  <h2 id="appearance-title">Appearance</h2>
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
          /><span>{value === "light" ? "Light" : "Dark"}</span></label
        >
      {/each}
    </div>
  </div>
</section>
