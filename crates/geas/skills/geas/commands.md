# Commands, options, files and formats

```console
$ geas --help
geas: hold agent-written code to claims a person has read
Coding agents: `geas skill` prints the guide; `geas skill --install <dir>` installs it.

usage:
  geas check <spec.geas>...           run every claim; exit 0 when all hold
  geas snap <spec.geas>...            run them and keep every observation as the baseline
  geas drift <spec.geas>...           run them again and report what changed since the baseline
  geas map <spec.geas>...             run them with coverage on and record the lines each claim runs
  geas affected <spec.geas> <diff|->  the claims a diff touches, and the changed code no claim runs
  geas explain <code>... | --all      what a code means and how to fix it
  geas skill [--install <dir>]        the guide for coding agents, or the guide written as a skill folder

options:
  --json            the answer as JSON
  --lang ja         messages in Japanese (also GEAS_LANG=ja)
  --jobs N, -j N    run up to N claims at once (also GEAS_JOBS); default 1
  --root <dir>      map, affected: the directory the record's paths are relative to
  --out <file>      map: where to write the record
  --map <file>      affected: a record to read; give two for both sides of the diff
  --install <dir>   skill: write the skill's files to <dir>/geas
  --force           skill: write over a <dir>/geas that is already there
  --help, -h        this text
  --version         the version

exit: 0 all held, or nothing to report · 1 something failed or changed · 2 the spec, a file, or the arguments are wrong
```

## The commands

| Command | What it does | Exit |
|---|---|---|
| `geas check <spec>…` | runs every claim and reports | 0 all hold, 1 some claim failed or could not run, 2 a spec that does not parse or a file that cannot be read or written |
| `geas snap <spec>…` | runs every claim and keeps every observation as the baseline | as `check` |
| `geas drift <spec>…` | runs the claims again and compares every observation with the baseline | 0 nothing changed, 1 something changed, 2 no baseline, an unreadable one, or a claim that could not run |
| `geas map <spec>…` | runs the claims with each runtime's coverage on, and writes the record of the lines each claim ran | as `check`; 2 for E065 or E066, and then no record is written |
| `geas affected <spec> <diff>` | reads a unified diff (`-` for stdin) and the record, runs nothing, and says which claims the change touches and which changed code no claim runs | 0 nothing to read again, 1 changed code no claim runs, a deleted file whose claims are unknown, or a changed spec or baseline, 2 no record, a stale one, or a diff that cannot be read |
| `geas explain <code>…`, `geas explain --all` | when a code appears, what usually fixes it, and the smallest spec that gives it | 0, or 2 for a code geas does not have |
| `geas skill` | prints the guide for coding agents (`SKILL.md`) | 0 |
| `geas skill --install <dir>` | writes the skill folder as `<dir>/geas/`; with `--force`, over one that is there | 0; 2 when the folder is there and `--force` is not given (E081) |

`check`, `snap`, `drift` and `map` take several specs, run them one after another, and exit with
the worst status of theirs. Flags may come before or after the command. An option a command does
not take is E080, never ignored.

| Option | For | What it does |
|---|---|---|
| `--json` | every command that reports | the answer as JSON on stdout (below) |
| `--lang ja` | every command | the messages in Japanese; `en` is the default. `ok`, `not ok` and the JSON keys stay as they are |
| `--jobs N`, `-j N`, `-jN` | `check`, `snap`, `drift`, `map` | up to N claims at once; the output is the same bytes as with one |
| `--root <dir>` | `map`, `affected` | the directory the record's paths are relative to; by default the nearest directory above the spec holding `.git`, else the spec's own |
| `--out <file>` | `map` | where to write the record, instead of `.geas/<stem>.map.jsonl` |
| `--map <file>` | `affected` | a record to read instead of the spec's; give it twice for a record of each side of the diff |
| `--install <dir>`, `--force` | `skill` | where to write the skill folder, and whether to write over one |

## Environment variables

| Variable | What it does |
|---|---|
| `GEAS_LANG` | `ja` gives the messages in Japanese, as `--lang ja` does; any other value gives English |
| `GEAS_JOBS` | the default of `--jobs`; a value that is not a whole number from 1 counts as unset |
| `GEAS_CHROME` | the Chrome to start for pages; when it is set, no other is looked for |
| `GEAS_LLVM_BIN` | the directory holding `llvm-profdata` and `llvm-cov`, for Rust in `map`; else the toolchain's sysroot, then PATH |
| `GEAS_PID_LOG` | a file geas appends `start <pid> <target>` and `stop <pid>` to, for every process it starts; geas's own tests use it to see that nothing is left running |

geas sets variables of its own on the processes it starts: in `map`, `PYTHONPATH` (with
`GEAS_COVER_OUT` and `GEAS_COVER_ROOT` for its Python hook), `NODE_V8_COVERAGE`, `NODE_OPTIONS`,
`GOCOVERDIR` and `LLVM_PROFILE_FILE`, on top of what the pins give ([map.md](map.md)); for a pixie
app, `PIXIE_SCRIPT` and `PIXIE_DUMP` ([gui.md](gui.md)).

## The files geas writes

Each spec's files go in `.geas/` beside it, named after it (`greeter.geas` gives `greeter.*`):

| File | Written by | What it holds |
|---|---|---|
| `.geas/<stem>.journal.jsonl` | `check`, `snap`, `drift`, `map` | every interaction and every check of the last run, in claim order |
| `.geas/<stem>.baseline.jsonl` | `snap` | every observation, whole, for `drift` to compare with |
| `.geas/<stem>.map.jsonl` | `map` | for each claim, the lines of each file it ran, and every source file's hash |

They are derived: the journal and the record can be thrown away and made again by running. The
baseline is what drift compares against, so it is kept until the person accepts a new one. No path
in them is absolute. While it runs, geas also writes coverage, Chrome's profile and pixie's
transcripts under `.geas/`, and removes them when it is done.

## Formats

### The journal

One JSON object a line, in claim order:

- `{"claim","event":"env","target","pins"}` at a claim's first use of a target that has pins;
- `{"claim","event":"when","line","target","call","obs"}`, the observation being
  `{"stdout","stderr","exit"}`, `{"status","headers","body"}` (headers in lower case, sorted), or
  `{"screen":<node>}`;
- `{"claim","event":"check","line","check","expected","actual","ok"}`;
- `{"claim","event":"error","line","code","message"}`.

```text
{"claim":"greets by name","event":"check","line":16,"check":"status is","expected":"200","actual":"200","ok":true}
```

A masked value is written `"<masked>"`, so two runs write the same bytes where nothing changed.

### The baseline

A first line `{"geas_baseline":1,"pins":{"<target>":{…}}}` with the pins of each target that has
any, then one line per observation, `{"claim","idx","target","call","obs":{"kind":"proc"|"http"|"screen",…}}`,
the observation whole: every header, the whole body, the whole screen.

### The record

```text
{"file":"examples/greeter/server.py","blob":"4a637b4b11ea00dc76be2b17b993b8758ce9f5db","lang":"python","code":"1-4,6,9-16,18-30,32,34,36-44,46,49-50"}
{"file":"examples/greeter/server_refactored.py","blob":"3dbf08d88e9789ca4a994fa7957930de7402fefc","lang":"python","code":null}
```

A header first (`{"geas_map":1,"spec","root","claims":[{"name","status","targets"}]}`); then one
line per source file under the root, with its git blob hash, its language (`python`, `node`, `go`,
`rust`) and the lines its runtime calls code, or `null` where no runtime reported it; then one line
per claim, target and file, `{"claim","target","file","ran"}`, the lines as ranges. Paths are
relative to the root and written with `/`. One run on one tree writes the same bytes.

### A screen

A node is `{"role","name","value","states":[…],"children":[…]}` without the members that are empty;
the screen itself is a node without a role. A node a `mask screen` fits is `{"role","masked":true}`.
In text, a screen is one node a line, two spaces a level: `button "Greet" disabled`,
`textbox "Your name" value "Ada"`.

### `--json`

- `check`, `snap`, `map`: `{"geas":1,"ok","file","claims":[{"name","line","status","error",
  "checks":[{"line","check","expected","actual","ok"}],"run":[{"line","call","observed"}]}]}`.
  `status` is `ok`, `fail` or `error`; `error` is null or `{"code","line","col","message","notes"}`;
  `run`, the run that gets there, comes only with a claim that is not ok. A failed `screen` check
  carries `screen`, the screen it saw. `map` adds `"map":{"record","files","code","ran"}` and
  `"diagnostics"`. Several specs print one object a line.
- A spec that does not parse: `{"geas":1,"ok":false,"file","diagnostics":[…]}`, each diagnostic
  `{"code","severity","file","line","col","message","notes","path"}`.
- `drift`: `{"geas":1,"file","compared","drifted","unclaimed","claimed","changes":[{"claim","when",
  "line","call","field","old","new","claimed"}],"notes","diagnostics"}`.
- `affected`: `{"geas":1,"spec","diff","records":[{"file","side"}],"claims":[{"index","name",
  "status","lines":[{"file","side","lines","how","startup"}]}],"unclaimed":[{"file","side","lines",
  "why"}],"deleted":[{"file","known"}],"outside","spec_changed","baseline_changed","ok"}`. A line's
  `side` is `after` (numbered in the code after the change) or `before` (a removed line); `how` is
  `ran` or `near`; `startup` names the target every claim of which runs the line.
- `explain`: one object a line, `{"geas":1,"code","severity","summary","when","fix","repro":{"args",
  "files":[{"name","text"}],"exit","needs","env"}}`.
