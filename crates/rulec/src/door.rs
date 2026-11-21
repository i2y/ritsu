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

/// Where the generated code puts the index of the element a sentence is about (§15.324): each
/// generator turns it into the index its walk or its reader is at, in the way its language builds a
/// string. The reference evaluator says the index itself.
pub const HERE: &str = "\u{1}";

/// Where the generated code puts the index of the earlier element of the two a sentence is about:
/// the element a value of a `unique` field was first seen on, or the element a `take_unique` first
/// took (§15.324).
pub const BEFORE: &str = "\u{2}";

/// One element of a sequence as a sentence names it (§15.324): `lines[1]`, the sequence by the key
/// the input's JSON has it under and the element by its index in that array, from 0.
pub fn place(seq: &str, i: impl std::fmt::Display) -> String {
    format!("{seq}[{i}]")
}

/// A field of one element as a sentence names it (§15.324): `lines[1].subtotal`. A `define` worked
/// out for each element is named the same way, by its own name.
pub fn element(seq: &str, i: impl std::fmt::Display, field: &str) -> String {
    format!("{seq}[{i}].{field}")
}

/// An element of the sequence that is not an object, by its place (§15.324): `lines[1] is not an
/// object`.
pub fn not_object(seq: &str, i: impl std::fmt::Display) -> String {
    not_an_object(&place(seq, i))
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

/// Two elements of a sequence with the same value of a field marked `unique` (§15.290), and which
/// two, the earlier one first (§15.324): the element the value was first seen on, and the one it
/// came round again on. The value is not in the sentence, as no other sentence carries one.
pub fn duplicate(seq: &str, field: &str, before: impl std::fmt::Display, here: impl std::fmt::Display) -> String {
    let (a, b) = (place(seq, before), place(seq, here));
    tr!("{seq} の要素に {field} が同じものが二つあります（{a} と {b}）", "{seq} has two elements with the same {field} ({a} and {b})")
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

/// Two elements both taking under `take_unique`: the rule has no answer. Which two, the earlier one
/// first (§15.324): the element the fold took, and the one that matched again.
pub fn fold_contradiction(verdict: &str, seq: &str, before: impl std::fmt::Display, here: impl std::fmt::Display) -> String {
    let (a, b) = (place(seq, before), place(seq, here));
    tr!(
        "畳み込み {verdict}: take_unique に二件当たりました（{a} と {b}）",
        "fold {verdict}: two elements matched a take_unique ({a} and {b})"
    )
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
    let ty = c.ty_of(name)?;
    let v = typed(c, name, name, &ty, Some(j)).ok()?;
    ranged(c, name, name, &ty, &v).ok().map(|_| v)
}

/// The first of the two looks the door takes at one value (§15.328), where every reader of the wire
/// takes it: there at all, and of its kind — a whole number that int64 holds, a `YYYY-MM-DD` day the
/// calendar has, a value of the enum, a truth value, a string. The absent value of an optional input
/// is `none`, as the reference evaluator reads it. `name` is the input or the field, and `said` what
/// the sentence calls it: the name itself, or for a field of an element its place, `lines[1].amount`
/// (§15.324).
fn typed(c: &Checked, name: &str, said: &str, ty: &Ty, j: Option<&Json>) -> Result<Val, String> {
    let Some(j) = j else { return Err(missing(said)) };
    if *j == Json::Null {
        return if matches!(ty, Ty::Opt(_)) { Ok(Val::Enum(crate::kw::NONE.into())) } else { Err(missing(said)) };
    }
    let inner = ty.present().clone();
    match &inner {
        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number => {
            let Some(n) = whole(j) else { return Err(not_integer(said)) };
            // A whole number int64 does not hold is past what a reader of every language takes,
            // there and not later (§15.298).
            if n > i64::MAX as i128 || n < i64::MIN as i128 {
                return Err(out_of_range(said));
            }
            Ok(Val::Num(crate::types::from_wire(n, c.wire_scale(name))))
        }
        Ty::Date => {
            let Some((y, m, d)) = j.as_str().and_then(civil) else { return Err(not_date(said)) };
            Ok(Val::Date(y, m, d))
        }
        Ty::Enum(en) => match j {
            Json::Str(s) if c.enums.get(en).is_some_and(|vs| vs.iter().any(|v| v == s)) => Ok(Val::Enum(s.clone())),
            _ => Err(not_in_enum(said, en)),
        },
        Ty::Bool => match j {
            Json::Bool(b) => Ok(Val::Bool(*b)),
            _ => Err(not_bool(said)),
        },
        Ty::Str => match j {
            Json::Str(s) => Ok(Val::Str(s.clone())),
            _ => Err(not_string(said)),
        },
        _ => Err(not_integer(said)),
    }
}

/// The second look at a value the first took (§15.328): inside the range its input or field
/// declares — a number between the ends its range writes and nothing past ±(2^63−1) as it travels
/// (§15.295), a date between its ends, a value of an ordered enum among those its range lets in
/// (§15.210). An optional input with no value has nothing to look at.
pub(crate) fn ranged(c: &Checked, name: &str, said: &str, ty: &Ty, v: &Val) -> Result<(), String> {
    if matches!(ty, Ty::Opt(_)) && *v == Val::Enum(crate::kw::NONE.into()) {
        return Ok(());
    }
    let outside = |x: crate::num::Rat| {
        c.ranges.get(name).is_some_and(|(lo, hi)| {
            lo.is_some_and(|l| x.cmp_to(l) == std::cmp::Ordering::Less) || hi.is_some_and(|h| x.cmp_to(h) == std::cmp::Ordering::Greater)
        })
    };
    let past = match v {
        Val::Num(x) => outside(*x) || crate::types::wire_int(*x, c.wire_scale(name)).abs() > WIRE_MAX,
        Val::Date(y, m, d) => outside(crate::types::date_ord(*y, *m, *d)),
        Val::Enum(s) => c.enum_ranges.get(name).is_some_and(|vs| !vs.contains(s)),
        _ => false,
    };
    if past { Err(out_of_range(said)) } else { Ok(()) }
}

/// What the door refuses one element for where the walk comes to it (§15.328).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementRefused {
    /// A field outside its range: the field, and the element's index.
    Range { field: String, at: usize },
    /// A field marked `unique` with the value an element before it had (§15.290): the field, the
    /// element the value was first seen on, and this one.
    Unique { field: String, before: usize, here: usize },
}

impl ElementRefused {
    /// The door's sentence of it, for the sequence it is an element of.
    pub fn said(&self, seq: &str) -> String {
        match self {
            ElementRefused::Range { field, at } => out_of_range(&element(seq, at, field)),
            ElementRefused::Unique { field, before, here } => duplicate(seq, field, before, here),
        }
    }
}

/// What the door says of element `k` of sequence `s` where the walk comes to it (§15.328): each of
/// its fields in the order the rule declares them, outside its range; then each field marked
/// `unique`, in the same order, with the value an element before it had (§15.290). `elems` are the
/// elements of the sequence, read in the first look. `None` when the door lets the element through.
pub(crate) fn element_refusal(f: &RuleFile, c: &Checked, s: usize, k: usize, elems: &[BTreeMap<String, Val>]) -> Option<ElementRefused> {
    let el = &f.elements[s];
    let e = &elems[k];
    for fd in &el.fields {
        let name = &fd.name.text;
        let Some(v) = e.get(name) else { continue };
        if ranged(c, name, name, &c.ty_of(name).unwrap_or(Ty::Unknown), v).is_err() {
            return Some(ElementRefused::Range { field: name.clone(), at: k });
        }
    }
    for fd in el.fields.iter().filter(|fd| fd.unique.is_some()) {
        let field = &fd.name.text;
        let Some(v) = e.get(field) else { continue };
        if let Some(before) = elems[..k].iter().position(|p| p.get(field) == Some(v)) {
            return Some(ElementRefused::Unique { field: field.clone(), before, here: k });
        }
    }
    None
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

/// What every way the rule is reached refuses this input for: the first thing wrong with it, in
/// the one order every door looks in (§15.328). `None` when the input is answered. `input` is the
/// object a vector's `in` is.
pub fn refusal(f: &RuleFile, c: &Checked, input_obj: &Json) -> Option<Refusal> {
    read(f, c, input_obj).err()
}

/// The input as the values the rule is run on, or what the door refuses it for: the same look as
/// [`refusal`], which is this with the values dropped. `rulec run` runs the reference evaluator on
/// what this gives, so it answers where a generated door answers and refuses where it refuses (the
/// sequences are in the map by their name, as `Val::Seq`).
///
/// An input with two or more things wrong with it is refused for the first of them, in this order,
/// by every way (§15.328):
///
/// 1. The first look, where a reader takes the arguments off the wire, in the order the function
///    takes them: each input in the order the rule declares it, there at all and of its kind (a
///    whole number int64 holds, a day the calendar has, a value of its enum, a truth value, a
///    string); then each sequence in the order the rule declares them, there at all, an array, and
///    each of its elements an object whose fields, in the order the rule declares them, are there
///    and of their kinds.
/// 2. The second look, where the function begins: each input inside its range, in the order the
///    rule declares them; each `constraint`; each `never` the door holds; the days of a koyomi date;
///    each sequence no longer than a `count` over it can be.
/// 3. What the function works out, in the order it does ([`crate::eval::walked_refusal`]): a value
///    worked out before the walks outside the range the rule accepts for it; then each walk, element
///    by element — on the first walk over a sequence, the element's fields inside their ranges and
///    its `unique` fields, then the values worked out for it and each summary's step (two elements
///    taking under `take_unique`, weights whose total passes int64) — and once the walk is over, a
///    `sum` or a `count` outside its range and a drawn value not below the weights; then what reads
///    what the walk answered. A walk whose work ends at a `stop` still looks at each element after
///    it with the door.
pub fn read(f: &RuleFile, c: &Checked, input_obj: &Json) -> Result<BTreeMap<String, Val>, Refusal> {
    let get = |k: &str| input_obj.get(k);
    let ty = |n: &str| c.ty_of(n).unwrap_or(Ty::Unknown);
    let mut vals: BTreeMap<String, Val> = BTreeMap::new();
    // 1. The first look: each argument off the wire, the inputs and then the sequences.
    for i in &f.inputs {
        let name = &i.name.text;
        vals.insert(name.clone(), typed(c, name, name, &ty(name), get(name)).map_err(input)?);
    }
    // Each sequence, in the order the rule declares them (§15.219). An element is said by its index
    // in the array, from 0 (§15.324).
    let mut lengths: Vec<usize> = Vec::new();
    for el in &f.elements {
        let seq = &el.name.text;
        let items = match get(seq) {
            None | Some(Json::Null) => return Err(input(missing(seq))),
            Some(Json::Arr(xs)) => xs,
            Some(_) => return Err(input(not_sequence(seq))),
        };
        let mut elems = Vec::new();
        for (k, x) in items.iter().enumerate() {
            if !matches!(x, Json::Obj(_)) {
                return Err(input(not_object(seq, k)));
            }
            let mut e = BTreeMap::new();
            for fd in &el.fields {
                let name = &fd.name.text;
                e.insert(name.clone(), typed(c, name, &element(seq, k, name), &ty(name), x.get(name)).map_err(input)?);
            }
            elems.push(e);
        }
        lengths.push(elems.len());
        vals.insert(seq.clone(), Val::Seq(elems));
    }
    // 2. The second look, where the function begins.
    for i in &f.inputs {
        let name = &i.name.text;
        ranged(c, name, name, &ty(name), &vals[name]).map_err(input)?;
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
    for (el, n) in f.elements.iter().zip(&lengths) {
        if let Some(cap) = count_cap(f, c, &el.name.text) {
            if *n as i128 > cap {
                return Err(input(too_many(&el.name.text, cap)));
            }
        }
    }
    // 3. What the function works out, in the order it does.
    match crate::eval::walked_refusal(f, c, vals.clone().into_iter().collect()) {
        None => Ok(vals),
        Some(crate::eval::Refused::Contradiction(b)) => match f.items.get(b.item) {
            Some(crate::ast::Item::Fold(fold)) => {
                Err(Refusal { kind: "contradiction", error: fold_contradiction(&fold.verdict, &fold.over, b.before, b.here) })
            }
            _ => Ok(vals),
        },
        Some(why) => Err(input(why.said(f).unwrap_or_default())),
    }
}

/// The first field marked `unique` that an element shares with an element before it (§15.290):
/// element by element, and for each element its fields in the order the rule declares them, which
/// is the order the generated walk meets them in. Values compare as values: a number by its value, a
/// date by its day. With the field, the index of the first element that had the value and of the
/// element that has it again (§15.324).
pub fn repeated<'e>(el: &'e crate::ast::ElementsDecl, elems: &[BTreeMap<String, Val>]) -> Option<(&'e str, usize, usize)> {
    let fields: Vec<&str> = el.fields.iter().filter(|fd| fd.unique.is_some()).map(|fd| fd.name.text.as_str()).collect();
    if fields.is_empty() {
        return None;
    }
    for (k, e) in elems.iter().enumerate() {
        for &field in &fields {
            let Some(v) = e.get(field) else { continue };
            if let Some(before) = elems[..k].iter().position(|p| p.get(field) == Some(v)) {
                return Some((field, before, k));
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
