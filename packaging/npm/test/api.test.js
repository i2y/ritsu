// The API of @i2y/ritsu: run and runSync, version and wasmPath (lib/index.js).

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { isAbsolute, join, relative } from "node:path";
import { pathToFileURL } from "node:url";
import { test } from "node:test";
import { api, copy, leftovers, pkg, plainEnv, scratch, skip } from "./helpers.js";

test("version and wasmPath name the module the package carries", { skip }, async () => {
  const { version, wasmPath, runSync } = await api();
  const manifest = JSON.parse(readFileSync(join(pkg, "package.json"), "utf8"));
  assert.equal(version, manifest.version);
  assert.ok(isAbsolute(wasmPath) && wasmPath.endsWith("ritsu.wasm") && existsSync(wasmPath));
  assert.equal(runSync(["--version"]).stdout, `ritsu ${version}\n`);
});

test("the exit codes are the command's: 0, 1 and 2", { skip }, async () => {
  const { runSync, run } = await api();
  const s = scratch("codes");
  try {
    const shop = copy("website/playground/shop", s.dir, "shop");
    const env = plainEnv();
    assert.deepEqual({ ...runSync(["--version"], { env }), stdout: "" }, { code: 0, stdout: "", stderr: "" });
    const failing = runSync(["check", "."], { cwd: shop, env });
    assert.equal(failing.code, 1);
    assert.match(failing.stdout, /error\[rulec E032\]/);
    const asked = await run(["check", ".", "--format", "json"], { cwd: shop, env });
    assert.equal(asked.code, 1);
    assert.equal(JSON.parse(asked.stdout).ok, false);
    const wrong = runSync(["nonsense"], { env });
    assert.equal(wrong.code, 2);
    assert.equal(wrong.stderr, "error: there is no command `nonsense`; run `ritsu --help`\n");
  } finally {
    s.done();
  }
});

test("a language's command is `ritsu <language> …`", { skip }, async () => {
  const { runSync, run, version } = await api();
  assert.equal(runSync(["rulec", "--version"]).stdout, `rulec ${version}\n`);
  assert.equal((await run(["sekisho", "--version"])).stdout, `sekisho ${version}\n`);
});

test("the standard input is what `stdin` gives, a string or bytes, and else nothing", { skip }, async () => {
  const { runSync, run } = await api();
  const s = scratch("stdin");
  try {
    const greeter = join(copy("crates/yuen/examples", s.dir, "examples"), "greeter");
    const diff = readFileSync(join(greeter, "changes/change.diff"));
    const env = plainEnv();
    const fromFile = runSync(["geas", "affected", "greeter.geas", "changes/change.diff", "--root", ".", "--map", ".geas/greeter.map.jsonl", "--map", "changes/after.map.jsonl"], { cwd: greeter, env });
    assert.equal(fromFile.code, 0, fromFile.stderr);
    const piped = runSync(["geas", "affected", "greeter.geas", "-", "--root", ".", "--map", ".geas/greeter.map.jsonl", "--map", "changes/after.map.jsonl"], { cwd: greeter, env, stdin: diff.toString("utf8") });
    assert.equal(piped.code, 0, piped.stderr);
    assert.deepEqual(piped, { ...fromFile, stdout: fromFile.stdout.replaceAll("changes/change.diff", "<stdin>") });
    const bytes = await run(["geas", "affected", "greeter.geas", "-", "--root", ".", "--map", ".geas/greeter.map.jsonl", "--map", "changes/after.map.jsonl"], { cwd: greeter, env, stdin: new Uint8Array(diff) });
    assert.deepEqual(bytes, piped);
    const after = ["--map", "greeter.geas=changes/after.map.jsonl"];
    const yuen = await run(["yuen", "affected", "greeter.req", "--root", ".", "--diff", "-", ...after], { cwd: greeter, env, stdin: diff });
    assert.equal(yuen.code, 0, yuen.stdout);
    const named = runSync(["yuen", "affected", "greeter.req", "--root", ".", "--diff", "changes/change.diff", ...after], { cwd: greeter, env });
    assert.deepEqual(yuen, { ...named, stdout: named.stdout.replaceAll("changes/change.diff", "<stdin>") });
    // nothing given: the end of the input at once
    const empty = runSync(["geas", "affected", "greeter.geas", "-", "--root", ".", "--map", ".geas/greeter.map.jsonl", "--map", "changes/after.map.jsonl"], { cwd: greeter, env });
    assert.deepEqual(empty, runSync(["geas", "affected", "greeter.geas", "-", "--root", ".", "--map", ".geas/greeter.map.jsonl", "--map", "changes/after.map.jsonl"], { cwd: greeter, env, stdin: "" }));
  } finally {
    s.done();
  }
});

test("the command works in `cwd`, which has to be a directory that is there", { skip }, async () => {
  const { runSync, run } = await api();
  const s = scratch("cwd");
  try {
    const corpus = copy("crates/rulec/tests/corpus", s.dir, "corpus");
    const env = plainEnv();
    assert.equal(runSync(["rulec", "check", "parcel_rate.rule"], { cwd: corpus, env }).code, 0);
    assert.equal(runSync(["rulec", "check", "parcel_rate.rule"], { cwd: s.dir, env }).code, 2);
    // a directory reached through a link is where its target is, as the shell's would be
    const link = join(s.dir, "through-a-link");
    symlinkSync(corpus, link);
    const there = runSync(["rulec", "doc", "parcel_rate.rule"], { cwd: link, env });
    assert.deepEqual(there, runSync(["rulec", "doc", "parcel_rate.rule"], { cwd: corpus, env }));
    assert.throws(() => runSync(["check"], { cwd: join(s.dir, "not-there") }), /is not there/);
    await assert.rejects(run(["check"], { cwd: join(corpus, "parcel_rate.rule") }), /is not a directory/);
  } finally {
    s.done();
  }
});

test("`env` replaces process.env, and RITSU_LANG chooses the language", { skip }, async () => {
  const { runSync, run } = await api();
  const s = scratch("env");
  try {
    const shop = copy("website/playground/shop", s.dir, "shop");
    const ja = runSync(["check", "."], { cwd: shop, env: { RITSU_LANG: "ja" } });
    assert.match(ja.stdout, /ritsu check: ファイル 17 個/);
    const before = process.env.RITSU_LANG;
    process.env.RITSU_LANG = "ja";
    try {
      const en = await run(["check", "."], { cwd: shop, env: { PATH: process.env.PATH } });
      assert.match(en.stdout, /ritsu check: 17 files/);
      assert.match((await run(["check", "."], { cwd: shop })).stdout, /ritsu check: ファイル 17 個/);
    } finally {
      if (before === undefined) delete process.env.RITSU_LANG;
      else process.env.RITSU_LANG = before;
    }
    // a value left undefined is left out, as child_process leaves it
    assert.match(runSync(["check", "."], { cwd: shop, env: { RITSU_LANG: undefined } }).stdout, /17 files/);
  } finally {
    s.done();
  }
});

test("run leaves the event loop free, and many runs go at once", { skip }, async () => {
  const { run, runSync } = await api();
  const s = scratch("many");
  try {
    const shop = copy("website/playground/shop", s.dir, "shop");
    const env = plainEnv();
    const want = runSync(["check", ".", "--format", "json"], { cwd: shop, env });
    let ticks = 0;
    const timer = setInterval(() => ticks++, 5);
    const all = await Promise.all(Array.from({ length: 8 }, (_, i) => (i % 2 ? run(["check", ".", "--format", "json"], { cwd: shop, env }) : run(["explain", "E201"], { env }))));
    clearInterval(timer);
    assert.ok(ticks > 0, "the timer ticked while the runs went on");
    for (const [i, r] of all.entries()) {
      if (i % 2) assert.deepEqual(r, want);
      else assert.match(r.stdout, /^E201 /);
    }
  } finally {
    s.done();
  }
});

test("no temporary file is left, whatever the run came to", { skip }, async () => {
  const { run, runSync } = await api();
  const before = leftovers();
  runSync(["--version"]);
  runSync(["nonsense"]);
  await run(["--version"]);
  await Promise.all([run(["check", "/no/such/place"]), run(["explain", "E201"]), run(["nonsense"])]);
  assert.throws(() => runSync(["check"], { cwd: "/no/such/place" }));
  assert.deepEqual(leftovers(), before);
  // a process that ends while runs go on removes theirs on its way out
  const child = spawnSync(process.execPath, ["--input-type=module", "-e", `import { run } from ${JSON.stringify(pathToFileURL(join(pkg, "lib/index.js")).href)}; run(["explain", "--all"]); setTimeout(() => process.exit(0), 50);`]);
  assert.equal(child.status, 0, child.stderr.toString());
  assert.deepEqual(leftovers(), before);
});

const modules = {
  // writes what a panic's hook writes to the standard error, then reaches `unreachable`, as an abort does
  abort: "AGFzbQEAAAABDAJgBH9/f38Bf2AAAAIjARZ3YXNpX3NuYXBzaG90X3ByZXZpZXcxCGZkX3dyaXRlAAADAgEBBQMBAAEHEwIGbWVtb3J5AgAGX3N0YXJ0AAEKHgEcAEEAQRA2AgBBBEE+NgIAQQJBAEEBQQgQABoACwtEAQBBEAs+cml0c3U6IGEgYnVnIGluIHJpdHN1LCBwbGVhc2UgcmVwb3J0IGl0OiBhIHRlc3Qgb2YgdGhlIGxvYWRlcgoAEgRuYW1lAQsBAAhmZF93cml0ZQ==",
  // calls itself until the engine's stack is used up
  deep: "AGFzbQEAAAABCQJgAX8Bf2AAAAMDAgABBQMBAAEHEwIGbWVtb3J5AgAGX3N0YXJ0AAEKEwIJACAAQQFqEAALBwBBABAAGgsADgRuYW1lAQcBAARkb3du",
  // exits with 3
  exits: "AGFzbQEAAAABCAJgAX8AYAAAAiQBFndhc2lfc25hcHNob3RfcHJldmlldzEJcHJvY19leGl0AAADAgEBBQMBAAEHEwIGbWVtb3J5AgAGX3N0YXJ0AAEKCAEGAEEDEAALAA4EbmFtZQEHAQAEZXhpdA==",
};

test("a trap is exit 2, with what the panic's hook wrote kept", { skip }, async () => {
  const wasi = await import(pathToFileURL(join(pkg, "lib/wasi.js")).href);
  const { openSync, closeSync } = await import("node:fs");
  const s = scratch("trap");
  try {
    const go = (name) => {
      const err = join(s.dir, `${name}.err`);
      writeFileSync(err, "");
      const fd = openSync(err, "w");
      try {
        const code = wasi.start({ args: [], env: {}, mod: new WebAssembly.Module(Buffer.from(modules[name], "base64")), stderr: fd });
        return { code, stderr: readFileSync(err, "utf8") };
      } finally {
        closeSync(fd);
      }
    };
    assert.deepEqual(go("exits"), { code: 3, stderr: "" });
    assert.deepEqual(go("abort"), { code: 2, stderr: "ritsu: a bug in ritsu, please report it: a test of the loader\n" });
    const deep = go("deep");
    assert.equal(deep.code, 2);
    assert.match(deep.stderr, /^ritsu: the WebAssembly module stopped: Maximum call stack size exceeded\n$/);
  } finally {
    s.done();
  }
});

test("Node's warning about WASI is not said; every other warning is", { skip }, () => {
  const index = JSON.stringify(pathToFileURL(join(pkg, "lib/index.js")).href);
  const r = spawnSync(process.execPath, ["--input-type=module", "-e", `process.emitWarning("a warning of its own", "ExperimentalWarning"); const m = await import(${index}); m.runSync(["--version"]); await m.run(["--version"]);`]);
  const err = r.stderr.toString();
  assert.equal(r.status, 0, err);
  assert.match(err, /a warning of its own/);
  assert.doesNotMatch(err, /WASI/);
});

test("CommonJS takes the package with require() on a Node that loads ES modules so", { skip: skip || (!process.features.require_module && "this Node does not require() an ES module") }, () => {
  const require = createRequire(join(pkg, "..", "index.js"));
  const m = require(pkg);
  assert.equal(m.runSync(["--version"]).stdout, `ritsu ${m.version}\n`);
});

test("on Windows the loader stops at once, saying why", { skip }, () => {
  const index = JSON.stringify(pathToFileURL(join(pkg, "lib/index.js")).href);
  const r = spawnSync(process.execPath, ["--input-type=module", "-e", `Object.defineProperty(process, "platform", { value: "win32" }); const m = await import(${index}); try { m.runSync(["--version"]); } catch (e) { console.log(e.message); }`]);
  assert.match(r.stdout.toString(), /does not run on Windows yet/);
});

test("one process runs command after command, on this thread and in workers", { skip }, async () => {
  // Node 22 died of SIGSEGV here, until the loader stopped its fast calls into WASI (lib/wasi.js)
  const { run, runSync } = await api();
  const s = scratch("long");
  try {
    const shop = copy("website/playground/shop", s.dir, "shop");
    const env = plainEnv();
    const want = runSync(["check", ".", "--format", "json"], { cwd: shop, env });
    for (let i = 0; i < 40; i++) {
      assert.deepEqual(runSync(["check", ".", "--format", "json"], { cwd: shop, env }), want);
    }
    for (let i = 0; i < 5; i++) {
      for (const r of await Promise.all(Array.from({ length: 8 }, () => run(["check", ".", "--format", "json"], { cwd: shop, env })))) {
        assert.deepEqual(r, want);
      }
    }
  } finally {
    s.done();
  }
});

test("dirs: the module opens those directories alone, each at its own path", { skip }, async () => {
  const { runSync, run } = await api();
  // under os.tmpdir(): on macOS /var/folders, through the link /var to /private/var, whose
  // directories above the project the module cannot see once it is given the project alone
  const s = scratch("dirs");
  try {
    const env = plainEnv();
    const project = copy("website/playground/shop", s.dir, "project");
    mkdirSync(join(project, ".git"));
    // the rule takes its enum from a .proto beside the project, not in it
    const proto = readFileSync(join(project, "proto/shop/v1/order.proto"), "utf8").replace("ORDER_STATUS_RETURNED = 5;", "ORDER_STATUS_RETURNED = 5;\n  ORDER_STATUS_HOST_SECRET_VALUE = 6;");
    writeFileSync(join(s.dir, "outside.proto"), proto);
    const rule = join(project, "billing/rules/billing_need.rule");
    writeFileSync(rule, readFileSync(rule, "utf8").replace('"../../proto/shop/v1/order.proto"', '"../../../outside.proto"'));
    // links in the project to a file outside it, to the system's /etc/hosts, and to /etc itself,
    // whose hosts uvwasi alone would let through: it leaves the directories on the way to the host
    writeFileSync(join(s.dir, "outside.rule"), "rule outside_secret_word v1\n");
    symlinkSync(join(s.dir, "outside.rule"), join(project, "linked.rule"));
    symlinkSync("../outside.rule", join(project, "relinked.rule"));
    symlinkSync("/etc/hosts", join(project, "hosts.rule"));
    symlinkSync("/etc", join(project, "etc"));
    // and links that stay in it
    symlinkSync("ordering/rules/urgency.rule", join(project, "urgency.rule"));
    symlinkSync("ordering/rules", join(project, "ordering-rules"));
    symlinkSync(join(project, "ordering"), join(project, "ordering-again"));
    const both = (r) => r.stdout + r.stderr;
    // the whole filesystem: what the native binary reads, the .proto outside the project too
    for (const args of [["rulec", "check", "billing/rules/billing_need.rule"], ["check", ".", "--root", "."]]) {
      assert.match(both(runSync(args, { cwd: project, env })), /host_secret_value/);
    }
    assert.match(both(runSync(["rulec", "check", "linked.rule"], { cwd: project, env })), /outside_secret_word/);
    // the project alone: nothing outside it is read, and a link out of it leads nowhere
    const narrowed = { cwd: project, env, dirs: [project] };
    for (const args of [["rulec", "check", "billing/rules/billing_need.rule"], ["check", ".", "--root", "."], ["check", "billing/rules/billing_need.rule"]]) {
      const r = runSync(args, narrowed);
      assert.doesNotMatch(both(r), /host_secret_value/, args.join(" "));
      assert.match(both(r), /outside\.proto/, `${args.join(" ")} says which file it could not read`);
      assert.notEqual(r.code, 0);
    }
    for (const file of ["linked.rule", "relinked.rule"]) {
      const r = runSync(["rulec", "check", file], narrowed);
      assert.doesNotMatch(both(r), /outside_secret_word/, file);
      assert.deepEqual(r, { code: 2, stdout: "", stderr: `error: cannot read \`${file}\`\n` });
    }
    assert.match(both(runSync(["rulec", "check", "etc/hosts"], { cwd: project, env })), /error\[E003\]/);
    for (const file of ["hosts.rule", "etc/hosts"]) {
      const r = runSync(["rulec", "check", file], narrowed);
      assert.deepEqual(r, { code: 2, stdout: "", stderr: `error: cannot read \`${file}\`\n` });
    }
    // with two directories, a path is followed from the directory it is opened from, in either order
    const other = join(s.dir, "other");
    mkdirSync(join(other, "etc"), { recursive: true });
    writeFileSync(join(other, "etc", "hosts"), "rule other_hosts v1\n");
    for (const dirs of [[project, other], [other, project]]) {
      assert.deepEqual(runSync(["rulec", "check", "etc/hosts"], { cwd: project, env, dirs }), { code: 2, stdout: "", stderr: "error: cannot read `etc/hosts`\n" });
    }
    for (const file of ["urgency.rule", "ordering-rules/urgency.rule", "ordering-again/rules/urgency.rule"]) {
      const r = runSync(["rulec", "check", file], narrowed);
      assert.deepEqual(r, { code: 0, stdout: `ok ${file}\n`, stderr: "" });
    }
    // nothing is written through a link out of them either
    const stock = copy("crates/ritsu/tests/projects/stockroom", s.dir, "stockroom");
    mkdirSync(join(stock, ".git"));
    mkdirSync(join(s.dir, "elsewhere"));
    symlinkSync(join(s.dir, "elsewhere"), join(stock, "elsewhere"));
    const out = runSync(["gen", "--target", "typescript", "--out", "elsewhere/generated"], { cwd: stock, env, dirs: [stock] });
    assert.deepEqual(out, { code: 2, stdout: "", stderr: "error: cannot create `elsewhere/generated/typescript/rules`\n" });
    assert.deepEqual(readdirSync(join(s.dir, "elsewhere")), []);
    // in a worker too, and the same as the whole filesystem gives where nothing outside is read
    assert.deepEqual(await run(["rulec", "check", "billing/rules/billing_need.rule"], narrowed), runSync(["rulec", "check", "billing/rules/billing_need.rule"], narrowed));
    const doc = runSync(["rulec", "doc", "ordering/rules/urgency.rule"], narrowed);
    assert.equal(doc.code, 0, both(doc));
    assert.deepEqual(doc, runSync(["rulec", "doc", "ordering/rules/urgency.rule"], { cwd: project, env }));
    // a directory below the one given, and what canonicalizes a path there (yuen finds its root
    // so): relative, absolute as the caller wrote it (on macOS through the link /var), and through
    // a link in it that points by an absolute path
    const examples = copy("crates/yuen/examples", s.dir, "examples");
    const greeter = join(examples, "greeter");
    symlinkSync(greeter, join(examples, "greeter-again"));
    for (const [cwd, args] of [
      [examples, ["yuen", "check", "greeter/greeter.req", "--root", "greeter"]],
      [greeter, ["yuen", "check", "greeter.req", "--root", "."]],
      [examples, ["yuen", "check", join(greeter, "greeter.req"), "--root", greeter]],
      [examples, ["yuen", "check", "greeter-again/greeter.req", "--root", "greeter-again"]],
      [greeter, ["geas", "affected", "greeter.geas", "changes/change.diff", "--root", ".", "--map", ".geas/greeter.map.jsonl", "--map", "changes/after.map.jsonl"]],
    ]) {
      const whole = runSync(args, { cwd, env });
      assert.equal(whole.code, 0, both(whole));
      assert.deepEqual(runSync(args, { cwd, env, dirs: [examples] }), whole, args.join(" "));
      // a relative one is taken from cwd
      assert.deepEqual(runSync(args, { cwd, env, dirs: [relative(cwd, examples) || "."] }), whole, args.join(" "));
    }
    // what cannot be: a working directory in none of them, a directory that is not there
    assert.throws(() => runSync(["check"], { cwd: s.dir, dirs: [project] }), /is in none of dirs/);
    assert.throws(() => runSync(["check"], { cwd: project, dirs: [join(s.dir, "nowhere")] }), /is not there/);
    assert.throws(() => runSync(["check"], { cwd: project, dirs: [] }), /one directory or more/);
    await assert.rejects(run(["check"], { cwd: s.dir, dirs: [project] }), /is in none of dirs/);
  } finally {
    s.done();
  }
});
