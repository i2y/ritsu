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

/// Bytes that are not UTF-8, which only the Wasm module is given: a host hands it bytes, and every
/// other door is handed a string (§15.278).
pub fn not_utf8() -> String {
    tr!("入力が UTF-8 ではありません", "the input is not UTF-8")
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

/// A place in the caller's object the projection function reads a field of, holding anything but
/// an object (§15.234): `order.shipping`, or `order.lines[]` for an element.
pub fn not_an_object(name: &str) -> String {
    tr!("{name} がオブジェクトではありません", "{name} is not an object")
}

/// A field a `where` compares with a number that may have a fraction, holding anything but a
/// number (§15.234).
pub fn not_number(name: &str) -> String {
    tr!("{name} が数ではありません", "{name} is not a number")
}

/// A number or a date outside its declared range.
pub fn out_of_range(name: &str) -> String {
    tr!("{name} が範囲の外です", "{name} is out of range")
}

/// A sequence longer than a count over it can be.
pub fn too_many(name: &str, cap: i128) -> String {
    tr!("{name} の要素が多すぎます（上限 {cap}）", "{name} has too many elements (at most {cap})")
}

/// Two elements of a sequence with the same value of a field marked `unique` (§15.290). The value is
/// not in the sentence, as no other sentence carries one.
pub fn duplicate(seq: &str, field: &str) -> String {
    tr!("{seq} の要素に {field} が同じものが二つあります", "{seq} has two elements with the same {field}")
}

/// A combination a `constraint` rules out, said the way the rule writes it.
pub fn constraint(said: &str) -> String {
    tr!("制約が成り立ちません: {said}", "the constraint does not hold: {said}")
}

/// A combination a `never` line rules out, said the way the rule writes it (§15.282). The line goes
/// into a string of the generated code as it is, so it is written without the characters a string
/// literal of some language would need escaped.
pub fn never(said: &str) -> String {
    let said = said.replace(['"', '\\', '`', '\'', '$'], "");
    tr!("起こらない組み合わせです: {said}", "this combination does not happen: {said}")
}

/// A day a koyomi date does not come to. The file is written without the characters a string
/// literal of some language would need escaped.
pub fn not_a_day(name: &str, file: &str, date: &str) -> String {
    let file = file.replace(['"', '\\', '`', '\'', '$'], "");
    tr!("{name} は {file} の {date} がとる日ではありません", "{name} is not a day {date} of {file} comes to")
}

/// The drawn value of a pick by weight that is not below the total of the weights (§15.226): a
/// value the caller was to draw from below the total, and did not.
pub fn draw_past(name: &str) -> String {
    tr!("{name} が重みの合計より小さくありません", "{name} is not below the total of the weights")
}

/// The weights of a pick by weight whose total does not fit in int64, by the sequence the pick
/// walks (§15.226): every language adds the weights up in an int64 and refuses the call the moment
/// the total would pass it. The sequence is the caller's, and so is how long it is.
pub fn weights_past(seq: &str) -> String {
    tr!("{seq} の重みの合計が int64 に収まりません", "the total of the weights of {seq} does not fit in int64")
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

/// The largest integer a number travels as, 2^63−1, and with its sign turned, the smallest: where
/// a number's range writes no end, the door holds it to these (§15.295). int64 goes one further
/// down, to −2^63, and no door takes that one, so what a value can be is the same on both sides of
/// zero, as the proof that a value fits in int64 (E108) reads it.
pub const WIRE_MAX: i128 = i64::MAX as i128;

/// The two integers a door holds a number or a date to, as it travels (§15.295): the ends its
/// range writes — on the number's wire scale, as a date's day number — and for a number nothing
/// past ±(2^63−1), whether its range writes an end there or not. A date whose range writes no end
/// is held by its reader, which takes the years 0001 to 9999 (§15.279), and there is nothing more
/// to compare. `wide` is a language whose integers go past int64 (Python, NumPy, TypeScript,
/// JavaScript, Ruby); in one that holds the number in an int64 the upper end is left out where it
/// is 2^63−1, as nothing that integer holds is past it. `None` for a value of another type.
pub fn ends(c: &Checked, name: &str, wide: bool) -> (Option<i128>, Option<i128>) {
    let Some(ty) = c.ty_of(name) else { return (None, None) };
    let sc = c.wire_scale(name);
    let (lo, hi) = c.ranges.get(name).copied().unwrap_or((None, None));
    let (lo, hi) = (lo.map(|v| crate::types::wire_int(v, sc)), hi.map(|v| crate::types::wire_int(v, sc)));
    match ty.present() {
        Ty::Date => (lo, hi),
        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number => {
            let lo = lo.map_or(-WIRE_MAX, |l| l.max(-WIRE_MAX));
            let hi = hi.map_or(WIRE_MAX, |h| h.min(WIRE_MAX));
            (Some(lo), (wide || hi < WIRE_MAX).then_some(hi))
        }
        _ => (None, None),
    }
}

/// One value of an input or a field, read off the wire the way its door reads it: `None` where the
/// door refuses it. What a case built on the wire is read back into the evaluator's values as.
pub(crate) fn value(c: &Checked, name: &str, j: &Json) -> Option<Val> {
    one(c, name, &c.ty_of(name)?, Some(j)).ok()
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
            let Some(n) = whole(j) else { return Err(not_integer(name)) };
            let v = crate::types::from_wire(n, c.wire_scale(name));
            range(v)?;
            // Nothing past ±(2^63−1) as it travels, whatever its range writes (§15.295).
            if n.abs() > WIRE_MAX {
                return Err(out_of_range(name));
            }
            Ok(Val::Num(v))
        }
        Ty::Date => {
            let Some((y, m, d)) = j.as_str().and_then(civil) else { return Err(not_date(name)) };
            range(crate::types::date_ord(y, m, d))?;
            Ok(Val::Date(y, m, d))
        }
        Ty::Enum(en) => match j {
            // A value of an ordered enum outside the range its input declares (§15.210).
            Json::Str(s) if c.enum_ranges.get(name).is_some_and(|vs| !vs.contains(s)) && c.enums.get(en).is_some_and(|vs| vs.contains(s)) => {
                Err(out_of_range(name))
            }
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

/// A JSON number whose value is a whole number, as that integer (§15.205): `1000`, and `1000.0`
/// or `-0.0`, which JSON does not tell apart from it and a caller's `float` writes. `1000.5` is
/// none, and neither is anything that is not a number. The reader keeps a fraction as the digits
/// it was written with and reads no exponent (ritsu's DESIGN 4.9), so `1e3` never comes this far.
pub fn whole(j: &Json) -> Option<i128> {
    match j {
        Json::Int(n) => Some(*n),
        Json::Frac(s) => {
            let (int, frac) = s.split_once('.')?;
            frac.bytes().all(|b| b == b'0').then(|| int.parse::<i128>().ok())?
        }
        _ => None,
    }
}

/// `YYYY-MM-DD`, four digits, two and two, of a day the calendar has from the year 0001 on: 2026-02-30
/// is not one, and neither is 0000-12-31 (§15.279).
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
    (y >= 1 && (1..=last).contains(&d)).then_some((y, m, d))
}

/// What the door of every generated language refuses this input for, in the order the doors
/// look: each input in the order the rule declares it — there at all, of its kind, inside its
/// range — then the sequence and each of its elements, two elements with the same value of a
/// `unique` field, then every `constraint`, then the days of a koyomi date, then what the walks
/// refuse (weights whose total passes int64, a sum past its range, a drawn value past the weights);
/// and last the rule's own contradiction, two elements taking under `take_unique`. `None` when the
/// input is answered.
/// `input` is the object a vector's `in` is.
pub fn refusal(f: &RuleFile, c: &Checked, input_obj: &Json) -> Option<Refusal> {
    read(f, c, input_obj).err()
}

/// The input as the values the rule is run on, or what the door refuses it for: the same walk
/// as [`refusal`], which is this with the values dropped. `rulec run` runs the reference
/// evaluator on what this gives, so it answers where a generated door answers and refuses where
/// it refuses (the sequences are in the map by their name, as `Val::Seq`).
pub fn read(f: &RuleFile, c: &Checked, input_obj: &Json) -> Result<BTreeMap<String, Val>, Refusal> {
    let get = |k: &str| input_obj.get(k);
    let mut vals: BTreeMap<String, Val> = BTreeMap::new();
    for i in &f.inputs {
        let name = &i.name.text;
        let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
        match one(c, name, &ty, get(name)) {
            Ok(v) => {
                vals.insert(name.clone(), v);
            }
            Err(e) => return Err(input(e)),
        }
    }
    // Each sequence, in the order the rule declares them (§15.219).
    for el in &f.elements {
        let seq = &el.name.text;
        let items = match get(seq) {
            None | Some(Json::Null) => return Err(input(missing(seq))),
            Some(Json::Arr(xs)) => xs,
            Some(_) => return Err(input(not_sequence(seq))),
        };
        if let Some(cap) = count_cap(f, c, seq) {
            if items.len() as i128 > cap {
                return Err(input(too_many(seq, cap)));
            }
        }
        let mut elems = Vec::new();
        for x in items {
            if !matches!(x, Json::Obj(_)) {
                return Err(input(not_object(seq)));
            }
            let mut e = BTreeMap::new();
            for fd in &el.fields {
                let name = &fd.name.text;
                let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
                match one(c, name, &ty, x.get(name)) {
                    Ok(v) => {
                        e.insert(name.clone(), v);
                    }
                    Err(err) => return Err(input(err)),
                }
            }
            elems.push(e);
        }
        // Two elements with the same value of a `unique` field (§15.290), once every element has
        // been read and before any sum over the sequence: element by element, each field in the
        // order the rule declares them, as the generated walk looks.
        if let Some(field) = repeated(el, &elems) {
            return Err(input(duplicate(seq, field)));
        }
        // A sum over a field of the elements is refused the moment its total passes the top of
        // the range it declares (§15.100); a sum over what a table decides per element is the
        // walk's to work out, and is not looked at here.
        for d in sums(f).into_iter().filter(|d| d.over == *seq) {
            let Some(hi) = c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi) else { continue };
            if !el.fields.iter().any(|fd| fd.name.text == d.column.text) {
                continue;
            }
            let total = elems.iter().fold(crate::num::Rat::int(0), |t, e| match e.get(&d.column.text) {
                Some(Val::Num(x)) => t.add(*x),
                _ => t,
            });
            if total.cmp_to(hi) == std::cmp::Ordering::Greater {
                return Err(input(out_of_range(&d.name.text)));
            }
        }
        vals.insert(seq.clone(), Val::Seq(elems));
    }
    for k in &f.constraints {
        if !crate::vectors::constraint_holds(k, &|n| vals.get(n)) {
            return Err(input(constraint(&k.said())));
        }
    }
    for n in f.door_nevers() {
        if !crate::vectors::never_clear(n, c, &|m| vals.get(m)) {
            return Err(input(never(&n.said())));
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
                return Err(input(not_a_day(name, &file, &date)));
            }
        }
    }
    // Weights of a pick by weight whose total passes int64 (§15.226): the generated walk refuses the
    // call the moment the total does, before it reads another element, so a walk that stopped there
    // reaches none of what is looked at below.
    if let Some(n) = crate::eval::weights_past(f, c, vals.clone().into_iter().collect()) {
        return Err(input(weights_past(&n)));
    }
    // A sum over what a table decides per element, past the top of its range once the walk is
    // over (§15.100): the generated code refuses it there.
    if let Some(n) = crate::eval::sum_past_range(f, c, vals.clone().into_iter().collect()) {
        return Err(input(out_of_range(&n)));
    }
    // A drawn value not below the total of the weights of a pick by weight (§15.226).
    if let Some(n) = crate::eval::drawn_past(f, c, vals.clone().into_iter().collect()) {
        return Err(input(draw_past(&n)));
    }
    if let Some(i) = crate::eval::contradiction(f, c, vals.clone().into_iter().collect()) {
        if let Some(crate::ast::Item::Fold(fold)) = f.items.get(i) {
            return Err(Refusal { kind: "contradiction", error: fold_contradiction(&fold.verdict) });
        }
    }
    Ok(vals)
}

/// The first field marked `unique` that an element shares with an element before it (§15.290):
/// element by element, and for each element its fields in the order the rule declares them, which
/// is the order the generated walk meets them in. Values compare as values: a number by its value, a
/// date by its day.
pub fn repeated<'e>(el: &'e crate::ast::ElementsDecl, elems: &[BTreeMap<String, Val>]) -> Option<&'e str> {
    let fields: Vec<&str> = el.fields.iter().filter(|fd| fd.unique.is_some()).map(|fd| fd.name.text.as_str()).collect();
    if fields.is_empty() {
        return None;
    }
    for (k, e) in elems.iter().enumerate() {
        for &field in &fields {
            let Some(v) = e.get(field) else { continue };
            if elems[..k].iter().any(|p| p.get(field) == Some(v)) {
                return Some(field);
            }
        }
    }
    None
}

/// The `sum`s the rule declares, over any of its sequences.
pub fn sums(f: &RuleFile) -> Vec<&crate::ast::AggDecl> {
    f.items
        .iter()
        .filter_map(|it| if let crate::ast::Item::Agg(d) = it { Some(d) } else { None })
        .filter(|d| d.kind == crate::ast::AggKind::Sum)
        .collect()
}

/// The cap a sequence is held to: the smallest upper bound any `count` over it declares, as the
/// generated door reads it (§15.58).
pub fn count_cap(f: &RuleFile, c: &Checked, seq: &str) -> Option<i128> {
    f.items
        .iter()
        .filter_map(|it| if let crate::ast::Item::Agg(d) = it { Some(d) } else { None })
        .filter(|d| d.kind == crate::ast::AggKind::Count && d.over == seq)
        .filter_map(|d| c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi))
        .map(|hi| crate::types::wire_int(hi, 1))
        .min()
}
