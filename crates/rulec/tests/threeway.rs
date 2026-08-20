//! 三者一致（§8.5、§9.3）。
//!
//! 参照評価器・生成 Python・生成 Go の三つに同じベクタを流し、正準 JSON の
//! バイト一致を見る。データも旧実装も要らないので、これが M1 の主力の保証になる。
//!
//! python3 と go が無い環境では、その言語だけ飛ばす（飛ばしたことは必ず言う）。

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str) -> bool {
    // go は `go version`、python3 は `python3 --version`。両方試す。
    // 片方だけ試して黙って飛ばすと、動いていないのに緑になる。
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

const CORPUS: &[(&str, &str)] = &[
    ("tests/corpus/ゆうパック運賃.rule", "yupack_fee"),
    ("tests/corpus/クーポン割引.rule", "coupon_discount"),
    ("tests/corpus/クーポン併用.rule", "coupon_stack"),
    ("tests/corpus/送料.rule", "shipping_fee"),
    ("tests/corpus/期間区分.rule", "period"),
    ("tests/corpus/適用順序.rule", "apply_order"),
];

#[test]
fn 評価器と生成コードが三者一致する() {
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
    assert!(py || go, "python3 も go も無いので三者一致を確かめられない");
    if !py {
        eprintln!("注意: python3 が無いので Python 側を飛ばした");
    }
    if !go {
        eprintln!("注意: go が無いので Go 側を飛ばした");
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
    eprintln!("三者一致: 規則 {} 本 / ベクタ {total} 件（Python {py} / Go {go}）", CORPUS.len());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 生成物は決定的である() {
    // §8.5: 同じ .rule と同じ rulec からはバイト同一。ハッシュ順の混入は不合格。
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
    // §8.1 基準 5。gofmt を後段で走らせずに、生成の時点で整形済みにする
    // （後段で走らせると生成物が環境依存になり、§8.5 の決定性が壊れる）。
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

    // Python は少なくとも構文が通ること（black や flake8 は環境に無くてもよい）。
    for (_, alias) in CORPUS {
        let p = dir.join("python").join(format!("{alias}.py"));
        let o = Command::new("python3")
            .args(["-c", "import ast,sys; ast.parse(open(sys.argv[1],encoding='utf-8').read())"])
            .arg(&p)
            .output()
            .expect("python3 を起動できない");
        assert!(o.status.success(), "{alias}.py が構文エラー: {}", String::from_utf8_lossy(&o.stderr));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 丸めヘルパは両言語で参照実装と一致する() {
    // §8.5: 表レベルの一致だけでは、端数の出ない表でヘルパの誤りが隠れる。
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
