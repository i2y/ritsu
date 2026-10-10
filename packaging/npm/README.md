# @i2y/ritsu

[ritsu](https://i2y.github.io/ritsu/) is eight small languages for the rules a system has to keep —
business rules (rulec), workflows (dandori), dates (koyomi), ledgers (chobo), claims about the code
(geas), where requirements come from (yuen), bounded contexts (sakai) and who may do what (sekisho) —
and one command that checks each file with its language and then the places where the languages
meet. From the checked files it generates code, pages for people, and the inputs to test with.

This package is that command built for WebAssembly (WASI), which Node runs: every command of the
native `ritsu`, printing what the native binary prints, and an API to run them from JavaScript. It
needs no native binary, has no dependencies, and runs nothing when it is installed.

In Japanese: [Node で使う](https://i2y.github.io/ritsu/ja/node/).

## Install

The package is not on the npm registry. Every release of ritsu from 0.26.0 on carries it as
`i2y-ritsu-<version>.tgz`, with its SHA-256 in the release's `SHA256SUMS`:

```console
$ npm install --save-dev https://github.com/i2y/ritsu/releases/download/v0.26.0/i2y-ritsu-0.26.0.tgz
```

Or download the file, check it against `SHA256SUMS`, and install the file itself
(`npm install --save-dev ./i2y-ritsu-0.26.0.tgz`), which works with `--ignore-scripts` and with no
network (`--offline`).

It needs Node 22 or later (22, 24 and 26 are tested), on Linux or macOS.

On Node 22, loading the package turns off V8's fast API calls for the whole process
(`v8.setFlagsFromString("--no-turbo-fast-api-calls")`): Node 22's WASI can crash the process when
it is called that way, and Node 23 and later do not. The flag makes those calls slower, not
different, but a program that loads the package on Node 22 runs under it too. On Node 23 and later
nothing is set.

## The commands

The package puts nine commands into `node_modules/.bin`: `ritsu`, and one named for each language,
`rulec`, `dandori`, `koyomi`, `chobo`, `geas`, `yuen`, `sakai` and `sekisho`, each the same as
`ritsu <language> …`. Run them with `npx`, or from the `scripts` of your package.json. In a project
with a rule, a calendar, a dates file, a ledger and a workflow:

<!-- run in crates/ritsu/tests/projects/stockroom -->
```console
$ npx ritsu check
ok rules/delivery.rule
…
ritsu check: 5 files (rulec 1, koyomi 2, chobo 1, dandori 1): all pass (4 warnings); borders between the languages: 4 checked, 4 undecided
$ npx rulec check rules/delivery.rule
ok rules/delivery.rule
$ npx ritsu gen --target typescript
generated: generated/typescript/rules/delivery.ts
generated: generated/typescript/dates/weekdays.ts
…
```

`ritsu gen --check` writes nothing and says what is stale, and with `--format json` it says it to a
program: every file of the packages with what became of it.

```console
$ echo '// changed by hand' >> generated/typescript/index.ts
$ npx ritsu gen --target typescript --check
generated file is stale or hand-edited: generated/typescript/index.ts
$ npx ritsu gen --target typescript --check --format json
{
…
  "ok": false,
  "check": true,
  "out": "generated",
  "targets": [
    "typescript"
  ],
  "files": [
    {
      "target": "typescript",
      "path": "generated/typescript/rules/delivery.ts",
      "from": "rules/delivery.rule",
      "state": "same"
    },
…
    {
      "target": "typescript",
      "path": "generated/typescript/index.ts",
      "from": null,
      "state": "stale"
    },
…
  ],
  "diagnostics": [],
  "error": null
}
```

The exit codes are the native command's: 0 nothing wrong (warnings may be printed), 1 an error, 2
bad arguments or a file that cannot be read. The standard input, output and error are the process's
own, so a command reads a pipe (`git diff | npx geas affected spec.geas -`) and prints as it goes.

## The API

```js
import { run, runSync, version, wasmPath } from "@i2y/ritsu";
```

- `run(args, options?)` runs `ritsu <args>` in a worker thread and returns a promise of
  `{ code, stdout, stderr }`. The event loop goes on while it runs, and any number of runs may go at
  once.
- `runSync(args, options?)` runs it on the thread that calls it and returns `{ code, stdout, stderr }`.
- `args` leaves out the program's name: `["check", ".", "--format", "json"]`, and a language's
  command is `["rulec", "doc", "fee.rule"]`.
- `options.cwd` is the directory the command works in (by default `process.cwd()`; one that is not
  there is an error), `options.env` its environment (by default `process.env`; given, it replaces
  it, as `child_process` does), and `options.stdin` what it reads from its standard input (a string
  or a `Uint8Array`; by default nothing).
- `options.dirs` narrows what the module can open to those directories, each at its own path (a
  relative one from `options.cwd`; by default the whole filesystem, as the native binary has it);
  `options.cwd` has to be in one of them. A service that checks a repository it was handed can
  give it that repository alone. A path in it that leads out of it, by its name
  (`import proto "../../other.proto"`) or through a symbolic link (to a file, or to a directory on
  the way), can then be neither read nor written: the command says it cannot read it, and nothing
  of what is outside comes into a diagnostic. It narrows what ritsu, which you trust, reads; it
  does not make Node's WASI a sandbox for code you do not trust, which Node's documentation says
  not to rely on it for.
- `version` is the version of ritsu the module was built from, and `wasmPath` the path of the module.

A run writes its standard input, output and error to temporary files of its own, which it removes
before it answers. The same project, its package changed by hand as above:

```js
import { run } from "@i2y/ritsu";

const { code, stdout } = await run(["gen", "--target", "typescript", "--check", "--format", "json"]);
const report = JSON.parse(stdout);
console.log(code, report.ok, report.files.filter((f) => f.state !== "same").map((f) => `${f.state} ${f.path}`));
```

```text
1 false [ 'stale generated/typescript/index.ts' ]
```

From CommonJS, `require("@i2y/ritsu")` gives the same, on Node 22.12 and later.

## What does not run here

A WebAssembly module under WASI can neither start another program nor open a connection. The
commands that need either stop with a message that says so and says to run them with the native
ritsu (a release's archive, Homebrew, the `.deb` or the `.rpm`); each exits with the code it gives
when the program it needs is missing:

| Command | What it needs |
|---|---|
| `rulec source fetch`, `rulec source outdated`, and the same of `koyomi` and `yuen` | the network (e-Gov, the eCFR, GOV.UK, a URL), through `curl` |
| `rulec test` | the toolchains the generated code is run with |
| `rulec verify --adapter …`, `rulec source fetch --via …` | the program it is given |
| `rulec mcp` | `rulec`, started for each call of a tool |
| `rulec check --diff-base`, `rulec diff` and `rulec replay` of `<file>@<revision>`, `chobo check --diff-base` | `git` |
| `geas check`, `geas snap`, `geas drift`, `geas map` | the programs the claims run |
| `sakai check` (and `ritsu check`) of a map with `code rust` | `cargo metadata`; sakai says E107 and checks the rest |

Everything else runs as the native binary does: every language's `check`, `ritsu check` across the
languages, `ritsu gen` and `ritsu run`, every `doc`, `api`, `vectors`, `export`, `trace`, `affected`,
`scenarios` and `explain`, and `ritsu skills install`.

On Windows the package stops at once, with a message: the working directory and the paths cannot
be handed to the WebAssembly module as they are. Use the native ritsu, or WSL.

## License

`(MIT OR Apache-2.0) AND Unicode-3.0 AND BSD-3-Clause`, as the native binary. ritsu's own code is
MIT OR Apache-2.0 (`LICENSE-MIT` and `LICENSE-APACHE` in the package); rulec holds names from
Unicode CLDR, and koyomi a table from the WHATWG Encoding Standard. `ritsu.wasm` also holds eight
crates from crates.io and, as every program built for WASI does, parts of wasi-libc, the C library
of WASI. `THIRD_PARTY_NOTICES` in the package says where each comes from and gives the text of its
license.
