# ADR 0027: Desktop appearance preferences

Status: accepted by the user request, 2026-10-07.

## Decision

Offer explicit Light and Dark choices in global Settings. Light remains the default and retains existing component colors. Dark uses layered charcoal backgrounds, lighter neutral typography, distinct controls/hover/selection, and readable semantic status colors. Shared CSS roles use the original light colors as fallbacks so conversion does not silently restyle light mode.

Use the supplied transparent black and white PNG logos directly, without recoloring, filtering or redesign. The in-app sidebar and caption switch together; existing Windows deployment icons remain separate shell assets.

Persist the presentation-only preference in local WebView storage, separate from Cask domain data and SQLite. Tauri appearance events synchronize currently open windows; storage changes and startup restoration cover reopened windows. Validate received values and avoid event echoes. Serialize native updates to preserve the latest preference during rapid toggles. A storage failure retains the session choice and reports that persistence failed.

A typed caller-scoped shell command sets native window theme and WebView background. New Cask windows inherit their owner's native theme. Apply the saved DOM theme before Svelte mounts, and apply the native theme before revealing a window; retain the existing hidden-window readiness model. Theme changes do not touch filesystem, resource ownership, cleanup policies or release capabilities.

## Consequences

Global preferences affect main and detail windows. Palette changes apply atomically to avoid low-contrast intermediate colors; existing micro-interactions respect reduced motion. Native Windows-owned dialogs may follow OS conventions; Casklet's own dialogs and controls use the shared palette. UI theme storage is not included in Cask metadata exports or database backups. Full platform accessibility and clean-machine release gates remain separate from this feature.
