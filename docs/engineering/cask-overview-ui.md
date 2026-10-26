# Reference Cask overview

Implemented 2026-10-26 from the supplied git workspace reference. Presentation only; no Rust, schema, IPC, ownership or destructive-policy changes.

## Composition and behavior

The shared independent-window header uses a 54px saved badge beside title/description, with the existing workspace back/menu controls. Overview navigation uses the bundled Boxes glyph, leaving main Casks navigation unchanged. Other Cask tabs share the same identity framing so switching tabs does not rescale it.

Inside this Cask is one panel with real counts: Total Items spans the top; Files & Folders and Applications occupy the lower two cells. Neutral icon tiles and right chevrons retain the existing browse handlers. Total Items remains file-resource count plus the existing application-row count. Saved Cask names, descriptions, icons, emoji and colors are reused; no reference sample is written to user state.

Storage retains all current byte calculations and metadata-only loading behavior. Its header adds the storage glyph, caption and right-aligned known-size total. The existing segmented bar now has a percentage alongside it: 0% for zero known bytes, otherwise 100% of the known-size composition represented by the segments. Its tooltip explicitly distinguishes this from disk capacity or reclaimable space. Managed files, owned applications and references retain their separate values. Refresh sizes is still an explicit shared-core query with `refresh: true`; category disclosure retains navigation and group information. Dates remain available below the summary rather than being deleted.

Add Items and Cleanup keep their handlers and capability guards. The footer gains the reference's restrained button shape and light semantic danger fill, using existing theme roles and a Light fallback. The existing accent mechanism still controls filled actions and active navigation.

## Verification

- Production build, Svelte check (zero errors/warnings), ESLint and all 128 frontend tests pass.
- `scripts/cask-overview-audit.mjs` passes 12 empty/populated × Light/Dark × 390/560/1024px cases. It checks actual counts, non-clipped total-row geometry, no horizontal overflow, navigation from both browse tiles, explicit size-refresh IPC, category disclosure, Cleanup navigation and zero axe/console errors.
- Visual inspection caught an inherited `width: 100%` stretching the Total Items icon; scoped icon/text widths correct it. Scoped section padding also overrides an older bottom-padding rule, keeping the category row visible above the footer at the normal 560×760 workspace size.
- Isolated native Windows WebView checks create a real empty QA Cask, inspect both themes, navigate both inventory tiles and refresh zero-byte storage. Captures are settled after transient notifications are dismissed. Existing debug host serves current production assets; no installed release is changed.
- Evidence: `.jury/cask-overview-reference`, including `native/light-overview.png`, `native/dark-overview.png` and result reports. Production fixture content is test-only and excluded from the app bundle.

## Consistent spacing follow-up

Contents uses a balanced 12px outer inset/gap, 16px tile insets and 12px icon/text gaps. Total and Files now share identical icon/text left edges instead of the former 18px/14px and 14px/10px split. Storage uses 16px horizontal insets, matching the outer contents heading/context edges. Legend labels, Refresh sizes and category-disclosure text share a 22px leading column. The disclosure retains its triangle and keyboard behavior. Narrow layouts use matching icon/tile insets and allow long tile labels to wrap rather than collide with chevrons. No data, action, typography-scale or core change. Production build and all 12 empty/populated/theme/width cases pass, including measured icon/inset equality and accessibility. Evidence: `.jury/cask-overview-spacing`. Native validation uses the isolated debug host with current production frontend.

## Glyph centering correction

A legacy `.compact-stats span` flex rule overrode the generic icon-tile grid display, leaving folder/application glyphs and chevrons near the tile corner. A scoped override explicitly uses grid centering and block SVG layout for these tiles. Panel padding and label/count alignment stay unchanged. The production overview audit now measures horizontal and vertical glyph-center error (under 1px) in addition to existing column/inset checks. Evidence: `.jury/overview-icon-alignment`.

## Storage and date-card refinement

The Storage card keeps the 16px shared horizontal alignment while gaining a 20px top inset, consistent 16px spacing around its meter/legend, 10px legend-row gaps and a better-padded category disclosure. Existing data and actions are unchanged. Created/Last Updated now share a bordered panel with 16px insets, equal columns, a quiet divider, aligned clock labels and consistent value spacing. Legacy padding on the nested date list is explicitly cleared to prevent doubled top spacing. Both themes use shared surface/border/text roles. Production build and all 12 overview audit cases pass, including equal date top/bottom insets; native screenshots and browse/refresh checks pass. Evidence: `.jury/overview-storage-spacing`, with close views in `native/light-storage.png` and `native/dark-storage.png`.

## Refresh-row and bottom-border correction

An older workspace button rule added 8px below Refresh sizes, shifting its content above the midpoint between separators. The scoped refresh rule now has zero margin, a 44px centered row and a label span for direct geometry checks. The generic last-section rule also removed the date panel’s bottom border; the scoped section selector now retains its full border. No data or action changes. Production build and native visual checks pass. The overview audit now verifies icon/label center error under 1px between separators and a nonzero date-card bottom border. Evidence: `.jury/overview-row-border`.
