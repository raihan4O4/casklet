# ADR 0023 — One-action installer orchestration

Status: Accepted for the October 5 installer-flow request, within existing ownership invariants.

## Decision

Select MSI/EXE performs read-only inspection in the background. Install is the explicit authorization to copy, validate and run those exact bytes. A shared typed application command orchestrates the existing resource, installer journal, inventory and ownership services; GUI and CLI call it. Windows handles the installer's requested elevation. There is no separate registration, launch-review checkbox or ownership checkbox in the normal flow.

New verified MSI products automatically gain Cask ownership using the existing absent-before baseline, exact ProductCode, successful outcome and current registration checks. Existing products, legacy launches and uncertain results do not gain uninstall authority. A single current matching new registration can automatically become an association when a successful tracked launch completes. Name/publisher correlation is sufficient only for a non-destructive association, never uninstall ownership.

EXE launches use a fixed system PowerShell `Start-Process -Wait` helper, with paths passed as environment values, to wait for the bootstrapper and its descendants. Native installer UI/UAC remain visible; Casklet stays unelevated. Timeouts, cancellation, errors and reboot requirements retain durable uncertainty and locks. Do not equate a parent-only exit with completed installation. Descendants that start services, escape tracking or remain as running applications can still need manual attention; no arbitrary-installer completion guarantee is made.

Successful verified ownership/association needs no manual confirmation or preserved closure. A completed launch with ambiguous/missing matching registrations closes as preserved without attaching an arbitrary application. Failed or interrupted work remains available behind Technical details, with one plain-language status in the primary view. Diagnostics, candidate evidence, operation IDs and history stay collapsed; confirmed candidates are not repeated as a second application list.

## Remaining provider limit

Arbitrary EXE uninstall is still unsupported. The Install action requests management, but cannot manufacture safe deletion authority from an executable exit code, display name or raw registry command. The UI states this limitation before installation and after any reference-only association. A future EXE provider must independently establish stable installation identity, provenance and typed uninstall capabilities before granting cleanup authority. This request's universal EXE ownership expectation is therefore not fully implemented.

## Migration and recovery

No schema change or ownership migration is needed. Existing low-level CLI inspection/registration/launch/recovery commands remain available. Saved references stay references. Existing MSI plans, journals, hashes and cleanup gates remain unchanged. Installer copies remain managed resources and can later be cleaned up through their own approved plans.

Native wait semantics: [Microsoft Start-Process documentation](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.management/start-process?view=powershell-5.1).

## Validation

`cargo xtask check` passed with 101 all-feature Rust tests and 25 GUI tests, additional default/Alpha profile checks, strict Clippy, formatting, frontend lint/type checks, contract consistency and production builds. The native fake executable test verifies waiting for a delayed child, paths containing metacharacters, timeout and cancellation. Core tests cover automatic MSI ownership, EXE reference-only association, ambiguous outcome closure, unchanged-byte/consent checks and retained locks for parent-only exits, failure and reboot. The GUI test covers selection without execution and a single Install command followed by immediate application refresh. Source logs: `target/installer-flow-quality-final.log`.

No real third-party installation, uninstall or UAC validation is claimed. Isolated Windows installer testing and independent security review remain open.

Presentation follow-up: [Applications library refinement](../engineering/applications-library-ux.md) supersedes the installer disclosure/history presentation described above. Ownership, launch, journal and cleanup decisions remain unchanged.
