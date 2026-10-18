//! The effects (DESIGN 4): what each function can fail with, inferred from the places of its body
//! that are not shown and from the functions it calls; a ring of calls may not stop; an effect
//! written is held to what is inferred (E401); the line that says a module passes says the effects;
//! and DESIGN 12's materials say E301 and E401 with what leads to them.

mod common;

use kaname::ast::FailKind;
use ritsu_base::text::Lang;
use ritsu_testkit::TempDir;

/// The module `src` as `main.kn` in `t`, checked with every language joined.
fn check(t: &TempDir, src: &str) -> std::rc::Rc<kaname::check::Outcome> {
    let f = t.write("main.kn", src);
    common::check(f.to_str().unwrap())
}

/// Each function's effect as its head would write it.
fn effects(o: &kaname::check::Outcome) -> Vec<String> {
    let c = o.analysed.as_ref().unwrap();
    c.analysis.get().unwrap().fns.iter().map(|f| f.effect.written()).collect()
}

const HEAD: &str = "module main v1\n\n";

#[test]
fn what_a_function_can_fail_with_and_what_its_callers_take() {
    let t = TempDir::new("kaname-effects");
    // a place the analysis cannot decide: the function can fail with its kind, and so can its caller
    let src = format!(
        "{HEAD}fn area(side: number range >=1 <=8) -> number range >=0 <=24\n  side * (10 - side)\n\nfn twice(side: number range >=1 <=8) -> number\n  area(side: side) * 2\n\nfn plain(n: number range >=0 <=9) -> number\n  n + 1\n"
    );
    let o = check(&t, &src);
    assert_eq!(common::codes(&o), ["W301"]);
    assert_eq!(effects(&o), ["fails range", "fails range", "total"]);
    assert_eq!(o.summary().unwrap().get(Lang::En), "3 functions: 2 fails range; 1 total");
    assert_eq!(o.summary().unwrap().get(Lang::Ja), "関数 3 個: fails range 2 個、total 1 個");
    // the kinds in their order, and a ring of calls: both functions may not stop, and share their kinds
    let src = format!(
        "{HEAD}fn even(n: number range >=0 <=50) -> bool\n  if n <= 0 then true else odd(n: n - 1)\n\nfn odd(n: number range >=0 <=50) -> bool\n  if n <= 0 then false else even(n: n - 1)\n\nfn each(a: money[USDc] range >=0USDc <=100USDc, n: number range >=0 <=5) -> money[USDc]\n  if even(n: n) then down(a / (n + 1), 1USDc) else a\n"
    );
    let o = check(&t, &src);
    assert_eq!(effects(&o), ["diverges", "diverges", "diverges"]);
    let c = o.analysed.as_ref().unwrap();
    let e = &c.analysis.get().unwrap().fns[2].effect;
    assert!(e.diverges && e.fails.is_empty() && !e.total());
}

#[test]
fn what_is_written_holds_what_is_inferred() {
    let t = TempDir::new("kaname-effects-written");
    let area = "fn area(side: number range >=1 <=8) -> number range >=0 <=24";
    // `total` on a function that can fail: E401, with the place
    let o = check(&t, &format!("{HEAD}{area} total\n  side * (10 - side)\n"));
    assert_eq!(common::codes(&o), ["E401", "W301"]);
    let text = common::shown(&o, Lang::En);
    assert!(text.contains("error[E401]: ") && text.contains(":3:1: `area` is written `total`, but it can fail with `range`\n"), "{text}");
    assert!(text.contains("  = Line 4: whether the answer of `area` stays in what it declares (range >=0 <=24) cannot be decided (W301)\n"), "{text}");
    // the kinds written hold it, or more of them
    assert_eq!(common::codes(&check(&t, &format!("{HEAD}{area} fails range\n  side * (10 - side)\n"))), ["W301"]);
    assert_eq!(common::codes(&check(&t, &format!("{HEAD}{area} fails range, zero\n  side * (10 - side)\n"))), ["W301"]);
    // another kind than the one written
    assert_eq!(common::codes(&check(&t, &format!("{HEAD}{area} fails zero\n  side * (10 - side)\n"))), ["E401", "W301"]);
    // a ring of calls is written `diverges`
    let steps = "fn steps(n: number range >=0 <=100) -> number range >=0 <=100";
    let body = "  if n <= 0 then 0 else min(steps(n: n - 1) + 1, 100)\n";
    assert_eq!(common::codes(&check(&t, &format!("{HEAD}{steps} total\n{body}"))), ["E401"]);
    assert_eq!(common::codes(&check(&t, &format!("{HEAD}{steps} diverges\n{body}"))), Vec::<&str>::new());
    assert_eq!(common::codes(&check(&t, &format!("{HEAD}{steps} fails range\n{body}"))), ["E401"]);
    // a caller written `total` of one that can fail: the call is the note
    let o = check(&t, &format!("{HEAD}{area}\n  side * (10 - side)\n\nfn twice(side: number range >=1 <=8) -> number total\n  area(side: side) * 2\n"));
    assert_eq!(common::codes(&o), ["W301", "E401"]);
    assert!(common::shown(&o, Lang::En).contains("  = Line 7: it calls `area`, whose effect is `fails range`\n"), "{}", common::shown(&o, Lang::En));
    let ja = common::shown(&o, Lang::Ja);
    assert!(ja.contains("エラー[E401]: ") && ja.contains("関数 `twice` には `total` と書いてありますが、`range` で失敗することがあります\n"), "{ja}");
}

/// A function of another module brings its effect with it.
#[test]
fn an_effect_comes_from_another_module() {
    let t = TempDir::new("kaname-effects-modules");
    t.write("pricing.kn", "module pricing v1\n\nfn area(side: number range >=1 <=8) -> number range >=0 <=24\n  side * (10 - side)\n");
    let o = check(&t, "module main v1\n\nuse module pricing from \"pricing.kn\"\n\nfn twice(side: number range >=1 <=8) -> number total\n  pricing.area(side: side) * 2\n");
    // the place is the other module's, said when that module is checked; the call brings its kind
    assert_eq!(common::codes(&o), ["E401"]);
    assert_eq!(effects(&o), ["fails range"]);
}

/// Verdicts written by hand, as ritsu-cross hands its own over (DESIGN 14.5): the plumbing kaname
/// reads them through, apart from the decisions.
struct Answers(Vec<ritsu_ports::CallVerdict>);

impl ritsu_ports::Verdicts for Answers {
    fn verdicts(&self, _file: &std::path::Path) -> Vec<ritsu_ports::CallVerdict> {
        self.0.clone()
    }
}

/// Every place across the borders of the module at `f`, answered `outcome` (the refusals of a
/// transfer, `reasons`), as one check of ritsu's would.
fn answers(f: &std::path::Path, outcome: impl Fn(&ritsu_ports::Given) -> ritsu_ports::Answer<ritsu_ports::Seen>, reasons: Vec<String>) -> Answers {
    use ritsu_ports::{Answer, CallVerdict, Crossing, Programs};
    let x = kaname::ports::Engine.crossings(f, &common::ports()).unwrap();
    let mut out = Vec::new();
    for t in &x.transfers {
        for a in &t.amounts {
            let o = outcome(a);
            let said = match &o {
                Answer::Holds => None,
                Answer::Fails(_) => Some(("E203".to_string(), ritsu_base::tr!("額になることがあります", "the amount can be one chobo does not take"))),
                Answer::Undecided(_) => Some(("W203".to_string(), ritsu_base::tr!("額を決められません", "whether chobo takes the amount cannot be decided"))),
            };
            out.push(CallVerdict { at: a.at, check: Crossing::Amount, about: a.to.clone(), outcome: o, said, refusals: None });
        }
        out.push(CallVerdict { at: t.at, check: Crossing::Refusals, about: format!("{}.{}", t.transfer, t.op), outcome: Answer::Holds, said: None, refusals: Some(reasons.clone()) });
    }
    Answers(out)
}

/// The verdicts of the checks across the languages decide the places they are about (DESIGN 4.3):
/// shown places leave the effect, and an undecided one or one with an example is a failure the
/// generated code checks for, held against what is written (E401, the note in ritsu's code).
#[test]
fn the_verdicts_decide_the_places_that_waited() {
    use ritsu_ports::Answer;
    let f = std::path::Path::new("examples/payout/payout.kn");
    let run = |a: Answers| kaname::check::Run::new(common::joined().with_verdicts(std::rc::Rc::new(a))).file(f.to_str().unwrap()).unwrap();
    // every amount shown: `settle` is total, and the reasons each operation can be refused with are kept
    let o = run(answers(f, |_| Answer::Holds, vec!["key_conflict".into()]));
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    assert_eq!(effects(&o), ["total"]);
    let c = o.checked.as_ref().unwrap();
    let facts = &c.analysis.get().unwrap().fns[0];
    assert_eq!(facts.refusals.iter().map(|r| (r.op.as_str(), r.reasons.clone())).collect::<Vec<_>>(), [("capture.do", Some(vec!["key_conflict".to_string()])), ("processing_fee.do", Some(vec!["key_conflict".to_string()]))]);
    // one amount undecided: `settle` can fail with `range`, which its `total` does not hold
    let why = ritsu_base::tr!("決められません", "cannot be decided");
    let o = run(answers(f, |a| if a.to == "amount" { Answer::Undecided(why.clone()) } else { Answer::Holds }, vec![]));
    assert_eq!(common::codes(&o), ["E401"]);
    assert_eq!(effects(&o), ["fails range"]);
    let text = common::shown(&o, Lang::En);
    assert!(text.contains("  = Line 21: whether chobo takes the amount cannot be decided (ritsu W203)
"), "{text}");
    let json = kaname::check::to_json(&o, std::path::Path::new("."), Lang::En).compact();
    assert!(json.contains(r#""check":"X4","code":"W203","outcome":"undecided""#), "{json}");
    // an example: the place keeps the run that breaks it
    let seen = ritsu_ports::Seen { at: ritsu_ports::At::default(), call: "settle(…)".into(), input: r#"{"payment":{"id":"a","seller":"a","amount":1,"international":false}}"#.into(), given: vec![("amount".into(), ritsu_ports::Value::Int(-48))] };
    let o = run(answers(f, |a| if a.to == "amount" { Answer::Fails(seen.clone()) } else { Answer::Holds }, vec![]));
    assert_eq!(common::codes(&o), ["E401"]);
    let c = o.analysed.as_ref().unwrap();
    let place = c.analysis.get().unwrap().fns[0].places.iter().find(|p| p.what.across().is_some() && p.failing()).unwrap();
    let kaname::ranges::Outcome::Example(w) = &place.outcome else { panic!("{:?}", place.outcome) };
    assert_eq!(w.value, Some(kaname::value::Value::Num(kaname::num::Rat::int(-48))));
    assert_eq!(w.input.len(), 1);
    assert_eq!(place.across.as_ref().map(|(code, _)| code.as_str()), Some("E203"));
}

/// A place that waits for the checks across the languages (X2, X3, X4, X6) is counted as undecided
/// until their answer comes (DESIGN 4.3): in the effect, but not held against what is written. A run
/// that hands over no verdicts (the tests here, without ritsu-cross) leaves them so.
#[test]
fn a_place_that_waits_is_counted_and_not_held_against_what_is_written() {
    for (f, effect) in [
        ("examples/payout/payout.kn", "fails range"),
        ("examples/late_payment/late_payment.kn", "fails date"),
        ("examples/media_mail/media_mail.kn", "total"),
        ("examples/discount_split/discount_split.kn", "fails rule"),
    ] {
        let o = common::check(f);
        assert!(o.diags.is_empty(), "{f}: {}", common::shown(&o, Lang::En));
        let all = effects(&o);
        assert!(all.iter().any(|e| e == effect), "{f}: {all:?}");
        let c = o.checked.as_ref().unwrap();
        for x in &c.analysis.get().unwrap().fns {
            assert!(x.effect.decided.is_empty(), "{f}: {x:?}");
        }
    }
    let c = common::check("examples/payout/payout.kn").checked.clone().unwrap();
    assert_eq!(c.analysis.get().unwrap().fns[0].effect.fails.iter().copied().collect::<Vec<_>>(), [FailKind::Range]);
}

/// DESIGN 12's materials: the late payment with its branch taken away (E301, with an input that
/// gives the rule days it does not take), and the weight of a cart that may have no lines, which
/// makes `quote`'s `total` E401 (DESIGN 17.3).
#[test]
fn the_materials_of_design_12() {
    let t = TempDir::new("kaname-effects-design");
    ritsu_testkit::tmp::copy_dir(std::path::Path::new("examples/late_payment"), t.path());
    let src = std::fs::read_to_string(t.path().join("late_payment.kn")).unwrap();
    let src = src.replace("  if late >= 1\n    let c = charges(debt: invoice.amount, days_late: late, reference_rate: reference_rate)\n    Claim { due: due, days_late: late, compensation: c.compensation, interest: c.interest }\n  else\n    none\n", "  let c = charges(debt: invoice.amount, days_late: late, reference_rate: reference_rate)\n  Claim { due: due, days_late: late, compensation: c.compensation, interest: c.interest }\n");
    let src = src.replace(") -> Claim? total\n", ") -> Claim?\n");
    let src = src.split("# The first is GOV.UK").next().unwrap().to_string();
    let f = t.write("late_payment.kn", src);
    let o = common::check(f.to_str().unwrap());
    assert_eq!(common::codes(&o), ["E301"]);
    let text = common::shown(&o, Lang::En);
    assert!(text.contains(":22:52: `late` can be outside >=1 <=1500, the range of the input `days_late` of the rule `charges`\n"), "{text}");
    assert!(text.contains("  = For example, `claim(invoice: Invoice { id: \"a\", invoiced: 2026-01-01, amount: 50000000GBPc }, paid_on: 2026-01-01, reference_rate: 0%)` gives it -29.\n"), "{text}");

    let t = TempDir::new("kaname-effects-weight");
    ritsu_testkit::tmp::copy_dir(std::path::Path::new("examples/media_mail"), t.path());
    let src = std::fs::read_to_string(t.path().join("media_mail.kn")).unwrap().replace("fn weight_of(lines: list[Line] count >=1 <=30)", "fn weight_of(lines: list[Line] count >=0 <=30)");
    let f = t.write("media_mail.kn", src);
    let o = common::check(f.to_str().unwrap());
    assert_eq!(common::codes(&o), ["E401", "W301"]);
    let text = common::shown(&o, Lang::En);
    assert!(text.contains(":15:1: `quote` is written `total`, but it can fail with `rule`\n"), "{text}");
    assert!(text.contains("  = The analysis of ranges finds the value in >=0oz <=240oz, and no input among the function's vectors takes it out.\n"), "{text}");
}
