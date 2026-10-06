//! What the tests share: the command run in this process with every language joined (as
//! `ritsu yuen` runs it), the `yuen` binary of this crate (which joins none), copying a fixture to
//! change it (PLAN 0.1), and the outside tools. A directory that is removed when the test is done,
//! copying a tree, golden files, finding a tool and the SKIP line are ritsu-testkit's.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

/// Every language yuen reads, joined through the ports, each language's own engine (ritsu's
/// DESIGN 3.3): what `ritsu yuen` hands it, the things of every file in one index.
pub fn suite() -> yuen::suite::Suite {
    suite_where(None)
}

/// [`suite`], with what the files of one language hold answered by `port` instead of the
/// language's engine.
pub fn suite_where(instead: Option<(ritsu_base::naming::Tool, Rc<dyn ritsu_ports::Items>)>) -> yuen::suite::Suite {
    use ritsu_base::naming::Tool;
    let rules = Rc::new(rulec::ports::Engine::new());
    let dates = Rc::new(koyomi::ports::Engine);
    let claims = Rc::new(geas::ports::Engine);
    let mut index = ritsu_ports::Index::new()
        .with_items(Tool::Rulec, rules.clone())
        .with_items(Tool::Koyomi, dates.clone())
        .with_items(Tool::Chobo, Rc::new(chobo::ports::Engine))
        .with_items(Tool::Geas, claims.clone())
        .with_items(Tool::Dandori, Rc::new(dandori::ports::Engine))
        .with_items(Tool::Sekisho, Rc::new(sekisho::ports::Engine::default()))
        .with_items(Tool::Sakai, Rc::new(sakai::ports::Engine));
    if let Some((tool, port)) = instead {
        index = index.with_items(tool, port);
    }
    let mut s = yuen::suite::Suite::default();
    s.index = Rc::new(index);
    s.sources.insert("rulec".into(), rules.clone());
    s.sources.insert("koyomi".into(), dates.clone());
    s.rules = Some(rules);
    s.dates = Some(dates);
    s.claims = Some(claims);
    s
}

/// The command, in this process, with every language joined (`ritsu yuen`), run where the tests
/// run (the crate's directory): paths are written from there, or absolute. English unless the
/// arguments say otherwise, whatever the environment says.
pub fn run(args: &[&str]) -> Ran {
    let mut args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    if !args.iter().any(|a| a == "--lang" || a.starts_with("--lang=")) {
        args.extend(["--lang".to_string(), "en".to_string()]);
    }
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = yuen::run::run(&args, suite(), &mut out, &mut err);
    Ran { code: code as i32, stdout: String::from_utf8_lossy(&out).to_string(), stderr: String::from_utf8_lossy(&err).to_string() }
}

/// [`yuen::check::check`], with every language joined.
pub fn check(path: &str) -> yuen::check::Checked {
    yuen::check::check_with(&[path.to_string()], Some(path), suite()).unwrap()
}

/// A directory under the system's temporary directory, removed on drop, its path through its
/// symbolic links (macOS's /var), so paths compare with what yuen sees.
pub use ritsu_testkit::TempDir;
/// Copy a directory tree.
pub use ritsu_testkit::tmp::copy_dir;

/// A copy of `tests/fixtures/<name>` in a fresh directory, as `<tmp>/<name>`.
pub fn fixture(name: &str) -> TempDir {
    let t = TempDir::new(name);
    copy_dir(&Path::new("tests/fixtures").join(name), &t.path().join(name));
    t
}

/// What the binary printed and how it ended.
pub struct Ran {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Run `yuen` in `dir` with `args`, no language from the environment.
pub fn yuen(dir: &Path, args: &[&str]) -> Ran {
    let o = Command::new(env!("CARGO_BIN_EXE_yuen")).current_dir(dir).args(args).env_remove("YUEN_LANG").env_remove("RITSU_LANG").output().unwrap();
    Ran { code: o.status.code().unwrap_or(-1), stdout: String::from_utf8_lossy(&o.stdout).to_string(), stderr: String::from_utf8_lossy(&o.stderr).to_string() }
}

/// The codes in what `check` printed, in order.
pub fn codes(out: &str) -> Vec<String> {
    out.lines()
        .filter_map(|l| {
            let l = l.strip_prefix("error[").or_else(|| l.strip_prefix("warning[")).or_else(|| l.strip_prefix("エラー[")).or_else(|| l.strip_prefix("警告["))?;
            Some(l[..4].to_string())
        })
        .collect()
}

/// Compare `text` with the golden file, or write it when `YUEN_BLESS` (or `RITSU_BLESS`) is set
/// (ritsu-testkit's).
pub fn golden(path: &str, text: &str, failures: &mut Vec<String>) {
    if let Err(e) = ritsu_testkit::golden::check(Path::new(path), text) {
        failures.push(e);
    }
}

// ── The changes PLAN B.7 makes to a copy of the `period` fixture ──

pub fn edit(d: &Path, rel: &str, from: &str, to: &str) {
    let p = d.join(rel);
    let s = std::fs::read_to_string(&p).unwrap();
    assert!(s.contains(from), "{rel} has no {from:?}");
    std::fs::write(&p, s.replacen(from, to, 1)).unwrap();
}

const COPY_142: &str = "sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml";

/// The 142nd article changed by one character, and its pin written again.
pub fn change_142(d: &Path) {
    edit(d, COPY_142, "その翌日に満了する", "その翌々日に満了する");
    let pin = ritsu_base::sha256::short(&std::fs::read(d.join(COPY_142)).unwrap());
    edit(d, "民法の期間.req", "第142条 sha256:fc8c35a0769d3b35", &format!("第142条 sha256:{pin}"));
}

pub fn change_text(d: &Path) {
    edit(d, "民法の期間.req", "応当する日の前日の終わりに満了する", "応当する日の前の日の終わりに満了する");
}

pub fn change_cal(d: &Path) {
    edit(d, "民法の期間.cal", "  if closed + 1 day                        # 末日が休みなら、その翌日", "  roll following                           # 末日が休みなら、休みが明けるまで");
}

// ── The twins of the changes above, for the fixture `period_of_months` ──

const COPY_1_7: &str = "sources/law/37-CFR-1@2026-01-01/1.7.xml";

/// Section 1.7 changed by a word, and its pin written again (the twin of `change_142`).
pub fn change_section(d: &Path) {
    edit(d, COPY_1_7, "on the next succeeding business day which is not a Saturday", "on the second succeeding business day which is not a Saturday");
    let pin = ritsu_base::sha256::short(&std::fs::read(d.join(COPY_1_7)).unwrap());
    edit(d, "period_of_months.req", "\"§1.7\" sha256:01de176ebe4740d7", &format!("\"§1.7\" sha256:{pin}"));
}

/// The text of `timely_filing` changed by a word (the twin of `change_text`).
pub fn change_text_en(d: &Path) {
    edit(d, "period_of_months.req", "before the period ends,", "before the period has ended,");
}

/// One line of the `.cal` (the twin of `change_cal`).
pub fn change_cal_en(d: &Path) {
    edit(d, "period_of_months.cal", "  if closed + 1 day                              # an end on a closed day moves to the day after", "  roll following                                 # an end on a closed day moves on until the closure ends");
}


// ── The outside tools the tests of stage C use (PLAN 0.1, C.10–C.12) ──

/// Whether a program is on the PATH.
pub fn on_path(name: &str) -> bool {
    ritsu_testkit::tools::on_path(name).is_some()
}

/// Run `yuen` in `dir` with `args` and these environment variables.
pub fn yuen_env(dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> Ran {
    let mut c = Command::new(env!("CARGO_BIN_EXE_yuen"));
    c.current_dir(dir).args(args).env_remove("YUEN_LANG").env_remove("RITSU_LANG");
    for (k, v) in envs {
        c.env(k, v);
    }
    let o = c.output().unwrap();
    Ran { code: o.status.code().unwrap_or(-1), stdout: String::from_utf8_lossy(&o.stdout).to_string(), stderr: String::from_utf8_lossy(&o.stderr).to_string() }
}

/// The Python that has `prov` and `reqif` (tools/requirements.txt): `YUEN_PYTHON`, else
/// `tools/.venv/bin/python`. None, with a `SKIP:` line, when neither imports both.
pub fn python(what: &str) -> Option<PathBuf> {
    if !ritsu_testkit::need(ritsu_testkit::Need::Python) {
        return None;
    }
    let candidates: Vec<PathBuf> = match std::env::var_os("YUEN_PYTHON") {
        Some(p) => vec![PathBuf::from(p)],
        None => vec![PathBuf::from("tools/.venv/bin/python")],
    };
    for p in candidates {
        let ok = Command::new(&p).args(["-c", "import prov, reqif"]).output().is_ok_and(|o| o.status.success());
        if ok {
            // Absolute, but not through its symbolic link: a venv's python is a link to the
            // interpreter it was made from, and only the link knows the venv.
            return Some(if p.is_absolute() { p } else { std::env::current_dir().unwrap().join(p) });
        }
    }
    ritsu_testkit::skip(&format!("no Python with prov and reqif (YUEN_PYTHON, or uv venv --python 3.13 tools/.venv and tools/requirements.txt); {what} is not run"));
    None
}

/// xmllint and the ReqIF schema with its catalog (tools/reqif/fetch.sh): `YUEN_XMLLINT`
/// (else xmllint on the PATH) and `YUEN_REQIF_XSD` (else tools/reqif/xsd). None, with a
/// `SKIP:` line, when either is missing.
pub fn xmllint_and_schema(what: &str) -> Option<(PathBuf, PathBuf)> {
    if !ritsu_testkit::need(ritsu_testkit::Need::Xmllint) {
        return None;
    }
    let lint = match std::env::var_os("YUEN_XMLLINT") {
        Some(p) => PathBuf::from(p),
        None if on_path("xmllint") => PathBuf::from("xmllint"),
        None => {
            ritsu_testkit::skip(&format!("xmllint is not on the PATH (or YUEN_XMLLINT); {what} is not run"));
            return None;
        }
    };
    let xsd = std::env::var_os("YUEN_REQIF_XSD").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("tools/reqif/xsd"));
    if !xsd.join("catalog.xml").is_file() || !xsd.join("www.omg.org/spec/ReqIF/20110401/reqif.xsd").is_file() {
        ritsu_testkit::skip(&format!("the ReqIF schema is not in {} (tools/reqif/fetch.sh fetches it, or set YUEN_REQIF_XSD); {what} is not run", xsd.display()));
        return None;
    }
    Some((lint, std::fs::canonicalize(&xsd).unwrap()))
}

/// Each example and its projects (one `.req` each: the English one first, its Japanese version
/// beside it), with the role that looks at its artifacts.
pub const EXAMPLES: &[(&str, &[&str], &str)] = &[
    ("osha", &["osha.req"], "development"),
    ("greeter", &["greeter.req", "greeter.ja.req"], "development"),
    ("payment_terms", &["payment_terms.req", "payment_terms.ja.req"], "development"),
    ("refunds", &["refunds.req", "refunds.ja.req"], "development"),
    ("refund_contracts", &["refund_contracts.req", "refund_contracts.ja.req"], "development"),
    ("civil_code_periods", &["civil_code_periods.ja.req"], "開発"),
    ("civil_code_periods_reread", &["civil_code_periods_reread.ja.req"], "開発"),
    ("stamp_tax", &["stamp_tax.ja.req"], "開発"),
    ("openspec_greeter", &["greeter.req", "greeter.ja.req"], "development"),
    ("openspec_greeter_archived", &["greeter.req", "greeter.ja.req"], "development"),
];
