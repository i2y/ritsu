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
