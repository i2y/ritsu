//! The lines that wait for a file that does not pass (DESIGN 8.3). A line of one file of the
//! project that reads another (`use dates terms from "…"`, `flow "stage.flow"`, `workflow … from
//! "…"`, `satisfied by koyomi "…"`), at which the file's language says the file read does not pass
//! its check (dandori's E005, sekisho's E208, yuen's E203 …), when that file is one of the
//! project's and does not pass: it is no error of its own, and goes once the file read passes. A
//! file whose every error is such a line waits, and so does one whose check fails only for errors
//! said at other files (a rule with a contribution that does not pass, whose error rulec says at
//! the contribution); the files that do not pass of their own are what the lines wait for, through
//! every file that waits (the roots). `ritsu check` and `ritsu gen` say a root's errors once, where
//! its language says them, and each line that waits once, under the roots it waits for, after
//! everything else.

use ritsu_base::diag::Severity;
use ritsu_base::naming::Tool;
use ritsu_base::text::Lang;
use ritsu_ports::{Checked, Finding, Part, Verdict};
use ritsu_project::{Joined, Project};
use std::collections::{BTreeMap, BTreeSet};

/// Whether a language says `code`, at a line that reads another file, when the file read does not
/// pass its check or cannot be read: rulec's `import proto` (E013), `apply` (E044), `import enum`
/// (E066), the file a contribution adds to or comes after, and the contribution a file reads (E067),
/// and `range from koyomi` (E130); koyomi's `use calendar` (E015); kaname's `use` (E201); dandori's
/// `use rule`, `use dates`, `use book` and `import enum` (E005), `flow "…"` (E015) and `use proto`
/// (E016); sekisho's `use` (E201) and `workflow … from` (E208); ringi's `use` (E202); yuen's links
/// (E203, E205); sakai's artifacts and `.proto` files (E105, E106). The same codes say other things wrong with the line
/// too (a child flow that does not fit, a path to no file, an enum the file does not have, an OpenAPI
/// document, which no language checks): a line waits only when what it reads is a file of the
/// project that does not pass.
pub fn reads_failing(tool: Tool, code: &str) -> bool {
    match tool {
        Tool::Rulec => matches!(code, "E013" | "E044" | "E066" | "E067" | "E130"),
        Tool::Koyomi => code == "E015",
        Tool::Kaname => code == "E201",
        Tool::Dandori => matches!(code, "E005" | "E015" | "E016"),
        Tool::Sekisho => matches!(code, "E201" | "E208"),
        Tool::Ringi => code == "E202",
        Tool::Yuen => matches!(code, "E203" | "E205"),
        Tool::Sakai => matches!(code, "E105" | "E106"),
        _ => false,
    }
}

/// A line that waits.
#[derive(Clone, Debug)]
pub struct Wait {
    /// The unit and the part of it the finding is: `units[at.0].1.parts[at.1]`.
    pub at: (usize, usize),
    pub tool: Tool,
    /// The file the line is in, from the root, and the line.
    pub file: String,
    pub line: usize,
    /// The file of the project the line reads, which does not pass: the file it names, or one a
    /// `.proto` it names imports, which is read with it.
    pub reads: String,
    /// The files that do not pass of their own that the line waits for, through every file that
    /// waits, from the root.
    pub roots: Vec<String>,
}

/// The lines that wait, and the files that wait, of a check.
#[derive(Clone, Debug, Default)]
pub struct Waiting {
    pub lines: Vec<Wait>,
    /// Each file whose every error is a line that waits, or whose check fails only for errors said
    /// at other files, with the roots it waits for.
    pub files: BTreeMap<String, Vec<String>>,
}

impl Waiting {
    /// The lines that wait among the findings of `units`, the units of the files of `project`
    /// (`others` are the findings said beside them: ritsu's own, sakai's), and the files that wait.
    /// What each line reads is what its language's references say the line names
    /// ([`Project::named`]); `one_file` are the languages whose unit is a file.
    pub fn of(project: &Project, units: &[(Tool, Checked)], others: &[&Finding], joined: &Joined, one_file: &[Tool]) -> Waiting {
        // the findings that may wait: an error at a line of a file, with a code that says the file
        // the line reads does not pass. rulec says a contribution it cannot read at the file it adds
        // to with no line, which its JSON writes as the first
        let mut maybe: Vec<((usize, usize), Tool, String, usize)> = Vec::new();
        for (i, (t, u)) in units.iter().enumerate() {
            for (j, p) in u.parts.iter().enumerate() {
                if let Part::Finding(f) = p
                    && f.severity == Severity::Error
                    && reads_failing(*t, &f.code)
                    && let Some(file) = &f.file
                    && let Some(line) = f.line.or_else(|| (*t == Tool::Rulec && f.code == "E067").then_some(1))
                {
                    maybe.push(((i, j), *t, file.clone(), line));
                }
            }
        }
        let held: BTreeSet<&str> = project.files.iter().map(|f| f.rel.as_str()).collect();
        let named = if maybe.is_empty() { Vec::new() } else { project.named(joined).0 };
        // what each reads among the files of the project; one that reads none of them (a path to
        // no file, a file outside the check) is an error of its own
        let mut cands: Vec<(((usize, usize), Tool, String, usize), Vec<String>)> = Vec::new();
        for m in maybe {
            let mut reads: Vec<String> = named.iter().filter(|(f, l, t)| f.rel == m.2 && *l == m.3 && *t != m.2 && held.contains(t.as_str())).map(|(_, _, t)| t.clone()).collect();
            // rulec reads the contributions to a file when it checks the file, though no line of the
            // file names them, and says one it cannot read at the file's first line (E067). A
            // contribution that cannot be read cannot say what it adds to either: the line reads the
            // contribution the finding names
            if reads.is_empty()
                && m.1 == Tool::Rulec
                && let Part::Finding(f) = &units[m.0.0].1.parts[m.0.1]
                && f.code == "E067"
            {
                let in_text = |c: &ritsu_project::File| f.text.contains(&format!("`{}`", c.rel)) || f.text.contains(&format!("`{}`", c.shown));
                reads = project.files.iter().filter(|c| c.tool == Tool::Rulec && c.rel != m.2 && in_text(c)).map(|c| c.rel.clone()).collect();
            }
            // a `.proto` is read with the `.proto` files it imports: a line that reads one reads them
            // too, and waits for one of them that does not pass, though the file it names passes
            let mut i = 0;
            while i < reads.len() {
                if project.holds(&reads[i]).is_some_and(|p| p.tool == Tool::Proto) {
                    let more: Vec<String> = named.iter().filter(|(p, _, t)| p.rel == reads[i] && held.contains(t.as_str()) && !reads.contains(t)).map(|(_, _, t)| t.clone()).collect();
                    reads.extend(more);
                }
                i += 1;
            }
            reads.sort();
            reads.dedup();
            // sakai says it at the artifact itself (E105, E106): its check of the references the
            // artifact makes waits for the artifact to pass its own language's check
            if reads.is_empty() && m.1 == Tool::Sakai && held.contains(m.2.as_str()) {
                reads.push(m.2.clone());
            }
            if !reads.is_empty() {
                cands.push((m, reads));
            }
        }
        let keys: BTreeSet<(usize, usize)> = cands.iter().map(|((k, ..), _)| *k).collect();
        // the files that do not pass for a reason of their own: one that cannot be checked, an error
        // at it that is no line that may wait, a unit of it that fails with no such line and nothing
        // said at it (what it says is said at other files)
        let error_at = |rel: &str| {
            units.iter().enumerate().any(|(i, (_, u))| {
                u.parts.iter().enumerate().any(|(j, p)| matches!(p, Part::Finding(f) if f.severity == Severity::Error && f.file.as_deref() == Some(rel) && !keys.contains(&(i, j))))
            }) || others.iter().any(|f| f.severity == Severity::Error && f.file.as_deref() == Some(rel))
        };
        let mut own: BTreeSet<String> = BTreeSet::new();
        // a file whose check fails only for errors said at other files of the project fails through
        // them: a rule with a contribution whose error rulec says at the contribution's line (DESIGN
        // 17.8). It waits for them, and what reads it waits for them through it. A rule of a
        // dependency that so fails passes, as the dependency's own check passes it, and what reads it
        // waits for them all the same
        let mut through: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for f in &project.files {
            let one = one_file.contains(&f.tool);
            let mine = |u: &&(Tool, Checked)| u.0 == f.tool && (!one || u.1.label == f.shown);
            let unchecked = units.iter().filter(mine).any(|(_, u)| u.verdict == Verdict::Unchecked);
            let elsewhere = |u: &Checked| -> Option<Vec<String>> {
                let mut at: Vec<Option<String>> = u.findings().filter(|x| x.severity == Severity::Error).map(|x| x.file.clone()).collect();
                // rulec says a finding once, for the first file whose check finds it: a rule whose
                // errors were all said for a file above says none, and its engine tells where they are
                if at.is_empty() && f.tool == Tool::Rulec {
                    at = joined.rulec.errors_at(&u.label).iter().map(|p| ritsu_base::paths::from_root(&project.root, std::path::Path::new(p))).collect();
                }
                let others: Vec<String> = at.iter().flatten().filter(|x| **x != f.rel && held.contains(x.as_str())).cloned().collect();
                (!at.is_empty() && others.len() == at.len()).then_some(others)
            };
            let mut silent = false;
            for (i, (t, u)) in units.iter().enumerate() {
                if !one || *t != f.tool || u.label != f.shown || u.verdict != Verdict::Fails || (0..u.parts.len()).any(|j| keys.contains(&(i, j))) {
                    continue;
                }
                match elsewhere(u) {
                    Some(at) => through.entry(f.rel.clone()).or_default().extend(at),
                    None => silent |= !(project.dependency_of(&f.rel).is_some() && project.failed_for_others(&f.rel, u)),
                }
            }
            if unchecked || error_at(&f.rel) || silent {
                own.insert(f.rel.clone());
            }
        }
        through.retain(|f, _| !own.contains(f));
        for at in through.values_mut() {
            at.sort();
            at.dedup();
        }
        if cands.is_empty() && through.is_empty() {
            return Waiting::default();
        }
        // every file with such a line does not pass, whether the line waits or not
        let bad: BTreeSet<String> = own.iter().cloned().chain(cands.iter().map(|((_, _, f, _), _)| f.clone())).chain(through.keys().cloned()).collect();
        // a line that reads only files that pass says something wrong of its own (a child flow that
        // does not fit it)
        let mut alone: BTreeSet<(usize, usize)> = cands.iter().filter(|(_, r)| r.iter().all(|t| !bad.contains(t))).map(|((k, ..), _)| *k).collect();
        let mut rooted: BTreeSet<String> = own.clone();
        rooted.extend(cands.iter().filter(|((k, ..), _)| alone.contains(k)).map(|((_, _, f, _), _)| f.clone()));
        let mut waits: BTreeSet<(usize, usize)> = BTreeSet::new();
        loop {
            let mut changed = false;
            for ((k, _, f, _), r) in &cands {
                if !waits.contains(k) && !alone.contains(k) && r.iter().any(|t| rooted.contains(t)) {
                    waits.insert(*k);
                    changed |= rooted.insert(f.clone());
                }
            }
            for (f, at) in &through {
                if !rooted.contains(f) && at.iter().any(|t| rooted.contains(t)) {
                    changed |= rooted.insert(f.clone());
                }
            }
            if changed {
                continue;
            }
            // what is left reads only files that read each other round in a ring, which nothing
            // outside makes fail: each such line is an error of its own, and what reads its file
            // waits for it
            let left: Vec<&(((usize, usize), Tool, String, usize), Vec<String>)> = cands.iter().filter(|((k, ..), _)| !waits.contains(k) && !alone.contains(k)).collect();
            if left.is_empty() {
                break;
            }
            for ((k, _, f, _), _) in left {
                alone.insert(*k);
                rooted.insert(f.clone());
            }
        }
        // the files that do not pass of their own: the roots
        let roots_own: BTreeSet<String> = own.iter().cloned().chain(cands.iter().filter(|((k, ..), _)| alone.contains(k)).map(|((_, _, f, _), _)| f.clone())).collect();
        let waiting: Vec<&(((usize, usize), Tool, String, usize), Vec<String>)> = cands.iter().filter(|((k, ..), _)| waits.contains(k)).collect();
        // the roots a file stands for: itself when it does not pass of its own, and the roots of
        // each of its lines that wait and of each file it fails through
        struct Graph<'a> {
            own: &'a BTreeSet<String>,
            waiting: &'a [&'a (((usize, usize), Tool, String, usize), Vec<String>)],
            through: &'a BTreeMap<String, Vec<String>>,
            rooted: &'a BTreeSet<String>,
        }
        fn roots_of(file: &str, g: &Graph, seen: &mut BTreeSet<String>) -> BTreeSet<String> {
            let mut out = BTreeSet::new();
            if !seen.insert(file.to_string()) {
                return out;
            }
            if g.own.contains(file) {
                out.insert(file.to_string());
            }
            let read = g.waiting.iter().filter(|((_, _, f, _), _)| f == file).flat_map(|(_, r)| r.iter());
            for t in read.chain(g.through.get(file).into_iter().flatten()).filter(|t| g.rooted.contains(*t)) {
                out.extend(roots_of(t, g, seen));
            }
            out
        }
        let g = Graph { own: &roots_own, waiting: &waiting, through: &through, rooted: &rooted };
        let mut out = Waiting::default();
        for ((k, tool, file, line), r) in &waiting {
            let read: Vec<&String> = r.iter().filter(|t| rooted.contains(*t)).collect();
            let mut roots = BTreeSet::new();
            for t in &read {
                roots.extend(roots_of(t, &g, &mut BTreeSet::new()));
            }
            out.lines.push(Wait { at: *k, tool: *tool, file: file.clone(), line: *line, reads: read.first().map(|s| s.to_string()).unwrap_or_default(), roots: roots.into_iter().collect() });
        }
        for w in &out.lines {
            if !roots_own.contains(&w.file) {
                let e = out.files.entry(w.file.clone()).or_default();
                e.extend(w.roots.iter().cloned());
                e.sort();
                e.dedup();
            }
        }
        // a file that fails through others waits for their roots; one of a dependency passes
        for f in through.keys().filter(|f| rooted.contains(*f) && project.dependency_of(f).is_none()) {
            let e = out.files.entry(f.clone()).or_default();
            e.extend(roots_of(f, &g, &mut BTreeSet::new()));
            e.sort();
            e.dedup();
        }
        out
    }

    /// The line that waits at part `j` of unit `i`, if it is one.
    pub fn at(&self, i: usize, j: usize) -> Option<&Wait> {
        self.lines.iter().find(|w| w.at == (i, j))
    }

    /// What `ritsu check` prints of the lines that wait: for each set of roots, in the order the
    /// first error of each is said, a line that names them, then each line that waits for them, in
    /// the order the files are checked, with its language's tool and code and the line as the file
    /// writes it. Each file is named as the rest of the text names it, from where the command runs.
    ///
    /// ```text
    /// waiting for dates/approval_terms.cal (koyomi E301 at line 19) to pass:
    ///   flows/stage.flow:5: [dandori E005] use dates terms from "../dates/approval_terms.cal"
    /// ```
    pub fn text(&self, project: &Project, units: &[(Tool, Checked)], others: &[(&str, &Finding)], lang: Lang) -> String {
        if self.lines.is_empty() {
            return String::new();
        }
        // the errors that are said, in the order they are said: each root's first, and how many
        let mut said: Vec<(&str, &Finding)> = Vec::new();
        for (i, (t, u)) in units.iter().enumerate() {
            for (j, p) in u.parts.iter().enumerate() {
                if let Part::Finding(f) = p
                    && f.severity == Severity::Error
                    && self.at(i, j).is_none()
                {
                    said.push((t.word(), f));
                }
            }
        }
        said.extend(others.iter().filter(|(_, f)| f.severity == Severity::Error).map(|(t, f)| (*t, *f)));
        // the file a finding is at: rulec files what it says of a line of a contribution under the
        // file it checks, and writes the line's file in the JSON's `where` (the check files it
        // there, ritsu-project's `place_findings`)
        let at = |t: &str, f: &Finding, root: &str| {
            let placed = (t == Tool::Rulec.word())
                .then(|| f.json.get("where").and_then(|w| w.get("file")).and_then(|x| x.as_str()))
                .flatten()
                .and_then(|s| ritsu_base::paths::from_root(&project.root, std::path::Path::new(s)));
            placed.or_else(|| f.file.clone()).as_deref() == Some(root)
        };
        let first = |root: &str| said.iter().position(|(t, f)| at(t, f, root)).unwrap_or(usize::MAX);
        let mut groups: Vec<(&Vec<String>, Vec<&Wait>)> = Vec::new();
        for w in &self.lines {
            match groups.iter_mut().find(|(r, _)| **r == w.roots) {
                Some((_, ws)) => ws.push(w),
                None => groups.push((&w.roots, vec![w])),
            }
        }
        groups.sort_by_key(|(r, _)| (r.iter().map(|x| first(x)).min().unwrap_or(usize::MAX), (*r).clone()));
        // each file as the rest of the text names it, from where the command runs
        let shown = |rel: &str| project.holds(rel).map_or_else(|| rel.to_string(), |f| f.shown.clone());
        let mut out = String::new();
        for (roots, ws) in groups {
            let named: Vec<String> = roots
                .iter()
                .map(|root| {
                    let errors: Vec<&(&str, &Finding)> = said.iter().filter(|(t, f)| at(t, f, root)).collect();
                    let unchecked = project.holds(root).is_some_and(|f| units.iter().any(|(t, u)| *t == f.tool && u.label == f.shown && u.verdict == Verdict::Unchecked));
                    let name = shown(root);
                    match errors.first() {
                        Some((tool, f)) => {
                            let more = errors.len() - 1;
                            let at = f.line.map(|l| (format!("、{l} 行目"), format!(" at line {l}"))).unwrap_or_default();
                            let (mj, me) = if more == 0 { (String::new(), String::new()) } else { (format!("、ほか {more} 件"), format!(", and {more} more")) };
                            let code = &f.code;
                            pick(lang, format!("{name}（{tool} {code}{}{mj}）", at.0), format!("{name} ({tool} {code}{}{me})", at.1))
                        }
                        None if unchecked => pick(lang, format!("{name}（確かめられません）"), format!("{name} (could not be checked)")),
                        None => name,
                    }
                })
                .collect();
            let (list, sep) = match lang {
                Lang::Ja => {
                    let list = named.iter().skip(1).fold(named.first().cloned().unwrap_or_default(), |a, b| if a.ends_with('）') { format!("{a}と {b}") } else { format!("{a} と {b}") });
                    let sep = if list.ends_with('）') { "" } else { " " };
                    (list, sep)
                }
                _ => (and_list(&named), ""),
            };
            out.push_str(&pick(lang, format!("{list}{sep}が通るのを待っています:\n"), format!("waiting for {list} to pass:\n")));
            for w in ws {
                let Part::Finding(f) = &units[w.at.0].1.parts[w.at.1] else { continue };
                let line = source_line(project, &w.file, w.line);
                let sep = if line.is_empty() { "" } else { " " };
                out.push_str(&format!("  {}:{}: [{} {}]{sep}{line}\n", shown(&w.file), w.line, w.tool.word(), f.code));
            }
        }
        out
    }
}

fn pick(lang: Lang, ja: String, en: String) -> String {
    if lang == Lang::Ja { ja } else { en }
}

/// `a`, `a and b`, `a, b and c`.
fn and_list(xs: &[String]) -> String {
    match xs {
        [] => String::new(),
        [a] => a.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The line `line` of the file `rel` of the project, as the file writes it without the space around
/// it; empty when it cannot be read.
fn source_line(project: &Project, rel: &str, line: usize) -> String {
    let Ok(bytes) = ritsu_base::fs::read(ritsu_base::paths::on_disk(&project.root, rel)) else { return String::new() };
    String::from_utf8_lossy(&bytes).lines().nth(line.saturating_sub(1)).map(|l| l.trim().to_string()).unwrap_or_default()
}
