// One run of `run` (index.js), in a worker thread: the module compiled once by the thread that
// asked, and the descriptors of the run's standard input, output and error, which that thread
// opened, and reads and closes once this one has ended.

import { parentPort, workerData } from "node:worker_threads";
import { start } from "./wasi.js";

const { mod, args, env, preopens, fds } = workerData;
try {
  const code = start({ mod, args, env, preopens, stdin: fds[0], stdout: fds[1], stderr: fds[2] });
  parentPort.postMessage({ code });
} catch (e) {
  parentPort.postMessage({ error: e?.stack ?? String(e) });
}
