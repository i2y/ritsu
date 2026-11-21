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
//! - [`generate`]: one file's generator, on the target the page picked: `rulec gen --target <t>`,
//!   `koyomi gen --target <t>`, `chobo gen --target <language> --books <database>`, `dandori gen
//!   --target <t>`, `sekisho gen --target <t>`, `sakai export cml`, `yuen export reqif|prov` (the
//!   last three given the project's root, as `ritsu check` gives it them). A target's library is
//!   what is written, with nothing `--extra` asks for. geas and `.proto` files generate nothing.
//!   A file of ringi has what its command prints or writes of it in place of a generator's targets,
//!   by its kind: `ringi api` of every kind; of a request, `ringi run` (every scenario), `ringi
//!   extract --out generated` (the rules and the dates files it is lowered to) and `ringi vectors`;
//!   of a shared route, `ringi extract` too. What `ritsu gen` writes of a request is not on the
//!   page: it writes a package of every language of the project at once.
//! - [`doc`]: one file's page for the people who read it, as HTML and as Markdown: `rulec doc`,
//!   `koyomi doc`, `chobo doc`, `dandori doc`, `sekisho doc`, `ringi doc` (given the project's
//!   root, so a shared route's page lists the requests that take it).
//!
//! Each answer is what the command prints (`out`, `err`), its exit code, and what it writes. Where
//! a language's command takes the writers it prints to (ritsu's `check`, rulec's `gen`, dandori,
//! yuen, sakai, sekisho), the command itself runs; dandori's as `ritsu dandori` runs it, with a flow's rules,
//! dates files and books read in the same process (`ritsu::languages::dandori`). rulec's `doc`, koyomi's and chobo's commands print to the
//! process's own output, which a page has none of; for them the functions here call the functions
//! the command calls, in its order, and say what it says. crates/ritsu/tests/playground.rs holds
//! every answer to what the `ritsu` binary prints and writes in a directory holding the same files.

use ritsu_base::cli::{cannot_create, cannot_read, cannot_write, refusal};
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
    /// By their paths from the project's root, `/` between the parts, each with its bytes.
    pub files: Vec<(String, Vec<u8>)>,
    pub lang: Lang,
    pub path: String,
    pub target: Option<String>,
}

impl Request {
    /// `{"files": {path: text}, "lang": "en"|"ja", "path": …, "target": …}`. A file that is not
    /// UTF-8 text (a CSV in Shift_JIS that a calendar reads) is `{"base64": "…"}` in place of its
    /// text (DESIGN 17.17, 17.22), and goes into memory as its bytes.
    pub fn from_json(v: &Value) -> Result<Request, String> {
        let given = v["files"].as_object().ok_or("the request has no `files`")?;
        let mut files = Vec::new();
        for (p, held) in given {
            let rel = ritsu_base::paths::join(".", p).map_err(|e| e.text(p).en)?;
            if rel == "." {
                return Err(format!("`{p}` names the project's root, not a file"));
            }
            let bytes = match held {
                Value::String(text) => text.as_bytes().to_vec(),
                Value::Object(o) if o.len() == 1 && o.contains_key("base64") => o["base64"]
                    .as_str()
                    .and_then(ritsu_base::sources::base64_decode)
                    .ok_or_else(|| format!("the base64 of the file {p} cannot be read"))?,
                _ => return Err(format!("the file {p} is neither text nor {{\"base64\": …}}")),
            };
            files.push((rel, bytes));
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
        for (p, bytes) in &self.files {
            m.add(p, bytes.clone());
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
        // the text and the JSON from one check: the question checks each file once (DESIGN 4.15)
        let (args, shown) = words(&["check", "."], r.lang);
        let mut json = Vec::new();
        let text = ran(shown, |o, e| ritsu::check::run_both(&args[1..], r.lang, o, &mut json, e));
        let mut v = text.json();
        v["json"] = serde_json::from_str(&utf8(json)).unwrap_or(Value::Null);
        v
    })
}

/// The language of a file, by its extension.
fn tool_of(path: &str) -> Option<Tool> {
    ritsu_project::project::kind_of(path)
}

/// The targets of the generators that take one, in the order of the command's own list, the first
/// the one a file opens on (rulec's `python`, koyomi's `typescript`, sekisho's `cedar`). chobo's are
/// its pairs of a database and a language by their names (`tigerbeetle-go`), each the flags
/// `--target <language> --books <database>` of `chobo gen` ([`chobo_build`]). ringi's are its
/// commands that print or write what is made of a file of the kind `text` is ([`ringi_targets`]).
fn targets(t: Tool, text: &[u8]) -> Vec<&'static str> {
    match t {
        Tool::Rulec => rulec::targets::target_names().to_vec(),
        Tool::Koyomi => koyomi::naming::TARGETS.iter().map(|t| t.key()).collect(),
        Tool::Chobo => chobo::target::Target::ALL.iter().map(|t| t.name()).collect(),
        Tool::Dandori => dandori::commands::TARGETS.to_vec(),
        Tool::Sekisho => sekisho::r#gen::TARGETS.to_vec(),
        Tool::Yuen => vec!["reqif", "prov"],
        Tool::Ringi => ringi_targets(text),
        _ => vec![],
    }
}

/// What the page offers of a file of ringi, each a command of `ritsu ringi`, by the kind its first
/// word says (a line that is blank or a `#` comment is not read): `api` of every kind; `run`,
/// `extract` and `vectors` of a request, which has the scenarios and the ticket; `extract` of a
/// shared route, which lowers its table to a rule. A file that names no kind is read as a request,
/// whose commands say what is wrong with it.
fn ringi_targets(text: &[u8]) -> Vec<&'static str> {
    let text = String::from_utf8_lossy(text);
    let first = text.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#')).and_then(|l| l.split_whitespace().next());
    match first {
        Some("route") => vec!["api", "extract"],
        Some("organization" | "masters") => vec!["api"],
        _ => vec!["api", "run", "extract", "vectors"],
    }
}

/// One file's generator, by its language: `{"tool", "command", "code", "out", "err", "files",
/// "targets", "target"}`, where `files` are what it wrote, by their paths from the working
/// directory. `{"tool", "none": true}` for a language that generates nothing.
pub fn generate(r: &Request) -> Value {
    let Some(tool) = tool_of(&r.path) else {
        return json!({ "tool": Value::Null, "none": true });
    };
    let ts = targets(tool, r.files.iter().find(|(p, _)| *p == r.path).map(|(_, b)| b.as_slice()).unwrap_or_default());
    let target = r.target.clone().filter(|t| ts.contains(&t.as_str())).or_else(|| ts.first().map(|t| t.to_string()));
    let m = r.memory();
    let p = r.path.as_str();
    let l = r.lang;
    let t = target.as_deref().unwrap_or_default();
    let did = fs::with(m.clone(), || -> Option<Ran> {
        let joined = Joined::new();
        Some(match tool {
            Tool::Rulec => {
                let (_, shown) = words(&["rulec", "gen", p, "--target", t, "--out", OUT], l);
                let asked = rulec::targets::Asked::new(&[t], &[]).expect("the target is one of rulec's own");
                rulec_lang(l, || ran(shown, |o, e| rulec::codegen::generate(&[p], OUT, &asked, false, false, o, e)))
            }
            Tool::Koyomi => koyomi_gen(p, l, t),
            Tool::Chobo => chobo_build(p, l, t),
            Tool::Dandori => {
                let (args, shown) = words(&["dandori", "gen", p, "--target", t, "--out", OUT], l);
                ran(shown, |o, e| ritsu::languages::dandori(&args[1..], o, e))
            }
            Tool::Sakai => {
                let (args, shown) = words(&["sakai", "export", "cml", p, "--root", "."], l);
                ran(shown, |o, e| sakai::run::run(&args[1..], joined.sakai(), o, e))
            }
            Tool::Yuen => {
                let (args, shown) = words(&["yuen", "export", target.as_deref().unwrap_or_default(), p, "--root", "."], l);
                ran(shown, |o, e| yuen::run::run(&args[1..], joined.yuen(), o, e))
            }
            // a gate's Cedar (the schema and the policies, in their text and their JSON), or the code
            // of a language that asks the gate, which reads the rules and the dates of the package
            // `ritsu gen` writes; as `ritsu sekisho` writes them, with the rules, the dates and the
            // flows a gate reads joined
            Tool::Sekisho => {
                let (args, shown) = words(&["sekisho", "gen", p, "--target", t, "--root", ".", "--out", OUT], l);
                ran(shown, |o, e| sekisho::run::run(&args[1..], joined.sekisho().into(), o, e))
            }
            // a request's description, its scenarios run, the files it is lowered to, or its vectors,
            // as `ritsu ringi` prints or writes them, with the calendars, books, flows and gate it
            // reads joined
            Tool::Ringi => {
                let w: Vec<&str> = match t {
                    "extract" => vec!["ringi", "extract", p, "--out", OUT],
                    _ => vec!["ringi", t, p],
                };
                let (args, shown) = words(&w, l);
                ran(shown, |o, e| ritsu::languages::ringi(&args[1..], o, e))
            }
            // kaname writes into a package of ritsu gen only, and is not on the page yet (kaname's
            // DESIGN 19)
            Tool::Geas | Tool::Proto | Tool::Openapi | Tool::Asyncapi | Tool::Cedar | Tool::File | Tool::Kaname | Tool::Jsonschema => return None,
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

/// One file's page for the people who read it, by its language: `{"tool", "html": {…}, "markdown":
/// {…}}`, each what the command prints, the page on `out`. `{"tool", "none": true}` for a language
/// that draws no page.
pub fn doc(r: &Request) -> Value {
    let Some(tool) = tool_of(&r.path) else {
        return json!({ "tool": Value::Null, "none": true });
    };
    let p = r.path.as_str();
    let l = r.lang;
    // both pages from one check of the file: the question checks each file once (DESIGN 4.15)
    let pages = fs::with(r.memory(), || -> Option<(Ran, Ran)> {
        Some(match tool {
            Tool::Rulec => rulec_docs(p, l),
            Tool::Koyomi => koyomi_docs(p, l),
            Tool::Chobo => chobo_docs(p, l),
            Tool::Sekisho => {
                let (args, md) = words(&["sekisho", "doc", p, "--root", "."], l);
                let (_, html) = words(&["sekisho", "doc", p, "--root", ".", "--format", "html"], l);
                let [h, m] = sekisho::run::doc_pages(&args[1..], Joined::new().sekisho().into());
                let as_ran = |command: String, (code, out, err): (u8, String, String)| Ran { command, code, out, err };
                (as_ran(html, h), as_ran(md, m))
            }
            Tool::Dandori => {
                let j = Joined::new();
                let one = |w: &[&str]| -> Ran {
                    let (args, shown) = words(w, l);
                    ran(shown, |o, e| ritsu::languages::dandori_in(&j, &args[1..], o, e))
                };
                (one(&["dandori", "doc", p, "--format", "html"]), one(&["dandori", "doc", p]))
            }
            Tool::Ringi => {
                let one = |w: &[&str]| -> Ran {
                    let (args, shown) = words(w, l);
                    ran(shown, |o, e| ritsu::languages::ringi(&args[1..], o, e))
                };
                (one(&["ringi", "doc", p, "--root", ".", "--format", "html"]), one(&["ringi", "doc", p, "--root", "."]))
            }
            _ => return None,
        })
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

/// The two command lines of a file's page, `<tool> doc <path> --format html` and `<tool> doc
/// <path>`, and what each printed: `r` for both when the file has no page.
fn pages_of(tool: &str, path: &str, l: Lang, r: Ran) -> (Ran, Ran) {
    let (_, html) = words(&[tool, "doc", path, "--format", "html"], l);
    let (_, md) = words(&[tool, "doc", path], l);
    (Ran { command: html, ..r.clone() }, Ran { command: md, ..r })
}

/// `rulec doc <path>` as HTML (`--format html`) and as Markdown (rulec's `cli::doc`, one file, no
/// `--out`), both from one check of the rule: nothing is rendered from a rule that does not pass
/// check (rulec's §1.6).
fn rulec_docs(path: &str, l: Lang) -> (Ran, Ran) {
    let say = |t: Text| format!("{}\n", refusal(t.get(l), l));
    rulec_lang(l, || {
        let mut r = Ran::default();
        let src = match fs::read_to_string(path) {
            Ok(src) => src,
            Err(e) => {
                r.err = say(cannot_read(path, &e));
                r.code = 2;
                return pages_of("rulec", path, l, r);
            }
        };
        let rep = rulec::report(&src, path);
        if rulec::has_error(&rep.diags) {
            let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
            for d in rep.diags.iter().filter(|d| d.severity == rulec::diag::Severity::Error) {
                r.out.push_str(&rulec::diag::render(d, &lines));
                r.out.push('\n');
            }
            r.err = say(tr!(
                "`{path}` は検査を通っていないので資料を書き出しません",
                "`{path}` does not pass check, so it is not rendered"
            ));
            r.code = 1;
            return pages_of("rulec", path, l, r);
        }
        let Ok((f, c)) = rulec::prepare(&src, path) else {
            r.err = say(tr!("`{path}` は検査を通っていません", "`{path}` does not pass check"));
            r.code = 1;
            return pages_of("rulec", path, l, r);
        };
        let js = rulec::codegen::Gen::new(&f, &c, &src, path).page_javascript();
        let (html, md) = pages_of("rulec", path, l, r);
        (Ran { out: rulec::doc::render_html(&f, &c, &src, path, &js), ..html }, Ran { out: rulec::doc::render(&f, &c, &src, path), ..md })
    })
}

// ── koyomi ───────────────────────────────────────────────────────────────────────────────────

/// `koyomi gen <path> --target <target> --out generated`, the library of the target the page picked
/// (koyomi's `run::gen_cmd`, one file and one target, into a `generated/` the project has no file
/// in), or what stops that target (E016, koyomi's DESIGN 15), with nothing written.
fn koyomi_gen(path: &str, l: Lang, target: &str) -> Ran {
    use koyomi::calendar::Loader;
    use koyomi::check::{self, Options};
    let (_, shown) = words(&["koyomi", "gen", path, "--target", target, "--out", OUT], l);
    let Some(t) = koyomi::naming::TARGETS.into_iter().find(|t| t.key() == target) else {
        return Ran { command: shown, code: 2, err: format!("there is no target `{target}`\n"), ..Ran::default() };
    };
    let mut r = Ran { command: shown, ..Ran::default() };
    let refuse = |mut r: Ran, t: Text| {
        r.err.push_str(&format!("{}\n", refusal(t.get(l), l)));
        r.code = 2;
        r
    };
    let mut loader = Loader::default();
    let o = match check::check_given(path, &Options::default(), &mut loader) {
        Ok(o) => o,
        Err(e) => return refuse(r, cannot_read(path, &e)),
    };
    let Some(checked) = o.checked.as_ref().filter(|_| !o.has_errors()) else {
        r.out.push_str(&check::render(&o, l));
        let why = tr!("`{path}` は検査を通らないので、生成しません", "`{path}` does not pass check, so nothing is generated from it");
        r.err.push_str(&format!("{}\n", refusal(why.get(l), l)));
        r.code = 1;
        return r;
    };
    let u = koyomi::codegen::unit_of(checked, l);
    let planned = match koyomi::codegen::build(&o, &u, t, false) {
        Ok(files) => files,
        Err(found) => {
            // each finding once, as `koyomi gen` says them, then why nothing is generated
            let mut stops: Vec<koyomi::diag::Diag> = Vec::new();
            for d in found {
                if !stops.iter().any(|s| s.place() == d.place() && s.message.en == d.message.en) {
                    stops.push(d);
                }
            }
            r.out.push_str(&stops.iter().map(|d| d.render(l)).collect::<String>());
            let why = tr!("`{path}` からは `{target}` のコードを生成できないので、何も生成しません", "`{path}` cannot be made into the code of `{target}`, so nothing is generated");
            r.err.push_str(&format!("{}\n", refusal(why.get(l), l)));
            r.code = 1;
            return r;
        }
    };
    for (rel, body) in planned {
        let p = Path::new(OUT).join(&rel);
        let shown = p.to_string_lossy().to_string();
        if let Some(dir) = p.parent()
            && let Err(e) = fs::create_dir_all(dir)
        {
            return refuse(r, cannot_create(&dir.display().to_string(), &e));
        }
        if let Err(e) = fs::write(&p, body) {
            return refuse(r, cannot_write(&shown, &e));
        }
        r.out.push_str(&format!("{}\n", tr!("生成しました: {shown}", "generated: {shown}").get(l)));
    }
    r
}

/// `koyomi doc <path>` as HTML (`--format html`) and as Markdown (koyomi's `run::doc_cmd`, the
/// months the file decides), both from one check of the file.
fn koyomi_docs(path: &str, l: Lang) -> (Ran, Ran) {
    use koyomi::calendar::Loader;
    use koyomi::check::{self, Options};
    let mut r = Ran::default();
    let mut loader = Loader::default();
    let o = match check::check_given(path, &Options::default(), &mut loader) {
        Ok(o) => o,
        Err(e) => {
            r.err = format!("{}\n", refusal(cannot_read(path, &e).get(l), l));
            r.code = 2;
            return pages_of("koyomi", path, l, r);
        }
    };
    match koyomi::doc::page(&o, l, &koyomi::doc::Options { months: None }) {
        Ok(p) => {
            let (html, md) = pages_of("koyomi", path, l, r);
            (Ran { out: koyomi::doc::render(&p, koyomi::doc::Format::Html), ..html }, Ran { out: koyomi::doc::render(&p, koyomi::doc::Format::Markdown), ..md })
        }
        Err(ds) => {
            for d in &ds {
                r.err.push_str(&d.render(l));
            }
            let msg = tr!("`{path}` にはページを作れないエラーがあるので、ページを出しません", "`{path}` has errors that keep its page from being made");
            r.err.push_str(&format!("{}\n", refusal(msg.get(l), l)));
            r.code = 1;
            pages_of("koyomi", path, l, r)
        }
    }
}

// ── chobo ────────────────────────────────────────────────────────────────────────────────────

/// chobo's `run::load`: the book checked, its diagnostics on standard error, and the book when it
/// has no error (else the exit code).
fn chobo_load(path: &str, l: Lang, r: &mut Ran) -> Option<(String, chobo::model::Book, chobo::check::Checked)> {
    use chobo::diag::Show;
    let (src, c) = match chobo::check::check_file(Path::new(path)) {
        Ok(x) => x,
        Err(e) => {
            r.err.push_str(&format!("{}\n", refusal(cannot_read(path, &e).get(l), l)));
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

/// `chobo gen <path> --target <language> --books <database> --out generated` for the pair the page
/// picked (`postgres-go`; chobo's `run::cmd_gen` with one target), written under `generated/<language>/`.
fn chobo_build(path: &str, l: Lang, target: &str) -> Ran {
    use chobo::diag::Show;
    let Some(t) = chobo::target::Target::parse(target) else {
        let (_, shown) = words(&["chobo", "gen", path, "--target", target, "--out", OUT], l);
        return Ran { command: shown, code: 2, err: format!("there is no target `{target}`\n"), ..Ran::default() };
    };
    let flags = t.flags();
    let mut w = vec!["chobo", "gen", path];
    w.extend(flags.split(' '));
    w.extend(["--out", OUT]);
    let (_, shown) = words(&w, l);
    let mut r = Ran { command: shown, ..Ran::default() };
    let Some((src, book, c)) = chobo_load(path, l, &mut r) else { return r };
    let stem = Path::new(path).file_name().map(|f| f.to_string_lossy().trim_end_matches(".book").to_string()).unwrap_or_default();
    let origin = chobo::target::Origin::named(path, src.as_bytes());
    let files = match chobo::target::build(&book, &stem, t, &origin) {
        Ok(f) => f,
        Err(found) => {
            // each finding once, as `chobo gen` says them, and a summary that counts the warnings
            // the load said as well
            let key = |d: &chobo::diag::Diag| (d.code, d.line, d.col, d.message.en.clone(), d.notes.iter().map(|n| n.en.clone()).collect::<Vec<_>>());
            let mut stops: Vec<chobo::diag::Diag> = Vec::new();
            for d in found {
                if !stops.iter().any(|s| key(s) == key(&d)) {
                    stops.push(d);
                }
            }
            for d in &stops {
                r.err.push_str(&d.shown(path, &src, l));
            }
            let mut all = c.diags.clone();
            all.extend(stops);
            r.err.push_str(&format!("{}\n", chobo::diag::summary(path, &all, l)));
            r.code = 1;
            return r;
        }
    };
    for (rel, text) in &files {
        let p = Path::new(OUT).join(t.lang().key()).join(rel);
        if let Err(e) = p.parent().map(fs::create_dir_all).unwrap_or(Ok(())).and_then(|_| fs::write(&p, text)) {
            r.err.push_str(&format!("{}\n", refusal(cannot_write(&p.display().to_string(), &e).get(l), l)));
            r.code = 2;
            return r;
        }
        let shown = p.display();
        r.err.push_str(&format!("{}\n", tr!("{shown} を書きました", "wrote {shown}").get(l)));
    }
    r
}

/// `chobo doc <path>` as HTML (`--format html`) and as Markdown (chobo's `run::cmd_doc`, no
/// `--out`), both from one check of the book.
fn chobo_docs(path: &str, l: Lang) -> (Ran, Ran) {
    let mut r = Ran::default();
    let Some((src, book, c)) = chobo_load(path, l, &mut r) else { return pages_of("chobo", path, l, r) };
    let rep = c.report.unwrap_or_else(|| chobo::check::report(&book));
    let input = chobo::doc::Input { book: &book, file: path, src: &src, diags: &c.diags, report: &rep, lang: l };
    let (html, md) = pages_of("chobo", path, l, r);
    (Ran { out: chobo::doc::html(&input), ..html }, Ran { out: chobo::doc::markdown(&input), ..md })
}
