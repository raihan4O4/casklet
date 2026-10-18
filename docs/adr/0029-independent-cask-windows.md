# ADR 0029: Independent Cask windows

Status: Accepted by the October 12 window-minimization request.

## Decision

Cask workspaces are independent top-level native windows. This supersedes the native ownership relationship in ADR 0018: Tauri's Windows parent configuration creates an owned window, which Windows hides when its owner minimizes. Do not emulate independence by restoring or raising windows on minimize events.

Keep stable window labels, hidden startup readiness, main-relative initial positioning, theme inheritance, explicit restore/focus when reopening, and main focus/inventory notification when a workspace closes. When the main window is destroyed, explicitly destroy its Cask workspaces to preserve the previous close lifecycle. Ordinary top-level taskbar and z-order behavior replaces the owned-window always-above-owner behavior so users can switch windows independently.

Cleanup Review starts collapsed with item count and nonzero action counts, including preservation and permanent deletion. Its summary and named item effects expand through an accessible disclosure with reduced-motion support. Review/approval/execution controls and execution confirmations stay outside the disclosure. Review also reveals the items; expansion alone never acknowledges review. New plan hashes reset the disclosure. Existing hash-bound acknowledgement, capability, expiry, approval, execution and permanent-confirmation guards remain authoritative.

## Validation

All 51 GUI tests pass, including collapsed summary/disclosure, review-only restrictions, mixed category plans, approval/execution and interrupted cleanup recovery. Rust formatting and strict desktop Clippy, frontend ESLint/formatting, Svelte checks (zero errors/warnings) and production frontend build pass. Native main/Cask minimize, restore, repeated selection, workspace close and main close remain manual desktop checks; light/dark styling uses existing semantic tokens.
