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

/// The values an integer kind can hold on the wire. A scalar's name is never written with a
/// package: `.shop.v1.int64` would be a message.
pub fn int_bounds(ty: &str) -> Option<(i128, i128)> {
    match ty {
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

/// How the values of one enum of a `.proto` read: the names a rule's `import proto`, a gate's
/// `use proto` and the check across the languages hold a contract's values to. The three read
/// with this one reading (rulec's DESIGN §15.59 has the table of the forms a name can take).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EnumNames {
    /// Each number the enum gives a name, once, in the order the file first gives it, without
    /// the one proto3 reads as not set.
    pub values: Vec<ValueNames>,
    /// The value proto3 reads as not set, when the file has one: the number 0, read `unspecified`.
    /// A field left out of a message arrives as it.
    pub unset: Option<ValueNames>,
}

/// One value of an enum: its number, and each name the file gives the number, the first one
/// first. Under `allow_alias` a number has more than one name, and it is still one value: a field
/// holds the number, and protojson writes it under its first name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueNames {
    pub number: i64,
    pub names: Vec<Name>,
}

/// One name of a value, as the file writes it and as it is read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    /// `CATEGORY_TRAVEL`.
    pub written: String,
    /// `travel`.
    pub read: String,
    pub how: How,
}

/// How a name came to be read as it is, which is what a reader says when the reading is not the
/// one the writer expected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum How {
    /// Without the enum's name before it (`CATEGORY_TRAVEL`, `Category_Travel`).
    Dropped,
    /// Whole: it does not begin with the enum's name (`TRAVEL`).
    Whole,
    /// Whole: the enum's name runs on into a word of it (`STATUSBAR_HIDDEN` of `Status`).
    RunsOn,
    /// Whole: what follows the enum's name begins with a digit (`SIZE_60`).
    Digit,
    /// Whole, though it begins with the enum's name: without it, it would be read as this name (in
    /// lower case), which is a name of another number (`COLOR_RED` beside `RED`).
    Kept(String),
    /// As written: its whole name in lower case is a name of another number too (`COLOR_RED` beside
    /// `Color_Red`).
    AsWritten,
}

impl ValueNames {
    /// The value's first name, read: what a reader says the value by.
    pub fn read(&self) -> &str {
        &self.names[0].read
    }

    /// The value's first name as the file writes it: what protojson writes the value as.
    pub fn written(&self) -> &str {
        &self.names[0].written
    }

    /// Whether `name` is one of the value's names, read.
    pub fn is(&self, name: &str) -> bool {
        self.names.iter().any(|n| n.read == name)
    }
}

/// How the values of the enum `enum_name` read, from each value's name and number in the order
/// the file writes them. The enum is named as the file names it, nested in a message or not
/// (`PayRequest.Category`); what its values carry is its own name (`Category`).
///
/// - A name that begins with the enum's own name drops it where the enum's name ends a word of
///   it: a `_` follows (`CATEGORY_TRAVEL`), or a capital that begins a word (`CategoryTravel`).
///   The case of the letters and where the `_`s fall do not matter (`Category_Travel`,
///   `category_travel`, and `MEMBERTIER_GOLD` of `MemberTier`). A name the enum's name runs on
///   into (`STATUSBAR_HIDDEN` of `Status`) keeps it, and so does one whose rest would begin with
///   a digit (`SIZE_60`).
/// - What is left, or the whole name, is read in lower case.
/// - A name that would be read the same as a name of another number keeps its prefix
///   (`COLOR_RED` beside `RED` is `color_red`), and two whole names that are still read the same —
///   they differ only in case — are read as written. Two numbers are never read as one name.
/// - The number 0 read `unspecified` is proto3's "not set" (`CATEGORY_UNSPECIFIED`,
///   `Category_Unspecified`, `UNSPECIFIED`). A 0 read as anything else (`UNKNOWN`) is a value.
pub fn names<'a>(enum_name: &str, values: impl IntoIterator<Item = (&'a str, i64)>) -> EnumNames {
    let own = enum_name.rsplit('.').next().unwrap_or(enum_name);
    let letters: Vec<u8> = own.bytes().filter(|b| *b != b'_').map(|b| b.to_ascii_lowercase()).collect();
    let vals: Vec<(&str, i64)> = values.into_iter().collect();
    let split: Vec<Split> = vals.iter().map(|(n, _)| split(&letters, n)).collect();
    let rest: Vec<Option<&str>> = split.iter().map(|s| if let Split::Rest(r) = s { Some(*r) } else { None }).collect();
    // a name read at a level: 0 without the prefix, 1 whole, 2 as written
    let read = |i: usize, level: u8| match (level, rest[i]) {
        (0, Some(r)) => r.to_ascii_lowercase(),
        (0 | 1, _) => vals[i].0.to_ascii_lowercase(),
        _ => vals[i].0.to_string(),
    };
    let zero_unset = (0..vals.len()).any(|i| vals[i].1 == 0 && read(i, 0) == "unspecified");
    let unset = |i: usize| zero_unset && vals[i].1 == 0;
    // a name read the same as one of another number goes a level up, the lower of the two (both,
    // at the same level), until no two numbers share a name
    let mut level: Vec<u8> = rest.iter().map(|r| if r.is_some() { 0 } else { 1 }).collect();
    loop {
        let up: Vec<usize> = (0..vals.len())
            .filter(|&i| level[i] < 2 && !unset(i))
            .filter(|&i| (0..vals.len()).any(|j| vals[j].1 != vals[i].1 && !unset(j) && level[i] <= level[j] && read(i, level[i]) == read(j, level[j])))
            .collect();
        if up.is_empty() {
            break;
        }
        for i in up {
            level[i] += 1;
        }
    }
    let mut out = EnumNames::default();
    for (i, &(written, number)) in vals.iter().enumerate() {
        let how = match (level[i], &split[i]) {
            (0, _) => How::Dropped,
            (1, Split::Rest(_)) => How::Kept(read(i, 0)),
            (1, Split::NotThere) => How::Whole,
            (1, Split::RunsOn) => How::RunsOn,
            (1, Split::Digit) => How::Digit,
            _ => How::AsWritten,
        };
        let name = Name { written: written.to_string(), read: read(i, level[i]), how };
        let slot = if unset(i) {
            out.unset.get_or_insert_with(|| ValueNames { number, names: Vec::new() })
        } else {
            let k = match out.values.iter().position(|v| v.number == number) {
                Some(k) => k,
                None => {
                    out.values.push(ValueNames { number, names: Vec::new() });
                    out.values.len() - 1
                }
            };
            &mut out.values[k]
        };
        if !slot.names.iter().any(|n| n.read == name.read) {
            slot.names.push(name);
        }
    }
    out
}

/// How a value's name begins, against the enum's own name.
enum Split<'n> {
    /// With the enum's name and a word break: what is left, which begins with a letter.
    Rest(&'n str),
    /// Not with the enum's name.
    NotThere,
    /// With the enum's name running on into a word.
    RunsOn,
    /// With the enum's name and a word break, and then a digit.
    Digit,
}

/// How `name` begins against the enum's own name (`letters`: its letters in lower case, without
/// its `_`s), whatever the case of its letters and wherever its `_`s fall.
fn split<'n>(letters: &[u8], name: &'n str) -> Split<'n> {
    let b = name.as_bytes();
    if letters.is_empty() || !name.is_ascii() {
        return Split::NotThere;
    }
    let (mut i, mut j) = (0, 0);
    while j < letters.len() {
        match b.get(i) {
            Some(b'_') => i += 1,
            Some(c) if c.to_ascii_lowercase() == letters[j] => {
                i += 1;
                j += 1;
            }
            _ => return Split::NotThere,
        }
    }
    // a word ends here: a `_` follows, or a capital that begins a word — after a small letter, or
    // before one (`MemberTierGold`, `MEMBERTIERGold`)
    let Some(&next) = b.get(i) else { return Split::NotThere };
    let begins = next.is_ascii_uppercase() && (b[i - 1].is_ascii_lowercase() || b.get(i + 1).is_some_and(|c| c.is_ascii_lowercase()));
    if next != b'_' && !begins {
        return Split::RunsOn;
    }
    while b.get(i) == Some(&b'_') {
        i += 1;
    }
    let left = &name[i..];
    if left.starts_with(|c: char| c.is_ascii_alphabetic()) {
        Split::Rest(left)
    } else if left.is_empty() {
        Split::NotThere
    } else {
        Split::Digit
    }
}

/// The values an enum of a `.proto` means under the rules on a field (`rules`, by number; default
/// for none), read as [`names`] reads them, without the one proto3 reads as not set.
pub fn enum_names(e: &crate::model::Enum, rules: &IntRules) -> Vec<ValueNames> {
    let passes = |n: i128| rules.konst.is_none_or(|k| k == n) && (rules.in_.is_empty() || rules.in_.contains(&n)) && !rules.not_in.contains(&n);
    names(&e.name, e.values.iter().map(|v| (v.name.as_str(), v.number))).values.into_iter().filter(|v| passes(v.number as i128)).collect()
}
