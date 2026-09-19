//! SQL, the eighth target (§15.46): the rule as one query over a relation of inputs.
//!
//! One row in, one row out, in order — which is the one thing the per-row functions cannot
//! do: recompute a million rows in one statement, in the database where the closing batch
//! and the analysts' numbers already live. The query is written for PostgreSQL and kept
//! inside what SQLite also runs, because that is how `rulec test` holds it to the reference
//! evaluator with nothing but python3: the runner loads the vectors into an in-memory
//! SQLite, runs the query, and prints the same records every other runner prints.
//!
//! The shape is a chain of CTEs. The input relation first, its numbers cast to BIGINT (the
//! product of two int4 columns overflows in PostgreSQL where the proof, E108, said int64 is
//! enough); one CTE per definition; two per table — the number of the row that matched,
//! as `CASE WHEN … THEN 1 …`, then every output column keyed on that number; the raw value
//! of every output; and the final SELECT with each output rounded. Rounding is arithmetic
//! over the bound raw column — `CASE`, `ABS` and `%`, which mean the same in both dialects,
//! and integer division truncates toward zero in both. `min`/`max` are LEAST and GREATEST,
//! which the SQLite runner registers.

use super::{cell_src, mode_fn, out_src, pub_name, round_cases, Gen};
use crate::ast::{Cell, CmpOp, Item, Lit, OutCell, Policy, Table};
use crate::num::{Rat, RoundMode};
use crate::types::Ty;

/// A quoted identifier. Every generated name is quoted, so an alias that is a keyword
/// (`total`, `date`) and an alias with capitals both survive in both dialects.
fn q(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// A string literal.
fn lit(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// The rounding modes as arithmetic over a bound column `x` and a literal grid `g`. The
/// same five definitions as §7.3, with the sign taken off and put back so that the
/// direction is about the magnitude, the way the Python and Go helpers do it.
fn round_sql(m: RoundMode, x: &str, g: &str) -> String {
    let a = format!("ABS({x})");
    let sign = format!("(CASE WHEN {x} < 0 THEN -1 ELSE 1 END)");
    let up = format!("({a} / {g} + 1) * {g}");
    let down = format!("({a} / {g}) * {g}");
    match m {
        // Integer division truncates toward zero in PostgreSQL and SQLite alike, which is
        // exactly this mode.
        RoundMode::Down => format!("(({x} / {g}) * {g})"),
        RoundMode::Up => format!("({sign} * (CASE WHEN {a} % {g} = 0 THEN {a} ELSE {up} END))"),
        RoundMode::Half => format!("({sign} * (CASE WHEN 2 * ({a} % {g}) >= {g} THEN {up} ELSE {down} END))"),
        RoundMode::HalfDown => format!("({sign} * (CASE WHEN 2 * ({a} % {g}) > {g} THEN {up} ELSE {down} END))"),
        RoundMode::Bankers => format!(
            "({sign} * (CASE WHEN 2 * ({a} % {g}) > {g} OR (2 * ({a} % {g}) = {g} AND ({a} / {g}) % 2 = 1) THEN {up} ELSE {down} END))"
        ),
    }
}

/// The text `Gen::expr` writes — Python's spelling, over a fixed grammar — read back into
/// SQL. A name becomes a quoted column, `//` becomes `/`, `_min`/`_max` become LEAST and
/// GREATEST, and a rounding call becomes the arithmetic above over its argument, which is
/// first bound to a column of its own when it is not already a plain name (`binds`).
struct Sql {
    binds: Vec<(String, String)>,
    n: usize,
}

fn tokens(s: &str) -> Vec<String> {
    let b: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == ' ' {
            i += 1;
        } else if matches!(c, '(' | ')' | ',') {
            out.push(c.to_string());
            i += 1;
        } else if c.is_ascii_digit() || (c == '-' && i + 1 < b.len() && b[i + 1].is_ascii_digit()) {
            let start = i;
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            out.push(b[start..i].iter().collect());
        } else if matches!(c, '+' | '-' | '*' | '/' | '<' | '>' | '=') {
            let start = i;
            i += 1;
            while i < b.len() && matches!(b[i], '/' | '=') {
                i += 1;
            }
            out.push(b[start..i].iter().collect());
        } else {
            let start = i;
            while i < b.len() && !matches!(b[i], ' ' | '(' | ')' | ',' | '+' | '*' | '/' | '<' | '>' | '=') {
                i += 1;
            }
            out.push(b[start..i].iter().collect());
        }
    }
    out
}

impl Sql {
    fn new() -> Sql {
        Sql { binds: Vec::new(), n: 0 }
    }

    fn translate(&mut self, text: &str) -> String {
        let t = tokens(text);
        let mut i = 0;
        let a = self.expr(&t, &mut i);
        assert!(i == t.len(), "expression not read to the end: {text}");
        a
    }

    fn expr(&mut self, t: &[String], i: &mut usize) -> String {
        let mut a = self.term(t, i);
        // `rescaled` writes `name * k` with no parentheses; everything else is nested.
        while *i < t.len() && Self::is_op(&t[*i]) {
            let op = t[*i].clone();
            *i += 1;
            let b = self.term(t, i);
            a = Self::binop(&op, &a, &b);
        }
        a
    }

    fn is_op(s: &str) -> bool {
        matches!(s, "+" | "-" | "*" | "//" | "<=" | "<" | ">=" | ">" | "==")
    }

    fn binop(op: &str, a: &str, b: &str) -> String {
        match op {
            "//" => format!("({a} / {b})"),
            "==" => format!("({a} = {b})"),
            _ => format!("({a} {op} {b})"),
        }
    }

    fn term(&mut self, t: &[String], i: &mut usize) -> String {
        let tok = t[*i].clone();
        *i += 1;
        if tok == "(" {
            let a = self.expr(t, i);
            assert!(t[*i] == ")", "unbalanced");
            *i += 1;
            return a;
        }
        if *i < t.len() && t[*i] == "(" {
            *i += 1;
            let a = self.expr(t, i);
            assert!(t[*i] == ",", "a call takes two arguments");
            *i += 1;
            let b = self.expr(t, i);
            assert!(t[*i] == ")", "unbalanced");
            *i += 1;
            return match tok.as_str() {
                "_min" => format!("LEAST({a}, {b})"),
                "_max" => format!("GREATEST({a}, {b})"),
                f => {
                    // The Python helper's name, not the mode's keyword: `half_up` is spelled
                    // `_round_half` and `half_even` `_round_bankers` there. A callee output
                    // rounded `half_up`, expanded into an `apply`, was the first rounding call
                    // in a definition with either — and it stopped `gen` for every language.
                    let m = super::mode_from_fn(f.trim_start_matches("_round_")).expect("a rounding call");
                    // The mode reads its argument several times, so anything that is not
                    // already a column is bound to one first.
                    let x = if a.starts_with('"') || a.parse::<i128>().is_ok() { a } else { self.bind(a) };
                    round_sql(m, &x, &b)
                }
            };
        }
        if tok.parse::<i128>().is_ok() {
            return tok;
        }
        match tok.as_str() {
            "True" => "TRUE".into(),
            "False" => "FALSE".into(),
            _ => q(&tok),
        }
    }

    fn bind(&mut self, e: String) -> String {
        self.n += 1;
        let name = format!("_r{}", self.n);
        self.binds.push((name.clone(), e));
        q(&name)
    }
}

/// The value of one output cell, in SQL. The number is the column's own integer; a word is
/// an enum member as its name, or a bound name brought to the column's scale.
fn out_value(g: &Gen, oc_name: &str, out: Option<&OutCell>, local: &dyn Fn(&str) -> String, sql: &mut Sql) -> String {
    match out {
        Some(OutCell::Lit(Lit::Num(n))) => {
            let ty = g.ty_of(oc_name);
            g.int_lit(n, &ty, g.scale(oc_name))
        }
        Some(OutCell::Lit(l)) => match l {
            Lit::Word(w) if w == crate::kw::TRUE => "TRUE".into(),
            Lit::Word(w) if w == crate::kw::FALSE => "FALSE".into(),
            Lit::Word(w) => lit(w),
            Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
            Lit::Str(s) => lit(s),
            Lit::Num(_) => unreachable!(),
        },
        Some(OutCell::Name(w)) => {
            if w == crate::kw::TRUE {
                "TRUE".into()
            } else if w == crate::kw::FALSE {
                "FALSE".into()
            } else if g.value_names.contains_key(w) {
                lit(w)
            } else {
                sql.translate(&g.rescaled(w, oc_name, local(w)))
            }
        }
        None => "0".into(),
    }
}

impl<'a> Gen<'a> {
    /// A cell as a SQL condition over the column `var`. A don't-care yields None.
    pub(super) fn sql_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        let col = q(var);
        let l = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "TRUE".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "FALSE".into(),
                Lit::Word(w) => lit(w),
                Lit::Num(n) => self.int_lit(n, inner, col_scale),
                Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => lit(s),
            }
        };
        let members = |ls: &Vec<Lit>| -> String {
            let mut out: Vec<String> = Vec::new();
            for x in ls {
                if let Lit::Word(w) = x {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| lit(m)));
                        continue;
                    }
                }
                out.push(l(x));
            }
            format!("({})", out.join(", "))
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!("{col} IS NULL"),
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => format!("{col} IN {}", members(&vec![Lit::Word(w.clone())])),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => col,
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("NOT {col}"),
            Cell::Lit(x) => format!("{col} = {}", l(x)),
            Cell::Set(ls) => format!("{col} IN {}", members(ls)),
            Cell::Not(ls) => format!("{col} NOT IN {}", members(ls)),
            Cell::Cmp(cs) => cs
                .iter()
                .map(|(o, x)| {
                    let op = match o {
                        CmpOp::Le => "<=",
                        CmpOp::Ge => ">=",
                        CmpOp::Lt => "<",
                        CmpOp::Gt => ">",
                    };
                    format!("{col} {op} {}", l(x))
                })
                .collect::<Vec<_>>()
                .join(" AND "),
        })
    }

    /// The row columns the query produces, in the order it builds them: one per table, the
    /// tables of a merged set in the order they are tried, each with the (merged) table its
    /// rows are read from.
    fn sql_members(&self) -> Vec<(String, String, &Table)> {
        let mut out = Vec::new();
        let mut k = 0;
        for it in &self.f.items {
            let Item::Table(pt) = it else { continue };
            let Some(t) = self.c.table_at(pt) else { continue };
            match self.set_for(t) {
                Some(set) if set.merged() => {
                    for m in set.members.iter().rev() {
                        let member = self
                            .f
                            .items
                            .iter()
                            .find_map(|it| match it {
                                Item::Table(u) if u.name.as_ref().is_some_and(|n| n.text == *m) => Some(u),
                                _ => None,
                            })
                            .unwrap_or(pt);
                        out.push((m.clone(), self.sql_row_col(member, k), t));
                        k += 1;
                    }
                }
                _ => {
                    out.push((t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default(), self.sql_row_col(pt, k), t));
                    k += 1;
                }
            }
        }
        out
    }

    /// The name of a table's row column: `<alias>_row`, kept clear of every declared name.
    fn sql_row_col(&self, t: &Table, k: usize) -> String {
        let base = t.name.as_ref().map(pub_name).unwrap_or_else(|| format!("t{}", k + 1));
        let mut n = format!("{base}_row");
        while self.idents.values().any(|v| *v == n) || self.c.syms.contains_key(&n) {
            n.push('_');
        }
        n
    }

    /// The SQL type a column is declared with, for the header and for `rulec api`.
    fn sql_ty(&self, ty: &Ty) -> &'static str {
        match ty {
            Ty::Enum(_) | Ty::Str => "text",
            Ty::Bool => "boolean",
            Ty::Opt(t) => self.sql_ty(t),
            _ => "bigint",
        }
    }

    /// `sql/<alias>.sql`: the query.
    pub fn sql(&self) -> String {
        let alias = pub_name(&self.f.name);
        let input_rel = format!("{alias}_input");
        let local = |n: &str| -> String { self.ident(n) };
        let mut o = self.header("--");
        o.push_str("--\n");
        for line in self.sql_doc().lines() {
            o.push_str(&format!("-- {line}\n"));
        }
        // The columns of the input relation, one line each, with what to put in them.
        o.push_str(&tr!("--\n-- {} の列（一行が一件）:\n", "--\n-- The columns of {} (one row per case):\n", q(&input_rel)));
        o.push_str(&tr!("--   \"_id\"  行を識別する値。そのまま返る\n", "--   \"_id\"  any value that identifies the row; it comes back unchanged\n"));
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            o.push_str(&format!("--   {}  {}: {}\n", q(&local(&i.name.text)), i.name.text, self.sql_col_doc(&i.name.text, &ty)));
        }
        o.push_str(&tr!("-- 出るもの: \"_id\"、入力、出力、表ごとに当てはまった行の番号、そして \"_input_error\"（宣言の外の入力なら、その文。中なら NULL）:\n", "-- Out come \"_id\", the inputs, the outputs, one column per table with the number of the row that matched, and \"_input_error\" (a sentence when an input is outside its declaration, else NULL):\n"));
        for od in &self.f.outputs {
            let ty = self.ty_of(&od.name.text);
            o.push_str(&format!("--   {}  {}: {}\n", q(&local(&od.name.text)), od.name.text, self.sql_col_doc(&od.name.text, &ty)));
        }
        for (tname, col, _) in self.sql_members() {
            o.push_str(&format!("--   {}  {}\n", q(&col), tr!("表 {tname} の行", "the row of table {tname}")));
        }
        for g in &self.f.groups {
            let ms: Vec<String> = g.members.iter().map(|m| m.text.clone()).collect();
            o.push_str(&format!("-- {} = {}\n", g.name.text, ms.join(", ")));
        }

        o.push_str("WITH\n");
        let mut cols: Vec<String> = vec![q("_id")];
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            let c = q(&local(&i.name.text));
            cols.push(match self.sql_ty(&ty) {
                "bigint" => format!("CAST({c} AS BIGINT) AS {c}"),
                _ => c,
            });
        }
        // The entry guard (§8.5), as a column over the relation as it came: NULL inside the
        // declared domain, otherwise the sentence the other languages raise. A query cannot
        // stop, so the runner does. Read before the cast, so that 18.3 in a column declared
        // in steps of 0.1% is refused rather than truncated to 18 (§15.43).
        let mut guard: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            let col = q(&local(&i.name.text));
            let inner = match &ty {
                Ty::Opt(t) => t.as_ref(),
                other => other,
            };
            if !matches!(ty, Ty::Opt(_)) {
                guard.push(format!("WHEN {col} IS NULL THEN {}", lit(&tr!("{} がありません", "{} is missing", i.name.text))));
            }
            match inner {
                Ty::Enum(en) => {
                    let vs: Vec<String> = self.c.enums.get(en).cloned().unwrap_or_default().iter().map(|v| lit(v)).collect();
                    guard.push(format!(
                        "WHEN {col} NOT IN ({}) THEN {}",
                        vs.join(", "),
                        lit(&tr!("{} が列挙 {en} の値ではありません", "{} is not a value of enum {en}", i.name.text))
                    ));
                }
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                    guard.push(format!(
                        "WHEN {col} <> CAST({col} AS BIGINT) THEN {}",
                        lit(&tr!("{} が整数ではありません", "{} is not an integer", i.name.text))
                    ));
                    if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) {
                        let sc = self.c.wire_scale(&i.name.text);
                        guard.push(format!(
                            "WHEN {col} < {} OR {col} > {} THEN {}",
                            crate::types::wire_int(lo, sc),
                            crate::types::wire_int(hi, sc),
                            lit(&tr!("{} が範囲の外です", "{} is out of range", i.name.text))
                        ));
                    }
                }
                _ => {}
            }
        }
        // The same door as the other backends (§15.55): a combination the caller said does not
        // happen is refused rather than answered, because no row was ever demanded for it.
        for k in &self.f.constraints {
            let op = k.op.word();
            let said = format!("{} {op} {}", k.left, k.right);
            guard.push(format!(
                "WHEN NOT ({} {op} {}) THEN {}",
                q(&local(&k.left)),
                q(&local(&k.right)),
                lit(&tr!("制約が成り立ちません: {said}", "the constraint does not hold: {said}"))
            ));
        }
        cols.push(if guard.is_empty() {
            format!("NULL AS {}", q("_input_error"))
        } else {
            format!("CASE {} ELSE NULL END AS {}", guard.join(" "), q("_input_error"))
        });
        o.push_str(&format!("{} AS (\n  SELECT {} FROM {}\n)", q("_c0"), cols.join(",\n    "), q(&input_rel)));
        let mut prev = q("_c0");
        let mut stage = 0usize;
        let mut sql = Sql::new();
        // Anything a stage bound on the way (the argument of a rounding call) is put into a
        // CTE of its own, before the stage that reads it.
        let flush_binds = |o: &mut String, prev: &mut String, sql: &mut Sql, stage: &mut usize| {
            if sql.binds.is_empty() {
                return;
            }
            *stage += 1;
            let name = q(&format!("_c{stage}"));
            let bs: Vec<String> = sql.binds.drain(..).map(|(n, e)| format!("{e} AS {}", q(&n))).collect();
            o.push_str(&format!(",\n{name} AS (\n  SELECT *, {} FROM {prev}\n)", bs.join(", ")));
            *prev = name;
        };

        let mut k = 0;
        let mut guard_tables: Vec<&Table> = Vec::new();
        let mut row_cols: Vec<String> = Vec::new();
        for it in &self.f.items {
            match it {
                // A rule that walks a sequence is not generated for SQL at all (§15.56).
                Item::Count(_) => {}
                Item::Derived(d) => {
                    let e = self.expr(&d.expr, &local);
                    let text = sql.translate(&e.text);
                    flush_binds(&mut o, &mut prev, &mut sql, &mut stage);
                    stage += 1;
                    let name = q(&format!("_c{stage}"));
                    o.push_str(&format!(
                        ",\n-- {}\n{name} AS (\n  SELECT *, {text} AS {} FROM {prev}\n)",
                        tr!("導出 {}", "derive {}", d.name.text),
                        q(&local(&d.name.text))
                    ));
                    prev = name;
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, &local);
                    let text = sql.translate(&e.text);
                    flush_binds(&mut o, &mut prev, &mut sql, &mut stage);
                    stage += 1;
                    let name = q(&format!("_c{stage}"));
                    o.push_str(&format!(
                        ",\n-- {}\n{name} AS (\n  SELECT *, {text} AS {} FROM {prev}\n)",
                        tr!("定義 {}", "define {}", d.name.text),
                        q(&local(&d.name.text))
                    ));
                    prev = name;
                }
                Item::Table(pt) => {
                    // The definition set is evaluated at its last member (§15.66);
                    // at any other member's position the query builds nothing.
                    let Some(t) = self.c.table_at(pt) else { continue };
                    let tname = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
                    // The tables to try, in the order they are tried, each with its rows of
                    // the (merged) table. A table that stands alone is the one entry.
                    let groups: Vec<(String, &Table, Vec<usize>)> = match self.set_for(t) {
                        Some(set) if set.merged() => set
                            .members
                            .iter()
                            .enumerate()
                            .rev()
                            .map(|(mi, m)| {
                                let member = self
                                    .f
                                    .items
                                    .iter()
                                    .find_map(|it| match it {
                                        Item::Table(u) if u.name.as_ref().is_some_and(|n| n.text == *m) => Some(u),
                                        _ => None,
                                    })
                                    .unwrap_or(pt);
                                (m.clone(), member, (0..t.rows.len()).filter(|&i| set.member_of[i] == mi).collect())
                            })
                            .collect(),
                        _ => vec![(tname.clone(), pt, (0..t.rows.len()).collect())],
                    };
                    let merged = groups.len() > 1;
                    let mut cols_here: Vec<String> = Vec::new();
                    for (mname, member, idxs) in &groups {
                        let row_col = self.sql_row_col(member, k);
                        let kind = if member.clause { tr!("節", "clause") } else { tr!("表", "table") };
                        let head = if merged {
                            tr!("{kind} {mname}（{tname} を定める。先に試した表か節が当たっていれば NULL）", "{kind} {mname} (defines {tname}; NULL when a table or clause tried earlier matched)")
                        } else if member.clause {
                            tr!("節 {tname}", "clause {tname}")
                        } else {
                            let policy = if member.policy == Policy::Unique { crate::kw::UNIQUE } else { crate::kw::FIRST };
                            tr!("表 {tname}（{} {policy}）", "table {tname} ({} {policy})", crate::kw::POLICY)
                        };
                        // The row that matched: one WHEN per row, in order, so `policy first` is
                        // the CASE's own rule and `policy unique` has been proved to make no
                        // difference. In a merged set the tables tried earlier come first.
                        stage += 1;
                        let rows_cte = q(&format!("_c{stage}"));
                        o.push_str(&format!(",\n-- {head}\n{rows_cte} AS (\n  SELECT *, CASE\n"));
                        if !cols_here.is_empty() {
                            let m = cols_here.iter().map(|c| format!("{} IS NOT NULL", q(c))).collect::<Vec<_>>().join(" OR ");
                            o.push_str(&format!("    WHEN {m} THEN NULL  -- {}\n", tr!("先に試した表が当たった", "a table tried earlier matched")));
                        }
                        for &ri in idxs {
                            let row = &t.rows[ri];
                            let conds: Vec<String> = t
                                .inputs
                                .iter()
                                .enumerate()
                                .filter_map(|(ci, (col, _))| {
                                    let ty = self.ty_of(col);
                                    self.sql_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                                })
                                .collect();
                            let cond = if conds.is_empty() { "TRUE".into() } else { conds.join(" AND ") };
                            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
                            o.push_str(&format!("    WHEN {cond} THEN {}  -- {}\n", row.index, self.row_head(t, ri, &cells.join(" | "))));
                        }
                        o.push_str(&format!("  END AS {} FROM {prev}\n)", q(&row_col)));
                        prev = rows_cte;
                        row_cols.push(row_col.clone());
                        cols_here.push(row_col);
                        k += 1;
                    }
                    // The output columns, keyed on that number — on whichever table's number,
                    // in a merged set.
                    let mut outs: Vec<String> = Vec::new();
                    for (oi, oc) in t.outputs.iter().enumerate() {
                        let mut arms: Vec<String> = Vec::new();
                        if merged {
                            for ((_, _, idxs), col) in groups.iter().zip(&cols_here) {
                                for &ri in idxs {
                                    arms.push(format!(
                                        "WHEN {} = {} THEN {}",
                                        q(col),
                                        t.rows[ri].index,
                                        out_value(self, &oc.name.text, t.rows[ri].outs.get(oi), &local, &mut sql)
                                    ));
                                }
                            }
                            outs.push(format!("CASE {} END AS {}", arms.join(" "), q(&local(&oc.name.text))));
                        } else {
                            for row in &t.rows {
                                arms.push(format!("WHEN {} THEN {}", row.index, out_value(self, &oc.name.text, row.outs.get(oi), &local, &mut sql)));
                            }
                            outs.push(format!("CASE {} {} END AS {}", q(&cols_here[0]), arms.join(" "), q(&local(&oc.name.text))));
                        }
                    }
                    flush_binds(&mut o, &mut prev, &mut sql, &mut stage);
                    stage += 1;
                    let vals_cte = q(&format!("_c{stage}"));
                    o.push_str(&format!(",\n{vals_cte} AS (\n  SELECT *, {} FROM {prev}\n)", outs.join(", ")));
                    prev = vals_cte;
                    guard_tables.push(t);
                }
            }
        }

        // The raw value of every output, then the rounding over it, exactly once (§15.16).
        let outs = &self.f.outputs;
        let mut raws: Vec<String> = Vec::new();
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => super::Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let ty = self.ty_of(out_name);
            let raw = q(&format!("_raw_{}", local(out_name)));
            let text = sql.translate(&res.text);
            raws.push(format!("{text} AS {raw}"));
            finals.push(match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    let rounded = round_sql(m, &raw, &grid_i.to_string());
                    if res.scale != os {
                        format!("{rounded} / {} AS {}", res.scale / os, q(&local(out_name)))
                    } else {
                        format!("{rounded} AS {}", q(&local(out_name)))
                    }
                }
                None => format!("{raw} AS {}", q(&local(out_name))),
            });
        }
        flush_binds(&mut o, &mut prev, &mut sql, &mut stage);
        stage += 1;
        let raw_cte = q(&format!("_c{stage}"));
        o.push_str(&format!(",\n-- {}\n{raw_cte} AS (\n  SELECT *, {} FROM {prev}\n)", tr!("出力の丸め前の値", "the outputs before rounding"), raws.join(", ")));
        prev = raw_cte;

        // W114: a pair of rows whose exclusivity could not be proven statically. The other
        // languages stop; here the row says what happened, and the runner stops on it.
        let mut contra: Vec<String> = Vec::new();
        for t in &guard_tables {
            let tname = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
            let Some(pairs) = self.w114.get(&tname) else { continue };
            for (i, j) in pairs {
                let mut conds: Vec<String> = Vec::new();
                for (ci, (col, _)) in t.inputs.iter().enumerate() {
                    let ty = self.ty_of(col);
                    let sc = self.scale(col);
                    for r in [*i, *j] {
                        let Some(cell) = t.rows[r].cells.get(ci) else { continue };
                        if let Some(c) = self.sql_cell(cell, &local(col), &ty, sc) {
                            if !conds.contains(&c) {
                                conds.push(c);
                            }
                        }
                    }
                }
                if conds.is_empty() {
                    continue;
                }
                contra.push(format!(
                    "WHEN {} THEN {}",
                    conds.join(" AND "),
                    lit(&tr!("表 {tname}: {} と {} が同時に当てはまりました", "table {tname}: {} and {} matched at the same time", self.row_name(t, *i), self.row_name(t, *j)))
                ));
            }
        }

        let mut select: Vec<String> = vec![q("_id")];
        select.extend(self.f.inputs.iter().map(|i| q(&local(&i.name.text))));
        select.extend(finals);
        select.extend(row_cols.iter().map(|rc| q(rc)));
        select.push(q("_input_error"));
        if !contra.is_empty() {
            select.push(format!("CASE {} ELSE NULL END AS {}", contra.join(" "), q("_contradiction")));
        }
        o.push_str(&format!("\nSELECT\n  {}\nFROM {prev}\nORDER BY {};\n", select.join(",\n  "), q("_id")));
        o
    }

    /// What a column holds, for the header of the query.
    fn sql_col_doc(&self, name: &str, ty: &Ty) -> String {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        let sc = self.c.wire_scale(name);
        let mut s = match inner {
            Ty::Enum(en) => {
                let vs = self.c.enums.get(en).cloned().unwrap_or_default();
                tr!("text。{} のどれか", "text, one of {}", vs.join(", "))
            }
            Ty::Str => "text".into(),
            Ty::Bool => "boolean".into(),
            Ty::Date => tr!("bigint。1970-01-01 からの日数", "bigint, the number of days since 1970-01-01"),
            Ty::Money { cur, tax } => {
                let t = match tax.as_deref() {
                    Some("incl_tax") => tr!("（税込）", " (tax included)"),
                    Some("excl_tax") => tr!("（税抜）", " (tax excluded)"),
                    _ => String::new(),
                };
                tr!("bigint。単位は {cur}{t}", "bigint, in {cur}{t}")
            }
            Ty::Qty { unit, .. } => tr!("bigint。単位は {unit}", "bigint, in {unit}"),
            Ty::Rate => {
                let step = if 100 % sc == 0 { format!("{}%", 100 / sc) } else { format!("{}%", 100.0 / sc as f64) };
                tr!("bigint。率を {step} 刻みの個数で（100% なら {sc}）", "bigint, the rate as a count of {step} steps (100% is {sc})")
            }
            _ => "bigint".into(),
        };
        if let Some((Some(lo), Some(hi))) = self.c.ranges.get(name).map(|(a, b)| (*a, *b)) {
            s.push_str(&format!(", {}..{}", crate::types::wire_int(lo, sc), crate::types::wire_int(hi, sc)));
        }
        if matches!(ty, Ty::Opt(_)) {
            s.push_str(&tr!("、NULL 可", ", or NULL"));
        }
        s
    }

    /// The `sql` entry of `rulec api`: the file, the input relation and its columns, the
    /// output columns with their rounding, and the row column of every table.
    pub(super) fn api_sql(&self) -> String {
        let alias = pub_name(&self.f.name);
        let local = |n: &str| -> String { self.ident(n) };
        let columns: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &local(&i.name.text), self.sql_ty(&ty), &ty)
            })
            .collect();
        let outputs: Vec<String> = self
            .f
            .outputs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &local(&od.name.text), self.sql_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let mut rows: Vec<String> = Vec::new();
        for (tname, col, _) in self.sql_members() {
            rows.push(crate::json::Obj::new().str("table", &tname).str("column", &col).finish());
        }
        crate::json::Obj::new()
            .str("file", &format!("{alias}.sql"))
            .str("input", &format!("{alias}_input"))
            .str("id", "_id")
            .str("guard", "_input_error")
            .str("dialect", "postgresql")
            .raw("runs_on", crate::json::strs(&["postgresql", "sqlite"]))
            .raw("columns", crate::json::arr(&columns))
            .raw("outputs", crate::json::arr(&outputs))
            .raw("rows", crate::json::arr(&rows))
            .finish()
    }

    fn sql_doc(&self) -> String {
        tr!(
            "規則 {} v{} を、入力の関係（一行が一件）に対する一つの問い合わせにしたもの。\nPostgreSQL 向けに書き、SQLite でも同じに動く範囲に留めてある。`rulec test` はその SQLite で\nベクタ全件を流し、参照評価器と一字一句突き合わせる。数は全部、宣言した単位の整数で、率は刻みの個数、\n日付は 1970-01-01 からの日数。列挙はその名前。丸めは問い合わせの中の算術で、関数は要らない。\nビューにするなら: CREATE VIEW {} AS <この問い合わせ>",
            "Rule {} v{} as one query over a relation of inputs, one row per case.\nWritten for PostgreSQL and kept inside what SQLite also runs: `rulec test` streams every vector\nthrough that SQLite and holds the result to the reference evaluator byte for byte. Every number\nis an integer in its declared unit, a rate a count of its steps, a date the number of days since\n1970-01-01, an enum its name. Rounding is arithmetic inside the query; no function is needed.\nAs a view: CREATE VIEW {} AS <this query>",
            self.f.name.text,
            self.f.version,
            pub_name(&self.f.name)
        )
    }

    /// `sql/<alias>_runner.py`: the vectors through the query on an in-memory SQLite, and
    /// the same record per line the other runners print.
    pub fn sql_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let local = |n: &str| -> String { self.ident(n) };
        let kind = |ty: &Ty| -> &'static str {
            match ty {
                Ty::Enum(_) => "enum",
                Ty::Str => "str",
                Ty::Bool => "bool",
                Ty::Date => "date",
                Ty::Opt(t) => match **t {
                    Ty::Enum(_) => "enum",
                    Ty::Str => "str",
                    Ty::Bool => "bool",
                    Ty::Date => "date",
                    _ => "int",
                },
                _ => "int",
            }
        };
        let py = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
        let ins: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("({}, {}, \"{}\")", py(&i.name.text), py(&local(&i.name.text)), kind(&self.ty_of(&i.name.text))))
            .collect();
        let outs: Vec<String> = self
            .f
            .outputs
            .iter()
            .map(|o| format!("({}, {}, \"{}\")", py(&o.name.text), py(&local(&o.name.text)), kind(&self.ty_of(&o.name.text))))
            .collect();
        let mut tables: Vec<String> = Vec::new();
        let mut labels: Vec<String> = Vec::new();
        let mut any_w114 = false;
        for (tname, col, t) in self.sql_members() {
            tables.push(format!("({}, {})", py(&tname), py(&col)));
            let key = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
            if self.w114.get(&key).is_some_and(|p| !p.is_empty()) {
                any_w114 = true;
            }
            for row in &t.rows {
                let mine = row.origin.as_deref().map(|o| o == tname).unwrap_or(true);
                if let (true, Some(l)) = (mine, &row.label) {
                    labels.push(format!("({}, {}): {}", py(&tname), row.index, py(&l.text)));
                }
            }
        }
        SQL_RUNNER
            .replace("@HEADER@", self.header("#").trim_end())
            .replace("@ALIAS@", &alias)
            .replace("@INPUTS@", &ins.join(", "))
            .replace("@OUTPUTS@", &outs.join(", "))
            .replace("@TABLES@", &tables.join(", "))
            .replace("@LABELS@", &labels.join(", "))
            .replace("@CONTRADICTION@", if any_w114 { "\"_contradiction\"" } else { "None" })
    }
}

const SQL_RUNNER: &str = r#"@HEADER@
import datetime
import json
import sqlite3
import sys
from pathlib import Path

SQL = Path(__file__).with_name("@ALIAS@.sql").read_text(encoding="utf-8")
INPUT = "@ALIAS@_input"
INPUTS = [@INPUTS@]
OUTPUTS = [@OUTPUTS@]
TABLES = [@TABLES@]
LABELS = {@LABELS@}
CONTRADICTION = @CONTRADICTION@


def _ord(s: str) -> int:
    y, m, d = (int(x) for x in s.split("-"))
    return (datetime.date(y, m, d) - datetime.date(1970, 1, 1)).days


def _civil(days: int) -> str:
    return (datetime.date(1970, 1, 1) + datetime.timedelta(days=days)).isoformat()


def _to_sql(kind: str, v: object) -> object:
    if v is None:
        return None
    if kind == "bool":
        return 1 if v else 0
    if kind == "date":
        return _ord(str(v))
    return v


def _echo(v: object) -> str:
    if v is None:
        return "null"
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, int):
        return str(v)
    return json.dumps(v, ensure_ascii=False)


def _from_sql(kind: str, v: object) -> str:
    if v is None:
        return "null"
    if kind == "bool":
        return "true" if v else "false"
    if kind == "date":
        return json.dumps(_civil(int(str(v))))
    if kind == "int":
        return str(v)
    return json.dumps(v, ensure_ascii=False)


db = sqlite3.connect(":memory:")
db.row_factory = sqlite3.Row
db.create_function("LEAST", 2, min, deterministic=True)
db.create_function("GREATEST", 2, max, deterministic=True)
db.execute('CREATE TABLE "%s" ("_id" INTEGER PRIMARY KEY, %s)' % (INPUT, ", ".join('"%s"' % a for _, a, _ in INPUTS)))
cases = []
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    d = json.loads(line)["in"]
    db.execute(
        'INSERT INTO "%s" VALUES (?%s)' % (INPUT, ", ?" * len(INPUTS)),
        (len(cases), *[_to_sql(k, d[jp]) for jp, _, k in INPUTS]),
    )
    cases.append(d)
for r in db.execute(SQL):
    d = cases[r["_id"]]
    if r["_input_error"] is not None:
        raise ValueError(r["_input_error"])
    if CONTRADICTION is not None and r[CONTRADICTION] is not None:
        raise AssertionError(r[CONTRADICTION])
    ins = ",".join(json.dumps(jp, ensure_ascii=False) + ":" + _echo(d[jp]) for jp, _, _ in INPUTS)
    obs = ",".join(json.dumps(jp, ensure_ascii=False) + ":" + _from_sql(k, r[a]) for jp, a, k in OUTPUTS)
    rows = ",".join(
        '{"table":' + json.dumps(t, ensure_ascii=False) + ',"row":' + str(r[c])
        + (',"label":' + json.dumps(LABELS[(t, r[c])], ensure_ascii=False) if (t, r[c]) in LABELS else "")
        + "}"
        for t, c in TABLES
        if r[c] is not None
    )
    print('{"in":{' + ins + '},"observed":{' + obs + '},"trace":[' + rows + "]}")
"#;

/// `sql/_round_test.py`: the five rounding expressions, evaluated by SQLite over the same
/// unit vectors every other language gets.
pub fn round_tests_sql() -> String {
    let mut o = format!(
        "# Code generated by rulec {}. DO NOT EDIT.\n# {}\nimport sqlite3\nimport sys\n\n",
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の五モード。SQL の算術で書いた丸めを、負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five modes of §7.3 as SQL arithmetic, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str("EXPR = {\n");
    for m in [RoundMode::Down, RoundMode::Up, RoundMode::Half, RoundMode::HalfDown, RoundMode::Bankers] {
        o.push_str(&format!("    \"{}\": \"{}\",\n", mode_fn(m), round_sql(m, "@X@", "@G@").replace('"', "\\\"")));
    }
    o.push_str("}\n\nCASES = [\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("    (\"{}\", {x}, {g}, {want}),\n", mode_fn(m)));
    }
    o.push_str(
        "]\n\ndb = sqlite3.connect(\":memory:\")\nbad = 0\nfor mode, x, g, want in CASES:\n    \
         sql = EXPR[mode].replace(\"@X@\", \"(%d)\" % x).replace(\"@G@\", str(g))\n    \
         got = db.execute(\"SELECT \" + sql).fetchone()[0]\n    \
         if got != want:\n        \
             print(f\"NG {mode}({x}, {g}) = {got}, want {want}\")\n        \
             bad += 1\nif bad:\n    sys.exit(1)\n",
    );
    o.push_str(&format!("print(f\"{}\")\n", tr!("ok {{len(CASES)}} 件", "ok {{len(CASES)}} cases")));
    o
}
