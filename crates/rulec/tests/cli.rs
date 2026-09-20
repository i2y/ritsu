//! The CLI surface (§11, principle 6). It is a contract for wiring into CI, so the shape is pinned,
//! not the wording.

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
    // 0 = notes only
    let (c, _, _) = run(&["check", "tests/corpus/ゆうパック運賃.rule"]);
    assert_eq!(c, 0, "警告だけの規則は 0");

    // W114 is a warning, so still 0
    let (c, _, _) = run(&["check", "tests/corpus/クーポン併用.rule"]);
    assert_eq!(c, 0, "W114 は警告なので 0");

    // 1 = errors present
    let (c, _, _) = run(&["check", "tests/mutants/m_e101.rule"]);
    assert_eq!(c, 1, "エラーがあれば 1");

    // 2 = internal failure (usage error, unreadable file)
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
    // With JSON, the human-oriented ok / note lines are not mixed in.
    assert!(!out.contains("ok "), "JSON に人向けの行が混ざっている");
    assert!(!out.contains("note "), "JSON に人向けの行が混ざっている");
}

#[test]
fn 通ったファイルはokを出す() {
    let (c, out, _) = run(&["check", "tests/corpus/クーポン割引.rule"]);
    assert_eq!(c, 0);
    assert!(out.contains("ok "), "通ったら ok を出す");
    // Shadowing is a single count line. With zero needs-confirmation pairs, no list is printed.
    assert!(out.contains("隠れ 10 対（階段 4、同じ答え 6、要確認 0）"), "件数行が出る: {out}");
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
    // The corpus must always be formatted (the same practice as with gofmt; §1.5).
    // **The whole of it.** Three files were named here and the rest was never looked at, so
    // `会員特典.rule` sat unformatted until CI ran `fmt --check` over the directory
    // (DESIGN §15.94). A directory, not a list, is what keeps this honest.
    let (c, out, _) = run(&["fmt", "--check", "tests/corpus/"]);
    assert_eq!(c, 0, "コーパスが整形されていない: {out}");

    // Applying it twice changes nothing.
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/ゆうパック運賃.rule"),
    )
    .unwrap();
    let once = rulec::fmt::format(&src);
    let twice = rulec::fmt::format(&once);
    assert_eq!(once, twice, "fmt が冪等でない");

    // A broken file is named by --check.
    let (c, out, _) = run(&["fmt", "--check", "tests/mutants/m_e102.rule"]);
    assert_eq!(c, 1, "崩れたファイルは 1");
    assert!(out.contains("整形されていません"), "{out}");
}

#[test]
fn fmt_は全角の比較記号と数字を正規化する() {
    let src = "rule 試し(t) v1\n\ntable x(x)\npolicy first\n| ａ | -> ｂ(b) : bool |\n| ≦１０ | true |\n";
    let got = rulec::fmt::format(src);
    assert!(got.contains("<=10"), "≦１０ が <=10 にならない: {got}");
    assert!(!got.contains('≦'));
    assert!(!got.contains('１'));
}

/// §1.5: the two symbols that used to need an IME. `→` and `・`/`、` are still read, and
/// `fmt` writes the ASCII forms `->` and `,`; a file written in ASCII from the start
/// checks clean.
#[test]
fn fmt_は矢印と集合区切りをasciiに正準化する() {
    let ime = "rule t(t) v1\n\nenum 色(c) = 赤(r) | 青(b) | 緑(g) | 黄(y)\n\ninputs\n  色(color) : 色\n\noutputs\n  r(r) : bool\n\n\
               table x(x)\npolicy unique\n| 色 | → r(r) : bool |\n| 赤 ・ 青 | true |\n| 緑、黄 | false |\n";
    let got = rulec::fmt::format(ime);
    assert!(got.contains("| -> r(r) : bool |"), "→ が -> にならない: {got}");
    assert!(got.contains("| 赤, 青 "), "・ が , にならない: {got}");
    assert!(got.contains("| 緑, 黄 "), "、 が , にならない: {got}");
    assert!(!got.contains('→') && !got.contains('・') && !got.contains('、'), "{got}");
    assert_eq!(rulec::fmt::format(&got), got, "fmt が冪等でない");
    // Both spellings parse to the same rule: no diagnostics in either, and the ASCII one
    // is what the corpus is written in.
    let ascii = got.clone();
    assert!(!rulec::has_error(&rulec::check_source(ime, "ime.rule")));
    assert!(!rulec::has_error(&rulec::check_source(&ascii, "ascii.rule")));
    // A comment keeps its prose arrow; the normalization stops at `#`.
    let commented = "rule t(t) v1  # 行1→行2 ・ そのまま\n";
    assert_eq!(rulec::fmt::format(commented), commented, "コメントの中を触っている");
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

const BASE: &str = "rule 試し(t) v1\n\n\
enum 判定(v) = 甲(a) | 乙(b) | 丙(c) default\n\n\
inputs\n  x(x) : bool\n  y(y) : bool\n\n\
outputs\n  判定結果(r) : 判定\n\n\
table t(t)\npolicy first\n\
| x  | y  | -> 判定結果(r) : 判定 |\n\
| true | -  | 甲              |\n\
| -  | true | 乙              |\n\
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

    // At the base there is one needs-confirmation pair.
    let (_, before) = run_in(&dir, &["check", "t.rule"]);
    assert_eq!(before.matches("warning[W105]").count(), 1, "{before}");

    // Add one row to create one more needs-confirmation pair.
    let after = BASE.replace(
        "| -  | true | 乙              |\n",
        "| -  | true | 乙              |\n| true | false | 丙              |\n",
    );
    std::fs::write(dir.join("t.rule"), &after).unwrap();

    let (_, plain) = run_in(&dir, &["check", "t.rule"]);
    assert_eq!(plain.matches("warning[W105]").count(), 2, "足した後は二件: {plain}");

    let (_, diffed) = run_in(&dir, &["check", "t.rule", "--diff-base", "HEAD"]);
    assert_eq!(diffed.matches("warning[W105]").count(), 1, "新規の一件だけ: {diffed}");
    assert!(diffed.contains("伏せました"), "伏せた件数を言う: {diffed}");

    let _ = std::fs::remove_dir_all(&dir);
}

/// §8.4: generated code is committed to git, and `--check` in CI verifies that it matches a fresh
/// regeneration. Unless all three cases — stale, hand-edited, and missing altogether — fail with 1,
/// it is no gate for CI.
#[test]
fn gen_check_は生成物のずれを見つける() {
    let dir = std::env::temp_dir().join(format!("rulec-genchk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.to_string_lossy().to_string();
    let rule = "tests/corpus/送料.rule";

    // No generated files at all. This is the case of forgetting to put them into CI.
    let (c, o, _) = run(&["gen", rule, "--out", &out, "--check"]);
    assert_eq!(c, 1, "生成物が無ければ 1: {o}");
    assert!(o.contains("生成物が古いか手で編集されています"), "{o}");
    assert!(!dir.exists(), "--check が書き込んでいる");

    // Checked right after generating, it is silently 0.
    let (c, _, _) = run(&["gen", rule, "--out", &out]);
    assert_eq!(c, 0);
    let (c, o, _) = run(&["gen", rule, "--out", &out, "--check"]);
    assert_eq!(c, 0, "生成直後は 0: {o}");
    assert_eq!(o.trim(), "", "一致していれば何も言わない: {o}");

    // Add one character by hand. The comparison is by content, so even a single space is caught.
    let py = dir.join("python").join("shipping_fee.py");
    let before = std::fs::read_to_string(&py).unwrap();
    std::fs::write(&py, format!("{before} ")).unwrap();
    let (c, o, _) = run(&["gen", rule, "--out", &out, "--check"]);
    assert_eq!(c, 1, "手で編集したら 1: {o}");
    assert!(o.contains("shipping_fee.py"), "どのファイルかを言う: {o}");
    assert_eq!(o.lines().count(), 1, "ずれた一件だけ言う: {o}");
    assert_eq!(std::fs::read_to_string(&py).unwrap(), format!("{before} "), "--check が直している");

    // Delete one. A missing file counts as a mismatch too.
    std::fs::remove_file(dir.join("go").join("shippingfee").join("shipping_fee.go")).unwrap();
    let (c, o, _) = run(&["gen", rule, "--out", &out, "--check"]);
    assert_eq!(c, 1);
    assert_eq!(o.lines().count(), 2, "編集一件と欠落一件: {o}");
    assert!(o.contains("shipping_fee.go"), "{o}");

    // Regenerating restores it.
    let (c, _, _) = run(&["gen", rule, "--out", &out]);
    assert_eq!(c, 0);
    let (c, o, _) = run(&["gen", rule, "--out", &out, "--check"]);
    assert_eq!(c, 0, "{o}");

    let _ = std::fs::remove_dir_all(&dir);
}

/// §12: CI writes `rulec check rules/`. A directory expands to the `.rule` files inside it.
#[test]
fn ディレクトリを渡すと中の規則を全部見る() {
    let (c, out, _) = run(&["check", "tests/corpus"]);
    assert_eq!(c, 0, "{out}");
    let ok = out.lines().filter(|l| l.starts_with("ok ")).count();
    // Only the rules: the directory also holds the copies of the sources they cite.
    let n = std::fs::read_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "rule"))
        .count();
    assert_eq!(ok, n, "コーパス {n} 本のはずが {ok} 本しか見ていない:\n{out}");

    // The order is deterministic. If the order of the report varied by environment, the diff would
    // be unreadable.
    let (_, again, _) = run(&["check", "tests/corpus"]);
    assert_eq!(out, again, "二度目で並びが変わった");

    // The argument of `test` is the output directory itself, so it must not be expanded.
    let dir = std::env::temp_dir().join(format!("rulec-dir-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let d = dir.to_string_lossy().to_string();
    let (c, _, _) = run(&["gen", "tests/corpus/期間区分.rule", "--out", &d]);
    assert_eq!(c, 0);
    let (c, r, _) = run(&["test", &d]);
    assert_eq!(c, 0, "test がディレクトリを展開してしまった:\n{r}");
    assert!(r.contains("period"), "{r}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── §2 of the plan: one table behind `--help` and the parser ─────────────
//
// The CLI is what an agent reads instead of asking a person, so two properties are
// pinned here: every subcommand explains itself, and a flag the tool does not know
// stops the run instead of being silently dropped.

/// Subcommand names, read out of the overall `rulec --help`.
fn subcommands() -> Vec<String> {
    let (c, out, _) = run(&["--help"]);
    assert_eq!(c, 0, "rulec --help は 0 で終わる");
    out.lines()
        .filter_map(|l| l.strip_prefix("  rulec "))
        .filter_map(|l| l.split_whitespace().next())
        .filter(|w| w.chars().all(|c| c.is_ascii_lowercase()))
        .map(|s| s.to_string())
        .collect()
}

/// The flags a command's own `--help` lists, with their value placeholders.
fn flags_of(cmd: &str) -> Vec<(String, Option<String>)> {
    let (c, out, _) = run(&[cmd, "--help"]);
    assert_eq!(c, 0, "`rulec {cmd} --help` は 0 で終わる");
    out.lines()
        .filter_map(|l| l.strip_prefix("  --"))
        .map(|l| {
            let mut it = l.split_whitespace();
            let name = format!("--{}", it.next().unwrap());
            let v = it.next().filter(|w| !w.starts_with('(') && !w.starts_with('-'));
            (name, v.map(|s| s.to_string()))
        })
        .collect()
}

#[test]
fn 全サブコマンドがヘルプを持つ() {
    let cmds = subcommands();
    assert!(cmds.len() >= 13, "サブコマンドが読み取れていない: {cmds:?}");
    for c in &cmds {
        let (code, out, _) = run(&[c, "--help"]);
        assert_eq!(code, 0, "`rulec {c} --help` が 0 でない");
        assert!(out.contains(&format!("rulec {c}")), "自分の名前を言わない: {out}");
        // §11 principle 6 pins the exit codes, so every help says what they mean here,
        // and every help shows two examples the reader can run.
        assert!(out.contains("exit"), "exit code の節が無い: {c}");
        let ex = out.lines().filter(|l| l.trim_start().starts_with("$ rulec ")).count();
        assert!(ex >= 2, "例が二つ未満: {c}\n{out}");
        // `rulec help <cmd>` is the same page.
        let (code2, out2, _) = run(&["help", c]);
        assert_eq!(code2, 0);
        assert_eq!(out2, out, "`rulec help {c}` と `rulec {c} --help` が違う");
    }
}

#[test]
fn ヘルプに載る全フラグを解析器が知っている() {
    // The rendering and the parsing read the same table, so a flag that `--help`
    // lists can never be one the parser rejects.
    for c in subcommands() {
        for (name, value) in flags_of(&c) {
            if name == "--help" {
                continue;
            }
            let mut args = vec![c.as_str(), name.as_str()];
            if value.is_some() {
                args.push("X");
            }
            let (_, _, err) = run(&args);
            assert!(
                !err.contains("unknown flag") && !err.contains("知らないフラグ"),
                "`rulec {c} {name}` が未知扱い: {err}"
            );
        }
    }
}

#[test]
fn 知らないフラグは黙殺せず2で止まる() {
    for c in subcommands() {
        let (code, _, err) = run(&[&c, "--no-such-flag"]);
        assert_eq!(code, 2, "`rulec {c} --no-such-flag` が 2 でない");
        assert!(err.contains("--no-such-flag"), "打ち間違いを名指ししない: {err}");
        assert!(err.contains(&format!("rulec {c} --help")), "どこを読めばよいか言わない: {err}");
    }
    // A misspelled value of a flag that has a closed set of values is caught too.
    let (code, _, err) = run(&["check", "tests/corpus/送料.rule", "--format", "jsn"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("jsn"), "{err}");
}

#[test]
fn 値を取るフラグは値なしで止まる() {
    let (code, _, err) = run(&["check", "tests/corpus/送料.rule", "--diff-base"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--diff-base"), "{err}");
}

#[test]
fn versionは版だけを言う() {
    let (code, out, _) = run(&["--version"]);
    assert_eq!(code, 0);
    assert_eq!(out.trim(), format!("rulec {}", env!("CARGO_PKG_VERSION")));
    assert_eq!(out.lines().count(), 1, "版だけを言う: {out}");
}

#[test]
fn 知らないサブコマンドは2で止まる() {
    let (code, _, err) = run(&["chekc", "x.rule"]);
    assert_eq!(code, 2);
    assert!(err.contains("chekc"), "{err}");
    assert!(err.contains("rulec --help"), "{err}");
}

/// The two flags no test ever passed. Counted by taking every `--flag` string in `main.rs`
/// and looking for it under `tests/` (§15.89): a flag nothing exercises is a flag whose
/// parsing, default and effect are all unverified.
///
/// `--budget` is the cap on the nodes the check visits, and E109 is what happens over it —
/// the one diagnostic that says "this was not proven" rather than "this is wrong", so it has
/// to be reachable on purpose and not only by accident on a large rule.
#[test]
fn budget_の上限を超えるとE109で止まる() {
    let rule = "tests/corpus/ゆうパック運賃.rule";
    // The default proves it.
    let (c, out, err) = run(&["check", rule]);
    assert_eq!(c, 0, "{out}{err}");
    assert!(!out.contains("E109"), "既定の予算で E109 が出ている: {out}");
    // A budget of ten nodes cannot.
    let (c, out, _) = run(&["check", rule, "--budget", "10"]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("E109"), "予算を切り詰めても E109 が出ない: {out}");
    // And the cap is read as a number, not ignored: a budget large enough proves it again.
    let (c, out, _) = run(&["check", rule, "--budget", "100000000"]);
    assert_eq!(c, 0, "{out}");
    assert!(!out.contains("E109"), "{out}");
}

/// `--outputs <n>` says how many of the trailing columns of a spreadsheet are outputs. With
/// the default of one, a two-output table comes back with the first of them read as an input.
#[test]
fn importのoutputsは末尾の列を出力にする() {
    let d = std::env::temp_dir().join(format!("rulec-cli-outputs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let csv = d.join("t.csv");
    std::fs::write(&csv, "区分,送料,手数料\n近,100円,10円\n遠,200円,20円\n").unwrap();
    let p = csv.to_str().unwrap();

    let (c, one, e) = run(&["import", "csv", p]);
    assert_eq!(c, 0, "{e}");
    assert!(one.contains("-> 手数料"), "既定では末尾の一列だけが出力: {one}");
    assert!(one.contains("| 区分 | 送料 |"), "送料 は入力の側にいる: {one}");

    let (c, two, e) = run(&["import", "csv", p, "--outputs", "2"]);
    assert_eq!(c, 0, "{e}");
    assert!(two.contains("-> 送料"), "二列を出力にしたら 送料 も出力: {two}");
    assert!(two.contains("手数料"), "{two}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Every flag the CLI declares is passed by some test.
///
/// This is the same shape as the diagnostic ledger and the site's colouring vocabulary: a
/// surface declared in one place, held to what exercises it. Two flags were found unexercised
/// this way (§15.89) — `--budget`, which is the only way to reach E109 on purpose, and
/// `--outputs`, which decides how a spreadsheet's columns are split. A flag nothing passes is
/// a flag whose parsing, default and effect are all unverified.
#[test]
fn 宣言したフラグは全部どこかのテストが渡している() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let main = std::fs::read_to_string(root.join("src/main.rs")).unwrap();
    // The CLI surface is what the `flag(...)` helper declares; a `--word` inside a call to
    // some other program (git, say) is not one.
    let mut declared: Vec<String> = Vec::new();
    for part in main.split("flag(\"--").skip(1) {
        if let Some(end) = part.find('"') {
            let f = format!("--{}", &part[..end]);
            if !declared.contains(&f) {
                declared.push(f);
            }
        }
    }
    assert!(declared.len() > 15, "フラグの宣言が見つからない: {declared:?}");

    let mut used = String::new();
    for e in std::fs::read_dir(root.join("tests")).unwrap().flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "rs") {
            used.push_str(&std::fs::read_to_string(&p).unwrap());
        }
    }
    let missing: Vec<&String> =
        declared.iter().filter(|f| !used.contains(&format!("\"{f}\""))).collect();
    assert!(
        missing.is_empty(),
        "どのテストも渡していないフラグ: {missing:?}。\
         フラグは宣言されただけでは動きません — 一つ渡すテストを書いてください"
    );
}
