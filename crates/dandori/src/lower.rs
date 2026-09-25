//! The first pass: read the rules through rulec, resolve every name, type every value,
//! and turn the statements into the checked tree of `model`. The run-dependent checks
//! (states of cases, assignment, exhaustiveness, exits) are the second pass, in `flow`.

use crate::diag::Diag;
use crate::model::*;
use crate::rulec::{self, RType};
use crate::syntax::{self, Block, Call, Expr, MachineUse, Program, Span, StmtKind, TypeExpr};
use std::collections::BTreeMap;
use std::path::Path;

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
    loop_depth: usize,
    in_on_failure: bool,
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
            monitors: BTreeMap::new(),
        },
        enum_ix: BTreeMap::new(),
        record_ix: BTreeMap::new(),
        rule_ix: BTreeMap::new(),
        task_ix: BTreeMap::new(),
        var_ty: BTreeMap::new(),
        site: 0,
        loop_depth: 0,
        in_on_failure: false,
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
    }
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
            self.m.rules.push(RuleUse { name: name.clone(), info, lambda: u.lambda.clone(), outputs: rec, line: sp.line });
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
            TypeExpr::Unit(u, _) => Some(Ty::Num(rulec::normalize_unit(u))),
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
                    hint_en = "a type is int, string, bool, timestamp, a unit such as money[円, incl_tax], an enum or a record".to_string();
                    hint_ja = "型は int・string・bool・timestamp・money[円, incl_tax] のような単位・列挙・レコードのどれかです".to_string();
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
            let result = match self.ty(&t.result) {
                Some(r) => r,
                None => continue,
            };
            let binding = t.binding.as_ref().map(|(b, _)| match b {
                syntax::Binding::Lambda(f) => Binding::Lambda(f.clone()),
                syntax::Binding::Http { method, url, form } => Binding::Http { method: method.clone(), url: url.clone(), form: *form },
            });
            let mut errors: Vec<(String, Option<u16>)> = Vec::new();
            for er in &t.errors {
                let (en, esp) = &er.name;
                if en == "timeout" || en == "failure" {
                    self.push(e("E007", *esp, format!("`{en}` is always there; declare only the task's own errors"), format!("`{en}` は宣言しなくても使えます。タスク自身のエラーだけを宣言します")));
                    continue;
                }
                if errors.iter().any(|(n, _)| n == en) {
                    self.push(e("E006", *esp, format!("the error `{en}` is written twice"), format!("エラー `{en}` が二度書かれています")));
                    continue;
                }
                match (&binding, er.status) {
                    (Some(Binding::Http { .. }), None) => {
                        self.push(e("E007", *esp, format!("give `{en}` the HTTP status it comes back with, as `{en} = 402`"), format!("`{en}` が返ってくるときの HTTP ステータスを `{en} = 402` のように書きます")));
                    }
                    (Some(Binding::Lambda(_)), Some(_)) => {
                        self.push(e("E007", *esp, "a Lambda task's error is named by the error type the function raises, without a status", "Lambda のタスクのエラーは関数が投げるエラーの型の名前で表し、ステータスは書きません"));
                    }
                    _ => {}
                }
                if let Some(st) = er.status {
                    if let Some((other, _)) = errors.iter().find(|(_, s2)| *s2 == Some(st)) {
                        self.push(e(
                            "E007",
                            *esp,
                            format!("`{other}` and `{en}` both come back as {st}; Step Functions tells errors apart by status, so give each its own"),
                            format!("`{other}` と `{en}` がどちらも {st} で返ってきます。Step Functions はエラーをステータスで見分けるので、別々のステータスにしてください"),
                        ));
                        continue;
                    }
                }
                errors.push((en.clone(), er.status));
            }
            if let Some((syntax::Binding::Http { url, .. }, bsp)) = t.binding.as_ref().map(|(b, s)| (b, s)) {
                for ph in placeholders(url) {
                    if !params.iter().any(|(n, _)| *n == ph) {
                        self.push(e("E007", *bsp, format!("the URL has `{{{ph}}}` but the task has no parameter `{ph}`"), format!("URL に `{{{ph}}}` がありますが、タスクに引数 `{ph}` がありません")));
                    }
                }
                if t.connection.is_none() {
                    self.push(Diag::warning("W104", bsp.line, bsp.col, "an HTTP task on Step Functions needs `connection \"<EventBridge connection ARN>\"`", "Step Functions で HTTP のタスクを呼ぶには `connection \"<EventBridge の接続の ARN>\"` が要ります"));
                }
            }
            if t.binding.is_none() {
                self.push(Diag::warning("W104", sp.line, sp.col, format!("`{name}` says neither `lambda` nor `http`, so it cannot be built for Step Functions"), format!("`{name}` には `lambda` も `http` も無いので、Step Functions 向けには作れません")));
            }
            if t.callback && !matches!(binding, Some(Binding::Lambda(_)) | None) {
                self.push(e("E007", *sp, "a `callback` task hands its token to a Lambda function; write `lambda`", "`callback` のタスクはトークンを Lambda 関数に渡します。`lambda` を書いてください"));
            }
            let retry = t.retry.as_ref().map(|r| {
                for (on, osp) in &r.on {
                    if on != "timeout" && on != "failure" && !errors.iter().any(|(n, _)| n == on) {
                        self.diags.push(e("E002", *osp, format!("`{on}` is not an error of `{name}`"), format!("`{on}` は `{name}` のエラーではありません")));
                    }
                }
                Retry { times: r.times, every: r.every, backoff: r.backoff, on: r.on.iter().map(|x| x.0.clone()).collect() }
            });
            if let Some((ra, rsp)) = &t.refused_as {
                if !errors.iter().any(|(n, _)| n == ra) {
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
            self.task_ix.insert(name.clone(), self.m.tasks.len());
            self.m.tasks.push(TaskDef {
                name: name.clone(),
                params,
                result,
                binding,
                connection: t.connection.clone(),
                errors,
                retry,
                timeout: t.timeout,
                key: t.key.is_some(),
                idempotent: t.idempotent,
                machine,
                refused_as: t.refused_as.as_ref().map(|x| x.0.clone()),
                callback: t.callback,
                line: sp.line,
            });
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

    /// Every variable and its type, before the statements are read: inputs, cases, `let`s.
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
        let mut lets: Vec<(String, Span, Call)> = Vec::new();
        fn scan(b: &Block, out: &mut Vec<(String, Span, Call)>) {
            for s in b {
                match &s.kind {
                    StmtKind::Let { name, call, handlers } => {
                        out.push((name.0.clone(), name.1, call.clone()));
                        for h in handlers {
                            scan(&h.body, out);
                        }
                    }
                    StmtKind::CaseCall { handlers, .. } => {
                        for h in handlers {
                            scan(&h.body, out);
                        }
                    }
                    StmtKind::Match { arms, .. } => {
                        for a in arms {
                            scan(&a.body, out);
                        }
                    }
                    StmtKind::Repeat { body, .. } => scan(body, out),
                    _ => {}
                }
            }
        }
        if let Some((b, _)) = &self.prog.flow {
            scan(b, &mut lets);
        }
        if let Some((b, _)) = &self.prog.on_failure {
            scan(b, &mut lets);
        }
        for (n, sp, call) in lets {
            let t = match self.callee_result(&call) {
                Some(t) => t,
                None => continue,
            };
            if self.m.cases.iter().any(|c| c.name == n) || self.m.inputs.iter().any(|(i, _)| *i == n) {
                self.push(e("E006", sp, format!("`{n}` is an input or a case; a `let` needs a new name"), format!("`{n}` は入力か案件です。`let` には別の名前を付けてください")));
                continue;
            }
            match self.var_ty.get(&n) {
                Some(prev) if *prev != t => {
                    let (a, b) = (self.m.ty_name(prev), self.m.ty_name(&t));
                    self.push(e("E003", sp, format!("`{n}` was `{a}` elsewhere and is `{b}` here; a name keeps one type"), format!("`{n}` はほかの場所で `{a}`、ここで `{b}` です。名前の型は一つです")));
                }
                _ => {
                    self.var_ty.insert(n, t);
                }
            }
        }
    }

    fn callee_result(&self, call: &Call) -> Option<Ty> {
        if let Some(t) = self.task_ix.get(&call.callee.0) {
            return Some(self.m.tasks[*t].result.clone());
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
            StmtKind::Let { name, call, handlers } => {
                let (callee, args) = self.call(call)?;
                let hs = self.handlers(&callee, handlers);
                if !self.var_ty.contains_key(&name.0) {
                    return None;
                }
                TK::Call { target: Some(Target::Let(name.0.clone())), callee, args, handlers: hs }
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
                let task = self.m.tasks[ti].clone();
                if task.result != Ty::Record(self.m.cases[ci].record) {
                    let (a, b) = (self.m.ty_name(&task.result), self.m.records[self.m.cases[ci].record].name.clone());
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
            StmtKind::Match { expr, arms } => {
                let te = self.expr(expr, None)?;
                let ty = te.ty();
                let domain: Vec<String> = match &ty {
                    Ty::Enum(e) => self.m.enums[*e].values.clone(),
                    Ty::Bool => vec!["true".into(), "false".into()],
                    other => {
                        let n = self.m.ty_name(other);
                        self.push(e("E009", expr.span(), format!("`match` works on an enum or a bool; this is `{n}`"), format!("`match` に渡せるのは列挙か bool です。これは `{n}` です")));
                        return None;
                    }
                };
                let is_case_state = match &te {
                    TExpr::Var { name, fields, .. } => {
                        fields.len() == 1 && self.m.case_index(name).map(|c| self.m.cases[c].state_field == fields[0]).unwrap_or(false)
                    }
                    _ => false,
                };
                let mut seen: Vec<String> = Vec::new();
                let mut tarms = Vec::new();
                for a in arms {
                    let mut values = Vec::new();
                    let mut none = false;
                    for (v, vsp) in &a.values {
                        if seen.contains(v) {
                            self.push(e("E011", *vsp, format!("`{v}` already has an arm above"), format!("`{v}` の行き先は上にもう書かれています")));
                            continue;
                        }
                        seen.push(v.clone());
                        if v == "none" {
                            if is_case_state {
                                none = true;
                            } else {
                                self.push(e("E003", *vsp, "`none` is written only when matching a case's state", "`none` を書けるのは案件の状態で分けるときだけです"));
                            }
                            continue;
                        }
                        if !domain.contains(v) {
                            self.push(e("E003", *vsp, format!("`{v}` is not a value of `{}` ({})", self.m.ty_name(&ty), domain.join(", ")), format!("`{v}` は `{}` の値ではありません（{}）", self.m.ty_name(&ty), domain.join("・"))));
                            continue;
                        }
                        values.push(v.clone());
                    }
                    let body = self.block(&a.body);
                    tarms.push(TArm { values, none, body, line: a.span.line });
                }
                TK::Match { expr: te, arms: tarms }
            }
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
                self.loop_depth += 1;
                let b = self.block(body);
                self.loop_depth -= 1;
                TK::Repeat { times: *times, body: b }
            }
            StmtKind::Pass => TK::Pass,
            StmtKind::Break => {
                if self.loop_depth == 0 {
                    self.push(e("E009", s.span, "`break` is written inside `repeat`", "`break` は `repeat` の中に書きます"));
                    return None;
                }
                TK::Break
            }
            StmtKind::Succeed { fields } => {
                if self.in_on_failure {
                    self.push(e("E009", s.span, "`on failure` ends the workflow as failed; it cannot `succeed`", "`on failure` はワークフローを失敗で終えます。`succeed` は書けません"));
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
                let missing: Vec<String> = self.m.outputs.iter().filter(|(o, _)| !out.iter().any(|(g, _)| g == o) && !fields.iter().any(|((f, _), _)| f == o)).map(|(o, _)| o.clone()).collect();
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
                TK::Fail { error: error.0.clone(), cause: cause.clone(), leaving: left }
            }
        };
        Some(TStmt { kind, line, site })
    }

    fn handlers(&mut self, callee: &Callee, hs: &[syntax::Handler]) -> Vec<THandler> {
        let declared: Vec<String> = match callee {
            Callee::Task(t) => self.m.tasks[*t].errors.iter().map(|(n, _)| n.clone()).collect(),
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
        let missing: Vec<String> = params.iter().filter(|(p, _)| !c.args.iter().any(|((a, _), _)| a == p)).map(|(p, _)| p.clone()).collect();
        if !missing.is_empty() {
            self.push(e("E004", *sp, format!("the call of `{name}` does not give {}", missing.join(", ")), format!("`{name}` の呼び出しに {} がありません", missing.join("・"))));
            ok = false;
        }
        if !ok {
            return None;
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
            Expr::Path(parts) => {
                let (first, fsp) = &parts[0];
                match self.var_ty.get(first).cloned() {
                    Some(mut t) => {
                        let mut fields = Vec::new();
                        for (f, fsp2) in &parts[1..] {
                            let rec = match t {
                                Ty::Record(r) => r,
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
                        TExpr::Var { name: first.clone(), fields, ty: t }
                    }
                    None => {
                        if parts.len() == 1 {
                            if let Some(Ty::Enum(id)) = expected {
                                if self.m.enums[*id].values.contains(first) {
                                    return Some(TExpr::Enum(first.clone(), *id));
                                }
                            }
                        }
                        let (hen, hja) = match expected {
                            Some(Ty::Enum(id)) => (
                                format!("the values of `{}` are {}", self.m.enums[*id].name, self.m.enums[*id].values.join(", ")),
                                format!("`{}` の値は {} です", self.m.enums[*id].name, self.m.enums[*id].values.join("・")),
                            ),
                            _ => ("a value is a variable, a field of one, a string, a number, true or false".to_string(), "値は変数・そのフィールド・文字列・数・true・false のどれかです".to_string()),
                        };
                        self.push(e("E002", *fsp, format!("there is no variable or value `{first}`"), format!("変数や値 `{first}` はありません")).note(hen, hja));
                        return None;
                    }
                }
            }
        };
        if let Some(want) = expected {
            let got = te.ty();
            let fits = got == *want || (matches!(te, TExpr::Int(_)) && matches!(want, Ty::Num(_)));
            if !fits {
                let (a, b) = (self.m.ty_name(want), self.m.ty_name(&got));
                self.push(e("E003", ex.span(), format!("expected `{a}` here, but this is `{b}`"), format!("ここには `{a}` が要りますが、これは `{b}` です")));
                return None;
            }
        }
        Some(te)
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
