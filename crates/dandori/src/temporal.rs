//! The workflow as TypeScript, for two platforms that run code by checkpoint and replay:
//! Temporal (the TypeScript SDK) and AWS Lambda durable functions (the durable execution
//! SDK for JavaScript). The two share the types, the shape of the code, the retries and the
//! checks; they differ in how a task, a wait and a callback are called.
//!
//! Temporal:
//! - `types.ts`: the records and enums, and a check for each (`is_<Type>`)
//! - `activities.ts`: the tasks' interface; the tasks that say `lambda`, `http`, `aws` or
//!   `agent` are written there, the others (`OwnTasks`) are yours
//! - `io.ts`: how those tasks reach Lambda, HTTP, the AWS APIs and OpenAI's agents (a `Transport`)
//! - `rules.ts`: the rules as activities, around the TypeScript rulec generates
//! - `runtime.ts`: what the workflow code shares — retries, error kinds, keys, callbacks
//! - `workflow.ts`: the workflow
//!
//! Lambda durable functions: `types.ts`, `tasks.ts` (as `activities.ts`), `io.ts`,
//! `runtime.ts`, `workflow.ts` with `makeHandler`, and the Lambda functions of the rules.
//!
//! It means what the state machine means: a task is tried once by the platform and retried
//! by the workflow's own code with the same retriers the ASL has, so that the platforms
//! retry the same errors the same number of times; an answer is checked against its type
//! and, for a case, against the states the checker said it can be in; `on failure` runs for
//! the errors of tasks that nothing handled, and not for `fail` or a failed check. A task
//! that the platform calls by Lambda, HTTP or an AWS API sends what Step Functions sends.

use crate::diag::Diag;
use crate::model::*;
use crate::render::{self, ident};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The version of OpenAI's Agents SDK for JavaScript that the default Transport is written against.
const AGENTS_SDK: &str = "^0.18.0";

/// The version of OpenAI's client for JavaScript, which the Agents SDK runs on, that the default
/// Transport hands the SDK so that the client does not retry.
const OPENAI_CLIENT: &str = "^7.2.0";

/// The version of Anthropic's SDK for JavaScript that the default Transport is written against.
const ANTHROPIC_SDK: &str = "^0.128.0";

/// The workflow's type on Temporal, which is also the task queue of its worker: the flow's name
/// and its version (`hotel_stay_v1`), so that a new version of a `.flow` is a new workflow, and
/// the runs of the old one go on with the old code on its own queue.
pub fn workflow_type(m: &Model) -> String {
    format!("{}_v{}", ident(&m.name), m.version)
}

/// How long an activity may run when its task says no `timeout`, in seconds. A task that the
/// workflow's own worker serves (no `queue`) heartbeats (`HEARTBEAT_SECONDS`), so a worker that
/// went away is noticed without a limit on the task: an HTTP request or an agent gets what Step
/// Functions' HTTP Task gives one, a Lambda function what Lambda gives one, and anything else no
/// limit of its own (a year). A task another worker serves may not heartbeat, and gets a minute.
/// The task that hands a callback's id on hands it on within a minute.
pub(crate) fn activity_timeout(t: &TaskDef) -> u64 {
    if t.callback {
        return 60;
    }
    if let Some(s) = t.timeout {
        return s;
    }
    if t.queue.is_some() {
        return 60;
    }
    match t.via(Platform::Temporal) {
        Some(Via::Http { .. }) | Some(Via::Agent { .. }) => crate::asl::HTTP_TASK_SECONDS,
        Some(Via::Lambda(_)) => LAMBDA_SECONDS,
        _ => 365 * 86_400,
    }
}

/// How long Lambda lets a function run.
const LAMBDA_SECONDS: u64 = 900;

/// How long Temporal waits for a heartbeat of an activity the workflow's own worker serves, and
/// how often the activity sends one.
pub(crate) const HEARTBEAT_SECONDS: u64 = 30;
pub(crate) const BEAT_SECONDS: u64 = 10;

/// How long a rule's activity may run: a rule is a pure function, done in no time.
pub(crate) const RULE_SECONDS: u64 = 10;

/// io.ts, with the numbers every target sends put in.
fn io_ts() -> String {
    IO.replace("{{CLAUDE_MAX_TOKENS}}", &render::CLAUDE_MAX_TOKENS.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavor {
    Temporal,
    Durable,
    /// the program that runs the tasks dandori writes in a container of an Argo workflow
    Argo,
}

impl Flavor {
    fn platform(self) -> Platform {
        match self {
            Flavor::Temporal => Platform::Temporal,
            Flavor::Durable => Platform::Durable,
            Flavor::Argo => Platform::Argo,
        }
    }
}

const TS_GLOBALS: &[&str] = &[
    "Array", "Boolean", "Date", "Error", "Function", "JSON", "Map", "Math", "Number", "Object", "Promise", "RegExp", "Set", "String", "Symbol",
    "WorkflowInput", "WorkflowOutput",
];

fn type_name(name: &str) -> String {
    let n = ident(&name.replace('.', "_"));
    if TS_GLOBALS.contains(&n.as_str()) {
        format!("{n}_")
    } else {
        n
    }
}

fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

/// The TypeScript type of `t`, as the workflow and the tasks' files name it (`T.` for types.ts).
fn ts_type(m: &Model, t: &Ty) -> String {
    match t {
        Ty::Int | Ty::Num(_) => "number".into(),
        Ty::Str | Ty::Timestamp => "string".into(),
        Ty::Bool => "boolean".into(),
        Ty::Enum(e) => format!("T.{}", type_name(&m.enums[*e].name)),
        Ty::Record(r) => format!("T.{}", type_name(&m.records[*r].name)),
        Ty::List(t) => format!("Array<{}>", ts_type(m, t)),
        Ty::Opt(t) => format!("{} | null", ts_type(m, t)),
        Ty::Json => "unknown".into(),
    }
}

/// A check that `x` is a well-formed value of `t`, as TypeScript. `p` is how the code
/// reaches types.ts: "" inside it, "T." from the workflow.
fn ts_check(m: &Model, x: &str, t: &Ty, rg: Option<Range>, p: &str, depth: usize) -> String {
    match t {
        Ty::Int | Ty::Num(_) => match rg {
            Some(r) => format!("(Number.isInteger({x}) && {})", r.tests(|op, n| format!("({x} as number) {op} {n}")).join(" && ")),
            None => format!("Number.isInteger({x})"),
        },
        Ty::Str => format!("typeof {x} === \"string\""),
        Ty::Timestamp => format!("(typeof {x} === \"string\" && {p}TIMESTAMP.test({x}))"),
        Ty::Bool => format!("typeof {x} === \"boolean\""),
        Ty::Enum(e) => format!("{p}{}_values.includes({x})", type_name(&m.enums[*e].name)),
        Ty::Record(r) => format!("{p}is_{}({x})", type_name(&m.records[*r].name)),
        Ty::List(inner) => {
            let v = format!("dd_v{depth}");
            format!("(Array.isArray({x}) && {x}.every(({v}: unknown) => {}))", ts_check(m, &v, inner, rg, p, depth + 1))
        }
        Ty::Opt(inner) => format!("({x} === undefined || {x} === null || {})", ts_check(m, x, inner, rg, p, depth)),
        Ty::Json => format!("({x} !== undefined)"),
    }
}

/// The enums and records the workflow uses, by index.
pub(crate) fn used_types(m: &Model) -> (BTreeSet<usize>, BTreeSet<usize>) {
    let mut enums = BTreeSet::new();
    let mut recs = BTreeSet::new();
    fn visit(m: &Model, t: &Ty, enums: &mut BTreeSet<usize>, recs: &mut BTreeSet<usize>) {
        match t {
            Ty::Enum(e) => {
                enums.insert(*e);
            }
            Ty::Record(r) => {
                if recs.insert(*r) {
                    for (_, ft) in &m.records[*r].fields {
                        visit(m, ft, enums, recs);
                    }
                }
            }
            Ty::List(t) | Ty::Opt(t) => visit(m, t, enums, recs),
            _ => {}
        }
    }
    for (_, t) in m.inputs.iter().chain(&m.outputs).chain(&m.vars) {
        visit(m, t, &mut enums, &mut recs);
    }
    for t in &m.tasks {
        if let Some(r) = &t.result {
            visit(m, r, &mut enums, &mut recs);
        }
        for (_, pt) in &t.params {
            visit(m, pt, &mut enums, &mut recs);
        }
    }
    for s in m.all_stmts() {
        if let TK::Call { callee: Callee::Rule(r), .. } = &s.kind {
            visit(m, &Ty::Record(m.rules[*r].outputs), &mut enums, &mut recs);
            let rn = &m.rules[*r].name;
            for c in &m.rules[*r].info.inputs {
                if let crate::rulec::RType::Enum(en) = &c.ty {
                    if let Some(i) = m.enums.iter().position(|x| x.name == format!("{rn}.{en}")) {
                        enums.insert(i);
                    }
                }
            }
        }
    }
    for s in m.all_stmts() {
        let mut see = |e: &TExpr| {
            let mut stack = vec![e.clone()];
            while let Some(x) = stack.pop() {
                visit(m, &x.ty(), &mut enums, &mut recs);
                match x {
                    TExpr::Record { fields, .. } => stack.extend(fields.into_iter().map(|(_, v)| v)),
                    TExpr::List { items, .. } => stack.extend(items),
                    _ => {}
                }
            }
        };
        match &s.kind {
            TK::Call { args, .. } => args.iter().for_each(|(_, e)| see(e)),
            TK::Assign { expr, .. } => see(expr),
            TK::Succeed { fields } => fields.iter().for_each(|(_, e)| see(e)),
            _ => {}
        }
    }
    (enums, recs)
}

fn types_file(m: &Model, header: &str) -> String {
    let (enums, recs) = used_types(m);
    let mut t = header.to_string();
    t.push_str(&format!("// The records and enums of {} v{}, and a check for each.\n\n", m.name, m.version));
    t.push_str(&format!("export const TIMESTAMP = /{}/;\n\n", render::TIMESTAMP_RE.replace("\\\\", "\\")));
    for e in &enums {
        let en = &m.enums[*e];
        let n = type_name(&en.name);
        let vals: Vec<String> = en.values.iter().map(|v| q(v)).collect();
        t.push_str(&format!("export const {n}_values: readonly unknown[] = [{}];\n", vals.join(", ")));
        t.push_str(&format!("export type {n} = {};\n\n", vals.join(" | ")));
    }
    let local = |m: &Model, ty: &Ty| ts_type(m, ty).replace("T.", "");
    for r in &recs {
        let rd = &m.records[*r];
        let n = type_name(&rd.name);
        t.push_str(&format!("export interface {n} {{\n"));
        for (f, ft) in &rd.fields {
            let opt = if matches!(ft, Ty::Opt(_)) { "?" } else { "" };
            t.push_str(&format!("  {}{opt}: {};\n", q(f), local(m, ft)));
        }
        t.push_str("}\n\n");
        let mut conds = vec!["typeof v === \"object\"".to_string(), "v !== null".to_string()];
        for (f, ft) in &rd.fields {
            conds.push(ts_check(m, &format!("v[{}]", q(f)), ft, rd.ranges.get(f).copied(), "", 0));
        }
        t.push_str(&format!("export function is_{n}(v: any): v is {n} {{\n  return {};\n}}\n\n", conds.join(" &&\n    ")));
    }
    t.push_str("export interface WorkflowInput {\n");
    for (n, ty) in &m.inputs {
        t.push_str(&format!("  {}: {};\n", q(n), local(m, ty)));
    }
    t.push_str("}\n\n");
    let mut conds = vec!["typeof v === \"object\"".to_string(), "v !== null".to_string()];
    for (n, ty) in &m.inputs {
        conds.push(ts_check(m, &format!("v[{}]", q(n)), ty, m.input_ranges.get(n).copied(), "", 0));
    }
    t.push_str(&format!("export function is_WorkflowInput(v: any): v is WorkflowInput {{\n  return {};\n}}\n\n", conds.join(" &&\n    ")));
    t.push_str("export interface WorkflowOutput {\n");
    for (n, ty) in &m.outputs {
        t.push_str(&format!("  {}: {};\n", q(n), local(m, ty)));
    }
    t.push_str("}\n");
    t
}

pub fn build(m: &Model) -> Result<Vec<(String, String)>, Vec<Diag>> {
    build_flavor(m, Flavor::Temporal)
}

pub fn build_flavor(m: &Model, flavor: Flavor) -> Result<Vec<(String, String)>, Vec<Diag>> {
    let dir = ident(&m.name);
    let header = format!("// Code generated by dandori from {}. DO NOT EDIT.\n", m.source_file);
    let types_ts = types_file(m, &header);
    let called: BTreeSet<usize> = m.all_stmts().iter().filter_map(|s| match &s.kind { TK::Call { callee: Callee::Rule(r), .. } => Some(*r), _ => None }).collect();
    let io = format!("{header}{}", io_ts());
    if flavor == Flavor::Durable {
        let mut errs = Vec::new();
        errs.extend(m.refuse_on_cancel(
            "Lambda durable functions ends an execution at once when it is stopped (StopDurableExecution) and runs nothing after, so `on cancel` cannot run there",
            "Lambda durable functions は実行を止めると（StopDurableExecution）その場で終え、あとに何も走らせないので、`on cancel` はそこでは動きません",
        ));
        errs.extend(crate::check::history_limit(m, Platform::Durable));
        errs.extend(m.refuse_events(Platform::Durable));
        for r in &called {
            if m.rules[*r].lambda.is_none() {
                let ru = &m.rules[*r];
                errs.push(Diag::error(
                    "E050",
                    ru.line,
                    1,
                    format!("the rule `{}` is called, so it needs `lambda \"<function>\"` under `use rule` to be invoked", ru.name),
                    format!("規則 `{}` は呼ばれているので、呼び出す先の `lambda \"<関数>\"` を `use rule` の下に書いてください", ru.name),
                ));
            }
        }
        for t in &m.tasks {
            if t.durable_function.is_some() && t.timeout.is_some() {
                errs.push(Diag::error(
                    "E050",
                    t.line,
                    1,
                    format!("a durable function invoked by `{}` cannot be timed out by the invoke; leave out `timeout` and let the function end itself", t.name),
                    format!("`{}` が呼ぶ durable function は、呼ぶ側からはタイムアウトさせられません。`timeout` を外し、呼ばれる関数の側で終わらせてください", t.name),
                ));
            }
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        let mut g = Gen { m, out: String::new(), loops: vec![], flavor, ctx: "context".into(), can: None };
        let wf = g.workflow(&header, !called.is_empty());
        let mut files = vec![
            (format!("{dir}/types.ts"), types_ts),
            (format!("{dir}/tasks.ts"), tasks_file(m, flavor, &header)),
            (format!("{dir}/io.ts"), io),
            (format!("{dir}/runtime.ts"), format!("{header}{RUNTIME_DURABLE}")),
            (format!("{dir}/workflow.ts"), wf),
        ];
        for r in &called {
            let (name, text) = crate::asl::lambda_handler(m, *r);
            files.push((format!("{dir}/{name}"), text));
        }
        return Ok(files);
    }
    if let Some(d) = crate::check::history_limit(m, Platform::Temporal) {
        return Err(vec![d]);
    }
    let mut g = Gen { m, out: String::new(), loops: vec![], flavor, ctx: String::new(), can: None };
    let wf = g.workflow(&header, !called.is_empty());
    let mut files = vec![
        (format!("{dir}/types.ts"), types_ts),
        (format!("{dir}/activities.ts"), tasks_file(m, flavor, &header)),
        (format!("{dir}/io.ts"), io),
        (format!("{dir}/runtime.ts"), format!("{header}{RUNTIME}")),
        (format!("{dir}/workflow.ts"), wf),
        (format!("{dir}/client.ts"), client_file(m, &header)),
    ];
    if !called.is_empty() {
        files.push((format!("{dir}/rules.ts"), rules_file(m, &header, &called)));
    }
    let id = build_id(&files);
    files.push((format!("{dir}/worker.ts"), worker_file(m, &header, !called.is_empty(), &id)));
    Ok(files)
}

/// The build id of Worker Deployment Versioning: a hash of the code dandori wrote, which
/// changes whenever the code does.
pub(crate) fn build_id(files: &[(String, String)]) -> String {
    // FNV-1a, 64 bits
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (name, text) in files {
        for b in name.bytes().chain([0u8]).chain(text.bytes()).chain([0u8]) {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("dandori-{h:016x}")
}

/// client.ts: starting the workflow, answering its callbacks, and asking where it is.
fn client_file(m: &Model, header: &str) -> String {
    let ty = workflow_type(m);
    let mut c = header.to_string();
    c.push_str(&format!("// Starting {} v{}, answering its callbacks, and asking where it is.\n\n", m.name, m.version));
    c.push_str("import { type Client, type WorkflowHandle } from \"@temporalio/client\";\n");
    c.push_str("import { WorkflowIdReusePolicy } from \"@temporalio/common\";\n");
    c.push_str("import type * as T from \"./types\";\n\n");
    c.push_str(&format!("/** The workflow's type. The `.flow`'s version is in it, so that a new version is a new workflow. */\nexport const WORKFLOW_TYPE = {};\n\n", q(&ty)));
    c.push_str(&format!("/** The task queue of the workflow and its activities (the tasks that say `queue` go to theirs). */\nexport const TASK_QUEUE = {};\n\n", q(&ty)));
    c.push_str("export interface StartOptions {\n  /** Keep the search attribute DandoriCases (a keyword list) up to date with the cases' states, as \"<case>=<state>\". Register it on the namespace first. */\n  searchAttributes?: boolean;\n}\n\n");
    c.push_str("/**\n * Start the workflow as `id`. The idempotency keys of its calls are made from the id, so an id\n * is used once: a second start with it is refused, even after the first run has ended.\n */\n");
    c.push_str("export async function start(client: Client, id: string, input: T.WorkflowInput, options: StartOptions = {}): Promise<WorkflowHandle> {\n");
    c.push_str("  return client.workflow.start(WORKFLOW_TYPE, {\n    args: [input],\n    taskQueue: TASK_QUEUE,\n    workflowId: id,\n    workflowIdReusePolicy: WorkflowIdReusePolicy.REJECT_DUPLICATE,\n    ...(options.searchAttributes ? { memo: { \"dandori.cases\": true } } : {}),\n  });\n}\n\n");
    c.push_str("/** A callback's answer: its value, or the error it names. */\nexport type CallbackAnswer = { ok: unknown } | { error: string; message?: string };\n\n");
    c.push_str("/**\n * Answer a callback, by the id its task handed on. The workflow says whether it took the answer:\n * it refuses one for a callback it does not wait for, and a second one (WorkflowUpdateFailedError).\n */\n");
    c.push_str("export async function answer(client: Client, callbackId: string, a: CallbackAnswer): Promise<void> {\n  const [workflowId] = JSON.parse(callbackId) as [string, string];\n  await client.workflow.getHandle(workflowId).executeUpdate(\"dandori.answer\", { args: [{ callback_id: callbackId, ...a }] });\n}\n\n");
    let events: Vec<String> = m.tasks.iter().filter(|t| t.event).map(|t| q(&t.name)).collect();
    if !events.is_empty() {
        c.push_str(&format!("/** The events the workflow waits for (the tasks that say `event`), by name. */\nexport const EVENTS = [{}] as const;\nexport type EventName = (typeof EVENTS)[number];\n\n", events.join(", ")));
        c.push_str("/**\n * Send an event to the workflow, by its id and the event's name: its value, or the error it\n * names. The workflow refuses an event it does not wait for now, and a second one\n * (WorkflowUpdateFailedError): send it again when the workflow waits for it (status: `events`).\n */\n");
        c.push_str("export async function send(client: Client, workflowId: string, event: EventName, a: CallbackAnswer): Promise<void> {\n  await client.workflow.getHandle(workflowId).executeUpdate(\"dandori.event\", { args: [{ event, ...a }] });\n}\n\n");
    }
    c.push_str("/** Where the workflow is: the line of the call or the wait it is at, each case's state (null before it starts), and the events it waits for now. */\nexport interface Status {\n  at: number | null;\n  cases: Record<string, string | null>;\n  events: string[];\n}\n\n");
    c.push_str("export async function status(client: Client, id: string): Promise<Status> {\n  return client.workflow.getHandle(id).query<Status>(\"dandori.status\");\n}\n\n");
    c.push_str("/**\n * The histories of the runs of this workflow that `query` finds, by default the ones going on:\n * replay them with new code (worker.ts: replay) before it takes them over.\n */\n");
    c.push_str("export async function* histories(client: Client, query = `WorkflowType = '${WORKFLOW_TYPE}' AND ExecutionStatus = 'Running'`): AsyncIterable<{ workflowId: string; history: Awaited<ReturnType<WorkflowHandle[\"fetchHistory\"]>> }> {\n");
    c.push_str("  for await (const w of client.workflow.list({ query })) {\n    yield { workflowId: w.workflowId, history: await client.workflow.getHandle(w.workflowId, w.runId).fetchHistory() };\n  }\n}\n");
    c
}

/// worker.ts: the worker that runs the workflow, its tasks and its rules.
fn worker_file(m: &Model, header: &str, rules: bool, build: &str) -> String {
    let mut w = header.to_string();
    w.push_str(&format!("// The worker of {} v{}: the workflow, the tasks dandori writes and the ones you write, and the rules.\n\n", m.name, m.version));
    w.push_str("import { NativeConnection, Worker, type ReplayHistoriesIterable, type ReplayWorkerOptions, type WorkerOptions } from \"@temporalio/worker\";\n");
    w.push_str("import { makeActivities, type OwnTasks } from \"./activities\";\n");
    w.push_str("import { TASK_QUEUE } from \"./client\";\n");
    w.push_str("import * as io from \"./io\";\n");
    if rules {
        w.push_str("import { rules } from \"./rules\";\n");
    }
    w.push('\n');
    w.push_str(&format!("/** A hash of the code dandori wrote: it changes whenever the code does. The build id of Worker Deployment Versioning. */\nexport const BUILD_ID = {};\n\n", q(build)));
    w.push_str("export interface WorkerConfig {\n  /** Where the workflow's code is; by default workflow.ts beside this file, as CommonJS finds it. */\n  workflowsPath?: string;\n  transport?: io.Transport;\n  namespace?: string;\n");
    w.push_str("  /**\n   * Worker Deployment Versioning: the deployment this worker is a version of. A run stays on the\n   * build it started on (PINNED), so the code dandori writes anew never replays an old run.\n   */\n  deployment?: string;\n}\n\n");
    w.push_str("/** The worker's options but the connection: the workflow, and the activities on TASK_QUEUE. */\n");
    w.push_str("export function workerOptions(own: OwnTasks, config: WorkerConfig = {}): Omit<WorkerOptions, \"connection\"> {\n  return {\n    namespace: config.namespace ?? \"default\",\n    taskQueue: TASK_QUEUE,\n    workflowsPath: config.workflowsPath ?? require.resolve(\"./workflow\"),\n");
    w.push_str(&format!("    activities: {{ ...makeActivities(own, config.transport){} }},\n", if rules { ", ...rules" } else { "" }));
    w.push_str("    ...(config.deployment === undefined\n      ? {}\n      : { workerDeploymentOptions: { useWorkerVersioning: true, version: { deploymentName: config.deployment, buildId: BUILD_ID }, defaultVersioningBehavior: \"PINNED\" } }),\n  };\n}\n\n");
    w.push_str("/** The worker, on the connection given or on one to the local server. */\nexport async function makeWorker(own: OwnTasks, config: WorkerConfig & { connection?: NativeConnection } = {}): Promise<Worker> {\n  const connection = config.connection ?? (await NativeConnection.connect({}));\n  return Worker.create({ ...workerOptions(own, config), connection });\n}\n\n");
    w.push_str("/**\n * Replay histories (client.ts: histories) with this code, and tell the runs it finds\n * nondeterministic, with why: before this code takes over runs that are going on, none may be.\n */\n");
    w.push_str("export async function replay(histories: ReplayHistoriesIterable, options: Partial<ReplayWorkerOptions> = {}): Promise<Array<{ workflowId: string; error: string }>> {\n");
    w.push_str("  const failed: Array<{ workflowId: string; error: string }> = [];\n  const workflowsPath = options.workflowsPath ?? require.resolve(\"./workflow\");\n");
    w.push_str("  for await (const r of Worker.runReplayHistories({ ...options, workflowsPath }, histories)) {\n    if (r.error) failed.push({ workflowId: r.workflowId, error: `${r.error.name}: ${r.error.message}` });\n  }\n  return failed;\n}\n");
    w
}

/// The parameters a task's code gets: its own, the key, and a callback's id.
fn task_params(m: &Model, task: &TaskDef) -> Vec<String> {
    let mut params: Vec<String> = task.params.iter().map(|(p, pt)| format!("{}: {}", q(p), ts_type(m, pt))).collect();
    if task.key {
        params.push("\"idempotency_key\": string".into());
    }
    if task.callback {
        params.push("\"callback_id\": string".into());
    }
    params
}

fn task_result(m: &Model, task: &TaskDef) -> String {
    if task.callback {
        return "void".into();
    }
    match &task.result {
        Some(t) => ts_type(m, t),
        None => "unknown".into(),
    }
}

/// activities.ts (Temporal) or tasks.ts (durable functions): every task the workflow calls
/// as an activity or a step, the ones the user writes, and the code for the others.
fn tasks_file(m: &Model, flavor: Flavor, header: &str) -> String {
    let p = flavor.platform();
    let tasks: Vec<&TaskDef> = m
        .tasks
        .iter()
        .filter(|t| !t.is_child(p) && !t.event)
        .filter(|t| flavor != Flavor::Argo || matches!(t.via(p), Some(Via::Lambda(_)) | Some(Via::Http { .. }) | Some(Via::Aws { .. }) | Some(Via::Agent { .. })))
        .collect();
    let own: Vec<&TaskDef> = tasks.iter().filter(|t| matches!(t.via(p), Some(Via::Own))).cloned().collect();
    let mut a = header.to_string();
    let (made, register) = match flavor {
        Flavor::Temporal => ("makeActivities", "the activities to register with the worker"),
        Flavor::Durable => ("makeTasks", "what the handler calls, each in a step of its own"),
        Flavor::Argo => ("makeTasks", "what call.ts runs"),
    };
    a.push_str("// The tasks the workflow calls.\n//\n");
    a.push_str("// - A task that says `lambda`, `http` or `aws` is written here: it sends what Step Functions\n");
    a.push_str("//   would send, through a Transport (io.ts), where the credentials and the clients are yours to set.\n");
    a.push_str("// - So is a task that says `agent`: the model gets the arguments as JSON text, as from Step\n");
    a.push_str("//   Functions, and answers { \"answer\": … } in the JSON Schema below; the Transport runs the agent.\n");
    a.push_str("//   A Claude agent's enum values are taken without regard to case (io.fold).\n");
    a.push_str("// - The others are yours to write (OwnTasks). ");
    match flavor {
        Flavor::Temporal => a.push_str("A declared error is thrown as\n//   ApplicationFailure.create({ type: \"<error>\", nonRetryable: true }).\n"),
        Flavor::Durable => a.push_str("A declared error is thrown as an Error whose\n//   name is the error's name: const e = new Error(\"...\"); e.name = \"card_declined\"; throw e;\n"),
        Flavor::Argo => a.push_str("On Argo Workflows they are containers of your own images,\n//   which the workflow runs; see the comment at the top of the workflow.\n"),
    }
    match flavor {
        Flavor::Argo => a.push_str("// - A call here is tried once; Argo tries the container again, as the `retry` of each task says.\n"),
        _ => a.push_str("// - The workflow retries by itself, as the `retry` of each task says; the platform does not.\n"),
    }
    a.push_str("// - A task with `key` gets `idempotency_key`: pass it on to the other side as it is.\n");
    match flavor {
        Flavor::Temporal => a.push_str(
            "// - A `callback` task gets `callback_id` and returns once it has handed the id on. The answer goes\n//   to the workflow `io.workflowOf(callback_id)` names, with client.ts's `answer` (the update\n//   `dandori.answer`, or the signal `dandori.callback`): { callback_id, ok } or { callback_id, error, message }.\n// - A task that says `event` is not here: nothing is called, and its value is sent to the workflow\n//   (client.ts: send).\n\n",
        ),
        Flavor::Argo => a.push_str(
            "// - A `callback` task gets `callback_id` and returns once it has handed the id on. The answer comes\n//   with `argo node set <workflow> --output-parameter answer=<{\"ok\": …} or {\"error\": …, \"message\": …}>\n//   --node-field-selector inputs.parameters.callback_id.value=<callback_id>` and then `argo resume` with\n//   the same selector; the workflow is the part of the id before the first `/`.\n\n",
        ),
        Flavor::Durable => a.push_str(
            "// - A `callback` task gets `callback_id` and returns once it has handed the id on. The answer\n//   comes when the other side calls SendDurableExecutionCallbackSuccess with the id and the\n//   answer as JSON, or SendDurableExecutionCallbackFailure with the error's name as ErrorType.\n\n",
        ),
    }
    if flavor == Flavor::Temporal {
        a.push_str("import { Context } from \"@temporalio/activity\";\n");
        a.push_str("import { ApplicationFailure } from \"@temporalio/common\";\n");
    }
    a.push_str("import type * as T from \"./types\";\n");
    a.push_str("import * as io from \"./io\";\n\n");
    a.push_str("export interface Tasks {\n");
    for task in &tasks {
        a.push_str(&format!("  {}(args: {{ {} }}): Promise<{}>;\n", ident(&task.name), task_params(m, task).join("; "), task_result(m, task)));
    }
    a.push_str("}\n\n");
    a.push_str("/** The tasks you write: the ones that say neither `lambda`, `http`, `aws` nor `agent`. */\n");
    a.push_str("export interface OwnTasks {\n");
    for task in &own {
        a.push_str(&format!("  {}(args: {{ {} }}): Promise<{}>;\n", ident(&task.name), task_params(m, task).join("; "), task_result(m, task)));
    }
    a.push_str("}\n\n");
    match flavor {
        Flavor::Temporal => {
            a.push_str("function fail(kind: string, message: string): never {\n  throw ApplicationFailure.create({ type: kind, message, nonRetryable: true });\n}\n\n");
            a.push_str(&format!(
                "/** Every task heartbeats while it runs, every {BEAT_SECONDS} seconds, so that Temporal notices a worker that went away. */\nfunction beating(tasks: Tasks): Tasks {{\n  const out: Record<string, (args: any) => Promise<unknown>> = {{}};\n  for (const [name, run] of Object.entries(tasks) as Array<[string, (args: any) => Promise<unknown>]>) {{\n    out[name] = async (args) => {{\n      const context = Context.current();\n      const timer = setInterval(() => context.heartbeat(), {});\n      try {{\n        return await run(args);\n      }} finally {{\n        clearInterval(timer);\n      }}\n    }};\n  }}\n  return out as unknown as Tasks;\n}}\n\n",
                BEAT_SECONDS * 1000
            ));
        }
        Flavor::Durable | Flavor::Argo => a.push_str("function fail(kind: string, message: string): never {\n  const e = new Error(message);\n  e.name = kind;\n  throw e;\n}\n\n"),
    }
    let agents: Vec<&TaskDef> = tasks.iter().filter(|t| matches!(t.via(p), Some(Via::Agent { .. }))).cloned().collect();
    if !agents.is_empty() {
        a.push_str("/** What each agent answers in: the JSON Schema its provider's structured outputs hold the model to. */\n");
        a.push_str("const SCHEMAS: Record<string, Record<string, unknown>> = {\n");
        for task in &agents {
            let schema = render::agent_schema(m, task).expect("the checker gives an agent an answer with a schema");
            a.push_str(&format!("  {}: {},\n", q(&task.name), render::layout(&schema, 1, "  ", false)));
        }
        a.push_str("};\n\n");
    }
    a.push_str(&format!("/** Your tasks and the ones dandori writes: {register}. */\n"));
    a.push_str(&format!("export function {made}(own: OwnTasks, transport: io.Transport = io.transport()): Tasks {{\n"));
    a.push_str(if flavor == Flavor::Temporal { "  return beating({\n" } else { "  return {\n" });
    for task in &tasks {
        let name = ident(&task.name);
        match task.via(p) {
            Some(Via::Own) => a.push_str(&format!("    {name}: (args) => own.{name}(args),\n")),
            Some(Via::Lambda(f)) => {
                let names = task.errors.iter().map(|e| format!("{}: {}", q(&e.name), q(&e.name))).collect::<Vec<_>>().join(", ");
                if task.callback {
                    a.push_str(&format!("    {name}: async (args) => {{\n      io.value(await transport.lambda({}, args), {}, fail);\n    }},\n", q(f), braces(&names)));
                } else {
                    a.push_str(&format!("    {name}: async (args) => io.value(await transport.lambda({}, args), {}, fail) as {},\n", q(f), braces(&names), task_result(m, task)));
                }
            }
            Some(Via::Http { method, url, form }) => {
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
                    url_parts.push(format!("String(args[{}])", q(&rest[i + 1..j])));
                    rest = &rest[j + 1..];
                }
                if !rest.is_empty() || url_parts.is_empty() {
                    url_parts.push(q(rest));
                }
                let mut req = vec![format!("http: {}", q(method)), format!("url: {}", url_parts.join(" + "))];
                let mut headers = Vec::new();
                if form {
                    headers.push("\"Content-Type\": \"application/x-www-form-urlencoded\"".to_string());
                }
                if task.key {
                    headers.push("\"Idempotency-Key\": args[\"idempotency_key\"]".to_string());
                }
                if !headers.is_empty() {
                    req.push(format!("headers: {{ {} }}", headers.join(", ")));
                }
                let rest_args: Vec<String> = task.params.iter().filter(|(p, _)| !used.contains(p)).map(|(p, _)| format!("{}: args[{}]", q(p), q(p))).collect();
                if !rest_args.is_empty() {
                    let place = if method == "GET" || method == "DELETE" { "query" } else { "body" };
                    req.push(format!("{place}: {{ {} }}", rest_args.join(", ")));
                }
                if form {
                    req.push("form: true".into());
                }
                let statuses = task.errors.iter().filter_map(|e| e.status.map(|s| format!("{}: {}", q(&s.to_string()), q(&e.name)))).collect::<Vec<_>>().join(", ");
                a.push_str(&format!("    {name}: async (args) => io.status(await transport.http({{ {} }}), {}, fail) as {},\n", req.join(", "), braces(&statuses), task_result(m, task)));
            }
            Some(Via::Aws { service, action }) => {
                let names = task.errors.iter().map(|e| format!("{}: {}", q(e.exception.as_deref().unwrap_or(&e.name)), q(&e.name))).collect::<Vec<_>>().join(", ");
                let mut input: Vec<String> = task
                    .params
                    .iter()
                    .map(|(p, _)| {
                        if task.callback && p == "MessageBody" {
                            format!("{}: {{ ...(args[{}] as object), \"callback_id\": args[\"callback_id\"] }}", q(p), q(p))
                        } else {
                            format!("{}: args[{}]", q(p), q(p))
                        }
                    })
                    .collect();
                if let (true, Some(kp)) = (task.key, &task.key_param) {
                    input.push(format!("{}: args[\"idempotency_key\"]", q(kp)));
                }
                let call = format!("transport.aws({}, {}, {{ {} }})", q(service), q(action), input.join(", "));
                if task.callback {
                    a.push_str(&format!("    {name}: async (args) => {{\n      io.value(await {call}, {}, fail);\n    }},\n", braces(&names)));
                } else {
                    a.push_str(&format!("    {name}: async (args) => io.value(await {call}, {}, fail) as {},\n", braces(&names), task_result(m, task)));
                }
            }
            Some(Via::Agent { provider, instructions, model }) => {
                let input: Vec<String> = task.params.iter().map(|(p, _)| format!("{}: args[{}]", q(p), q(p))).collect();
                // Claude may answer an enum's value in another case: it is taken as the value
                let fold = provider == Provider::Claude && !render::enums_of(m, task.result.as_ref().unwrap_or(&Ty::Json)).is_empty();
                let i = if fold { "          " } else { "        " };
                let call = [
                    "await transport.agent({".to_string(),
                    format!("  agent: {},", q(&task.name)),
                    format!("  provider: {},", q(provider.name())),
                    format!("  model: {},", q(model)),
                    format!("  instructions: {},", q(instructions)),
                    format!("  input: {{ {} }},", input.join(", ")),
                    format!("  schema: SCHEMAS[{}],", q(&task.name)),
                    "}),".into(),
                ]
                .iter()
                .map(|l| format!("{i}{l}\n"))
                .collect::<String>();
                a.push_str(&format!("    {name}: async (args) =>\n      io.answer(\n"));
                if fold {
                    a.push_str(&format!("        io.fold(\n{call}          SCHEMAS[{}],\n        ),\n", q(&task.name)));
                } else {
                    a.push_str(&call);
                }
                a.push_str(&format!("      ) as {},\n", task_result(m, task)));
            }
            _ => {}
        }
    }
    a.push_str(if flavor == Flavor::Temporal { "  });\n}\n" } else { "  };\n}\n" });
    a
}

/// `{ a, b }`, or `{}` when there is nothing in it.
fn braces(inner: &str) -> String {
    if inner.is_empty() {
        "{}".into()
    } else {
        format!("{{ {inner} }}")
    }
}

/// The program that runs, in a container of an Argo workflow, the tasks dandori writes (the
/// ones that say `lambda`, `http`, `aws` or `agent`) and the rules: `caller/` in the Argo build.
pub fn argo_caller(m: &Model, header: &str) -> Vec<(String, String)> {
    let called: BTreeSet<usize> = m.all_stmts().iter().filter_map(|s| match &s.kind { TK::Call { callee: Callee::Rule(r), .. } => Some(*r), _ => None }).collect();
    // Node runs these directly, stripping the types, and wants the extension on a relative import
    let ts = |text: String| text.replace("from \"./types\"", "from \"./types.ts\"").replace("from \"./io\"", "from \"./io.ts\"");
    let mut files = vec![
        ("caller/types.ts".to_string(), types_file(m, header)),
        ("caller/tasks.ts".to_string(), ts(tasks_file(m, Flavor::Argo, header))),
        ("caller/io.ts".to_string(), format!("{header}{}", io_ts())),
        (
            "caller/transport.ts".to_string(),
            format!("{header}// Where the tasks dandori writes send their calls: set the credentials and the AWS SDK's\n// configuration here, as io.ts's Options say.\n\nimport * as io from \"./io.ts\";\n\nexport const transport: io.Transport = io.transport();\n"),
        ),
    ];
    if !called.is_empty() {
        let rules = rules_file(m, header, &called).lines().map(|l| {
            if l.starts_with("import ") && l.contains("./rulec/typescript/") {
                l.replacen("\";", ".ts\";", 1)
            } else {
                l.to_string()
            }
        }).collect::<Vec<_>>().join("\n") + "\n";
        files.push(("caller/rules.ts".to_string(), rules));
    }
    let mut declared = Vec::new();
    for t in &m.tasks {
        if matches!(t.via(Platform::Argo), Some(Via::Lambda(_)) | Some(Via::Http { .. }) | Some(Via::Aws { .. }) | Some(Via::Agent { .. })) {
            declared.push(format!("  {}: [{}],", q(&ident(&t.name)), t.errors.iter().map(|e| q(&e.name)).collect::<Vec<_>>().join(", ")));
        }
    }
    let mut call = header.to_string();
    call.push_str("// Runs one task or rule in a container of the Argo workflow: the call comes in DANDORI_CALL, the\n");
    call.push_str("// answer goes to /tmp/dandori/answer.json; a declared error goes to /tmp/dandori/error.json and\n");
    call.push_str("// the exit code is 3; any other failure exits 1.\n//\n//   node call.ts <task or rule_<rule>>\n\n");
    call.push_str("import fs from \"node:fs\";\nimport { makeTasks } from \"./tasks.ts\";\n");
    call.push_str("import { transport } from \"./transport.ts\";\n\n");
    call.push_str(&format!("const DECLARED: Record<string, readonly string[]> = {{\n{}\n}};\n\n", declared.join("\n")));
    call.push_str("const name = process.argv[2];\nconst args = JSON.parse(process.env.DANDORI_CALL ?? \"{}\");\nfs.mkdirSync(\"/tmp/dandori\", { recursive: true });\n");
    // the rules are read only when one is called, so a task's container needs no rulec module
    if called.is_empty() {
        call.push_str("const all: Record<string, (args: any) => Promise<unknown>> = { ...makeTasks({}, transport) };\n");
    } else {
        call.push_str("const all: Record<string, (args: any) => Promise<unknown>> = name.startsWith(\"rule_\") ? (await import(\"./rules.ts\")).rules : { ...makeTasks({}, transport) };\n");
    }
    call.push_str("try {\n  const out = await all[name](args);\n  fs.writeFileSync(\"/tmp/dandori/answer.json\", JSON.stringify(out ?? null));\n  process.exit(0);\n} catch (e) {\n");
    call.push_str("  const kind = e instanceof Error ? e.name : \"Error\";\n  const message = e instanceof Error ? e.message : String(e);\n");
    call.push_str("  fs.writeFileSync(\"/tmp/dandori/error.json\", JSON.stringify({ error: kind, message }));\n");
    call.push_str("  if ((DECLARED[name] ?? []).includes(kind)) {\n    try {\n      fs.writeFileSync(\"/dev/termination-log\", kind);\n    } catch {\n      // not in a container\n    }\n    process.exit(3);\n  }\n  process.stderr.write(`${kind}: ${message}\\n`);\n  process.exit(1);\n}\n");
    files.push(("caller/call.ts".to_string(), call));
    // the packages the default Transport loads, with the versions they are written against
    let mut deps: BTreeMap<&str, &str> = BTreeMap::new();
    for t in &m.tasks {
        match t.via(Platform::Argo) {
            Some(Via::Lambda(_)) => {
                deps.insert("@aws-sdk/client-lambda", "^3");
            }
            Some(Via::Aws { service, .. }) => {
                if let Some((_, pkg, _)) = crate::aws::SDK_CLIENTS.iter().find(|(s, _, _)| *s == service) {
                    deps.insert(pkg, "^3");
                }
            }
            Some(Via::Agent { provider: Provider::OpenAi, .. }) => {
                deps.insert("@openai/agents", AGENTS_SDK);
                deps.insert("openai", OPENAI_CLIENT);
            }
            Some(Via::Agent { provider: Provider::Claude, .. }) => {
                deps.insert("@anthropic-ai/sdk", ANTHROPIC_SDK);
            }
            _ => {}
        }
    }
    let deps_json = deps.iter().map(|(d, v)| format!("    {}: {}", q(d), q(v))).collect::<Vec<_>>().join(",\n");
    files.push((
        "caller/package.json".to_string(),
        format!("{{\n  \"name\": {},\n  \"private\": true,\n  \"type\": \"module\",\n  \"description\": \"Runs the tasks and rules dandori writes for the Argo workflow {} v{}\",\n  \"dependencies\": {{\n{deps_json}\n  }}\n}}\n", q(&format!("dandori-caller-{}", ident(&m.name).to_lowercase())), m.name, m.version),
    ));
    let mut docker = String::new();
    docker.push_str(&format!("# Code generated by dandori from {}. DO NOT EDIT.\n", m.source_file));
    docker.push_str("# The image that runs the tasks dandori writes and the rules. Build it here, after\n");
    if !called.is_empty() {
        docker.push_str("# `rulec gen <rule> --out rulec` for every rule the workflow calls, ");
    } else {
        docker.push_str("# ");
    }
    docker.push_str("and give its name to the workflow as the parameter `dandori-caller`.\n");
    docker.push_str("FROM node:24-alpine\nWORKDIR /app\nCOPY package.json ./\nRUN npm install --omit=dev\nCOPY . .\n");
    files.push(("caller/Dockerfile".to_string(), docker));
    files
}

fn rules_file(m: &Model, header: &str, called: &BTreeSet<usize>) -> String {
    let mut rules_ts = header.to_string();
    rules_ts.push_str("// The rules the workflow calls, as activities around the TypeScript rulec generates.\n");
    rules_ts.push_str("// `rulec gen <rule> --out rulec` writes the modules these imports read.\n\n");
    // one import a module: two rules of the flow may be the same rule
    let mut imports: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for r in called {
        let ts = &m.rules[*r].info.api["typescript"];
        let module = ts["module"].as_str().unwrap_or("rule").trim_end_matches(".ts").to_string();
        let names = imports.entry(module).or_default();
        names.insert(ts["function"].as_str().unwrap_or("rule").to_string());
        let enum_aliases: Vec<&str> = ts["enums"].as_array().map(|a| a.iter().filter_map(|e| e["alias"].as_str()).collect()).unwrap_or_default();
        for alias in &enum_aliases {
            if ts["params"].as_array().unwrap_or(&vec![]).iter().any(|p| p["type"].as_str() == Some(alias)) {
                names.insert(format!("parse{alias}"));
            }
        }
        // a number with a unit is a bigint with a brand, which rulec's own runner makes by `as`
        for p in ts["params"].as_array().unwrap_or(&vec![]) {
            let ty = p["type"].as_str().unwrap_or("");
            if !["boolean", "string", "bigint"].contains(&ty) && !enum_aliases.contains(&ty) {
                names.insert(format!("type {ty}"));
            }
        }
    }
    for (module, names) in imports {
        rules_ts.push_str(&format!("import {{ {} }} from \"./rulec/typescript/{module}\";\n", names.into_iter().collect::<Vec<_>>().join(", ")));
    }
    rules_ts.push_str("\nexport const rules = {\n");
    for r in called {
        let ru = &m.rules[*r];
        let ts = &ru.info.api["typescript"];
        let enum_aliases: Vec<String> = ts["enums"].as_array().unwrap_or(&vec![]).iter().map(|e| e["alias"].as_str().unwrap_or("").to_string()).collect();
        let mut args = Vec::new();
        let mut arg_types = Vec::new();
        for p in ts["params"].as_array().unwrap_or(&vec![]) {
            let name = p["name"].as_str().unwrap_or("");
            let ty = p["type"].as_str().unwrap_or("");
            let get = format!("args[{}]", q(name));
            if enum_aliases.iter().any(|x| x == ty) {
                args.push(format!("parse{ty}(String({get}))"));
                arg_types.push(format!("{}: string", q(name)));
            } else if ty == "boolean" {
                args.push(format!("Boolean({get})"));
                arg_types.push(format!("{}: boolean", q(name)));
            } else if ty == "string" {
                args.push(format!("String({get})"));
                arg_types.push(format!("{}: string", q(name)));
            } else if ty == "bigint" {
                args.push(format!("BigInt({get})"));
                arg_types.push(format!("{}: number", q(name)));
            } else {
                args.push(format!("BigInt({get}) as {ty}"));
                arg_types.push(format!("{}: number", q(name)));
            }
        }
        let mut outs = Vec::new();
        for o in ts["outputs"].as_array().unwrap_or(&vec![]) {
            let name = o["name"].as_str().unwrap_or("");
            let alias = o["alias"].as_str().unwrap_or("");
            let ty = o["type"].as_str().unwrap_or("");
            let v = if enum_aliases.iter().any(|x| x == ty) || ty == "boolean" || ty == "string" { format!("out.{alias}") } else { format!("Number(out.{alias})") };
            outs.push(format!("{}: {v}", q(name)));
        }
        rules_ts.push_str(&format!(
            "  async {}(args: {{ {} }}): Promise<Record<string, unknown>> {{\n    const out = {}({});\n    return {{ {} }};\n  }},\n",
            render::rule_activity(&ru.name),
            arg_types.join("; "),
            ts["function"].as_str().unwrap_or("rule"),
            args.join(", "),
            outs.join(", ")
        ));
    }
    rules_ts.push_str("};\n");
    rules_ts
}

const IO: &str = r#"// How the tasks that say `lambda`, `http`, `aws` or `agent` reach the other side. They go
// through a Transport, so that the credentials, the clients and a test's stand-in are yours to
// set; the default one uses fetch, the AWS SDK for JavaScript v3, OpenAI's Agents SDK
// (@openai/agents, which reads OPENAI_API_KEY) and Anthropic's SDK (@anthropic-ai/sdk, which
// reads ANTHROPIC_API_KEY), each loaded when first needed.

/** What Lambda or an AWS API answered: the value, or an error by the name the other side gives it. */
export type Answer = { ok: unknown } | { error: string; message: string };

/** An HTTP request as Step Functions' HTTP Task sends it; `form` asks for a URL-encoded body. */
export interface HttpRequest {
  http: string;
  url: string;
  headers?: Record<string, string>;
  body?: Record<string, unknown>;
  query?: Record<string, unknown>;
  form?: boolean;
}

/** A call of an agent: the model is told `instructions`, reads `input` as JSON text, and answers in `schema`. */
export interface AgentCall {
  /** the task's name */
  agent: string;
  /** whose models: OpenAI's (run with the Agents SDK) or Claude (with Anthropic's SDK) */
  provider: "openai" | "claude";
  model: string;
  instructions: string;
  input: Record<string, unknown>;
  /** the answer's JSON Schema, { "answer": … }, in the strict form of OpenAI's Structured Outputs, which Claude's take too */
  schema: Record<string, unknown>;
}

export interface Transport {
  /** Invoke a Lambda function; an error the function throws comes back by its type. */
  lambda(fn: string, payload: Record<string, unknown>): Promise<Answer>;
  /** Send an HTTP request; the answer is the status and the body (parsed when it is JSON). */
  http(req: HttpRequest): Promise<{ status: number; body: unknown }>;
  /** Call an AWS API, named as Step Functions names it (`sns`, `publish`); an exception comes back by its name. */
  aws(service: string, action: string, input: Record<string, unknown>): Promise<Answer>;
  /** Run an agent once; the answer is the JSON it gave, as `schema` says. A refusal throws. */
  agent(call: AgentCall): Promise<unknown>;
}

/** The most a Claude agent's answer may take, thinking included, as every target asks for. */
export const CLAUDE_MAX_TOKENS = {{CLAUDE_MAX_TOKENS}};

/** A Claude agent that did not end its turn with an answer: it refused, or stopped at the limit. */
export class AgentStopped extends Error {
  readonly reason: string;
  constructor(reason: string) {
    super(`the model stopped with ${reason}, not with an answer`);
    this.reason = reason;
    this.name = "AgentStopped";
  }
}

export interface Options {
  /** Headers to add to an HTTP request, such as the credentials the other side wants. */
  headers?: (url: string) => Record<string, string> | Promise<Record<string, string>>;
  /** The configuration of the AWS SDK clients, such as the region. */
  aws?: Record<string, unknown>;
  /**
   * The Agents SDK's run configuration, such as { modelProvider } for models other than OpenAI's.
   * Without a modelProvider, the agents run on OpenAI's, through a client that does not retry by
   * itself: the workflow retries, as `retry` says.
   */
  agents?: Record<string, unknown>;
  /**
   * The options of Anthropic's client for the Claude agents, such as { apiKey } or { baseURL }. The
   * client does not retry by itself unless these say so: the workflow retries, as `retry` says.
   */
  claude?: Record<string, unknown>;
}

/** The package and the client of the AWS SDK for each service the default transport knows. */
const CLIENTS: Record<string, [string, string]> = {
  bedrockruntime: ["@aws-sdk/client-bedrock-runtime", "BedrockRuntime"],
  dynamodb: ["@aws-sdk/client-dynamodb", "DynamoDB"],
  ecs: ["@aws-sdk/client-ecs", "ECS"],
  eventbridge: ["@aws-sdk/client-eventbridge", "EventBridge"],
  kinesis: ["@aws-sdk/client-kinesis", "Kinesis"],
  lambda: ["@aws-sdk/client-lambda", "Lambda"],
  s3: ["@aws-sdk/client-s3", "S3"],
  secretsmanager: ["@aws-sdk/client-secrets-manager", "SecretsManager"],
  sesv2: ["@aws-sdk/client-sesv2", "SESv2"],
  sfn: ["@aws-sdk/client-sfn", "SFN"],
  sns: ["@aws-sdk/client-sns", "SNS"],
  sqs: ["@aws-sdk/client-sqs", "SQS"],
  ssm: ["@aws-sdk/client-ssm", "SSM"],
};

/** `a=1&b[c]=2`, the way a URL-encoded body nests. */
function encode(o: Record<string, unknown>, prefix = ""): string {
  const parts: string[] = [];
  for (const [k, v] of Object.entries(o)) {
    const key = prefix ? `${prefix}[${k}]` : k;
    if (v === undefined) continue;
    if (v !== null && typeof v === "object") parts.push(encode(v as Record<string, unknown>, key));
    else parts.push(`${encodeURIComponent(key)}=${encodeURIComponent(v === null ? "" : String(v))}`);
  }
  return parts.filter((p) => p !== "").join("&");
}

export function transport(options: Options = {}): Transport {
  const clients = new Map<string, any>();
  let openai: any;
  let claude: any;
  async function client(service: string): Promise<any> {
    const known = CLIENTS[service];
    if (!known) throw new Error(`the default transport does not know the AWS SDK client for "${service}"; pass a Transport whose aws() calls it`);
    let c = clients.get(service);
    if (!c) {
      const mod: any = await import(known[0]);
      c = new mod[known[1]](options.aws ?? {});
      clients.set(service, c);
    }
    return c;
  }
  return {
    async lambda(fn, payload) {
      const c = await client("lambda");
      const out = await c.invoke({ FunctionName: fn, Payload: new TextEncoder().encode(JSON.stringify(payload)) });
      const text = out.Payload ? new TextDecoder().decode(out.Payload) : "";
      const body = text ? JSON.parse(text) : null;
      if (out.FunctionError) return { error: String(body?.errorType ?? out.FunctionError), message: String(body?.errorMessage ?? "") };
      return { ok: body };
    },
    async http(req) {
      let url = req.url;
      if (req.query) url += (url.includes("?") ? "&" : "?") + encode(req.query);
      const headers: Record<string, string> = { ...(req.headers ?? {}), ...(options.headers ? await options.headers(req.url) : {}) };
      let body: string | undefined;
      if (req.body !== undefined) {
        if (req.form) body = encode(req.body);
        else {
          body = JSON.stringify(req.body);
          if (!headers["Content-Type"]) headers["Content-Type"] = "application/json";
        }
      }
      const res = await fetch(url, { method: req.http, headers, body });
      const text = await res.text();
      let parsed: unknown = text;
      if ((res.headers.get("content-type") ?? "").includes("json")) {
        try {
          parsed = JSON.parse(text);
        } catch {
          parsed = text;
        }
      }
      return { status: res.status, body: parsed };
    },
    async aws(service, action, input) {
      const c = await client(service);
      let args = input;
      // SQS takes the message as text; Step Functions writes a JSON body out the same way
      if (service === "sqs" && action === "sendMessage" && typeof input.MessageBody === "object") args = { ...input, MessageBody: JSON.stringify(input.MessageBody) };
      try {
        const out = await c[action](args);
        const { $metadata, ...rest } = out ?? {};
        return { ok: rest };
      } catch (e: any) {
        if (e && e.$metadata && typeof e.name === "string") return { error: e.name, message: String(e.message ?? "") };
        throw e;
      }
    },
    async agent(call) {
      if (call.provider === "claude") {
        if (!claude) {
          const mod: any = await import("@anthropic-ai/sdk");
          claude = new mod.default({ maxRetries: 0, ...(options.claude ?? {}) });
        }
        // what Step Functions sends: the instructions as the system prompt, the input as the user's message
        const message = await claude.messages.create({
          model: call.model,
          max_tokens: CLAUDE_MAX_TOKENS,
          system: call.instructions,
          messages: [{ role: "user", content: JSON.stringify(call.input) }],
          output_config: { format: { type: "json_schema", schema: call.schema } },
        });
        if (message.stop_reason !== "end_turn") throw new AgentStopped(String(message.stop_reason));
        return JSON.parse(message.content.filter((b: any) => b.type === "text").map((b: any) => b.text).join(""));
      }
      const sdk: any = await import("@openai/agents");
      // no model settings of the SDK's own, so that the model gets what Step Functions sends
      const agent = new sdk.Agent({
        name: call.agent,
        instructions: call.instructions,
        model: call.model,
        modelSettings: {},
        outputType: { type: "json_schema", name: "answer", strict: true, schema: call.schema },
      });
      const config: Record<string, unknown> = { ...(options.agents ?? {}) };
      if (!config.modelProvider) {
        // OpenAI's client retries twice by itself unless it is told not to
        if (!openai) {
          const client: any = await import("openai");
          openai = new sdk.OpenAIProvider({ openAIClient: new client.default({ maxRetries: 0 }) });
        }
        config.modelProvider = openai;
      }
      const result = await new sdk.Runner(config).run(agent, JSON.stringify(call.input));
      return result.finalOutput;
    },
  };
}

/** Temporal: the workflow a callback's answer goes to, from the id the callback task was given. */
export function workflowOf(callbackId: string): string {
  return JSON.parse(callbackId)[0];
}

/** The value of an answer, or the declared error it names (`names`: the other side's name → the error). */
export function value(a: Answer, names: Record<string, string>, fail: (kind: string, message: string) => never): unknown {
  if ("ok" in a) return a.ok;
  const kind = names[a.error];
  return fail(kind ?? `Dandori.Failure.${a.error}`, a.message);
}

/**
 * A Claude agent's answer with every enum value in it spelled as `schema` spells it. Claude's
 * structured outputs do not keep an enum value's case, so a value that differs from one only in
 * case is taken as that one; what does not fit stays, for the answer's check to find.
 */
export function fold(v: unknown, schema: any): unknown {
  if (schema === null || typeof schema !== "object") return v;
  if (Array.isArray(schema.enum)) {
    if (typeof v !== "string" || schema.enum.includes(v)) return v;
    const hit = schema.enum.find((e: unknown) => typeof e === "string" && e.toLowerCase() === v.toLowerCase());
    return hit ?? v;
  }
  if (Array.isArray(schema.anyOf)) return fold(v, schema.anyOf.find((s: any) => s?.type !== "null"));
  if (schema.type === "array") return Array.isArray(v) ? v.map((x) => fold(x, schema.items)) : v;
  if (schema.type === "object" && v !== null && typeof v === "object" && !Array.isArray(v)) {
    const out: Record<string, unknown> = { ...(v as Record<string, unknown>) };
    for (const [k, s] of Object.entries(schema.properties ?? {})) if (k in out) out[k] = fold(out[k], s);
    return out;
  }
  return v;
}

/** What an agent answered: the value under `answer`. An answer without it fails the call. */
export function answer(out: unknown): unknown {
  if (out !== null && typeof out === "object" && "answer" in out) return (out as { answer: unknown }).answer;
  throw new Error("the agent's answer has no `answer`");
}

/** The body of a 2xx answer, or the declared error its status names. */
export function status(r: { status: number; body: unknown }, names: Record<string, string>, fail: (kind: string, message: string) => never): unknown {
  if (r.status >= 200 && r.status < 300) return r.body;
  const kind = names[String(r.status)];
  return fail(kind ?? `Dandori.HttpStatus.${r.status}`, typeof r.body === "string" ? r.body : JSON.stringify(r.body));
}
"#;

const RUNTIME_DURABLE: &str = r#"// What the generated handler shares. The code outside the steps runs again on every replay,
// so it reads nothing that can change between runs: the clock is read in a step.

import { CallbackTimeoutError, DurableOperationError, InvokeError, type DurableContext } from "@aws/durable-execution-sdk-js";

/** A task that failed after its retries, with the kind of error the `.flow` names it by. */
export class TaskError extends Error {
  readonly kind: string;
  constructor(kind: string, message: string) {
    super(message);
    this.kind = kind;
    this.name = "TaskError";
  }
}

/**
 * A deliberate end of the workflow as failed. Its name is the error's; the error's name is
 * also in ErrorData as { "error": … }, so that a durable function that invoked this one can
 * tell which error it was.
 */
export class Failure extends DurableOperationError {
  errorType: string;
  constructor(error: string, cause: string) {
    super(cause, undefined, JSON.stringify({ error }));
    this.errorType = error;
    this.name = error;
  }
}

/** A retrier, as the state machine's Retry has it: the kinds it takes ("*" is every kind). */
export interface Retrier {
  on: readonly string[];
  max: number;
  every: number;
  backoff: number;
}

/** The steps do not retry by themselves; the workflow does, as its `retry` says. */
export const noRetry = () => ({ shouldRetry: false });

/** The kind of an error from a step, an invoke or a callback: a declared error, "timeout", or "failure". */
export function kindOf(e: unknown, declared: readonly string[]): string {
  if (e instanceof CallbackTimeoutError) return "timeout";
  if (e instanceof InvokeError) {
    // the invoked function's error comes by its ErrorData: { "error": … }
    try {
      const d = JSON.parse(e.errorData ?? "null");
      if (d && typeof d.error === "string" && declared.includes(d.error)) return d.error;
    } catch {
      // not a dandori failure
    }
    return "failure";
  }
  const cause = (e as { cause?: { name?: unknown } } | null)?.cause;
  const name = cause && typeof cause.name === "string" ? cause.name : e instanceof Error ? e.name : "";
  if (name === "Dandori.Timeout") return "timeout";
  if (declared.includes(name)) return name;
  return "failure";
}

function messageOf(e: unknown): string {
  const cause = (e as { cause?: { message?: unknown } } | null)?.cause;
  if (cause && typeof cause.message === "string") return cause.message;
  return e instanceof Error ? e.message : String(e);
}

/** Call once, and again as the retriers say, waiting between as Step Functions would. */
export async function attempt<R>(context: DurableContext, name: string, call: () => Promise<R>, retriers: readonly Retrier[], declared: readonly string[]): Promise<R> {
  const counts = retriers.map(() => 0);
  for (;;) {
    try {
      return await call();
    } catch (e) {
      const kind = kindOf(e, declared);
      let again = false;
      for (let i = 0; i < retriers.length; i++) {
        const r = retriers[i];
        if (r.on.includes(kind) || r.on.includes("*")) {
          if (counts[i] < r.max) {
            await context.wait(`${name} retry`, { seconds: Math.max(1, Math.ceil(r.every * Math.pow(r.backoff, counts[i]))) });
            counts[i]++;
            again = true;
          }
          break;
        }
      }
      if (!again) throw new TaskError(kind, messageOf(e));
    }
  }
}

/** A task's `timeout`, inside its step. */
export async function within<R>(p: Promise<R>, seconds: number | null): Promise<R> {
  if (seconds === null) return p;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const late = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      const e = new Error(`no answer in ${seconds} seconds`);
      e.name = "Dandori.Timeout";
      reject(e);
    }, seconds * 1000);
  });
  try {
    return await Promise.race([p, late]);
  } finally {
    clearTimeout(timer);
  }
}

/** A callback's answer comes as JSON text. */
export function parse(raw: unknown): unknown {
  return typeof raw === "string" ? JSON.parse(raw) : raw;
}

/** The idempotency key of a call: the execution, the call's place, and the round of each loop around it. */
export function key(context: DurableContext, site: number, rounds: readonly number[]): string {
  return [context.executionContext.durableExecutionArn, String(site), ...rounds.map(String)].join("/");
}

/** How many rounds of `for … in parallel` run at a time: all, when the `.flow` does not say. */
export function atATime(k: number): number | undefined {
  return k > 0 ? k : undefined;
}

/** A parallel round's end, as a value the checkpoint can keep: what it yields, or how it failed. */
export type Round<R> = { ok: R } | { fail: { task: boolean; kind: string; message: string } };

export function envelope(e: unknown): Round<never> {
  if (e instanceof TaskError) return { fail: { task: true, kind: e.kind, message: e.message } };
  if (e instanceof Error) return { fail: { task: false, kind: e.name, message: e.message } };
  return { fail: { task: false, kind: "Error", message: String(e) } };
}

/** What every round yields, in the list's order; or, when a round failed, the first such failure again. */
export function settle<R>(rounds: readonly Round<R>[]): R[] {
  for (const r of rounds) {
    if ("fail" in r) throw r.fail.task ? new TaskError(r.fail.kind, r.fail.message) : fail(r.fail.kind, r.fail.message);
  }
  return rounds.map((r) => (r as { ok: R }).ok);
}

/** A deliberate end of the workflow as failed. */
export function fail(error: string, cause: string | null): Error {
  return new Failure(error, cause ?? "");
}

/** A task's error that nothing handled ends the workflow with the error's kind. */
export function asFailure(e: unknown): unknown {
  return e instanceof TaskError ? fail(e.kind, e.message) : e;
}
"#;

const RUNTIME: &str = r#"// What the generated workflow code shares. It runs inside the workflow, so it uses nothing
// but @temporalio/workflow, and @temporalio/common's search attribute keys.

import { SearchAttributeType, defineSearchAttributeKey } from "@temporalio/common";
import {
  ActivityFailure,
  ApplicationFailure,
  ChildWorkflowFailure,
  TimeoutFailure,
  condition,
  defineQuery,
  defineSignal,
  defineUpdate,
  isCancellation,
  setHandler,
  sleep,
  upsertSearchAttributes,
  workflowInfo,
} from "@temporalio/workflow";

/** A task that failed after its retries, with the kind of error the `.flow` names it by. */
export class TaskError extends Error {
  readonly kind: string;
  constructor(kind: string, message: string) {
    super(message);
    this.kind = kind;
    this.name = "TaskError";
  }
}

/** A callback that got no answer in time. */
export class CallbackTimeout extends Error {
  constructor() {
    super("no answer in time");
    this.name = "CallbackTimeout";
  }
}

/** A callback answered with an error. */
export class CallbackError extends Error {
  readonly kind: string;
  constructor(kind: string, message: string) {
    super(message);
    this.kind = kind;
    this.name = "CallbackError";
  }
}

/** A retrier, as the state machine's Retry has it: the kinds it takes ("*" is every kind). */
export interface Retrier {
  on: readonly string[];
  max: number;
  every: number;
  backoff: number;
}

/**
 * The kind of an error from an activity, a child workflow or a callback: a declared error,
 * "timeout", or "failure". A local activity's failure comes as it is, not in an
 * ActivityFailure; so does its timeout when the workflow is replayed, though it came wrapped
 * the first time. Both ways read alike here, or a replay would go another way.
 */
export function kindOf(e: unknown, declared: readonly string[]): string {
  if (e instanceof CallbackTimeout) return "timeout";
  if (e instanceof CallbackError) return declared.includes(e.kind) ? e.kind : "failure";
  const c = e instanceof ActivityFailure || e instanceof ChildWorkflowFailure ? e.cause : e;
  if (c instanceof TimeoutFailure) return "timeout";
  if (c instanceof ApplicationFailure && c.type && declared.includes(c.type)) return c.type;
  return "failure";
}

function messageOf(e: unknown): string {
  if ((e instanceof ActivityFailure || e instanceof ChildWorkflowFailure) && e.cause) return e.cause.message;
  return e instanceof Error ? e.message : String(e);
}

/** Call once, and again as the retriers say, waiting between as Step Functions would. A cancellation is no task's error: it goes on as it is. */
export async function attempt<R>(call: () => Promise<R>, retriers: readonly Retrier[], declared: readonly string[]): Promise<R> {
  const counts = retriers.map(() => 0);
  for (;;) {
    try {
      return await call();
    } catch (e) {
      if (isCancellation(e)) throw e;
      const kind = kindOf(e, declared);
      let again = false;
      for (let i = 0; i < retriers.length; i++) {
        const r = retriers[i];
        if (r.on.includes(kind) || r.on.includes("*")) {
          if (counts[i] < r.max) {
            await sleep(ms(r.every * Math.pow(r.backoff, counts[i])));
            counts[i]++;
            again = true;
          }
          break;
        }
      }
      if (!again) throw new TaskError(kind, messageOf(e));
    }
  }
}

/** A duration of the workflow's timers, in milliseconds: every wait of the workflow goes through here. */
export function ms(seconds: number): number {
  return seconds * 1000;
}

/** How long it is until a moment given as an RFC 3339 string, by the workflow's clock, in milliseconds. */
export function until(at: string): number {
  return ms(Math.max(0, (Date.parse(at) - Date.now()) / 1000));
}

/** The idempotency key of a call: the workflow, the call's place, and the round of each loop around it. */
export function key(site: number, rounds: readonly number[]): string {
  return [workflowInfo().workflowId, String(site), ...rounds.map(String)].join("/");
}

/** The workflow id of a child workflow: this workflow's, the call's place, the rounds, and the try. */
export function childId(site: number, rounds: readonly number[], n: number): string {
  return [workflowInfo().workflowId, String(site), ...rounds.map(String), String(n)].join("/");
}

/** A callback's answer, as the signal brings it. */
export interface CallbackAnswer {
  callback_id: string;
  ok?: unknown;
  error?: string;
  message?: string;
}

/** The signal that brings a callback's answer. */
export const callbackSignal = defineSignal<[CallbackAnswer]>("dandori.callback");

/**
 * The update that brings a callback's answer, and tells the one who answers whether the workflow
 * took it: it refuses an answer for a callback the workflow does not wait for (an id it never
 * handed on, or one it gave up on), and a second answer.
 */
export const answerUpdate = defineUpdate<void, [CallbackAnswer]>("dandori.answer");

const answers = new Map<string, CallbackAnswer>();
/** The callbacks whose ids the workflow has handed on, and that it still waits for. */
const waiting = new Set<string>();

/** Take the answers of callbacks as they come. */
export function listen(): void {
  setHandler(callbackSignal, (a: CallbackAnswer) => {
    if (waiting.has(a.callback_id)) answers.set(a.callback_id, a);
  });
  setHandler(
    answerUpdate,
    (a: CallbackAnswer) => {
      answers.set(a.callback_id, a);
    },
    {
      validator: (a: CallbackAnswer) => {
        if (!waiting.has(a.callback_id)) throw new Error(`no callback waits for an answer with the id ${a.callback_id}`);
        if (answers.has(a.callback_id)) throw new Error(`the callback ${a.callback_id} has its answer already`);
      },
    },
  );
}

/** The id a callback task hands on: which workflow to answer, and which call it answers. */
export function callbackId(site: number, rounds: readonly number[], n: number): string {
  const id = JSON.stringify([workflowInfo().workflowId, [String(site), ...rounds.map(String), String(n)].join("/")]);
  waiting.add(id);
  return id;
}

/** Wait for the answer of the callback with this id. */
export async function awaitCallback(id: string, seconds: number): Promise<unknown> {
  const got = await condition(() => answers.has(id), ms(seconds));
  waiting.delete(id);
  if (!got) throw new CallbackTimeout();
  const a = answers.get(id)!;
  answers.delete(id);
  if (typeof a.error === "string") throw new CallbackError(a.error, a.message ?? "");
  return a.ok;
}

/** An event, as the update brings it: its name, and its value or the error it names. */
export interface EventAnswer {
  event: string;
  ok?: unknown;
  error?: string;
  message?: string;
}

/**
 * The update that brings an event, sent to the workflow by its id and the event's name (a task
 * that says `event`; client.ts: send). The workflow refuses an event it does not wait for now,
 * and a second one: the one who sends it learns so, and can send it again when it waits.
 */
export const eventUpdate = defineUpdate<void, [EventAnswer]>("dandori.event");

const events = new Map<string, EventAnswer>();
/** The events the workflow waits for now. */
const awaited = new Set<string>();

/** Take the events the workflow waits for as they come. */
export function listenForEvents(): void {
  setHandler(
    eventUpdate,
    (a: EventAnswer) => {
      events.set(a.event, a);
    },
    {
      validator: (a: EventAnswer) => {
        if (!awaited.has(a.event)) throw new Error(`the workflow does not wait for the event ${a.event} now`);
        if (events.has(a.event)) throw new Error(`the event ${a.event} has come already`);
      },
    },
  );
}

/** Wait for the event with this name, at most `seconds`. */
export async function awaitEvent(name: string, seconds: number): Promise<unknown> {
  awaited.add(name);
  let got: boolean;
  try {
    got = await condition(() => events.has(name), ms(seconds));
  } finally {
    awaited.delete(name);
  }
  if (!got) throw new CallbackTimeout();
  const a = events.get(name)!;
  events.delete(name);
  if (typeof a.error === "string") throw new CallbackError(a.error, a.message ?? "");
  return a.ok;
}

/**
 * What the query `dandori.status` answers: the line of the call or the wait the workflow is at,
 * each case's state (null before it starts), and the events it waits for now.
 */
export interface Status {
  at: number | null;
  cases: Record<string, string | null>;
  events: string[];
}

export const statusQuery = defineQuery<Status>("dandori.status");

let where: number | null = null;
let cases: () => Record<string, string | null> = () => ({});

/** The workflow is at the call or the wait on this line. */
export function at(line: number): void {
  where = line;
}

/** Answer the query `dandori.status`, reading the cases' states with `read`. */
export function report(read: () => Record<string, string | null>): void {
  cases = read;
  setHandler(statusQuery, () => ({ at: where, cases: cases(), events: [...awaited].sort() }));
}

/**
 * The search attribute that lists the cases' states, as "<case>=<state>". The workflow keeps it
 * up to date only when it was started so (client.ts: start(…, { searchAttributes: true }), which
 * puts `dandori.cases` in the memo), since it must be registered on the namespace first.
 */
export const CASES = defineSearchAttributeKey("DandoriCases", SearchAttributeType.KEYWORD_LIST);

/** A case moved: show the cases' states in the search attribute, when the workflow was started so. */
export function shown(): void {
  if (workflowInfo().memo?.["dandori.cases"] !== true) return;
  const now = Object.entries(cases())
    .filter(([, state]) => state !== null)
    .map(([name, state]) => `${name}=${state}`);
  upsertSearchAttributes([{ key: CASES, value: now }]);
}

/**
 * What a run hands on to the run that goes on from it (Continue-As-New): at which loop at the top
 * of the flow it goes on (the first such loop is 1), from which round, the variables, and the
 * loop's list and what the loop has yielded so far.
 */
export interface Resume {
  at: number;
  round: number;
  vars: Record<string, unknown>;
  items?: unknown[];
  out?: unknown[];
}

/**
 * How many events a run's history has before a loop at the top of the flow goes on in a new run,
 * at the start of a round. The server suggests it sooner when the history grows large.
 */
export const CONTINUE_AT = 10000;

/** Whether a loop at the top of the flow should go on in a new run: the history is long, or the server suggests it. */
export function historyIsLong(): boolean {
  const info = workflowInfo();
  return info.continueAsNewSuggested || info.historyLength >= CONTINUE_AT;
}

/** What a run that goes on at the `at`-th loop was handed (the round, the list, or what the loop yielded), or else what `fresh` makes. */
export function carried<T>(resume: Resume | null, at: number, what: "round" | "items" | "out", fresh: () => T): T {
  return resume !== null && resume.at === at ? (resume[what] as T) : fresh();
}

/** How many rounds of `for … in parallel` run at a time: all, when the `.flow` does not say. */
export function atATime(k: number): number {
  return k;
}

/**
 * Run a round for every item, `k` at a time (0: all at once). Every round runs to its end;
 * then what they yield comes back in the list's order, or the first failure by place in
 * the list is thrown again.
 */
export async function rounds<I, R>(items: readonly I[], k: number, round: (item: I, index: number) => Promise<R>): Promise<R[]> {
  const out: Array<{ ok: R } | { fail: unknown }> = new Array(items.length);
  let next = 0;
  const lanes = k > 0 ? Math.min(k, items.length) : items.length;
  async function lane(): Promise<void> {
    for (;;) {
      const i = next++;
      if (i >= items.length) return;
      try {
        out[i] = { ok: await round(items[i], i) };
      } catch (e) {
        out[i] = { fail: e };
      }
    }
  }
  await Promise.all(Array.from({ length: lanes }, () => lane()));
  // a cancellation stops every round, whatever failed before it
  const cancelled = out.find((r) => "fail" in r && isCancellation(r.fail));
  if (cancelled) throw (cancelled as { fail: unknown }).fail;
  for (const r of out) {
    if ("fail" in r) throw r.fail;
  }
  return out.map((r) => (r as { ok: R }).ok);
}

/** A deliberate end of the workflow as failed. */
export function fail(error: string, cause: string | null): ApplicationFailure {
  return ApplicationFailure.create({ type: error, message: cause ?? "", nonRetryable: true });
}

/** A task's error that nothing handled ends the workflow with the error's kind. */
export function asFailure(e: unknown): unknown {
  return e instanceof TaskError ? fail(e.kind, e.message) : e;
}
"#;

struct Gen<'a> {
    m: &'a Model,
    out: String,
    /// the counters of the loops around the current statement
    loops: Vec<usize>,
    flavor: Flavor,
    /// durable functions: the context the current code calls through
    ctx: String,
    /// Temporal: the loop at the top of the flow that the next statement is, which goes on in a
    /// new run once the history is long, by its place among such loops (the first is 1)
    can: Option<usize>,
}

impl<'a> Gen<'a> {
    fn line(&mut self, depth: usize, s: &str) {
        for _ in 0..depth {
            self.out.push_str("  ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn var(&self, name: &str) -> String {
        ident(name)
    }

    fn expr(&self, e: &TExpr) -> String {
        match e {
            TExpr::Str(s) => q(s),
            TExpr::Int(n) => n.to_string(),
            TExpr::Bool(b) => b.to_string(),
            TExpr::Enum(v, _) => q(v),
            TExpr::None(_) => "null".into(),
            TExpr::Var { name, fields, ty } => {
                let mut s = self.var(name);
                let is_input = self.m.inputs.iter().any(|(i, _)| i == name);
                // a variable is declared with null in it until it is set; the checker says it is set here
                if fields.is_empty() && !is_input && !matches!(ty, Ty::Opt(_)) {
                    s.push('!');
                }
                for (i, f) in fields.iter().enumerate() {
                    if i == 0 && !is_input {
                        s.push('!');
                    }
                    s.push_str(&format!("[{}]", q(f)));
                }
                if !fields.is_empty() && matches!(ty, Ty::Opt(_)) {
                    // an absent field reads as null
                    s = format!("({s} ?? null)");
                }
                s
            }
            TExpr::Record { fields, .. } => format!("{{ {} }}", fields.iter().map(|(f, x)| format!("{}: {}", q(f), self.expr(x))).collect::<Vec<_>>().join(", ")),
            TExpr::List { items, .. } => format!("[{}]", items.iter().map(|x| self.expr(x)).collect::<Vec<_>>().join(", ")),
            TExpr::Interp(parts) => {
                let mut out: Vec<String> = Vec::new();
                for p in parts {
                    match p {
                        IPart::Lit(s) => out.push(q(s)),
                        IPart::Hole(x) => out.push(format!("String({})", self.expr(x))),
                    }
                }
                if !matches!(parts.first(), Some(IPart::Lit(_))) {
                    out.insert(0, "\"\"".into());
                }
                format!("({})", out.join(" + "))
            }
        }
    }

    fn workflow(&mut self, header: &str, rules: bool) -> String {
        let m = self.m;
        self.out.push_str(header);
        self.out.push_str(&format!("// {} v{}{}\n\n", m.name, m.version, if m.description.is_empty() { String::new() } else { format!(": {}", m.description) }));
        let ret = if m.outputs.is_empty() { "null".to_string() } else { "T.WorkflowOutput".to_string() };
        if self.flavor == Flavor::Durable {
            self.out.push_str("import { StepSemantics, withDurableExecution, type DurableContext } from \"@aws/durable-execution-sdk-js\";\n");
            self.out.push_str("import { makeTasks, type OwnTasks, type Tasks } from \"./tasks\";\n");
            self.out.push_str("import * as io from \"./io\";\n");
            self.out.push_str("import * as T from \"./types\";\n");
            self.out.push_str("import * as dd from \"./runtime\";\n\n");
            self.out.push_str("/** The Lambda handler: `export const handler = makeHandler(yourTasks);` */\n");
            self.out.push_str("export function makeHandler(own: OwnTasks, transport: io.Transport = io.transport()) {\n");
            self.out.push_str("  const tasks = makeTasks(own, transport);\n");
            self.out.push_str(&format!("  return withDurableExecution(async (input: T.WorkflowInput, context: DurableContext): Promise<{ret}> => {{\n"));
            self.out.push_str("    try {\n      return await run(input, context, tasks);\n    } catch (e) {\n      throw dd.asFailure(e);\n    }\n  });\n}\n\n");
            self.out.push_str(&format!("async function run(input: T.WorkflowInput, context: DurableContext, tasks: Tasks): Promise<{ret}> {{\n"));
            self.body();
            return std::mem::take(&mut self.out);
        }
        let callbacks = m.tasks.iter().any(|t| t.callback);
        let children = m.tasks.iter().any(|t| t.is_child(Platform::Temporal));
        let called: BTreeSet<usize> = m.all_stmts().iter().filter_map(|s| match &s.kind { TK::Call { callee: Callee::Rule(r), .. } => Some(*r), _ => None }).collect();
        let local_rules = called.iter().any(|r| m.rules[*r].local);
        let other_rules = called.iter().any(|r| !m.rules[*r].local);
        let mut names = vec!["proxyActivities", "sleep"];
        if local_rules {
            names.insert(1, "proxyLocalActivities");
        }
        if self.resumes() {
            names.insert(0, "continueAsNew");
        }
        if children {
            names.push("executeChild");
        }
        if m.on_cancel.is_some() {
            names.extend(["CancellationScope", "isCancellation"]);
        }
        self.out.push_str(&format!("import {{ {} }} from \"@temporalio/workflow\";\n", names.join(", ")));
        self.out.push_str("import type { Tasks } from \"./activities\";\n");
        if rules {
            self.out.push_str("import type { rules } from \"./rules\";\n");
        }
        self.out.push_str("import * as T from \"./types\";\n");
        self.out.push_str("import * as dd from \"./runtime\";\n\n");
        if callbacks {
            self.out.push_str("/** The signal that brings a callback's answer. */\nexport const callbackSignal = dd.callbackSignal;\n\n");
        }
        // one proxy per task, so each has its own timeout and queue; Temporal does not retry, the
        // workflow does; the worker of the workflow heartbeats from the tasks it serves
        for t in &m.tasks {
            if t.is_child(Platform::Temporal) || t.event {
                continue;
            }
            let timeout = activity_timeout(t);
            let rest = match &t.queue {
                Some(qn) => format!(", taskQueue: {}", q(qn)),
                None => format!(", heartbeatTimeout: \"{HEARTBEAT_SECONDS} seconds\""),
            };
            self.out.push_str(&format!(
                "const {} = proxyActivities<Tasks>({{ startToCloseTimeout: \"{timeout} seconds\"{rest}, retry: {{ maximumAttempts: 1 }} }}).{};\n",
                ident(&format!("task_{}", t.name)),
                ident(&t.name)
            ));
        }
        if rules && other_rules {
            self.out.push_str(&format!("const rule_calls = proxyActivities<typeof rules>({{ startToCloseTimeout: \"{RULE_SECONDS} seconds\", retry: {{ maximumAttempts: 1 }} }});\n"));
        }
        if rules && local_rules {
            // the rules that say `local` run in the worker that runs the workflow; the history keeps each answer as a marker
            self.out.push_str(&format!("const local_rule_calls = proxyLocalActivities<typeof rules>({{ startToCloseTimeout: \"{RULE_SECONDS} seconds\", retry: {{ maximumAttempts: 1 }} }});\n"));
        }
        self.out.push('\n');
        let fname = workflow_type(m);
        if self.resumes() {
            // a run that goes on from an earlier one gets what that one handed on
            self.out.push_str(&format!("export async function {fname}(input: T.WorkflowInput, resume: dd.Resume | null = null): Promise<{ret}> {{\n"));
        } else {
            self.out.push_str(&format!("export async function {fname}(input: T.WorkflowInput): Promise<{ret}> {{\n"));
        }
        if callbacks {
            self.out.push_str("  dd.listen();\n");
        }
        if m.tasks.iter().any(|t| t.event) {
            self.out.push_str("  dd.listenForEvents();\n");
        }
        let args = if self.resumes() { "input, resume" } else { "input" };
        self.out.push_str(&format!("  try {{\n    return await run({args});\n  }} catch (e) {{\n    throw dd.asFailure(e);\n  }}\n}}\n\n"));
        if self.resumes() {
            self.out.push_str(&format!("async function run(input: T.WorkflowInput, dd_resume: dd.Resume | null): Promise<{ret}> {{\n"));
        } else {
            self.out.push_str(&format!("async function run(input: T.WorkflowInput): Promise<{ret}> {{\n"));
        }
        self.body();
        std::mem::take(&mut self.out)
    }

    /// The body of `run`: the input's check, the variables, the flow and `on failure`.
    fn body(&mut self) {
        let m = self.m;
        self.line(1, "if (!T.is_WorkflowInput(input)) throw dd.fail(\"Dandori.BadInput\", \"the execution's input does not have the declared shape\");");
        let locals = crate::asl::parallel_locals(m);
        for (v, ty) in &m.vars {
            if m.inputs.iter().any(|(i, _)| i == v) {
                self.line(1, &format!("const {}: {} = input[{}];", self.var(v), ts_type(m, ty), q(v)));
            } else if !locals.contains(v) {
                let t = if matches!(ty, Ty::Opt(_)) { ts_type(m, ty) } else { format!("{} | null", ts_type(m, ty)) };
                self.line(1, &format!("let {}: {t} = null;", self.var(v)));
            }
        }
        if self.resumes() {
            let carried: Vec<&String> = m.vars.iter().map(|(v, _)| v).filter(|v| !m.inputs.iter().any(|(i, _)| i == *v) && !locals.contains(*v)).collect();
            self.line(1, "// a run that goes on from an earlier one (Continue-As-New) takes up its variables, and starts at its loop");
            self.line(1, "const dd_from = dd_resume === null ? 0 : dd_resume.at;");
            if !carried.is_empty() {
                self.line(1, "if (dd_resume !== null) {");
                for v in &carried {
                    let x = self.var(v);
                    self.line(2, &format!("{x} = dd_resume.vars[{}] as typeof {x};", q(v)));
                }
                self.line(1, "}");
            }
            let vars: Vec<String> = carried.iter().map(|v| format!("{}: {}", q(v), self.var(v))).collect();
            if vars.is_empty() {
                self.line(1, "const dd_vars = () => ({});");
            } else {
                self.line(1, &format!("const dd_vars = () => ({{ {} }});", vars.join(", ")));
            }
        }
        if self.flavor == Flavor::Temporal {
            // what the query `dandori.status` and the search attribute say of the cases
            let cases: Vec<String> = m
                .cases
                .iter()
                .map(|c| {
                    let v = self.var(&c.name);
                    format!("{}: {v} === null ? null : {v}[{}]", q(&c.name), q(&c.state_field))
                })
                .collect();
            if cases.is_empty() {
                self.line(1, "dd.report(() => ({}));");
            } else {
                self.line(1, &format!("dd.report(() => ({{ {} }}));", cases.join(", ")));
            }
        }
        // a cancellation reaches past `on failure` to `on cancel`
        let d = if m.on_cancel.is_some() && self.flavor == Flavor::Temporal {
            self.line(1, "try {");
            2
        } else {
            1
        };
        // a flow with outputs ends with `succeed` or `fail` (the checker says so), never at its end
        let end = if m.outputs.is_empty() { "return null;" } else { "throw dd.fail(\"Dandori.NoOutput\", \"the flow ended without succeed\");" };
        match &m.on_failure {
            Some(block) => {
                self.line(d, "try {");
                self.flow(d + 1);
                self.line(d + 1, end);
                self.line(d, "} catch (dd_e) {");
                self.line(d + 1, "if (!(dd_e instanceof dd.TaskError)) throw dd_e;");
                self.line(d + 1, "// on failure");
                self.block(block, d + 1);
                self.line(d + 1, "throw dd_e;");
                self.line(d, "}");
            }
            None => {
                self.flow(d);
                self.line(d, end);
            }
        }
        if let (Some(block), Flavor::Temporal) = (&m.on_cancel, self.flavor) {
            self.line(1, "} catch (dd_c) {");
            self.line(2, "if (!isCancellation(dd_c)) throw dd_c;");
            self.line(2, "// on cancel: out of the cancellation's reach; then the workflow ends as cancelled");
            self.line(2, "await CancellationScope.nonCancellable(async () => {");
            self.block(block, 3);
            self.line(2, "});");
            self.line(2, "throw dd_c;");
            self.line(1, "}");
        }
        self.out.push_str("}\n");
    }

    fn block(&mut self, ss: &[TStmt], d: usize) {
        for s in ss {
            self.stmt(s, d);
            if matches!(s.kind, TK::Succeed { .. } | TK::Fail { .. } | TK::Break) {
                break;
            }
        }
    }

    /// Temporal: whether a loop at the top of the flow goes on in a new run once the history is long.
    fn resumes(&self) -> bool {
        self.flavor == Flavor::Temporal && self.m.flow.iter().any(crate::check::continues_as_new)
    }

    /// The flow's statements. On Temporal, a loop at the top of the flow goes on in a new run once
    /// the history is long (Continue-As-New); a run that goes on so starts at that loop, the
    /// `dd_from`-th such loop, and skips what comes before it.
    fn flow(&mut self, d: usize) {
        let ss = &self.m.flow;
        if !self.resumes() {
            self.block(ss, d);
            return;
        }
        let total = ss.iter().filter(|s| crate::check::continues_as_new(s)).count();
        let mut k = 0;
        // the statements under the `if` that is open run when dd_from is at most this
        let mut open: Option<usize> = None;
        for s in ss {
            let can = crate::check::continues_as_new(s);
            let guard = if can { Some(k + 1) } else if k < total { Some(k) } else { None };
            if open != guard {
                if open.is_some() {
                    self.line(d, "}");
                }
                match guard {
                    Some(0) => self.line(d, "if (dd_resume === null) {"),
                    Some(g) => self.line(d, &format!("if (dd_from <= {g}) {{")),
                    None => {}
                }
                open = guard;
            }
            if can {
                k += 1;
                self.can = Some(k);
            }
            self.stmt(s, if open.is_some() { d + 1 } else { d });
            if matches!(s.kind, TK::Succeed { .. } | TK::Fail { .. } | TK::Break) {
                break;
            }
        }
        if open.is_some() {
            self.line(d, "}");
        }
    }

    /// At the start of a round of the `k`-th loop at the top of the flow but its first in this
    /// run: once the history is long, go on in a new run, with the variables, the round, and
    /// `more` (the loop's list, and what it has yielded).
    fn continue_as_new(&mut self, d: usize, site: usize, k: usize, more: &str) {
        let fname = workflow_type(self.m);
        self.line(
            d,
            &format!("if (dd_loop_{site} > dd_first_{site} && dd.historyIsLong()) await continueAsNew<typeof {fname}>(input, {{ at: {k}, round: dd_loop_{site}, vars: dd_vars(){more} }});"),
        );
    }

    fn rounds_expr(&self) -> String {
        let rounds: Vec<String> = self.loops.iter().map(|l| format!("dd_loop_{l}")).collect();
        format!("[{}]", rounds.join(", "))
    }

    fn stmt(&mut self, s: &TStmt, d: usize) {
        let m = self.m;
        let ctx = self.ctx.clone();
        match &s.kind {
            TK::Pass => {}
            TK::Break => {
                let site = *self.loops.last().expect("lowering keeps break inside a loop");
                self.line(d, &format!("break loop_{site};"));
            }
            TK::Wait { seconds } => match self.flavor {
                Flavor::Temporal => {
                    self.line(d, &format!("dd.at({});", s.line));
                    self.line(d, &format!("await sleep(dd.ms({seconds}));"));
                }
                Flavor::Durable | Flavor::Argo => self.line(d, &format!("await {ctx}.wait({}, {{ seconds: {seconds} }});", q(&format!("{} wait", s.line)))),
            },
            TK::WaitUntil { at } => {
                let x = self.expr(at);
                match self.flavor {
                    Flavor::Temporal => {
                        self.line(d, &format!("dd.at({});", s.line));
                        self.line(d, &format!("await sleep(dd.until({x}));"));
                    }
                    Flavor::Durable | Flavor::Argo => {
                        // the clock is read in a step, so that a replay reads the same moment
                        let site = s.site;
                        self.line(d, &format!("const dd_now_{site} = await {ctx}.step({}, async () => Date.now(), {{ retryStrategy: dd.noRetry }});", q(&format!("{} now", s.line))));
                        self.line(d, &format!("const dd_wait_{site} = Math.ceil((Date.parse({x}) - dd_now_{site}) / 1000);"));
                        self.line(d, &format!("if (dd_wait_{site} > 0) await {ctx}.wait({}, {{ seconds: dd_wait_{site} }});", q(&format!("{} wait until", s.line))));
                    }
                }
            }
            TK::Assign { name, expr } => {
                let x = self.expr(expr);
                self.line(d, &format!("{} = {x};", self.var(name)));
            }
            TK::Succeed { fields } => {
                if fields.is_empty() {
                    self.line(d, "return null;");
                } else {
                    let parts: Vec<String> = fields.iter().map(|(f, e)| format!("{}: {}", q(f), self.expr(e))).collect();
                    self.line(d, &format!("return {{ {} }};", parts.join(", ")));
                }
            }
            TK::Fail { error, cause, .. } => {
                let c = cause.as_ref().map(|c| self.expr(c)).unwrap_or_else(|| "null".into());
                self.line(d, &format!("throw dd.fail({}, {c});", q(error)));
            }
            TK::Repeat { times, body } => {
                let site = s.site;
                let can = self.can.take();
                if let Some(k) = can {
                    self.line(d, &format!("// line {}: a round starts in a new run once the history is long", s.line));
                    self.line(d, &format!("const dd_first_{site} = dd.carried(dd_resume, {k}, \"round\", () => 0);"));
                    self.line(d, &format!("loop_{site}: for (let dd_loop_{site} = dd_first_{site}; dd_loop_{site} < {times}; dd_loop_{site}++) {{"));
                    self.continue_as_new(d + 1, site, k, "");
                } else {
                    self.line(d, &format!("loop_{site}: for (let dd_loop_{site} = 0; dd_loop_{site} < {times}; dd_loop_{site}++) {{"));
                }
                self.loops.push(site);
                self.block(body, d + 1);
                self.loops.pop();
                self.line(d, "}");
            }
            TK::For { var, list, max, parallel, body, result, locals } => {
                let site = s.site;
                let can = self.can.take();
                let items = format!("dd_items_{site}");
                self.line(d, "{");
                self.line(d + 1, &format!("// line {}: for {var} in {}", s.line, list.show()));
                let lx = self.expr(list);
                match can {
                    // a run that goes on in the loop takes up the list the loop began with
                    Some(k) => self.line(d + 1, &format!("const {items} = dd.carried(dd_resume, {k}, \"items\", () => {lx});")),
                    None => self.line(d + 1, &format!("const {items} = {lx};")),
                }
                self.line(
                    d + 1,
                    &format!("if ({items}.length > {max}) throw dd.fail(\"Dandori.TooManyItems\", {});", q(&format!("line {}: the list has more than {max} items", s.line))),
                );
                match parallel {
                    None => {
                        match (result, can) {
                            (Some(_), Some(k)) => self.line(d + 1, &format!("const dd_out_{site}: unknown[] = dd.carried(dd_resume, {k}, \"out\", () => []);")),
                            (Some(_), None) => self.line(d + 1, &format!("const dd_out_{site}: unknown[] = [];")),
                            (None, _) => {}
                        }
                        if let Some(k) = can {
                            self.line(d + 1, "// a round starts in a new run once the history is long");
                            self.line(d + 1, &format!("const dd_first_{site} = dd.carried(dd_resume, {k}, \"round\", () => 0);"));
                            self.line(d + 1, &format!("loop_{site}: for (let dd_loop_{site} = dd_first_{site}; dd_loop_{site} < {items}.length; dd_loop_{site}++) {{"));
                            let more = if result.is_some() { format!(", items: {items}, out: dd_out_{site}") } else { format!(", items: {items}") };
                            self.continue_as_new(d + 2, site, k, &more);
                        } else {
                            self.line(d + 1, &format!("loop_{site}: for (let dd_loop_{site} = 0; dd_loop_{site} < {items}.length; dd_loop_{site}++) {{"));
                        }
                        self.line(d + 2, &format!("{} = {items}[dd_loop_{site}];", self.var(var)));
                        self.loops.push(site);
                        self.block(body, d + 2);
                        let ends = body.last().map(|x| matches!(x.kind, TK::Succeed { .. } | TK::Fail { .. } | TK::Break)).unwrap_or(false);
                        if let (Some((_, y)), false) = (result, ends) {
                            let yv = self.expr(y);
                            self.line(d + 2, &format!("dd_out_{site}.push({yv});"));
                        }
                        self.loops.pop();
                        self.line(d + 1, "}");
                        if let Some((r, _)) = result {
                            self.line(d + 1, &format!("{} = dd_out_{site} as any;", self.var(r)));
                        }
                    }
                    Some(k) => {
                        let elem = match list.ty() {
                            Ty::List(t) => ts_type(m, &t),
                            _ => "unknown".into(),
                        };
                        let declare = |g: &mut Gen, d: usize| {
                            g.line(d, &format!("let {}: {} = dd_item;", g.var(var), elem));
                            for l in locals {
                                if l != var {
                                    let t = match m.var_ty(l) {
                                        Some(t @ Ty::Opt(_)) => ts_type(m, t),
                                        Some(t) => format!("{} | null", ts_type(m, t)),
                                        None => "unknown".into(),
                                    };
                                    g.line(d, &format!("let {}: {t} = null;", g.var(l)));
                                }
                            }
                        };
                        let yv = |g: &Gen| result.as_ref().map(|(_, y)| g.expr(y)).unwrap_or_else(|| "null".into());
                        match self.flavor {
                            Flavor::Temporal => {
                                self.line(d + 1, &format!("const dd_res_{site} = await dd.rounds({items}, dd.atATime({k}), async (dd_item, dd_loop_{site}) => {{"));
                                declare(self, d + 2);
                                self.loops.push(site);
                                self.block(body, d + 2);
                                self.loops.pop();
                                let y = yv(self);
                                self.line(d + 2, &format!("return {y};"));
                                self.line(d + 1, "});");
                            }
                            Flavor::Durable | Flavor::Argo => {
                                let round_ctx = format!("dd_ctx_{site}");
                                self.line(
                                    d + 1,
                                    &format!("const dd_map_{site} = await {ctx}.map({}, {items}, async ({round_ctx}, dd_item, dd_loop_{site}) => {{", q(&format!("{} for {var}", s.line))),
                                );
                                self.line(d + 2, "try {");
                                declare(self, d + 3);
                                self.loops.push(site);
                                let outer = std::mem::replace(&mut self.ctx, round_ctx);
                                self.block(body, d + 3);
                                self.ctx = outer;
                                self.loops.pop();
                                let y = yv(self);
                                self.line(d + 3, &format!("return {{ ok: {y} }};"));
                                self.line(d + 2, "} catch (dd_e) {");
                                self.line(d + 3, "return dd.envelope(dd_e);");
                                self.line(d + 2, "}");
                                self.line(d + 1, &format!("}}, {{ maxConcurrency: dd.atATime({k}) }});"));
                                self.line(d + 1, &format!("const dd_res_{site} = dd.settle(dd_map_{site}.getResults());"));
                            }
                        }
                        if let Some((r, _)) = result {
                            self.line(d + 1, &format!("{} = dd_res_{site} as any;", self.var(r)));
                        }
                    }
                }
                self.line(d, "}");
            }
            TK::Match { expr, arms } => {
                let x = match expr {
                    TExpr::Var { name, fields, .. } if m.case_index(name).is_some() => {
                        let v = self.var(name);
                        let mut s = v.clone();
                        for f in fields {
                            s.push_str(&format!("[{}]", q(f)));
                        }
                        format!("{v} === null ? null : {s}")
                    }
                    other => self.expr(other),
                };
                self.line(d, "{");
                self.line(d + 1, &format!("const dd_v: any = {x};"));
                for (i, a) in arms.iter().enumerate() {
                    let mut parts = Vec::new();
                    if a.none {
                        parts.push("dd_v === null || dd_v === undefined".to_string());
                    }
                    if a.some.is_some() {
                        parts.push("dd_v !== null && dd_v !== undefined".to_string());
                    }
                    if !a.values.is_empty() {
                        if expr.ty().inner() == &Ty::Bool {
                            for v in &a.values {
                                parts.push(format!("dd_v === {v}"));
                            }
                        } else {
                            parts.push(format!("[{}].includes(dd_v as string)", a.values.iter().map(|v| q(v)).collect::<Vec<_>>().join(", ")));
                        }
                    }
                    let kw = if i == 0 { "if" } else { "} else if" };
                    self.line(d + 1, &format!("{kw} ({}) {{", parts.join(" || ")));
                    if let Some(v) = &a.some {
                        self.line(d + 2, &format!("{} = dd_v;", self.var(v)));
                    }
                    self.block(&a.body, d + 2);
                }
                self.line(d + 1, "} else {");
                self.line(d + 2, &format!("throw dd.fail(\"Dandori.UnexpectedValue\", {});", q(&format!("line {}: {} took a value that no arm names", s.line, expr.show()))));
                self.line(d + 1, "}");
                self.line(d, "}");
            }
            TK::Call { target, callee, args, handlers } => self.call(s, target.as_ref(), callee, args, handlers, d),
        }
    }

    fn retriers(&self, callee: &Callee) -> String {
        let m = self.m;
        let v: Value = match callee {
            Callee::Rule(_) => crate::asl::rule_retriers(),
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
            out.push(format!(
                "{{ on: [{}], max: {}, every: {}, backoff: {} }}",
                kinds.iter().map(|k| q(k)).collect::<Vec<_>>().join(", "),
                r["MaxAttempts"].as_u64().unwrap_or(3),
                r["IntervalSeconds"].as_u64().unwrap_or(1),
                r["BackoffRate"].as_f64().unwrap_or(2.0)
            ));
        }
        format!("[{}]", out.join(", "))
    }

    fn call(&mut self, s: &TStmt, target: Option<&Target>, callee: &Callee, args: &[(String, TExpr)], handlers: &[THandler], d: usize) {
        let m = self.m;
        let site = s.site;
        let ctx = self.ctx.clone();
        let p = self.flavor.platform();
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
                let c = if self.flavor == Flavor::Durable { format!("{ctx}, ") } else { String::new() };
                parts.push(format!("\"idempotency_key\": dd.key({c}{site}, {})", self.rounds_expr()));
            }
        }
        let retriers = self.retriers(callee);
        let op = q(&format!("{} {}", s.line, cname));
        let declared = declared.join(", ");
        let i = "  ".repeat(d + 2);
        let invocation = match (self.flavor, callee) {
            (Flavor::Argo, _) => unreachable!("the Argo build writes no workflow code in TypeScript"),
            (Flavor::Temporal, Callee::Rule(r)) => {
                let calls = if m.rules[*r].local { "local_rule_calls" } else { "rule_calls" };
                format!("dd.attempt(() => {calls}.{}({{ {} }}), {retriers}, [])", render::rule_activity(&m.rules[*r].name), parts.join(", "))
            }
            (Flavor::Durable, Callee::Rule(r)) => {
                let arn = q(m.rules[*r].lambda.as_deref().unwrap_or(""));
                format!("dd.attempt({ctx}, {op}, () => {ctx}.invoke({op}, {arn}, {{ {} }}), {retriers}, [])", parts.join(", "))
            }
            (Flavor::Temporal, Callee::Task(t)) => {
                let task = &m.tasks[*t];
                let f = ident(&format!("task_{}", task.name));
                match task.via(p) {
                    Some(Via::Workflow(ty)) => {
                        let mut opts = vec![format!("args: [{{ {} }}]", parts.join(", ")), format!("workflowId: dd.childId({site}, {}, ++dd_n)", self.rounds_expr())];
                        if let Some(qn) = &task.queue {
                            opts.push(format!("taskQueue: {}", q(qn)));
                        }
                        if let Some(t) = task.timeout {
                            opts.push(format!("workflowExecutionTimeout: \"{t} seconds\""));
                        }
                        format!("dd.attempt(() => executeChild({}, {{ {} }}), {retriers}, [{declared}])", q(ty), opts.join(", "))
                    }
                    // nothing is called: the workflow waits for the event, as long as a callback
                    Some(Via::Event) => format!("dd.attempt(() => dd.awaitEvent({}, {}), {retriers}, [{declared}])", q(&task.name), task.timeout.unwrap_or(86_400)),
                    _ if task.callback => {
                        let timeout = task.timeout.unwrap_or(86_400);
                        let mut with_id = parts.clone();
                        with_id.push("\"callback_id\": dd_id".into());
                        format!(
                            "dd.attempt(async () => {{\n{i}  const dd_id = dd.callbackId({site}, {}, ++dd_n);\n{i}  await {f}({{ {} }});\n{i}  return await dd.awaitCallback(dd_id, {timeout});\n{i}}}, {retriers}, [{declared}])",
                            self.rounds_expr(),
                            with_id.join(", ")
                        )
                    }
                    _ => format!("dd.attempt(() => {f}({{ {} }}), {retriers}, [{declared}])", parts.join(", ")),
                }
            }
            (Flavor::Durable, Callee::Task(t)) => {
                let task = &m.tasks[*t];
                let method = ident(&task.name);
                match task.via(p) {
                    Some(Via::DurableFunction(f)) => format!("dd.attempt({ctx}, {op}, () => {ctx}.invoke({op}, {}, {{ {} }}), {retriers}, [{declared}])", q(f), parts.join(", ")),
                    _ if task.callback => {
                        let timeout = task.timeout.unwrap_or(86_400);
                        let submit = q(&format!("{} {} submit", s.line, cname));
                        let mut with_id = parts.clone();
                        with_id.push("\"callback_id\": dd_id".into());
                        format!(
                            "dd.attempt({ctx}, {op}, async () => {{\n{i}  const [dd_p, dd_id] = await {ctx}.createCallback({op}, {{ timeout: {{ seconds: {timeout} }} }});\n{i}  await {ctx}.step({submit}, async () => {{\n{i}    await tasks.{method}({{ {} }});\n{i}    return null;\n{i}  }}, {{ retryStrategy: dd.noRetry }});\n{i}  return dd.parse(await dd_p);\n{i}}}, {retriers}, [{declared}])",
                            with_id.join(", ")
                        )
                    }
                    _ => {
                        // a call that changes things and carries no key must not run twice unseen
                        let semantics = if task.changes_things() && !task.key { "AtMostOncePerRetry" } else { "AtLeastOncePerRetry" };
                        let timeout = task.timeout.map(|t| t.to_string()).unwrap_or_else(|| "null".into());
                        format!(
                            "dd.attempt({ctx}, {op}, () => {ctx}.step({op}, async () => dd.within(tasks.{method}({{ {} }}), {timeout}), {{ retryStrategy: dd.noRetry, semantics: StepSemantics.{semantics} }}), {retriers}, [{declared}])",
                            parts.join(", ")
                        )
                    }
                }
            }
        };
        let counted = match callee {
            Callee::Task(t) => {
                let task = &m.tasks[*t];
                self.flavor == Flavor::Temporal && (task.callback || matches!(task.via(p), Some(Via::Workflow(_))))
            }
            _ => false,
        };
        self.line(d, &format!("call_{site}: {{"));
        self.line(d + 1, &format!("// line {}: {cname}", s.line));
        if self.flavor == Flavor::Temporal {
            self.line(d + 1, &format!("dd.at({});", s.line));
        }
        if counted {
            self.line(d + 1, "let dd_n = 0;");
        }
        self.line(d + 1, "let dd_r: any;");
        self.line(d + 1, "try {");
        self.line(d + 2, &format!("dd_r = await {invocation};"));
        self.line(d + 1, "} catch (dd_e) {");
        self.line(d + 2, "if (!(dd_e instanceof dd.TaskError)) throw dd_e;");
        for h in handlers {
            let conds: Vec<String> = h
                .errors
                .iter()
                .map(|e| match e {
                    HErr::Failure => "true".to_string(),
                    HErr::Timeout => "dd_e.kind === \"timeout\"".to_string(),
                    HErr::Declared(n) => format!("dd_e.kind === {}", q(n)),
                })
                .collect();
            self.line(d + 2, &format!("if ({}) {{", conds.join(" || ")));
            self.block(&h.body, d + 3);
            let ends = h.body.last().map(|x| matches!(x.kind, TK::Succeed { .. } | TK::Fail { .. } | TK::Break)).unwrap_or(false);
            if !ends {
                self.line(d + 3, &format!("break call_{site};"));
            }
            self.line(d + 2, "}");
        }
        self.line(d + 2, "throw dd_e;");
        self.line(d + 1, "}");
        let var = match target {
            Some(Target::Let(v)) => Some(self.var(v)),
            Some(Target::Case(c)) => Some(self.var(&m.cases[*c].name)),
            None => None,
        };
        // the answer: its declared type, and for a case the states it may carry
        if let (Some(v), Some(ty)) = (var, result_ty) {
            let check = format!("({})", ts_check(m, "dd_r", &ty, m.answer_range(callee), "T.", 0));
            self.line(d + 1, &format!("if (!{check}) throw dd.fail(\"Dandori.BadResponse\", {});", q(&format!("line {}: the answer from {} does not have the declared shape", s.line, cname))));
            if let (Some(Target::Case(c)), Some((_, allowed))) = (target, m.monitors.get(&site)) {
                let field = &m.cases[*c].state_field;
                self.line(
                    d + 1,
                    &format!(
                        "if (![{}].includes(dd_r[{}])) throw dd.fail(\"Dandori.UnexpectedState\", {});",
                        allowed.iter().map(|v| q(v)).collect::<Vec<_>>().join(", "),
                        q(field),
                        q(&format!("line {}: {} answered with a state the machine does not lead to here (expected one of {})", s.line, cname, allowed.join(", ")))
                    ),
                );
            }
            self.line(d + 1, &format!("{v} = dd_r;"));
            if let (Some(Target::Case(_)), Flavor::Temporal) = (target, self.flavor) {
                self.line(d + 1, "dd.shown();");
            }
        }
        self.line(d, "}");
    }
}
