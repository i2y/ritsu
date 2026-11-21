//! The range a rule accepts for a value it works out: `define … range` (§15.313).
//!
//! The corpus has the materials, English first with the Japanese version of each beside it: a fee
//! worked out once for a call (`card_fee`, `カード手数料`), a subtotal worked out on each line
//! (`order_line_subtotals`, `明細の小計`) and a total worked out from random inputs (`dice_total`,
//! `さいころの合計`). The ones written here are a range wider than anything the value comes to, ends
//! that do not take the value they write, a value worked out after a table, a rule that applies a
//! rule with such a range, and a contribution that adds one.
//!
//! What is held: what `rulec run` answers inside each range, at its ends and one step past them, in
//! both languages; the lines of the generated code of every target that refuse a call where the
//! value is worked out; and what `check` says of a range — nothing of one wider than what the value
//! reaches, and the error of one it cannot take. Golden files under `tests/golden/define_range`
//! (`RULEC_BLESS=1 cargo test --test define_range` writes them again).

use ritsu_testkit::TempDir;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rulec(dir: &Path, lang: &str, args: &[&str], stdin: Option<&str>) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rulec"));
    c.env("RULEC_LANG", lang).current_dir(dir).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().expect("cannot start rulec");
    let mut si = child.stdin.take().unwrap();
    if let Some(s) = stdin {
        si.write_all(s.as_bytes()).unwrap();
    }
    drop(si);
    let o = child.wait_with_output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn golden(name: &str, text: &str) {
    if let Err(e) = ritsu_testkit::golden::check(&root().join("tests/golden/define_range").join(name), text) {
        panic!("{e}");
    }
}

const WIDER: &str = "rule wider_fee v1
description \"A card fee with a range wider than anything the fee comes to: nothing is said of it, and no call is refused for it\"

inputs
  amount : money[JPY]  range >=1JPY <=1_000_000JPY

define fee : money[JPY] = down(amount * 3%, 1JPY)  range >=0JPY <=50_000JPY

outputs
  charged : money[JPY]  round down(1JPY)

result charged = fee
";

const WIDER_JA: &str = "rule 広い範囲の手数料(wider_fee_ja) v1
description \"手数料がとりうるどの値よりも広い範囲を書いたカードの手数料。何も言われず、その範囲で受け付けない呼び出しも無い\"

inputs
  金額(amount) : money[円]  range >=1円 <=1000000円

define 手数料(fee) : money[円] = down(金額 * 3%, 1円)  range >=0円 <=50000円

outputs
  請求額(charged) : money[円]  round down(1円)

result 請求額 = 手数料
";

const STRICT: &str = "rule strict_fee v1
description \"A card fee accepted above 0 yen and below 20,000 yen: ends that do not take the value they write, read on the yen the fee is kept in\"

inputs
  amount : money[JPY]  range >=1JPY <=1_000_000JPY

define fee : money[JPY] = down(amount * 3%, 1JPY)  range >0JPY <20_000JPY

outputs
  charged : money[JPY]  round down(1JPY)

result charged = fee
";

const STRICT_JA: &str = "rule 端を含まない手数料(strict_fee_ja) v1
description \"0 円より大きく 2 万円より小さい手数料だけを受け付けるカードの手数料。書いた値を含まない端を、手数料を持つ円の刻みで読む\"

inputs
  金額(amount) : money[円]  range >=1円 <=1000000円

define 手数料(fee) : money[円] = down(金額 * 3%, 1円)  range >0円 <20000円

outputs
  請求額(charged) : money[円]  round down(1円)

result 請求額 = 手数料
";

const AFTER_TABLE: &str = "rule parcel_charge v1
description \"The charge for a parcel: a base for its zone, which a table gives, and a yen for each gram. The charge is accepted from 600 yen up to 20,000 yen, so a parcel is refused after the table, where the charge is worked out\"

enum zone = near | far
enum band = low | high

inputs
  zone   : zone
  weight : number  range >=1 <=30_000

table base_of
policy unique
| zone | -> base : money[JPY] |
| near | 500JPY               |
| far  | 1_500JPY             |

define charge : money[JPY] = base + weight * 1JPY  range >=600JPY <=20_000JPY

outputs
  charged : money[JPY]  round down(1JPY)
  band    : band

result charged = charge

table band_of
policy unique
| charge     | -> band |
| <=5_000JPY | low     |
| >5_000JPY  | high    |
";

const AFTER_TABLE_JA: &str = "rule 小包の料金(parcel_charge_ja) v1
description \"小包の料金。地域ごとの基本料（表で決まる）に、1 グラムにつき 1 円を足す。料金は 600 円から 2 万円までを受け付けるので、表を引いたあと、料金を計算したところで受け付けない\"

enum 地域(zone) = 近い(near) | 遠い(far)
enum 区分(band) = 安い(low) | 高い(high)

inputs
  地域(zone)   : 地域
  重さ(weight) : number  range >=1 <=30000

table 基本料(base_of)
policy unique
| 地域 | -> 基本料金(base) : money[円] |
| 近い | 500円                        |
| 遠い | 1500円                       |

define 料金(charge) : money[円] = 基本料金 + 重さ * 1円  range >=600円 <=20000円

outputs
  請求額(charged) : money[円]  round down(1円)
  区分(band)      : 区分

result 請求額 = 料金

table 区分の表(band_of)
policy unique
| 料金      | -> 区分 |
| <=5000円 | 安い    |
| >5000円  | 高い    |
";

/// The callee of the apply below, and the pin of its meaning.
const CALLEE: &str = "rule card_fee v1
description \"A card fee of 3% of the amount, accepted only up to 20,000 yen\"

inputs
  amount : money[JPY]  range >=0JPY <=1_000_000JPY

outputs
  charged : money[JPY]  round down(1JPY)

define fee : money[JPY] = down(amount * 3%, 1JPY)  range <=20_000JPY

table charged_by_fee
policy unique
| fee         | -> charged |
| <=20_000JPY | fee        |
";

const CALLER: &str = "rule order v1
description \"An order that pays the card fee on its total: the range the callee accepts its fee in comes in under the apply's name\"

inputs
  total : money[JPY]  range >=0JPY <=1_000_000JPY

outputs
  paid : money[JPY]  round down(1JPY)

apply card = \"card_fee.rule\" meaning:93d21bcaa53b0e15
  amount = total
  charged -> card_charged

define paid_raw : money[JPY] = total + card_charged

table paid_by_total
policy unique
| total | -> paid   |
| -     | paid_raw  |
";

/// A contribution to the corpus's `order_discount` that adds a value it accepts only up to 6,000
/// dollars: the merged rule refuses an order past it.
const CONTRIBUTION: &str = "contribution capped_campaign(capped_campaign) v1
description \"A campaign of the north region that takes orders of at most 3,000 dollars: what it counts is twice the subtotal, accepted up to 6,000 dollars\"
to \"../order_discount.rule\" signature:fe7307da2694d75a

inputs
  north(north) : bool

define counted : money[USD] = subtotal * 2  range <=6000USD

table campaign_parts
policy unique
| north | -> campaign_off : money[USD] |
| true  | 7USD                         |
| false | 0USD                         |

into stackable
  campaign_off
";

/// Writes the materials of this file into a directory: the rules by name, and the contribution
/// beside a copy of the corpus rule it adds to.
fn materials() -> TempDir {
    let t = TempDir::new("define-range");
    let d = t.path();
    for (name, body) in [
        ("wider_fee.rule", WIDER),
        ("広い範囲の手数料.rule", WIDER_JA),
        ("strict_fee.rule", STRICT),
        ("端を含まない手数料.rule", STRICT_JA),
        ("parcel_charge.rule", AFTER_TABLE),
        ("小包の料金.rule", AFTER_TABLE_JA),
        ("card_fee.rule", CALLEE),
        ("order.rule", CALLER),
    ] {
        std::fs::write(d.join(name), body).unwrap();
    }
    std::fs::copy(root().join("tests/corpus/order_discount.rule"), d.join("order_discount.rule")).unwrap();
    std::fs::create_dir_all(d.join("c")).unwrap();
    std::fs::write(d.join("c/capped_campaign.rule"), CONTRIBUTION).unwrap();
    t
}

/// What is asked of each rule, under the names it declares: `name=value` words, or a JSON object
/// (for a sequence).
fn asked() -> Vec<(PathBuf, &'static str, Vec<&'static str>)> {
    let c = |f: &str| root().join("tests/corpus").join(f);
    // 10 at 1,000,000 yen is the top end; at 1,000,001 yen the second line is refused, and eleven
    // lines at the top end take the amount past its own range
    let subtotals = |seq: &str, q: &str, p: &str| -> Vec<&'static str> {
        let line = |qv: &str, pv: &str| format!("{{\"{q}\":{qv},\"{p}\":{pv}}}");
        let lines = |ls: Vec<String>| -> &'static str { Box::leak(format!("{{\"{seq}\":[{}]}}", ls.join(",")).into_boxed_str()) };
        vec![
            lines(vec![line("2", "1500"), line("10", "1000000")]),
            lines(vec![line("2", "1500"), line("10", "1000001")]),
            lines((0..11).map(|_| line("10", "1000000")).collect()),
        ]
    };
    let fees = |amount: &str| -> Vec<&'static str> {
        ["1", "666666", "666699", "666700", "1000000"].iter().map(|v| -> &'static str { Box::leak(format!("{amount}={v}").into_boxed_str()) }).collect()
    };
    let dice = |first: &str, second: &str| -> Vec<&'static str> {
        [("1", "1"), ("4", "6"), ("5", "6"), ("6", "6")].iter().map(|(a, b)| -> &'static str { Box::leak(format!("{first}={a} {second}={b}").into_boxed_str()) }).collect()
    };
    vec![
        (c("card_fee.rule"), "en", fees("amount")),
        (c("カード手数料.rule"), "ja", fees("金額")),
        (c("order_line_subtotals.rule"), "en", subtotals("lines", "quantity", "price")),
        (c("明細の小計.rule"), "ja", subtotals("明細", "数量", "単価")),
        (c("dice_total.rule"), "en", dice("first", "second")),
        (c("さいころの合計.rule"), "ja", dice("一つ目", "二つ目")),
    ]
}

/// One run, as a reader would type it, and what it printed.
fn shown(dir: &Path, lang: &str, rule: &str, what: &str, json: bool) -> String {
    let mut args = vec!["run", rule];
    let stdin = if what.starts_with('{') {
        args.extend(["--input", "-"]);
        Some(what)
    } else {
        args.extend(what.split(' '));
        None
    };
    if json {
        args.extend(["--format", "json"]);
    }
    let (code, out, e) = rulec(dir, lang, &args, stdin);
    assert_eq!(code, 0, "{rule} {what}: {e}{out}");
    let typed = match stdin {
        Some(s) => format!("echo '{s}' | rulec run {rule} --input -"),
        None => format!("rulec run {rule} {what}"),
    };
    format!("$ {typed}{}\n{out}", if json { " --format json" } else { "" })
}

/// Inside the range, at its ends and one step past them, for each material in its language; the
/// first of each as JSON as well. The answer at an end is the one the table gives there, and past
/// it the run is refused with the sentence a range at the door gives, naming the value.
#[test]
fn run_answers_inside_the_range_and_refuses_past_its_ends() {
    let t = materials();
    let mut text = String::new();
    for (rule, lang, whats) in asked() {
        let dir = rule.parent().unwrap();
        let name = rule.file_name().unwrap().to_string_lossy().into_owned();
        for (i, what) in whats.iter().enumerate() {
            text += &shown(dir, lang, &name, what, false);
            if i == 0 {
                text += &shown(dir, lang, &name, what, true);
            }
        }
        text += "\n";
    }
    let d = t.path();
    for (rule, lang, whats) in [
        ("strict_fee.rule", "en", &["amount=1", "amount=34", "amount=666666", "amount=666667"][..]),
        ("端を含まない手数料.rule", "ja", &["金額=1", "金額=34", "金額=666666", "金額=666667"][..]),
        ("parcel_charge.rule", "en", &["zone=near weight=99", "zone=near weight=100", "zone=far weight=18500", "zone=far weight=18501"][..]),
        ("小包の料金.rule", "ja", &["地域=近い 重さ=99", "地域=近い 重さ=100", "地域=遠い 重さ=18500", "地域=遠い 重さ=18501"][..]),
        ("order.rule", "en", &["total=666699", "total=666700"][..]),
        ("wider_fee.rule", "en", &["amount=1000000"][..]),
    ] {
        for what in whats {
            text += &shown(d, lang, rule, what, false);
        }
        text += "\n";
    }
    for s in [
        "refused (input): fee is out of range",
        // a value worked out on each element, named at its place on the list (§15.324)
        "refused (input): lines[1].subtotal is out of range",
        "refused (input): amount is out of range",
        "refused (input): total is out of range",
        "refused (input): charge is out of range",
        "refused (input): card:fee is out of range",
        "（input）: 手数料 が範囲の外です",
        "（input）: 明細[1].小計 が範囲の外です",
        "（input）: 金額 が範囲の外です",
        "（input）: 合計 が範囲の外です",
        "（input）: 料金 が範囲の外です",
    ] {
        assert!(text.contains(s), "no `{s}`:\n{text}");
    }
    golden("run.txt", &text);
}

/// The lines of each file that refuse a call for a value past the range it is accepted in: from the
/// line that works the value out to the one that refuses, in every target `gen` writes, the proof
/// harness and the plan NumPy reads included. A table above the value is no part of it, and the
/// sentence is the one the reference evaluator gives.
#[test]
fn the_generated_code_refuses_where_the_value_is_worked_out() {
    let t = TempDir::new("define-range-gen");
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    let mut args = vec!["gen", "tests/corpus/card_fee.rule", "tests/corpus/order_line_subtotals.rule", "--out", o.as_str()];
    args.extend(rulec::targets::UP_TO_0_40);
    let (code, said, e) = rulec(&root(), "en", &args, None);
    assert_eq!(code, 0, "{said}{e}");
    let mut files = Vec::new();
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else {
                out.push(p);
            }
        }
    }
    walk(&out, &mut files);
    files.sort();
    let sentences = ["fee is out of range", "subtotal is out of range"];
    let mut text = String::new();
    let mut targets = std::collections::BTreeSet::new();
    for f in &files {
        let rel = f.strip_prefix(&out).unwrap().to_string_lossy().replace('\\', "/");
        // the vectors are the evaluator's, and a page carries the module it is written beside
        if rel.starts_with("vectors/") || rel.ends_with("_page.html") {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(f) else { continue };
        let lines: Vec<&str> = body.lines().collect();
        let mut taken = Vec::new();
        for (i, l) in lines.iter().enumerate() {
            if !sentences.iter().any(|s| l.contains(s)) {
                continue;
            }
            if rel.ends_with(".json") {
                // NumPy's plan: the `accept` of the step that works the value out
                for s in sentences {
                    let mut at = 0;
                    while let Some(k) = l[at..].find("\"accept\":") {
                        let start = at + k;
                        let end = start + l[start..].find('}').unwrap() + 1;
                        if l[start..end].contains(s) {
                            taken.push(l[start..end].to_string());
                        }
                        at = end;
                    }
                }
                continue;
            }
            let from = (i.saturating_sub(4)..i).find(|&j| lines[j].contains("definition")).unwrap_or(i);
            taken.extend(lines[from..=i].iter().map(|s| s.trim_end().to_string()));
        }
        if !taken.is_empty() {
            targets.insert(rel.split('/').next().unwrap().to_string());
            text += &format!("== {rel}\n{}\n", taken.join("\n"));
        }
    }
    for want in ["python", "numpy", "typescript", "javascript", "rust", "ruby", "php", "go", "swift", "java", "sql", "wasm"] {
        assert!(targets.contains(want), "{want}: no line refuses the call: {targets:?}\n{text}");
    }
    golden("code.txt", &text);
}

/// A range wider than anything the value comes to is said nothing of, refuses nothing, is no
/// precondition, and puts no test in the generated code.
#[test]
fn a_range_wider_than_what_the_value_comes_to_is_said_nothing_of() {
    let t = materials();
    let d = t.path();
    for (rule, lang) in [("wider_fee.rule", "en"), ("広い範囲の手数料.rule", "ja")] {
        let (code, out, e) = rulec(d, lang, &["check", rule], None);
        assert_eq!((code, out.trim(), e.as_str()), (0, format!("ok {rule}").as_str(), ""), "{rule}");
        let (code, api, e) = rulec(d, lang, &["api", rule], None);
        assert_eq!(code, 0, "{e}");
        let api = rulec::json::parse(api.trim()).unwrap();
        assert_eq!(api.get("preconditions").and_then(|p| p.as_arr()).map(|p| p.len()), Some(0), "{rule}: {api:?}");
        let made = d.join(format!("gen-{lang}"));
        let (code, said, e) = rulec(d, lang, &["gen", rule, "--target", "python", "--out", &made.to_string_lossy()], None);
        assert_eq!(code, 0, "{said}{e}");
        let py: String = std::fs::read_dir(made.join("python")).unwrap().flatten().filter(|e| e.path().extension().is_some_and(|x| x == "py")).map(|e| std::fs::read_to_string(e.path()).unwrap()).collect();
        assert!(!py.contains("is out of range\", _repr(fee") && !py.contains("が範囲の外です\", _repr(fee") && py.contains("    fee = _round_down((amount * 3), 100) // 100  # "), "{rule}:\n{py}");
    }
}

/// What `check` says of a range it cannot take: a word after it (E047), no end (E149), no value
/// between its ends (E061), the days of a koyomi date (E065), a value that is not a number (E103,
/// a truth value and a date), and an end of another currency (E103). And a `constraint` on a
/// value worked out from the inputs (E017), whose note shows the range to write on a `define`
/// instead. In English, then Japanese.
#[test]
fn check_says_what_it_cannot_take_in_a_range() {
    // the date is an input only where the definition reads it
    let rule = |define: &str| {
        let date = if define.contains("paid_on") { "\n  paid_on : date  range >=2026-01-01 <=2026-12-31" } else { "" };
        format!(
            "rule card_fee v1\ndescription \"A card fee\"\n\ninputs\n  amount : money[JPY]  range >=1JPY <=1_000_000JPY{date}\n\n{define}\n\noutputs\n  charged : money[JPY]  round down(1JPY)\n\nresult charged = down(amount * 3%, 1JPY)\n"
        )
    };
    let cases = [
        ("E047", "define fee : money[JPY] = down(amount * 3%, 1JPY)  range >=0JPY <=20_000JPY round"),
        ("E149", "define fee : money[JPY] = down(amount * 3%, 1JPY)  range"),
        ("E061", "define fee : money[JPY] = down(amount * 3%, 1JPY)  range >=100JPY <100JPY"),
        ("E065", "define fee : money[JPY] = down(amount * 3%, 1JPY)  range from koyomi \"terms.cal\" date payment"),
        ("E103", "define large : bool = amount >= 500_000JPY  range >=1"),
        ("E103", "define due : date = paid_on  range <=2026-06-30"),
        ("E103", "define fee : money[JPY] = down(amount * 3%, 1JPY)  range >=0USD <=20_000JPY"),
        ("E017", "constraint amount * 3% <= 20_000JPY"),
    ];
    let t = TempDir::new("define-range-check");
    for lang in ["en", "ja"] {
        let mut text = String::new();
        for (i, (want, define)) in cases.iter().enumerate() {
            let name = format!("case{}.rule", i + 1);
            std::fs::write(t.path().join(&name), rule(define)).unwrap();
            let (code, out, e) = rulec(t.path(), lang, &["check", &name, "--format", "json"], None);
            assert_eq!(code, 1, "{define}: {out}{e}");
            let codes: Vec<String> = out.lines().filter_map(|l| rulec::json::parse(l).ok()).filter_map(|j| j.get("code").and_then(|c| c.as_str()).map(String::from)).collect();
            assert_eq!(codes, vec![want.to_string()], "{define}: {out}");
            let (_, said, _) = rulec(t.path(), lang, &["check", &name], None);
            text += &format!("$ rulec check {name}   # {define}\n{said}\n");
        }
        golden(&format!("check.{lang}.txt"), &text);
    }
}

/// What an apply brings in keeps the range the callee accepts its value in, and the caller refuses
/// the call there under the apply's name; a contribution's `define` keeps its range in the merged
/// rule, whose refused inputs carry it.
#[test]
fn an_apply_and_a_contribution_keep_the_range() {
    let t = materials();
    let d = t.path();
    let (code, out, e) = rulec(d, "en", &["check", "order.rule"], None);
    assert_eq!(code, 0, "{out}{e}");
    let (_, out, _) = rulec(d, "en", &["run", "order.rule", "total=666700"], None);
    assert_eq!(out.trim(), "refused (input): card:fee is out of range");
    let (_, out, _) = rulec(d, "en", &["run", "order.rule", "total=666699"], None);
    assert_eq!(out.trim(), "paid = 686_699JPY");
    let made = d.join("gen");
    let (code, said, e) = rulec(d, "en", &["gen", "order_discount.rule", "c/capped_campaign.rule", "--target", "python", "--extra", "vectors", "--out", &made.to_string_lossy()], None);
    assert_eq!(code, 0, "{said}{e}");
    let refused = std::fs::read_to_string(made.join("vectors/order_discount.refused.jsonl")).unwrap();
    let line = refused.lines().find(|l| l.contains("capped_campaign:counted is out of range")).unwrap_or_else(|| panic!("{refused}"));
    assert!(line.contains("\"subtotal\":3001") && line.contains("past the top of the range it accepts"), "{line}");
    // the vectors go up to the top end the contribution accepts, and never past it
    let vectors = std::fs::read_to_string(made.join("vectors/order_discount.jsonl")).unwrap();
    assert!(vectors.lines().any(|l| l.contains("\"subtotal\":3000,")), "{vectors}");
    let top = vectors.lines().filter_map(|l| rulec::json::parse(l).ok()).filter_map(|j| j.get("in").and_then(|i| i.get("subtotal")).and_then(|s| s.as_int())).max();
    assert_eq!(top, Some(3000), "{vectors}");
    let py = std::fs::read_to_string(made.join("python/order_discount.py")).unwrap();
    assert!(py.contains("raise RuleInputError(\"capped_campaign:counted is out of range\""), "{py}");
}

/// The ends of an accepted range are solved for by moving each input the value is worked out
/// from, and a move never leaves the range the input is held to: a value no move of its input
/// changes — the difference of a square and itself — has no slope to follow, and the search for
/// one stopped only at a step of a hundred sextillion, whose square is past what the evaluator's
/// numbers hold. The vectors are the two ends of the input, with the value on its range.
#[test]
fn the_search_for_an_end_keeps_its_inputs_inside_their_ranges() {
    let src = "rule flat v1\ndescription \"A value its input does not move\"\n\n\
               inputs\n  amount : number  range >=1 <=1000\n\n\
               define spread : number = amount * amount - amount * amount  range >=0 <=0\n\n\
               outputs\n  out : number  round down(1)\n\ndefine out : number = spread\n";
    let (f, c) = rulec::prepare(src, "flat.rule").unwrap_or_else(|d| panic!("{d:?}"));
    let suite = rulec::vectors::suite(&f, &c);
    let amounts: Vec<String> = suite.vectors.iter().map(|v| rulec::vectors::in_object(&f, &c, v)).collect();
    assert_eq!(amounts, [r#"{"amount":1}"#, r#"{"amount":1000}"#], "{amounts:?}");
    // an input with no top written is held to the end of its type, and the search stops there
    let open = src.replace("range >=1 <=1000", "range >=1").replace("amount * amount - amount * amount", "amount - amount");
    let (f, c) = rulec::prepare(&open, "flat.rule").unwrap_or_else(|d| panic!("{d:?}"));
    assert!(!rulec::vectors::suite(&f, &c).vectors.is_empty());
}
