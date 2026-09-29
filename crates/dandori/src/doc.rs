//! `doc`: a workflow drawn for the person who reviews it. The flow becomes a picture of its
//! statements (a call, a match, a wait, a loop, an end), and beside it are the facts the checker
//! knows that the text does not show: what each call calls and where its errors go, what each
//! case can be after a call, and every way the workflow can end. As Markdown, the picture is a
//! Mermaid flowchart, which GitHub draws in a pull request; as HTML, it is drawn here (`draw`),
//! and a scenario, or the run a diagnostic gives, lights up the way it goes.

use crate::diag::{At, Diag, Lang, Severity};
use crate::flow::{CaseFact, Ending, Facts, Tri};
use crate::interp::Visit;
use crate::model::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

/// What `doc` draws from: a model whose names and types resolve, and what the walk of the runs
/// knows at each statement.
pub struct Input<'a> {
    pub m: &'a Model,
    pub src: &'a str,
    /// the `.flow`'s path, as the page names it
    pub file: &'a str,
    pub facts: &'a Facts,
    pub diags: &'a [Diag],
    pub lang: Lang,
}

pub fn tr(lang: Lang, en: &str, ja: &str) -> String {
    match lang {
        Lang::En => en.to_string(),
        Lang::Ja => ja.to_string(),
    }
}

/// At most `n` characters, with `…` for the rest.
fn short(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut o: String = s.chars().take(n.saturating_sub(1)).collect();
        o.push('…');
        o
    }
}

// ---------------------------------------------------------------------------
// The picture: nodes, and the pieces of each block

/// What a node is, which decides its shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Start,
    /// a call of a task
    Call,
    Rule,
    /// a task whose value comes from outside: an event, or a callback's answer
    Receive,
    Match,
    Wait,
    /// `let x = <value>`
    Let,
    Succeed,
    Fail,
    Break,
    /// where `on failure` and `on cancel` begin
    Entry,
    /// where a block that runs to its end ends
    End,
}

pub struct Node {
    /// `s<site>` for a statement; `start`, `fin`, `onf`, `onfEnd`, `onc`, `oncEnd` for the others
    pub id: String,
    pub kind: Kind,
    /// the first is what the statement is; the others say more, drawn fainter
    pub lines: Vec<String>,
    /// the line in the `.flow`; 0 for a node of no line
    pub line: usize,
    pub site: Option<usize>,
}

/// A statement as drawn, with the blocks it holds.
pub enum Piece {
    /// one node, and on to what follows
    Node(usize),
    /// a call whose errors have handlers: the answer goes on, and so does each handler's body
    Call { node: usize, handlers: Vec<Branch> },
    Match { node: usize, arms: Vec<Branch> },
    Loop { site: usize, title: String, footer: Option<String>, body: Vec<Piece>, parallel: bool, exit: String },
    /// `succeed` or `fail`: the run ends
    Stop(usize),
    /// leaves the loop around it
    Break(usize),
}

/// An arm of a match or a handler of a call, and its block.
pub struct Branch {
    pub label: String,
    pub cond: Cond,
    pub dashed: bool,
    pub body: Vec<Piece>,
}

/// What must have happened for a run to go along an edge, besides passing both of its ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cond {
    Arm(usize, usize),
    Handler(usize, usize),
    /// the call's answer came back
    Ok(usize),
    /// the loop went round again
    Again(usize),
    /// the loop ran out of rounds or of items
    Done(usize),
}

/// The flow, `on failure` or `on cancel`: where it begins, its pieces, and where it ends when it
/// runs to its end.
pub struct Panel {
    pub key: &'static str,
    pub entry: usize,
    pub body: Vec<Piece>,
    pub end: usize,
}

pub struct Graph {
    pub nodes: Vec<Node>,
    pub panels: Vec<Panel>,
    /// the title of each loop's frame, by the loop's site
    pub loops: BTreeMap<usize, String>,
}

impl Graph {
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

pub fn graph(i: &Input) -> Graph {
    let m = i.m;
    let mut b = Builder { i, nodes: vec![], loops: BTreeMap::new() };
    let mut panels = Vec::new();
    let start = b.node("start".into(), Kind::Start, vec![format!("{} v{}", m.name, m.version)], 0, None);
    let body = b.block(&m.flow);
    let end = b.node("fin".into(), Kind::End, vec![tr(i.lang, "end: succeeds", "終わり（成功）")], 0, None);
    panels.push(Panel { key: "flow", entry: start, body, end });
    if let Some(f) = &m.on_failure {
        // the line of `on failure`: the last one written so before the block's first statement
        let first = f.first().map(|s| s.line).unwrap_or(0);
        let line = i.src.lines().enumerate().take(first.saturating_sub(1)).filter(|(_, l)| l.trim_end() == "on failure").map(|(k, _)| k + 1).last().unwrap_or(0);
        let entry = b.node("onf".into(), Kind::Entry, vec!["on failure".into()], line, None);
        let body = b.block(f);
        let end = b.node("onfEnd".into(), Kind::End, vec![tr(i.lang, "fails with the same error", "同じエラーで失敗する")], 0, None);
        panels.push(Panel { key: "on-failure", entry, body, end });
    }
    if let Some(f) = &m.on_cancel {
        let entry = b.node("onc".into(), Kind::Entry, vec!["on cancel".into()], m.on_cancel_line, None);
        let body = b.block(f);
        let end = b.node("oncEnd".into(), Kind::End, vec![tr(i.lang, "ends cancelled", "キャンセルで終わる")], 0, None);
        panels.push(Panel { key: "on-cancel", entry, body, end });
    }
    Graph { nodes: b.nodes, panels, loops: b.loops }
}

struct Builder<'a> {
    i: &'a Input<'a>,
    nodes: Vec<Node>,
    loops: BTreeMap<usize, String>,
}

impl<'a> Builder<'a> {
    fn node(&mut self, id: String, kind: Kind, lines: Vec<String>, line: usize, site: Option<usize>) -> usize {
        self.nodes.push(Node { id, kind, lines, line, site });
        self.nodes.len() - 1
    }

    fn stmt_node(&mut self, s: &TStmt, kind: Kind, lines: Vec<String>) -> usize {
        self.node(format!("s{}", s.site), kind, lines, s.line, Some(s.site))
    }

    fn block(&mut self, ss: &[TStmt]) -> Vec<Piece> {
        let lang = self.i.lang;
        let mut out = Vec::new();
        for s in ss {
            match &s.kind {
                TK::Pass => {}
                TK::Call { target, callee, args, handlers } => {
                    let (kind, lines) = call_lines(self.i, target.as_ref(), callee, args);
                    let n = self.stmt_node(s, kind, lines);
                    if handlers.is_empty() {
                        out.push(Piece::Node(n));
                    } else {
                        let hs = handlers
                            .iter()
                            .enumerate()
                            .map(|(j, h)| Branch { label: format!("on {}", herr_names(&h.errors).join(", ")), cond: Cond::Handler(s.site, j), dashed: true, body: self.block(&h.body) })
                            .collect();
                        out.push(Piece::Call { node: n, handlers: hs });
                    }
                }
                TK::Match { expr, arms } => {
                    let n = self.stmt_node(s, Kind::Match, vec![format!("match {}", expr.show())]);
                    let arms = arms.iter().enumerate().map(|(k, a)| Branch { label: arm_label(a), cond: Cond::Arm(s.site, k), dashed: false, body: self.block(&a.body) }).collect();
                    out.push(Piece::Match { node: n, arms });
                }
                TK::Wait { seconds } => {
                    let n = self.stmt_node(s, Kind::Wait, vec![format!("wait {}", crate::flow::show_dur(*seconds))]);
                    out.push(Piece::Node(n));
                }
                TK::WaitUntil { at } => {
                    let n = self.stmt_node(s, Kind::Wait, vec![format!("wait until {}", expr_text(at))]);
                    out.push(Piece::Node(n));
                }
                TK::Assign { name, expr } => {
                    let n = self.stmt_node(s, Kind::Let, vec![short(&format!("let {name} = {}", expr_text(expr)), 56)]);
                    out.push(Piece::Node(n));
                }
                TK::Repeat { times, body } => {
                    let (body, _) = self.loop_body(s, body, None);
                    let exit = match lang {
                        Lang::En => format!("after {times} round{}", if *times == 1 { "" } else { "s" }),
                        Lang::Ja => format!("{times} 回終えたら"),
                    };
                    let title = format!("repeat at most {times} times");
                    self.loops.insert(s.site, title.clone());
                    out.push(Piece::Loop { site: s.site, title, footer: None, body, parallel: false, exit });
                }
                TK::For { var, list, max, parallel, body, result, .. } => {
                    let footer = result.as_ref().map(|(_, y)| format!("yield {}", expr_text(y)));
                    let (body, footer) = self.loop_body(s, body, footer);
                    let mut title = String::new();
                    if let Some((r, _)) = result {
                        title.push_str(&format!("let {r} = "));
                    }
                    title.push_str(&format!("for {var} in {} at most {max}", expr_text(list)));
                    match parallel {
                        Some(0) => title.push_str(" in parallel"),
                        Some(k) => title.push_str(&format!(" in parallel, {k} at a time")),
                        None => {}
                    }
                    self.loops.insert(s.site, title.clone());
                    let exit = if parallel.is_some() { tr(lang, "every round done", "すべてのイテレーションが終わったら") } else { tr(lang, "after the last item", "最後の項目のあと") };
                    out.push(Piece::Loop { site: s.site, title, footer, body, parallel: parallel.is_some(), exit });
                }
                TK::Break => {
                    let n = self.stmt_node(s, Kind::Break, vec!["break".into()]);
                    out.push(Piece::Break(n));
                }
                TK::Succeed { fields } => {
                    let text = if fields.is_empty() { "succeed".to_string() } else { format!("succeed {}", fields.iter().map(|(f, e)| format!("{f} = {}", expr_text(e))).collect::<Vec<_>>().join(", ")) };
                    let n = self.stmt_node(s, Kind::Succeed, vec![short(&text, 56)]);
                    out.push(Piece::Stop(n));
                }
                TK::Fail { error, cause, leaving } => {
                    let mut lines = vec![format!("fail {error}")];
                    if let Some(c) = cause {
                        lines.push(short(&expr_text(c), 48));
                    }
                    if !leaving.is_empty() {
                        lines.push(format!("leaving {}", leaving.iter().map(|c| self.i.m.cases[*c].name.clone()).collect::<Vec<_>>().join(", ")));
                    }
                    let n = self.stmt_node(s, Kind::Fail, lines);
                    out.push(Piece::Stop(n));
                }
            }
        }
        out
    }

    /// A loop's pieces, and what to write at its foot. A loop whose body draws nothing still gets
    /// a node to go round: its `yield`, which then leaves the foot, or its `pass`.
    fn loop_body(&mut self, s: &TStmt, body: &[TStmt], footer: Option<String>) -> (Vec<Piece>, Option<String>) {
        let pieces = self.block(body);
        if !pieces.is_empty() {
            return (pieces, footer);
        }
        let line = body.iter().find(|x| matches!(x.kind, TK::Pass)).map(|x| x.line).unwrap_or(0);
        let text = footer.clone().unwrap_or_else(|| "pass".into());
        let n = self.node(format!("s{}p", s.site), Kind::Let, vec![text], line, None);
        (vec![Piece::Node(n)], None)
    }
}

fn herr_names(errors: &[HErr]) -> Vec<String> {
    errors
        .iter()
        .map(|e| match e {
            HErr::Declared(n) => n.clone(),
            HErr::Timeout => "timeout".into(),
            HErr::Failure => "failure".into(),
        })
        .collect()
}

fn arm_label(a: &TArm) -> String {
    let mut vals: Vec<String> = a.values.clone();
    if a.none {
        vals.push("none".into());
    }
    if let Some(v) = &a.some {
        vals.push(format!("some {v}"));
    }
    vals.join(", ")
}

/// An expression as the `.flow` writes it, strings in quotes.
fn expr_text(e: &TExpr) -> String {
    match e {
        TExpr::Str(s) => format!("\"{s}\""),
        TExpr::Interp(_) => format!("\"{}\"", e.show()),
        other => other.show(),
    }
}

/// The lines of the `.flow`, by number from 1.
fn src_line(src: &str, line: usize) -> &str {
    src.lines().nth(line.saturating_sub(1)).unwrap_or("")
}

/// A task's declaration: its first line, the clauses under it, and the comment just above.
fn decl_lines(src: &str, line: usize) -> Vec<(usize, String)> {
    let lines: Vec<&str> = src.lines().collect();
    if line == 0 || line > lines.len() {
        return vec![];
    }
    let mut from = line - 1;
    while from > 0 && lines[from - 1].trim_start().starts_with('#') {
        from -= 1;
    }
    let mut to = line;
    while to < lines.len() && lines[to].starts_with([' ', '\t']) && !lines[to].trim().is_empty() {
        to += 1;
    }
    (from..to).map(|k| (k + 1, lines[k].to_string())).collect()
}

/// The quoted text on a line: the file of `use rule … from "<file>"`.
fn quoted(line: &str) -> Option<&str> {
    let a = line.find('"')?;
    let b = line[a + 1..].find('"')?;
    Some(&line[a + 1..a + 1 + b])
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// A Lambda function as it is named for people: the name out of an ARN.
fn function_name(f: &str) -> &str {
    match f.find(":function:") {
        Some(k) => &f[k + ":function:".len()..],
        None => f,
    }
}

/// How a task is called, in a few words: the clause of its declaration, or what the model says;
/// and whether that is `.flow` (not words, as for a task you write).
pub fn how_called(i: &Input, t: &TaskDef) -> (String, bool) {
    let lang = i.lang;
    if t.event {
        return ("event".into(), true);
    }
    // the clause as written, for the calls whose form the model spells out from the API description
    let clause = decl_lines(i.src, t.line).into_iter().map(|(_, l)| l.trim().to_string()).find(|l| ["http ", "connect ", "aws "].iter().any(|k| l.starts_with(k)));
    let mut how = match &t.binding {
        Some(Binding::Http { .. }) => clause.map(|c| c.trim_start_matches("http ").replace('"', "")),
        Some(Binding::Aws { service, action }) => Some(format!("aws {service}:{action}")),
        Some(Binding::Lambda(f)) => Some(format!("lambda {}", function_name(f))),
        Some(Binding::Agent { provider, model, url, .. }) => {
            // with `url`, the agent is on a server of Open Responses, not OpenAI's
            let mut s = match url {
                Some(_) => "agent".to_string(),
                None => format!("agent {}", provider.name()),
            };
            if !model.is_empty() {
                s.push_str(&format!(" · {model}"));
            }
            if let Some(u) = url {
                s.push_str(&format!(" · {u}"));
            }
            Some(s)
        }
        Some(Binding::Jev(j)) => {
            let mut s = "jev".to_string();
            if !j.model.is_empty() {
                s.push_str(&format!(" · {}", j.model));
            }
            if let Some((f, e)) = &j.floor {
                s.push_str(&format!(" · confidence {f} else {e}"));
            }
            Some(s)
        }
        None => None,
    };
    if t.connect.is_some() {
        how = decl_lines(i.src, t.line).into_iter().map(|(_, l)| l.trim().replace('"', "")).find(|l| l.starts_with("connect "));
    }
    let how = how
        .or_else(|| t.flow.as_ref().map(|f| format!("flow {}", file_name(&f.path))))
        .or_else(|| t.workflow.as_ref().map(|w| format!("workflow {w}")))
        .or_else(|| t.state_machine.as_ref().map(|w| format!("state machine {}", w.rsplit(':').next().unwrap_or(w))))
        .or_else(|| t.durable_function.as_ref().map(|w| format!("durable function {}", function_name(w))))
        .or_else(|| t.argo_template.as_ref().map(|w| format!("workflow template {w}")))
        .or_else(|| t.image.as_ref().map(|w| format!("image {w}")));
    match (how, t.callback) {
        (Some(h), true) => (format!("{h} · callback"), true),
        (Some(h), false) => (h, true),
        (None, true) => (tr(lang, "a task you write, answered by a callback", "自分で書くタスク（応答はコールバック）"), false),
        (None, false) => (tr(lang, "a task you write", "自分で書くタスク"), false),
    }
}

/// A rule as it is named for people: its file.
fn rule_file(i: &Input, r: &RuleUse) -> String {
    quoted(src_line(i.src, r.line)).map(|p| file_name(p).to_string()).unwrap_or_else(|| r.name.clone())
}

/// A rule's file as `use rule` writes it.
fn rule_path(i: &Input, r: &RuleUse) -> String {
    quoted(src_line(i.src, r.line)).map(str::to_string).unwrap_or_else(|| r.name.clone())
}

/// Each rule the workflow uses, with what `rulec doc` renders for it in the page's language:
/// Markdown, or the page on which whoever approves the rule tries a case. rulec draws its rules;
/// the page shows them as it drew them.
fn rule_docs<'a>(i: &Input<'a>, html: bool) -> Vec<(&'a RuleUse, Result<String, String>)> {
    i.m.rules.iter().map(|r| (r, crate::sources::rulec_doc(&r.info.path, html, i.lang))).collect()
}

fn retry_text(r: &Retry) -> String {
    let mut s = format!("retry {} times every {}", r.times, crate::flow::show_dur(r.every));
    if r.backoff != 2.0 {
        s.push_str(&format!(" backoff {}", r.backoff));
    }
    if !r.on.is_empty() {
        s.push_str(&format!(" on {}", r.on.join(", ")));
    }
    s
}

/// A call's node: its shape, and its lines — the call, how it is called, and what else its
/// declaration says that the flow does not show.
fn call_lines(i: &Input, target: Option<&Target>, callee: &Callee, args: &[(String, TExpr)]) -> (Kind, Vec<String>) {
    let m = i.m;
    let name = match callee {
        Callee::Task(t) => m.tasks[*t].name.clone(),
        Callee::Rule(r) => m.rules[*r].name.clone(),
    };
    let parens = if args.is_empty() { "()" } else { "(…)" };
    let first = match target {
        Some(Target::Let(v)) => format!("{v} = {name}{parens}"),
        Some(Target::Case(c)) => format!("{} ← {name}{parens}", m.cases[*c].name),
        None => format!("{name}{parens}"),
    };
    match callee {
        Callee::Rule(r) => {
            let ru = &m.rules[*r];
            let mut how = format!("rule {}", rule_file(i, ru));
            if ru.local {
                how.push_str(" · local");
            }
            (Kind::Rule, vec![first, how])
        }
        Callee::Task(t) => {
            let t = &m.tasks[*t];
            let mut lines = vec![first, how_called(i, t).0];
            let mut more = Vec::new();
            match &t.machine {
                Some(TaskMachine::Starts { .. }) => more.push("starts".to_string()),
                Some(TaskMachine::Sends { event, .. }) => more.push(format!("sends {event}")),
                Some(TaskMachine::Observes) => more.push("observes".into()),
                None => {}
            }
            if let Some(r) = &t.retry {
                more.push(retry_text(r));
            }
            if let Some(s) = t.timeout {
                more.push(format!("timeout {}", crate::flow::show_dur(s)));
            }
            if !more.is_empty() {
                lines.push(more.join(" · "));
            }
            (if t.event || t.callback { Kind::Receive } else { Kind::Call }, lines)
        }
    }
}

// ---------------------------------------------------------------------------
// Mermaid

/// Text in a Mermaid label: what Mermaid would read as syntax, as its entity codes.
fn mm(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '"' => o.push_str("#quot;"),
            '#' => o.push_str("#35;"),
            '<' => o.push_str("#lt;"),
            '>' => o.push_str("#gt;"),
            '|' => o.push_str("#124;"),
            '&' => o.push_str("#amp;"),
            c => o.push(c),
        }
    }
    o
}

#[derive(Clone)]
struct Loose {
    from: String,
    label: Option<String>,
    dashed: bool,
}

impl Loose {
    fn at(id: &str) -> Loose {
        Loose { from: id.to_string(), label: None, dashed: false }
    }
}

struct Mm<'g> {
    g: &'g Graph,
    lang: Lang,
    decl: String,
    edges: Vec<String>,
    breaks: Vec<Vec<Loose>>,
    ok: Vec<String>,
    bad: Vec<String>,
}

impl<'g> Mm<'g> {
    fn declare(&mut self, n: usize, depth: usize) {
        let node = &self.g.nodes[n];
        let text = node.lines.iter().map(|l| mm(l)).collect::<Vec<_>>().join("<br>");
        let (open, close) = match node.kind {
            Kind::Start | Kind::Succeed | Kind::Fail | Kind::End | Kind::Entry | Kind::Break => ("([\"", "\"])"),
            Kind::Call | Kind::Let => ("[\"", "\"]"),
            Kind::Rule => ("[[\"", "\"]]"),
            Kind::Receive => ("[/\"", "\"/]"),
            Kind::Match => ("{{\"", "\"}}"),
            Kind::Wait => ("(\"", "\")"),
        };
        let _ = writeln!(self.decl, "{}{}{open}{text}{close}", "    ".repeat(depth + 1), node.id);
        match node.kind {
            Kind::Succeed => self.ok.push(node.id.clone()),
            Kind::Fail => self.bad.push(node.id.clone()),
            _ => {}
        }
    }

    fn entry_of(&self, p: &Piece) -> String {
        match p {
            Piece::Node(n) | Piece::Call { node: n, .. } | Piece::Match { node: n, .. } | Piece::Stop(n) | Piece::Break(n) => self.g.nodes[*n].id.clone(),
            Piece::Loop { body, .. } => self.entry_of(&body[0]),
        }
    }

    fn connect(&mut self, loose: &[Loose], to: &str) {
        for l in loose {
            let arrow = if l.dashed { "-.->" } else { "-->" };
            let label = l.label.as_ref().map(|t| format!("|\"{}\"|", mm(t))).unwrap_or_default();
            self.edges.push(format!("    {} {arrow}{label} {to}", l.from));
        }
    }

    fn block(&mut self, pieces: &[Piece], mut loose: Vec<Loose>, depth: usize) -> Vec<Loose> {
        for p in pieces {
            let to = self.entry_of(p);
            self.connect(&loose, &to);
            loose = self.piece(p, depth);
        }
        loose
    }

    fn branches(&mut self, node: usize, branches: &[Branch], depth: usize) -> Vec<Loose> {
        let id = self.g.nodes[node].id.clone();
        let mut out = Vec::new();
        for b in branches {
            let l = Loose { from: id.clone(), label: Some(b.label.clone()), dashed: b.dashed };
            if b.body.is_empty() {
                out.push(l);
            } else {
                out.extend(self.block(&b.body, vec![l], depth));
            }
        }
        out
    }

    fn piece(&mut self, p: &Piece, depth: usize) -> Vec<Loose> {
        match p {
            Piece::Node(n) => {
                self.declare(*n, depth);
                vec![Loose::at(&self.g.nodes[*n].id)]
            }
            Piece::Call { node, handlers } => {
                self.declare(*node, depth);
                let mut out = vec![Loose::at(&self.g.nodes[*node].id)];
                out.extend(self.branches(*node, handlers, depth));
                out
            }
            Piece::Match { node, arms } => {
                self.declare(*node, depth);
                self.branches(*node, arms, depth)
            }
            Piece::Loop { site, title, footer, body, parallel, exit } => {
                let sg = format!("L{site}");
                let mut t = title.clone();
                if let Some(f) = footer {
                    t.push_str(&format!(" · {f}"));
                }
                let _ = writeln!(self.decl, "{}subgraph {sg} [\"{}\"]", "    ".repeat(depth + 1), mm(&t));
                self.breaks.push(vec![]);
                let first = self.entry_of(&body[0]);
                let ends = self.block(body, vec![], depth + 1);
                let _ = writeln!(self.decl, "{}end", "    ".repeat(depth + 1));
                let breaks = self.breaks.pop().unwrap_or_default();
                if !parallel {
                    let again = tr(self.lang, "next round", "次のイテレーション");
                    let back: Vec<Loose> = ends.into_iter().map(|l| Loose { label: Some(l.label.unwrap_or_else(|| again.clone())), ..l }).collect();
                    self.connect(&back, &first);
                }
                let mut out = vec![Loose { from: sg, label: Some(exit.clone()), dashed: false }];
                out.extend(breaks);
                out
            }
            Piece::Stop(n) => {
                self.declare(*n, depth);
                vec![]
            }
            Piece::Break(n) => {
                self.declare(*n, depth);
                let l = Loose::at(&self.g.nodes[*n].id);
                if let Some(b) = self.breaks.last_mut() {
                    b.push(l);
                }
                vec![]
            }
        }
    }
}

/// One panel as a Mermaid flowchart.
pub fn mermaid(g: &Graph, p: &Panel, lang: Lang) -> String {
    let mut e = Mm { g, lang, decl: String::new(), edges: vec![], breaks: vec![], ok: vec![], bad: vec![] };
    e.declare(p.entry, 0);
    let loose = e.block(&p.body, vec![Loose::at(&g.nodes[p.entry].id)], 0);
    if !loose.is_empty() {
        e.declare(p.end, 0);
        let end = g.nodes[p.end].id.clone();
        e.connect(&loose, &end);
    }
    let mut o = String::from("flowchart TD\n");
    o.push_str(&e.decl);
    for l in &e.edges {
        o.push_str(l);
        o.push('\n');
    }
    if !e.ok.is_empty() || !e.bad.is_empty() {
        o.push_str("    classDef ok stroke:#2da44e,stroke-width:2px\n");
        o.push_str("    classDef bad stroke:#cf222e,stroke-width:2px\n");
        if !e.ok.is_empty() {
            o.push_str(&format!("    class {} ok\n", e.ok.join(",")));
        }
        if !e.bad.is_empty() {
            o.push_str(&format!("    class {} bad\n", e.bad.join(",")));
        }
    }
    o
}

// ---------------------------------------------------------------------------
// The facts: what each call does, and how the workflow ends

/// Which block a statement is in, which decides where an error nothing handles goes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Blk {
    Flow,
    OnFailure,
    OnCancel,
}

pub struct CallRow {
    pub site: usize,
    pub line: usize,
    pub call: String,
    pub how: String,
    pub retries: String,
    pub timeout: String,
    /// the errors, and where each goes
    pub fails: Vec<(Vec<String>, String)>,
    /// what the case can be after the call, for a call on a case: the case, and its states
    pub after: Option<(String, Vec<String>)>,
    /// some error goes on with nothing at the call to take it
    pub unhandled: bool,
}

pub struct EndRow {
    pub ending: Ending,
    pub line: usize,
    pub end: String,
    pub cases: Vec<String>,
}

/// Every call, in the order of the file.
pub fn call_rows(i: &Input) -> Vec<CallRow> {
    let m = i.m;
    let mut out = Vec::new();
    fn walk(i: &Input, ss: &[TStmt], blk: Blk, in_round: bool, out: &mut Vec<CallRow>) {
        for s in ss {
            match &s.kind {
                TK::Call { target, callee, args, handlers } => {
                    out.push(call_row(i, s, target.as_ref(), callee, args, handlers, blk, in_round));
                    for h in handlers {
                        walk(i, &h.body, blk, in_round, out);
                    }
                }
                TK::Match { arms, .. } => {
                    for a in arms {
                        walk(i, &a.body, blk, in_round, out);
                    }
                }
                TK::Repeat { body, .. } => walk(i, body, blk, in_round, out),
                TK::For { body, parallel, .. } => walk(i, body, blk, in_round || parallel.is_some(), out),
                _ => {}
            }
        }
    }
    walk(i, &m.flow, Blk::Flow, false, &mut out);
    if let Some(f) = &m.on_failure {
        walk(i, f, Blk::OnFailure, false, &mut out);
    }
    if let Some(f) = &m.on_cancel {
        walk(i, f, Blk::OnCancel, false, &mut out);
    }
    out.sort_by_key(|r| r.line);
    out
}

#[allow(clippy::too_many_arguments)]
fn call_row(i: &Input, s: &TStmt, target: Option<&Target>, callee: &Callee, args: &[(String, TExpr)], handlers: &[THandler], blk: Blk, in_round: bool) -> CallRow {
    let m = i.m;
    let lang = i.lang;
    let (_, lines) = call_lines(i, target, callee, args);
    let (how, retries, timeout) = match callee {
        Callee::Rule(r) => {
            let ru = &m.rules[*r];
            let mut how = match lang {
                Lang::En => format!("rule `{}`", rule_file(i, ru)),
                Lang::Ja => format!("規則 `{}`", rule_file(i, ru)),
            };
            if ru.local {
                how.push_str(&tr(lang, ", a local activity on Temporal", "（Temporal ではローカルアクティビティ）"));
            }
            // every platform retries a rule's failure `check::RULE_RETRIES` times, one second apart and then two
            let retries = tr(lang, "2 times, after 1 second and 2 (failure)", "2 回（1 秒後と 2 秒後、failure）");
            (how, retries, String::new())
        }
        Callee::Task(t) => {
            let t = &m.tasks[*t];
            let (h, code) = how_called(i, t);
            let mut how = if code { format!("`{h}`") } else { h };
            match &t.machine {
                Some(TaskMachine::Starts { rule, then }) => {
                    let mut s = format!("starts {}", m.rules[*rule].info.machine.as_ref().map(|mc| format!("{}.{}", m.rules[*rule].name, mc.name)).unwrap_or_default());
                    if !then.is_empty() {
                        s.push_str(&format!(" then {}", then.join(", ")));
                    }
                    how.push_str(&format!(", `{s}`"));
                }
                Some(TaskMachine::Sends { event, .. }) => how.push_str(&format!(", `sends {event}`")),
                Some(TaskMachine::Observes) => how.push_str(", `observes`"),
                None => {}
            }
            if t.key {
                how.push_str(", `key`");
            } else if t.idempotent {
                how.push_str(", `idempotent`");
            }
            let retries = t.retry.as_ref().map(|r| retry_words(r, lang)).unwrap_or_default();
            let timeout = t
                .timeout
                .map(|s| match lang {
                    Lang::En => crate::flow::show_dur(s),
                    Lang::Ja => crate::flow::show_dur_ja(s),
                })
                .unwrap_or_default();
            (how, retries, timeout)
        }
    };
    // the errors the call can end with, in the order the task declares them, then a timeout and a failure
    let mut kinds: Vec<String> = match callee {
        Callee::Task(t) => m.tasks[*t].errors.iter().map(|e| e.name.clone()).collect(),
        Callee::Rule(_) => vec![],
    };
    kinds.push("timeout".into());
    kinds.push("failure".into());
    let mut fails: Vec<(Vec<String>, String)> = Vec::new();
    let mut unhandled = false;
    for k in &kinds {
        let taken = handlers.iter().position(|h| {
            h.errors.iter().any(|e| match e {
                HErr::Failure => true,
                HErr::Timeout => k == "timeout",
                HErr::Declared(n) => n == k,
            })
        });
        let dest = match taken {
            Some(j) => match lang {
                Lang::En => format!("line {}", handlers[j].line),
                Lang::Ja => format!("{} 行目", handlers[j].line),
            },
            None => {
                unhandled = true;
                let on_failure = blk == Blk::Flow && m.on_failure.is_some();
                match (in_round, on_failure) {
                    (false, true) => "`on failure`".to_string(),
                    (false, false) => tr(lang, "the workflow fails", "ワークフローが失敗する"),
                    (true, true) => tr(lang, "the round fails, then `on failure`", "そのイテレーションが失敗し、`on failure` へ"),
                    (true, false) => tr(lang, "the round fails, and then the workflow", "そのイテレーションが失敗し、ワークフローも失敗する"),
                }
            }
        };
        match fails.last_mut() {
            Some((ks, d)) if *d == dest => ks.push(k.clone()),
            _ => fails.push((vec![k.clone()], dest)),
        }
    }
    let after = match target {
        Some(Target::Case(c)) => m.monitors.get(&s.site).map(|(_, st)| (m.cases[*c].name.clone(), st.clone())),
        _ => None,
    };
    CallRow { site: s.site, line: s.line, call: lines[0].clone(), how, retries, timeout, fails, after, unhandled }
}

fn retry_words(r: &Retry, lang: Lang) -> String {
    let every = match lang {
        Lang::En => crate::flow::show_dur(r.every),
        Lang::Ja => crate::flow::show_dur_ja(r.every),
    };
    let mut s = match lang {
        Lang::En => format!("{} time{} every {every}", r.times, if r.times == 1 { "" } else { "s" }),
        Lang::Ja => format!("{every}おきに {} 回", r.times),
    };
    if r.backoff != 2.0 {
        s.push_str(&match lang {
            Lang::En => format!(", the wait × {} each time", r.backoff),
            Lang::Ja => format!("（待ちは毎回 {} 倍）", r.backoff),
        });
    }
    let on = if r.on.is_empty() { "failure, timeout".to_string() } else { r.on.join(", ") };
    s.push_str(&match lang {
        Lang::En => format!(" ({on})"),
        Lang::Ja => format!("（{on}）"),
    });
    s
}

/// What a case can be, in words.
pub fn case_words(f: &CaseFact, lang: Lang) -> String {
    let states = f.states.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", ");
    let body = match f.started {
        Tri::No => tr(lang, "not started", "始まっていない"),
        Tri::Maybe if states.is_empty() => tr(lang, "not started", "始まっていない"),
        Tri::Maybe => match lang {
            Lang::En => format!("not started, or {states}"),
            Lang::Ja => format!("始まっていないか、{states}"),
        },
        Tri::Yes => states,
    };
    if f.leaving && f.started != Tri::No {
        match lang {
            Lang::En => format!("handed over as it is: {body}"),
            Lang::Ja => format!("そのまま引き渡す: {body}"),
        }
    } else {
        body
    }
}

/// Every way the workflow can end, in the order of the file.
pub fn end_rows(i: &Input) -> Vec<EndRow> {
    let m = i.m;
    let lang = i.lang;
    let mut stmt_of: BTreeMap<usize, &TStmt> = BTreeMap::new();
    for s in m.all_stmts() {
        stmt_of.insert(s.site, s);
    }
    let last = |ss: &[TStmt]| -> usize {
        let mut n = 0;
        Model::walk(ss, &mut |s| n = n.max(s.line));
        n
    };
    let mut out = Vec::new();
    for (ending, cases) in &i.facts.ends {
        let (line, end) = match ending {
            Ending::Stmt(site) => {
                let s = stmt_of[site];
                let text = match &s.kind {
                    TK::Succeed { fields } if fields.is_empty() => "`succeed`".to_string(),
                    TK::Succeed { fields } => format!("`succeed {}`", fields.iter().map(|(f, e)| format!("{f} = {}", expr_text(e))).collect::<Vec<_>>().join(", ")),
                    TK::Fail { error, cause, leaving } => {
                        let mut t = format!("`fail {error}`");
                        if let Some(c) = cause {
                            t.push_str(&format!(" {}", expr_text(c)));
                        }
                        if !leaving.is_empty() {
                            t.push_str(&format!(" `leaving {}`", leaving.iter().map(|c| m.cases[*c].name.clone()).collect::<Vec<_>>().join(", ")));
                        }
                        t
                    }
                    _ => String::new(),
                };
                (s.line, text)
            }
            Ending::Flow => (last(&m.flow), tr(lang, "the flow runs to its end, and the workflow succeeds", "flow が最後まで走り、ワークフローは成功する")),
            Ending::OnFailure => (
                m.on_failure.as_deref().map(last).unwrap_or(0),
                tr(lang, "`on failure` runs to its end, and the workflow fails with the error that started it", "`on failure` が最後まで走り、ワークフローは始まりのエラーで失敗する"),
            ),
            Ending::OnCancel => (m.on_cancel.as_deref().map(last).unwrap_or(0), tr(lang, "`on cancel` runs to its end, and the workflow ends cancelled", "`on cancel` が最後まで走り、ワークフローはキャンセルで終わる")),
        };
        out.push(EndRow { ending: *ending, line, end, cases: cases.iter().map(|f| case_words(f, lang)).collect() });
    }
    // an end of a block comes after the block's last statement
    out.sort_by_key(|r| (r.line, !matches!(r.ending, Ending::Stmt(_))));
    out
}

/// The lines of the calls in the flow whose errors can go on to `on failure`.
fn failing_lines(rows: &[CallRow], i: &Input) -> Vec<usize> {
    let mut flow_sites = BTreeSet::new();
    Model::walk(&i.m.flow, &mut |s| {
        flow_sites.insert(s.site);
    });
    rows.iter().filter(|r| r.unhandled && flow_sites.contains(&r.site)).map(|r| r.line).collect()
}

fn list_lines(lines: &[usize], lang: Lang) -> String {
    let ns: Vec<String> = lines.iter().map(|n| n.to_string()).collect();
    match lang {
        Lang::En => match ns.len() {
            0 => String::new(),
            1 => format!("line {}", ns[0]),
            _ => format!("lines {} and {}", ns[..ns.len() - 1].join(", "), ns[ns.len() - 1]),
        },
        Lang::Ja => format!("{} 行目", ns.join("・")),
    }
}

// ---------------------------------------------------------------------------
// The Markdown page

pub fn markdown(i: &Input) -> String {
    let m = i.m;
    let lang = i.lang;
    let g = graph(i);
    let rows = call_rows(i);
    let mut o = String::new();
    let _ = writeln!(o, "# {} v{}\n", m.name, m.version);
    if !m.description.is_empty() {
        let _ = writeln!(o, "{}\n", m.description);
    }
    let io = |list: &[(String, Ty)]| list.iter().map(|(n, t)| format!("`{n}: {}`", m.ty_name(t))).collect::<Vec<_>>().join(", ");
    match lang {
        Lang::En => {
            let _ = write!(o, "`{}`, drawn by `dandori doc`.", i.file);
            if !m.inputs.is_empty() {
                let _ = write!(o, " Inputs: {}.", io(&m.inputs));
            }
            if !m.outputs.is_empty() {
                let _ = write!(o, " Outputs: {}.", io(&m.outputs));
            }
        }
        Lang::Ja => {
            let _ = write!(o, "`{}` を `dandori doc` で描いたものです。", i.file);
            if !m.inputs.is_empty() {
                let _ = write!(o, "入力は {}", io(&m.inputs));
                let _ = write!(o, "{}", if m.outputs.is_empty() { " です。" } else { "、" });
            }
            if !m.outputs.is_empty() {
                let _ = write!(o, "出力は {} です。", io(&m.outputs));
            }
        }
    }
    o.push_str("\n\n");
    if diag_errors(i.diags) > 0 {
        let _ = writeln!(
            o,
            "{}\n",
            match lang {
                Lang::En => format!("**`dandori check` finds {} error(s) in this workflow**; they are at the end, each with the run that gets there.", diag_errors(i.diags)),
                Lang::Ja => format!("**`dandori check` はこのワークフローにエラーを {} 件見つけています。** 最後に、それぞれのそうなる例と一緒に載せています。", diag_errors(i.diags)),
            }
        );
    }
    for p in &g.panels {
        match p.key {
            "flow" => {
                let _ = writeln!(o, "## flow\n");
            }
            "on-failure" => {
                let _ = writeln!(o, "## on failure\n");
                let lines = failing_lines(&rows, i);
                let _ = writeln!(
                    o,
                    "{}\n",
                    match lang {
                        Lang::En => {
                            let from = if lines.is_empty() { String::new() } else { format!(": from the calls on {}", list_lines(&lines, lang)) };
                            format!("Runs when a call fails and nothing at the call handles the error{from}. When it runs to its end, the workflow fails with that error.")
                        }
                        Lang::Ja => {
                            let from = if lines.is_empty() { String::new() } else { format!("（{}の呼び出しから）", list_lines(&lines, lang)) };
                            format!("呼び出しが失敗し、そのエラーをその場で処理しないときに走ります{from}。最後まで走ると、ワークフローはそのエラーで失敗します。")
                        }
                    }
                );
            }
            _ => {
                let _ = writeln!(o, "## on cancel\n");
                let _ = writeln!(
                    o,
                    "{}\n",
                    tr(
                        lang,
                        "Runs when the workflow is cancelled, from the call or the wait the run is at. When it runs to its end, the workflow ends cancelled.",
                        "ワークフローがキャンセルされると、そのとき待っている呼び出しや wait から、ここに来ます。最後まで走ると、ワークフローはキャンセルで終わります。"
                    )
                );
            }
        }
        let _ = writeln!(o, "```mermaid\n{}```\n", mermaid(&g, p, lang));
        if p.key == "flow" {
            let _ = writeln!(o, "{}\n", legend_text(lang));
        }
    }
    if !rows.is_empty() {
        let _ = writeln!(o, "## {}\n", tr(lang, "Calls", "呼び出し"));
        let cased = !m.cases.is_empty();
        let head = match lang {
            Lang::En => "| Line | Call | Calls | Retries | Timeout | When it fails |",
            Lang::Ja => "| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき |",
        };
        let _ = write!(o, "{head}");
        if cased {
            let _ = write!(o, " {} |", tr(lang, "Case after", "呼び出しのあとの案件"));
        }
        let _ = write!(o, "\n|---:|---|---|---|---|---|");
        if cased {
            o.push_str("---|");
        }
        o.push('\n');
        for r in &rows {
            let fails = r.fails.iter().map(|(ks, d)| format!("{} → {d}", ks.iter().map(|k| format!("`{k}`")).collect::<Vec<_>>().join(", "))).collect::<Vec<_>>().join("<br>");
            let _ = write!(o, "| {} | `{}` | {} | {} | {} | {} |", r.line, r.call, r.how, dash(&r.retries), dash(&r.timeout), fails);
            if cased {
                let after = r.after.as_ref().map(|(c, st)| format!("`{c}`: {}", st.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", "))).unwrap_or_default();
                let _ = write!(o, " {} |", dash(&after));
            }
            o.push('\n');
        }
        o.push('\n');
    }
    let ends = end_rows(i);
    if !ends.is_empty() {
        let _ = writeln!(o, "## {}\n", tr(lang, "Ends", "終わり方"));
        let _ = writeln!(
            o,
            "{}\n",
            if m.cases.is_empty() {
                tr(lang, "Every way the workflow can end.", "ワークフローの終わり方のすべてです。")
            } else {
                tr(lang, "Every way the workflow can end, and what each case can be then, the events on the other side included.", "ワークフローの終わり方のすべてと、そのとき各案件がとりうる状態です。外部のサービスで起きるイベントも含めています。")
            }
        );
        let _ = write!(o, "| {} | {} |", tr(lang, "Line", "行"), tr(lang, "End", "終わり方"));
        for c in &m.cases {
            let _ = write!(o, " `{}` |", c.name);
        }
        let _ = write!(o, "\n|---:|---|");
        for _ in &m.cases {
            o.push_str("---|");
        }
        o.push('\n');
        for r in &ends {
            let _ = write!(o, "| {} | {} |", r.line, r.end);
            for c in &r.cases {
                let _ = write!(o, " {c} |");
            }
            o.push('\n');
        }
        o.push('\n');
    }
    let docs = rule_docs(i, false);
    if !docs.is_empty() {
        let _ = writeln!(o, "## {}\n", tr(lang, "Rules", "規則"));
        let _ = writeln!(o, "{}\n", tr(lang, "The rules this workflow calls, as `rulec doc` renders them for whoever approves them.", "このワークフローが呼ぶ規則を、`rulec doc` が承認する人向けに描いたものです。"));
        for (r, text) in docs {
            let _ = writeln!(o, "<details>\n<summary><code>{}</code> · {} v{} · <code>{}</code></summary>\n", esc(&r.name), esc(&r.info.rule), esc(&r.info.version), esc(&rule_path(i, r)));
            match text {
                Ok(t) => {
                    let _ = writeln!(o, "{}\n", t.trim_end());
                }
                Err(e) => {
                    let _ = writeln!(o, "{}\n\n```text\n{}\n```\n", tr(lang, "rulec could not render this rule:", "rulec はこの規則を描けませんでした。"), e.trim_end());
                }
            }
            o.push_str("</details>\n\n");
        }
    }
    if !i.diags.is_empty() {
        let _ = writeln!(o, "## {}\n", tr(lang, "What check says", "検査の結果"));
        let _ = writeln!(o, "{}\n", tr(lang, "What `dandori check` says of this workflow, each with the run that gets there.", "`dandori check` の結果です。それぞれにそうなる例が付いています。"));
        o.push_str("```text\n");
        for d in i.diags {
            o.push_str(&d.render(i.file, i.src, lang));
        }
        o.push_str("```\n");
    }
    o
}

fn dash(s: &str) -> &str {
    if s.is_empty() {
        "—"
    } else {
        s
    }
}

fn diag_errors(diags: &[Diag]) -> usize {
    diags.iter().filter(|d| d.severity == Severity::Error).count()
}

pub fn legend_text(lang: Lang) -> String {
    tr(
        lang,
        "A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.",
        "四角はタスク、両脇に線のある四角は規則、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。",
    )
}

// ---------------------------------------------------------------------------
// Lighting a run up

/// What a run lights up: the nodes it passed (and how often), the edges it went along, and the
/// calls whose error went on with nothing to take it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lit {
    pub nodes: BTreeSet<String>,
    pub edges: BTreeSet<String>,
    pub counts: BTreeMap<String, usize>,
    pub failed: BTreeSet<String>,
}

/// An edge as `draw` lays it out: its id, its ends, and what the run must have done to go along it.
pub struct EdgeInfo {
    pub id: String,
    pub from: String,
    pub to: String,
    pub conds: Vec<Cond>,
}

/// The loops of a graph: for each, by its site, the id of its first node and of its breaks.
fn loops_of(g: &Graph) -> BTreeMap<usize, (String, Vec<String>)> {
    fn first(g: &Graph, p: &Piece) -> String {
        match p {
            Piece::Node(n) | Piece::Call { node: n, .. } | Piece::Match { node: n, .. } | Piece::Stop(n) | Piece::Break(n) => g.nodes[*n].id.clone(),
            Piece::Loop { body, .. } => first(g, &body[0]),
        }
    }
    fn breaks(g: &Graph, ps: &[Piece], out: &mut Vec<String>) {
        for p in ps {
            match p {
                Piece::Break(n) => out.push(g.nodes[*n].id.clone()),
                Piece::Call { handlers: bs, .. } | Piece::Match { arms: bs, .. } => {
                    for b in bs {
                        breaks(g, &b.body, out);
                    }
                }
                _ => {}
            }
        }
    }
    fn walk(g: &Graph, ps: &[Piece], out: &mut BTreeMap<usize, (String, Vec<String>)>) {
        for p in ps {
            match p {
                Piece::Loop { site, body, .. } => {
                    let mut b = vec![];
                    breaks(g, body, &mut b);
                    out.insert(*site, (first(g, &body[0]), b));
                    walk(g, body, out);
                }
                Piece::Call { handlers: bs, .. } | Piece::Match { arms: bs, .. } => {
                    for b in bs {
                        walk(g, &b.body, out);
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = BTreeMap::new();
    for p in &g.panels {
        walk(g, &p.body, &mut out);
    }
    out
}

/// What a scenario's run lights up, from what the reference interpreter says it went through.
pub fn lit_by_visits(g: &Graph, edges: &[EdgeInfo], visits: &[Visit]) -> Lit {
    let mut lit = Lit::default();
    let mut conds: BTreeSet<Cond> = BTreeSet::new();
    lit.nodes.insert("start".into());
    for v in visits {
        match v {
            Visit::Stmt(site) => {
                if let Some(id) = stmt_id(g, *site) {
                    *lit.counts.entry(id.clone()).or_insert(0) += 1;
                    lit.nodes.insert(id);
                }
            }
            Visit::Arm(site, k) => {
                conds.insert(Cond::Arm(*site, *k));
            }
            Visit::Answer(site, kind) if kind == "ok" => {
                conds.insert(Cond::Ok(*site));
            }
            Visit::Answer(..) => {}
            Visit::Handler(site, j) => {
                conds.insert(Cond::Handler(*site, *j));
            }
            Visit::Unhandled(site) => {
                lit.failed.extend(stmt_id(g, *site));
            }
            Visit::Round(site, n) => {
                if *n >= 1 {
                    conds.insert(Cond::Again(*site));
                }
                // a loop whose body draws nothing has a node that stands for it
                let stand_in = format!("s{site}p");
                if g.node(&stand_in).is_some() {
                    *lit.counts.entry(stand_in.clone()).or_insert(0) += 1;
                    lit.nodes.insert(stand_in);
                }
            }
            Visit::Done(site) => {
                conds.insert(Cond::Done(*site));
            }
            Visit::OnFailure => {
                lit.nodes.insert("onf".into());
            }
            Visit::OnCancel => {
                lit.nodes.insert("onc".into());
            }
            Visit::FlowEnd => {
                lit.nodes.insert("fin".into());
            }
            Visit::OnFailureEnd => {
                lit.nodes.insert("onfEnd".into());
            }
            Visit::OnCancelEnd => {
                lit.nodes.insert("oncEnd".into());
            }
        }
    }
    light_edges(&mut lit, edges, &conds);
    lit
}

/// What a diagnostic's run lights up, from the places of its steps.
pub fn lit_by_steps(g: &Graph, edges: &[EdgeInfo], d: &Diag) -> Lit {
    let mut lit = Lit::default();
    let mut conds: BTreeSet<Cond> = BTreeSet::new();
    lit.nodes.insert("start".into());
    let mut handled: BTreeSet<usize> = BTreeSet::new();
    for s in &d.path {
        match s.at {
            Some(At::Stmt(site)) | Some(At::Cancelled(site)) => {
                if let Some(id) = stmt_id(g, site) {
                    *lit.counts.entry(id.clone()).or_insert(0) += 1;
                    lit.nodes.insert(id);
                }
            }
            Some(At::Arm(site, k)) => {
                lit.nodes.extend(stmt_id(g, site));
                conds.insert(Cond::Arm(site, k));
            }
            Some(At::Handler(site, j)) => {
                lit.nodes.extend(stmt_id(g, site));
                handled.insert(site);
                conds.insert(Cond::Handler(site, j));
            }
            Some(At::Fails(site)) => {
                lit.nodes.extend(stmt_id(g, site));
                handled.insert(site);
                lit.failed.extend(stmt_id(g, site));
            }
            Some(At::OnFailure) => {
                lit.nodes.insert("onf".into());
            }
            Some(At::OnCancel) => {
                lit.nodes.insert("onc".into());
            }
            Some(At::FlowEnd) => {
                lit.nodes.insert("fin".into());
            }
            Some(At::OnFailureEnd) => {
                lit.nodes.insert("onfEnd".into());
            }
            Some(At::OnCancelEnd) => {
                lit.nodes.insert("oncEnd".into());
            }
            None => {}
        }
    }
    // the steps do not say how a call came back or how a loop went on; read them off what else the run passed
    for n in &g.nodes {
        if let Some(site) = n.site {
            if lit.nodes.contains(&n.id) && !handled.contains(&site) && matches!(n.kind, Kind::Call | Kind::Rule | Kind::Receive) {
                conds.insert(Cond::Ok(site));
            }
        }
    }
    for (site, (first, breaks)) in loops_of(g) {
        if lit.counts.get(&first).copied().unwrap_or(0) >= 2 {
            conds.insert(Cond::Again(site));
        }
        if !breaks.iter().any(|b| lit.nodes.contains(b)) {
            conds.insert(Cond::Done(site));
        }
    }
    light_edges(&mut lit, edges, &conds);
    lit
}

fn light_edges(lit: &mut Lit, edges: &[EdgeInfo], conds: &BTreeSet<Cond>) {
    for e in edges {
        if lit.nodes.contains(&e.from) && lit.nodes.contains(&e.to) && e.conds.iter().all(|c| conds.contains(c)) {
            lit.edges.insert(e.id.clone());
        }
    }
}

/// What a statement is drawn as: its node, or its loop's frame; None for one not drawn (`pass`).
fn stmt_id(g: &Graph, site: usize) -> Option<String> {
    let id = format!("s{site}");
    if g.node(&id).is_some() {
        Some(id)
    } else if g.loops.contains_key(&site) {
        Some(format!("L{site}"))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// The HTML page

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The facts' text as HTML: escaped, and `.flow` between backquotes as code.
fn inline(s: &str) -> String {
    let mut o = String::new();
    for (k, part) in s.split('`').enumerate() {
        if k % 2 == 1 {
            o.push_str(&format!("<code>{}</code>", esc(part)));
        } else {
            o.push_str(&esc(part).replace("&lt;br&gt;", "<br>"));
        }
    }
    o
}

/// Lines of the `.flow`, numbered, and moved left as far as the least indented of them.
fn src_block(src: &str, lines: &[usize]) -> String {
    let mut seen = BTreeSet::new();
    let picked: Vec<(usize, &str)> = lines.iter().filter(|n| **n > 0 && seen.insert(**n)).map(|n| (*n, src_line(src, *n))).collect();
    let indent = picked.iter().filter(|(_, l)| !l.trim().is_empty()).map(|(_, l)| l.len() - l.trim_start().len()).min().unwrap_or(0);
    let mut o = String::from("<pre class=\"src\">");
    for (n, l) in picked {
        o.push_str(&format!("<span class=\"ln\">{n:>4}</span>  {}\n", esc(l.get(indent..).unwrap_or(l.trim_start()))));
    }
    o.push_str("</pre>");
    o
}

fn decl_block(src: &str, line: usize) -> String {
    let lines: Vec<usize> = decl_lines(src, line).into_iter().map(|(n, _)| n).collect();
    src_block(src, &lines)
}

/// What an error the generated code raises itself means.
fn own_error(name: &str, lang: Lang) -> Option<String> {
    let (en, ja) = match name {
        "Dandori.BadResponse" => ("an answer not of the declared type", "宣言した型に合わない結果"),
        "Dandori.UnexpectedState" => ("a case in a state the machine does not lead to there", "ステートマシンがそこでは行かない状態の案件"),
        "Dandori.BadInput" => ("an input not of the declared type", "宣言した型に合わない入力"),
        "Dandori.TooManyItems" => ("a list longer than the loop's bound", "ループの上限より長いリスト"),
        "Dandori.UnexpectedValue" => ("a value no arm takes", "どの分岐にも当たらない値"),
        _ => return None,
    };
    Some(tr(lang, en, ja))
}

/// How a run ended, as the trace says: a word for the group, and a class.
fn trace_end(end: &serde_json::Value, lang: Lang) -> (String, &'static str) {
    if end.get("succeed").is_some() {
        (tr(lang, "succeeds", "成功"), "ok")
    } else if let Some(f) = end.get("fail") {
        let name = f["error"].as_str().unwrap_or("");
        match own_error(name, lang) {
            Some(what) => (format!("fail {name}: {what}"), "bad"),
            None => (
                match lang {
                    Lang::En => format!("fails with {name}"),
                    Lang::Ja => format!("{name} で失敗する"),
                },
                "bad",
            ),
        }
    } else if end.get("cancel").is_some() {
        (tr(lang, "ends cancelled", "キャンセルで終わる"), "cancel")
    } else {
        (String::new(), "")
    }
}

struct Page<'a> {
    i: &'a Input<'a>,
    g: Graph,
    stmt_of: BTreeMap<usize, TStmt>,
    rows: Vec<CallRow>,
}

impl<'a> Page<'a> {
    fn lang(&self) -> Lang {
        self.i.lang
    }

    fn here(&self, site: usize) -> String {
        let m = self.i.m;
        let Some(facts) = self.i.facts.at.get(&site) else { return String::new() };
        let mut o = String::new();
        for (c, f) in facts.iter().enumerate() {
            let _ = write!(o, "<li><code>{}</code>: {}</li>", esc(&m.cases[c].name), inline(&case_words(f, self.lang())));
        }
        if o.is_empty() {
            return o;
        }
        format!("<h3>{}</h3><ul class=\"facts\">{o}</ul>", tr(self.lang(), "The cases when a run gets here", "ここに来たときの案件"))
    }

    fn row(&self, site: usize) -> Option<&CallRow> {
        self.rows.iter().find(|r| r.site == site)
    }

    fn call_facts(&self, r: &CallRow) -> String {
        let lang = self.lang();
        let mut o = String::from("<dl class=\"facts\">");
        let _ = write!(o, "<dt>{}</dt><dd>{}</dd>", tr(lang, "Calls", "呼ぶもの"), inline(&r.how));
        if !r.retries.is_empty() {
            let _ = write!(o, "<dt>{}</dt><dd>{}</dd>", tr(lang, "Retries", "リトライ"), inline(&r.retries));
        }
        if !r.timeout.is_empty() {
            let _ = write!(o, "<dt>{}</dt><dd>{}</dd>", tr(lang, "Timeout", "タイムアウト"), inline(&r.timeout));
        }
        let fails = r.fails.iter().map(|(ks, d)| format!("{} → {}", ks.iter().map(|k| format!("<code>{}</code>", esc(k))).collect::<Vec<_>>().join(", "), inline(d))).collect::<Vec<_>>().join("<br>");
        let _ = write!(o, "<dt>{}</dt><dd>{fails}</dd>", tr(lang, "When it fails", "失敗したとき"));
        if let Some((c, st)) = &r.after {
            let _ = write!(o, "<dt>{}</dt><dd><code>{}</code>: {}</dd>", tr(lang, "The case after it", "呼び出しのあとの案件"), esc(c), st.iter().map(|s| format!("<code>{}</code>", esc(s))).collect::<Vec<_>>().join(", "));
        }
        o.push_str("</dl>");
        o
    }

    fn ends_of(&self, e: Ending) -> String {
        let m = self.i.m;
        let Some(facts) = self.i.facts.ends.get(&e) else { return String::new() };
        if facts.is_empty() {
            return String::new();
        }
        let mut o = format!("<h3>{}</h3><ul class=\"facts\">", tr(self.lang(), "The cases when it ends", "終わるときの案件"));
        for (c, f) in facts.iter().enumerate() {
            let _ = write!(o, "<li><code>{}</code>: {}</li>", esc(&m.cases[c].name), inline(&case_words(f, self.lang())));
        }
        o.push_str("</ul>");
        o
    }

    fn head(&self, title: &str, line: usize) -> String {
        let ln = if line > 0 {
            match self.lang() {
                Lang::En => format!(" <span class=\"dl\">line {line}</span>"),
                Lang::Ja => format!(" <span class=\"dl\">{line} 行目</span>"),
            }
        } else {
            String::new()
        };
        format!("<h2 class=\"dh\"><code>{}</code>{ln}</h2>", esc(title))
    }

    /// What the detail pane says of a node.
    fn node_detail(&self, n: &Node) -> String {
        let i = self.i;
        let m = i.m;
        let lang = self.lang();
        let mut o = self.head(&n.lines[0], n.line);
        match n.id.as_str() {
            "start" => {
                if !m.description.is_empty() {
                    let _ = write!(o, "<p>{}</p>", esc(&m.description));
                }
                let io = |list: &[(String, Ty)]| list.iter().map(|(n, t)| format!("<code>{}: {}</code>", esc(n), esc(&m.ty_name(t)))).collect::<Vec<_>>().join(", ");
                o.push_str("<dl class=\"facts\">");
                if !m.inputs.is_empty() {
                    let _ = write!(o, "<dt>{}</dt><dd>{}</dd>", tr(lang, "Inputs", "入力"), io(&m.inputs));
                }
                if !m.outputs.is_empty() {
                    let _ = write!(o, "<dt>{}</dt><dd>{}</dd>", tr(lang, "Outputs", "出力"), io(&m.outputs));
                }
                let _ = write!(o, "<dt>{}</dt><dd><code>{}</code></dd></dl>", tr(lang, "File", "ファイル"), esc(i.file));
                return o;
            }
            "onf" => {
                let lines = failing_lines(&self.rows, i);
                let from = if lines.is_empty() { String::new() } else { match lang {
                    Lang::En => format!(" It comes here from the calls on {}.", list_lines(&lines, lang)),
                    Lang::Ja => format!("{}の呼び出しから、ここに来ます。", list_lines(&lines, lang)),
                } };
                let _ = write!(o, "<p>{}{from}</p>", tr(lang, "Runs when a call fails and nothing at the call handles the error. When it runs to its end, the workflow fails with that error.", "呼び出しが失敗し、そのエラーをその場で処理しないときに走ります。最後まで走ると、ワークフローはそのエラーで失敗します。"));
                return o;
            }
            "onc" => {
                let _ = write!(o, "<p>{}</p>", tr(lang, "Runs when the workflow is cancelled, from the call or the wait the run is at. When it runs to its end, the workflow ends cancelled.", "ワークフローがキャンセルされると、そのとき待っている呼び出しや wait から、ここに来ます。最後まで走ると、ワークフローはキャンセルで終わります。"));
                return o;
            }
            "fin" => {
                let _ = write!(o, "<p>{}</p>{}", tr(lang, "The flow runs to its end, and the workflow succeeds.", "flow が最後まで走り、ワークフローは成功します。"), self.ends_of(Ending::Flow));
                return o;
            }
            "onfEnd" => {
                let _ = write!(o, "<p>{}</p>{}", tr(lang, "`on failure` runs to its end, and the workflow fails with the error that started it.", "`on failure` が最後まで走り、ワークフローは始まりのエラーで失敗します。").replace('`', ""), self.ends_of(Ending::OnFailure));
                return o;
            }
            "oncEnd" => {
                let _ = write!(o, "<p>{}</p>{}", tr(lang, "`on cancel` runs to its end, and the workflow ends cancelled.", "`on cancel` が最後まで走り、ワークフローはキャンセルで終わります。").replace('`', ""), self.ends_of(Ending::OnCancel));
                return o;
            }
            _ => {}
        }
        let Some(site) = n.site else { return o };
        let Some(s) = self.stmt_of.get(&site) else { return o };
        let mut lines = vec![s.line];
        match &s.kind {
            TK::Call { handlers, .. } => lines.extend(handlers.iter().map(|h| h.line)),
            TK::Match { arms, .. } => lines.extend(arms.iter().map(|a| a.line)),
            _ => {}
        }
        o.push_str(&src_block(i.src, &lines));
        o.push_str(&self.here(site));
        match &s.kind {
            TK::Call { callee, .. } => {
                if let Some(r) = self.row(site) {
                    o.push_str(&self.call_facts(r));
                }
                let (title, line) = match callee {
                    Callee::Task(t) => (tr(lang, "The task", "タスクの宣言"), m.tasks[*t].line),
                    Callee::Rule(r) => (tr(lang, "The rule", "規則の宣言"), m.rules[*r].line),
                };
                let _ = write!(o, "<h3>{title}</h3>{}", decl_block(i.src, line));
                if let Callee::Rule(r) = callee {
                    let _ = write!(o, "<p><button class=\"rule\" type=\"button\" data-rule=\"{r}\">{}</button></p>", tr(lang, "Open the rule's page", "規則のページを開く"));
                }
            }
            TK::Succeed { .. } | TK::Fail { .. } => o.push_str(&self.ends_of(Ending::Stmt(site))),
            _ => {}
        }
        o
    }

    fn frame_detail(&self, site: usize) -> String {
        let Some(s) = self.stmt_of.get(&site) else { return String::new() };
        let title = self.g.loops.get(&site).cloned().unwrap_or_default();
        let mut o = self.head(&title, s.line);
        o.push_str(&src_block(self.i.src, &[s.line]));
        o.push_str(&self.here(site));
        o
    }

    /// A scenario's run, step by step, from what it went through.
    fn run_steps(&self, sc: &serde_json::Value, visits: &[Visit]) -> String {
        let m = self.i.m;
        let lang = self.lang();
        let answers = sc["answers"].as_array().cloned().unwrap_or_default();
        let mut next = 0;
        let mut items: Vec<String> = Vec::new();
        let mut cur: Option<String> = None;
        let flush = |cur: &mut Option<String>, items: &mut Vec<String>| {
            if let Some(c) = cur.take() {
                items.push(c);
            }
        };
        for v in visits {
            match v {
                Visit::Stmt(site) => {
                    flush(&mut cur, &mut items);
                    let Some(s) = self.stmt_of.get(site) else { continue };
                    let text = match &s.kind {
                        TK::Repeat { .. } | TK::For { .. } => self.g.loops.get(site).cloned().unwrap_or_default(),
                        TK::Pass => continue,
                        _ => self.g.node(&format!("s{site}")).map(|n| n.lines[0].clone()).unwrap_or_default(),
                    };
                    cur = Some(format!("<span class=\"ln\">{}</span> <code>{}</code>", s.line, esc(&text)));
                }
                Visit::Answer(site, kind) => {
                    let a = answers.get(next).cloned().unwrap_or_default();
                    next += 1;
                    let chip = if kind == "ok" {
                        let state = match self.stmt_of.get(site).map(|s| &s.kind) {
                            Some(TK::Call { target: Some(Target::Case(c)), .. }) => a["ok"].get(&m.cases[*c].state_field).and_then(|x| x.as_str()).map(|x| format!(": {x}")),
                            _ => None,
                        };
                        format!(" <span class=\"chip ok\">ok{}</span>", esc(&state.unwrap_or_default()))
                    } else {
                        format!(" <span class=\"chip bad\">{}</span>", esc(kind))
                    };
                    if let Some(c) = cur.as_mut() {
                        c.push_str(&chip);
                    }
                }
                Visit::Arm(site, k) => {
                    if let (Some(c), Some(TK::Match { arms, .. })) = (cur.as_mut(), self.stmt_of.get(site).map(|s| &s.kind)) {
                        c.push_str(&format!(" → <code>{}</code>", esc(&arm_label(&arms[*k]))));
                    }
                }
                Visit::Handler(site, j) => {
                    if let (Some(c), Some(TK::Call { handlers, .. })) = (cur.as_mut(), self.stmt_of.get(site).map(|s| &s.kind)) {
                        c.push_str(&format!(" → <code>on {}</code>", esc(&herr_names(&handlers[*j].errors).join(", "))));
                    }
                }
                Visit::Unhandled(_) => {
                    if let Some(c) = cur.as_mut() {
                        c.push_str(&format!(" → {}", tr(lang, "not handled here", "ここでは処理しない")));
                    }
                }
                Visit::Round(_, n) => {
                    if *n >= 1 {
                        flush(&mut cur, &mut items);
                        items.push(format!("<span class=\"round\">{}</span>", match lang {
                            Lang::En => format!("round {}", n + 1),
                            Lang::Ja => format!("{} 回目", n + 1),
                        }));
                    }
                }
                Visit::Done(_) => {}
                Visit::OnFailure => {
                    flush(&mut cur, &mut items);
                    items.push("<code>on failure</code>".into());
                }
                Visit::OnCancel => {
                    flush(&mut cur, &mut items);
                    items.push("<code>on cancel</code>".into());
                }
                Visit::FlowEnd | Visit::OnFailureEnd | Visit::OnCancelEnd => {
                    flush(&mut cur, &mut items);
                    let id = match v {
                        Visit::FlowEnd => "fin",
                        Visit::OnFailureEnd => "onfEnd",
                        _ => "oncEnd",
                    };
                    if let Some(n) = self.g.node(id) {
                        items.push(esc(&n.lines[0]));
                    }
                }
            }
        }
        flush(&mut cur, &mut items);
        let mut o = String::from("<ol class=\"steps\">");
        for it in items {
            let _ = write!(o, "<li>{it}</li>");
        }
        o.push_str("</ol>");
        o
    }
}

/// Where a run ended, from what it went through: the node, or None when it failed at a call.
fn end_node(g: &Graph, stmt_of: &BTreeMap<usize, TStmt>, visits: &[Visit]) -> Option<String> {
    for v in visits.iter().rev() {
        match v {
            Visit::FlowEnd => return Some("fin".into()),
            Visit::OnFailureEnd => return Some("onfEnd".into()),
            Visit::OnCancelEnd => return Some("oncEnd".into()),
            Visit::Stmt(site) => {
                return match stmt_of.get(site).map(|s| &s.kind) {
                    Some(TK::Succeed { .. }) | Some(TK::Fail { .. }) => stmt_id(g, *site),
                    _ => None,
                }
            }
            _ => {}
        }
    }
    None
}

fn lit_json(l: &Lit) -> serde_json::Value {
    serde_json::json!({ "nodes": l.nodes, "edges": l.edges, "counts": l.counts, "failed": l.failed })
}

fn calls_table(rows: &[CallRow], cased: bool, lang: Lang) -> String {
    let mut o = String::from("<table><thead><tr>");
    let heads = match lang {
        Lang::En => ["Line", "Call", "Calls", "Retries", "Timeout", "When it fails"],
        Lang::Ja => ["行", "呼び出し", "呼ぶもの", "リトライ", "タイムアウト", "失敗したとき"],
    };
    for h in heads {
        let _ = write!(o, "<th>{h}</th>");
    }
    if cased {
        let _ = write!(o, "<th>{}</th>", tr(lang, "Case after", "呼び出しのあとの案件"));
    }
    o.push_str("</tr></thead><tbody>");
    for r in rows {
        let fails = r.fails.iter().map(|(ks, d)| format!("{} → {}", ks.iter().map(|k| format!("<code>{}</code>", esc(k))).collect::<Vec<_>>().join(", "), inline(d))).collect::<Vec<_>>().join("<br>");
        let _ = write!(o, "<tr><td class=\"num\">{}</td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{fails}</td>", r.line, esc(&r.call), inline(&r.how), inline(dash(&r.retries)), inline(dash(&r.timeout)));
        if cased {
            let after = r.after.as_ref().map(|(c, st)| format!("<code>{}</code>: {}", esc(c), st.iter().map(|s| format!("<code>{}</code>", esc(s))).collect::<Vec<_>>().join(", "))).unwrap_or_else(|| "—".into());
            let _ = write!(o, "<td>{after}</td>");
        }
        o.push_str("</tr>");
    }
    o.push_str("</tbody></table>");
    o
}

fn ends_table(i: &Input, rows: &[EndRow]) -> String {
    let lang = i.lang;
    let mut o = format!("<table><thead><tr><th>{}</th><th>{}</th>", tr(lang, "Line", "行"), tr(lang, "End", "終わり方"));
    for c in &i.m.cases {
        let _ = write!(o, "<th><code>{}</code></th>", esc(&c.name));
    }
    o.push_str("</tr></thead><tbody>");
    for r in rows {
        let _ = write!(o, "<tr><td class=\"num\">{}</td><td>{}</td>", r.line, inline(&r.end));
        for c in &r.cases {
            let _ = write!(o, "<td>{}</td>", inline(c));
        }
        o.push_str("</tr>");
    }
    o.push_str("</tbody></table>");
    o
}

fn legend(lang: Lang) -> String {
    let items: [(&str, String); 9] = [
        ("<rect class=\"lg\" x=\"2\" y=\"2\" width=\"26\" height=\"13\" rx=\"2\"/>", tr(lang, "a task", "タスク")),
        ("<rect class=\"lg\" x=\"2\" y=\"2\" width=\"26\" height=\"13\" rx=\"2\"/><path class=\"lg\" d=\"M6,2 V15 M24,2 V15\"/>", tr(lang, "a rule", "規則")),
        ("<polygon class=\"lg\" points=\"7,2 28,2 23,15 2,15\"/>", tr(lang, "a task whose value comes from outside: an event, or the answer to a callback", "外から値が届くタスク（イベントやコールバックの応答）")),
        ("<polygon class=\"lg\" points=\"1,8.5 7,2 23,2 29,8.5 23,15 7,15\"/>", tr(lang, "a match", "match")),
        ("<rect class=\"lg\" x=\"2\" y=\"2\" width=\"26\" height=\"13\" rx=\"6.5\"/>", tr(lang, "a wait", "待ち")),
        ("<rect class=\"lg frame\" x=\"2\" y=\"2\" width=\"26\" height=\"13\" rx=\"3\"/>", tr(lang, "a loop, around the steps it repeats", "ループ（繰り返すステップを囲む）")),
        ("<rect class=\"lg ok\" x=\"2\" y=\"2\" width=\"12\" height=\"13\" rx=\"6\"/><rect class=\"lg bad\" x=\"16\" y=\"2\" width=\"12\" height=\"13\" rx=\"6\"/>", tr(lang, "an end: succeed, fail", "終わり（succeed、fail）")),
        ("<path class=\"lg-e dash\" d=\"M2,8.5 H28\"/>", tr(lang, "an error the call handles", "呼び出しがその場で処理するエラー")),
        ("<circle class=\"lg-tick\" cx=\"15\" cy=\"8.5\" r=\"6\"/><text class=\"lg-tick-t\" x=\"15\" y=\"12\">!</text>", tr(lang, "a call with an error it does not handle", "その場で処理しないエラーがある呼び出し")),
    ];
    let mut o = String::from("<ul class=\"legend\">");
    for (svg, text) in items {
        let _ = write!(o, "<li><svg viewBox=\"0 0 30 17\" width=\"30\" height=\"17\" aria-hidden=\"true\">{svg}</svg><span>{}</span></li>", esc(&text));
    }
    o.push_str("</ul>");
    o
}

pub fn html(i: &Input) -> String {
    let m = i.m;
    let lang = i.lang;
    let g = graph(i);
    let rows = call_rows(i);
    let ends = end_rows(i);
    let mut stmt_of = BTreeMap::new();
    for s in m.all_stmts() {
        stmt_of.insert(s.site, s.clone());
    }
    let page = Page { i, g, stmt_of, rows };
    let g = &page.g;
    let tick = |id: &str| -> Option<String> {
        let r = page.rows.iter().find(|r| format!("s{}", r.site) == id && r.unhandled)?;
        Some(r.fails.iter().map(|(ks, d)| format!("{} → {}", ks.join(", "), d.replace('`', ""))).collect::<Vec<_>>().join("; "))
    };
    let drawn: Vec<(&Panel, crate::draw::Drawn)> = g.panels.iter().map(|p| (p, crate::draw::panel(g, p, &tick))).collect();
    let edges: Vec<EdgeInfo> = drawn.iter().flat_map(|(_, d)| d.edges.iter().map(|e| EdgeInfo { id: e.id.clone(), from: e.from.clone(), to: e.to.clone(), conds: e.conds.clone() })).collect();
    let errors = diag_errors(i.diags);

    // the scenarios, when the workflow passes check: each run, what it lights up, and where it ends
    struct Shown {
        name: String,
        lit: Lit,
        end: Option<String>,
        steps: String,
        word: String,
        cls: &'static str,
    }
    let mut runs: Vec<Shown> = Vec::new();
    if errors == 0 {
        for sc in crate::scenarios::generate(m) {
            let Ok((trace, visits)) = crate::interp::run_visits(m, &sc, crate::render::View::Temporal) else { continue };
            let (word, cls) = trace_end(&trace["end"], lang);
            runs.push(Shown {
                name: sc["name"].as_str().unwrap_or("").trim_start_matches("run ").to_string(),
                lit: lit_by_visits(g, &edges, &visits),
                end: end_node(g, &page.stmt_of, &visits),
                steps: page.run_steps(&sc, &visits),
                word,
                cls,
            });
        }
    }
    let checks: Vec<Lit> = i.diags.iter().map(|d| lit_by_steps(g, &edges, d)).collect();
    let docs = rule_docs(i, true);

    let mut o = String::new();
    let _ = write!(
        o,
        "<!doctype html>\n<html lang=\"{}\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"generator\" content=\"dandori doc\">\n<title>{} v{}</title>\n<style>{CSS}</style>\n</head>\n<body>\n",
        if lang == Lang::Ja { "ja" } else { "en" },
        esc(&m.name),
        m.version
    );
    // the arrowheads every picture shares
    o.push_str("<svg width=\"0\" height=\"0\" style=\"position:absolute\" aria-hidden=\"true\"><defs><marker id=\"dd-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\"><path class=\"ah\" d=\"M0,1 L10,5 L0,9 z\"/></marker><marker id=\"dd-arrow-lit\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\"><path class=\"ah lit\" d=\"M0,1 L10,5 L0,9 z\"/></marker></defs></svg>\n");
    o.push_str("<div class=\"app\">\n");
    let _ = write!(o, "<header class=\"top\"><h1><code>{}</code> <span class=\"ver\">v{}</span></h1><p class=\"file\"><code>{}</code> · dandori doc</p>", esc(&m.name), m.version, esc(i.file));
    if !m.description.is_empty() {
        let _ = write!(o, "<p class=\"desc\" title=\"{}\">{}</p>", esc(&m.description), esc(&m.description));
    }
    if errors > 0 {
        let _ = write!(
            o,
            "<p class=\"banner\">{}</p>",
            match lang {
                Lang::En => format!("<code>dandori check</code> finds {errors} error(s) in this workflow. Pick one on the left to see the run that gets there."),
                Lang::Ja => format!("<code>dandori check</code> はこのワークフローにエラーを {errors} 件見つけています。左で選ぶと、そうなる例が図の上で光ります。"),
            }
        );
    }
    o.push_str("</header>\n<main class=\"grid\">\n<nav class=\"side\" aria-label=\"runs\">");
    if !i.diags.is_empty() {
        let _ = write!(o, "<section><h2>{}</h2><ul class=\"checks\">", tr(lang, "What check says", "検査の結果"));
        for (k, d) in i.diags.iter().enumerate() {
            let sev = if d.severity == Severity::Error { "err" } else { "warn" };
            let _ = write!(o, "<li><button class=\"check\" data-check=\"{k}\" aria-pressed=\"false\"><span class=\"code {sev}\">{}</span> <span class=\"dl\">{}</span> {}</button></li>", d.code, d.line, esc(&short(d.message(lang), 120)));
        }
        o.push_str("</ul></section>");
    }
    if !docs.is_empty() {
        let _ = write!(
            o,
            "<section><h2>{}</h2><p class=\"note\">{}</p><ul class=\"rules\">",
            tr(lang, "Rules", "規則"),
            tr(lang, "Each as <code>rulec doc</code> renders it for whoever approves it; a case can be tried on it.", "<code>rulec doc</code> が承認する人向けに描いたページです。ケースを打って試せます。")
        );
        for (k, (r, _)) in docs.iter().enumerate() {
            let _ = write!(o, "<li><button class=\"rule\" type=\"button\" data-rule=\"{k}\"><code>{}</code> <span class=\"dl\">{} v{}</span></button></li>", esc(&r.name), esc(&r.info.rule), esc(&r.info.version));
        }
        o.push_str("</ul></section>");
    }
    let _ = write!(o, "<section><h2>{}</h2>", tr(lang, "Scenarios", "シナリオ"));
    if errors > 0 {
        let _ = write!(o, "<p class=\"note\">{}</p>", tr(lang, "None: `dandori check` finds errors in this workflow.", "検査でエラーが見つかっているので、シナリオはありません。").replace('`', ""));
    } else {
        let every = if m.cases.is_empty() {
            match lang {
                Lang::En => format!("{} scenarios that together take every arm and every handler of an error, grouped by how they end. Pick one to light up the way it goes.", runs.len()),
                Lang::Ja => format!("すべての分岐と、エラーを処理するすべての箇所を通る {} 本のシナリオを、終わり方ごとにまとめています。選ぶと、その実行が通るところが光ります。", runs.len()),
            }
        } else {
            match lang {
                Lang::En => format!("{} scenarios that together take every arm, every handler of an error and every way a case can move, grouped by how they end. Pick one to light up the way it goes.", runs.len()),
                Lang::Ja => format!("すべての分岐、エラーを処理するすべての箇所、案件の状態の移り方のすべてを通る {} 本のシナリオを、終わり方ごとにまとめています。選ぶと、その実行が通るところが光ります。", runs.len()),
            }
        };
        let _ = write!(o, "<p class=\"note\">{every}</p>");
        // grouped by where they end, in the order of the file
        let mut groups: Vec<(String, String, &'static str, Vec<usize>)> = Vec::new();
        for (k, r) in runs.iter().enumerate() {
            let (key, label) = match r.end.as_ref().and_then(|id| g.node(id)) {
                Some(n) => (n.id.clone(), if n.line > 0 { format!("{} {}", n.line, n.lines[0]) } else { n.lines[0].clone() }),
                None => (format!("x:{}", r.word), r.word.clone()),
            };
            match groups.iter_mut().find(|x| x.0 == key) {
                Some(gr) => gr.3.push(k),
                None => groups.push((key, label, r.cls, vec![k])),
            }
        }
        let order = |key: &str| -> (usize, usize) {
            match g.node(key) {
                Some(n) if n.line > 0 => (0, n.line),
                Some(n) => (1, g.nodes.iter().position(|x| x.id == n.id).unwrap_or(0)),
                None => (2, 0),
            }
        };
        groups.sort_by_key(|x| order(&x.0));
        o.push_str("<ul class=\"groups\">");
        for (_, label, cls, ks) in &groups {
            let _ = write!(o, "<li class=\"group\"><div class=\"gl {cls}\"><code>{}</code> <span class=\"gn\">{}</span></div><div class=\"runs\">", esc(label), ks.len());
            for k in ks {
                let _ = write!(o, "<button class=\"run\" data-run=\"{k}\" aria-pressed=\"false\">{}</button>", esc(&runs[*k].name));
            }
            o.push_str("</div></li>");
        }
        o.push_str("</ul>");
    }
    o.push_str("</section></nav>\n<section class=\"canvas\" tabindex=\"-1\">");
    for (p, d) in &drawn {
        let cap = match p.key {
            "flow" => "flow",
            "on-failure" => "on failure",
            _ => "on cancel",
        };
        let _ = write!(o, "<figure class=\"panel\" data-panel=\"{}\"><figcaption><code>{cap}</code></figcaption>{}</figure>", p.key, d.svg);
    }
    o.push_str("</section>\n<aside class=\"detail\" id=\"dd-detail\" aria-live=\"polite\"></aside>\n</main>\n</div>\n");
    if !docs.is_empty() {
        // a rule's page wants the whole window, and runs its own script: it opens over this page, in a
        // frame whose script cannot reach this one
        let _ = writeln!(
            o,
            "<div class=\"sheet\" id=\"dd-sheet\" role=\"dialog\" aria-modal=\"true\" aria-label=\"rulec doc\" hidden><div class=\"bar\"><span class=\"what\"></span><button class=\"close\" type=\"button\" title=\"{c}\" aria-label=\"{c}\">×</button></div><iframe sandbox=\"allow-scripts\" title=\"rulec doc\"></iframe></div>",
            c = tr(lang, "Close", "閉じる")
        );
    }
    // what does not need a script: the facts, as the Markdown has them
    let _ = write!(o, "<section class=\"tables\"><h2>{}</h2>{}", tr(lang, "Calls", "呼び出し"), calls_table(&page.rows, !m.cases.is_empty(), lang));
    if !ends.is_empty() {
        let _ = write!(o, "<h2>{}</h2>{}", tr(lang, "Ends", "終わり方"), ends_table(i, &ends));
    }
    o.push_str("</section>\n");
    // the detail pane's pages
    let _ = writeln!(
        o,
        "<template id=\"t-default\"><h2 class=\"dh\">{}</h2><p>{}</p>{}</template>",
        tr(lang, "Reading the picture", "図の読み方"),
        tr(lang, "Pick a step to see what the checker knows there. Pick a scenario on the left to light up the way its run goes; the number beside a step is how often the run passes it.", "ステップを選ぶと、そこで検査が知っていることが出ます。左のシナリオを選ぶと、その実行が通るところが光ります。ステップの横の数は、その実行が通った回数です。"),
        legend(lang)
    );
    for n in &g.nodes {
        let _ = writeln!(o, "<template id=\"t-{}\">{}</template>", n.id, page.node_detail(n));
    }
    for site in g.loops.keys() {
        let _ = writeln!(o, "<template id=\"t-L{site}\">{}</template>", page.frame_detail(*site));
    }
    for (k, r) in runs.iter().enumerate() {
        let title = match lang {
            Lang::En => format!("scenario {}", r.name),
            Lang::Ja => format!("シナリオ {}", r.name),
        };
        let _ = writeln!(o, "<template id=\"t-run-{k}\"><h2 class=\"dh\">{title} <span class=\"chip {}\">{}</span></h2>{}</template>", r.cls, esc(&r.word), r.steps);
    }
    for (k, d) in i.diags.iter().enumerate() {
        let _ = writeln!(o, "<template id=\"t-check-{k}\"><h2 class=\"dh\">{}</h2><pre class=\"diag\">{}</pre></template>", d.code, esc(&d.render(i.file, i.src, lang)));
    }
    let data = serde_json::json!({
        "runs": runs.iter().map(|r| lit_json(&r.lit)).collect::<Vec<_>>(),
        "checks": checks.iter().map(lit_json).collect::<Vec<_>>(),
        "through": match lang {
            Lang::En => "{k} of the {n} scenarios pass here.",
            Lang::Ja => "{n} 本のシナリオのうち {k} 本がここを通ります。",
        },
        "rules": docs.iter().map(|(r, page)| serde_json::json!({
            "name": r.name,
            "title": format!("{} v{}", r.info.rule, r.info.version),
            "file": rule_path(i, r),
            "page": page.as_ref().ok(),
            "error": page.as_ref().err().map(|e| format!("{}\n\n{e}", tr(lang, "rulec could not render this rule:", "rulec はこの規則を描けませんでした。"))),
        })).collect::<Vec<_>>(),
    });
    let json = serde_json::to_string(&data).unwrap().replace("</", "<\\/");
    let _ = write!(o, "<script type=\"application/json\" id=\"dd-data\">{json}</script>\n<script>{JS}</script>\n</body>\n</html>\n");
    o
}

const CSS: &str = r#"
:root { color-scheme: light dark; --bg: #ffffff; --fg: #1f2328; --muted: #59636e; --line: #8c959f; --node: #f6f8fa; --stroke: #59636e; --ok: #1a7f37; --bad: #cf222e; --lit: #0969da; --litbg: #ddf4ff; --sel: #9a6700; --border: #d1d9e0; --code: #eff2f5; --side: #f6f8fa; }
@media (prefers-color-scheme: dark) { :root { --bg: #0d1117; --fg: #e6edf3; --muted: #9198a1; --line: #6e7681; --node: #151b23; --stroke: #9198a1; --ok: #3fb950; --bad: #f85149; --lit: #4493f8; --litbg: #0c2d6b; --sel: #d29922; --border: #3d444d; --code: #262c36; --side: #0f141b; } }
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--fg); font: 14px/1.55 -apple-system, BlinkMacSystemFont, "Segoe UI", "Noto Sans", "Hiragino Sans", "Noto Sans JP", sans-serif; }
.app { height: 100vh; display: flex; flex-direction: column; }
code, pre, .dl, .ln { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace; }
code { font-size: .9em; background: var(--code); padding: .08em .3em; border-radius: 4px; }
header.top { padding: 12px 16px 10px; border-bottom: 1px solid var(--border); }
header.top h1 { margin: 0; font-size: 18px; }
header.top h1 code { background: none; padding: 0; font-size: 1em; }
header.top .ver { color: var(--muted); font-weight: 400; font-size: 14px; }
header.top .file { margin: 2px 0 0; color: var(--muted); font-size: 12px; }
header.top .file code { background: none; padding: 0; }
header.top .desc { margin: 6px 0 0; max-width: 72em; font-size: 13px; color: var(--muted); display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; overflow: hidden; cursor: pointer; }
header.top .desc.open { -webkit-line-clamp: unset; }
.banner { margin: 8px 0 0; padding: 6px 10px; border: 1px solid var(--bad); border-radius: 6px; color: var(--bad); font-size: 13px; }
.grid { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(13rem, 17rem) minmax(0, 1fr) minmax(18rem, 26rem); }
.side, .detail { overflow: auto; padding: 12px 14px; background: var(--side); }
.side { border-right: 1px solid var(--border); }
.detail { border-left: 1px solid var(--border); }
.side h2 { font-size: 13px; margin: 4px 0 6px; text-transform: none; }
.side section + section { margin-top: 16px; }
.note { font-size: 12px; color: var(--muted); margin: 0 0 8px; }
ul.groups, ul.checks, ul.rules, ul.legend, ul.facts, ol.steps { list-style: none; margin: 0; padding: 0; }
.group { margin: 0 0 10px; }
.gl { font-size: 12px; margin-bottom: 3px; }
.gl code { background: none; padding: 0; }
.gl.ok code { color: var(--ok); } .gl.bad code { color: var(--bad); }
.gn { color: var(--muted); font-size: 11px; }
.gn::before { content: "× "; }
.runs { display: flex; flex-wrap: wrap; gap: 4px; }
button.run { font: 12px ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; min-width: 2.2em; padding: 1px 6px; border: 1px solid var(--border); border-radius: 5px; background: var(--bg); color: var(--fg); cursor: pointer; }
button.run:hover, button.check:hover { border-color: var(--lit); }
button.run[aria-pressed="true"] { background: var(--lit); border-color: var(--lit); color: #fff; }
.picking button.run { opacity: .35; }
.picking button.run.through { opacity: 1; border-color: var(--lit); }
button.check { display: block; width: 100%; text-align: left; font: inherit; font-size: 12px; padding: 5px 7px; margin: 0 0 6px; border: 1px solid var(--border); border-radius: 6px; background: var(--bg); color: var(--fg); cursor: pointer; }
button.check[aria-pressed="true"] { border-color: var(--lit); box-shadow: 0 0 0 1px var(--lit); }
button.rule { display: block; width: 100%; text-align: left; font: inherit; font-size: 12px; padding: 5px 7px; margin: 0 0 6px; border: 1px solid var(--border); border-radius: 6px; background: var(--bg); color: var(--fg); cursor: pointer; }
button.rule:hover { border-color: var(--lit); }
button.rule code { background: none; padding: 0; }
.detail button.rule { display: inline-block; width: auto; margin: 10px 0 0; }
.sheet { position: fixed; inset: 0; z-index: 10; display: flex; flex-direction: column; background: var(--bg); }
.sheet[hidden] { display: none; }
.sheet .bar { display: flex; align-items: center; gap: 10px; padding: 8px 14px; border-bottom: 1px solid var(--border); background: var(--side); font-size: 13px; }
.sheet .what { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sheet button.close { font: inherit; font-size: 16px; line-height: 1; padding: 3px 9px; border: 1px solid var(--border); border-radius: 6px; background: var(--bg); color: var(--fg); cursor: pointer; }
.sheet button.close:hover { border-color: var(--lit); }
.sheet iframe { flex: 1; width: 100%; border: 0; }
.code { font: 600 11px ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }
.code.err { color: var(--bad); } .code.warn { color: var(--sel); }
.dl { color: var(--muted); font-size: 11px; }
.canvas { overflow: auto; padding: 14px; display: flex; flex-wrap: wrap; gap: 24px; align-content: flex-start; align-items: flex-start; outline: none; }
figure.panel { margin: 0; }
figure.panel figcaption { margin: 0 0 2px 6px; font-size: 12px; color: var(--muted); }
figure.panel figcaption code { background: none; padding: 0; }
.dd-svg { display: block; }
.dd-svg text { font: 12px ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace; fill: var(--fg); }
.n { cursor: pointer; }
.n .shape { fill: var(--node); stroke: var(--stroke); stroke-width: 1.2; }
.n .side { fill: none; stroke: var(--stroke); stroke-width: 1.2; }
.n .t.sub { fill: var(--muted); font-size: 11px; }
.n .ln { fill: var(--muted); font-size: 10px; }
.k-succeed .shape, .n.ok .shape { stroke: var(--ok); stroke-width: 2; }
.k-fail .shape, .n.bad .shape { stroke: var(--bad); stroke-width: 2; }
.k-start .shape { stroke-width: 2; }
.k-entry .shape { stroke-dasharray: 5 3; stroke-width: 1.6; }
.frame { cursor: pointer; }
.frame .shape { fill: none; stroke: var(--line); stroke-width: 1.2; stroke-dasharray: 6 4; }
.frame .ft { fill: var(--muted); }
.e path { fill: none; stroke: var(--line); stroke-width: 1.3; marker-end: url(#dd-arrow); }
.e.inner path { marker-end: none; }
.e.dash path { stroke-dasharray: 5 4; }
.e .el { fill: var(--muted); font-size: 11px; }
.ah { fill: var(--line); } .ah.lit { fill: var(--lit); }
.tick circle { fill: var(--bg); stroke: var(--bad); stroke-width: 1.2; }
.tick text { fill: var(--bad); font-size: 10px; font-weight: 700; text-anchor: middle; }
.n:focus { outline: none; } .frame:focus { outline: none; }
.n.sel .shape, .n:focus-visible .shape { stroke: var(--sel); stroke-width: 3; }
.frame.sel .shape, .frame:focus-visible .shape { stroke: var(--sel); stroke-width: 2.2; }
.running .n, .running .e, .running .frame { opacity: .22; transition: opacity .12s; }
.running .lit { opacity: 1; }
.running .n.lit .shape { stroke: var(--lit); stroke-width: 2.4; fill: var(--litbg); }
.running .frame.lit .shape { stroke: var(--lit); }
.running .e.lit path { stroke: var(--lit); stroke-width: 2.4; marker-end: url(#dd-arrow-lit); }
.running .e.lit .el { fill: var(--lit); font-weight: 600; }
.running .tick.lit circle { fill: var(--bad); }
.running .tick.lit text { fill: var(--bg); }
.count { fill: var(--lit); font-weight: 700; font-size: 11px; }
.detail h2.dh { font-size: 14px; margin: 2px 0 8px; }
.detail h2.dh code { background: none; padding: 0; font-size: 13px; }
.detail h3 { font-size: 12px; color: var(--muted); margin: 14px 0 4px; font-weight: 600; }
pre.src, pre.diag { margin: 0; padding: 8px 10px; background: var(--bg); border: 1px solid var(--border); border-radius: 6px; font-size: 12px; overflow: auto; white-space: pre; }
pre.src .ln { color: var(--muted); }
dl.facts { display: grid; grid-template-columns: max-content 1fr; gap: 3px 10px; margin: 10px 0 0; font-size: 12.5px; }
dl.facts dt { color: var(--muted); }
dl.facts dd { margin: 0; }
ul.facts li { font-size: 12.5px; margin: 2px 0; }
ol.steps li { font-size: 12.5px; padding: 3px 0; border-bottom: 1px solid var(--border); }
ol.steps .ln { color: var(--muted); font-size: 11px; display: inline-block; min-width: 2.4em; }
ol.steps code { background: none; padding: 0; }
.round { color: var(--muted); font-size: 11px; }
.chip { font: 11px ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; padding: 0 5px; border-radius: 9px; border: 1px solid var(--border); white-space: nowrap; }
.chip.ok { color: var(--ok); border-color: var(--ok); } .chip.bad { color: var(--bad); border-color: var(--bad); } .chip.cancel { color: var(--sel); border-color: var(--sel); }
p.through { font-size: 12px; color: var(--muted); margin: 12px 0 0; }
ul.legend li { display: flex; gap: 8px; align-items: center; font-size: 12.5px; margin: 5px 0; }
ul.legend svg { flex: none; }
.lg { fill: var(--node); stroke: var(--stroke); stroke-width: 1.2; }
.lg.frame { fill: none; stroke: var(--line); stroke-dasharray: 4 3; }
.lg.ok { stroke: var(--ok); stroke-width: 2; } .lg.bad { stroke: var(--bad); stroke-width: 2; }
.lg-e { stroke: var(--line); stroke-width: 1.4; fill: none; } .lg-e.dash { stroke-dasharray: 4 3; }
.lg-tick { fill: var(--bg); stroke: var(--bad); stroke-width: 1.2; }
.lg-tick-t { fill: var(--bad); font: 700 10px sans-serif; text-anchor: middle; }
.tables { padding: 16px; border-top: 1px solid var(--border); overflow: auto; }
.tables h2 { font-size: 15px; margin: 8px 0; }
.tables table { border-collapse: collapse; font-size: 12.5px; margin-bottom: 16px; }
.tables th, .tables td { border: 1px solid var(--border); padding: 4px 8px; vertical-align: top; text-align: left; }
.tables td.num { text-align: right; color: var(--muted); font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }
.tables th { background: var(--side); }
@media (max-width: 900px) {
  .app { height: auto; display: block; }
  .grid { display: block; }
  .side, .detail { border: 0; border-bottom: 1px solid var(--border); }
  .canvas { max-height: 80vh; }
}
@media print { .side, .detail, .sheet { display: none; } .grid { display: block; } }
"#;

const JS: &str = r#"
(function () {
  var data = JSON.parse(document.getElementById('dd-data').textContent);
  var canvas = document.querySelector('.canvas');
  var detail = document.getElementById('dd-detail');
  var side = document.querySelector('.side');
  var SVG = 'http://www.w3.org/2000/svg';
  var state = { node: null, run: null, check: null, rule: null };
  var sheet = document.getElementById('dd-sheet');
  var opener = null;
  function all(sel) { return Array.prototype.slice.call(document.querySelectorAll(sel)); }
  function byId(id) { return all('[data-id="' + id + '"]'); }
  function show(id) {
    var t = document.getElementById('t-' + id) || document.getElementById('t-default');
    detail.replaceChildren(t.content.cloneNode(true));
  }
  function unlight() {
    canvas.classList.remove('running');
    all('.lit').forEach(function (e) { e.classList.remove('lit'); });
    all('.count').forEach(function (e) { e.remove(); });
    all('button[aria-pressed="true"]').forEach(function (b) { b.setAttribute('aria-pressed', 'false'); });
  }
  function light(l) {
    canvas.classList.add('running');
    l.nodes.forEach(function (id) { byId(id).forEach(function (e) { e.classList.add('lit'); }); });
    l.edges.forEach(function (id) { byId(id).forEach(function (e) { e.classList.add('lit'); }); });
    l.failed.forEach(function (id) { byId(id).forEach(function (e) { var t = e.querySelector('.tick'); if (t) t.classList.add('lit'); }); });
    Object.keys(l.counts).forEach(function (id) {
      var n = l.counts[id];
      if (n < 2) return;
      byId(id).forEach(function (g) {
        if (!g.classList.contains('n')) return;
        var b = g.querySelector('.shape').getBBox();
        var t = document.createElementNS(SVG, 'text');
        t.setAttribute('class', 'count');
        t.setAttribute('x', b.x + b.width + 5);
        t.setAttribute('y', b.y + b.height - 3);
        t.textContent = '×' + n;
        g.appendChild(t);
      });
    });
  }
  function unpick() {
    all('.n.sel, .frame.sel').forEach(function (e) { e.classList.remove('sel'); });
    side.classList.remove('picking');
    all('button.run.through').forEach(function (b) { b.classList.remove('through'); });
  }
  function pick(id) {
    unpick();
    byId(id).forEach(function (e) { if (e.classList.contains('n') || e.classList.contains('frame')) e.classList.add('sel'); });
    show(id);
    if (data.runs.length) {
      var k = 0;
      data.runs.forEach(function (r, i) {
        if (r.nodes.indexOf(id) >= 0) { k++; var b = document.querySelector('button.run[data-run="' + i + '"]'); if (b) b.classList.add('through'); }
      });
      side.classList.add('picking');
      var p = document.createElement('p');
      p.className = 'through';
      p.textContent = data.through.replace('{k}', k).replace('{n}', data.runs.length);
      detail.appendChild(p);
    }
  }
  function remember() {
    var h = state.run !== null ? 'run=' + (state.run + 1) : state.check !== null ? 'check=' + (state.check + 1) : '';
    if (state.node) h += (h ? '&' : '') + 'node=' + state.node;
    if (state.rule !== null) h += (h ? '&' : '') + 'rule=' + encodeURIComponent(data.rules[state.rule].name);
    try { history.replaceState(null, '', h ? '#' + h : location.pathname + location.search); } catch (e) {}
  }
  function run(k) {
    unlight();
    state.run = k; state.check = null;
    light(data.runs[k]);
    var b = document.querySelector('button.run[data-run="' + k + '"]');
    if (b) b.setAttribute('aria-pressed', 'true');
    if (!state.node) show('run-' + k);
  }
  function check(k) {
    unlight();
    state.check = k; state.run = null;
    light(data.checks[k]);
    var b = document.querySelector('button.check[data-check="' + k + '"]');
    if (b) b.setAttribute('aria-pressed', 'true');
    if (!state.node) show('check-' + k);
  }
  function node(id) { state.node = id; pick(id); }
  function reset() { state = { node: null, run: null, check: null, rule: null }; unlight(); unpick(); show('default'); remember(); }
  // a rule's page, over this one: the page rulec doc renders, or why there is none
  function code(text) { var c = document.createElement('code'); c.textContent = text; return c; }
  function openRule(k) {
    var r = data.rules[k];
    if (!r || !sheet) return;
    if (sheet.hidden) opener = document.activeElement;
    sheet.querySelector('.what').replaceChildren(code(r.name), ' · ' + r.title + ' · ', code(r.file), ' · rulec doc');
    var err = document.createElement('pre');
    err.textContent = r.error || '';
    sheet.querySelector('iframe').srcdoc = r.page !== null ? r.page : '<!doctype html><meta charset="utf-8"><pre style="white-space:pre-wrap;font:13px ui-monospace,monospace;padding:16px">' + err.innerHTML + '</pre>';
    sheet.hidden = false;
    state.rule = k;
    sheet.querySelector('.close').focus();
  }
  function closeRule() {
    if (!sheet || sheet.hidden) return false;
    sheet.hidden = true;
    sheet.querySelector('iframe').srcdoc = '';
    state.rule = null;
    if (opener && opener.focus) opener.focus();
    return true;
  }
  if (sheet) sheet.querySelector('.close').addEventListener('click', function () { closeRule(); remember(); });
  detail.addEventListener('click', function (ev) {
    var b = ev.target.closest('button[data-rule]');
    if (b) { openRule(+b.dataset.rule); remember(); }
  });
  side.addEventListener('click', function (ev) {
    var b = ev.target.closest('button');
    if (!b) return;
    if (b.dataset.rule !== undefined) { openRule(+b.dataset.rule); remember(); return; }
    state.node = null; unpick();
    if (b.dataset.run !== undefined) { if (state.run === +b.dataset.run) { reset(); return; } run(+b.dataset.run); show('run-' + b.dataset.run); }
    if (b.dataset.check !== undefined) { if (state.check === +b.dataset.check) { reset(); return; } check(+b.dataset.check); show('check-' + b.dataset.check); }
    remember();
  });
  canvas.addEventListener('click', function (ev) {
    var g = ev.target.closest('.n, .frame');
    if (!g) { if (state.node) { state.node = null; unpick(); if (state.run !== null) show('run-' + state.run); else if (state.check !== null) show('check-' + state.check); else show('default'); remember(); } return; }
    node(g.dataset.id);
    remember();
  });
  canvas.addEventListener('keydown', function (ev) {
    var g = ev.target.closest && ev.target.closest('.n, .frame');
    if (g && (ev.key === 'Enter' || ev.key === ' ')) { ev.preventDefault(); node(g.dataset.id); remember(); }
  });
  document.addEventListener('keydown', function (ev) {
    if (ev.key !== 'Escape') return;
    if (closeRule()) remember(); else reset();
  });
  var desc = document.querySelector('header.top .desc');
  if (desc) desc.addEventListener('click', function () { desc.classList.toggle('open'); });
  function fromHash() {
    var h = {};
    location.hash.replace(/^#/, '').split('&').forEach(function (kv) { var p = kv.split('='); if (p[0]) h[p[0]] = decodeURIComponent(p[1] || ''); });
    if (h.run !== undefined && data.runs[+h.run - 1]) run(+h.run - 1);
    else if (h.check !== undefined && data.checks[+h.check - 1]) check(+h.check - 1);
    if (h.node && document.getElementById('t-' + h.node)) node(h.node);
    else if (h.run === undefined && h.check === undefined) show('default');
    for (var k = 0; k < data.rules.length; k++) { if (data.rules[k].name === h.rule) openRule(k); }
  }
  fromHash();
})();
"#;
