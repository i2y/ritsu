//! An input with two things wrong with it is refused for the first of them, in the one order every
//! way looks in (§15.328).
//!
//! The ways looked in orders of their own. A runner read the sequences before the inputs, so an
//! input of another kind and an element of another kind were refused for the element by one
//! language and for the input by the next; the Connect service looked for every field left out
//! before it read any; a walk ended at a `stop` and never looked at the elements after it, so the
//! code answered an input the reference evaluator refuses; rulec's port held each input to its
//! range as it read it, before the next was read. Now the reference evaluator, the module of every
//! language, its runner, the MCP servers, the Connect service and rulec's port read the arguments in
//! the order the function takes them, each there and of its kind, then hold each to its range, the
//! constraints and the caps, and then walk, looking at every element on the first walk over a
//! sequence even past a `stop`.
//!
//! The materials are the rule below, English first and its Japanese version beside it, which between
//! them have each pair of places a door looks at: two inputs, an input and a field of an element, two
//! fields of elements, the cap of a sequence and a constraint, a value worked out before the walk
//! and a field of an element, and two sequences; and a fold that stops on an element before one
//! outside its range. `gen` writes an input with two things wrong for each, with the sentence of the
//! one the order puts first, and every way says it. The modules of the four languages whose functions
//! look at what they are given are also called directly, with a number where a string goes, beside a
//! rule of the corpus whose input is a string (§15.339).

use ritsu_ports::{RuleError, Rules, Value, Values};
use ritsu_testkit::run::{finish, finish_with_input};
use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, need, side_by_side, skip};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;

const ENGLISH: &str = r#"rule door_order v1
description "The fee of one delivery run, from the lines a member sends and the stops on the way. A sketch of an input with two things wrong in it, for each pair of places a door looks at"

enum tier = basic < silver < gold < platinum
enum zone = near | far
enum pick = skip | halt | take | hold

inputs
  member  : tier  range >=silver <=gold
  budget  : money[JPY]  range >=0JPY <=1_000_000JPY
  ship_on : date  range >=2026-01-01 <=2026-12-31
  due_on  : date  range >=2026-01-01 <=2026-12-31
  express : bool

constraint ship_on <= due_on

elements lines
  sku      : string  unique
  zone     : zone
  weight   : number  range >=1 <=1000
  line_fee : money[JPY]  range >=0JPY <=100_000JPY

elements stops
  stop_zone : zone
  threshold : money[JPY]  range >=0JPY <=1_000_000JPY
  stop_fee  : money[JPY]  range >=0JPY <=100_000JPY

outputs
  fee   : money[JPY]  round down(1JPY)
  total : money[JPY]  round down(1JPY)
  far   : number  round down(1)
  left  : money[JPY]  round down(1JPY)

define left : money[JPY] = budget - 100JPY  range >=0JPY <=1_000_000JPY

count far over lines where zone = far  range >=0 <=5
sum total over lines of line_fee  range >=0JPY <=10_000_000JPY

table row_of
policy unique
| stop_zone | threshold | -> verdict : pick |
| near      | <=1000JPY | take              |
| near      | >1000JPY  | hold              |
| far       | <=1000JPY | skip              |
| far       | >1000JPY  | halt              |

fold verdict over stops -> fee
  skip      -> next
  halt      -> stop with 0JPY
  take      -> take_unique stop_fee
  hold      -> keep_max stop_fee by threshold
  empty     -> 0JPY
  exhausted -> held

sequence two_lines
| sku | zone | weight | line_fee |
| "a" | near | 1      | 100JPY   |
| "b" | far  | 2      | 200JPY   |

sequence halting
| stop_zone | threshold | stop_fee |
| far       | 2000JPY   | 10JPY    |
| near      | 500JPY    | 800JPY   |

sequence taking
| stop_zone | threshold | stop_fee |
| near      | 500JPY    | 800JPY   |
| near      | 2000JPY   | 1500JPY  |

examples
| member | budget  | ship_on    | due_on     | express | lines     | stops   | -> fee | total  | far | left   |
| silver | 1000JPY | 2026-05-01 | 2026-05-02 | false   | two_lines | taking  | 800JPY | 300JPY | 1   | 900JPY |
| gold   | 500JPY  | 2026-05-01 | 2026-05-01 | true    | two_lines | halting | 0JPY   | 300JPY | 1   | 400JPY |
"#;

const JAPANESE: &str = r#"rule 入口の順(door_order_ja) v1
description "配送一回の料金。会員が送る明細と、途中の停車地から決める。入力に悪い所が二つあるとき、入口がどちらを言うかを、入口が見る所の組ごとにそろえた見本"

enum 段階(tier) = 一般(basic) < シルバー(silver) < ゴールド(gold) < プラチナ(platinum)
enum 地域区分(zone) = 近距離(near) | 遠距離(far)
enum 採否(pick) = 見送り(skip) | 打ち切り(halt) | 採用(take) | 保留(hold)

inputs
  会員(member)   : 段階  range >=シルバー <=ゴールド
  予算(budget)   : money[円]  range >=0円 <=1000000円
  出荷日(ship_on) : date  range >=2026-01-01 <=2026-12-31
  期日(due_on)   : date  range >=2026-01-01 <=2026-12-31
  速達(express)  : bool

constraint 出荷日 <= 期日

elements 明細(lines)
  品番(sku)       : string  unique
  地域(zone)      : 地域区分
  重さ(weight)    : number  range >=1 <=1000
  明細料(line_fee) : money[円]  range >=0円 <=100000円

elements 停車地(stops)
  停車地域(stop_zone) : 地域区分
  閾値(threshold)     : money[円]  range >=0円 <=1000000円
  停車料(stop_fee)    : money[円]  range >=0円 <=100000円

outputs
  料金(fee)   : money[円]  round down(1円)
  合計(total) : money[円]  round down(1円)
  遠方(far)   : number  round down(1)
  残り(left)  : money[円]  round down(1円)

define 残り : money[円] = 予算 - 100円  range >=0円 <=1000000円

count 遠方 over 明細 where 地域 = 遠距離  range >=0 <=5
sum 合計 over 明細 of 明細料  range >=0円 <=10000000円

table 停車地の判定(row_of)
policy unique
| 停車地域 | 閾値     | -> 判定(verdict) : 採否 |
| 近距離   | <=1000円 | 採用                    |
| 近距離   | >1000円  | 保留                    |
| 遠距離   | <=1000円 | 見送り                  |
| 遠距離   | >1000円  | 打ち切り                |

fold 判定 over 停車地 -> 料金
  見送り    -> next
  打ち切り  -> stop with 0円
  採用      -> take_unique 停車料
  保留      -> keep_max 停車料 by 閾値
  empty     -> 0円
  exhausted -> held

sequence 明細二つ(two_lines)
| 品番 | 地域   | 重さ | 明細料 |
| "a"  | 近距離 | 1    | 100円  |
| "b"  | 遠距離 | 2    | 200円  |

sequence 打ち切る(halting)
| 停車地域 | 閾値   | 停車料 |
| 遠距離   | 2000円 | 10円   |
| 近距離   | 500円  | 800円  |

sequence 採る(taking)
| 停車地域 | 閾値   | 停車料 |
| 近距離   | 500円  | 800円  |
| 近距離   | 2000円 | 1500円 |

examples
| 会員     | 予算   | 出荷日     | 期日       | 速達  | 明細     | 停車地   | -> 料金 | 合計  | 遠方 | 残り  |
| シルバー | 1000円 | 2026-05-01 | 2026-05-02 | false | 明細二つ | 採る     | 800円   | 300円 | 1    | 900円 |
| ゴールド | 500円  | 2026-05-01 | 2026-05-01 | true  | 明細二つ | 打ち切る | 0円     | 300円 | 1    | 400円 |
"#;

/// One of the two rules: the language its files are generated in, its file, its alias and its text.
struct Rule {
    lang: &'static str,
    file: &'static str,
    alias: &'static str,
    src: &'static str,
}

const EN: Rule = Rule { lang: "en", file: "door_order.rule", alias: "door_order", src: ENGLISH };
const JA: Rule = Rule { lang: "ja", file: "入口の順.rule", alias: "door_order_ja", src: JAPANESE };

/// The inputs with two things wrong the file of refused inputs has to hold, by why each is there,
/// with the sentence every way says of it: the one of the two the order puts first.
fn two_faults(r: &Rule) -> Vec<(&'static str, &'static str, &'static str)> {
    if r.lang == "ja" {
        vec![
            ("会員 を範囲の外に、あとの 予算 を別の種類の値に。種類のほうを言う", "input", "予算 が整数ではありません"),
            ("予算 を int64 で持てない数に、あとの 出荷日 を別の種類の値に。先に読む 予算 を言う", "input", "予算 が範囲の外です"),
            ("会員 を範囲の外に、明細 の要素の 品番 を別の種類の値に。種類のほうを言う", "input", "明細[0].品番 が文字列ではありません"),
            ("会員 を別の種類の値に、明細 の要素の 品番 も別の種類の値に。先の引数の 会員 を言う", "input", "会員 が列挙 段階 の値ではありません"),
            ("明細 の一つ目の要素の 重さ を範囲の外に、二つ目の 品番 を別の種類の値に。種類のほうを言う", "input", "明細[1].品番 が文字列ではありません"),
            ("明細 の二つ目の要素の 品番 を一つ目と同じに、その 重さ を範囲の外に。範囲のほうを言う", "input", "明細[1].重さ が範囲の外です"),
            ("明細 の二つ目の要素の 品番 を一つ目と同じに、三つ目の 重さ を範囲の外に。歩きが先に会う重なりのほうを言う", "input", "明細 の要素に 品番 が同じものが二つあります（明細[0] と 明細[1]）"),
            ("明細 の要素を、数え上げの上限より一つ多くし、制約 出荷日 <= 期日 も破る。制約のほうを言う", "input", "制約が成り立ちません: 出荷日 <= 期日"),
            ("定義 残り を、受け付ける範囲の外に、明細 の要素の 重さ も範囲の外に。歩きの前に求める定義のほうを言う", "input", "残り が範囲の外です"),
            ("会員 を範囲の外に、停車地 の要素の 停車地域 を別の種類の値に。種類のほうを言う", "input", "停車地[0].停車地域 が列挙 地域区分 の値ではありません"),
            ("会員 を別の種類の値に、停車地 の要素の 停車地域 も別の種類の値に。先の引数の 会員 を言う", "input", "会員 が列挙 段階 の値ではありません"),
            ("停車地 の一つ目の要素の 閾値 を範囲の外に、二つ目の 停車地域 を別の種類の値に。種類のほうを言う", "input", "停車地[1].停車地域 が列挙 地域区分 の値ではありません"),
            ("定義 残り を、受け付ける範囲の外に、停車地 の要素の 閾値 も範囲の外に。歩きの前に求める定義のほうを言う", "input", "残り が範囲の外です"),
            ("停車地 の要素で fold が止まり、その次の要素の 閾値 を範囲の外に。止まったあとの要素も入口で確かめる", "input", "停車地[1].閾値 が範囲の外です"),
            ("停車地 の二つの要素が take_unique で当たり、三つ目の 閾値 を範囲の外に。歩きが先に会う二件のほうを言う", "contradiction", "畳み込み 判定: take_unique に二件当たりました（停車地[0] と 停車地[1]）"),
            ("明細 の要素の 重さ を範囲の外に、停車地 の要素の 停車地域 を別の種類の値に。種類のほうを言う", "input", "停車地[0].停車地域 が列挙 地域区分 の値ではありません"),
        ]
    } else {
        vec![
            ("member outside its range and budget after it of another kind: the kind is said", "input", "budget is not an integer"),
            ("budget past what int64 holds and ship_on after it of another kind: budget, read first, is said", "input", "budget is out of range"),
            ("member outside its range and sku of an element of lines of another kind: the kind is said", "input", "lines[0].sku is not a string"),
            ("member of another kind and sku of an element of lines of another kind too: member, the argument before, is said", "input", "member is not a value of enum tier"),
            ("weight of the first element of lines outside its range and sku of the second of another kind: the kind is said", "input", "lines[1].sku is not a string"),
            ("sku of the second element of lines the same as the first's, and its weight outside its range: the range is said", "input", "lines[1].weight is out of range"),
            ("sku of the second element of lines the same as the first's, and weight of the third outside its range: the two the same, which the walk meets first, are said", "input", "lines has two elements with the same sku (lines[0] and lines[1])"),
            ("lines one element past its cap and the constraint ship_on <= due_on broken: the constraint is said", "input", "the constraint does not hold: ship_on <= due_on"),
            ("the definition left outside the range it accepts and weight of an element of lines outside its range: the definition, worked out before the walk, is said", "input", "left is out of range"),
            ("member outside its range and stop_zone of an element of stops of another kind: the kind is said", "input", "stops[0].stop_zone is not a value of enum zone"),
            ("member of another kind and stop_zone of an element of stops of another kind too: member, the argument before, is said", "input", "member is not a value of enum tier"),
            ("threshold of the first element of stops outside its range and stop_zone of the second of another kind: the kind is said", "input", "stops[1].stop_zone is not a value of enum zone"),
            ("the definition left outside the range it accepts and threshold of an element of stops outside its range: the definition, worked out before the walk, is said", "input", "left is out of range"),
            ("the fold stops on an element of stops, and threshold of the element after it outside its range: the door looks at the elements past the stop", "input", "stops[1].threshold is out of range"),
            ("two elements of stops taking under a take_unique, and threshold of the third outside its range: the two, which the walk meets first, are said", "contradiction", "fold verdict: two elements matched a take_unique (stops[0] and stops[1])"),
            ("weight of an element of lines outside its range and stop_zone of an element of stops of another kind: the kind is said", "input", "stops[0].stop_zone is not a value of enum zone"),
        ]
    }
}

/// How long a run of rulec may take: `rulec test` talks to the servers of the generated code and
/// waits for their answers without a limit of its own. Past it, rulec is stopped with every process
/// it started, and the test fails saying what it ran and what it printed until then.
const RULEC_LIMIT: std::time::Duration = std::time::Duration::from_secs(20 * 60);

/// `rulec gen` of the rule of `r`, written into a directory of its own, with the targets and extras
/// of `targets`: the directory and the output.
fn generated(r: &Rule, targets: &[&str]) -> (TempDir, PathBuf) {
    let t = TempDir::new(&format!("refused-order-{}", r.lang));
    std::fs::write(t.path().join(r.file), r.src).unwrap();
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    // what `gen` writes is in the language of `--lang` alone (ritsu's DESIGN 4.1)
    let mut args = vec!["gen", r.file, "--out", o.as_str(), "--lang", r.lang];
    args.extend(targets);
    let ran = finish(Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", r.lang).current_dir(t.path()).args(&args), RULEC_LIMIT, "rulec gen");
    assert_eq!(ran.code, Some(0), "[{}] {}{}", r.lang, ran.stdout, ran.stderr);
    (t, out)
}

/// The lines of the file of refused inputs under `out`, each as its input, its kind, its sentence
/// and why it is there.
fn refused(out: &Path, alias: &str) -> Vec<(Json, String, String, String)> {
    let body = std::fs::read_to_string(out.join("vectors").join(format!("{alias}.refused.jsonl"))).unwrap();
    body.lines()
        .map(|l| {
            let j = rulec::json::parse(l).unwrap_or_else(|e| panic!("{alias}: {e}: {l}"));
            let s = |k: &str| j.get(k).and_then(|v| v.as_str()).unwrap_or_else(|| panic!("{alias}: no {k}: {l}")).to_string();
            (j.get("in").cloned().unwrap_or_else(|| panic!("{alias}: no in: {l}")), s("refused"), s("error"), s("why"))
        })
        .collect()
}

fn lang_of(r: &Rule) -> rulec::i18n::Lang {
    if r.lang == "ja" { rulec::i18n::Lang::Ja } else { rulec::i18n::Lang::En }
}

/// `gen` writes an input with two things wrong for each pair of places the door looks at, and one
/// whose fold stops before an element outside its range, each with the sentence of the one the order
/// puts first; the reference evaluator says it, and `rulec run` says it too.
#[test]
fn gen_writes_an_input_with_two_things_wrong_for_each_pair_of_places() {
    for r in [&EN, &JA] {
        let (t, out) = generated(r, &["--target", "python", "--extra", "vectors"]);
        let lines = refused(&out, r.alias);
        let path = t.path().join(r.file).to_string_lossy().into_owned();
        let (f, c) = rulec::prepare(r.src, &path).unwrap_or_else(|_| panic!("{} does not prepare", r.file));
        for (why, wanted, said) in two_faults(r) {
            let (input, kind, error, _) = lines.iter().find(|(.., w)| w == why).unwrap_or_else(|| panic!("[{}] no input `{why}`:\n{:#?}", r.lang, lines.iter().map(|l| &l.3).collect::<Vec<_>>()));
            assert_eq!((kind.as_str(), error.as_str()), (wanted, said), "[{}] {why}", r.lang);
            let got = rulec::i18n::with(lang_of(r), || rulec::door::refusal(&f, &c, input)).unwrap_or_else(|| panic!("[{}] {why}: answered", r.lang));
            assert_eq!((got.kind, got.error.as_str()), (wanted, said), "[{}] {why}", r.lang);
        }
        // `rulec run`, the input of another kind before the element of another kind, and the fold
        // that stops before the element outside its range.
        for (why, _, said) in two_faults(r).into_iter().filter(|(w, ..)| w.contains("先の引数") || w.contains("the argument before") || w.contains("fold") || w.contains("take_unique")) {
            let input = &lines.iter().find(|(.., w)| w == why).unwrap().0;
            let o = finish_with_input(
                Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", r.lang).current_dir(t.path()).args(["run", r.file, "--input", "-", "--format", "json"]),
                Some(input.compact().as_bytes()),
                RULEC_LIMIT,
                "rulec run",
            );
            assert_eq!(o.code, Some(0), "[{}] {why}: {}{}", r.lang, o.stdout, o.stderr);
            let j = rulec::json::parse(&o.stdout).unwrap_or_else(|e| panic!("{e}: {}", o.stdout));
            assert_eq!(j.get("error").and_then(|v| v.as_str()), Some(said), "[{}] {why}: {}", r.lang, o.stdout);
        }
    }
}

/// A value of the input on a line of the file of refused inputs as a caller of rulec's port gives it:
/// a value of the declared type as the port carries that type, and one of another kind as the port
/// carries what it is, a number as a number and a string as a string.
fn port_value(c: &rulec::types::Checked, name: &str, j: &Json) -> Value {
    use rulec::types::Ty;
    let ty = c.ty_of(name).unwrap_or_else(|| panic!("{name} has no type"));
    match j {
        Json::Null => Value::None,
        Json::Bool(b) => Value::Bool(*b),
        Json::Int(n) => Value::Int(*n),
        Json::Str(s) => match ty.present() {
            Ty::Date => Value::Date(s.clone()),
            Ty::Enum(_) => Value::Enum(s.clone()),
            _ => Value::Str(s.clone()),
        },
        other => panic!("{name}: {other:?} is not a value the port carries"),
    }
}

/// The input of a line of the file of refused inputs as a caller of rulec's port gives it.
fn port_values(f: &rulec::ast::RuleFile, c: &rulec::types::Checked, input: &Json) -> Values {
    let pairs = input.as_obj().unwrap_or_else(|| panic!("not an object: {}", input.compact()));
    pairs
        .iter()
        .map(|(name, j)| {
            let v = if f.elements.iter().any(|el| el.name.text == *name) {
                let xs = j.as_arr().unwrap_or_else(|| panic!("{name} is not a list: {}", j.compact()));
                Value::List(
                    xs.iter()
                        .map(|x| x.as_obj().unwrap_or_else(|| panic!("an element of {name} is not an object: {}", x.compact())).iter().map(|(k, v)| (k.clone(), port_value(c, k, v))).collect())
                        .collect(),
                )
            } else {
                port_value(c, name, j)
            };
            (name.clone(), v)
        })
        .collect()
}

/// What rulec's port says of each input of `two_faults`, in the same order: the one the order puts
/// first, in the port's own words, in the language of the rule.
fn port_says(r: &Rule) -> Vec<(&'static str, &'static str)> {
    if r.lang == "ja" {
        vec![
            ("input", "`予算` に money[円] として読めない値が渡されています"),
            ("input", "入力 `予算` が範囲の外です"),
            ("input", "`明細[0].品番` に string として読めない値が渡されています"),
            ("input", "`会員` に 段階 として読めない値が渡されています"),
            ("input", "`明細[1].品番` に string として読めない値が渡されています"),
            ("input", "要素のフィールド `明細[1].重さ` が範囲の外です"),
            ("input", "並び `明細` の要素に、`品番` が同じものが二つあります（`明細[0]` と `明細[1]`）"),
            ("input", "`出荷日 <= 期日` が成り立ちません"),
            ("input", "計算した `残り` が、受け付ける範囲の外です"),
            ("input", "`停車地[0].停車地域` に 地域区分 として読めない値が渡されています"),
            ("input", "`会員` に 段階 として読めない値が渡されています"),
            ("input", "`停車地[1].停車地域` に 地域区分 として読めない値が渡されています"),
            ("input", "計算した `残り` が、受け付ける範囲の外です"),
            ("input", "要素のフィールド `停車地[1].閾値` が範囲の外です"),
            ("contradiction", "畳み込み `判定` で、`take_unique` に二つの要素が当たりました（`停車地[0]` と `停車地[1]`）"),
            ("input", "`停車地[0].停車地域` に 地域区分 として読めない値が渡されています"),
        ]
    } else {
        vec![
            ("input", "`budget` is given a value that is not a money[JPY]"),
            ("input", "the input `budget` is outside its range"),
            ("input", "`lines[0].sku` is given a value that is not a string"),
            ("input", "`member` is given a value that is not a tier"),
            ("input", "`lines[1].sku` is given a value that is not a string"),
            ("input", "an element's field `lines[1].weight` is outside its range"),
            ("input", "the list `lines` has two elements with the same `sku` (`lines[0]` and `lines[1]`)"),
            ("input", "`ship_on <= due_on` does not hold"),
            ("input", "the `left` it works out is outside the range the rule accepts for it"),
            ("input", "`stops[0].stop_zone` is given a value that is not a zone"),
            ("input", "`member` is given a value that is not a tier"),
            ("input", "`stops[1].stop_zone` is given a value that is not a zone"),
            ("input", "the `left` it works out is outside the range the rule accepts for it"),
            ("input", "an element's field `stops[1].threshold` is outside its range"),
            ("contradiction", "two elements matched a `take_unique` of the fold `verdict` (`stops[0]` and `stops[1]`)"),
            ("input", "`stops[0].stop_zone` is given a value that is not a zone"),
        ]
    }
}

/// rulec's port, through which the other languages of ritsu call a rule, refuses each input with two
/// things wrong for the one the order puts first, in its own words. A whole number int64 does not
/// hold is refused where it is read, before the values after it, as the door refuses it.
#[test]
fn the_port_says_the_first_of_the_two_in_its_own_words() {
    let e = rulec::ports::Engine::new();
    for r in [&EN, &JA] {
        let (t, out) = generated(r, &["--target", "python", "--extra", "vectors"]);
        let lines = refused(&out, r.alias);
        let path = t.path().join(r.file);
        let (f, c) = rulec::prepare(r.src, &path.to_string_lossy()).unwrap_or_else(|_| panic!("{} does not prepare", r.file));
        for ((why, ..), (wanted, said)) in two_faults(r).into_iter().zip(port_says(r)) {
            let input = &lines.iter().find(|(.., w)| w == why).unwrap_or_else(|| panic!("[{}] no input `{why}`", r.lang)).0;
            let (kind, t) = match e.eval(&path, &port_values(&f, &c, input)) {
                Err(RuleError::Input(t)) => ("input", t),
                Err(RuleError::Contradiction(t)) => ("contradiction", t),
                other => panic!("[{}] {why}: {other:?}", r.lang),
            };
            let got = if r.lang == "ja" { t.ja } else { t.en };
            assert_eq!((kind, got.as_str()), (wanted, said), "[{}] {why}", r.lang);
        }
    }
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

/// `rulec test` over the code `gen` writes for the two rules: every way the rule is reached — each
/// language's module and runner, the MCP servers, the Connect service, the Wasm module — says of each
/// input with two things wrong the sentence the reference evaluator says. The English rule and the
/// Japanese version run side by side.
#[test]
fn every_way_says_the_first_of_the_two() {
    // Every language there is a toolchain for, the tools level's (ritsu's DESIGN 10.2).
    if !need(Need::Python) {
        return;
    }
    side_by_side(&[&EN, &JA], |r| {
        let (_t, out) = generated(r, rulec::targets::UP_TO_0_40);
        let lines = refused(&out, r.alias);
        assert!(two_faults(r).iter().all(|(why, ..)| lines.iter().any(|(.., w)| w == why)), "[{}] an input with two things wrong is missing", r.lang);
        let (code, j) = rulec_test(&out);
        assert_eq!(code, 0, "[{}] {j:?}", r.lang);
        let mut ways = Vec::new();
        for x in j.get("results").and_then(|v| v.as_arr()).unwrap_or_default() {
            if x.get("rule").and_then(|v| v.as_str()) != Some(r.alias) || x.get("ran").and_then(|v| v.as_bool()) != Some(true) || !puts_refused(x) {
                continue;
            }
            assert_eq!(x.get("ok").and_then(|v| v.as_bool()), Some(true), "[{}] {x:?}", r.lang);
            assert_eq!(x.get("refused").and_then(|v| v.as_int()), Some(lines.len() as i128), "[{}] not every input was put: {x:?}", r.lang);
            ways.push(format!("{}/{}", x.get("lang").and_then(|v| v.as_str()).unwrap_or_default(), x.get("via").and_then(|v| v.as_str()).unwrap_or_default()));
        }
        for way in ["python/runner", "python/mcp", "typescript/runner", "typescript/mcp", "rust/runner"] {
            assert!(ways.iter().any(|w| w == way), "[{}] {way} did not run: {ways:?}", r.lang);
        }
        println!("two things wrong [{}]: {} ways: {}", r.lang, ways.len(), ways.join(", "));
    });
}

/// A way that ends its walk at the `stop`, or that reads the sequences before the inputs, is caught
/// on the input that tells it apart: the Python module whose walk leaves where the fold stops, as
/// every language's did, answers the input whose element after the stop is outside its range, and
/// the TypeScript runner that reads the elements first says the element of another kind where the
/// input before it is of another kind too. The file holds the inputs with two things wrong and no
/// other, so each is the first line its way says otherwise; every other language still says what
/// the reference says.
#[test]
fn a_way_that_ends_at_the_stop_or_reads_the_elements_first_is_caught() {
    if !need(Need::Python) {
        return;
    }
    let (_t, out) = generated(&EN, rulec::targets::UP_TO_0_40);
    // Only the inputs with two things wrong.
    let file = out.join("vectors").join("door_order.refused.jsonl");
    let whys: Vec<&str> = two_faults(&EN).iter().map(|(w, ..)| *w).collect();
    let kept: Vec<String> = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .filter(|l| rulec::json::parse(l).ok().and_then(|j| j.get("why").and_then(|v| v.as_str()).map(|w| whys.contains(&w))).unwrap_or(false))
        .map(str::to_string)
        .collect();
    assert_eq!(kept.len(), whys.len(), "{kept:#?}");
    std::fs::write(&file, kept.join("\n") + "\n").unwrap();
    // The Python module's walk over the stops leaves where the fold stops.
    let py = out.join("python/door_order.py");
    let src = std::fs::read_to_string(&py).unwrap();
    let past = "            stopped = True\n            done = True\n";
    assert_eq!(src.matches(past).count(), 1, "the stop that goes on walking:\n{src}");
    std::fs::write(&py, src.replace(past, "            stopped = True\n            break\n")).unwrap();
    // The TypeScript runner reads the elements before the inputs.
    let ts = out.join("typescript/door_order_runner.ts");
    let src = std::fs::read_to_string(&ts).unwrap();
    let mut lines: Vec<&str> = src.lines().collect();
    let ins = lines.iter().position(|l| l.trim_start().starts_with("const ins = [")).expect("the line that reads the inputs");
    let call = lines.iter().position(|l| l.trim_start().starts_with("const a = [...ins")).expect("the line that hands the arguments over");
    let moved = lines.remove(ins);
    lines.insert(call - 1, moved);
    std::fs::write(&ts, lines.join("\n") + "\n").unwrap();
    let (code, j) = rulec_test(&out);
    assert_eq!(code, 1, "{j:?}");
    let result = |lang: &str| -> Json {
        j.get("results")
            .and_then(|v| v.as_arr())
            .unwrap_or_default()
            .iter()
            .find(|r| r.get("lang").and_then(|v| v.as_str()) == Some(lang) && r.get("via").and_then(|v| v.as_str()) == Some("runner"))
            .cloned()
            .unwrap_or_else(|| panic!("{lang}/runner did not run: {j:?}"))
    };
    let p = result("python");
    assert_eq!(p.get("ok").and_then(|v| v.as_bool()), Some(false), "{p:?}");
    let stop = 1 + kept.iter().position(|l| l.contains("the fold stops")).expect("the input whose fold stops");
    let said = p.get("error").and_then(|v| v.as_str()).unwrap_or_default();
    assert!(said.starts_with(&format!("answered an input the reference evaluator refuses (line {stop}): ")), "{said}");
    let t = result("typescript");
    assert_eq!(t.get("ok").and_then(|v| v.as_bool()), Some(false), "{t:?}");
    let d = t.get("first_diff").unwrap_or_else(|| panic!("{t:?}"));
    let error = |k: &str| -> String {
        let line = d.get(k).and_then(|v| v.as_str()).unwrap_or_else(|| panic!("{t:?}"));
        rulec::json::parse(line).ok().and_then(|x| x.get("error").and_then(|v| v.as_str()).map(str::to_string)).unwrap_or_else(|| panic!("{line}"))
    };
    assert_eq!((error("generated").as_str(), error("expected").as_str()), ("lines[0].sku is not a string", "member is not a value of enum tier"), "{t:?}");
    for lang in ["javascript", "go", "rust", "ruby", "php", "java", "swift", "wasm"] {
        let Some(r) = j.get("results").and_then(|v| v.as_arr()).unwrap_or_default().iter().find(|r| r.get("lang").and_then(|v| v.as_str()) == Some(lang) && r.get("via").and_then(|v| v.as_str()) == Some("runner")).cloned() else { continue };
        if r.get("ran").and_then(|v| v.as_bool()) == Some(true) {
            assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "{lang}: {r:?}");
        }
    }
}

/// The rule of an input that is a string, English first and its Japanese version beside it: the
/// files of the corpus.
const SKU_EN: Rule = Rule { lang: "en", file: "sku_prefix_handling.rule", alias: "sku_prefix_handling", src: include_str!("corpus/sku_prefix_handling.rule") };
const SKU_JA: Rule = Rule { lang: "ja", file: "品番の扱い.rule", alias: "sku_handling", src: include_str!("corpus/品番の扱い.rule") };

/// The calls of [`a_module_called_directly_holds_a_string_to_its_kind`], each with what the module
/// is to say: the sentence of the refusal, or `answered`.
fn string_calls(r: &Rule) -> [(&'static str, &'static str); 6] {
    let (element, input) = if r.lang == "ja" {
        ("明細[0].品番 が文字列ではありません", "品番 が文字列ではありません")
    } else {
        ("lines[0].sku is not a string", "sku is not a string")
    };
    [
        ("sku of the element a number", element),
        ("sku of the element a number and member outside its range", element),
        ("sku of the element a string", "answered"),
        ("the input sku a number", input),
        ("the input sku a number and boxes after it outside its range", input),
        ("the input sku a string", "answered"),
    ]
}

/// The calls of [`string_calls`] in Python, through the module of each of the two rules, printing
/// for each what it said.
fn python_calls(door: &str, sku: &str) -> String {
    format!(
        "import {door} as m, {sku} as s\n\
         def t(f):\n    try:\n        f()\n        print('answered')\n    except (m.RuleInputError, s.RuleInputError) as e:\n        print(e.what)\n    except Exception as e:\n        print('broke', type(e).__name__, e)\n\
         L = m.LinesElement\n\
         t(lambda: m.{door}(m.Tier.GOLD, 1000, 20454, 20454, False, [L(5, m.Zone.NEAR, 1, 0)], []))\n\
         t(lambda: m.{door}(m.Tier.BASIC, 1000, 20454, 20454, False, [L(5, m.Zone.NEAR, 1, 0)], []))\n\
         t(lambda: m.{door}(m.Tier.GOLD, 1000, 20454, 20454, False, [L('a', m.Zone.NEAR, 1, 0)], []))\n\
         t(lambda: s.{sku}(5, 3))\n\
         t(lambda: s.{sku}(5, 500))\n\
         t(lambda: s.{sku}('FZ-1', 3))\n"
    )
}

/// The same in JavaScript, for the module of either language (`ext`).
fn js_calls(door: &str, sku: &str, ext: &str) -> String {
    format!(
        "import * as m from './{door}.{ext}';\n\
         import * as s from './{sku}.{ext}';\n\
         const t = (f) => {{ try {{ f(); console.log('answered'); }} catch (e) {{ console.log(e.name === 'RuleInputError' ? e.what : 'broke ' + e); }} }};\n\
         const L = (sku) => ({{ sku, zone: m.Zone.NEAR, weight: 1n, line_fee: 0n }});\n\
         t(() => m.{door}(m.Tier.GOLD, 1000n, 20454n, 20454n, false, [L(5n)], []));\n\
         t(() => m.{door}(m.Tier.BASIC, 1000n, 20454n, 20454n, false, [L(5n)], []));\n\
         t(() => m.{door}(m.Tier.GOLD, 1000n, 20454n, 20454n, false, [L('a')], []));\n\
         t(() => s.{sku}(5n, 3n));\n\
         t(() => s.{sku}(5n, 500n));\n\
         t(() => s.{sku}('FZ-1', 3n));\n"
    )
}

/// The same in Ruby.
fn ruby_calls(door: &str, sku: &str) -> String {
    let module = |a: &str| -> String { a.split('_').map(|w| w[..1].to_uppercase() + &w[1..]).collect() };
    let (m, s) = (module(door), module(sku));
    format!(
        "require './{door}.rb'\nrequire './{sku}.rb'\n\
         def t\n  yield\n  puts 'answered'\nrescue {m}::RuleInputError, {s}::RuleInputError => e\n  puts e.what\nrescue => e\n  puts \"broke #{{e.class}} #{{e.message}}\"\nend\n\
         L = {m}::LinesElement\n\
         t {{ {m}.{door}({m}::Tier::GOLD, 1000, 20454, 20454, false, [L.new(5, {m}::Zone::NEAR, 1, 0)], []) }}\n\
         t {{ {m}.{door}({m}::Tier::BASIC, 1000, 20454, 20454, false, [L.new(5, {m}::Zone::NEAR, 1, 0)], []) }}\n\
         t {{ {m}.{door}({m}::Tier::GOLD, 1000, 20454, 20454, false, [L.new('a', {m}::Zone::NEAR, 1, 0)], []) }}\n\
         t {{ {s}.{sku}(5, 3) }}\n\
         t {{ {s}.{sku}(5, 500) }}\n\
         t {{ {s}.{sku}('FZ-1', 3) }}\n"
    )
}

/// The module of each of the four languages whose functions look at what they are given, called
/// directly, holds a string to its kind in the first look, as the runner, the MCP servers, the
/// Connect service and the reference evaluator do (§15.339): a number given for a string input or
/// for a string field of an element is refused for that, and before an input outside its range.
/// The modules looked at integers, truth values, enums and dates and not strings, so a number for
/// the `sku` of an element was answered, and one for the input `sku` broke inside the table with the
/// language's own error, or was refused for the input after it. The Python module without the look
/// is caught.
#[test]
fn a_module_called_directly_holds_a_string_to_its_kind() {
    if !need(Need::Python) {
        return;
    }
    let targets = ["--target", "python", "--target", "typescript", "--target", "javascript", "--target", "ruby"];
    for (door, sku) in [(&EN, &SKU_EN), (&JA, &SKU_JA)] {
        let (_t, out) = generated(door, &targets);
        let (_u, out2) = generated(sku, &targets);
        // the two modules of each language side by side, where one script imports both
        for lang in ["python", "typescript", "javascript", "ruby"] {
            for e in std::fs::read_dir(out2.join(lang)).unwrap() {
                let e = e.unwrap();
                if e.path().is_file() {
                    std::fs::copy(e.path(), out.join(lang).join(e.file_name())).unwrap();
                }
            }
        }
        let want: Vec<&str> = string_calls(door).iter().map(|(_, w)| *w).collect();
        let said = |o: ritsu_testkit::run::Ran, lang: &str| -> Vec<String> {
            assert_eq!(o.code, Some(0), "[{}] {lang}: {}{}", door.lang, o.stdout, o.stderr);
            o.stdout.lines().map(str::to_string).collect()
        };
        let mut runs: Vec<(&str, Vec<String>)> = Vec::new();
        let py = python_calls(door.alias, sku.alias);
        runs.push(("Python", said(finish(Command::new("python3").current_dir(out.join("python")).args(["-B", "-c", &py]), RULEC_LIMIT, "python3"), "Python")));
        if need(Need::Node) {
            for (lang, sub, ext) in [("JavaScript", "javascript", "mjs"), ("TypeScript", "typescript", "ts")] {
                let script = out.join(sub).join(format!("_calls.{ext}"));
                std::fs::write(&script, js_calls(door.alias, sku.alias, ext)).unwrap();
                runs.push((lang, said(finish(Command::new("node").current_dir(out.join(sub)).args(["--no-warnings", &format!("_calls.{ext}")]), RULEC_LIMIT, "node"), lang)));
            }
        }
        if need(Need::Ruby) {
            let rb = ruby_calls(door.alias, sku.alias);
            runs.push(("Ruby", said(finish(Command::new("ruby").current_dir(out.join("ruby")).args(["-e", &rb]), RULEC_LIMIT, "ruby"), "Ruby")));
        }
        for (lang, got) in &runs {
            for (k, (why, w)) in string_calls(door).iter().enumerate() {
                assert_eq!(got.get(k).map(String::as_str), Some(*w), "[{}] {lang}: {why}\n{got:#?}", door.lang);
            }
        }
        println!("strings called directly [{}]: {}", door.lang, runs.iter().map(|(l, _)| *l).collect::<Vec<_>>().join(", "));
        // Without the look, the Python module answers the element of another kind and breaks on the
        // input of another kind.
        for (alias, look) in [(door.alias, "        if not _isinstance(elem.sku, _str):\n"), (sku.alias, "    if not _isinstance(sku, _str):\n")] {
            let py = out.join("python").join(format!("{alias}.py"));
            let src = std::fs::read_to_string(&py).unwrap();
            let at = src.find(look).unwrap_or_else(|| panic!("[{}] the look at a string:\n{src}", door.lang));
            let end = at + look.len() + src[at + look.len()..].find('\n').unwrap() + 1;
            std::fs::write(&py, format!("{}{}", &src[..at], &src[end..])).unwrap();
        }
        let o = finish(Command::new("python3").current_dir(out.join("python")).args(["-B", "-c", &python_calls(door.alias, sku.alias)]), RULEC_LIMIT, "python3");
        let got: Vec<&str> = o.stdout.lines().collect();
        assert_eq!(got.first().copied(), Some("answered"), "[{}] {got:#?}", door.lang);
        assert!(got.get(3).is_some_and(|l| l.starts_with("broke")), "[{}] {got:#?}", door.lang);
        assert_ne!(got, want, "[{}]", door.lang);
    }
}
