//! A project (DESIGN 1.1, 2.2, PLAN B.3): the `.req` files the command line names, read as
//! one, with the root their paths are written from, and the names they declare and use —
//! requirements and their versions and aliases, roles, sources, and the artifacts the links
//! and the scopes name (E007–E013, E403).

use crate::ast::*;
use crate::diag::Diag;
use crate::i18n::Text;
use crate::names::{self, Name, Tool};
use crate::parse;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

pub struct SrcFile {
    /// The path as the command line named it (a directory joined with what was found in it).
    pub display: String,
    /// From the root.
    pub rel: String,
    /// The directory, from the root; `""` for the root itself.
    pub dir: String,
    pub abs: PathBuf,
    pub src: String,
    pub ast: ReqFile,
}

/// One version of a requirement: a `requirement` block.
#[derive(Clone, Debug)]
pub struct ReqVer {
    pub file: usize,
    pub idx: usize,
    pub name: String,
    pub version: u32,
}

/// What a scope names, and the kind it gathers (DESIGN 1.8).
pub type Scoped = (Name, Option<String>);

/// What a project's names resolve to (stage 2). Indexed like the blocks they come from.
#[derive(Default)]
pub struct Names {
    /// For each version, for each `from`: the version a `from <requirement>` points at.
    pub from: Vec<Vec<Option<usize>>>,
    /// For each version, for each `replaces`: the version it replaces (the last, unless one
    /// is written).
    pub replaces: Vec<Vec<Option<usize>>>,
    /// For each version, for each link: the artifact it names.
    pub links: Vec<Vec<Option<Name>>>,
    /// For each file, for each scope: what it names and the kind it gathers.
    pub scopes: Vec<Vec<Option<Scoped>>>,
    /// For each file, for each source: what a borrowed source names, and for a `file`
    /// source the path of its copy from the root.
    pub sources: Vec<Vec<Option<Name>>>,
}

pub struct Project {
    /// The root, absolute (DESIGN 2.2).
    pub root: PathBuf,
    /// The root as seen from where yuen runs: `.` when it runs there.
    pub root_shown: String,
    /// Where yuen runs, absolute: what `shown` writes a path from.
    pub cwd: PathBuf,
    /// The paths the command line gave, for the line that says what was checked.
    pub label: String,
    /// The same paths, one by one, for the commands a diagnostic suggests.
    pub args: Vec<String>,
    /// The `--root` the command line gave, for the same.
    pub root_flag: Option<String>,
    pub files: Vec<SrcFile>,
    pub reqs: Vec<ReqVer>,
    /// Each requirement's versions, by name, in the order of their numbers.
    pub by_name: BTreeMap<String, Vec<usize>>,
    /// The aliases, each to its requirement's name.
    pub aliases: BTreeMap<String, String>,
    pub names: Names,
}

/// What stops a command before anything is checked: exit 2.
#[derive(Debug)]
pub struct Refusal(pub Text);

/// `to` as seen from the directory `from`, both absolute.
pub fn relative(from: &Path, to: &Path) -> String {
    let a: Vec<Component> = from.components().collect();
    let b: Vec<Component> = to.components().collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<String> = Vec::new();
    for _ in common..a.len() {
        parts.push("..".into());
    }
    for c in &b[common..] {
        parts.push(c.as_os_str().to_string_lossy().to_string());
    }
    if parts.is_empty() { ".".to_string() } else { parts.join("/") }
}

/// The root of a project (DESIGN 2.2): the nearest directory at or above the first path given
/// that has a `.git`, else the directory given (a file's own directory).
pub fn find_root(first: &Path) -> Option<PathBuf> {
    let start = if first.is_dir() { first.to_path_buf() } else { first.parent().map(|p| if p.as_os_str().is_empty() { Path::new(".") } else { p }).unwrap_or(Path::new(".")).to_path_buf() };
    let abs = std::fs::canonicalize(&start).ok()?;
    for d in abs.ancestors() {
        if d.join(".git").exists() {
            return Some(d.to_path_buf());
        }
    }
    Some(abs)
}

/// The `.req` files a path stands for: itself, or every `.req` under a directory, in path
/// order, leaving out what is hidden, `target` and `node_modules`.
pub fn expand(arg: &str) -> Vec<String> {
    let p = Path::new(arg);
    if !p.is_dir() {
        return vec![arg.to_string()];
    }
    let mut out = Vec::new();
    fn walk(d: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        let mut es: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        es.sort();
        for e in es {
            let name = e.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if e.is_dir() {
                if name.starts_with('.') || name == "target" || name == "node_modules" {
                    continue;
                }
                walk(&e, out);
            } else if e.extension().is_some_and(|x| x == "req") {
                out.push(e.to_string_lossy().to_string());
            }
        }
    }
    walk(p, &mut out);
    out
}

/// Read and parse every file the paths stand for. The diagnostics are the first stage's
/// (E001–E006); the project comes back only when there are none.
pub fn load(args: &[String], root_flag: Option<&str>) -> Result<(Option<Project>, Vec<Diag>), Refusal> {
    let Some(first) = args.first() else {
        return Err(Refusal(tr!(".req のファイルかディレクトリを渡します", "Give .req files or directories")));
    };
    for a in args {
        if !Path::new(a).exists() {
            return Err(Refusal(tr!("`{a}` がありません", "`{a}` does not exist")));
        }
    }
    let root = match root_flag {
        Some(r) => match std::fs::canonicalize(r) {
            Ok(p) if p.is_dir() => p,
            _ => return Err(Refusal(tr!("`--root {r}` はディレクトリではありません", "`--root {r}` is not a directory"))),
        },
        None => find_root(Path::new(first)).ok_or_else(|| Refusal(tr!("`{first}` のルートを決められません", "Cannot tell the root of `{first}`")))?,
    };
    let cwd = std::env::current_dir().ok().and_then(|c| std::fs::canonicalize(c).ok()).unwrap_or_default();
    let root_shown = relative(&cwd, &root);
    let mut files = Vec::new();
    let mut diags = Vec::new();
    let mut ok = true;
    let mut seen = std::collections::BTreeSet::new();
    for a in args {
        let found = expand(a);
        if found.is_empty() {
            return Err(Refusal(tr!("`{a}` の下に .req のファイルがありません", "There is no .req file under `{a}`")));
        }
        for display in found {
            let abs = std::fs::canonicalize(&display).map_err(|e| Refusal(tr!("`{display}` を読めません: {e}", "Cannot read `{display}`: {e}")))?;
            if !seen.insert(abs.clone()) {
                continue;
            }
            let Ok(rel_path) = abs.strip_prefix(&root) else {
                let r = root_shown.clone();
                return Err(Refusal(tr!("`{display}` はルート（{r}）の外にあります", "`{display}` is outside the root ({r})")));
            };
            let rel = rel_path.to_string_lossy().replace('\\', "/");
            let dir = rel_path.parent().map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            let bytes = std::fs::read(&abs).map_err(|e| Refusal(tr!("`{display}` を読めません: {e}", "Cannot read `{display}`: {e}")))?;
            let Ok(src) = String::from_utf8(bytes) else {
                return Err(Refusal(tr!("`{display}` は UTF-8 ではありません", "`{display}` is not UTF-8")));
            };
            let parsed = parse::parse(&display, &rel, &src);
            diags.extend(parsed.diags);
            match parsed.file {
                Some(ast) => files.push(SrcFile { display, rel, dir, abs, src, ast }),
                None => ok = false,
            }
        }
    }
    if !ok {
        return Ok((None, diags));
    }
    let label = args.join(", ");
    Ok((Some(Project { root, root_shown, cwd, label, args: args.to_vec(), root_flag: root_flag.map(|r| r.to_string()), files, reqs: vec![], by_name: BTreeMap::new(), aliases: BTreeMap::new(), names: Names::default() }), diags))
}

impl Project {
    pub fn decl(&self, r: usize) -> &ReqDecl {
        let v = &self.reqs[r];
        &self.files[v.file].ast.requirements[v.idx]
    }

    pub fn file_of(&self, r: usize) -> &SrcFile {
        &self.files[self.reqs[r].file]
    }

    /// A path from the root as the directory yuen runs in sees it (DESIGN 2.2): what the
    /// text of a diagnostic, `trace` and the `source` commands write, so that a person can open
    /// it from where they ran yuen. The JSON keeps the path from the root.
    ///
    /// The shortest way there: when yuen runs below the root, a path under where it runs is
    /// written from there (`tests/x.req`), not up to the root and down again
    /// (`../../crates/yuen/tests/x.req`).
    pub fn shown(&self, rel: &str) -> String {
        if self.root_shown == "." {
            return rel.to_string();
        }
        let target = if rel == "." { self.root.clone() } else { self.root.join(rel) };
        relative(&self.cwd, &target)
    }

    pub fn err(&self, fi: usize, code: &'static str, s: Span, msg: Text) -> Diag {
        let f = &self.files[fi];
        Diag::error(code, &f.display, &f.rel, s.line, s.col, msg).source(&f.src)
    }

    pub fn warn(&self, fi: usize, code: &'static str, s: Span, msg: Text) -> Diag {
        let f = &self.files[fi];
        Diag::warning(code, &f.display, &f.rel, s.line, s.col, msg).source(&f.src)
    }

    /// A requirement version as a person reads it: `満了日` or `支払日 v2` when it has more
    /// than one.
    pub fn req_label(&self, r: usize) -> String {
        let v = &self.reqs[r];
        if self.by_name.get(&v.name).is_some_and(|vs| vs.len() > 1) { format!("{} v{}", v.name, v.version) } else { v.name.clone() }
    }

    /// Where a requirement is: `民法の期間.req:33`.
    pub fn req_place(&self, r: usize) -> (String, usize) {
        (self.file_of(r).display.clone(), self.decl(r).span.line)
    }

    /// The version a reference points at, if the name is known.
    pub fn lookup(&self, name: &str, version: Option<u32>) -> Option<usize> {
        let vs = self.by_name.get(name)?;
        match version {
            Some(n) => vs.iter().copied().find(|r| self.reqs[*r].version == n),
            None if vs.len() == 1 => Some(vs[0]),
            None => None,
        }
    }

    /// A requirement as `--requirement` names it: the name or the alias, and a version.
    pub fn find_req(&self, spec: &str) -> Result<Vec<usize>, Text> {
        let spec = spec.trim();
        let (name, version) = match spec.rsplit_once(' ') {
            Some((n, v)) if v.starts_with('v') && v[1..].chars().all(|c| c.is_ascii_digit()) && v.len() > 1 => (n.trim(), v[1..].parse::<u32>().ok()),
            _ => (spec, None),
        };
        let name = self.aliases.get(name).map(|s| s.as_str()).unwrap_or(name);
        let Some(vs) = self.by_name.get(name) else {
            return Err(tr!("要件「{name}」はこのプロジェクトにありません", "There is no requirement {name} in this project"));
        };
        match version {
            Some(n) => match vs.iter().copied().find(|r| self.reqs[*r].version == n) {
                Some(r) => Ok(vec![r]),
                None => Err(tr!("要件「{name}」に v{n} はありません", "The requirement {name} has no v{n}")),
            },
            None => Ok(vs.clone()),
        }
    }
}

const ROLES_HOW: (&str, &str) = (
    "役割は `role 法務 \"条文の読み方を決める\"` のように宣言してから使います。書き間違い（`法務部` と `法務`）を止めるためです。",
    "Declare a role with `role legal \"decides how the articles read\"` before it is used; that stops a misspelling (`Legal` for `legal`).",
);

/// The second stage (DESIGN 5.1): the names. Fills `p.reqs`, `p.by_name`, `p.aliases` and
/// `p.names`.
pub fn check_names(p: &mut Project) -> Vec<Diag> {
    let mut diags = Vec::new();
    // Headings: one name each across the project.
    let mut heads: BTreeMap<String, (usize, Span)> = BTreeMap::new();
    for (fi, f) in p.files.iter().enumerate() {
        let h = &f.ast.header;
        if let Some((gi, _)) = heads.get(&h.name) {
            let other = p.files[*gi].display.clone();
            diags.push(p.err(fi, "E007", h.span, tr!("見出しの名前「{}」は {other} でも使われています", "The heading {} is also the heading of {other}", h.name)));
        } else {
            heads.insert(h.name.clone(), (fi, h.span));
        }
        if h.version == 0 {
            diags.push(p.err(fi, "E009", h.span, tr!("版は v1 から数えます", "Versions count from v1")));
        }
    }
    // Roles.
    let mut roles: BTreeMap<String, (usize, Span)> = BTreeMap::new();
    for (fi, f) in p.files.iter().enumerate() {
        for r in &f.ast.roles {
            if let Some((gi, gs)) = roles.get(&r.name) {
                let at = format!("{}:{}", p.files[*gi].display, gs.line);
                diags.push(p.err(fi, "E007", r.span, tr!("役割「{}」は {at} でも宣言されています", "The role {} is also declared at {at}", r.name)));
            } else {
                roles.insert(r.name.clone(), (fi, r.span));
            }
        }
    }
    let role_known = |name: &str| roles.contains_key(name);
    // Sources: one name each within a file.
    for (fi, f) in p.files.iter().enumerate() {
        let mut seen: BTreeMap<&str, Span> = BTreeMap::new();
        for s in &f.ast.sources {
            if let Some(o) = seen.get(s.name.as_str()) {
                diags.push(p.err(fi, "E007", s.span, tr!("出典「{}」はこのファイルの {} 行目でも宣言されています", "The source {} is also declared on line {} of this file", s.name, o.line)));
            } else {
                seen.insert(&s.name, s.span);
            }
        }
    }
    // Requirements and their versions.
    let mut groups: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
    for (fi, f) in p.files.iter().enumerate() {
        for (ri, r) in f.ast.requirements.iter().enumerate() {
            groups.entry(r.name.clone()).or_default().push((fi, ri));
        }
    }
    let mut reqs = Vec::new();
    let mut by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (name, blocks) in &groups {
        let decl = |b: &(usize, usize)| &p.files[b.0].ast.requirements[b.1];
        for b in blocks {
            if let Some((0, s)) = decl(b).version {
                diags.push(p.err(b.0, "E009", s, tr!("版は v1 から数えます", "Versions count from v1")));
            }
        }
        if blocks.len() > 1 {
            if blocks.iter().all(|b| decl(b).version.is_none()) {
                let (f0, r0) = blocks[0];
                let at = format!("{}:{}", p.files[f0].display, p.files[f0].ast.requirements[r0].span.line);
                for b in &blocks[1..] {
                    diags.push(p.err(b.0, "E007", decl(b).span, tr!("要件「{name}」は {at} でも宣言されています", "The requirement {name} is also declared at {at}")).note(tr!(
                        "同じ要件の版なら、`v1`、`v2` と書き分け、どの版にも `in force` を書きます。別の要件なら、名前を変えます。",
                        "If they are versions of one requirement, write `v1` and `v2` and give each an `in force`; if not, rename one."
                    )));
                }
            } else {
                let mut seen: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
                for b in blocks {
                    match decl(b).version {
                        None => diags.push(p.err(b.0, "E009", decl(b).span, tr!(
                            "要件「{name}」には版が二つ以上あるので、どの版にも `v1`、`v2` のように版を書きます",
                            "The requirement {name} has more than one version, so each writes its version, as `v1`, `v2`"
                        ))),
                        Some((n, s)) => {
                            if let Some(o) = seen.get(&n) {
                                let at = format!("{}:{}", p.files[o.0].display, p.files[o.0].ast.requirements[o.1].span.line);
                                diags.push(p.err(b.0, "E009", s, tr!("要件「{name}」の v{n} は {at} にもあります", "The requirement {name} has another v{n}, at {at}")));
                            } else {
                                seen.insert(n, *b);
                            }
                        }
                    }
                }
            }
        }
        let mut ids: Vec<usize> = Vec::new();
        for b in blocks {
            let version = decl(b).version.map(|(n, _)| n).unwrap_or(1);
            ids.push(reqs.len());
            reqs.push(ReqVer { file: b.0, idx: b.1, name: name.clone(), version });
        }
        ids.sort_by_key(|i| (reqs[*i].version, reqs[*i].file, reqs[*i].idx));
        by_name.insert(name.clone(), ids);
    }
    // Keep the versions in the order of the files and the blocks, as the files read.
    let mut order: Vec<usize> = (0..reqs.len()).collect();
    order.sort_by_key(|i| (reqs[*i].file, reqs[*i].idx));
    let mut renum = vec![0; reqs.len()];
    for (new, old) in order.iter().enumerate() {
        renum[*old] = new;
    }
    let reqs: Vec<ReqVer> = order.iter().map(|i| reqs[*i].clone()).collect();
    for ids in by_name.values_mut() {
        for i in ids.iter_mut() {
            *i = renum[*i];
        }
    }
    p.reqs = reqs;
    p.by_name = by_name;
    // Aliases (DESIGN 1.2).
    let mut aliases: BTreeMap<String, (String, usize, Span)> = BTreeMap::new();
    for r in 0..p.reqs.len() {
        let v = p.reqs[r].clone();
        let d = p.decl(r);
        match &d.alias {
            Some((a, s)) => {
                if let Some((owner, fi, os)) = aliases.get(a)
                    && owner != &v.name
                {
                    let at = format!("{}:{}", p.files[*fi].display, os.line);
                    diags.push(p.err(v.file, "E007", *s, tr!("別名 `{a}` は要件「{owner}」の別名でもあります（{at}）", "The alias `{a}` is also the alias of {owner} ({at})")));
                    continue;
                }
                if let Some(other) = p.by_name.keys().find(|n| *n == a && *n != &v.name) {
                    diags.push(p.err(v.file, "E007", *s, tr!("別名 `{a}` は要件「{other}」の名前と同じです", "The alias `{a}` is the name of the requirement {other}")));
                    continue;
                }
                let same_req_other_alias = aliases.iter().find(|(k, (o, _, _))| o == &v.name && *k != a).map(|(k, _)| k.clone());
                if let Some(o) = same_req_other_alias {
                    diags.push(p.err(v.file, "E007", *s, tr!("要件「{}」に別名が二つあります（{o} と {a}）", "The requirement {} has two aliases ({o} and {a})", v.name)).note(tr!(
                        "別名は要件に一つで、どの版にも同じ別名を書きます。",
                        "A requirement has one alias, written the same on every version."
                    )));
                    continue;
                }
                aliases.entry(a.clone()).or_insert((v.name.clone(), v.file, *s));
            }
            None if !parse::is_alias(&v.name) => {
                let name = &v.name;
                diags.push(p.err(v.file, "E010", d.span, tr!("要件「{name}」に別名がありません", "The requirement {name} has no alias")).note(tr!(
                    "名前が ASCII の小文字・数字・`_` でない要件には、`{name}(payment_day)` のように別名を付けます。ReqIF と PROV の識別子と、コマンドの引数に使います。",
                    "A requirement whose name is not lowercase ASCII, digits and `_` gets an alias, as in `{name}(payment_day)`; ReqIF and PROV identify it by the alias, and so can a command line."
                )));
            }
            None => {}
        }
    }
    p.aliases = aliases.into_iter().map(|(a, (n, _, _))| (a, n)).collect();
    // What each requirement must have, and the roles and requirements it names.
    let mut from_t = Vec::new();
    let mut repl_t = Vec::new();
    for r in 0..p.reqs.len() {
        let fi = p.reqs[r].file;
        let d = p.decl(r).clone();
        let name = p.req_label(r);
        if d.text.is_none() {
            diags.push(p.err(fi, "E010", d.span, tr!("要件「{name}」に `text` がありません", "The requirement {name} has no `text`")).note(tr!(
                "`text \"…\"` に、求めていることを短い一文で書きます。",
                "Write what it asks for in a short sentence, as `text \"…\"`."
            )));
        }
        match &d.owner {
            None => diags.push(p.err(fi, "E010", d.span, tr!("要件「{name}」に `owner` がありません", "The requirement {name} has no `owner`")).note(tr!(
                "持ち主は、要件が変わるときに承認する役割です。`affected` が「誰に聞くか」を答えるのに使います。",
                "The owner is the role that approves a change to the requirement; `affected` answers whom to ask with it."
            ))),
            Some((o, s)) if !role_known(o) => diags.push(p.err(fi, "E008", *s, tr!("役割「{o}」は宣言されていません", "The role {o} is not declared")).note(tr!("{}", "{}", ROLES_HOW.0; ROLES_HOW.1))),
            _ => {}
        }
        if d.from.is_empty() && d.decided.is_empty() {
            diags.push(p.err(fi, "E010", d.span, tr!("要件「{name}」に出どころがありません（`from` も `decided` もありません）", "The requirement {name} has no origin (no `from` and no `decided`)")).note(tr!(
                "出典から来た要件は `from @<出典> <条>` で、人が決めた要件は `decided <日付> by <役割> \"<理由>\"` で、どこから来たかを書きます。",
                "A requirement read from a source says `from @<source> <article>`; one people decided says `decided <date> by <role> \"<why>\"`."
            )));
        }
        for dc in &d.decided {
            if !role_known(&dc.by.0) {
                let o = &dc.by.0;
                diags.push(p.err(fi, "E008", dc.by.1, tr!("役割「{o}」は宣言されていません", "The role {o} is not declared")).note(tr!("{}", "{}", ROLES_HOW.0; ROLES_HOW.1)));
            }
        }
        let records = d.from.iter().filter_map(|f| f.record.as_ref()).chain(d.links.iter().filter_map(|l| l.record.as_ref())).chain(d.waivers.iter().filter_map(|w| w.record.as_ref()));
        for rec in records {
            if let Ok(rc) = &rec.parsed
                && !role_known(&rc.by)
            {
                let o = &rc.by;
                diags.push(p.err(fi, "E008", rc.by_span, tr!("役割「{o}」は宣言されていません", "The role {o} is not declared")).note(tr!("{}", "{}", ROLES_HOW.0; ROLES_HOW.1)));
            }
        }
        let mut targets = Vec::new();
        for f in &d.from {
            targets.push(match &f.what {
                FromWhat::Req(rr) => resolve_ref(p, fi, rr, &mut diags),
                FromWhat::Cite { .. } => None,
            });
        }
        from_t.push(targets);
        let mut rt = Vec::new();
        for rr in &d.replaces {
            // `replaces X` without a version means X's last version (DESIGN 5.5).
            let t = match (rr.version, p.by_name.get(&rr.name)) {
                (None, Some(vs)) => vs.last().copied(),
                _ => resolve_ref(p, fi, rr, &mut diags),
            };
            rt.push(t);
        }
        repl_t.push(rt);
    }
    p.names.from = from_t;
    p.names.replaces = repl_t;
    // The artifacts the links name (DESIGN 1.6, 2).
    let mut links = Vec::new();
    for r in 0..p.reqs.len() {
        let fi = p.reqs[r].file;
        let dir = p.files[fi].dir.clone();
        let d = p.decl(r).clone();
        let mut out = Vec::new();
        for l in &d.links {
            out.push(match names::resolve(&l.naming, &dir, false) {
                Err(e) => {
                    diags.push(name_diag(p, fi, l.span.line, e));
                    None
                }
                Ok((n, _)) => match placed(&n, Place::Link(l.side)) {
                    Err((code, msg, notes)) => {
                        let mut dg = p.err(fi, code, Span { line: l.span.line, col: l.naming.tool.col }, msg);
                        dg.notes = notes;
                        diags.push(dg);
                        None
                    }
                    Ok(()) => Some(n),
                },
            });
        }
        links.push(out);
    }
    p.names.links = links;
    // Scopes and borrowed sources.
    let mut scopes = Vec::new();
    let mut sources = Vec::new();
    for fi in 0..p.files.len() {
        let dir = p.files[fi].dir.clone();
        let ast = p.files[fi].ast.clone();
        let mut sc = Vec::new();
        for s in &ast.scopes {
            sc.push(match names::resolve(&s.naming, &dir, true) {
                Err(e) => {
                    diags.push(name_diag(p, fi, s.span.line, e));
                    None
                }
                Ok((n, g)) => match placed(&n, Place::Scope).and(gathered_ok(&n, g.as_ref().map(|w| w.text.as_str()))) {
                    Err((code, msg, notes)) => {
                        let mut dg = p.err(fi, code, Span { line: s.span.line, col: s.naming.tool.col }, msg);
                        dg.notes = notes;
                        diags.push(dg);
                        None
                    }
                    Ok(()) => Some((n, g.map(|w| w.text))),
                },
            });
        }
        scopes.push(sc);
        let mut so = Vec::new();
        for s in &ast.sources {
            so.push(match &s.kind {
                SourceKind::Borrowed { naming } => match names::resolve(naming, &dir, false) {
                    Err(e) => {
                        diags.push(name_diag(p, fi, s.span.line, e));
                        None
                    }
                    Ok((n, _)) => match placed(&n, Place::Borrowed) {
                        Err((code, msg, notes)) => {
                            let mut dg = p.err(fi, code, Span { line: s.span.line, col: naming.tool.col }, msg);
                            dg.notes = notes;
                            diags.push(dg);
                            None
                        }
                        Ok(()) => Some(n),
                    },
                },
                SourceKind::File { path, path_span, .. } => {
                    if path.is_empty() || names::is_absolute(path) {
                        diags.push(p.err(fi, "E013", *path_span, tr!("出典の写しのパス `{path}` は、.req からの相対で書きます", "Write the path `{path}` of the copy from the directory of the .req")));
                        None
                    } else {
                        match names::collapse(&dir, path) {
                            None => {
                                diags.push(p.err(fi, "E013", *path_span, tr!("`{path}` はルートの外に出ます", "`{path}` goes outside the root")));
                                None
                            }
                            Some(full) => Some(Name { tool: Tool::File, path: full, items: vec![] }),
                        }
                    }
                }
                SourceKind::Law { .. } => None,
            });
        }
        sources.push(so);
    }
    p.names.scopes = scopes;
    p.names.sources = sources;
    diags.sort_by(|a, b| (a.file.as_str(), a.line, a.col).cmp(&(b.file.as_str(), b.line, b.col)));
    diags
}

fn name_diag(p: &Project, fi: usize, line: usize, e: names::NameError) -> Diag {
    let mut d = p.err(fi, e.code, Span { line, col: e.col }, e.msg);
    d.notes = e.notes;
    d
}

fn resolve_ref(p: &Project, fi: usize, rr: &ReqRef, diags: &mut Vec<Diag>) -> Option<usize> {
    let n = &rr.name;
    let Some(vs) = p.by_name.get(n) else {
        diags.push(p.err(fi, "E008", rr.span, tr!("要件「{n}」は宣言されていません", "The requirement {n} is not declared")));
        return None;
    };
    match rr.version {
        Some(v) => match vs.iter().copied().find(|r| p.reqs[*r].version == v) {
            Some(r) => Some(r),
            None => {
                let have: Vec<String> = vs.iter().map(|r| format!("v{}", p.reqs[*r].version)).collect();
                let (ja, en) = (have.join("、"), have.join(", "));
                diags.push(p.err(fi, "E009", rr.span, tr!("要件「{n}」に v{v} はありません（あるのは {ja}）", "The requirement {n} has no v{v} (it has {en})")));
                None
            }
        },
        None if vs.len() == 1 => Some(vs[0]),
        None => {
            let have: Vec<String> = vs.iter().map(|r| format!("v{}", p.reqs[*r].version)).collect();
            let (ja, en) = (have.join("、"), have.join(", "));
            diags.push(p.err(fi, "E009", rr.span, tr!("要件「{n}」には版が二つ以上あるので、どの版かを書きます（{ja}）", "The requirement {n} has more than one version, so say which ({en})")));
            None
        }
    }
}

/// Where a naming is written, for what may be written there.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Link(Side),
    Scope,
    Borrowed,
}

type Refused = (&'static str, Text, Vec<Text>);

/// Whether a naming may be written where it is (DESIGN 1.4, 1.6, 2.3): `source` only in a
/// borrowed source, `yuen` nowhere in a `.req`, and on the side that verifies only what can
/// fail (E403).
pub fn placed(n: &Name, place: Place) -> Result<(), Refused> {
    let kind = n.kind();
    if place == Place::Borrowed {
        return match (n.tool, kind) {
            (Tool::Rulec | Tool::Koyomi, Some("source")) if n.items.len() == 1 => Ok(()),
            (Tool::Yuen, _) => Err(("E012", tr!("別の .req の出典は借りられません", "A source of another .req cannot be borrowed"), vec![tr!(
                "借りられるのは、rulec と koyomi のファイルが固定している出典だけです（`source 民法 = koyomi \"民法の期間.cal\" source 民法`）。",
                "Only a source a rule or a calendar pins can be borrowed (`source 民法 = koyomi \"civil_code.cal\" source 民法`)."
            )])),
            _ => Err(("E012", tr!("借りる出典は `rulec` か `koyomi` のファイルの `source` です", "A borrowed source is the `source` of a rulec or a koyomi file"), vec![tr!(
                "`source 民法 = koyomi \"民法の期間.cal\" source 民法` のように書きます。",
                "Write it like `source 民法 = koyomi \"civil_code.cal\" source 民法`."
            )])),
        };
    }
    if n.tool == Tool::Yuen {
        return Err(("E012", tr!("`yuen` の名指しは .req の中には書けません", "A `yuen` naming is not written in a .req"), vec![tr!(
            "要件どうしは `from <要件>` で、名前だけで書きます。`yuen` の名指しは、ほかの言語が yuen の要件を指すためのものです。",
            "One requirement points at another with `from <requirement>`, by name; a `yuen` naming is how the other languages point at a requirement."
        )]));
    }
    if n.items.iter().any(|(k, _)| k == "source") {
        return Err(("E012", tr!("出典（`source`）はリンクや範囲に書けません", "A source (`source`) is not written in a link or a scope"), vec![tr!(
            "出典は要件の出どころで、要件を満たすものではありません。`source <名前> = koyomi \"…\" source <名前>` と借りて、`from @<名前> <条>` で引きます。",
            "A source is where a requirement comes from, not what meets it: borrow it with `source <name> = koyomi \"…\" source <name>` and cite it with `from @<name> <article>`."
        )]));
    }
    if place == Place::Link(Side::Verified) {
        let ok = matches!(
            (n.tool, kind),
            (Tool::Geas | Tool::Koyomi, Some("claim")) | (Tool::Rulec | Tool::Koyomi | Tool::Chobo | Tool::Dandori | Tool::Geas | Tool::Sakai | Tool::File, None)
        );
        if !ok {
            let t = n.text();
            return Err(("E403", tr!("{t} は何も確かめないので、`verified by` に書けません", "{t} checks nothing, so it cannot be written after `verified by`"), vec![
                tr!(
                    "確かめる側に書けるのは、geas と koyomi の主張（`claim`）、検査するツールのファイル全体（`rulec \"x.rule\"` なら rulec の検査）、テストのファイル（`file \"…\"`）のような、落ちることのあるものです。",
                    "The side that verifies takes what can fail: a geas or koyomi claim (`claim`), the whole file of a tool that checks it (`rulec \"x.rule\"` stands for rulec's check), or a test file (`file \"…\"`)."
                ),
                tr!("満たす成果物なら `satisfied by` に書きます。", "If it is what meets the requirement, it goes after `satisfied by`."),
            ]));
        }
    }
    Ok(())
}

/// Whether the kind a scope gathers can be gathered: not `source`.
fn gathered_ok(_n: &Name, g: Option<&str>) -> Result<(), Refused> {
    if g == Some("source") {
        return Err(("E012", tr!("出典（`source`）は範囲に書けません", "A source (`source`) is not written in a scope"), vec![]));
    }
    Ok(())
}
