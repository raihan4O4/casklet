# Casklet: collection workbench

## Brief and stack

Reimagine the complete local Windows lifecycle manager as a place to work with collections, not a dashboard to supervise. People organize activities, bring files and applications together, and eventually review what can safely leave. The explicitly authorized replacement supersedes improve-ui's identity-preservation and read-only recommendations. Its traceable-evidence discipline informs the audit. Awwwards supplies craft standards; jury supplies independent evidence review. Website SEO, marketing content, cloud publishing and theatrical loaders do not apply to this desktop application.

Keep Svelte 5, TypeScript, Vite and Tauri 2: the existing stack already connects every feature to the shared Rust core. Use local Inter, Lucide and supplied logos. No new production dependencies or invented content. Preserve the starting dirty tree, including the earlier resource-atlas functional work, .agents, skills-lock.json and brag-output.

## Analysis and capability map

| Surface              | Responsibilities retained                                                                                                                             | New presentation                                                                              |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Main library         | Metadata-only loading, polling/invalidation, cards/list, search, stable sorting, native Cask opening, creation                                        | Compact persistent rail; open statistics; separate library tools; collection-spine marks      |
| Global installations | Windows inventory search and refresh, publisher/version/scope/registration, lazy native icons                                                         | Shared inventory typography, neutral row surfaces and clear search toolbar                    |
| Activity             | Durable operations, installer outcomes, attention/recovery, display-history clearing, diagnostics                                                     | Same rail and page hierarchy; chronological entries with operational status                   |
| Global Settings      | Theme, custom accent, backup, diagnostics, build capabilities                                                                                         | Grouped settings panels; previews; consistent fields and action levels                        |
| Cask workspace       | Independent windows, native readiness, close/reopen lifecycle, drop target                                                                            | Shared vertical rail; separate identity header, content and persistent action footer          |
| Overview             | Known logical sizes, owned/reference distinctions, category accounting, refresh sizes, dates, direct contents navigation                              | At-a-glance context; measured storage panel; direct inventory shortcuts                       |
| Files & Folders      | Categories, nested/unknown sizes, folder inspection, resource overlay, category edits, explicit ownership changes                                     | Grouped inventory with consistent category surfaces and focused modal editors                 |
| Intake               | Multi-selection/drop/Explorer review, references, explicit dedicated-data consent, automatic categories, partial-result retry                         | Consequence-led choices, consistent fields and visible commit/cancel actions                  |
| Applications         | Registered association, MSI/EXE installation/elevation/recovery, portable ZIP and WinGet, search/relationship filters, launch/detach/reviewed removal | Shared inventory and workflow panels; retained real icons and readable relationship labels    |
| Cask Settings        | Revision-checked name/description/archive, categories, exports                                                                                        | Focused disclosure sections and persistent Reset/Save controls                                |
| Cleanup              | Scope/method, exact-plan review, hash-bound approval, confirmation, execution, saved history/recovery, whole-Cask finalization                        | Clear task context; explicit capability and consequence messaging; consistent review surfaces |
| Secondary UI         | Creation/removal/Explorer dialogs, menus, notifications, native controls                                                                              | Shared palette, radii, focus, borders, type and elevation                                     |

Presentation currently orchestrates typed IPC in MainWindow and Workspace. Domain policy stays in Rust. The redesign changes shell composition and shared tokens; it does not move policy into UI or alter database/schema/contracts. Automated browser fixtures remain test-only. Slow measurements stay explicit and independent of opening/navigation.

Main regression risks: narrow workspace content after adding a rail, active navigation visibility, modal focus, retained intake/installer state, accent contrast on new surfaces, selection identity and unchanged destructive guards. Test these through existing frontend tests and production-rendered workflows.

## Concepts considered

Scores: originality / product fit / feasibility / portability (lower portability is better).

| Territory | Idea and anchor                                                                             | Signature, emotion and screenshot                                                                                               | Scores        |
| --------- | ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- | ------------- |
| Material  | Collection workbench: resources stay distributed, but the Cask provides one working surface | A full-width contents-first workspace with inspectable relationships; capable and calm; contents and storage under one identity | 4 / 5 / 5 / 2 |
| Lifecycle | Departure board: each item advances from intake to reviewed retirement                      | Explicit status transitions; deliberate; review and preserved outcomes in the same task                                         | 4 / 4 / 4 / 2 |
| Formal    | Evidence register: organize the entire UI around source and relationship columns            | Dense metadata inventory; precise; comparable evidence rows                                                                     | 3 / 4 / 5 / 3 |

Choose collection workbench because everyday access to real contents should lead. Safety belongs beside consequential actions, rather than dominating normal navigation.

## Concept card

Idea: Everything for an activity, on one working surface.
Metaphor: A collection workbench with a clear index and focused inspection.
Signature: A Cask opens a native full-width workspace; contents lead and inspection never displaces the collection underneath.
Emotion: Oriented, calm and capable in three seconds.
Motion verb: Settle; brief inset, press and lift feedback. Still at rest is intentional.
Screenshot: Mineral library with a horizontal command header, opening into a contents-first native workspace.

## Architecture and design system

Main navigation is a full-width command header: Casks, Installations, Activity and Settings. Local storage remains a utility action. Independent Cask windows have identity, a horizontal section index, a scrolling task surface and persistent task actions. Overview leads with actual files/applications before measurement. Cleanup remains deliberate footer navigation and retains exact-plan Review / Approve / Execute transitions. The same shared component owns active state and Alt+1 through Alt+4, with modal suppression. Its historical NavigationRail filename remains to preserve existing consumers and tests; it now renders semantic horizontal navigation.

Light roles: mineral ground #f4f5f3, panel #ffffff, quiet surface #f0f2ef, ink #202936, secondary #566273, line #d9dfd9. Cobalt #315bb5 is the default action, retaining users' existing color vocabulary. Dark uses charcoal #202226, panels #282b30, secondary #2d3035 and pale cobalt #b6cbff. Semantic danger/warning/success remain distinct. Arbitrary custom accents continue to derive readable ink across every current surface.

Local variable Inter: library display 34/38, Cask identity 30/36, section 16/23, item identity 13 to 17, controls/body 13/20, secondary 12/19, compact metadata 11/17. Tabular numbers for sizes; names and source paths retain full accessible text. Spacing uses 4/8/12/16/20/24/32. Main inset 36, workspace inset 24, narrowing to 16. Panels 8px, controls 7px, modals 10px. Borders do the grouping; shadows belong to floating controls. Collection marks use a neutral tile and a small identity-color marker, never a safety signal.

Primary actions use solid accent. Secondary actions use borders; utility actions stay quiet. Active navigation uses a tinted field and baseline marker. Category inventories use white headers and a secondary open state. Application intake is a vertically ranked choice list instead of a grid of competing cards. Settings use shared bordered groups. Dialogs, menus, inspectors, notifications and cleanup inherit the same role tokens. Native selects/radios, visible keyboard focus, modal trapping and Escape/origin focus remain. Primary hit targets are at least 38px high. Feedback takes 120ms, entries 180ms, cubic-bezier(.2,.8,.2,1); reduced motion disables entry effects. Native scrolling stays native.

## Composition

```text
Library [brand / Casks / Installations / Activity / Settings / storage]
        [identity + Create                real library measurements]
        [library / search / view / sort]
        [collection identity + description | item count + known size]
        [scrolling collections; dense list available]

Cask    [close / activity identity / contextual menu]
        [Overview | Files & Folders | Applications | Settings]
        [actual contents / focused workflow / inspected results]
        [measurement and dates below contents on Overview]
        [Add Items or Save                              Cleanup]

Task    [context and consequence]
        [source / scope / options]
        [consent / validation / review / durable progress]
        [cancel or recovery                         current action]

Modal   [resource identity / close]
        [facts and focused category/ownership editor]
        [error or success / Cancel                          Save]
```

## Coverage and regression boundaries

Preserve batch reference/explicit ownership intake, automatic/manual category selection, duplicates and partial retry; resource overlay, nested folder inspection and explicit refresh; installer MSI/EXE/elevation/recovery, association, portable ZIP, WinGet, launch and reviewed removal; metadata/archive, categories, all exports, backup, diagnostics, recovery and saved cleanup history. Large/unknown/nested measurements retain their honest labels. The overview is navigation, never inferred deletion authority.

No new schema, IPC shape, Rust domain policy, native window lifecycle or capability changes. Review-only remains the default. Browser fixtures are QA-only and cannot prove provider or filesystem correctness. Focus, retained intake state, active navigation, narrow viewports, custom accent contrast, metadata-only loading and exact-plan destructive guards are primary regression risks.

## Genericness check

The design depends on Casklet's distinctive relationships: a Cask unifies distributed resources, referenced bytes count toward logical size, association never implies ownership, and cleanup requires evidence. Remove invented health metrics, fake capacity gauges and decorative illustrations. The signature is an actual inspected collection in a separate native window. No idle animation or scroll framework is needed for this utility.

## Validation

See docs/engineering/collection-workbench-ui.md for actual test results, independent critique and remaining release gates. This document defines the implemented presentation; it does not authorize public release.

## Review-driven final corrections

Independent evidence reviews led to shorter library chrome and cards, a combined toolbar, local file search, assigned-category glyphs, and one resource identity per editor. Cleanup item effects now stack filename, consequence and full saved path. Item-kept counts use natural singular/plural wording. The weakest accessory, repeated editor framing, was removed. Dead library-heading/editor-target styles were removed after their compositions changed. Actual checks and independent scores are recorded in docs/engineering/collection-workbench-ui.md; no score is treated as release approval.

## October 25 focused refinements

The dashboard uses a restrained shared panel with larger Cask count/known size and a 46px New Cask action. Item count and Includes references remain secondary at readable 12px. Search begins at the container edge without the redundant library label. Application lists use 16px horizontal insets. Hover changes navigation surface without a bottom shadow; the separate active marker stays. Collection tiles now show saved automatically allocated colors across their full surface, with neutral readable glyphs. The 12-color palette is local UI preference data, not ownership or operational metadata. Global installations offer a native Cask chooser that calls existing reference-association commands. See ADR 0043 and docs/engineering/main-window-improvements.md.

## Cask icon and emoji customization

ADR 0044 extends the automatic-color system with an optional compact picker during creation and explicit appearance editing in Cask Settings. Its Icons/Emoji switch, searchable 24-entry sets, 12 existing swatches, Automatic, Clear and live badge preview use shared surface/focus tokens. Creation stays collapsed until requested. Settings drafts require Save appearance; Cancel changes no preference. A shared CaskBadge keeps symbols and colors consistent across desktop collection/destination identities. The local preference pair is keyed by stable ID, preserves automatic defaults and survives restart. See docs/engineering/cask-appearance-picker.md for validation and profile-local limits.

## Library height allocation

The Casks page uses intrinsic header and toolbar rows followed by a `minmax(0, 1fr)` collection section. The collection reserves an intrinsic sorting row and a bounded internal scroll viewport. Title and New Cask share a header row, with the description below and secondary measurement notes inline when space allows. Compact nested padding preserves the prominent existing typography while returning space to actual collections. At narrow widths the summary stacks naturally; card styling is unchanged. No fixed listing height or positioning offset. Geometry and resize acceptance are recorded in docs/engineering/main-window-improvements.md.

New Cask now lives directly to the right of the search field in a shared responsive toolbar group, matching its 40px height. The summary header contains only title/description and measurements. This placement supersedes the preceding header-action arrangement while retaining bounded collection scrolling.


## October 26 supplied dashboard reference and density correction

The supplied image governs the Casks composition, illustration and icon treatment. Its raster dimensions do not set the application's logical-pixel density. The follow-up correction restores the original 1024×720 Windows window density: 64px navigation, 160px identity/measurement panel, 44px search/create/view controls, compact shared sorting and 130px cards. The collection retains 320px at this window size and only the listing scrolls. Grid and list share the same sorting row; list controls never stretch into data columns. Secondary screens and Cask windows retain their compositions. Theme and accent preferences remain independent. Details and actual browser/native evidence: docs/engineering/reference-dashboard-ui.md.


## Quiet collection elevation

The October 26 polish keeps the corrected logical-pixel density and supplied reference composition. Default cards have a faint neutral edge and two soft shadow layers; hover slightly raises the shadow, while the current Cask retains a deeper shadow and a subtle neutral surface gradient. Persistent selection has no accent outline. The explicit keyboard focus indicator remains distinct from selection. Card/list overflow menus are removed and their reserved space returned to content. Grid gaps are 14px with small scroll gutters to preserve shadow edges. Artwork reuses the supplied bitmap through an SVG luminance-to-alpha filter and feathered edge masks rather than compositing its opaque screenshot backing. Neutral controls/surfaces and existing configurable accents/identity colors keep their roles.


## Shared section scale and artwork fade

The final October 26 correction removes the dashboard-only navigation size rules entirely. Every main section uses NavigationRail's shared dimensions and typography; title size comes from shared h1 rules, and search/actions use the same 38px density as Installations. Library item names use inventory typography and 44px identity badges. The 160px summary and two-column card composition remain. No CSS zoom, host DPI or native window changes. The artwork's alpha feather extends across the lower 28% and wider side/top edges so its shadow is transparent before the crop boundary in both themes.


## Reference Cask cards

The supplied wave-card reference adds a 54px identity badge, 16px name and muted description above a fine divider, with count/date/storage aligned below. A faint two-layer wave at the upper right follows the saved identity color; white uses neutral ink and Dark reduces its strength. The existing card height, grid spacing, neutral selected elevation, no-menu behavior, shared section scale and header remain. List view has no decorative wave or divider.


## Reference Cask overview

The independent workspace identity places the 54px saved Cask badge beside a title/description column. Overview groups a full-width Total Items row above Files & Folders and Applications browse tiles, with neutral icon squares, fine separators and chevrons. Storage has its own icon, known-size total, composition bar/percentage, three legend rows, explicit Refresh sizes and category disclosure. Existing counts and storage calculations remain authoritative; the percentage is zero for no known bytes and otherwise represents the complete known-size breakdown, not disk use or reclaimable capacity. Add Items and Cleanup retain existing actions and guards. Dates remain available below the summary. All dimensions are logical desktop pixels, preserving the current main-window scale and theme/accent system.
