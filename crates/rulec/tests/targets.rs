//! What `rulec gen` writes (§15.277): the library of each target it is given, and beside them the
//! extras it is asked for, nothing else. With no `--target` it writes nothing and stops at 2,
//! naming the arguments that write what 0.40.0 wrote with none. A file of an extra the run does not
//! ask for, found where it would be written, is named and kept. `rulec test` runs what is there.

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A rule every target and every extra writes something for: its enum is read from the standard
/// library, so the rule is the one file it needs.
const RULE: &str = "tests/corpus/member_shipping_fee.rule";

/// The library of each target, for [`RULE`]: what `--target <name>` alone writes.
const LIBRARY: &[(&str, &[&str])] = &[
    ("python", &["python/member_shipping_fee.py"]),
    ("numpy", &["numpy/member_shipping_fee.json", "numpy/rulec_np.py"]),
    ("typescript", &["typescript/member_shipping_fee.ts"]),
    ("javascript", &["javascript/member_shipping_fee.mjs"]),
    ("rust", &["rust/member_shipping_fee.rs"]),
    ("ruby", &["ruby/member_shipping_fee.rb", "ruby/sig/member_shipping_fee.rbs"]),
    ("php", &["php/member_shipping_fee.php"]),
    ("go", &["go/membershippingfee/go.mod", "go/membershippingfee/member_shipping_fee.go"]),
    ("swift", &["swift/member_shipping_fee.swift"]),
    ("java", &["java/MemberShippingFee.java"]),
    ("sql", &["sql/member_shipping_fee.sql", "sql/member_shipping_fee_function.sql"]),
    ("wasm", &["wasm/member_shipping_fee.rs", "wasm/member_shipping_fee.wit", "wasm/member_shipping_fee_wasm.rs"]),
    ("proto", &["proto/buf.yaml", "proto/rulec/member_shipping_fee/v4/member_shipping_fee.proto"]),
];

/// What each extra writes beside the library, for [`RULE`]: the targets it is asked with, and the
/// files it adds.
const EXTRA: &[(&[&str], &str, &[&str])] = &[
    (&["python"], "runner", &["python/member_shipping_fee_runner.py"]),
    (&["numpy"], "runner", &["numpy/member_shipping_fee_runner.py"]),
    (&["typescript"], "runner", &["typescript/member_shipping_fee_runner.ts"]),
    (&["javascript"], "runner", &["javascript/member_shipping_fee_runner.mjs"]),
    (&["rust"], "runner", &["rust/member_shipping_fee_runner.rs"]),
    (&["ruby"], "runner", &["ruby/member_shipping_fee_runner.rb"]),
    (&["php"], "runner", &["php/member_shipping_fee_runner.php"]),
    (&["go"], "runner", &["go/membershippingfeerunner/go.mod", "go/membershippingfeerunner/main.go"]),
    (&["swift"], "runner", &["swift/member_shipping_fee_runner.swift"]),
    (&["java"], "runner", &["java/MemberShippingFeeRunner.java"]),
    (&["sql"], "runner", &["sql/member_shipping_fee_function_runner.py", "sql/member_shipping_fee_runner.py"]),
    (&["wasm"], "runner", &["wasm/member_shipping_fee_runner.mjs"]),
    (&["python"], "mcp", &["python/member_shipping_fee_mcp.py", "python/member_shipping_fee_page.html"]),
    (&["typescript"], "mcp", &["typescript/member_shipping_fee_mcp.ts", "typescript/member_shipping_fee_page.html"]),
    (&["javascript"], "mcp", &["javascript/member_shipping_fee_mcp.mjs", "javascript/member_shipping_fee_page.html"]),
    (&["python", "proto"], "connect", &["proto/buf.gen.yaml", "python/member_shipping_fee_service.py"]),
    (&["python"], "round-test", &["python/_round_test.py"]),
    (&["numpy"], "round-test", &["numpy/_round_test.py"]),
    (&["typescript"], "round-test", &["typescript/_round_test.ts"]),
    (&["javascript"], "round-test", &["javascript/_round_test.mjs"]),
    (&["rust"], "round-test", &["rust/_round_test.rs"]),
    (&["ruby"], "round-test", &["ruby/_round_test.rb"]),
    (&["php"], "round-test", &["php/_round_test.php"]),
    (&["go"], "round-test", &["go/membershippingfee/round_test.go"]),
    (&["swift"], "round-test", &["swift/_round_test.swift"]),
    (&["java"], "round-test", &["java/_RoundTest.java"]),
    (&["sql"], "round-test", &["sql/_round_test.py"]),
    (&["wasm"], "round-test", &["wasm/_round.rs", "wasm/_round_test.mjs"]),
    (&["rust"], "proof", &["rust/member_shipping_fee_proof.rs"]),
    (&["python"], "vectors", &["vectors/member_shipping_fee.expected.jsonl", "vectors/member_shipping_fee.jsonl", "vectors/member_shipping_fee.refused.jsonl"]),
];

/// The runner of the Connect service: written only when both `runner` and `connect` are asked for.
const CONNECT_RUNNER: &str = "python/member_shipping_fee_connect_runner.py";

/// `rulec <args>` in `dir`, in `lang`: the exit code, the standard output and the standard error.
/// `rulec test` has its TMPDIR under `dir`, so what a toolchain leaves there goes with it.
fn rulec_in(dir: &Path, lang: &str, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rulec"));
    c.env("RULEC_LANG", lang).env_remove("RITSU_LANG").current_dir(dir).args(args);
    if args.first() == Some(&"test") {
        c.env("TMPDIR", tmpdir_in(dir));
    }
    let o = c.output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Every file under `dir`, by its path from `dir`, with its bytes.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                todo.push(p);
            } else {
                out.insert(p.strip_prefix(dir).unwrap().to_string_lossy().into_owned(), std::fs::read(&p).unwrap());
            }
        }
    }
    out
}

/// A directory with a copy of [`RULE`] in it, for a run that writes beside it.
fn with_rule(name: &str) -> TempDir {
    let t = TempDir::new(name);
    std::fs::copy(root().join(RULE), t.path().join("member_shipping_fee.rule")).unwrap();
    t
}

/// What `rulec gen <RULE> <args> --out out` writes, by path, in English.
fn written(args: &[&str]) -> BTreeMap<String, Vec<u8>> {
    let t = with_rule("targets");
    let mut a = vec!["gen", "member_shipping_fee.rule"];
    a.extend_from_slice(args);
    a.extend(["--out", "out"]);
    let (code, said, err) = rulec_in(t.path(), "en", &a);
    assert_eq!(code, 0, "gen {args:?}: {said}{err}");
    files(&t.path().join("out"))
}

/// `--target <t>` for each of `targets`, and `--extra <e>` for each of `extras`.
fn flags<'a>(targets: &[&'a str], extras: &[&'a str]) -> Vec<&'a str> {
    let mut v = Vec::new();
    for t in targets {
        v.extend(["--target", *t]);
    }
    for e in extras {
        v.extend(["--extra", *e]);
    }
    v
}

fn library_of(target: &str) -> &'static [&'static str] {
    LIBRARY.iter().find(|(t, _)| *t == target).map(|(_, f)| *f).unwrap_or_else(|| panic!("no library listed for {target}"))
}

/// Each target alone writes its library and nothing else, the same bytes as when every target and
/// every extra is written; the targets are the languages of src/backend.rs and the `.proto`.
#[test]
fn each_target_alone_writes_its_library_and_nothing_else() {
    let names: Vec<&str> = LIBRARY.iter().map(|(t, _)| *t).collect();
    assert_eq!(names, rulec::targets::target_names(), "a target was added or taken away: list its library here");
    let full = written(rulec::targets::UP_TO_0_40);
    for (t, want) in LIBRARY {
        let one = written(&["--target", t]);
        assert_eq!(one.keys().map(String::as_str).collect::<Vec<_>>(), want.to_vec(), "--target {t}");
        for (p, body) in &one {
            assert_eq!(Some(body), full.get(p), "--target {t}: {p} is not what it is beside every other target and extra");
        }
    }
    let all = written(&flags(&names, &[]));
    let libraries: Vec<&str> = LIBRARY.iter().flat_map(|(_, f)| f.iter().copied()).collect();
    let mut sorted = libraries.clone();
    sorted.sort();
    assert_eq!(all.keys().map(String::as_str).collect::<Vec<_>>(), sorted, "every target, no extra");
}

/// Each extra adds its files and no other to the libraries of the targets it is asked with; with
/// every target and every extra, what is written is the 60 files 0.40.0 wrote with no `--target`
/// and nothing else, the runner of the Connect service among them, which takes both `runner` and
/// `connect`.
#[test]
fn each_extra_writes_its_files_beside_the_library() {
    for (targets, extra, adds) in EXTRA {
        let got = written(&flags(targets, &[extra]));
        let mut want: Vec<&str> = targets.iter().flat_map(|t| library_of(t).iter().copied()).chain(adds.iter().copied()).collect();
        want.sort();
        assert_eq!(got.keys().map(String::as_str).collect::<Vec<_>>(), want, "{targets:?} with --extra {extra}");
    }
    // the extras every target has, all listed above
    for t in rulec::targets::target_names() {
        for e in rulec::targets::EXTRAS.iter().filter(|e| rulec::targets::has_extra(t, e) && **e != "connect") {
            assert!(EXTRA.iter().any(|(ts, x, _)| *ts == [*t] && **x == **e), "{t} has --extra {e}, and it is not listed here");
        }
    }
    let both = written(&flags(&["python", "proto"], &["runner", "connect"]));
    assert!(both.contains_key(CONNECT_RUNNER), "runner and connect: {:?}", both.keys());
    let full = written(rulec::targets::UP_TO_0_40);
    let mut want: Vec<&str> = LIBRARY.iter().flat_map(|(_, f)| f.iter().copied()).chain(EXTRA.iter().flat_map(|(_, _, f)| f.iter().copied())).chain([CONNECT_RUNNER]).collect();
    want.sort();
    want.dedup();
    assert_eq!(full.keys().map(String::as_str).collect::<Vec<_>>(), want);
    assert_eq!(full.len(), 60);
}

/// The files are written in the order of the lists of targets and extras, not the order of the
/// flags, so two command lines that ask for the same say the same.
#[test]
fn the_order_of_the_flags_changes_nothing() {
    let said = |args: &[&str]| -> String {
        let t = with_rule("targets-order");
        let mut a = vec!["gen", "member_shipping_fee.rule", "--out", "out"];
        a.extend_from_slice(args);
        let (code, out, err) = rulec_in(t.path(), "en", &a);
        assert_eq!(code, 0, "{out}{err}");
        out
    };
    let one = said(&["--target", "python", "--target", "go", "--extra", "runner", "--extra", "round-test"]);
    let two = said(&["--extra", "round-test", "--target", "go", "--extra", "runner", "--target", "python"]);
    assert_eq!(one, two);
    let order: Vec<&str> = one.lines().filter_map(|l| l.strip_prefix("generated: out/")).collect();
    assert_eq!(order.first(), Some(&"python/member_shipping_fee.py"), "{one}");
    assert!(order.iter().position(|p| p.starts_with("python/")) < order.iter().position(|p| p.starts_with("go/")), "{one}");
}

/// With no `--target`, nothing is written, and the refusal says what to give, and the command that
/// writes what 0.40.0 wrote with none: the words given (an `--extra` among them left out, since the
/// command asks for every extra), with every target and every extra after them. That command does.
#[test]
fn with_no_target_nothing_is_written_and_the_command_of_0_40_is_named() {
    let names = "python|numpy|typescript|javascript|rust|ruby|php|go|swift|java|sql|wasm|proto";
    let extras = "runner|mcp|connect|round-test|proof|vectors";
    let up_to = rulec::targets::UP_TO_0_40.join(" ");
    for (lang, first, note) in [
        (
            "en",
            format!("error: `rulec gen` writes only the targets it is given: give one or more `--target {names}`, and `--extra {extras}` for what to write beside them"),
            "note: up to 0.40.0, `rulec gen` with no `--target` wrote every target with every extra; this writes the same files:",
        ),
        (
            "ja",
            format!("error: `rulec gen` が書くのは、指定した出力先だけです。`--target {names}` を一つ以上付けてください。横に書く付属物は `--extra {extras}` で指定します"),
            "注記: 0.40.0 までの `rulec gen` は、`--target` が無ければ、全部の出力先を全部の付属物つきで書きました。同じファイルを書くのは次のコマンドです",
        ),
    ] {
        let t = with_rule("targets-none");
        let (code, out, err) = rulec_in(t.path(), lang, &["gen", "member_shipping_fee.rule", "--extra", "runner", "--out", "my out", "--lang", lang, "--check"]);
        assert_eq!((code, out.as_str()), (2, ""), "{lang}: {err}");
        assert_eq!(err, format!("{first}\n{note}\n  rulec gen member_shipping_fee.rule --out 'my out' --lang {lang} --check {up_to}\n"), "{lang}");
        assert!(!t.path().join("my out").exists() && !t.path().join("generated").exists(), "{lang}: written before it stopped");
        // the command named, as a shell would split it, writes the 60 files
        let (code, out, err) = rulec_in(t.path(), lang, &rulec::targets::up_to_0_40(&["gen", "member_shipping_fee.rule", "--out", "my out", "--lang", lang]));
        assert_eq!(code, 0, "{lang}: {out}{err}");
        assert_eq!(files(&t.path().join("my out")).len(), 60, "{lang}");
    }
}

/// A target or an extra given twice, a name that is not a target, an extra none of the targets
/// given has, and `connect` without both Python and the `.proto` stop at 2, before anything is
/// written, in both languages.
#[test]
fn what_cannot_be_written_stops_at_2_before_anything_is() {
    let cases: &[(&[&str], &str, &str)] = &[
        (&["--target", "python", "--target", "python"], "`--target python` is given twice", "`--target python` が二度書かれています"),
        (&["--target", "go", "--extra", "runner", "--extra", "runner"], "`--extra runner` is given twice", "`--extra runner` が二度書かれています"),
        (
            &["--target", "cobol"],
            "`--target cobol` is not a value this flag takes; it takes only python | numpy | typescript | javascript | rust | ruby | php | go | swift | java | sql | wasm | proto",
            "`--target` に `cobol` は使えません。書けるのは python | numpy | typescript | javascript | rust | ruby | php | go | swift | java | sql | wasm | proto だけです",
        ),
        (&["--target", "python", "--extra", "proof"], "`--extra proof` is an extra of rust, and there is no `--target rust`", "`--extra proof` は rust の付属物です。`--target rust` がありません"),
        (
            &["--target", "sql", "--target", "proto", "--extra", "mcp"],
            "`--extra mcp` is an extra of python, typescript and javascript, and none of them is given as a `--target`",
            "`--extra mcp` は python・typescript・javascript の付属物です。どれも `--target` にありません",
        ),
        (
            &["--target", "python", "--extra", "connect"],
            "`--extra connect` needs both `--target python` and `--target proto`: the service is Python, and it reads the stubs buf builds from the `.proto`",
            "`--extra connect` には `--target python` と `--target proto` の両方が要ります。サービスは Python で、buf が `.proto` から作るスタブを読みます",
        ),
        (
            &["--target", "proto", "--extra", "connect"],
            "`--extra connect` needs both `--target python` and `--target proto`: the service is Python, and it reads the stubs buf builds from the `.proto`",
            "`--extra connect` には `--target python` と `--target proto` の両方が要ります。サービスは Python で、buf が `.proto` から作るスタブを読みます",
        ),
    ];
    for (args, en, ja) in cases {
        for (lang, says) in [("en", en), ("ja", ja)] {
            let t = with_rule("targets-refused");
            let mut a = vec!["gen", "member_shipping_fee.rule", "--out", "out"];
            a.extend_from_slice(args);
            let (code, out, err) = rulec_in(t.path(), lang, &a);
            assert_eq!((code, out.as_str(), err.as_str()), (2, "", format!("error: {says}\n").as_str()), "{lang}: {args:?}");
            assert!(!t.path().join("out").exists(), "{lang}: {args:?} wrote before it stopped");
        }
    }
}

/// Over what every target and extra wrote, a run that asks for Python and Go with their runners
/// writes nothing new, and names the files of the extras it does not ask for that rulec wrote for
/// its targets, a line each and one note, in its text and in `left`; it deletes none, and `--check`
/// stays 0. A file rulec did not write there, and the other targets' files, are not named.
#[test]
fn files_of_extras_not_asked_for_are_named_and_kept() {
    let left = [
        "out/python/member_shipping_fee_mcp.py",
        "out/python/member_shipping_fee_service.py",
        "out/python/member_shipping_fee_connect_runner.py",
        "out/python/member_shipping_fee_page.html",
        "out/go/membershippingfee/round_test.go",
    ];
    for (lang, line, note) in [
        (
            "en",
            "generated no more, and still there: ",
            "note: `--extra mcp`, `--extra connect` and `--extra round-test` write these files, and this run does not ask for them; gen deletes nothing in the output directory, so they can be deleted if they are not wanted",
        ),
        (
            "ja",
            "もう生成しないファイルが残っています: ",
            "注記: これらは `--extra mcp`・`--extra connect`・`--extra round-test` が書くファイルで、この実行は頼んでいません。gen は出力先のファイルを消さないので、要らなければ消してかまいません",
        ),
    ] {
        // the files in the language of the run, whose prose the libraries carry
        let t = with_rule("targets-left");
        let (code, out, err) = rulec_in(t.path(), lang, &rulec::targets::up_to_0_40(&["gen", "member_shipping_fee.rule", "--out", "out"]));
        assert_eq!(code, 0, "{lang}: {out}{err}");
        // a file of the reader's own where the round test of Python would be
        std::fs::write(t.path().join("out/python/_round_test.py"), "print('mine')\n").unwrap();
        let before = files(&t.path().join("out"));
        let mut a = vec!["gen", "member_shipping_fee.rule", "--out", "out", "--target", "python", "--target", "go", "--extra", "runner"];
        let (code, out, err) = rulec_in(t.path(), lang, &a);
        assert_eq!(code, 0, "{lang}: {out}{err}");
        let mut want: Vec<String> = left.iter().map(|p| format!("{line}{p}")).collect();
        want.push(note.to_string());
        let said: Vec<&str> = out.lines().chain(err.lines()).collect();
        assert_eq!(said, want, "{lang}");
        assert_eq!(files(&t.path().join("out")), before, "{lang}: a file changed or went");
        a.extend(["--check", "--format", "json"]);
        let (code, out, err) = rulec_in(t.path(), lang, &a);
        assert_eq!(code, 0, "{lang}: --check counts what is left: {out}{err}");
        let j = rulec::json::parse(out.trim()).unwrap_or_else(|e| panic!("{lang}: {e}: {out}"));
        let got: Vec<&str> = j.get("left").and_then(|l| l.as_arr()).unwrap().iter().map(|p| p.as_str().unwrap()).collect();
        assert_eq!(got, left, "{lang}");
        for k in ["written", "stale", "missing"] {
            assert_eq!(j.get(k).and_then(|l| l.as_arr()).map(<[_]>::len), Some(0), "{lang}: {k}: {out}");
        }
    }
}

/// `rulec test` runs what was generated: a language with its runner, its vectors through it; a
/// language without one is skipped, which `--require-all` counts as a failure, since it cannot be
/// held to the reference evaluator; a part that was not generated (MCP, Connect, the round tests)
/// is a note, not a failure. Without vectors, or with no language's code, it stops at 2 and names
/// the extras that write what it needs.
#[test]
fn rulec_test_runs_what_was_generated() {
    let t = with_rule("targets-test");
    let make = |out: &str, args: &[&str]| {
        let mut a = vec!["gen", "member_shipping_fee.rule", "--out", out];
        a.extend_from_slice(args);
        let (code, said, err) = rulec_in(t.path(), "en", &a);
        assert_eq!(code, 0, "{said}{err}");
    };
    make("bare", &["--target", "python"]);
    let (code, _, err) = rulec_in(t.path(), "en", &["test", "bare"]);
    assert_eq!((code, err.as_str()), (2, "error: no vectors in `bare/vectors`; run `rulec gen <file.rule>... --target <target> --extra runner --extra vectors --out bare` first\n"));
    let (code, _, err) = rulec_in(t.path(), "ja", &["test", "bare"]);
    assert_eq!((code, err.as_str()), (2, "error: `bare/vectors` にベクタがありません。先に `rulec gen <file.rule>... --target <出力先> --extra runner --extra vectors --out bare` を実行してください\n"));
    make("vectors-only", &["--target", "python", "--extra", "vectors"]);
    std::fs::remove_dir_all(t.path().join("vectors-only/python")).unwrap();
    let (code, _, err) = rulec_in(t.path(), "en", &["test", "vectors-only"]);
    assert_eq!((code, err.as_str()), (2, "error: no language's code is in `vectors-only`; run `rulec gen <file.rule>... --target <target> --extra runner --extra vectors --out vectors-only` first\n"));

    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "no python3; the generated Python is not run") {
        return;
    }
    let notes = [
        "MCP: the servers were not generated, so none was run (`rulec gen --extra mcp` writes them)",
        "Connect: the service and its runner were not generated, so it was not run (`rulec gen --target python --target proto --extra connect --extra runner` writes them)",
        "the round tests were not generated, so the rounding helpers were not run (`rulec gen --extra round-test` writes them)",
    ];
    let strings = |j: &rulec::json::Json, k: &str| -> Vec<String> { j.get(k).and_then(|v| v.as_arr()).unwrap_or_default().iter().map(|s| s.as_str().unwrap().to_string()).collect() };

    // without its runner: skipped, a failure under --require-all
    make("no-runner", &["--target", "python", "--extra", "vectors"]);
    let (code, out, err) = rulec_in(t.path(), "en", &["test", "no-runner", "--format", "json"]);
    assert_eq!(code, 0, "{out}{err}");
    let j = rulec::json::parse(out.lines().next().unwrap_or_default()).unwrap_or_else(|e| panic!("{e}: {out}"));
    assert_eq!(strings(&j, "skipped"), ["Python was generated without its runner, so no vector was run through it (member_shipping_fee; `rulec gen --extra runner` writes it)"]);
    assert_eq!(strings(&j, "without"), notes);
    assert_eq!(j.get("results").and_then(|r| r.as_arr()).map(<[_]>::len), Some(0));
    let (code, out, err) = rulec_in(t.path(), "en", &["test", "no-runner", "--require-all"]);
    assert_eq!(code, 1, "{out}{err}");
    let (_, out, err) = rulec_in(t.path(), "ja", &["test", "no-runner"]);
    assert!(
        format!("{out}{err}").contains("Python はランナーなしで生成されているので、ベクタを走らせていません（member_shipping_fee。ランナーは `rulec gen --extra runner` が書きます）"),
        "{out}{err}"
    );

    // with its runner: every vector through it, and the parts not generated are notes
    make("runner", &["--target", "python", "--extra", "runner", "--extra", "vectors"]);
    let (code, out, err) = rulec_in(t.path(), "en", &["test", "runner", "--require-all", "--format", "json"]);
    assert_eq!(code, 0, "{out}{err}");
    let j = rulec::json::parse(out.lines().next().unwrap_or_default()).unwrap_or_else(|e| panic!("{e}: {out}"));
    assert_eq!(strings(&j, "skipped"), Vec::<String>::new());
    assert_eq!(strings(&j, "without"), notes);
    let results = j.get("results").and_then(|r| r.as_arr()).unwrap();
    assert_eq!(results.len(), 1, "{out}");
    let (_, out, _) = rulec_in(t.path(), "en", &["test", "runner"]);
    assert!(out.contains("ok    member_shipping_fee (Python) 70 vectors, 16 refused"), "{out}");
}

/// The MCP tool takes the targets as an array of the names `--target` takes, at least one, and
/// the extras as an array of theirs; a call reaches the command with them.
#[test]
fn the_mcp_tool_takes_the_targets_as_an_array() {
    let t = with_rule("targets-mcp");
    let out = t.path().join("out");
    let call = |id: u32, arguments: String| format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"rulec_gen","arguments":{arguments}}}}}"#);
    let rule = t.path().join("member_shipping_fee.rule");
    let requests = [
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#.to_string(),
        call(2, format!(r#"{{"files":[{}],"target":["python","go"],"extra":["runner"],"out":{}}}"#, json_str(&rule), json_str(&out))),
        call(3, format!(r#"{{"files":[{}],"out":{}}}"#, json_str(&rule), json_str(&t.path().join("none")))),
    ];
    let mut child = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .env_remove("RITSU_LANG")
        .current_dir(t.path())
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("cannot start rulec mcp");
    {
        let mut stdin = child.stdin.take().unwrap();
        for r in &requests {
            writeln!(stdin, "{r}").unwrap();
        }
    }
    let answers: Vec<rulec::json::Json> = BufReader::new(child.stdout.take().unwrap())
        .lines()
        .map(|l| l.unwrap())
        .filter(|l| !l.trim().is_empty())
        .map(|l| rulec::json::parse(&l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .collect();
    assert!(child.wait().unwrap().success());
    let tools = answers[0].get("result").and_then(|r| r.get("tools")).and_then(|t| t.as_arr()).unwrap();
    let tool = tools.iter().find(|t| t.get("name").and_then(|n| n.as_str()) == Some("rulec_gen")).expect("rulec_gen");
    let schema = tool.get("inputSchema").unwrap();
    let props = schema.get("properties").unwrap();
    let names = |p: &str| -> Vec<String> { props.get(p).and_then(|x| x.get("items")).and_then(|i| i.get("enum")).and_then(|e| e.as_arr()).unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect() };
    let target = props.get("target").unwrap();
    assert_eq!(target.get("type").and_then(|x| x.as_str()), Some("array"));
    assert_eq!(names("target"), rulec::targets::target_names());
    assert_eq!(target.get("minItems").and_then(|x| x.as_int()), Some(1));
    assert_eq!(names("extra"), rulec::targets::EXTRAS);
    let required: Vec<&str> = schema.get("required").and_then(|r| r.as_arr()).unwrap().iter().map(|s| s.as_str().unwrap()).collect();
    assert_eq!(required, ["files", "target"]);

    let text = |a: &rulec::json::Json, i: usize| -> String { a.get("result").and_then(|r| r.get("content")).and_then(|c| c.as_arr()).unwrap()[i].get("text").and_then(|x| x.as_str()).unwrap().to_string() };
    assert_eq!(text(&answers[1], 1), "exit code 0");
    let wrote: Vec<String> = files(&out).into_keys().collect();
    assert_eq!(wrote, ["go/membershippingfee/go.mod", "go/membershippingfee/member_shipping_fee.go", "go/membershippingfeerunner/go.mod", "go/membershippingfeerunner/main.go", "python/member_shipping_fee.py", "python/member_shipping_fee_runner.py"]);
    assert_eq!(text(&answers[2], 1), "exit code 2");
    assert!(text(&answers[2], 0).starts_with("error: `rulec gen` writes only the targets it is given"), "{}", text(&answers[2], 0));
    assert!(!t.path().join("none").exists());
}

/// A path as a JSON string.
fn json_str(p: &Path) -> String {
    format!("\"{}\"", p.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\""))
}

/// The page of `gen` names `--target` as given at least once and more than once if wanted, and
/// `--extra` as something that may be given more than once.
#[test]
fn the_page_of_gen_names_the_targets_and_the_extras() {
    for (lang, usage) in [
        ("en", "  rulec gen <file.rule>... --target <target>... [--extra <extra>...] [--out <dir>] [--check] [--format json]"),
        ("ja", "  rulec gen <file.rule>... --target <出力先>... [--extra <付属物>...] [--out <dir>] [--check] [--format json]"),
    ] {
        let (code, out, _) = rulec_in(&root(), lang, &["gen", "--help"]);
        assert_eq!(code, 0);
        assert!(out.lines().any(|l| l == usage), "{lang}:\n{out}");
        for name in rulec::targets::target_names().iter().chain(rulec::targets::EXTRAS) {
            assert!(out.contains(name), "{lang}: {name} is not on the page");
        }
    }
}

/// `--out generated/` is `--out generated`: every path `gen` says has one slash between the
/// directory and what is under it, in its text and in its JSON, and so has every path `vectors` and
/// `doc` say. Up to 0.40.0 they said `generated//python/…` of the same files (§15.277).
#[test]
fn a_slash_at_the_end_of_out_is_not_said_twice() {
    let t = with_rule("targets-slash");
    let rule = "member_shipping_fee.rule";
    let (code, out, err) = rulec_in(t.path(), "en", &["gen", rule, "--target", "python", "--extra", "runner", "--out", "out/"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.lines().any(|l| l == "generated: out/python/member_shipping_fee.py"), "{out}");
    assert!(!out.contains("//"), "{out}");
    // the runner is left there by a run that does not ask for it, and named by the same path
    let (code, out, err) = rulec_in(t.path(), "en", &["gen", rule, "--target", "python", "--out", "out/", "--check", "--format", "json"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains("//"), "{out}");
    let j = rulec::json::parse(out.trim()).unwrap_or_else(|e| panic!("{e}: {out}"));
    let left: Vec<&str> = j.get("left").and_then(|l| l.as_arr()).unwrap().iter().map(|p| p.as_str().unwrap()).collect();
    assert_eq!(left, ["out/python/member_shipping_fee_runner.py"], "{out}");
    for (args, said) in [
        (["vectors", rule, "--out", "vec/"], "70 vectors: vec/member_shipping_fee.jsonl"),
        (["doc", rule, "--out", "doc/"], "rendered: doc/member_shipping_fee.md"),
    ] {
        let (code, out, err) = rulec_in(t.path(), "en", &args);
        assert_eq!(code, 0, "{args:?}: {out}{err}");
        assert!(out.lines().any(|l| l == said), "{args:?}: {out}");
    }
}
