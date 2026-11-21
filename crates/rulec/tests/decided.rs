//! What decides each output (§15.323). An output that nothing decides, an output read above what
//! decides it, and a table whose heading has no `->` used to pass the check: `rulec run` answered
//! the output with no value, a table that read it matched no row, and the generated code read or
//! returned a name it had never set. The check stops each of them, before the completeness check
//! (E151, E152), and the rule written in order still passes.
//!
//! The materials: `tests/decided/` — an order graded and reviewed (`order_review.rule`) and the
//! shapes that break it — in English, with each one's Japanese version beside it.

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/decided")
}

fn run_in(dir: &Path, lang: &str, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", lang).current_dir(dir).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn has_all(out: &str, said: &[&str]) {
    for s in said {
        assert!(out.contains(s), "missing `{s}`:\n{out}");
    }
}

/// Each shape, in English and in Japanese: the code, what is said, and that neither the
/// completeness check nor the check of dead rows speaks about a table that reads a value nothing
/// decides (the T4 hint of E101 asked for answers to rows with nothing to answer).
#[test]
fn each_shape_is_stopped_in_english_and_in_japanese() {
    #[rustfmt::skip]
    let cases: &[(&str, &str, &[&str], &str, &[&str])] = &[
        ("unwritten.rule", "en", &["error[E151]: Nothing decides the output `grade`", "no table or definition decides it", "take `grade` out of `outputs`"], "E151", &[]),
        ("決めるものの無い出力.rule", "ja", &["エラー[E151]: 出力 `等級` を決めるものがありません", "どの表も定義も、この出力を決めていません", "`outputs` から消してください"], "E151", &[]),
        ("unwritten_optional.rule", "en", &["Nothing decides the output `grade`", "An output that may have no value (`?`) is held to the same"], "E151", &[]),
        ("値の無いことがある出力.rule", "ja", &["出力 `等級` を決めるものがありません", "値の無いことがある出力（`?`）でも同じです"], "E151", &[]),
        ("no_tables.rule", "en", &["Nothing decides the output `grade`"], "E151", &[]),
        ("表の無い規則.rule", "ja", &["出力 `等級` を決めるものがありません"], "E151", &[]),
        // The table that reads the grade is where it is read: one error, on the declaration.
        ("read_unwritten.rule", "en", &["Nothing decides the output `grade`", "|   ^^^^^ no table or definition decides it", "^^^^^ it is read here"], "E151", &[]),
        ("決めるものの無い等級を読む.rule", "ja", &["出力 `等級` を決めるものがありません", "ここで読んでいます"], "E151", &[]),
        ("read_above.rule", "en", &["error[E151]: The output `grade` is read before it is decided", "--> read_above.rule:16 table review_of", "this is what decides `grade`, after it is read", "hint: move what reads `grade` below what decides it (line 23)."], "E151", &[]),
        ("決める表より上で読む.rule", "ja", &["エラー[E151]: 出力 `等級` を、決まる前に読んでいます", "`等級` を決めるのはここで、読む所より後です", "ヒント: `等級` を読む行を、決める行（23 行目）より下へ移してください。"], "E151", &[]),
        // The tables of one output are worked out together at the last of them.
        ("read_between.rule", "en", &["The output `grade` is read before it is decided", "--> read_between.rule:20 table review_of", "(line 26)"], "E151", &[]),
        ("二つの表の間で読む.rule", "ja", &["出力 `等級` を、決まる前に読んでいます", "（26 行目）"], "E151", &[]),
        ("read_result.rule", "en", &["The output `fee` is read before it is decided", "this `result` line is what decides `fee`", "hint: decide `fee` above what reads it, with `define fee : <type> = <expression>` in place of the `result` line."], "E151", &[]),
        ("結果の行より前に読む.rule", "ja", &["出力 `手数料` を、決まる前に読んでいます", "`手数料` を決めるのは、この `result` の行です", "`define 手数料 : <型> = <式>` と書いて決めてください"], "E151", &[]),
        ("define_reads_above.rule", "en", &["The output `fee` is read before it is decided", "define total : money[JPY] = amount + fee", "(line 16)"], "E151", &[]),
        ("決める表より上の定義.rule", "ja", &["出力 `手数料` を、決まる前に読んでいます", "（16 行目）"], "E151", &[]),
        ("reads_itself.rule", "en", &["The output `fee` is read before it is decided", "the expression that decides `fee` reads `fee` itself", "hint: write the expression of `fee` from the inputs and the values decided above it."], "E151", &["after it is read"]),
        ("自分を読む定義.rule", "ja", &["出力 `手数料` を、決まる前に読んでいます", "`手数料` を決める式が、`手数料` 自身を読んでいます"], "E151", &["読む所より後です"]),
        // A heading whose last column names an output: that column is named and the heading is
        // shown with the `->` in place. The grade it would decide is not said again.
        ("no_arrow.rule", "en", &["error[E152]: Table grade_of has no output column", "the column of the output `grade`, read as a condition because no `->` stands before it", "hint: write `->` before `grade`, the first column the rows answer: `| amount | -> grade |`."], "E152", &["E151", "`?`"]),
        ("矢印の無い見出し.rule", "ja", &["エラー[E152]: 表 等級を決める に出力の列がありません", "出力 `等級` の列ですが、`->` の前にあるので条件として読まれます", "ヒント: 行が答える最初の列 `等級` の前に `->` を書いてください（`| 金額 | -> 等級 |`）。"], "E152", &["E151", "`?`"]),
        // No column names an output: the hint says where `->` goes, and the output nothing decides
        // is said too.
        ("no_arrow_inputs.rule", "en", &["Table review_of has no output column", "there is no `->` in the heading", "hint: write `->` before the first column the rows answer.", "Nothing decides the output `review`"], "E152", &[]),
        ("入力だけの見出し.rule", "ja", &["表 確認の要否 に出力の列がありません", "見出しに `->` がありません", "ヒント: 行が答える最初の列の前に `->` を書いてください。", "出力 `確認` を決めるものがありません"], "E152", &[]),
    ];
    for (file, lang, said, code, not) in cases {
        let (c, out, err) = run_in(&dir(), lang, &["check", file]);
        assert_eq!(c, 1, "{file} must not pass:\n{out}{err}");
        assert!(out.contains(&format!("[{code}]")), "{file} must print {code}:\n{out}");
        has_all(&out, said);
        for n in *not {
            assert!(!out.contains(n), "{file} must not print `{n}`:\n{out}");
        }
        for other in ["E101", "E102", "E107"] {
            assert!(!out.contains(other), "{file} must not get to {other}:\n{out}");
        }
        // The same, as JSON: the code at the place it is said.
        let (c, out, _) = run_in(&dir(), "en", &["check", file, "--format", "json"]);
        assert_eq!(c, 1);
        assert!(out.lines().any(|l| l.contains(&format!("\"code\":\"{code}\"")) && l.contains("\"severity\":\"error\"")), "{file}:\n{out}");
    }
}

/// The rule written in order passes and answers both outputs, in English and in Japanese; the
/// shapes above answered the review with no value for every one of these calls.
#[test]
fn the_rule_written_in_order_answers() {
    for (file, grade, review, inputs) in [
        ("order_review.rule", "grade", "review", [("amount=5000", "member=false"), ("amount=50000", "member=false"), ("amount=50000", "member=true")]),
        ("注文の確認.rule", "等級", "確認", [("金額=5000", "会員=false"), ("金額=50000", "会員=false"), ("金額=50000", "会員=true")]),
    ] {
        let (c, out, err) = run_in(&dir(), "en", &["check", file]);
        assert_eq!(c, 0, "{file}:\n{out}{err}");
        let want = [("low", "false"), ("high", "true"), ("high", "false")];
        let want_ja = [("低", "false"), ("高", "true"), ("高", "false")];
        for (k, (a, m)) in inputs.iter().enumerate() {
            let (c, out, err) = run_in(&dir(), "en", &["run", file, a, m]);
            assert_eq!(c, 0, "{file} {a} {m}:\n{out}{err}");
            let (g, r) = if file.starts_with('o') { want[k] } else { want_ja[k] };
            has_all(&out, &[&format!("{grade} = {g}"), &format!("{review} = {r}")]);
            assert!(!out.contains("none"), "{file} {a} {m}:\n{out}");
        }
    }
}

/// What the hints say mends the rule: the `->` written where E152 shows it, and the table that
/// reads the grade moved below the one that decides it, give the rule in order back.
#[test]
fn the_hints_mend_the_rule() {
    let t = TempDir::new("decided-hints");
    let order = std::fs::read_to_string(dir().join("order_review.rule")).unwrap();
    let body = |s: &str| s.lines().skip_while(|l| !l.starts_with("enum")).collect::<Vec<_>>().join("\n");
    // E152: the heading with `->` before the grade.
    let src = std::fs::read_to_string(dir().join("no_arrow.rule")).unwrap().replace("| amount      | grade |", "| amount      | -> grade |");
    std::fs::write(t.path().join("no_arrow.rule"), &src).unwrap();
    let (c, out, err) = run_in(t.path(), "en", &["check", "no_arrow.rule"]);
    assert_eq!(c, 0, "{out}{err}");
    // E151: the table of the review moved below the table of the grade.
    let src = std::fs::read_to_string(dir().join("read_above.rule")).unwrap();
    let (head, tables) = src.split_at(src.find("table review_of").unwrap());
    let (review, grade) = tables.split_at(tables.find("table grade_of").unwrap());
    let moved = format!("{head}{}\n\n{}\n", grade.trim_end(), review.trim_end());
    assert_eq!(body(&moved), body(&order).trim_end(), "the moved rule is the rule in order");
    std::fs::write(t.path().join("read_above.rule"), &moved).unwrap();
    let (c, out, err) = run_in(t.path(), "en", &["check", "read_above.rule"]);
    assert_eq!(c, 0, "{out}{err}");
}

/// The commands that read a rule do not read one that fails the check: `gen` writes nothing, and
/// `run`, `vectors`, `doc` and `api` say the error and stop, exit 1.
#[test]
fn the_commands_that_read_a_rule_refuse_these() {
    let t = TempDir::new("decided-refuse");
    for file in ["read_unwritten.rule", "no_arrow.rule"] {
        let out_dir = t.path().join(file.trim_end_matches(".rule"));
        let o = out_dir.to_string_lossy().into_owned();
        for args in [
            vec!["gen", file, "--out", &o, "--target", "python", "--target", "typescript"],
            vec!["run", file, "amount=5000", "member=false"],
            vec!["vectors", file],
            vec!["doc", file],
            vec!["api", file],
        ] {
            let (c, out, err) = run_in(&dir(), "en", &args);
            assert_eq!(c, 1, "{args:?}:\n{out}{err}");
            assert!(out.contains("error[E15") || err.contains("error[E15"), "{args:?}:\n{out}{err}");
        }
        assert!(!out_dir.exists(), "gen wrote {}", out_dir.display());
    }
}
