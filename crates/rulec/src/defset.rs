//! Definition sets — the unit the checks, the evaluator and the generators work on.
//!
//! A table used to be that unit. Several tables may now define the same output and say with
//! `overrides` which of them takes precedence (DESIGN-draft §2), so the unit becomes
//! "everything that defines one output": a merged table whose rows remember the table they
//! were written in, and a precedence relation between those rows. A table that shares its
//! output with no other table is a set of one, and nothing about it changes.
//!
//! The rows of a merged table are in **evaluation order**: the members from the last declared
//! to the first, each member's rows as written. An `overrides` line may only point upward, so
//! a winner is always declared after its loser, and this order is a linear extension of the
//! precedence relation. The evaluator and every generator keep their "first row that matches
//! wins" shape; only the order of the rows is decided here (DESIGN-draft §2.5).

use crate::ast::*;
use crate::diag::{Diag, Span};
use std::collections::HashMap;

/// One `overrides` target, resolved: every row of member `winner` takes precedence over the
/// rows of member `loser`, or over its one row `loser_row` (a merged index).
pub struct Edge {
    pub winner: usize,
    pub loser: usize,
    pub loser_row: Option<usize>,
    pub span: Span,
}

pub struct DefSet {
    /// The output the tables share, or the table's own name when it stands alone.
    pub key: String,
    /// The member tables, in declaration order.
    pub members: Vec<String>,
    pub policies: Vec<Policy>,
    /// Whether each member was written as a `clause`.
    pub clause: Vec<bool>,
    /// The `apply` each member came in through, if any (§15.69).
    pub applied: Vec<Option<String>>,
    /// The merged table. Its name is `key`; every row carries `origin` and its own `index`.
    pub table: Table,
    /// Merged row → position in `members`.
    pub member_of: Vec<usize>,
    /// `beats[i]`: the merged rows that take precedence over row `i`, transitively closed.
    pub beats: Vec<Vec<usize>>,
    pub edges: Vec<Edge>,
    /// The member at whose position the set is evaluated: the last one declared, so that
    /// everything any member reads is bound by then.
    pub eval_at: String,
}

impl DefSet {
    pub fn merged(&self) -> bool {
        self.members.len() > 1
    }

    /// Whether the precedence relation orders the two rows. Rows come in evaluation order,
    /// so for `i < j` this is exactly "row i beats row j".
    pub fn comparable(&self, i: usize, j: usize) -> bool {
        self.beats[j].contains(&i) || self.beats[i].contains(&j)
    }

    /// The table row `i` was written in.
    pub fn row_table(&self, i: usize) -> &str {
        match &self.table.rows[i].origin {
            Some(o) => o.as_str(),
            None => self.table.name.as_ref().map(|n| n.text.as_str()).unwrap_or(""),
        }
    }

    pub fn same_member(&self, i: usize, j: usize) -> bool {
        self.member_of[i] == self.member_of[j]
    }

    pub fn policy_of(&self, i: usize) -> Policy {
        self.policies[self.member_of[i]]
    }

    /// Whether row `i` was written as a clause.
    pub fn is_clause_row(&self, i: usize) -> bool {
        self.clause[self.member_of[i]]
    }

    /// How a member is called: 表 or 節.
    pub fn kind_word(&self, mi: usize) -> String {
        if self.clause[mi] { tr!("節", "clause") } else { tr!("表", "table") }
    }
}

fn table_name(t: &Table) -> String {
    t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default()
}

/// A set of one table: the table itself, with the precedence its policy gives.
pub fn single(t: &Table) -> DefSet {
    let n = t.rows.len();
    let mut beats: Vec<Vec<usize>> = vec![Vec::new(); n];
    if t.policy == Policy::TopDown {
        for j in 0..n {
            beats[j] = (0..j).collect();
        }
    }
    let name = table_name(t);
    DefSet {
        key: name.clone(),
        members: vec![name.clone()],
        policies: vec![t.policy],
        clause: vec![t.clause],
        applied: vec![t.applied.clone()],
        table: t.clone(),
        member_of: vec![0; n],
        beats,
        edges: Vec::new(),
        eval_at: name,
    }
}

struct RawEdge {
    winner: usize,
    loser: usize,
    loser_label: Option<String>,
    span: Span,
}

fn find(p: &mut [usize], i: usize) -> usize {
    let mut r = i;
    while p[r] != r {
        r = p[r];
    }
    let mut k = i;
    while p[k] != r {
        let nx = p[k];
        p[k] = r;
        k = nx;
    }
    r
}

fn union(p: &mut [usize], a: usize, b: usize) {
    let ra = find(p, a);
    let rb = find(p, b);
    if ra != rb {
        p[ra.max(rb)] = ra.min(rb);
    }
}

/// Builds the sets of a rule, in the order of each set's first member, and the map from a
/// table's name to its set and whether the set is evaluated at that table.
pub fn build(f: &RuleFile, path: &str) -> (Vec<DefSet>, HashMap<String, (usize, bool)>, Vec<Diag>) {
    let mut diags: Vec<Diag> = Vec::new();
    let tables: Vec<&Table> = f
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Table(t) => Some(t),
            _ => None,
        })
        .collect();
    let at = |t: &Table, line: usize| tr!("{path}:{line} 表 {}", "{path}:{line} table {}", table_name(t));

    let mut parent: Vec<usize> = (0..tables.len()).collect();
    let mut by_out: HashMap<String, Vec<usize>> = HashMap::new();
    for (k, t) in tables.iter().enumerate() {
        for oc in &t.outputs {
            by_out.entry(oc.name.text.clone()).or_default().push(k);
        }
    }
    for ks in by_out.values() {
        for w in ks.windows(2) {
            union(&mut parent, w[0], w[1]);
        }
    }

    // The `overrides` targets. An exception is written after what it excepts, so a target is
    // a table declared above (E035), and it defines the same output (E036).
    let mut raw: Vec<RawEdge> = Vec::new();
    for (k, t) in tables.iter().enumerate() {
        for r in &t.overrides {
            // `a:b` is table `a`, row `b` — unless `a:b` is the name of a table brought in by
            // an `apply` (§15.69), which is what the whole spelling names then.
            let joined = r.row.as_ref().map(|l| format!("{}:{l}", r.table));
            let (tname, row) = match joined.filter(|j| tables.iter().any(|u| table_name(u) == *j)) {
                Some(j) => (j, None),
                None => (r.table.clone(), r.row.clone()),
            };
            let target = tables.iter().position(|u| table_name(u) == tname);
            let shown = match &row {
                Some(l) => format!("{tname}:{l}"),
                None => tname.clone(),
            };
            match target {
                None => {
                    diags.push(
                        Diag::error("E035", tr!("`{}` の指す先 `{shown}` がありません", "The target `{shown}` of `{}` does not exist", crate::kw::OVERRIDES))
                            .at(at(t, r.span.line))
                            .mark(r.span.clone(), tr!("この名前の表は上にありません", "no table of this name is declared above"))
                            .note(tr!(
                                "指せるのは、この表より上で宣言した表か、その行（`表:行ラベル`）です。例外は本文の後に書きます。",
                                "A target is a table declared above this one, or one of its rows (`table:label`). The exception is written after what it excepts."
                            )),
                    );
                    continue;
                }
                Some(u) if u >= k => {
                    diags.push(
                        Diag::error("E035", tr!("`{}` の指す先 `{shown}` は、この表より後ろにあります", "The target `{shown}` of `{}` is declared after this table", crate::kw::OVERRIDES))
                            .at(at(t, r.span.line))
                            .mark(r.span.clone(), if u == k { tr!("自分自身です", "this is the table itself") } else { tr!("後ろで宣言されています", "declared below") })
                            .note(tr!(
                                "優先する側を後に書いてください。例外は本文の後に来ます。",
                                "Write the side that takes precedence later. The exception comes after the main rule."
                            )),
                    );
                    continue;
                }
                Some(u) => {
                    if let Some(l) = &row {
                        let has = tables[u].rows.iter().any(|row| row.label.as_ref().is_some_and(|x| x.text == *l));
                        if !has {
                            diags.push(
                                Diag::error("E035", tr!("表 {} に行ラベル `{l}` はありません", "Table {} has no row labelled `{l}`", tname))
                                    .at(at(t, r.span.line))
                                    .mark(r.span.clone(), "")
                                    .note(tr!(
                                        "行を指すには、その行の先頭にラベルを書きます（`{l} | … |`）。番号では指せません。行を挿すと番号は動きます。",
                                        "To name a row, write a label at its head (`{l} | … |`). A row cannot be named by its number: numbers move when a row is inserted."
                                    )),
                            );
                            continue;
                        }
                    }
                    let shared = t.outputs.iter().any(|o| tables[u].outputs.iter().any(|p| p.name.text == o.name.text));
                    if !shared {
                        let mine: Vec<String> = t.outputs.iter().map(|o| o.name.text.clone()).collect();
                        let theirs: Vec<String> = tables[u].outputs.iter().map(|o| o.name.text.clone()).collect();
                        diags.push(
                            Diag::error("E036", tr!("`{}` の相手 `{}` は、同じ出力を定めていません", "The target `{}` of `{}` does not define the same output", crate::kw::OVERRIDES, tname))
                                .at(at(t, r.span.line))
                                .mark(r.span.clone(), tr!("{} を定めています", "defines {}", theirs.join(", ")))
                                .note(tr!(
                                    "この表は {} を定めます。優先の順序は、同じ出力を定める定義のあいだにだけあります。",
                                    "This table defines {}. Precedence exists only between definitions of the same output.",
                                    mine.join(", ")
                                )),
                        );
                        continue;
                    }
                    union(&mut parent, k, u);
                    raw.push(RawEdge { winner: k, loser: u, loser_label: row.clone(), span: r.span.clone() });
                }
            }
        }
    }

    // Group the tables. A merged set holds tables of one output each (E045): a row that
    // served two outputs would belong to two sets at once, and the generated branch would
    // have to be written twice.
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for k in 0..tables.len() {
        let r = find(&mut parent, k);
        groups.entry(r).or_default().push(k);
    }
    let mut order: Vec<Vec<usize>> = groups.into_values().collect();
    for g in order.iter_mut() {
        g.sort_unstable();
    }
    order.sort_by_key(|g| g[0]);

    let mut sets: Vec<DefSet> = Vec::new();
    let mut set_of: HashMap<String, (usize, bool)> = HashMap::new();
    for members in order {
        let mut ok = members.len() > 1;
        if ok {
            for &m in &members {
                let t = tables[m];
                if t.outputs.len() > 1 {
                    ok = false;
                    let others: Vec<String> = members.iter().filter(|&&x| x != m).map(|&x| table_name(tables[x])).collect();
                    diags.push(
                        Diag::error("E045", tr!("出力を共有する表 {} に、出力の列が二つ以上あります", "Table {} shares an output and has two or more output columns", table_name(t)))
                            .at(at(t, t.span.line))
                            .mark(t.span.clone(), "")
                            .note(tr!(
                                "{} と出力を共有する表は、出力の列を一つだけ持ちます。二つ目の出力は別の表に分けてください。",
                                "A table that shares an output with {} has exactly one output column. Move the second output to a table of its own.",
                                others.join(", ")
                            )),
                    );
                }
            }
        }
        if !ok {
            for &m in &members {
                let idx = sets.len();
                let s = single(tables[m]);
                set_of.insert(s.key.clone(), (idx, true));
                sets.push(s);
            }
            continue;
        }
        let idx = sets.len();
        let s = merge(&tables, &members, &raw);
        for m in &s.members {
            set_of.insert(m.clone(), (idx, *m == s.eval_at));
        }
        sets.push(s);
    }
    (sets, set_of, diags)
}

/// The merged table of tables `members` (declaration order), with its precedence relation.
fn merge(tables: &[&Table], members: &[usize], raw: &[RawEdge]) -> DefSet {
    let names: Vec<String> = members.iter().map(|&m| table_name(tables[m])).collect();
    let policies: Vec<Policy> = members.iter().map(|&m| tables[m].policy).collect();
    let clause: Vec<bool> = members.iter().map(|&m| tables[m].clause).collect();
    let applied: Vec<Option<String>> = members.iter().map(|&m| tables[m].applied.clone()).collect();
    let key = tables[members[0]].outputs[0].name.text.clone();

    // The columns, in order of first appearance.
    let mut cols: Vec<(String, Span)> = Vec::new();
    for &m in members {
        for (n, sp) in &tables[m].inputs {
            if !cols.iter().any(|(c, _)| c == n) {
                cols.push((n.clone(), sp.clone()));
            }
        }
    }
    // The output column, as the first member declared it (a later member may omit the type).
    let out = tables[members[0]].outputs[0].clone();

    // The rows in evaluation order: members from the last declared to the first.
    let mut rows: Vec<Row> = Vec::new();
    let mut member_of: Vec<usize> = Vec::new();
    let mut span_of_member: Vec<(usize, usize)> = vec![(0, 0); members.len()];
    for (mi, &m) in members.iter().enumerate().rev() {
        let t = tables[m];
        let start = rows.len();
        for row in &t.rows {
            let mut cells = Vec::with_capacity(cols.len());
            let mut cell_spans = Vec::with_capacity(cols.len());
            for (cn, _) in &cols {
                match t.inputs.iter().position(|(n, _)| n == cn) {
                    Some(ci) => {
                        cells.push(row.cells.get(ci).cloned().unwrap_or(Cell::DontCare));
                        cell_spans.push(row.cell_spans.get(ci).cloned().unwrap_or(row.span.clone()));
                    }
                    None => {
                        cells.push(Cell::DontCare);
                        cell_spans.push(row.span.clone());
                    }
                }
            }
            rows.push(Row {
                cells,
                cell_spans,
                outs: row.outs.clone(),
                out_spans: row.out_spans.clone(),
                span: row.span.clone(),
                index: row.index,
                label: row.label.clone(),
                origin: Some(names[mi].clone()),
                cite: None,
            });
            member_of.push(mi);
        }
        span_of_member[mi] = (start, rows.len());
    }

    let n = rows.len();
    let mut beats: Vec<Vec<usize>> = vec![Vec::new(); n];
    // Position order inside a `first` member.
    for (mi, &m) in members.iter().enumerate() {
        if tables[m].policy == Policy::TopDown {
            let (s, e) = span_of_member[mi];
            for j in s..e {
                beats[j].extend(s..j);
            }
        }
    }
    // The declared edges.
    let mut edges: Vec<Edge> = Vec::new();
    for r in raw {
        let (Some(wi), Some(li)) = (members.iter().position(|&m| m == r.winner), members.iter().position(|&m| m == r.loser)) else {
            continue;
        };
        let (ws, we) = span_of_member[wi];
        let (ls, le) = span_of_member[li];
        let loser_row = r.loser_label.as_ref().and_then(|l| (ls..le).find(|&j| rows[j].label.as_ref().is_some_and(|x| x.text == *l)));
        let losers: Vec<usize> = match loser_row {
            Some(j) => vec![j],
            None => (ls..le).collect(),
        };
        for &l in &losers {
            beats[l].extend(ws..we);
        }
        edges.push(Edge { winner: wi, loser: li, loser_row, span: r.span.clone() });
    }
    // Transitive closure. Small, so the plain iteration is fine.
    loop {
        let mut changed = false;
        for i in 0..n {
            let mut add: Vec<usize> = Vec::new();
            for &w in &beats[i] {
                for &ww in &beats[w] {
                    if !beats[i].contains(&ww) && !add.contains(&ww) {
                        add.push(ww);
                    }
                }
            }
            if !add.is_empty() {
                beats[i].extend(add);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for b in beats.iter_mut() {
        b.sort_unstable();
        b.dedup();
    }

    let first = tables[members[0]];
    let table = Table {
        name: Some(Name { text: key.clone(), ascii: out.name.ascii.clone(), span: first.span.clone() }),
        policy: Policy::Unique,
        inputs: cols,
        outputs: vec![out],
        rows,
        span: first.span.clone(),
        overrides: Vec::new(),
        clause: false,
        cite: None,
        applied: None,
    };
    let eval_at = names[members.len() - 1].clone();
    DefSet { key, members: names, policies, clause, applied, table, member_of, beats, edges, eval_at }
}
