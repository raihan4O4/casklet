# Explorer integration verification

Implementation: ADR 0031. Native Explorer styling and the installed executable's bundled icon supply the menu presentation. Registration is per user; uninstall removes only Casklet's Explorer keys and leaves operational metadata and resource content intact.

## User journey

Select up to 32 files/folders, right-click, choose **Show more options → Casklet**, and choose a Cask. A batch uses one destination and individual consent reviews. Review the populated Files & Folders intake and select Reference or explicitly consent to ownership. The first five active Casks use the shared core's stable creation order. **More casks…** searches every active Cask. The limit does not apply to the chooser.

An EXE/MSI selection offers **Install Application**, **Add Installer to Cask**, and **Cancel**. Installation opens inspection/review and still requires Install. File intake retains explicit ownership/reference selection and never runs the file. Names/extensions alone do not establish that an EXE is a trustworthy installer. Existing installer inspection, uncertain outcomes, UAC, correlation and uninstall ownership rules remain authoritative.

With no active Casks, Explorer opens the same creation form used by the main window. Creation immediately routes the still-pending selection to the new Cask for existing ownership/reference or installer review. Cancelling creation dismisses the batch without adding resources; a creation error keeps the name and source, and a routing error retries the created destination without making another Cask. Success feedback appears only after the existing add command succeeds.

The chooser displays names only, with searchable radio selection and a bounded scrolling list. It loads current active Casks on every request and refreshes automatically on focus and every three seconds while choosing. There is no manual Refresh button. A failed list request retries automatically and does not assume the library is empty.

An unavailable destination opens the chooser. Missing/inaccessible sources show an actionable error and remain cancellable. Busy workflows keep subsequent intents queued with an explicit cancel action. No success notification appears merely because Explorer delivered a request.

## Automated coverage

- Strict launch contract, Unicode/metacharacter preservation, invalid identity/path/argument rejection.
- Bounded FIFO intents; per-window visibility/acknowledgement; retry-safe acknowledgement; requeue on workspace destruction.
- Actual Windows registry projection under a unique test subtree: zero/one/five/six/eight Casks, names and escaping, file/directory COM registration, atomic snapshot replacement and shrinkage.
- GUI: searching/selecting a destination beyond five; missing destination; source routing errors; native-dialog cancellation; ordinary/installer file review preserving consent; installation inspection without launch.
- Native C++ COM ABI, HDROP selection-array intake, selection limits, menu command IDs, default-verb behavior, Windows argument round trips, invalid verb rejection, and DLL lifetime/unload behavior.
- Existing ownership, folder synchronization, cleanup and installer lifecycle suites remain required.

## Packaged checks

Verification on October 19–20, 2026:

- `cargo xtask check` passed the complete Rust/all-feature/default/Alpha suites, strict lint/format, generated contracts, frontend types, 59 GUI tests and builds. Final native boundary/registration refinements also passed scoped strict Clippy and native contract checks.
- `cargo deny check` passed advisories, bans, licenses and sources.
- Separately named QA NSIS packages passed silent installation, CLI-driven rename/archive menu refresh, installed startup and single-instance reuse. An installed Unicode source was reviewed and added as a reference through the existing GUI, with the shared-core record verified as REFERENCED/USER_REFERENCE.
- Actual installed COM activation, submenu construction (five Casks plus More), icon bitmap, and mixed-location selection passed the opt-in native API test. This is a headless ABI/integration check, not a substitute for final Explorer UI acceptance.
- Registration also passed with an older DLL still present; it binds the compiled-in version instead of guessing from directory contents. QA uninstall removed registration and both current/older fixture DLLs, preserved the fixture database/source, and left the pre-existing Casklet executable hash unchanged. No native binary removal remained pending in this unloaded-DLL case.

Evidence: `target/explorer-final-quality.log`, `target/explorer-dependency-policy.log`, `target/explorer-native-final-contracts.log`, `target/explorer-release-native-verification.log`, `target/explorer-package-session.json` and the `target/explorer-*-package*.log`/upgrade logs. Production packaging uses `target/explorer-production-package.log`.

The host has an existing Casklet installation. Packaging tests must preserve it and its database. A separately named QA package built from the same source may be installed into a fixture directory with a separate profile and instance identifier to exercise hooks and installed UI. This verifies packaging mechanics, not a clean-machine production upgrade.

Desktop UI automation was stopped by the user's physical Escape key. No further desktop interaction was performed. Final visual Explorer submenu/chooser/installer-choice acceptance remains open, alongside clean Windows 11 install/upgrade/uninstall, uninstall while Explorer holds the DLL, real third-party MSI/EXE/UAC outcomes, and assistive-technology/high-DPI review. The primary Windows 11 menu is not implemented by this NSIS strategy. Existing independent security/publication gates remain open.

To reproduce unattended packaging checks on a host without an existing Casklet Explorer registration, build a QA configuration with productName `Casklet Explorer QA`, identifier `app.casklet.explorerqa`, and an empty beforeBuildCommand; build the frontend and NSIS bundle with that overlay. Build the release CLI, then run `scripts/explorer-package-smoke.ps1 -SkipLaunch`, `cargo test -p casklet-desktop registered_native_shell -- --ignored --nocapture`, and `scripts/explorer-package-smoke.ps1 -Complete`. The session file is written before assertions so Complete can also clean up a failed fixture. Omitting SkipLaunch enables the installed GUI/single-instance checks. The script refuses to replace an existing Explorer integration.

October 20 intake refinement coverage adds direct creation from a startup-style queued request, original Unicode folder handoff through successful reference consent, batch preservation, creation failure/cancellation, blocked Escape during submission, routing retry without duplicate creation, names-only entries and automatic focus refresh. The shared CreateCaskForm uses the existing create_cask command in both main-window and Explorer flows.

## Direct nested actions (October 20)

Hover over a Cask to select **Add as Owned** or **Add as Reference**. These commit through the existing core without revealing or focusing Casklet. Owned means the data is dedicated to that Cask and may be removed only through later reviewed cleanup; the source stays at its current location. References never gain delete authority. A single real EXE/MSI file also offers **Install Application**, which opens the existing installer inspection and still requires Install. Multiple selections and directories offer only file-management actions. Installer files added as content never execute.

The existing process accepts background requests without activating windows. Cold direct-only startup stays hidden and exits after the requests finish. Native completion notifications are subject to Windows notification settings; errors use actionable native messages. Partial batches identify item errors and instruct users to inspect Cask/Activity state before retrying only items still missing. Existing journals and storage refresh remain in use. More and no-Cask creation retain the names-only automatic-loading chooser and original-selection handoff.

Deterministic native fixtures use a process-local registry override under a unique test key, restore HKCU before exit, and verify five nested Casks plus More, action labels, ordinary versus installer command counts, IDs, insufficient command ranges and existing quoting/ABI behavior. Shared-core tests verify owned versus reference classification, source byte preservation, non-execution of EXE/MSI content and honest partial failure. GUI tests verify direct installer inspection never executes or adds the artifact automatically. The installed background smoke script is scripts/explorer-direct-smoke.ps1; its separately named QA installer omits Explorer registration hooks, uses a separate instance identifier, and validates cold hidden startup, forwarding while running, automatic exit and dedicated source preservation without replacing installed Explorer keys.

Direct-action verification passed the complete repository gate (including 67 GUI tests, Rust core/default/Alpha profiles, strict Clippy/format/types and builds), dependency policy, and seven desktop/native tests with the separate registered-COM test intentionally opt-in. The installed QA test also loaded the packaged DLL, initialized an actual HDROP selection, queried the nested installer menu and invoked Add as Reference through IContextMenu::InvokeCommand; the source arrived in SQLite as a reference without visible Casklet UI or execution. A concurrent cold-start pair verified running-instance forwarding and automatic exit. Native window checks exclude only the official single-instance plugin and Tao transparent event targets, not application windows or error dialogs. The QA product was uninstalled and the existing production Explorer projection stayed unchanged. Evidence: target/explorer-direct-quality.log, target/explorer-direct-policy.log, target/explorer-direct-tests.log, target/explorer-direct-gui.log and target/explorer-direct-installed-smoke.log. New Alpha packaging uses target/explorer-direct-alpha-package.log. Existing clean-machine, visual pointer-navigation, assistive-technology and public-release gates remain open.

## Installed-menu diagnosis and update

The October 20 missing-submenu report was checked against the actual installation, not only packaged QA. The running per-user executable still dated October 7 and the new COM handler was not registered. The idle old application was closed through its normal close request after a consistent backup at target/submenu-upgrade-2e772d2c-f220-4c61-9c9b-18b7822ea671/before.db; the verified Alpha installer then updated the existing installation successfully. Its registered versioned DLL hash matches the tested adapter. Cask, resource, ownership-grant, application and installation records remain equal to the backup, and SQLite quick_check passes.

The native harness now supports --inspect-registered and --inspect-registered-installer. These load the actual registered COM server, check each Cask's hSubMenu, verify exact leaf labels and app icon, and perform no resource or installer action. They accept the real active Cask count instead of assuming the seven-Cask QA fixture. Both passed on the updated installation: one Cask with Owned/Reference, and the same Cask with Install Application/Owned/Reference for an installer selection. These checks verify the native submenu structure; visual pointer-navigation and clean-machine checks remain separate gates.

## Explorer's retained class factory

The subsequent user report disproved the earlier conclusion that applying the installer and checking fresh COM activation was sufficient. Explorer still had casklet-explorer-0.1.0-alpha.1-a9325de72816525d.dll loaded while InprocServer32 pointed to the nested-menu 6459460f34b0ee81 adapter. The actual Explorer menu was reproduced visually and showed a flat Cask leaf. ADR 0033 now ties the CLSID to the source identifier in the DLL name; native code and Rust registration derive matching identities. Installer association notification announces the new class; live Explorer can still retain its old handler list until restart. Active and legacy registration cleanup remains scoped to Casklet.

The same Explorer process retained the flat menu even after new-class registration and F5. Restart permission was requested before interrupting Explorer. The installer now checks for mapped adapters without changing their bytes and displays an explicit restart instruction only when a loaded adapter is detected, rather than implying that reinstall alone refreshes every existing Explorer process.

ADR 0035 refinement: all duplicate Explorer add modes preserve existing ownership/category; ownership changes require the dedicated item action. Completion text confirms processed selections without claiming that an ignored duplicate changed policy. New content uses the shared automatic category classifier, including EXE/MSI content independently of application installation.
