// The package installed as a job over many repositories installs one:
// `npm install --ignore-scripts` from the .tgz, offline, the standard input closed; then its bins
// run in a clone of a repository, and `ritsu gen` writes the packages there. Where the system can
// take the network away from a process (sandbox-exec on macOS, a network namespace on Linux),
// every step runs without it.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { copy, plainEnv, scratch, skip, tgz } from "./helpers.js";

/** `cmd args` with the network taken away, where the system can; else as it is, and why not. */
function offline(cmd, args) {
  if (process.platform === "darwin" && existsSync("/usr/bin/sandbox-exec")) {
    return { cmd: "/usr/bin/sandbox-exec", args: ["-p", "(version 1)(allow default)(deny network*)", cmd, ...args], cut: true };
  }
  if (process.platform === "linux" && spawnSync("unshare", ["-rn", "true"]).status === 0) {
    return { cmd: "unshare", args: ["-rn", cmd, ...args], cut: true };
  }
  return { cmd, args, cut: false };
}

function sh(cmd, args, options) {
  const o = offline(cmd, args);
  const r = spawnSync(o.cmd, o.args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], maxBuffer: 1 << 26, ...options });
  return { code: r.status, stdout: r.stdout ?? "", stderr: r.stderr ?? "", cut: o.cut, error: r.error };
}

test("installed offline from the .tgz, with no script run, the bins work in a repository", { skip: skip || (!tgz && "RITSU_NPM names no .tgz") }, () => {
  const s = scratch("install");
  try {
    const project = join(s.dir, "project");
    copy("crates/ritsu/tests/projects/stockroom", project, ".");
    const cache = join(s.dir, "npm-cache");
    const env = plainEnv({ npm_config_cache: cache, npm_config_update_notifier: "false", npm_config_fund: "false", npm_config_audit: "false" });
    // the job installs into the repository it cloned
    writeFileSync(join(project, "package.json"), JSON.stringify({ name: "a-service", private: true }, null, 2));
    const npm = process.platform === "win32" ? "npm.cmd" : "npm";
    const install = sh(npm, ["install", "--ignore-scripts", "--offline", "--no-save", tgz], { cwd: project, env });
    assert.equal(install.code, 0, `${install.stdout}${install.stderr}${install.error ?? ""}`);
    if (!install.cut) {
      console.log("# note: the network could not be taken away here; the install ran with it");
    }
    const bins = readdirSync(join(project, "node_modules/.bin")).sort();
    assert.deepEqual(bins, ["chobo", "dandori", "geas", "koyomi", "ritsu", "rulec", "sakai", "sekisho", "yuen"]);
    const { version } = JSON.parse(readFileSync(join(project, "node_modules/@i2y/ritsu/package.json"), "utf8"));
    const v = sh(join(project, "node_modules/.bin/ritsu"), ["--version"], { cwd: project, env });
    assert.deepEqual([v.code, v.stdout, v.stderr], [0, `ritsu ${version}\n`, ""]);
    const r = sh(join(project, "node_modules/.bin/rulec"), ["--version"], { cwd: project, env });
    assert.equal(r.stdout, `rulec ${version}\n`);
    const gen = sh(join(project, "node_modules/.bin/ritsu"), ["gen", "--root", ".", "--module", "example.com/stockroom/generated"], { cwd: project, env });
    assert.equal(gen.code, 0, gen.stderr);
    assert.match(gen.stdout, /^generated: generated\/typescript\/rules\/delivery\.ts$/m);
    for (const f of ["generated/typescript/index.ts", "generated/python/generated/__init__.py", "generated/go/doc.go"]) {
      assert.ok(existsSync(join(project, f)), f);
    }
    const check = sh(join(project, "node_modules/.bin/ritsu"), ["gen", "--root", ".", "--module", "example.com/stockroom/generated", "--check", "--format", "json"], { cwd: project, env });
    assert.equal(check.code, 0, check.stderr);
    const json = JSON.parse(check.stdout);
    assert.equal(json.ok, true);
    assert.ok(json.files.length > 0 && json.files.every((f) => f.state === "same"));
    // CommonJS, as many a Node service is built, takes the package with require() where Node can
    if (process.features.require_module) {
      const cjs = sh(process.execPath, ["-e", `const r = require("@i2y/ritsu"); console.log(r.runSync(["--version"]).stdout.trim())`], { cwd: project, env });
      assert.equal(cjs.stdout, `ritsu ${version}\n`, cjs.stderr);
    }
  } finally {
    s.done();
  }
});
