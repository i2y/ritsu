//! `ritsu mod fetch` (DESIGN 17.12): the modules of the lock, or the versions named, fetched from
//! their registries into the store, each checked against its digests and its signatures; [`tree`]
//! and [`version`], the same fetch for `ritsu mod diff` and `ritsu mod report` (DESIGN 17.9.6,
//! 17.10), which fetch what they read before they read it; and [`for_choosing`], which `ritsu mod
//! lock`, `add`, `get` and `remove` call when a version they choose by is in neither the store nor
//! the module's copy of it.
//!
//! A version is fetched in this order, and nothing is put in the store until every version of the
//! run has passed: its OCI manifest by its digest (the bytes' sha256 held to it; its type, its two
//! layers and their sizes checked), the `ritsu.mod` layer and the tar of the files (each held to its
//! digest, the files' to the lock's too), the tar read (the forms ritsu writes, and nothing else), its
//! `ritsu.mod` the first layer's bytes, with the name and the version asked for. Then the `signature`
//! lines that apply (DESIGN 17.14: the module at hand's, and those of each module that requires the
//! version directly) are checked with cosign, and the versions are put in the store (made under
//! `tmp/`, then renamed), each with its record and its line of the version index.
//!
//! A module that is here already (in the module's copy of the store, or in the store) has its files
//! held to the lock's digest: the store's are fetched again when they differ, and a copy in the
//! module that differs stops the run (`ritsu mod vendor` puts it again). A line that applies to it
//! and that its record does not name is checked with cosign, and the record (where `check` reads it)
//! names it after.
//!
//! The tree is read through the files of the thread (`ritsu_base::fs`). `--rev` reads it at a git
//! revision ([`GitRevision`]): the `ritsu.mod`, the lock, the module's copy of the store, the
//! modules a path reaches and the key files as the revision has them, a path outside the repository
//! from the disk; `ritsu mod report` reads the base's tree so. A key file read through other files
//! than the disk is written under the store's `tmp/` for cosign, and removed after. What is put in
//! the store is written to the disk, and the module's copy of the store at a revision is not written.

use super::cosign::{self, Rule};
use super::http::NoAnswer;
use super::registry::{self, Remote};
use super::{Ctx, store_write};
use ritsu_base::cli::Args;
use ritsu_base::diag::Diag;
use ritsu_base::fs::{self, GitRevision};
use ritsu_base::json::Json;
use ritsu_base::modules::graph::{self, Problem};
use ritsu_base::modules::lock::{self as lockfile, Lock, Pinned};
use ritsu_base::modules::manifest::{self, Manifest, Policy, Signature, Source};
use ritsu_base::modules::names::{self, ModuleName};
use ritsu_base::modules::semver::Version;
use ritsu_base::modules::store::{self, Parts, Record, Store};
use ritsu_base::modules::{E304, artifact, digest, tar};
use ritsu_base::text::{Lang, Text, plural};
use ritsu_base::tr;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// How a fetch stops.
enum Stop {
    /// What was fetched, or what is here, is not what it has to be: exit 1.
    Refused(Text),
    /// A digest of the lock that the OCI manifest does not have (E304): exit 1, said as the lock's
    /// diagnostic.
    Lock(Box<Diag>),
    /// The run cannot go on (the network, the credentials, no cosign, no lock): exit 2.
    Cannot(Text),
}

impl Stop {
    /// What it says, as one sentence.
    fn text(self) -> Text {
        match self {
            Stop::Refused(t) | Stop::Cannot(t) => t,
            Stop::Lock(d) => d.message,
        }
    }
}

/// How a fetch stops when a request to the registry has no answer: an answer over the bytes it may
/// have is what the registry holds, and not what it has to be (exit 1, said as `over` says it);
/// anything else is that the registry cannot be asked (exit 2).
fn unanswered(n: NoAnswer, over: impl FnOnce(Text) -> Text) -> Stop {
    match n {
        NoAnswer::Over(t) => Stop::Refused(over(t)),
        NoAnswer::Other(t) => Stop::Cannot(t),
    }
}

/// What a layer says whose answer is over the `size` bytes its OCI manifest names.
fn over_layer(name: &ModuleName, version: &Version, what: &str, want: &str, size: u64) -> Text {
    tr!(
        "`{name}` {version} の層 `{what}` として受け取るバイトが、OCI のマニフェストが書く {size} バイト（{want}）を超えました",
        "the bytes fetched as the layer `{what}` of `{name}` {version} are over the {size} bytes its OCI manifest names ({want})"
    )
}

/// A version fetched, its digests checked, not yet in the store.
struct Staged {
    name: ModuleName,
    version: Version,
    manifest: Vec<u8>,
    digest: String,
    module: Vec<u8>,
    files: Vec<(String, Vec<u8>)>,
    files_digest: String,
}

/// A module of the tree, as a fetch sees it.
struct Member {
    name: ModuleName,
    manifest: Option<Manifest>,
    /// Where its files are: its directory, or the `files/` of the entry that is read.
    dir: Option<PathBuf>,
    /// For one staged, its place among the versions staged.
    staged: Option<usize>,
    /// For a module from a registry: its version, the digest of its OCI manifest, and the copy of
    /// the store it is read from when it is here.
    pinned: Option<(Version, String, Option<Store>)>,
    /// Whether it is here in the module's copy of the store.
    copy: bool,
}

/// What a run did, for what it says: each version fetched, and each that was here, with the lines
/// its signature was checked against in this run.
#[derive(Default)]
pub struct Done {
    fetched: Vec<(String, String, String, Vec<Rule>)>,
    present: Vec<(String, String, String, Vec<Rule>)>,
}

impl Done {
    /// What the run did, a line each: each version fetched (with the lines checked under it), and
    /// each line checked of a version that was here. None when it fetched and checked nothing.
    fn lines(&self, lang: Lang) -> Vec<String> {
        let mut out = Vec::new();
        for (m, v, d, rs) in &self.fetched {
            out.push(tr!("{m} {v} を取り出しました（{d}）", "fetched {m} {v} ({d})").get(lang).to_string());
            for r in rs {
                let line = &r.line;
                out.push(format!("  {}", tr!("署名を `{line}` で確かめました", "checked its signature against `{line}`").get(lang)));
            }
        }
        for (m, v, _, rs) in &self.present {
            for r in rs {
                let line = &r.line;
                out.push(tr!("{m} {v} の署名を `{line}` で確かめました", "checked the signature of {m} {v} against `{line}`").get(lang).to_string());
            }
        }
        out
    }
}

/// What a fetch for another command did, said on the standard error (the standard output is that
/// command's): a line for each version fetched and each line checked; nothing when nothing was.
pub fn told(lang: Lang, done: &Done) {
    for l in done.lines(lang) {
        eprintln!("{l}");
    }
}

/// The `ritsu.mod` of a version, read, as the sentence of a version fetched names it.
fn read_mod(bytes: &[u8], name: &ModuleName, version: &Version) -> Result<Manifest, Stop> {
    manifest::parse(bytes, manifest::FILE).map_err(|ds| {
        let first = ds.first().map(|d| d.message.clone()).unwrap_or_default();
        Stop::Refused(tr!("`{name}` {version} の `ritsu.mod` を読めません。", "the `ritsu.mod` of `{name}` {version} cannot be read: ").then(&first))
    })
}

/// The version `version` of `name` fetched from `remote` by `pinned` (the digest of its OCI manifest;
/// else its tag), and checked: none of it is in the store. `files` is the digest of its files the
/// lock pins, at the lock's line `line`.
fn stage(remote: &Remote, name: &ModuleName, version: &Version, pinned: Option<&str>, files: Option<(&str, usize)>) -> Result<Staged, Stop> {
    let reference = pinned.map(str::to_string).unwrap_or_else(|| version.to_string());
    let Some(bytes) = remote.manifest(&reference).map_err(|n| unanswered(n, |t| tr!("`{name}` {version}: ", "`{name}` {version}: ").then(&t)))? else {
        return Err(Stop::Cannot(tr!("レジストリに `{name}` {version}（{reference}）がありません", "the registry has no `{name}` {version} ({reference})")));
    };
    let d = digest::of(&bytes);
    if let Some(p) = pinned
        && d != p
    {
        return Err(Stop::Refused(tr!(
            "`{name}` {version} の OCI のマニフェストとして受け取ったバイトのダイジェストは {d} で、求めた {p} ではありません",
            "the bytes fetched as the OCI manifest of `{name}` {version} have the digest {d}, not the {p} asked for"
        )));
    }
    let layers = artifact::check(&bytes).map_err(|t| Stop::Refused(tr!("`{name}` {version}: ", "`{name}` {version}: ").then(&t)))?;
    if let Some((f, line)) = files
        && layers.files_digest != f
    {
        let theirs = &layers.files_digest;
        return Err(Stop::Lock(Box::new(
            Diag::at(E304, lockfile::FILE, line, 0, tr!(
                "`ritsu.lock` は `{name}` {version} のファイルを {f} に固定していますが、OCI のマニフェスト {d} のファイルの層は {theirs} です。`ritsu mod lock` を走らせてください",
                "`ritsu.lock` pins the files of `{name}` {version} at {f}, and the files' layer of the OCI manifest {d} is {theirs}; run `ritsu mod lock`"
            ))
            .rel(lockfile::FILE),
        )));
    }
    let layer = |what: &str, want: &str, size: u64| -> Result<Vec<u8>, Stop> {
        let b = remote.blob(want, size).map_err(|n| unanswered(n, |_| over_layer(name, version, what, want, size)))?;
        let got = digest::of(&b);
        if got != want || b.len() as u64 != size {
            let n = b.len();
            return Err(Stop::Refused(tr!(
                "`{name}` {version} の層 `{what}` として受け取った {n} バイトのダイジェストは {got} で、OCI のマニフェストが書く {want}（{size} バイト）ではありません",
                "the {n} bytes fetched as the layer `{what}` of `{name}` {version} have the digest {got}, not the {want} ({size} bytes) its OCI manifest names"
            )));
        }
        Ok(b)
    };
    let module = layer(artifact::MOD_TYPE, &layers.module_digest, layers.module_size)?;
    let tarred = layer(artifact::FILES_TYPE, &layers.files_digest, layers.files_size)?;
    let files = tar::read(&tarred).map_err(|e| Stop::Refused(tr!("`{name}` {version} のファイルの tar: ", "the tar of the files of `{name}` {version}: ").then(&e.text())))?;
    if !files.iter().any(|(p, b)| p == manifest::FILE && *b == module) {
        return Err(Stop::Refused(tr!(
            "`{name}` {version} のファイルの `ritsu.mod` が、一つ目の層の `ritsu.mod` と同じではありません",
            "the `ritsu.mod` among the files of `{name}` {version} is not the `ritsu.mod` of its first layer"
        )));
    }
    let m = read_mod(&module, name, version)?;
    if &m.module.value != name {
        let other = &m.module.value;
        return Err(Stop::Refused(tr!(
            "`{name}` {version} として取った `ritsu.mod` は、`module {other}` です",
            "the `ritsu.mod` fetched as `{name}` {version} says `module {other}`"
        )));
    }
    if m.version.as_ref().map(|v| &v.value) != Some(version) {
        let theirs = m.version.as_ref().map(|v| v.value.to_string()).unwrap_or_else(|| "-".to_string());
        return Err(Stop::Refused(tr!(
            "`{name}` {version} として取った `ritsu.mod` の `version` は {theirs} です",
            "the `ritsu.mod` fetched as `{name}` {version} has the `version` {theirs}"
        )));
    }
    Ok(Staged { name: name.clone(), version: version.clone(), manifest: bytes, digest: d, module, files, files_digest: layers.files_digest })
}

/// The module at hand and every module its paths reach, read through the files of this thread; a
/// path is joined and folded by its letters, as the tree is read (`graph::load`).
fn locals(root: &Path, main: Manifest) -> Vec<Member> {
    let mut out = vec![Member { name: main.module.value.clone(), manifest: Some(main), dir: Some(root.to_path_buf()), staged: None, pinned: None, copy: false }];
    let mut i = 0;
    while i < out.len() {
        let dir = out[i].dir.clone().unwrap_or_default();
        let reqs = out[i].manifest.as_ref().map(|m| m.requires.clone()).unwrap_or_default();
        for r in reqs {
            let Source::Path { path } = &r.source else { continue };
            let d = ritsu_base::paths::absolute(&dir.join(path));
            if out.iter().any(|m| m.dir.as_deref() == Some(d.as_path())) {
                continue;
            }
            let Some(m) = fs::raw::read(&d.join(manifest::FILE)).ok().and_then(|b| manifest::parse(&b, manifest::FILE).ok()) else { continue };
            out.push(Member { name: m.module.value.clone(), manifest: Some(m), dir: Some(d), staged: None, pinned: None, copy: false });
        }
        i += 1;
    }
    out
}

/// Whether the tree of `members` asks for a module from a registry, which needs a lock.
fn needs_lock(members: &[Member]) -> bool {
    let local: BTreeSet<&ModuleName> = members.iter().map(|m| &m.name).collect();
    members.iter().filter_map(|m| m.manifest.as_ref()).flat_map(|m| &m.requires).any(|r| match &r.source {
        Source::Oci { repository, .. } => ModuleName::parse(repository).is_ok_and(|n| !local.contains(&n)),
        Source::Path { .. } => false,
    })
}

/// The key files a run writes for cosign, under the store's `tmp/` (made when the first is
/// written), removed when the run is done with them.
#[derive(Default)]
struct Keys {
    scratch: Option<PathBuf>,
    written: Vec<PathBuf>,
}

impl Keys {
    /// `bytes` written to a file of their own: its path, absolute.
    fn write(&mut self, bytes: &[u8]) -> Result<PathBuf, Stop> {
        let dir = match &self.scratch {
            Some(d) => d.clone(),
            None => {
                let d = store_write::scratch().map_err(Stop::Cannot)?;
                self.scratch = Some(d.clone());
                d
            }
        };
        let f = store_write::unique(&dir, "key");
        std::fs::write(&f, bytes).map_err(|e| {
            let d = f.display();
            Stop::Cannot(tr!("{d} に書けません（{e}）", "cannot write {d} ({e})"))
        })?;
        self.written.push(f.clone());
        Ok(ritsu_base::paths::absolute(&f))
    }
}

impl Drop for Keys {
    fn drop(&mut self) {
        for k in &self.written {
            let _ = std::fs::remove_file(k);
        }
    }
}

/// The line `s` of the member `w`, as a rule to check: for a key, its file read from the member's
/// files through the files of this thread. cosign reads a key from the disk: a key of a version
/// staged, and one read through other files than the disk (a git revision's), is written to a file
/// of [`Keys`]; any other is given at its own path.
fn rule_of(members: &[Member], staged: &[Staged], w: usize, s: &Signature, keys: &mut Keys) -> Result<Rule, Stop> {
    let Policy::Key { path, .. } = &s.policy else {
        return Ok(Rule { checked: s.checked(None), line: s.text(), policy: s.policy.clone(), key: None });
    };
    let who = &members[w].name;
    let missing = || {
        let line = s.text();
        Stop::Refused(tr!("`{who}` の `{line}` の鍵のファイル `{path}` がありません", "the key file `{path}` of `{line}` in `{who}` is not there"))
    };
    let (bytes, file) = match (&members[w].dir, members[w].staged) {
        (Some(dir), _) => {
            let f = ritsu_base::paths::absolute(&dir.join(path));
            let b = fs::raw::read(&f).map_err(|_| missing())?;
            let file = if fs::current().is_some() { keys.write(&b)? } else { f };
            (b, file)
        }
        (None, Some(k)) => {
            let b = staged[k].files.iter().find(|(p, _)| p == path).map(|(_, b)| b.clone()).ok_or_else(missing)?;
            let file = keys.write(&b)?;
            (b, file)
        }
        (None, None) => return Err(missing()),
    };
    Ok(Rule { checked: s.checked(Some(&bytes)), line: s.text(), policy: s.policy.clone(), key: Some(file) })
}

/// The rules that apply to each member from a registry (DESIGN 17.14): the lines of the module at
/// hand that cover it, and those of each member that requires it directly. With `reach_all`, every
/// member counts as reached (the versions named, all of one tree); else those the module at hand's
/// `require`s reach.
fn rules(members: &[Member], staged: &[Staged], reach_all: bool, keys: &mut Keys) -> Result<Vec<Vec<Rule>>, Stop> {
    let n = members.len();
    let local: Vec<usize> = (0..n).filter(|i| members[*i].pinned.is_none()).collect();
    // each `oci` require of each member, to the member it reaches (a path wins)
    let target = |name: &ModuleName| -> Option<usize> {
        local.iter().copied().find(|i| members[*i].name == *name).or_else(|| (0..n).find(|j| members[*j].pinned.is_some() && members[*j].name == *name))
    };
    let edges: Vec<Vec<usize>> = members
        .iter()
        .map(|m| {
            m.manifest
                .as_ref()
                .map(|mm| {
                    mm.requires
                        .iter()
                        .filter_map(|r| match &r.source {
                            Source::Oci { repository, .. } => ModuleName::parse(repository).ok().and_then(|x| target(&x)),
                            Source::Path { .. } => None,
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();
    let mut reached = vec![reach_all; n];
    let mut queue: Vec<usize> = local.clone();
    for i in &local {
        reached[*i] = true;
    }
    while let Some(i) = queue.pop() {
        for j in &edges[i] {
            if !reached[*j] {
                reached[*j] = true;
                queue.push(*j);
            }
        }
    }
    let mut out = vec![Vec::new(); n];
    for j in 0..n {
        if members[j].pinned.is_none() {
            continue;
        }
        let name = members[j].name.to_string();
        let writers: Vec<usize> = (0..n).filter(|w| *w == 0 && members[0].pinned.is_none() || (*w != j && reached[*w] && edges[*w].contains(&j))).collect();
        let mut rs: Vec<Rule> = Vec::new();
        for w in writers {
            let lines = members[w].manifest.as_ref().map(|m| m.signatures.clone()).unwrap_or_default();
            for s in lines.iter().filter(|s| names::covers(&s.prefix, &name)) {
                let r = rule_of(members, staged, w, s, keys)?;
                if !rs.iter().any(|x| x.checked == r.checked) {
                    rs.push(r);
                }
            }
        }
        out[j] = rs;
    }
    Ok(out)
}

/// What a run says when it stops: the exit code.
fn stopped(ctx: &Ctx, root: Option<&Path>, s: Stop) -> u8 {
    let (code, text) = match s {
        Stop::Refused(t) => (1, t),
        Stop::Cannot(t) => (2, t),
        Stop::Lock(d) => {
            if !ctx.json() {
                let here = ctx.here();
                let found = ctx.findings(root.unwrap_or(&here), std::slice::from_ref(&*d));
                eprint!("{}", found.iter().map(|f| f.text_of("ritsu")).collect::<String>());
            }
            (1, d.message.clone())
        }
    };
    let head = tr!("何も置き場に置いていません。", "nothing was put in the store: ");
    let said = head.then(&text);
    if ctx.json() {
        let o = Json::obj([
            ("ritsu", Json::str(env!("CARGO_PKG_VERSION"))),
            ("fetched", Json::arr([])),
            ("present", Json::arr([])),
            ("error", Json::str(ctx.say(&said))),
        ]);
        println!("{}", o.pretty());
    } else {
        let h = if ctx.lang == ritsu_base::text::Lang::Ja { "エラー" } else { "error" };
        eprintln!("{h}: {}", ctx.say(&said));
    }
    code
}

/// What the members and the versions staged come to: the rules checked with cosign, the versions put
/// in the store, and the records that name a line newly checked written again. `bad` holds the
/// entries of the store whose files differ from the lock, taken away before the versions fetched
/// again are put.
fn apply(store: &Store, members: &[Member], staged: &[Staged], bad: &[String], reach_all: bool) -> Result<Done, Stop> {
    let mut keys = Keys::default();
    let applying = rules(members, staged, reach_all, &mut keys)?;
    // the lines each member has not been checked against
    let mut todo: Vec<Vec<Rule>> = vec![Vec::new(); members.len()];
    for (j, m) in members.iter().enumerate() {
        let Some((_, d, at)) = &m.pinned else { continue };
        let have: Vec<String> = at.as_ref().and_then(|s| s.record(d)).map(|r| r.verified.into_iter().map(|(p, _)| p).collect()).unwrap_or_default();
        todo[j] = applying[j].iter().filter(|r| !have.contains(&r.checked)).cloned().collect();
    }
    // the module's copy of the store at a git revision is the revision's, whose records are not
    // written: one that lacks a line stops the run before cosign is asked
    if let Some(rev) = fs::revision() {
        for (j, m) in members.iter().enumerate() {
            if let (true, Some((version, ..)), Some(r)) = (m.copy, &m.pinned, todo[j].first()) {
                let (name, line) = (&m.name, &r.line);
                return Err(Stop::Refused(tr!(
                    "git の版 `{rev}` の同梱のコピーにある `{name}` {version} の記録に、署名を `{line}` で確かめたことが書かれていません。git の版のファイルには書かないので、作業ツリーで `ritsu mod fetch` を走らせ、同梱のコピーをコミットしてください",
                    "the record of `{name}` {version} in this module's copy of the store at the git revision `{rev}` names no check of its signature against `{line}`, and the files of a revision are not written; run `ritsu mod fetch` in the working tree and commit the copy"
                )));
            }
        }
    }
    for (j, m) in members.iter().enumerate() {
        let Some((version, d, _)) = &m.pinned else { continue };
        let reference = format!("{}@{d}", m.name);
        for r in &todo[j] {
            match cosign::verify(r, &reference, registry::on_loopback(&m.name)) {
                Ok(()) => {}
                Err(cosign::Failed::Missing(t)) => return Err(Stop::Cannot(t)),
                Err(cosign::Failed::Refused(said)) => {
                    let (name, line) = (&m.name, &r.line);
                    return Err(Stop::Refused(tr!(
                        "`{name}` {version} の署名が `{line}` で確かめられません。cosign の出力：\n{said}",
                        "the signature of `{name}` {version} does not pass `{line}`; cosign said:\n{said}"
                    )));
                }
            }
        }
    }
    drop(keys);
    let today = store_write::today();
    let mut done = Done::default();
    for (j, m) in members.iter().enumerate() {
        let Some((version, d, at)) = &m.pinned else { continue };
        if let Some(k) = m.staged {
            let s = &staged[k];
            if bad.contains(&s.digest) {
                store_write::remove_entry(store, &s.digest).map_err(Stop::Cannot)?;
            }
            let record = Record {
                module: s.name.to_string(),
                version: s.version.to_string(),
                files: s.files_digest.clone(),
                fetched: today.clone(),
                verified: applying[j].iter().map(|r| (r.checked.clone(), today.clone())).collect(),
            };
            let parts = Parts { manifest: &s.manifest, module: &s.module, files: Some(&s.files), record };
            store::place(store, &parts, Some((&s.name, &s.version))).map_err(Stop::Cannot)?;
            done.fetched.push((m.name.to_string(), version.to_string(), d.clone(), applying[j].clone()));
        } else {
            if !todo[j].is_empty()
                && let Some(at) = at
            {
                let mut verified = at.record(d).map(|r| r.verified).unwrap_or_default();
                verified.extend(todo[j].iter().map(|r| (r.checked.clone(), today.clone())));
                store_write::set_verified(at, d, verified).map_err(Stop::Cannot)?;
            }
            done.present.push((m.name.to_string(), version.to_string(), d.clone(), todo[j].clone()));
        }
    }
    // a tree kept on this thread is kept while its files read the same, and the store is not among
    // them: one that stopped at what the store lacked is read again (`ritsu mod diff` and `report`
    // read the trees after they fetch)
    if !done.fetched.is_empty() || done.present.iter().any(|(.., rs)| !rs.is_empty()) {
        ritsu_base::modules::refer::forget();
    }
    Ok(done)
}

/// What a run that went through prints.
fn said(ctx: &Ctx, done: &Done) {
    if ctx.json() {
        let entry = |(m, v, d, rs): &(String, String, String, Vec<Rule>)| {
            Json::obj([
                ("module", Json::str(m)),
                ("version", Json::str(v)),
                ("digest", Json::str(d)),
                ("verified", Json::arr(rs.iter().map(|r| Json::str(&r.checked)))),
            ])
        };
        let o = Json::obj([
            ("ritsu", Json::str(env!("CARGO_PKG_VERSION"))),
            ("fetched", Json::arr(done.fetched.iter().map(entry))),
            ("present", Json::arr(done.present.iter().map(entry))),
            ("error", Json::Null),
        ]);
        println!("{}", o.pretty());
        return;
    }
    for l in done.lines(ctx.lang) {
        println!("{l}");
    }
    let (f, p) = (done.fetched.len(), done.present.len());
    let line = match (f, p) {
        (0, 0) => tr!("取り出すモジュールはありません", "there is no module to fetch"),
        (0, p) => {
            let modules = plural(p, "module", "modules");
            tr!("モジュールはどれもそろっています（{p} 個）", "every module is here already ({modules})")
        }
        (f, 0) => {
            let modules = plural(f, "module", "modules");
            tr!("モジュール {f} 個を取り出しました", "fetched {modules}")
        }
        (f, p) => {
            let modules = plural(f, "module", "modules");
            tr!("モジュール {f} 個を取り出しました（{p} 個はそろっていました）", "fetched {modules} ({p} here already)")
        }
    };
    println!("{}", ctx.say(&line));
}

/// Why the fetch of a tree ends: before it fetches anything, or stopped.
enum Halt {
    /// The module at hand's `ritsu.mod` cannot be read.
    NoMod(std::io::Error),
    /// Its `ritsu.mod` does not parse.
    Diags(Vec<Diag>),
    /// There is no lock, and a module from a registry needs one.
    NoLock,
    /// The lock cannot be read: what to say.
    Lock(Text),
    /// What stops the fetch.
    Stop(Stop),
}

/// The tree of the module at `root`, read through the files of this thread, and the modules its
/// lock pins fetched: what `ritsu mod fetch` does (with `--rev`, at a git revision), and what
/// [`tree`] does for `diff` and `report`. With no lock and no module from a registry, nothing is.
fn of_tree(root: &Path, store: &Store) -> Result<Done, Halt> {
    let main = fs::raw::read(&root.join(manifest::FILE)).map_err(Halt::NoMod)?;
    let main = manifest::parse(&main, manifest::FILE).map_err(Halt::Diags)?;
    let mut members = locals(root, main);
    let Ok(lock) = fs::raw::read(&root.join(lockfile::FILE)) else {
        return if needs_lock(&members) { Err(Halt::NoLock) } else { Ok(Done::default()) };
    };
    let lock: Lock = match lockfile::parse(&lock, lockfile::FILE) {
        Ok(l) => l,
        Err(lockfile::Error::Newer(n)) => return Err(Halt::Lock(lockfile::Error::Newer(n).text())),
        Err(lockfile::Error::Unreadable(_)) => {
            return Err(Halt::Lock(tr!("`ritsu.lock` を読めません。`ritsu mod lock` で書き直してください", "`ritsu.lock` cannot be read; run `ritsu mod lock` to write it again")));
        }
    };
    let (staged, bad) = gather(root, store, &mut members, &lock).map_err(Halt::Stop)?;
    apply(store, &members, &staged, &bad, false).map_err(Halt::Stop)
}

/// The modules from a registry the lock pins (the module at hand and the modules a path reaches left
/// out), each made a member: here (in the module's copy of the store, then the store, its files held
/// to the lock's digest), or staged from its registry. The versions staged, and the entries of the
/// store whose files differ from the lock.
fn gather(root: &Path, store: &Store, members: &mut Vec<Member>, lock: &Lock) -> Result<(Vec<Staged>, Vec<String>), Stop> {
    let local: BTreeSet<ModuleName> = members.iter().map(|m| m.name.clone()).collect();
    let vendored = Store::vendored(root);
    let mut staged: Vec<Staged> = Vec::new();
    let mut bad: Vec<String> = Vec::new();
    for l in &lock.modules {
        let Pinned::Oci { version, digest: d, files } = &l.pinned else { continue };
        if local.contains(&l.name) {
            continue;
        }
        let mut here = None;
        for s in [&vendored, store] {
            if !s.has_files(d) {
                continue;
            }
            if digest::files(&s.files(d)).ok().as_deref() == Some(files.as_str()) {
                here = Some(s.clone());
                break;
            }
            if *s == vendored {
                let name = &l.name;
                return Err(Stop::Refused(tr!(
                    "同梱のコピーにある `{name}` {version} のファイルが、`ritsu.lock` のダイジェストと違います。`ritsu mod vendor` で置き直してください",
                    "the files of `{name}` {version} in this module's copy of the store differ from the digest of `ritsu.lock`; run `ritsu mod vendor` to copy them again"
                )));
            }
            bad.push(d.clone());
        }
        if let Some(s) = here {
            let m = s.module_bytes(d).and_then(|b| manifest::parse(&b, manifest::FILE).ok());
            let copy = s == vendored;
            members.push(Member { name: l.name.clone(), manifest: m, dir: Some(s.files(d)), staged: None, pinned: Some((version.clone(), d.clone(), Some(s))), copy });
            continue;
        }
        let remote = Remote::of(&l.name, false).map_err(Stop::Cannot)?;
        let s = stage(&remote, &l.name, version, Some(d), Some((files, l.line)))?;
        let m = manifest::parse(&s.module, manifest::FILE).ok();
        members.push(Member { name: l.name.clone(), manifest: m, dir: None, staged: Some(staged.len()), pinned: Some((version.clone(), d.clone(), None)), copy: false });
        staged.push(s);
    }
    Ok((staged, bad))
}

/// `ritsu mod fetch [<name>@<version>...] [--rev <revision>]`: the exit code.
pub fn fetch(ctx: &Ctx, a: &Args) -> u8 {
    let Some(store) = Store::located() else {
        return ctx.refuse(tr!(
            "置き場の場所が分かりません（`RITSU_MODULES` も `HOME` もありません）",
            "there is no place for the store: neither `RITSU_MODULES` nor `HOME` is set"
        ));
    };
    if !a.pos.is_empty() {
        if a.get("--rev").is_some() {
            return ctx.refuse(tr!(
                "`--rev`（git の版の lock のモジュールを取り出す）は、版を挙げるときには付けられません",
                "`--rev` fetches the modules of that revision's lock, and goes with no version named"
            ));
        }
        return named(ctx, a, &store);
    }
    let root = match ctx.root() {
        Ok(r) => r,
        Err(t) => return ctx.refuse(t),
    };
    let rev = a.get("--rev");
    let run = match rev {
        // the tree as the revision has it: its files read with git, a path outside it from the disk
        Some(rev) => match GitRevision::open(&root, rev) {
            Ok(g) => fs::with(Rc::new(g), || of_tree(&root, &store)),
            Err(t) => return ctx.refuse(t),
        },
        None => of_tree(&root, &store),
    };
    match run {
        Ok(done) => {
            said(ctx, &done);
            0
        }
        Err(Halt::NoMod(e)) => ctx.refuse(match rev {
            Some(rev) => tr!("git の版 `{rev}` に、このモジュールの `ritsu.mod` がありません", "the git revision `{rev}` has no `ritsu.mod` of this module"),
            None => tr!("ritsu.mod を読めません（{e}）", "cannot read ritsu.mod ({e})"),
        }),
        Err(Halt::Diags(ds)) => super::stopped(ctx, &root, &Problem::Diags(ds)),
        Err(Halt::NoLock) => stopped(ctx, Some(&root), Stop::Cannot(tr!(
            "`ritsu.lock` がありません。レジストリから取る依存の版は lock で決まるので、`ritsu mod lock` を走らせてください",
            "there is no `ritsu.lock`: a dependency from a registry is fetched at the version the lock pins; run `ritsu mod lock`"
        ))),
        Err(Halt::Lock(t)) => stopped(ctx, Some(&root), Stop::Cannot(t)),
        Err(Halt::Stop(s)) => stopped(ctx, Some(&root), s),
    }
}

/// `ritsu mod fetch <name>@<version>...`: [`of_versions`] for the versions named.
fn named(ctx: &Ctx, a: &Args, store: &Store) -> u8 {
    let root = ctx.root().ok();
    let mut wanted: Vec<(ModuleName, Version)> = Vec::new();
    for arg in &a.pos {
        let Some((name, v)) = arg.rsplit_once('@') else {
            return ctx.refuse(tr!("`{arg}` に版がありません。`<名前>@<版>` で書いてください", "`{arg}` has no version; write `<name>@<version>`"));
        };
        let module = match registry::module_named(ctx, name) {
            Ok(m) => m,
            Err(t) => return ctx.refuse(t),
        };
        let version = match Version::parse(v) {
            Ok(x) => x,
            Err(e) => return ctx.refuse(e.text(v)),
        };
        wanted.push((module, version));
    }
    match of_versions(root.as_deref(), store, &wanted) {
        Ok(done) => {
            said(ctx, &done);
            0
        }
        Err(s) => stopped(ctx, root.as_deref(), s),
    }
}

/// The versions `wanted`, the `ritsu.mod` of every version the choice of the versions from them
/// reaches, and the files of the versions chosen (DESIGN 17.12); the lines of the module at `root`,
/// when there is one, apply too. No lock is written.
fn of_versions(root: Option<&Path>, store: &Store, wanted: &[(ModuleName, Version)]) -> Result<Done, Stop> {
    let writer = root.and_then(|r| fs::raw::read(&r.join(manifest::FILE)).ok().and_then(|b| manifest::parse(&b, manifest::FILE).ok()).map(|m| (m, r.to_path_buf())));
    of_versions_by(writer, store, wanted)
}

/// [`of_versions`], with the lines of `writer` (a `ritsu.mod` and the directory its key files are
/// read from) applying to every version.
fn of_versions_by(writer: Option<(Manifest, PathBuf)>, store: &Store, wanted: &[(ModuleName, Version)]) -> Result<Done, Stop> {
    let mut members: Vec<Member> = Vec::new();
    if let Some((m, dir)) = writer {
        members.push(Member { name: m.module.value.clone(), manifest: Some(m), dir: Some(dir), staged: None, pinned: None, copy: false });
    }
    // the choice of the versions from the versions named, each `ritsu.mod` from the store or fetched
    let picked = graph::mvs_try(wanted, |k, v| -> Result<Vec<(ModuleName, Version)>, Stop> {
        let d = match store.find(k, v).filter(|d| store.has(d)) {
            Some(d) => d,
            None => chosen(store, k, v)?,
        };
        let bytes = store.module_bytes(&d).ok_or_else(|| Stop::Cannot(tr!("置き場に `{k}` {v} の `ritsu.mod` がありません", "the store has no `ritsu.mod` of `{k}` {v}")))?;
        let m = read_mod(&bytes, k, v)?;
        if &m.module.value != k || m.version.as_ref().map(|x| &x.value) != Some(v) {
            let other = format!("{} {}", m.module.value, m.version.as_ref().map(|x| x.value.to_string()).unwrap_or_default());
            return Err(Stop::Refused(tr!("タグ `{k}` {v} の `ritsu.mod` は `{other}` です", "the `ritsu.mod` of the tag `{k}` {v} is `{other}`")));
        }
        let mut out = Vec::new();
        for r in &m.requires {
            match &r.source {
                Source::Oci { repository, min } => {
                    if let Ok(n) = ModuleName::parse(repository) {
                        out.push((n, min.clone()));
                    }
                }
                Source::Path { path } => {
                    let rn = &r.name;
                    return Err(Stop::Refused(tr!(
                        "レジストリから取ったモジュール `{k}` {v} が、`require {rn} path \"{path}\"` を持っています",
                        "the module `{k}` {v}, from a registry, has `require {rn} path \"{path}\"`"
                    )));
                }
            }
        }
        Ok(out)
    })?;
    let mut staged: Vec<Staged> = Vec::new();
    let mut bad: Vec<String> = Vec::new();
    for (k, v) in &picked {
        let Some(d) = store.find(k, v) else {
            return Err(Stop::Cannot(tr!("置き場の版の索引に `{k}` {v} がありません", "the store's index of versions has no `{k}` {v}")));
        };
        let files = store.record(&d).map(|r| r.files);
        if store.has_files(&d) {
            if digest::files(&store.files(&d)).ok() == files {
                let m = store.module_bytes(&d).and_then(|b| manifest::parse(&b, manifest::FILE).ok());
                members.push(Member { name: k.clone(), manifest: m, dir: Some(store.files(&d)), staged: None, pinned: Some((v.clone(), d.clone(), Some(store.clone()))), copy: false });
                continue;
            }
            bad.push(d.clone());
        }
        let remote = Remote::of(k, false).map_err(Stop::Cannot)?;
        let s = stage(&remote, k, v, Some(&d), None)?;
        if files.as_deref().is_some_and(|f| f != s.files_digest) {
            return Err(Stop::Refused(tr!("`{k}` {v} のファイルのダイジェストが、置き場の記録と違います", "the digest of the files of `{k}` {v} differs from the store's record")));
        }
        let m = manifest::parse(&s.module, manifest::FILE).ok();
        members.push(Member { name: k.clone(), manifest: m, dir: None, staged: Some(staged.len()), pinned: Some((v.clone(), d.clone(), None)), copy: false });
        staged.push(s);
    }
    apply(store, &members, &staged, &bad, true)
}

/// The modules the tree of the module at `root` pins, read through the files of this thread (the
/// working tree, a git revision, the module at hand with another lock), fetched as `ritsu mod
/// fetch` fetches them: what `ritsu mod diff` and `ritsu mod report` read, fetched before they read
/// it (DESIGN 17.9.6, 17.10). Nothing is asked of a registry or of cosign when every module is here
/// and its record names every line that applies. A tree that cannot be read (no `ritsu.mod`, one
/// that does not parse, no lock, a lock that cannot be read), and a run with no place for the store,
/// are left to what reads the tree after, which says why. Err, with what stops the fetch (exit 2).
pub fn tree(root: &Path) -> Result<Done, Text> {
    let Some(store) = Store::located() else { return Ok(Done::default()) };
    match of_tree(root, &store) {
        Ok(done) => Ok(done),
        Err(Halt::Stop(s)) => Err(s.text()),
        Err(Halt::NoMod(_) | Halt::Diags(_) | Halt::NoLock | Halt::Lock(_)) => Ok(Done::default()),
    }
}

/// The version `v` of `name`, and what the choice of the versions from it reaches, fetched as `ritsu
/// mod fetch <name>@<version>` fetches them, for `ritsu mod diff` (DESIGN 17.9.6); the lines of the
/// module at `root`, when there is one, apply too. Err, with what stops the fetch (exit 2).
pub fn version(root: Option<&Path>, name: &ModuleName, v: &Version) -> Result<Done, Text> {
    let Some(store) = Store::located() else {
        return Err(tr!(
            "`{name}` {v} を取り出す置き場の場所が分かりません（`RITSU_MODULES` も `HOME` もありません）",
            "there is no place for the store to fetch `{name}` {v} into: neither `RITSU_MODULES` nor `HOME` is set"
        ));
    };
    of_versions(root, &store, &[(name.clone(), v.clone())]).map_err(Stop::text)
}

/// The version `version` of `name` for a list of modules (`ritsu mod site`, DESIGN 17.17): fetched
/// as `ritsu mod fetch <name>@<version>` fetches it, with what the choice of the versions from it
/// reaches, each checked against the list's `signature` lines (`lines`, whose key files are read
/// from `dir`, the list's directory); then the modules its own lock pins, fetched and checked against
/// its own lines, as `ritsu mod diff` reads a version (its files are read as a module of their own,
/// with that lock). The store, and the digest of the version's OCI manifest; or the exit code (1:
/// what the registry holds is not what it has to be, or a signature does not pass; 2: the registry
/// or cosign cannot be asked) and what stops it.
pub fn listed(dir: &Path, lines: &[Signature], name: &ModuleName, version: &Version) -> Result<(Store, String), (u8, Text)> {
    let Some(store) = Store::located() else {
        return Err((2, tr!(
            "`{name}` {version} を取り出す置き場の場所が分かりません（`RITSU_MODULES` も `HOME` もありません）",
            "there is no place for the store to fetch `{name}` {version} into: neither `RITSU_MODULES` nor `HOME` is set"
        )));
    };
    let code = |s: Stop| -> (u8, Text) {
        match s {
            Stop::Cannot(t) => (2, t),
            other => (1, other.text()),
        }
    };
    // the list's lines, as the lines of a module of no name that applies them to every version
    let text: String = std::iter::once("module site\n".to_string()).chain(lines.iter().map(|l| format!("{}\n", l.text()))).collect();
    let writer = manifest::parse(text.as_bytes(), manifest::FILE).map_err(|ds| (2, ds.first().map(|d| d.message.clone()).unwrap_or_default()))?;
    of_versions_by(Some((writer, dir.to_path_buf())), &store, &[(name.clone(), version.clone())]).map_err(code)?;
    let Some(d) = store.find(name, version) else {
        return Err((2, tr!("置き場の版の索引に `{name}` {version} がありません", "the store's index of versions has no `{name}` {version}")));
    };
    match of_tree(&store.files(&d), &store) {
        Err(Halt::Stop(s)) => Err(code(s)),
        // what the version's own tree says of itself is said where its files are read
        _ => Ok((store, d)),
    }
}

/// The version `version` of the module `name`, which the choice of the versions reaches (DESIGN
/// 17.7) and which is in neither the store nor the module's copy: its tag looked up, its OCI
/// manifest checked, its `ritsu.mod` layer fetched and held to its digest, and the entry put in
/// `store` without its files (DESIGN 17.11, 17.15), with the version's line of the index. The digest
/// of its OCI manifest, from which the store gives the `ritsu.mod` and the record; or what stops the
/// command (exit 2).
pub fn for_choosing(store: Option<&Store>, name: &ModuleName, version: &Version) -> Result<String, Text> {
    let Some(store) = store else {
        return Err(tr!(
            "依存 `{name}` {version} を取り出す置き場の場所が分かりません（`RITSU_MODULES` も `HOME` もありません）",
            "there is no place for the store to fetch the dependency `{name}` {version} into: neither `RITSU_MODULES` nor `HOME` is set"
        ));
    };
    chosen(store, name, version).map_err(Stop::text)
}

/// [`for_choosing`], with what stops it told apart: what the registry answered is not what it has
/// to be (exit 1), or the registry cannot be asked (exit 2).
fn chosen(store: &Store, name: &ModuleName, version: &Version) -> Result<String, Stop> {
    let remote = Remote::of(name, false).map_err(Stop::Cannot)?;
    let Some(bytes) = remote.manifest(&version.to_string()).map_err(|n| unanswered(n, |t| tr!("`{name}` {version}: ", "`{name}` {version}: ").then(&t)))? else {
        return Err(Stop::Cannot(tr!("レジストリに `{name}` の版 {version} がありません", "the registry has no version {version} of `{name}`")));
    };
    let layers = artifact::check(&bytes).map_err(|t| Stop::Refused(tr!("`{name}` {version}: ", "`{name}` {version}: ").then(&t)))?;
    let module = remote
        .blob(&layers.module_digest, layers.module_size)
        .map_err(|n| unanswered(n, |_| over_layer(name, version, artifact::MOD_TYPE, &layers.module_digest, layers.module_size)))?;
    if digest::of(&module) != layers.module_digest || module.len() as u64 != layers.module_size {
        let want = &layers.module_digest;
        return Err(Stop::Refused(tr!(
            "`{name}` {version} の `ritsu.mod` の層のダイジェストが、OCI のマニフェストが書く {want} ではありません",
            "the `ritsu.mod` layer of `{name}` {version} does not have the digest {want} its OCI manifest names"
        )));
    }
    let record = Record { module: name.to_string(), version: version.to_string(), files: layers.files_digest.clone(), fetched: store_write::today(), verified: Vec::new() };
    store::place(store, &Parts { manifest: &bytes, module: &module, files: None, record }, Some((name, version))).map_err(Stop::Cannot)
}
