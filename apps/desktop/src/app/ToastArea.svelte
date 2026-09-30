<script lang="ts">
  import { fly } from "svelte/transition";
  import { notifications, dismissNotice } from "./notifications";
  import Icon from "./Icon.svelte";
  const reduced =
    typeof matchMedia !== "undefined" &&
    matchMedia("(prefers-reduced-motion: reduce)").matches;
</script>

<aside class="toast-area" aria-label="Notifications">
  {#each $notifications as notice (notice.id)}
    <div
      class="toast"
      class:error={notice.kind === "error"}
      class:warning={notice.kind === "warning"}
      class:success={notice.kind === "success"}
      role={notice.kind === "error" ? "alert" : "status"}
      in:fly={{ y: reduced ? 0 : -5, duration: reduced ? 0 : 150 }}
      out:fly={{ y: reduced ? 0 : -3, duration: reduced ? 0 : 100 }}
    >
      <Icon
        name={notice.kind === "success" ? "check" : "info"}
        size={16}
      /><span>{notice.message}</span><button
        class="icon-button"
        type="button"
        aria-label="Dismiss notification"
        onclick={() => dismissNotice(notice.id)}
        ><Icon name="close" size={14} /></button
      >
    </div>
  {/each}
</aside>
