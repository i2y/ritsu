//! A `.gate` from its text to its diagnostics: the syntax (`parse.rs`, E001–E005), the names and
//! the types (`names.rs`, E006–E008, E101–E107, E201, E209), then the checks of every combination
//! over the scope the names give (DESIGN 4). What `sekisho check` prints for a file, as text and as
//! JSON, is here too.

use crate::diag::{self, Diag};
use crate::names::{self, Scope};
use crate::suite::Suite;
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text, plural};
use std::path::{Path, PathBuf};

/// How many combinations an action's check walks before it stops with E307 (DESIGN 4.1): koyomi's.
pub const DEFAULT_BUDGET: u64 = 100_000_000;

/// What a check is run with.
#[derive(Clone, Debug)]
pub struct Options {
    /// `--budget`: the most combinations of one action the check walks.
    pub budget: u64,
    /// `--root`: the root the references are written from (DESIGN 2.6), the project's for `ritsu
    /// check`. None finds it from the file checked, as yuen and sakai find theirs: the nearest
    /// directory above it that holds `.git`, else the file's directory ([`root_of`]).
    pub root: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Options {
        Options { budget: DEFAULT_BUDGET, root: None }
    }
}

/// The root of a run (ritsu's DESIGN 6.2, item 3): `--root`, else the nearest directory above the
/// first path given that holds `.git`, else the directory of that file.
pub fn root_of(root: Option<&Path>, first: &str) -> PathBuf {
    match root {
        Some(r) => ritsu_base::paths::absolute(r),
        None => ritsu_base::paths::find_root(Path::new(first)),
    }
}

/// What the check of one file comes to.
pub struct Outcome {
    /// The file, as it was given.
    pub path: String,
    pub diags: Vec<Diag>,
    /// The scope, when the names hold: what the later checks and the generators read.
    pub scope: Option<Scope>,
    /// What the checks of the borders, the contracts and every combination found (DESIGN 4), when
    /// the names hold: the model, what the other languages said, and each action's walk.
    pub walked: Option<crate::checks::Checked>,
    /// What was checked, for the line that says the file passes; None when something is wrong.
    pub ok: Option<Text>,
}

impl Outcome {
    pub fn has_errors(&self) -> bool {
        diag::has_errors(&self.diags)
    }

    /// Whether the file reads what this run cannot (E209): the command is run where a language is
    /// not joined, which is not what the file says, and exits 2.
    pub fn unjoined(&self) -> bool {
        self.diags.iter().any(|d| d.code == "E209")
    }

    /// The exit code of `sekisho check` for this file alone: 0, 1, or 2 for E209.
    pub fn exit(&self) -> u8 {
        if self.unjoined() {
            2
        } else if self.has_errors() {
            1
        } else {
            0
        }
    }
}

/// Check the text of `path`, with the languages `suite` joins.
pub fn check_text(path: &str, src: &str, suite: &Suite, opts: &Options) -> Outcome {
    let parsed = crate::parse::parse(path, src);
    let mut diags = parsed.diags;
    // a key in the file is in the repository whether the file reads or not (W901)
    diags.extend(crate::security::keys(path, src));
    let Some(file) = parsed.file else {
        diag::sort(&mut diags);
        return Outcome { path: path.to_string(), diags, scope: None, walked: None, ok: None };
    };
    let named = names::check(file, suite);
    diags.extend(named.diags);
    let scope = named.scope;
    // The checks of every combination (DESIGN 4: E3xx, and E202–E208 across the borders) read the
    // scope here, with `opts.budget`, and write the operations an action guards from the root.
    let root = root_of(opts.root.as_deref(), path);
    let walked = scope.as_ref().map(|s| {
        let (more, walked) = crate::checks::run(s, suite, opts.budget, &root);
        diags.extend(more);
        walked
    });
    diag::sort(&mut diags);
    let ok = match (&scope, diag::has_errors(&diags)) {
        (Some(s), false) => Some(summary(s)),
        _ => None,
    };
    Outcome { path: path.to_string(), diags, scope, walked, ok }
}

/// Check the file at `path`; Err when it cannot be read. It is read through ritsu-base's `fs`, the
/// disk or the files a page in the browser holds (ritsu's DESIGN 4.15).
pub fn check_file(path: &str, suite: &Suite, opts: &Options) -> Result<Outcome, String> {
    let src = ritsu_base::fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ok(check_text(path, &src, suite, opts))
}

/// `3 actions, 10 policies (7 permits, 3 forbids), 3 expectations, 1 separation`.
fn summary(s: &Scope) -> Text {
    let f = s.file();
    let permits = f.policies.iter().filter(|p| p.effect == crate::ast::Effect::Permit).count();
    let forbids = f.policies.len() - permits;
    let (a, p, e, sep) = (f.actions.len(), f.policies.len(), f.expects.len(), f.separates.len());
    tr!(
        "action {}、ポリシー {}（permit {}、forbid {}）、期待 {}、職務の分離 {}",
        "{}, {} ({}, {}), {}, {}",
        a, p, permits, forbids, e, sep;
        plural(a, "action", "actions"),
        plural(p, "policy", "policies"),
        plural(permits, "permit", "permits"),
        plural(forbids, "forbid", "forbids"),
        plural(e, "expectation", "expectations"),
        plural(sep, "separation", "separations")
    )
}

/// What `sekisho check` prints for one file: each diagnostic, then the line that says it passes.
pub fn render(o: &Outcome, lang: Lang) -> String {
    let mut s: String = o.diags.iter().map(|d| d.render(lang)).collect();
    if let Some(ok) = &o.ok {
        s.push_str(&format!("{}: ok — {}\n", o.path, ok.get(lang)));
    }
    s
}

/// The `--format json` of one file: `file`, `ok`, `summary`, `diagnostics`, in that order.
pub fn to_json(o: &Outcome, lang: Lang) -> Json {
    Json::obj([
        ("file", Json::str(&o.path)),
        ("ok", Json::from(!o.has_errors())),
        ("summary", Json::opt_str(o.ok.as_ref().map(|t| t.get(lang)))),
        ("diagnostics", Json::arr(o.diags.iter().map(|d| d.to_json(lang)))),
    ])
}

/// `sekisho check` of each file, as `ritsu check` prints it (ritsu's DESIGN 8.3): every finding, as
/// the command prints it and as its `--format json` prints it, then the line that says the file
/// passes. `files` are as the person gave them, from where the program runs; `root` is the
/// project's, which the references are written from too. A file that cannot be read, or that reads
/// a language `suite` does not join (E209), is one the command does not check (its exit 2).
/// `ritsu check` checks through the engine of the ports ([`crate::ports::Engine::checked`]), which
/// keeps each check for the questions asked of the gate after it.
pub fn checked(root: &Path, files: &[String], suite: &Suite, lang: Lang) -> Vec<ritsu_ports::Checked> {
    let opts = Options { root: Some(root.to_path_buf()), ..Options::default() };
    files.iter().map(|f| unit(root, f, check_file(f, suite, &opts).as_ref().map_err(|e| e.clone()), lang)).collect()
}

/// What `ritsu check` prints of one file (`f`, as the person gave it) that `check` came to, or
/// could not read.
pub fn unit(root: &Path, f: &str, checked: Result<&Outcome, String>, lang: Lang) -> ritsu_ports::Checked {
    use ritsu_ports::{Checked, Finding, Part, Verdict};
    match checked {
        Ok(o) => {
            let mut parts: Vec<Part> = o.diags.iter().map(|d| Part::Finding(Finding::of(d, ritsu_base::paths::from_root(root, Path::new(&d.file)), lang))).collect();
            if let Some(ok) = &o.ok {
                parts.push(Part::Text(format!("{}: ok — {}\n", o.path, ok.get(lang))));
            }
            let verdict = if o.unjoined() {
                Verdict::Unchecked
            } else if o.has_errors() {
                Verdict::Fails
            } else {
                Verdict::Passes
            };
            Checked { label: f.to_string(), parts, verdict }
        }
        Err(e) => {
            let msg = tr!("`{f}` を読めません: {e}", "cannot read `{f}`: {e}");
            let head = if lang == Lang::Ja { "エラー" } else { "error" };
            Checked::unchecked(f, format!("{head}: {}\n", msg.get(lang)))
        }
    }
}

/// Shorthand for a test or a tool: the file at `path` with no other language joined.
pub fn check(path: &str) -> Result<Outcome, String> {
    check_file(path, &Suite::default(), &Options::default())
}
