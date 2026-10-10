// What the tests of the package share. They run on the package packaging/npm/build.sh made,
// unpacked: RITSU_NPM_PACKAGE names its directory and RITSU_NPM its .tgz
// (crates/ritsu/tests/npm.rs runs them so). Without the package a test is skipped.

import { cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const repo = join(dirname(fileURLToPath(import.meta.url)), "../../..");
export const pkg = process.env.RITSU_NPM_PACKAGE ?? "";
export const tgz = process.env.RITSU_NPM ?? "";
export const skip = !pkg || !existsSync(join(pkg, "package.json")) ? "RITSU_NPM_PACKAGE names no package (crates/ritsu/tests/npm.rs runs these tests with one)" : false;

/** The package's API, imported as a program imports it. */
export async function api() {
  return import(pathToFileURL(join(pkg, "lib/index.js")).href);
}

/** The package's bin of `name`. */
export function bin(name) {
  return join(pkg, "bin", `${name}.js`);
}

/** A directory of its own, removed by `done`. */
export function scratch(what) {
  const dir = mkdtempSync(join(tmpdir(), `ritsu-npm-test-${what}-`));
  return { dir, done: () => rmSync(dir, { recursive: true, force: true }) };
}

/** A copy of a directory of the repository in `dir`, as `name`, with an empty .git beside it. */
export function copy(from, dir, name) {
  mkdirSync(join(dir, ".git"), { recursive: true });
  cpSync(join(repo, from), join(dir, name), { recursive: true });
  return join(dir, name);
}

/** The run's own temporary directories left in the system's. */
export function leftovers() {
  return readdirSync(tmpdir()).filter((n) => n.startsWith("ritsu-npm-") && !n.startsWith("ritsu-npm-test-")).sort();
}

/** An environment with no language asked for. */
export function plainEnv(extra = {}) {
  const env = { ...process.env };
  for (const k of Object.keys(env)) {
    if (k.endsWith("_LANG") && ["RITSU", "RULEC", "DANDORI", "KOYOMI", "CHOBO", "GEAS", "YUEN", "SAKAI", "SEKISHO"].includes(k.slice(0, -5))) {
      delete env[k];
    }
  }
  return { ...env, ...extra };
}
