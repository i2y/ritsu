//! The `chances` table (§15.224), read by shape alone.
//!
//! ```text
//! chances
//! | plans_left | escort_fits | -> find | coins | chance |
//! | true       | true        | coins   | -     | 5/14   |
//! | -          | -           | chart   | -     | 1/7    |
//! | -          | false       | escorts | -     | 0      |
//! ```
//!
//! The columns before `->` are the inputs that are not drawn (and, for a rule that walks, the
//! sequences, each cell naming a `sequence`); the columns after it are the outputs a claim is about,
//! each cell a condition like a table's (`gold >=50`, `-`); the last one, `chance`, holds the claim.
//! Which names are inputs, which are drawn, and whether the cells fit their columns is the type
//! check's question (`crate::chances`).

use super::{P, span_of, split_cells};
use crate::ast::{ChanceRow, ChanceTest, ChancesDecl, CmpOp};
use crate::diag::{Diag, Span};
use crate::lex::{Kind, Token};
use crate::num::Rat;

/// How the section is written, for the notes of E070.
pub(crate) fn shape() -> String {
    tr!(
        "形は `chances` の行と、その下の表です。見出しは、乱数でない入力の全部の列、`->`、主張に要る出力の列、最後に `chance` の列です。`chance` のセルには、正確な有理数（`5/14`、`0`、`1`）、率（`35%`）、それとの比べ（`<=1/100`、`>=3/4`）を書きます。",
        "The shape is a `chances` line with a table under it. The header names every input that is not random, then `->`, the outputs the claims are about, and last a `chance` column. A `chance` cell holds an exact rational (`5/14`, `0`, `1`), a rate (`35%`), or comparisons with one (`<=1/100`, `>=3/4`)."
    )
}

/// One value a chance is written with: `5/14`, `0`, `1`, `35%`, `0.25`. `None` when the tokens are
/// not one; the caller says so.
fn ratio(ts: &[Token]) -> Option<Rat> {
    let num = |t: &Token| -> Option<(Option<String>, Rat)> {
        match &t.kind {
            Kind::Num(n) => crate::types::comparable(n),
            _ => None,
        }
    };
    match ts {
        [a] => match num(a)? {
            (None, v) => Some(v),
            (Some(d), v) if d == crate::kw::RATE => Some(v),
            _ => None,
        },
        [a, s, b] if s.is(&Kind::Slash) => match (num(a)?, num(b)?) {
            ((None, x), (None, y)) if x.is_int() && y.is_int() && y.num != 0 => Some(x.div(y)),
            _ => None,
        },
        _ => None,
    }
}

/// The chance cell: a value, or comparisons with values. Each value lies between 0 and 1.
fn chance_test(ts: &[Token]) -> Result<ChanceTest, String> {
    let bad = || {
        tr!(
            "確率として読めません。`5/14`、`0`、`1`、`35%`、`<=1/100`、`>=1/4 <=1/2` のように書いてください",
            "this is not a chance: write it like `5/14`, `0`, `1`, `35%`, `<=1/100` or `>=1/4 <=1/2`"
        )
    };
    let within = |v: Rat| -> Result<Rat, String> {
        if v.num < 0 || v.cmp_to(Rat::int(1)) == std::cmp::Ordering::Greater {
            let v = if v.den == 1 { format!("{}", v.num) } else { format!("{}/{}", v.num, v.den) };
            Err(tr!("確率は 0 から 1 のあいだです（{v} はその外です）", "a chance lies between 0 and 1 ({v} does not)"))
        } else {
            Ok(v)
        }
    };
    let op_of = |t: &Token| match t.kind {
        Kind::Le => Some(CmpOp::Le),
        Kind::Ge => Some(CmpOp::Ge),
        Kind::Lt => Some(CmpOp::Lt),
        Kind::Gt => Some(CmpOp::Gt),
        _ => None,
    };
    if ts.first().and_then(op_of).is_none() {
        return ratio(ts).ok_or_else(bad).and_then(within).map(ChanceTest::Is);
    }
    let mut out = Vec::new();
    let mut k = 0;
    while k < ts.len() {
        let Some(op) = op_of(&ts[k]) else { return Err(bad()) };
        let end = (k + 1..ts.len()).find(|&j| op_of(&ts[j]).is_some()).unwrap_or(ts.len());
        let v = ratio(&ts[k + 1..end]).ok_or_else(bad).and_then(within)?;
        out.push((op, v));
        k = end;
    }
    Ok(ChanceTest::Cmp(out))
}

impl P {
    /// `chances` and the table under it. The heading and the rows are consumed whatever goes wrong,
    /// so that rows under a broken heading are not read again as declarations of their own.
    pub(super) fn chances(&mut self, line: &[Token]) -> Option<ChancesDecl> {
        let span = span_of(line);
        self.i += 1;
        if line.len() > 1 {
            let at = self.at(span.line);
            self.err(
                Diag::error("E070", tr!("`chances` の行には何も続けられません", "Nothing follows `chances` on its line"))
                    .at(at)
                    .mark(span_of(&line[1..]), "")
                    .note(shape()),
            );
        }
        let is_row = |l: &[Token]| l.first().is_some_and(|t| t.is(&Kind::Pipe));
        let mut raw: Vec<Vec<Token>> = Vec::new();
        while self.cur().is_some_and(|l| is_row(l)) {
            raw.push(self.cur().cloned().unwrap());
            self.i += 1;
        }
        let Some(head) = raw.first().cloned() else {
            let at = self.at(span.line);
            self.err(
                Diag::error("E070", tr!("`chances` の下に表がありません", "There is no table under `chances`"))
                    .at(at)
                    .mark(span.clone(), tr!("見出しの直後に、表の行を書いてください", "write the table right under the heading"))
                    .note(shape()),
            );
            return None;
        };
        self.ctx = tr!("確率の主張", "chances");
        let out = self.chances_grid(&head, &raw[1..], span);
        self.ctx.clear();
        out
    }

    fn chances_grid(&mut self, head: &[Token], rows: &[Vec<Token>], span: Span) -> Option<ChancesDecl> {
        let header = split_cells(head);
        let mut inputs: Vec<(String, Span)> = Vec::new();
        let mut outputs: Vec<(String, Span)> = Vec::new();
        let mut arrow = false;
        let mut ok = self.closed(head);
        for (cell, sp) in &header {
            let here = cell.first().is_some_and(|t| t.is(&Kind::Arrow));
            arrow |= here;
            let words = &cell[usize::from(here)..];
            match words {
                [t] if t.ident().is_some() => {
                    let w = t.ident().unwrap_or_default().to_string();
                    if arrow { outputs.push((w, sp.clone())) } else { inputs.push((w, sp.clone())) }
                }
                _ => {
                    let what = tr!("列の名前として読めません", "this cannot be read as a column's name");
                    self.bad_cell(cell, head, what);
                    ok = false;
                }
            }
        }
        if !ok {
            return None;
        }
        let at = self.at(head[0].span.line);
        if !arrow {
            self.err(
                Diag::error("E070", tr!("`chances` の見出しに `->` がありません", "The header of `chances` has no `->`"))
                    .at(at)
                    .mark(span_of(head), tr!("出力の列の前に `->` を書いてください", "write `->` before the output columns"))
                    .note(shape()),
            );
            return None;
        }
        match outputs.last() {
            Some((w, _)) if w == crate::kw::CHANCE => {
                outputs.pop();
            }
            _ => {
                self.err(
                    Diag::error("E070", tr!("`chances` の見出しの最後の列が `chance` ではありません", "The last column of the `chances` header is not `chance`"))
                        .at(at)
                        .mark(header.last().map(|(_, s)| s.clone()).unwrap_or_else(|| span_of(head)), tr!("ここに `chance` が要ります", "`chance` belongs here"))
                        .note(shape()),
                );
                return None;
            }
        }
        if outputs.is_empty() {
            self.err(
                Diag::error("E070", tr!("`chances` に、主張する出力の列がありません", "`chances` names no output to make a claim about"))
                    .at(at)
                    .mark(span_of(head), tr!("`->` と `chance` のあいだに、出力の列を書いてください", "write the outputs between `->` and `chance`"))
                    .note(shape()),
            );
            return None;
        }
        let width = header.len();
        let mut out = Vec::new();
        for (n, rt) in rows.iter().enumerate() {
            let cells_t = split_cells(rt);
            if !self.closed(rt) {
                continue;
            }
            if cells_t.len() != width {
                let at = self.at(rt[0].span.line);
                self.err(
                    Diag::error("E064", tr!("行が見出しと合いません", "The row does not fit the header"))
                        .at(at)
                        .mark(span_of(rt), tr!("この行のセルは {} 個で、見出しの列は {} 個です", "this row has {} cells, and the header {} columns", cells_t.len(), width))
                        .note(tr!(
                            "一つの列に一つずつセルを書いてください。どの値でもよい列には `-` を書いてください。",
                            "Write one cell per column; a column that takes any value gets `-`."
                        )),
                );
                continue;
            }
            let mut row = ChanceRow {
                cells: Vec::new(),
                cell_spans: Vec::new(),
                outs: Vec::new(),
                out_spans: Vec::new(),
                chance: None,
                chance_text: String::new(),
                chance_span: span_of(rt),
                span: span_of(rt),
                index: n + 1,
            };
            for (ci, (ct, csp)) in cells_t.iter().enumerate() {
                if ct.is_empty() {
                    let at = self.at(rt[0].span.line);
                    self.err(
                        Diag::error("E008", tr!("空のセルがあります", "Empty cell"))
                            .at(at)
                            .mark(csp.clone(), "")
                            .note(tr!("どの値にも当てるなら `-` と書いてください（空のセルは、書き忘れと区別がつきません）", "Write `-` to match any value (an empty cell cannot be told apart from an omission)")),
                    );
                    continue;
                }
                if ci + 1 == width {
                    row.chance_text = ct.iter().enumerate().fold(String::new(), |mut acc, (j, t)| {
                        if j > 0 && matches!(t.kind, Kind::Le | Kind::Ge | Kind::Lt | Kind::Gt) {
                            acc.push(' ');
                        }
                        acc.push_str(&token_text(t));
                        acc
                    });
                    row.chance_span = csp.clone();
                    match chance_test(ct) {
                        Ok(t) => row.chance = Some(t),
                        Err(what) => {
                            let at = self.at(rt[0].span.line);
                            self.err(
                                Diag::error("E070", tr!("確率のセルを読めません", "The chance cell cannot be read"))
                                    .at(at)
                                    .mark(csp.clone(), what)
                                    .note(shape()),
                            );
                        }
                    }
                } else if ci < inputs.len() {
                    row.cells.push(self.cell(ct).unwrap_or(crate::ast::Cell::DontCare));
                    row.cell_spans.push(csp.clone());
                } else {
                    row.outs.push(self.cell(ct).unwrap_or(crate::ast::Cell::DontCare));
                    row.out_spans.push(csp.clone());
                }
            }
            out.push(row);
        }
        if out.is_empty() {
            let at = self.at(span.line);
            self.err(
                Diag::error("E070", tr!("`chances` の表に行がありません", "The `chances` table has no rows"))
                    .at(at)
                    .mark(span.clone(), tr!("見出しの下に、主張を一行ずつ書いてください", "write one claim per row under the header"))
                    .note(shape()),
            );
            return None;
        }
        Some(ChancesDecl { inputs, outputs, rows: out, span })
    }
}

/// A token as it was written, for the chance as the page and the messages quote it.
fn token_text(t: &Token) -> String {
    match &t.kind {
        Kind::Num(n) => n.raw.clone(),
        Kind::Ident(w) => w.clone(),
        Kind::Slash => "/".into(),
        Kind::Le => "<=".into(),
        Kind::Ge => ">=".into(),
        Kind::Lt => "<".into(),
        Kind::Gt => ">".into(),
        _ => String::new(),
    }
}
