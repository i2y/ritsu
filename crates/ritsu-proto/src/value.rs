//! An option's value in protobuf's text format, kept as written (`1`, `-5`, `0x10`, `"a"`,
//! `IGNORE_ALWAYS`, `[1, 2]`, `{gte: 1, lte: 5}`), and the tree a file's options make (dandori's
//! way of reading them): `(a).b.c = v` is `{"a": {"b": {"c": v}}}`.

use crate::model::Opt;
use ritsu_base::json::Json;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A number as written: `1`, `-5`, `0x10`, `1.5`.
    Num(String),
    /// A string, its escapes read; strings written side by side are one.
    Str(String),
    /// A name: `true`, `IGNORE_IF_ZERO_VALUE`, `NO_SIDE_EFFECTS`.
    Id(String),
    List(Vec<Value>),
    /// `{a: 1, b {…}}` (or `<…>`): the fields in the order written, a field written twice
    /// there twice. An extension's key keeps its brackets: `[buf.validate.ext]`.
    Msg(Vec<(String, Value)>),
}

impl Value {
    /// The value as JSON, as dandori reads an option: a whole number as an integer (decimal,
    /// `0x…`), `true` and `false` (and `True`, `False`) as booleans, a name as a string, a field
    /// written again as a list of its values, an extension's key without its brackets. A
    /// fraction keeps the digits it was written with.
    pub fn to_json(&self) -> Json {
        match self {
            Value::Num(n) => number(n),
            Value::Str(s) => Json::str(s),
            Value::Id(w) => match w.as_str() {
                "true" | "True" => Json::Bool(true),
                "false" | "False" => Json::Bool(false),
                _ => Json::str(w),
            },
            Value::List(vs) => Json::arr(vs.iter().map(Value::to_json)),
            Value::Msg(kv) => {
                let mut out: Vec<(String, Json)> = Vec::new();
                for (k, v) in kv {
                    let k = match k.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                        Some(inner) => inner.chars().filter(|c| !c.is_whitespace()).collect(),
                        None => k.clone(),
                    };
                    let v = v.to_json();
                    match out.iter_mut().find(|(x, _)| *x == k) {
                        None => out.push((k, v)),
                        Some((_, Json::Arr(a))) => match v {
                            Json::Arr(more) => a.extend(more),
                            one => a.push(one),
                        },
                        Some((_, old)) => {
                            let mut a = vec![std::mem::replace(old, Json::Null)];
                            match v {
                                Json::Arr(more) => a.extend(more),
                                one => a.push(one),
                            }
                            *old = Json::Arr(a);
                        }
                    }
                }
                Json::Obj(out)
            }
        }
    }

    /// The strings of a value that is a string or a list of them: `fails: "A"` and
    /// `fails: ["A", "B"]` alike.
    pub fn strings(&self) -> Vec<String> {
        match self {
            Value::Str(s) => vec![s.clone()],
            Value::List(vs) => vs.iter().filter_map(|v| if let Value::Str(s) = v { Some(s.clone()) } else { None }).collect(),
            _ => Vec::new(),
        }
    }
}

/// A number as JSON: a whole number (decimal or `0x…`, a `+` before it dropped) as an
/// integer, anything else as written.
fn number(text: &str) -> Json {
    let t = text.strip_prefix('+').unwrap_or(text);
    if let Ok(n) = t.parse::<i128>()
        && (i64::MIN as i128..=u64::MAX as i128).contains(&n)
    {
        return Json::Int(n);
    }
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X"))
        && let Ok(n) = i64::from_str_radix(h, 16)
    {
        return Json::Int(n as i128);
    }
    if t.parse::<f64>().is_ok_and(f64::is_finite) {
        return Json::Frac(t.to_string());
    }
    Json::str(text)
}

/// The options as a tree keyed by the name of the option or of the extension that sets it
/// (`json_name`, `buf.validate.field`, `dandori.v1.start`), in the order first written:
/// `(a).b.c = v` puts `v` at `{"a": {"b": {"c": v}}}`, laid over what the other options of the
/// same extension wrote. An option whose name is not a word or `(<extension>)` followed by
/// `.<field>…`, or whose value does not read, is left out.
pub fn tree(opts: &[Opt]) -> Json {
    let mut tree = Json::Obj(Vec::new());
    for o in opts {
        let (Some((name, path)), Some(v)) = (split_name(&o.name), &o.value) else { continue };
        let Json::Obj(top) = &mut tree else { unreachable!() };
        let slot = match top.iter().position(|(k, _)| *k == name) {
            Some(i) => &mut top[i].1,
            None => {
                top.push((name, Json::Null));
                &mut top.last_mut().unwrap().1
            }
        };
        put(slot, &path, v.to_json());
    }
    tree
}

/// `(buf.validate.field).int32.gte` → (`buf.validate.field`, [`int32`, `gte`]); `json_name` →
/// (`json_name`, []); `(.dandori.v1.start)` names the extension from the root, the same one.
fn split_name(name: &str) -> Option<(String, Vec<String>)> {
    let n: String = name.chars().filter(|c| !c.is_whitespace()).collect();
    let word = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
    if let Some(rest) = n.strip_prefix('(') {
        let (ext, after) = rest.split_once(')')?;
        let ext = ext.trim_start_matches('.');
        if !word(ext) {
            return None;
        }
        if after.is_empty() {
            return Some((ext.to_string(), Vec::new()));
        }
        let path = after.strip_prefix('.')?;
        if !word(path) || path.starts_with('.') {
            return None;
        }
        return Some((ext.to_string(), path.split('.').map(String::from).collect()));
    }
    if word(&n) && !n.starts_with('.') { Some((n, Vec::new())) } else { None }
}

fn put(slot: &mut Json, path: &[String], v: Json) {
    match path.split_first() {
        None => lay(slot, v),
        Some((k, rest)) => {
            if !matches!(slot, Json::Obj(_)) {
                *slot = Json::Obj(Vec::new());
            }
            let Json::Obj(kv) = slot else { unreachable!() };
            let child = match kv.iter().position(|(x, _)| x == k) {
                Some(i) => &mut kv[i].1,
                None => {
                    kv.push((k.clone(), Json::Null));
                    &mut kv.last_mut().unwrap().1
                }
            };
            put(child, rest, v);
        }
    }
}

fn lay(slot: &mut Json, v: Json) {
    match (slot, v) {
        (Json::Obj(a), Json::Obj(b)) => {
            for (k, x) in b {
                match a.iter_mut().find(|(y, _)| *y == k) {
                    Some((_, s)) => lay(s, x),
                    None => a.push((k, x)),
                }
            }
        }
        (slot, v) => *slot = v,
    }
}
