//! The playground in the browser (DESIGN 8.7): what the page asks and what it is answered.
//!
//! The page holds a small project: files by their paths, edited in tabs. Every request hands all of
//! them over, with the language to answer in, and the module puts them in memory under a working
//! directory ([`HOME`]) and runs the command there, as a person would run it in a directory holding
//! the same files:
//!
//! - [`check`]: `ritsu check .` — every file by its language's own check, the languages joined once,
//!   then what ritsu checks across them. It is ritsu's own `check` (`ritsu::check::run`), so a check
//!   ritsu gains later is in the page with nothing changed here.
//! - [`generate`]: one file's generator: `rulec gen`, `koyomi gen`, `chobo build`, `dandori build`,
//!   `sakai export cml`, `yuen export` (the last two given the project's root, as `ritsu check` gives
//!   it them). geas and `.proto` files generate nothing.
//! - [`doc`]: one file's page for whoever approves it, as HTML and as Markdown: `rulec doc`,
//!   `koyomi doc`, `chobo doc`, `dandori doc`.
//!
//! Each answer is what the command prints (`out`, `err`), its exit code, and what it writes. Where
//! a language's command takes the writers it prints to (ritsu's `check`, rulec's `gen`, dandori,
//! yuen, sakai), the command itself runs. rulec's `doc`, koyomi's and chobo's commands print to the
//! process's own output, which a page has none of; for them the functions here call the functions
//! the command calls, in its order, and say what it says. crates/ritsu/tests/playground.rs holds
//! every answer to what the `ritsu` binary prints and writes in a directory holding the same files.

use ritsu_base::fs::{self, Memory};
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_project::Joined;
use serde_json::{Value, json};
use std::path::Path;
use std::rc::Rc;

/// The working directory the commands run in, where the project's files are.
pub const HOME: &str = "/playground";
/// Where `gen` writes, from the working directory.
pub const OUT: &str = "generated";

/// What the page asks: the project's files, the language to answer in, and for `gen` and `doc` the
/// file asked about (and for a generator that takes one, the target).
#[derive(Clone, Debug)]
pub struct Request {
    /// By their paths from the project's root, `/` between the parts.
    pub files: Vec<(String, String)>,
    pub lang: Lang,
    pub path: String,
    pub target: Option<String>,
}

impl Request {
    /// `{"files": {path: text}, "lang": "en"|"ja", "path": …, "target": …}`.
    pub fn from_json(v: &Value) -> Result<Request, String> {
        let given = v["files"].as_object().ok_or("the request has no `files`")?;
        let mut files = Vec::new();
        for (p, text) in given {
            let rel = ritsu_base::paths::join(".", p).map_err(|e| e.text(p).en)?;
            if rel == "." {
                return Err(format!("`{p}` names the project's root, not a file"));
            }
            files.push((rel, text.as_str().ok_or_else(|| format!("the file {p} is not text"))?.to_string()));
        }
        Ok(Request {
            files,
            lang: if v["lang"] == "ja" { Lang::Ja } else { Lang::En },
            path: v["path"].as_str().unwrap_or_default().to_string(),
            target: v["target"].as_str().map(str::to_string),
        })
    }

    /// The files in memory, run in [`HOME`].
    fn memory(&self) -> Rc<Memory> {
        let m = Memory::new(HOME);
        for (p, text) in &self.files {
            m.add(p, text.as_bytes().to_vec());
        }
        Rc::new(m)
    }
}

/// What a command printed, and its exit code.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ran {
    /// The command line as a person types it (the language is said only when it is Japanese).
    pub command: String,
    pub code: u8,
    pub out: String,
    pub err: String,
}

impl Ran {
    fn json(&self) -> Value {
        json!({ "command": self.command, "code": self.code, "out": self.out, "err": self.err })
    }
}

/// The words of a command line, `--lang <l>` last, and the line a person types.
fn words(args: &[&str], lang: Lang) -> (Vec<String>, String) {
    let mut v: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let mut shown = format!("ritsu {}", args.join(" "));
    if lang == Lang::Ja {
        shown.push_str(" --lang ja");
    }
    v.push("--lang".into());
    v.push(lang.code().into());
    (v, shown)
}

fn utf8(b: Vec<u8>) -> String {
    String::from_utf8(b).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

/// A command that takes the writers it prints to.
fn ran(shown: String, f: impl FnOnce(&mut Vec<u8>, &mut Vec<u8>) -> u8) -> Ran {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = f(&mut out, &mut err);
    Ran { command: shown, code, out: utf8(out), err: utf8(err) }
}

/// `ritsu check .`: the text, and the JSON (`--format json`) the page marks the files with.
pub fn check(r: &Request) -> Value {
    fs::with(r.memory(), || {
        let (args, shown) = words(&["check", "."], r.lang);
        let text = ran(shown, |o, e| ritsu::check::run(&args[1..], r.lang, o, e));
        let (args, shown) = words(&["check", ".", "--format", "json"], r.lang);
        let json = ran(shown, |o, e| ritsu::check::run(&args[1..], r.lang, o, e));
        let mut v = text.json();
        v["json"] = serde_json::from_str(&json.out).unwrap_or(Value::Null);
        v
    })
}

/// The language of a file, by its extension.
fn tool_of(path: &str) -> Option<Tool> {
    ritsu_project::project::kind_of(path)
}

/// The targets of the generators that take one, the first the one a file opens on.
fn targets(t: Tool) -> Vec<&'static str> {
    match t {
        Tool::Dandori => dandori::commands::TARGETS.to_vec(),
        Tool::Chobo => chobo::target::Target::ALL.iter().map(|t| t.name()).collect(),
        Tool::Yuen => vec!["reqif", "prov"],
        _ => vec![],
    }
}

/// One file's generator, by its language: `{"tool", "command", "code", "out", "err", "files",
/// "targets", "target"}`, where `files` are what it wrote, by their paths from the working
/// directory. `{"tool", "none": true}` for a language that generates nothing.
pub fn generate(r: &Request) -> Value {
    let Some(tool) = tool_of(&r.path) else {
        return json!({ "tool": Value::Null, "none": true });
    };
    let ts = targets(tool);
    let target = r.target.clone().filter(|t| ts.contains(&t.as_str())).or_else(|| ts.first().map(|t| t.to_string()));
    let m = r.memory();
    let p = r.path.as_str();
    let l = r.lang;
    let did = fs::with(m.clone(), || -> Option<Ran> {
        let joined = Joined::new();
        Some(match tool {
            Tool::Rulec => {
                let (_, shown) = words(&["rulec", "gen", p, "--out", OUT], l);
                rulec_lang(l, || ran(shown, |o, e| rulec::codegen::generate(&[p], OUT, false, false, o, e)))
            }
            Tool::Koyomi => koyomi_gen(p, l),
            Tool::Chobo => chobo_build(p, l, target.as_deref().unwrap_or_default()),
            Tool::Dandori => {
                let (args, shown) = words(&["dandori", "build", p, "--target", target.as_deref().unwrap_or_default(), "--out", OUT], l);
                ran(shown, |o, e| dandori::cli::run(&args[1..], joined.rules(), o, e))
            }
            Tool::Sakai => {
                let (args, shown) = words(&["sakai", "export", "cml", p, "--root", "."], l);
                ran(shown, |o, e| sakai::run::run(&args[1..], joined.sakai(), o, e))
            }
            Tool::Yuen => {
                let (args, shown) = words(&["yuen", "export", target.as_deref().unwrap_or_default(), p, "--root", "."], l);
                ran(shown, |o, e| yuen::run::run(&args[1..], joined.yuen(), o, e))
            }
            Tool::Geas | Tool::Proto | Tool::File => return None,
        })
    });
    let Some(did) = did else {
        return json!({ "tool": tool.word(), "none": true });
    };
    let files: Vec<Value> = m.written().into_iter().map(|(path, body)| json!({ "path": path, "body": utf8(body) })).collect();
    let mut v = did.json();
    v["tool"] = json!(tool.word());
    v["files"] = Value::Array(files);
    v["targets"] = json!(ts);
    v["target"] = json!(target);
    v
}

/// One file's page for whoever approves it, by its language: `{"tool", "html": {…}, "markdown":
/// {…}}`, each what the command prints, the page on `out`. `{"tool", "none": true}` for a language
/// that draws no page.
pub fn doc(r: &Request) -> Value {
    let Some(tool) = tool_of(&r.path) else {
        return json!({ "tool": Value::Null, "none": true });
    };
    let p = r.path.as_str();
    let l = r.lang;
    let pages = fs::with(r.memory(), || -> Option<(Ran, Ran)> {
        let joined = Joined::new();
        let one = |html: bool| -> Ran {
            match tool {
                Tool::Rulec => rulec_doc(p, l, html),
                Tool::Koyomi => koyomi_doc(p, l, html),
                Tool::Chobo => chobo_doc(p, l, html),
                _ => {
                    let w: &[&str] = if html { &["dandori", "doc", p, "--format", "html"] } else { &["dandori", "doc", p] };
                    let (args, shown) = words(w, l);
                    ran(shown, |o, e| dandori::cli::run(&args[1..], joined.rules(), o, e))
                }
            }
        };
        match tool {
            Tool::Rulec | Tool::Koyomi | Tool::Chobo | Tool::Dandori => Some((one(true), one(false))),
            _ => None,
        }
    });
    match pages {
        Some((html, md)) => json!({ "tool": tool.word(), "html": html.json(), "markdown": md.json() }),
        None => json!({ "tool": tool.word(), "none": true }),
    }
}

/// One request of the page, as the module takes it and answers: `what` is check, gen or doc.
pub fn answer(what: &str, request: &str) -> String {
    let v: Value = match serde_json::from_str(request) {
        Ok(v) => v,
        Err(e) => return json!({ "error": format!("the request is not JSON: {e}") }).to_string(),
    };
    let r = match Request::from_json(&v) {
        Ok(r) => r,
        Err(e) => return json!({ "error": e }).to_string(),
    };
    match what {
        "check" => check(&r),
        "gen" => generate(&r),
        "doc" => doc(&r),
        _ => json!({ "error": format!("there is no command `{what}`") }),
    }
    .to_string()
}

// ── rulec ────────────────────────────────────────────────────────────────────────────────────

/// rulec as `ritsu rulec` runs it: its sentences in the language of the thread (rulec's
/// `i18n::with`), as its command sets it from `--lang`, and the days of a `range from koyomi` read
/// through koyomi's port (rulec's `days::with`, ritsu's DESIGN 7.5 (b)).
fn rulec_lang<R>(l: Lang, f: impl FnOnce() -> R) -> R {
    rulec::days::with(Some(Joined::dates()), || rulec::i18n::with(if l == Lang::Ja { rulec::i18n::Lang::Ja } else { rulec::i18n::Lang::En }, f))
}

/// `rulec doc <path> [--format html]` (rulec's `cli::doc`, one file, no `--out`): nothing is
/// rendered from a rule that does not pass check (rulec's §1.6).
fn rulec_doc(path: &str, l: Lang, html: bool) -> Ran {
    let w: &[&str] = if html { &["rulec", "doc", path, "--format", "html"] } else { &["rulec", "doc", path] };
    let (_, shown) = words(w, l);
    let say = |t: Text| format!("{}\n", t.get(l));
    rulec_lang(l, || {
        let mut r = Ran { command: shown, ..Ran::default() };
        let Ok(src) = fs::read_to_string(path) else {
            r.err = say(tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            r.code = 2;
            return r;
        };
        let rep = rulec::report(&src, path);
        if rulec::has_error(&rep.diags) {
            let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
            for d in rep.diags.iter().filter(|d| d.severity == rulec::diag::Severity::Error) {
                r.out.push_str(&rulec::diag::render(d, &lines));
                r.out.push('\n');
            }
            r.err = say(tr!(
                "error: `{path}` は検査を通っていないので資料を書き出しません（§1.6）",
                "error: `{path}` does not pass check, so it is not rendered (§1.6)"
            ));
            r.code = 1;
            return r;
        }
        let Ok((f, c)) = rulec::prepare(&src, path) else {
            r.err = say(tr!("error: `{path}` は検査を通っていません", "error: `{path}` does not pass check"));
            r.code = 1;
            return r;
        };
        r.out = if html {
            let js = rulec::codegen::Gen::new(&f, &c, &src, path).javascript();
            rulec::doc::render_html(&f, &c, &src, path, &js)
        } else {
            rulec::doc::render(&f, &c, &src, path)
        };
        r
    })
}

// ── koyomi ───────────────────────────────────────────────────────────────────────────────────

fn head(l: Lang) -> &'static str {
    if l == Lang::Ja { "エラー" } else { "error" }
}

/// `koyomi gen <path> --out generated` (koyomi's `run::gen_cmd`, one file, every target).
fn koyomi_gen(path: &str, l: Lang) -> Ran {
    use koyomi::calendar::Loader;
    use koyomi::check::{self, Options};
    use koyomi::naming::TARGETS;
    let (_, shown) = words(&["koyomi", "gen", path, "--out", OUT], l);
    let mut r = Ran { command: shown, ..Ran::default() };
    let refuse = |mut r: Ran, t: Text| {
        r.err.push_str(&format!("{}: {}\n", head(l), t.get(l)));
        r.code = 2;
        r
    };
    let mut loader = Loader::default();
    let o = match check::check_file(path, &Options::default(), &mut loader) {
        Ok(o) => o,
        Err(e) => return refuse(r, tr!("`{path}` を読めません: {e}", "cannot read `{path}`: {e}")),
    };
    let Some(checked) = o.checked.as_ref().filter(|_| !o.has_errors()) else {
        r.out.push_str(&check::render(&o, l));
        let why = tr!("`{path}` は検査を通らないので、生成しません", "`{path}` does not pass check, so nothing is generated from it");
        r.err.push_str(&format!("{}: {}\n", head(l), why.get(l)));
        r.code = 1;
        return r;
    };
    let u = koyomi::codegen::unit_of(checked, l);
    let planned: Vec<(String, String)> = TARGETS.iter().flat_map(|t| koyomi::codegen::files(&u, *t)).collect();
    for (rel, body) in planned {
        let p = Path::new(OUT).join(&rel);
        let shown = p.to_string_lossy().to_string();
        if let Some(dir) = p.parent()
            && fs::create_dir_all(dir).is_err()
        {
            return refuse(r, tr!("`{}` を作れません", "cannot create `{}`", dir.display()));
        }
        if fs::write(&p, body).is_err() {
            return refuse(r, tr!("`{shown}` に書けません", "cannot write `{shown}`"));
        }
        r.out.push_str(&format!("{}\n", tr!("生成しました: {shown}", "generated: {shown}").get(l)));
    }
    r
}

/// `koyomi doc <path> [--format html]` (koyomi's `run::doc_cmd`, the months the file decides).
fn koyomi_doc(path: &str, l: Lang, html: bool) -> Ran {
    use koyomi::calendar::Loader;
    use koyomi::check::{self, Options};
    let w: &[&str] = if html { &["koyomi", "doc", path, "--format", "html"] } else { &["koyomi", "doc", path] };
    let (_, shown) = words(w, l);
    let mut r = Ran { command: shown, ..Ran::default() };
    let format = if html { koyomi::doc::Format::Html } else { koyomi::doc::Format::Markdown };
    let mut loader = Loader::default();
    let o = match check::check_file(path, &Options::default(), &mut loader) {
        Ok(o) => o,
        Err(e) => {
            r.err = format!("{}: {}\n", head(l), tr!("`{path}` を読めません: {e}", "cannot read `{path}`: {e}").get(l));
            r.code = 2;
            return r;
        }
    };
    match koyomi::doc::page(&o, l, &koyomi::doc::Options { months: None }) {
        Ok(p) => r.out = koyomi::doc::render(&p, format),
        Err(ds) => {
            for d in &ds {
                r.err.push_str(&d.render(l));
            }
            let msg = tr!("`{path}` にはページを作れないエラーがあるので、ページを出しません", "`{path}` has errors that keep its page from being made");
            r.err.push_str(&format!("{}: {}\n", head(l), msg.get(l)));
            r.code = 1;
        }
    }
    r
}

// ── chobo ────────────────────────────────────────────────────────────────────────────────────

/// chobo's `run::load`: the book checked, its diagnostics on standard error, and the book when it
/// has no error (else the exit code).
fn chobo_load(path: &str, l: Lang, r: &mut Ran) -> Option<(String, chobo::model::Book, chobo::check::Checked)> {
    use chobo::diag::Show;
    let (src, c) = match chobo::check::check_file(Path::new(path)) {
        Ok(x) => x,
        Err(e) => {
            r.err.push_str(&format!("{e}\n"));
            r.code = 2;
            return None;
        }
    };
    for d in &c.diags {
        r.err.push_str(&d.shown(path, &src, l));
    }
    if chobo::diag::has_errors(&c.diags) {
        r.err.push_str(&format!("{}\n", chobo::diag::summary(path, &c.diags, l)));
        r.code = 1;
        return None;
    }
    let book = c.book.clone().expect("a book with no error is read");
    Some((src, book, c))
}

/// `chobo build <path> --target <target> --out generated` (chobo's `run::cmd_build`).
fn chobo_build(path: &str, l: Lang, target: &str) -> Ran {
    use chobo::diag::Show;
    let (_, shown) = words(&["chobo", "build", path, "--target", target, "--out", OUT], l);
    let mut r = Ran { command: shown, ..Ran::default() };
    let Some(t) = chobo::target::Target::parse(target) else {
        r.err = format!("there is no target `{target}`\n");
        r.code = 2;
        return r;
    };
    let Some((src, book, _)) = chobo_load(path, l, &mut r) else { return r };
    let stem = Path::new(path).file_name().map(|f| f.to_string_lossy().trim_end_matches(".book").to_string()).unwrap_or_default();
    let files = match chobo::target::build(&book, &stem, t) {
        Ok(f) => f,
        Err(diags) => {
            for d in &diags {
                r.err.push_str(&d.shown(path, &src, l));
            }
            r.err.push_str(&format!("{}\n", chobo::diag::summary(path, &diags, l)));
            r.code = 1;
            return r;
        }
    };
    for (rel, text) in &files {
        let p = Path::new(OUT).join(rel);
        if let Err(e) = p.parent().map(fs::create_dir_all).unwrap_or(Ok(())).and_then(|_| fs::write(&p, text)) {
            r.err.push_str(&format!("{}: {e}\n", p.display()));
            r.code = 2;
            return r;
        }
        let shown = p.display();
        r.err.push_str(&format!("{}\n", tr!("{shown} を書きました", "wrote {shown}").get(l)));
    }
    r
}

/// `chobo doc <path> [--format html]` (chobo's `run::cmd_doc`, no `--out`).
fn chobo_doc(path: &str, l: Lang, html: bool) -> Ran {
    let w: &[&str] = if html { &["chobo", "doc", path, "--format", "html"] } else { &["chobo", "doc", path] };
    let (_, shown) = words(w, l);
    let mut r = Ran { command: shown, ..Ran::default() };
    let Some((src, book, c)) = chobo_load(path, l, &mut r) else { return r };
    let rep = c.report.unwrap_or_else(|| chobo::check::report(&book));
    let input = chobo::doc::Input { book: &book, file: path, src: &src, diags: &c.diags, report: &rep, lang: l };
    r.out = if html { chobo::doc::html(&input) } else { chobo::doc::markdown(&input) };
    r
}
