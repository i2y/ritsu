// What OpenSpec's own readers make of the files beside this script: the requirement blocks of each
// spec as archive reads them, the scenarios `openspec show --json` gives, and what each delta spec
// does. tests/openspec.rs holds ritsu-base's reader to expected.json, which this writes:
//
//   $ npm install --prefix <dir> @fission-ai/openspec@1.14.0
//   $ OPENSPEC=<dir>/node_modules/@fission-ai/openspec node expected.mjs > expected.json
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = process.env.OPENSPEC;
const load = (m) => import(pathToFileURL(join(pkg, 'dist/core/parsers', m)).href);
const { extractRequirementsSection, parseDeltaSpec } = await load('requirement-blocks.js');
const { MarkdownParser } = await load('markdown-parser.js');

const specs = {};
for (const cap of ['greeting', 'edges']) {
  const text = readFileSync(join(here, 'openspec/specs', cap, 'spec.md'), 'utf-8');
  const blocks = extractRequirementsSection(text).bodyBlocks.map((b) => ({ name: b.name, block: b.raw }));
  const shown = new MarkdownParser(text).parseSpec(cap).requirements.map((r) => ({ name: r.name, scenarios: r.scenarios.map((s) => s.name) }));
  specs[cap] = { blocks, shown };
}
const deltas = {};
for (const id of ['trim-names', 'rework']) {
  const text = readFileSync(join(here, 'openspec/changes', id, 'specs/greeting/spec.md'), 'utf-8');
  const d = parseDeltaSpec(text);
  deltas[id] = {
    renamed: d.renamed,
    removed: d.removed,
    modified: d.modified.map((b) => ({ name: b.name, block: b.raw })),
    added: d.added.map((b) => ({ name: b.name, block: b.raw })),
  };
}
console.log(JSON.stringify({ openspec: '1.14.0', specs, deltas }, null, 2));
