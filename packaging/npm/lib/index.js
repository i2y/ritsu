// @i2y/ritsu: ritsu and its nine languages, built for WASI and run by Node (ritsu's DESIGN 8.8).
//
//   import { run, runSync, version, wasmPath } from "@i2y/ritsu";
//   const { code, stdout, stderr } = await run(["check", ".", "--format", "json"], { cwd: repo });
//
// `run` works in a worker thread and leaves the event loop free; `runSync` works on the thread
// that calls it. Both hand the command temporary files of their own for its standard input, output
// and error, which are gone from the disk before the command starts: the run holds them open and
// reads what was printed from them.

import { fstatSync, mkdtempSync, openSync, readFileSync, readSync, rmSync, writeFileSync } from "node:fs";
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

/**
 * The files of one run's standard input (holding `stdin`), output and error, open: `fds`. They are
 * in a directory of their own, which is removed as soon as they are open, before the command
 * starts: nothing of the run is on the disk while it goes on, and a process that ends or is killed
 * while the command runs leaves nothing of it. A worker is handed the descriptors, never a path: one
 * that opened its files by their paths, as the process ended, made them again in the directory
 * being removed, and the directory stayed. Where the system cannot remove a directory while files
 * in it are open (NFS keeps them as `.nfs…` files), it is `left`, for `close` to remove after the
 * command.
 */
function files(stdin) {
  const dir = mkdtempSync(join(tmpdir(), "ritsu-npm-"));
  const fds = [];
  try {
    const input = join(dir, "stdin");
    writeFileSync(input, stdin);
    fds.push(openSync(input, "r"), openSync(join(dir, "stdout"), "w+"), openSync(join(dir, "stderr"), "w+"));
  } catch (e) {
    closeAll(fds);
    removed(dir);
    throw e;
  }
  return { fds, left: removed(dir) ? null : dir };
}

/** Whether the directory `dir` is gone, with all in it. */
function removed(dir) {
  try {
    rmSync(dir, { recursive: true, force: true });
    return true;
  } catch {
    return false;
  }
}

/** The end of a run's files: closed, and their directory removed if it was left. */
function close(f) {
  closeAll(f.fds);
  if (f.left) {
    removed(f.left);
  }
}

/** What was written to the descriptor `fd`: its file from the start, as UTF-8. */
function written(fd) {
  const bytes = Buffer.allocUnsafe(fstatSync(fd).size);
  let at = 0;
  while (at < bytes.length) {
    const n = readSync(fd, bytes, at, bytes.length - at, at);
    if (n === 0) {
      break;
    }
    at += n;
  }
  return bytes.toString("utf8", 0, at);
}

function answer(code, fds) {
  return { code, stdout: written(fds[1]), stderr: written(fds[2]) };
}

/**
 * Run `ritsu <args>` on the thread that calls it.
 * @param {string[]} args
 * @param {{cwd?: string, env?: Record<string, string | undefined>, stdin?: string | Uint8Array, dirs?: string[]}} [options]
 * @returns {{code: number, stdout: string, stderr: string}}
 */
export function runSync(args, options) {
  const o = prepare(args, options);
  const f = files(o.stdin);
  try {
    const code = start({ args: o.args, env: o.env, preopens: o.preopens, stdin: f.fds[0], stdout: f.fds[1], stderr: f.fds[2] });
    return answer(code, f.fds);
  } finally {
    close(f);
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
  const f = files(o.stdin);
  try {
    const code = await new Promise((resolve, reject) => {
      const worker = new Worker(new URL("./worker.js", import.meta.url), {
        workerData: { mod, args: o.args, env: o.env, preopens: o.preopens, fds: f.fds },
        // not the flags the caller's Node was started with: some are not for a worker
        // (`--input-type`), and the run needs none
        execArgv: [],
        // the module's own stack is in its memory; this is the engine's, for its calls
        resourceLimits: { stackSizeMb: 8 },
      });
      let said;
      let failed;
      worker.once("message", (m) => {
        said = m;
      });
      worker.once("error", (e) => {
        failed = e;
      });
      // only once the thread has ended: the descriptors are closed after this, and a number closed
      // while the thread still wrote to it could be another file's by then
      worker.once("exit", (exit) => {
        if (said && typeof said.code === "number") {
          resolve(said.code);
        } else if (failed) {
          reject(failed);
        } else if (said && said.error) {
          reject(new Error(said.error));
        } else {
          reject(new Error(`@i2y/ritsu: the worker stopped before it answered (exit ${exit})`));
        }
      });
    });
    return answer(code, f.fds);
  } finally {
    close(f);
  }
}
