//! The generated code is silent under each language's strict checker (§15.340 of DESIGN.md).
//!
//! A rule may work out a value and read nothing of it: a definition, a derived value, a summary, a
//! fold's answer, a column of a table, an input, an output of a rule it applies. The generated code
//! still works it out, so that each line of the rule stays a line of the code, and reads it once on
//! purpose where the language says of a value never read: `_ = x` in Go, Ruby and Swift, `let _ = x;`
//! in Rust, `void x;` in TypeScript. A column every row of a table below tests with `-` is such a
//! value: the table names it and tests nothing of it, and Go did not build such a rule. So is an
//! element of a walk whose tables test each of its fields with `-`: the walk names it and reads
//! none of it, and neither Go, Rust nor Swift built such a rule. A fold
//! declares the flags and the values only its arms set: one no arm sets is no `mut` in Rust and a
//! `let` in Swift, and the key of a `keep_max` is there only with one. Python declares a column of
//! an enum or a truth value whose first row hands on a name, as mypy narrows that name to the one
//! value its row tests. Every file of Rust rulec writes allows `dead_code` and clippy's lints at
//! its head: the helpers every module holds are not each called, the shapes are the ones the proofs
//! read, and a lint a later clippy adds is no stop to a crate built with `-D warnings`. The Go is
//! what gofmt writes (§8.1): `gofmt -l` names no file of it.
//!
//! The materials are in `tests/unread`, English first, with a Japanese version beside each: three
//! rules that apply `shared/authority.rule` or `shared/fee.rule` and read one output of it, one with
//! a value of each kind nothing reads, one with a fold whose answer nothing reads, one whose tables
//! hand on, in their first rows, the enum and the truth value they test, and one whose walk reads no
//! field of its elements. Beside them, the
//! shapes of other tests the checkers found: the column only `-` tests (`above_never`), folds with
//! some arms (`corpus`, `stages`) and a column of an enum handed on in its first row (`strength`).
//! The last test reads every rule of the tree that passes `check`.

use ritsu_testkit::run::finish;
use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, need, ready, skip};
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Each rule, by its path from the crate, beside its Japanese version.
const MATERIALS: [(&str, &str); 13] = [
    ("tests/unread/who_decides.rule", "tests/unread/購入を決める人.rule"),
    ("tests/unread/who_heads.rule", "tests/unread/決める人の格.rule"),
    ("tests/unread/review_only.rule", "tests/unread/確かめだけを読む.rule"),
    ("tests/unread/unread_kinds.rule", "tests/unread/読まない値の種類.rule"),
    ("tests/unread/unread_fold.rule", "tests/unread/読まない畳み込み.rule"),
    ("tests/unread/handed_on.rule", "tests/unread/確かめた値を渡す.rule"),
    ("tests/unread/unread_element.rule", "tests/unread/読まない要素.rule"),
    ("tests/above_never/flag_first.rule", "tests/above_never/対象と段階の値引き.rule"),
    ("tests/above_never/optional_lift.rule", "tests/above_never/値が無いことのある段階.rule"),
    ("tests/corpus/case_routing.rule", "tests/corpus/案件の振り分け.rule"),
    ("tests/stages/size_to_ship.rule", "tests/stages/送るサイズ.rule"),
    ("tests/stages/best_offer.rule", "tests/stages/最高の申し出.rule"),
    ("tests/strength/door.rule", "tests/strength/door.ja.rule"),
];

/// The languages whose strict checkers the tests hold the code to. tests/threeway.rs holds every
/// rule of the tests to tsc, with the flags of unused locals and parameters.
const TARGETS: [&str; 7] = ["python", "rust", "wasm", "go", "swift", "ruby", "java"];

/// How long a run of rulec may take.
const RULEC_LIMIT: std::time::Duration = std::time::Duration::from_secs(20 * 60);

fn rulec(args: &[&str]) -> (i32, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rulec"));
    c.env("RULEC_LANG", "en").current_dir(root()).args(args);
    let o = finish(&mut c, RULEC_LIMIT, &format!("rulec {}", args.join(" ")));
    (o.code.unwrap_or(-1), o.stdout + &o.stderr)
}

/// Whether a command answers. swiftc leaves an empty directory in its TMPDIR almost every time it
/// answers `--version`, so the probe gets a TMPDIR of its own, which goes when this returns.
fn have(cmd: &str, args: &[&str]) -> bool {
    let tmp = TempDir::new("unread-probe");
    Command::new(cmd).args(args).env("TMPDIR", tmp.path()).output().map(|o| o.status.success()).unwrap_or(false)
}

/// `mypy`, installed or through `uvx`, the way tests/threeway.rs finds it.
fn mypy() -> Option<Vec<String>> {
    if have("mypy", &["--version"]) {
        return Some(vec!["mypy".into()]);
    }
    have("uvx", &["mypy", "--version"]).then(|| vec!["uvx".into(), "mypy".into()])
}

/// The checkers that are here, each asked once: a SKIP line for one that is not.
struct Checkers {
    mypy: Option<Vec<String>>,
    rustc: bool,
    clippy: bool,
    go: bool,
    swift: bool,
    ruby: bool,
    java: bool,
}

impl Checkers {
    fn here() -> Checkers {
        let mypy = if need(Need::Python) {
            let my = mypy();
            if my.is_none() {
                skip("mypy is not here, nor uvx; the Python is not held to mypy --strict");
            }
            my
        } else {
            None
        };
        let rustc = ready(Need::Rustc, || have("rustc", &["--version"]), "rustc is not here; the Rust is not built with -D warnings");
        let clippy = rustc && have("clippy-driver", &["--version"]);
        if rustc && !clippy {
            skip("clippy-driver is not here (rustup component add clippy); the Rust is not held to clippy's lints");
        }
        Checkers {
            mypy,
            rustc,
            clippy,
            go: ready(Need::Go, || have("go", &["version"]), "go is not here; the Go is not vetted"),
            swift: ready(Need::Swift, || have("swiftc", &["--version"]), "swiftc is not here; the Swift is not checked with -warnings-as-errors"),
            ruby: ready(Need::Ruby, || have("ruby", &["--version"]), "ruby is not here; the Ruby is not read with -w"),
            java: ready(Need::Java, || have("javac", &["-version"]), "javac is not here; the Java is not compiled with -Xlint:all -Werror"),
        }
    }
}

/// The files of `dir` with the extension `ext`, by name, but the ones a file name marks as not a
/// module of a rule or of an `enums` file (the runners and their like are not written here).
fn files(dir: &Path, ext: &str) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(ext) && !n.starts_with('_')).collect())
        .unwrap_or_default();
    out.sort();
    out
}

/// What each strict checker that is here says of the modules generated into `out`: an empty list
/// where each is silent. Rust builds the modules as one crate, whose root allows the lints of the
/// names' letters as a package's does.
fn strictly(out: &Path, ck: &Checkers) -> Vec<String> {
    let mut said = Vec::new();
    let mut ran = |what: String, c: &mut Command| {
        let o = c.output().unwrap_or_else(|e| panic!("cannot start {what}: {e}"));
        let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        let quiet = text.lines().all(|l| l.trim().is_empty() || l.trim() == "Syntax OK" || l.starts_with("Success: no issues found"));
        if !o.status.success() || !quiet {
            said.push(format!("{what}:\n{text}"));
        }
    };
    let tmp = tmpdir_in(out);
    let py = files(&out.join("python"), ".py");
    if let (Some(my), false) = (&ck.mypy, py.is_empty()) {
        ran("mypy --strict".into(), Command::new(&my[0]).args(&my[1..]).args(["--strict", "--no-color-output", "--no-incremental", "--cache-dir"]).arg(tmp.join("mypy")).args(&py).current_dir(out.join("python")));
    }
    let rs = files(&out.join("rust"), ".rs");
    let tools: Vec<&str> = ["rustc", "clippy-driver"].into_iter().filter(|t| *t == "rustc" || ck.clippy).collect();
    if ck.rustc && !rs.is_empty() {
        // The modules of a rule and of the `enums` files it reads are one crate, as a package
        // holds them: a rule's module reads an enum's type from `crate::<enums file>`.
        let mods: String = rs.iter().map(|f| format!("#[path = \"{f}\"]\npub mod {};\n", f.trim_end_matches(".rs"))).collect();
        std::fs::write(out.join("rust/_crate.rs"), format!("#![allow(confusable_idents, mixed_script_confusables, uncommon_codepoints)]\n{mods}")).unwrap();
        for tool in &tools {
            ran(format!("{tool} -D warnings"), Command::new(tool).args(["--edition", "2021", "--crate-type", "lib", "-D", "warnings", "--emit=metadata", "--out-dir"]).arg(tmp.join(tool)).arg("_crate.rs").current_dir(out.join("rust")));
        }
    }
    // The WASI module of each rule is a crate of its own, which holds the rule's module.
    for w in files(&out.join("wasm"), "_wasm.rs") {
        for tool in tools.iter().filter(|_| ck.rustc) {
            ran(format!("{tool} -D warnings ({w})"), Command::new(tool).args(["--edition", "2021", "--crate-type", "lib", "-D", "warnings", "--emit=metadata", "--out-dir"]).arg(tmp.join(format!("wasm-{tool}"))).arg(&w).current_dir(out.join("wasm")));
        }
    }
    if ck.go {
        for pkg in std::fs::read_dir(out.join("go")).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.join("go.mod").exists()).collect::<Vec<_>>()).unwrap_or_default() {
            ran(format!("go vet ({})", pkg.display()), Command::new("go").args(["vet", "./..."]).env("GOPROXY", "off").env("GOFLAGS", "-mod=mod").current_dir(&pkg));
        }
        // gofmt names each file it would write otherwise (§8.1).
        if out.join("go").is_dir() {
            ran("gofmt -l".into(), Command::new("gofmt").arg("-l").arg(out.join("go")));
        }
    }
    // A module of Swift is one list of files, which share one scope: each rule's file is built
    // with the modules of the `enums` files it reads, as `rulec test` builds it (the vectors name
    // them, §15.212), and a rule that declares those enums itself is built without them
    // (§15.326). The module of an `enums` file no rule here is built with is built on its own.
    let sw = files(&out.join("swift"), ".swift");
    if ck.swift && !sw.is_empty() {
        let vectors = out.join("vectors");
        let mut lists: Vec<Vec<String>> = Vec::new();
        for f in &sw {
            let alias = f.trim_end_matches(".swift");
            if !vectors.join(format!("{alias}.jsonl")).exists() {
                continue;
            }
            let owners = std::fs::read_to_string(vectors.join(format!("{alias}.owners.json"))).ok().and_then(|t| rulec::json::parse(t.trim()).ok());
            let mut list = vec![f.clone()];
            if let Some(rulec::json::Json::Arr(ms)) = owners {
                list.extend(ms.iter().filter_map(|m| m.as_str()).map(|m| format!("{m}.swift")));
            }
            lists.push(list);
        }
        let built: Vec<String> = lists.iter().flatten().cloned().collect();
        lists.extend(sw.iter().filter(|f| !built.contains(f)).map(|f| vec![f.clone()]));
        for list in lists {
            ran(format!("swiftc -warnings-as-errors ({})", list.join(" ")), Command::new("swiftc").args(["-typecheck", "-warnings-as-errors"]).args(&list).env("TMPDIR", &tmp).current_dir(out.join("swift")));
        }
    }
    if ck.ruby {
        for rb in files(&out.join("ruby"), ".rb") {
            ran(format!("ruby -wc {rb}"), Command::new("ruby").arg("-wc").arg(&rb).current_dir(out.join("ruby")));
        }
    }
    let java = files(&out.join("java"), ".java");
    if ck.java && !java.is_empty() {
        ran("javac -Xlint:all -Werror".into(), Command::new("javac").args(["-Xlint:all", "-Werror", "-d"]).arg(tmp.join("classes")).args(&java).current_dir(out.join("java")));
    }
    said
}

/// `rulec gen` of `rule` into `out` for the languages of [`TARGETS`] that W121 does not name, with
/// the vectors, which name the modules a rule's Swift is built with: a language E153 stops for the
/// rule is left out, as a caller who reads the error leaves it out. None where `gen` stops for
/// another reason.
fn generate(rule: &str, out: &Path) -> Option<Vec<String>> {
    let mut targets: Vec<&str> = TARGETS.to_vec();
    let mut left_out = Vec::new();
    for _ in 0..TARGETS.len() {
        let _ = std::fs::remove_dir_all(out);
        let mut args = vec!["gen", rule, "--extra", "vectors", "--out"];
        let o = out.to_string_lossy().into_owned();
        args.push(&o);
        for t in &targets {
            args.extend(["--target", t]);
        }
        let (code, said) = rulec(&args);
        if code == 0 {
            return Some(left_out);
        }
        let named: Vec<String> = said
            .lines()
            .filter(|l| l.contains("E153"))
            .flat_map(|l| l.split("`--target ").skip(1).map(|r| r.split('`').next().unwrap_or_default().to_string()).collect::<Vec<_>>())
            .collect();
        if named.is_empty() {
            return None;
        }
        targets.retain(|t| !named.iter().any(|n| n == t));
        left_out.extend(named);
    }
    None
}

/// Every material, the English rules and the Japanese versions, is silent under each strict
/// checker that is here, and the new rules answer their vectors in every language as the reference
/// evaluator does.
#[test]
fn every_material_is_silent_under_each_strict_checker() {
    if !need(Need::Rustc) {
        return;
    }
    let ck = Checkers::here();
    let mut failures = Vec::new();
    for (en, ja) in MATERIALS {
        for rule in [en, ja] {
            let t = TempDir::new("unread");
            let out = t.path().join("out");
            let left_out = generate(rule, &out).unwrap_or_else(|| panic!("{rule}: gen stops"));
            assert!(left_out.is_empty(), "{rule}: E153 stops {left_out:?}");
            failures.extend(strictly(&out, &ck).into_iter().map(|s| format!("{rule}: {s}")));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    if !ready(Need::Python, || have("python3", &["--version"]), "python3 is not here; the rules were not run") {
        return;
    }
    for rules in [MATERIALS[..7].iter().map(|m| m.0).collect::<Vec<_>>(), MATERIALS[..7].iter().map(|m| m.1).collect()] {
        let t = TempDir::new("unread-run");
        let out = t.path().join("out");
        let o = out.to_string_lossy().into_owned();
        let mut args = vec!["gen"];
        args.extend(&rules);
        args.extend(["--out", &o]);
        let (code, said) = rulec(&rulec::targets::up_to_0_40(&args));
        assert_eq!(code, 0, "{said}");
        let (_, said) = rulec(&["test", &o, "--format", "json"]);
        let j = rulec::json::parse(said.lines().next().unwrap_or_default()).unwrap_or_else(|e| panic!("{e}: {said}"));
        for why in j.get("skipped").and_then(|v| v.as_arr()).unwrap_or_default() {
            skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
        }
        let results = j.get("results").and_then(|v| v.as_arr()).unwrap_or_default();
        assert!(results.iter().filter(|r| r.get("ran").and_then(|v| v.as_bool()) == Some(true)).count() >= rules.len() * 2, "{said}");
        for r in results {
            assert_ne!(r.get("ok").and_then(|v| v.as_bool()), Some(false), "{r:?}");
        }
    }
}

/// Every file of Rust rulec writes allows `dead_code` and clippy's lints at its head: the module of
/// a rule and of an `enums` file, the runners, the proof, the Wasm module and the round tests.
#[test]
fn every_file_of_rust_allows_dead_code_and_clippy_at_its_head() {
    let t = TempDir::new("unread-heads");
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    let (code, said) = rulec(&rulec::targets::up_to_0_40(&["gen", "tests/unread/unread_kinds.rule", "--out", &o]));
    assert_eq!(code, 0, "{said}");
    let mut seen = 0;
    for dir in ["rust", "wasm", "proof", "round"] {
        for f in std::fs::read_dir(out.join(dir)).map(|rd| rd.flatten().map(|e| e.path()).collect::<Vec<_>>()).unwrap_or_default() {
            if f.extension().is_none_or(|x| x != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&f).unwrap();
            let head = text.lines().find(|l| l.starts_with("#![allow(")).unwrap_or_else(|| panic!("{} allows nothing at its head", f.display()));
            assert!(head.contains("dead_code") && head.contains("clippy::all"), "{}: {head}", f.display());
            seen += 1;
        }
    }
    assert!(seen >= 4, "fewer files of Rust than there are kinds: {seen}");
}

/// Every `.rule` under the workspace's crates and site, by its path from the workspace's root.
fn tree_rules() -> Vec<String> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                if !matches!(name.as_str(), "target" | "node_modules" | ".venv" | ".lake" | ".git") {
                    walk(&p, base, out);
                }
            } else if name.ends_with(".rule") {
                out.push(p.strip_prefix(base).unwrap().to_string_lossy().into_owned());
            }
        }
    }
    let base = root().join("../..").canonicalize().unwrap();
    let mut out = Vec::new();
    for d in ["crates", "website"] {
        walk(&base.join(d), &base, &mut out);
    }
    out.sort();
    out
}

/// Every rule of the tree that passes `check` is silent under each strict checker that is here,
/// in each language of [`TARGETS`] W121 does not name for it. The rules are generated one by one,
/// as many at once as half the machine's cores.
#[test]
fn every_rule_of_the_tree_is_silent_under_each_strict_checker() {
    if !need(Need::Rustc) {
        return;
    }
    let ck = Checkers::here();
    let base = root().join("../..").canonicalize().unwrap();
    let rules: Vec<String> = tree_rules()
        .into_iter()
        .filter(|rel| {
            let path = base.join(rel);
            let Ok(src) = std::fs::read_to_string(&path) else { return false };
            !rulec::i18n::with(rulec::i18n::Lang::En, || rulec::check_source(&src, &path.to_string_lossy())).iter().any(|d| d.severity == rulec::diag::Severity::Error)
        })
        .collect();
    assert!(rules.len() >= 600, "fewer rules of the tree pass check than there were: {}", rules.len());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let failures = std::sync::Mutex::new(Vec::new());
    let ways = std::thread::available_parallelism().map(|n| (n.get() / 2).max(1)).unwrap_or(2);
    std::thread::scope(|s| {
        for _ in 0..ways {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(rel) = rules.get(i) else { break };
                let t = TempDir::new("unread-tree");
                let out = t.path().join("out");
                let path = base.join(rel).to_string_lossy().into_owned();
                let Some(_) = generate(&path, &out) else { continue };
                let said = strictly(&out, &ck);
                failures.lock().unwrap().extend(said.into_iter().map(|s| format!("{rel}: {s}")));
            });
        }
    });
    let failures = failures.into_inner().unwrap();
    assert!(failures.is_empty(), "{} of {} rules:\n{}", failures.len(), rules.len(), failures.join("\n"));
    println!("unread: {} rules of the tree are silent under each strict checker here", rules.len());
}
