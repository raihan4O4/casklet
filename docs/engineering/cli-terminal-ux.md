# CLI terminal experience

The CLI presentation layer is confined to `crates/casklet-cli`. All command families
were reviewed: Cask, Resource (including folder contents and ownership), Category,
Storage, Portable, Installed, WinGet, Installer, MSI/applications, Export, Manifest,
Operation, Recovery, Diagnostics and Cleanup/Plan. Existing command spellings,
aliases, core requests, ownership rules and build capability gates remain intact.
`cask get` now also accepts `cask overview`.

Human inventories use borderless tables, falling back to labelled records when
columns do not fit. Console width is detected, with `COLUMNS` taking precedence
and an 80-column fallback. Unicode display widths and terminal control-character
sanitization prevent broken layout and injected terminal escape sequences.
Long values wrap without truncation. Empty results are explicit.

`--color auto|always|never` controls restrained headings and status colors. Auto
respects output redirection, `NO_COLOR` and `TERM=dumb`. Status words remain
readable without color. Progress is terminal-only, throttled, and sent to stderr;
JSON mode suppresses it. Slow operations emit a waiting status every ten seconds.
There is no fabricated percentage for opaque installer/provider work.

`--verbose` exposes IDs, revisions and support references. Human output keeps
hashes/plan IDs where required for a reviewed follow-up command. `--output json`
retains complete DTO fields and existing JSON shapes. Commands that previously
returned prose even in JSON mode now return a JSON object with a `message` field.
Errors remain JSON on stderr in machine mode; ordinary human errors show the
problem and a next step. Clap retains usage errors and spelling suggestions.

Cask lists and overviews use the shared metadata-only library query: no directory
scan or content hash is introduced into these views. Sizes are saved logical
measurements, unknown entries are explicit, and shared content is not presented
as independently reclaimable bytes. Existing live resource/storage inspection
commands retain their behavior. Cleanup shows each action/reason, preserved data,
failure count, expiry, approval token and next step. File execution prints the
current plan before confirmation, and piped/JSON invocation requires `--yes`.
The core still revalidates and journals every destructive step.

Two CLI-only dependencies are justified by terminal requirements: `terminal_size`
reads the actual console width through a safe portable interface; `unicode-width`
measures terminal cells, including wide and combining characters. Standard Rust
string lengths cannot supply either capability. Neither adds application policy,
GUI behavior, network activity, a service or a new runtime command.

## Manual testing (PowerShell)

Run from the repository root. Use an isolated debug data directory and an explicit
Alpha build, which supports reviewed file cleanup but does not enable WinGet
uninstall. Release builds use production data and do not honor this debug root.

```powershell
cargo build -p casklet-cli --features alpha-cleanup --locked
$env:CASKLET_DEV_ROOT = Join-Path $PWD ("target/cli-demo-" + [guid]::NewGuid())
$cli = Join-Path $PWD 'target/debug/casklet.exe'
New-Item -ItemType Directory -Path $env:CASKLET_DEV_ROOT | Out-Null
$note = Join-Path $env:CASKLET_DEV_ROOT 'notes.txt'
Set-Content -LiteralPath $note -Value 'Disposable CLI demo'
```

Help and creation; capture stable IDs using JSON rather than parsing a table:

```powershell
& $cli --help
& $cli resource --help
& $cli cleanup --help
$cask = & $cli --output json cask create 'Terminal Demo' | ConvertFrom-Json
& $cli cask list
& $cli --verbose cask list
& $cli cask overview $cask.id
```

Add a reference (files remain where they are), inspect ownership and storage:

```powershell
& $cli resource add $cask.id $note --mode reference
& $cli resource list $cask.id
$item = @(& $cli --output json resource list $cask.id | ConvertFrom-Json)[0]
& $cli resource get $item.id
& $cli storage summary $cask.id
& $cli category list $cask.id
& $cli installed list $cask.id
& $cli portable list $cask.id
& $cli installer list $cask.id
& $cli operation list --cask-id $cask.id
& $cli recovery status
```

Review-only preview: the reference is preserved and this command deletes nothing:

```powershell
& $cli cleanup create $cask.id
& $cli cleanup list --cask-id $cask.id
```

Optional real cleanup of **only the disposable notes file created above**. Explicit
ownership consent is necessary; preview, hash-bound approval and revalidation are
still required. This moves the file to the Recycle Bin and retains the Cask:

```powershell
& $cli resource ownership $cask.id $item.id --owned --confirm-ownership
& $cli cleanup create $cask.id
$plan = & $cli --output json cleanup create $cask.id | ConvertFrom-Json
& $cli cleanup approve $plan.id --hash $plan.planHash
& $cli cleanup execute $plan.id --hash $plan.planHash --yes --no-input
& $cli cask overview $cask.id
```

Output modes, application/provider display, narrow layout and errors:

```powershell
& $cli --color never cask list
& $cli --color always cask list
$env:COLUMNS = '32'
& $cli cask list
& $cli resource list $cask.id
Remove-Item Env:COLUMNS
& $cli --output json cask get $cask.id
& $cli --verbose resource get missing
& $cli --output json resource get missing
& $cli cask lst
& $cli winget capabilities
& $cli diagnostics backup
```

Typical wide-terminal inventory (timestamps and measurements vary):

```text
Casks (1)

Name           Status  Items  Known size  Unknown  Updated
-------------  ------  -----  ----------  -------  ----------
Terminal Demo  active  1      22 B        0        2026-10-22
```

Tests cover every command's help, cross-process inventory/ownership/cleanup,
JSON DTO preservation/errors, redirected output, terminal-control sanitization,
Unicode/narrow layouts, timestamps, sizes and explicit empty states. The shared
local CI command is `cargo xtask check`; its evidence is saved in
`target/cli-quality.log`. Manual provider installation/uninstallation is not part
of the terminal demonstration; it requires an appropriate disposable environment.

## Verification evidence (2026-10-22)

- `cargo xtask check` passed: formatting, strict workspace Clippy, all-feature
  workspace tests, default/Alpha profile tests, native workspace build, frontend
  formatting/lint/type checks, all 90 GUI regression tests, frontend production
  build and generated-contract consistency. The existing native Explorer test
  required access to its isolated temporary HKCU registry fixture outside the
  restricted sandbox. No Explorer or GUI source was changed.
- `cargo deny check` passed advisories, bans, licenses and sources, including the
  two terminal dependencies. Evidence: `target/cli-dependency-policy.log`.
- Final CLI-only strict Clippy and all-feature/default/Alpha tests were rerun
  after wording and rendering refinements; the Alpha debug CLI was built.
- The manual sequence passed against isolated debug data, including actual
  Recycle Bin disposal of the disposable notes file, preserved Cask overview,
  subsequent empty-Cask removal, diagnostics backup and deliberate invalid input.
  Evidence: `target/cli-manual.log`. WinGet correctly reported its unavailable
  execution alias on this host; no provider installation was attempted.
- No public release, installer rebuild or GUI redesign is part of this change.
  Existing release/security/manual accessibility gates remain unchanged.
