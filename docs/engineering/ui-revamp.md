# Collected workspace UI

Date: October 21, 2026. Concept, alternatives and presentation system: [DESIGN.md](../../DESIGN.md).

The redesign retains Svelte/Tauri, supplied logos, Inter, Lucide, fixed native windows, global appearance, typed IPC and all existing ownership/cleanup safeguards. It adds no application dependencies or production sample data.

## Presentation

- A quiet navigation rail and compact totals strip replace the dashboard-card emphasis. Search has its own toolbar. Five ordinary Cask rows fit in the initial window; longer inventories scroll inside the list.
- Detail Overview uses a single storage panel, plain item totals and visible dates. Measurement limitations remain available in an explicit disclosure, with incomplete state visible before expansion.
- Add Items is the primary workspace action. While intake is open, its footer action becomes Close Add Items, previously expanded details collapse, and the underlying resource inventory is temporarily hidden. Closing restores the list without registering anything. Reopening from another tab retains the pending review.
- File rows expose Managed/Reference and Shared relationships without requiring expansion. Paths and factual metadata use a larger reading size. Activity shows the affected Cask next to state; technical recovery tools follow recent activity.
- Cleanup navigation disappears while already in Cleanup. Its plan-creation control carries primary emphasis, and review-only builds explain that no cleanup will run. No review, approval, confirmation or backend eligibility checks change.
- Shared Light/Dark surface roles, hover/press/focus feedback, native title truncation/tooltips, reduced motion and floating notifications remain consistent. Main-window notifications are offset clear of the page title and primary header action.
- The same bundled Inter font is encoded as WOFF2 (350,484 bytes instead of 876,576). Glyph coverage and variable axes were compared after conversion. Logo PNGs remain unchanged; both are preloaded deliberately for immediate theme changes on local disk.

## Reproducible visual checks

Install Playwright and axe-core into an external tool cache, then run:

```powershell
corepack pnpm build
node scripts/ui-audit.mjs --tools C:\path\to\tool-cache --out .jury/desktop/final
```

The helper serves only the production bundle and injects a test-only, contract-shaped Tauri transport. Its data is fictional QA content and never imported by the application. It captures real compiled Svelte screens at 1024 × 720 and 560 × 760, both themes, empty/search/error states, menus, forms, intake modes, cleanup, long names, pixel scaling, keyboard focus and reduced motion. It checks sorting, menu Escape, theme switching, console errors, requests, horizontal overflow and WCAG A/AA rules. Browser paint metrics are fixture measurements, not native startup or installer evidence. The 125% capture changes pixel ratio, not logical text size.

Two independent screenshot reviews and subsequent refinements are recorded in `.jury/desktop/scorecard.md`; raw runs are under `.jury/desktop/pass-1`, `pass-2` and `final`. Website-only mobile/SEO requirements and the website award calibration do not apply to this desktop app. A static rest state is deliberate. Neither a jury-style estimate nor automated axe results certify a complete accessibility audit.

Relevant checks are the existing 73 GUI regressions, Svelte diagnostics, ESLint, formatting, production frontend build and a bundled native debug build. The installed application was not replaced during visual testing; no user data or real cleanup workflow was used.

## Final evidence

- All 73 GUI tests pass, including retained ownership/cleanup gates and the intake close/reopen regression. Svelte reports zero errors/warnings; ESLint and changed-file formatting pass.
- Final production capture: 42 states, zero axe violations, zero console errors or failed requests, no horizontal overflow, and visible focus treatment at 18 sampled inventory stops. The sampled reduced-motion state has zero running animations. Four-times CPU browser-fixture LCP was 504 ms with negligible CLS; this is not a native startup measurement.
- The bundled Tauri debug app was inspected with a fresh isolated data root under `target/ui-revamp-native`. Inventory, supplied theme logos, Light/Dark Settings, persisted Dark after an ordinary restart, and an independently listed Cask window rendered correctly. Native Tab/Shift+Tab focus and Enter-to-close intake restored the resource inventory. Diagnostic startup inspection was followed by a normal launch without browser arguments. The QA process was closed afterward.
- Independent screenshot reviews use strict website-derived criteria adapted to desktop. Final weighted score: 7.07/10. It is an estimate, not affiliated with Awwwards, and no website award-band offset is applied. Remaining critic suggestions concern terminology, the deliberately sparse initial resource chooser, long-title reveal and additional assistive-technology evidence. Management and application ownership terms retain their different domain meanings; truncated titles retain full accessible text and native tooltips.
- Evidence logs: `target/ui-revamp-tests-final.log`, `target/ui-revamp-native-final.log`, and `.jury/desktop/final`. Screen-reader, Windows logical text-size settings, precise native startup/flicker measurements and installer release gates are not certified by these checks.
- A subsequent user-requested Windows Alpha NSIS build includes the redesign. Nine source/checksum-bound release artifacts passed verification; local setup paths were refreshed. Packaging explicitly records the uncommitted working snapshot rather than claiming a clean release revision. Evidence: `target/ui-revamp-package.log` and `target/ui-revamp-package-verification.log`. Binary packages remain local ignored artifacts.
