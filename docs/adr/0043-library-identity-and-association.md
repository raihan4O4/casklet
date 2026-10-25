# ADR 0043: Library identity colors and direct application association

Status: Accepted under the October 25, 2026 main-window improvement request.

The later icon/emoji editor in ADR 0044 supersedes only the decision below to omit manual symbol selection; automatic base colors and association behavior remain.

## Decision

Choose automatic Cask colors (Option A), without an icon/emoji editor. Assign from a curated 12-color palette, preferring a stable-ID-derived starting point and a least-used color. Preserve every saved assignment; reuse colors evenly after exhausting the palette. Existing Casks receive defaults through the same path. Tint the entire collection icon tile, retaining readable theme-aware neutral glyphs. Names remain the accessible identifiers; color conveys neither ownership nor health.

Save the mapping under the local `casklet.cask-colors.v1` appearance preference, keyed by stable Cask ID. This is local UI state like the existing theme/accent preferences, not operational metadata. SQLite and manifests retain all lifecycle/ownership authority; no schema or contract change is needed. Preferences synchronize between windows through storage/Tauri events. Invalid preferences are ignored; save failure keeps session colors and reports a warning. These colors survive ordinary application restarts but are not portable manifest fields or database-backup content.

Replace global installation `Present` text with an Associate action. A native modal lists active Casks through the existing versioned ListCasks query and checks each destination through list_associated_applications. Existing links and unverifiable destinations are disabled. Explicit confirmation calls only associate_installed_application, preserving its reference/idempotency/revalidation behavior. Success updates the row, library and open target workspace; failure retains the destination for retry. No eligible Casks offers creation. Filtering clears the selection so an invisible target cannot be submitted.

## Reason and boundaries

The supplied picker reference establishes a quiet color vocabulary, but automatic assignment satisfies identification without adding customization complexity. Existing ownership, installation, provider, removal and native window behavior remain authoritative and unchanged. The header statistics become prominent, the redundant library label disappears, inventories gain shared insets, and navigation loses only the hover inset shadow while retaining its active marker.
