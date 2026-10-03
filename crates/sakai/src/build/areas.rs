//! The groups an import is checked by (DESIGN 7.1; PLAN C.6): for one language, the directories
//! of its code, grouped into a context's inside, the code made from a published language, a
//! shared kernel and an anticorruption layer, and which group may import which. The four tools'
//! settings are this table, each in its tool's words.
//!
//! A code file belongs to the group of the deepest directory that holds it (DESIGN 1.3); at one
//! depth, a published language's code, a layer and a shared kernel come before the inside that
//! lists the same directory. A group with no code file in it is not made.

use crate::ast::Role;
use crate::check::Checked;
use crate::diag::{self, Diag};
use ritsu_base::text::Text;
use crate::model::{Model, RelK};
use crate::naming::Tool;
use crate::owners;
use crate::paths;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Python,
    TypeScript,
    Java,
    Go,
}

impl Language {
    /// The word of the map's `code` line.
    pub fn word(self) -> &'static str {
        match self {
            Language::Python => "python",
            Language::TypeScript => "typescript",
            Language::Java => "java",
            Language::Go => "go",
        }
    }
}

/// What a group is.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Internal,
    /// The code made from the published language of this package.
    Published(String),
    /// An anticorruption layer toward this upstream context.
    Layer(usize),
    /// The shared kernel with this context.
    Kernel(usize),
}

impl Kind {
    /// Which of two kinds of the same directory wins.
    fn rank(&self) -> u8 {
        match self {
            Kind::Internal => 0,
            Kind::Kernel(_) => 1,
            Kind::Layer(_) => 2,
            Kind::Published(_) => 3,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Area {
    /// The context it belongs to.
    pub ctx: usize,
    pub kind: Kind,
    /// Its directories, from the code directory (`.` for the code directory itself), in order.
    pub roots: Vec<String>,
    /// Its code files, from the code directory, in order.
    pub files: Vec<String>,
}

/// The groups of one language and which may import which.
pub struct Areas {
    pub language: Language,
    /// The code directory, from the root.
    pub dir: String,
    pub areas: Vec<Area>,
    /// `allowed[from][to]`: whether the code of group `from` may import the code of group `to`.
    pub allowed: Vec<Vec<bool>>,
    /// What the map's `except` leaves out under the code directory, from it.
    pub except: Vec<String>,
}

impl Areas {
    /// The directories of other groups under `root` of group `i`: what the group's directory
    /// holds and the group does not.
    pub fn nested(&self, i: usize, root: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (j, a) in self.areas.iter().enumerate() {
            if j == i {
                continue;
            }
            for r in &a.roots {
                if r != root && paths::contains(root, r) && !out.contains(r) {
                    out.push(r.clone());
                }
            }
        }
        out.sort();
        out
    }

    /// The groups that may import group `to`, itself among them, in order.
    pub fn importers(&self, to: usize) -> Vec<usize> {
        (0..self.areas.len()).filter(|&f| self.allowed[f][to]).collect()
    }
}

/// `sakai-<alias>`, `sakai-<alias>-pl-<package>`, `sakai-<alias>-layer-<upstream's alias>`,
/// `sakai-<alias>-kernel-<the other side's alias>`: the name each tool's setting calls the group.
pub fn name(m: &Model, a: &Area) -> String {
    let alias = &m.contexts[a.ctx].alias;
    match &a.kind {
        Kind::Internal => format!("sakai-{alias}"),
        Kind::Published(p) => format!("sakai-{alias}-pl-{p}"),
        Kind::Layer(u) => format!("sakai-{alias}-layer-{}", m.contexts[*u].alias),
        Kind::Kernel(o) => format!("sakai-{alias}-kernel-{}", m.contexts[*o].alias),
    }
}

/// What the group is, in the words of the map.
pub fn phrase(m: &Model, a: &Area) -> Text {
    let c = &m.contexts[a.ctx];
    let (n, alias) = (&c.name, &c.alias);
    match &a.kind {
        Kind::Internal => tr!("「{n}」({alias}) の内側", "the inside of {n} ({alias})"),
        Kind::Published(p) => tr!("「{n}」の公表された言語 {p} から生成したコード", "the code made from {n}'s published language {p}"),
        Kind::Layer(u) => {
            let un = &m.contexts[*u].name;
            tr!("「{n}」の「{un}」に向けた腐敗防止層", "{n}'s anticorruption layer toward {un}")
        }
        Kind::Kernel(o) => {
            let on = &m.contexts[*o].name;
            tr!("「{n}」と「{on}」の共有カーネル", "the shared kernel of {n} and {on}")
        }
    }
}

/// Who may import group `to`, in the words of the map: a context whose groups may all import
/// it is named once; otherwise each of its groups is. A shared kernel is left out of the count:
/// it imports nothing but itself (DESIGN 7.1).
pub fn importers_phrase(m: &Model, areas: &Areas, to: usize) -> Text {
    let allowed = areas.importers(to);
    let mut parts: Vec<Text> = Vec::new();
    let mut ctxs: Vec<usize> = Vec::new();
    for &f in &allowed {
        if !ctxs.contains(&areas.areas[f].ctx) {
            ctxs.push(areas.areas[f].ctx);
        }
    }
    for c in ctxs {
        let counted = |i: usize| areas.areas[i].ctx == c && (!matches!(areas.areas[i].kind, Kind::Kernel(_)) || allowed.contains(&i));
        let all: Vec<usize> = (0..areas.areas.len()).filter(|&i| counted(i)).collect();
        let mine: Vec<usize> = all.iter().copied().filter(|i| allowed.contains(i)).collect();
        if mine.len() == all.len() {
            let n = &m.contexts[c].name;
            parts.push(tr!("「{n}」", "{n}"));
        } else {
            parts.extend(mine.iter().map(|&i| phrase(m, &areas.areas[i])));
        }
    }
    Text::list(&parts)
}

/// The sentence a setting describes group `to` with: who may import it. `verb` is the tool's word
/// for it (`import する` and `imported`; ArchUnit reads the dependencies of classes, `使う` and
/// `used`).
pub fn rule_text(m: &Model, areas: &Areas, to: usize, verb: &Text, lang: ritsu_base::text::Lang) -> String {
    let p = phrase(m, &areas.areas[to]);
    let who = importers_phrase(m, areas, to);
    ritsu_base::text::spaced(&tr!("{}は、{}だけが{}", "{} is {} only by {}", p.ja, who.ja, verb.ja; p.en, verb.en, who.en), lang)
}

fn e501(file: &str, line: usize, col: usize, src: &str, msg: Text, note: Text) -> Diag {
    diag::at("E501", file, line, col, msg).source(src).note(note)
}

/// The groups of `language` in a map that passed check, or why its settings cannot be written
/// (E501).
pub fn areas(c: &Checked, language: Language) -> Result<Areas, Vec<Diag>> {
    let m = &c.model;
    let lw = language.word();
    let Some(code) = m.map.code.iter().find(|x| x.language == lw) else {
        let h = &m.map.ast.heading;
        return Err(vec![
            diag::at("E501", &m.map.file, h.pos.line, h.pos.col, tr!("地図に `code {lw}` の行がありません", "The map has no `code {lw}` line"))
                .source(&m.map.src)
                .note(tr!(
                    "その言語のコードの置き場所を `code {lw} \"<パス>\"` で書くと、その下のコードの設定を書けます。",
                    "Write where the code of the language is with `code {lw} \"<path>\"`; the settings are for the code under it."
                )),
        ]);
    };
    let dir = code.path.clone();
    let mut diags = Vec::new();
    // Every directory the map names under the code directory, and what it is.
    let mut roots: Vec<(String, usize, Kind)> = Vec::new();
    let under = |p: &str| paths::contains(&dir, p);
    let rel = |p: &str| paths::relative(&dir, p);
    let exts = owners::code_extensions(lw);
    let is_code = |p: &str| p.rsplit_once('.').is_some_and(|(_, e)| exts.contains(&e));
    let file_entry = |diags: &mut Vec<Diag>, ci: usize, o: &crate::model::Own| {
        let cx = &m.contexts[ci];
        let t = o.text();
        diags.push(e501(
            &cx.file,
            o.pos.line,
            o.pos.col,
            &cx.src,
            tr!("{t} はコードのファイルを一つ名指していて、import の検査の設定に書けません", "The entry {t} names one file of code, and the settings of an import linter cannot hold it"),
            tr!(
                "import の検査の設定は、ディレクトリの単位で書きます（Go と Java では、パッケージがディレクトリです）。`dir \"…\"` で書きます。",
                "The settings of the import linters are written by directories (in Go and Java a package is one); write it with `dir \"…\"`."
            ),
        ));
    };
    for (ci, cx) in m.contexts.iter().enumerate() {
        for o in &cx.owns {
            if o.is_dir() {
                if under(&o.path) {
                    roots.push((rel(&o.path), ci, Kind::Internal));
                } else if paths::contains(&o.path, &dir) {
                    roots.push((".".to_string(), ci, Kind::Internal));
                }
            } else if under(&o.path) && is_code(&o.path) {
                file_entry(&mut diags, ci, o);
            }
        }
        for p in &cx.published {
            for (g, _) in &p.generated {
                if under(g) {
                    roots.push((rel(g), ci, Kind::Published(p.package.clone())));
                }
            }
        }
        for r in &cx.rels {
            match &r.kind {
                RelK::Upstream { layer, .. } => {
                    for o in layer {
                        if o.is_dir() && under(&o.path) {
                            roots.push((rel(&o.path), ci, Kind::Layer(r.partner)));
                        } else if !o.is_dir() && under(&o.path) && is_code(&o.path) {
                            file_entry(&mut diags, ci, o);
                        }
                    }
                }
                RelK::Kernel(items) => {
                    for o in items {
                        if o.is_dir() && under(&o.path) {
                            // A kernel is written on both sides; it belongs to the side that owns it.
                            let owner = owners::context_of(m, &o.path).unwrap_or(ci);
                            let other = if owner == ci { r.partner } else { ci };
                            let rp = rel(&o.path);
                            if !roots.iter().any(|(p, _, k)| *p == rp && matches!(k, Kind::Kernel(_))) {
                                roots.push((rp, owner, Kind::Kernel(other)));
                            }
                        } else if !o.is_dir() && under(&o.path) && is_code(&o.path) {
                            file_entry(&mut diags, ci, o);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    // The code files, each to the deepest directory that holds it.
    let files: Vec<String> = c.artifacts.iter().filter(|a| a.tool == Tool::File && under(&a.path) && is_code(&a.path)).map(|a| rel(&a.path)).collect();
    let mut held: Vec<Vec<String>> = vec![Vec::new(); roots.len()];
    for f in &files {
        let best = roots
            .iter()
            .enumerate()
            .filter(|(_, (r, _, _))| paths::contains(r, f))
            .max_by_key(|(_, (r, _, k))| (paths::depth(r), k.rank()))
            .map(|(i, _)| i);
        if let Some(i) = best {
            held[i].push(f.clone());
        }
    }
    // The groups, in the order of the contexts and of the kinds.
    let mut out: Vec<Area> = Vec::new();
    for (i, (r, ci, k)) in roots.iter().enumerate() {
        if held[i].is_empty() {
            continue;
        }
        match out.iter_mut().find(|a| a.ctx == *ci && a.kind == *k) {
            Some(a) => {
                if !a.roots.contains(r) {
                    a.roots.push(r.clone());
                }
                a.files.extend(held[i].iter().cloned());
            }
            None => out.push(Area { ctx: *ci, kind: k.clone(), roots: vec![r.clone()], files: held[i].clone() }),
        }
    }
    for a in &mut out {
        a.roots.sort();
        a.files.sort();
    }
    out.sort_by(|a, b| (a.ctx, &a.kind).cmp(&(b.ctx, &b.kind)));
    if out.is_empty() && diags.is_empty() {
        let line = m.map.ast.code.iter().find(|x| x.language == lw).map(|x| x.pos).unwrap_or_default();
        let d = dir.clone();
        let sd = paths::shown(&d);
        diags.push(e501(
            &m.map.file,
            line.line,
            line.col,
            &m.map.src,
            tr!("{sd} の下に、どのコンテキストのコードもありません", "No context has code under {sd}"),
            tr!(
                "コンテキストの `owns` に、この置き場所の下のディレクトリを書くと、その設定を書けます。",
                "Write directories under it in the `owns` of the contexts; the settings are for those."
            ),
        ));
    }
    // A directory's name has to be a module of the language.
    if matches!(language, Language::Python | Language::Java) {
        for a in &out {
            for r in &a.roots {
                for part in r.split('/').filter(|p| *p != ".") {
                    if !module_word(language, part) {
                        let (cx, l, col) = declared(m, a, &paths::join(&dir, r).unwrap_or_default());
                        let shown = paths::shown(&paths::join(&dir, r).unwrap_or_default());
                        let lang = if language == Language::Python { "Python" } else { "Java" };
                        diags.push(e501(
                            &m.contexts[cx].file,
                            l,
                            col,
                            &m.contexts[cx].src,
                            tr!("ディレクトリ {shown} の名前 `{part}` は、{lang} のモジュールの名前になりません", "The directory {shown}: `{part}` is not a name a {lang} module can have"),
                            tr!(
                                "モジュールの名前は文字か `_` で始まり、文字、数字、`_` だけからなり、予約語ではありません。",
                                "A module's name starts with a letter or `_`, has letters, digits and `_` only, and is not a keyword."
                            ),
                        ));
                    }
                }
            }
        }
    }
    if !diags.is_empty() {
        return Err(diags);
    }
    let except: Vec<String> = m.map.except.iter().filter(|e| under(e)).map(|e| rel(e)).collect();
    let allowed = allowed_table(c, &out);
    Ok(Areas { language, dir, areas: out, allowed, except })
}

/// Where the map names a group's directory: the context file, line and column.
fn declared(m: &Model, a: &Area, path: &str) -> (usize, usize, usize) {
    let cx = &m.contexts[a.ctx];
    let pos = match &a.kind {
        Kind::Internal => cx.owns.iter().find(|o| o.path == path).map(|o| o.pos),
        Kind::Published(p) => cx.published.iter().filter(|x| x.package == *p).flat_map(|x| x.generated.iter()).find(|(g, _)| g == path).map(|(_, pos)| *pos),
        Kind::Layer(u) => cx.rels.iter().find_map(|r| match &r.kind {
            RelK::Upstream { layer, .. } if r.partner == *u => layer.iter().find(|o| o.path == path).map(|o| o.pos),
            _ => None,
        }),
        Kind::Kernel(_) => cx.rels.iter().find_map(|r| match &r.kind {
            RelK::Kernel(items) => items.iter().find(|o| o.path == path).map(|o| o.pos),
            _ => None,
        }),
    };
    let p = pos.unwrap_or(cx.ast.heading.pos);
    (a.ctx, p.line, p.col)
}

const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
    "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield",
];

const JAVA_KEYWORDS: &[&str] = &[
    "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char", "class", "const", "continue", "default", "do", "double", "else", "enum", "extends", "final",
    "finally", "float", "for", "goto", "if", "implements", "import", "instanceof", "int", "interface", "long", "native", "new", "package", "private", "protected", "public",
    "return", "short", "static", "strictfp", "super", "switch", "synchronized", "this", "throw", "throws", "transient", "try", "void", "volatile", "while", "true", "false",
    "null", "_",
];

/// Whether a directory's name can be a module (Python) or a package (Java) name.
pub fn module_word(language: Language, w: &str) -> bool {
    let mut cs = w.chars();
    let Some(first) = cs.next() else { return false };
    let shape = (first.is_alphabetic() || first == '_') && cs.all(|c| c.is_alphanumeric() || c == '_');
    let keyword = match language {
        Language::Python => PYTHON_KEYWORDS.contains(&w),
        Language::Java => JAVA_KEYWORDS.contains(&w),
        _ => false,
    };
    shape && !keyword
}

/// Which group may import which (DESIGN 7.1).
fn allowed_table(c: &Checked, areas: &[Area]) -> Vec<Vec<bool>> {
    let m = &c.model;
    let n = areas.len();
    let mut t = vec![vec![false; n]; n];
    for (f, from) in areas.iter().enumerate() {
        for (g, to) in areas.iter().enumerate() {
            t[f][g] = f == g || may_import(m, c, from, to);
        }
    }
    t
}

fn may_import(m: &Model, c: &Checked, from: &Area, to: &Area) -> bool {
    let (x, y) = (from.ctx, to.ctx);
    // A shared kernel imports nothing but itself: depending on either side's inside would drag the
    // other team along with it.
    if matches!(from.kind, Kind::Kernel(_)) {
        return false;
    }
    match &to.kind {
        Kind::Kernel(other) => x == y || x == *other,
        Kind::Internal | Kind::Layer(_) => x == y,
        Kind::Published(pkg) => {
            if x == y {
                return true;
            }
            if let Kind::Published(fpkg) = &from.kind {
                // The code made from one published language imports another's when its proto imports
                // the other's, and the map allows that import.
                let from_files: Vec<&String> = m.contexts[x].published.iter().filter(|p| p.package == *fpkg).flat_map(|p| p.protos.iter().map(|(f, _)| f)).collect();
                let to_files: Vec<&String> = m.contexts[y].published.iter().filter(|p| p.package == *pkg).flat_map(|p| p.protos.iter().map(|(f, _)| f)).collect();
                return c.crossings.iter().any(|cr| cr.allowed.is_some() && from_files.contains(&&cr.from) && to_files.contains(&&cr.to));
            }
            let partners = m.writes(x, y, |k| matches!(k, RelK::Partnership)) && m.writes(y, x, |k| matches!(k, RelK::Partnership));
            if partners {
                return true;
            }
            let Some(r) = m.upstream(x, y) else { return false };
            let RelK::Upstream { through, layer, .. } = &r.kind else { return false };
            if !through.iter().any(|(p, _)| p == pkg) {
                return false;
            }
            if r.has(Role::Acl) && !layer.is_empty() {
                // Only the layer, when the anticorruption layer writes one.
                return from.kind == Kind::Layer(y);
            }
            true
        }
    }
}
