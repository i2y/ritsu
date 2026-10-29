//! `ritsu mod push [--dry-run] [--format text|json]` (DESIGN 17.12): the module at hand checked,
//! built into its OCI artifact and pushed to the registry its name says.
//!
//! In this order, and each step stops the push: `ritsu.mod` read (a published name, a `version`, a
//! `ritsu >=`, no `require … path`, no `require … git`: DESIGN 17.12.1) and the tree with its lock;
//! the files the module carries gathered and held to the limits; every one of them scanned for keys
//! (W901's table, which leaves out a line marked `ritsu: test secret`); a copy of them checked as
//! the module (with the same lock and the same store, its own `.geas` claims not run); the artifact
//! built and its digest said, where `--dry-run` ends, before any request; the tag looked up (the
//! same files are pushed already, other files refuse the push: a version does not move); each blob
//! the registry does not have uploaded; the OCI manifest put at the tag. Then the reference and the
//! commands that sign it with cosign.
//!
//! Exit 0 (pushed, or pushed already), 1 (refused: the check, a key, a limit, a path dependency or
//! one from git, other files at the version's tag), 2 (cannot: the network, the credentials, no
//! lock).

use super::cosign;
use super::http::NoAnswer;
use super::registry::{self, Remote};
use super::{Ctx, store_write};
use ritsu_base::cli::Args;
use ritsu_base::diag::{Diag, Severity};
use ritsu_base::json::Json;
use ritsu_base::modules::graph::{self, Problem};
use ritsu_base::modules::manifest::{self, Source};
use ritsu_base::modules::store;
use ritsu_base::modules::{E303, artifact, digest};
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text, capitalize, plural};
use ritsu_base::tr;
use std::path::{Path, PathBuf};

/// How a push stops.
enum Stop {
    /// What would be pushed is refused: exit 1.
    Refused(Text),
    /// The push cannot go on: exit 2.
    Cannot(Text),
}

/// What is known of the module when a push stops, for the JSON.
#[derive(Default)]
struct Known {
    module: Option<String>,
    version: Option<String>,
    /// The digests of the OCI manifest and of the files, once the artifact is built.
    built: Option<(String, String)>,
}

/// What a push came to.
struct Done {
    module: String,
    version: String,
    /// The digest of the OCI manifest at the tag: the one built, or the one there already.
    digest: String,
    files_digest: String,
    /// `new`, `already` or `dry-run`.
    pushed: &'static str,
    reference: String,
    /// The commands that sign it: with no key, and with a key.
    sign: (String, String),
}

/// What a push says when it stops: the exit code.
fn stopped(ctx: &Ctx, known: &Known, s: Stop) -> u8 {
    let (code, text) = match s {
        Stop::Refused(t) => (1, t),
        Stop::Cannot(t) => (2, t),
    };
    if ctx.json() {
        let opt = |s: &Option<String>| s.as_deref().map(Json::str).unwrap_or(Json::Null);
        let (d, f) = known.built.clone().unzip();
        let o = Json::obj([
            ("ritsu", Json::str(env!("CARGO_PKG_VERSION"))),
            ("module", opt(&known.module)),
            ("version", opt(&known.version)),
            ("digest", opt(&d)),
            ("files_digest", opt(&f)),
            ("pushed", Json::Null),
            ("reference", Json::Null),
            ("sign", Json::Null),
            ("error", Json::str(ctx.say(&text))),
        ]);
        println!("{}", o.pretty());
    } else {
        let h = if ctx.lang == Lang::Ja { "エラー" } else { "error" };
        eprintln!("{h}: {}", ctx.say(&text));
    }
    code
}

/// What a push that went through prints.
fn said(ctx: &Ctx, d: &Done) {
    if ctx.json() {
        let o = Json::obj([
            ("ritsu", Json::str(env!("CARGO_PKG_VERSION"))),
            ("module", Json::str(&d.module)),
            ("version", Json::str(&d.version)),
            ("digest", Json::str(&d.digest)),
            ("files_digest", Json::str(&d.files_digest)),
            ("pushed", Json::str(d.pushed)),
            ("reference", Json::str(&d.reference)),
            ("sign", Json::str(&d.sign.0)),
            ("error", Json::Null),
        ]);
        println!("{}", o.pretty());
        return;
    }
    let (m, v) = (&d.module, &d.version);
    let head = match d.pushed {
        "new" => tr!("{m} {v} を送り出しました", "pushed {m} {v}"),
        "already" => tr!("{m} {v} は送り出し済みです（レジストリのファイルが同じです）", "{m} {v} is pushed already: the registry has the same files"),
        _ => tr!("{m} {v} の成果物を作りました（`--dry-run` なので、送っていません）", "built {m} {v}, and pushed nothing (`--dry-run`)"),
    };
    println!("{}", ctx.say(&head));
    let rows = [
        (tr!("ダイジェスト", "digest"), &d.digest),
        (tr!("ファイルのダイジェスト", "files"), &d.files_digest),
        (tr!("参照", "reference"), &d.reference),
    ];
    for (k, v) in rows {
        println!("  {}: {v}", ctx.say(&k));
    }
    if d.pushed != "dry-run" {
        println!("{}", ctx.say(&tr!("cosign で署名してください:", "sign it with cosign:")));
        println!("  {}: {}", ctx.say(&tr!("CI で鍵を持たずに", "in CI, with no key")), d.sign.0);
        println!("  {}: {}", ctx.say(&tr!("鍵で", "with a key")), d.sign.1);
    }
}

/// W901 for each key written in the files to push (DESIGN 17.12, step 3), with W901's sentence:
/// the kind of key, its prefix and its length, never the key, and no line quoted.
fn keys_in(files: &[(String, Vec<u8>)]) -> Vec<Diag> {
    let mut out = Vec::new();
    for (rel, bytes) in files {
        let text = String::from_utf8_lossy(bytes);
        for f in ritsu_base::secrets::scan(&text).iter().filter(|f| !f.test) {
            let name = &f.kind.name;
            let msg = if f.kind.provider.is_empty() {
                tr!("{}がここに書かれています（{}）", "{} is written here ({})", name.ja, f.shown; capitalize(&name.en), f.shown)
            } else {
                tr!("{}がここに書かれています（{}、{} 文字）", "{} is written here ({}, {} characters)", name.ja, f.shown, f.len; capitalize(&name.en), f.shown, f.len)
            };
            out.push(Diag::warning("W901", rel, f.line, f.col, msg).rel(rel).note(tr!(
                "送り出したファイルは、レジストリからモジュールを取るすべての人に渡ります。鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置いてください。",
                "A file pushed reaches everyone who fetches the module from the registry. Keep the key where the code runs (an environment variable, the platform's connection or secret store)."
            )));
        }
    }
    out
}

/// A directory removed when the push is done with it.
struct Gone(PathBuf);

impl Drop for Gone {
    fn drop(&mut self) {
        let _ = store::remove(&self.0);
    }
}

/// The files under `from` written under `to` (the module's copy of the store, for the check).
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let t = e.file_type()?;
        if t.is_dir() {
            copy_tree(&e.path(), &to.join(e.file_name()))?;
        } else if t.is_file() {
            std::fs::write(to.join(e.file_name()), std::fs::read(e.path())?)?;
        }
    }
    Ok(())
}

/// The files to push, written to a directory of their own and checked there as the module (DESIGN
/// 17.12, step 4): with the same lock (among the files) and the same store (and the module's copy of
/// it), the module's own `.geas` claims not run. What the check printed goes to the standard error
/// when it fails.
fn check_copy(ctx: &Ctx, root: &Path, files: &[(String, Vec<u8>)]) -> Result<(), Stop> {
    // nothing a language checks but `.geas` files, whose claims are not run
    if !files.iter().any(|(p, _)| ritsu_project::project::kind_of(p).is_some_and(|t| t != Tool::Geas)) {
        return Ok(());
    }
    let disk = |e: std::io::Error| Stop::Cannot(tr!("送り出すファイルを検査のために書けません（{e}）", "cannot write the files to push for their check ({e})"));
    let scratch = store_write::scratch().map_err(Stop::Cannot)?;
    let dir = store_write::unique(&scratch, "push");
    let _gone = Gone(dir.clone());
    for (rel, bytes) in files {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(disk)?;
        }
        std::fs::write(&p, bytes).map_err(disk)?;
    }
    let vendored = root.join(".ritsu").join("modules");
    if vendored.is_dir() {
        copy_tree(&vendored, &dir.join(".ritsu").join("modules")).map_err(disk)?;
    }
    let place = store::location().map(|p| ritsu_base::paths::absolute(&p));
    let here = std::env::current_dir().map_err(disk)?;
    std::env::set_current_dir(&dir).map_err(disk)?;
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let args = ["--root".to_string(), ".".to_string()];
    let (o, e): (&mut dyn std::io::Write, &mut dyn std::io::Write) = (&mut out, &mut err);
    let run = move || ritsu_project::project::without_own_claims(move || ritsu::check::run(&args, ctx.lang, o, e));
    let code = match &place {
        Some(p) => store::with_location(p, run),
        None => run(),
    };
    let _ = std::env::set_current_dir(&here);
    if code == 0 {
        return Ok(());
    }
    eprint!("{}{}", String::from_utf8_lossy(&out), String::from_utf8_lossy(&err));
    Err(if code == 2 {
        Stop::Cannot(tr!("送り出すファイルを検査できません（上の出力）。送り出していません", "the files to push could not be checked (above), and nothing was pushed"))
    } else {
        Stop::Refused(tr!(
            "送り出すファイルの検査にエラーがあります（上の出力）。送り出していません。`ritsu check` で確かめて直してください",
            "the check of the files to push found errors (above), and nothing was pushed; fix them, with `ritsu check` to see them"
        ))
    })
}

/// `ritsu mod push [--dry-run]`: the exit code.
pub fn push(ctx: &Ctx, a: &Args) -> u8 {
    let mut known = Known::default();
    match run(ctx, a, &mut known) {
        Ok(d) => {
            said(ctx, &d);
            0
        }
        Err(s) => stopped(ctx, &known, s),
    }
}

fn run(ctx: &Ctx, a: &Args, known: &mut Known) -> Result<Done, Stop> {
    let root = ctx.root().map_err(Stop::Cannot)?;
    let mod_bytes = std::fs::read(root.join(manifest::FILE)).map_err(|e| Stop::Cannot(tr!("ritsu.mod を読めません（{e}）", "cannot read ritsu.mod ({e})")))?;
    let say_diags = |ds: &[Diag]| {
        let found = ctx.findings(&root, ds);
        eprint!("{}", found.iter().map(|f| f.text_of("ritsu")).collect::<String>());
        found.iter().filter(|f| f.severity == Severity::Error).count()
    };
    let unread = |n: usize| {
        let errors = plural(n, "error", "errors");
        tr!("モジュールを読めません（エラー {n} 件）。送り出していません", "the modules could not be read ({errors}), and nothing was pushed")
    };
    // 1. `ritsu.mod`, and the tree with its lock
    let m = match manifest::parse(&mod_bytes, manifest::FILE) {
        Ok(m) => m,
        Err(ds) => return Err(Stop::Refused(unread(say_diags(&ds)))),
    };
    let name = m.module.value.clone();
    known.module = Some(name.to_string());
    if !name.is_published() {
        return Err(Stop::Refused(tr!(
            "モジュールの名前 `{name}` は配る名前ではないので、送り出せません。`module` に `<レジストリ>/<リポジトリ>` の形の名前を書いてください",
            "the module `{name}` has a name that is not published, so it cannot be pushed; write `<registry>/<repository>` on its `module` line"
        )));
    }
    let Some(version) = m.version.as_ref().map(|v| v.value.clone()) else {
        return Err(Stop::Refused(tr!("`ritsu.mod` に `version` がありません。送り出す版を書いてください", "`ritsu.mod` has no `version`; write the version to push")));
    };
    known.version = Some(version.to_string());
    if m.ritsu.is_none() {
        let running = graph::running();
        return Err(Stop::Refused(tr!(
            "`ritsu.mod` に `ritsu` の行がありません。モジュールを読める ritsu のいちばん低い版を、`ritsu >={running}` のように書いてください",
            "`ritsu.mod` has no `ritsu` line; write the least ritsu that reads the module, as `ritsu >={running}`"
        )));
    }
    if let Some(r) = m.requires.iter().find(|r| matches!(r.source, Source::Path { .. })) {
        let (rn, line, words) = (&r.name, r.line, r.source.words());
        return Err(Stop::Refused(tr!(
            "`ritsu.mod` の {line} 行目は `require {rn} {words}` です。パスの依存を持つモジュールは送り出せません（取る側にそのパスがありません）。依存を先に送り出し、この行を `require {rn} oci \"<リポジトリ>\" <版>` に書き換えてから、`ritsu mod lock` を走らせてください",
            "line {line} of `ritsu.mod` is `require {rn} {words}`, and a module with a path dependency cannot be pushed (whoever fetches it has no such path); push the dependency first, change this line to `require {rn} oci \"<repository>\" <version>`, and run `ritsu mod lock`"
        )));
    }
    if let Some(r) = m.requires.iter().find(|r| matches!(r.source, Source::Git { .. })) {
        let (rn, line, words) = (&r.name, r.line, r.source.words());
        return Err(Stop::Refused(tr!(
            "`ritsu.mod` の {line} 行目は `require {rn} {words}` です。git の依存を持つモジュールはレジストリに送り出せません（取る側がそのリポジトリに届くとは限らず、git の依存には確かめる署名もありません）。依存をレジストリに送り出し、この行を `require {rn} oci \"<リポジトリ>\" <版>` に書き換えてから、`ritsu mod lock` を走らせてください",
            "line {line} of `ritsu.mod` is `require {rn} {words}`, and a module with a dependency from git cannot be pushed to a registry (whoever fetches it may not reach that repository, and a dependency from git has no signature to check); push the dependency to a registry, change this line to `require {rn} oci \"<repository>\" <version>`, and run `ritsu mod lock`"
        )));
    }
    let located = store::Store::located();
    match graph::load_with(&root, located.as_ref()).0 {
        Ok(_) => {}
        Err(Problem::Stop(said)) => {
            let mut t = Text::default();
            for (i, s) in said.iter().enumerate() {
                t = if i == 0 { s.clone() } else { Text::new(format!("{}\n{}", t.ja, s.ja), format!("{}\n{}", t.en, s.en)) };
            }
            return Err(Stop::Cannot(t));
        }
        Err(Problem::Diags(ds)) => {
            let n = say_diags(&ds);
            return Err(if ds.iter().any(|d| d.code == E303) { Stop::Cannot(unread(n)) } else { Stop::Refused(unread(n)) });
        }
    }
    // 2. the files, held to the limits
    let files = digest::module_files(&root, &m).map_err(Stop::Refused)?;
    let built = artifact::build(&m, &mod_bytes, &files).map_err(Stop::Refused)?;
    // 3. no key in them
    let keys = keys_in(&files);
    if !keys.is_empty() {
        say_diags(&keys);
        let n = keys.len();
        let k = plural(n, "key", "keys");
        return Err(Stop::Refused(tr!(
            "送り出すファイルに、鍵が {n} 件書かれています。送り出していません。鍵をファイルから除いてください（テスト用の値なら、同じ行に `ritsu: test secret` と書いてください）",
            "the files to push have {k} written in them, and nothing was pushed; take them out of the files (a value for tests takes `ritsu: test secret` on its line)"
        )));
    }
    // 4. the check of what is pushed
    check_copy(ctx, &root, &files)?;
    // 5. the artifact
    known.built = Some((built.digest.clone(), built.files_digest.clone()));
    let loopback = registry::on_loopback(&name);
    let done = |digest: &str, pushed: &'static str| {
        let reference = format!("{name}@{digest}");
        Done {
            module: name.to_string(),
            version: version.to_string(),
            digest: digest.to_string(),
            files_digest: built.files_digest.clone(),
            pushed,
            sign: cosign::sign_commands(&reference, loopback),
            reference,
        }
    };
    if a.has("--dry-run") {
        return Ok(done(&built.digest, "dry-run"));
    }
    // 6. the tag: what is there and is no module (an OCI manifest over its limit too) is refused
    let remote = Remote::of(&name, true).map_err(Stop::Cannot)?;
    let tag = version.to_string();
    let not_a_module = |t: Text| {
        Stop::Refused(tr!(
            "レジストリの {name} のタグ {version} には、ritsu のモジュールでないものがあります。{}。版を上げて送り出してください",
            "the registry has something other than a module of ritsu at the tag {version} of {name}; raise the version and push again: {}",
            t.ja; t.en
        ))
    };
    if remote.has_manifest(&tag).map_err(Stop::Cannot)?
        && let Some(bytes) = remote.manifest(&tag).map_err(|n| match n {
            NoAnswer::Over(t) => not_a_module(t),
            NoAnswer::Other(t) => Stop::Cannot(t),
        })?
    {
        let ours = &built.files_digest;
        return match artifact::check(&bytes) {
            Ok(l) if l.files_digest == *ours => Ok(done(&digest::of(&bytes), "already")),
            Ok(l) => {
                let theirs = l.files_digest;
                Err(Stop::Refused(tr!(
                    "レジストリの {name} {version} は、ファイルのダイジェストが {theirs} で、送り出すファイル（{ours}）と違います。送り出した版は動かさないので、`ritsu.mod` の `version` を上げて送り出してください",
                    "the registry's {name} {version} has the files {theirs}, not the {ours} of the files to push; a version pushed does not move: raise the `version` of `ritsu.mod` and push again"
                )))
            }
            Err(t) => Err(not_a_module(t)),
        };
    }
    // 7. the blobs it does not have
    let blobs: [(&str, &[u8]); 3] = [(artifact::EMPTY_DIGEST, artifact::EMPTY), (&built.module_digest, &built.module), (&built.files_digest, &built.tar)];
    for (d, b) in blobs {
        if !remote.has_blob(d).map_err(Stop::Cannot)? {
            remote.upload(d, b).map_err(Stop::Cannot)?;
        }
    }
    // 8. the OCI manifest, at the tag
    if let Some(theirs) = remote.put_manifest(&tag, &built.manifest).map_err(Stop::Cannot)?
        && theirs != built.digest
    {
        let ours = &built.digest;
        return Err(Stop::Cannot(tr!(
            "レジストリが、送った OCI のマニフェストのダイジェストを {theirs} と答えました（送ったものは {ours}）。レジストリがバイトを変えています",
            "the registry answered the digest {theirs} for the OCI manifest pushed, which is {ours}: the registry changed its bytes"
        )));
    }
    Ok(done(&built.digest, "new"))
}
