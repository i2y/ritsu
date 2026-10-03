// The work directory of a runner: made under the system's temporary directory, and removed when
// the runner's process exits, however it exits — at its end, by process.exit, or on an error
// nothing caught. A runner that ends early used to leave its directory there, and they came to
// thousands. A process killed outright removes nothing; the runner that starts another one gives
// it a directory of its own for its temporary files (TMPDIR), and removes that.
//
//   import { workDir } from "../work.mjs";
//   const work = workDir("dandori-temporal-");

import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const made = [];
process.on("exit", () => {
  for (const d of made) fs.rmSync(d, { recursive: true, force: true });
});

/** A fresh directory, `<prefix>` and six characters, removed when the process exits. */
export function workDir(prefix) {
  const d = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  made.push(d);
  return d;
}
