//! The workflow as Python, for Temporal's Python SDK. It means what the TypeScript for
//! Temporal means (see `temporal.rs`), and it puts the same names on the wire — the workflow
//! type, the activities, the signal that brings a callback's answer, the ids of child
//! workflows and callbacks — so that a worker in either language can serve the other.
//!
//! A package named after the workflow:
//! - `types.py`: the records and enums, and a check for each (`is_<Type>`)
//! - `activities.py`: the tasks; the ones that say `lambda`, `http`, `aws` or `agent` are
//!   written there, the others (`OwnTasks`) are yours; `make_activities(own, transport)` gives
//!   the activities
//! - `io.py`: how those tasks reach Lambda, HTTP, the AWS APIs and OpenAI's agents (a `Transport`)
//! - `rules.py`: the rules as activities, around the Python rulec generates
//! - `runtime.py`: what the workflow code shares — retries, error kinds, keys, callbacks
//! - `workflow.py`: the workflow, and `workflows`, the list to give the worker
//!
//! Python has no labelled `break`, so a call with handlers is a `try` whose `except` takes
//! the task's error and whose `else` checks the answer, and a `break` is Python's own: the
//! loops of the `.flow` are the only loops in the code.

use crate::diag::Diag;
use crate::model::*;
use crate::render::{self, ident};
use serde_json::Value;
use std::collections::BTreeSet;

/// What a flow's name cannot be in the generated Python, with an `_` after it when it is:
/// the keywords, the builtins, and the names the generated code uses.
const PY_RESERVED: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else", "except", "finally", "for", "from",
    "global", "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield", "match", "case", "type", "_",
    "abs", "all", "any", "bool", "bytes", "callable", "dict", "enumerate", "Exception", "filter", "float", "format", "frozenset", "getattr", "hasattr", "hash", "id",
    "input", "int", "isinstance", "iter", "len", "list", "map", "max", "min", "next", "object", "open", "print", "range", "repr", "reversed", "round", "set", "setattr",
    "slice", "sorted", "str", "sum", "super", "tuple", "vars", "zip", "self", "T", "dd", "io", "asyncio", "timedelta", "workflow", "activity", "json", "re", "Any",
    "Literal", "NotRequired", "Protocol", "TypedDict", "ApplicationError", "RetryPolicy", "NO_RETRY", "TIMESTAMP", "is_int", "rules", "workflows", "fail", "own",
    "make_activities", "WorkflowInput", "WorkflowOutput", "OwnTasks",
];

/// A name of the flow as a Python identifier.
pub fn py_name(name: &str) -> String {
    let s = ident(name);
    if PY_RESERVED.contains(&s.as_str()) || s.starts_with("dd_") {
        format!("{s}_")
    } else {
        s
    }
}

/// The method of `OwnTasks` that runs the task.
pub fn method(task: &str) -> String {
    py_name(task)
}

/// The package the build writes: the workflow's name.
pub fn package(m: &Model) -> String {
    ident(&m.name)
}

pub(crate) fn type_name(name: &str) -> String {
    py_name(&name.replace('.', "_"))
}

/// A Python string literal (JSON's is one).
pub(crate) fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

/// The Python type of `t`, as a hint in the workflow and the tasks (`T.` for types.py).
pub(crate) fn py_type(m: &Model, t: &Ty) -> String {
    match t {
        Ty::Int | Ty::Num(_) => "int".into(),
        Ty::Str | Ty::Timestamp => "str".into(),
        Ty::Bool => "bool".into(),
        Ty::Enum(e) => format!("T.{}", type_name(&m.enums[*e].name)),
        Ty::Record(r) => format!("T.{}", type_name(&m.records[*r].name)),
        Ty::List(t) => format!("list[{}]", py_type(m, t)),
        Ty::Opt(t) => format!("{} | None", py_type(m, t)),
        Ty::Json => "Any".into(),
    }
}

/// A check that `x` is a well-formed value of `t`. `p` is how the code reaches types.py: ""
/// inside it, "T." from the workflow.
pub(crate) fn py_check(m: &Model, x: &str, t: &Ty, p: &str, depth: usize) -> String {
    match t {
        Ty::Int | Ty::Num(_) => format!("{p}is_int({x})"),
        Ty::Str => format!("isinstance({x}, str)"),
        Ty::Timestamp => format!("(isinstance({x}, str) and {p}TIMESTAMP.match({x}) is not None)"),
        Ty::Bool => format!("isinstance({x}, bool)"),
        Ty::Enum(e) => format!("({x} in {p}{}_values)", type_name(&m.enums[*e].name)),
        Ty::Record(r) => format!("{p}is_{}({x})", type_name(&m.records[*r].name)),
        Ty::List(inner) => {
            let v = format!("dd_v{depth}");
            format!("(isinstance({x}, list) and all({} for {v} in {x}))", py_check(m, &v, inner, p, depth + 1))
        }
        Ty::Opt(inner) => format!("({x} is None or {})", py_check(m, x, inner, p, depth)),
        Ty::Json => "True".into(),
    }
}

/// The checks of the fields of a dict `v`: an absent field reads as None; a `json` field must be there.
fn field_checks(m: &Model, v: &str, fields: &[(String, Ty)], p: &str) -> Vec<String> {
    let mut conds = vec![format!("isinstance({v}, dict)")];
    for (f, ft) in fields {
        match ft {
            Ty::Json => conds.push(format!("{} in {v}", q(f))),
            _ => conds.push(py_check(m, &format!("{v}.get({})", q(f)), ft, p, 0)),
        }
    }
    conds
}

pub(crate) fn types_file(m: &Model, header: &str) -> String {
    let (enums, recs) = crate::temporal::used_types(m);
    let mut t = header.to_string();
    t.push_str(&format!("# The records and enums of {} v{}, and a check for each.\n\n", m.name, m.version));
    t.push_str("from __future__ import annotations\n\nimport re\nfrom typing import Any, Literal, NotRequired, TypedDict\n\n");
    t.push_str(&format!("TIMESTAMP = re.compile(r\"{}\")\n\n\n", render::TIMESTAMP_RE.replace("\\\\", "\\")));
    t.push_str("def is_int(v: Any) -> bool:\n    \"\"\"An integer, as JSON carries it: a float with nothing after the point is one too.\"\"\"\n");
    t.push_str("    return (isinstance(v, int) and not isinstance(v, bool)) or (isinstance(v, float) and v.is_integer())\n\n\n");
    for e in &enums {
        let en = &m.enums[*e];
        let n = type_name(&en.name);
        let vals: Vec<String> = en.values.iter().map(|v| q(v)).collect();
        t.push_str(&format!("{n}_values: tuple[str, ...] = ({}{})\n", vals.join(", "), if vals.len() == 1 { "," } else { "" }));
        t.push_str(&format!("{n} = Literal[{}]\n\n", vals.join(", ")));
    }
    // the types as strings, so that a record may name one defined after it
    let hint = |m: &Model, ty: &Ty| py_type(m, ty).replace("T.", "");
    for r in &recs {
        let rd = &m.records[*r];
        let n = type_name(&rd.name);
        let fields: Vec<String> = rd
            .fields
            .iter()
            .map(|(f, ft)| match ft {
                Ty::Opt(_) => format!("{}: \"NotRequired[{}]\"", q(f), hint(m, ft)),
                _ => format!("{}: \"{}\"", q(f), hint(m, ft)),
            })
            .collect();
        t.push_str(&format!("\n{n} = TypedDict({}, {{{}}})\n\n\n", q(&n), fields.join(", ")));
        t.push_str(&format!("def is_{n}(v: Any) -> bool:\n    return (\n        {}\n    )\n\n", field_checks(m, "v", &rd.fields, "").join("\n        and ")));
    }
    let inputs: Vec<String> = m.inputs.iter().map(|(n, ty)| format!("{}: \"{}\"", q(n), hint(m, ty))).collect();
    t.push_str(&format!("\nWorkflowInput = TypedDict(\"WorkflowInput\", {{{}}})\n\n\n", inputs.join(", ")));
    t.push_str(&format!("def is_WorkflowInput(v: Any) -> bool:\n    return (\n        {}\n    )\n\n", field_checks(m, "v", &m.inputs, "").join("\n        and ")));
    let outputs: Vec<String> = m
        .outputs
        .iter()
        .map(|(n, ty)| match ty {
            Ty::Opt(_) => format!("{}: \"NotRequired[{}]\"", q(n), hint(m, ty)),
            _ => format!("{}: \"{}\"", q(n), hint(m, ty)),
        })
        .collect();
    t.push_str(&format!("\nWorkflowOutput = TypedDict(\"WorkflowOutput\", {{{}}})\n", outputs.join(", ")));
    t
}

pub fn build(m: &Model) -> Result<Vec<(String, String)>, Vec<Diag>> {
    let dir = package(m);
    let header = format!("# Code generated by dandori from {}. DO NOT EDIT.\n", m.source_file);
    let called: BTreeSet<usize> = m.all_stmts().iter().filter_map(|s| match &s.kind { TK::Call { callee: Callee::Rule(r), .. } => Some(*r), _ => None }).collect();
    let mut g = Gen { m, out: String::new(), loops: vec![] };
    let wf = g.workflow(&header, !called.is_empty());
    let mut files = vec![
        (format!("{dir}/__init__.py"), format!("{header}# {} v{}: the workflow for Temporal's Python SDK. See workflow.py.\n", m.name, m.version)),
        (format!("{dir}/types.py"), types_file(m, &header)),
        (format!("{dir}/activities.py"), tasks_file(m, &header)),
        (format!("{dir}/io.py"), format!("{header}{IO}")),
        (format!("{dir}/runtime.py"), format!("{header}{RUNTIME}")),
        (format!("{dir}/workflow.py"), wf),
    ];
    if !called.is_empty() {
        files.push((format!("{dir}/rules.py"), rules_file(m, &header, &called, true)));
    }
    Ok(files)
}

/// What a task's code gets, as a docstring: its arguments, the key, a callback's id, and the answer.
pub(crate) fn task_doc(m: &Model, task: &TaskDef) -> String {
    let mut params: Vec<String> = task.params.iter().map(|(p, pt)| format!("{}: {}", q(p), py_type(m, pt))).collect();
    if task.key {
        params.push("\"idempotency_key\": str".into());
    }
    if task.callback {
        params.push("\"callback_id\": str".into());
    }
    let answer = if task.callback {
        "nothing; the answer comes to the callback".to_string()
    } else {
        match &task.result {
            Some(t) => py_type(m, t),
            None => "anything; the workflow does not read it".into(),
        }
    };
    format!("args: {{{}}}. Answers {answer}.", params.join(", "))
}

/// The body of the function that runs a task, as lines: the user's own (`own`), or the call
/// that dandori writes for `lambda`, `http`, `aws` and `agent` through the Transport `t`. A
/// declared error is raised as `fail(kind, message)`.
pub(crate) fn task_impl(task: &TaskDef, p: Platform) -> Vec<String> {
    let fname = method(&task.name);
    match task.via(p) {
        Some(Via::Own) => vec![format!("return await own.{fname}(args)")],
        Some(Via::Lambda(f)) => {
            let names = task.errors.iter().map(|e| format!("{}: {}", q(&e.name), q(&e.name))).collect::<Vec<_>>().join(", ");
            let call = format!("io.value(await t.lambda_({}, args), {{{names}}}, fail)", q(f));
            if task.callback {
                vec![call, "return None".into()]
            } else {
                vec![format!("return {call}")]
            }
        }
        Some(Via::Http { method: hm, url, form }) => {
            let used = crate::lower::placeholders(url);
            let mut url_parts: Vec<String> = Vec::new();
            let mut rest = url;
            while let Some(i) = rest.find('{') {
                let j = match rest[i..].find('}') {
                    Some(j) => i + j,
                    None => break,
                };
                if i > 0 {
                    url_parts.push(q(&rest[..i]));
                }
                url_parts.push(format!("io.text(args.get({}))", q(&rest[i + 1..j])));
                rest = &rest[j + 1..];
            }
            if !rest.is_empty() || url_parts.is_empty() {
                url_parts.push(q(rest));
            }
            let mut req = vec![format!("\"http\": {}", q(hm)), format!("\"url\": {}", url_parts.join(" + "))];
            let mut headers = Vec::new();
            if form {
                headers.push("\"Content-Type\": \"application/x-www-form-urlencoded\"".to_string());
            }
            if task.key {
                headers.push("\"Idempotency-Key\": args[\"idempotency_key\"]".to_string());
            }
            if !headers.is_empty() {
                req.push(format!("\"headers\": {{{}}}", headers.join(", ")));
            }
            let rest_args: Vec<String> = task.params.iter().filter(|(p, _)| !used.contains(p)).map(|(p, _)| format!("{}: args.get({})", q(p), q(p))).collect();
            if !rest_args.is_empty() {
                let place = if hm == "GET" || hm == "DELETE" { "query" } else { "body" };
                req.push(format!("\"{place}\": {{{}}}", rest_args.join(", ")));
            }
            if form {
                req.push("\"form\": True".into());
            }
            let statuses = task.errors.iter().filter_map(|e| e.status.map(|s| format!("{}: {}", q(&s.to_string()), q(&e.name)))).collect::<Vec<_>>().join(", ");
            vec![format!("return io.status(await t.http({{{}}}), {{{statuses}}}, fail)", req.join(", "))]
        }
        Some(Via::Aws { service, action }) => {
            let names = task.errors.iter().map(|e| format!("{}: {}", q(e.exception.as_deref().unwrap_or(&e.name)), q(&e.name))).collect::<Vec<_>>().join(", ");
            let mut input: Vec<String> = task
                .params
                .iter()
                .map(|(p, _)| {
                    if task.callback && p == "MessageBody" {
                        format!("{}: {{**args[{}], \"callback_id\": args[\"callback_id\"]}}", q(p), q(p))
                    } else {
                        format!("{}: args.get({})", q(p), q(p))
                    }
                })
                .collect();
            if let (true, Some(kp)) = (task.key, &task.key_param) {
                input.push(format!("{}: args[\"idempotency_key\"]", q(kp)));
            }
            let call = format!("io.value(await t.aws({}, {}, {{{}}}), {{{names}}}, fail)", q(service), q(action), input.join(", "));
            if task.callback {
                vec![call, "return None".into()]
            } else {
                vec![format!("return {call}")]
            }
        }
        Some(Via::Agent { instructions, model }) => {
            let input: Vec<String> = task.params.iter().map(|(p, _)| format!("{}: args.get({})", q(p), q(p))).collect();
            vec![
                "return io.answer(".into(),
                "    await t.agent(".into(),
                "        {".into(),
                format!("            \"agent\": {},", q(&task.name)),
                format!("            \"model\": {},", q(model)),
                format!("            \"instructions\": {},", q(instructions)),
                format!("            \"input\": {{{}}},", input.join(", ")),
                format!("            \"schema\": SCHEMAS[{}],", q(&task.name)),
                "        }".into(),
                "    )".into(),
                ")".into(),
            ]
        }
        _ => vec!["raise NotImplementedError".into()],
    }
}

/// What each agent answers in, as a Python dict of the JSON Schemas: nothing when no task is an agent.
pub(crate) fn schemas_block(m: &Model, tasks: &[&TaskDef], p: Platform) -> String {
    let agents: Vec<&&TaskDef> = tasks.iter().filter(|t| matches!(t.via(p), Some(Via::Agent { .. }))).collect();
    if agents.is_empty() {
        return String::new();
    }
    let mut out = String::from("# What each agent answers in: the JSON Schema OpenAI's Structured Outputs hold its model to.\nSCHEMAS: dict[str, Any] = {\n");
    for task in agents {
        let schema = task.result.as_ref().and_then(|t| render::agent_schema(m, t)).expect("the checker gives an agent an answer with a schema");
        out.push_str(&format!("    {}: {},\n", q(&task.name), render::layout(&schema, 1, "    ", true)));
    }
    out.push_str("}\n\n\n");
    out
}

/// activities.py: every task the workflow calls as an activity, the ones the user writes, and
/// the code for the others.
fn tasks_file(m: &Model, header: &str) -> String {
    let p = Platform::Temporal;
    let tasks: Vec<&TaskDef> = m.tasks.iter().filter(|t| !t.is_child(p)).collect();
    let own: Vec<&TaskDef> = tasks.iter().filter(|t| matches!(t.via(p), Some(Via::Own))).cloned().collect();
    let mut a = header.to_string();
    a.push_str("# The tasks the workflow calls.\n#\n");
    a.push_str("# - A task that says `lambda`, `http` or `aws` is written here: it sends what Step Functions\n");
    a.push_str("#   would send, through a Transport (io.py), where the credentials and the clients are yours to set.\n");
    a.push_str("# - So is a task that says `agent`: the model gets the arguments as JSON text, as from Step\n");
    a.push_str("#   Functions, and answers {\"answer\": …} in the JSON Schema below; the Transport runs the agent.\n");
    a.push_str("# - The others are yours to write (OwnTasks). A declared error is raised as\n");
    a.push_str("#   ApplicationError(\"...\", type=\"<error>\", non_retryable=True).\n");
    a.push_str("# - The workflow retries by itself, as the `retry` of each task says; the platform does not.\n");
    a.push_str("# - A task with `key` gets `idempotency_key`: pass it on to the other side as it is.\n");
    a.push_str("# - A `callback` task gets `callback_id` and returns once it has handed the id on. The answer comes\n");
    a.push_str("#   as the signal `dandori.callback`, with {\"callback_id\", \"ok\"} or {\"callback_id\", \"error\", \"message\"},\n");
    a.push_str("#   to the workflow `io.workflow_of(callback_id)` names.\n\n");
    a.push_str("from __future__ import annotations\n\nfrom typing import Any, Protocol\n\nfrom temporalio import activity\nfrom temporalio.exceptions import ApplicationError\n\n");
    a.push_str("from . import io\nfrom . import types as T\n\n\n");
    a.push_str(&schemas_block(m, &tasks, p));
    a.push_str("class OwnTasks(Protocol):\n    \"\"\"The tasks you write: the ones that say neither `lambda`, `http`, `aws` nor `agent`.\"\"\"\n");
    if own.is_empty() {
        a.push('\n');
    }
    for task in &own {
        a.push_str(&format!("\n    async def {}(self, args: dict[str, Any]) -> Any:\n        \"\"\"{}\"\"\"\n        ...\n", method(&task.name), task_doc(m, task)));
    }
    a.push_str("\n\ndef fail(kind: str, message: str) -> ApplicationError:\n    return ApplicationError(message, type=kind, non_retryable=True)\n\n\n");
    a.push_str("def make_activities(own: OwnTasks, transport: io.Transport | None = None) -> list[Any]:\n");
    a.push_str("    \"\"\"Your tasks and the ones dandori writes: the activities to register with the worker.\"\"\"\n");
    a.push_str("    t = transport if transport is not None else io.transport()\n");
    let mut names = Vec::new();
    for task in &tasks {
        let fname = method(&task.name);
        names.push(fname.clone());
        a.push_str(&format!("\n    @activity.defn(name={})\n    async def {fname}(args: dict[str, Any]) -> Any:\n        \"\"\"{}\"\"\"\n", q(&ident(&task.name)), task_doc(m, task)));
        for l in task_impl(task, p) {
            a.push_str(&format!("        {l}\n"));
        }
    }
    a.push_str(&format!("\n    return [{}]\n", names.join(", ")));
    a
}

/// rules.py: every rule the flow calls, around the Python rulec generates; as Temporal
/// activities, or as plain functions.
pub(crate) fn rules_file(m: &Model, header: &str, called: &BTreeSet<usize>, activities: bool) -> String {
    let mut t = header.to_string();
    if activities {
        t.push_str("# The rules the workflow calls, as activities around the Python rulec generates.\n");
    } else {
        t.push_str("# The rules the graph calls, as functions around the Python rulec generates.\n");
    }
    t.push_str("# `rulec gen <rule> --out <this package>/rulec` writes the modules these imports read.\n\n");
    t.push_str("from __future__ import annotations\n\nfrom typing import Any\n\n");
    if activities {
        t.push_str("from temporalio import activity\n\n");
    }
    for r in called {
        let py = &m.rules[*r].info.api["python"];
        let module = py["module"].as_str().unwrap_or("rule");
        let mut names: BTreeSet<String> = BTreeSet::new();
        names.insert(py["function"].as_str().unwrap_or("rule").to_string());
        let enums: Vec<String> = py["enums"].as_array().map(|a| a.iter().map(|e| e["alias"].as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
        for p in py["params"].as_array().unwrap_or(&vec![]) {
            let ty = p["type"].as_str().unwrap_or("");
            if enums.iter().any(|e| e == ty) {
                names.insert(ty.to_string());
            }
        }
        t.push_str(&format!("from .rulec.python.{module} import {}\n", names.into_iter().collect::<Vec<_>>().join(", ")));
    }
    let mut acts = Vec::new();
    for r in called {
        let ru = &m.rules[*r];
        let py = &ru.info.api["python"];
        let enums: Vec<String> = py["enums"].as_array().map(|a| a.iter().map(|e| e["alias"].as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
        let is_enum = |ty: &str| enums.iter().any(|a| a == ty);
        let mut args = Vec::new();
        for p in py["params"].as_array().unwrap_or(&vec![]) {
            let name = p["name"].as_str().unwrap_or("");
            let ty = p["type"].as_str().unwrap_or("");
            let get = format!("args[{}]", q(name));
            if is_enum(ty) {
                args.push(format!("{ty}({get})"));
            } else if ty == "bool" {
                args.push(format!("bool({get})"));
            } else if ty == "str" {
                args.push(format!("str({get})"));
            } else {
                args.push(format!("int({get})"));
            }
        }
        let mut outs = Vec::new();
        for o in py["outputs"].as_array().unwrap_or(&vec![]) {
            let name = o["name"].as_str().unwrap_or("");
            let alias = o["alias"].as_str().unwrap_or("");
            let ty = o["type"].as_str().unwrap_or("");
            let v = if is_enum(ty) {
                format!("out.{alias}.value")
            } else if ty == "bool" || ty == "str" {
                format!("out.{alias}")
            } else {
                format!("int(out.{alias})")
            };
            outs.push(format!("        {}: {v},", q(name)));
        }
        let act = render::rule_activity(&ru.name);
        let decorator = if activities { format!("@activity.defn(name={})\n", q(&act)) } else { String::new() };
        t.push_str(&format!(
            "\n\n{decorator}async def {act}(args: dict[str, Any]) -> dict[str, Any]:\n    \"\"\"The rule {} v{}.\"\"\"\n    out = {}({})\n    return {{\n{}\n    }}\n",
            ru.info.rule,
            ru.info.version,
            py["function"].as_str().unwrap_or("rule"),
            args.join(", "),
            outs.join("\n")
        ));
        acts.push(act);
    }
    t.push_str(&format!("\n\nrules = [{}]\n", acts.join(", ")));
    t
}

pub(crate) const IO: &str = r#"# How the tasks that say `lambda`, `http`, `aws` or `agent` reach the other side. They go
# through a Transport, so that the credentials, the clients and a test's stand-in are yours to
# set; the default one uses the standard library for HTTP, boto3 (the AWS SDK for Python) for
# Lambda and the AWS APIs, and OpenAI's Agents SDK (openai-agents, which reads OPENAI_API_KEY)
# for the agents, each imported when first needed.

from __future__ import annotations

import asyncio
import json
import re
import urllib.error
import urllib.parse
import urllib.request
from typing import Any, Callable, Protocol

# What Lambda or an AWS API answered: {"ok": value}, or {"error": name, "message": text} with
# the name the other side gives the error.
Answer = dict

# An HTTP request as Step Functions' HTTP Task sends it: {"http": method, "url", "headers",
# "body", "query", "form"}; `form` asks for a URL-encoded body.
HttpRequest = dict

# A call of an agent: {"agent": the task's name, "model", "instructions", "input", "schema"}.
# The model is told `instructions`, reads `input` as JSON text, and answers in `schema`, the
# JSON Schema of {"answer": …} in the strict form of OpenAI's Structured Outputs.
AgentCall = dict


class Transport(Protocol):
    async def lambda_(self, fn: str, payload: dict[str, Any]) -> Answer:
        """Invoke a Lambda function; an error the function raises comes back by its type."""
        ...

    async def http(self, req: HttpRequest) -> dict[str, Any]:
        """Send an HTTP request; the answer is {"status", "body"}, the body parsed when it is JSON."""
        ...

    async def aws(self, service: str, action: str, input: dict[str, Any]) -> Answer:
        """Call an AWS API, named as Step Functions names it (`sns`, `publish`); an exception comes back by its name."""
        ...

    async def agent(self, call: AgentCall) -> Any:
        """Run an agent once; the answer is the JSON it gave, as `schema` says. A refusal raises."""
        ...


# boto3's name of a service, where it is not the one Step Functions uses
SERVICES = {"bedrockruntime": "bedrock-runtime", "eventbridge": "events", "sfn": "stepfunctions"}


def text(v: Any) -> str:
    """A value put into a URL or a string: text as it is, nothing for none, anything else as JSON."""
    if isinstance(v, str):
        return v
    if v is None:
        return ""
    return json.dumps(v, ensure_ascii=False, separators=(",", ":"))


def encode(o: dict[str, Any], prefix: str = "") -> str:
    """`a=1&b[c]=2`, the way a URL-encoded body nests."""
    parts = []
    for k, v in o.items():
        key = f"{prefix}[{k}]" if prefix else str(k)
        if isinstance(v, dict):
            parts.append(encode(v, key))
        else:
            parts.append(f"{urllib.parse.quote(key, safe='')}={urllib.parse.quote('' if v is None else text(v), safe='')}")
    return "&".join(p for p in parts if p)


class DefaultTransport:
    def __init__(self, headers: Callable[[str], dict[str, str]] | None = None, aws: dict[str, Any] | None = None, agents: Any = None) -> None:
        """`headers`: what to add to an HTTP request, such as the credentials the other side
        wants; `aws`: the keyword arguments of boto3's clients, such as `region_name`;
        `agents`: the Agents SDK's RunConfig, such as one whose model_provider serves models
        other than OpenAI's."""
        self._headers = headers
        self._aws = aws or {}
        self._agents = agents
        self._clients: dict[str, Any] = {}

    def _client(self, service: str) -> Any:
        if service not in self._clients:
            import boto3

            self._clients[service] = boto3.client(SERVICES.get(service, service), **self._aws)
        return self._clients[service]

    async def lambda_(self, fn: str, payload: dict[str, Any]) -> Answer:
        def call() -> Answer:
            out = self._client("lambda").invoke(FunctionName=fn, Payload=json.dumps(payload).encode())
            raw = out["Payload"].read()
            body = json.loads(raw) if raw else None
            if out.get("FunctionError"):
                return {"error": str((body or {}).get("errorType", out["FunctionError"])), "message": str((body or {}).get("errorMessage", ""))}
            return {"ok": body}

        return await asyncio.to_thread(call)

    async def http(self, req: HttpRequest) -> dict[str, Any]:
        def call() -> dict[str, Any]:
            url = req["url"]
            if req.get("query") is not None:
                url += ("&" if "?" in url else "?") + encode(req["query"])
            headers = dict(req.get("headers") or {})
            if self._headers is not None:
                headers.update(self._headers(req["url"]))
            data = None
            if req.get("body") is not None:
                if req.get("form"):
                    data = encode(req["body"]).encode()
                else:
                    data = json.dumps(req["body"]).encode()
                    headers.setdefault("Content-Type", "application/json")
            r = urllib.request.Request(url, data=data, method=req["http"], headers=headers)
            try:
                with urllib.request.urlopen(r) as res:
                    status, ctype, raw = res.status, res.headers.get("content-type") or "", res.read()
            except urllib.error.HTTPError as e:
                status, ctype, raw = e.code, e.headers.get("content-type") or "", e.read()
            body: Any = raw.decode("utf-8", "replace")
            if "json" in ctype:
                try:
                    body = json.loads(body)
                except ValueError:
                    pass
            return {"status": status, "body": body}

        return await asyncio.to_thread(call)

    async def aws(self, service: str, action: str, input: dict[str, Any]) -> Answer:
        def call() -> Answer:
            from botocore.exceptions import ClientError

            args = dict(input)
            # SQS takes the message as text; Step Functions writes a JSON body out the same way
            if service == "sqs" and action == "sendMessage" and isinstance(args.get("MessageBody"), (dict, list)):
                args["MessageBody"] = json.dumps(args["MessageBody"])
            method = re.sub(r"(?<!^)(?=[A-Z])", "_", action).lower()
            try:
                out = getattr(self._client(service), method)(**args)
            except ClientError as e:
                err = e.response.get("Error", {})
                return {"error": str(err.get("Code", "")), "message": str(err.get("Message", ""))}
            out = dict(out or {})
            out.pop("ResponseMetadata", None)
            return {"ok": out}

        return await asyncio.to_thread(call)

    async def agent(self, call: AgentCall) -> Any:
        from agents import Agent, AgentOutputSchemaBase, ModelBehaviorError, ModelSettings, Runner

        class Answer(AgentOutputSchemaBase):
            def is_plain_text(self) -> bool:
                return False

            def name(self) -> str:
                return "answer"

            def json_schema(self) -> dict[str, Any]:
                return call["schema"]

            def is_strict_json_schema(self) -> bool:
                return True

            def validate_json(self, json_str: str) -> Any:
                try:
                    return json.loads(json_str)
                except ValueError as e:
                    raise ModelBehaviorError(f"the agent's answer is not JSON: {e}") from e

        # no model settings of the SDK's own, so that the model gets what Step Functions sends
        agent = Agent(name=call["agent"], instructions=call["instructions"], model=call["model"], model_settings=ModelSettings(), output_type=Answer())
        result = await Runner.run(agent, text(call["input"]), run_config=self._agents)
        return result.final_output


def transport(headers: Callable[[str], dict[str, str]] | None = None, aws: dict[str, Any] | None = None, agents: Any = None) -> Transport:
    return DefaultTransport(headers, aws, agents)


def workflow_of(callback_id: str) -> str:
    """The workflow a callback's answer goes to, from the id the callback task was given."""
    return json.loads(callback_id)[0]


def value(a: Answer, names: dict[str, str], fail: Callable[[str, str], Exception]) -> Any:
    """The value of an answer, or the declared error it names (`names`: the other side's name → the error)."""
    if "ok" in a:
        return a["ok"]
    kind = names.get(a["error"])
    raise fail(kind if kind is not None else f"Dandori.Failure.{a['error']}", a.get("message", ""))


def answer(out: Any) -> Any:
    """What an agent answered: the value under "answer". An answer without it fails the call."""
    if isinstance(out, dict) and "answer" in out:
        return out["answer"]
    raise ValueError("the agent's answer has no \"answer\"")


def status(r: dict[str, Any], names: dict[str, str], fail: Callable[[str, str], Exception]) -> Any:
    """The body of a 2xx answer, or the declared error its status names."""
    if 200 <= r["status"] < 300:
        return r["body"]
    kind = names.get(str(r["status"]))
    body = r["body"]
    raise fail(kind if kind is not None else f"Dandori.HttpStatus.{r['status']}", body if isinstance(body, str) else json.dumps(body))
"#;

const RUNTIME: &str = r#"# What the generated workflow code shares. It runs inside the workflow, so it uses nothing
# but temporalio.workflow and what the workflow sandbox lets through.

from __future__ import annotations

import asyncio
import itertools
import json
from datetime import datetime, timedelta
from typing import Any, Awaitable, Callable, Sequence

from temporalio import workflow
from temporalio.exceptions import ActivityError, ApplicationError, ChildWorkflowError
from temporalio.exceptions import TimeoutError as TemporalTimeout


class TaskError(Exception):
    """A task that failed after its retries, with the kind of error the `.flow` names it by."""

    def __init__(self, kind: str, message: str) -> None:
        super().__init__(message)
        self.kind = kind
        self.message = message


class CallbackTimeout(Exception):
    """A callback that got no answer in time."""

    def __init__(self) -> None:
        super().__init__("no answer in time")
        self.message = "no answer in time"


class CallbackError(Exception):
    """A callback answered with an error."""

    def __init__(self, kind: str, message: str) -> None:
        super().__init__(message)
        self.kind = kind
        self.message = message


def kind_of(e: BaseException, declared: Sequence[str]) -> str:
    """The kind of an error from an activity, a child workflow or a callback: a declared error, "timeout", or "failure"."""
    if isinstance(e, CallbackTimeout):
        return "timeout"
    if isinstance(e, CallbackError):
        return e.kind if e.kind in declared else "failure"
    if isinstance(e, (ActivityError, ChildWorkflowError)):
        c = e.cause
        if isinstance(c, TemporalTimeout):
            return "timeout"
        if isinstance(c, ApplicationError) and c.type and c.type in declared:
            return c.type
    return "failure"


def message_of(e: BaseException) -> str:
    """The error's message as the other side sent it; the SDK reads an empty one as "Application error"."""
    c = e.cause if isinstance(e, (ActivityError, ChildWorkflowError)) and e.cause is not None else e
    raw = getattr(c, "failure", None)
    if raw is not None:
        return raw.message
    return str(getattr(c, "message", c))


async def attempt(call: Callable[[], Awaitable[Any]], retriers: Sequence[dict[str, Any]], declared: Sequence[str]) -> Any:
    """Call once, and again as the retriers say, waiting between as Step Functions would."""
    counts = [0] * len(retriers)
    while True:
        try:
            return await call()
        except (ActivityError, ChildWorkflowError, CallbackTimeout, CallbackError) as e:
            kind = kind_of(e, declared)
            again = False
            for i, r in enumerate(retriers):
                if kind in r["on"] or "*" in r["on"]:
                    if counts[i] < r["max"]:
                        await asyncio.sleep(r["every"] * r["backoff"] ** counts[i])
                        counts[i] += 1
                        again = True
                    break
            if not again:
                raise TaskError(kind, message_of(e)) from None


def key(site: int, rounds: Sequence[int]) -> str:
    """The idempotency key of a call: the workflow, the call's place, and the round of each loop around it."""
    return "/".join([workflow.info().workflow_id, str(site), *(str(r) for r in rounds)])


def child_id(site: int, rounds: Sequence[int], n: int) -> str:
    """The workflow id of a child workflow: this workflow's, the call's place, the rounds, and the try."""
    return "/".join([workflow.info().workflow_id, str(site), *(str(r) for r in rounds), str(n)])


def counter() -> Any:
    """The tries of one call, counted from 1."""
    return itertools.count(1)


def callback_id(site: int, rounds: Sequence[int], n: int) -> str:
    """The id a callback task hands on: which workflow to signal, and which call it answers."""
    return json.dumps([workflow.info().workflow_id, "/".join([str(site), *(str(r) for r in rounds), str(n)])], ensure_ascii=False, separators=(",", ":"))


async def await_callback(answers: dict[str, dict[str, Any]], id: str, seconds: int) -> Any:
    """Wait for the answer of the callback with this id, which the signal puts in `answers`."""
    try:
        await workflow.wait_condition(lambda: id in answers, timeout=timedelta(seconds=seconds))
    except asyncio.TimeoutError:
        raise CallbackTimeout() from None
    a = answers.pop(id)
    if isinstance(a.get("error"), str):
        raise CallbackError(a["error"], a.get("message") or "")
    return a.get("ok")


def at_a_time(k: int) -> int:
    """How many rounds of `for … in parallel` run at a time: all, when the `.flow` does not say (0)."""
    return k


async def rounds(items: Sequence[Any], k: int, round: Callable[[Any, int], Awaitable[Any]]) -> list[Any]:
    """Run a round for every item, `k` at a time (0: all at once). Every round runs to its end;
    then what they yield comes back in the list's order, or the first failure by place in the
    list is raised again."""
    out: list[Any] = [None] * len(items)
    failed: list[BaseException | None] = [None] * len(items)
    taken = itertools.count()
    lanes = min(k, len(items)) if k > 0 else len(items)

    async def lane() -> None:
        while True:
            i = next(taken)
            if i >= len(items):
                return
            try:
                out[i] = await round(items[i], i)
            except (TaskError, ApplicationError) as e:
                failed[i] = e

    await asyncio.gather(*(lane() for _ in range(lanes)))
    for e in failed:
        if e is not None:
            raise e
    return out


def text(v: Any) -> str:
    """A value put into a string: text as it is, nothing for none, anything else as JSON."""
    if isinstance(v, str):
        return v
    if v is None:
        return ""
    return json.dumps(v, ensure_ascii=False, separators=(",", ":"))


async def wait_until(at: str) -> None:
    """Wait until a moment given as an RFC 3339 string in UTC, by the workflow's clock."""
    seconds = (datetime.fromisoformat(at.replace("Z", "+00:00")) - workflow.now()).total_seconds()
    if seconds > 0:
        await asyncio.sleep(seconds)


def fail(error: str, cause: str | None) -> ApplicationError:
    """A deliberate end of the workflow as failed."""
    return ApplicationError(cause or "", type=error, non_retryable=True)
"#;

/// The retriers of a call, as the list `dd.attempt` takes: the ASL's, by the `.flow`'s kinds.
pub(crate) fn retriers(m: &Model, callee: &Callee) -> String {
    let v: Value = match callee {
        Callee::Rule(_) => serde_json::json!([
            { "ErrorEquals": ["States.Timeout"], "MaxAttempts": 0 },
            { "ErrorEquals": ["States.ALL"], "IntervalSeconds": crate::asl::RULE_RETRY_INTERVAL, "MaxAttempts": crate::check::RULE_RETRIES, "BackoffRate": crate::asl::RULE_RETRY_BACKOFF }
        ]),
        Callee::Task(t) => match &m.tasks[*t].retry {
            Some(r) => crate::asl::retriers(m, callee, &m.tasks[*t], r),
            None => serde_json::json!([]),
        },
    };
    let mut out = Vec::new();
    for r in v.as_array().unwrap() {
        let kinds: Vec<String> = r["ErrorEquals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| {
                let n = x.as_str().unwrap();
                if n == "States.ALL" {
                    "*".to_string()
                } else if n == "States.Timeout" {
                    "timeout".to_string()
                } else {
                    match callee {
                        Callee::Task(t) => m.tasks[*t]
                            .errors
                            .iter()
                            .find(|e| render::asl_error(m, callee, &HErr::Declared(e.name.clone())).iter().any(|a| a == n))
                            .map(|e| e.name.clone())
                            .unwrap_or_else(|| n.to_string()),
                        Callee::Rule(_) => n.to_string(),
                    }
                }
            })
            .collect();
        let backoff = r["BackoffRate"].as_f64().unwrap_or(2.0);
        out.push(format!(
            "{{\"on\": [{}], \"max\": {}, \"every\": {}, \"backoff\": {}}}",
            kinds.iter().map(|k| q(k)).collect::<Vec<_>>().join(", "),
            r["MaxAttempts"].as_u64().unwrap_or(3),
            r["IntervalSeconds"].as_u64().unwrap_or(1),
            if backoff.fract() == 0.0 { format!("{backoff:.1}") } else { backoff.to_string() }
        ));
    }
    format!("[{}]", out.join(", "))
}

struct Gen<'a> {
    m: &'a Model,
    out: String,
    /// the counters of the loops around the current statement
    loops: Vec<usize>,
}

impl<'a> Gen<'a> {
    fn line(&mut self, depth: usize, s: &str) {
        if !s.is_empty() {
            for _ in 0..depth {
                self.out.push_str("    ");
            }
            self.out.push_str(s);
        }
        self.out.push('\n');
    }

    fn var(&self, name: &str) -> String {
        py_name(name)
    }

    fn expr(&self, e: &TExpr) -> String {
        match e {
            TExpr::Str(s) => q(s),
            TExpr::Int(n) => n.to_string(),
            TExpr::Bool(b) => if *b { "True".into() } else { "False".into() },
            TExpr::Enum(v, _) => q(v),
            TExpr::None(_) => "None".into(),
            TExpr::Var { name, fields, ty } => {
                let mut s = self.var(name);
                for (i, f) in fields.iter().enumerate() {
                    // an absent field reads as None
                    if i + 1 == fields.len() && matches!(ty, Ty::Opt(_)) {
                        s = format!("{s}.get({})", q(f));
                    } else {
                        s = format!("{s}[{}]", q(f));
                    }
                }
                s
            }
            TExpr::Record { fields, .. } => format!("{{{}}}", fields.iter().map(|(f, x)| format!("{}: {}", q(f), self.expr(x))).collect::<Vec<_>>().join(", ")),
            TExpr::List { items, .. } => format!("[{}]", items.iter().map(|x| self.expr(x)).collect::<Vec<_>>().join(", ")),
            TExpr::Interp(parts) => {
                let out: Vec<String> = parts
                    .iter()
                    .map(|p| match p {
                        IPart::Lit(s) => q(s),
                        IPart::Hole(x) => format!("dd.text({})", self.expr(x)),
                    })
                    .collect();
                if out.is_empty() {
                    "\"\"".into()
                } else {
                    format!("({})", out.join(" + "))
                }
            }
        }
    }

    fn rounds_expr(&self) -> String {
        let rounds: Vec<String> = self.loops.iter().map(|l| format!("dd_loop_{l}")).collect();
        format!("[{}]", rounds.join(", "))
    }

    fn workflow(&mut self, header: &str, rules: bool) -> String {
        let m = self.m;
        self.out.push_str(header);
        self.out.push_str(&format!("# {} v{}{}\n\n", m.name, m.version, if m.description.is_empty() { String::new() } else { format!(": {}", m.description) }));
        self.out.push_str("from __future__ import annotations\n\nimport asyncio\nfrom datetime import timedelta\nfrom typing import Any\n\n");
        self.out.push_str("from temporalio import workflow\nfrom temporalio.common import RetryPolicy\n\n");
        self.out.push_str("from . import runtime as dd\nfrom . import types as T\n\n");
        self.out.push_str("# Temporal tries an activity once; the workflow retries, as the `retry` of each task says\nNO_RETRY = RetryPolicy(maximum_attempts=1)\n\n");
        let callbacks = m.tasks.iter().any(|t| t.callback);
        for t in &m.tasks {
            if t.is_child(Platform::Temporal) {
                continue;
            }
            let timeout = if t.callback { 60 } else { t.timeout.unwrap_or(60) };
            let queue = t.queue.as_ref().map(|qn| format!(", task_queue={}", q(qn))).unwrap_or_default();
            self.out.push_str(&format!(
                "\ndef dd_task_{}(args: dict[str, Any]) -> Any:\n    return workflow.execute_activity({}, args, start_to_close_timeout=timedelta(seconds={timeout}), retry_policy=NO_RETRY{queue})\n\n",
                ident(&t.name),
                q(&ident(&t.name))
            ));
        }
        if rules {
            self.out.push_str("\ndef dd_rule(name: str, args: dict[str, Any]) -> Any:\n    return workflow.execute_activity(name, args, start_to_close_timeout=timedelta(seconds=60), retry_policy=NO_RETRY)\n\n");
        }
        let class = py_name(&m.name);
        let ret = "dict[str, Any] | None";
        self.out.push_str(&format!("\n@workflow.defn(name={})\nclass {class}:\n", q(&ident(&m.name))));
        if callbacks {
            self.out.push_str("    def __init__(self) -> None:\n        self.dd_answers: dict[str, dict[str, Any]] = {}\n\n");
            self.out.push_str("    @workflow.signal(name=\"dandori.callback\")\n    def dd_callback(self, a: dict[str, Any]) -> None:\n        \"\"\"The answer of a callback: {\"callback_id\", \"ok\"} or {\"callback_id\", \"error\", \"message\"}.\"\"\"\n        self.dd_answers[a[\"callback_id\"]] = a\n\n");
        }
        self.out.push_str(&format!("    @workflow.run\n    async def run(self, input: dict[str, Any]) -> {ret}:\n        try:\n            return await self.flow(input)\n        except dd.TaskError as e:\n            # a task's error that nothing handled ends the workflow with the error's kind\n            raise dd.fail(e.kind, e.message) from None\n\n"));
        self.out.push_str(&format!("    async def flow(self, input: dict[str, Any]) -> {ret}:\n"));
        self.body();
        self.out.push_str(&format!("\n\nworkflows = [{class}]\n"));
        std::mem::take(&mut self.out)
    }

    /// The body of `flow`: the input's check, the variables, the flow and `on failure`.
    fn body(&mut self) {
        let m = self.m;
        let d = 2;
        self.line(d, "if not T.is_WorkflowInput(input):");
        self.line(d + 1, "raise dd.fail(\"Dandori.BadInput\", \"the execution's input does not have the declared shape\")");
        let locals = crate::asl::parallel_locals(m);
        for (v, ty) in &m.vars {
            if m.inputs.iter().any(|(i, _)| i == v) {
                self.line(d, &format!("{}: {} = input.get({})", self.var(v), py_type(m, ty), q(v)));
            } else if !locals.contains(v) {
                let t = if matches!(ty, Ty::Opt(_)) { py_type(m, ty) } else { format!("{} | None", py_type(m, ty)) };
                self.line(d, &format!("{}: {t} = None", self.var(v)));
            }
        }
        match &m.on_failure {
            Some(block) => {
                self.line(d, "try:");
                self.block(&m.flow, d + 1);
                self.line(d + 1, "return None");
                self.line(d, "except dd.TaskError as dd_failed:");
                self.line(d + 1, "# on failure");
                self.block(block, d + 1);
                self.line(d + 1, "raise dd_failed");
            }
            None => {
                self.block(&m.flow, d);
                self.line(d, "return None");
            }
        }
    }

    /// The statements of a block, up to the first that ends it; `pass` when there are none.
    fn block(&mut self, ss: &[TStmt], d: usize) {
        let before = self.out.len();
        for s in ss {
            self.stmt(s, d);
            if matches!(s.kind, TK::Succeed { .. } | TK::Fail { .. } | TK::Break) {
                break;
            }
        }
        if self.out.len() == before {
            self.line(d, "pass");
        }
    }

    fn stmt(&mut self, s: &TStmt, d: usize) {
        let m = self.m;
        match &s.kind {
            TK::Pass => {}
            TK::Break => self.line(d, "break"),
            TK::Wait { seconds } => self.line(d, &format!("await asyncio.sleep({seconds})")),
            TK::WaitUntil { at } => {
                let x = self.expr(at);
                self.line(d, &format!("await dd.wait_until({x})"));
            }
            TK::Assign { name, expr } => {
                let x = self.expr(expr);
                self.line(d, &format!("{} = {x}", self.var(name)));
            }
            TK::Succeed { fields } => {
                if fields.is_empty() {
                    self.line(d, "return None");
                } else {
                    let parts: Vec<String> = fields.iter().map(|(f, e)| format!("{}: {}", q(f), self.expr(e))).collect();
                    self.line(d, &format!("return {{{}}}", parts.join(", ")));
                }
            }
            TK::Fail { error, cause, .. } => {
                let c = cause.as_ref().map(|c| self.expr(c)).unwrap_or_else(|| "None".into());
                self.line(d, &format!("raise dd.fail({}, {c})", q(error)));
            }
            TK::Repeat { times, body } => {
                let site = s.site;
                self.line(d, &format!("for dd_loop_{site} in range({times}):"));
                self.loops.push(site);
                self.block(body, d + 1);
                self.loops.pop();
            }
            TK::For { var, list, max, parallel, body, result, locals } => {
                let site = s.site;
                let items = format!("dd_items_{site}");
                self.line(d, &format!("# line {}: for {var} in {}", s.line, list.show()));
                let lx = self.expr(list);
                self.line(d, &format!("{items} = {lx}"));
                self.line(d, &format!("if len({items}) > {max}:"));
                self.line(d + 1, &format!("raise dd.fail(\"Dandori.TooManyItems\", {})", q(&format!("line {}: the list has more than {max} items", s.line))));
                match parallel {
                    None => {
                        if result.is_some() {
                            self.line(d, &format!("dd_out_{site}: list[Any] = []"));
                        }
                        self.line(d, &format!("for dd_loop_{site}, {} in enumerate({items}):", self.var(var)));
                        self.loops.push(site);
                        let before = self.out.len();
                        self.block(body, d + 1);
                        let ends = body.last().map(|x| matches!(x.kind, TK::Succeed { .. } | TK::Fail { .. } | TK::Break)).unwrap_or(false);
                        if let (Some((_, y)), false) = (result, ends) {
                            let yv = self.expr(y);
                            // the `pass` of an empty body goes; the yield is the body now
                            if self.out[before..].trim() == "pass" {
                                self.out.truncate(before);
                            }
                            self.line(d + 1, &format!("dd_out_{site}.append({yv})"));
                        }
                        self.loops.pop();
                        if let Some((r, _)) = result {
                            self.line(d, &format!("{} = dd_out_{site}", self.var(r)));
                        }
                    }
                    Some(k) => {
                        self.line(d, "");
                        self.line(d, &format!("async def dd_round_{site}(dd_item: Any, dd_loop_{site}: int) -> Any:"));
                        self.line(d + 1, &format!("{} = dd_item", self.var(var)));
                        for l in locals {
                            if l != var {
                                self.line(d + 1, &format!("{} = None", self.var(l)));
                            }
                        }
                        self.loops.push(site);
                        self.block(body, d + 1);
                        self.loops.pop();
                        let ends = body.last().map(|x| matches!(x.kind, TK::Succeed { .. } | TK::Fail { .. } | TK::Break)).unwrap_or(false);
                        if !ends {
                            let y = result.as_ref().map(|(_, y)| self.expr(y)).unwrap_or_else(|| "None".into());
                            self.line(d + 1, &format!("return {y}"));
                        }
                        self.line(d, "");
                        self.line(d, &format!("dd_res_{site} = await dd.rounds({items}, dd.at_a_time({k}), dd_round_{site})"));
                        if let Some((r, _)) = result {
                            self.line(d, &format!("{} = dd_res_{site}", self.var(r)));
                        }
                    }
                }
            }
            TK::Match { expr, arms } => {
                let site = s.site;
                let x = match expr {
                    TExpr::Var { name, fields, .. } if m.case_index(name).is_some() => {
                        // a case that has not started reads as none
                        let v = self.var(name);
                        let mut s = v.clone();
                        for f in fields {
                            s.push_str(&format!("[{}]", q(f)));
                        }
                        format!("None if {v} is None else {s}")
                    }
                    other => self.expr(other),
                };
                let dv = format!("dd_v_{site}");
                self.line(d, &format!("{dv} = {x}"));
                for (i, a) in arms.iter().enumerate() {
                    let mut parts = Vec::new();
                    if a.none {
                        parts.push(format!("{dv} is None"));
                    }
                    if a.some.is_some() {
                        parts.push(format!("{dv} is not None"));
                    }
                    if !a.values.is_empty() {
                        if expr.ty().inner() == &Ty::Bool {
                            for v in &a.values {
                                parts.push(format!("{dv} is {}", if v == "true" { "True" } else { "False" }));
                            }
                        } else {
                            parts.push(format!("{dv} in ({}{})", a.values.iter().map(|v| q(v)).collect::<Vec<_>>().join(", "), if a.values.len() == 1 { "," } else { "" }));
                        }
                    }
                    let kw = if i == 0 { "if" } else { "elif" };
                    self.line(d, &format!("{kw} {}:", parts.join(" or ")));
                    if let Some(v) = &a.some {
                        self.line(d + 1, &format!("{} = {dv}", self.var(v)));
                    }
                    self.block(&a.body, d + 1);
                }
                self.line(d, "else:");
                self.line(d + 1, &format!("raise dd.fail(\"Dandori.UnexpectedValue\", {})", q(&format!("line {}: {} took a value that no arm names", s.line, expr.show()))));
            }
            TK::Call { target, callee, args, handlers } => self.call(s, target.as_ref(), callee, args, handlers, d),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn call(&mut self, s: &TStmt, target: Option<&Target>, callee: &Callee, args: &[(String, TExpr)], handlers: &[THandler], d: usize) {
        let m = self.m;
        let site = s.site;
        let p = Platform::Temporal;
        let (cname, declared, result_ty) = match callee {
            Callee::Task(t) => {
                let task = &m.tasks[*t];
                (task.name.clone(), task.errors.iter().map(|e| q(&e.name)).collect::<Vec<_>>(), task.result.clone())
            }
            Callee::Rule(r) => (m.rules[*r].name.clone(), vec![], Some(Ty::Record(m.rules[*r].outputs))),
        };
        let mut parts: Vec<String> = args.iter().map(|(a, e)| format!("{}: {}", q(a), self.expr(e))).collect();
        if let Callee::Task(t) = callee {
            if m.tasks[*t].key {
                parts.push(format!("\"idempotency_key\": dd.key({site}, {})", self.rounds_expr()));
            }
        }
        let retriers = retriers(m, callee);
        let declared = format!("[{}]", declared.join(", "));
        self.line(d, &format!("# line {}: {cname}", s.line));
        let invocation = match callee {
            Callee::Rule(r) => format!("dd.attempt(lambda: dd_rule({}, {{{}}}), {retriers}, [])", q(&render::rule_activity(&m.rules[*r].name)), parts.join(", ")),
            Callee::Task(t) => {
                let task = &m.tasks[*t];
                let f = format!("dd_task_{}", ident(&task.name));
                match task.via(p) {
                    Some(Via::Workflow(ty)) => {
                        self.line(d, &format!("dd_n_{site} = dd.counter()"));
                        let mut opts = vec![format!("id=dd.child_id({site}, {}, next(dd_n_{site}))", self.rounds_expr())];
                        if let Some(qn) = &task.queue {
                            opts.push(format!("task_queue={}", q(qn)));
                        }
                        if let Some(t) = task.timeout {
                            opts.push(format!("execution_timeout=timedelta(seconds={t})"));
                        }
                        format!("dd.attempt(lambda: workflow.execute_child_workflow({}, {{{}}}, {}), {retriers}, {declared})", q(ty), parts.join(", "), opts.join(", "))
                    }
                    _ if task.callback => {
                        let timeout = task.timeout.unwrap_or(86_400);
                        let mut with_id = parts.clone();
                        with_id.push("\"callback_id\": dd_id".into());
                        self.line(d, &format!("dd_n_{site} = dd.counter()"));
                        self.line(d, "");
                        self.line(d, &format!("async def dd_call_{site}() -> Any:"));
                        self.line(d + 1, &format!("dd_id = dd.callback_id({site}, {}, next(dd_n_{site}))", self.rounds_expr()));
                        self.line(d + 1, &format!("await {f}({{{}}})", with_id.join(", ")));
                        self.line(d + 1, &format!("return await dd.await_callback(self.dd_answers, dd_id, {timeout})"));
                        self.line(d, "");
                        format!("dd.attempt(dd_call_{site}, {retriers}, {declared})")
                    }
                    _ => format!("dd.attempt(lambda: {f}({{{}}}), {retriers}, {declared})", parts.join(", ")),
                }
            }
        };
        let var = match target {
            Some(Target::Let(v)) => Some(self.var(v)),
            Some(Target::Case(c)) => Some(self.var(&m.cases[*c].name)),
            None => None,
        };
        let answer = |g: &mut Gen, d: usize| {
            // the answer: its declared type, and for a case the states it may carry
            if let (Some(v), Some(ty)) = (&var, &result_ty) {
                g.line(d, &format!("if not {}:", py_check(m, "dd_r", ty, "T.", 0)));
                g.line(d + 1, &format!("raise dd.fail(\"Dandori.BadResponse\", {})", q(&format!("line {}: the answer from {} does not have the declared shape", s.line, cname))));
                if let (Some(Target::Case(c)), Some((_, allowed))) = (target, m.monitors.get(&site)) {
                    let field = &m.cases[*c].state_field;
                    g.line(d, &format!("if dd_r[{}] not in ({}{}):", q(field), allowed.iter().map(|v| q(v)).collect::<Vec<_>>().join(", "), if allowed.len() == 1 { "," } else { "" }));
                    g.line(
                        d + 1,
                        &format!(
                            "raise dd.fail(\"Dandori.UnexpectedState\", {})",
                            q(&format!("line {}: {} answered with a state the machine does not lead to here (expected one of {})", s.line, cname, allowed.join(", ")))
                        ),
                    );
                }
                g.line(d, &format!("{v} = dd_r"));
            }
        };
        let reads = var.is_some() && result_ty.is_some();
        if handlers.is_empty() {
            self.line(d, &format!("{}await {invocation}", if reads { "dd_r = " } else { "" }));
            answer(self, d);
            return;
        }
        let e = format!("dd_e_{site}");
        self.line(d, "try:");
        self.line(d + 1, &format!("{}await {invocation}", if reads { "dd_r = " } else { "" }));
        self.line(d, &format!("except dd.TaskError as {e}:"));
        for (i, h) in handlers.iter().enumerate() {
            let conds: Vec<String> = h
                .errors
                .iter()
                .map(|x| match x {
                    HErr::Failure => "True".to_string(),
                    HErr::Timeout => format!("{e}.kind == \"timeout\""),
                    HErr::Declared(n) => format!("{e}.kind == {}", q(n)),
                })
                .collect();
            let kw = if i == 0 { "if" } else { "elif" };
            self.line(d + 1, &format!("{kw} {}:", conds.join(" or ")));
            self.block(&h.body, d + 2);
        }
        self.line(d + 1, "else:");
        self.line(d + 2, "raise");
        if reads {
            self.line(d, "else:");
            answer(self, d + 1);
        }
    }
}
