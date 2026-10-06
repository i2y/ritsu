//! `cargo xtask`: the work of developing ritsu (DESIGN 3.4, 10.2–10.4, PLAN C.3).
//!
//! ```text
//! cargo xtask test [--level fast|tools|platforms] [--changed <rev>] [-p <crate>]... [-- <cargo test args>...]
//! cargo xtask deps
//! ```
//!
//! `test` chooses the crates — those given, those a change since `<rev>` touched and every crate
//! that depends on them, or the whole workspace — runs their tests with `RITSU_TEST_LEVEL` and
//! `RITSU_SKIP_LOG`, and prints the SKIP lines as a table. With a level, a SKIP for something
//! missing from the machine that `ci/skips/<level>.txt` does not allow fails the run: what did not
//! run must not look as if it ran. A SKIP the level itself asked for (a test of a higher level) is
//! what the level means, and is not held to the list. `deps` holds every crate's dependencies to
//! the layers of DESIGN 3.1, and names each one that breaks them.

use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("test") => test(&args[1..]),
        Some("deps") => deps(),
        Some("--help") | Some("-h") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        None => {
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
        Some(other) => {
            eprintln!("xtask: there is no command `{other}`\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

const USAGE: &str = "cargo xtask test [--level fast|tools|platforms] [--changed <rev>] [-p <crate>]... [-- <cargo test args>...]
    run the tests of the crates given, of those a change since <rev> touched and the crates
    that depend on them, or of the whole workspace; print the SKIP lines as a table, and with a
    level, fail on a SKIP that ci/skips/<level>.txt does not allow
cargo xtask deps
    hold every crate's dependencies to the layers of DESIGN 3.1
";

// ── The workspace ───────────────────────────────────────────────────────────

/// One crate of the workspace: its name, its directory from the root, and what it depends on.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Krate {
    name: String,
    dir: String,
    deps: Vec<Dep>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Dep {
    name: String,
    /// `normal`, `dev` or `build`.
    kind: String,
    /// Whether it comes from outside the workspace (a registry or git).
    external: bool,
}

/// The crates of the workspace, from `cargo metadata --no-deps`.
fn krates(meta: &Value) -> Vec<Krate> {
    let root = meta["workspace_root"].as_str().unwrap_or("");
    let members: BTreeSet<&str> = meta["workspace_members"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let mut out = Vec::new();
    for p in meta["packages"].as_array().into_iter().flatten() {
        if !members.contains(p["id"].as_str().unwrap_or("")) {
            continue;
        }
        let manifest = Path::new(p["manifest_path"].as_str().unwrap_or(""));
        let dir = manifest.parent().and_then(|d| d.strip_prefix(root).ok()).map(|d| d.to_string_lossy().replace('\\', "/")).unwrap_or_default();
        let deps = p["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|d| Dep { name: d["name"].as_str().unwrap_or("").to_string(), kind: d["kind"].as_str().unwrap_or("normal").to_string(), external: d["path"].is_null() })
            .collect();
        out.push(Krate { name: p["name"].as_str().unwrap_or("").to_string(), dir, deps });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn metadata() -> Result<Value, String> {
    let out = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["metadata", "--format-version", "1", "--no-deps", "--offline"])
        .output()
        .map_err(|e| format!("cannot run cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata did not print JSON: {e}"))
}

// ── deps ────────────────────────────────────────────────────────────────────

/// The layers of DESIGN 3.1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layer {
    Base,
    Language,
    Glue,
    Entry,
    Test,
    Tool,
}

const BASE: &[&str] = &["ritsu-base", "ritsu-units", "ritsu-ports", "ritsu-proto", "ritsu-emit"];
const LANGUAGES: &[&str] = &["rulec", "dandori", "koyomi", "chobo", "geas", "yuen", "sakai", "sekisho"];
/// The languages that read and write JSON with serde_json; rulec, geas and sekisho depend on nothing.
const WITH_SERDE: &[&str] = &["dandori", "koyomi", "chobo", "yuen", "sakai"];

fn layer(name: &str) -> Option<Layer> {
    if BASE.contains(&name) {
        Some(Layer::Base)
    } else if LANGUAGES.contains(&name) {
        Some(Layer::Language)
    } else {
        match name {
            "ritsu-project" | "ritsu-cross" => Some(Layer::Glue),
            "ritsu" | "ritsu-wasm" => Some(Layer::Entry),
            "ritsu-testkit" | "ritsu-model" => Some(Layer::Test),
            "xtask" => Some(Layer::Tool),
            _ => None,
        }
    }
}

/// What a crate of the workspace may depend on outside its dev-dependencies (DESIGN 3.1): Ok,
/// or the rule it breaks. Both crates are placed in a layer.
fn allowed(from: &str, to: &str) -> Result<(), String> {
    let (Some(lf), Some(lt)) = (layer(from), layer(to)) else { return Err("a crate DESIGN 3.1 does not place in a layer".into()) };
    let only = |list: &[&str]| -> Result<(), String> {
        if list.contains(&to) { Ok(()) } else { Err(format!("{from} may depend on {} only", if list.is_empty() { "std".to_string() } else { list.join(", ") })) }
    };
    match lf {
        Layer::Base => match from {
            "ritsu-base" => only(&[]),
            "ritsu-ports" => only(&["ritsu-base", "ritsu-units"]),
            _ => only(&["ritsu-base"]),
        },
        Layer::Language => match lt {
            Layer::Base => Ok(()),
            Layer::Language => Err("a language does not depend on another language (rule 1; a dev-dependency may)".into()),
            _ => Err("a language depends on the base layer only (rule 3)".into()),
        },
        Layer::Glue => match (from, lt) {
            (_, Layer::Base) => Ok(()),
            ("ritsu-project", Layer::Language) => Ok(()),
            ("ritsu-cross", Layer::Language) => Err("ritsu-cross reads a language through the ports only".into()),
            ("ritsu-cross", Layer::Glue) if to == "ritsu-project" => Ok(()),
            ("ritsu-project", _) => Err("ritsu-project depends on the base layer and the languages only".into()),
            _ => Err("ritsu-cross depends on the base layer and ritsu-project only".into()),
        },
        Layer::Entry => match lt {
            Layer::Test | Layer::Tool => Err(format!("{to} is not a dependency of an entry")),
            _ => Ok(()),
        },
        Layer::Test => Err(format!("{from} depends on std only")),
        Layer::Tool => Err("xtask depends on std and serde_json only".into()),
    }
}

/// Every dependency that breaks the rules of DESIGN 3.1, named.
fn check_deps(ks: &[Krate]) -> Vec<String> {
    let mut out = Vec::new();
    for k in ks {
        let Some(lk) = layer(&k.name) else {
            out.push(format!("{}: a crate DESIGN 3.1 does not place in a layer; add it there and to xtask", k.name));
            continue;
        };
        for d in &k.deps {
            let what = format!("{} -> {} ({})", k.name, d.name, d.kind);
            if d.external {
                // ritsu-model's tests read what the languages answer as JSON, as the languages' own do
                let serde_ok = d.name == "serde_json"
                    && match lk {
                        Layer::Language => WITH_SERDE.contains(&k.name.as_str()),
                        Layer::Glue | Layer::Entry | Layer::Tool => true,
                        Layer::Test => k.name == "ritsu-model" && d.kind == "dev",
                        Layer::Base => false,
                    };
                if !serde_ok {
                    out.push(format!("{what}: an outside crate; the only one is serde_json, for the five languages that use it, the layers above them and xtask (rule 4)"));
                }
                continue;
            }
            if layer(&d.name).is_none() {
                out.push(format!("{what}: a crate DESIGN 3.1 does not place in a layer"));
                continue;
            }
            if d.name == "ritsu-testkit" {
                if d.kind != "dev" {
                    out.push(format!("{what}: ritsu-testkit is a dev-dependency only"));
                }
                continue;
            }
            if d.name == "ritsu-model" {
                // the comparison with the Lean models is a crate of tests that nothing builds on (DESIGN 11.3)
                out.push(format!("{what}: nothing depends on ritsu-model"));
                continue;
            }
            if d.kind == "dev" {
                // A test may take a crate that does not depend on it, which makes no loop (DESIGN
                // 3.3): the receiving language's tests run the providing language's ports.
                continue;
            }
            if let Err(rule) = allowed(&k.name, &d.name) {
                out.push(format!("{what}: {rule}"));
            }
        }
    }
    out
}

fn deps() -> ExitCode {
    let meta = match metadata() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("xtask: {e}");
            return ExitCode::from(2);
        }
    };
    let ks = krates(&meta);
    let broken = check_deps(&ks);
    if broken.is_empty() {
        println!("deps: ok — {} crates, every dependency within the layers of DESIGN 3.1", ks.len());
        ExitCode::SUCCESS
    } else {
        for b in &broken {
            println!("{b}");
        }
        println!("deps: {} dependencies break the layers of DESIGN 3.1", broken.len());
        ExitCode::from(1)
    }
}

// ── test ────────────────────────────────────────────────────────────────────

/// What `test` was asked for.
#[derive(Debug, Default, PartialEq, Eq)]
struct TestArgs {
    level: Option<String>,
    changed: Option<String>,
    crates: Vec<String>,
    rest: Vec<String>,
}

fn parse_test(args: &[String]) -> Result<TestArgs, String> {
    let mut t = TestArgs::default();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--" {
            t.rest = args[i + 1..].to_vec();
            break;
        }
        let value = args.get(i + 1).cloned();
        match a {
            "--level" => {
                let v = value.ok_or("--level is missing its value")?;
                if !matches!(v.as_str(), "fast" | "tools" | "platforms") {
                    return Err(format!("--level {v} is not a level; it is fast, tools or platforms"));
                }
                if t.level.replace(v).is_some() {
                    return Err("--level is given twice".into());
                }
            }
            "--changed" => {
                let v = value.ok_or("--changed is missing its value")?;
                if t.changed.replace(v).is_some() {
                    return Err("--changed is given twice".into());
                }
            }
            "-p" | "--package" => t.crates.push(value.ok_or("-p is missing its value")?),
            other => return Err(format!("unknown flag `{other}`")),
        }
        i += 2;
    }
    Ok(t)
}

/// The crates the changed files are in (a file outside every crate is the workspace's own: every
/// crate), and every crate that depends on one of them, by any kind of dependency.
fn affected(ks: &[Krate], files: &[String]) -> BTreeSet<String> {
    let mut hit: BTreeSet<String> = BTreeSet::new();
    for f in files {
        match ks.iter().filter(|k| f.starts_with(&format!("{}/", k.dir))).max_by_key(|k| k.dir.len()) {
            Some(k) => {
                hit.insert(k.name.clone());
            }
            None => return ks.iter().map(|k| k.name.clone()).collect(),
        }
    }
    loop {
        let more: Vec<String> = ks.iter().filter(|k| !hit.contains(&k.name) && k.deps.iter().any(|d| !d.external && hit.contains(&d.name))).map(|k| k.name.clone()).collect();
        if more.is_empty() {
            return hit;
        }
        hit.extend(more);
    }
}

/// The files that differ from `rev`, and the files git does not track yet, from the root.
fn changed_files(root: &Path, rev: &str) -> Result<Vec<String>, String> {
    let git = |args: &[&str]| -> Result<Vec<String>, String> {
        let out = Command::new("git").arg("-C").arg(root).args(args).output().map_err(|e| format!("cannot run git: {e}"))?;
        if !out.status.success() {
            return Err(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
        }
        Ok(String::from_utf8_lossy(&out.stdout).lines().map(String::from).filter(|l| !l.is_empty()).collect())
    };
    let mut files = git(&["-c", "core.quotepath=off", "diff", "--name-only", rev])?;
    files.extend(git(&["-c", "core.quotepath=off", "ls-files", "--others", "--exclude-standard"])?);
    files.sort();
    files.dedup();
    Ok(files)
}

/// One allowed SKIP: a crate and a test of it, `*` for every test of it.
#[derive(Debug, PartialEq, Eq)]
struct Allowed {
    krate: String,
    test: String,
}

/// `ci/skips/<level>.txt`: a `<crate> <test>` a line (`<crate>` alone, or `<crate> *`, for every
/// test of it); `#` starts a comment. A file that is not there allows nothing.
fn read_allowed(text: &str) -> Vec<Allowed> {
    text.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            Some(Allowed { krate: it.next()?.to_string(), test: it.next().unwrap_or("*").to_string() })
        })
        .collect()
}

/// One line of a SKIP log (`ritsu-testkit`'s): the crate, the test, why (`level` or `missing`),
/// the reason, between tabs.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Skip {
    krate: String,
    test: String,
    level: bool,
    reason: String,
}

/// The lines of a SKIP log, each once; a line not of that form is passed over.
fn read_skips(text: &str) -> Vec<Skip> {
    let mut v: Vec<Skip> = text
        .lines()
        .filter_map(|l| {
            let mut it = l.splitn(4, '\t');
            let (krate, test, why, reason) = (it.next()?, it.next()?, it.next()?, it.next()?);
            let level = match why {
                "level" => true,
                "missing" => false,
                _ => return None,
            };
            Some(Skip { krate: krate.to_string(), test: test.to_string(), level, reason: reason.to_string() })
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The SKIPs as a table, the columns lined up, and how many the level does not allow: a SKIP the
/// level asked for is allowed; one for something missing is allowed when the list has it.
fn skip_table(skips: &[Skip], allowed: Option<&[Allowed]>) -> (String, usize) {
    let width = |s: &str| s.chars().map(|c| if (c as u32) >= 0x1100 { 2 } else { 1 }).sum::<usize>();
    let (wc, wt) = skips.iter().fold((5, 4), |(a, b), s| (a.max(width(&s.krate)), b.max(width(&s.test))));
    let pad = |s: &str, w: usize| format!("{s}{}", " ".repeat(w.saturating_sub(width(s))));
    let mut o = format!("{}  {}  reason\n", pad("crate", wc), pad("test", wt));
    let mut bad = 0;
    for s in skips {
        let ok = s.level || allowed.is_none_or(|a| a.iter().any(|x| x.krate == s.krate && (x.test == "*" || x.test == s.test)));
        if !ok {
            bad += 1;
        }
        o.push_str(&format!("{}  {}  {}{}\n", pad(&s.krate, wc), pad(&s.test, wt), s.reason, if ok { "" } else { "  (not allowed)" }));
    }
    (o, bad)
}

/// The root of the workspace: xtask's own manifest is at `crates/xtask`.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap_or_else(|_| PathBuf::from("."))
}

fn test(args: &[String]) -> ExitCode {
    let t = match parse_test(args) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("xtask test: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let root = workspace_root();
    let meta = match metadata() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("xtask: {e}");
            return ExitCode::from(2);
        }
    };
    let ks = krates(&meta);
    let mut chosen: BTreeSet<String> = t.crates.iter().cloned().collect();
    if let Some(c) = chosen.iter().find(|c| !ks.iter().any(|k| &k.name == *c)) {
        eprintln!("xtask test: there is no crate `{c}` in the workspace");
        return ExitCode::from(2);
    }
    if let Some(rev) = &t.changed {
        match changed_files(&root, rev) {
            Ok(files) => {
                let a = affected(&ks, &files);
                println!("xtask: {} files changed since {rev}: {}", files.len(), if a.is_empty() { "no crate".to_string() } else { a.iter().cloned().collect::<Vec<_>>().join(", ") });
                if a.is_empty() && chosen.is_empty() {
                    return ExitCode::SUCCESS;
                }
                chosen.extend(a);
            }
            Err(e) => {
                eprintln!("xtask test: {e}");
                return ExitCode::from(2);
            }
        }
    }
    let log = std::env::temp_dir().join(format!("ritsu-xtask-skips-{}.tsv", std::process::id()));
    let _ = std::fs::remove_file(&log);
    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.current_dir(&root).args(["test", "--no-fail-fast"]);
    let which = if chosen.is_empty() {
        cmd.arg("--workspace");
        "--workspace".to_string()
    } else {
        for c in &chosen {
            cmd.args(["-p", c]);
        }
        chosen.iter().map(|c| format!("-p {c}")).collect::<Vec<_>>().join(" ")
    };
    cmd.arg("--");
    if !t.rest.iter().any(|a| a == "--nocapture") {
        cmd.arg("--nocapture");
    }
    cmd.args(&t.rest).env("RITSU_SKIP_LOG", &log);
    match &t.level {
        Some(l) => cmd.env("RITSU_TEST_LEVEL", l),
        None => cmd.env_remove("RITSU_TEST_LEVEL"),
    };
    println!("xtask: cargo test {which} (RITSU_TEST_LEVEL {})", t.level.as_deref().unwrap_or("not set"));
    let status = cmd.status();
    let skips = read_skips(&std::fs::read_to_string(&log).unwrap_or_default());
    let _ = std::fs::remove_file(&log);
    let allowed = t.level.as_ref().map(|l| read_allowed(&std::fs::read_to_string(root.join("ci/skips").join(format!("{l}.txt"))).unwrap_or_default()));
    let (table, bad) = skip_table(&skips, allowed.as_deref());
    println!("\nxtask: {} SKIP lines from the crates that log them (ritsu-testkit's skip)", skips.len());
    if !skips.is_empty() {
        print!("{table}");
    }
    if !status.as_ref().is_ok_and(|s| s.success()) {
        println!("xtask: the tests failed");
        return ExitCode::from(1);
    }
    if bad > 0 {
        println!("xtask: {bad} SKIP lines are not in ci/skips/{}.txt", t.level.as_deref().unwrap_or(""));
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(name: &str, dir: &str, deps: &[(&str, &str, bool)]) -> Krate {
        Krate { name: name.into(), dir: dir.into(), deps: deps.iter().map(|(n, kind, ext)| Dep { name: n.to_string(), kind: kind.to_string(), external: *ext }).collect() }
    }

    fn workspace() -> Vec<Krate> {
        vec![
            k("ritsu-base", "crates/ritsu-base", &[("ritsu-testkit", "dev", false)]),
            k("ritsu-testkit", "crates/ritsu-testkit", &[]),
            k("rulec", "crates/rulec", &[]),
            k("koyomi", "crates/koyomi", &[("serde_json", "normal", true), ("ritsu-base", "normal", false)]),
            k("dandori", "crates/dandori", &[("serde_json", "normal", true), ("rulec", "dev", false)]),
            k("xtask", "crates/xtask", &[("serde_json", "normal", true)]),
        ]
    }

    /// The crate of the comparison with the Lean models, as it should be: the languages and
    /// serde_json as dev-dependencies only.
    fn model() -> Krate {
        k("ritsu-model", "crates/ritsu-model", &[("koyomi", "dev", false), ("ritsu-testkit", "dev", false), ("serde_json", "dev", true)])
    }

    #[test]
    fn the_workspace_as_it_should_be_passes() {
        assert_eq!(check_deps(&workspace()), Vec::<String>::new());
        let mut ws = workspace();
        ws.push(model());
        assert_eq!(check_deps(&ws), Vec::<String>::new(), "ritsu-model takes the languages as dev-dependencies");
    }

    #[test]
    fn each_rule_names_what_breaks_it() {
        let mut ws = workspace();
        ws.push(k("chobo", "crates/chobo", &[("koyomi", "normal", false)]));
        ws.push(k("geas", "crates/geas", &[("serde_json", "normal", true)]));
        ws.push(k("ritsu-units", "crates/ritsu-units", &[("rulec", "normal", false)]));
        ws.push(k("sakai", "crates/sakai", &[("ritsu-testkit", "normal", false), ("regex", "normal", true)]));
        ws.push(k("ritsu-cross", "crates/ritsu-cross", &[("rulec", "normal", false), ("ritsu-project", "normal", false)]));
        ws.push(k("ritsu-project", "crates/ritsu-project", &[("ritsu", "normal", false)]));
        ws.push(k("ritsu-base-extra", "crates/x", &[]));
        let mut m = model();
        m.deps.push(Dep { name: "rulec".into(), kind: "normal".into(), external: false });
        ws.push(m);
        ws.iter_mut().find(|c| c.name == "chobo").unwrap().deps.push(Dep { name: "ritsu-model".into(), kind: "dev".into(), external: false });
        let broken = check_deps(&ws);
        let want = [
            "chobo -> koyomi (normal): a language does not depend on another language (rule 1; a dev-dependency may)",
            "geas -> serde_json (normal): an outside crate; the only one is serde_json, for the five languages that use it, the layers above them and xtask (rule 4)",
            "ritsu-units -> rulec (normal): ritsu-units may depend on ritsu-base only",
            "sakai -> ritsu-testkit (normal): ritsu-testkit is a dev-dependency only",
            "sakai -> regex (normal): an outside crate; the only one is serde_json, for the five languages that use it, the layers above them and xtask (rule 4)",
            "ritsu-cross -> rulec (normal): ritsu-cross reads a language through the ports only",
            "ritsu-project -> ritsu (normal): ritsu-project depends on the base layer and the languages only",
            "ritsu-base-extra: a crate DESIGN 3.1 does not place in a layer; add it there and to xtask",
            "ritsu-model -> rulec (normal): ritsu-model depends on std only",
            "chobo -> ritsu-model (dev): nothing depends on ritsu-model",
        ];
        for w in want {
            assert!(broken.iter().any(|b| b == w), "missing: {w}\nall: {broken:#?}");
        }
        assert_eq!(broken.len(), want.len(), "{broken:#?}");
    }

    #[test]
    fn what_changed_and_what_depends_on_it() {
        let ws = workspace();
        let a = affected(&ws, &["crates/rulec/src/main.rs".into()]);
        assert_eq!(a.into_iter().collect::<Vec<_>>(), ["dandori", "rulec"], "dandori's tests take rulec");
        let a = affected(&ws, &["crates/ritsu-base/src/lib.rs".into()]);
        assert_eq!(a.into_iter().collect::<Vec<_>>(), ["koyomi", "ritsu-base"]);
        let a = affected(&ws, &["crates/ritsu-testkit/src/lib.rs".into()]);
        assert_eq!(a.into_iter().collect::<Vec<_>>(), ["koyomi", "ritsu-base", "ritsu-testkit"]);
        assert_eq!(affected(&ws, &["Cargo.lock".into()]).len(), ws.len(), "a file of the workspace's own is every crate");
        assert!(affected(&ws, &[]).is_empty());
    }

    #[test]
    fn the_flags_of_test() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(
            parse_test(&s(&["--level", "tools", "-p", "rulec", "-p", "koyomi", "--", "--skip", "temporal"])).unwrap(),
            TestArgs { level: Some("tools".into()), changed: None, crates: s(&["rulec", "koyomi"]), rest: s(&["--skip", "temporal"]) }
        );
        assert_eq!(parse_test(&s(&["--changed", "HEAD~1"])).unwrap().changed.as_deref(), Some("HEAD~1"));
        assert!(parse_test(&s(&["--level", "everything"])).is_err());
        assert!(parse_test(&s(&["--level", "fast", "--level", "tools"])).is_err());
        assert!(parse_test(&s(&["--changed"])).is_err());
        assert!(parse_test(&s(&["--frobnicate"])).is_err());
    }

    #[test]
    fn the_skips_a_level_allows() {
        let allowed = read_allowed("# what fast leaves to the tools level\nrulec\ndandori   examples   # the platforms\n\n");
        assert_eq!(allowed, [Allowed { krate: "rulec".into(), test: "*".into() }, Allowed { krate: "dandori".into(), test: "examples".into() }]);
        let skips = read_skips(
            "dandori\texamples\tmissing\tno Temporal\nkoyomi\tdoc\tmissing\tChrome is not found\nrulec\tbackends\tmissing\tno swiftc\nrulec\tbackends\tmissing\tno swiftc\nchobo\tdoc\tlevel\tneeds chrome (the tools level); RITSU_TEST_LEVEL is fast\nbroken line\nx\ty\tother\tz\n",
        );
        assert_eq!(skips.len(), 4, "a line twice is one, a line not of the form is passed over");
        let (table, bad) = skip_table(&skips, Some(&allowed));
        assert_eq!(bad, 1, "a SKIP the level asked for is allowed");
        assert_eq!(
            table,
            "crate    test      reason\nchobo    doc       needs chrome (the tools level); RITSU_TEST_LEVEL is fast\ndandori  examples  no Temporal\nkoyomi   doc       Chrome is not found  (not allowed)\nrulec    backends  no swiftc\n"
        );
        assert_eq!(skip_table(&skips, None).1, 0, "without a level nothing is held to a list");
    }

    #[test]
    fn the_crates_from_the_metadata() {
        let meta: Value = serde_json::from_str(
            r#"{"workspace_root":"/w","workspace_members":["path+file:///w/crates/a#0.1.0"],"packages":[
                {"id":"path+file:///w/crates/a#0.1.0","name":"a","manifest_path":"/w/crates/a/Cargo.toml","dependencies":[
                    {"name":"serde_json","kind":null,"path":null,"source":"registry+https://github.com/rust-lang/crates.io-index"},
                    {"name":"b","kind":"dev","path":"/w/crates/b","source":null}]},
                {"id":"registry+x#serde_json@1","name":"serde_json","manifest_path":"/r/serde_json/Cargo.toml","dependencies":[]}]}"#,
        )
        .unwrap();
        assert_eq!(
            krates(&meta),
            [Krate { name: "a".into(), dir: "crates/a".into(), deps: vec![Dep { name: "serde_json".into(), kind: "normal".into(), external: true }, Dep { name: "b".into(), kind: "dev".into(), external: false }] }]
        );
    }
}
