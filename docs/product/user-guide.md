# Casklet user guide and safety

Casklet groups an activity's files and applications in a Cask. Create a Cask in the desktop app or with `casklet cask create <name>`. A Cask has a stable ID; its name can be reused. Development builds store data under `.casklet-dev`; release builds use `%LOCALAPPDATA%\\CaskletData` for the database and `%USERPROFILE%\\Casklets` for managed files.

## Add and inspect resources

The fixed main window opens to **Casks**, with search, sortable inventory, and **New Cask**. Selecting a row opens a separate desktop workspace; selecting it again focuses that workspace. Use its back arrow or close control to return to the inventory. Both windows provide minimize and close controls without maximize.

Each workspace has **Overview**, **Files & Folders**, **Applications**, **Activity**, and **Settings**. Add items or generate a cleanup plan from its bottom action bar. Cask Settings edits the name and description and can archive a Cask without removing its resources. Global sidebar Settings provides database backup and diagnostics; global Activity includes interrupted operations and manifest recovery. Installed applications are discovered through the Installations page and associated from a Cask's Applications tab.

Storage summaries show known managed logical sizes, not invented volume capacity or installed-application measurements. Shared labels describe multiple active associations and never imply cleanup ownership.

Choose or drop one file or folder. Review **Copy**, **Move**, or **Reference** before pressing **Add resource**. Copy retains the source; Move changes its location after verification; Reference leaves it in place and never grants deletion ownership. Inspect the resource's availability and size in the Cask detail. Storage values distinguish managed, referenced, physical, and reclaimable estimates; unknown values are shown as unknown.

Portable ZIPs are inspected before installation. WinGet packages require explicit review. Existing Windows applications are associated as references only. A registered MSI/EXE installer is copied and inspected; launching it is an explicit separate action. A resulting Windows registration is a candidate for review, never automatic ownership.

## Export, recovery, and cleanup

Export portable JSON for sharing or a readable report. Exports contain paths and application details, so inspect before sharing. The recovery area lists interrupted operations; inspect source, staging, and destination before choosing recovery or **Keep files and close**. Use **Back up database metadata** in Settings or `casklet diagnostics backup` to make a consistent local SQLite backup. It contains metadata only, never managed files. If startup reports database corruption, preserve the database and its backup and use local manifests for conservative recovery; do not replace the damaged file in place.

Cleanup preview lists every proposed action and reason. Referenced, shared, changed, and uncertain items are preserved. Approved plans are revalidated immediately before execution. Recycle Bin recovery depends on the volume and Windows settings. Permanent deletion cannot be undone. Public builds keep destructive cleanup and WinGet uninstall disabled until independent security approval.

## Privacy and limitations

Casklet works locally without an account, cloud sync, or default telemetry. It stores paths, application metadata, and operation history on this machine. Diagnostics and exports can expose private paths; review them before sharing. Casklet does not sandbox applications, trace arbitrary installers, or guarantee complete cleanup of an application's external changes. Signed installers can still be harmful. Inaccessible or ambiguous items remain for manual review.

## Troubleshooting

- If a Cask or resource is missing, check the recovery area and manifest scan before changing files manually.
- If an operation is uncertain, inspect all listed paths; preservation is the safe choice when identity cannot be confirmed.
- If WinGet cannot find or verify a package, refresh provider inventory and verify the exact package ID and version.
- If startup rejects the database, retain `%LOCALAPPDATA%\\CaskletData\\db` and `backups`; a fresh database must not be created over a corrupt one automatically.
- Use **Save diagnostics** when reporting a failure, then inspect the bundle for private paths before sharing.
