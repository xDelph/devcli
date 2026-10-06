#!/usr/bin/env node
// Keeps the website version in sync with the workspace version in Cargo.toml.
//
// The workspace Cargo.toml is the single source of truth (release-please bumps
// it). This script is run automatically before `astro dev`/`astro build`
// (see website/package.json) so the site never hardcodes a stale version.
//
// Idempotent: running it twice makes no change.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

const cargo = readFileSync(join(root, 'Cargo.toml'), 'utf8');
const match = cargo.match(/\[workspace\.package\][\s\S]*?\nversion\s*=\s*"([^"]+)"/);
if (!match) {
  console.error('sync-version: could not find [workspace.package] version in Cargo.toml');
  process.exit(1);
}
const version = match[1];

const targets = [
  {
    file: 'website/package.json',
    replace: (body) => body.replace(/("version"\s*:\s*")[^"]+(")/, `$1${version}$2`),
  },
  {
    file: 'website/src/components/Footer.astro',
    replace: (body) => body.replace(/(\d+\.\d+\.\d+)_LATEST/, `${version}_LATEST`),
  },
  {
    file: 'website/src/layouts/DocsLayout.astro',
    replace: (body) =>
      body.replace(
        /(VERSION:<\/span>\s*<span class="text-\[var\(--text-main\)\]">)[^<]+(<\/span>)/,
        `$1${version}$2`,
      ),
  },
];

let changed = 0;
for (const { file, replace } of targets) {
  const path = join(root, file);
  if (!existsSync(path)) {
    console.error(`sync-version: missing ${file}`);
    process.exit(1);
  }
  const before = readFileSync(path, 'utf8');
  const after = replace(before);
  if (after !== before) {
    writeFileSync(path, after);
    changed += 1;
    console.log(`sync-version: ${file} -> ${version}`);
  }
}

console.log(`sync-version: workspace version ${version} (${changed} file(s) updated)`);
