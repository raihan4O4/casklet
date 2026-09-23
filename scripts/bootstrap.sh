#!/usr/bin/env sh
set -eu

for tool in git rustup cargo node corepack; do
  command -v "$tool" >/dev/null 2>&1 || { echo "Missing prerequisite: $tool" >&2; exit 1; }
done

corepack pnpm --version
cargo xtask bootstrap-check
