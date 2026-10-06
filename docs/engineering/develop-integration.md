# Develop integration and installed launch validation

Date: 2026-10-06. Windows 11 x64; local Alpha validation.

## Branch investigation

Local develop started at 5869cc5. After fetching, origin/develop was f277847: PR #16 already merged codex/alpha-readiness (840f30d), and PR #17 already merged the subsequent installer-cleanup commit (126663b). The active checkout was codex/alpha-installer-cleanup with the new application lifecycle changes uncommitted. Those changes were preserved as a038386 before integration. No branch content needs to be replaced or discarded.

## Launch investigation

The user confirmed that the previously failing installation had been uninstalled. The default executable, uninstall registration and shortcuts were absent; only a saved install-location setting and persistent data remained. The original failing binary and its runtime evidence are unavailable. Its historical failure is not represented as having a confirmed root cause.

A concrete defect exists in alpha-readiness: its desktop main entry point has no Windows GUI subsystem attribute. That allows console allocation; closing an attached console can terminate the application. The subsequent 126663b entry-point fix sets the GUI subsystem for every Windows profile. Integration retains that fix. The CLI appropriately keeps its console subsystem.

The retained local NSIS candidate launched from an actual fixture installation and after reinstall. Both launches showed Casklet, the PE subsystem was GUI (2), and no shell descendants were observed. Install/reinstall/uninstall exited zero and fixture data/content survived. Evidence: target/installed-desktop-baseline.log. These results establish that the retained package can launch on this host, not why the removed historical installation failed.

## Repeatable verification

Run scripts/installed-desktop-smoke.ps1 after packaging. It refuses an existing install/process, installs into a unique workspace target directory, isolates data/profile, seeds a Cask, checks the installed PE subsystem, waits for the Casklet window, checks shell descendants, repeats after reinstall, uninstalls only the fixture, checks data preservation, and restores the saved install-location setting. It retains fixture evidence; it does not delete user data. Native UI inspection supplements its window-handle checks because a process alone does not prove rendered content.

The workflow now runs on develop pushes as well as main pushes and pull requests. Hosted results require a push; local checks are recorded separately. Packaging records supported registered-EXE uninstall capability alongside MSI capability.

The final candidate must pass the shared local CI gate, dependency audits, clean-source packaging/checksums, real Windows fixture cleanup, and the installed-desktop test. Real third-party MSI/EXE/UAC, clean-machine and signing gates remain open; fixture tests do not close them.

## Merged source validation

Merged develop passed cargo xtask check: 107 all-feature Rust tests, separate default/Alpha profiles, 29 GUI tests, strict Clippy, formatting, frontend lint/type checks, generated contracts and builds. Bootstrap, frozen-lockfile install, baseline secret scan and branch ancestry checks passed. Rust dependency policy passed with existing duplicate-version/allowance warnings. The frontend audit initially failed for source-map-js 1.2.1; pinning the [maintainer's patched 1.2.2 release](https://github.com/7rulnik/source-map-js/releases/tag/v1.2.2) passed the audit. Evidence: target/develop-quality.log, target/develop-rust-policy.log, target/develop-frontend-audit.log, target/develop-bootstrap.log.

Final packaging and installed/cleanup results are retained in target/develop-package.log, target/develop-installed-smoke.log and target/develop-alpha-smoke.log. These executable checks must complete successfully for the candidate; their logs are separate from hosted CI results. The package must identify clean develop source and pass scripts/verify-release.ps1.
