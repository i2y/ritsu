//! What an output comes to when some inputs take only part of their range (ritsu's port
//! `Rules::outputs_over`; sekisho's DESIGN 3.2). sekisho cuts an input a policy compares with a
//! constant into intervals, holds an enum or a bool a policy reads (or a gate writes as a constant)
//! to each of its values, and counts, for each, the values a rule's output can come to there; the
//! rule's own declared range is not the question any more.
//!
//! rulec answers by reading the rule again with each input held to the range given, inside what
//! the rule declares for it ([`with`], read by the type check and by the axes of the region, the
//! way `checked_over` holds a date input to koyomi's days; an enum or a bool input to the values
//! given, the coordinates of its other values masked out of every row as those an upstream table
//! never produces are), and then from two sides:
//!
//! - From above: the rows of the table that decides the output which some input may reach
//!   ([`crate::region::live_rows`]: the region of the row, less what the rows that take
//!   precedence cover, through the sieve the completeness proof uses — the derived values'
//!   reach, the `constraint`s, the tables above, the linear model). A row the sieve rules out is
//!   never reached, so the values the other rows write hold every value the output comes to. A
//!   row that passes an enum or a bool input on (`| paid | pay | state |`) writes the values of
//!   that input its own cell lets in (and the hold, when there is one).
//! - From below: an input for each of those values, from the rule's vectors over the held
//!   ranges, run through the evaluator. A value with an input is a value the output comes to. A
//!   value a row passes on that no vector gives (the vectors take one value of each class the
//!   cells cannot tell apart, §9.1) is tried with a vector whose passed input is set to it.
//!
//! When the two meet, the answer is exact. When a live row's value has no input, the sieve could
//! not rule the row out (two derived values over the same inputs, a computed `define` in a
//! column), and no input reaches it: the answer is undecided, with the value and its rows.

use crate::ast::{Lit, OutCell, RuleFile};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use ritsu_base::text::Text;
use ritsu_ports::{Found, Value, Values};
use std::cell::RefCell;

/// An interval of true values; an end that is None is open.
pub type Ival = (Option<Rat>, Option<Rat>);

/// How many names in a row a define may pass an output on through.
const MAX_PASSES: usize = 16;

/// What `outputs_over` holds the inputs to: a number or a date to an interval, an enum or a bool
/// to some of its values (each by its name in the rule, `true` and `false` for a bool).
#[derive(Clone, Debug, Default)]
pub struct Held {
    pub ranges: Vec<(String, Ival)>,
    pub values: Vec<(String, Vec<String>)>,
}

thread_local! {
    /// The inputs `outputs_over` holds.
    static HELD: RefCell<Option<Held>> = const { RefCell::new(None) };
}

/// Puts back what was held however the closure ends.
struct Restore(Option<Held>);

impl Drop for Restore {
    fn drop(&mut self) {
        let old = self.0.take();
        HELD.with(|c| *c.borrow_mut() = old);
    }
}

/// Runs `f` with each input of `held` taking only the values given (inside what the rule declares
/// for it), on this thread.
pub fn with<R>(held: Held, f: impl FnOnce() -> R) -> R {
    let old = HELD.with(|c| std::mem::replace(&mut *c.borrow_mut(), Some(held)));
    let _restore = Restore(old);
    f()
}

/// Runs `f` with nothing held: the check of a rule this one applies (§15.69), which is the other
/// rule as it is, whatever this one's inputs are held to.
pub fn without<R>(f: impl FnOnce() -> R) -> R {
    let old = HELD.with(|c| c.borrow_mut().take());
    let _restore = Restore(old);
    f()
}

/// The interval `name` is held to, if it is.
pub fn held(name: &str) -> Option<Ival> {
    HELD.with(|c| c.borrow().as_ref().and_then(|h| h.ranges.iter().find(|(n, _)| n == name).map(|(_, i)| *i)))
}

/// The values the enum or bool input `name` is held to, if it is: each value of an enum by its
/// name in the rule, `true` and `false` for a bool.
pub fn held_values(name: &str) -> Option<Vec<String>> {
    HELD.with(|c| c.borrow().as_ref().and_then(|h| h.values.iter().find(|(n, _)| n == name).map(|(_, vs)| vs.clone())))
}

/// The values two intervals share: the higher of the low ends, the lower of the high ends.
pub fn meet(a: Ival, b: Ival) -> Ival {
    use std::cmp::Ordering::*;
    let lo = match (a.0, b.0) {
        (Some(x), Some(y)) => Some(if x.cmp_to(y) == Less { y } else { x }),
        (x, y) => x.or(y),
    };
    let hi = match (a.1, b.1) {
        (Some(x), Some(y)) => Some(if x.cmp_to(y) == Greater { y } else { x }),
        (x, y) => x.or(y),
    };
    (lo, hi)
}

/// Whether `v` lies in `iv`, both ends in.
fn inside(iv: Ival, v: Rat) -> bool {
    iv.0.is_none_or(|l| l.cmp_to(v) != std::cmp::Ordering::Greater) && iv.1.is_none_or(|h| v.cmp_to(h) != std::cmp::Ordering::Greater)
}

/// The boundaries and the ends of a numeric axis, narrowed to the interval `held`: the new ends
/// are boundaries too, and a boundary outside them is no coordinate, as with a declared range.
pub fn narrow(bounds: &mut Vec<Rat>, ends: Ival, held: Ival) -> Ival {
    let (lo, hi) = meet(ends, held);
    bounds.extend(lo);
    bounds.extend(hi);
    bounds.retain(|x| inside((lo, hi), *x));
    bounds.sort_by(|x, y| x.cmp_to(*y));
    bounds.dedup_by(|x, y| x.cmp_to(*y) == std::cmp::Ordering::Equal);
    (lo, hi)
}

/// The value a word written in an output cell stands for, when it is one of the output's values
/// (an enum's value, `true`, `false`, `none`) rather than the name of something the rule computes.
fn word_value(w: &str, all: &[Value]) -> Option<Value> {
    let v = match w {
        crate::kw::NONE => Value::None,
        crate::kw::TRUE if all.contains(&Value::Bool(true)) => Value::Bool(true),
        crate::kw::FALSE if all.contains(&Value::Bool(false)) => Value::Bool(false),
        _ => Value::Enum(w.to_string()),
    };
    all.contains(&v).then_some(v)
}

/// The values an output cell that names an enum or a bool input passes on (`| paid | pay | state |`
/// writes the order's own state): the input's values (those `outputs_over` holds it to, when it
/// does) that the row's own cell for that input lets in, as values of the output. None for a cell
/// that names anything else, whose value the rule computes.
fn passed_on(f: &RuleFile, c: &Checked, t: &crate::ast::Table, row: &crate::ast::Row, oi: usize, all: &[Value]) -> Option<Vec<Value>> {
    let Some(OutCell::Name(w)) = row.outs.get(oi) else { return None };
    if !f.inputs.iter().any(|i| i.name.text == *w) {
        return None;
    }
    let ty = c.ty_of(w)?;
    let inner = match &ty {
        Ty::Opt(t) => (**t).clone(),
        t => t.clone(),
    };
    let mut values: Vec<String> = match &inner {
        Ty::Enum(e) => c.enums.get(e)?.clone(),
        Ty::Bool => vec![crate::kw::TRUE.to_string(), crate::kw::FALSE.to_string()],
        _ => return None,
    };
    if matches!(ty, Ty::Opt(_)) {
        values.insert(0, crate::kw::NONE.to_string());
    }
    if let Some(h) = held_values(w) {
        values.retain(|v| h.contains(v));
    }
    let cell = t.inputs.iter().position(|(n, _)| n == w).and_then(|k| row.cells.get(k));
    let lets_in = |v: &str| {
        let Some(cell) = cell else { return true };
        match v {
            crate::kw::NONE => crate::eval::cell_matches(c, cell, &crate::eval::Val::Enum(v.to_string()), &ty),
            _ if inner == Ty::Bool => crate::eval::cell_matches(c, cell, &crate::eval::Val::Bool(v == crate::kw::TRUE), &inner),
            _ => crate::eval::cell_matches(c, cell, &crate::eval::Val::Enum(v.to_string()), &inner),
        }
    };
    Some(values.iter().filter(|v| lets_in(v)).filter_map(|v| word_value(v, all)).collect())
}

/// The values each table's enum and bool outputs can come to, narrowed to what the rows some
/// input may reach write (`Checked::out_values`, which the tables below read their columns
/// against), table by table from the top until nothing narrows. A table that writes a name it
/// computes (rather than a value) keeps what it had. Values only ever leave, and only the values
/// of rows no input reaches, so a table below still sees every value it can be given.
fn narrow_upstream(f: &RuleFile, c: &mut Checked) {
    for _ in 0..=c.sets.len() {
        let mut narrowed = false;
        for si in 0..c.sets.len() {
            let mut now: Vec<(String, Vec<String>)> = Vec::new();
            {
                let set = &c.sets[si];
                let live = crate::region::live_rows(set, c, f, crate::region::DEFAULT_BUDGET);
                for (oi, oc) in set.table.outputs.iter().enumerate() {
                    let name = &oc.name.text;
                    let Some(before) = c.out_values.get(name) else { continue };
                    let values: Vec<String> = match c.ty_of(name) {
                        Some(Ty::Enum(e)) => c.enums.get(&e).cloned().unwrap_or_default(),
                        Some(Ty::Bool) => vec![crate::kw::TRUE.to_string(), crate::kw::FALSE.to_string()],
                        _ => continue,
                    };
                    let mut written: Vec<&String> = Vec::new();
                    let mut computed = false;
                    for (ri, row) in set.table.rows.iter().enumerate() {
                        if live.as_ref().is_some_and(|l| !l[ri]) {
                            continue;
                        }
                        match row.outs.get(oi) {
                            Some(OutCell::Name(w)) if values.contains(w) => written.push(w),
                            _ => computed = true,
                        }
                    }
                    let kept: Vec<String> = before.iter().filter(|v| written.contains(v)).cloned().collect();
                    if !computed && kept.len() < before.len() {
                        now.push((name.clone(), kept));
                    }
                }
            }
            narrowed |= !now.is_empty();
            c.out_values.extend(now);
        }
        if !narrowed {
            break;
        }
    }
}

/// A value as a message shows it.
fn shown(v: &Value) -> String {
    match v {
        Value::Enum(s) | Value::Str(s) | Value::Date(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::None => crate::kw::NONE.to_string(),
        Value::List(_) => "[…]".to_string(),
    }
}

/// What the output `output` (an enum or a bool) comes to when each input of `ranges` takes only
/// the values between its ends (the integers on the wire: a date as its day number; an enum by the
/// places of its values in the order the rule declares them, from 0; a bool as 0 for false and 1
/// for true), and every other input its whole range: each value with an input that comes to it, in
/// the order the values are declared, an enum's value by its public name (`上限まで(within_limit)`
/// is `within_limit`, what a gate writes into Cedar) and the input as `eval` takes it; or
/// undecided, with why. `f` and `c` are the rule as checked; `src` and
/// `path` are read again with the inputs held.
pub fn outputs_over(f: &RuleFile, c: &Checked, src: &str, path: &str, output: &str, ranges: &[(String, Option<i128>, Option<i128>)]) -> Found<Vec<(Value, Values)>> {
    if !f.outputs.iter().any(|o| o.name.text == output) {
        return Found::Undecided(ritsu_base::tr!("`{output}` は、この規則の出力ではありません", "`{output}` is not an output of this rule"));
    }
    // every value the output can take
    let ty = c.ty_of(output).unwrap_or(Ty::Unknown);
    let (inner, optional) = match &ty {
        Ty::Opt(t) => ((**t).clone(), true),
        t => (t.clone(), false),
    };
    let mut all: Vec<Value> = Vec::new();
    if optional {
        all.push(Value::None);
    }
    match &inner {
        Ty::Enum(e) => all.extend(c.enums.get(e).cloned().unwrap_or_default().into_iter().map(Value::Enum)),
        Ty::Bool => all.extend([Value::Bool(true), Value::Bool(false)]),
        _ => {
            return Found::Undecided(ritsu_base::tr!(
                "`{output}` は列挙でも真偽でもない出力です。とる値を数えて答えるのは、列挙と真偽の出力だけです",
                "`{output}` is neither an enum nor a bool; the values are counted for enum and bool outputs only"
            ));
        }
    }
    if let Some(el) = &f.elements {
        let list = &el.name.text;
        return Found::Undecided(ritsu_base::tr!(
            "この規則は並び `{list}` の要素を一つずつ見るので、入力を範囲に限って数えられません",
            "this rule walks the list `{list}`, so its outputs are not counted over ranges of its inputs"
        ));
    }
    // the ranges, as the rule's true values, inside what it declares
    let mut held = Held::default();
    for (name, lo, hi) in ranges {
        if !f.inputs.iter().any(|i| i.name.text == *name) {
            return Found::Undecided(ritsu_base::tr!("`{name}` は、この規則の入力ではありません", "`{name}` is not an input of this rule"));
        }
        let t = match c.ty_of(name) {
            Some(Ty::Opt(t)) => *t,
            Some(t) => t,
            None => Ty::Unknown,
        };
        // an enum by the places of its values, a bool as 0 and 1: the values between the ends
        let words: Option<Vec<String>> = match &t {
            Ty::Enum(e) => Some(c.enums.get(e).cloned().unwrap_or_default()),
            Ty::Bool => Some(vec![crate::kw::FALSE.to_string(), crate::kw::TRUE.to_string()]),
            _ => None,
        };
        if let Some(words) = words {
            let (from, to) = (lo.unwrap_or(0).max(0), hi.unwrap_or(i128::MAX).min(words.len() as i128 - 1));
            let these: Vec<String> = if from > to { Vec::new() } else { words[from as usize..=to as usize].to_vec() };
            // an input given more than once takes the values of each (a gate's enum with fewer
            // values than the rule's, whose places need not run on), in the rule's order
            match held.values.iter_mut().find(|(n, _)| n == name) {
                Some((_, was)) => {
                    let all: Vec<String> = words.iter().filter(|w| was.contains(w) || these.contains(w)).cloned().collect();
                    *was = all;
                }
                None => held.values.push((name.clone(), these)),
            }
            continue;
        }
        let date = t == Ty::Date;
        if !date && !t.is_numeric() {
            return Found::Undecided(ritsu_base::tr!(
                "`{name}` は数でも日付でも列挙でも真偽でもない入力なので、範囲に限れません",
                "`{name}` is neither a number, a date, an enum nor a bool, so it cannot be held to a range"
            ));
        }
        let sc = c.wire_scale(name);
        let at = |v: Option<i128>| v.map(|v| if date { Rat::int(v) } else { crate::types::from_wire(v, sc) });
        let declared = c.ranges.get(name).copied().unwrap_or((None, None));
        let iv = meet(declared, (at(*lo), at(*hi)));
        if let (Some(l), Some(h)) = iv
            && l.cmp_to(h) == std::cmp::Ordering::Greater
        {
            // no input lies inside the ranges, so the output comes to nothing
            return Found::Value(Vec::new());
        }
        held.ranges.push((name.clone(), iv));
    }
    if held.values.iter().any(|(_, vs)| vs.is_empty()) {
        // an enum or a bool held to no value: no input lies inside, so the output comes to nothing
        return Found::Value(Vec::new());
    }
    with(held.clone(), || {
        let Ok((f2, mut c2)) = crate::prepare(src, path) else {
            return Found::Undecided(ritsu_base::tr!("入力を範囲に限ると、規則の検査が通りません", "the rule does not pass its check with the inputs held to the ranges"));
        };
        narrow_upstream(&f2, &mut c2);
        // From above: the values the rows some input may reach write, each with those rows (none
        // where no table decides the output). The output is a table's, or the one `result` names.
        let mut candidates: Vec<(Value, Vec<Text>)> = Vec::new();
        let mut add = |v: Value, row: Option<Text>| match candidates.iter_mut().find(|(x, _)| *x == v) {
            Some((_, rows)) => rows.extend(row),
            None => candidates.push((v, row.into_iter().collect())),
        };
        let mut decided = match (&f2.result, f2.outputs.first().is_some_and(|o| o.name.text == output)) {
            (Some(r), true) => match &r.expr {
                crate::ast::Expr::Name(n, _) => Some(n.clone()),
                _ => None,
            },
            _ => Some(output.to_string()),
        };
        // a define that is one name passes that name on (an output a rule applied writes, §15.69)
        for _ in 0..MAX_PASSES {
            let Some(d) = decided.clone() else { break };
            if c2.sets.iter().any(|s| s.table.outputs.iter().any(|o| o.name.text == d)) {
                break;
            }
            decided = f2.items.iter().find_map(|it| match it {
                crate::ast::Item::Define(x) if x.name.text == d => match &x.expr {
                    crate::ast::Expr::Name(m, _) => Some(m.clone()),
                    _ => None,
                },
                _ => None,
            });
        }
        let set = decided.as_ref().and_then(|d| c2.sets.iter().find(|s| s.table.outputs.iter().any(|o| o.name.text == *d)));
        // the inputs a row some input may reach passes on as the output
        let mut passes: Vec<String> = Vec::new();
        match (set, decided) {
            (Some(set), Some(decided)) => {
                let oi = set.table.outputs.iter().position(|o| o.name.text == decided).unwrap_or(0);
                let live = crate::region::live_rows(set, &c2, &f2, crate::region::DEFAULT_BUDGET);
                for (ri, row) in set.table.rows.iter().enumerate() {
                    if live.as_ref().is_some_and(|l| !l[ri]) {
                        continue;
                    }
                    let table = row.origin.clone().or_else(|| set.table.name.as_ref().map(|n| n.text.clone())).unwrap_or_default();
                    let at = ritsu_base::tr!("{table} の行 {}", "row {} of {table}", row.index);
                    let word = match row.outs.get(oi) {
                        Some(OutCell::Name(w)) | Some(OutCell::Lit(Lit::Word(w))) => word_value(w, &all),
                        _ => None,
                    };
                    match (word, passed_on(&f2, &c2, &set.table, row, oi, &all)) {
                        (Some(v), _) => add(v, Some(at)),
                        // an enum or a bool input the row passes on: the values its cell lets in
                        (None, Some(vs)) => {
                            if let Some(OutCell::Name(w)) = row.outs.get(oi)
                                && !passes.contains(w)
                            {
                                passes.push(w.clone());
                            }
                            vs.into_iter().for_each(|v| add(v, Some(at.clone())));
                        }
                        // a name of something the rule computes: any value of the output
                        (None, None) => all.iter().for_each(|v| add(v.clone(), Some(at.clone()))),
                    }
                }
            }
            _ => all.iter().for_each(|v| add(v.clone(), None)),
        }
        // From below: an input for each value, from the vectors over the held ranges.
        let mut found: Vec<(Value, Values)> = Vec::new();
        // the inputs in the order the rule declares them
        let as_input = |input: &std::collections::BTreeMap<String, crate::eval::Val>| -> Values {
            f2.inputs.iter().filter_map(|i| input.get(&i.name.text).map(|x| (i.name.text.clone(), crate::ports::from_val(&c2, &i.name.text, x)))).collect()
        };
        let vectors: Vec<crate::vectors::Vector> = crate::vectors::generate(&f2, &c2)
            .into_iter()
            .filter(|v| {
                held.ranges.iter().all(|(n, iv)| match v.input.get(n) {
                    Some(crate::eval::Val::Num(x)) => inside(*iv, *x),
                    Some(crate::eval::Val::Date(y, m, d)) => inside(*iv, crate::types::date_ord(*y, *m, *d)),
                    _ => true,
                }) && held.values.iter().all(|(n, vs)| match v.input.get(n) {
                    Some(crate::eval::Val::Enum(w)) => vs.contains(w),
                    Some(crate::eval::Val::Bool(b)) => vs.contains(&b.to_string()),
                    _ => false,
                })
            })
            .collect();
        for v in &vectors {
            let Some((_, Some(x))) = v.outputs.iter().find(|(n, _)| n == output) else { continue };
            let value = crate::ports::from_val(&c2, output, x);
            if !found.iter().any(|(y, _)| *y == value) {
                found.push((value, as_input(&v.input)));
            }
        }
        // A value a row passes on from an input: the vectors take one value of each class of the
        // input's values the cells cannot tell apart (§9.1), so a value of a group (`unconfirmed`
        // is two states) may be in none. A vector with that input set to the value, run through
        // the evaluator, is an input that comes to it, if it does.
        for (cand, _) in &candidates {
            if found.iter().any(|(v, _)| v == cand) {
                continue;
            }
            let (x, word) = match cand {
                Value::Enum(w) => (crate::eval::Val::Enum(w.clone()), w.clone()),
                Value::Bool(b) => (crate::eval::Val::Bool(*b), b.to_string()),
                _ => continue,
            };
            // an input held to values takes only those
            let reached = passes.iter().filter(|w| held_values(w).is_none_or(|h| h.contains(&word))).find_map(|w| {
                vectors.iter().find_map(|v| {
                    let mut input = v.input.clone();
                    input.insert(w.clone(), x.clone());
                    if !crate::vectors::allowed(&f2, &input) || !crate::vectors::days_ok(&c2, &input) {
                        return None;
                    }
                    let (outs, _, _) = crate::eval::run_all(&f2, &c2, input.clone().into_iter().collect());
                    let comes = outs.iter().find(|(n, _)| n == output).and_then(|(_, y)| y.as_ref()).map(|y| crate::ports::from_val(&c2, output, y));
                    (comes.as_ref() == Some(cand)).then(|| as_input(&input))
                })
            });
            if let Some(input) = reached {
                found.push((cand.clone(), input));
            }
        }
        if let Some((v, _)) = found.iter().find(|(v, _)| !candidates.iter().any(|(c, _)| c == v)) {
            let v = shown(v);
            return Found::Undecided(ritsu_base::tr!(
                "rulec の解析は `{output}` が {v} にならないと言い、評価器は {v} になる入力を見つけました。食い違うので決めません",
                "rulec's analysis says `{output}` never comes to {v}, and its evaluator finds an input that does; the two disagree, so nothing is decided"
            ));
        }
        let missing: Vec<&(Value, Vec<Text>)> = candidates.iter().filter(|(c, _)| !found.iter().any(|(v, _)| v == c)).collect();
        if !missing.is_empty() {
            let values: Vec<String> = missing.iter().map(|(v, _)| shown(v)).collect();
            let (values_ja, values_en) = (values.join("、"), values.join(", "));
            let rows: Vec<&Text> = missing.iter().flat_map(|(_, rs)| rs).collect();
            return Found::Undecided(if rows.is_empty() {
                ritsu_base::tr!(
                    "`{output}` が {values_ja} になる入力があるかを決められません。範囲の中に、そうなる入力が見つかりません",
                    "whether `{output}` comes to {values_en} cannot be decided: no input inside the ranges is found that does"
                )
            } else {
                let rows_ja = rows.iter().map(|r| r.ja.as_str()).collect::<Vec<_>>().join("、");
                let rows_en = rows.iter().map(|r| r.en.as_str()).collect::<Vec<_>>().join(", ");
                ritsu_base::tr!(
                    "`{output}` が {values_ja} になる入力があるかを決められません。{rows_ja} に当たる入力が無いとは示せず、範囲の中に、そこに当たる入力も見つかりません",
                    "whether `{output}` comes to {values_en} cannot be decided: it cannot be shown that no input matches {rows_en}, and none inside the ranges is found that does"
                )
            });
        }
        found.sort_by_key(|(v, _)| all.iter().position(|a| a == v));
        // an enum's value by its public name, what a gate writes into Cedar
        if let Ty::Enum(e) = &inner {
            for (v, _) in &mut found {
                if let Value::Enum(name) = v {
                    *name = crate::codegen::public_value(&f2, e, name);
                }
            }
        }
        Found::Value(found)
    })
}
