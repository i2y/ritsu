//! `dandori api`: what a `.flow` that passes its check declares, as JSON, for the tools and the
//! agents that read it without reading the file. The shape follows the `api` of the other languages
//! (the tool and its version, the file, then what the file is made of), and it is built from the
//! checked program ([`Model`]) the generators read, not from any one platform's code: a task's
//! deadline, errors and retries, what it calls, the ranges of the values, the secrets, and every way
//! the workflow ends are what the program says, whichever platform it is written for.

use crate::model::*;
use crate::rulec::{Column, RType};
use ritsu_base::json::Json;

fn line(n: usize) -> Json {
    if n == 0 { Json::Null } else { Json::from(n) }
}

fn opt<T: Into<Json>>(x: Option<T>) -> Json {
    x.map(Into::into).unwrap_or(Json::Null)
}

/// The place a range is given for, as the `.flow` writes it (`text`), and as data: the first and
/// the last number, day or moment it lets in; for an ordered enum, the values it lets in.
fn range_json(m: &Model, ty: &Ty, rg: &Range) -> Json {
    let mut inner = ty;
    while let Ty::Opt(x) | Ty::List(x) = inner {
        inner = x;
    }
    let text = Json::str(m.range_text(ty, rg));
    match (inner, ty.enum_inside()) {
        (_, Some(e)) => {
            let values = m.values_in(e, Some(*rg));
            Json::obj([
                ("text", text),
                ("min", opt(values.first().cloned())),
                ("max", opt(values.last().cloned())),
                ("values", Json::arr(values.into_iter().map(Json::str))),
            ])
        }
        (Ty::Date, _) => Json::obj([("text", text), ("min", opt(rg.lo.map(day_text))), ("max", opt(rg.hi.map(day_text)))]),
        (Ty::Timestamp, _) => Json::obj([("text", text), ("min", opt(rg.lo.map(moment_text))), ("max", opt(rg.hi.map(moment_text)))]),
        _ => Json::obj([("text", text), ("min", opt(rg.lo)), ("max", opt(rg.hi))]),
    }
}

/// A number that is not a whole number, as the digits it prints with; a whole one as an integer.
fn number(x: f64) -> Json {
    if x.fract() == 0.0 && x.abs() < 1e15 { Json::Int(x as i128) } else { Json::Frac(x.to_string()) }
}

/// `file` as seen from the directory of the `.flow` (both as dandori reaches them), so that no
/// path of the place the command ran from is in the JSON: `../specs/stripe.json`.
fn beside(m: &Model, file: &str) -> String {
    let dir = std::path::Path::new(&m.source_reached).parent().unwrap_or(std::path::Path::new(""));
    std::path::Path::new(file).strip_prefix(dir).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| file.to_string())
}

fn secret_json(s: &SecretMark) -> Json {
    Json::obj([("mark", Json::str(s.mark.clone())), ("at", Json::str(s.at.clone())), ("file", Json::str(s.file.clone())), ("line", line(s.line))])
}

/// A place a value is put: its name, its type as the `.flow` writes it, its range, whether it may be
/// absent, and the secret mark on it.
fn place(m: &Model, name: &str, ty: &Ty, rg: Option<&Range>, secret: Option<&SecretMark>, at: usize) -> Vec<(&'static str, Json)> {
    vec![
        ("name", Json::str(name)),
        ("type", Json::str(m.ty_name(ty))),
        ("optional", Json::Bool(matches!(ty, Ty::Opt(_)))),
        ("range", rg.map(|r| range_json(m, ty, r)).unwrap_or(Json::Null)),
        ("secret", secret.map(secret_json).unwrap_or(Json::Null)),
        ("line", line(at)),
    ]
}

fn io(m: &Model) -> (Json, Json) {
    let side = |xs: &[(String, Ty)], ranges: &std::collections::BTreeMap<String, Range>, secrets: &std::collections::BTreeMap<String, SecretMark>, lines: &std::collections::BTreeMap<String, usize>| {
        Json::arr(xs.iter().map(|(n, t)| Json::obj(place(m, n, t, ranges.get(n), secrets.get(n), lines.get(n).copied().unwrap_or(0)))))
    };
    (side(&m.inputs, &m.input_ranges, &m.input_secrets, &m.input_lines), side(&m.outputs, &m.output_ranges, &m.output_secrets, &m.output_lines))
}

fn column_json(c: &Column) -> Json {
    let (ty, range) = match &c.ty {
        RType::Bool => ("bool".to_string(), Json::Null),
        RType::Str => ("string".to_string(), Json::Null),
        RType::Date => ("date".to_string(), Json::Null),
        RType::Enum { name, range } => (name.clone(), range.as_ref().map(|vs| Json::obj([("text", Json::str(ends_text(vs))), ("values", Json::arr(vs.iter().cloned().map(Json::str)))])).unwrap_or(Json::Null)),
        RType::Num { unit, min, max } => (unit.to_string(), if min.is_some() || max.is_some() { Json::obj([("text", Json::str(Range { lo: *min, hi: *max }.show())), ("min", opt(*min)), ("max", opt(*max))]) } else { Json::Null }),
    };
    Json::obj([("name", Json::str(c.name.clone())), ("alias", Json::str(c.alias.clone())), ("type", Json::str(ty)), ("range", range)])
}

fn error_json(e: &ErrDef) -> Json {
    Json::obj([("name", Json::str(e.name.clone())), ("status", opt(e.status.map(u32::from))), ("exception", opt(e.exception.clone()))])
}

/// The `use` lines, each with what the file it reads says: a rule's name, version, digest, inputs and
/// outputs, and how the workflow calls it (`lambda`, `local`, `connect`, `connection`); the dates a
/// dates file's calls come to; a book's transfers.
fn uses_json(m: &Model) -> Json {
    Json::arr(m.uses.iter().map(|u| {
        let mut ps: Vec<(&str, Json)> = vec![("kind", Json::str(u.kind)), ("name", Json::str(u.name.clone())), ("file", Json::str(u.path.clone())), ("line", line(u.line))];
        match u.kind {
            "rule" => {
                if let Some(r) = m.rules.iter().find(|r| r.is_rule() && r.name == u.name) {
                    ps.push(("rule", Json::str(r.info.rule.clone())));
                    ps.push(("rule_version", Json::str(r.info.version.clone())));
                    ps.push(("sha256", Json::str(r.info.sha256.clone())));
                    ps.push(("inputs", Json::arr(r.info.inputs.iter().map(column_json))));
                    ps.push(("outputs", Json::arr(r.info.outputs.iter().map(column_json))));
                    ps.push(("machine", opt(r.info.machine.as_ref().map(|mc| mc.name.clone()))));
                    ps.push(("lambda", opt(r.lambda.clone())));
                    ps.push(("local", Json::Bool(r.local)));
                    ps.push(("connect", opt(r.connect.as_ref().map(|c| c.url.clone()))));
                    ps.push(("connection", opt(r.connection.clone())));
                    ps.push(("preconditions", Json::from(r.info.preconditions.len())));
                }
                ps.push(("plaintext", opt(u.plaintext.clone())));
            }
            "dates" => {
                let dates = m.rules.iter().filter_map(|r| r.date()).filter(|d| d.file == u.name).map(|d| {
                    Json::obj([
                        ("name", Json::str(d.date.clone())),
                        ("params", Json::arr(d.params.iter().map(|(n, _, day)| Json::obj([("name", Json::str(n.clone())), ("type", Json::str(if *day { "date" } else { "int" }))])))),
                        ("at", Json::Bool(d.at)),
                        ("offset", opt(d.offset.map(i64::from))),
                    ])
                });
                ps.push(("dates", Json::arr(dates)));
            }
            "book" => {
                if let Some(b) = m.books.iter().find(|b| b.name == u.name) {
                    ps.push(("transfers", Json::arr(b.facts.transfers.iter().map(|t| Json::str(t.name.clone())))));
                    ps.push(("lambda", opt(b.lambda.clone())));
                }
            }
            _ => {
                ps.push(("url", opt(u.url.clone())));
                ps.push(("plaintext", opt(u.plaintext.clone())));
            }
        }
        Json::obj(ps)
    }))
}

fn enums_json(m: &Model) -> Json {
    Json::arr(m.enums.iter().map(|e| Json::obj([("name", Json::str(e.name.clone())), ("values", Json::arr(e.values.iter().cloned().map(Json::str))), ("ordered", Json::Bool(e.ordered)), ("from", opt(e.from.as_ref().map(|(file, name)| Json::obj([("file", Json::str(file.clone())), ("enum", Json::str(name.clone()))]))))])))
}

fn records_json(m: &Model) -> Json {
    Json::arr(m.records.iter().map(|r| {
        let origin = match &r.origin {
            RecordOrigin::Local => Json::str("local"),
            RecordOrigin::RuleOutputs(_) => Json::str("rule"),
            RecordOrigin::Hold { .. } => Json::str("hold"),
            RecordOrigin::Proto { api } => Json::str(format!("proto {api}")),
        };
        Json::obj([
            ("name", Json::str(r.name.clone())),
            ("origin", origin),
            ("fields", Json::arr(r.fields.iter().map(|(n, t)| Json::obj(place(m, n, t, r.ranges.get(n), r.secrets.get(n), 0))))),
        ])
    }))
}

/// How a task is done: its kind, and what the kind says about where.
fn task_kind(m: &Model, t: &TaskDef) -> (&'static str, Json) {
    if t.event {
        return ("event", Json::Null);
    }
    if let Some(c) = &t.flow {
        return ("workflow", Json::obj([("file", Json::str(c.path.clone())), ("workflow", Json::str(c.model.name.clone()))]));
    }
    if let Some(n) = &t.state_machine {
        return ("state_machine", Json::obj([("name", Json::str(n.clone()))]));
    }
    if let Some(n) = &t.workflow {
        return ("workflow", Json::obj([("workflow", Json::str(n.clone()))]));
    }
    if let Some(n) = &t.durable_function {
        return ("durable_function", Json::obj([("name", Json::str(n.clone()))]));
    }
    if let Some(n) = &t.argo_template {
        return ("argo_template", Json::obj([("name", Json::str(n.clone()))]));
    }
    if let Some(n) = &t.image {
        return ("image", Json::obj([("image", Json::str(n.clone()))]));
    }
    match &t.binding {
        Some(Binding::Http { method, url, form }) => {
            let kind = if t.connect.is_some() { "connect" } else { "http" };
            (kind, Json::obj([("method", Json::str(method.clone())), ("url", Json::str(url.clone())), ("form", Json::Bool(*form)), ("api", opt(t.api_file.as_deref().map(|f| beside(m, f))))]))
        }
        Some(Binding::Lambda(f)) => ("lambda", Json::obj([("function", Json::str(f.clone()))])),
        Some(Binding::Aws { service, action }) => ("aws", Json::obj([("service", Json::str(service.clone())), ("action", Json::str(action.clone()))])),
        Some(Binding::Agent { provider, instructions, model, url, effort }) => (
            "agent",
            Json::obj([("provider", Json::str(provider.name())), ("model", Json::str(model.clone())), ("url", opt(url.clone())), ("effort", opt(effort.clone())), ("instructions", Json::str(instructions.clone()))]),
        ),
        Some(Binding::Jev(j)) => (
            "jev",
            Json::obj([
                ("api", Json::str(match j.api {
                    DecisionApi::SystemOne => "systemone",
                    DecisionApi::Decisions => "decisions",
                })),
                ("url", Json::str(j.endpoint())),
                ("model", Json::str(j.model.clone())),
                ("questions", Json::arr(j.questions.iter().map(|q| {
                    Json::obj([
                        ("id", Json::str(q.id.clone())),
                        ("field", opt(q.field.clone())),
                        ("kind", Json::str(match q.kind {
                            QuestionKind::Choice => "choice",
                            QuestionKind::Score => "score",
                            QuestionKind::Noul => "noul",
                        })),
                        ("instructions", Json::str(q.instructions.clone())),
                    ])
                }))),
                ("floor", j.floor.as_ref().map(|(f, e)| Json::obj([("confidence", number(*f)), ("error", Json::str(e.clone()))])).unwrap_or(Json::Null)),
                ("refusal", opt(j.refusal_error().map(String::from))),
            ]),
        ),
        Some(Binding::Book(b)) => ("book", Json::obj([("transfer", Json::str(b.transfer.clone())), ("operation", Json::str(b.op.clone()))])),
        None => ("own", Json::Null),
    }
}

fn task_json(m: &Model, t: &TaskDef) -> Json {
    let (kind, calls) = task_kind(m, t);
    let book = t.book().map(|b| Json::str(m.books[b.book].name.clone())).unwrap_or(Json::Null);
    let machine = match &t.machine {
        None => Json::Null,
        Some(TaskMachine::Starts { rule, then }) => Json::obj([("role", Json::str("starts")), ("rule", Json::str(m.rules[*rule].machine_label())), ("then", Json::arr(then.iter().cloned().map(Json::str)))]),
        Some(TaskMachine::Sends { event, column }) => Json::obj([("role", Json::str("sends")), ("event", Json::str(event.clone())), ("column", opt(column.clone()))]),
        Some(TaskMachine::Observes) => Json::obj([("role", Json::str("observes"))]),
    };
    let params = t.params.iter().map(|(n, ty)| {
        let secret = t.param_secrets.get(n).and_then(|v| v.first()).map(|(_, s)| s);
        Json::obj(place(m, n, ty, t.param_ranges.get(n), secret, t.line))
    });
    let result = t.result.as_ref().map(|ty| {
        let mut ps = place(m, "", ty, t.result_range.as_ref(), t.result_secrets.first().map(|(_, s)| s), t.line);
        ps.remove(0);
        Json::obj(ps)
    });
    Json::obj([
        ("name", Json::str(t.name.clone())),
        ("line", line(t.line)),
        ("kind", Json::str(kind)),
        ("calls", calls),
        ("book", book),
        ("params", Json::arr(params)),
        ("result", result.unwrap_or(Json::Null)),
        ("timeout", opt(t.timeout)),
        ("deadline", opt(t.deadline.clone())),
        ("retry", t.retry.as_ref().map(|r| Json::obj([("times", Json::from(r.times)), ("every", Json::from(r.every)), ("backoff", number(r.backoff)), ("on", Json::arr(r.on.iter().cloned().map(Json::str)))])).unwrap_or(Json::Null)),
        ("key", Json::Bool(t.key)),
        ("key_param", opt(t.key_param.clone())),
        ("idempotent", Json::Bool(t.idempotent)),
        ("changes_things", Json::Bool(t.changes_things())),
        ("callback", Json::Bool(t.callback)),
        ("event", Json::Bool(t.event)),
        ("queue", opt(t.queue.clone())),
        ("connection", opt(t.connection.clone())),
        ("errors", Json::arr(t.errors.iter().map(error_json))),
        ("refused_as", opt(t.refused_as.clone())),
        ("machine", machine),
        ("discloses", Json::arr(t.discloses.iter().map(|(p, why)| Json::obj([("param", Json::str(p.clone())), ("why", Json::str(why.clone()))])))),
    ])
}

fn cases_json(m: &Model) -> Json {
    Json::arr(m.cases.iter().enumerate().map(|(ci, c)| {
        let mc = m.machine(ci);
        Json::obj([
            ("name", Json::str(c.name.clone())),
            ("line", line(c.line)),
            ("record", Json::str(m.records[c.record].name.clone())),
            ("follows", Json::str(m.rules[c.rule].machine_label())),
            ("state_field", Json::str(c.state_field.clone())),
            ("states", Json::arr(mc.states.iter().cloned().map(Json::str))),
            ("final", Json::arr(mc.finals.iter().map(|&i| Json::str(mc.states[i].clone())))),
            ("held", Json::arr(c.held_values.iter().map(|(i, v)| Json::obj([("input", Json::str(i.clone())), ("value", Json::str(v.clone()))])))),
            ("external", Json::arr(c.external.iter().map(|(_, _, n)| Json::str(n.clone())))),
            ("refused_when", c.refused_when.as_ref().map(|(ix, v)| Json::obj([("output", Json::str(mc.decides.get(*ix).cloned().unwrap_or_default())), ("value", Json::str(v.clone()))])).unwrap_or(Json::Null)),
        ])
    }))
}

fn show(e: &TExpr) -> String {
    match e {
        TExpr::Str(s) => s.clone(),
        other => other.show(),
    }
}

/// Every way the workflow ends, with the block it is in (`flow`, `on failure`, `on cancel`): the
/// `fail`s with their error name and message, and the `succeed`s with the outputs they set.
fn endings(m: &Model) -> (Json, Json) {
    let mut fails = Vec::new();
    let mut succeeds = Vec::new();
    let blocks: [(&str, Option<&Vec<TStmt>>); 3] = [("flow", Some(&m.flow)), ("on failure", m.on_failure.as_ref()), ("on cancel", m.on_cancel.as_ref())];
    for (name, block) in blocks {
        let Some(block) = block else { continue };
        Model::walk(block, &mut |s| match &s.kind {
            TK::Fail { error, cause, leaving } => fails.push(Json::obj([
                ("error", Json::str(error.clone())),
                ("message", opt(cause.as_ref().map(show))),
                ("leaving", Json::arr(leaving.iter().map(|&i| Json::str(m.cases[i].name.clone())))),
                ("in", Json::str(name)),
                ("line", line(s.line)),
            ])),
            TK::Succeed { fields } => succeeds.push(Json::obj([
                ("outputs", Json::Obj(fields.iter().map(|(n, e)| (n.clone(), Json::str(show(e)))).collect())),
                ("in", Json::str(name)),
                ("line", line(s.line)),
            ])),
            _ => {}
        });
    }
    (Json::arr(fails), Json::arr(succeeds))
}

fn service_json(m: &Model) -> Json {
    let Some(s) = &m.service else { return Json::Null };
    Json::obj([
        ("api", Json::str(s.api.clone())),
        ("file", Json::str(s.file.clone())),
        ("name", Json::str(s.name.clone())),
        ("line", line(s.line)),
        ("methods", Json::arr(s.methods.iter().map(|x| {
            let marks = x.marks.iter().map(|k| match k {
                Mark::Start { fails } => Json::obj([("mark", Json::str("start")), ("fails", Json::arr(fails.iter().cloned().map(Json::str)))]),
                Mark::Event { task } => Json::obj([("mark", Json::str("event")), ("task", Json::str(task.clone()))]),
                Mark::Answer { task } => Json::obj([("mark", Json::str("answer")), ("task", Json::str(task.clone()))]),
                Mark::Status => Json::obj([("mark", Json::str("status"))]),
            });
            Json::obj([("name", Json::str(x.name.clone())), ("request", Json::str(x.request.clone())), ("response", Json::str(x.response.clone())), ("streams", Json::Bool(x.streams)), ("marks", Json::arr(marks))])
        }))),
    ])
}

fn children_json(m: &Model) -> Json {
    Json::arr(m.tasks.iter().filter_map(|t| t.flow.as_ref().map(|c| (t, c))).map(|(t, c)| {
        let (inputs, outputs) = io(&c.model);
        Json::obj([
            ("task", Json::str(t.name.clone())),
            ("file", Json::str(c.path.clone())),
            ("workflow", Json::str(c.model.name.clone())),
            ("version", Json::from(c.model.version)),
            ("inputs", inputs),
            ("outputs", outputs),
        ])
    }))
}

/// The JSON of `dandori api` for a workflow that passes.
pub fn json(m: &Model) -> Json {
    let (inputs, outputs) = io(m);
    let (fails, succeeds) = endings(m);
    Json::obj([
        ("dandori", Json::str(env!("CARGO_PKG_VERSION"))),
        ("file", Json::str(m.source_path.clone())),
        ("workflow", Json::obj([
            ("name", Json::str(m.name.clone())),
            ("version", Json::from(m.version)),
            ("description", Json::str(m.description.clone())),
            ("sha256", Json::str(m.source_sha256.clone())),
            ("kind", Json::str(match m.kind {
                crate::syntax::Kind::Standard => "standard",
                crate::syntax::Kind::Express => "express",
            })),
        ])),
        ("implements", if m.service.is_some() { service_json(m) } else { Json::Null }),
        ("period", m.period.as_ref().map(|p| Json::obj([("text", Json::str(p.text.clone())), ("min", opt(p.range.lo.map(moment_text))), ("max", opt(p.range.hi.map(moment_text))), ("line", line(p.line))])).unwrap_or(Json::Null)),
        ("history_encrypted", opt(m.history_encrypted)),
        ("uses", uses_json(m)),
        ("inputs", inputs),
        ("outputs", outputs),
        ("enums", enums_json(m)),
        ("records", records_json(m)),
        ("tasks", Json::arr(m.tasks.iter().map(|t| task_json(m, t)))),
        ("cases", cases_json(m)),
        ("children", children_json(m)),
        ("fails", fails),
        ("succeeds", succeeds),
        ("on_failure", Json::Bool(m.on_failure.is_some())),
        ("on_cancel", opt(m.on_cancel.as_ref().map(|_| m.on_cancel_line))),
    ])
}
