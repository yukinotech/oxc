#!/usr/bin/env node

const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');

const targets = [
  { dir: 'npm/oxlint', scopedName: '@ytk-oxlint/oxlint' },
  { dir: 'npm/oxlint-darwin-arm64', scopedName: '@ytk-oxlint/darwin-arm64' },
  { dir: 'npm/oxlint-linux-x64-gnu', scopedName: '@ytk-oxlint/linux-x64-gnu' },
];

for (const { dir, scopedName } of targets) {
  const filePath = path.join(root, dir, 'package.json');
  const raw = fs.readFileSync(filePath, 'utf8');
  const data = JSON.parse(raw);

  if (data.name === scopedName) {
    continue;
  }

  data.name = scopedName;
  if (dir === 'npm/oxlint' && data.optionalDependencies) {
    const renamedOptionalDeps = {};
    for (const [depName, depVersion] of Object.entries(data.optionalDependencies)) {
      if (depName.startsWith('@oxlint/')) {
        const suffix = depName.slice('@oxlint/'.length);
        renamedOptionalDeps[`@ytk-oxlint/${suffix}`] = depVersion;
      } else {
        renamedOptionalDeps[depName] = depVersion;
      }
    }
    data.optionalDependencies = renamedOptionalDeps;
  }

  const updated = `${JSON.stringify(data, null, 2)}\n`;
  fs.writeFileSync(filePath, updated);
}
