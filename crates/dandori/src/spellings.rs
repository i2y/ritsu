//! The names a flow writes that the code of a target cannot keep (DESIGN 22).
//!
//! A generator spells a name as its target spells names: Go's `exported` makes `send_mail` into
//! `SendMail`, and a rule's `payment_intent.status` is `payment_intent_status` in the code of
//! TypeScript and Python. It never picks another name for one that does not fit. So when two
//! names of the flow or its `.proto` are one identifier in a target's code, or a name is one the
//! code has for itself, `check` says so (W105), and `gen` writes nothing for that target (E051),
//! whatever the other targets can do.
//!
//! The generators say what they declare (`temporal_go::clashes`, `temporal::clashes`,
//! `temporal_py::clashes`), and the clients of a service say it as they are written, so that this
//! file and the code cannot come apart. What a name is called inside the generated code, where
//! nobody reads it (a variable, a state of a state machine, a template of Argo, what a file
//! imports a rule as), is the generator's to choose, and stays so.

use crate::diag::{Diag, Text};
use crate::model::Model;
use crate::syntax::{Program, Span};
use ritsu_base::tr;
use std::collections::BTreeMap;

/// The targets of the code a name can stop, and the language they write it in, in the order the
/// notes go by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lang {
    TypeScript,
    Python,
    Go,
}

impl Lang {
    pub fn title(self) -> &'static str {
        match self {
            Lang::TypeScript => "TypeScript",
            Lang::Python => "Python",
            Lang::Go => "Go",
        }
    }
}

/// A name as the flow or the `.proto` writes it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Name {
    /// a record's or an enum's
    Type(String),
    /// a task's, as the method you implement it by
    Task(String),
    /// a field of a record
    Field(String, String),
    Input(String),
    Output(String),
    /// a method of the service the workflow implements
    Method(String),
    /// a message of the service, by its full name
    Message(String),
}

impl Name {
    fn text(&self) -> Text {
        match self {
            Name::Type(n) => tr!("型 `{n}`", "the type `{n}`"),
            Name::Task(n) => tr!("タスク `{n}`", "the task `{n}`"),
            Name::Field(r, f) => tr!("`{r}` のフィールド `{f}`", "the field `{f}` of `{r}`"),
            Name::Input(n) => tr!("入力 `{n}`", "the input `{n}`"),
            Name::Output(n) => tr!("出力 `{n}`", "the output `{n}`"),
            Name::Method(n) => tr!("サービスのメソッド `{n}`", "the service's method `{n}`"),
            Name::Message(n) => tr!("サービスのメッセージ `{n}`", "the service's message `{n}`"),
        }
    }

    fn in_proto(&self) -> bool {
        matches!(self, Name::Method(_) | Name::Message(_))
    }
}

/// What a name meets.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum With {
    /// another name of the flow or the `.proto`, one identifier with it
    Name(Name),
    /// what the code has for itself, or the language keeps
    Own,
    /// the same message for two types, which an alias of its name cannot be
    TwoTypes,
}

/// What one generator cannot write.
#[derive(Clone, Debug)]
pub struct Clash {
    pub lang: Lang,
    pub target: &'static str,
    /// the identifier in that code
    pub id: String,
    pub name: Name,
    pub with: With,
    /// what happens there, with the name as is
    pub why: Text,
    /// only in a package of `ritsu gen`
    pub in_package: bool,
}

/// One target's words on a found name.
#[derive(Clone, Debug)]
pub struct Said {
    pub lang: Lang,
    pub target: &'static str,
    pub why: Text,
    pub in_package: bool,
}

/// A name of the flow that a target's code cannot keep, and where it is written.
#[derive(Clone, Debug)]
pub struct Found {
    pub name: Name,
    pub with: With,
    pub line: usize,
    pub col: usize,
    pub said: Vec<Said>,
}

/// Where the names are written, as the lowering knows it.
pub struct Places<'a> {
    pub prog: &'a Program,
    /// where a type made from a rule, a book or a `.proto` is first read
    pub made_at: &'a BTreeMap<String, Span>,
    /// where a `use` line names what it reads
    pub use_spans: &'a BTreeMap<String, Span>,
}

impl Places<'_> {
    fn at(&self, name: &Name, m: &Model) -> (usize, usize) {
        let sp = |s: &Span| (s.line, s.col);
        let start = (1, 1);
        match name {
            Name::Type(n) => {
                let declared = self.prog.enums.iter().map(|d| &d.name).chain(self.prog.records.iter().map(|d| &d.name)).find(|(x, _)| x == n).map(|(_, s)| *s);
                declared.or_else(|| self.made_at.get(n).copied()).or_else(|| self.use_spans.get(n.split('.').next().unwrap_or(n)).copied()).map(|s| sp(&s)).unwrap_or(start)
            }
            Name::Task(n) => self.prog.tasks.iter().find(|t| t.name.0 == *n).map(|t| sp(&t.name.1)).unwrap_or(start),
            Name::Field(r, f) => {
                let declared = self.prog.records.iter().find(|d| d.name.0 == *r).and_then(|d| d.fields.iter().find(|x| x.name.0 == *f)).map(|x| x.name.1);
                declared.map(|s| sp(&s)).unwrap_or_else(|| self.at(&Name::Type(r.clone()), m))
            }
            Name::Input(n) => self.prog.inputs.iter().find(|x| x.name.0 == *n).map(|x| sp(&x.name.1)).unwrap_or(start),
            Name::Output(n) => self.prog.outputs.iter().find(|x| x.name.0 == *n).map(|x| sp(&x.name.1)).unwrap_or(start),
            Name::Method(_) | Name::Message(_) => m.service.as_ref().map(|s| (s.line, s.col)).unwrap_or(start),
        }
    }
}

/// Every name of the flow that the code of a target cannot keep, each once, by where it is written.
pub fn find(m: &Model, places: &Places) -> Vec<Found> {
    let mut clashes = crate::temporal::clashes(m);
    clashes.extend(crate::temporal_py::clashes(m));
    clashes.extend(crate::temporal_go::clashes(m));
    let mut found: Vec<Found> = Vec::new();
    for c in clashes {
        let said = Said { lang: c.lang, target: c.target, why: c.why, in_package: c.in_package };
        // two names one way and the other are one finding
        let same = |f: &Found| (f.name == c.name && f.with == c.with) || matches!(&f.with, With::Name(o) if *o == c.name && With::Name(f.name.clone()) == c.with);
        match found.iter_mut().find(|f| same(f)) {
            Some(f) if f.said.iter().any(|s| s.target == said.target) => {}
            Some(f) => f.said.push(said),
            None => {
                let (line, col) = places.at(&c.name, m);
                found.push(Found { name: c.name, with: c.with, line, col, said: vec![said] });
            }
        }
    }
    found.sort_by(|a, b| (a.line, a.col, &a.name).cmp(&(b.line, b.col, &b.name)));
    found
}

/// The languages a finding names, each once, and for each its targets.
fn languages(f: &Found) -> Vec<(Lang, Vec<&Said>)> {
    let mut out: Vec<(Lang, Vec<&Said>)> = Vec::new();
    let mut said: Vec<&Said> = f.said.iter().collect();
    said.sort_by_key(|s| s.lang);
    for s in said {
        match out.last_mut() {
            Some((l, v)) if *l == s.lang => v.push(s),
            _ => out.push((s.lang, vec![s])),
        }
    }
    out
}

/// The head of a message: what the names are, in the code of `place`.
fn headline(f: &Found, place: Text) -> Text {
    let (pj, pe) = (place.ja, place.en);
    let name = f.name.text();
    let (nj, ne) = (name.ja, capital(&name.en));
    match &f.with {
        With::Name(other) => {
            let o = other.text();
            let (oj, oe) = (o.ja, capital(&o.en));
            let ne = name_lower(&ne);
            tr!("{oj}と{nj}は、{pj}で一つの名前になります", "{oe} and {ne} are one name in {pe}")
        }
        With::Own => tr!("{nj}は、{pj}でそのままの名前では使えません", "{ne} cannot keep its name in {pe}"),
        With::TwoTypes => tr!("{nj}は、{pj}で二つの型の名前になります", "{ne} is the name of two types in {pe}"),
    }
}

fn name_lower(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_lowercase().chain(c).collect()).unwrap_or_default()
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// How to put it right.
fn fix(f: &Found) -> Text {
    let proto = f.name.in_proto() && matches!(&f.with, With::Name(o) if o.in_proto()) || (f.name.in_proto() && !matches!(f.with, With::Name(_)));
    if proto {
        tr!(
            "作らない出力先なら、そのままで構いません。作る出力先なら、`.proto` のメソッドかメッセージの名前を変えてください。メソッドの名前は Connect のパスでもあるので、呼ぶ側と合わせて変えてください。",
            "For a target you do not build, leave it as it is. For one you do, rename the method or the message in the `.proto`. A method's name is also its path on Connect, so change the callers with it."
        )
    } else {
        tr!(
            "作らない出力先なら、そのままで構いません。作る出力先なら、`.flow` で名前を変えてください。タスクの名前はアクティビティの名前で、フィールドと入出力の名前は JSON の鍵でもあるので、動いている実行があるなら、版を上げてください。",
            "For a target you do not build, leave it as it is. For one you do, rename it in the `.flow`. A task's name is also its activity's, and a field's or an input's its key in JSON, so if runs are going on, change the workflow's version with it."
        )
    }
}

fn note(lang: Lang, target_list: &[&Said]) -> Text {
    let targets: Vec<String> = target_list.iter().map(|s| format!("`--target {}`", s.target)).collect();
    let (title, list) = (lang.title(), targets.join(", "));
    let why = &target_list[0].why;
    let (ja, en) = (&why.ja, &why.en);
    tr!("{title}（{list}）：{ja}", "{title} ({list}): {en}")
}

/// W105: what `check` says of the names a target's code cannot keep.
pub fn warnings(m: &Model) -> Vec<Diag> {
    m.spellings
        .iter()
        .map(|f| {
            let langs = languages(f);
            let titles: Vec<&str> = langs.iter().map(|(l, _)| l.title()).collect();
            let ja = titles.join("、");
            let en = match titles.as_slice() {
                [init @ .., last] if !init.is_empty() => format!("{} and {last}", init.join(", ")),
                _ => titles.join(""),
            };
            let mut d = Diag::warning("W105", f.line, f.col, headline(f, tr!("{ja} のコード", "the code of {en}")));
            for (l, said) in &langs {
                d = d.note(note(*l, said));
            }
            d.note(fix(f))
        })
        .collect()
}

/// E051: what stops `gen` for `target`: nothing is written for it until the names are put right.
pub fn stops(m: &Model, target: &str) -> Vec<Diag> {
    m.spellings
        .iter()
        .filter_map(|f| {
            let said: Vec<&Said> = f.said.iter().filter(|s| s.target == target && (!s.in_package || m.package.is_some())).collect();
            let first = said.first()?;
            let d = Diag::error("E051", f.line, f.col, headline(f, tr!("`--target {target}` のコード", "the code of `--target {target}`")));
            Some(d.note(note(first.lang, &said)).note(fix(f)))
        })
        .collect()
}

/// The names the client of a service has so far: what it has for itself, the functions that do
/// what a method may do, and what it has written for the methods and the messages (DESIGN 1.14).
pub(crate) struct ClientNames {
    own: Vec<(String, With)>,
    functions: &'static [&'static str],
    written: Vec<(String, Name, String)>,
    /// what the client cannot write: the identifier, the name, and what it meets
    pub clashes: Vec<(String, Name, With)>,
}

/// A message's type in a client.
pub(crate) enum Alias {
    /// a type of this name to write
    Write(String),
    /// the type there is, by this name
    Use(String),
}

impl ClientNames {
    pub(crate) fn new(own: Vec<(String, With)>, functions: &'static [&'static str]) -> ClientNames {
        ClientNames { own, functions, written: Vec::new(), clashes: Vec::new() }
    }

    fn holder(&self, id: &str) -> Option<With> {
        if let Some((_, n, _)) = self.written.iter().find(|w| w.0 == id) {
            return Some(With::Name(n.clone()));
        }
        self.own.iter().find(|o| o.0 == id).map(|o| o.1.clone())
    }

    /// The function of `method`, when the client is to write one: not when one of the client's own
    /// does what the method does (`same`), nor when the name is taken, which is noted.
    pub(crate) fn function(&mut self, id: String, method: &str, same: bool) -> Option<String> {
        let name = Name::Method(method.to_string());
        if self.functions.contains(&id.as_str()) {
            if !same {
                self.clashes.push((id, name, With::Own));
            }
            return None;
        }
        if let Some(with) = self.holder(&id) {
            self.clashes.push((id, name, with));
            return None;
        }
        self.written.push((id.clone(), name, String::new()));
        Some(id)
    }

    /// The type of `message`, which is `ty` here. A message two methods share is one type, when
    /// they read it as one. When the name is taken: the client's own type, if it is what the
    /// message means (`same`); else it is noted, and the client writes nothing for it.
    pub(crate) fn message(&mut self, id: String, message: &str, ty: &str, same: bool) -> Alias {
        // a type of the flow of the message's name, which the message is: nothing to write
        if id == ty {
            return Alias::Use(id);
        }
        let name = Name::Message(message.to_string());
        if let Some((_, Name::Message(m), t)) = self.written.iter().find(|w| w.0 == id) {
            if m == message {
                if t != ty {
                    self.clashes.push((id.clone(), name, With::TwoTypes));
                }
                return Alias::Use(id);
            }
        }
        let held = self.holder(&id).or_else(|| self.functions.contains(&id.as_str()).then_some(With::Own));
        match held {
            None => {
                self.written.push((id.clone(), name, ty.to_string()));
                Alias::Write(id)
            }
            Some(_) if same => Alias::Use(ty.to_string()),
            Some(with) => {
                self.clashes.push((id.clone(), name, with));
                Alias::Use(id)
            }
        }
    }
}
