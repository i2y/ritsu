//! The reference interpreter (DESIGN 8), with rulec, koyomi and chobo joined through their own
//! engines as `ritsu kaname` joins them: what a module answers, the failures and where they are, the
//! order the evaluation goes in, the places that hold values to what they declare, and a recursion
//! that does not stop.

mod common;

use kaname::check::Checked;
use kaname::interp::{Event, Interp, Kind, Stop};
use kaname::value::Program;
use ritsu_base::json::{self, Json};
use ritsu_base::text::Lang;
use std::rc::Rc;

/// The module `src`, as the file `t.kn` beside the crate (so `use` reads the examples by
/// their paths), whose names and types pass. The modules here fail on purpose when they run, and the
/// analysis of ranges says where (E301–E303, W301–W303): what the interpreter does there is what is
/// tested.
fn module(src: &str) -> Rc<Checked> {
    let o = common::check_text("t.kn", src);
    assert!(common::codes_before_analysis(&o).is_empty(), "{}", common::shown(&o, Lang::En));
    o.analysed.clone().unwrap()
}

/// What `name` of `c` comes to on the JSON `input`: its answer on the wire, or its failure (the kind,
/// the line, what it says).
fn run(c: &Rc<Checked>, name: &str, input: &str) -> Result<Json, (Kind, usize, String)> {
    let p = Program::of(c);
    let suite = common::joined();
    let mut it = Interp::new(&p, &suite);
    let f = kaname::interp::function(c, name).unwrap_or_else(|| panic!("no function {name}"));
    let args = it.enter_json(c, f, &json::parse(input).unwrap()).map_err(|fl| (fl.kind, fl.line, fl.message.en.clone()))?;
    match it.call(c, f, args, 0) {
        Ok(v) => Ok(p.to_json(&v, &c.decls.fns[f].ret.ty)),
        Err(Stop::Failed(fl)) => Err((fl.kind, fl.line, fl.message.en.clone())),
        Err(Stop::Diverged(d)) => Err((Kind::Range, d.line, format!("diverged at {}", d.depth))),
    }
}

/// The answer, compact.
fn ok(c: &Rc<Checked>, name: &str, input: &str) -> String {
    match run(c, name, input) {
        Ok(j) => j.compact(),
        Err(e) => panic!("{name}({input}): {e:?}"),
    }
}

/// The kind and the line of the failure.
fn fails(c: &Rc<Checked>, name: &str, input: &str) -> (Kind, usize) {
    match run(c, name, input) {
        Ok(j) => panic!("{name}({input}) answers {}", j.compact()),
        Err((k, line, _)) => (k, line),
    }
}

const CALC: &str = include_str!("modules/calc.kn");

#[test]
fn numbers_in_their_units() {
    let c = module(CALC);
    // a weight times a count, added up, in ounces
    assert_eq!(ok(&c, "total_weight", r#"{"lines": [{"qty": 2, "weight": 22}, {"qty": 1, "weight": 12}]}"#), "56");
    assert_eq!(ok(&c, "total_weight", r#"{"lines": []}"#), "0");
    // 15lb is read in ounces: 240
    assert_eq!(ok(&c, "heavy", r#"{"w": 240}"#), "false");
    assert_eq!(ok(&c, "heavy", r#"{"w": 241}"#), "true");
    // a `let` with a type reads its constant in it, `in` converts
    assert_eq!(ok(&c, "pounds", r#"{"w": 3}"#), "80");
    assert_eq!(ok(&c, "in_cents", r#"{"d": 12}"#), "1200");
    // rulec's fee: 3.49% and 49 cents, half up to a cent
    assert_eq!(ok(&c, "fee", r#"{"a": 10000}"#), "398");
    assert_eq!(ok(&c, "fee", r#"{"a": 1}"#), "49");
    // a ratio rounded to a rate's step: 1/3 is 33.3%, on the wire 333 steps of 0.1%
    assert_eq!(ok(&c, "ratio", r#"{"a": 1, "b": 3}"#), "333");
    // the difference of two dates is the days between them
    assert_eq!(ok(&c, "days", r#"{"a": "2026-03-02", "b": "2026-04-01"}"#), "30");
    assert_eq!(ok(&c, "days", r#"{"a": "2026-04-01", "b": "2026-03-02"}"#), "-30");
}

/// A temperature in another unit is read with that unit's zero (DESIGN 3.2): 30℃ beside a ℉ is
/// 86℉, not 54℉.
#[test]
fn a_temperature_is_read_with_its_zero() {
    let c = module("module temp v1\n\nfn warm(t: temperature[℉] range >=0℉ <=200℉) -> bool\n  if t > 30℃ then true else false\n\nfn cold(t: temperature[℃] range >=-40℃ <=50℃) -> bool\n  if t < 32℉ then true else false\n");
    assert_eq!(ok(&c, "warm", r#"{"t": 86}"#), "false");
    assert_eq!(ok(&c, "warm", r#"{"t": 87}"#), "true");
    assert_eq!(ok(&c, "cold", r#"{"t": 0}"#), "false");
    assert_eq!(ok(&c, "cold", r#"{"t": -1}"#), "true");
}

#[test]
fn what_cannot_be_computed_fails_where_it_is() {
    let c = module(CALC);
    // a division by a value that is 0 (DESIGN 5.6)
    assert_eq!(fails(&c, "per", r#"{"a": 100, "n": 0}"#), (Kind::Zero, 27));
    assert_eq!(ok(&c, "per", r#"{"a": 100, "n": 3}"#), "33");
    // a value that leaves int64
    assert_eq!(fails(&c, "big", r#"{"a": 5000000000000000000}"#), (Kind::Overflow, 33));
    assert_eq!(ok(&c, "big", r#"{"a": 4000000000000000000}"#), "8000000000000000000");
    let (_, _, msg) = run(&c, "big", r#"{"a": 5000000000000000000}"#).unwrap_err();
    assert_eq!(msg, "The value of `a * 2` does not fit in int64");
}

#[test]
fn the_order_of_the_evaluation() {
    let c = module(CALC);
    // the right of `and` is not evaluated once the left decides: no division by 0
    assert_eq!(ok(&c, "safe", r#"{"a": 5, "b": 0}"#), "false");
    assert_eq!(ok(&c, "safe", r#"{"a": 5, "b": 2}"#), "true");
    // the arguments from the left, as written: the first that fails is the one said
    assert_eq!(fails(&c, "left_first", r#"{"a": 3, "z": 0}"#).0, Kind::Zero);
    assert_eq!(fails(&c, "right_first", r#"{"a": 3, "z": 0}"#).0, Kind::Overflow);
}

#[test]
fn a_host_gives_what_the_parameters_declare() {
    let c = module(CALC);
    // a value outside its range, a list longer than its count, a field missing, a wrong type, a name
    // that is no parameter: the failure `input`, at the function's line, before anything is computed
    assert_eq!(fails(&c, "heavy", r#"{"w": 1001}"#), (Kind::Input, 10));
    assert_eq!(fails(&c, "total_weight", r#"{"lines": [{"qty": 1, "weight": 1}, {"qty": 1, "weight": 1}, {"qty": 1, "weight": 1}, {"qty": 1, "weight": 1}, {"qty": 1, "weight": 1}, {"qty": 1, "weight": 1}]}"#).0, Kind::Input);
    assert_eq!(fails(&c, "total_weight", r#"{"lines": [{"qty": 21, "weight": 1}]}"#).0, Kind::Input);
    assert_eq!(fails(&c, "total_weight", r#"{"lines": [{"qty": 2}]}"#).0, Kind::Input);
    assert_eq!(fails(&c, "heavy", r#"{"w": "heavy"}"#).0, Kind::Input);
    assert_eq!(fails(&c, "heavy", r#"{"w": 2, "x": 1}"#).0, Kind::Input);
    assert_eq!(fails(&c, "days", r#"{"a": "2026-02-30", "b": "2026-04-01"}"#).0, Kind::Input);
    let (_, _, msg) = run(&c, "total_weight", r#"{"lines": [{"qty": 21, "weight": 1}]}"#).unwrap_err();
    assert_eq!(msg, "The argument `lines[0].qty` of `total_weight` is 21, outside its range >=1 <=20");
}

const PLACES: &str = include_str!("modules/places.kn");

#[test]
fn the_places_hold_what_they_declare() {
    let c = module(PLACES);
    // a value given to another function's parameter, put in a record's field, answered: `range`
    assert_eq!(ok(&c, "outer", r#"{"n": 5}"#), "5");
    assert_eq!(fails(&c, "outer", r#"{"n": 6}"#), (Kind::Range, 12));
    assert_eq!(fails(&c, "boxed", r#"{"n": 6}"#), (Kind::Range, 15));
    assert_eq!(fails(&c, "capped", r#"{"n": 6}"#), (Kind::Range, 18));
    // isqrt has no answer below zero
    assert_eq!(ok(&c, "root", r#"{"n": 9}"#), "3");
    assert_eq!(fails(&c, "root", r#"{"n": -4}"#), (Kind::Range, 21));
    // a rate with a step and no range is held to 0% to 100% at the entry (DESIGN 3.9)
    assert_eq!(ok(&c, "share", r#"{"r": 100}"#), "100");
    assert_eq!(fails(&c, "share", r#"{"r": 101}"#).0, Kind::Input);
}

#[test]
fn enums_and_values_that_may_be_none() {
    let c = module(PLACES);
    assert_eq!(ok(&c, "better", r#"{"a": "gold", "b": "silver"}"#), "\"gold\"");
    assert_eq!(ok(&c, "better", r#"{"a": "basic", "b": "silver"}"#), "\"silver\"");
    assert_eq!(ok(&c, "code", r#"{"t": "basic"}"#), "\"S\"");
    assert_eq!(ok(&c, "code", r#"{"t": "gold"}"#), "\"G\"");
    assert_eq!(fails(&c, "code", r#"{"t": "platinum"}"#).0, Kind::Input);
    assert_eq!(ok(&c, "or_zero", r#"{"x": null}"#), "0");
    assert_eq!(ok(&c, "or_zero", r#"{"x": 4}"#), "5");
    // an optional parameter may be left out
    assert_eq!(ok(&c, "or_zero", "{}"), "0");
}

const LISTS: &str = include_str!("modules/lists.kn");

#[test]
fn the_builtins_walk_lists() {
    let c = module(LISTS);
    let lines = r#"{"lines": [{"label": "apple", "amount": 120, "media": false}, {"label": "book", "amount": 1500, "media": true}, {"label": "bag", "amount": 0, "media": true}]}"#;
    assert_eq!(
        ok(&c, "stats", lines),
        r#"{"count":3,"media":2,"total":1620,"largest":1500,"first":"book","all":false,"any":true,"labels":["apple=120USDc","book=1500USDc"]}"#
    );
    assert_eq!(ok(&c, "stats", r#"{"lines": []}"#), r#"{"count":0,"media":0,"total":0,"largest":null,"first":null,"all":true,"any":false,"labels":[]}"#);
    assert_eq!(
        ok(&c, "totals", lines),
        r#"[{"label":"apple","before":0,"upto":120,"total":1620},{"label":"book","before":120,"upto":1620,"total":1620},{"label":"bag","before":1620,"upto":1620,"total":1620}]"#
    );
    assert_eq!(ok(&c, "product", r#"{"xs": [2, 3, 4]}"#), "24");
    assert_eq!(ok(&c, "product", r#"{"xs": []}"#), "1");
    assert_eq!(ok(&c, "smallest", r#"{"xs": ["2026-05-01", "2026-02-01", "2026-03-01"]}"#), "\"2026-02-01\"");
    assert_eq!(ok(&c, "smallest", r#"{"xs": []}"#), "null");
}

const INNER: &str = include_str!("modules/inner.kn");

#[test]
fn rules_dates_and_transfers() {
    let c = module(INNER);
    // the rule answers through rulec's engine, the date through koyomi's
    assert_eq!(ok(&c, "charge", r#"{"days": 50}"#), "1150");
    assert_eq!(fails(&c, "charge", r#"{"days": 0}"#), (Kind::Rule, 9));
    // the message is what the generated code says too; the reason is the trace's (DESIGN 10.4)
    let (_, _, msg) = run(&c, "charge", r#"{"days": 0}"#).unwrap_err();
    assert_eq!(msg, "The rule `charges` refuses its inputs");
    let p = Program::of(&c);
    let suite = common::joined();
    let mut it = Interp::new(&p, &suite);
    let f = kaname::interp::function(&c, "charge").unwrap();
    let args = it.enter_json(&c, f, &json::parse(r#"{"days": 0}"#).unwrap()).unwrap();
    assert!(it.call(&c, f, args, 0).is_err());
    let called: Vec<&str> = it.trace.calls().map(|x| x.text.as_str()).collect();
    assert!(called.iter().any(|t| t.contains("outside its range")), "{called:?}");
    assert_eq!(ok(&c, "due", r#"{"on": "2026-03-02"}"#), "\"2026-03-31\"");
    assert_eq!(fails(&c, "due", r#"{"on": "2025-06-01"}"#), (Kind::Date, 12));
    assert_eq!(ok(&c, "gap", r#"{"on": "2027-12-31"}"#), "29");
    // a rule's output that may be none comes as none (DESIGN 3.8)
    assert_eq!(ok(&c, "discount", r#"{"c": null}"#), "0");
    assert_eq!(ok(&c, "discount", r#"{"c": 250}"#), "250");
    // a transfer is a value; an amount below 0 is `range` (DESIGN 6.3, 6.4)
    assert_eq!(ok(&c, "pay", r#"{"id": "p1", "amount": 500, "fee": 49}"#), r#"{"book":"ledger","transfer":"capture","op":"do","args":{"payment":"p1","seller":"s_1","amount":451}}"#);
    assert_eq!(fails(&c, "pay", r#"{"id": "p1", "amount": 1, "fee": 49}"#), (Kind::Range, 23));
}

#[test]
fn the_trace_keeps_the_calls_and_the_lets_in_order() {
    let c = common::check("examples/late_payment/late_payment.kn").checked.clone().unwrap();
    let p = Program::of(&c);
    let suite = common::joined();
    let mut it = Interp::new(&p, &suite);
    it.lets = true;
    let input = r#"{"invoice": {"id": "inv_1", "invoiced": "2026-03-02", "amount": 100000}, "paid_on": "2026-05-20", "reference_rate": 50}"#;
    let args = it.enter_json(&c, 0, &json::parse(input).unwrap()).unwrap();
    let v = it.call(&c, 0, args, 0).unwrap();
    assert_eq!(p.show(&c, &v, &c.decls.fns[0].ret.ty), "Claim { due: 2026-03-31, days_late: 50, compensation: 70GBP, interest: 1150GBPc }");
    let seen: Vec<String> = it
        .trace
        .events
        .iter()
        .map(|e| match e {
            Event::Call(x) => format!("{}: {}", x.line, x.text),
            Event::Let(b) => format!("{}: let {} = {}", b.line, b.name, b.text),
        })
        .collect();
    assert_eq!(
        seen,
        [
            "20: terms.due(invoiced: 2026-03-02) = 2026-03-31",
            "20: let due = 2026-03-31",
            "21: let late = 50",
            "23: charges(debt: 100000GBPc, days_late: 50, reference_rate: 0.5%) = { compensation: 70GBP, interest: 1150GBPc }",
            "23: let c = { compensation: 70GBP, interest: 1150GBPc }",
        ]
    );
}

/// A recursion of kaname's functions goes as deep as [`kaname::interp::DEPTH`], and a call below it
/// stops the evaluation without a failure (DESIGN 8). It runs on a stack of its own, as deep as the
/// evaluation needs, the way the binary's main thread would give it.
#[test]
fn a_recursion_that_does_not_stop() {
    let deep = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(|| {
            let c = module(include_str!("modules/rec.kn"));
            let p = Program::of(&c);
            let suite = common::joined();
            let at = |n: i128| {
                let mut it = Interp::new(&p, &suite);
                it.call(&c, 0, vec![kaname::value::Value::Num(ritsu_units::Rat::int(n))], 0).map(|v| p.to_json(&v, &c.decls.fns[0].ret.ty).compact())
            };
            let depth = kaname::interp::DEPTH as i128;
            let reached = at(depth - 1);
            let stopped = at(depth);
            (reached, stopped)
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(deep.0, Ok((kaname::interp::DEPTH - 1).to_string()));
    match deep.1 {
        Err(Stop::Diverged(d)) => assert_eq!((d.function.as_str(), d.depth, d.line), ("countdown", kaname::interp::DEPTH, 3)),
        other => panic!("{other:?}"),
    }
}

/// A function of another module, called with a record of that module: evaluated in its own module,
/// its failures said with that module's name and line.
#[test]
fn a_function_of_another_module() {
    let t = ritsu_testkit::TempDir::new("kaname-interp-modules");
    // the answer goes above its range for a quantity of 5 alone, which none of the vectors of
    // `line_total` tries: the analysis cannot decide it (W301), so the module passes
    t.write("pricing.kn", "module pricing v1\n\nrecord Line\n  qty   : number  range >=1 <=8\n  price : money[USDc]  range >=0USDc <=1_000USDc\n\nfn line_total(line: Line) -> money[USDc] range >=0USDc <=24_000USDc\n  line.price * (line.qty * (10 - line.qty))\n");
    let main = t.write(
        "main.kn",
        "module main v1\n\nuse module pricing from \"pricing.kn\"\n\nfn total(a: pricing.Line, b: pricing.Line) -> money[USDc]\n  pricing.line_total(line: a) + pricing.line_total(line: b)\n",
    );
    let o = kaname::check::Run::new(common::joined()).file(main.to_str().unwrap()).unwrap();
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    let c = o.checked.clone().unwrap();
    assert_eq!(ok(&c, "total", r#"{"a": {"qty": 2, "price": 150}, "b": {"qty": 1, "price": 99}}"#), "3291");
    // 1,000 cents times 5 × 5 is above what `line_total` answers: its own module and line
    let p = Program::of(&c);
    let suite = common::joined();
    let mut it = Interp::new(&p, &suite);
    let args = it.enter_json(&c, 0, &json::parse(r#"{"a": {"qty": 5, "price": 1000}, "b": {"qty": 1, "price": 1}}"#).unwrap()).unwrap();
    match it.call(&c, 0, args, 0) {
        Err(Stop::Failed(fl)) => {
            assert_eq!((fl.kind, fl.module.as_str(), fl.line), (Kind::Range, "pricing", 8));
            assert_eq!(fl.message.en, "The answer of `line_total`, 25000USDc, is outside its range >=0USDc <=24000USDc");
            assert_eq!(fl.stack, [("total".to_string(), 0), ("line_total".to_string(), 6)]);
        }
        other => panic!("{other:?}"),
    }
}
