//! `check`: parse, lower, walk the runs, and the checks that look at the whole program —
//! retries that may repeat a change, and what an Express workflow cannot do. How long the
//! execution history can grow on each platform is here too, for each build to check (E040).

use crate::diag::Diag;
use crate::model::*;
use crate::syntax::{self, Kind};
use std::path::Path;

pub struct Checked {
    pub model: Option<Model>,
    pub diags: Vec<Diag>,
}

pub fn check_file(path: &Path) -> Result<(String, Checked), String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let checked = check_source(&src, path);
    Ok((src, checked))
}

pub fn check_source(src: &str, path: &Path) -> Checked {
    let prog = match syntax::parse(src) {
        Ok(p) => p,
        Err(d) => return Checked { model: None, diags: vec![d] },
    };
    let (model, mut diags) = crate::lower::lower(&prog, path);
    let mut model = match model {
        Some(m) => m,
        None => {
            diags.sort_by(|a, b| (a.line, a.col, a.code).cmp(&(b.line, b.col, b.code)));
            return Checked { model: None, diags };
        }
    };
    let fr = crate::flow::analyze(&model);
    diags.extend(fr.diags);
    model.monitors = fr.monitors;
    diags.extend(whole(&model));
    diags.sort_by(|a, b| (a.line, a.col, a.code).cmp(&(b.line, b.col, b.code)));
    let ok = !crate::diag::has_errors(&diags);
    Checked { model: if ok { Some(model) } else { None }, diags }
}

fn whole(m: &Model) -> Vec<Diag> {
    let mut out = Vec::new();
    for t in &m.tasks {
        if let Some(r) = &t.retry {
            let repeats_change = t.changes_things() && !t.key;
            let retries_failures = r.on.is_empty() || r.on.iter().any(|x| x == "failure" || x == "timeout");
            if repeats_change && retries_failures {
                let starts_or_sends = matches!(t.machine, Some(TaskMachine::Starts { .. }) | Some(TaskMachine::Sends { .. }));
                let (en, ja) = (
                    format!("`{}` is retried after a failure or a timeout, when the first try may have gone through on the other side; give it `key` so the other side can tell a retry from a new request, or mark it `idempotent` if doing it twice is the same as once", t.name),
                    format!("`{}` は失敗やタイムアウトのあとにやり直されますが、最初の一回が相手の側で通っていることがあります。相手がやり直しを見分けられるよう `key` を付けるか、二度しても一度と同じなら `idempotent` と書いてください", t.name),
                );
                if starts_or_sends {
                    out.push(Diag::error("E030", t.line, 1, en, ja));
                } else {
                    out.push(Diag::warning("W030", t.line, 1, en, ja));
                }
            }
        }
    }
    if m.kind == Kind::Express {
        for t in &m.tasks {
            if t.callback {
                out.push(Diag::error("E031", t.line, 1, format!("an Express workflow cannot wait for a callback (`{}`)", t.name), format!("Express のワークフローはコールバックを待てません（`{}`）", t.name)));
            }
            if t.state_machine.is_some() {
                out.push(Diag::error(
                    "E031",
                    t.line,
                    1,
                    format!("an Express workflow cannot wait for a nested execution to end (`{}`)", t.name),
                    format!("Express のワークフローは、入れ子の実行が終わるのを待てません（`{}`）", t.name),
                ));
            }
            if t.changes_things() && !t.key {
                out.push(Diag::error(
                    "E031",
                    t.line,
                    1,
                    format!("an asynchronous Express workflow may run twice, and `{}` changes things without `key`", t.name),
                    format!("非同期の Express のワークフローは二度走ることがあり、`{}` は `key` を持たずに相手の側を変えます", t.name),
                ));
            }
        }
        let longest = max_wait(&m.flow).saturating_add(m.on_failure.as_ref().map(|f| max_wait(f)).unwrap_or(0)).saturating_add(m.on_cancel.as_ref().map(|f| max_wait(f)).unwrap_or(0));
        if longest > 300 {
            out.push(Diag::error(
                "E031",
                1,
                1,
                "an Express workflow runs for at most five minutes, and this one can wait longer",
                "Express のワークフローは五分までしか動けませんが、これはそれより長く待つことがあります",
            ));
        }
    }
    out
}

/// A platform's refusal of a workflow whose execution can outgrow the platform (E040): the
/// history of Step Functions and Temporal, the nodes of an Argo Workflow, the operations of a
/// durable execution. On Temporal, a loop at the top of the flow continues as new once the
/// history is long, so a run's history is bounded by one round of it past `CONTINUE_AT`.
pub fn history_limit(m: &Model, p: Platform) -> Option<Diag> {
    let (en, ja) = match p {
        Platform::StepFunctions if m.kind == Kind::Express => return None,
        Platform::StepFunctions | Platform::Temporal => {
            let (name, limit, n) = if p == Platform::StepFunctions { ("Step Functions", 25_000, bound(m, &ASL_COST)) } else { ("Temporal", TEMPORAL_LIMIT, temporal_bound(m)) };
            if n <= limit {
                return None;
            }
            let (more_en, more_ja) = match p {
                Platform::Temporal if m.flow.iter().any(continues_as_new) => (
                    "; the loops at the top of the flow go on in a new run (Continue-As-New) as the history grows, so what is too long is the rest of the flow, or one round of such a loop",
                    "。フローの一番外のループは履歴が長くなると新しい実行で続ける（Continue-As-New）ので、長すぎるのはその外の部分か、ループの一回です",
                ),
                Platform::Temporal => (
                    "; a `repeat`, or a `for` that is not parallel, at the top of the flow would go on in a new run (Continue-As-New) as the history grows",
                    "。フローの一番外に置いた `repeat` や並列でない `for` なら、履歴が長くなると新しい実行で続けます（Continue-As-New）",
                ),
                _ => ("", ""),
            };
            (
                format!("on {name}, an execution of this workflow can write up to about {n} history events, past the limit of {limit}{more_en}; lower the loop counts, or start a new execution to continue"),
                format!("{name} では、このワークフローの一回の実行が実行履歴を最大でおよそ {n} 件書きます。上限の {limit} 件を超えます{more_ja}。ループの回数を減らすか、続きを新しい実行で始めてください"),
            )
        }
        Platform::Argo => {
            let n = bound(m, &ARGO_COST);
            if n <= ARGO_LIMIT {
                return None;
            }
            (
                format!("on Argo Workflows, an execution of this workflow can make up to about {n} nodes, past {ARGO_LIMIT}; the Workflow keeps every node, and past 1 MiB even compressed it cannot be stored without a database for the node status; lower the loop counts, or start a new execution to continue"),
                format!("Argo Workflows では、このワークフローの一回の実行がノードを最大でおよそ {n} 個作ります。目安の {ARGO_LIMIT} 個を超えます。Workflow はノードをすべて持ち、圧縮しても 1 MiB を超えると、ノードの状態を置くデータベースなしには保存できません。ループの回数を減らすか、続きを新しい実行で始めてください"),
            )
        }
        Platform::Durable => {
            let n = bound(m, &DURABLE_COST);
            if n <= DURABLE_LIMIT {
                return None;
            }
            (
                format!("on Lambda durable functions, an execution of this workflow can take up to about {n} durable operations, past the limit of {DURABLE_LIMIT}; lower the loop counts, or start a new execution to continue"),
                format!("Lambda durable functions では、このワークフローの一回の実行が durable の操作を最大でおよそ {n} 回使います。上限の {DURABLE_LIMIT} 回を超えます。ループの回数を減らすか、続きを新しい実行で始めてください"),
            )
        }
        Platform::Graph => return None,
    };
    Some(Diag::error("E040", 1, 1, en, ja))
}

/// Temporal's limit on a run's history.
pub const TEMPORAL_LIMIT: u64 = 51_200;

/// How long a run's history grows on Temporal before a loop at the top of the flow continues as
/// new, at the start of a round (or sooner, when the server suggests it).
pub const CONTINUE_AT: u64 = 10_000;

/// Whether a statement at the top of the flow is a loop that goes on in a new run on Temporal
/// (Continue-As-New) once the history is long: a `repeat`, or a `for` that is not parallel.
pub fn continues_as_new(s: &TStmt) -> bool {
    matches!(s.kind, TK::Repeat { .. } | TK::For { parallel: None, .. })
}

/// The history of one run on Temporal: as `bound`, but a loop at the top of the flow that goes on
/// in a new run once the history reaches `CONTINUE_AT` costs at most that and one round more.
pub fn temporal_bound(m: &Model) -> u64 {
    let c = &TEMPORAL_COST;
    let mut body: u64 = 0;
    for s in &m.flow {
        let full = cost(m, std::slice::from_ref(s), c);
        let here = match &s.kind {
            TK::Repeat { body: b, .. } | TK::For { body: b, .. } if continues_as_new(s) => full.min(CONTINUE_AT + cost(m, b, c) + c.loop_iter + 2 * c.choice),
            _ => full,
        };
        body = body.saturating_add(here);
    }
    let cleanup = m.on_failure.as_ref().map(|f| cost(m, f, c)).unwrap_or(0) + m.on_cancel.as_ref().map(|f| cost(m, f, c)).unwrap_or(0);
    c.start + body + cleanup + c.end
}

fn max_wait(ss: &[TStmt]) -> u64 {
    let mut total: u64 = 0;
    for s in ss {
        total = total.saturating_add(match &s.kind {
            TK::Wait { seconds } => *seconds,
            TK::WaitUntil { .. } => u64::MAX / 4,
            TK::Match { arms, .. } => arms.iter().map(|a| max_wait(&a.body)).max().unwrap_or(0),
            TK::Repeat { times, body } => (*times as u64).saturating_mul(max_wait(body)),
            TK::For { max, parallel: None, body, .. } => (*max as u64).saturating_mul(max_wait(body)),
            TK::For { body, .. } => max_wait(body),
            TK::Call { handlers, .. } => handlers.iter().map(|h| max_wait(&h.body)).max().unwrap_or(0),
            _ => 0,
        });
    }
    total
}

/// What each thing costs in history events on a platform. These are estimates on the
/// high side, so the check errs toward saying too much rather than too little.
pub struct Cost {
    pub start: u64,
    pub call: u64,
    pub retry: u64,
    pub check: u64,
    pub choice: u64,
    pub wait: u64,
    pub loop_iter: u64,
    pub end: u64,
    /// `let x = <value>`
    pub assign: u64,
    /// a `for … in parallel`, and each of its rounds
    pub map_start: u64,
    pub map_iter: u64,
    /// what waiting for a callback adds to a call
    pub callback: u64,
    /// what each arm of a match and each handler of a call adds, whether it runs or not
    pub arm: u64,
    /// `break`
    pub brk: u64,
    /// what a call on a case adds: Temporal writes the cases' states to a search attribute
    pub case_call: u64,
    /// a call of a rule that says `local`, and a retry of it: on Temporal, a local activity
    pub local_call: u64,
    pub local_retry: u64,
}

/// Step Functions: a task is entered, scheduled, started, succeeds and is exited (5); a
/// failed try adds scheduled, started, failed (3); a Choice, Pass or Wait is entered and
/// exited (2); a Map is entered, started, succeeds and is exited (4), and each round
/// starts and succeeds (2); the execution starts and ends.
pub const ASL_COST: Cost = Cost { start: 5, call: 6, retry: 3, check: 2, choice: 2, wait: 2, loop_iter: 4, end: 3, assign: 2, map_start: 6, map_iter: 2, callback: 0, arm: 0, brk: 0, case_call: 0, local_call: 6, local_retry: 3 };

/// Temporal: an activity is scheduled, started, completed, and a workflow task follows
/// (6); a retry, done in the workflow's code so that it matches Step Functions, adds a
/// timer and another activity (11); a timer is started and fired, and a workflow task
/// follows (5); a match costs nothing; a callback adds the update that brings the answer
/// (accepted, completed), its workflow task, and the timer of its timeout (7); a call on a
/// case, the search attribute's upsert (1). A local activity leaves a marker, and a workflow
/// task may follow (4); its retry, a timer and another (9).
pub const TEMPORAL_COST: Cost = Cost { start: 4, call: 6, retry: 11, check: 0, choice: 0, wait: 5, loop_iter: 0, end: 4, assign: 0, map_start: 0, map_iter: 0, callback: 7, arm: 0, brk: 0, case_call: 1, local_call: 4, local_retry: 9 };

/// Lambda durable functions: a task is one step, or a callback and its submit step (2); a
/// rule is one invoke; a retry adds a wait and another call (3); a wait is one operation, a
/// wait until reads the clock in a step first (2); a map is one operation, and each round
/// runs in a child context of its own (1). The limit is 3,000 operations an execution and
/// cannot be raised.
pub const DURABLE_COST: Cost = Cost { start: 0, call: 2, retry: 3, check: 0, choice: 0, wait: 2, loop_iter: 0, end: 0, assign: 0, map_start: 1, map_iter: 1, callback: 0, arm: 0, brk: 0, case_call: 0, local_call: 2, local_retry: 3 };
pub const DURABLE_LIMIT: u64 = 3_000;

/// Argo Workflows: the nodes in the Workflow's status. A statement is a step group of its
/// block and a step; a template that computes is a template, a step group and a step that
/// does not run (3), so a `let`, a `fail`, a `succeed` or a `break` is 4. A call is 13: its
/// template, its step groups for the arguments, the try, the after and the handlers, the two
/// templates that compute, and the pod; a retry adds the retry's node and a pod; a callback,
/// the wait and its step group. A match is 8, one node an arm (the arm's block, or a step that
/// does not run), and one for the arm of a value no arm names. A wait until computes its
/// seconds first (8). A round of a loop is 12 (its template, step groups, start, the body's
/// template, and what it yields), of a parallel one 11; around the rounds, a loop takes 11
/// nodes and a parallel one 13. The run begins with 7 and ends with 22, with `on failure`'s
/// check and frame. In the runs of tests/flows/edges.flow, a node took 510 to 612 bytes,
/// about 60 compressed; the template, which Argo keeps twice in the Workflow, took 240 KB
/// of it in the runs of examples/hotel.
pub const ARGO_COST: Cost = Cost { start: 7, call: 13, retry: 2, check: 0, choice: 9, wait: 8, loop_iter: 12, end: 22, assign: 4, map_start: 13, map_iter: 11, callback: 2, arm: 1, brk: 4, case_call: 0, local_call: 13, local_retry: 2 };
/// Argo stores a Workflow of up to 1 MiB, compressing the node status when it is larger
/// (MAX_WORKFLOW_SIZE), unless the status goes to a database. At about 60 bytes a node, that
/// is some 15,000 nodes; values larger than the tests' leave fewer, so the check stops at 10,000.
pub const ARGO_LIMIT: u64 = 10_000;

pub fn bound(m: &Model, c: &Cost) -> u64 {
    let body = cost(m, &m.flow, c);
    // a run can fail, run `on failure`, be cancelled in it, and run `on cancel`
    let cleanup = m.on_failure.as_ref().map(|f| cost(m, f, c)).unwrap_or(0) + m.on_cancel.as_ref().map(|f| cost(m, f, c)).unwrap_or(0);
    c.start + body + cleanup + c.end
}

fn cost(m: &Model, ss: &[TStmt], c: &Cost) -> u64 {
    let mut total: u64 = 0;
    for s in ss {
        let here = match &s.kind {
            TK::Call { callee, handlers, target, .. } => {
                let retries = match callee {
                    Callee::Task(t) => m.tasks[*t].retry.as_ref().map(|r| r.times as u64).unwrap_or(0),
                    Callee::Rule(_) => RULE_RETRIES as u64,
                };
                // an event is a wait like a callback's, with no call before it
                let waits = match callee {
                    Callee::Task(t) if m.tasks[*t].callback || m.tasks[*t].event => c.callback,
                    _ => 0,
                };
                let after = (handlers.len() as u64 * c.arm + handlers.iter().map(|h| cost(m, &h.body, c)).max().unwrap_or(0)).max(c.check);
                let shown = if matches!(target, Some(Target::Case(_))) { c.case_call } else { 0 };
                let (call, retry) = match callee {
                    Callee::Rule(r) if m.rules[*r].local => (c.local_call, c.local_retry),
                    Callee::Task(t) if m.tasks[*t].event => (0, 0),
                    _ => (c.call, c.retry),
                };
                call + waits + retries * (retry + waits) + after + shown
            }
            TK::Assign { .. } => c.assign,
            TK::For { max, parallel: None, body, .. } => 2 * c.choice + *max as u64 * (cost(m, body, c) + c.loop_iter),
            TK::For { max, body, .. } => c.map_start + c.choice + *max as u64 * (cost(m, body, c) + c.map_iter),
            TK::Match { arms, .. } => c.choice + arms.len() as u64 * c.arm + arms.iter().map(|a| cost(m, &a.body, c)).max().unwrap_or(0),
            TK::Wait { .. } | TK::WaitUntil { .. } => c.wait,
            TK::Repeat { times, body } => c.choice + *times as u64 * (cost(m, body, c) + c.loop_iter),
            TK::Break => c.brk,
            TK::Pass => 0,
            TK::Succeed { .. } | TK::Fail { .. } => c.choice,
        };
        total = total.saturating_add(here);
    }
    total
}

/// A rule is a pure function, so repeating a call of it is harmless: every target retries
/// a failed call of a rule twice, one second apart and then two.
pub const RULE_RETRIES: u32 = 2;
