//! `yuen check` (DESIGN 5, PLAN B.9): the seven stages, in order.
//!
//! They are: words and lines (E001–E006); names (E007–E013, E403); sources (E101–E106,
//! W101); artifacts (E201–E203, E205, and E107, which holds a requirement's copies to those of
//! the rules and calendars that meet it); cycles and periods (E405–E409); links and hashes
//! (E301–E305, W301); coverage and scope (E401, E402, E404, W401). An error in the first two stops there:
//! until the names are known nothing can be compared. Errors of sources and artifacts let the
//! check go on, leaving out the links whose ends could not be read.

use crate::coverage::{self, ScopeResult};
use crate::diag::{self, Diag, DiagExt};
use crate::ends::{self, End, Unread};
use crate::graph;
use ritsu_base::text::{Lang, Text, plural};
use crate::marks::{self, Ctx, LinkKind, LinkState};
use crate::names::Name;
use crate::project::{self, Project, Refusal};
use crate::sources::{self, Sources};
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

thread_local! {
    /// The command line `run` was given, words after the program's name, to say again with
    /// `ritsu yuen` in front when a language is not joined.
    pub static COMMAND: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

/// A word as a shell reads it back: as it is, or in single quotes.
fn shell_word(w: &str) -> String {
    if !w.is_empty() && w.chars().all(|c| !c.is_whitespace() && !"'\"\\$`;&|<>()*?[]#~".contains(c)) {
        w.to_string()
    } else {
        format!("'{}'", w.replace('\'', "'\\''"))
    }
}

/// The command to run instead: the one given, with `ritsu yuen` in front.
pub fn with_ritsu(p: &Project) -> String {
    let words = COMMAND.with(|c| c.borrow().clone()).unwrap_or_else(|| {
        let mut w = vec!["check".to_string()];
        w.extend(p.args.iter().cloned());
        if let Some(r) = &p.root_flag {
            w.extend(["--root".to_string(), r.clone()]);
        }
        w
    });
    let shown: Vec<String> = words.iter().map(|w| shell_word(w)).collect();
    format!("ritsu yuen {}", shown.join(" "))
}

/// E206 (ritsu's DESIGN 2.3): the languages a project's names need, and that are not handed to
/// yuen — the binary of yuen's own crate holds no other language. Said once for each language, at
/// the first thing named of it (a link, a scope, or with `only_sources` a borrowed source alone),
/// with the same command to run through `ritsu yuen`. A command that meets one exits 2: it is
/// where yuen runs, not what the project says.
pub fn unjoined(p: &Project, only_sources: bool) -> Vec<Diag> {
    use crate::suite::Suite;
    // (file, line, column, naming), in the order written
    let mut needed: Vec<(usize, usize, usize, Name)> = Vec::new();
    if !only_sources {
        for r in 0..p.reqs.len() {
            let fi = p.reqs[r].file;
            for (li, n) in p.names.links[r].iter().enumerate() {
                if let Some(n) = n {
                    let l = &p.decl(r).links[li];
                    needed.push((fi, l.span.line, l.naming.tool.at, n.clone()));
                }
            }
        }
        for fi in 0..p.files.len() {
            for (si, sc) in p.names.scopes[fi].iter().enumerate() {
                if let Some((n, _)) = sc {
                    let d = &p.files[fi].ast.scopes[si];
                    needed.push((fi, d.span.line, d.naming.tool.at, n.clone()));
                }
            }
        }
    }
    for fi in 0..p.files.len() {
        for (si, n) in p.names.sources[fi].iter().enumerate() {
            let s = &p.files[fi].ast.sources[si];
            if let (Some(n), crate::ast::SourceKind::Borrowed { naming }) = (n, &s.kind) {
                needed.push((fi, s.span.line, naming.tool.at, n.clone()));
            }
        }
    }
    needed.sort_by_key(|(fi, line, col, _)| (*fi, *line, *col));
    let mut said: Vec<crate::names::Tool> = Vec::new();
    let mut out = Vec::new();
    let cmd = with_ritsu(p);
    for (fi, line, col, n) in needed {
        if !Suite::READ.contains(&n.tool) || p.suite.reads(n.tool) || said.contains(&n.tool) {
            continue;
        }
        said.push(n.tool);
        let (t, what) = (n.tool.word(), n.text());
        out.push(p.err(fi, "E206", crate::ast::Span { line, col }, tr!("yuen 単独のバイナリは {t} の成果物を読めません: {what}", "this yuen cannot read {t} artifacts: {what}")).note(tr!(
            "yuen 単独のバイナリには、ほかの言語が入っていません。同じコマンドを、すべての言語をつないだ ritsu で、`{cmd}` のように走らせてください。",
            "The binary of yuen's own crate holds no other language; run it with every language joined, through ritsu: `{cmd}`."
        )));
    }
    out
}

/// Whether diagnostics hold an E206: the command exits 2.
pub fn has_unjoined(diags: &[Diag]) -> bool {
    diags.iter().any(|d| d.code == "E206")
}

/// E203: the language of what a line names cannot answer for its file — it does not pass the
/// language's check, or does not read — with what the language says (ritsu's DESIGN 6.1).
pub fn refused(p: &Project, fi: usize, span: crate::ast::Span, n: &Name, said: &[ritsu_ports::Said]) -> Diag {
    let (t, tool) = (n.text(), n.tool.word());
    let mut d = p.err(fi, "E203", span, tr!("{tool} から {t} の情報を得られません", "{tool} cannot answer for {t}"));
    for s in said.iter().take(5) {
        let at = match s.line {
            Some(l) => format!("{}:{l}", p.shown_any(&s.file)),
            None => p.shown_any(&s.file),
        };
        let code = if s.code.is_empty() { String::new() } else { format!("[{}] ", s.code) };
        d = d.note(tr!("{tool} の診断: {code}{at}: {}", "what {tool} says: {code}{at}: {}", s.message.ja; s.message.en));
    }
    if said.len() > 5 {
        let more = said.len() - 5;
        d = d.note(tr!("ほかに {more} 件", "and {more} more"));
    }
    d.note(tr!(
        "そのファイルを、{tool} の検査を通るように直してください。検査を通らないファイルや読めないファイルからは成果物の定義を読み取れないので、yuen はその成果物のハッシュを取れません。",
        "Make the file pass {tool}'s check; no end is made from a file that does not."
    ))
}

/// The diagnostic of an artifact that cannot be read (stage 4); None for a language not joined,
/// which is a refusal.
fn unread_diag(p: &Project, fi: usize, line: usize, col: usize, n: &Name, u: &Unread, recorded: Option<&str>) -> Option<Diag> {
    let span = crate::ast::Span { line, col };
    Some(match u {
        Unread::Missing { dir } => missing(p, fi, line, col, n, *dir),
        Unread::NoPort(_) => return None,
        Unread::Refused(said) => refused(p, fi, span, n, said),
        Unread::Proto(file, why) => {
            let f = p.shown(file);
            p.err(fi, "E205", span, tr!("{f} を proto として読めません: {}", "{f} does not read as a .proto: {}", why.ja; why.en)).note(tr!(
                "yuen は、ritsu の .proto のパーサーで読みます。proto3 の `.proto` に直してください。",
                "yuen reads it with ritsu's reader of .proto files; make it a proto3 `.proto`."
            ))
        }
        Unread::NoSuchName { same_kind } => no_such_name(p, fi, span, n, same_kind, recorded),
    })
}

/// E202 (DESIGN 2.4, 4.5): the name is not in the file. Written with an alias, it says the name;
/// renamed, the candidates are the things of the same kind whose end is what was looked at, else
/// those no link names — and when that is one, how it differs from what was looked at.
fn no_such_name(p: &Project, fi: usize, span: crate::ast::Span, n: &Name, same_kind: &[(Name, End)], recorded: Option<&str>) -> Diag {
    let (t, kind) = (n.text(), n.kind().unwrap_or(""));
    let name = n.items.last().map(|(_, v)| v.clone()).unwrap_or_default();
    let written = crate::names::word_or_quote(&name);
    let shown = p.shown(&n.path);
    let mut d = p.err(fi, "E202", span, tr!("{shown} に {kind} {written} はありません", "{shown} has no {kind} {written}"));
    if let Some(real) = alias_of(p, n) {
        let fixed = Name { items: [&n.items[..n.items.len() - 1], &[(kind.to_string(), real.clone())]].concat(), ..n.clone() };
        return d.note(tr!("`{name}` は {real} の別名です。名指しには、別名ではなくツールの名前を書いてください。", "`{name}` is the alias of {real}; a naming writes the tool's name.")).candidates(vec![fixed.text()]);
    }
    let named: Vec<&Name> = p.names.links.iter().flatten().flatten().collect();
    let by_hash: Vec<String> = same_kind.iter().filter(|(_, e)| Some(e.hash.as_str()) == recorded).map(|(m, _)| m.text()).collect();
    let unlinked: Vec<&(Name, End)> = same_kind.iter().filter(|(m, _)| !named.contains(&m)).collect();
    if !by_hash.is_empty() {
        d = d.note(tr!("名前が変わったようです。確かめたときと同じ定義のものがあります。", "It looks renamed: these have the definition that was looked at.")).candidates(by_hash);
    } else if !unlinked.is_empty() {
        d = d.note(tr!("候補は、同じ種類のもののうち、どのリンクも名指していないものです。", "The ones of the same kind no link names.")).candidates(unlinked.iter().take(5).map(|(m, _)| m.text()).collect());
        // one candidate, and what was looked at kept: how the two differ (DESIGN 4.5)
        if let ([(m, e)], Some(h)) = (unlinked.as_slice(), recorded)
            && let Some(before) = crate::marks::reviewed_content(p, fi, h)
        {
            let (old, new) = (String::from_utf8_lossy(&before).to_string(), String::from_utf8_lossy(&e.bytes).to_string());
            let (lines, more) = crate::diff::unified(&old, &new);
            let c = m.text();
            d = d.diff(tr!("確かめたときの {t} から、いまの {c} への差分", "from {t} as it was looked at to {c} now"), lines);
            if more > 0 {
                d = d.note(tr!("差分はほかに {more} 行あります。", "{more} more lines of the diff are not shown."));
            }
        }
    }
    d
}

/// The name a link wrote, when it is the alias of a rule's input, output or enum, or of a dates
/// file's input or date: the name to write instead.
fn alias_of(p: &Project, n: &Name) -> Option<String> {
    let (kind, written) = n.items.last()?;
    let abs = p.root.join(&n.path);
    match n.tool {
        crate::names::Tool::Rulec => {
            let f = p.suite.rules.as_ref()?.facts(&abs).ok()?;
            let cols = match kind.as_str() {
                "input" => f.inputs.iter().map(|c| (c.name.clone(), c.alias.clone())).collect::<Vec<_>>(),
                "output" => f.outputs.iter().map(|c| (c.name.clone(), c.alias.clone())).collect(),
                "enum" => f.enums.iter().map(|e| (e.name.clone(), e.alias.clone())).collect(),
                "value" => f.enums.iter().filter(|e| n.items.first().is_some_and(|(_, en)| *en == e.name)).flat_map(|e| e.values.iter().map(|v| (v.name.clone(), v.alias.clone()))).collect(),
                _ => vec![],
            };
            cols.into_iter().find(|(_, a)| a == written).map(|(nm, _)| nm)
        }
        crate::names::Tool::Koyomi => {
            let f = p.suite.dates.as_ref()?.facts(&abs).ok()?;
            let pairs: Vec<(String, String)> = match kind.as_str() {
                "input" => f.inputs.iter().map(|i| (i.name.clone(), i.alias.clone())).collect(),
                "date" => f.functions.iter().map(|d| (d.name.clone(), d.alias.clone())).collect(),
                _ => vec![],
            };
            pairs.into_iter().find(|(_, a)| a == written).map(|(nm, _)| nm)
        }
        _ => None,
    }
}

pub fn check(args: &[String], root: Option<&str>) -> Result<Checked, Refusal> {
    check_with(args, root, crate::suite::Suite::default())
}

/// [`check`], reading what another language holds through the ports `suite` joins.
pub fn check_with(args: &[String], root: Option<&str>, suite: crate::suite::Suite) -> Result<Checked, Refusal> {
    let (project, d1) = project::load_with(args, root, suite)?;
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
    // The languages the names need are joined (E206); else nothing more can be read.
    let d2b = unjoined(&p, false);
    if !d2b.is_empty() {
        diags.extend(d2b);
        return Ok(Checked { project: Some(p), diags, stage: 2, model: None });
    }
    // 3. Sources.
    let (srcs, d3) = sources::check_sources(&p);
    diags.extend(d3);
    // 4. Artifacts.
    let mut artifacts: BTreeMap<Name, Result<End, Unread>> = BTreeMap::new();
    let mut d4 = Vec::new();
    for r in 0..p.reqs.len() {
        let fi = p.reqs[r].file;
        for (li, n) in p.names.links[r].iter().enumerate() {
            let Some(n) = n else { continue };
            let link = &p.decl(r).links[li];
            let (line, col) = (link.span.line, link.naming.tool.at);
            let e = artifacts.entry(n.clone()).or_insert_with(|| ends::artifact_end(&p, n)).clone();
            if let Err(u) = &e {
                let recorded = link.record.as_ref().and_then(|r| r.parsed.as_ref().ok()).and_then(|r| r.down.clone());
                d4.extend(unread_diag(&p, fi, line, col, n, u, recorded.as_deref()));
            }
        }
    }
    let mut scopes = Vec::new();
    for fi in 0..p.files.len() {
        for (si, sc) in p.names.scopes[fi].iter().enumerate() {
            let Some((n, kind)) = sc else { continue };
            let decl = &p.files[fi].ast.scopes[si];
            let text = match kind {
                Some(k) => format!("{} {k}", n.text()),
                None => n.text(),
            };
            match coverage::gather(&p, n, kind.as_deref()) {
                Err(Unread::Missing { .. }) => {
                    let path = p.shown(&n.path);
                    d4.push(p.err(fi, "E201", crate::ast::Span { line: decl.span.line, col: decl.naming.path.as_ref().map(|w| w.at).unwrap_or(1) }, tr!("範囲のパス {path} がありません", "The path {path} of the scope is not there")));
                }
                Err(u) => d4.extend(unread_diag(&p, fi, decl.span.line, decl.naming.tool.at, n, &u, None)),
                Ok(items) => scopes.push(ScopeResult { file: fi, idx: si, text, kind: kind.clone(), artifacts: items, untraced: vec![] }),
            }
        }
    }
    // what a requirement reads, against what meets it (E107, DESIGN 3.3)
    d4.extend(sources::mismatches(&p, &srcs));
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
    let ran = coverage::ran_by_claims(&p);
    for s in &mut scopes {
        s.untraced = s.artifacts.iter().filter(|a| !coverage::traced(a, &links) && !(a.tool == crate::names::Tool::File && ran.files.contains(&a.path))).cloned().collect();
    }
    diags.extend(coverage::scope_diags(&p, &scopes, &ran));
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
            "パスは、この .req のあるディレクトリからの相対パスで書いてください。ファイルの名前を変えたのなら、リンクも直してください。",
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
        // a kind is the tool's own word, set off from the Japanese around it
        Some(k) => (format!(" {k}"), k.to_string(), format!("{k}s")),
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
    let warn = if w > 0 { tr!("警告は {w} 件です。", " ({})", ; plural(w, "warning", "warnings")) } else { Text::default() };
    let head = tr!(
        "{label}: ok — {}の{}が、確かめたときのままです。どの要件にも、満たすものと確かめるもの（無ければ見送り）があ",
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
        "diagnostics": c.diags.iter().map(|d| crate::diag::value(&d.to_json(lang))).collect::<Vec<_>>(),
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
