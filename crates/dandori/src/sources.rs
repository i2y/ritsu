//! Where dandori reads what a `.flow` names: the child `.flow`s it runs, the descriptions of the
//! APIs it calls, and the rules, through what rulec's command line prints for them. The command
//! reads them from the disk and runs rulec. The playground in the browser can do neither, so it
//! reads them from a bundle recorded beforehand: the files the examples read, and what rulec
//! printed for their rules. rulec is still read only through its command line; the bundle holds
//! that output as rulec printed it.

use crate::diag::Lang;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

pub trait Sources {
    /// The text of a file, or why it cannot be read.
    fn read(&self, path: &Path) -> Result<String, String>;
    /// One name for a file however the path to it is written, to tell a flow that runs itself.
    fn canonical(&self, path: &Path) -> PathBuf;
    /// What `rulec <cmd> <rule>` prints, read as JSON.
    fn rulec(&self, cmd: &str, rule: &Path) -> Result<Value, String>;
    /// What `rulec doc` renders for a rule, in a language: Markdown, or with `html` the page on
    /// which whoever approves the rule tries a case. It is shown as rulec wrote it.
    fn rulec_doc(&self, rule: &Path, html: bool, lang: Lang) -> Result<String, String>;
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

/// What is read from now: the disk and rulec, unless `with` says otherwise.
fn current() -> Rc<dyn Sources> {
    CURRENT.with(|c| c.borrow().clone()).unwrap_or_else(|| Rc::new(Disk))
}

pub fn read(path: &Path) -> Result<String, String> {
    current().read(path)
}

pub fn canonical(path: &Path) -> PathBuf {
    current().canonical(path)
}

pub fn rulec(cmd: &str, rule: &Path) -> Result<Value, String> {
    current().rulec(cmd, rule)
}

pub fn rulec_doc(rule: &Path, html: bool, lang: Lang) -> Result<String, String> {
    current().rulec_doc(rule, html, lang)
}

/// `rulec doc` as it is run, which is also how a bundle names what it printed.
pub fn doc_command(html: bool, lang: Lang) -> String {
    let lang = if lang == Lang::Ja { "ja" } else { "en" };
    if html {
        format!("doc --format html --lang {lang}")
    } else {
        format!("doc --lang {lang}")
    }
}

/// The rulec to run: `DANDORI_RULEC`, else `rulec` on the PATH.
pub fn rulec_binary() -> String {
    std::env::var("DANDORI_RULEC").unwrap_or_else(|_| "rulec".to_string())
}

/// The disk, and rulec run as a process.
pub struct Disk;

impl Sources for Disk {
    fn read(&self, path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| e.to_string())
    }

    fn canonical(&self, path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    fn rulec(&self, cmd: &str, rule: &Path) -> Result<Value, String> {
        let bin = rulec_binary();
        let out = Command::new(&bin).arg(cmd).arg(rule).output().map_err(|e| format!("could not run `{bin}`: {e}"))?;
        if !out.status.success() {
            let mut msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if msg.is_empty() {
                msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
            }
            let first: Vec<&str> = msg.lines().take(6).collect();
            return Err(format!("`rulec {cmd}` failed:\n{}", first.join("\n")));
        }
        serde_json::from_slice(&out.stdout).map_err(|e| format!("`rulec {cmd}` did not print JSON: {e}"))
    }

    /// rulec runs in the rule's directory and is given the file's name, so that what it writes
    /// names the file alone and not where this machine keeps it.
    fn rulec_doc(&self, rule: &Path, html: bool, lang: Lang) -> Result<String, String> {
        let bin = rulec_binary();
        // a relative path to rulec is from here, not from the rule's directory
        let program = match std::path::PathBuf::from(&bin) {
            p if p.is_relative() && p.components().count() > 1 => std::env::current_dir().map(|d| d.join(&p)).unwrap_or(p),
            p => p,
        };
        let dir = rule.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let file = rule.file_name().ok_or_else(|| format!("{} names no file", rule.display()))?;
        let command = doc_command(html, lang);
        let out = Command::new(&program)
            .current_dir(dir)
            .args(command.split(' ').take(1))
            .arg(file)
            .args(command.split(' ').skip(1))
            .output()
            .map_err(|e| format!("could not run `{bin}`: {e}"))?;
        if !out.status.success() {
            let mut msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if msg.is_empty() {
                msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
            }
            let first: Vec<&str> = msg.lines().take(6).collect();
            return Err(format!("`rulec doc` failed:\n{}", first.join("\n")));
        }
        String::from_utf8(out.stdout).map_err(|e| format!("`rulec doc` did not print text: {e}"))
    }
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

/// Files, and what rulec printed for the rules among them, by their paths from one directory.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bundle {
    pub files: BTreeMap<String, String>,
    /// for each rule, what each command printed: JSON, or for `rulec doc` the text
    pub rulec: BTreeMap<String, BTreeMap<String, Value>>,
}

impl Bundle {
    /// `{"files": {path: text}, "rulec": {path: {command: output}}}`
    pub fn to_json(&self) -> Value {
        json!({ "files": self.files, "rulec": self.rulec })
    }

    pub fn from_json(v: &Value) -> Result<Bundle, String> {
        let files = v["files"].as_object().ok_or("the bundle has no `files`")?;
        let rulec = v["rulec"].as_object().ok_or("the bundle has no `rulec`")?;
        let mut b = Bundle::default();
        for (path, text) in files {
            b.files.insert(path.clone(), text.as_str().ok_or_else(|| format!("the file {path} is not text"))?.to_string());
        }
        for (path, outputs) in rulec {
            let outputs = outputs.as_object().ok_or_else(|| format!("what rulec printed for {path} is not an object"))?;
            b.rulec.insert(path.clone(), outputs.iter().map(|(cmd, out)| (cmd.clone(), out.clone())).collect());
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

    fn rulec(&self, cmd: &str, rule: &Path) -> Result<Value, String> {
        match self.bundle.rulec.get(&key(rule)).and_then(|outputs| outputs.get(cmd)) {
            Some(v) => Ok(v.clone()),
            None => Err(match self.lang {
                Lang::En => format!("this page does not run rulec, and has what `rulec {cmd}` printed for the rules of the examples only"),
                Lang::Ja => format!("このページでは rulec を動かせません。`rulec {cmd}` の出力があるのは例の規則だけです"),
            }),
        }
    }

    fn rulec_doc(&self, rule: &Path, html: bool, lang: Lang) -> Result<String, String> {
        match self.rulec(&doc_command(html, lang), rule)? {
            Value::String(text) => Ok(text),
            _ => Err(format!("what `rulec doc` printed for {} is not text", key(rule))),
        }
    }
}

/// The disk and rulec, keeping what was read by its path from `root`: the bundle of what the
/// command read, for the playground to read the same.
pub struct Recorder {
    root: String,
    pub got: RefCell<Bundle>,
}

impl Recorder {
    pub fn new(root: &Path) -> Recorder {
        Recorder { root: key(root), got: RefCell::new(Bundle::default()) }
    }

    fn rel(&self, path: &Path) -> String {
        let k = key(path);
        k.strip_prefix(&format!("{}/", self.root)).map(str::to_string).unwrap_or(k)
    }
}

impl Sources for Recorder {
    fn read(&self, path: &Path) -> Result<String, String> {
        let text = Disk.read(path)?;
        self.got.borrow_mut().files.insert(self.rel(path), text.clone());
        Ok(text)
    }

    fn canonical(&self, path: &Path) -> PathBuf {
        Disk.canonical(path)
    }

    fn rulec(&self, cmd: &str, rule: &Path) -> Result<Value, String> {
        let out = Disk.rulec(cmd, rule)?;
        self.got.borrow_mut().rulec.entry(self.rel(rule)).or_default().insert(cmd.to_string(), out.clone());
        Ok(out)
    }

    fn rulec_doc(&self, rule: &Path, html: bool, lang: Lang) -> Result<String, String> {
        let out = Disk.rulec_doc(rule, html, lang)?;
        self.got.borrow_mut().rulec.entry(self.rel(rule)).or_default().insert(doc_command(html, lang), Value::String(out.clone()));
        Ok(out)
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
