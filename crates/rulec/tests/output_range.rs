//! The range written on an output's line, and the range of the output a `result` decides
//! (DESIGN §15.268).
//!
//! A range on an output's line used to be stepped over in silence. It is now held to what the
//! output comes to — E140 when it leaves some of that out, or when nothing says what that is —
//! and is then the output's range wherever an input's or a derive's is said: the certificate's
//! `ranges`, the port the other languages read a rule through, the page, the schema, and `diff`.
//! The output a `result` decides has the range of what it comes to in the certificate and the
//! port when its line writes none, and two shares of one amount taken at running totals in order
//! are read as what they are: the second never below the first.

use ritsu_ports::{ColumnType, Found, Rules};
use rulec::i18n::Lang;
use std::path::{Path, PathBuf};
use std::process::Command;
use ritsu_testkit::TempDir;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rulec(dir: &Path, args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").current_dir(dir).args(args).output().expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

/// The codes `src` says, and what it prints in each language.
fn said(src: &str) -> (Vec<&'static str>, String, String) {
    said_at(src, "t.rule")
}

/// [`said`] of `src` checked as the file at `path`, which finds the files it names beside it.
fn said_at(src: &str, path: &str) -> (Vec<&'static str>, String, String) {
    let shown = |lang| {
        rulec::i18n::with(lang, || {
            let ds = rulec::check_source(src, path);
            let lines: Vec<String> = src.lines().map(str::to_string).collect();
            ds.iter().map(|d| rulec::diag::render(d, &lines)).collect::<Vec<_>>().join("\n")
        })
    };
    let codes = rulec::i18n::with(Lang::En, || rulec::check_source(src, path).iter().map(|d| d.code).collect());
    (codes, shown(Lang::En), shown(Lang::Ja))
}

/// The fix E140 carries: the range that holds what the output comes to.
fn fix(src: &str) -> Option<String> {
    rulec::check_source(src, "t.rule").iter().find(|d| d.code == "E140").and_then(|d| d.fix.text.clone())
}

/// The certificate's `ranges`, as `name → [lo, hi]`.
fn ranges(dir: &Path, file: &str) -> rulec::json::Json {
    certificate(dir, file).get("ranges").cloned().unwrap_or_else(|| panic!("no ranges in the certificate of {file}"))
}

/// The certificate of `file`.
fn certificate(dir: &Path, file: &str) -> rulec::json::Json {
    let (code, out) = rulec(dir, &["certificate", file]);
    assert_eq!(code, 0, "{out}");
    rulec::json::parse(&out).unwrap()
}

fn ends(r: &rulec::json::Json, name: &str) -> Option<(String, String)> {
    let a = r.get(name)?.as_arr()?;
    let s = |j: &rulec::json::Json| j.as_str().map(str::to_string).unwrap_or_else(|| format!("{j:?}"));
    Some((s(&a[0]), s(&a[1])))
}

fn pair(lo: &str, hi: &str) -> Option<(String, String)> {
    Some((lo.to_string(), hi.to_string()))
}

/// A fee of a tenth of the amount, rounded down to the yen: it comes to 0JPY to 1,000JPY.
fn fee(range: &str) -> String {
    format!(
        "rule fee v1\n\ninputs\n  amount : money[JPY]  range >=0JPY <=10_000JPY\n\noutputs\n  fee : money[JPY]  round down(1JPY){range}\n\nresult fee = amount * 10%\n\nexamples\n| amount  | -> fee |\n| 1000JPY | 100JPY |\n"
    )
}

/// The same rule in Japanese.
fn fee_ja(range: &str) -> String {
    format!(
        "rule 手数料(fee) v1\n\ninputs\n  金額(amount) : money[円]  range >=0円 <=10_000円\n\noutputs\n  手数料(fee) : money[円]  round down(1円){range}\n\nresult 手数料 = 金額 * 10%\n\nexamples\n| 金額  | -> 手数料 |\n| 1000円 | 100円     |\n"
    )
}

/// A fee a table decides: 500JPY up to ten, 900JPY above.
fn ship(range: &str) -> String {
    format!(
        "rule ship v1\n\ninputs\n  weight : number  range >=1 <=40\n\noutputs\n  fee : money[JPY]  round down(1JPY){range}\n\ntable by_weight\npolicy unique\n| weight | -> fee |\n| <=10   | 500JPY |\n| >10    | 900JPY |\n\nexamples\n| weight | -> fee |\n| 1      | 500JPY |\n"
    )
}

#[test]
fn a_range_on_an_output_holds_what_the_output_comes_to() {
    // what the `result` comes to, put out to the grid of its rounding
    let (codes, en, ja) = said(&fee("  range >=0JPY <=500JPY"));
    assert_eq!(codes, ["E140"], "{en}");
    assert!(en.contains("The range of the output fee does not hold the values it can take") && en.contains("it can take >=0JPY <=1000JPY"), "{en}");
    assert!(en.contains("fee is decided by the `result`"), "{en}");
    assert!(ja.contains("出力 fee の範囲が、とりうる値を含んでいません") && ja.contains("とりうる値は >=0JPY <=1000JPY です"), "{ja}");
    assert_eq!(fix(&fee("  range >=0JPY <=500JPY")).as_deref(), Some("range >=0JPY <=1000JPY"));
    let (_, _, ja) = said(&fee_ja("  range >=0円 <=500円"));
    assert!(ja.contains("出力 手数料 の範囲が、とりうる値を含んでいません") && ja.contains("とりうる値は >=0円 <=1000円 です"), "{ja}");
    // a strict end that a value it comes to sits on, and a range that holds exactly what it comes to
    assert_eq!(said(&fee("  range >0JPY <=1_000JPY")).0, ["E140"]);
    assert_eq!(said(&fee("  range >=0JPY <1_000JPY")).0, ["E140"]);
    for ok in ["  range >=0JPY <=1_000JPY", "  range >=-1JPY <1_001JPY", "  range >=0JPY", "  range <=1_000JPY", ""] {
        let (codes, en, _) = said(&fee(ok));
        assert!(codes.is_empty(), "{ok}: {en}");
    }
    // the rows of a table that decides the output
    let (codes, en, ja) = said(&ship("  range >=500JPY <=800JPY"));
    assert_eq!(codes, ["E140"], "{en}");
    assert!(en.contains("it can take >=500JPY <=900JPY") && en.contains("fee takes the values of what decides it"), "{en}");
    assert!(ja.contains("とりうる値は >=500JPY <=900JPY です"), "{ja}");
    assert!(said(&ship("  range >=0JPY <=1_000JPY")).0.is_empty());
}

#[test]
fn a_range_that_cannot_be_checked_is_e140_too() {
    // an input with no range: nothing says what the fee comes to
    let open = fee("  range >=0JPY <=500JPY").replace("amount : money[JPY]  range >=0JPY <=10_000JPY", "amount : money[JPY]");
    let (codes, en, ja) = said(&open);
    assert_eq!(codes, ["E140"], "{en}");
    assert!(en.contains("The range of the output fee cannot be checked") && en.contains("rulec cannot work out what fee takes from its `result`"), "{en}");
    assert!(en.contains("Give the inputs it reads a range, or take this range off."), "{en}");
    assert!(ja.contains("出力 fee の範囲を確かめられません") && ja.contains("`result` の式から fee のとる値の範囲を求められませんでした"), "{ja}");
    assert_eq!(fix(&open), None);
    // with no range written, the output with no range to take is no finding, as before
    assert!(said(&fee("").replace("amount : money[JPY]  range >=0JPY <=10_000JPY", "amount : money[JPY]")).0.is_empty());
    // an output that is no number: the range is not taken on trust
    let tier = "rule tier v1\n\nenum level = basic < silver < gold\n\ninputs\n  spent : money[JPY]  range >=0JPY <=100_000JPY\n\noutputs\n  tier : level  range >=basic <=silver\n\ntable by_spent\npolicy unique\n| spent                   | -> tier |\n| <10_000JPY              | basic   |\n| >=10_000JPY <50_000JPY  | silver  |\n| >=50_000JPY             | gold    |\n\nexamples\n| spent | -> tier |\n| 0JPY  | basic   |\n";
    let (codes, en, ja) = said(tier);
    assert_eq!(codes, ["E140"], "{en}");
    assert!(en.contains("Only an output of numbers, money, quantities or rates has its range checked, and tier is level.") && en.contains("Take this range off."), "{en}");
    assert!(!en.contains("Give the inputs it reads a range"), "{en}");
    assert!(ja.contains("範囲を確かめられるのは、数、お金、量、率の出力だけです。tier は level です。"), "{ja}");
}

/// What other checks say of a range on an output's line is said by them alone: a range with no
/// value in it (E061), a bound that is no value of the type (E103), a koyomi date (E065) and a
/// drawn value (E070) on an output. An `allocate` that E117 turns away leaves what the output
/// comes to unknown, and E117 is the reason: the range is not said to be one that cannot be
/// checked.
#[test]
fn the_other_findings_of_an_output_range_stand_alone() {
    let (codes, en, _) = said(&fee("  range >=10JPY <=5JPY"));
    assert_eq!(codes, ["E061"], "{en}");
    assert!(en.contains("no value of the output lies in it"), "{en}");
    assert_eq!(said(&fee("  range >=0USD <=5USD")).0, ["E103", "E103"]);
    let (codes, en, _) = said(&fee("  range from koyomi \"terms.cal\" date payment"));
    assert_eq!(codes, ["E065"], "{en}");
    let (codes, en, _) = said(&fee("  random"));
    assert_eq!(codes, ["E070"], "{en}");
    let split = std::fs::read_to_string(root().join("tests/corpus/pro_rata_allocation.rule")).unwrap();
    assert!(split.contains("range >=0JPY <=1_000_000JPY\n\nderive") && split.contains("constraint upto <= base\n"));
    let (codes, en, _) = said(&split.replace("constraint upto <= base\n", ""));
    assert_eq!(codes, ["E117", "E117"], "{en}");
}

/// A word an output's line does not read is E047, as one on an input's line is; and so is a word
/// of another kind of line: `contract_only` on an output, `round` on a `derive`, a `range` after
/// a `from`, anything after a `shape`.
#[test]
fn a_word_a_line_does_not_read_is_e047() {
    let (codes, en, ja) = said(&fee("  range >=0JPY <=1_000JPY incl_tax"));
    assert_eq!(codes, ["E047"], "{en}");
    assert!(en.contains("An output's line holds `range` and `round`, and nothing else."), "{en}");
    assert!(ja.contains("出力の行に置けるのは `range` と `round` だけです。"), "{ja}");
    let (codes, en, _) = said(&fee("  contract_only"));
    assert_eq!(codes, ["E047"], "{en}");
    // a `round` on a derive, written after its range or on the next line
    let derive = "rule d v1\n\ninputs\n  amount : money[JPY]  range >=0JPY <=10_000JPY\n\noutputs\n  fee : money[JPY]  round down(1JPY)\n\nderive tenth : money[JPY] = amount * 10%  range >=0JPY <=1_000JPY{tail}\n\nresult fee = down(tenth, 1JPY)\n\nexamples\n| amount  | -> fee |\n| 1000JPY | 100JPY |\n";
    assert!(said(&derive.replace("{tail}", "")).0.is_empty());
    let (codes, en, ja) = said(&derive.replace("{tail}", "  round down(1JPY)"));
    assert_eq!(codes, ["E047"], "{en}");
    assert!(en.contains("A `derive` line holds `range`, and nothing else."), "{en}");
    assert!(ja.contains("`derive` の行に置けるのは `range` だけです。"), "{ja}");
    // an input's line still takes all three of its words
    let marks = fee("").replace("amount : money[JPY]  range >=0JPY <=10_000JPY", "amount : money[JPY]  range >=0JPY <=10_000JPY  contract_only");
    assert!(!said(&marks).0.contains(&"E047"), "{:?}", said(&marks));
    // a range after the path of an input's `from`, which left the input with no range at all
    let read = std::fs::read_to_string(root().join("tests/collection_path/order_lines_read.rule")).unwrap();
    let at = "tests/collection_path/variant.rule";
    assert!(said_at(&read, at).0.is_empty(), "{:?}", said_at(&read, at));
    for (from, after) in [
        ("  pieces    : number  range >=0 <=1980   from sum order.lines of quantity\n", "  pieces    : number  from sum order.lines of quantity  range >=0 <=1980\n"),
        ("  services  : number  range >=0 <=20     from count order.lines where category = service\n", "  services  : number  from count order.lines  range >=0 <=20\n"),
    ] {
        assert!(read.contains(from), "{from}");
        let (codes, en, ja) = said_at(&read.replace(from, after), at);
        assert_eq!(codes, ["E047"], "{en}");
        assert!(en.contains("What follows `from` is its path, then `of`, `where` or `empty`, and nothing else; an input's range and marks go before `from`."), "{en}");
        assert!(ja.contains("`from` の後ろに置けるのは、パスと `of`、`where`、`empty` だけです。"), "{ja}");
    }
    // a word after a `shape`, which says so of the line it is on
    let shape = "shape order = jsonschema \"orders.schema.json\" \"#/$defs/Order\"\n";
    assert!(read.contains(shape));
    let (codes, en, _) = said_at(&read.replace(shape, &shape.replace('\n', "  strict\n")), at);
    assert_eq!(codes, ["E047"], "{en}");
    assert!(en.contains("A `shape` line holds nothing after its declaration.") && !en.contains("A range is"), "{en}");
}

#[test]
fn the_range_written_is_the_outputs_range() {
    let t = TempDir::new("output-range");
    let dir = t.path();
    let write = |name: &str, src: &str| std::fs::write(dir.join(name), src).unwrap();
    // written wider than what the fee comes to: the range written; one end written: that end, and
    // what the fee comes to at the other
    write("wide.rule", &fee("  range >=0JPY <=2_000JPY"));
    write("low.rule", &fee("  range >=0JPY"));
    write("high.rule", &fee("  range <=5_000JPY"));
    write("none.rule", &fee(""));
    assert_eq!(ends(&ranges(dir, "wide.rule"), "fee"), pair("0", "2000"));
    assert_eq!(ends(&ranges(dir, "low.rule"), "fee"), pair("0", "1000"));
    assert_eq!(ends(&ranges(dir, "high.rule"), "fee"), pair("0", "5000"));
    // the schema and the port carry the range written, as an input's
    let (_, schema) = rulec(dir, &["schema", "wide.rule"]);
    let j = rulec::json::parse(&schema).unwrap();
    let out = j.get("properties").and_then(|p| p.get("out")).and_then(|o| o.get("properties")).and_then(|p| p.get("fee")).unwrap_or_else(|| panic!("{schema}"));
    assert_eq!((out.get("minimum").and_then(|x| x.as_int()), out.get("maximum").and_then(|x| x.as_int())), (Some(0), Some(2000)), "{schema}");
    let e = rulec::ports::Engine::new();
    let facts = e.facts(&dir.join("wide.rule")).unwrap_or_else(|x| panic!("{x:?}"));
    let col = facts.outputs.iter().find(|c| c.name == "fee").unwrap();
    assert!(matches!(col.ty, ColumnType::Num { min: Some(0), max: Some(2000), .. }), "{:?}", col.ty);
    match e.output_values(&dir.join("wide.rule"), "fee").unwrap() {
        Found::Value(v) => assert_eq!((v.min, v.max), (Some(0), Some(2000))),
        Found::Undecided(t) => panic!("{t:?}"),
    }
    // the page says it, beside the output
    let (_, page) = rulec(dir, &["doc", "wide.rule"]);
    assert!(page.lines().any(|l| l.starts_with("| fee |") && l.contains("range 0JPY to 2000JPY")), "{page}");
    let (_, page) = rulec(dir, &["doc", "none.rule"]);
    assert!(!page.lines().any(|l| l.starts_with("| fee |") && l.contains("range")), "{page}");
}

/// With no range written, the output a `result` decides has the range of what it comes to where
/// the other languages read it — the certificate and the port — and nowhere the generated code
/// reads: its schema, its facts and the code are as they were.
#[test]
fn the_output_a_result_decides_has_the_range_it_comes_to() {
    let t = TempDir::new("result-range");
    let dir = t.path();
    std::fs::write(dir.join("fee.rule"), fee("")).unwrap();
    assert_eq!(ends(&ranges(dir, "fee.rule"), "fee"), pair("0", "1000"));
    let e = rulec::ports::Engine::new();
    match e.output_values(&dir.join("fee.rule"), "fee").unwrap() {
        Found::Value(v) => assert_eq!((v.min, v.max), (Some(0), Some(1000))),
        Found::Undecided(t) => panic!("{t:?}"),
    }
    let facts = e.facts(&dir.join("fee.rule")).unwrap_or_else(|x| panic!("{x:?}"));
    let col = facts.outputs.iter().find(|c| c.name == "fee").unwrap();
    assert!(matches!(col.ty, ColumnType::Num { min: None, max: None, .. }), "{:?}", col.ty);
    let (_, schema) = rulec(dir, &["schema", "fee.rule"]);
    let j = rulec::json::parse(&schema).unwrap();
    let out = j.get("properties").and_then(|p| p.get("out")).and_then(|o| o.get("properties")).and_then(|p| p.get("fee")).unwrap_or_else(|| panic!("{schema}"));
    assert!(out.get("minimum").is_none() && out.get("maximum").is_none(), "{schema}");
}

/// With no range written, an output a `define` of its name or a table decides carries what decides
/// it put out to the grid of its rounding (§15.269). A card fee of 3.49% and 49 cents comes to
/// 49.0349 to 83.9 cents, and the rule answers it rounded half up to the cent: 49 to 84. The port
/// and the certificate's `output_ranges` carry 49 to 84, which holds every answer — 84 at 1,000
/// cents, as the example says — where the certificate stated 49.0349 (49 for the table) to 83.9 and
/// the port carried 49 to 83. `ranges` keeps what decides it.
#[test]
fn an_output_with_no_range_written_carries_what_decides_it_put_out_to_its_grid() {
    let t = TempDir::new("define-range");
    let dir = t.path();
    let by_define = "rule card_fee v1\n\ninputs\n  amount : money[USDc]  range >=1USDc <=1000USDc\n\noutputs\n  fee : money[USDc]  round half_up(1USDc)\n\ndefine fee : money[USDc] = amount × 3.49% + 49USDc\n\nexamples\n| amount   | -> fee |\n| 1000USDc | 84USDc |\n| 1USDc    | 49USDc |\n";
    let by_table = "rule card_fee v1\n\ninputs\n  amount : money[USDc]  range >=1USDc <=1000USDc\n  waived : bool\n\noutputs\n  fee : money[USDc]  round half_up(1USDc)\n\ndefine charged : money[USDc] = amount × 3.49% + 49USDc\n\ntable fees\npolicy unique\n| waived | -> fee   |\n| false  | charged  |\n| true   | 49USDc   |\n\nexamples\n| amount   | waived | -> fee |\n| 1000USDc | false  | 84USDc |\n";
    // What decides it: the `define`, and the rows of the table, one of which writes 49 cents.
    for (what, src, decided) in [("define", by_define, "490349/10000"), ("table", by_table, "49")] {
        let (codes, en, _) = said(src);
        assert!(codes.is_empty(), "{what}: {en}");
        std::fs::write(dir.join("fee.rule"), src).unwrap();
        let cert = certificate(dir, "fee.rule");
        assert_eq!(ends(cert.get("output_ranges").unwrap(), "fee"), pair("49", "84"), "{what}");
        assert_eq!(ends(cert.get("ranges").unwrap(), "fee"), pair(decided, "839/10"), "{what}");
        let e = rulec::ports::Engine::new();
        match e.output_values(&dir.join("fee.rule"), "fee").unwrap() {
            Found::Value(v) => assert_eq!((v.min, v.max), (Some(49), Some(84)), "{what}"),
            Found::Undecided(t) => panic!("{what}: {t:?}"),
        }
    }
}

/// Two shares of one amount over one whole, the first at a running total the `constraint` lines
/// keep at or below the second's: what is left between them is never below zero, and never above
/// the amount (`floor(T × C ÷ S)` rises with C). The proportional split of the corpus, and the
/// rule kaname's discount_split calls. Without the order of the running totals, with another
/// whole, or with another amount, nothing is known of the order, and the difference reaches below
/// zero as it did.
#[test]
fn two_shares_at_running_totals_in_order_leave_no_less_than_zero() {
    let corpus = root().join("tests/corpus");
    let r = ranges(&corpus, "pro_rata_allocation.rule");
    for name in ["gap", "allocated", "share"] {
        assert_eq!(ends(&r, name), pair("0", "1000000"), "{name}");
    }
    let r = ranges(&corpus, "比例配分.rule");
    assert_eq!(ends(&r, "配分額"), pair("0", "1000000"));
    let kaname = root().join("../kaname/examples/discount_split/rules");
    assert_eq!(ends(&ranges(&kaname, "discount_share.rule"), "share"), pair("0", "10000000"));
    assert_eq!(ends(&ranges(&kaname, "値引きの配分.rule"), "配分"), pair("0", "10000000"));

    // the rule without its source and its examples, which the variants below would not keep
    let whole = std::fs::read_to_string(kaname.join("discount_share.rule")).unwrap();
    let src = whole[..whole.find("\n# $10.00 off").unwrap()]
        .replace("  @va", "")
        .replace("source va = file \"sources/23vac10-210-250.md\" sha256:73c0c7337d586218  # the Virginia Administrative Code\n\n", "");
    assert!(!src.contains("@va") && !src.contains("\nexamples"), "{src}");
    let t = TempDir::new("shares-in-order");
    let dir = t.path();
    let cases = [
        ("same.rule", src.clone(), ("0", "10000000")),
        ("unordered.rule", src.replace("constraint before <= upto\n", "constraint before <= total\n"), ("-10000000", "10000000")),
        (
            "two_wholes.rule",
            src.replace("  total    : money[USDc]  range >=1USDc <=100_000_000USDc\n", "  total    : money[USDc]  range >=1USDc <=100_000_000USDc\n  other    : money[USDc]  range >=1USDc <=100_000_000USDc\n")
                .replace("constraint upto <= total\n", "constraint upto <= total\nconstraint upto <= other\n")
                .replace("allocate(discount, before, total)", "allocate(discount, before, other)"),
            ("-10000000", "10000000"),
        ),
        (
            "two_amounts.rule",
            src.replace("  before   : money[USDc]", "  more     : money[USDc]  range >=0USDc <=10_000_000USDc\n  before   : money[USDc]")
                .replace("allocate(discount, before, total)", "allocate(more, before, total)"),
            ("-10000000", "10000000"),
        ),
    ];
    for (name, text, (lo, hi)) in cases {
        std::fs::write(dir.join(name), &text).unwrap();
        let (code, out) = rulec(dir, &["check", name]);
        assert_eq!(code, 0, "{name}: {out}");
        assert_eq!(ends(&ranges(dir, name), "share"), pair(lo, hi), "{name}");
    }
    // the range of the difference is the one the output may write
    std::fs::write(dir.join("written.rule"), src.replace("  share : money[USDc]  round down(1USDc)", "  share : money[USDc]  round down(1USDc)  range >=0USDc <=10_000_000USDc")).unwrap();
    let (code, out) = rulec(dir, &["check", "written.rule"]);
    assert_eq!(code, 0, "{out}");
}

/// `diff` says an output's range moved where either version writes one, as it says an input's.
#[test]
fn diff_says_an_outputs_range_moved() {
    let t = TempDir::new("output-range-diff");
    let dir = t.path();
    std::fs::write(dir.join("a.rule"), fee("  range >=0JPY <=1_000JPY")).unwrap();
    std::fs::write(dir.join("b.rule"), fee("  range >=0JPY <=2_000JPY")).unwrap();
    std::fs::write(dir.join("c.rule"), fee("")).unwrap();
    let moved = |a: &str, b: &str| {
        let (_, out) = rulec(dir, &["diff", a, b, "--format", "json"]);
        let j = rulec::json::parse(&out).unwrap_or_else(|_| panic!("{out}"));
        j.get("domain")
            .and_then(|d| d.as_arr())
            .map(|d| {
                d.iter()
                    .filter(|x| x.get("what").and_then(|w| w.as_str()) == Some("output_range"))
                    .map(|x| (x.get("old").and_then(|v| v.as_str()).unwrap_or("").to_string(), x.get("new").and_then(|v| v.as_str()).unwrap_or("").to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    assert_eq!(moved("a.rule", "b.rule"), [("0..1000".to_string(), "0..2000".to_string())]);
    // written on one side only: the range the other side has, what the fee comes to
    assert_eq!(moved("c.rule", "b.rule"), [("0..1000".to_string(), "0..2000".to_string())]);
    assert!(moved("a.rule", "c.rule").is_empty());
    assert!(moved("a.rule", "a.rule").is_empty());
    let (_, text) = rulec(dir, &["diff", "a.rule", "b.rule"]);
    assert!(text.contains("an output changed range"), "{text}");
}
