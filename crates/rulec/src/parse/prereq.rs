//! The `prerequisites` block (§15.222): its heading and the lines under it, read by shape alone.
//!
//! ```text
//! prerequisites research over tech
//!   start            agriculture
//!   pottery          requires agriculture
//!   the_wheel        requires animal_husbandry, archery
//!   charge           requires shock_2 or drill_2
//!   retired_tech     requires nothing "dropped from the tree"
//!   ordered by era_of
//! ```
//!
//! Every word of the block is read by its place in the line, so none is reserved: a line whose
//! second word is `requires` is a value's own whatever the value is called (`start requires …`
//! is the line of a value named `start`). Whether the names are values of the enum, and the rest
//! of what the block says, is `crate::prereq`'s.

use super::{P, span_of};
use crate::ast::{Name, Needs, PrereqDecl, RequiresLine};
use crate::diag::{Diag, Span};
use crate::lex::{Kind, Token};

/// How the block is written, for the notes of E071.
fn shape() -> String {
    tr!(
        "形は `prerequisites <名前>(<別名>) over <列挙>` の行と、その下に `start <値>, …`、値ごとに一行の `<値> requires <前提>, …`（`or` でどれか一つ）、`<値> requires nothing \"<理由>\"`、`ordered by <表>` の行です。",
        "The shape is `prerequisites <name>(<alias>) over <enum>`, with under it `start <value>, …`, one `<value> requires <prerequisite>, …` line per value (`or` for any one of), `<value> requires nothing \"<reason>\"`, and `ordered by <table>`."
    )
}

fn name_of(t: &Token, w: &str) -> Name {
    Name { text: w.to_string(), ascii: None, span: t.span.clone() }
}

impl P {
    /// `prerequisites <name>(<alias>) over <enum> [open]` and the block under it, up to the next
    /// blank line. The heading and the block are consumed whatever goes wrong, so that the lines
    /// under a broken heading are not read again as declarations of their own.
    pub(super) fn prerequisites(&mut self, line: &[Token]) -> Option<PrereqDecl> {
        let span = span_of(line);
        self.i += 1;
        let block = self.prereq_lines();
        let end_line = block.last().map(|l| span_of(l).line).unwrap_or(span.line);
        let bad = |p: &mut Self, at: Span, title: String, what: String| {
            p.err(Diag::error("E071", title).at(p.at(at.line)).mark(at, what).note(shape()));
        };
        let heading = tr!("`prerequisites` の見出しの形が違います", "A `prerequisites` heading is not shaped like this");
        let Some((name, k)) = self.name_at(line, 1) else {
            bad(self, span.clone(), heading, tr!("名前がありません", "there is no name"));
            return None;
        };
        let over = match (line.get(k).and_then(|t| t.ident()), line.get(k + 1)) {
            (Some(crate::kw::OVER), Some(t)) => t.ident().map(|w| name_of(t, w)),
            _ => None,
        };
        let Some(over) = over else {
            bad(self, span.clone(), heading, tr!("`over <列挙>` がありません", "`over <enum>` is missing"));
            return None;
        };
        // The `open` mark after the enum (§15.222): syntax here, and what it lets in is the
        // contributions'.
        let mut open = None;
        let mut j = k + 2;
        if line.get(j).and_then(|t| t.ident()) == Some(crate::kw::OPEN) {
            open = Some(line[j].span.clone());
            j += 1;
        }
        if j < line.len() {
            bad(self, span_of(&line[j..]), heading, tr!("余分な語があります", "there are words left over"));
            return None;
        }
        let mut d = PrereqDecl {
            name,
            over: Some(over),
            open,
            start: Vec::new(),
            start_span: None,
            lines: Vec::new(),
            ordered_by: None,
            span: span.clone(),
            end_line,
        };
        let mut ok = true;
        for l in &block {
            let sp = span_of(l);
            let first = l.first().and_then(|t| t.ident()).unwrap_or("");
            let second = l.get(1).and_then(|t| t.ident());
            if second == Some(crate::kw::REQUIRES) {
                match self.requires_line(l) {
                    Some(r) => d.lines.push(r),
                    None => ok = false,
                }
            } else if first == crate::kw::START {
                let values = list(&l[1..]);
                match values {
                    Some(vs) if d.start_span.is_none() => {
                        d.start = vs;
                        d.start_span = Some(sp);
                    }
                    Some(_) => {
                        bad(
                            self,
                            sp,
                            tr!("`prerequisites` の中に `start` の行が二行あります", "There are two `start` lines under `prerequisites`"),
                            tr!("二行目です。値は一行に `,` で並べてください", "this is the second; list the values on one line, with `,` between them"),
                        );
                        ok = false;
                    }
                    None => {
                        bad(
                            self,
                            sp,
                            tr!("`start` の行の形が違います", "A `start` line is not shaped like this"),
                            tr!("`start <値>, <値>, …` の形で、値を一つ以上書いてください", "write one value or more, as `start <value>, <value>, …`"),
                        );
                        ok = false;
                    }
                }
            } else if first == crate::kw::ORDERED && second == Some(crate::kw::BY) {
                match (l.get(2), l.len()) {
                    (Some(t), 3) if t.ident().is_some() && d.ordered_by.is_none() => {
                        d.ordered_by = Some((name_of(t, t.ident().unwrap_or("")), sp));
                    }
                    (Some(t), 3) if t.ident().is_some() => {
                        bad(
                            self,
                            sp,
                            tr!("`prerequisites` の中に `ordered by` の行が二行あります", "There are two `ordered by` lines under `prerequisites`"),
                            tr!("二行目です", "this is the second"),
                        );
                        ok = false;
                    }
                    _ => {
                        bad(
                            self,
                            sp,
                            tr!("`ordered by` の行の形が違います", "An `ordered by` line is not shaped like this"),
                            tr!("`ordered by <表>` の形で、表の名前を一つ書いてください", "write one table's name, as `ordered by <table>`"),
                        );
                        ok = false;
                    }
                }
            } else {
                bad(
                    self,
                    sp,
                    tr!("`prerequisites` の中の行を読めません", "A line under `prerequisites` cannot be read"),
                    tr!("`start`、`<値> requires …`、`ordered by` のどれでもありません", "it is none of `start`, `<value> requires …` and `ordered by`"),
                );
                ok = false;
            }
        }
        ok.then_some(d)
    }

    /// The lines under a `prerequisites` heading, up to the next blank line. A line head ends the
    /// block too, where a blank line was left out — but not on a value's own line: `table requires
    /// …` is the line of a value named `table`.
    fn prereq_lines(&mut self) -> Vec<Vec<Token>> {
        let mut out = Vec::new();
        while let Some(line) = self.cur() {
            if line.is_empty() || line.first().is_some_and(|t| t.is(&Kind::Pipe)) {
                break;
            }
            let w = line.first().and_then(|t| t.ident()).unwrap_or("");
            let own = line.get(1).and_then(|t| t.ident()) == Some(crate::kw::REQUIRES);
            if !own && super::KEYWORDS.contains(&w) {
                break;
            }
            out.push(line.clone());
            self.i += 1;
        }
        out
    }

    /// `<value> requires <prerequisite>, …`, the groups split at `,` and the values of one group
    /// at `or`; or `<value> requires nothing "<reason>"`. `nothing` is the word only right after
    /// `requires` and before a string: anywhere else it is a value's name.
    pub(super) fn requires_line(&mut self, l: &[Token]) -> Option<RequiresLine> {
        let sp = span_of(l);
        let bad = |p: &mut Self, at: Span, what: String| {
            p.err(
                Diag::error("E071", tr!("`requires` の行の形が違います", "A `requires` line is not shaped like this"))
                    .at(p.at(sp.line))
                    .mark(at, what)
                    .note(shape()),
            );
        };
        let Some(v) = l.first().and_then(|t| t.ident()) else {
            bad(self, sp.clone(), tr!("`requires` の前に値がありません", "there is no value before `requires`"));
            return None;
        };
        let value = name_of(&l[0], v);
        let rest = &l[2..];
        if rest.is_empty() {
            bad(
                self,
                sp.clone(),
                tr!(
                    "`requires` の後に前提がありません。手に入らない値なら `requires nothing \"<理由>\"` と書いてください",
                    "there is nothing after `requires`; for a value no case can take, write `requires nothing \"<reason>\"`"
                ),
            );
            return None;
        }
        if rest[0].ident() == Some(crate::kw::NOTHING) && rest.get(1).is_some_and(|t| matches!(t.kind, Kind::Str(_))) {
            let Kind::Str(reason) = &rest[1].kind else { return None };
            if rest.len() > 2 {
                bad(self, span_of(&rest[2..]), tr!("理由の後に余分な語があります", "there are words left over after the reason"));
                return None;
            }
            if reason.trim().is_empty() {
                bad(self, rest[1].span.clone(), tr!("理由が空です。なぜ手に入らないのかを書いてください", "the reason is empty; say why no case can take the value"));
                return None;
            }
            return Some(RequiresLine { value, needs: Needs::Nothing { reason: reason.clone(), span: rest[1].span.clone() }, span: sp });
        }
        let mut groups: Vec<Vec<Name>> = Vec::new();
        for part in rest.split(|t| t.is(&Kind::Comma)) {
            let mut group: Vec<Name> = Vec::new();
            for alt in part.split(|t| t.ident() == Some(crate::kw::OR)) {
                match alt {
                    [t] if t.ident().is_some() => group.push(name_of(t, t.ident().unwrap_or(""))),
                    [] => {
                        let at = if part.is_empty() { sp.clone() } else { span_of(part) };
                        bad(self, at, tr!("`,` か `or` の両側に値が要ります", "a value is needed on both sides of `,` and `or`"));
                        return None;
                    }
                    ts => {
                        bad(self, span_of(ts), tr!("値の名前を一つずつ書いてください", "write the values one name at a time"));
                        return None;
                    }
                }
            }
            groups.push(group);
        }
        Some(RequiresLine { value, needs: Needs::Groups(groups), span: sp })
    }
}

/// `a, b, c`: the names of a list, or `None` when it is empty or is not names between commas.
fn list(ts: &[Token]) -> Option<Vec<Name>> {
    let mut out = Vec::new();
    for part in ts.split(|t| t.is(&Kind::Comma)) {
        match part {
            [t] => out.push(name_of(t, t.ident()?)),
            _ => return None,
        }
    }
    (!out.is_empty()).then_some(out)
}
