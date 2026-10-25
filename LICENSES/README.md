# License notices

Casklet's project license is Apache-2.0; see the top-level `LICENSE` and `NOTICE`. Dependency notices are generated with the SPDX SBOM for each release candidate and must be reviewed before public distribution.

The desktop bundles Inter (Google Fonts Inter variable font) under the SIL Open Font License 1.1; its notice is `Inter-OFL.txt`. Lucide Svelte 1.48.0 is used under ISC; its notice is `Lucide-ISC.txt`. Both notices are included in the desktop installer resources. The supplied Casklet logo is used directly as the project's official branding asset.

# Third-party license review

The Rust dependency policy in `deny.toml` allows the usual permissive licenses and narrowly lists the current Tauri-related exceptions: `cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext`, and `selectors` under MPL-2.0, plus `target-lexicon` under Apache-2.0 WITH LLVM-exception.

Before distributing a public binary, review the exact versions in `Cargo.lock`, include required license notices, and make the applicable MPL-covered source available to recipients as required by MPL-2.0. New crates with these licenses require a separate review and exception.

Bundled Unicode 18.0 emoji data uses Unicode License V3 (`Unicode-LICENSE.txt`), included in installer resources. Source: https://www.unicode.org/Public/18.0.0/emoji/emoji-test.txt.
