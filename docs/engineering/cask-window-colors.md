# Cask window color roles

## Design language

- Audited surface: independent Cask windows (Overview, Files & Folders, Applications, Settings, shared controls and resource dialogs).
- Design sources: user-approved accent-role requirements, DESIGN.md, existing theme/accent implementation and reference workspace cards.
- Documented decisions: active/selected controls use readable accent ink and accent surface; primary fills use the raw action color plus resolved foreground; secondary/informational icons stay neutral; destructive/error/success/warning roles remain semantic.
- Governing owners and consumers: App initializes theme and starts cross-window synchronization; theme.ts applies accentPalette to the document root; NavigationRail, Workspace, ApplicationLibrary and appearance controls consume these properties through workspace.css and motion.css. Icon SVGs use currentColor.
- Explicit exceptions: per-Cask identity colors and explicitly white badges are saved content appearance, not global selection accent. Third-party application artwork, emoji and artwork retain their content colors. Theme preview thumbnails depict their own theme.

## Findings implemented

| # | Problem | Evidence | Proposed change | Scope | Confidence |
| --- | --- | --- | --- | --- | --- |
| 1 | Applications selected filters were neutral while navigation and icon selection used accent | ApplicationLibrary renders chosen buttons and the shared indicator; reference-card CSS used text-heading/surface-secondary for those selected states | Use existing accent/accent-surface for chosen filter and its moving surface | Cask Applications only | High |
| 2 | List-level SVG rules overrode action foregrounds | global.css application-list/resource-list SVG selectors assigned text-muted to icons, including nested action buttons | Let button SVGs inherit the surrounding action's resolved foreground | Cask action buttons | High |
| 3 | Active disclosure and browse affordances remained indistinguishable from inactive neutral indicators | Workspace uses neutral category/settings chevron tiles and Overview browse chevrons for both resting and active interaction | Apply accent ink/surface to open disclosure chevrons and hovered/keyboard-focused Overview browse affordances | Cask disclosures and Overview | High |

## Improve first

Selected Applications filters had the clearest contradiction with active navigation and the user's explicit selected-state requirement. They now use the existing resolved accent roles, without changing filtering or the moving indicator's geometry/timing.

## Implementation boundaries

Corrections are scoped to `.detail-window`; there are no new hardcoded accent colors or palette copies. Informational heading/category/storage/date icons, application fallbacks and secondary actions remain neutral. Primary icons inherit action-text, including white/black accents where contrast chooses an appropriate foreground. Semantic danger/status colors and saved identity badges are retained. The audit found no need to change theme propagation: existing native events and storage synchronization already update open windows.

## Verification

Ten production theme/accent cases cover Light/Dark with purple, green, amber, black and white preferences. Assertions inspect actual SVG/control colors, moving filter surface, open file/settings indicators, Overview hover, primary/secondary/danger controls, checkboxes and selected appearance icons. Accessibility and console checks pass. Screenshots: `.jury/cask-accent-roles`. An isolated native main/Cask WebView test verifies accent changes propagate to the already-open Cask window in both themes, alongside real navigation/refresh smoke. All 129 frontend tests, lint, Svelte checks and build pass. Main-window styles are outside the new selector scope.
