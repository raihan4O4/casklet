# Shared UI motion

October 26, 2026. Interaction polish only; layouts, theme roles, saved preferences, provider capabilities and cleanup rules remain unchanged.

## Motion vocabulary

`styles/motion.css` is loaded last. Shared CSS tokens define 120ms immediate interactions/exit, 180ms state/disclosure changes and 200ms surface entry. The cubic-out curve has no bounce. Legacy duration aliases resolve to these tokens. Buttons, links, text fields, selection controls, navigation, badges, clickable rows, cards and disclosure chevrons interpolate paint/shadow or their existing small press/rotation changes. No hover changes dimensions or introduces scaling. Existing shadow-only list behavior is preserved.

`app/motion.ts` reads these tokens and the operating-system reduced-motion preference at each interaction, rather than capturing it on component creation. Svelte menus/toasts share floating-motion settings with only 2–3px travel. Dialogs use opacity-only entry/exit; outgoing Svelte elements are inert, native modal focus behavior remains, and the New Cask cancellation closes the native dialog after its outro rather than hiding it before the fade. Backdrops fade separately. Environments without Web Animations fall back to immediate transitions.

Native details use `::details-content`, intrinsic-size interpolation and discrete content visibility where supported. This requires layout during intentional expansion, but uses no JavaScript height measurements, persistent observers or frame loops. Unsupported runtimes retain functional native disclosure. Cleanup review's existing Svelte slide uses the same durations/easing. Native select menus remain platform-owned. Theme palette swaps intentionally disable interpolation to avoid low-contrast intermediate colors.

Reduced motion removes animations, transitions and smooth scrolling, and zeros shared durations. CSS responds immediately; menus/dialogs/toasts read the preference again when invoked. There are no new dependencies.

## Verification

All 128 frontend tests pass. Following the final dialog cancellation adjustment, the eight MainWindow/AssociateApplication tests pass again; Svelte checks report zero errors/warnings and ESLint/build pass. Production interaction tests cover Light/Dark with normal/reduced motion, dialog/menu open/close and focus restoration, rapid disclosure toggling, frame-sampled disclosure interpolation, instant reduced-motion expansion, and live preference changes while the app stays open. Twelve list theme/width/motion cases and Settings/Application theme/width accessibility checks also pass. Isolated native WebView creation, both themes, browse and refresh smoke passes against the production assets. No installed release was replaced.

Evidence: `.jury/shared-motion`, `.jury/list-motion`, `.jury/settings-applications`; native smoke evidence under `.jury/overview-row-border/native`. Screenshots establish final geometry; sampled frames and interaction assertions verify motion behavior. No physical-device FPS or full assistive-technology certification is claimed.

## Selection continuity and hover follow-up

Uses the animation-systems skill for continuity and feedback within the existing restrained product styling. `slidingSelection.ts` is shared by main/Cask NavigationRail and Applications filters. A decorative, pointer-transparent surface moves behind labels; the navigation underline travels with it. Selection semantics, shortcuts and filtering remain owned by their existing components.

The 200ms cubic-out movement uses a transform-only FLIP animation. Geometry is read when selection/layout changes, never in a production frame loop. Interrupted animations rebase from the currently displayed rectangle. ResizeObserver handles responsive layout/font changes and cleans up on unmount; unchanged observations do not cancel an active glide. Reduced-motion changes cancel/snap immediately. Indicators never receive focus, replace buttons, delay filtering, or consume clicks.

Overview browse tiles and Settings disclosure summaries gain soft elevation across the whole target. Hover transitions use the existing 120/180ms tokens; Settings no longer depends on heading-color changes. No scaling, decorative glow or list displacement is introduced.

The duration parser now supports both `200ms` and minified `.2s` CSS tokens. This fixes an observed production-only near-instant transition; a new unit test guards unit conversion and live reduced-motion behavior.

Choreography: selection/list content updates immediately; the shared surface glides to the new selection, with interrupted travel continuing from its current position. First paint and resize align without an entrance sweep. Hover depth builds and releases gradually without moving text.

Integration recipe:

```svelte
<div use:slidingSelection={{ key: filter, selector: 'button[aria-pressed="true"]' }}>
  <!-- Existing selection buttons and handlers stay here. -->
</div>
```

Validation: 129 frontend tests, lint, Svelte checks/build, production Light/Dark normal/reduced selection/hover audits, repeated changes during active movement, responsive indicator alignment, Alt-key navigation, live reduced-motion cancellation and native WebView smoke pass. Frame samples verify actual intermediate positions; screenshots confirm final styling. Evidence: `.jury/selection-hover` and `.jury/shared-motion`.

October 26 Settings hover boundary fix moves the Categories/Cask settings/Export disclosure hover surface from the inset summary to the outer card. Hover includes padding and expanded content, with no nested summary shadow. The selection/hover audit verifies outer-card shadow, absent inner shadow, and padding hover in both themes and motion preferences. Build and browser checks pass.
