//! The stories README.md and README.ja.md tell, run as they tell them. Each runs in
//! a scratch directory holding the examples it needs, the greeter's in a git
//! repository of its own, as a project would hold it. Its transcript, the `$ `
//! lines a person would type and what each command printed, is a golden under
//! `tests/golden/{en,ja}/readme/`, and `tests/docs.rs` holds every console block of
//! the READMEs, the examples' READMEs and the skill to these transcripts, so what
//! the pages show is what geas prints. A step the test takes itself, such as
//! writing an agent's change into a file, has no `$ ` line; the pages say it in
//! words.

mod common;
use common::*;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A terminal session in a scratch directory, written down as the terminal shows it.
struct Session {
    s: Scratch,
    /// The temporary directory of the Chrome a story's geas starts.
    chrome: ChromeTmp,
    text: String,
    log: PathBuf,
}

impl Session {
    fn new(name: &str) -> Session {
        let s = Scratch::new(name);
        let log = pid_log(&s);
        Session { s, chrome: ChromeTmp::new(), text: String::new(), log }
    }

    fn path(&self) -> &Path {
        self.s.path()
    }

    /// Runs the geas command the line holds (its words split on blanks; the pages'
    /// commands have no quotes), with `input` on its stdin, and writes the line and
    /// what it printed, stdout first, as a terminal shows a run that prints its
    /// warnings last.
    fn run(&mut self, line: &str, input: Option<&[u8]>) -> i32 {
        let command = line.rsplit("| ").next().expect("a command");
        let words: Vec<&str> = command.split_whitespace().collect();
        assert_eq!(words[0], "geas", "{line}");
        let log = self.log.to_string_lossy().into_owned();
        let mut env = vec![("GEAS_PID_LOG", log.as_str())];
        env.extend(self.chrome.vars());
        let (out, err, code) = run_with_input(self.s.path(), &words[1..], &env, input);
        self.text.push_str(&format!("$ {line}\n{out}{err}"));
        code
    }

    fn geas(&mut self, line: &str) -> i32 {
        self.run(line, None)
    }

    /// `$ echo $?`, after the command whose exit status the page shows.
    fn status(&mut self, code: i32) {
        self.text.push_str(&format!("$ echo $?\n{code}\n"));
    }

    /// A `$ ` line for a command the test ran itself.
    fn typed(&mut self, line: &str) {
        self.text.push_str(&format!("$ {line}\n"));
    }

    /// Keeps the transcript so far as a golden, and starts the next.
    fn keep(&mut self, name: &str) {
        golden(name, &std::mem::take(&mut self.text));
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            no_process_left(&self.log);
        }
    }
}

/// Runs git in the scratch repository, apart from the developer's own settings.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .stdin(Stdio::null())
        .output()
        .expect("start git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).expect("git prints UTF-8")
}

fn git_at_hand() -> bool {
    if have("git", &["--version"]) {
        true
    } else {
        skip("git is not on PATH; the greeter's story in the READMEs is not run");
        false
    }
}

/// The greeter's first server.py, as an agent might write it: no check for an
/// empty name.
const THE_CHECK: &str = "            if not name:\n                self._send(400, \"name required\")\n                return\n";

/// The story of the greeter: a claim catching a missing check, drift on a refactor,
/// and the claims an agent's change touches. `spec` is the claims file, `flag` what
/// the commands add for the language.
fn greeter_story(lang: &str, spec: &str, flag: &str) {
    if !python3("the greeter's story in the READMEs") || !git_at_hand() {
        return;
    }
    let mut t = Session::new(&format!("readme-greeter-{lang}"));
    copy_example("greeter", &t.s);
    let dir = t.path().to_path_buf();
    git(&dir, &["init", "-q"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["-c", "user.name=geas", "-c", "user.email=geas@example.invalid", "commit", "-q", "-m", "the greeter"]);
    let spec = format!("examples/greeter/{spec}");
    let server = t.s.read("examples/greeter/server.py");
    assert!(server.contains(THE_CHECK), "server.py no longer has the empty-name check");

    // the loop: the first version fails a claim, the fix holds it
    t.s.write("examples/greeter/server.py", &server.replace(THE_CHECK, ""));
    assert_eq!(t.geas(&format!("geas check {spec}{flag}")), 1);
    t.status(1);
    t.s.write("examples/greeter/server.py", &server);
    assert_eq!(t.geas(&format!("geas check {spec}{flag}")), 0);
    t.status(0);
    t.keep(&format!("{lang}/readme/loop.txt"));

    // drift: an agent's refactor keeps every claim and changes seven things
    assert_eq!(t.geas(&format!("geas snap {spec}{flag}")), 0);
    let refactored = t.s.read("examples/greeter/server_refactored.py");
    t.typed("cp examples/greeter/server_refactored.py examples/greeter/server.py");
    t.s.write("examples/greeter/server.py", &refactored);
    assert_eq!(t.geas(&format!("geas check {spec}{flag}")), 0);
    assert_eq!(t.geas(&format!("geas drift {spec}{flag}")), 1);
    t.status(1);
    t.keep(&format!("{lang}/readme/drift.txt"));

    // the diff: an agent's change to the first server.py, recorded and read
    let change = repo_file("tests/changes/greeter/after/server.py");
    t.s.write("examples/greeter/server.py", &change);
    assert_eq!(t.geas(&format!("geas map {spec}{flag}")), 0);
    let diff = git(&dir, &["diff"]);
    assert!(diff.contains("+        elif u.path == \"/health\":"), "{diff}");
    assert_eq!(t.run(&format!("git diff | geas affected {spec} -{flag}"), Some(diff.as_bytes())), 1);
    t.status(1);
    t.keep(&format!("{lang}/readme/affected.txt"));
}

#[test]
fn the_greeter_in_english() {
    greeter_story("en", "greeter.geas", "");
}

#[test]
fn the_greeter_in_japanese() {
    greeter_story("ja", "greeter.ja.geas", " --lang ja");
}

/// The greeter's page in Chrome: the claims hold, and then an agent turns the
/// Greet button into a `<div>` with a click handler.
fn web_story(lang: &str, flag: &str) {
    if !python3("the web greeter's story in the READMEs") || chrome("the web greeter's story in the READMEs").is_none() {
        return;
    }
    let mut t = Session::new(&format!("readme-web-{lang}"));
    copy_example("web-greeter", &t.s);
    let spec = "examples/web-greeter/web-greeter.geas";
    assert_eq!(t.geas(&format!("geas check {spec}{flag}")), 0);
    // with every pin, drift on the unchanged page is quiet
    assert_eq!(t.geas(&format!("geas snap {spec}{flag}")), 0);
    assert_eq!(t.geas(&format!("geas drift {spec}{flag}")), 0);
    t.s.write("examples/web-greeter/index.html", &repo_file("tests/changes/web-greeter/after/index.html"));
    assert_eq!(t.geas(&format!("geas check {spec}{flag}")), 1);
    t.status(1);
    t.keep(&format!("{lang}/readme/web.txt"));
    no_chrome_left(&t.s);
}

#[test]
fn the_web_greeter_in_english() {
    web_story("en", "");
}

#[test]
fn the_web_greeter_in_japanese() {
    web_story("ja", " --lang ja");
}

/// pixie's greeter, when a built one is at hand (`GEAS_PIXIE_GREETER`).
fn pixie_story(lang: &str, flag: &str) {
    if !ritsu_testkit::need(ritsu_testkit::Need::Pixie) {
        return;
    }
    let app = match std::env::var_os("GEAS_PIXIE_GREETER").map(PathBuf::from) {
        Some(p) if p.is_file() => p,
        _ => {
            skip("GEAS_PIXIE_GREETER does not name a built pixie greeter; the pixie greeter's story in the READMEs is not run");
            return;
        }
    };
    let mut t = Session::new(&format!("readme-pixie-{lang}"));
    copy_example("pixie-greeter", &t.s);
    std::os::unix::fs::symlink(&app, t.path().join("examples/pixie-greeter/greeter")).expect("link the greeter");
    assert_eq!(t.geas(&format!("geas check examples/pixie-greeter/greeter.geas{flag}")), 0);
    t.keep(&format!("{lang}/readme/pixie.txt"));
}

#[test]
fn the_pixie_greeter_in_english() {
    pixie_story("en", "");
}

#[test]
fn the_pixie_greeter_in_japanese() {
    pixie_story("ja", " --lang ja");
}

/// The help, and the skill installed into a project.
fn commands_story(lang: &str, flag: &str) {
    let mut t = Session::new(&format!("readme-commands-{lang}"));
    assert_eq!(t.geas(&format!("geas --help{flag}")), 0);
    t.keep(&format!("{lang}/readme/help.txt"));
    assert_eq!(t.geas(&format!("geas skill --install .claude/skills{flag}")), 0);
    assert_eq!(t.geas(&format!("geas skill --install .claude/skills{flag}")), 2);
    t.status(2);
    assert_eq!(t.geas(&format!("geas skill --install .claude/skills --force{flag}")), 0);
    t.keep(&format!("{lang}/readme/skill.txt"));
    assert_eq!(t.s.read(".claude/skills/geas/SKILL.md"), repo_file("../../skills/geas/SKILL.md"));
}

#[test]
fn the_commands_in_english() {
    commands_story("en", "");
}

#[test]
fn the_commands_in_japanese() {
    commands_story("ja", " --lang ja");
}
