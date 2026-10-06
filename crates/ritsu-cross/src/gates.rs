//! What sekisho's gates allow, held to what the project's contexts open and what its workflows call
//! (sekisho's DESIGN 4.6): X15 and X16. The gates are read through the port `Gates` (sekisho answers
//! for a `.gate` and for a pair of Cedar written by hand), the map through `Maps` (sakai), and what
//! a workflow calls through `Flows::operation_calls` (dandori). An operation is the same operation
//! wherever it is named by the same reference (ritsu's DESIGN 6.2): a gate's `guards`, a schema's
//! `@guards`, a context's `open host service` and a task's `http` or `connect` all come to it.
//!
//! **X15**: every operation a context of a map opens to the others (`open host service`: each method
//! of a service of a `.proto`, each operation of an OpenAPI document) is guarded by an action of a
//! gate, or its document says anyone may call it (`security: []`). A context some of whose
//! operations an action guards, and others not, is E907 for each one not guarded; a context none
//! of whose operations is guarded is one W907 (it has written no authorization with sekisho yet).
//! Each operation is a border. A context that holds a gate or a pair of Cedar that does not answer
//! (its own check says why) is not looked at; a map that does not pass sakai's stages, neither; nor
//! a project that writes no authorization with sekisho at all (no `.gate`, and no Cedar its maps
//! or requirements name), which has nothing to cover its operations with.
//!
//! **X16**: each call a workflow a gate names (`workflow … from "…"`) makes of an operation an action
//! of that gate guards is a border: the gate allows the workflow the action in some combination
//! (held, when the task declares the error of a denial or the workflow is allowed in every one); in
//! none (E908: every run that comes to the call is denied); in some only, and the task declares no
//! error for the denial, the one that comes back with 403 or `permission_denied` (W909), or whether
//! it does cannot be decided (W909, with why). An action the workflow is allowed whose operations
//! the flow never calls is W908 (it is allowed more than it needs), which is no border. The
//! calls of operations no action of the gate guards are not the gate's to say.

use crate::Borders;
use ritsu_base::diag::Diag;
use ritsu_base::naming::{Name, Tool};
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_ports::{Allowance, Asker, Finding, Flows, Found, GateFacts, Gates, Maps, OperationCall, Ports, PublishedOperation, References};
use ritsu_project::{Joined, Project};
use std::collections::{BTreeMap, BTreeSet};

/// The gates of a project: each `.gate`, and each pair of Cedar a map or a requirement names, with
/// what sekisho answers for it (or what it says, when it cannot).
pub struct Read {
    /// The file (from the root), and its facts or what its check says.
    pub gates: Vec<(String, Result<GateFacts, Vec<ritsu_ports::Said>>)>,
}

impl Read {
    /// The operations the actions guard, each with the file and the action that guards it.
    pub fn guarded(&self) -> BTreeMap<Name, Vec<(String, String)>> {
        let mut out: BTreeMap<Name, Vec<(String, String)>> = BTreeMap::new();
        for (file, f) in &self.gates {
            let Ok(f) = f else { continue };
            for a in &f.actions {
                for (n, _) in &a.guards {
                    out.entry(n.clone()).or_default().push((file.clone(), a.name.clone()));
                }
            }
        }
        out
    }
}

/// The name the files of a pair of Cedar share: `policies/refunds` of `policies/refunds.cedarschema`.
fn stem(file: &str) -> Option<&str> {
    [".cedarschema.json", ".cedarschema", ".cedar"].iter().find_map(|x| file.strip_suffix(x))
}

/// The files of Cedar's the project's maps and requirements name (`cedar "…"`), from the root, one
/// of each pair (its schema, when it is named, else its policies).
fn cedar_files(project: &Project, index: &ritsu_ports::Index, requirements: &dyn References) -> Vec<String> {
    let mut named: Vec<String> = Vec::new();
    let mut add = |rs: Vec<ritsu_ports::Reference>| {
        for r in rs {
            if r.target.tool == Tool::Cedar && !named.contains(&r.target.path) {
                named.push(r.target.path);
            }
        }
    };
    for f in project.of(Tool::Sakai) {
        if let Some(Ok(rs)) = index.references(Tool::Sakai, &project.root, &f.rel) {
            add(rs);
        }
    }
    for f in project.of(Tool::Yuen) {
        if let Ok(rs) = requirements.references(&project.root, &f.rel) {
            add(rs);
        }
    }
    // one file of each pair, the schema first: it holds the actions and their `@guards`
    named.sort_by_key(|f| (stem(f).map(str::to_string), !f.contains(".cedarschema")));
    let mut seen = BTreeSet::new();
    named.retain(|f| stem(f).is_some_and(|s| seen.insert(s.to_string())));
    named
}

/// What sekisho answers for every gate of the project and every pair of Cedar it names.
pub fn read(project: &Project, gates: &dyn Gates, index: &ritsu_ports::Index, requirements: &dyn References) -> Read {
    let mut out = Read { gates: Vec::new() };
    for f in project.of(Tool::Sekisho) {
        out.gates.push((f.rel.clone(), gates.facts(&project.root, &f.rel)));
    }
    for f in cedar_files(project, index, requirements) {
        let facts = gates.facts(&project.root, &f);
        out.gates.push((f, facts));
    }
    out
}

/// What one operation a context opens comes to (X15).
#[derive(Clone, Debug, PartialEq)]
pub enum Opened {
    /// An action guards it: the files and the actions.
    Guarded(Vec<(String, String)>),
    /// Its document says anyone may call it.
    Open,
    /// No action guards it, and some of the context's operations are guarded (E907).
    Unguarded,
    /// No action guards any of the context's operations (W907).
    NotYet,
}

/// X15 for one map's operations: each operation with what it comes to, in the order given; the
/// operations of a context in `skipped` (a gate of it does not answer) are left out.
pub fn judge_opened(ops: &[PublishedOperation], guarded: &BTreeMap<Name, Vec<(String, String)>>, skipped: &BTreeSet<String>) -> Vec<(PublishedOperation, Opened)> {
    let mut out = Vec::new();
    let contexts: Vec<&String> = ops.iter().map(|o| &o.context).fold(Vec::new(), |mut v, c| {
        if !v.contains(&c) {
            v.push(c);
        }
        v
    });
    for c in contexts {
        if skipped.contains(c) {
            continue;
        }
        let mine: Vec<&PublishedOperation> = ops.iter().filter(|o| o.context == *c).collect();
        let any = mine.iter().any(|o| guarded.contains_key(&o.operation));
        for o in mine {
            let came = match guarded.get(&o.operation) {
                Some(by) => Opened::Guarded(by.clone()),
                None if o.open_to_anyone => Opened::Open,
                None if any => Opened::Unguarded,
                None => Opened::NotYet,
            };
            out.push((o.clone(), came));
        }
    }
    out
}

/// What one call of a workflow comes to (X16).
#[derive(Clone, Debug, PartialEq)]
pub enum Called {
    /// Allowed in some combination, and every way the call ends is the flow's to handle.
    Held,
    /// Allowed in no combination (E908).
    Never,
    /// Denied in some combinations, and the task declares no error for it (W909): one allowed
    /// combination and one denied.
    Unhandled { allowed: Text, denied: Text },
    /// Whether the workflow is allowed cannot be decided, and the task declares no error for a
    /// denial (W909): why.
    Undecided(Text),
}

/// X16 for one call: what the gate's answer and the task's errors come to.
pub fn judge_call(call: &OperationCall, allowance: &Found<Allowance>) -> Called {
    match (allowance, &call.denied) {
        (Found::Value(Allowance::Never), _) => Called::Never,
        (Found::Value(Allowance::Always), _) | (_, Some(_)) => Called::Held,
        (Found::Value(Allowance::Sometimes { allowed, denied }), None) => Called::Unhandled { allowed: allowed.clone(), denied: denied.clone() },
        (Found::Undecided(why), None) => Called::Undecided(why.clone()),
    }
}

/// X15 and X16 over the project, through the ports `joined` holds.
pub(crate) fn check(project: &Project, joined: &Joined, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    check_with(project, &*joined.gates(), &*joined.dandori, &*joined.maps(), &joined.ports(), &joined.index, &*joined.requirements(), lang, borders)
}

/// [`check`], with each port given: what sekisho says of the gates, what dandori says the flows
/// call, what sakai says of the maps, the index of what the files name, and what the requirements
/// name (the Cedar they point at).
#[allow(clippy::too_many_arguments)]
pub fn check_with(project: &Project, gates: &dyn Gates, flows: &dyn Flows, maps: &dyn Maps, ports: &Ports, index: &ritsu_ports::Index, requirements: &dyn References, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    let read = read(project, gates, index, requirements);
    let mut out = opened(project, &read, maps, lang, borders);
    out.extend(called(project, &read, gates, flows, ports, lang, borders));
    out
}

/// The text of a file of the project, from the root.
fn text_of(project: &Project, rel: &str) -> String {
    ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(&project.root, rel)).unwrap_or_default()
}

/// A few things, in both languages: the first `n`, and how many more.
fn some(xs: &[String], n: usize) -> Text {
    let shown: Vec<Text> = xs.iter().take(n).map(|x| Text::same(x.clone())).collect();
    let l = Text::list(&shown);
    if xs.len() > n {
        let more = xs.len() - n;
        tr!("{}、ほか {more} 個", "{} and {more} more", l.ja; l.en)
    } else {
        l
    }
}

/// X15: E907 and W907.
fn opened(project: &Project, read: &Read, maps: &dyn Maps, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    // a project that writes no authorization with sekisho (no `.gate`, no Cedar a map or a
    // requirement names) is not asked to cover its operations with it
    if read.gates.is_empty() {
        return Vec::new();
    }
    let guarded = read.guarded();
    let mut out = Vec::new();
    let mut seen: BTreeSet<(String, Name)> = BTreeSet::new();
    for m in project.of(Tool::Sakai) {
        // a map, passing the stages that decide who owns what; a context file is none
        let Ok(Some(_)) = maps.map(&project.root, &m.rel) else { continue };
        let Ok(ops) = maps.published_operations(&project.root, &m.rel) else { continue };
        let ops: Vec<PublishedOperation> = ops.into_iter().filter(|o| seen.insert((o.file.clone(), o.operation.clone()))).collect();
        // the contexts a gate or a pair of Cedar belongs to that cannot answer
        let skipped: BTreeSet<String> = read.gates.iter().filter(|(_, f)| f.is_err()).filter_map(|(file, _)| maps.context_of(&project.root, &m.rel, file).ok().flatten()).collect();
        let judged = judge_opened(&ops, &guarded, &skipped);
        // what a context guards, for the note of E907
        let mut not_yet: BTreeMap<String, Vec<&PublishedOperation>> = BTreeMap::new();
        for (o, came) in &judged {
            match came {
                Opened::Guarded(_) | Opened::Open => borders.held += 1,
                Opened::Unguarded => {
                    borders.failed += 1;
                    let others: Vec<String> = judged.iter().filter(|(x, c)| x.context == o.context && matches!(c, Opened::Guarded(_))).map(|(x, _)| x.operation.text()).collect();
                    out.push(Finding::of(&unguarded(project, o, &others, &m.rel), Some(o.file.clone()), lang));
                }
                Opened::NotYet => {
                    borders.undecided += 1;
                    not_yet.entry(o.context.clone()).or_default().push(o);
                }
            }
        }
        // one W907 a context, at its first operation, in the order of the map
        let mut said: Vec<&str> = Vec::new();
        for (o, came) in &judged {
            if *came == Opened::NotYet && !said.contains(&o.context.as_str()) {
                said.push(&o.context);
                out.push(Finding::of(&not_yet_written(project, &not_yet[&o.context], &m.rel), Some(o.file.clone()), lang));
            }
        }
    }
    out
}

fn shown_of(project: &Project, rel: &str) -> String {
    project.shown.path(rel)
}

/// E907: an operation a context opens that no action guards.
fn unguarded(project: &Project, o: &PublishedOperation, others: &[String], map: &str) -> Diag {
    let (ctx, op) = (&o.context, o.operation.text());
    let fix = if o.operation.tool == Tool::Openapi {
        tr!(
            "その操作を守る action を、「{ctx}」の .gate に書いてください（`guards <API> <操作>`）。だれでも呼べるようにわざとしている操作なら、文書でその操作に `security: []` と書いてください。",
            "Write an action that guards it in a .gate of {ctx} (`guards <api> <operation>`). If anyone may call it on purpose, write `security: []` on the operation in its document."
        )
    } else {
        tr!(
            "その操作を守る action を、「{ctx}」の .gate に書いてください（`guards <API> \"<サービス>/<メソッド>\"`）。",
            "Write an action that guards it in a .gate of {ctx} (`guards <api> \"<service>/<method>\"`)."
        )
    };
    let o_text = some(others, 3);
    Diag::at("E907", &shown_of(project, &o.file), o.line, 0, tr!("「{ctx}」がほかのコンテキストに公開する {op} を、どの action も守っていません", "{ctx} opens {op} to the other contexts, and no action guards it"))
        .source(&text_of(project, &o.file))
        .rel(&o.file)
        .note(tr!(
            "「{ctx}」のほかの操作は、ゲートの action が守っています（{}）。この操作だけは、だれが呼べるかを書いていません（地図 {map}）。",
            "{ctx} guards its other operations with the actions of its gates ({}); this one says nothing of who may call it (the map {map}).",
            o_text.ja; o_text.en
        ))
        .note(fix)
}

/// W907: a context opens operations, and no action guards any of them.
fn not_yet_written(project: &Project, ops: &[&PublishedOperation], map: &str) -> Diag {
    let o = ops[0];
    let (ctx, n) = (&o.context, ops.len());
    let names: Vec<String> = ops.iter().map(|x| x.operation.text()).collect();
    let l = some(&names, 5);
    let message = if n == 1 {
        tr!("「{ctx}」はほかのコンテキストに操作を一つ公開していますが、それを守る action がありません", "{ctx} opens an operation to the other contexts, and no action guards it")
    } else {
        tr!("「{ctx}」はほかのコンテキストに操作を {n} 個公開していますが、どれも守る action がありません", "{ctx} opens {n} operations to the other contexts, and no action guards any of them")
    };
    Diag::at("W907", &shown_of(project, &o.file), o.line, 0, message)
        .source(&text_of(project, &o.file))
        .rel(&o.file)
        .note(tr!("公開している操作: {}（地図 {map}）。", "The operations it opens: {} (the map {map}).", l.ja; l.en))
        .note(tr!(
            "「{ctx}」は、まだ sekisho で認可を書いていません。認証した相手に何をさせてよいかが、ritsu の確かめられる形で書かれていません。「{ctx}」の .gate を書き、公開する操作をそれぞれ action で守ってください。",
            "{ctx} has written no authorization with sekisho yet: what someone who has signed in may do is written nowhere ritsu can check. Write a .gate of {ctx}, and guard each operation it opens with an action."
        ))
}

/// X16: E908, W909 and W908.
fn called(project: &Project, read: &Read, gates: &dyn Gates, flows: &dyn Flows, ports: &Ports, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    let mut out = Vec::new();
    for (file, facts) in &read.gates {
        let Ok(f) = facts else { continue };
        for w in &f.workflows {
            let Ok((_, calls)) = flows.operation_calls(&project.root, &w.flow.path, ports) else { continue };
            let asker = Asker::Workflow(w.alias.clone());
            // the action of this gate that guards each operation
            let guarding = |n: &Name| f.actions.iter().find(|a| a.guards.iter().any(|(g, _)| g == n));
            let flow_src = text_of(project, &w.flow.path);
            for c in &calls {
                let Some(a) = guarding(&c.operation) else { continue };
                let Ok(allowance) = gates.allowed(&project.root, file, &a.alias, &asker) else { continue };
                let came = judge_call(c, &allowance);
                match &came {
                    Called::Held => borders.held += 1,
                    Called::Never => borders.failed += 1,
                    Called::Unhandled { .. } | Called::Undecided(_) => borders.undecided += 1,
                }
                if let Some(d) = call_said(project, &flow_src, file, &w.name, &w.flow, c, &a.name, &came) {
                    out.push(Finding::of(&d, Some(w.flow.path.clone()), lang));
                }
            }
            // an action the workflow is allowed, none of whose operations the flow calls
            for a in f.actions.iter().filter(|a| !a.guards.is_empty() && a.principals.iter().any(|p| p == "Workflow")) {
                if calls.iter().any(|c| a.guards.iter().any(|(g, _)| *g == c.operation)) {
                    continue;
                }
                let Ok(Found::Value(Allowance::Always | Allowance::Sometimes { .. })) = gates.allowed(&project.root, file, &a.alias, &asker) else { continue };
                let ops: Vec<String> = a.guards.iter().map(|(g, _)| g.text()).collect();
                let l = some(&ops, 3);
                let (wn, an, flow) = (&w.name, &a.name, w.flow.text());
                let d: Diag = Diag::at("W908", &shown_of(project, file), w.line, 0, tr!(
                    "ワークフロー `{wn}` は action `{an}` を許されていますが、{flow} はその操作（{}）をどこでも呼びません",
                    "The workflow `{wn}` is allowed the action `{an}`, and {flow} calls none of its operations ({})",
                    l.ja; l.en
                ))
                .source(&text_of(project, file))
                .rel(file)
                .note(tr!(
                    "ワークフローは要るより多く許されています。ワークフローを許す permit から、この action を外してください。",
                    "The workflow is allowed more than it needs: take the action out of the permits that allow it."
                ));
                out.push(Finding::of(&d, Some(file.clone()), lang));
            }
        }
    }
    out
}

/// A piece of Japanese text a word follows: a space after it when it ends in a word of ASCII or in
/// code, as the suite's Japanese writes it (`status：paid のとき`, `（status：paid）のとき`).
fn spaced(piece: &str) -> String {
    match piece.chars().last() {
        Some(c) if c.is_ascii_alphanumeric() || c == '`' || c == '"' || c == '_' => format!("{piece} "),
        _ => piece.to_string(),
    }
}

/// E908 and W909, at the call in the flow.
#[allow(clippy::too_many_arguments)]
fn call_said(project: &Project, flow_src: &str, gate: &str, workflow: &str, flow: &Name, c: &OperationCall, action: &str, came: &Called) -> Option<Diag> {
    let (task, op) = (&c.task, c.operation.text());
    let at = |code: &'static str, message: Text| -> Diag { Diag::at(code, &shown_of(project, &flow.path), c.line, 0, message).source(flow_src).rel(&flow.path) };
    let declare = if c.operation.tool == Tool::Proto {
        tr!(
            "タスクに `errors denied = permission_denied` と書き、拒まれたときにどうするかを呼び出しの下に書いてください（`on denied => …`）。",
            "Declare the error on the task, `errors denied = permission_denied`, and say under the call what happens when it is denied (`on denied => …`)."
        )
    } else {
        tr!(
            "タスクに `errors denied = 403` と書き、拒まれたときにどうするかを呼び出しの下に書いてください（`on denied => …`）。",
            "Declare the error on the task, `errors denied = 403`, and say under the call what happens when it is denied (`on denied => …`)."
        )
    };
    match came {
        Called::Held => None,
        Called::Never => Some(
            at("E908", tr!(
                "タスク `{task}` は {op} を呼びますが、それを守る {gate} の action `{action}` は、ワークフロー `{workflow}` をどの組み合わせでも許しません",
                "The task `{task}` calls {op}, and the action `{action}` of {gate} that guards it allows the workflow `{workflow}` in no combination"
            ))
            .note(tr!("この呼び出しまで進んだ実行は、いつもそこで拒まれます。", "Every run that comes to the call is denied there."))
            .note(tr!(
                "{gate} に、ワークフローを許す permit を書くか（`principal is workflow {workflow}`）、呼び出しを消してください。",
                "Write a permit in {gate} that allows the workflow (`principal is workflow {workflow}`), or take the call out."
            )),
        ),
        Called::Unhandled { allowed, denied } => Some(
            at("W909", tr!(
                "タスク `{task}` の呼び出しは、{gate} の action `{action}` に拒まれることがありますが、タスクは拒まれたときのエラーを宣言していません",
                "The call of the task `{task}` can be denied by the action `{action}` of {gate}, and the task declares no error for it"
            ))
            .note(tr!("たとえば、{}のときは許され、{}のときは拒まれます。", "Allowed, for example: {}. Denied, for example: {}.", spaced(&allowed.ja), spaced(&denied.ja); allowed.en, denied.en))
            .note(tr!("拒まれると、ワークフローは宣言していない失敗で止まります。", "A run that is denied stops with a failure the workflow does not declare."))
            .note(declare),
        ),
        Called::Undecided(why) => Some(
            at("W909", tr!(
                "タスク `{task}` の呼び出しを、{gate} の action `{action}` が許すかを決められず、タスクは拒まれたときのエラーを宣言していません",
                "Whether the action `{action}` of {gate} allows the call of the task `{task}` cannot be decided, and the task declares no error for a denial"
            ))
            .note(tr!("決められない理由: {}。", "Why it cannot be decided: {}.", why.ja; why.en))
            .note(declare),
        ),
    }
}
