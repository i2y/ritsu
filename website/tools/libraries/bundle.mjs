// node bundle.mjs <out>: entry.js and what it imports, bundled by esbuild into one minified ES module,
// with the packages it holds and the texts of their licenses above the code. make_libraries.sh runs
// it once `npm ci` has installed the lock.
//
// The list names every package a file of which esbuild took into the bundle (its metafile), at the
// version installed, which is the one package-lock.json pins; the texts are the packages' own license
// files, each text once, after the names of the packages that carry it.
import { build } from "esbuild";
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const out = process.argv[2];
if (!out) throw new Error("usage: node bundle.mjs <out>");

const result = await build({
  entryPoints: [join(here, "entry.js")],
  absWorkingDir: here,
  bundle: true,
  format: "esm",
  target: "es2020",
  minify: true,
  legalComments: "none",
  charset: "utf8",
  metafile: true,
  write: false,
});

// The packages the bundle holds code of: node_modules/<name> or node_modules/@<scope>/<name>.
const names = new Set();
for (const input of Object.keys(result.metafile.inputs)) {
  const m = input.match(/(?:^|\/)node_modules\/((?:@[^/]+\/)?[^/]+)\//);
  if (m) names.add(m[1]);
}
const packages = [...names].sort().map((name) => {
  const dir = join(here, "node_modules", name);
  const manifest = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
  const file = readdirSync(dir).find((f) => /^licen[cs]e(\.(md|txt))?$/i.test(f));
  if (!file) throw new Error(`${name} has no license file`);
  const text = readFileSync(join(dir, file), "utf8").replace(/\r\n/g, "\n").trimEnd() + "\n";
  if (text.includes("*/")) throw new Error(`the license of ${name} closes a comment`);
  const repo = String((manifest.repository && manifest.repository.url) || manifest.repository || "")
    .replace(/^git\+/, "")
    .replace(/\.git$/, "");
  return { name, version: manifest.version, license: manifest.license, repo, file, text };
});

const width = Math.max(...packages.map((p) => `${p.name} ${p.version}`.length));
const lines = [
  "/*!",
  "libraries.js: the editor and the Markdown of ritsu's page in the browser.",
  "",
  "website/tools/make_libraries.sh bundles website/tools/libraries/entry.js and what it imports, at",
  "the versions website/tools/libraries/package-lock.json pins, into this one file. It holds the code",
  "of these packages, each under its own license, whose text follows as the package gives it:",
  "",
  ...packages.map((p) => `  ${`${p.name} ${p.version}`.padEnd(width)}  ${p.license}  ${p.repo}`),
];
// each text once, after the packages that carry it
const texts = new Map();
for (const p of packages) {
  if (!texts.has(p.text)) texts.set(p.text, []);
  texts.get(p.text).push(p);
}
for (const [text, carriers] of texts) {
  lines.push("", `---- ${carriers.map((p) => `${p.name} (${p.file})`).join(", ")}`, "", text.trimEnd());
}
lines.push("*/", "");
const code = new TextDecoder().decode(result.outputFiles[0].contents);
writeFileSync(out, lines.join("\n") + code);
console.log(`${out}: ${packages.length} packages, ${code.length} characters of code`);
