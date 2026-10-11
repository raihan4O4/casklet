<script lang="ts">
  import type { RemovalPlanView } from "../contracts/generated/casklet";
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import {
    planExpired,
    cleanupStatus,
    cleanupActionHint,
    cleanupSummary,
    cleanupItemEffect,
    cleanupReason,
  } from "./cleanup";
  let {
    plan,
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
  const expired = $derived(planExpired(plan, nowMs));
  onMount(() => {
    const timer = setInterval(() => {
      nowMs = Date.now();
    }, 1000);
    return () => clearInterval(timer);
  });
</script>

<div class="cleanup-review-heading">
  <h3 id="cleanup-plan-heading" tabindex="-1">Cleanup Review</h3>
  <span
    class="cleanup-status"
    class:reviewed={!expired &&
      (reviewedPlanHash === plan.planHash || plan.state === "COMPLETE")}
    role="status"
    aria-label={`Cleanup status: ${cleanupStatus(plan, reviewedPlanHash, nowMs)}`}
  >
    {#if !expired && (reviewedPlanHash === plan.planHash || plan.state === "COMPLETE")}<Icon
        name="check"
        size={14}
      />{/if}{cleanupStatus(plan, reviewedPlanHash, nowMs)}
  </span>
</div>
{#if expired}<p>Plan expired. Create a new cleanup plan.</p>{/if}
<p class="cleanup-summary">{cleanupSummary(plan)}</p>
<h4>Items</h4>
<ul class="cleanup-items">
  {#each plan.items as item (item.id)}<li>
      <strong>{item.name}</strong><span>{cleanupItemEffect(item)}</span>
    </li>{/each}
</ul>
{#if plan.state === "DRAFT" || plan.state === "APPROVED"}
  <div class="cleanup-actions">
    <button
      type="button"
      disabled={expired || removalBusy || reviewedPlanHash === plan.planHash}
      onclick={onReview}
      ><Icon name="check" size={16} />{reviewedPlanHash === plan.planHash
        ? "Reviewed"
        : "Review Cleanup"}</button
    >
    <button
      type="button"
      title={cleanupActionHint(
        plan,
        removalEnabled,
        reviewedPlanHash,
        executionConfirmed,
        permanentConfirmation,
        nowMs,
      )}
      aria-describedby={!removalEnabled ? "cleanup-limit" : undefined}
      disabled={expired ||
        !removalEnabled ||
        removalBusy ||
        plan.state !== "DRAFT" ||
        reviewedPlanHash !== plan.planHash}
      onclick={onApprove}>Approve Cleanup</button
    >
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
      aria-describedby={!removalEnabled ? "cleanup-limit" : undefined}
      disabled={expired ||
        !removalEnabled ||
        removalBusy ||
        plan.state !== "APPROVED" ||
        !executionConfirmed ||
        reviewedPlanHash !== plan.planHash ||
        (plan.permanent && !permanentConfirmation)}
      onclick={onExecute}>Execute Cleanup</button
    >
  </div>
  {#if plan.state === "APPROVED" && removalEnabled && !expired && reviewedPlanHash === plan.planHash}
    <label class="checkbox-label"
      ><input type="checkbox" bind:checked={executionConfirmed} /> I confirm the
      cleanup actions shown above</label
    >
    {#if plan.permanent}<label class="checkbox-label"
        ><input type="checkbox" bind:checked={permanentConfirmation} /> I understand
        permanent deletion cannot be undone</label
      >{/if}
  {/if}
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
    The Cask remains until it has no dependencies and you confirm its removal.
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
