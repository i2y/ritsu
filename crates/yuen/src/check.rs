//! `yuen check` (DESIGN 5, PLAN B.9): the seven stages, in order.
//!
//! They are: words and lines (E001–E006); names (E007–E013, E403); sources (E101–E105,
//! W101); artifacts (E201); cycles and periods (E405–E409); links and hashes (E301–E305,
//! W301); coverage and scope (E401, E402, E404, W401). An error in the first two stops there:
//! until the names are known nothing can be compared. Errors of sources and artifacts let the
//! check go on, leaving out the links whose ends could not be read.

use crate::coverage::{self, ScopeResult};
use crate::diag::{self, Diag};
use crate::ends::{self, End, Unread};
use crate::graph;
use crate::i18n::{Lang, Text, plural};
use crate::marks::{self, Ctx, LinkKind, LinkState};
use crate::names::Name;
use crate::project::{self, Project, Refusal};
use crate::sources::{self, Resolved, Sources};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// What the check found, kept for the commands built on it (`review`, `trace`, `api`).
pub struct Model {
    pub sources: Sources,
    pub in_cycle: Vec<bool>,
    pub req_ends: Vec<Option<End>>,
    pub artifacts: BTreeMap<Name, Result<End, Unread>>,
    pub states: Vec<LinkState>,
    pub scopes: Vec<ScopeResult>,
}

pub struct Checked {
    pub project: Option<Project>,
    pub diags: Vec<Diag>,
    /// The last stage that ran.
    pub stage: u8,
    pub model: Option<Model>,
}

impl Checked {
    pub fn has_errors(&self) -> bool {
        diag::has_errors(&self.diags)
    }

    /// Whether the project's names were all known (stages 1 and 2 passed).
    pub fn named(&self) -> bool {
        self.model.is_some()
    }
}

/// What this yuen cannot read yet: the artifacts of the suite's tools, and the sources they
/// pin, are read by the stage of yuen that runs them (PLAN C). Until then a project that
/// names one is refused, rather than passed unchecked (DESIGN 2.3).
fn not_yet(p: &Project, tool: crate::names::Tool, what: &str, fi: usize, line: usize) -> Refusal {
    let t = tool.word();
    let at = format!("{}:{line}", p.files[fi].display);
    Refusal(tr!(
        "yuen はまだ {t} の成果物を読めません: {what}（{at}）。いまの yuen が読めるのは file の成果物と法令の写しです",
        "yuen cannot read {t} artifacts yet: {what} ({at}); for now it reads file artifacts and copies of laws"
    ))
}

pub fn check(args: &[String], root: Option<&str>) -> Result<Checked, Refusal> {
    let (project, d1) = project::load(args, root)?;
    let Some(mut p) = project else {
        return Ok(Checked { project: None, diags: d1, stage: 1, model: None });
    };
    let mut diags = d1;
    // 2. Names.
    let d2 = project::check_names(&mut p);
    let stop = diag::has_errors(&d2);
    diags.extend(d2);
    if stop {
        return Ok(Checked { project: Some(p), diags, stage: 2, model: None });
    }
    // 3. Sources.
    let (srcs, d3) = sources::check_sources(&p);
    diags.extend(d3);
    for (fi, f) in srcs.files.iter().enumerate() {
        for (si, (_, r)) in f.iter().enumerate() {
            if let Resolved::Borrowed { name } = r {
                let line = p.files[fi].ast.sources[si].span.line;
                return Err(not_yet(&p, name.tool, &name.text(), fi, line));
            }
        }
    }
    // 4. Artifacts.
    let mut artifacts: BTreeMap<Name, Result<End, Unread>> = BTreeMap::new();
    let mut d4 = Vec::new();
    for r in 0..p.reqs.len() {
        let fi = p.reqs[r].file;
        for (li, n) in p.names.links[r].iter().enumerate() {
            let Some(n) = n else { continue };
            let line = p.decl(r).links[li].span.line;
            let col = p.decl(r).links[li].naming.tool.col;
            let e = artifacts.entry(n.clone()).or_insert_with(|| ends::artifact_end(&p, n)).clone();
            match e {
                Err(Unread::NotYet(t)) => return Err(not_yet(&p, t, &n.text(), fi, line)),
                Err(Unread::Missing { dir }) => d4.push(missing(&p, fi, line, col, n, dir)),
                Ok(_) => {}
            }
        }
    }
    let mut scopes = Vec::new();
    for fi in 0..p.files.len() {
        for (si, sc) in p.names.scopes[fi].iter().enumerate() {
            let Some((n, kind)) = sc else { continue };
            let decl = &p.files[fi].ast.scopes[si];
            match coverage::gather(&p, n, kind.as_deref()) {
                Err(Unread::NotYet(t)) => {
                    let text = match kind {
                        Some(k) => format!("{} {k}", n.text()),
                        None => n.text(),
                    };
                    return Err(not_yet(&p, t, &text, fi, decl.span.line));
                }
                Err(Unread::Missing { .. }) => {
                    let path = p.shown(&n.path);
                    d4.push(p.err(fi, "E201", crate::ast::Span { line: decl.span.line, col: decl.naming.path.as_ref().map(|w| w.col).unwrap_or(1) }, tr!("範囲のパス {path} がありません", "The path {path} of the scope is not there")));
                }
                Ok(items) => {
                    let text = match kind {
                        Some(k) => format!("{} {k}", n.text()),
                        None => n.text(),
                    };
                    scopes.push(ScopeResult { file: fi, idx: si, text, kind: kind.clone(), artifacts: items, untraced: vec![] });
                }
            }
        }
    }
    diags.extend(d4);
    // 5. Cycles and periods.
    let (d5, in_cycle) = graph::cycles(&p);
    diags.extend(d5);
    diags.extend(graph::periods(&p));
    // 6. Links and hashes.
    let req_ends = ends::requirement_ends(&p, &srcs, &in_cycle);
    let lookup = |n: &Name| artifacts.get(n).and_then(|e| e.as_ref().ok()).cloned();
    let states = marks::link_states(&p, &srcs, &req_ends, &lookup);
    diags.extend(marks::mark_diags(&p, &Ctx { sources: &srcs, req_ends: &req_ends }, &states));
    // 7. Coverage and scope.
    diags.extend(coverage::coverage(&p));
    let links: Vec<&Name> = p.names.links.iter().flatten().flatten().collect();
    for s in &mut scopes {
        s.untraced = s.artifacts.iter().filter(|a| !coverage::traced(a, &links)).cloned().collect();
    }
    diags.extend(coverage::scope_diags(&p, &scopes));
    let model = Model { sources: srcs, in_cycle, req_ends, artifacts, states, scopes };
    Ok(Checked { project: Some(p), diags, stage: 7, model: Some(model) })
}

fn missing(p: &Project, fi: usize, line: usize, col: usize, n: &Name, dir: bool) -> Diag {
    let t = n.text();
    let span = crate::ast::Span { line, col };
    if dir {
        p.err(fi, "E201", span, tr!("{t} はディレクトリです。リンクが名指すのはファイルです", "{t} is a directory; a link names a file")).note(tr!(
            "ディレクトリを書けるのは `scope` だけです（`scope file \"src/\"`）。",
            "Only a `scope` takes a directory (`scope file \"src/\"`)."
        ))
    } else {
        p.err(fi, "E201", span, tr!("{t} がありません", "{t} is not there")).note(tr!(
            "パスは、この .req のあるディレクトリからの相対で書きます。名前を変えたのなら、リンクも直します。",
            "The path is written from the directory of this .req; if the file was renamed, correct the link."
        ))
    }
}

/// The counts the summary says.
pub struct Counts {
    pub names: usize,
    pub versions: usize,
    pub links: usize,
    pub waivers: usize,
    pub in_scope: usize,
    pub scope_kind: Option<String>,
}

pub fn counts(c: &Checked) -> Option<Counts> {
    let p = c.project.as_ref()?;
    let m = c.model.as_ref()?;
    let links = m.states.iter().filter(|s| !s.is_waiver()).count();
    let waivers = m.states.iter().filter(|s| s.is_waiver()).count();
    let in_scope: usize = m.scopes.iter().map(|s| s.artifacts.len()).sum();
    let kinds: Vec<Option<String>> = m.scopes.iter().map(|s| s.kind.clone()).collect();
    let scope_kind = if kinds.windows(2).all(|w| w[0] == w[1]) { kinds.first().cloned().flatten() } else { Some(String::new()) };
    Some(Counts { names: p.by_name.len(), versions: p.reqs.len(), links, waivers, in_scope, scope_kind })
}

/// What a scope's artifacts are called: the Japanese noun, and the English one for one and
/// for more. Kinds are the tools' own words (`date`, `output`).
fn scope_noun(kind: &Option<String>) -> (String, String, String) {
    match kind.as_deref() {
        None => ("ファイル".into(), "file".into(), "files".into()),
        Some("") => ("成果物".into(), "artifact".into(), "artifacts".into()),
        Some(k) => (k.to_string(), k.to_string(), format!("{k}s")),
    }
}

/// The line after the diagnostics: what was checked, or how many errors and warnings.
pub fn summary(c: &Checked, label: &str) -> Text {
    let (e, w) = diag::count(&c.diags);
    if e > 0 {
        let wj = if w > 0 { format!("、警告 {w} 件") } else { String::new() };
        let we = if w > 0 { format!(", {}", plural(w, "warning", "warnings")) } else { String::new() };
        return tr!("{label}: エラー {e} 件{wj}", "{label}: {}{we}", ; plural(e, "error", "errors"));
    }
    let Some(k) = counts(c) else { return tr!("{label}: ok", "{label}: ok") };
    let reqs = if k.names == k.versions {
        tr!("要件 {} 件", "{}", k.names; plural(k.names, "requirement", "requirements"))
    } else {
        tr!("要件 {} 件（版は {} 個）", "{} ({} versions)", k.names, k.versions; plural(k.names, "requirement", "requirements"), k.versions)
    };
    let what = match (k.links, k.waivers) {
        (l, 0) => tr!("リンク {l} 本", "{}", ; plural(l, "link", "links")),
        (0, w) => tr!("見送り {w} 件", "{}", ; plural(w, "waiver", "waivers")),
        (l, w) => tr!("リンク {l} 本と見送り {w} 件", "{} and {}", ; plural(l, "link", "links"), plural(w, "waiver", "waivers")),
    };
    let warn = if w > 0 { tr!("（警告 {w} 件）", " ({})", ; plural(w, "warning", "warnings")) } else { Text::default() };
    let head = tr!(
        "{label}: ok — {}の{}が、確かめたときのままです。どの要件にも、満たすものと確かめるもの（か、その見送り）があ",
        "{label}: ok — {}, whose {} are as they were looked at; every requirement is met and checked, or waived",
        reqs.ja, what.ja;
        reqs.en, what.en
    );
    let tail = if c.model.as_ref().is_some_and(|m| !m.scopes.is_empty()) {
        let (ja, one, many) = scope_noun(&k.scope_kind);
        let n = k.in_scope;
        if n == 1 {
            tr!("り、範囲の{ja} 1 個は、要件に辿れます。", "; the {one} in scope traces to a requirement")
        } else {
            tr!("り、範囲の{ja} {n} 個は、どれも要件に辿れます。", "; the {n} {many} in scope all trace to a requirement")
        }
    } else {
        tr!("ります。", "")
    };
    head.then(&tail).then(&warn)
}

/// `yuen check` for a person: the diagnostics, then the summary.
pub fn render(c: &Checked, label: &str, lang: Lang) -> String {
    let mut out = String::new();
    for d in &c.diags {
        out.push_str(&d.render(lang));
    }
    out.push_str(summary(c, label).get(lang).trim_end());
    out.push('\n');
    out
}

/// `yuen check --format json` (DESIGN 6.1): one object for the project.
pub fn to_json(c: &Checked, label: &str, lang: Lang) -> Value {
    json!({
        "root": c.project.as_ref().map(|p| p.root_shown.clone()),
        "ok": !c.has_errors(),
        "summary": summary(c, label).get(lang).trim_end(),
        "diagnostics": c.diags.iter().map(|d| d.to_json(lang)).collect::<Vec<_>>(),
    })
}

/// The states of one requirement's links, in their order.
pub fn states_of(m: &Model, r: usize) -> Vec<&LinkState> {
    m.states.iter().filter(|s| s.req == r).collect()
}

/// A requirement's link of a kind.
pub fn state(m: &Model, r: usize, k: LinkKind) -> Option<&LinkState> {
    m.states.iter().find(|s| s.req == r && s.kind == k)
}
