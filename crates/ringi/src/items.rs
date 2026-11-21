//! What a `.ringi` holds, by kind and name (DESIGN 9.7): what ritsu's port `Items` hands yuen and
//! the index, and what `ringi api` lists. A thing is the lines it is written on and its definition,
//! the text a change of meaning changes: each line without its comment, its spaces run together, so
//! that lining a table up again changes nothing.
//!
//! | kind | name | what it is |
//! |---|---|---|
//! | `field` | the field (`amount`; a field of rows as `items.price`, as `labels` writes it) | its line, with its options or its rows' fields |
//! | `input` | what a shared route takes | its line |
//! | `table` | `route` for the route's table, or the name of a table | the header and the rows |
//! | `stage` | the stage | in a table, its column (each row's inputs and its cell); on a line of its own, the line and the deadlines under it |
//! | `who` | the way to choose an approver | its line |
//! | `deadline` | `every_stage`, or the stage that writes its own | the lines of the deadline |
//! | `ticket` | the rule (`self_approval`, `send_back`, …), or what an `on` line does and to what (`hold_budget.reserve`, `launch_pay`) | its line; every `on` line that does it |
//! | `claim` | the claim | its line |
//! | `example` | `route` | the examples' table |
//! | `scenario` | the scenario | its lines and its table of events |
//! | `position`, `team_kind` | of an organization | its line |
//! | `master` | of masters | its block |
//!
//! The names are the names as written (`金額`, not the ASCII name after it).

use crate::ast::{Field, File, Route, TicketLine};

/// One thing a file holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thing {
    pub kind: &'static str,
    /// The name as written, which a naming takes.
    pub name: String,
    /// The ASCII name, which the JSON takes (`amount` for `金額(amount)`).
    pub ascii: String,
    /// The first and the last line it is written on, from 1.
    pub lines: (usize, usize),
    /// Its definition.
    pub text: String,
}

/// The kinds, in the order [`things`] gives them.
pub const KINDS: [&str; 13] = ["field", "input", "table", "stage", "who", "deadline", "ticket", "claim", "example", "scenario", "position", "team_kind", "master"];

/// A line without its comment, the spaces outside its strings run together.
pub fn tidy(line: &str) -> String {
    let (mut out, mut in_str, mut esc, mut space) = (String::new(), false, false, false);
    for c in line.chars() {
        if in_str {
            out.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '#' => break,
            c if c.is_whitespace() => space = true,
            c => {
                if space && !out.is_empty() {
                    out.push(' ');
                }
                space = false;
                in_str = c == '"';
                out.push(c);
            }
        }
    }
    out
}

/// The cells of a row of a table, tidied: what is between its bars.
fn cells(row: &str) -> Vec<String> {
    let (mut out, mut cell, mut in_str, mut esc) = (Vec::new(), String::new(), false, false);
    for c in tidy(row).chars() {
        if in_str {
            cell.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '|' => out.push(std::mem::take(&mut cell).trim().to_string()),
            c => {
                in_str = c == '"';
                cell.push(c);
            }
        }
    }
    // what is before the first bar is nothing
    out.into_iter().skip(1).collect()
}

struct Lines<'s> {
    all: Vec<&'s str>,
}

impl Lines<'_> {
    fn indent(l: &str) -> usize {
        l.chars().take_while(|c| c.is_whitespace()).count()
    }

    fn row(l: &str) -> bool {
        l.trim_start().starts_with('|')
    }

    /// The block that starts on `line` (from 1): the lines under it that are written further in,
    /// and the rows of the table it heads. A blank line or a comment does not end it, and is not
    /// its last line.
    fn block(&self, line: usize) -> (usize, usize) {
        let Some(first) = self.all.get(line.wrapping_sub(1)) else { return (line, line) };
        let ind = Self::indent(first);
        let mut last = line;
        for (i, l) in self.all.iter().enumerate().skip(line) {
            if tidy(l).is_empty() {
                continue;
            }
            if Self::indent(l) > ind || Self::row(l) {
                last = i + 1;
            } else {
                break;
            }
        }
        (line, last)
    }

    /// The lines `from` through `to`, tidied, the empty ones left out.
    fn text(&self, (from, to): (usize, usize)) -> String {
        self.all.get(from.saturating_sub(1)..to.min(self.all.len())).unwrap_or_default().iter().map(|l| tidy(l)).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
    }
}

/// The name of a rule of the ticket, and of what an `on` line does: `alias` gives the name the
/// line's book or flow goes by (as written, or in ASCII).
fn ticket_name(l: &TicketLine, alias: &dyn Fn(&str) -> String) -> String {
    match l {
        TicketLine::SelfApproval { .. } => "self_approval".to_string(),
        TicketLine::SameApprover { .. } => "same_approver".to_string(),
        TicketLine::SubmittedBy { .. } => "submitted_by".to_string(),
        TicketLine::SendBack { .. } => "send_back".to_string(),
        TicketLine::Resubmit { .. } => "resubmit".to_string(),
        TicketLine::Withdraw { .. } => "withdraw".to_string(),
        TicketLine::Retract { .. } => "retract".to_string(),
        TicketLine::Pause { .. } => "pause".to_string(),
        TicketLine::ApproversEdit { .. } => "approvers_edit".to_string(),
        TicketLine::CommentRequired { .. } => "comment_required".to_string(),
        TicketLine::On { action, call, .. } => {
            let target: Vec<String> = call.target.iter().enumerate().map(|(i, w)| if i == 0 { alias(&w.text) } else { w.text.clone() }).collect();
            format!("{}_{}", action.word(), target.join("."))
        }
    }
}

/// Everything the file holds, in the order of [`KINDS`], then as written.
pub fn things(f: &File, src: &str) -> Vec<Thing> {
    let ls = Lines { all: src.lines().collect() };
    let mut out: Vec<Thing> = Vec::new();
    let block = |kind: &'static str, name: &str, ascii: &str, line: usize| {
        let b = ls.block(line);
        Thing { kind, name: name.to_string(), ascii: ascii.to_string(), lines: b, text: ls.text(b) }
    };
    let named = |kind: &'static str, n: &crate::ast::Name, line: usize| block(kind, &n.text, n.ascii(), line);
    fn fields(ls: &Lines, under: (&str, &str), fs: &[Field], out: &mut Vec<Thing>) {
        for fd in fs {
            let join = |under: &str, name: &str| if under.is_empty() { name.to_string() } else { format!("{under}.{name}") };
            let (name, ascii) = (join(under.0, &fd.name.text), join(under.1, fd.name.ascii()));
            let b = ls.block(fd.line);
            out.push(Thing { kind: "field", name: name.clone(), ascii: ascii.clone(), lines: b, text: ls.text(b) });
            fields(ls, (&name, &ascii), &fd.children, out);
        }
    }
    fields(&ls, ("", ""), &f.form, &mut out);
    out.extend(f.inputs.iter().map(|i| named("input", &i.name, i.line)));
    // the tables: the route's, then the named ones
    let mut stages = Vec::new();
    let mut deadlines = Vec::new();
    match &f.route {
        Some(Route::Table(t)) => {
            let table = block("table", "route", "route", t.line);
            // the header is the first row of the block
            let rows: Vec<Vec<String>> = (table.lines.0..=table.lines.1).filter_map(|n| ls.all.get(n - 1)).filter(|l| Lines::row(l)).map(|l| cells(l)).collect();
            let inputs = t.inputs.len();
            for (i, s) in t.stages.iter().enumerate() {
                let column: Vec<String> = rows
                    .iter()
                    .filter_map(|r| {
                        let cell = r.get(inputs + i)?;
                        Some(format!("{} : {}", r.iter().take(inputs).cloned().collect::<Vec<_>>().join(" | "), cell.trim_start_matches("->").trim()))
                    })
                    .collect();
                stages.push(Thing { kind: "stage", name: s.text.clone(), ascii: s.ascii().to_string(), lines: table.lines, text: column.join("\n") });
            }
            out.push(table);
        }
        Some(Route::Stages(ss)) => {
            for s in ss {
                let stage = named("stage", &s.name, s.line);
                if s.deadlines.is_some() && stage.lines.1 > s.line {
                    let b = (s.line + 1, stage.lines.1);
                    deadlines.push(Thing { kind: "deadline", name: s.name.text.clone(), ascii: s.name.ascii().to_string(), lines: b, text: ls.text(b) });
                }
                stages.push(stage);
            }
        }
        Some(Route::Shared { .. }) | None => {}
    }
    out.extend(f.tables.iter().map(|t| named("table", &t.name, t.line)));
    out.extend(stages);
    out.extend(f.who.iter().map(|w| named("who", &w.name, w.line)));
    if let Some(d) = &f.every_stage {
        out.push(block("deadline", "every_stage", "every_stage", d.line));
    }
    out.extend(deadlines);
    // the rules of the ticket; the `on` lines that do the same to the same are one thing
    if let Some(t) = &f.ticket {
        let mut rules: Vec<Thing> = Vec::new();
        for l in &t.lines {
            // the book or the flow by its alias, whose ASCII name the JSON takes
            let ascii = |written: &str| f.uses.iter().filter_map(|u| u.alias.as_ref()).find(|a| a.text == written).map(|a| a.ascii().to_string()).unwrap_or_else(|| written.to_string());
            let one = block("ticket", &ticket_name(l, &|w| w.to_string()), &ticket_name(l, &ascii), l.line());
            match rules.iter_mut().find(|r| r.name == one.name) {
                Some(r) => {
                    r.lines = (r.lines.0.min(one.lines.0), r.lines.1.max(one.lines.1));
                    r.text = format!("{}\n{}", r.text, one.text);
                }
                None => rules.push(one),
            }
        }
        out.extend(rules);
    }
    out.extend(f.claims.iter().map(|c| block("claim", &c.name.text, &c.name.text, c.line)));
    if let Some(e) = &f.examples {
        out.push(block("example", "route", "route", e.line));
    }
    out.extend(f.scenarios.iter().map(|s| block("scenario", &s.name.text, &s.name.text, s.line)));
    out.extend(f.positions.iter().map(|p| named("position", &p.name, p.line)));
    out.extend(f.team_kinds.iter().map(|k| named("team_kind", &k.name, k.line)));
    out.extend(f.masters.iter().map(|m| named("master", &m.name, m.line)));
    out
}
