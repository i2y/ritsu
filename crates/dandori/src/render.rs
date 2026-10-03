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
            // a rule called at its Connect service is an HTTP request, which every platform sends alike
            if let Some(c) = &ru.connect {
                return json!({ "http": "POST", "url": c.url, "headers": { "Connect-Protocol-Version": "1" }, "body": rule_request(c, args) });
            }
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
        Some(Via::Agent { provider, instructions, model, url, effort }) => {
            let input = agent_input(task, args);
            let schema = agent_schema(m, task).unwrap_or(Value::Null);
            match view {
                // the HTTP Task's request; the input goes as the arguments' JSON text
                View::Asl => {
                    let mut w = Map::new();
                    w.insert("http".into(), json!("POST"));
                    w.insert("url".into(), json!(agent_url(provider, url)));
                    if let Some(h) = agent_headers(provider) {
                        w.insert("headers".into(), h);
                    }
                    w.insert("body".into(), agent_request(provider, model, instructions, json!(Value::Object(input).to_string()), schema, effort));
                    Value::Object(w)
                }
                _ => {
                    let mut c = json!({ "agent": task.name, "provider": provider.name(), "model": model, "instructions": instructions, "input": input, "schema": schema });
                    if let Some(u) = url {
                        c["url"] = json!(u);
                    }
                    if let Some(e) = effort {
                        c["effort"] = json!(e);
                    }
                    c
                }
            }
        }
        // an HTTP request, which every target sends as Step Functions' HTTP Task does
        Some(Via::Jev(j)) => json!({ "http": "POST", "url": JEV_URL, "body": jev_request(j, Value::Object(agent_input(task, args))) }),
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

/// Where every target sends an agent's call over HTTP: OpenAI's Responses API, or that of the
/// server `url` names (Open Responses, whose path is the same), or Claude's Messages API.
pub fn agent_url(p: Provider, url: Option<&str>) -> String {
    match (p, url) {
        (Provider::OpenAi, Some(u)) => format!("{}/responses", u.trim_end_matches('/')),
        (Provider::OpenAi, None) => "https://api.openai.com/v1/responses".into(),
        (Provider::Claude, _) => "https://api.anthropic.com/v1/messages".into(),
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
/// input as the user's message, and a limit on the answer's length. An `effort` goes where each
/// API takes it: `reasoning.effort` on the Responses API, `output_config.effort` on Claude's.
pub fn agent_request(p: Provider, model: &str, instructions: &str, input: Value, schema: Value, effort: Option<&str>) -> Value {
    match p {
        Provider::OpenAi => {
            let mut r = json!({
                "model": model,
                "instructions": instructions,
                "input": input,
                "text": { "format": { "type": "json_schema", "name": "answer", "strict": true, "schema": schema } }
            });
            if let Some(e) = effort {
                r["reasoning"] = json!({ "effort": e });
            }
            r
        }
        Provider::Claude => {
            let mut config = json!({ "format": { "type": "json_schema", "schema": schema } });
            if let Some(e) = effort {
                config["effort"] = json!(e);
            }
            json!({
                "model": model,
                "max_tokens": CLAUDE_MAX_TOKENS,
                "system": instructions,
                "messages": [{ "role": "user", "content": input }],
                "output_config": config
            })
        }
    }
}

/// The body of a Jev task's request: its state (the arguments, as an agent's input), the model,
/// and the questions by their ids, as TypeSafe's API takes them.
pub fn jev_request(j: &Jev, state: Value) -> Value {
    json!({ "state": state, "model": j.model, "questions": jev_questions(j) })
}

/// The questions of a Jev task: a choice's `criteria` are its options with what each means (null
/// where the task says nothing), a score's the levels' meanings from the lowest, and a noul's what
/// yes and no mean, when the task says.
pub fn jev_questions(j: &Jev) -> Value {
    let mut qs = Map::new();
    for q in &j.questions {
        let mut o = Map::new();
        let kind = match q.kind {
            QuestionKind::Choice => "choice",
            QuestionKind::Score => "score",
            QuestionKind::Noul => "noul",
        };
        o.insert("type".into(), json!(kind));
        o.insert("instructions".into(), json!(q.instructions));
        match q.kind {
            QuestionKind::Choice => {
                o.insert("criteria".into(), Value::Object(q.options.iter().map(|(v, m)| (v.clone(), json!(m))).collect()));
            }
            QuestionKind::Score => {
                o.insert("criteria".into(), Value::Array(q.options.iter().map(|(_, m)| json!(m)).collect()));
            }
            QuestionKind::Noul if !q.options.is_empty() => {
                o.insert("criteria".into(), Value::Object(q.options.iter().map(|(v, m)| (v.clone(), json!(m))).collect()));
            }
            QuestionKind::Noul => {}
        }
        qs.insert(q.id.clone(), Value::Object(o));
    }
    Value::Object(qs)
}

/// Jev's answer to one question, read: the value it takes and how sure Jev is of it, from 0 to 1.
/// A choice and a score say how sure (`confidence`); a score's place is taken at the nearest
/// level, a half going up; a noul's answer is yes when its probability is over one half, and Jev
/// is as sure of it as its probability. None when the answer is not there or not of its kind.
fn jev_one(q: &Question, a: &Value) -> Option<(Value, f64)> {
    let unit = |x: Option<&Value>| x.and_then(|x| x.as_f64()).filter(|c| (0.0..=1.0).contains(c));
    match q.kind {
        QuestionKind::Choice => {
            let v = a.get("choice")?.as_str()?;
            if !q.options.iter().any(|(o, _)| o == v) {
                return None;
            }
            Some((json!(v), unit(a.get("confidence"))?))
        }
        QuestionKind::Score => {
            let level = (a.get("score")?.as_f64()? + 0.5).floor();
            if level < 0.0 || level >= q.options.len() as f64 {
                return None;
            }
            Some((json!(q.options[level as usize].0), unit(a.get("confidence"))?))
        }
        QuestionKind::Noul => {
            let p = unit(a.get("noul"))?;
            Some((json!(p > 0.5), if p > 0.5 { p } else { 1.0 - p }))
        }
    }
}

/// How sure Jev is, from 0 to 1, as a count of a rate's steps: rounded down, so that the rate is
/// never surer than Jev, after a billionth that takes up how a decimal falls between binary
/// fractions (0.29 × 100 is 28.999…). Every target counts the same way.
pub fn rate_steps(c: f64, per: u64) -> i64 {
    (c * per as f64 + 1e-9).floor() as i64
}

/// A Jev task's answer, read from the body of Jev's response as every target reads it: the value
/// of the task's type, null where an answer is not there or not of its kind (which the answer's
/// check then refuses), and whether every answer is there and one is less sure than the task's
/// `confidence`, which fails the call with its error.
pub fn jev_read(j: &Jev, body: &Value) -> (Value, bool) {
    let answers = body.get("answers").filter(|a| a.is_object());
    let read: Vec<Option<(Value, f64)>> = j.questions.iter().map(|q| answers.and_then(|a| a.get(&q.id)).filter(|a| a.is_object()).and_then(|a| jev_one(q, a))).collect();
    let all = read.iter().all(|x| x.is_some());
    let low = all && j.floor.as_ref().is_some_and(|(f, _)| read.iter().flatten().any(|(_, c)| c < f));
    let value = match j.questions.as_slice() {
        [q] if q.field.is_none() => read[0].as_ref().map(|(v, _)| v.clone()).unwrap_or(Value::Null),
        _ => {
            let mut o = Map::new();
            for (q, r) in j.questions.iter().zip(&read) {
                o.insert(q.field.clone().unwrap_or_default(), r.as_ref().map(|(v, _)| v.clone()).unwrap_or(Value::Null));
            }
            for (f, qi, per) in &j.confidences {
                o.insert(f.clone(), read[*qi].as_ref().map(|(_, c)| json!(rate_steps(*c, *per))).unwrap_or(Value::Null));
            }
            Value::Object(o)
        }
    };
    (value, low)
}

/// Jev's response to a task's questions, with `v` the value of the task's type it reads as: every
/// answer as sure as the task's `confidence` asks and no more (wholly sure without one), or with
/// `low`, less sure. The scenarios answer the calls with it.
pub fn jev_wire(j: &Jev, v: &Value, low: bool) -> Value {
    let floor = j.floor.as_ref().map(|(f, _)| *f);
    // numbers of a few decimals, as Jev gives them, which every language reads back alike; a
    // whole number is written as one (1, not 1.0), as JavaScript writes it
    let short = |x: f64| (x * 10_000.0).round() / 10_000.0;
    let num = |x: f64| if x.fract() == 0.0 && x.abs() < 1e15 { json!(x as i64) } else { json!(x) };
    let sure = match (floor, low) {
        (Some(f), true) => short((f - 0.01).max(0.0)),
        (Some(f), false) => f,
        (None, _) => 1.0,
    };
    let mut answers = Map::new();
    for q in &j.questions {
        let x = match &q.field {
            Some(f) => v.get(f).cloned().unwrap_or(Value::Null),
            None => v.clone(),
        };
        let at = |x: &Value| q.options.iter().position(|(o, _)| Some(o.as_str()) == x.as_str()).unwrap_or(0);
        // the rest of the probability, shared by the other options
        let rest = |n: usize| if n > 1 { short((1.0 - sure) / (n - 1) as f64) } else { 0.0 };
        let a = match q.kind {
            QuestionKind::Choice => {
                let chosen = at(&x);
                let probabilities: Map<String, Value> = q.options.iter().enumerate().map(|(i, (o, _))| (o.clone(), num(if i == chosen { sure } else { rest(q.options.len()) }))).collect();
                json!({ "type": "choice", "choice": q.options[chosen].0, "probabilities": probabilities, "confidence": num(sure) })
            }
            QuestionKind::Score => {
                let level = at(&x);
                let legend: Map<String, Value> = q.options.iter().enumerate().map(|(i, (_, m))| (i.to_string(), json!(m))).collect();
                let probabilities: Map<String, Value> = (0..q.options.len()).map(|i| (i.to_string(), num(if i == level { sure } else { rest(q.options.len()) }))).collect();
                json!({ "type": "score", "score": level, "legend": legend, "probabilities": probabilities, "confidence": num(sure) })
            }
            QuestionKind::Noul => {
                let yes = x.as_bool().unwrap_or(true);
                let p = match (floor, low) {
                    // as sure as the floor, of yes or of no; or not sure either way
                    (Some(f), false) if f > 0.5 => {
                        if yes {
                            f
                        } else {
                            1.0 - f
                        }
                    }
                    (Some(f), true) if f > 0.5 => 0.5,
                    _ => {
                        if yes {
                            1.0
                        } else {
                            0.0
                        }
                    }
                };
                json!({ "type": "noul", "noul": num(p) })
            }
        };
        answers.insert(q.id.clone(), a);
    }
    json!({ "model": j.model, "answers": answers, "usage": { "input_tokens": 120, "output_tokens": 20 } })
}

/// What the code dandori writes asks Jev for a task and reads its answer by (io's `jev`): the
/// questions of the request; for each, its id, the field of the answer it fills (none when the
/// answer is its value), its kind and the values it takes (a score's from the lowest level); the
/// fields that take how sure Jev is, as a count of a rate's steps; and how sure every answer must
/// be, with the error and its cause when one is not.
pub fn jev_spec(task: &TaskDef, j: &Jev) -> Value {
    let read: Vec<Value> = j
        .questions
        .iter()
        .map(|q| {
            let kind = match q.kind {
                QuestionKind::Choice => "choice",
                QuestionKind::Score => "score",
                QuestionKind::Noul => "noul",
            };
            let values: Vec<&String> = if q.kind == QuestionKind::Noul { vec![] } else { q.options.iter().map(|(v, _)| v).collect() };
            let mut o = json!({ "id": q.id, "kind": kind, "values": values });
            if let Some(f) = &q.field {
                o["field"] = json!(f);
            }
            o
        })
        .collect();
    let confidences: Vec<Value> = j.confidences.iter().map(|(f, qi, per)| json!({ "field": f, "question": qi, "per": per })).collect();
    let mut spec = json!({ "questions": jev_questions(j), "read": read, "confidences": confidences });
    if let Some((at, error)) = &j.floor {
        spec["floor"] = json!({ "at": at, "error": error, "cause": jev_low_cause(task) });
    }
    spec
}

/// What a Jev task's call fails with when Jev is less sure of an answer than the task asks: the
/// cause beside the error `confidence … else <error>` names, the same on every target.
pub fn jev_low_cause(task: &TaskDef) -> String {
    let floor = task.jev().and_then(|j| j.floor.as_ref()).map(|(f, _)| *f).unwrap_or(0.0);
    format!("Jev is less sure of an answer of {} than {floor}", task.name)
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

/// The value of an expression, in JSONata. A field that may be absent (a `T?`, a `json`) reads as
/// null, so that no expression the state machine evaluates comes out undefined.
pub fn jsonata_expr(e: &TExpr) -> String {
    match e {
        TExpr::Var { name, fields, ty } => {
            let p = jsonata_path(name, fields);
            if !fields.is_empty() && matches!(ty, Ty::Opt(_) | Ty::Json) {
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

// ---------------------------------------------------------------------------
// A rule called at its Connect service (DESIGN 1.13)

/// The greatest whole number every platform holds as itself: 2^53 − 1, a JavaScript number's.
pub const SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// A whole number written out in decimal, `-12`, and nothing else (`+5`, `1.0` and `1e3` are not):
/// at most 16 digits, and not above 2^53 − 1, so that a number is read as the same number on every
/// platform, in JavaScript and in JSONata, whose numbers are doubles, as in Rust and Python.
fn decimal(s: &str) -> Option<i64> {
    let digits = s.strip_prefix('-').unwrap_or(s);
    if digits.is_empty() || digits.len() > 16 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok().filter(|n: &i64| n.abs() <= SAFE_INTEGER)
}

/// One argument as protobuf's JSON writes the field it goes to: a number as its decimal string (the
/// 64-bit integer it is), an enum's value by the `.proto`'s name for it. What is not of the field's
/// kind, or an enum's value the table does not have, is sent as it is, for the service to refuse.
fn wire_arg(f: &crate::rulec::WireField, v: &Value) -> Value {
    use crate::rulec::WireKind;
    match (&f.kind, v) {
        (WireKind::Int, v) if v.is_i64() || v.is_u64() => json!(v.to_string()),
        (WireKind::Enum { values, .. }, Value::String(s)) => json!(values.iter().find(|(rule, _)| rule == s).map(|(_, proto)| proto.as_str()).unwrap_or(s)),
        _ => v.clone(),
    }
}

/// The body of the POST that calls a rule's service: each argument under its JSON key, as `wire_arg`
/// writes it. An argument that is null is not sent; every other one is, at its zero value too (false,
/// 0, an enum's value 0), since the service tells a field left out from one set to zero and refuses
/// an input left out. The arguments are by the rule's names for them.
pub fn rule_request(c: &RuleConnect, args: &Map<String, Value>) -> Value {
    let mut body = Map::new();
    for f in &c.request {
        if let Some(v) = args.get(&f.name).filter(|v| !v.is_null()) {
            body.insert(f.json.clone(), wire_arg(f, v));
        }
    }
    Value::Object(body)
}

/// What a rule's service answers, read as the rule's own would be: the zero values its JSON leaves
/// out put back (protobuf reads them so; an enum left out is its value 0, which is the rule's value
/// when a contract puts one at 0), then each field by the rule's name for it, a number from
/// its decimal string, an enum's value by the rule's name for it. What is not one of those (a number
/// that is not a decimal, a name the table does not have, a body that is not an object) is left as
/// it is, for the check of the answer to refuse: nothing here raises, so that every platform ends
/// such an answer the same way. A field the answer does not have is null.
pub fn rule_read(c: &RuleConnect, body: &Value) -> Value {
    use crate::rulec::WireKind;
    if !body.is_object() {
        return body.clone();
    }
    let filled = crate::apis::fill(body, &c.zeros);
    let mut out = Map::new();
    for f in &c.response {
        let read = match (&f.kind, filled.get(&f.json)) {
            (_, None) => Value::Null,
            (WireKind::Int, Some(Value::String(s))) => decimal(s).map(|n| json!(n)).unwrap_or_else(|| Value::String(s.clone())),
            (WireKind::Enum { values, .. }, Some(Value::String(s))) => values.iter().find(|(_, proto)| proto == s).map(|(rule, _)| json!(rule)).unwrap_or_else(|| Value::String(s.clone())),
            (_, Some(v)) => v.clone(),
        };
        out.insert(f.name.clone(), read);
    }
    Value::Object(out)
}

/// A rule's answer, the record of its outputs, as its service writes it: the inverse of `rule_read`.
/// A field at its zero value (false, 0, an empty string, and an enum's value 0 when that is a value
/// of the rule's, as a contract's can be) is left out, as protobuf's JSON leaves it out, and so is a
/// null; a number is its decimal string and an enum's value the `.proto`'s name for it. What a
/// scenario answers a call of the rule with.
pub fn rule_wire(c: &RuleConnect, record: &Value) -> Value {
    use crate::rulec::WireKind;
    let Some(o) = record.as_object() else { return record.clone() };
    let mut body = Map::new();
    for f in &c.response {
        let Some(v) = o.get(&f.name).filter(|v| !v.is_null()) else { continue };
        let zero = match (&f.kind, v) {
            (WireKind::Bool, Value::Bool(false)) => true,
            (WireKind::Int, v) => v.as_i64() == Some(0),
            (WireKind::Str, Value::String(s)) => s.is_empty(),
            (WireKind::Enum { .. }, Value::String(s)) => f.kind.zero_value() == Some(s.as_str()),
            _ => false,
        };
        if !zero {
            body.insert(f.json.clone(), wire_arg(f, v));
        }
    }
    Value::Object(body)
}

/// The table of an enum's values: by the rule's name for each, to the `.proto`'s (what a request
/// sends), or the other way (what a response is read by).
fn enum_table(values: &[(String, String)], to_rule: bool) -> Value {
    Value::Object(values.iter().map(|(rule, proto)| if to_rule { (proto.clone(), json!(rule)) } else { (rule.clone(), json!(proto)) }).collect())
}

/// What the code dandori writes needs to call a rule's service: the URL, the fields of the request
/// (an enum's table is rule name → `.proto` name) and of the response (`.proto` name → rule name),
/// and the zero values the response leaves out. The generated code carries it as a table.
pub fn rule_wire_spec(c: &RuleConnect) -> Value {
    use crate::rulec::WireKind;
    let field = |f: &crate::rulec::WireField, to_rule: bool| {
        let mut o = Map::new();
        o.insert("name".into(), json!(f.name));
        o.insert("json".into(), json!(f.json));
        match &f.kind {
            WireKind::Bool => o.insert("kind".into(), json!("bool")),
            WireKind::Int => o.insert("kind".into(), json!("int")),
            WireKind::Str => o.insert("kind".into(), json!("str")),
            WireKind::Enum { values, .. } => {
                o.insert("kind".into(), json!("enum"));
                o.insert("values".into(), enum_table(values, to_rule))
            }
        };
        Value::Object(o)
    };
    json!({
        "url": c.url,
        "request": c.request.iter().map(|f| field(f, false)).collect::<Vec<_>>(),
        "response": c.response.iter().map(|f| field(f, true)).collect::<Vec<_>>(),
        "zeros": c.zeros,
    })
}

/// A JSONata expression for the value of `x` through a table of names, or `x` itself when it is not
/// a string the table has. The table is asked only for the names it has as its own: a name that
/// an object has of its own accord (`constructor`, `toString`, `__proto__`) is not one that a
/// lookup in it may find, and a service can answer anything.
fn jsonata_enum_by_table(x: &str, table: &Value) -> String {
    format!("($dd_v := {x}; $dd_t := {table}; $type($dd_v) = \"string\" and $dd_v in $keys($dd_t) ? $lookup($dd_t, $dd_v) : $dd_v)")
}

/// The body of a rule's request as Step Functions' HTTP Task sends it: the fields by their JSON
/// keys, each a value known now or a `{% … %}` JSONata expression that writes it as `rule_request`
/// does (a number through `$string`, an enum's value through the table).
pub fn jsonata_rule_request(c: &RuleConnect, args: &[(String, TExpr)]) -> Value {
    use crate::rulec::WireKind;
    let mut body = Map::new();
    for f in &c.request {
        let Some((_, e)) = args.iter().find(|(a, _)| *a == f.name) else { continue };
        let sent = match literal(e) {
            Some(Value::Null) => continue,
            Some(v) => wire_arg(f, &v),
            None => {
                let x = jsonata_expr(e);
                match &f.kind {
                    WireKind::Int => json!(format!("{{% $string({x}) %}}")),
                    WireKind::Enum { values, .. } => json!(format!("{{% {} %}}", jsonata_enum_by_table(&x, &enum_table(values, false)))),
                    _ => json!(format!("{{% {x} %}}")),
                }
            }
        };
        body.insert(f.json.clone(), sent);
    }
    Value::Object(body)
}

/// A JSONata expression for what `rule_read` makes of the response `x`: the zero values filled in,
/// then the record of the rule's outputs. A number is read from a decimal string only when it is
/// one, and an enum's value through the table only when the table has it: JSONata would raise on
/// a value of another kind, and the answer's check is to refuse it instead.
pub fn jsonata_rule_read(c: &RuleConnect, x: &str) -> String {
    use crate::rulec::WireKind;
    let mut parts = Vec::new();
    for f in &c.response {
        let get = format!("$lookup($dd_b, {})", jsonata_string(&f.json));
        let read = match &f.kind {
            WireKind::Int => format!("($dd_v := {get}; ($type($dd_v) = \"string\" and $contains($dd_v, /^-?[0-9]{{1,16}}$/) and $abs($number($dd_v)) <= {SAFE_INTEGER}) ? $number($dd_v) : $dd_v)"),
            WireKind::Enum { values, .. } => jsonata_enum_by_table(&get, &enum_table(values, true)),
            _ => get,
        };
        parts.push(format!("{}: {read}", jsonata_string(&f.name)));
    }
    format!("($dd_b := {}; $type($dd_b) = \"object\" ? {{{}}} : $dd_b)", jsonata_fill(x, &c.zeros), parts.join(", "))
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
                    // a `json` field that is not there reads as null, which is a value of it
                    if *ft == Ty::Json {
                        continue;
                    }
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
                // a field that is not there reads as null: a value of a `T?` and of a `json`
                None => matches!(ft, Ty::Opt(_) | Ty::Json),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rulec::{WireField, WireKind};

    /// The urgency rule's service: a bool and a number in, a bool and an enum out.
    fn urgency() -> RuleConnect {
        let carrier = WireKind::Enum { zero: "CARRIER_UNSPECIFIED".into(), values: vec![("standard".into(), "CARRIER_STANDARD".into()), ("next_day".into(), "CARRIER_NEXTDAY".into())] };
        let field = |name: &str, json: &str, kind: WireKind| WireField { name: name.into(), json: json.into(), kind, optional: false };
        RuleConnect {
            base: "https://rules.example.com".into(),
            url: "https://rules.example.com/rulec.urgency.v1.UrgencyService/Decide".into(),
            request: vec![field("会員", "member", WireKind::Bool), field("金額", "amount", WireKind::Int)],
            response: vec![field("急ぎ", "urgent", WireKind::Bool), field("便", "carrier", carrier)],
            zeros: json!({ "f": { "urgent": false, "carrier": "CARRIER_UNSPECIFIED" } }),
        }
    }

    fn args(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn a_request_is_written_as_protobufs_json_writes_it() {
        let c = urgency();
        // a number is its decimal string, the keys are the JSON names, and what is null is not sent
        assert_eq!(rule_request(&c, &args(json!({ "会員": true, "金額": 5000 }))), json!({ "member": true, "amount": "5000" }));
        assert_eq!(rule_request(&c, &args(json!({ "会員": false, "金額": -1 }))), json!({ "member": false, "amount": "-1" }));
        assert_eq!(rule_request(&c, &args(json!({ "会員": null, "金額": 7 }))), json!({ "amount": "7" }));
        // an enum's value is the `.proto`'s name for it, and one the table has not is sent as it is
        let mut e = urgency();
        e.request = e.response.clone();
        assert_eq!(rule_request(&e, &args(json!({ "急ぎ": true, "便": "next_day" }))), json!({ "urgent": true, "carrier": "CARRIER_NEXTDAY" }));
        assert_eq!(rule_request(&e, &args(json!({ "便": "drone" }))), json!({ "carrier": "drone" }));
    }

    #[test]
    fn a_response_is_read_with_its_zero_values_put_back() {
        let c = urgency();
        assert_eq!(rule_read(&c, &json!({ "urgent": true, "carrier": "CARRIER_NEXTDAY" })), json!({ "急ぎ": true, "便": "next_day" }));
        // what protobuf's JSON leaves out is there: false, and the enum's zero value, which no value of the rule is
        assert_eq!(rule_read(&c, &json!({ "carrier": "CARRIER_STANDARD" })), json!({ "急ぎ": false, "便": "standard" }));
        assert_eq!(rule_read(&c, &json!({})), json!({ "急ぎ": false, "便": "CARRIER_UNSPECIFIED" }));
        // a null is a field left out; a name nothing has stays as it is, and so does a body that is not an object
        assert_eq!(rule_read(&c, &json!({ "urgent": null, "carrier": "CARRIER_DRONE" })), json!({ "急ぎ": false, "便": "CARRIER_DRONE" }));
        assert_eq!(rule_read(&c, &json!([])), json!([]));
        assert_eq!(rule_read(&c, &json!("no")), json!("no"));
    }

    #[test]
    fn a_number_is_read_from_a_decimal_and_from_nothing_else() {
        let mut c = urgency();
        c.response = vec![WireField { name: "額".into(), json: "amount".into(), kind: WireKind::Int, optional: false }];
        c.zeros = json!({ "f": { "amount": "0" } });
        let read = |v: Value| rule_read(&c, &json!({ "amount": v }))["額"].clone();
        assert_eq!(read(json!("12000")), json!(12000));
        assert_eq!(read(json!("-3")), json!(-3));
        assert_eq!(read(json!("007")), json!(7));
        // a number the service wrote as a number is one; what is not a decimal is left for the check to refuse
        assert_eq!(read(json!(12000)), json!(12000));
        // the greatest number every platform holds as itself, and one more, which is left as the text it is
        assert_eq!(read(json!("9007199254740991")), json!(9_007_199_254_740_991_i64));
        assert_eq!(read(json!("-9007199254740991")), json!(-9_007_199_254_740_991_i64));
        assert_eq!(read(json!("0000000000000012")), json!(12));
        for bad in [json!("9007199254740992"), json!("00000000000000012"), json!("9999999999999999")] {
            assert_eq!(read(bad.clone()), bad);
        }
        for bad in [json!("+5"), json!("1.5"), json!("1e3"), json!(""), json!("-"), json!("12a"), json!(" 5"), json!("99999999999999999999"), json!(true), json!(null)] {
            let got = read(bad.clone());
            // a null is a field left out, which reads as the zero it is
            let want = if bad.is_null() { json!(0) } else { bad.clone() };
            assert_eq!(got, want, "{bad}");
        }
        // left out, the number is zero, as protobuf reads it
        assert_eq!(rule_read(&c, &json!({}))["額"], json!(0));
    }

    #[test]
    fn what_a_service_writes_is_read_back_as_the_answer_it_was() {
        let c = urgency();
        // every answer of the rule: both bools, each enum value; the zero values are left out of what the service writes
        for urgent in [true, false] {
            for carrier in ["standard", "next_day"] {
                let answer = json!({ "急ぎ": urgent, "便": carrier });
                let wire = rule_wire(&c, &answer);
                assert_eq!(wire.get("urgent").is_some(), urgent, "false is left out: {wire}");
                assert_eq!(rule_read(&c, &wire), answer, "{wire}");
            }
        }
        // a number is a string, and 0 is left out
        let mut n = urgency();
        n.response = vec![WireField { name: "額".into(), json: "amount".into(), kind: WireKind::Int, optional: false }];
        n.zeros = json!({ "f": { "amount": "0" } });
        assert_eq!(rule_wire(&n, &json!({ "額": 12000 })), json!({ "amount": "12000" }));
        assert_eq!(rule_wire(&n, &json!({ "額": 0 })), json!({}));
        for v in [0, 1, -1, 12000, 1_200_000] {
            assert_eq!(rule_read(&n, &rule_wire(&n, &json!({ "額": v }))), json!({ "額": v }));
        }
    }

    /// A rule whose enum is a contract's that puts a value of its own at 0 and gives its values no
    /// prefix (`enum Status { ACTIVE = 0; CLOSED = 1; }`): the account fee of tests/fixtures/rules.
    fn account() -> RuleConnect {
        let state = WireKind::Enum { zero: "ACTIVE".into(), values: vec![("有効".into(), "ACTIVE".into()), ("解約".into(), "CLOSED".into())] };
        let field = |name: &str, json: &str, kind: WireKind| WireField { name: name.into(), json: json.into(), kind, optional: false };
        RuleConnect {
            base: "https://rules.example.com".into(),
            url: "https://rules.example.com/rulec.account_fee.v1.AccountFeeService/Decide".into(),
            request: vec![field("状態", "state", state.clone()), field("残高", "balance", WireKind::Int)],
            response: vec![field("手数料", "fee", WireKind::Int), field("次の状態", "nextState", state)],
            zeros: json!({ "f": { "fee": "0", "nextState": "ACTIVE" } }),
        }
    }

    #[test]
    fn an_enums_value_0_that_is_the_rules_is_sent_and_read_as_any_other() {
        let c = account();
        assert_eq!(c.response[1].kind.zero_value(), Some("有効"));
        assert_eq!(urgency().response[1].kind.zero_value(), None);
        // sent by its name, as the zero values of the other kinds are: the service refuses an input left out
        assert_eq!(rule_request(&c, &args(json!({ "状態": "有効", "残高": 0 }))), json!({ "state": "ACTIVE", "balance": "0" }));
        // the service leaves it out of its answer, and it is read back as the rule's value
        assert_eq!(rule_wire(&c, &json!({ "手数料": 0, "次の状態": "有効" })), json!({}));
        assert_eq!(rule_wire(&c, &json!({ "手数料": 110, "次の状態": "解約" })), json!({ "fee": "110", "nextState": "CLOSED" }));
        assert_eq!(rule_read(&c, &json!({})), json!({ "手数料": 0, "次の状態": "有効" }));
        assert_eq!(rule_read(&c, &json!({ "nextState": null })), json!({ "手数料": 0, "次の状態": "有効" }));
        for fee in [0, 110] {
            for state in ["有効", "解約"] {
                let answer = json!({ "手数料": fee, "次の状態": state });
                assert_eq!(rule_read(&c, &rule_wire(&c, &answer)), answer);
            }
        }
        // the state machine fills it in and reads it by the same table
        let read = jsonata_rule_read(&c, "$states.result.ResponseBody");
        assert!(read.contains("\"nextState\":\"ACTIVE\""), "{read}");
        assert!(read.contains("{\"ACTIVE\":\"有効\",\"CLOSED\":\"解約\"}"), "{read}");
    }

    #[test]
    fn jsonata_writes_what_the_functions_do() {
        let c = urgency();
        // a value known now is written out as the request writes it; the rest are expressions
        let body = jsonata_rule_request(&c, &[("会員".into(), TExpr::Bool(true)), ("金額".into(), TExpr::Int(5000))]);
        assert_eq!(body, json!({ "member": true, "amount": "5000" }));
        let var = |n: &str, ty: Ty| TExpr::Var { name: "受注".into(), fields: vec![n.into()], ty };
        let body = jsonata_rule_request(&c, &[("会員".into(), var("会員", Ty::Bool)), ("金額".into(), var("金額", Ty::Num("円".into())))]);
        assert_eq!(body["member"], json!("{% $受注.`会員` %}"));
        assert_eq!(body["amount"], json!("{% $string($受注.`金額`) %}"));
        let read = jsonata_rule_read(&c, "$states.result.ResponseBody");
        // the table is asked only for the names it has of its own (not `constructor`, `toString`, `__proto__`)
        assert!(read.contains("$dd_t := {\"CARRIER_STANDARD\":\"standard\",\"CARRIER_NEXTDAY\":\"next_day\"}; $type($dd_v) = \"string\" and $dd_v in $keys($dd_t) ? $lookup($dd_t, $dd_v) : $dd_v"), "{read}");
        assert!(read.contains("\"急ぎ\": $lookup($dd_b, \"urgent\")"), "{read}");
    }
}
