//! The ends of a range, read the same by the check and by every door (§15.314).
//!
//! Two things were read two ways. An end written with `>` or `<` was read by the door, and by the
//! check of the tables, as if it took the value it writes; and the bottom of the range of a `sum`
//! or a `count` was what the check of the tables started from, while no walk was held to it. A
//! table that passed as complete then had no row for a call the door took.
//!
//! The corpus has the materials, English first with the Japanese version of each beside it: ends
//! that do not take the value they write on an amount, a rate, a date, an optional number and an
//! enum (`season_ticket`, `シーズン券`); a sum and a count whose ranges start above zero
//! (`gift_basket`, `贈り物のかご`); and the same ends on the field, the sum and the count of a walk
//! (`parcel_batch`, `小包のまとめ出荷`). The ones written here are a range with no value on its
//! step, claims written with such ends, a rate whose ends are off its step, a rule applied with
//! such a range, and an example on a walk under a bottom.
//!
//! What is held: what `rulec run` answers on each end and one step past it, in both languages;
//! the lines of the generated code of every target that refuse a walk under a bottom, and that a
//! rule whose ranges start at zero has none; the vectors on each end of what a walk is held to and
//! the refused inputs one step past; what `api`, `diff` and the port say of a bottom; and what
//! `check` says of a range it reads this way. Golden files under `tests/golden/range_ends`
//! (`RULEC_BLESS=1 cargo test --test range_ends` writes them again).

use ritsu_ports::{Answer, Precondition, RuleError, Rules, Value};
use ritsu_testkit::{Need, TempDir, ready};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use ritsu_testkit::run::finish;

/// How long a run of rulec may take: `rulec test` talks to the servers of the generated code,
/// and waits for their answers without a limit of its own. Past it, rulec is stopped with every
/// process it started, and the test fails saying what it ran and what it printed until then.
const RULEC_LIMIT: std::time::Duration = std::time::Duration::from_secs(20 * 60);

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rulec(dir: &Path, lang: &str, args: &[&str], stdin: Option<&str>) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rulec"));
    c.env("RULEC_LANG", lang).current_dir(dir).args(args).args(["--lang", lang]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
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
    if let Err(e) = ritsu_testkit::golden::check(&root().join("tests/golden/range_ends").join(name), text) {
        panic!("{e}");
    }
}

/// One run on a JSON input, as a reader would type it, and what it printed.
fn shown(dir: &Path, lang: &str, rule: &str, input: &str) -> String {
    let (code, out, e) = rulec(dir, lang, &["run", rule, "--input", "-"], Some(input));
    assert_eq!(code, 0, "{rule} {input}: {e}{out}");
    format!("$ echo '{input}' | rulec run {rule} --input -\n{out}")
}

/// A rate whose ends are written between two of its steps: each is read as the nearest step
/// inside, 1% and 99%.
const OFF_STEP: &str = "rule loan_share v1
description \"Whether a share of a loan is the larger part. The ends of the share are written between two steps of the rate, and each is read as the nearest step inside: from 1% to 99%\"

inputs
  share : rate[step 1%]  range >=0.5% <=99.5%

outputs
  larger : bool

table larger_of
policy unique
| share | -> larger |
| <50%  | false     |
| >=50% | true      |
";

const OFF_STEP_JA: &str = "rule 借入の持分(loan_share_ja) v1
description \"借入の持分が半分以上か。持分の端を、率の刻みと刻みのあいだに書いてある。どちらも内側のいちばん近い刻みで読むので、1% から 99% になる\"

inputs
  持分(share) : rate[step 1%]  range >=0.5% <=99.5%

outputs
  半分以上(larger) : bool

table 半分以上の判定(larger_of)
policy unique
| 持分  | -> 半分以上 |
| <50%  | false       |
| >=50% | true        |
";

/// An amount, and the price of a line, each above 0 yen and below 100 yen, read by a table that
/// has no boundary near either end: nothing but the range asks for a vector on 1 yen or on 99 yen.
const NARROW: &str = "rule entry_fee v1
description \"Whether an entry fee is of the larger kind. The fee is above 0 yen and below 100 yen\"

inputs
  amount : money[JPY]  range >0JPY <100JPY

outputs
  large : bool

table large_of
policy unique
| amount  | -> large |
| <50JPY  | false    |
| >=50JPY | true     |
";

const NARROW_LINES: &str = "rule entry_fees v1
description \"Whether the entry fees of a group come to the larger kind. Each fee is above 0 yen and below 100 yen\"

elements lines
  price : money[JPY]  range >0JPY <100JPY

sum amount over lines of price  range >=0JPY <=1_000JPY

outputs
  large : bool

table large_of
policy unique
| amount  | -> large |
| <50JPY  | false    |
| >=50JPY | true     |
";

/// What `check` is asked about a range read this way: the line under the inputs of one small
/// rule, and the code it gives (none where the rule passes).
/// A count that lets the lines be seven, and a sum they all fit under: no table reads the count,
/// so the pool has no case of seven lines, and one is made — each line with a code of its own.
const SEVEN: &str = "rule seven_lines v1

elements lines
  code  : number  range >=1 <=9  unique
  price : money[JPY]  range >=1JPY <=100JPY
  gift  : bool

count gifts over lines where gift  range >=0 <=7
sum amount over lines of price  range >=0JPY <=300JPY

outputs
  big : bool

table big_of
| amount   | -> big |
| <200JPY  | false  |
| >=200JPY | true   |
";

/// The same lines under a sum of 5 yen at most: seven lines of 1 yen come to 7 yen, and a line of
/// 100 yen is past it alone, so neither end has a call with an answer.
const SEVEN_SHORT: &str = "rule seven_short v1

elements lines
  code  : number  range >=1 <=9  unique
  price : money[JPY]  range >=1JPY <=100JPY
  gift  : bool

count gifts over lines where gift  range >=0 <=7
sum amount over lines of price  range >=0JPY <=5JPY

outputs
  big : bool

table big_of
| amount  | -> big |
| <3JPY   | false  |
| >=3JPY  | true   |
";

/// An order whose goods come to 2 yen at least, said on the value worked out from the total and
/// not on the total: an order with no lines is refused where the value is worked out, the pool has
/// no case of it (a case the rule refuses there is none of its cases), and one is made. The case
/// one step past the bottom of the value is an order of one line of 1 yen, so the order with no
/// lines is a case of its own.
const PAID_LINES: &str = "rule paid_lines v1

elements lines
  price : money[JPY]  range >=0JPY <=50_000JPY

sum total over lines of price  range >=0JPY <=100_000JPY

define goods : money[JPY] = total  range >=2JPY <=100_000JPY

outputs
  free : bool

table free_of
policy unique
| goods      | -> free |
| <5_000JPY  | false   |
| >=5_000JPY | true    |
";

fn cases() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        // no whole yen lies above 0 yen and below 1 yen
        ("", "inputs\n  amount : money[JPY]  range >0JPY <1JPY\n\noutputs\n  out : money[JPY]  round down(1JPY)\n\nresult out = amount", "E061"),
        // one value, 1 yen: a range, and the rule passes
        ("", "inputs\n  amount : money[JPY]  range >0JPY <2JPY\n\noutputs\n  out : money[JPY]  round down(1JPY)\n\nresult out = amount", ""),
        // no step of the rate lies between 0.2% and 0.8%
        ("", "inputs\n  share : rate[step 1%]  range >=0.2% <=0.8%\n  amount : money[JPY]  range >=0JPY <=100JPY\n\noutputs\n  out : money[JPY]  round down(1JPY)\n\nresult out = down(amount * share, 1JPY)", "E061"),
        // no day lies after the 1st and before the 2nd
        ("", "inputs\n  day : date  range >2026-01-01 <2026-01-02\n\noutputs\n  late : bool\n\ntable late_of\npolicy unique\n| day | -> late |\n| -   | false   |", "E061"),
        // a claim written with `>` and `<` is held to the value it writes: the value comes to 0 yen and to 200 yen
        ("", "inputs\n  amount : money[JPY]  range >=0JPY <=100JPY\n\nderive twice : money[JPY] = amount * 2  range >0JPY <200JPY\n\noutputs\n  out : money[JPY]  round down(1JPY)\n\nresult out = twice", "E112"),
        // each end alone: the top the claim leaves out, then the bottom
        ("", "inputs\n  amount : money[JPY]  range >=0JPY <=100JPY\n\nderive twice : money[JPY] = amount * 2  range >=0JPY <200JPY\n\noutputs\n  out : money[JPY]  round down(1JPY)\n\nresult out = twice", "E112"),
        ("", "inputs\n  amount : money[JPY]  range >=0JPY <=100JPY\n\nderive twice : money[JPY] = amount * 2  range >0JPY <=200JPY\n\noutputs\n  out : money[JPY]  round down(1JPY)\n\nresult out = twice", "E112"),
        // the largest price of a list is 100 yen where a line has it, and 0 yen where there is no line
        ("", "elements lines\n  price : money[JPY]  range >=0JPY <=100JPY\n\nmax top over lines of price empty 0JPY  range >=0JPY <100JPY\n\noutputs\n  big : bool\n\ntable big_of\npolicy unique\n| top     | -> big |\n| <50JPY  | false  |\n| >=50JPY | true   |", "E112"),
        ("", "elements lines\n  price : money[JPY]  range >=0JPY <=100JPY\n\nmax top over lines of price empty 0JPY  range >0JPY <=100JPY\n\noutputs\n  big : bool\n\ntable big_of\npolicy unique\n| top     | -> big |\n| <50JPY  | false  |\n| >=50JPY | true   |", "E112"),
        // the same claim one step wider holds
        ("", "inputs\n  amount : money[JPY]  range >=0JPY <=100JPY\n\nderive twice : money[JPY] = amount * 2  range >-1JPY <201JPY\n\noutputs\n  out : money[JPY]  round down(1JPY)\n\nresult out = twice", ""),
        // what an output answers reaches the end its range leaves out
        ("", "inputs\n  amount : money[JPY]  range >=0JPY <=100JPY\n\noutputs\n  out : money[JPY]  round down(1JPY)  range >0JPY <=200JPY\n\nresult out = amount * 2", "E140"),
        // and the top it leaves out
        ("", "inputs\n  amount : money[JPY]  range >=0JPY <=100JPY\n\noutputs\n  out : money[JPY]  round down(1JPY)  range >=0JPY <200JPY\n\nresult out = amount * 2", "E140"),
        // a table over a count written `>0 <4` starts at 1: no row for 0 is asked for
        ("", "elements lines\n  hit : bool\n\ncount hits over lines where hit  range >0 <4\n\noutputs\n  many : bool\n\ntable many_of\npolicy unique\n| hits | -> many |\n| 1    | false   |\n| >=2  | true    |", ""),
        // and a row for 0 is one no call reaches, as a row beyond the range of an input is
        ("", "elements lines\n  hit : bool\n\ncount hits over lines where hit  range >0 <4\n\noutputs\n  many : bool\n\ntable many_of\npolicy unique\n| hits | -> many |\n| 0    | false   |\n| >=1  | true    |", "E102"),
        // a cell that names a value under the bottom asks for no row there: `>0` over a count from 2 to 5
        ("", "elements lines\n  hit : bool\n\ncount hits over lines where hit  range >=2 <=5\n\noutputs\n  many : bool\n\ntable many_of\npolicy unique\n| hits  | -> many |\n| >0 <4 | false   |\n| >=4   | true    |", ""),
        // a row past the top of a sum is one no call reaches either
        ("", "elements lines\n  price : money[JPY]  range >=0JPY <=100JPY\n\nsum amount over lines of price  range >=0JPY <=300JPY\n\noutputs\n  many : bool\n\ntable many_of\npolicy unique\n| amount   | -> many |\n| <=300JPY | false   |\n| 500JPY   | true    |", "E102"),
        // an example on a walk the rule refuses: no element, under a count that starts at 1
        (
            "",
            "elements lines\n  hit : bool\n\ncount hits over lines where hit  range >=1 <=3\n\noutputs\n  many : bool\n\ntable many_of\npolicy unique\n| hits | -> many |\n| 1    | false   |\n| >=2  | true    |\n\nexamples\n| lines | -> many |\n| none_ | false   |\n\nsequence none_\n| hit |",
            "E019",
        ),
        // a value passed to a rule that is applied is held to the callee's range as its door reads it:
        // `entry_fee.rule` takes an amount above 0 yen and below 100 yen, so neither 0 yen nor 100 yen
        ("", "inputs\n  total : money[JPY]  range >=0JPY <=100JPY\n\noutputs\n  big : bool\n\napply fee = \"entry_fee.rule\" meaning:fd82f22cc17fe298\n  amount = total\n  large -> big", "E043"),
        // each end alone: the bottom the callee leaves out, then the top
        ("", "inputs\n  total : money[JPY]  range >=0JPY <=99JPY\n\noutputs\n  big : bool\n\napply fee = \"entry_fee.rule\" meaning:fd82f22cc17fe298\n  amount = total\n  large -> big", "E043"),
        ("", "inputs\n  total : money[JPY]  range >=1JPY <=100JPY\n\noutputs\n  big : bool\n\napply fee = \"entry_fee.rule\" meaning:fd82f22cc17fe298\n  amount = total\n  large -> big", "E043"),
        // the same range on this side, and the one it is read as, both pass
        ("", "inputs\n  total : money[JPY]  range >0JPY <100JPY\n\noutputs\n  big : bool\n\napply fee = \"entry_fee.rule\" meaning:fd82f22cc17fe298\n  amount = total\n  large -> big", ""),
        ("", "inputs\n  total : money[JPY]  range >=1JPY <=99JPY\n\noutputs\n  big : bool\n\napply fee = \"entry_fee.rule\" meaning:fd82f22cc17fe298\n  amount = total\n  large -> big", ""),
        // an open value is held to its range the same way: its terms come to 0 dollars, which `>0USD` leaves out
        ("", "inputs\n  member : bool\n\noutputs\n  discount : money[USD]  round down(1USD)\n\ntable member_part\npolicy unique\n| member | -> member_off : money[USD] |\n| true   | 5USD                       |\n| false  | 0USD                       |\n\nopen stackable : money[USD]  combine sum  range >0USD <=5USD\n  member_off\n\ndefine discount : money[USD] = stackable", "E131"),
        // and to 5 dollars, which `<5USD` leaves out
        ("", "inputs\n  member : bool\n\noutputs\n  discount : money[USD]  round down(1USD)\n\ntable member_part\npolicy unique\n| member | -> member_off : money[USD] |\n| true   | 5USD                       |\n| false  | 0USD                       |\n\nopen stackable : money[USD]  combine sum  range >=0USD <5USD\n  member_off\n\ndefine discount : money[USD] = stackable", "E131"),
        // from 1 dollar to 5 dollars, inside `>0USD <6USD`
        ("", "inputs\n  member : bool\n\noutputs\n  discount : money[USD]  round down(1USD)\n\ntable member_part\npolicy unique\n| member | -> member_off : money[USD] |\n| true   | 5USD                       |\n| false  | 1USD                       |\n\nopen stackable : money[USD]  combine sum  range >0USD <6USD\n  member_off\n\ndefine discount : money[USD] = stackable", ""),
    ]
}

/// On each end and one step past it, for each material in its language. On an end the rule
/// answers; one step past it the run is refused, for the range of the name that passed it.
#[test]
fn run_answers_on_each_end_and_refuses_one_step_past_it() {
    let c = root().join("tests/corpus");
    let ticket = |price: &str, discount: &str, day: &str, age: &str, seat: &str, ja: bool| -> String {
        let (p, d, y, a, s) = if ja { ("価格", "割引率", "日付", "年齢", "席") } else { ("price", "discount", "day", "age", "seat") };
        format!("{{\"{p}\":{price},\"{d}\":{discount},\"{y}\":\"{day}\",\"{a}\":{age},\"{s}\":\"{seat}\"}}")
    };
    let tickets = |ja: bool| -> Vec<String> {
        let (standing, reserved, premium) = if ja { ("立ち見", "指定席", "特別席") } else { ("standing", "reserved", "premium") };
        vec![
            // every input on the bottom of its range, then on the top
            ticket("1", "1", "2026-04-01", "6", reserved, ja),
            ticket("99999", "49", "2026-09-30", "119", premium, ja),
            // each one step past an end: the value the rule writes there
            ticket("0", "1", "2026-04-01", "6", reserved, ja),
            ticket("100000", "1", "2026-04-01", "6", reserved, ja),
            ticket("1", "0", "2026-04-01", "6", reserved, ja),
            ticket("1", "50", "2026-04-01", "6", reserved, ja),
            ticket("1", "1", "2026-03-31", "6", reserved, ja),
            ticket("1", "1", "2026-10-01", "6", reserved, ja),
            ticket("1", "1", "2026-04-01", "5", reserved, ja),
            ticket("1", "1", "2026-04-01", "120", reserved, ja),
            ticket("1", "1", "2026-04-01", "null", standing, ja),
        ]
    };
    let seq = |name: &str, fields: (&str, &str), rows: &[(i64, bool)]| -> String {
        let rows: Vec<String> = rows.iter().map(|(n, b)| format!("{{\"{}\":{n},\"{}\":{b}}}", fields.0, fields.1)).collect();
        format!("{{\"{name}\":[{}]}}", rows.join(","))
    };
    let baskets = |ja: bool| -> Vec<String> {
        let (name, fields) = if ja { ("明細", ("価格", "包装")) } else { ("lines", ("price", "wrapped")) };
        vec![
            // the least a basket can be, and the most
            seq(name, fields, &[(500, true)]),
            seq(name, fields, &[(20000, true), (20000, true), (20000, false), (20000, false), (20000, false)]),
            // no line at all, nothing wrapped, one yen short, one yen over, one line too many
            seq(name, fields, &[]),
            seq(name, fields, &[(500, false)]),
            seq(name, fields, &[(499, true)]),
            seq(name, fields, &[(50000, true), (50000, true), (1, true)]),
            seq(name, fields, &[(100, true); 6]),
        ]
    };
    let batches = |ja: bool| -> Vec<String> {
        let (name, fields) = if ja { ("小包", ("重さ", "割れ物")) } else { ("parcels", ("weight", "fragile")) };
        vec![
            // the least a batch can be, and the most
            seq(name, fields, &[(1000, true)]),
            seq(name, fields, &[(30000, true), (29999, true), (1, true)]),
            // each on the value its range writes with `>` or `<`
            seq(name, fields, &[(999, true)]),
            seq(name, fields, &[(30000, true), (30000, true), (1, true)]),
            seq(name, fields, &[(1000, false)]),
            seq(name, fields, &[(0, true)]),
            seq(name, fields, &[(30001, true)]),
            seq(name, fields, &[(1000, true); 4]),
        ]
    };
    let mut text = String::new();
    for (rule, lang, inputs) in [
        ("season_ticket.rule", "en", tickets(false)),
        ("シーズン券.rule", "ja", tickets(true)),
        ("gift_basket.rule", "en", baskets(false)),
        ("贈り物のかご.rule", "ja", baskets(true)),
        ("parcel_batch.rule", "en", batches(false)),
        ("小包のまとめ出荷.rule", "ja", batches(true)),
    ] {
        for input in &inputs {
            text += &shown(&c, lang, rule, input);
        }
        text += "\n";
    }
    let t = TempDir::new("range-ends-run");
    for (name, body, lang, key) in [("loan_share.rule", OFF_STEP, "en", "share"), ("借入の持分.rule", OFF_STEP_JA, "ja", "持分")] {
        std::fs::write(t.path().join(name), body).unwrap();
        for v in ["0", "1", "99", "100"] {
            text += &shown(t.path(), lang, name, &format!("{{\"{key}\":{v}}}"));
        }
        text += "\n";
    }
    for (s, times) in [
        ("refused (input): price is out of range", 2),
        ("refused (input): discount is out of range", 2),
        ("refused (input): day is out of range", 2),
        ("refused (input): age is out of range", 2),
        ("refused (input): seat is out of range", 1),
        ("refused (input): wrapped_items is out of range", 2),
        ("refused (input): amount is out of range", 2),
        ("refused (input): lines has too many elements (at most 5)", 1),
        ("refused (input): total is out of range", 2),
        ("refused (input): fragile_parcels is out of range", 1),
        ("refused (input): parcels[0].weight is out of range", 2),
        ("refused (input): parcels has too many elements (at most 3)", 1),
        ("refused (input): share is out of range", 2),
        ("（input）: 価格 が範囲の外です", 2),
        ("（input）: 包装した品 が範囲の外です", 2),
        ("（input）: 金額 が範囲の外です", 2),
        ("（input）: 小包[0].重さ が範囲の外です", 2),
        ("（input）: 総重量 が範囲の外です", 2),
        ("（input）: 割れ物の数 が範囲の外です", 1),
        ("（input）: 持分 が範囲の外です", 2),
    ] {
        assert_eq!(text.matches(s).count(), times, "`{s}`:\n{text}");
    }
    golden("run.txt", &text);
}

/// The lines of each file that refuse a walk for a `sum` or a `count` outside its range, in every
/// target `gen` writes and the proof harness: the comparison with the top, and with the bottom
/// where the range starts above zero. A rule whose ranges start at zero compares no bottom, and
/// its code is as it was.
#[test]
fn the_generated_code_compares_the_bottom_a_walk_can_come_in_under() {
    let t = TempDir::new("range-ends-gen");
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    let mut args = vec!["gen", "tests/corpus/gift_basket.rule", "tests/corpus/parcel_batch.rule", "tests/corpus/cart_shipping_fee.rule", "--out", o.as_str()];
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
    // the summaries of the three rules: the first four have a bottom above zero
    let floored = ["wrapped_items is out of range", "amount is out of range", "total is out of range", "fragile_parcels is out of range"];
    let mut text = String::new();
    let mut targets = std::collections::BTreeSet::new();
    for f in &files {
        let rel = f.strip_prefix(&out).unwrap().to_string_lossy().replace('\\', "/");
        // the vectors are the evaluator's, a page carries the module it is written beside, and a
        // runner, a server and a round test call the module
        if rel.starts_with("vectors/") || rel.ends_with("_page.html") || rel.to_lowercase().contains("runner") || rel.contains("_mcp") || rel.contains("connect") || rel.contains("round") {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(f) else { continue };
        let lines: Vec<&str> = body.lines().collect();
        let mut taken = Vec::new();
        for (i, l) in lines.iter().enumerate() {
            if floored.iter().any(|s| l.contains(s)) && !l.trim_start().starts_with("//") && !l.trim_start().starts_with('#') {
                // the comparison is on the line, or on the one above it
                let from = if l.contains('<') || l.contains('>') || l.contains("cover?") { i } else { i.saturating_sub(1) };
                taken.extend(lines[from..=i].iter().map(|s| s.trim().to_string()));
            }
        }
        if !taken.is_empty() {
            let target = rel.split('/').next().unwrap().to_string();
            // a bottom is compared in the module of each language
            if !rel.contains("cart_shipping_fee") && taken.iter().any(|l| l.contains("< 500") || l.contains("< 1000") || l.contains("< 1")) {
                targets.insert(target);
            }
            text += &format!("== {rel}\n{}\n", taken.join("\n"));
        }
    }
    for want in ["python", "typescript", "javascript", "rust", "ruby", "php", "go", "swift", "java"] {
        assert!(targets.contains(want), "{want}: no line compares a bottom: {targets:?}\n{text}");
    }
    // `total` of cart_shipping_fee starts at 0 yen: its top is compared, and nothing else
    let cart = std::fs::read_to_string(out.join("python/cart_shipping_fee.py")).unwrap();
    assert!(cart.contains("    if total > 1000000:\n") && !cart.contains("    if total < "), "{cart}");
    golden("code.txt", &text);
}

/// The vectors put each summary a walk is held to on its ends, and the refused inputs take it one
/// step past them: the top of a sum, the bottom of a sum and of a count that starts above zero,
/// and the first value inside an end written with `>` or `<`.
#[test]
fn the_vectors_stand_on_each_end_and_the_refused_inputs_one_step_past() {
    let t = TempDir::new("range-ends-vectors");
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    let (code, said, e) = rulec(
        &root(),
        "en",
        &["gen", "tests/corpus/gift_basket.rule", "tests/corpus/parcel_batch.rule", "tests/corpus/season_ticket.rule", "tests/corpus/cart_shipping_fee.rule", "--target", "python", "--extra", "vectors", "--out", &o],
        None,
    );
    assert_eq!(code, 0, "{said}{e}");
    let read = |f: &str| std::fs::read_to_string(out.join("vectors").join(f)).unwrap();
    let has = |file: &str, input: &str, rest: &str| {
        let body = read(file);
        assert!(body.lines().any(|l| l.starts_with(&format!("{{\"in\":{input},")) && l.contains(rest)), "{file}: no line with {input} and {rest}:\n{body}");
    };
    // the basket: 500 yen and 100,000 yen are answered, 499 yen and 100,001 yen are refused for the
    // amount, and no line at all for the count
    has("gift_basket.jsonl", r#"{"lines":[{"price":500,"wrapped":true}]}"#, "\"out\"");
    has("gift_basket.jsonl", r#"{"lines":[{"price":50000,"wrapped":true},{"price":50000,"wrapped":true}]}"#, "\"out\"");
    has("gift_basket.refused.jsonl", r#"{"lines":[{"price":499,"wrapped":true}]}"#, "\"error\":\"amount is out of range\",\"why\":\"the sum amount of lines past the bottom of its range\"");
    has("gift_basket.refused.jsonl", r#"{"lines":[{"price":50000,"wrapped":true},{"price":50000,"wrapped":true},{"price":1,"wrapped":true}]}"#, "\"error\":\"amount is out of range\",\"why\":\"the sum amount of lines past the top of its range\"");
    has("gift_basket.refused.jsonl", r#"{"lines":[]}"#, "\"error\":\"wrapped_items is out of range\",\"why\":\"the count wrapped_items of lines past the bottom of its range\"");
    // the batch: 1,000 g and 60,000 g are answered, 999 g and 60,001 g refused; a parcel of 0 g and
    // one of 30,001 g are the values its range writes, each put second and third after a parcel the
    // batch takes, and the first of them is named (§15.324)
    has("parcel_batch.jsonl", r#"{"parcels":[{"weight":1000,"fragile":true}]}"#, "\"out\"");
    has("parcel_batch.jsonl", r#"{"parcels":[{"weight":30000,"fragile":true},{"weight":30000,"fragile":true}]}"#, "\"out\"");
    has("parcel_batch.refused.jsonl", r#"{"parcels":[{"weight":999,"fragile":true}]}"#, "\"error\":\"total is out of range\"");
    has("parcel_batch.refused.jsonl", r#"{"parcels":[{"weight":30000,"fragile":true},{"weight":30000,"fragile":true},{"weight":1,"fragile":true}]}"#, "\"error\":\"total is out of range\"");
    has("parcel_batch.refused.jsonl", r#"{"parcels":[{"weight":29999,"fragile":true},{"weight":0,"fragile":true},{"weight":0,"fragile":true}]}"#, "\"error\":\"parcels[1].weight is out of range\"");
    has("parcel_batch.refused.jsonl", r#"{"parcels":[{"weight":29999,"fragile":true},{"weight":30001,"fragile":true},{"weight":30001,"fragile":true}]}"#, "\"error\":\"parcels[1].weight is out of range\"");
    assert!(read("parcel_batch.refused.jsonl").lines().any(|l| l.contains("\"error\":\"fragile_parcels is out of range\"")), "{}", read("parcel_batch.refused.jsonl"));
    // the ticket: each input on the first value inside each end, and refused on the value written
    let ticket = read("season_ticket.jsonl");
    for on in ["\"price\":1,", "\"price\":99999,", "\"discount\":1,", "\"discount\":49,", "\"day\":\"2026-04-01\"", "\"day\":\"2026-09-30\"", "\"age\":6,", "\"age\":119,"] {
        assert!(ticket.lines().any(|l| l.contains(on)), "season_ticket.jsonl: no vector with {on}");
    }
    let refused = read("season_ticket.refused.jsonl");
    for (past, said) in [
        ("\"price\":0,", "price is out of range"),
        ("\"price\":100000,", "price is out of range"),
        ("\"discount\":0,", "discount is out of range"),
        ("\"discount\":50,", "discount is out of range"),
        ("\"day\":\"2026-03-31\"", "day is out of range"),
        ("\"day\":\"2026-10-01\"", "day is out of range"),
        ("\"age\":5,", "age is out of range"),
        ("\"age\":120,", "age is out of range"),
        ("\"seat\":\"standing\"", "seat is out of range"),
    ] {
        assert!(refused.lines().any(|l| l.contains(past) && l.contains(&format!("\"error\":\"{said}\""))), "season_ticket.refused.jsonl: no line with {past} refused as `{said}`:\n{refused}");
    }
    // a sum that starts at zero: on its top and one yen past it, and nothing about a bottom
    let top = format!("\"lines\":[{}]", vec![r#"{"amount":100000}"#; 10].join(","));
    assert!(read("cart_shipping_fee.jsonl").lines().any(|l| l.contains(&top) && l.contains("\"out\"")), "{}", read("cart_shipping_fee.jsonl"));
    assert!(read("cart_shipping_fee.refused.jsonl").lines().any(|l| l.contains("{\"amount\":1}]") && l.contains("the sum total of lines past the top of its range")), "{}", read("cart_shipping_fee.refused.jsonl"));
    assert!(!read("cart_shipping_fee.refused.jsonl").contains("past the bottom"));

    // an end no cell stands near: the vectors still go to the first value inside it, and the
    // refused inputs to the value it writes
    let t = TempDir::new("range-ends-narrow");
    std::fs::write(t.path().join("entry_fee.rule"), NARROW).unwrap();
    std::fs::write(t.path().join("entry_fees.rule"), NARROW_LINES).unwrap();
    let (code, said, e) = rulec(t.path(), "en", &["gen", "entry_fee.rule", "entry_fees.rule", "--target", "python", "--extra", "vectors", "--out", "out"], None);
    assert_eq!(code, 0, "{said}{e}");
    let read = |f: &str| std::fs::read_to_string(t.path().join("out/vectors").join(f)).unwrap();
    for (file, inside, past) in [
        ("entry_fee", [r#"{"in":{"amount":1},"out""#, r#"{"in":{"amount":99},"out""#], [r#"{"in":{"amount":0},"refused":"input","error":"amount is out of range""#, r#"{"in":{"amount":100},"refused":"input","error":"amount is out of range""#]),
        (
            "entry_fees",
            // 99 yen stands in the lines of a case the suite had
            [r#"{"in":{"lines":[{"price":1}]},"out""#, r#"{"in":{"lines":[{"price":99},{"price":99},"#],
            [
                r#"{"in":{"lines":[{"price":1},{"price":0},{"price":0}]},"refused":"input","error":"lines[1].price is out of range""#,
                r#"{"in":{"lines":[{"price":1},{"price":100},{"price":100}]},"refused":"input","error":"lines[1].price is out of range""#,
            ],
        ),
    ] {
        let (answered, refused) = (read(&format!("{file}.jsonl")), read(&format!("{file}.refused.jsonl")));
        for l in inside {
            assert!(answered.lines().any(|x| x.starts_with(l)), "{file}.jsonl: no line that starts {l}:\n{answered}");
        }
        for l in past {
            assert!(refused.lines().any(|x| x.starts_with(l)), "{file}.refused.jsonl: no line that starts {l}:\n{refused}");
        }
    }
}

/// Every end a range of the corpus writes has a case standing on it (§15.314): an input and a
/// field of an element on each end of its range — a number, an amount, a rate, a day, an optional
/// value, an ordered enum — a sequence with as many elements as a count lets it have, and a
/// sequence with none. The case is one with an answer; where no call on the end has one — a
/// `define` works out past the range the rule accepts for it, a `constraint` rules the end out, a
/// sum or a count starts above what no elements come to — it is a refused case, with what the call
/// is refused for. And no input is written twice in the refused cases.
#[test]
fn every_written_end_of_the_corpus_has_a_case_on_it() {
    let mut rules: Vec<PathBuf> = std::fs::read_dir(root().join("tests/corpus")).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "rule")).collect();
    rules.sort();
    let (mut ends, mut answered, mut refused_only) = (0, 0, Vec::new());
    for p in &rules {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let src = std::fs::read_to_string(p).unwrap();
        let Ok((f, c)) = rulec::prepare(&src, &p.to_string_lossy()) else { continue };
        let suite = rulec::vectors::suite(&f, &c);
        for (why, on, turned) in rulec::vectors::ends_stood_on(&f, &c, &suite) {
            ends += 1;
            assert!(on || turned, "{name}: no case on the end: {why}");
            if on {
                answered += 1;
            } else {
                refused_only.push(format!("{name}: {why}"));
            }
        }
        let lines = rulec::vectors::refused_lines(&f, &c, &suite);
        let mut seen = std::collections::BTreeSet::new();
        for l in &lines {
            let j = rulec::json::parse(l).unwrap();
            let input = j.get("in").unwrap().compact();
            assert!(seen.insert(input.clone()), "{name}: the refused cases write {input} twice");
            assert!(!suite.vectors.iter().any(|v| rulec::json::parse(&rulec::vectors::in_object(&f, &c, v)).unwrap().compact() == input), "{name}: {input} is answered and refused");
        }
    }
    println!("ends: {ends}, with an answered case {answered}, with a refused case alone {}", refused_only.len());
    // the ends no call stands on with an answer: the fee of a card payment of 1,000,000 yen is past
    // what the rule accepts, so is the subtotal of one unit at 50,000,000 yen, and a quota used up
    // leaves no room for a request of one; and a basket and a batch with nothing in them come in
    // under the bottom of a count and of a total
    assert_eq!(
        refused_only,
        [
            "card_fee.rule: amount at the top of its range",
            "gift_basket.rule: no elements of lines",
            "order_line_subtotals.rule: price at the top of its range, in an element of lines",
            "parcel_batch.rule: no elements of parcels",
            "quota_request.rule: used at the top of its range",
            "カード手数料.rule: 金額 at the top of its range",
            "割り当ての申し込み.rule: 使った量 at the top of its range",
            "小包のまとめ出荷.rule: no elements of 小包",
            "明細の小計.rule: 単価 at the top of its range, in an element of 明細",
            "贈り物のかご.rule: no elements of 明細",
        ]
    );
    assert!(ends >= 600 && answered + refused_only.len() == ends, "{ends} {answered}");
}

/// What the refused cases say of an end no call stands on with an answer, and what the vectors
/// say of one that was not among them before: the reason names the end, and the refused line
/// carries what the reference evaluator refuses the call for.
#[test]
fn an_end_with_no_answer_is_a_refused_case_with_its_reason() {
    let t = TempDir::new("range-ends-on");
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    let rules = ["card_fee", "order_line_subtotals", "quota_request", "osha_noise", "order_period", "parcel_fee", "gift_basket", "カード手数料"];
    let mut args: Vec<String> = vec!["gen".into()];
    args.extend(rules.iter().map(|r| format!("tests/corpus/{r}.rule")));
    args.extend(["--target", "python", "--extra", "vectors", "--out", &o].map(String::from));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, said, e) = rulec(&root(), "en", &args, None);
    assert_eq!(code, 0, "{said}{e}");
    let read = |f: &str| std::fs::read_to_string(out.join("vectors").join(f)).unwrap();
    let has = |file: &str, line: &str| {
        let body = read(file);
        assert!(body.lines().any(|l| l.starts_with(line)), "{file}: no line that starts {line}:\n{body}");
    };
    // no answer on the end: the fee, the subtotal and the constraint are what the call is refused for
    has("card_fee.refused.jsonl", r#"{"in":{"amount":1000000},"refused":"input","error":"fee is out of range","why":"amount at the top of its range, where no case tried is answered"}"#);
    has("order_line_subtotals.refused.jsonl", r#"{"in":{"lines":[{"quantity":1,"price":50000000}]},"refused":"input","error":"lines[0].subtotal is out of range","why":"price at the top of its range, in an element of lines, where no case tried is answered"}"#);
    has("quota_request.refused.jsonl", r#"{"in":{"used":1000,"requested":1,"quota":1},"refused":"input","error":"the constraint does not hold: used + requested <= quota","why":"a combination the constraint used + requested <= quota rules out"}"#);
    // the same in Japanese
    let ja = t.path().join("ja");
    let (code, said, e) = rulec(&root(), "ja", &["gen", "tests/corpus/カード手数料.rule", "--target", "python", "--extra", "vectors", "--out", &ja.to_string_lossy()], None);
    assert_eq!(code, 0, "{said}{e}");
    let body = std::fs::read_to_string(ja.join("vectors/card_fee_ja.refused.jsonl")).unwrap();
    assert!(body.lines().any(|l| l.starts_with(r#"{"in":{"金額":1000000},"refused":"input","#) && l.ends_with(r#""why":"金額 を範囲の上端に（試した組み合わせのどれにも答えが無い）"}"#)), "{body}");
    // an answer on the end: 130 dB, the last day of the year, five parcels, a line of 0 yen
    has("osha_noise.jsonl", r#"{"in":{"level":130},"out":"#);
    has("osha_noise.jsonl", r#"{"in":{"level":90},"out":"#);
    has("order_period.jsonl", r#"{"in":{"order_date":"2026-12-31"},"out":"#);
    has("order_period.jsonl", r#"{"in":{"order_date":"2026-01-01"},"out":"#);
    let five = read("parcel_fee.jsonl").lines().filter(|l| l.matches("\"weight\":").count() == 5).count();
    assert!(five >= 1, "parcel_fee.jsonl: no vector with five parcels:\n{}", read("parcel_fee.jsonl"));
    assert!(read("gift_basket.jsonl").lines().any(|l| l.contains(r#"{"price":0,"#) && l.contains("\"out\"")), "{}", read("gift_basket.jsonl"));
    // seven lines, made where the pool has none; and where seven lines and a line of 100 yen have
    // no answer, both are refused for the amount
    let t = TempDir::new("range-ends-seven");
    std::fs::write(t.path().join("seven_lines.rule"), SEVEN).unwrap();
    std::fs::write(t.path().join("seven_short.rule"), SEVEN_SHORT).unwrap();
    let (code, said, e) = rulec(t.path(), "en", &["gen", "seven_lines.rule", "seven_short.rule", "--target", "python", "--extra", "vectors", "--out", "out"], None);
    assert_eq!(code, 0, "{said}{e}");
    let made = |f: &str| std::fs::read_to_string(t.path().join("out/vectors").join(f)).unwrap();
    let seven: Vec<String> = (1..=7).map(|n| format!(r#"{{"code":{n},"price":1,"gift":true}}"#)).collect();
    let seven = format!(r#"{{"in":{{"lines":[{}]}},"#, seven.join(","));
    assert!(made("seven_lines.jsonl").lines().any(|l| l.starts_with(&seven) && l.ends_with(r#""why":"lines with as many elements as a count over it can be"}"#)), "{}", made("seven_lines.jsonl"));
    let short = made("seven_short.refused.jsonl");
    assert!(short.lines().any(|l| l == format!(r#"{seven}"refused":"input","error":"amount is out of range","why":"lines with as many elements as a count over it can be, where no case tried is answered"}}"#)), "{short}");
    assert!(
        short.lines().any(|l| l.starts_with(r#"{"in":{"lines":[{"code":1,"price":100,"gift":true}]},"refused":"input","error":"amount is out of range","why":"price at the top of its range, in an element of lines, where no case tried is answered"}"#)),
        "{short}"
    );
    assert!(!made("seven_short.jsonl").contains("\"price\":100"), "{}", made("seven_short.jsonl"));
    // two inputs under one end both stay: the line of 0 yen the pool found under the bottom of the
    // amount, beside the one made one yen under it; and a batch with no parcels beside the one a
    // gram under the bottom of the total
    let basket = read("gift_basket.refused.jsonl");
    assert_eq!(basket.lines().filter(|l| l.contains("\"error\":\"amount is out of range\"")).count(), 3, "{basket}");
    has("gift_basket.refused.jsonl", r#"{"in":{"lines":[{"price":0,"wrapped":true}]},"refused":"input","error":"amount is out of range","#);
    has("gift_basket.refused.jsonl", r#"{"in":{"lines":[{"price":499,"wrapped":true}]},"refused":"input","error":"amount is out of range","why":"the sum amount of lines past the bottom of its range"}"#);
    let (code, said, e) = rulec(&root(), "en", &["gen", "tests/corpus/parcel_batch.rule", "tests/corpus/小包のまとめ出荷.rule", "--target", "python", "--extra", "vectors", "--out", &o], None);
    assert_eq!(code, 0, "{said}{e}");
    has("parcel_batch.refused.jsonl", r#"{"in":{"parcels":[]},"refused":"input","error":"total is out of range","why":"no elements"}"#);
    has("parcel_batch.refused.jsonl", r#"{"in":{"parcels":[{"weight":999,"fragile":true}]},"refused":"input","error":"total is out of range","why":"the sum total of parcels past the bottom of its range"}"#);
    has("parcel_batch_ja.refused.jsonl", r#"{"in":{"小包":[]},"refused":"input","error":"総重量 is out of range","#);
    has("gift_basket.refused.jsonl", r#"{"in":{"lines":[]},"refused":"input","error":"wrapped_items is out of range","#);
}

/// A generated door that refuses the value on an end itself is stopped by `rulec test`: the top
/// and the bottom of an input, of a day and of a field of an element, and a sequence of as many
/// elements as a count lets it have. Each change is one a hand could make to the generated Python,
/// and before the vectors stood on every end, the top of the level, of the day and of the count,
/// and the bottom of the line's price, passed. So is a walk that holds a total to its bottom only
/// when there is an element, or only when the total is not zero: the batch with no parcels and the
/// basket of one line of 0 yen are refused for their totals, and those lines of the refused cases
/// — the walks the suite found far under the bottom, beside the ones made one step under it — are
/// the ones that stop them.
#[test]
fn rulec_test_stops_a_door_that_refuses_the_value_on_an_end() {
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not here") {
        return;
    }
    let t = TempDir::new("range-ends-mutants");
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    let (code, said, e) = rulec(
        &root(),
        "en",
        &[
            "gen",
            "tests/corpus/osha_noise.rule",
            "tests/corpus/order_period.rule",
            "tests/corpus/gift_basket.rule",
            "tests/corpus/parcel_fee.rule",
            "tests/corpus/parcel_batch.rule",
            "--target",
            "python",
            "--extra",
            "runner",
            "--extra",
            "vectors",
            "--out",
            &o,
        ],
        None,
    );
    assert_eq!(code, 0, "{said}{e}");
    // every rule that ran, and whether it agreed
    let test = || -> Vec<(String, bool)> {
        let tmp = TempDir::new("range-ends-mutants-tmp");
        let run = finish(Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp.path()).args(["test", &o, "--format", "json"]), RULEC_LIMIT, "rulec test running the generated code");
        let said = run.stdout.clone();
        let j = rulec::json::parse(said.lines().next().unwrap_or_default()).unwrap_or_else(|x| panic!("{x}: {said}{}", run.stderr));
        j.get("results")
            .and_then(|v| v.as_arr())
            .unwrap_or_default()
            .iter()
            .filter(|r| r.get("ran").and_then(|v| v.as_bool()) == Some(true))
            .map(|r| (r.get("rule").and_then(|v| v.as_str()).unwrap_or_default().to_string(), r.get("ok").and_then(|v| v.as_bool()) == Some(true)))
            .collect()
    };
    let clean = test();
    assert_eq!(clean.len(), 5, "{clean:?}");
    assert!(clean.iter().all(|(_, ok)| *ok), "{clean:?}");
    for (rule, written, changed) in [
        ("osha_noise", "    if not 90 <= level <= 130:\n", "    if not 90 <= level <= 129:\n"),
        ("osha_noise", "    if not 90 <= level <= 130:\n", "    if not 91 <= level <= 130:\n"),
        ("order_period", "    if not 20454 <= order_date <= 20818:\n", "    if not 20454 <= order_date <= 20817:\n"),
        ("order_period", "    if not 20454 <= order_date <= 20818:\n", "    if not 20455 <= order_date <= 20818:\n"),
        ("gift_basket", "        if not 0 <= elem.price <= 50000:\n", "        if not 0 <= elem.price <= 49999:\n"),
        ("gift_basket", "        if not 0 <= elem.price <= 50000:\n", "        if not 1 <= elem.price <= 50000:\n"),
        ("parcel_fee", "    if _len(parcels) > 5:\n", "    if _len(parcels) > 4:\n"),
        ("parcel_batch", "    if total < 1000:\n", "    if total < 1000 and _len(parcels) > 0:\n"),
        ("gift_basket", "    if amount < 500:\n", "    if amount < 500 and amount > 0:\n"),
    ] {
        let file = out.join("python").join(format!("{rule}.py"));
        let body = std::fs::read_to_string(&file).unwrap();
        assert_eq!(body.matches(written).count(), 1, "{rule}.py: `{}` is not there once", written.trim());
        std::fs::write(&file, body.replace(written, changed)).unwrap();
        let got = test();
        std::fs::write(&file, &body).unwrap();
        for (r, ok) in &got {
            assert_eq!(*ok, r != rule, "{rule}.py with `{}`: {got:?}", changed.trim());
        }
    }
}

/// A sequence with no elements has a case in every rule that walks one, whatever refuses it: where
/// the pool has none — the value worked out from the total is what refuses the empty order — the
/// case is made, and it is the line that stops a generated door which holds the value to its
/// bottom only when there is a line.
#[test]
fn a_sequence_with_no_elements_has_a_case_where_the_pool_has_none() {
    let t = TempDir::new("range-ends-empty");
    std::fs::write(t.path().join("paid_lines.rule"), PAID_LINES).unwrap();
    let (code, said, e) = rulec(t.path(), "en", &["gen", "paid_lines.rule", "--target", "python", "--extra", "runner", "--extra", "vectors", "--out", "out"], None);
    assert_eq!(code, 0, "{said}{e}");
    let refused = std::fs::read_to_string(t.path().join("out/vectors/paid_lines.refused.jsonl")).unwrap();
    assert!(
        refused.lines().any(|l| l == r#"{"in":{"lines":[]},"refused":"input","error":"goods is out of range","why":"no elements of lines, where no case tried is answered"}"#),
        "{refused}"
    );
    let answered = std::fs::read_to_string(t.path().join("out/vectors/paid_lines.jsonl")).unwrap();
    assert!(!answered.contains(r#""lines":[]"#), "{answered}");
    let (code, said, e) = rulec(t.path(), "ja", &["gen", "paid_lines.rule", "--target", "python", "--extra", "vectors", "--out", "ja"], None);
    assert_eq!(code, 0, "{said}{e}");
    let ja = std::fs::read_to_string(t.path().join("ja/vectors/paid_lines.refused.jsonl")).unwrap();
    assert!(ja.lines().any(|l| l.starts_with(r#"{"in":{"lines":[]},"refused":"input","#) && l.ends_with(r#""why":"lines の要素ゼロ件（試した組み合わせのどれにも答えが無い）"}"#)), "{ja}");
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not here") {
        return;
    }
    let out = t.path().join("out").to_string_lossy().into_owned();
    let agrees = || -> bool {
        let tmp = TempDir::new("range-ends-empty-tmp");
        let run = finish(Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp.path()).args(["test", &out, "--format", "json"]), RULEC_LIMIT, "rulec test running the generated code");
        let said = run.stdout.clone();
        let j = rulec::json::parse(said.lines().next().unwrap_or_default()).unwrap_or_else(|x| panic!("{x}: {said}{}", run.stderr));
        let results = j.get("results").and_then(|v| v.as_arr()).unwrap_or_default();
        assert!(results.iter().any(|r| r.get("ran").and_then(|v| v.as_bool()) == Some(true)), "{said}");
        results.iter().filter(|r| r.get("ran").and_then(|v| v.as_bool()) == Some(true)).all(|r| r.get("ok").and_then(|v| v.as_bool()) == Some(true))
    };
    assert!(agrees());
    let file = t.path().join("out/python/paid_lines.py");
    let body = std::fs::read_to_string(&file).unwrap();
    let (written, changed) = ("    if goods < 2:\n", "    if goods < 2 and len(lines) > 0:\n");
    assert_eq!(body.matches(written).count(), 1, "{body}");
    std::fs::write(&file, body.replace(written, changed)).unwrap();
    assert!(!agrees(), "a door that answers an order with no lines passed");
}

/// What `check` says of a range read this way, in English and in Japanese: no value on the step
/// between the ends (E061), a claim held to the value it writes (E112, E140), a table over a count
/// that starts above zero, an example on a walk the rule refuses (E019), a value passed to a rule
/// that is applied (E043), and an open value whose terms come to an end its range leaves out (E131).
#[test]
fn check_reads_the_ends_as_written() {
    let t = TempDir::new("range-ends-check");
    // the rule the `apply` cases apply: an amount above 0 yen and below 100 yen
    std::fs::write(t.path().join("entry_fee.rule"), NARROW).unwrap();
    for lang in ["en", "ja"] {
        let mut text = String::new();
        for (i, (_, body, want)) in cases().iter().enumerate() {
            let name = format!("case{}.rule", i + 1);
            std::fs::write(t.path().join(&name), format!("rule case{} v1\ndescription \"A range read on its step\"\n\n{body}\n", i + 1)).unwrap();
            let (code, out, e) = rulec(t.path(), lang, &["check", &name, "--format", "json"], None);
            let codes: Vec<String> = out.lines().filter_map(|l| rulec::json::parse(l).ok()).filter_map(|j| j.get("code").and_then(|c| c.as_str()).map(String::from)).collect();
            let wanted: Vec<String> = if want.is_empty() { Vec::new() } else { vec![want.to_string()] };
            assert_eq!(codes, wanted, "case {}: {out}{e}", i + 1);
            assert_eq!(code, if want.starts_with('E') { 1 } else { 0 }, "case {}: {out}{e}", i + 1);
            let (_, said, _) = rulec(t.path(), lang, &["check", &name], None);
            text += &format!("$ rulec check {name}\n{said}\n");
        }
        golden(&format!("check.{lang}.txt"), &text);
    }
}

/// A bottom above zero is a precondition: `api` writes the `min` of the sum and a `count` entry,
/// and a rule whose ranges start at zero has neither. `diff` says the range of a sum or a count
/// changed, and with records turns away those under the new bottom.
#[test]
fn api_and_diff_say_the_bottom() {
    let (code, api, e) = rulec(&root(), "en", &["api", "tests/corpus/gift_basket.rule"], None);
    assert_eq!(code, 0, "{e}");
    let api = rulec::json::parse(api.trim()).unwrap();
    let pre: Vec<String> = api.get("preconditions").and_then(|p| p.as_arr()).unwrap().iter().map(|p| p.compact()).collect();
    assert_eq!(
        pre,
        [
            r#"{"kind":"sum","name":"amount","over":"lines","of":"price","max":100000,"min":500}"#,
            r#"{"kind":"count","name":"wrapped_items","over":"lines","min":1}"#,
            r#"{"kind":"length","sequence":"lines","max":5}"#,
        ]
    );
    let (_, api, _) = rulec(&root(), "en", &["api", "tests/corpus/cart_shipping_fee.rule"], None);
    let api = rulec::json::parse(api.trim()).unwrap();
    let pre: Vec<String> = api.get("preconditions").and_then(|p| p.as_arr()).unwrap().iter().map(|p| p.compact()).collect();
    assert_eq!(pre, [r#"{"kind":"sum","name":"total","over":"lines","of":"amount","max":1000000}"#]);

    // the basket before it had a bottom, and the one of the corpus
    let t = TempDir::new("range-ends-diff");
    let now = std::fs::read_to_string(root().join("tests/corpus/gift_basket.rule")).unwrap();
    let before = now.replace("range >=1 <=5", "range >=0 <=5").replace("range >=500JPY <=100_000JPY", "range >=0JPY <=100_000JPY").replace("| 1             | false   |", "| <=1           | false   |");
    assert_ne!(before, now);
    std::fs::write(t.path().join("before.rule"), &before).unwrap();
    std::fs::write(t.path().join("now.rule"), &now).unwrap();
    std::fs::write(
        t.path().join("records.jsonl"),
        "{\"in\":{\"lines\":[]},\"out\":{\"free_shipping\":false,\"card\":false}}\n{\"in\":{\"lines\":[{\"price\":499,\"wrapped\":true}]},\"out\":{\"free_shipping\":false,\"card\":false}}\n{\"in\":{\"lines\":[{\"price\":500,\"wrapped\":true}]},\"out\":{\"free_shipping\":false,\"card\":false}}\n",
    )
    .unwrap();
    let mut text = String::new();
    for lang in ["en", "ja"] {
        // what `diff` says of the two domains, before the cells it compares
        let (_, out, e) = rulec(t.path(), lang, &["diff", "before.rule", "now.rule"], None);
        let head: Vec<&str> = out.lines().take_while(|l| !l.trim().is_empty()).enumerate().filter(|(i, l)| *i == 0 || l.starts_with("  ")).map(|(_, l)| l).collect();
        assert_eq!(head.len(), 3, "{out}{e}");
        text += &format!("$ rulec diff before.rule now.rule\n{}\n\n", head.join("\n"));
        let (code, out, e) = rulec(t.path(), lang, &["diff", "before.rule", "now.rule", "--fixtures", "records.jsonl"], None);
        assert_eq!(code, 1, "{out}{e}");
        text += &format!("$ rulec diff before.rule now.rule --fixtures records.jsonl\n{out}\n");
    }
    assert!(text.contains("a count changed range: wrapped_items  0..5 → 1..5") && text.contains("a sum changed range: amount  0JPY..100_000JPY → 500JPY..100_000JPY"), "{text}");
    assert!(text.contains("the count wrapped_items over lines is under the bottom of the new version's range, 1") && text.contains("the sum amount over lines is under the bottom of the new version's range, 500JPY"), "{text}");
    golden("diff.txt", &text);
}

/// The port says the bottom too: the facts carry it as a precondition; a question over ranges
/// decides the bottom of a total where it says how short the list can be (the shortest list with
/// every element at the bottom of its range is the least), leaves it undecided where it does not,
/// and leaves the bottom of a count undecided unless the list cannot reach it even at its longest;
/// and the evaluator refuses a list under a bottom as an input of the caller's, not as a rule with
/// no answer.
#[test]
fn the_port_holds_a_list_to_the_bottom() {
    let e = rulec::ports::Engine::new();
    let path = root().join("tests/corpus/gift_basket.rule");
    let facts = e.facts(&path).unwrap_or_else(|x| panic!("{x:?}"));
    assert!(facts.preconditions.contains(&Precondition::Sum { name: "amount".into(), over: "lines".into(), of: "price".into(), max: 100000, min: Some(500), unit: ritsu_units::Unit::money("JPY", None) }), "{:?}", facts.preconditions);
    assert!(facts.preconditions.contains(&Precondition::Count { name: "wrapped_items".into(), over: "lines".into(), min: 1 }), "{:?}", facts.preconditions);
    // two lines of at most 50,000 yen keep the top; the bottom is not decided where the question
    // does not say how short the list can be
    let held = e.preconditions_hold(&path, &[], None, Some(2)).unwrap_or_else(|x| panic!("{x:?}"));
    for (p, a) in &held {
        match p {
            Precondition::Sum { .. } | Precondition::Count { .. } => assert!(matches!(a, Answer::Undecided(_)), "{p:?}: {a:?}"),
            Precondition::Length { .. } => assert!(matches!(a, Answer::Holds), "{p:?}: {a:?}"),
            other => panic!("{other:?}"),
        }
    }
    // one line or two of 500 yen to 50,000 yen: the least is one line of 500 yen, on the bottom.
    // Which lines are wrapped is nothing a range says, so the count stays open
    let price = |lo: i128| vec![("price".to_string(), Some(lo), Some(50_000))];
    let sum_of = |ranges: &[(String, Option<i128>, Option<i128>)], least: i128| {
        let all = e.preconditions_hold(&path, ranges, Some(least), Some(2)).unwrap_or_else(|x| panic!("{x:?}"));
        assert!(all.iter().any(|(p, a)| matches!(p, Precondition::Count { .. }) && matches!(a, Answer::Undecided(_))), "{all:?}");
        all.into_iter().find(|(p, _)| matches!(p, Precondition::Sum { .. })).map(|(_, a)| a).expect("the sum is a precondition")
    };
    assert_eq!(sum_of(&price(500), 1), Answer::Holds);
    // one yen less a line, and that one line is the example
    assert_eq!(sum_of(&price(499), 1), Answer::Fails(vec![("lines".to_string(), Value::List(vec![vec![("price".to_string(), Value::Int(499))]]))]));
    // a list that may be empty comes to 0 yen
    assert_eq!(sum_of(&price(500), 0), Answer::Fails(vec![("lines".to_string(), Value::List(Vec::new()))]));
    // two lines of 250 yen at the least reach the bottom
    assert_eq!(sum_of(&price(250), 2), Answer::Holds);
    // Two lines of at most 250 yen reach the bottom at their largest, so no list is the example,
    // and without the fewest lines it is not decided; a yen less a line and no list comes to the
    // bottom, the longest at its largest being the example
    let up_to = |hi: i128| {
        let all = e.preconditions_hold(&path, &[("price".to_string(), Some(0), Some(hi))], None, Some(2)).unwrap_or_else(|x| panic!("{x:?}"));
        all.into_iter().find(|(p, _)| matches!(p, Precondition::Sum { .. })).map(|(_, a)| a).expect("the sum is a precondition")
    };
    assert!(matches!(up_to(250), Answer::Undecided(_)), "{:?}", up_to(250));
    assert_eq!(up_to(249), Answer::Fails(vec![("lines".to_string(), Value::List(vec![vec![("price".to_string(), Value::Int(249))]; 2]))]));
    // A list of one line at the most can hold the one wrapped item the count starts at: it is
    // the list shorter than the bottom that breaks it
    let one_line = e.preconditions_hold(&path, &[], None, Some(1)).unwrap_or_else(|x| panic!("{x:?}"));
    assert!(one_line.iter().any(|(p, a)| matches!(p, Precondition::Count { .. }) && matches!(a, Answer::Undecided(_))), "{one_line:?}");
    // a list that is always empty never reaches either bottom, and the empty list is the example
    for (p, a) in e.preconditions_hold(&path, &[], None, Some(0)).unwrap_or_else(|x| panic!("{x:?}")) {
        match p {
            Precondition::Sum { .. } | Precondition::Count { .. } => assert_eq!(a, Answer::Fails(vec![("lines".to_string(), Value::List(Vec::new()))]), "{p:?}"),
            Precondition::Length { .. } => assert_eq!(a, Answer::Holds),
            other => panic!("{other:?}"),
        }
    }
    let line = |price: i128, wrapped: bool| vec![("price".to_string(), Value::Int(price)), ("wrapped".to_string(), Value::Bool(wrapped))];
    let basket = |lines: Vec<Vec<(String, Value)>>| vec![("lines".to_string(), Value::List(lines))];
    let got = e.eval(&path, &basket(vec![line(500, true)])).unwrap_or_else(|x| panic!("{x:?}"));
    assert!(got.contains(&("free_shipping".to_string(), Value::Bool(false))) && got.contains(&("card".to_string(), Value::Bool(false))), "{got:?}");
    for (lines, name) in [
        (vec![], "wrapped_items"),
        (vec![line(499, true)], "amount"),
        (vec![line(50000, true), line(50000, true), line(1, true)], "amount"),
    ] {
        match e.eval(&path, &basket(lines)) {
            Err(RuleError::Input(t)) => {
                assert_eq!(t.en, format!("the `{name}` worked out over a list is outside its range"));
                assert_eq!(t.ja, format!("並びから求めた `{name}` が、範囲の外です"));
            }
            other => panic!("{name}: {other:?}"),
        }
    }
    match e.eval(&path, &basket(vec![line(100, true); 6])) {
        Err(RuleError::Input(t)) => assert_eq!(t.en, "the list `lines` has too many elements (at most 5)"),
        other => panic!("{other:?}"),
    }
}

/// What a basket is charged is worked out once the walk is over, from a total and a count, and the
/// shop takes it within a range: the ends of that range are ends a door holds too.
const BASKET_CHARGE: &str = "rule basket_charge v1
description \"What a basket is charged: the prices of its lines, and 500 yen for each line wrapped as a gift. The shop takes a charge from 1,000 yen to 80,000 yen\"

elements lines
  price : money[JPY]  range >=0JPY <=50_000JPY
  gift  : bool

sum goods over lines of price  range >=0JPY <=100_000JPY
count gifts over lines where gift  range >=0 <=5

define charged : money[JPY] = goods + gifts * 500JPY  range >=1_000JPY <=80_000JPY

outputs
  free_delivery : bool

table delivery
policy unique
| charged     | -> free_delivery |
| <5_000JPY   | false            |
| >=5_000JPY  | true             |
";

/// The same in Japanese.
const BASKET_CHARGE_JA: &str = "rule かごの請求額(basket_charge_ja) v1
description \"かごの請求額。明細の価格の合計に、贈り物として包む明細一つにつき 500 円を足す。店が受け付ける請求額は 1,000 円から 80,000 円まで\"

elements 明細(lines)
  価格(price)  : money[円]  range >=0円 <=50000円
  贈り物(gift) : bool

sum 商品の合計(goods) over 明細 of 価格  range >=0円 <=100000円
count 包む数(gifts) over 明細 where 贈り物  range >=0 <=5

define 請求額(charged) : money[円] = 商品の合計 + 包む数 * 500円  range >=1000円 <=80000円

outputs
  送料無料(free_delivery) : bool

table 送料(delivery)
policy unique
| 請求額   | -> 送料無料 |
| <5000円  | false       |
| >=5000円 | true        |
";

/// A value worked out once a walk is over — from what a `sum` and a `count` come to — has a case
/// on each end of the range the rule accepts for it, and one a step past each: a sequence that
/// brings the total where the value needs it. Without them a generated door that holds the value
/// to a range one yen off passed `rulec test` in three of the four ways (a yen under the bottom, a
/// yen either way at the top); and the row of the table under the value's bottom side had no case
/// when the bottom refused what the pool gave it. The boundary the table writes on the value has
/// its pair as well — one yen under it and on it, the two apart in the sequence alone — so a row
/// generated one yen off is stopped too.
#[test]
fn a_value_worked_out_once_a_walk_is_over_has_a_case_on_each_end_of_its_range() {
    let t = TempDir::new("range-ends-walked");
    std::fs::write(t.path().join("basket_charge.rule"), BASKET_CHARGE).unwrap();
    std::fs::write(t.path().join("かごの請求額.rule"), BASKET_CHARGE_JA).unwrap();
    let (code, said, e) = rulec(t.path(), "en", &["gen", "basket_charge.rule", "--target", "python", "--extra", "runner", "--extra", "vectors", "--out", "out"], None);
    assert_eq!(code, 0, "{said}{e}");
    // what a case is charged, from its lines
    let charged = |j: &rulec::json::Json| -> i128 {
        let lines = j.get("in").and_then(|i| i.get("lines")).and_then(|l| l.as_arr()).unwrap_or_default();
        lines.iter().map(|l| l.get("price").and_then(|p| p.as_int()).unwrap_or(0) + if l.get("gift").and_then(|g| g.as_bool()) == Some(true) { 500 } else { 0 }).sum()
    };
    let read = |file: &str| -> Vec<rulec::json::Json> {
        std::fs::read_to_string(t.path().join(file)).unwrap().lines().map(|l| rulec::json::parse(l).unwrap_or_else(|x| panic!("{x}: {l}"))).collect()
    };
    let why = |j: &rulec::json::Json| j.get("why").and_then(|w| w.as_str()).unwrap_or_default().to_string();
    let answered = read("out/vectors/basket_charge.jsonl");
    for (said, at) in [("the definition charged at the bottom of the range it accepts", 1000), ("the definition charged at the top of the range it accepts", 80000)] {
        let on: Vec<i128> = answered.iter().filter(|j| why(j) == said).map(charged).collect();
        assert_eq!(on, [at], "{said}: {answered:?}");
    }
    let refused = read("out/vectors/basket_charge.refused.jsonl");
    for (said, at) in [("the definition charged past the bottom of the range it accepts", 999), ("the definition charged past the top of the range it accepts", 80001)] {
        let past: Vec<(i128, &str, &str)> = refused
            .iter()
            .filter(|j| why(j) == said)
            .map(|j| (charged(j), j.get("refused").and_then(|x| x.as_str()).unwrap_or_default(), j.get("error").and_then(|x| x.as_str()).unwrap_or_default()))
            .collect();
        assert_eq!(past, [(at, "input", "charged is out of range")], "{said}: {refused:?}");
    }
    // both rows of the table have a case and the boundary between them its pair, which the audit
    // says of the suite
    let (code, said, e) = rulec(t.path(), "en", &["coverage", "basket_charge.rule"], None);
    assert_eq!(code, 0, "{said}{e}");
    for line in [["row", "coverage", "2", "/", "2", "satisfied"], ["boundary-pair", "coverage", "2", "/", "2", "satisfied"]] {
        assert!(said.lines().any(|l| l.split_whitespace().collect::<Vec<_>>() == line), "{line:?}: {said}{e}");
    }
    for at in [4999, 5000] {
        assert!(answered.iter().any(|j| charged(j) == at), "no case charged {at}: {answered:?}");
    }
    // the Japanese version says the reasons in Japanese
    let (code, said, e) = rulec(t.path(), "ja", &["gen", "かごの請求額.rule", "--target", "python", "--extra", "vectors", "--out", "ja"], None);
    assert_eq!(code, 0, "{said}{e}");
    let ja: Vec<String> = read("ja/vectors/basket_charge_ja.jsonl").iter().chain(read("ja/vectors/basket_charge_ja.refused.jsonl").iter()).map(why).collect();
    for said in ["定義 請求額 を、受け付ける範囲の下端に", "定義 請求額 を、受け付ける範囲の上端に", "定義 請求額 を、受け付ける範囲の下端の先に", "定義 請求額 を、受け付ける範囲の上端の先に"] {
        assert_eq!(ja.iter().filter(|w| *w == said).count(), 1, "{said}: {ja:?}");
    }
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not here") {
        return;
    }
    let out = t.path().join("out").to_string_lossy().into_owned();
    let agrees = || -> bool {
        let tmp = TempDir::new("range-ends-walked-tmp");
        let run = finish(Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp.path()).args(["test", &out, "--format", "json"]), RULEC_LIMIT, "rulec test running the generated code");
        let said = run.stdout.clone();
        let j = rulec::json::parse(said.lines().next().unwrap_or_default()).unwrap_or_else(|x| panic!("{x}: {said}{}", run.stderr));
        let results = j.get("results").and_then(|v| v.as_arr()).unwrap_or_default();
        assert!(results.iter().any(|r| r.get("ran").and_then(|v| v.as_bool()) == Some(true)), "{said}");
        results.iter().filter(|r| r.get("ran").and_then(|v| v.as_bool()) == Some(true)).all(|r| r.get("ok").and_then(|v| v.as_bool()) == Some(true))
    };
    assert!(agrees());
    let file = t.path().join("out/python/basket_charge.py");
    let body = std::fs::read_to_string(&file).unwrap();
    let written = "    if not 1000 <= charged <= 80000:\n";
    assert_eq!(body.matches(written).count(), 1, "{body}");
    for changed in [
        "    if not 1001 <= charged <= 80000:\n",
        "    if not 999 <= charged <= 80000:\n",
        "    if not 1000 <= charged <= 79999:\n",
        "    if not 1000 <= charged <= 80001:\n",
    ] {
        std::fs::write(&file, body.replace(written, changed)).unwrap();
        assert!(!agrees(), "a door written `{}` passed", changed.trim());
    }
    // the boundary between the rows of the table, one yen off either way
    let (first, second) = ("    if charged < 5000:", "    elif charged >= 5000:");
    assert_eq!((body.matches(first).count(), body.matches(second).count()), (1, 1), "{body}");
    for at in ["5001", "4999"] {
        let changed = body.replace(first, &first.replace("5000", at)).replace(second, &second.replace("5000", at));
        std::fs::write(&file, changed).unwrap();
        assert!(!agrees(), "the rows written with the boundary at {at} passed");
    }
    std::fs::write(&file, &body).unwrap();
    assert!(agrees());
}

/// Whether an order is insured goes by its dearest line, and the carrier takes a cover in a range
/// the cases of the pool all fall outside: an order with no lines and one of a line of nothing are
/// under it, and a line at the top of what a line can cost is over it.
const DEAREST_LINE: &str = "rule dearest_line v1
description \"Whether an order is insured: by its dearest line and a handling charge of 500 yen. The carrier takes a cover from 600 yen to 30,500 yen\"

elements lines
  price : money[JPY]  range >=0JPY <=50_000JPY

max dearest over lines of price empty 0JPY

define cover : money[JPY] = dearest + 500JPY  range >=600JPY <=30_500JPY

outputs
  insured : bool

table insure
policy unique
| cover       | -> insured |
| <10_000JPY  | false      |
| >=10_000JPY | true       |
";

/// A count of wrapped lines times a count of boxed ones is never 7: each is 5 at most, and 7 is
/// the product of no two such numbers. `check` reads the range as one the value comes into (0 to
/// 25 holds 7), and what a product of two counts takes is not read off a step (§15.323), so the
/// rule passes, and no call is answered. (A count doubled and one more, held to 2, is E150.)
const NEVER_SEVEN: &str = "rule never_seven v1

elements lines
  gift  : bool
  boxed : bool

count gifts over lines where gift  range >=0 <=5
count boxes over lines where boxed  range >=0 <=5

define packs : number = gifts * boxes  range >=7 <=7

outputs
  many : bool

table many_of
policy unique
| packs | -> many |
| -     | true    |
";

/// The points of an order are worked out from two counts over the same lines.
const WRAP_POINTS: &str = "rule wrap_points v1
description \"The points of an order: 10 for each line wrapped as a gift and 1 for each line in a box. An order earns from 12 points to 40\"

elements lines
  gift : bool
  box  : bool

count gifts over lines where gift  range >=0 <=5
count boxes over lines where box  range >=0 <=5

define points : number = gifts * 10 + boxes  range >=12 <=40

outputs
  bonus : bool

table bonus_of
policy unique
| points | -> bonus |
| <25    | false    |
| >=25   | true     |
";

/// A value worked out from two counts over one sequence: a case that moves one count leaves the
/// other where it was, by putting lines after the ones there are — two wrapped lines in boxes and
/// two more lines in boxes for 24 points, and one more for 25.
#[test]
fn a_value_worked_out_from_two_counts_keeps_the_one_it_does_not_move() {
    let t = TempDir::new("range-ends-two-counts");
    std::fs::write(t.path().join("wrap_points.rule"), WRAP_POINTS).unwrap();
    let (code, said, e) = rulec(t.path(), "en", &["gen", "wrap_points.rule", "--target", "python", "--extra", "runner", "--extra", "vectors", "--out", "out"], None);
    assert_eq!(code, 0, "{said}{e}");
    let points = |j: &rulec::json::Json| -> i128 {
        let lines = j.get("in").and_then(|i| i.get("lines")).and_then(|l| l.as_arr()).unwrap_or_default();
        let on = |l: &rulec::json::Json, k: &str| l.get(k).and_then(|g| g.as_bool()) == Some(true);
        lines.iter().map(|l| if on(l, "gift") { 10 } else { 0 } + if on(l, "box") { 1 } else { 0 }).sum()
    };
    let read = |file: &str| -> Vec<rulec::json::Json> {
        std::fs::read_to_string(t.path().join(file)).unwrap().lines().map(|l| rulec::json::parse(l).unwrap_or_else(|x| panic!("{x}: {l}"))).collect()
    };
    let answered = read("out/vectors/wrap_points.jsonl");
    for at in [12, 24, 25, 40] {
        assert!(answered.iter().any(|j| points(j) == at), "no case of {at} points: {answered:?}");
    }
    let refused = read("out/vectors/wrap_points.refused.jsonl");
    for (said, at) in [("the definition points past the bottom of the range it accepts", 11), ("the definition points past the top of the range it accepts", 41)] {
        let past: Vec<(i128, &str)> = refused.iter().filter(|j| j.get("why").and_then(|w| w.as_str()) == Some(said)).map(|j| (points(j), j.get("error").and_then(|x| x.as_str()).unwrap_or_default())).collect();
        assert_eq!(past, [(at, "points is out of range")], "{said}: {refused:?}");
    }
    let (code, said, e) = rulec(t.path(), "en", &["coverage", "wrap_points.rule"], None);
    assert_eq!(code, 0, "{said}{e}");
    assert!(said.lines().any(|l| l.split_whitespace().collect::<Vec<_>>() == ["boundary-pair", "coverage", "2", "/", "2", "satisfied"]), "{said}{e}");
}

/// A range that leaves out every case the pool has still gets its cases: the ends of the value are
/// solved from the cases it refuses. The suite of `dearest_line` used to be empty — a vectors file
/// of one empty line and no refused cases — and `rulec test` failed the generated code that was
/// right, for printing no line where the file had one. A rule no call is answered for at all has a
/// vectors file with no line, the case the value refuses as a refused case, and `rulec test` says
/// 0 vectors of it.
#[test]
fn a_range_that_leaves_out_every_case_of_the_pool_still_has_its_cases() {
    let t = TempDir::new("range-ends-outside");
    std::fs::write(t.path().join("dearest_line.rule"), DEAREST_LINE).unwrap();
    std::fs::write(t.path().join("never_seven.rule"), NEVER_SEVEN).unwrap();
    let (code, said, e) = rulec(t.path(), "en", &["gen", "dearest_line.rule", "--target", "python", "--extra", "runner", "--extra", "vectors", "--out", "out"], None);
    assert_eq!(code, 0, "{said}{e}");
    let read = |file: &str| std::fs::read_to_string(t.path().join(file)).unwrap();
    let answered = read("out/vectors/dearest_line.jsonl");
    for line in [
        r#"{"in":{"lines":[{"price":100}]},"out":{"insured":false},"trace":["table insure row 1"],"why":"the definition cover at the bottom of the range it accepts"}"#,
        r#"{"in":{"lines":[{"price":30000}]},"out":{"insured":true},"trace":["table insure row 2"],"why":"the definition cover at the top of the range it accepts"}"#,
        r#"{"in":{"lines":[{"price":9499}]},"out":{"insured":false},"trace":["table insure row 1"],"why":"cover brought to 9999JPY"}"#,
        r#"{"in":{"lines":[{"price":9500}]},"out":{"insured":true},"trace":["table insure row 2"],"why":"cover brought to 10_000JPY"}"#,
    ] {
        assert!(answered.lines().any(|l| l == line), "{line}:\n{answered}");
    }
    let refused = read("out/vectors/dearest_line.refused.jsonl");
    for line in [
        r#"{"in":{"lines":[{"price":99}]},"refused":"input","error":"cover is out of range","why":"the definition cover past the bottom of the range it accepts"}"#,
        r#"{"in":{"lines":[{"price":30001}]},"refused":"input","error":"cover is out of range","why":"the definition cover past the top of the range it accepts"}"#,
        r#"{"in":{"lines":[]},"refused":"input","error":"cover is out of range","why":"no elements of lines, where no case tried is answered"}"#,
    ] {
        assert!(refused.lines().any(|l| l == line), "{line}:\n{refused}");
    }
    let (code, said, e) = rulec(t.path(), "en", &["coverage", "dearest_line.rule"], None);
    assert_eq!(code, 0, "{said}{e}");

    // no call is answered: no line of vectors, and the case the value refuses
    let (code, said, e) = rulec(t.path(), "en", &["gen", "never_seven.rule", "--target", "python", "--extra", "runner", "--extra", "vectors", "--out", "none"], None);
    assert_eq!(code, 0, "{said}{e}");
    assert_eq!((read("none/vectors/never_seven.jsonl"), read("none/vectors/never_seven.expected.jsonl")), (String::new(), String::new()));
    assert_eq!(
        read("none/vectors/never_seven.refused.jsonl"),
        "{\"in\":{\"lines\":[]},\"refused\":\"input\",\"error\":\"packs is out of range\",\"why\":\"an input the definition packs refuses, where no case found is answered\"}\n"
    );
    let (code, said, e) = rulec(t.path(), "ja", &["gen", "never_seven.rule", "--target", "python", "--extra", "vectors", "--out", "ja"], None);
    assert_eq!(code, 0, "{said}{e}");
    assert_eq!(
        read("ja/vectors/never_seven.refused.jsonl"),
        "{\"in\":{\"lines\":[]},\"refused\":\"input\",\"error\":\"packs が範囲の外です\",\"why\":\"定義 packs が受け付けない入力（答えのある入力が見つからない）\"}\n"
    );
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not here") {
        return;
    }
    let ran = |out: &str, lang: &str, format: &[&str]| -> (i32, String) {
        let tmp = TempDir::new("range-ends-outside-tmp");
        let dir = t.path().join(out).to_string_lossy().into_owned();
        let run = finish(Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", lang).env("TMPDIR", tmp.path()).arg("test").arg(&dir).args(format), RULEC_LIMIT, "rulec test running the generated code");
        (run.code.unwrap_or(-1), run.stdout.clone() + &run.stderr)
    };
    let (code, said) = ran("out", "en", &[]);
    assert!(code == 0 && said.contains("ok    dearest_line (Python) 4 vectors, 14 refused"), "{code} {said}");
    let (code, said) = ran("none", "en", &[]);
    assert!(code == 0 && said.contains("ok    never_seven (Python) 0 vectors, 1 refused"), "{code} {said}");
    let (code, said) = ran("none", "ja", &[]);
    assert!(code == 0 && said.contains("ok    never_seven (Python) ベクタ 0 件、受け付けない入力 1 件"), "{code} {said}");
    // a door one yen off at either end of the cover, and a door that lets the empty order through
    for (out, file, written, changed) in [
        ("out", "out/python/dearest_line.py", "    if not 600 <= cover <= 30500:\n", "    if not 599 <= cover <= 30500:\n"),
        ("out", "out/python/dearest_line.py", "    if not 600 <= cover <= 30500:\n", "    if not 601 <= cover <= 30500:\n"),
        ("out", "out/python/dearest_line.py", "    if not 600 <= cover <= 30500:\n", "    if not 600 <= cover <= 30501:\n"),
        ("out", "out/python/dearest_line.py", "    if not 600 <= cover <= 30500:\n", "    if not 600 <= cover <= 30499:\n"),
        ("none", "none/python/never_seven.py", "    if not 7 <= packs <= 7:\n", "    if not 0 <= packs <= 7:\n"),
    ] {
        let path = t.path().join(file);
        let body = std::fs::read_to_string(&path).unwrap();
        assert_eq!(body.matches(written).count(), 1, "{body}");
        std::fs::write(&path, body.replace(written, changed)).unwrap();
        let (code, said) = ran(out, "en", &[]);
        assert!(code != 0 && said.contains("FAIL"), "a door written `{}` passed: {said}", changed.trim());
        std::fs::write(&path, &body).unwrap();
    }
}
