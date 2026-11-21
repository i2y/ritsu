//! A validator of the part of JSON Schema 2020-12 that the schemas of A2UI 1.0 write
//! (`tests/fixtures/a2ui-1.0/`), for the screens ringi writes: the workspace takes no validator
//! from outside (ritsu's DESIGN 3.1), and one that needs nothing but cargo runs at every level of
//! the tests. It reads every keyword those schemas write and stops at any other, so that a copy of
//! a later schema with a keyword more is not read as if the keyword were not there: `$ref` (into
//! the documents it is given, by their `$id`), the applicators (`allOf`, `anyOf`, `oneOf`, `not`,
//! `if`/`then`/`else`, `properties`, `patternProperties`, `additionalProperties`, `propertyNames`,
//! `items`, `contains`), the assertions (`type`, `const`, `enum`, `required`, `minItems`,
//! `minimum`, `minProperties`, `maxProperties`, `uniqueItems`, `pattern`, `format`), and
//! `unevaluatedProperties`, with the properties each applicator evaluates. `pattern` is read for
//! the three patterns the schemas write; `format` asserts `date`, `time` and `date-time` as RFC
//! 3339 writes them, as A2UI's own validation does (with no assertion, a `oneOf` of the three
//! formats would take no string at all). A `$ref` to JSON Schema's own meta-schema, where the
//! schema of a catalog says that a part or a function is described by a schema, takes what it is
//! given: the meta-schema is not kept, and the tests hold those places themselves.

use ritsu_base::json::Json;
use std::collections::{BTreeMap, BTreeSet};

/// JSON Schema 2020-12's meta-schema.
const META: &str = "https://json-schema.org/draft/2020-12/schema";

/// The keywords that describe and assert nothing: annotations of JSON Schema, and the words A2UI's
/// catalogs write beside a part's or a function's schema.
const NOTES: [&str; 14] = [
    "$schema",
    "$id",
    "$comment",
    "$defs",
    "title",
    "description",
    "default",
    "examples",
    "discriminator",
    "returnType",
    "allowedCallers",
    "requiresUserActivation",
    "allowedParents",
    "allowedChildren",
];

/// The keywords read.
const READ: [&str; 26] = [
    "$ref",
    "type",
    "const",
    "enum",
    "format",
    "pattern",
    "minimum",
    "minItems",
    "uniqueItems",
    "items",
    "contains",
    "required",
    "minProperties",
    "maxProperties",
    "propertyNames",
    "properties",
    "patternProperties",
    "additionalProperties",
    "allOf",
    "anyOf",
    "oneOf",
    "not",
    "if",
    "then",
    "else",
    "unevaluatedProperties",
];

/// The documents a schema may lead to, by their URIs.
#[derive(Default)]
pub struct Schemas {
    docs: BTreeMap<String, Json>,
}

/// What a schema says of a value: what is wrong with it, and the properties of the object it
/// evaluated (for `unevaluatedProperties`).
#[derive(Default)]
struct Said {
    errors: Vec<String>,
    seen: BTreeSet<String>,
}

impl Schemas {
    /// A document, at its `$id`.
    pub fn add(&mut self, doc: Json) {
        let id = doc.get("$id").and_then(Json::as_str).expect("a document with its $id").to_string();
        self.docs.insert(id, doc);
    }

    /// What is wrong with `x` by the schema at `uri` (a document's URI, with a fragment or none).
    pub fn errors(&self, uri: &str, x: &Json) -> Vec<String> {
        let (doc, s) = self.resolve(uri, "").unwrap_or_else(|| panic!("no schema at {uri}"));
        self.eval(s, &doc, x, "", false).errors
    }

    /// The document and the schema `r` leads to from the document at `base`; None for the
    /// meta-schema.
    fn resolve(&self, r: &str, base: &str) -> Option<(String, &Json)> {
        let (at, fragment) = r.split_once('#').unwrap_or((r, ""));
        let doc = if at.is_empty() {
            base.to_string()
        } else if at.contains("://") {
            at.to_string()
        } else {
            let dir = base.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            format!("{dir}/{at}")
        };
        if doc == META || doc == format!("{META}/") {
            return None;
        }
        let root = self.docs.get(&doc).unwrap_or_else(|| panic!("`{r}` leads to {doc}, which is not loaded"));
        let mut s = root;
        for step in fragment.split('/').filter(|p| !p.is_empty()) {
            let key = step.replace("~1", "/").replace("~0", "~");
            s = match s {
                Json::Arr(items) => key.parse::<usize>().ok().and_then(|i| items.get(i)),
                _ => s.get(&key),
            }
            .unwrap_or_else(|| panic!("`{r}` leads nowhere in {doc}"));
        }
        Some((doc, s))
    }

    /// Whether `x` holds by the schema `s` of the document at `base`, with what it evaluates: as
    /// `oneOf`, `anyOf`, `not`, `if`, `contains` and `propertyNames` ask, which say nothing of why.
    fn holds(&self, s: &Json, base: &str, x: &Json) -> Option<BTreeSet<String>> {
        let said = self.eval(s, base, x, "", true);
        said.errors.is_empty().then_some(said.seen)
    }

    /// What the schema `s` of the document at `base` says of `x`, at the place `at` of the value.
    /// `quick`: whether it stops at the first thing wrong, for an applicator that asks only whether
    /// the value holds.
    fn eval(&self, s: &Json, base: &str, x: &Json, at: &str, quick: bool) -> Said {
        let mut out = Said::default();
        let pairs = match s {
            Json::Bool(true) => return out,
            Json::Bool(false) => {
                out.errors.push(format!("{at}: nothing is taken here"));
                return out;
            }
            Json::Obj(pairs) => pairs,
            _ => panic!("a schema that is no object at {at}: {}", s.compact()),
        };
        for (k, _) in pairs {
            assert!(READ.contains(&k.as_str()) || NOTES.contains(&k.as_str()), "the schema writes `{k}`, which this validator does not read (at {at})");
        }
        macro_rules! stop {
            () => {
                if quick && !out.errors.is_empty() {
                    return out;
                }
            };
        }
        let err = |out: &mut Said, why: String| out.errors.push(format!("{at}: {why}"));
        // in place: what a part of the schema evaluates is evaluated here, when the part holds
        let take = |out: &mut Said, said: Said| {
            if said.errors.is_empty() {
                out.seen.extend(said.seen);
            } else {
                out.errors.extend(said.errors);
            }
        };
        // what asserts first, so that a value of another shape stops before what applies
        if let Some(t) = s.get("type") {
            let types: Vec<&str> = match t {
                Json::Str(one) => vec![one.as_str()],
                Json::Arr(many) => many.iter().filter_map(Json::as_str).collect(),
                _ => panic!("`type` at {at}"),
            };
            if !types.iter().any(|t| is_type(x, t)) {
                err(&mut out, format!("{} is not {}", short(x), types.join(" or ")));
            }
        }
        if let Some(c) = s.get("const")
            && !same(c, x)
        {
            err(&mut out, format!("{} is not {}", short(x), c.compact()));
        }
        if let Some(Json::Arr(values)) = s.get("enum")
            && !values.iter().any(|v| same(v, x))
        {
            err(&mut out, format!("{} is none of {}", short(x), Json::Arr(values.clone()).compact()));
        }
        if let (Some(f), Json::Str(text)) = (s.get("format").and_then(Json::as_str), x)
            && !format_holds(f, text)
        {
            err(&mut out, format!("`{text}` is no {f}"));
        }
        if let (Some(p), Json::Str(text)) = (s.get("pattern").and_then(Json::as_str), x)
            && !pattern_holds(p, text)
        {
            err(&mut out, format!("`{text}` does not match `{p}`"));
        }
        if let (Some(m), Some(n)) = (s.get("minimum"), number(x))
            && number(m).is_some_and(|m| n < m)
        {
            err(&mut out, format!("{n} is less than {}", m.compact()));
        }
        if let Json::Obj(props) = x {
            if let Some(Json::Arr(names)) = s.get("required") {
                for n in names.iter().filter_map(Json::as_str) {
                    if !props.iter().any(|(k, _)| k == n) {
                        err(&mut out, format!("no `{n}`"));
                    }
                }
            }
            if let Some(n) = s.get("minProperties").and_then(Json::as_int)
                && (props.len() as i128) < n
            {
                err(&mut out, format!("{} properties, fewer than {n}", props.len()));
            }
            if let Some(n) = s.get("maxProperties").and_then(Json::as_int)
                && (props.len() as i128) > n
            {
                err(&mut out, format!("{} properties, more than {n}", props.len()));
            }
        }
        if let Json::Arr(items) = x {
            if let Some(n) = s.get("minItems").and_then(Json::as_int)
                && (items.len() as i128) < n
            {
                err(&mut out, format!("{} items, fewer than {n}", items.len()));
            }
            if s.get("uniqueItems").and_then(Json::as_bool) == Some(true) && items.iter().enumerate().any(|(i, a)| items[..i].iter().any(|b| same(a, b))) {
                err(&mut out, "two items are the same".to_string());
            }
        }
        stop!();
        if let Some(r) = s.get("$ref").and_then(Json::as_str)
            && let Some((doc, t)) = self.resolve(r, base)
        {
            let said = self.eval(t, &doc, x, at, quick);
            take(&mut out, said);
            stop!();
        }
        if let Json::Arr(items) = x {
            if let Some(each) = s.get("items") {
                for (i, item) in items.iter().enumerate() {
                    let said = self.eval(each, base, item, &format!("{at}/{i}"), quick);
                    out.errors.extend(said.errors);
                    stop!();
                }
            }
            if let Some(c) = s.get("contains")
                && !items.iter().any(|item| self.holds(c, base, item).is_some())
            {
                err(&mut out, "no item is what `contains` asks for".to_string());
                stop!();
            }
        }
        if let Json::Obj(props) = x {
            if let Some(names) = s.get("propertyNames") {
                for (k, _) in props {
                    if self.holds(names, base, &Json::str(k)).is_none() {
                        err(&mut out, format!("`{k}` is not a name this object takes"));
                        stop!();
                    }
                }
            }
            let declared = s.get("properties").and_then(Json::as_obj).unwrap_or(&[]);
            let patterns = s.get("patternProperties").and_then(Json::as_obj).unwrap_or(&[]);
            let more = s.get("additionalProperties");
            if !declared.is_empty() || !patterns.is_empty() || more.is_some() {
                for (k, v) in props {
                    let here = || format!("{at}/{k}");
                    let mut matched = false;
                    if let Some((_, p)) = declared.iter().find(|(n, _)| n == k) {
                        matched = true;
                        out.errors.extend(self.eval(p, base, v, &here(), quick).errors);
                    }
                    for (pattern, p) in patterns {
                        if pattern_holds(pattern, k) {
                            matched = true;
                            out.errors.extend(self.eval(p, base, v, &here(), quick).errors);
                        }
                    }
                    if !matched && let Some(more) = more {
                        matched = true;
                        if matches!(more, Json::Bool(false)) {
                            err(&mut out, format!("`{k}` is not a property this object takes"));
                        } else {
                            out.errors.extend(self.eval(more, base, v, &here(), quick).errors);
                        }
                    }
                    if matched {
                        out.seen.insert(k.clone());
                    }
                    stop!();
                }
            }
        }
        if let Some(Json::Arr(all)) = s.get("allOf") {
            for p in all {
                let said = self.eval(p, base, x, at, quick);
                take(&mut out, said);
                stop!();
            }
        }
        if let Some(Json::Arr(any)) = s.get("anyOf") {
            let mut held = false;
            for p in any {
                if let Some(seen) = self.holds(p, base, x) {
                    held = true;
                    out.seen.extend(seen);
                }
            }
            if !held {
                err(&mut out, format!("none of `anyOf` holds for {}", short(x)));
                stop!();
            }
        }
        if let Some(Json::Arr(one)) = s.get("oneOf") {
            let held: Vec<BTreeSet<String>> = one.iter().filter_map(|p| self.holds(p, base, x)).collect();
            match held.len() {
                1 => out.seen.extend(held.into_iter().next().unwrap()),
                0 => err(&mut out, format!("none of `oneOf` holds for {}", short(x))),
                n => err(&mut out, format!("{n} of `oneOf` hold for {}", short(x))),
            }
            stop!();
        }
        if let Some(not) = s.get("not")
            && self.holds(not, base, x).is_some()
        {
            err(&mut out, format!("{} is what `not` refuses", short(x)));
            stop!();
        }
        if let Some(cond) = s.get("if") {
            let branch = match self.holds(cond, base, x) {
                Some(seen) => {
                    out.seen.extend(seen);
                    s.get("then")
                }
                None => s.get("else"),
            };
            if let Some(b) = branch {
                let said = self.eval(b, base, x, at, quick);
                take(&mut out, said);
                stop!();
            }
        }
        if let (Some(rest), Json::Obj(props)) = (s.get("unevaluatedProperties"), x) {
            for (k, v) in props.iter().filter(|(k, _)| !out.seen.contains(k)) {
                if matches!(rest, Json::Bool(false)) {
                    out.errors.push(format!("{at}: `{k}` is not a property this object takes"));
                } else {
                    let said = self.eval(rest, base, v, &format!("{at}/{k}"), quick);
                    out.errors.extend(said.errors);
                }
                if quick && !out.errors.is_empty() {
                    break;
                }
            }
            out.seen.extend(props.iter().map(|(k, _)| k.clone()));
        }
        out
    }
}

/// A value, short enough for a message.
fn short(x: &Json) -> String {
    let c = x.compact();
    if c.chars().count() > 120 { format!("{}…", c.chars().take(120).collect::<String>()) } else { c }
}

fn number(x: &Json) -> Option<f64> {
    match x {
        Json::Int(n) => Some(*n as f64),
        Json::Frac(digits) => digits.parse().ok(),
        _ => None,
    }
}

fn is_type(x: &Json, t: &str) -> bool {
    match t {
        "object" => matches!(x, Json::Obj(_)),
        "array" => matches!(x, Json::Arr(_)),
        "string" => matches!(x, Json::Str(_)),
        "boolean" => matches!(x, Json::Bool(_)),
        "null" => matches!(x, Json::Null),
        "number" => number(x).is_some(),
        "integer" => matches!(x, Json::Int(_)) || number(x).is_some_and(|n| n.fract() == 0.0),
        _ => panic!("the type `{t}`"),
    }
}

/// Whether two values are the same value, as JSON Schema compares them: an object's keys in any order.
fn same(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Obj(x), Json::Obj(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.iter().any(|(l, w)| k == l && same(v, w))),
        (Json::Arr(x), Json::Arr(y)) => x.len() == y.len() && x.iter().zip(y).all(|(v, w)| same(v, w)),
        _ if number(a).is_some() && number(b).is_some() => number(a) == number(b),
        _ => a == b,
    }
}

/// Whether `text` matches the pattern `p`: one of the three the schemas write.
fn pattern_holds(p: &str, text: &str) -> bool {
    match p {
        // a name that starts with one `@`, which A2UI keeps for its own properties
        "^@([^@]|$)" => text.starts_with('@') && !text[1..].starts_with('@'),
        // a Unicode identifier (UAX #31), read with ritsu-emit's tables of names: a little narrower
        // than XID_Continue after the first character (no combining mark), where no key of a screen is
        r"^[\p{XID_Start}_][\p{XID_Continue}]*$" => ritsu_emit::ident::rust_takes(text),
        // a version as Semantic Versioning writes it, with its patch number left out or not
        r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:\.(0|[1-9][0-9]*))?(?:-((?:0|[1-9][0-9]*|[0-9]*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9][0-9]*|[0-9]*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$" => version(text),
        _ => panic!("the pattern `{p}`, which this validator does not read"),
    }
}

/// Whether `text` is a version: `1.0`, `1.0.2`, with a pre-release and a build after it or not.
fn version(text: &str) -> bool {
    let number = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && (s == "0" || !s.starts_with('0'));
    let ident = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    let (rest, build) = text.split_once('+').map_or((text, None), |(a, b)| (a, Some(b)));
    if build.is_some_and(|b| !b.split('.').all(ident)) {
        return false;
    }
    let (core, pre) = rest.split_once('-').map_or((rest, None), |(a, b)| (a, Some(b)));
    if pre.is_some_and(|p| !p.split('.').all(|i| ident(i) && (!i.bytes().all(|b| b.is_ascii_digit()) || number(i)))) {
        return false;
    }
    let parts: Vec<&str> = core.split('.').collect();
    (2..=3).contains(&parts.len()) && parts.iter().all(|p| number(p))
}

/// Whether `text` is of the format `f`, as RFC 3339 writes a day (`full-date`), a time of day with
/// its offset (`full-time`) and the two with `T` between.
fn format_holds(f: &str, text: &str) -> bool {
    match f {
        "date" => date(text),
        "time" => time(text),
        "date-time" => text.split_once(['T', 't']).is_some_and(|(d, t)| date(d) && time(t)),
        _ => panic!("the format `{f}`, which this validator does not read"),
    }
}

fn digits(s: &str, n: usize) -> Option<u32> {
    (s.len() == n && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().unwrap())
}

fn date(s: &str) -> bool {
    let p: Vec<&str> = s.split('-').collect();
    let [y, m, d] = p[..] else { return false };
    let (Some(y), Some(m), Some(d)) = (digits(y, 4), digits(m, 2), digits(d, 2)) else { return false };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let last = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    (1..=12).contains(&m) && d >= 1 && d <= last[(m - 1) as usize]
}

fn time(s: &str) -> bool {
    let (clock, offset) = if let Some(c) = s.strip_suffix(['Z', 'z']) {
        (c, None)
    } else {
        let Some(i) = s.rfind(['+', '-']) else { return false };
        (&s[..i], Some(&s[i + 1..]))
    };
    let hms = |c: &str| -> bool {
        let (whole, frac) = c.split_once('.').map_or((c, None), |(a, b)| (a, Some(b)));
        let p: Vec<&str> = whole.split(':').collect();
        let [h, m, sec] = p[..] else { return false };
        matches!((digits(h, 2), digits(m, 2), digits(sec, 2)), (Some(h), Some(m), Some(sec)) if h < 24 && m < 60 && sec <= 60) && frac.is_none_or(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()))
    };
    let off = |o: &str| -> bool {
        let p: Vec<&str> = o.split(':').collect();
        matches!(p[..], [h, m] if digits(h, 2).is_some_and(|h| h < 24) && digits(m, 2).is_some_and(|m| m < 60))
    };
    hms(clock) && offset.is_none_or(off)
}

#[test]
fn the_validator_reads_the_formats_and_patterns_as_rfc_3339_and_the_schemas_write_them() {
    assert!(date("2026-02-28") && date("2024-02-29") && !date("2026-02-29") && !date("2026-13-01") && !date("26-01-01"));
    assert!(time("12:30:00Z") && time("23:59:60+09:00") && time("08:00:00.250-05:00") && !time("12:30") && !time("12:30:00"));
    assert!(format_holds("date-time", "2026-11-16T09:00:00+09:00") && !format_holds("date-time", "2026-11-16"));
    assert!(version("1.0") && version("1.0.2") && version("1.0.0-rc.1+build.5") && !version("01.0") && !version("1") && !version("1.0.0-01"));
    assert!(pattern_holds("^@([^@]|$)", "@path") && pattern_holds("^@([^@]|$)", "@") && !pattern_holds("^@([^@]|$)", "@@x") && !pattern_holds("^@([^@]|$)", "x@"));
}
