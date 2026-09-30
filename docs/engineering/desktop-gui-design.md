# Desktop GUI design

Implemented 2026-09-30. See ADR 0018 for window and application decisions.

The main window uses a narrow navigation rail, page header with search and creation, three compact summaries, and a sortable inventory list. A row opens its dedicated native workspace without replacing inventory. Menus provide secondary actions. Global Installations, Activity, and Settings expose discovery, operation/recovery status, database backup, and diagnostics.

Each Cask workspace has a compact title/description, five tabs, an independently scrolling content area, and persistent Add Items / Create Cleanup Plan actions. Overview summarizes measured logical storage, counts, location, and dates. Files & Folders identifies managed, referenced, and shared associations, with locations and measurement status. Applications separates portable installations, registered associations, WinGet lifecycle, and reviewed installer artifacts. Advanced workflows are revealed deliberately through application-method choices. Activity queries durable state; Settings edits metadata, archives without deleting content, manages categories, and exports.

Resource intake explicitly offers file, folder, and reference selection and requires copy/move/reference review. Drops never imply a move. Cleanup shows the application's plan and warnings before approval; execution remains disabled in distributable builds pending independent review.

The visual system uses local Inter, charcoal controls, white/light gray surfaces, one-pixel neutral borders, 6–8 px radii, and Lucide outline icons. Green/yellow/red are reserved for state. Paths and names truncate with full values available through native browser tooltips. There are no browser responsive breakpoints, fabricated capacities, decorative charts, or permanent right-side inspector.
