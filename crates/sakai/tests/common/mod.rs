//! What the tests share (PLAN 0.1): a temporary directory removed when the test is done (and
//! those of test processes that have ended, removed by the first one), copying a fixture, finding
//! a tool, running a program with a time limit (macOS has no `timeout`), and golden files.

#![allow(dead_code)]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// A directory under the system's temporary directory, `sakai-test-<pid>-<n>`, removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new() -> TempDir {
        static SWEPT: std::sync::Once = std::sync::Once::new();
        SWEPT.call_once(remove_ended);
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("sakai-test-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn write(&self, rel: &str, body: &str) {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Removes the directories of [`TempDir`] whose test process has ended: `kill -0` says there is
/// no such process. A process that is there, or that kill may not ask about, keeps its own.
fn remove_ended() {
    let me = std::process::id().to_string();
    let mut ended = std::collections::HashMap::new();
    for e in std::fs::read_dir(std::env::temp_dir()).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(pid) = name.strip_prefix("sakai-test-").and_then(|n| n.split('-').next()) else { continue };
        if pid == me || pid.is_empty() || !pid.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let gone = *ended.entry(pid.to_string()).or_insert_with(|| {
            Command::new("kill").args(["-0", pid]).output().is_ok_and(|o| !o.status.success() && String::from_utf8_lossy(&o.stderr).contains("No such process"))
        });
        if gone {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

/// Copy a directory, every file under it.
pub fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    let mut es: Vec<_> = std::fs::read_dir(from).unwrap().map(|e| e.unwrap()).collect();
    es.sort_by_key(|e| e.file_name());
    for e in es {
        let p = e.path();
        let q = to.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &q);
        } else {
            std::fs::copy(&p, &q).unwrap();
        }
    }
}

/// A tool named by an environment variable, else at `fallback` (relative to the repository),
/// else on the PATH under `name`.
pub fn tool(var: &str, fallback: &str, name: &str) -> Option<String> {
    if let Ok(p) = std::env::var(var)
        && !p.is_empty()
    {
        return Some(p);
    }
    if !fallback.is_empty() && Path::new(fallback).exists() {
        return Some(std::fs::canonicalize(fallback).unwrap().to_string_lossy().to_string());
    }
    let runs = Command::new(name).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
    if runs { Some(name.to_string()) } else { None }
}

/// What a program printed and how it ended.
pub struct Ran {
    pub ok: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Run a command to its end, killing it after `limit`.
pub fn run(cmd: &mut Command, limit: Duration) -> Ran {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap_or_else(|e| panic!("cannot run {cmd:?}: {e}"));
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let o = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = out.read_to_end(&mut s);
        s
    });
    let e = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = err.read_to_end(&mut s);
        s
    });
    let start = Instant::now();
    let (ok, code, timed_out) = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break (s.success(), s.code(), false);
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            break (false, None, true);
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    Ran { ok, code, stdout: String::from_utf8_lossy(&o.join().unwrap()).to_string(), stderr: String::from_utf8_lossy(&e.join().unwrap()).to_string(), timed_out }
}

/// `sakai` with these arguments, in `dir`, with `SAKAI_LANG` unset.
pub fn sakai_in(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_sakai")).args(args).current_dir(dir).env_remove("SAKAI_LANG").output().unwrap()
}

pub fn sakai(args: &[&str]) -> std::process::Output {
    sakai_in(Path::new("."), args)
}

/// Hold `got` to the golden file `path`; `SAKAI_BLESS=1` writes it instead. The failure, if any,
/// is returned for the caller to collect.
pub fn golden(path: &str, got: &str) -> Option<String> {
    if std::env::var("SAKAI_BLESS").is_ok() {
        std::fs::create_dir_all(Path::new(path).parent().unwrap()).unwrap();
        std::fs::write(path, got).unwrap();
        return None;
    }
    let want = std::fs::read_to_string(path).unwrap_or_default();
    if want == got { None } else { Some(format!("{path} differs:\n--- want\n{want}--- got\n{got}")) }
}

/// The base a mutant names: a map of `tests/maps/`, or the example (`examples/通販`).
pub fn base_dir(base: &str) -> PathBuf {
    let base = base.trim();
    if base.starts_with("examples/") { PathBuf::from(base) } else { Path::new("tests/maps").join(base) }
}

/// The mutant `name` of `tests/mutants/` laid out in a temporary directory: its base fixture
/// (named in its file `base`) copied first, the mutant's own files over it, and the paths its
/// file `remove` lists taken away. A mutant's `README.md` says why it gives more than its code,
/// and its file `command`, when there is one, the command it is run with (`build …`).
pub fn mutant(name: &str) -> TempDir {
    let dir = TempDir::new();
    let src = Path::new("tests/mutants").join(name);
    if let Ok(base) = std::fs::read_to_string(src.join("base")) {
        copy_dir(&base_dir(&base), dir.path());
    }
    lay_over(&src, &src, dir.path());
    if let Ok(rm) = std::fs::read_to_string(src.join("remove")) {
        for l in rm.lines().filter(|l| !l.trim().is_empty()) {
            let p = dir.path().join(l.trim());
            if p.is_dir() {
                std::fs::remove_dir_all(&p).unwrap();
            } else {
                std::fs::remove_file(&p).unwrap_or_else(|e| panic!("{name}: cannot remove {l}: {e}"));
            }
        }
    }
    dir
}

fn lay_over(top: &Path, from: &Path, to: &Path) {
    let mut es: Vec<_> = std::fs::read_dir(from).unwrap().map(|e| e.unwrap()).collect();
    es.sort_by_key(|e| e.file_name());
    for e in es {
        let p = e.path();
        if from == top && (e.file_name() == "base" || e.file_name() == "remove" || e.file_name() == "README.md" || e.file_name() == "command") {
            continue;
        }
        let q = to.join(e.file_name());
        if p.is_dir() {
            std::fs::create_dir_all(&q).unwrap();
            lay_over(top, &p, &q);
        } else {
            std::fs::copy(&p, &q).unwrap();
        }
    }
}

/// The names of the mutants, in order.
pub fn mutants() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir("tests/mutants").unwrap().filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    v.sort();
    v
}

/// A base map of `tests/maps/` laid out in a temporary directory, with each `(file, old, new)`
/// replacing the first `old` of the file with `new` (a file named with no `old` is written whole).
pub fn variant(base: &str, edits: &[(&str, &str, &str)]) -> TempDir {
    let dir = TempDir::new();
    copy_dir(&Path::new("tests/maps").join(base), dir.path());
    for (file, old, new) in edits {
        let p = dir.path().join(file);
        if old.is_empty() {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, new).unwrap();
            continue;
        }
        let s = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(s.contains(old), "{file} has no {old:?}");
        std::fs::write(&p, s.replacen(old, new, 1)).unwrap();
    }
    dir
}

/// `sakai check` on a directory, with the directory as the root.
pub fn check_dir(dir: &Path) -> Vec<sakai::check::Outcome> {
    sakai::check::check_args(dir, &[".".to_string()]).unwrap()
}

/// The codes the outcomes give, in order.
pub fn codes(os: &[sakai::check::Outcome]) -> Vec<&'static str> {
    os.iter().flat_map(|o| o.diags.iter().map(|d| d.code)).collect()
}

/// What the outcomes print, in a language.
pub fn text(os: &[sakai::check::Outcome], lang: sakai::i18n::Lang) -> String {
    os.iter().map(|o| sakai::check::render(o, lang)).collect()
}

/// Says that a test did not run what it is for, and why: the line `SKIP: <why>` (PLAN 0.1).
pub fn skip(why: &str) {
    println!("SKIP: {why}");
}

/// The example of the repository.
pub const EXAMPLE: &str = "examples/通販";

/// A program at the place an environment variable names, else at `fallback` (relative to the
/// repository), else on the PATH, run with `probe` to see that it runs.
pub fn program(var: &str, fallback: &str, name: &str, probe: &[&str]) -> Option<String> {
    if let Ok(p) = std::env::var(var)
        && !p.is_empty()
    {
        return Some(p);
    }
    if !fallback.is_empty() && Path::new(fallback).exists() {
        return Some(std::fs::canonicalize(fallback).unwrap().to_string_lossy().to_string());
    }
    let runs = Command::new(name).args(probe).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
    if runs { Some(name.to_string()) } else { None }
}

pub fn lint_imports() -> Option<String> {
    program("SAKAI_LINT_IMPORTS", "tools/.venv/bin/lint-imports", "lint-imports", &["--help"])
}

pub fn depcruise() -> Option<String> {
    program("SAKAI_DEPCRUISE", "tools/node_modules/.bin/depcruise", "depcruise", &["--version"])
}

/// `java` or `javac`: `SAKAI_JAVA` or `SAKAI_JAVAC`, else under `JAVA_HOME`, else on the PATH.
pub fn java(name: &str) -> Option<String> {
    let var = if name == "javac" { "SAKAI_JAVAC" } else { "SAKAI_JAVA" };
    if let Ok(p) = std::env::var(var)
        && !p.is_empty()
    {
        return Some(p);
    }
    if let Ok(h) = std::env::var("JAVA_HOME") {
        let p = Path::new(&h).join("bin").join(name);
        if p.exists() {
            return Some(p.to_string_lossy().to_string());
        }
    }
    let runs = Command::new(name).arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
    if runs { Some(name.to_string()) } else { None }
}

/// A directory of jars: the environment variable's, else `fallback`, when `jar` is in it.
pub fn jars(var: &str, fallback: &str, jar: &str) -> Option<PathBuf> {
    let d = match std::env::var(var) {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => PathBuf::from(fallback),
    };
    if d.join(jar).exists() { Some(std::fs::canonicalize(d).unwrap()) } else { None }
}

pub fn archunit_lib() -> Option<PathBuf> {
    jars("SAKAI_ARCHUNIT_LIB", "tools/java/lib", "archunit-1.5.1.jar")
}

pub fn cml_lib() -> Option<PathBuf> {
    jars("SAKAI_CML_LIB", "tools/cml/context-mapper-cli-6.12.0/lib", "context-mapper-cli-6.12.0.jar")
}

pub fn go() -> Option<String> {
    program("SAKAI_GO", "", "go", &["version"])
}

pub fn go_arch_lint() -> Option<String> {
    program("SAKAI_GO_ARCH_LINT", "tools/go/bin/go-arch-lint", "go-arch-lint", &["version"])
}

/// A tool of the suite (rulec, koyomi, chobo, dandori): `SAKAI_<NAME>`, else the PATH.
pub fn suite(name: &str) -> Option<String> {
    program(&format!("SAKAI_{}", name.to_uppercase()), "", name, &["--version"])
}

/// The codes of the diagnostics a run printed, in order: `error[E501]: …` gives `E501`.
pub fn printed_codes(out: &str) -> Vec<String> {
    out.lines()
        .filter_map(|l| {
            let rest = ["error[", "warning[", "note[", "エラー[", "警告[", "備考["].iter().find_map(|p| l.strip_prefix(p))?;
            Some(rest.split(']').next()?.to_string())
        })
        .collect()
}

/// The command of a mutant, when it has one (its file `command`): the arguments after `sakai`.
pub fn mutant_command(name: &str) -> Option<Vec<String>> {
    let s = std::fs::read_to_string(Path::new("tests/mutants").join(name).join("command")).ok()?;
    Some(s.split_whitespace().map(String::from).collect())
}
