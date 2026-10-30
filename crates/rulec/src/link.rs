//! Merging contributions into the rules they add to (§15.227–§15.231).
//!
//! A rule says where it may be added to — an `open` value and its terms, an `open` enum, a table
//! or a clause marked `open`, an `open` block of prerequisites — and a `contribution` file adds to
//! those places, naming the rule with `to`. Which contributions are merged into a rule is decided
//! by the files a run is given (`with_set`): every contribution among them, and every one those
//! come after (`after`), that reaches the rule. There is no list of what to load, and the four
//! ways of combining a value do not depend on an order.
//!
//! The merge happens on the parse tree, after the `apply`s are expanded and before anything is
//! typed, so everything downstream — the checks, the evaluator, the twelve generators, the page,
//! the certificate — sees one ordinary rule, as it sees an `apply` expanded. A contribution's own
//! names come in as `<contribution>:<name>`; its inputs are the rule's inputs, the same input when
//! the name is the same; its tables and clauses join the definitions of the open outputs they
//! define, placed right after the last of them, since a set of definitions is decided where its
//! last member is.
//!
//! An `open` value becomes a `define` of its combination where it is written (`lower`). For the
//! checks it is not computed: its declared range is the whole of what the tables below it are
//! checked over (types: `Checked::opens`), so the rule's own check holds whatever the contributions
//! are; that the combination stays inside the range is E131.

use crate::ast::*;
use crate::diag::{Diag, Severity, Span};
use std::cell::RefCell;
use std::path::{Component, Path, PathBuf};

/// The pins: the shape a contribution relies on, and the meaning an `apply` relies on (§15.230).
pub mod pin;

/// How a contribution file is written, for the notes of E067.
pub(crate) fn shape_note() -> String {
    tr!(
        "寄与のファイルは `contribution <名前>(<別名>) v<版>` で始め、`to \"<元のファイル>\" signature:<16 桁>` で足す先を言い、`after \"<寄与のファイル>\" signature:<16 桁>`、`inputs`、`extend <列挙> with <値>`、表・節・定義・導出、`into <開いた値>` と項、`prerequisites <塊>` と `<値> requires …`、`examples` を書きます。",
        "A contribution file starts with `contribution <name>(<alias>) v<version>`, says where it adds with `to \"<file>\" signature:<16 hex>`, and holds `after \"<contribution>\" signature:<16 hex>`, `inputs`, `extend <enum> with <value>`, tables, clauses, definitions and derives, `into <open value>` with its terms, `prerequisites <block>` with `<value> requires …` lines, and `examples`."
    )
}

/// How an `open` line is written, for the notes of E068.
pub(crate) fn open_shape_note() -> String {
    tr!(
        "形は `open <名前>(<別名>) : <型> combine sum|product|max|min [empty <値>] range >=… <=…` の行と、その下に一行に一つずつの項の名前です。",
        "The shape is `open <name>(<alias>) : <type> combine sum|product|max|min [empty <value>] range >=… <=…`, with the names of its terms under it, one to a line."
    )
}

thread_local! {
    /// The contribution files of the run, as given (`with_set`). `None` outside any run that
    /// gives files: then no contribution is merged, as before contributions were.
    static SET: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// Puts the set back however the closure ends.
struct Restore(Option<Vec<String>>);

impl Drop for Restore {
    fn drop(&mut self) {
        let b = self.0.take();
        SET.with(|s| *s.borrow_mut() = b);
    }
}

/// Runs `f` with the contributions among `files` (a run's files, directories expanded) as what is
/// merged into the rules they reach (§15.227).
pub fn with_set<R>(files: &[String], f: impl FnOnce() -> R) -> R {
    let contribs: Vec<String> = files.iter().filter(|p| is_contribution_file(p)).map(|p| normalize(Path::new(p))).collect();
    with_only(contribs, f)
}

fn with_only<R>(contribs: Vec<String>, f: impl FnOnce() -> R) -> R {
    let _r = Restore(SET.with(|s| s.replace(Some(contribs))));
    f()
}

/// Runs `f` with no contribution merged: what the rule claims when there are none (§15.231).
pub fn without<R>(f: impl FnOnce() -> R) -> R {
    with_only(Vec::new(), f)
}

/// The contribution files of the run.
fn active() -> Vec<String> {
    SET.with(|s| s.borrow().clone().unwrap_or_default())
}

/// Whether the file at `path` is a `contribution` file: its first line that is neither blank nor a
/// comment starts with `contribution`. A file that cannot be read is not one.
pub fn is_contribution_file(path: &str) -> bool {
    let Ok(bytes) = crate::vfs::read(Path::new(path)) else { return false };
    let src = String::from_utf8_lossy(&bytes);
    src.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .is_some_and(|l| l.split_whitespace().next() == Some(crate::kw::CONTRIBUTION))
}

/// `p` with `.` and `name/..` folded, `/` between its parts.
pub fn normalize(p: &Path) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut abs = false;
    for c in p.components() {
        match c {
            Component::RootDir => abs = true,
            Component::CurDir | Component::Prefix(_) => {}
            Component::ParentDir => {
                if out.last().is_some_and(|l| l != "..") {
                    out.pop();
                } else if !abs {
                    out.push("..".into());
                }
            }
            Component::Normal(s) => out.push(s.to_string_lossy().into_owned()),
        }
    }
    let s = out.join("/");
    if abs {
        format!("/{s}")
    } else if s.is_empty() {
        ".".into()
    } else {
        s
    }
}

/// The file `rel` names from the directory of the file at `base`, as the run shows it. None for a
/// path the module of ritsu's the file is in refuses (ritsu's DESIGN 17.6), which E067 says where
/// the pins are checked ([`pin::check_pins`]).
pub fn join(base: &str, rel: &str) -> Option<String> {
    crate::vfs::follow(crate::vfs::dir_of(base, ""), rel).ok().map(|p| normalize(&p))
}

/// The file a path names, its links followed: how two spellings of one file are told to be one.
fn canon(p: &str) -> PathBuf {
    ritsu_base::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(normalize(Path::new(p))))
}

fn same_file(a: &str, b: &str) -> bool {
    a == b || canon(a) == canon(b)
}

/// One file read for the merge: as the run shows it, its text, and its parse. `tagged` marks every
/// span of it as in this file (`diag::in_file`), for a file that is not the one being checked.
struct Read {
    shown: String,
    src: String,
    file: Option<RuleFile>,
    diags: Vec<Diag>,
}

fn read(shown: &str, tagged: bool) -> Option<Read> {
    let bytes = crate::vfs::read(Path::new(shown)).ok()?;
    let src = String::from_utf8_lossy(&bytes).into_owned();
    let parsed = if tagged { crate::diag::in_file(shown, &src, || crate::parse::parse(&src, shown)) } else { crate::parse::parse(&src, shown) };
    Some(Read { shown: shown.to_string(), src, file: parsed.file, diags: parsed.diags })
}

/// The contributions the run merges, and every one those come after or add to: the contributions
/// `starts` names, closed over `after` and over a `to` that names a contribution (§15.227).
fn closure(starts: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut queue: Vec<String> = starts.to_vec();
    while let Some(p) = queue.pop() {
        if out.iter().any(|q| same_file(q, &p)) {
            continue;
        }
        let Some(f) = read(&p, true).and_then(|r| r.file) else {
            out.push(p);
            continue;
        };
        if f.kind != FileKind::Contribution {
            continue;
        }
        if let Some(h) = &f.contrib {
            for a in &h.afters {
                queue.extend(join(&p, &a.file));
            }
            if let Some(t) = &h.to
                && let Some(tp) = join(&p, &t.file)
                && is_contribution_file(&tp)
            {
                queue.push(tp);
            }
        }
        out.push(p);
    }
    out.sort();
    out
}

/// The rule or `enums` file a contribution comes down to, following `to` through the
/// contributions it adds to. `None` when a `to` is missing or the chain goes round.
fn root_of(path: &str) -> Option<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut p = path.to_string();
    loop {
        if seen.iter().any(|q| same_file(q, &p)) {
            return None;
        }
        seen.push(p.clone());
        let f = read(&p, true)?.file?;
        if f.kind != FileKind::Contribution {
            return Some(p);
        }
        let t = f.contrib.as_ref()?.to.as_ref()?;
        p = join(&p, &t.file)?;
    }
}

/// Whether a contribution of the run reaches the rule at `path`: what makes its check two, one of
/// the rule alone and one of the rule merged (§15.231).
pub fn reaches(path: &str) -> bool {
    let set = active();
    if set.is_empty() {
        return false;
    }
    closure(&set).iter().any(|c| root_of(c).is_some_and(|r| same_file(&r, path)))
}

/// The contributions of the run merged into the rule at `path`, as the run shows them: for the
/// generated header, `check`'s line and the page.
pub fn merged_into(path: &str) -> Vec<String> {
    let set = active();
    closure(&set).into_iter().filter(|c| root_of(c).is_some_and(|r| same_file(&r, path))).collect()
}

/// What a contribution names outside itself, its `to` and each `after`, with the line each is
/// written on and the file as the run shows it: what `ritsu check` reads for the contributions that
/// cross from one module to another, and for the dependencies a module uses (ritsu's DESIGN 17.8).
/// Empty for a file that is no contribution, or that does not read.
pub fn names_of(path: &str) -> Vec<(usize, String)> {
    let Some(f) = read(path, false).and_then(|r| r.file) else { return Vec::new() };
    let Some(h) = &f.contrib else { return Vec::new() };
    h.to.iter().chain(&h.afters).filter_map(|t| Some((t.span.line, join(path, &t.file)?))).collect()
}

/// The file a contribution adds to, as the run shows it, and the rule it comes down to.
pub fn target_of(path: &str) -> Option<(String, String)> {
    let f = read(path, false)?.file?;
    let t = f.contrib.as_ref()?.to.as_ref()?;
    Some((join(path, &t.file)?, root_of(path)?))
}

/// A contribution read for the merge: as the run shows it, its text and its parse, and the file it
/// adds to.
pub(crate) struct Contrib {
    pub(crate) shown: String,
    pub(crate) src: String,
    pub(crate) file: RuleFile,
    pub(crate) to: String,
}

/// Merges into `f`, read from `path`, what the run adds to it, and lowers its `open` values to
/// `define`s (§15.227). For a rule: every contribution of the run that reaches it, with its own
/// examples left out (they are claims about the rule alone, which `check` runs separately). For an
/// `enums` file: the values contributions add to its open enums and the lines they add to its open
/// blocks. For a contribution: the rule it comes down to, merged with it and the contributions it
/// comes after, with its own examples kept — its scope, which is what its check is about.
pub fn merge(f: &mut RuleFile, path: &str) -> Vec<Diag> {
    match f.kind {
        FileKind::Contribution => scope(f, path),
        _ => {
            let set = active();
            let mut cs: Vec<String> = Vec::new();
            if !set.is_empty() {
                for c in closure(&set) {
                    if root_of(&c).is_some_and(|r| same_file(&r, path)) {
                        cs.push(c);
                    }
                }
            }
            let mut out = Vec::new();
            let mut read_cs: Vec<Contrib> = Vec::new();
            for c in &cs {
                match read_contrib(c, true) {
                    Ok(x) => read_cs.push(x),
                    Err(ds) => out.extend(ds),
                }
            }
            if has_error(&out) {
                return out;
            }
            if !read_cs.is_empty() {
                // The rule's examples, sequences and scenarios are claims about it alone; they run
                // in its check without the contributions. Each contribution's run in its own. Its
                // claims of chances hold whatever is added (§15.224), so the sequences they name stay.
                let named = chance_sequences(f);
                f.examples.clear();
                f.sequences.retain(|sq| named.contains(&sq.name.text));
                f.scenarios.clear();
            }
            out.extend(merge_into(f, path, read_cs, None));
            out
        }
    }
}

/// The sequences a rule's claims of chances name, in a column of the elements they walk (§15.224).
fn chance_sequences(f: &RuleFile) -> Vec<String> {
    let mut out = Vec::new();
    for ch in &f.chances {
        for (ci, (n, _)) in ch.inputs.iter().enumerate() {
            if !f.elements.iter().any(|el| el.name.text == *n) {
                continue;
            }
            for row in &ch.rows {
                if let Some(Cell::Lit(Lit::Word(w))) = row.cells.get(ci) {
                    out.push(w.clone());
                }
            }
        }
    }
    out
}

fn has_error(ds: &[Diag]) -> bool {
    ds.iter().any(|d| d.severity == Severity::Error)
}

/// Reads a contribution for the merge. Its own parse errors stop the merge; the check of the file
/// itself says them, where they are.
fn read_contrib(path: &str, tagged: bool) -> Result<Contrib, Vec<Diag>> {
    let Some(r) = read(path, tagged) else {
        return Err(vec![Diag::error("E067", tr!("寄与のファイル `{path}` を読めません", "The contribution file `{path}` cannot be read")).at(path.to_string())]);
    };
    if has_error(&r.diags) {
        return Err(vec![
            Diag::error("E067", tr!("寄与のファイル `{path}` を読めません", "The contribution file `{path}` cannot be read"))
                .at(path.to_string())
                .note(tr!("先にそのファイルを `rulec check {path}` で直してください。", "Fix it first: `rulec check {path}` says what is wrong with it.")),
        ]);
    }
    let Some(file) = r.file else { return Err(Vec::new()) };
    let to = match file.contrib.as_ref().and_then(|h| h.to.as_ref()) {
        Some(t) => join(path, &t.file).unwrap_or_default(),
        None => String::new(),
    };
    Ok(Contrib { shown: r.shown, src: r.src, file, to })
}

/// A contribution's check is about its scope (§15.231): the rule it comes down to, merged with it
/// and with what it comes after, and its own examples. `f` is the contribution as parsed (its spans
/// are the file being checked); it becomes that merged rule.
fn scope(f: &mut RuleFile, path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    let head = f.contrib.clone().unwrap_or_else(|| ContribHead { to: None, afters: Vec::new(), extends: Vec::new(), intos: Vec::new(), prereq_lines: Vec::new() });
    let Some(to) = &head.to else {
        out.push(
            Diag::error("E067", tr!("寄与に `to` の行がありません", "The contribution has no `to` line"))
                .at(format!("{path}:{}", f.name.span.line))
                .mark(f.name.span.clone(), tr!("どのファイルに足すかを言っていません", "it does not say which file it adds to"))
                .note(shape_note()),
        );
        return out;
    };
    let dir = crate::vfs::dir_of(path, "");
    let followed = crate::vfs::follow(dir, &to.file);
    if let Some(why) = crate::vfs::refused(&followed) {
        out.push(Diag::error("E067", why).at(format!("{path}:{}", to.span.line)).mark(to.span.clone(), ""));
        return out;
    }
    let target = followed.map(|p| normalize(&p)).unwrap_or_default();
    let Some(root) = root_of(&target) else {
        out.push(
            Diag::error("E067", tr!("足す先 `{}` を読めません", "The file `{}` it adds to cannot be read", to.file))
                .at(format!("{path}:{}", to.span.line))
                .mark(to.span.clone(), "")
                .note(tr!("パスは寄与のファイルのある場所からたどります（探した先: {target}）。", "The path is followed from the directory of the contribution (looked for: {target})."))
                .maybe_note(crate::vfs::outside_note(dir, &to.file)),
        );
        return out;
    };
    // What this one comes after and adds to, the contributions among them read from their files.
    // Those that come down to another file — the values a contribution adds to an `enums` file the
    // rule reads — reach the rule through that file, as it is read below.
    let others: Vec<String> = closure(&[path.to_string()]).into_iter().filter(|c| !same_file(c, path)).collect();
    let mut cs: Vec<Contrib> = Vec::new();
    for c in &others {
        if !root_of(c).is_some_and(|r| same_file(&r, &root)) {
            continue;
        }
        match read_contrib(c, true) {
            Ok(x) => cs.push(x),
            Err(ds) => out.extend(ds),
        }
    }
    let src = crate::vfs::read(Path::new(path)).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    cs.push(Contrib { shown: normalize(Path::new(path)), src, file: f.clone(), to: target.clone() });
    if has_error(&out) {
        return out;
    }
    let Some(r) = read(&root, true) else { return out };
    if has_error(&r.diags) {
        out.push(
            Diag::error("E067", tr!("足す先 `{root}` が check を通りません", "The file `{root}` it adds to does not pass check"))
                .at(format!("{path}:{}", to.span.line))
                .mark(to.span.clone(), "")
                .note(tr!("先に `rulec check {root}` でそのファイルを直してください。", "Fix it first: `rulec check {root}` says what is wrong with it.")),
        );
        return out;
    }
    let Some(mut rf) = r.file else { return out };
    // What the rule cites is followed from the rule's directory, not the contribution's (§15.235).
    cites_from(&mut rf, &root);
    let mut mine = others.clone();
    mine.push(normalize(Path::new(path)));
    // The rule is read as its own check reads it — its enums from their files, its applies
    // expanded — with only this scope's contributions merged into the files it reads.
    let ds = with_only(mine, || {
        let mut ds = crate::diag::in_file(&root, &r.src, || crate::enums::read_imports(&mut rf, &root));
        if !has_error(&ds) {
            ds.extend(crate::diag::in_file(&root, &r.src, || crate::apply::expand(&mut rf, &root)));
        }
        ds
    });
    if ds.iter().any(|d| d.severity == Severity::Error && d.code != "E040") {
        out.push(
            Diag::error("E067", tr!("足す先 `{root}` が check を通りません", "The file `{root}` it adds to does not pass check"))
                .at(format!("{path}:{}", to.span.line))
                .mark(to.span.clone(), "")
                .note(tr!("先に `rulec check {root}` でそのファイルを直してください。", "Fix it first: `rulec check {root}` says what is wrong with it.")),
        );
        return out;
    }
    // The rule's own examples are its claims alone; the scope's are this contribution's. The
    // sequences the rule's claims of chances name stay beside the contribution's (§15.224).
    let named = chance_sequences(&rf);
    let mut sequences = f.sequences.clone();
    for sq in &rf.sequences {
        if named.contains(&sq.name.text) && !sequences.iter().any(|x| x.name.text == sq.name.text) {
            sequences.push(sq.clone());
        }
    }
    rf.examples = f.examples.clone();
    rf.sequences = sequences;
    rf.scenarios.clear();
    let at = cs.len() - 1;
    out.extend(merge_into(&mut rf, &root, cs, Some(at)));
    *f = rf;
    out
}

/// Marks every file `f` cites as followed from the directory of `base`, the file `f` was read from
/// (§15.235): its contracts (`shape`, `implements`), the enums it reads from a `.proto` or a schema (`import`), the
/// koyomi files of its dates, and the copies of its sources. A contribution's check is about the rule
/// it adds to, and runs under the contribution's path; what the rule names is the rule's to find.
/// A source so marked is one the rule's own check holds to its copies, as one an `apply` brought in.
fn cites_from(f: &mut RuleFile, base: &str) {
    for d in f.shapes.iter_mut() {
        d.base.get_or_insert_with(|| base.to_string());
    }
    for im in f.enum_imports.iter_mut() {
        im.base.get_or_insert_with(|| base.to_string());
    }
    if let Some(d) = f.implements.as_mut() {
        d.base.get_or_insert_with(|| base.to_string());
    }
    for s in f.sources.iter_mut() {
        s.base.get_or_insert_with(|| base.to_string());
    }
    let days = f.inputs.iter_mut().chain(f.elements.iter_mut().flat_map(|el| el.fields.iter_mut())).filter_map(|i| i.range.as_mut().and_then(|r| r.days.as_mut()));
    for d in days {
        d.base.get_or_insert_with(|| base.to_string());
    }
}

/// The order the contributions are merged in: each after the ones it comes after and the one it
/// adds to, and otherwise by its file. A ring of `after` is E067.
fn order(cs: Vec<Contrib>) -> (Vec<Contrib>, Vec<Diag>) {
    let n = cs.len();
    let deps: Vec<Vec<usize>> = cs
        .iter()
        .map(|c| {
            let h = c.file.contrib.as_ref();
            let mut d: Vec<usize> = Vec::new();
            let mut wants: Vec<String> = h.map(|h| h.afters.iter().filter_map(|a| join(&c.shown, &a.file)).collect()).unwrap_or_default();
            wants.push(c.to.clone());
            for w in wants {
                if let Some(k) = cs.iter().position(|o| same_file(&o.shown, &w)) {
                    d.push(k);
                }
            }
            d
        })
        .collect();
    let mut done = vec![false; n];
    let mut seq: Vec<usize> = Vec::new();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|a, b| cs[*a].shown.cmp(&cs[*b].shown));
    loop {
        let next = idx.iter().copied().find(|&i| !done[i] && deps[i].iter().all(|&d| done[d] || d == i));
        match next {
            Some(i) => {
                done[i] = true;
                seq.push(i);
            }
            None => break,
        }
    }
    let mut out = Vec::new();
    if seq.len() < n {
        let ring: Vec<String> = (0..n).filter(|i| !done[*i]).map(|i| cs[i].shown.clone()).collect();
        let first = (0..n).find(|i| !done[*i]).unwrap_or(0);
        let sp = cs[first].file.contrib.as_ref().and_then(|h| h.afters.first().map(|a| a.span.clone())).unwrap_or_else(|| cs[first].file.name.span.clone());
        out.push(
            Diag::error("E067", tr!("`after` が輪になっています", "The `after` lines go round"))
                .at(format!("{}:{}", cs[first].shown, sp.line))
                .mark(sp, "")
                .note(tr!("輪になっている寄与: {}", "The contributions in the ring: {}", ring.join(" → ")))
                .note(tr!(
                    "`after` は、ほかの寄与の後に合わせることを言います。互いの後にはなれないので、どちらかの `after` を外してください。",
                    "`after` says a contribution is merged after another. Two cannot each come after the other: take one of the `after` lines out."
                )),
        );
        for i in 0..n {
            if !done[i] {
                seq.push(i);
            }
        }
    }
    let mut slots: Vec<Option<Contrib>> = cs.into_iter().map(Some).collect();
    (seq.into_iter().filter_map(|i| slots[i].take()).collect(), out)
}

/// The names a contribution declares itself, which come into the rule as `<contribution>:<name>`:
/// its definitions, derives and summaries, the answers of its folds, its tables and clauses and the
/// columns they write that name nothing of the file it adds to, and its own open values.
fn locals(c: &RuleFile, outer: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let add = |n: &str, out: &mut Vec<String>| {
        if !out.iter().any(|x| x == n) {
            out.push(n.to_string());
        }
    };
    for it in &c.items {
        match it {
            Item::Derived(d) => add(&d.name.text, &mut out),
            Item::Define(d) => add(&d.name.text, &mut out),
            Item::Agg(d) => add(&d.name.text, &mut out),
            Item::Fold(d) => {
                if let Some(a) = &d.answer {
                    if !outer(&a.name.text) {
                        add(&a.name.text, &mut out);
                    }
                }
            }
            Item::Table(t) => {
                if let Some(n) = &t.name {
                    add(&n.text, &mut out);
                }
                for o in &t.outputs {
                    if !outer(&o.name.text) {
                        add(&o.name.text, &mut out);
                    }
                }
            }
        }
    }
    for o in &c.opens {
        add(&o.name.text, &mut out);
    }
    out
}

/// What a rule declares under `name` that is a value and not an input, and where (§15.262): an
/// output, a sequence or a field of its elements, an open value, or what an item decides. An input
/// a contribution adds under such a name would be one name for two values of the merged rule.
fn value_named(f: &RuleFile, name: &str) -> Option<(String, Span)> {
    let named = |n: &Name| (n.text == name).then(|| n.span.clone());
    let what = |ja: &str, en: &str| if crate::i18n::ja() { ja.to_string() } else { en.to_string() };
    f.outputs
        .iter()
        .find_map(|o| named(&o.name).map(|s| (what("出力", "an output"), s)))
        .or_else(|| f.elements.iter().find_map(|el| named(&el.name).map(|s| (what("たどる並び", "a sequence"), s))))
        .or_else(|| f.elements.iter().flat_map(|el| el.fields.iter()).find_map(|fd| named(&fd.name).map(|s| (what("要素のフィールド", "a field of an element"), s))))
        .or_else(|| f.opens.iter().find_map(|o| named(&o.name).map(|s| (what("開いた値", "an open value"), s))))
        .or_else(|| {
            f.items.iter().find_map(|it| match it {
                Item::Derived(d) => named(&d.name).map(|s| (what("導出", "a derive"), s)),
                Item::Define(d) => named(&d.name).map(|s| (what("定義", "a definition"), s)),
                Item::Agg(d) => named(&d.name).map(|s| (tr!("`{}` の値", "a `{}`", d.kind.word()), s)),
                Item::Fold(d) => d.answer.as_ref().and_then(|a| named(&a.name)).map(|s| (what("畳み込みの答え", "a fold's answer"), s)),
                Item::Table(t) => t.outputs.iter().find_map(|o| named(&o.name)).map(|s| (if t.clause { what("節の出力", "a clause's output") } else { what("表の出力", "a table's output") }, s)),
            })
        })
}

/// The names a rule declares: inputs, outputs, element fields and sequences, what its items bind,
/// its tables and clauses, its open values.
fn declared_names(f: &RuleFile) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    out.extend(f.inputs.iter().map(|i| i.name.text.clone()));
    out.extend(f.outputs.iter().map(|o| o.name.text.clone()));
    for el in &f.elements {
        out.push(el.name.text.clone());
        out.extend(el.fields.iter().map(|x| x.name.text.clone()));
    }
    for it in &f.items {
        out.extend(crate::types::item_binds(f, it));
        if let Item::Table(t) = it {
            if let Some(n) = &t.name {
                out.push(n.text.clone());
            }
        }
    }
    out.extend(f.opens.iter().map(|o| o.name.text.clone()));
    out
}

/// What a name written in a contribution names in the merged rule: one of its own names, prefixed;
/// one of a contribution it adds to, prefixed with that one's name; anything else as written.
struct Names {
    /// (prefix, its local names), the contribution's own first, then the contributions it adds to,
    /// nearest first.
    scopes: Vec<(String, Vec<String>)>,
    alias: String,
}

impl Names {
    fn get(&self, w: &str) -> String {
        for (p, ls) in &self.scopes {
            if ls.iter().any(|l| l == w) {
                return format!("{p}:{w}");
            }
        }
        w.to_string()
    }

    fn is_own(&self, w: &str) -> bool {
        self.scopes.first().is_some_and(|(_, ls)| ls.iter().any(|l| l == w))
    }

    fn name(&self, n: &Name) -> Name {
        let text = self.get(&n.text);
        if text == n.text {
            return n.clone();
        }
        Name { text, ascii: Some(format!("{}_{}", self.alias, n.ascii.clone().unwrap_or_else(|| n.text.clone()))), span: n.span.clone() }
    }

    fn expr(&self, e: &Expr) -> Expr {
        match e {
            Expr::Name(w, sp) => Expr::Name(self.get(w), sp.clone()),
            Expr::Lit(..) => e.clone(),
            Expr::Bin(a, op, b, sp) => Expr::Bin(Box::new(self.expr(a)), *op, Box::new(self.expr(b)), sp.clone()),
            Expr::Call(f, args, sp) => Expr::Call(f.clone(), args.iter().map(|x| self.expr(x)).collect(), sp.clone()),
        }
    }

    fn item(&self, it: &Item) -> Item {
        match it {
            Item::Derived(d) => Item::Derived(DerivedDecl { name: self.name(&d.name), expr: self.expr(&d.expr), ..d.clone() }),
            Item::Define(d) => Item::Define(DefineDecl { name: self.name(&d.name), expr: self.expr(&d.expr), ..d.clone() }),
            Item::Agg(d) => Item::Agg(AggDecl {
                name: self.name(&d.name),
                column: Name { text: self.get(&d.column.text), ..d.column.clone() },
                filter: d.filter.as_ref().map(|(c, v)| (Name { text: self.get(&c.text), ..c.clone() }, v.clone())),
                ..d.clone()
            }),
            Item::Fold(d) => Item::Fold(FoldDecl {
                verdict: self.get(&d.verdict),
                answer: d.answer.as_ref().map(|a| FoldAnswer { name: self.name(&a.name), ty: a.ty.clone() }),
                arms: d
                    .arms
                    .iter()
                    .map(|(v, arm, sp)| {
                        let arm = match arm {
                            Arm::Stop(Some(x)) => Arm::Stop(Some(self.expr(x))),
                            Arm::Take { expr, unique } => Arm::Take { expr: self.expr(expr), unique: *unique },
                            Arm::KeepMax { expr, key } => Arm::KeepMax { expr: self.expr(expr), key: self.expr(key) },
                            other => other.clone(),
                        };
                        (v.clone(), arm, sp.clone())
                    })
                    .collect(),
                empty: d.empty.as_ref().map(|x| self.expr(x)),
                exhausted: d.exhausted.as_ref().map(|x| self.expr(x)),
                ..d.clone()
            }),
            Item::Table(t) => Item::Table(Table {
                name: t.name.as_ref().map(|n| self.name(n)),
                inputs: t.inputs.iter().map(|(n, sp)| (self.get(n), sp.clone())).collect(),
                outputs: t.outputs.iter().map(|o| OutCol { name: self.name(&o.name), ..o.clone() }).collect(),
                rows: t
                    .rows
                    .iter()
                    .map(|r| Row {
                        outs: r
                            .outs
                            .iter()
                            .map(|o| match o {
                                OutCell::Name(w) => OutCell::Name(self.get(w)),
                                other => other.clone(),
                            })
                            .collect(),
                        ..r.clone()
                    })
                    .collect(),
                overrides: t
                    .overrides
                    .iter()
                    .map(|o| {
                        // `表:行` of its own table, or a table or clause named as written.
                        let table = if o.table.contains(':') { o.table.clone() } else { self.get(&o.table) };
                        OverrideRef { table, ..o.clone() }
                    })
                    .collect(),
                ..t.clone()
            }),
        }
    }
}

/// Puts `item` into `items` at `i`, moving the `at` of every open value at or after it.
fn insert(f: &mut RuleFile, i: usize, item: Item) {
    f.items.insert(i, item);
    for o in f.opens.iter_mut() {
        if o.at >= i {
            o.at += 1;
        }
    }
}

/// Where the last item that binds one of `names` is, when one does.
fn last_binding(f: &RuleFile, names: &[String]) -> Option<usize> {
    f.items.iter().enumerate().filter(|(_, it)| crate::types::item_binds(f, it).iter().any(|n| names.contains(n))).map(|(i, _)| i).last()
}

/// The way an open value's line is written, for E068 and E131.
pub(crate) fn open_text(o: &OpenDecl) -> String {
    let mut s = format!("{} {} : {}", crate::kw::OPEN, o.name.text, type_text(&o.ty));
    if let Some((c, _)) = &o.combine {
        s.push_str(&format!(" {} {}", crate::kw::COMBINE, c.word()));
    }
    if let Some((l, _)) = &o.empty {
        s.push_str(&format!(" {} {}", crate::kw::EMPTY, l.src()));
    }
    if let Some(r) = &o.range {
        s.push_str(&format!(" {} {}", crate::kw::RANGE, range_text(r)));
    }
    s
}

/// A type as it is written: `money[USD, incl_tax]`, `rate[step 1%]`, `tier?`.
pub fn type_text(t: &TypeRef) -> String {
    let mut s = t.base.clone();
    if !t.args.is_empty() {
        let args: Vec<String> = t
            .args
            .iter()
            .map(|a| match a {
                TypeArg::Word(w) => w.clone(),
                TypeArg::Scaled(w, n) => format!("{w} {}", n.raw),
            })
            .collect();
        s.push_str(&format!("[{}]", args.join(", ")));
    }
    if t.optional {
        s.push('?');
    }
    s
}

/// A range as it is written: `>=0USD <=500USD`.
pub fn range_text(r: &Range) -> String {
    match &r.days {
        Some(d) => format!("from koyomi \"{}\" date {}", d.file, d.date),
        None => r.bounds.iter().map(|(op, l)| format!("{}{}", op.word(), l.src())).collect::<Vec<_>>().join(" "),
    }
}

/// A literal of the type with the value `digits` (`0`, `1`, `100%`): the start of `sum` and
/// `product` when the line writes no `empty`.
fn literal(ty: &TypeRef, digits: &str, rate_digits: &str, sp: &Span) -> Expr {
    let unit = match ty.base.as_str() {
        crate::kw::RATE => Some("%".to_string()),
        crate::kw::NUMBER => None,
        _ => ty.args.iter().find_map(|a| if let TypeArg::Word(w) = a { Some(w.clone()) } else { None }),
    };
    let d = if ty.base == crate::kw::RATE { rate_digits } else { digits };
    let raw = format!("{d}{}", unit.clone().unwrap_or_default());
    Expr::Lit(Lit::Num(crate::lex::Num { neg: false, digits: d.to_string(), frac: String::new(), mult: 1, unit, raw }), sp.clone())
}

/// The combination of an open value's terms (§15.227): the `empty` value, or the start of `sum` (0)
/// and `product` (1), then every term, put together the way `combine` says. `max` and `min` take
/// the `empty` value as one of the values, so a term at or below it (at or above it, for `min`) is
/// the one that changes nothing.
pub fn combination(o: &OpenDecl) -> Expr {
    let sp = o.name.span.clone();
    let how = o.combine.as_ref().map(|(c, _)| *c).unwrap_or(Combine::Sum);
    let start: Option<Expr> = match (&o.empty, how) {
        (Some((l, lsp)), _) => Some(Expr::Lit(l.clone(), lsp.clone())),
        (None, Combine::Sum) if o.terms.is_empty() => Some(literal(&o.ty, "0", "0", &sp)),
        (None, Combine::Product) => Some(literal(&o.ty, "1", "100", &sp)),
        _ => None,
    };
    let mut acc = start;
    for t in &o.terms {
        let x = Expr::Name(t.name.text.clone(), t.name.span.clone());
        acc = Some(match acc {
            None => x,
            Some(a) => match how {
                Combine::Sum => Expr::Bin(Box::new(a), BinOp::Add, Box::new(x), sp.clone()),
                Combine::Product => Expr::Bin(Box::new(a), BinOp::Mul, Box::new(x), sp.clone()),
                Combine::Max => Expr::Call(crate::kw::MAX.to_string(), vec![a, x], sp.clone()),
                Combine::Min => Expr::Call(crate::kw::MIN.to_string(), vec![a, x], sp.clone()),
            },
        });
    }
    acc.unwrap_or_else(|| literal(&o.ty, "0", "0", &sp))
}

/// Puts a `define` for every open value of `f` where it is written, with a placeholder value the
/// merge fills in once every term is in (`fill`). The ones written at the same place go in the
/// order written.
fn place_opens(f: &mut RuleFile) {
    let mut idx: Vec<usize> = (0..f.opens.len()).collect();
    idx.sort_by(|a, b| (f.opens[*b].at, *b).cmp(&(f.opens[*a].at, *a)));
    for k in idx {
        let o = f.opens[k].clone();
        let at = o.at.min(f.items.len());
        f.items.insert(at, Item::Define(DefineDecl { name: o.name.clone(), ty: o.ty.clone(), expr: combination(&o), span: o.span.clone(), cite: None }));
        for (j, p) in f.opens.iter_mut().enumerate() {
            if j != k && p.at >= at && (p.at > at || j > k) {
                p.at += 1;
            }
        }
    }
}

/// The `define` of every open value takes the combination of all its terms.
fn fill(f: &mut RuleFile) {
    for o in f.opens.clone() {
        let e = combination(&o);
        for it in f.items.iter_mut() {
            if let Item::Define(d) = it {
                if d.name.text == o.name.text {
                    d.expr = e.clone();
                    d.ty = o.ty.clone();
                }
            }
        }
    }
}

/// The checks of an open value's line that need no types (§15.227, E068): a way of combining, a
/// range, an `empty` for `max` and `min`, a type the way of combining can put together.
fn open_shape(f: &RuleFile, path_of: &dyn Fn(&OpenDecl) -> String) -> Vec<Diag> {
    let mut out = Vec::new();
    for o in &f.opens {
        let at = format!("{}:{}", path_of(o), o.span.line);
        let note_shape = open_shape_note();
        let mut bad = |title: String, mark: String, fix: Option<String>| {
            let mut d = Diag::error("E068", title).at(at.clone()).mark(o.span.clone(), mark).note(note_shape.clone());
            if let Some(t) = fix {
                d = d.note(t);
            }
            out.push(d);
        };
        let Some((how, _)) = &o.combine else {
            bad(
                tr!("開いた値 {} に合わせ方がありません", "The open value {} says no way of combining", o.name.text),
                tr!("`combine sum|product|max|min` がありません", "`combine sum|product|max|min` is missing"),
                Some(tr!(
                    "寄与の項を足すなら `combine sum`、掛けるなら `combine product`、いちばん大きいものなら `combine max`、小さいものなら `combine min` です。",
                    "To add the terms up, `combine sum`; to multiply them, `combine product`; to take the largest, `combine max`; the smallest, `combine min`."
                )),
            );
            continue;
        };
        if o.range.is_none() {
            bad(
                tr!("開いた値 {} に範囲がありません", "The open value {} declares no range", o.name.text),
                tr!("`range >=… <=…` がありません", "`range >=… <=…` is missing"),
                Some(tr!(
                    "範囲は、寄与を全部合わせても値がその外に出ないという約束で、下の表はこの範囲の全体で検査されます。だから元の規則の検査は、どんな寄与が来ても成り立ちます。",
                    "The range is the promise that the value stays inside it whatever is added, and the tables below are checked over the whole of it — which is why the rule's own check holds whatever the contributions are."
                )),
            );
        }
        if matches!(how, Combine::Max | Combine::Min) && o.empty.is_none() {
            bad(
                tr!("`combine {}` の開いた値 {} に `empty` がありません", "The open value {1} with `combine {0}` has no `empty`", how.word(), o.name.text),
                tr!("`empty <値>` がありません", "`empty <value>` is missing"),
                Some(tr!(
                    "`empty` の値は、項が何も無いときの答えで、項の中でいちばん小さい（`min` なら大きい）値として合わさります。どの値にするかは業務の判断です。",
                    "The `empty` value is the answer when there is no term, and it takes part as one of the values: the smallest (the largest, for `min`) the answer can be. Which value it is is a business decision."
                )),
            );
        }
        let numeric = matches!(
            o.ty.base.as_str(),
            crate::kw::NUMBER | crate::kw::MONEY | crate::kw::MASS | crate::kw::LENGTH | crate::kw::AREA | crate::kw::VOLUME | crate::kw::DURATION | crate::kw::RATE
        );
        let ok = match how {
            Combine::Sum => numeric,
            Combine::Product => matches!(o.ty.base.as_str(), crate::kw::NUMBER | crate::kw::RATE),
            Combine::Max | Combine::Min => numeric || matches!(o.ty.base.as_str(), crate::kw::TEMPERATURE | crate::kw::SOUND),
        };
        if !ok && !o.ty.optional {
            let why = match how {
                Combine::Product => tr!(
                    "掛け合わせられるのは `number` と `rate` です。お金や量どうしを掛けた値は、同じ型になりません。",
                    "Only a `number` or a `rate` multiplies: money or a quantity times another is not of the same type."
                ),
                _ => tr!(
                    "合わせられるのは、数（`number`）、お金、量、率です（`max` と `min` は温度と音量も）。",
                    "What combines is a `number`, money, a quantity or a rate (and, for `max` and `min`, a temperature or a sound level)."
                ),
            };
            bad(tr!("開いた値 {} の型 {} は `combine {}` で合わせられません", "The open value {} of type {} cannot be put together with `combine {}`", o.name.text, type_text(&o.ty), how.word()), String::new(), Some(why));
        } else if o.ty.optional {
            bad(tr!("開いた値 {} はオプショナルにできません", "The open value {} cannot be optional", o.name.text), String::new(), None);
        }
    }
    out
}

/// Merges the contributions `cs` into `f`, the file at `path` they come down to (§15.227–§15.229).
/// `main` is the contribution whose check this is, if one is: its spans are the file's own.
fn merge_into(f: &mut RuleFile, path: &str, cs: Vec<Contrib>, main: Option<usize>) -> Vec<Diag> {
    let mut out: Vec<Diag> = Vec::new();
    let main_shown = main.and_then(|i| cs.get(i)).map(|c| c.shown.clone());
    out.extend(open_shape(f, &|_| path.to_string()));
    // The rule's own open values get their `define`s first, so that what is merged is placed
    // around them.
    place_opens(f);
    let (cs, ring) = order(cs);
    out.extend(ring);
    let root_names = declared_names(f);
    // Each contribution's own names, by its file, so that one that adds to another reads that
    // one's names.
    let mut local_of: Vec<(String, String, Vec<String>)> = Vec::new();
    for c in &cs {
        let outer = |n: &str| root_names.iter().any(|r| r == n);
        local_of.push((c.shown.clone(), c.file.name.text.clone(), locals(&c.file, &outer)));
    }
    let mut merged_names: Vec<String> = Vec::new();
    for c in &cs {
        let is_main = main_shown.as_deref() == Some(c.shown.as_str());
        let shown = c.shown.clone();
        let where_ = |sp: &Span| format!("{shown}:{}", sp.line);
        let h = c.file.contrib.clone().unwrap_or_else(|| ContribHead { to: None, afters: Vec::new(), extends: Vec::new(), intos: Vec::new(), prereq_lines: Vec::new() });
        let cname = c.file.name.text.clone();
        if merged_names.contains(&cname) {
            out.push(
                Diag::error("E067", tr!("寄与の名前 {cname} が二度あります", "Two contributions are called {cname}"))
                    .at(where_(&c.file.name.span))
                    .mark(c.file.name.span.clone(), "")
                    .note(tr!(
                        "寄与の名前は、合わせた規則の中で、その寄与の表や定義の名前の前に付きます（`{cname}:<名前>`）。二つあると、どちらのものか決まりません。どちらかの名前を変えてください。",
                        "A contribution's name comes before the names of its tables and definitions in the merged rule (`{cname}:<name>`); with two of one name, which is whose cannot be told. Rename one."
                    )),
            );
            continue;
        }
        merged_names.push(cname.clone());
        // The chain of contributions it adds to, nearest first, and the names they declare.
        let mut scopes: Vec<(String, Vec<String>)> = Vec::new();
        if let Some((_, n, ls)) = local_of.iter().find(|(s, _, _)| *s == c.shown) {
            scopes.push((n.clone(), ls.clone()));
        }
        let mut t = c.to.clone();
        let mut hops = 0;
        while let Some((_, n, ls)) = local_of.iter().find(|(s, _, _)| same_file(s, &t)) {
            scopes.push((n.clone(), ls.clone()));
            let next = cs.iter().find(|o| same_file(&o.shown, &t)).map(|o| o.to.clone());
            match next {
                Some(nx) if hops < 32 => {
                    t = nx;
                    hops += 1;
                }
                _ => break,
            }
        }
        let alias = c.file.name.ascii.clone().unwrap_or_else(|| cname.clone());
        let names = Names { scopes, alias };
        // The pins: the shape of the file it adds to and of the ones it comes after (§15.230).
        out.extend(pin::check_pins(c, &cs, path));
        // Its inputs: the rule's when the name is the same, a new input otherwise (in the order of
        // the files, after the rule's own).
        let mut added_inputs: Vec<String> = Vec::new();
        for i in &c.file.inputs {
            // What the caller draws (§15.224) is the rule's to say, with the claims of chances that
            // go with it: a contribution reads the drawn inputs the rule has, and draws none.
            if let Some(sp) = &i.random {
                out.push(
                    Diag::error("E067", tr!("寄与の入力 {} は乱数にできません", "The contribution's input {} cannot be drawn at random", i.name.text))
                        .at(where_(&i.span))
                        .mark(sp.clone(), "")
                        .note(tr!(
                            "乱数の入力は、呼ぶ側が引いて渡す値で、確率の主張と一緒に、足す先の規則が決めます。寄与は、規則が引く入力を読めますが、引く入力を足せません。足す先の規則の乱数の入力を読むか、ふつうの入力にしてください。",
                            "A random input is a value every caller draws and passes, and the rule a contribution adds to says which, with its claims of chances. A contribution reads the drawn inputs the rule has, and adds none: read one of those, or make this an ordinary input."
                        )),
                );
                continue;
            }
            match f.inputs.iter().find(|x| x.name.text == i.name.text) {
                Some(x) => {
                    let same_ty = type_text(&x.ty) == type_text(&i.ty);
                    let same_range = x.range.as_ref().map(range_text) == i.range.as_ref().map(range_text);
                    if !same_ty || !same_range {
                        out.push(
                            Diag::error("E042", tr!("寄与の入力 {} の型か範囲が、元の入力と違います", "The contribution's input {} differs from the rule's in type or range", i.name.text))
                                .at(where_(&i.span))
                                .mark(i.name.span.clone(), tr!("{} {}", "{} {}", type_text(&i.ty), i.range.as_ref().map(range_text).unwrap_or_default()))
                                .note(tr!(
                                    "同じ名前の入力は同じ入力で、元の規則では `{} {}` です。呼ぶ側が渡す値は一つなので、型と範囲を元の規則にそろえるか、別の名前の入力にしてください。",
                                    "An input of the same name is the same input, and the rule declares it `{} {}`. The caller passes one value, so make the type and the range the rule's, or give the input another name.",
                                    type_text(&x.ty),
                                    x.range.as_ref().map(range_text).unwrap_or_default()
                                )),
                        );
                    }
                }
                None => {
                    // A name the rule declares as another value: the rule's tables and expressions
                    // would read one of the two, and the caller would pass the other (§15.262).
                    if let Some((what, sp)) = value_named(f, &i.name.text) {
                        out.push(
                            Diag::error("E034", tr!("寄与の入力 `{}` は、足す先の規則の{what}と同じ名前です", "The contribution's input `{}` has the name of {what} of the rule it adds to", i.name.text))
                                .at(where_(&i.span))
                                .mark(i.name.span.clone(), "")
                                .note(tr!(
                                    "足す先の規則は、{} 行目でこの名前を宣言しています。一つの名前は、合わせた規則の中で一つの値を指すので、入力の名前を変えてください。",
                                    "The rule declares the name at line {}. A name names one value in the merged rule: rename the input.",
                                    sp.line
                                )),
                        );
                        continue;
                    }
                    // The days of a date it adds come from a koyomi file named from its own
                    // directory, whichever file the merged rule is checked as (§15.235).
                    let mut i = i.clone();
                    if let Some(d) = i.range.as_mut().and_then(|r| r.days.as_mut()) {
                        d.base.get_or_insert_with(|| shown.clone());
                    }
                    added_inputs.push(i.name.text.clone());
                    f.inputs.push(i);
                }
            }
        }
        // The rule's claims of chances (§15.224) speak of every combination of the inputs that are
        // not drawn: an input a contribution adds is one more of them, and a claim of the rule holds
        // whatever value it takes, as a `-` column says.
        for ch in f.chances.iter_mut() {
            for n in &added_inputs {
                if ch.inputs.iter().any(|(x, _)| x == n) {
                    continue;
                }
                ch.inputs.push((n.clone(), ch.span.clone()));
                for row in ch.rows.iter_mut() {
                    row.cells.push(Cell::DontCare);
                    row.cell_spans.push(row.span.clone());
                }
            }
        }
        // The values it adds to open enums (§15.228).
        for e in &h.extends {
            out.extend(extend(f, path, e, &cname, &shown));
        }
        // Its open values: they come in as its own names, each a `define` placed where its terms
        // are known; another contribution may add to them.
        let first_new_open = f.opens.len();
        for o in &c.file.opens {
            let mut o = o.clone();
            o.name = names.name(&o.name);
            for t in o.terms.iter_mut() {
                t.name = Name { text: names.get(&t.name.text), ..t.name.clone() };
            }
            f.opens.push(o);
        }
        out.extend(open_shape(&RuleFile { opens: f.opens[first_new_open..].to_vec(), ..empty_like(f) }, &|_| shown.clone()));
        // Its items, each placed after what it reads; a table or clause that defines an open
        // output, right after the last definition of that output, where the set is decided.
        for it in &c.file.items {
            let item = names.item(it);
            let reads = crate::types::item_reads(&item);
            let after = last_binding(f, &reads);
            let mut at = after.map(|i| i + 1).unwrap_or(0);
            if let Item::Table(t) = &item {
                let mut joins: Vec<String> = Vec::new();
                for oc in &t.outputs {
                    let own = names.is_own(&oc.name.text) || oc.name.text.starts_with(&format!("{cname}:"));
                    if own {
                        continue;
                    }
                    joins.push(oc.name.text.clone());
                }
                if !joins.is_empty() {
                    let what = if t.clause { tr!("節", "clause") } else { tr!("表", "table") };
                    let tname = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
                    let mut ok = true;
                    for o in &joins {
                        let members: Vec<&Table> = f.items.iter().filter_map(|i| if let Item::Table(u) = i { Some(u) } else { None }).filter(|u| u.outputs.iter().any(|x| x.name.text == *o)).collect();
                        let open = members.iter().any(|u| u.open.is_some());
                        if !open {
                            let why = if members.is_empty() {
                                if f.inputs.iter().any(|i| i.name.text == *o) {
                                    tr!("{o} は入力です。入力の値は呼ぶ側が渡します", "{o} is an input, which the caller passes")
                                } else if f.opens.iter().any(|x| x.name.text == *o) {
                                    tr!("{o} は開いた値です。項を足すには `into {o}` と書いてください", "{o} is an open value: add a term to it with `into {o}`")
                                } else {
                                    tr!("足す先の規則に、{o} を決める表も節もありません", "no table or clause of the rule it adds to decides {o}")
                                }
                            } else {
                                tr!("{o} を決める表と節のどれにも `open` が付いていません", "none of the tables and clauses that decide {o} is marked `open`")
                            };
                            let sp = t.name.as_ref().map(|n| n.span.clone()).unwrap_or_else(|| t.span.clone());
                            out.push(
                                Diag::error("E067", tr!("{what} {tname} は、開いていない出力 {o} を決めます", "The {what} {tname} decides {o}, which is not open"))
                                    .at(where_(&sp))
                                    .mark(sp, why)
                                    .note(tr!(
                                        "寄与が表や節を足せるのは、元の規則が `table <名前> open` か `clause <名前> -> <出力> open` で開いた出力だけです。どこに足せるかは元の規則の作り手が決めます。",
                                        "A contribution adds tables and clauses only to an output the rule opens with `table <name> open` or `clause <name> -> <output> open`; where a rule may be added to is its author's to decide."
                                    )),
                            );
                            ok = false;
                            continue;
                        }
                        // Right after the last definition of the output: the set is decided there.
                        if let Some(last) = last_binding(f, std::slice::from_ref(o)) {
                            if let Some(a) = after.filter(|a| *a > last) {
                                let late: Vec<String> = reads.iter().filter(|r| f.items.get(a).is_some_and(|i| crate::types::item_binds(f, i).contains(r))).cloned().collect();
                                let sp = t.name.as_ref().map(|n| n.span.clone()).unwrap_or_else(|| t.span.clone());
                                out.push(
                                    Diag::error("E067", tr!("{what} {tname} は、{o} を決める最後の定義より下の名前を読んでいます", "The {what} {tname} reads a name below the last definition of {o}"))
                                        .at(where_(&sp))
                                        .mark(sp, tr!("読んでいる名前: {}", "it reads: {}", late.join(", ")))
                                        .note(tr!(
                                            "一つの出力を決める定義の集まりは、最後の定義の位置で決まります。寄与の定義はその位置に入るので、読めるのは、そこより上で決まる名前だけです。",
                                            "The definitions of one output are decided together where the last of them is. A contribution's definition goes there, so it reads only what is decided above it."
                                        )),
                                );
                                ok = false;
                            }
                            at = at.max(last + 1);
                        }
                    }
                    if !ok {
                        continue;
                    }
                }
            }
            let item = match item {
                Item::Table(mut t) => {
                    t.contributed = Some((cname.clone(), shown.clone()));
                    Item::Table(t)
                }
                other => other,
            };
            insert(f, at.min(f.items.len()), item);
        }
        // Its open values' `define`s, after what their terms read.
        for k in first_new_open..f.opens.len() {
            let o = f.opens[k].clone();
            let reads: Vec<String> = o.terms.iter().map(|t| t.name.text.clone()).collect();
            let at = last_binding(f, &reads).map(|i| i + 1).unwrap_or(0).min(f.items.len());
            f.items.insert(at, Item::Define(DefineDecl { name: o.name.clone(), ty: o.ty.clone(), expr: combination(&o), span: o.span.clone(), cite: None }));
            for (j, p) in f.opens.iter_mut().enumerate() {
                if j != k && p.at >= at {
                    p.at += 1;
                }
            }
            f.opens[k].at = at;
        }
        // The terms it adds to open values (§15.227).
        for d in &h.intos {
            let target = names_for_target(&names, &d.target.text);
            let Some(k) = f.opens.iter().position(|o| o.name.text == target) else {
                let why = if root_names.iter().any(|n| *n == d.target.text) || declared_names(f).contains(&target) {
                    tr!("{} は開いた値ではありません", "{} is not an open value", d.target.text)
                } else {
                    tr!("{} という値は足す先にありません", "the file it adds to has no value called {}", d.target.text)
                };
                out.push(
                    Diag::error("E067", tr!("`into {}` の先が、寄与を受ける値ではありません", "`into {}` names no value that takes contributions", d.target.text))
                        .at(where_(&d.span))
                        .mark(d.target.span.clone(), why)
                        .note(tr!(
                            "`into` が指せるのは、足す先の規則が `open <名前> : <型> combine …` で開いた値だけです。",
                            "`into` names a value the rule it adds to opens with `open <name> : <type> combine …`."
                        )),
                );
                continue;
            };
            let at_open = f.items.iter().position(|i| matches!(i, Item::Define(x) if x.name.text == target));
            for term in &d.terms {
                let tn = names.get(&term.text);
                let bound = last_binding(f, std::slice::from_ref(&tn));
                let input = f.inputs.iter().any(|i| i.name.text == tn);
                if bound.is_none() && !input {
                    out.push(
                        Diag::error("E067", tr!("項 {} が宣言されていません", "The term {} is not declared", term.text))
                            .at(where_(&term.span))
                            .mark(term.span.clone(), "")
                            .note(tr!(
                                "項に書けるのは、この寄与が決める名前か、足す先の規則で開いた値より上にある名前です。",
                                "A term is a name this contribution decides, or a name of the rule it adds to declared above the open value."
                            )),
                    );
                    continue;
                }
                if let (Some(b), Some(a)) = (bound, at_open) {
                    if b >= a {
                        out.push(
                            Diag::error("E067", tr!("項 {} は、開いた値 {} より下で決まります", "The term {} is decided below the open value {}", term.text, d.target.text))
                                .at(where_(&term.span))
                                .mark(term.span.clone(), "")
                                .note(tr!(
                                    "開いた値は、書いた位置で、その上で決まった項を合わせます。項を決める表や定義が、開いた値より下で決まる名前を読んでいないかを確かめてください。",
                                    "An open value puts together, where it is written, the terms decided above it. See whether the table or definition that decides the term reads a name decided below the open value."
                                )),
                        );
                        continue;
                    }
                }
                f.opens[k].terms.push(Term { name: Name { text: tn, ..term.clone() }, from: Some((cname.clone(), shown.clone())) });
            }
        }
        // The lines it adds to open prerequisites (§15.229).
        for (block, lines) in &h.prereq_lines {
            out.extend(prereq_lines(f, block, lines, &cname, &shown));
        }
        // Each of its items as close as it can be to what reads it: right before the first item
        // that does, so that the merged rule reads the contribution's table where its value is
        // used rather than at the top. A definition of an open output stays where the set is
        // decided.
        fill(f);
        sink(f, &cname);
        f.contributions.push(Merged {
            name: c.file.name.clone(),
            version: c.file.version.clone(),
            description: c.file.description.clone(),
            file: shown.clone(),
            sha256: crate::sha256::hex(c.src.as_bytes()),
            signature: h.to.as_ref().and_then(|t| t.signature.clone()),
            to: c.to.clone(),
            inputs: added_inputs,
            src: c.src.clone(),
            via: None,
        });
        let _ = is_main;
    }
    fill(f);
    out
}

/// The name an `into` line names, from the contribution's scope: one of its own open values, one of
/// a contribution it adds to, or one of the rule's.
fn names_for_target(names: &Names, w: &str) -> String {
    names.get(w)
}

/// A file with nothing in it but the kind and the name: what `open_shape` is asked of for one
/// contribution's open values.
fn empty_like(f: &RuleFile) -> RuleFile {
    RuleFile {
        kind: f.kind,
        name: f.name.clone(),
        version: f.version.clone(),
        description: None,
        imports: Vec::new(),
        enum_imports: Vec::new(),
        enum_froms: Vec::new(),
        sources: Vec::new(),
        applies: Vec::new(),
        enums: Vec::new(),
        groups: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        items: Vec::new(),
        result: None,
        examples: Vec::new(),
        constraints: Vec::new(),
        shapes: Vec::new(),
        implements: None,
        elements: Vec::new(),
        sequences: Vec::new(),
        machine: None,
        scenarios: Vec::new(),
        prerequisites: Vec::new(),
        chances: Vec::new(),
        opens: Vec::new(),
        contrib: None,
        contributions: Vec::new(),
    }
}

/// One `extend` line (§15.228): the value goes into the open enum of the file it adds to, after
/// the value `after` names when the enum is ordered, at the end when it is not.
fn extend(f: &mut RuleFile, path: &str, e: &ExtendDecl, cname: &str, shown: &str) -> Vec<Diag> {
    let at = format!("{shown}:{}", e.span.line);
    let Some(k) = f.enums.iter().position(|d| d.name.text == e.enum_name.text) else {
        return vec![
            Diag::error("E067", tr!("足す先に列挙 {} がありません", "The file it adds to has no enum {}", e.enum_name.text))
                .at(at)
                .mark(e.enum_name.span.clone(), "")
                .note(tr!("足す先の列挙: {}", "Its enums: {}", f.enums.iter().map(|d| d.name.text.clone()).collect::<Vec<_>>().join(", "))),
        ];
    };
    let d = &f.enums[k];
    if let Some(o) = &d.from {
        return vec![
            Diag::error("E067", tr!("列挙 {} は {} のものです", "The enum {} belongs to {}", e.enum_name.text, o.file))
                .at(at)
                .mark(e.enum_name.span.clone(), "")
                .note(tr!(
                    "列挙の持ち主は一つです。値は、持ち主のファイル（{}）に足す寄与で足してください。その列挙を読む規則には全部、その値が入ります。",
                    "An enum has one owner. Add the value with a contribution to its file ({}): every rule that reads the enum gets it.",
                    o.file
                )),
        ];
    }
    if d.open.is_none() {
        return vec![
            Diag::error("E067", tr!("列挙 {} は開いていません", "The enum {} is not open", e.enum_name.text))
                .at(at)
                .mark(e.enum_name.span.clone(), tr!("{path} の列挙に `open` が付いていません", "the enum in {path} is not marked `open`"))
                .note(tr!(
                    "寄与が値を足せるのは、列挙の行の最後に `open` を付けた列挙だけです。値を足してよいかは、列挙の持ち主が決めます。",
                    "A contribution adds values only to an enum whose line ends with `open`; whether values may be added is the enum's owner's to decide."
                )),
        ];
    }
    if d.values.iter().any(|v| v.text == e.value.text) {
        return vec![
            Diag::error("E067", tr!("列挙 {} には値 {} があります", "The enum {} has a value {} already", e.enum_name.text, e.value.text))
                .at(at)
                .mark(e.value.span.clone(), "")
                .note(tr!("足すのは、まだ無い値です。綴りを確かめてください。", "What is added is a value the enum does not have; check the spelling.")),
        ];
    }
    // A value every language writes as one of the enum's (§15.263): `Web` beside `web` is `Web` in
    // Rust and Go and `WEB` where the members are in upper case.
    let id = |n: &Name| n.ascii.clone().unwrap_or_else(|| n.text.clone());
    let member = crate::codegen::spellings::member(&id(&e.value));
    if let Some(v) = d.values.iter().find(|v| crate::codegen::spellings::member(&id(v)) == member) {
        let (added, enum_name, value) = (&e.value.text, &e.enum_name.text, &v.text);
        return vec![
            Diag::error(
                "E034",
                tr!(
                    "寄与が足す値 `{added}` は、列挙 {enum_name} の値 `{value}` と、生成するコードで同じ名前になります",
                    "The value `{added}` the contribution adds takes the name of the value `{value}` of the enum {enum_name} in the generated code"
                ),
            )
            .at(at)
            .mark(e.value.span.clone(), "")
            .note(tr!(
                "列挙の値は、識別子（別名、無ければ名前）を PascalCase にした名前（どちらも `{member}`）で書き、言語によってはそれを大文字か lowerCamelCase にします。合わせた規則の列挙に同じ名前の値が二つになるので、足す値に別の名前か別名を付けてください。",
                "A value of an enum is written by its identifier (the alias, or the name where there is none) in PascalCase (both are `{member}`), which some languages then put in upper case or in lowerCamelCase, so the merged rule's enum would declare one value twice. Give the value it adds another name or an alias."
            )),
        ];
    }
    let pos = match (&e.after, d.ordered) {
        (Some(a), true) => match d.values.iter().position(|v| v.text == a.text) {
            Some(p) => p + 1,
            None => {
                return vec![
                    Diag::error("E067", tr!("`after {}` の値が列挙 {} にありません", "`after {}` names no value of the enum {}", a.text, e.enum_name.text))
                        .at(at)
                        .mark(a.span.clone(), "")
                        .note(tr!("値: {}", "Its values: {}", d.values.iter().map(|v| v.text.clone()).collect::<Vec<_>>().join(" < "))),
                ];
            }
        },
        (None, true) => {
            return vec![
                Diag::error("E067", tr!("順序のある列挙 {} に足す値に、位置がありません", "The value added to the ordered enum {} has no place", e.enum_name.text))
                    .at(at)
                    .mark(e.value.span.clone(), tr!("`after <値>` がありません", "`after <value>` is missing"))
                    .note(tr!(
                        "順序のある列挙では、値の位置が比べるセルの意味を決めます。`extend {} with {} after <値>` と書いて、どの値の次に来るかを言ってください。",
                        "In an ordered enum, a value's place decides what the comparing cells take. Write `extend {} with {} after <value>` to say which value it follows.",
                        e.enum_name.text,
                        e.value.text
                    )),
            ];
        }
        (Some(a), false) => {
            return vec![
                Diag::error("E067", tr!("順序の無い列挙 {} に `after` は書けません", "The enum {} has no order, so `after` cannot be written", e.enum_name.text))
                    .at(at)
                    .mark(a.span.clone(), "")
                    .note(tr!("順序の無い列挙では、値の位置に意味がありません。`after` を外してください。", "An enum without an order gives a value's place no meaning; take `after` out.")),
            ];
        }
        (None, false) => d.values.len(),
    };
    let d = &mut f.enums[k];
    d.values.insert(pos, e.value.clone());
    d.default_marks.insert(pos.min(d.default_marks.len()), e.default);
    d.labels.insert(pos.min(d.labels.len()), e.label.clone());
    d.added.push(Added { value: e.value.text.clone(), by: cname.to_string(), file: shown.to_string(), span: e.span.clone() });
    Vec::new()
}

/// The `requires` lines a contribution adds to an open block of prerequisites (§15.229): only for
/// values a contribution added, never for the block's own.
fn prereq_lines(f: &mut RuleFile, block: &Name, lines: &[RequiresLine], cname: &str, shown: &str) -> Vec<Diag> {
    let at = format!("{shown}:{}", block.span.line);
    let Some(k) = f.prerequisites.iter().position(|p| p.name.text == block.text) else {
        return vec![
            Diag::error("E067", tr!("足す先に前提のつながりの塊 {} がありません", "The file it adds to has no prerequisites block {}", block.text))
                .at(at)
                .mark(block.span.clone(), "")
                .note(tr!("足す先の塊: {}", "Its blocks: {}", f.prerequisites.iter().map(|p| p.name.text.clone()).collect::<Vec<_>>().join(", "))),
        ];
    };
    if f.prerequisites[k].open.is_none() {
        return vec![
            Diag::error("E067", tr!("前提のつながりの塊 {} は開いていません", "The prerequisites block {} is not open", block.text))
                .at(at)
                .mark(block.span.clone(), "")
                .note(tr!(
                    "寄与が行を足せるのは、見出しの最後に `open` を付けた塊だけです（`prerequisites {} over <列挙> open`）。",
                    "A contribution adds lines only to a block whose heading ends with `open` (`prerequisites {} over <enum> open`).",
                    block.text
                )),
        ];
    }
    let over = f.prerequisites[k].over_name().to_string();
    let added: Vec<String> = f.enums.iter().filter(|e| e.name.text == over).flat_map(|e| e.added.iter().map(|a| a.value.clone())).collect();
    let mut out = Vec::new();
    for l in lines {
        if !added.contains(&l.value.text) {
            out.push(
                Diag::error("E067", tr!("値 {} の行は、元の塊のものです", "The line of {} is the block's own", l.value.text))
                    .at(format!("{shown}:{}", l.span.line))
                    .mark(l.value.span.clone(), "")
                    .note(tr!(
                        "寄与が足せるのは、寄与が `extend` で足した値の行だけです。元の値の前提は、元のファイルが決めます。",
                        "A contribution adds the lines of the values contributions add with `extend`, and no other: what a value of the block's own needs is its file's to decide."
                    )),
            );
            continue;
        }
        f.prerequisites[k].lines.push(l.clone());
    }
    let _ = cname;
    out
}

/// Every finding about a line of a file merged into the one being checked is written as being in
/// that file (§15.227): the `--> path:line` of a finding whose first mark is there names it.
pub fn relocate(diags: &mut [Diag], path: &str) {
    let head = format!("{path}:");
    for d in diags.iter_mut() {
        let Some(fp) = d.marks.first().and_then(|m| m.span.file_path()).map(str::to_string) else { continue };
        if let Some(rest) = d.where_.strip_prefix(&head) {
            d.where_ = format!("{fp}:{rest}");
        }
    }
}

/// The values a contribution added to the enums of a rule, each with the contribution and its file
/// (§15.228): what the notes of E101 and E033 name, and what the page says of a value.
pub fn added_values(f: &RuleFile) -> Vec<(String, String, Added)> {
    f.enums.iter().flat_map(|e| e.added.iter().map(move |a| (e.name.text.clone(), a.value.clone(), a.clone()))).collect()
}

/// What the merge proves once the rule is typed (§15.228): a value a contribution added to an enum
/// falls to no row of a table in silence. A row that names it decides it; a `default` on the
/// `extend` line says that falling to the rows that do not name it is what is meant. Anything else
/// — a `-`, a `not:` that leaves it out, a comparing cell of an ordered enum that now covers it —
/// takes it with nobody having decided, and that is E033, row by row, as a value a contract added
/// is. A table with no row that takes it at all is E101, which the region check says.
pub fn check(f: &RuleFile, c: &crate::types::Checked, path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    let added: Vec<(String, Added, bool)> = f
        .enums
        .iter()
        .flat_map(|e| {
            e.added.iter().map(move |a| {
                let i = e.values.iter().position(|v| v.text == a.value);
                let default = i.and_then(|i| e.default_marks.get(i).copied()).unwrap_or(false);
                (e.name.text.clone(), a.clone(), default)
            })
        })
        .collect();
    if added.is_empty() {
        return out;
    }
    let groups: Vec<&String> = c.groups.keys().collect();
    for set in &c.sets {
        let t = &set.table;
        for (ci, (col, _)) in t.inputs.iter().enumerate() {
            let Some(ty) = c.ty_of(col) else { continue };
            let crate::types::Ty::Enum(en) = ty.present().clone() else { continue };
            for (e, a, default) in &added {
                if *e != en || *default {
                    continue;
                }
                let v = a.value.as_str();
                let names = |l: &Lit| matches!(l, Lit::Word(w) if w == v);
                let named = t.rows.iter().any(|r| match r.cells.get(ci) {
                    Some(Cell::Lit(l)) => names(l),
                    Some(Cell::Set(ls)) => ls.iter().any(names),
                    _ => false,
                });
                if named {
                    continue;
                }
                let catching: Vec<(usize, &Row)> = t
                    .rows
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| match r.cells.get(ci) {
                        None | Some(Cell::DontCare) => true,
                        Some(Cell::Not(ls)) => !ls.iter().any(|l| names(l) || matches!(l, Lit::Word(w) if groups.contains(&w) && c.groups.get(w).is_some_and(|(_, ms)| ms.iter().any(|m| m == v)))),
                        Some(Cell::Ranked { values, .. }) => values.iter().any(|x| x == v),
                        _ => false,
                    })
                    .collect();
                if catching.is_empty() {
                    continue;
                }
                let tname = set.key.clone();
                let (ri, row) = catching[0];
                let sp = row.cell_spans.get(ci).cloned().unwrap_or_else(|| row.span.clone());
                let rows_said: Vec<String> = catching.iter().map(|(i, r)| format!("{} {}", set.row_table(*i), r.index)).collect();
                let ranked = matches!(row.cells.get(ci), Some(Cell::Ranked { .. }));
                let mut d = Diag::error("E033", tr!("足した値 {v} に、表 {tname} の行も `default` もありません", "The added value {v} has neither a row of table {tname} nor `default`"))
                    .at(format!("{path}:{} {}", sp.line, tr!("表 {tname}", "table {tname}")))
                    .table(tname.clone())
                    .row(row.index)
                    .mark(
                        sp,
                        if ranked {
                            tr!("この比べるセルが {v} も取るようになりました", "this comparing cell now takes {v} too")
                        } else {
                            tr!("{v} はこの行に黙って落ちます", "{v} falls to this row in silence")
                        },
                    )
                    .fix(crate::diag::FixKind::MarkDefault, crate::kw::DEFAULT)
                    .note(tr!(
                        "{v} は {} が `extend {e} with {v}` で足した値です（{}）。",
                        "{v} is the value {} adds with `extend {e} with {v}` ({}).",
                        a.by,
                        a.file
                    ))
                    .note(tr!("{v} を取る行: {}", "The rows that take {v}: {}", rows_said.join(", ")))
                    .note(tr!(
                        "この表には {v} と書いた行が無いので、{v} の答えは、ほかの値のために書いた行から黙って決まります。表が開いていれば、寄与が {v} の行を足せます。そうでなければ、`extend` の行に `default` を付けて、この答えでよいと言ってください。",
                        "No row of this table names {v}, so its answer comes in silence from a row written for other values. If the table is open, a contribution can add a row for {v}; otherwise put `default` on the `extend` line to say that this answer is meant."
                    ));
                for (i, _) in catching.iter().skip(1) {
                    d = d.rowref(set.row_table(*i).to_string(), t.rows[*i].index);
                }
                let _ = ri;
                out.push(d);
            }
        }
    }
    out
}

/// Every finding that is about a row a contribution wrote, or about a value a contribution added,
/// says which contribution it is (§15.231): E101's example input names an added value, E102 and
/// E105 name a contributed row.
pub fn attribute(diags: &mut [Diag], f: &RuleFile) {
    if f.contributions.is_empty() && !f.enums.iter().any(|e| !e.added.is_empty()) {
        return;
    }
    let contributed: Vec<(String, String, String)> = f
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Table(t) => t.contributed.as_ref().map(|(c, file)| (t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default(), c.clone(), file.clone())),
            _ => None,
        })
        .collect();
    let added = added_values(f);
    for d in diags.iter_mut() {
        let mut notes: Vec<String> = Vec::new();
        if d.code == "E101" {
            for (_, v) in &d.witness.inputs {
                if let crate::diag::WVal::Str(s) = v {
                    for (_, val, a) in &added {
                        if val == s {
                            notes.push(tr!("{s} は {} が足した値です（{}）。", "{s} is a value {} adds ({}).", a.by, a.file));
                        }
                    }
                }
            }
        }
        if matches!(d.code, "E101" | "E102" | "E105" | "W114" | "W117") {
            let mut tables: Vec<String> = d.rows.iter().map(|r| r.table.clone()).collect();
            if let Some(t) = &d.table {
                tables.push(t.clone());
            }
            for (t, c, file) in &contributed {
                if tables.iter().any(|x| x == t) {
                    notes.push(tr!("{t} は寄与 {c} の定義です（{file}）。", "{t} is a definition contribution {c} adds ({file})."));
                }
            }
        }
        // A claim of chances that the merged rule breaks (§15.224): the definitions contributions add
        // that the claimed answer is decided from, through whatever they are read by.
        if d.code == "E133" && !f.contributions.is_empty() {
            let mut want: std::collections::HashSet<String> = f
                .chances
                .iter()
                .filter(|ch| ch.rows.iter().any(|r| d.marks.iter().any(|m| m.span.same(&r.chance_span))))
                .flat_map(|ch| ch.outputs.iter().map(|(n, _)| n.clone()))
                .collect();
            loop {
                let before = want.len();
                for it in &f.items {
                    if crate::types::item_binds(f, it).iter().any(|n| want.contains(n)) {
                        want.extend(crate::types::item_reads(it));
                    }
                }
                if want.len() == before {
                    break;
                }
            }
            for it in &f.items {
                let Item::Table(t) = it else { continue };
                let Some((c, file)) = &t.contributed else { continue };
                if t.outputs.iter().any(|o| want.contains(&o.name.text)) {
                    let tn = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
                    notes.push(tr!(
                        "{tn} は寄与 {c} の定義で、この答えを決める値に関わります（{file}）。規則だけの検査でこの主張が成り立つなら、答えの確率を変えたのはこの寄与です。",
                        "{tn}, a definition contribution {c} adds ({file}), takes part in deciding this answer. If the claim holds for the rule alone, this contribution is what moved its chance."
                    ));
                }
            }
        }
        notes.dedup();
        for n in notes {
            if !d.notes.contains(&n) {
                d.notes.push(n);
            }
        }
    }
}

/// The files a command that works on rules works on, given `files` (§15.231): a contribution stands
/// for the rule it comes down to, which is merged with every contribution of the run that reaches
/// it; each rule once, in the order first named.
pub fn roots(files: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in files {
        let r = if is_contribution_file(p) { root_of(p).unwrap_or_else(|| p.clone()) } else { p.clone() };
        if !out.iter().any(|q| same_file(q, &r)) {
            out.push(r);
        }
    }
    out
}

/// Moves every item of the contribution `cname` (its own names are `<cname>:…`; a table or clause
/// of an open output stays where it is) down to right before the first item that reads what it
/// binds, the last of them first, so that a chain moves together.
fn sink(f: &mut RuleFile, cname: &str) {
    let prefix = format!("{cname}:");
    let own = |f: &RuleFile, it: &Item| -> bool {
        let names = crate::types::item_binds(f, it);
        match it {
            Item::Table(t) => t.contributed.as_ref().is_some_and(|(c, _)| c == cname) && names.iter().all(|n| n.starts_with(&prefix)),
            _ => !names.is_empty() && names.iter().all(|n| n.starts_with(&prefix)),
        }
    };
    let mut idx: Vec<usize> = (0..f.items.len()).filter(|&i| own(f, &f.items[i])).collect();
    idx.reverse();
    for i in idx {
        let binds = crate::types::item_binds(f, &f.items[i]);
        let first_reader = (i + 1..f.items.len()).find(|&j| crate::types::item_reads(&f.items[j]).iter().any(|r| binds.contains(r)));
        let Some(j) = first_reader else { continue };
        if j > i + 1 {
            let it = f.items.remove(i);
            f.items.insert(j - 1, it);
        }
    }
}

/// The path of `to` as written from the directory of the file at `from`: how a generated header
/// and the certificate name a contribution, as an `apply` names its callee.
pub fn relative(from: &str, to: &str) -> String {
    let a = normalize(Path::new(from));
    let b = normalize(Path::new(to));
    let dir: Vec<&str> = match a.rfind('/') {
        Some(k) => a[..k].split('/').filter(|x| !x.is_empty() && *x != ".").collect(),
        None => Vec::new(),
    };
    let to_parts: Vec<&str> = b.split('/').filter(|x| !x.is_empty() && *x != ".").collect();
    if a.starts_with('/') != b.starts_with('/') {
        return b;
    }
    let common = dir.iter().zip(&to_parts).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<String> = std::iter::repeat_n("..".to_string(), dir.len() - common).collect();
    parts.extend(to_parts[common..].iter().map(|x| x.to_string()));
    parts.join("/")
}

/// `rulec api`'s `contributions` (§15.231): each contribution merged into the rule, its file from
/// the rule's directory, the shape it pinned, its SHA-256, and the inputs it added; then each merged
/// into a rule it applies, named `<apply>:<name>`, which adds no input (§15.237).
pub fn contributions_json(f: &RuleFile, shown: &str) -> String {
    // The ones merged into a rule it applies come after its own, under the apply's name (§15.237).
    let direct = f.contributions.iter().filter(|m| m.via.is_none());
    crate::json::arr(
        &direct
            .chain(f.contributions.iter().filter(|m| m.via.is_some()))
            .map(|m| {
                crate::json::Obj::new()
                    .str("name", &m.shown_name())
                    .str("file", &relative(shown, &m.file))
                    .str("signature", m.signature.as_deref().unwrap_or(""))
                    .str("sha256", &m.sha256)
                    .raw("inputs", crate::json::strs(&m.inputs))
                    .finish()
            })
            .collect::<Vec<_>>(),
    )
}

/// `rulec api`'s `opens` (§15.227): each open value, how its terms are put together, the value they
/// start from, its range, and every term with the contribution it comes from.
pub fn opens_json(f: &RuleFile) -> String {
    crate::json::arr(
        &f.opens
            .iter()
            .map(|o| {
                let terms: Vec<String> = o
                    .terms
                    .iter()
                    .map(|t| crate::json::Obj::new().str("name", &t.name.text).opt_str("from", t.from.as_ref().map(|(c, _)| c.as_str())).finish())
                    .collect();
                crate::json::Obj::new()
                    .str("name", &o.name.text)
                    .str("type", &type_text(&o.ty))
                    .str("combine", o.combine.as_ref().map(|(c, _)| c.word()).unwrap_or(""))
                    .opt_str("empty", o.empty.as_ref().map(|(l, _)| l.src()).as_deref())
                    .str("range", &o.range.as_ref().map(range_text).unwrap_or_default())
                    .raw("terms", crate::json::arr(&terms))
                    .finish()
            })
            .collect::<Vec<_>>(),
    )
}
