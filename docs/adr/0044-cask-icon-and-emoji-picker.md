# ADR 0044: Compact Cask icon and emoji selection

Status: Accepted under the October 25, 2026 icon-customization request. Extends ADR 0043 and supersedes its decision to omit an icon/emoji editor.

## Decision

Add one compact reusable picker with 24 curated Lucide icons, 24 named emojis, the existing 12-color palette, search, automatic color and a live badge preview. Creation keeps it optional and collapsed; Cask Settings offers Change appearance, with the draft committed or reset through the existing workspace Save changes / Reset changes footer (refined under the explicit October 26, 2026 request). Clear restores the collection symbol and automatic color. Search Enter never submits creation; Escape collapses the open picker before the containing dialog closes. Commit controls remain visible while expanded.

Store validated symbol and optional color overrides under `casklet.cask-appearance.v1`, keyed by the core's stable Cask ID. Keep existing automatic-color assignments intact under ADR 0043's key. Missing, corrupt or unsupported customization falls back to the collection symbol and saved automatic color. One local record atomically saves a symbol/color pair; blocked storage retains session presentation and reports that persistence failed. Storage and validated Tauri events synchronize open windows.

Use CaskBadge for library cards/rows, workspace identity, the installation association chooser, Explorer destination selection, creation preview and Settings. The supplied image guides the compact tab/search/swatch/grid composition; existing semantic surfaces and focus roles govern Light/Dark styling. Emoji remain real text using the Windows/system emoji font. Neutral icon glyphs preserve contrast over colored tiles. Names remain accessible identifiers; decorative badge symbols are hidden from assistive technology.

## Boundaries

This extends existing local desktop appearance preferences, not the operational domain. No new dependency, schema, generated contract, installer path, filesystem authority or ownership/removal behavior. CreateCask's typed request stays unchanged; preference saving uses the returned stable ID before the new native Cask window opens. Appearance remains local to the desktop profile and is excluded from metadata backups and portable manifests. Native Explorer shell-menu branding is unchanged.

## Shared Settings action refinement

Workspace owns the appearance draft alongside metadata. Appearance-only saves use local preferences; combined saves finish revision-checked metadata updates before appearance persistence. Failed metadata saves keep the draft for retry. Pristine appearance drafts follow saved cross-window updates; Reset restores the latest saved values. This supersedes the separate appearance action buttons while retaining explicit user commits, stable-ID preferences and the existing core/storage boundaries.
