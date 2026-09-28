# ADR 0016 — Apache-2.0 project license

Status: Accepted on 2026-09-28

## Context

Casklet is intended for public source and binary distribution, but its own license was previously undecided. The repository already reviews third-party dependencies separately under `deny.toml` and `LICENSES/README.md`.

## Decision

License Casklet's original code and documentation under Apache License, Version 2.0. Keep the full text in `LICENSE`, attribution in `NOTICE`, package metadata set to `Apache-2.0`, and both files in release candidate artifacts. The release process still reviews the license and notice obligations of each bundled third-party dependency.

## Rationale

Apache-2.0 permits broad use and redistribution while providing an express patent grant and contribution terms. This is useful for a desktop tool that may accept outside contributions. Its notice requirements are more involved than MIT's; the release checklist includes a distinct third-party notice review so packaging cannot imply that an automatically generated SBOM completes that work.

## Consequences

Contributors retain their own copyrights while licensing contributions under the project license. Redistributors must follow Apache-2.0 conditions, including retaining required notices. MPL-covered dependencies continue to have their own obligations. A license choice does not close the independent security or Windows manual-test gates.
