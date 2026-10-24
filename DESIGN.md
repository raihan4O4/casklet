# Casklet: collection desk

## Brief and scope

Reimagine the complete existing Windows activity/resource manager as a clear collection of activities and focused workspaces. The audience is people managing project files, installed tools and their eventual cleanup. A Cask groups resources by purpose without claiming that references belong to it. The interface must explain that distinction and keep every existing core command and safety gate.

Use the existing Svelte 5 / TypeScript / Vite / Tauri 2 stack: it already serves every production workflow through typed Rust IPC. Retain supplied Light/Dark logos, local variable Inter and Lucide. No production sample content, new application dependencies, filesystem access from JavaScript or changes to ownership, journals, provider permissions or native window lifecycles.

The working tree initially contained untracked `.agents/` and `skills-lock.json`; preserve these user-provided files.

## Audit

Existing screens: main Casks library, installation inventory, Activity with recovery tools, global Settings, independent Cask Overview, Files & Folders and inline resource inspectors, Applications with installer/existing/portable/WinGet methods, Cask Settings/categories/exports, Cleanup (file and application queues), creation/removal/Explorer destination dialogs, popup menus and floating notifications.

The preceding design was functional but relied on a dense table, tiny undifferentiated controls and sparse initial intake. Overview totals were passive, long workspace titles were single-line truncated, and navigation/readouts competed at similar weight. Preserve the underlying information architecture and redesign composition rather than inventing new product modules.

## Concepts considered

Scores: originality / product fit / implementation feasibility / portability (lower portability is better).

| Territory | Concept and anchor                                                                                                                                                        | Signature, emotion, screenshot                                                                                                            | Scores        |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- | ------------- |
| Identity  | Collection desk: named activities are collections, with their own workspace and real resource relationships. Anchored in Cask identity and independent native workspaces. | Tinted collection marks settle on hover; each card opens a real workspace. Calm and capable. Collection cards beside a focused inventory. | 4 / 5 / 5 / 2 |
| Process   | Lifecycle path: acquisition, use and deliberate retirement form the visual hierarchy. Anchored in durable intake and removal journals.                                    | An explicit plan/review/approval handoff. Reassuring, but risks overemphasizing deletion during ordinary use.                             | 4 / 4 / 4 / 2 |
| Formal    | Resource register: a compact ledger with exact counts, paths and relationship labels. Anchored in SQLite associations and cautious accounting.                            | Inline disclosures reveal facts. Precise, but too close to the previous table-centric experience.                                         | 3 / 4 / 5 / 3 |

Choose collection desk. It gives everyday organization priority, with lifecycle safety expressed at the point of action. The resource register remains available through List view.

## Concept card

Idea: Everything for an activity, in one collection with a deliberate next action.

Metaphor: A collection desk and its focused workspaces.

Signature: A collection mark responds to focus/hover; a card opens the fully initialized native Cask workspace. Overview counts lead directly to the relevant contents.

Emotion: Composed, readable and trustworthy within three seconds.

Motion verb: Settle. Short 1–4 px responses and surface changes. No movement while idle.

Screenshot: Purposeful collection cards, matching the clear segmented navigation and measured storage of a Cask workspace.

## Visual system

- Light roles: ground `#ffffff`, navigation `#f2f5f5`, ink `#182c30`, muted `#586e73`, line `#dfe7e7`, action/accent `#235d65`. Petrol expresses organization and active controls; semantic error/warning/success colors remain separate. Identity colors derive from stable Cask IDs and do not mean health or ownership.
- Dark retains native charcoal ground `#202226`, layered surfaces, readable pale text and deliberate accent `#b1d8df`. Theme changes commit the whole palette together to avoid unreadable intermediate fades.
- Type: bundled variable Inter, offline, continuing the established Windows product identity. Library heading 30/36 px; workspace heading 26/34 px, wrapping up to three lines; storage 30 px with tabular figures; section heading 17/24 px; operational body/controls 13/20 px; metadata 12/18 px; short secondary labels 11 px. No network font requests.
- Rhythm: 4/8/12/16/24/32 px tokens. Main rail 188 px, content inset 30 px; workspace inset 24 px (20/16 px on smaller screens). Two-column collection grid with 14 px gutters, plus compact sortable list view.
- Shapes: 8 px control and 12 px panel radii. One-pixel borders; floating elements and hovered collections alone receive shallow shadows. No gradients, glass, photography or decorative charts.
- Motion: `cubic-bezier(.2,.8,.2,1)`, 120 ms feedback, 180 ms content entry/disclosure. CSS and the existing Svelte transitions avoid a new animation dependency. Native scrolling and window readiness remain unchanged. Reduced motion stops animations/transitions and preserves all content.
- States: visible focus, disabled controls, explicit selection, progress/status announcements, actionable empty/search/error views. Cleanup review, approval and execution remain separately gated by exact hashes and core capabilities.

## Composition

```text
Library  [brand / nav] [title / description                 New Cask]
         [          ] [Casks / known logical storage / item totals]
         [          ] [Search                   count   cards/list]
         [          ] [Sort by                 Name Items Size Date]
         [          ] [collection + description] [collection + description]
         [local data] [items / bytes / updated ] [items / bytes / updated ]

Cask     [back / workspace context                           actions]
         [wrapping name / activity description                    ]
         [Overview | Files & Folders | Applications | Settings     ]
         [measured storage / proportions / explicit limitations   ]
         [total count                                             ]
         [browse files + count] [browse applications + count       ]
         [created / updated                                       ]
         [Add Items                                        Cleanup]

Intake   [task / close]
         [folder symbol / concise instruction]
         [Add File       explanation                      forward]
         [Add Folder     explanation                      forward]
         [then: source, explicit management/reference choice,
          category, required dedicated-data consent, commit]

Apps     [heading + count                         Add Application]
         [search                              relationship filters]
         [method choice or focused workflow]
         [app / publisher / relationship / deliberate row actions]

Cleanup  [heading / return]
         [visible capability notice]
         [scope / deletion-method choice]
         [generate / compact review / separate guarded actions]
         [confirmation / diagnostics / history]

Settings [appearance description       accessible preview choices]
         [local data / backup / diagnostic explanation and actions]
Activity [date groups / action / affected Cask / status / disclosure]
```

## Genericness test and assumptions

The collection cards represent actual Casks, rather than generic feature tiles. Their known bytes, references, deterministic identity and independent workspace behavior are product-specific. Keep a list option for scanning larger libraries. No fake capacities or generated artwork. The theme previews are small control diagrams, never purported application screenshots. Idle motion is intentionally absent in a local productivity utility.

Existing fixed native 1024 × 720 and 560 × 760 windows remain supported. Responsive CSS additionally composes narrow 768/390 px browser previews and larger logical text without changing native window policy or promising a mobile release. Website SEO, public hosting, loaders, GL, OG assets and award calibration are inapplicable. The invoked skills supply craft and evidence standards, not new product scope.

## QA contract

Build production, run existing GUI tests and new navigation/layout regression checks, Svelte diagnostics, ESLint, formatting, then inspect production-bundle captures with isolated typed IPC fixtures. Run two complete visual/accessibility passes and a fresh independent screenshot critic each time. Extend captures to both collection layouts, every application method, ownership/reference intake, cleanup review states, narrow viewports and reduced motion. Browser fixtures never touch user data and cannot certify real installer/UAC, filesystem cleanup, native startup, screen readers or public release readiness. Record actual results and limits in `docs/engineering/ui-revamp.md` and `.jury/collection-desk/`.

## Resource management and Settings refinement

User-requested follow-up: the Files & Folders disclosure and its dropdowns/buttons, plus both Cask and global Settings. Retain the existing Svelte/Tauri stack, palette, real core commands and safety boundaries.

Three scoped concepts were considered: a resource sheet (facts and policy rows; originality/fit/feasibility/portability 4/5/5/2), a separate inspector drawer (4/3/3/3; too costly in a 560 px native window), and an always-editable form (3/3/4/4; weak separation between reading and committing policy). Choose the resource sheet: it makes current facts visible and opens one focused editor at a time without changing the current route.

Concept: a resource has a readable location/status record, deliberate category and ownership controls, and a separate cleanup preview. Its signature is the inline row opening into an explicit editor, with keyboard focus moving in and returning on cancel. The emotion is clarity and control; the motion verb remains settle. A representative frame shows the category row expanded with a full-width native select and aligned Cancel/Save actions.

Composition:

```text
[resource name / size / disclosure]
  [Management details                         close]
  [Location / wrapped full path                    ]
  [Size             Availability        Management ]
  [Category / current value                  Change]
    [label / native select with clear arrow        ]
    [effect explanation              Cancel / Save]
  [Ownership / current policy                Change]
    [Owned by Cask / Reference, explicit effects    ]
    [dedicated-data consent          Cancel / Save]
  [bounded folder contents, when applicable        ]
  [cleanup explanation                    Preview ]

Settings
  [Identity / description / reversible archive     ]
  [save explanation                        Save   ]
  [Category list / name                   Create   ]
  [Export / readable report and portable inventory ]
  [machine paths behind an explicit disclosure     ]
Global
  [Appearance / saved choices                      ]
  [Local data / backup row / diagnostics row        ]
  [real build cleanup capability                   ]
```

The six shared palette roles, Inter/Lucide, 8/12 px radius rules, 12–16 px operational text and short reduced-motion-aware settle timing are retained. New forms have consistent full-width fields, distinct explanatory copy, explicit commit/cancel actions, radio policy choices and native select behavior. No custom dropdown library, additional modal, decorative illustration, idle movement or production placeholder is introduced. Native select keyboard/platform behavior is preferable to an inaccessible simulated menu. The cleanup action is visually separate from ordinary metadata edits. Category creation guards pending requests and retains the name on failure.

QA extends production captures to category/ownership editors, Escape focus restoration, folder descendants and category/export Settings. Existing safety regressions remain required; screenshots and independent reviews live under `.jury/management-settings/`.

Final scoped refinements: editor and metadata commit controls live in the persistent native workspace footer, using real form associations. Reading details steps aside during an edit; the active resource name remains visible. Escape returns focus. Ownership opens with the current policy selected. Category creation is immediate with inline feedback; metadata Reset/Save applies only to its own dirty fields. The final accessory reduction removes decorative category pill surfaces in favor of passive category text.

## October 24: Category shelves and batch intake

Brief: retain the local-first Svelte 5/Tauri design and the existing configurable accent. Organize actual resource metadata, keep opening lightweight, make selection-wide consent explicit, and keep independent windows alive. No new motion engine, theme or dependency is needed for this scoped change.

Concept alternatives (originality / fit / feasibility / portability, 1-5, lower portability is better):

- Category shelves: activity resources arranged by their assigned role; signature is opening a category to its real items. Calm organization; screenshot is the grouped inventory and matching compact storage breakdown. Anchor: Cask-resource category assignments. 4/5/5/2.
- Storage ledger: accounting-style totals first; signature is expanding byte contributions. Precise but overemphasizes measurement over activity management. Anchor: saved logical resource sizes. 3/4/5/3.
- Selection tray: focus the selection and progress; signature is a mixed selection flowing into categories. Useful for intake, weaker as the organizing system. Anchor: shared ownership consent for one selection. 4/4/4/3.

Chosen concept card: Category shelves. Idea: keep an activity's resources organized by purpose. Metaphor: labeled shelves, each with a count and saved size. Signature: the same used categories organize storage and file navigation. Emotion: calm and predictable. Motion verb: open and settle, using existing disclosure feedback. Screenshot: Files & folders with two or three distinct populated category groups.

System: reuse current Inter, configured accent and semantic foregrounds; ground #ffffff/#202226, ink #17333b/#e6edf0, line #dbe5e8, default accent #235d65. Existing 20px workspace gutters, 8/12/16px rhythm, restrained borders and 8px controls remain. Numeric sizes use tabular figures. Desktop and narrow layouts share aligned labels/counts on the left and sizes on the right; long names wrap. Overview stays summary-first with a collapsed category breakdown; inventory uses initially collapsed native details; Settings uses initially collapsed native details. Existing hover/focus/press states and reduced-motion rules remain. No idle animation: this is a file-management workspace.

Composition: Overview [total + existing segmented bar] / [Refresh sizes] / [collapsed Storage by category]; Files [title + count] / [category + count | saved size | chevron] / [item rows]; Intake [selection count + inspectable path list] / [ownership choice + explicit consent] / [automatic category] / [action + results]. Genericness check: categories and ownership are existing persisted Cask semantics, not a decorative taxonomy or generic dashboard. Unknown and nested sizes remain explicit rather than fabricated totals.

The category readability refinement uses bounded panels with a secondary-surface header, 14px category identity, 13px filenames/amounts and 12px secondary facts. The same assigned-category Lucide mapping marks headers, items and Overview rows; no per-category palette or extension-based classification is introduced. Manual expansion survives updates within the mounted file view, while fresh entry resets all groups closed. Focus restoration after recategorization may deliberately reveal the destination group.
