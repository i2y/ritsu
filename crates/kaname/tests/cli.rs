//! The command (DESIGN 11.1): `check` and `explain`, their exit codes, the JSON, `--help` and
//! `--version`, as `ritsu kaname` runs them (the library's `run` with every language joined) and as
//! the binary of kaname's own crate runs them (no other language: E202, exit 2).

mod common;

use kaname::suite::Suite;
use ritsu_testkit::TempDir;
use std::process::Command;

fn run(args: &[&str], suite: Suite) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = kaname::run::run(&args, suite, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn joined(args: &[&str]) -> (u8, String, String) {
    run(args, common::joined())
}

#[test]
fn version_and_help() {
    let (code, out, _) = joined(&["--version"]);
    assert_eq!((code, out), (0, format!("kaname {}\n", env!("CARGO_PKG_VERSION"))));
    let (code, out, _) = joined(&["--help"]);
    assert_eq!(code, 0);
    assert!(out.starts_with(&format!("kaname {}\n\nA small language for the code that joins rules, dates and books", env!("CARGO_PKG_VERSION"))), "{out}");
    assert!(out.contains("  kaname check <file.kn>...") && out.contains("  kaname explain <code>"), "{out}");
    assert!(out.contains("  kaname run <file.kn> <function>") && out.contains("  kaname vectors <file.kn>"), "{out}");
    let (code, out, _) = joined(&["help", "run", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("kaname run — 関数を一つ、JSON で渡した入力で参照インタプリタに流す。"), "{out}");
    let (code, out, _) = joined(&["help", "check", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("kaname check — 検査する。"), "{out}");
    assert!(out.contains("E001 E002 E003"), "{out}");
    let (code, out, err) = joined(&[]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(err.contains("Usage:"), "{err}");
}

#[test]
fn what_stops_the_command() {
    let (code, _, err) = joined(&["build", "x.kn"]);
    assert_eq!((code, err.as_str()), (2, "error: there is no command `build`; run `kaname --help`\n"));
    let (code, _, err) = joined(&["check", "x.kn", "--fast"]);
    assert_eq!((code, err.as_str()), (2, "error: unknown flag `--fast`; run `kaname check --help`\n"));
    let (code, _, err) = joined(&["check"]);
    assert_eq!((code, err.as_str()), (2, "error: `kaname check` needs .kn files\n"));
    let (code, _, err) = joined(&["check", "nowhere.kn", "--lang", "ja"]);
    assert_eq!(code, 2);
    assert!(err.starts_with("エラー: `nowhere.kn` を読めません: "), "{err}");
    let (code, _, err) = joined(&["check", "examples/payout/payout.kn", "--root", "nowhere"]);
    assert_eq!((code, err.as_str()), (2, "error: `--root nowhere` is not a directory\n"));
}

#[test]
fn check_exits_by_what_it_finds() {
    // the functions with their effects (DESIGN 11.1): the amounts of the transfers wait for the
    // checks across the languages (PLAN C3), and are counted as undecided until then
    let (code, out, _) = joined(&["check", "examples/payout/payout.kn", "examples/media_mail/media_mail.kn"]);
    assert_eq!((code, out.as_str()), (0, "ok examples/payout/payout.kn: 1 function, fails range\nok examples/media_mail/media_mail.kn: 2 functions, total\n"));
    let (code, out, _) = joined(&["check", "examples/media_mail/media_mail.ja.kn", "--lang", "ja"]);
    assert_eq!((code, out.as_str()), (0, "ok examples/media_mail/media_mail.ja.kn: 関数 2 個、total\n"));
    let t = TempDir::new("kaname-cli");
    let bad = t.write("bad.kn", "module bad v1\n\nfn f() -> number\n  x\n");
    let bad = bad.to_str().unwrap();
    let (code, out, _) = joined(&["check", bad]);
    assert_eq!(code, 1);
    assert_eq!(out, format!("error[E101]: {bad}:4:3: There is no `x`\n     4 |   x\n"));
    // the worst of the files is the exit code
    let (code, _, _) = joined(&["check", "examples/payout/payout.kn", bad]);
    assert_eq!(code, 1);
}

#[test]
fn check_in_json() {
    let (code, out, _) = joined(&["check", "examples/late_payment/late_payment.kn", "--format", "json", "--root", "examples"]);
    assert_eq!(code, 0);
    let v = ritsu_base::json::parse(&out).unwrap();
    let keys: Vec<&str> = v.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["kaname", "file", "ok", "summary", "functions", "diagnostics"]);
    assert_eq!(v.get("file").and_then(|f| f.as_str()), Some("late_payment/late_payment.kn"));
    assert_eq!(v.get("summary").and_then(|f| f.as_str()), Some("1 function, fails date"));
    // each function with its effect, what is written, and the places that are not shown: the date
    // given to koyomi waits for the checks across the languages (X6, PLAN C3)
    let f = &v.get("functions").unwrap().as_arr().unwrap()[0];
    assert_eq!(
        f.compact(),
        r#"{"function":"claim","effect":{"fails":["date"],"diverges":false},"written":{"total":true,"fails":[],"diverges":false},"places":[{"line":20,"kind":"date","check":"X6","code":null,"outcome":"waiting"}],"calls":[]}"#
    );
    let t = TempDir::new("kaname-json");
    let bad = t.write("bad.kn", "module bad v1\n\nfn f(n: number) -> number\n  n\n");
    let (code, out, _) = joined(&["check", bad.to_str().unwrap(), "--format", "json", "--root", t.path().to_str().unwrap(), "--lang", "ja"]);
    assert_eq!(code, 1);
    let v = ritsu_base::json::parse(&out).unwrap();
    let d = &v.get("diagnostics").unwrap().as_arr().unwrap()[0];
    assert_eq!(d.get("code").and_then(|c| c.as_str()), Some("E108"));
    assert_eq!(d.get("file").and_then(|c| c.as_str()), Some("bad.kn"));
    assert_eq!(d.get("message").and_then(|c| c.as_str()), Some("関数 `f` の引数 `n` に、範囲（`range`）がありません"));
    assert!(v.get("summary").unwrap().is_null());
    assert!(v.get("functions").unwrap().is_null());
}

#[test]
fn explain() {
    let (code, out, _) = joined(&["explain", "e102"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("E102 (error) — The types do not agree\n\nWhen: "), "{out}");
    assert!(out.contains("    module example v1\n"), "{out}");
    let (code, out, _) = joined(&["explain", "E102", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("E102 (エラー) — 型が合いません\n"), "{out}");
    assert!(out.contains("    module 例(example) v1\n"), "{out}");
    let (code, out, _) = joined(&["explain", "--all", "--format", "json"]);
    assert_eq!(code, 0);
    assert_eq!(ritsu_base::json::parse(&out).unwrap().as_arr().unwrap().len(), 33);
    let (code, out, _) = joined(&["explain", "W501"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("W501 (warning) — A decision is written in code; a table should hold it\n"), "{out}");
    assert!(out.contains("  if total >= 35USD then 0USD else 5USD\n"), "{out}");
    let (code, _, err) = joined(&["explain", "E999"]);
    assert_eq!((code, err.as_str()), (2, "error: there is no diagnostic code `E999`; `kaname explain --all` lists them all\n"));
    let (code, out, _) = joined(&["explain", "--all", "--format", "markdown"]);
    assert_eq!(code, 0);
    assert_eq!(out, std::fs::read_to_string("docs/codes.md").unwrap());
}

#[test]
fn the_binary_of_the_crate() {
    // a module that reads a rule: E202, exit 2, and the command to run instead
    let o = Command::new(env!("CARGO_BIN_EXE_kaname")).args(["check", "examples/payout/payout.kn"]).env_remove("KANAME_LANG").env_remove("RITSU_LANG").output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.starts_with("error[E202]: examples/payout/payout.kn:4:1: This kaname cannot read rules (rulec is not in it), and the module reads `card_fee`\n"), "{out}");
    assert!(out.contains("through ritsu: `ritsu kaname check examples/payout/payout.kn`."), "{out}");
    // a module that reads modules only checks in full
    let t = TempDir::new("kaname-bin");
    t.write("a.kn", "module a v1\n\nfn one() -> number\n  1\n");
    let b = t.write("b.kn", "module b v1\n\nuse module a from \"a.kn\"\n\nfn two() -> number\n  a.one() + 1\n");
    let o = Command::new(env!("CARGO_BIN_EXE_kaname")).args(["check", b.to_str().unwrap()]).env_remove("KANAME_LANG").env_remove("RITSU_LANG").output().unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stdout));
    // the language from KANAME_LANG
    let o = Command::new(env!("CARGO_BIN_EXE_kaname")).args(["explain", "E101"]).env("KANAME_LANG", "ja").env_remove("RITSU_LANG").output().unwrap();
    assert!(String::from_utf8_lossy(&o.stdout).starts_with("E101 (エラー) — この名前のものはありません"));
}

#[test]
fn checked_as_ritsu_check_prints_it() {
    let root = std::path::Path::new(".").canonicalize().unwrap();
    let files = vec!["examples/payout/payout.kn".to_string(), "examples/nowhere.kn".to_string()];
    // with no verdicts of the checks across the borders, the amounts of the transfers wait for them
    let units = kaname::ports::Engine.checked_with(&root, &files, &common::ports(), None, ritsu_base::text::Lang::En);
    assert_eq!(units[0].text(), "ok examples/payout/payout.kn: 1 function, fails range\n");
    assert_eq!(units[0].verdict, ritsu_ports::Verdict::Passes);
    assert_eq!(units[1].verdict, ritsu_ports::Verdict::Unchecked);
    assert!(units[1].text().starts_with("error: cannot read `examples/nowhere.kn`: "), "{}", units[1].text());
}
