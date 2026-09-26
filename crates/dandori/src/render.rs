//! What a call looks like on the wire of each target, with concrete values. The reference
//! interpreter renders its calls with these functions, and the runners that execute the
//! generated ASL, the generated TypeScript and the generated WorkflowTemplate report their
//! calls in the same shape, so they can be compared line for line. The generators build the
//! same shapes as JSONata, TypeScript and expr-lang expressions; if they drift apart, the
//! comparison says so.

use crate::model::*;
use serde_json::{json, Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Asl,
    Temporal,
    /// Lambda durable functions: a task is a step, a callback task a submit, a rule an invoke
    Durable,
    /// Argo Workflows: a task is a container, a rule a container of dandori's code
    Argo,
    /// pydantic-graph: a task is a function the graph calls, a rule a function around rulec's Python
    Graph,
}

impl View {
    pub fn platform(self) -> Platform {
        match self {
            View::Asl => Platform::StepFunctions,
            View::Temporal => Platform::Temporal,
            View::Durable => Platform::Durable,
            View::Argo => Platform::Argo,
            View::Graph => Platform::Graph,
        }
    }
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

/// A call as the target sees it. `args` are by parameter name. A task that the platform
/// calls by Lambda, HTTP or an AWS API looks the same on every platform, since the code
/// dandori writes for Temporal and durable functions sends what Step Functions sends.
pub fn call(m: &Model, view: View, callee: &Callee, args: &Map<String, Value>, key: Option<&str>) -> Value {
    let task = match callee {
        Callee::Rule(r) => {
            let ru = &m.rules[*r];
            return match view {
                View::Temporal => json!({ "activity": rule_activity(&ru.name), "args": args }),
                View::Durable => json!({ "invoke": ru.lambda.clone().unwrap_or_default(), "payload": args }),
                View::Asl => json!({ "lambda": ru.lambda.clone().unwrap_or_default(), "payload": args }),
                View::Argo | View::Graph => json!({ "rule": ru.name, "args": args }),
            };
        }
        Callee::Task(t) => &m.tasks[*t],
    };
    let with_key = |name: &str| {
        let mut a = args.clone();
        if let Some(k) = key {
            a.insert(name.into(), json!(k));
        }
        a
    };
    match task.via(view.platform()) {
        Some(Via::Lambda(f)) => json!({ "lambda": f, "payload": with_key("idempotency_key") }),
        Some(Via::Http { method, url, form }) => {
            let (url, rest) = fill_url(url, args);
            let mut headers = Map::new();
            if form {
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
        Some(Via::Aws { service, action }) => {
            let a = match &task.key_param {
                Some(p) => with_key(p),
                None => args.clone(),
            };
            json!({ "aws": format!("{service}:{action}"), "args": a })
        }
        Some(Via::Agent { instructions, model }) => {
            let input = agent_input(task, args);
            let schema = task.result.as_ref().and_then(|t| agent_schema(m, t)).unwrap_or(Value::Null);
            match view {
                // the HTTP Task's request; the input goes as the arguments' JSON text
                View::Asl => json!({ "http": "POST", "url": AGENT_URL, "body": agent_request(model, instructions, json!(Value::Object(input).to_string()), schema) }),
                _ => json!({ "agent": task.name, "model": model, "instructions": instructions, "input": input, "schema": schema }),
            }
        }
        Some(Via::StateMachine(arn)) => json!({ "state_machine": arn, "input": args }),
        Some(Via::Workflow(t)) => json!({ "child_workflow": t, "args": args }),
        Some(Via::DurableFunction(f)) => json!({ "invoke": f, "payload": args }),
        Some(Via::ArgoTemplate(t)) => json!({ "workflow_template": t, "args": args }),
        Some(Via::Own) | Some(Via::Image(_)) => {
            let a = with_key("idempotency_key");
            match view {
                View::Temporal => json!({ "activity": task.name, "args": a }),
                _ => {
                    let kind = if task.callback { "callback" } else { "task" };
                    json!({ kind: task.name, "args": a })
                }
            }
        }
        None => json!({ "task": task.name, "args": args }),
    }
}

/// Where Step Functions sends an agent's call: OpenAI's Responses API.
pub const AGENT_URL: &str = "https://api.openai.com/v1/responses";

/// What an agent reads: the arguments, in the order of the task's parameters. Every target
/// gives it to the model as this object's JSON text.
pub fn agent_input(task: &TaskDef, args: &Map<String, Value>) -> Map<String, Value> {
    task.params.iter().filter_map(|(p, _)| args.get(p).map(|v| (p.clone(), v.clone()))).collect()
}

/// The Responses API's request for an agent's call, as Step Functions sends it: the model,
/// what it is to do, the input as text, and the JSON Schema its answer is held to.
pub fn agent_request(model: &str, instructions: &str, input: Value, schema: Value) -> Value {
    json!({
        "model": model,
        "instructions": instructions,
        "input": input,
        "text": { "format": { "type": "json_schema", "name": "answer", "strict": true, "schema": schema } }
    })
}

/// What an agent answers in: `{"answer": …}` with a value of the task's type inside, since
/// OpenAI's Structured Outputs want an object at the top whatever the type is. None when
/// the type cannot be said as such a schema (`json_schema`).
pub fn agent_schema(m: &Model, t: &Ty) -> Option<Value> {
    Some(json!({ "type": "object", "properties": { "answer": json_schema(m, t)? }, "required": ["answer"], "additionalProperties": false }))
}

/// The JSON Schema of a value of type `t`, in the strict form of OpenAI's Structured
/// Outputs: a record's fields are all required and no others are allowed, a value that may
/// be absent is a choice with null, a timestamp has the form Step Functions' Wait takes, and
/// a number with a unit says its unit. None when `t` holds `json`, or a record that holds
/// itself through others, which such a schema cannot say.
pub fn json_schema(m: &Model, t: &Ty) -> Option<Value> {
    fn go(m: &Model, t: &Ty, within: &mut Vec<RecordId>) -> Option<Value> {
        Some(match t {
            Ty::Int => json!({ "type": "integer" }),
            Ty::Num(unit) => json!({ "type": "integer", "description": unit }),
            Ty::Str => json!({ "type": "string" }),
            Ty::Bool => json!({ "type": "boolean" }),
            Ty::Timestamp => json!({ "type": "string", "pattern": TIMESTAMP_RE }),
            Ty::Enum(e) => json!({ "type": "string", "enum": m.enums[*e].values }),
            Ty::List(x) => json!({ "type": "array", "items": go(m, x, within)? }),
            Ty::Opt(x) => json!({ "anyOf": [go(m, x, within)?, { "type": "null" }] }),
            Ty::Json => return None,
            Ty::Record(r) => {
                if within.contains(r) {
                    return None;
                }
                within.push(*r);
                let mut props = Map::new();
                for (f, ft) in &m.records[*r].fields {
                    props.insert(f.clone(), go(m, ft, within)?);
                }
                within.pop();
                let required: Vec<&String> = m.records[*r].fields.iter().map(|(f, _)| f).collect();
                json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
            }
        })
    }
    go(m, t, &mut Vec::new())
}

/// A JSON value laid out for generated code: what fits in 72 characters stays on one line,
/// the rest goes one entry a line, `unit` deeper at each level. With `py`, in Python's words
/// (`None`, `True`, `False`).
pub fn layout(v: &Value, depth: usize, unit: &str, py: bool) -> String {
    fn scalar(v: &Value, py: bool) -> String {
        match (v, py) {
            (Value::Null, true) => "None".into(),
            (Value::Bool(true), true) => "True".into(),
            (Value::Bool(false), true) => "False".into(),
            _ => v.to_string(),
        }
    }
    fn one_line(v: &Value, py: bool) -> String {
        let (open, close) = if py { ("{", "}") } else { ("{ ", " }") };
        match v {
            Value::Array(a) => format!("[{}]", a.iter().map(|x| one_line(x, py)).collect::<Vec<_>>().join(", ")),
            Value::Object(o) if o.is_empty() => "{}".into(),
            Value::Object(o) => format!("{open}{}{close}", o.iter().map(|(k, x)| format!("{}: {}", Value::String(k.clone()), one_line(x, py))).collect::<Vec<_>>().join(", ")),
            _ => scalar(v, py),
        }
    }
    let flat = one_line(v, py);
    if flat.chars().count() <= 72 || !(v.is_array() || v.is_object()) {
        return flat;
    }
    let pad = unit.repeat(depth + 1);
    let end = unit.repeat(depth);
    match v {
        Value::Array(a) => format!("[\n{}{end}]", a.iter().map(|x| format!("{pad}{},\n", layout(x, depth + 1, unit, py))).collect::<String>()),
        Value::Object(o) => format!("{{\n{}{end}}}", o.iter().map(|(k, x)| format!("{pad}{}: {},\n", Value::String(k.clone()), layout(x, depth + 1, unit, py))).collect::<String>()),
        _ => flat,
    }
}

/// How deep the objects of a JSON Schema nest, how many properties it has, and how many enum
/// values: OpenAI's Structured Outputs take at most 10, 5,000 and 1,000.
pub fn schema_size(s: &Value) -> (usize, usize, usize) {
    let mut depth = 0;
    let (mut props, mut values) = (0, 0);
    if let Some(e) = s.get("enum").and_then(|e| e.as_array()) {
        values += e.len();
    }
    let mut inner: Vec<&Value> = Vec::new();
    if let Some(p) = s.get("properties").and_then(|p| p.as_object()) {
        props += p.len();
        inner.extend(p.values());
    }
    inner.extend(s.get("items"));
    if let Some(a) = s.get("anyOf").and_then(|a| a.as_array()) {
        inner.extend(a.iter());
    }
    for x in inner {
        let (d, p, v) = schema_size(x);
        depth = depth.max(d);
        props += p;
        values += v;
    }
    if s.get("type").and_then(|t| t.as_str()) == Some("object") {
        depth += 1;
    }
    (depth, props, values)
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

/// The value of an expression, in JSONata. A field that may be absent reads as null, so
/// that no expression the state machine evaluates comes out undefined.
pub fn jsonata_expr(e: &TExpr) -> String {
    match e {
        TExpr::Var { name, fields, ty } => {
            let p = jsonata_path(name, fields);
            if !fields.is_empty() && matches!(ty, Ty::Opt(_)) {
                format!("($exists({p}) ? {p} : null)")
            } else {
                p
            }
        }
        TExpr::Str(s) => jsonata_string(s),
        TExpr::Int(n) => n.to_string(),
        TExpr::Bool(b) => b.to_string(),
        TExpr::Enum(v, _) => jsonata_string(v),
        TExpr::None(_) => "null".into(),
        TExpr::Record { fields, .. } => {
            format!("{{{}}}", fields.iter().map(|(f, x)| format!("{}: {}", jsonata_string(f), jsonata_expr(x))).collect::<Vec<_>>().join(", "))
        }
        TExpr::List { items, .. } => {
            if items.iter().any(|x| x.ty().inner() == &Ty::Json) {
                // a `json` value may be an array itself, which `[…]` would flatten into the list
                let mut acc = "[]".to_string();
                for x in items {
                    acc = format!("$append({acc}, {})", jsonata_one(&jsonata_expr(x), &x.ty()));
                }
                acc
            } else {
                format!("[{}]", items.iter().map(jsonata_expr).collect::<Vec<_>>().join(", "))
            }
        }
        TExpr::Interp(parts) => {
            let mut out: Vec<String> = Vec::new();
            for p in parts {
                match p {
                    IPart::Lit(s) => out.push(jsonata_string(s)),
                    IPart::Hole(x) => match x.ty() {
                        Ty::Str | Ty::Timestamp | Ty::Enum(_) => out.push(jsonata_expr(x)),
                        _ => out.push(format!("$string({})", jsonata_expr(x))),
                    },
                }
            }
            if out.len() == 1 && !matches!(parts[0], IPart::Lit(_)) {
                out.insert(0, "\"\"".into());
            }
            out.join(" & ")
        }
    }
}

/// A list of one item, `x`, which `$append` adds to a list as one item even when it is an
/// array itself (only a `json` value can be).
pub fn jsonata_one(x: &str, t: &Ty) -> String {
    if t.inner() == &Ty::Json {
        format!("($type({x}) = \"array\" ? [[{x}]] : [{x}])")
    } else {
        format!("[{x}]")
    }
}

/// A value that needs no evaluation, as JSON.
pub fn literal(e: &TExpr) -> Option<Value> {
    match e {
        TExpr::Str(s) => Some(json!(s)),
        TExpr::Int(n) => Some(json!(n)),
        TExpr::Bool(b) => Some(json!(b)),
        TExpr::Enum(v, _) => Some(json!(v)),
        TExpr::None(_) => Some(Value::Null),
        TExpr::Record { fields, .. } => {
            let mut o = Map::new();
            for (f, x) in fields {
                o.insert(f.clone(), literal(x)?);
            }
            Some(Value::Object(o))
        }
        TExpr::List { items, .. } => items.iter().map(literal).collect::<Option<Vec<_>>>().map(Value::Array),
        TExpr::Var { .. } | TExpr::Interp(_) => None,
    }
}

/// A JSONata test that `x` is a well-formed value of type `t`.
pub fn jsonata_check(m: &Model, x: &str, t: &Ty, depth: usize) -> String {
    match t {
        Ty::List(inner) => {
            let v = format!("$dd_v{depth}");
            format!("($type({x}) = \"array\" and $count($filter({x}, function({v}) {{ $not({}) }})) = 0)", jsonata_check(m, &v, inner, depth + 1))
        }
        Ty::Opt(inner) => format!("($not($exists({x})) or {x} = null or {})", jsonata_check(m, x, inner, depth)),
        Ty::Json => format!("$exists({x})"),
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
        Ty::List(inner) => v.as_array().map(|a| a.iter().all(|x| value_fits(m, x, inner))).unwrap_or(false),
        Ty::Opt(inner) => v.is_null() || value_fits(m, v, inner),
        Ty::Json => true,
        Ty::Str => v.is_string(),
        Ty::Bool => v.is_boolean(),
        Ty::Timestamp => v.as_str().map(is_timestamp).unwrap_or(false),
        Ty::Int | Ty::Num(_) => v.as_f64().map(|f| f.fract() == 0.0).unwrap_or(false),
        Ty::Enum(e) => v.as_str().map(|s| m.enums[*e].values.iter().any(|x| x == s)).unwrap_or(false),
        Ty::Record(r) => match v.as_object() {
            Some(o) => m.records[*r].fields.iter().all(|(f, ft)| match o.get(f) {
                Some(x) => value_fits(m, x, ft),
                None => matches!(ft, Ty::Opt(_)),
            }),
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
                let decl = task.error(n);
                if task.callback {
                    // whoever answers the callback names the error
                    return vec![n.clone()];
                }
                match (task.via(Platform::StepFunctions), decl) {
                    (Some(Via::Http { .. }), Some(ErrDef { status: Some(st), .. })) => vec![format!("States.Http.StatusCode.{st}")],
                    (Some(Via::Aws { service, .. }), Some(d)) => vec![crate::aws::error_name(service, d.exception.as_deref().unwrap_or(&d.name))],
                    _ => vec![n.clone()],
                }
            }
            Callee::Rule(_) => vec![n.clone()],
        },
    }
}
