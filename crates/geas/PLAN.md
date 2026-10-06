# geas v1: the plan of work

This is the order in which v1 is built, for whoever builds it. DESIGN.md
§6–§16 say what v1 is and why; this file says in what order to build it,
what each step touches, and which tests have to pass before the next
stage starts. Where this plan and DESIGN.md disagree, DESIGN.md wins:
build what it says, and report the disagreement.

There are three stages, each meant for one implementer, with a review
between them:

- **B. Diff to claims**, after a test scaffold and the diagnostics every
  later step reports through (DESIGN §7, §13).
- **C. GUI targets, matchers, parallel runs, quoting, pins** (DESIGN §8–§12).
- **D. The skill, the examples, README.md and README.ja.md** (DESIGN §14).

Each stage ends with a list headed "done when". A stage is finished when
every item on it holds, and not before.

## Ground rules (every stage)

1. Rust edition 2024, built and tested with the stable toolchain on this
   machine (rustc 1.94.1). `[dependencies]` and `[dev-dependencies]` stay
   empty.
2. `cargo build` and `cargo test` print no warnings. An `#[allow]` carries
   a comment saying why.
3. Write only inside this repository. Do not write in, build in, or run
   git in any other tree (pixie, rulec, dandori or any other); reading
   them is fine. Do not commit: the coordinator commits.
4. Every process you or a test starts is stopped before the test returns:
   services, Chrome, Python servers. B0 gives the tests a helper that
   checks it.
5. Temporary files: tests work under `env!("CARGO_TARGET_TMPDIR")` and
   remove what they made; experiments outside the tests go in the scratch
   directory the coordinator names. Leave nothing large behind: Go builds
   always take `-trimpath`, and Rust fixtures are compiled with `rustc`
   into a test's temporary directory, never with cargo into a new target
   directory. The disk has about 48 GB free.
6. Nothing committed may hold an absolute path, a user name or a machine
   name: not a golden, a record, a fixture or a document. Tests run geas
   with a temporary directory as the working directory and relative paths,
   and pass `--root` to `map` and `affected` (`CARGO_TARGET_TMPDIR` lies
   inside this repository, so the search for `.git` would find the
   repository's own).
7. A test that lacks an outside tool (python3, node, go, rustc with
   llvm-tools, Chrome, a built pixie app) prints `SKIP: <reason>` on
   stderr and passes. Before reporting, run
   `cargo test -- --nocapture 2>&1 | grep '^SKIP'`, and report how many
   there were and why.
8. Golden files live in `tests/golden/`, English under `en/`, Japanese
   under `ja/`, JSON beside them. `GEAS_BLESS=1 cargo test` rewrites them.
   Read the difference before blessing; never bless a change you cannot
   explain.
9. Messages are English by default and Japanese with `--lang ja`; each is
   written once, in both languages, in the code table (B1). Japanese
   follows Appendix B.
10. Any output or number written into README.md, README.ja.md, the skill
    or DESIGN.md comes from a run: a golden file or a command you ran.
    Stage D's docs test enforces it for the READMEs and the skill.
11. When building forces a design change, change the DESIGN.md section
    too, and say so in the report.

The report at the end of a stage gives: the files made and changed; the
decisions taken, with reasons (and a mark on the ones the author might
have wanted to take); the test results (counts, SKIP lines and their
reasons, anything that failed and was run again); what was left, and why;
notes for the next stage; and questions for the coordinator.

## Where things go

The spike has seven modules. After v1, roughly (split further where a
module gets hard to read; keep the spike's style of small functions and
comments that say why):

```
src/main.rs        commands, flags, exit codes, --help, --version
src/diag.rs        Lang, Diag, rendering as text and JSON (dandori's shape)
src/codes.rs       the code table: summaries, explanations, repros, in en and ja
src/report.rs      the report of a run: TAP lines, failed checks with their run,
                   claim errors, summaries, the JSON of check and snap
src/lex.rs         tokens
src/parse.rs       DESIGN §6's grammar into the model; static checks E001-E013
src/model.rs       Spec, Target, Claim, Step, Call, Check, Subject, Matcher,
                   Value, node patterns, Pin
src/words.rs       command strings into words (§11), {port}
src/run.rs         one claim: its whens, its checks, its journal events
src/proc.rs        the process and service adapters: env, timeouts, ports,
                   SIGTERM then SIGKILL
src/http.rs        the HTTP client, moved out of run.rs
src/check.rs       evaluating every matcher
src/pins.rs        reading pinned values: an RFC 3339 time, a locale
src/regex.rs       patterns: parser and Pike VM
src/json.rs        the spike's JSON, plus what writing needs
src/drift.rs       snap and drift: headers, screens, pins, --json
src/sched.rs       --jobs: workers, port locks, serial, claim order
src/hash.rs        SHA-1, git blob hash, base64
src/lines.rs       line sets as ranges
src/tree.rs        the root, the source files under it, runtimes by extension
src/cover/         the coverage switches and readers: mod.rs, python.rs,
                   node.rs, go.rs, rust.rs, and the two hooks
                   (sitecustomize.py, geas_cover.cjs) taken in with include_str!;
                   the readers' tests read recorded runs under tests/cover/
src/map.rs         geas map; writing and reading the record
src/diff.rs        unified diffs; the two sides' hashes; undoing hunks
src/affected.rs    geas affected
src/screen.rs      the screen tree: roles, text and JSON, patterns, tree diff
src/gui.rs         GUI actions: live and replayed drivers
src/driver.rs      the driver protocol (§8.5)
src/pixie.rs       the pixie driver (§8.3)
src/ws.rs          the WebSocket client
src/cdp.rs         Chrome: finding, starting, sessions, the browser driver
src/skill.rs       geas skill (the skills/geas files, taken in)
```

Tests are integration tests that run the built binary
(`env!("CARGO_BIN_EXE_geas")`), plus `#[cfg(test)]` modules for the pure
parts (lexer, parser, JSON, hashes, line sets, diffs, patterns, words,
screen trees, coverage readers).

---

## Stage B: diff to claims

### B0. A test scaffold, before anything changes

The spike has no tests. Pin down what it does now, so the rest of B can
change it on purpose.

- `tests/common/mod.rs`:
  - `geas()`: the binary's path.
  - `Scratch`: a fresh directory under `CARGO_TARGET_TMPDIR`, removed when
    dropped.
  - `copy_example(name, scratch)`: copies `examples/<name>` to
    `<scratch>/examples/<name>`, so that geas, run with the scratch as its
    working directory, prints `examples/<name>/…` as the README does.
  - `run(cwd, args, env) -> (stdout, stderr, code)`: kills geas after 120 s
    so that a hang fails the test instead of the suite.
  - `golden(name, actual)`: compares with `tests/golden/<name>`; with
    `GEAS_BLESS` set it writes the file instead.
  - `have(tool, args) -> bool` and `skip(reason)`.
  - `port_lock()`: one process-wide mutex for the tests that use a fixed
    port (the greeter's 8123). Test binaries run one after another, so a
    lock per binary is enough.
  - `port_closed(port)`: after a run, nothing answers on the port. This is
    how B0 checks that the greeter's service was stopped; B1 gives a
    better check (`GEAS_PID_LOG`).
- `tests/check.rs`: `calc` and `greeter` hold (report goldens); greeter
  with `--json`; the journal of each (byte for byte); a greeter whose
  `server.py` lost the empty-name check (written into the scratch copy) is
  `not ok 2`, exit 1; a `serve` command that exits at once is an error,
  exit 1.
- `tests/drift.rs`: snap then drift on the unchanged greeter is quiet,
  exit 0; with `server_refactored.py` copied over `server.py`, the seven
  unclaimed changes the README shows, exit 1; with `/total` off by one, a
  claimed change, exit 1; drift with no baseline, exit 2.
- `tests/spec.rs`: every message the spike's parser can give, one spec
  each under `tests/specs/v0/`, stderr golden, exit 2.
- Unit tests in `lex.rs`, `parse.rs`, `json.rs` (escapes, surrogate pairs,
  numbers, `path_get`).
- Tests that run the Python examples print SKIP without python3.

Done when `cargo test` passes and every golden is what the spike prints
today. No source file outside `tests/` and the `#[cfg(test)]` modules
changes in B0.

### B1. Diagnostics, codes, Japanese, the command line

- `diag.rs` after dandori's `src/diag.rs`: `Lang::pick(flag)` (`--lang`,
  then `GEAS_LANG`, then English); `Diag { code, severity, line, col, en,
  ja, notes, path }`. As text: `error[E004]: <file>:<line>:<col>: <message>`
  (`エラー[E004]: …` in Japanese, `warning`/`警告` for W codes), then
  `  <n> | <the spec's line>`, then each note as `  = <note>`, then
  `  the run that gets there:` (`  ここまでの実行:`) and one step a line,
  `    <line>  <step>`. As JSON: `code`, `severity`, `file`, `line`, `col`,
  `message`, `notes`, `path` (each step `{line, step}`). `file` is the file
  the line is in (the baseline's for E051), `null` for the command line; a
  diagnostic without a line (E050, E080, E081) has `line` 0 and its place is
  printed as `<file>: ` or not at all. A control character in a quoted line
  is shown as U+FFFD.
- `codes.rs`: DESIGN §13's table. Each code has a one-line summary, an
  explanation (when it appears, what usually fixes it), and a repro (a
  minimal spec, or a command), in English and Japanese.
  `geas explain <code>` prints one; `geas explain --all` prints all, in
  code order. Codes that later steps bring (E009-E013, E034-E037,
  E060-E066, W060-W061) are added by the step that brings them.
- Every error the spike prints becomes a code: the parser's (E001, E002,
  E004, E005, E006), the runner's (E030 a program that would not start,
  E031 a `run` over 5 s, E032 a service that exited or never opened its
  port, E033 an HTTP exchange that failed), the baseline's (E050, E051),
  the command line's (E080, E081). New static checks: E003 (a target or
  claim name used twice), E007 (a subject the `when` does not observe,
  found before running, where the spike reported a failed check), E008
  (the spike's three matcher rules, coded).
- A syntax error (E001) stops the parse at the first; once a spec parses,
  every static check runs and every error is reported, in line order. (As
  built: an unknown word (E002) and the checks the parser makes as it goes
  (E005, E008) do not stop it; it skips the rest of that line, up to a `}`,
  and goes on, so what it found before the first E001 is reported with it.
  A target with a line E002 refused does not also get E004 for a line it
  seems to lack.)
- A failed check keeps the spike's line first, since it is short and an
  agent can grep it, then adds the spec line and the run that gets there:

  ```text
  not ok 2 - rejects an empty name
      examples/greeter/greeter.geas:21: status is 400 — got 200
          21 |   then status is 400
        the run that gets there:
            20  when api.get("/greet?name=")  →  200, body "{\"message\": \"Hello, \"}"
  ```

  (As built: `tests/golden/en/check/greeter-no-empty-name-check.txt`. The
  block is dandori's layout indented four spaces under the `not ok` line,
  the same as a claim error's diagnostic; the run is given once per claim,
  up to its last failed check.) An observation in a step is cut
  to one line of at most 100 characters, ending in `…` when cut: for a
  process `exit 1, stdout "…", stderr "…"`, for HTTP `200, body "…"`. In
  Japanese the check line reads `status is 400 のはずが、実際は 200` and the
  heading `ここまでの実行:`.
- A claim error: `not ok <n> - <name> (error)`, then the diagnostic, with
  the command's words and the last lines of its stderr as notes. (As built:
  the diagnostic is indented four spaces under the `not ok` line; the words
  are printed as a shell would read them back; for E033 the notes are the
  last lines of the service's stderr, taken once the claim has stopped it;
  in Japanese the line ends `（エラー）`.)
- The summary: `4 claims · 3 ok · 1 failed · journal: <path>` as now;
  Japanese `主張 4 件 · 成り立った 3 件 · 成り立たなかった 1 件 · ジャーナル: <path>`.
- `--json`: the spike's object gains `"geas": 1`, an error object with
  `code` (and `col` and `notes`), and, for a claim that is not ok, `run`
  (Appendix A). A spec that
  does not parse prints `{"geas":1,"ok":false,"file":…,"diagnostics":[…]}`
  on stdout and exits 2.
- The command line: `geas <command> <spec>… [flags]`; `--help`/`-h` and
  `--version`; Appendix A has the help text. A flag a command does not take,
  or a missing argument, is E080 (exit 2); a file that cannot be read or
  written is E081 (exit 2). `check`, `snap`, `drift` take several specs.
  (As built: flags may come before the command; `--lang` takes `en` or `ja`
  and anything else is E080, said in English or as `GEAS_LANG` asks, since
  the language is what is wrong; `GEAS_LANG` with another value is English.
  `--help` prints the help in the language asked for. `explain` takes one
  or more codes, in either case, or `--all`, and `--json`, which gives each
  entry with its repro as files and arguments; `tests/explain.rs` runs them
  from there.)
- `.geas/` files named after the spec (DESIGN §6).
- `drift --json` (Appendix A). Drift's tags in Japanese: `[主張なし]` and
  `[主張あり — 判定は geas check]`. (As built: a `when` is shown as the spec
  writes it, `api.get("/total")`, target included; a claim that cannot run
  during drift is printed as its diagnostic, and drift exits 2.)
- A test aid: when `GEAS_PID_LOG` names a file, geas appends
  `start <pid> <target>` for every process it starts and `stop <pid>` once
  it has reaped it. It changes nothing else. `tests/common` gains
  `no_process_left(log)`: every started pid was stopped, and none is alive
  (`kill(pid, 0)` fails). Every test that starts a service, a driver or
  Chrome uses it from here on. (A service's command line does not hold the
  test's directory, so `pgrep` cannot find it.)

Tests:

- `tests/spec.rs`: at least one spec per cause of each code E001-E008
  under `tests/specs/E0nn-<what>.geas`; stderr golden in English and in
  Japanese, and the `--json` golden.
- `tests/errors.rs`: E030 (a program that does not exist), E031
  (`run "sleep 10"`, which costs 5 s), E032 (a service that exits at once,
  and one that never opens its port, 5 s), E033 (a service that accepts
  and closes without answering), E050, E051, E080, E081: English and
  Japanese goldens.
- `tests/explain.rs`: `explain --all` in both languages (goldens), and
  every repro in the table run and checked to give its own code, which
  keeps the table honest.
- `tests/check.rs` and `tests/drift.rs` re-blessed for the new shapes,
  plus Japanese goldens of a failing claim and of a drift report.
- `tests/cli.rs`: `--help` golden; `--version`; E080; two specs in one
  directory keep two journals.

### B2. Hashes, line sets, the root

- `hash.rs`: SHA-1; the git blob hash (`blob <size>\0<bytes>`); hex;
  base64 (encoding only).
- `lines.rs`: a set of line numbers to and from ranges (`1-4,6,9-11`).
- The root (DESIGN §7.4): the nearest directory above the spec holding a
  `.git` directory or file; `--root` overrides it.
- Unit tests: SHA-1 of the empty string, of `abc`
  (`a9993e364706816aba3e25717850c26c9cd0d89d`), and of a million `a`s
  (`34aa973cd4c4daa4f61eeb2bdbad27316534016f`); the blob hash of
  `hello\n` (`ce013625030ba8dba906f756967f9e9ca394464a`); base64 of RFC
  4648's vectors (`""`, `f`, `fo`, `foo`, `foob`, `fooba`, `foobar`);
  ranges both ways; the root with a `.git` directory, with a `.git` file,
  and with neither.

### B3. Coverage switches and readers

- `cover/mod.rs`: in `map`, every process the run starts for a target
  (`run`, `serve`; not a driver, not Chrome, not a pixie app) gets the
  switches of DESIGN §7.2, pointing into a directory of its own under its
  claim's, `.geas/cover/<n>/<k>/` (as built: one directory per process, so
  that what each target ran is known, W060 is per target, and a Rust
  profile is matched to the program by the pid of the process geas
  started, DESIGN §7.2, §7.3):
  - `PYTHONPATH`: the hook directory first, then whatever was there;
    `GEAS_COVER_OUT` the claim's directory; `GEAS_COVER_ROOT` the root;
  - `NODE_V8_COVERAGE` the claim's directory; `NODE_OPTIONS` the caller's,
    then `--require "<hook>"` (quoted, for a path with spaces);
  - `GOCOVERDIR` the claim's directory (it has to exist before the start);
  - `LLVM_PROFILE_FILE` `<claim dir>/rust-%p-%m.profraw`.
  The hooks are written to `.geas/hook/` when `map` starts and removed when
  it ends, even on failure.
- `cover/sitecustomize.py`: start from the measured probe in Appendix C,
  then: leave out paths with an excluded component (DESIGN §7.4) and
  anything under the hook directory; drop line 0; write
  `python-<pid>.json` as `{"files":{"<abs path>":{"code":[…],"ran":[…]}}}`;
  write at exit and on SIGTERM (handler: write, then `os._exit(143)`); then
  find the project's own `sitecustomize` on `sys.path` without the hook
  directory (`importlib.machinery.PathFinder.find_spec`) and run it.
- `cover/geas_cover.cjs`: Appendix C's hook, as measured.
- Readers, each giving `path relative to root -> (code lines or none, run
  lines)`, and taking the raw text and a way to read a source file, so
  that unit tests can feed them recorded inputs:
  - `python.rs`: the JSON above.
  - `node.rs`: every `coverage-*.json` in the directory; `result[].url`
    `file://…` (percent-decoded) under the root; for each non-blank line,
    the UTF-16 offset of its first non-blank character; the innermost
    range holding it (the shortest; on a tie, the later one); run when its
    count is above zero. Code is every non-blank line. A line run in any
    of the files is run.
  - `go.rs`: when `covmeta.*` is there, `go tool covdata textfmt
    -i=<dir> -o=<dir>/go.txt`; each block `<import path>:<l1>.<c1>,<l2>.<c2>
    <n> <count>` makes lines l1..l2 code, and run when the count is above
    zero; import paths become files through the `module` line of every
    `go.mod` under the root (the longest module path that is a prefix
    wins). Metadata without counters is E066. No `go` on PATH, or the tool
    failing, is E065.
  - `rust.rs`: when `*.profraw` is there, `llvm-profdata merge -sparse`
    into one profile, then `llvm-cov export -format=lcov
    -instr-profile=<profile> <first object> [-object <next>]…`. The
    objects are the programs the targets' commands start (first word,
    resolved against the spec's directory, then PATH) whose first bytes
    are a Mach-O (`CF FA ED FE`) or ELF (`7F 45 4C 46`) header. `SF:` paths
    are made relative to the root (others dropped); `DA:<line>,<count>`
    gives code and run. The tools come from `GEAS_LLVM_BIN` when it is set
    (and then only from there), else from
    `<rustc --print sysroot>/lib/rustlib/<host>/bin` (host from
    `rustc -vV`), else from PATH; missing or failing is E065. A profile none
    of the objects matches is W061. (As built: a profile is matched by the
    pid in its name to the process geas started, whose program has to hold
    `__llvm_covmap`, or by its signature to one so matched; `llvm-cov`
    refuses a whole export when handed a program without coverage data.)
- `proc.rs`: in `map`, a service is stopped with SIGTERM (an
  `unsafe extern "C" { fn kill(pid: i32, sig: i32) -> i32; }` block,
  SIGTERM = 15), waited for up to 5 s, then killed; killed is E066.
  `check`, `snap` and `drift` keep killing at once.
- The readers' tests: recorded raw output of one small program per
  language under `tests/cover/<lang>/`, with the sources, and the expected
  lines as goldens. Record them once (with the commands of Appendix C) and
  replace the absolute directory in them with `/ROOT` before committing;
  the tests read sources through the injected reader with `/ROOT` mapped to
  the fixture directory. These tests need no toolchain. (As built: they are
  `#[cfg(test)]` tests in `src/cover/*.rs`, since geas is a binary crate and
  an integration test cannot call a reader; their goldens are in
  `tests/golden/cover/`.)

### B4. `geas map`

- `geas map <spec>… [--root <dir>] [--out <file>] [--json]`: runs every
  claim as `check` does, with the switches on and the report printed the
  same way; after each claim, reads its directory into the claim's slice
  and removes it; at the end, walks the root for source files (by
  extension, leaving out excluded components), hashes each, and writes the
  record (Appendix A), sorted.
- After the report: `map: <claims> claims · <files> files · <code> lines of
  code, <ran> run by some claim → <record path>` (Japanese in Appendix B),
  then the warnings W060 (a target that gave no record at all) and W061.
  (As built: `check`'s summary line comes first, so the map line leaves the
  claims out: `map: 1 files · 16 lines of code, 14 run by some claim →
  examples/calc/.geas/calc.map.jsonl`; the diagnostics go to stderr.)
- Exit status as `check` (0 or 1), 2 for E065 or E066, and then no record
  is written.
- `tests/map.rs`: `examples/calc` (report and record goldens, `--root` the
  scratch copy); `examples/greeter` (a service stopped by SIGTERM; record
  golden); W060 (`run "sh -c 'echo 5'"`); E066 (a Python service that
  sets SIGTERM to be ignored, killed after 5 s); E065 (a small Python
  fixture writes a file named as `LLVM_PROFILE_FILE` asks, with
  `GEAS_LLVM_BIN` naming an empty directory). (As built: W060 runs
  `run "echo 5"`, since command strings split on blanks only until C1; W061
  is a script that builds and starts a Rust program; the root is found
  through a `.git` directory and a `.git` file; `--out`, several specs,
  `--json` and one run's bytes against a second's are tested too.)

### B5. Diffs and `geas affected`

- `diff.rs`: unified diffs from `git diff` (headers `diff --git`, `index
  <a>..<b> [mode]`, `new file mode`, `deleted file mode`, `rename from`
  and `rename to`, `similarity index`, `Binary files … differ`) and from
  `diff -u` (`--- path<TAB>date`); hunks with `\ No newline at end of
  file`. Git diffs lose their `a/` and `b/`; paths are relative to the
  root.
- The two sides' hashes, per file (DESIGN §7.5): from `index` lines (match
  by prefix), else from the disk: if the disk fits the after side, hash it
  and undo the hunks for the before side; if it fits the before side, the
  other way round; else E064.
- `affected.rs`: DESIGN §7.5 and §7.6, with the report of Appendix A.
  `-` reads the diff from stdin; `--map` can be given twice; each record is
  matched to a side per file by its hashes.
- `tests/affected.rs`, on small hand-written records and diffs under
  `tests/affected/<case>/` (no toolchain needed), one case each: exact
  touches on the after side; unclaimed lines; a file no runtime reported;
  a document outside the record; the spec changed; the baseline changed;
  an untouched file changed on disk (E062); only a before-side record and
  added lines (E063); a plain diff with the disk at the after side; with
  the disk at the before side; with the disk at neither (E064); a removal
  attributed "near"; two records, the removal exact; an added file; a
  deleted file; a rename; a binary file; startup code ("every claim that
  starts …"); the diff from stdin; `--json`; no record (E060); a record of
  another spec (E061). English goldens for every case, Japanese for a full
  report and for E060-E064. (As built: the cases share one fixture, a shop
  before and after one change, under `tests/affected/before/` and
  `after/`, with a record of each made once by `geas map` and the diffs
  made from variants of the two trees with `git diff` and `diff -u`.)

### B6. The four languages, end to end

- `examples/tally-node`: a small service in TypeScript (`node:http`), run
  as `node server.ts <port>`, fixed port 8124 until C brings `port auto`;
  a claims file of four or five claims.
- `examples/tally-go`: the same service in Go, stopping on SIGTERM through
  `signal.NotifyContext` and `Server.Shutdown`, fixed port 8125; the test
  builds it with `go build -cover -coverpkg=./... -trimpath -o tally .`
  inside the scratch copy (the binary is ignored by git).
- `examples/tally-rust`: a command line in Rust (`tally add 5 7` prints
  `12`; bad input exits 2 with a message), built in the test with
  `rustc --edition 2024 -C instrument-coverage -o tally src/main.rs`.
- `examples/calc` and `examples/greeter` are the Python ones.
- Each of the five gets an agent-style change under
  `tests/changes/<example>/` (not inside the example, where `map` would
  record the change's own files as sources): the changed files
  (`after/`), the change as a git diff (`change.diff`, with `index` lines,
  made once with `git diff --no-index` in a scratch directory) and as a
  plain one (`change.plain.diff`, `diff -u`). The change touches some
  claims, adds a line no claim runs, and removes a line, so each part of
  the report shows. The test checks that the `index` hashes are geas's
  blob hashes of the before and after files.
- `tests/languages.rs`: per example: copy it, build it when needed,
  `geas map --root .` (report and record goldens), copy `after/` in,
  `geas affected <spec> <the change.diff>` and the plain one (goldens).
  SKIP per missing tool: python3; node; go; rustc and llvm-tools. (As
  built: the change adds lines, which need a record of the code after it
  (E063 otherwise), so the test maps again after copying `after/` in and
  rebuilding, keeps the first record as `before.map.jsonl`, and runs
  `affected` with the record of the code after the change and with both;
  the Go build cache is a directory in the scratch, removed with it.)

### Stage B is done when

- `cargo build` prints no warning, and `cargo test` passes.
- On this machine (python3, node, go, rustc with llvm-tools present),
  `cargo test -- --nocapture 2>&1 | grep -c '^SKIP'` prints 0.
- Every code from E001 to E008, E030 to E033, E050, E051, E060 to E066,
  E080, E081 and W060, W061 has a repro that gives it (`tests/explain.rs`)
  and goldens in English and Japanese.
- `geas map` and `geas affected` give the goldens on all five examples,
  in both diff forms.
- Nothing is left in `CARGO_TARGET_TMPDIR` after the run, and no process a
  test started is alive (`no_process_left`).
- The report says how long `cargo test` took.

---

## Stage C: GUI targets, matchers, parallel runs, quoting, pins

### C1. Command strings

- `words.rs`: DESIGN §11. A command becomes words once, at parse time;
  E010 for an open quote or a trailing backslash, pointing at the column
  inside the string. `{port}` is replaced in each word after splitting,
  and is E010 on a target without a port.
- Unit tests: a table of strings and their words (blanks, both quotes,
  backslashes in and out of quotes, empty words `''`, `{port}` inside a
  word).
- `tests/words.rs`: a spec runs `python3 'my calc.py'` (a file name with
  a space) and holds; E010 goldens in both languages.
- (As built: blanks are white space, as v0's `split_whitespace`; a command
  with no word is E010 at the string, where B left it to the run as E030;
  string tokens carry the column of each character, so E010 points inside
  the string. The E010 goldens come from `tests/specs/E010-*.geas`, which
  `tests/spec.rs` runs in both languages and as JSON, as for the other
  static codes; `tests/words.rs` runs the file name with a space written
  three ways, every kind of word, a named shell, and `{port}` in a
  service's command.)

### C2. Matchers and the header subject

- The model gains the matchers and values of DESIGN §9 and the subjects
  `header "<name>"` and `screen` (the screen's checks come in C5);
  `check.rs` evaluates them; E008 grows to every rule of §9.
- `regex.rs`: the pattern language of §9, parsed into a program and run as
  a Pike VM over the whole value. E009 for a pattern that does not parse,
  with the column.
- Drift: a header's change is claimed when a check of that `when` names
  the header.
- `tests/matchers.rs`: two fixtures, `tests/impl/echo.py` (prints given
  stdout and stderr and exits with a given code) and `tests/impl/answer.py`
  (a service answering fixed JSON with fixed headers), and a spec with one
  claim that holds and one that fails for every pair of matcher and subject
  kind in the table of §9; report goldens in both languages; E008 and E009
  goldens; a drift run where a header change is claimed.
- Unit tests for `regex.rs`, including a pattern such as `(a|aa)*b`
  against 30,000 `a`s finishing well inside a second.
- (As built: `screen` comes with its node patterns in C5, since its checks
  cannot be read without them. The evaluation is `src/check.rs` and the
  HTTP client `src/http.rs`. A backslash before a letter in a pattern is
  written as it is (`matches "\d+"`): the lexer keeps such an escape and
  marks it, and every string but a pattern refuses it with E001, as before.
  JSON paths are checked before running, E009 as well. The journal's HTTP
  observation gained `headers`, and the journal and the baseline write a
  masked value as `<masked>` (DESIGN §10), so the greeter's journal golden
  keeps its bytes. The specs are in `tests/matchers/`, the fixtures in
  `tests/impl/`, and the E008 and E009 goldens come from `tests/specs/`.)

### C3. Pins

- The model gains pins (top level and per target, the target's winning);
  `proc.rs` builds each process's environment from them: `env clean` is
  `Command::env_clear()` plus `PATH` plus what is set and passed; `tz` sets
  `TZ`; `locale "ja-JP"` sets `LANG` and `LC_ALL` to `ja_JP.UTF-8`; `clock`
  and `seed` with `env "NAME"` set NAME. E011 at parse time for `clock` or
  `seed` without `env` on a target that is not a page in Chrome, for a
  locale not written as a language and a region (`ja-JP`, `de-DE`: what
  both `LANG` and Chrome can take), and for a clock that is not an RFC
  3339 time.
- The journal's `env` event and the baseline's first line (Appendix A);
  drift notes pins that differ from the baseline's.
- `tests/pins.rs` with `tests/impl/env.py` (prints the variables it is
  asked about, as JSON): each pin, `env clean` (a variable the test sets
  for geas is gone, PATH is there), `env pass`, the journal's `env` event
  (golden), E011 goldens in both languages, drift's note.
- (As built: `env.py` prints `NAME=value` lines, which a claim reads more
  easily than JSON, and `?NAME` says only whether a variable is set, so
  PATH's value stays out of the goldens. The E011 goldens, and those of
  E003 for a pin given twice and E010 for `{port}` in an `env` value, come
  from `tests/specs/`. `src/pins.rs` reads the RFC 3339 time (to seconds,
  which C8's virtual time needs) and the locale. `tests/pins.rs` also runs
  `map` on a target with `env clean`, and drift on a baseline written
  before pins.)

### C4. Parallel runs

- `sched.rs`: `--jobs N`, `-j N`, `GEAS_JOBS`, default 1. Up to N workers
  take claims in order; a claim waits while a fixed port of a target it
  touches is in use, or while another claim on a `serial` target it
  touches runs. The report, the journal and the baseline are written in
  claim order.
- `port auto`: a registry of the ports geas has handed out and not yet
  taken back; bind `127.0.0.1:0`, read the port, close, and skip numbers
  the registry holds. `{port}` in the command and in `env` values.
  Observations of a `port auto` target have `127.0.0.1:<port>`,
  `localhost:<port>` and `[::1]:<port>` written `{port}` (DESIGN §10).
- The examples switch to `port auto` (greeter, tally-node, tally-go), and
  B6's goldens are blessed again; the difference has to be only that.
- `tests/parallel.rs`: greeter with `-j1` and `-j4` prints and writes the
  same bytes (report, journal, baseline); a fixture service
  (`tests/impl/slow.py`: 400 ms per request, appending `start`/`end` with
  its pid and time to a log in its directory) under six claims: with
  `port auto` and `-j3` some claims overlap in the log; with `serial` none
  do; with a fixed port none do; a service that redirects to its own
  address, checked with `{port}`, and drift quiet across two runs on
  different ports; `GEAS_JOBS`.
- (As built: switching the examples to `port auto` changed none of B6's
  goldens, since no record, report or diff holds the port; the greeter's
  goldens moved by one line, a comment having grown, and its error golden
  names "the port geas gave it". `tally-node` and `tally-go` gained `mask
  header "date"`, without which their journals and baselines differ from
  run to run. Every program runs in a process group of its own (DESIGN
  §10.1), which B2 left open; `tests/parallel.rs` also runs a server behind
  `sh -c`, under `check` and `map`, and interrupts geas in the middle of a
  claim. `tests/languages.rs` holds `map`, `check` and `snap` on all five
  examples to the same bytes with `-j4`. The port registry is in
  `proc.rs`, beside the process groups; `tests/impl/redirect.py` is the
  redirecting service. Stage B's Node hook lost a claim's coverage now and
  then, which byte-for-byte runs cannot have; DESIGN §7.2 says why and
  how it was fixed.)

### C5. The screen

- `screen.rs`: the node of DESIGN §8.1; the roles (Appendix D lists them);
  text rendering (one node a line, two spaces a level, `role "name" value
  "v" disabled`) and JSON both ways; patterns (role, label in its three
  forms, value, state, `exactly n`, `in <role> ["<name>"]`); `mask screen`;
  the tree diff of §8.6.
- The parser gains the GUI calls and node patterns, and E006 and E007
  cover them (a GUI call on a `run` target; `screen` after `get`).
- Unit tests: patterns against sample trees; the diff of two trees
  (appeared, disappeared, value changed, state changed, reordered
  siblings); masks; rendering both ways.
- (As built: `screen.rs` holds the node, the roles of Appendix D, the
  states, the text and JSON forms, §8.1's rules, node patterns, `mask
  screen`, the tree diff, and the 40-node cut a failed check shows. A node
  carries a `handle` (Chrome's backend node id) that is never shown,
  written or compared. The model's `Call` gains the six actions and `Place`
  (`field:`, `into:` with `nth:`); `TargetKind` gains `Pixie` and
  `Driver`; `Matcher` gains `ContainsNode` and `NotContainsNode`; `Mask`
  gains `Screen`. The parser's rules for the GUI forms are in DESIGN §6's
  last list; its static specs are `tests/specs/E0nn-*.geas` like the rest,
  E012 and E013 among them, and its unit tests parse every form and its
  display.)

### C6. The driver protocol

- `driver.rs`: DESIGN §8.5. The driver is started per claim, in the spec's
  directory with the target's pins; geas sends the pins line and waits for
  `ok`; then the actions as the claim reaches them; then `close`. Each
  answer waits at most 5 s.
- `gui.rs`: what live and replayed drivers share: the claim's actions on a
  target, the screen after each, E035 and E037 as diagnostics with the run
  that gets there, the screen of a failed check in the report (cut to 40
  nodes, the matching part first).
- `tests/drivers/fake_driver.py`: a driver of a few dozen lines that reads
  a JSON description of an app (screens, and which action leads from which
  screen to which) named on its command line, and can be told to answer an
  action with an error, with a line that is not JSON, or not at all.
- `tests/gui.rs`: a claim that holds and one that fails (goldens with the
  screens in the run); E035; E037 (a bad line, and no answer: 5 s); E011
  from a refused pin; a claim mixing a `run` target and a driver target
  between two actions; drift on screens (a node that appears is
  unclaimed; one a check names is claimed; `mask screen` hides it);
  `--json`.
- (As built: the fake driver reads `tests/drivers/greeter.json`: screens,
  moves keyed `"<screen> <action>"`, `{1}` and `{2}` standing for what was
  typed into the first and second field, the pins it refuses, and the
  buttons whose click answers a line that is not JSON (`garble`), nothing
  (`hang`), or exits (`die`). Its root is `window "Greeter"`, a role outside
  Appendix D, so the tests also show a root with a role and a role no
  pattern names. The specs are in `tests/gui/`; `tests/gui.rs` adds a
  driver that exits (E037) and `-j2` against `-j1`. A failed screen check
  shows the screen under the check (`the screen:`, `画面:`), once for checks
  of one `when` that share it. `sched::each` hands each claim its worker's
  number, for the Chrome a worker keeps. `proc::Proc` is a program geas
  talks to or waits for (a driver, a pixie app, Chrome), in a process group
  of its own and logged in `GEAS_PID_LOG` like the others, and stopped when
  dropped.)

### C7. pixie

- `pixie.rs`: DESIGN §8.3. Translate the claim's actions on the target into
  one script with `a11y` after each; always set `PIXIE_SCRIPT`; set
  `PIXIE_DUMP` to `.geas/<stem>.pixie-<n>.txt` and read it when it exists,
  stdout otherwise; take the lines starting with `group` as the trees;
  parse each with pixie's role names as anchors and check that it prints
  back to the same line (E037 otherwise); map roles and checkbox values.
  On exit 101, run the prefixes of the script until the first that fails,
  and report E035 with the screen before the refused action and the names
  of its buttons and links. Remove the dump files.
- Static checks in the parser: E012 (another target's `when` between two
  actions on a pixie target), E013 (`into:` on pixie, `open("/x")` on
  pixie, `nth: 0`, `field: 0`).
- `tests/pixie.rs`, run when `GEAS_PIXIE_GREETER` names a built pixie
  greeter app and SKIP otherwise; the test links it into its scratch
  directory as `greeter` so the spec says `pixie "greeter"`: typing and
  clicking, with screen checks that hold (golden); a check that fails
  (golden, with the screen); `click("nope")` (E035, golden, naming
  `greet`); text holding a comma; `open()` alone; drift quiet on the
  unchanged app. E012 and E013 go in `tests/spec.rs`, which needs no app.
- (As built: DESIGN §8.3's last list. The specs are `tests/pixie/greeter.geas`
  (open alone; typing, clicking and Enter; a comma; the second field), whose
  report and journal are goldens, and `tests/pixie/fails.geas` (a check that
  fails, and the refused click), golden in both languages. The test also
  holds the transcripts to being removed, drift to being quiet, and `-j4` to
  printing what `-j1` prints. The greeter has nothing bound to Enter, so the
  claims send it with `submit()`.)

### C8. Chrome

- `ws.rs`: a WebSocket client on a `TcpStream` (DESIGN §8.4): the
  handshake with `Sec-WebSocket-Accept` checked, masked text frames, 16-
  and 64-bit lengths, continuation frames joined, pings answered with
  pongs, close.
- `cdp.rs`: find Chrome (`GEAS_CHROME`, the macOS application,
  `google-chrome`, `chromium`, `chromium-browser`); start it as Appendix C
  lists, with its profile in `.geas/chrome-<n>/`; read the port and the
  browser's WebSocket path from `DevToolsActivePort`; one socket to the
  browser; per claim, `Target.createBrowserContext`,
  `Target.createTarget`, `Target.attachToTarget` with `flatten: true`, and
  the session's commands carrying `sessionId`; dispose of the context at
  the claim's end. Stop Chrome and remove the profile when geas exits,
  panics, or gets SIGINT or SIGTERM (a guard with `Drop`, and the run
  loop checking a flag set by a signal handler declared through the same
  `extern` block as `kill`).
- The browser driver: pins first (Appendix C's order), then `open`,
  `click`, `input`, `submit`, `press`, `advance` as DESIGN §8.4 says, a
  virtual-time budget after each, the tree read and normalized. E034,
  E036, and E013 for a browser action before `open` and a path without
  `/`.
- `tests/pages/app/`: a page and its server (Python, the standard library
  only): a form whose submit adds an item to a list; a status set 300 ms
  after a click; the page's date in the pinned time zone and locale; a
  number from `Math.random`; a counter kept in `localStorage`; and an API
  route the page and the claims both call.
- `tests/browser.rs`, run with Chrome and python3, SKIP otherwise: the
  first screen (golden); typing and submitting, the item appearing; the
  300 ms status, there without waiting in real time; a pinned clock, time
  zone and locale on the page; the same random number in two claims with
  one seed; the counter starting from zero in each claim; `get` and `post`
  in the same claim as the page; drift quiet on the unchanged page, and an
  unclaimed node when the page changes; E034 (`GEAS_CHROME` naming a file
  that does not exist); E035 (`click("nope")`, golden); E036 (a path the
  server never answers: 10 s); `-j2` printing what `-j1` prints. After
  each test: no process whose command line holds the scratch path (Chrome's
  profile is there), and no profile directory left.
- (As built: DESIGN §8.4's last list. The page's server also answers
  `/broken` with two `Content-Length` headers, which Chrome refuses the same
  way every time, so E036 has a case that is golden in both languages
  without waiting 10 s; the 10 s case is golden in English. The reminder
  comes a minute after its click, and `advance(60000)` brings it, so the
  test can hold the claims to taking less than 30 s even on a loaded
  machine. The specs pin `locale`, `tz`, `clock` and `seed`, and mask the
  `date` and `server` headers of Python's server, so their goldens read the
  same on any machine. `tests/browser.rs` also interrupts geas while a page
  loads (exit 130, neither Chrome nor the service alive, no profile left)
  and runs `map` on the page's spec, which records the service behind it.)

### C9. Messages for stage C

- Every code C brings (E009-E013, E034-E037) has its summary, explanation
  and repro in both languages in `codes.rs`, and `tests/explain.rs` runs the
  repros (those needing Chrome or pixie are run when present).
- (As built, for C1-C4: E009, E010 and E011 are in the table with their
  repros, and E002, E003, E004, E007, E008 and E080 say what C1-C4 added to
  them. E012, E013 and E034-E037 come with C5-C8.)
- (As built, for C5-C8: E012 and E013 are static repros; E034's sets
  `GEAS_CHROME` to a file that is not there and needs python3 for its
  service; E035's and E037's are Python drivers of a dozen lines; E036's is
  a service that answers with two `Content-Length` headers, and needs
  python3 and Chrome (`needs` names `chrome`, which `tests/explain.rs`
  looks for as geas does). E002, E004, E006, E007, E008 and E011 say what
  C5-C8 added to them.)

### Stage C is done when

- `cargo build` prints no warning, and `cargo test` passes.
- With python3, node, go, rustc with llvm-tools and Chrome present and
  `GEAS_PIXIE_GREETER` set, `cargo test -- --nocapture 2>&1 | grep -c
  '^SKIP'` prints 0; without the pixie app, the only SKIP lines are
  `tests/pixie.rs`'s.
- `-j1` and `-j4` print and write the same bytes on every example.
- Every code in DESIGN §13 has a repro that gives it, and goldens in both
  languages.
- No Chrome, service or driver process is left after the suite, and
  nothing is left in `CARGO_TARGET_TMPDIR`.
- The report says how long `cargo test` took.

---

## Stage D: the skill, the examples, the READMEs

### D1. `skills/geas/`

- `SKILL.md`, written by hand, with frontmatter `name: geas`, a
  `description` that says when to use it (writing or changing code that a
  `.geas` claims file holds; running the gate; fixing a geas diagnostic;
  proposing claims), and `compatibility` (the binary on PATH, and what
  each optional feature needs), and `license: MIT OR Apache-2.0` (chosen
  by the owner on 2026-10-03; until then it was left out).
- Its sections, after dandori's SKILL.md: when this applies; the division
  of labour (the person reads the claims; the agent writes the code; the
  rules of DESIGN §14); the loop (`check` until it holds, `snap` once the
  person accepts the behaviour, `drift` and `affected` on every change,
  `map` before `affected`); the language on one page (every form of §6,
  with a working claims file); reading each answer (the report, the run
  that gets there, drift's tags, `affected`'s report and exit codes);
  from a diagnostic to a fix (a table of codes); what each language needs
  for `map`; GUI targets; pins and `--jobs`; what to ask the person.
- Reference pages beside it: `language.md` (the whole language, with
  examples), `commands.md` (commands, flags, exit codes, environment
  variables, the formats of Appendix A), `codes.md` (the output of
  `geas explain --all`, copied by a script so it cannot drift),
  `gui.md` (pixie, Chrome, the driver protocol), `map.md` (recording,
  `affected`, what each language needs, CI recipes), `examples.md` (the
  examples and what each shows).
- `skills/README.md`: installing the skill, as dandori's does; and how to
  point an agent that does not read Agent Skills at `SKILL.md` (a line in
  its own instructions file).
- `skills/sync.sh`: writes `codes.md` from `geas explain --all`.
- (As built: seven pages, as listed; pins and `--jobs` are in
  `language.md`, `commands.md` and SKILL.md's §8 rather than a page of
  their own. `codes.md` is a heading, two sentences and the output of
  `geas explain --all` fenced as text; `sync.sh` runs `$GEAS`, else
  `target/debug/geas`, else the geas on PATH. `gui.md` gives a whole
  driver in Python and claims for it, which `tests/skill.rs` runs.
  SKILL.md also tells the agent to `git add -N` a file the change adds
  before `git diff | geas affected`, since an untracked file is not in
  the diff. The frontmatter has `compatibility`, and `license` since the owner chose
  one on 2026-10-03.)

### D2. `geas skill`

- `skill.rs`: the files of `skills/geas/` taken in with `include_str!`.
  `geas skill` prints `SKILL.md`; `geas skill --install <dir>` writes them
  to `<dir>/geas/`, and refuses (E081) to overwrite an existing folder
  without `--force`.
- `--help` names it in its first lines (Appendix A).
- (As built: `--install <dir>` and `--force` are options of `skill` only;
  a file given to `skill`, or `--force` without `--install`, is E080.
  With `--force`, the skill's files are written again and any other file
  in `<dir>/geas` is left as it is. E080's and E081's explanations name
  these cases. `skill.rs` has two unit tests of its own.)

### D3. Examples

- `examples/web-greeter`: the greeter's page in the browser, its server in
  Python, `port auto`, claims that type, click and check the screen, and
  claims on the same service's API.
- `examples/pixie-greeter`: a claims file for pixie's greeter, and a
  README saying how to build the app with pixie and where to put it
  (`examples/pixie-greeter/greeter`, ignored by git).
- Every example gets a short README: what it shows, how to run it, what to
  look for in the output.
- The comments in each claims file are written for the person who audits
  it.
- (As built: `web-greeter` is a page and its Python service with six
  claims, three on the page, one on the API, and two across both; it pins
  `locale`, `tz`, `clock` and `seed`, since the page writes the date,
  greets by the hour and draws a fortune with `Math.random`, and masks the
  `date` and `server` headers. `tests/changes/web-greeter/after/index.html`
  is the page with its Greet button made a `<div>`, which the README runs.
  `pixie-greeter`'s README gives pixie's build commands as pixie's README
  has them; they were not run here, since pixie's tree is not ours to
  build in, and the claims ran against the greeter built earlier
  (`GEAS_PIXIE_GREETER`). `examples/greeter/greeter.ja.geas` is the
  greeter's claims named in Japanese, for README.ja.md. Every example has a
  README; the outputs they quote are numbers from the goldens, and they
  link to the goldens of `tests/languages.rs` for the rest.)

### D4. README.md

Rewritten around v1, in the spike's voice, every output a real one (a
golden, or a command run for the README and kept as a golden):

- what geas is, and the division of labour (keep the thesis);
- the loop on the greeter: `check` failing and then holding;
- drift: the refactor's unclaimed changes;
- the diff half: `map` and `affected` on the greeter's change;
- GUI: a page in Chrome (`examples/web-greeter`), and pixie's greeter
  when the app was at hand for the golden;
- install; the commands (the `--help` output); environment variables;
- for AI agents: `geas skill`, `skills/geas`;
- the examples;
- how it is checked: the tests, the tools they use, what SKIP means;
- status: what is not built (DESIGN §15); naming and licensing still open.
- (As built: every output is a transcript `tests/readme.rs` keeps, the
  `$ ` lines included, recorded in a scratch directory holding the
  examples; the greeter's story runs in a git repository of its own, so
  the README shows `git diff | geas affected`. The README's GUI section
  is the web greeter passing, and then failing once an agent makes its
  button a `<div>`; pixie's greeter follows. A License section was added once the owner
  chose MIT OR Apache-2.0 on 2026-10-03.)

### D5. README.ja.md

Written from scratch in Japanese for a Japanese reader, not translated
from README.md: the same facts, in the order a Japanese reader needs them,
following Appendix B. Its outputs are real runs with `--lang ja` where the
prose is Japanese (the claims and the commands stay as they are).
(As built: the greeter's story runs `greeter.ja.geas`, whose claims are
named in Japanese; the web greeter's and pixie's claims stay English, as
their apps are. Links to pages in English say so.)

### D6. Docs tests

- `tests/docs.rs`: every block fenced as `geas` in README.md, README.ja.md
  and `skills/geas/*.md` parses (a fragment is fenced `text` instead);
  every output line of a `console` block in them (the lines after a `$ `
  line) appears in a golden file; every code they name is in the table;
  every `geas` command and flag they name is in `--help`; every relative
  link resolves.
- `tests/skill.rs`: the copy in the binary is `skills/geas/`; `geas skill`
  prints `SKILL.md`; `geas skill --install` into a scratch directory
  writes the same files; the frontmatter has `name` and `description`;
  no link leaves the folder; `codes.md` is `geas explain --all`.
- (As built: `tests/docs.rs` also takes `skills/README.md` and the
  examples' READMEs. An output is a console block with a line that is not
  a `$ ` line, or a text block that starts as geas's output does; it has
  to be lines of a golden file as they are, `$ ` lines included, `…`
  standing for lines left out. A `geas` block with `…` lines is an excerpt
  of a claims file in the repository; any other parses, which
  `geas drift --json` tells by stopping at E050. The count of codes the
  READMEs give is held to the table. `tests/readme.rs` records the
  transcripts. A `geas` block that starts as an example's claims file has
  to be that file as it is. Twelve pages broken on purpose each failed the
  test they should: a changed count in a README's output, a `geas` block
  that no longer parses, an excerpt that is no longer the file's, a quoted
  example that is no longer the example, an unknown code, an unknown
  option, a broken link, a wrong count of codes, a driver in `gui.md` that
  does not answer the pins, a `$ ` line changed above its output, a stale
  `codes.md`, and a link out of the skill.)

### D7. DESIGN.md

- §1 and §3 get a line pointing at §6 and §8; the status at the top says
  what v1 built. Nothing else in §0-§5 changes.
- (As built: §14 ends with what building the skill decided, §15 gathers
  what C and D left, and §16 what D measured.)

### Stage D is done when

- `cargo test` passes, with `tests/docs.rs` and `tests/skill.rs`.
- Every output in README.md, README.ja.md and the skill is a golden
  excerpt (the docs test says so).
- README.ja.md reads as Japanese written for its reader (Appendix B's
  rules and words), and every example has its README.
- The SKIP count and its reasons are in the report, as are the suite's
  time and the size of the release binary.

---

## Stage E: OpenSpec's scenarios (2026-10-05, built)

DESIGN §17. Built in one night, after ritsu's release v0.23.0, as a
proposal the owner reads before it is taken in.

- `ritsu_base::openspec` (ritsu-base, shared with yuen): a spec's
  requirements, blocks and scenarios, a delta spec's four sections, the
  layout of `openspec/`, held to what OpenSpec 1.14.0's own readers make of
  the same files (ritsu-base's `tests/openspec.rs` and `expected.json`).
- `src/scenarios.rs`: `geas scenarios <spec.geas>... --openspec <path>...`
  with `--json` and `--draft`; `src/cli.rs` (the command, `--openspec`
  given any number of times, `--draft` not with `--json`); E090 in
  `src/codes.rs`, E081's text for a path given to `--openspec`.
- The greeter's spec in OpenSpec, English and Japanese, under
  `examples/greeter/openspec/` and `examples/greeter/ja/openspec/`, with
  the change `trim-names`; `tests/openspec/near.geas`.
- Tests: `tests/scenarios.rs` (the report in both languages, JSON, the
  draft refused by `check` with E005, a near name, E090, E081, E080) and
  the README's story in `tests/readme.rs`; the help and `explain --all`
  goldens.
- Pages: README.md and README.ja.md (a section, the help, 35 codes),
  `examples/greeter/README.md`, the skill's SKILL.md (who writes what for
  scenarios, E090) and commands.md; `skills/sync.sh` writes codes.md.

Done when: `cargo test -p geas` passes with every tool at hand, the pages
hold to the goldens (`tests/docs.rs`), and nothing here starts a program
(`geas scenarios` runs nothing).

## Appendix A. Formats and texts

### The help text (a starting point; the test pins whatever is built)

```text
geas: hold agent-written code to claims a person has read
Coding agents: `geas skill` prints the guide; `geas skill --install <dir>` installs it.

usage:
  geas check <spec.geas>...           run every claim; exit 0 when all hold
  geas snap <spec.geas>...            run them and keep every observation as the baseline
  geas drift <spec.geas>...           run them again and report what changed since the baseline
  geas map <spec.geas>...             run them with coverage on and record the lines each claim runs
  geas affected <spec.geas> <diff|->  the claims a diff touches, and the changed code no claim runs
  geas explain <code> | --all         what a code means and how to fix it
  geas skill [--install <dir> [--force]]  the guide for coding agents

options:
  --json            the answer as JSON
  --lang ja         messages in Japanese (also GEAS_LANG=ja)
  --jobs N, -j N    run up to N claims at once (also GEAS_JOBS); default 1
  --root <dir>      map, affected: the directory paths are relative to
  --out <file>      map: where to write the record
  --map <file>      affected: a record to read; give two for both sides of the diff

exit: 0 all held, or nothing to report · 1 something failed or changed · 2 the spec, a file, or the arguments are wrong
```

(As built: `tests/golden/en/cli/help.txt` and `ja/cli/help.txt`. The usage
line is `geas skill [--install <dir>]`, and `--install <dir>` and `--force`
are among the options.)

### The journal: `.geas/<stem>.journal.jsonl`

One event a line, in claim order:

- `{"claim":…,"event":"env","target":…,"pins":{…}}` (C3, at a claim's
  first use of a target that has pins);
- `{"claim":…,"event":"when","line":…,"target":…,"call":…,"obs":…}`, the
  observation being `{"stdout":…,"stderr":…,"exit":…}`,
  `{"status":…,"headers":{…},"body":…}` (headers lower-cased, sorted), or
  `{"screen":<node>}` (B1 added `line` and, to `error`, `code`; C2 added
  `headers`, with the value of a masked header, and of a masked JSON path
  in the body, written `"<masked>"`, as in the baseline, so that a `date`
  the spec masks does not keep two runs' journals from matching byte for
  byte);
- `{"claim":…,"event":"check","line":…,"check":…,"expected":…,"actual":…,"ok":…}`
  (for a `screen` check, `expected` is the node as the spec writes it and
  `actual` how many nodes matched: `no such node`, `1 such node`, `2 such
  nodes`);
- `{"claim":…,"event":"error","line":…,"code":…,"message":…}`.

### The baseline: `.geas/<stem>.baseline.jsonl`

- From C3, a first line `{"geas_baseline":1,"pins":{"<target>":{…}}}`; a
  baseline without it (B's, or the spike's) is still read, as one whose
  targets had no pins. Only targets that have pins are in it. A target's
  pins are the members that are set, in this order: `"clean":true`,
  `"pass":[…]`, `"env":{…}`, `"tz"`, `"locale"`, `"clock"`, `"clock_env"`,
  `"seed"`, `"seed_env"`; `{port}` is written as it is. The journal's
  `env` event carries the same object.
- One line per observation:
  `{"claim":…,"idx":…,"target":…,"call":…,"obs":{"kind":"proc"|"http"|"screen",…}}`.

A screen, in the journal, the baseline (`"screen":<node>`) and `--json`, is
a node: `{"role":…,"name":…,"value":…,"states":[…],"children":[…]}`
without the members that are empty, the screen itself being a node without
a role (`{"children":[…]}`). A node a `mask screen` matches is written
`{"role":…,"masked":true}`, its other members and what is under it left
out.

### The record: `.geas/<stem>.map.jsonl`

```text
{"geas_map":1,"spec":"examples/greeter/greeter.geas","root":"../..","claims":[{"name":"greets by name","status":"ok","targets":["api"]},…]}
{"file":"examples/greeter/server.py","blob":"<40 hex>","lang":"python","code":"1-4,6,9-11"}
{"file":"examples/greeter/server_refactored.py","blob":"<40 hex>","lang":"python","code":null}
{"claim":"greets by name","target":"api","file":"examples/greeter/server.py","ran":"1-4,9-11"}
```

The header first; then the files, sorted by path; then the claims' lines,
in claim order, by target (in spec order) and by path within a claim.
`spec` is the spec's path relative to the root; `root` is the root
relative to the spec's directory; a claim's `targets` are those it
started, which `affected` needs for startup code; `lang` is `python`,
`node`, `go` or `rust`. (As built: B4 added `targets` and `target`, so
that what each target ran is kept apart.)

### `--json` answers

- `check`, `snap`, `map`: `{"geas":1,"ok":…,"file":…,"claims":[{"name",
  "line","status","error":null|{"code","line","col","message","notes"},
  "checks":[{"line","check","expected","actual","ok"}],"run":[{"line","call",
  "observed"}]}]}`; `run` only for a claim that is not ok, `call` as the
  spec writes it after `when` (`api.get("/")`), `observed` `null` for the
  `when` a claim error stopped at. A failed `screen` check carries
  `"screen"`, the screen it saw. A check's `check` is its subject and its
  matcher's words (`status is between`), `expected` the matcher's value as
  the spec writes it (`200 and 299`, a pattern as `"\d+"`), empty for
  `exists` and `does not exist`; the journal's `check` event is the same. `check` adds `"diagnostics"`, the
  keys written in the spec (W901), `[]` when it has none; `map` adds
  `"map":{"record","files","code","ran"}` and `"diagnostics"` for the
  warnings. Several specs print one object a line.
- A spec that does not parse: `{"geas":1,"ok":false,"file":…,
  "diagnostics":[…]}`.
- `drift`: `{"geas":1,"file":…,"compared":…,"drifted":…,"unclaimed":…,
  "claimed":…,"changes":[{"claim","when","line","call","field","old","new",
  "claimed"}],"notes":[…],"diagnostics":[…]}`: `when` is the `when`'s place
  in its claim from 1, `line` its line; `old` and `new` are JSON values, a
  string for a text or a header, a number for exit and status, the value
  itself for a `body json` path, `null` when absent; `diagnostics` holds
  the claims that could not run. A screen's change has the `field`
  `screen`, or `screen in <role> "<name>"` for a node under another, and
  `old` and `new` are the node without its children.
- `explain`: one object a line, `{"geas":1,"code","severity","summary",
  "when","fix","repro":{"args":[…],"files":[{"name","text"}],"exit",
  "needs":[…],"env":{…}}}`; `env` holds the variables the repro sets for
  geas (E065's `GEAS_LLVM_BIN`), and `needs` may name `llvm-tools`, the
  component in the Rust toolchain's sysroot.
- `affected`: `{"geas":1,"spec":…,"diff":…,"records":[{"file","side"}],
  "claims":[{"index","name","status","lines":[{"file","side","lines","how",
  "startup"}]}],"unclaimed":[{"file","side","lines","why"}],
  "deleted":[{"file","known"}],"outside":[…],"spec_changed":[…],
  "baseline_changed":…,"ok":…}`. A line's `side` is `after` (numbered in
  the code after the change) or `before` (a removed line, numbered in the
  code before it); a record's `side` is one of those, `either` when no
  changed source file tells, or `mixed`. `how` is `ran` or `near`;
  `startup` is the target every claim of which runs the line, or null; `why`
  is `not run` or `not reported`; `known` says whether a record of the code
  before the change says which claims ran a deleted file. (As built: B5 put
  the side on every line, since removed lines are numbered in the code
  before the change, and split `startup` from `how`, since a removal next
  to startup code is both.)

## Appendix B. Japanese

The Japanese messages (`--lang ja`) and README.ja.md have to read as
Japanese a person would write, not as English carried over word by word.
The rules:

- Do not make a kanji compound out of an English concept word. If a
  two-kanji word comes to mind for an English term, doubt it first. Where
  a katakana loanword is what people say, write it (イベント, リトライ,
  タイムアウト, リクエスト, レスポンス, スクリプト, カバレッジ, ゲート).
- Do not put たち on things. Do not copy English sentence structure: keep
  the subject right, do not point back with それ at one option inside a
  long sentence, and when a sentence ends in しません, answer the reader's
  next question (why, and what instead).
- Words that read as translation, kept out of every Japanese text:
  道, 升目, 目録, 雛形, 欄, 証人, 断片, 束縛, 台本, 活動, 平たい, 印字,
  描画, 番人, 格子, 遮蔽, 被覆, 正準, 原本, 字面, 群, 入力空間, 排他,
  糖衣, 箱, 門, 建てる.
- One word, one meaning, within a paragraph.

The words geas uses:

| English | Japanese |
|---|---|
| claim | 主張 |
| the claims file, the spec | 主張のファイル |
| target | ターゲット |
| check (a `then` line) | チェック |
| observation (what a `when` gives its checks) | 結果 |
| subject (what a check names: stdout, status, …) | チェックするもの |
| matcher (is, contains, …) | 比べ方 |
| call (run, get, post, …) | 呼び出し |
| interaction (one `when` and what it gave) | やりとり |
| body (HTTP) | ボディ |
| holds / does not hold | 成り立つ / 成り立たない |
| expected … got … | … のはずが、実際は … |
| the run that gets there | ここまでの実行 |
| journal | ジャーナル |
| baseline | ベースライン |
| drift (the command) / a change it finds | ドリフト / 変化 |
| unclaimed / claimed (drift's tags) | 主張なし / 主張あり |
| mask | マスク |
| the record (`map`'s file); to record | 記録; 記録を取る |
| the root (of the record's paths) | ルート |
| the code before / after the change | 変更前のコード / 変更後のコード |
| a removed line, a deleted file | 削除した行, 削除したファイル |
| a removal attributed by its neighbours ("next to lines it runs") | 通る行の隣 |
| outside the record | 記録の外のファイル |
| source file | ソースファイル |
| runtime; profile (Rust's); counters, metadata (Go's) | ランタイム; プロファイル; カウンター, メタデータ |
| header (a hunk's, the record's first line) | ヘッダー |
| kill (a process, after SIGTERM) | 強制終了する |
| stale | 古い |
| diff, hunk | 差分, ハンク |
| touches (a claim) | （主張に）関わる |
| unclaimed code | どの主張も通らないコード |
| coverage | カバレッジ |
| service, process, port | サービス, プロセス, ポート |
| screen | 画面 |
| accessibility tree | アクセシビリティツリー |
| role, name, value | ロール, 名前, 値 |
| action (click, input, …) | 操作 |
| refused | 拒否された |
| settle | 落ち着く |
| virtual time | 仮想時間 |
| browser context | ブラウザーコンテキスト |
| driver | ドライバー |
| pin (a pin, to pin) | 固定（固定する） |
| environment variable, time zone, locale, seed | 環境変数, タイムゾーン, ロケール, 乱数のシード |
| script (pixie's) | スクリプト |
| run side by side | 並列に走らせる |
| skill | スキル |
| a command's word; blank (white space) | 語; 空白 |
| quote (single, double); backslash; escape | クォート（シングルクォート、ダブルクォート）; バックスラッシュ; エスケープ |
| pattern; character class; group; repetition, count | パターン; 文字クラス; グループ; 繰り返し, 回数 |
| brace, bracket; lookaround | 中かっこ, 角かっこ; 先読み |
| a pattern matches (the whole value) | （値の全体と）照らし合わせる |
| number matcher (`is above`, …) | 数の比べ方 |
| clock (the pin) | 時刻 |
| the port of `port auto`; a free port | geas が渡したポート; 空きポート |
| node (of a screen) | ノード |
| a node that matches a pattern ("such node") | 当てはまるノード |
| state (`disabled`, `checked`) | 状態 |
| page (in a browser); to load it | ページ; 読み込む |
| button, link, checkbox, radio button, switch, tab, menu item, option | ボタン, リンク, チェックボックス, ラジオボタン, スイッチ, タブ, メニュー項目, 選択肢 |
| text field | テキストフィールド |
| control (what a person operates on a screen) | コントロール |
| static text (the `text` role) | 静的な文字列 |
| click; what can be clicked | クリック; クリックできるもの |
| a field's place among the fields (`field: n`) | 何番目か |
| whole number; a count (`exactly n`) | 整数; 個数 |
| the page's clock | ページの時刻 |
| headless (Chrome) | ヘッドレス |
| debugging port (Chrome's) | デバッグ用のポート |
| profile directory (Chrome's) | プロファイルのディレクトリ |
| a driver's answer; the first line, which hands it the pins | 応答; 固定を渡す最初の行 |
| screen reader | スクリーンリーダー |
| (a screen) as above | 上と同じ |
| the guide `geas skill` prints | 手引き |
| skill folder; install it; overwrite | スキルのフォルダー; インストールする; 上書きする |
| override (a method) | オーバーライド |
| the fortune the example page draws | 占いの文句 |

A word not in the table: check how the repository already says it
(`grep -r` over the Japanese files), and if nobody says it yet, ask before
coining it.

## Appendix C. Measured starting points

These were run on this machine on 2026-10-02 (DESIGN §16); start from them.

### Python: the probe's `sitecustomize.py`

What was measured: it recorded `calc.py`'s lines and a service's lines
before SIGTERM. B3 lists what to add.

```python
import os, sys, json, signal, atexit

_out = os.environ.get("GEAS_COVER_OUT")
_root = os.path.realpath(os.environ.get("GEAS_COVER_ROOT", os.getcwd()))
if _out and hasattr(sys, "monitoring"):
    _mon = sys.monitoring
    _TOOL = _mon.COVERAGE_ID
    _hits = {}   # file -> set(lines)
    _codes = {}  # file -> top-level code objects seen

    def _mine(fn):
        try:
            p = os.path.realpath(fn)
        except Exception:
            return None
        return p if p.startswith(_root + os.sep) and p.endswith(".py") else None

    def _line(code, line):
        p = _mine(code.co_filename)
        if p is None:
            return _mon.DISABLE
        _hits.setdefault(p, set()).add(line)
        return _mon.DISABLE

    def _start(code, offset):
        p = _mine(code.co_filename)
        if p is not None and code.co_name == "<module>":
            _codes.setdefault(p, []).append(code)
        return _mon.DISABLE

    _mon.use_tool_id(_TOOL, "geas")
    _mon.register_callback(_TOOL, _mon.events.LINE, _line)
    _mon.register_callback(_TOOL, _mon.events.PY_START, _start)
    _mon.set_events(_TOOL, _mon.events.LINE | _mon.events.PY_START)

    def _executable(code, acc):
        for _, _, line in code.co_lines():
            if line is not None:
                acc.add(line)
        for c in code.co_consts:
            if hasattr(c, "co_lines"):
                _executable(c, acc)

    _written = False
    def _write():
        global _written
        if _written:
            return
        _written = True
        files = {}
        for p in set(_hits) | set(_codes):
            ex = set()
            for c in _codes.get(p, []):
                _executable(c, ex)
            files[p] = {"executable": sorted(ex), "executed": sorted(_hits.get(p, ()))}
        os.makedirs(_out, exist_ok=True)
        with open(os.path.join(_out, "python-%d.json" % os.getpid()), "w") as f:
            json.dump({"files": files}, f)

    atexit.register(_write)
    def _term(signum, frame):
        _write()
        os._exit(128 + signum)
    signal.signal(signal.SIGTERM, _term)
```

(The probe's keys were `executable` and `executed`; v1's are `code` and
`ran`.)

### Node: the hook, as measured

```js
process.on("SIGTERM", function geasCover() {
  if (process.listeners("SIGTERM").length === 1) process.exit(143);
});
```

(The hook measured on 2026-10-02 called `v8.takeCoverage()` first. That
resets the counts, and the exit then writes a second file under the same
name, `coverage-<pid>-<ms>-0.json`, without them: in stage C, 33 runs of
60 lost the script's lines that way, and stage B's `tally-node` record
once lost a claim. `process.exit(143)` alone is a normal exit, and V8
writes the counts then; 30 runs of 30 kept them, with and without a
handler of the program's own.)

### Go and Rust: the commands

```sh
go build -cover -coverpkg=./... -trimpath -o tally .
GOCOVERDIR=<dir> ./tally …
go tool covdata textfmt -i=<dir> -o <dir>/go.txt

rustc --edition 2024 -C instrument-coverage -o tally src/main.rs
LLVM_PROFILE_FILE=<dir>/rust-%p-%m.profraw ./tally …
<llvm bin>/llvm-profdata merge -sparse <dir>/*.profraw -o <dir>/rust.profdata
<llvm bin>/llvm-cov export -format=lcov -instr-profile=<dir>/rust.profdata ./tally
```

### Chrome: what the probe did, in order

1. Start: `<chrome> --headless=new --remote-debugging-port=0
   --user-data-dir=<profile> --no-first-run --no-default-browser-check
   --disable-gpu --disable-extensions --disable-background-networking
   --disable-sync --mute-audio about:blank`, stdin, stdout and stderr to
   null.
2. Poll `<profile>/DevToolsActivePort` every 20 ms (up to 15 s): line 1 is
   the port, line 2 the browser's WebSocket path.
3. Connect the WebSocket to `127.0.0.1:<port><path>`. Reading HTTP answers
   from the debugging port: by `Content-Length`, with a read timeout; a
   read to the end of the stream hung.
4. Per claim, on the browser socket: `Target.createBrowserContext`
   `{"disposeOnDetach":true}`; `Target.createTarget` `{"url":"about:blank",
   "browserContextId":…}`; `Target.attachToTarget` `{"targetId":…,
   "flatten":true}`, whose `sessionId` goes on every later command for
   that page.
5. In the session: `Page.enable`, `Accessibility.enable`, `DOM.enable`,
   `Runtime.enable`; the pins (`Emulation.setLocaleOverride`
   `{"locale":…}`, `Emulation.setTimezoneOverride` `{"timezoneId":…}`,
   `Page.addScriptToEvaluateOnNewDocument` `{"source":<the seeded
   Math.random>}`); `Emulation.setVirtualTimePolicy` `{"policy":"pause",
   "initialVirtualTime":<seconds since 1970>}`.
6. `Page.navigate` `{"url":…}`, then `Emulation.setVirtualTimePolicy`
   `{"policy":"pauseIfNetworkFetchesPending","budget":1000}`, then wait for
   the event `Emulation.virtualTimeBudgetExpired` with this `sessionId`.
7. Read: `Accessibility.getFullAXTree` (nodes with `nodeId`, `parentId`,
   `childIds`, `ignored`, `role.value`, `name.value`, `value.value`,
   `properties[]` such as `disabled`, and `backendDOMNodeId`).
8. Click: `DOM.scrollIntoViewIfNeeded` and `DOM.getBoxModel`
   `{"backendNodeId":…}`; the centre of `model.content`'s four points;
   `Input.dispatchMouseEvent` `mousePressed` then `mouseReleased`
   (`"button":"left","clickCount":1`). Type: `DOM.focus`
   `{"backendNodeId":…}`, `Input.insertText` `{"text":…}`. Enter:
   `Input.dispatchKeyEvent` `keyDown` with `"key":"Enter","code":"Enter",
   "windowsVirtualKeyCode":13,"text":"\r"`, then `keyUp` without `text`.
   After each action, step 6's budget again.
9. At the claim's end, `Target.disposeBrowserContext`; at the end of the
   run, kill Chrome, wait for it, and remove the profile.

As built (stage C), on top of these steps: `Page.navigate` is answered
only once the service begins to answer, so its own 10 s is where a page
that never comes is found (E036); `input` resolves the field
(`DOM.resolveNode`) and selects what it holds (`Runtime.callFunctionOn`)
before `Input.insertText`; `submit` focuses the field and sends Enter;
`press` sends `keyDown` and `keyUp` with `key`, `code`,
`windowsVirtualKeyCode` and `modifiers` (alt 1, ctrl 2, cmd 4, shift 8),
and the key's `text` unless cmd, ctrl or alt is held;
`Page.javascriptDialogOpening` is answered with
`Page.handleJavaScriptDialog` `{"accept":true}`; Chrome is stopped by
killing its process group, and its profile removed after.

The seeded `Math.random` the probe installed (an xorshift on 32 bits) gave
the same first number in two contexts:

```js
(() => { let x = SEED >>> 0 || 1; Math.random = function () { x ^= x << 13; x >>>= 0; x ^= x >>> 17; x ^= x << 5; x >>>= 0; return x / 4294967296; }; })();
```

### pixie

- `PIXIE_SCRIPT="input:Ada,click:greet"` on the greeter printed two
  element trees (start and end) in 32 ms; with `a11y` steps, one
  `group[…]` line per step.
- A refused step: exit 101, nothing on stderr, and the trees of earlier
  `a11y` steps lost.
- `PIXIE_DUMP=<file>`: the transcript went to the file, stdout stayed
  empty.
- `input:a\, b` typed `a, b`; the tree printed `=a, b` unescaped.
- Never run a pixie app without `PIXIE_SCRIPT`: it opens a window.

### The machine

node v23.11.0 (runs `.ts` without flags), python3 3.14.6, go 1.25.5,
rustc 1.94.1 with llvm-tools in the stable toolchain, Chrome
154.0.8037.93. Environment variables the tests read: `GEAS_BLESS`,
`GEAS_PIXIE_GREETER`, and `GEAS_CHROME`, which they hand on to geas; geas
itself reads `GEAS_LANG`, `GEAS_JOBS`, `GEAS_CHROME`, `GEAS_LLVM_BIN`.

## Appendix D. Roles

The role names a node pattern may use: WAI-ARIA 1.2's non-abstract roles,
and `text`.

```text
alert alertdialog application article banner blockquote button caption
cell checkbox code columnheader combobox complementary contentinfo
definition deletion dialog directory document emphasis feed figure form
generic grid gridcell group heading img insertion link list listbox
listitem log main marquee math menu menubar menuitem menuitemcheckbox
menuitemradio meter navigation none note option paragraph presentation
progressbar radio radiogroup region row rowgroup rowheader scrollbar
search searchbox separator slider spinbutton status strong subscript
superscript switch tab table tablist tabpanel term textbox time timer
toolbar tooltip tree treegrid treeitem text
```

A role a driver reports outside this list (Chrome's internal ones, such
as `Iframe`) is kept as it is: it shows in the screen and in drift, and no
pattern can name it.
