//! The element a refusal is about, named by its index (§15.324): `lines[1].subtotal is out of range`.
//!
//! A refusal of one element of a sequence named the field and not the element, so a caller who
//! passed many elements looked for the one at fault by its values. Every reason the door has for
//! one element now names it by its index in the input's array, counted from 0 as JSON counts it,
//! and a reason two elements give together (a `unique` field, a `take_unique`) names both, the
//! earlier first. Where more than one element is at fault, every way names the first.
//!
//! The materials are the two rules below, English first and its Japanese version beside it, which
//! between them have every reason the door has for one element: a field left out; a value of
//! another kind (a string where an integer goes, a fraction, a string where a truth value goes, a
//! number where a string goes, a day the calendar does not have, a value the enum does not have);
//! a whole number past 64 bits; a value outside the range of its field (a number's and a date's);
//! a value worked out for each element outside the range the rule accepts for it; an element that
//! is not an object; two elements with the same value of a `unique` field; and two elements both
//! taking under a `take_unique`. Each is put in three ways: the one element of the sequence, the
//! last of three, and the second and the fourth of four.

use ritsu_ports::{RuleError, Rules, Value, Values};
use ritsu_testkit::run::{finish, finish_with_input};
use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, need, side_by_side, skip};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;

const ENGLISH: &str = r#"rule order_lines_checked v1
description "The wrapping fee of an order: the one gift line that is wrapped pays its price, and two such lines are the rule contradicting itself. A sketch of every reason a door can refuse one element for"

enum line_kind = normal | gift
enum pick = skip | take

elements lines
  sku      : string  unique
  quantity : number  range >=1 <=100
  price    : money[JPY]  range >=1JPY <=100_000JPY
  kind     : line_kind
  wrapped  : bool
  ship_on  : date  range >=2026-01-01 <=2026-12-31

define subtotal : money[JPY] = quantity * price  range >=1JPY <=1_000_000JPY

sum amount over lines of subtotal  range >=0JPY <=100_000_000JPY

outputs
  fee   : money[JPY]  round down(1JPY)
  total : money[JPY]  round down(1JPY)

define total : money[JPY] = amount

table pick_of
policy unique
| kind   | wrapped | -> verdict : pick |
| normal | false   | skip              |
| normal | true    | skip              |
| gift   | false   | skip              |
| gift   | true    | take              |

fold verdict over lines -> fee
  skip      -> next
  take      -> take_unique price
  empty     -> 0JPY
  exhausted -> held
"#;

const JAPANESE: &str = r#"rule 明細の確かめ(order_lines_checked_ja) v1
description "注文の包装料。包装した贈り物の明細が一件あれば、その単価が包装料になる。そうした明細が二件あれば、規則そのものの矛盾になる。入口が要素について断る理由を一通りそろえた見本"

enum 明細区分(line_kind) = 通常(normal) | 贈答(gift)
enum 採否(pick) = 見送り(skip) | 採用(take)

elements 明細(lines)
  品番(sku)      : string  unique
  数量(quantity) : number  range >=1 <=100
  単価(price)    : money[円]  range >=1円 <=100000円
  区分(kind)     : 明細区分
  包装(wrapped)  : bool
  出荷日(ship_on) : date  range >=2026-01-01 <=2026-12-31

define 小計(subtotal) : money[円] = 数量 * 単価  range >=1円 <=1000000円

sum 金額(amount) over 明細 of 小計  range >=0円 <=100000000円

outputs
  包装料(fee) : money[円]  round down(1円)
  合計(total) : money[円]  round down(1円)

define 合計 : money[円] = 金額

table 採否表(pick_of)
policy unique
| 区分 | 包装  | -> 判定(verdict) : 採否 |
| 通常 | false | 見送り                  |
| 通常 | true  | 見送り                  |
| 贈答 | false | 見送り                  |
| 贈答 | true  | 採用                    |

fold 判定 over 明細 -> 包装料
  見送り    -> next
  採用      -> take_unique 単価
  empty     -> 0円
  exhausted -> held
"#;

/// One of the two rules: the language its files are generated in, its file and alias, and the
/// names its input and its sentences have.
struct Names {
    lang: &'static str,
    file: &'static str,
    alias: &'static str,
    src: &'static str,
    lines: &'static str,
    sku: &'static str,
    quantity: &'static str,
    price: &'static str,
    kind: &'static str,
    wrapped: &'static str,
    ship_on: &'static str,
    subtotal: &'static str,
    normal: &'static str,
    gift: &'static str,
    line_kind: &'static str,
    verdict: &'static str,
}

const EN: Names = Names {
    lang: "en",
    file: "order_lines_checked.rule",
    alias: "order_lines_checked",
    src: ENGLISH,
    lines: "lines",
    sku: "sku",
    quantity: "quantity",
    price: "price",
    kind: "kind",
    wrapped: "wrapped",
    ship_on: "ship_on",
    subtotal: "subtotal",
    normal: "normal",
    gift: "gift",
    line_kind: "line_kind",
    verdict: "verdict",
};

const JA: Names = Names {
    lang: "ja",
    file: "明細の確かめ.rule",
    alias: "order_lines_checked_ja",
    src: JAPANESE,
    lines: "明細",
    sku: "品番",
    quantity: "数量",
    price: "単価",
    kind: "区分",
    wrapped: "包装",
    ship_on: "出荷日",
    subtotal: "小計",
    normal: "通常",
    gift: "贈答",
    line_kind: "明細区分",
    verdict: "判定",
};

/// One reason the door has for one element: what makes an element bad (fields put over a good
/// element, `None` leaving one out; none at all for an element that is not an object), the field
/// the sentence names, and the sentence after the place, English and Japanese.
struct Reason {
    tag: &'static str,
    bad: fn(&Names) -> Vec<(&'static str, Option<&'static str>)>,
    field: fn(&Names) -> Option<&'static str>,
    en: &'static str,
    ja: &'static str,
}

fn reasons() -> Vec<Reason> {
    vec![
        Reason { tag: "not an object", bad: |_| Vec::new(), field: |_| None, en: "is not an object", ja: "がオブジェクトではありません" },
        Reason { tag: "a field left out", bad: |n| vec![(n.quantity, None)], field: |n| Some(n.quantity), en: "is missing", ja: "がありません" },
        Reason { tag: "a string for an integer", bad: |n| vec![(n.quantity, Some("\"2\""))], field: |n| Some(n.quantity), en: "is not an integer", ja: "が整数ではありません" },
        Reason { tag: "a fraction", bad: |n| vec![(n.price, Some("1.5"))], field: |n| Some(n.price), en: "is not an integer", ja: "が整数ではありません" },
        Reason { tag: "past 64 bits", bad: |n| vec![(n.quantity, Some("99999999999999999999"))], field: |n| Some(n.quantity), en: "is out of range", ja: "が範囲の外です" },
        Reason { tag: "past the range of a number", bad: |n| vec![(n.quantity, Some("101"))], field: |n| Some(n.quantity), en: "is out of range", ja: "が範囲の外です" },
        Reason { tag: "a day the calendar does not have", bad: |n| vec![(n.ship_on, Some("\"2026-02-30\""))], field: |n| Some(n.ship_on), en: "is not a date", ja: "が日付ではありません" },
        Reason { tag: "past the range of a date", bad: |n| vec![(n.ship_on, Some("\"2027-01-01\""))], field: |n| Some(n.ship_on), en: "is out of range", ja: "が範囲の外です" },
        Reason { tag: "a value the enum does not have", bad: |n| vec![(n.kind, Some("\"other\""))], field: |n| Some(n.kind), en: "is not a value of enum", ja: "が列挙" },
        Reason { tag: "a string for a truth value", bad: |n| vec![(n.wrapped, Some("\"true\""))], field: |n| Some(n.wrapped), en: "is not a boolean", ja: "が真偽ではありません" },
        Reason { tag: "a number for a string", bad: |n| vec![(n.sku, Some("7"))], field: |n| Some(n.sku), en: "is not a string", ja: "が文字列ではありません" },
        Reason {
            tag: "a value worked out for the element past the range the rule accepts",
            bad: |n| vec![(n.quantity, Some("100")), (n.price, Some("10001"))],
            field: |n| Some(n.subtotal),
            en: "is out of range",
            ja: "が範囲の外です",
        },
    ]
}

/// One element, good unless `bad` says otherwise: its fields as the rule declares them, with `sku`,
/// and the fields of `bad` put over them. An element that is not an object is `0`.
fn element(n: &Names, sku: &str, bad: Option<&Reason>) -> String {
    if bad.is_some_and(|r| r.tag == "not an object") {
        return "0".to_string();
    }
    let mut fields: Vec<(&str, Option<String>)> = vec![
        (n.sku, Some(format!("{sku:?}"))),
        (n.quantity, Some("1".into())),
        (n.price, Some("100".into())),
        (n.kind, Some(format!("{:?}", n.normal))),
        (n.wrapped, Some("false".into())),
        (n.ship_on, Some("\"2026-03-01\"".into())),
    ];
    for (k, v) in bad.map(|r| (r.bad)(n)).unwrap_or_default() {
        if let Some(f) = fields.iter_mut().find(|(name, _)| *name == k) {
            f.1 = v.map(str::to_string);
        }
    }
    let body: Vec<String> = fields.into_iter().filter_map(|(k, v)| v.map(|v| format!("{k:?}:{v}"))).collect();
    format!("{{{}}}", body.join(","))
}

/// A gift line that is wrapped, which the fold takes under its `take_unique`.
fn taking(n: &Names, sku: &str) -> String {
    element(n, sku, None).replace(&format!("{:?}", n.normal), &format!("{:?}", n.gift)).replace("false", "true")
}

/// Every input of the cases: why it is there, the input's JSON, and the kind and the sentence every
/// way has to say of it.
fn cases(n: &Names) -> Vec<(String, String, &'static str, String)> {
    let ja = n.lang == "ja";
    let seq = |els: Vec<String>| format!("{{{:?}:[{}]}}", n.lines, els.join(","));
    let mut out = Vec::new();
    for r in reasons() {
        let said = |k: usize| -> String {
            let place = match (r.field)(n) {
                Some(f) => format!("{}[{k}].{f}", n.lines),
                None => format!("{}[{k}]", n.lines),
            };
            match (ja, r.tag) {
                (false, "a value the enum does not have") => format!("{place} {} {}", r.en, n.line_kind),
                (true, "a value the enum does not have") => format!("{place} {} {} の値ではありません", r.ja, n.line_kind),
                (false, _) => format!("{place} {}", r.en),
                (true, _) => format!("{place} {}", r.ja),
            }
        };
        let bad = |i: usize| element(n, &format!("b{i}"), Some(&r));
        let good = |i: usize| element(n, &format!("g{i}"), None);
        out.push((format!("{}: the one element", r.tag), seq(vec![bad(0)]), "input", said(0)));
        out.push((format!("{}: the last of three", r.tag), seq(vec![good(0), good(1), bad(2)]), "input", said(2)));
        out.push((format!("{}: the second and the fourth of four", r.tag), seq(vec![good(0), bad(1), good(2), bad(3)]), "input", said(1)));
    }
    let twice = |a: usize, b: usize| -> String {
        if ja {
            format!("{l} の要素に {s} が同じものが二つあります（{l}[{a}] と {l}[{b}]）", l = n.lines, s = n.sku)
        } else {
            format!("{l} has two elements with the same {s} ({l}[{a}] and {l}[{b}])", l = n.lines, s = n.sku)
        }
    };
    let el = |sku: &str| element(n, sku, None);
    out.push(("a unique value on the first and the third".into(), seq(vec![el("a"), el("b"), el("a")]), "input", twice(0, 2)));
    out.push(("a unique value twice, two pairs".into(), seq(vec![el("a"), el("b"), el("b"), el("a")]), "input", twice(1, 2)));
    out.push(("a unique value on the last two".into(), seq(vec![el("a"), el("b"), el("c"), el("c")]), "input", twice(2, 3)));
    let both = |a: usize, b: usize| -> String {
        if ja {
            format!("畳み込み {v}: take_unique に二件当たりました（{l}[{a}] と {l}[{b}]）", v = n.verdict, l = n.lines)
        } else {
            format!("fold {v}: two elements matched a take_unique ({l}[{a}] and {l}[{b}])", v = n.verdict, l = n.lines)
        }
    };
    out.push(("two taking, the second and the fourth".into(), seq(vec![el("a"), taking(n, "b"), el("c"), taking(n, "d")]), "contradiction", both(1, 3)));
    out.push(("three taking".into(), seq(vec![taking(n, "a"), taking(n, "b"), taking(n, "c")]), "contradiction", both(0, 1)));
    out
}

/// The rule of `n` written into `dir`, and read.
fn prepared(n: &Names, dir: &Path) -> (rulec::ast::RuleFile, rulec::types::Checked) {
    let path = dir.join(n.file);
    std::fs::write(&path, n.src).unwrap();
    let path = path.to_string_lossy().into_owned();
    rulec::prepare(n.src, &path).unwrap_or_else(|_| panic!("{} does not prepare", n.file))
}

fn lang_of(n: &Names) -> rulec::i18n::Lang {
    if n.lang == "ja" { rulec::i18n::Lang::Ja } else { rulec::i18n::Lang::En }
}

/// How long a run of rulec may take: `rulec test` talks to the servers of the generated code and
/// waits for their answers without a limit of its own. Past it, rulec is stopped with every process
/// it started, and the test fails saying what it ran and what it printed until then.
const RULEC_LIMIT: std::time::Duration = std::time::Duration::from_secs(20 * 60);

/// The reference evaluator names the first element at fault by its index, for every reason the
/// door has for one element, in English and in Japanese, and the two elements of a reason two give;
/// `rulec run` says the same.
#[test]
fn the_reference_names_the_first_element_at_fault_by_its_index() {
    for n in [&EN, &JA] {
        let t = TempDir::new(&format!("refused-element-{}", n.lang));
        let (f, c) = prepared(n, t.path());
        let all = cases(n);
        assert_eq!(all.len(), 3 * reasons().len() + 5, "{}", n.lang);
        for (why, input, kind, said) in &all {
            let j = rulec::json::parse(input).unwrap_or_else(|e| panic!("{why}: {e}: {input}"));
            let got = rulec::i18n::with(lang_of(n), || rulec::door::refusal(&f, &c, &j)).unwrap_or_else(|| panic!("[{}] {why}: answered {input}", n.lang));
            assert_eq!((got.kind, got.error.as_str()), (*kind, said.as_str()), "[{}] {why}: {input}", n.lang);
        }
        // `rulec run`, a reason of one element and a reason of two.
        for (why, input, kind, said) in all.iter().filter(|(w, ..)| w == "past the range of a number: the second and the fourth of four" || w == "a unique value on the first and the third") {
            let o = finish_with_input(
                Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", n.lang).current_dir(t.path()).args(["run", n.file, "--input", "-", "--format", "json"]),
                Some(input.as_bytes()),
                RULEC_LIMIT,
                "rulec run",
            );
            assert_eq!(o.code, Some(0), "[{}] {why}: {}{}", n.lang, o.stdout, o.stderr);
            let j = rulec::json::parse(&o.stdout).unwrap_or_else(|e| panic!("{e}: {}", o.stdout));
            let s = |k: &str| j.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string();
            assert_eq!((s("refused"), s("error")), (kind.to_string(), said.clone()), "[{}] {why}", n.lang);
        }
    }
}

/// The input as the values the port takes: None where the port has no value for one of them (a
/// fraction, an element that is not an object).
fn port_values(f: &rulec::ast::RuleFile, c: &rulec::types::Checked, j: &Json) -> Option<Values> {
    use rulec::types::Ty;
    let one = |ty: &Ty, x: &Json| -> Option<Value> {
        Some(match (ty, x) {
            (_, Json::Bool(b)) => Value::Bool(*b),
            (_, Json::Int(n)) => Value::Int(*n),
            (Ty::Enum(_), Json::Str(s)) => Value::Enum(s.clone()),
            (Ty::Date, Json::Str(s)) => Value::Date(s.clone()),
            (_, Json::Str(s)) => Value::Str(s.clone()),
            _ => return None,
        })
    };
    let mut values: Values = Vec::new();
    for el in &f.elements {
        let mut rows = Vec::new();
        for x in j.get(&el.name.text)?.as_arr()? {
            if !matches!(x, Json::Obj(_)) {
                return None;
            }
            let mut row = Vec::new();
            for fd in &el.fields {
                let Some(v) = x.get(&fd.name.text) else { continue };
                row.push((fd.name.text.clone(), one(&c.ty_of(&fd.name.text).unwrap_or(Ty::Unknown), v)?));
            }
            rows.push(row);
        }
        values.push((el.name.text.clone(), Value::List(rows)));
    }
    Some(values)
}

/// The port (`Rules::eval`, which kaname, dandori, sekisho and ritsu-cross call) names the element
/// at its place too, in its own words: a field left out, past its range or of another kind, a value
/// worked out for the element, two elements with the same `unique` value, two elements under a
/// `take_unique`. Wherever it names an element, it is the one the door names.
#[test]
fn the_port_names_the_element_at_its_place() {
    let e = rulec::ports::Engine::new();
    for n in [&EN, &JA] {
        let t = TempDir::new(&format!("refused-element-port-{}", n.lang));
        let (f, c) = prepared(n, t.path());
        let path = t.path().join(n.file);
        let l = n.lines;
        let mut seen = 0;
        for (why, input, kind, _) in cases(n) {
            let j = rulec::json::parse(&input).unwrap();
            let Some(values) = port_values(&f, &c, &j) else { continue };
            let got = e.eval(&path, &values);
            let text = match (&got, kind) {
                (Err(RuleError::Input(t)), "input") | (Err(RuleError::Contradiction(t)), "contradiction") => t.clone(),
                _ => panic!("[{}] {why}: {got:?}", n.lang),
            };
            let want: Option<(String, String)> = match why.as_str() {
                "a field left out: the last of three" => Some((format!("an element's field `{l}[2].{}` is missing", n.quantity), format!("要素のフィールド `{l}[2].{}` がありません", n.quantity))),
                "past the range of a number: the second and the fourth of four" => {
                    Some((format!("an element's field `{l}[1].{}` is outside its range", n.quantity), format!("要素のフィールド `{l}[1].{}` が範囲の外です", n.quantity)))
                }
                "a value worked out for the element past the range the rule accepts: the last of three" => Some((
                    format!("the `{l}[2].{}` it works out is outside the range the rule accepts for it", n.subtotal),
                    format!("計算した `{l}[2].{}` が、受け付ける範囲の外です", n.subtotal),
                )),
                "a number for a string: the one element" => Some((format!("`{l}[0].{}` is given a value that is not a string", n.sku), format!("`{l}[0].{}` に string として読めない値が渡されています", n.sku))),
                "a unique value twice, two pairs" => Some((
                    format!("the list `{l}` has two elements with the same `{}` (`{l}[1]` and `{l}[2]`)", n.sku),
                    format!("並び `{l}` の要素に、`{}` が同じものが二つあります（`{l}[1]` と `{l}[2]`）", n.sku),
                )),
                "two taking, the second and the fourth" => Some((
                    format!("two elements matched a `take_unique` of the fold `{}` (`{l}[1]` and `{l}[3]`)", n.verdict),
                    format!("畳み込み `{}` で、`take_unique` に二つの要素が当たりました（`{l}[1]` と `{l}[3]`）", n.verdict),
                )),
                _ => None,
            };
            if let Some((en, ja)) = want {
                assert_eq!((text.en.as_str(), text.ja.as_str()), (en.as_str(), ja.as_str()), "[{}] {why}", n.lang);
                seen += 1;
            }
            let place = |k: usize| format!("{l}[{k}]");
            let k = (0..4).find(|k| text.en.contains(&place(*k))).unwrap_or_else(|| panic!("[{}] {why}: the port names no element: {}", n.lang, text.en));
            let door = rulec::i18n::with(lang_of(n), || rulec::door::refusal(&f, &c, &j)).unwrap();
            assert!(door.error.contains(&place(k)), "[{}] {why}: the port says {} and the door {}", n.lang, text.en, door.error);
        }
        assert_eq!(seen, 6, "[{}]", n.lang);
    }
}

/// The code `gen` writes for the rule of `n`, every target and extra, with the cases put after the
/// refused inputs `gen` wrote: the directory it is in, the output, and how many refused inputs the
/// file holds.
fn generated(n: &Names) -> (TempDir, PathBuf, usize) {
    let t = TempDir::new(&format!("refused-element-gen-{}", n.lang));
    let (f, c) = prepared(n, t.path());
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    let ran = finish(
        Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", n.lang).current_dir(t.path()).args(rulec::targets::up_to_0_40(&["gen", n.file, "--out", &o, "--lang", n.lang])),
        RULEC_LIMIT,
        "rulec gen",
    );
    assert_eq!(ran.code, Some(0), "{}{}", ran.stdout, ran.stderr);
    let file = out.join("vectors").join(format!("{}.refused.jsonl", n.alias));
    let mut body = std::fs::read_to_string(&file).unwrap();
    for (why, input, kind, said) in cases(n) {
        let j = rulec::json::parse(&input).unwrap();
        let r = rulec::i18n::with(lang_of(n), || rulec::door::refusal(&f, &c, &j)).unwrap();
        assert_eq!((r.kind, r.error.as_str()), (kind, said.as_str()), "[{}] {why}", n.lang);
        body.push_str(&format!("{{\"in\":{},\"refused\":{},\"error\":{},\"why\":{}}}\n", j.compact(), rulec::json::quote(kind), rulec::json::quote(&said), rulec::json::quote(&why)));
    }
    std::fs::write(&file, &body).unwrap();
    let lines = body.lines().count();
    (t, out, lines)
}

/// `rulec test` over a generated directory, as JSON, with a SKIP line for whatever it did not run.
fn rulec_test(out: &Path) -> (i32, Json) {
    let o = finish(
        Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmpdir_in(out)).args(["test", &out.to_string_lossy(), "--format", "json"]),
        RULEC_LIMIT,
        "rulec test running the generated code",
    );
    let j = rulec::json::parse(o.stdout.lines().next().unwrap_or_else(|| panic!("no result\n{}", o.stderr))).unwrap_or_else(|x| panic!("{x}: {}", o.stdout));
    for why in j.get("skipped").and_then(|v| v.as_arr()).unwrap_or_default() {
        skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
    }
    (o.code.unwrap_or(-1), j)
}

/// The ways `rulec test` reports that put the refused inputs: not a proof, not the functions of
/// prerequisites, labels or chances.
fn puts_refused(r: &Json) -> bool {
    !matches!(r.get("via").and_then(|v| v.as_str()), Some("proof" | "prerequisites" | "labels" | "function-labels" | "chances"))
}

/// `rulec test` over the code `gen` writes for the two rules, with the cases put among the refused
/// inputs: every way the rule is reached — each language's runner, the MCP servers, the Connect
/// service, the Wasm module — names the element the reference evaluator names. The English rule and
/// the Japanese version run side by side.
#[test]
fn every_way_names_the_element_the_reference_names() {
    // Every language there is a toolchain for, the tools level's (ritsu's DESIGN 10.2).
    if !need(Need::Python) {
        return;
    }
    side_by_side(&[&EN, &JA], |n| {
        let (_t, out, refused) = generated(n);
        let (code, j) = rulec_test(&out);
        assert_eq!(code, 0, "[{}] {j:?}", n.lang);
        let mut ways = Vec::new();
        for r in j.get("results").and_then(|v| v.as_arr()).unwrap_or_default() {
            if r.get("rule").and_then(|v| v.as_str()) != Some(n.alias) || r.get("ran").and_then(|v| v.as_bool()) != Some(true) || !puts_refused(r) {
                continue;
            }
            assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "[{}] {r:?}", n.lang);
            assert_eq!(r.get("refused").and_then(|v| v.as_int()), Some(refused as i128), "[{}] not every input was put: {r:?}", n.lang);
            ways.push(format!("{}/{}", r.get("lang").and_then(|v| v.as_str()).unwrap_or_default(), r.get("via").and_then(|v| v.as_str()).unwrap_or_default()));
        }
        for way in ["python/runner", "python/mcp", "typescript/runner", "typescript/mcp", "rust/runner"] {
            assert!(ways.iter().any(|w| w == way), "[{}] {way} did not run: {ways:?}", n.lang);
        }
        println!("refused element [{}]: {} ways: {}", n.lang, ways.len(), ways.join(", "));
    });
}

/// One element the Rust caller below builds: its `sku`, quantity and price, and whether it is a gift
/// line that is wrapped (which the fold takes) or a normal line that is not.
type Line = (&'static str, i64, i64, bool);

/// The element as the input's JSON, as `element` writes it, the day of 2026-03-01 among its fields.
fn line_json(n: &Names, (sku, quantity, price, taken): Line) -> String {
    let (kind, wrapped) = if taken { (n.gift, "true") } else { (n.normal, "false") };
    format!(
        "{{{:?}:{sku:?},{:?}:{quantity},{:?}:{price},{:?}:{kind:?},{:?}:{wrapped},{:?}:\"2026-03-01\"}}",
        n.sku, n.quantity, n.price, n.kind, n.wrapped, n.ship_on
    )
}

/// What a caller of the generated Rust module holds when it refuses a call: the `RuleError`'s
/// sentence, a `Said`, shows the sentence the reference evaluator says, the element's index in it
/// (`to_string()`, `Display` and the error's own `Display`); it compares equal by `==` to that
/// sentence and to no string one character shorter or longer, nor to one with another index, or
/// with a digit put after the index; and it hands the pieces around the indices (`words()`) and the
/// indices (`places()`). A sentence with no index is made from a string (`From`) and is that string.
/// The English rule and the Japanese version run side by side.
#[test]
fn the_rust_a_caller_holds_shows_and_compares_the_sentence_with_its_place() {
    if !ritsu_testkit::ready(Need::Rustc, || Command::new("rustc").arg("--version").output().is_ok_and(|o| o.status.success()), "rustc is not here; the Rust a caller holds is not built") {
        return;
    }
    side_by_side(&[&EN, &JA], |n| {
        let t = TempDir::new(&format!("refused-element-said-{}", n.lang));
        let (f, c) = prepared(n, t.path());
        let out = t.path().join("out");
        let ran = finish(
            Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", n.lang).current_dir(t.path()).args(["gen", n.file, "--target", "rust", "--out", &out.to_string_lossy(), "--lang", n.lang]),
            RULEC_LIMIT,
            "rulec gen",
        );
        assert_eq!(ran.code, Some(0), "{}{}", ran.stdout, ran.stderr);
        let module = out.join("rust").join(format!("{}.rs", n.alias));
        let text = std::fs::read_to_string(&module).unwrap();
        // the type of the rule's money, `JPY` or `Yen`
        let money = text.lines().find_map(|l| l.strip_prefix("pub struct ")?.strip_suffix("(pub i64);")).unwrap_or_else(|| panic!("[{}] no money type in the module", n.lang));
        let cases: [(&str, Vec<Line>, Vec<usize>); 4] = [
            ("past the range, the second and the third of three", vec![("a", 1, 100, false), ("b", 101, 100, false), ("c", 101, 100, false)], vec![1]),
            ("worked out past the range, the last of three", vec![("a", 1, 100, false), ("b", 1, 100, false), ("c", 100, 10001, false)], vec![2]),
            ("a unique value on the first and the third", vec![("a", 1, 100, false), ("b", 1, 100, false), ("a", 1, 100, false)], vec![0, 2]),
            ("two taking, the second and the fourth", vec![("a", 1, 100, false), ("b", 1, 100, true), ("c", 1, 100, false), ("d", 1, 100, true)], vec![1, 3]),
        ];
        let mut host = format!(
            "#[allow(dead_code, warnings)]\n#[path = {:?}]\nmod rule;\nuse rule::{{Element, LineKind, RuleError, Said, {money}}};\n\n\
             fn line(sku: &str, quantity: i64, price: i64, taken: bool) -> Element {{\n    \
             let kind = if taken {{ LineKind::Gift }} else {{ LineKind::Normal }};\n    \
             Element {{ sku: sku.to_string(), quantity, price: {money}(price), kind, wrapped: taken, ship_on: 20513 }}\n}}\n\n\
             fn holds(tag: &str, got: Result<rule::Output, RuleError>, kind: &str, said: &str, others: &[&str], words: &[&str], places: &[usize]) {{\n    \
             let (k, what, shown) = match &got {{\n        \
             Err(e @ RuleError::Input {{ what, .. }}) => (\"input\", *what, e.to_string()),\n        \
             Err(e @ RuleError::Contradiction {{ what }}) => (\"contradiction\", *what, e.to_string()),\n        \
             Ok(_) => panic!(\"{{tag}}: answered\"),\n    }};\n    \
             assert_eq!(k, kind, \"{{tag}}\");\n    \
             assert_eq!(what, said, \"{{tag}}\");\n    \
             assert!(what == said, \"{{tag}}\");\n    \
             assert_eq!(what.to_string(), said, \"{{tag}}\");\n    \
             assert_eq!(format!(\"{{what}}\"), said, \"{{tag}}\");\n    \
             assert!(shown == said || shown.strip_prefix(said).is_some_and(|v| v.starts_with(\": \")), \"{{tag}}: {{shown}}\");\n    \
             for o in others {{\n        assert_ne!(what, *o, \"{{tag}}\");\n    }}\n    \
             assert_eq!(what.words(), words, \"{{tag}}\");\n    \
             assert_eq!(what.places(), places, \"{{tag}}\");\n    \
             println!(\"{{tag}}\\t{{k}}\\t{{what}}\");\n}}\n\n\
             fn main() {{\n",
            module.to_string_lossy()
        );
        let mut want = String::new();
        for (tag, lines, places) in &cases {
            let input = format!("{{{:?}:[{}]}}", n.lines, lines.iter().map(|l| line_json(n, *l)).collect::<Vec<_>>().join(","));
            let j = rulec::json::parse(&input).unwrap();
            let r = rulec::i18n::with(lang_of(n), || rulec::door::refusal(&f, &c, &j)).unwrap_or_else(|| panic!("[{}] {tag}: the reference answers {input}", n.lang));
            let said = r.error.clone();
            // the pieces around the indices, as the reference's sentence has them
            let mut words: Vec<String> = Vec::new();
            let mut rest = said.as_str();
            for k in places {
                let at = rest.find(&format!("[{k}]")).unwrap_or_else(|| panic!("[{}] {tag}: no [{k}] in {said}", n.lang)) + 1;
                words.push(rest[..at].to_string());
                rest = &rest[at + k.to_string().len()..];
            }
            words.push(rest.to_string());
            let shorter = &said[..said.char_indices().last().unwrap().0];
            let first = places[0];
            let others = [
                shorter.to_string(),
                format!("{said}."),
                said.replacen(&format!("[{first}]"), &format!("[{}]", first + 1), 1),
                said.replacen(&format!("[{first}]"), &format!("[{first}0]"), 1),
                String::new(),
            ];
            let elements: Vec<String> = lines.iter().map(|(s, q, p, x)| format!("line({s:?}, {q}, {p}, {x})")).collect();
            host.push_str(&format!(
                "    holds({tag:?}, rule::{}(&[{}]), {:?}, {said:?}, &{others:?}, &{words:?}, &{places:?});\n",
                n.alias,
                elements.join(", "),
                r.kind
            ));
            want.push_str(&format!("{tag}\t{}\t{said}\n", r.kind));
        }
        // A sentence with no index in it.
        host.push_str(
            "    let plain: Said = \"no index\".into();\n    assert!(plain == \"no index\" && plain != \"no inde\" && plain != \"no index.\" && plain != \"\");\n    \
             assert!(plain.words() == [\"no index\"] && plain.places().is_empty());\n    \
             assert!(Said::of(\"\") == \"\" && Said::of(\"\") != \"a\");\n    println!(\"no index\\t{plain}\");\n}\n",
        );
        want.push_str("no index\tno index\n");
        let main = t.path().join("main.rs");
        std::fs::write(&main, &host).unwrap();
        let bin = t.path().join("caller");
        let built = finish(
            Command::new("rustc").current_dir(t.path()).args(["--edition", "2021", "main.rs", "-o"]).arg(&bin),
            RULEC_LIMIT,
            "rustc building the caller",
        );
        assert_eq!(built.code, Some(0), "[{}] {}\n{host}", n.lang, built.stderr);
        let said = finish(&mut Command::new(&bin), RULEC_LIMIT, "the caller");
        assert_eq!(said.code, Some(0), "[{}] {}{}", n.lang, said.stdout, said.stderr);
        assert_eq!(said.stdout, want, "[{}]", n.lang);
    });
}

/// The index in a sentence: the number in its first `[…]`, if it has one.
fn first_index(s: &str) -> Option<usize> {
    let from = s.find('[')? + 1;
    let to = from + s[from..].find(']')?;
    s[from..to].parse().ok()
}

/// A way that names another element is caught, for each way it can go wrong: an index off by one
/// (the Python module and the Rust module say the index one past the element), the last element at
/// fault said and not the first (the TypeScript module walks the elements from the last), and no
/// element named (the Go runner says the sentence without its index). Every other language still
/// names the element the reference names.
#[test]
fn a_way_that_names_another_element_is_caught() {
    if !need(Need::Python) {
        return;
    }
    let (_t, out, _) = generated(&EN);
    let edit = |file: &str, from: &str, to: &str| {
        let p = out.join(file);
        let src = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(src.contains(from), "{file} has no `{from}`");
        std::fs::write(&p, src.replace(from, to)).unwrap();
    };
    // An index off by one.
    edit("python/order_lines_checked.py", "f\"lines[{index}].quantity is out of range\"", "f\"lines[{index + 1}].quantity is out of range\"");
    edit("rust/order_lines_checked.rs", "Said::at(\"lines[\", index, \"].quantity is out of range\")", "Said::at(\"lines[\", index + 1, \"].quantity is out of range\")");
    // The last element at fault said, not the first.
    edit("typescript/order_lines_checked.ts", "of lines.entries()", "of [...lines.entries()].reverse()");
    // No element named.
    edit("go/orderlinescheckedrunner/main.go", "fmt.Sprintf(\"lines[%d].quantity is missing\", i)", "\"quantity is missing\"");
    let (code, j) = rulec_test(&out);
    assert_eq!(code, 1, "{j:?}");
    let result = |lang: &str| -> Option<Json> {
        j.get("results")?.as_arr()?.iter().find(|r| r.get("lang").and_then(|v| v.as_str()) == Some(lang) && r.get("via").and_then(|v| v.as_str()) == Some("runner")).cloned()
    };
    // The first line the way says otherwise: its sentence, and the one the reference says.
    let differs = |lang: &str| -> (String, String) {
        let r = result(lang).unwrap_or_else(|| panic!("{lang}/runner did not run: {j:?}"));
        assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(false), "{lang}: {r:?}");
        let d = r.get("first_diff").unwrap_or_else(|| panic!("{lang}: {r:?}"));
        let error = |k: &str| -> String {
            let line = d.get(k).and_then(|v| v.as_str()).unwrap_or_else(|| panic!("{lang}: {r:?}"));
            let said = rulec::json::parse(line).unwrap_or_else(|e| panic!("{lang}: {e}: {line}"));
            said.get("error").and_then(|v| v.as_str()).unwrap_or_else(|| panic!("{lang}: {line}")).to_string()
        };
        (error("generated"), error("expected"))
    };
    for lang in ["python", "rust"] {
        let (g, x) = differs(lang);
        assert!(g.contains("].quantity is out of range") && x.contains("].quantity is out of range"), "{lang}: {g} / {x}");
        assert_eq!(first_index(&g), first_index(&x).map(|k| k + 1), "{lang}: {g} / {x}");
    }
    let (g, x) = differs("typescript");
    assert!(first_index(&g) > first_index(&x), "typescript: {g} / {x}");
    let (g, x) = differs("go");
    assert!(g.starts_with("quantity is missing") && x.ends_with("].quantity is missing"), "go: {g} / {x}");
    for lang in ["javascript", "ruby", "php", "java", "swift", "wasm"] {
        let Some(r) = result(lang) else { continue };
        if r.get("ran").and_then(|v| v.as_bool()) == Some(true) {
            assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "{lang}: {r:?}");
        }
    }
}

/// A shelf whose labels have one field, a string: no language's walk holds a string to anything,
/// so no sentence of the walk names an element. English first; its Japanese version is below.
const SHELF: &str = r#"rule shelf_labels v1
description "Whether a shelf holds a frozen label, from the codes of its labels. A sketch of a sequence whose one field is a string"

enum shelf_kind = frozen | plain

elements labels
  code : string

outputs
  kind : shelf_kind

table cold_of
policy first
| code              | -> cold : bool |
| starts_with "FZ-" | true           |
| -                 | false          |

count cold_labels over labels where cold = true  range >=0 <=20

table kind_of
policy unique
| cold_labels | -> kind : shelf_kind |
| 0           | plain                |
| >=1         | frozen               |

sequence one_cold
| code      |
| "FZ-1001" |

sequence one_plain
| code      |
| "AB-0004" |

examples
| labels    | -> kind |
| one_cold  | frozen  |
| one_plain | plain   |
"#;

const SHELF_JA: &str = r#"rule 棚の札(shelf_labels_ja) v1
description "棚に貼った札の品番から、冷凍の札がある棚かを決める。フィールドが文字列一つの並びの例"

enum 棚の種類(shelf_kind) = 冷凍(frozen) | 常温(plain)

elements 札(labels)
  品番(code) : string

outputs
  種類(kind) : 棚の種類

table 冷凍の札か(cold_of)
policy first
| 品番              | -> 冷凍(cold) : bool |
| starts_with "FZ-" | true                 |
| -                 | false                |

count 冷凍の札の数(cold_labels) over 札 where 冷凍 = true  range >=0 <=20

table 棚の種類の判定(kind_of)
policy unique
| 冷凍の札の数 | -> 種類(kind) : 棚の種類 |
| 0            | 常温                     |
| >=1          | 冷凍                     |

sequence 冷凍が一枚(one_cold)
| 品番      |
| "FZ-1001" |

sequence 常温が一枚(one_plain)
| 品番      |
| "AB-0004" |

examples
| 札         | -> 種類 |
| 冷凍が一枚 | 冷凍    |
| 常温が一枚 | 常温    |
"#;

/// The languages whose walks are looked at, by the name `--target` takes and the extension of the
/// module it writes.
const WALKING: [(&str, &str); 9] =
    [("python", "py"), ("typescript", "ts"), ("javascript", "mjs"), ("go", "go"), ("rust", "rs"), ("ruby", "rb"), ("java", "java"), ("php", "php"), ("swift", "swift")];

/// Whether `line` has `word` as a word of its own, not inside a longer name.
fn has_word(line: &str, word: &str) -> bool {
    line.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|w| w == word)
}

/// A walk counts its elements only where one of its sentences names an element (§15.324). Where
/// none does, it walks as it did before, with no index it does not read, which Go does not build:
/// a sequence whose one field is a string, which no language's walk holds to anything, and one of
/// truth values, which the typed languages leave to their types while Python, TypeScript,
/// JavaScript and Ruby hold each field to being one at the door, in a loop of its own before the
/// walk that names the element (§15.328), so that the walk names none. The English rules and
/// their Japanese versions.
#[test]
fn a_walk_whose_sentences_name_no_element_does_not_count_them() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("corpus");
    let read = |file: &str| std::fs::read_to_string(corpus.join(file)).unwrap();
    let (supplier, supplier_ja) = (read("supplier_matching.rule"), read("納入先照合.rule"));
    let naming: &[&str] = &["python", "typescript", "javascript", "ruby"];
    // The file, the rule, the language it is generated in, and the languages whose door walks the
    // sequence first and names an element.
    let rules: [(&str, &str, &str, &[&str]); 4] = [
        ("shelf_labels.rule", SHELF, "en", naming),
        ("棚の札.rule", SHELF_JA, "ja", naming),
        ("supplier_matching.rule", &supplier, "en", naming),
        ("納入先照合.rule", &supplier_ja, "ja", naming),
    ];
    for (file, src, lang, names) in rules {
        let t = TempDir::new("refused-element-walk");
        std::fs::write(t.path().join(file), src).unwrap();
        let out = t.path().join("out");
        let o = out.to_string_lossy().into_owned();
        let mut args = vec!["gen", file, "--out", o.as_str()];
        for (target, _) in WALKING {
            args.extend(["--target", target]);
        }
        let ran = finish(Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", lang).current_dir(t.path()).args(&args), RULEC_LIMIT, "rulec gen");
        assert_eq!(ran.code, Some(0), "{file}: {}{}", ran.stdout, ran.stderr);
        for (target, ext) in WALKING {
            let mut files = vec![out.join(target)];
            let mut code = String::new();
            while let Some(at) = files.pop() {
                for e in std::fs::read_dir(&at).unwrap().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        files.push(p);
                    } else if p.extension().is_some_and(|x| x == ext) {
                        code.push_str(&std::fs::read_to_string(&p).unwrap());
                    }
                }
            }
            let opens = |l: &str| has_word(l, "elem") && (l.starts_with("for ") || l.starts_with("foreach ") || l.contains(".each"));
            let loops: Vec<&str> = code.lines().map(str::trim).filter(|l| opens(l)).collect();
            let counts = names.contains(&target);
            // The door's loop comes first in the module, and counts the elements it names.
            let walks = if counts {
                assert_eq!(loops.len(), 2, "{file} [{target}]: the door's loop and the walk\n{code}");
                assert!(has_word(loops[0], "index"), "{file} [{target}]: the door's loop: {}", loops[0]);
                &loops[1..]
            } else {
                &loops[..]
            };
            assert!(!walks.is_empty(), "{file} [{target}]: no walk\n{code}");
            for w in walks {
                assert!(!has_word(w, "index"), "{file} [{target}]: {w}");
            }
            if counts {
                // The index is read: a sentence names the element by it.
                let reads = code.lines().map(str::trim).filter(|l| !opens(l) && has_word(l, "index")).count();
                assert!(reads > 0, "{file} [{target}]: the index is bound and never read\n{code}");
            }
        }
    }
}
