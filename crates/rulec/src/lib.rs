//! rulec — checks decision tables of business rules and generates code for every target
//! [`backend::ALL`] names.
//! The design is in DESIGN.md.
//!
//! The modules are public so that the tests, ritsu (its commands and its playground) and
//! `rulec mcp` can reach them, not as an interface: they change with the tool, and
//! docs/compatibility.md leaves them out of what 1.x keeps. Use the command line, `rulec mcp`,
//! or the generated code.

/// A user-facing sentence in both languages: `tr!("日本語", "English")`.
///
/// Expands to a `String` in the current output language (`i18n`). Both texts are
/// `format!` strings and receive the same arguments, so `tr!("{} 件", "{} items", n)`
/// and inline captures such as `tr!("{name} が…", "{name} is…")` both work.
/// Keep the two texts adjacent: a message that exists in only one language is
/// a bug, and the compiler checks the placeholders of both.
#[macro_export]
macro_rules! tr {
    ($ja:literal, $en:literal) => {
        if $crate::i18n::ja() { ::std::format!($ja) } else { ::std::format!($en) }
    };
    ($ja:literal, $en:literal, $($arg:tt)+) => {
        if $crate::i18n::ja() { ::std::format!($ja, $($arg)+) } else { ::std::format!($en, $($arg)+) }
    };
}

pub mod ast;
pub mod apply;
/// How likely each answer is when the `random` inputs are drawn (§15.224).
pub mod chances;
pub mod backend;
pub mod cel;
pub mod cert;
pub mod child;
/// The `rulec` command, as a function: the binary runs it, and so does `ritsu rulec`. Everywhere but
/// the page in the browser (wasm32-unknown-unknown, which has no command line): the `ritsu` built for
/// WASI runs it too (ritsu's DESIGN 8.8).
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub mod cli;
/// The days a date input takes from a koyomi file (`range from koyomi …`, §15.174), read
/// through ritsu's port of dates.
pub mod days;
pub mod defset;
pub mod diag;
pub mod doc;
pub mod door;
pub mod docx;
pub mod enums;
pub mod eval;
pub mod fixtures;
pub mod fmt;
pub mod fourier;
pub mod grid;
pub mod graph;
pub mod i18n;
pub mod extract;
pub mod import;
pub mod codegen;
pub mod codes;
pub mod coverage;
pub mod json;
pub mod jsonschema;
pub mod kw;
pub mod lex;
/// Merging contributions into the rules they add to (§15.227–§15.231).
pub mod link;
pub mod machine;
/// One name, one declaration: what a file declares twice stops the check (§15.262).
pub mod names;
pub mod num;
pub mod ooxml;
/// What an output comes to with some inputs held to part of their range (ritsu's port
/// `Rules::outputs_over`).
pub mod over;
pub mod parse;
/// Which values of an enum need which before them (§15.222).
pub mod prereq;
/// ritsu's ports, as rulec answers them (ritsu's DESIGN 3.2).
pub mod ports;
pub mod prelude;
pub mod proto;
pub mod region;
pub mod relation;
pub mod replay;
pub mod projection;
pub mod report;
pub mod runtest;
/// SHA-256 (FIPS 180-4), ritsu-base's: the pinned digests of sources (§15.68) and the generated
/// headers.
pub use ritsu_base::sha256;
pub mod sources;
pub mod types;
pub mod vfs;
pub mod vdiff;
pub mod vectors;
pub mod verify;
pub mod xlsx;

use diag::{Diag, Severity};

/// Checks one `.rule` and returns its diagnostics. There are three stages, and when a
/// stage reports an error the next one does not run (type failures stacked on top of
/// syntax failures are unreadable).
pub struct Report {
    pub diags: Vec<Diag>,
    /// Total number of nodes the check visited (measured to set the §15-2 budget).
    pub nodes: i64,
    pub quiet: Vec<Diag>,
    pub shadow: region::Shadow,
    /// The contributions merged into the rule the report is about, as the run shows them
    /// (§15.231): what `check`'s last line names.
    pub merged: Vec<String>,
    /// For a contribution: the file it adds to, as the run shows it.
    pub adds_to: Option<String>,
}

impl Report {
    fn of(diags: Vec<Diag>, quiet: Vec<Diag>, shadow: region::Shadow, nodes: i64) -> Report {
        Report { diags, quiet, shadow, nodes, merged: Vec::new(), adds_to: None }
    }
}

/// What makes two findings the same, for a report of two checks (the rule alone and the rule
/// merged, §15.231) and for a run that checks a rule and the contributions to it: the code, the
/// title, where it is, and the place of every mark in its file.
/// Where a finding points: the place of every mark in its file.
fn finding_marks(d: &Diag, path: &str) -> String {
    d.marks.iter().map(|m| format!("{}:{}:{}:{}", m.span.file_path().unwrap_or(path), m.span.line, m.span.col, m.span.len)).collect::<Vec<_>>().join(",")
}

pub fn finding_key(d: &Diag, path: &str) -> String {
    let marks: Vec<String> = d.marks.iter().map(|m| format!("{}:{}:{}:{}", m.span.file_path().unwrap_or(path), m.span.line, m.span.col, m.span.len)).collect();
    format!("{}|{}|{}|{}|{}", d.code, d.title, d.where_, marks.join(","), d.notes.join("\n"))
}

pub fn check_source(src: &str, path: &str) -> Vec<Diag> {
    report(src, path).diags
}

pub fn report(src: &str, path: &str) -> Report {
    report_with(src, path, region::DEFAULT_BUDGET)
}

/// [`report`] with a budget. The keys written in the file come first (W901, ritsu's DESIGN
/// 16.3): they are said whatever else the rule holds, and they stop nothing.
pub fn report_with(src: &str, path: &str, budget: i64) -> Report {
    let mut r = stages(src, path, budget);
    r.diags.splice(0..0, diag::keys(src, path));
    r
}

/// The three stages of the check. A rule a contribution of the run reaches is checked twice
/// (§15.231): alone, with its examples — what it claims when there are none — and merged with them,
/// without its examples, which are about it alone. The report holds what both say, each finding
/// once.
fn stages(src: &str, path: &str, budget: i64) -> Report {
    let parsed = parse::parse(src, path);
    let diags = parsed.diags.clone();
    if !diags.is_empty() {
        return Report::of(diags, Vec::new(), region::Shadow::default(), 0);
    }
    let Some(f) = parsed.file else { return Report::of(diags, Vec::new(), region::Shadow::default(), 0) };
    match f.kind {
        ast::FileKind::Rule if link::reaches(path) => {
            let alone = link::without(|| pipeline(f.clone(), path, budget));
            let mut merged = pipeline(f, path, budget);
            // A claim of chances the rule alone breaks the same way is said once, as the rule's
            // (§15.224): the merged rule says it again only when the contributions move the chance.
            let alone_claims: Vec<(String, Option<String>)> = alone
                .diags
                .iter()
                .filter(|d| d.code == "E133")
                .map(|d| (finding_marks(d, path), d.notes.first().cloned()))
                .collect();
            merged.diags.retain(|d| d.code != "E133" || !alone_claims.contains(&(finding_marks(d, path), d.notes.first().cloned())));
            let seen: std::collections::HashSet<String> = alone.diags.iter().map(|d| finding_key(d, path)).collect();
            let mut diags = alone.diags;
            diags.extend(merged.diags.drain(..).filter(|d| !seen.contains(&finding_key(d, path))));
            let qseen: std::collections::HashSet<String> = alone.quiet.iter().map(|d| finding_key(d, path)).collect();
            let mut quiet = alone.quiet;
            quiet.extend(merged.quiet.drain(..).filter(|d| !qseen.contains(&finding_key(d, path))));
            Report { diags, quiet, shadow: merged.shadow, nodes: alone.nodes + merged.nodes, merged: merged.merged, adds_to: None }
        }
        _ => pipeline(f, path, budget),
    }
}

/// The check of one parsed file, from the enums it reads to the claims of its machine.
fn pipeline(mut f: ast::RuleFile, path: &str, budget: i64) -> Report {
    let mut r = pipeline_inner(&mut f, path, budget);
    // A finding about a line of a file merged into this one says that file (§15.227).
    link::relocate(&mut r.diags, path);
    link::relocate(&mut r.quiet, path);
    if r.adds_to.is_none() {
        // The contributions merged into it, those merged into a rule it applies (§15.237), and those
        // that added a value to an enum it reads from an `enums` file: the check was of the rule
        // with them.
        let mut merged: Vec<String> = Vec::new();
        let direct = f.contributions.iter().filter(|m| m.via.is_none());
        let applied = f.contributions.iter().filter(|m| m.via.is_some());
        for file in direct.chain(applied).map(|m| &m.file).chain(f.enums.iter().flat_map(|e| &e.added).map(|a| &a.file)) {
            if !merged.contains(file) {
                merged.push(file.clone());
            }
        }
        r.merged = merged;
    }
    r
}

fn pipeline_inner(f: &mut ast::RuleFile, path: &str, budget: i64) -> Report {
    let mut diags = Vec::new();
    let mut quiet = Vec::new();
    let mut shadow = region::Shadow::default();
    let mut nodes = 0i64;
    let adds_to = (f.kind == ast::FileKind::Contribution).then(|| f.contrib.as_ref().and_then(|h| h.to.as_ref()).and_then(|t| link::join(path, &t.file))).flatten();
    let done = |diags: Vec<Diag>, quiet: Vec<Diag>, shadow: region::Shadow, nodes: i64| Report { diags, quiet, shadow, nodes, merged: Vec::new(), adds_to: adds_to.clone() };
    // A name the file's lines declare twice, or two names given one identifier (§15.262): whatever
    // reads the name below would read one of the two, so the check stops before anything does.
    diags.extend(names::check(f, path));
    if has_error(&diags) {
        return done(diags, quiet, shadow, nodes);
    }
    // The enums it reads from an `enums` file come first (§15.212): everything below, the
    // rule an `apply` brings in included, is checked over them as over the rule's own.
    diags.extend(enums::read_imports(f, path));
    if has_error(&diags) {
        return done(diags, quiet, shadow, nodes);
    }
    // A rule applied by this one is read and expanded into it first (§15.69). A callee that
    // cannot be expanded leaves holes everything below would trip over, so those errors
    // return here; a pin that does not match (E040) is reported and the check goes on, so
    // that what the changed callee does to this rule can be seen.
    diags.extend(apply::expand(f, path));
    if diags.iter().any(|d| d.severity == Severity::Error && d.code != "E040") {
        return done(diags, quiet, shadow, nodes);
    }
    // The contributions the run gives are merged into the rule they reach, and its open values
    // become `define`s (§15.227); a contribution becomes the rule it adds to, merged with it and
    // with what it comes after. Its pins (E040) are said and the check goes on, as an apply's.
    diags.extend(link::merge(f, path));
    if diags.iter().any(|d| d.severity == Severity::Error && d.code != "E040") {
        return done(diags, quiet, shadow, nodes);
    }
    // Every spelling of an imported division becomes the one value it stands for (§15.182).
    let spelled = prelude::normalize(f, path);
    if has_error(&spelled) {
        diags.extend(spelled);
        return done(diags, quiet, shadow, nodes);
    }
    // A bare word is read where the type of the place says what it is: a value of the column's
    // enum, or a name (§15.208). Two readings left open (E075) go on with the type errors.
    diags.extend(types::read_words(f, path));
    let f = &*f;

    let t = types::check(f, path);
    diags.extend(t.diags.iter().cloned());
    diags.extend(apply::check(f, &t, path));
    // What the merge proves once the rule is typed: the values a contribution added fall to no
    // row in silence (E033), and every finding about a contributed row says whose it is.
    diags.extend(link::check(f, &t, path));
    // The stages that read other files: the enums declared outside it (§15.59, §15.60), and
    // the copies of the sources it cites (§15.68).
    diags.extend(enums::check(f, &t, path));
    diags.extend(projection::check(f, &t, path));
    diags.extend(sources::check(f, path));
    // The shape of the prerequisites (§15.222): what the graph below is read from.
    diags.extend(prereq::check_shape(f, &t, path));
    enrich_e104(&mut diags, f, &t);
    enrich_arith(&mut diags, f, &t);
    if diags.iter().any(|d| d.severity == Severity::Error) {
        return done(diags, quiet, shadow, nodes);
    }
    // The unit is the definition set: one table, or every table that defines one output
    // (§15.66).
    for set in &t.sets {
        let r = region::check_set(set, &t, f, path, budget);
        diags.extend(r.diags);
        quiet.extend(r.quiet);
        shadow.structural += r.shadow.structural;
        shadow.equivalent += r.shadow.equivalent;
        shadow.confirm += r.shadow.confirm;
        nodes += r.nodes;
    }
    diags.extend(eval::check_examples(f, &t, path));
    // The machine the rule is one step of (§15.148). Its claims are about sequences of calls,
    // and a step with a hole or two answers is not a step yet: they wait for the tables.
    let tables_hold = !diags.iter().any(|d| d.severity == Severity::Error && d.code.starts_with("E1") && d.code != "E107");
    if f.machine.is_some() {
        diags.extend(machine::check_scenarios(f, &t, path));
        if tables_hold {
            diags.extend(machine::check(f, &t, path, budget.max(0) as usize));
        }
    }
    // The prerequisites (§15.222): a finite graph, walked whole. The order an `ordered by` table
    // gives is read once the tables hold.
    diags.extend(prereq::check_graph(f, &t, path, tables_hold));
    // The claims about the chances (§15.224): counted once the tables hold, as the machine's are.
    if tables_hold && !f.chances.is_empty() {
        diags.extend(chances::check(f, &t, path, budget.max(0) as usize));
    }
    link::attribute(&mut diags, f);
    done(diags, quiet, shadow, nodes)
}

/// Per-set check results (shadowing pairs and dead rows) that the §9.2 coverage decision
/// needs. Returned in the order of `c.sets`.
pub fn table_checks(
    f: &ast::RuleFile,
    c: &types::Checked,
    path: &str,
) -> Vec<region::TableCheck> {
    c.sets.iter().map(|s| region::check_set(s, c, f, path, region::DEFAULT_BUDGET)).collect()
}

pub fn has_error(ds: &[Diag]) -> bool {
    ds.iter().any(|d| d.severity == Severity::Error)
}

/// Every finding of one file, in the frame §11 fixes, with a blank line between them.
///
/// `main` walks the list itself (it also counts, and `--format json` and `--terse` are
/// other renderings of the same list), and prints this for the default one; `ports::Engine`
/// returns it for `ritsu check`, which ritsu's playground runs too. Two implementations would
/// be two sets of answers to keep true.
pub fn findings_text(diags: &[Diag], lines: &[String]) -> String {
    let mut out = String::new();
    for d in diags {
        out.push_str(&diag::render(d, lines));
        out.push('\n');
    }
    out
}

/// What `check` prints after the findings: the count of findings the base revision already
/// had (`--diff-base`), the count of shadowing pairs (§4), and the `ok` line when nothing
/// is an error. `--format json` prints none of it; `--terse` prints all of it.
pub fn check_tail(shadow: &region::Shadow, diags: &[Diag], path: &str, suppressed: usize) -> String {
    check_tail_of(shadow, diags, path, suppressed, &[], None)
}

/// [`check_tail`] of a report that merged contributions or is about one (§15.231): the `ok` line
/// names the contributions merged into the rule, or the file a contribution adds to.
pub fn check_tail_of(shadow: &region::Shadow, diags: &[Diag], path: &str, suppressed: usize, merged: &[String], adds_to: Option<&str>) -> String {
    let mut out = String::new();
    if suppressed > 0 {
        out.push_str(&tr!(
            "note {path}: 基準リビジョンに既にあった指摘 {suppressed} 件は伏せました（--diff-base）\n",
            "note {path}: suppressed {suppressed} findings already present at the base revision (--diff-base)\n"
        ));
    }
    // §4: only the pairs that need review are listed; the rest is a single count line.
    if shadow.total() > 0 {
        out.push_str(&tr!(
            "note {path}: 隠れ {} 対（階段 {}、同じ答え {}、要確認 {}）\n",
            "note {path}: {} shadow pairs ({} structural, {} equivalent, {} needs review)\n",
            shadow.total(),
            shadow.structural,
            shadow.equivalent,
            shadow.confirm
        ));
    }
    if !has_error(diags) {
        if !merged.is_empty() {
            let n = merged.len();
            let list = merged.join(", ");
            let word = if n == 1 { "contribution" } else { "contributions" };
            out.push_str(&tr!("ok {path}（寄与 {n} 件: {list}）\n", "ok {path} ({n} {word}: {list})\n"));
        } else if let Some(t) = adds_to {
            out.push_str(&tr!("ok {path}（{t} への寄与）\n", "ok {path} (contributes to {t})\n"));
        } else {
            out.push_str(&format!("ok {path}\n"));
        }
    }
    out
}

/// Splits the E104 wording in two. The branch is not a guess: witnesses are actually
/// evaluated and the choice depends on whether a fraction appears. An output that is only
/// looked up from a table never produces a fraction, so for it the second job of the
/// rounding declaration (checking that literals sit on the grid) is given as the reason.
/// The input behind E132 and E139: one that makes the divisor of a rounded quotient zero, or the
/// argument of an `isqrt` negative (§15.213, §15.215), named in the note and in the witness. It is
/// looked for over the inputs the expression turns on, as E101's example input is (§15.194):
/// every combination where there are few enough, else each input at the ends of its range and at
/// zero. The input has to be one the door takes — inside every `constraint` and on koyomi's days —
/// and the reference evaluator has to say the expression comes to zero, or below it. None found,
/// nothing is named: the ranges reach the value, and that is what the error says.
fn enrich_arith(diags: &mut [diag::Diag], f: &ast::RuleFile, t: &types::Checked) {
    use eval::Val;
    use num::Rat;
    use std::collections::{BTreeMap, BTreeSet};
    for w in &t.arith_witness {
        let same = |s: &diag::Span| s.same(&w.span);
        let Some(d) = diags.iter_mut().find(|d| d.code == w.code && d.marks.first().is_some_and(|m| same(&m.span))) else { continue };
        // E131 (§15.227): an input whose terms put the open value outside the range it declares.
        let open_range = f.opens.iter().find(|o| o.span.same(&w.span)).and_then(|o| t.opens.get(&o.name.text)).copied();
        let hits = |a: &BTreeMap<String, Val>| -> bool {
            if !vectors::allowed(f, a) || !vectors::days_ok(t, a) {
                return false;
            }
            let (_, _, b) = eval::run_bindings(f, t, a.clone().into_iter().collect());
            match eval::Env::new(t, b).expr(&w.expr) {
                Some(Val::Num(v)) => {
                    if w.code == "E132" {
                        v.num == 0
                    } else if w.code == "E131" {
                        let (lo, hi) = open_range.unwrap_or((None, None));
                        lo.is_some_and(|l| v.cmp_to(l) == std::cmp::Ordering::Less) || hi.is_some_and(|h| v.cmp_to(h) == std::cmp::Ordering::Greater)
                    } else {
                        v.num < 0
                    }
                }
                _ => false,
            }
        };
        let mut names = Vec::new();
        types::collect_names(&w.expr, &mut names);
        let found = match grid::exhaust_input(f, t, &names, &BTreeMap::new(), &BTreeSet::new(), hits) {
            Some(Some(a)) => Some(a),
            Some(None) => None,
            // Too many to walk: each input the expression reads at the ends of its range and at
            // zero, every other input at the bottom of its own.
            None => {
                let inputs: Vec<&ast::VarDecl> = f.inputs.iter().collect();
                let mut leaves: Vec<String> = Vec::new();
                let reads = coverage::reads_of(f);
                for n in &names {
                    for l in coverage::leaves(&reads, n) {
                        if inputs.iter().any(|i| i.name.text == l) && !leaves.contains(&l) {
                            leaves.push(l);
                        }
                    }
                }
                let base: BTreeMap<String, Val> = inputs
                    .iter()
                    .filter_map(|i| {
                        let (lo, _) = t.ranges.get(&i.name.text).copied()?;
                        Some((i.name.text.clone(), Val::Num(lo?)))
                    })
                    .collect();
                let points = |n: &String| -> Vec<Rat> {
                    let (lo, hi) = t.ranges.get(n).copied().unwrap_or((None, None));
                    let mut ps: Vec<Rat> = lo.into_iter().chain(hi).collect();
                    if lo.is_none_or(|l| l.num <= 0) && hi.is_none_or(|h| h.num >= 0) {
                        ps.push(Rat::zero());
                    }
                    ps
                };
                let lists: Vec<Vec<Rat>> = leaves.iter().map(points).collect();
                if leaves.len() > 6 || lists.iter().any(|l| l.is_empty()) || base.len() != inputs.len() {
                    None
                } else {
                    let mut at = vec![0usize; leaves.len()];
                    let mut got = None;
                    'walk: loop {
                        let mut a = base.clone();
                        for (i, n) in leaves.iter().enumerate() {
                            a.insert(n.clone(), Val::Num(lists[i][at[i]]));
                        }
                        if hits(&a) {
                            got = Some(a);
                            break 'walk;
                        }
                        let mut i = 0;
                        loop {
                            if i == at.len() {
                                break 'walk;
                            }
                            at[i] += 1;
                            if at[i] < lists[i].len() {
                                break;
                            }
                            at[i] = 0;
                            i += 1;
                        }
                    }
                    got
                }
            }
        };
        let Some(a) = found else { continue };
        let shown: Vec<String> = f
            .inputs
            .iter()
            .filter(|i| names.iter().any(|n| coverage::leaves(&coverage::reads_of(f), n).contains(&i.name.text)))
            .filter_map(|i| a.get(&i.name.text).map(|v| (i.name.text.clone(), v.clone())))
            .map(|(n, v)| format!("{n} = {}", vectors::show_named(t, &n, &v)))
            .collect();
        for i in &f.inputs {
            if let Some(v) = a.get(&i.name.text) {
                d.witness.inputs.push((i.name.text.clone(), eval::wval(t, &i.name.text, v)));
            }
        }
        let b = shown.join(", ");
        d.notes.insert(
            0,
            if w.code == "E132" {
                tr!("割る数を 0 にする入力: {b}", "An input that makes the divisor zero: {b}")
            } else if w.code == "E131" {
                tr!("範囲の外にする入力: {b}", "An input that takes it out of the range: {b}")
            } else {
                tr!("引数を負にする入力: {b}", "An input that makes the argument negative: {b}")
            },
        );
    }
}

fn enrich_e104(diags: &mut [diag::Diag], f: &ast::RuleFile, t: &types::Checked) {
    use num::{Rat, RoundMode, RoundTo};
    let Some(raw) = eval::unrounded_output(f, t) else { return };
    // The example is about this rule's own output, so it is written in that output's unit.
    // It used to be yen whatever the rule counted, which told the writer of a rule in dollars
    // that "some input computes to 35.5 yen" and to try `round up(10円)` — a message about a
    // currency the rule does not have (§15.111). A rate is held as a fraction and written in
    // percent, so it is put in percent before anything is said about it: 12% used to come out
    // as "0.12%", rounded as if it were 0.12%.
    let unit = unit_word(f, t);
    let u = unit.as_str();
    let first = f.outputs.first().map(|o| o.name.text.as_str()).unwrap_or("");
    let rate = matches!(t.syms.get(first).map(|s| s.ty.present()), Some(types::Ty::Rate));
    let scale = t.wire_scale(first);
    // A fraction of the grid the output is stored on: a whole yen, one step of a rate.
    let whole = raw.mul(Rat::int(scale)).is_int();
    let raw = if rate { raw.mul(Rat::int(100)) } else { raw };
    // The grid a hint names: a rate's own step, ten of anything else.
    let grid = if rate { format!("{}%", Rat::new(100, scale)) } else { format!("10{u}") };
    let typo = if rate { "1.451%".to_string() } else { format!("1451{u}") };
    for d in diags.iter_mut().filter(|d| d.code == "E104") {
        // Only form A (a fraction can occur) says "an unrounded value reaches the output".
        // Giving form B the same first line would claim a leak where nothing leaks.
        if !whole {
            d.title = tr!("丸めていない値が出力に到達します", "An unrounded value reaches the output");
        }
        if whole {
            d.notes.push(tr!(
                "入力をいくつか試しましたが、この出力に端数は生まれませんでした（表から引いた値がそのまま出るか、式の中で既に丸めているためです）。",
                "Several witnesses were tried and none produced a fraction in this output (either the amount looked up from the table is emitted as is, or the expression already rounds it)."
            ));
            d.notes.push(tr!(
                "ここでは、丸めの宣言は別の役にも立ちます。出力のセルに書いた値がその刻みに載っているかを検査するので、{typo} のような打ち間違いを E106 で見つけられます。",
                "Here the rounding declaration does its second job: it is used to check that the literals in the output cells sit on that grid, so a mistyped digit such as {typo} is stopped by E106."
            ));
            d.notes.push(tr!(
                "ヒント: 出力の宣言に丸めを書いてください。例: {} {}({grid})",
                "Hint: add rounding to the output declaration, e.g. {} {}({grid})",
                crate::kw::ROUND, crate::kw::UP
            ));
        } else {
            d.notes.push(tr!(
                "端数がどう決まるかを宣言しないと、生成コードが黙って決めます。",
                "Unless you declare how the fraction is settled, the generated code decides silently."
            ));
            let one = Rat::int(1);
            let ten = Rat::int(10);
            let down = raw.round_to(RoundMode::Down, one);
            let half = raw.round_to(RoundMode::Half, one);
            let up10 = raw.round_to(RoundMode::Up, ten);
            let spread = up10.sub(down);
            d.notes.push(tr!(
                "例: 計算値が {raw}{u} になる入力があります。{}(1{u}) なら {down}{u}、{}(1{u}) なら {half}{u}、{}(10{u}) なら {up10}{u} と、丸め方で最大 {spread}{u} 動きます。",
                "Example: some input computes to {raw}{u}. {}(1{u}) gives {down}{u}, {}(1{u}) gives {half}{u} and {}(10{u}) gives {up10}{u}, so the rounding mode moves the result by up to {spread}{u}.",
                crate::kw::DOWN, crate::kw::HALF_UP, crate::kw::UP
            ));
            d.notes.push(tr!(
                "ヒント: 出力の宣言に丸めを書いてください。例: {} {}({grid})",
                "Hint: add rounding to the output declaration, e.g. {} {}({grid})",
                crate::kw::ROUND, crate::kw::UP
            ));
        }
    }
}

/// The unit the first numeric output counts in, as a literal writes it: `円`, `USD`, `g`,
/// `%`. A `number` counts nothing, so it is empty and the messages read as bare numbers. An
/// optional output counts in the unit of the type it wraps (§15.249).
fn unit_word(f: &ast::RuleFile, t: &types::Checked) -> String {
    let first = f.outputs.first().map(|o| o.name.text.as_str()).unwrap_or("");
    match t.syms.get(first).map(|s| s.ty.present()) {
        Some(types::Ty::Money { cur, .. }) => cur.clone(),
        Some(types::Ty::Qty { unit, .. }) => unit.clone(),
        Some(types::Ty::Rate) => "%".to_string(),
        _ => String::new(),
    }
}

/// Gathers everything generation needs in one go. Nothing is generated if syntax or types
/// fail.
pub fn prepare<'a>(src: &'a str, path: &str) -> Result<(ast::RuleFile, types::Checked), Vec<Diag>> {
    prepare_with(src, path, false)
}

/// [`prepare`], with an unpinned or changed callee (E040) let through: what `rulec diff`
/// needs, since the difference a changed callee makes is what E040 asks to be looked at.
pub fn prepare_lenient<'a>(src: &'a str, path: &str) -> Result<(ast::RuleFile, types::Checked), Vec<Diag>> {
    prepare_with(src, path, true)
}

fn prepare_with(src: &str, path: &str, tolerate_pin: bool) -> Result<(ast::RuleFile, types::Checked), Vec<Diag>> {
    let parsed = parse::parse(src, path);
    if !parsed.diags.is_empty() {
        return Err(parsed.diags);
    }
    let Some(mut f) = parsed.file else { return Err(Vec::new()) };
    // What `schema`, `api` and `gen` write from a file that declares a name twice would hold both
    // declarations, or one of the two (§15.262).
    let ds = names::check(&f, path);
    if has_error(&ds) {
        return Err(ds);
    }
    let ds = enums::read_imports(&mut f, path);
    if has_error(&ds) {
        return Err(ds);
    }
    let ds = apply::expand(&mut f, path);
    if ds.iter().any(|d| d.severity == Severity::Error && !(tolerate_pin && d.code == "E040")) {
        return Err(ds);
    }
    // What is generated from a rule is the rule merged with the contributions the run gives
    // (§15.231); from a contribution, the rule it adds to, merged with it.
    let ds = link::merge(&mut f, path);
    if ds.iter().any(|d| d.severity == Severity::Error && !(tolerate_pin && d.code == "E040")) {
        return Err(ds);
    }
    let ds = prelude::normalize(&mut f, path);
    if has_error(&ds) {
        return Err(ds);
    }
    let ds = types::read_words(&mut f, path);
    if has_error(&ds) {
        return Err(ds);
    }
    let t = types::check(&f, path);
    if t.diags.iter().any(|d| d.severity == Severity::Error) {
        return Err(t.diags);
    }
    let ds = apply::check(&f, &t, path);
    if ds.iter().any(|d| d.severity == Severity::Error) {
        return Err(ds);
    }
    // What is generated from the rule writes a name some language keeps as a keyword with `_`
    // after it (§15.210), and what an `apply` or a contribution brings in under an identifier no
    // other name of the rule is written as, in any language (§15.260). The checks above ran on the
    // names as declared.
    let mut f = f;
    codegen::assign_identifiers(&mut f);
    Ok((f, t))
}
