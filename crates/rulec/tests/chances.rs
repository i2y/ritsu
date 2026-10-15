//! Inputs drawn at random, and how likely each answer is (§15.224 to §15.226): the mark `random`,
//! the `chances` table, and the pick by weight (`take_weighted`, `draw`).
//!
//! What the corpus-wide suites already hold of the rules of the corpus that draw — their vectors and
//! their functions in every language, the function that counts the chances among them
//! (tests/threeway.rs), their certificates in both re-checkers (tests/cert.rs, tests/lean.rs), the
//! mutants' codes (tests/m0.rs), the goldens, and the twins' claims (tests/twins.rs) — is not
//! repeated here. These are the rest: the words read by their place, the shapes E070 and E073 stop, a
//! claim broken and one past its budget, the counts W131 and E108 read, what `rulec gen` and
//! `rulec api` say, what the vectors hold the functions to, a certificate that lies, and the pages.

use std::path::{Path, PathBuf};
use std::process::Command;
use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready, skip};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("cannot start rulec");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn run(args: &[&str]) -> (i32, String, String) {
    run_in(&root(), args)
}

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn json(text: &str) -> rulec::json::Json {
    rulec::json::parse(text.trim()).unwrap_or_else(|e| panic!("not JSON: {e}\n{text}"))
}

/// The diagnostics `check` prints for a rule written into `dir`, as JSON, one per line.
fn diags_of(dir: &Path, name: &str, src: &str, more: &[&str]) -> Vec<rulec::json::Json> {
    std::fs::write(dir.join(name), src).unwrap();
    let mut args = vec!["check", name, "--format", "json"];
    args.extend_from_slice(more);
    let (_, out, _) = run_in(dir, &args);
    out.lines().filter_map(|l| rulec::json::parse(l).ok()).filter(|j| j.get("code").is_some()).collect()
}

/// The codes `check` prints for a rule written into `dir`, each once, sorted.
fn codes_of(dir: &Path, name: &str, src: &str) -> Vec<String> {
    let mut v: Vec<String> = diags_of(dir, name, src, &[]).iter().filter_map(|j| j.get("code").and_then(|c| c.as_str()).map(String::from)).collect();
    v.sort();
    v.dedup();
    v
}

const SAMPLING: &str = "tests/corpus/invoice_sampling.rule";
const LOTTERY: &str = "tests/corpus/prize_lottery.rule";
const ROUTING: &str = "tests/corpus/case_routing.rule";
const BOX: &str = "tests/corpus/reward_box.rule";
const CONTEST: &str = "tests/corpus/contested_roll.rule";

/// The rules of the corpus that draw, in English and in Japanese, pass with nothing said: every claim
/// holds, and a random input that only the `draw` line reads is used.
#[test]
fn the_corpus_that_draws_passes_with_nothing_said() {
    for rel in [
        SAMPLING,
        LOTTERY,
        ROUTING,
        BOX,
        CONTEST,
        "tests/corpus/伝票の抜き取り.rule",
        "tests/corpus/くじの景品.rule",
        "tests/corpus/案件の振り分け.rule",
        "tests/corpus/抽選の特典.rule",
        "tests/corpus/乱数で決める一回.rule",
    ] {
        let (c, out, _) = run(&["check", rel]);
        assert_eq!(c, 0, "{rel}:\n{out}");
        assert_eq!(out.trim(), format!("ok {rel}"), "{rel} says more than ok:\n{out}");
    }
}

/// None of the four words is reserved (§15.157): `random` is read at the end of an input's line,
/// `chances` at the head of a line, `take_weighted` after the `->` of an arm, and `draw` as
/// `draw -> <name>` alone. Each may be a name, and `draw -> next` is the arm of a verdict called
/// `draw`.
#[test]
fn the_new_words_are_read_by_their_place() {
    let t = TempDir::new("chances-words");
    let src = "rule words v1\n\nenum step = draw | random | chances | take_weighted\n\nelements turns\n  kind : step\n\ninputs\n  random  : bool\n  chances : number  range >=0 <=9\n\noutputs\n  take_weighted : number  round down(1)\n\n\
               table chance_table\npolicy unique\n| kind          | -> verdict : step |\n| draw          | draw              |\n| random        | random            |\n| chances       | chances           |\n| take_weighted | take_weighted     |\n\n\
               fold verdict over turns -> picked : number\n  draw          -> next\n  random        -> take_first chances\n  chances       -> next\n  take_weighted -> stop\n  empty         -> 0\n  exhausted     -> held\n\n\
               table out\npolicy unique\n| random | -> take_weighted |\n| true   | picked           |\n| false  | 0                |\n";
    assert!(codes_of(t.path(), "words.rule", src).is_empty(), "a name spelled like one of the words is not read as a name");
    // Nothing in it is drawn, so there is nothing to count.
    let (_, api, _) = run_in(t.path(), &["api", "words.rule"]);
    assert!(json(&api).get("chances").is_none(), "{api}");
}

/// What cannot be drawn, and a `chances` table that is not shaped right, stop the check with E070 —
/// each form on a small rule that passes as it is.
#[test]
fn what_cannot_be_drawn_or_claimed_is_e070() {
    let t = TempDir::new("chances-e070");
    let base = "rule t v1\n\ninputs\n  roll : number  range >=0 <=9  random\n  vip  : bool\n\noutputs\n  win : bool\n\ntable j\npolicy unique\n| roll | vip | -> win |\n| <3   | -   | true   |\n| >=3  | -   | false  |\n\nchances\n| vip | -> win | chance |\n| -   | true   | 3/10   |\n";
    assert!(codes_of(t.path(), "base.rule", base).is_empty());
    let forms: &[(&str, &str, &str)] = &[
        ("a number with no range", "  roll : number  range >=0 <=9  random", "  roll : number  random"),
        ("an input that may be absent", "  roll : number  range >=0 <=9  random", "  roll : number?  range >=0 <=9  random"),
        ("an output", "  win : bool\n", "  win : bool  random\n"),
        ("something after `chances`", "chances\n", "chances now\n"),
        ("no table under it", "| vip | -> win | chance |\n| -   | true   | 3/10   |\n", ""),
        ("a header with no `->`", "| vip | -> win | chance |", "| vip | win | chance |"),
        ("a last column that is not `chance`", "| vip | -> win | chance |", "| vip | -> win | odds |"),
        ("no output to claim", "| vip | -> win | chance |\n| -   | true   | 3/10   |", "| vip | -> chance |\n| -   | 3/10   |"),
        ("no rows", "| -   | true   | 3/10   |\n", ""),
        ("an input that is not random left out", "| vip | -> win | chance |\n| -   | true   | 3/10   |", "| -> win | chance |\n| true   | 3/10   |"),
        ("a random input as a column", "| vip | -> win | chance |\n| -   | true   | 3/10   |", "| roll | vip | -> win | chance |\n| -    | -   | true   | 3/10   |"),
        ("an output on the left", "| vip | -> win | chance |\n| -   | true   | 3/10   |", "| vip | win | -> win | chance |\n| -   | -   | true   | 3/10   |"),
        ("an input on the right", "| vip | -> win | chance |\n| -   | true   | 3/10   |", "| vip | -> win | vip | chance |\n| -   | true   | -   | 3/10   |"),
        ("a chance past one", "| -   | true   | 3/10   |", "| -   | true   | 3/2    |"),
        ("a word for a chance", "| -   | true   | 3/10   |", "| -   | true   | most   |"),
        ("a division by zero", "| -   | true   | 3/10   |", "| -   | true   | 3/0    |"),
        ("claims with nothing drawn", "  roll : number  range >=0 <=9  random", "  roll : number  range >=0 <=9"),
    ];
    for (what, from, to) in forms {
        assert!(base.contains(from), "{what}: the base has no {from:?}");
        let src = base.replacen(from, to, 1);
        assert_eq!(codes_of(t.path(), "t.rule", &src), ["E070"], "{what}:\n{src}");
    }
    // A field of an element is drawn once per element, so it cannot be random either.
    let field = "rule t v1\n\nelements xs\n  ok : bool  random\n\ncount many over xs where ok  range >=0 <=5\n\ninputs\n  vip : bool\n\noutputs\n  r : bool\n\ntable j\npolicy unique\n| many | vip | -> r  |\n| 0    | -   | true  |\n| >=1  | -   | false |\n";
    assert_eq!(codes_of(t.path(), "field.rule", field), ["E070"], "a random field passes");
    assert!(codes_of(t.path(), "plain.rule", &field.replace("  ok : bool  random", "  ok : bool")).is_empty());
}

/// What `from` reads out of the caller's object (§15.223) is the caller's value, not a draw: `random`
/// on such an input is E070, written before the `from` or after it, and never stepped over with the
/// tail of the `from`.
#[test]
fn a_value_read_out_of_the_callers_object_is_not_drawn() {
    let t = TempDir::new("chances-from");
    std::fs::create_dir_all(t.path().join("contracts")).unwrap();
    std::fs::copy(root().join("tests/corpus/contracts/order_lines.schema.json"), t.path().join("contracts/order_lines.schema.json")).unwrap();
    let src = std::fs::read_to_string(root().join("tests/corpus/order_handling_fee.rule")).unwrap();
    let line = "range >=0USD <=100_000USD  from sum order.lines of amount_usd\n";
    assert!(src.contains(line));
    assert!(codes_of(t.path(), "plain.rule", &src).is_empty());
    for (what, to) in [
        ("after the `from`", "range >=0USD <=100_000USD  from sum order.lines of amount_usd  random\n"),
        ("before the `from`", "range >=0USD <=100_000USD  random  from sum order.lines of amount_usd\n"),
    ] {
        let ds = diags_of(t.path(), "drawn.rule", &src.replacen(line, to, 1), &[]);
        let codes: Vec<&str> = ds.iter().filter_map(|d| d.get("code").and_then(|c| c.as_str())).collect();
        assert_eq!(codes, ["E070"], "{what}");
        assert!(rulec::json::unparse(&ds[0]).contains("total is taken out of the caller's object"), "{what}: {:?}", ds[0]);
    }
    // A field of an element that is read out of its element: the mark is not a path.
    let field = src.replacen("  from quantity\n", "  from quantity  random\n", 1);
    assert!(!codes_of(t.path(), "field.rule", &field).is_empty(), "a mark after a field's path is stepped over");
}

/// A pick by weight that is not shaped right stops the check with E073: each form on a small rule
/// that passes as it is.
#[test]
fn a_misshapen_pick_by_weight_is_e073() {
    let t = TempDir::new("chances-e073");
    let base = "rule t v1\n\nenum v = skip | pick\n\nelements os\n  ix : number  range >=1 <=9\n  ok : bool\n  w  : number  range >=0 <=10\n\ninputs\n  roll : number  range >=0 <=99  random\n  cap  : number  range >=0 <=5\n\noutputs\n  p : number  round down(1)\n\n\
                table j\npolicy unique\n| ok    | cap | -> v : v |\n| true  | -   | pick     |\n| false | -   | skip     |\n\nfold v over os\n  skip      -> next\n  pick      -> take_weighted ix by w\n  draw      -> roll\n  empty     -> 0\n  exhausted -> held\n";
    assert!(codes_of(t.path(), "base.rule", base).is_empty());
    let forms: &[(&str, &str, &str)] = &[
        ("no `by`", "  pick      -> take_weighted ix by w", "  pick      -> take_weighted ix"),
        ("two `draw` lines", "  draw      -> roll\n", "  draw      -> roll\n  draw      -> roll\n"),
        ("a `draw` line with no pick by weight", "  pick      -> take_weighted ix by w", "  pick      -> take_first ix"),
        ("another kind of arm", "  skip      -> next", "  skip      -> stop"),
        ("a weight that is not a number", "  w  : number  range >=0 <=10", "  w  : money[JPY]  range >=0JPY <=10JPY"),
        ("a weight that can be negative", "  w  : number  range >=0 <=10", "  w  : number  range >=-1 <=10"),
        ("no `draw` line", "  draw      -> roll\n", ""),
        ("a drawn value that is not random", "  draw      -> roll\n", "  draw      -> cap\n"),
        ("a drawn value from 1", "  roll : number  range >=0 <=99  random", "  roll : number  range >=1 <=99  random"),
        ("a drawn value that is not declared", "  draw      -> roll\n", "  draw      -> dice\n"),
    ];
    for (what, from, to) in forms {
        assert!(base.contains(from), "{what}: the base has no {from:?}");
        let src = base.replacen(from, to, 1);
        assert!(codes_of(t.path(), "t.rule", &src).contains(&"E073".to_string()), "{what}:\n{src}");
    }
    let two = format!("{base}\nfold v over os -> q : number\n  skip      -> next\n  pick      -> take_weighted ix by w\n  draw      -> roll\n  empty     -> 0\n  exhausted -> held\n");
    assert!(codes_of(t.path(), "two.rule", &two).contains(&"E073".to_string()), "two picks by weight pass");
}

/// A claim that does not hold names the combination it fails at, with the chance there, in the
/// witness as well as in the note; a rule with nothing that is not random says the chance alone.
#[test]
fn a_broken_claim_is_e133_with_the_combination_and_the_chance() {
    let t = TempDir::new("chances-e133");
    let src = std::fs::read_to_string(root().join(LOTTERY)).unwrap().replacen("| false    | third    | 5%     |", "| false    | third    | 6%     |", 1);
    let ds = diags_of(t.path(), "lottery.rule", &src, &[]);
    let d = ds.iter().find(|d| d.get("code").and_then(|c| c.as_str()) == Some("E133")).expect("no E133");
    let notes = rulec::json::unparse(d.get("notes").unwrap());
    assert!(notes.contains("With campaign = false, the chance of prize = third is 1/20 (5.0%)."), "{notes}");
    assert_eq!(d.get("witness").and_then(|w| w.get("inputs")).and_then(|i| i.get("campaign")).map(rulec::json::unparse).as_deref(), Some("false"));
    let alone = "rule t v1\n\ninputs\n  roll : number  range >=0 <=9  random\n\noutputs\n  r : bool\n\ntable j\npolicy unique\n| roll | -> r  |\n| <3   | true  |\n| >=3  | false |\n\nchances\n| -> r | chance |\n| true | 1/3    |\n";
    let ds = diags_of(t.path(), "alone.rule", alone, &[]);
    let notes = rulec::json::unparse(ds[0].get("notes").unwrap());
    assert!(notes.contains("The chance of r = true is 3/10 (30.0%)."), "{notes}");
    // A comparison passes or fails as written: `>=1/4 <=1/2` holds of 3/10, `>1/3` does not.
    assert!(codes_of(t.path(), "between.rule", &alone.replace("| true | 1/3    |", "| true | >=1/4 <=1/2 |")).is_empty());
    assert_eq!(codes_of(t.path(), "above.rule", &alone.replace("| true | 1/3    |", "| true | >1/3 |")), ["E133"]);
    // A rate is a chance too.
    assert!(codes_of(t.path(), "rate.rule", &alone.replace("| true | 1/3    |", "| true | 30% |")).is_empty());
}

/// The points a claim runs one by one are held to its share of `--budget`: past it the claim is
/// W129 and neither holds nor fails, and a budget that takes them decides it.
#[test]
fn a_claim_past_its_budget_is_w129_until_the_budget_takes_it() {
    let t = TempDir::new("chances-w129");
    // The roll and the strength are read together, so every pair of values is run.
    let src = "rule t v1\n\ninputs\n  roll : number  range >=0 <=99  random\n  a    : number  range >=0 <=99\n\noutputs\n  r : bool\n\nderive d : number = roll - a  range >=-99 <=99\n\n\
               table j\npolicy unique\n| d   | -> r  |\n| <0  | true  |\n| >=0 | false |\n\nchances\n| a   | -> r | chance |\n| -   | true | <=99/100 |\n| 0   | true | 0        |\n| 50  | true | 1/2      |\n";
    let ds = diags_of(t.path(), "t.rule", src, &["--budget", "250000"]);
    let w: Vec<&rulec::json::Json> = ds.iter().filter(|d| d.get("code").and_then(|c| c.as_str()) == Some("W129")).collect();
    assert_eq!(w.len(), 1, "{ds:?}");
    assert!(rulec::json::unparse(w[0]).contains("the 100 combinations of the inputs that are not random, times the 100 points of the random inputs, go past the budget (5000 points)"), "{:?}", w[0]);
    assert!(diags_of(t.path(), "t.rule", src, &[]).is_empty(), "the default budget does not decide it");
}

/// W131 counts the points the function runs on every call, and a pick by weight whose drawn value
/// only `draw` reads is counted from its weights, however wide the draw; E108 stops a rule whose
/// points a count of ways cannot hold.
#[test]
fn the_points_are_counted_for_w131_and_e108() {
    let t = TempDir::new("chances-points");
    let (c, out, _) = run(&["check", "tests/mutants/m_w131.rule"]);
    assert_eq!(c, 0);
    assert!(out.contains("warning[W131]: The chances function runs the rule 10000000 times on every call"), "{out}");
    let wide = std::fs::read_to_string(root().join(ROUTING)).unwrap().replacen("range >=0 <=9999  random", "range >=0 <=99_999_999  random", 1);
    assert!(codes_of(t.path(), "wide.rule", &wide).is_empty(), "a pick by weight is said to run its points");
    let (c, api, _) = run_in(t.path(), &["api", "wide.rule"]);
    assert_eq!(c, 0);
    let ch = json(&api);
    let ch = ch.get("chances").unwrap();
    assert_eq!(ch.get("counted").and_then(|x| x.as_str()), Some("weights"), "{api}");
    let huge = "rule t v1\n\ninputs\n  a : number  range >=0 <=9999999999  random\n  b : number  range >=0 <=9999999999  random\n\noutputs\n  r : bool\n\ntable j\npolicy unique\n| a   | b | -> r  |\n| <5  | - | true  |\n| >=5 | - | false |\n";
    assert_eq!(codes_of(t.path(), "huge.rule", huge), ["E108"]);
}

/// The pick by weight in the reference evaluator: a drawn value past the total of the weights is
/// refused, as the generated code refuses it once the walk is over, every edge of the shares is a
/// case, and weights that add up to 0 answer `exhausted`.
#[test]
fn a_pick_by_weight_refuses_a_draw_past_the_weights() {
    let t = TempDir::new("chances-refused");
    let (c, _, e) = run(&["gen", ROUTING, "--out", t.path().to_str().unwrap()]);
    assert_eq!(c, 0, "{e}");
    let refused = std::fs::read_to_string(t.path().join("vectors/case_routing.refused.jsonl")).unwrap();
    assert!(
        refused.lines().any(|l| l.contains("\"refused\":\"input\"") && l.contains("\"error\":\"roll is not below the total of the weights\"")),
        "no draw past the weights is refused:\n{refused}"
    );
    let (c, out, _) = run(&["vectors", ROUTING]);
    assert_eq!(c, 0);
    assert!(out.contains("(an edge of the weights)"), "{out}");
    let lines: Vec<rulec::json::Json> = out.lines().filter(|l| !l.trim().is_empty()).map(json).collect();
    let zero = lines.iter().find(|j| {
        let Some(rulec::json::Json::Arr(es)) = j.get("in").and_then(|i| i.get("reviewers")) else { return false };
        let one = |k: &str| es.first().and_then(|e| e.get(k)).map(rulec::json::unparse).unwrap_or_default();
        es.len() == 1 && one("present") == "true" && one("capacity") == "0"
    });
    let zero = zero.expect("no case of weights that add up to 0");
    assert_eq!(zero.get("out").and_then(|o| o.get("reviewer")).map(rulec::json::unparse).as_deref(), Some("0"));
}

/// `rulec gen` names the targets with no room for the function, writes the cases the functions are
/// held to, and `rulec api` gives the function in every language with how it counts.
#[test]
fn gen_and_api_say_where_the_functions_are() {
    let t = TempDir::new("chances-gen");
    let out = t.path().join("gen");
    let (c, said, e) = run(&["gen", LOTTERY, "--out", out.to_str().unwrap()]);
    assert_eq!(c, 0, "{e}");
    // One line for each target, as for the other functions a target has no room for.
    assert!(
        said.contains("SQL: the function that counts the chances of the answers (`prize_lottery_chances`) is not generated for this target: a query that walked the points would give up the one-to-one of its branches and the rule's rows"),
        "{said}"
    );
    assert!(
        said.contains("Wasm: the function that counts the chances of the answers (`prize_lottery_chances`) is not generated for this target: it has one entry, and no room for a second function"),
        "{said}"
    );
    let lines = std::fs::read_to_string(out.join("vectors/prize_lottery.chances.jsonl")).unwrap();
    let expected = std::fs::read_to_string(out.join("vectors/prize_lottery.chances.expected")).unwrap();
    assert_eq!(lines.lines().count(), 2, "one line for each campaign:\n{lines}");
    assert_eq!(
        expected,
        "{\"total\":1000}\n1\t{\"prize\":\"first\"}\n9\t{\"prize\":\"second\"}\n90\t{\"prize\":\"third\"}\n900\t{\"prize\":\"no_prize\"}\n\
         {\"total\":1000}\n1\t{\"prize\":\"first\"}\n9\t{\"prize\":\"second\"}\n50\t{\"prize\":\"third\"}\n940\t{\"prize\":\"no_prize\"}\n"
    );
    let py = std::fs::read_to_string(out.join("python/prize_lottery.py")).unwrap();
    assert!(py.contains("class Chances(NamedTuple):") && py.contains("def prize_lottery_chances(campaign: bool) -> Chances:"), "{py}");
    assert!(!std::fs::read_to_string(out.join("sql/prize_lottery.sql")).unwrap().contains("chances"));

    let (c, api, _) = run(&["api", LOTTERY]);
    assert_eq!(c, 0);
    let j = json(&api);
    let ch = j.get("chances").expect("no chances");
    assert_eq!(ch.get("points").map(rulec::json::unparse).as_deref(), Some("1000"));
    assert_eq!(ch.get("counted").and_then(|x| x.as_str()), Some("points"));
    let f = ch.get("functions").unwrap();
    let at = |lang: &str| f.get(lang).and_then(|x| x.get("function")).and_then(|x| x.as_str()).unwrap_or_default().to_string();
    assert_eq!(at("typescript"), "prizeLotteryChances");
    assert_eq!(at("go"), "PrizeLotteryChances");
    assert_eq!(at("java"), "PrizeLottery.prizeLotteryChances");
    assert_eq!(at("numpy"), "Rule.chances(**fixed)");
    // What `rulec api` says a target lacks is what `rulec gen` said of it.
    let without = ch.get("without").unwrap();
    assert_eq!(without.get("wasm").and_then(|x| x.as_str()), Some("it has one entry, and no room for a second function"));
    assert!(without.get("sql").is_some() && without.get("python").is_none());
    // A rule that draws nothing has no key: no other inventory changes.
    let (_, api, _) = run(&["api", "tests/corpus/member_tier_shipping.rule"]);
    assert!(json(&api).get("chances").is_none());
    // A pick by weight is counted from its weights.
    let (_, api, _) = run(&["api", ROUTING]);
    assert_eq!(json(&api).get("chances").and_then(|c| c.get("counted")).and_then(|x| x.as_str()), Some("weights"));
}

/// The functions that count the chances answer as the reference does, in the languages at hand —
/// all ten are held to it over the corpus by tests/threeway.rs.
#[test]
fn the_functions_count_the_chances_the_reference_counts() {
    if !ready(Need::Python, || have("python3"), "no python3, so nothing was run") {
        return;
    }
    let t = TempDir::new("chances-langs");
    let generated = t.path().join("gen");
    let (c, _, e) = run(&["gen", ROUTING, BOX, "--out", generated.to_str().unwrap()]);
    assert_eq!(c, 0, "{e}");
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .env("TMPDIR", tmpdir_in(&generated))
        .args(["test", generated.to_str().unwrap()])
        .output()
        .expect("cannot start rulec test");
    let said = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{said}");
    for (rule, cases) in [("case_routing", 11), ("reward_box", 20)] {
        let langs: Vec<&str> = said.lines().filter(|l| l.starts_with(&format!("ok    {rule} (")) && l.contains(", chances)")).collect();
        assert!(!langs.is_empty(), "{rule}: no language counted the chances:\n{said}");
        assert!(langs.iter().all(|l| l.ends_with(&format!("{cases} chance cases"))), "{said}");
    }
    assert!(said.contains("note: SQL: reward_box's chances function is not in this target, so none was compared"), "{said}");
    for l in said.lines().filter(|l| l.starts_with("warning")) {
        skip(l);
    }
}

/// The certificate's `chances` is held by `tools/recheck.py` to the rule's ranges, to its rows and to
/// its own arithmetic: the boxes tile the random axes, each holds the product of its lengths, the rows
/// answer in each box what the certificate says, and the hits over the points pass the claim. One that
/// lies is refused.
#[test]
fn the_certificate_of_the_chances_is_rechecked() {
    if !ready(Need::Python, || have("python3"), "no python3, so tools/recheck.py was not run") {
        return;
    }
    let recheck = |cert: &str| -> (bool, String) {
        let t = TempDir::new("chances-cert");
        let p = t.path().join("c.json");
        std::fs::write(&p, cert).unwrap();
        let o = Command::new("python3").arg(root().join("tools/recheck.py")).arg(&p).output().expect("cannot start python3");
        (o.status.success(), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
    };
    let (c, cert, _) = run(&["certificate", LOTTERY]);
    assert_eq!(c, 0);
    let (ok, said) = recheck(&cert);
    assert!(ok, "{said}");
    assert!(said.contains("chances: 6 of 6 claims re-checked — 8 classes counted box by box over the rows, 0 points run one by one"), "{said}");
    let lies: &[(&str, &str, &str)] = &[
        ("a box with a point more", "\"points\":\"48\"", "\"points\":\"49\""),
        ("a box that does not pass said to", "\"outputs\":{\"prize\":\"first\"},\"hit\":true", "\"outputs\":{\"prize\":\"first\"},\"hit\":false"),
        ("a chance the counts do not give", "\"chance\":{\"is\":\"1/1000\"}", "\"chance\":{\"is\":\"1/999\"}"),
        ("a draw of more points", "\"points\":\"1000\"", "\"points\":\"1001\""),
    ];
    for (what, from, to) in lies {
        assert!(cert.contains(from), "{what}: the certificate has no {from}");
        let (ok, said) = recheck(&cert.replacen(from, to, 1));
        assert!(!ok, "{what}: a lying certificate passed:\n{said}");
    }
    // A claim whose points were run one by one is run again, and its two ends are held to what that comes to.
    let (c, cert, _) = run(&["certificate", CONTEST]);
    assert_eq!(c, 0);
    let (ok, said) = recheck(&cert);
    assert!(ok && said.contains("chances: 5 of 5 claims re-checked — 0 classes counted box by box over the rows, 25000 points run one by one"), "{said}");
    let (ok, said) = recheck(&cert.replacen("\"low\":\"3/5\"", "\"low\":\"2/5\"", 1));
    assert!(!ok, "a lowest chance that fails the claim passed:\n{said}");
}

/// The page for people says what each random input is drawn from, shows each claim beside what the
/// check counted, and the HTML page counts the chances of the case entered with the generated code;
/// the customer reads the claims as percentages.
#[test]
fn the_pages_show_the_draws_and_the_claims() {
    let (c, md, _) = run(&["doc", BOX]);
    assert_eq!(c, 0);
    for want in [
        "| pick | number | 0 to 13 | `random`: drawn uniformly from the 14 whole numbers from 0 to 13 |",
        "## Chances (verified)",
        "| true | true | false | false | false | true | true | bonus_points | - | 5/14 | holds: 5/14 (35.7%) |",
        "| - | - | - | - | - | - | - | catalog | - | 1/7 | holds: 1/7 (14.3%) |",
        "| - | false | - | - | - | - | - | accessory | - | 0 | holds: 0 (0.0%) |",
        "| - | - | - | - | - | - | - | bonus_points | - | >=5/14 <=4/7 | holds: from 5/14 (35.7%) to 4/7 (57.1%) |",
        "`reward_box_chances` (in Python)",
    ] {
        assert!(md.contains(want), "no {want:?} in:\n{md}");
    }
    let (c, md, _) = run(&["doc", ROUTING]);
    assert_eq!(c, 0);
    assert!(md.contains("`take_weighted` adds the weight of each element that lands on its verdict to a running total"), "{md}");
    assert!(md.contains("`held` is the value taken (`take_weighted`), else the `empty` answer."), "{md}");
    let (c, md, _) = run(&["doc", CONTEST]);
    assert_eq!(c, 0);
    assert!(md.contains("(among the points the constraints let in)") && md.contains("points for these claims"), "{md}");
    let (c, customer, _) = run(&["doc", LOTTERY, "--audience", "customer"]);
    assert_eq!(c, 0);
    assert!(customer.contains("## How likely each answer is") && customer.contains("| false | third | 5% (5.0%) |"), "{customer}");
    let (c, md, _) = run(&["doc", "tests/corpus/抽選の特典.rule", "--lang", "ja"]);
    assert_eq!(c, 0);
    assert!(md.contains("`random`: 0 〜 13 の整数 14 通りから一様に選ばれます") && md.contains("成り立つ: 5/14（35.7%）"), "{md}");

    let t = TempDir::new("chances-doc");
    let (c, _, e) = run(&["doc", BOX, "--format", "html", "--out", t.path().to_str().unwrap()]);
    assert_eq!(c, 0, "{e}");
    let html = std::fs::read_to_string(t.path().join("reward_box.html")).unwrap();
    assert!(html.contains("chances: rewardBoxChances") && html.contains("<div id=\"try-chances\"></div>") && html.contains("\"random\":true"), "the panel has no chances");
    // A page of a rule that draws nothing has none of it.
    let (c, _, _) = run(&["doc", "tests/corpus/course_catalog.rule", "--format", "html", "--out", t.path().to_str().unwrap()]);
    assert_eq!(c, 0);
    let plain = std::fs::read_to_string(t.path().join("course_catalog.html")).unwrap();
    assert!(!plain.contains("try-chances") && !plain.contains("rulecChances"), "a page that draws nothing carries the chances");
}

/// A DOM small enough to run the panel the page carries: elements, the ids the script looks up, and
/// the listeners it attaches. Setting the text of an element empties it, as in a browser.
const DOM: &str = r##"
class El {
  constructor(tag) {
    this.tagName = tag.toUpperCase();
    this.children = [];
    this.listeners = {};
    this.classList = { add() {}, remove() {}, contains: () => false, toggle() {} };
    this.style = {};
    this.value = "";
    this.checked = false;
    this._text = "";
    this.name = "";
  }
  get textContent() { return this._text + this.children.map((c) => c.textContent).join("|"); }
  set textContent(v) { this._text = String(v); this.children = []; }
  appendChild(c) { this.children.push(c); c.parentNode = this; return c; }
  remove() { const p = this.parentNode; if (p) p.children = p.children.filter((x) => x !== this); }
  addEventListener(k, f) { (this.listeners[k] ||= []).push(f); }
  click() { for (const f of this.listeners.click || []) f(); }
  querySelectorAll() { return []; }
  get elements() {
    const o = {};
    const walk = (n) => { if (n.name) o[n.name] = n; n.children.forEach(walk); };
    this.children.forEach(walk);
    return o;
  }
}
const ids = {};
for (const id of ["try-form", "try-rows", "try-buttons", "try-result", "try-record", "try-chances"]) {
  ids["#" + id] = new El(id === "try-form" ? "form" : "div");
}
globalThis.document = { createElement: (t) => new El(t), querySelector: (s) => ids[s] ?? null, querySelectorAll: () => [] };
globalThis.CSS = { escape: (s) => s };
globalThis.history = { replaceState() {} };
globalThis.location = { hash: "", search: "" };
globalThis.window = { addEventListener() {} };
globalThis.window.parent = globalThis.window;
export { ids };
"##;

/// Driving that panel: the lottery out of a campaign, then in one. What it prints is the chances
/// table of the case, a row to a line.
const DRIVE: &str = r##"
const fields = ids["#try-form"].elements;
const rowsOf = (el) => el.children.filter((c) => c.tagName === "TABLE").flatMap((t) => t.children.map((tr) => tr.children.map((td) => td.textContent).join(" | ")));
fields.campaign.checked = false;
fields.ticket.value = "7";
ids["#try-buttons"].children[0].click();
console.log(JSON.stringify({ result: ids["#try-result"].textContent, chances: rowsOf(ids["#try-chances"]) }));
fields.campaign.checked = true;
ids["#try-buttons"].children[0].click();
console.log(JSON.stringify({ result: ids["#try-result"].textContent, chances: rowsOf(ids["#try-chances"]) }));
"##;

/// The panel counts the chances of the case entered with the generated JavaScript, from the inputs
/// that are not random — whatever the random one holds — and shows each answer with its share.
#[test]
fn the_panel_counts_the_chances_of_the_case_entered() {
    if !ready(Need::Node, || have("node"), "no node, so the page's script was not run") {
        return;
    }
    let t = TempDir::new("chances-panel");
    let (c, _, e) = run(&["doc", LOTTERY, "--format", "html", "--out", t.path().to_str().unwrap()]);
    assert_eq!(c, 0, "{e}");
    let html = std::fs::read_to_string(t.path().join("prize_lottery.html")).unwrap();
    let head = html.find("<script type=\"module\">").expect("no script on the page") + "<script type=\"module\">".len();
    let body = &html[head..head + html[head..].find("</script>").expect("the script is not closed")];
    std::fs::write(t.path().join("dom.mjs"), DOM).unwrap();
    let page = t.path().join("page.mjs");
    std::fs::write(&page, format!("import {{ ids }} from \"./dom.mjs\";\n{body}\n{DRIVE}")).unwrap();
    let o = Command::new("node").arg(&page).output().expect("cannot start node");
    let said = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(o.status.success(), "{said}{}", String::from_utf8_lossy(&o.stderr));
    let mut lines = said.lines();
    let off = json(lines.next().expect("no first line"));
    let on = json(lines.next().expect("no second line"));
    assert!(off.get("result").and_then(|v| v.as_str()).unwrap_or_default().contains("prize = second"), "{said}");
    let rows = |j: &rulec::json::Json| -> Vec<String> {
        j.get("chances").and_then(|c| c.as_arr()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
    };
    assert_eq!(rows(&off), ["prize | chance", "first | 1/1000 (0.1%)", "second | 9/1000 (0.9%)", "third | 1/20 (5.0%)", "no_prize | 47/50 (94.0%)"], "{said}");
    assert_eq!(rows(&on), ["prize | chance", "first | 1/1000 (0.1%)", "second | 9/1000 (0.9%)", "third | 9/100 (9.0%)", "no_prize | 9/10 (90.0%)"], "{said}");
}

/// `rulec fmt` lines the claims up like any table, and leaves the chances as written.
#[test]
fn fmt_aligns_the_claims() {
    let t = TempDir::new("chances-fmt");
    let src = std::fs::read_to_string(root().join(LOTTERY)).unwrap();
    let messy = src.replace("| true     | third    | 9%     |", "|true|third|9%|");
    assert_ne!(messy, src);
    std::fs::write(t.path().join("messy.rule"), &messy).unwrap();
    let (c, _, e) = run_in(t.path(), &["fmt", "messy.rule"]);
    assert_eq!(c, 0, "{e}");
    assert_eq!(std::fs::read_to_string(t.path().join("messy.rule")).unwrap(), src);
}
