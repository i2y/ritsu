//! `rulec strength` (§15.288): what a file's claims leave open, measured.
//!
//! `check` says a table breaks none of the file's claims. It does not say the claims decide the
//! table: a cell can often be changed with nothing in the file objecting. This makes every small
//! change it can think of, one at a time and in the **source text**, and hands each to the same
//! check: an output cell to another value, a row taken out, a boundary of a column moved in every
//! row that writes it. What comes back is who noticed: the claims and the checks, only a scenario or
//! an example, only a new warning, or nobody. A change that alters no answer (`vdiff` walks the two)
//! is the same rule written another way, and is counted apart.
//!
//! The source and not the tree, because the changes are also what a person reads and the other
//! table is a file they can run: comments and spelling stay. It says nothing about the tables a file
//! applies from another (they are that file's), and nothing to the generated code.
//!
//! The changes that nobody notices alone are then piled up, each checked on top of the last, into
//! a table that keeps everything the file says and answers differently. Two changes that pass only
//! together are not looked for, so that table is one that exists and not the largest.

use crate::ast::*;
use crate::diag::{Diag, Severity, Span};
use crate::eval::Val;
use crate::json::Obj;
use crate::lex::{Kind as Tok, lex_line};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use crate::vdiff::{self, VDiff};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::atomic::{self, AtomicUsize};

/// How many changes a file can come to before the run says how many it is about to check.
pub const MANY: usize = 10_000;

/// How long a sequence of calls is written out in the text. Longer ones are in the JSON.
const CALLS_SHOWN: usize = 10;

/// The stack of a thread that checks: as much as a main thread has, and a little more for a file
/// whose expressions nest deep.
const STACK: usize = 16 * 1024 * 1024;

/// What one run is asked for.
pub struct Options {
    /// The nodes one check may visit, as `check --budget`.
    pub budget: i64,
    /// Take the scenarios and the examples out first: how far the claims alone decide the table.
    pub claims_only: bool,
    /// Where the other table is going to be written: it has to pass there too.
    pub other: Option<String>,
}

/// Why a run did not start.
pub enum Refused {
    /// The file does not pass `check`, and what is noticed about a change to it means nothing yet.
    NotPassing(Vec<Diag>, String),
    /// Anything else, in a sentence.
    Said(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Output,
    Row,
    Boundary,
}

impl Move {
    fn word(self) -> &'static str {
        match self {
            Move::Output => "output",
            Move::Row => "row",
            Move::Boundary => "boundary",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// An error with the scenarios and the examples taken out.
    Claims,
    /// An error only with them in.
    Tests,
    /// No error, and a warning the file did not have.
    Warnings,
    Nothing,
    /// No call gets a different answer.
    Same,
}

/// A replacement in the text the changes are made to, as byte offsets.
#[derive(Debug, Clone)]
struct Edit {
    at: usize,
    end: usize,
    text: String,
}

impl Edit {
    fn meets(&self, o: &Edit) -> bool {
        self.at < o.end && o.at < self.end
    }
}

/// One change.
#[derive(Debug, Clone)]
pub struct Change {
    pub table: String,
    pub clause: bool,
    /// The row, as a diagnostic numbers it: the first that writes the boundary for a boundary.
    pub row: usize,
    pub kind: Move,
    pub output: Option<String>,
    pub column: Option<String>,
    pub from: String,
    pub to: Option<String>,
    /// The row's conditions as they read: `state = shipped, event = cancel`.
    pub when: String,
    /// The output cells it takes away or changes.
    cells: usize,
    edits: Vec<Edit>,
    order: (usize, usize, usize, usize),
}

/// A call that shows the change, already in words and in JSON: a `Checked` stays on its thread.
#[derive(Debug, Clone)]
pub struct Evidence {
    pub lines: Vec<String>,
    pub call: String,
    pub calls: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub class: Class,
    pub by: Vec<&'static str>,
    pub evidence: Option<Evidence>,
}

/// The table that keeps everything the file says.
#[derive(Debug, Clone, Default)]
pub struct Other {
    /// Its text; `None` when no change could be kept.
    pub text: Option<String>,
    /// The output cells that differ from the file's, those of the rows it takes out among them.
    pub cells: usize,
    pub of: usize,
    pub rows: usize,
    pub boundaries: usize,
}

/// The whole answer.
pub struct Strength {
    pub file: String,
    pub claims_only: bool,
    pub results: Vec<(Change, Outcome)>,
    pub other: Other,
}

impl Strength {
    pub fn count(&self, c: Class) -> usize {
        self.results.iter().filter(|(_, o)| o.class == c).count()
    }
    /// The changes counted: every one but those that alter no answer.
    pub fn changes(&self) -> usize {
        self.results.len() - self.count(Class::Same)
    }
    pub fn nothing(&self) -> usize {
        self.count(Class::Nothing)
    }
}

// ---------------------------------------------------------------------------------------
// The text

/// The lines of a text, for going from a span to its bytes.
struct Lines<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    fn new(text: &'a str) -> Lines<'a> {
        let mut starts = vec![0];
        starts.extend(text.char_indices().filter(|(_, c)| *c == '\n').map(|(i, _)| i + 1));
        Lines { text, starts }
    }
    fn offset(&self, s: &Span) -> usize {
        self.starts.get(s.line.saturating_sub(1)).copied().unwrap_or(self.text.len()) + s.col
    }
    fn of(&self, s: &Span) -> &'a str {
        let at = self.offset(s);
        self.text.get(at..at + s.len).unwrap_or("")
    }
    fn line_end(&self, line: usize) -> usize {
        self.starts.get(line).copied().unwrap_or(self.text.len())
    }
    fn line(&self, line: usize) -> &'a str {
        let a = self.starts.get(line.saturating_sub(1)).copied().unwrap_or(self.text.len());
        self.text[a..self.line_end(line)].trim_end_matches(['\n', '\r'])
    }
    /// The lines from `from` to `to`, and a blank line after them, so that none is left doubled.
    fn cut(&self, from: usize, to: usize) -> Edit {
        let at = self.starts.get(from.saturating_sub(1)).copied().unwrap_or(self.text.len());
        let mut last = to;
        if last < self.starts.len() && self.line(last + 1).trim().is_empty() && self.line_end(last + 1) > self.line_end(last) {
            last += 1;
        }
        Edit { at, end: self.line_end(last), text: String::new() }
    }
    /// `new` in place of the bytes at `at`, the spaces after them kept so that the row's bars stay
    /// where they were when `pad`.
    fn put(&self, at: usize, len: usize, new: &str, pad: bool) -> Edit {
        let end = at + len;
        if !pad {
            return Edit { at, end, text: new.to_string() };
        }
        let (old_w, new_w) = (self.text[at..end].chars().count(), new.chars().count());
        if new_w < old_w {
            return Edit { at, end, text: format!("{new}{}", " ".repeat(old_w - new_w)) };
        }
        let spaces = self.text[end..].bytes().take_while(|b| *b == b' ').count();
        Edit { at, end: end + (new_w - old_w).min(spaces.saturating_sub(1)), text: new.to_string() }
    }
}

fn apply(text: &str, edits: &[&Edit]) -> String {
    let mut sorted: Vec<&Edit> = edits.to_vec();
    sorted.sort_by_key(|e| e.at);
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for e in sorted {
        out.push_str(&text[at..e.at]);
        out.push_str(&e.text);
        at = e.end;
    }
    out.push_str(&text[at..]);
    out
}

/// The lines of the scenarios and the examples, each with the comment that stands right above it.
fn test_blocks(text: &str, f: &RuleFile) -> Vec<Edit> {
    let lines = Lines::new(text);
    let block = |head: usize, rows: &[Row]| -> Edit {
        let to = rows.last().map_or(head, |r| r.span.line);
        let mut from = head;
        while from > 1 && lines.line(from - 1).trim_start().starts_with('#') {
            from -= 1;
        }
        lines.cut(from, to)
    };
    let mut out: Vec<Edit> = f.scenarios.iter().map(|s| block(s.span.line, &s.table.rows)).collect();
    out.extend(f.examples.iter().map(|t| block(t.span.line, &t.rows)));
    out
}

// ---------------------------------------------------------------------------------------
// What can be changed

/// A value as a cell writes it, in the unit the column declares.
fn value_text(v: Rat, ty: &Ty) -> Option<String> {
    Some(match ty.present() {
        Ty::Money { cur, .. } => format!("{}{cur}", decimal(v)?),
        Ty::Qty { unit, .. } => format!("{}{unit}", decimal(v)?),
        Ty::Rate => format!("{}%", decimal(v.mul(Rat::int(100)))?),
        Ty::Number => decimal(v)?,
        Ty::Date => {
            let (y, m, d) = crate::types::ord_to_date(v);
            format!("{y:04}-{m:02}-{d:02}")
        }
        _ => return None,
    })
}

/// A rational as a decimal, when it is one.
fn decimal(v: Rat) -> Option<String> {
    let (n, d) = (v.num.unsigned_abs(), v.den.unsigned_abs());
    let mut rem = n % d;
    let mut frac = String::new();
    while rem != 0 {
        if frac.len() == 12 {
            return None;
        }
        rem *= 10;
        frac.push(char::from_digit((rem / d) as u32, 10)?);
        rem %= d;
    }
    let sign = if v.num < 0 { "-" } else { "" };
    Some(if frac.is_empty() { format!("{sign}{}", n / d) } else { format!("{sign}{}.{frac}", n / d) })
}

fn lit_value(l: &Lit, ty: &Ty) -> Option<Rat> {
    match l {
        Lit::Num(n) => crate::types::lit_value_in_pub(n, ty),
        Lit::Date(y, m, d) if *ty.present() == Ty::Date => Some(crate::types::date_ord(*y, *m, *d)),
        _ => None,
    }
}

/// The step an output's values sit on: its rounding grid, or what it travels at, or what it is held in.
fn output_step(name: &str, c: &Checked) -> Rat {
    if let Some((_, g)) = c.roundings.get(name) {
        return *g;
    }
    if let Some(k) = c.out_scales.get(name) {
        return Rat::new(1, (*k).max(1));
    }
    c.value_step(name)
}

fn in_bounds(v: Rat, decl: Option<&Range>, ty: &Ty) -> bool {
    let Some(r) = decl else { return true };
    r.bounds.iter().all(|(op, l)| match lit_value(l, ty) {
        Some(b) => {
            let o = v.cmp_to(b);
            match op {
                CmpOp::Le => o != Ordering::Greater,
                CmpOp::Ge => o != Ordering::Less,
                CmpOp::Lt => o == Ordering::Less,
                CmpOp::Gt => o == Ordering::Greater,
            }
        }
        None => true,
    })
}

/// What an output cell can become: for an enum its other values and the inputs of that enum (the
/// row that stays where it is), for a boolean its other value, and for the rest what else that
/// output's cells write in the file, and one step to either side of a literal.
fn alternatives(cur: &str, cell: &OutCell, name: &str, f: &RuleFile, c: &Checked, seen: &[String]) -> Vec<String> {
    let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
    let mut v: Vec<String> = Vec::new();
    match ty.present() {
        Ty::Enum(e) => {
            v.extend(c.enums.get(e).cloned().unwrap_or_default());
            v.extend(f.inputs.iter().filter(|i| c.ty_of(&i.name.text).is_some_and(|t| t == ty || t == *ty.present())).map(|i| i.name.text.clone()));
        }
        Ty::Bool => {
            let (yes, no) = (crate::kw::TRUE, crate::kw::FALSE);
            if cur != no {
                v.push(no.to_string());
            }
            if cur != yes {
                v.push(yes.to_string());
            }
        }
        _ => {
            v.extend(seen.iter().cloned());
            if let OutCell::Lit(l) = cell
                && let Some(x) = lit_value(l, &ty)
            {
                let step = output_step(name, c);
                let decl = f.outputs.iter().find(|o| o.name.text == name).and_then(|o| o.range.as_ref());
                for w in [x.sub(step), x.add(step)] {
                    if in_bounds(w, decl, &ty)
                        && let Some(t) = value_text(w, &ty)
                    {
                        v.push(t);
                    }
                }
            }
        }
    }
    if matches!(ty, Ty::Opt(_)) {
        v.push(crate::kw::NONE.to_string());
    }
    let mut out: Vec<String> = Vec::new();
    for x in v {
        if x != cur && !out.contains(&x) {
            out.push(x);
        }
    }
    out
}

/// The conditions of a row, as it reads.
fn when_text(t: &Table, row: &Row, lines: &Lines) -> String {
    let parts: Vec<String> = t
        .inputs
        .iter()
        .zip(&row.cells)
        .zip(&row.cell_spans)
        .filter(|((_, cell), _)| !matches!(cell, Cell::DontCare))
        .map(|(((col, _), cell), span)| match cell {
            Cell::Cmp(_) | Cell::Ranked { .. } => format!("{col} {}", lines.of(span)),
            _ => format!("{col} = {}", lines.of(span)),
        })
        .collect();
    parts.join(", ")
}

/// One comparison's literal in the text.
struct Written {
    at: usize,
    len: usize,
    op: CmpOp,
    text: String,
    table: usize,
    row: usize,
    col: usize,
    when: String,
    pad: bool,
}

/// The tables that define one output together have one column, whichever of them writes the row.
fn definition_sets(tables: &[&Table]) -> Vec<Vec<usize>> {
    let mut sets: Vec<(Vec<usize>, Vec<&str>)> = Vec::new();
    for (i, t) in tables.iter().enumerate() {
        let names: Vec<&str> = t.outputs.iter().map(|o| o.name.text.as_str()).collect();
        let (hit, rest): (Vec<_>, Vec<_>) = sets.drain(..).partition(|(_, ns)| ns.iter().any(|n| names.contains(n)));
        sets = rest;
        let mut merged = (Vec::new(), names);
        for (ts, ns) in hit {
            merged.0.extend(ts);
            merged.1.extend(ns);
        }
        merged.0.push(i);
        merged.0.sort();
        sets.push(merged);
    }
    sets.sort_by_key(|(ts, _)| ts[0]);
    sets.into_iter().map(|(ts, _)| ts).collect()
}

/// The moves of a column's boundaries. A boundary is the place between two values: `<65` and `>=65`
/// write the same one, and so do `<=1` and `>=2` on a whole column. It moves in every row at once,
/// since a row moved alone leaves a gap or an overlap and is stopped for that alone.
fn boundaries(set: &[usize], tables: &[&Table], c: &Checked, lines: &Lines, out: &mut Vec<Change>) {
    let mut columns: Vec<&str> = Vec::new();
    for &ti in set {
        for (col, _) in &tables[ti].inputs {
            if !columns.contains(&col.as_str()) {
                columns.push(col);
            }
        }
    }
    'column: for col in columns {
        let Some(ty) = c.ty_of(col) else { continue };
        let ranks: Option<&Vec<String>> = c.ordered_enum(&ty).and_then(|e| c.enums.get(e));
        let step = if ranks.is_some() { Rat::int(1) } else { c.value_step(col) };
        let mut cuts: Vec<(Rat, Vec<Written>)> = Vec::new();
        let mut points: Vec<Rat> = Vec::new();
        for &ti in set {
            let t = tables[ti];
            let Some(ci) = t.inputs.iter().position(|(n, _)| n == col) else { continue };
            for row in &t.rows {
                let (Some(cell), Some(span)) = (row.cells.get(ci), row.cell_spans.get(ci)) else { continue };
                match cell {
                    Cell::Cmp(_) => {
                        let Ok(toks) = lex_line(1, lines.of(span)) else { continue 'column };
                        for pair in toks.chunks(2) {
                            let [o, l] = pair else { continue 'column };
                            let op = match o.kind {
                                Tok::Le => CmpOp::Le,
                                Tok::Ge => CmpOp::Ge,
                                Tok::Lt => CmpOp::Lt,
                                Tok::Gt => CmpOp::Gt,
                                _ => continue 'column,
                            };
                            let v = match (&l.kind, ranks) {
                                (Tok::Ident(w), Some(r)) => r.iter().position(|x| x == w).map(|p| Rat::int(p as i128)),
                                (Tok::Num(n), None) => crate::types::lit_value_in_pub(n, &ty),
                                (Tok::Date(y, m, d), None) => Some(crate::types::date_ord(*y, *m, *d)),
                                _ => None,
                            };
                            let Some(v) = v else { continue 'column };
                            let cut = match op {
                                CmpOp::Lt | CmpOp::Ge => v,
                                CmpOp::Le | CmpOp::Gt => v.add(step),
                            };
                            let at = lines.offset(span) + l.span.col;
                            let text = lines.text[at..at + l.span.len].to_string();
                            let w = Written { at, len: l.span.len, op, text, table: ti, row: row.index, col: ci, when: when_text(t, row, lines), pad: !t.clause };
                            match cuts.iter_mut().find(|(k, _)| *k == cut) {
                                Some((_, ws)) => ws.push(w),
                                None => cuts.push((cut, vec![w])),
                            }
                        }
                    }
                    Cell::Lit(l) => points.extend(lit_value(l, &ty)),
                    Cell::Set(ls) => points.extend(ls.iter().filter_map(|l| lit_value(l, &ty))),
                    _ => {}
                }
            }
        }
        cuts.sort_by(|a, b| a.0.cmp_to(b.0));
        let (lo, hi) = match ranks {
            Some(r) => (Some(Rat::int(0)), Some(Rat::int(r.len() as i128 - 1))),
            None => c.ranges.get(col).copied().unwrap_or((None, None)),
        };
        let places: Vec<Rat> = cuts.iter().map(|(k, _)| *k).collect();
        let mut alt = 0;
        for (cut, written) in &cuts {
            // The edge of the column's range lies between no two rows: moving it can only open a hole.
            if lo.is_some_and(|l| cut.cmp_to(l.add(step)) == Ordering::Less) || hi.is_some_and(|h| cut.cmp_to(h) == Ordering::Greater) {
                continue;
            }
            // A point of the column stands against this boundary, and no move leaves it covered once.
            if points.iter().any(|p| *p == *cut || p.add(step) == *cut) {
                continue;
            }
            let mut to: Vec<Rat> = vec![cut.sub(step), cut.add(step)];
            to.extend(places.iter().copied());
            let mut tried: Vec<Rat> = vec![*cut];
            for new in to {
                if tried.contains(&new) {
                    continue;
                }
                tried.push(new);
                if lo.is_some_and(|l| new.cmp_to(l.add(step)) == Ordering::Less) || hi.is_some_and(|h| new.cmp_to(h) == Ordering::Greater) {
                    continue;
                }
                let (a, b) = if new.cmp_to(*cut) == Ordering::Less { (new, *cut) } else { (*cut, new) };
                if points.iter().any(|p| p.cmp_to(a) != Ordering::Less && p.cmp_to(b) == Ordering::Less) {
                    continue;
                }
                let Some(edits) = moved(written, new, step, &ty, ranks, lines) else { continue };
                let from = spelled(written.iter().map(|w| format!("{}{}", w.op.word(), w.text)));
                let to_text = spelled(written.iter().zip(&edits).map(|(w, e)| format!("{}{}", w.op.word(), e.text.trim_end())));
                let first = &written[0];
                out.push(Change {
                    table: tables[first.table].name.as_ref().map(|n| n.text.clone()).unwrap_or_default(),
                    clause: tables[first.table].clause,
                    row: first.row,
                    kind: Move::Boundary,
                    output: None,
                    column: Some(col.to_string()),
                    from,
                    to: Some(to_text),
                    when: first.when.clone(),
                    cells: 0,
                    edits,
                    order: (first.table, first.row, first.col, alt),
                });
                alt += 1;
            }
        }
    }
}

/// The first of each way of writing, in the order met.
fn spelled(it: impl Iterator<Item = String>) -> String {
    let mut seen: Vec<String> = Vec::new();
    for s in it {
        if !seen.contains(&s) {
            seen.push(s);
        }
    }
    seen.join(", ")
}

/// The boundary's literals, written at `new`.
fn moved(written: &[Written], new: Rat, step: Rat, ty: &Ty, ranks: Option<&Vec<String>>, lines: &Lines) -> Option<Vec<Edit>> {
    written
        .iter()
        .map(|w| {
            let v = match w.op {
                CmpOp::Lt | CmpOp::Ge => new,
                CmpOp::Le | CmpOp::Gt => new.sub(step),
            };
            let text = match ranks {
                Some(r) => r.get(usize::try_from(v.num / v.den).ok()?)?.clone(),
                None => value_text(v, ty)?,
            };
            Some(lines.put(w.at, w.len, &text, w.pad))
        })
        .collect()
}

/// Every change, in the order of the tables, their rows and their columns.
fn all_changes(text: &str, raw: &RuleFile, f: &RuleFile, c: &Checked) -> Vec<Change> {
    let lines = Lines::new(text);
    let tables: Vec<&Table> = raw.items.iter().filter_map(|i| if let Item::Table(t) = i { Some(t) } else { None }).collect();
    // What each output's cells write, across the tables that define it.
    let mut seen: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for t in &tables {
        for row in &t.rows {
            for (o, s) in t.outputs.iter().zip(&row.out_spans) {
                let e = seen.entry(o.name.text.as_str()).or_default();
                let x = lines.of(s).to_string();
                if !e.contains(&x) {
                    e.push(x);
                }
            }
        }
    }
    let mut out: Vec<Change> = Vec::new();
    for (ti, t) in tables.iter().enumerate() {
        let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        for row in &t.rows {
            let when = when_text(t, row, &lines);
            for (k, (o, cell)) in t.outputs.iter().zip(&row.outs).enumerate() {
                let Some(span) = row.out_spans.get(k) else { continue };
                let cur = lines.of(span);
                let others = seen.get(o.name.text.as_str()).map_or(&[][..], |v| v.as_slice());
                for (alt, v) in alternatives(cur, cell, &o.name.text, f, c, others).into_iter().enumerate() {
                    let edit = lines.put(lines.offset(span), span.len, &v, !t.clause);
                    out.push(Change {
                        table: name.clone(),
                        clause: t.clause,
                        row: row.index,
                        kind: Move::Output,
                        output: Some(o.name.text.clone()),
                        column: None,
                        from: cur.to_string(),
                        to: Some(v),
                        when: when.clone(),
                        cells: 1,
                        edits: vec![edit],
                        order: (ti, row.index, t.inputs.len() + k, alt),
                    });
                }
            }
            // A clause, or a table of one row, goes whole: a table with no rows does not read.
            let whole = t.clause || t.rows.len() == 1;
            let edit = if whole {
                let mut to = row.out_spans.iter().map(|s| s.line).chain([row.span.line]).max().unwrap_or(row.span.line);
                // A clause's `overrides` line comes after its `then`.
                while lines.line(to + 1).split_whitespace().next() == Some(crate::kw::OVERRIDES) {
                    to += 1;
                }
                lines.cut(t.span.line, to)
            } else {
                Edit { at: lines.offset(&Span::new(row.span.line, 0, 0)), end: lines.line_end(row.span.line), text: String::new() }
            };
            out.push(Change {
                table: name.clone(),
                clause: t.clause,
                row: row.index,
                kind: Move::Row,
                output: None,
                column: None,
                from: lines.line(row.span.line).split('#').next().unwrap_or("").trim().to_string(),
                to: None,
                when,
                cells: t.outputs.len(),
                edits: vec![edit],
                order: (ti, row.index, t.inputs.len() + t.outputs.len(), 0),
            });
        }
    }
    for set in definition_sets(&tables) {
        boundaries(&set, &tables, c, &lines, &mut out);
    }
    out.sort_by_key(|ch| ch.order);
    out
}

// ---------------------------------------------------------------------------------------
// Who notices

/// What makes two findings one, across versions of a file whose lines and row numbers have moved.
fn key_of(d: &Diag) -> String {
    let title: String = d.title.chars().filter(|c| !c.is_ascii_digit()).collect();
    format!("{}|{title}", d.code)
}

fn keys(ds: &[Diag], severity: Severity) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for d in ds.iter().filter(|d| d.severity == severity && d.code != "E109") {
        *out.entry(key_of(d)).or_insert(0) += 1;
    }
    out
}

/// The codes of the findings `base` does not hold as many of.
fn new_codes(ds: &[Diag], severity: Severity, base: &BTreeMap<String, usize>) -> Vec<&'static str> {
    let mut left = base.clone();
    let mut codes: Vec<&'static str> = Vec::new();
    for d in ds.iter().filter(|d| d.severity == severity && d.code != "E109") {
        let n = left.entry(key_of(d)).or_insert(0);
        if *n > 0 {
            *n -= 1;
        } else {
            codes.push(d.code);
        }
    }
    codes.sort();
    codes.dedup();
    codes
}

/// The file as the changes are made to it, and what it says before any is.
struct Base {
    text: String,
    path: String,
    /// The scenarios and the examples, when they are still in it.
    tests: Vec<Edit>,
    warnings: BTreeMap<String, usize>,
    /// What the file says with them out.
    bare_errors: BTreeMap<String, usize>,
    budget: i64,
}

fn inputs_of(f: &RuleFile) -> Vec<&str> {
    f.inputs.iter().map(|i| i.name.text.as_str()).chain(f.elements.iter().map(|e| e.name.text.as_str())).collect()
}

/// The inputs of one call, as `trace_lines` writes them: the state first, what else it carries beside it.
fn call_head(f: &RuleFile, c: &Checked, inputs: &BTreeMap<String, Val>) -> String {
    let carried = f.machine.as_ref().and_then(|m| m.carried()).map(|(i, _)| i.to_string());
    let more: Vec<String> = f.machine.as_ref().map(|m| m.more().iter().map(|(i, _)| i.to_string()).collect()).unwrap_or_default();
    let show = |n: &str, v: &Val| format!("{n} = {}", crate::machine::human(c, n, v));
    let args: Vec<String> = inputs_of(f)
        .into_iter()
        .filter(|n| carried.as_deref() != Some(*n) && !more.iter().any(|m| m == n))
        .filter_map(|n| inputs.get(n).map(|v| show(n, v)))
        .collect();
    let Some(state) = carried.as_ref().and_then(|n| inputs.get(n).map(|v| crate::machine::human(c, n, v))) else {
        return args.join(", ");
    };
    let held: Vec<String> = more.iter().filter_map(|n| inputs.get(n).map(|v| show(n, v))).collect();
    let from = if held.is_empty() { state } else { tr!("{}（{}）", "{} ({})", state, held.join(", ")) };
    match (args.is_empty(), crate::i18n::ja()) {
        (true, true) => format!("{from} のとき"),
        (true, false) => format!("at {from}"),
        (false, true) => format!("{from} のとき {}", args.join(", ")),
        (false, false) => format!("at {from}, {}", args.join(", ")),
    }
}

fn answers_text(c: &Checked, o: &[(String, Option<Val>)]) -> String {
    let parts: Vec<String> = o
        .iter()
        .map(|(k, v)| match v {
            Some(v) => format!("{k} = {}", crate::machine::human(c, k, v)),
            None => format!("{k} = {}", crate::kw::NONE),
        })
        .collect();
    parts.join(", ")
}

fn inputs_json(c: &Checked, m: &BTreeMap<String, Val>) -> String {
    let mut o = Obj::new();
    for (k, v) in m {
        o = o.raw(k, crate::diag::WVal::json(&crate::eval::wval(c, k, v)));
    }
    o.finish()
}

fn answers_json(c: &Checked, x: &[(String, Option<Val>)]) -> String {
    let mut o = Obj::new();
    for (k, v) in x {
        o = o.raw(
            k,
            match v {
                Some(v) => crate::diag::WVal::json(&crate::eval::wval(c, k, v)),
                None => "null".to_string(),
            },
        );
    }
    o.finish()
}

/// The call `vdiff` found, and the calls that reach it when the file is a machine.
fn evidence_of(f: &RuleFile, c: &Checked, d: &VDiff) -> Option<Evidence> {
    let ch = d.changes.first()?;
    let outs: Vec<String> = ch.outs.iter().map(|(k, a, b)| Obj::new().str("output", k).str("old", a).str("new", b).finish()).collect();
    let call = Obj::new().raw("inputs", inputs_json(c, &ch.witness)).raw("outputs", crate::json::arr(&outs)).finish();
    let moved: Vec<String> = ch.shown.iter().filter(|(_, a, b)| a != b).map(|(k, a, b)| format!("{k} {a} -> {b}")).collect();
    let one = tr!(
        "答えが変わる呼び出し: {}: {}",
        "an answer that changes: {}: {}",
        call_head(f, c, &ch.witness),
        moved.join(", ")
    );
    let Some(m) = &d.machine else {
        return Some(Evidence { lines: vec![one], call, calls: None });
    };
    let steps: Vec<String> = m
        .shortest
        .iter()
        .map(|st| Obj::new().raw("inputs", inputs_json(c, &st.inputs)).raw("old", answers_json(c, &st.old)).raw("new", answers_json(c, &st.new)).finish())
        .collect();
    let calls = Some(crate::json::arr(&steps));
    if m.shortest.is_empty() {
        let note = tr!(
            "（{} から始まる呼び出しの並びでは、ここに着くものを見つけていません）",
            "(no sequence of calls from {} was found that gets there)",
            m.initial.0
        );
        return Some(Evidence { lines: vec![one, note], call, calls });
    }
    if m.shortest.len() > CALLS_SHOWN {
        let note = tr!(
            "（{} 回の呼び出しで着きます。並びは --format json にあります）",
            "(it takes {} calls to get there; --format json has them)",
            m.shortest.len()
        );
        return Some(Evidence { lines: vec![one, note], call, calls });
    }
    let mut lines = vec![tr!("手順（{} から）:", "Calls (from {}):", m.initial.0)];
    let last = m.shortest.len() - 1;
    for (k, st) in m.shortest.iter().enumerate() {
        let n = k + 1;
        let (head, answers) = (call_head(f, c, &st.inputs), answers_text(c, &st.old));
        if k < last {
            lines.push(format!("  {n}. {head} → {answers}"));
            continue;
        }
        let parted: Vec<(String, Option<Val>)> = st.new.iter().filter(|(k, v)| st.old.iter().find(|(o, _)| o == k).map(|(_, w)| w) != Some(v)).cloned().collect();
        lines.push(format!("  {n}. {head} → {answers}"));
        lines.push(tr!("     変えると {}", "     with this change, {}", answers_text(c, &parted)));
    }
    Some(Evidence { lines, call, calls })
}

/// What one change comes to.
fn judge(b: &Base, p: &(RuleFile, Checked), ch: &Change) -> Outcome {
    let refs: Vec<&Edit> = ch.edits.iter().collect();
    let text = apply(&b.text, &refs);
    let mut compared: Option<(RuleFile, Checked, VDiff)> = None;
    if let Ok((nf, nc)) = crate::prepare_lenient(&text, &b.path) {
        let budget = (b.budget.max(0) as usize / crate::machine::NODES_PER_CELL).max(1);
        let mut d = vdiff::diff_cells((&p.0, &p.1), (&nf, &nc), budget);
        if d.changes.is_empty() && !d.total() {
            vdiff::settle_by_walking((&p.0, &p.1), (&nf, &nc), &mut d);
        }
        if d.changes.is_empty() && d.total() {
            return Outcome { class: Class::Same, by: Vec::new(), evidence: None };
        }
        compared = Some((nf, nc, d));
    }
    let r = crate::report_with(&text, &b.path, b.budget);
    let mut errors: Vec<&'static str> = r.diags.iter().filter(|d| d.severity == Severity::Error && d.code != "E109").map(|d| d.code).collect();
    errors.sort();
    errors.dedup();
    let (class, by) = if !errors.is_empty() {
        let class = if b.tests.is_empty() {
            Class::Claims
        } else {
            let mut all = refs.clone();
            all.extend(b.tests.iter());
            let bare = crate::report_with(&apply(&b.text, &all), &b.path, b.budget);
            if new_codes(&bare.diags, Severity::Error, &b.bare_errors).is_empty() { Class::Tests } else { Class::Claims }
        };
        (class, errors)
    } else {
        let codes = new_codes(&r.diags, Severity::Warning, &b.warnings);
        (if codes.is_empty() { Class::Nothing } else { Class::Warnings }, codes)
    };
    let mut evidence = None;
    if let Some((nf, nc, mut d)) = compared {
        if class == Class::Nothing {
            let budget = (b.budget.max(0) as usize / crate::machine::NODES_PER_CELL).max(1);
            d.machine = vdiff::machine_diff((&p.0, &p.1), (&nf, &nc), &d, budget);
        }
        evidence = evidence_of(&p.0, &p.1, &d);
    }
    Outcome { class, by, evidence }
}

/// Every change on as many threads as there are, each with what the run's thread checks with: the
/// port of dates, the language, and the file's place among the contributions. The answers come back
/// in the changes' order, however the threads took them.
fn judge_all(b: &Base, changes: &[Change]) -> Vec<Outcome> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(changes.len().max(1));
    let next = AtomicUsize::new(0);
    let worker = || -> Vec<(usize, Outcome)> {
        let Ok(p) = crate::prepare_lenient(&b.text, &b.path) else { return Vec::new() };
        let mut out = Vec::new();
        loop {
            let i = next.fetch_add(1, atomic::Ordering::Relaxed);
            let Some(ch) = changes.get(i) else { break };
            out.push((i, judge(b, &p, ch)));
        }
        out
    };
    let port = crate::days::port();
    let lang = crate::i18n::current();
    let set = [b.path.clone()];
    let mut all: Vec<(usize, Outcome)> = std::thread::scope(|s| {
        let mut handles = Vec::new();
        for _ in 1..threads {
            let (port, worker, set) = (port.clone(), &worker, &set);
            let spawned = std::thread::Builder::new().stack_size(STACK).spawn_scoped(s, move || crate::days::with(port, || crate::i18n::with(lang, || crate::link::with_set(set, worker))));
            // Where no thread can be started, this one does the work.
            if let Ok(h) = spawned {
                handles.push(h);
            }
        }
        let mut all = worker();
        for h in handles {
            match h.join() {
                Ok(v) => all.extend(v),
                Err(e) => std::panic::resume_unwind(e),
            }
        }
        all
    });
    all.sort_by_key(|(i, _)| *i);
    all.into_iter().map(|(_, o)| o).collect()
}

/// Piles up the changes nobody notices, each checked on top of the last, and keeps those that still
/// pass: a table that keeps everything the file says.
fn another(b: &Base, raw: &RuleFile, results: &[(Change, Outcome)]) -> Other {
    let of = raw.items.iter().map(|i| if let Item::Table(t) = i { t.rows.len() * t.outputs.len() } else { 0 }).sum();
    let mut other = Other { of, ..Other::default() };
    let unnoticed = |k: Move| results.iter().filter(move |(ch, o)| o.class == Class::Nothing && ch.kind == k).map(|(ch, _)| ch);
    let mut kept: Vec<&Edit> = Vec::new();
    // The cells first: they are what the count is of. A row with a changed cell stays in the table.
    for ch in unnoticed(Move::Output).chain(unnoticed(Move::Boundary)).chain(unnoticed(Move::Row)) {
        if ch.edits.iter().any(|e| kept.iter().any(|k| k.meets(e))) {
            continue;
        }
        let mut all = kept.clone();
        all.extend(ch.edits.iter());
        let r = crate::report_with(&apply(&b.text, &all), &b.path, b.budget);
        if crate::has_error(&r.diags) || !new_codes(&r.diags, Severity::Warning, &b.warnings).is_empty() {
            continue;
        }
        kept = all;
        other.cells += ch.cells;
        match ch.kind {
            Move::Output => {}
            Move::Row => other.rows += 1,
            Move::Boundary => other.boundaries += 1,
        }
    }
    if !kept.is_empty() {
        other.text = Some(apply(&b.text, &kept));
    }
    other
}

/// `rulec strength`: the file, as it is or with its scenarios and examples out, under every change.
pub fn measure(src: &str, path: &str, o: &Options) -> Result<Strength, Refused> {
    let not_passing = |ds: Vec<Diag>, text: &str| {
        let errors: Vec<Diag> = ds.into_iter().filter(|d| d.severity == Severity::Error).collect();
        Refused::NotPassing(errors, text.to_string())
    };
    let first = crate::report_with(src, path, o.budget);
    if crate::has_error(&first.diags) {
        return Err(not_passing(first.diags, src));
    }
    let parsed = crate::parse::parse(src, path);
    let Some(file) = parsed.file else { return Err(Refused::Said(tr!("`{path}` を読めません", "cannot read `{path}`"))) };
    let tests = test_blocks(src, &file);
    let test_refs: Vec<&Edit> = tests.iter().collect();
    let bare_text = apply(src, &test_refs);
    let bare = crate::report_with(&bare_text, path, o.budget);
    let (text, raw, tests, warnings, bare_errors) = if o.claims_only {
        if crate::has_error(&bare.diags) {
            return Err(Refused::NotPassing(bare.diags.into_iter().filter(|d| d.severity == Severity::Error).collect(), bare_text));
        }
        let parsed = crate::parse::parse(&bare_text, path);
        let Some(raw) = parsed.file else { return Err(Refused::Said(tr!("`{path}` を読めません", "cannot read `{path}`"))) };
        let warnings = keys(&bare.diags, Severity::Warning);
        (bare_text, raw, Vec::new(), warnings, BTreeMap::new())
    } else {
        let warnings = keys(&first.diags, Severity::Warning);
        let errors = keys(&bare.diags, Severity::Error);
        (src.to_string(), file, tests, warnings, errors)
    };
    let (f, c) = crate::prepare_lenient(&text, path).map_err(|_| Refused::Said(tr!("`{path}` は検査を通っていません", "`{path}` does not pass check")))?;
    let changes = all_changes(&text, &raw, &f, &c);
    if changes.len() > MANY {
        eprintln!(
            "{}",
            tr!(
                "note {path}: 変更は {} 件あります。一件ごとに検査を一回走らせるので時間がかかります。",
                "note {path}: there are {} changes to check, one check each; this will take a while.",
                changes.len()
            )
        );
    }
    let b = Base { text, path: path.to_string(), tests, warnings, bare_errors, budget: o.budget };
    let outcomes = judge_all(&b, &changes);
    let results: Vec<(Change, Outcome)> = changes.into_iter().zip(outcomes).collect();
    let other = another(&b, &raw, &results);
    // The table that is handed over is checked again from the beginning, where it is going to live.
    if let Some(t) = &other.text {
        let at = o.other.as_deref().unwrap_or(path);
        let r = crate::report_with(t, at, o.budget);
        if crate::has_error(&r.diags) {
            let mut codes: Vec<&str> = r.diags.iter().filter(|d| d.severity == Severity::Error).map(|d| d.code).collect();
            codes.sort();
            codes.dedup();
            let codes = codes.join(", ");
            return Err(Refused::Said(tr!(
                "別の表は `{at}` では検査を通りません（{codes}）。読み込むファイルがあるなら、`{path}` と同じディレクトリに書いてください",
                "the other table does not pass check at `{at}` ({codes}); if the file reads other files, write it in the directory of `{path}`"
            )));
        }
    }
    Ok(Strength { file: path.to_string(), claims_only: o.claims_only, results, other })
}

// ---------------------------------------------------------------------------------------
// What it says

fn where_text(ch: &Change) -> String {
    let (ja, when) = (crate::i18n::ja(), &ch.when);
    let head = match (ch.clause, ch.kind, ja) {
        (true, Move::Boundary, true) => format!("節 {} の列 {}", ch.table, ch.column.as_deref().unwrap_or("")),
        (true, Move::Boundary, false) => format!("clause {} column {}", ch.table, ch.column.as_deref().unwrap_or("")),
        (false, Move::Boundary, true) => format!("表 {} の列 {}", ch.table, ch.column.as_deref().unwrap_or("")),
        (false, Move::Boundary, false) => format!("table {} column {}", ch.table, ch.column.as_deref().unwrap_or("")),
        (true, _, true) => format!("節 {}", ch.table),
        (true, _, false) => format!("clause {}", ch.table),
        (false, _, true) => format!("表 {} の {} 行目", ch.table, ch.row),
        (false, _, false) => format!("table {} row {}", ch.table, ch.row),
    };
    if ch.kind == Move::Boundary {
        return head;
    }
    if when.is_empty() {
        return head;
    }
    if ja { format!("{head}（{when}）") } else { format!("{head} ({when})") }
}

/// One line for the changes of one place, however many values it can take: `a -> b | c`.
fn change_text(ch: &Change, to: &str) -> String {
    let place = where_text(ch);
    match ch.kind {
        Move::Output => format!("{place}: {} {} -> {to}", ch.output.as_deref().unwrap_or(""), ch.from),
        Move::Row if ch.clause => tr!("{place}: 節を消す", "{place}: the clause taken out"),
        Move::Row => tr!("{place}: 行を消す", "{place}: the row taken out"),
        Move::Boundary => tr!("{place}: 境目 {} -> {to}", "{place}: the boundary {} -> {to}", ch.from),
    }
}

/// Whether two changes are of one place and differ only in what they change it to.
fn same_place(a: &Change, b: &Change) -> bool {
    (a.order.0, a.row, a.kind, &a.output, &a.column, &a.from) == (b.order.0, b.row, b.kind, &b.output, &b.column, &b.from)
}

/// What `rulec strength` prints. `shown` is where the other table went, when it was written.
pub fn render(s: &Strength, shown: Option<&str>) -> String {
    let path = &s.file;
    let (total, nobody) = (s.changes(), s.nothing());
    let mut out = String::new();
    let scope = if s.claims_only { tr!("（シナリオと例を抜いて）", " (its scenarios and examples taken out)") } else { String::new() };
    if total == 0 && s.results.is_empty() {
        out.push_str(&tr!("{path}{scope}: 変えられるものがありません。\n", "{path}{scope}: there is nothing to change.\n"));
        return out;
    }
    if total == 0 {
        let n = s.results.len();
        out.push_str(&tr!("{path}{scope}: 変更 {n} 件は、どれも答えを変えません。\n", "{path}{scope}: none of {n} changes to the rule moves an answer.\n"));
    } else if nobody == 0 {
        out.push_str(&tr!("{path}{scope}: 規則への変更 {total} 件は、どれも誰かが気づきます。\n", "{path}{scope}: every one of {total} changes to the rule is noticed.\n"));
    } else {
        out.push_str(&tr!(
            "{path}{scope}: 規則への変更 {total} 件のうち {nobody} 件は、誰も気づきません。\n",
            "{path}{scope}: {nobody} of {total} changes to the rule are noticed by nothing.\n"
        ));
    }
    let same = s.count(Class::Same);
    let width = [s.count(Class::Claims), s.count(Class::Tests), s.count(Class::Warnings), nobody, same].iter().map(|n| n.to_string().len()).max().unwrap_or(1);
    let mut line = |n: usize, what: String| out.push_str(&format!("  {n:>width$} {what}\n"));
    line(s.count(Class::Claims), tr!("件は主張と検査が気づく", "noticed by the claims and checks"));
    if !s.claims_only {
        line(s.count(Class::Tests), tr!("件はシナリオか例だけが気づく", "noticed only by a scenario or an example"));
    }
    line(s.count(Class::Warnings), tr!("件は警告だけが気づく", "noticed only by a warning"));
    line(nobody, tr!("件は誰も気づかない", "noticed by nothing"));
    line(same, tr!("件は答えが変わらない", "change no answer"));
    if nobody > 0 {
        out.push('\n');
        out.push_str(&tr!("誰も気づかないもの:\n", "Noticed by nothing:\n"));
        let unnoticed: Vec<&(Change, Outcome)> = s.results.iter().filter(|(_, o)| o.class == Class::Nothing).collect();
        // One place, however many values nobody minds there, with the call of the first of them.
        for group in unnoticed.chunk_by(|a, b| same_place(&a.0, &b.0)) {
            let tos: Vec<&str> = group.iter().filter_map(|(ch, _)| ch.to.as_deref()).collect();
            let (ch, o) = group[0];
            out.push_str(&format!("  {}\n", change_text(ch, &tos.join(" | "))));
            match &o.evidence {
                Some(e) => {
                    for l in &e.lines {
                        out.push_str(&format!("    {l}\n"));
                    }
                }
                None => out.push_str(&format!(
                    "    {}\n",
                    tr!("答えが変わる呼び出しは出せていません（比べきれませんでした）", "no call that shows it was worked out (the two could not be compared all the way)")
                )),
            }
        }
    }
    if s.other.text.is_some() {
        out.push('\n');
        let o = &s.other;
        let (cells, of) = (o.cells, o.of);
        let also = match (o.rows, o.boundaries, crate::i18n::ja()) {
            (0, 0, _) => String::new(),
            (r, 0, true) => format!("消した行は {r} 行です。"),
            (0, k, true) => format!("動かした境目は {k} か所です。"),
            (r, k, true) => format!("消した行は {r} 行、動かした境目は {k} か所です。"),
            (1, 0, false) => " One row is taken out.".to_string(),
            (r, 0, false) => format!(" {r} rows are taken out."),
            (0, 1, false) => " One boundary is moved.".to_string(),
            (0, k, false) => format!(" {k} boundaries are moved."),
            (r, k, false) => format!(" {r} {} taken out and {k} {} moved.", if r == 1 { "row is" } else { "rows are" }, if k == 1 { "boundary is" } else { "boundaries are" }),
        };
        out.push_str(&tr!(
            "ファイルに書いてあることを全部守る別の表があります。出力のセル {of} 個のうち、少なくとも {cells} 個が違います。{also}\n",
            "Another table keeps everything the file says and differs in at least {cells} of {of} output cells.{also}\n"
        ));
        out.push_str(&tr!(
            "  一緒に変えて初めて通る組み合わせは探していないので、これが一番違う表だとは言えません。\n",
            "  Changes that pass only together were not looked for, so this is not said to be the most different one.\n"
        ));
        match shown {
            Some(w) => out.push_str(&tr!(
                "  `{w}` に書きました。`rulec check {w}` が通り、`rulec diff {path} {w}` で元と比べられます。\n",
                "  It is written to `{w}`: `rulec check {w}` passes, and `rulec diff {path} {w}` shows where the two part.\n"
            )),
            None => {
                let flag = if s.claims_only { " --claims-only" } else { "" };
                out.push_str(&tr!("  rulec strength {path}{flag} --other other.rule で書き出します。\n", "  rulec strength {path}{flag} --other other.rule writes it.\n"));
            }
        }
    }
    out
}

fn item_json(ch: &Change, o: &Outcome, with_by: bool) -> String {
    let (call, calls) = match &o.evidence {
        Some(e) => (e.call.clone(), e.calls.clone().unwrap_or_else(|| "null".to_string())),
        None => ("null".to_string(), "null".to_string()),
    };
    let mut j = Obj::new()
        .str("table", &ch.table)
        .bool("clause", ch.clause)
        .int("row", ch.row as i128)
        .str("kind", ch.kind.word())
        .raw("output", ch.output.as_ref().map_or("null".to_string(), |x| crate::json::quote(x)))
        .raw("column", ch.column.as_ref().map_or("null".to_string(), |x| crate::json::quote(x)))
        .str("from", &ch.from)
        .raw("to", ch.to.as_ref().map_or("null".to_string(), |x| crate::json::quote(x)))
        .str("when", &ch.when);
    if with_by {
        let class = match o.class {
            Class::Claims => "claims",
            Class::Tests => "tests",
            _ => "warnings",
        };
        j = j.str("class", class).raw("by", crate::json::strs(&o.by));
    }
    j.raw("call", call).raw("calls", calls).finish()
}

/// `rulec strength --format json`: one object (docs/formats.md).
pub fn render_json(s: &Strength, shown: Option<&str>) -> String {
    let pick = |f: &dyn Fn(&Outcome) -> bool, by: bool| -> String {
        let v: Vec<String> = s.results.iter().filter(|(_, o)| f(o)).map(|(ch, o)| item_json(ch, o, by)).collect();
        crate::json::arr(&v)
    };
    let other = Obj::new()
        .int("cells", s.other.cells as i128)
        .int("of", s.other.of as i128)
        .int("rows", s.other.rows as i128)
        .int("boundaries", s.other.boundaries as i128)
        .raw("written", shown.map_or("null".to_string(), crate::json::quote))
        .finish();
    Obj::new()
        .str("file", &s.file)
        .bool("claims_only", s.claims_only)
        .int("changes", s.changes() as i128)
        .int("claims", s.count(Class::Claims) as i128)
        .int("tests", s.count(Class::Tests) as i128)
        .int("warnings", s.count(Class::Warnings) as i128)
        .int("nothing", s.nothing() as i128)
        .int("same_answers", s.count(Class::Same) as i128)
        .raw("unnoticed", pick(&|o| o.class == Class::Nothing, false))
        .raw("noticed", pick(&|o| matches!(o.class, Class::Claims | Class::Tests | Class::Warnings), true))
        .raw("same", pick(&|o| o.class == Class::Same, false))
        .raw("other", other)
        .finish()
}
