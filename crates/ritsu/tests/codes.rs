//! ritsu's own codes (DESIGN 4.3, 7.1; PLAN E.3): `ritsu explain` looks them up, and every
//! code's reproduction — the files of a small project — printed by `ritsu check .`, with the
//! code's headline carrying `ritsu`, or by `ritsu mod diff`, among the differences of an interface
//! with its tool (E310); and the numbers the sentences of the checks say, written as the files
//! write them (DESIGN 6.2 item 8).

use ritsu_testkit::TempDir;
use std::path::Path;
use std::process::Command;

/// `ritsu`, run in `dir`, with no language asked of the environment.
fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_ritsu")).current_dir(dir).env_remove("RITSU_LANG").args(args).output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Every reproduction prints its code, in English and in Japanese.
#[test]
fn every_reproduction_prints_its_code() {
    let l = ritsu_cross::codes::ledger();
    let scratch = TempDir::new("codes");
    let failures = ritsu_base::ledger::check_every(&l, scratch.path(), |e, dir| {
        let ritsu_base::ledger::Repro::Dir { command, .. } = &e.repro else { return vec![] };
        let mut codes = Vec::new();
        for lang in ["en", "ja"] {
            let mut args: Vec<&str> = command[1..].to_vec();
            args.extend(["--lang", lang]);
            // `ritsu mod diff` says ritsu's codes among the differences of an interface, which its
            // JSON gives with their tool
            if command[1..].starts_with(&["mod", "diff"]) {
                args.extend(["--format", "json"]);
                let (_, out, _) = ritsu_in(dir, &args);
                let v: serde_json::Value = serde_json::from_str(&out).unwrap_or_default();
                for i in v["interface"].as_array().into_iter().flatten() {
                    codes.extend(i["breaks"].as_array().into_iter().flatten().filter(|b| b["tool"] == "ritsu").filter_map(|b| b["code"].as_str().map(String::from)));
                }
                continue;
            }
            let (_, out, err) = ritsu_in(dir, &args);
            for l in out.lines().chain(err.lines()) {
                if let Some(rest) = l.split_once("[ritsu ").map(|(_, r)| r)
                    && let Some((code, _)) = rest.split_once(']')
                {
                    codes.push(code.to_string());
                }
            }
        }
        codes.dedup();
        codes
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The reproductions of the borders call the rule, the date or the transfer from a module of kaname
/// too, and the module's call prints the code beside the workflow's, in English and in Japanese.
/// E204 has none: a transfer a module builds leaves its refusals to the host, and they go on the
/// type kaname generates for it; W204 comes to a module only when chobo cannot answer (kaname's
/// DESIGN 14.4). E206 and W206 are of a workflow's waits, which a module does not have.
#[test]
fn the_reproductions_of_the_borders_call_from_a_module_too() {
    const WITH_A_MODULE: [&str; 8] = ["E201", "W201", "E202", "W202", "E203", "W203", "E205", "W205"];
    let l = ritsu_cross::codes::ledger();
    let scratch = TempDir::new("codes-kaname");
    let mut wrong = Vec::new();
    for e in &l.entries {
        let ritsu_base::ledger::Repro::Dir { files, .. } = &e.repro else { continue };
        let module = files.iter().find(|(n, _)| n.ends_with(".kn")).map(|(n, _)| *n);
        let Some(kn) = module else {
            if WITH_A_MODULE.contains(&e.code) {
                wrong.push(format!("{}: the reproduction has no module", e.code));
            }
            continue;
        };
        if !WITH_A_MODULE.contains(&e.code) {
            wrong.push(format!("{}: a module in a reproduction this test does not expect one in", e.code));
            continue;
        }
        let dir = scratch.path().join(e.code);
        l.lay_out(e, &dir).unwrap();
        for lang in ["en", "ja"] {
            let (_, out, _) = ritsu_in(&dir, &["check", ".", "--lang", lang]);
            if !out.lines().any(|line| line.contains(&format!("[ritsu {}]: {kn}:", e.code))) {
                wrong.push(format!("{} ({lang}): {kn} does not print the code:\n{out}", e.code));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The numbers kept out of the sweep of [`the_checks_say_a_number_as_the_files_write_it`], by the
/// output they are in and as they are written: `paid` and `asked` of the rule of X2's tests, the
/// `int` ends of the ranges of dandori's test flows and the limit of a durable execution's
/// operations (E040, a count) are numbers with no unit, and four digits have no `_`.
const BARE_NUMBERS: [(&str, &str); 8] = [
    ("golden/cross/x2-rebound-E201.", "5000"),
    ("golden/cross/x2-result-E201.", "9000"),
    ("dandori/tests/doc/tests-flows-quota.", "1000"),
    ("dandori/tests/doc/tests-flows-http_places.", "1000"),
    ("dandori/tests/doc/tests-flows-http_places-ja.", "1000"),
    ("dandori/tests/doc/tests-flows-none_values.", "1000"),
    ("dandori/tests/doc/tests-flows-none_values-ja.", "1000"),
    ("dandori/tests/fixtures/history", "3000"),
];

/// The sentences of ritsu's own checks, of rulec's and of dandori's say a number as the files
/// write it (DESIGN 6.2 item 8, 7.1; rulec's §15.333; dandori's DESIGN 1.3): in its unit, with its
/// digits grouped by `_` from five on (`10_000_000JPY`, `5000JPY`), as `ritsu_units::written`
/// writes it, and never as the bare whole number on the wire. The sweep reads the headline and the
/// notes of each diagnostic of ritsu's, of rulec's (with the labels of its frames) and of
/// dandori's, in every reproduction of the ledger run in English and in Japanese, in every golden
/// of the checks between the languages (`golden/cross`, `golden/fed_by`, which their tests hold to
/// what `ritsu check` prints), and in what dandori says itself ([`dandori_outputs`]); and the
/// ranges of the pages `dandori doc` writes. A number of four digits or more fails when it has
/// neither a unit nor `_` (`10000000`), and one of five or more when its digits are grouped any
/// other way (`10000`, `10,000`). The calls kaname writes of a function (`route(paid: 0, asked:
/// 10_000)`) are read with the rest; an expression a diagnostic quotes is the file's own, which
/// the reproductions write as the files do (`mod(amount, 300_000)`). Left out, by name:
///
/// - a whole number a `.flow` writes, which dandori quotes as it is written (`` `1500000` is
///   1_500_000JPY ``);
/// - what a sentence of rulec's quotes between backticks, the source or a line to write in it,
///   which rulec writes as the file does; and a number of rulec's of four digits with no unit,
///   which is how rulec says a count or a plain number (`it ranges from 0 to 1000`), as rulec's
///   own sweep reads it (its `tests/numbers_as_written.rs`);
/// - chobo's limit, which the note on what chobo takes keeps in the unit's smallest step and writes
///   `2⁶³ − 1`;
/// - the numbers of [`BARE_NUMBERS`].
#[test]
fn the_checks_say_a_number_as_the_files_write_it() {
    let l = ritsu_cross::codes::ledger();
    let scratch = TempDir::new("codes-numbers");
    // each output, with whether a code with no tool before it (`error[E014]`) is dandori's: in what
    // dandori prints itself
    let mut outputs: Vec<(String, String, bool)> = Vec::new();
    let failures = ritsu_base::ledger::check_every(&l, scratch.path(), |e, dir| {
        let ritsu_base::ledger::Repro::Dir { command, .. } = &e.repro else { return vec![] };
        let mut codes = Vec::new();
        for lang in ["en", "ja"] {
            let mut args: Vec<&str> = command[1..].to_vec();
            args.extend(["--lang", lang]);
            let (_, out, err) = ritsu_in(dir, &args);
            let text = out + &err;
            codes.extend(text.lines().filter_map(|l| l.split_once("[ritsu ").and_then(|(_, r)| r.split_once(']')).map(|(c, _)| c.to_string())));
            outputs.push((format!("{} in {} ({lang})", e.code, dir.file_name().unwrap_or_default().to_string_lossy()), text, false));
        }
        // `ritsu mod diff` says its codes in its JSON, which the test above reads
        if command[1..].starts_with(&["mod", "diff"]) {
            codes.push(e.code.to_string());
        }
        codes
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    for d in ["cross", "fed_by"] {
        let mut files: Vec<_> = std::fs::read_dir(golden.join(d)).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
        files.sort();
        for p in files {
            outputs.push((format!("golden/{d}/{}", p.file_name().unwrap().to_string_lossy()), std::fs::read_to_string(&p).unwrap(), false));
        }
    }
    outputs.extend(dandori_outputs().into_iter().map(|(name, text)| (name, text, true)));
    let mut wrong = Vec::new();
    let mut kept = std::collections::BTreeSet::new();
    let (mut said, mut said_rulec, mut said_dandori, mut ranges) = (0, 0, 0, 0);
    let mut sweep = |name: &str, sentence: &str, tool: Tool| {
        let read = match tool {
            Tool::Dandori => without_literals(sentence),
            Tool::Rulec => sentence.split('`').step_by(2).collect::<Vec<_>>().join(" "),
            Tool::Ritsu => sentence.to_string(),
        };
        for (n, why) in numbers_off(&read, tool != Tool::Rulec) {
            if let Some(k) = BARE_NUMBERS.iter().find(|(o, v)| name.starts_with(o) && n == *v) {
                kept.insert(*k);
                continue;
            }
            wrong.push(format!("{name}: `{n}` {why}: {sentence}"));
        }
    };
    for (name, text, bare) in &outputs {
        for (tool, sentence) in sentences(text, *bare) {
            sweep(name, &sentence, tool);
            match tool {
                Tool::Ritsu => said += 1,
                Tool::Rulec => said_rulec += 1,
                Tool::Dandori => said_dandori += 1,
            }
        }
        // a page of `dandori doc`: the inputs and the outputs with their ranges
        if name.ends_with(".md") {
            for range in text.split('`').skip(1).step_by(2).filter(|p| p.contains(" range ")) {
                sweep(name, range, Tool::Dandori);
                ranges += 1;
            }
        }
    }
    eprintln!("read: {said} sentences of ritsu's diagnostics, {said_rulec} of rulec's, {said_dandori} of dandori's, and {ranges} ranges of dandori's pages");
    assert!(said >= 500, "{said} sentences of ritsu's diagnostics read: were the outputs or their form changed?");
    assert!(said_rulec >= 1, "{said_rulec} sentences of rulec's diagnostics read: were the outputs or their form changed?");
    assert!(said_dandori >= 3000 && ranges >= 60,"{said_dandori} sentences of dandori's diagnostics and {ranges} ranges of its pages read: were the outputs or their form changed?");
    assert_eq!(kept.len(), BARE_NUMBERS.len(), "a number left out by name is no longer said: {kept:?}");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// What dandori says itself, by the output it is in: its golden files of what `dandori check` says
/// of each fixture, each spelling and a module (which its tests hold to its checker), and of the
/// pages `dandori doc` writes (`tests/doc`); and what `ritsu dandori check` says of each of its
/// examples, in English and in Japanese.
fn dandori_outputs() -> Vec<(String, String)> {
    fn under(dir: &Path, ext: &str, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                under(&p, ext, out);
            } else if p.extension().is_some_and(|x| x == ext) {
                out.push(p);
            }
        }
    }
    let dandori = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dandori");
    let mut out = Vec::new();
    for (d, ext) in [("tests/fixtures", "txt"), ("tests/spellings", "txt"), ("tests/modules", "txt"), ("tests/doc", "md")] {
        let mut files = Vec::new();
        under(&dandori.join(d), ext, &mut files);
        files.sort();
        for p in files {
            out.push((format!("dandori/{}", p.strip_prefix(&dandori).unwrap().display()), std::fs::read_to_string(&p).unwrap()));
        }
    }
    let mut flows = Vec::new();
    under(&dandori.join("examples"), "flow", &mut flows);
    flows.sort();
    for f in flows {
        let file = f.strip_prefix(&dandori).unwrap().display().to_string();
        for lang in ["en", "ja"] {
            let (_, o, e) = ritsu_in(&dandori, &["dandori", "check", &file, "--lang", lang]);
            out.push((format!("dandori check {file} ({lang})"), o + &e));
        }
    }
    out
}

/// Whose diagnostic a sentence is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Ritsu,
    Rulec,
    Dandori,
}

/// The headline and the notes of each diagnostic of ritsu's, of rulec's and of dandori's in `text`
/// (the headline without its severity, code and place; of rulec's, the labels of its frame too),
/// each with whose it is. A code with no tool before it (`error[E014]`) is dandori's when `bare`
/// says so, in what dandori prints itself; the other tools' are not read here. rulec writes a
/// note one space in (`diag::render`), and a label past the carets of its frame.
fn sentences(text: &str, bare: bool) -> Vec<(Tool, String)> {
    let mut out = Vec::new();
    let mut on: Option<Tool> = None;
    for line in text.lines() {
        if let Some((head, rest)) = line.split_once('[')
            && ["error", "warning", "エラー", "警告"].contains(&head)
            && let Some((code, rest)) = rest.split_once("]: ")
            && let Some(tool) = match code.split_once(' ') {
                Some(("ritsu", _)) => Some(Tool::Ritsu),
                Some(("rulec", _)) => Some(Tool::Rulec),
                Some(("dandori", _)) => Some(Tool::Dandori),
                None => bare.then_some(Tool::Dandori),
                Some(_) => None,
            }
        {
            // rulec's headline has no place in it: the place is on the line below
            let (_, sentence) = if tool == Tool::Rulec { ("", rest) } else { rest.split_once(": ").unwrap_or(("", rest)) };
            out.push((tool, sentence.to_string()));
            on = Some(tool);
        } else if let Some(tool) = on {
            let t = line.trim_start();
            let frame = t.starts_with('|') || t.split_once(" |").is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
            let rulec = tool == Tool::Rulec;
            if let Some(note) = line.strip_prefix("  = ") {
                out.push((tool, note.to_string()));
            } else if rulec && (frame || line.starts_with("  --> ")) {
                // the place, and the frame of the file, where what is past the carets is a label
                if let Some(label) = t.strip_prefix('|').map(str::trim_start).filter(|g| g.starts_with('^')).map(|g| g.trim_start_matches('^').trim_start()).filter(|l| !l.is_empty()) {
                    out.push((tool, label.to_string()));
                }
            } else if rulec && let Some(note) = line.strip_prefix(' ').filter(|n| !n.is_empty() && !n.starts_with(' ')) {
                out.push((tool, note.to_string()));
            } else if !frame {
                // past the lines of the file the diagnostic points at
                on = None;
            }
        }
    }
    out
}

/// `sentence` without the whole numbers a `.flow` writes, which dandori quotes as they are written:
/// a number alone between two backticks.
fn without_literals(sentence: &str) -> String {
    let mut out = String::new();
    for (k, part) in sentence.split('`').enumerate() {
        let literal = !part.is_empty() && part.strip_prefix('-').unwrap_or(part).chars().all(|c| c.is_ascii_digit());
        if k % 2 == 0 {
            out.push_str(part);
        } else if !literal {
            out.push('`');
            out.push_str(part);
            out.push('`');
        }
    }
    out
}

/// The numbers of `sentence` not written as ritsu writes them, each with what is wrong with it;
/// a number of four digits with neither a unit nor `_` among them when `four_bare` says so.
/// Dates, times, offsets and hashes are not numbers here.
fn numbers_off(sentence: &str, four_bare: bool) -> Vec<(String, &'static str)> {
    let s = sentence.replace("2⁶³ − 1", "");
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        if !cs[i].is_ascii_digit() || (i > 0 && (cs[i - 1].is_alphanumeric() || matches!(cs[i - 1], '_' | '.' | ':' | '/' | '#'))) {
            i += 1;
            continue;
        }
        let begin = i;
        while i < cs.len() && (cs[i].is_ascii_digit() || (matches!(cs[i], '_' | ',' | '.' | '-' | ':') && cs.get(i + 1).is_some_and(char::is_ascii_digit))) {
            i += 1;
        }
        let token: String = cs[begin..i].iter().collect();
        let hex: Vec<char> = cs[begin..].iter().copied().take_while(char::is_ascii_hexdigit).collect();
        if token.contains(['-', ':']) || (hex.len() >= 12 && hex.iter().any(char::is_ascii_alphabetic)) {
            continue;
        }
        let unit = cs.get(i).is_some_and(|&c| c.is_ascii_alphabetic() || matches!(c, '%' | '℃' | '℉') || ('\u{4e00}'..='\u{9fff}').contains(&c));
        let whole = token.split('.').next().unwrap_or_default();
        let digits: String = whole.chars().filter(char::is_ascii_digit).collect();
        if token.contains(',') {
            out.push((token, "is grouped with `,`"));
        } else if digits.len() >= 5 && digits.parse::<i128>().is_ok_and(|n| whole != ritsu_units::written::digits(n)) {
            out.push((token, "is not grouped by `_` from five digits"));
        } else if four_bare && digits.len() >= 4 && !unit && !whole.contains('_') {
            out.push((token, "has neither a unit nor `_`"));
        }
    }
    out
}

/// `ritsu explain` looks up ritsu's codes, and says where a language's code is looked up.
#[test]
fn ritsu_explain() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (code, out, err) = ritsu_in(here, &["explain", "E101"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.starts_with("E101 (error) — A file that does not read as a .proto\n"), "{out}");
    let (code, out, _) = ritsu_in(here, &["explain", "e101", "--lang", "ja"]);
    assert!(code == 0 && out.starts_with("E101 (エラー) — .proto として読めないファイル\n"), "{out}");
    let (code, out, _) = ritsu_in(here, &["explain", "--all", "--format", "markdown"]);
    assert_eq!(code, 0);
    assert_eq!(out, std::fs::read_to_string(here.join("../ritsu-cross/docs/codes.md")).unwrap());
    let (code, out, _) = ritsu_in(here, &["explain", "--all", "--format", "json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v[0]["code"], "E101");
    assert_eq!(v[0]["repro"]["command"], serde_json::json!(["ritsu", "check", "."]));
    let (code, _, err) = ritsu_in(here, &["explain", "E032"]);
    assert_eq!(code, 2);
    assert_eq!(err, "error: there is no code `E032`; run `ritsu explain --all` (a language's code is looked up with `ritsu <language> explain E032`)\n");
    let (code, _, err) = ritsu_in(here, &["explain"]);
    assert_eq!((code, err.as_str()), (2, "error: `ritsu explain` takes <CODE>... | --all\nusage: ritsu explain <CODE>... | --all [--format markdown|json]\n"));
}

/// `ritsu explain` looks up the codes of the modules (DESIGN 17.20) as it does the others: in
/// English and in Japanese, as text, as the section of the pages of codes, and as JSON with the
/// files of its reproduction, which hold a dependency from a registry and its lock.
#[test]
fn ritsu_explain_a_code_of_the_modules() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (code, out, err) = ritsu_in(here, &["explain", "E304"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.starts_with("E304 (error) — `ritsu.lock` does not match `ritsu.mod`\n"), "{out}");
    assert!(out.contains("\nFix: Run `ritsu mod lock` to write the lock again.\n"), "{out}");
    assert!(out.contains("    require tax oci \"example.com/acme/tax\" 1.2.0\n"), "{out}");
    let (code, out, _) = ritsu_in(here, &["explain", "E304", "--lang", "ja"]);
    assert!(code == 0 && out.starts_with("E304 (エラー) — `ritsu.lock` が `ritsu.mod` に合いません\n"), "{out}");
    assert!(out.contains("\n直し方: `ritsu mod lock` を走らせて、lock を書き直してください。\n"), "{out}");
    for (lang, page) in [("en", "codes.md"), ("ja", "codes.ja.md")] {
        let (code, out, _) = ritsu_in(here, &["explain", "E304", "--format", "markdown", "--lang", lang]);
        assert_eq!(code, 0);
        assert!(out.starts_with("<a id=\"e304\"></a>\n"), "{out}");
        let page = std::fs::read_to_string(here.join("../ritsu-cross/docs").join(page)).unwrap();
        assert!(page.contains(&out), "the section of E304 ({lang}) is the one of the page of codes:\n{out}");
    }
    let (code, out, _) = ritsu_in(here, &["explain", "E304", "--format", "json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!((v["code"].as_str(), v["severity"].as_str()), (Some("E304"), Some("error")));
    assert_eq!(v["repro"]["command"], serde_json::json!(["ritsu", "check", "."]));
    let paths: Vec<&str> = v["repro"]["files"].as_array().unwrap().iter().filter_map(|f| f["path"].as_str()).collect();
    assert_eq!(paths, ["ritsu.mod", "ritsu.lock", "fee.rule"]);
    assert_eq!(v["related"], serde_json::json!(["E303", "W302"]));
}
