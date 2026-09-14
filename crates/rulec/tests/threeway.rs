//! Agreement across every implementation (§8.5, §9.3).
//!
//! Feed the same vectors to all of them — the reference evaluator and the generated Python,
//! TypeScript, Rust, Ruby and Go — and check that the canonical JSON matches byte
//! for byte. It needs neither data nor a legacy implementation, which makes it the main
//! guarantee of M1.
//!
//! Where a toolchain is missing, only that language is skipped, and the skip is always
//! reported — a silent skip would be a green run that proved nothing.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str) -> bool {
    // go takes `go version`, python3 takes `python3 --version`. Try both.
    // Trying only one and skipping silently would go green without anything having run.
    ["--version", "version"]
        .iter()
        .any(|a| Command::new(cmd).arg(a).output().map(|o| o.status.success()).unwrap_or(false))
}

fn rulec(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(args)
        .output()
        .expect("rulec を起動できない");
    assert!(out.status.success(), "rulec {args:?} が失敗: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Every rule in the corpus, with the name its generated files take. Hand-written lists rot:
/// two rules had fallen out of this one, and one of them was returning a number a hundred
/// times too large in all four languages while the suite stayed green. `コーパスは全部載っている`
/// keeps the list honest.
const CORPUS: &[(&str, &str)] = &[
    ("tests/corpus/ゆうパック運賃.rule", "yupack_fee"),
    ("tests/corpus/クーポン割引.rule", "coupon_discount"),
    ("tests/corpus/クーポン併用.rule", "coupon_stack"),
    ("tests/corpus/送料.rule", "shipping_fee"),
    ("tests/corpus/期間区分.rule", "period"),
    ("tests/corpus/適用順序.rule", "apply_order"),
    ("tests/corpus/クーポン一枚.rule", "coupon_step"),
    ("tests/corpus/決済手数料.rule", "payment_fee"),
    ("tests/corpus/ポイント付与.rule", "points"),
    ("tests/corpus/評価ランク.rule", "rank"),
    ("tests/corpus/値引の充当.rule", "allocate"),
    ("tests/corpus/会員特典.rule", "member_perk"),
    ("tests/corpus/ec261.rule", "ec261"),
];

#[test]
fn コーパスは全部載っている() {
    let dir = root().join("tests/corpus");
    let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rule"))
        .map(|p| format!("tests/corpus/{}", p.file_name().unwrap().to_string_lossy()))
        .collect();
    on_disk.sort();
    let mut listed: Vec<String> = CORPUS.iter().map(|(f, _)| (*f).to_string()).collect();
    listed.sort();
    assert_eq!(
        on_disk, listed,
        "コーパスに足した規則が一致検査の一覧に載っていません。tests/threeway.rs の CORPUS に足してください"
    );
}

#[test]
fn 評価器と生成コードが全言語で一致する() {
    let dir = std::env::temp_dir().join(format!("rulec-3way-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.to_string_lossy().to_string();

    let files: Vec<&str> = CORPUS.iter().map(|(f, _)| *f).collect();
    let mut args = vec!["gen"];
    args.extend(files.iter().copied());
    args.push("--out");
    args.push(&out);
    rulec(&args);

    let py = have("python3");
    let go = have("go");
    let ts = have("node");
    let rs = have("rustc");
    let rb = have("ruby");
    assert!(py || go || ts || rs || rb, "どの toolchain も無いので一致を確かめられない");
    for (ok, name) in [(py, "python3"), (ts, "node"), (rs, "rustc"), (rb, "ruby"), (go, "go")] {
        if !ok {
            eprintln!("注意: {name} が無いのでその言語を飛ばした");
        }
    }

    let mut total = 0usize;
    for (_, alias) in CORPUS {
        let vec_path = dir.join("vectors").join(format!("{alias}.jsonl"));
        let exp = std::fs::read_to_string(dir.join("vectors").join(format!("{alias}.expected.jsonl")))
            .expect("期待値が無い");
        let vectors = std::fs::read_to_string(&vec_path).expect("ベクタが無い");
        assert!(!vectors.trim().is_empty(), "{alias}: ベクタがゼロ件");
        total += vectors.lines().count();

        if py {
            let o = Command::new("python3")
                .current_dir(dir.join("python"))
                .arg(format!("{alias}_runner.py"))
                .stdin(std::fs::File::open(&vec_path).unwrap())
                .output()
                .expect("python3 を起動できない");
            let got = String::from_utf8_lossy(&o.stdout).into_owned();
            assert!(o.status.success(), "{alias}: Python が落ちた: {}", String::from_utf8_lossy(&o.stderr));
            assert_eq!(got, exp, "{alias}: 評価器と生成 Python が食い違う");
        }
        if ts {
            let o = Command::new("node")
                .current_dir(dir.join("typescript"))
                .args(["--no-warnings", &format!("{alias}_runner.ts")])
                .stdin(std::fs::File::open(&vec_path).unwrap())
                .output()
                .expect("node を起動できない");
            let got = String::from_utf8_lossy(&o.stdout).into_owned();
            assert!(o.status.success(), "{alias}: TypeScript が落ちた: {}", String::from_utf8_lossy(&o.stderr));
            assert_eq!(got, exp, "{alias}: 評価器と生成 TypeScript が食い違う");
        }
        if rs {
            let cwd = dir.join("rust");
            let built = Command::new("rustc")
                .current_dir(&cwd)
                .args(["--edition", "2021", "-O", &format!("{alias}_runner.rs"), "-o", alias])
                .output()
                .expect("rustc を起動できない");
            assert!(
                built.status.success(),
                "{alias}: 生成した Rust がコンパイルできない:\n{}",
                String::from_utf8_lossy(&built.stderr)
            );
            let o = Command::new(format!("./{alias}"))
                .current_dir(&cwd)
                .stdin(std::fs::File::open(&vec_path).unwrap())
                .output()
                .expect("生成した Rust を起動できない");
            let got = String::from_utf8_lossy(&o.stdout).into_owned();
            assert!(o.status.success(), "{alias}: Rust が落ちた: {}", String::from_utf8_lossy(&o.stderr));
            assert_eq!(got, exp, "{alias}: 評価器と生成 Rust が食い違う");
        }
        if rb {
            let o = Command::new("ruby")
                .current_dir(dir.join("ruby"))
                .arg(format!("{alias}_runner.rb"))
                .stdin(std::fs::File::open(&vec_path).unwrap())
                .output()
                .expect("ruby を起動できない");
            let got = String::from_utf8_lossy(&o.stdout).into_owned();
            assert!(o.status.success(), "{alias}: Ruby が落ちた: {}", String::from_utf8_lossy(&o.stderr));
            assert_eq!(got, exp, "{alias}: 評価器と生成 Ruby が食い違う");
        }
        if go {
            let pkg = alias.replace('_', "");
            let o = Command::new("go")
                .current_dir(dir.join("go").join(format!("{pkg}runner")))
                .args(["run", "."])
                .stdin(std::fs::File::open(&vec_path).unwrap())
                .output()
                .expect("go を起動できない");
            let got = String::from_utf8_lossy(&o.stdout).into_owned();
            assert!(o.status.success(), "{alias}: Go が落ちた: {}", String::from_utf8_lossy(&o.stderr));
            assert_eq!(got, exp, "{alias}: 評価器と生成 Go が食い違う");
        }
    }
    eprintln!(
        "一致: 規則 {} 本 / ベクタ {total} 件（Python {py} / TypeScript {ts} / Rust {rs} / Ruby {rb} / Go {go}）",
        CORPUS.len()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 生成物は決定的である() {
    // §8.5: the same .rule and the same rulec give byte-identical output. Any leakage of hash order
    // is a failure.
    let mk = |tag: &str| -> Vec<(String, String)> {
        let dir = std::env::temp_dir().join(format!("rulec-det-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let out = dir.to_string_lossy().to_string();
        let mut args = vec!["gen"];
        let files: Vec<&str> = CORPUS.iter().map(|(f, _)| *f).collect();
        args.extend(files.iter().copied());
        args.push("--out");
        args.push(&out);
        rulec(&args);
        let mut v: Vec<(String, String)> = Vec::new();
        collect(&dir, &dir, &mut v);
        v.sort();
        let _ = std::fs::remove_dir_all(&dir);
        v
    };
    assert_eq!(mk("a"), mk("b"), "二度生成して食い違った");
}

fn collect(base: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(base, &p, out);
        } else if let Ok(s) = std::fs::read_to_string(&p) {
            out.push((p.strip_prefix(base).unwrap().to_string_lossy().into(), s));
        }
    }
}

#[test]
fn 生成物は両言語の整形器に素で通る() {
    // §8.1 criterion 5. The output is formatted at generation time rather than by running gofmt
    // afterwards (running it afterwards makes the output environment-dependent and breaks the
    // determinism of §8.5).
    if !have("go") {
        eprintln!("注意: go が無いので gofmt の検査を飛ばした");
        return;
    }
    let dir = std::env::temp_dir().join(format!("rulec-fmt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.to_string_lossy().to_string();
    let mut args = vec!["gen"];
    let files: Vec<&str> = CORPUS.iter().map(|(f, _)| *f).collect();
    args.extend(files.iter().copied());
    args.push("--out");
    args.push(&out);
    rulec(&args);

    let go_dir = dir.join("go");
    let o = Command::new("gofmt")
        .arg("-l")
        .arg(&go_dir)
        .output()
        .expect("gofmt を起動できない");
    let listed = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(listed.trim().is_empty(), "gofmt が直したいファイルがある:\n{listed}");

    // Python must at least parse.
    for (_, alias) in CORPUS {
        let p = dir.join("python").join(format!("{alias}.py"));
        let o = Command::new("python3")
            .args(["-c", "import ast,sys; ast.parse(open(sys.argv[1],encoding='utf-8').read())"])
            .arg(&p)
            .output()
            .expect("python3 を起動できない");
        assert!(o.status.success(), "{alias}.py が構文エラー: {}", String::from_utf8_lossy(&o.stderr));
    }

    // The PEP 8 side (the counterpart of an empty `gofmt -l`). Unlike Go, Python has no single
    // formatter, so the claim is split in two and measured separately.
    let Some(ruff) = ruff() else {
        eprintln!("注意: ruff が無いので PEP 8 の検査を飛ばした");
        let _ = std::fs::remove_dir_all(&dir);
        return;
    };
    let py_dir = dir.join("python");

    // (1) The pycodestyle family (E/W) reports nothing, line length aside.
    let o = Command::new(&ruff[0])
        .args(&ruff[1..])
        .args(["check", "--select", "E,W", "--ignore", "E501", "--isolated", "--no-cache"])
        .arg(&py_dir)
        .output()
        .expect("ruff を起動できない");
    assert!(
        o.status.success(),
        "PEP 8（行長を除く）に違反がある:\n{}",
        String::from_utf8_lossy(&o.stdout)
    );

    // (2) The only lines the formatter wants to touch are lines longer than 88 columns.
    // §8.1 demands "one table row per line, with the source cells attached", and that does not fit
    // in 88 columns. A single complaint that is not about wrapping is something the generator
    // should fix, so it fails.
    let o = Command::new(&ruff[0])
        .args(&ruff[1..])
        .args(["format", "--check", "--diff", "--no-cache"])
        .arg(&py_dir)
        .output()
        .expect("ruff を起動できない");
    let diff = String::from_utf8_lossy(&o.stdout).into_owned();
    let short: Vec<&str> = diff
        .lines()
        .filter(|l| l.starts_with('-') && !l.starts_with("---"))
        .map(|l| &l[1..])
        .filter(|l| rulec::diag::width(l) <= 88)
        .collect();
    assert!(
        short.is_empty(),
        "整形器の指摘のうち、行長で説明できないものがある:\n{}",
        short.join("\n")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// How to launch ruff. Use it if it is on PATH, otherwise go through uvx. If neither exists, skip.
fn ruff() -> Option<Vec<String>> {
    if have("ruff") {
        return Some(vec!["ruff".into()]);
    }
    let ok = Command::new("uvx")
        .args(["ruff", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    ok.then(|| vec!["uvx".into(), "ruff".into()])
}

#[test]
fn 丸めヘルパは両言語で参照実装と一致する() {
    // §8.5: table-level agreement alone lets a helper bug hide in a table that yields no fractions.
    let dir = std::env::temp_dir().join(format!("rulec-round-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.to_string_lossy().to_string();
    rulec(&["gen", CORPUS[0].0, "--out", &out]);

    if have("python3") {
        let o = Command::new("python3")
            .current_dir(dir.join("python"))
            .arg("_round_test.py")
            .output()
            .expect("python3 を起動できない");
        assert!(o.status.success(), "Python の丸めが合わない: {}", String::from_utf8_lossy(&o.stdout));
    }
    if have("go") {
        let pkg = CORPUS[0].1.replace('_', "");
        let o = Command::new("go")
            .current_dir(dir.join("go").join(&pkg))
            .args(["test", "./..."])
            .output()
            .expect("go を起動できない");
        assert!(o.status.success(), "Go の丸めが合わない: {}", String::from_utf8_lossy(&o.stdout));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
