//! What a command says when it cannot start its work (DESIGN 6.2, item 10), held for every command
//! of ritsu and of its ten languages. The commands are read from the help that lists them (`ritsu
//! --help`, `ritsu mod --help`, `ritsu <language> --help`), and the test holds a line for each: a
//! command a help lists that the test does not hold fails it, so that a command added is held to the
//! same words.
//!
//! Each command is given what stops it before its work, and says it in the words every command says
//! it in, on the standard error, and exits 2: after the word of an error in the language of the run
//! (`error:`, `エラー:`; geas puts its code in the head, `error[E080]:`), the sentence of the scene,
//! with the paths and the commands in `` ` ``.
//!
//! - A file that is not there, and a file that is not UTF-8.
//! - A flag the command does not have: the help it points to is named the way the command was run,
//!   `ritsu <language> <command> --help` through ritsu, `<language> <command> --help` by the
//!   language's name (the links of a release).
//! - No argument, to a command that takes one: what it takes or needs, and its usage line.
//! - A command the language does not have, and a code its `explain` does not know.
//! - With `--format json` on the line, the same sentence on the standard output too, as
//!   `{"error": {"code": 2, "message": …}}`: the last line, or the `error` of the one document the
//!   command writes. geas, asked for JSON with `--json`, writes its diagnostic in its own JSON
//!   (E080 for a flag it does not have, E081 for a file), and the sentence on the standard error.
//!
//! In English, and in Japanese with `--lang ja`. The other half of the scenes (what `--out` cannot
//! make, a flag's value) are held where each language's table is: they are said by ritsu-base's
//! table alone. Every tool's `explain` given several codes, codes with `--all`, codes it does not
//! have, and a flag given three times, is held by the second test; the sources write no head of an
//! error themselves (the last test).

mod common;

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;

const LANG_VARS: [&str; 11] = common::LANG_VARS;

/// A command and the words after it that have it read a file of the line first: `{F}` the file
/// (`none.<ext>`, or `bytes.<ext>`, which is not UTF-8), `{F.<ext>}` one of another extension, `{O}`
/// a directory to write in. Empty for a command that reads no file of the line.
type Line = (&'static str, &'static str);

/// The commands of a language, `ext` the extension of its files.
struct Language {
    name: &'static str,
    ext: &'static str,
    commands: &'static [Line],
}

const LANGUAGES: &[Language] = &[
    Language {
        name: "rulec",
        ext: "rule",
        commands: &[
            ("check", "{F}"),
            ("explain", ""),
            ("fmt", "{F}"),
            ("gen", "{F} --target typescript --out {O}"),
            ("test", "{F}"),
            ("coverage", "{F}"),
            ("vectors", "{F}"),
            ("run", "{F}"),
            ("source", "fetch {F}"),
            ("doc", "{F}"),
            ("api", "{F}"),
            ("graph", "{F}"),
            ("certificate", "{F}"),
            ("schema", "{F}"),
            ("adapter", "{F}"),
            ("verify", "{F} --adapter true"),
            ("fixtures", "lint empty.jsonl {F}"),
            ("replay", "{F} --fixtures empty.jsonl"),
            ("import", "csv {F.csv}"),
            ("mcp", ""),
            ("strength", "{F}"),
            ("diff", "{F} {F}"),
        ],
    },
    Language {
        name: "dandori",
        ext: "flow",
        commands: &[
            ("check", "{F}"),
            ("gen", "{F} --target asl --out {O}"),
            ("run", "{F} --scenario scenario.json"),
            ("scenarios", "{F} --out {O}"),
            ("doc", "{F}"),
            ("api", "{F}"),
            ("explain", ""),
        ],
    },
    Language {
        name: "koyomi",
        ext: "cal",
        commands: &[
            ("check", "{F}"),
            ("eval", "{F} x=1"),
            ("diff", "{F} {F}"),
            ("gen", "{F} --target typescript --out {O}"),
            ("vectors", "{F}"),
            ("doc", "{F}"),
            ("api", "{F}"),
            ("source", "fetch {F}"),
            ("explain", ""),
        ],
    },
    Language {
        name: "chobo",
        ext: "book",
        commands: &[
            ("check", "{F}"),
            ("run", "{F} --scenario scenario.json"),
            ("scenarios", "{F} --out {O}"),
            ("gen", "{F} --target sql --out {O}"),
            ("doc", "{F}"),
            ("api", "{F}"),
            ("explain", ""),
        ],
    },
    Language {
        name: "geas",
        ext: "geas",
        commands: &[
            ("check", "{F}"),
            ("snap", "{F}"),
            ("drift", "{F}"),
            ("map", "{F}"),
            ("affected", "{F} empty.diff"),
            ("scenarios", "{F} --openspec openspec"),
            ("explain", ""),
            ("skill", ""),
        ],
    },
    Language {
        name: "sekisho",
        ext: "gate",
        commands: &[
            ("check", "{F}"),
            ("gen", "{F} --target cedar --out {O}"),
            ("diff", "{F} {F}"),
            ("vectors", "{F}"),
            ("api", "{F}"),
            ("doc", "{F}"),
            ("explain", ""),
        ],
    },
    Language {
        name: "kaname",
        ext: "kn",
        commands: &[("check", "{F}"), ("run", "{F} f --input scenario.json"), ("vectors", "{F}"), ("doc", "{F}"), ("api", "{F}"), ("explain", "")],
    },
    Language {
        name: "yuen",
        ext: "req",
        commands: &[
            ("check", "{F}"),
            ("review", "{F} --by someone --all"),
            ("trace", "{F} --requirement x"),
            ("affected", "{F} --diff empty.diff"),
            ("doc", "{F}"),
            ("api", "{F}"),
            ("export", "reqif {F}"),
            ("source", "fetch {F}"),
            ("explain", ""),
        ],
    },
    Language {
        name: "sakai",
        ext: "ctx",
        commands: &[
            ("check", "{F}"),
            ("gen", "{F} --target import-linter --out {O}"),
            ("export", "cml {F}"),
            ("doc", "{F}"),
            ("api", "{F}"),
            ("vectors", "{F}"),
            ("explain", ""),
        ],
    },
    Language {
        name: "ringi",
        ext: "ringi",
        commands: &[
            ("check", "{F}"),
            ("simulate", "{F} --requester x"),
            ("run", "{F}"),
            ("vectors", "{F}"),
            ("api", "{F}"),
            ("doc", "{F}"),
            ("diff", "{F} {F}"),
            ("extract", "{F} --out {O}"),
            ("init", ""),
            ("explain", ""),
        ],
    },
];

/// ritsu's own commands, and those of `ritsu mod`.
const RITSU: &[Line] = &[
    ("check", "{F.rule}"),
    ("run", "{F.flow} --scenario scenario.json"),
    ("gen", "{F.rule} --target typescript --out {O}"),
    ("explain", ""),
    ("impact", "--base HEAD --route {F.rule}"),
    ("skills", ""),
    ("mod", ""),
];

const MOD: &[Line] = &[
    ("init", ""),
    ("add", ""),
    ("get", ""),
    ("remove", ""),
    ("lock", ""),
    ("fetch", ""),
    ("list", ""),
    ("versions", ""),
    ("vendor", ""),
    ("clean", ""),
    ("where", ""),
    ("push", ""),
    ("diff", "{F.rule} {F.rule}"),
    ("report", ""),
    ("site", "{F.list} --out {O}"),
];

/// The commands whose file is not read as text before anything else, and why: they are held to a
/// file that is not there alone.
const NOT_TEXT_FIRST: &[(&str, &str)] = &[
    // takes the directory the generated code is in; a file there holds no vectors
    ("rulec", "test"),
    // reads first the record `geas map` wrote beside the spec
    ("geas", "affected"),
    // a file it cannot read is a line of the report, on the standard output (DESIGN 6.2, item 10)
    ("ritsu", "check"),
    ("ritsu", "gen"),
    ("ritsu mod", "diff"),
];

/// A command whose next word is a command of its own: a flag there is a command it does not have,
/// held with the others below.
const GROUPS: &[(&str, &str)] = &[("ritsu", "mod")];

/// The commands that, given no argument, read the directory they run in.
const HERE_WITHOUT_ARGUMENTS: &[(&str, &str)] = &[("ritsu", "check"), ("ritsu", "gen")];

/// How a run is called: through `ritsu`, or by a language's name (a link to it).
#[derive(Clone, Copy)]
enum Called {
    Ritsu,
    Link(&'static str),
}

/// What a run has to say.
enum Want {
    /// Exactly this sentence after the head, on the standard error alone.
    Said(String),
    /// A sentence that starts so (the one that adds a note after it).
    Starts(String),
    /// What `command` (as the run names it) says given no argument: what it takes or needs, and
    /// the usage line after it.
    Takes(String),
    /// The sentence of the standard error, in JSON on the standard output as well.
    Json,
    /// geas's `--json`: its diagnostic (E080 for the line, E081 for the file), this sentence, in
    /// its JSON on the standard output and as ever on the standard error.
    GeasJson(&'static str, String),
}

struct Case {
    called: Called,
    args: Vec<String>,
    lang: &'static str,
    want: Want,
}

/// The word of an error in the head, as the language of the run says it, with geas's code.
fn head(lang: &str, geas_code: Option<&str>) -> String {
    let word = if lang == "ja" { "エラー" } else { "error" };
    match geas_code {
        Some(c) => format!("{word}[{c}]: "),
        None => format!("{word}: "),
    }
}

fn not_there(lang: &str, f: &str) -> String {
    if lang == "ja" { format!("`{f}` がありません") } else { format!("`{f}` is not there") }
}

fn not_utf8(lang: &str, f: &str) -> String {
    if lang == "ja" { format!("`{f}` は UTF-8 ではありません") } else { format!("`{f}` is not UTF-8") }
}

fn unknown_flag(lang: &str, run: &str) -> String {
    if lang == "ja" { format!("`--nosuch` というフラグはありません。`{run} --help` を読んでください") } else { format!("unknown flag `--nosuch`; run `{run} --help`") }
}

fn no_command(lang: &str, run: &str) -> String {
    if lang == "ja" { format!("`nosuch` というコマンドはありません。`{run} --help` を読んでください") } else { format!("there is no command `nosuch`; run `{run} --help`") }
}

fn no_code(lang: &str, run: &str) -> String {
    if lang == "ja" { format!("`E9999` というコードはありません。`{run} explain --all` を読んでください") } else { format!("there is no code `E9999`; run `{run} explain --all`") }
}

/// The words of a line, `{F}` the file named `stem` of the extension `ext`.
fn words(line: &str, stem: &str, ext: &str) -> Vec<String> {
    line.split_whitespace()
        .map(|w| {
            if w == "{F}" {
                format!("{stem}.{ext}")
            } else if let Some(other) = w.strip_prefix("{F.").and_then(|r| r.strip_suffix('}')) {
                format!("{stem}.{other}")
            } else if w == "{O}" {
                "out".to_string()
            } else {
                w.to_string()
            }
        })
        .collect()
}

/// The file a line reads first, given `stem`: its `{F}`.
fn file_of(line: &str, stem: &str, ext: &str) -> String {
    let w = line.split_whitespace().find(|w| w.starts_with("{F")).expect("the line names a file");
    words(w, stem, ext).remove(0)
}

/// The cases of the commands `commands` of `tool` (`rulec`, `ritsu`, `ritsu mod`), `ext` the
/// extension of its files, `before` the words that call it (`rulec`, or `mod`).
fn cases_of(tool: &str, link: Option<&'static str>, before: &[&str], ext: &str, commands: &[Line], out: &mut Vec<Case>) {
    let geas = tool == "geas";
    for (name, line) in commands {
        let call = |more: Vec<String>| -> Vec<String> { before.iter().map(|w| w.to_string()).chain(std::iter::once(name.to_string())).chain(more).collect() };
        for lang in ["en", "ja"] {
            // a flag it does not have: through ritsu, and by the language's name
            let mut ways = vec![(Called::Ritsu, format!("{} {name}", as_run(tool, true)))];
            if let Some(l) = link {
                ways.push((Called::Link(l), format!("{l} {name}")));
            }
            for (called, run) in ways {
                if GROUPS.contains(&(tool, name)) {
                    continue;
                }
                let args = match called {
                    Called::Ritsu => call(vec!["--nosuch".into()]),
                    Called::Link(_) => std::iter::once(name.to_string()).chain(std::iter::once("--nosuch".to_string())).collect(),
                };
                let said = format!("{}{}", head(lang, geas.then_some("E080")), unknown_flag(lang, &run));
                out.push(Case { called, args, lang, want: Want::Said(said) });
            }
            if line.is_empty() {
                continue;
            }
            // a file that is not there
            let missing = file_of(line, "none", ext);
            let said = format!("{}{}", head(lang, geas.then_some("E081")), not_there(lang, &missing));
            out.push(Case { called: Called::Ritsu, args: call(words(line, "none", ext)), lang, want: Want::Said(said) });
            // a file that is not UTF-8
            if !NOT_TEXT_FIRST.contains(&(tool, name)) {
                let bytes = file_of(line, "bytes", ext);
                let said = format!("{}{}", head(lang, geas.then_some("E081")), not_utf8(lang, &bytes));
                out.push(Case { called: Called::Ritsu, args: call(words(line, "bytes", ext)), lang, want: Want::Said(said) });
            }
            // no argument: what it takes, and its usage line
            if !HERE_WITHOUT_ARGUMENTS.contains(&(tool, name)) {
                out.push(Case { called: Called::Ritsu, args: call(Vec::new()), lang, want: Want::Takes(format!("{} {name}", as_run(tool, true))) });
            }
        }
        if line.is_empty() {
            continue;
        }
        // JSON asked for
        if geas {
            let missing = file_of(line, "none", ext);
            let mut args = call(words(line, "none", ext));
            args.push("--json".into());
            out.push(Case { called: Called::Ritsu, args, lang: "en", want: Want::GeasJson("E081", not_there("en", &missing)) });
            // a flag it does not have, on a line that asks for JSON
            let run = format!("{} {name}", as_run(tool, true));
            out.push(Case { called: Called::Ritsu, args: call(vec!["--nosuch".into(), "--json".into()]), lang: "en", want: Want::GeasJson("E080", unknown_flag("en", &run)) });
        } else {
            let mut args = call(words(line, "none", ext));
            args.extend(["--format".to_string(), "json".to_string()]);
            out.push(Case { called: Called::Ritsu, args, lang: "en", want: Want::Json });
        }
    }
}

/// `tool` named as a run through ritsu names it: `ritsu koyomi`, `ritsu`, `ritsu mod`.
fn as_run(tool: &str, through: bool) -> String {
    if tool == "ritsu" || tool.starts_with("ritsu ") || !through { tool.to_string() } else { format!("ritsu {tool}") }
}

/// The commands a help lists, each the word after `prefix` at the start of a line.
fn listed(help: &str, prefix: &str) -> Vec<String> {
    let mut v: Vec<String> = help
        .lines()
        .filter_map(|l| l.strip_prefix(prefix))
        .filter_map(|rest| rest.split_whitespace().next())
        .filter(|w| w.starts_with(|c: char| c.is_ascii_lowercase()))
        .map(str::to_string)
        .collect();
    v.sort();
    v.dedup();
    v
}

fn run(dir: &Path, case: &Case) -> (i32, String, String) {
    let program: PathBuf = match case.called {
        Called::Ritsu => PathBuf::from(env!("CARGO_BIN_EXE_ritsu")),
        Called::Link(l) => dir.join("links").join(l),
    };
    let mut c = Command::new(program);
    c.current_dir(dir).args(&case.args).args(["--lang", case.lang]);
    for v in LANG_VARS {
        c.env_remove(v);
    }
    let o = c.output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// What is wrong with one run, if anything.
fn wrong(dir: &Path, case: &Case) -> Option<String> {
    let (code, out, err) = run(dir, case);
    let line = match case.called {
        Called::Ritsu => format!("ritsu {} --lang {}", case.args.join(" "), case.lang),
        Called::Link(l) => format!("{l} {} --lang {}", case.args.join(" "), case.lang),
    };
    let shown = || format!("{line}: exit {code}\n  stdout: {out}\n  stderr: {err}");
    if code != 2 {
        return Some(shown());
    }
    let held = match &case.want {
        Want::Said(s) => out.is_empty() && err == format!("{s}\n"),
        Want::Starts(s) => out.is_empty() && err.starts_with(s.as_str()),
        Want::Takes(command) => {
            let (h, usage) = if case.lang == "ja" { ("エラー", "使い方: ") } else { ("error", "usage: ") };
            let mut lines = err.lines();
            let first = lines.next().unwrap_or("");
            // the head, with geas's code; the command in `` ` ``, and what it takes or needs
            let said = first.split_once(": ").map_or("", |(hd, rest)| if hd.starts_with(h) { rest } else { "" });
            let quoted = said.starts_with(&format!("`{command}` "));
            let second = lines.next().unwrap_or("").trim_start_matches(' ').trim_start_matches("= ");
            out.is_empty() && quoted && second.starts_with(&format!("{usage}{command}"))
        }
        Want::Json => {
            let Some(said) = err.strip_prefix("error: ").map(|s| s.trim_end_matches('\n')) else { return Some(shown()) };
            let want = serde_json::json!({ "code": 2, "message": said });
            // the one document the command writes, with its `error`; else the last line, `{"error": …}`
            let whole = serde_json::from_str::<serde_json::Value>(&out).ok().filter(|v| v.get("error").is_some());
            match whole {
                Some(v) => v["error"] == want,
                None => out.lines().last().and_then(|l| serde_json::from_str::<serde_json::Value>(l).ok()) == Some(serde_json::json!({ "error": want })),
            }
        }
        Want::GeasJson(code, s) => {
            err == format!("{}{s}\n", head(case.lang, Some(code)))
                && serde_json::from_str::<serde_json::Value>(&out).is_ok_and(|v| v["diagnostics"][0]["code"] == *code && v["diagnostics"][0]["message"] == s.as_str())
        }
    };
    (!held).then(shown)
}

/// The material: a file that is not UTF-8 of each extension, a scenario, an empty diff, an empty
/// record, the directory of a change, and a link by each language's name.
fn material() -> TempDir {
    let t = TempDir::new("refusals");
    for ext in LANGUAGES.iter().map(|l| l.ext).chain(["csv", "flow", "rule", "list"]) {
        std::fs::write(t.path().join(format!("bytes.{ext}")), b"\xff\xfe\x00 not UTF-8\n").unwrap();
    }
    std::fs::write(t.path().join("scenario.json"), "{\"input\": {}}\n").unwrap();
    std::fs::write(t.path().join("empty.diff"), "").unwrap();
    std::fs::write(t.path().join("empty.jsonl"), "").unwrap();
    std::fs::create_dir_all(t.path().join("openspec")).unwrap();
    std::fs::create_dir_all(t.path().join("links")).unwrap();
    for l in LANGUAGES {
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_ritsu"), t.path().join("links").join(l.name)).unwrap();
    }
    t
}

#[test]
fn every_command_says_what_stops_it_in_the_same_words() {
    let t = material();
    let help = |args: &[&str]| run(t.path(), &Case { called: Called::Ritsu, args: args.iter().map(|s| s.to_string()).chain(std::iter::once("--help".to_string())).collect(), lang: "en", want: Want::Json }).1;
    // the table here holds every command the helps list
    let names = |lines: &[Line]| -> Vec<String> {
        let mut v: Vec<String> = lines.iter().map(|(n, _)| n.to_string()).collect();
        v.sort();
        v
    };
    let mut ritsu: Vec<String> = listed(&help(&[]), "  ritsu ");
    ritsu.retain(|c| !LANGUAGES.iter().any(|l| l.name == c));
    assert_eq!(ritsu, names(RITSU), "the commands `ritsu --help` lists, and the ones held here: add a line for the new one");
    assert_eq!(listed(&help(&["mod"]), "  ritsu mod "), names(MOD), "the commands `ritsu mod --help` lists, and the ones held here");
    for l in LANGUAGES {
        assert_eq!(listed(&help(&[l.name]), &format!("  ritsu {} ", l.name)), names(l.commands), "the commands `ritsu {} --help` lists, and the ones held here", l.name);
    }
    assert_eq!(LANGUAGES.len(), 10);

    let mut cases = Vec::new();
    cases_of("ritsu", None, &[], "rule", RITSU, &mut cases);
    cases_of("ritsu mod", None, &["mod"], "rule", MOD, &mut cases);
    for l in LANGUAGES {
        cases_of(l.name, Some(l.name), &[l.name], l.ext, l.commands, &mut cases);
    }
    // a command the language does not have, and a code it does not know: through ritsu, and by
    // the language's name
    for lang in ["en", "ja"] {
        for l in LANGUAGES {
            let geas = (l.name == "geas").then_some("E080");
            for (called, run, before) in [(Called::Ritsu, format!("ritsu {}", l.name), vec![l.name.to_string()]), (Called::Link(l.name), l.name.to_string(), vec![])] {
                let with = |more: &[&str]| before.iter().cloned().chain(more.iter().map(|s| s.to_string())).collect::<Vec<String>>();
                cases.push(Case { called, args: with(&["nosuch"]), lang, want: Want::Said(format!("{}{}", head(lang, geas), no_command(lang, &run))) });
                cases.push(Case { called, args: with(&["explain", "E9999"]), lang, want: Want::Said(format!("{}{}", head(lang, geas), no_code(lang, &run))) });
            }
        }
        cases.push(Case { called: Called::Ritsu, args: vec!["nosuch".into()], lang, want: Want::Said(format!("{}{}", head(lang, None), no_command(lang, "ritsu"))) });
        cases.push(Case { called: Called::Ritsu, args: vec!["mod".into(), "nosuch".into()], lang, want: Want::Said(format!("{}{}", head(lang, None), no_command(lang, "ritsu mod"))) });
        // ritsu's own codes: a language's code is looked up with the language, which it adds
        cases.push(Case { called: Called::Ritsu, args: vec!["explain".into(), "E9999".into()], lang, want: Want::Starts(format!("{}{}", head(lang, None), no_code(lang, "ritsu"))) });
    }
    // a command that is not there, asked in JSON
    cases.push(Case { called: Called::Ritsu, args: vec!["nosuch".into(), "--format".into(), "json".into()], lang: "en", want: Want::Json });
    cases.push(Case { called: Called::Ritsu, args: vec!["koyomi".into(), "nosuch".into(), "--format".into(), "json".into()], lang: "en", want: Want::Json });

    let failures = wrong_runs(t.path(), &cases);
    assert!(cases.len() > 900, "{} runs", cases.len());
    assert!(failures.is_empty(), "{} of {} runs do not say it in the common words:\n{}", failures.len(), cases.len(), failures.join("\n"));
}

/// What is wrong with each of `cases`, a few runs at a time: each is a process of its own.
fn wrong_runs(dir: &Path, cases: &[Case]) -> Vec<String> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let next = &next;
                s.spawn(move || {
                    let mut found = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let Some(c) = cases.get(i) else { break };
                        found.extend(wrong(dir, c));
                    }
                    found
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}

/// Each tool's `explain`: two of its codes, given in the order the ledger does not have them so that
/// the order given shows, and the words a line asks it for JSON with.
const EXPLAINS: &[(&str, [&str; 2], &[&str])] = &[
    ("ritsu", ["E201", "E101"], &["--format", "json"]),
    ("rulec", ["E002", "E001"], &["--format", "json"]),
    ("dandori", ["E002", "E001"], &["--format", "json"]),
    ("koyomi", ["E002", "E001"], &["--format", "json"]),
    ("chobo", ["E002", "E001"], &["--format", "json"]),
    ("geas", ["E002", "E001"], &["--json"]),
    ("sekisho", ["E002", "E001"], &["--format", "json"]),
    ("kaname", ["E002", "E001"], &["--format", "json"]),
    ("yuen", ["E002", "E001"], &["--format", "json"]),
    ("sakai", ["E002", "E001"], &["--format", "json"]),
    ("ringi", ["E002", "E001"], &["--format", "json"]),
];

fn no_codes(lang: &str, run: &str) -> String {
    if lang == "ja" {
        format!("`E9998`、`E9999` というコードはありません。`{run} explain --all` を読んでください")
    } else {
        format!("there are no codes `E9998`, `E9999`; run `{run} explain --all`")
    }
}

fn twice(lang: &str, flag: &str) -> String {
    if lang == "ja" { format!("`{flag}` が二度書かれています") } else { format!("`{flag}` is given twice") }
}

/// The JSON values `explain` wrote: one document, an array of codes or one code. Anything else
/// (lines of JSON among it) is none.
fn json_values(out: &str) -> Vec<serde_json::Value> {
    match serde_json::from_str::<serde_json::Value>(out) {
        Ok(serde_json::Value::Array(v)) => v,
        Ok(v) => vec![v],
        Err(_) => Vec::new(),
    }
}

/// Every tool's `explain` takes any number of codes and explains them in the order given, as its
/// `--all` writes codes (DESIGN 6.2, item 10): one code as it is written alone, two in the order
/// given and nothing else, in text and in JSON. Codes with `--all`, and neither, stop it with what
/// it takes; codes it does not have stop it, all of them named in one sentence, before anything is
/// printed; a flag given three times stops it at the second. In English and in Japanese, and the
/// lines that stop it in JSON too.
#[test]
fn explain_takes_any_number_of_codes_in_the_order_given() {
    let t = material();
    let go = |args: Vec<String>, lang: &'static str| run(t.path(), &Case { called: Called::Ritsu, args, lang, want: Want::Json });
    let mut failures = Vec::new();
    let mut cases = Vec::new();
    for &(tool, [b, a], json) in EXPLAINS {
        let before: Vec<String> = if tool == "ritsu" { Vec::new() } else { vec![tool.to_string()] };
        let call = |more: &[&str]| -> Vec<String> { before.iter().cloned().chain(std::iter::once("explain".to_string())).chain(more.iter().map(|s| s.to_string())).collect() };
        let geas = (tool == "geas").then_some("E080");
        let run_as = as_run(tool, true);
        for lang in ["en", "ja"] {
            // two codes: each as it is written alone, in the order given, and nothing else
            let (_, one_b, _) = go(call(&[b]), lang);
            let (_, one_a, _) = go(call(&[a]), lang);
            let (code, out, err) = go(call(&[b, a]), lang);
            let in_order = out.starts_with(&format!("{one_b}\n{one_a}")) && out.len() <= one_b.len() + one_a.len() + 2;
            if code != 0 || !err.is_empty() || one_b.is_empty() || one_a.is_empty() || !in_order {
                failures.push(format!("{run_as} explain {b} {a} --lang {lang}: exit {code}\n  stdout: {out}\n  stderr: {err}"));
            }
            // codes with `--all`, and neither: what it takes, in the same words
            let (_, _, neither) = go(call(&[]), lang);
            let (_, _, with_all) = go(call(&[a, "--all"]), lang);
            if neither != with_all {
                failures.push(format!("{run_as} explain {a} --all --lang {lang} says\n  {with_all}which is not what it says given nothing:\n  {neither}"));
            }
            for args in [call(&[a, "--all"]), call(&[])] {
                cases.push(Case { called: Called::Ritsu, args, lang, want: Want::Takes(format!("{run_as} explain")) });
            }
            // codes it does not have: all of them in one sentence, in the order given, and nothing
            // printed of the code it has
            let said = format!("{}{}", head(lang, geas), no_codes(lang, &run_as));
            let want = if tool == "ritsu" { Want::Starts(said) } else { Want::Said(said) };
            cases.push(Case { called: Called::Ritsu, args: call(&["E9998", a, "E9999"]), lang, want });
            // a flag given three times stops it at the second, in the words of one given twice (a
            // flag that does not ask for JSON, so that nothing is written on stdout)
            let flag: &[&str] = if geas.is_some() { &["--all"] } else { &["--format", "markdown"] };
            let mut thrice = vec![a];
            for _ in 0..3 {
                thrice.extend_from_slice(flag);
            }
            cases.push(Case { called: Called::Ritsu, args: call(&thrice), lang, want: Want::Said(format!("{}{}", head(lang, geas), twice(lang, flag[0]))) });
        }
        // two codes in JSON: the document of each, in the order given, as `--all` writes them
        let docs = |codes: &[&str]| json_values(&go(call(&[codes, json].concat()), "en").1);
        let (one_b, one_a, two) = (docs(&[b]), docs(&[a]), docs(&[b, a]));
        let codes: Vec<&str> = two.iter().map(|v| v["code"].as_str().unwrap_or("")).collect();
        if one_b.len() != 1 || one_a.len() != 1 || two != [one_b, one_a].concat() || codes != [b, a] {
            failures.push(format!("{run_as} explain {b} {a} {}: the codes {codes:?}", json.join(" ")));
        }
        // and what stops it, in JSON as every line that asks for it is answered
        let (both, unknown) = (call(&[&[a, "--all"][..], json].concat()), call(&[&["E9998", "E9999"][..], json].concat()));
        if geas.is_some() {
            let (code, out, err) = go(both.clone(), "en");
            let said = err.lines().next().and_then(|l| l.strip_prefix("error[E080]: ")).unwrap_or("");
            let doc = serde_json::from_str::<serde_json::Value>(&out).unwrap_or_default();
            if code != 2 || said.is_empty() || doc["diagnostics"][0]["code"] != "E080" || doc["diagnostics"][0]["message"] != said {
                failures.push(format!("{run_as} {}: exit {code}\n  stdout: {out}\n  stderr: {err}", both.join(" ")));
            }
            cases.push(Case { called: Called::Ritsu, args: unknown, lang: "en", want: Want::GeasJson("E080", no_codes("en", &run_as)) });
        } else {
            cases.push(Case { called: Called::Ritsu, args: both, lang: "en", want: Want::Json });
            cases.push(Case { called: Called::Ritsu, args: unknown, lang: "en", want: Want::Json });
        }
    }
    failures.extend(wrong_runs(t.path(), &cases));
    assert!(failures.is_empty(), "{} of the runs of `explain` are not as every tool's:\n{}", failures.len(), failures.join("\n"));
}

/// Every tool's `explain` writes its JSON in one shape (DESIGN 6.2, item 10), so that one reader
/// reads them all: one code as one indented document, and two or more codes, or `--all`, as one
/// indented array of the codes' documents, in the order given. Through ritsu, in English.
#[test]
fn explain_writes_its_json_in_one_shape() {
    let t = material();
    let go = |args: Vec<String>| run(t.path(), &Case { called: Called::Ritsu, args, lang: "en", want: Want::Json });
    let codes_of = |v: &serde_json::Value| -> Vec<String> { v.as_array().map(|a| a.iter().map(|e| e["code"].as_str().unwrap_or("").to_string()).collect()).unwrap_or_default() };
    let mut failures = Vec::new();
    for &(tool, [b, a], json) in EXPLAINS {
        let before: Vec<String> = if tool == "ritsu" { Vec::new() } else { vec![tool.to_string()] };
        let call = |more: &[&str]| -> Vec<String> { before.iter().cloned().chain(std::iter::once("explain".to_string())).chain(more.iter().chain(json).map(|s| s.to_string())).collect() };
        let run_as = format!("{} explain", as_run(tool, true));
        // one code: one indented document, the code's
        let (code, out, err) = go(call(&[a]));
        let doc = serde_json::from_str::<serde_json::Value>(&out).unwrap_or_default();
        if code != 0 || !err.is_empty() || doc["code"] != a || !out.starts_with("{\n  \"") || !out.ends_with("}\n") {
            failures.push(format!("{run_as} {a} {}: exit {code}, not one indented document\n  stdout: {out}\n  stderr: {err}", json.join(" ")));
        }
        // two codes: one indented array, in the order given
        let (code, out, err) = go(call(&[b, a]));
        let two = serde_json::from_str::<serde_json::Value>(&out).unwrap_or_default();
        if code != 0 || !err.is_empty() || codes_of(&two) != [b, a] || two[1] != doc || !out.starts_with("[\n  {\n") || !out.ends_with("]\n") {
            failures.push(format!("{run_as} {b} {a} {}: exit {code}, not one indented array of the two\n  stdout: {out}\n  stderr: {err}", json.join(" ")));
        }
        // every code: one indented array, each code once
        let (code, out, err) = go(call(&["--all"]));
        let all = codes_of(&serde_json::from_str::<serde_json::Value>(&out).unwrap_or_default());
        let once: std::collections::BTreeSet<&String> = all.iter().collect();
        if code != 0 || !err.is_empty() || all.len() < 2 || once.len() != all.len() || ![a, b].iter().all(|c| all.iter().any(|x| x == c)) || !out.starts_with("[\n  {\n") || !out.ends_with("]\n") {
            failures.push(format!("{run_as} --all {}: exit {code}, not one indented array of every code ({} read)\n  stderr: {err}", json.join(" "), all.len()));
        }
    }
    assert!(failures.is_empty(), "the JSON of `explain` is in another shape in {} of the runs:\n{}", failures.len(), failures.join("\n"));
}

/// No source writes the head of an error itself (`"error: "`, `"エラー["`, a choice between the two
/// words): ritsu-base's `cli::refusal` and the diagnostics write it, in the language of the run.
#[test]
fn no_source_writes_the_head_of_an_error_itself() {
    // what reads another program's words, and why
    const READS_OTHERS: &[(&str, &str)] = &[("kaname/src/lean.rs", "the lines Lean's `lake build` prints")];
    let crates = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let heads = ["\"error: ", "\"エラー: ", "\"error[", "\"エラー[", "{ \"エラー\" } else { \"error\" }", "{ \"error\" } else { \"エラー\" }"];
    let mut found = Vec::new();
    let mut files = 0;
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    for c in std::fs::read_dir(&crates).unwrap().flatten() {
        let src = c.path().join("src");
        let name = c.file_name().to_string_lossy().into_owned();
        if !src.is_dir() || name == "ritsu-testkit" {
            continue;
        }
        let mut paths = Vec::new();
        walk(&src, &mut paths);
        for p in paths {
            let rel = p.strip_prefix(&crates).unwrap().to_string_lossy().into_owned();
            if READS_OTHERS.iter().any(|(f, _)| *f == rel) {
                continue;
            }
            files += 1;
            let text = std::fs::read_to_string(&p).unwrap();
            let lines: Vec<&str> = text.lines().collect();
            for (i, l) in lines.iter().enumerate() {
                // the tests of a module, at its end
                if l.trim() == "#[cfg(test)]" && lines.get(i + 1).is_some_and(|n| n.trim_start().starts_with("mod ")) {
                    break;
                }
                if l.trim_start().starts_with("//") {
                    continue;
                }
                if heads.iter().any(|h| l.contains(h)) {
                    found.push(format!("{rel}:{}: {}", i + 1, l.trim()));
                }
            }
        }
    }
    assert!(files > 500, "{files} files read");
    assert!(found.is_empty(), "a head of an error written by hand; say it with ritsu_base::cli::refusal (or refuse):\n{}", found.join("\n"));
}
