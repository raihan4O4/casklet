# ADR 0033: Version the Explorer COM identity with the adapter

Status: Accepted for the installed submenu upgrade fix, 2026-10-20.

## Evidence

The registered adapter was the new nested-menu DLL, but the existing Explorer process still loaded the older flat-menu DLL. A fresh COM test activated the new registration and passed, while the user's real menu remained flat. Computer-use reproduction confirmed the missing arrow. Updating an InprocServer32 path does not replace a class factory already retained by a process; a distinct DLL filename alone was insufficient.

## Decision

Derive the native COM identity from the same source identifier used in the versioned DLL filename. Preserve Casklet's GUID namespace prefix, set the UUID variant bits, and use the adapter's 64-bit source identifier for the remaining bytes. The identity is versioning metadata, not a security hash or resource authority. Generate the C++ GUID header during the build and derive the matching registration GUID from the validated adapter filename in Rust. Changes to desktop code that leave the native adapter unchanged retain its identity.

Register the new class first, update the existing named Casklet file/directory handlers, and notify shell association changes through the existing installer hook. Retire previous Casklet class registrations only within this namespace and only when their server paths belong to the same installation's adapter directory. Never delete a DLL loaded in Explorer or stop Explorer as part of this upgrade. Uninstall removes the active namespace-validated class and the legacy fixed class before removing handler/settings keys. Existing reboot-scheduled deletion remains available for locked binary files.

Tests cover distinct identities across adapter revisions, exact byte formatting, invalid filenames, registration replacement and retired-class cleanup. Native COM checks prove build and Rust registration agree. Installed upgrade validation must check the actual Explorer process and visible native menu; fresh-process COM verification alone does not close this regression.

## Live-process limitation

Further visual verification showed that the existing Explorer process also retained its handler list after versioned registration and folder refresh. The new identity avoids class-factory aliasing but cannot promise live cache eviction. Installation now delivers association notification synchronously and checks for mapped adapter files by attempting an OPEN_EXISTING write handle without writing any data. If an adapter is locked, normal installation explicitly tells the user to finish file transfers and restart Explorer (or sign out); silent installation records the same message in its log. The installer never terminates Explorer automatically. Agent-driven restart requires user approval because active transfers and open Explorer windows may be interrupted.
