//! `geas affected` (DESIGN §7.5, §7.6): which claims a diff touches, and which
//! changed code no claim runs, read from a record of the lines each claim ran. It
//! runs nothing. A record is held to the code it was made on, file by file, and a
//! record of other code is refused (E062) rather than read.

use crate::diag::{self, Diag, Show, count};
use ritsu_base::text::{Lang, Text};
use crate::diff::{self, Blob, FileDiff, Mark};
use crate::hash;
use crate::json;
use crate::lines::{self, Lines};
use crate::map::{self, Record};
use crate::report;
use crate::tree;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};

pub struct Opts<'a> {
    pub root: Option<&'a str>,
    pub maps: &'a [String],
    pub json: bool,
    pub lang: Lang,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Side {
    Before,
    After,
}

impl Side {
    fn word(self) -> &'static str {
        match self {
            Side::Before => "before",
            Side::After => "after",
        }
    }
}

/// A line of the change that touches a claim.
#[derive(Clone, Debug)]
struct Touch {
    claim: usize,
    file: String,
    side: Side,
    line: u32,
    /// A removed line attributed by the lines around where it was.
    near: bool,
    /// The target whose every claim runs the line: startup code.
    startup: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Why {
    NotRun,
    NotReported,
}

/// A line of the change that no claim runs.
#[derive(Clone, Debug)]
struct Unclaimed {
    file: String,
    side: Side,
    line: u32,
    why: Why,
}

/// A record read from its file, with what `affected` looks up in it.
struct Rec {
    /// The path as the person gave it, or the default one.
    shown: String,
    text: String,
    r: Record,
    blobs: BTreeMap<String, String>,
    /// file -> (claim, target, lines)
    ran: BTreeMap<String, Vec<(usize, String, Lines)>>,
    /// target -> the claims that started it
    starts: BTreeMap<String, Vec<usize>>,
}

impl Rec {
    fn new(shown: String, text: String, r: Record) -> Rec {
        let blobs = r.files.iter().map(|f| (f.path.clone(), f.blob.clone())).collect();
        let mut ran: BTreeMap<String, Vec<(usize, String, Lines)>> = BTreeMap::new();
        for x in &r.ran {
            ran.entry(x.file.clone()).or_default().push((x.claim, x.target.clone(), x.lines.clone()));
        }
        let mut starts: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, c) in r.claims.iter().enumerate() {
            for tg in &c.targets {
                starts.entry(tg.clone()).or_default().push(i);
            }
        }
        Rec { shown, text, r, blobs, ran, starts }
    }

    fn blob(&self, path: Option<&str>) -> Option<&str> {
        path.and_then(|p| self.blobs.get(p)).map(String::as_str)
    }

    /// The claims that ran any of `lines` of a file, and the targets every claim of
    /// which ran one of them through that target: startup code, when a target has
    /// two claims or more.
    fn who_ran(&self, file: &str, lines: &[u32]) -> (BTreeSet<usize>, BTreeSet<String>) {
        let mut claims = BTreeSet::new();
        let runs = self.ran.get(file).map(Vec::as_slice).unwrap_or(&[]);
        for (c, _, l) in runs {
            if lines.iter().any(|n| l.contains(n)) {
                claims.insert(*c);
            }
        }
        let mut startup = BTreeSet::new();
        for (tg, cs) in &self.starts {
            if cs.len() >= 2
                && cs.iter().all(|c| {
                    runs.iter().any(|(rc, rt, l)| rc == c && rt == tg && lines.iter().any(|n| l.contains(n)))
                })
            {
                startup.insert(tg.clone());
            }
        }
        (claims, startup)
    }

    fn code(&self, file: &str) -> Option<&Lines> {
        self.r.file(file).and_then(|f| f.code.as_ref())
    }
}

/// What the whole change comes to.
struct Answer {
    diff_shown: String,
    records: Vec<(String, String)>,
    claims: Vec<map::RecClaim>,
    touches: Vec<Touch>,
    unclaimed: Vec<Unclaimed>,
    deleted: Vec<(String, bool)>,
    outside: Vec<String>,
    spec_changed: Vec<String>,
    baseline_changed: Option<String>,
}

impl Answer {
    fn ok(&self) -> bool {
        self.unclaimed.is_empty()
            && self.deleted.iter().all(|(_, known)| *known)
            && self.spec_changed.is_empty()
            && self.baseline_changed.is_none()
    }
}

/// A diagnostic, the file it points into (for its place), and that file's text.
type Problem = (String, Diag, String);

pub fn command(spec_file: &str, diff_arg: &str, o: &Opts) -> i32 {
    match answer(spec_file, diff_arg, o) {
        Ok(a) => {
            if o.json {
                println!("{}", to_json(&a, spec_file, diff_arg));
            } else {
                print!("{}", to_text(&a, o.lang));
            }
            if a.ok() { 0 } else { 1 }
        }
        Err(problems) => {
            if o.json {
                let pairs: Vec<(String, Diag)> = problems.iter().map(|(f, d, _)| (f.clone(), d.clone())).collect();
                println!("{}", report::failure_json(spec_file, &pairs, o.lang));
            } else {
                for (f, d, src) in &problems {
                    eprint!("{}", d.shown(f, src, o.lang));
                }
            }
            2
        }
    }
}

fn e081(file: &str, e: std::io::Error) -> Problem {
    let d = diag::error("E081", 0, 0, tr!("このファイルを読めません: {e}", "cannot read this file: {e}"));
    (file.to_string(), d, String::new())
}

fn answer(spec_file: &str, diff_arg: &str, o: &Opts) -> Result<Answer, Vec<Problem>> {
    // the diff
    let (diff_shown, bytes) = if diff_arg == "-" {
        let mut b = Vec::new();
        std::io::stdin().read_to_end(&mut b).map_err(|e| vec![e081("-", e)])?;
        ("<stdin>".to_string(), b)
    } else {
        (diff_arg.to_string(), std::fs::read(diff_arg).map_err(|e| vec![e081(diff_arg, e)])?)
    };
    answer_for(spec_file, o.root, o.maps, diff_shown, bytes)
}

/// What a diff (its bytes, and the name it is shown by) comes to for a spec, from its records:
/// what `geas affected` prints, and what ritsu's port of claims hands over (`ports.rs`).
fn answer_for(spec_file: &str, root: Option<&str>, maps: &[String], diff_shown: String, bytes: Vec<u8>) -> Result<Answer, Vec<Problem>> {
    let (root, spec_rel) =
        map::root_and_spec(spec_file, root).map_err(|d| vec![(spec_file.to_string(), d, String::new())])?;
    let spec_path = Path::new(spec_file);
    let stem = spec_path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "spec".into());
    // beside the spec, as `check` writes its journal
    let geas_dir = match spec_path.parent().filter(|d| !d.as_os_str().is_empty()) {
        Some(d) => d.join(".geas"),
        None => PathBuf::from(".geas"),
    };
    let dir_rel = spec_rel.rsplit_once('/').map_or(String::new(), |(d, _)| format!("{d}/"));
    let baseline_rel = format!("{dir_rel}.geas/{stem}.baseline.jsonl");

    // the records
    let given: Vec<String> = if maps.is_empty() {
        vec![geas_dir.join(format!("{stem}.map.jsonl")).display().to_string()]
    } else {
        maps.to_vec()
    };
    let mut recs = Vec::new();
    let mut problems = Vec::new();
    for shown in given {
        match read_record(&shown, spec_file, &spec_rel) {
            Ok(r) => recs.push(r),
            Err(p) => problems.push(p),
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }

    let diff_text = String::from_utf8_lossy(&bytes).into_owned();
    let e064 = |line: usize, msg: Text, notes: Vec<Text>| {
        let mut d = diag::error("E064", line, 0, msg);
        d.notes = notes;
        vec![(diff_shown.clone(), d, diff_text.clone())]
    };
    let files = diff::parse(&bytes).map_err(|(line, why)| {
        e064(line, tr!("差分を読めません: {}", "the diff cannot be read: {}", why.ja; why.en), vec![])
    })?;

    // sort the diff's files
    let mut answer = Answer {
        diff_shown: diff_shown.clone(),
        records: vec![],
        claims: recs[0].r.claims.clone(),
        touches: vec![],
        unclaimed: vec![],
        deleted: vec![],
        outside: vec![],
        spec_changed: vec![],
        baseline_changed: None,
    };
    let mut sources: Vec<(&FileDiff, Blob, Blob)> = Vec::new();
    for f in &files {
        let paths: Vec<&str> = [f.old_path.as_deref(), f.new_path.as_deref()].into_iter().flatten().collect();
        if paths.contains(&spec_rel.as_str()) {
            answer.spec_changed.push(f.path().to_string());
        } else if paths.contains(&baseline_rel.as_str()) {
            answer.baseline_changed = Some(baseline_rel.clone());
        } else if f.binary || !paths.iter().any(|p| tree::is_source(p)) {
            answer.outside.push(f.path().to_string());
        } else {
            let (b, a) = diff::sides(f, &root).map_err(|why| {
                e064(
                    f.at,
                    why,
                    vec![tr!(
                        "`index` の行のない差分（`diff -u` が書くもの）は、ディスクのファイルと突き合わせます。ディスクのファイルは、差分の変更前か変更後のどちらかでなければなりません。`git diff` なら両方の blob が書かれています",
                        "a diff without `index` lines, as `diff -u` writes it, is held to the file on disk, which has to be one of its two sides; a `git diff` names both",
                    )],
                )
            })?;
            sources.push((f, b, a));
        }
    }

    // hold every record to the code, file by file
    let disk = Disk::new(&root);
    let mut sides: Vec<Vec<BTreeSet<Side>>> = Vec::new(); // per record, per source file
    for rec in &recs {
        let (per_file, stale) = hold(rec, &sources, &disk);
        if !stale.is_empty() {
            problems.push(e062(rec, &stale, spec_file));
        }
        sides.push(per_file);
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    for (i, rec) in recs.iter().enumerate() {
        let all: BTreeSet<Side> = sides[i].iter().filter(|s| s.len() == 1).flat_map(|s| s.iter().copied()).collect();
        let side = match all.len() {
            0 => "either",
            1 => all.iter().next().expect("one").word(),
            _ => "mixed",
        };
        answer.records.push((rec.shown.clone(), side.to_string()));
    }

    // look each changed line up on the side a record covers
    let mut e063: Option<(Diag, Vec<String>)> = None;
    for (k, (f, _, _)) in sources.iter().enumerate() {
        let on = |side: Side| recs.iter().enumerate().find(|(i, _)| sides[*i][k].contains(&side)).map(|(_, r)| r);
        let (before, after) = (on(Side::Before), on(Side::After));
        if f.deleted() {
            answer.deleted.push((f.path().to_string(), before.is_some()));
        }
        let changes = changed_lines(f);
        if let Some(first) = changes.added.first()
            && after.is_none()
        {
            if let Some((_, more)) = e063.as_mut() {
                more.push(f.path().to_string());
                continue;
            }
            let rec = before.expect("a record fits one side of every file");
            let d = diag::error(
                "E063",
                first.at,
                0,
                tr!(
                    "差分が {} に行を足していますが、変更後のコードの記録がありません",
                    "the diff adds lines to {}, and no record is of the code after the change",
                    f.path(),
                ),
            )
            .note(tr!(
                "記録 {0} は変更前のコードのものです。変更後のコードでも記録を取り（`geas map {spec_file} --out <after.jsonl>`）、`--map {0} --map <after.jsonl>` の形で両方を渡してください",
                "the record {0} is of the code before the change: record the changed code as well (`geas map {spec_file} --out <after.jsonl>`) and give both, `--map {0} --map <after.jsonl>`",
                rec.shown,
            ));
            e063 = Some((d, vec![]));
            continue;
        }
        if let (Some(a), Some(path)) = (after, f.new_path.as_deref()) {
            for l in &changes.added {
                look_up(a, path, Side::After, l.line, &mut answer);
            }
        }
        if let (Some(b), Some(path)) = (before, f.old_path.as_deref()) {
            for l in &changes.removed {
                look_up(b, path, Side::Before, l.line, &mut answer);
            }
        } else if let (Some(a), Some(path)) = (after, f.new_path.as_deref()) {
            for block in &changes.pure_removals {
                near(a, path, f.old_path.as_deref().unwrap_or(path), block, &mut answer);
            }
        }
    }
    if let Some((mut d, more)) = e063 {
        if !more.is_empty() {
            let (n, list) = (more.len(), more.join(", "));
            d = d.note(Text::new(
                format!("ほかに {n} 件のファイルにも行を足しています: {}", more.join("、")),
                format!("it adds lines to {n} more file(s) as well: {list}"),
            ));
        }
        problems.push((diff_shown.clone(), d, diff_text.clone()));
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(answer)
}

/// Reads a record, and holds it to this spec: E060 when there is none, E061 when
/// it does not read or is another spec's.
fn read_record(shown: &str, spec_file: &str, spec_rel: &str) -> Result<Rec, Problem> {
    let text = match std::fs::read_to_string(shown) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let d = diag::error("E060", 0, 0, tr!("{shown} に記録がありません", "there is no record at {shown}"))
                .note(tr!(
                    "先に `geas map {spec_file}` を走らせてください。`geas affected` はそれが書く記録を読みます",
                    "run `geas map {spec_file}` first; `geas affected` reads the record it writes",
                ));
            return Err((spec_file.to_string(), d, String::new()));
        }
        Err(e) => return Err(e081(shown, e)),
    };
    let again = tr!(
        "記録は `geas map` が書きます。`geas map {spec_file}` を走らせると、記録が書き直されます",
        "the record is written by `geas map`; run `geas map {spec_file}` to write it again",
    );
    let r = match Record::parse(&text) {
        Ok(r) => r,
        Err((line, why)) => {
            let d = diag::error("E061", line, 0, tr!("記録を読めません: {}", "the record cannot be read: {}", why.ja; why.en))
                .note(again);
            return Err((shown.to_string(), d, text));
        }
    };
    if r.spec != spec_rel {
        let d = diag::error(
            "E061",
            1,
            0,
            tr!(
                "この記録は `{}` のもので、`{spec_rel}` のものではありません",
                "the record is of `{}`, not of `{spec_rel}`",
                r.spec,
            ),
        )
        .note(tr!(
            "記録には、ルートから見た主張のファイルのパスが入っています。別の `--root` で取った記録では、このパスが違います",
            "a record holds its spec's path relative to the root, and a record made with another `--root` names it otherwise",
        ))
        .note(again);
        return Err((shown.to_string(), d, text));
    }
    Ok(Rec::new(shown.to_string(), text, r))
}

/// The source files on disk, and their blobs, read once.
struct Disk {
    files: BTreeMap<String, String>,
}

impl Disk {
    fn new(root: &Path) -> Disk {
        let mut files = BTreeMap::new();
        for p in tree::sources(root).unwrap_or_default() {
            if let Ok(b) = std::fs::read(root.join(&p)) {
                files.insert(p, hash::blob(&b));
            }
        }
        Disk { files }
    }
}

/// One file that tells a record is not of this code.
struct Stale {
    path: String,
    why: Text,
}

/// The sides a record is of, per changed source file, and the files that show it
/// is of other code (DESIGN §7.5).
fn hold(rec: &Rec, sources: &[(&FileDiff, Blob, Blob)], disk: &Disk) -> (Vec<BTreeSet<Side>>, Vec<Stale>) {
    let mut per_file = Vec::new();
    let mut stale = Vec::new();
    let mut touched: BTreeSet<&str> = BTreeSet::new();
    for (f, before, after) in sources {
        let (old, new) = (f.old_path.as_deref(), f.new_path.as_deref());
        touched.extend(old);
        touched.extend(new);
        let mut sides = BTreeSet::new();
        // a renamed file's other path is absent from a record of one side
        if before.matches(rec.blob(old)) && (old == new || rec.blob(new).is_none()) {
            sides.insert(Side::Before);
        }
        if after.matches(rec.blob(new)) && (old == new || rec.blob(old).is_none()) {
            sides.insert(Side::After);
        }
        if sides.is_empty() {
            let has = rec.blob(new).or(rec.blob(old)).map_or("-".to_string(), |h| h.chars().take(7).collect());
            stale.push(Stale {
                path: f.path().to_string(),
                why: tr!(
                    "記録では {has} ですが、差分の変更前は {}、変更後は {} です",
                    "the record has {has}, and the diff's sides are {} before the change and {} after it",
                    before.short(),
                    after.short(),
                ),
            });
        }
        per_file.push(sides);
    }
    let paths: BTreeSet<&str> = rec.blobs.keys().map(String::as_str).chain(disk.files.keys().map(String::as_str)).collect();
    let short = |h: &str| h.chars().take(7).collect::<String>();
    for p in paths {
        if touched.contains(p) {
            continue;
        }
        let why = match (rec.blobs.get(p), disk.files.get(p)) {
            (Some(r), Some(d)) if r == d => continue,
            (Some(r), Some(d)) => tr!(
                "記録では {}、ディスクでは {} で、差分はこのファイルに触れていません",
                "the record has {}, the file on disk {}, and the diff does not touch it",
                short(r),
                short(d),
            ),
            (Some(_), None) => tr!(
                "記録にあってディスクになく、差分もこのファイルを削除していません",
                "in the record and not on disk, and the diff does not delete it",
            ),
            (None, Some(_)) => tr!(
                "ディスクにあって記録になく、差分もこのファイルを足していません",
                "on disk and not in the record, and the diff does not add it",
            ),
            (None, None) => continue,
        };
        stale.push(Stale { path: p.to_string(), why });
    }
    (per_file, stale)
}

/// E062: a record of other code, with each file that shows it.
fn e062(rec: &Rec, stale: &[Stale], spec_file: &str) -> Problem {
    let n = stale.len();
    let line = stale.iter().find_map(|s| rec.r.file_lines.get(&s.path).copied()).unwrap_or(0);
    let mut d = diag::error(
        "E062",
        line,
        0,
        tr!(
            "この記録は今のコードのものではありません。記録を取ったときと違うファイルが {n} 件あります",
            "the record is not of this code: {n} file(s) differ from the code it was made on",
        ),
    );
    const SHOWN: usize = 8;
    for s in stale.iter().take(SHOWN) {
        d = d.note(tr!("{}: {}", "{}: {}", s.path, s.why.ja; s.path, s.why.en));
    }
    if n > SHOWN {
        d = d.note(tr!("ほかに {} 件", "and {} more", n - SHOWN));
    }
    d = d.note(tr!(
        "記録から分かるのは、それを取ったときのコードのことだけです。`geas map {spec_file}` で記録を取り直してください。新しく足したファイルは、差分に入れておいてください（`git add -N`）",
        "a record answers only for the code it was made on: record this code again with `geas map {spec_file}`; a file new to the change has to be in the diff (`git add -N`)",
    ));
    (rec.shown.clone(), d, rec.text.clone())
}

/// A changed line: its number on its side, and its line in the diff.
struct Changed {
    line: u32,
    at: usize,
}

struct Changes {
    added: Vec<Changed>,
    removed: Vec<Changed>,
    /// Runs of removed lines with no added line in their place: the line of the
    /// code after the change they were below, and their numbers before it.
    pure_removals: Vec<(u32, Vec<u32>)>,
}

/// The lines a file's hunks add and remove, blank ones left out: a blank line is
/// never code.
fn changed_lines(f: &FileDiff) -> Changes {
    let mut c = Changes { added: vec![], removed: vec![], pure_removals: vec![] };
    let blank = |text: &[u8]| text.iter().all(|b| b.is_ascii_whitespace());
    for h in &f.hunks {
        let mut old = if h.old_len == 0 { h.old_start + 1 } else { h.old_start };
        let mut new = if h.new_len == 0 { h.new_start + 1 } else { h.new_start };
        let mut run: Vec<u32> = Vec::new();
        let mut run_has_added = false;
        let flush = |run: &mut Vec<u32>, has_added: &mut bool, new: u32, out: &mut Vec<(u32, Vec<u32>)>| {
            if !run.is_empty() && !*has_added {
                out.push((new - 1, std::mem::take(run)));
            }
            run.clear();
            *has_added = false;
        };
        for l in &h.lines {
            match l.mark {
                Mark::Context => {
                    flush(&mut run, &mut run_has_added, new, &mut c.pure_removals);
                    old += 1;
                    new += 1;
                }
                Mark::Removed => {
                    if !blank(&l.text) {
                        c.removed.push(Changed { line: old, at: l.at });
                        run.push(old);
                    }
                    old += 1;
                }
                Mark::Added => {
                    if !blank(&l.text) {
                        c.added.push(Changed { line: new, at: l.at });
                    }
                    run_has_added = true;
                    new += 1;
                }
            }
        }
        flush(&mut run, &mut run_has_added, new, &mut c.pure_removals);
    }
    c
}

/// One changed line looked up in a record of its side.
fn look_up(rec: &Rec, path: &str, side: Side, line: u32, answer: &mut Answer) {
    let Some(code) = rec.code(path) else {
        answer.unclaimed.push(Unclaimed { file: path.to_string(), side, line, why: Why::NotReported });
        return;
    };
    let (claims, startup) = rec.who_ran(path, &[line]);
    if claims.is_empty() {
        if code.contains(&line) {
            answer.unclaimed.push(Unclaimed { file: path.to_string(), side, line, why: Why::NotRun });
        }
        return;
    }
    touch(rec, &Touched { path, side, line, near: false }, &claims, &startup, answer);
}

/// A run of removed lines, with only a record of the code after the change: it
/// touches the claims that run the nearest code above and below where it was.
fn near(rec: &Rec, new_path: &str, old_path: &str, block: &(u32, Vec<u32>), answer: &mut Answer) {
    let (above, removed) = block;
    let Some(code) = rec.code(new_path) else {
        for l in removed {
            answer.unclaimed.push(Unclaimed { file: old_path.to_string(), side: Side::Before, line: *l, why: Why::NotReported });
        }
        return;
    };
    let up = code.range(..=*above).next_back().copied();
    let down = code.range(above + 1..).next().copied();
    let around: Vec<u32> = up.into_iter().chain(down).collect();
    let (claims, startup) = rec.who_ran(new_path, &around);
    for l in removed {
        if claims.is_empty() {
            answer.unclaimed.push(Unclaimed { file: old_path.to_string(), side: Side::Before, line: *l, why: Why::NotRun });
        } else {
            touch(rec, &Touched { path: old_path, side: Side::Before, line: *l, near: true }, &claims, &startup, answer);
        }
    }
}

/// Where a changed line that touches claims is.
struct Touched<'a> {
    path: &'a str,
    side: Side,
    line: u32,
    near: bool,
}

fn touch(rec: &Rec, at: &Touched, claims: &BTreeSet<usize>, startup: &BTreeSet<String>, answer: &mut Answer) {
    // a startup target's claims all ran the line, and the others ran it too
    let mut all: BTreeSet<usize> = claims.clone();
    for tg in startup {
        all.extend(rec.starts.get(tg).into_iter().flatten().copied());
    }
    for c in all {
        let by = startup.iter().find(|tg| rec.starts.get(*tg).is_some_and(|cs| cs.contains(&c))).cloned();
        answer.touches.push(Touch { claim: c, file: at.path.to_string(), side: at.side, line: at.line, near: at.near, startup: by });
    }
}

// ---------- the answer, for a person and as JSON ----------

/// Lines grouped as the report shows them: per file, side and kind, as sets; the
/// code after the change first within a file, as a person reads a change.
fn grouped<'a>(items: impl Iterator<Item = (&'a str, Side, bool, u32)>) -> Vec<(String, Side, bool, Lines)> {
    let mut m: BTreeMap<(String, std::cmp::Reverse<Side>, bool), Lines> = BTreeMap::new();
    for (file, side, flag, line) in items {
        m.entry((file.to_string(), std::cmp::Reverse(side), flag)).or_default().insert(line);
    }
    m.into_iter().map(|((f, s, x), l)| (f, s.0, x, l)).collect()
}

fn lines_text(side: Side, near: bool, l: &Lines, they: bool, lang: Lang) -> String {
    let r = lines::to_ranges(l);
    match (side, near, lang) {
        (Side::After, _, _) => r,
        (Side::Before, false, Lang::En) => format!("removed {r}"),
        (Side::Before, false, Lang::Ja) => format!("削除した行 {r}"),
        (Side::Before, true, Lang::En) if they => format!("removed {r} (next to lines they run)"),
        (Side::Before, true, Lang::En) => format!("removed {r} (next to lines it runs)"),
        (Side::Before, true, Lang::Ja) => format!("削除した行 {r}（通る行の隣）"),
    }
}

fn status_note(status: &str, lang: Lang) -> &'static str {
    match (status, lang) {
        ("fail", Lang::En) => " (it did not hold when the record was made)",
        ("fail", Lang::Ja) => "（記録を取ったときは成り立っていなかった）",
        ("error", Lang::En) => " (it ended in an error when the record was made)",
        ("error", Lang::Ja) => "（記録を取ったときはエラーで終わった）",
        _ => "",
    }
}

fn to_text(a: &Answer, lang: Lang) -> String {
    let mut out = String::new();
    let side_text = |s: &str| match (s, lang) {
        ("before", Lang::En) => "the code before the change",
        ("after", Lang::En) => "the code after the change",
        ("either", Lang::En) => "it fits either side",
        (_, Lang::En) => "some files before the change, some after it",
        ("before", Lang::Ja) => "変更前のコード",
        ("after", Lang::Ja) => "変更後のコード",
        ("either", Lang::Ja) => "どちらの側にも合う",
        (_, Lang::Ja) => "変更前のファイルと変更後のファイルが混在",
    };
    let recs: Vec<String> = a
        .records
        .iter()
        .map(|(f, s)| match lang {
            Lang::En => format!("{f} ({})", side_text(s)),
            Lang::Ja => format!("{f}（{}）", side_text(s)),
        })
        .collect();
    out.push_str(&match (lang, recs.len()) {
        (Lang::En, 1) => format!("diff: {} · record: {}\n", a.diff_shown, recs[0]),
        (Lang::En, _) => format!("diff: {} · records: {}\n", a.diff_shown, recs.join(", ")),
        (Lang::Ja, _) => format!("差分: {} · 記録: {}\n", a.diff_shown, recs.join("、")),
    });

    let touched: BTreeSet<usize> = a.touches.iter().map(|t| t.claim).collect();
    if !touched.is_empty() {
        out.push_str(tr!("この変更が関わる主張:\n", "claims the change touches:\n").get(lang));
        for c in &touched {
            let own: Vec<&Touch> = a.touches.iter().filter(|t| t.claim == *c && t.startup.is_none()).collect();
            if own.is_empty() {
                continue;
            }
            let rc = &a.claims[*c];
            out.push_str(&format!("  {} - {}{}\n", c + 1, rc.name, status_note(&rc.status, lang)));
            for (file, side, near, l) in grouped(own.iter().map(|t| (t.file.as_str(), t.side, t.near, t.line))) {
                out.push_str(&format!("      {file}: {}\n", lines_text(side, near, &l, false, lang)));
            }
        }
        let targets: BTreeSet<&str> = a.touches.iter().filter_map(|t| t.startup.as_deref()).collect();
        for tg in targets {
            let of: Vec<&Touch> = a.touches.iter().filter(|t| t.startup.as_deref() == Some(tg)).collect();
            let claims: BTreeSet<usize> = of.iter().map(|t| t.claim + 1).collect();
            let list: Vec<String> = claims.iter().map(|n| n.to_string()).collect();
            out.push_str(&match lang {
                Lang::En => format!("  every claim that starts `{tg}`: {}\n", list.join(", ")),
                Lang::Ja => format!("  `{tg}` を起動するすべての主張: {}\n", list.join("、")),
            });
            for (file, side, near, l) in grouped(of.iter().map(|t| (t.file.as_str(), t.side, t.near, t.line))) {
                out.push_str(&format!("      {file}: {}\n", lines_text(side, near, &l, true, lang)));
            }
        }
    }
    if !a.unclaimed.is_empty() {
        out.push_str(tr!("どの主張も通らないコードの変更:\n", "changed code no claim runs:\n").get(lang));
        for why in [Why::NotRun, Why::NotReported] {
            let of = a.unclaimed.iter().filter(|u| u.why == why);
            for (file, side, _, l) in grouped(of.map(|u| (u.file.as_str(), u.side, false, u.line))) {
                let tail = match (why, lang) {
                    (Why::NotRun, _) => "",
                    (Why::NotReported, Lang::En) => " (no runtime reported this file)",
                    (Why::NotReported, Lang::Ja) => "（どのランタイムもこのファイルを報告していません）",
                };
                out.push_str(&format!("  {file}: {}{tail}\n", lines_text(side, false, &l, false, lang)));
            }
        }
    }
    if !a.deleted.is_empty() {
        out.push_str(tr!("削除したファイル:\n", "deleted:\n").get(lang));
        for (f, known) in &a.deleted {
            let tail = match (known, lang) {
                (true, _) => "",
                (false, Lang::En) => " (with no record of the code before the change, the claims that ran it are not known)",
                (false, Lang::Ja) => "（変更前のコードの記録がないので、これを通っていた主張は分かりません）",
            };
            out.push_str(&format!("  {f}{tail}\n"));
        }
    }
    if !a.outside.is_empty() {
        out.push_str(tr!("記録の外のファイル:\n", "outside the record:\n").get(lang));
        for f in &a.outside {
            out.push_str(&format!("  {f}\n"));
        }
    }
    for f in &a.spec_changed {
        out.push_str(&match lang {
            Lang::En => format!("the spec changed: {f}; read its claims again\n"),
            Lang::Ja => format!("主張のファイルが変わりました: {f}。主張を読み直してください\n"),
        });
    }
    if let Some(f) = &a.baseline_changed {
        out.push_str(&match lang {
            Lang::En => format!("the baseline changed: {f}; drift compares against what it holds now\n"),
            Lang::Ja => format!("ベースラインが変わりました: {f}。ドリフトはこれから、この内容と比べます\n"),
        });
    }
    let unclaimed: BTreeSet<(&str, Side, u32)> = a.unclaimed.iter().map(|u| (u.file.as_str(), u.side, u.line)).collect();
    let (n, k, u) = (a.claims.len(), touched.len(), unclaimed.len());
    out.push_str(&match lang {
        Lang::En => format!(
            "{} · {k} touched · {} no claim runs\n",
            count(n, "claim", "claims"),
            count(u, "changed line", "changed lines")
        ),
        Lang::Ja => format!("主張 {n} 件 · 関わる主張 {k} 件 · どの主張も通らない変更 {u} 行\n"),
    });
    out
}

/// The lines of the change each touched claim ran, grouped by file, side (after first), how
/// and startup target: what the JSON lists under each claim, in its order.
type ClaimLines = BTreeMap<(String, std::cmp::Reverse<Side>, bool, Option<String>), Lines>;

fn touched_claims(a: &Answer) -> Vec<(usize, ClaimLines)> {
    let touched: BTreeSet<usize> = a.touches.iter().map(|t| t.claim).collect();
    touched
        .iter()
        .map(|c| {
            let mut m: ClaimLines = BTreeMap::new();
            for t in a.touches.iter().filter(|t| t.claim == *c) {
                m.entry((t.file.clone(), std::cmp::Reverse(t.side), t.near, t.startup.clone())).or_default().insert(t.line);
            }
            (*c, m)
        })
        .collect()
}

/// The changed lines no claim runs, grouped by file, side and why.
fn unclaimed_lines(a: &Answer) -> BTreeMap<(String, std::cmp::Reverse<Side>, Why), Lines> {
    let mut um: BTreeMap<(String, std::cmp::Reverse<Side>, Why), Lines> = BTreeMap::new();
    for u in &a.unclaimed {
        um.entry((u.file.clone(), std::cmp::Reverse(u.side), u.why)).or_default().insert(u.line);
    }
    um
}

fn why_word(why: Why) -> &'static str {
    match why {
        Why::NotRun => "not run",
        Why::NotReported => "not reported",
    }
}

/// What a diff comes to for a spec, as ritsu's port of claims hands it over (ritsu's DESIGN
/// 3.2): the answer `geas affected --json` prints, as types; or, when geas refuses to answer
/// (no record, a record of other code, a diff it cannot read), what it says.
pub(crate) fn for_port(spec_file: &str, root: Option<&str>, maps: &[String], diff_shown: &str, bytes: &[u8]) -> Result<ritsu_ports::Affected, Vec<ritsu_ports::Said>> {
    match answer_for(spec_file, root, maps, diff_shown.to_string(), bytes.to_vec()) {
        Err(problems) => Err(problems.into_iter().map(|(file, d, _)| ritsu_ports::Said { code: d.code.to_string(), file, line: d.line, message: d.message.clone() }).collect()),
        Ok(a) => Ok(ritsu_ports::Affected {
            records: a.records.clone(),
            claims: touched_claims(&a)
                .into_iter()
                .map(|(c, m)| ritsu_ports::Touched {
                    name: a.claims[c].name.clone(),
                    status: a.claims[c].status.clone(),
                    lines: m
                        .into_iter()
                        .map(|((file, side, near, startup), l)| ritsu_ports::TouchedLines { file, side: side.0.word().into(), lines: lines::to_ranges(&l), how: if near { "near" } else { "ran" }.into(), startup })
                        .collect(),
                })
                .collect(),
            unclaimed: unclaimed_lines(&a).into_iter().map(|((file, side, why), l)| ritsu_ports::Untouched { file, side: side.0.word().into(), lines: lines::to_ranges(&l), why: why_word(why).into() }).collect(),
            deleted: a.deleted.clone(),
            outside: a.outside.clone(),
            spec_changed: a.spec_changed.clone(),
            baseline_changed: a.baseline_changed.is_some(),
        }),
    }
}

fn to_json(a: &Answer, spec: &str, diff_arg: &str) -> String {
    let records: Vec<String> =
        a.records.iter().map(|(f, s)| format!("{{\"file\":{},\"side\":\"{s}\"}}", json::quote(f))).collect();
    let claims: Vec<String> = touched_claims(a)
        .iter()
        .map(|(c, m)| {
            let c = *c;
            let lines: Vec<String> = m
                .iter()
                .map(|((file, side, near, startup), l)| {
                    format!(
                        "{{\"file\":{},\"side\":\"{}\",\"lines\":\"{}\",\"how\":\"{}\",\"startup\":{}}}",
                        json::quote(file),
                        side.0.word(),
                        lines::to_ranges(l),
                        if *near { "near" } else { "ran" },
                        startup.as_deref().map_or("null".to_string(), json::quote)
                    )
                })
                .collect();
            let rc = &a.claims[c];
            format!(
                "{{\"index\":{},\"name\":{},\"status\":{},\"lines\":[{}]}}",
                c + 1,
                json::quote(&rc.name),
                json::quote(&rc.status),
                lines.join(",")
            )
        })
        .collect();
    let unclaimed: Vec<String> = unclaimed_lines(a)
        .iter()
        .map(|((file, side, why), l)| {
            format!(
                "{{\"file\":{},\"side\":\"{}\",\"lines\":\"{}\",\"why\":\"{}\"}}",
                json::quote(file),
                side.0.word(),
                lines::to_ranges(l),
                why_word(*why)
            )
        })
        .collect();
    let deleted: Vec<String> =
        a.deleted.iter().map(|(f, known)| format!("{{\"file\":{},\"known\":{known}}}", json::quote(f))).collect();
    let outside: Vec<String> = a.outside.iter().map(|f| json::quote(f)).collect();
    let spec_changed: Vec<String> = a.spec_changed.iter().map(|f| json::quote(f)).collect();
    format!(
        "{{\"geas\":1,\"spec\":{},\"diff\":{},\"records\":[{}],\"claims\":[{}],\"unclaimed\":[{}],\"deleted\":[{}],\"outside\":[{}],\"spec_changed\":[{}],\"baseline_changed\":{},\"ok\":{}}}",
        json::quote(spec),
        json::quote(diff_arg),
        records.join(","),
        claims.join(","),
        unclaimed.join(","),
        deleted.join(","),
        outside.join(","),
        spec_changed.join(","),
        a.baseline_changed.is_some(),
        a.ok()
    )
}
