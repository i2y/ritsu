//! CLI の面（§11 原則 6）。CI に組むための約束なので、文面ではなく形を固定する。

use std::process::Command;

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn exit_code_は三値() {
    // 0 = 注記のみ
    let (c, _, _) = run(&["check", "tests/corpus/ゆうパック運賃.rule"]);
    assert_eq!(c, 0, "警告だけの規則は 0");

    // W114 は警告なので 0 のまま
    let (c, _, _) = run(&["check", "tests/corpus/クーポン併用.rule"]);
    assert_eq!(c, 0, "W114 は警告なので 0");

    // 1 = エラーあり
    let (c, _, _) = run(&["check", "tests/mutants/m_e101.rule"]);
    assert_eq!(c, 1, "エラーがあれば 1");

    // 2 = 内部異常（使い方の誤り、読めないファイル）
    let (c, _, _) = run(&["check"]);
    assert_eq!(c, 2, "引数なしは 2");
    let (c, _, _) = run(&["check", "tests/corpus/ありません.rule"]);
    assert_eq!(c, 2, "読めないファイルは 2");
}

#[test]
fn json_は一行一件で流せる形() {
    let (code, out, _) = run(&["check", "tests/mutants/m_e101.rule", "--format", "json"]);
    assert_eq!(code, 1);
    let lines: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(!lines.is_empty(), "JSON が出ていない");
    for l in &lines {
        assert!(l.starts_with('{') && l.ends_with('}'), "一行一件の JSON でない: {l}");
        for key in ["\"severity\"", "\"code\"", "\"file\"", "\"line\"", "\"column\"", "\"title\"", "\"notes\""] {
            assert!(l.contains(key), "{key} が無い: {l}");
        }
    }
    assert!(lines.iter().any(|l| l.contains("\"code\":\"E101\"")), "E101 が無い");
    // JSON のときは人向けの ok / note 行を混ぜない。
    assert!(!out.contains("ok "), "JSON に人向けの行が混ざっている");
    assert!(!out.contains("note "), "JSON に人向けの行が混ざっている");
}

#[test]
fn 通ったファイルはokを出す() {
    let (c, out, _) = run(&["check", "tests/corpus/クーポン割引.rule"]);
    assert_eq!(c, 0);
    assert!(out.contains("ok "), "通ったら ok を出す");
    // 遮蔽は件数一行だけ。要確認がゼロなら一覧は出ない。
    assert!(out.contains("遮蔽 10 対（構造的 4、同値 6、要確認 0）"), "件数行が出る: {out}");
    assert!(!out.contains("warning[W105]"), "要確認ゼロなら一覧は出ない");
}

#[test]
fn show_shadow_で構造的も一覧に出る() {
    let (_, plain, _) = run(&["check", "tests/corpus/ゆうパック運賃.rule"]);
    let (_, verbose, _) = run(&["check", "tests/corpus/ゆうパック運賃.rule", "--show-shadow"]);
    assert!(!plain.contains("warning[W105]"));
    assert_eq!(verbose.matches("warning[W105]").count(), 21, "21 対すべてが出る");
}

#[test]
fn fmt_は冪等で_check_は直すべきものを言う() {
    // コーパスは常に整形済みであること（gofmt と同じ運用。§1.5）。
    let (c, out, _) = run(&["fmt", "--check", "tests/corpus/ゆうパック運賃.rule",
                            "tests/corpus/クーポン割引.rule", "tests/corpus/クーポン併用.rule"]);
    assert_eq!(c, 0, "コーパスが整形されていない: {out}");

    // 二度掛けても変わらない。
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/ゆうパック運賃.rule"),
    )
    .unwrap();
    let once = rulec::fmt::format(&src);
    let twice = rulec::fmt::format(&once);
    assert_eq!(once, twice, "fmt が冪等でない");

    // 崩したものは --check が名指しする。
    let (c, out, _) = run(&["fmt", "--check", "tests/mutants/m_e102.rule"]);
    assert_eq!(c, 1, "崩れたファイルは 1");
    assert!(out.contains("整形されていません"), "{out}");
}

#[test]
fn fmt_は全角の比較記号と数字を正規化する() {
    let src = "規則 試し(t) v1\n\n表 x(x)\n方式 上から\n| ａ | → ｂ(b) : 真偽 |\n| ≦１０ | 真 |\n";
    let got = rulec::fmt::format(src);
    assert!(got.contains("<=10"), "≦１０ が <=10 にならない: {got}");
    assert!(!got.contains('≦'));
    assert!(!got.contains('１'));
}

fn run_in(dir: &std::path::Path, args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned())
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let ok = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    assert!(ok, "git {args:?} に失敗");
}

const BASE: &str = "規則 試し(t) v1\n\n\
型 判定(v) = 甲(a) | 乙(b) | 丙(c) 既定扱い\n\n\
入力\n  x(x) : 真偽\n  y(y) : 真偽\n\n\
出力\n  判定結果(r) : 判定\n\n\
表 t(t)\n方式 上から\n\
| x  | y  | → 判定結果(r) : 判定 |\n\
| 真 | -  | 甲              |\n\
| -  | 真 | 乙              |\n\
| -  | -  | 甲              |\n";

#[test]
fn diff_base_は新たに生じた発見だけを出す() {
    let dir = std::env::temp_dir().join(format!("rulec-diff-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "t"]);
    std::fs::write(dir.join("t.rule"), BASE).unwrap();
    git(&dir, &["add", "t.rule"]);
    git(&dir, &["commit", "-qm", "base"]);

    // 基準の時点で 要確認 が一件ある。
    let (_, before) = run_in(&dir, &["check", "t.rule"]);
    assert_eq!(before.matches("warning[W105]").count(), 1, "{before}");

    // 行を一つ足して、要確認 をもう一件作る。
    let after = BASE.replace(
        "| -  | 真 | 乙              |\n",
        "| -  | 真 | 乙              |\n| 真 | 偽 | 丙              |\n",
    );
    std::fs::write(dir.join("t.rule"), &after).unwrap();

    let (_, plain) = run_in(&dir, &["check", "t.rule"]);
    assert_eq!(plain.matches("warning[W105]").count(), 2, "足した後は二件: {plain}");

    let (_, diffed) = run_in(&dir, &["check", "t.rule", "--diff-base", "HEAD"]);
    assert_eq!(diffed.matches("warning[W105]").count(), 1, "新規の一件だけ: {diffed}");
    assert!(diffed.contains("伏せました"), "伏せた件数を言う: {diffed}");

    let _ = std::fs::remove_dir_all(&dir);
}

/// §8.4: 生成物は git にコミットし、CI の `--check` が再生成との一致を見る。
/// 生成物が古い・手で編集された・そもそも無い、の三つとも 1 で落ちないと
/// CI の門にならない。
#[test]
fn gen_check_は生成物のずれを見つける() {
    let dir = std::env::temp_dir().join(format!("rulec-genchk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.to_string_lossy().to_string();
    let rule = "tests/corpus/送料.rule";

    // 生成物が一つも無い状態。CI に生成物を入れ忘れた場合がこれ。
    let (c, o, _) = run(&["gen", rule, "--out", &out, "--check"]);
    assert_eq!(c, 1, "生成物が無ければ 1: {o}");
    assert!(o.contains("生成物が古いか手で編集されています"), "{o}");
    assert!(!dir.exists(), "--check が書き込んでいる");

    // 生成してから見れば黙って 0。
    let (c, _, _) = run(&["gen", rule, "--out", &out]);
    assert_eq!(c, 0);
    let (c, o, _) = run(&["gen", rule, "--out", &out, "--check"]);
    assert_eq!(c, 0, "生成直後は 0: {o}");
    assert_eq!(o.trim(), "", "一致していれば何も言わない: {o}");

    // 手で一文字足す。中身で比べているので、空白一つでも捕まる。
    let py = dir.join("python").join("shipping_fee.py");
    let before = std::fs::read_to_string(&py).unwrap();
    std::fs::write(&py, format!("{before} ")).unwrap();
    let (c, o, _) = run(&["gen", rule, "--out", &out, "--check"]);
    assert_eq!(c, 1, "手で編集したら 1: {o}");
    assert!(o.contains("shipping_fee.py"), "どのファイルかを言う: {o}");
    assert_eq!(o.lines().count(), 1, "ずれた一件だけ言う: {o}");
    assert_eq!(std::fs::read_to_string(&py).unwrap(), format!("{before} "), "--check が直している");

    // 一つ消す。無いものも「ずれ」として数える。
    std::fs::remove_file(dir.join("go").join("shippingfee").join("shipping_fee.go")).unwrap();
    let (c, o, _) = run(&["gen", rule, "--out", &out, "--check"]);
    assert_eq!(c, 1);
    assert_eq!(o.lines().count(), 2, "編集一件と欠落一件: {o}");
    assert!(o.contains("shipping_fee.go"), "{o}");

    // 直せば戻る。
    let (c, _, _) = run(&["gen", rule, "--out", &out]);
    assert_eq!(c, 0);
    let (c, o, _) = run(&["gen", rule, "--out", &out, "--check"]);
    assert_eq!(c, 0, "{o}");

    let _ = std::fs::remove_dir_all(&dir);
}
