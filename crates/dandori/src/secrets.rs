//! What a flow does with keys and secrets (DESIGN 1.18; ritsu's DESIGN 16): a key written into the
//! `.flow` (W901), a secret kept in the history of a run (W904), a secret sent to a party outside the
//! project (E906), and, for ritsu's port of flows, every call that gives a secret to a file of the
//! project (`sends`), which ritsu-cross holds to the map (its E905). W902, a URL called without
//! encryption, is said where the URL is lowered (`lower`).
//!
//! A value is a secret when it is read from a place marked secret: an input, an output, a field, a
//! parameter or an answer whose type `secret` follows; a field of a record made from a message of a
//! `.proto` whose field is marked `debug_redact`; a parameter or the answer of a task held to an
//! OpenAPI operation whose property is marked (`x-data-classification`, `x-sensitive-data`,
//! `format: password`). A record holds the secrets of its fields, a list those of its items, a string
//! with values put in is a secret when one of them is, and a `json` value made from a secret is one.
//! What a rule answers is a decision, not a secret, and the answer of a task with no mark is none.
//! What a variable holds is gathered from every value put in it anywhere in the flow, as its range is
//! (DESIGN 1.3): not along the runs, so a value put in on a run that never happens counts too.

use crate::diag::{Diag, Text};
use crate::model::*;
use std::collections::BTreeMap;

/// A secret a value holds: where inside the value (empty: the value itself), how the flow writes the
/// value it is read as (`account.number`), and its mark.
#[derive(Clone, Debug, PartialEq)]
pub struct Held {
    pub path: Vec<String>,
    pub shown: String,
    pub mark: SecretMark,
}

/// Where a call gives secrets: the line of the call, the task or the rule called, the file of the
/// project it sends to, as dandori reaches it, each secret given with the parameter that carries it,
/// and the parameters the task says it discloses, with why.
#[derive(Clone, Debug, PartialEq)]
pub struct Sent {
    pub line: usize,
    pub callee: String,
    pub to: String,
    pub secrets: Vec<(String, Held)>,
    pub disclosed: Vec<(String, String)>,
}

/// The scheme and the host of a URL that goes unencrypted (`http://`, `ws://`) to a host that is not
/// this machine: ritsu's one judgement of it (ritsu's DESIGN 16.4).
pub fn plain_url(url: &str) -> Option<(String, String)> {
    ritsu_base::urls::plaintext(url)
}

/// The host of an absolute URL, and whether it is this machine.
fn host_of(url: &str) -> Option<(String, bool)> {
    let (_, host) = ritsu_base::urls::scheme_and_host(url)?;
    let local = ritsu_base::urls::is_loopback(&host);
    Some((host, local))
}

/// The check of a flow that lowers: W904 for each secret its runs keep in the history (none when
/// it says `history encrypted`), E906 for each secret it sends outside the project that its task
/// does not say it discloses. W901, a key in the text, is `keys`, which needs no model.
pub fn check(m: &Model) -> Vec<Diag> {
    let mut out = Vec::new();
    let vars = variables(m);
    if m.history_encrypted.is_none() {
        out.extend(kept(m, &vars));
    }
    out.extend(sent_outside(m, &vars));
    out
}

// ── W901 ────────────────────────────────────────────────────────────────────

/// W901: each key of a kind ritsu knows, written in the `.flow` (in a string or a comment alike),
/// whose line does not say `ritsu: test secret`. The message carries the key's prefix and length,
/// not the key, and so does the line shown (`Diag::render` masks every key on it).
pub fn keys(src: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    for f in ritsu_base::secrets::scan(src).into_iter().filter(|f| !f.test) {
        let (name, shown, len) = (&f.kind.name, f.shown.clone(), f.len);
        let provider = f.kind.provider;
        let revoke = if provider.is_empty() {
            tr!("本物の鍵なら、まず別の鍵に替えてください。ファイルから消しても、リポジトリの履歴には残ります。", "If this key is real, replace it first: taking it out of the file leaves it in the history of the repository.")
        } else {
            tr!("本物の鍵なら、まず {provider} で無効にしてください。ファイルから消しても、リポジトリの履歴には残ります。", "If this key is real, revoke it with {provider} first: taking it out of the file leaves it in the history of the repository.")
        };
        let d = Diag::warning("W901", f.line, f.col, tr!("{}がここに書かれています（{shown}、{len} 文字）", "{} is written here ({shown}, {len} characters)", name.ja; name.en))
            .note(tr!(
                "ファイルに書いた鍵は、リポジトリとその履歴とビルドを読めるすべての人に渡ります。鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。",
                "A key in a file reaches everyone who can read the repository, its history and its builds. Keep it where the code runs (an environment variable, the platform's connection or secret store) and read it from there."
            ))
            .note(revoke)
            .note(tr!("テスト用の値なら、同じ行のコメントに `{}` と書いてください。", "If it is a value for tests, write `{}` in a comment on the same line.", ritsu_base::secrets::TEST_MARK));
        out.push(d);
    }
    out
}

// ── What a value holds ──────────────────────────────────────────────────────

fn joined(shown: &str, path: &[String]) -> String {
    std::iter::once(shown).chain(path.iter().map(String::as_str)).collect::<Vec<_>>().join(".")
}

/// Put `h` in `out`, unless a secret at the same place with the same mark is there.
fn add(out: &mut Vec<Held>, h: Held) {
    if !out.iter().any(|x| x.path == h.path && x.mark == h.mark) {
        out.push(h);
    }
}

/// The secrets a value of type `t` holds by its type: every field of a record marked secret, down
/// through records, lists and `?`. `shown` is how the flow writes the value.
pub fn of_type(m: &Model, t: &Ty, shown: &str) -> Vec<Held> {
    fn go(m: &Model, t: &Ty, path: &mut Vec<String>, shown: &str, seen: &mut Vec<RecordId>, out: &mut Vec<Held>) {
        match t {
            Ty::List(x) | Ty::Opt(x) => go(m, x, path, shown, seen, out),
            Ty::Record(r) if !seen.contains(r) => {
                seen.push(*r);
                let rd = &m.records[*r];
                for (f, ft) in &rd.fields {
                    path.push(f.clone());
                    match rd.secrets.get(f) {
                        Some(mark) => add(out, Held { path: path.clone(), shown: joined(shown, path), mark: mark.clone() }),
                        None => go(m, ft, path, shown, seen, out),
                    }
                    path.pop();
                }
                seen.pop();
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    go(m, t, &mut Vec::new(), shown, &mut Vec::new(), &mut out);
    out
}

/// A `json` value holds what it was made from as a whole: it cannot be read into.
fn as_json(t: &Ty, held: Vec<Held>) -> Vec<Held> {
    let json = matches!(t.inner(), Ty::Json) || matches!(t.inner(), Ty::List(x) if matches!(x.inner(), Ty::Json));
    if !json {
        return held;
    }
    let mut out = Vec::new();
    for h in held {
        add(&mut out, Held { path: vec![], ..h });
    }
    out
}

/// The secrets of a value: what its type marks, and what the variables it reads hold.
pub fn of_expr(m: &Model, vars: &BTreeMap<String, Vec<Held>>, e: &TExpr) -> Vec<Held> {
    match e {
        TExpr::Var { name, fields, ty } => read(m, vars, name, fields, ty),
        TExpr::Record { fields, ty } => {
            let marked = match ty.inner() {
                Ty::Record(r) => Some(&m.records[*r].secrets),
                _ => None,
            };
            let mut out = Vec::new();
            for (f, x) in fields {
                if let Some(mark) = marked.and_then(|s| s.get(f)) {
                    add(&mut out, Held { path: vec![f.clone()], shown: x.show(), mark: mark.clone() });
                }
                for h in of_expr(m, vars, x) {
                    let path = std::iter::once(f.clone()).chain(h.path).collect();
                    add(&mut out, Held { path, ..h });
                }
            }
            as_json(ty, out)
        }
        TExpr::List { items, ty } => {
            let mut out = Vec::new();
            for x in items {
                for h in of_expr(m, vars, x) {
                    add(&mut out, h);
                }
            }
            as_json(ty, out)
        }
        // a string is a secret when a value put in it is
        TExpr::Interp(parts) => {
            let mut out = Vec::new();
            for p in parts {
                if let IPart::Hole(x) = p {
                    for h in of_expr(m, vars, x) {
                        add(&mut out, Held { path: vec![], ..h });
                    }
                }
            }
            out
        }
        _ => vec![],
    }
}

/// The secrets of `name.fields…`: what its type marks, a field marked secret on the way to it, and
/// what the variable holds there.
fn read(m: &Model, vars: &BTreeMap<String, Vec<Held>>, name: &str, fields: &[String], ty: &Ty) -> Vec<Held> {
    let shown = joined(name, fields);
    let mut out = of_type(m, ty, &shown);
    // a part of a field marked secret is a secret
    let mut t = m.var_ty(name).cloned();
    for f in fields {
        let Some(r) = t.as_ref().and_then(|t| match t.inner() {
            Ty::Record(r) => Some(*r),
            _ => None,
        }) else {
            break;
        };
        if let Some(mark) = m.records[r].secrets.get(f) {
            add(&mut out, Held { path: vec![], shown: shown.clone(), mark: mark.clone() });
            break;
        }
        t = m.field_ty(r, f).cloned();
    }
    for h in vars.get(name).into_iter().flatten() {
        if h.path.starts_with(fields) {
            add(&mut out, Held { path: h.path[fields.len()..].to_vec(), ..h.clone() });
        } else if fields.starts_with(&h.path) {
            add(&mut out, Held { path: vec![], ..h.clone() });
        }
    }
    out
}

/// What each variable holds: every secret put in it anywhere in the flow, gathered until nothing
/// more changes. An input marked secret holds itself; a task's answer holds what its marks say; a
/// rule's answer holds none.
pub fn variables(m: &Model) -> BTreeMap<String, Vec<Held>> {
    let mut vars: BTreeMap<String, Vec<Held>> = BTreeMap::new();
    for (n, mark) in &m.input_secrets {
        vars.entry(n.clone()).or_default().push(Held { path: vec![], shown: n.clone(), mark: mark.clone() });
    }
    let stmts = m.all_stmts();
    for _ in 0..=m.vars.len() + 1 {
        let mut next = vars.clone();
        let mut put = |name: &str, held: Vec<Held>| {
            let ty = m.var_ty(name).cloned().unwrap_or(Ty::Json);
            let e = next.entry(name.to_string()).or_default();
            for h in as_json(&ty, held) {
                add(e, h);
            }
        };
        for s in &stmts {
            match &s.kind {
                TK::Call { target: Some(target), callee: Callee::Task(t), .. } => {
                    let x = match target {
                        Target::Let(x) => x.clone(),
                        Target::Case(c) => m.cases[*c].name.clone(),
                    };
                    let held = m.tasks[*t].result_secrets.iter().map(|(p, mark)| Held { path: p.split('.').filter(|s| !s.is_empty()).map(String::from).collect(), shown: joined(&x, &p.split('.').filter(|s| !s.is_empty()).map(String::from).collect::<Vec<_>>()), mark: mark.clone() }).collect();
                    put(&x, held);
                }
                TK::Assign { name, expr } => put(name, of_expr(m, &vars, expr)),
                TK::For { var, list, result, .. } => {
                    put(var, of_expr(m, &vars, list));
                    if let Some((r, y)) = result {
                        put(r, of_expr(m, &vars, y));
                    }
                }
                TK::Match { expr, arms } => {
                    for a in arms {
                        if let Some(x) = &a.some {
                            put(x, of_expr(m, &vars, expr));
                        }
                    }
                }
                _ => {}
            }
        }
        if next == vars {
            break;
        }
        vars = next;
    }
    vars
}

/// The secrets a call gives as `param`: what the value holds, and the parameter's own marks.
fn given(m: &Model, vars: &BTreeMap<String, Vec<Held>>, callee: &Callee, param: &str, e: &TExpr) -> Vec<Held> {
    let mut out = of_expr(m, vars, e);
    if let Callee::Task(t) = callee {
        for (p, mark) in m.tasks[*t].param_secrets.get(param).into_iter().flatten() {
            let path: Vec<String> = p.split('.').filter(|s| !s.is_empty()).map(String::from).collect();
            add(&mut out, Held { shown: joined(&e.show(), &path), path, mark: mark.clone() });
        }
    }
    out
}

fn callee_name(m: &Model, c: &Callee) -> String {
    match c {
        Callee::Task(t) => m.tasks[*t].name.clone(),
        Callee::Rule(r) => m.rules[*r].name.clone(),
    }
}

// ── W904 ────────────────────────────────────────────────────────────────────

fn platforms_keep() -> Text {
    tr!(
        "Temporal、Step Functions、Lambda durable functions、Argo Workflows は、ワークフローとすべての呼び出しの入力と出力を履歴に残し、実行を読める人はだれでもそれを読めます。",
        "Temporal, Step Functions, Lambda durable functions and Argo Workflows keep the inputs and outputs of the workflow and of every call in the history, which whoever may read the executions can read."
    )
}

fn or_encrypted() -> Text {
    tr!(
        "履歴を自分の持つ鍵で暗号化しているなら（Temporal はペイロードのコーデック、Step Functions と Lambda durable functions はカスタマー管理の KMS キー）、`workflow` の下に `history encrypted` と書いてください。",
        "If the history is encrypted with a key you hold (Temporal: a payload codec; Step Functions and Lambda durable functions: a customer managed KMS key), write `history encrypted` under `workflow`."
    )
}

/// The secrets in `held` by how the flow writes them, each with every mark it has, in the order met:
/// one diagnostic says one value, and the notes say each place that marks it.
fn by_value(held: Vec<Held>) -> Vec<(Held, Vec<SecretMark>)> {
    let mut out: Vec<(Held, Vec<SecretMark>)> = Vec::new();
    for h in held {
        match out.iter_mut().find(|(x, _)| x.shown == h.shown && x.path == h.path) {
            Some((_, marks)) => {
                if !marks.contains(&h.mark) {
                    marks.push(h.mark);
                }
            }
            None => {
                let mark = h.mark.clone();
                out.push((h, vec![mark]));
            }
        }
    }
    out
}

fn keep(line: usize, said: Text, marks: &[SecretMark], fix: Text) -> Diag {
    let Text { en, ja } = or_encrypted();
    let Text { en: fen, ja: fja } = fix;
    let mut d = Diag::warning("W904", line, 1, said);
    for mark in marks {
        d = d.note(mark.note());
    }
    d.note(platforms_keep()).note(tr!("{fja}{ja}", "{fen} {en}"))
}

fn pass_a_reference() -> Text {
    tr!("値の代わりに参照（ID やシークレットの名前）を渡し、タスクの中で値を取ってきてください。", "Pass a reference instead (an ID, the name of a secret) and fetch the value inside the task.")
}

/// W904: each secret the history of a run keeps — an input or an output of the workflow, the
/// answer of a task it calls, a value given to a call, the reason of a `fail`.
fn kept(m: &Model, vars: &BTreeMap<String, Vec<Held>>) -> Vec<Diag> {
    let mut out = Vec::new();
    let whole_or_part = |held: &Held, whole: Text, part: Text| if held.path.is_empty() { whole } else { part };
    for (n, t) in &m.inputs {
        let line = m.input_lines.get(n).copied().unwrap_or(1);
        let mut held = Vec::new();
        if let Some(mark) = m.input_secrets.get(n) {
            add(&mut held, Held { path: vec![], shown: n.clone(), mark: mark.clone() });
        }
        for h in of_type(m, t, n) {
            add(&mut held, h);
        }
        for (h, marks) in by_value(held) {
            let shown = &h.shown;
            let said = whole_or_part(&h, tr!("入力 `{n}` は秘密の値で、ワークフローの履歴に残ります", "the input `{n}` is a secret, kept in the history of the workflow"), tr!("入力 `{n}` が持つ秘密の値 `{shown}` が、ワークフローの履歴に残ります", "the input `{n}` holds the secret `{shown}`, kept in the history of the workflow"));
            out.push(keep(line, said, &marks, tr!("ワークフローには値の代わりに参照（ID やシークレットの名前）を渡し、それを使うタスクの中で値を取ってきてください。", "Start the workflow with a reference instead (an ID, the name of a secret), and fetch the value inside the task that uses it.")));
        }
    }
    for (o, t) in &m.outputs {
        let line = m.output_lines.get(o).copied().unwrap_or(1);
        let mut held = Vec::new();
        if let Some(mark) = m.output_secrets.get(o) {
            add(&mut held, Held { path: vec![], shown: o.clone(), mark: mark.clone() });
        }
        for h in of_type(m, t, o) {
            add(&mut held, h);
        }
        for (h, marks) in by_value(held) {
            let shown = &h.shown;
            let said = whole_or_part(&h, tr!("出力 `{o}` は秘密の値で、ワークフローの履歴に残ります", "the output `{o}` is a secret, kept in the history of the workflow"), tr!("出力 `{o}` が持つ秘密の値 `{shown}` が、ワークフローの履歴に残ります", "the output `{o}` holds the secret `{shown}`, kept in the history of the workflow"));
            out.push(keep(line, said, &marks, tr!("ワークフローには値の代わりに参照（ID やシークレットの名前）を返させてください。", "Have the workflow answer a reference instead (an ID, the name of a secret).")));
        }
    }
    // the answers of the tasks the flow calls: kept whether the flow reads them or not
    let mut called: Vec<usize> = Vec::new();
    for s in m.all_stmts() {
        if let TK::Call { callee: Callee::Task(t), .. } = &s.kind {
            if !called.contains(t) {
                called.push(*t);
            }
        }
    }
    called.sort_unstable();
    for t in called {
        let task = &m.tasks[t];
        let Some(rt) = &task.result else { continue };
        let name = &task.name;
        let mut held = Vec::new();
        for (p, mark) in &task.result_secrets {
            let path: Vec<String> = p.split('.').filter(|s| !s.is_empty()).map(String::from).collect();
            add(&mut held, Held { shown: path.join("."), path, mark: mark.clone() });
        }
        for h in of_type(m, rt, "") {
            add(&mut held, Held { shown: h.path.join("."), ..h });
        }
        for (h, marks) in by_value(held) {
            let shown = &h.shown;
            let said = whole_or_part(&h, tr!("`{name}` の結果は秘密の値で、ワークフローの履歴に残ります", "the answer of `{name}` is a secret, kept in the history of the workflow"), tr!("`{name}` の結果が持つ秘密の値 `{shown}` が、ワークフローの履歴に残ります", "the answer of `{name}` holds the secret `{shown}`, kept in the history of the workflow"));
            out.push(keep(task.line, said, &marks, tr!("タスクには値の代わりに参照（ID やシークレットの名前）を返させ、値はそれを使うタスクの中で取ってきてください。", "Have the task answer a reference instead (an ID, the name of a secret), and fetch the value inside the task that uses it.")));
        }
    }
    for s in m.all_stmts() {
        match &s.kind {
            TK::Call { callee, args, .. } => {
                let who = callee_name(m, callee);
                for (p, e) in args {
                    for (h, marks) in by_value(given(m, vars, callee, p, e)) {
                        let shown = &h.shown;
                        out.push(keep(s.line, tr!("秘密の値 `{shown}` が、`{who}` の引数 `{p}` として、ワークフローの履歴に残ります", "the secret `{shown}` is kept in the history of the workflow, as the argument `{p}` of `{who}`"), &marks, pass_a_reference()));
                    }
                }
            }
            TK::Succeed { fields } => {
                for (o, e) in fields {
                    // what the output's own mark or type says is said at its declaration
                    let declared: Vec<Held> = m.outputs.iter().filter(|(n, _)| n == o).flat_map(|(_, t)| of_type(m, t, o)).collect();
                    let held: Vec<Held> = of_expr(m, vars, e).into_iter().filter(|h| !m.output_secrets.contains_key(o) && !declared.iter().any(|d| d.path == h.path && d.mark == h.mark)).collect();
                    for (h, marks) in by_value(held) {
                        let shown = &h.shown;
                        out.push(keep(s.line, tr!("秘密の値 `{shown}` が、出力 `{o}` として、ワークフローの履歴に残ります", "the secret `{shown}` is kept in the history of the workflow, as the output `{o}`"), &marks, tr!("出力には値の代わりに参照（ID やシークレットの名前）を書いてください。", "Write a reference into the output instead (an ID, the name of a secret).")));
                    }
                }
            }
            TK::Fail { error, cause: Some(e), .. } => {
                for (h, marks) in by_value(of_expr(m, vars, e)) {
                    let shown = &h.shown;
                    out.push(keep(s.line, tr!("秘密の値 `{shown}` が、`fail {error}` の理由として、ワークフローの履歴に残ります", "the secret `{shown}` is kept in the history of the workflow, in the reason of `fail {error}`"), &marks, tr!("理由には秘密の値を埋め込まず、参照（ID など）を書いてください。", "Leave the secret out of the reason, and write a reference (an ID, say) instead.")));
                }
            }
            _ => {}
        }
    }
    out
}

// ── E906 ────────────────────────────────────────────────────────────────────

/// Who a task sends what it is given to, when that is outside the project: a model provider, Jev,
/// the host an `http` task names by its URL alone, an AWS service. None for a call that stays in the
/// project or on this machine (an operation of an API's description, a `.proto`'s method, a rule, a
/// child flow, a book, the user's own code, an agent or an `http` call to this machine).
pub fn outside(t: &TaskDef) -> Option<String> {
    match &t.binding {
        Some(Binding::Agent { provider, url: None, .. }) => Some(match provider {
            Provider::OpenAi => "OpenAI".to_string(),
            Provider::Claude => "Anthropic".to_string(),
        }),
        Some(Binding::Agent { url: Some(u), .. }) => host_of(u).filter(|(_, local)| !local).map(|(h, _)| h),
        Some(Binding::Jev(_)) => Some("TypeSafe (Jev)".to_string()),
        Some(Binding::Http { url, .. }) if t.api_file.is_none() => host_of(url).filter(|(_, local)| !local).map(|(h, _)| h),
        Some(Binding::Aws { service, .. }) => Some(format!("AWS ({service})")),
        _ => None,
    }
}

/// E906: each secret a call gives a task that sends it outside the project, unless the task says
/// it discloses the parameter.
fn sent_outside(m: &Model, vars: &BTreeMap<String, Vec<Held>>) -> Vec<Diag> {
    let mut out = Vec::new();
    for s in m.all_stmts() {
        let TK::Call { callee: callee @ Callee::Task(t), args, .. } = &s.kind else { continue };
        let task = &m.tasks[*t];
        let Some(dest) = outside(task) else { continue };
        let name = &task.name;
        for (p, e) in args {
            if task.discloses.iter().any(|(d, _)| d == p) {
                continue;
            }
            for (h, marks) in by_value(given(m, vars, callee, p, e)) {
                let shown = &h.shown;
                let mut d = Diag::error("E906", s.line, 1, tr!("タスク `{name}` が、秘密の値 `{shown}` をプロジェクトの外の {dest} に送ります", "the task `{name}` sends the secret `{shown}` to {dest}, outside the project"));
                for mark in &marks {
                    d = d.note(mark.note());
                }
                out.push(d.note(tr!("参照か、相手に要るものだけを送ってください。そこへ送ることを意図しているなら、タスクの下に `discloses {p} \"<理由>\"` と書いてください。", "Send a reference or only what the other side needs. If sending it there is intended, write `discloses {p} \"<why>\"` under the task.")));
            }
        }
    }
    out
}

// ── The port ────────────────────────────────────────────────────────────────

/// Every call of the flow that gives a secret to a file of the project (ritsu's E905 holds them to
/// the map): an operation of an OpenAPI document, a method of a `.proto`, a rule called at its
/// Connect service, a child `.flow`, an operation of a book, a date of a dates file. The paths are
/// as dandori reaches them.
pub fn sends(m: &Model) -> Vec<Sent> {
    let vars = variables(m);
    let dir = std::path::Path::new(&m.source_reached).parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let mut out = Vec::new();
    for s in m.all_stmts() {
        let TK::Call { callee, args, .. } = &s.kind else { continue };
        let (to, disclosed) = match callee {
            Callee::Task(t) => {
                let task = &m.tasks[*t];
                let to = if let Some(f) = &task.api_file {
                    Some(f.clone())
                } else if let Some(c) = &task.flow {
                    Some(dir.join(&c.path).to_string_lossy().to_string())
                } else {
                    task.book().map(|b| m.books[b.book].path.to_string_lossy().to_string())
                };
                (to, task.discloses.clone())
            }
            Callee::Rule(r) => {
                let ru = &m.rules[*r];
                let to = (ru.connect.is_some() || ru.date().is_some()).then(|| ru.info.path.to_string_lossy().to_string());
                (to, vec![])
            }
        };
        let Some(to) = to else { continue };
        let mut secrets = Vec::new();
        for (p, e) in args {
            for h in given(m, &vars, callee, p, e) {
                secrets.push((p.clone(), h));
            }
        }
        if !secrets.is_empty() {
            out.push(Sent { line: s.line, callee: callee_name(m, callee), to, secrets, disclosed });
        }
    }
    out
}
