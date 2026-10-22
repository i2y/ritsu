//! The analysis of ranges (DESIGN 5, 16.1): an estimate from above. Every value the reference
//! interpreter gives an expression, on every vector of every function of the examples and of
//! the test modules, is among the values the analysis says the expression can be; and what the
//! analysis narrows, the relations it keeps, the places it shows and the examples it finds.

mod common;

use kaname::check::Checked;
use kaname::interp::{Interp, Watch};
use kaname::ranges::{Abs, Outcome, Rel, What};
use kaname::value::{Program, Value};
use ritsu_base::text::Lang;
use std::collections::BTreeSet;
use std::rc::Rc;

/// The modules whose vectors are run: the examples, and the test modules (checked as the file `t.kn`
/// beside the crate, where they read the examples' rules by their paths).
fn modules() -> Vec<(String, Rc<Checked>)> {
    let mut out = Vec::new();
    for f in common::EXAMPLES {
        let o = common::check(f);
        out.push((f.to_string(), o.analysed.clone().unwrap_or_else(|| panic!("{f}: {}", common::shown(&o, Lang::En)))));
    }
    for (name, src) in [
        ("calc", include_str!("modules/calc.kn")),
        ("places", include_str!("modules/places.kn")),
        ("lists", include_str!("modules/lists.kn")),
        ("inner", include_str!("modules/inner.kn")),
        ("rec", include_str!("modules/rec.kn")),
        ("ranges", include_str!("modules/ranges.kn")),
        ("walks", include_str!("modules/walks.kn")),
        ("calls", include_str!("modules/calls.kn")),
        ("names", include_str!("modules/names.kn")),
        ("corners", include_str!("modules/corners.kn")),
        ("bounds", include_str!("modules/bounds.kn")),
        ("lift", include_str!("modules/lift.kn")),
        ("lift.ja", include_str!("modules/lift.ja.kn")),
        ("open", include_str!("modules/open.kn")),
        ("open.ja", include_str!("modules/open.ja.kn")),
        ("sums", include_str!("modules/sums.kn")),
        ("sums.ja", include_str!("modules/sums.ja.kn")),
        ("temps", include_str!("modules/temps.kn")),
        ("temps.ja", include_str!("modules/temps.ja.kn")),
        ("dicts", include_str!("modules/dicts.kn")),
        ("dicts.ja", include_str!("modules/dicts.ja.kn")),
        ("lists2", include_str!("modules/lists2.kn")),
        ("lists2.ja", include_str!("modules/lists2.ja.kn")),
        ("texts", include_str!("modules/texts.kn")),
        ("texts.ja", include_str!("modules/texts.ja.kn")),
        ("generic", include_str!("modules/generic.kn")),
        ("generic.ja", include_str!("modules/generic.ja.kn")),
        ("results", include_str!("modules/results.kn")),
        ("results.ja", include_str!("modules/results.ja.kn")),
        ("gcalls", include_str!("modules/gcalls.kn")),
        ("hof", include_str!("modules/hof.kn")),
        ("hof.ja", include_str!("modules/hof.ja.kn")),
        ("trees", include_str!("modules/trees.kn")),
        ("trees.ja", include_str!("modules/trees.ja.kn")),
    ] {
        let o = common::check_text("t.kn", src);
        out.push((name.to_string(), o.analysed.clone().unwrap_or_else(|| panic!("{name}: {}", common::shown(&o, Lang::En)))));
    }
    out
}

/// The ends of an abstract value marked as its type's (DESIGN 22.2), each with which end it is.
fn marked(a: &Abs) -> Vec<(&'static str, ritsu_units::Rat)> {
    match a {
        Abs::Num(i) | Abs::Date(i) => {
            let mut out = Vec::new();
            if i.open_lo
                && let Some(l) = i.lo
            {
                out.push(("lo", l));
            }
            if i.open_hi
                && let Some(h) = i.hi
            {
                out.push(("hi", h));
            }
            out
        }
        Abs::Opt(_, x) => marked(x),
        _ => Vec::new(),
    }
}

/// Run `f` on a stack as deep as the recursions of the test modules need (`interp::DEPTH` calls).
fn deep<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new().stack_size(256 << 20).spawn(f).unwrap().join().unwrap()
}

/// A value of what a place declares, drawn from a sequence of numbers that is the same every run:
/// a number or a date in its range and on its step, a list of up to six elements, a record and a value
/// with fields field by field, a result `ok` or `err`, `none` one time in four. Four records deep, the
/// base value of the place: a list as few elements as it may have, so that a value of a type that
/// holds itself ends (DESIGN 28.1).
fn sample(p: &Program, slot: &kaname::ty::Slot, rng: &mut u64, depth: usize) -> Value {
    use kaname::ty::Ty;
    use ritsu_units::Rat;
    let mut next = || {
        *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*rng >> 33) as i128
    };
    match &slot.ty {
        Ty::Opt(x) => {
            if next() % 4 == 0 {
                Value::None
            } else {
                sample(p, &kaname::ty::Slot { ty: (**x).clone(), ..slot.clone() }, rng, depth)
            }
        }
        Ty::List(x) => {
            let (lo, hi) = slot.count.as_ref().map(|c| (c.min.map(|v| v.num).unwrap_or(0), c.max.map(|v| v.num).unwrap_or(6))).unwrap_or((0, 6));
            if depth >= 4 {
                return kaname::vectors::base_of(p, &kaname::ty::Slot { count: Some(kaname::ty::Range { min: Some(Rat::int(lo)), max: Some(Rat::int(lo)) }), ..slot.clone() });
            }
            let n = lo + next().rem_euclid((hi.min(lo + 6) - lo + 1).max(1));
            let elem = kaname::ty::Slot { ty: (**x).clone(), range: slot.range.clone(), count: None };
            Value::List((0..n).map(|_| sample(p, &elem, rng, depth + 1)).collect())
        }
        // a dict and a set: up to six keys drawn, each one key once (DESIGN 24.4)
        Ty::Dict(k, x) => {
            let (lo, hi) = slot.count.as_ref().map(|c| (c.min.map(|v| v.num).unwrap_or(0), c.max.map(|v| v.num).unwrap_or(6))).unwrap_or((0, 6));
            let n = lo + next().rem_euclid((hi.min(lo + 6) - lo + 1).max(1));
            let (ks, vs) = (kaname::ty::Slot::of((**k).clone()), kaname::ty::Slot { ty: (**x).clone(), range: slot.range.clone(), count: None });
            let pairs = (0..n).map(|_| (sample(p, &ks, rng, depth + 1), sample(p, &vs, rng, depth + 1))).collect();
            Value::Dict(p.dict_of(k, pairs))
        }
        Ty::Set(x) => {
            let (lo, hi) = slot.count.as_ref().map(|c| (c.min.map(|v| v.num).unwrap_or(0), c.max.map(|v| v.num).unwrap_or(6))).unwrap_or((0, 6));
            let n = lo + next().rem_euclid((hi.min(lo + 6) - lo + 1).max(1));
            let es = kaname::ty::Slot { ty: (**x).clone(), range: slot.range.clone(), count: None };
            let members = (0..n).map(|_| sample(p, &es, rng, depth + 1)).collect();
            Value::Set(p.set_of(x, members))
        }
        Ty::Num(nt) => {
            let step = if nt.is_rate() { nt.step.unwrap_or(Rat::new(1, 100)) } else { Rat::int(1) };
            let r = slot.range.clone().or_else(|| nt.is_rate().then(|| kaname::ty::Range { min: Some(Rat::zero()), max: Some(Rat::int(1)) }));
            let (lo, hi) = r.map(|r| (r.min.unwrap_or(Rat::int(-1000)), r.max.unwrap_or(Rat::int(1000)))).unwrap_or((Rat::int(-1000), Rat::int(1000)));
            let (a, b) = (lo.div(step), hi.div(step));
            let (a, b) = (a.num.div_euclid(a.den), b.num.div_euclid(b.den));
            let k = if b > a { a + next().rem_euclid(b - a + 1) } else { a };
            Value::Num(Rat::int(k).mul(step))
        }
        Ty::Date => {
            let r = slot.range.clone().unwrap_or_default();
            let (lo, hi) = (r.min.map(|v| v.num).unwrap_or(20_000), r.max.map(|v| v.num).unwrap_or(21_000));
            Value::Date((lo + next().rem_euclid((hi - lo + 1).max(1))) as i64)
        }
        Ty::Bool => Value::Bool(next() % 2 == 0),
        Ty::Str => Value::Str(if next() % 2 == 0 { "a".into() } else { "b".into() }),
        Ty::Enum(id) => {
            let vs = p.values(id).0;
            let v = vs[(next() as usize) % vs.len().max(1)].text.clone();
            // a value with fields, field by field (DESIGN 23.6)
            let fs = p.value_fields(id, &v);
            match (fs.is_empty(), depth < 4) {
                (true, _) => Value::Enum(v),
                (false, true) => Value::Variant(v, fs.iter().map(|(f, s)| (f.text.clone(), sample(p, s, rng, depth + 1))).collect()),
                (false, false) => kaname::vectors::base_of(p, slot),
            }
        }
        Ty::Record(id @ kaname::ty::RecId::Variant { value, .. }) if depth < 4 => Value::Variant(value.clone(), p.fields(id).iter().map(|(f, s)| (f.text.clone(), sample(p, s, rng, depth + 1))).collect()),
        Ty::Record(id) if depth < 4 => Value::Record(p.fields(id).iter().map(|(f, s)| (f.text.clone(), sample(p, s, rng, depth + 1))).collect()),
        Ty::Record(_) => kaname::vectors::base_of(p, slot),
        // a result, `ok` or `err` with its value (DESIGN 26.4)
        Ty::Result(a, e) => {
            let (tag, field, inner) = if next() % 2 == 0 { ("ok", "value", a) } else { ("err", "error", e) };
            Value::Variant(tag.into(), vec![(field.into(), sample(p, &kaname::ty::Slot::of((**inner).clone()), rng, depth + 1))])
        }
        // a value of a type parameter, given to a generic function called alone: a number (its
        // analysis takes any value there, DESIGN 26.5)
        Ty::Param(_) => Value::Num(Rat::int(next().rem_euclid(100))),
        _ => Value::None,
    }
}

/// Every vector of every function, and 200 inputs drawn from what its parameters declare, run with
/// every value watched: each value is among what the analysis says of its expression (C2.5's third
/// condition: no place where a value seen is outside), and each failure is of a kind the function's
/// effect says it can fail with (a place that can fail is never missing: DESIGN 4.3, 15.2.3).
#[test]
fn every_value_seen_is_in_what_the_analysis_says() {
    let (exprs, values, vectors, outside, failed, unsaid, (taken, marked)) = deep(|| {
        let suite = common::joined();
        // each module's expressions apart: the test modules are all checked as `t.kn`
        let mut seen_exprs: BTreeSet<(String, std::path::PathBuf, usize)> = BTreeSet::new();
        // the ends marked as the type's (DESIGN 22.2) of each module's own expressions, and those a run
        // takes
        let mut marked_ends: BTreeSet<(String, usize, &'static str)> = BTreeSet::new();
        let mut taken_ends: BTreeSet<(String, usize, &'static str)> = BTreeSet::new();
        let (mut values, mut vectors, mut failed) = (0usize, 0usize, 0usize);
        let mut outside: Vec<String> = Vec::new();
        let mut unsaid: Vec<String> = Vec::new();
        for (name, c) in modules() {
            let p = Program::of(&c);
            for (id, a) in c.analysis.get().unwrap().at.iter().enumerate() {
                for (which, _) in a.as_ref().map(marked).unwrap_or_default() {
                    marked_ends.insert((name.clone(), id, which));
                }
            }
            for f in 0..c.decls.fns.len() {
                // a function that takes functions is run through the functions that call it: no value
                // drawn is a function (DESIGN 27.1)
                if c.decls.fns[f].takes_fn() {
                    continue;
                }
                let (mut vs, _) = kaname::vectors::choose(&p, &c, f);
                let mut rng = 0x5eed_u64 + f as u64;
                for _ in 0..200 {
                    let input = c.decls.fns[f].params.iter().map(|prm| sample(&p, &prm.slot, &mut rng, 0)).collect();
                    vs.push(kaname::vectors::Vector { input, why: None });
                }
                let effect = &c.analysis.get().unwrap().fns[f].effect;
                for v in &vs {
                    let mut it = Interp::new(&p, &suite);
                    if it.check_entry(&c, f, &v.input).is_err() {
                        continue;
                    }
                    vectors += 1;
                    it.watch = Some(Watch::default());
                    if let Err(kaname::interp::Stop::Failed(fl)) = it.call(&c, f, v.input.clone(), 0) {
                        failed += 1;
                        if !effect.fails.iter().any(|k| k.word() == fl.kind.word()) {
                            unsaid.push(format!("{name}, {}: {:?} fails with `{}` at line {}, and the effect is {}", c.decls.fns[f].name.text, v.input, fl.kind.word(), fl.line, effect.written()));
                        }
                    }
                    for (file, id, value) in it.watch.take().unwrap().seen {
                        let m = p.module(&file);
                        let at = m.analysis.get().and_then(|a| a.at.get(id).cloned().flatten());
                        values += 1;
                        seen_exprs.insert((name.clone(), file.clone(), id));
                        let v = match &value {
                            Value::Num(r) => Some(*r),
                            Value::Date(d) => Some(ritsu_units::Rat::int(*d as i128)),
                            _ => None,
                        };
                        if file == c.canon {
                            for (which, end) in at.as_ref().map(marked).unwrap_or_default() {
                                if v == Some(end) {
                                    taken_ends.insert((name.clone(), id, which));
                                }
                            }
                        }
                        match at {
                            Some(abs) if abs.has(&value) => {}
                            other => outside.push(format!("{name}, {}: expression {id} is {value:?}, and the analysis says {other:?}", c.decls.fns[f].name.text)),
                        }
                    }
                }
            }
        }
        (seen_exprs.len(), values, vectors, outside, failed, unsaid, (taken_ends.len(), marked_ends.len()))
    });
    eprintln!("ranges: {taken} of the {marked} ends marked as a type's are taken by a run (a vector at the end of a type)");
    assert!(taken > 0 && marked > 0, "no end marked as a type's is taken: {taken} of {marked}");
    eprintln!("ranges: {values} values of {exprs} expressions seen on {vectors} vectors; {} outside what the analysis says", outside.len());
    eprintln!("ranges: {failed} of the runs fail; {} with a kind the function's effect does not say", unsaid.len());
    // each function the values outside and the failures unsaid are of, with how many and the first
    let mut by: std::collections::BTreeMap<String, (usize, String)> = std::collections::BTreeMap::new();
    for s in outside.iter().chain(&unsaid) {
        let (head, rest) = s.split_once(':').unwrap_or((s, ""));
        let e = by.entry(head.to_string()).or_insert((0, rest.trim().to_string()));
        e.0 += 1;
    }
    for (f, (n, first)) in &by {
        eprintln!("ranges:   {f}: {n}, the first: {first}");
    }
    assert!(outside.is_empty(), "{}", outside.iter().take(20).cloned().collect::<Vec<_>>().join("\n"));
    assert!(unsaid.is_empty(), "{}", unsaid.iter().take(20).cloned().collect::<Vec<_>>().join("\n"));
    assert!(exprs > 300 && values > 50_000, "{exprs} expressions, {values} values");
}

/// What the analysis says of the examples (DESIGN 17): the places kaname decides are shown; the ones
/// the checks across the languages decide wait for them (PLAN C3).
#[test]
fn the_examples_show_what_kaname_decides() {
    for f in common::EXAMPLES {
        let o = common::check(f);
        let c = o.checked.clone().unwrap();
        let a = c.analysis.get().unwrap();
        for (fi, facts) in a.fns.iter().enumerate() {
            let shown = facts.places.iter().filter(|p| p.outcome == Outcome::Shown).count();
            let waiting: Vec<String> = facts.places.iter().filter_map(|p| p.what.across().map(|w| format!("{w} line {}", p.span.line))).collect();
            let examples = facts.places.iter().filter(|p| matches!(p.outcome, Outcome::Example(_))).count();
            eprintln!("{f}, {}: {} places, {shown} shown, {examples} examples, waiting {waiting:?}; effect {}", c.decls.fns[fi].name.text, facts.places.len(), facts.effect.written());
            for p in &facts.places {
                let waits = p.what.across().is_some();
                assert_eq!(matches!(p.outcome, Outcome::Pending), waits, "{f}, {}: {p:?}", c.decls.fns[fi].name.text);
                if !waits {
                    assert_eq!(p.outcome, Outcome::Shown, "{f}, {}: {p:?}", c.decls.fns[fi].name.text);
                }
            }
        }
    }
    let waits = |f: &str| -> Vec<(usize, &'static str)> {
        let c = common::check(f).checked.clone().unwrap();
        c.analysis.get().unwrap().fns.iter().flat_map(|x| x.places.iter().filter_map(|p| p.what.across().map(|w| (p.span.line, w))).collect::<Vec<_>>()).collect()
    };
    assert_eq!(waits("examples/payout/payout.kn"), [(21, "X4"), (22, "X4")]);
    assert_eq!(waits("examples/late_payment/late_payment.kn"), [(20, "X6")]);
    assert_eq!(waits("examples/media_mail/media_mail.kn"), []);
    assert_eq!(waits("examples/discount_split/discount_split.kn"), [(17, "X2"), (17, "X2")]);
}

/// The values DESIGN 17 says of the examples: the days late, narrowed in the branch; the weight of the
/// cart; the running totals and the relations between them.
#[test]
fn the_values_of_the_examples() {
    let iv_of = |c: &Checked, e: &kaname::ast::Expr| -> (Option<i128>, Option<i128>) {
        let i = c.analysis.get().unwrap().at[e.id].as_ref().and_then(Abs::iv).unwrap();
        (i.lo.map(|v| v.num), i.hi.map(|v| v.num))
    };
    // `paid_on - due` is −758 to 1431 days; in the branch of `late >= 1`, 1 to 1431
    let c = common::check("examples/late_payment/late_payment.kn").checked.clone().unwrap();
    let body = &c.module.fns[0].body;
    assert_eq!(iv_of(&c, &body.lets[1].value), (Some(-758), Some(1431)));
    let facts = &c.analysis.get().unwrap().fns[0];
    let days = facts.places.iter().find(|p| matches!(&p.what, What::RuleInput { input, .. } if input == "days_late")).unwrap();
    assert_eq!(days.value.iv().map(|i| (i.lo.map(|v| v.num), i.hi.map(|v| v.num))), Some((Some(1), Some(1431))));
    // the due date is koyomi's: 29 days after the date invoiced
    assert_eq!(iv_of(&c, &body.lets[0].value), (Some(20483), Some(21212)));
    // the weight of the cart is 1 to 96,000 ounces; in the branch of `weight <= 15lb`, 1 to 240
    let c = common::check("examples/media_mail/media_mail.kn").checked.clone().unwrap();
    let a = c.analysis.get().unwrap();
    assert_eq!(a.fns[0].result.iv().map(|i| (i.lo.map(|v| v.num), i.hi.map(|v| v.num))), Some((Some(1), Some(96000))));
    let w = a.fns[1].places.iter().find(|p| matches!(&p.what, What::RuleInput { input, .. } if input == "weight")).unwrap();
    assert_eq!(w.value.iv().map(|i| (i.lo.map(|v| v.num), i.hi.map(|v| v.num))), Some((Some(1), Some(240))));
    // the running totals keep `before <= upto <= total` at the call of the rule (what X2 reads)
    let c = common::check("examples/discount_split/discount_split.kn").checked.clone().unwrap();
    let facts = &c.analysis.get().unwrap().fns[0];
    let kaname::ranges::Kept { line, rule, rels } = &facts.relations[0];
    assert_eq!((*line, rule.as_str()), (17, "discount_share"));
    assert!(rels.contains(&("before".to_string(), Rel::Le, "upto".to_string())), "{rels:?}");
    assert!(rels.contains(&("upto".to_string(), Rel::Le, "total".to_string())), "{rels:?}");
    let before = facts.places.iter().find(|p| matches!(&p.what, What::RuleInput { input, .. } if input == "before")).unwrap();
    assert_eq!(before.value.iv().map(|i| (i.lo.map(|v| v.num), i.hi.map(|v| v.num))), Some((Some(0), Some(90_000_000))));
}

/// The modules of the interpreter's tests fail on purpose; the analysis finds where, with an input of
/// the function that the interpreter then fails at (DESIGN 5.7).
#[test]
fn the_examples_found_fail_where_they_are_said_to() {
    deep(|| {
        let suite = common::joined();
        let mut found = Vec::new();
        for (name, c) in modules() {
            let p = Program::of(&c);
            for (f, facts) in c.analysis.get().unwrap().fns.iter().enumerate() {
                for pl in &facts.places {
                    let Outcome::Example(w) = &pl.outcome else { continue };
                    let mut it = Interp::new(&p, &suite);
                    let fails = it.call(&c, f, w.input.clone(), 0);
                    assert!(fails.is_err(), "{name}, {}: the example of line {} answers", c.decls.fns[f].name.text, pl.span.line);
                    found.push(format!("{name}:{}:{}", pl.span.line, kaname::ranges::code_of(pl).unwrap()));
                }
            }
        }
        assert_eq!(found, ["calc:27:E303", "calc:33:E302", "calc:42:E303", "calc:45:E302", "calc:45:E303", "places:12:E301", "places:15:E301", "places:18:E301", "places:21:E301", "inner:9:E301", "ranges:85:E301", "bounds:12:E302", "bounds:17:E302", "bounds:32:E301", "lift:8:E301", "lift.ja:8:E301", "open:53:E302", "open:67:E301", "open.ja:52:E302", "open.ja:64:E301"]);
    });
}

/// What the arms of a `match` narrow (DESIGN 23.7): a value of an enum with fields to the values an
/// arm names, with its fields, the name the arm gives it; `else` to the values left; the literals
/// of an arm to what they are, joined; and a record made from another keeps what its fields
/// declare, the fields taken too.
#[test]
fn what_the_arms_of_a_match_narrow() {
    let t = ritsu_testkit::TempDir::new("kaname-ranges-arms");
    let codes = |src: &str| -> Vec<&'static str> {
        let f = t.write("m.kn", src);
        common::codes(&common::check(f.to_str().unwrap()))
    };
    let order = "module m v1\n\nenum Order\n  draft\n    n : number  range >=0 <=5\n  placed\n    n : number  range >=10 <=20\n  cancelled\n\n";
    // the arm's value: its fields are its own, not the other values'
    assert_eq!(codes(&format!("{order}fn f(o: Order) -> number range >=10 <=20\n  match o\n    placed p => p.n\n    else => 10\n")), Vec::<&str>::new());
    // `else` takes what is left: here `draft` and `cancelled`, so `placed` reaches no `else`
    assert_eq!(codes(&format!("{order}fn f(o: Order) -> number range >=0 <=5\n  match o\n    placed => 5\n    else =>\n      match o\n        draft d => d.n\n        else => 0\n")), Vec::<&str>::new());
    // two literals in one arm: the value is either, not the last
    assert_eq!(codes("module m v1\n\nfn g(n: number range >=0 <=9) -> number range >=5 <=5\n  match n\n    0, 1 => n + 5\n    else => 5\n"), ["E301"]);
    assert_eq!(codes("module m v1\n\nfn g(n: number range >=0 <=9) -> number range >=5 <=6\n  match n\n    0, 1 => n + 5\n    else => 5\n"), Vec::<&str>::new());
    // a field taken by `..x` is held to the range of the record it goes to
    let wide = "module m v1\n\nrecord Wide\n  n : number  range >=0 <=100\n\nrecord Narrow\n  n : number  range >=0 <=10\n  m : number  range >=0 <=10\n\n";
    assert_eq!(codes(&format!("{wide}fn f(x: Narrow, k: number range >=0 <=10) -> Narrow\n  Narrow {{ ..x, m: k }}\n")), Vec::<&str>::new());
    assert_eq!(codes(&format!("{wide}fn f(x: Narrow, k: number range >=0 <=100) -> Narrow\n  Narrow {{ ..x, m: k }}\n")), ["E301"]);
}

/// What a branch narrows and what is known after a call answers (DESIGN 5.3): the days given a rule
/// once it answered, a list's count once `count(xs) = 0` is false, the difference of two names whose
/// order a condition keeps.
#[test]
fn what_a_branch_and_a_call_tell() {
    let t = ritsu_testkit::TempDir::new("kaname-ranges");
    ritsu_testkit::tmp::copy_dir(std::path::Path::new("examples/late_payment/rules"), &t.path().join("rules"));
    let codes = |src: &str| -> Vec<&'static str> {
        let f = t.write("m.kn", src);
        common::codes(&common::check(f.to_str().unwrap()))
    };
    let head = "module m v1\n\nuse rule charges from \"rules/late_payment_charges.rule\"\n\nrecord Claim\n  days : number  range >=1 <=1500\n\n";
    // once the rule answers, the days it took are in its range: the record that follows holds them
    assert_eq!(codes(&format!("{head}fn f(days: number range >=1 <=1430) -> Claim\n  let c = charges(debt: 100GBPc, days_late: days, reference_rate: 1%)\n  Claim {{ days: days }}\n")), Vec::<&str>::new());
    assert_eq!(codes(&format!("{head}fn f(days: number range >=0 <=1430) -> Claim\n  Claim {{ days: days }}\n")), ["E301"]);
    // an empty list is left out by its count
    assert_eq!(codes("module m v1\n\nfn f(xs: list[number] count >=0 <=9 range >=0 <=9) -> number?\n  if count(xs) = 0 then none else down(sum(xs, x => x) / count(xs), 1)\n"), Vec::<&str>::new());
    // `m - n` where `n < m`
    assert_eq!(codes("module m v1\n\nfn f(n: number range >=0 <=9, m: number range >=0 <=9) -> number\n  if n < m then isqrt(m - n) else 0\n"), Vec::<&str>::new());
    assert_eq!(codes("module m v1\n\nfn f(n: number range >=0 <=9, m: number range >=0 <=9) -> number\n  isqrt(m - n)\n"), ["E301"]);
    // a divisor that is always 0: the evaluation stops there, and nothing after it is reached
    assert_eq!(codes("module m v1\n\nfn g(x: number range >=0 <=9, y: number range >=0 <=9) -> number\n  x + y\n\nfn f(a: number range >=1 <=9, z: number range >=0 <=0) -> number\n  g(x: down(a / z, 1), y: a * 4_000_000_000_000_000_000)\n"), ["E303"]);
}

/// A rule's input of an ordered enum held to a range over its values (ritsu's DESIGN 3.2, DESIGN
/// 6.1: `campaign : tier  range >=basic <=gold`), in English and in Japanese: a value given as it is
/// can be one the range does not let in (E301, with the value that breaks it, where the interpreter
/// fails `rule` as the port's evaluator refuses it); narrowed with a `match`, or with an `if` that
/// leaves a value out, it is shown and the function is total; the tier another call answers is
/// undecided (W301), and the generated code checks it when it runs.
#[test]
fn an_enum_input_of_a_rule_is_held_to_its_range() {
    deep(|| {
        let suite = common::joined();
        for (path, src, outside, inside) in [
            ("lift.kn", include_str!("modules/lift.kn"), "platinum", "gold"),
            ("lift.ja.kn", include_str!("modules/lift.ja.kn"), "プラチナ", "ゴールド"),
        ] {
            let o = common::check_text(path, src);
            let said: Vec<(Option<usize>, &str)> = o.diags.iter().map(|d| (d.line, d.code)).collect();
            assert_eq!(said, [(Some(8), "E301"), (Some(28), "W301")], "{path}: {}", common::shown(&o, Lang::En));
            let c = o.analysed.clone().unwrap();
            let a = c.analysis.get().unwrap();
            let enums = |f: usize| -> Vec<&Outcome> { a.fns[f].places.iter().filter(|p| matches!(p.what, What::RuleEnum { .. })).map(|p| &p.outcome).collect() };
            match enums(0)[..] {
                [Outcome::Example(w)] => assert_eq!(w.input, [Value::Enum(outside.into())], "{path}"),
                ref other => panic!("{path}: {other:?}"),
            }
            assert_eq!(enums(1), [&Outcome::Shown], "{path}");
            assert_eq!(enums(2), [&Outcome::Shown], "{path}");
            // the first call is given gold, the second the tier the first answers
            assert_eq!(enums(3), [&Outcome::Shown, &Outcome::Undecided], "{path}");
            let effects: Vec<String> = a.fns.iter().map(|x| x.effect.written()).collect();
            assert_eq!(effects, ["fails rule", "total", "total", "fails rule"], "{path}");
            // what the port's evaluator refuses, the interpreter fails `rule` at: the line of the call
            let p = Program::of(&c);
            let mut it = Interp::new(&p, &suite);
            match it.call(&c, 0, vec![Value::Enum(outside.into())], 0) {
                Err(kaname::interp::Stop::Failed(fl)) => assert_eq!((fl.kind.word(), fl.line), ("rule", 8), "{path}"),
                other => panic!("{path}: {other:?}"),
            }
            let mut it = Interp::new(&p, &suite);
            assert_eq!(it.call(&c, 0, vec![Value::Enum(inside.into())], 0).ok(), Some(Value::Bool(true)), "{path}");
            let mut it = Interp::new(&p, &suite);
            assert!(it.call(&c, 1, vec![Value::Enum(outside.into())], 0).is_ok(), "{path}: the narrowed function answers every tier");
        }
    });
}
