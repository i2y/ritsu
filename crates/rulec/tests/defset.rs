//! Definition sets (DESIGN-draft §2): several tables defining one output, ordered by
//! `overrides`; row labels; and what the evaluator, the generators and the page do with them.

use std::collections::HashMap;

const SPLIT: &str = "tests/corpus/印紙税の本則と軽減.rule";

fn read(rel: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap()
}

fn codes(src: &str) -> Vec<String> {
    rulec::check_source(src, "t.rule").iter().map(|d| d.code.to_string()).collect()
}

fn checked(src: &str) -> (rulec::ast::RuleFile, rulec::types::Checked) {
    let p = rulec::parse::parse(src, "t.rule");
    assert!(p.diags.is_empty(), "{:?}", p.diags.iter().map(|d| d.code).collect::<Vec<_>>());
    let f = p.file.expect("parsed");
    let c = rulec::types::check(&f, "t.rule");
    (f, c)
}

#[test]
fn 本則と軽減の二表は通る() {
    let src = read(SPLIT);
    // At its real path: the copies of its sources sit beside it (§15.68).
    let cs: Vec<String> = rulec::check_source(&src, SPLIT).iter().map(|d| d.code.to_string()).collect();
    assert!(cs.iter().all(|c| c.starts_with('W')), "エラーが出た: {cs:?}");
    assert!(!cs.contains(&"W117".to_string()), "効いている例外に W117 が出た");
}

#[test]
fn 二表は一つの定義集合にまとまり_例外が先に試される() {
    let src = read(SPLIT);
    let (_, c) = checked(&src);
    let set = c.sets.iter().find(|s| s.merged()).expect("merged set");
    assert_eq!(set.key, "印紙税額");
    assert_eq!(set.members, vec!["本則".to_string(), "非課税".to_string(), "軽減".to_string()]);
    assert_eq!(set.clause, vec![false, true, false]);
    assert_eq!(set.eval_at, "軽減");
    // Evaluation order: the rows of 軽減 first, then the clause, then 本則, each as written.
    assert_eq!(set.row_table(0), "軽減");
    assert_eq!(set.table.rows[0].index, 1);
    let exempt = set.table.rows.iter().position(|r| r.origin.as_deref() == Some("非課税")).unwrap();
    assert!(set.is_clause_row(exempt));
    let first_base = set.table.rows.iter().position(|r| r.origin.as_deref() == Some("本則")).unwrap();
    assert!(exempt < first_base);
    assert_eq!(set.table.rows[first_base].index, 1);
    assert_eq!(set.table.rows[first_base].label.as_ref().map(|l| l.text.as_str()), Some("記載なし"));
    // Every row of 本則 is beaten by every row of 軽減, and by nothing else.
    for j in first_base..set.table.rows.len() {
        assert_eq!(set.beats[j], (0..exempt).collect::<Vec<_>>());
    }
    // Rows of 軽減 (unique) beat nothing among themselves; the clause is ordered with nothing.
    assert!(set.beats[0].is_empty());
    assert!(set.beats[exempt].is_empty());
}

#[test]
fn 評価は勝った表の行を書かれた位置で記録する() {
    let src = read(SPLIT);
    let (f, c) = checked(&src);
    use rulec::eval::Val;
    use rulec::num::Rat;
    let run = |amount: i128, stated: bool, (y, m, d): (i32, u32, u32)| {
        let mut ins: HashMap<String, Val> = HashMap::new();
        ins.insert("契約金額".into(), Val::Num(Rat::int(amount)));
        ins.insert("金額の記載あり".into(), Val::Bool(stated));
        ins.insert("作成日".into(), Val::Date(y, m, d));
        let (outs, _, fired, _) = rulec::eval::run_all_traced(&f, &c, ins);
        (outs, fired)
    };
    // 3,000万円 in the reduced period: 軽減 row 5 wins, 10,000円.
    let (outs, fired) = run(30_000_000, true, (2026, 9, 16));
    assert_eq!(fired, vec![("軽減".to_string(), 5)]);
    assert_eq!(outs[0].1, Some(Val::Num(Rat::int(10_000))));
    // The day after the period: 本則 row 7, 20,000円 — and only that one row fires.
    let (outs, fired) = run(30_000_000, true, (2027, 4, 1));
    assert_eq!(fired, vec![("本則".to_string(), 7)]);
    assert_eq!(outs[0].1, Some(Val::Num(Rat::int(20_000))));
    // A labelled row reports its position, and the label is looked up from it.
    let (_, fired) = run(0, false, (2026, 9, 16));
    assert_eq!(fired, vec![("本則".to_string(), 1)]);
    assert_eq!(c.label_of("本則", 1), Some("記載なし"));
    assert_eq!(c.label_of("本則", 7), Some("r8"));
    assert_eq!(c.label_of("軽減", 1), None);
    // A clause fires as the one row of a table named after it.
    let (outs, fired) = run(5_000, true, (2026, 9, 16));
    assert_eq!(fired, vec![("非課税".to_string(), 1)]);
    assert_eq!(outs[0].1, Some(Val::Num(Rat::int(0))));
}

#[test]
fn 順序の無い交わりはE105で_優先を書けば消える() {
    let base = "\
rule t(t) v1

inputs
  a(a) : number range >=0 <=10

outputs
  x(x) : number round down(1)

table 甲(ko)
| a   | -> x |
| <5  | 1    |
| >=5 | 2    |

table 乙(otsu)
{OVERRIDES}| a      | -> x |
| >=3 <7 | 3    |
";
    let unordered = base.replace("{OVERRIDES}", "");
    let cs = codes(&unordered);
    assert_eq!(cs.iter().filter(|c| *c == "E105").count(), 2, "{cs:?}");
    // With the precedence written, the two overlaps are decided and nothing is reported;
    // neither row of 甲 is covered entirely, so no E102 either.
    let ordered = base.replace("{OVERRIDES}", "overrides 甲\n");
    let cs = codes(&ordered);
    assert!(cs.is_empty(), "{cs:?}");
}

#[test]
fn 優先する表に丸ごと覆われた行はE102() {
    let src = "\
rule t(t) v1

inputs
  a(a) : number range >=0 <=10

outputs
  x(x) : number round down(1)

table 甲(ko)
| a   | -> x |
| <5  | 1    |
| >=5 | 2    |

table 乙(otsu)
overrides 甲
| a | -> x |
| - | 3    |
";
    let ds = rulec::check_source(src, "t.rule");
    let e102: Vec<_> = ds.iter().filter(|d| d.code == "E102").collect();
    assert_eq!(e102.len(), 2, "{:?}", ds.iter().map(|d| d.code).collect::<Vec<_>>());
    assert!(e102.iter().all(|d| d.notes.iter().any(|n| n.contains("表 乙"))), "the note names the winning table");
}

#[test]
fn 指す先の誤りと出力の食い違いと二出力の共有() {
    let e035 = "\
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool

table 乙(otsu)
overrides 甲
| a | -> x |
| - | true |

table 甲(ko)
| a | -> x  |
| - | false |
";
    assert!(codes(e035).contains(&"E035".to_string()), "a target declared below");
    let e036 = "\
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool
  y(y) : bool

table 甲(ko)
| a | -> x |
| - | true |

table 乙(otsu)
overrides 甲
| a | -> y |
| - | true |
";
    assert!(codes(e036).contains(&"E036".to_string()));
    let e045 = "\
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool
  y(y) : bool

table 甲(ko)
| a | -> x | y |
| - | true | true |

table 乙(otsu)
overrides 甲
| a    | -> x  |
| true | false |
";
    assert!(codes(e045).contains(&"E045".to_string()));
}

#[test]
fn ラベルの重複と予約語() {
    let dup = "\
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)
r1 | a     | -> x  |
r1 | true  | true  |
r1 | false | false |
";
    // A label on the header line is not a label (the header starts with a bar); the two rows
    // below share one.
    let _ = dup;
    let dup2 = dup.replace("r1 | a     | -> x  |", "| a     | -> x  |");
    assert_eq!(codes(&dup2).iter().filter(|c| *c == "E034").count(), 1);
    let kw = dup2.replace("r1 | false", "policy | false");
    assert!(codes(&kw).contains(&"E009".to_string()), "a keyword as a label is E009");
}

#[test]
fn fmtはラベルを一列に揃えて冪等() {
    let src = "\
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)
| a | -> x |
記載なし | true | true |
r2 | false | false |
";
    let once = rulec::fmt::format(src);
    assert!(once.contains("         | a     | -> x  |\n記載なし | true  | true  |\nr2       | false | false |\n"), "{once}");
    assert_eq!(rulec::fmt::format(&once), once, "not idempotent");
}

// ── Clauses (DESIGN-draft §2.2) ─────────────────────────────────────────────

const PROVISO: &str = "tests/corpus/送料のただし書.rule";

#[test]
fn 節は一行の表として集合に入り_例外が先に試される() {
    let src = read(PROVISO);
    let (f, c) = checked(&src);
    let set = c.sets.iter().find(|s| s.key == "送料").expect("the set of 送料");
    assert_eq!(set.members, vec!["通常".to_string(), "無料".to_string()]);
    assert_eq!(set.clause, vec![true, true]);
    // 無料 (declared later, `overrides 通常`) is tried first; its one row beats 通常's.
    assert_eq!(set.row_table(0), "無料");
    assert_eq!(set.row_table(1), "通常");
    assert_eq!(set.beats[1], vec![0]);
    // `when always` is a row with no cells of its own; the merged table fills the columns.
    let regular = f.items.iter().find_map(|it| match it {
        rulec::ast::Item::Table(t) if t.clause && t.name.as_ref().is_some_and(|n| n.text == "通常") => Some(t),
        _ => None,
    }).unwrap();
    assert!(regular.inputs.is_empty() && regular.rows[0].cells.is_empty());
    assert_eq!(set.table.inputs.len(), 2);

    use rulec::eval::Val;
    use rulec::num::Rat;
    let run = |dest: &str, member: bool, total: i128| {
        let mut ins: HashMap<String, Val> = HashMap::new();
        ins.insert("あて先".into(), Val::Enum(dest.into()));
        ins.insert("サイズ".into(), Val::Enum("S60".into()));
        ins.insert("会員".into(), Val::Bool(member));
        ins.insert("注文金額".into(), Val::Num(Rat::int(total)));
        rulec::eval::run_all_traced(&f, &c, ins)
    };
    let (outs, _, fired, _) = run("北海道", true, 3900);
    assert_eq!(fired, vec![("運賃表".to_string(), 1), ("無料".to_string(), 1)]);
    assert_eq!(outs[0].1, Some(Val::Num(Rat::int(0))));
    let (outs, _, fired, _) = run("北海道", true, 3899);
    assert_eq!(fired, vec![("運賃表".to_string(), 1), ("通常".to_string(), 1)]);
    assert_eq!(outs[0].1, Some(Val::Num(Rat::int(1150))));
}

#[test]
fn 節の形の誤りはE046() {
    let ok = "\
rule t(t) v1

inputs
  a(a) : bool
  n(n) : number range >=0 <=9

outputs
  x(x) : bool

clause 例外(exception) -> x
  when a true and n >=5
  then false

clause 本文(base) -> x
  when always
  then true
";
    // Written the other way round: the exception is declared after what it excepts.
    let ordered = ok.replace("clause 本文(base) -> x\n  when always\n  then true\n", "")
        + "clause 本文(base) -> x\n  when always\n  then true\n";
    let _ = ordered;
    let good = "\
rule t(t) v1

inputs
  a(a) : bool
  n(n) : number range >=0 <=9

outputs
  x(x) : bool

clause 本文(base) -> x
  when always
  then true

clause 例外(exception) -> x
  when a true and n >=5
  then false
  overrides 本文
";
    assert!(codes(good).is_empty(), "{:?}", codes(good));
    for (broken, what) in [
        (good.replace("  then false\n", ""), "then が無い"),
        (good.replace("  when a true and n >=5\n", ""), "when が無い"),
        (good.replace("when a true and n >=5", "when a true and a false"), "同じ列が二度"),
        (good.replace("when a true and n >=5", "when a true and n"), "条件の無い列"),
        (good.replace("clause 例外(exception) -> x", "clause 例外(exception)"), "出力が無い"),
    ] {
        assert!(codes(&broken).contains(&"E046".to_string()), "{what}: {:?}", codes(&broken));
    }
    // The two clauses of one output cannot share a name with a table or each other.
    let dup = good.replace("clause 例外(exception) -> x", "clause 本文(base2) -> x");
    assert!(codes(&dup).contains(&"E034".to_string()), "{:?}", codes(&dup));
}

#[test]
fn fmtは節の本体を二字下げにして冪等() {
    let src = "\
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool

clause 本文(base) -> x
when   always
     then   true   # 注記
";
    let once = rulec::fmt::format(src);
    assert!(once.contains("clause 本文(base) -> x\n  when always\n  then true  # 注記\n"), "{once}");
    assert_eq!(rulec::fmt::format(&once), once, "not idempotent");
}
