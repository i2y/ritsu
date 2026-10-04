// geas's coverage hook for Node. While `geas map` runs, geas
// preloads it with --require in NODE_OPTIONS. V8 writes NODE_V8_COVERAGE when a
// process exits normally, and not when a signal ends it, so on SIGTERM this exits
// with 143, a normal exit, unless the program handles SIGTERM itself, in which case
// the program's own shutdown ends it. It does not call v8.takeCoverage(): that
// resets the counts, and the exit then writes a file without them under the same
// name when it falls in the same millisecond, which it did in 33 runs of 60.
"use strict";
const { isMainThread } = require("node:worker_threads");
if (isMainThread) {
  process.on("SIGTERM", function geasCover() {
    if (process.listeners("SIGTERM").length === 1) process.exit(143);
  });
}
