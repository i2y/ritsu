//! `constraint` (§15.55): a relation between inputs that says which combinations happen.
//!
//! What is tested is the whole line the feature draws — the checker stops demanding rows for
//! what cannot happen, the witnesses it does hand back are reachable, the vectors respect it,
//! every generated language refuses a violating input at the door, and an example that breaks
//! one is an error rather than a case.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rulec-constraint-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

/// Two counts where one can never exceed the other: the shape a rule takes when a predicate
/// narrows a set and the answer depends on how many are left.
const RULE: &str = r#"rule 納入先判定(dest_of) v1
description "絞った件数から、自動で確定してよいかを決める"

enum 判定(verdict) = 自動確定(auto) | 候補複数(many) | 該当なし(none)

inputs
  会社名一致数(name_hits) : number  range >=0 <=999
  全条件一致数(all_hits)  : number  range >=0 <=999

constraint 全条件一致数 <= 会社名一致数

outputs
  結果(verdict) : 判定

table 判定表(verdict_of)
policy unique
| 全条件一致数 | 会社名一致数 | -> 結果(verdict) : 判定 |
| 0            | 0            | 該当なし                |
| 0            | >=1          | 候補複数                |
| >=1          | >=1          | 自動確定                |

examples
| 全条件一致数 | 会社名一致数 | -> 結果 |
| 1            | 3            | 自動確定 |
"#;

fn write(d: &PathBuf, name: &str, body: &str) -> String {
    let p = d.join(name);
    std::fs::write(&p, body).unwrap();
    p.to_str().unwrap().to_string()
}

#[test]
fn 起きない組み合わせには行を要求しない() {
    let d = dir("gap");
    let with = write(&d, "with.rule", RULE);
    let without = write(&d, "without.rule", &RULE.replace("constraint 全条件一致数 <= 会社名一致数\n\n", ""));

    // Without the constraint the table is incomplete, and the witness is the combination the
    // constraint is there to rule out.
    let (code, out, _) = run(&["check", &without, "--format", "json"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("\"code\":\"E101\""), "{out}");
    assert!(out.contains("\"全条件一致数\":1") && out.contains("\"会社名一致数\":0"), "{out}");

    // With it, the same table is complete over everything that can happen.
    let (code, out, e) = run(&["check", &with]);
    assert_eq!(code, 0, "{out}{e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn ベクタは制約を満たすものだけ() {
    let d = dir("vectors");
    let p = write(&d, "r.rule", RULE);
    let (code, out, e) = run(&["vectors", &p]);
    assert_eq!(code, 0, "{e}");
    let mut n = 0;
    for line in out.lines().filter(|l| !l.trim().is_empty()) {
        let j = rulec::json::parse(line).unwrap();
        let get = |k: &str| j.get("in").and_then(|i| i.get(k)).and_then(|v| v.as_int()).unwrap();
        assert!(get("全条件一致数") <= get("会社名一致数"), "制約を破るベクタ: {line}");
        n += 1;
    }
    assert!(n > 0, "ベクタが空");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn 生成物は制約を破る入力を断る() {
    if !have("python3") {
        eprintln!("skip: python3 が無い");
        return;
    }
    let d = dir("guard");
    let p = write(&d, "r.rule", RULE);
    let out = d.join("out");
    let (code, o, e) = run(&["gen", &p, "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{o}{e}");

    // Every language writes the door; Python is the one run here.
    for (lang, file) in [
        ("python", "dest_of.py"),
        ("typescript", "dest_of.ts"),
        ("javascript", "dest_of.mjs"),
        ("rust", "dest_of.rs"),
        ("ruby", "dest_of.rb"),
        ("go", "destof/dest_of.go"),
        ("swift", "dest_of.swift"),
        ("sql", "dest_of.sql"),
    ] {
        let body = std::fs::read_to_string(out.join(lang).join(file)).unwrap_or_default();
        assert!(
            body.contains("全条件一致数 <= 会社名一致数"),
            "{lang}: 生成物に制約の門が無い"
        );
    }

    let script = "import sys; sys.path.insert(0, '.')\n\
                  import dest_of as m\n\
                  try:\n\
                  \x20   m.dest_of(0, 1)\n\
                  \x20   print('NO ERROR')\n\
                  except m.RuleInputError as e:\n\
                  \x20   print('refused:', e)\n";
    let o = Command::new("python3")
        .current_dir(out.join("python"))
        .args(["-B", "-c", script])
        .output()
        .expect("python3 を起動できない");
    let said = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(said.starts_with("refused:"), "断っていない: {said}{}", String::from_utf8_lossy(&o.stderr));
    assert!(said.contains("全条件一致数 <= 会社名一致数"), "何を破ったか言っていない: {said}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn 制約を破る例は誤り() {
    let d = dir("example");
    let p = write(&d, "r.rule", &RULE.replace("| 1            | 3            | 自動確定 |", "| 3            | 1            | 自動確定 |"));
    let (code, out, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("\"code\":\"E019\""), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn 形と型が違えば断る() {
    let d = dir("bad");
    // No comparison at all.
    let a = write(&d, "a.rule", &RULE.replace("constraint 全条件一致数 <= 会社名一致数", "constraint 全条件一致数"));
    let (code, out, _) = run(&["check", &a, "--format", "json"]);
    assert_eq!(code, 1);
    assert!(out.contains("\"code\":\"E017\""), "{out}");

    // A side that is not an input.
    let b = write(&d, "b.rule", &RULE.replace("constraint 全条件一致数 <= 会社名一致数", "constraint 全条件一致数 <= 結果"));
    let (code, out, _) = run(&["check", &b, "--format", "json"]);
    assert_eq!(code, 1);
    assert!(out.contains("\"code\":\"E018\""), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// The approver is told what the checks were allowed to assume (§1.6).
#[test]
fn 資料は起きない組み合わせを載せる() {
    let d = dir("doc");
    let p = write(&d, "r.rule", RULE);
    let (code, out, e) = run(&["doc", &p, "--lang", "ja"]);
    assert_eq!(code, 0, "{e}");
    assert!(out.contains("起きない組み合わせ"), "{out}");
    assert!(out.contains("全条件一致数 <= 会社名一致数"), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Two rates of one type but different steps, related by a constraint. The door used to
/// compare the stored integers as they stood — 50 for 50% in 1% steps, 100 for 10% in 0.1%
/// steps — and let 50% ≤ 10% through (§15.139). Both sides are brought to the step they
/// share now, and the door refuses exactly what the relation says does not happen.
const STEPS: &str = r#"rule 割引上限(cap_of) v1
description "割引率は上限率を超えない。刻みの違う二つの率を比べる"

enum 判定(verdict) = 通常(plain) | 特別(special)

inputs
  割引率(rate) : rate[step 1%]    range >=0% <=100%
  上限率(cap)  : rate[step 0.1%]  range >=0% <=100%

constraint 割引率 <= 上限率

outputs
  結果(verdict) : 判定

table 判定表(t)
policy unique
| 割引率 | -> 結果(verdict) : 判定 |
| <=10%  | 通常                   |
| >10%   | 特別                   |

examples
| 割引率 | 上限率 | -> 結果 |
| 5%     | 20%    | 通常    |
| 30%    | 30%    | 特別    |
"#;

#[test]
fn 刻みの違う率どうしの制約を門が正しく比べる() {
    if !have("python3") {
        eprintln!("skip: python3 が無い");
        return;
    }
    let d = dir("steps");
    let p = write(&d, "r.rule", STEPS);
    let out = d.join("out");
    let (code, o, e) = run(&["gen", &p, "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{o}{e}");
    // 50% in 1% steps is 50; 10% in 0.1% steps is 100. 50% ≤ 10% is false, so the door has to
    // refuse it — and 5% against 20% (5 and 200) has to pass.
    let script = "import sys; sys.path.insert(0, '.')\n\
                  import cap_of as m\n\
                  for a, b in [(50, 100), (5, 200), (30, 300)]:\n\
                  \x20   try:\n\
                  \x20       m.cap_of(a, b)\n\
                  \x20       print(a, b, 'ok')\n\
                  \x20   except m.RuleInputError as e:\n\
                  \x20       print(a, b, 'refused')\n";
    let o = Command::new("python3")
        .current_dir(out.join("python"))
        .args(["-B", "-c", script])
        .output()
        .expect("python3 を起動できない");
    let said = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(said.contains("50 100 refused"), "50% ≤ 10% を通している: {said}{}", String::from_utf8_lossy(&o.stderr));
    assert!(said.contains("5 200 ok") && said.contains("30 300 ok"), "成り立つ組を断っている: {said}");
    let _ = std::fs::remove_dir_all(&d);
}

/// A rate passed in with no step counted whole units of 100%, so `10%` in its range became
/// `1` at the door (§15.139). It is refused now, with the step to write.
#[test]
fn 刻みの無い率の入力は止まる() {
    let src = STEPS.replace("割引率(rate) : rate[step 1%]", "割引率(rate) : rate");
    let d = dir("nostep");
    let p = write(&d, "r.rule", &src);
    let (code, o, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1, "{o}");
    assert!(o.contains("\"E103\""), "{o}");
    assert!(o.contains("rate[step 1%]"), "刻みの書き方を言っていない: {o}");
    let _ = std::fs::remove_dir_all(&d);
}
