# Milestone 6 — Portable ZIP applications

Portable installation accepts a reviewed ZIP hash and one executable path chosen from a preflight list. The provider rejects unsafe paths, links, unsupported entries and compression, case-colliding names, excessive path depth, entry counts, file sizes, archive size, total expanded size, and expansion ratio. Extraction goes to operation-owned staging under the Cask root; a successful extraction is moved into the existing managed `content/other/<resource-id>` layout. An interrupted install keeps its journal and staged data for explicit recovery inspection.

Application, installed instance, installation location, and Cask association each have distinct IDs. The source archive hash, selected executable hash, provider, version text, and installed root identity are retained as provenance. ZIP v1 supports inspection, installation, verified launch, and managed removal planning. It does not claim ownership of another installation merely because its name or path matches.

Launch accepts an installation ID, retrieves the recorded instance, verifies the managed root and executable hash, then starts that exact executable through a typed process API without a shell. A changed or missing installation is refused. CLI and desktop commands use the same application service.

Portable installation directories are registered as managed directory Resources. Cleanup uses the existing immutable removal preview, revalidation, journal, and feature gate. Completing a removal retires the installation and association records. No portable-specific direct delete path exists.

The archive is treated as untrusted input. The source hash prevents a reviewed ZIP from silently changing before extraction; it does not attest to software publisher trust. Users remain responsible for deciding whether to run an executable from a ZIP.
