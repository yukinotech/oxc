#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

const root = process.cwd();
const files = [
  "oxlint.darwin-arm64.node",
  "oxc_language_server-darwin-arm64",
  "oxlint.linux-x64-gnu.node",
  "oxc_language_server-linux-x64-gnu",
];

for (const filename of files) {
  const fullPath = path.join(root, filename);
  try {
    fs.rmSync(fullPath);
    console.log(`Removed ${filename}`);
  } catch (err) {
    if (err.code !== "ENOENT") throw err;
  }
}
