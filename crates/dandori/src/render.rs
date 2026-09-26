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
            if task.connect.is_some() {
                headers.insert("Connect-Protocol-Version".into(), json!("1"));
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
        Some(Via::Agent { provider, instructions, model }) => {
            let input = agent_input(task, args);
            let schema = agent_schema(m, task).unwrap_or(Value::Null);
            match view {
                // the HTTP Task's request; the input goes as the arguments' JSON text
                View::Asl => {
                    let mut w = Map::new();
                    w.insert("http".into(), json!("POST"));
                    w.insert("url".into(), json!(agent_url(provider)));
                    if let Some(h) = agent_headers(provider) {
                        w.insert("headers".into(), h);
                    }
                    w.insert("body".into(), agent_request(provider, model, instructions, json!(Value::Object(input).to_string()), schema));
                    Value::Object(w)
                }
                _ => json!({ "agent": task.name, "provider": provider.name(), "model": model, "instructions": instructions, "input": input, "schema": schema }),
            }
        }
        Some(Via::StateMachine(arn)) => json!({ "state_machine": arn, "input": args }),
        // nothing is called: the workflow waits for the event by its name
        Some(Via::Event) => json!({ "event": task.name }),
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

/// Where Step Functions sends an agent's call: OpenAI's Responses API, or Claude's Messages API.
pub fn agent_url(p: Provider) -> &'static str {
    match p {
        Provider::OpenAi => "https://api.openai.com/v1/responses",
        Provider::Claude => "https://api.anthropic.com/v1/messages",
    }
}

/// The version of the Messages API that every target asks for.
pub const CLAUDE_VERSION: &str = "2023-06-01";

/// The most a Claude agent's answer may take, thinking included: the Messages API wants a
/// number, and every target sends this one.
pub const CLAUDE_MAX_TOKENS: u64 = 16000;

/// How many parameters with a choice (`anyOf`) Claude's structured outputs take in a request.
pub const CLAUDE_UNIONS: usize = 16;

/// The headers of the HTTP Task beyond what its connection adds (the API key).
pub fn agent_headers(p: Provider) -> Option<Value> {
    match p {
        Provider::OpenAi => None,
        Provider::Claude => Some(json!({ "anthropic-version": CLAUDE_VERSION })),
    }
}

/// What an agent reads: the arguments, in the order of the task's parameters. Every target
/// gives it to the model as this object's JSON text.
pub fn agent_input(task: &TaskDef, args: &Map<String, Value>) -> Map<String, Value> {
    task.params.iter().filter_map(|(p, _)| args.get(p).map(|v| (p.clone(), v.clone()))).collect()
}

/// The request for an agent's call, as Step Functions sends it: the model, what it is to do,
/// the input as text, and the JSON Schema its answer is held to. OpenAI's Responses API takes
/// them as they are; Claude's Messages API wants the instructions as the system prompt, the
/// input as the user's message, and a limit on the answer's length.
pub fn agent_request(p: Provider, model: &str, instructions: &str, input: Value, schema: Value) -> Value {
    match p {
        Provider::OpenAi => json!({
            "model": model,
            "instructions": instructions,
            "input": input,
            "text": { "format": { "type": "json_schema", "name": "answer", "strict": true, "schema": schema } }
        }),
        Provider::Claude => json!({
            "model": model,
            "max_tokens": CLAUDE_MAX_TOKENS,
            "system": instructions,
            "messages": [{ "role": "user", "content": input }],
            "output_config": { "format": { "type": "json_schema", "schema": schema } }
        }),
    }
}

/// What an agent task answers in (`answer_schema`).
pub fn agent_schema(m: &Model, task: &TaskDef) -> Option<Value> {
    let provider = match &task.binding {
        Some(Binding::Agent { provider, .. }) => *provider,
        _ => Provider::OpenAi,
    };
    answer_schema(m, task.result.as_ref()?, task.result_range, provider)
}

/// What an agent answers in: `{"answer": …}` with a value of the task's type inside, since
/// OpenAI's Structured Outputs want an object at the top whatever the type is. None when
/// the type cannot be said as such a schema (`json_schema`).
pub fn answer_schema(m: &Model, t: &Ty, rg: Option<Range>, p: Provider) -> Option<Value> {
    Some(json!({ "type": "object", "properties": { "answer": json_schema(m, t, rg, p)? }, "required": ["answer"], "additionalProperties": false }))
}

/// The JSON Schema of a value of type `t`, in the strict form of OpenAI's Structured
/// Outputs: a record's fields are all required and no others are allowed, a value that may
/// be absent is a choice with null, a timestamp has the form Step Functions' Wait takes, and
/// a number with a unit says its unit. A number's range is its `minimum` and `maximum` for
/// OpenAI, and words in its description for Claude, whose structured outputs take neither.
/// None when `t` holds `json`, or a record that holds itself through others, which such a
/// schema cannot say.
pub fn json_schema(m: &Model, t: &Ty, rg: Option<Range>, p: Provider) -> Option<Value> {
    fn number(unit: Option<&str>, rg: Option<Range>, p: Provider) -> Value {
        let mut o = Map::new();
        o.insert("type".into(), json!("integer"));
        let words: Vec<String> = unit.map(String::from).into_iter().chain(rg.filter(|_| p == Provider::Claude).map(|r| r.show())).collect();
        if !words.is_empty() {
            o.insert("description".into(), json!(words.join(", ")));
        }
        if let (Some(r), Provider::OpenAi) = (rg, p) {
            if let Some(lo) = r.lo {
                o.insert("minimum".into(), json!(lo));
            }
            if let Some(hi) = r.hi {
                o.insert("maximum".into(), json!(hi));
            }
        }
        Value::Object(o)
    }
    fn go(m: &Model, t: &Ty, rg: Option<Range>, p: Provider, within: &mut Vec<RecordId>) -> Option<Value> {
        Some(match t {
            Ty::Int => number(None, rg, p),
            Ty::Num(unit) => number(Some(unit), rg, p),
            Ty::Str => json!({ "type": "string" }),
            Ty::Bool => json!({ "type": "boolean" }),
            Ty::Timestamp => json!({ "type": "string", "pattern": TIMESTAMP_RE }),
            Ty::Enum(e) => json!({ "type": "string", "enum": m.enums[*e].values }),
            Ty::List(x) => json!({ "type": "array", "items": go(m, x, rg, p, within)? }),
            Ty::Opt(x) => json!({ "anyOf": [go(m, x, rg, p, within)?, { "type": "null" }] }),
            Ty::Json => return None,
            Ty::Record(r) => {
                if within.contains(r) {
                    return None;
                }
                within.push(*r);
                let mut props = Map::new();
                for (f, ft) in &m.records[*r].fields {
                    props.insert(f.clone(), go(m, ft, m.field_range(*r, f), p, within)?);
                }
                within.pop();
                let required: Vec<&String> = m.records[*r].fields.iter().map(|(f, _)| f).collect();
                json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
            }
        })
    }
    go(m, t, rg, p, &mut Vec::new())
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

/// How many choices (`anyOf`) a JSON Schema has, the ones inside others included.
pub fn schema_unions(s: &Value) -> usize {
    match s {
        Value::Object(o) => usize::from(o.contains_key("anyOf")) + o.values().map(schema_unions).sum::<usize>(),
        Value::Array(a) => a.iter().map(schema_unions).sum(),
        _ => 0,
    }
}

/// The enums a value of type `t` can hold, each once.
pub fn enums_of(m: &Model, t: &Ty) -> Vec<usize> {
    fn go(m: &Model, t: &Ty, out: &mut Vec<usize>, within: &mut Vec<RecordId>) {
        match t {
            Ty::Enum(e) => {
                if !out.contains(e) {
                    out.push(*e);
                }
            }
            Ty::List(x) | Ty::Opt(x) => go(m, x, out, within),
            Ty::Record(r) => {
                if !within.contains(r) {
                    within.push(*r);
                    for (_, ft) in &m.records[*r].fields {
                        go(m, ft, out, within);
                    }
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    go(m, t, &mut out, &mut Vec::new());
    out
}

/// The value of the enum `e` that `s` spells without regard to case, when it is not one already.
fn enum_folded(m: &Model, e: usize, s: &str) -> Option<String> {
    let values = &m.enums[e].values;
    if values.iter().any(|v| v == s) {
        return None;
    }
    values.iter().find(|v| v.to_lowercase() == s.to_lowercase()).cloned()
}

/// A Claude agent's answer with every enum value in it spelled as the enum spells it. Claude's
/// structured outputs do not keep the case of an enum's value, so an answer may differ from a
/// value only in case; dandori takes it as that value. What is not of the type stays as it is,
/// for the answer's check to find.
pub fn fold_enums(m: &Model, v: &Value, t: &Ty) -> Value {
    match (t, v) {
        (Ty::Enum(e), Value::String(s)) => enum_folded(m, *e, s).map(Value::String).unwrap_or_else(|| v.clone()),
        (Ty::Opt(x), _) => fold_enums(m, v, x),
        (Ty::List(x), Value::Array(a)) => Value::Array(a.iter().map(|i| fold_enums(m, i, x)).collect()),
        (Ty::Record(r), Value::Object(o)) => {
            let mut out = o.clone();
            for (f, ft) in &m.records[*r].fields {
                if let Some(x) = o.get(f) {
                    out.insert(f.clone(), fold_enums(m, x, ft));
                }
            }
            Value::Object(out)
        }
        _ => v.clone(),
    }
}

/// `fold_enums` in JSONata, for Step Functions to read a Claude agent's answer `x` with; None
/// when `t` holds no enum, and there is nothing to fold.
pub fn jsonata_fold(m: &Model, x: &str, t: &Ty, depth: usize) -> Option<String> {
    if enums_of(m, t).is_empty() {
        return None;
    }
    let v = format!("$dd_f{depth}");
    Some(match t {
        Ty::Enum(e) => {
            // each value by its lowercase spelling; the checker refuses two values that differ only in case
            let spelled: Map<String, Value> = m.enums[*e].values.iter().map(|x| (x.to_lowercase(), json!(x))).collect();
            let w = format!("$dd_w{depth}");
            format!("({v} := {x}; $type({v}) = \"string\" ? ({w} := $lookup({}, $lowercase({v})); $exists({w}) ? {w} : {v}) : {v})", Value::Object(spelled))
        }
        Ty::Opt(inner) => jsonata_fold(m, x, inner, depth)?,
        Ty::List(inner) => {
            let item = format!("$dd_i{depth}");
            let f = jsonata_fold(m, &item, inner, depth + 1)?;
            format!("({v} := {x}; $type({v}) = \"array\" ? [$map({v}, function({item}) {{ {f} }})] : {v})")
        }
        Ty::Record(r) => {
            let mut parts = Vec::new();
            for (f, ft) in &m.records[*r].fields {
                if let Some(e) = jsonata_fold(m, &format!("{v}.{}", jsonata_field(f)), ft, depth + 1) {
                    parts.push(format!("{}: {e}", jsonata_string(f)));
                }
            }
            format!("({v} := {x}; $type({v}) = \"object\" ? $merge([{v}, {{{}}}]) : {v})", parts.join(", "))
        }
        _ => return None,
    })
}

/// A value of type `t` whose enum values are spelled in another case where they have one (the
/// first letter's case turned), as Claude may answer: for the scenarios to see that the targets
/// take it as the value.
pub fn recase(m: &Model, v: &Value, t: &Ty) -> Value {
    match (t, v) {
        (Ty::Enum(_), Value::String(s)) => {
            let mut cs = s.chars();
            match cs.next() {
                Some(c) if c.is_lowercase() => Value::String(c.to_uppercase().chain(cs).collect()),
                Some(c) if c.is_uppercase() => Value::String(c.to_lowercase().chain(cs).collect()),
                _ => v.clone(),
            }
        }
        (Ty::Opt(x), _) => recase(m, v, x),
        (Ty::List(x), Value::Array(a)) => Value::Array(a.iter().map(|i| recase(m, i, x)).collect()),
        (Ty::Record(r), Value::Object(o)) => {
            let mut out = o.clone();
            for (f, ft) in &m.records[*r].fields {
                if let Some(x) = o.get(f) {
                    out.insert(f.clone(), recase(m, x, ft));
                }
            }
            Value::Object(out)
        }
        _ => v.clone(),
    }
}

/// Whether a value of type `t` can hold an enum value that has a case to turn.
pub fn has_cased_enum(m: &Model, t: &Ty) -> bool {
    enums_of(m, t).iter().any(|e| m.enums[*e].values.iter().any(|v| v.chars().next().is_some_and(|c| c.is_lowercase() || c.is_uppercase())))
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

/// A JSONata expression for `x` with the zero values filled in that protobuf's JSON leaves out
/// (`apis::fill`): what a `connect` task's answer reads as.
pub fn jsonata_fill(x: &str, zeros: &Value) -> String {
    format!(
        "($dd_fill := function($v, $z) {{ $type($v) = \"object\" ? ($d := $z.f ? $sift($z.f, function($x, $k) {{ $not($exists($lookup($v, $k))) or $lookup($v, $k) = null }}) : {{}}; $o := $merge([$v, $d ? $d : {{}}]); \
$m := $z.m ? $merge($each($z.m, function($d, $k) {{ $type($lookup($o, $k)) = \"object\" ? {{ $k: $dd_fill($lookup($o, $k), $d) }} : {{}} }})) : {{}}; \
$l := $z.l ? $merge($each($z.l, function($d, $k) {{ $type($lookup($o, $k)) = \"array\" ? {{ $k: [$map($lookup($o, $k), function($i) {{ $dd_fill($i, $d) }})] }} : {{}} }})) : {{}}; \
$merge([$o, $m, $l])) : $v }}; $dd_fill({x}, {}))",
        zeros
    )
}

/// A JSONata test that `x` is a well-formed value of type `t`, and in `rg` when it is a number
/// (or its numbers, when it is a `?` or a list).
pub fn jsonata_check(m: &Model, x: &str, t: &Ty, rg: Option<Range>, depth: usize) -> String {
    match t {
        Ty::List(inner) => {
            let v = format!("$dd_v{depth}");
            format!("($type({x}) = \"array\" and $count($filter({x}, function({v}) {{ $not({}) }})) = 0)", jsonata_check(m, &v, inner, rg, depth + 1))
        }
        Ty::Opt(inner) => format!("($not($exists({x})) or {x} = null or {})", jsonata_check(m, x, inner, rg, depth)),
        Ty::Json => format!("$exists({x})"),
        Ty::Str => format!("$type({x}) = \"string\""),
        Ty::Bool => format!("$type({x}) = \"boolean\""),
        Ty::Timestamp => format!("($type({x}) = \"string\" and $contains({x}, /{TIMESTAMP_RE}/))"),
        Ty::Int | Ty::Num(_) => {
            let mut parts = vec![format!("$type({x}) = \"number\""), format!("{x} = $floor({x})")];
            parts.extend(rg.map(|r| r.tests(|op, n| format!("{x} {op} {n}"))).unwrap_or_default());
            format!("({})", parts.join(" and "))
        }
        Ty::Enum(e) => format!("{x} in {}", jsonata_list(&m.enums[*e].values)),
        Ty::Record(r) => {
            let mut parts = vec![format!("$type({x}) = \"object\"")];
            if depth < 4 {
                for (f, ft) in &m.records[*r].fields {
                    parts.push(jsonata_check(m, &format!("{x}.{}", jsonata_field(f)), ft, m.field_range(*r, f), depth + 1));
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

/// Whether a JSON value is a well-formed value of type `t`, in `rg` (the interpreter's side of `jsonata_check`).
pub fn value_fits(m: &Model, v: &Value, t: &Ty, rg: Option<Range>) -> bool {
    match t {
        Ty::List(inner) => v.as_array().map(|a| a.iter().all(|x| value_fits(m, x, inner, rg))).unwrap_or(false),
        Ty::Opt(inner) => v.is_null() || value_fits(m, v, inner, rg),
        Ty::Json => true,
        Ty::Str => v.is_string(),
        Ty::Bool => v.is_boolean(),
        Ty::Timestamp => v.as_str().map(is_timestamp).unwrap_or(false),
        Ty::Int | Ty::Num(_) => v.as_f64().is_some_and(|f| f.fract() == 0.0 && rg.map_or(true, |r| r.lo.map_or(true, |lo| f >= lo as f64) && r.hi.map_or(true, |hi| f <= hi as f64))),
        Ty::Enum(e) => v.as_str().map(|s| m.enums[*e].values.iter().any(|x| x == s)).unwrap_or(false),
        Ty::Record(r) => match v.as_object() {
            Some(o) => m.records[*r].fields.iter().all(|(f, ft)| match o.get(f) {
                Some(x) => value_fits(m, x, ft, m.field_range(*r, f)),
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
