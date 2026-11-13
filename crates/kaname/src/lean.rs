//! `--lean` (DESIGN 38.6): the proofs beside the modules held, by Lean, to the statements kaname
//! writes of their properties. kaname proves nothing itself. It writes the module's Lean again from
//! the `.kn` into a directory of its own, reads each proof file as text for what a proof file may not
//! hold (38.5), copies what it read beside the Lean, and has Lean build it (`lake build`), replay its
//! declarations in the kernel (`leanchecker`), and say of each property whether the proof module
//! holds a theorem whose type is the statement itself and which rests on no axiom but Lean's three
//! (`Check.lean`, a program of kaname's own that imports only `Lean`).
//!
//! The text is read before anything is built because a build runs what the file says: a command
//! that evaluates code, a macro, an attribute that installs an elaborator. What is proved is not read
//! from the text, which notation and macros can disguise, but from the environment Lean built and
//! from the kernel.

use crate::check::Checked;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

/// The program that reads the environment (DESIGN 38.6, step 6).
pub const CHECK: &str = include_str!("gen/runtime/lean/Check.lean");

/// `lake`: on the `PATH`, else elan's (`$HOME/.elan/bin/lake`).
pub fn find_lake() -> Option<PathBuf> {
    if cfg!(target_family = "wasm") {
        return None;
    }
    let on_path = std::env::var_os("PATH").and_then(|ps| std::env::split_paths(&ps).map(|d| d.join("lake")).find(|p| p.is_file()));
    on_path.or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".elan/bin/lake")).filter(|p| p.is_file()))
}

/// How `--lean` runs Lean.
#[derive(Clone, Debug)]
pub struct LeanOptions {
    /// How long each run of Lean may take (`lake build`, `leanchecker`, the check): 10 minutes.
    pub timeout: Duration,
    /// Whether the proof files are read for what they may not hold before they are built. Only the
    /// tests turn it off, to show that the stages after it stop what they can stop too.
    #[doc(hidden)]
    pub scan: bool,
}

impl Default for LeanOptions {
    fn default() -> LeanOptions {
        LeanOptions { timeout: Duration::from_secs(600), scan: true }
    }
}

// ── The scan ────────────────────────────────────────────────────────────────

/// What the scan found in a proof file that it may not hold (DESIGN 38.5): where, what kind, and
/// the words as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Breach {
    pub line: usize,
    pub col: usize,
    pub kind: BreachKind,
    /// What is written there: `import Lean`, `#eval`, `@[implemented_by]`, `set_option
    /// debug.skipKernelTC`.
    pub what: String,
}

/// The kinds of what a proof file may not hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreachKind {
    /// An import of a module whose name does not start with `Kaname.`.
    Import,
    /// A command that runs code or adds syntax, a `#` command other than `#check`, `#print`,
    /// `#reduce` and `#guard_msgs`, or `unsafe`.
    Command,
    /// What has compiled code run in the build: `native_decide`, `decide +native` and a `decide`
    /// given a configuration, `Lean.reduceBool` and its kin.
    Native,
    /// An attribute other than `simp`, `grind`, `reducible`, `irreducible` and `instance`.
    Attribute,
    /// A `set_option` of an option other than those of time, depth, printing and lints.
    Option,
    /// A string that holds `{`: in an interpolated string, what is between braces is code.
    Interpolation,
}

/// The `#` commands a proof file may hold: they print, and run nothing.
const HASH_ALLOWED: [&str; 4] = ["check", "print", "reduce", "guard_msgs"];

/// The commands, tactics and terms that run code while the file is built, or add syntax that
/// changes how the rest of the file is read; and `unsafe`, the code that can do input and output
/// when anything evaluates it. Each is a keyword of Lean, so it cannot be a name.
const COMMANDS: [&str; 44] = [
    "run_cmd",
    "run_elab",
    "run_meta",
    "run_tac",
    "by_elab",
    "initialize",
    "builtin_initialize",
    "syntax",
    "macro",
    "macro_rules",
    "elab",
    "elab_rules",
    "declare_syntax_cat",
    "notation",
    "infix",
    "infixl",
    "infixr",
    "prefix",
    "postfix",
    "binder_predicate",
    "simproc",
    "dsimproc",
    "simproc_decl",
    "dsimproc_decl",
    "builtin_simproc",
    "builtin_dsimproc",
    "builtin_simproc_decl",
    "builtin_dsimproc_decl",
    "cbv_simproc",
    "cbv_simproc_decl",
    "builtin_cbv_simproc",
    "builtin_cbv_simproc_decl",
    "grind_propagator",
    "builtin_grind_propagator",
    "register_simp_attr",
    "register_error_explanation",
    "register_tactic_tag",
    "register_option",
    "register_builtin_option",
    "declare_simp_like_tactic",
    "declare_config_elab",
    "include_str",
    "unsafe",
    "builtin_macro",
];

/// The names whose use has the kernel or the elaborator run compiled code: a name of these, or a
/// name one of whose parts is one of these.
const NATIVE: [&str; 7] = ["native_decide", "native", "reduceBool", "ofReduceBool", "reduceNat", "ofReduceNat", "trustCompiler"];

/// The attributes a proof file may give: they change how a lemma is used, and install no code.
const ATTRIBUTES: [&str; 5] = ["simp", "grind", "reducible", "irreducible", "instance"];

/// Whether `set_option` may set the option `name`: the limits of time and depth, and how Lean
/// prints and lints and traces, but not the profiler, which can write a file.
fn option_allowed(name: &str) -> bool {
    let traced = name.strip_prefix("trace.").is_some_and(|t| t != "profiler" && !t.starts_with("profiler."));
    matches!(name, "maxHeartbeats" | "maxRecDepth") || traced || ["synthInstance.", "linter.", "pp."].iter().any(|p| name.starts_with(p))
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tk {
    /// A name, its parts joined by `.`, without the `«»` that escape a part.
    Ident(String),
    /// `#` and the word after it.
    Hash(String),
    /// A sign: `@[` as one, the rest a character each.
    Sym(String),
    /// A string, with where a `{` in it is, if one is.
    Str(Option<(usize, usize)>),
    /// A number or a character.
    Lit,
}

#[derive(Clone, Debug)]
struct Tok {
    tk: Tk,
    line: usize,
    col: usize,
}

/// The characters a name continues with, as Lean reads them.
fn id_rest(c: char) -> bool {
    ritsu_emit::ident::is_letter_or_number(c) && !matches!(c, 'λ' | 'Π' | 'Σ') || matches!(c, '_' | '\'' | '!' | '?') || ('\u{2080}'..='\u{209C}').contains(&c) || ('\u{1D62}'..='\u{1D6A}').contains(&c)
}

/// The characters a name starts with.
fn id_start(c: char) -> bool {
    (ritsu_emit::ident::is_letter(c) && !matches!(c, 'λ' | 'Π' | 'Σ')) || c == '_'
}

/// Where a character literal that starts at `i` ends (its closing `'`): `'a'`, `'"'`, `'\n'`,
/// `'\''`, `'\x41'`, `'\u{41}'`. None where the `'` starts none.
fn char_end(cs: &[char], i: usize) -> Option<usize> {
    if cs.get(i) != Some(&'\'') {
        return None;
    }
    match cs.get(i + 1) {
        None | Some('\'') | Some('\n') => None,
        Some('\\') => (i + 3..(i + 14).min(cs.len())).take_while(|&k| cs[k] != '\n').find(|&k| cs[k] == '\''),
        Some(_) => (cs.get(i + 2) == Some(&'\'')).then_some(i + 2),
    }
}

/// The tokens of a proof file, with its comments (`--` to the end of the line, `/- … -/` nested),
/// its strings and its characters read past (DESIGN 38.5).
fn tokens(text: &str) -> Vec<Tok> {
    let cs: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line, mut col) = (0usize, 1usize, 1usize);
    // advances past `n` characters, counting lines
    let step = |i: &mut usize, line: &mut usize, col: &mut usize, n: usize| {
        for _ in 0..n {
            if *i >= cs.len() {
                return;
            }
            if cs[*i] == '\n' {
                *line += 1;
                *col = 1;
            } else {
                *col += 1;
            }
            *i += 1;
        }
    };
    let at = |i: usize, s: &str| -> bool {
        let mut k = i;
        for c in s.chars() {
            if cs.get(k) != Some(&c) {
                return false;
            }
            k += 1;
        }
        true
    };
    while i < cs.len() {
        let c = cs[i];
        let (l0, c0) = (line, col);
        if c.is_whitespace() {
            step(&mut i, &mut line, &mut col, 1);
        } else if at(i, "--") {
            while i < cs.len() && cs[i] != '\n' {
                step(&mut i, &mut line, &mut col, 1);
            }
        } else if at(i, "/-") {
            // nested, as Lean reads them: only `/-` and `-/` count inside
            let mut depth = 0usize;
            while i < cs.len() {
                if at(i, "/-") {
                    depth += 1;
                    step(&mut i, &mut line, &mut col, 2);
                } else if at(i, "-/") {
                    depth -= 1;
                    step(&mut i, &mut line, &mut col, 2);
                    if depth == 0 {
                        break;
                    }
                } else {
                    step(&mut i, &mut line, &mut col, 1);
                }
            }
        } else if c == '"' {
            step(&mut i, &mut line, &mut col, 1);
            let mut brace = None;
            while i < cs.len() && cs[i] != '"' {
                if cs[i] == '\\' {
                    step(&mut i, &mut line, &mut col, 2);
                    continue;
                }
                if cs[i] == '{' && brace.is_none() {
                    brace = Some((line, col));
                }
                step(&mut i, &mut line, &mut col, 1);
            }
            step(&mut i, &mut line, &mut col, 1);
            out.push(Tok { tk: Tk::Str(brace), line: l0, col: c0 });
        } else if c == 'r' && {
            let mut k = i + 1;
            while cs.get(k) == Some(&'#') {
                k += 1;
            }
            cs.get(k) == Some(&'"')
        } {
            // a raw string: `r"…"`, `r#"…"#`, with as many `#` at its end as at its start
            let mut hashes = 0;
            step(&mut i, &mut line, &mut col, 1);
            while i < cs.len() && cs[i] == '#' {
                hashes += 1;
                step(&mut i, &mut line, &mut col, 1);
            }
            step(&mut i, &mut line, &mut col, 1);
            let mut brace = None;
            while i < cs.len() {
                if cs[i] == '"' && (1..=hashes).all(|h| cs.get(i + h) == Some(&'#')) {
                    step(&mut i, &mut line, &mut col, 1 + hashes);
                    break;
                }
                if cs[i] == '{' && brace.is_none() {
                    brace = Some((line, col));
                }
                step(&mut i, &mut line, &mut col, 1);
            }
            out.push(Tok { tk: Tk::Str(brace), line: l0, col: c0 });
        } else if let Some(end) = char_end(&cs, i) {
            let n = end + 1 - i;
            step(&mut i, &mut line, &mut col, n);
            out.push(Tok { tk: Tk::Lit, line: l0, col: c0 });
        } else if id_start(c) || c == '«' {
            let mut name = String::new();
            loop {
                if i < cs.len() && cs[i] == '«' {
                    step(&mut i, &mut line, &mut col, 1);
                    while i < cs.len() && cs[i] != '»' {
                        name.push(cs[i]);
                        step(&mut i, &mut line, &mut col, 1);
                    }
                    step(&mut i, &mut line, &mut col, 1);
                } else {
                    while i < cs.len() && id_rest(cs[i]) {
                        name.push(cs[i]);
                        step(&mut i, &mut line, &mut col, 1);
                    }
                }
                // a part follows a `.` written right after the name
                if i + 1 < cs.len() && cs[i] == '.' && (id_start(cs[i + 1]) || cs[i + 1] == '«') {
                    name.push('.');
                    step(&mut i, &mut line, &mut col, 1);
                } else {
                    break;
                }
            }
            out.push(Tok { tk: Tk::Ident(name), line: l0, col: c0 });
        } else if c.is_ascii_digit() {
            while i < cs.len() && (cs[i].is_ascii_alphanumeric() || cs[i] == '_' || (cs[i] == '.' && cs.get(i + 1).is_some_and(char::is_ascii_digit))) {
                step(&mut i, &mut line, &mut col, 1);
            }
            out.push(Tok { tk: Tk::Lit, line: l0, col: c0 });
        } else if c == '#' && cs.get(i + 1).is_some_and(|&n| id_start(n)) {
            step(&mut i, &mut line, &mut col, 1);
            let mut word = String::new();
            while i < cs.len() && id_rest(cs[i]) {
                word.push(cs[i]);
                step(&mut i, &mut line, &mut col, 1);
            }
            out.push(Tok { tk: Tk::Hash(word), line: l0, col: c0 });
        } else if at(i, "@[") {
            step(&mut i, &mut line, &mut col, 2);
            out.push(Tok { tk: Tk::Sym("@[".into()), line: l0, col: c0 });
        } else {
            step(&mut i, &mut line, &mut col, 1);
            out.push(Tok { tk: Tk::Sym(c.to_string()), line: l0, col: c0 });
        }
    }
    out
}

/// The attributes of a list that starts after `toks[start]` (`@[` or `attribute [`): each one's name
/// (after `local`, `scoped` and the `-` that takes one away), with where it is; and the token after
/// the list.
fn attributes(toks: &[Tok], start: usize) -> (Vec<(String, usize, usize)>, usize) {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut fresh = true;
    let mut k = start + 1;
    while k < toks.len() {
        match &toks[k].tk {
            Tk::Sym(s) if (s == "]" || s == ")") && depth == 0 => return (out, k + 1),
            Tk::Sym(s) if s == "[" || s == "(" || s == "@[" => depth += 1,
            Tk::Sym(s) if s == "]" || s == ")" => depth -= 1,
            Tk::Sym(s) if s == "," && depth == 0 => fresh = true,
            Tk::Sym(s) if s == "-" && fresh => {}
            Tk::Ident(w) if fresh && (w == "local" || w == "scoped") => {}
            Tk::Ident(w) if fresh => {
                out.push((w.clone(), toks[k].line, toks[k].col));
                fresh = false;
            }
            _ if fresh => {
                // a list entry that does not start with a name: read as an attribute of no name
                out.push((String::new(), toks[k].line, toks[k].col));
                fresh = false;
            }
            _ => {}
        }
        k += 1;
    }
    (out, k)
}

/// What a proof file holds that it may not (DESIGN 38.5), in the order written. Comments, strings and
/// characters are read past; a name escaped with `«»` is read as the name it is.
pub fn scan(text: &str) -> Vec<Breach> {
    let toks = tokens(text);
    let mut out = Vec::new();
    let breach = |t: &Tok, kind: BreachKind, what: String| Breach { line: t.line, col: t.col, kind, what };
    let ident = |k: usize| match toks.get(k).map(|t| &t.tk) {
        Some(Tk::Ident(w)) => Some(w.as_str()),
        _ => None,
    };
    let sym = |k: usize, s: &str| matches!(toks.get(k).map(|t| &t.tk), Some(Tk::Sym(x)) if x == s);
    let mut k = 0;
    while k < toks.len() {
        let t = &toks[k];
        match &t.tk {
            Tk::Ident(w) if w == "import" => {
                let mut m = k + 1;
                if ident(m) == Some("all") {
                    m += 1;
                }
                let module = ident(m).unwrap_or("");
                if !module.starts_with("Kaname.") {
                    out.push(breach(t, BreachKind::Import, format!("import {module}").trim_end().to_string()));
                }
            }
            Tk::Ident(w) if COMMANDS.contains(&w.as_str()) => out.push(breach(t, BreachKind::Command, w.clone())),
            Tk::Ident(w) if w == "set_option" => {
                let name = ident(k + 1).unwrap_or("");
                if !option_allowed(name) {
                    out.push(breach(t, BreachKind::Option, format!("set_option {name}").trim_end().to_string()));
                }
            }
            Tk::Ident(w) if w == "attribute" && sym(k + 1, "[") => {
                let (names, next) = attributes(&toks, k + 1);
                for (n, line, col) in names.into_iter().filter(|(n, _, _)| !ATTRIBUTES.contains(&n.as_str())) {
                    out.push(Breach { line, col, kind: BreachKind::Attribute, what: format!("attribute [{n}]") });
                }
                k = next;
                continue;
            }
            Tk::Ident(w) if w == "decide" && sym(k + 1, "(") && ident(k + 2) == Some("config") => {
                out.push(breach(t, BreachKind::Native, "decide (config := …)".into()));
            }
            Tk::Ident(w) if w.split('.').any(|part| NATIVE.contains(&part)) => out.push(breach(t, BreachKind::Native, w.clone())),
            Tk::Hash(w) if !HASH_ALLOWED.contains(&w.as_str()) => out.push(breach(t, BreachKind::Command, format!("#{w}"))),
            Tk::Sym(s) if s == "@[" => {
                let (names, next) = attributes(&toks, k);
                for (n, line, col) in names.into_iter().filter(|(n, _, _)| !ATTRIBUTES.contains(&n.as_str())) {
                    out.push(Breach { line, col, kind: BreachKind::Attribute, what: format!("@[{n}]") });
                }
                k = next;
                continue;
            }
            Tk::Str(Some((line, col))) => out.push(Breach { line: *line, col: *col, kind: BreachKind::Interpolation, what: "{".into() }),
            _ => {}
        }
        k += 1;
    }
    out
}

/// Where the theorem of the property `prop` is written in a proof file, read as text: the line and
/// column of the word `theorem` before its name. Only for saying where: what the theorem proves is
/// read from Lean.
pub fn theorem_at(text: &str, alias: &str, prop: &str) -> Option<(usize, usize)> {
    let toks = tokens(text);
    let full = format!("Proofs.{alias}.{prop}");
    toks.windows(2).find_map(|w| match (&w[0].tk, &w[1].tk) {
        (Tk::Ident(a), Tk::Ident(n)) if (a == "theorem" || a == "lemma") && (n == prop || *n == full || n.ends_with(&format!(".{prop}"))) => Some((w[0].line, w[0].col)),
        _ => None,
    })
}

// ── The answers ─────────────────────────────────────────────────────────────

/// A property sent to Lean: the place of its module among those given, and its place among the
/// module's properties.
pub type PropertyId = (usize, usize);

/// What Lean says of one property.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The proof module holds a theorem of the statement, resting on these of Lean's three axioms.
    Shown(Vec<String>),
    /// The proof module builds, and holds no theorem of that name: no proof of it yet.
    NoTheorem,
    /// The writer could not write its statement (why, as the comment in the Lean says it).
    NotWritten(String),
    Rejected(Rejection),
}

/// Why Lean does not take a proof (E604).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// `lake build` stopped on the proof file: where, in the proof file, and what Lean said.
    Build { line: usize, col: usize, message: String },
    /// A declaration of the theorem's name that is not a theorem (what it is: `def`, `axiom`).
    NotATheorem(String),
    /// A theorem of that name declared in another module (which).
    Elsewhere(String),
    /// A theorem whose type is not the statement (its type).
    StatesOtherwise(String),
    /// A theorem that rests on other axioms (their names).
    RestsOn(Vec<String>),
    /// The kernel does not take a declaration of the proof file (what `leanchecker` said).
    Kernel(String),
    /// The proof module and the module written from the `.kn` cannot be read together (what Lean
    /// said: the proof file declares a name of the module again).
    Load(String),
    /// The file holds what a proof file may not, and is not built.
    Breach(Breach),
    /// Lean did not finish within the time.
    Timeout(Duration),
}

impl Rejection {
    /// Whether it is said of the file once rather than of each property.
    pub fn of_the_file(&self) -> bool {
        matches!(self, Rejection::Build { .. } | Rejection::Kernel(_) | Rejection::Load(_) | Rejection::Breach(_) | Rejection::Timeout(_))
    }
}

/// Why `--lean` could not run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeanError {
    /// `lake` is neither on the `PATH` nor in `~/.elan/bin`.
    Missing,
    /// The directory, the files or a process could not be made (what was said).
    Io(String),
}

impl LeanError {
    pub fn text(&self) -> ritsu_base::text::Text {
        match self {
            LeanError::Missing => {
                let tc = crate::r#gen::lean::TOOLCHAIN;
                tr!(
                    "--lean には Lean が要ります。`lake` が PATH にも ~/.elan/bin にもありません。Lean の版を管理する elan を入れてください。証明が使う toolchain（{tc}）は、最初に走らせたときに入ります",
                    "--lean needs Lean: `lake` is neither on PATH nor in ~/.elan/bin. Install elan, the Lean version manager; the toolchain the proofs use ({tc}) is installed on the first run"
                )
            }
            LeanError::Io(e) => tr!("--lean が Lean を走らせられません: {e}", "--lean cannot run Lean: {e}"),
        }
    }
}

// ── Running Lean ────────────────────────────────────────────────────────────

/// A directory of its own under the system's, taken away when this goes.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch() -> Result<Scratch, LeanError> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("kaname-lean-{}-{stamp}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&dir).map_err(|e| LeanError::Io(format!("{}: {e}", dir.display())))?;
    Ok(Scratch(dir))
}

/// How a run of a program ended.
enum Ran {
    Done { ok: bool, said: String },
    TimedOut,
}

/// `lake <args>` in `dir`, in a process group of its own, stopped with what it started when it
/// takes longer than `limit`: what it printed on either stream.
fn lake_run(lake: &Path, dir: &Path, args: &[&str], limit: Duration) -> Result<Ran, LeanError> {
    use std::process::{Command, Stdio};
    let log = dir.join(format!(".kaname-{}.log", args.first().copied().unwrap_or("lake")));
    let file = std::fs::File::create(&log).map_err(|e| LeanError::Io(format!("{}: {e}", log.display())))?;
    let err = file.try_clone().map_err(|e| LeanError::Io(e.to_string()))?;
    let mut cmd = Command::new(lake);
    cmd.current_dir(dir).args(args).stdin(Stdio::null()).stdout(Stdio::from(file)).stderr(Stdio::from(err));
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    let mut child = cmd.spawn().map_err(|e| LeanError::Io(format!("{}: {e}", lake.display())))?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let said = std::fs::read(&log).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
                return Ok(Ran::Done { ok: status.success(), said });
            }
            Ok(None) if start.elapsed() >= limit => {
                // std signals no group, and kaname depends on nothing that does
                #[cfg(unix)]
                let group = format!("-{}", child.id());
                #[cfg(unix)]
                let _ = Command::new("kill").args(["-KILL", "--", &group]).stdout(Stdio::null()).stderr(Stdio::null()).status();
                let _ = child.kill();
                let _ = child.wait();
                // what lake started is gone before its directory is taken away
                #[cfg(unix)]
                for _ in 0..100 {
                    let alive = Command::new("kill").args(["-0", "--", &group]).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success());
                    if !alive {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                return Ok(Ran::TimedOut);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(LeanError::Io(e.to_string())),
        }
    }
}

/// The first error `lake build` printed of the file `rel` (`Proofs/<alias>.lean`): its line,
/// column and message (its first lines). The lines of what lake ran (`trace:`) are passed over.
fn first_error(said: &str, rel: &str) -> Option<(usize, usize, String)> {
    let lines: Vec<&str> = said.lines().filter(|l| !l.starts_with("trace:")).collect();
    for (k, l) in lines.iter().enumerate() {
        let Some(rest) = l.strip_prefix("error: ") else { continue };
        let rest = rest.trim_start_matches("./").trim_start_matches("./");
        let Some(after) = rest.strip_prefix(rel).and_then(|r| r.strip_prefix(':')) else { continue };
        let mut parts = after.splitn(3, ':');
        let (Some(line), Some(col), Some(msg)) = (parts.next(), parts.next(), parts.next()) else { continue };
        let (Ok(line), Ok(col)) = (line.trim().parse::<usize>(), col.trim().parse::<usize>()) else { continue };
        let mut message = msg.trim().to_string();
        for more in lines[k + 1..].iter().take_while(|m| m.starts_with(' ') || !(m.starts_with("error") || m.starts_with("warning") || m.starts_with("info") || m.starts_with('✖') || m.starts_with('✔') || m.starts_with('⚠') || m.starts_with("Some required") || m.starts_with("- "))).take(8) {
            // indented under the first line, Lean's own indentation kept
            message.push_str("\n    ");
            message.push_str(more.trim_end());
        }
        // Lean counts columns from 0; a person and the other diagnostics count from 1
        return Some((line, col + 1, message));
    }
    None
}

/// The first error of the build, of whatever file, for a proof module whose file has none: the
/// Lean it reads did not build.
fn any_error(said: &str) -> String {
    said.lines().filter(|l| l.starts_with("error: ") && !l.starts_with("error: build failed")).map(|l| l.trim_start_matches("error: ").to_string()).next().unwrap_or_else(|| said.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").to_string())
}

/// Each property of `modules` that is written for Lean, held by Lean to its statement, with the proof
/// beside its module (DESIGN 38.6, steps 2 to 6): `(module, property)` with the verdict. A module
/// with no proof beside it gets no verdicts. `lake` is looked for only when a proof file passes the
/// scan.
pub fn check(modules: &[Rc<Checked>], opts: &LeanOptions) -> Result<Vec<(PropertyId, Verdict)>, LeanError> {
    let mut out: Vec<(PropertyId, Verdict)> = Vec::new();
    // the package, written again from the `.kn` files
    let (files, stated) = crate::r#gen::lean::package_stating(modules);
    // each module's proof file, read as text and scanned
    struct Sent {
        m: usize,
        alias: String,
        props: Vec<(usize, String)>,
        rel: String,
    }
    let mut sent: Vec<Sent> = Vec::new();
    let mut proofs: Vec<(PathBuf, String)> = Vec::new();
    for (m, c) in modules.iter().enumerate() {
        let path = crate::properties::proof_path(c);
        if c.properties.is_empty() || !ritsu_base::fs::is_file(&path) {
            continue;
        }
        let alias = c.module.name.ascii().to_string();
        let written = stated.get(&c.canon).cloned().unwrap_or_default();
        let full = c.lowered.clone().unwrap_or_else(|| c.clone());
        let mut props = Vec::new();
        for (k, cp) in c.properties.iter().enumerate() {
            if written.contains(&k) {
                props.push((k, cp.decl.name.ascii().to_string()));
            } else if crate::r#gen::lean::obstacle(&crate::value::Program::of(&full), &full, cp.lowered).is_none() {
                out.push(((m, k), Verdict::NotWritten(written_why(&files, &alias, cp.decl.name.ascii()))));
            }
        }
        if props.is_empty() {
            continue;
        }
        let text = match ritsu_base::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                let r = Rejection::Build { line: 1, col: 1, message: format!("the file cannot be read: {e}") };
                out.extend(props.iter().map(|(k, _)| ((m, *k), Verdict::Rejected(r.clone()))));
                continue;
            }
        };
        if opts.scan
            && let Some(b) = scan(&text).into_iter().next()
        {
            out.extend(props.iter().map(|(k, _)| ((m, *k), Verdict::Rejected(Rejection::Breach(b.clone())))));
            continue;
        }
        let rel = format!("Proofs/{alias}.lean");
        proofs.push((PathBuf::from(&rel), text));
        sent.push(Sent { m, alias, props, rel });
    }
    if sent.is_empty() {
        return Ok(out);
    }
    let lake = find_lake().ok_or(LeanError::Missing)?;
    let dir = scratch()?;
    let root = dir.0.clone();
    let write = |rel: &Path, text: &str| -> Result<(), LeanError> {
        let p = root.join(rel);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| LeanError::Io(format!("{}: {e}", d.display())))?;
        }
        std::fs::write(&p, text).map_err(|e| LeanError::Io(format!("{}: {e}", p.display())))
    };
    for (rel, text) in files.iter().chain(&proofs) {
        write(rel, text)?;
    }
    // step 4: the build, of the modules written from the `.kn` files and of the proof modules (a
    // proof file that does not import its module still has it built, to be read beside it)
    let targets: Vec<String> = sent.iter().flat_map(|s| [format!("Kaname.{}", s.alias), format!("Proofs.{}", s.alias)]).collect();
    let mut args = vec!["build"];
    args.extend(targets.iter().map(String::as_str));
    let built = lake_run(&lake, &root, &args, opts.timeout)?;
    let olean = |alias: &str| root.join(format!(".lake/build/lib/lean/Proofs/{alias}.olean"));
    let mut next: Vec<&Sent> = Vec::new();
    for s in &sent {
        if olean(&s.alias).is_file() {
            next.push(s);
            continue;
        }
        let r = match &built {
            Ran::TimedOut => Rejection::Timeout(opts.timeout),
            Ran::Done { said, .. } => match first_error(said, &s.rel) {
                Some((line, col, message)) => Rejection::Build { line, col, message },
                None => Rejection::Build { line: 1, col: 1, message: any_error(said) },
            },
        };
        out.extend(s.props.iter().map(|(k, _)| ((s.m, *k), Verdict::Rejected(r.clone()))));
    }
    // step 5: each proof module's declarations replayed in the kernel
    let mut checked: Vec<&Sent> = Vec::new();
    for s in next {
        let module = format!("Proofs.{}", s.alias);
        match lake_run(&lake, &root, &["env", "leanchecker", &module], opts.timeout)? {
            Ran::Done { ok: true, .. } => checked.push(s),
            Ran::Done { said, .. } => {
                let r = Rejection::Kernel(said.lines().filter(|l| !l.trim().is_empty()).take(4).collect::<Vec<_>>().join("\n"));
                out.extend(s.props.iter().map(|(k, _)| ((s.m, *k), Verdict::Rejected(r.clone()))));
            }
            Ran::TimedOut => out.extend(s.props.iter().map(|(k, _)| ((s.m, *k), Verdict::Rejected(Rejection::Timeout(opts.timeout))))),
        }
    }
    if checked.is_empty() {
        return Ok(out);
    }
    // step 6: what the environment holds of each property
    let asked = ritsu_base::json::Json::arr(checked.iter().map(|s| {
        ritsu_base::json::Json::obj([
            ("module", ritsu_base::json::Json::str(s.alias.clone())),
            ("properties", ritsu_base::json::Json::arr(s.props.iter().map(|(_, p)| ritsu_base::json::Json::str(p.clone())))),
        ])
    }));
    write(Path::new("Check.lean"), CHECK)?;
    write(Path::new("asked.json"), &asked.compact())?;
    let said = match lake_run(&lake, &root, &["env", "lean", "--run", "Check.lean", "asked.json"], opts.timeout)? {
        Ran::Done { said, .. } => said,
        Ran::TimedOut => {
            for s in &checked {
                out.extend(s.props.iter().map(|(k, _)| ((s.m, *k), Verdict::Rejected(Rejection::Timeout(opts.timeout)))));
            }
            return Ok(out);
        }
    };
    let lines: Vec<ritsu_base::json::Json> = said.lines().filter_map(|l| ritsu_base::json::parse(l).ok()).collect();
    for s in &checked {
        let of = |j: &&ritsu_base::json::Json| j.get("module").and_then(|x| x.as_str()) == Some(s.alias.as_str());
        if let Some(e) = lines.iter().filter(of).find_map(|j| j.get("error").and_then(|x| x.as_str())) {
            let r = Rejection::Load(e.to_string());
            out.extend(s.props.iter().map(|(k, _)| ((s.m, *k), Verdict::Rejected(r.clone()))));
            continue;
        }
        for (k, p) in &s.props {
            let found = lines.iter().filter(of).find(|j| j.get("property").and_then(|x| x.as_str()) == Some(p.as_str()));
            let strs = |j: &ritsu_base::json::Json, key: &str| -> Vec<String> { j.get(key).and_then(|x| x.as_arr()).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default() };
            let v = match found {
                None => Verdict::Rejected(Rejection::Load(format!("Lean said nothing of it:\n{}", said.lines().take(6).collect::<Vec<_>>().join("\n")))),
                Some(j) => {
                    let ty = j.get("type").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    match j.get("verdict").and_then(|x| x.as_str()).unwrap_or("") {
                        "proved" => Verdict::Shown(strs(j, "axioms")),
                        "missing" => Verdict::NoTheorem,
                        "not-a-theorem" => Verdict::Rejected(Rejection::NotATheorem(ty)),
                        "elsewhere" => Verdict::Rejected(Rejection::Elsewhere(ty)),
                        "states-otherwise" => Verdict::Rejected(Rejection::StatesOtherwise(ty)),
                        "rests-on" => Verdict::Rejected(Rejection::RestsOn(strs(j, "axioms"))),
                        other => Verdict::Rejected(Rejection::Load(format!("the statement is not the one written from the module ({other})"))),
                    }
                }
            };
            out.push(((s.m, *k), v));
        }
    }
    Ok(out)
}

/// Why the writer wrote no statement of the property `prop` of the module `alias`: the comment it
/// left in the module's Lean.
fn written_why(files: &[(PathBuf, String)], alias: &str, prop: &str) -> String {
    let path = PathBuf::from(format!("Kaname/{alias}.lean"));
    let head = "-- The property `";
    files
        .iter()
        .find(|(p, _)| *p == path)
        .and_then(|(_, t)| t.lines().find(|l| l.starts_with(head) && l.contains(&format!("{prop}`"))))
        .and_then(|l| l.split_once(": ").map(|(_, why)| why.trim_end_matches('.').to_string()))
        .unwrap_or_else(|| "its statement is not written".into())
}

/// The names the axioms a proof rests on are said by: `native_decide` for the axiom Lean makes of
/// each use of it (`<theorem>._native.native_decide.ax_1_1`), each once.
pub fn axiom_names(axioms: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for a in axioms {
        let n = if a.contains("._native.native_decide.") { "native_decide".to_string() } else { a.clone() };
        if !out.contains(&n) {
            out.push(n);
        }
    }
    out
}
