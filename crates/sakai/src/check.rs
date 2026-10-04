//! `sakai check` (DESIGN 3.1; PLAN B.11): the stages in their order, and the line that says what
//! was checked when nothing is wrong.
//!
//! 1. the words, the sections, the names and the paths (E001 to E012);
//! 2. who owns what (E101 to E103, W101);
//! 3. the `.proto` files (E106, W102, E103), what the other languages say of their artifacts
//!    through ritsu's ports (E104, E105), the crates of the Rust code as Cargo says them (E107),
//!    then the elements the map names (E007, E011);
//! 4. the patterns (E301 to E313, W301);
//! 5. the references that cross a boundary (E201 to E209);
//! 6. the mappings and the glossaries (E401 to E410, W401, W402).
//!
//! An error of stage 1 or 2 stops the stages after it: without the names, or without knowing
//! who owns what, there is nothing to hold the references to. From stage 3 on every stage runs,
//! on what could be read, so one unreadable file does not hide the rest of the map.

use crate::diag::{self, Diag, has_errors};
use crate::elements::{self, Elements};
use ritsu_base::text::{Lang, Text, spaced};
use crate::model::{Model, RelK};
use crate::owners::{self, Artifact};
use crate::paths::shown;
use crate::proto::{self, Issue, Protos};
use crate::refs::{self, Crossing};
use crate::resolve;
use crate::{mapping, patterns, terms};
use serde_json::{Value, json};
use std::path::Path;

/// What checking one map found.
pub struct Outcome {
    /// The map, from the root.
    pub file: String,
    pub diags: Vec<Diag>,
    /// What was checked, when nothing is wrong.
    pub summary: Option<Text>,
    pub checked: Option<Checked>,
    /// The context files the map names.
    pub reads: Vec<String>,
}

impl Outcome {
    pub fn has_errors(&self) -> bool {
        has_errors(&self.diags)
    }
}

/// Everything the stages found, for `api`.
pub struct Checked {
    pub model: Model,
    pub artifacts: Vec<Artifact>,
    pub protos: Protos,
    pub elements: Elements,
    pub crossings: Vec<Crossing>,
    /// What the other languages said of their artifacts.
    pub read: crate::suite::Read,
}

fn sorted(mut ds: Vec<Diag>) -> Vec<Diag> {
    ds.sort_by(|a, b| (&a.file, a.line, a.col).cmp(&(&b.file, b.line, b.col)));
    ds
}

/// The diagnostics of reading the `.proto` files.
fn proto_diags(m: &Model, issues: &[Issue]) -> Vec<Diag> {
    let src = |f: &str| ritsu_base::fs::read_to_string(crate::paths::on_disk(&m.root, f)).unwrap_or_default();
    let mut out = Vec::new();
    for i in issues {
        match i {
            Issue::Unreadable { file, err } => {
                let msg = &err.message("sakai");
                let f = shown(file);
                out.push(diag::at("E106", file, err.line, err.col, tr!("{f} を読めません: {}", "The file {f} cannot be read: {}", msg.ja; msg.en)).source(&src(file)));
            }
            Issue::NotFound { file, import, tried } => {
                let ip = &import.path;
                let f = shown(file);
                let tried: Vec<String> = tried.iter().map(|t| shown(t)).collect();
                let (tj, te) = (tried.join("、"), tried.join(", "));
                out.push(
                    diag::at("W102", file, import.line, import.col, tr!("{f} の import \"{ip}\" が見つかりません。範囲の外のものとして扱います", "The import \"{ip}\" of {f} is not found; it is taken as outside the scope"))
                        .source(&src(file))
                        .note(tr!("探した場所: {tj}", "looked for at: {te}"))
                        .note(tr!(
                            "そのファイルの型は分からないものとして、参照の検査から外します。地図の `proto root` に、import を探すディレクトリを書けます。",
                            "The types of that file are not known, and are left out of the check of the references; the map's `proto root` says where imports are looked for."
                        )),
                );
            }
            Issue::OutOfScope { file, import, at } => {
                let (f, at) = (shown(file), shown(at));
                out.push(
                    diag::at("E103", file, import.line, import.col, tr!("{f} が、地図の範囲の外の {at} を import しています", "The file {f} imports {at}, which is outside the map's scope"))
                        .source(&src(file))
                        .note(owners::scope_note(m)),
                );
            }
        }
    }
    out
}

/// How many relationships the map has: an upstream relationship is one (a customer's with the
/// supplier's word for it), and a shared kernel, a partnership or separate ways is one a pair.
pub fn relationship_count(m: &Model) -> usize {
    let mut n = 0;
    let mut pairs: Vec<(usize, usize, &'static str)> = Vec::new();
    for (ci, c) in m.contexts.iter().enumerate() {
        for r in &c.rels {
            match &r.kind {
                RelK::Upstream { .. } => n += 1,
                RelK::Downstream => {}
                k => {
                    let key = (ci.min(r.partner), ci.max(r.partner), k.words());
                    if !pairs.contains(&key) {
                        pairs.push(key);
                        n += 1;
                    }
                }
            }
        }
    }
    n
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

/// The line `check` prints when nothing is wrong (DESIGN 3.1): the crossings counted by the
/// language of the artifact that refers, in the order proto, rulec, koyomi, dandori, rust (the
/// manifests of Rust's crates).
pub fn summary(m: &Model, arts: &[Artifact], crossings: &[Crossing]) -> Text {
    use crate::naming::Tool;
    let (c, r, a, x) = (m.contexts.len(), relationship_count(m), arts.len(), crossings.len());
    let mut by: Vec<String> = [Tool::Proto, Tool::Rulec, Tool::Koyomi, Tool::Dandori]
        .iter()
        .filter_map(|t| {
            let n = crossings.iter().filter(|c| c.tool() == *t).count();
            (n > 0).then(|| format!("{} {n}", t.word()))
        })
        .collect();
    let rust = crossings.iter().filter(|c| matches!(c.kind, crate::refs::Kind::Crate { .. })).count();
    if rust > 0 {
        by.push(format!("rust {rust}"));
    }
    let by = by.join(", ");
    let by_ja = by.replace(", ", "、");
    let en = format!(
        "{}, {}; {}, each in one context; {}",
        plural(c, "context", "contexts"),
        plural(r, "relationship", "relationships"),
        plural(a, "artifact", "artifacts"),
        if x == 0 { "no crossing to check".to_string() } else { format!("{} checked ({by})", plural(x, "crossing", "crossings")) }
    );
    let ja = format!(
        "コンテキスト {c}、関係 {r}。成果物 {a} 件は、どれも一つのコンテキストに属する。{}",
        if x == 0 { "境界を越える参照は無い".to_string() } else { format!("境界を越える参照 {x} 件を確かめた（{by_ja}）") }
    );
    Text::new(ja, en)
}

/// Check the map `map` (a path from `root`), with no other language joined.
pub fn check_map(root: &Path, map: &str) -> Result<Outcome, Text> {
    check_map_with(root, map, &crate::suite::Suite::default())
}

/// [`check_map`], reading what the other languages hold through the ports `suite` joins.
pub fn check_map_with(root: &Path, map: &str, suite: &crate::suite::Suite) -> Result<Outcome, Text> {
    let loaded = resolve::load(root, map)?;
    let reads = loaded.reads;
    let Some(m) = loaded.model else {
        return Ok(Outcome { file: map.to_string(), diags: sorted(loaded.diags), summary: None, checked: None, reads });
    };
    let mut diags = sorted(loaded.diags);
    // 2. Who owns what.
    let (arts, d2) = owners::own(&m);
    let stop = has_errors(&d2);
    diags.extend(sorted(d2));
    if stop {
        return Ok(Outcome { file: map.to_string(), diags, summary: None, checked: None, reads });
    }
    // 3. The `.proto` files, and the elements the map names.
    let proto_files: Vec<String> = arts.iter().filter(|a| a.tool == crate::naming::Tool::Proto).map(|a| a.path.clone()).collect();
    let (ps, issues) = proto::load(&m.root, &proto_files, &m.map.proto_roots);
    diags.extend(sorted(proto_diags(&m, &issues)));
    let (read, d3b) = crate::suite::read(&m, &arts, suite);
    diags.extend(sorted(d3b));
    let (crates, d3c) = crate::cargo::read(&m);
    diags.extend(sorted(d3c));
    let (el, d3) = elements::resolve(&m, &ps, &arts, &read);
    diags.extend(sorted(d3));
    // 4. The patterns.
    diags.extend(sorted(patterns::check(&m, &ps, &arts, &read, crates.as_ref())));
    // 5. The crossings.
    let mut crossings = refs::proto_crossings(&ps, &arts);
    let (more, d5) = refs::suite_crossings(&m, &ps, &arts, &read);
    crossings.extend(more);
    let (more, d5c) = refs::crate_crossings(&m, &arts, crates.as_ref());
    crossings.extend(more);
    let mut d5 = d5;
    d5.extend(d5c);
    d5.extend(refs::check(&m, &mut crossings, &read));
    d5.extend(refs::implements(&m, &arts, &read));
    diags.extend(sorted(d5));
    // 6. The mappings and the glossaries.
    let mut d6 = mapping::check(&m, &ps, &el, &crossings, &read);
    d6.extend(terms::check(&m, &ps, &el, &crossings, read.complete, &read));
    diags.extend(sorted(d6));
    let summary = (!has_errors(&diags)).then(|| summary(&m, &arts, &crossings));
    Ok(Outcome { file: map.to_string(), diags, summary, checked: Some(Checked { model: m, artifacts: arts, protos: ps, elements: el, crossings, read }), reads })
}

/// What a person reads.
pub fn render(o: &Outcome, lang: Lang) -> String {
    let mut s: String = o.diags.iter().map(|d| d.render(lang)).collect();
    if let Some(t) = &o.summary {
        s.push_str(&format!("{}: ok — {}\n", shown(&o.file), spaced(t, lang)));
    }
    s
}

/// The `--format json` of one map (DESIGN 5.1): every path from the root, and the root as seen
/// from where sakai runs.
pub fn to_json(o: &Outcome, lang: Lang) -> Value {
    json!({
        "root": crate::paths::shown_root(),
        "file": o.file,
        "ok": !o.has_errors(),
        "summary": o.summary.as_ref().map(|t| spaced(t, lang)),
        "diagnostics": o.diags.iter().map(|d| crate::diag::value(&d.to_json(lang))).collect::<Vec<_>>(),
    })
}

/// Every `.ctx` under a directory (a path from the root), in path order.
fn ctx_files(root: &Path, dir: &str) -> Vec<String> {
    let mut files = Vec::new();
    crate::paths::walk(root, dir, &[], &mut files);
    files.retain(|f| f.ends_with(".ctx"));
    files
}

/// Check what the command line names (DESIGN 6): a map file, or every map file under a
/// directory. Under a directory, a context file no map there reads gets W103, and a `.ctx`
/// that is neither a map nor a context gets what reading it says (E003). `args` are paths from
/// the root.
pub fn check_args(root: &Path, args: &[String]) -> Result<Vec<Outcome>, Text> {
    check_args_with(root, args, &crate::suite::Suite::default())
}

/// [`check_args`], reading what the other languages hold through the ports `suite` joins.
pub fn check_args_with(root: &Path, args: &[String], suite: &crate::suite::Suite) -> Result<Vec<Outcome>, Text> {
    let mut out = Vec::new();
    for a in args {
        let disk = crate::paths::on_disk(root, a);
        if ritsu_base::fs::is_dir(&disk) {
            let files = ctx_files(root, a);
            if files.is_empty() {
                let sa = shown(a);
                return Err(if sa == "." {
                    tr!("このディレクトリの下に .ctx のファイルがありません", "there is no .ctx file under this directory")
                } else {
                    tr!("{sa} の下に .ctx のファイルがありません", "there is no .ctx file under {sa}")
                });
            }
            let mut maps = Vec::new();
            let mut contexts = Vec::new();
            let mut neither = Vec::new();
            for f in &files {
                let src = ritsu_base::fs::read_to_string(crate::paths::on_disk(root, f)).map_err(|e| {
                    let (e, sf) = (e.to_string(), shown(f));
                    tr!("{sf} を読めません: {e}", "{sf} cannot be read: {e}")
                })?;
                match crate::parse::kind_of(&src) {
                    Some("map") => maps.push(f.clone()),
                    Some(_) => contexts.push((f.clone(), src)),
                    None => neither.push((f.clone(), src)),
                }
            }
            let mut reads: Vec<String> = Vec::new();
            let mut all_read = true;
            for m in &maps {
                let o = check_map_with(root, m, suite)?;
                all_read &= !(o.checked.is_none() && o.reads.is_empty());
                reads.extend(o.reads.iter().cloned());
                out.push(o);
            }
            // A file that is neither a map nor a context, and that no map reads, is told what reading
            // it says; one a map reads is told through that map.
            for (f, src) in neither {
                if !reads.contains(&f) {
                    let (_, ds) = crate::parse::parse(&f, &src);
                    out.push(Outcome { file: f, diags: ds, summary: None, checked: None, reads: vec![] });
                }
            }
            if all_read {
                for (f, src) in contexts {
                    if !reads.contains(&f) {
                        let sf = shown(&f);
                        let d = diag::at("W103", &f, 1, 1, tr!("{sf} を読む地図がありません", "No map reads {sf}")).source(&src).note(tr!(
                            "地図の `use context` に足すか、ファイルを消します。どの地図にも読まれないコンテキストは、検査されません。",
                            "Add it to a map's `use context`, or delete the file; a context no map reads is not checked."
                        ));
                        out.push(Outcome { file: f, diags: vec![d], summary: None, checked: None, reads: vec![] });
                    }
                }
            }
        } else {
            let sa = shown(a);
            let src = ritsu_base::fs::read_to_string(&disk).map_err(|e| {
                let e = e.to_string();
                tr!("{sa} を読めません: {e}", "{sa} cannot be read: {e}")
            })?;
            match crate::parse::kind_of(&src) {
                Some("map") => out.push(check_map_with(root, a, suite)?),
                Some(_) => return Err(tr!("{sa} は context のファイルです。map のファイルかディレクトリを渡します", "{sa} is a context file; give a map file, or a directory")),
                None => {
                    let (_, ds) = crate::parse::parse(a, &src);
                    out.push(Outcome { file: a.clone(), diags: ds, summary: None, checked: None, reads: vec![] });
                }
            }
        }
    }
    Ok(out)
}
