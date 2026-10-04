//! Where dandori reads what a `.flow` names: the child `.flow`s it runs, the descriptions of the
//! APIs it calls, and the rules, through ritsu's port of rules (`ritsu_ports::Rules`, ritsu's
//! DESIGN 3.2). dandori holds no rulec: the program that runs it hands it the port.
//!
//! - `Disk`: the disk, and the port it is handed. `ritsu dandori` and the tests hand it rulec's own
//!   answer (`rulec::ports::Engine`); the dandori binary of this crate hands it `NoRules`, which
//!   reads no rule and says to run the flow with `ritsu dandori` (ritsu's DESIGN 2.3).
//! - `Playground`: the page in the browser, which can neither read the disk nor hold rulec. It
//!   reads a bundle recorded beforehand: the files the examples read, and what rulec answered for
//!   their rules (`Recorded`, which answers the port from the record).
//! - `Recorder`: the disk and a port, keeping what was read, to record the bundle.

use crate::diag::Lang;
use ritsu_base::text::Text;
use ritsu_ports::{Answer, DaySet, Precondition, RuleError, RuleFacts, Rules, Said, Values};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

pub trait Sources {
    /// The text of a file, or why it cannot be read.
    fn read(&self, path: &Path) -> Result<String, String>;
    /// One name for a file however the path to it is written, to tell a flow that runs itself.
    fn canonical(&self, path: &Path) -> PathBuf;
    /// The port the rules are read through.
    fn rules(&self) -> &dyn Rules;
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<dyn Sources>>> = const { RefCell::new(None) };
}

/// Run `f` reading from `s`, then read from what was read from before.
pub fn with<R>(s: Rc<dyn Sources>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Rc<dyn Sources>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let before = self.0.take();
            CURRENT.with(|c| *c.borrow_mut() = before);
        }
    }
    let _restore = Restore(CURRENT.with(|c| c.borrow_mut().replace(s)));
    f()
}

/// Run `f` reading the disk, and the rules through `rules`.
pub fn with_rules<R>(rules: Rc<dyn Rules>, f: impl FnOnce() -> R) -> R {
    with(Rc::new(Disk::new(rules)), f)
}

/// What is read from now: the disk with no rules, unless `with` says otherwise.
fn current() -> Rc<dyn Sources> {
    CURRENT.with(|c| c.borrow().clone()).unwrap_or_else(|| Rc::new(Disk::new(Rc::new(NoRules))))
}

pub fn read(path: &Path) -> Result<String, String> {
    current().read(path)
}

pub fn canonical(path: &Path) -> PathBuf {
    current().canonical(path)
}

/// What rulec knows of a rule, or what is said instead.
pub fn rule(path: &Path) -> Result<RuleFacts, Vec<Said>> {
    current().rules().facts(path)
}

/// The page `rulec doc` draws for a rule, in a language: Markdown, or with `html` the page on which
/// whoever approves the rule tries a case. It names the file alone, not where this machine keeps
/// it, and is shown as rulec drew it.
pub fn rule_doc(path: &Path, html: bool, lang: Lang) -> Result<String, Vec<Said>> {
    let shown = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    current().rules().doc(path, &shown, html, lang)
}

/// What was said of a rule, as notes of the diagnostic that names it: each thing rulec says, with
/// its code and where it is.
pub fn said_notes(said: &[Said]) -> Vec<Text> {
    said.iter()
        .map(|s| {
            let at = match s.line {
                Some(l) => Text::new(format!("{} の {l} 行目", s.file), format!("{}, line {l}", s.file)),
                None => Text::same(s.file.clone()),
            };
            match s.code.as_str() {
                "" => s.message.clone(),
                code => Text::new(format!("{code}: {}（{}）", s.message.ja, at.ja), format!("{code}: {} ({})", s.message.en, at.en)),
            }
        })
        .collect()
}

/// `rulec doc` as it is run, which is also how a bundle names what it drew.
pub fn doc_command(html: bool, lang: Lang) -> String {
    let lang = if lang == Lang::Ja { "ja" } else { "en" };
    if html {
        format!("doc --format html --lang {lang}")
    } else {
        format!("doc --lang {lang}")
    }
}

/// The disk, and the port of rules the program that runs dandori hands over.
pub struct Disk {
    rules: Rc<dyn Rules>,
}

impl Disk {
    pub fn new(rules: Rc<dyn Rules>) -> Disk {
        Disk { rules }
    }
}

impl Sources for Disk {
    fn read(&self, path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| e.to_string())
    }

    fn canonical(&self, path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    fn rules(&self) -> &dyn Rules {
        &*self.rules
    }
}

/// The port of the dandori binary of this crate, which holds no rulec (ritsu's DESIGN 2.3): every
/// question about a rule is answered with what to run instead. A flow that uses no rule never asks.
pub struct NoRules;

fn no_rules(rule: &Path) -> Vec<Said> {
    vec![Said {
        code: String::new(),
        file: rule.display().to_string(),
        line: None,
        message: ritsu_base::tr!(
            "この dandori は規則を読めません。規則を使うワークフローは `ritsu dandori …` で走らせてください",
            "this dandori does not read rules; run a workflow that uses rules with `ritsu dandori …`"
        ),
    }]
}

impl Rules for NoRules {
    fn facts(&self, rule: &Path) -> Result<RuleFacts, Vec<Said>> {
        Err(no_rules(rule))
    }

    fn preconditions_hold(&self, rule: &Path, _: &[(String, Option<i128>, Option<i128>)]) -> Result<Vec<(Precondition, Answer<Values>)>, Vec<Said>> {
        Err(no_rules(rule))
    }

    fn checked_over(&self, rule: &Path, _: &str, _: &DaySet) -> Result<Answer<Text>, Vec<Said>> {
        Err(no_rules(rule))
    }

    fn eval(&self, rule: &Path, _: &Values) -> Result<Values, RuleError> {
        Err(RuleError::Unread(no_rules(rule)))
    }

    fn doc(&self, rule: &Path, _: &str, _: bool, _: Lang) -> Result<String, Vec<Said>> {
        Err(no_rules(rule))
    }

    fn joined(&self) -> bool {
        false
    }
}

/// Whether the rules a flow uses can be read here: false in the dandori binary of this crate,
/// which holds no rulec (E018).
pub fn rules_joined() -> bool {
    current().rules().joined()
}

/// A path as a bundle names it: without `.` and `..`, with `/` between the parts.
pub fn key(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut rooted = false;
    for c in path.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => rooted = true,
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.last().is_some_and(|p| p != "..") {
                    parts.pop();
                } else if !rooted {
                    parts.push("..".into());
                }
            }
            Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
        }
    }
    let joined = parts.join("/");
    if rooted {
        format!("/{joined}")
    } else {
        joined
    }
}

/// What rulec answered for rules, by their paths from one directory: each rule's facts, and the
/// pages `rulec doc` drew for it, by the command that draws them (`doc --format html --lang en`).
/// It answers the port from the record, and says it has nothing else.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recorded {
    pub facts: BTreeMap<String, RuleFacts>,
    pub pages: BTreeMap<String, BTreeMap<String, String>>,
}

impl Recorded {
    fn missing(&self, rule: &Path) -> Vec<Said> {
        let message = ritsu_base::tr!("このページでは rulec を動かせないので、読めるのは例の規則だけです", "this page does not run rulec, and reads the rules of the examples only");
        vec![Said { code: String::new(), file: key(rule), line: None, message }]
    }
}

impl Rules for Recorded {
    fn facts(&self, rule: &Path) -> Result<RuleFacts, Vec<Said>> {
        self.facts.get(&key(rule)).cloned().ok_or_else(|| self.missing(rule))
    }

    fn preconditions_hold(&self, rule: &Path, _: &[(String, Option<i128>, Option<i128>)]) -> Result<Vec<(Precondition, Answer<Values>)>, Vec<Said>> {
        Err(self.missing(rule))
    }

    fn checked_over(&self, rule: &Path, _: &str, _: &DaySet) -> Result<Answer<Text>, Vec<Said>> {
        Err(self.missing(rule))
    }

    fn eval(&self, rule: &Path, _: &Values) -> Result<Values, RuleError> {
        Err(RuleError::Unread(self.missing(rule)))
    }

    fn doc(&self, rule: &Path, _: &str, html: bool, lang: Lang) -> Result<String, Vec<Said>> {
        let cmd = doc_command(html, lang);
        self.pages.get(&key(rule)).and_then(|p| p.get(&cmd)).cloned().ok_or_else(|| self.missing(rule))
    }
}

/// Files, and what rulec answered for the rules among them, by their paths from one directory.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bundle {
    pub files: BTreeMap<String, String>,
    pub rules: Recorded,
}

impl Bundle {
    /// `{"files": {path: text}, "rules": {path: {"facts": facts, "doc …": page}}}`
    pub fn to_json(&self) -> Value {
        let mut rules = serde_json::Map::new();
        let paths: std::collections::BTreeSet<&String> = self.rules.facts.keys().chain(self.rules.pages.keys()).collect();
        for p in paths {
            let mut one = serde_json::Map::new();
            if let Some(f) = self.rules.facts.get(p) {
                one.insert("facts".into(), crate::record::facts_to_json(f));
            }
            for (cmd, page) in self.rules.pages.get(p).into_iter().flatten() {
                one.insert(cmd.clone(), json!(page));
            }
            rules.insert(p.clone(), Value::Object(one));
        }
        json!({ "files": self.files, "rules": rules })
    }

    pub fn from_json(v: &Value) -> Result<Bundle, String> {
        let files = v["files"].as_object().ok_or("the bundle has no `files`")?;
        let rules = v["rules"].as_object().ok_or("the bundle has no `rules`")?;
        let mut b = Bundle::default();
        for (path, text) in files {
            b.files.insert(path.clone(), text.as_str().ok_or_else(|| format!("the file {path} is not text"))?.to_string());
        }
        for (path, one) in rules {
            let one = one.as_object().ok_or_else(|| format!("what rulec answered for {path} is not an object"))?;
            for (k, x) in one {
                if k == "facts" {
                    b.rules.facts.insert(path.clone(), crate::record::facts_from_json(x).map_err(|e| format!("the facts of {path}: {e}"))?);
                } else {
                    let page = x.as_str().ok_or_else(|| format!("what `rulec {k}` drew for {path} is not text"))?;
                    b.rules.pages.entry(path.clone()).or_default().insert(k.clone(), page.to_string());
                }
            }
        }
        Ok(b)
    }
}

/// What the playground reads from: a bundle, with the file the reader edits as they have it now.
pub struct Playground {
    pub bundle: Rc<Bundle>,
    /// the file being edited, by its path in the bundle
    pub path: String,
    pub text: String,
    /// the language of what it says of a file it does not have
    pub lang: Lang,
}

impl Sources for Playground {
    fn read(&self, path: &Path) -> Result<String, String> {
        let k = key(path);
        if k == self.path {
            return Ok(self.text.clone());
        }
        self.bundle.files.get(&k).cloned().ok_or_else(|| match self.lang {
            Lang::En => "this page has only the files of the examples".to_string(),
            Lang::Ja => "このページにあるのは例のファイルだけです".to_string(),
        })
    }

    fn canonical(&self, path: &Path) -> PathBuf {
        PathBuf::from(key(path))
    }

    fn rules(&self) -> &dyn Rules {
        &self.bundle.rules
    }
}

/// The disk and a port, keeping what was read by its path from `root`: the bundle of what the
/// command read, for the playground to read the same.
pub struct Recorder {
    root: String,
    pub got: RefCell<Bundle>,
    rules: Recording,
}

/// A port that answers as another does, and keeps what it answered (a rule's facts, and the pages
/// drawn), by the path from the recorder's root.
struct Recording {
    root: String,
    inner: Rc<dyn Rules>,
    got: RefCell<Recorded>,
}

impl Recording {
    fn rel(&self, path: &Path) -> String {
        rel(&self.root, path)
    }
}

fn rel(root: &str, path: &Path) -> String {
    let k = key(path);
    k.strip_prefix(&format!("{root}/")).map(str::to_string).unwrap_or(k)
}

impl Rules for Recording {
    fn facts(&self, rule: &Path) -> Result<RuleFacts, Vec<Said>> {
        let f = self.inner.facts(rule)?;
        self.got.borrow_mut().facts.insert(self.rel(rule), f.clone());
        Ok(f)
    }

    fn preconditions_hold(&self, rule: &Path, ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<Vec<(Precondition, Answer<Values>)>, Vec<Said>> {
        self.inner.preconditions_hold(rule, ranges)
    }

    fn checked_over(&self, rule: &Path, input: &str, days: &DaySet) -> Result<Answer<Text>, Vec<Said>> {
        self.inner.checked_over(rule, input, days)
    }

    fn eval(&self, rule: &Path, inputs: &Values) -> Result<Values, RuleError> {
        self.inner.eval(rule, inputs)
    }

    fn doc(&self, rule: &Path, shown: &str, html: bool, lang: Lang) -> Result<String, Vec<Said>> {
        let page = self.inner.doc(rule, shown, html, lang)?;
        self.got.borrow_mut().pages.entry(self.rel(rule)).or_default().insert(doc_command(html, lang), page.clone());
        Ok(page)
    }
}

impl Recorder {
    pub fn new(root: &Path, rules: Rc<dyn Rules>) -> Recorder {
        let root = key(root);
        Recorder { root: root.clone(), got: RefCell::new(Bundle::default()), rules: Recording { root, inner: rules, got: RefCell::new(Recorded::default()) } }
    }

    /// What was read, the rules' answers with it.
    pub fn bundle(&self) -> Bundle {
        let mut b = self.got.borrow().clone();
        b.rules = self.rules.got.borrow().clone();
        b
    }
}

impl Sources for Recorder {
    fn read(&self, path: &Path) -> Result<String, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        self.got.borrow_mut().files.insert(rel(&self.root, path), text.clone());
        Ok(text)
    }

    fn canonical(&self, path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    fn rules(&self) -> &dyn Rules {
        &self.rules
    }
}

#[cfg(test)]
mod tests {
    use super::key;
    use std::path::Path;

    #[test]
    fn a_key_takes_out_dots() {
        assert_eq!(key(Path::new("examples/hotel/temporal/../rules/hold_amount.rule")), "examples/hotel/rules/hold_amount.rule");
        assert_eq!(key(Path::new("tests/fixtures/../../examples/hotel/rules/x.rule")), "examples/hotel/rules/x.rule");
        assert_eq!(key(Path::new("./a/./b.flow")), "a/b.flow");
        assert_eq!(key(Path::new("../a.flow")), "../a.flow");
        assert_eq!(key(Path::new("/a/../../b")), "/b");
    }
}
