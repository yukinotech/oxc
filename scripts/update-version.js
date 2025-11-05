#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");

const rootDir = path.resolve(__dirname, "..");

const envVersion = process.env.OXC_VERSION;

if (!envVersion) {
  console.error("Environment variable OXC_VERSION is not set.");
  process.exit(1);
}

const version = envVersion.trim();

const packageJsonPaths = [
  path.join(rootDir, "apps/oxlint/package.json"),
  path.join(rootDir, "npm/oxlint/package.json"),
];

for (const packageJsonPath of packageJsonPaths) {
  const source = fs.readFileSync(packageJsonPath, "utf8");
  const json = JSON.parse(source);
  json.version = version;
  fs.writeFileSync(packageJsonPath, `${JSON.stringify(json, null, 2)}\n`);
  console.log(
    `Updated ${path.relative(rootDir, packageJsonPath)} to ${version}`,
  );
}
