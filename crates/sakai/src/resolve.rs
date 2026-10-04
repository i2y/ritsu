//! Reading a map and its contexts into a [`Model`] (PLAN B.3): the files `use context` names,
//! every path made a path from the root (E009, E011, E012), the names that must differ (E006),
//! the aliases (E008), the partners of the relationships (E007), and what `use context` may name
//! (E010). The things a name points at inside an artifact are resolved later, once the `.proto`
//! files are read (`elements.rs`).

use crate::ast::*;
use crate::diag::{self, Diag};
use ritsu_base::text::Text;
use crate::model::*;
use crate::naming::Tool;
use crate::parse;
use crate::paths;
use std::path::Path;

/// What reading a map came to.
pub struct Loaded {
    pub model: Option<Model>,
    pub diags: Vec<Diag>,
    /// The context files the map names (from the root), read or not: a directory's check says of
    /// the others that no map reads them (W103).
    pub reads: Vec<String>,
}

struct R<'a> {
    root: &'a Path,
    diags: Vec<Diag>,
}

impl R<'_> {
    fn at(&mut self, code: &'static str, file: &str, src: &str, p: Pos, msg: Text) -> &mut Diag {
        self.diags.push(diag::at(code, file, p.line, p.col, msg).source(src));
        self.diags.last_mut().unwrap()
    }

    /// A path written in `file` (whose directory is `base`), as a path from the root (E012), that
    /// is on the disk (E009) and is a directory or a file as `dir` says (E011).
    fn path(&mut self, file: &str, src: &str, base: &str, s: &Str, dir: Option<bool>) -> Option<String> {
        let p = match paths::join(base, &s.value) {
            Ok(p) => p,
            Err(e) => {
                let d = self.at("E012", file, src, s.pos, paths::error_text(e, &s.value));
                if e != paths::PathError::Empty {
                    d.notes.push(root_note());
                }
                return None;
            }
        };
        let v = &s.value;
        match ritsu_base::fs::metadata(paths::on_disk(self.root, &p)) {
            Err(_) => {
                let sp = paths::shown(&p);
                self.at("E009", file, src, s.pos, tr!("パス \"{v}\" がありません", "The path \"{v}\" is not there")).notes.push(tr!(
                    "パスは、この .ctx のあるディレクトリからの相対パスです。書いてあるパスは {sp} を指します。",
                    "A path counts from the directory of this .ctx; as written, it points at {sp}."
                ));
                None
            }
            Ok(m) => {
                if dir == Some(true) && !m.is_dir() {
                    self.at("E011", file, src, s.pos, tr!("`dir` の先 \"{v}\" はファイルです", "The `dir` \"{v}\" is a file"))
                        .notes
                        .push(tr!("ファイルは `rulec \"…\"` のように、そのツールの語で書いてください。", "A file is written with its tool, like `rulec \"…\"`."));
                    return None;
                }
                if dir == Some(false) && m.is_dir() {
                    self.at("E011", file, src, s.pos, tr!("\"{v}\" はディレクトリです", "\"{v}\" is a directory")).notes.push(tr!("ディレクトリは `dir \"…\"` で書いてください。", "A directory is written `dir \"…\"`."));
                    return None;
                }
                Some(p)
            }
        }
    }

    /// A file or directory of `owns`, `layer` or `shared kernel with`.
    fn item(&mut self, file: &str, src: &str, base: &str, it: &Item) -> Option<Own> {
        let p = self.path(file, src, base, &it.path, Some(it.tool.is_none()))?;
        if let Some(t) = it.tool
            && let Some(ext) = t.extension()
            && !p.ends_with(&format!(".{ext}"))
        {
            let w = t.word();
            let v = &it.path.value;
            self.at("E011", file, src, it.path.pos, tr!("{w} の成果物は .{ext} のファイルです（\"{v}\"）", "A {w} artifact is a .{ext} file (\"{v}\")"));
            return None;
        }
        Some(Own { path: p, tool: it.tool, pos: it.path.pos })
    }
}

fn root_note() -> Text {
    tr!(
        "ルートは、sakai に渡したパスの上で .git を持つ一番近いディレクトリです（無ければ渡したディレクトリ。--root で替えられます）。",
        "The root is the nearest directory above the path given to sakai that holds .git (else the directory given; --root changes it)."
    )
}

/// Read the map `map` (a path from the root) and the contexts it names.
pub fn load(root: &Path, map: &str) -> Result<Loaded, Text> {
    let shown_map = paths::shown(map);
    let src = ritsu_base::fs::read_to_string(paths::on_disk(root, map)).map_err(|e| {
        let e = e.to_string();
        tr!("{shown_map} を読めません: {e}", "{shown_map} cannot be read: {e}")
    })?;
    let mut r = R { root, diags: Vec::new() };
    let mut reads = Vec::new();
    let (f, ds) = parse::parse(map, &src);
    r.diags.extend(ds);
    let ast = match f {
        Some(File::Map(m)) => m,
        Some(File::Context(_)) => return Err(tr!("{shown_map} は context のファイルです。map のファイルを渡してください", "{shown_map} is a context file; give a map file")),
        None => return Ok(Loaded { model: None, diags: r.diags, reads }),
    };
    if !r.diags.is_empty() {
        // The map does not read, but the files it names are named.
        reads = ast.uses.iter().filter_map(|u| paths::join(&paths::parent(map), &u.value).ok()).collect();
        return Ok(Loaded { model: None, diags: r.diags, reads });
    }
    let dir = paths::parent(map);
    let mut covers = Vec::new();
    for s in &ast.covers {
        covers.extend(r.path(map, &src, &dir, s, None));
    }
    let mut except = Vec::new();
    for s in &ast.except {
        except.extend(r.path(map, &src, &dir, s, None));
    }
    let mut proto_roots = Vec::new();
    for s in &ast.proto_roots {
        proto_roots.extend(r.path(map, &src, &dir, s, Some(true)));
    }
    let mut code = Vec::new();
    for c in &ast.code {
        let p = r.path(map, &src, &dir, &c.path, Some(true));
        let t = c.test.as_ref().and_then(|t| r.path(map, &src, &dir, t, Some(true)));
        if let Some(p) = p {
            code.push(CodeDir { language: c.language.clone(), path: p, test: t });
        }
    }
    // The contexts.
    let mut files: Vec<(String, String, ContextFile)> = Vec::new();
    for u in &ast.uses {
        let Some(p) = r.path(map, &src, &dir, u, Some(false)) else { continue };
        reads.push(p.clone());
        let sp = paths::shown(&p);
        if files.iter().any(|(f, _, _)| *f == p) {
            r.at("E010", map, &src, u.pos, tr!("{sp} を二度読んでいます", "The map reads {sp} twice")).notes.push(tr!("`use context` で同じファイルを読むのは、一度だけにしてください。", "`use context` names a file once."));
            continue;
        }
        let csrc = match ritsu_base::fs::read_to_string(paths::on_disk(root, &p)) {
            Ok(s) => s,
            Err(e) => {
                let e = e.to_string();
                r.at("E009", map, &src, u.pos, tr!("{sp} を読めません: {e}", "The context file {sp} cannot be read: {e}"));
                continue;
            }
        };
        let (cf, cds) = parse::parse(&p, &csrc);
        r.diags.extend(cds);
        match cf {
            Some(File::Context(c)) => files.push((p, csrc, c)),
            Some(File::Map(_)) => {
                r.at("E010", map, &src, u.pos, tr!("`use context` の先 {sp} は、context のファイルではなく map のファイルです", "What `use context` names, {sp}, is a map file, not a context file"))
                    .notes
                    .push(tr!("地図から、ほかの地図は読めません。", "A map does not read another map."));
            }
            None => {}
        }
    }
    if !r.diags.is_empty() {
        return Ok(Loaded { model: None, diags: r.diags, reads });
    }
    let names: Vec<String> = files.iter().map(|(_, _, c)| c.heading.name.clone()).collect();
    // E006: the names and aliases of the contexts all differ.
    for (i, (f, s, c)) in files.iter().enumerate() {
        let alias = c.heading.alias.clone().unwrap_or_default();
        for (j, (fj, _, cj)) in files[..i].iter().enumerate() {
            let aj = cj.heading.alias.clone().unwrap_or_default();
            let n = &c.heading.name;
            let clash = if *n == cj.heading.name {
                Some(tr!("コンテキストの名前「{n}」が二度宣言されています", "The context name {n} is declared twice"))
            } else if alias == aj {
                Some(tr!("別名 {alias} が二度宣言されています", "The alias {alias} is declared twice"))
            } else if *n == aj || alias == cj.heading.name {
                Some(tr!("コンテキストの名前と別名がぶつかっています（{n}、{alias}）", "A context's name and alias collide ({n}, {alias})"))
            } else {
                None
            };
            if let Some(m) = clash {
                let other = &files[j].2.heading.name;
                let fj = paths::shown(fj);
                r.at("E006", f, s, c.heading.name_pos, m).notes.push(tr!(
                    "{fj} の「{other}」とぶつかります。一つの地図の中で、コンテキストの名前と別名はどれも違うものにしてください。",
                    "It collides with {other} of {fj}; in one map, the names and aliases of the contexts all differ."
                ));
            }
        }
    }
    let mut contexts: Vec<Ctx> = Vec::new();
    let mut packages: Vec<(String, String)> = Vec::new();
    for (f, s, c) in &files {
        let cdir = paths::parent(f);
        let mut owns = Vec::new();
        for it in &c.owns {
            if let Some(o) = r.item(f, s, &cdir, it) {
                if owns.iter().any(|x: &Own| x.path == o.path) {
                    let t = o.text();
                    r.at("E006", f, s, it.path.pos, tr!("`owns` に {t} が二度あります", "`owns` lists {t} twice"));
                    continue;
                }
                owns.push(o);
            }
        }
        let mut published = Vec::new();
        for p in &c.published {
            if let Some((_, other)) = packages.iter().find(|(k, _)| *k == p.package) {
                let k = &p.package;
                let other = paths::shown(other);
                r.at("E006", f, s, p.package_pos, tr!("公表された言語 {k} が二度宣言されています", "The published language {k} is declared twice"))
                    .notes
                    .push(tr!("{other} も同じ package を公表しています。一つの package を公表できるのは、一つのコンテキストだけです。", "The context file {other} publishes the same package; a package is published by one context."));
                continue;
            }
            packages.push((p.package.clone(), f.clone()));
            let mut protos = Vec::new();
            for pf in &p.protos {
                if let Some(x) = r.path(f, s, &cdir, pf, Some(false)) {
                    if !x.ends_with(".proto") {
                        let v = &pf.value;
                        r.at("E011", f, s, pf.pos, tr!("proto の行に書けるのは .proto のファイルです（\"{v}\"）", "A proto line names a .proto file (\"{v}\")"));
                        continue;
                    }
                    protos.push((x, pf.pos));
                }
            }
            let rulec = p.rulec.as_ref().and_then(|rf| {
                let x = r.path(f, s, &cdir, rf, Some(false))?;
                if !x.ends_with(".rule") {
                    let v = &rf.value;
                    r.at("E011", f, s, rf.pos, tr!("rulec の行に書けるのは .rule のファイルです（\"{v}\"）", "A rulec line names a .rule file (\"{v}\")"));
                    return None;
                }
                Some((x, rf.pos))
            });
            let krate = p.krate.as_ref().and_then(|kf| {
                let x = r.path(f, s, &cdir, kf, None)?;
                if !ritsu_base::fs::is_dir(paths::on_disk(r.root, &x)) {
                    let v = &kf.value;
                    r.at("E011", f, s, kf.pos, tr!("crate の行に書けるのは、Rust のクレートのディレクトリです（\"{v}\" はファイルです）", "A crate line names the directory of a Rust crate (\"{v}\" is a file)"))
                        .notes
                        .push(tr!("`Cargo.toml` のあるディレクトリを書いてください。", "Write the directory its `Cargo.toml` is in."));
                    return None;
                }
                Some((x, kf.pos))
            });
            let mut generated = Vec::new();
            for g in &p.generated {
                if let Some(x) = r.path(f, s, &cdir, g, Some(true)) {
                    generated.push((x, g.pos));
                }
            }
            published.push(Pub { package: p.package.clone(), pos: p.pos, package_pos: p.package_pos, protos, rulec, krate, services: p.services.clone(), generated });
        }
        // E006: the terms and their other names all differ.
        let mut seen: Vec<String> = Vec::new();
        for t in &c.terms {
            let mut ns = vec![(t.name.clone(), t.pos)];
            ns.extend(t.also.iter().map(|a| (a.value.clone(), a.pos)));
            for (n, at) in ns {
                if seen.contains(&n) {
                    r.at("E006", f, s, at, tr!("語の名前「{n}」が二度宣言されています", "The term name {n} is declared twice"))
                        .notes
                        .push(tr!("一つのコンテキストの中で、語の名前と `also` の名前はどれも違うものにしてください。", "In one context, the names of the terms and their `also` names all differ."));
                } else {
                    seen.push(n);
                }
            }
            if let Some(a) = &t.as_term
                && !names.contains(&a.context)
            {
                let ac = &a.context;
                r.at("E007", f, s, a.pos, tr!("コンテキスト「{ac}」は地図にありません", "There is no context {ac} in the map")).notes.push(known(&names));
            }
            for e in &t.means {
                if let Element::Long { written, pos } = e {
                    r.element_path(f, s, &cdir, written, *pos);
                }
            }
        }
        let mut rels = Vec::new();
        let mut kinds_seen: Vec<(String, &'static str)> = Vec::new();
        for rel in &c.relations {
            let Some(partner) = names.iter().position(|n| *n == rel.partner) else {
                let pn = &rel.partner;
                r.at("E007", f, s, rel.partner_pos, tr!("コンテキスト「{pn}」は地図にありません", "There is no context {pn} in the map")).notes.push(known(&names));
                continue;
            };
            let kind = match &rel.kind {
                RelKind::Upstream(u) => {
                    let mut layer = Vec::new();
                    for it in &u.layer {
                        layer.extend(r.item(f, s, &cdir, it));
                    }
                    let mut froms: Vec<&str> = Vec::new();
                    for em in &u.enums {
                        if froms.contains(&em.from.as_str()) {
                            let x = &em.from;
                            r.at("E006", f, s, em.from_pos, tr!("列挙 {x} の対応が二度あります", "The enum {x} is mapped twice"));
                        }
                        froms.push(&em.from);
                        let mut vs: Vec<&str> = Vec::new();
                        for v in &em.values {
                            if vs.contains(&v.from.as_str()) {
                                let x = &v.from;
                                r.at("E006", f, s, v.from_pos, tr!("値 {x} の対応が二度あります", "The value {x} is mapped twice"));
                            }
                            vs.push(&v.from);
                        }
                        if let Target::Element(Element::Long { written, pos }) = &em.target {
                            r.element_path(f, s, &cdir, written, *pos);
                        }
                    }
                    let mut ts: Vec<&str> = Vec::new();
                    for tm in &u.terms {
                        if ts.contains(&tm.from.as_str()) {
                            let x = &tm.from;
                            r.at("E006", f, s, tm.from_pos, tr!("語「{x}」の対応が二度あります", "The term {x} is mapped twice"));
                        }
                        ts.push(&tm.from);
                    }
                    RelK::Upstream { roles: u.roles.clone(), through: u.through.clone(), layer, enums: u.enums.clone(), terms: u.terms.clone() }
                }
                RelKind::Downstream => RelK::Downstream,
                RelKind::SharedKernel(items) => {
                    let mut os = Vec::new();
                    for it in items {
                        os.extend(r.item(f, s, &cdir, it));
                    }
                    RelK::Kernel(os)
                }
                RelKind::Partnership => RelK::Partnership,
                RelKind::SeparateWays => RelK::Separate,
            };
            let words = kind.words();
            if kinds_seen.iter().any(|(p, w)| *p == rel.partner && *w == words) {
                let pn = &rel.partner;
                r.at("E006", f, s, rel.pos, tr!("「{pn}」との `{words}` が二度あります", "There are two `{words}` toward {pn}"));
                continue;
            }
            kinds_seen.push((rel.partner.clone(), words));
            rels.push(Rel { partner, kind, pos: rel.pos, partner_pos: rel.partner_pos });
        }
        contexts.push(Ctx { name: c.heading.name.clone(), alias: c.heading.alias.clone().unwrap_or_default(), file: f.clone(), dir: cdir, src: s.clone(), ast: c.clone(), owns, published, rels });
    }
    if !r.diags.is_empty() {
        return Ok(Loaded { model: None, diags: r.diags, reads });
    }
    let model = Model { root: root.to_path_buf(), map: MapInfo { file: map.to_string(), dir, src, ast, covers, except, proto_roots, code }, contexts };
    Ok(Loaded { model: Some(model), diags: r.diags, reads })
}

impl R<'_> {
    /// The path of a name in its long form (DESIGN 2.1), held to the tool's extension.
    fn element_path(&mut self, file: &str, src: &str, base: &str, w: &crate::naming::Written, pos: Pos) {
        let s = Str { value: w.path.clone(), pos: Pos { line: pos.line, col: pos.col + w.path_at } };
        let Some(p) = self.path(file, src, base, &s, Some(false)) else { return };
        if let Some(ext) = w.tool.extension()
            && !p.ends_with(&format!(".{ext}"))
        {
            let t = w.tool.word();
            let v = &w.path;
            self.at("E011", file, src, s.pos, tr!("{t} の成果物は .{ext} のファイルです（\"{v}\"）", "A {t} artifact is a .{ext} file (\"{v}\")"));
        }
    }
}

fn known(names: &[String]) -> Text {
    let ja = names.iter().map(|n| format!("「{n}」")).collect::<Vec<_>>().join("");
    let en = names.join(", ");
    tr!("地図が読むコンテキストは{ja}です。", "The map reads the contexts {en}.")
}

/// The tool whose artifact a file is, by its extension, if it is one of the suite's.
pub fn tool_of(path: &str) -> Option<Tool> {
    let ext = path.rsplit_once('.').map(|(_, e)| e)?;
    Tool::ALL.into_iter().find(|t| t.extension() == Some(ext) && !matches!(t, Tool::Yuen | Tool::Sakai))
}
