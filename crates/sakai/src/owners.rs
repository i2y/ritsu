//! The scope and who owns what (DESIGN 1.3, 3.2; PLAN B.4): every artifact of the scope belongs
//! to the context of the deepest entry of `owns` that holds it (E101, E102), the entries are in
//! the scope (E103), and hold something (W101).

use crate::diag::{self, Diag, DiagExt, Ref};
use ritsu_base::text::Text;
use crate::model::{Model, Own};
use crate::naming::{Name, Tool};
use crate::paths;
use crate::proto;
use crate::resolve::tool_of;

/// An artifact of the scope, and the context it belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct Artifact {
    /// From the root.
    pub path: String,
    pub tool: Tool,
    /// The context, and the entry of its `owns` that decided it.
    pub owner: Option<(usize, usize)>,
    /// An OpenAPI or AsyncAPI document, and whether it is a part of one that another reaches by
    /// `$ref` (DESIGN 15.2); its tool is `file`.
    pub contract: Option<(crate::contracts::Kind, bool)>,
}

impl Artifact {
    pub fn name(&self) -> Name {
        Name::file(self.tool, self.path.clone())
    }

    pub fn ctx(&self) -> Option<usize> {
        self.owner.map(|(c, _)| c)
    }
}

/// The extensions of the code of a language (DESIGN 1.3).
pub fn code_extensions(language: &str) -> &'static [&'static str] {
    match language {
        "python" => &["py"],
        "typescript" => &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"],
        "java" => &["java"],
        "go" => &["go"],
        "rust" => &["rs"],
        _ => &[],
    }
}

/// The file of a crate's manifest, an artifact of the code of Rust beside its `.rs` files: the
/// dependencies that cross a boundary are its lines (DESIGN 1.3, 7.7). A workspace's manifest
/// that is no crate's (no `[package]`) is in no context, as a table of holidays is in none.
pub const MANIFEST: &str = "Cargo.toml";

/// Whether the manifest `p` (a path from the root) is a crate's: it has a `[package]` table.
fn is_crate_manifest(m: &Model, p: &str) -> bool {
    ritsu_base::fs::read_to_string(paths::on_disk(&m.root, p)).is_ok_and(|s| s.lines().any(|l| l.trim_start().starts_with("[package]")))
}

/// Whether a `.proto` is one of the files known without being read (`google/protobuf/…`,
/// `buf/validate/…`, `dandori/v1/options.proto`), by its path from a `proto root`, from the
/// map's directory, or from the root.
pub fn is_known_proto(m: &Model, p: &str) -> bool {
    let mut bases: Vec<&str> = m.map.proto_roots.iter().map(String::as_str).collect();
    bases.push(&m.map.dir);
    bases.push(".");
    bases.iter().any(|b| paths::contains(b, p) && proto::is_known(&paths::relative(b, p)))
}

/// The tool of an artifact of the scope, or None for a file that is not one.
pub fn artifact_tool(m: &Model, p: &str) -> Option<Tool> {
    if let Some(t) = tool_of(p) {
        if t == Tool::Proto && is_known_proto(m, p) {
            return None;
        }
        return Some(t);
    }
    let ext = p.rsplit_once('.').map(|(_, e)| e)?;
    let manifest = p.rsplit('/').next() == Some(MANIFEST);
    m.map.code.iter().any(|c| paths::contains(&c.path, p) && (code_extensions(&c.language).contains(&ext) || (manifest && c.language == "rust" && is_crate_manifest(m, p)))).then_some(Tool::File)
}

/// An artifact of the scope: its path, its tool, and for an OpenAPI or AsyncAPI document its
/// kind and whether it is a part of one.
pub type InScope = (String, Tool, Option<(crate::contracts::Kind, bool)>);

/// Every artifact of the scope, in path order: under `covers`, not under `except`, and with no
/// part of its path that the scope always leaves out. A file of YAML or JSON is one when it is an
/// OpenAPI or AsyncAPI document, or a part of one that a document reaches by `$ref` (DESIGN 15.2).
pub fn scope(m: &Model) -> Vec<InScope> {
    let mut files = Vec::new();
    for c in &m.map.covers {
        paths::walk(&m.root, c, &m.map.except, &mut files);
    }
    files.sort();
    files.dedup();
    let mut out: Vec<InScope> = Vec::new();
    let mut docs: Vec<(String, crate::contracts::Kind)> = Vec::new();
    for p in files {
        if let Some(t) = artifact_tool(m, &p) {
            out.push((p, t, None));
        } else if crate::contracts::may_be(&p)
            && let Ok(text) = ritsu_base::fs::read_to_string(paths::on_disk(&m.root, &p))
            && let Some(k) = crate::contracts::sniff(&text, p.ends_with(".json"))
        {
            docs.push((p.clone(), k));
            out.push((p, Tool::File, Some((k, false))));
        }
    }
    for (p, k) in crate::contracts::parts(m, &docs) {
        if !out.iter().any(|(q, _, _)| *q == p) {
            out.push((p, Tool::File, Some((k, true))));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Whether a path is in the scope of the map.
pub fn in_scope(m: &Model, p: &str) -> bool {
    m.map.covers.iter().any(|c| paths::contains(c, p) || paths::contains(p, c)) && !m.map.except.iter().any(|e| paths::contains(e, p)) && !paths::has_skipped_part(p)
}

/// The deepest entry of any context's `owns` that holds `p`: an entry naming the file is deeper
/// than any directory. On a tie, the first context in the order of `use context`.
pub fn owner_of(m: &Model, p: &str) -> Option<(usize, usize)> {
    let mut best: Option<((usize, usize), usize)> = None;
    for (ci, c) in m.contexts.iter().enumerate() {
        for (oi, o) in c.owns.iter().enumerate() {
            if !o.holds(p) {
                continue;
            }
            let d = if o.is_dir() { paths::depth(&o.path) } else { usize::MAX };
            if best.is_none_or(|(_, bd)| d > bd) {
                best = Some(((ci, oi), d));
            }
        }
    }
    best.map(|(x, _)| x)
}

/// The context a file or directory belongs to by the same rule.
pub fn context_of(m: &Model, p: &str) -> Option<usize> {
    owner_of(m, p).map(|(c, _)| c)
}

/// The artifacts and their owners, and what is wrong with the entries of `owns`.
pub fn own(m: &Model) -> (Vec<Artifact>, Vec<Diag>) {
    let mut diags = Vec::new();
    let arts: Vec<Artifact> = scope(m).into_iter().map(|(p, t, contract)| Artifact { owner: owner_of(m, &p), path: p, tool: t, contract }).collect();
    // E102: one entry written by two contexts. Said once, at the later one.
    for (ci, c) in m.contexts.iter().enumerate() {
        for o in &c.owns {
            if let Some((cj, _)) = m.contexts[..ci].iter().enumerate().find_map(|(j, d)| d.owns.iter().find(|x| x.path == o.path).map(|x| (j, x))) {
                let (a, b) = (&m.contexts[cj].name, &c.name);
                let t = o.text();
                diags.push(
                    diag::at("E102", &c.file, o.pos.line, o.pos.col, tr!("{t} を、「{a}」と「{b}」の二つのコンテキストが同じ深さで持っています", "Both {a} and {b} own {t}, at the same depth"))
                        .source(&c.src)
                        .note(tr!(
                            "成果物は、それを含むいちばん深い項のコンテキストに属します。同じ深さの項が二つあると、どちらのものか決められません。",
                            "An artifact belongs to the context of the deepest entry that holds it; two entries at the same depth leave it undecided."
                        ))
                        .refer(Ref::line(Some(a), &m.contexts[cj].file, m.contexts[cj].owns.iter().find(|x| x.path == o.path).unwrap().pos.line, Text::same(t.clone())))
                        .refer(Ref::line(Some(b), &c.file, o.pos.line, Text::same(t.clone()))),
                );
            }
        }
    }
    // E103 and W101: every entry is in the scope and holds an artifact.
    for c in &m.contexts {
        for o in &c.owns {
            if !in_scope(m, &o.path) {
                diags.push(out_of_scope(m, &c.file, &c.src, o));
                continue;
            }
            if !arts.iter().any(|a| o.holds(&a.path)) {
                let t = o.text();
                diags.push(
                    diag::at("W101", &c.file, o.pos.line, o.pos.col, tr!("{t} は成果物を一つも含みません", "The entry {t} holds no artifact"))
                        .source(&c.src)
                        .note(tr!(
                            "パスの書き誤りかもしれません。成果物は、.rule、.flow、.cal、.book、.geas、.proto のファイル、OpenAPI と AsyncAPI の文書、地図の `code` に書いた言語の、その置き場所の下のコードです。",
                            "The path may be mistyped. The artifacts are the .rule, .flow, .cal, .book, .geas and .proto files, the OpenAPI and AsyncAPI documents, and the code of the languages the map's `code` lines name, under the places they give."
                        )),
                );
            }
        }
    }
    // E101: the artifacts no context owns, a directory of them said once.
    let unowned: Vec<&Artifact> = arts.iter().filter(|a| a.owner.is_none()).collect();
    let mut said: Vec<String> = Vec::new();
    for a in &unowned {
        if said.iter().any(|d| paths::contains(d, &a.path)) {
            continue;
        }
        // The topmost directory above it that holds no owned artifact.
        let parts: Vec<&str> = a.path.split('/').collect();
        let mut group = None;
        for k in 0..parts.len() {
            let d = if k == 0 { ".".to_string() } else { parts[..k].join("/") };
            if !m.map.covers.iter().any(|c| paths::contains(c, &d)) {
                continue;
            }
            if !arts.iter().any(|x| x.owner.is_some() && paths::contains(&d, &x.path)) {
                group = Some(d);
                break;
            }
        }
        let note = tr!(
            "どれかのコンテキストの `owns` に、そのディレクトリかファイルを書いてください。成果物は、どれもちょうど一つのコンテキストに属します。",
            "Write the directory or the file under the `owns` of a context; every artifact belongs to exactly one context."
        );
        match group {
            Some(d) if unowned.iter().filter(|x| paths::contains(&d, &x.path)).count() > 1 => {
                let n = unowned.iter().filter(|x| paths::contains(&d, &x.path)).count();
                let dir = if d == "." { "./".to_string() } else { format!("{d}/") };
                let mut dg = diag::whole("E101", &dir, tr!("この下のファイル {n} 件は、どのコンテキストにも属しません", "No context owns the {n} files under it")).note(note);
                for x in unowned.iter().filter(|x| paths::contains(&d, &x.path)).take(3) {
                    dg = dg.refer(Ref::name(None, x.name(), Text::default()));
                }
                diags.push(dg);
                said.push(d);
            }
            _ => {
                let sp = paths::shown(&a.path);
                diags.push(diag::whole("E101", &a.path, tr!("{sp} は、どのコンテキストにも属しません", "No context owns {sp}")).note(note));
                said.push(a.path.clone());
            }
        }
    }
    (arts, diags)
}

pub fn out_of_scope(m: &Model, file: &str, src: &str, o: &Own) -> Diag {
    let t = o.text();
    diag::at("E103", file, o.pos.line, o.pos.col, tr!("{t} は地図の範囲の外です", "The entry {t} is outside the map's scope")).source(src).note(scope_note(m))
}

/// What the scope is (DESIGN 1.3), with the map's `covers` as paths from the root.
pub fn scope_note(m: &Model) -> Text {
    let covers = m.map.covers.iter().map(|c| crate::naming::quote(c)).collect::<Vec<_>>().join(", ");
    tr!(
        "範囲は地図の `covers`（{covers}）で決まり、`except` と、パスに . で始まる名前、node_modules、site-packages、__pycache__、target を含むものは外れます。",
        "The scope is the map's `covers` ({covers}), without `except` and without any path with a name that starts with ., node_modules, site-packages, __pycache__ or target in it."
    )
}
