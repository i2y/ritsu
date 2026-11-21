//! The command (DESIGN 16.1): `check`, `simulate`, `extract` and `explain`, their flags, exit codes
//! and JSON, `--help` and `--version`, as `ritsu ringi` runs them (the library's `run` with every
//! language joined) and as the binary of ringi's own crate runs them (no other language: a request
//! says E008 and exits 2, an organization and masters pass). `run` and `vectors` are in
//! `tests/machine.rs` and `tests/vectors.rs`, and `api` in `tests/items.rs`.

mod common;

use ringi::suite::Suite;
use ritsu_base::text::Lang;
use std::process::Command;

fn run(args: &[&str], suite: Suite) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = ringi::run::run(&args, suite, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn joined(args: &[&str]) -> (u8, String, String) {
    run(args, common::joined())
}

fn alone(args: &[&str]) -> (u8, String, String) {
    run(args, Suite::default())
}

#[test]
fn version_and_help() {
    let (code, out, _) = joined(&["--version"]);
    assert_eq!((code, out), (0, format!("ringi {}\n", env!("CARGO_PKG_VERSION"))));
    let (code, out, _) = joined(&["--help"]);
    assert_eq!(code, 0);
    assert!(out.starts_with(&format!("ringi {}\n\nA small language for a company's applications and approvals", env!("CARGO_PKG_VERSION"))), "{out}");
    assert!(out.contains("  ringi check <path>...") && out.contains("  ringi simulate <file.ringi>") && out.contains("  ringi api <file.ringi>") && out.contains("  ringi explain <CODE>"), "{out}");
    let (code, out, _) = joined(&["help", "check", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("ringi check — 検査する。"), "{out}");
    assert!(out.contains("E001 E002 E003"), "{out}");
    let (code, out, _) = joined(&["check", "--help"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("ringi check — check the words and the names"), "{out}");
    let (code, out, err) = joined(&[]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(err.contains("Usage:"), "{err}");
}

#[test]
fn what_stops_the_command() {
    let (code, _, err) = joined(&["build", "x.ringi"]);
    assert_eq!((code, err.as_str()), (2, "error: there is no command `build`; run `ringi --help`\n"));
    let (code, _, err) = joined(&["check", "x.ringi", "--fast"]);
    // the flag it was most likely meant to be, as every command says it (ritsu's DESIGN 6.2, item 10)
    assert_eq!((code, err.as_str()), (2, "error: unknown flag `--fast` (did you mean `--at`?); run `ringi check --help`\n"));
    let (code, _, err) = joined(&["check", "x.ringi", "--format", "yaml"]);
    assert_eq!(code, 2, "{err}");
    let (code, _, err) = joined(&["check"]);
    assert_eq!((code, err.as_str()), (2, "error: `ringi check` takes <path>...\nusage: ringi check <path>... [--format json] [--root <dir>] [--org <dir>] [--at <date>] [--through <date>]\n"));
    let (code, _, err) = joined(&["check", "examples/org"]);
    assert_eq!((code, err.as_str()), (2, "error: there is no .ringi file under `examples/org`\n"));
    let (code, _, err) = joined(&["check", "examples/leave.ringi", "--at", "2026-13-01"]);
    assert_eq!((code, err.as_str()), (2, "error: `--at 2026-13-01` is not a date such as 2027-04-01\n"));
    let (code, _, err) = joined(&["check", "examples/leave.ringi", "--through", "soon"]);
    assert_eq!((code, err.as_str()), (2, "error: `--through soon` is not a date such as 2027-04-01\n"));
    let (code, _, err) = joined(&["check", "examples/leave.ringi", "--org", "nowhere"]);
    assert_eq!((code, err.as_str()), (2, "error: `--org nowhere` is not a directory\n"));
    let (code, _, err) = joined(&["check", "nowhere.ringi", "--lang", "ja"]);
    assert_eq!((code, err.as_str()), (2, "エラー: `nowhere.ringi` がありません\n"));
    let (code, _, err) = joined(&["check", "examples/expense.ringi", "--root", "nowhere"]);
    assert_eq!((code, err.as_str()), (2, "error: `--root nowhere` is not a directory\n"));
}

/// `init` writes what `ritsu gen` makes too, which this crate does not hold: here it says to run
/// `ritsu ringi init`, writes nothing, and exits 2 (ritsu's `tests/ringi_init.rs` runs it).
#[test]
fn init_runs_inside_ritsu_only() {
    let t = ritsu_testkit::TempDir::new("ringi-init-alone");
    let dir = t.path().join("approvals").to_string_lossy().into_owned();
    let (code, out, err) = alone(&["init", &dir, "--module", "example.com/acme/approvals"]);
    assert_eq!((code, out.as_str(), err.as_str()), (2, "", "error: `init` writes the code ritsu gen makes too, so it runs inside ritsu only: run `ritsu ringi init`\n"));
    let (code, _, err) = joined(&["init", &dir, "--module", "example.com/acme/approvals", "--lang", "ja"]);
    assert_eq!((code, err.as_str()), (2, "エラー: `init` は、ritsu gen で作るコードも書くので、ritsu の中でだけ走ります。`ritsu ringi init` を走らせてください\n"));
    assert!(!t.path().join("approvals").exists());
    let (code, out, _) = alone(&["init", "--help"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("ringi init — write the first files of a Go project"), "{out}");
}

/// The lines under an organization's `ok` line and a request's: what the data holds, the days the
/// request can be used on, and within how long a ticket of it ends.
const COMPANY_OK: &str = "ok examples/shared/company.ringi: an organization, 5 positions, 5 kinds of team\n  data `../org`: 2 versions (2026-04-01, 2027-04-01), 13 teams and 23 people in the last, 2 proxies\n";
const MASTERS_OK: &str = "ok examples/shared/masters.ringi: 3 masters\n  data: accounts 4 rows, vendors 3 rows, systems 3 rows\n";
const USED: &str = "  used from 2026-04-01: the first version of the organization `shared/company.ringi` (data `../org`)\n  used with no end: no data it reads ends\n";
const EXPENSE_ENDS: &str = "  ends within 93 business days at the most (4 × 21 at its stages, 3 × 3 with the requester)\n";
const LEAVE_ENDS: &str = "  ends within 40 business days at the most (3 × 10 at its stages, 2 × 5 with the requester)\n";
/// The lines under the expense request's `ok` line after the days: what the shape of the
/// organization does to its stages, and the command that shows whose requests they are.
const EXPENSE_SHAPED: &str = "  requesters whose stages (`precheck`, `first`, `second`, `decide`) go elsewhere or are skipped by the shape of the organization: 14 of 24 in the organization from 2026-04-01, and 13 of 23 from 2027-04-01\n  requesters whose late stages (`second`) have nowhere to go up to by the shape of the organization: 1 of 24 in the organization from 2026-04-01, and 1 of 23 from 2027-04-01\n  who they are, and where their stages go: `ringi simulate examples/expense.ringi --every-requester`\n";

#[test]
fn check_exits_by_what_it_finds() {
    let (code, out, err) = joined(&["check", "examples/shared/company.ringi", "examples/shared/masters.ringi"]);
    assert_eq!((code, out, err.as_str()), (0, format!("{COMPANY_OK}{MASTERS_OK}"), ""));
    // warnings of the organization's data: exit 0, and the request passes
    let (code, out, _) = joined(&["check", "examples/expense.ringi"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("warning[W502]: examples/expense.ringi:33:93: "), "{out}");
    assert!(out.ends_with(&format!("ok examples/expense.ringi: a request, 8 fields, 4 stages\n{USED}{EXPENSE_ENDS}{EXPENSE_SHAPED}")), "{out}");
    let (code, out, _) = joined(&["check", "examples/expense.ja.ringi", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(
        out.ends_with(concat!(
            "ok examples/expense.ja.ringi: 申請の種類、フィールド 8 個、ステップ 4 個\n",
            "  2026-04-01 から使えます: 組織 `shared/company.ringi` の最初の版（データ `../org`）\n",
            "  終わり無く使えます: 読むデータに終わりがありません\n",
            "  長くても 93 営業日で終わります（ステップに 4 × 21、申請者のもとで 3 × 3）\n",
            "  組織の形で、ステップ（`確かめ`、`一次`、`二次`、`決裁`）がほかへ回るか飛ばされる申請者: 2026-04-01 からの組織の 24 人のうち 14 人、2027-04-01 からの 23 人のうち 13 人\n",
            "  組織の形で、期限を過ぎたステップ（`二次`）を上へ回せない申請者: 2026-04-01 からの組織の 24 人のうち 1 人、2027-04-01 からの 23 人のうち 1 人\n",
            "  だれの申請の、どのステップがどこへ回るか: `ringi simulate examples/expense.ja.ringi --every-requester`\n"
        )),
        "{out}"
    );
    // an error: exit 1, and what is printed is the mutant's golden file
    let bad = "tests/mutants/E004_a_cell_not_in_who.ringi";
    let (code, out, _) = joined(&["check", bad]);
    assert_eq!(code, 1);
    assert_eq!(out, std::fs::read_to_string("tests/golden/E004_a_cell_not_in_who.en.txt").unwrap());
    // the worst of the files is the exit code, and every file is checked
    let (code, out, _) = joined(&["check", bad, "examples/leave.ringi"]);
    assert_eq!(code, 1);
    // the leave request says who may submit it (DESIGN 5.13) after the days it can be used on, and
    // then what the shape of the organization does to its stages, counting those who may submit
    assert!(
        out.ends_with(&format!(
            "ok examples/leave.ringi: a request, 6 fields, 2 stages\n{USED}{LEAVE_ENDS}{}{}{}{}",
            "  submitted by 19 of 24 people in the organization from 2026-04-01, and 18 of 23 from 2027-04-01 (`everyone except team board`)\n",
            "  requesters whose stages (`head`) go elsewhere or are skipped by the shape of the organization: 9 of 19 in the organization from 2026-04-01, and 8 of 18 from 2027-04-01\n",
            "  requesters whose late stages (`manager`, `head`) have nowhere to go up to by the shape of the organization: 2 of 19 in the organization from 2026-04-01, and 2 of 18 from 2027-04-01\n",
            "  who they are, and where their stages go: `ringi simulate examples/leave.ringi --every-requester`\n"
        )),
        "{out}"
    );
    let (code, _, err) = joined(&["check", "examples/leave.ringi", "nowhere.ringi"]);
    assert_eq!(code, 2);
    // the file before it is checked and printed; the one that is not there is said as every command
    // of ritsu says it (ritsu's DESIGN 6.2, item 10)
    assert_eq!(err, "error: `nowhere.ringi` is not there\n");
}

#[test]
fn alone_a_request_says_to_run_it_through_ritsu() {
    // the organization and the masters need no other language
    let (code, out, _) = alone(&["check", "examples/shared/company.ringi", "examples/shared/masters.ringi"]);
    assert_eq!((code, out), (0, format!("{COMPANY_OK}{MASTERS_OK}")));
    // a request needs kaname and rulec at least: E008, exit 2, and the same command line through ritsu
    let (code, out, _) = alone(&["check", "examples/leave.ringi"]);
    assert_eq!(code, 2);
    assert!(out.starts_with("error[E008]: examples/leave.ringi:1:1: The file needs kaname, which this ringi does not hold (a request is lowered to a module of kaname's)\n"), "{out}");
    assert!(out.contains("through ritsu: `ritsu ringi check examples/leave.ringi`."), "{out}");
    assert!(!out.contains("\nok "), "{out}");
    let (code, out, _) = alone(&["check", "examples/shared/manager_approval.ringi", "--lang", "ja"]);
    assert_eq!(code, 2);
    assert!(out.starts_with("エラー[E008]: examples/shared/manager_approval.ringi:"), "{out}");
    assert!(out.contains("`ritsu ringi check examples/shared/manager_approval.ringi --lang ja`"), "{out}");
}

#[test]
fn check_in_json() {
    let (code, out, _) = joined(&["check", "examples/expense.ringi", "--format", "json", "--root", "examples"]);
    assert_eq!(code, 0);
    let v = ritsu_base::json::parse(&out).unwrap();
    let keys: Vec<&str> = v.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["ringi", "file", "ok", "summary", "facts", "usable", "diagnostics"]);
    assert_eq!(v.get("file").and_then(|f| f.as_str()), Some("expense.ringi"));
    assert_eq!(v.get("ok").and_then(|f| f.as_bool()), Some(true));
    assert_eq!(v.get("summary").and_then(|f| f.as_str()), Some("a request, 8 fields, 4 stages"));
    // each line under `ok` with its cases: what the shape of the organization does to a stage, each
    // with the versions, what happens, why, and every requester
    let facts = v.get("facts").and_then(|f| f.as_arr()).unwrap();
    let text: Vec<&str> = facts.iter().filter_map(|f| f.get("text").and_then(|t| t.as_str())).collect();
    assert_eq!(text.iter().map(|f| format!("  {f}\n")).collect::<String>(), format!("{USED}{EXPENSE_ENDS}{EXPENSE_SHAPED}"));
    let keys: Vec<&str> = facts[0].as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["text", "cases"]);
    assert!(facts[0].get("cases").and_then(|c| c.as_arr()).unwrap().is_empty());
    let case = &facts[3].get("cases").and_then(|c| c.as_arr()).unwrap()[0];
    let keys: Vec<&str> = case.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["versions", "stage", "stage_written", "way", "way_written", "what", "cause", "notes", "lines", "requesters"]);
    let late = &facts[4].get("cases").and_then(|c| c.as_arr()).unwrap()[0];
    assert_eq!((late.get("stage").and_then(|x| x.as_str()), late.get("what").and_then(|x| x.as_str())), (Some("second"), Some("late")));
    // the days it can be used on, each end with what decides it; the data's directory from the root
    let u = v.get("usable").unwrap();
    assert_eq!(u.get("from").and_then(|x| x.as_str()), Some("2026-04-01"));
    assert!(u.get("through").is_some_and(|x| x.is_null()));
    let by = &u.get("from_by").and_then(|x| x.as_arr()).unwrap()[0];
    let keys: Vec<&str> = by.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["kind", "name", "written", "file", "line", "data", "text"]);
    assert_eq!((by.get("kind").and_then(|x| x.as_str()), by.get("name").and_then(|x| x.as_str()), by.get("file").and_then(|x| x.as_str())), (Some("organization"), Some("org"), Some("org")));
    // the warnings of the organization's data, and no error
    let d = v.get("diagnostics").and_then(|d| d.as_arr()).unwrap();
    assert!(!d.is_empty() && d.iter().all(|x| x.get("severity").and_then(|c| c.as_str()) == Some("warning")), "{out}");
    // one object a file, each on its own lines
    let (code, out, _) = joined(&["check", "examples/leave.ringi", "examples/shared/masters.ringi", "--format", "json", "--lang", "ja"]);
    assert_eq!(code, 0);
    let objects: Vec<&str> = out.split("\n}\n").filter(|s| !s.trim().is_empty()).collect();
    assert_eq!(objects.len(), 2, "{out}");
    let v = ritsu_base::json::parse(&format!("{}\n}}", objects[1])).unwrap();
    assert_eq!(v.get("summary").and_then(|f| f.as_str()), Some("マスタ 3 個"));
    assert_eq!(v.get("facts").and_then(|f| f.as_arr()).and_then(|f| f[0].get("text")).and_then(|t| t.as_str()), Some("データ: accounts 4 行、vendors 3 行、systems 3 行"));
    assert!(v.get("usable").is_some_and(|x| x.is_null()), "masters are not used on days of their own");
    // alone, a request is not ok, and says why in the language asked for
    let (code, out, _) = alone(&["check", "examples/leave.ringi", "--format", "json"]);
    assert_eq!(code, 2);
    let v = ritsu_base::json::parse(&out).unwrap();
    assert_eq!(v.get("ok").and_then(|f| f.as_bool()), Some(false));
    let d = v.get("diagnostics").and_then(|d| d.as_arr()).unwrap();
    assert!(!d.is_empty() && d.iter().all(|x| x.get("code").and_then(|c| c.as_str()) == Some("E008")), "{out}");
}

/// Every string of `json` that is a name: the ones not under a key that holds a sentence for a
/// person, a text of the file or a name as written (`written`, `…_written`), each with its key.
fn names_of(json: &ritsu_base::json::Json, left_out: &[&str], key: &str, out: &mut Vec<(String, String)>) {
    use ritsu_base::json::Json;
    match json {
        Json::Obj(pairs) => {
            for (k, v) in pairs.iter().filter(|(k, _)| k != "written" && !k.ends_with("_written") && !left_out.contains(&k.as_str())) {
                names_of(v, left_out, k, out);
            }
        }
        Json::Arr(items) => items.iter().for_each(|v| names_of(v, left_out, key, out)),
        Json::Str(s) => out.push((key.to_string(), s.clone())),
        _ => {}
    }
}

/// The JSON of `check`, `simulate` and `api` names things in ASCII (DESIGN 3.4, 16.2), as the JSON
/// of `run` and the vectors do: for a request in Japanese names, a stage, a way to choose, a field,
/// the alias of a `use`, a master and a kind of team is its ASCII name, with the name as written
/// beside it under `written` or the same key and `_written`. What is left in Japanese is what a
/// person reads (the sentences, the titles) and what the file writes (a definition, a unit, the
/// name of a claim or a scenario, which has no other).
#[test]
fn the_json_names_things_in_ascii() {
    let parsed = |args: &[&str]| {
        let (code, out, err) = joined(args);
        assert!(code == 0 || code == 1, "{args:?}: {err}");
        ritsu_base::json::parse(&out).unwrap_or_else(|e| panic!("{args:?}: {e:?}\n{out}"))
    };
    let ascii = |v: &ritsu_base::json::Json, left_out: &[&str], what: &str| {
        let mut names = Vec::new();
        names_of(v, left_out, "", &mut names);
        assert!(names.len() > 10, "{what}: {names:?}");
        for (key, name) in names {
            assert!(name.is_ascii(), "{what}: `{key}` is `{name}`");
        }
    };
    let text = |v: &ritsu_base::json::Json, path: &[&str]| path.iter().try_fold(v, |v, k| v.get(k)).and_then(|x| x.as_str()).map(str::to_string);
    for name in ["account", "contract", "expense", "leave", "purchase"] {
        let file = format!("examples/{name}.ja.ringi");
        for lang in ["en", "ja"] {
            let v = parsed(&["check", &file, "--format", "json", "--lang", lang]);
            ascii(&v, &["summary", "text", "cause", "notes", "message", "fix", "source", "label"], &format!("check {file}"));
            let v = parsed(&["simulate", &file, "--every-requester", "--on", "2026-11-16", "--format", "json", "--lang", lang]);
            ascii(&v, &["due", "case", "nobody", "late"], &format!("simulate {file}"));
        }
        // the contract request has an error in the second version of the organization (E504), and
        // `api` is of a file that passes
        if name != "contract" {
            let v = parsed(&["api", &file]);
            // a unit is the unit table's spelling (`money[円]`), and a claim and a scenario have
            // the name they are written by
            let mut v = v.clone();
            if let ritsu_base::json::Json::Obj(pairs) = &mut v {
                for (k, x) in pairs.iter_mut() {
                    if let ("items", ritsu_base::json::Json::Arr(items)) = (k.as_str(), x) {
                        items.retain(|i| !matches!(i.get("kind").and_then(|k| k.as_str()), Some("claim" | "scenario")));
                    }
                }
            }
            ascii(&v, &["title", "description", "label", "text", "type", "where", "none", "may_be_less", "undecided", "fix"], &format!("api {file}"));
        }
    }
    // a stage and a way of a case under `ok`, and what decides the days a request is used on
    let v = parsed(&["check", "examples/expense.ja.ringi", "--format", "json", "--root", "examples"]);
    let facts = v.get("facts").and_then(|f| f.as_arr()).unwrap();
    let case = facts.iter().filter_map(|f| f.get("cases").and_then(|c| c.as_arr())).find(|c| !c.is_empty()).map(|c| &c[0]).unwrap();
    assert_eq!(
        (text(case, &["stage"]), text(case, &["stage_written"]), text(case, &["way"]), text(case, &["way_written"])),
        (Some("precheck".into()), Some("確かめ".into()), Some("check".into()), Some("エージェント確認".into()))
    );
    let by = &v.get("usable").and_then(|u| u.get("from_by")).and_then(|x| x.as_arr()).unwrap()[0];
    assert_eq!((text(by, &["name"]), text(by, &["written"])), (Some("org".into()), Some("組織".into())));
    // a way a rule gives is named by the rule's alias in ASCII
    let v = parsed(&["check", "examples/purchase.ja.ringi", "--format", "json"]);
    let ways: Vec<(Option<String>, Option<String>)> = v
        .get("facts")
        .and_then(|f| f.as_arr())
        .unwrap()
        .iter()
        .flat_map(|f| f.get("cases").and_then(|c| c.as_arr()).unwrap_or_default().iter())
        .map(|c| (text(c, &["way"]), text(c, &["way_written"])))
        .collect();
    assert!(ways.contains(&(Some("authority.decider".into()), Some("権限表.decider".into()))), "{ways:?}");
    // the stages of a diagnostic's witness
    let v = parsed(&["check", "examples/contract.ja.ringi", "--format", "json"]);
    let d = v.get("diagnostics").and_then(|d| d.as_arr()).unwrap().iter().find(|d| d.get("code").and_then(|c| c.as_str()) == Some("E504")).unwrap();
    let w = d.get("witness").unwrap();
    assert_eq!((w.get("stages").map(|x| x.compact()), w.get("stages_written").map(|x| x.compact())), (Some(r#"["second"]"#.to_string()), Some(r#"["二人目"]"#.to_string())));
    // `simulate`: what the form computes is the value a ticket's JSON holds, with the value as the
    // request writes it; a stage, what its cell writes, its ways and the stage an approval is done
    // with
    let v = parsed(&["simulate", "examples/expense.ja.ringi", "--requester", "e1002", "--form", "種別 = 購買, 明細 = [{品目: \"机\", 金額: 200_000円}]", "--on", "2026-11-16", "--format", "json"]);
    assert_eq!(v.get("computed").map(|c| c.compact()), Some(r#"[{"name":"amount","written":"合計","value":200000,"value_written":"200_000円"}]"#.to_string()));
    let stages = v.get("requesters").and_then(|x| x.as_arr()).unwrap()[0].get("stages").and_then(|x| x.as_arr()).unwrap();
    let second = stages.iter().find(|s| text(s, &["stage"]).as_deref() == Some("second")).unwrap();
    assert_eq!(
        (text(second, &["stage_written"]), text(second, &["spec"]), text(second, &["spec_written"])),
        (Some("二次".into()), Some("accounting".into()), Some("経理".into()))
    );
    let way = &second.get("ways").and_then(|x| x.as_arr()).unwrap()[0];
    assert_eq!(
        (text(way, &["way"]), text(way, &["way_written"]), text(way, &["once_with"]), text(way, &["once_with_written"])),
        (Some("accounting".into()), Some("経理".into()), Some("first".into()), Some("一次".into()))
    );
    // `api`: a choice from a master names the master in ASCII
    let v = parsed(&["api", "examples/account.ja.ringi"]);
    let system = v.get("form").and_then(|f| f.as_arr()).unwrap().iter().find(|f| text(f, &["name"]).as_deref() == Some("system")).unwrap();
    assert_eq!((text(system, &["type"]), text(system, &["type_written"])), (Some("choice from m.systems".into()), Some("choice from マスタ.systems".into())));
}

/// `extract` writes the files the check lays (PLAN C1.2, DESIGN 16.1): the same as
/// `tests/golden/lowered/expense/`, but for the path to the calendar, which is written from `--out`; and
/// what it writes passes rulec's and koyomi's checks where it is. A file that does not pass writes
/// nothing, and one that lowers to nothing says so.
#[test]
fn extract_writes_what_the_check_lays() {
    let tmp = ritsu_testkit::TempDir::new("ringi-extract");
    let dir = tmp.path().join("expense");
    let d = dir.display().to_string();
    let (code, out, err) = joined(&["extract", "examples/expense.ringi", "--out", &d]);
    assert_eq!(code, 0, "{err}");
    let names = ["choices.rule", "form.rule", "next_up.rule", "route.rule", "terms.cal", "values.rule"];
    assert_eq!(out, names.iter().map(|n| format!("wrote {}\n", dir.join(n).display())).collect::<String>());
    let suite = common::joined();
    for n in names {
        let got = std::fs::read_to_string(dir.join(n)).unwrap();
        let model = std::fs::read_to_string(std::path::Path::new("tests/golden/lowered/expense").join(n)).unwrap();
        if n == "terms.cal" {
            let line = |t: &str| t.lines().find(|l| l.starts_with("use calendar ")).unwrap().to_string();
            let cal = std::path::Path::new("examples/shared/office.cal").canonicalize().unwrap();
            assert_eq!(line(&got), format!("use calendar \"{}\"", ritsu_base::paths::between(&dir, &cal)));
            assert_eq!(got.replace(&line(&got), ""), model.replace(&line(&model), ""));
        } else {
            assert_eq!(got, model, "{n}");
        }
        let p = dir.join(n);
        let found = if n.ends_with(".rule") { suite.rules.as_ref().unwrap().findings(&p, Lang::En) } else { suite.dates.as_ref().unwrap().findings(&p, Lang::En) };
        assert!(matches!(&found, Ok(ritsu_ports::Found::Value(v)) if v.is_empty()), "{n}: {found:?}");
    }
    // in Japanese
    let dir = tmp.path().join("expense.ja");
    let d = dir.display().to_string();
    let (code, out, _) = joined(&["extract", "examples/expense.ja.ringi", "--out", &d, "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.starts_with(&format!("生成しました: {}\n", dir.join("choices.rule").display())), "{out}");
    // a file that does not pass: what check prints, exit 1, nothing written
    let dir = tmp.path().join("bad");
    let d = dir.display().to_string();
    let bad = "tests/mutants/E004_a_cell_not_in_who.ringi";
    let (code, out, _) = joined(&["extract", bad, "--out", &d]);
    assert_eq!(code, 1);
    assert_eq!(out, std::fs::read_to_string("tests/golden/E004_a_cell_not_in_who.en.txt").unwrap());
    assert!(!dir.exists());
    // an organization lowers to nothing
    let (code, out, _) = joined(&["extract", "examples/shared/company.ringi", "--out", &d]);
    assert_eq!((code, out.as_str()), (0, "`examples/shared/company.ringi` lowers to nothing\n"));
    assert!(!dir.exists());
    // alone: E008, exit 2, nothing written
    let (code, out, _) = alone(&["extract", "examples/expense.ringi", "--out", &d]);
    assert_eq!(code, 2);
    assert!(out.starts_with("error[E008]: "), "{out}");
    assert!(!dir.exists());
    // the command line
    let (code, _, err) = joined(&["extract", "examples/expense.ringi"]);
    let usage = "usage: ringi extract <file.ringi> --out <dir> [--org <dir>] [--at <date>] [--through <date>]";
    assert_eq!((code, err), (2, format!("error: `ringi extract` needs `--out <dir>`\n{usage}\n")));
    let (code, _, err) = joined(&["extract", "examples/expense.ringi", "examples/leave.ringi", "--out", &d]);
    assert_eq!((code, err), (2, format!("error: `ringi extract` takes <file.ringi>\n{usage}\n")));
}

#[test]
fn explain() {
    let (code, out, _) = joined(&["explain", "e101"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("E101 (error) — "), "{out}");
    let (code, out, _) = joined(&["explain", "E101", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("E101 (エラー) — "), "{out}");
    // a code whose check comes with a later stage is explained all the same
    let (code, out, _) = joined(&["explain", "E701"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("E701 (error) — "), "{out}");
    let (code, out, _) = joined(&["explain", "--all", "--format", "json"]);
    assert_eq!(code, 0);
    assert_eq!(ritsu_base::json::parse(&out).unwrap().as_arr().unwrap().len(), 62);
    let (code, _, err) = joined(&["explain", "E999"]);
    assert_eq!((code, err.as_str()), (2, "error: there is no code `E999`; run `ringi explain --all`\n"));
    let (code, _, err) = joined(&["explain"]);
    let takes = "error: `ringi explain` takes <CODE>... | --all\nusage: ringi explain <CODE>... | --all [--format markdown|json] [--lang ja|en]\n";
    assert_eq!((code, err.as_str()), (2, takes));
    // codes and `--all` are the two ways to call it, not one
    assert_eq!(joined(&["explain", "E101", "--all"]), (2, String::new(), takes.to_string()));
    let (code, out, _) = joined(&["explain", "--all", "--format", "markdown"]);
    assert_eq!(code, 0);
    assert_eq!(out, std::fs::read_to_string("docs/codes.md").unwrap());
    let (code, out, _) = joined(&["explain", "--all", "--format", "markdown", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert_eq!(out, std::fs::read_to_string("docs/codes.ja.md").unwrap());
}

/// `explain` takes any number of codes (ritsu's DESIGN 6.2, item 10): one is written as it is alone,
/// two or more in the order given, as `--all` writes every code (the JSON one array); codes it does
/// not have are named together, each once, and nothing is explained.
#[test]
fn explain_takes_any_number_of_codes() {
    let ledger = ringi::codes::ledger();
    let text = |c: &str| ledger.render_text(ledger.find(c).unwrap(), Lang::En);
    let json = |c: &str| ledger.to_json(ledger.find(c).unwrap(), Lang::En);
    // one code: as it is written alone; the JSON one object
    let (code, out, err) = joined(&["explain", "W502"]);
    assert_eq!((code, out, err.as_str()), (0, text("W502"), ""));
    let (code, out, _) = joined(&["explain", "w502", "--format", "json"]);
    assert_eq!((code, ritsu_base::json::parse(&out).unwrap()), (0, json("W502")));
    // two codes, in the order given, not the ledger's: the texts with a blank line between, and the
    // JSON one array, as `--all` writes them
    let (code, out, _) = joined(&["explain", "W502", "E101"]);
    assert_eq!((code, out), (0, format!("{}\n{}", text("W502"), text("E101"))));
    let (code, out, _) = joined(&["explain", "W502", "E101", "--format", "json"]);
    assert_eq!(code, 0);
    assert_eq!(ritsu_base::json::parse(&out).unwrap(), ritsu_base::json::Json::arr([json("W502"), json("E101")]));
    assert_eq!(out, format!("{}\n", ritsu_base::json::Json::arr([json("W502"), json("E101")]).pretty()));
    let (code, out, _) = joined(&["explain", "E101", "E501", "W502", "--format", "markdown"]);
    let one = |c: &str| ledger.render_markdown_one(ledger.find(c).unwrap(), Lang::En);
    assert_eq!((code, out), (0, format!("{}\n{}\n{}", one("E101"), one("E501"), one("W502"))));
    // in Japanese too
    let (code, out, _) = joined(&["explain", "E101", "E501", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("E101 (エラー) — ") && out.contains("\n\nE501 (エラー) — "), "{out}");
    // codes it does not have: every one in one sentence, in the order given, each once; nothing is
    // explained, not even the codes it has
    assert_eq!(joined(&["explain", "E101", "E999", "X1", "E999"]), (2, String::new(), "error: there are no codes `E999`, `X1`; run `ringi explain --all`\n".to_string()));
    assert_eq!(joined(&["explain", "E999", "X1", "--lang", "ja"]), (2, String::new(), "エラー: `E999`、`X1` というコードはありません。`ringi explain --all` を読んでください\n".to_string()));
    // asked for JSON, what stops it is the JSON's too (ritsu's DESIGN 6.2, item 10)
    let (code, out, _) = joined(&["explain", "E999", "--format", "json"]);
    assert_eq!((code, out.as_str()), (2, "{\"error\":{\"code\":2,\"message\":\"there is no code `E999`; run `ringi explain --all`\"}}\n"));
}

/// What stops a command before its work, as every command of ritsu says it (ritsu's DESIGN 6.2, item
/// 10): what it takes and its usage line, the command named the way it was run; a file that is not
/// there; and, on a line that asks for JSON, the same sentence as the JSON's `error`, whether or not
/// the command takes `--format`.
#[test]
fn what_stops_a_command_is_said_as_ritsu_says_it() {
    // a command it does not have, asked for its help too
    assert_eq!(joined(&["help", "build"]), (2, String::new(), "error: there is no command `build`; run `ringi --help`\n".to_string()));
    // the arguments of each command, and its usage line
    for (args, takes) in [
        (&["api"][..], "`ringi api` takes <file.ringi>\nusage: ringi api <file.ringi> [--org <dir>] [--at <date>] [--through <date>] [--root <dir>]"),
        (&["doc", "a.ringi", "b.ringi"][..], "`ringi doc` takes <file.ringi>\nusage: ringi doc <file.ringi> [--format markdown|html] [--longest [--on <date>]] [--org <dir>] [--at <date>] [--through <date>] [--root <dir>] [--lang ja|en]"),
        (&["diff", "a.ringi"][..], "`ringi diff` takes <old.ringi> <new.ringi>\nusage: ringi diff (<old.ringi> <new.ringi> | --versions <date> <date> <file.ringi>) [--format text|markdown|html|json] [--org <dir>] [--at <date>] [--root <dir>] [--budget <n>] [--lang ja|en]"),
        (
            &["diff", "--versions", "2026-04-01", "examples/expense.ringi"][..],
            "`ringi diff --versions` takes <date> <date> <file.ringi>\nusage: ringi diff (<old.ringi> <new.ringi> | --versions <date> <date> <file.ringi>) [--format text|markdown|html|json] [--org <dir>] [--at <date>] [--root <dir>] [--budget <n>] [--lang ja|en]",
        ),
    ] {
        assert_eq!(joined(args), (2, String::new(), format!("error: {takes}\n")), "{args:?}");
    }
    // in Japanese, the usage line under its own head
    let (code, _, err) = joined(&["api", "--lang", "ja"]);
    assert_eq!((code, err.as_str()), (2, "エラー: `ringi api` に渡すのは <file.ringi> です\n使い方: ringi api <file.ringi> [--org <dir>] [--at <date>] [--through <date>] [--root <dir>]\n"));
    // a budget of the walk, which `--versions` does not walk, and one that is no positive integer
    let (code, _, err) = joined(&["diff", "--versions", "2026-04-01", "2027-04-01", "examples/expense.ringi", "--budget", "10"]);
    assert_eq!((code, err.as_str()), (2, "error: `--budget` caps the walk that compares the routes of two files: it does not go with `--versions`\n"));
    let (code, _, err) = joined(&["diff", "examples/leave.ringi", "examples/leave.ringi", "--budget", "0"]);
    assert_eq!((code, err.as_str()), (2, "error: `--budget 0` is not a positive integer\n"));
    // the same sentences on the standard output, as the JSON's `error`, when the line asks for JSON:
    // after what the command wrote before it stopped (the file it checked before the one not there)
    let (code, out, err) = joined(&["check", "examples/shared/masters.ringi", "nowhere.ringi", "--format", "json"]);
    assert_eq!((code, err.as_str()), (2, "error: `nowhere.ringi` is not there\n"));
    let last = out.trim_end().rsplit('\n').next().unwrap();
    assert_eq!(last, "{\"error\":{\"code\":2,\"message\":\"`nowhere.ringi` is not there\"}}");
    assert_eq!(ritsu_base::json::parse(out.trim_end().strip_suffix(last).unwrap()).unwrap().get("ok"), Some(&ritsu_base::json::Json::from(true)));
    // a command that takes no `--format` (the table refuses the flag), and one that takes `--format
    // json` stopped by what it takes
    let (code, out, err) = joined(&["extract", "examples/expense.ringi", "--format", "json"]);
    let said = err.strip_prefix("error: ").and_then(|s| s.strip_suffix('\n')).unwrap();
    assert!(code == 2 && said.starts_with("unknown flag `--format`"), "{err}");
    assert_eq!(out, format!("{}\n", ritsu_base::json::Json::obj([("error", ritsu_base::json::Json::obj([("code", ritsu_base::json::Json::int(2)), ("message", ritsu_base::json::Json::str(said))]))]).compact()));
    let (code, out, _) = joined(&["simulate", "examples/expense.ringi", "--format", "json"]);
    let said = "`ringi simulate` takes one of `--requester` and `--every-requester`\nusage: ringi simulate <file.ringi> (--requester <id> | --every-requester) [--form <values>] [--on <date>] [--org <dir>] [--format json]";
    assert_eq!((code, out), (2, format!("{}\n", ritsu_base::json::Json::obj([("error", ritsu_base::json::Json::obj([("code", ritsu_base::json::Json::int(2)), ("message", ritsu_base::json::Json::str(said))]))]).compact())));
    // a line that does not ask for JSON writes nothing on the standard output
    assert_eq!(joined(&["simulate", "examples/expense.ringi"]).1, "");
}

#[test]
fn the_binary_of_the_crate() {
    // a request: E008, exit 2, and the command to run instead
    let o = Command::new(env!("CARGO_BIN_EXE_ringi")).args(["check", "examples/expense.ringi"]).env_remove("RINGI_LANG").env_remove("RITSU_LANG").output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.starts_with("error[E008]: examples/expense.ringi:1:1: The file needs kaname, which this ringi does not hold"), "{out}");
    assert!(out.contains("through ritsu: `ritsu ringi check examples/expense.ringi`."), "{out}");
    // the organization and the masters pass
    let o = Command::new(env!("CARGO_BIN_EXE_ringi")).args(["check", "examples/shared/company.ringi", "examples/shared/masters.ringi"]).env_remove("RINGI_LANG").env_remove("RITSU_LANG").output().unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stdout));
    // the language from RINGI_LANG
    let o = Command::new(env!("CARGO_BIN_EXE_ringi")).args(["explain", "E101"]).env("RINGI_LANG", "ja").env_remove("RITSU_LANG").output().unwrap();
    assert!(String::from_utf8_lossy(&o.stdout).starts_with("E101 (エラー) — "));
}

#[test]
fn checked_as_ritsu_check_prints_it() {
    let root = std::path::Path::new(".").canonicalize().unwrap();
    let files = vec!["examples/expense.ringi".to_string(), "examples/shared/company.ringi".to_string(), "examples/nowhere.ringi".to_string()];
    let units = ringi::ports::Engine::new(common::joined()).checked(&root, &files, Lang::En);
    // the warnings of the organization's data, then the line that says it passes, with the days it
    // can be used on
    assert!(units[0].text().starts_with("warning[W502]: examples/expense.ringi:33:93: "), "{}", units[0].text());
    // (the command to run names ritsu's, as `ritsu check` is what runs it)
    let shaped = EXPENSE_SHAPED.replace("`ringi simulate ", "`ritsu ringi simulate ");
    assert!(units[0].text().ends_with(&format!("ok examples/expense.ringi: a request, 8 fields, 4 stages\n{USED}{EXPENSE_ENDS}{shaped}")), "{}", units[0].text());
    assert_eq!(units[0].verdict, ritsu_ports::Verdict::Passes);
    assert_eq!(units[1].text(), COMPANY_OK);
    assert_eq!(units[1].verdict, ritsu_ports::Verdict::Passes);
    assert_eq!(units[2].verdict, ritsu_ports::Verdict::Unchecked);
    // a file that cannot be read, as every command of ritsu says it (ritsu's DESIGN 6.2, item 10)
    assert_eq!(units[2].text(), "error: `examples/nowhere.ringi` is not there\n");
    // with no other language joined, a request is not checked, and an organization is
    let units = ringi::ports::Engine::default().checked(&root, &files[..2], Lang::En);
    assert_eq!(units[0].verdict, ritsu_ports::Verdict::Unchecked);
    assert_eq!(units[1].verdict, ritsu_ports::Verdict::Passes);
}

/// A directory is every `.ringi` file under it, in the order of their paths, but for the ones whose
/// names start with a dot and the directories `extract` writes into (`*.lowered`).
#[test]
fn check_takes_directories() {
    let (code, out, _) = joined(&["check", "examples"]);
    assert_eq!(code, 1, "the contract's errors of the organization's data");
    let said: Vec<&str> = out.lines().filter_map(|l| l.strip_prefix("ok ").or_else(|| l.strip_prefix("error[").and_then(|l| l.split_once("]: ")).map(|(_, r)| r))).map(|l| l.split(':').next().unwrap()).collect();
    let mut files = said.clone();
    files.dedup();
    assert_eq!(files.len(), 13, "{files:?}");
    assert_eq!(files, common::example_files().iter().map(|s| s.as_str()).collect::<Vec<_>>());
    assert!(out.contains(COMPANY_OK), "{out}");
    // the same file named twice, as a file and under a directory, is checked once
    let (_, out, _) = joined(&["check", "examples/shared", "examples/shared/company.ringi"]);
    assert_eq!(out.matches("ok examples/shared/company.ringi:").count(), 1, "{out}");
    // a directory with no .ringi file under it
    let tmp = ritsu_testkit::TempDir::new("ringi-dirs");
    std::fs::create_dir_all(tmp.path().join("x.lowered")).unwrap();
    std::fs::write(tmp.path().join("x.lowered/inside.ringi"), "organization x v1 \"X\"\n").unwrap();
    std::fs::write(tmp.path().join(".hidden.ringi"), "organization x v1 \"X\"\n").unwrap();
    let d = tmp.path().display().to_string();
    let (code, _, err) = joined(&["check", &d]);
    assert_eq!((code, err), (2, format!("error: there is no .ringi file under `{d}`\n")));
}

/// `--org` reads the data from another directory for every organization of the run; `--at` checks
/// the version in effect on one day; with no version that day, nothing about the approvers is
/// decided (W500), and the organization's line says why.
#[test]
fn check_with_the_data_of_a_day() {
    // the contract's findings of both versions, and of 2026-04-01 alone: it fails in the version
    // laid ahead, where the general counsel has left, and passes with warnings in the one before
    let (code, out, _) = joined(&["check", "examples/contract.ringi"]);
    assert_eq!(code, 1);
    assert!(out.contains("In the organization from 2027-04-01, `counsel` names someone who is not there"), "{out}");
    let (code, out, _) = joined(&["check", "examples/contract.ringi", "--at", "2026-06-01"]);
    assert_eq!(code, 0);
    // the general counsel's own request goes up from `counsel`, as the shape of the organization does
    assert!(out.contains("  requesters whose stages (`manager`, `counsel`) go elsewhere or are skipped by the shape of the organization: 2 of 24 in the organization from 2026-04-01\n") && !out.contains("2027-04-01"), "{out}");
    // the organization's file says which version it checked
    let (code, out, _) = joined(&["check", "examples/shared/company.ringi", "--at", "2026-11-16"]);
    assert_eq!(
        (code, out.as_str()),
        (0, "ok examples/shared/company.ringi: an organization, 5 positions, 5 kinds of team\n  data `../org`: the version from 2026-04-01, the one in effect on 2026-11-16 (`--at`), 14 teams and 24 people, 2 proxies\n")
    );
    let (_, out, _) = joined(&["check", "examples/shared/company.ringi", "--at", "2026-11-16", "--lang", "ja"]);
    assert_eq!(out, "ok examples/shared/company.ringi: 組織、役職 5 個、チームの種類 5 個\n  データ `../org`: 2026-11-16（`--at`）に効いている 2026-04-01 からの版、チーム 14 個と 24 人、代理 2 行\n");
    // a day before the first version
    let (code, out, _) = joined(&["check", "examples/leave.ringi", "examples/shared/company.ringi", "--at", "2025-01-01"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("warning[W500]: examples/leave.ringi:5:1: No version of the organization is in effect on 2025-01-01, the day `--at` gives, so whether each stage has an approver is not decided\n"), "{out}");
    assert!(out.contains("  = Give `--at` a day on or after 2026-04-01, or add a version's directory that takes effect by 2025-01-01.\n"), "{out}");
    assert!(out.ends_with("ok examples/shared/company.ringi: an organization, 5 positions, 5 kinds of team\n  no data: no version is in effect on 2025-01-01, the day `--at` gives; the first takes effect on 2026-04-01\n"), "{out}");
    // an organization with no `data` line, given its data with `--org`: W500 goes, and the
    // president, who has no manager, is found
    let w500 = "tests/mutants/W500_an_organization_with_no_data.ringi";
    let (code, out, _) = joined(&["check", w500]);
    assert_eq!(code, 0);
    assert!(out.starts_with("warning[W500]: "), "{out}");
    let tmp = ritsu_testkit::TempDir::new("ringi-org");
    let v = tmp.path().join("2026-04-01");
    std::fs::create_dir_all(&v).unwrap();
    std::fs::write(v.join("teams.csv"), "id,name,kind,parent,head\ncompany,Example Co. (fictional),company,,p1\nsales,Sales,department,company,p2\n").unwrap();
    std::fs::write(v.join("people.csv"), "id,name\np1,Head of the company\np2,Head of Sales\np3,Sales staff\n").unwrap();
    std::fs::write(v.join("members.csv"), "person,team,position,primary\np1,company,dept_head,yes\np2,sales,dept_head,yes\np3,sales,staff,yes\n").unwrap();
    let d = tmp.path().display().to_string();
    let (code, out, _) = joined(&["check", w500, "--org", &d]);
    assert_eq!(code, 1, "{out}");
    assert!(out.starts_with("error[E502]: tests/mutants/W500_an_organization_with_no_data.ringi:9:10: In the organization from 2026-04-01, the stage `check` has no approver\n"), "{out}");
    assert!(out.contains("  = For 1 requester: p1 (Head of the company).\n"), "{out}");
}

/// `--through` holds a request to being usable through a day: past the end of a calendar's data,
/// E404 at the deadline that counts in it; on a day before, it passes and says its end.
#[test]
fn check_through_a_day() {
    let tmp = ritsu_testkit::TempDir::new("ringi-through");
    for f in ["finite.cal", "holidays.csv"] {
        std::fs::copy(std::path::Path::new("tests/mutants/shared").join(f), tmp.path().join(f)).unwrap();
    }
    let src = "request leave v1 \"Leave\"\n\nuse calendar office from \"finite.cal\"\n\nform\n  first_day : date \"First day\"  range >=2026-01-01 <=2030-12-31\n  reason    : text \"Reason\"  count <=200\n\nroute\n  check  managers\n    due in 3 business days at end of day\n\nwho\n  managers = team managers\n\nticket\n  self approval  not allowed\n";
    let f = tmp.path().join("leave.ringi");
    std::fs::write(&f, src).unwrap();
    let f = f.display().to_string();
    let (code, out, _) = joined(&["check", &f]);
    assert_eq!(code, 0, "{out}");
    // the calendar's file as the request's path reaches it
    let cal = tmp.path().join("finite.cal").display().to_string();
    let used = format!("  used from 2026-01-01: the first day of the data of the calendar `office` (`{cal}`)\n  used through 2027-12-28: the data of the calendar `office` (`{cal}`) ends on 2027-12-31, and the deadlines of a stage started later cannot be counted in it\n  ends within 3 business days at the most\n");
    assert!(out.ends_with(&used), "{out}");
    let (code, out, _) = joined(&["check", &f, "--through", "2027-06-30"]);
    assert_eq!(code, 0, "{out}");
    let (code, out, _) = joined(&["check", &f, "--through", "2028-03-31"]);
    assert_eq!(code, 1);
    assert!(out.contains("error[E404]: ") && out.contains(":11:1: The request cannot be used through 2028-03-31 (`--through`): it can be used through 2027-12-28\n"), "{out}");
    let (code, out, _) = joined(&["check", &f, "--through", "2028-03-31", "--lang", "ja"]);
    assert_eq!(code, 1);
    assert!(out.contains("2028-03-31（`--through`）まで使えません。使える最後の日は 2027-12-28 です\n"), "{out}");
    // the port hands over the same days, with the data that decides each end (DESIGN 9.4, 9.6)
    use ritsu_ports::Requests;
    let engine = ringi::ports::Engine::new(common::joined());
    let u = engine.usable(std::path::Path::new(&f)).unwrap();
    let (from, through) = (u.from.unwrap(), u.through.unwrap());
    assert_eq!((ritsu_ports::day_text(from.day), from.by.len(), from.by[0].kind.as_str()), ("2026-01-01".to_string(), 1, "calendar"));
    assert_eq!((ritsu_ports::day_text(through.day), through.by[0].kind.as_str(), through.by[0].name.as_str(), through.by[0].data.map(ritsu_ports::day_text)), ("2027-12-28".to_string(), "calendar", "office", Some("2027-12-31".to_string())));
    assert_eq!(through.by[0].file, cal);
    // a request that reads the organization alone: from its first version, with no end
    let u = engine.usable(std::path::Path::new("examples/leave.ringi")).unwrap();
    let from = u.from.unwrap();
    assert_eq!((ritsu_ports::day_text(from.day), from.by[0].kind.as_str(), from.by[0].file.as_str()), ("2026-04-01".to_string(), "organization", "examples/org"));
    assert!(u.through.is_none());
    // a file that does not pass: what its errors say
    let said = engine.usable(std::path::Path::new("tests/mutants/E004_a_cell_not_in_who.ringi")).unwrap_err();
    assert_eq!(said.iter().map(|s| s.code.as_str()).collect::<Vec<_>>(), ["E004"]);
}

const FORM: &str = "category = purchase, items = [{what: \"Server\", price: 800_000JPY}, {what: \"Storage\", price: 400_000JPY}]";

/// `simulate` (DESIGN 16.1): who answers each stage for one requester, the values of the form and a
/// day, and when each stage is due; every requester, grouped; the JSON; and what stops it.
#[test]
fn simulate_shows_who_answers() {
    let (code, out, err) = joined(&["simulate", "examples/expense.ringi", "--requester", "e1201", "--form", FORM, "--on", "2026-11-16"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(
        out,
        "examples/expense.ringi on 2026-11-16, in the organization from 2026-04-01
  computed: amount = 1_200_000JPY
requester e1201 (Sales staff 1)
  first   manager: e1101 (Manager of Section 1A)
          due 2026-11-19 at the end of the day, reminder on 2026-11-18 09:00
  second  dept_head and accounting
            dept_head: e3101 (Head of Sales Department 1)
            accounting: one of e1001 (Head of Accounting), e1002 (Accountant)
          due 3 business days after it starts, at the end of the day
  decide  division_head: e0101 (Head of Sales Division)
          due 3 business days after it starts, at the end of the day
"
    );
    let (code, out, _) = joined(&["simulate", "examples/expense.ringi", "--requester", "e1201", "--form", FORM, "--on", "2026-11-16", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("examples/expense.ringi（2026-11-16、2026-04-01 からの組織）\n  計算：amount = 1_200_000JPY\n申請者 e1201（営業担当 1）\n"), "{out}");
    assert!(out.contains("          期限は 2026-11-19 の終わり、催促は 2026-11-18 09:00\n"), "{out}");
    // the JSON: the people by their IDs
    let (code, out, _) = joined(&["simulate", "examples/expense.ringi", "--requester", "e1201", "--form", FORM, "--on", "2026-11-16", "--format", "json"]);
    assert_eq!(code, 0);
    let v = ritsu_base::json::parse(&out).unwrap();
    let keys: Vec<&str> = v.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["ringi", "file", "on", "on_given", "today_in", "version", "computed", "may_not_submit", "requesters"]);
    // the day is the one `--on` gives
    assert_eq!((v.get("on_given").and_then(|x| x.as_bool()), v.get("today_in").map(|x| x.is_null())), (Some(true), Some(true)));
    let r = &v.get("requesters").and_then(|x| x.as_arr()).unwrap()[0];
    let stages = r.get("stages").and_then(|x| x.as_arr()).unwrap();
    let first: Vec<&str> = stages[0].get("ways").and_then(|x| x.as_arr()).unwrap()[0].get("people").and_then(|x| x.as_arr()).unwrap().iter().filter_map(|p| p.as_str()).collect();
    assert_eq!(first, ["e1101"]);
    // `same approver approves once` (DESIGN 5.12): the stages the approver of `first` answers next
    // are done with that approval, and have no due
    let desk = "category = purchase, items = [{what: \"Desk\", price: 200_000JPY}]";
    let (code, out, err) = joined(&["simulate", "examples/expense.ringi", "--requester", "e1002", "--form", desk, "--on", "2026-11-16"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(
        out,
        "examples/expense.ringi on 2026-11-16, in the organization from 2026-04-01
  computed: amount = 200_000JPY
requester e1002 (Accountant)
  first   manager: e1001 (Head of Accounting)
          due 2026-11-19 at the end of the day, reminder on 2026-11-18 09:00
  second  accounting: e1001 (Head of Accounting); done with the approval of `first` (`same approver approves once`)
  decide  dept_head: e1001 (Head of Accounting); done with the approval of `first` (`same approver approves once`)
"
    );
    let (code, out, _) = joined(&["simulate", "examples/expense.ringi", "--requester", "e1002", "--form", desk, "--on", "2026-11-16", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.contains("\n  second  accounting：e1001（経理部長）。`first` の承認で済みます（`same approver approves once`）\n"), "{out}");
    let (code, out, _) = joined(&["simulate", "examples/expense.ringi", "--requester", "e1002", "--form", desk, "--on", "2026-11-16", "--format", "json"]);
    assert_eq!(code, 0);
    let v = ritsu_base::json::parse(&out).unwrap();
    let r = &v.get("requesters").and_then(|x| x.as_arr()).unwrap()[0];
    let once: Vec<(Option<&str>, bool)> = r
        .get("stages")
        .and_then(|x| x.as_arr())
        .unwrap()
        .iter()
        .map(|s| {
            let way = &s.get("ways").and_then(|x| x.as_arr()).unwrap()[0];
            (way.get("once_with").and_then(|x| x.as_str()), s.get("due").is_some_and(|d| !d.is_null()))
        })
        .collect();
    assert_eq!(once, [(None, true), (Some("first"), false), (Some("first"), false)]);
    // a way that looks up a master: with the form, the row the form chose alone
    let (code, out, _) = joined(&["simulate", "examples/account.ringi", "--requester", "e1201", "--form", "system = crm, level = read", "--on", "2026-11-16"]);
    assert_eq!(code, 0);
    assert!(out.contains("\n  owner       system_owner (the row `crm` of the master `systems`): e3101 (Head of Sales Department 1)\n"), "{out}");
    assert!(!out.contains("`erp`") && !out.contains("`wiki`"), "{out}");
    // with no form: every stage the route can come to; a stage with nobody is said as it comes
    let (code, out, _) = joined(&["simulate", "examples/contract.ringi", "--requester", "e9002", "--on", "2027-05-10"]);
    assert_eq!(code, 0);
    assert!(out.contains("  second   legal: 2 approvals are needed, and only 1 can give one (e9003 (Lawyer 3))\n"), "{out}");
    assert!(out.contains("  counsel  counsel: e9001 is not in the organization; it cannot go up\n"), "{out}");
    // every requester, the ones who come to the same approvers together; the leave request is
    // `submitted by everyone except team board` (DESIGN 5.13): the members of the board are not among
    // them, and are named at the head
    let (code, out, _) = joined(&["simulate", "examples/leave.ringi", "--every-requester", "--on", "2027-04-01"]);
    assert_eq!(code, 0);
    assert!(out.lines().filter(|l| l.starts_with("requester") || l.starts_with("requesters")).count() >= 2, "{out}");
    assert_eq!(
        out.lines().nth(2),
        Some("  5 people may not submit it (`submitted by everyone except team board`): e0001 (President (fictional)), e0201 (Director 1), e0202 (Director 2) and 2 more"),
        "{out}"
    );
    assert!(!out.contains("e0201 (Director 1)\n") && !out.contains("requester e0001"), "{out}");
    let (code, out, _) = joined(&["simulate", "examples/leave.ringi", "--every-requester", "--on", "2027-04-01", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.contains("\n  出せない人 5 人（`submitted by everyone except team board`）：e0001（社長（架空））、e0201（取締役 1）、e0202（取締役 2）、ほか 2 人\n"), "{out}");
    let (code, out, _) = joined(&["simulate", "examples/leave.ringi", "--every-requester", "--on", "2027-04-01", "--format", "json"]);
    assert_eq!(code, 0);
    let v = ritsu_base::json::parse(&out).unwrap();
    let not: Vec<&str> = v.get("may_not_submit").and_then(|x| x.as_arr()).unwrap().iter().filter_map(|x| x.as_str()).collect();
    assert_eq!(not, ["e0001", "e0201", "e0202", "e0203", "e0204"]);
}

/// `simulate` without `--on` (DESIGN 10.3): today is the day the instant is in, in the UTC offset of
/// the calendars the request uses (its own and its shared route's), as its deadlines are read; in
/// UTC where none says one; and the head and the JSON say how. Calendars that say different offsets
/// read different days, and are refused for `--on` to give the day. The instant is given, not read
/// from the clock: 2026-11-14T15:16:00Z is 00:16 on 2026-11-15 in +09:00.
#[test]
fn simulate_reads_today_in_the_offset_of_the_calendars() {
    use ringi::check::{Options, Run};
    use ringi::simulate::{Ask, On, Today, day_at, offset_text};
    let day = |d: &str| ritsu_ports::day_of(d).unwrap();
    let after_midnight = day("2026-11-14") * 86_400 + 15 * 3_600 + 16 * 60;
    let before_midnight = day("2026-11-14") * 86_400 + 14 * 3_600 + 59 * 60;
    let ask = |path: &str, who: &str, on: On| {
        let run = Run::with(common::joined(), Options { unlisted: true, ..Options::default() });
        let o = run.file(path).unwrap();
        ringi::simulate::simulate(&o, &run, &Ask { requester: Some(who.to_string()), every: false, form: None, on })
    };
    let office = || Some(Today { offset: 540, calendars: vec!["office".to_string()], ascii: vec!["office".to_string()] });
    // the calendar of the leave request (and of its shared route) is `+09:00`
    let a = ask("examples/leave.ringi", "e1201", On::Now(after_midnight)).unwrap();
    assert_eq!((a.on, a.today.clone()), (day("2026-11-15"), office()));
    let b = ask("examples/leave.ringi", "e1201", On::Now(before_midnight)).unwrap();
    assert_eq!((b.on, b.today.clone()), (day("2026-11-14"), office()));
    // a request with no calendar reads it in UTC
    let utc = ask("tests/mutants/W510_nobody_answers_the_president.ringi", "e03", On::Now(after_midnight)).unwrap();
    assert_eq!((utc.on, utc.today.clone()), (day("2026-11-14"), Some(Today { offset: 0, calendars: Vec::new(), ascii: Vec::new() })));
    // `--on`: the day given
    let given = ask("examples/leave.ringi", "e1201", On::Day(day("2026-11-16"))).unwrap();
    assert_eq!((given.on, given.today.clone()), (day("2026-11-16"), None));
    // the head says how today was read
    let render = |path: &str, a: &ringi::simulate::Answer, lang: Lang| ringi::simulate::render(path, a.org_version().unwrap(), a, lang);
    let shown = render("examples/leave.ringi", &a, Lang::En);
    assert!(
        shown.starts_with("examples/leave.ringi on 2026-11-15, in the organization from 2026-04-01\n  today, read in the UTC offset of the calendar `office` (+09:00), as the deadlines are; `--on <date>` gives another day\n"),
        "{shown}"
    );
    let shown = render("examples/leave.ringi", &a, Lang::Ja);
    assert!(shown.starts_with("examples/leave.ringi（2026-11-15、2026-04-01 からの組織）\n  今日を、カレンダー `office` の UTC オフセット +09:00 で読みました（期限の時刻と同じ読み方です）。ほかの日は `--on <日付>` で選べます\n"), "{shown}");
    let f = "tests/mutants/W510_nobody_answers_the_president.ringi";
    let shown = render(f, &utc, Lang::En);
    assert!(shown.starts_with(&format!("{f} on 2026-11-14, in the organization from 2026-04-01\n  today, read in UTC: the request uses no calendar that says its UTC offset; `--on <date>` gives another day\n")), "{shown}");
    let shown = render(f, &utc, Lang::Ja);
    assert!(shown.contains("\n  今日を UTC で読みました（オフセットを書いたカレンダーを使わない申請です）。ほかの日は `--on <日付>` で選べます\n"), "{shown}");
    assert!(!render("examples/leave.ringi", &given, Lang::En).contains("today"));
    // and the JSON
    let how = |a: &ringi::simulate::Answer| {
        let v = ringi::simulate::to_json("x.ringi", a, Lang::En);
        let t = v.get("today_in").unwrap();
        let cals: Option<Vec<String>> = t.get("calendars").and_then(|x| x.as_arr()).map(|cs| cs.iter().filter_map(|c| c.as_str()).map(str::to_string).collect());
        (v.get("on").and_then(|x| x.as_str()).map(str::to_string), v.get("on_given").and_then(|x| x.as_bool()), t.get("offset").and_then(|x| x.as_str()).map(str::to_string), cals)
    };
    let s = |x: &str| Some(x.to_string());
    assert_eq!(how(&a), (s("2026-11-15"), Some(false), s("+09:00"), Some(vec!["office".to_string()])));
    assert_eq!(how(&utc), (s("2026-11-14"), Some(false), s("+00:00"), Some(Vec::new())));
    assert_eq!(how(&given), (s("2026-11-16"), Some(true), None, None));
    // the calendars by their ASCII aliases, with the aliases as written: the request's own `use`
    // (`本社(office)`), then the one of the shared route it uses (`office`), which reads the same
    // file
    let ja = ask("examples/leave.ja.ringi", "e1201", On::Now(after_midnight)).unwrap();
    assert_eq!(ja.today, Some(Today { offset: 540, calendars: vec!["本社".to_string(), "office".to_string()], ascii: vec!["office".to_string(), "office".to_string()] }));
    let v = ringi::simulate::to_json("x.ringi", &ja, Lang::En);
    assert_eq!(v.get("today_in").map(|c| c.compact()), Some(r#"{"offset":"+09:00","calendars":["office","office"],"calendars_written":["本社","office"]}"#.to_string()));
    // the day of an instant at an offset, and an offset as a calendar writes it
    assert_eq!([day_at(0, 0), day_at(86_399, 0), day_at(-1, 0), day_at(0, -300), day_at(15 * 3_600, 540)], [0, 0, -1, -1, 1]);
    assert_eq!([offset_text(540), offset_text(-330), offset_text(0)], ["+09:00", "-05:30", "+00:00"]);
    // two calendars that say different offsets: refused without `--on`, whatever the instant
    let tmp = ritsu_testkit::TempDir::new("ringi-two-offsets");
    let shared = std::path::Path::new("tests/mutants/shared");
    std::fs::create_dir_all(tmp.path().join("org/2026-04-01")).unwrap();
    for f in ["members.csv", "people.csv", "teams.csv"] {
        std::fs::copy(shared.join("org-small/2026-04-01").join(f), tmp.path().join("org/2026-04-01").join(f)).unwrap();
    }
    std::fs::write(tmp.path().join("company.ringi"), std::fs::read_to_string(shared.join("small.ringi")).unwrap().replace("data \"org-small\"", "data \"org\"")).unwrap();
    std::fs::copy(shared.join("office.cal"), tmp.path().join("office.cal")).unwrap();
    std::fs::write(tmp.path().join("ny.cal"), "calendar ny v1\ndescription \"Saturdays and Sundays are closed, in New York time (fictional)\"\noffset -05:00\n\nclosed weekly sat, sun\n").unwrap();
    let leave = "request leave v1 \"Leave\"

use organization org from \"company.ringi\"
use calendar office from \"office.cal\"
use calendar ny from \"ny.cal\"

form
  days : number \"Days\"  range >=1 <=20

route
  check  manager

who
  manager = manager of requester

every stage
  due in 3 business days in office at end of day

ticket
  self approval  allowed
";
    std::fs::write(tmp.path().join("leave.ringi"), leave).unwrap();
    let path = tmp.path().join("leave.ringi").display().to_string();
    let refused = "the calendars the request uses say different UTC offsets (`office` +09:00, `ny` -05:00), and which day today is is not decided: give the day with `--on <date>`";
    for now in [after_midnight, before_midnight] {
        let why = ask(&path, "e03", On::Now(now)).err().unwrap();
        assert_eq!(why.en, refused);
        assert_eq!(why.ja, "申請が使うカレンダーの UTC オフセットが違い（`office` +09:00、`ny` -05:00）、今日がどの日かが決まりません。`--on <日付>` で日を選んでください");
    }
    assert_eq!(joined(&["simulate", &path, "--requester", "e03"]), (2, String::new(), format!("error: {refused}\n")));
    let (code, out, err) = joined(&["simulate", &path, "--requester", "e03", "--on", "2026-11-16"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert!(out.starts_with(&format!("{path} on 2026-11-16, in the organization from 2026-04-01\n  without `--form`")), "{out}");
}

/// With `self approval not allowed`, the requester answers no stage of their own request, as an
/// approver nor as an approver's proxy (DESIGN 5.5, 5.8): e1101 is e3101's proxy for the expense from
/// 2026-12-28, and is shown as one on another's request, not on their own; in the text and in the JSON.
#[test]
fn a_requester_is_no_proxy_on_their_own_request() {
    let e = "examples/expense.ringi";
    let proxy = "[e1101 (Manager of Section 1A) too, as the proxy of e3101]";
    let (code, out, err) = joined(&["simulate", e, "--requester", "e1201", "--on", "2026-12-29"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert!(out.contains(proxy), "{out}");
    let (code, out, err) = joined(&["simulate", e, "--requester", "e1101", "--on", "2026-12-29"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert!(out.contains("e3101 (Head of Sales Department 1)") && !out.contains("as the proxy of"), "{out}");
    let (_, out, _) = joined(&["simulate", e, "--requester", "e1101", "--on", "2026-12-29", "--format", "json"]);
    assert!(out.contains("\"proxies\": []") && !out.contains("\"proxy\": \"e1101\""), "{out}");
    let (_, out, _) = joined(&["simulate", e, "--requester", "e1201", "--on", "2026-12-29", "--format", "json"]);
    assert!(out.contains("\"proxy\": \"e1101\""), "{out}");
    // every requester: e1101's own group shows no proxy of e1101, and the others' do
    let (code, out, _) = joined(&["simulate", e, "--every-requester", "--on", "2026-12-29"]);
    assert_eq!(code, 0);
    assert!(out.contains(proxy), "{out}");
    // each group: the line that names its requesters, and its stages
    let blocks: Vec<&str> = out.split("\nrequester").skip(1).collect();
    let of = |who: &str| blocks.iter().find(|b| b.lines().next().is_some_and(|l| l.contains(who))).copied().unwrap_or_else(|| panic!("{out}"));
    let own = of("e1101 (Manager of Section 1A)");
    assert!(own.contains("e3101 (Head of Sales Department 1)") && !own.contains("as the proxy of"), "{own}");
    assert!(of("e1201 (Sales staff 1)").contains(proxy), "{out}");
}

#[test]
fn what_stops_simulate() {
    let run = |args: &[&str]| {
        let (code, out, err) = joined(args);
        (code, if err.is_empty() { out } else { err })
    };
    let e = "examples/expense.ringi";
    // what it takes, and its usage line (ritsu's DESIGN 6.2, item 10): one requester or every one
    let usage = "usage: ringi simulate <file.ringi> (--requester <id> | --every-requester) [--form <values>] [--on <date>] [--org <dir>] [--format json]";
    let one_of = format!("error: `ringi simulate` takes one of `--requester` and `--every-requester`\n{usage}\n");
    assert_eq!(run(&["simulate", e]), (2, one_of.clone()));
    assert_eq!(run(&["simulate", e, "--requester", "e1201", "--every-requester"]), (2, one_of));
    assert_eq!(run(&["simulate", e, "examples/leave.ringi", "--requester", "e1201"]), (2, format!("error: `ringi simulate` takes <file.ringi>\n{usage}\n")));
    assert_eq!(run(&["simulate", "examples/shared/company.ringi", "--requester", "e1201"]), (2, "error: `simulate` tries a request's file\n".to_string()));
    assert_eq!(run(&["simulate", e, "--requester", "e9999", "--on", "2026-11-16"]), (2, "error: e9999 is not in the organization from 2026-04-01\n".to_string()));
    assert_eq!(run(&["simulate", e, "--requester", "e1201", "--on", "2025-01-01"]), (2, "error: no version of the organization is in effect on 2025-01-01\n".to_string()));
    assert_eq!(run(&["simulate", e, "--requester", "e1201", "--form", "bogus = 1", "--on", "2026-11-16"]), (2, "error: the form has no field `bogus`\n".to_string()));
    assert_eq!(
        run(&["simulate", "examples/leave.ringi", "--requester", "e0201", "--on", "2026-11-16"]),
        (2, "error: e0201 (Director 1) may not submit leave.ringi in the organization from 2026-04-01 (`submitted by everyone except team board`)\n".to_string())
    );
    assert_eq!(
        run(&["simulate", "examples/account.ringi", "--requester", "e1201", "--form", "system = hr, level = read", "--on", "2026-11-16"]),
        (2, "error: `hr` of `system` is not a row of the master `systems` to choose on 2026-11-16\n".to_string())
    );
    assert_eq!(
        run(&["simulate", e, "--requester", "e1201", "--form", "category = purchase, items = [{what: \"Server\", price: 1_200_000JPY}]", "--on", "2026-11-16"]),
        (2, "error: `1_200_000JPY` is outside the range of `items[0].price` (`>=1JPY <=1_000_000JPY`)\n".to_string())
    );
    // a value of a field the form hides for the values is refused in the machine's words (the
    // refusal `form`), in text and in JSON alike; shown, it is taken
    let hidden = "category = purchase, trip_to = \"Osaka\", items = [{what: \"Server\", price: 1_000JPY}]";
    let said = "error: `trip_to` is not shown for this request (its `shown when` does not hold), and takes no value\n".to_string();
    assert_eq!(run(&["simulate", e, "--requester", "e1201", "--form", hidden, "--on", "2026-11-16"]), (2, said.clone()));
    assert_eq!(run(&["simulate", e, "--requester", "e1201", "--form", hidden, "--on", "2026-11-16", "--format", "json"]), (2, said));
    assert_eq!(
        run(&["simulate", "examples/expense.ja.ringi", "--requester", "e1201", "--form", "種別 = 購買, 行き先 = \"大阪\", 明細 = [{品目: \"サーバー\", 金額: 1_000円}]", "--on", "2026-11-16", "--lang", "ja"]),
        (2, "エラー: `行き先` は、この申請では見せないフィールドです（`shown when` が成り立たない）。値は送りません\n".to_string())
    );
    let (code, _) = run(&["simulate", e, "--requester", "e1201", "--form", "category = travel, trip_to = \"Osaka\", items = [{what: \"Train\", price: 1_000JPY}]", "--on", "2026-11-16"]);
    assert_eq!(code, 0);
    assert_eq!(
        run(&["simulate", "tests/mutants/W500_an_organization_with_no_data.ringi", "--requester", "e01"]),
        (2, "error: the organization has no data (the organization's file has no `data` line, and no `--org` is given)\n".to_string())
    );
    // a file that does not pass: what check prints, exit 1
    let bad = "tests/mutants/E004_a_cell_not_in_who.ringi";
    let (code, out) = run(&["simulate", bad, "--requester", "e01"]);
    assert_eq!((code, out), (1, std::fs::read_to_string("tests/golden/E004_a_cell_not_in_who.en.txt").unwrap()));
}

/// `extract` takes the flags of `check` that change what is lowered (`--org`, `--at`, `--through`),
/// and writes nothing when the request cannot be used through the day `--through` gives; the
/// organization's data does not stop it (the approvers are listed by `check`, not lowered).
#[test]
fn extract_takes_the_flags_of_check() {
    let tmp = ritsu_testkit::TempDir::new("ringi-extract-flags");
    let dir = tmp.path().join("contract");
    let d = dir.display().to_string();
    // the contract's E502 and E504 are the organization's data, which extract does not list
    let (code, out, err) = joined(&["extract", "examples/contract.ringi", "--out", &d]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(dir.join("route.rule").is_file());
    let dir = tmp.path().join("leave");
    let d = dir.display().to_string();
    let (code, _, err) = joined(&["extract", "examples/leave.ringi", "--out", &d, "--at", "2027-04-01", "--through", "2030-03-31"]);
    assert_eq!(code, 0, "{err}");
    let (code, _, err) = joined(&["extract", "examples/leave.ringi", "--out", &d, "--at", "someday"]);
    assert_eq!((code, err.as_str()), (2, "error: `--at someday` is not a date such as 2027-04-01\n"));
}
