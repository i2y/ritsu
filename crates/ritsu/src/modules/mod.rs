//! `ritsu mod` (DESIGN 17.19): the commands of ritsu's modules, with a table of their own as each
//! language has one (DESIGN 4.4). `ritsu mod --help` lists the fifteen; each reads its command line
//! from [`cli`], which hands the arguments read (and a [`Ctx`]) to one function of one file.
//!
//! - [`edit`]: `init`, `add` with a path, `remove` (DESIGN 17.3).
//! - [`lock`]: `lock`, and the lock `add` and `remove` write again when the tree changes (DESIGN 17.4,
//!   17.7).
//! - [`list`]: `list` and `where` (DESIGN 17.6, 17.21).
//! - [`vendor`]: `vendor` and `clean`, from and on the store (DESIGN 17.15).
//!
//! The commands the stages after this one make have their files here already, each with the
//! function the table calls, which says that this version does not have the command yet (exit 2),
//! so that each stage fills its own files and no other: [`diff`] and [`report`] (stage C, DESIGN
//! 17.9.6, 17.10); [`registry`] (`add` from a registry, `get`, `versions`), [`fetch`] (`fetch`, and
//! the `ritsu.mod` a version is chosen by when it is not in the store), [`push`], and [`http`],
//! [`auth`], [`store_write`] and [`cosign`] for what those need (stage D, DESIGN 17.12–17.14);
//! [`site`] (stage F, DESIGN 17.17); and [`git`], the git a dependency from a git repository is
//! fetched with (stage I, DESIGN 17.12.1).

pub mod cli;
mod edit;
mod list;
mod lock;
mod vendor;

// stage C
pub mod diff;
pub mod report;

// stage D
pub mod auth;
pub mod cosign;
pub mod fetch;
pub mod http;
pub mod push;
pub mod registry;
pub mod store_write;

// stage F
pub mod site;

// stage I
pub mod git;

use ritsu_base::diag::{Diag, Severity};
use ritsu_base::modules::graph::Problem;
use ritsu_base::modules::manifest;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_ports::Finding;
use std::path::{Path, PathBuf};

/// What every command of `ritsu mod` is given beside its own arguments: the command, the language
/// of what it says, `--format` and `--root`, which every command reads in the same way.
pub struct Ctx {
    /// The command, as `ritsu mod <command>` names it.
    pub command: &'static str,
    pub lang: Lang,
    /// `--format`: `text` unless the command line says otherwise.
    pub format: String,
    /// `--root`, as given.
    pub root_flag: Option<String>,
}

impl Ctx {
    /// Whether the command prints one JSON object (`--format json`, DESIGN 17.21).
    pub fn json(&self) -> bool {
        self.format == "json"
    }

    /// `t` in the language of the run.
    pub fn say(&self, t: &Text) -> String {
        t.get(self.lang).to_string()
    }

    /// What stops the command, said on the standard error: exit 2.
    pub fn refuse(&self, t: Text) -> u8 {
        let head = if self.lang == Lang::Ja { "エラー" } else { "error" };
        eprintln!("{head}: {}", t.get(self.lang));
        2
    }

    /// The directory the run is in, made canonical.
    pub fn here(&self) -> PathBuf {
        let cwd = ritsu_base::fs::current_dir().unwrap_or_default();
        ritsu_base::fs::canonicalize(&cwd).unwrap_or(cwd)
    }

    /// The root of the module at hand (DESIGN 17.19), made canonical: `--root`, which has to hold a
    /// `ritsu.mod`; else the nearest directory at or above the one the run is in that holds one,
    /// not past the nearest one with a `.git`.
    pub fn root(&self) -> Result<PathBuf, Text> {
        match &self.root_flag {
            Some(r) => {
                let p = ritsu_base::fs::canonicalize(Path::new(r)).map_err(|_| tr!("`--root {r}` がありません", "`--root {r}` is not there"))?;
                if !p.join(manifest::FILE).is_file() {
                    return Err(tr!("`--root {r}` に `ritsu.mod` がありません", "`--root {r}` has no `ritsu.mod`"));
                }
                Ok(p)
            }
            None => ritsu_base::modules::refer::raw_module_root(&self.here()).ok_or_else(|| {
                tr!(
                    "このディレクトリにも、その上にも `ritsu.mod` がありません。モジュールを作るなら、`ritsu mod init <名前>` を走らせてください",
                    "there is no `ritsu.mod` here or above; run `ritsu mod init <name>` to make a module"
                )
            }),
        }
    }

    /// A path as the run writes it: from the directory the run is in.
    pub fn shown(&self, p: &Path) -> String {
        ritsu_base::paths::between(&self.here(), p)
    }

    /// The diagnostics of the tree whose root is `root`, as ritsu's findings, each file written from
    /// the directory the run is in, as `ritsu check` writes them.
    pub fn findings(&self, root: &Path, ds: &[Diag]) -> Vec<Finding> {
        let shown = ritsu_base::paths::Shown::new(root, &self.shown(root));
        ds.iter()
            .map(|d| {
                let mut d = d.clone();
                let rel = d.rel.clone();
                d.file = shown.path(&rel);
                Finding::of(&d, Some(rel), self.lang)
            })
            .collect()
    }
}

/// What stops a command whose tree cannot be read (DESIGN 17.8), said on the standard error: what has
/// to be run first, or the tree's diagnostics and that nothing was written. Exit 2.
fn stopped(ctx: &Ctx, root: &Path, p: &Problem) -> u8 {
    match p {
        Problem::Stop(said) => {
            for t in said {
                ctx.refuse(t.clone());
            }
        }
        Problem::Diags(ds) => {
            let found = ctx.findings(root, ds);
            let text: String = found.iter().map(|f| f.text_of("ritsu")).collect();
            eprint!("{text}");
            let n = found.iter().filter(|f| f.severity == Severity::Error).count();
            let errors = ritsu_base::text::plural(n, "error", "errors");
            ctx.refuse(tr!("モジュールを読めません（エラー {n} 件）。何も書いていません", "the modules could not be read ({errors}); nothing was written"));
        }
    }
    2
}
