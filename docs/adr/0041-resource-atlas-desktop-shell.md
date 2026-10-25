# ADR 0041: Resource atlas desktop shell

Status: Superseded in shell placement and visual composition by ADR 0042 on 2026-10-24. Original functional implementation remains recorded below.

## Decision

Replace separate main-window sidebar and Cask segmented navigation with one reusable NavigationRail. Main navigation retains Casks, Installations, Activity and Settings. Independent Cask windows retain Overview, Files & Folders, Applications and Settings, with explicit Cleanup task navigation in the footer. Cask identity, scrollable content and task actions occupy separate grid regions.

A neutral light ground, layered dark surfaces and cobalt default actions form the resource-atlas presentation system described in DESIGN.md. Existing custom accents remain supported. Alt+1 through Alt+4 use the same selection callback as pointer navigation and are disabled behind modal dialogs.

## Reason

The same location/selection vocabulary should apply across native windows. A compact persistent rail lets users recognize section changes without mixing navigation with operational actions. Keeping existing typed handlers preserves resource identity, metadata-only opening, provider lifecycle and destructive safeguards.

## Consequences

Narrow Cask windows have less horizontal content room, requiring explicit reflow and production audits of every task family. NavigationRail owns navigation semantics and keyboard handling; it owns no product data or IPC. Workspace and MainWindow remain the orchestration owners. No database, native lifecycle, ownership authority or capability-profile change is introduced. Still-at-rest presentation and existing native scrolling avoid unnecessary dependencies and motion.
