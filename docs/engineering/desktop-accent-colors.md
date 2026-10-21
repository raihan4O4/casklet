# Custom desktop accent colors

The October 21 appearance extension stays within the existing presentation layer: Svelte stores, shared CSS roles, local WebView preferences and validated Tauri events. No dependencies, Rust commands, database migrations or domain changes are required. See ADR 0027.

## Token audit and behavior

The collection-desk teal literals were confined to the Light/Dark role declarations in `workspace.css`. Their consumers include primary actions, navigation, text actions, icons, selected controls, focus outlines, native checkbox/radio accents, resource editors, cleanup guidance and storage charts. The legacy `global.css` controls and Explorer intake also consume these shared roles. Theme preview miniatures retain schematic neutral colors; their selection outline uses the current accent.

`accent.ts` generates one custom palette, applied by `theme.ts` as root CSS properties. Filled actions preserve the exact chosen six-digit sRGB color. WCAG relative luminance selects the higher-contrast black/white foreground (at least 4.5:1); the hover fill moves away from that foreground. A derived accent ink adjusts toward black in Light and white in Dark until it reaches 4.5:1 on every shared neutral/selection surface. This ink also supplies control outlines and focus indicators, exceeding their 3:1 target. Pale/tinted selection backgrounds and storage segments follow the same hue. Primary controls retain a distinguishable outline even for white or black fills. Semantic danger/warning/success colors, supplied logos and stable Cask identity colors keep their separate meanings.

## Settings and persistence

Appearance offers six labelled presets, a native custom color chooser with a styled swatch, editable HEX, a live component preview and Reset to default. Valid choices update immediately; incomplete/invalid HEX drafts never change the active color. Invalid commits retain the last valid theme and explain the expected format. Color controls are keyboard accessible; the picker delegates full arbitrary-color selection to the platform chooser.

The optional `casklet.accent` preference is independent of `casklet.appearance`. It restores before mount, recalculates when Light/Dark changes and synchronizes open windows through `accent-changed` plus storage events. Incoming values use strict six-digit validation; receivers do not echo. Broadcasts serialize and skip superseded local choices. Storage failure keeps the session color and warns; synchronization failure is reported. Reset removes only this preference and its generated role overrides, restoring the original mode-specific palettes. Updates use the existing atomic, transition-disabled palette application and respect reduced motion.

## Validation

Tests independently calculate contrast across 216 RGB samples in each mode against the actual CSS surfaces, including action hover, text, focus and selection. Interaction tests cover arbitrary input, invalid drafts, restoration, reset, persistence failure, rapid changes and native/storage event handling/disposal. Production browser checks add extreme white/black/yellow/blue fills, reload persistence, reset and narrow reduced-motion Settings to the existing application regression audit. Native WebView checks exercise cross-window updates and restoration without touching user resources or executing cleanup.

Evidence and remaining platform limits are recorded in the project context. Native OS chooser appearance follows Windows; exhaustive screen-reader/forced-colors acceptance remains a separate manual gate.
