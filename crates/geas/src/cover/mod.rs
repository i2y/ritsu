//! The coverage switches and their readers (DESIGN §7.2, §7.3). During `geas map`,
//! every process geas starts for a `run` or `serve` target gets the switches of the
//! four runtimes at once, each pointing into a directory of its own,
//! `.geas/cover/<claim>/<k>/`, which the processes it starts in turn inherit. A
//! target does not say what it is written in; each runtime picks up its own switch
//! and ignores the rest. After a claim, its directories are read into what each of
//! its targets ran, and removed.

pub mod go;
pub mod node;
pub mod python;
pub mod rust;

use ritsu_base::text::Text;
use crate::lines::Lines;
use crate::proc::Env;
use crate::run::Started;
use crate::tree;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

const SITECUSTOMIZE: &str = include_str!("sitecustomize.py");
const NODE_HOOK: &str = include_str!("geas_cover.cjs");

/// What a runtime said about one file: the lines that are code, and those that ran.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileLines {
    pub code: Lines,
    pub ran: Lines,
}

/// Files by their path relative to the root.
pub type Report = BTreeMap<String, FileLines>;

/// Adds what one more report says about a file: code and run lines are unions, and
/// a line that ran is code.
pub fn add(into: &mut Report, rel: String, f: FileLines) {
    let e = into.entry(rel).or_default();
    e.code.extend(f.code.iter().copied());
    e.code.extend(f.ran.iter().copied());
    e.ran.extend(f.ran);
}

pub fn merge(into: &mut Report, from: Report) {
    for (rel, f) in from {
        add(into, rel, f);
    }
}

/// A problem met while reading coverage, at the `when` that started the process.
pub struct Problem {
    pub code: &'static str,
    pub msg: Text,
    pub notes: Vec<Text>,
    pub target: String,
    pub line: usize,
}

/// The switches of one `map` run: the hooks under `.geas/hook/`, the processes'
/// directories under `.geas/cover/`. Both are removed when it is dropped, also when
/// the run fails or panics.
pub struct Session {
    pub root: PathBuf,
    hook: PathBuf,
    cover: PathBuf,
    /// `--require "<hook>"`, the hook's path quoted for NODE_OPTIONS.
    node_require: String,
}

impl Session {
    /// Writes the hooks into `<geas_dir>/hook/`. `geas_dir` and `root` are absolute
    /// and canonical.
    pub fn start(geas_dir: &Path, root: &Path) -> io::Result<Session> {
        let hook = geas_dir.join("hook");
        let cover = geas_dir.join("cover");
        let _ = std::fs::remove_dir_all(&hook);
        let _ = std::fs::remove_dir_all(&cover);
        std::fs::create_dir_all(&hook)?;
        std::fs::create_dir_all(&cover)?;
        std::fs::write(hook.join("sitecustomize.py"), SITECUSTOMIZE)?;
        let node_hook = hook.join("geas_cover.cjs");
        std::fs::write(&node_hook, NODE_HOOK)?;
        // NODE_OPTIONS reads a double-quoted word with `\"` and `\\` in it
        let quoted = node_hook.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
        let node_require = format!("--require \"{quoted}\"");
        Ok(Session { root: root.to_path_buf(), hook, cover, node_require })
    }

    fn claim_dir(&self, claim: usize) -> PathBuf {
        self.cover.join(claim.to_string())
    }

    /// The directory of the k-th process of a claim, made.
    pub fn process(&self, claim: usize, k: usize) -> io::Result<PathBuf> {
        let dir = self.claim_dir(claim).join(k.to_string());
        std::fs::create_dir_all(&dir)?; // GOCOVERDIR has to exist before the start
        Ok(dir)
    }

    /// Adds the switches that point every runtime into a process's directory to the
    /// environment its pins give it. `PYTHONPATH` gains the hook in front of what
    /// the program would have seen, and `NODE_OPTIONS` the hook after it, so that a
    /// variable a pin sets, or `env clean` removes, is still what the program gets.
    pub fn switch(&self, env: &mut Env, dir: &Path) {
        let d = dir.as_os_str();
        let mut python = self.hook.clone().into_os_string();
        if let Some(p) = env.get("PYTHONPATH").filter(|p| !p.is_empty()) {
            python.push(":");
            python.push(p);
        }
        env.set("PYTHONPATH", python);
        env.set("GEAS_COVER_OUT", d);
        env.set("GEAS_COVER_ROOT", self.root.as_os_str());
        env.set("NODE_V8_COVERAGE", d);
        let node = match env.get("NODE_OPTIONS").map(|o| o.to_string_lossy().trim().to_string()) {
            Some(o) if !o.is_empty() => format!("{o} {}", self.node_require),
            _ => self.node_require.clone(),
        };
        env.set("NODE_OPTIONS", node);
        env.set("GOCOVERDIR", d);
        let mut profile = d.to_os_string();
        profile.push("/rust-%p-%m.profraw");
        env.set("LLVM_PROFILE_FILE", profile);
    }

    /// Removes what a claim's processes wrote, once it has been read.
    pub fn done_with(&self, claim: usize) {
        let _ = std::fs::remove_dir_all(self.claim_dir(claim));
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.hook);
        let _ = std::fs::remove_dir_all(&self.cover);
    }
}

/// The LLVM tools, once looked for.
enum Llvm {
    Found { profdata: PathBuf, cov: PathBuf },
    Missing(Text, Vec<Text>),
}

/// What reading needs across claims: the Go modules under the root, and the LLVM
/// tools, looked for once by whichever claim needs them first. Claims run side by
/// side under `--jobs`, so a reader is shared between threads.
pub struct Reader {
    root: PathBuf,
    modules: Vec<go::Module>,
    llvm: OnceLock<Llvm>,
}

/// What a claim's targets ran, per target, and the problems met reading it.
pub struct Slice {
    pub by_target: BTreeMap<String, Report>,
    pub problems: Vec<Problem>,
}

impl Reader {
    pub fn new(root: &Path) -> Reader {
        Reader { root: root.to_path_buf(), modules: go_modules(root), llvm: OnceLock::new() }
    }

    /// Reads what the processes a claim started wrote.
    pub fn read_claim(&self, started: &[Started]) -> Slice {
        let mut slice = Slice { by_target: BTreeMap::new(), problems: vec![] };
        let mut targets: Vec<&str> = started.iter().map(|s| s.target.as_str()).collect();
        targets.sort();
        targets.dedup();
        for target in targets {
            let procs: Vec<&Started> = started.iter().filter(|s| s.target == target && s.dir.is_some()).collect();
            let mut report = Report::new();
            for p in &procs {
                let dir = p.dir.as_deref().expect("filtered");
                self.read_python(p, dir, &mut report, &mut slice.problems);
                self.read_node(p, dir, &mut report, &mut slice.problems);
            }
            self.read_go(&procs, &mut report, &mut slice.problems);
            self.read_rust(&procs, &mut report, &mut slice.problems);
            slice.by_target.insert(target.to_string(), report);
        }
        slice
    }

    fn read_python(&self, p: &Started, dir: &Path, report: &mut Report, problems: &mut Vec<Problem>) {
        for f in files_named(dir, "python-", ".json") {
            match std::fs::read_to_string(&f).map_err(|e| e.to_string()).and_then(|text| python::read(&text, &self.root)) {
                Ok(r) => merge(report, r),
                Err(why) => problems.push(unreadable(p, &f, &why)),
            }
        }
    }

    fn read_node(&self, p: &Started, dir: &Path, report: &mut Report, problems: &mut Vec<Problem>) {
        let source = |path: &Path| std::fs::read_to_string(path).ok();
        for f in files_named(dir, "coverage-", ".json") {
            match std::fs::read_to_string(&f).map_err(|e| e.to_string()).and_then(|text| node::read(&text, &self.root, &source)) {
                Ok(r) => merge(report, r),
                Err(why) => problems.push(unreadable(p, &f, &why)),
            }
        }
    }

    /// Go writes `covmeta.<hash>` when a program built with `-cover` starts and
    /// `covcounters.<hash>.<pid>.<time>` when it exits normally. Metadata without
    /// counters is a program that did not exit normally: E066.
    fn read_go(&self, procs: &[&Started], report: &mut Report, problems: &mut Vec<Problem>) {
        let mut dirs = Vec::new();
        for p in procs {
            let dir = p.dir.as_deref().expect("filtered");
            let metas = files_named(dir, "covmeta.", "");
            if metas.is_empty() {
                continue;
            }
            let counters = files_named(dir, "covcounters.", "");
            let unwritten: Vec<String> = metas
                .iter()
                .filter_map(|m| {
                    let hash = m.file_name()?.to_str()?.strip_prefix("covmeta.")?.to_string();
                    let has = counters.iter().any(|c| {
                        c.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&format!("covcounters.{hash}.")))
                    });
                    (!has).then_some(hash)
                })
                .collect();
            if !unwritten.is_empty() {
                problems.push(Problem {
                    code: "E066",
                    msg: tr!(
                        "`{}` が起動した Go のプログラムは、カバレッジのメタデータを書きましたが、カウンターを書いていません。正常に終了しなかったためです",
                        "the Go program that `{}` started wrote its coverage metadata but no counters: it did not exit normally",
                        p.target,
                    ),
                    notes: vec![
                        command_note(p),
                        tr!(
                            "Go のプログラムがカウンターを書くのは、main から戻ったときか os.Exit を呼んだときです。サービスは SIGTERM を受けてそうする必要があります（signal.NotifyContext と Server.Shutdown）",
                            "a Go program writes its counters when it returns from main or calls os.Exit; a service has to do so on SIGTERM (signal.NotifyContext and Server.Shutdown)",
                        ),
                    ],
                    target: p.target.clone(),
                    line: p.line,
                });
            }
            if !counters.is_empty() {
                dirs.push(dir.to_path_buf());
            }
        }
        let Some(first) = dirs.first() else {
            return;
        };
        let p = procs[0];
        let out = first.join("go.txt");
        let list: Vec<String> = dirs.iter().map(|d| d.to_string_lossy().into_owned()).collect();
        let run = Command::new("go")
            .args(["tool", "covdata", "textfmt"])
            .arg(format!("-i={}", list.join(",")))
            .arg(format!("-o={}", out.to_string_lossy()))
            .env_remove("GOCOVERDIR")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output();
        let failed = |msg: Text, notes: Vec<Text>| Problem { code: "E065", msg, notes, target: p.target.clone(), line: p.line };
        match run {
            Err(e) if e.kind() == io::ErrorKind::NotFound => problems.push(failed(
                tr!(
                    "PATH に go がありません。geas は Go のプログラムのカバレッジを `go tool covdata` で読みます",
                    "go is not on PATH, and geas reads a Go program's coverage with `go tool covdata`",
                ),
                vec![command_note(p)],
            )),
            Err(e) => problems.push(failed(
                tr!("`go tool covdata` を起動できません: {e}", "cannot start `go tool covdata`: {e}"),
                vec![],
            )),
            Ok(o) if !o.status.success() => problems.push(failed(
                tr!("`go tool covdata textfmt` が失敗しました", "`go tool covdata textfmt` failed"),
                tail(&o.stderr),
            )),
            Ok(_) => match std::fs::read_to_string(&out).map_err(|e| e.to_string()).and_then(|text| go::read(&text, &self.modules)) {
                Ok(r) => merge(report, r),
                Err(why) => problems.push(unreadable(p, &out, &why)),
            },
        }
    }

    /// Profiles are converted with the programs that wrote them, which geas knows
    /// when it started them itself: a profile whose pid is a process geas started,
    /// running a program that carries coverage mapping, or one with the same
    /// signature. Any other profile came from a program geas did not start: W061.
    fn read_rust(&self, procs: &[&Started], report: &mut Report, problems: &mut Vec<Problem>) {
        let mut found: Vec<(&Started, PathBuf, u32, String)> = Vec::new();
        for p in procs {
            let dir = p.dir.as_deref().expect("filtered");
            for f in files_named(dir, "rust-", ".profraw") {
                let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                if let Some((pid, sig)) = rust::profile_name(&name) {
                    found.push((p, f, pid, sig.to_string()));
                }
            }
        }
        if found.is_empty() {
            return;
        }
        let first = found[0].0;
        let (profdata_tool, cov_tool) = match self.llvm_tools() {
            Ok(tools) => tools,
            Err((msg, mut notes)) => {
                notes.insert(0, command_note(first));
                problems.push(Problem { code: "E065", msg, notes, target: first.target.clone(), line: first.line });
                return;
            }
        };
        // the signature of each program geas started
        let mut programs: BTreeMap<String, PathBuf> = BTreeMap::new();
        for (p, _, pid, sig) in &found {
            if *pid == p.pid
                && let Some(prog) = &p.program
                && has_coverage_mapping(prog)
            {
                programs.insert(sig.clone(), prog.clone());
            }
        }
        let mut profiles = Vec::new();
        let mut objects: BTreeSet<PathBuf> = BTreeSet::new();
        let mut strays: Vec<&Started> = Vec::new();
        for (p, f, _, sig) in &found {
            match programs.get(sig) {
                Some(prog) => {
                    profiles.push(f.clone());
                    objects.insert(prog.clone());
                }
                None => {
                    if !strays.iter().any(|s| std::ptr::eq(*s, *p)) {
                        strays.push(p);
                    }
                }
            }
        }
        for p in strays {
            problems.push(Problem {
                code: "W061",
                msg: tr!(
                    "`{}` が起動した Rust のプログラムがプロファイルを書きましたが、geas が直接起動したプログラムではないので、その実行は記録に入れていません",
                    "a Rust program that `{}` started wrote a profile, but geas did not start that program itself, so what it ran is left out of the record",
                    p.target,
                ),
                notes: vec![
                    command_note(p),
                    tr!(
                        "llvm-cov には、プロファイルを書いたプログラムが要ります。geas が知っているのは自分で起動したプログラムだけです。-C instrument-coverage を付けてビルドしたプログラムを、`cargo run` やスクリプトを通さず、コマンドで直接起動してください（`run \"./tally\"`）",
                        "llvm-cov needs the program a profile came from, and geas knows only the programs it starts: make the command start the program built with -C instrument-coverage itself (`run \"./tally\"`), not through `cargo run` or a script",
                    ),
                ],
                target: p.target.clone(),
                line: p.line,
            });
        }
        if profiles.is_empty() {
            return;
        }
        let p = procs[0];
        let dir = p.dir.as_deref().expect("filtered");
        let merged = dir.join("rust.profdata");
        let failed = |what: &str, stderr: &[u8]| Problem {
            code: "E065",
            msg: tr!("`{what}` が失敗しました", "`{what}` failed"),
            notes: tail(stderr),
            target: p.target.clone(),
            line: p.line,
        };
        let merge_run = Command::new(&profdata_tool)
            .args(["merge", "-sparse"])
            .args(&profiles)
            .arg("-o")
            .arg(&merged)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output();
        match merge_run {
            Ok(o) if o.status.success() => {}
            Ok(o) => return problems.push(failed("llvm-profdata merge", &o.stderr)),
            Err(e) => return problems.push(failed("llvm-profdata merge", e.to_string().as_bytes())),
        }
        let mut export = Command::new(&cov_tool);
        export.args(["export", "-format=lcov"]).arg(format!("-instr-profile={}", merged.to_string_lossy()));
        for (i, o) in objects.iter().enumerate() {
            if i > 0 {
                export.arg("-object");
            }
            export.arg(o);
        }
        match export.stdin(Stdio::null()).stderr(Stdio::piped()).stdout(Stdio::piped()).output() {
            Ok(o) if o.status.success() => match rust::read_lcov(&String::from_utf8_lossy(&o.stdout), &self.root) {
                Ok(r) => merge(report, r),
                Err(why) => problems.push(Problem {
                    code: "E065",
                    msg: tr!(
                        "`llvm-cov export` の出力を読めません: {why}",
                        "geas cannot read what `llvm-cov export` wrote: {why}",
                    ),
                    notes: vec![],
                    target: p.target.clone(),
                    line: p.line,
                }),
            },
            Ok(o) => problems.push(failed("llvm-cov export", &o.stderr)),
            Err(e) => problems.push(failed("llvm-cov export", e.to_string().as_bytes())),
        }
    }

    /// `GEAS_LLVM_BIN` when it is set, and then only there; else the toolchain's
    /// sysroot (`rustc --print sysroot`, `lib/rustlib/<host>/bin`); else PATH.
    fn llvm_tools(&self) -> Result<(PathBuf, PathBuf), (Text, Vec<Text>)> {
        match self.llvm.get_or_init(look_for_llvm) {
            Llvm::Found { profdata, cov } => Ok((profdata.clone(), cov.clone())),
            Llvm::Missing(msg, notes) => Err((msg.clone(), notes.clone())),
        }
    }
}

fn look_for_llvm() -> Llvm {
    let names = ["llvm-profdata", "llvm-cov"];
    if let Some(dir) = std::env::var_os("GEAS_LLVM_BIN") {
        let d = PathBuf::from(&dir);
        let shown = d.to_string_lossy().into_owned();
        for n in names {
            if !d.join(n).is_file() {
                return Llvm::Missing(
                    tr!(
                        "GEAS_LLVM_BIN は `{shown}` ですが、そこに `{n}` がありません",
                        "GEAS_LLVM_BIN is `{shown}`, and it has no `{n}`",
                    ),
                    vec![tr!(
                        "GEAS_LLVM_BIN には llvm-profdata と llvm-cov のあるディレクトリを書きます。これが設定されていると、geas はほかの場所を探しません",
                        "GEAS_LLVM_BIN names the directory holding llvm-profdata and llvm-cov; when it is set, geas looks nowhere else",
                    )],
                );
            }
        }
        return Llvm::Found { profdata: d.join(names[0]), cov: d.join(names[1]) };
    }
    if let Some(bin) = sysroot_bin()
        && names.iter().all(|n| bin.join(n).is_file())
    {
        return Llvm::Found { profdata: bin.join(names[0]), cov: bin.join(names[1]) };
    }
    if let (Some(p), Some(c)) = (on_path(names[0]), on_path(names[1])) {
        return Llvm::Found { profdata: p, cov: c };
    }
    Llvm::Missing(
        tr!(
            "Rust のプログラムがプロファイルを書きましたが、それを読む llvm-profdata と llvm-cov が見つかりません。Rust のツールチェーンの sysroot にも PATH にもありません",
            "a Rust program wrote a profile, and geas cannot find llvm-profdata and llvm-cov to read it: they are neither in the Rust toolchain's sysroot nor on PATH",
        ),
        vec![tr!(
            "rustup の llvm-tools コンポーネントに入っています（`rustup component add llvm-tools`）。ほかの場所にあるなら、GEAS_LLVM_BIN にそのディレクトリを設定してください",
            "they come with rustup's llvm-tools component (`rustup component add llvm-tools`); or set GEAS_LLVM_BIN to a directory that holds them",
        )],
    )
}

/// `<rustc --print sysroot>/lib/rustlib/<host>/bin`, the host from `rustc -vV`.
fn sysroot_bin() -> Option<PathBuf> {
    let out = |args: &[&str]| -> Option<String> {
        let o = Command::new("rustc").args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
        o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let sysroot = out(&["--print", "sysroot"])?;
    let host = out(&["-vV"])?.lines().find_map(|l| l.strip_prefix("host: ").map(str::to_string))?;
    Some(PathBuf::from(sysroot.trim()).join("lib/rustlib").join(host.trim()).join("bin"))
}

/// A program on PATH, as the OS would find it.
pub fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(name)).find(|p| std::fs::metadata(p).is_ok_and(|m| m.is_file() && executable(&m)))
}

#[cfg(unix)]
fn executable(m: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    m.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_: &std::fs::Metadata) -> bool {
    true
}

/// Whether a file is a Mach-O or ELF program with LLVM's coverage mapping in it:
/// the only programs `llvm-cov` can read lines from.
fn has_coverage_mapping(p: &Path) -> bool {
    let Ok(bytes) = std::fs::read(p) else {
        return false;
    };
    let magic = bytes.get(..4);
    let native = magic == Some(&[0xCF, 0xFA, 0xED, 0xFE]) || magic == Some(&[0x7F, b'E', b'L', b'F']);
    native && bytes.windows(13).any(|w| w == b"__llvm_covmap")
}

/// The Go modules under the root, from the `module` line of every `go.mod`.
fn go_modules(root: &Path) -> Vec<go::Module> {
    let mut out = Vec::new();
    find_go_mods(root, "", &mut out);
    out
}

fn find_go_mods(dir: &Path, rel: &str, out: &mut Vec<go::Module>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let Some(name) = e.file_name().to_str().map(String::from) else {
            continue;
        };
        if tree::EXCLUDED.contains(&name.as_str()) {
            continue;
        }
        let Ok(kind) = e.file_type() else {
            continue;
        };
        if kind.is_dir() {
            let r = if rel.is_empty() { name } else { format!("{rel}/{name}") };
            find_go_mods(&e.path(), &r, out);
        } else if kind.is_file()
            && name == "go.mod"
            && let Some(path) = std::fs::read_to_string(e.path()).ok().as_deref().and_then(go::module_path)
        {
            out.push(go::Module { path, dir: rel.to_string() });
        }
    }
}

/// The files in `dir` whose names start with `prefix` and end with `suffix`, sorted.
fn files_named(dir: &Path, prefix: &str, suffix: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut v: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| {
            e.file_name().to_str().is_some_and(|n| n.starts_with(prefix) && n.ends_with(suffix) && n.len() > prefix.len())
        })
        .map(|e| e.path())
        .collect();
    v.sort();
    v
}

fn command_note(p: &Started) -> Text {
    let w = crate::proc::shell_words(&p.words);
    tr!("コマンド: {w}", "command: {w}")
}

/// The last lines a tool wrote on stderr, one note each.
fn tail(stderr: &[u8]) -> Vec<Text> {
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let from = lines.len().saturating_sub(5);
    lines[from..].iter().map(|l| Text::same(crate::diag::cut(l, 160))).collect()
}

fn unreadable(p: &Started, file: &Path, why: &str) -> Problem {
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Problem {
        code: "E065",
        msg: tr!(
            "`{}` が残したカバレッジのファイル {name} を読めません: {why}",
            "geas cannot read the coverage file {name} that `{}` left: {why}",
            p.target,
        ),
        notes: vec![command_note(p)],
        target: p.target.clone(),
        line: p.line,
    }
}

/// What the reader tests share: the recorded fixtures under `tests/cover/`, and
/// the golden files the readers' answers are held to.
#[cfg(test)]
pub mod fixtures {
    use super::Report;
    use std::path::{Path, PathBuf};

    pub fn dir(lang: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cover").join(lang)
    }

    pub fn read(lang: &str, name: &str) -> String {
        let p = dir(lang).join(name);
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
    }

    /// A file's code and run lines, one file a paragraph.
    pub fn render(r: &Report) -> String {
        r.iter()
            .map(|(path, f)| {
                format!(
                    "{path}\n  code: {}\n  ran:  {}\n",
                    crate::lines::to_ranges(&f.code),
                    crate::lines::to_ranges(&f.ran)
                )
            })
            .collect()
    }

    /// Holds `actual` to `tests/golden/cover/<name>`; GEAS_BLESS writes it instead.
    pub fn golden(name: &str, actual: &str) {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/cover").join(name);
        if std::env::var_os("GEAS_BLESS").is_some() {
            std::fs::create_dir_all(p.parent().expect("a directory")).expect("make the golden directory");
            std::fs::write(&p, actual).expect("write the golden");
            return;
        }
        let want = std::fs::read_to_string(&p)
            .unwrap_or_else(|_| panic!("tests/golden/cover/{name} is missing; with GEAS_BLESS=1 it would hold:\n{actual}"));
        assert_eq!(want, actual, "the reader's answer differs from tests/golden/cover/{name}");
    }
}
