// The commands of @i2y/ritsu: `ritsu` and the eight languages, each a bin of its own (bin/).

import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { api, bin, copy, pkg, plainEnv, scratch, skip } from "./helpers.js";

const NAMES = ["ritsu", "rulec", "dandori", "koyomi", "chobo", "geas", "yuen", "sakai", "sekisho"];

function cli(name, args, options = {}) {
  const r = spawnSync(process.execPath, [bin(name), ...args], { env: plainEnv(), encoding: "utf8", maxBuffer: 1 << 28, ...options });
  return { code: r.status, stdout: r.stdout, stderr: r.stderr };
}

test("each bin is its command, and says the version of the package", { skip }, () => {
  const { version } = JSON.parse(readFileSync(join(pkg, "package.json"), "utf8"));
  for (const name of NAMES) {
    assert.deepEqual(cli(name, ["--version"]), { code: 0, stdout: `${name} ${version}\n`, stderr: "" });
  }
});

test("a bin exits with the command's code, and prints what the API answers", { skip }, async () => {
  const { runSync } = await api();
  const s = scratch("bin");
  try {
    const shop = copy("website/playground/shop", s.dir, "shop");
    const r = cli("ritsu", ["check", "."], { cwd: shop });
    assert.equal(r.code, 1);
    assert.deepEqual(r, runSync(["check", "."], { cwd: shop, env: plainEnv() }));
    assert.equal(cli("ritsu", ["nonsense"]).code, 2);
    assert.deepEqual(cli("rulec", ["check", "parcel_rate.rule"], { cwd: join(s.dir, "shop") }).code, 2);
    const corpus = copy("crates/rulec/tests/corpus", s.dir, "corpus");
    assert.deepEqual(cli("rulec", ["check", "parcel_rate.rule"], { cwd: corpus }), runSync(["rulec", "check", "parcel_rate.rule"], { cwd: corpus, env: plainEnv() }));
  } finally {
    s.done();
  }
});

test("a bin reads a pipe on its standard input", { skip }, () => {
  const s = scratch("pipe");
  try {
    const greeter = join(copy("crates/yuen/examples", s.dir, "examples"), "greeter");
    const diff = readFileSync(join(greeter, "changes/change.diff"));
    const maps = ["--root", ".", "--map", ".geas/greeter.map.jsonl", "--map", "changes/after.map.jsonl"];
    const piped = cli("geas", ["affected", "greeter.geas", "-", ...maps], { cwd: greeter, input: diff });
    assert.equal(piped.code, 0, piped.stderr);
    const named = cli("geas", ["affected", "greeter.geas", "changes/change.diff", ...maps], { cwd: greeter });
    assert.deepEqual(piped, { ...named, stdout: named.stdout.replaceAll("changes/change.diff", "<stdin>") });
    const after = ["--map", "greeter.geas=changes/after.map.jsonl"];
    const yuen = cli("yuen", ["affected", "greeter.req", "--root", ".", "--diff", "-", ...after], { cwd: greeter, input: diff });
    assert.equal(yuen.code, 0, yuen.stdout);
    const file = cli("yuen", ["affected", "greeter.req", "--root", ".", "--diff", "changes/change.diff", ...after], { cwd: greeter });
    assert.deepEqual(yuen, { ...file, stdout: file.stdout.replaceAll("changes/change.diff", "<stdin>") });
  } finally {
    s.done();
  }
});

/** Run a bin with its standard output a pipe read `slowly` (paused that long first). */
function piped(name, args, { cwd, slowly = 0, nonBlocking = false } = {}) {
  return new Promise((resolve) => {
    // a parent that writes to its own standard output (a pipe) first, as npm does, makes that
    // pipe non-blocking for every process that shares it
    const argv = nonBlocking
      ? ["-e", `process.stdout.write(""); const r = require("node:child_process").spawnSync(process.execPath, ${JSON.stringify([bin(name), ...args])}, { stdio: "inherit" }); process.exit(r.status ?? 99);`]
      : [bin(name), ...args];
    const child = spawn(process.execPath, argv, { cwd, env: plainEnv(), stdio: ["ignore", "pipe", "pipe"] });
    const out = [];
    let err = "";
    child.stderr.on("data", (d) => (err += d));
    child.stdout.on("data", (d) => out.push(d));
    if (slowly) {
      child.stdout.pause();
      setTimeout(() => child.stdout.resume(), slowly);
    }
    child.on("close", (code) => resolve({ code, stdout: Buffer.concat(out).toString("utf8"), stderr: err }));
  });
}

test("a reader that goes away ends the command quietly, as SIGPIPE ends the native one", { skip }, () => {
  // `| head -1` in a shell: the reader takes one line of more than a pipe holds and goes
  const s = scratch("sigpipe");
  try {
    const err = join(s.dir, "stderr");
    const script = `"${process.execPath}" "${bin("rulec")}" explain --all --format markdown 2>"$1" | head -1 >/dev/null; echo "\${PIPESTATUS[0]}"`;
    const r = spawnSync("bash", ["-c", script, "bash", err], { env: plainEnv(), encoding: "utf8" });
    assert.equal(r.stdout, "141\n", r.stderr);
    assert.equal(readFileSync(err, "utf8"), "");
  } finally {
    s.done();
  }
});

test("a pipe made non-blocking by a process that shares it is waited on, not given up", { skip }, async () => {
  const want = cli("rulec", ["explain", "--all", "--format", "markdown"]).stdout;
  assert.ok(want.length > 1 << 16, "more than a pipe holds");
  const r = await piped("rulec", ["explain", "--all", "--format", "markdown"], { slowly: 1000, nonBlocking: true });
  assert.equal(r.stderr, "");
  assert.equal(r.code, 0);
  assert.equal(r.stdout, want);
});

test("no warning of WASI comes from a bin", { skip }, () => {
  const r = cli("ritsu", ["explain", "E201"]);
  assert.equal(r.code, 0);
  assert.equal(r.stderr, "");
});
