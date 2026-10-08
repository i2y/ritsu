//! The door every generated language keeps, as the reference evaluator keeps it (§15.203,
//! §15.204): what it refuses, and the sentence it says, in the language the code is generated in.
//!
//! The generators write these sentences into the code of every language, and the reference
//! evaluator says them of the inputs it refuses, so `rulec test` can hold every language to the
//! sentence and not only to refusing. One function per sentence is what keeps the twelve, the
//! servers and the reference saying the same bytes.

use crate::ast::RuleFile;
use crate::eval::Val;
use crate::json::Json;
use crate::types::{Checked, Ty};
use std::collections::BTreeMap;

/// An input that is not there: a key the object does not have, or `null` for an input that is
/// not optional.
pub fn missing(name: &str) -> String {
    tr!("{name} がありません", "{name} is missing")
}

/// A number that is not a whole count of its unit or its step, or a value that is not a number.
pub fn not_integer(name: &str) -> String {
    tr!("{name} が整数ではありません", "{name} is not an integer")
}

/// A date that is not a `YYYY-MM-DD` string of a day the calendar has.
pub fn not_date(name: &str) -> String {
    tr!("{name} が日付ではありません", "{name} is not a date")
}

/// A value the enum does not have, by the name the rule declares the enum under.
pub fn not_in_enum(name: &str, en: &str) -> String {
    tr!("{name} が列挙 {en} の値ではありません", "{name} is not a value of enum {en}")
}

/// A truth value that is not JSON's `true` or `false`.
pub fn not_bool(name: &str) -> String {
    tr!("{name} が真偽ではありません", "{name} is not a boolean")
}

/// A string input given anything else.
pub fn not_string(name: &str) -> String {
    tr!("{name} が文字列ではありません", "{name} is not a string")
}

/// The sequence a rule walks, given anything but an array.
pub fn not_sequence(name: &str) -> String {
    tr!("{name} が並びではありません", "{name} is not a sequence")
}

/// An element of the sequence that is not an object.
pub fn not_object(name: &str) -> String {
    tr!("{name} の要素がオブジェクトではありません", "an element of {name} is not an object")
}

/// A number or a date outside its declared range.
pub fn out_of_range(name: &str) -> String {
    tr!("{name} が範囲の外です", "{name} is out of range")
}

/// A sequence longer than a count over it can be.
pub fn too_many(name: &str, cap: i128) -> String {
    tr!("{name} の要素が多すぎます（上限 {cap}）", "{name} has too many elements (at most {cap})")
}

/// A combination a `constraint` rules out, said the way the rule writes it.
pub fn constraint(said: &str) -> String {
    tr!("制約が成り立ちません: {said}", "the constraint does not hold: {said}")
}

/// A day a koyomi date does not come to. The file is written without the characters a string
/// literal of some language would need escaped.
pub fn not_a_day(name: &str, file: &str, date: &str) -> String {
    let file = file.replace(['"', '\\', '`', '\'', '$'], "");
    tr!("{name} は {file} の {date} がとる日ではありません", "{name} is not a day {date} of {file} comes to")
}

/// Two elements both taking under `take_unique`: the rule has no answer.
pub fn fold_contradiction(verdict: &str) -> String {
    tr!("畳み込み {verdict}: take_unique に二件当たりました", "fold {verdict}: two elements matched a take_unique")
}

/// What a door refuses an input for: `input` for the caller's side of the contract, raised as
/// `RuleInputError`, and `contradiction` for the rule's own, raised as `RuleContradictionError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub kind: &'static str,
    pub error: String,
}

fn input(error: String) -> Refusal {
    Refusal { kind: "input", error }
}

/// The input as one value of its declared type, or the sentence its door refuses it with:
/// missing, of another kind, outside its range. The absent value of an optional input is
/// `none`, as the reference evaluator reads it.
fn one(c: &Checked, name: &str, ty: &Ty, j: Option<&Json>) -> Result<Val, String> {
    let Some(j) = j else { return Err(missing(name)) };
    if *j == Json::Null {
        return if matches!(ty, Ty::Opt(_)) { Ok(Val::Enum(crate::kw::NONE.into())) } else { Err(missing(name)) };
    }
    let inner = ty.present().clone();
    let range = |v: crate::num::Rat| -> Result<(), String> {
        match c.ranges.get(name) {
            Some((lo, hi))
                if lo.is_some_and(|l| v.cmp_to(l) == std::cmp::Ordering::Less)
                    || hi.is_some_and(|h| v.cmp_to(h) == std::cmp::Ordering::Greater) =>
            {
                Err(out_of_range(name))
            }
            _ => Ok(()),
        }
    };
    match &inner {
        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number => {
            let Json::Int(n) = j else { return Err(not_integer(name)) };
            let v = crate::types::from_wire(*n, c.wire_scale(name));
            range(v)?;
            Ok(Val::Num(v))
        }
        Ty::Date => {
            let Some((y, m, d)) = j.as_str().and_then(civil) else { return Err(not_date(name)) };
            range(crate::types::date_ord(y, m, d))?;
            Ok(Val::Date(y, m, d))
        }
        Ty::Enum(en) => match j {
            Json::Str(s) if c.enums.get(en).is_some_and(|vs| vs.iter().any(|v| v == s)) => Ok(Val::Enum(s.clone())),
            _ => Err(not_in_enum(name, en)),
        },
        Ty::Bool => match j {
            Json::Bool(b) => Ok(Val::Bool(*b)),
            _ => Err(not_bool(name)),
        },
        Ty::Str => match j {
            Json::Str(s) => Ok(Val::Str(s.clone())),
            _ => Err(not_string(name)),
        },
        _ => Err(not_integer(name)),
    }
}

/// `YYYY-MM-DD`, four digits, two and two, of a day the calendar has: 2026-02-30 is not one.
pub fn civil(s: &str) -> Option<(i32, u32, u32)> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let digits = |r: std::ops::Range<usize>| -> Option<u32> {
        b[r.clone()].iter().all(u8::is_ascii_digit).then(|| s[r].parse().ok())?
    };
    let (y, m, d) = (digits(0..4)? as i32, digits(5..7)?, digits(8..10)?);
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let last = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    (1..=last).contains(&d).then_some((y, m, d))
}

/// What the door of every generated language refuses this input for, in the order the doors
/// look: each input in the order the rule declares it — there at all, of its kind, inside its
/// range — then the sequence and each of its elements, then every `constraint`, then the days of
/// a koyomi date; and last the rule's own contradiction, two elements taking under
/// `take_unique`. `None` when the input is answered. `input` is the object a vector's `in` is.
pub fn refusal(f: &RuleFile, c: &Checked, input_obj: &Json) -> Option<Refusal> {
    let get = |k: &str| input_obj.get(k);
    let mut vals: BTreeMap<String, Val> = BTreeMap::new();
    for i in &f.inputs {
        let name = &i.name.text;
        let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
        match one(c, name, &ty, get(name)) {
            Ok(v) => {
                vals.insert(name.clone(), v);
            }
            Err(e) => return Some(input(e)),
        }
    }
    if let Some(el) = &f.elements {
        let seq = &el.name.text;
        let items = match get(seq) {
            None | Some(Json::Null) => return Some(input(missing(seq))),
            Some(Json::Arr(xs)) => xs,
            Some(_) => return Some(input(not_sequence(seq))),
        };
        if let Some(cap) = count_cap(f, c) {
            if items.len() as i128 > cap {
                return Some(input(too_many(seq, cap)));
            }
        }
        let mut elems = Vec::new();
        for x in items {
            if !matches!(x, Json::Obj(_)) {
                return Some(input(not_object(seq)));
            }
            let mut e = BTreeMap::new();
            for fd in &el.fields {
                let name = &fd.name.text;
                let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
                match one(c, name, &ty, x.get(name)) {
                    Ok(v) => {
                        e.insert(name.clone(), v);
                    }
                    Err(err) => return Some(input(err)),
                }
            }
            elems.push(e);
        }
        // A sum over a field of the elements is refused the moment its total passes the top of
        // the range it declares (§15.100); a sum over what a table decides per element is the
        // walk's to work out, and is not looked at here.
        for d in sums(f) {
            let Some(hi) = c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi) else { continue };
            if !el.fields.iter().any(|fd| fd.name.text == d.column.text) {
                continue;
            }
            let total = elems.iter().fold(crate::num::Rat::int(0), |t, e| match e.get(&d.column.text) {
                Some(Val::Num(x)) => t.add(*x),
                _ => t,
            });
            if total.cmp_to(hi) == std::cmp::Ordering::Greater {
                return Some(input(out_of_range(&d.name.text)));
            }
        }
        vals.insert(seq.clone(), Val::Seq(elems));
    }
    for k in &f.constraints {
        if !crate::vectors::constraint_holds(k, vals.get(&k.left), vals.get(&k.right)) {
            return Some(input(constraint(&format!("{} {} {}", k.left, k.op.word(), k.right))));
        }
    }
    let mut names: Vec<&String> = c.day_sets.keys().collect();
    names.sort();
    for name in names {
        let set = &c.day_sets[name];
        if let Some(Val::Date(y, m, d)) = vals.get(name) {
            let ord = crate::types::date_ord(*y, *m, *d);
            if set.days.binary_search(&(ord.num as i64)).is_err() {
                let (file, date) = set.from.as_ref().map(|fr| (fr.file.clone(), fr.date.clone())).unwrap_or_default();
                return Some(input(not_a_day(name, &file, &date)));
            }
        }
    }
    if let Some(fold) = &f.fold {
        let (outs, _, _, _) = crate::eval::run_all_traced(f, c, vals.into_iter().collect());
        if outs.first().is_some_and(|(_, v)| v.is_none()) {
            return Some(Refusal { kind: "contradiction", error: fold_contradiction(&fold.verdict) });
        }
    }
    None
}

/// The `sum`s the rule declares over its sequence.
pub fn sums(f: &RuleFile) -> Vec<&crate::ast::AggDecl> {
    f.items
        .iter()
        .filter_map(|it| if let crate::ast::Item::Agg(d) = it { Some(d) } else { None })
        .filter(|d| d.kind == crate::ast::AggKind::Sum)
        .collect()
}

/// The cap a sequence is held to: the smallest upper bound any `count` over it declares, as the
/// generated door reads it (§15.58).
pub fn count_cap(f: &RuleFile, c: &Checked) -> Option<i128> {
    f.items
        .iter()
        .filter_map(|it| if let crate::ast::Item::Agg(d) = it { Some(d) } else { None })
        .filter(|d| d.kind == crate::ast::AggKind::Count)
        .filter_map(|d| c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi))
        .map(|hi| crate::types::wire_int(hi, 1))
        .min()
}
