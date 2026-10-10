# ritsu in Node

From 0.26.0, every release of ritsu also carries an npm package, `@i2y/ritsu`: the same `ritsu`,
built for WebAssembly (WASI), which Node runs. Its commands print what the native binary prints
(all but the few that start another program or reach the network, [below](#what-does-not-run-in-node)),
and a program can call them from JavaScript. It needs no native binary, has no dependencies
and runs nothing when it is installed, so it fits where a native binary is a nuisance or is not
allowed: the CI of a JavaScript or TypeScript project, a Node service that checks the repositories
it knows about, or a job that installs a package into a clone of each repository with
`npm install --ignore-scripts`, runs its command there with no network, and opens a pull request
when the command changed something.

## Install

The package is not on the npm registry; a release carries it as `i2y-ritsu-<version>.tgz`, with its
SHA-256 in the release's `SHA256SUMS`, on the
[releases page](https://github.com/i2y/ritsu/releases). Install it from there:

```console
$ npm install --save-dev https://github.com/i2y/ritsu/releases/download/v0.26.0/i2y-ritsu-0.26.0.tgz
```

Or download the file, check it against `SHA256SUMS`, and install the file itself
(`npm install --save-dev ./i2y-ritsu-0.26.0.tgz`), which works with `--ignore-scripts` and with no
network (`--offline`). It needs Node 22 or later, on Linux or macOS; 22, 24 and 26 are tested.

On Node 22, loading the package turns off V8's fast API calls for the whole process
(`v8.setFlagsFromString("--no-turbo-fast-api-calls")`): Node 22's WASI can crash the process when
it is called that way, and Node 23 and later do not. The flag makes those calls slower, not
different, but a program that loads the package on Node 22 runs under it too. On Node 23 and later
nothing is set.

## The commands

The package puts nine commands into `node_modules/.bin`: `ritsu`, and `rulec`, `dandori`, `koyomi`,
`chobo`, `geas`, `yuen`, `sakai` and `sekisho`, each the same as `ritsu <language> …`, as the links
of a release's archive are. In a project with a rule, a calendar, a dates file, a ledger and a
workflow:

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

The exit codes are the native command's: 0 nothing wrong (warnings may be printed), 1 an error, 2
bad arguments or a file that cannot be read. The standard input, output and error are the process's
own: a command reads a pipe, as `git diff | npx geas affected spec.geas -` does, and prints as it
goes.

## What `ritsu gen --format json` says

`ritsu gen --check` writes nothing and says which files of the packages are missing, stale or
changed by hand, or are left from what is generated no more. With `--format json` it says the same
to a program, which can keep, for each repository, whether its generated code is current:

```console
$ echo '// changed by hand' >> generated/typescript/index.ts
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

The first three keys are those of `ritsu check --format json`: the version, the root, and whether all
is well (the exit code is 0). Then whether it was `--check`, `--out` and the languages asked for;
`files`, each file with the language of its package (or `cedar`), its path as the text prints it,
the file of the project it is made from (null for what the package itself is made of), and its
state — `written`, `same` or `removed` when ritsu writes, and `same`, `missing`, `stale` or `left`
under `--check`. When nothing is generated, `files` is empty, `error` says why with the exit code,
and `diagnostics` holds the diagnostics of the files that do not pass their checks, each as
`ritsu check --format json` writes one. The same JSON comes without the package, from the native
binary.

## From JavaScript

```js
import { run, runSync, version, wasmPath } from "@i2y/ritsu";
```

`run(args, options?)` runs `ritsu <args>` in a worker thread and returns a promise of
`{ code, stdout, stderr }`; the event loop goes on while it runs, and any number of runs may go at
once. `runSync(args, options?)` runs it on the thread that calls it. `args` leaves out the
program's name (`["check", ".", "--format", "json"]`; a language's command is
`["rulec", "doc", "fee.rule"]`). `options.cwd` is the directory the command works in (by default
`process.cwd()`), `options.env` its environment (by default `process.env`; given, it replaces it),
and `options.stdin` what it reads from its standard input (by default nothing). `options.dirs`
narrows what the module can open to those directories, each at its own path (a relative one from
`options.cwd`; by default the whole filesystem, as the native binary has it), and `options.cwd` has
to be in one of them: a service that checks a repository it was handed can give it that repository
alone, and a path in it that leads out of it, by its name (`import proto "../../other.proto"`) or
through a symbolic link (to a file, or to a directory on the way), can then be neither read nor
written; the command says it cannot read it, and nothing of what is outside comes into a diagnostic.
It narrows what ritsu, which you trust, reads; it does not make Node's WASI a sandbox for code you
do not trust, which Node's documentation says not to rely on it for. `version` is the version of
ritsu the module was built from, and `wasmPath` the module's path. In the same project:

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

## What does not run in Node

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
`scenarios` and `explain`, and `ritsu skills install`. Each release holds the package to the native
binary on the examples of the eight languages, ritsu's test projects and the projects of the
[playground](playground.md): the same output, the same exit code and the same files written, for
every command a person runs there, in English and in Japanese.

On Windows the package stops at once, with a message: the working directory and the paths cannot
be handed to the WebAssembly module as they are. Use the native ritsu, or WSL.

## License

`(MIT OR Apache-2.0) AND Unicode-3.0 AND BSD-3-Clause`, as the native binary. Besides what the
binary holds from others, the module holds parts of wasi-libc, the C library of WASI;
[THIRD_PARTY_NOTICES](https://github.com/i2y/ritsu/blob/main/THIRD_PARTY_NOTICES), which the
package carries too, says where each comes from and gives the text of its license.
