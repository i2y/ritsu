// The host of handles: it takes what people typed, and the generated code of the module says the
// handle kept for each, or why there is none. Written beside the package `ritsu gen` writes
// (typescript/), compiled by tsc and run by Node.
import { handle, tag } from "./glue/handles";

for (const typed of [" Grace Hopper", "9", "Root", "ünïcode"]) {
  const r = handle(typed);
  console.log(r.kind === "ok" ? `"${typed}" -> ${r.value}` : `"${typed}" -> refused: ${r.error.join("; ")}`);
}
for (const typed of ["Ada Lovelace#42", "ada#0"]) {
  const r = tag(typed);
  console.log(r.kind === "ok" ? `"${typed}" -> ${r.value.handle} #${r.value.number}` : `"${typed}" -> refused: ${r.error.join("; ")}`);
}
