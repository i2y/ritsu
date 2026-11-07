//! Protovalidate, as far as a value's bounds go (rulec's §15.132 and §15.140): what
//! `(buf.validate.field)` lets through a field, and what `(buf.validate.message)` and
//! `(buf.validate.oneof)` ask of a message.
//!
//! Only the rules that bound a value the way a rule's input is bounded are read: the integer
//! comparisons, the element count of a collection, the listed values of a string, and the CEL
//! expressions, which are kept as text (rulec's `src/cel.rs` reads them). Anything else that
//! could narrow what passes — a predefined rule, a pattern — is recorded by name in `unread` and
//! not interpreted. Reading it as not there makes the field look wider than it is, never
//! narrower: a check that uses this may then speak where it did not need to, and never stays
//! quiet where it should have spoken.

use crate::model::{Field, Opt};
use crate::value::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rules {
    pub required: bool,
    /// `IGNORE_IF_ZERO_VALUE` and its older names, or `IGNORE_ALWAYS`.
    pub ignore: Option<String>,
    pub int: IntRules,
    /// The rules on an enum: its values by number. `defined_only` is what a reader of protojson
    /// holds it to anyway, since a number it does not know does not read as a string.
    pub enum_values: IntRules,
    pub min_items: Option<i128>,
    pub max_items: Option<i128>,
    pub str_const: Option<String>,
    pub str_in: Vec<String>,
    pub str_not_in: Vec<String>,
    /// The least length the string rules ask for (`min_len`, `len`, and the two in bytes): what
    /// a date needs to know of them, which is whether "" passes.
    pub str_min_len: Option<i128>,
    /// The CEL expressions on the field, from `cel` and `cel_expression`, `this` being the field.
    pub cel: Vec<String>,
    pub unread: Vec<String>,
}

/// The integer rules, whichever of the ten integer kinds they were written under.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IntRules {
    pub konst: Option<i128>,
    pub gt: Option<i128>,
    pub gte: Option<i128>,
    pub lt: Option<i128>,
    pub lte: Option<i128>,
    pub in_: Vec<i128>,
    pub not_in: Vec<i128>,
}

/// The rules on a message rather than on one field: `(buf.validate.message)` in its options,
/// and the `oneof`s it declares.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MsgRules {
    /// The CEL expressions on the message, from `cel` and `cel_expression`, `this` being the
    /// message.
    pub cel: Vec<String>,
    /// The groups of fields of which at most one may be set, in the order they are written:
    /// every `oneof` the message declares (at its end), and every
    /// `(buf.validate.message).oneof`. `required` asks for exactly one.
    pub oneofs: Vec<OneofRule>,
    /// `(buf.validate.message).disabled`. Protovalidate has since removed it; where a release
    /// that still reads it validates the message, it validates nothing, so every rule of the
    /// message is dropped rather than trusted.
    pub disabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OneofRule {
    pub fields: Vec<String>,
    pub required: bool,
}

/// The ten integer kinds of a `.proto`, which are also the names Protovalidate files their
/// rules under.
pub const INT_KINDS: &[&str] = &["int32", "int64", "uint32", "uint64", "sint32", "sint64", "fixed32", "fixed64", "sfixed32", "sfixed64"];

const FIELD: &str = "(buf.validate.field)";
const MESSAGE: &str = "(buf.validate.message)";
const ONEOF: &str = "(buf.validate.oneof)";

/// An option's name without its spaces, as the rules are matched by.
fn bare(o: &Opt) -> String {
    o.name.chars().filter(|c| !c.is_whitespace()).collect()
}

/// What `(buf.validate.field)` says about one field, in either spelling: `.int64.gte = 1` or
/// `.int64 = {gte: 1}`.
pub fn field_rules(opts: &[Opt]) -> Rules {
    let mut r = Rules::default();
    let mut leaves = Vec::new();
    for o in opts {
        let Some(v) = &o.value else { continue };
        let name = bare(o);
        let Some(rest) = name.strip_prefix(FIELD) else { continue };
        if rest.contains('(') {
            // `(buf.validate.field).int64.(my.rule) = …`: a predefined rule.
            r.unread.push(rest.trim_start_matches('.').to_string());
            continue;
        }
        let base: Vec<String> = rest.split('.').filter(|s| !s.is_empty()).map(String::from).collect();
        flatten(base, v.clone(), &mut leaves);
    }
    for (path, v) in leaves {
        let p: Vec<&str> = path.iter().map(String::as_str).collect();
        match p.as_slice() {
            ["required"] => r.required = matches!(&v, Value::Id(t) if t == "true"),
            ["ignore"] => r.ignore = if let Value::Id(t) = &v { Some(t.clone()) } else { None },
            [k, rule] if INT_KINDS.contains(k) => match *rule {
                "const" => r.int.konst = int_of(&v),
                "gt" => r.int.gt = int_of(&v),
                "gte" => r.int.gte = int_of(&v),
                "lt" => r.int.lt = int_of(&v),
                "lte" => r.int.lte = int_of(&v),
                "in" => r.int.in_.extend(ints_of(&v)),
                "not_in" => r.int.not_in.extend(ints_of(&v)),
                "example" => {}
                _ => r.unread.push(path.join(".")),
            },
            ["enum", "const"] => r.enum_values.konst = int_of(&v),
            ["enum", "in"] => r.enum_values.in_.extend(ints_of(&v)),
            ["enum", "not_in"] => r.enum_values.not_in.extend(ints_of(&v)),
            ["repeated", "min_items"] => r.min_items = int_of(&v),
            ["repeated", "max_items"] => r.max_items = int_of(&v),
            // `unique` and the rules on each element say nothing about how many there are.
            ["repeated", ..] => {}
            ["string", "const"] => r.str_const = strs_of(&v).into_iter().next(),
            ["string", "in"] => r.str_in.extend(strs_of(&v)),
            ["string", "not_in"] => r.str_not_in.extend(strs_of(&v)),
            ["string", "example"] => {}
            ["string", "min_len" | "len" | "min_bytes" | "len_bytes"] => {
                r.str_min_len = r.str_min_len.max(int_of(&v));
                r.unread.push(path.join("."));
            }
            ["string", ..] => r.unread.push(path.join(".")),
            ["cel", "expression"] | ["cel_expression"] => r.cel.extend(strs_of(&v)),
            ["cel"] => {
                if let Value::List(xs) = &v {
                    for x in xs {
                        if let Value::Msg(kv) = x {
                            r.cel.extend(kv.iter().filter(|(k, _)| k == "expression").flat_map(|(_, v)| strs_of(v)));
                        }
                    }
                }
            }
            ["cel", ..] => {}
            _ if p.iter().any(|s| s.starts_with('[')) => r.unread.push(path.join(".")),
            // The rules of the other kinds (`double`, `timestamp`, `map`, `enum`, …) bound
            // nothing this reader compares.
            _ => {}
        }
    }
    r
}

/// One `option` statement of a message: what `(buf.validate.message)` asks of it.
pub fn message_option(o: &Opt, rules: &mut MsgRules) {
    let Some(v) = &o.value else { return };
    if let Some(rest) = bare(o).strip_prefix(MESSAGE) {
        let path: Vec<&str> = rest.split('.').filter(|s| !s.is_empty()).collect();
        message_rule(&path, v, rules);
    }
}

/// One `option` statement of a `oneof`: whether `(buf.validate.oneof)` asks for one member.
pub fn oneof_option(o: &Opt, rule: &mut OneofRule) {
    let Some(v) = &o.value else { return };
    if let Some(rest) = bare(o).strip_prefix(ONEOF) {
        let required = match (rest.trim_start_matches('.'), v) {
            ("required", Value::Id(t)) => t == "true",
            ("", Value::Msg(kv)) => kv.iter().any(|(k, v)| k == "required" && matches!(v, Value::Id(t) if t == "true")),
            _ => false,
        };
        rule.required |= required;
    }
}

/// A message `(buf.validate.message).disabled`: no rule of a field is trusted, the CEL goes, and
/// of the groups only the `oneof`s of the wire format stay, asking for nothing.
pub fn disable(fields: &mut [Field], rules: &mut MsgRules) {
    for f in fields.iter_mut() {
        f.rules = Rules { unread: vec![format!("{MESSAGE}.disabled")], ..Rules::default() };
    }
    rules.cel.clear();
    rules.oneofs.retain(|o| o.fields.iter().all(|n| fields.iter().any(|f| f.name == *n && f.oneof.is_some())));
    for o in &mut rules.oneofs {
        o.required = false;
    }
}

/// One part of `(buf.validate.message)`, in any of the ways text format lets it be written.
fn message_rule(path: &[&str], v: &Value, rules: &mut MsgRules) {
    let expression = |kv: &[(String, Value)]| {
        kv.iter().find_map(|(k, v)| match v {
            Value::Str(s) if k == "expression" => Some(s.clone()),
            _ => None,
        })
    };
    match (path, v) {
        ([], Value::Msg(kv)) => {
            for (k, v) in kv {
                message_rule(&[k.as_str()], v, rules);
            }
        }
        (["cel"], Value::Msg(kv)) => rules.cel.extend(expression(kv)),
        (["cel", "expression"], Value::Str(s)) => rules.cel.push(s.clone()),
        (["cel_expression"], Value::Str(s)) => rules.cel.push(s.clone()),
        (["cel" | "cel_expression" | "oneof"], Value::List(xs)) => {
            for x in xs {
                message_rule(path, x, rules);
            }
        }
        (["oneof"], Value::Msg(kv)) => {
            let fields = kv.iter().filter(|(k, _)| k == "fields").flat_map(|(_, v)| strs_of(v)).collect();
            let required = kv.iter().any(|(k, v)| k == "required" && matches!(v, Value::Id(t) if t == "true"));
            rules.oneofs.push(OneofRule { fields, required });
        }
        (["disabled"], Value::Id(t)) => rules.disabled |= t == "true",
        _ => {}
    }
}

fn flatten(path: Vec<String>, v: Value, out: &mut Vec<(Vec<String>, Value)>) {
    match v {
        Value::Msg(kv) => {
            for (k, v) in kv {
                let mut p = path.clone();
                p.push(k);
                flatten(p, v, out);
            }
        }
        other => out.push((path, other)),
    }
}

/// An integer as written: decimal or `0x…`, with a sign; `1.0` is an integer written as a
/// fraction, and `1.5` bounds nothing an integer can be.
pub fn int_of(v: &Value) -> Option<i128> {
    let Value::Num(s) = v else { return None };
    let (neg, digits) = match s.strip_prefix('-') {
        Some(d) => (true, d),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let n = match digits.strip_prefix("0x").or_else(|| digits.strip_prefix("0X")) {
        Some(h) => i128::from_str_radix(h, 16).ok()?,
        None => match digits.split_once('.') {
            Some((w, f)) if f.chars().all(|c| c == '0') => w.parse().ok()?,
            Some(_) => return None,
            None => digits.parse().ok()?,
        },
    };
    Some(if neg { -n } else { n })
}

fn ints_of(v: &Value) -> Vec<i128> {
    match v {
        Value::List(vs) => vs.iter().filter_map(int_of).collect(),
        one => int_of(one).into_iter().collect(),
    }
}

fn strs_of(v: &Value) -> Vec<String> {
    v.strings()
}

// --- What passes a field, as integers (rulec's §15.132) ------------------------------------

/// Integers as closed intervals, sorted and apart; `None` is an open end.
#[derive(Debug, Clone, PartialEq)]
pub struct Ints(pub Vec<(Option<i128>, Option<i128>)>);

impl Ints {
    pub fn between(lo: Option<i128>, hi: Option<i128>) -> Ints {
        match (lo, hi) {
            (Some(a), Some(b)) if a > b => Ints(Vec::new()),
            _ => Ints(vec![(lo, hi)]),
        }
    }

    pub fn points(vs: &[i128]) -> Ints {
        Ints(vs.iter().map(|&x| (Some(x), Some(x))).collect()).normal()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn intersect(&self, o: &Ints) -> Ints {
        let mut out = Vec::new();
        for &(a1, b1) in &self.0 {
            for &(a2, b2) in &o.0 {
                let lo = match (a1, a2) {
                    (None, x) | (x, None) => x,
                    (Some(x), Some(y)) => Some(x.max(y)),
                };
                let hi = match (b1, b2) {
                    (None, x) | (x, None) => x,
                    (Some(x), Some(y)) => Some(x.min(y)),
                };
                out.push((lo, hi));
            }
        }
        Ints(out).normal()
    }

    pub fn union(&self, o: &Ints) -> Ints {
        let mut v = self.0.clone();
        v.extend(o.0.iter().copied());
        Ints(v).normal()
    }

    pub fn without(&self, x: i128) -> Ints {
        let mut out = Vec::new();
        for &(a, b) in &self.0 {
            if a.is_some_and(|a| x < a) || b.is_some_and(|b| x > b) {
                out.push((a, b));
                continue;
            }
            if a != Some(x) {
                out.push((a, Some(x - 1)));
            }
            if b != Some(x) {
                out.push((Some(x + 1), b));
            }
        }
        Ints(out).normal()
    }

    /// Sorted, the empty ones dropped, and merged where they meet.
    pub fn normal(self) -> Ints {
        let mut v: Vec<_> = self.0.into_iter().filter(|(a, b)| !matches!((a, b), (Some(a), Some(b)) if a > b)).collect();
        v.sort_by(|x, y| match (x.0, y.0) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, _) => std::cmp::Ordering::Less,
            (_, None) => std::cmp::Ordering::Greater,
            (Some(a), Some(b)) => a.cmp(&b),
        });
        let mut out: Vec<(Option<i128>, Option<i128>)> = Vec::new();
        for (a, b) in v {
            if let Some(last) = out.last_mut() {
                let meets = match (last.1, a) {
                    (None, _) | (_, None) => true,
                    (Some(h), Some(a)) => a <= h.saturating_add(1),
                };
                if meets {
                    last.1 = match (last.1, b) {
                        (None, _) | (_, None) => None,
                        (Some(x), Some(y)) => Some(x.max(y)),
                    };
                    continue;
                }
            }
            out.push((a, b));
        }
        Ints(out)
    }

    pub fn min(&self) -> Option<i128> {
        self.0.first().and_then(|(a, _)| *a)
    }

    pub fn max(&self) -> Option<i128> {
        self.0.last().and_then(|(_, b)| *b)
    }
}

/// The values an integer kind can hold on the wire.
pub fn int_bounds(ty: &str) -> Option<(i128, i128)> {
    match ty.rsplit('.').next().unwrap_or(ty) {
        "int32" | "sint32" | "sfixed32" => Some((i32::MIN as i128, i32::MAX as i128)),
        "uint32" | "fixed32" => Some((0, u32::MAX as i128)),
        "int64" | "sint64" | "sfixed64" => Some((i64::MIN as i128, i64::MAX as i128)),
        "uint64" | "fixed64" => Some((0, u64::MAX as i128)),
        _ => None,
    }
}

/// Whether Protovalidate lets an unset field of implicit presence through whatever the other
/// rules say: `IGNORE_IF_ZERO_VALUE`, and the two names it had before.
pub fn ignores_zero(r: &Rules) -> bool {
    r.ignore.as_deref().is_some_and(|g| g.starts_with("IGNORE_IF"))
}

/// The integers Protovalidate lets through a field. An unset field of implicit presence is its
/// zero value, and the rules are applied to it like to any other value — which is why a field
/// with no rules at all lets 0 through.
pub fn field_ints(ty: &str, r: &Rules, optional: bool) -> Option<Ints> {
    let (tmin, tmax) = int_bounds(ty)?;
    let full = Ints::between(Some(tmin), Some(tmax));
    if r.ignore.as_deref() == Some("IGNORE_ALWAYS") {
        return Some(open_ends(full, tmin, tmax));
    }
    let i = &r.int;
    let lower = i.gt.map(|g| g + 1).or(i.gte);
    let upper = i.lt.map(|l| l - 1).or(i.lte);
    // Protovalidate reads a lower bound written above the upper one as the two ends of an
    // excluded range: `gt: 10, lt: 5` lets through what is above 10 or below 5.
    let reversed = matches!((i.gt.or(i.gte), i.lt.or(i.lte)), (Some(a), Some(b)) if a > b);
    let mut s = if reversed {
        Ints::between(None, upper).union(&Ints::between(lower, None))
    } else {
        Ints::between(lower, upper)
    }
    .intersect(&full);
    if let Some(k) = i.konst {
        s = s.intersect(&Ints::points(&[k]));
    }
    if !i.in_.is_empty() {
        s = s.intersect(&Ints::points(&i.in_));
    }
    for x in &i.not_in {
        s = s.without(*x);
    }
    // `required` rules 0 out only where the field has no presence of its own; on an `optional`
    // field it asks that the field be set, and a set 0 passes it (rulec's §15.140).
    if r.required && !optional {
        s = s.without(0);
    }
    if !optional && ignores_zero(r) {
        s = s.union(&Ints::points(&[0]));
    }
    Some(open_ends(s, tmin, tmax))
}

/// A bound that is only the kind's own limit is shown open: an `int64` with no rule reads as
/// "any integer", not as nineteen digits. The 0 an unsigned kind starts at stays, because it
/// says something.
pub fn open_ends(s: Ints, tmin: i128, tmax: i128) -> Ints {
    Ints(s.0.into_iter().map(|(a, b)| (a.filter(|&x| x != tmin || tmin == 0), b.filter(|&x| x != tmax))).collect())
}

/// The strings Protovalidate lets through a field, or None when it does not list them.
pub fn field_strs(r: &Rules, optional: bool) -> Option<Vec<String>> {
    if r.ignore.as_deref() == Some("IGNORE_ALWAYS") {
        return None;
    }
    let mut list = r.str_const.clone().map(|k| vec![k]);
    if !r.str_in.is_empty() {
        list = Some(match list {
            Some(l) => l.into_iter().filter(|x| r.str_in.contains(x)).collect(),
            None => r.str_in.clone(),
        });
    }
    let mut list = list?;
    list.retain(|x| !r.str_not_in.contains(x));
    // The zero value of a string is "", and IGNORE_IF_ZERO_VALUE lets it through unchecked.
    if !optional && ignores_zero(r) && !list.iter().any(String::is_empty) {
        list.push(String::new());
    }
    Some(list)
}

// --- The names of an enum's values, as a rule and a reader of a `.proto` call them ------------

/// `MemberTier` → `MEMBER_TIER`: an enum's name as the prefix its values carry.
///
/// The split is buf's, because `buf lint` is what asks for the prefix (`ENUM_VALUE_PREFIX`)
/// and protobuf-py splits the same way when it takes it off. A capital begins a word when the
/// letter before it is lower case or the one after it is, so `HTTPMethod` is `HTTP_METHOD` —
/// not `H_T_T_P_METHOD`, which buf refuses — and a digit begins none: `Tier2` is `TIER2`.
pub fn upper_snake(camel: &str) -> String {
    let b: Vec<char> = camel.chars().collect();
    let mut out = String::new();
    for (i, &ch) in b.iter().enumerate() {
        if ch == '_' {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            continue;
        }
        if i > 0 && ch.is_ascii_uppercase() && !out.ends_with('_') {
            let lower_before = b[i - 1].is_ascii_lowercase();
            let lower_after = b.get(i + 1).is_some_and(|n| !n.is_ascii_uppercase() && !n.is_ascii_digit() && *n != '_');
            if lower_before || lower_after {
                out.push('_');
            }
        }
        out.push(ch.to_ascii_uppercase());
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

/// A value's name with the enum's prefix taken off, when what is left is a name of its own
/// (`is_value` says whether a name is another value's whole name, which would make two values
/// one). It is not when it begins with a digit — `SIZE_60` would leave `60`, which no alias can
/// be. The name then stays as written, prefix and all, which is also what protobuf-py does with it.
pub fn value_local<'v>(enum_name: &str, value: &'v str, is_value: impl Fn(&str) -> bool) -> &'v str {
    let prefix = format!("{}_", upper_snake(enum_name));
    match value.strip_prefix(prefix.as_str()) {
        Some(rest) if rest.starts_with(|c: char| c.is_ascii_alphabetic()) && !is_value(rest) => rest,
        _ => value,
    }
}

/// The values an enum of a `.proto` means under the rules on a field (`rules`, by number; default
/// for none), each as a rule's `import proto` calls it: the prefix taken off, in lower case, without
/// the value 0 named `…_UNSPECIFIED`, which proto3 reads as not set (rulec's `Enum::named`).
pub fn enum_names(e: &crate::model::Enum, rules: &IntRules) -> Vec<String> {
    let is_value = |n: &str| e.values.iter().any(|o| o.name == n);
    let passes = |n: i128| rules.konst.is_none_or(|k| k == n) && (rules.in_.is_empty() || rules.in_.contains(&n)) && !rules.not_in.contains(&n);
    e.values
        .iter()
        .filter(|v| passes(v.number as i128))
        .filter_map(|v| {
            let local = value_local(&e.name, &v.name, is_value);
            (v.number != 0 || !local.eq_ignore_ascii_case("unspecified")).then(|| local.to_ascii_lowercase())
        })
        .collect()
}
