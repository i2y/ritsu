//! What the tests share: running `sakai`, laying out a fixture or a mutant in a temporary
//! directory, and the outside tools (PLAN 0.1). A directory removed when the test is done (and
//! those of test processes that have ended), copying a tree, running a program with a time limit
//! (macOS has no `timeout`), golden files, finding a tool and the SKIP line are ritsu-testkit's.

#![allow(dead_code, unused_imports)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// A directory under the system's temporary directory, removed on drop.
pub use ritsu_testkit::TempDir;
/// What a program printed and how it ended; running it to its end, killed after a limit.
pub use ritsu_testkit::run::{Ran, run};
/// Copy a directory, every file under it.
pub use ritsu_testkit::tmp::copy_dir;

/// `sakai` with these arguments, in `dir`, with no language from the environment.
pub fn sakai_in(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_sakai")).args(args).current_dir(dir).env_remove("SAKAI_LANG").env_remove("RITSU_LANG").output().unwrap()
}

pub fn sakai(args: &[&str]) -> std::process::Output {
    sakai_in(Path::new("."), args)
}

/// Hold `got` to the golden file `path`; `SAKAI_BLESS=1` (or `RITSU_BLESS=1`) writes it instead
/// (ritsu-testkit's). The failure, if any, is returned for the caller to collect.
pub fn golden(path: &str, got: &str) -> Option<String> {
    ritsu_testkit::golden::check(Path::new(path), got).err()
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
    let dir = TempDir::new(name);
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
    let dir = TempDir::new(base);
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
pub fn text(os: &[sakai::check::Outcome], lang: ritsu_base::text::Lang) -> String {
    os.iter().map(|o| sakai::check::render(o, lang)).collect()
}

/// Says that a test did not run what it is for, and why: the line `SKIP: sakai: <why>`
/// (ritsu-testkit's).
pub fn skip(why: &str) {
    ritsu_testkit::skip(why);
}

/// The example of the repository.
pub const EXAMPLE: &str = "examples/通販";

/// A program named by `RITSU_<TOOL>` or `SAKAI_<TOOL>`, else at `fallback` (relative to the
/// repository), else on the PATH when `name probe…` runs (ritsu-testkit's).
pub fn program(tool: &str, fallback: &str, name: &str, probe: &[&str]) -> Option<String> {
    let fallback = (!fallback.is_empty()).then(|| Path::new(fallback));
    ritsu_testkit::tools::find(tool, fallback, name, probe).map(|p| p.to_string_lossy().to_string())
}

/// The linters that hold generated code to a map (import-linter, dependency-cruiser, ArchUnit,
/// go-arch-lint, the Context Mapper CLI) run at the `tools` level.
pub fn linters() -> bool {
    ritsu_testkit::need(ritsu_testkit::Need::Linters)
}

pub fn lint_imports() -> Option<String> {
    program("LINT_IMPORTS", "tools/.venv/bin/lint-imports", "lint-imports", &["--help"])
}

pub fn depcruise() -> Option<String> {
    program("DEPCRUISE", "tools/node_modules/.bin/depcruise", "depcruise", &["--version"])
}

/// `java` or `javac`: `RITSU_JAVA` or `SAKAI_JAVA` (`…_JAVAC`), else under `JAVA_HOME`, else on
/// the PATH.
pub fn java(name: &str) -> Option<String> {
    if let Some(p) = ritsu_testkit::tools::from_vars(&name.to_uppercase()) {
        return Some(p);
    }
    if let Ok(h) = std::env::var("JAVA_HOME") {
        let p = Path::new(&h).join("bin").join(name);
        if p.exists() {
            return Some(p.to_string_lossy().to_string());
        }
    }
    if ritsu_testkit::tools::runs(name, &["-version"]) { Some(name.to_string()) } else { None }
}

/// A directory of jars: `RITSU_<TOOL>` or `SAKAI_<TOOL>`, else `fallback`, when `jar` is in it.
pub fn jars(tool: &str, fallback: &str, jar: &str) -> Option<PathBuf> {
    let d = PathBuf::from(ritsu_testkit::tools::from_vars(tool).unwrap_or_else(|| fallback.to_string()));
    if d.join(jar).exists() { Some(std::fs::canonicalize(d).unwrap()) } else { None }
}

pub fn archunit_lib() -> Option<PathBuf> {
    jars("ARCHUNIT_LIB", "tools/java/lib", "archunit-1.5.1.jar")
}

pub fn cml_lib() -> Option<PathBuf> {
    jars("CML_LIB", "tools/cml/context-mapper-cli-6.12.0/lib", "context-mapper-cli-6.12.0.jar")
}

pub fn go() -> Option<String> {
    program("GO", "", "go", &["version"])
}

pub fn go_arch_lint() -> Option<String> {
    program("GO_ARCH_LINT", "tools/go/bin/go-arch-lint", "go-arch-lint", &["version"])
}

/// A tool of the suite (rulec, koyomi, chobo): `RITSU_<NAME>` or `SAKAI_<NAME>`, else
/// the PATH.
pub fn suite(name: &str) -> Option<String> {
    program(&name.to_uppercase(), "", name, &["--version"])
}

/// `ritsu`, which runs dandori with the rules a workflow uses read in the same process: `RITSU_RITSU`
/// or `SAKAI_RITSU`, else the workspace's build of it, else the PATH.
pub fn ritsu() -> Option<String> {
    program("RITSU", "../../target/debug/ritsu", "ritsu", &["--version"])
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
