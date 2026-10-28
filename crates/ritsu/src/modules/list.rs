//! `ritsu mod list` and `ritsu mod where` (DESIGN 17.19, 17.21).
//!
//! `list` reads the modules of the tree from the manifests and the lock, not from the files of the
//! modules from a registry: a module of the lock whose files are neither in the store nor in the
//! module's copy is listed with `present: false`, which is what the list is for. The tree is read as
//! `ritsu check` reads it as well, for its diagnostics (a lock that does not match, E304; a line the
//! tree does not reach, W302), which the list says after the modules (exit 1 for an error).

use super::Ctx;
use ritsu_base::cli::Args;
use ritsu_base::diag::{Diag, Severity};
use ritsu_base::json::Json;
use ritsu_base::modules::graph::{self, Problem};
use ritsu_base::modules::lock::{self as lockfile, Lock, Pinned};
use ritsu_base::modules::manifest::{self, Manifest, Source};
use ritsu_base::modules::names::ModuleName;
use ritsu_base::modules::store::Store;
use ritsu_base::modules::vendor::{self, Where};
use ritsu_base::text::{Text, plural};
use ritsu_base::tr;
use std::collections::{BTreeMap, VecDeque};
use std::path::{Component, Path, PathBuf};

/// A module of the tree, as the list says it.
struct Listed {
    name: ModuleName,
    manifest: Option<Manifest>,
    version: Option<String>,
    /// For a module a path reaches: its directory, and the path to it from the root.
    dir: Option<PathBuf>,
    path: Option<String>,
    /// For a module from a registry: its two digests, from the lock.
    digests: Option<(String, String)>,
    prefix: Option<String>,
    required_by: Vec<(String, String, Option<String>)>,
    present: bool,
    at: Option<Where>,
    verified: Vec<String>,
}

/// A path with its `.` and `..` folded by their letters.
fn fold(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn manifest_at(file: &Path) -> Option<Manifest> {
    manifest::parse(&std::fs::read(file).ok()?, manifest::FILE).ok()
}

/// The modules of the tree whose root is `root`, the module at hand first, each with what the
/// manifests and the lock say of it, in the order of the walk of their prefixes (DESIGN 17.6), as
/// the tree orders them; a module no walk reaches keeps the order it was found in.
fn listing(root: &Path, main: Manifest, lock: Option<&Lock>, store: Option<&Store>) -> Vec<Listed> {
    let mut nodes = vec![Listed {
        name: main.module.value.clone(),
        version: main.version.as_ref().map(|v| v.value.to_string()),
        manifest: Some(main),
        dir: Some(root.to_path_buf()),
        path: Some(".".to_string()),
        digests: None,
        prefix: Some(String::new()),
        required_by: Vec::new(),
        present: true,
        at: Some(Where::Here),
        verified: Vec::new(),
    }];
    let mut by_name: BTreeMap<ModuleName, usize> = BTreeMap::from([(nodes[0].name.clone(), 0)]);
    let target = |nodes: &[Listed], i: usize, path: &str| -> (PathBuf, String) {
        let rel = graph::up_join(nodes[i].path.as_deref().unwrap_or("."), path);
        (fold(&root.join(&rel)), rel)
    };
    // the modules the paths reach, then those of a registry, as the tree reads them: a module a path
    // reaches is read from the path, whatever a registry would give
    let mut queue = VecDeque::from([0usize]);
    while let Some(i) = queue.pop_front() {
        let reqs = nodes[i].manifest.as_ref().map(|m| m.requires.clone()).unwrap_or_default();
        for r in reqs {
            let Source::Path { path } = &r.source else { continue };
            let (dir, rel) = target(&nodes, i, path);
            let Some(m) = manifest_at(&dir.join(manifest::FILE)) else { continue };
            if by_name.contains_key(&m.module.value) {
                continue;
            }
            by_name.insert(m.module.value.clone(), nodes.len());
            queue.push_back(nodes.len());
            nodes.push(Listed {
                name: m.module.value.clone(),
                version: m.version.as_ref().map(|v| v.value.to_string()),
                manifest: Some(m),
                dir: Some(dir),
                path: Some(rel),
                digests: None,
                prefix: None,
                required_by: Vec::new(),
                present: true,
                at: Some(Where::Path),
                verified: Vec::new(),
            });
        }
    }
    let mut queue: VecDeque<usize> = (0..nodes.len()).collect();
    while let Some(i) = queue.pop_front() {
        let reqs = nodes[i].manifest.as_ref().map(|m| m.requires.clone()).unwrap_or_default();
        for r in reqs {
            let Source::Oci { repository, .. } = &r.source else { continue };
            let Ok(name) = ModuleName::parse(repository) else { continue };
            if by_name.contains_key(&name) {
                continue;
            }
            let pinned = lock.and_then(|l| l.get(&name)).and_then(|l| match &l.pinned {
                Pinned::Oci { version, digest, files } => Some((version.to_string(), digest.clone(), files.clone())),
                Pinned::Path { .. } => None,
            });
            let (version, digests, found, m, verified) = match &pinned {
                Some((v, d, f)) => {
                    let found = vendor::find(root, d, store);
                    // the files' `ritsu.mod` when the files are here; else the first layer, which an
                    // entry fetched to choose the versions by has too
                    let m = match &found {
                        Some((s, _)) => manifest_at(&s.files(d).join(manifest::FILE)),
                        None => [Some(Store::vendored(root)), store.cloned()].into_iter().flatten().find_map(|s| s.module_bytes(d)).and_then(|b| manifest::parse(&b, manifest::FILE).ok()),
                    };
                    let verified = found.as_ref().and_then(|(s, _)| s.record(d)).map(|r| r.verified.into_iter().map(|(p, _)| p).collect()).unwrap_or_default();
                    (Some(v.clone()), Some((d.clone(), f.clone())), found, m, verified)
                }
                None => (None, None, None, None, Vec::new()),
            };
            by_name.insert(name.clone(), nodes.len());
            queue.push_back(nodes.len());
            nodes.push(Listed {
                name,
                manifest: m,
                version,
                dir: None,
                path: None,
                digests,
                prefix: None,
                required_by: Vec::new(),
                present: found.is_some(),
                at: found.map(|(_, w)| w),
                verified,
            });
        }
    }
    // each `require`, to the module it reaches, in the order of the `require`s
    let mut deps: Vec<Vec<(String, usize)>> = vec![Vec::new(); nodes.len()];
    for i in 0..nodes.len() {
        let Some(m) = &nodes[i].manifest else { continue };
        for r in &m.requires {
            let j = match &r.source {
                Source::Path { path } => {
                    let (dir, _) = target(&nodes, i, path);
                    nodes.iter().position(|n| n.dir.as_ref() == Some(&dir))
                }
                Source::Oci { repository, .. } => ModuleName::parse(repository).ok().and_then(|n| by_name.get(&n).copied()),
            };
            if let Some(j) = j {
                deps[i].push((r.name.clone(), j));
            }
        }
    }
    for (i, ds) in deps.iter().enumerate() {
        for (name, j) in ds {
            let by = (nodes[i].name.to_string(), name.clone(), nodes[i].version.clone());
            nodes[*j].required_by.push(by);
        }
    }
    // the prefixes, by the walk from the module at hand
    let mut order = vec![0usize];
    let mut at = 0;
    while at < order.len() {
        let i = order[at];
        at += 1;
        for (name, j) in &deps[i] {
            if nodes[*j].prefix.is_none() {
                let p = nodes[i].prefix.clone().unwrap_or_default();
                nodes[*j].prefix = Some(if p.is_empty() { format!("@{name}") } else { format!("{p}/@{name}") });
                order.push(*j);
            }
        }
    }
    let unreached: Vec<usize> = (0..nodes.len()).filter(|i| !order.contains(i)).collect();
    order.extend(unreached);
    let mut slots: Vec<Option<Listed>> = nodes.into_iter().map(Some).collect();
    order.into_iter().filter_map(|i| slots[i].take()).collect()
}

/// The JSON of a module of the list (DESIGN 17.21).
fn module_json(m: &Listed) -> Json {
    let (digest, files) = match &m.digests {
        Some((d, f)) => (Some(d.as_str()), Some(f.as_str())),
        None => (None, None),
    };
    Json::obj([
        ("name", Json::str(m.name.to_string())),
        ("version", Json::opt_str(m.version.clone())),
        ("source", Json::str(if m.path.is_some() { "path" } else { "oci" })),
        ("path", Json::opt_str(m.path.clone())),
        ("digest", Json::opt_str(digest)),
        ("files_digest", Json::opt_str(files)),
        ("prefix", Json::opt_str(m.prefix.clone())),
        (
            "required_by",
            Json::arr(m.required_by.iter().map(|(module, name, version)| Json::obj([("module", Json::str(module)), ("name", Json::str(name)), ("version", Json::opt_str(version.clone()))]))),
        ),
        ("present", Json::Bool(m.present)),
        ("where", Json::opt_str(m.at.map(Where::word))),
        ("verified", Json::arr(m.verified.iter().map(Json::str))),
    ])
}

/// Where a module of the list is, as the text says it.
fn place_text(m: &Listed) -> Text {
    match (&m.path, m.at) {
        (Some(p), _) => tr!("パス {p}", "path {p}"),
        (None, Some(Where::Vendor)) => tr!("レジストリ、同梱のコピー", "registry, in this module's copy"),
        (None, Some(_)) => tr!("レジストリ、置き場", "registry, in the store"),
        (None, None) if m.version.is_none() => tr!("レジストリ、lock に無い", "registry, not in the lock"),
        (None, None) => tr!("レジストリ、取り出していない", "registry, not fetched"),
    }
}

fn pad(s: &str, w: usize) -> String {
    format!("{s}{}", " ".repeat(w.saturating_sub(s.chars().count())))
}

/// `ritsu mod list [--format text|json]`: the exit code.
pub fn list(ctx: &Ctx, _a: &Args) -> u8 {
    let root = match ctx.root() {
        Ok(r) => r,
        Err(t) => return ctx.refuse(t),
    };
    let store = Store::located();
    let main = match std::fs::read(root.join(manifest::FILE)).ok().map(|b| manifest::parse(&b, manifest::FILE)) {
        Some(Ok(m)) => m,
        Some(Err(ds)) => return super::stopped(ctx, &root, &Problem::Diags(ds)),
        None => return ctx.refuse(tr!("ritsu.mod を読めません", "cannot read ritsu.mod")),
    };
    let lock = match std::fs::read(root.join(lockfile::FILE)).ok().map(|b| lockfile::parse(&b, lockfile::FILE)) {
        Some(Ok(l)) => Some(l),
        Some(Err(lockfile::Error::Newer(n))) => return ctx.refuse(lockfile::Error::Newer(n).text()),
        // the tree says what is wrong with it
        Some(Err(lockfile::Error::Unreadable(_))) | None => None,
    };
    // the tree, read as `ritsu check` reads it, for its diagnostics; what stops it is said as notes,
    // since the list says what is missing
    let (said, notes): (Vec<Diag>, Vec<Text>) = match graph::load_with(&root, store.as_ref()).0 {
        Ok(t) => (t.found, Vec::new()),
        Err(Problem::Diags(ds)) => (ds, Vec::new()),
        Err(Problem::Stop(ts)) => (Vec::new(), ts),
    };
    let found = ctx.findings(&root, &said);
    let errors = found.iter().filter(|f| f.severity == Severity::Error).count();
    let module = (main.module.value.to_string(), main.version.as_ref().map(|v| v.value.to_string()));
    let all = listing(&root, main, lock.as_ref(), store.as_ref());
    let deps = &all[1..];
    if ctx.json() {
        let o = Json::obj([
            ("ritsu", Json::str(env!("CARGO_PKG_VERSION"))),
            ("root", Json::str(ctx.shown(&root))),
            ("module", Json::obj([("name", Json::str(&module.0)), ("version", Json::opt_str(module.1.clone()))])),
            ("lock", match &lock {
                Some(l) => Json::obj([("lock", Json::int(lockfile::FORMAT)), ("ritsu", Json::str(l.ritsu.to_string()))]),
                None => Json::Null,
            }),
            ("modules", Json::arr(deps.iter().map(module_json))),
            ("diagnostics", Json::arr(found.iter().map(|f| ritsu::check::entry("ritsu", f)))),
        ]);
        println!("{}", o.pretty());
        return u8::from(errors > 0);
    }
    let head = match &module.1 {
        Some(v) => format!("{} {v}", module.0),
        None => module.0.clone(),
    };
    let mut out = ctx.say(&tr!("モジュール {head}\n", "module {head}\n"));
    let cells: Vec<(String, String, String)> = deps
        .iter()
        .map(|m| {
            let what = match &m.version {
                Some(v) => format!("{} {v}", m.name),
                None => m.name.to_string(),
            };
            (m.prefix.clone().unwrap_or_else(|| "-".to_string()), what, ctx.say(&place_text(m)))
        })
        .collect();
    let (w0, w1) = (cells.iter().map(|c| c.0.chars().count()).max().unwrap_or(0), cells.iter().map(|c| c.1.chars().count()).max().unwrap_or(0));
    for (p, what, place) in &cells {
        out.push_str(&format!("{}  {}  {place}\n", pad(p, w0), pad(what, w1)));
    }
    for f in &found {
        out.push_str(&f.text_of("ritsu"));
    }
    let n = deps.len();
    let by_path = deps.iter().filter(|m| m.path.is_some()).count();
    let from_registry = n - by_path;
    let missing = deps.iter().filter(|m| !m.present).count();
    let dependencies = plural(n, "dependency", "dependencies");
    let mut t = tr!(
        "ritsu mod list: 依存 {n} 個（パス {by_path} 個、レジストリ {from_registry} 個）",
        "ritsu mod list: {dependencies} ({by_path} by a path, {from_registry} from a registry)"
    );
    t = t.then(&match (&lock, from_registry) {
        (Some(l), _) => {
            let v = l.ritsu.to_string();
            tr!("。ritsu.lock は ritsu {v} が書きました", "; ritsu.lock written by ritsu {v}")
        }
        (None, 0) => tr!("。lock は要りません", "; no lock is needed"),
        (None, _) => tr!("。ritsu.lock がありません", "; there is no ritsu.lock"),
    });
    if missing > 0 {
        t = t.then(&tr!("。取り出していないもの {missing} 個", "; {missing} not fetched"));
    }
    if errors > 0 {
        let e = plural(errors, "error", "errors");
        t = t.then(&tr!("。エラー {errors} 件", "; {e}"));
    }
    out.push_str(&ctx.say(&t));
    out.push('\n');
    print!("{out}");
    let head = ctx.say(&tr!("備考", "note"));
    for n in notes {
        eprintln!("{head}: {}", ctx.say(&n));
    }
    u8::from(errors > 0)
}

/// `ritsu mod where <@path>`: where a path that starts with `@` is on the disk, read from the root
/// of the module as a path written in a file there is (DESIGN 17.6). A file of the store is written
/// whole; any other from where ritsu runs. The exit code.
pub fn where_is(ctx: &Ctx, a: &Args) -> u8 {
    let p = a.pos[0].as_str();
    if !p.starts_with('@') {
        return ctx.refuse(tr!(
            "`{p}` は `@` で始まりません。`ritsu mod where` が言うのは、依存を `@<名前>/` で指すパスの場所です",
            "`{p}` does not start with `@`; `ritsu mod where` says where a path that names a dependency by `@<name>/` is"
        ));
    }
    let root = match ctx.root() {
        Ok(r) => r,
        Err(t) => return ctx.refuse(t),
    };
    let view = match ritsu_base::modules::refer(&root, p) {
        Ok(v) => v,
        Err(r) => return ctx.refuse(r.text()),
    };
    if !ritsu_base::fs::exists(&view) {
        return ctx.refuse(ritsu_base::modules::missing(&view).unwrap_or_else(|| tr!("`{p}` がありません", "`{p}` is not there")));
    }
    let Some(real) = ritsu_base::modules::map(&view) else {
        return ctx.refuse(tr!("`{p}` は依存を指していません", "`{p}` names no dependency"));
    };
    let real = ritsu_base::fs::canonicalize(&real).unwrap_or(real);
    let store = Store::located().and_then(|s| ritsu_base::fs::canonicalize(&s.root).ok());
    if store.is_some_and(|s| real.starts_with(&s)) {
        println!("{}", real.display());
    } else {
        println!("{}", ctx.shown(&real));
    }
    0
}
