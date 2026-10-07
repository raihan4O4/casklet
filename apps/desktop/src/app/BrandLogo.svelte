<script lang="ts">
  import { theme } from "./theme";
  import blackLogo from "../assets/casklet-logo-black.png";
  import whiteLogo from "../assets/casklet-logo-white.png";
  let { alt = "Casklet logo" }: { alt?: string } = $props();
</script>

<svelte:head>
  <link rel="preload" as="image" href={blackLogo} />
  <link rel="preload" as="image" href={whiteLogo} />
</svelte:head>

<img src={$theme === "dark" ? whiteLogo : blackLogo} {alt} draggable="false" />
