# From a diff to the claims: `geas map` and `geas affected`

`geas drift` reads the run: what a person could observe changed where no claim promises anything.
`geas map` and `geas affected` read the diff: which claims this change touches, and which changed
code no claim runs. A diff names files and lines and a claim names neither, so the two meet through
a record of, for each claim, the lines of the implementation its run executed.

```text
geas map <spec.geas> [--root <dir>] [--out <file>]
geas affected <spec.geas> <diff | -> [--map <file>]... [--root <dir>]
```

`map` is `check` with each runtime's coverage switched on: it runs every claim, prints the same
report, and writes the record to `.geas/<stem>.map.jsonl`. `affected` reads a unified diff (`git
diff`, or `diff -u`) and the record, runs nothing, and says what the change touches.

Nothing in a claim changes for this. The record is measured by each language's own coverage
machinery, switched on from outside; geas never edits, wraps or rebuilds the implementation. The
record never decides a verdict, either: `check` and `drift` do not read it. It decides where a
reviewer looks, and `check` never runs fewer claims because of it.

## On the greeter

An agent changes the greeter's `server.py`: it strips the name before checking it, adds a `/health`
route, and drops the handler's `log_message` override. `map` runs the claims on the changed code,
and `affected` reads `git diff`:

```console
$ geas map examples/greeter/greeter.geas
ok 1 - greets by name
ok 2 - rejects an empty name
ok 3 - totals accumulate across requests
ok 4 - unknown paths are 404
4 claims · 4 ok · 0 failed · journal: examples/greeter/.geas/greeter.journal.jsonl
map: 2 files · 40 lines of code, 38 run by some claim → examples/greeter/.geas/greeter.map.jsonl
$ git diff | geas affected examples/greeter/greeter.geas -
diff: <stdin> · record: examples/greeter/.geas/greeter.map.jsonl (the code after the change)
claims the change touches:
  1 - greets by name
      examples/greeter/server.py: 21
  2 - rejects an empty name
      examples/greeter/server.py: 21
  4 - unknown paths are 404
      examples/greeter/server.py: 29
  every claim that starts `api`: 1, 2, 3, 4
      examples/greeter/server.py: removed 10-11 (next to lines they run)
changed code no claim runs:
  examples/greeter/server.py: 30
4 claims · 4 touched · 1 changed line no claim runs
$ echo $?
1
```

- Line 21, the name stripped, is run by the two claims that send a name: read them again.
- Line 29 is the new `elif` for `/health`. Claim 4 asks for `/nope`, which runs that condition
  on its way to the `else`.
- Line 30, the answer to `/health`, is run by no claim: the agent has added behavior nobody
  promised. Propose a claim (`when api.get("/health")`, `then status is 200`), or say why none is
  needed.
- The removed lines 10-11, the `log_message` override, sat between lines every claim runs when it
  starts the service: the class and its `def` lines, which Python runs on import. With only a
  record of the code after the change, a removal is placed by the code around it ("next to lines
  they run").

## What each language needs

geas sets every switch below for every process it starts during `map`, and each runtime picks up its
own. A target does not say what it is written in, and a Python service that runs a Go command is
recorded in both.

| Language | geas sets | The target has to | Read with |
|---|---|---|---|
| Python 3.12 and later | `PYTHONPATH` gains a directory under `.geas/` holding geas's `sitecustomize.py` | nothing | the JSON that file writes |
| Node: JavaScript, and TypeScript by Node's type stripping | `NODE_V8_COVERAGE`; `NODE_OPTIONS` gains `--require` of geas's hook | nothing | V8's coverage JSON |
| Go | `GOCOVERDIR` | be built with `go build -cover -coverpkg=./...`; as a service, return from `main` on SIGTERM | `go tool covdata textfmt` |
| Rust | `LLVM_PROFILE_FILE` | be built with `-C instrument-coverage`, and be the program the command starts; as a service, exit normally on SIGTERM | `llvm-profdata` and `llvm-cov` from rustup's `llvm-tools` |

- **Python.** The hook uses `sys.monitoring` and runs the project's own `sitecustomize`, if any,
  after it. It writes when the program exits and on SIGTERM. Before 3.12 there is no
  `sys.monitoring`, and `map` warns (W060).
- **Node.** V8 writes its coverage when the process exits normally; geas's hook turns a SIGTERM
  into a normal exit when the program has no SIGTERM handler of its own. Node blanks TypeScript's
  types instead of moving the code, so the lines are the `.ts` file's own.
- **Go.** The counters are written when the program exits normally. A service killed by SIGTERM
  leaves only its metadata, which is E066, so stop on SIGTERM (`signal.NotifyContext` and
  `Server.Shutdown` do). Import paths are mapped to files through the `go.mod` files under the
  root. Built with `-cover`, a program prints `warning: GOCOVERDIR not set, no coverage data
  emitted` when it runs outside `map`, so keep that build for `map`.
- **Rust.** The profile is written by `std::process::exit` and a normal return, not by a signal,
  and the standard library cannot catch SIGTERM, so a Rust service needs code of its own to stop on
  it. `llvm-cov` needs the instrumented binary, which is why the target's command has to start it
  itself: a profile from a program started through a script or `cargo run` is left out (W061). The
  tools are looked for in `GEAS_LLVM_BIN`, then the toolchain's sysroot, then PATH (E065 without
  them). Outside `map`, an instrumented program writes `default_*.profraw` into its working
  directory.

In `map`, a service is stopped with SIGTERM and given 5 s; one that has to be killed after that may
not have written its record, and the claim's lines cannot be trusted: E066. A program the switches
do not reach (a shell script, a binary built without coverage, Python before 3.12) adds nothing,
and a target that gave no record at all is W060.

## The record

`.geas/<stem>.map.jsonl` holds every source file under the root (`.py`; `.js .mjs .cjs .ts .mts
.cts`; `.go`; `.rs`), loaded or not, with its git blob hash, and, for each claim and target, the
lines of each file it ran ([commands.md](commands.md) has the format). The root is the nearest
directory above the spec holding `.git`, else the spec's own directory; `--root` overrides it.
Paths with a component `.geas`, `.git`, `node_modules`, `site-packages`, `__pycache__` or `target`
are left out. One run on one tree writes the same bytes, so a record can be cached or committed;
nobody audits it.

A record is exactly the code it was measured on, by hash. `affected` holds it to the diff file by
file and refuses one that does not fit (E062): a file the diff does not touch must have the
recorded hash on disk, and a file it touches must have the hash of one side of the diff in the
record. A git diff names both sides in its `index` lines; a plain `diff -u` is held to the file on
disk. `affected` never answers "no claim touched" from a record of other code; after changing the
code, `map` again.

## What a change touches

- An added or rewritten line needs a record of the code after the change (E063 otherwise). There
  it is run by some claims, which it touches; or it is code no claim runs; or it is not code (a
  blank, a comment), which counts for nothing.
- A removed line is looked up in a record of the code before the change when one is given:
  `--map` can be given twice, one record of each side. With only a record of the code after, a run
  of removed lines touches the claims that run the code just above and below where it was, marked
  `(next to lines they run)`.
- A line every claim that starts a target runs, when two claims or more start it, is shown once,
  under `every claim that starts` that target: imports, route tables, the code every request passes.
- A deleted file is listed apart; without a record of the code before, which claims ran it is not
  known, and that makes the exit status 1.
- A source file no runtime reported is run by no claim as a whole.
- A file git does not track is not in `git diff`, so a file the change adds is read only once it
  is in the diff: `git add -N <file>` first. A record of the code before the change notices such a
  file, which the diff does not explain (E062); a record of the code after it does not.
- Other files (documents, configuration, data) are listed as outside the record and do not change
  the exit status.
- A change to the spec itself, or to its baseline, is listed apart and makes the exit status 1:
  the claims changed, or drift now compares against something new, and either needs a person.

Exit 0 when no changed code is unclaimed, no deleted file's claims are unknown, and neither the
spec nor its baseline changed; 1 otherwise; 2 when a record is missing or stale, or the diff cannot
be read.

## Limits

- Lines are the unit. A line holding two branches counts as run when either ran. Node decides a line
  by its first character, so `} else if (…) {` counts as run when the block its `}` closes ran.
- Python and Node report only the files a run loaded; a file nothing loaded is known by its
  extension, not by its lines.
- Templates, SQL, configuration and generated files are outside the record, and so are pixie apps
  and the scripts of a page in the browser.

## CI recipes

The baseline is what drift compares against, so commit it; the journal and the record are made
again on every run. In the project's `.gitignore`:

```text
**/.geas/*
!**/.geas/*.baseline.jsonl
```

On a pull request, with the base branch fetched as `origin/main`:

```sh
geas check claims.geas
geas drift claims.geas
geas map claims.geas
git diff origin/main...HEAD | geas affected claims.geas -
```

To place removed lines exactly, record the base too, and hand `affected` both records:

```sh
git checkout origin/main
geas map claims.geas --out before.map.jsonl
git checkout -
geas map claims.geas
git diff origin/main...HEAD | geas affected claims.geas - --map before.map.jsonl --map .geas/claims.map.jsonl
```

`affected` exits 1 whenever there is something for a person to read, which is most changes. A CI
step that should report rather than fail lets that status through and posts the report, or the
`--json` of it.
