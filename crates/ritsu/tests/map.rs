//! ritsu's own map (DESIGN 3.4, 7.13): `ritsu check ritsu.ctx` holds every crate's dependencies,
//! as Cargo says them, to the contexts of `ritsu.ctx` and `contexts/`; and a language's crate
//! that depends on another language's crate is named by sakai at the line of its Cargo.toml.

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The root of the repository.
fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `ritsu`, run in `dir`, with no language asked of the environment.
fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args);
    for v in ["RITSU_LANG", "SAKAI_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

#[test]
fn ritsu_keeps_to_its_own_map() {
    let (code, out, err) = ritsu_in(&repo(), &["check", "ritsu.ctx"]);
    assert_eq!(code, 0, "{out}{err}");
    let first = out.lines().next().unwrap_or("");
    assert!(first.starts_with("ritsu.ctx: ok — ") && first.contains(" crossings checked (rust "), "{out}");
    assert!(out.ends_with("ritsu check: 1 file (sakai 1): all pass; borders between the languages: 0 checked, 0 undecided\n"), "{out}");
}

/// The workspace's manifests, the map and its contexts, laid out in `t`: a crate's sources are an
/// empty `src/lib.rs` or `src/main.rs`, as many as Cargo needs to read it, and what the map
/// leaves out of its scope is an empty directory.
fn lay_out_the_workspace(t: &TempDir) {
    let root = repo();
    for f in ["Cargo.toml", "ritsu.ctx"] {
        t.write(f, std::fs::read(root.join(f)).unwrap());
    }
    for e in std::fs::read_dir(root.join("contexts")).unwrap() {
        let p = e.unwrap().path();
        t.write(&format!("contexts/{}", p.file_name().unwrap().to_string_lossy()), std::fs::read(&p).unwrap());
    }
    for e in std::fs::read_dir(root.join("crates")).unwrap() {
        let dir = e.unwrap().path();
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        if !dir.join("Cargo.toml").is_file() {
            continue;
        }
        t.write(&format!("crates/{name}/Cargo.toml"), std::fs::read(dir.join("Cargo.toml")).unwrap());
        for (f, body) in [("lib.rs", ""), ("main.rs", "fn main() {}\n")] {
            if dir.join("src").join(f).is_file() {
                t.write(&format!("crates/{name}/src/{f}"), body);
            }
        }
    }
    let map = std::fs::read_to_string(root.join("ritsu.ctx")).unwrap();
    let except = map.lines().find(|l| l.starts_with("except ")).expect("the map has an except line");
    for d in except.split('"').skip(1).step_by(2) {
        std::fs::create_dir_all(t.path().join(d)).unwrap();
    }
}

/// `text` with `line` put right after its line `after`, and the number of the line it went on.
fn put_after(text: &str, after: &str, line: &str) -> (String, usize) {
    let at = text.lines().position(|l| l == after).unwrap_or_else(|| panic!("no line {after}"));
    let mut lines: Vec<&str> = text.lines().collect();
    lines.insert(at + 1, line);
    (lines.join("\n") + "\n", at + 2)
}

#[test]
fn a_language_that_depends_on_another_is_named_at_its_line() {
    let t = TempDir::new("self-map");
    lay_out_the_workspace(&t);
    // as laid out, the map passes
    let (code, out, err) = ritsu_in(t.path(), &["check", "ritsu.ctx", "--root", "."]);
    assert_eq!(code, 0, "{out}{err}");
    // koyomi's crate depends on rulec's: a language on another language
    let koyomi = t.read("crates/koyomi/Cargo.toml");
    let dep = "rulec = { path = \"../rulec\" }";
    let (mutant, line) = put_after(&koyomi, "[dependencies]", dep);
    t.write("crates/koyomi/Cargo.toml", &mutant);
    let (code, out, _) = ritsu_in(t.path(), &["check", "ritsu.ctx", "--root", "."]);
    assert_eq!(code, 1, "{out}");
    let want = format!(
        "error[sakai E201]: crates/koyomi/Cargo.toml:{line}:1: The file crates/koyomi/Cargo.toml of Calendars depends on crates/rulec of Rules (dependencies), which Calendars has no relationship with\n{line:>6} | {dep}\n"
    );
    assert!(out.starts_with(&want), "{out}");
    assert!(out.ends_with("ritsu check: 1 file (sakai 1): 1 fail (1 error); borders between the languages: 0 checked, 0 undecided\n"), "{out}");
    let (_, out, _) = ritsu_in(t.path(), &["check", "ritsu.ctx", "--root", ".", "--lang", "ja"]);
    let want = format!("エラー[sakai E201]: crates/koyomi/Cargo.toml:{line}:1: 「Calendars」の crates/koyomi/Cargo.toml が、関係の無い「Rules」の crates/rulec に依存しています（dependencies）\n");
    assert!(out.starts_with(&want), "{out}");
    // the same crate for the tests only is no dependency of the crate (sakai's DESIGN 3.7)
    let (tests_only, _) = put_after(&koyomi, "[dev-dependencies]", dep);
    t.write("crates/koyomi/Cargo.toml", &tests_only);
    let (code, out, err) = ritsu_in(t.path(), &["check", "ritsu.ctx", "--root", "."]);
    assert_eq!(code, 0, "{out}{err}");
    // the base depending on a language goes against the map too
    t.write("crates/koyomi/Cargo.toml", &koyomi);
    let units = t.read("crates/ritsu-units/Cargo.toml");
    let (mutant, line) = put_after(&units, "[dependencies]", "chobo = { path = \"../chobo\" }");
    t.write("crates/ritsu-units/Cargo.toml", &mutant);
    let (code, out, _) = ritsu_in(t.path(), &["check", "ritsu.ctx", "--root", "."]);
    assert_eq!(code, 1, "{out}");
    assert!(out.starts_with(&format!("error[sakai E201]: crates/ritsu-units/Cargo.toml:{line}:1: The file crates/ritsu-units/Cargo.toml of Base depends on crates/chobo of Books (dependencies), which Base has no relationship with\n")), "{out}");
    assert!(out.contains("Books is downstream of Base, the other way round"), "{out}");
}
