//! `--extra wasm` (ringi's DESIGN 12.6): the project's package of Rust, the crate `--target rust`
//! writes, built for wasm32-unknown-unknown with `ritsu_emit::wasm::CARGO_LINE` into one module,
//! `<out>/requests/kinds.wasm`, which a server loads to run the kinds of request with no code of
//! their own built into it; and `<out>/requests/wasm.json`, how the server calls it. The module
//! imports nothing and exports `call` (kaname's DESIGN 36.10–36.12); a kind's entry is a line of JSON
//! to the crate's door, as the list of the kinds of request of Rust writes it (`ticket_kinds.rs`).
//!
//! - The crate is built where nothing else is: in a directory of its own under the system's
//!   temporary directory, removed after, with cargo's home and target there too and no `CARGO_*`,
//!   `RUSTFLAGS` or wrapper of rustc from the environment, so that what one machine's settings say
//!   does not change the bytes. Its manifest leaves out the dependencies it has for every target
//!   but wasm32 (the drivers the clients of the books wait on), which cargo would otherwise look
//!   for in its registry: the module is built with nothing to fetch (`--offline`). The paths the
//!   module holds are the crate's own, from its root (`glue/ticket.rs`), and the standard library's,
//!   which its toolchain names (`/rustc/<commit>/library/…`): the same crate and the same toolchain
//!   give the same bytes wherever they are built.
//! - Without cargo, or without the standard library of wasm32-unknown-unknown, nothing is written,
//!   and the run says how to get them.
//! - `--check` builds nothing. `wasm.json` records the digest of the crate as it was built (its
//!   files, and the line that built it) and that of the module: the module is what the run writes
//!   when both are those of the crate the run makes and of the module on the disk. A toolchain is
//!   no part of what is checked, as ritsu's version is no part of a lock (DESIGN 17.4): the same
//!   check passes in CI with no toolchain for wasm32, and `wasm.json` says which rustc built the
//!   module.

use super::{Refusal, refusal};
use ritsu_base::json::Json;
use ritsu_base::text::Lang;
use ritsu_base::tr;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::sync::atomic::{AtomicI32, Ordering as AtomicOrdering};

/// The module, in `<out>/requests/`.
pub const MODULE: &str = "kinds.wasm";

/// How a server calls it, beside it.
pub const ABOUT: &str = "wasm.json";

/// The version of the shape of `wasm.json`: a reader refuses one it does not know.
pub const SHAPE: u32 = 1;

/// The target the module is built for.
const TARGET: &str = "wasm32-unknown-unknown";

/// A kind of request as a host calls it through the door: its name and version, the name its
/// module declares (what the door knows it by), and whether its form works something out (its
/// glue's `worked_out`; else `worked_none`, whose answer is no field).
pub struct Door {
    pub name: String,
    pub version: u32,
    pub module: String,
    pub works: bool,
}

/// What `--extra wasm` comes to: the module, and `wasm.json`. Under `--check`, the module is the
/// one on the disk when it is what the run writes, and None when it is not (missing, or not the
/// build of the crate the run makes).
pub struct Made {
    pub module: Option<Vec<u8>>,
    pub about: String,
}

/// The module and `wasm.json` of the crate `files` (the package of Rust, each file by its path under
/// the crate) named `name`, for the kinds `kinds`, with the contract's version and the language of
/// the entries' errors (`lang`, `en` or `ja`). Under `check`, nothing is built: `dir` is
/// `<out>/requests/`, whose `wasm.json` and module are read. `said` is the language of a refusal.
pub fn make(files: &[(String, String, Option<String>)], name: &str, kinds: &[Door], contract: u32, lang: &str, check: bool, dir: &Path, said: Lang) -> Result<Made, Refusal> {
    let built = for_wasm(files);
    let source = source_digest(&built);
    if check {
        let module = std::fs::read(dir.join(MODULE)).ok();
        let recorded = std::fs::read_to_string(dir.join(ABOUT)).ok().and_then(|t| ritsu_base::json::parse(&t).ok());
        let field = |k: &str| recorded.as_ref().and_then(|j| j.get(k)).and_then(Json::as_str).map(str::to_string);
        let built_field = |k: &str| recorded.as_ref().and_then(|j| j.get("built")).and_then(|b| b.get(k)).and_then(Json::as_str).map(str::to_string);
        let sha = module.as_deref().map(ritsu_base::sha256::hex).or_else(|| field("sha256")).unwrap_or_default();
        let fresh = module.is_some() && field("sha256").as_deref() == Some(sha.as_str()) && built_field("source").as_deref() == Some(source.as_str());
        let rustc = built_field("rustc").unwrap_or_default();
        return Ok(Made { module: module.filter(|_| fresh), about: about(kinds, contract, lang, &sha, &source, &rustc) });
    }
    let (module, rustc) = build(&built, name, said)?;
    let sha = ritsu_base::sha256::hex(&module);
    Ok(Made { about: about(kinds, contract, lang, &sha, &source, &rustc), module: Some(module) })
}

/// The crate as the module is built from it: its files, with a manifest that leaves out what it
/// depends on for every target but wasm32 (`[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`,
/// the drivers of the books' clients, which the module leaves out with the clients), by their paths,
/// in the order of their bytes.
fn for_wasm(files: &[(String, String, Option<String>)]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = files.iter().map(|(p, text, _)| (p.clone(), if p == "Cargo.toml" { own_workspace(&without_native(text)) } else { text.clone() })).collect();
    out.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    out
}

/// The manifest of the temporary crate, made one workspace of its own so that no `Cargo.toml`
/// above the scratch directory can take it into a workspace.
fn own_workspace(toml: &str) -> String {
    format!("{toml}\n[workspace]\n")
}

/// A manifest without the table of what the crate depends on for every target but wasm32: the
/// table's header and its lines, up to the next table or the end.
fn without_native(toml: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in toml.split_inclusive('\n') {
        if line.trim_end() == super::rust::NOT_WASM32 {
            skipping = true;
            // the blank line before the table goes with it
            if out.ends_with("\n\n") {
                out.pop();
            }
            continue;
        }
        if skipping && line.starts_with('[') {
            skipping = false;
        }
        if !skipping {
            out.push_str(line);
        }
    }
    out
}

/// The digest of the crate as it is built: the line that builds it, then each file's path and
/// bytes, each written after its length in eight bytes, big-endian; in hexadecimal.
fn source_digest(files: &[(String, String)]) -> String {
    let mut b: Vec<u8> = Vec::new();
    let mut put = |bytes: &[u8]| {
        b.extend((bytes.len() as u64).to_be_bytes());
        b.extend(bytes);
    };
    put(ritsu_emit::wasm::CARGO_LINE.as_bytes());
    for (p, text) in files {
        put(p.as_bytes());
        put(text.as_bytes());
    }
    ritsu_base::sha256::hex(&b)
}

/// `wasm.json`: the head, the shape's version, the contract's, the language of the entries' errors,
/// the module's file and digest, the names of its exports, each kind with the line its entries are
/// called with, and how the module was built (the digest of the crate, rustc's version, the line).
fn about(kinds: &[Door], contract: u32, lang: &str, sha: &str, source: &str, rustc: &str) -> String {
    let entry = |function: &str, takes: &[&str], now: bool| {
        Json::obj([("function", Json::str(function)), ("in", Json::arr(takes.iter().map(|t| Json::str(*t)))), ("now", Json::Bool(now))])
    };
    let kind = |k: &Door| {
        let worked = if k.works {
            entry(super::kinds::WORKED_OUT, &["input"], false)
        } else {
            let mut pairs = match entry(super::kinds::WORKED_NONE, &["input"], false) {
                Json::Obj(p) => p,
                _ => unreachable!("an entry is an object"),
            };
            pairs.push(("answer".to_string(), Json::obj([("fields", Json::obj(Vec::<(String, Json)>::new()))])));
            Json::Obj(pairs)
        };
        Json::obj([
            ("name", Json::str(&k.name)),
            ("version", Json::int(i128::from(k.version))),
            ("module", Json::str(&k.module)),
            ("start", entry("start", &["submit", "organization"], true)),
            ("next", entry("next", &["ticket", "event", "organization"], true)),
            ("worked", worked),
        ])
    };
    let j = Json::obj([
        ("//", Json::str(ritsu_emit::header::generated("ritsu"))),
        ("wasm", Json::int(i128::from(SHAPE))),
        ("contract", Json::int(i128::from(contract))),
        ("lang", Json::str(lang)),
        ("file", Json::str(MODULE)),
        ("sha256", Json::str(sha)),
        ("exports", Json::obj([("call", Json::str("call")), ("realloc", Json::str("cabi_realloc")), ("post_return", Json::str("cabi_post_call")), ("memory", Json::str("memory"))])),
        ("kinds", Json::arr(kinds.iter().map(kind))),
        ("built", Json::obj([("source", Json::str(source)), ("rustc", Json::str(rustc)), ("cargo", Json::str(ritsu_emit::wasm::CARGO_LINE))])),
    ]);
    format!("{}\n", j.pretty())
}

/// A directory of its own, readable by the user alone, removed with it.
struct Scratch(PathBuf);

impl Scratch {
    /// Err(None) when there is no directory another user cannot change to make it under.
    fn new() -> Result<Scratch, Option<std::io::Error>> {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let base = scratch_base().ok_or(None)?;
        clean_finished(&base);
        loop {
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
            let d = base.join(format!("ritsu-wasm-{}-{}-{stamp}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
            match std::fs::create_dir(&d) {
                Ok(()) => {
                    // removed with the value, also when it cannot be made the user's alone
                    let s = Scratch(d);
                    private(&s.0)?;
                    return Ok(s);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(Some(e)),
            }
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The directory the scratch directory is made under: the system's temporary directory when neither
/// it nor a directory above it can be changed by another user; else the user's runtime directory,
/// else a directory of ritsu's in the user's cache. The build is run from `/`, so a file planted
/// above the scratch directory chooses neither the toolchain nor cargo's settings; this check keeps
/// the crate itself somewhere another user cannot replace. None when no candidate is such a place.
fn scratch_base() -> Option<PathBuf> {
    let mut candidates = vec![std::env::temp_dir()];
    if let Some(d) = std::env::var_os("XDG_RUNTIME_DIR").filter(|x| !x.is_empty()) {
        candidates.push(PathBuf::from(d));
    }
    if let Some(home) = std::env::var_os("HOME").filter(|x| !x.is_empty()) {
        candidates.push(PathBuf::from(home).join(".cache").join("ritsu").join("wasm"));
    }
    // A build or test tree ritsu is run from is a better last resort than failing where the
    // system's temporary directory is shared and the user's cache cannot be written. In an
    // installation this directory is usually not writable, and the loop leaves it out.
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join("ritsu-wasm"));
    }
    for d in candidates {
        if d.starts_with(std::env::temp_dir()) || d.exists() || std::fs::create_dir_all(&d).is_ok() {
            if d.exists() && ancestors_private(&d) {
                return Some(d);
            }
        }
    }
    None
}

/// Whether no directory at or above `d` can be changed by another user: each is owned by root or by
/// this user, and neither its group nor others can write to it.
#[cfg(unix)]
fn ancestors_private(d: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    let me = unsafe { getuid() };
    // the path as it opens, so that a symbolic link cannot make a shared directory look private
    let Ok(real) = std::fs::canonicalize(d) else { return false };
    real.ancestors().all(|x| std::fs::metadata(x).is_ok_and(|m| m.is_dir() && (m.uid() == 0 || m.uid() == me) && m.mode() & 0o022 == 0))
}

#[cfg(not(unix))]
fn ancestors_private(d: &Path) -> bool {
    d.ancestors().all(|x| x.is_dir())
}

#[cfg(unix)]
fn private(d: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn private(_d: &Path) -> std::io::Result<()> {
    Ok(())
}

/// The scratch directories of runs that are no longer alive, best effort. The name holds the
/// process's id after `ritsu-wasm-`.
fn clean_finished(base: &Path) {
    let Ok(rd) = std::fs::read_dir(base) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(rest) = name.strip_prefix("ritsu-wasm-") else { continue };
        let Some(pid) = rest.split('-').next().and_then(|x| x.parse::<i32>().ok()).filter(|p| *p > 0) else { continue };
        if pid != std::process::id() as i32 && !alive(pid) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

#[cfg(unix)]
fn alive(pid: i32) -> bool {
    if unsafe { kill(pid, 0) } == 0 {
        return true;
    }
    // EPERM: there is such a process, but it is not this user's
    std::io::Error::last_os_error().raw_os_error() == Some(1)
}

#[cfg(not(unix))]
fn alive(_pid: i32) -> bool {
    true
}

/// The process group of the tool that runs now, 0 when none does.
#[cfg(unix)]
static CHILD: AtomicI32 = AtomicI32::new(0);

/// The signal the run of the tool that runs now (or ran last) was given, 0 when none was.
#[cfg(unix)]
static SIGNALED: AtomicI32 = AtomicI32::new(0);

#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
    // the handler as C's `sighandler_t`, a number: SIG_DFL is 0, SIG_IGN is 1
    fn signal(signum: i32, handler: usize) -> usize;
}

/// SIGINT and SIGTERM, which a run of a tool hands to the tool.
#[cfg(unix)]
const HANDED: [i32; 2] = [2, 15];

/// The signal a run of a tool was given while the tool ran: note it, and give it to the tool's own
/// process group so that cargo's children stop with it. Only `kill` and the atomics run here.
#[cfg(unix)]
extern "C" fn sighed(n: i32) {
    SIGNALED.store(n, AtomicOrdering::SeqCst);
    let pid = CHILD.load(AtomicOrdering::SeqCst);
    if pid > 0 {
        unsafe { kill(-pid, n) };
    }
}

/// Run a tool with its output read, in a process group of its own on Unix. While it runs, a SIGINT
/// or a SIGTERM of this process is handed to the tool's group (one this process was started
/// ignoring stays ignored), so that the tool stops with its children and the run answers as the
/// stopped tool does; before and after the run, the two signals do what they did.
#[cfg(unix)]
fn watched(c: &mut Command) -> std::io::Result<std::process::Output> {
    use std::os::unix::process::CommandExt;
    const SIG_IGN: usize = 1;
    c.process_group(0);
    SIGNALED.store(0, AtomicOrdering::SeqCst);
    let before = HANDED.map(|n| unsafe { signal(n, sighed as extern "C" fn(i32) as usize) });
    for (n, h) in HANDED.into_iter().zip(before) {
        if h == SIG_IGN {
            unsafe { signal(n, SIG_IGN) };
        }
    }
    let out = c.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().and_then(|child| {
        let pid = child.id() as i32;
        CHILD.store(pid, AtomicOrdering::SeqCst);
        // a signal that came before the tool's id was known
        let n = SIGNALED.load(AtomicOrdering::SeqCst);
        if n != 0 {
            unsafe { kill(-pid, n) };
        }
        let out = child.wait_with_output();
        CHILD.store(0, AtomicOrdering::SeqCst);
        out
    });
    for (n, h) in HANDED.into_iter().zip(before) {
        unsafe { signal(n, h) };
    }
    out
}

#[cfg(not(unix))]
fn watched(c: &mut Command) -> std::io::Result<std::process::Output> {
    c.output()
}

/// Whether the run of the tool that ran last was given a SIGINT or a SIGTERM.
#[cfg(unix)]
fn signaled() -> bool {
    SIGNALED.load(AtomicOrdering::SeqCst) != 0
}

#[cfg(not(unix))]
fn signaled() -> bool {
    false
}

/// The program of a tool: the variable's (`CARGO`, `RUSTC`), as cargo reads them, else its name on
/// the path.
fn tool(var: &str, name: &str) -> std::ffi::OsString {
    std::env::var_os(var).filter(|v| !v.is_empty()).unwrap_or_else(|| name.into())
}

/// A command of a tool, run in `dir` with nothing of cargo's or rustc's settings from the
/// environment but the tools themselves: cargo's home and target directory are the run's own.
fn clean(program: &std::ffi::OsString, dir: &Path, scratch: &Path, toolchain: Option<&str>) -> Command {
    let mut c = Command::new(program);
    c.current_dir(dir);
    for (k, _) in std::env::vars_os() {
        let k = k.to_string_lossy().into_owned();
        if k.starts_with("CARGO_") || ["RUSTFLAGS", "RUSTDOCFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"].contains(&k.as_str()) {
            c.env_remove(&k);
        }
    }
    c.env("CARGO_HOME", scratch.join("cargo-home")).env("CARGO_TARGET_DIR", scratch.join("target"));
    if let Some(t) = toolchain {
        c.env("RUSTUP_TOOLCHAIN", t);
    }
    c
}

/// The words of a line of the shell with no quote but `'…'` ([`ritsu_emit::wasm::CARGO_LINE`]).
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut any = false;
    for c in line.chars() {
        match c {
            '\'' => {
                quoted = !quoted;
                any = true;
            }
            ' ' if !quoted => {
                if any {
                    out.push(std::mem::take(&mut word));
                }
                any = false;
            }
            c => {
                word.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(word);
    }
    out
}

/// How to get what the build needs, said when it is missing.
fn get_rust() -> ritsu_base::text::Text {
    tr!(
        "Rust のツールチェーンを入れてください（https://rustup.rs）。入れたあと `rustup target add {TARGET}` で Wasm の標準ライブラリも入れます",
        "install Rust's toolchain (https://rustup.rs), then the standard library of Wasm with `rustup target add {TARGET}`"
    )
}

/// The toolchain the build names for rustup's shims: the caller's `RUSTUP_TOOLCHAIN` when it is
/// set, else the default rustup answers outside any directory a file of it could name a toolchain
/// from. Nothing changes a cargo or rustc that is no shim.
fn toolchain() -> Option<String> {
    if let Some(t) = std::env::var("RUSTUP_TOOLCHAIN").ok().filter(|x| !x.is_empty()) {
        return Some(t);
    }
    let o = Command::new("rustup").current_dir("/").args(["show", "active-toolchain"]).output().ok()?;
    if !o.status.success() {
        return None;
    }
    String::from_utf8_lossy(&o.stdout).split_whitespace().next().map(str::to_string)
}

/// The crate built for wasm32-unknown-unknown: the module, and what `rustc -V` says of the compiler.
fn build(files: &[(String, String)], name: &str, said: Lang) -> Result<(Vec<u8>, String), Refusal> {
    let (cargo, rustc) = (tool("CARGO", "cargo"), tool("RUSTC", "rustc"));
    let toolchain = toolchain();
    let scratch = match Scratch::new() {
        Ok(s) => s,
        Err(None) => {
            return Err(refusal(
                2,
                tr!(
                    "`--extra wasm` は、ほかの利用者が変えられない一時のディレクトリが見つからないので、何も書きません。自分だけが書けるディレクトリを TMPDIR に指定してください",
                    "`--extra wasm` finds no temporary directory another user cannot change, so nothing is written; set TMPDIR to a directory only you can write to"
                ),
                said,
            ));
        }
        Err(Some(e)) => return Err(refusal(2, ritsu_base::cli::cannot_create(&std::env::temp_dir().display().to_string(), &e), said)),
    };
    let dir = scratch.0.join("crate");
    // a run of a tool this process was signaled in stops the build, whatever the tool answered
    let stopped = || refusal(2, tr!("`--extra wasm` は、組むあいだにシグナルで止められたので、何も書きません。", "`--extra wasm` was stopped by a signal while it built, so nothing is written"), said);
    let ran = |c: &mut Command| {
        let o = watched(c);
        if signaled() { Err(stopped()) } else { Ok(o) }
    };
    let get = get_rust();
    let missing = |what: &str| refusal(2, tr!("`--extra wasm` は、{what} が見つからないので、何も書きません。", "`--extra wasm` cannot find {what}, so nothing is written; ").then(&get), said);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return Err(refusal(2, ritsu_base::cli::cannot_create(&dir.display().to_string(), &e), said));
    }
    match ran(clean(&cargo, Path::new("/"), &scratch.0, toolchain.as_deref()).arg("-V"))? {
        Ok(o) if o.status.success() => {}
        _ => return Err(missing("cargo")),
    }
    let version = match ran(clean(&rustc, Path::new("/"), &scratch.0, toolchain.as_deref()).arg("-V"))? {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => return Err(missing("rustc")),
    };
    // the standard library of the target: rustc names its directory whether it is there or not
    let libdir = match ran(clean(&rustc, Path::new("/"), &scratch.0, toolchain.as_deref()).args(["--print", "target-libdir", "--target", TARGET]))? {
        Ok(o) if o.status.success() => Some(PathBuf::from(String::from_utf8_lossy(&o.stdout).trim())),
        _ => None,
    };
    let has_core = libdir.and_then(|d| std::fs::read_dir(d).ok()).is_some_and(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().starts_with("libcore-")));
    if !has_core {
        return Err(refusal(
            2,
            tr!(
                "`--extra wasm` は、{version} に {TARGET} の標準ライブラリが無いので、何も書きません。`rustup target add {TARGET}` で入れてください",
                "`--extra wasm` finds no standard library of {TARGET} for {version}, so nothing is written; install it with `rustup target add {TARGET}`"
            ),
            said,
        ));
    }
    for (rel, text) in files {
        let p = dir.join(rel);
        if let Some(parent) = p.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            return Err(refusal(2, ritsu_base::cli::cannot_create(&parent.display().to_string(), &e), said));
        }
        if let Err(e) = std::fs::write(&p, text) {
            return Err(refusal(2, ritsu_base::cli::cannot_write(&p.display().to_string(), &e), said));
        }
    }
    let line = words(ritsu_emit::wasm::CARGO_LINE);
    let (program, args) = line.split_first().expect("the line names cargo");
    debug_assert_eq!(program, "cargo");
    let (sub, rest) = args.split_first().expect("the line names cargo's command");
    let manifest = dir.join("Cargo.toml");
    let overrides = [
        "--config",
        "build.rustc-wrapper=\"\"",
        "--config",
        "build.rustc-workspace-wrapper=\"\"",
        "--config",
        "build.rustflags=[]",
        "--config",
        "target.wasm32-unknown-unknown.linker=\"rust-lld\"",
    ];
    let o = match ran(clean(&cargo, Path::new("/"), &scratch.0, toolchain.as_deref()).arg(sub).args(["--offline", "--quiet", "--manifest-path"]).arg(&manifest).args(overrides).args(rest))? {
        Ok(o) => o,
        Err(_) => return Err(missing("cargo")),
    };
    if !o.status.success() {
        let err = String::from_utf8_lossy(&o.stderr);
        let tail: Vec<&str> = err.lines().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect();
        let tail = tail.join("\n");
        return Err(refusal(
            2,
            tr!(
                "Rust のパッケージを {TARGET} に組めませんでした。ritsu の不具合です。報告してください（{version}）\n{tail}",
                "the package of Rust does not build for {TARGET}: a bug in ritsu, please report it ({version})\n{tail}"
            ),
            said,
        ));
    }
    let out = scratch.0.join("target").join(TARGET).join("release").join(format!("{name}.wasm"));
    let module = std::fs::read(&out).map_err(|e| refusal(2, ritsu_base::cli::cannot_read(&out.display().to_string(), &e), said))?;
    if let Err(why) = shape(&module) {
        return Err(refusal(
            2,
            tr!("組んだ Wasm が、約束の形ではありません（{why}）。ritsu の不具合です。報告してください", "the Wasm built is not of the shape it keeps ({why}): a bug in ritsu, please report it"),
            said,
        ));
    }
    Ok((module, version))
}

/// The exports a host calls the module through, and of what kind each is (0 a function, 2 a memory).
pub const EXPORTS: [(&str, u8); 4] = [("call", 0), ("cabi_realloc", 0), ("cabi_post_call", 0), ("memory", 2)];

/// Whether a module is of the shape a server takes: it imports nothing, and exports [`EXPORTS`].
/// Err with what is not.
pub fn shape(b: &[u8]) -> Result<(), String> {
    fn leb(b: &[u8], i: &mut usize) -> Result<u64, String> {
        let (mut n, mut shift) = (0u64, 0u32);
        loop {
            let byte = *b.get(*i).ok_or("the module ends inside a number")?;
            *i += 1;
            n |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(n);
            }
            shift += 7;
            if shift > 35 {
                return Err("a number of the module is too long".into());
            }
        }
    }
    fn name(b: &[u8], i: &mut usize) -> Result<String, String> {
        let n = leb(b, i)? as usize;
        let s = b.get(*i..*i + n).ok_or("the module ends inside a name")?;
        *i += n;
        Ok(String::from_utf8_lossy(s).into_owned())
    }
    if b.get(..8) != Some(b"\0asm\x01\0\0\0") {
        return Err("it is no module of Wasm 1".into());
    }
    let mut i = 8;
    let mut exports: Vec<(String, u8)> = Vec::new();
    while i < b.len() {
        let id = b[i];
        i += 1;
        let size = leb(b, &mut i)? as usize;
        let end = i.checked_add(size).filter(|e| *e <= b.len()).ok_or("a section ends past the module")?;
        let mut j = i;
        match id {
            2 => {
                let n = leb(b, &mut j)?;
                if n > 0 {
                    let (m, f) = (name(b, &mut j)?, name(b, &mut j)?);
                    return Err(format!("it imports {n}, the first `{m}`.`{f}`"));
                }
            }
            7 => {
                for _ in 0..leb(b, &mut j)? {
                    let n = name(b, &mut j)?;
                    let kind = *b.get(j).ok_or("the module ends inside an export")?;
                    j += 1;
                    leb(b, &mut j)?;
                    exports.push((n, kind));
                }
            }
            _ => {}
        }
        i = end;
    }
    for (n, kind) in EXPORTS {
        if !exports.iter().any(|(e, k)| e == n && *k == kind) {
            return Err(format!("it does not export `{n}`"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_is_split_as_the_shell_splits_it() {
        let w = words(ritsu_emit::wasm::CARGO_LINE);
        assert_eq!(&w[..3], ["cargo", "rustc", "--lib"]);
        assert!(w.contains(&"profile.release.opt-level=\"s\"".to_string()), "{w:?}");
        assert!(w.contains(&"profile.release.strip=\"symbols\"".to_string()), "{w:?}");
        assert_eq!(w.iter().filter(|x| *x == "--config").count(), 4);
    }

    #[test]
    fn the_table_of_native_dependencies_is_left_out() {
        let toml = format!("[package]\nname = \"p\"\n\n[lib]\npath = \"lib.rs\"\n\n{}\nsqlx = \"=0.9.0\"\ntokio = \"=1\"\n", super::super::rust::NOT_WASM32);
        assert_eq!(without_native(&toml), "[package]\nname = \"p\"\n\n[lib]\npath = \"lib.rs\"\n");
        assert_eq!(without_native("[package]\nname = \"p\"\n"), "[package]\nname = \"p\"\n");
    }

    #[test]
    fn a_module_that_imports_or_lacks_an_export_is_said() {
        assert!(shape(b"not wasm").is_err());
        // no section at all: none of the exports
        assert_eq!(shape(b"\0asm\x01\0\0\0"), Err("it does not export `call`".to_string()));
        // an import section with one import, `env`.`f`, a function of type 0
        let mut m = b"\0asm\x01\0\0\0".to_vec();
        m.extend([2, 9, 1, 3, b'e', b'n', b'v', 1, b'f', 0, 0]);
        assert_eq!(shape(&m), Err("it imports 1, the first `env`.`f`".to_string()));
    }
}

#[cfg(test)]
mod scratch_tests {
    use super::*;

    #[test]
    fn the_scratch_crate_is_a_workspace_of_its_own() {
        assert!(own_workspace("[package]\nname = \"p\"\n").ends_with("\n[workspace]\n"));
    }

    #[test]
    fn commands_are_run_from_the_root_with_the_toolchain_named() {
        let c = clean(&std::ffi::OsString::from("cargo"), Path::new("/"), Path::new("/tmp/ritsu-test"), Some("stable"));
        assert_eq!(c.get_current_dir(), Some(Path::new("/")));
        assert_eq!(c.get_envs().find(|(k, _)| *k == "RUSTUP_TOOLCHAIN").and_then(|(_, v)| v), Some(std::ffi::OsStr::new("stable")));
        assert_eq!(c.get_envs().find(|(k, _)| *k == "CARGO_HOME").and_then(|(_, v)| v), Some(std::ffi::OsStr::new("/tmp/ritsu-test/cargo-home")));
    }

    #[cfg(unix)]
    #[test]
    fn a_scratch_directory_is_readable_by_the_user_alone() {
        use std::os::unix::fs::PermissionsExt;
        let s = Scratch::new().unwrap();
        assert_eq!(std::fs::metadata(&s.0).unwrap().permissions().mode() & 0o777, 0o700);
    }

    #[test]
    fn a_finished_processes_scratch_directory_is_cleaned() {
        let base = std::env::current_exe().unwrap().parent().unwrap().join(format!("ritsu-clean-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let stale = base.join(format!("ritsu-wasm-{}-0-0", i32::MAX));
        std::fs::create_dir(&stale).unwrap();
        clean_finished(&base);
        assert!(!stale.exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn a_shared_temporaries_permissions_fail_the_check() {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::current_exe().unwrap().parent().unwrap().join(format!("ritsu-ancestors-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        assert!(ancestors_private(&base));
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(!ancestors_private(&base));
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::remove_dir_all(&base);
    }

    /// After a run of a tool, SIGINT and SIGTERM do what they did before it, so that a build that
    /// ended does not keep the rest of the run from stopping.
    #[cfg(unix)]
    #[test]
    fn the_signals_do_what_they_did_after_a_run() {
        // each signal's handler, put back as it was
        let now = || {
            HANDED.map(|n| unsafe {
                let h = signal(n, 0);
                signal(n, h);
                h
            })
        };
        let before = now();
        watched(&mut Command::new("true")).unwrap();
        assert_eq!(now(), before);
        assert!(!signaled());
    }
}
