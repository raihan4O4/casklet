# ADR 0042: Collection workbench desktop experience

Status: Accepted under the explicit complete-redesign request, 2026-10-24. Supersedes ADR 0041's rail placement and resource-atlas visual composition.

## Decision

Use full-width horizontal navigation in main and independent Cask windows. Separate identity, navigation, scrolling contents and persistent task actions. Lead Overview with browsable files/applications, then show honest saved storage measurements. Use a mineral Light ground, layered charcoal Dark, local Inter, neutral collection markers, restrained borders and the existing configurable action accent. Replace the application-method card grid with one ranked choice list.

Retain the existing shared NavigationRail component's public contract and shortcut handling while changing its rendered structure. The historical filename avoids disrupting the pre-existing dirty-tree consumers and tests. Add compact presentation for native Cask windows. Navigation owns no IPC or domain state; MainWindow and Workspace retain orchestration.

## Reason

The vertical rail reduced scarce space in the 560px Cask window. Full-width contents improve inventory, consent and cleanup readability. Common active-state vocabulary and shortcuts unify the windows without forcing identical header sizes. The new composition replaces the previous layout while preserving verified operations and safety state.

## Consequences

At narrow widths the main navigation moves to a dedicated second header row; workspace navigation remains a four-column section index. Window controls, independent windows, custom accents, metadata-only opening and exact-plan safeguards remain. No new production dependencies, schema, capability gates or Rust policy changes. Production visual/accessibility audits and existing regression suites are required. Existing native/public-release gates remain open.
