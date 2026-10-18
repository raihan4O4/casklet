import type { RemovalPlanView } from "../contracts/generated/casklet";

export function planExpired(plan: RemovalPlanView, nowMs: number) {
  return (
    ["DRAFT", "APPROVED"].includes(plan.state) && plan.expiresAtMs <= nowMs
  );
}

export function cleanupStatus(
  plan: RemovalPlanView,
  reviewedPlanHash: string,
  nowMs = Date.now(),
) {
  if (planExpired(plan, nowMs)) return "Expired";
  if (plan.state === "DRAFT")
    return reviewedPlanHash === plan.planHash ? "Reviewed" : "Ready for review";
  if (plan.state === "APPROVED") return "Approved";
  if (plan.state === "COMPLETE") {
    if (plan.items.some((item) => ["UNKNOWN", "FAILED"].includes(item.state)))
      return "Closed with unresolved items";
    return plan.items.some((item) => item.state === "PRESERVED")
      ? "Completed; items kept"
      : "Completed";
  }
  return "Needs attention";
}
export function cleanupActionHint(
  plan: RemovalPlanView,
  removalEnabled: boolean,
  reviewedPlanHash: string,
  executionConfirmed: boolean,
  permanentConfirmation: boolean,
  nowMs = Date.now(),
) {
  if (planExpired(plan, nowMs))
    return "Create a new cleanup plan; this one has expired.";
  if (!removalEnabled)
    return "Approval and execution are unavailable in this build.";
  if (reviewedPlanHash !== plan.planHash)
    return "Review the items before approving or executing cleanup.";
  if (plan.state === "DRAFT") return "Approve this reviewed plan first.";
  if (!executionConfirmed || (plan.permanent && !permanentConfirmation))
    return "Confirm the cleanup actions before executing.";
  return "";
}
export function cleanupSummary(plan: RemovalPlanView) {
  if (plan.state === "COMPLETE")
    return plan.items.some((item) => ["UNKNOWN", "FAILED"].includes(item.state))
      ? "Cleanup closed. Unresolved items still need inspection."
      : plan.items.some((item) => item.state === "PRESERVED")
        ? "Cleanup finished with items kept. Their data remains on disk; inspect the planned actions."
        : "Cleanup finished. Remove the Cask once no dependencies remain.";
  if (plan.state === "PARTIAL")
    return "Cleanup stopped. Inspect the items that need attention.";
  if (plan.state === "EXECUTING")
    return "Cleanup is running. Wait for it to finish.";
  const effects = [
    ["TRASH", "moved to the Recycle Bin"],
    ["UNINSTALL", "uninstalled from Windows"],
    ["DETACH", "removed from this Cask with the data kept"],
    ["PRESERVE", "kept"],
    ["DELETE_PERMANENTLY", "permanently deleted"],
  ];
  return effects
    .map(([action, effect]) => {
      const count = plan.items.filter((item) => item.action === action).length;
      return count
        ? `${count} ${count === 1 ? "item" : "items"} will be ${effect}.`
        : "";
    })
    .filter(Boolean)
    .join(" ");
}
export function cleanupCounts(plan: RemovalPlanView) {
  return [
    {
      label: "Recycle Bin",
      count: plan.items.filter((item) => item.action === "TRASH").length,
    },
    {
      label: "Uninstall",
      count: plan.items.filter((item) => item.action === "UNINSTALL").length,
    },
    {
      label: "Data kept",
      count: plan.items.filter((item) =>
        ["DETACH", "PRESERVE"].includes(item.action),
      ).length,
    },
    {
      label: "Permanent deletion",
      count: plan.items.filter((item) => item.action === "DELETE_PERMANENTLY")
        .length,
    },
  ].filter(({ count }) => count > 0);
}
export function cleanupAction(action: string) {
  return (
    (
      {
        UNINSTALL: "Uninstall from Windows",
        TRASH: "Move to Recycle Bin",
        DETACH: "Remove from this Cask; keep the data",
        PRESERVE: "Keep this item",
        DELETE_PERMANENTLY: "Permanently delete file",
      } as Record<string, string>
    )[action] ?? action
  );
}
export function cleanupItemEffect(item: RemovalPlanView["items"][number]) {
  if (item.resultCode === "REVALIDATION_FAILED")
    return "Item changed or could not be verified; nothing removed. Create a new plan.";
  if (
    ["TRASH_OUTCOME_UNCERTAIN", "DELETE_OUTCOME_UNCERTAIN"].includes(
      item.resultCode ?? "",
    )
  )
    return "Removal could not be confirmed; inspect the original and recovery locations.";
  if (item.reasonCode === "APPLICATION_OWNERSHIP") {
    if (item.state === "RUNNING") return "Uninstalling...";
    if (item.state === "FAILED" || item.state === "UNKNOWN")
      return "Uninstall failed; ownership retained";
    if (item.action === "DETACH")
      return item.state === "SUCCEEDED"
        ? "Reference removed; application remains installed"
        : "Remove reference; leave installed on Windows";
    if (item.state === "PENDING")
      return item.action === "UNINSTALL"
        ? "Waiting to uninstall"
        : "Keep installed; verification required";
  }
  if (item.state === "SUCCEEDED") {
    return (
      (
        {
          UNINSTALL: "Uninstalled from Windows",
          TRASH: "Moved to the Recycle Bin",
          DETACH: "Removed from this Cask; data kept",
          DELETE_PERMANENTLY: "Permanently deleted",
        } as Record<string, string>
      )[item.action] ?? "Finished"
    );
  }
  if (item.state === "PRESERVED") return "Kept";
  if (item.state === "UNKNOWN" || item.state === "FAILED")
    return "Needs attention; inspect Advanced details";
  return cleanupAction(item.action);
}
export function cleanupReason(reason: string) {
  return (
    (
      {
        NO_DELETE_AUTHORITY:
          "This association grants no authority to delete the data.",
        SHARED: "Another registered resource overlaps or depends on this data.",
        UNVERIFIED:
          "The recorded identity or complete contents could not be verified. Data is preserved.",
        ELIGIBLE_MANAGED:
          "Recorded ownership, current identity, and absence of other registered dependents allow cleanup.",
        PERMANENT_DIRECTORY_UNSUPPORTED:
          "Permanent deletion is unsupported for directories; use a Recycle Bin plan.",
        PERMANENT_RESOURCE_UNSUPPORTED:
          "Permanent deletion is unavailable for this resource.",
      } as Record<string, string>
    )[reason] ?? reason.toLowerCase().replaceAll("_", " ")
  );
}
