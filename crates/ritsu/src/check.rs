//! `ritsu check` (DESIGN 8.1, 8.3, 8.4): every file of a project checked by its language's own
//! `check`, the languages that give facts first, each with the languages it reads joined; then
//! what ritsu checks across the languages; then one line that sums it up. The text is what each
//! language's `check` prints, with the tool's word in each headline (`error[rulec E032]`), since
//! the codes of the languages overlap; the JSON is one object, each language's diagnostic in its
//! own JSON with the tool and the file from the root put outside it.

use crate::cli;
use ritsu_base::diag::Severity;
use ritsu_base::json::Json;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_ports::{Checked, Finding, Part, Verdict};
use ritsu_project::{File, Joined, Project};
use std::io::Write;

/// What the checks across the languages came to (DESIGN 7.1, P5): each border shown to hold, with
/// an example where it does not, or undecided. The checks themselves come in the second part of
/// stage E (PLAN E.4); until then every count is 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Borders {
    pub held: usize,
    pub failed: usize,
    pub undecided: usize,
}

/// How a file of the project came out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Passes,
    Fails,
    Unchecked,
}

/// Everything `ritsu check` says of a project.
pub struct Report<'a> {
    pub project: &'a Project,
    /// What each language's `check` said, unit by unit, in the order they were checked.
    pub units: Vec<(Tool, Checked)>,
    /// What ritsu says itself, across the languages.
    pub ours: Vec<Finding>,
    pub borders: Borders,
}

/// The languages whose `check` takes one file at a time: whatever their check of a file comes
/// to is that file's.
const ONE_FILE: [Tool; 5] = [Tool::Rulec, Tool::Koyomi, Tool::Chobo, Tool::Geas, Tool::Dandori];

impl Report<'_> {
    fn findings(&self) -> impl Iterator<Item = (&str, &Finding)> {
        self.units.iter().flat_map(|(t, u)| u.findings().map(move |f| (t.word(), f))).chain(self.ours.iter().map(|f| ("ritsu", f)))
    }

    fn status(&self, f: &File) -> Status {
        let unit = |t: Tool| self.units.iter().filter(move |(ut, _)| *ut == t).map(|(_, u)| u);
        let own = ONE_FILE.contains(&f.tool);
        if (own && unit(f.tool).any(|u| u.label == f.shown && u.verdict == Verdict::Unchecked)) || (!own && unit(f.tool).any(|u| u.verdict == Verdict::Unchecked)) {
            return Status::Unchecked;
        }
        let error_in_it = self.findings().any(|(_, x)| x.severity == Severity::Error && x.file.as_deref() == Some(f.rel.as_str()));
        if error_in_it || (own && unit(f.tool).any(|u| u.label == f.shown && u.verdict == Verdict::Fails)) {
            return Status::Fails;
        }
        Status::Passes
    }

    /// 0 when nothing is wrong (warnings may be printed), 1 when a language or ritsu found an
    /// error, 2 when a file could not be checked (DESIGN 8.4).
    pub fn code(&self) -> u8 {
        if self.units.iter().any(|(_, u)| u.verdict == Verdict::Unchecked) {
            2
        } else if self.units.iter().any(|(_, u)| u.verdict == Verdict::Fails) || self.findings().any(|(_, f)| f.severity == Severity::Error) {
            1
        } else {
            0
        }
    }

    /// What a person reads: each language's text, ritsu's own findings, and the line that sums
    /// them up.
    pub fn text(&self, lang: Lang) -> String {
        let mut out = String::new();
        for (t, u) in &self.units {
            for p in &u.parts {
                match p {
                    Part::Finding(f) => out.push_str(&f.text_of(t.word())),
                    Part::Text(s) => out.push_str(s),
                }
            }
        }
        for f in &self.ours {
            out.push_str(&f.text_of("ritsu"));
        }
        out.push_str(self.summary().get(lang));
        out.push('\n');
        out
    }

    /// The last line: how many files of each language, how they came out, and the borders between
    /// the languages ritsu checked.
    fn summary(&self) -> Text {
        let n = self.project.files.len();
        let by_tool: Vec<String> = self.project.counts().iter().map(|(t, k)| format!("{} {k}", t.word())).collect();
        let (ja_by, en_by) = (by_tool.join("、"), by_tool.join(", "));
        let statuses: Vec<Status> = self.project.files.iter().map(|f| self.status(f)).collect();
        let fails = statuses.iter().filter(|s| **s == Status::Fails).count();
        let unchecked = statuses.iter().filter(|s| **s == Status::Unchecked).count();
        let errors = self.findings().filter(|(_, f)| f.severity == Severity::Error).count();
        let warnings = self.findings().filter(|(_, f)| f.severity == Severity::Warning).count();
        let mut counts_ja = Vec::new();
        let mut counts_en = Vec::new();
        if errors > 0 {
            counts_ja.push(format!("エラー {errors} 件"));
            counts_en.push(ritsu_base::text::plural(errors, "error", "errors"));
        }
        if warnings > 0 {
            counts_ja.push(format!("警告 {warnings} 件"));
            counts_en.push(ritsu_base::text::plural(warnings, "warning", "warnings"));
        }
        let in_brackets = |ja: &str, en: &str| -> (String, String) {
            if counts_ja.is_empty() { (ja.to_string(), en.to_string()) } else { (format!("{ja}（{}）", counts_ja.join("、")), format!("{en} ({})", counts_en.join(", "))) }
        };
        let (ja, en) = if fails == 0 && unchecked == 0 {
            in_brackets("どれも検査を通った", "all pass")
        } else {
            let mut ja = Vec::new();
            let mut en = Vec::new();
            if fails > 0 {
                let (j, e) = in_brackets(&format!("検査を通らないもの {fails} 個"), &format!("{fails} fail"));
                ja.push(j);
                en.push(e);
            } else if !counts_ja.is_empty() {
                ja.push(counts_ja.join("、"));
                en.push(counts_en.join(", "));
            }
            if unchecked > 0 {
                ja.push(format!("確かめられなかったもの {unchecked} 個"));
                en.push(format!("{unchecked} not checked"));
            }
            (ja.join("、"), en.join(", "))
        };
        let (checked, undecided) = (self.borders.held + self.borders.failed + self.borders.undecided, self.borders.undecided);
        let files_en = if n == 1 { "file" } else { "files" };
        Text::new(
            format!("ritsu check: ファイル {n} 個（{ja_by}）。{ja}。言語の境目: 確かめた {checked} か所、決められない {undecided} か所"),
            format!("ritsu check: {n} {files_en} ({en_by}): {en}; borders between the languages: {checked} checked, {undecided} undecided"),
        )
    }

    /// The JSON (DESIGN 8.3): the version, the root as seen from where ritsu runs, whether all is
    /// well, every file with its language and whether it passes, every diagnostic, and what the
    /// checks of the borders between the languages came to.
    pub fn json(&self) -> Json {
        let files = self.project.files.iter().map(|f| Json::obj([("tool", Json::str(f.tool.word())), ("file", Json::str(&f.rel)), ("ok", Json::Bool(self.status(f) == Status::Passes))]));
        let diagnostics = self.findings().map(|(t, f)| entry(t, f));
        Json::obj([
            ("ritsu", Json::str(env!("CARGO_PKG_VERSION"))),
            ("root", Json::str(self.project.root_shown())),
            ("ok", Json::Bool(self.code() == 0)),
            ("files", Json::arr(files)),
            ("diagnostics", Json::arr(diagnostics)),
            (
                "borders",
                Json::obj([("held", Json::int(self.borders.held as i128)), ("failed", Json::int(self.borders.failed as i128)), ("undecided", Json::int(self.borders.undecided as i128))]),
            ),
        ])
    }
}

/// One diagnostic of the JSON: the language's own object, with the tool put first and the file
/// written from the project's root (in place of the language's own `file`, or after the tool when
/// the language writes none).
fn entry(tool: &str, f: &Finding) -> Json {
    let file = match &f.file {
        Some(p) => Json::str(p),
        None => Json::Null,
    };
    let mut o: Vec<(String, Json)> = vec![("tool".into(), Json::str(tool))];
    match &f.json {
        Json::Obj(kv) => {
            if !kv.iter().any(|(k, _)| k == "file") {
                o.push(("file".into(), file.clone()));
            }
            for (k, v) in kv {
                o.push((k.clone(), if k == "file" { file.clone() } else { v.clone() }));
            }
        }
        _ => {
            o.push(("file".into(), file));
            o.push(("code".into(), Json::str(&f.code)));
            o.push(("severity".into(), Json::str(f.severity.key())));
            o.push(("line".into(), f.line.into()));
        }
    }
    Json::Obj(o)
}

fn refuse(msg: Text, lang: Lang) -> u8 {
    let head = if lang == Lang::Ja { "エラー" } else { "error" };
    eprintln!("{head}: {}", msg.get(lang));
    2
}

/// `ritsu check [<path>...] [--root <dir>] [--format json]`: the exit code.
pub fn command(args: &[String], lang: Lang) -> u8 {
    let table = cli::table();
    let cmd = table.command("check").expect("check is in the table");
    // `--lang` anywhere on the line is ritsu's, decided before anything is said
    let asked = args.iter().enumerate().find_map(|(i, x)| if x == "--lang" { args.get(i + 1).cloned() } else { x.strip_prefix("--lang=").map(str::to_string) });
    let lang = if asked.is_some() { Lang::pick(asked.as_deref(), "RITSU_LANG") } else { lang };
    let a = match table.parse(cmd, args) {
        Ok(a) => a,
        Err(e) => return refuse(e, lang),
    };
    if a.has("--help") {
        print!("{}", table.help_cmd(cmd, lang));
        return 0;
    }
    let project = match Project::load(&a.pos, a.get("--root")) {
        Ok(p) => p,
        Err(e) => return refuse(e, lang),
    };
    // a spec's claims start process groups; a signal stops them before ritsu goes
    if project.files.iter().any(|f| f.tool == Tool::Geas) {
        geas::proc::stop_groups_on_signals();
    }
    let joined = Joined::new();
    let units = project.check(&joined, lang);
    let report = Report { project: &project, units, ours: vec![], borders: Borders::default() };
    let mut out = std::io::stdout();
    if a.get("--format") == Some("json") {
        let _ = writeln!(out, "{}", report.json().pretty());
    } else {
        let _ = write!(out, "{}", report.text(lang));
    }
    let _ = out.flush();
    report.code()
}
