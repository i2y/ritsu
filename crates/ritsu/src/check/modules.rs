//! What `ritsu check` says of the modules of a project (DESIGN 17.8, 17.20): the diagnostics of the
//! tree, a `require` no file uses (W301), a path through a dependency to a module the file's own
//! module does not require (W303), the contributions that add to a rule of another module, and the
//! modules a walk passed over. Each is ritsu's own; a language's diagnostics of a dependency's file
//! are the language's. What the files name is ritsu-project's ([`Project::named`]), which `ritsu
//! gen` reads too.

use ritsu_base::diag::Diag;
use ritsu_base::modules::{self, Tree};
use ritsu_base::naming::Tool;
use ritsu_base::paths;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_ports::Finding;
use ritsu_project::{Joined, Project};
use std::path::Path;

/// A contribution of one module that adds to a rule of another (DESIGN 17.8), by their paths from
/// the root.
pub struct Crossing {
    pub file: String,
    pub to: String,
}

/// A diagnostic of the tree (E301–E308, W302) as ritsu's finding, its file as the run writes it.
pub fn finding(project: &Project, d: &Diag, lang: Lang) -> Finding {
    let mut d = d.clone();
    let rel = d.rel.clone();
    d.file = project.shown.path(&rel);
    Finding::of(&d, Some(rel), lang)
}

/// The node of the tree a path from the root is in.
fn node_of(tree: &Tree, rel: &str) -> Option<usize> {
    tree.by_prefix(&modules::prefix_of(rel).unwrap_or_default())
}

/// A module, as the notes write it: this module, or the dependency with its name and version.
fn module_words(tree: &Tree, node: Option<usize>) -> Text {
    match node.filter(|i| *i > 0).map(|i| &tree.nodes[i]) {
        None => tr!("このモジュール", "this module"),
        Some(n) => {
            let (p, name) = (&n.prefix, n.name.to_string());
            match n.version() {
                Some(v) => tr!("依存 `{p}`（{name} {v}）", "the dependency `{p}` ({name} {v})"),
                None => tr!("依存 `{p}`（{name}）", "the dependency `{p}` ({name})"),
            }
        }
    }
}

/// W303 for each path a file of the project writes through a dependency to a module its own module
/// does not require (`@pricing/@tax/…`), and W301 for each `require` of a module that no file of
/// that module uses, when the whole module is read: what a file of a module names is written from
/// that module's own root ([`modules::from_own_root`]). What a file names is what its language's
/// references give, the `to` and the `after` of a contribution, and the imports of a `.proto`
/// ([`Project::named`]). A module with a file that does not read is given no W301: what it names
/// is not known.
pub fn uses(project: &Project, joined: &Joined, tree: &Tree, lang: Lang) -> Vec<Finding> {
    let (named, refused) = project.named(joined);
    let mut out = Vec::new();
    for (f, line, target) in &named {
        let own = modules::from_own_root(&project.root, &f.rel, target);
        let Some(chain) = modules::prefix_of(&own) else { continue };
        let names: Vec<&str> = chain.split('/').map(|p| p.trim_start_matches('@')).collect();
        if names.len() < 2 {
            continue;
        }
        let (first, last) = (names[0], names[names.len() - 1]);
        let src = ritsu_base::fs::read_to_string(paths::on_disk(&project.root, &f.rel)).unwrap_or_default();
        let d: Diag = Diag::at(
            "W303",
            &f.shown,
            *line,
            1,
            tr!(
                "`{own}` は、このモジュールが `require` していないモジュールを指しています。{first} の新しい版が {last} を使わなくなると、このファイルは壊れます。`require` に足してください",
                "`{own}` reaches a module this module does not require; require it, so that a version of {first} that drops it does not break this file"
            ),
        )
        .source(&src)
        .rel(&f.rel);
        out.push(Finding::of(&d, Some(f.rel.clone()), lang));
    }
    if !project.whole {
        return out;
    }
    for (m, node) in tree.nodes.iter().enumerate() {
        if refused.iter().any(|r| node_of(tree, r) == Some(m)) {
            continue;
        }
        let file = if node.prefix.is_empty() { modules::manifest::FILE.to_string() } else { format!("{}/{}", node.prefix, modules::manifest::FILE) };
        for r in &node.manifest.requires {
            let Some(d) = tree.dep(m, &r.name) else { continue };
            if named.iter().any(|(f, _, t)| node_of(tree, &f.rel) == Some(m) && node_of(tree, t) == Some(d)) {
                continue;
            }
            let (name, module) = (&r.name, tree.nodes[d].name.to_string());
            let diag: Diag = Diag::at(
                "W301",
                &project.shown.path(&file),
                r.line,
                r.col,
                tr!(
                    "このモジュールのどのファイルも、依存 `{name}`（{module}）を使っていません。`@{name}/…` のファイルを指すものがありません",
                    "No file of this module uses its dependency `{name}` ({module}): none names a file of `@{name}/…`"
                ),
            )
            .source(&node.manifest.text())
            .rel(&file)
            .note(tr!(
                "使わないなら、`require` の行を消してください（`ritsu mod remove {name}`）。使うのなら、使うところで `@{name}/…` のファイルを指してください。",
                "If the module does not use it, delete the `require` (`ritsu mod remove {name}`); if it is to, name a file of `@{name}/…` where it does."
            ));
            out.push(Finding::of(&diag, Some(file.clone()), lang));
        }
    }
    out
}

/// The contributions of one module that add to a rule of another (DESIGN 17.8): a contribution
/// whose `to` is a file of another module.
pub fn crossings(project: &Project) -> Vec<Crossing> {
    if project.tree.is_none() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for f in project.of(Tool::Rulec) {
        let Some((to, _)) = rulec::link::target_of(&f.shown) else { continue };
        let Some(to) = paths::from_root(&project.root, Path::new(&to)) else { continue };
        if project.dependency_of(&f.rel) != project.dependency_of(&to) {
            out.push(Crossing { file: f.rel.clone(), to });
        }
    }
    out
}

/// The notes before the summary: each contribution that crosses from one module to another, each
/// module read from a path in place of the version a registry gives it elsewhere in the tree
/// (DESIGN 17.7), and each module the walk passed over (DESIGN 17.6).
pub fn notes(project: &Project, crossings: &[Crossing]) -> Vec<Text> {
    let mut out = Vec::new();
    if let Some(Ok(tree)) = project.tree.as_deref() {
        for c in crossings {
            let (f, to) = (project.shown.path(&c.file), project.shown.path(&c.to));
            let (from, of) = (module_words(tree, node_of(tree, &c.file)), module_words(tree, node_of(tree, &c.to)));
            out.push(tr!(
                "備考: {}の寄与 `{f}` は、{}の規則 `{to}` に足しています",
                "note: `{f}`, a contribution of {}, adds to `{to}`, a rule of {}",
                from.ja, of.ja;
                from.en, of.en
            ));
        }
        for name in &tree.path_won {
            out.push(tr!(
                "備考: モジュール {name} は、木のほかの `require` がレジストリの版を求めていますが、`require … path` のパスから読んでいます",
                "note: the module {name} is read from the path a `require … path` gives, in place of the version from a registry another `require` of the tree asks for"
            ));
        }
    }
    for m in &project.skipped {
        out.push(paths::other_module_note(&project.shown.path(m), "ritsu check"));
    }
    out
}
