# ADR 0018: Fixed desktop Cask workspaces

Status: accepted by the GUI implementation request, 2026-09-30.

## Decision

Use a 1024 × 720 logical-pixel main window and 560 × 760 Cask windows. Both are fixed, non-maximizable Tauri windows. A stable `cask-{id}` label ensures repeated selections focus the existing workspace. Secondary windows are owned by the main window, initially centered against it, and return focus on close. They are ordinary desktop windows, not web dialogs. Dimensions live in Tauri configuration and host constants.

Windows retains a disabled maximize caption button with native fixed-size decorations. Use a minimal draggable application caption with accessible minimize and close controls, while keeping native minimize, close, ownership, shadow, and taskbar behavior. Grant only the corresponding window capabilities.

Use the supplied PNG logo unchanged in branding and generate deployment icon sizes from it. Bundle Inter under OFL-1.1 and use Lucide Svelte outline icons under ISC. No network font dependency or decorative imagery is needed.

## Application boundary

The application supplies a typed library summary and metadata update command. SQLite schema 11 adds descriptions; optimistic revisions and mutation locks protect edits. Archiving changes lifecycle metadata only and preserves storage. CLI and GUI share this command. Manifest projections, recovery, and inventory exports preserve descriptions, including compatibility with old manifests without descriptions.

Library totals deduplicate stable resource and installation identities. Shared associations are queried from active associations; sharing never grants deletion eligibility. Known logical bytes are presented honestly: physical allocation, volume capacity, and registered application sizes remain unmeasured. Do not substitute illustrative reference data.

Filesystem intake, providers, export, recovery, and removal remain application-layer workflows. Progress events target their initiating window; durable operation queries remain authoritative after reconnection. Cleanup stays a reviewed immutable backend plan, with existing release execution gates intact.

## Consequences

There is one frontend entry point with inventory and Cask workspace routes. Main inventory and detail workflows evolve independently, and each workspace scrolls internally. Multiple open Casks increase WebView memory. Native keyboard, high-DPI, screen-reader, clean-machine, and installer release checks remain required.

## Startup refinement

Create windows hidden with a white native WebView background. Reveal through a caller-scoped typed transport command after initial UI data, styles, fonts, and layout are ready. This prevents exposing an uninitialized WebView and avoids a timer or overlay workaround. Hidden-window layout readiness must not rely on animation frames, which can be throttled.
