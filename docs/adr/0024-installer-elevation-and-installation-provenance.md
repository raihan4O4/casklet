# ADR 0024 — Installer elevation and Cask-installed provenance

Status: Accepted for the October 6 installation-flow request.

## Decision

Install Application exposes one optional Request administrator privileges checkbox. GUI and CLI pass the same typed `allowElevation` choice to the core. EXE elevation runs the exact validated installer with Windows RunAs. Requested MSI elevation runs the system msiexec executable with fixed install arguments through RunAs. Paths are environment values, never executable script text. Casklet and its hidden helper remain unelevated; only the installer requests UAC. Permission refusal is distinct from launch failure.

A successfully completed, process-tree-tracked installation with exactly one current matching new Windows registration is automatically registered as installed for its Cask. This is durable installation provenance, separate from provider uninstall authority. No Associate Existing App step is required. That action continues to create references to outside installations.

Schema 15 adds an installed-for-Cask flag to the operation journal. Automatic confirmation records it in the same transaction as the confirmed installation ID. Manual legacy candidate confirmation does not set it. The association query exposes INSTALLED_BY_CASKLET provenance for these records, and the Applications library labels them Installed for Cask. Names alone never establish stable identity or automatic uninstall authority.

Verified new MSI products retain the stronger schema 14 ownership and approved uninstall-plan path. EXE applications installed for a Cask lack a safe arbitrary-uninstall provider, so automatic uninstall remains unavailable. Keep app explicitly removes their Cask link while preserving software; Windows handles manual uninstall. Dropping this link clears installed-for-Cask attribution so later external association cannot inherit ownership. Supported MSI owners still cannot detach around their cleanup checks.

Typed installer-artifact resource IDs are excluded from Files & Folders and library file counts. Installer copies remain managed storage resources for inspection, recovery and separately approved file cleanup. Ordinary EXE files added as files remain files; classification does not depend on extensions or categories.

Ambiguous or missing matching registrations remain unresolved and visible. Failed, reboot-required, parent-only exits and interrupted installations never gain install attribution. Casklet cannot promise to identify every arbitrary installer or claim unrelated concurrently installed applications.

## Migration and recovery

Existing references are not promoted. Database startup performs the normal backed-up schema upgrade. Operation attribution and stable installation identity remain in SQLite. Manifests preserve informational installationProvenance but retain detach-only recovery semantics; importing a manifest cannot grant uninstall authority. No resource data is moved or removed by this migration.

## Validation

Tests cover explicit elevation forwarding, automatic MSI ownership, EXE install attribution without MSI uninstall authority, release of attribution on detach, ambiguous outcomes, and legacy/manual reference preservation. GUI tests cover checkbox payload, immediate Applications refresh, hidden installer copies, and Cask-installed EXE presentation. The complete repository quality gate and packaging evidence are recorded in PROJECT_CONTEXT after validation.

Real third-party installer/UAC behavior still requires isolated Windows manual testing. No real installed application was changed during this implementation.

`cargo xtask check` passed with 102 all-feature Rust tests and 29 GUI tests, default/Alpha profile checks, strict linting, contracts, formatting and production build. One opt-in performance benchmark remains intentionally ignored. Evidence: `target/installer-elevation-quality-verified.log`.
