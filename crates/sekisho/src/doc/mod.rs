//! `sekisho doc` (DESIGN 7): the page for people — who reviews who may do what, who owns the terms
//! the rules and the dates hold, who audits where the Cedar came from. This module puts together
//! what the page says, in one language, as blocks; `markdown.rs` and `html.rs` write the blocks
//! out.
//!
//! What the page says is what the check found of a file that passes (the walk of every
//! combination, `checks::Report`), the Cedar the file compiles to, and what the other languages
//! answer through ritsu's ports: the page `rulec doc` draws for a rule a computed value reads, and
//! the page `koyomi doc` draws for a dates file or a calendar. The page holds, in order:
//!
//! 1. the head: the `.gate` with its digest, sekisho's version, the namespace, and the digest of
//!    each file `gen --target cedar` writes from it;
//! 2. each action's table, the rows merged (`table.rs`): those that allow, then what the forbids
//!    deny, with the rows that deny folded;
//! 3. each policy as the `.gate` writes it beside the Cedar it compiles to, with what it decides;
//! 4. each computed value: what computes it, its values, and the rule's or the date's page;
//! 5. the expectations, the separations, and how far each role alone is allowed each action;
//! 6. each workflow: the actions it is allowed and, given what its flow calls (`FlowCalls`, from
//!    dandori's answer), each call with the action that guards it;
//! 7. the operations each action guards, by their references, and the actions that guard none;
//!
//! and last what `sekisho check` warns of, when it warns.

pub mod html;
pub mod markdown;

use crate::check::Outcome;
use crate::checks::Checked;
use crate::model::*;
use crate::names::Scope;
use crate::suite::Suite;
use crate::walk::{Domain, FactorKind, Kn, Slot, Space, Val};
use ritsu_base::naming::{Name, Tool};
use ritsu_base::text::{Lang, Text, count};
use ritsu_ports::{Allowance, Asker, Found, Said};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The two forms of the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// What GitHub shows as it is; the pages of the rules and the dates are folded sections.
    Markdown,
    /// One HTML file that reads nothing from outside; the pages of the rules and the dates open
    /// over it.
    Html,
}

/// One call of an operation in a workflow's flow, as dandori answers it through the port of flows
/// (`Flows::operation_calls`): the line, the task, the operation as a reference from the root, and
/// the error the task declares for a denial.
pub use ritsu_ports::OperationCall as Call;

/// What the flow of one workflow of the gate calls, for the part of the page on workflows
/// ([`calls_of`], where dandori is joined: `ritsu sekisho doc`); without it, the page says what
/// each workflow is allowed and no more.
#[derive(Clone, Debug)]
pub struct FlowCalls {
    /// The workflow, by its index in the gate's.
    pub workflow: usize,
    /// The calls, or what dandori said when it could not read the flow.
    pub calls: Result<Vec<Call>, Vec<Said>>,
}

/// A piece of the page. Text in a block may hold `code` in backquotes; the writers turn it into
/// their own form.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    /// A heading of level 2 to 4, and the id the HTML gives it.
    Heading { level: u8, text: String, id: String },
    Para(String),
    /// A paragraph set apart (GitHub's `[!NOTE]`).
    Note(String),
    List(Vec<String>),
    /// A table: its head, its rows, and how each row reads (allows, denies, or neither).
    Table { head: Vec<String>, rows: Vec<Vec<String>>, marks: Vec<Mark> },
    /// Blocks folded under a summary, closed until the reader opens them.
    Fold { summary: String, body: Vec<Block> },
    /// Lines of a file as they are: `gate`, `cedar` or `text`, under a caption.
    Code { lang: &'static str, caption: String, text: String },
    /// Blocks side by side where the page is wide enough (a policy's `.gate` and its Cedar).
    Side(Vec<Block>),
    /// The page of a rule, a dates file or a calendar, by its index in [`Page::embedded`].
    Embed(usize),
}

/// How a row of a table reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Allow,
    Deny,
    Plain,
}

/// The page of another language that the page holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Embedded {
    /// `rulec` or `koyomi`: the language whose `doc` drew it.
    pub tool: &'static str,
    /// The file, from the root.
    pub file: String,
    /// The page, in the form of the page that holds it, or what the language said when it could
    /// not draw one.
    pub page: Result<String, String>,
}

/// The page: its language, its title, the `.gate` it is drawn from (from the root) with the first
/// sixteen hex digits of its SHA-256, the blocks in order, and the pages of the other languages it
/// holds.
#[derive(Clone, Debug)]
pub struct Page {
    pub lang: Lang,
    pub title: String,
    pub description: Option<String>,
    pub file: String,
    pub sha: String,
    pub blocks: Vec<Block>,
    pub embedded: Vec<Embedded>,
}

/// The page of a file that passes its check, drawn with the languages `suite` joins, in `lang`,
/// for `format`; `calls` is what the workflows' flows call, when whoever draws it has dandori's
/// answer. None when the file does not pass (it has no walk to show).
pub fn page(o: &Outcome, suite: &Suite, lang: Lang, format: Format, calls: Option<&[FlowCalls]>) -> Option<Page> {
    let (Some(scope), Some(checked)) = (o.scope.as_ref(), o.walked.as_ref()) else { return None };
    if o.has_errors() || checked.report.spaces.iter().any(Option::is_none) {
        return None;
    }
    let mut b = Builder::new(o, scope, checked, suite, lang, format);
    b.head();
    b.actions();
    b.policies();
    b.computed();
    b.expectations();
    b.workflows(calls);
    b.operations();
    b.warnings();
    Some(b.done())
}

/// What the flows of the gate's workflows call, as dandori answers it through the port of flows
/// (`Flows::operation_calls`, read with the rules, the dates files and the books the suite joins),
/// for the part of the page on workflows. None where dandori is not joined (the binary of sekisho's
/// own crate). A flow outside the root, whose operations no reference names, is said to be.
pub fn calls_of(o: &Outcome, suite: &Suite) -> Option<Vec<FlowCalls>> {
    let flows = suite.flows.as_ref()?;
    let ports = ritsu_ports::Ports { rules: suite.rules.clone()?, dates: suite.dates.clone()?, books: suite.books.clone()? };
    let g = &o.walked.as_ref()?.gate;
    let calls = g
        .workflows
        .iter()
        .enumerate()
        .map(|(wi, w)| {
            let calls = match g.from_root(&w.file) {
                Some(rel) => flows.operation_calls(&g.root, &rel, &ports).map(|(_, cs)| cs),
                None => Err(vec![Said { code: String::new(), file: w.flow.clone(), line: None, message: tr!("`{}` はルートの外にあります", "`{}` is outside the root", w.flow) }]),
            };
            FlowCalls { workflow: wi, calls }
        })
        .collect();
    Some(calls)
}

/// The page written out in its form.
pub fn write(p: &Page, format: Format) -> String {
    match format {
        Format::Markdown => markdown::write(p),
        Format::Html => html::write(p),
    }
}

/// `s` in backquotes, as code in the page's text.
fn code(s: &str) -> String {
    format!("`{s}`")
}

/// A name as the page shows it: the name, and its alias when Cedar calls it otherwise
/// (`返金する`（`refund_order`）).
fn named(lang: Lang, n: &Named) -> String {
    if n.name == n.alias {
        code(&n.name)
    } else if lang == Lang::Ja {
        format!("{}（{}）", code(&n.name), code(&n.alias))
    } else {
        format!("{} ({})", code(&n.name), code(&n.alias))
    }
}

/// A list of items, as the language joins them.
fn joined(lang: Lang, items: &[String]) -> String {
    items.join(if lang == Lang::Ja { "、" } else { ", " })
}

/// Items as a sentence lists them: `a と b`, `a、b、c`; `a and b`, `a, b and c`.
fn and_list(lang: Lang, items: &[String]) -> String {
    match (lang, items.len()) {
        (Lang::Ja, 2) => format!("{} と {}", items[0], items[1]),
        (Lang::Ja, _) => items.join("、"),
        (Lang::En, _) => Text::list(&items.iter().map(Text::same).collect::<Vec<_>>()).en,
    }
}

/// A count of combinations: `1,056`.
fn n(x: u128) -> String {
    count(x.min(u64::MAX as u128) as u64)
}

/// `combination` or `combinations`.
fn combos(x: u128) -> &'static str {
    if x == 1 { "combination" } else { "combinations" }
}

/// An id of the HTML from a name: ASCII letters, digits and `-`.
fn id_of(prefix: &str, alias: &str) -> String {
    let s: String = alias.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    format!("{prefix}-{s}")
}

/// What one policy decides of one action's combinations.
#[derive(Clone, Copy, Debug, Default)]
struct PolStat {
    /// The combinations it applies to.
    holds: u128,
    /// Those it is among the deciding policies of, and those it decides alone.
    decides: u128,
    alone: u128,
    /// A forbid: those a permit applies to as well (and Cedar denies, since the forbid wins).
    overturns: u128,
}

/// What one action's walk comes to, for the page.
#[derive(Clone, Debug, Default)]
struct ActStat {
    combinations: u128,
    allowed: u128,
    /// Denied with a forbid that applies, and denied with no permit that applies (and no forbid).
    by_forbid: u128,
    no_permit: u128,
    /// By the index of the policy in the file.
    policies: BTreeMap<usize, PolStat>,
}

fn stats(s: &Space) -> ActStat {
    let mut a = ActStat::default();
    s.each(|c| {
        a.combinations += 1;
        if c.decision.allow {
            a.allowed += 1;
        } else if (0..s.policies.len()).any(|k| c.applies[k] && !s.permit[k]) {
            a.by_forbid += 1;
        } else {
            a.no_permit += 1;
        }
        let permit_applies = (0..s.policies.len()).any(|k| c.applies[k] && s.permit[k]);
        for (k, &pi) in s.policies.iter().enumerate() {
            if !c.applies[k] {
                continue;
            }
            let p = a.policies.entry(pi).or_default();
            p.holds += 1;
            if !s.permit[k] && permit_applies {
                p.overturns += 1;
            }
            if c.decision.determining.contains(&k) {
                p.decides += 1;
                if c.decision.determining.len() == 1 {
                    p.alone += 1;
                }
            }
        }
    });
    a
}

/// Which combinations of an action are an asker's (as `checks::allowance` picks them), and how many
/// of them are allowed.
fn asker_counts(g: &Gate, s: &Space, asker: &Asker) -> (u128, u128) {
    let pick = |f: &crate::walk::Frame, env: &[Val]| -> bool {
        match asker {
            Asker::Workflow(name) => {
                let Some(w) = g.workflows.iter().position(|x| x.named.is(name)) else { return false };
                g.types[f.principal].kind == Kind::Workflow && f.place(&Slot::Workflow).is_none_or(|at| env[at] == Val::Workflow(w))
            }
            Asker::Roles { ty, roles } => {
                let Some(t) = g.types.iter().position(|x| x.named.is(ty)) else { return false };
                let mask = roles.iter().filter_map(|r| g.roles.iter().position(|x| x.named.is(r))).fold(0u128, |m, r| m | crate::walk::bit(r));
                f.principal == t && matches!(&env[0], Val::Roles { direct, .. } if *direct == mask & f.walked_roles)
            }
        }
    };
    let (mut picked, mut allowed) = (0u128, 0u128);
    s.each(|c| {
        if pick(&s.frames[c.frame], c.env) {
            picked += 1;
            if c.decision.allow {
                allowed += 1;
            }
        }
    });
    (picked, allowed)
}

/// The words of an answer of `checks::allowance`.
fn allowance_word(lang: Lang, a: &Found<Allowance>) -> String {
    match a {
        Found::Value(Allowance::Always) => tr!("いつも許す", "always").get(lang).to_string(),
        Found::Value(Allowance::Sometimes { .. }) => tr!("組み合わせによる", "sometimes").get(lang).to_string(),
        Found::Value(Allowance::Never) => tr!("許さない", "never").get(lang).to_string(),
        Found::Undecided(_) => tr!("決められない", "undecided").get(lang).to_string(),
    }
}

/// What puts the page together.
struct Builder<'a> {
    o: &'a Outcome,
    scope: &'a Scope,
    checked: &'a Checked,
    g: &'a Gate,
    suite: &'a Suite,
    lang: Lang,
    format: Format,
    blocks: Vec<Block>,
    embedded: Vec<Embedded>,
    stats: Vec<ActStat>,
    /// The Cedar of each policy, by its index in the file, laid out as `cedar format` lays it out.
    cedar: BTreeMap<usize, String>,
}

impl<'a> Builder<'a> {
    fn new(o: &'a Outcome, scope: &'a Scope, checked: &'a Checked, suite: &'a Suite, lang: Lang, format: Format) -> Builder<'a> {
        let g = &checked.gate;
        let stats = checked.report.spaces.iter().map(|s| s.as_ref().map(stats).unwrap_or_default()).collect();
        let shape = crate::cedar::shape(g, scope, checked);
        let set = crate::cedar::policies(g, scope, &shape);
        let mut cedar = BTreeMap::new();
        for (pi, _) in g.policies.iter().enumerate() {
            let id = crate::cedar::policy_id(g, pi);
            if let Some(p) = set.policies.iter().find(|p| p.id == id)
                && let Ok(text) = ritsu_base::cedar::write_policy(p)
                && let Ok(laid) = ritsu_base::cedar::format_policies(&text, 80, 2)
            {
                cedar.insert(pi, laid.trim_end().to_string());
            }
        }
        Builder { o, scope, checked, g, suite, lang, format, blocks: Vec::new(), embedded: Vec::new(), stats, cedar }
    }

    fn done(self) -> Page {
        let f = self.scope.file();
        let title = format!("{} {}", f.name.text, f.version);
        Page {
            lang: self.lang,
            title,
            description: f.description.as_ref().map(|(d, _)| d.clone()),
            file: self.shown_path(&self.g.file),
            sha: ritsu_base::sha256::short(f.src.as_bytes()),
            blocks: self.blocks,
            embedded: self.embedded,
        }
    }

    /// `tr!` in the page's language.
    fn t(&self, t: Text) -> String {
        t.get(self.lang).to_string()
    }

    fn ja(&self) -> bool {
        self.lang == Lang::Ja
    }

    /// A description as a sentence: ending with the full stop of the language it is written in
    /// (an English description on a Japanese page ends with `.`).
    fn sentence(&self, d: &str) -> String {
        let d = d.trim_end();
        if d.ends_with(['.', '!', '?', '。', '！', '？']) {
            d.to_string()
        } else if d.chars().last().is_some_and(|c| c.is_ascii()) {
            format!("{d}.")
        } else {
            format!("{d}。")
        }
    }

    /// `next` after what a role's item says so far: right after its name when there is no
    /// description (`clerk: It includes …`), else as the next sentence.
    fn and_then_role(&self, s: &mut String, next: &str) {
        if s.ends_with('：') || s.ends_with(": ") {
            s.push_str(next);
        } else {
            self.and_then(s, next);
        }
    }

    /// `next` after the sentences of `s`: a space between them in English, and in Japanese after a
    /// sentence that ends in English.
    fn and_then(&self, s: &mut String, next: &str) {
        if !self.ja() || s.chars().last().is_some_and(|c| c.is_ascii()) {
            s.push(' ');
        }
        s.push_str(next);
    }

    fn push(&mut self, b: Block) {
        self.blocks.push(b);
    }

    fn heading(&mut self, level: u8, text: String, id: String) {
        self.push(Block::Heading { level, text, id });
    }

    /// A file the gate reads, as the page names it: from the root, else as given.
    fn shown_path(&self, file: &str) -> String {
        self.g.from_root(Path::new(file)).unwrap_or_else(|| file.to_string())
    }

    /// A file a `use` line reads, from the root (as the references of the page write it), else as
    /// the line writes it.
    fn use_path(&self, u: usize) -> String {
        match self.g.uses.get(u) {
            Some(x) => self.g.from_root(&x.file).unwrap_or_else(|| x.path.clone()),
            None => String::new(),
        }
    }

    fn action_name(&self, ai: usize) -> String {
        named(self.lang, &self.g.actions[ai].named)
    }

    fn action_names(&self, xs: &[usize]) -> String {
        let v: Vec<String> = xs.iter().map(|&a| code(&self.g.actions[a].named.name)).collect();
        joined(self.lang, &v)
    }

    // -----------------------------------------------------------------------------------------
    // 1. The head

    fn head(&mut self) {
        let f = self.scope.file();
        let version = env!("CARGO_PKG_VERSION");
        let file = self.shown_path(&self.g.file);
        let sha = ritsu_base::sha256::short(f.src.as_bytes());
        let mut facts = vec![
            self.t(tr!("ファイル：{}（gate {} {}、sha256:{sha}）", "File: {} (gate {} {}, sha256:{sha})", code(&file), f.name.text, f.version)),
            self.t(tr!("Cedar の名前空間：{}", "Cedar namespace: {}", code(&self.g.namespace))),
        ];
        facts.push(self.t(tr!("sekisho：{version}", "sekisho: {version}")));
        self.push(Block::List(facts));
        self.push(Block::Para(self.t(tr!(
            "上のファイルと、それが読むファイルを sekisho {version} で検査して作ったページです。ファイルのハッシュが今のものと違えば、このページは古くなっています。",
            "sekisho {version} made this page by checking the file above and the files it reads. If the file's digest is no longer what it says here, the page is out of date."
        ))));
        // the files `gen --target cedar` writes, in the page's language (sekisho writes the @doc of
        // what it computes, and the head, in it)
        let shown = ritsu_emit::header::file_name(&self.o.path);
        if let Ok(files) = crate::cedar::files(self.scope, self.checked, &shown, self.lang) {
            let lang = self.lang.code();
            self.push(Block::Para(self.t(tr!(
                "このファイルから `sekisho gen --target cedar --lang {lang}` が書く Cedar：",
                "The Cedar `sekisho gen --target cedar --lang {lang}` writes from it:"
            ))));
            let rows: Vec<Vec<String>> = files.paths(self.g.named.alias.as_str()).iter().map(|(p, text)| vec![code(p), code(&ritsu_base::sha256::short(text.as_bytes()))]).collect();
            let marks = vec![Mark::Plain; rows.len()];
            self.push(Block::Table { head: vec![self.t(tr!("ファイル", "File")), self.t(tr!("SHA-256（先頭 16 桁）", "SHA-256 (first 16 digits)"))], rows, marks });
        }
        self.push(Block::Note(self.summary()));
    }

    /// What the check found, in a few sentences.
    fn summary(&self) -> String {
        let r = &self.checked.report;
        let g = self.g;
        let total: u128 = r.actions.iter().map(|a| a.combinations).sum();
        let acts = g.actions.len();
        let mut out = vec![if acts == 1 {
            self.t(tr!(
                "`sekisho check` は、action の組み合わせ {} 通りをすべて数え、どれも Cedar と同じ決まりで、許すか拒むかを決めました。",
                "`sekisho check` walked all {} {} of the action, and decided each as Cedar does.",
                n(total);
                n(total),
                combos(total)
            ))
        } else {
            self.t(tr!(
                "`sekisho check` は、{acts} つの action の組み合わせ {} 通りをすべて数え、どれも Cedar と同じ決まりで、許すか拒むかを決めました。",
                "`sekisho check` walked all {} {} of the {acts} actions, and decided each as Cedar does.",
                n(total);
                n(total),
                combos(total)
            ))
        }];
        // an expectation that picks nothing holds too, and checks nothing (W304); one that does not
        // hold in a file that passes is one the check could not decide (W303)
        let held = r.expects.iter().filter(|e| e.broken == 0).count();
        let empty = r.expects.iter().filter(|e| e.picked == 0).count();
        let exps = g.expects.len();
        if exps > 0 {
            out.push(match (held == exps, exps) {
                (true, 1) => self.t(tr!("期待は成り立ちます。", "The expectation holds.")),
                (true, _) => self.t(tr!("{exps} つの期待は、どれも成り立ちます。", "Each of the {exps} expectations holds.")),
                (false, _) => self.t(tr!("{exps} つの期待のうち {held} つが成り立ち、ほかは成り立つかを決められません。", "{held} of the {exps} expectations hold, and the rest are undecided.")),
            });
            if empty > 0 {
                out.push(match (empty, exps) {
                    (1, 1) => self.t(tr!("ただし、どの組み合わせも選ばないので、何も確かめていません。", "It picks no combination, though, and checks nothing.")),
                    (1, _) => self.t(tr!("そのうち 1 つは、どの組み合わせも選ばないので、何も確かめていません。", "One of them picks no combination, and checks nothing.")),
                    _ => self.t(tr!("そのうち {empty} つは、どの組み合わせも選ばないので、何も確かめていません。", "{empty} of them pick no combination, and check nothing.")),
                });
            }
        }
        let seps = g.separates.len();
        if seps > 0 {
            let ok = r.separations.iter().filter(|s| **s == Some(true)).count();
            out.push(if ok == seps {
                self.t(Text::new(if seps == 1 { "職務の分離も成り立ちます。".to_string() } else { format!("{seps} つの職務の分離も、どれも成り立ちます。") }, if seps == 1 { "The separation holds too.".to_string() } else { format!("Each of the {seps} separations holds too.") }))
            } else {
                self.t(tr!("{seps} つの職務の分離のうち {ok} つが成り立ち、ほかは成り立つかを決められません。", "{ok} of the {seps} separations hold, and the rest are undecided."))
            });
        }
        let with_can = g.roles.iter().filter(|x| x.can.is_some()).count();
        if with_can > 0 {
            out.push(self.t(Text::new(
                if with_can == 1 { "`can` を書いた役割は、`can` に並べた action だけを許されます。".to_string() } else { format!("`can` を書いた {with_can} つの役割は、どれも、`can` に並べた action だけを許されます。") },
                if with_can == 1 { "The role that writes `can` is allowed the actions its `can` lists, and no other.".to_string() } else { format!("Each of the {with_can} roles that write `can` is allowed the actions its `can` lists, and no other.") },
            )));
        }
        out.join(if self.ja() { "" } else { " " })
    }

    // -----------------------------------------------------------------------------------------
    // 2. The actions

    fn actions(&mut self) {
        self.heading(2, self.t(tr!("だれが何をできるか", "Who may do what")), "actions".into());
        self.push(Block::Para(self.t(tr!(
            "action ごとに、`sekisho check` が数えた組み合わせを、Cedar の答えと決めたポリシーが同じものどうしでまとめた表です。一つの列だけが違う行は一つにまとめ、その列にはまとめた値を書きます。その列がとるすべての値をまとめたときは「どれでも」、一つを除くすべてのときは「〜以外」と書きます。「-」は、その行には当てはまらない列です（ワークフローの役割など）。",
            "For each action, the combinations `sekisho check` walked, put together where Cedar answers them alike and the same policies decide. Rows that differ in one column only are one row, the column holding their values: `any` is every value the column takes, `not …` every value but one. `-` is a column that does not apply to the row (the roles of a workflow, say)."
        ))));
        for ai in 0..self.g.actions.len() {
            self.action(ai);
        }
    }

    fn action(&mut self, ai: usize) {
        let g = self.g;
        let lang = self.lang;
        let a = &g.actions[ai];
        let decl = self.scope.file().actions.iter().find(|x| x.name.ascii() == a.named.alias);
        self.heading(3, format!("action {}", self.action_name(ai)), id_of("action", &a.named.alias));
        if let Some((d, _)) = decl.and_then(|d| d.description.as_ref()) {
            self.push(Block::Para(d.clone()));
        }
        let mut facts = Vec::new();
        let refs = a.references();
        if refs.is_empty() {
            facts.push(self.t(tr!("どの操作も守りません。", "It guards no operation.")));
        } else {
            let ops: Vec<String> = refs.iter().map(|(r, _)| code(&r.text())).collect();
            facts.push(self.t(tr!("守る操作：{}", "It guards {}.", joined(lang, &ops); Text::list(&ops.iter().map(Text::same).collect::<Vec<_>>()).en)));
        }
        let types = |xs: &[usize]| -> String { joined(lang, &xs.iter().map(|&t| code(&g.types[t].named.name)).collect::<Vec<_>>()) };
        let (ps, rs) = (types(&a.principals), types(&a.resources));
        facts.push(match &a.from {
            Some((arg, _)) => self.t(tr!("principal の型：{ps}。resource の型：{rs}（ID は操作の引数 {} から読む）", "Principals: {ps}. Resources: {rs}, the id read from the operation's argument {}.", code(arg))),
            None => self.t(tr!("principal の型：{ps}。resource の型：{rs}", "Principals: {ps}. Resources: {rs}.")),
        });
        for inp in &a.inputs {
            facts.push(self.t(tr!("input {}：{}", "Input {}: {}.", named(lang, &inp.named), field_type(lang, g, inp); named(lang, &inp.named), field_type(lang, g, inp))));
        }
        if !a.computed.is_empty() {
            let cs: Vec<String> = a.computed.iter().map(|c| code(&c.named.name)).collect();
            facts.push(self.t(tr!("計算した値：{}（下の「計算した値」）", "Computed: {} (under Computed values below).", joined(lang, &cs))));
        }
        if let Some((why, _)) = &a.nobody {
            facts.push(self.t(tr!("だれにも許さない：{why}", "Allowed to no one: {why}.")));
        }
        let st = self.stats[ai].clone();
        facts.push(match (st.combinations, st.allowed) {
            (1, 1) => self.t(tr!("組み合わせは 1 通りで、それを許します。", "1 combination, allowed.")),
            (1, _) => self.t(tr!("組み合わせは 1 通りで、それを拒みます。", "1 combination, denied.")),
            _ => self.t(tr!(
                "組み合わせは {} 通りで、そのうち {} 通りを許します。",
                "{} {}, {} of them allowed.",
                n(st.combinations), n(st.allowed);
                n(st.combinations), combos(st.combinations), n(st.allowed)
            )),
        });
        self.push(Block::List(facts));
        let Some(s) = self.checked.report.spaces[ai].as_ref() else { return };
        if s.inexact() {
            self.push(Block::Note(self.t(tr!(
                "この action の組み合わせには、ほかの言語が起こるかどうかを言えなかった値を、起こるかもしれないものとして数えたものがあります。下の表にもその行が入っています（「検査の警告」）。",
                "Some combinations of this action rest on a value another language could not say can happen, and are counted in case it can; the tables below hold them too (see What check warns of)."
            ))));
        }
        let t = crate::table::table(g, s);
        let policies_of = |pol: &[usize]| -> String {
            if pol.is_empty() {
                return tr!("当てはまる permit なし", "no permit applies").get(lang).to_string();
            }
            joined(lang, &pol.iter().map(|p| g.policies[*p].named.name.clone()).collect::<Vec<_>>())
        };
        let mut head: Vec<String> = t.columns.iter().map(|c| c.get(lang).to_string()).collect();
        head.push(self.t(tr!("決めたポリシー", "Deciding policies")));
        let rows_of = |allow: bool| -> Vec<Vec<String>> {
            t.rows
                .iter()
                .filter(|r| r.allow == allow)
                .map(|r| {
                    let mut cells: Vec<String> = r.cells.iter().map(|c| c.get(lang).to_string()).collect();
                    cells.push(policies_of(&r.policies));
                    cells
                })
                .collect()
        };
        let aid = id_of("action", &a.named.alias);
        self.heading(4, self.t(tr!("許す組み合わせ", "Allowed")), format!("{aid}-allowed"));
        let allowed = rows_of(true);
        if allowed.is_empty() {
            self.push(Block::Para(self.t(tr!("許す組み合わせはありません。", "No combination is allowed."))));
        } else {
            let marks = vec![Mark::Allow; allowed.len()];
            self.push(Block::Table { head: head.clone(), rows: allowed, marks });
        }
        self.heading(4, self.t(tr!("拒む組み合わせ", "Denied")), format!("{aid}-denied"));
        let denied = rows_of(false);
        let mut lines = Vec::new();
        for &pi in &g.policies_on(ai) {
            let p = &g.policies[pi];
            if p.permit {
                continue;
            }
            let ps = st.policies.get(&pi).copied().unwrap_or_default();
            let when = self.policy_lines(pi);
            lines.push(self.t(tr!(
                "{}（{when}）は {} 通りに当てはまり、どれも拒みます。そのうち {} 通りには permit も当てはまりますが、forbid が勝ちます。",
                "{} ({when}) applies to {} {}, and denies each; a permit applies to {} of them too, and the forbid wins.",
                code(&p.named.name), n(ps.holds), n(ps.overturns);
                code(&p.named.name), n(ps.holds), combos(ps.holds), n(ps.overturns)
            )));
        }
        if st.no_permit > 0 && lines.is_empty() {
            lines.push(self.t(tr!(
                "{} 通りは、どの permit も当てはまらないので拒みます。",
                "No permit applies to the {} {} that {} denied.",
                n(st.no_permit);
                n(st.no_permit), combos(st.no_permit), if st.no_permit == 1 { "is" } else { "are" }
            )));
        } else if st.no_permit > 0 {
            lines.push(self.t(tr!(
                "残る {} 通りは、どの permit も当てはまらないので拒みます。",
                "No permit applies to the {} other {} that {} denied.",
                n(st.no_permit);
                n(st.no_permit), combos(st.no_permit), if st.no_permit == 1 { "is" } else { "are" }
            )));
        }
        if denied.is_empty() {
            self.push(Block::Para(self.t(tr!("拒む組み合わせはありません。", "No combination is denied."))));
            return;
        }
        if !lines.is_empty() {
            self.push(Block::List(lines));
        }
        let rows = denied.len();
        let marks = vec![Mark::Deny; rows];
        self.push(Block::Fold {
            summary: self.t(Text::new(format!("拒む行の表（{rows} 行）"), if rows == 1 { "The 1 row that denies".to_string() } else { format!("The {rows} rows that deny") })),
            body: vec![Block::Table { head, rows: denied, marks }],
        });
    }

    /// A policy's `principal`, `when` and `unless` lines, joined as one: what picks its combinations.
    fn policy_lines(&self, pi: usize) -> String {
        let p = &self.g.policies[pi];
        let mut parts: Vec<String> = Vec::new();
        if let Some(pol) = self.policy_decl(p)
            && let Some((who, _)) = &pol.body.principal
        {
            parts.push(code(&who_text(who)));
        }
        for c in &p.conds {
            parts.push(code(&format!("{} {}", if c.when { "when" } else { "unless" }, c.text)));
        }
        if parts.is_empty() {
            return self.t(tr!("この action のすべての組み合わせ", "every combination of the action"));
        }
        joined(self.lang, &parts)
    }

    /// The `.gate` that writes a policy, and the policy there: the file's own, or the file a forbid
    /// is read from.
    fn policy_file(&self, p: &Policy) -> Option<&'a crate::ast::File> {
        match &p.from {
            None => Some(self.scope.file()),
            Some((alias, _)) => self.scope.files.iter().find(|f| f.alias() == alias),
        }
    }

    fn policy_decl(&self, p: &Policy) -> Option<&'a crate::ast::Policy> {
        self.policy_file(p)?.policies.iter().find(|x| x.name.ascii() == p.named.alias)
    }

    // -----------------------------------------------------------------------------------------
    // 3. The policies

    fn policies(&mut self) {
        let g = self.g;
        let lang = self.lang;
        self.heading(2, self.t(tr!("ポリシー", "Policies")), "policies".into());
        let mut intro = self.t(tr!(
            "ポリシーごとに、`.gate` に書いた行と、`sekisho gen --target cedar` が生成する Cedar を並べます。permit は、当てはまる組み合わせを許します。forbid は、当てはまる組み合わせを拒み、permit も当てはまるときは forbid が勝ちます。どのポリシーも当てはまらない組み合わせは、Cedar が拒みます。",
            "Each policy as the `.gate` writes it, beside the Cedar `sekisho gen --target cedar` generates for it. A permit allows the combinations it applies to; a forbid denies those it applies to, and wins where a permit applies too. Cedar denies a combination no policy applies to."
        ));
        if let Some((rule_name, cedar)) = self.rule_value_example() {
            intro.push_str(if self.ja() { "" } else { " " });
            intro.push_str(&self.t(tr!(
                "規則の答えを比べる条件は、`.gate` では `.rule` に書いた値の名前（{}）で、Cedar では Cedar に渡す文字列（{}）で書きます。",
                "A condition on a rule's answer names the value as the `.rule` writes it ({}) in the `.gate`, and as the string Cedar is given ({}) in the Cedar.",
                code(&rule_name), code(&format!("\"{cedar}\""))
            )));
        }
        self.push(Block::Para(intro));
        for pi in 0..g.policies.len() {
            let p = &g.policies[pi];
            if p.actions.is_empty() {
                continue;
            }
            let effect = if p.permit { "permit" } else { "forbid" };
            self.heading(3, format!("{effect} {}", named(lang, &p.named)), id_of("policy", &crate::cedar::policy_id(g, pi).replace('/', "-")));
            let decl = self.policy_decl(p);
            if let Some((d, _)) = decl.and_then(|d| d.body.description.as_ref()) {
                self.push(Block::Para(d.clone()));
            }
            let mut lines = Vec::new();
            if let Some((_, file)) = &p.from {
                lines.push(self.t(tr!(
                    "{} に書かれた forbid で、`use gate` で読んでいます。",
                    "A forbid written in {}, read with `use gate`.",
                    code(&self.shown_path(file))
                )));
            }
            for &ai in &p.actions {
                let ps = self.stats[ai].policies.get(&pi).copied().unwrap_or_default();
                let an = code(&g.actions[ai].named.name);
                lines.push(if p.permit && ps.alone == ps.decides && ps.decides == 1 {
                    self.t(tr!("{an} では 1 通りを許します。ほかの permit は、それを許しません。", "On {an}, it allows 1 combination, which no other permit allows."))
                } else if p.permit && ps.alone == ps.decides {
                    self.t(tr!(
                        "{an} では {} 通りを許します。どれも、ほかの permit は許しません。",
                        "On {an}, it allows {} {}, and no other permit allows any of them.",
                        n(ps.decides);
                        n(ps.decides), combos(ps.decides)
                    ))
                } else if p.permit && ps.alone == 0 && ps.decides == 1 {
                    self.t(tr!("{an} では 1 通りを許します。ほかの permit も、それを許します。", "On {an}, it allows 1 combination, which another permit allows too."))
                } else if p.permit && ps.alone == 0 {
                    self.t(tr!(
                        "{an} では {} 通りを許します。どれも、ほかの permit も許します。",
                        "On {an}, it allows {} {}, and another permit allows each of them too.",
                        n(ps.decides);
                        n(ps.decides), combos(ps.decides)
                    ))
                } else if p.permit {
                    self.t(tr!(
                        "{an} では {} 通りを許します。そのうち {} 通りは、ほかの permit が許しません。",
                        "On {an}, it allows {} {}; no other permit allows {} of them.",
                        n(ps.decides), n(ps.alone);
                        n(ps.decides), combos(ps.decides), n(ps.alone)
                    ))
                } else {
                    self.t(tr!(
                        "{an} では {} 通りに当てはまり、どれも拒みます。そのうち {} 通りには permit も当てはまります。",
                        "On {an}, it applies to {} {} and denies them; a permit applies to {} of them too.",
                        n(ps.holds), n(ps.overturns);
                        n(ps.holds), combos(ps.holds), n(ps.overturns)
                    ))
                });
            }
            self.push(Block::List(lines));
            let mut side = Vec::new();
            if let Some(d) = decl
                && let Some(f) = self.policy_file(p)
            {
                let src: Vec<&str> = f.src.lines().collect();
                let (first, last) = d.lines;
                let text = src.get(first.saturating_sub(1)..last.min(src.len())).map(|ls| ls.join("\n")).unwrap_or_default();
                side.push(Block::Code { lang: "gate", caption: self.shown_path(&f.path), text });
            }
            if let Some(c) = self.cedar.get(&pi) {
                side.push(Block::Code { lang: "cedar", caption: format!("cedar/{}.cedar", g.named.alias), text: c.clone() });
            }
            if !side.is_empty() {
                self.push(Block::Side(side));
            }
        }
    }

    /// A condition of a policy on a value of a rule's enum, for the sentence that says the `.gate`
    /// and the Cedar write it differently: the rule's name of the value, and the string Cedar is
    /// given.
    fn rule_value_example(&self) -> Option<(String, String)> {
        fn atoms<'e>(e: &'e Expr, out: &mut Vec<&'e Atom>) {
            match e {
                Expr::Atom(a) => out.push(a),
                Expr::Not(x) => atoms(x, out),
                Expr::And(xs) | Expr::Or(xs) => xs.iter().for_each(|x| atoms(x, out)),
            }
        }
        for p in &self.g.policies {
            for c in &p.conds {
                let mut found = Vec::new();
                atoms(&c.expr, &mut found);
                for at in found {
                    let Atom::Is(crate::model::Path::Value(v), Literal::Word(w)) = at else { continue };
                    for &ai in &p.actions {
                        let Some((ci, _)) = self.g.actions[ai].computed_value(v) else { continue };
                        if let Some(Kn::Rule { domain: Domain::Enum(vs), .. }) = self.checked.known.computed.get(&(ai, ci))
                            && let Some(x) = vs.iter().find(|x| x.is(w))
                        {
                            return Some((x.name.clone(), x.alias.clone()));
                        }
                    }
                }
            }
        }
        None
    }

    // -----------------------------------------------------------------------------------------
    // 4. The computed values

    fn computed(&mut self) {
        let g = self.g;
        if g.actions.iter().all(|a| a.computed.is_empty()) {
            return;
        }
        self.heading(2, self.t(tr!("計算した値", "Computed values")), "computed".into());
        self.push(Block::Para(self.t(tr!(
            "action が計算して、Cedar の context に入れる値です。生成したコードが、サービスのデータ、操作の引数、サーバーの時計から計算し、呼ぶ側からは受け取りません。規則と日付のページは、rulec と koyomi が描いた、人が読むページをそのまま載せています。",
            "What an action computes and gives Cedar in its context. The generated code computes each one from the service's own data, the operation's arguments and the server's clock, and never takes it from the caller. The pages of the rules and the dates are the pages for people that rulec and koyomi draw, as they draw them."
        ))));
        // each file another language draws a page of, once
        let mut drawn: BTreeMap<usize, String> = BTreeMap::new();
        for ai in 0..g.actions.len() {
            let sets = self.date_sets(ai);
            for ci in 0..g.actions[ai].computed.len() {
                self.value(ai, ci, &mut drawn, &sets);
            }
        }
    }

    fn value(&mut self, ai: usize, ci: usize, drawn: &mut BTreeMap<usize, String>, sets: &[(Vec<usize>, Vec<Vec<Val>>)]) {
        let g = self.g;
        let lang = self.lang;
        let a = &g.actions[ai];
        let cv = &a.computed[ci];
        let decl = self.scope.file().actions.iter().find(|x| x.name.ascii() == a.named.alias);
        let cdecl = decl.and_then(|d| d.context.iter().find(|x| x.name.ascii() == cv.named.alias));
        let heading = if self.ja() {
            format!("{} の {}", code(&a.named.name), named(lang, &cv.named))
        } else {
            format!("{} of {}", named(lang, &cv.named), code(&a.named.name))
        };
        self.heading(3, heading, id_of("computed", &format!("{}-{}", a.named.alias, cv.named.alias)));
        let mut facts = Vec::new();
        if let Some(c) = cdecl {
            facts.push(self.t(tr!("書き方：{}", "Written: {}", code(&format!("{} = {}", c.name.text, c.text)))));
        }
        let sources = |args: &[(String, Source)]| -> Vec<String> {
            let mut v: Vec<String> = Vec::new();
            for (_, s) in args {
                let t = code(&source_text(g, a, s));
                if !v.contains(&t) {
                    v.push(t);
                }
            }
            v
        };
        let offset = g.today.as_ref().map(|t| offset_text(t.offset)).unwrap_or_default();
        // the file another language draws a page of, by its `use`
        let mut page_of: Option<(usize, &'static str)> = None;
        match &cv.how {
            How::Rule { rule, args, output } => {
                let r = Name::file(Tool::Rulec, self.use_path(*rule)).with("output", output.clone());
                let from = sources(args);
                facts.push(self.t(tr!(
                    "{} が、{} から計算します。",
                    "Computed by {}, from {}.",
                    code(&r.text()), and_list(lang, &from);
                    code(&r.text()), and_list(lang, &from)
                )));
                page_of = Some((*rule, "rulec"));
            }
            How::Date { op, of: DateOf::Call { dates, function, args } } => {
                let d = Name::file(Tool::Koyomi, self.use_path(*dates)).with("date", function.clone());
                let from = sources(args);
                let (ja, en) = compared(*op, "その日", "it");
                facts.push(self.t(tr!(
                    "{} として計算します。koyomi が {} から日付を求め、today が{ja}なら「はい」です。",
                    "Computed as {}: koyomi gives the date from {}, and the value is yes when today is {en}.",
                    code(&format!("today {} {}", op.symbol(), d.text())), and_list(lang, &from);
                    code(&format!("today {} {}", op.symbol(), d.text())), and_list(lang, &from)
                )));
                page_of = Some((*dates, "koyomi"));
            }
            How::Date { op, of: DateOf::Attr(o, attr) } => {
                let s = source_text(g, a, &Source::Attr(*o, attr.clone()));
                let (ja, en) = compared(*op, &format!(" {} の日付", code(&s)), &code(&s));
                facts.push(self.t(tr!(
                    "{} として計算します。today が{ja}なら「はい」です。",
                    "Computed as {}: yes when today is {en}.",
                    code(&format!("today {} {s}", op.symbol()))
                )));
            }
            How::Open { calendar } => {
                let c = Name::file(Tool::Koyomi, self.use_path(*calendar));
                facts.push(self.t(tr!(
                    "{} として計算します。today がそのカレンダーの営業日なら「はい」です。",
                    "Computed as {}: yes when today is a business day of the calendar.",
                    code(&format!("today is open in {}", c.text()))
                )));
                page_of = Some((*calendar, "koyomi"));
            }
        }
        if !matches!(cv.how, How::Rule { .. }) {
            facts.push(self.t(tr!(
                "today は、UTC からのオフセットが {offset} の日付です（`today` の行）。",
                "Today is the date at the offset {offset} from UTC (the line `today`)."
            )));
        }
        match self.checked.known.computed.get(&(ai, ci)) {
            Some(Kn::Rule { domain: Domain::Enum(vs), .. }) => {
                let names: Vec<String> = vs.iter().map(|v| code(&v.name)).collect();
                let strings: Vec<String> = vs.iter().map(|v| code(&format!("\"{}\"", v.alias))).collect();
                facts.push(self.t(tr!(
                    "値：{}。Cedar に渡す文字列は {} です。",
                    "Its values: {}; Cedar is given the strings {}.",
                    joined(lang, &names), joined(lang, &strings);
                    Text::list(&names.iter().map(Text::same).collect::<Vec<_>>()).en, Text::list(&strings.iter().map(Text::same).collect::<Vec<_>>()).en
                )));
            }
            _ => facts.push(self.t(tr!("値：はい、いいえ（Cedar では true と false）。", "Its values: yes and no (true and false in Cedar)."))),
        }
        if let (Some(d), Some(c)) = (decl, cdecl) {
            let (ps, rs) = self.scope.computed_for(d, c);
            let all_p = ps.len() == a.principals.len();
            let all_r = rs.len() == a.resources.len();
            let shown = |xs: &[String]| joined(lang, &xs.iter().map(|x| code(&self.type_name(x))).collect::<Vec<_>>());
            if ps.is_empty() || rs.is_empty() {
                facts.push(self.t(tr!(
                    "計算することはありません。読むものを持つ principal か resource の型が、この action にありません。",
                    "It is never computed: no principal or resource type of the action has what it reads."
                )));
            } else if !all_p || !all_r {
                let mut only = Vec::new();
                if !all_p {
                    only.push(self.t(tr!("principal の型が {}", "a principal of type {}", shown(&ps))));
                }
                if !all_r {
                    only.push(self.t(tr!("resource の型が {}", "a resource of type {}", shown(&rs))));
                }
                facts.push(if self.ja() {
                    format!("{} のときだけ計算します。ほかの型のときは、context にこの値がありません。", only.join("で、"))
                } else {
                    format!("It is computed only for {}; for another type, the context does not have it.", only.join(" and "))
                });
            }
        }
        self.push(Block::List(facts));
        // after the last of the values that read today together, what they come to together
        for (which, set) in sets {
            if which.last() == Some(&ci) {
                self.push(Block::Para(self.date_para(ai, which, set)));
            }
        }
        if let Some((u, tool)) = page_of {
            match drawn.get(&u) {
                Some(first) => {
                    let first = first.clone();
                    self.push(Block::Para(self.t(tr!("このファイルのページは、上の {first} のところに載せています。", "The page of this file is under {first} above."))));
                }
                None => {
                    drawn.insert(u, code(&cv.named.name));
                    let k = self.embed(u, tool);
                    self.push(Block::Embed(k));
                }
            }
        }
    }

    /// A type's name as the gate writes it, from its alias.
    fn type_name(&self, alias: &str) -> String {
        self.g.types.iter().find(|t| t.named.alias == alias).map(|t| t.named.name.clone()).unwrap_or_else(|| alias.to_string())
    }

    /// The page another language draws of the file of the `use` line `u`.
    fn embed(&mut self, u: usize, tool: &'static str) -> usize {
        let file = self.use_path(u);
        let path = self.g.uses[u].file.clone();
        let html = self.format == Format::Html;
        let said = |s: Vec<Said>| -> String {
            s.iter()
                .map(|x| {
                    let place = match x.line {
                        Some(l) => format!("{}:{l}", x.file),
                        None => x.file.clone(),
                    };
                    format!("{place}: {}", x.message.get(self.lang))
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let page = match tool {
            "rulec" => match self.suite.rules.as_ref() {
                Some(r) => r.doc(&path, &file, html, self.lang).map_err(said),
                None => Err(String::new()),
            },
            _ => match self.suite.dates.as_ref() {
                Some(d) => d.doc(&path, &file, html, self.lang).map_err(said),
                None => Err(String::new()),
            },
        };
        self.embedded.push(Embedded { tool, file, page });
        self.embedded.len() - 1
    }

    /// The values of an action that read today, as the walk counts them together (DESIGN 3.3): for
    /// each set of them, which values they come to together on some day, as koyomi computes every
    /// day, the truths first.
    fn date_sets(&self, ai: usize) -> Vec<(Vec<usize>, Vec<Vec<Val>>)> {
        let Some(s) = self.checked.report.spaces[ai].as_ref() else { return Vec::new() };
        let mut sets: BTreeMap<Vec<usize>, BTreeSet<Vec<(bool, Val)>>> = BTreeMap::new();
        for f in &s.frames {
            for x in &f.factors {
                if x.kind != FactorKind::Dates {
                    continue;
                }
                let at: Vec<(usize, usize)> = x.places.iter().enumerate().filter_map(|(k, &at)| if let Slot::Computed(i) = f.slots[at] { Some((k, i)) } else { None }).collect();
                if at.len() < 2 {
                    continue;
                }
                let set = sets.entry(at.iter().map(|(_, i)| *i).collect()).or_default();
                for p in &x.points {
                    // true before false, then absent
                    set.insert(at.iter().map(|(k, _)| (p.vals[*k] != Val::Bool(true), p.vals[*k].clone())).collect());
                }
            }
        }
        sets.into_iter().map(|(which, set)| (which, set.into_iter().map(|v| v.into_iter().map(|(_, x)| x).collect()).collect())).collect()
    }

    /// What the values that read today come to together, in a sentence.
    fn date_para(&self, ai: usize, which: &[usize], sets: &[Vec<Val>]) -> String {
        let lang = self.lang;
        let a = &self.g.actions[ai];
        let word = |v: &Val| -> String {
            match v {
                Val::Bool(true) => tr!("はい", "yes"),
                Val::Bool(false) => tr!("いいえ", "no"),
                _ => tr!("無い", "absent"),
            }
            .get(lang)
            .to_string()
        };
        let names: Vec<String> = which.iter().map(|&i| code(&a.computed[i].named.name)).collect();
        let en_names = Text::list(&names.iter().map(Text::same).collect::<Vec<_>>()).en;
        let all = 1usize << which.len();
        let every = sets.len() == all && sets.iter().all(|v| v.iter().all(|x| matches!(x, Val::Bool(_))));
        let shown: Vec<String> = sets.iter().map(|v| if lang == Lang::Ja { format!("（{}）", v.iter().map(word).collect::<Vec<_>>().join("、")) } else { format!("({})", v.iter().map(word).collect::<Vec<_>>().join(", ")) }).collect();
        let (both_ja, both_en) = if which.len() == 2 { ("どちらも", "both") } else { ("どれも", "all") };
        if every {
            self.t(tr!(
                "{} は、{both_ja} today を読みます。koyomi が宣言した範囲のすべての日で計算すると、値の組 {all} 通りのどれも起こります。",
                "{} {both_en} read today. Computed by koyomi on every day of the declared ranges, each of the {all} sets of their values can happen.",
                and_list(lang, &names);
                en_names
            ))
        } else {
            self.t(tr!(
                "{} は、{both_ja} today を読みます。koyomi が宣言した範囲のすべての日で計算すると、起こる値の組は {} です。",
                "{} {both_en} read today. Computed by koyomi on every day of the declared ranges, the sets of their values that can happen are {}.",
                and_list(lang, &names), joined(lang, &shown);
                en_names, shown.join(", ")
            ))
        }
    }

    // -----------------------------------------------------------------------------------------
    // 5. The expectations, the separations and the roles

    fn expectations(&mut self) {
        let g = self.g;
        let r = &self.checked.report;
        // the roles have a section of their own when there is nothing else to say here
        let alone = g.expects.is_empty() && g.separates.is_empty();
        if !alone {
            self.heading(2, self.t(tr!("期待、職務の分離、役割", "Expectations, separations and roles")), "checks".into());
        }
        if !g.expects.is_empty() {
            self.heading(3, self.t(tr!("期待", "Expectations")), "expectations".into());
            let mut items = Vec::new();
            for (ei, e) in g.expects.iter().enumerate() {
                let c = &r.expects[ei];
                let decl = self.scope.file().expects.iter().find(|x| x.name.text == e.name);
                let what = if e.allow { tr!("許すことを期待", "expects allow") } else { tr!("拒むことを期待", "expects deny") };
                let mut s = if self.ja() {
                    format!("{}（{} で{}）", code(&e.name), self.action_names(&e.actions), what.ja)
                } else {
                    format!("{} ({}, on {})", code(&e.name), what.en, self.action_names(&e.actions))
                };
                if let Some((d, _)) = decl.and_then(|d| d.body.description.as_ref()) {
                    s.push_str(if self.ja() { "：" } else { ": " });
                    s.push_str(&self.sentence(d));
                } else {
                    s.push_str(if self.ja() { "。" } else { "." });
                }
                let result = if c.picked == 0 {
                    self.t(tr!("どの組み合わせも選ばないので、成り立ちますが、何も確かめていません。", "It picks no combination: it holds, and checks nothing."))
                } else if c.broken == 0 {
                    self.t(tr!("この期待が選ぶ {} 通りのすべてで成り立ちます。", "It holds on all {} {} it picks.", n(c.picked); n(c.picked), combos(c.picked)))
                } else {
                    self.t(tr!("成り立つかを決められません。", "Whether it holds is undecided."))
                };
                self.and_then(&mut s, &result);
                items.push(s);
            }
            self.push(Block::List(items));
        }
        if !g.separates.is_empty() {
            self.heading(3, self.t(tr!("職務の分離", "Separations")), "separations".into());
            let mut items = Vec::new();
            for (si, sep) in g.separates.iter().enumerate() {
                let decl = self.scope.file().separates.iter().find(|x| x.name.text == sep.name);
                let mut s = if self.ja() { format!("{}（{}）", code(&sep.name), self.action_names(&sep.actions)) } else { format!("{} ({})", code(&sep.name), self.action_names(&sep.actions)) };
                if let Some((d, _)) = decl.and_then(|d| d.description.as_ref()) {
                    s.push_str(if self.ja() { "：" } else { ": " });
                    s.push_str(&self.sentence(d));
                } else {
                    s.push_str(if self.ja() { "。" } else { "." });
                }
                let result = match r.separations.get(si) {
                    Some(Some(true)) => self.t(tr!("成り立ちます。二つ以上を許される principal はいません。", "It holds: no principal is allowed two of them.")),
                    _ => self.t(tr!("成り立つかを決められません。", "Whether it holds is undecided.")),
                };
                self.and_then(&mut s, &result);
                items.push(s);
            }
            self.push(Block::List(items));
        }
        self.roles(alone);
    }

    /// The roles, under the heading of the section before (`alone`: as a section of their own).
    fn roles(&mut self, alone: bool) {
        let g = self.g;
        let lang = self.lang;
        // the principals who ask: each role alone, on each type that can hold it; a type that holds
        // no role, as itself
        let mut askers: Vec<(String, usize, Asker)> = Vec::new();
        for (ti, t) in g.types.iter().enumerate() {
            if t.kind != Kind::Principal {
                continue;
            }
            if t.roles.is_empty() {
                askers.push((code(&t.named.name), ti, Asker::Roles { ty: t.named.alias.clone(), roles: vec![] }));
                continue;
            }
            for &ri in &t.roles {
                let role = &g.roles[ri];
                let label = if self.ja() { format!("{} を持つ {}", code(&role.named.name), code(&t.named.name)) } else { format!("{} holding {}", code(&t.named.name), code(&role.named.name)) };
                askers.push((label, ti, Asker::Roles { ty: t.named.alias.clone(), roles: vec![role.named.alias.clone()] }));
            }
        }
        if askers.is_empty() {
            return;
        }
        self.heading(if alone { 2 } else { 3 }, self.t(tr!("役割", "Roles")), "roles".into());
        if !g.roles.is_empty() {
            let mut items = Vec::new();
            for role in &g.roles {
                let decl = self.scope.files.iter().find_map(|f| f.roles.iter().find(|x| x.name.ascii() == role.named.alias));
                let inc: Vec<String> = role.includes.iter().map(|&i| code(&g.roles[i].named.name)).collect();
                let mut s = named(lang, &role.named);
                match decl.and_then(|d| d.description.as_ref()) {
                    Some((d, _)) => {
                        s.push_str(if self.ja() { "：" } else { ": " });
                        s.push_str(&self.sentence(d));
                    }
                    None => s.push_str(if self.ja() { "：" } else { ": " }),
                }
                if !inc.is_empty() {
                    let next = self.t(tr!("{} を含みます。", "It includes {}.", and_list(lang, &inc); and_list(lang, &inc)));
                    self.and_then_role(&mut s, &next);
                }
                if let Some((can, _)) = &role.can {
                    let l: Vec<String> = can.iter().map(|&a| code(&g.actions[a].named.name)).collect();
                    let en = Text::list(&l.iter().map(Text::same).collect::<Vec<_>>()).en;
                    let next = self.t(tr!("`can` に {} を並べています。", "Its `can` lists {}.", joined(lang, &l); en));
                    self.and_then_role(&mut s, &next);
                }
                if s.ends_with('：') || s.ends_with(": ") {
                    s.truncate(s.trim_end_matches(['：', ':', ' ']).len());
                }
                items.push(s);
            }
            self.push(Block::List(items));
        }
        self.push(Block::Para(self.t(tr!(
            "役割を一つだけ持つ principal（その役割が含む役割も持つ）が、action を許されるかです。「いつも許す」はほかの値のどの組み合わせでも許すこと、「組み合わせによる」は許す組み合わせと拒む組み合わせの両方があること、「許さない」はどの組み合わせでも拒むことです。「-」は、action がその型の principal を取らないことを表します。役割に `can` を書くと、いつも許すか組み合わせによる action が `can` と同じであることを `sekisho check` が確かめます。",
            "Whether a principal holding one role alone (and the roles it includes) is allowed each action: always, in every combination of the rest; sometimes, allowed in some combinations and denied in others; never, denied in every one. `-` is an action that does not take the principal's type. Where a role writes `can`, `sekisho check` holds the actions it is always or sometimes allowed to it."
        ))));
        let mut head = vec!["principal".to_string()];
        head.extend(g.actions.iter().map(|a| code(&a.named.name)));
        let known = &self.checked.known;
        let langs = crate::borders::Langs {
            rules: self.suite.rules.as_deref().filter(|r| r.joined()),
            dates: self.suite.dates.as_deref().filter(|d| d.joined()),
            books: self.suite.books.as_deref().filter(|b| b.joined()),
            flows: None,
        };
        let mut rows = Vec::new();
        for (label, ti, asker) in &askers {
            let mut row = vec![label.clone()];
            for (ai, a) in g.actions.iter().enumerate() {
                if !a.principals.contains(ti) {
                    row.push("-".into());
                    continue;
                }
                let Some(s) = self.checked.report.spaces[ai].as_ref() else {
                    row.push("-".into());
                    continue;
                };
                row.push(allowance_word(lang, &crate::checks::allowance(g, s, known, &langs, asker)));
            }
            rows.push(row);
        }
        let marks = vec![Mark::Plain; rows.len()];
        self.push(Block::Table { head, rows, marks });
    }

    // -----------------------------------------------------------------------------------------
    // 6. The workflows

    fn workflows(&mut self, calls: Option<&[FlowCalls]>) {
        let g = self.g;
        let lang = self.lang;
        if g.workflows.is_empty() {
            return;
        }
        self.heading(2, self.t(tr!("ワークフロー", "Workflows")), "workflows".into());
        self.push(Block::Para(self.t(tr!(
            "ワークフローは、自分の資格で操作を呼びます。Cedar では principal `Workflow::\"<名前>\"` です。ワークフローごとに、どの action をどれだけ許されるかを並べます。",
            "A workflow calls the operations as itself: in Cedar, the principal `Workflow::\"<name>\"`. For each workflow, the actions it is allowed, and how far."
        ))));
        let known = &self.checked.known;
        let langs = crate::borders::Langs {
            rules: self.suite.rules.as_deref().filter(|r| r.joined()),
            dates: self.suite.dates.as_deref().filter(|d| d.joined()),
            books: self.suite.books.as_deref().filter(|b| b.joined()),
            flows: None,
        };
        let Some(wt) = g.workflow_type() else { return };
        for (wi, w) in g.workflows.iter().enumerate() {
            let flow = Name::file(Tool::Dandori, self.shown_path(&w.file.to_string_lossy()));
            self.heading(3, format!("{} {}", self.t(tr!("ワークフロー", "workflow")), named(lang, &w.named)), id_of("workflow", &w.named.alias));
            let decl = self.scope.files.iter().find_map(|f| f.workflows.iter().find(|x| x.name.ascii() == w.named.alias));
            if let Some((d, _)) = decl.and_then(|d| d.description.as_ref()) {
                self.push(Block::Para(d.clone()));
            }
            self.push(Block::List(vec![self.t(tr!("フロー：{}", "Its flow: {}.", code(&flow.text())))]));
            let asker = Asker::Workflow(w.named.alias.clone());
            let mut rows = Vec::new();
            let mut answers: BTreeMap<usize, Found<Allowance>> = BTreeMap::new();
            for (ai, a) in g.actions.iter().enumerate() {
                if !a.principals.contains(&wt) {
                    continue;
                }
                let Some(s) = self.checked.report.spaces[ai].as_ref() else { continue };
                let found = crate::checks::allowance(g, s, known, &langs, &asker);
                let (picked, allowed) = asker_counts(g, s, &asker);
                let word = allowance_word(lang, &found);
                let how = if matches!(found, Found::Undecided(_)) {
                    word
                } else {
                    self.t(tr!("{word}（{} 通りのうち {} 通りを許す）", "{word} ({} of {} {} allowed)", n(picked), n(allowed); n(allowed), n(picked), combos(picked)))
                };
                rows.push(vec![self.action_name(ai), how]);
                answers.insert(ai, found);
            }
            if rows.is_empty() {
                self.push(Block::Para(self.t(tr!("principal に `Workflow` を取る action がありません。", "No action takes `Workflow` as a principal."))));
            } else {
                let marks = vec![Mark::Plain; rows.len()];
                self.push(Block::Table { head: vec!["action".into(), self.t(tr!("許されるか", "Allowed"))], rows, marks });
            }
            let Some(fc) = calls.and_then(|cs| cs.iter().find(|c| c.workflow == wi)) else { continue };
            self.heading(4, self.t(tr!("フローが呼ぶ操作", "What the flow calls")), format!("{}-calls", id_of("workflow", &w.named.alias)));
            match &fc.calls {
                Err(said) => {
                    let text = said.iter().map(|x| format!("{}: {}", x.file, x.message.get(lang))).collect::<Vec<_>>().join("\n");
                    self.push(Block::Para(self.t(tr!("dandori はこのフローを読めませんでした。", "dandori could not read the flow."))));
                    self.push(Block::Code { lang: "text", caption: String::new(), text });
                }
                Ok(cs) if cs.is_empty() => self.push(Block::Para(self.t(tr!("フローは、契約の操作をどれも呼びません。", "The flow calls no operation of a contract.")))),
                Ok(cs) => {
                    self.push(Block::Para(self.t(tr!(
                        "フローのタスクが呼ぶ操作と、それを守る action、そのワークフローがその action を許されるかです。`ritsu check` は、これを言語をまたいで確かめます。",
                        "Each operation a task of the flow calls, the action that guards it, and whether the workflow is allowed that action: what `ritsu check` holds across the languages."
                    ))));
                    let mut rows = Vec::new();
                    for c in cs {
                        let guard = g.actions.iter().position(|a| a.references().iter().any(|(r, _)| *r == c.operation));
                        let (by, allowed) = match guard {
                            Some(ai) => (
                                self.action_name(ai),
                                match answers.get(&ai) {
                                    Some(f) => allowance_word(lang, f),
                                    None => tr!("許さない（Workflow を取らない）", "never (it takes no Workflow)").get(lang).to_string(),
                                },
                            ),
                            None => (self.t(tr!("（守る action なし）", "(no action guards it)")), "-".to_string()),
                        };
                        let denied = match &c.denied {
                            Some(e) => code(e),
                            None => self.t(tr!("宣言していない", "none declared")),
                        };
                        rows.push(vec![c.line.to_string(), code(&c.task), code(&c.operation.text()), by, allowed, denied]);
                    }
                    let marks = vec![Mark::Plain; rows.len()];
                    let head = vec![
                        self.t(tr!("行", "Line")),
                        self.t(tr!("タスク", "Task")),
                        self.t(tr!("操作", "Operation")),
                        self.t(tr!("守る action", "Guarded by")),
                        self.t(tr!("許されるか", "Allowed")),
                        self.t(tr!("拒まれたときのエラー", "Error on a denial")),
                    ];
                    self.push(Block::Table { head, rows, marks });
                }
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // 7. The operations guarded

    fn operations(&mut self) {
        let g = self.g;
        self.heading(2, self.t(tr!("守る操作", "Operations guarded")), "operations".into());
        if g.actions.iter().all(|a| a.references().is_empty()) {
            self.push(Block::Para(self.t(tr!("どの action も、契約の操作を守りません。", "No action guards an operation of a contract."))));
            return;
        }
        self.push(Block::Para(self.t(tr!(
            "action が守る契約の操作です。サービスは、操作を行う前に、その action について Cedar に尋ねます。参照のパスはルートからです。",
            "The operations of the contracts each action guards: before it does an operation, the service asks Cedar about the action. The paths of the references are from the root."
        ))));
        let mut docs = Contracts::default();
        let mut rows = Vec::new();
        let mut none = Vec::new();
        for (ai, a) in g.actions.iter().enumerate() {
            let refs = a.references();
            if refs.is_empty() {
                none.push(self.action_name(ai));
                continue;
            }
            for gd in &a.guards {
                let Some(r) = &gd.reference else { continue };
                let how = docs.called(g, gd, r).unwrap_or_default();
                rows.push(vec![self.action_name(ai), code(&r.text()), if how.is_empty() { "-".into() } else { code(&how) }]);
            }
        }
        if !rows.is_empty() {
            let marks = vec![Mark::Plain; rows.len()];
            self.push(Block::Table { head: vec!["action".into(), self.t(tr!("操作", "Operation")), self.t(tr!("呼び出し方", "How it is called"))], rows, marks });
        }
        self.push(Block::Para(if none.is_empty() {
            self.t(tr!("どの action も、操作を守っています。", "Every action guards an operation."))
        } else {
            self.t(tr!(
                "契約の操作を一つも守らない action：{}。",
                "Actions that guard no operation of a contract: {}.",
                joined(self.lang, &none);
                and_list(self.lang, &none)
            ))
        }));
    }

    // -----------------------------------------------------------------------------------------
    // The warnings

    fn warnings(&mut self) {
        let warned: Vec<&crate::diag::Diag> = self.o.diags.iter().filter(|d| !d.is_error()).collect();
        if warned.is_empty() {
            return;
        }
        self.heading(2, self.t(tr!("検査の警告", "What check warns of")), "warnings".into());
        self.push(Block::Para(self.t(tr!("`sekisho check` の警告です。ファイルは検査を通っています。", "The warnings of `sekisho check`; the file passes."))));
        let text: String = warned.iter().map(|d| d.render(self.lang)).collect();
        self.push(Block::Code { lang: "text", caption: String::new(), text: text.trim_end().to_string() });
    }
}

/// The `principal` line of a policy, as written.
fn who_text(w: &crate::ast::Who) -> String {
    use crate::ast::Who;
    match w {
        Who::In(rs) => format!("principal in {}", rs.iter().map(|r| r.word.as_str()).collect::<Vec<_>>().join(", ")),
        Who::Is(r) => format!("principal is {}", r.word),
        Who::Workflow(r) => format!("principal is workflow {}", r.word),
    }
}

/// A field's type as the page says it: `money[GBP, incl_tax], 1GBP to 10,000GBP`.
fn field_type(lang: Lang, g: &Gate, f: &Field) -> String {
    let opt = if f.optional { "?" } else { "" };
    match &f.ty {
        FieldType::Bool => format!("`bool{opt}`"),
        FieldType::Enum(e) => {
            let vs: Vec<String> = g.enums[*e].values.iter().map(|v| code(&v.name)).collect();
            let en = &g.enums[*e].named.name;
            if lang == Lang::Ja { format!("`{en}{opt}`（{}）", joined(lang, &vs)) } else { format!("`{en}{opt}` ({})", joined(lang, &vs)) }
        }
        FieldType::Num { unit, written, lo, hi } => {
            let (a, b) = (crate::cells::show(*lo, unit), crate::cells::show(*hi, unit));
            if lang == Lang::Ja { format!("`{written}{opt}`、{a}〜{b}") } else { format!("`{written}{opt}`, {a} to {b}") }
        }
        FieldType::Date { lo, hi } => {
            let (a, b) = (crate::types::day_text(*lo), crate::types::day_text(*hi));
            if lang == Lang::Ja { format!("`date{opt}`、{a}〜{b}") } else { format!("`date{opt}`, {a} to {b}") }
        }
        FieldType::Entity(t) => format!("`{}{opt}`", g.types[*t].named.name),
    }
}

/// How today stands to a date (`x`, in each language), for each comparison: the words after
/// `today が` and `today is`.
fn compared(op: crate::ast::Op, x_ja: &str, x_en: &str) -> (String, String) {
    use crate::ast::Op;
    match op {
        Op::Lt => (format!("{x_ja}より前"), format!("before {x_en}")),
        Op::Le => (format!("{x_ja}かそれより前"), format!("on or before {x_en}")),
        Op::Gt => (format!("{x_ja}より後"), format!("after {x_en}")),
        Op::Ge => (format!("{x_ja}かそれより後"), format!("on or after {x_en}")),
        Op::Is => (format!("{x_ja}と同じ日"), format!("on {x_en}")),
    }
}

/// What a value given to a rule or a date is, as the gate writes it.
fn source_text(g: &Gate, a: &Action, s: &Source) -> String {
    match s {
        Source::Attr(o, n) => {
            let ts = if *o == Owner::Principal { &a.principals } else { &a.resources };
            let shown = ts.iter().find_map(|&t| g.types[t].attr(n)).map(|(_, f)| f.named.name.clone()).unwrap_or_else(|| n.clone());
            format!("{}.{shown}", o.word())
        }
        Source::Input(n) => a.input(n).map(|(_, f)| f.named.name.clone()).unwrap_or_else(|| n.clone()),
        Source::Lit(Literal::Num(num)) => num.raw.clone(),
        Source::Lit(Literal::Date(d)) => crate::types::day_text(*d),
        Source::Lit(Literal::Bool(b)) => b.to_string(),
        Source::Lit(Literal::Word(w)) => w.clone(),
        Source::Today => "today".to_string(),
    }
}

/// `+00:00` from minutes east of UTC.
fn offset_text(m: i32) -> String {
    let sign = if m < 0 { '-' } else { '+' };
    format!("{sign}{:02}:{:02}", m.abs() / 60, m.abs() % 60)
}

/// The contracts the page reads again, to say how an operation is called: each read once.
#[derive(Default)]
struct Contracts {
    docs: BTreeMap<usize, Option<ritsu_base::openapi::Document>>,
    protos: BTreeMap<usize, Option<ritsu_proto::ProtoFile>>,
}

impl Contracts {
    /// How the operation a guard finds is called: `POST /orders/{orderId}/refunds`, `send
    /// orders/{orderId}`, `/shop.v1.Orders/Refund`; a book's operation as its transfer and its
    /// operation.
    fn called(&mut self, g: &Gate, gd: &Guard, r: &Name) -> Option<String> {
        let u = g.uses.get(gd.api)?;
        let item = |k: &str| r.items.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
        match u.kind {
            UseKind::OpenApi | UseKind::AsyncApi => {
                let doc = self.docs.entry(gd.api).or_insert_with(|| {
                    let path = u.file.to_string_lossy().to_string();
                    let text = ritsu_base::fs::read_to_string(&u.file).ok()?;
                    let load = |p: &str| ritsu_base::fs::read_to_string(Path::new(p)).ok();
                    ritsu_base::openapi::read_with(&path, &text, &load).ok()
                });
                let op = doc.as_ref()?.operation(&item("operation")?)?;
                Some(format!("{} {}", op.method, op.path))
            }
            UseKind::Proto => {
                let pf = self.protos.entry(gd.api).or_insert_with(|| {
                    let path = u.file.to_string_lossy().to_string();
                    let text = ritsu_base::fs::read_to_string(&u.file).ok()?;
                    ritsu_proto::read(&path, &text).ok()
                });
                let pf = pf.as_ref()?;
                let (s, m) = (item("service")?, item("method")?);
                Some(format!("/{}/{m}", pf.full(&s)))
            }
            UseKind::Book => Some(format!("{} {}", item("transfer")?, item("operation")?)),
            _ => None,
        }
    }
}
