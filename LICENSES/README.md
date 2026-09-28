# License notices

Casklet's project license is Apache-2.0; see the top-level `LICENSE` and `NOTICE`. Dependency notices are generated with the SPDX SBOM for each release candidate and must be reviewed before public distribution.

# Third-party license review

The Rust dependency policy in `deny.toml` allows the usual permissive licenses and narrowly lists the current Tauri-related exceptions: `cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext`, and `selectors` under MPL-2.0, plus `target-lexicon` under Apache-2.0 WITH LLVM-exception.

Before distributing a public binary, review the exact versions in `Cargo.lock`, include required license notices, and make the applicable MPL-covered source available to recipients as required by MPL-2.0. New crates with these licenses require a separate review and exception.
