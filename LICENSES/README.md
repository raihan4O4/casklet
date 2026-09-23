# License notices

Casklet's project license is intentionally undecided. Dependency notices and the chosen project license will be added before public distribution.

# Third-party license review

The Rust dependency policy in `deny.toml` allows the usual permissive licenses and narrowly lists the current Tauri-related exceptions: `cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext`, and `selectors` under MPL-2.0, plus `target-lexicon` under Apache-2.0 WITH LLVM-exception.

Before distributing a public binary, review the exact versions in `Cargo.lock`, include required license notices, and make the applicable MPL-covered source available to recipients as required by MPL-2.0. New crates with these licenses require a separate review and exception. Casklet's own public license is still undecided; unpublished workspace crates are excluded from the third-party license check.
