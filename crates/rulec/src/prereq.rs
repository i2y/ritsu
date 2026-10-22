//! Which values of an enum need which before them (§15.222).
//!
//! A `prerequisites` block is a finite graph over the values of one enum: each value is had from
//! the start, needs some groups of others (every group, each by any one of its values), or can
//! never be had. Everything said about it is a property of that graph, found by walking it once
//! or twice, so the checks always end and their answers are exact:
//!
//! - E071: the block is misshapen — a name that is no value, a value written twice or not at all,
//!   an `ordered by` whose table cannot order the values.
//! - E136: a cycle. Every alternative of an `or` counts as an edge.
//! - E137: a value that cannot be reached from `start`: what is reached is the closure of the
//!   start values under "every group has a reached value", and a value outside it waits on one
//!   that can never be had. Only the first such value on each way is said, with the ones behind
//!   it.
//! - E138: a value whose `ordered by` answer is below a prerequisite's.
//! - W130: a prerequisite another prerequisite of the same value already needs. The values
//!   inside an `or` are left out on both sides.
//!
//! The same graph answers what the generated code answers: the groups of a value
//! ([`Graph::requires`]), and whether a value can be taken with what is held
//! ([`Graph::available`]). The vectors are built from these.

use crate::ast::{FileKind, Item, Lit, Needs, OutCell, PrereqDecl, RuleFile, Table};
use crate::diag::{Diag, Span};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use std::collections::{BTreeMap, BTreeSet};

/// One block read over its enum: what each value needs.
#[derive(Debug, Clone)]
pub struct Graph {
    /// The block's name, and the identifier its functions are named after (its alias, or the
    /// name itself).
    pub name: String,
    pub alias: String,
    /// The enum, and its values in the order declared.
    pub over: String,
    pub values: Vec<String>,
    /// The values that need nothing, in the order written.
    pub start: Vec<String>,
    /// What a value with a `requires` line needs: every group, each by any one of its values.
    pub groups: BTreeMap<String, Vec<Vec<String>>>,
    /// The values no case can take, with the reason written beside each.
    pub never: BTreeMap<String, String>,
    /// The table `ordered by` names, when the block has one.
    pub ordered_by: Option<String>,
}

/// What the order of an `ordered by` table is made of: the places of an ordered enum, or numbers.
#[derive(Debug, Clone, PartialEq)]
pub enum Order {
    /// The answer's enum, its values in their order, and each value's answer.
    Enum { en: String, order: Vec<String>, of: BTreeMap<String, String> },
    /// Each value's number.
    Number { of: BTreeMap<String, Rat> },
}

impl Order {
    /// A value's answer as the table writes it.
    pub fn shown(&self, v: &str) -> String {
        match self {
            Order::Enum { of, .. } => of.get(v).cloned().unwrap_or_default(),
            Order::Number { of } => of.get(v).map(|r| r.to_string()).unwrap_or_default(),
        }
    }

    /// Whether `v`'s answer is below `p`'s. A value without an answer is below nothing.
    pub fn below(&self, v: &str, p: &str) -> bool {
        match self {
            Order::Enum { order, of, .. } => {
                let place = |x: &str| of.get(x).and_then(|a| order.iter().position(|o| o == a));
                matches!((place(v), place(p)), (Some(a), Some(b)) if a < b)
            }
            Order::Number { of } => matches!((of.get(v), of.get(p)), (Some(a), Some(b)) if a.cmp_to(*b) == std::cmp::Ordering::Less),
        }
    }
}

impl Graph {
    /// The block over the enum it names, or `None` when the enum is not one the rule has. Names
    /// that are no value of the enum are kept as written: E071 says so, and nothing is built from
    /// a block that does not check.
    pub fn of(d: &PrereqDecl, c: &Checked) -> Option<Graph> {
        let over = d.over.as_ref()?.text.clone();
        let values = c.enums.get(&over)?.clone();
        let mut groups = BTreeMap::new();
        let mut never = BTreeMap::new();
        for l in &d.lines {
            match &l.needs {
                Needs::Groups(gs) => {
                    groups.insert(l.value.text.clone(), gs.iter().map(|g| g.iter().map(|n| n.text.clone()).collect()).collect());
                }
                Needs::Nothing { reason, .. } => {
                    never.insert(l.value.text.clone(), reason.clone());
                }
            }
        }
        Some(Graph {
            name: d.name.text.clone(),
            alias: d.name.ascii.clone().unwrap_or_else(|| d.name.text.clone()),
            over,
            values,
            start: d.start.iter().map(|n| n.text.clone()).collect(),
            groups,
            never,
            ordered_by: d.ordered_by.as_ref().map(|(n, _)| n.text.clone()),
        })
    }

    /// Every block of the file that reads, in the order written.
    pub fn all(f: &RuleFile, c: &Checked) -> Vec<Graph> {
        f.prerequisites.iter().filter_map(|d| Graph::of(d, c)).collect()
    }

    /// What `v` needs, as the generated `<name>_requires` returns it: every group, each by one of
    /// its values. A value had from the start needs none (`[]`); one no case can take has a group
    /// no value is in (`[[]]`), which nothing held can meet.
    pub fn requires(&self, v: &str) -> Vec<Vec<String>> {
        if self.never.contains_key(v) {
            return vec![Vec::new()];
        }
        self.groups.get(v).cloned().unwrap_or_default()
    }

    /// Whether `v` can be taken with `held`, as the generated `<name>_available` answers: it is not
    /// held yet, and every group of what it needs has a value in `held`.
    pub fn available(&self, v: &str, held: &[String]) -> bool {
        !held.iter().any(|h| h == v) && self.requires(v).iter().all(|g| g.iter().any(|p| held.contains(p)))
    }

    /// Every edge from a value to one it may need, each alternative of an `or` too, in the order
    /// of the enum and of the line.
    pub fn edges(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for v in &self.values {
            for g in self.groups.get(v).into_iter().flatten() {
                for p in g {
                    if !out.contains(&(v.clone(), p.clone())) {
                        out.push((v.clone(), p.clone()));
                    }
                }
            }
        }
        out
    }

    /// The values a value may need, every alternative of an `or` included.
    fn needs_of(&self, v: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for g in self.groups.get(v).into_iter().flatten() {
            for p in g {
                if !out.contains(p) {
                    out.push(p.clone());
                }
            }
        }
        out
    }

    /// The cycles, one for each set of values that reach one another: the values of the set in
    /// the enum's order, and one way round it from the first of them, back to it. A value that
    /// needs itself is a set of one.
    pub fn cycles(&self) -> Vec<(Vec<String>, Vec<String>)> {
        // Tarjan's strongly connected components, iterative over the enum's order.
        let n = self.values.len();
        let idx = |v: &str| self.values.iter().position(|x| x == v);
        let adj: Vec<Vec<usize>> = self.values.iter().map(|v| self.needs_of(v).iter().filter_map(|p| idx(p)).collect()).collect();
        let mut index = vec![usize::MAX; n];
        let mut low = vec![0usize; n];
        let mut on = vec![false; n];
        let mut stack: Vec<usize> = Vec::new();
        let mut next = 0usize;
        let mut comps: Vec<Vec<usize>> = Vec::new();
        for root in 0..n {
            if index[root] != usize::MAX {
                continue;
            }
            let mut work: Vec<(usize, usize)> = vec![(root, 0)];
            while let Some(&(v, k)) = work.last() {
                if k == 0 && index[v] == usize::MAX {
                    index[v] = next;
                    low[v] = next;
                    next += 1;
                    stack.push(v);
                    on[v] = true;
                }
                if k < adj[v].len() {
                    let w = adj[v][k];
                    if let Some(top) = work.last_mut() {
                        top.1 += 1;
                    }
                    if index[w] == usize::MAX {
                        work.push((w, 0));
                    } else if on[w] {
                        low[v] = low[v].min(index[w]);
                    }
                    continue;
                }
                work.pop();
                if let Some(&(parent, _)) = work.last() {
                    low[parent] = low[parent].min(low[v]);
                }
                if low[v] == index[v] {
                    let mut comp = Vec::new();
                    while let Some(w) = stack.pop() {
                        on[w] = false;
                        comp.push(w);
                        if w == v {
                            break;
                        }
                    }
                    comps.push(comp);
                }
            }
        }
        let mut out = Vec::new();
        for mut comp in comps {
            comp.sort();
            let first = comp[0];
            let looped = comp.len() > 1 || adj[first].contains(&first);
            if !looped {
                continue;
            }
            // The shortest way from the first value back to itself, inside the set.
            let inside: BTreeSet<usize> = comp.iter().copied().collect();
            let mut prev: BTreeMap<usize, usize> = BTreeMap::new();
            let mut queue = std::collections::VecDeque::from([first]);
            let mut seen = BTreeSet::from([first]);
            let mut back = None;
            'bfs: while let Some(v) = queue.pop_front() {
                for &w in &adj[v] {
                    if w == first {
                        back = Some(v);
                        break 'bfs;
                    }
                    if inside.contains(&w) && seen.insert(w) {
                        prev.insert(w, v);
                        queue.push_back(w);
                    }
                }
            }
            let mut way = vec![first];
            if let Some(mut v) = back {
                let mut rev = Vec::new();
                while v != first {
                    rev.push(v);
                    v = prev[&v];
                }
                rev.reverse();
                way.extend(rev);
            }
            way.push(first);
            out.push((comp.iter().map(|&i| self.values[i].clone()).collect(), way.iter().map(|&i| self.values[i].clone()).collect()));
        }
        out.sort();
        out
    }

    /// The values in an order where every value comes after each one it may need, every
    /// alternative of an `or` included: what the certificate shows the graph has no cycle by.
    /// `None` when it has one.
    pub fn topological(&self) -> Option<Vec<String>> {
        let mut out: Vec<String> = Vec::new();
        let mut left: Vec<String> = self.values.clone();
        while !left.is_empty() {
            let ready: Vec<String> = left.iter().filter(|v| self.needs_of(v).iter().all(|p| out.contains(p))).cloned().collect();
            if ready.is_empty() {
                return None;
            }
            left.retain(|v| !ready.contains(v));
            out.extend(ready);
        }
        Some(out)
    }

    /// The values reached from `start`, in the order they are: the start values, then wave after
    /// wave of the values every group of which has a value reached before. A value no case can
    /// take is never reached, nor is one that waits on it.
    pub fn reached(&self) -> Vec<String> {
        let mut out: Vec<String> = self.values.iter().filter(|v| self.start.contains(v)).cloned().collect();
        loop {
            let wave: Vec<String> = self
                .values
                .iter()
                .filter(|v| !out.contains(v) && !self.never.contains_key(*v))
                .filter(|v| self.groups.get(*v).is_some_and(|gs| gs.iter().all(|g| g.iter().any(|p| out.contains(p)))))
                .cloned()
                .collect();
            if wave.is_empty() {
                return out;
            }
            out.extend(wave);
        }
    }

    /// The values reached from `start`, wave by wave: the start values, then the values every
    /// group of which has a value in an earlier wave. What the page lays the values out by when no
    /// table orders them.
    pub fn waves(&self) -> Vec<Vec<String>> {
        let mut out: Vec<Vec<String>> = vec![self.values.iter().filter(|v| self.start.contains(v)).cloned().collect()];
        let mut seen: Vec<String> = out[0].clone();
        loop {
            let wave: Vec<String> = self
                .values
                .iter()
                .filter(|v| !seen.contains(v) && !self.never.contains_key(*v))
                .filter(|v| self.groups.get(*v).is_some_and(|gs| gs.iter().all(|g| g.iter().any(|p| seen.contains(p)))))
                .cloned()
                .collect();
            if wave.is_empty() {
                return out;
            }
            seen.extend(wave.iter().cloned());
            out.push(wave);
        }
    }

    /// The values a value cannot be had without, through the groups of one value each: what is
    /// read for W130, which leaves the values inside an `or` out.
    fn must(&self, v: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut todo = vec![v.to_string()];
        while let Some(x) = todo.pop() {
            for g in self.groups.get(&x).into_iter().flatten() {
                if let [p] = g.as_slice() {
                    if out.insert(p.clone()) {
                        todo.push(p.clone());
                    }
                }
            }
        }
        out
    }

    /// The prerequisites another prerequisite of the same value already needs: (the value, the
    /// prerequisite, the one that needs it), each once, in the order of the enum and the line.
    pub fn redundant(&self) -> Vec<(String, String, String)> {
        let mut out = Vec::new();
        for v in &self.values {
            let Some(gs) = self.groups.get(v) else { continue };
            let singles: Vec<&String> = gs.iter().filter_map(|g| if let [p] = g.as_slice() { Some(p) } else { None }).collect();
            for p in &singles {
                if let Some(w) = singles.iter().find(|w| *w != p && self.must(w).contains(*p)) {
                    out.push((v.clone(), (*p).clone(), (*w).clone()));
                }
            }
        }
        out
    }
}

/// The order an `ordered by` table gives the values of its block, from its rows; `Err` says why
/// the table cannot give one (E071).
pub fn order_of(f: &RuleFile, c: &Checked, g: &Graph) -> Option<Result<Order, String>> {
    let tname = g.ordered_by.as_ref()?;
    Some(read_order(f, c, g, tname))
}

fn read_order(f: &RuleFile, c: &Checked, g: &Graph, tname: &str) -> Result<Order, String> {
    if f.kind == FileKind::Enums {
        return Err(tr!(
            "`{}` のファイルには表が無いので、`ordered by` は書けません。表を持つ規則の側に書いてください",
            "an `{}` file has no tables, so it cannot have `ordered by`; write it in a rule that has the table",
            crate::kw::ENUMS
        ));
    }
    let Some(t) = f.items.iter().find_map(|it| match it {
        Item::Table(t) if t.applied.is_none() && t.name.as_ref().is_some_and(|n| n.text == tname) => Some(t),
        _ => None,
    }) else {
        return Err(tr!("表 {tname} がありません", "there is no table {tname}"));
    };
    // A name is declared above every line that uses it (no forward reference).
    if let Some(d) = f.prerequisites.iter().find(|d| d.name.text == g.name) {
        if t.span.line > d.span.line {
            return Err(tr!(
                "表 {tname} は前提のつながりより下にあります。名前は、使う行より上で宣言します",
                "table {tname} is declared below the block; a name is declared above the lines that use it"
            ));
        }
    }
    // The definition set the table is evaluated in (§15.66), which is the table itself when it is
    // the only one that defines its output.
    let t: &Table = c.table_at(t).unwrap_or(t);
    let cols: Vec<&str> = t.inputs.iter().map(|(n, _)| n.as_str()).collect();
    let over_ty = Ty::Enum(g.over.clone());
    if cols.len() != 1 || c.ty_of(cols[0]).as_ref() != Some(&over_ty) {
        return Err(tr!(
            "表 {tname} の条件の列は、列挙 {} の列一つだけでなければなりません（いまは {}）",
            "the columns table {tname} tests must be one column of enum {} and nothing else (it tests {})",
            g.over,
            cols.join(", ")
        ));
    }
    if t.outputs.len() != 1 {
        return Err(tr!("表 {tname} が出す列は一つでなければなりません", "table {tname} must write one column"));
    }
    let out = &t.outputs[0].name.text;
    let oty = c.ty_of(out).unwrap_or(Ty::Unknown);
    let ordered_en = c.ordered_enum(&oty).map(|s| s.to_string()).filter(|_| !matches!(oty, Ty::Opt(_)));
    if ordered_en.is_none() && oty != Ty::Number {
        return Err(tr!(
            "表 {tname} が出す {out} は {oty} です。順序のある列挙（`<` で区切った列挙）か `number` でなければ、値の順になりません",
            "table {tname} writes {out}, a {oty}; it has to be an ordered enum (one whose values are separated by `<`) or a `number` to order the values"
        ));
    }
    let mut enum_of: BTreeMap<String, String> = BTreeMap::new();
    let mut num_of: BTreeMap<String, Rat> = BTreeMap::new();
    for v in &g.values {
        let Some(row) = t.rows.iter().find(|r| r.cells.first().is_some_and(|cell| crate::eval::cell_matches(c, cell, &crate::eval::Val::Enum(v.clone()), &over_ty))) else {
            continue;
        };
        let written = match row.outs.first() {
            Some(OutCell::Lit(l)) => crate::eval::lit_to_val(l, &oty),
            // A word no name shares is a value of the column's enum (§15.208).
            Some(OutCell::Name(w)) if c.syms.get(w).is_none() => crate::eval::lit_to_val(&Lit::Word(w.clone()), &oty),
            _ => None,
        };
        match (written, &ordered_en) {
            (Some(crate::eval::Val::Enum(a)), Some(_)) => {
                enum_of.insert(v.clone(), a);
            }
            (Some(crate::eval::Val::Num(r)), None) => {
                num_of.insert(v.clone(), r);
            }
            _ => {
                return Err(tr!(
                    "表 {tname} の行 {} が {out} に値を書いていません。行には {out} の値をそのまま書いてください",
                    "row {} of table {tname} does not write a value into {out}; write the value itself in each row",
                    row.index
                ))
            }
        }
    }
    Ok(match ordered_en {
        Some(en) => Order::Enum { order: c.enums.get(&en).cloned().unwrap_or_default(), en, of: enum_of },
        None => Order::Number { of: num_of },
    })
}

/// Where a finding about the block is: the file, the line, and the block.
fn at(path: &str, line: usize, name: &str) -> String {
    tr!("{path}:{line} 前提のつながり {name}", "{path}:{line} prerequisites {name}")
}

/// Whether a name needs an ASCII alias to name a generated function (§1.3), as a rule's does.
fn needs_alias(n: &crate::ast::Name) -> bool {
    if n.ascii.is_some() {
        return false;
    }
    let mut cs = n.text.chars();
    let first_ok = cs.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    !(first_ok && cs.all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

/// What is wrong with the shape of each block (E071), and with its name (E011). Said with the type
/// check: nothing below it is worth reading about a block that does not read. Two blocks of one name
/// were said before anything was typed (E034, `names`).
pub fn check_shape(f: &RuleFile, c: &Checked, path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    for d in &f.prerequisites {
        let name = d.name.text.as_str();
        let here = |line: usize| at(path, line, name);
        if needs_alias(&d.name) {
            out.push(
                Diag::error("E011", tr!("公開される名前に ASCII の別名がありません", "A public name has no ASCII alias"))
                    .at(here(d.span.line))
                    .fix_kind(crate::diag::FixKind::AddAlias)
                    .mark(d.name.span.clone(), "")
                    .note(tr!("不足: 前提のつながり {name}", "Missing: prerequisites {name}"))
                    .note(tr!(
                        "名前は生成する関数（`<別名>_requires`、`<別名>_available`）の名前になります。宣言の位置に丸括弧で書いてください。例: 履修(enrollment)",
                        "The name names the generated functions (`<alias>_requires`, `<alias>_available`). Write it in parentheses at the declaration, e.g. 履修(enrollment)"
                    )),
            );
        }
        let Some(over) = &d.over else { continue };
        let Some(values) = c.enums.get(&over.text) else {
            out.push(
                Diag::error("E071", tr!("前提のつながりの形が違います: {} は列挙ではありません", "The prerequisites are misshapen: {} is not an enum", over.text))
                    .at(here(d.span.line))
                    .mark(over.span.clone(), tr!("`over` の後には、規則が持つ列挙を書きます", "`over` takes an enum the rule has"))
                    .note(tr!(
                        "前提のつながりは、一つの列挙の値どうしの関係です。列挙を宣言するか、`import enum … from` で読んでください。",
                        "The prerequisites are about the values of one enum. Declare it, or read it with `import enum … from`."
                    )),
            );
            continue;
        };
        let misshapen = |title: String, sp: Span, what: String| -> Diag {
            Diag::error("E071", tr!("前提のつながりの形が違います: {title}", "The prerequisites are misshapen: {title}")).at(here(sp.line)).mark(sp, what)
        };
        if d.start_span.is_none() {
            out.push(
                misshapen(tr!("`start` の行がありません", "there is no `start` line"), d.span.clone(), String::new()).note(tr!(
                    "`start <値>, …` に、何も要らない値を書いてください。ほかの値は、そこからたどり着けるかを確かめます。",
                    "Name the values that need nothing on a `start <value>, …` line: whether every other value can be reached is worked out from there."
                )),
            );
        }
        // Every name a value of the enum.
        let unknown = |n: &crate::ast::Name, out: &mut Vec<Diag>| {
            if values.contains(&n.text) {
                return true;
            }
            let mut d = misshapen(
                tr!("{} は列挙 {} の値ではありません", "{} is not a value of enum {}", n.text, over.text),
                n.span.clone(),
                String::new(),
            )
            .note(tr!("{} の値: {}", "The values of {}: {}", over.text, values.join(", ")));
            if n.text == crate::kw::NOTHING {
                d = d.note(tr!(
                    "手に入らない値なら、`requires nothing \"<理由>\"` と理由を添えて書いてください。",
                    "For a value no case can take, write `requires nothing \"<reason>\"`, with the reason."
                ));
            }
            out.push(d);
            false
        };
        // Each value once: on the `start` line, or on a line of its own.
        let mut seen: BTreeMap<String, Span> = BTreeMap::new();
        let mut twice = |n: &crate::ast::Name, out: &mut Vec<Diag>| {
            if let Some(first) = seen.get(&n.text) {
                out.push(
                    misshapen(tr!("値 {} を二度書いています", "value {} is written twice", n.text), n.span.clone(), tr!("二度目です", "this is the second time"))
                        .note(tr!(
                            "{} 行目にも書いてあります。値は、`start` か自分の `requires` の行の、どちらか一か所に書きます。",
                            "It is on line {} too. A value is written in one place: on the `start` line, or on a `requires` line of its own.",
                            first.line
                        )),
                );
            } else {
                seen.insert(n.text.clone(), n.span.clone());
            }
        };
        for n in &d.start {
            if unknown(n, &mut out) {
                twice(n, &mut out);
            }
        }
        for l in &d.lines {
            if unknown(&l.value, &mut out) {
                twice(&l.value, &mut out);
            }
            if let Needs::Groups(gs) = &l.needs {
                let mut named: Vec<&str> = Vec::new();
                for p in gs.iter().flatten() {
                    if !unknown(p, &mut out) {
                        continue;
                    }
                    if named.contains(&p.text.as_str()) {
                        out.push(
                            misshapen(tr!("{} の前提に {} を二度書いています", "{} names {} twice among what it needs", l.value.text, p.text), p.span.clone(), tr!("二度目です", "this is the second time"))
                                .note(tr!("一つの行に、同じ値は一度だけ書きます。", "A line names a value once.")),
                        );
                    }
                    named.push(&p.text);
                }
            }
        }
        let missing: Vec<&String> = values.iter().filter(|v| !seen.contains_key(*v)).collect();
        if !missing.is_empty() {
            out.push(
                misshapen(
                    tr!("行の無い値があります: {}", "some values have no line: {}", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")),
                    d.span.clone(),
                    tr!("列挙 {} の値は、どれも `start` の行か自分の行に書きます", "every value of enum {} is on the `start` line or has a line of its own", over.text),
                )
                .note(tr!(
                    "値を足したときに前提を書き忘れると、何も要らない値として通ってしまいます。何も要らない値は `start` に、手に入らない値は `<値> requires nothing \"<理由>\"` と書いてください。",
                    "A value added without its prerequisites would pass as one that needs nothing. Put a value that needs nothing on the `start` line, and write a value no case can take as `<value> requires nothing \"<reason>\"`."
                )),
            );
        }
        // The table that orders the values, when there is one.
        if let (Some((tn, sp)), Some(g)) = (&d.ordered_by, Graph::of(d, c)) {
            if let Some(Err(why)) = order_of(f, c, &g) {
                out.push(
                    misshapen(tr!("`ordered by {}` で値を並べられません", "`ordered by {}` cannot order the values", tn.text), sp.clone(), why).note(tr!(
                        "`ordered by` の表は、列挙 {} の列一つを条件にし、値ごとに順序のある列挙か `number` の値を一つ書く表です。",
                        "The table `ordered by` names tests one column of enum {} and writes, for each value, one value of an ordered enum or one `number`.",
                        over.text
                    )),
                );
            }
        }
    }
    out
}

/// What the graph of each block says once the tables hold (E136, E137, E138, W130). A block that
/// does not read (E071) has already been said, and is not looked at again.
pub fn check_graph(f: &RuleFile, c: &Checked, path: &str, tables_hold: bool) -> Vec<Diag> {
    let mut out = Vec::new();
    for d in &f.prerequisites {
        let Some(g) = Graph::of(d, c) else { continue };
        let here = |line: usize| at(path, line, &g.name);
        let line_of = |v: &str| d.line_of(v).map(|l| l.span.clone()).unwrap_or_else(|| d.span.clone());
        // Where a value is named on a value's line.
        let named_at = |v: &str, p: &str| -> Span {
            d.line_of(v)
                .and_then(|l| match &l.needs {
                    Needs::Groups(gs) => gs.iter().flatten().find(|n| n.text == p).map(|n| n.span.clone()),
                    Needs::Nothing { .. } => None,
                })
                .unwrap_or_else(|| line_of(v))
        };
        let cycles = g.cycles();
        for (set, way) in &cycles {
            let mut diag = Diag::error("E136", tr!("前提が循環しています: {}", "The prerequisites go round in a cycle: {}", way.join(" → ")))
                .at(here(line_of(&way[0]).line))
                .mark(named_at(&way[0], &way[1]), tr!("ここから輪が始まります", "the cycle starts here"))
                .note(tr!(
                    "どの値にも、矢印の先の値が要ります。輪の上の値は、どれもほかの値を先に持っていないと手に入らないので、輪のどこからも始められません。",
                    "Each value needs the one its arrow points to. Every value on the cycle waits for another on it, so the cycle cannot be entered anywhere."
                ));
            if set.len() + 1 > way.len() {
                diag = diag.note(tr!("互いにたどれる値: {}", "The values that reach one another: {}", set.join(", ")));
            }
            out.push(
                diag.note(tr!(
                    "`or` で別の値から入れる場合も、輪は前提の読み違いのもとなので、エラーにしています。輪のどこかの前提を外してください。",
                    "Even where another value of an `or` leads in, a cycle is where prerequisites are misread, so it is an error. Drop one of the prerequisites on it."
                )),
            );
        }
        // What cannot be reached waits on a cycle (said above) or on a value no case can take.
        if cycles.is_empty() {
            let reached: BTreeSet<String> = g.reached().into_iter().collect();
            let unreached: Vec<&String> = g.values.iter().filter(|v| !reached.contains(*v) && !g.never.contains_key(*v)).collect();
            // A group none of whose values is reached, and each one's reason.
            let open_groups = |v: &str| -> Vec<Vec<String>> { g.requires(v).into_iter().filter(|gr| !gr.iter().any(|p| reached.contains(p))).collect() };
            // The first such value on a way: one with a group of values no case can take.
            let first: Vec<&String> = unreached.iter().copied().filter(|v| open_groups(v).iter().any(|gr| gr.iter().all(|p| g.never.contains_key(p)))).collect();
            for v in &first {
                // The values that wait on it.
                let mut behind: Vec<String> = Vec::new();
                for u in &unreached {
                    if first.contains(u) {
                        continue;
                    }
                    let mut todo = vec![(*u).clone()];
                    let mut seen = BTreeSet::new();
                    let mut hit = false;
                    while let Some(x) = todo.pop() {
                        if !seen.insert(x.clone()) {
                            continue;
                        }
                        for gr in open_groups(&x) {
                            for p in gr {
                                if p == **v {
                                    hit = true;
                                } else if !reached.contains(&p) && !g.never.contains_key(&p) {
                                    todo.push(p);
                                }
                            }
                        }
                    }
                    if hit {
                        behind.push((*u).clone());
                    }
                }
                let groups: Vec<Vec<String>> = open_groups(v).into_iter().filter(|gr| gr.iter().all(|p| g.never.contains_key(p))).collect();
                let blocked: Vec<String> = groups.iter().map(|gr| gr.join(tr!(" か ", " or ").as_str())).collect();
                let p0 = groups.iter().flatten().next().cloned().unwrap_or_default();
                let mut diag = Diag::error("E137", tr!("{v} は `start` からたどり着けません", "{v} cannot be reached from `start`"))
                    .at(here(line_of(v).line))
                    .mark(named_at(v, &p0), tr!("手に入らない値です", "no case can take this"))
                    .note(tr!("{v} には {} が要りますが、手に入らない値です。", "{v} needs {}, which no case can take.", blocked.join(tr!("、", ", ").as_str())));
                for p in groups.iter().flatten() {
                    diag = diag.note(format!("{p}: `{} {} \"{}\"`", crate::kw::REQUIRES, crate::kw::NOTHING, g.never[p]));
                }
                if !behind.is_empty() {
                    diag = diag.note(tr!("{v} にたどり着けないあいだは、{} にもたどり着けません。", "Until {v} can be reached, neither can {}.", behind.join(", ")));
                }
                out.push(diag.note(tr!(
                    "手に入らない前提を外すか、{v} も手に入らないなら `{v} requires nothing \"<理由>\"` と書いてください。",
                    "Drop the prerequisite no case can take, or if {v} cannot be had either, write `{v} requires nothing \"<reason>\"`."
                )));
            }
            for (v, p, w) in g.redundant() {
                out.push(
                    Diag::warning("W130", tr!("{v} の前提の {p} は、{w} の前提からたどれます", "{v} needs {p}, which its prerequisite {w} already needs"))
                        .at(here(line_of(&v).line))
                        .mark(named_at(&v, &p), tr!("ここは無くても同じです", "this changes nothing"))
                        .note(tr!(
                            "{w} を持っていれば {p} も持っているので、{p} を書いても書かなくても、{v} を取れるときは同じです。消すと、前提を直すときに書き直す所が一つ減ります。",
                            "Whoever holds {w} holds {p} too, so {v} can be taken at the same times with {p} written or not. Dropping it leaves one place less to correct when the prerequisites change."
                        )),
                );
            }
        }
        // The table's order, kept along every edge.
        // An edge on a cycle is E136's, and a value no case can take orders nothing.
        if tables_hold {
            if let Some(Ok(order)) = order_of(f, c, &g) {
                let tname = g.ordered_by.clone().unwrap_or_default();
                let looped = |v: &str, p: &str| cycles.iter().any(|(set, _)| set.iter().any(|x| x == v) && set.iter().any(|x| x == p));
                for (v, p) in g.edges() {
                    if order.below(&v, &p) && !looped(&v, &p) && !g.never.contains_key(&p) {
                        out.push(
                            Diag::error("E138", tr!("{v} の {tname} は {}、前提の {p} は {} です", "{tname} puts {v} at {}, and its prerequisite {p} at {}", order.shown(&v), order.shown(&p)))
                                .at(here(line_of(&v).line))
                                .mark(named_at(&v, &p), tr!("前提が後ろにあります", "this prerequisite comes later"))
                                .note(tr!(
                                    "`ordered by {tname}` は、どの値も前提より前に来ないことを言っています。表の答えか、前提のどちらかを直してください。",
                                    "`ordered by {tname}` says no value comes before what it needs. Correct the table's answer, or the prerequisite."
                                )),
                        );
                    }
                }
            }
        }
    }
    out
}

/// `prerequisites` in the certificate (§15.222): each block's graph, with the two orders that show
/// what `check` found — `topological`, every value after each one it may need, so the graph has
/// no cycle; `reached`, every value no case is kept from (`never`) after one value of each of its
/// groups, starting from `start`, so each can be reached — and, where `ordered by` names a table,
/// each value's answer from it, which a re-checker works out again from that table's rows in
/// `tables` and holds along every edge whose prerequisite can be had. `None` for a file with no
/// block, so no other certificate changes.
pub fn certificate_json(f: &RuleFile, c: &Checked) -> Option<String> {
    use crate::json::{arr, strs, Obj};
    if f.prerequisites.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for g in Graph::all(f, c) {
        let mut req = Obj::new();
        for v in g.values.iter().filter(|v| g.groups.contains_key(*v)) {
            req = req.raw(v, arr(&g.groups[v].iter().map(|gr| strs(gr)).collect::<Vec<_>>()));
        }
        let never: Vec<String> = g.values.iter().filter(|v| g.never.contains_key(*v)).cloned().collect();
        let ordered = match order_of(f, c, &g) {
            Some(Ok(order)) => {
                let tname = g.ordered_by.clone().unwrap_or_default();
                let column = f
                    .items
                    .iter()
                    .find_map(|it| match it {
                        Item::Table(t) if t.name.as_ref().is_some_and(|n| n.text == tname) => t.outputs.first().map(|o| o.name.text.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let mut vs = Obj::new();
                for v in &g.values {
                    let shown = order.shown(v);
                    if !shown.is_empty() {
                        vs = vs.str(v, &shown);
                    }
                }
                let o = Obj::new().str("table", &tname).str("column", &column);
                let o = match &order {
                    Order::Enum { en, order, .. } => o.str("kind", "enum").str("enum", en).raw("order", strs(order)),
                    Order::Number { .. } => o.str("kind", "number").raw("order", "null"),
                };
                o.raw("values", vs.finish()).finish()
            }
            _ => "null".to_string(),
        };
        out.push(
            Obj::new()
                .str("name", &g.name)
                .str("over", &g.over)
                .raw("start", strs(&g.start))
                .raw("requires", req.finish())
                .raw("never", strs(&never))
                .raw("topological", g.topological().map(|t| strs(&t)).unwrap_or_else(|| "null".into()))
                .raw("reached", strs(&g.reached()))
                .raw("ordered_by", ordered)
                .finish(),
        );
    }
    Some(arr(&out))
}
