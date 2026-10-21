//! `kaname vectors` (DESIGN 9): the vectors of every example, in English and in Japanese, held to
//! `tests/golden/vectors/` (`KANAME_BLESS=1` writes them again); the two versions of an example give the
//! same answers once their names are read as their aliases (DESIGN 16.6); and what the command takes.

mod common;

use kaname::check::Checked;
use kaname::ty::Ty;
use kaname::value::Program;
use ritsu_base::json::{self, Json};
use std::rc::Rc;

fn joined(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = kaname::run::run(&args, common::joined(), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

const NAMES: [&str; 4] = ["payout", "late_payment", "media_mail", "discount_split"];

#[test]
fn every_example_has_the_vectors_the_golden_files_hold() {
    for name in NAMES {
        for (kn, lang, golden) in [(format!("examples/{name}/{name}.kn"), "en", name.to_string()), (format!("examples/{name}/{name}.ja.kn"), "ja", format!("{name}.ja"))] {
            let (code, out, err) = joined(&["vectors", &kn, "--lang", lang]);
            assert_eq!(code, 0, "{kn}: {out}{err}");
            assert!(err.is_empty(), "{kn}: {err}");
            ritsu_testkit::golden(format!("tests/golden/vectors/{golden}.jsonl"), &out);
        }
    }
}

/// A value on the wire of type `t`, with the names a person declared read as their aliases, and a
/// transfer's by their places in the book: what the English and the Japanese versions share.
fn alias(p: &Program, j: &Json, t: &Ty) -> Json {
    match (j, t) {
        (Json::Null, _) => Json::Null,
        (_, Ty::Opt(x)) => alias(p, j, x),
        (Json::Arr(xs), Ty::List(x)) => Json::arr(xs.iter().map(|v| alias(p, v, x))),
        (Json::Obj(fs), Ty::Record(id)) => {
            let fields = p.fields(id);
            Json::obj(fs.iter().map(|(k, v)| {
                let (n, s) = fields.iter().find(|(n, _)| n.text == *k).unwrap_or_else(|| panic!("no field {k}"));
                (n.ascii().to_string(), alias(p, v, &s.ty))
            }))
        }
        (Json::Str(s), Ty::Enum(id)) => Json::str(p.values(id).0.iter().find(|v| v.text == *s).map(|v| v.ascii().to_string()).unwrap_or_else(|| s.clone())),
        (Json::Obj(_), Ty::Transfer(id)) => {
            let (book, params) = p.transfer(id).unwrap();
            let at = book.transfers.iter().position(|x| x.name == id.transfer).unwrap();
            let args = j.get("args").and_then(Json::as_obj).unwrap();
            Json::obj([
                ("transfer", Json::int(at as i128)),
                ("op", Json::str(id.op.clone())),
                ("args", Json::arr(args.iter().map(|(k, v)| {
                    let (i, (_, pt)) = params.iter().enumerate().find(|(_, (n, _))| n == k).unwrap();
                    Json::arr([Json::int(i as i128), alias(p, v, pt)])
                }))),
            ])
        }
        _ => j.clone(),
    }
}

/// The vectors of a module, each line read as its aliases: the function, the input, and the answer
/// or the kind of failure.
fn read_as_aliases(c: &Rc<Checked>, text: &str) -> Vec<Json> {
    let p = Program::of(c);
    text.lines()
        .map(|l| {
            let v = json::parse(l).unwrap();
            let fname = v.get("function").and_then(Json::as_str).unwrap();
            let f = c.decls.fns.iter().find(|f| f.name.text == fname).unwrap();
            let input = v.get("in").and_then(Json::as_obj).unwrap();
            let input = Json::obj(input.iter().map(|(k, x)| {
                let prm = f.params.iter().find(|q| q.name.text == *k).unwrap();
                (prm.name.ascii().to_string(), alias(&p, x, &prm.slot.ty))
            }));
            let answer = match (v.get("out"), v.get("fails"), v.get("diverged")) {
                (Some(o), _, _) => alias(&p, o, &f.ret.ty),
                (_, Some(fl), _) => Json::obj([("fails", fl.get("kind").cloned().unwrap())]),
                _ => Json::str("diverged"),
            };
            Json::obj([("function", Json::str(f.name.ascii())), ("in", input), ("answer", answer)])
        })
        .collect()
}

#[test]
fn the_two_versions_of_an_example_answer_alike() {
    let mut compared = 0;
    for name in NAMES {
        let (en, ja) = (format!("examples/{name}/{name}.kn"), format!("examples/{name}/{name}.ja.kn"));
        let (_, en_out, _) = joined(&["vectors", &en]);
        let (_, ja_out, _) = joined(&["vectors", &ja]);
        let en_c = common::check(&en).checked.clone().unwrap();
        let ja_c = common::check(&ja).checked.clone().unwrap();
        let (a, b) = (read_as_aliases(&en_c, &en_out), read_as_aliases(&ja_c, &ja_out));
        assert_eq!(a.len(), b.len(), "{name}");
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.compact(), y.compact(), "{name}");
        }
        compared += a.len();
    }
    eprintln!("the two versions of the examples: {compared} vectors answer alike");
    assert!(compared > 0);
}

/// The shares the discount is spread in come to the discount on every vector (DESIGN 17.4): what the
/// running totals given to the rule hold to, which no analysis of ranges says.
#[test]
fn the_shares_come_to_the_discount() {
    for kn in ["examples/discount_split/discount_split.kn", "examples/discount_split/discount_split.ja.kn"] {
        let (_, out, _) = joined(&["vectors", kn]);
        let mut n = 0;
        for l in out.lines() {
            let v = json::parse(l).unwrap();
            let input = v.get("in").and_then(Json::as_obj).unwrap();
            let discount = input[1].1.as_int().unwrap();
            let shares = v.get("out").and_then(Json::as_arr).expect("every vector answers");
            let sum: i128 = shares.iter().map(|s| s.as_obj().unwrap()[1].1.as_int().unwrap()).sum();
            assert_eq!(sum, discount, "{l}");
            n += 1;
        }
        assert_eq!(n, 12, "{kn}");
    }
}

/// A rule given what it does not take is a vector that fails with `rule` (DESIGN 9): the late
/// payment's function with its branch taken away, the days late going to the rule as they are. The
/// check finds that input (E301), so `vectors` does not write the module's; the vectors are the
/// library's, of the module as its types read it.
#[test]
fn a_vector_the_rule_refuses() {
    let t = ritsu_testkit::TempDir::new("kaname-vectors-refused");
    ritsu_testkit::tmp::copy_dir(std::path::Path::new("examples/late_payment/dates"), &t.path().join("dates"));
    ritsu_testkit::tmp::copy_dir(std::path::Path::new("examples/late_payment/rules"), &t.path().join("rules"));
    let kn = t.write(
        "late_payment.kn",
        "module late_payment v1\n\nuse dates terms from \"dates/payment_terms.cal\"\nuse rule charges from \"rules/late_payment_charges.rule\"\n\nrecord Invoice\n  id       : string\n  invoiced : date  range >=2026-01-01 <=2027-12-31\n  amount   : money[GBPc]  range >=1GBPc <=100_000_000GBPc\n\nfn interest(invoice: Invoice, paid_on: date range >=2026-01-01 <=2029-12-31, reference_rate: rate[step 0.01%] range >=0% <=15%) -> money[GBPc]\n  let late = paid_on - terms.due(invoiced: invoice.invoiced)\n  charges(debt: invoice.amount, days_late: late, reference_rate: reference_rate).interest\n",
    );
    let (code, out, _) = joined(&["check", kn.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(out.contains("error[E301]: ") && out.contains(":13:44: `late` can be outside >=1 <=1500, the range of the input `days_late` of the rule `charges`\n"), "{out}");
    let o = common::check(kn.to_str().unwrap());
    let c = o.analysed.clone().unwrap();
    let p = Program::of(&c);
    let (vs, _) = kaname::vectors::choose(&p, &c, 0);
    let first = kaname::vectors::line(&p, &common::joined(), &c, 0, &vs[0], false, ritsu_base::text::Lang::En);
    assert_eq!(
        first.compact(),
        r#"{"function":"interest","in":{"invoice":{"id":"a","invoiced":"2026-01-01","amount":50000000},"paid_on":"2026-01-01","reference_rate":0},"fails":{"kind":"rule","module":"late_payment","line":13,"message":"The rule `charges` refuses its inputs"},"why":"paid_on: the lower end; reference_rate: the lower end"}"#
    );
    let (_, golden, _) = joined(&["vectors", "examples/late_payment/late_payment.kn"]);
    assert_eq!(
        golden.lines().next().unwrap(),
        r#"{"function":"claim","in":{"invoice":{"id":"a","invoiced":"2026-01-01","amount":50000000},"paid_on":"2026-01-01","reference_rate":0},"out":null,"why":"paid_on: the lower end; reference_rate: the lower end"}"#
    );
}

#[test]
fn what_the_command_takes() {
    // one function, by its name or its alias
    let (code, out, _) = joined(&["vectors", "examples/media_mail/media_mail.kn", "--function", "quote"]);
    assert_eq!(code, 0);
    assert!(out.lines().all(|l| l.starts_with("{\"function\":\"quote\",")), "{out}");
    let (_, by_alias, _) = joined(&["vectors", "examples/media_mail/media_mail.ja.kn", "--function", "quote"]);
    assert!(by_alias.lines().all(|l| l.starts_with("{\"function\":\"見積もる\",")), "{by_alias}");
    let (code, _, err) = joined(&["vectors", "examples/media_mail/media_mail.kn", "--function", "price"]);
    assert_eq!((code, err.as_str()), (2, "error: `examples/media_mail/media_mail.kn` has no function `price` (its functions are `weight_of`, `quote`)\n"));
    // the calls with their answers, with --trace
    let (_, out, _) = joined(&["vectors", "examples/late_payment/late_payment.kn", "--trace"]);
    let first = json::parse(out.lines().nth(2).unwrap()).unwrap();
    let calls = first.get("calls").and_then(Json::as_arr).unwrap();
    assert_eq!(calls.len(), 2, "{}", first.compact());
    assert_eq!(calls[0].get("date").and_then(Json::as_str), Some("terms.due"));
    assert_eq!(calls[1].get("rule").and_then(Json::as_str), Some("charges"));
    // the reason in the language asked for
    let (_, out, _) = joined(&["vectors", "examples/payout/payout.ja.kn", "--lang", "ja"]);
    assert!(out.lines().next().unwrap().ends_with("\"why\":\"基本の値\"}"), "{out}");
}

/// The candidates around a literal of a temperature of the other unit are read with that unit's zero
/// (DESIGN 9, 3.10): a ℉ compared with 30℃ is tried at 85℉, 86℉ and 87℉, and a ℃ compared with 50℉
/// at 9℃, 10℃ and 11℃. Read by the factor alone, they were 53℉ to 55℉, and none at all around 50℉
/// (250/9℃ is on no whole number).
#[test]
fn a_temperature_of_the_other_unit_is_tried_around_its_value() {
    for (function, want) in [
        (
            "hot",
            [
                r#"{"function":"hot","in":{"t":85},"out":"not hot","why":"t: a step below 30℃"}"#,
                r#"{"function":"hot","in":{"t":86},"out":"hot","why":"t: compared with 30℃"}"#,
                r#"{"function":"hot","in":{"t":87},"out":"hot","why":"t: a step above 30℃"}"#,
            ],
        ),
        (
            "cool",
            [
                r#"{"function":"cool","in":{"t":9},"out":"cool","why":"t: a step below 50℉"}"#,
                r#"{"function":"cool","in":{"t":10},"out":"mild","why":"t: compared with 50℉"}"#,
                r#"{"function":"cool","in":{"t":11},"out":"mild","why":"t: a step above 50℉"}"#,
            ],
        ),
    ] {
        let (code, out, err) = joined(&["vectors", "tests/modules/temps.kn", "--function", function]);
        assert_eq!(code, 0, "{err}");
        for w in want {
            assert!(out.lines().any(|l| l == w), "{w} is not among\n{out}");
        }
    }
}

/// A function with more than 256 combinations of its parameters' candidates moves each alone, and
/// the pairs that meet in one comparison or one call together; a function never has more than 2,000.
#[test]
fn many_candidates_are_taken_a_pair_at_a_time() {
    let t = ritsu_testkit::TempDir::new("kaname-vectors");
    let kn = t.write(
        "many.kn",
        "module many v1\n\nfn f(a: number range >=0 <=100, b: number range >=0 <=100, c: number range >=0 <=100, d: number range >=0 <=100, e: number range >=0 <=100) -> bool\n  a + b > 3 and c > 5 and d > 7 and e > 9\n",
    );
    let (code, out, _) = joined(&["vectors", kn.to_str().unwrap()]);
    assert_eq!(code, 0);
    let n = out.lines().count();
    // each moved alone, and a and b as a pair (they meet in one comparison)
    assert!(n > 10 && n < 256, "{n}");
    assert!(out.lines().any(|l| l.contains("\"why\":\"a: the lower end; b: the upper end\"")), "{out}");
}
