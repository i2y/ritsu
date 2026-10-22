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

/// A temperature read in the other unit with that unit's zero, wherever it is read there: a constant
/// compared with a value (as a literal and kept in a `let`), given as an argument, held in a field,
/// taken by `max`, converted with `in`; and a value converted with `in` (DESIGN 3.2, 3.10). Both
/// versions of the module pass `check` with nothing said but the W501 of `frozen`, which makes a
/// value of a comparison with a threshold (DESIGN 7), so their `examples` hold.
#[test]
fn a_temperature_read_in_the_other_unit() {
    for (name, src, frozen) in [("temps", include_str!("modules/temps.kn"), 72), ("temps.ja", include_str!("modules/temps.ja.kn"), 71)] {
        let o = common::check_text("t.kn", src);
        let said: Vec<_> = o.diags.iter().map(|d| (d.code, d.line.unwrap_or(0))).collect();
        assert_eq!(said, [("W501", frozen)], "{name}: {}", common::shown(&o, Lang::En));
    }
    let c = module(include_str!("modules/temps.kn"));
    // 30℃ beside a ℉ is 86℉, written there and kept in a let; 50℉ beside a ℃ is 10℃
    assert_eq!(ok(&c, "hot", r#"{"t": 86}"#), r#""hot""#);
    assert_eq!(ok(&c, "hot", r#"{"t": 85}"#), r#""not hot""#);
    assert_eq!(ok(&c, "hot_by_line", r#"{"t": 86}"#), r#""hot""#);
    assert_eq!(ok(&c, "hot_by_line", r#"{"t": 85}"#), r#""not hot""#);
    assert_eq!(ok(&c, "cool", r#"{"t": 9}"#), r#""cool""#);
    assert_eq!(ok(&c, "cool", r#"{"t": 10}"#), r#""mild""#);
    // 0℃ is 32℉ and -40℃ is -40℉; 37℃ is 98.6℉, half up to 99℉; 100℉ is 37.8℃, half even to 38℃,
    // and 0℉ is -17.8℃, to -18℃
    assert_eq!(ok(&c, "in_fahrenheit", r#"{"t": 0}"#), "32");
    assert_eq!(ok(&c, "in_fahrenheit", r#"{"t": -40}"#), "-40");
    assert_eq!(ok(&c, "in_fahrenheit", r#"{"t": 37}"#), "99");
    assert_eq!(ok(&c, "in_celsius", r#"{"t": 100}"#), "38");
    assert_eq!(ok(&c, "in_celsius", r#"{"t": 0}"#), "-18");
    // 38℃ is 100.4℉, above 100℉; 37℃ is 98.6℉
    assert_eq!(ok(&c, "feverish", r#"{"t": 38}"#), r#""fever""#);
    assert_eq!(ok(&c, "feverish", r#"{"t": 37}"#), r#""no fever""#);
    // 100℃ in ℉ is 212℉; 20℃ in ℉ is 68℉, read back beside a ℃ as 20℃
    assert_eq!(ok(&c, "boiling", "{}"), "212");
    assert_eq!(ok(&c, "warm", r#"{"t": 20}"#), r#""warm""#);
    assert_eq!(ok(&c, "warm", r#"{"t": 19}"#), r#""not warm""#);
    // 99℉ in ℃ is 37.2℃, half up to 37℃
    assert_eq!(ok(&c, "fever_line", "{}"), "37");
    // 41℉ given to a ℃ is 5℃, 77℉ in a field of ℃ is 25℃, and 14℉ beside a ℃ in a `max` is -10℃
    assert_eq!(ok(&c, "fridge", "{}"), r#""cool""#);
    assert_eq!(ok(&c, "reading", "{}"), r#"{"at":25,"where":"porch"}"#);
    assert_eq!(ok(&c, "floor_of", r#"{"r": {"at": -20, "where": "freezer"}}"#), "-10");
    assert_eq!(ok(&c, "floor_of", r#"{"r": {"at": -9, "where": "freezer"}}"#), "-9");
}

/// A constant below zero of a temperature, read in the other unit with that unit's zero, and a
/// value read in the other unit with `in` (DESIGN 3.2, 3.10).
#[test]
fn a_temperature_below_zero() {
    let c = module(include_str!("modules/temps.kn"));
    assert_eq!(ok(&c, "frozen", r#"{"t": -18}"#), "true");
    assert_eq!(ok(&c, "frozen", r#"{"t": -17}"#), "false");
    // -20℃ beside a ℉ is -4℉
    assert_eq!(ok(&c, "deep_frozen", r#"{"t": -4}"#), r#""deep frozen""#);
    assert_eq!(ok(&c, "deep_frozen", r#"{"t": -3}"#), r#""above""#);
    // -4℉ beside a ℃ is -20℃
    assert_eq!(ok(&c, "below", r#"{"t": -21}"#), r#""below""#);
    assert_eq!(ok(&c, "below", r#"{"t": -20}"#), r#""at or above""#);
    // 0℃ is 32℉, -40℃ is -40℉; -18℃ is -0.4℉, half up to 0℉
    assert_eq!(ok(&c, "in_fahrenheit", r#"{"t": 0}"#), "32");
    assert_eq!(ok(&c, "in_fahrenheit", r#"{"t": -40}"#), "-40");
    assert_eq!(ok(&c, "in_fahrenheit", r#"{"t": -18}"#), "0");
    assert_eq!(ok(&c, "freezer_limit", "{}"), "-4");
    assert_eq!(ok(&c, "mark", r#"{"t": -40}"#), r#""the floor""#);
    assert_eq!(ok(&c, "owed", r#"{"balance": -100}"#), r#""owed""#);
    assert_eq!(ok(&c, "owed", r#"{"balance": -99}"#), r#""settled""#);
}

/// The values of an enum that have fields on the wire, a `match` that names the value it takes,
/// `else` and literals, a record made from another, and values compared whole (DESIGN 23.3–23.6).
#[test]
fn values_with_fields() {
    let c = module(include_str!("modules/sums.kn"));
    let draft = r#"{"draft": {"items": [{"sku": "a", "qty": 2, "price": 150}]}}"#;
    let placed = r#"{"placed": {"items": [{"sku": "a", "qty": 2, "price": 150}], "on": "2026-03-01"}}"#;
    // a value with fields is an object of one key, its name; a value with none, its name
    assert_eq!(ok(&c, "next", &format!(r#"{{"o": {draft}, "e": {{"place": {{"on": "2026-03-01"}}}}}}"#)), format!(r#"{{"moved":{{"order":{}}}}}"#, placed.replace(' ', "")));
    assert_eq!(ok(&c, "next", &format!(r#"{{"o": {draft}, "e": "reopen"}}"#)), r#"{"refused":{"why":"a draft is placed first"}}"#);
    assert_eq!(ok(&c, "next", &format!(r#"{{"o": {placed}, "e": "reopen"}}"#)), r#"{"moved":{"order":{"draft":{"items":[{"sku":"a","qty":2,"price":150}]}}}}"#);
    assert_eq!(ok(&c, "next", r#"{"o": {"cancelled": {"reason": "late"}}, "e": "reopen"}"#), r#"{"refused":{"why":"the order is closed"}}"#);
    // the amount paid is the total: 300 cents
    assert_eq!(ok(&c, "next", &format!(r#"{{"o": {placed}, "e": {{"pay": {{"cents": 300}}}}}}"#)), r#"{"moved":{"order":{"paid":{"items":[{"sku":"a","qty":2,"price":150}],"paid":300}}}}"#);
    assert_eq!(ok(&c, "next", &format!(r#"{{"o": {placed}, "e": {{"pay": {{"cents": 299}}}}}}"#)), r#"{"refused":{"why":"the amount is not the total"}}"#);
    // two values of the sum compared whole: placed again on the same day stays as it is
    assert_eq!(ok(&c, "stays", &format!(r#"{{"o": {placed}, "e": {{"place": {{"on": "2026-03-01"}}}}}}"#)), "true");
    assert_eq!(ok(&c, "stays", &format!(r#"{{"o": {placed}, "e": {{"place": {{"on": "2026-03-02"}}}}}}"#)), "false");
    // the type of a value: what a host gives is the value, on the wire as the sum's
    assert_eq!(ok(&c, "placed_on", &format!(r#"{{"p": {placed}}}"#)), r#""2026-03-01""#);
    assert_eq!(ok(&c, "moved_to", &format!(r#"{{"p": {placed}, "on": "2027-01-02"}}"#)), r#"{"placed":{"items":[{"sku":"a","qty":2,"price":150}],"on":"2027-01-02"}}"#);
    // literals, read in the unit split: `-1`, `0USDc`, a date, a string, and `else`
    assert_eq!(ok(&c, "size", r#"{"n": -1}"#), r#""minus one""#);
    assert_eq!(ok(&c, "size", r#"{"n": 7}"#), r#""more""#);
    assert_eq!(ok(&c, "label", r#"{"c": 0}"#), r#""free""#);
    assert_eq!(ok(&c, "holiday", r#"{"d": "2026-12-25"}"#), "true");
    assert_eq!(ok(&c, "grade", r#"{"code": "C"}"#), "0");
    // lists, values that may be none, records: compared whole
    assert_eq!(ok(&c, "same_items", r#"{"a": [{"sku": "a", "qty": 1, "price": 1}], "b": [{"sku": "a", "qty": 1, "price": 1}]}"#), "true");
    assert_eq!(ok(&c, "same_items", r#"{"a": [{"sku": "a", "qty": 1, "price": 1}], "b": []}"#), "false");
    assert_eq!(ok(&c, "same_count", r#"{"a": null, "b": null}"#), "true");
    assert_eq!(ok(&c, "same_count", r#"{"a": 2, "b": null}"#), "false");
    assert_eq!(ok(&c, "differ", r#"{"a": {"sku": "a", "qty": 1, "price": 1}, "b": {"sku": "a", "qty": 1, "price": 2}}"#), "true");
    // what a host gives is held to the value's fields: the day placed, the value's name, its shape
    let (k, line, msg) = run(&c, "next", r#"{"o": {"placed": {"items": [{"sku": "a", "qty": 1, "price": 1}], "on": "2028-01-01"}}, "e": "reopen"}"#).unwrap_err();
    assert_eq!((k, line, msg.as_str()), (Kind::Input, 41, "The argument `o.placed.on` of `next` is 2028-01-01, outside its range >=2026-01-01 <=2027-12-31"));
    let (k, _, msg) = run(&c, "next", r#"{"o": "placed", "e": "reopen"}"#).unwrap_err();
    assert_eq!((k, msg.as_str()), (Kind::Input, "An argument of `next`: `o` is \"placed\", a value of the enum `Order` that has fields, given as `{\"placed\": {…}}`"));
    let (k, _, _) = run(&c, "next", r#"{"o": {"shipped": {}}, "e": "reopen"}"#).unwrap_err();
    assert_eq!(k, Kind::Input);
    let (k, _, _) = run(&c, "placed_on", r#"{"p": {"draft": {"items": []}}}"#).unwrap_err();
    assert_eq!(k, Kind::Input);
}

/// A record made from another: the value after `..` first, then the fields written; each field
/// that declares a range is held to it, the ones taken as well (DESIGN 23.4, 23.6).
#[test]
fn a_record_made_from_another() {
    let c = module(
        "module upd v1\n\nrecord Item\n  sku   : string\n  price : money[USDc]  range >=1USDc <=100USDc\n\nfn repriced(i: Item, by: money[USDc] range >=0USDc <=100USDc) -> Item\n  Item { ..i, price: i.price + by }\n\nfn renamed(i: Item) -> Item\n  Item { ..i, sku: \"b\" }\n",
    );
    assert_eq!(ok(&c, "repriced", r#"{"i": {"sku": "a", "price": 50}, "by": 10}"#), r#"{"sku":"a","price":60}"#);
    assert_eq!(fails(&c, "repriced", r#"{"i": {"sku": "a", "price": 50}, "by": 51}"#), (Kind::Range, 8));
    assert_eq!(ok(&c, "renamed", r#"{"i": {"sku": "a", "price": 50}}"#), r#"{"sku":"b","price":50}"#);
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

const DICTS: &str = include_str!("modules/dicts.kn");
const LISTS2: &str = include_str!("modules/lists2.kn");

/// Dicts and sets (DESIGN 24.2, 24.4): on the wire, a dict is the array of its pairs and a set the
/// array of its members, both in the order of their keys; an input in another order is put in that
/// order, and a key or a member given twice is the failure `input`. Keys go in the order their type
/// gives: numbers from the least, strings by their code points, dates from the earliest, the values
/// of an enum in the order they are declared, a record field by field.
#[test]
fn dicts_and_sets_go_in_the_order_of_their_keys() {
    let c = module(DICTS);
    assert_eq!(
        ok(&c, "diff", r#"{"before": [["b", 2], ["a", 1]], "after": [["c", 4], ["b", 3]]}"#),
        r#"[{"removed":{"sku":"a","qty":1}},{"changed":{"sku":"b","was":2,"now":3}},{"added":{"sku":"c","qty":4}}]"#
    );
    assert_eq!(fails(&c, "diff", r#"{"before": [["a", 1], ["a", 2]], "after": []}"#), (Kind::Input, 45));
    assert_eq!(fails(&c, "both", r#"{"a": ["a", "a"], "b": []}"#).0, Kind::Input);
    let (_, _, msg) = run(&c, "diff", r#"{"before": [["a", 1], ["a", 2]], "after": []}"#).unwrap_err();
    assert_eq!(msg, "An argument of `diff`: `before` has the key \"a\" twice");
    // the orders of the keys' types
    assert_eq!(ok(&c, "marks", "{}"), r#"["z","｡","😀"]"#);
    assert_eq!(ok(&c, "days", r#"{"ds": ["2026-05-01", "2026-02-01", "2026-05-01"]}"#), r#"["2026-02-01","2026-05-01"]"#);
    assert_eq!(ok(&c, "rates", r#"{"rs": [5, 3, 5]}"#), "[3,5]");
    let tasks = r#"{"tasks": [{"name": "x", "priority": 2, "tier": "gold"}, {"name": "y", "priority": 1, "tier": "gold"}, {"name": "z", "priority": 3, "tier": "basic"}]}"#;
    assert_eq!(ok(&c, "per_tier", tasks), r#"[["basic",1],["gold",2]]"#);
    assert_eq!(ok(&c, "tiers", tasks), r#"["basic","gold"]"#);
    assert_eq!(ok(&c, "names_by_priority", tasks), r#"[[1,["y"]],[2,["x"]],[3,["z"]]]"#);
    assert_eq!(ok(&c, "ranked", tasks), r#"["z","y","x"]"#);
    // what a dict answers is in the order of its keys, a later pair wins
    assert_eq!(ok(&c, "table", "{}"), r#"[["a",100],["b",200]]"#);
    assert_eq!(ok(&c, "combined", r#"{"a": [["b", 2], ["a", 1]], "b": [["b", 3], ["c", 9]]}"#), r#"[["a",1],["b",5],["c",9]]"#);
    assert_eq!(ok(&c, "later", r#"{"a": [["b", 2], ["a", 1]], "b": [["b", 3]]}"#), r#"[["a",1],["b",3]]"#);
    assert_eq!(ok(&c, "restock", r#"{"stock": [["a", 1]], "sku": "a", "qty": 2}"#), r#"[["a",3]]"#);
    assert_eq!(ok(&c, "restock", r#"{"stock": [["b", 1]], "sku": "a", "qty": 2}"#), r#"[["a",2],["b",1]]"#);
    assert_eq!(ok(&c, "without", r#"{"stock": [["a", 1], ["b", 2]], "sku": "c"}"#), r#"[["a",1],["b",2]]"#);
    assert_eq!(ok(&c, "in_stock", r#"{"stock": [["a", 0], ["b", 2]]}"#), r#"[["b",2]]"#);
    assert_eq!(ok(&c, "shown", r#"{"stock": [["b", 0], ["a", 2]]}"#), r#"["a=2","b=0"]"#);
    assert_eq!(ok(&c, "quantities", r#"{"lines": [{"sku": "b", "qty": 1, "price": 0}, {"sku": "a", "qty": 2, "price": 0}, {"sku": "b", "qty": 3, "price": 0}]}"#), r#"[["a",2],["b",3]]"#);
    // two dicts or two sets are equal when they hold the same, whatever order they came in
    assert_eq!(ok(&c, "same_stock", r#"{"a": [["b", 2], ["a", 1]], "b": [["a", 1], ["b", 2]]}"#), "true");
    assert_eq!(ok(&c, "same_stock", r#"{"a": [["a", 1]], "b": [["a", 2]]}"#), "false");
    assert_eq!(ok(&c, "same_tiers", r#"{"a": ["gold", "basic"], "b": ["basic", "gold"]}"#), "true");
    assert_eq!(ok(&c, "both", r#"{"a": ["c", "a", "b"], "b": ["b", "c"]}"#), r#"["b","c"]"#);
}

/// `mod` by a value answers from 0 to one less than the size of the divisor, whatever the signs
/// (DESIGN 24.3), and fails `zero` where the divisor is 0; `numbers` counts from the first to the last,
/// none where the last is before the first.
#[test]
fn a_remainder_by_a_value_and_the_numbers_between_two() {
    let c = module(DICTS);
    for (day, players, want) in [(-3, 4, "1"), (3, 4, "3"), (-8, 4, "0"), (10, 1, "0")] {
        assert_eq!(ok(&c, "turn", &format!(r#"{{"day": {day}, "players": {players}}}"#)), want, "mod({day}, {players})");
    }
    for (x, n, want) in [(7, -3, "1"), (-7, -3, "2"), (-1, -1, "0")] {
        assert_eq!(ok(&c, "wrapped", &format!(r#"{{"x": {x}, "n": {n}}}"#)), want, "mod({x}, {n})");
    }
    assert_eq!(ok(&c, "spread", r#"{"n": 5}"#), "[2,4,1,3,0]");
    assert_eq!(ok(&c, "spread", r#"{"n": 0}"#), "[]");
    assert_eq!(ok(&c, "between", r#"{"first": 2, "last": 4}"#), "9");
    assert_eq!(ok(&c, "between", r#"{"first": 3, "last": 1}"#), "0");
    let z = module("module z v1\n\nfn rem(x: number range >=0 <=9, n: number range >=0 <=3) -> number\n  mod(x, n)\n");
    assert_eq!(ok(&z, "rem", r#"{"x": 7, "n": 3}"#), "1");
    assert_eq!(fails(&z, "rem", r#"{"x": 7, "n": 0}"#), (Kind::Zero, 4));
}

/// The builtins of a list (DESIGN 24.3): an order keeps two elements of one key in the order of the
/// list, the places past a list's ends cut it and fail nothing, `last` finds from the end.
#[test]
fn the_builtins_of_lists_keep_their_order_and_cut_where_they_end() {
    let c = module(LISTS2);
    let items = r#"{"items": [{"name": "b", "price": 5, "qty": 1}, {"name": "a", "price": 2, "qty": 1}, {"name": "c", "price": 2, "qty": 1}, {"name": "d", "price": 5, "qty": 0}]}"#;
    assert_eq!(ok(&c, "by_price", items), r#"["a","c","b","d"]"#);
    assert_eq!(ok(&c, "cheapest", items), r#"{"name":"a","price":2,"qty":1}"#);
    assert_eq!(ok(&c, "dearest", items), r#""b""#);
    assert_eq!(ok(&c, "dearest", r#"{"items": []}"#), "null");
    assert_eq!(ok(&c, "ordered", r#"{"xs": [3, -5, 0, 3]}"#), "[-5,0,3,3]");
    assert_eq!(ok(&c, "unique", r#"{"xs": ["b", "a", "b", "c"]}"#), r#"["b","a","c"]"#);
    for (start, stop, want) in [(-1, 2, "[1,2]"), (3, 1, "[]"), (2, 5, "[3,4]"), (-1, 5, "[1,2,3,4]")] {
        assert_eq!(ok(&c, "cut", &format!(r#"{{"xs": [1, 2, 3, 4], "start": {start}, "stop": {stop}}}"#)), want, "slice({start}, {stop})");
    }
    assert_eq!(ok(&c, "turned", r#"{"xs": [1, 2, 3, 4], "n": -1}"#), "[4,3,2,1]");
    assert_eq!(ok(&c, "turned", r#"{"xs": [1, 2, 3, 4], "n": 5}"#), "[1,2,3,4]");
    assert_eq!(ok(&c, "picked", r#"{"xs": ["a", "b"], "i": -1}"#), "null");
    assert_eq!(ok(&c, "picked", r#"{"xs": ["a", "b"], "i": 1}"#), r#""b""#);
    assert_eq!(ok(&c, "last_above", r#"{"xs": [7, 1, 9, 2], "limit": 5}"#), "9");
    assert_eq!(ok(&c, "last_above", r#"{"xs": [7, 1, 9, 2], "limit": 9}"#), "null");
    assert_eq!(ok(&c, "ends", r#"{"xs": []}"#), "-1");
    assert_eq!(ok(&c, "flat", r#"{"xss": [[1], [], [2, 3]]}"#), "[1,2,3]");
    assert_eq!(ok(&c, "has", r#"{"xs": [{"name": "b", "price": 2, "qty": 1}], "x": {"name": "b", "price": 2, "qty": 2}}"#), "false");
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

/// Values of types that hold themselves (DESIGN 28.1): a host's argument nests them 200 deep at most,
/// one 201 deep failing `input` with the type named, before what follows walks it; a call of kaname
/// holds nothing inside such a value (a rate with no range past 100%, built here, is given on), and a
/// host's argument is held inside it at every depth.
#[test]
fn values_that_hold_themselves() {
    // on a stack as deep as 200 calls need (DESIGN 8)
    std::thread::Builder::new().stack_size(256 << 20).spawn(values_that_hold_themselves_deep).unwrap().join().unwrap();
}

fn values_that_hold_themselves_deep() {
    let c = module(include_str!("modules/trees.kn"));
    let p = Program::of(&c);
    let suite = common::joined();
    let size = kaname::interp::function(&c, "size").unwrap();
    let t = c.decls.fns[size].params[0].slot.ty.clone();
    let deep = |n: usize| kaname::vectors::nested_value(&p, &t, n).unwrap();
    assert_eq!((p.nesting(&deep(200), &t), p.nesting(&deep(201), &t)), (200, 201));
    let mut it = Interp::new(&p, &suite);
    assert!(it.check_entry(&c, size, &[deep(200)]).is_ok());
    let fl = it.check_entry(&c, size, &[deep(201)]).unwrap_err();
    assert_eq!((fl.kind, fl.line), (Kind::Input, c.decls.fns[size].line));
    assert_eq!(fl.message.en, "The argument `c` of `size` nests `Category` more than 200 deep");
    assert_eq!(fl.message.ja, "関数 `size` の引数 `c` は、`Category` の入れ子が 200 段を超えています");
    // 200 calls deep, as deep as the interpreter goes (DESIGN 8)
    match it.call(&c, size, vec![deep(200)], 0) {
        Ok(v) => assert_eq!(p.to_json(&v, &c.decls.fns[size].ret.ty).compact(), "200"),
        other => panic!("{other:?}"),
    }
    // a value of an enum and of two types that hold each other, nested as deep
    for (name, ty) in [("value_of", "Expr"), ("values_in", "Doc")] {
        let f = kaname::interp::function(&c, name).unwrap();
        let t = c.decls.fns[f].params[0].slot.ty.clone();
        let v = kaname::vectors::nested_value(&p, &t, 201).unwrap();
        let fl = Interp::new(&p, &suite).check_entry(&c, f, &[v]).unwrap_err();
        assert!(fl.message.en.contains(&format!("nests `{ty}` more than 200 deep")), "{}", fl.message.en);
    }
    // a call of kaname gives a share of 500% on; a host's share past 100% fails, inside the value too
    assert_eq!(ok(&c, "wide_shares", r#"{"p": 500}"#), "2");
    let shares = c.decls.fns[kaname::interp::function(&c, "shares").unwrap()].line;
    assert_eq!(fails(&c, "shares", r#"{"s": {"part": 101, "under": []}}"#), (Kind::Input, shares));
    match run(&c, "shares", r#"{"s": {"part": 1, "under": [{"part": 150, "under": []}]}}"#) {
        Err((Kind::Input, line, msg)) => {
            assert_eq!(line, shares);
            assert_eq!(msg, "The argument `s.under[0].part` of `shares` is 150%, outside its range >=0% <=100%");
        }
        other => panic!("{other:?}"),
    }
    // a tree built, compared and counted
    assert_eq!(ok(&c, "size", r#"{"c": {"name": "a", "children": [{"name": "b", "children": [{"name": "c", "children": []}]}]}}"#), "3");
    assert_eq!(ok(&c, "same", r#"{"a": {"name": "a", "children": []}, "b": {"name": "a", "children": []}}"#), "true");
    assert_eq!(ok(&c, "length", r#"{"n": {"value": 1, "next": {"value": 2, "next": {"value": 3, "next": null}}}}"#), "3");
}
