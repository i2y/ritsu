//! The commands that write `ritsu.mod` (DESIGN 17.3): `init`, `add` with a directory, and `remove`.
//! Each change is made a line at a time by ritsu-base's functions (`manifest::init`, `added`,
//! `removed`), which keep the comments, the order of the lines and their endings. `add` and `remove`
//! change `ritsu.mod` and `ritsu.lock` together or not at all: when the tree cannot be read after the
//! change, or its lock cannot be chosen, `ritsu.mod` is put back as it was (exit 2).

use super::lock::{self, Done};
use super::{Ctx, stopped};
use ritsu_base::cli::Args;
use ritsu_base::json::Json;
use ritsu_base::modules::graph;
use ritsu_base::modules::manifest::{self, Manifest, Source};
use ritsu_base::modules::names::ModuleName;
use ritsu_base::modules::semver::Version;
use ritsu_base::modules::store::Store;
use ritsu_base::text::Text;
use ritsu_base::tr;
use std::path::Path;

/// `t` after the sentence that says which name or version it is about.
fn sep(t: &Text) -> Text {
    Text::new(format!("。{}", t.ja), format!(": {}", t.en))
}

/// `ritsu mod init <module> [--version <version>]`: the exit code.
pub fn init(ctx: &Ctx, a: &Args) -> u8 {
    let name = a.pos[0].as_str();
    let module = match ModuleName::parse(name) {
        Ok(m) => m,
        Err(e) => return ctx.refuse(tr!("モジュールの名前 `{name}` は使えません", "`{name}` cannot name a module").then(&sep(&e.text()))),
    };
    let written = a.get("--version").unwrap_or("0.1.0");
    let version = match Version::parse(written) {
        Ok(v) => v,
        Err(e) => return ctx.refuse(e.text(written)),
    };
    let dir = match &ctx.root_flag {
        Some(r) => match ritsu_base::fs::canonicalize(Path::new(r)) {
            Ok(p) if p.is_dir() => p,
            _ => return ctx.refuse(tr!("`--root {r}` はディレクトリではありません", "`--root {r}` is not a directory")),
        },
        None => ctx.here(),
    };
    let file = dir.join(manifest::FILE);
    if file.exists() {
        let shown = ctx.shown(&file);
        return ctx.refuse(tr!("`{shown}` がもうあります", "`{shown}` is there already"));
    }
    if let Err(e) = std::fs::write(&file, manifest::init(&module, &version, &graph::running())) {
        let shown = ctx.shown(&file);
        return ctx.refuse(tr!("`{shown}` に書けません（{e}）", "cannot write `{shown}` ({e})"));
    }
    println!("{}", ctx.say(&tr!("ritsu.mod を書きました（モジュール {module} {version}）", "wrote ritsu.mod: module {module} {version}")));
    0
}

/// The `ritsu.mod` at `file`, read: what stops the command when it cannot be. `add` from a registry
/// and `get` (stage D) read it so too.
pub(super) fn read(ctx: &Ctx, root: &Path, file: &Path) -> Result<(Vec<u8>, Manifest), u8> {
    let bytes = std::fs::read(file).map_err(|e| {
        let shown = ctx.shown(file);
        ctx.refuse(tr!("`{shown}` を読めません（{e}）", "cannot read `{shown}` ({e})"))
    })?;
    match manifest::parse(&bytes, manifest::FILE) {
        Ok(m) => Ok((bytes, m)),
        Err(ds) => Err(stopped(ctx, root, &graph::Problem::Diags(ds))),
    }
}

/// `text` written to `ritsu.mod`, then the lock the tree wants: `ritsu.mod` put back as it was
/// (`before`) when the lock cannot be chosen. What was done to the lock, or the exit code. `add`
/// from a registry and `get` (stage D) change `ritsu.mod` through it too, so that every command that
/// writes `ritsu.mod` writes the lock with it or neither (DESIGN 17.19).
pub(super) fn change(ctx: &Ctx, root: &Path, file: &Path, before: &[u8], text: &str) -> Result<lock::Plan, u8> {
    let shown = || ctx.shown(file);
    std::fs::write(file, text).map_err(|e| {
        let shown = shown();
        ctx.refuse(tr!("`{shown}` に書けません（{e}）", "cannot write `{shown}` ({e})"))
    })?;
    let put_back = || {
        let _ = std::fs::write(file, before);
    };
    let plan = match lock::plan(root, Store::located().as_ref()) {
        Ok(p) => p,
        Err(p) => {
            put_back();
            return Err(stopped(ctx, root, &p));
        }
    };
    if let Err(t) = plan.write(root) {
        put_back();
        return Err(ctx.refuse(t));
    }
    Ok(plan)
}

/// The line of the lock a command says when it wrote or removed the lock.
pub(super) fn lock_line(ctx: &Ctx, plan: &lock::Plan) {
    if matches!(plan.done, Done::Written | Done::Removed) {
        println!("{}", ctx.say(&plan.said(false)));
    }
}

/// `ritsu mod add <name> ./…|../…`: a dependency by a path (DESIGN 17.3, 17.19), the path given from
/// where ritsu runs and written from the root of the module (★). The exit code.
pub fn add(ctx: &Ctx, a: &Args) -> u8 {
    let (name, source) = (a.pos[0].as_str(), a.pos[1].as_str());
    let root = match ctx.root() {
        Ok(r) => r,
        Err(t) => return ctx.refuse(t),
    };
    let Ok(target) = ritsu_base::fs::canonicalize(Path::new(source)) else {
        return ctx.refuse(tr!("`{source}` がありません", "`{source}` is not there"));
    };
    let theirs = target.join(manifest::FILE);
    if !theirs.is_file() {
        return ctx.refuse(tr!(
            "`{source}` に `ritsu.mod` がありません。パスの依存は、`ritsu.mod` のあるディレクトリを指します",
            "`{source}` has no `ritsu.mod`; a dependency by a path names a directory with a `ritsu.mod` in it"
        ));
    }
    let path = ritsu_base::paths::between(&root, &target);
    if let Some(t) = manifest::path_problem(&path, false) {
        return ctx.refuse(t);
    }
    let file = root.join(manifest::FILE);
    let (before, m) = match read(ctx, &root, &file) {
        Ok(x) => x,
        Err(code) => return code,
    };
    let source = Source::Path { path: path.clone() };
    let text = match m.added(name, &source) {
        Ok(t) => t,
        Err(e) => return ctx.refuse(e.text()),
    };
    let plan = match change(ctx, &root, &file, &before, &text) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let line = format!("require {name} {}", source.words());
    // the lock was chosen with the tree read, so the dependency's `ritsu.mod` reads
    let added = std::fs::read(&theirs).ok().and_then(|b| manifest::parse(&b, manifest::FILE).ok());
    let module = added.as_ref().map(|m| m.module.value.to_string()).unwrap_or_default();
    let version = added.as_ref().and_then(|m| m.version.as_ref()).map(|v| v.value.to_string());
    if ctx.json() {
        println!("{}", added_json(ctx, &root, name, &module, version, Some(&path), plan.done).pretty());
        return 0;
    }
    let what = match &version {
        Some(v) => format!("{module} {v}"),
        None => module.clone(),
    };
    println!("{}", ctx.say(&tr!("ritsu.mod に `{line}` を足しました（{what}）", "added `{line}` to ritsu.mod ({what})")));
    lock_line(ctx, &plan);
    0
}

/// What `ritsu mod add --format json` prints (DESIGN 17.21): the dependency added, from a directory
/// when `path` is given and from a registry when it is not (stage D's `add` prints the same), and
/// what was done to the lock.
pub(super) fn added_json(ctx: &Ctx, root: &Path, name: &str, module: &str, version: Option<String>, path: Option<&str>, done: Done) -> Json {
    Json::obj([
        ("ritsu", Json::str(env!("CARGO_PKG_VERSION"))),
        ("root", Json::str(ctx.shown(root))),
        (
            "require",
            Json::obj([
                ("name", Json::str(name)),
                ("module", Json::str(module)),
                ("version", Json::opt_str(version)),
                ("source", Json::str(if path.is_some() { "path" } else { "oci" })),
                ("path", Json::opt_str(path)),
            ]),
        ),
        ("manifest", Json::str("written")),
        ("lock", Json::str(done.word())),
    ])
}

/// `ritsu mod remove <name>`: the dependency's `require` line removed (DESIGN 17.3), and the lock
/// written again when the tree changes. The exit code.
pub fn remove(ctx: &Ctx, a: &Args) -> u8 {
    let name = a.pos[0].as_str();
    let root = match ctx.root() {
        Ok(r) => r,
        Err(t) => return ctx.refuse(t),
    };
    let file = root.join(manifest::FILE);
    let (before, m) = match read(ctx, &root, &file) {
        Ok(x) => x,
        Err(code) => return code,
    };
    let line = m.require(name).map(|r| format!("require {} {}", r.name, r.source.words()));
    let text = match m.removed(name) {
        Ok(t) => t,
        Err(e) => return ctx.refuse(e.text()),
    };
    let plan = match change(ctx, &root, &file, &before, &text) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let line = line.unwrap_or_default();
    println!("{}", ctx.say(&tr!("ritsu.mod から `{line}` を消しました", "removed `{line}` from ritsu.mod")));
    lock_line(ctx, &plan);
    0
}
