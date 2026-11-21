//! The `yuen` command as a function (ritsu's DESIGN 3.3): the binary of yuen's own crate runs it
//! with no other language joined, and `ritsu yuen` with every one (ritsu's DESIGN 8.6). What
//! exists and what it takes is `cli.rs`'s table.

use crate::cli::{self, Args};
use crate::suite::Suite;
use ritsu_base::text::{Lang, Text};
use std::io::Write;

fn refuse(w: &mut dyn Write, msg: Text, lang: Lang) -> u8 {
    // in the words of every command of ritsu (ritsu's DESIGN 6.2, item 10)
    ritsu_base::cli::refuse(w, &msg, lang)
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

/// The `yuen` command: `args` without the program's name, the languages `suite` joins (none for
/// the binary of yuen's own crate, every one for `ritsu yuen`), and where to print. The exit code.
pub fn run(args: &[String], suite: Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    // a line that asks for JSON is answered in JSON when the command stops before its work too: the
    // last value on standard output is `{"error": …}` (ritsu's DESIGN 6.2, item 10)
    ritsu_base::cli::answering(args, out, |out| command(args, suite, out, err))
}

fn command(args: &[String], suite: Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    crate::check::COMMAND.with(|c| *c.borrow_mut() = Some(args.to_vec()));
    let code = run_command(args, suite, out, err);
    crate::check::COMMAND.with(|c| *c.borrow_mut() = None);
    code
}

fn run_command(args: &[String], suite: Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let args = args.to_vec();
    let lang = Lang::pick(lang_flag(&args).as_deref(), "YUEN_LANG");
    let table = cli::table();
    let Some(first) = args.first() else {
        let _ = write!(err, "{}", table.help_all(lang));
        return (2) as u8;
    };
    match first.as_str() {
        "--help" | "-h" => {
            let _ = write!(out, "{}", table.help_all(lang));
            return 0;
        }
        "--version" | "-V" => {
            let _ = writeln!(out, "yuen {}", env!("CARGO_PKG_VERSION"));
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
                    None => refuse(err, ritsu_base::cli::no_command("yuen", n), lang),
                },
            };
        }
        _ => {}
    }
    let Some(cmd) = table.command(first) else {
        return refuse(err, ritsu_base::cli::no_command("yuen", first), lang);
    };
    let a = match table.parse(cmd, &args[1..]) {
        Ok(a) => a,
        Err(e) => return refuse(err, e, lang),
    };
    if a.has("--help") {
        let _ = write!(out, "{}", table.help_cmd(cmd, lang));
        return 0;
    }
    // a path that starts with `@`, in a module, is read as a path written in a file is (ritsu's DESIGN 17.6)
    if let Err(t) = ritsu_base::modules::given_all(&a.pos) {
        return refuse(err, t, lang);
    }
    // The word `export` and `source` take before their paths and the code `explain` takes are no
    // paths; every other positional argument is one.
    let (first, end) = match cmd.name {
        "export" | "source" => (1, a.pos.len()),
        "explain" => (0, 0),
        _ => (0, a.pos.len()),
    };
    // the paths the run holds are written the way the first path given was (ritsu's DESIGN 4.15),
    // and so are the paths given: `./refunds.req` and `x/../refunds.req` are `refunds.req` (6.2
    // item 8)
    let _way = ritsu_base::paths::run_from(a.pos.get(first).filter(|_| first < end).map_or(".", String::as_str));
    let mut a = a;
    for p in a.pos.iter_mut().take(end).skip(first) {
        *p = ritsu_base::paths::show_given(p);
    }
    // and so are the paths the flags take: `--out ./refunds.md` is `refunds.md`, and so are the
    // files of `--diff`, of `--map <spec.geas>=<record>` and of `--at <file.req>:<line>`
    for (flag, v) in &mut a.got {
        let held = match *flag {
            "--out" | "--diff" => ritsu_base::paths::fold_given(v),
            "--map" => match v.split_once('=') {
                Some((spec, record)) => format!("{}={}", ritsu_base::paths::fold_given(spec), ritsu_base::paths::fold_given(record)),
                None => continue,
            },
            "--at" => match v.rsplit_once(':') {
                Some((file, line)) => format!("{}:{line}", ritsu_base::paths::fold_given(file)),
                None => continue,
            },
            _ => continue,
        };
        *v = held;
    }
    // A directory given is walked without entering another module below it; when the walk did not
    // start in a module, one line each says which it passed over (ritsu's DESIGN 17.6).
    let (paths, command): (&[String], String) = match cmd.name {
        "check" | "review" | "trace" | "affected" | "doc" | "api" => (&a.pos, format!("yuen {}", cmd.name)),
        "export" | "source" => (a.pos.get(1..).unwrap_or(&[]), format!("yuen {} {}", cmd.name, a.pos.first().map(String::as_str).unwrap_or(""))),
        _ => (&[], String::new()),
    };
    for m in crate::project::other_modules(paths) {
        let _ = writeln!(err, "{}", ritsu_base::paths::other_module_note(&m, &command).get(lang));
    }
    match cmd.name {
        "check" => check_cmd(&a, lang, &suite, out, err),
        "review" => review_cmd(&a, lang, &suite, out, err),
        "trace" => trace_cmd(&a, lang, &suite, out, err),
        "affected" => crate::affected::command(&a, lang, &suite, out, err),
        "doc" => doc_cmd(&a, lang, &suite, out, err),
        "api" => api_cmd(&a, lang, &suite, out, err),
        "export" => export_cmd(&a, lang, &suite, out, err),
        "source" => source_cmd(&a, lang, &suite, out, err),
        "explain" => explain_cmd(&a, lang, &suite, out, err),
        _ => unreachable!("every command in the table is dispatched"),
    }
}

fn check_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if a.pos.is_empty() {
        return refuse(err, cli::table().takes_of("check"), lang);
    }
    let c = match crate::check::check_with(&a.pos, a.get("--root"), suite.clone()) {
        Ok(c) => c,
        Err(r) => return refuse(err, r.0, lang),
    };
    let label = a.pos.join(", ");
    if a.get("--format") == Some("json") {
        let _ = writeln!(out, "{}", crate::check::to_json(&c, &label, lang));
    } else {
        let _ = write!(out, "{}", crate::check::render(&c, &label, lang));
    }
    exit_of(&c)
}

/// 2 when a language the project names is not joined (E206: where yuen runs, not what the
/// project says), 1 for any other error, else 0.
fn exit_of(c: &crate::check::Checked) -> u8 {
    if crate::check::has_unjoined(&c.diags) {
        2
    } else if c.has_errors() {
        1
    } else {
        0
    }
}

fn review_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if a.pos.is_empty() {
        return refuse(err, cli::table().takes_of("review"), lang);
    }
    let Some(by) = a.get("--by") else {
        return refuse(err, cli::table().needs_of("review", "--by <role>"), lang);
    };
    let choice = crate::review::Choice { at: a.all("--at"), requirements: a.all("--requirement"), all: a.has("--all") };
    match crate::review::review_with(&a.pos, a.get("--root"), &choice, by, a.get("--date"), suite.clone()) {
        Ok(o) => {
            let _ = write!(out, "{}", crate::review::render(&o, lang));
            (o.exit) as u8
        }
        Err(r) => refuse(err, r.0, lang),
    }
}

/// Check a project for a command built on the check: None, with the diagnostics on standard
/// error and exit 1, when its words or names have errors.
pub(crate) fn checked(a: &Args, lang: Lang, cmd: &str, suite: &Suite, err: &mut dyn Write) -> Result<crate::check::Checked, u8> {
    checked_paths(&a.pos, a.get("--root"), lang, cmd, suite, err)
}

pub(crate) fn checked_paths(paths: &[String], root: Option<&str>, lang: Lang, cmd: &str, suite: &Suite, err: &mut dyn Write) -> Result<crate::check::Checked, u8> {
    if paths.is_empty() {
        return Err(refuse(err, cli::table().takes_of(cmd), lang));
    }
    let c = match crate::check::check_with(paths, root, suite.clone()) {
        Ok(c) => c,
        Err(r) => return Err(refuse(err, r.0, lang)),
    };
    if !c.named() {
        let label = paths.join(", ");
        let _ = write!(err, "{}", crate::check::render(&c, &label, lang));
        return Err(exit_of(&c));
    }
    Ok(c)
}

/// `yuen export reqif|prov <path>...` (DESIGN 12, 13).
fn export_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let what = a.pos.first().map(|s| s.as_str()).unwrap_or("");
    if !matches!(what, "reqif" | "prov") {
        return refuse(err, cli::table().takes_of("export"), lang);
    }
    if what == "reqif" && a.has("--format") {
        return refuse(err, tr!("`--format` は prov のときだけ書けます（ReqIF の形は一つです）", "`--format` is for prov only (ReqIF has one form)"), lang);
    }
    if what == "prov" && a.has("--time") {
        return refuse(err, tr!("`--time` は reqif のときだけ書けます（PROV の時刻は、記録の日付だけです）", "`--time` is for reqif only (the times of PROV are the days the records give)"), lang);
    }
    let time = match a.get("--time") {
        None => None,
        Some(t) => match crate::export::reqif::read_time(t) {
            Some(x) => Some(x),
            None => return refuse(err, tr!("`--time {t}` は RFC 3339 の日時ではありません（`2026-10-03T09:00:00+09:00` のように、時差まで書いてください）", "`--time {t}` is not an RFC 3339 date and time (`2026-10-03T09:00:00+09:00`, with its offset)"), lang),
        },
    };
    let paths: Vec<String> = a.pos[1..].to_vec();
    let c = match checked_paths(&paths, a.get("--root"), lang, "export", suite, err) {
        Ok(c) => c,
        Err(code) => return code,
    };
    if crate::export::blocked(&c) {
        let label = paths.join(", ");
        let _ = write!(err, "{}", crate::check::render(&c, &label, lang));
        return (1) as u8;
    }
    let (p, m) = (c.project.as_ref().unwrap(), c.model.as_ref().unwrap());
    let g = crate::export::graph(p, m);
    let text = match what {
        "reqif" => match crate::export::reqif::write(&g, time.as_deref(), crate::api::VERSION) {
            Ok(t) => t,
            Err(r) => return refuse(err, r.0, lang),
        },
        _ if a.get("--format") == Some("json") => crate::export::prov::provjson(&g),
        _ => crate::export::prov::provn(&g),
    };
    match a.get("--out") {
        Some(file) => {
            if let Err(e) = ritsu_base::fs::write(file, &text) {
                return refuse(err, ritsu_base::cli::cannot_write(file, &e), lang);
            }
            let _ = writeln!(out, "{}", tr!("書き出しました: {file}", "wrote: {file}").get(lang));
        }
        None => {
            let _ = write!(out, "{text}");
        }
    }
    0
}

/// `yuen source fetch|pin|outdated <path>...` (DESIGN 14): the project read as far as its
/// names, and checked no further.
fn source_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let verb = a.pos.first().map(|s| s.as_str()).unwrap_or("");
    if !matches!(verb, "fetch" | "pin" | "outdated") {
        return refuse(err, cli::table().takes_of("source"), lang);
    }
    let paths: Vec<String> = a.pos[1..].to_vec();
    if paths.is_empty() {
        return refuse(err, cli::table().takes_of("source"), lang);
    }
    let label = paths.join(", ");
    let (project, mut diags) = match crate::project::load_with(&paths, a.get("--root"), suite.clone()) {
        Ok(x) => x,
        Err(r) => return refuse(err, r.0, lang),
    };
    let named = project.map(|mut p| {
        diags.extend(crate::project::check_names(&mut p));
        // `outdated` asks the sources a rule or a calendar pins: their languages are joined (E206)
        if verb == "outdated" && !crate::diag::has_errors(&diags) {
            diags.extend(crate::check::unjoined(&p, true));
        }
        p
    });
    let p = match named {
        Some(p) if !crate::diag::has_errors(&diags) => p,
        _ => {
            for d in &diags {
                let _ = write!(err, "{}", d.render(lang));
            }
            let (e, _) = crate::diag::count(&diags);
            let _ = writeln!(err, "{}", tr!("{label}: エラー {e} 件", "{label}: {}", ; ritsu_base::text::plural(e, "error", "errors")).get(lang));
            return if crate::check::has_unjoined(&diags) { 2 } else { 1 };
        }
    };
    let result = match verb {
        "fetch" => crate::fetch::fetch(&p),
        "pin" => {
            let (texts, o) = crate::fetch::pin(&p);
            for (fi, text) in texts {
                let f = &p.files[fi];
                if let Err(e) = ritsu_base::fs::write(&f.abs, text) {
                    return refuse(err, ritsu_base::cli::cannot_write(&f.display, &e), lang);
                }
            }
            Ok(o)
        }
        _ => crate::fetch::outdated(&p),
    };
    match result {
        Ok(o) => {
            if o.lines.is_empty() {
                let _ = writeln!(out, "{}", tr!("{label}: 出典の宣言がありません", "{label}: no source is declared").get(lang));
            }
            for l in &o.lines {
                let _ = writeln!(out, "{}", l.get(lang));
            }
            (if verb == "outdated" && o.changed { 1 } else { 0 }) as u8
        }
        Err(e) => refuse(err, e, lang),
    }
}

fn trace_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let picks: Vec<crate::trace::Start> = [
        a.get("--requirement").map(|s| crate::trace::Start::Requirement(s.to_string())),
        a.get("--artifact").map(|s| crate::trace::Start::Artifact(s.to_string())),
        a.get("--source").map(|s| crate::trace::Start::Source(s.to_string())),
    ]
    .into_iter()
    .flatten()
    .collect();
    if picks.len() != 1 {
        return refuse(err, cli::table().one_of("trace", &["--requirement", "--artifact", "--source"]), lang);
    }
    let c = match checked(a, lang, "trace", suite, err) {
        Ok(c) => c,
        Err(code) => return code,
    };
    match crate::trace::trace(&c, &picks[0]) {
        Ok(t) => {
            if a.get("--format") == Some("json") {
                let _ = writeln!(out, "{}", serde_json::to_string_pretty(&t.json).unwrap());
            } else {
                let _ = write!(out, "{}", t.render(lang));
            }
            0
        }
        Err(r) => refuse(err, r.0, lang),
    }
}

/// `yuen doc <path>... [--format markdown|html] [--out <dir>]` (DESIGN 10): no page while the
/// words, names, sources or artifacts have errors (stages 1 to 4); the marks, gaps and what the
/// scope misses (stages 5 to 7) are on the page, and it exits 0.
fn doc_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let c = match checked(a, lang, "doc", suite, err) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let label = a.pos.join(", ");
    if crate::doc::blocked(&c) {
        let _ = write!(err, "{}", crate::check::render(&c, &label, lang));
        return exit_of(&c);
    }
    // the page is in `--lang`, else English, whatever the environment says (ritsu's DESIGN 4.1)
    let made = Lang::made(a.get("--lang"));
    let page = crate::doc::page(&c, &label, made);
    let html = a.get("--format") == Some("html");
    let text = if html { crate::doc::html::render(&page, made) } else { crate::doc::markdown::render(&page, made) };
    match a.get("--out") {
        Some(dir) => {
            let p = c.project.as_ref().unwrap();
            let first = p.files.first().map(|f| f.abs.clone()).unwrap_or_default();
            let stem = first.file_name().map(|n| n.to_string_lossy().trim_end_matches(".req").to_string()).unwrap_or_else(|| "yuen".into());
            let file = std::path::Path::new(dir).join(format!("{stem}.{}", if html { "html" } else { "md" }));
            let shown = file.to_string_lossy().to_string();
            if let Err(e) = ritsu_base::fs::create_dir_all(dir) {
                return refuse(err, ritsu_base::cli::cannot_create(dir, &e), lang);
            }
            if let Err(e) = ritsu_base::fs::write(&file, &text) {
                return refuse(err, ritsu_base::cli::cannot_write(&shown, &e), lang);
            }
            let _ = writeln!(out, "{}", tr!("書き出しました: {shown}", "wrote: {shown}").get(lang));
        }
        None => {
            let _ = write!(out, "{text}");
        }
    }
    0
}

fn api_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let c = match checked(a, lang, "api", suite, err) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let label = a.pos.join(", ");
    // the JSON's sentences are in `--lang`, else English, whatever the environment says (ritsu's
    // DESIGN 4.1)
    match crate::api::api(&c, &label, Lang::made(a.get("--lang"))) {
        Some(v) => {
            let _ = writeln!(out, "{}", serde_json::to_string_pretty(&v).unwrap());
            0
        }
        None => (1) as u8,
    }
}

fn explain_cmd(a: &Args, lang: Lang, _suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let format = a.get("--format");
    let ledger = crate::codes::ledger();
    if a.has("--all") {
        // codes and `--all` are the two ways to call it, not one: what it takes
        if !a.pos.is_empty() {
            return refuse(err, cli::table().takes_of("explain"), lang);
        }
        match format {
            Some("markdown") => {
                let _ = write!(out, "{}", ledger.render_markdown(lang));
            }
            Some("json") => {
                let _ = writeln!(out, "{}", ritsu_base::json::Json::arr(ledger.entries.iter().map(|e| ledger.to_json(e, lang))).pretty());
            }
            _ => {
                for (i, e) in ledger.entries.iter().enumerate() {
                    if i > 0 {
                        let _ = writeln!(out);
                    }
                    let _ = write!(out, "{}", ledger.render_text(e, lang));
                }
            }
        }
        return 0;
    }
    if a.pos.is_empty() {
        return refuse(err, cli::table().takes_of("explain"), lang);
    }
    match ledger.look_up(&a.pos) {
        Ok(entries) => {
            let shape = match format {
                Some("markdown") => ritsu_base::ledger::Shape::Markdown,
                Some("json") => ritsu_base::ledger::Shape::Json,
                _ => ritsu_base::ledger::Shape::Text,
            };
            let _ = write!(out, "{}", ledger.explained(&entries, shape, lang));
            0
        }
        Err(unknown) => refuse(err, ritsu_base::cli::no_codes("yuen", &unknown), lang),
    }
}
