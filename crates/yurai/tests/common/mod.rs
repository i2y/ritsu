//! What the tests share: a directory that is removed when the test is done, running the
//! `yurai` binary, and copying a fixture to change it (PLAN 0.1).

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A directory under the system's temporary directory, removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(what: &str) -> TempDir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("yurai-{what}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        // Resolve symbolic links (macOS's /var), so paths compare with what yurai sees.
        TempDir(std::fs::canonicalize(&p).unwrap())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Write a file under the directory, making its directories.
    pub fn write(&self, rel: &str, bytes: &[u8]) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, bytes).unwrap();
        p
    }

    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.0.join(rel)).unwrap()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Copy a directory tree.
pub fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let t = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &t);
        } else {
            std::fs::copy(e.path(), t).unwrap();
        }
    }
}

/// A copy of `tests/fixtures/<name>` in a fresh directory, as `<tmp>/<name>`.
pub fn fixture(name: &str) -> TempDir {
    let t = TempDir::new(name);
    copy_dir(&Path::new("tests/fixtures").join(name), &t.0.join(name));
    t
}

/// What the binary printed and how it ended.
pub struct Ran {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Run `yurai` in `dir` with `args`, no language from the environment.
pub fn yurai(dir: &Path, args: &[&str]) -> Ran {
    let o = Command::new(env!("CARGO_BIN_EXE_yurai")).current_dir(dir).args(args).env_remove("YURAI_LANG").output().unwrap();
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

/// Compare `text` with the golden file, or write it when `YURAI_BLESS` is set.
pub fn golden(path: &str, text: &str, failures: &mut Vec<String>) {
    if std::env::var("YURAI_BLESS").is_ok() {
        std::fs::create_dir_all(Path::new(path).parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
        return;
    }
    let want = std::fs::read_to_string(path).unwrap_or_default();
    if want != text {
        failures.push(format!("{path} differs:\n--- want\n{want}--- got\n{text}"));
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
    let pin = yurai::sha256::short(&std::fs::read(d.join(COPY_142)).unwrap());
    edit(d, "民法の期間.req", "第142条 sha256:fc8c35a0769d3b35", &format!("第142条 sha256:{pin}"));
}

pub fn change_text(d: &Path) {
    edit(d, "民法の期間.req", "応当する日の前日の終わりに満了する", "応当する日の前の日の終わりに満了する");
}

pub fn change_cal(d: &Path) {
    edit(d, "民法の期間.cal", "  if closed + 1 day                        # 末日が休みなら、その翌日", "  roll following                           # 末日が休みなら、休みが明けるまで");
}


// ── The outside tools the tests of stage C use (PLAN 0.1, C.10–C.12) ──

/// Whether a program is on the PATH.
pub fn on_path(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
}

/// Run `yurai` in `dir` with `args` and these environment variables.
pub fn yurai_env(dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> Ran {
    let mut c = Command::new(env!("CARGO_BIN_EXE_yurai"));
    c.current_dir(dir).args(args).env_remove("YURAI_LANG");
    for (k, v) in envs {
        c.env(k, v);
    }
    let o = c.output().unwrap();
    Ran { code: o.status.code().unwrap_or(-1), stdout: String::from_utf8_lossy(&o.stdout).to_string(), stderr: String::from_utf8_lossy(&o.stderr).to_string() }
}

/// The Python that has `prov` and `reqif` (tools/requirements.txt): `YURAI_PYTHON`, else
/// `tools/.venv/bin/python`. None, with a `SKIP:` line, when neither imports both.
pub fn python(what: &str) -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = match std::env::var_os("YURAI_PYTHON") {
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
    println!("SKIP: no Python with prov and reqif (YURAI_PYTHON, or uv venv --python 3.13 tools/.venv and tools/requirements.txt); {what} is not run");
    None
}

/// xmllint and the ReqIF schema with its catalog (tools/reqif/fetch.sh): `YURAI_XMLLINT`
/// (else xmllint on the PATH) and `YURAI_REQIF_XSD` (else tools/reqif/xsd). None, with a
/// `SKIP:` line, when either is missing.
pub fn xmllint_and_schema(what: &str) -> Option<(PathBuf, PathBuf)> {
    let lint = match std::env::var_os("YURAI_XMLLINT") {
        Some(p) => PathBuf::from(p),
        None if on_path("xmllint") => PathBuf::from("xmllint"),
        None => {
            println!("SKIP: xmllint is not on the PATH (or YURAI_XMLLINT); {what} is not run");
            return None;
        }
    };
    let xsd = std::env::var_os("YURAI_REQIF_XSD").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("tools/reqif/xsd"));
    if !xsd.join("catalog.xml").is_file() || !xsd.join("www.omg.org/spec/ReqIF/20110401/reqif.xsd").is_file() {
        println!("SKIP: the ReqIF schema is not in {} (tools/reqif/fetch.sh fetches it, or set YURAI_REQIF_XSD); {what} is not run", xsd.display());
        return None;
    }
    Some((lint, std::fs::canonicalize(&xsd).unwrap()))
}
