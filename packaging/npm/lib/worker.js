// One run of `run` (index.js), in a worker thread: the module compiled once by the thread that
// asked, the files of the run's standard input, output and error, which that thread removes.

import { openSync } from "node:fs";
import { parentPort, workerData } from "node:worker_threads";
import { closeAll, start } from "./wasi.js";

const { mod, args, env, preopens, paths } = workerData;
const fds = [openSync(paths.stdin, "r"), openSync(paths.stdout, "w"), openSync(paths.stderr, "w")];
try {
  const code = start({ mod, args, env, preopens, stdin: fds[0], stdout: fds[1], stderr: fds[2] });
  parentPort.postMessage({ code });
} catch (e) {
  parentPort.postMessage({ error: e?.stack ?? String(e) });
} finally {
  closeAll(fds);
}
