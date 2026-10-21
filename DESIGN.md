# Casklet: collected workspace

## Brief

Revamp the existing Windows productivity app while keeping its minimal aesthetic, exact supplied logos, Light/Dark preference, fixed 1024 × 720 main window and separate 560 × 760 Cask windows. Audience: people organizing the files and applications that belong to an activity. Success means finding the right Cask, understanding its contents and taking a safe next action without deciphering a dashboard.

Use existing Svelte 5, Tauri, local Inter and Lucide. Real application queries remain authoritative. No fabricated capacities, sample data in production, filesystem work in the frontend, changes to ownership or cleanup rules, or dependency-heavy visual engine.

## Concepts considered

Scores: originality / fit / feasibility / portability (lower portability is better).

| Territory              | Idea and anchor                                                                                                                                       | Signature / emotion / shareable frame                                                                                                            | Scores        |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | ------------- |
| Container identity     | Collected workspace: a calm index of activities, each opening its own focused workspace. Anchored in Casklet's Cask identity and independent windows. | A row quietly comes forward on focus, then its Cask opens fully initialized. Composed and trustworthy; inventory beside its dedicated workspace. | 4 / 5 / 5 / 2 |
| Resource relationships | Connection ledger: emphasize ownership/reference relationships through an annotated inventory. Anchored in resources existing outside their Cask.     | Relationship labels expand into location facts. Precise; one expanded resource.                                                                  | 4 / 4 / 4 / 2 |
| Formal system          | One reading column: typography replaces most panels and content follows a single baseline. Anchored in a deliberately small desktop viewport.         | Sections settle into one alignment. Quiet; an exceptionally clear dense list.                                                                    | 3 / 4 / 5 / 4 |

Choose collected workspace: it expresses the product's primary concept without making users learn another metaphor. Borrow the connection ledger's honest relationship labels only where they already carry domain meaning.

## Concept card

Idea: Everything for an activity, collected into one focused workspace.

Metaphor: A well-kept collection with an index and individual workspaces.

Signature: A subtly inset selected row and an initialized independent Cask window; no simulated browser modal or animated window clone.

Emotion: Clear, composed and immediately usable.

Motion verb: Settle. A 1–2 px response, quiet surface changes and brief opacity/translate transitions that never block interaction.

Screenshot: A compact inventory beside a focused Cask with clear resource relationships.

## System

- Light: ground `#ffffff`, navigation `#f5f6f7`, ink `#202327`, muted `#646c76`, line `#e7e9ec`, action `#25282d`.
- Dark: ground `#202226`, navigation `#24262a`, ink `#eef0f3`, muted `#a6adb7`, line `#3b4048`, action `#eceef2`. Existing warning, error and ownership roles remain semantic.
- Local Inter is retained deliberately: continuity with the existing Windows app, offline operation, strong small-size readability and the supplied brand. Page headings 30/36 px at 620; Cask titles 25/32 px at 620; section headings 16/22 px; body 13–14/20 px; metadata 12/18 px. Tabular figures for measurements.
- Grid: 192 px sidebar; 32 px content inset; inventory name plus 56/88/92 px data columns. Detail inset 24 px. Space scale 4/8/12/16/24/32.
- Radius: controls 7 px, surfaces 10 px, tiny states 4 px. No ornamental card stacks or shadows on ordinary panels.
- Motion: existing CSS/Svelte transitions, one `cubic-bezier(.2,.8,.2,1)` family, 100 ms press / 150 ms hover / 180 ms disclosure. Native scrolling, no scroll interception, GSAP, loader, idle animation or WebGL: continuous idle motion would distract from a local utility. Reduced motion removes movement and retains focus/selection feedback.
- State language: consistent visible focus, readable disabled controls, explicit empty/search/error states, one meaningful primary action per group. Safety warnings remain visible and unchanged.

## Composition

```text
Main:  [brand       ] [Casks                          New Cask]
       [navigation  ] [purpose                                ]
       [            ] [Casks / known size / items, one strip  ]
       [            ] [Search casks                  result # ]
       [            ] [Name                 Items Size Updated]
       [            ] [activity + description | measurements  ]
       [storage     ] [                                        ]

Detail: [close workspace                         actions]
        [identity / Cask name / description             ]
        [Overview   Files & Folders   Applications Settings]
        [Storage                              known size]
        [thin measured bar + honest breakdown           ]
        [item totals, single compact strip              ]
        [Created / Updated                               ]
        [Add Items                              Cleanup ]

Files: [section title + count]
       [resource rows / concise relationship labels]
       [expanded factual details and deliberate actions]

Apps:  [section title                  Add Application]
       [search field]
       [compact filters]
       [results with restrained ownership labels]

Activity: [date group]
          [icon | action / quieter state + timestamp]

Settings: [Appearance          Light / Dark]
          [local data / backup and diagnostics]
```

## Genericness test

Removed the repeated dashboard-card motif and decorative statistic icons. The distinctive moment is the real Cask workspace, not a marketing animation. Preserve meaningful names, byte accounting, reference/ownership distinctions and safe reviews. Do not add photography, new brand marks, giant display type, web navigation, decorative indices or an artificial capacity bar.

## QA adaptation

The invoked awwwards/jury skills target websites. Apply their craft, accessibility and evidence standards to this desktop app at its actual fixed window sizes, plus 125% text/scale and reduced motion. A 390 px phone layout, SEO/social metadata, public hosting and a marketing-site calibrated jury estimate do not apply. Production-bundle browser captures use a test-only typed IPC fixture, never production mock data; native build checks retain the real boundary. Run two evidence/review passes with fresh critics. Record measurements and limits honestly instead of claiming unmeasured FPS or native behavior from browser fixtures.
