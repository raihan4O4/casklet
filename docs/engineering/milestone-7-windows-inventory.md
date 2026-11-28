# Milestone 7 — Windows installed-application inventory

The Windows adapter reads uninstall registrations from current-user and local-machine hives in both registry views. It does not execute uninstall strings, query `Win32_Product`, or infer ownership. A malformed individual registration without a usable display name is skipped. A failed hive or subkey enumeration fails the whole scan so refresh cannot falsely mark previously seen records missing. Searches scan live metadata without writing SQLite.

Each installation ID is derived from the registry hive, view, and subkey, not its display name. Logical application records also stay separate for each registration until a provider supplies stronger matching evidence; a shared name and publisher never merge them. The database retains raw read-only registration evidence and a hash of that evidence. A refresh records changed or missing observations without deleting historical records. A later complete observation can mark an instance present again.

Association rechecks the selected installation ID against a live scan and records an explicit `ASSOCIATED` and `DETACH_ONLY` relationship with the Cask. Removing that association changes only the Cask relationship. These records never gain install provenance, managed Resource ownership, or automatic uninstall eligibility. Portable ZIP installations remain in their own owned lifecycle tables.

Per-Cask manifests project detected associations. A database restore accepts the metadata only as an unverified `AMBIGUOUS` installation and a detach-only association. A later complete registry refresh can verify presence, change, or absence; the manifest alone grants no uninstall authority.

The CLI offers `installed search`, `refresh`, `associate`, `disassociate`, and `list`; the desktop UI offers the same discovery and association workflow through the shared application service. Duplicate names are labelled for review, with the registry source shown to distinguish instances. The registry adapter is Windows-only. Ordinary tests use recorded synthetic registrations; a read-only scan on Windows validates the adapter against the host registry.
