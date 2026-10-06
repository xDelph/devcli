#!/usr/bin/env node
// Generates the website documentation pages from the single source of truth
// in ./docs. Run automatically before `astro dev`/`astro build` (see
// website/package.json). Do not edit website/src/pages/docs/*.md by hand —
// they are generated and git-ignored.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const docsDir = join(root, 'docs');
const outDir = join(root, 'website', 'src', 'pages', 'docs');

// repo docs filename -> website route
const route = {
  'getting-started.md': '/docs/getting-started',
  'configuration-reference.md': '/docs/configuration',
  'commands-reference.md': '/docs/commands',
  'advanced-features.md': '/docs/advanced',
  'troubleshooting.md': '/docs/troubleshooting',
};

const pages = [
  {
    src: 'getting-started.md',
    out: 'getting-started.md',
    title: 'Getting Started',
    description: 'A comprehensive guide to get you up and running with DevCLI in minutes',
  },
  {
    src: 'configuration-reference.md',
    out: 'configuration.md',
    title: 'Configuration',
    description: 'Complete reference for the DevCLI configuration file',
  },
  {
    src: 'commands-reference.md',
    out: 'commands.md',
    title: 'Commands',
    description: 'All DevCLI CLI commands with examples',
  },
  {
    src: 'advanced-features.md',
    out: 'advanced.md',
    title: 'Advanced Features',
    description: 'Health checks, metrics, logging, dependencies, and more',
  },
  {
    src: 'troubleshooting.md',
    out: 'troubleshooting.md',
    title: 'Troubleshooting',
    description: 'Common issues and solutions for DevCLI',
  },
  {
    src: '../AGENTS.md',
    out: 'agents.md',
    title: 'AI Agents',
    description: 'How to drive DevCLI from autonomous agents: JSON, exit codes, non-interactive usage',
  },
];

mkdirSync(outDir, { recursive: true });

for (const page of pages) {
  let body = readFileSync(join(docsDir, page.src), 'utf8');

  // Rewrite relative doc links (e.g. ./commands-reference.md#anchor) to site routes.
  body = body.replace(/\]\(\.\/([a-z0-9-]+\.md)(#[^)]*)?\)/g, (match, file, anchor = '') =>
    route[file] ? `](${route[file]}${anchor})` : match,
  );

  // The agent contract lives at the repo root; it is published as a docs page.
  body = body.replace(/\]\(\.\.\/AGENTS\.md\)/g, '](/docs/agents)');

  const frontmatter =
    '---\n' +
    'layout: ../../layouts/DocsLayout.astro\n' +
    `title: ${JSON.stringify(page.title)}\n` +
    `description: ${JSON.stringify(page.description)}\n` +
    '---\n\n';

  writeFileSync(join(outDir, page.out), frontmatter + body);
  console.log(`generated website/src/pages/docs/${page.out} (from docs/${page.src})`);
}
