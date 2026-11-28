<script lang="ts">
  import Icon from "./Icon.svelte";
  import {
    applicationIcon,
    type ApplicationIconSource,
  } from "./applicationIcons";
  let {
    source,
    fallback = "package",
  }: { source?: ApplicationIconSource; fallback?: "package" | "link" } =
    $props();
  let image = $state<string | null>(null);
  let element: HTMLSpanElement;
  $effect(() => {
    const current = source;
    image = null;
    if (!current) return;
    let active = true;
    let requested = false;
    const load = () => {
      if (requested) return;
      requested = true;
      void applicationIcon(current).then((value) => {
        if (active) image = value;
      });
    };
    const observer =
      typeof IntersectionObserver === "undefined"
        ? null
        : new IntersectionObserver(
            (entries) => {
              if (entries.some((entry) => entry.isIntersecting)) {
                observer?.disconnect();
                load();
              }
            },
            { rootMargin: "160px" },
          );
    if (observer) observer.observe(element);
    else load();
    return () => {
      active = false;
      observer?.disconnect();
    };
  });
</script>

<span
  bind:this={element}
  class="application-icon"
  class:has-application-image={!!image}
  aria-hidden="true"
>
  {#if image}<img
      src={image}
      alt=""
      width="28"
      height="28"
      onerror={() => {
        image = null;
      }}
    />
  {:else}<Icon name={fallback} size={20} />{/if}
</span>
