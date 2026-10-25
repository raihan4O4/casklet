<script lang="ts">
  import type { RemovalPlanView } from "../contracts/generated/casklet";
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import {
    planExpired,
    cleanupStatus,
    cleanupActionHint,
    cleanupSummary,
    cleanupItemEffect,
    cleanupReason,
    cleanupCounts,
  } from "./cleanup";
  let {
    plan,
    removeCask = false,
    removalEnabled,
    removalBusy,
    reviewedPlanHash,
    executionConfirmed = $bindable(false),
    permanentConfirmation = $bindable(false),
    preserveConfirmed = $bindable(false),
    onReview,
    onApprove,
    onExecute,
    onClose,
  }: {
    plan: RemovalPlanView;
    removeCask?: boolean;
    removalEnabled: boolean;
    removalBusy: boolean;
    reviewedPlanHash: string;
    executionConfirmed?: boolean;
    permanentConfirmation?: boolean;
    preserveConfirmed?: boolean;
    onReview: () => void;
    onApprove: () => void;
    onExecute: () => void;
    onClose: () => void;
  } = $props();
  let nowMs = $state(Date.now());
  let expanded = $state(false);
  let displayedPlanHash = $state("");
  $effect(() => {
    if (displayedPlanHash !== plan.planHash) {
      displayedPlanHash = plan.planHash;
      expanded = false;
    }
  });
  const counts = $derived(cleanupCounts(plan));
  const disclosureDuration = () =>
    window.matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : 160;
  const expired = $derived(planExpired(plan, nowMs));
  const positiveStatus = $derived(
    !expired &&
      ((["DRAFT", "APPROVED"].includes(plan.state) &&
        reviewedPlanHash === plan.planHash) ||
        (plan.state === "COMPLETE" &&
          !plan.items.some((item) =>
            ["FAILED", "UNKNOWN"].includes(item.state),
          ))),
  );
  onMount(() => {
    const timer = setInterval(() => {
      nowMs = Date.now();
    }, 1000);
    return () => clearInterval(timer);
  });
</script>

<section class="cleanup-review-panel" aria-labelledby="cleanup-plan-heading">
  <div class="cleanup-review-heading">
    <h3 id="cleanup-plan-heading" tabindex="-1">Cleanup Review</h3>
    <span
      class="cleanup-status"
      class:reviewed={positiveStatus}
      role="status"
      aria-label={`Cleanup status: ${cleanupStatus(plan, reviewedPlanHash, nowMs)}`}
    >
      {#if positiveStatus}<Icon name="check" size={14} />{/if}{cleanupStatus(
        plan,
        reviewedPlanHash,
        nowMs,
      )}
    </span>
  </div>
  <ol class="cleanup-stages" aria-label="Cleanup stages">
    <li
      class:complete={reviewedPlanHash === plan.planHash}
      aria-current={reviewedPlanHash !== plan.planHash || !removalEnabled
        ? "step"
        : undefined}
    >
      Review
    </li>
    <li
      class:complete={plan.state === "APPROVED" || plan.state === "COMPLETE"}
      aria-current={removalEnabled &&
      reviewedPlanHash === plan.planHash &&
      plan.state === "DRAFT"
        ? "step"
        : undefined}
    >
      Approve
    </li>
    <li
      aria-current={removalEnabled && plan.state === "APPROVED"
        ? "step"
        : undefined}
    >
      Execute
    </li>
  </ol>
  <button
    type="button"
    class="cleanup-review-toggle"
    aria-label={expanded ? "Collapse cleanup items" : "Expand cleanup items"}
    aria-expanded={expanded}
    aria-controls="cleanup-review-items"
    onclick={() => {
      expanded = !expanded;
    }}
  >
    <span class="cleanup-compact-summary">
      <strong
        >{plan.items.length}
        {plan.items.length === 1 ? "item" : "items"} in this plan</strong
      >
      <span class="cleanup-counts">
        {#each counts as count (count.label)}<span
            >{count.count}
            {count.label === "Data kept"
              ? count.count === 1
                ? "item kept"
                : "items kept"
              : count.label}</span
          >{/each}
      </span>
      {#if !["DRAFT", "APPROVED"].includes(plan.state)}
        <span class="cleanup-outcome">{cleanupSummary(plan)}</span>
      {/if}
    </span>
    <span class="cleanup-chevron" class:expanded
      ><Icon name="down" size={18} /></span
    >
  </button>
  <div id="cleanup-review-items">
    {#if expanded}
      <div
        class="cleanup-review-content"
        transition:slide={{ duration: disclosureDuration() }}
      >
        {#if ["DRAFT", "APPROVED"].includes(plan.state)}
          <p class="cleanup-summary">{cleanupSummary(plan)}</p>
        {/if}
        <h4>Items</h4>
        <ul class="cleanup-items">
          {#each plan.items as item (item.id)}<li>
              <strong>{item.name}</strong><span>{cleanupItemEffect(item)}</span>
              <small class="cleanup-source" title={item.path}>{item.path}</small
              >
            </li>{/each}
        </ul>
      </div>
    {/if}
  </div>
</section>
{#if removeCask}<p class="cleanup-summary">
    After successful cleanup, this Cask will be permanently removed. Referenced
    files and folders stay on disk. Failures or remaining dependencies keep the
    Cask available.
  </p>{/if}
{#if expired}<p>Plan expired. Create a new cleanup plan.</p>{/if}
{#if plan.state === "DRAFT" || plan.state === "APPROVED"}
  <p class="cleanup-next" id="cleanup-next" aria-live="polite">
    {#if expired}Create a new plan to continue; this preview has expired.
    {:else if removalBusy}Updating this cleanup plan…
    {:else if !removalEnabled}You can review the effects below. This build
      cannot approve or run cleanup.
    {:else if reviewedPlanHash !== plan.planHash}Next: Review the planned
      actions. Nothing has been removed.
    {:else if plan.state === "DRAFT"}Next: Approve this exact plan. Approval
      does not run cleanup.
    {:else if !executionConfirmed || (plan.permanent && !permanentConfirmation)}Next:
      Confirm the actions below to enable Execute Cleanup.
    {:else}Ready to execute. Casklet will revalidate the plan before removing
      anything.{/if}
  </p>
  {#if plan.state === "APPROVED" && removalEnabled && !expired && reviewedPlanHash === plan.planHash}
    <label class="checkbox-label"
      ><input type="checkbox" bind:checked={executionConfirmed} /> I confirm the
      cleanup actions shown above{removeCask
        ? " and removal of this Cask after they succeed"
        : ""}</label
    >
    {#if plan.permanent}<label class="checkbox-label"
        ><input type="checkbox" bind:checked={permanentConfirmation} /> I understand
        permanent deletion cannot be undone</label
      >{/if}
  {/if}
  {#if !expired}<div class="cleanup-actions">
      {#if reviewedPlanHash !== plan.planHash}
        <button
          class:primary-button={plan.state === "DRAFT" &&
            reviewedPlanHash !== plan.planHash}
          type="button"
          disabled={expired ||
            removalBusy ||
            reviewedPlanHash === plan.planHash}
          onclick={() => {
            expanded = true;
            onReview();
          }}
          ><Icon name="check" size={16} />{reviewedPlanHash === plan.planHash
            ? "Reviewed"
            : "Review Cleanup"}</button
        >
      {/if}
      {#if removalEnabled && plan.state === "DRAFT" && reviewedPlanHash === plan.planHash}
        <button
          class="primary-button"
          type="button"
          title={cleanupActionHint(
            plan,
            removalEnabled,
            reviewedPlanHash,
            executionConfirmed,
            permanentConfirmation,
            nowMs,
          )}
          aria-describedby="cleanup-next"
          disabled={expired ||
            !removalEnabled ||
            removalBusy ||
            plan.state !== "DRAFT" ||
            reviewedPlanHash !== plan.planHash}
          onclick={onApprove}>Approve Cleanup</button
        >
      {/if}
      {#if removalEnabled && plan.state === "APPROVED" && reviewedPlanHash === plan.planHash}
        <button
          class="danger-button"
          type="button"
          title={cleanupActionHint(
            plan,
            removalEnabled,
            reviewedPlanHash,
            executionConfirmed,
            permanentConfirmation,
            nowMs,
          )}
          aria-describedby="cleanup-next"
          disabled={expired ||
            !removalEnabled ||
            removalBusy ||
            plan.state !== "APPROVED" ||
            !executionConfirmed ||
            reviewedPlanHash !== plan.planHash ||
            (plan.permanent && !permanentConfirmation)}
          onclick={onExecute}>Execute Cleanup</button
        >
      {/if}
    </div>{/if}
{/if}
{#if plan.state === "PARTIAL"}<p>
    Some outcomes need inspection. Open Advanced details for recovery locations,
    or review Activity before continuing.
  </p>{/if}
<details class="cleanup-details">
  <summary>Advanced details</summary>
  <p>
    Plan ID: {plan.id}. State: {plan.state}. Expires: {new Date(
      plan.expiresAtMs,
    ).toLocaleString()}.
  </p>
  <p>Plan hash: {plan.planHash}</p>
  {#if plan.runId}<p>Run ID: {plan.runId}</p>{/if}
  <ul>
    {#each plan.items as item (item.id)}<li>
        {item.name}: {cleanupReason(item.reasonCode)}<br /><code
          >{item.path}</code
        ><br />Status: {item.state}{item.resultCode
          ? ` (${item.resultCode})`
          : ""}{#if item.recoveryPath}<br />Possible recovery location:
          <code>{item.recoveryPath}</code>{/if}
      </li>{/each}
  </ul>
  <p>
    Reviewing acknowledges this exact plan; approval and execution are separate.
    {#if removeCask}The Cask is removed only after all phases succeed and its
      final dependency checks pass.{:else}This scope cleans up its selected
      contents; the Cask remains.{/if}
  </p>
  {#if !removalEnabled}<p>
      Approval and execution remain disabled pending independent security
      review.
    </p>{/if}
</details>

{#if removalEnabled && plan.state === "PARTIAL"}
  <label class="checkbox-label"
    ><input type="checkbox" bind:checked={preserveConfirmed} />
    {plan.items.some((item) => item.reasonCode === "APPLICATION_OWNERSHIP")
      ? "I inspected the applications and will preserve unresolved results"
      : "I inspected the recovery locations and will keep unresolved items"}</label
  >
  <button
    type="button"
    disabled={expired || removalBusy || !preserveConfirmed}
    onclick={onClose}>Keep unresolved items and close cleanup</button
  >
{/if}
