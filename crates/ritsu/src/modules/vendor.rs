//! `ritsu mod vendor [--check]` and `ritsu mod clean [--keep <lock>…] [--dry-run]` (DESIGN 17.15):
//! the module's copy of the store, made from the store, and the store's entries removed.
//!
//! `vendor` puts each module of the lock that comes from a registry or a git repository in
//! `.ritsu/modules/` in the store's shape (no `refs`, no `git`, no `tmp`), from the store, checked against the lock's digest of its
//! files first; an entry there that the lock does not pin is removed, so that the copy holds the
//! lock and nothing else. Nothing is written until every module is found. `--check` writes nothing,
//! and says each module that is missing or differs, and each entry the lock does not pin (exit 1).
//!
//! `clean` removes the entries of the store, and the lines of its index of versions and of its git
//! index that lead to them; with `--keep`, those no lock given pins. Its `tmp/` is left alone: a fetch run at the same
//! time may be making an entry there.

use super::Ctx;
use ritsu_base::cli::Args;
use ritsu_base::modules::digest;
use ritsu_base::modules::graph::{self, Problem};
use ritsu_base::modules::lock::{self as lockfile, Lock};
use ritsu_base::modules::store::{self, Parts, Store};
use ritsu_base::text::{Text, plural};
use ritsu_base::tr;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// A module of the lock from a registry or a git repository: its name and version as the text says
/// them, and its two digests.
struct Pin {
    what: String,
    digest: String,
    files: String,
}

fn pins(lock: &Lock) -> Vec<Pin> {
    lock.modules
        .iter()
        .filter_map(|l| l.pinned.digests().map(|(version, digest, files)| Pin { what: format!("{} {version}", l.name), digest: digest.to_string(), files: files.to_string() }))
        .collect()
}

/// The digests of the entries of a store (`sha256/<hex>/`), in the order of their names.
fn entries(s: &Store) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(s.root.join("sha256")) else { return Vec::new() };
    let mut out: Vec<String> = rd.flatten().filter(|e| e.path().is_dir()).filter_map(|e| e.file_name().to_str().map(|n| format!("sha256:{n}"))).filter(|d| digest::is_digest(d)).collect();
    out.sort();
    out
}

/// Whether the files of an entry are those of the digest `files`.
fn holds(s: &Store, d: &str, files: &str) -> bool {
    s.has_files(d) && digest::files(&s.files(d)).ok().as_deref() == Some(files)
}

/// The lock of the module at hand: none when there is none, and what stops the command when it
/// cannot be read.
fn read_lock(ctx: &Ctx, root: &Path) -> Result<Option<Lock>, u8> {
    let Ok(bytes) = std::fs::read(root.join(lockfile::FILE)) else { return Ok(None) };
    match lockfile::parse(&bytes, lockfile::FILE) {
        Ok(l) => Ok(Some(l)),
        Err(e) => Err(ctx.refuse(e.text())),
    }
}

/// `ritsu mod vendor [--check]`: the exit code.
pub fn vendor(ctx: &Ctx, a: &Args) -> u8 {
    let root = match ctx.root() {
        Ok(r) => r,
        Err(t) => return ctx.refuse(t),
    };
    let check = a.has("--check");
    let own = Store::vendored(&root);
    let lock = match read_lock(ctx, &root) {
        Ok(l) => l,
        Err(code) => return code,
    };
    let pinned = match &lock {
        Some(l) => pins(l),
        None => {
            // no lock: none is needed when no dependency comes from a registry or a git repository
            match graph::choose(&root, |_, _, _| Err(Problem::Stop(Vec::new()))) {
                Ok(_) => Vec::new(),
                Err(Problem::Stop(_)) => return ctx.refuse(tr!("ritsu.lock がありません。`ritsu mod lock` を走らせてください", "there is no ritsu.lock; run `ritsu mod lock`")),
                Err(p) => return super::stopped(ctx, &root, &p),
            }
        }
    };
    let wanted: BTreeSet<&str> = pinned.iter().map(|p| p.digest.as_str()).collect();
    let extra: Vec<String> = entries(&own).into_iter().filter(|d| !wanted.contains(d.as_str())).collect();
    if pinned.is_empty() && extra.is_empty() {
        let head = if check { "ritsu mod vendor --check" } else { "ritsu mod vendor" };
        println!(
            "{}",
            ctx.say(&tr!(
                "{head}: レジストリからも git のリポジトリからも取る依存が無いので、.ritsu/modules に置くものはありません",
                "{head}: no dependency comes from a registry or a git repository, so .ritsu/modules holds nothing"
            ))
        );
        return 0;
    }
    if check {
        let mut wrong: Vec<Text> = Vec::new();
        for p in &pinned {
            let what = &p.what;
            if !own.has_files(&p.digest) {
                wrong.push(tr!("{what} がありません", "missing: {what}"));
            } else if !holds(&own, &p.digest, &p.files) {
                wrong.push(tr!("{what} が lock と違います", "differs from the lock: {what}"));
            }
        }
        for d in &extra {
            wrong.push(tr!("{d} は lock にありません", "not in the lock: {d}"));
        }
        for w in &wrong {
            println!("{}", ctx.say(w));
        }
        let n = pinned.len();
        let modules = plural(n, "module", "modules");
        if wrong.is_empty() {
            println!("{}", ctx.say(&tr!("ritsu mod vendor --check: .ritsu/modules は lock に合います（モジュール {n} 個）", "ritsu mod vendor --check: .ritsu/modules matches the lock ({modules})")));
            return 0;
        }
        println!(
            "{}",
            ctx.say(&tr!(
                "ritsu mod vendor --check: .ritsu/modules が lock に合いません。`ritsu mod vendor` を走らせてください",
                "ritsu mod vendor --check: .ritsu/modules does not match the lock; run `ritsu mod vendor`"
            ))
        );
        return 1;
    }
    // every module found first: nothing is written when one is missing
    let store = Store::located();
    let mut to_place: Vec<&Pin> = Vec::new();
    for p in &pinned {
        if holds(&own, &p.digest, &p.files) {
            continue;
        }
        let what = &p.what;
        match &store {
            Some(s) if s.has_files(&p.digest) => {
                if !holds(s, &p.digest, &p.files) {
                    return ctx.refuse(tr!(
                        "置き場にある `{what}` のファイルが、`ritsu.lock` のダイジェストと違います。`ritsu mod clean` で消してから `ritsu mod fetch` で取り直してください",
                        "the files of `{what}` in the store differ from the digest of `ritsu.lock`; run `ritsu mod clean`, then `ritsu mod fetch` to fetch them again"
                    ));
                }
                to_place.push(p);
            }
            _ => {
                return ctx.refuse(tr!("`{what}` が置き場にありません。`ritsu mod fetch` を走らせてください", "`{what}` is not in the store; run `ritsu mod fetch`"));
            }
        }
    }
    let Some(s) = &store else { return finish(ctx, &own, &extra, 0) };
    for p in &to_place {
        if let Err(t) = place(s, &own, p) {
            return ctx.refuse(t);
        }
    }
    finish(ctx, &own, &extra, to_place.len())
}

/// The entry of `p` placed in the module's copy, from the store `s`, as a fetch places one in the
/// store; a broken one there removed first.
fn place(s: &Store, own: &Store, p: &Pin) -> Result<(), Text> {
    let d = &p.digest;
    let unreadable = || {
        let w = &p.what;
        tr!("置き場の `{w}` の項目を読めません", "cannot read the entry of `{w}` in the store")
    };
    if own.has(d) {
        store::remove(&own.entry(d)).map_err(|e| tr!("{d} を消せません（{e}）", "cannot remove {d} ({e})"))?;
    }
    let manifest = std::fs::read(s.entry(d).join("manifest.json")).map_err(|_| unreadable())?;
    let module = s.module_bytes(d).ok_or_else(unreadable)?;
    let files = digest::files_in(&s.files(d))?;
    let record = s.record(d).ok_or_else(unreadable)?;
    let placed = store::place(own, &Parts { manifest: &manifest, module: &module, files: Some(&files), record }, None)?;
    if placed != *d {
        let w = &p.what;
        return Err(tr!("置き場の `{w}` の OCI のマニフェストが、ダイジェストと違います", "the OCI manifest of `{w}` in the store differs from its digest"));
    }
    Ok(())
}

/// The entries the lock does not pin removed from the module's copy, and its `tmp/` (which placing
/// makes): what `vendor` says, and the exit code.
fn finish(ctx: &Ctx, own: &Store, extra: &[String], placed: usize) -> u8 {
    for d in extra {
        if let Err(e) = store::remove(&own.entry(d)) {
            return ctx.refuse(tr!("{d} を消せません（{e}）", "cannot remove {d} ({e})"));
        }
    }
    let _ = std::fs::remove_dir_all(own.root.join("tmp"));
    let n = entries(own).len();
    let removed = extra.len();
    let modules = plural(n, "module", "modules");
    println!(
        "{}",
        ctx.say(&tr!(
            "ritsu mod vendor: .ritsu/modules に lock のモジュール {n} 個があります（置いたもの {placed} 個、消したもの {removed} 個）",
            "ritsu mod vendor: .ritsu/modules holds the {modules} of the lock ({placed} placed, {removed} removed)"
        ))
    );
    0
}

/// The files under `dir`, every level down.
fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files_under(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// The directories under `dir` that hold nothing, removed, the deepest first.
fn prune(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            prune(&p);
            let _ = std::fs::remove_dir(&p);
        }
    }
}

/// `ritsu mod clean [--keep <lock>…] [--dry-run]`: the exit code.
pub fn clean(ctx: &Ctx, a: &Args) -> u8 {
    let Some(s) = Store::located() else {
        return ctx.refuse(tr!(
            "置き場の場所が分かりません。RITSU_MODULES か HOME を設定してください",
            "the place of the store is not known; set RITSU_MODULES, or HOME"
        ));
    };
    let mut keep: BTreeSet<String> = BTreeSet::new();
    for k in a.all("--keep") {
        let read = std::fs::read(k).map_err(|e| e.to_string()).and_then(|b| lockfile::parse(&b, k).map_err(|e| e.text().get(ctx.lang).to_string()));
        match read {
            Ok(l) => keep.extend(pins(&l).into_iter().map(|p| p.digest)),
            Err(e) => return ctx.refuse(tr!("--keep の `{k}` を lock として読めません（{e}）", "cannot read `{k}`, given with --keep, as a lock ({e})")),
        }
    }
    let dry = a.has("--dry-run");
    let (gone, kept): (Vec<String>, Vec<String>) = entries(&s).into_iter().partition(|d| !keep.contains(d));
    for d in &gone {
        let what = s.record(d).map(|r| format!("{} {}", r.module, r.version)).unwrap_or_else(|| "-".to_string());
        if !dry && let Err(e) = store::remove(&s.entry(d)) {
            return ctx.refuse(tr!("{d} を消せません（{e}）", "cannot remove {d} ({e})"));
        }
        let line = if dry { tr!("{d}（{what}）を消すことになります", "would remove {d} ({what})") } else { tr!("{d}（{what}）を消しました", "removed {d} ({what})") };
        println!("{}", ctx.say(&line));
    }
    if !dry {
        // the lines of the index of versions, and of the git index (`<commit> <digest>`), whose entry
        // is gone
        for index in ["refs", "git"] {
            let mut lines = Vec::new();
            files_under(&s.root.join(index), &mut lines);
            for f in lines {
                let text = std::fs::read_to_string(&f).unwrap_or_default();
                let d = text.trim_end().rsplit(' ').next().unwrap_or_default();
                if !s.has(d) {
                    let _ = std::fs::remove_file(&f);
                }
            }
            prune(&s.root.join(index));
        }
    }
    let (n, k) = (gone.len(), kept.len());
    let entries_en = plural(n, "entry", "entries");
    println!(
        "{}",
        ctx.say(&if dry {
            tr!(
                "ritsu mod clean --dry-run: 置き場の項目 {n} 個を消し、{k} 個を残すことになります。何も消していません",
                "ritsu mod clean --dry-run: {entries_en} of the store would be removed, {k} kept; nothing was removed"
            )
        } else {
            tr!("ritsu mod clean: 置き場の項目を {n} 個消し、{k} 個残しました", "ritsu mod clean: removed {entries_en} of the store, kept {k}")
        })
    );
    0
}
