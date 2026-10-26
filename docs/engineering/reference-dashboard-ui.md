# Supplied Casks dashboard reference

Implemented 2026-10-26 from the user-supplied 1500 × 1011 PNG. This is a presentation change to the main Casks screen, not a domain or architecture change.

The existing MainWindow owns library queries, filtering, sorting, creation and Cask opening. Its markup now includes the extracted reference illustration, summary glyphs, search shortcut hint, count beside sorting and card metadata. The existing PopupMenu gains an optional vertical trigger; library menus expose existing Open Cask and export handlers. Ctrl/Cmd+K focuses search outside dialogs. Cards remain real data with saved appearance choices; the reference's names, counts and symbols are test fixtures only.

The illustration asset is a crop of the provided reference, not a replacement illustration or placeholder. It retains the supplied pixels and uses edge masks to blend into the panel. Dark adapts it through inversion. Icons reuse bundled Lucide for layers, pie, calendar, terminal, storage, grid and ellipsis; the three-circle collection mark is traced as SVG because no identical library asset exists. Emoji glyph rendering remains OS-dependent. Existing emoji/color preferences are not migrated or overwritten to force sample appearances.

Dashboard neutral roles are scoped to the main window while Casks is selected. Primary actions, selected navigation, focus and hover retain the generated accent roles. Light plus custom #000000 produces the supplied black accent; Dark and custom color settings remain separate and persisted. No schema, Rust, IPC, ownership, provider or cleanup change. The native title bar remains, outside the reference content area.

## Verification

- Production Vite build passes with the existing bundle-size advisory.
- Svelte check: zero errors/warnings. ESLint passes.
- Frontend: 123 tests pass across 11 suites. The existing sorting/menu regression now verifies visible menu switching and Escape dismissal rather than asserting that menus are absent.
- `node scripts/reference-dashboard-audit.mjs` serves the production build with the test-only IPC fixture. It captures a 1500×1011 window body in Light/black, Dark/black and Light/blue, checks accent application, search shortcut and menu availability, and captures narrower widths. The full viewport includes the retained 32px native chrome.
- Reference geometry after correction: identity panel (32,117), 1436×277; toolbar (32,418), 1436×67; first card (33,574), 707.5×190. The supplied reference has a first card near (33,574), 709×191. Wide layout explicitly stays two columns, overriding the old three-column breakpoint.
- Captures are under `.jury/reference-dashboard/`. Direct visual inspection corrected the old three-column override, omitted artwork, metadata placement, oversized heading clearance and icon variants.

Browser fixture evidence verifies rendering and frontend interactions. It does not establish native Windows installer/provider behavior or a mathematically exact pixel match; source-image antialiasing and OS emoji differ. No installer or public-release claim. Existing release/security gates remain open.

## Density correction after native screenshot feedback

The first implementation incorrectly treated the supplied raster's sizes as CSS logical pixels and verified mainly at its large image dimensions. This inflated the native fixed 1024×720 window; its list view also inherited grid-column styling on the sorting row. The user's subsequent native screenshot exposed both problems.

The dashboard override block is replaced rather than accumulating another scale layer. Navigation is 64px with 13px labels, title 38px, summary 160px, statistics 40px, search/create/view 44px and cards 130px. The reference illustration and design composition remain. No CSS zoom, transform scaling, device-pixel-ratio hacks or native window-size change. Grid/list sorting explicitly uses flex with intrinsic-width buttons; list rows reserve menu space and align their own data columns. Incomplete measurement notes remain within the summary. Neutral secondary text is adjusted to pass contrast checks. This supersedes the earlier 1500px reference geometry described above.

Validation of the correction:

- All 60 cases in `scripts/library-layout-audit.mjs` pass across both themes, grid/list, five viewport dimensions and empty/four/forty collections. This retains the original acceptance criteria: at least 320px of inventory at 1024×720, four cards fully visible, bounded internal scroll, reachable final card, no accessibility/console findings or horizontal overflow. Failure output now includes geometry and accessibility targets. Evidence: `.jury/dashboard-density`.
- The updated reference audit checks the reported 1020×708 size at browser device scale factors 1, 1.25 and 1.5, both views, incomplete-size notes, summary bounds, intrinsic sort widths and at least 300px of inventory. Existing theme/accent/shortcut/menu checks pass. Evidence: `.jury/dashboard-reference-density`.
- An isolated debug Windows WebView loads the production frontend against real SQLite, creates four disposable QA Casks, opens/closes their separate native windows, then checks both themes and both views. Actual WebView dimensions are 1024×720 at system DPR 1, with a 320px collection viewport and all four cards/rows visible. No console errors or horizontal overflow. Production user state is untouched; the preexisting theme is restored before closure. Evidence and retained isolated state location: `.jury/dashboard-density/native/result.json`. Higher DPI cases are browser emulation, not claims of changed host DPI.
- This uses the existing native debug host with the current frontend served locally; it is not a rebuilt or installed release package. No Rust/schema/IPC or destructive behavior changed.

## Reference-led card and artwork polish

The subsequent request supersedes the visible library action menus and prominent accent selection. MainWindow removes the menus and their now-unused export-report handler/imports; report exports remain in the existing Cask workspace. PopupMenu's temporary vertical option and dedicated glyph are removed because no callers remain. Other screens' menus are unchanged. The active Cask button exposes `aria-current`; state still follows the existing native-open result.

Cards use the same faint neutral edge in default, hover and selected states. Two shadow layers provide tight contact plus diffuse elevation; hover and selected depths differ, with a subtle neutral selected gradient. Neither selected edge nor surface uses the accent. Dark uses the same hierarchy with black shadow roles and a light neutral surface mix. Keyboard focus retains the existing visible accent indicator. No decorative translation or independent badge lift. Saved appearance choices and configurable accent behavior are preserved. The 130px cards reclaim their former menu padding, have 14px grid gaps, and receive 4px side/top shadow gutters plus 12px lower breathing room. The 320px listing viewport at 1024×720 is unchanged. List rows reclaim the right menu gutter and use quiet neutral current-state treatment.

`CasksIllustration.svelte` keeps the original bitmap intact. An SVG filter converts grayscale luminance to alpha and fills the remaining artwork with the theme's heading ink; white screenshot backing becomes transparent. SVG masks feather outer sides and the clipped shadow edge. This replaces CSS multiply/screen/invert compositing and leaves no opaque rectangular backing. No generated replacement art, extra bitmap, dependency or Rust change.

Verification:

- Production browser state captures include default, hover, selected and keyboard-focused cards in both themes. Assertions verify current-state semantics, distinct resting/hover/selected shadows, unchanged neutral selected edges and absence of card menus in both views. Existing theme, custom accent, shortcut, reported-size and 100/125/150% DPI checks pass. Evidence: `.jury/dashboard-polish`.
- All 60 original layout/accessibility/scrolling cases pass with no console errors or horizontal overflow: `.jury/dashboard-polish/layout`. Four cards still fit at 1024×720.
- Isolated real Windows WebView captures and metrics pass for grid/list in both themes, with real disposable QA Casks, no console errors and a 320px inventory viewport: `.jury/dashboard-polish/native`. The existing native debug host serves the latest production frontend; no installed release was changed.
- Type check and ESLint pass; all 123 frontend tests pass. The prior sorting/menu test now verifies that both views have no action-menu triggers while preserving sorting checks.

## Shared section scaling and final image fade

Follow-up feedback identified that Casks still enlarged the shared shell when selected. This was caused by dashboard-scoped overrides, not WebView zoom or Windows DPI: brand 18px versus 16px, navigation labels 13px versus 12px, navigation glyphs 20px versus 17px, and controls 44px versus 40px. Casks also overrode h1 at 38px and search/action height at 44px. All dashboard-only navigation rules, including responsive variants, are removed rather than copied onto other sections. Dashboard-specific h1 size overrides are removed so the normal responsive h1 rules apply. Casks search/actions are 38px like Installations; names use 13px inventory typography and identity badges return to 44px. Statistics remain distinct measurement roles. No global zoom, DPI, window dimensions, theme preference, data or core behavior changes.

The prior artwork mask preserved full opacity until 93% of the crop height and then faded over its last few rendered pixels. The new multi-stop mask begins its lower fade at 72%, with progressively lower alpha at 82/90/96% before reaching zero at the boundary. Wider side/top fades remove the residual square shadow envelope. The bitmap and luminance-to-alpha renderer remain unchanged; the fade uses transparency so it composites into the actual Light/Dark surface rather than a painted color.

Verification:

- `scripts/reference-dashboard-audit.mjs` compares computed navigation, brand, navigation-label/icon, h1 and DPR values across Casks, Installations, Activity and Settings. Values match; Casks/Installations search height and input font also match. Full results: `.jury/dashboard-scale-blend/section-scale.json`. Existing interaction/state, responsive and device-scale checks pass. The same run captures the artwork alone in Light/Dark for edge inspection.
- Real isolated Windows WebView confirms identical Casks/Installations measurements: 65px navigation, 12px labels, 40px navigation buttons, 17px icons, 30px headings, 38px search, DPR 1. Both collection views/themes fit four QA Casks without overflow; the listing grows to 325px. Native captures/result: `.jury/dashboard-scale-blend/native`. Production user data/preferences are preserved. This uses the existing debug native host with the current production frontend, not a new installed release.
- All 60 layout/accessibility/scrolling cases pass: `.jury/dashboard-scale-blend/layout`. Production build, type checking, ESLint, formatting and all 123 frontend tests pass.

## Reference header proportions

The next supplied Light/Dark crops clarify the intended header balance: artwork should read larger and sit farther left, with more even breathing room around the statistics. The desktop hero retains its 160px height and 24px title inset; shared typography/navigation/action scales remain unchanged. Its grid now uses a 194px art track between 0.9fr description and 1.9fr summary tracks, with 16px horizontal gaps. A bounded intrinsic row centers the 194×160 SVG box without pushing content below the panel; the transparent edges let the illustration use the full header height. Statistics use 24px horizontal padding in both columns. Existing narrower-window adaptations remain.

The same source artwork and transparency rendering remain; the lower fade now starts at 80%, with smooth 86/92/97% falloff stops, preserving more of the enlarged foreground panel while still reaching zero opacity at the edge. Light and Dark use the same geometry and theme-derived ink. No main-navigation/card redesign, window-size change, data or core behavior change.

Production build, type check and ESLint pass. All 60 layout/accessibility cases pass, and reference/state/section-scale/device-scale checks pass. Isolated native grid/list checks and Light/Dark visual inspection confirm unchanged section sizing and a 325px inventory viewport. Evidence: `.jury/dashboard-header-balance` (including `native`). Native checks use the existing debug host and current production frontend, not an updated installed release.

## Clear original artwork contours

The latest reference explicitly requires complete, unfaded top and bottom panel edges. Earlier alpha masks were applied to the panels themselves, and luminance-to-alpha made their light material too transparent on Dark. The renderer now has two layers: the existing theme-ink shadow/backdrop retains its feather masks, while a silhouette-clipped image above it retains the original bitmap colors at full opacity. The clip follows the union of the three panels, excluding the screenshot backing. No gradient or theme wash touches this foreground layer. The original light/silver panels stay clear in Dark as well as Light. Existing 194×160 placement and all section/layout dimensions are unchanged.

Production build, type check and ESLint pass. The production browser audit passes its section-scale, card-state, theme/accent, responsive and device-scale checks. Isolated native grid/list checks pass in both themes with no overflow/console errors and the unchanged 325px listing viewport. Visually inspected native captures show full top/bottom contours: `.jury/dashboard-clear-artwork/native/light-grid.png` and `dark-grid.png`. Browser evidence: `.jury/dashboard-clear-artwork`. Native validation uses the existing debug host with the current production frontend; no installed release is replaced.

## Smaller artwork and dark material

The requested final adjustment reduces the SVG box from 194×160 to 174×144 (about 10%), centered within the same track so title/statistic positions do not move. A Dark-only CSS filter on the opaque panel layer inverts the original luminance, then balances brightness/contrast to charcoal panels with readable light contours and icon. Light retains the original image colors. Neither treatment changes opacity or fades the object edges. Production build, browser section/state/DPI checks and isolated native grid/list checks in both themes pass. Visual evidence: `.jury/dashboard-dark-artwork`, including `native`. Shared scale, header height and the 325px inventory viewport remain unchanged.

## Dark-only material and shadow refinement

The approved Light illustration is unchanged. Dark-only panel rendering now uses a grayscale SVG component transfer calibrated to the application's blue-charcoal surface family, replacing the generic inversion/brightness/contrast filter. Original gradients and contour/icon details remain visible at full opacity. The separate shadow layer uses black ink and 55% strength in Dark, replacing the pale heading-ink glow. No size, layout, accent or Light-mode rule is changed.

Production build and browser section-scale/state/interaction/DPI checks pass. Image comparison confirms `.jury/dashboard-dark-artwork/light-artwork.png` and `.jury/dashboard-dark-material/light-artwork.png` are pixel-identical. Isolated native grid/list/theme checks pass with unchanged geometry, no overflow and no console errors. Dark native capture was visually inspected: `.jury/dashboard-dark-material/native/dark-grid.png`. This uses the existing debug host with the current production frontend, not an updated installed release.

## Dark contact shadow

The requested grounding pass adds a soft black contact ellipse beneath the panels, shown only in Dark, and raises the existing black backdrop shadow strength from 55% to 85%. Panel material, opacity, size and placement stay unchanged. Light rendering is pixel-identical to the preceding approved capture. Production build and browser visual/state/section-scale/device-scale checks pass. Evidence: `.jury/dashboard-contact-shadow`.

## Original soft shadow below the artwork

The added contact ellipse was rejected as artificial and is removed along with its blur definition/styles. Dark now reuses the same bitmap-derived shadow shape as Light, softened with a 4px blur and shifted 3px downward. This affects only the shadow layer; panel material and sharp edges remain unchanged. Light is pixel-identical to the preceding capture. Production build and browser visual/section/state/DPI checks pass. Evidence: `.jury/dashboard-soft-shadow`.

## Supplied wave-card reference

The latest reference governs only the Cask card display. Cards retain the existing 130px height, two-column grid, 14px gap and neutral elevation. The identity row now uses a 54px badge and 16px semibold name, with the existing description beneath it. A fine divider separates that row from item count, calendar/update and right-aligned known size. Metadata retains its real values and unknown-size suffix.

A decorative, non-interactive two-path SVG wave sits at the upper right behind content. It derives color from the same explicit appearance/base-color precedence as CaskBadge and updates reactively with saved appearance preferences. White identities get a restrained neutral wave; Dark lowers wave strength. Archived identities use the same muted fallback as their badges. No sample names/icons are written to operational data. The decoration and divider are hidden in list view. Card menus remain absent, and existing open/selection/keyboard behavior and the main header are unchanged.

Validation: production build, type checking, ESLint and all 125 frontend tests pass. The 60-case layout/accessibility matrix passes, along with section-scale/state/theme/interaction/device-scale browser checks. Isolated Windows WebView grid/list checks in both themes confirm four full cards, a 325px inventory viewport, no overflow/console errors and unchanged shared navigation/title/search sizes. Captures and reports: `.jury/cask-card-reference` and its `layout`/`native` subdirectories. Native evidence uses the existing debug host with current production assets, not an installed release.

## Subtler card colors

At the user’s request, grid badges now use 16% saved-color fill and an 18% border mix instead of the stronger tint. Decorative waves render at 55% layer strength in Light and 40% in Dark; white identities use a 30% neutral-wave strength and retain their explicit white badge surface. No geometry, preference values, list-view or other-screen changes. Production build and browser visual/state/section-scale/device-scale checks pass; captures: `.jury/cask-card-muted`.

## October 26 list reference and motion

List view now uses individually rounded rows within the inventory container, neutral saved-identity icon tiles, name/description and aligned file-count/database-size/calendar-update columns with fine separators. At narrow widths the date moves below the other metadata. Real saved icons, sizes, unknown suffixes, dates, sorting and opening behavior remain. Persistent selection still has the same resting appearance. Grid presentation remains unchanged.

Shared interactive controls, cards and disclosures receive 170–200ms paint/state transitions only when reduced motion is not requested. Hover does not move list geometry or animate dimensions. Twelve production screenshot/accessibility checks cover both themes, three widths and both motion preferences, including hover geometry and reduced-motion duration assertions. Svelte checks, lint and production build pass. Evidence: `.jury/list-motion`.

October 26 list correction restores the same 16% saved-color badge surface and 18% border mix as grid view, including the explicit white choice. List hover now paints the containing row because the shared button rule forces transparent backgrounds; a 5% neutral tint and soft shadow work for both normal and selected rows. Known-size text uses the heading role to retain contrast on hover. The twelve-case list audit now compares badge colors across grid/list and asserts that selected-row hover actually changes the rendered surface without geometry movement. Build and all twelve theme/width/motion/accessibility cases pass.

October 26 shadow-only list hover replaces the fill/border highlight with the exact shared grid hover shadow. Selected/resting row backgrounds and borders stay unchanged; saved badge colors remain. The twelve-case list audit verifies grid-shadow equality, unchanged fill/border, stable geometry, both themes and reduced motion. Production build passes.
