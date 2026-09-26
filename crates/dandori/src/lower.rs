//! The first pass: read the rules through rulec, resolve every name, type every value,
//! and turn the statements into the checked tree of `model`. The run-dependent checks
//! (states of cases, assignment, exhaustiveness, exits) are the second pass, in `flow`.

use crate::diag::Diag;
use crate::model::*;
use crate::rulec::{self, RType};
use crate::syntax::{self, Block, Call, Expr, MachineUse, Part, Program, Span, StmtKind, TypeExpr};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const TYPE_HINT_EN: &str = "a type is int, string, bool, timestamp, json, a unit such as money[円, incl_tax], an enum, a record, list[T] or T?";
const TYPE_HINT_JA: &str = "型は int・string・bool・timestamp・json・money[円, incl_tax] のような単位・列挙・レコード・list[T]・T? のどれかです";

pub struct Lowerer<'a> {
    prog: &'a Program,
    pub diags: Vec<Diag>,
    m: Model,
    enum_ix: BTreeMap<String, EnumId>,
    record_ix: BTreeMap<String, RecordId>,
    rule_ix: BTreeMap<String, usize>,
    task_ix: BTreeMap<String, usize>,
    var_ty: BTreeMap<String, Ty>,
    site: usize,
    /// the loops around the statement being lowered: true for `for … in parallel`
    loops: Vec<bool>,
    /// how many `for … in parallel` are around it
    par_depth: usize,
    in_on_failure: bool,
    in_on_cancel: bool,
    /// a variable's type grew to take `none` in this pass over the names
    widened: bool,
}

fn e(code: &'static str, sp: Span, en: impl Into<String>, ja: impl Into<String>) -> Diag {
    Diag::error(code, sp.line, sp.col, en, ja)
}

pub fn lower(prog: &Program, file: &Path) -> (Option<Model>, Vec<Diag>) {
    let name = match &prog.name {
        Some((n, _)) => n.clone(),
        None => {
            return (
                None,
                vec![Diag::error("E001", 1, 1, "a .flow starts with `workflow <name> v<n>`", ".flow は `workflow <名前> v<版>` で始めます")],
            )
        }
    };
    let mut lw = Lowerer {
        prog,
        diags: vec![],
        m: Model {
            name,
            version: prog.version,
            description: prog.description.clone().unwrap_or_default(),
            kind: prog.kind.clone(),
            source_file: file.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default(),
            rules: vec![],
            enums: vec![],
            records: vec![],
            inputs: vec![],
            outputs: vec![],
            tasks: vec![],
            cases: vec![],
            vars: vec![],
            flow: vec![],
            on_failure: None,
            on_cancel: None,
            on_cancel_line: 0,
            monitors: BTreeMap::new(),
        },
        enum_ix: BTreeMap::new(),
        record_ix: BTreeMap::new(),
        rule_ix: BTreeMap::new(),
        task_ix: BTreeMap::new(),
        var_ty: BTreeMap::new(),
        site: 0,
        loops: vec![],
        par_depth: 0,
        in_on_failure: false,
        in_on_cancel: false,
        widened: false,
    };
    let base = file.parent().unwrap_or(Path::new("."));
    lw.rules(base);
    lw.local_types();
    lw.io();
    lw.tasks();
    lw.cases();
    lw.variables();
    if let Some((b, _)) = &prog.flow {
        lw.m.flow = lw.block(b);
    } else {
        lw.diags.push(Diag::error("E009", 1, 1, "a workflow needs a `flow`", "ワークフローには `flow` が要ります"));
    }
    if let Some((b, _)) = &prog.on_failure {
        lw.in_on_failure = true;
        let t = lw.block(b);
        lw.m.on_failure = Some(t);
        lw.in_on_failure = false;
    }
    if let Some((b, sp)) = &prog.on_cancel {
        lw.m.on_cancel_line = sp.line;
        lw.in_on_cancel = true;
        let t = lw.block(b);
        lw.m.on_cancel = Some(t);
        lw.in_on_cancel = false;
    }
    lw.parallel_scopes();
    lw.m.vars = lw.var_ty.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let ok = !crate::diag::has_errors(&lw.diags);
    (if ok { Some(lw.m) } else { None }, lw.diags)
}

impl<'a> Lowerer<'a> {
    fn push(&mut self, d: Diag) {
        self.diags.push(d);
    }

    fn next_site(&mut self) -> usize {
        self.site += 1;
        self.site
    }

    // -----------------------------------------------------------------------
    // Declarations

    fn rules(&mut self, base: &Path) {
        for u in &self.prog.uses {
            let (name, sp) = &u.name;
            if self.rule_ix.contains_key(name) {
                self.push(e("E006", *sp, format!("the rule `{name}` is used twice"), format!("規則 `{name}` が二度読み込まれています")));
                continue;
            }
            let path = base.join(&u.path);
            let info = match rulec::load(&path) {
                Ok(i) => i,
                Err(msg) => {
                    self.push(
                        e("E005", *sp, format!("could not read the rule `{}`", u.path), format!("規則 `{}` を読めませんでした", u.path))
                            .note(msg.clone(), msg),
                    );
                    continue;
                }
            };
            let ix = self.m.rules.len();
            for (en, values) in &info.enums {
                let q = format!("{name}.{en}");
                self.enum_ix.insert(q.clone(), self.m.enums.len());
                self.m.enums.push(EnumDef { name: q, values: values.clone() });
            }
            let fields: Vec<(String, Ty)> = info.outputs.iter().map(|c| (c.name.clone(), self.rty(name, &c.ty))).collect();
            let rec = self.m.records.len();
            self.m.records.push(RecordDef { name: format!("{name}.outputs"), fields, origin: RecordOrigin::RuleOutputs(ix) });
            self.rule_ix.insert(name.clone(), ix);
            self.m.rules.push(RuleUse { name: name.clone(), info, lambda: u.lambda.clone(), local: u.local, outputs: rec, line: sp.line });
        }
    }

    fn rty(&self, rule: &str, t: &RType) -> Ty {
        match t {
            RType::Bool => Ty::Bool,
            RType::Str => Ty::Str,
            RType::Enum(n) => Ty::Enum(*self.enum_ix.get(&format!("{rule}.{n}")).expect("rule enums are registered first")),
            // rulec's `number` has no unit: it is an integer like dandori's `int`
            RType::Num { unit, .. } if unit == "number" => Ty::Int,
            RType::Num { unit, .. } => Ty::Num(unit.clone()),
        }
    }

    fn local_types(&mut self) {
        for en in &self.prog.enums {
            let (name, sp) = &en.name;
            if self.enum_ix.contains_key(name) || self.prog.records.iter().any(|r| r.name.0 == *name) {
                self.push(e("E006", *sp, format!("`{name}` is declared twice"), format!("`{name}` が二度宣言されています")));
                continue;
            }
            let mut values: Vec<String> = Vec::new();
            for (v, vsp) in &en.values {
                if v == "none" {
                    self.push(e("E006", *vsp, "`none` is kept for a case that has not started; name the value otherwise", "`none` は始まっていない案件を表す語なので、値には別の名前を付けてください"));
                } else if values.contains(v) {
                    self.push(e("E006", *vsp, format!("the value `{v}` is written twice"), format!("値 `{v}` が二度書かれています")));
                } else {
                    values.push(v.clone());
                }
            }
            self.enum_ix.insert(name.clone(), self.m.enums.len());
            self.m.enums.push(EnumDef { name: name.clone(), values });
        }
        // records: names first, so that a field can have a record type declared further down
        for r in &self.prog.records {
            let (name, sp) = &r.name;
            if self.record_ix.contains_key(name) {
                self.push(e("E006", *sp, format!("`{name}` is declared twice"), format!("`{name}` が二度宣言されています")));
                continue;
            }
            self.record_ix.insert(name.clone(), self.m.records.len());
            self.m.records.push(RecordDef { name: name.clone(), fields: vec![], origin: RecordOrigin::Local });
        }
        for r in &self.prog.records {
            let ix = match self.record_ix.get(&r.name.0) {
                Some(i) => *i,
                None => continue,
            };
            let mut fields = Vec::new();
            for f in &r.fields {
                if fields.iter().any(|(n, _): &(String, Ty)| *n == f.name.0) {
                    self.push(e("E006", f.name.1, format!("the field `{}` is written twice", f.name.0), format!("フィールド `{}` が二度書かれています", f.name.0)));
                    continue;
                }
                if let Some(t) = self.ty(&f.ty) {
                    if t == Ty::Record(ix) {
                        self.push(e("E003", f.ty.span(), "a record cannot contain itself", "レコードは自分自身を含められません"));
                        continue;
                    }
                    fields.push((f.name.0.clone(), t));
                }
            }
            self.m.records[ix].fields = fields;
        }
    }

    fn ty(&mut self, te: &TypeExpr) -> Option<Ty> {
        match te {
            TypeExpr::Int(_) => Some(Ty::Int),
            TypeExpr::Str(_) => Some(Ty::Str),
            TypeExpr::Bool(_) => Some(Ty::Bool),
            TypeExpr::Timestamp(_) => Some(Ty::Timestamp),
            TypeExpr::Json(_) => Some(Ty::Json),
            TypeExpr::List(inner, sp) => {
                let t = self.ty(inner)?;
                if matches!(t.inner(), Ty::List(_)) {
                    self.push(e("E003", *sp, "a list of lists is not supported; put the inner list in a record", "リストのリストは書けません。内側のリストはレコードに入れてください"));
                    return None;
                }
                Some(Ty::List(Box::new(t)))
            }
            TypeExpr::Opt(inner, _) => Some(Ty::Opt(Box::new(self.ty(inner)?))),
            TypeExpr::Unit(u, sp) => {
                let kind = u.split('[').next().unwrap_or("");
                if !syntax::UNIT_KINDS.contains(&kind) {
                    self.push(e("E002", *sp, format!("there is no type `{u}`"), format!("型 `{u}` はありません")).note(TYPE_HINT_EN, TYPE_HINT_JA));
                    return None;
                }
                Some(Ty::Num(rulec::normalize_unit(u)))
            }
            TypeExpr::Named(parts) => {
                let text = parts.iter().map(|p| p.0.as_str()).collect::<Vec<_>>().join(".");
                if let Some(i) = self.enum_ix.get(&text) {
                    return Some(Ty::Enum(*i));
                }
                if let Some(i) = self.record_ix.get(&text) {
                    return Some(Ty::Record(*i));
                }
                let sp = parts[0].1;
                let hint_en;
                let hint_ja;
                if parts.len() == 2 && self.rule_ix.contains_key(&parts[0].0) {
                    let r = &self.m.rules[self.rule_ix[&parts[0].0]];
                    let names: Vec<String> = r.info.enums.iter().map(|(n, _)| n.clone()).collect();
                    hint_en = format!("the enums of `{}` are {}", parts[0].0, names.join(", "));
                    hint_ja = format!("`{}` の列挙は {} です", parts[0].0, names.join("・"));
                } else {
                    hint_en = TYPE_HINT_EN.to_string();
                    hint_ja = TYPE_HINT_JA.to_string();
                }
                self.push(e("E002", sp, format!("there is no type `{text}`"), format!("型 `{text}` はありません")).note(hint_en, hint_ja));
                None
            }
        }
    }

    fn io(&mut self) {
        let mut seen: Vec<String> = vec![];
        for f in &self.prog.inputs {
            if seen.contains(&f.name.0) {
                self.push(e("E006", f.name.1, format!("the input `{}` is written twice", f.name.0), format!("入力 `{}` が二度書かれています", f.name.0)));
                continue;
            }
            seen.push(f.name.0.clone());
            if let Some(t) = self.ty(&f.ty) {
                self.m.inputs.push((f.name.0.clone(), t));
            }
        }
        let mut seen: Vec<String> = vec![];
        for f in &self.prog.outputs {
            if seen.contains(&f.name.0) {
                self.push(e("E006", f.name.1, format!("the output `{}` is written twice", f.name.0), format!("出力 `{}` が二度書かれています", f.name.0)));
                continue;
            }
            seen.push(f.name.0.clone());
            if let Some(t) = self.ty(&f.ty) {
                self.m.outputs.push((f.name.0.clone(), t));
            }
        }
    }

    fn tasks(&mut self) {
        for t in &self.prog.tasks {
            let (name, sp) = &t.name;
            if self.task_ix.contains_key(name) || self.rule_ix.contains_key(name) {
                self.push(e("E006", *sp, format!("`{name}` is declared twice (tasks and rules share names)"), format!("`{name}` が二度宣言されています（タスクと規則は名前を共有します）")));
                continue;
            }
            let mut params = Vec::new();
            for p in &t.params {
                if params.iter().any(|(n, _): &(String, Ty)| *n == p.name.0) {
                    self.push(e("E006", p.name.1, format!("the parameter `{}` is written twice", p.name.0), format!("引数 `{}` が二度書かれています", p.name.0)));
                    continue;
                }
                if let Some(ty) = self.ty(&p.ty) {
                    params.push((p.name.0.clone(), ty));
                }
            }
            let result = match &t.result {
                Some(te) => match self.ty(te) {
                    Some(r) => Some(r),
                    None => continue,
                },
                None => None,
            };
            let binding = t.binding.as_ref().map(|(b, _)| match b {
                syntax::Binding::Lambda(f) => Binding::Lambda(f.clone()),
                syntax::Binding::Http { method, url, form } => Binding::Http { method: method.clone(), url: url.clone(), form: *form },
                syntax::Binding::Aws { service, action } => Binding::Aws { service: service.clone(), action: action.clone() },
                syntax::Binding::Agent { provider, instructions } => Binding::Agent {
                    // an unknown provider is refused in `agent` below
                    provider: match provider.as_ref().map(|x| x.0.as_str()) {
                        Some("claude") => Provider::Claude,
                        _ => Provider::OpenAi,
                    },
                    instructions: instructions.clone(),
                    model: t.model.as_ref().map(|x| x.0.clone()).unwrap_or_default(),
                },
            });
            self.agent(t, result.as_ref());
            let child = t.workflow.as_ref().or(t.state_machine.as_ref()).or(t.durable_function.as_ref()).or(t.argo_template.as_ref()).map(|(_, s)| *s);
            if let Some((syntax::Binding::Aws { service, .. }, bsp)) = &t.binding {
                if crate::aws::exception_prefix(service).is_none() {
                    self.push(e(
                        "E007",
                        *bsp,
                        format!("Step Functions has no AWS SDK integration for the service `{service}`; write it as the Task's Resource names it, such as `dynamodb` or `secretsmanager`"),
                        format!("Step Functions の AWS SDK 統合に、サービス `{service}` はありません。`dynamodb` や `secretsmanager` のように、Task の Resource での名前で書いてください"),
                    ));
                }
            }
            let mut errors: Vec<ErrDef> = Vec::new();
            for er in &t.errors {
                let (en, esp) = &er.name;
                if en == "timeout" || en == "failure" {
                    self.push(e("E007", *esp, format!("`{en}` is always there; declare only the task's own errors"), format!("`{en}` は宣言しなくても使えます。タスク自身のエラーだけを宣言します")));
                    continue;
                }
                if errors.iter().any(|x| x.name == *en) {
                    self.push(e("E006", *esp, format!("the error `{en}` is written twice"), format!("エラー `{en}` が二度書かれています")));
                    continue;
                }
                if matches!(binding, Some(Binding::Agent { .. })) {
                    self.push(e(
                        "E007",
                        *esp,
                        "an agent declares no errors of its own: when the model refuses or the call fails it is `failure`, and past its time `timeout`",
                        "エージェントのタスクにはエラーを宣言できません。モデルが断ったときや呼び出しが失敗したときは `failure`、時間を過ぎたときは `timeout` になります",
                    ));
                    continue;
                }
                match (&binding, er.status, &er.exception) {
                    (Some(Binding::Http { .. }), None, _) => {
                        self.push(e("E007", *esp, format!("give `{en}` the HTTP status it comes back with, as `{en} = 402`"), format!("`{en}` が返ってくるときの HTTP ステータスを `{en} = 402` のように書きます")));
                    }
                    (Some(Binding::Http { .. }), Some(_), None) => {}
                    (Some(Binding::Aws { .. }), None, _) => {}
                    (Some(Binding::Aws { .. }), Some(_), _) => {
                        self.push(e("E007", *esp, format!("an AWS API's error is named by its exception, as `{en} = ConditionalCheckFailedException`, not by a status"), format!("AWS の API のエラーは、ステータスではなく `{en} = ConditionalCheckFailedException` のように例外の名前で書きます")));
                    }
                    (Some(Binding::Lambda(_)), None, None) => {}
                    (Some(Binding::Lambda(_)), _, _) => {
                        self.push(e("E007", *esp, "a Lambda task's error is named by the error type the function raises, without a status", "Lambda のタスクのエラーは関数が投げるエラーの型の名前で表し、ステータスは書きません"));
                    }
                    (None, None, None) => {}
                    (None, _, _) => {
                        self.push(e("E007", *esp, format!("`{en} = …` says how an HTTP or AWS call names the error; this task has neither `http` nor `aws`"), format!("`{en} = …` は HTTP や AWS の呼び出しでのエラーの表し方です。このタスクには `http` も `aws` もありません")));
                    }
                    (Some(Binding::Http { .. }), Some(_), Some(_)) => {}
                    // refused above
                    (Some(Binding::Agent { .. }), _, _) => {}
                }
                if let Some(st) = er.status {
                    if let Some(other) = errors.iter().find(|x| x.status == Some(st)) {
                        let other = other.name.clone();
                        self.push(e(
                            "E007",
                            *esp,
                            format!("`{other}` and `{en}` both come back as {st}; Step Functions tells errors apart by status, so give each its own"),
                            format!("`{other}` と `{en}` がどちらも {st} で返ってきます。Step Functions はエラーをステータスで見分けるので、別々のステータスにしてください"),
                        ));
                        continue;
                    }
                }
                errors.push(ErrDef { name: en.clone(), status: er.status, exception: er.exception.clone() });
            }
            if let Some((syntax::Binding::Http { url, .. }, bsp)) = t.binding.as_ref().map(|(b, s)| (b, s)) {
                for ph in placeholders(url) {
                    if !params.iter().any(|(n, _)| *n == ph) {
                        self.push(e("E007", *bsp, format!("the URL has `{{{ph}}}` but the task has no parameter `{ph}`"), format!("URL に `{{{ph}}}` がありますが、タスクに引数 `{ph}` がありません")));
                    }
                }
            }
            if let Some(csp) = t.callback {
                let ok = match &binding {
                    None | Some(Binding::Lambda(_)) => true,
                    Some(Binding::Aws { service, action }) => service == "sqs" && action == "sendMessage",
                    Some(Binding::Http { .. }) | Some(Binding::Agent { .. }) => false,
                };
                if !ok {
                    self.push(e(
                        "E007",
                        csp,
                        "a `callback` task hands its token over by `lambda` or by `aws sqs:sendMessage`",
                        "`callback` のタスクがトークンを渡せるのは `lambda` か `aws sqs:sendMessage` です",
                    ));
                } else if matches!(binding, Some(Binding::Aws { .. })) {
                    match params.iter().find(|(p, _)| p == "MessageBody").map(|(_, t)| t.inner().clone()) {
                        Some(Ty::Record(_)) | Some(Ty::Json) => {}
                        _ => self.push(e(
                            "E007",
                            csp,
                            "a callback through SQS puts its token into `MessageBody`, so the task needs a parameter `MessageBody` that is a record or `json`",
                            "SQS を通すコールバックはトークンを `MessageBody` に入れるので、レコードか `json` の引数 `MessageBody` が要ります",
                        )),
                    }
                }
                if let Some(c) = child {
                    self.push(e("E007", c, "a child workflow answers when it ends; it is not a `callback` task", "子ワークフローは終わったときに答えるので、`callback` のタスクにはなりません"));
                }
            }
            if let Some(ksp) = t.key {
                if child.is_some() {
                    self.push(e("E007", ksp, "the platform starts a child workflow once for each call, so `key` does not apply to it", "子ワークフローはプラットフォームが呼び出しごとに一度だけ始めるので、`key` は使えません"));
                }
                match (&binding, &t.key_param) {
                    (Some(Binding::Agent { .. }), _) => self.push(e("E007", ksp, "an agent changes nothing on the other side, so it takes no `key`", "エージェントは相手の側を何も変えないので、`key` は要りません")),
                    (Some(Binding::Aws { .. }), None) => self.push(e(
                        "E007",
                        ksp,
                        "an AWS API takes the idempotency key as one of its own parameters; write `key <parameter>`, as `key ClientToken`",
                        "AWS の API は冪等キーを自分の引数の一つで受け取ります。`key ClientToken` のように `key <引数>` と書いてください",
                    )),
                    (Some(Binding::Aws { .. }), Some((kp, kpsp))) => {
                        if params.iter().any(|(p, _)| p == kp) {
                            self.push(e("E007", *kpsp, format!("`{kp}` is filled by `key`; leave it out of the parameters"), format!("`{kp}` は `key` が埋めるので、引数からは外してください")));
                        }
                    }
                    (_, Some((_, kpsp))) => self.push(e("E007", *kpsp, "`key <parameter>` is for an AWS API; write just `key`", "`key <引数>` は AWS の API のための書き方です。ここでは `key` だけを書きます")),
                    _ => {}
                }
            }
            let retry = t.retry.as_ref().map(|r| {
                for (on, osp) in &r.on {
                    if on != "timeout" && on != "failure" && !errors.iter().any(|x| x.name == *on) {
                        self.diags.push(e("E002", *osp, format!("`{on}` is not an error of `{name}`"), format!("`{on}` は `{name}` のエラーではありません")));
                    }
                }
                Retry { times: r.times, every: r.every, backoff: r.backoff, on: r.on.iter().map(|x| x.0.clone()).collect() }
            });
            if let Some((ra, rsp)) = &t.refused_as {
                if !errors.iter().any(|x| x.name == *ra) {
                    self.push(e("E007", *rsp, format!("declare `{ra}` in `errors` too"), format!("`{ra}` を `errors` にも書いてください")));
                }
            }
            let machine = t.machine.as_ref().and_then(|(mu, msp)| match mu {
                MachineUse::Starts { machine, then } => {
                    let rule = self.machine_rule(machine, *msp)?;
                    Some(TaskMachine::Starts { rule, then: then.iter().map(|x| x.0.clone()).collect() })
                }
                MachineUse::Sends { event, column } => Some(TaskMachine::Sends { event: event.0.clone(), column: column.as_ref().map(|c| c.0.clone()) }),
                MachineUse::Observes => Some(TaskMachine::Observes),
            });
            if t.refused_as.is_some() && !matches!(machine, Some(TaskMachine::Sends { .. })) {
                self.push(e("E007", t.refused_as.as_ref().unwrap().1, "`refused as` belongs to a task that `sends` an event", "`refused as` は出来事を `sends` するタスクに書きます"));
            }
            if machine.is_some() && result.is_none() {
                self.push(e("E008", *sp, format!("`{name}` moves a case, so it answers with the case's record; write `-> <record>`"), format!("`{name}` は案件を動かすので、案件のレコードを答えます。`-> <レコード>` を書いてください")));
            }
            self.task_ix.insert(name.clone(), self.m.tasks.len());
            self.m.tasks.push(TaskDef {
                name: name.clone(),
                params,
                result,
                binding,
                connection: t.connection.clone(),
                queue: t.queue.clone(),
                workflow: t.workflow.as_ref().map(|x| x.0.clone()),
                state_machine: t.state_machine.as_ref().map(|x| x.0.clone()),
                durable_function: t.durable_function.as_ref().map(|x| x.0.clone()),
                image: t.image.as_ref().map(|x| x.0.clone()),
                argo_template: t.argo_template.as_ref().map(|x| x.0.clone()),
                errors,
                retry,
                timeout: t.timeout,
                key: t.key.is_some(),
                key_param: t.key_param.as_ref().map(|x| x.0.clone()),
                idempotent: t.idempotent,
                machine,
                refused_as: t.refused_as.as_ref().map(|x| x.0.clone()),
                callback: t.callback.is_some(),
                line: sp.line,
            });
        }
    }

    /// What an agent task must have, and what it cannot: a provider dandori knows, a model, an
    /// answer whose type the provider's structured outputs can hold the model to, and nothing that
    /// moves a case.
    fn agent(&mut self, t: &syntax::TaskDecl, result: Option<&Ty>) {
        let (provider, bsp) = match (&t.binding, &t.model) {
            (Some((syntax::Binding::Agent { provider, .. }, bsp)), _) => (provider.clone(), *bsp),
            (_, Some((_, msp))) => {
                self.push(e("E007", *msp, "`model` says which model an agent uses; this task has no `agent`", "`model` はエージェントが使うモデルを書くところです。このタスクには `agent` がありません"));
                return;
            }
            _ => return,
        };
        let provider = match provider {
            None => Provider::OpenAi,
            Some((p, _)) if p == "openai" => Provider::OpenAi,
            Some((p, _)) if p == "claude" => Provider::Claude,
            Some((p, psp)) => {
                self.push(e(
                    "E007",
                    psp,
                    format!("`{p}` is not an agent dandori knows; write `agent openai \"…\"` (or just `agent \"…\"`) or `agent claude \"…\"`"),
                    format!("`{p}` は dandori の知らないエージェントです。`agent openai \"…\"`（`agent \"…\"` だけでも同じ）か `agent claude \"…\"` と書いてください"),
                ));
                return;
            }
        };
        let (outputs_en, outputs_ja) = match provider {
            // with a space before the Japanese that follows an English word
            Provider::OpenAi => ("OpenAI's Structured Outputs", "OpenAI の Structured Outputs "),
            Provider::Claude => ("Claude's structured outputs", "Claude の構造化出力"),
        };
        if t.model.is_none() {
            let example = match provider {
                Provider::OpenAi => "gpt-5.4-mini",
                Provider::Claude => "claude-sonnet-5",
            };
            self.push(e(
                "E007",
                bsp,
                format!("an agent needs the model it uses; write it as `model \"{example}\"`"),
                format!("エージェントには、使うモデルを `model \"{example}\"` のように書いてください"),
            ));
        }
        if let Some((_, msp)) = &t.machine {
            self.push(e(
                "E007",
                *msp,
                "an agent reads what it is given and answers; it has no case on the other side to start, move or look at",
                "エージェントは渡されたものを読んで答えるだけで、相手の側の案件を始めたり動かしたり見たりはしません",
            ));
        }
        let Some(r) = result else {
            self.push(e("E007", bsp, "an agent answers; write the type of its answer as `-> <type>`", "エージェントは答えを返します。答えの型を `-> <型>` と書いてください"));
            return;
        };
        let Some(schema) = crate::render::agent_schema(&self.m, r) else {
            let (en, ja) = if has_json(&self.m, r, &mut Vec::new()) {
                (
                    format!("{outputs_en} hold an agent's answer to a JSON Schema, and `json` has none; give the answer a type without `json`"),
                    format!("エージェントの答えは、{outputs_ja}で JSON Schema に合わせて返させます。`json` は Schema に書けないので、`json` を含まない型にしてください"),
                )
            } else {
                (
                    format!("{outputs_en} hold an agent's answer to a JSON Schema written out in full, and a record that holds itself through others has no end; give the answer a type without it"),
                    format!("エージェントの答えは、{outputs_ja}で JSON Schema に合わせて返させます。ほかのレコードを通して自分を含むレコードは Schema に書き切れないので、それを含まない型にしてください"),
                )
            };
            self.push(e("E007", bsp, en, ja));
            return;
        };
        let mut over: Vec<(String, String)> = Vec::new();
        match provider {
            Provider::OpenAi => {
                let (depth, props, values) = crate::render::schema_size(&schema);
                if depth > 10 {
                    over.push((format!("its objects nest {depth} deep (at most 10)"), format!("オブジェクトの入れ子が {depth} 段（10 段まで）")));
                }
                if props > 5000 {
                    over.push((format!("it has {props} properties (at most 5,000)"), format!("プロパティが {props} 個（5,000 個まで）")));
                }
                if values > 1000 {
                    over.push((format!("it has {values} enum values (at most 1,000)"), format!("列挙の値が {values} 個（1,000 個まで）")));
                }
            }
            Provider::Claude => {
                let unions = crate::render::schema_unions(&schema);
                if unions > crate::render::CLAUDE_UNIONS {
                    over.push((
                        format!("it has {unions} values that may be absent, each a choice with null (at most {})", crate::render::CLAUDE_UNIONS),
                        format!("無いことがある値（null との選択）が {unions} 個（{} 個まで）", crate::render::CLAUDE_UNIONS),
                    ));
                }
            }
        }
        if !over.is_empty() {
            self.push(e(
                "E007",
                bsp,
                format!("the answer's JSON Schema, in `{{\"answer\": …}}`, is larger than {outputs_en} take: {}", over.iter().map(|x| x.0.clone()).collect::<Vec<_>>().join(", ")),
                format!("答えの JSON Schema（`{{\"answer\": …}}` に包んだもの）が、{outputs_ja}の受け付ける大きさを超えています。{}", over.iter().map(|x| x.1.clone()).collect::<Vec<_>>().join("、")),
            ));
        }
        if provider == Provider::Claude {
            // Claude may answer an enum's value in another case, which dandori takes as the value
            // it differs from only in case; two values that differ only in case would be one
            for en in crate::render::enums_of(&self.m, r) {
                let name = self.m.enums[en].name.clone();
                let values = self.m.enums[en].values.clone();
                for (i, a) in values.iter().enumerate() {
                    if let Some(b) = values[..i].iter().find(|b| b.to_lowercase() == a.to_lowercase()) {
                        self.push(e(
                            "E007",
                            bsp,
                            format!("Claude may answer an enum's value in another case, and dandori takes it as the value it matches without regard to case, so `{b}` and `{a}` of `{name}` would be the same; give them names that differ in more than case"),
                            format!("Claude は列挙の値の大文字と小文字を変えて答えることがあり、dandori は大文字と小文字を区別せずに値を読みます。このため `{name}` の `{b}` と `{a}` は同じ値になります。大文字と小文字のほかにも違いのある名前にしてください"),
                        ));
                    }
                }
            }
        }
    }

    /// `payment_intent.payment`: a rule that is used, and its machine's name.
    fn machine_rule(&mut self, q: &[(String, Span)], sp: Span) -> Option<usize> {
        if q.len() != 2 {
            self.push(e("E002", sp, "name a machine as <rule>.<machine>", "ステートマシンは <規則>.<ステートマシン> と書きます"));
            return None;
        }
        let rix = match self.rule_ix.get(&q[0].0) {
            Some(r) => *r,
            None => {
                self.push(e("E002", q[0].1, format!("no rule `{}` is used", q[0].0), format!("規則 `{}` は読み込まれていません", q[0].0)));
                return None;
            }
        };
        match &self.m.rules[rix].info.machine {
            Some(mc) if mc.name == q[1].0 => Some(rix),
            Some(mc) => {
                let n = mc.name.clone();
                self.push(e("E002", q[1].1, format!("the machine of `{}` is `{n}`", q[0].0), format!("`{}` のステートマシンは `{n}` です", q[0].0)));
                None
            }
            None => {
                self.push(e("E002", q[0].1, format!("`{}` has no machine", q[0].0), format!("`{}` にはステートマシンがありません", q[0].0)));
                None
            }
        }
    }

    fn cases(&mut self) {
        for c in &self.prog.cases {
            let (name, sp) = &c.name;
            if self.m.cases.iter().any(|x| x.name == *name) {
                self.push(e("E006", *sp, format!("the case `{name}` is declared twice"), format!("案件 `{name}` が二度宣言されています")));
                continue;
            }
            let record = match self.ty(&c.record) {
                Some(Ty::Record(r)) if self.m.records[r].origin == RecordOrigin::Local => r,
                Some(_) => {
                    self.push(e("E008", c.record.span(), "a case is held in a record declared in this file", "案件の型には、このファイルで宣言したレコードを使います"));
                    continue;
                }
                None => continue,
            };
            let rule = match self.machine_rule(&c.machine, c.machine[0].1) {
                Some(r) => r,
                None => continue,
            };
            let mc = self.m.rules[rule].info.machine.clone().unwrap();
            let state_enum = *self.enum_ix.get(&format!("{}.{}", self.m.rules[rule].name, mc.state_enum)).unwrap();
            let fields = self.m.records[record].fields.clone();
            let state_field = match &c.state_field {
                Some((f, fsp)) => match fields.iter().find(|(n, _)| n == f) {
                    Some((_, t)) if *t == Ty::Enum(state_enum) => f.clone(),
                    Some(_) => {
                        self.push(e("E008", *fsp, format!("`{f}` is not of the machine's state type"), format!("`{f}` はステートマシンの状態の型ではありません")));
                        continue;
                    }
                    None => {
                        self.push(e("E002", *fsp, format!("`{}` has no field `{f}`", self.m.records[record].name), format!("`{}` にフィールド `{f}` はありません", self.m.records[record].name)));
                        continue;
                    }
                },
                None => {
                    let cands: Vec<&String> = fields.iter().filter(|(_, t)| *t == Ty::Enum(state_enum)).map(|(n, _)| n).collect();
                    match cands.len() {
                        1 => cands[0].clone(),
                        0 => {
                            self.push(e(
                                "E008",
                                c.record.span(),
                                format!("`{}` has no field of type `{}` to hold the case's state", self.m.records[record].name, self.m.enums[state_enum].name),
                                format!("`{}` に、案件の状態を入れる `{}` 型のフィールドがありません", self.m.records[record].name, self.m.enums[state_enum].name),
                            ));
                            continue;
                        }
                        _ => {
                            self.push(e("E008", c.record.span(), "more than one field could hold the state; write `state <field>`", "状態を入れられるフィールドが二つ以上あります。`state <フィールド>` を書いてください"));
                            continue;
                        }
                    }
                }
            };
            let mut held = Vec::new();
            let mut held_values = Vec::new();
            for ((inp, isp), (val, vsp)) in &c.held {
                if !mc.held.contains(inp) {
                    self.push(e(
                        "E008",
                        *isp,
                        format!("`{inp}` is not held by the machine; its held inputs are {}", if mc.held.is_empty() { "none".to_string() } else { mc.held.join(", ") }),
                        format!("`{inp}` はステートマシンの held ではありません（held は {}）", if mc.held.is_empty() { "ありません".to_string() } else { mc.held.join("・") }),
                    ));
                    continue;
                }
                if let Some(a) = mc.axis_of(inp) {
                    match mc.axes[a].coords.iter().position(|x| x == val) {
                        Some(ci) => held.push((a, ci)),
                        None => {
                            self.push(e("E003", *vsp, format!("`{val}` is not a value of `{inp}` ({})", mc.axes[a].coords.join(", ")), format!("`{val}` は `{inp}` の値ではありません（{}）", mc.axes[a].coords.join("・"))));
                            continue;
                        }
                    }
                }
                held_values.push((inp.clone(), val.clone()));
            }
            let mut external = Vec::new();
            for (ev, esp) in &c.external {
                let axes = mc.axes_with_value(ev);
                match axes.len() {
                    1 => external.push((axes[0], mc.axes[axes[0]].coords.iter().position(|x| x == ev).unwrap(), ev.clone())),
                    0 => self.push(e("E008", *esp, format!("no input of the machine has the event `{ev}`"), format!("ステートマシンのどの入力にも出来事 `{ev}` はありません"))),
                    _ => self.push(e("E008", *esp, format!("more than one input has the value `{ev}`"), format!("値 `{ev}` を持つ入力が二つ以上あります"))),
                }
            }
            let refused_when = match &c.refused_when {
                Some(((o, osp), (v, _))) => match mc.decides.iter().position(|d| d == o) {
                    Some(i) => Some((i, v.clone())),
                    None => {
                        self.push(e("E008", *osp, format!("the machine's table does not write `{o}`; it writes {}", mc.decides.join(", ")), format!("ステートマシンの表は `{o}` を書きません（書くのは {}）", mc.decides.join("・"))));
                        None
                    }
                },
                None => None,
            };
            self.m.cases.push(CaseDef { name: name.clone(), record, rule, state_field, held, held_values, external, refused_when, line: sp.line });
        }
    }

    /// Every variable and its type, before the statements are read: inputs, cases, and every
    /// name a statement sets — `let`, `for`, `some`. A value's type can depend on another
    /// variable's, so the names are read again until no new one turns up.
    fn variables(&mut self) {
        for (n, t) in self.m.inputs.clone() {
            self.var_ty.insert(n, t);
        }
        for c in self.m.cases.clone() {
            if self.var_ty.contains_key(&c.name) {
                self.push(Diag::error("E006", c.line, 1, format!("`{}` is both an input and a case", c.name), format!("`{}` が入力と案件の両方にあります", c.name)));
            }
            self.var_ty.insert(c.name.clone(), Ty::Record(c.record));
        }
        let mut problems: BTreeMap<(usize, usize), Diag> = BTreeMap::new();
        loop {
            let before = self.var_ty.len();
            self.widened = false;
            let blocks: Vec<Block> = [&self.prog.flow, &self.prog.on_failure, &self.prog.on_cancel].iter().filter_map(|b| b.as_ref().map(|(b, _)| b.clone())).collect();
            for b in &blocks {
                self.scan_vars(b, &mut problems);
            }
            if self.var_ty.len() == before && !self.widened {
                break;
            }
            problems.clear();
        }
        for (_, d) in problems {
            self.push(d);
        }
    }

    fn reserved(&self, n: &str) -> bool {
        self.m.cases.iter().any(|c| c.name == n) || self.m.inputs.iter().any(|(i, _)| *i == n)
    }

    fn declare(&mut self, n: &str, sp: Span, t: Ty, problems: &mut BTreeMap<(usize, usize), Diag>) {
        if self.reserved(n) {
            problems.insert(
                (sp.line, sp.col),
                e("E006", sp, format!("`{n}` is an input or a case; a variable set here needs a new name"), format!("`{n}` は入力か案件です。ここで値を入れる変数には別の名前を付けてください")),
            );
            return;
        }
        match self.var_ty.get(n).cloned() {
            // a value that fits the name's type is set to it; a name set to both `T` and `T?` is `T?`
            Some(prev) if t.fits(&prev) => {}
            Some(prev) if prev.fits(&t) && matches!(t, Ty::Opt(_)) => {
                self.var_ty.insert(n.to_string(), t);
                self.widened = true;
            }
            Some(prev) if prev != t => {
                let (a, b) = (self.m.ty_name(&prev), self.m.ty_name(&t));
                problems.insert(
                    (sp.line, sp.col),
                    e("E003", sp, format!("`{n}` was `{a}` elsewhere and is `{b}` here; a name keeps one type"), format!("`{n}` はほかの場所で `{a}`、ここで `{b}` です。名前の型は一つです")),
                );
            }
            Some(_) => {}
            None => {
                self.var_ty.insert(n.to_string(), t);
            }
        }
    }

    /// Run `f` for its answer only: what it would say is dropped.
    fn quiet<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let n = self.diags.len();
        let out = f(self);
        self.diags.truncate(n);
        out
    }

    fn quiet_ty(&mut self, te: &TypeExpr) -> Option<Ty> {
        self.quiet(|x| x.ty(te))
    }

    fn quiet_expr(&mut self, ex: &Expr, want: Option<&Ty>) -> Option<Ty> {
        self.quiet(|x| x.expr(ex, want).map(|t| t.ty()))
    }

    fn scan_vars(&mut self, b: &Block, problems: &mut BTreeMap<(usize, usize), Diag>) {
        for s in b {
            match &s.kind {
                StmtKind::Let { name, ty, call, handlers } => {
                    let t = match ty {
                        Some(te) => self.quiet_ty(te),
                        None => self.callee_result(call),
                    };
                    if let Some(t) = t {
                        self.declare(&name.0, name.1, t, problems);
                    }
                    for h in handlers {
                        self.scan_vars(&h.body, problems);
                    }
                }
                StmtKind::Assign { name, ty, expr } => {
                    let t = match ty {
                        Some(te) => self.quiet_ty(te),
                        None => self.quiet_expr(expr, None),
                    };
                    if let Some(t) = t {
                        self.declare(&name.0, name.1, t, problems);
                    }
                }
                StmtKind::Call { handlers, .. } | StmtKind::CaseCall { handlers, .. } => {
                    for h in handlers {
                        self.scan_vars(&h.body, problems);
                    }
                }
                StmtKind::Match { expr, arms } => {
                    for a in arms {
                        if let Some((v, vsp)) = &a.some {
                            if let Some(Ty::Opt(inner)) = self.quiet_expr(expr, None) {
                                self.declare(v, *vsp, *inner, problems);
                            }
                        }
                        self.scan_vars(&a.body, problems);
                    }
                }
                StmtKind::Repeat { body, .. } => self.scan_vars(body, problems),
                StmtKind::For { var, list, body, result, .. } => {
                    if let Some(Ty::List(elem)) = self.quiet_expr(list, None) {
                        self.declare(&var.0, var.1, *elem, problems);
                    }
                    self.scan_vars(body, problems);
                    if let Some((r, rty)) = result {
                        let t = match rty {
                            Some(te) => self.quiet_ty(te),
                            None => match body.last().map(|x| &x.kind) {
                                Some(StmtKind::Yield { expr }) => self.quiet_expr(expr, None).map(|t| Ty::List(Box::new(t))),
                                _ => None,
                            },
                        };
                        if let Some(t) = t {
                            self.declare(&r.0, r.1, t, problems);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn callee_result(&self, call: &Call) -> Option<Ty> {
        if let Some(t) = self.task_ix.get(&call.callee.0) {
            return self.m.tasks[*t].result.clone();
        }
        if let Some(r) = self.rule_ix.get(&call.callee.0) {
            return Some(Ty::Record(self.m.rules[*r].outputs));
        }
        None
    }

    // -----------------------------------------------------------------------
    // Statements

    fn block(&mut self, b: &Block) -> Vec<TStmt> {
        let mut out = Vec::new();
        for s in b {
            if let Some(t) = self.stmt(s) {
                out.push(t);
            }
        }
        out
    }

    fn stmt(&mut self, s: &syntax::Stmt) -> Option<TStmt> {
        let site = self.next_site();
        let line = s.span.line;
        let kind = match &s.kind {
            StmtKind::Let { name, ty, call, handlers } => {
                let (callee, args) = self.call(call)?;
                let result = match &callee {
                    Callee::Task(t) => self.m.tasks[*t].result.clone(),
                    Callee::Rule(r) => Some(Ty::Record(self.m.rules[*r].outputs)),
                };
                let result = match result {
                    Some(r) => r,
                    None => {
                        self.push(e("E003", call.callee.1, format!("`{}` answers nothing to keep; call it without `let`", call.callee.0), format!("`{}` は何も答えません。`let` を付けずに呼んでください", call.callee.0)));
                        return None;
                    }
                };
                if let Some(te) = ty {
                    let want = self.ty(te)?;
                    if !result.fits(&want) {
                        let (a, b) = (self.m.ty_name(&result), self.m.ty_name(&want));
                        self.push(e("E003", call.callee.1, format!("`{}` answers `{a}`, which is not `{b}`", call.callee.0), format!("`{}` が答えるのは `{a}` で、`{b}` ではありません", call.callee.0)));
                        return None;
                    }
                }
                let hs = self.handlers(&callee, handlers);
                if !self.var_ty.contains_key(&name.0) || self.reserved(&name.0) {
                    return None;
                }
                TK::Call { target: Some(Target::Let(name.0.clone())), callee, args, handlers: hs }
            }
            StmtKind::Assign { name, ty, expr } => {
                let want = match ty {
                    Some(te) => Some(self.ty(te)?),
                    None => self.var_ty.get(&name.0).cloned(),
                };
                if want.is_none() {
                    if let Expr::Record(_, sp) = expr {
                        self.push(e("E003", *sp, format!("write the record's type: `let {}: <record> = {{…}}`", name.0), format!("レコードの型を `let {}: <レコード> = {{…}}` のように書いてください", name.0)));
                        return None;
                    }
                }
                let x = self.expr(expr, want.as_ref())?;
                if self.reserved(&name.0) || !self.var_ty.contains_key(&name.0) {
                    return None;
                }
                TK::Assign { name: name.0.clone(), expr: x }
            }
            StmtKind::Call { call, handlers } => {
                let (callee, args) = self.call(call)?;
                if let Callee::Rule(_) = callee {
                    self.push(e("E009", call.callee.1, "a rule only answers; keep its answer with `let`", "規則は答えを返すだけです。`let` で答えを受けてください"));
                    return None;
                }
                let hs = self.handlers(&callee, handlers);
                TK::Call { target: None, callee, args, handlers: hs }
            }
            StmtKind::CaseCall { case, call, handlers } => {
                let ci = match self.m.case_index(&case.0) {
                    Some(c) => c,
                    None => {
                        self.push(e("E002", case.1, format!("there is no case `{}`", case.0), format!("案件 `{}` はありません", case.0)));
                        return None;
                    }
                };
                let (callee, args) = self.call(call)?;
                let ti = match callee {
                    Callee::Task(t) => t,
                    Callee::Rule(_) => {
                        self.push(e("E008", call.callee.1, "a case moves by a task that `starts`, `sends` or `observes`; a rule is called with `let`", "案件を動かすのは `starts`・`sends`・`observes` を書いたタスクです。規則は `let` で呼びます"));
                        return None;
                    }
                };
                if self.par_depth > 0 {
                    self.push(e(
                        "E009",
                        case.1,
                        format!("the case `{}` cannot be moved from inside `for … in parallel`, where the rounds run at the same time", case.0),
                        format!("同時に回る `for … in parallel` の中からは、案件 `{}` を動かせません", case.0),
                    ));
                    return None;
                }
                let task = self.m.tasks[ti].clone();
                if task.result != Some(Ty::Record(self.m.cases[ci].record)) {
                    let a = task.result.as_ref().map(|r| self.m.ty_name(r)).unwrap_or_else(|| "nothing".into());
                    let b = self.m.records[self.m.cases[ci].record].name.clone();
                    self.push(e("E003", call.callee.1, format!("`{}` returns `{a}`, but the case `{}` is held in `{b}`", task.name, case.0), format!("`{}` が返すのは `{a}` ですが、案件 `{}` の型は `{b}` です", task.name, case.0)));
                    return None;
                }
                match &task.machine {
                    None => {
                        self.push(e("E008", call.callee.1, format!("`{}` does not say what it does to a case; write `starts`, `sends` or `observes` under it", task.name), format!("`{}` には、案件に何をするかが書かれていません。`starts`・`sends`・`observes` のどれかを書いてください", task.name)));
                        return None;
                    }
                    Some(TaskMachine::Starts { rule, .. }) if *rule != self.m.cases[ci].rule => {
                        self.push(e("E008", call.callee.1, format!("`{}` starts a case of another machine", task.name), format!("`{}` が始めるのは別のステートマシンの案件です", task.name)));
                        return None;
                    }
                    _ => {}
                }
                let hs = self.handlers(&callee, handlers);
                TK::Call { target: Some(Target::Case(ci)), callee, args, handlers: hs }
            }
            StmtKind::Match { expr, arms } => self.lower_match(expr, arms)?,
            StmtKind::Wait { seconds } => {
                if *seconds == 0 {
                    self.push(e("E009", s.span, "a wait of zero does nothing", "0 の待ちは何もしません"));
                }
                TK::Wait { seconds: *seconds }
            }
            StmtKind::WaitUntil { at } => {
                let te = self.expr(at, Some(&Ty::Timestamp))?;
                TK::WaitUntil { at: te }
            }
            StmtKind::Repeat { times, body } => {
                self.loops.push(false);
                let b = self.block(body);
                self.loops.pop();
                TK::Repeat { times: *times, body: b }
            }
            StmtKind::For { var, list, max, parallel, body, result } => {
                let lx = self.expr(list, None)?;
                let elem = match lx.ty() {
                    Ty::List(t) => *t,
                    Ty::Opt(_) => {
                        self.push(e("E003", list.span(), format!("`{}` may be absent here; `match` it with `none` and `some <name>` first", lx.show()), format!("ここでは `{}` が無いことがあります。先に `none` と `some <名前>` で `match` してください", lx.show())));
                        return None;
                    }
                    other => {
                        let n = self.m.ty_name(&other);
                        self.push(e("E003", list.span(), format!("`for` goes through a list; this is `{n}`"), format!("`for` で回せるのはリストです。これは `{n}` です")));
                        return None;
                    }
                };
                if self.reserved(&var.0) {
                    self.push(e("E006", var.1, format!("`{}` is an input or a case; the loop's variable needs a new name", var.0), format!("`{}` は入力か案件です。ループの変数には別の名前を付けてください", var.0)));
                    return None;
                }
                if self.var_ty.get(&var.0) != Some(&elem) {
                    return None;
                }
                let mut body = body.clone();
                let yielded = match result {
                    Some((r, _)) => match body.last().map(|x| x.kind.clone()) {
                        Some(StmtKind::Yield { expr }) => {
                            body.pop();
                            Some((r.clone(), expr))
                        }
                        _ => {
                            self.push(e("E009", s.span, format!("`let {} = for …` needs `yield <value>` as the last line of its body", r.0), format!("`let {} = for …` の本体の最後の行には `yield <値>` が要ります", r.0)));
                            return None;
                        }
                    },
                    None => None,
                };
                self.loops.push(parallel.is_some());
                if parallel.is_some() {
                    self.par_depth += 1;
                }
                let b = self.block(&body);
                let result = match yielded {
                    Some((r, ex)) => {
                        let want = match self.var_ty.get(&r.0) {
                            Some(Ty::List(t)) => Some((**t).clone()),
                            _ => None,
                        };
                        let x = self.expr(&ex, want.as_ref());
                        match x {
                            Some(x) => Some((r.0.clone(), x)),
                            None => {
                                self.loops.pop();
                                if parallel.is_some() {
                                    self.par_depth -= 1;
                                }
                                return None;
                            }
                        }
                    }
                    None => None,
                };
                if parallel.is_some() {
                    self.par_depth -= 1;
                }
                self.loops.pop();
                if let Some((r, _)) = &result {
                    if self.reserved(r) || !self.var_ty.contains_key(r) {
                        return None;
                    }
                }
                TK::For { var: var.0.clone(), list: lx, max: *max, parallel: *parallel, body: b, result, locals: vec![] }
            }
            StmtKind::Yield { expr } => {
                self.push(e("E009", expr.span(), "`yield` is the last line of the body of `let <name> = for …`", "`yield` は `let <名前> = for …` の本体の最後の行に書きます"));
                return None;
            }
            StmtKind::Pass => TK::Pass,
            StmtKind::Break => {
                match self.loops.last() {
                    None => {
                        self.push(e("E009", s.span, "`break` is written inside `repeat` or `for`", "`break` は `repeat` か `for` の中に書きます"));
                        return None;
                    }
                    Some(true) => {
                        self.push(e("E009", s.span, "the rounds of `for … in parallel` run at the same time, so there is no `break` from them", "`for … in parallel` の各回は同時に回るので、`break` で抜けられません"));
                        return None;
                    }
                    Some(false) => {}
                }
                TK::Break
            }
            StmtKind::Succeed { fields } => {
                if self.in_on_failure {
                    self.push(e("E009", s.span, "`on failure` ends the workflow as failed; it cannot `succeed`", "`on failure` はワークフローを失敗で終えます。`succeed` は書けません"));
                    return None;
                }
                if self.in_on_cancel {
                    self.push(e("E009", s.span, "`on cancel` ends the workflow as cancelled; it cannot `succeed`", "`on cancel` はワークフローをキャンセルされたとして終えます。`succeed` は書けません"));
                    return None;
                }
                if self.par_depth > 0 {
                    self.push(e("E009", s.span, "the workflow cannot `succeed` from inside `for … in parallel`, where other rounds may still run", "ほかの回がまだ動いていることがあるので、`for … in parallel` の中からは `succeed` できません"));
                    return None;
                }
                let mut out = Vec::new();
                for ((n, nsp), ex) in fields {
                    let oty = match self.m.outputs.iter().find(|(o, _)| o == n) {
                        Some((_, t)) => t.clone(),
                        None => {
                            self.push(e("E002", *nsp, format!("there is no output `{n}`"), format!("出力 `{n}` はありません")));
                            continue;
                        }
                    };
                    if out.iter().any(|(o, _): &(String, TExpr)| o == n) {
                        self.push(e("E006", *nsp, format!("`{n}` is given twice"), format!("`{n}` が二度書かれています")));
                        continue;
                    }
                    if let Some(te) = self.expr(ex, Some(&oty)) {
                        out.push((n.clone(), te));
                    }
                }
                let missing: Vec<String> = self
                    .m
                    .outputs
                    .iter()
                    .filter(|(o, t)| !matches!(t, Ty::Opt(_)) && !out.iter().any(|(g, _)| g == o) && !fields.iter().any(|((f, _), _)| f == o))
                    .map(|(o, _)| o.clone())
                    .collect();
                if !missing.is_empty() {
                    self.push(e("E004", s.span, format!("`succeed` does not give {}", missing.join(", ")), format!("`succeed` が {} を書いていません", missing.join("・"))));
                }
                TK::Succeed { fields: out }
            }
            StmtKind::Fail { error, cause, leaving } => {
                let mut left = Vec::new();
                for (l, lsp) in leaving {
                    match self.m.case_index(l) {
                        Some(c) => left.push(c),
                        None => self.push(e("E002", *lsp, format!("there is no case `{l}`"), format!("案件 `{l}` はありません"))),
                    }
                }
                let cause = match cause {
                    Some(c) => Some(self.expr(c, Some(&Ty::Str))?),
                    None => None,
                };
                TK::Fail { error: error.0.clone(), cause, leaving: left }
            }
        };
        Some(TStmt { kind, line, site })
    }

    fn lower_match(&mut self, expr: &Expr, arms: &[syntax::Arm]) -> Option<TK> {
        let te = self.expr(expr, None)?;
        let ty = te.ty();
        let (domain, optional): (Vec<String>, bool) = match &ty {
            Ty::Enum(e) => (self.m.enums[*e].values.clone(), false),
            Ty::Bool => (vec!["true".into(), "false".into()], false),
            Ty::Opt(inner) => match &**inner {
                Ty::Enum(e) => (self.m.enums[*e].values.clone(), true),
                Ty::Bool => (vec!["true".into(), "false".into()], true),
                _ => (vec![], true),
            },
            other => {
                let n = self.m.ty_name(other);
                self.push(e(
                    "E009",
                    expr.span(),
                    format!("`match` works on an enum, a bool, or a value that may be absent (`T?`); this is `{n}`"),
                    format!("`match` に渡せるのは列挙・bool・無いことがある値（`T?`）です。これは `{n}` です"),
                ));
                return None;
            }
        };
        let is_case_state = match &te {
            TExpr::Var { name, fields, .. } => fields.len() == 1 && self.m.case_index(name).map(|c| self.m.cases[c].state_field == fields[0]).unwrap_or(false),
            _ => false,
        };
        let has_some = arms.iter().any(|a| a.some.is_some());
        let has_values = arms.iter().any(|a| a.values.iter().any(|(v, _)| v != "none"));
        let mut seen: Vec<String> = Vec::new();
        let mut tarms = Vec::new();
        for a in arms {
            if let Some((v, vsp)) = &a.some {
                if !optional {
                    self.push(e("E003", *vsp, "`some` is written when matching a value that may be absent (`T?`)", "`some` を書けるのは、無いことがある値（`T?`）で分けるときだけです"));
                    continue;
                }
                if seen.iter().any(|x| x == "some") {
                    self.push(e("E011", a.span, "`some` already has an arm above", "`some` の行き先は上にもう書かれています"));
                    continue;
                }
                if has_values {
                    self.push(e("E003", a.span, "`some` stands for every value that is there; write either the values or `some`", "`some` は値があるときのすべてを表します。値を並べるか `some` を書くかのどちらかにしてください"));
                    continue;
                }
                seen.push("some".into());
                if self.reserved(v) {
                    self.push(e("E006", *vsp, format!("`{v}` is an input or a case; `some` needs a new name"), format!("`{v}` は入力か案件です。`some` には別の名前を付けてください")));
                    continue;
                }
                let body = self.block(&a.body);
                tarms.push(TArm { values: vec![], none: false, some: Some(v.clone()), body, line: a.span.line });
                continue;
            }
            let mut values = Vec::new();
            let mut none = false;
            for (v, vsp) in &a.values {
                if seen.contains(v) {
                    self.push(e("E011", *vsp, format!("`{v}` already has an arm above"), format!("`{v}` の行き先は上にもう書かれています")));
                    continue;
                }
                seen.push(v.clone());
                if v == "none" {
                    if is_case_state || optional {
                        none = true;
                    } else {
                        self.push(e("E003", *vsp, "`none` is written when matching a case's state, or a value that may be absent (`T?`)", "`none` を書けるのは、案件の状態か、無いことがある値（`T?`）で分けるときだけです"));
                    }
                    continue;
                }
                if optional && domain.is_empty() {
                    let n = self.m.ty_name(&ty);
                    self.push(e("E003", *vsp, format!("`{}` is `{n}`; its arms are `none` and `some <name>`", te.show()), format!("`{}` は `{n}` です。行き先は `none` と `some <名前>` です", te.show())));
                    continue;
                }
                if !domain.contains(v) {
                    let n = self.m.ty_name(ty.inner());
                    self.push(e("E003", *vsp, format!("`{v}` is not a value of `{n}` ({})", domain.join(", ")), format!("`{v}` は `{n}` の値ではありません（{}）", domain.join("・"))));
                    continue;
                }
                values.push(v.clone());
            }
            let _ = has_some;
            let body = self.block(&a.body);
            tarms.push(TArm { values, none, some: None, body, line: a.span.line });
        }
        Some(TK::Match { expr: te, arms: tarms })
    }

    fn handlers(&mut self, callee: &Callee, hs: &[syntax::Handler]) -> Vec<THandler> {
        let declared: Vec<String> = match callee {
            Callee::Task(t) => self.m.tasks[*t].errors.iter().map(|x| x.name.clone()).collect(),
            Callee::Rule(_) => vec![],
        };
        let mut seen: Vec<String> = Vec::new();
        let mut out = Vec::new();
        for h in hs {
            let mut errs = Vec::new();
            for (n, sp) in &h.errors {
                if seen.contains(n) {
                    self.push(e("E011", *sp, format!("`{n}` is handled above already"), format!("`{n}` は上ですでに受けています")));
                    continue;
                }
                seen.push(n.clone());
                match n.as_str() {
                    "timeout" => errs.push(HErr::Timeout),
                    "failure" => errs.push(HErr::Failure),
                    _ if declared.contains(n) => errs.push(HErr::Declared(n.clone())),
                    _ => {
                        let list = if declared.is_empty() { "timeout, failure".to_string() } else { format!("{}, timeout, failure", declared.join(", ")) };
                        self.push(e("E002", *sp, format!("`{n}` is not an error this call can raise ({list})"), format!("`{n}` はこの呼び出しが投げるエラーではありません（{list}）")));
                    }
                }
            }
            let body = self.block(&h.body);
            out.push(THandler { errors: errs, body, line: h.span.line });
        }
        // `failure` catches everything the arms before it did not; an arm after it never runs
        let mut after_failure = false;
        for (i, h) in hs.iter().enumerate() {
            if after_failure {
                self.push(e("E011", h.span, "this arm comes after `on failure`, which already takes every error", "この行き先は、すべてのエラーを受ける `on failure` のあとにあります"));
            }
            if out.get(i).map(|x| x.errors.contains(&HErr::Failure)).unwrap_or(false) {
                after_failure = true;
            }
        }
        out
    }

    fn call(&mut self, c: &Call) -> Option<(Callee, Vec<(String, TExpr)>)> {
        let (name, sp) = &c.callee;
        let (callee, params): (Callee, Vec<(String, Ty)>) = if let Some(t) = self.task_ix.get(name) {
            (Callee::Task(*t), self.m.tasks[*t].params.clone())
        } else if let Some(r) = self.rule_ix.get(name) {
            let r = *r;
            let rn = self.m.rules[r].name.clone();
            let ps = self.m.rules[r].info.inputs.iter().map(|col| (col.name.clone(), self.rty(&rn, &col.ty))).collect();
            (Callee::Rule(r), ps)
        } else {
            self.push(e("E002", *sp, format!("there is no task or rule `{name}`"), format!("タスクか規則 `{name}` はありません")));
            return None;
        };
        let mut args = Vec::new();
        let mut ok = true;
        for ((an, asp), ex) in &c.args {
            let pty = match params.iter().find(|(p, _)| p == an) {
                Some((_, t)) => t.clone(),
                None => {
                    let list: Vec<&str> = params.iter().map(|(p, _)| p.as_str()).collect();
                    self.push(e("E004", *asp, format!("`{name}` has no parameter `{an}` ({})", list.join(", ")), format!("`{name}` に引数 `{an}` はありません（{}）", list.join("・"))));
                    ok = false;
                    continue;
                }
            };
            if args.iter().any(|(a, _): &(String, TExpr)| a == an) {
                self.push(e("E004", *asp, format!("`{an}` is given twice"), format!("`{an}` が二度書かれています")));
                ok = false;
                continue;
            }
            match self.expr(ex, Some(&pty)) {
                Some(te) => args.push((an.clone(), te)),
                None => ok = false,
            }
        }
        // an optional parameter may be left out; it is then sent as null
        let missing: Vec<String> = params.iter().filter(|(p, t)| !matches!(t, Ty::Opt(_)) && !c.args.iter().any(|((a, _), _)| a == p)).map(|(p, _)| p.clone()).collect();
        if !missing.is_empty() {
            self.push(e("E004", *sp, format!("the call of `{name}` does not give {}", missing.join(", ")), format!("`{name}` の呼び出しに {} がありません", missing.join("・"))));
            ok = false;
        }
        if !ok {
            return None;
        }
        for (p, t) in &params {
            if matches!(t, Ty::Opt(_)) && !args.iter().any(|(a, _)| a == p) {
                args.push((p.clone(), TExpr::None(t.clone())));
            }
        }
        // keep the parameters' order, so that the generated code reads like the declaration
        args.sort_by_key(|(a, _)| params.iter().position(|(p, _)| p == a).unwrap_or(usize::MAX));
        Some((callee, args))
    }

    fn expr(&mut self, ex: &Expr, expected: Option<&Ty>) -> Option<TExpr> {
        let te = match ex {
            Expr::Str(s, _) => TExpr::Str(s.clone()),
            Expr::Int(n, _) => TExpr::Int(*n),
            Expr::Bool(b, _) => TExpr::Bool(*b),
            Expr::Interp(parts, sp) => {
                let mut out = Vec::new();
                for p in parts {
                    match p {
                        Part::Lit(s) => out.push(IPart::Lit(s.clone())),
                        Part::Hole(path) => {
                            let x = self.path(path, None)?;
                            let t = x.ty();
                            if !matches!(t, Ty::Str | Ty::Int | Ty::Num(_) | Ty::Bool | Ty::Enum(_) | Ty::Timestamp) {
                                let n = self.m.ty_name(&t);
                                let (en, ja) = if matches!(t, Ty::Opt(_)) {
                                    (
                                        format!("`{}` is `{n}` and may be absent, so it cannot be put in a string as it is; `match` it with `none` and `some <name>` first", x.show()),
                                        format!("`{}` は `{n}` で、無いことがあるので、そのままでは文字列に入れられません。先に `none` と `some <名前>` で `match` してください", x.show()),
                                    )
                                } else {
                                    (format!("`{}` is `{n}`, which cannot be put in a string", x.show()), format!("`{}` は `{n}` なので、文字列に入れられません", x.show()))
                                };
                                self.push(e("E003", *sp, en, ja));
                                return None;
                            }
                            out.push(IPart::Hole(x));
                        }
                    }
                }
                TExpr::Interp(out)
            }
            Expr::Record(fields, sp) => {
                let want = expected.map(|t| t.inner().clone());
                match want {
                    Some(Ty::Record(r)) => {
                        let decl = self.m.records[r].fields.clone();
                        let rn = self.m.records[r].name.clone();
                        let mut out: Vec<(String, TExpr)> = Vec::new();
                        for ((fname, fsp), fe) in fields {
                            let fty = match decl.iter().find(|(n, _)| n == fname) {
                                Some((_, t)) => t.clone(),
                                None => {
                                    let list: Vec<String> = decl.iter().map(|(n, _)| n.clone()).collect();
                                    self.push(e("E002", *fsp, format!("`{rn}` has no field `{fname}` ({})", list.join(", ")), format!("`{rn}` にフィールド `{fname}` はありません（{}）", list.join("・"))));
                                    return None;
                                }
                            };
                            if out.iter().any(|(n, _)| n == fname) {
                                self.push(e("E006", *fsp, format!("`{fname}` is given twice"), format!("`{fname}` が二度書かれています")));
                                return None;
                            }
                            let x = self.expr(fe, Some(&fty))?;
                            out.push((fname.clone(), x));
                        }
                        let missing: Vec<String> = decl.iter().filter(|(n, t)| !matches!(t, Ty::Opt(_)) && !out.iter().any(|(g, _)| g == n)).map(|(n, _)| n.clone()).collect();
                        if !missing.is_empty() {
                            self.push(e("E004", *sp, format!("this `{rn}` does not give {}", missing.join(", ")), format!("この `{rn}` に {} がありません", missing.join("・"))));
                            return None;
                        }
                        out.sort_by_key(|(n, _)| decl.iter().position(|(d, _)| d == n).unwrap_or(usize::MAX));
                        TExpr::Record { fields: out, ty: Ty::Record(r) }
                    }
                    Some(Ty::Json) => {
                        let mut out: Vec<(String, TExpr)> = Vec::new();
                        for ((fname, fsp), fe) in fields {
                            if out.iter().any(|(n, _)| n == fname) {
                                self.push(e("E006", *fsp, format!("`{fname}` is given twice"), format!("`{fname}` が二度書かれています")));
                                return None;
                            }
                            let x = self.expr(fe, Some(&Ty::Json))?;
                            out.push((fname.clone(), x));
                        }
                        TExpr::Record { fields: out, ty: Ty::Json }
                    }
                    _ => {
                        self.push(e(
                            "E003",
                            *sp,
                            "a record written as `{…}` takes its type from where it goes: an argument, an output, or `let x: <record> = {…}`",
                            "`{…}` で書いたレコードは、型が決まるところ（引数・出力・`let x: <レコード> = {…}`）に書きます",
                        ));
                        return None;
                    }
                }
            }
            Expr::List(items, sp) => {
                let want_elem = match expected.map(|t| t.inner()) {
                    Some(Ty::List(t)) => Some((**t).clone()),
                    Some(Ty::Json) => Some(Ty::Json),
                    _ => None,
                };
                let mut elem = want_elem;
                let mut out = Vec::new();
                for it in items {
                    let x = self.expr(it, elem.as_ref())?;
                    if elem.is_none() {
                        elem = Some(x.ty());
                    }
                    out.push(x);
                }
                let elem = match elem {
                    Some(t) => t,
                    None => {
                        self.push(e("E003", *sp, "the type of an empty list is not known here; write it where a list is expected, or as `let x: list[T] = []`", "空のリストの型がここでは分かりません。リストを渡す先に書くか、`let x: list[T] = []` と書いてください"));
                        return None;
                    }
                };
                if matches!(elem.inner(), Ty::List(_)) {
                    self.push(e("E003", *sp, "a list of lists is not supported; put the inner list in a record", "リストのリストは書けません。内側のリストはレコードに入れてください"));
                    return None;
                }
                TExpr::List { items: out, ty: Ty::List(Box::new(elem)) }
            }
            Expr::Path(parts) => {
                if parts.len() == 1 && parts[0].0 == "none" && !self.var_ty.contains_key("none") {
                    match expected {
                        Some(t @ Ty::Opt(_)) => return Some(TExpr::None(t.clone())),
                        Some(Ty::Json) => return Some(TExpr::None(Ty::Opt(Box::new(Ty::Json)))),
                        _ => {
                            self.push(e("E003", parts[0].1, "`none` is given only where a value may be absent (`T?`)", "`none` を渡せるのは、無いことがある値（`T?`）のところだけです"));
                            return None;
                        }
                    }
                }
                self.path(parts, expected)?
            }
        };
        if let Some(want) = expected {
            let got = te.ty();
            let fits = got.fits(want) || (matches!(te, TExpr::Int(_)) && matches!(want.inner(), Ty::Num(_)));
            if !fits {
                let (a, b) = (self.m.ty_name(want), self.m.ty_name(&got));
                let (en, ja) = if matches!(got, Ty::Opt(_)) && got.inner().fits(want) {
                    (
                        format!("expected `{a}` here, but this is `{b}`, which may be absent; `match` it with `none` and `some <name>` first"),
                        format!("ここには `{a}` が要りますが、これは無いことがある `{b}` です。先に `none` と `some <名前>` で `match` してください"),
                    )
                } else {
                    (format!("expected `{a}` here, but this is `{b}`"), format!("ここには `{a}` が要りますが、これは `{b}` です"))
                };
                self.push(e("E003", ex.span(), en, ja));
                return None;
            }
        }
        Some(te)
    }

    /// A variable and its fields: `pi.status`. A bare name that is no variable may be a value of the enum expected here.
    fn path(&mut self, parts: &[(String, Span)], expected: Option<&Ty>) -> Option<TExpr> {
        let (first, fsp) = &parts[0];
        match self.var_ty.get(first).cloned() {
            Some(mut t) => {
                let mut fields = Vec::new();
                for (f, fsp2) in &parts[1..] {
                    let so_far = std::iter::once(first.clone()).chain(fields.iter().cloned()).collect::<Vec<String>>().join(".");
                    let rec = match t {
                        Ty::Record(r) => r,
                        Ty::Opt(ref inner) if matches!(**inner, Ty::Record(_)) => {
                            self.push(e(
                                "E003",
                                *fsp2,
                                format!("`{so_far}` may be absent here; `match` it with `none` and `some <name>` first"),
                                format!("ここでは `{so_far}` が無いことがあります。先に `none` と `some <名前>` で `match` してください"),
                            ));
                            return None;
                        }
                        Ty::Json => {
                            self.push(e("E003", *fsp2, format!("`{so_far}` is `json`, which is carried as it is and not looked into"), format!("`{so_far}` は `json` で、中を見ずにそのまま運ぶ値です")));
                            return None;
                        }
                        ref other => {
                            let n = self.m.ty_name(other);
                            self.push(e("E003", *fsp2, format!("`{n}` has no fields"), format!("`{n}` にはフィールドがありません")));
                            return None;
                        }
                    };
                    match self.m.field_ty(rec, f) {
                        Some(ft) => {
                            t = ft.clone();
                            fields.push(f.clone());
                        }
                        None => {
                            let rn = self.m.records[rec].name.clone();
                            let list: Vec<String> = self.m.records[rec].fields.iter().map(|(n, _)| n.clone()).collect();
                            self.push(e("E002", *fsp2, format!("`{rn}` has no field `{f}` ({})", list.join(", ")), format!("`{rn}` にフィールド `{f}` はありません（{}）", list.join("・"))));
                            return None;
                        }
                    }
                }
                Some(TExpr::Var { name: first.clone(), fields, ty: t })
            }
            None => {
                if parts.len() == 1 {
                    if let Some(Ty::Enum(id)) = expected.map(|t| t.inner()) {
                        if self.m.enums[*id].values.contains(first) {
                            return Some(TExpr::Enum(first.clone(), *id));
                        }
                    }
                }
                let (hen, hja) = match expected.map(|t| t.inner()) {
                    Some(Ty::Enum(id)) => (
                        format!("the values of `{}` are {}", self.m.enums[*id].name, self.m.enums[*id].values.join(", ")),
                        format!("`{}` の値は {} です", self.m.enums[*id].name, self.m.enums[*id].values.join("・")),
                    ),
                    _ => ("a value is a variable, a field of one, a string, a number, true or false".to_string(), "値は変数・そのフィールド・文字列・数・true・false のどれかです".to_string()),
                };
                self.push(e("E002", *fsp, format!("there is no variable or value `{first}`"), format!("変数や値 `{first}` はありません")).note(hen, hja));
                None
            }
        }
    }

    /// The variables a `for … in parallel` round sets are its own. A name set inside such a
    /// round and also outside it would be one variable shared by rounds that run at the
    /// same time; that is refused. `locals` gets the names each round keeps.
    fn parallel_scopes(&mut self) {
        // for every name: the scopes that set it (the path of parallel loops around, by site)
        let mut sets: BTreeMap<String, BTreeSet<Vec<usize>>> = BTreeMap::new();
        fn put(sets: &mut BTreeMap<String, BTreeSet<Vec<usize>>>, n: &str, sc: &[usize]) {
            sets.entry(n.to_string()).or_default().insert(sc.to_vec());
        }
        fn visit(ss: &[TStmt], scope: &mut Vec<usize>, sets: &mut BTreeMap<String, BTreeSet<Vec<usize>>>) {
            for s in ss {
                match &s.kind {
                    TK::Call { target, handlers, .. } => {
                        if let Some(Target::Let(v)) = target {
                            put(sets, v, scope);
                        }
                        for h in handlers {
                            visit(&h.body, scope, sets);
                        }
                    }
                    TK::Assign { name, .. } => put(sets, name, scope),
                    TK::Match { arms, .. } => {
                        for a in arms {
                            if let Some(v) = &a.some {
                                put(sets, v, scope);
                            }
                            visit(&a.body, scope, sets);
                        }
                    }
                    TK::Repeat { body, .. } => visit(body, scope, sets),
                    TK::For { var, parallel, body, result, .. } => {
                        if let Some((r, _)) = result {
                            put(sets, r, scope);
                        }
                        if parallel.is_some() {
                            scope.push(s.site);
                            put(sets, var, scope);
                            visit(body, scope, sets);
                            scope.pop();
                        } else {
                            put(sets, var, scope);
                            visit(body, scope, sets);
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut scope = Vec::new();
        visit(&self.m.flow, &mut scope, &mut sets);
        if let Some(f) = &self.m.on_failure {
            visit(f, &mut scope, &mut sets);
        }
        if let Some(f) = &self.m.on_cancel {
            visit(f, &mut scope, &mut sets);
        }
        // a name may be set in sibling rounds of different loops, but never in two scopes one of which holds the other
        let mut bad: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        for (n, scs) in &sets {
            let list: Vec<&Vec<usize>> = scs.iter().collect();
            for i in 0..list.len() {
                for j in 0..list.len() {
                    if i != j && list[j].len() > list[i].len() && list[j].starts_with(list[i]) {
                        bad.entry(*list[j].last().unwrap()).or_default().push(n.clone());
                    }
                }
            }
        }
        let mut lines: BTreeMap<usize, usize> = BTreeMap::new();
        Model::walk(&self.m.flow, &mut |s| {
            lines.insert(s.site, s.line);
        });
        for f in [&self.m.on_failure, &self.m.on_cancel].into_iter().flatten() {
            Model::walk(f, &mut |s| {
                lines.insert(s.site, s.line);
            });
        }
        for (site, mut names) in bad {
            names.sort();
            names.dedup();
            let line = lines.get(&site).cloned().unwrap_or(1);
            let list = names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ");
            self.push(Diag::error(
                "E009",
                line,
                1,
                format!("{list} is set both inside this `for … in parallel` and outside it; the rounds run at the same time and each keeps its own variables, so give them their own names"),
                format!("{list} は、この `for … in parallel` の中と外の両方で値を入れられています。各回は同時に回り、自分の変数を持つので、別の名前にしてください"),
            ));
        }
        // each parallel loop's own names
        let mut own: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        for (n, scs) in &sets {
            for sc in scs {
                if let Some(site) = sc.last() {
                    own.entry(*site).or_default().push(n.clone());
                }
            }
        }
        fn fill(ss: &mut [TStmt], own: &BTreeMap<usize, Vec<String>>) {
            for s in ss {
                let site = s.site;
                match &mut s.kind {
                    TK::Call { handlers, .. } => handlers.iter_mut().for_each(|h| fill(&mut h.body, own)),
                    TK::Match { arms, .. } => arms.iter_mut().for_each(|a| fill(&mut a.body, own)),
                    TK::Repeat { body, .. } => fill(body, own),
                    TK::For { body, locals, parallel, .. } => {
                        if parallel.is_some() {
                            *locals = own.get(&site).cloned().unwrap_or_default();
                        }
                        fill(body, own);
                    }
                    _ => {}
                }
            }
        }
        let mut flow = std::mem::take(&mut self.m.flow);
        fill(&mut flow, &own);
        self.m.flow = flow;
        if let Some(mut f) = self.m.on_failure.take() {
            fill(&mut f, &own);
            self.m.on_failure = Some(f);
        }
        if let Some(mut f) = self.m.on_cancel.take() {
            fill(&mut f, &own);
            self.m.on_cancel = Some(f);
        }
    }
}

/// `{id}` in a URL
pub fn placeholders(url: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = url;
    while let Some(i) = rest.find('{') {
        if let Some(j) = rest[i..].find('}') {
            out.push(rest[i + 1..i + j].to_string());
            rest = &rest[i + j + 1..];
        } else {
            break;
        }
    }
    out
}

/// Whether a value of type `t` can hold `json` somewhere inside.
fn has_json(m: &Model, t: &Ty, within: &mut Vec<RecordId>) -> bool {
    match t {
        Ty::Json => true,
        Ty::List(x) | Ty::Opt(x) => has_json(m, x, within),
        Ty::Record(r) => {
            if within.contains(r) {
                return false;
            }
            within.push(*r);
            let found = m.records[*r].fields.iter().any(|(_, ft)| has_json(m, ft, within));
            within.pop();
            found
        }
        _ => false,
    }
}
