<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { CONTRACT_VERSION } from "../contracts/generated/casklet";
  import type { MsiCleanupPlan } from "../contracts/generated/casklet";
  import { label } from "./format";
  let {
    caskId,
    enabled,
    onChanged,
    hasManaged,
    installationId = "",
  }: {
    caskId: string;
    enabled: boolean;
    hasManaged: boolean;
    installationId?: string;
    onChanged: () => Promise<void>;
  } = $props();
  let plan: MsiCleanupPlan | null = $state(null);
  let busy = $state(false);
  let reviewedHash = $state("");
  let confirmed = $state(false);
  let keepConfirmed = $state(false);
  let error = $state("");
  let clock = $state(Date.now());
  const interrupted = $derived.by(() => {
    const current = plan as MsiCleanupPlan | null;
    return current && ["EXECUTING", "PARTIAL"].includes(current.state);
  });
  const expired = $derived.by(() => {
    const current = plan as MsiCleanupPlan | null;
    return current && current.expiresAtMs <= clock;
  });
  let loadRevision = 0;
  async function load() {
    const token = ++loadRevision;
    const result =
      (await invoke<MsiCleanupPlan | null>("latest_msi_cleanup", {
        request: { version: CONTRACT_VERSION, id: caskId },
      })) ?? null;
    if (token === loadRevision) plan = result?.id ? result : null;
  }
  async function action(command: string) {
    if (busy) return;
    busy = true;
    error = "";
    try {
      plan = await invoke<MsiCleanupPlan>(
        command === "create_msi_cleanup" && installationId
          ? "create_application_removal"
          : command,
        {
          request:
            command === "create_msi_cleanup"
              ? installationId
                ? { version: CONTRACT_VERSION, caskId, installationId }
                : { version: CONTRACT_VERSION, id: caskId }
              : {
                  version: CONTRACT_VERSION,
                  id: plan!.id,
                  planHash: plan!.planHash,
                },
        },
      );
      if (command === "create_msi_cleanup" || command === "close_msi_cleanup")
        reviewedHash = "";
      loadRevision++;
      confirmed = false;
      keepConfirmed = false;
      await onChanged();
    } catch (cause) {
      error =
        typeof cause === "object" && cause && "message" in cause
          ? String(cause.message)
          : String(cause);
      await load().catch(() => {});
    } finally {
      busy = false;
    }
  }
  async function retry() {
    await action("close_msi_cleanup");
    if (plan?.state === "CLOSED_PRESERVED") await action("create_msi_cleanup");
  }
  async function keep(installationId: string) {
    if (!keepConfirmed || busy) return;
    busy = true;
    try {
      await invoke("release_managed_msi", {
        request: { version: CONTRACT_VERSION, caskId, installationId },
      });
      plan = null;
      reviewedHash = "";
      confirmed = false;
      keepConfirmed = false;
      await onChanged();
    } catch (cause) {
      error =
        typeof cause === "object" && cause && "message" in cause
          ? String(cause.message)
          : String(cause);
    } finally {
      busy = false;
    }
  }
  onMount(() => {
    void load().catch((cause) => {
      error = String(cause?.message ?? cause);
    });
    const interval = window.setInterval(() => {
      clock = Date.now();
      if (busy) void load().catch(() => {});
    }, 1000);
    return () => window.clearInterval(interval);
  });
</script>

{#if hasManaged || plan}
  <section class="msi-cleanup" aria-label="Managed application cleanup">
    <div class="section-heading">
      <h3>Application cleanup</h3>
      {#if plan}<span class="state-badge"
          >{expired &&
          !interrupted &&
          ["DRAFT", "APPROVED"].includes(plan.state)
            ? "Expired"
            : label(plan.state)}</span
        >{/if}
    </div>
    <p>
      References are detached. Casklet-owned applications uninstall one at a
      time.
    </p>
    <button
      type="button"
      disabled={!hasManaged || busy || !!interrupted}
      onclick={() => void action("create_msi_cleanup")}
      >{installationId
        ? "Preview selected application removal"
        : "Preview application cleanup"}</button
    >
    {#if plan}
      {#if plan.items.some((item) => item.action === "PRESERVE") && !interrupted}<label
          class="checkbox-label"
          ><input
            type="checkbox"
            bind:checked={keepConfirmed}
            disabled={busy}
          /> I confirm keeping the selected application installed and removing its
          Cask association.</label
        >{/if}
      <ul class="cleanup-items">
        {#each plan.items as item (item.installationId)}<li>
            <strong>{item.name}</strong><span
              >{["UNKNOWN", "FAILED"].includes(item.state)
                ? `Failed - ${item.result ?? "Result needs inspection"}`
                : item.state === "RUNNING"
                  ? "Uninstalling..."
                  : item.state === "DETACHED"
                    ? "Reference / Detached"
                    : item.action === "DETACH"
                      ? "Reference / Remove from Cask"
                      : item.state === "SUCCEEDED"
                        ? "Uninstalled"
                        : item.action === "UNINSTALL"
                          ? "Waiting - Uninstall application"
                          : `Keep - ${item.result ?? "shared or no longer verified"}`}</span
            >{#if item.action === "PRESERVE" && !interrupted}<button
                type="button"
                disabled={busy || !keepConfirmed}
                onclick={() => void keep(item.installationId)}
                >Keep installed and detach</button
              >{/if}
          </li>{/each}
      </ul>
      {#if !enabled}<p class="cleanup-notice">
          Application uninstall is unavailable in this build. You can review
          this preview.
        </p>{/if}
      {#if expired && ["DRAFT", "APPROVED"].includes(plan.state)}<p>
          Plan expired. Create a new preview.
        </p>{/if}
      {#if ["DRAFT", "APPROVED"].includes(plan.state)}
        <div class="tool-actions">
          <button
            type="button"
            disabled={busy || !!expired || reviewedHash === plan.planHash}
            onclick={() => {
              reviewedHash = plan!.planHash;
              confirmed = false;
            }}
            >{reviewedHash === plan.planHash
              ? "Reviewed"
              : "Review applications"}</button
          >
          <button
            type="button"
            title={!enabled
              ? "Uninstall is unavailable in this build"
              : "Review the application preview first"}
            disabled={busy ||
              !enabled ||
              !!expired ||
              reviewedHash !== plan.planHash ||
              plan.state !== "DRAFT"}
            onclick={() => void action("approve_msi_cleanup")}
            >Approve uninstall</button
          >
          <button
            class="danger-button"
            type="button"
            disabled={busy ||
              !enabled ||
              !!expired ||
              reviewedHash !== plan.planHash ||
              plan.state !== "APPROVED" ||
              !confirmed}
            onclick={() => void action("execute_msi_cleanup")}
            >Uninstall applications</button
          >
        </div>
        <label class="checkbox-label"
          ><input type="checkbox" bind:checked={confirmed} disabled={busy} />
          {plan.items.some((item) => item.action === "UNINSTALL")
            ? "I confirm uninstalling the listed applications. Windows may request permission."
            : "I confirm keeping these applications installed and removing their Cask associations."}</label
        >
      {/if}
      {#if plan.state === "PARTIAL"}<p role="alert">
          Cleanup finished with {plan.items.filter((item) =>
            ["UNKNOWN", "FAILED"].includes(item.state),
          ).length} failed application(s). Ownership is retained. Inspect the result,
          close this queue, then create a fresh preview to retry.
        </p>{/if}
      {#if interrupted}<p>
          Cleanup needs inspection. Check Windows before closing it; uncertain
          results are never repeated automatically.
        </p>
        <label class="checkbox-label"
          ><input type="checkbox" bind:checked={confirmed} disabled={busy} /> I inspected
          the applications and will preserve unresolved results</label
        ><button
          type="button"
          disabled={busy || !confirmed}
          onclick={() => void action("close_msi_cleanup")}
          >Close results and allow a new retry preview</button
        ><button
          type="button"
          disabled={busy || !confirmed}
          onclick={() => void retry()}
          >Retry failed applications - preview</button
        >{/if}
      {#if plan.state === "COMPLETE"}<p>
          Application cleanup finished. Continue with file cleanup below.
        </p>{/if}
      <details class="metadata-disclosure">
        <summary>Technical details</summary>
        <p class="mono">Plan {plan.id}</p>
        {#each plan.items as item (item.installationId)}<p class="mono">
            {item.productCode} · {item.installationId} · {item.state}
          </p>{/each}
      </details>
    {/if}
    {#if busy}<p role="status">
        Working… Windows may be waiting for your confirmation.
      </p>{/if}
    {#if error}<p role="alert">{error}</p>{/if}
  </section>
{/if}
