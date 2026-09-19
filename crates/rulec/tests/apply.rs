//! A rule applied by another (§15.69, §15.66): the callee is read beside
//! the rule, held to its pinned digest, checked on its own, and expanded into the rule under
//! the apply's name with the bound inputs substituted. What is pinned here is the shape of the
//! expansion the rest of the tool sees, and the flow from a changed callee to a new pin.

use rulec::ast::Item;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const CALLER: &str = "tests/corpus/非常勤退職手当.rule";
const CALLEE: &str = "tests/corpus/退職手当.rule";

fn prepared(rel: &str) -> (rulec::ast::RuleFile, rulec::types::Checked) {
    let src = std::fs::read_to_string(root().join(rel)).unwrap();
    rulec::prepare(&src, &root().join(rel).to_string_lossy()).unwrap_or_else(|ds| panic!("{rel} は検査を通らない: {ds:?}"))
}

#[test]
fn 呼び先の定義は呼び出しの名前の下に展開される() {
    let (f, c) = prepared(CALLER);
    let tables: Vec<String> = f
        .items
        .iter()
        .filter_map(|it| if let Item::Table(t) = it { t.name.as_ref().map(|n| n.text.clone()) } else { None })
        .collect();
    // 支給表 and the main clause come in; the excepted 減額 does not.
    assert_eq!(tables, vec!["退職手当:支給表", "退職手当:本則"]);
    for it in &f.items {
        if let Item::Table(t) = it {
            assert_eq!(t.applied.as_deref(), Some("退職手当"), "展開した表は呼び出しの名前を持つ");
        }
    }
    // The bound inputs are this rule's names; the callee's own define is renamed.
    let schedule = f.items.iter().find_map(|it| match it {
        Item::Table(t) if t.name.as_ref().is_some_and(|n| n.text == "退職手当:支給表") => Some(t),
        _ => None,
    }).unwrap();
    let cols: Vec<&str> = schedule.inputs.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(cols, vec!["在職期間", "任期終了事由"]);
    let defines: Vec<String> = f.items.iter().filter_map(|it| if let Item::Define(d) = it { Some(d.name.text.clone()) } else { None }).collect();
    assert!(defines.contains(&"退職手当:満額".to_string()), "{defines:?}");
    assert!(!defines.iter().any(|d| d == "退職手当:減額後"), "除いた節しか読まない定義は消える: {defines:?}");
    assert!(defines.contains(&"非常勤手当".to_string()), "呼び先の出力はこの規則の定義になる: {defines:?}");
    // The enum cells are spelled in this rule's values, and a row naming only a value that
    // stands for nothing here (死亡) is gone.
    assert_eq!(schedule.rows.len(), 3);
    assert!(c.ty_of("退職手当:支給月数").is_some());
    // The apply remembers what it learned.
    let a = &f.applies[0];
    assert_eq!(a.hash.as_deref(), Some("fb21d081458c197e"));
    // 支給表, 満額, 本則 and the definition of 非常勤手当 (the callee has no `result`, so
    // its output is the clause's column itself).
    assert_eq!(a.count, 4);
    assert_eq!(a.defines, vec!["非常勤手当"]);
    assert!(a.callee.as_ref().is_some_and(|k| k.rule.text == "退職手当"));
}

#[test]
fn 呼び先の行は呼び先のとおりに当たり_trace_は呼び出しの名前で言う() {
    let (f, c) = prepared(CALLER);
    let mut inputs = std::collections::HashMap::new();
    inputs.insert("在職期間".to_string(), rulec::eval::Val::Num(rulec::num::Rat::int(3)));
    inputs.insert("任期終了事由".to_string(), rulec::eval::Val::Enum("辞職".into()));
    inputs.insert("報酬月額".to_string(), rulec::eval::Val::Num(rulec::num::Rat::int(300000)));
    let (outs, fired, _, _) = rulec::eval::run_all_traced(&f, &c, inputs);
    assert_eq!(outs[0].1, Some(rulec::eval::Val::Num(rulec::num::Rat::int(1500000))), "減額は準用しないので満額");
    assert_eq!(fired, vec!["表 退職手当:支給表 行1", "表 退職手当:本則 行1"]);
    // The label travels with the row, under the table's name in this rule.
    assert_eq!(c.label_of("退職手当:支給表", 1), Some("短期"));
}

#[test]
fn 固定と違う呼び先は_e040_で止まり_pin_が見出しを書き換える() {
    let dir = std::env::temp_dir().join(format!("rulec-apply-pin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(root().join(CALLEE), dir.join("退職手当.rule")).unwrap();
    let caller = std::fs::read_to_string(root().join(CALLER)).unwrap().replace("sha256:fb21d081458c197e", "sha256:0000000000000000  # 固定");
    let path = dir.join("非常勤退職手当.rule");
    std::fs::write(&path, &caller).unwrap();
    let p = path.to_string_lossy().into_owned();
    let ds = rulec::report(&caller, &p).diags;
    let e040: Vec<_> = ds.iter().filter(|d| d.code == "E040").collect();
    assert_eq!(e040.len(), 1, "{ds:?}");
    assert_eq!(e040[0].fix.text.as_deref(), Some("apply 退職手当(retirement) = \"退職手当.rule\" sha256:fb21d081458c197e"));
    // The check goes on past E040, so the rest of the rule is still judged.
    assert!(ds.iter().all(|d| d.code == "E040" || d.severity != rulec::diag::Severity::Error), "{ds:?}");
    // gen refuses; diff may go on.
    assert!(rulec::prepare(&caller, &p).is_err());
    assert!(rulec::prepare_lenient(&caller, &p).is_ok());
    // pin rewrites the heading's digest and keeps the comment.
    let parsed = rulec::parse::parse(&caller, &p);
    let (text, o) = rulec::sources::pin(parsed.file.as_ref().unwrap(), &p, &caller).unwrap();
    assert!(o.changed);
    assert!(text.contains("sha256:fb21d081458c197e  # 固定"), "{text}");
    assert!(rulec::report(&text, &p).diags.iter().all(|d| d.code != "E040"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 呼び先が無ければ_e044_で止まる() {
    let src = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=1 <=3\n\noutputs\n  y(y) : number  round down(1)\n\napply 呼(c) = \"無い.rule\" sha256:0000000000000000\n  a = n\n  x -> y\n";
    let ds = rulec::report(src, "t.rule").diags;
    assert_eq!(ds.iter().map(|d| d.code).collect::<Vec<_>>(), vec!["E044"]);
}

#[test]
fn 整形は呼び出しの本体を二字下げる() {
    let src = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=1 <=3\n\noutputs\n  y(y) : number  round down(1)\n\napply 呼(c) = \"呼び先.rule\" sha256:0000000000000000\n    a   =  n\nexcept 表\n  x  ->   y\n";
    let out = rulec::fmt::format(src);
    assert!(out.contains("\n  a = n\n  except 表\n  x -> y\n"), "{out}");
}
