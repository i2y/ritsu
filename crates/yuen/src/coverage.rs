//! The seventh stage (DESIGN 5.2, 5.3, PLAN B.8): every version of every requirement has what
//! meets it and what checks it, or a waiver for each (E401, E402, W401); and every artifact in
//! a scope traces back to a requirement (E404).

use crate::ast::Side;
use crate::diag::Diag;
use crate::ends::Unread;
use crate::names::{Name, Tool};
use crate::project::Project;
use std::path::Path;

/// What a scope gathers, and what of it no requirement reaches.
#[derive(Clone, Debug)]
pub struct ScopeResult {
    pub file: usize,
    pub idx: usize,
    /// The scope as text: the naming, and the kind it gathers.
    pub text: String,
    /// What a person calls what it gathers, for the summary: `files`, `dates`.
    pub kind: Option<String>,
    pub artifacts: Vec<Name>,
    pub untraced: Vec<Name>,
}

/// The directories a `scope file "<dir>"` leaves out (DESIGN 1.8).
fn skipped(name: &str) -> bool {
    name.starts_with('.') || name == "target" || name == "node_modules"
}

/// Every file under a directory, from the root, in path order.
fn files_under(root: &Path, rel: &str, ext: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(root: &Path, d: &Path, ext: Option<&str>, out: &mut Vec<String>) {
        let Ok(rd) = ritsu_base::fs::read_dir(d) else { return };
        let mut es: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        es.sort();
        for e in es {
            let name = e.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if ritsu_base::fs::is_dir(&e) {
                if !skipped(&name) {
                    walk(root, &e, ext, out);
                }
            } else if ext.is_none_or(|x| e.extension().is_some_and(|y| y == x)) && let Ok(r) = e.strip_prefix(root) {
                out.push(r.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let start = if rel == "." { root.to_path_buf() } else { root.join(rel) };
    walk(root, &start, ext, &mut out);
    out
}

/// What a scope gathers (DESIGN 1.8). Without a kind, the files: the file named, or every file
/// of the tool under the directory (`file` takes any). With a kind, the things of that kind in
/// those files, under the pairs written before it (`scope proto "x.proto" service S method`):
/// read through the tool's language (`Items`, in the project's index), or for a `.proto` by yuen
/// itself (DESIGN 3.4).
pub fn gather(p: &Project, n: &Name, kind: Option<&str>) -> Result<Vec<Name>, Unread> {
    let abs = if n.path == "." { p.root.clone() } else { p.root.join(&n.path) };
    if !ritsu_base::fs::exists(&abs) {
        return Err(Unread::Missing { dir: false });
    }
    let files: Vec<String> = if ritsu_base::fs::is_dir(&abs) { files_under(&p.root, &n.path, n.tool.extension()) } else { vec![n.path.clone()] };
    let Some(kind) = kind else {
        return Ok(files.into_iter().map(|path| Name { tool: n.tool, path, items: vec![] }).collect());
    };
    let mut out = Vec::new();
    if n.tool == Tool::Proto {
        for f in &files {
            let ps = crate::proto::load(&p.root, f).map_err(|(file, why)| Unread::Proto(file, why))?;
            out.extend(crate::proto::gather(&ps, &Name { tool: n.tool, path: f.clone(), items: n.items.clone() }, kind));
        }
        return Ok(out);
    }
    if !p.suite.reads(n.tool) {
        return Err(Unread::NoPort(n.tool));
    }
    for f in &files {
        let items = match p.suite.index.items(n.tool, &p.root, f) {
            Some(Ok(items)) => items,
            Some(Err(said)) => return Err(Unread::Refused(said)),
            None => return Err(Unread::NoPort(n.tool)),
        };
        out.extend(items.into_iter().filter(|i| i.kind() == kind && i.naming.items.len() == n.items.len() + 1 && i.naming.items[..n.items.len()] == n.items[..]).map(|i| i.naming));
    }
    Ok(out)
}

/// The files the claims the links name ran (DESIGN 5.3, rule 4), from the records geas keeps of
/// them (`geas map`), each from the root; and the specs a link names that have no record.
#[derive(Default)]
pub struct Ran {
    pub files: std::collections::BTreeSet<String>,
    pub unrecorded: Vec<String>,
}

/// What the claims a link names ran: a link to a claim takes that claim's files, a link to a spec
/// every claim's. The record's paths are from its root, written from the spec's directory.
pub fn ran_by_claims(p: &Project) -> Ran {
    let mut ran = Ran::default();
    let Some(claims) = &p.suite.claims else { return ran };
    let mut specs: Vec<(String, Option<String>)> = Vec::new();
    for n in p.names.links.iter().flatten().flatten().filter(|n| n.tool == Tool::Geas) {
        let claim = n.items.first().map(|(_, c)| c.clone());
        if !specs.contains(&(n.path.clone(), claim.clone())) {
            specs.push((n.path.clone(), claim));
        }
    }
    for (spec, claim) in &specs {
        match claims.map_record(&p.root.join(spec)) {
            Ok(Some(rec)) => {
                let dir = ritsu_base::paths::parent(spec);
                let base = ritsu_base::paths::join(&dir, &rec.root).unwrap_or(dir);
                for r in rec.ran.iter().filter(|r| claim.as_ref().is_none_or(|c| *c == r.claim)) {
                    if let Ok(f) = ritsu_base::paths::join(&base, &r.file) {
                        ran.files.insert(f);
                    }
                }
            }
            _ => {
                if !ran.unrecorded.contains(spec) {
                    ran.unrecorded.push(spec.clone());
                }
            }
        }
    }
    ran
}

/// Whether an artifact traces to a requirement (DESIGN 5.3): a link names it, names what
/// contains it, or names something in it.
pub fn traced(a: &Name, links: &[&Name]) -> bool {
    links.iter().any(|l| *l == a || l.contains(a) || a.contains(l))
}

/// E401, E402 and W401, for every version.
pub fn coverage(p: &Project) -> Vec<Diag> {
    let mut diags = Vec::new();
    for r in 0..p.reqs.len() {
        let fi = p.reqs[r].file;
        let d = p.decl(r);
        let me = p.req_label(r);
        for side in [Side::Satisfied, Side::Verified] {
            let links = d.links.iter().filter(|l| l.side == side).count();
            let waivers: Vec<_> = d.waivers.iter().filter(|w| w.side == side).collect();
            if links == 0 && waivers.is_empty() {
                diags.push(match side {
                    Side::Satisfied => p.err(fi, "E401", d.span, tr!("{me} には、満たす成果物も、それを置かない見送りもありません", "{me} has nothing that meets it, and no waiver for that")).note(tr!(
                        "`satisfied by <成果物>` を書くか、満たすものを置かないなら `not satisfied \"<理由>\"` を書いて承認してもらいます。",
                        "Write `satisfied by <artifact>`, or, if nothing is to meet it, `not satisfied \"<why>\"`, and have that approved."
                    )),
                    Side::Verified => p.err(fi, "E402", d.span, tr!("{me} には、確かめる主張も、それを置かない見送りもありません", "{me} has nothing that checks it, and no waiver for that")).note(tr!(
                        "`verified by <主張>` を書くか、確かめるものを置かないなら `not verified \"<理由>\"` を書いて承認してもらいます。",
                        "Write `verified by <claim>`, or, if nothing is to check it, `not verified \"<why>\"`, and have that approved."
                    )),
                });
            }
            if links > 0 {
                for w in waivers {
                    let (word, words) = match side {
                        Side::Satisfied => ("not satisfied", "satisfied by"),
                        Side::Verified => ("not verified", "verified by"),
                    };
                    diags.push(p.warn(fi, "W401", w.span, tr!("{me} には `{words}` があるので、この `{word}` は要りません", "{me} has a `{words}`, so this `{word}` is not needed")).note(tr!(
                        "見送りは、リンクを置かないと決めたときに書くものです。リンクを置いたのなら、見送りを消します。",
                        "A waiver says no link is to be there; now that there is one, delete the waiver."
                    )));
                }
            }
        }
    }
    diags
}

/// E404 for the artifacts of the scopes no requirement reaches.
pub fn scope_diags(p: &Project, scopes: &[ScopeResult], ran: &Ran) -> Vec<Diag> {
    let mut diags = Vec::new();
    for s in scopes {
        let decl = &p.files[s.file].ast.scopes[s.idx];
        let reached: Vec<&Name> = s.artifacts.iter().filter(|a| !s.untraced.contains(a)).collect();
        for a in &s.untraced {
            let t = a.text();
            let mut d = p.err(s.file, "E404", decl.span, tr!("{t} は範囲にありますが、どの要件からも辿れません", "{t} is in scope, and no requirement leads to it"));
            if !reached.is_empty() {
                let shown: Vec<String> = reached.iter().take(5).map(|n| n.text()).collect();
                let more = reached.len().saturating_sub(5);
                let (ja, en) = (shown.join("、"), shown.join(", "));
                d = d.note(if more > 0 {
                    tr!("範囲のほかのものは辿れます: {ja}、ほか {more} 個", "The rest of the scope is reached: {en}, and {more} more")
                } else {
                    tr!("範囲のほかのものは辿れます: {ja}", "The rest of the scope is reached: {en}")
                });
            }
            if a.tool == Tool::File && !ran.unrecorded.is_empty() {
                let specs: Vec<String> = ran.unrecorded.iter().map(|s| Name { tool: Tool::Geas, path: s.clone(), items: vec![] }.text()).collect();
                let (ja, en) = (specs.join("、"), specs.join(", "));
                d = d.note(tr!(
                    "{ja} には geas の記録がありません。`geas map` で記録を作れば、その主張が走らせたファイルは要件に辿れます。",
                    "{en} has no geas record; with one (`geas map`), the files its claims run trace to their requirements."
                ));
            }
            d = d.note(tr!("`satisfied by` か `verified by` でこれを名指す要件を足すか、範囲を狭めます。", "Add a requirement whose `satisfied by` or `verified by` names it, or narrow the scope."));
            diags.push(d);
        }
    }
    diags
}
