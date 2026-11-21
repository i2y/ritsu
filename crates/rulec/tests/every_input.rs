//! Small rules made from fixed seeds, every input of each run through the reference evaluator, and
//! what the check says held to what the inputs come to (§15.322).
//!
//! Each rule has an amount, and beside it sometimes a kind (sometimes optional) or a quantity, and
//! a value worked out from them: a band or a fee a table of the same rule writes (`policy unique`,
//! or `policy first` with a catch-all), an output of a rule it applies (a word, read as it is or
//! over again with `with`, a number, an optional truth value), or a truth value or a number a
//! `define` works out (once from the amount to the yen, under a `range`, from the amount twice, to
//! a step of 5 or 10 yen, or from the amount and the quantity). Sometimes a second table reads that
//! value, and sometimes an input beside it, and writes a tier. The table `pick` reads the amount,
//! the value and whatever else there is, cut where the tables above cut or elsewhere, with cells of
//! one value, a set, `not:`, a range or `-`: as rows for exactly the combinations that occur, for
//! every combination, with one left out, with one that never occurs added, with a row over another,
//! and, in a `policy first` table, shuffled, with a wide row among them, or with a catch-all in
//! place of the last rows.
//!
//! What the inputs come to is the truth, and the check is held to it. A rule that passes has no
//! input that matches no row of a table, and none that matches two rows of a `policy unique` table.
//! A row E102 names is reached by no input. An example called an input is one: some input gives
//! its values and falls in the gap, or matches both rows; an example no input was found for is
//! called a combination and the witness holds nothing. Two rows E105 names meet on some input.
//! A rule whose every input finds its row, where no rows meet and every row is reached, passes, and
//! E101 asks for a row only where an input finds none: the check reads every shape here exactly
//! (§15.325). A row no input reaches is named, by E102 or W128.

use ritsu_testkit::TempDir;
use rulec::diag::{Diag, Severity, WVal};
use rulec::eval::Val;
use rulec::i18n::{self, Lang};
use rulec::num::Rat;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

/// SplitMix64: the same rules on every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as usize) as i64
    }
    fn chance(&mut self, p: f64) -> bool {
        ((self.next() >> 11) as f64) / ((1u64 << 53) as f64) < p
    }
    fn pick<T: Clone>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len())].clone()
    }
    fn shuffle<T>(&mut self, xs: &mut [T]) {
        for i in (1..xs.len()).rev() {
            let j = self.below(i + 1);
            xs.swap(i, j);
        }
    }
}

/// A value an input or a column takes: a whole number in its unit, or a word (`true`, `false` and
/// `none` among them).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum V {
    N(i64),
    W(String),
}

fn w(s: &str) -> V {
    V::W(s.to_string())
}

fn lit(v: &V, unit: &str) -> String {
    match v {
        V::N(n) => format!("{n}{unit}"),
        V::W(s) => s.clone(),
    }
}

/// A cell: any value, a range `a < x <= b` written one of four ways, one value, a set, or `not:`.
#[derive(Clone, Debug)]
enum Cell {
    Any,
    Range(Option<i64>, Option<i64>, u8),
    One(V),
    Set(Vec<V>),
    Not(Vec<V>),
}

impl Cell {
    fn holds(&self, v: &V) -> bool {
        match self {
            Cell::Any => true,
            Cell::Range(a, b, _) => match v {
                V::N(x) => a.is_none_or(|a| *x > a) && b.is_none_or(|b| *x <= b),
                V::W(_) => false,
            },
            Cell::One(o) => o == v,
            Cell::Set(s) => s.contains(v),
            Cell::Not(s) => !s.contains(v),
        }
    }
    fn text(&self, unit: &str) -> String {
        match self {
            Cell::Any => "-".into(),
            Cell::Range(a, b, style) => {
                let mut parts = Vec::new();
                if let Some(a) = a {
                    parts.push(if style % 2 == 0 { format!(">{a}{unit}") } else { format!(">={}{unit}", a + 1) });
                }
                if let Some(b) = b {
                    parts.push(if *style < 2 { format!("<={b}{unit}") } else { format!("<{}{unit}", b + 1) });
                }
                if parts.is_empty() { "-".into() } else { parts.join(" ") }
            }
            Cell::One(v) => lit(v, unit),
            Cell::Set(s) => s.iter().map(|v| lit(v, unit)).collect::<Vec<_>>().join(", "),
            Cell::Not(s) => format!("not: {}", s.iter().map(|v| lit(v, unit)).collect::<Vec<_>>().join(", ")),
        }
    }
}

/// Cells that split whole numbers into bands, cut at `cuts` or at up to `most` values of their own;
/// every band holds at least one of the values.
fn bands(rng: &mut Rng, values: &[i64], most: usize, cuts: Option<&[i64]>) -> (Vec<Cell>, Vec<i64>) {
    let (lo, hi) = (values[0], values[values.len() - 1]);
    let mut cuts: Vec<i64> = match cuts {
        Some(c) => c.to_vec(),
        None if values.len() > 1 => (0..rng.int(1, most as i64)).map(|_| values[rng.below(values.len() - 1)]).collect(),
        None => Vec::new(),
    };
    cuts.retain(|c| lo <= *c && *c < hi);
    cuts.sort();
    cuts.dedup();
    let mut out = Vec::new();
    let mut prev = None;
    for c in &cuts {
        out.push(Cell::Range(prev, Some(*c), rng.below(4) as u8));
        prev = Some(*c);
    }
    out.push(Cell::Range(prev, None, rng.below(4) as u8));
    (out, cuts)
}

/// Words split into cells: one each, or one and a set of the rest, or one and `not:` that one.
fn word_cells(rng: &mut Rng, values: &[V], sets: bool) -> Vec<Cell> {
    if sets && values.len() >= 3 && rng.chance(0.35) {
        let one = rng.pick(values);
        let rest: Vec<V> = values.iter().filter(|v| **v != one).cloned().collect();
        if rng.chance(0.5) {
            return vec![Cell::One(one), Cell::Set(rest)];
        }
        return vec![Cell::One(one.clone()), Cell::Not(vec![one])];
    }
    values.iter().cloned().map(Cell::One).collect()
}

/// What a column holds: whole numbers in a unit, or words.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Num,
    Words,
}

/// A table the rule writes, with its rows as cells: what the inputs come to is held to it.
struct Judged {
    table: String,
    reads: Vec<String>,
    rows: Vec<Vec<Cell>>,
    first: bool,
}

/// One rule's inputs and what is worked out above `pick`.
struct Family {
    label: String,
    /// Each input: its name, the values it takes, its unit and its declaration.
    inputs: Vec<(String, Vec<V>, &'static str, String)>,
    enums: Vec<String>,
    /// `define` and `apply` lines.
    items: String,
    /// The tables above `pick`.
    above: String,
    /// The rule an `apply` brings in: its file name and text.
    callee: Option<(String, String)>,
    /// The columns worked out, with their unit, what they hold and every value they may take.
    cols: Vec<(String, &'static str, Kind, Vec<V>)>,
    /// Where the table above cuts the amount.
    upper_cuts: Vec<i64>,
    /// A `define … range` is wanted: its ends are chosen once what the define comes to is known.
    ranged: bool,
    /// A `define … range`: the values outside refuse the call.
    range: Option<(String, i64, i64)>,
    judged: Vec<Judged>,
}

impl Family {
    fn unit_of(&self, name: &str) -> &'static str {
        self.inputs.iter().find(|i| i.0 == name).map(|i| i.2).or_else(|| self.cols.iter().find(|c| c.0 == name).map(|c| c.1)).unwrap_or("")
    }
    fn head(&self) -> String {
        let decl: Vec<&str> = self.inputs.iter().map(|i| i.3.as_str()).collect();
        format!("{}\n\ninputs\n{}\n{}", self.enums.join("\n"), decl.join("\n"), self.items)
    }
}

/// The shapes of what `pick` reads beside the inputs.
#[derive(Clone, Copy, Debug)]
enum Shape {
    Table { first: bool, words: bool },
    Apply { out: Out },
    DefineBool,
    DefineOnce { range: bool },
    DefineTwice,
    DefineStep,
    DefineQty,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Out {
    Word,
    WordWith,
    Num,
    Opt,
}

/// The rows of a table over `reads` that writes one of `values` (a grid of bands and words, in a
/// `policy first` table shuffled with its last rows given to a catch-all), and where it cuts the
/// amount.
fn upper_rows(rng: &mut Rng, fam: &Family, reads: &[String], values: &[V], first: bool) -> (Vec<(Vec<Cell>, V)>, Vec<i64>) {
    let mut per: Vec<Vec<Cell>> = Vec::new();
    let mut cuts = Vec::new();
    for n in reads {
        let (_, dom, _, _) = fam.inputs.iter().find(|i| i.0 == *n).unwrap();
        if let V::N(_) = dom[0] {
            let nums: Vec<i64> = dom.iter().map(|v| if let V::N(x) = v { *x } else { 0 }).collect();
            let (cells, c) = bands(rng, &nums, if reads.len() > 1 { 2 } else { 3 }, None);
            if n == "amount" {
                cuts = c;
            }
            per.push(cells);
        } else {
            per.push(word_cells(rng, dom, !dom.contains(&w("none"))));
        }
    }
    let mut rows: Vec<(Vec<Cell>, V)> = product(&per).into_iter().map(|g| (g, rng.pick(values))).collect();
    if first && rows.len() > 1 {
        rng.shuffle(&mut rows);
        let keep = rows.len().saturating_sub(rng.int(1, 2) as usize).max(1);
        rows.truncate(keep);
        rows.push((vec![Cell::Any; reads.len()], rng.pick(values)));
    }
    (rows, cuts)
}

fn product(per: &[Vec<Cell>]) -> Vec<Vec<Cell>> {
    let mut out: Vec<Vec<Cell>> = vec![Vec::new()];
    for cells in per {
        out = out.into_iter().flat_map(|g| cells.iter().map(move |c| [g.clone(), vec![c.clone()]].concat())).collect();
    }
    out
}

fn rows_text(reads: &[String], rows: &[(Vec<Cell>, V)], units: &[&str], out_unit: &str) -> String {
    rows.iter()
        .map(|(cells, v)| format!("| {} | {} |", cells.iter().zip(units).map(|(c, u)| c.text(u)).collect::<Vec<_>>().join(" | "), lit(v, out_unit)))
        .collect::<Vec<_>>()
        .join("\n")
        + if reads.is_empty() { "" } else { "\n" }
}

fn money(n: i64) -> String {
    format!("{n}JPY")
}

/// A family of one shape: the inputs, and what is worked out above `pick` but its second table.
fn family(rng: &mut Rng, shape: Shape) -> Family {
    let lo = rng.pick(&[1, 1, 0, -40]);
    let hi = lo + rng.int(40, 100);
    let mut fam = Family {
        label: String::new(),
        inputs: vec![("amount".into(), (lo..=hi).map(V::N).collect(), "JPY", format!("  amount : money[JPY]  range >={lo}JPY <={hi}JPY"))],
        enums: vec!["enum choice = a | b".into()],
        items: String::new(),
        above: String::new(),
        callee: None,
        cols: Vec::new(),
        upper_cuts: Vec::new(),
        ranged: false,
        range: None,
        judged: Vec::new(),
    };
    let second = rng.next() % 100;
    let qty_wanted = matches!(shape, Shape::DefineQty);
    if qty_wanted || (25..45).contains(&second) {
        fam.inputs.push(("qty".into(), (1..=4).map(V::N).collect(), "", "  qty : number  range >=1 <=4".into()));
    } else if second < 25 {
        let optional = rng.chance(0.3);
        let mut dom = vec![w("p"), w("q"), w("r")];
        if optional {
            dom.push(w("none"));
        }
        fam.inputs.push(("kind".into(), dom, "", format!("  kind : k{}", if optional { "?" } else { "" })));
        fam.enums.push("enum k = p | q | r".into());
    }
    let (p, s, k) = (rng.pick(&[1, 3, 7, 12, 30, 45]), rng.pick(&[5, 10]), rng.int(0, 40));
    match shape {
        Shape::DefineBool => {
            let k = rng.int(lo, hi);
            let op = rng.pick(&[">", ">=", "<", "<="]);
            fam.items = format!("\ndefine big : bool = amount {op} {k}JPY\n");
            fam.cols.push(("big".into(), "", Kind::Words, vec![w("true"), w("false")]));
            fam.label = format!("define bool {op}");
        }
        Shape::DefineOnce { range } => {
            let expr = rng.pick(&["down(amount * {p}%, 1JPY)", "up(amount * {p}%, 1JPY)", "half_up(amount * {p}%, 1JPY)", "{k}JPY - down(amount * {p}%, 1JPY)", "max(down(amount * {p}%, 1JPY), {k}JPY)"]);
            let expr = expr.replace("{p}", &p.to_string()).replace("{k}", &k.to_string());
            fam.items = format!("\ndefine fee : money[JPY] = {expr}\n");
            fam.cols.push(("fee".into(), "JPY", Kind::Num, Vec::new()));
            fam.label = format!("define once {}", expr.replace(&format!("{p}%"), "N%").replace(&format!("{k}JPY"), "NJPY"));
            if range {
                fam.label = "define once, under a range".into();
                fam.ranged = true;
            }
        }
        Shape::DefineTwice => {
            let expr = rng.pick(&["amount - down(amount * {p}%, 1JPY)", "max(amount, {k}JPY) - amount", "down(amount * {p}%, 1JPY) - up(amount * 2%, 1JPY)"]);
            let label = expr.replace("{p}", "N").replace("{k}", "N");
            let expr = expr.replace("{p}", &p.to_string()).replace("{k}", &k.to_string());
            fam.items = format!("\ndefine fee : money[JPY] = {expr}\n");
            fam.cols.push(("fee".into(), "JPY", Kind::Num, Vec::new()));
            fam.label = format!("define twice {label}");
        }
        Shape::DefineStep => {
            let round = rng.pick(&["down", "up", "half_up", "half_even"]);
            fam.items = format!("\ndefine fee : money[JPY] = {round}(amount * {p}%, {s}JPY)\n");
            fam.cols.push(("fee".into(), "JPY", Kind::Num, Vec::new()));
            fam.label = format!("define step {round}");
        }
        Shape::DefineQty => {
            let expr = rng.pick(&["amount * qty", "down(amount * {p}%, 1JPY) * qty", "amount - qty * {k}JPY"]);
            let label = expr.replace("{p}", "N").replace("{k}", "N");
            let expr = expr.replace("{p}", &p.to_string()).replace("{k}", &k.to_string());
            fam.items = format!("\ndefine fee : money[JPY] = {expr}\n");
            fam.cols.push(("fee".into(), "JPY", Kind::Num, Vec::new()));
            fam.label = format!("define qty {label}");
        }
        Shape::Table { first, words } => {
            let mut reads = vec!["amount".to_string()];
            for extra in ["kind", "qty"] {
                if fam.inputs.iter().any(|i| i.0 == extra) && rng.chance(0.5) {
                    reads.push(extra.into());
                }
            }
            let (values, unit, ty, name) = if words {
                fam.enums.push("enum size = small | medium | large".into());
                (vec![w("small"), w("medium"), w("large")], "", "size".to_string(), "band")
            } else {
                let mut v: Vec<V> = (0..3).map(|_| V::N(rng.int(0, 30) * 10)).collect();
                v.sort();
                v.dedup();
                (v, "JPY", "money[JPY]".to_string(), "fee")
            };
            let (rows, cuts) = upper_rows(rng, &fam, &reads, &values, first);
            fam.upper_cuts = cuts;
            let units: Vec<&str> = reads.iter().map(|r| fam.unit_of(r)).collect();
            fam.above = format!(
                "\ntable {name}_of\n{}| {} | -> {name} : {ty} |\n{}",
                if first { "policy first\n" } else { "" },
                reads.join(" | "),
                rows_text(&reads, &rows, &units, unit)
            );
            fam.judged.push(Judged { table: format!("{name}_of"), reads: reads.clone(), rows: rows.iter().map(|r| r.0.clone()).collect(), first });
            fam.cols.push((name.into(), unit, if words { Kind::Words } else { Kind::Num }, values));
            fam.label = format!("table{} {}{}", if first { " first" } else { "" }, if words { "word" } else { "num" }, reads[1..].iter().map(|r| format!(" +{r}")).collect::<String>());
        }
        Shape::Apply { out } => {
            let reads = vec!["amount".to_string()];
            let (values, unit, ty): (Vec<V>, &str, &str) = match out {
                Out::Word | Out::WordWith => (vec![w("small"), w("medium"), w("large")], "", "size"),
                Out::Num => {
                    let mut v: Vec<V> = (0..3).map(|_| V::N(rng.int(0, 30) * 10)).collect();
                    v.sort();
                    v.dedup();
                    (v, "JPY", "money[JPY]  round down(1JPY)")
                }
                Out::Opt => (vec![w("none"), w("false"), w("true")], "", "bool?"),
            };
            let (rows, cuts) = upper_rows(rng, &fam, &reads, &values, false);
            fam.upper_cuts = cuts;
            let callee = format!(
                "rule callee v1\n\n{}inputs\n{}\n\noutputs\n  got : {ty}\n\ntable t\n| amount | -> got |\n{}",
                if ty == "size" { "enum size = small | medium | large\n\n" } else { "" },
                fam.inputs[0].3,
                rows_text(&reads, &rows, &["JPY"], unit)
            );
            let pin = rulec::link::pin::meaning(&callee);
            let (with, col_values) = if out == Out::WordWith {
                fam.enums.push("enum grade = low | mid | high".into());
                (" with small -> low, medium -> mid, large -> high", vec![w("low"), w("mid"), w("high")])
            } else {
                ("", values.clone())
            };
            fam.items = format!("\napply t = \"callee.rule\" meaning:{pin}\n  amount = amount\n  got -> t_got{with}\n");
            fam.callee = Some(("callee.rule".into(), callee));
            fam.cols.push(("t_got".into(), unit, if unit.is_empty() { Kind::Words } else { Kind::Num }, col_values));
            fam.label = format!("apply {}", match out {
                Out::Word => "word",
                Out::WordWith => "word with",
                Out::Num => "num",
                Out::Opt => "optional",
            });
        }
    }
    if fam.inputs.iter().any(|i| i.0 == "kind" && i.3.ends_with('?')) {
        fam.label.push_str(" (kind optional)");
    }
    fam
}

/// Every input of the family with what it comes to: the inputs and every column worked out, as
/// the reference evaluator gives them. An input past a `define … range` refuses the call and is
/// left out.
fn run_every_input(fam: &Family, dir: &Path, above: &str) -> Vec<BTreeMap<String, V>> {
    let src = format!("rule probe v1\n\n{}\noutputs\n  x : choice\n{above}\ntable pick\n| amount | -> x |\n| - | a |\n", fam.head());
    let path = dir.join("probe.rule").to_string_lossy().into_owned();
    std::fs::write(&path, &src).unwrap();
    let (f, c) = rulec::prepare(&src, &path).unwrap_or_else(|e| panic!("the probe does not read: {e:?}\n{src}"));
    let names: Vec<&String> = fam.inputs.iter().map(|i| &i.0).collect();
    let doms: Vec<&Vec<V>> = fam.inputs.iter().map(|i| &i.1).collect();
    let mut points = Vec::new();
    let mut at = vec![0usize; doms.len()];
    'all: loop {
        let mut env = HashMap::new();
        let mut pt = BTreeMap::new();
        for (k, n) in names.iter().enumerate() {
            let v = doms[k][at[k]].clone();
            env.insert(n.to_string(), match &v {
                V::N(x) => Val::Num(Rat::int(*x as i128)),
                V::W(s) => Val::Enum(s.clone()),
            });
            pt.insert(n.to_string(), v);
        }
        let (_, _, _, binds) = rulec::eval::run_all_traced(&f, &c, env);
        // past a `define … range` the call is refused, and nothing below it is asked
        let refused = fam.range.as_ref().is_some_and(|(rn, rlo, rhi)| matches!(binds.get(rn), Some(Val::Num(r)) if r.cmp_to(Rat::int(*rlo as i128)).is_lt() || r.cmp_to(Rat::int(*rhi as i128)).is_gt()));
        if !refused {
            for (name, _, _, _) in &fam.cols {
                let v = match binds.get(name) {
                    Some(Val::Num(r)) if r.den == 1 => V::N(r.num as i64),
                    Some(Val::Bool(b)) => w(if *b { "true" } else { "false" }),
                    Some(Val::Enum(e)) => V::W(e.clone()),
                    other => panic!("{name} came to {other:?} for {pt:?}\n{src}"),
                };
                pt.insert(name.clone(), v);
            }
            points.push(pt);
        }
        for k in (0..at.len()).rev() {
            at[k] += 1;
            if at[k] < doms[k].len() {
                continue 'all;
            }
            at[k] = 0;
        }
        break;
    }
    points
}

/// The values a column takes over the inputs, in order.
fn took(points: &[BTreeMap<String, V>], name: &str) -> Vec<V> {
    points.iter().map(|p| p[name].clone()).collect::<BTreeSet<_>>().into_iter().collect()
}

/// The cells one column of a table that reads a worked-out value is cut into: for a value worked
/// out, around values it takes (or only over them, for the second table above `pick`).
fn column_cells(rng: &mut Rng, fam: &Family, points: &[BTreeMap<String, V>], name: &str, aligned: bool, reachable_only: bool) -> Vec<Cell> {
    if let Some((_, dom, _, _)) = fam.inputs.iter().find(|i| i.0 == name) {
        if let V::N(_) = dom[0] {
            let nums: Vec<i64> = dom.iter().map(|v| if let V::N(x) = v { *x } else { 0 }).collect();
            if name == "qty" && rng.chance(0.5) {
                return dom.iter().cloned().map(Cell::One).collect();
            }
            let cuts = (aligned && name == "amount" && !fam.upper_cuts.is_empty()).then_some(fam.upper_cuts.as_slice());
            return bands(rng, &nums, 3, cuts).0;
        }
        return word_cells(rng, dom, !dom.contains(&w("none")));
    }
    let (_, _, kind, declared) = fam.cols.iter().find(|c| c.0 == name).unwrap();
    let took = took(points, name);
    match kind {
        Kind::Num => {
            let nums: Vec<i64> = took.iter().map(|v| if let V::N(x) = v { *x } else { 0 }).collect();
            if nums.len() <= 4 && rng.chance(0.5) {
                if nums.len() >= 3 && rng.chance(0.4) {
                    return vec![Cell::One(took[0].clone()), Cell::Set(took[1..].to_vec())];
                }
                return took.into_iter().map(Cell::One).collect();
            }
            let mut sample = nums.clone();
            rng.shuffle(&mut sample);
            sample.truncate(3);
            let deltas: &[i64] = if reachable_only { &[0] } else { &[-1, 0, 1] };
            let mut near: Vec<i64> = sample.iter().flat_map(|v| deltas.iter().map(move |d| v + d)).collect();
            near.sort();
            near.dedup();
            rng.shuffle(&mut near);
            near.truncate(rng.int(1, 3) as usize);
            near.sort();
            if reachable_only {
                let last = nums[nums.len() - 1];
                let cuts: Vec<i64> = near.into_iter().filter(|c| *c < last).collect();
                return bands(rng, &nums, 3, Some(&cuts)).0;
            }
            let mut out = Vec::new();
            let mut prev = None;
            for c in &near {
                out.push(Cell::Range(prev, Some(*c), rng.below(4) as u8));
                prev = Some(*c);
            }
            out.push(Cell::Range(prev, None, rng.below(4) as u8));
            out
        }
        Kind::Words => {
            let values = if reachable_only || declared.is_empty() { &took } else { declared };
            word_cells(rng, values, !reachable_only && !values.contains(&w("none")))
        }
    }
}

/// What the inputs come to in one table: the inputs no row takes, the inputs two rows of a
/// `policy unique` table take with the rows, and the rows some input reaches.
struct Truth {
    gaps: Vec<usize>,
    meets: Vec<(usize, Vec<usize>)>,
    reached: BTreeSet<usize>,
}

fn truth(points: &[BTreeMap<String, V>], j: &Judged) -> Truth {
    let mut t = Truth { gaps: Vec::new(), meets: Vec::new(), reached: BTreeSet::new() };
    for (pi, p) in points.iter().enumerate() {
        let hit: Vec<usize> = (0..j.rows.len()).filter(|ri| j.reads.iter().zip(&j.rows[*ri]).all(|(n, c)| c.holds(&p[n]))).collect();
        if hit.is_empty() {
            t.gaps.push(pi);
        } else if j.first {
            t.reached.insert(hit[0]);
        } else {
            t.reached.extend(hit.iter().copied());
            if hit.len() > 1 {
                t.meets.push((pi, hit));
            }
        }
    }
    t
}

/// What was found over the rules of one test, by what is counted.
#[derive(Default)]
struct Tally {
    counts: BTreeMap<String, usize>,
    said: Vec<String>,
}

impl Tally {
    /// Count one `key`, and keep `line` among the first 40 said; a row no input reaches that nothing
    /// is said of is always kept.
    fn add(&mut self, key: &str, line: Option<String>) {
        *self.counts.entry(key.to_string()).or_default() += 1;
        if let Some(l) = line
            && (self.said.len() < 40 || key.starts_with("a row no input reaches"))
        {
            self.said.push(format!("{key}: {l}"));
        }
    }
    fn get(&self, key: &str) -> usize {
        self.counts.get(key).copied().unwrap_or(0)
    }
}

/// The value a witness or a note gives, as an input or a column takes it.
fn wval(v: &WVal) -> V {
    match v {
        WVal::Int(n) => V::N(*n as i64),
        WVal::Str(s) => V::W(s.clone()),
        WVal::Bool(b) => w(if *b { "true" } else { "false" }),
    }
}

/// `k = v, …` as a note writes an input, the units taken off the numbers.
fn pairs(s: &str) -> Vec<(String, V)> {
    s.split(", ")
        .filter_map(|kv| kv.split_once(" = "))
        .map(|(k, v)| {
            let digits = v.trim_end_matches(|ch: char| ch.is_ascii_alphabetic());
            (k.to_string(), digits.parse::<i64>().map(V::N).unwrap_or_else(|_| V::W(v.to_string())))
        })
        .collect()
}

const PRODUCING: &str = "An input producing this example: ";
const COMBINATION: &str = "A combination no row takes: ";

/// The points an example may stand for: every one that gives the witness's values and, where a
/// note gives the input behind it, that input.
fn example_points(d: &Diag, points: &[BTreeMap<String, V>]) -> Option<Vec<usize>> {
    if d.witness.inputs.is_empty() {
        return None;
    }
    let mut want: Vec<(String, V)> = d.witness.inputs.iter().map(|(k, v)| (k.clone(), wval(v))).collect();
    if let Some(line) = d.notes.iter().find_map(|n| n.strip_prefix(PRODUCING)) {
        want.extend(pairs(line));
    }
    Some((0..points.len()).filter(|pi| want.iter().all(|(k, v)| points[*pi].get(k).is_none_or(|x| x == v))).collect())
}

fn table_of(d: &Diag) -> Option<String> {
    d.table.clone().or_else(|| d.rows.first().map(|r| r.table.clone()))
}

/// The rules of `count` families from `seed`, each of a shape `shapes` picks, held to their inputs.
fn hold(seed: u64, count: usize, shapes: &dyn Fn(&mut Rng) -> Shape) -> Tally {
    let tmp = TempDir::new("every-input");
    let mut tally = Tally::default();
    for n in 0..count {
        let mut rng = Rng(seed.wrapping_mul(0x1000_0000_01B3).wrapping_add(n as u64));
        let shape = shapes(&mut rng);
        let mut fam = family(&mut rng, shape);
        let dir = tmp.path().join(format!("f{n}"));
        std::fs::create_dir_all(&dir).unwrap();
        if let Some((name, text)) = &fam.callee {
            std::fs::write(dir.join(name), text).unwrap();
        }
        let mut points = run_every_input(&fam, &dir, &fam.above.clone());
        let c1 = fam.cols[0].0.clone();
        // A `define … range`, which sometimes keeps only part of what the define comes to.
        if fam.ranged {
            let rn = c1.clone();
            let vals: Vec<i64> = took(&points, &rn).iter().map(|v| if let V::N(x) = v { *x } else { 0 }).collect();
            let (lo, hi) = (vals[0], vals[vals.len() - 1]);
            let (rlo, rhi) = if rng.chance(0.5) && hi > lo { (lo, lo + (hi - lo) / 2) } else { (lo - rng.int(0, 5), hi + rng.int(0, 5)) };
            fam.items = fam.items.trim_end().to_string() + &format!("  range >={} <={}\n", money(rlo), money(rhi));
            fam.range = Some((rn, rlo, rhi));
            points = run_every_input(&fam, &dir, &fam.above.clone());
        }
        if points.is_empty() {
            tally.add("family with no input", Some(fam.label.clone()));
            continue;
        }
        // Sometimes a second table reads the value, and an input beside it, and writes a tier.
        if rng.chance(0.35) {
            let mut reads2 = vec![c1.clone()];
            if rng.chance(0.45) {
                let i = rng.below(fam.inputs.len());
                reads2.push(fam.inputs[i].0.clone());
            }
            let per: Vec<Vec<Cell>> = reads2.iter().map(|r| column_cells(&mut rng, &fam, &points, r, false, true)).collect();
            let occurs = |g: &Vec<Cell>| points.iter().any(|p| reads2.iter().zip(g).all(|(r, c)| c.holds(&p[r])));
            let rows2: Vec<(Vec<Cell>, V)> = product(&per).into_iter().filter(occurs).map(|g| (g, w(["t1", "t2", "t3"][rng.below(3)]))).collect();
            let units: Vec<&str> = reads2.iter().map(|r| fam.unit_of(r)).collect();
            fam.enums.push("enum level = t1 | t2 | t3".into());
            fam.above.push_str(&format!("\ntable tier_of\n| {} | -> tier : level |\n{}", reads2.join(" | "), rows_text(&reads2, &rows2, &units, "")));
            fam.judged.push(Judged { table: "tier_of".into(), reads: reads2.clone(), rows: rows2.iter().map(|r| r.0.clone()).collect(), first: false });
            fam.cols.push(("tier".into(), "", Kind::Words, vec![w("t1"), w("t2"), w("t3")]));
            fam.label.push_str(if reads2.len() > 1 { " +tier(+input)" } else { " +tier" });
            points = run_every_input(&fam, &dir, &fam.above.clone());
        }
        // What `pick` reads, and how its columns are cut.
        let mut reads: Vec<String> = fam.inputs.iter().map(|i| i.0.clone()).filter(|i| rng.chance(if i == "amount" { 0.9 } else { 0.6 })).collect();
        let computed: Vec<String> = fam.cols.iter().map(|c| c.0.clone()).collect();
        reads.extend(computed.iter().filter(|_| rng.chance(0.85)).cloned());
        if !reads.iter().any(|r| computed.contains(r)) {
            reads.push(rng.pick(&computed));
        }
        if !reads.iter().any(|r| fam.inputs.iter().any(|i| i.0 == *r)) {
            reads.insert(0, "amount".into());
        }
        let aligned = rng.chance(0.3);
        let per: Vec<Vec<Cell>> = reads.iter().map(|r| column_cells(&mut rng, &fam, &points, r, aligned, false)).collect();
        let grid = product(&per);
        let occurs = |g: &Vec<Cell>| points.iter().any(|p| reads.iter().zip(g).all(|(r, c)| c.holds(&p[r])));
        let (exact, never): (Vec<Vec<Cell>>, Vec<Vec<Cell>>) = grid.iter().cloned().partition(occurs);
        let mut variants: Vec<(&str, Vec<Vec<Cell>>, bool)> = vec![("exact", exact.clone(), false)];
        if grid.len() <= 48 {
            variants.push(("full", grid.clone(), false));
        }
        if exact.len() > 1 {
            let drop = rng.below(exact.len());
            let mut g = exact.clone();
            g.remove(drop);
            variants.push(("gap", g, false));
        }
        if !never.is_empty() {
            variants.push(("extra", [exact.clone(), vec![rng.pick(&never)]].concat(), false));
        }
        if !exact.is_empty() {
            let mut wide = rng.pick(&exact);
            let i = rng.below(reads.len());
            wide[i] = Cell::Any;
            variants.push(("meet", [exact.clone(), vec![wide.clone()]].concat(), false));
            let mut shuffled = exact.clone();
            rng.shuffle(&mut shuffled);
            variants.push(("first exact", shuffled.clone(), true));
            let at = rng.below(shuffled.len() + 1);
            let mut with_wide = shuffled.clone();
            with_wide.insert(at, wide);
            variants.push(("first wide", with_wide, true));
            let k = rng.int(1, (shuffled.len() as i64 / 2).max(1)) as usize;
            let mut catch = shuffled[..shuffled.len() - k.min(shuffled.len())].to_vec();
            catch.push(vec![Cell::Any; reads.len()]);
            variants.push(("first catch", catch, true));
        }
        let units: Vec<&str> = reads.iter().map(|r| fam.unit_of(r)).collect();
        for (vname, rows, first) in variants {
            let outs: Vec<(Vec<Cell>, V)> = rows.iter().enumerate().map(|(i, r)| (r.clone(), w(["a", "b"][i % 2]))).collect();
            let src = format!(
                "rule r{n} v1\n\n{}\noutputs\n  x : choice\n{}\ntable pick\n{}| {} | -> x |\n{}",
                fam.head(),
                fam.above,
                if first { "policy first\n" } else { "" },
                reads.join(" | "),
                rows_text(&reads, &outs, &units, "")
            );
            let path = dir.join(format!("r{n}_{}.rule", vname.replace(' ', "_")));
            std::fs::write(&path, &src).unwrap();
            let p = path.to_string_lossy().into_owned();
            let ds = i18n::with(Lang::En, || rulec::check_source(&src, &p));
            let at = format!("{} ({}, {vname})", path.file_name().unwrap().to_string_lossy(), fam.label);
            let pick = Judged { table: "pick".into(), reads: reads.clone(), rows: rows.clone(), first };
            let mut judged: Vec<&Judged> = fam.judged.iter().collect();
            judged.push(&pick);
            let facts: BTreeMap<String, Truth> = judged.iter().map(|j| (j.table.clone(), truth(&points, j))).collect();
            let errors: Vec<&Diag> = ds.iter().filter(|d| d.severity == Severity::Error).collect();
            if let Some(d) = errors.iter().find(|d| !["E101", "E102", "E105"].contains(&d.code) || !table_of(d).is_some_and(|t| facts.contains_key(&t))) {
                tally.add("rule stopped for another reason", Some(format!("{at}: {} {}\n{src}", d.code, d.title)));
                continue;
            }
            tally.add("rules checked", None);
            let has_gap = facts.values().any(|t| !t.gaps.is_empty());
            let has_meet = facts.values().any(|t| !t.meets.is_empty());
            if errors.is_empty() && (has_gap || has_meet) {
                tally.add("MISSED (passes, and an input matches no row or two)", Some(format!("{at}\n{src}")));
            }
            for d in &errors {
                let t = table_of(d).unwrap();
                let f = &facts[&t];
                match d.code {
                    "E102" => {
                        for r in &d.rows {
                            if f.reached.contains(&(r.row - 1)) {
                                tally.add("FALSE E102 (an input reaches the row)", Some(format!("{at}: table {t} row {}\n{src}", r.row)));
                            } else {
                                tally.add("E102 of a row no input reaches", None);
                            }
                        }
                    }
                    "E101" | "E105" => {
                        let pair: BTreeSet<usize> = d.rows.iter().map(|r| r.row - 1).collect();
                        let real: Vec<usize> =
                            if d.code == "E101" { f.gaps.clone() } else { f.meets.iter().filter(|(_, hit)| pair.iter().all(|r| hit.contains(r))).map(|(p, _)| *p).collect() };
                        if real.is_empty() {
                            let key = if d.code == "E101" { "FALSE E101 (every input has its row)" } else { "FALSE E105 (no input matches both rows)" };
                            tally.add(key, Some(format!("{at}: table {t}: {}", d.notes.first().map(String::as_str).unwrap_or(""))));
                            tally.add(&format!("{} by shape: {}", &key[..10], fam.label), None);
                            continue;
                        }
                        match example_points(d, &points) {
                            None if d.notes.iter().any(|n| n.starts_with(COMBINATION)) => tally.add(&format!("{} example: a combination, not called an input", d.code), None),
                            None => tally.add("EXAMPLE CALLED AN INPUT, WITH NOTHING IN THE WITNESS", Some(format!("{at}: {:?}", d.notes))),
                            Some(ps) if ps.is_empty() => tally.add("EXAMPLE NO INPUT GIVES", Some(format!("{at}: {:?} {:?}", d.witness.inputs, d.notes))),
                            Some(ps) if ps.iter().any(|p| real.contains(p)) => tally.add(&format!("{} example: an input in the finding", d.code), None),
                            Some(_) => tally.add("EXAMPLE OUTSIDE THE FINDING", Some(format!("{at}: {:?} {:?}", d.witness.inputs, d.notes))),
                        }
                    }
                    _ => unreachable!(),
                }
            }
            let valid = !has_gap && !has_meet && judged.iter().all(|j| facts[&j.table].reached.len() == j.rows.len());
            for j in &judged {
                let told: BTreeSet<usize> = ds.iter().filter(|d| ["E102", "W128"].contains(&d.code) && table_of(d).as_deref() == Some(j.table.as_str())).flat_map(|d| d.rows.iter().map(|r| r.row - 1)).collect();
                for ri in 0..j.rows.len() {
                    if !facts[&j.table].reached.contains(&ri) && !told.contains(&ri) {
                        tally.add("QUIET ROW (no input reaches it, and nothing is said of it)", Some(format!("{at}: table {} row {}", j.table, ri + 1)));
                        tally.add(&format!("quiet row by variant: {vname}"), None);
                    }
                }
            }
            if valid {
                tally.add("rules every input of which finds its row, every row reached", None);
                if errors.is_empty() {
                    tally.add("  of them, pass", None);
                } else {
                    tally.add("REFUSED (every input finds its row)", Some(format!("{at}: {}\n{src}", errors.iter().map(|d| format!("{} {}", d.code, d.notes.first().map(String::as_str).unwrap_or(""))).collect::<Vec<_>>().join("; "))));
                }
            }
        }
    }
    tally
}

/// What must never be found, and the counts, printed.
fn settle(name: &str, t: &Tally) {
    let mut out = format!("{name}:\n");
    for (k, v) in &t.counts {
        out.push_str(&format!("{v:>6}  {k}\n"));
    }
    for l in &t.said {
        out.push_str(&format!("  {l}\n"));
    }
    eprintln!("{out}");
    for key in [
        "MISSED (passes, and an input matches no row or two)",
        "FALSE E102 (an input reaches the row)",
        "FALSE E101 (every input has its row)",
        "FALSE E105 (no input matches both rows)",
        "EXAMPLE CALLED AN INPUT, WITH NOTHING IN THE WITNESS",
        "EXAMPLE NO INPUT GIVES",
        "EXAMPLE OUTSIDE THE FINDING",
        "QUIET ROW (no input reaches it, and nothing is said of it)",
        "REFUSED (every input finds its row)",
        "rule stopped for another reason",
    ] {
        assert_eq!(t.get(key), 0, "{out}");
    }
    assert!(t.get("rules checked") > 0, "{out}");
}

/// A table of the same rule writes the value: a band or a fee, `policy unique`, or `policy first`
/// with its last rows given to a catch-all.
#[test]
fn a_table_of_the_same_rule() {
    let t = hold(1, 120, &|rng| Shape::Table { first: rng.chance(0.4), words: rng.chance(0.5) });
    settle("a table of the same rule", &t);
}

/// An applied rule's output: a word, the same word read over again with `with`, a number, an
/// optional truth value.
#[test]
fn an_output_of_an_applied_rule() {
    let t = hold(2, 120, &|rng| Shape::Apply { out: rng.pick(&[Out::Word, Out::WordWith, Out::Num, Out::Opt]) });
    settle("an output of an applied rule", &t);
}

/// A `define`: a truth value, and a number worked out once from the amount to the yen, sometimes
/// under a `range` that keeps only part of it.
#[test]
fn a_define_worked_out_once() {
    let t = hold(3, 120, &|rng| if rng.chance(0.3) { Shape::DefineBool } else { Shape::DefineOnce { range: rng.chance(0.4) } });
    settle("a define worked out once", &t);
}

/// A `define` read from more than one value, or to a step: the amount read twice, which the check
/// works out over pieces of the amount up to a budget, the amount and the quantity, which it reads
/// wider than they come to, and a step of 5 or 10 yen, whose grid it reads exactly.
#[test]
fn a_define_read_wider() {
    let t = hold(4, 120, &|rng| [Shape::DefineTwice, Shape::DefineStep, Shape::DefineQty][rng.below(3)]);
    settle("a define read wider", &t);
}
