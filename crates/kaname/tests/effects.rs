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

/// A value of an enum built with fields that declare ranges is a place of the kind `range`, as a
/// record built is; so is one made from another (DESIGN 23.4, 23.10). A `match` that splits a value
/// of the enum and builds none is total.
#[test]
fn a_value_with_fields_is_built_as_a_record_is() {
    let t = TempDir::new("kaname-effects-sums");
    let order = "enum Order\n  draft\n  placed\n    on : date  range >=2026-01-01 <=2026-12-31\n\n";
    let o = check(
        &t,
        &format!("{HEAD}{order}fn place(d: date range >=2026-01-01 <=2027-12-31) -> Order\n  Order.placed {{ on: d }}\n\nfn later(p: Order.placed, d: date range >=2026-01-01 <=2027-12-31) -> Order\n  Order.placed {{ ..p, on: d }}\n\nfn kept(p: Order.placed) -> Order\n  Order.placed {{ ..p }}\n\nfn on(o: Order) -> date?\n  match o\n    placed p => p.on\n    else => none\n"),
    );
    assert_eq!(common::codes(&o), ["E301", "E301"]);
    assert_eq!(effects(&o), ["fails range", "fails range", "total", "total"]);
}

/// The builtins of dicts, sets and lists fail nothing (DESIGN 24.7): a place past a list's ends cuts
/// it or answers none. The two that have places are `mod` by a value that may be 0 (`zero`) and the
/// count of `numbers` (`overflow`, where it may pass int64).
#[test]
fn the_builtins_of_collections_fail_nothing_but_mod_and_numbers() {
    let t = TempDir::new("kaname-effects-collections");
    let o = check(
        &t,
        &format!(
            "{HEAD}fn walk(xs: list[number] count <=9 range >=0 <=9, d: dict[string, number] count <=9, i: number range >=-9 <=9) -> number\n  xs.sort().distinct().slice(i, i + 3).take(i).drop(i).reverse().concat(d.values()).append(1).count() + d.put(\"a\", 1).remove(\"b\").count()\n\nfn at(xs: list[number] count <=9 range >=0 <=9, i: number range >=-9 <=9) -> number?\n  xs.at(i)\n\nfn rem(x: number range >=0 <=9, n: number range >=1 <=3) -> number\n  mod(x, n)\n\nfn rem0(x: number range >=0 <=9, n: number range >=0 <=3) -> number\n  mod(x, n)\n\nfn some_numbers(a: number range >=1 <=9, b: number range >=0 <=9) -> number\n  numbers(a, b).count()\n\nfn all_numbers(a: number, b: number) -> number\n  numbers(a, b).count()\n"
        ),
    );
    assert_eq!(effects(&o), ["total", "total", "total", "fails zero", "total", "fails overflow"]);
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

/// An overflow that comes from the end of a type, a value whose range is not written (DESIGN 22.2,
/// 22.3): nothing is said, the effect has `overflow`, and `total` written on the function is E401 with
/// the place in a note; one narrowed by a branch is shown, and one from a range written, even all of
/// int64, is said as it was.
#[test]
fn an_overflow_from_the_end_of_a_type_is_in_the_effect() {
    let t = TempDir::new("kaname-effects-open");
    let o = check(&t, &format!("{HEAD}fn next_id(last: number) -> number\n  last + 1\n\nfn caller(n: number) -> number\n  next_id(last: n)\n"));
    assert_eq!(common::codes(&o), Vec::<&str>::new());
    assert_eq!(effects(&o), ["fails overflow", "fails overflow"]);
    let c = o.analysed.as_ref().unwrap();
    assert_eq!(c.analysis.get().unwrap().fns[0].places[0].outcome, kaname::ranges::Outcome::Unbounded);
    // written `total`: E401, the place in its note
    let o = check(&t, &format!("{HEAD}fn doubled(n: number) -> number total\n  n * 2\n"));
    assert_eq!(common::codes(&o), ["E401"]);
    let en = common::shown(&o, Lang::En);
    assert!(en.contains("  = Line 4: the value of `n * 2` can leave int64, as it comes from a value whose range is not written (`overflow`)\n"), "{en}");
    let ja = common::shown(&o, Lang::Ja);
    assert!(ja.contains("  = 4 行目: `n * 2` の値は、範囲を書かない値から来るので、int64 を超えることがあります（`overflow`）\n"), "{ja}");
    // narrowed by a branch: shown, and total holds
    let o = check(&t, &format!("{HEAD}fn doubled(n: number) -> number total\n  if n >= 0 and n <= 1_000\n    n * 2\n  else\n    0\n"));
    assert_eq!(common::codes(&o), Vec::<&str>::new());
    // the whole of int64 written is a promise, and an input of it takes the product past int64
    let o = check(&t, &format!("{HEAD}fn doubled(n: number range >=-9_223_372_036_854_775_808 <=9_223_372_036_854_775_807) -> number\n  n * 2\n"));
    assert_eq!(common::codes(&o), ["E302"]);
    // a function in a ring with no range on its answer answers the whole of its type: one more
    // leaves int64 at its end, said by nothing, and the effect is as it was
    let o = check(&t, &format!("{HEAD}fn countdown(n: number range >=0 <=100) -> number\n  if n <= 0 then 0 else countdown(n: n - 1) + 1\n"));
    assert_eq!(common::codes(&o), Vec::<&str>::new());
    assert_eq!(effects(&o), ["fails overflow diverges"]);
    // a sum over a list whose count is not written
    let o = check(&t, &format!("{HEAD}fn total(xs: list[money[USDc]] range >=0USDc <=100USDc) -> money[USDc]\n  sum(xs, x => x)\n"));
    assert_eq!(common::codes(&o), Vec::<&str>::new());
    assert_eq!(effects(&o), ["fails overflow"]);
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
    assert!(json.contains(r#""check":"transfer_amount","code":"W203","outcome":"undecided""#), "{json}");
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
/// makes `quote`'s `total` E401 (DESIGN 17.8).
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

/// A generic function's own effect is that of the analysis made once, from the types declared: a sum
/// of a number whose range is not written may overflow. A call that is read again with the values it
/// gives takes the effect of that reading: the same function called with a number of 0 to 5 is total
/// there (DESIGN 26.5, 26.6). A call past four readings, and a call in a ring, take the function's own
/// effect; and E401 says which call of which reading brings a kind in.
#[test]
fn a_call_read_again_takes_the_effect_of_that_reading() {
    let t = TempDir::new("kaname-effects-generic");
    let hops = "fn hop1[T](x: T, n: number) -> number\n  hop2(x: x, n: n)\n\nfn hop2[T](x: T, n: number) -> number\n  hop3(x: x, n: n)\n\nfn hop3[T](x: T, n: number) -> number\n  hop4(x: x, n: n)\n\nfn hop4[T](x: T, n: number) -> number\n  bump(x: x, n: n)\n\nfn bump[T](x: T, n: number) -> number\n  n + 1\n\n";
    let o = check(&t, &format!("{HEAD}{hops}fn near(n: number range >=0 <=5) -> number\n  hop3(x: \"a\", n: n)\n\nfn far(n: number range >=0 <=5) -> number\n  hop1(x: \"a\", n: n)\n\nfn own(n: number) -> number\n  bump(x: true, n: n)\n"));
    assert_eq!(common::codes(&o), Vec::<&str>::new());
    assert_eq!(effects(&o), ["fails overflow", "fails overflow", "fails overflow", "fails overflow", "fails overflow", "total", "fails overflow", "fails overflow"]);
    // a ring of generic calls is read as declared, and may not stop
    let o = check(&t, &format!("{HEAD}fn count_down[T](x: T, n: number range >=0 <=10) -> number range >=0 <=10\n  if n = 0 then 0 else count_down(x: x, n: n - 1)\n\nfn ring(n: number range >=0 <=10) -> number range >=0 <=10\n  count_down(x: true, n: n)\n"));
    assert_eq!(effects(&o), ["diverges", "diverges"]);
    // `total` written of a call past four readings: E401, with the call that brings `overflow` in
    let o = check(&t, &format!("{HEAD}{hops}fn far(n: number range >=0 <=5) -> number total\n  hop1(x: \"a\", n: n)\n"));
    assert_eq!(common::codes(&o), ["E401"]);
    let said = o.diags[0].render(Lang::En);
    assert!(said.contains("overflow"), "{said}");
}

/// Function values (DESIGN 27.5): the places of a lambda's body are the places of the function that
/// wrote it, wherever the lambda is read; a function given by its name is called by the function that
/// gives it, and naming a function is calling it, for the rings too; a call of a parameter of a
/// function type takes the effect written on the type, none where none is written, and that
/// parameter's effect goes on to the callers (`through`); and an effect written on a function type is
/// held to the function given (E402).
#[test]
fn the_effects_of_function_values() {
    let t = TempDir::new("kaname-effects-hof");
    let o = check(&t, include_str!("modules/hof.kn"));
    assert_eq!(common::codes(&o), Vec::<&str>::new());
    let c = o.analysed.as_ref().unwrap();
    let a = c.analysis.get().unwrap();
    let of = |name: &str| {
        let f = c.decls.fns.iter().position(|x| x.name.text == name).unwrap();
        a.fns[f].effect.written()
    };
    // what takes functions has no place of its own but those it writes: a sum nothing bounds overflows
    assert_eq!(of("total_by"), "fails overflow");
    assert_eq!(of("apply_all"), "total");
    assert_eq!(of("apply_each"), "total");
    // read again where it is called, the lines and the lambda bound the sum
    assert_eq!(of("order_total"), "total");
    // a lambda that can overflow is the caller's, not the function's that calls it
    assert_eq!(of("bumped"), "fails overflow");
    assert_eq!(of("shifted"), "total");
    // a function given by its name that can overflow: its effect is the effect of the one that gives it
    assert_eq!(of("doubled"), "fails overflow");
    assert_eq!(of("all_doubled"), "fails overflow");
    // the effect written on a function type is taken where it is called
    assert_eq!(of("sum_with"), "fails overflow");
    assert_eq!(of("squares"), "total");
    assert_eq!(of("all_within"), "total");
    // a function given itself is in a ring, and may not stop
    assert_eq!(of("steps_down"), "fails overflow diverges");
    assert_eq!(of("via"), "total");
    // past four readings, the lambda is read once with every value, and overflows
    assert_eq!(of("deep"), "fails overflow");
    // `through`: the parameters of a function type with no effect written on them
    let fns: Vec<String> = c.decls.fns.iter().map(|f| f.name.text.clone()).collect();
    let info = |name: &str| &c.decls.fns[fns.iter().position(|x| x == name).unwrap()];
    assert_eq!(kaname::effects::through(info("total_by")), ["amount"]);
    assert_eq!(kaname::effects::through(info("all_within")), Vec::<String>::new());
    assert_eq!(kaname::effects::through(info("sum_with")), Vec::<String>::new());
    // `total` written on a function that gives a lambda that can overflow: E401; on a function type
    // given one: E402
    let head = "module main v1\n\nfn apply_each[T](xs: list[T], f: (T) -> T) -> list[T]\n  xs.map(x => f(x))\n\nfn all_within(xs: list[number] count <=9, test: (number) -> bool total) -> bool\n  xs.all(x => test(x))\n\n";
    let o = check(&t, &format!("{head}fn bumped(xs: list[number] count <=9) -> list[number] total\n  apply_each(xs: xs, f: x => x + 1)\n"));
    assert_eq!(common::codes(&o), ["E401"]);
    let o = check(&t, &format!("{head}fn grows(xs: list[number] count <=9) -> bool\n  all_within(xs: xs, test: x => x + 1 > x)\n"));
    assert_eq!(common::codes(&o), ["E402"]);
    let said = o.diags[0].render(Lang::En);
    assert!(said.contains("fails overflow"), "{said}");
    let o = check(&t, &format!("{head}fn grows(xs: list[number] count <=9 range >=0 <=9) -> bool\n  all_within(xs: xs, test: x => x + 1 > x)\n"));
    assert_eq!(common::codes(&o), Vec::<&str>::new());
    // a function given by its name that can overflow, to a type written `total`: E402
    let o = check(&t, &format!("{head}fn next(n: number) -> bool\n  n + 1 > n\n\nfn grows(xs: list[number] count <=9) -> bool\n  all_within(xs: xs, test: next)\n"));
    assert_eq!(common::codes(&o), ["E402"]);
}

/// The functions over values of types that hold themselves (DESIGN 28.1): a function that walks one
/// recurses, and may not stop; giving such a value to a function is no place of `range` (a call holds
/// nothing inside it), wherever it comes from, three steps under the parameter too; a sum of what a
/// recursion answers, its range not written, is an overflow from the end of a type.
#[test]
fn the_effects_of_functions_over_values_that_hold_themselves() {
    let t = TempDir::new("kaname-effects-trees");
    let o = check(&t, include_str!("modules/trees.kn"));
    assert_eq!(common::codes(&o), ["W302"]);
    let c = o.analysed.as_ref().unwrap();
    let a = c.analysis.get().unwrap();
    let effect = |name: &str| a.fns[kaname::interp::function(c, name).unwrap()].effect.written();
    assert_eq!(effect("size"), "fails overflow diverges");
    assert_eq!(effect("names"), "diverges");
    assert_eq!(effect("chain"), "diverges");
    assert_eq!(effect("same"), "total");
    assert_eq!(effect("named"), "total");
    assert_eq!(effect("literal"), "total");
    assert_eq!(effect("wide_shares"), "fails overflow diverges");
    assert_eq!(effect("grand"), "fails overflow diverges");
    // no place of a parameter that takes a value of a type that holds itself is left to the code
    for f in &a.fns {
        for pl in &f.places {
            assert!(!(pl.kind == FailKind::Range && pl.outcome != kaname::ranges::Outcome::Shown), "{:?}", pl);
        }
    }
}
