//! A table that reads a value the rule works out beside the inputs it is worked out from (§15.322).
//!
//! A table may read an output of a rule it applies, a `define`, or a value a table of the same rule
//! writes, together with the inputs that decide it. The check reads such a value as the rule works
//! it out: one a table writes, by the rows of that table, followed through what `apply` and `with`
//! pass on to a table that reads the inputs; one an expression works out, by what the expression
//! comes to with the inputs it reads held to each cell. So the completeness check (E101) asks for
//! rows only where some input reaches, and the unreachable rows (E102), the overlaps (E105), W128,
//! the examples, the hints, the certificate and the port's `outputs_over` read the value the same
//! way.
//!
//! The materials are in `tests/computed`, English first, with a Japanese version beside each.
//! `purchase_route.rule` applies the company's table of authority (`shared/authority.rule`) and
//! reads whether finance reviews the request beside the kind and the amount that decide it; it
//! passes, and without its row for goods up to a million yen with no review it is E101, with that
//! combination as the example. `E101_fed_by_another_rule.rule` takes the same value as an input
//! `fed by` that rule. `fed by` does not say what the rule was given, so the value is read free of
//! the other inputs, and the E101 says so and points to `apply`; a gap that turns on two such
//! inputs names both in one note (a test writes that rule out itself).
//! `E102_a_fee_no_amount_gives.rule` reads a fee of 3%, rounded down, beside the amount: its first
//! row asks for a fee over 900 yen on an amount up to 30,000 yen, which no amount gives, and is
//! E102. Before, the first two stopped at E101 and the third passed with W128.
//!
//! `tests/computed/shapes` holds a rule for each way such a value comes, with the rules they apply
//! in `shapes/shared`: an enum output, the same read through `with`, a number a table writes, an
//! optional output, a rule that applies a rule, a value passed that the rule works out, a `define`
//! of a truth value and one of a number, a number a table of the same rule writes, a rule applied
//! with a `constraint` and one with a `define … range`, two rows that meet only where no input
//! takes both, a table that reads the band alone, and two tables that cut the amount where the
//! table above does not (a band cut at 50 yen read with the amount cut at 80 yen as well, and an
//! applied fee read beside the amount cut elsewhere). The rest that pass — a value on the grid a
//! rounding puts it on, the rows of a `policy first` table above, a way that goes one table further
//! up, whole yen between two cells, a box ruled out amount by amount, a date a table above writes,
//! a date or a fee that may be absent — are told where they are listed (`PASSING`). And six do not
//! pass: an output an
//! expression works out (E102), a gap whose first amount gives a fee outside it (E101), two rows
//! that meet where the band is large (E105), the band table without the row from 51 yen to 80 yen
//! (E101), a fee rounded to 10 yen, whose example comes from the part of the gap an input falls in
//! (E101), and a row that meets another only from 17 yen, where the example is (E105).
//!
//! What `rulec check` prints for each rule that does not pass, in English and in Japanese, is its
//! golden file in `tests/golden/computed` (`RULEC_BLESS=1 cargo test --test computed_columns`
//! writes them again). Each example is run through `rulec run` of the rule that works the value
//! out; each certificate passes both re-checkers, and a forgery of each new kind of fact is
//! refused by both. The point the certificate gives for a row reached through a way one table
//! further up is run too, and takes that row.

use ritsu_base::json::Json;
use ritsu_ports::{Found, Rules, Value};
use ritsu_testkit::{Need, ready};
use rulec::i18n::{self, Lang};
use rulec::ports::Engine;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The rules that pass, each with its Japanese version, by their paths under `tests/computed`.
/// Each up to `tier_of_small_orders` but the rules applied and `band_alone` stopped at E101 before,
/// `overlap_none_occurs` at E105 as well. The five from `fee_in_steps` are read by the grid a
/// rounding puts a value on, by pieces of the amount a define reads twice, by the amounts the rows
/// of a `policy first` table above write a word and a number from, and by the truth value of a
/// `define` a table above reads, whose certificate states where the table writes each tier on it.
///
/// The ten after them (§15.325) are read by the rows of a `policy first` table above that take an
/// amount before a later row can (one row, two rows over a column the table below does not read, a
/// number, two rows on either side of a cut, and two rows of the table below that meet only there),
/// by a way that goes one table further up, through the table that decides a column both tables
/// read, by a row above that takes nothing of a box once the amounts are whole yen, by a box ruled
/// out amount by amount on facts the certificate states, and by the point that reaches a row
/// through such a way. Of those, three stopped at E101 before; five passed with a certificate both
/// re-checkers refused; `first_band_rows_apart` passed with the two rows it parts stated in its
/// certificate rather than proved; and the last passed with a point for its first row that no
/// input reaches.
///
/// The five after them are read by the day a date a table above writes stands for (§15.325): a rule
/// applied for the day an order ships, a table that reads that day alone and one that reads it with
/// the zone it is decided from, a date of the same rule's table that is absent for an order picked up
/// at the shop, and a fee absent the same way. The three that read a date stopped at E101 before,
/// asking for a row for the day between the two the table above writes; the one that reads the fee
/// passed with a certificate both re-checkers refused, as an axis of numbers with the absent value on
/// it.
///
/// The two after them name each day in a cell of its own: the day the applied rule writes, and the
/// date of the same rule's table under `policy first`, after a row for the absent value. The first
/// stopped at E101 before, as `date_output` did; the second passed with a certificate both
/// re-checkers refused, as `same_rule_optional_number` did. Both hold the check to taking the day a
/// row above writes as the date it is where a cell names one day: taken as a plain number, it met
/// no such cell, and their rows were called unreachable (E102).
///
/// The twelve after them (§15.325) all stopped at E101 before. They are read by what a `define`
/// comes to with the amount it reads held, as the amount less itself where a minimum tops it up; by
/// the whole number of 37-yen steps a fee sits on; by a `define` worked out again on the amounts a
/// row of a table that reads it takes, as a number, as the truth of a comparison, with an input the
/// row cuts held to its span, and over pieces of the amount where one span is not enough (twice);
/// by pieces of an input a `define` is cut into on a table that reads only the `define`; by two rows
/// on a way through the tables above that take no value of a column together; by a number a row
/// above writes that no fee of the box stands on; by the span a `define`'s `range` accepts; and by
/// inputs that are whole numbers.
const PASSING: [(&str, &str); 58] = [
    ("shared/authority", "shared/決裁権限"),
    ("purchase_route", "購買の経路"),
    ("shapes/shared/tier", "shapes/shared/金額の区分"),
    ("shapes/shared/band_only", "shapes/shared/区分だけ"),
    ("shapes/shared/twolevel", "shapes/shared/二段"),
    ("shapes/shared/calc", "shapes/shared/手数料の計算"),
    ("shapes/shared/span", "shapes/shared/幅"),
    ("shapes/shared/fee_band", "shapes/shared/手数料と区分"),
    ("shapes/shared/flat_fee", "shapes/shared/一律の手数料"),
    ("shapes/enum_output", "shapes/列挙の出力"),
    ("shapes/enum_output_with", "shapes/with_で読み替えた出力"),
    ("shapes/number_output", "shapes/数の出力"),
    ("shapes/optional_output", "shapes/値の無いことがある出力"),
    ("shapes/two_levels", "shapes/二段の取り込み"),
    ("shapes/computed_binding", "shapes/計算した値を渡す"),
    ("shapes/bool_define", "shapes/真偽の定義"),
    ("shapes/number_define", "shapes/数の定義"),
    ("shapes/same_rule_number", "shapes/同じ規則の表の数"),
    ("shapes/callee_constraint", "shapes/制約のある規則"),
    ("shapes/callee_define_range", "shapes/範囲のある定義の規則"),
    ("shapes/overlap_none_occurs", "shapes/重ならない行"),
    ("shapes/band_alone", "shapes/区分だけを読む表"),
    ("shapes/bands_cut_elsewhere", "shapes/別の所で切った区分"),
    ("shapes/fee_cut_elsewhere", "shapes/別の所で切った手数料"),
    ("shapes/fee_in_steps", "shapes/十円刻みの手数料だけを読む表"),
    ("shapes/amount_read_twice", "shapes/金額を二度読む不足額"),
    ("shapes/first_band_above", "shapes/先に当たる行で決まる区分"),
    ("shapes/first_fee_above", "shapes/先に当たる行で決まる手数料"),
    ("shapes/tier_of_small_orders", "shapes/小口の注文で決まる段階"),
    ("shapes/first_band_by_kind", "shapes/種類と金額で先に決まる区分"),
    ("shapes/first_band_rows_together", "shapes/二つの行が先にとる区分"),
    ("shapes/first_fee_taken", "shapes/前の行がとる手数料"),
    ("shapes/first_band_two_rows_one_cut", "shapes/一つの区切りを二つの行がとる区分"),
    ("shapes/first_band_rows_apart", "shapes/先にとる行で離れる二つの行"),
    ("shapes/tier_through_band", "shapes/区分と金額で決まる段階"),
    ("shapes/band_cut_between_yen", "shapes/円の間で切った区分"),
    ("shapes/fee_and_tier_in_whole_yen", "shapes/円で数える手数料と段階"),
    ("shapes/tier_and_fee_point_by_point", "shapes/点ごとに外す段階と手数料"),
    ("shapes/tier_witness_through", "shapes/手数料で決まる段階に届く点"),
    ("shapes/shared/ship_day", "shapes/shared/出荷日"),
    ("shapes/date_output", "shapes/日付の出力"),
    ("shapes/date_with_zone", "shapes/地域と一緒に読む日付"),
    ("shapes/same_rule_date", "shapes/同じ規則の表の日付"),
    ("shapes/same_rule_optional_number", "shapes/値の無いことがある手数料"),
    ("shapes/date_on_its_day", "shapes/一日ずつ読む日付"),
    ("shapes/same_rule_date_on_its_day", "shapes/同じ規則の表の日付を一日ずつ読む"),
    ("shapes/top_up_to_minimum", "shapes/最低額への上乗せ"),
    ("shapes/boxes_at_37_yen", "shapes/一箱37円の手数料"),
    ("shapes/tier_from_the_fee", "shapes/手数料で決まる区分"),
    ("shapes/tier_from_a_threshold", "shapes/金額の境で決まる区分"),
    ("shapes/packing_on_small_orders", "shapes/小さな注文の梱包料"),
    ("shapes/net_by_amount_band", "shapes/金額の帯と差で決まる段階"),
    ("shapes/top_up_tier_by_items", "shapes/品数で決まる上乗せの段階"),
    ("shapes/net_of_two_roundings", "shapes/二つの丸めの差"),
    ("shapes/shipping_tier_by_region", "shapes/地域ごとの送料の段階"),
    ("shapes/fee_written_above", "shapes/上の表が書く手数料"),
    ("shapes/fee_the_rule_accepts", "shapes/規則が受け付ける手数料"),
    ("shapes/items_over_the_limit", "shapes/上限を超える品数"),
];

/// The rules that do not pass, each with its Japanese version: the one code each stops at, and
/// what the English report says.
const FAILING: [(&str, &str, &str, &[&str]); 16] = [
    (
        "E101_fed_by_another_rule",
        "E101_ほかの規則が入れる値",
        "E101",
        &[
            "An input that matches no row: kind = goods, amount = 1JPY, authority_finance = true",
            "`authority_finance` is an input another rule's output feeds (`fed by`)",
            "bring the rule in with `apply`",
        ],
    ),
    (
        "E102_a_fee_no_amount_gives",
        "E102_どの金額も出さない手数料",
        "E102",
        &["row 1: no input reaches here", ">=0JPY <=900JPY over the values of `amount` in this row"],
    ),
    (
        "shapes/E102_an_output_an_expression_works_out",
        "shapes/E102_式で決まる出力",
        "E102",
        &["row 1: no input reaches here", "`c_fee` is computed from `amount`"],
    ),
    (
        "shapes/E101_a_gap_the_first_amount_misses",
        "shapes/E101_最初の金額では当たらない穴",
        "E101",
        &["An input that matches no row: amount = 33_367JPY, f_fee = 1001JPY", "`| >30_000JPY | >1_000JPY | ? |`"],
    ),
    (
        "shapes/E105_rows_that_meet_on_the_large_band",
        "shapes/E105_大の区分で重なる行",
        "E105",
        &["Both rows match: amount = 100_001JPY, t_band = large"],
    ),
    (
        "shapes/E101_a_band_row_left_out",
        "shapes/E101_区分の行を抜いた表",
        "E101",
        &["No row matches: amount from 51JPY to 80JPY, band = small", "An input that matches no row: amount = 51JPY, band = small", "`| >50JPY <=80JPY | small | ? |`"],
    ),
    (
        "shapes/E101_a_fee_in_steps_of_ten",
        "shapes/E101_十円刻みの手数料",
        "E101",
        &["amount from 227JPY to 229JPY, fee 12JPY or more", "An input that matches no row: amount = 227JPY, fee = 100JPY"],
    ),
    (
        "shapes/E105_rows_that_meet_from_17_yen",
        "shapes/E105_17円から重なる行",
        "E105",
        &["Both rows match: amount = 17JPY, big = true"],
    ),
    (
        "shapes/E102_a_row_the_table_above_leaves_out",
        "shapes/E102_上の表が外す行",
        "E102",
        &["row 3: no input reaches here", "No input reaches this row: the tables above leave out every input it asks for that the earlier rows do not take first."],
    ),
    // The grid a rounding puts a value on leaves the row out: on its own, where the table reads
    // nothing the define reads (a note that once had no name to give, and stopped the check), and
    // over what the row takes of the amount.
    (
        "shapes/E102_a_fee_between_two_steps",
        "shapes/E102_刻みの間の手数料",
        "E102",
        &["row 2: no input reaches here", "`fee` only comes to multiples of 10JPY, and this row asks for none of them."],
    ),
    (
        "shapes/E102_a_parcel_fee_between_two_steps",
        "shapes/E102_刻みの間の小包の手数料",
        "E102",
        &["row 1: no input reaches here", "`parcel_fee` only comes to 5JPY plus multiples of 10JPY within >=15JPY <=35JPY, none of which this row asks for."],
    ),
    // A boolean `define` read with its amount leaves out part of a row, and the earlier rows take the
    // rest: a row that once passed with nothing said.
    (
        "shapes/E102_a_row_left_to_the_earlier_rows",
        "shapes/E102_前の行に残された行",
        "E102",
        &[
            "row 4: no input reaches here",
            "Over the values of `amount` this row takes, `big` is always false.",
            "Because of `policy first`, the earlier rows take the rest of this row's values first.",
        ],
    ),
    // Rows no input reaches that the check once left undecided (W128) and passed the rule with
    // (§15.325): an area no whole side squares to, shown over pieces of the side, and a tier the
    // table above gives only from 82 yen, where the net is never what the row asks for, shown by
    // working the net out again on the amounts the rows above that give the tier take.
    (
        "shapes/E102_an_area_no_whole_side_gives",
        "shapes/E102_整数の辺では出ない面積",
        "E102",
        &["row 2: no input reaches here", "`area` never comes to what this row asks for."],
    ),
    (
        "shapes/E102_a_first_tier_at_a_net_of_minus_one",
        "shapes/E102_差が負の一段",
        "E102",
        &[
            "row 5: no input reaches here",
            "At some of the values of `amount` this row takes, `net` does not come to some of what this row asks for.",
            "`tier` is t1 only where row 2 or row 4 of table tier_of fires, and where either does, no combination of values meets all of this row's conditions.",
        ],
    ),
    // Days an applied table writes, each named in a cell of its own (§15.325). The day between the two
    // is one no row above writes; and a local order on the day only a national order ships is a zone
    // and a day that each come but never together, which the note says once the row's day is read as
    // the date it is (read as any value, the row is still unreachable, for a reason that names
    // neither).
    (
        "shapes/E102_a_day_the_table_above_never_writes",
        "shapes/E102_上の表が書かない日",
        "E102",
        &["row 2: no input reaches here", "The upstream table never produces the values this row names."],
    ),
    (
        "shapes/E102_a_day_the_zone_never_ships_on",
        "shapes/E102_その地域では出荷しない日",
        "E102",
        &["row 3: no input reaches here", "Either half happens; the two together do not."],
    ),
];

fn source(stem: &str) -> (String, String) {
    let rel = format!("tests/computed/{stem}.rule");
    let src = std::fs::read_to_string(root().join(&rel)).unwrap_or_else(|_| panic!("cannot read {rel}"));
    (rel, src)
}

/// What `rulec check <rel>` prints, in `lang`.
fn printed(rel: &str, src: &str, lang: Lang) -> String {
    let lines: Vec<String> = src.lines().map(String::from).collect();
    i18n::with(lang, || {
        let r = rulec::report(src, rel);
        format!("{}{}", rulec::findings_text(&r.diags, &lines), rulec::check_tail(&r.shadow, &r.diags, rel, 0))
    })
}

/// The findings of a rule's text, as (code, line where it is reported).
fn found(src: &str, rel: &str) -> Vec<(String, usize)> {
    i18n::with(Lang::En, || {
        rulec::check_source(src, rel)
            .iter()
            .map(|d| (d.code.to_string(), d.where_.strip_prefix(&format!("{rel}:")).and_then(|t| t.split(' ').next()).and_then(|n| n.parse().ok()).unwrap_or(0)))
            .collect()
    })
}

fn errors(src: &str, rel: &str) -> Vec<&'static str> {
    i18n::with(Lang::En, || rulec::check_source(src, rel).iter().filter(|d| d.severity == rulec::diag::Severity::Error).map(|d| d.code).collect())
}

/// The golden file of a rule's report, by the rule's file name.
fn golden(stem: &str, tag: &str, text: &str) -> Result<(), String> {
    let name = stem.rsplit('/').next().unwrap_or(stem);
    ritsu_testkit::golden::check(&root().join(format!("tests/golden/computed/{name}.{tag}.txt")), text)
}

/// A value of JSON as `rulec run` takes it on its command line.
fn plain(j: &Json) -> String {
    match j {
        Json::Int(n) => n.to_string(),
        Json::Bool(b) => b.to_string(),
        Json::Str(s) => s.clone(),
        other => panic!("not a value an input takes: {other:?}"),
    }
}

/// What `rulec run` answers for the rule at `rel` under `tests/computed`, given `args`
/// (`name=value`): each output, as `plain` writes it.
fn run(rel: &str, args: &[String]) -> Vec<(String, String)> {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .current_dir(root().join("tests/computed"))
        .arg("run")
        .arg(rel)
        .args(args)
        .args(["--format", "json"])
        .output()
        .expect("cannot start rulec");
    assert!(o.status.success(), "rulec run {rel} {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    let j = ritsu_base::json::parse(&String::from_utf8_lossy(&o.stdout)).unwrap_or_else(|e| panic!("rulec run {rel}: {e:?}"));
    let answer = j.get("answer").and_then(Json::as_obj).unwrap_or_else(|| panic!("rulec run {rel}: no answer"));
    answer.iter().map(|(k, v)| (k.clone(), plain(v))).collect()
}

fn answer_of(answer: &[(String, String)], output: &str) -> String {
    answer.iter().find(|(k, _)| k == output).map(|(_, v)| v.clone()).unwrap_or_else(|| panic!("no `{output}` in {answer:?}"))
}

/// The example `rulec check --format json` gives with `code` for the rule at `rel`: each column it
/// names, as `plain` writes its value.
fn example(rel: &str, code: &str) -> Vec<(String, String)> {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .current_dir(root())
        .args(["check", rel, "--format", "json"])
        .output()
        .expect("cannot start rulec");
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    for line in text.lines() {
        let Ok(j) = ritsu_base::json::parse(line) else { continue };
        if j.get("code").and_then(Json::as_str) == Some(code) {
            let inputs = j.get("witness").and_then(|w| w.get("inputs")).and_then(Json::as_obj).unwrap_or_else(|| panic!("{rel}: {code} gives no example"));
            return inputs.iter().map(|(k, v)| (k.clone(), plain(v))).collect();
        }
    }
    panic!("{rel}: no {code} in\n{text}")
}

/// Every rule here is listed above, with its Japanese version.
#[test]
fn every_rule_here_is_listed_with_its_japanese_version() {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, base, out);
            } else if let Some(stem) = p.strip_prefix(base).unwrap().to_string_lossy().strip_suffix(".rule") {
                out.push(stem.to_string());
            }
        }
    }
    let base = root().join("tests/computed");
    let mut here = Vec::new();
    walk(&base, &base, &mut here);
    here.sort();
    let mut listed: Vec<String> = PASSING.iter().flat_map(|(en, ja)| [en.to_string(), ja.to_string()]).chain(FAILING.iter().flat_map(|(en, ja, ..)| [en.to_string(), ja.to_string()])).collect();
    listed.sort();
    assert_eq!(here, listed, "every rule of tests/computed is listed here, with its Japanese version");
}

/// Each rule that reads a computed value beside the inputs it comes from passes with nothing said:
/// no row is asked for a combination that does not occur, and no pair of rows that meet only there
/// is called an overlap.
#[test]
fn the_rules_that_read_a_computed_value_beside_its_inputs_pass_with_nothing_said() {
    let mut failures = Vec::new();
    for (en, ja) in PASSING {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            let said = found(&src, &rel);
            if !said.is_empty() {
                failures.push(format!("{rel}: {said:?}\n{}", printed(&rel, &src, Lang::En)));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Each rule that does not pass stops at its one finding, says what is listed for it, and prints
/// what its golden file holds in both languages.
#[test]
fn the_ones_that_do_not_pass_say_what_their_golden_files_hold() {
    let mut failures = Vec::new();
    for (en, ja, code, said) in FAILING {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            let codes: Vec<String> = found(&src, &rel).into_iter().map(|(c, _)| c).collect();
            if codes != [code] {
                failures.push(format!("{rel}: {codes:?}, not one {code}"));
                continue;
            }
            for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
                let out = printed(&rel, &src, lang);
                if stem == en && tag == "en" {
                    failures.extend(said.iter().filter(|s| !out.contains(*s)).map(|s| format!("{rel} does not say `{s}`:\n{out}")));
                }
                if let Err(e) = golden(stem, tag, &out) {
                    failures.push(e);
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The Japanese version is the same rule with other names: the same findings on the same lines.
#[test]
fn the_japanese_version_says_the_same_things_in_the_same_places() {
    for (en, ja) in PASSING.iter().copied().chain(FAILING.iter().map(|(en, ja, ..)| (*en, *ja))) {
        let (re, se) = source(en);
        let (rj, sj) = source(ja);
        assert_eq!(found(&se, &re), found(&sj, &rj), "{en} and {ja}");
    }
}

/// Two inputs `fed by` a rule that one gap turns on are named together in one note, which speaks of
/// them as two, in English and in Japanese, for the rule in either language.
#[test]
fn two_inputs_fed_by_a_rule_in_one_gap_are_named_in_one_note() {
    const EN: &str = "rule two_fed v1\n\nenum kind = goods | capex\n\ninputs\n  kind : kind\n  a    : bool  fed by rulec \"shared/authority.rule\" output finance\n  b    : bool  fed by rulec \"shared/authority.rule\" output finance\n\noutputs\n  ok : bool\n\ntable t\n| kind  | a     | b     | -> ok |\n| goods | false | false | true  |\n| capex | true  | true  | false |\n";
    const JA: &str = "rule 二つの受け取り(two_fed) v1\n\nenum 種類(kind) = 物品(goods) | 設備(capex)\n\ninputs\n  種類(kind)    : 種類\n  経理を通す(a) : bool  fed by rulec \"shared/決裁権限.rule\" output 経理\n  法務を通す(b) : bool  fed by rulec \"shared/決裁権限.rule\" output 経理\n\noutputs\n  通す(ok) : bool\n\ntable 表(t)\n| 種類 | 経理を通す | 法務を通す | -> 通す |\n| 物品 | false      | false      | true    |\n| 設備 | true       | true       | false   |\n";
    let cases = [
        ("tests/computed/two_fed.rule", EN, "`a` and `b`", "`a` と `b`"),
        ("tests/computed/二つの受け取り.rule", JA, "`経理を通す` and `法務を通す`", "`経理を通す` と `法務を通す`"),
    ];
    for (rel, src, en, ja) in cases {
        assert_eq!(errors(src, rel), ["E101"], "{rel}");
        let out = printed(rel, src, Lang::En);
        let said = format!("{en} are inputs other rules' outputs feed (`fed by`)");
        assert!(out.contains(&said) && out.contains("bring the rules in with `apply`") && out.matches("(`fed by`)").count() == 1, "{rel}:\n{out}");
        let out = printed(rel, src, Lang::Ja);
        let said = format!("{ja} は、ほかの規則の出力を受け取る入力です（`fed by`）");
        assert!(out.contains(&said) && out.contains("それぞれの規則を `apply` で取り込んでください") && out.matches("（`fed by`）").count() == 1, "{rel}:\n{out}");
    }
}

/// A combination that occurs is still asked for: `purchase_route` without its row for goods up to a
/// million yen with no review by finance is E101, with that combination as the example and in the
/// row the hint adds, and the table of authority answers no review for the example's kind and
/// amount. The value the rows no longer write is W111, as before. The gap and the hint say the
/// amounts up to a million yen, the only ones goods come with no review by finance (§15.325), where
/// they said any amount before.
#[test]
fn a_row_for_a_combination_that_occurs_is_still_asked_for() {
    let cases = [
        (
            "purchase_route",
            "| goods    | <=1_000_000JPY | false             | manager  | none         | dept_head     |\n",
            "E101_without_the_row_for_goods",
            "An input that matches no row: kind = goods, amount = 1JPY, authority_finance = false",
            "`| goods | <=1_000_000JPY | false | ? | ? | ? |`",
            ("shared/authority.rule", ["kind=goods", "amount=1"], "finance"),
        ),
        (
            "購買の経路",
            "| 物品         | <=1_000_000円 | false      | 課長    | none   | 部長   |\n",
            "E101_物品の行の無い経路",
            "An input that matches no row: 種類 = 物品, 金額 = 1円, 経理を通す = false",
            "`| 物品 | <=1_000_000円 | false | ? | ? | ? |`",
            ("shared/決裁権限.rule", ["種類=物品", "金額=1"], "経理"),
        ),
    ];
    for (stem, row, name, said, hint, (callee, args, output)) in cases {
        let (rel, src) = source(stem);
        let without = src.replacen(row, "", 1);
        assert_ne!(without, src, "{rel}: the row is not there to take out");
        // the examples go with it: the first of them is the row's
        let without = without.split("\nexamples\n").next().unwrap().to_string();
        assert_eq!(errors(&without, &rel), ["E101"], "{rel}");
        let out = printed(&rel, &without, Lang::En);
        assert!(out.contains(said) && out.contains(hint), "{rel} without the row:\n{out}");
        for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            if let Err(e) = golden(name, tag, &printed(&rel, &without, lang)) {
                panic!("{e}");
            }
        }
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        assert_eq!(answer_of(&run(callee, &args), output), "false", "{callee} {args:?}");
    }
}

/// Every example a report gives here is one that occurs: run through `rulec run` of the rule that
/// works the value out, the input the example names comes to the value it shows. The E101 of a
/// value `fed by` another rule is the exception the report names: for the example's kind and
/// amount, the table of authority answers no review, which the input that rule feeds would then
/// carry.
#[test]
fn each_example_comes_to_what_the_rule_that_works_it_out_answers() {
    // the rule, its code, the rule applied, the input passed on, the column, the applied output
    let cases = [
        ("shapes/E101_a_gap_the_first_amount_misses", "E101", "shapes/shared/fee_band.rule", "amount", "f_fee", "charged"),
        ("shapes/E101_最初の金額では当たらない穴", "E101", "shapes/shared/手数料と区分.rule", "金額", "受けた手数料", "請求額"),
        ("shapes/E105_rows_that_meet_on_the_large_band", "E105", "shapes/shared/tier.rule", "amount", "t_band", "band"),
        ("shapes/E105_大の区分で重なる行", "E105", "shapes/shared/金額の区分.rule", "金額", "受けた区分", "区分"),
    ];
    for (stem, code, callee, input, column, output) in cases {
        let rel = format!("tests/computed/{stem}.rule");
        let shown = example(&rel, code);
        let value = |name: &str| shown.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()).unwrap_or_else(|| panic!("{rel}: the example names no `{name}`: {shown:?}"));
        let answer = run(callee, &[format!("{input}={}", value(input))]);
        assert_eq!(answer_of(&answer, output), value(column), "{rel}: {shown:?}");
    }
    for (stem, callee, kind, amount, fed, output) in [
        ("E101_fed_by_another_rule", "shared/authority.rule", "kind", "amount", "authority_finance", "finance"),
        ("E101_ほかの規則が入れる値", "shared/決裁権限.rule", "種類", "金額", "経理を通す", "経理"),
    ] {
        let rel = format!("tests/computed/{stem}.rule");
        let shown = example(&rel, "E101");
        let value = |name: &str| shown.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()).unwrap();
        assert_eq!(value(fed), "true", "{rel}: {shown:?}");
        let answer = run(callee, &[format!("{kind}={}", value(kind)), format!("{amount}={}", value(amount))]);
        assert_eq!(answer_of(&answer, output), "false", "{rel}: {shown:?}");
    }
}

/// What the reports and the descriptions say of the fees, run: 3% of an amount, rounded down to
/// the yen, is 900 yen from 30,000 yen to 30,033 yen and passes 900 yen only from 30,034 yen; the
/// fee a `define … range` works out passes 1,000 yen only from 33,367 yen.
#[test]
fn the_fees_come_to_what_the_reports_say() {
    for (callee, input, output) in [
        ("shapes/shared/calc.rule", "amount", "fee"),
        ("shapes/shared/手数料の計算.rule", "金額", "手数料"),
        ("shapes/shared/fee_band.rule", "amount", "charged"),
        ("shapes/shared/手数料と区分.rule", "金額", "請求額"),
    ] {
        for (amount, fee) in [(30_000, 900), (30_001, 900), (30_033, 900), (30_034, 901), (33_366, 1000), (33_367, 1001)] {
            let answer = run(callee, &[format!("{input}={amount}")]);
            assert_eq!(answer_of(&answer, output), fee.to_string(), "{callee} at {amount}");
        }
    }
}

/// `rulec certificate` of the rule at `rel`.
fn certificate(rel: &str) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").current_dir(root()).args(["certificate", rel]).output().expect("cannot start rulec");
    assert!(o.status.success(), "rulec certificate {rel}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}

/// One certificate fed to a re-checker with the rule it is about: the exit code and what it says.
fn fed(program: &Path, args: &[&str], cert: &str) -> (i32, String) {
    use std::io::Write;
    let mut p = Command::new(program).current_dir(root()).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap_or_else(|e| panic!("cannot start {}: {e}", program.display()));
    p.stdin.as_mut().unwrap().write_all(cert.as_bytes()).unwrap();
    let o = p.wait_with_output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
}

/// The forgeries, each of one new kind of fact: the rule, what is replaced, with what, and what the
/// lie is.
const FORGED: [(&str, &str, &str, &str); 47] = [
    (
        "shapes/bool_define",
        r#"{"axis":0,"by":[{"axis":1,"coord":2}],"coords":[0]}"#,
        r#"{"axis":0,"by":[{"axis":1,"coord":2}],"coords":[1]}"#,
        "at 100,000 yen, `big` called never false",
    ),
    ("shapes/bool_define", r#"{"narrowed":2},{"row":2}"#, r#"{"narrowed":2},{"narrowed":3}"#, "a box row 2 takes, called one the define never comes to"),
    (
        "shapes/callee_define_range",
        r#"{"axis":0,"by":[{"axis":1,"coord":3}],"coords":[0]}"#,
        r#"{"axis":0,"by":[{"axis":1,"coord":3}],"coords":[0,1]}"#,
        "past 30,000 yen, a fee of 900 yen called one the define never comes to",
    ),
    (
        "shapes/number_define",
        r#"{"row":1,"at":[0,0],"values":{"amount":1,"fee":899},"at_values":["899","1"]"#,
        r#"{"row":1,"at":[1,0],"values":{"amount":1,"fee":900},"at_values":["900","1"]"#,
        "the point that reaches row 1 moved to a fee of 900 yen on an amount of 1 yen, which the define never comes to",
    ),
    ("shapes/enum_output_with", r#""via":[["t:bands",1]]"#, r#""via":[["t:bands",2]]"#, "another row named on the way the value comes"),
    ("shapes/enum_output_with", r#""via":[["t:bands",1]],"#, "", "the way the value comes left out"),
    ("shapes/two_levels", r#""via":[["w:inner:bands",3]]"#, r#""via":[["w:inner:bands",2]]"#, "another row of the rule applied in turn named on the way"),
    ("shapes/number_output", r#""above":{"never":[{"axis":1,"coord":0},"#, r#""above":{"never":[{"axis":1,"coord":1},{"axis":1,"coord":0},"#, "a fee the table above writes, called one it never writes"),
    (
        "shapes/number_output",
        r#"{"above_rows":{"axis":1,"column":"t_fee","value":"300JPY","rows":[{"row":2,"farkas":[{"cond":{"name":"amount","hi":false,"at":"10000","open":true}"#,
        r#"{"above_rows":{"axis":1,"column":"t_fee","value":"300JPY","rows":[{"row":2,"farkas":[{"cond":{"name":"amount","hi":false,"at":"20000","open":true}"#,
        "an end the row above does not keep: it takes amounts from 10,001 yen",
    ),
    (
        "shapes/overlap_none_occurs",
        r#"{"row":2,"at":[2,5],"values":{"amount":100001,"t_band":"large"},"at_values":[null,"100001"]"#,
        r#"{"row":2,"at":[2,5],"values":{"amount":50001,"t_band":"large"},"at_values":[null,"50001"]"#,
        "the point that reaches row 2 moved to an amount whose band is medium",
    ),
    (
        "shapes/overlap_none_occurs",
        r#""rows":[{"row":3,"farkas":[{"cond":{"name":"amount","hi":false,"at":"100000","open":true}"#,
        r#""rows":[{"row":2,"farkas":[{"cond":{"name":"amount","hi":false,"at":"100000","open":true}"#,
        "the two rows parted by a row above that does not write the band they take",
    ),
    (
        "shapes/fee_in_steps",
        r#"{"axis":0,"by":[],"coords":[2,4]}"#,
        r#"{"axis":0,"by":[],"coords":[2,3,4]}"#,
        "a fee of 20 yen, which the grid of 10 yen holds, called one the define never comes to",
    ),
    (
        "shapes/amount_read_twice",
        r#"{"axis":0,"by":[{"axis":1,"coord":3}],"coords":[0,2]}"#,
        r#"{"axis":0,"by":[{"axis":1,"coord":3}],"coords":[0,1,2]}"#,
        "from 31 yen to 39 yen, a shortfall of 0 yen called one never reached, where the amount less itself is 0 yen",
    ),
    (
        "shapes/top_up_to_minimum",
        r#""cuts":{"axis":0,"at":["16","30"]}"#,
        r#""cuts":{"axis":0,"at":["30"]}"#,
        "the amounts from 2 yen to 29 yen read as one piece, over which the top-up is not shown to stay under 24 yen",
    ),
    (
        "shapes/top_up_to_minimum",
        r#","cuts":{"axis":0,"at":["16","30"]}"#,
        "",
        "the pieces left out, so that the amounts from 2 yen to 57 yen are worked out together",
    ),
    (
        "shapes/first_fee_above",
        r#"{"a":{"axis":1,"coord":1},"b":{"axis":0,"coord":2},"input":"amount","spans":[["0","37"],["38","38"]]}"#,
        r#"{"a":{"axis":1,"coord":1},"b":{"axis":0,"coord":2},"input":"amount","spans":[["0","36"],["38","38"]]}"#,
        "the amounts a fee of 50 yen comes from written narrower than the rows above give them",
    ),
    (
        "shapes/first_band_above",
        r#""spans":[["115","115"],["102","102"]]"#,
        r#""spans":[["102","102"],["102","102"]]"#,
        "small written as coming at 102 yen, where the rows above write medium",
    ),
    (
        "shapes/tier_of_small_orders",
        r#""b":{"axis":1,"coord":0},"input":"small""#,
        r#""b":{"axis":1,"coord":1},"input":"small""#,
        "gold called apart from an order that is not small, on the truth value of `small`",
    ),
    (
        "shapes/first_band_rows_together",
        r#""earlier":{"step":0,"rows":[1,2]}"#,
        r#""earlier":{"step":0,"rows":[1]}"#,
        "the rows that take every order under 46 yen before the last row can, named without the row for services",
    ),
    (
        "shapes/first_fee_taken",
        r#"{"row":1,"farkas":[{"cond":{"name":"amount","hi":false,"at":"34","open":false},"y":"1/5"},{"coord":{"axis":1,"hi":true,"at":"29","open":true},"y":"1/5"}]}"#,
        r#"{"row":1,"earlier":{"step":0,"rows":[2]}}"#,
        "the first row of the table above called taken by the second, which comes after it",
    ),
    (
        "shapes/first_fee_taken",
        r#"{"row":3,"earlier":{"step":0,"rows":[2]}}"#,
        r#"{"row":3,"misses":{"step":0}}"#,
        "the last row of the table above called one that takes none of the box, where only the second row keeps it from deciding",
    ),
    (
        "shapes/first_band_two_rows_one_cut",
        r#""earlier":{"step":0,"rows":[3]}"#,
        r#""earlier":{"step":0,"rows":[1]}"#,
        "the amounts from 1 yen to 12 yen called taken by the first row, which takes only those under 1 yen",
    ),
    (
        "shapes/first_band_rows_apart",
        r#"{"row":2,"at":[0,1,2],"values":{"amount":100,"kind":"goods","band":"medium"},"at_values":[null,null,"100"]"#,
        r#"{"row":2,"at":[0,1,0],"values":{"amount":1,"kind":"goods","band":"medium"},"at_values":[null,null,"1"]"#,
        "the point that reaches row 2 moved to goods of 1 yen, which are small: the last row above lets them in, but the first takes them before it can",
    ),
    (
        "shapes/tier_through_band",
        r#""earlier":{"step":1,"rows":[1,2]}"#,
        r#""earlier":{"step":0,"rows":[1,2]}"#,
        "the rows that take the amount before the band can be small named in the table of tiers, which is not `policy first`",
    ),
    (
        "shapes/tier_through_band",
        r#""value":"t3","through":true,"#,
        r#""value":"t3","#,
        "a way that goes on to the table of bands called one that stops at the table of tiers",
    ),
    (
        "shapes/band_cut_between_yen",
        r#"{"row":1,"misses":{"step":0}}"#,
        r#"{"row":1,"earlier":{"step":0,"rows":[]}}"#,
        "a row of a table that is not `policy first` called taken by the rows before it",
    ),
    (
        "shapes/tier_and_fee_point_by_point",
        r#",{"a":{"axis":0,"coord":1},"b":{"axis":2,"coord":1},"input":"amount","spans":[["23","55"],["2","17"]]}"#,
        "",
        "the fact a box ruled out amount by amount rests on left out: tier t3 comes only from 23 yen to 55 yen, and the box's amounts are 2 yen to 17 yen",
    ),
    (
        "shapes/tier_witness_through",
        r#""values":{"amount":36,"fee":190,"tier":"t3"},"at_values":[null,"36","190"]"#,
        r#""values":{"amount":34,"fee":190,"tier":"t3"},"at_values":[null,"34","190"]"#,
        "the point that reaches row 1 moved to 34 yen, where the fee is 210 yen",
    ),
    (
        "shapes/same_rule_date",
        r#""value":"20635"},"source":{"line":15"#,
        r#""value":"20637"},"source":{"line":15"#,
        "the day the first row of the table above writes read as 3 July, the second row's day, where the file writes 1 July",
    ),
    (
        "shapes/same_rule_date",
        r#""above":{"never":[{"axis":0,"coord":1},{"axis":0,"coord":3}"#,
        r#""above":{"never":[{"axis":0,"coord":1},{"axis":0,"coord":2},{"axis":0,"coord":3}"#,
        "1 July, which the table above writes for a local order, called a day it never writes",
    ),
    (
        "shapes/date_output",
        r#""above":{"never":[{"axis":0,"coord":0},"#,
        r#""above":{"never":[{"axis":0,"coord":1},{"axis":0,"coord":0},"#,
        "1 July, which the applied table writes for a local order, called a day it never writes",
    ),
    (
        "shapes/date_with_zone",
        r#""value":"2026-07-03","rows":[{"row":2,"clash":0}]"#,
        r#""value":"2026-07-03","rows":[{"row":1,"clash":0}]"#,
        "the row above that writes 3 July named as the one for local orders, which writes 1 July",
    ),
    (
        "shapes/same_rule_optional_number",
        r#""bounds":[null,[null,"100"]"#,
        r#""bounds":[["0","0"],[null,"100"]"#,
        "the coordinate of no fee called a fee of 0 yen, so that the axis has none for the absent value",
    ),
    (
        "shapes/top_up_to_minimum",
        r#"{"axis":1,"by":[{"axis":0,"coord":3}],"coords":[1,2,3,4]}"#,
        r#"{"axis":1,"by":[{"axis":0,"coord":3}],"coords":[0,1,2,3,4]}"#,
        "from 59 yen to 350 yen, a top-up of 2 yen or less called one never reached, where it is 0 yen",
    ),
    (
        "shapes/boxes_at_37_yen",
        r#"{"axis":1,"by":[{"axis":0,"coord":1}],"coords":[2,3,5]}"#,
        r#"{"axis":1,"by":[{"axis":0,"coord":1}],"coords":[1,2,3,5]}"#,
        "from 1 yen to 77 yen, a fee of 37 yen called one never reached, where one box comes to it",
    ),
    (
        "shapes/fee_the_rule_accepts",
        r#"{"axis":0,"by":[{"axis":1,"coord":8}],"coords":[2,3]}"#,
        r#"{"axis":0,"by":[{"axis":1,"coord":8}],"coords":[1,2,3]}"#,
        "at 48 yen, a fee of 5 yen, which the rule accepts, called one it never comes to",
    ),
    (
        "shapes/items_over_the_limit",
        r#"{"axis":0,"by":[{"axis":1,"coord":1}],"coords":[2]}"#,
        r#"{"axis":0,"by":[{"axis":1,"coord":3}],"coords":[2]}"#,
        "from 3 to 9 items, an excess above 0 called one never reached, where 3 items over a limit of 1 exceed it",
    ),
    (
        "shapes/net_of_two_roundings",
        r#""coords":[0,3],"cuts":{"name":"amount","at":["54","80","93","100"]}"#,
        r#""coords":[0,2,3],"cuts":{"name":"amount","at":["54","80","93","100"]}"#,
        "a net of -1 yen, which an amount up to 50 yen gives, called one no amount gives",
    ),
    (
        "shapes/net_of_two_roundings",
        r#""cuts":{"name":"amount","at":["54","80","93","100"]}"#,
        r#""cuts":{"name":"amount","at":["54","80","93"]}"#,
        "the amounts from 93 yen to 105 yen read as one piece, over which the net is not shown to stay at -2 yen or -1 yen",
    ),
    (
        "shapes/tier_from_the_fee",
        r#"{"row":2,"reach":{"step":0,"axis":0,"by":[1]}}"#,
        r#"{"row":2,"reach":{"step":0,"axis":0,"by":[]}}"#,
        "the fee worked out again with the amount not held, where it comes to 1 yen",
    ),
    (
        "shapes/tier_from_a_threshold",
        r#"{"row":2,"reach":{"step":0,"axis":0,"by":[1]}}"#,
        r#"{"row":2,"reach":{"step":0,"axis":0,"by":[]}}"#,
        "the truth of the comparison read with the amount not held, where it can be true",
    ),
    (
        "shapes/packing_on_small_orders",
        r#"{"row":2,"reach":{"step":0,"axis":0,"by":[1],"row_by":[1]}}"#,
        r#"{"row":2,"reach":{"step":0,"axis":0,"by":[1]}}"#,
        "the fee worked out again without the amount held to the span the row takes, where it comes to 8 yen",
    ),
    (
        "shapes/top_up_tier_by_items",
        r#"{"row":9,"reach":{"step":0,"axis":1,"by":[1],"cuts":{"axis":1,"at":["19"]}}}"#,
        r#"{"row":9,"reach":{"step":0,"axis":1,"by":[1]}}"#,
        "the top-up worked out over the amounts from 6 yen to 30 yen at once, where it is not shown under 14 yen",
    ),
    (
        "shapes/top_up_tier_by_items",
        r#"{"row":9,"reach":{"step":0,"axis":1,"by":[1],"cuts":{"axis":1,"at":["19"]}}}"#,
        r#"{"row":9,"reach":{"step":0,"axis":1,"by":[1],"cuts":{"axis":1,"at":["10"]}}}"#,
        "the amounts cut at 10 yen, where the piece from 10 yen to 30 yen is not shown under 14 yen",
    ),
    (
        "shapes/shipping_tier_by_region",
        r#""disjoint":{"steps":[0,1],"axes":[0,0]}"#,
        r#""disjoint":{"steps":[0,1],"axes":[0,1]}"#,
        "two rows of a way said to take no value together of two columns that are not one, the region and the amount",
    ),
    (
        "shapes/fee_written_above",
        r#"{"row":4,"via":[["fee_of",1]],"writes":{"step":1,"axis":1}}"#,
        r#"{"row":4,"via":[["fee_of",1]],"writes":{"step":1,"axis":0}}"#,
        "the fee a row above writes set against the axis of the tier, which another table decides",
    ),
    (
        "shapes/fee_written_above",
        r#"{"row":3,"via":[["fee_of",6]],"farkas":[{"cond":{"name":"balance","hi":false,"at":"10","open":false},"y":"1/50"},{"coord":{"axis":2,"hi":true,"at":"-40","open":false},"y":"1/50"}]}"#,
        r#"{"row":3,"via":[["fee_of",6]],"writes":{"step":1,"axis":1}}"#,
        "the row of the fee table that writes 80 yen called one no fee of the box stands on, where the fees under 101 yen take it",
    ),
];

/// Each certificate here passes `tools/recheck.py` and the Lean program that carries the proofs,
/// and each forgery of a new fact (§15.322) — what a `define` comes to with the axes it reads held,
/// on the grid its values sit on and over the pieces one held coordinate is cut into, the way a value
/// comes through the tables above, a number a table above never writes, the point that reaches a
/// row, the rows above that part two rows, the amounts the rows of a `policy first` table above
/// write a word or a number from — is refused by both. So is each forgery of a fact §15.325 adds:
/// the rows of a `policy first` table above that take a box before a later row can, a row above
/// that takes nothing of a box, a way that goes one table further up, the facts a box ruled out
/// amount by amount rests on, the point that reaches a row through such a way, the day a row above
/// writes and a day it never writes, the row above that writes a day, and the absent value of a fee.
/// And so is each forgery of the facts §15.325 adds to rule out what a `define` never comes to: what
/// it comes to with the amount held, read as the amount less itself; on the whole steps of a fee;
/// within the span its `range` accepts; on whole numbers; over the pieces of an input a table that
/// reads only the `define` is ruled out on; worked out again with the axes a row of a table that
/// reads it takes held, with an input that row cuts held to its span, and over pieces of the amount;
/// the truth of a comparison so worked out; two rows of a way that take no value of a column
/// together; and a number a row above writes that no coordinate of the box takes.
#[test]
fn every_certificate_here_passes_both_rechecks_and_no_forgery_does() {
    let have = Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    let python = ready(Need::Python, || have, "python3 is not here, so tools/recheck.py is not run").then(|| PathBuf::from("python3"));
    let bin = root().join("../../proofs/.lake/build/bin/rulec-recheck");
    let lean = ready(Need::Lean, || bin.exists(), "ritsu's proofs/ is not built (lake build there makes it)").then_some(bin);
    let check = |rel: &str, cert: &str| -> Vec<(&'static str, i32, String)> {
        let mut said = Vec::new();
        if let Some(p) = &python {
            let (c, s) = fed(p, &["tools/recheck.py", "--rule", rel], cert);
            said.push(("tools/recheck.py", c, s));
        }
        if let Some(b) = &lean {
            let (c, s) = fed(b, &["--rule", rel], cert);
            said.push(("Lean", c, s));
        }
        said
    };
    for (en, ja) in PASSING {
        for stem in [en, ja] {
            let rel = format!("tests/computed/{stem}.rule");
            let cert = certificate(&rel);
            for (who, c, s) in check(&rel, &cert) {
                assert_eq!(c, 0, "{rel} under {who}:\n{s}");
            }
        }
    }
    for (stem, from, to, what) in FORGED {
        let rel = format!("tests/computed/{stem}.rule");
        let cert = certificate(&rel);
        assert!(cert.contains(from), "{what}: the certificate of {rel} no longer has the shape the forgery is written against:\n{cert}");
        let forged = cert.replacen(from, to, 1);
        for (who, c, s) in check(&rel, &forged) {
            assert_eq!(c, 1, "{what}: {who} let the forgery of {rel} through:\n{s}");
            assert!(s.contains("FAILED") || who != "Lean", "{what}: {s}");
        }
    }
}

/// Two rows of a table that meet only where the first row of a `policy first` table above takes
/// the amount before the row that would write the band they ask for (§15.325) are parted in the
/// certificate by the rows above, not left as a pair it states without proof.
#[test]
fn rows_that_meet_only_where_a_row_above_comes_first_are_proved_apart() {
    for (stem, table) in [("shapes/first_band_rows_apart", "pick"), ("shapes/先にとる行で離れる二つの行", "選ぶ")] {
        let rel = format!("tests/computed/{stem}.rule");
        let cert = ritsu_base::json::parse(&certificate(&rel)).unwrap_or_else(|e| panic!("{rel}: {e:?}"));
        let tables = cert.get("tables").and_then(Json::as_arr).unwrap();
        let t = tables.iter().find(|t| t.get("table").and_then(Json::as_str) == Some(table)).unwrap_or_else(|| panic!("{rel}: no table {table}"));
        let stated = t.get("undecided").and_then(Json::as_arr).map(|a| a.len()).unwrap_or(0);
        assert_eq!(stated, 0, "{rel}: pairs stated, not proved: {:?}", t.get("undecided"));
        let apart = t.get("above_apart").and_then(Json::as_arr).unwrap_or_else(|| panic!("{rel}: no pair parted by the rows above"));
        let pair: Vec<(i128, i128)> = apart.iter().map(|d| (d.get("a").and_then(Json::as_int).unwrap_or(0), d.get("b").and_then(Json::as_int).unwrap_or(0))).collect();
        assert_eq!(pair, [(2, 4)], "{rel}");
    }
}

/// A date a table above writes is read as the day it stands for (§15.325). The table below is asked
/// for rows only on the days the rows above write: its certificate says the day between, 2 July, is
/// one the tables above never write, and both re-checkers settle that from the rows above, whether
/// the table below cuts the days at the two or names each in a cell of its own. A date of the same
/// rule's table that is absent for an order picked up at the shop, and a fee absent the same way,
/// leave nothing stated rather than proved: each day the rows write is read back from the file, the
/// date sits on a step of one day, and the absent value has its own coordinate. So does the date
/// named day by day under `policy first`.
#[test]
fn a_date_a_table_above_writes_is_read_as_its_day() {
    for (stem, table) in [
        ("shapes/date_output", "pick"),
        ("shapes/日付の出力", "選ぶ"),
        ("shapes/same_rule_date", "pick"),
        ("shapes/同じ規則の表の日付", "選ぶ"),
        ("shapes/date_on_its_day", "pick"),
        ("shapes/一日ずつ読む日付", "選ぶ"),
    ] {
        let rel = format!("tests/computed/{stem}.rule");
        let cert = ritsu_base::json::parse(&certificate(&rel)).unwrap_or_else(|e| panic!("{rel}: {e:?}"));
        let tables = cert.get("tables").and_then(Json::as_arr).unwrap();
        let t = tables.iter().find(|t| t.get("table").and_then(Json::as_str) == Some(table)).unwrap_or_else(|| panic!("{rel}: no table {table}"));
        let axis = t.get("axes").and_then(Json::as_arr).and_then(|a| a.first()).unwrap_or_else(|| panic!("{rel}: {table} has no axis"));
        let coords: Vec<String> = axis.get("coords").and_then(Json::as_arr).unwrap().iter().map(plain).collect();
        let between = coords.iter().position(|c| c == "2026-07-02").unwrap_or_else(|| panic!("{rel}: no coordinate for 2 July in {coords:?}")) as i128;
        let never: Vec<i128> = t.get("above").and_then(|a| a.get("never")).and_then(Json::as_arr).unwrap().iter().filter_map(|f| f.get("coord").and_then(Json::as_int)).collect();
        assert!(never.contains(&between), "{rel}: 2 July is not among the days the tables above never write: {never:?}");
    }
    let have = Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    let python = ready(Need::Python, || have, "python3 is not here, so tools/recheck.py is not run").then(|| PathBuf::from("python3"));
    let bin = root().join("../../proofs/.lake/build/bin/rulec-recheck");
    let lean = ready(Need::Lean, || bin.exists(), "ritsu's proofs/ is not built (lake build there makes it)").then_some(bin);
    for stem in [
        "shapes/same_rule_date",
        "shapes/同じ規則の表の日付",
        "shapes/same_rule_optional_number",
        "shapes/値の無いことがある手数料",
        "shapes/same_rule_date_on_its_day",
        "shapes/同じ規則の表の日付を一日ずつ読む",
    ] {
        let rel = format!("tests/computed/{stem}.rule");
        let cert = certificate(&rel);
        if let Some(p) = &python {
            let (c, s) = fed(p, &["tools/recheck.py", "--rule", &rel], &cert);
            assert!(c == 0 && s.contains("every claim this program states was proved"), "{rel} under tools/recheck.py:\n{s}");
        }
        if let Some(b) = &lean {
            let (c, s) = fed(b, &["--rule", &rel], &cert);
            assert!(c == 0 && s.contains("OK: every claim this program states was proved"), "{rel} under Lean:\n{s}");
        }
    }
}

/// The rows `rulec run --trace` takes, by table and number, for the rule at `rel` given `args`.
fn taken(rel: &str, args: &[String]) -> Vec<(String, i128)> {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .current_dir(root())
        .arg("run")
        .arg(rel)
        .args(args)
        .args(["--format", "json", "--trace"])
        .output()
        .expect("cannot start rulec");
    assert!(o.status.success(), "rulec run {rel} {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    let j = ritsu_base::json::parse(&String::from_utf8_lossy(&o.stdout)).unwrap_or_else(|e| panic!("rulec run {rel}: {e:?}"));
    let rows = j.get("rows").and_then(Json::as_arr).unwrap_or_else(|| panic!("rulec run {rel}: no rows"));
    rows.iter().map(|r| (r.get("table").and_then(Json::as_str).unwrap_or("").to_string(), r.get("row").and_then(Json::as_int).unwrap_or(0))).collect()
}

/// The point a certificate gives for a row reached through a way that goes one table further up
/// (§15.325) is one an input reaches: the amount it names, run, takes that row. The tier is t3 for a
/// fee of 190 yen, which the amount gives up to 20 yen and from 36 yen; the point for the row that
/// asks for both above 33 yen was 34 yen, where the fee is 210 yen.
#[test]
fn the_point_for_a_row_reached_through_a_further_table_is_one_an_input_reaches() {
    for (stem, table, amount) in [("shapes/tier_witness_through", "pick", "amount"), ("shapes/手数料で決まる段階に届く点", "選ぶ", "金額")] {
        let rel = format!("tests/computed/{stem}.rule");
        let cert = ritsu_base::json::parse(&certificate(&rel)).unwrap_or_else(|e| panic!("{rel}: {e:?}"));
        let tables = cert.get("tables").and_then(Json::as_arr).unwrap();
        let t = tables.iter().find(|t| t.get("table").and_then(Json::as_str) == Some(table)).unwrap_or_else(|| panic!("{rel}: no table {table}"));
        let reach = t.get("reach").and_then(Json::as_arr).unwrap();
        let first = reach.iter().find(|r| r.get("row").and_then(Json::as_int) == Some(1)).unwrap_or_else(|| panic!("{rel}: no point for row 1"));
        let at = first.get("values").and_then(|v| v.get(amount)).map(plain).unwrap_or_else(|| panic!("{rel}: the point names no {amount}"));
        assert_eq!(at, "36", "{rel}: the point for row 1");
        let rows = taken(&rel, &[format!("{amount}={at}")]);
        assert!(rows.contains(&(table.to_string(), 1)), "{rel} at {amount} = {at} takes {rows:?}");
    }
}

/// An enum's value as `outputs_over` answers it, by its public name.
fn public(e: &Engine, path: &str, output: &str, v: &Value) -> Value {
    let Value::Enum(name) = v else { return v.clone() };
    let facts = e.facts(Path::new(path)).unwrap();
    let en = match facts.outputs.iter().find(|c| c.name == output).map(|c| c.ty.clone()) {
        Some(ritsu_ports::ColumnType::Enum { name: en, .. }) => en,
        Some(ritsu_ports::ColumnType::Opt(t)) => match *t {
            ritsu_ports::ColumnType::Enum { name: en, .. } => en,
            _ => return v.clone(),
        },
        _ => return v.clone(),
    };
    let value = facts.enums.iter().find(|x| x.name == en).and_then(|x| x.values.iter().find(|y| y.name == *name)).unwrap_or_else(|| panic!("{path}: {name} is no value of {en}"));
    Value::Enum(value.public.clone())
}

/// What the port answers over ranges reads the value the same way: held to amounts up to 10,000
/// yen, a table that reads only the band another rule gives comes to one value, and so does the
/// route of goods up to a million yen. Each value comes with an input the evaluator takes to it. A
/// rule whose row no input reaches does not pass, and the port answers nothing of it.
#[test]
fn the_port_over_ranges_reads_the_rows_above_too() {
    let e = Engine::new();
    let one = |s: &str| Value::Enum(s.to_string());
    let cases: Vec<(&str, &str, Vec<(String, Option<i128>, Option<i128>)>, Vec<Value>)> = vec![
        ("tests/computed/shapes/band_alone.rule", "x", vec![("amount".into(), Some(1), Some(10_000))], vec![one("one")]),
        ("tests/computed/shapes/band_alone.rule", "x", vec![("amount".into(), Some(10_001), Some(100_000))], vec![one("two")]),
        ("tests/computed/shapes/band_alone.rule", "x", vec![], vec![one("one"), one("two"), one("three")]),
        ("tests/computed/shapes/区分だけを読む表.rule", "答え", vec![("金額".into(), Some(1), Some(10_000))], vec![one("one")]),
        ("tests/computed/purchase_route.rule", "decide", vec![("kind".into(), Some(0), Some(0)), ("amount".into(), Some(1), Some(1_000_000))], vec![one("dept_head")]),
        ("tests/computed/purchase_route.rule", "finance", vec![("kind".into(), Some(0), Some(0)), ("amount".into(), Some(1), Some(1_000_000))], vec![Value::None]),
        ("tests/computed/purchase_route.rule", "decide", vec![], vec![one("dept_head"), one("division_head"), one("president")]),
        ("tests/computed/購買の経路.rule", "決裁", vec![("種類".into(), Some(0), Some(0)), ("金額".into(), Some(1), Some(1_000_000))], vec![one("dept_head")]),
    ];
    for (path, output, held, want) in cases {
        let found = e.outputs_over(Path::new(path), output, &held).unwrap_or_else(|x| panic!("{path}: {x:?}"));
        let Found::Value(vs) = found else { panic!("{path} {output} over {held:?}: {found:?}") };
        assert_eq!(vs.iter().map(|(v, _)| v.clone()).collect::<Vec<_>>(), want, "{path} {output} over {held:?}");
        for (v, input) in &vs {
            let outs = e.eval(Path::new(path), input).unwrap_or_else(|x| panic!("{path}: {x:?}"));
            let got = outs.iter().find(|(n, _)| n == output).map(|(_, x)| public(&e, path, output, x));
            assert_eq!(got.as_ref(), Some(v), "{path} {output}: {input:?}");
        }
    }
    assert!(e.outputs_over(Path::new("tests/computed/E102_a_fee_no_amount_gives.rule"), "hidden", &[]).is_err());
}
