#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

const root = process.cwd();

function copyArtifact(srcRel, destName) {
  const src = path.join(root, srcRel);
  const dest = path.join(root, destName);
  fs.copyFileSync(src, dest);
}

copyArtifact('target/aarch64-apple-darwin/release/liboxlint.dylib', 'oxlint.darwin-arm64.node');
copyArtifact('target/aarch64-apple-darwin/release/oxc_language_server', 'oxc_language_server-darwin-arm64');

copyArtifact('target/x86_64-unknown-linux-gnu/release/liboxlint.so', 'oxlint.linux-x64-gnu.node');
copyArtifact('target/x86_64-unknown-linux-gnu/release/oxc_language_server', 'oxc_language_server-linux-x64-gnu');
