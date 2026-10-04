//! The `sakai` command as a function (ritsu's DESIGN 3.3): the binary of sakai's own crate runs
//! it with no other language joined, and `ritsu sakai` with every one (ritsu's DESIGN 8.6). What
//! exists and what it takes is `cli.rs`'s table.

use crate::check;
use crate::cli::{self, Args};
use crate::paths;
use crate::suite::Suite;
use ritsu_base::text::{Lang, Text};
use std::io::Write;
use std::path::{Path, PathBuf};

fn refuse(err: &mut dyn Write, msg: Text, lang: Lang) -> u8 {
    let head = if lang == Lang::Ja { "エラー" } else { "error" };
    let _ = writeln!(err, "{head}: {}", msg.get(lang));
    2
}

/// `--lang ja`, `--lang=ja`, anywhere on the line: decided before anything is printed.
fn lang_flag(args: &[String]) -> Option<String> {
    for (i, a) in args.iter().enumerate() {
        if a == "--lang" {
            return args.get(i + 1).cloned();
        }
        if let Some(v) = a.strip_prefix("--lang=") {
            return Some(v.to_string());
        }
    }
    None
}

/// The `sakai` command: `args` without the program's name, the languages `suite` joins (none for
/// the binary of sakai's own crate, every one for `ritsu sakai`), and where to print. The exit
/// code.
pub fn run(args: &[String], suite: Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    crate::suite::COMMAND.with(|c| *c.borrow_mut() = Some(args.to_vec()));
    let code = run_command(args, &suite, out, err);
    crate::suite::COMMAND.with(|c| *c.borrow_mut() = None);
    paths::show_from_the_root();
    code
}

fn run_command(args: &[String], suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let args = args.to_vec();
    let lang = Lang::pick(lang_flag(&args).as_deref(), "SAKAI_LANG");
    let table = cli::table();
    let Some(first) = args.first() else {
        let _ = write!(err, "{}", table.help_all(lang));
        return 2;
    };
    match first.as_str() {
        "--help" | "-h" => {
            let _ = write!(out, "{}", table.help_all(lang));
            return 0;
        }
        "--version" | "-V" => {
            let _ = writeln!(out, "sakai {}", env!("CARGO_PKG_VERSION"));
            return 0;
        }
        "help" => {
            let mut rest = Vec::new();
            let mut i = 1;
            while i < args.len() {
                if args[i] == "--lang" {
                    i += 2;
                    continue;
                }
                if !args[i].starts_with("--lang=") {
                    rest.push(args[i].clone());
                }
                i += 1;
            }
            return match rest.first() {
                None => {
                    let _ = write!(out, "{}", table.help_all(lang));
                    0
                }
                Some(n) => match table.command(n) {
                    Some(c) => {
                        let _ = write!(out, "{}", table.help_cmd(c, lang));
                        0
                    }
                    None => refuse(err, tr!("`{n}` というコマンドはありません。`sakai --help` を読んでください", "there is no command `{n}`; run `sakai --help`"), lang),
                },
            };
        }
        _ => {}
    }
    let Some(cmd) = table.command(first) else {
        return refuse(err, tr!("`{first}` というコマンドはありません。`sakai --help` を読んでください", "there is no command `{first}`; run `sakai --help`"), lang);
    };
    let a = match table.parse(cmd, &args[1..]) {
        Ok(a) => a,
        Err(e) => return refuse(err, e, lang),
    };
    if a.has("--help") {
        let _ = write!(out, "{}", table.help_cmd(cmd, lang));
        return 0;
    }
    match cmd.name {
        "check" => check_cmd(&a, lang, suite, out, err),
        "build" => build_cmd(&a, lang, suite, out, err),
        "export" => export_cmd(&a, lang, suite, out, err),
        "api" => api_cmd(&a, lang, suite, out, err),
        "explain" => explain_cmd(&a, lang, suite, out, err),
        _ => unreachable!("every command in the table is dispatched"),
    }
}

/// The root (DESIGN 2.4): `--root`, else found from the first path.
fn root_of(a: &Args, first: &str) -> Result<PathBuf, Text> {
    root_from(a.get("--root"), first)
}

/// The root of `--root <flag>`, else found from the first path.
fn root_from(flag: Option<&str>, first: &str) -> Result<PathBuf, Text> {
    match flag {
        Some(r) => {
            let p = paths::absolute(Path::new(r));
            if p.is_dir() {
                Ok(p)
            } else {
                Err(tr!("`--root {r}` はディレクトリではありません", "`--root {r}` is not a directory"))
            }
        }
        None => Ok(paths::find_root(Path::new(first))),
    }
}

/// The paths of the command line, as paths from the root.
fn from_root(root: &Path, args: &[String]) -> Result<Vec<String>, Text> {
    args.iter()
        .map(|f| {
            if !Path::new(f).exists() {
                return Err(tr!("`{f}` がありません", "`{f}` is not there"));
            }
            paths::from_root(root, Path::new(f)).ok_or_else(|| {
                let r = root.display().to_string();
                tr!("`{f}` はルート {r} の外にあります", "`{f}` is outside the root {r}")
            })
        })
        .collect()
}

/// `sakai check <args>... [--root <root>]`, as `ritsu check` prints it (ritsu's DESIGN 8.3): for
/// each map, its findings, as the command prints them and as its `--format json` prints them,
/// then the line that says what was checked. `args` and `root` are as the person would write them,
/// from where the program runs; the findings' files are from the root.
pub fn checked(args: &[String], root: Option<&str>, suite: &Suite, lang: Lang) -> Vec<ritsu_ports::Checked> {
    use ritsu_ports::{Checked as Unit, Finding, Part, Verdict};
    let label = args.join(" ");
    let refused = |e: Text| {
        let head = if lang == Lang::Ja { "エラー" } else { "error" };
        vec![Unit::unchecked(&label, format!("{head}: {}\n", e.get(lang)))]
    };
    let Some(first) = args.first() else { return vec![] };
    let mut command = vec!["check".to_string()];
    command.extend(args.iter().cloned());
    if let Some(r) = root {
        command.extend(["--root".to_string(), r.to_string()]);
    }
    crate::suite::COMMAND.with(|c| *c.borrow_mut() = Some(command));
    let units = (|| -> Result<Vec<Unit>, Text> {
        let root = root_from(root, first)?;
        paths::show_from(paths::Shown::new(&root, first));
        let rel = from_root(&root, args)?;
        let outcomes = check::check_args_with(&root, &rel, suite)?;
        Ok(outcomes
            .iter()
            .map(|o| {
                let mut parts: Vec<Part> = o.diags.iter().map(|d| Part::Finding(Finding::of(d, (!d.rel.is_empty()).then(|| d.rel.clone()), lang))).collect();
                let tail = check::render(&check::Outcome { file: o.file.clone(), diags: vec![], summary: o.summary.clone(), checked: None, reads: vec![] }, lang);
                if !tail.is_empty() {
                    parts.push(Part::Text(tail));
                }
                Unit { label: paths::shown(&o.file), parts, verdict: if o.has_errors() { Verdict::Fails } else { Verdict::Passes } }
            })
            .collect())
    })();
    crate::suite::COMMAND.with(|c| *c.borrow_mut() = None);
    paths::show_from_the_root();
    units.unwrap_or_else(refused)
}

fn check_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let Some(first) = a.pos.first() else {
        return refuse(err, tr!("`sakai check` には map のファイルかディレクトリが要ります", "`sakai check` needs map files or directories"), lang);
    };
    let root = match root_of(a, first) {
        Ok(r) => r,
        Err(e) => return refuse(err, e, lang),
    };
    paths::show_from(paths::Shown::new(&root, first));
    let rel = match from_root(&root, &a.pos) {
        Ok(r) => r,
        Err(e) => return refuse(err, e, lang),
    };
    let outcomes = match check::check_args_with(&root, &rel, suite) {
        Ok(o) => o,
        Err(e) => return refuse(err, e, lang),
    };
    let json = a.get("--format") == Some("json");
    let mut errors = false;
    for o in &outcomes {
        errors |= o.has_errors();
        if json {
            let _ = writeln!(out, "{}", check::to_json(o, lang));
        } else {
            let _ = write!(out, "{}", check::render(o, lang));
        }
    }
    if errors { 1 } else { 0 }
}

/// The one map file a command takes: the root, the map from it, and the paths shown from here.
fn one_map(a: &Args, cmd: &str, lang: Lang, err: &mut dyn Write) -> Result<(PathBuf, String), u8> {
    let [map] = a.pos.as_slice() else {
        return Err(refuse(err, tr!("`sakai {cmd}` には map のファイルを一つ渡します", "`sakai {cmd}` takes one map file"), lang));
    };
    if Path::new(map).is_dir() {
        return Err(refuse(err, tr!("`sakai {cmd}` には map のファイルを渡します（ディレクトリではなく）", "`sakai {cmd}` takes a map file, not a directory"), lang));
    }
    let root = root_of(a, map).map_err(|e| refuse(err, e, lang))?;
    paths::show_from(paths::Shown::new(&root, map));
    let rel = from_root(&root, &a.pos).map_err(|e| refuse(err, e, lang))?;
    Ok((root, rel[0].clone()))
}

fn build_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let Some(target) = a.get("--target").and_then(crate::build::Target::from_word) else {
        return refuse(err, 
            tr!("`sakai build` には --target が要ります（import-linter | dependency-cruiser | archunit | go-arch-lint）", "`sakai build` needs --target (import-linter | dependency-cruiser | archunit | go-arch-lint)"),
            lang,
        );
    };
    let (root, map) = match one_map(a, "build", lang, err) {
        Ok(x) => x,
        Err(code) => return code,
    };
    let built = match crate::build::run_with(&root, &map, target, suite, a.get("--out").map(Path::new), a.has("--check"), lang) {
        Ok(b) => b,
        Err(e) => return refuse(err, e, lang),
    };
    let o = &built.outcome;
    for d in &o.diags {
        let _ = write!(out, "{}", d.render(lang));
    }
    if let Some(done) = &built.done {
        let _ = write!(out, "{}", crate::build::render_done(&root, done, lang));
    }
    if o.has_errors() { 1 } else { 0 }
}

fn export_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if a.pos.first().map(String::as_str) != Some("cml") {
        return refuse(err, tr!("`sakai export` のあとには、書き出す形 `cml` を書きます", "`sakai export` is followed by the form to write, `cml`"), lang);
    }
    let rest = Args { got: a.got.clone(), pos: a.pos[1..].to_vec() };
    let (root, map) = match one_map(&rest, "export cml", lang, err) {
        Ok(x) => x,
        Err(code) => return code,
    };
    let o = match check::check_map_with(&root, &map, suite) {
        Ok(o) => o,
        Err(e) => return refuse(err, e, lang),
    };
    if o.has_errors() {
        let _ = write!(err, "{}", check::render(&o, lang));
        return 1;
    }
    let c = o.checked.as_ref().expect("a map with no errors is checked through");
    let name = Path::new(&map).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let text = crate::cml::render(c, &name, lang);
    match a.get("--out") {
        Some(f) => {
            if let Err(e) = std::fs::write(f, &text) {
                let e = e.to_string();
                return refuse(err, tr!("{f} に書けません: {e}", "cannot write {f}: {e}"), lang);
            }
        }
        None => {
            let _ = write!(out, "{text}");
        }
    }
    0
}

fn api_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if a.pos.len() != 1 {
        return refuse(err, tr!("`sakai api` には map のファイルを一つ渡します", "`sakai api` takes one map file"), lang);
    }
    if Path::new(&a.pos[0]).is_dir() {
        return refuse(err, tr!("`sakai api` には map のファイルを渡します（ディレクトリではなく）", "`sakai api` takes a map file, not a directory"), lang);
    }
    let root = match root_of(a, &a.pos[0]) {
        Ok(r) => r,
        Err(e) => return refuse(err, e, lang),
    };
    paths::show_from(paths::Shown::new(&root, &a.pos[0]));
    let rel = match from_root(&root, &a.pos) {
        Ok(r) => r,
        Err(e) => return refuse(err, e, lang),
    };
    let o = match check::check_map_with(&root, &rel[0], suite) {
        Ok(o) => o,
        Err(e) => return refuse(err, e, lang),
    };
    if o.has_errors() {
        let _ = write!(err, "{}", check::render(&o, lang));
        return 1;
    }
    let v = crate::api::api(o.checked.as_ref().expect("a map with no errors is checked through"));
    let _ = writeln!(out, "{}", serde_json::to_string_pretty(&v).unwrap());
    0
}

fn explain_cmd(a: &Args, lang: Lang, _suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let md = a.get("--format") == Some("markdown");
    let ledger = crate::codes::ledger();
    if a.has("--all") {
        if !a.pos.is_empty() {
            return refuse(err, tr!("`--all` とコードは一緒に書けません", "`--all` takes no code"), lang);
        }
        if md {
            let _ = write!(out, "{}", ledger.render_markdown(lang));
        } else {
            for (i, e) in ledger.entries.iter().enumerate() {
                if i > 0 {
                    let _ = writeln!(out, );
                }
                let _ = write!(out, "{}", ledger.render_text(e, lang));
            }
        }
        return 0;
    }
    let [code] = a.pos.as_slice() else {
        return refuse(err, tr!("`sakai explain` には `E201` のようなコードを一つ渡します", "`sakai explain` takes one code, like `E201`"), lang);
    };
    match ledger.find(code) {
        Some(e) => {
            if md {
                let _ = write!(out, "{}", ledger.render_markdown_one(e, lang));
            } else {
                let _ = write!(out, "{}", ledger.render_text(e, lang));
            }
            0
        }
        None => refuse(err, tr!("`{code}` という診断のコードはありません", "there is no diagnostic code `{code}`"), lang),
    }
}
