//! What a call looks like on the wire of each target, with concrete values. The reference
//! interpreter renders its calls with these functions, and the runners that execute the
//! generated ASL and the generated TypeScript report their calls in the same shape, so
//! the three can be compared line for line. The generators build the same shapes as
//! JSONata and TypeScript expressions; if they drift apart, the comparison says so.

use crate::model::*;
use serde_json::{json, Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Asl,
    Temporal,
    /// Lambda durable functions: a task is a step, a callback task a submit, a rule an invoke
    Durable,
}

/// The idempotency key of one call: the execution, the call's place in the source, and
/// the round of every loop around it, so that a retry repeats the key and a new round does not.
pub fn key(execution: &str, site: usize, rounds: &[u64]) -> String {
    let mut k = format!("{execution}/{site}");
    for r in rounds {
        k.push_str(&format!("/{r}"));
    }
    k
}

pub fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// A call as the target sees it. `args` are by parameter name.
pub fn call(m: &Model, view: View, callee: &Callee, args: &Map<String, Value>, key: Option<&str>) -> Value {
    match (view, callee) {
        (View::Temporal, Callee::Task(t)) => {
            let mut a = args.clone();
            if let Some(k) = key {
                a.insert("idempotency_key".into(), json!(k));
            }
            json!({ "activity": m.tasks[*t].name, "args": a })
        }
        (View::Temporal, Callee::Rule(r)) => json!({ "activity": rule_activity(&m.rules[*r].name), "args": args }),
        (View::Durable, Callee::Task(t)) => {
            let mut a = args.clone();
            if let Some(k) = key {
                a.insert("idempotency_key".into(), json!(k));
            }
            let kind = if m.tasks[*t].callback { "callback" } else { "task" };
            json!({ kind: m.tasks[*t].name, "args": a })
        }
        (View::Durable, Callee::Rule(r)) => json!({ "invoke": m.rules[*r].lambda.clone().unwrap_or_default(), "payload": args }),
        (View::Asl, Callee::Rule(r)) => json!({ "lambda": m.rules[*r].lambda.clone().unwrap_or_default(), "payload": args }),
        (View::Asl, Callee::Task(t)) => {
            let task = &m.tasks[*t];
            match &task.binding {
                Some(Binding::Lambda(f)) => {
                    let mut p = args.clone();
                    if let Some(k) = key {
                        p.insert("idempotency_key".into(), json!(k));
                    }
                    json!({ "lambda": f, "payload": p })
                }
                Some(Binding::Http { method, url, form }) => {
                    let (url, rest) = fill_url(url, args);
                    let mut headers = Map::new();
                    if *form {
                        headers.insert("Content-Type".into(), json!("application/x-www-form-urlencoded"));
                    }
                    if let Some(k) = key {
                        headers.insert("Idempotency-Key".into(), json!(k));
                    }
                    let mut w = Map::new();
                    w.insert("http".into(), json!(method));
                    w.insert("url".into(), json!(url));
                    if !headers.is_empty() {
                        w.insert("headers".into(), Value::Object(headers));
                    }
                    if !rest.is_empty() {
                        let place = if method == "GET" || method == "DELETE" { "query" } else { "body" };
                        w.insert(place.into(), Value::Object(rest));
                    }
                    Value::Object(w)
                }
                None => json!({ "task": task.name, "args": args }),
            }
        }
    }
}

/// `https://…/{id}/confirm` with `id` put in; the arguments the URL did not take.
pub fn fill_url(url: &str, args: &Map<String, Value>) -> (String, Map<String, Value>) {
    let used = crate::lower::placeholders(url);
    let mut out = url.to_string();
    for p in &used {
        let v = args.get(p).map(value_text).unwrap_or_default();
        out = out.replace(&format!("{{{p}}}"), &v);
    }
    let rest = args.iter().filter(|(k, _)| !used.contains(k)).map(|(k, v)| (k.clone(), v.clone())).collect();
    (out, rest)
}

pub fn rule_activity(rule: &str) -> String {
    format!("rule_{}", ident(rule))
}

/// A name the generated code can use as an identifier. JavaScript and Step Functions both
/// take Unicode identifiers, so Japanese names stay as they are; what cannot start one is
/// prefixed.
pub fn ident(name: &str) -> String {
    let mut s: String = name.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if s.is_empty() || s.chars().next().map(|c| c.is_ascii_digit() || c == '_').unwrap_or(true) {
        s = format!("v{s}");
    }
    s
}

/// A variable of the state machine. Step Functions wants a Unicode identifier that does
/// not start with `_`, and `states` is its own.
pub fn asl_var(name: &str) -> String {
    let s = ident(name);
    if s == "states" {
        "v_states".into()
    } else {
        s
    }
}

/// A field in a JSONata path: plain when it is a plain ASCII name, in backquotes otherwise.
pub fn jsonata_field(name: &str) -> String {
    let plain = !name.is_empty()
        && name.chars().next().map(|c| c.is_ascii_alphabetic() || c == '_').unwrap_or(false)
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !["and", "or", "in", "true", "false", "null", "function"].contains(&name);
    if plain {
        name.to_string()
    } else {
        format!("`{}`", name.replace('`', "\\`"))
    }
}

pub fn jsonata_string(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

/// `$pi.status`
pub fn jsonata_path(var: &str, fields: &[String]) -> String {
    let mut s = format!("${}", asl_var(var));
    for f in fields {
        s.push('.');
        s.push_str(&jsonata_field(f));
    }
    s
}

pub fn jsonata_expr(e: &TExpr) -> String {
    match e {
        TExpr::Var { name, fields, .. } => jsonata_path(name, fields),
        TExpr::Str(s) => jsonata_string(s),
        TExpr::Int(n) => n.to_string(),
        TExpr::Bool(b) => b.to_string(),
        TExpr::Enum(v, _) => jsonata_string(v),
    }
}

/// A JSONata test that `x` is a well-formed value of type `t`.
pub fn jsonata_check(m: &Model, x: &str, t: &Ty, depth: usize) -> String {
    match t {
        Ty::Str => format!("$type({x}) = \"string\""),
        Ty::Bool => format!("$type({x}) = \"boolean\""),
        Ty::Timestamp => format!("($type({x}) = \"string\" and $contains({x}, /{TIMESTAMP_RE}/))"),
        Ty::Int | Ty::Num(_) => format!("($type({x}) = \"number\" and {x} = $floor({x}))"),
        Ty::Enum(e) => format!("{x} in {}", jsonata_list(&m.enums[*e].values)),
        Ty::Record(r) => {
            let mut parts = vec![format!("$type({x}) = \"object\"")];
            if depth < 4 {
                for (f, ft) in &m.records[*r].fields {
                    parts.push(jsonata_check(m, &format!("{x}.{}", jsonata_field(f)), ft, depth + 1));
                }
            }
            format!("({})", parts.join(" and "))
        }
    }
}

/// What Step Functions' Wait takes: RFC 3339 with an uppercase T, in UTC with an uppercase Z.
pub const TIMESTAMP_RE: &str = "^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\\.[0-9]+)?Z$";

pub fn is_timestamp(s: &str) -> bool {
    let b = s.as_bytes();
    let digits = |r: std::ops::Range<usize>| r.clone().all(|i| b.get(i).map(|c| c.is_ascii_digit()).unwrap_or(false));
    if b.len() < 20 || !digits(0..4) || b[4] != b'-' || !digits(5..7) || b[7] != b'-' || !digits(8..10) || b[10] != b'T' || !digits(11..13) || b[13] != b':' || !digits(14..16) || b[16] != b':' || !digits(17..19) {
        return false;
    }
    let rest = &s[19..];
    match rest.strip_suffix('Z') {
        Some("") => true,
        Some(frac) => frac.len() > 1 && frac.starts_with('.') && frac[1..].bytes().all(|c| c.is_ascii_digit()),
        None => false,
    }
}

pub fn jsonata_list(values: &[String]) -> String {
    format!("[{}]", values.iter().map(|v| jsonata_string(v)).collect::<Vec<_>>().join(", "))
}

/// Whether a JSON value is a well-formed value of type `t` (the interpreter's side of `jsonata_check`).
pub fn value_fits(m: &Model, v: &Value, t: &Ty) -> bool {
    match t {
        Ty::Str => v.is_string(),
        Ty::Bool => v.is_boolean(),
        Ty::Timestamp => v.as_str().map(is_timestamp).unwrap_or(false),
        Ty::Int | Ty::Num(_) => v.as_f64().map(|f| f.fract() == 0.0).unwrap_or(false),
        Ty::Enum(e) => v.as_str().map(|s| m.enums[*e].values.iter().any(|x| x == s)).unwrap_or(false),
        Ty::Record(r) => match v.as_object() {
            Some(o) => m.records[*r].fields.iter().all(|(f, ft)| o.get(f).map(|x| value_fits(m, x, ft)).unwrap_or(false)),
            None => false,
        },
    }
}

/// The errors a call can end with, as the ASL names them.
pub fn asl_error(m: &Model, callee: &Callee, e: &HErr) -> Vec<String> {
    match e {
        HErr::Timeout => vec!["States.Timeout".into()],
        HErr::Failure => vec!["States.ALL".into()],
        HErr::Declared(n) => match callee {
            Callee::Task(t) => {
                let task = &m.tasks[*t];
                match (&task.binding, task.errors.iter().find(|(x, _)| x == n)) {
                    (Some(Binding::Http { .. }), Some((_, Some(st)))) => vec![format!("States.Http.StatusCode.{st}")],
                    _ => vec![n.clone()],
                }
            }
            Callee::Rule(_) => vec![n.clone()],
        },
    }
}
