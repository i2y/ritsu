// The commands of the package: `ritsu`, and each of the eight languages by its own name (bin/),
// as the links of a release's archive are. The module works on this process's own standard input,
// output and error, so that a command reads a pipe (`geas affected spec.geas -`) and prints as it
// goes; the exit code is the module's. Nothing is written through process.stdout or
// process.stderr, which would make a pipe of theirs non-blocking under the module.

import { writeSync } from "node:fs";
import { environment, start, supported, workingDirectory } from "./wasi.js";

/** Run the command `name` (`ritsu`, `rulec`, …) on the arguments this process was given. */
export function main(name) {
  let code;
  try {
    supported();
    const cwd = workingDirectory(process.cwd());
    code = start({ argv0: name, args: process.argv.slice(2), env: environment(process.env, cwd) });
  } catch (e) {
    writeSync(2, `${name}: ${e?.message ?? e}\n`);
    code = 2;
  }
  process.exitCode = code;
}
