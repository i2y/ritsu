// @i2y/ritsu: ritsu and its eight languages, built for WASI and run by Node (ritsu's DESIGN 8.8).
//
//   import { run, runSync, version, wasmPath } from "@i2y/ritsu";
//   const { code, stdout, stderr } = await run(["check", ".", "--format", "json"], { cwd: repo });
//
// `run` works in a worker thread and leaves the event loop free; `runSync` works on the thread
// that calls it. Both write what the command reads and prints to temporary files of their own,
// which they remove before they answer.

import { mkdtempSync, openSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Worker } from "node:worker_threads";
import { closeAll, environment, openedDirectories, start, supported, version, wasmPath, workingDirectory } from "./wasi.js";

export { version, wasmPath };

/** What a run is given: the arguments after the program's name, and the options. */
function prepare(args, options = {}) {
  supported();
  if (!Array.isArray(args) || !args.every((a) => typeof a === "string")) {
    throw new TypeError("@i2y/ritsu: the arguments are an array of strings, without the program's name");
  }
  const cwd = workingDirectory(options.cwd);
  const preopens = openedDirectories(options.dirs, cwd, resolve(options.cwd ?? process.cwd()));
  const env = environment(options.env ?? process.env, cwd);
  const stdin = options.stdin ?? "";
  return { args, cwd, env, stdin, preopens };
}

/** The directories of the runs going on, removed when a run ends, or when the process does first. */
const open = new Set();
let watching = false;

/** A directory of its own for one run's standard input, output and error. */
function files(stdin) {
  if (!watching) {
    watching = true;
    process.on("exit", () => {
      for (const dir of open) {
        rmSync(dir, { recursive: true, force: true });
      }
    });
  }
  const dir = mkdtempSync(join(tmpdir(), "ritsu-npm-"));
  open.add(dir);
  const paths = { dir, stdin: join(dir, "stdin"), stdout: join(dir, "stdout"), stderr: join(dir, "stderr") };
  writeFileSync(paths.stdin, stdin);
  writeFileSync(paths.stdout, "");
  writeFileSync(paths.stderr, "");
  return paths;
}

function remove(paths) {
  rmSync(paths.dir, { recursive: true, force: true });
  open.delete(paths.dir);
}

function answer(code, paths) {
  return { code, stdout: readFileSync(paths.stdout, "utf8"), stderr: readFileSync(paths.stderr, "utf8") };
}

/**
 * Run `ritsu <args>` on the thread that calls it.
 * @param {string[]} args
 * @param {{cwd?: string, env?: Record<string, string | undefined>, stdin?: string | Uint8Array, dirs?: string[]}} [options]
 * @returns {{code: number, stdout: string, stderr: string}}
 */
export function runSync(args, options) {
  const o = prepare(args, options);
  const paths = files(o.stdin);
  try {
    const fds = [openSync(paths.stdin, "r"), openSync(paths.stdout, "w"), openSync(paths.stderr, "w")];
    let code;
    try {
      code = start({ args: o.args, env: o.env, preopens: o.preopens, stdin: fds[0], stdout: fds[1], stderr: fds[2] });
    } finally {
      closeAll(fds);
    }
    return answer(code, paths);
  } finally {
    remove(paths);
  }
}

let compiling;
/** The module, compiled once, off the event loop, for the workers to share. */
function compiled() {
  compiling ??= WebAssembly.compile(readFileSync(wasmPath));
  return compiling;
}

/**
 * Run `ritsu <args>` in a worker thread; the event loop goes on while it runs, and any number may
 * run at once.
 * @param {string[]} args
 * @param {{cwd?: string, env?: Record<string, string | undefined>, stdin?: string | Uint8Array, dirs?: string[]}} [options]
 * @returns {Promise<{code: number, stdout: string, stderr: string}>}
 */
export async function run(args, options) {
  const o = prepare(args, options);
  const mod = await compiled();
  const paths = files(o.stdin);
  try {
    const code = await new Promise((resolve, reject) => {
      const worker = new Worker(new URL("./worker.js", import.meta.url), {
        workerData: { mod, args: o.args, env: o.env, preopens: o.preopens, paths },
        // not the flags the caller's Node was started with: some are not for a worker
        // (`--input-type`), and the run needs none
        execArgv: [],
        // the module's own stack is in its memory; this is the engine's, for its calls
        resourceLimits: { stackSizeMb: 8 },
      });
      let said;
      worker.once("message", (m) => {
        said = m;
      });
      worker.once("error", reject);
      worker.once("exit", (exit) => {
        if (said && typeof said.code === "number") {
          resolve(said.code);
        } else if (said && said.error) {
          reject(new Error(said.error));
        } else {
          reject(new Error(`@i2y/ritsu: the worker stopped before it answered (exit ${exit})`));
        }
      });
    });
    return answer(code, paths);
  } finally {
    remove(paths);
  }
}
