#!/usr/bin/env bash

export OXC_VERSION="1.25.0-alpha-2"

node scripts/update-version.js

# 7. Build macOS aarch64 language server
cargo build --release -p oxc_language_server --target aarch64-apple-darwin

# 8. Build macOS aarch64 oxlint N-API module
cargo build --release -p oxlint --features napi --target aarch64-apple-darwin

# 9. Install cross for Linux cross-compilation
# cargo install cross --git https://github.com/cross-rs/cross

# 10. Build Linux x86_64 language server with cross
cross build --release -p oxc_language_server --target x86_64-unknown-linux-gnu

# 11. Build Linux x86_64 oxlint N-API module with cross
cross build --release -p oxlint --features napi --target x86_64-unknown-linux-gnu

pnpm --filter oxlint run build


node scripts/copy-artifacts.js
node npm/oxlint/scripts/generate-packages.js
node scripts/clean-artifacts.js
node scripts/scope-oxlint-packages.js