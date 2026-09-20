//! Code generation (§8).
//!
//! Readability of the generated code is defined by the five criteria of §8.1. In particular,
//! "one table row is one branch" and "conditions already known to be true from earlier branches
//! are not dropped" are the promises that let a reader match `.rule` rows to generated lines by
//! eye, so no clever optimization is attempted.

use crate::ast::*;
use crate::num::{Rat, RoundMode};
use crate::types::{Checked, Ty};
use std::collections::BTreeMap;

/// Build the ASCII brand name from a unit. Units are a closed set, so a fixed table suffices.
fn brand_of(ty: &Ty) -> String {
    match ty {
        Ty::Money { cur, tax } => {
            let c = match cur.as_str() {
                "円" => "Yen",
                "銭" => "Sen",
                other => other,
            };
            let t = match tax.as_deref() {
                Some(crate::kw::INCL_TAX) => "InclTax",
                Some(crate::kw::EXCL_TAX) => "ExclTax",
                _ => "",
            };
            format!("{c}{t}")
        }
        Ty::Qty { unit, .. } => match unit.as_str() {
            "mg" => "Milligram".into(),
            "g" => "Gram".into(),
            "kg" => "Kilogram".into(),
            "t" => "Tonne".into(),
            "oz" => "Ounce".into(),
            "lb" => "Pound".into(),
            "mm" => "Millimeter".into(),
            "cm" => "Cm".into(),
            "m" => "Meter".into(),
            "km" => "Kilometer".into(),
            "in" => "Inch".into(),
            "ft" => "Foot".into(),
            "yd" => "Yard".into(),
            "mi" => "Mile".into(),
            "mm2" => "SquareMillimeter".into(),
            "cm2" => "SquareCentimeter".into(),
            "m2" => "SquareMeter".into(),
            "a" => "Are".into(),
            "ha" => "Hectare".into(),
            "km2" => "SquareKilometer".into(),
            "坪" => "Tsubo".into(),
            "in2" => "SquareInch".into(),
            "ft2" => "SquareFoot".into(),
            "yd2" => "SquareYard".into(),
            "mi2" => "SquareMile".into(),
            "ac" => "Acre".into(),
            "mm3" => "CubicMillimeter".into(),
            "cm3" => "CubicCentimeter".into(),
            "m3" => "CubicMeter".into(),
            "mL" => "Milliliter".into(),
            "L" => "Liter".into(),
            "kL" => "Kiloliter".into(),
            "ms" => "Millisecond".into(),
            "s" => "Second".into(),
            "min" => "Minute".into(),
            "h" => "Hour".into(),
            "d" => "Day".into(),
            "w" => "Week".into(),
            "℃" => "Celsius".into(),
            "℉" => "Fahrenheit".into(),
            "dB" => "Decibel".into(),
            other => other.into(),
        },
        Ty::Rate => "Rate".into(),
        // A number carries no unit, so there is nothing to brand it apart from. Two counts
        // of different things would get the same brand anyway, and the caller would be left
        // wrapping every integer for no protection at all.
        Ty::Number => "int".into(),
        Ty::Date => "Date".into(),
        Ty::Bool => "bool".into(),
        Ty::Str => "str".into(),
        Ty::Enum(n) => n.clone(),
        Ty::Opt(t) => format!("Optional{}", brand_of(t)),
        Ty::Unknown => "int".into(),
    }
}

/// `member_kind` → `MemberKind`.
fn pascal(s: &str) -> String {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut ch = p.chars();
            match ch.next() {
                Some(c) => c.to_uppercase().collect::<String>() + ch.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// The public surface uses the ASCII alias; internals keep the Japanese name (§8.1).
/// The public spelling of a declared name, for the callers outside this module that have
/// to say the same thing (the approver's page names the inputs by their aliases).
pub fn pub_name_of(n: &Name) -> String {
    pub_name(n)
}

fn pub_name(n: &Name) -> String {
    n.ascii.clone().unwrap_or_else(|| n.text.clone())
}

/// Which side of a counting walk an item is emitted on (§15.58).
///
/// A rule that counts has items before the walk's summary and after it, so the body is
/// written in two passes over the same list: `Walk` for what one element does, `Main` for
/// the rule that the counts feed. Everything else is `All`, which is every rule that came
/// before counting existed.
#[derive(Clone, Copy, PartialEq)]
pub enum Phase {
    All,
    Walk,
    Main,
}

pub struct Gen<'a> {
    f: &'a RuleFile,
    c: &'a Checked,
    /// Table name → the row pairs reported by W114. Guards are emitted only for these.
    w114: BTreeMap<String, Vec<(usize, usize)>>,
    /// Table output column → storage scale. Rate columns declare no step, so the scale is the
    /// least common multiple of the denominators of that column's literals. Unless it is fixed
    /// once per column, 50% and 100% in the same column would come out at different scales and
    /// the values would be corrupted.
    /// Declared name → the identifier the generated code uses for it (§1.3).
    idents: BTreeMap<String, String>,
    /// Type name → the ASCII alias of that enum (PascalCase).
    enum_names: BTreeMap<String, String>,
    /// Enum value → (type name, ASCII alias).
    value_names: BTreeMap<String, (String, String)>,
    src_hash: String,
    /// The source itself and where it was read from, for the approver's page the MCP server
    /// serves (§15.52). The page is rendered from the rule, not from the generated code.
    src: String,
    path: String,
}

mod sql;
pub use sql::round_tests_sql;
mod numpy;
pub use numpy::{np_runtime, round_tests_numpy};
mod php;
pub use php::round_tests_php;
mod java;
pub use java::{java_class, round_tests_java};
mod tool;

impl<'a> Gen<'a> {
    /// The counts this rule declares, in source order (§15.58).
    fn counts(&self) -> Vec<&crate::ast::AggDecl> {
        self.f.items.iter().filter_map(|it| if let Item::Agg(d) = it { Some(d) } else { None }).collect()
    }

    /// The cap the sequence is held to: the smallest upper bound any **count** declares.
    ///
    /// A count can be as large as the sequence, so a longer sequence could take a count
    /// outside the universe the completeness proof quantified over. The guard is the same
    /// shape as the one a number outside its range gets (§15.43, §15.58). A `sum` says
    /// nothing about the length — what holds it is the running total itself (§15.100).
    fn count_cap(&self) -> Option<i128> {
        self.counts()
            .iter()
            .filter(|d| d.kind == crate::ast::AggKind::Count)
            .filter_map(|d| self.c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi))
            .map(|hi| crate::types::wire_int(hi, 1))
            .min()
    }

    /// The word a summary is called by, for the comment beside its accumulator.
    fn agg_word(&self, d: &crate::ast::AggDecl) -> String {
        if d.kind == crate::ast::AggKind::Sum { tr!("合計", "sum") } else { tr!("数え上げ", "count") }
    }

    /// The upper bound a `sum` is held to, on the wire (§15.100). The walk refuses the
    /// moment the running total passes it, which is why the summed column has to be
    /// non-negative: the total only ever rises, so one test at the end is one test
    /// everywhere, and the accumulator never leaves `max + one element`.
    fn sum_caps(&self) -> Vec<(&crate::ast::AggDecl, i128)> {
        self.counts()
            .into_iter()
            .filter(|d| d.kind == crate::ast::AggKind::Sum)
            .filter_map(|d| {
                let hi = self.c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi)?;
                Some((d, crate::types::wire_int(hi, self.c.scales.get(&d.name.text).copied().unwrap_or(1))))
            })
            .collect()
    }

    /// What one element has to look like to be counted: the value its column must take, or
    /// `None` when the column is a bool the test reads as it is.
    fn count_member<'d>(&self, d: &'d crate::ast::AggDecl) -> Option<&'d crate::ast::Name> {
        match self.ty_of(&d.column.text) {
            Ty::Bool => None,
            _ => d.value.as_ref(),
        }
    }

    /// A bool column counted for `false` rather than for `true`.
    fn count_negated(&self, d: &crate::ast::AggDecl) -> bool {
        matches!(self.ty_of(&d.column.text), Ty::Bool)
            && d.value.as_ref().is_some_and(|v| v.text == crate::kw::FALSE)
    }

    /// Whether an item is emitted in this phase. The split is the evaluator's: an item is
    /// element-scoped exactly when it reads something that is (§15.58).
    fn in_phase(&self, it: &Item, phase: Phase) -> bool {
        if matches!(it, Item::Agg(_)) {
            return false;
        }
        if phase == Phase::All {
            return true;
        }
        let scoped = crate::types::element_scoped(self.f);
        let walk = match it {
            Item::Derived(d) => scoped.contains(&d.name.text),
            Item::Define(d) => scoped.contains(&d.name.text),
            Item::Table(t) => t.outputs.iter().any(|o| scoped.contains(&o.name.text)),
            Item::Agg(_) => false,
        };
        walk == (phase == Phase::Walk)
    }

    /// The definition set a table stands for: the merged table's name is the set's key, and
    /// a table that stands alone is its own set (§15.66).
    fn set_for(&self, t: &Table) -> Option<&crate::defset::DefSet> {
        let name = t.name.as_ref().map(|n| n.text.as_str()).unwrap_or("");
        self.c.sets.iter().find(|s| s.key == name)
    }

    /// The comment over a table's branches: the table and its policy, or, for a merged set,
    /// the tables it was built from in the order they are tried.
    fn table_head(&self, t: &Table) -> String {
        let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        match self.set_for(t) {
            Some(s) if s.merged() => {
                let order: Vec<String> =
                    (0..s.members.len()).rev().map(|mi| format!("{} {}", s.kind_word(mi), s.members[mi])).collect();
                tr!(
                    "{name} を定める表と節。例外を先に試す: {}",
                    "the tables and clauses defining {name}, exceptions first: {}",
                    order.join(if crate::i18n::ja() { "、" } else { ", " })
                )
            }
            _ if t.clause => tr!("節 {name}", "clause {name}"),
            _ => {
                let policy = if t.policy == Policy::Unique { crate::kw::UNIQUE } else { crate::kw::FIRST };
                tr!("表 {name}（{} {policy}）", "table {name} ({} {policy})", crate::kw::POLICY)
            }
        }
    }

    /// Whether row `ri` of `t` was written as a clause: `t` itself is one, or the row of a
    /// merged table came from one.
    fn is_clause_row(&self, t: &Table, ri: usize) -> bool {
        t.clause || self.set_for(t).is_some_and(|s| s.merged() && s.is_clause_row(ri))
    }

    /// How a branch names its row: the position as written, its label, and — in a merged
    /// set — the table it was written in.
    fn row_name(&self, t: &Table, ri: usize) -> String {
        let row = &t.rows[ri];
        if self.is_clause_row(t, ri) {
            let tn = row.origin.clone().unwrap_or_else(|| t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default());
            return tr!("節 {tn}", "clause {tn}");
        }
        let n = match &row.label {
            Some(l) => tr!("行{}（{}）", "row {} ({})", row.index, l.text),
            None => tr!("行{}", "row {}", row.index),
        };
        match &row.origin {
            Some(tn) => tr!("表 {tn} {n}", "table {tn} {n}"),
            None => n,
        }
    }

    /// The comment on one branch: the row as written.
    fn row_head(&self, t: &Table, ri: usize, cells: &str) -> String {
        format!("{}: {cells}", self.row_name(t, ri))
    }

    /// A group's constant in Ruby and in its signature: the ASCII alias when the group has
    /// one, else a name by declaration order. Ruby accepts `GROUP_遠隔地`, but RBS reads only
    /// ASCII identifiers, and the two have to agree (§15.67).
    fn rb_group(&self, name: &str) -> String {
        let id = self.ident(name);
        if id.is_ascii() {
            return id;
        }
        let k = self.f.groups.iter().position(|g| g.name.text == name).map(|i| i + 1).unwrap_or(0);
        format!("g{k}")
    }

    /// What a branch pushes on the trace: the table the row was written in, its position
    /// there, and its label (empty when it has none).
    fn fired_of(&self, t: &Table, ri: usize) -> (String, usize, String) {
        let row = &t.rows[ri];
        let tn = row.origin.clone().unwrap_or_else(|| t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default());
        (tn, row.index, row.label.as_ref().map(|l| l.text.clone()).unwrap_or_default())
    }

    pub fn new(f: &'a RuleFile, c: &'a Checked, src: &str) -> Self {
        let mut enum_names = BTreeMap::new();
        let mut value_names = BTreeMap::new();
        for e in &f.enums {
            let ty = pascal(&pub_name(&e.name));
            enum_names.insert(e.name.text.clone(), ty.clone());
            for v in &e.values {
                value_names.insert(v.text.clone(), (ty.clone(), pascal(&pub_name(v))));
            }
        }
        if f.imports.iter().any(|(p, _)| p.ends_with("都道府県")) {
            enum_names.insert("都道府県".into(), "Prefecture".into());
            for (j, r) in crate::prelude::PREFECTURES {
                value_names.insert((*j).into(), ("Prefecture".into(), (*r).into()));
            }
        }
        let mut w114: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
        for set in &c.sets {
            let r = crate::region::check_set(set, c, f, "", crate::region::DEFAULT_BUDGET);
            if !r.w114.is_empty() {
                w114.insert(set.key.clone(), r.w114);
            }
        }
        // §1.3: a name that declared an ASCII alias is written out under that alias, inside
        // the function as well as on the public face.
        let mut idents: BTreeMap<String, String> = BTreeMap::new();
        for n in f
            .inputs
            .iter()
            .map(|i| &i.name)
            .chain(f.outputs.iter().map(|o| &o.name))
            .chain(f.groups.iter().map(|g| &g.name))
            .chain(f.items.iter().flat_map(|it| -> Vec<&crate::ast::Name> {
                match it {
                    Item::Derived(d) => vec![&d.name],
                    Item::Define(d) => vec![&d.name],
                    Item::Agg(d) => vec![&d.name],
                    Item::Table(t) => t.outputs.iter().map(|o| &o.name).collect(),
                }
            }))
        {
            if n.ascii.is_some() {
                idents.insert(n.text.clone(), pub_name(n));
            }
        }
        Gen {
            f,
            c,
            w114,
            enum_names,
            value_names,
            idents,
            src_hash: hash(src),
            src: src.to_string(),
            path: String::new(),
        }
    }

    /// Where the rule was read from. The approver's page names its source, so a caller that
    /// has a file says which; one that has none (the playground) says nothing, and the page
    /// is the same either way apart from that line.
    pub fn at(mut self, path: &str) -> Self {
        self.path = path.to_string();
        self
    }

    /// The page an approver reads, as `rulec doc --format html` renders it: the same
    /// renderer, running the same generated JavaScript (§15.37, §15.52).
    pub fn page(&self) -> String {
        crate::doc::render_html(self.f, self.c, &self.src, &self.path, &self.javascript())
    }

    fn ty_of(&self, n: &str) -> Ty {
        self.c.ty_of(n).unwrap_or(Ty::Unknown)
    }

    /// The identifier a declared name gets in the generated code. §1.3: the public face
    /// always uses its ASCII alias, and an internal name uses one when the author wrote it
    /// and the name itself when they did not. Every lookup in this file keys on the declared
    /// name; only what is *written out* goes through here.
    fn ident(&self, n: &str) -> String {
        self.idents.get(n).cloned().unwrap_or_else(|| n.to_string())
    }

    /// A name for a generated temporary that nothing in the rule already answers to. The
    /// value held before the last rounding used to be called `raw` unconditionally, which
    /// collided the day an output declared the alias `raw` — Python quietly rebound it and
    /// TypeScript refused to parse.
    fn temp(&self, base: &str) -> String {
        let f = self.f;
        let declared: Vec<&crate::ast::Name> = f
            .inputs
            .iter()
            .map(|i| &i.name)
            .chain(f.outputs.iter().map(|o| &o.name))
            .chain(f.groups.iter().map(|g| &g.name))
            .chain(f.items.iter().flat_map(|it| -> Vec<&crate::ast::Name> {
                match it {
                    Item::Derived(d) => vec![&d.name],
                    Item::Define(d) => vec![&d.name],
                    Item::Agg(d) => vec![&d.name],
                    Item::Table(t) => t.outputs.iter().map(|o| &o.name).collect(),
                }
            }))
            .collect();
        let mut n = base.to_string();
        // A name declared without an alias is its own identifier, so the text counts too.
        while declared.iter().any(|d| d.text == n || pub_name(d) == n) {
            n.push('_');
        }
        n
    }

    /// Storage scale of a value (values are held as multiples of 1/k; this is that k). §7.1.
    fn scale(&self, n: &str) -> i128 {
        *self.c.scales.get(n).unwrap_or(&1)
    }

    /// A value written into an output column, brought to that column's scale.
    ///
    /// A name carries the scale of whatever it names — a definition over a rate input is
    /// held in hundredths, say — while the column has a scale of its own, fixed by the
    /// literals that appear in it. Assigning one into the other without this is how a rate
    /// landed in a column a hundred times its size.
    fn rescaled(&self, from: &str, to: &str, text: String) -> String {
        let (a, b) = (self.scale(from), self.scale(to));
        // The column is sized to hold whatever is written into it (see `Gen::new`), so this
        // only ever widens. Anything else would silently drop precision the evaluator keeps.
        if a == b || a == 0 || b % a != 0 {
            text
        } else {
            // No parentheses: this lands on the right of an assignment, and both formatters
            // strip a redundant pair there.
            format!("{text} * {}", b / a)
        }
    }

    /// An integer literal adjusted to the type and the column's scale.
    fn int_lit(&self, n: &crate::lex::Num, ty: &Ty, scale: i128) -> String {
        let v = crate::types::lit_value_in_pub(n, ty).unwrap_or(Rat::zero());
        format!("{}", v.mul(Rat::int(scale)).num)
    }

    fn header(&self, comment: &str) -> String {
        let mut h = tr!(
            "{comment} Code generated by rulec {}. DO NOT EDIT.\n\
             {comment} もと: {} (規則 {} v{}, sha256:{})\n",
            "{comment} Code generated by rulec {}. DO NOT EDIT.\n\
             {comment} Source: {} (rule {} v{}, sha256:{})\n",
            env!("CARGO_PKG_VERSION"),
            self.f.name.text,
            self.f.name.text,
            self.f.version,
            &self.src_hash[..12]
        );
        // A rule applied by this one is part of what was generated (§15.69): its path and the
        // digest the rule pinned, so the file says which callee it was made from.
        for a in &self.f.applies {
            h.push_str(&tr!(
                "{comment} 準用: {} = {} (sha256:{})\n",
                "{comment} Applies: {} = {} (sha256:{})\n",
                a.name.text,
                a.path,
                a.hash.as_deref().unwrap_or("")
            ));
        }
        // The documents the rule transcribes (§15.68): each law at the date it was read, with
        // the pinned digest of every fragment cited, so the file says which text of the law it
        // was made from — two dates when the rule folds two periods (§15.71).
        for s in &self.f.sources {
            let what = match &s.kind {
                SourceKind::Law { id, asof } => {
                    let pins: Vec<String> = s.pins.iter().map(|p| format!("{} sha256:{}", p.fragment, p.hash)).collect();
                    let pinned = if pins.is_empty() { String::new() } else { format!(" ({})", pins.join(", ")) };
                    format!("{} {id} {} {asof}{pinned}", crate::kw::LAW, crate::kw::ASOF)
                }
                SourceKind::File { path, url, hash } => {
                    let u = url.as_ref().map(|u| format!(" {} {u}", crate::kw::URL)).unwrap_or_default();
                    // The tables taken out of the document, as a law's articles are (§15.82).
                    let pins: Vec<String> = s.pins.iter().map(|p| format!("{} sha256:{}", p.fragment, p.hash)).collect();
                    let pinned = if pins.is_empty() { String::new() } else { format!(" ({})", pins.join(", ")) };
                    format!(
                        "{} {path}{u}{}{pinned}",
                        crate::kw::FILE,
                        hash.as_ref().map(|h| format!(" sha256:{h}")).unwrap_or_default()
                    )
                }
            };
            h.push_str(&tr!("{comment} 出典: {} = {what}\n", "{comment} Cites: {} = {what}\n", s.name.text));
        }
        h
    }
}

/// The SHA-256 of the source text, as the header says it is. Until §15.68 the header named
/// SHA-256 and computed FNV-1a; the digest is now the one it names, from `sha256.rs`, with no
/// dependency added.
pub fn hash(s: &str) -> String {
    crate::sha256::hex(s.as_bytes())
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

/// An expression rendered as text together with its scale. Both languages get the same integer
/// arithmetic.
pub(crate) struct Expr2 {
    text: String,
    scale: i128,
}

impl<'a> Gen<'a> {
    fn expr(&self, e: &Expr, local: &dyn Fn(&str) -> String) -> Expr2 {
        match e {
            Expr::Name(n, _) => Expr2 { text: local(n), scale: self.scale(n) },
            Expr::Lit(Lit::Num(n), _) => {
                let ty = crate::types::lit_ty_pub(n);
                let v = crate::types::lit_value_in_pub(n, &ty).unwrap_or(Rat::zero());
                // Not the reduced denominator: the scale `types` records for a name defined
                // over this literal. Storing `3.6%` at 250 and reading the name back at 1000
                // is a silent factor of four (§7.1).
                let den = v.den.max(1);
                let s = crate::types::lit_scale(n).filter(|s| s % den == 0).unwrap_or(den);
                Expr2 { text: format!("{}", v.num * (s / den)), scale: s }
            }
            Expr::Lit(Lit::Word(w), _) if w == crate::kw::TRUE || w == crate::kw::FALSE => {
                Expr2 { text: (if w == crate::kw::TRUE { "True" } else { "False" }).into(), scale: 1 }
            }
            // A date in an expression is its day number, the same integer the cells use. It
            // used to fall through to `0` with the other literals, so a definition such as
            // `作成日 <= 2027-03-31` compiled to `made <= 0` in every language while the
            // evaluator read the date — the first rule to compare a date with a literal in a
            // definition found it (§15.38).
            Expr::Lit(Lit::Date(y, m, d), _) => {
                Expr2 { text: format!("{}", crate::types::date_ord(*y, *m, *d).num), scale: 1 }
            }
            Expr::Lit(..) => Expr2 { text: "0".into(), scale: 1 },
            Expr::Call(name, args, _) => {
                let a: Vec<Expr2> = args.iter().map(|x| self.expr(x, local)).collect();
                match (name.as_str(), a.as_slice()) {
                    (crate::kw::MIN, [x, y]) | (crate::kw::MAX, [x, y]) => {
                        let s = lcm(x.scale, y.scale);
                        let f = if name == crate::kw::MIN { "_min" } else { "_max" };
                        Expr2 {
                            text: format!("{f}({}, {})", rescale(x, s), rescale(y, s)),
                            scale: s,
                        }
                    }
                    (m, [x, g]) if RoundMode::parse(m).is_some() => {
                        // Rounding snaps to a multiple of the grid, so the result is a whole
                        // number of grid steps and comes back at the grid's scale — which is
                        // what the checker records for it (`types::scale`). It used to stay at
                        // x's scale with no division, so a callee output rounded to 1円 from a
                        // value held in 1/20000 円 reached the applying rule 20,000 times too
                        // large in every language (§15.72).
                        let mode = RoundMode::parse(m).unwrap();
                        let s = lcm(x.scale, g.scale);
                        // A literal grid is folded to one number: `(1 * 100)` nested inside
                        // the division below is a product gofmt would tighten.
                        let gs = match g.text.parse::<i128>() {
                            Ok(v) => (v * (s / g.scale)).to_string(),
                            Err(_) => rescale(g, s),
                        };
                        let rounded = format!("_round_{}({}, {})", mode_fn(mode), rescale(x, s), gs);
                        if s == g.scale {
                            Expr2 { text: rounded, scale: s }
                        } else {
                            Expr2 { text: format!("({rounded} // {})", s / g.scale), scale: g.scale }
                        }
                    }
                    _ => Expr2 { text: "0".into(), scale: 1 },
                }
            }
            Expr::Bin(l, op, r, _) => {
                let a = self.expr(l, local);
                let b = self.expr(r, local);
                use BinOp::*;
                match op {
                    Add | Sub => {
                        let s = lcm(a.scale, b.scale);
                        let o = if *op == Add { "+" } else { "-" };
                        Expr2 { text: format!("({} {o} {})", rescale(&a, s), rescale(&b, s)), scale: s }
                    }
                    Mul => Expr2 {
                        text: format!("({} * {})", a.text, b.text),
                        scale: a.scale * b.scale,
                    },
                    Div => {
                        // Dividing by the constant k leaves the stored integer alone and
                        // multiplies the scale by k, so the result is exact. Emitting `//`
                        // would truncate here and Go's `/` would truncate the other way.
                        let k = b.text.parse::<i128>().ok().filter(|k| b.scale > 0 && k % b.scale == 0);
                        match k {
                            Some(k) => Expr2 { text: a.text, scale: a.scale * (k / b.scale) },
                            None => Expr2 { text: format!("({} // {})", a.text, b.text), scale: a.scale },
                        }
                    }
                    Le | Lt | Ge | Gt | Eq => {
                        let s = lcm(a.scale, b.scale);
                        let o = match op {
                            Le => "<=",
                            Lt => "<",
                            Ge => ">=",
                            Gt => ">",
                            _ => "==",
                        };
                        Expr2 {
                            text: format!("({} {o} {})", rescale(&a, s), rescale(&b, s)),
                            scale: 1,
                        }
                    }
                }
            }
        }
    }
}

/// A local for a runner that must not shadow the function the runner has to call.
///
/// The TypeScript and Swift runners call the rule by its bare name, in the same scope as
/// the locals that hold the parsed line. A rule aliased `d` therefore bound the JSON object
/// to `d` and the call resolved to *that* — a type error in Swift, a runtime one in
/// TypeScript. Python, Ruby, Rust and Go all qualify the call (`m.d`, `Mod.d`, `r::d`,
/// `pkg.D`) and never had the problem.
///
/// Only the colliding name moves, so the output of every rule that does not collide is
/// unchanged.
fn runner_local(base: &str, fname: &str) -> String {
    let mut n = base.to_string();
    while n == fname {
        n.push('_');
    }
    n
}

/// The name of the intermediate that holds one output's value before rounding. One output
/// keeps the plain `raw` of the example in §8.2; a second needs a name of its own, or the
/// two assignments would land on the same variable.
/// The doc line of the public function, which decides the same thing as its traced twin
/// and drops the rows that matched.
fn plain_doc(name: &str, version: &str, twin: &str) -> String {
    tr!(
        "規則 {name} v{version}。{twin} と同じ判定で、当てはまった行を返さない。",
        "Rule {name} v{version}: the same decision as {twin}, without the rows that matched."
    )
}

/// The doc line of the traced function, which holds the branches.
fn traced_doc(name: &str, version: &str) -> String {
    tr!(
        "規則 {name} v{version}。分岐はもとの表の行と 1:1 に対応する。出力と、当てはまった行（表ごとに一つ、順に）を返す。",
        "Rule {name} v{version}. Each branch corresponds 1:1 to a row of the rule source. Returns the outputs and the rows that matched, one per table, in order."
    )
}

/// How a value travels on the wire (§10.2), which is all the record function has to know
/// about a type.
enum Wire {
    Enum,
    Bool,
    Int,
    Brand,
    Date,
    Str,
    Opt(Box<Wire>),
}

fn wire_of(ty: &Ty) -> Wire {
    match ty {
        Ty::Enum(_) => Wire::Enum,
        Ty::Bool => Wire::Bool,
        Ty::Date => Wire::Date,
        Ty::Str => Wire::Str,
        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => Wire::Brand,
        Ty::Opt(t) => Wire::Opt(Box::new(wire_of(t))),
        _ => Wire::Int,
    }
}

/// The doc line of the record function.
fn record_doc() -> String {
    tr!(
        "一件を記録の形（docs/formats.md の fixtures）で一行の JSON にする。入力、出た値、当てはまった行。tag は空なら書かない。",
        "One call as a record in the fixtures format (docs/formats.md): the inputs, what came out, and the rows that matched, as one JSON line. An empty tag is left out."
    )
}

/// The doc line of the `Fired` type.
fn fired_doc() -> String {
    tr!("当てはまった行。表の名前と、1 から数えた行番号と、あればラベル。", "A row that matched: the table's name, its 1-based row number, and its label when it has one.")
}

fn raw_base(oi: usize) -> String {
    if oi == 0 {
        "raw".into()
    } else {
        format!("raw{}", oi + 1)
    }
}

fn mode_fn(m: RoundMode) -> &'static str {
    match m {
        RoundMode::Up => "up",
        RoundMode::Down => "down",
        RoundMode::Half => "half",
        RoundMode::Bankers => "bankers",
        RoundMode::HalfDown => "half_down",
    }
}

/// The mode a Python helper's name stands for: the inverse of [`mode_fn`], for the one
/// place that reads the Python spelling back (the SQL translator).
pub(crate) fn mode_from_fn(name: &str) -> Option<RoundMode> {
    [RoundMode::Up, RoundMode::Down, RoundMode::Half, RoundMode::Bankers, RoundMode::HalfDown].into_iter().find(|m| mode_fn(*m) == name)
}

fn rescale(e: &Expr2, to: i128) -> String {
    if e.scale == to {
        e.text.clone()
    } else {
        format!("({} * {})", e.text, to / e.scale)
    }
}

fn lcm(a: i128, b: i128) -> i128 {
    fn g(mut a: i128, mut b: i128) -> i128 {
        while b != 0 {
            let t = a % b;
            a = b;
            b = t;
        }
        a.max(1)
    }
    a / g(a, b) * b
}


// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

impl<'a> Gen<'a> {
    fn py_value(&self, v: &str) -> String {
        match self.value_names.get(v) {
            Some((ty, alias)) => format!("{ty}.{}", alias.to_uppercase()),
            None => format!("{v:?}"),
        }
    }

    /// Render a cell as a Python condition. A don't-care yields None (no condition).
    fn py_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "True".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "False".into(),
                Lit::Word(w) => self.py_value(w),
                Lit::Num(n) => self.int_lit(n, inner, col_scale),
                Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => format!("{s:?}"),
            }
        };
        let members = |ls: &Vec<Lit>| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.py_value(m)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            format!("{{{}}}", out.join(", "))
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!("{var} is None"),
            Cell::Prefix(ps) => ps
                .iter()
                .map(|p| format!("{var}.startswith({})", format!("{p:?}")))
                .collect::<Vec<_>>()
                .join(" or "),
            // A cell that names one group uses the set the module already declares, rather
            // than writing the members out again — otherwise that set is dead code.
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                format!("{var} in _{}", self.ident(w))
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("not {var}"),
            Cell::Lit(l) => format!("{var} == {}", lit(l)),
            Cell::Set(ls) => format!("{var} in {}", members(ls)),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                format!("{var} not in _{}", self.ident(w))
            }
            Cell::Not(ls) => format!("{var} not in {}", members(ls)),
            Cell::Cmp(cs) => cs
                .iter()
                .map(|(o, l)| {
                    let op = match o {
                        CmpOp::Le => "<=",
                        CmpOp::Ge => ">=",
                        CmpOp::Lt => "<",
                        CmpOp::Gt => ">",
                    };
                    format!("{var} {op} {}", lit(l))
                })
                .collect::<Vec<_>>()
                .join(" and "),
        })
    }

    pub fn python(&self) -> String {
        let mut o = self.header("#");
        // Import from typing only what is used. NamedTuple is needed only with multiple outputs.
        o.push_str("from __future__ import annotations\n\nimport enum\nfrom typing import NamedTuple, NewType\n\n");

        // Brands. They work with mypy and pyright and cost nothing at runtime.
        let mut brands: BTreeMap<String, String> = BTreeMap::new();
        for v in self.f.inputs.iter().map(|i| &i.name.text).chain(self.f.outputs.iter().map(|o| &o.name.text)) {
            let ty = self.ty_of(v);
            // A number gets no brand: it is a plain integer on purpose, and branding it
            // would shadow the language's own `int`.
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                brands.insert(brand_of(&ty), format!("{ty}"));
            }
        }
        for (b, doc) in &brands {
            o.push_str(&format!("{b} = NewType(\"{b}\", int)  # {doc}\n"));
        }
        if !brands.is_empty() {
            o.push('\n');
        }

        // Enums. Each value carries its Japanese name, used in logs and in the wire format (§10).
        let mut emitted: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if emitted.contains(ascii) {
                continue;
            }
            emitted.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            o.push_str(&format!("class {ascii}(enum.Enum):\n"));
            for v in vals {
                let name = self.value_names.get(v).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| v.clone());
                o.push_str(&format!("    {name} = \"{v}\"\n"));
            }
            o.push('\n');
        }

        // The sentence and the value travel apart (§15.95): nothing is formatted until the
        // error is printed, and a caller that wants the value does not parse it back out.
        o.push_str(&format!(
            "_NOVALUE = object()\n\n\nclass RuleInputError(ValueError):\n    \"\"\"{}\"\"\"\n\n\
             \x20   def __init__(self, what: str, value: object = _NOVALUE) -> None:\n\
             \x20       super().__init__(what)\n\
             \x20       self.what = what\n\
             \x20       self.value = value\n\n\
             \x20   def __str__(self) -> str:\n\
             \x20       return self.what if self.value is _NOVALUE else f\"{{self.what}}: {{self.value!r}}\"\n\n",
            tr!("宣言した範囲の外。呼び出し側の契約違反。", "Outside the declared input domain: a contract violation by the caller.")
        ));
        o.push_str(&format!(
            "class RuleContradictionError(AssertionError):\n    \"\"\"{}\"\"\"\n\n\
             \x20   def __init__(self, what: str) -> None:\n\
             \x20       super().__init__(what)\n\
             \x20       self.what = what\n\n",
            tr!("規則そのものの矛盾。呼び出し側の誤りではない。", "A contradiction in the rule itself, not a mistake by the caller.")
        ));
        o.push_str(&format!(
            "class Fired(NamedTuple):\n    \"\"\"{}\"\"\"\n\n    table: str\n    row: int\n    label: str = \"\"\n\n",
            fired_doc()
        ));
        // The alias exists so that the local inside the function can be annotated without
        // naming `list`: an input aliased `list` shadows the builtin there, and mypy then
        // reads the annotation as the parameter (§15.33).
        o.push_str("_Trace = list[Fired]\n\n");

        // Groups
        for g in &self.f.groups {
            let ms: Vec<String> = g.members.iter().map(|m| self.py_value(&m.text)).collect();
            o.push_str(&format!("_{} = frozenset({{{}}})\n", self.ident(&g.name.text), ms.join(", ")));
        }
        if !self.f.groups.is_empty() {
            o.push('\n');
        }

        o.push_str(&round_py());
        o.push_str(&self.py_fn());
        o.push_str("\n\n");
        o.push_str(&self.py_record());
        pep8_blanks(&o)
    }
}

/// The five modes of §7.3. Negative values and exact halves are pinned to the spec as well;
/// nothing is left to the language's native division (Python's `//` goes toward −∞, Go's
/// toward 0).
fn round_py() -> String {
    format!(
        r#"
{note}
_isinstance = isinstance


def _min(a: int, b: int) -> int:
    return a if a < b else b


def _max(a: int, b: int) -> int:
    return a if a > b else b


def _round_down(x: int, g: int) -> int:
    """{down}"""
    v = abs(x) // g * g
    return -v if x < 0 else v


def _round_up(x: int, g: int) -> int:
    """{up}"""
    q, r = abs(x) // g, abs(x) % g
    v = (q + 1) * g if r else q * g
    return -v if x < 0 else v


def _round_half(x: int, g: int) -> int:
    """{half}"""
    q, r = abs(x) // g, abs(x) % g
    v = (q + 1) * g if 2 * r >= g else q * g
    return -v if x < 0 else v


def _round_half_down(x: int, g: int) -> int:
    """{half_down}"""
    q, r = abs(x) // g, abs(x) % g
    v = (q + 1) * g if 2 * r > g else q * g
    return -v if x < 0 else v


def _round_bankers(x: int, g: int) -> int:
    """{bankers}"""
    q, r = abs(x) // g, abs(x) % g
    if 2 * r > g or (2 * r == g and q % 2 == 1):
        q += 1
    v = q * g
    return -v if x < 0 else v

"#,
        note = tr!(
            "# 生成コードは組み込みを裸で呼ばない。入力の ASCII 別名が `min` や `list` の\n\
             # ような名前でも壊れないようにするため（衝突の族ごと消す）。",
            "# Generated code never calls a builtin bare, so that an input whose ASCII alias is a\n\
             # name like `min` or `list` does not break it (the whole family of collisions is gone)."
        ),
        down = tr!("0 へ寄せる。-4.8円 → -4円。", "Toward zero: -4.8 yen → -4 yen."),
        up = tr!("0 から遠ざける。-4.2円 → -5円。", "Away from zero: -4.2 yen → -5 yen."),
        half = tr!("半分ちょうどは 0 から遠ざける。", "An exact half goes away from zero."),
        half_down = tr!("半分ちょうどは 0 へ寄せる。", "An exact half goes toward zero."),
        bankers = tr!("半分ちょうどは偶数へ。", "An exact half goes to the even neighbor."),
    )
}

impl<'a> Gen<'a> {
    /// The Python annotation for a type. The public surface uses brands; enums are classes.
    fn py_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => self.enum_names.get(n).cloned().unwrap_or_else(|| "str".into()),
            Ty::Bool => "bool".into(),
            Ty::Date => "int".into(),
            Ty::Str => "str".into(),
            Ty::Opt(t) => format!("{} | None", self.py_ty(t)),
            _ => brand_of(ty),
        }
    }

    /// Storage scale of an output: a single integer in the declared unit (§7.1).
    fn out_scale(&self, name: &str) -> i128 {
        match self.ty_of(name) {
            Ty::Rate => *self.c.scales.get(name).unwrap_or(&100),
            _ => 1,
        }
    }

    /// Indent a block that was emitted at the base level, so it can go inside a loop. The
    /// emitters write their own indentation into the format strings, so the body of a walk is
    /// built first and moved in afterwards — one place instead of a parameter threaded
    /// through every line of six backends (§15.56).
    fn indent_block(body: &str, by: &str) -> String {
        body.lines()
            .map(|l| if l.trim().is_empty() { l.to_string() } else { format!("{by}{l}") })
            .collect::<Vec<_>>()
            .join("\n")
            + if body.ends_with('\n') { "\n" } else { "" }
    }

    /// The fields of one element, in declaration order.
    fn element_fields(&self) -> Vec<&crate::ast::VarDecl> {
        self.f.elements.iter().flat_map(|e| &e.fields).collect()
    }

    /// Names a walk binds itself: the accumulators, and `held` inside `exhausted`.
    fn fold_locals(&self) -> (String, String, String, String, String) {
        (
            self.temp("answer"),
            self.temp("stopped"),
            self.temp("taken"),
            self.temp("kept"),
            self.temp("best"),
        )
    }

    /// The rule's items — derived values, definitions and tables — as Python, at the base
    /// indentation. `local` decides how a name is spelled, which is where a walk puts the
    /// element's own fields.
    fn py_items(&self, local: &dyn Fn(&str) -> String, trace: &str, phase: Phase) -> String {
        let mut o = String::new();
        for it in &self.f.items {
            if !self.in_phase(it, phase) {
                continue;
            }
            match it {
                Item::Derived(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!("    {} = {}  # {}\n", self.ident(&d.name.text), unparen(&e.text), tr!("導出", "derived value")));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!("    {} = {}  # {}\n", self.ident(&d.name.text), unparen(&e.text), tr!("定義", "definition")));
                }
                Item::Agg(_) => {}
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        o.push_str(&self.py_table(t, local, trace));
                    }
                }
            }
        }
        o
    }

    /// The walk itself (§15.56): the accumulators, one pass per element with the body inside
    /// it, and the answer bound to the output's own name so that the rounding and the return
    /// below are the ones every rule uses.
    fn py_walk(&self, fold: &crate::ast::FoldDecl, trace: &str) -> String {
        let el = self.f.elements.as_ref().expect("a fold has elements");
        let seq = pub_name(&el.name);
        let fields: BTreeMap<String, String> =
            el.fields.iter().map(|fd| (fd.name.text.clone(), pub_name(&fd.name))).collect();
        let (answer, stopped, taken, kept, best) = self.fold_locals();
        let e = self.temp("elem");
        let held = self.temp("held");
        // Inside the walk a field is read off the element, and `held` is the accumulator.
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => format!("{e}.{f}"),
                None if n == crate::kw::HELD => held.clone(),
                None => self.ident(n),
            }
        };
        let out_name = self.f.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
        let out_ty = self.ty_of(&out_name);
        let acc_ty = if out_ty.is_numeric() { "int".to_string() } else { self.py_ty(&out_ty) };
        let text = |x: &Option<Expr>| -> String {
            x.as_ref().map(|x| unparen(&self.expr(x, &local).text).to_string()).unwrap_or_else(|| "0".to_string())
        };

        let mut o = String::new();
        o.push_str(&format!("    {answer}: {acc_ty} = {}  # {}\n", text(&fold.empty), tr!("要素ゼロ件の答え", "the answer for no elements")));
        o.push_str(&format!("    {stopped} = False\n"));
        o.push_str(&format!("    {taken}: {acc_ty} | None = None\n"));
        o.push_str(&format!("    {kept}: {acc_ty} | None = None\n"));
        o.push_str(&format!("    {best}: int | None = None\n"));
        o.push_str(&format!("    for {e} in {seq}:\n"));

        let mut body = self.py_element_guards(&local);
        body.push_str(&self.py_items(&local, trace, Phase::All));
        // The verdict the table wrote for this element, and what the walk does about it.
        let v = local(&fold.verdict);
        for (name, arm, _) in &fold.arms {
            let cls = self.py_ty(&self.ty_of(&fold.verdict));
            let member = self.value_names.get(&name.text).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| name.text.clone());
            body.push_str(&format!("    if {v} is {cls}.{member}:  # {}\n", name.text));
            match arm {
                Arm::Next => body.push_str("        pass\n"),
                Arm::Stop(None) => body.push_str("        break\n"),
                Arm::Stop(Some(x)) => {
                    body.push_str(&format!("        {answer} = {}\n", unparen(&self.expr(x, &local).text)));
                    body.push_str(&format!("        {stopped} = True\n        break\n"));
                }
                Arm::Take { expr, unique } => {
                    if *unique {
                        body.push_str(&format!("        if {taken} is not None:\n"));
                        body.push_str(&format!(
                            "            raise RuleContradictionError(\"{}\")\n",
                            tr!(
                                "畳み込み {}: take_unique に二件当たりました",
                                "fold {}: two elements matched a take_unique",
                                fold.verdict
                            )
                        ));
                        body.push_str(&format!("        {taken} = {}\n", unparen(&self.expr(expr, &local).text)));
                    } else {
                        body.push_str(&format!("        if {taken} is None:\n"));
                        body.push_str(&format!("            {taken} = {}\n", unparen(&self.expr(expr, &local).text)));
                    }
                }
                Arm::KeepMax { expr, key } => {
                    let k = self.temp("key");
                    body.push_str(&format!("        {k} = {}\n", unparen(&self.expr(key, &local).text)));
                    body.push_str(&format!("        if {best} is None or {k} > {best}:\n"));
                    body.push_str(&format!("            {best} = {k}\n"));
                    body.push_str(&format!("            {kept} = {}\n", unparen(&self.expr(expr, &local).text)));
                }
            }
        }
        o.push_str(&Self::indent_block(&body, "    "));

        // A sequence with nothing in it answers `empty` and never reaches `exhausted`; a walk
        // that ended on `stop with` has its answer already.
        o.push_str(&format!("    if {seq} and not {stopped}:\n"));
        o.push_str(&format!(
            "        {held} = {taken} if {taken} is not None else ({kept} if {kept} is not None else {})\n",
            text(&fold.empty)
        ));
        o.push_str(&format!("        {answer} = {}\n", text(&fold.exhausted)));
        o.push_str(&format!("    {} = {answer}\n", self.ident(&out_name)));
        o
    }

    /// The counting walk (§15.58): the counters, one pass over the sequence with the items of
    /// one element inside it, and then the rest of the rule with each count bound.
    ///
    /// Unlike a fold, the walk here is not the answer — it is what the answer is computed
    /// from, so the lines after the loop are the ordinary ones every rule has.
    fn py_count_walk(&self, outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let el = self.f.elements.as_ref().expect("a count has elements");
        let seq = pub_name(&el.name);
        let fields: BTreeMap<String, String> =
            el.fields.iter().map(|fd| (fd.name.text.clone(), pub_name(&fd.name))).collect();
        let e = self.temp("elem");
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => format!("{e}.{f}"),
                None => self.ident(n),
            }
        };

        let mut o = String::new();
        if let Some(cap) = self.count_cap() {
            o.push_str(&format!(
                "    if len({seq}) > {cap}:\n        raise RuleInputError(\"{}\", len({seq}))\n",
                tr!(
                    "{} の要素が多すぎます（上限 {cap}）",
                    "{} has too many elements (at most {cap})",
                    el.name.text
                )
            ));
        }
        for d in self.counts() {
            o.push_str(&format!(
                "    {} = 0  # {}\n",
                self.ident(&d.name.text),
                self.agg_word(d)
            ));
        }
        o.push_str(&format!("    for {e} in {seq}:\n"));
        let mut body = self.py_element_guards(&local);
        body.push_str(&self.py_items(&local, trace, Phase::Walk));
        for d in self.counts() {
            let v = local(&d.column.text);
            if d.kind == crate::ast::AggKind::Sum {
                body.push_str(&format!("    {} += {v}  # {}\n", self.ident(&d.name.text), d.name.text));
                continue;
            }
            let test = match self.count_member(d) {
                Some(w) => {
                    let cls = self.py_ty(&self.ty_of(&d.column.text));
                    let member = self
                        .value_names
                        .get(&w.text)
                        .map(|(_, a)| a.to_uppercase())
                        .unwrap_or_else(|| w.text.clone());
                    format!("{v} is {cls}.{member}")
                }
                None if self.count_negated(d) => format!("not {v}"),
                None => v,
            };
            body.push_str(&format!("    if {test}:  # {}\n", d.name.text));
            body.push_str(&format!("        {} += 1\n", self.ident(&d.name.text)));
        }
        o.push_str(&Self::indent_block(&body, "    "));
        for (d, cap) in self.sum_caps() {
            let n = self.ident(&d.name.text);
            o.push_str(&format!(
                "    if {n} > {cap}:\n        raise RuleInputError(\"{}\", {n})\n",
                tr!("{} が範囲の外です", "{} is out of range", d.name.text)
            ));
        }
        o.push_str(&self.py_items(outer, trace, Phase::Main));
        o
    }

    /// The entry guards of one element, inside the walk: the same checks an input gets, on the
    /// values the caller filled in for this element.
    fn py_element_guards(&self, local: &dyn Fn(&str) -> String) -> String {
        let mut o = String::new();
        for i in self.element_fields() {
            let v = local(&i.name.text);
            let ty = self.ty_of(&i.name.text);
            match &ty {
                Ty::Enum(_) => o.push_str(&format!(
                    "    if not _isinstance({v}, {}):\n        raise RuleInputError(\"{}\", {v})\n",
                    self.py_ty(&ty),
                    tr!("{} が列挙 {} の値ではありません", "{} is not a value of enum {}", i.name.text, self.py_ty(&ty))
                )),
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                    o.push_str(&format!(
                        "    if not _isinstance({v}, int) or _isinstance({v}, bool):\n        raise RuleInputError(\"{}\", {v})\n",
                        tr!("{} が整数ではありません", "{} is not an integer", i.name.text)
                    ));
                    if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) {
                        let sc = self.c.wire_scale(&i.name.text);
                        o.push_str(&format!(
                            "    if not {} <= {v} <= {}:\n        raise RuleInputError(\"{}\", {v})\n",
                            crate::types::wire_int(lo, sc),
                            crate::types::wire_int(hi, sc),
                            tr!("{} が範囲の外です", "{} is out of range", i.name.text)
                        ));
                    }
                }
                _ => {}
            }
        }
        o
    }

    fn py_fn(&self) -> String {
        let fname = pub_name(&self.f.name);
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.py_ty(&self.ty_of(&i.name.text))))
            .collect();
        // The sequence a walk reads comes last, after the values that hold for the whole call
        // (§15.56).
        if let Some(el) = &self.f.elements {
            params.push(format!("{}: list[Element]", pub_name(&el.name)));
        }
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 {
            self.py_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let mut o = String::new();

        // One element is a row of inputs, so it is the same NamedTuple an `Output` is.
        if let Some(el) = &self.f.elements {
            o.push_str("class Element(NamedTuple):\n");
            o.push_str(&format!("    \"\"\"{}\"\"\"\n\n", tr!("{} の一件", "one of {}", el.name.text)));
            for fd in &el.fields {
                o.push_str(&format!(
                    "    {}: {}\n",
                    pub_name(&fd.name),
                    self.py_ty(&self.ty_of(&fd.name.text))
                ));
            }
            o.push_str("\n\n");
        }

        // Multiple outputs are a NamedTuple (§8.5).
        if outs.len() > 1 {
            o.push_str("class Output(NamedTuple):\n");
            for od in outs {
                o.push_str(&format!("    {}: {}\n", pub_name(&od.name), self.py_ty(&self.ty_of(&od.name.text))));
            }
            o.push('\n');
        }

        // The public function keeps the plain signature; the branches live in its traced
        // twin, which also returns the rows that matched (§15.33).
        let mut args: Vec<String> = self.f.inputs.iter().map(|i| pub_name(&i.name)).collect();
        if let Some(el) = &self.f.elements {
            args.push(pub_name(&el.name));
        }
        let traced = format!("{fname}_traced");
        o.push_str(&format!("def {fname}({}) -> {ret}:\n", params.join(", ")));
        o.push_str(&format!("    \"\"\"{}\"\"\"\n", plain_doc(&self.f.name.text, &self.f.version, &traced)));
        o.push_str(&format!("    out, _ = {traced}({})\n    return out\n\n\n", args.join(", ")));
        o.push_str(&format!("def {traced}({}) -> tuple[{ret}, list[Fired]]:\n", params.join(", ")));
        o.push_str(&format!("    \"\"\"{}\"\"\"\n", traced_doc(&self.f.name.text, &self.f.version)));

        // Entry guards (§8.5). They enforce at runtime what the proof assumes: inputs lie within
        // their declared domains.
        let local = |n: &str| -> String { self.ident(n) };
        for i in &self.f.inputs {
            let v = pub_name(&i.name);
            let ty = self.ty_of(&i.name.text);
            match &ty {
                Ty::Enum(_) => o.push_str(&format!(
                    "    if not _isinstance({v}, {}):\n        raise RuleInputError(\"{}\", {v})\n",
                    self.py_ty(&ty),
                    tr!(
                        "{} が列挙 {} の値ではありません",
                        "{} is not a value of enum {}",
                        i.name.text,
                        self.py_ty(&ty)
                    )
                )),
                // A date is an ordinal, and its declared range is the universe the
                // completeness proof used, so it is guarded like any other number. Without
                // this, a date outside the declared range falls into whichever branch happens
                // to catch it and the caller gets a silently wrong answer.
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                    // Not an integer is refused before the range is looked at (§15.43): a
                    // float sits inside any range, and 18.3 for a rate declared in steps of
                    // 0.1% would otherwise be taken as 1.83% and answered without a word.
                    // `bool` is an `int` in Python, and a bool here is a mistake too.
                    o.push_str(&format!(
                        "    if not _isinstance({v}, int) or _isinstance({v}, bool):\n        raise RuleInputError(\"{}\", {v})\n",
                        tr!("{} が整数ではありません", "{} is not an integer", i.name.text)
                    ));
                    if let Some((lo, hi)) = self.c.ranges.get(&i.name.text) {
                        if let (Some(lo), Some(hi)) = (lo, hi) {
                            // The bound has to be in the units the argument arrives in, which
                            // for a rate is the number of steps. Taking the numerator alone
                            // turned `range >=0% <=100%` into `0 <= r <= 1` and refused every
                            // value above one step.
                            let sc = self.c.wire_scale(&i.name.text);
                            o.push_str(&format!(
                                "    if not {} <= {v} <= {}:\n        raise RuleInputError(\"{}\", {v})\n",
                                crate::types::wire_int(*lo, sc),
                                crate::types::wire_int(*hi, sc),
                                tr!("{} が範囲の外です", "{} is out of range", i.name.text)
                            ));
                        }
                    }
                }
                _ => {}
            }
        }

        o.push_str(&self.constraint_guards(&local, Lang::Py, "    ", |m| {
            format!("        raise RuleInputError(\"{m}\")\n")
        }));

        let trace = self.temp("trace");
        o.push_str(&format!("    {trace}: _Trace = []\n"));

        // Derived values and definitions, in declaration order (define-before-use, §5.1).
        // A walk runs the same body once per element, so it is written once and moved in
        // (§15.56).
        match &self.f.fold {
            Some(fold) => o.push_str(&self.py_walk(fold, &trace)),
            None if self.counts().is_empty() => o.push_str(&self.py_items(&local, &trace, Phase::All)),
            None => o.push_str(&self.py_count_walk(&local, &trace)),
        }

        // Every output takes the same three steps: its source — the `result` expression for
        // the first output, the binding of its own name otherwise — brought to the wire
        // scale, then its declared rounding applied exactly once. The one-output and the
        // many-output returns each used to compute this, and the second copy had neither
        // step: a `result` was dropped from any rule with two outputs, and a value held in
        // hundredths came back a hundred times too large (§15.16).
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let ty = self.ty_of(out_name);
            finals.push(match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    // Bind the expression to an intermediate instead of nesting it (the same shape as
                    // the example in §8.2). Criterion 1 demands readability, and deep nesting breaks
                    // the visual correspondence with the generated code.
                    if res.scale != os {
                        let raw = self.temp(&raw_base(oi));
                        o.push_str(&format!(
                            "    {raw} = {}  # {}\n",
                            unparen(&res.text),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("_round_{}({raw}, {}) // {}", mode_fn(m), grid_i, res.scale / os)
                    } else {
                        format!("_round_{}({}, {})", mode_fn(m), res.text, grid_i)
                    }
                }
                None => res.text.clone(),
            });
        }
        // The brand has to be put back on. Arithmetic on a `NewType` yields the supertype —
        // `YenInclTax * int` is `int` — so the value that comes out of the rounding helper
        // is a plain `int` and assigning it to a branded output is exactly the mistake the
        // brand exists to catch. Eight of the thirteen corpus rules did not pass
        // `mypy --strict` because of this (§15.22). `NewType.__call__` returns its argument.
        let branded: Vec<String> = outs
            .iter()
            .zip(&finals)
            .map(|(od, f)| {
                let ty = self.ty_of(&od.name.text);
                if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                    format!("{}({f})", brand_of(&ty))
                } else {
                    f.clone()
                }
            })
            .collect();
        if outs.len() == 1 {
            o.push_str(&format!("    return {}, {trace}\n", branded[0]));
        } else {
            // Multiple outputs are a NamedTuple (§8.5); each carries its own rounding.
            o.push_str(&format!("    return Output({}), {trace}\n", branded.join(", ")));
        }
        o
    }

    fn py_table(&self, t: &Table, local: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = format!("    # {}\n", self.table_head(t));
        for (ri, row) in t.rows.iter().enumerate() {
            // §8.1 criterion 1: write out every cell. Conditions already known to be true from
            // earlier branches are not dropped either.
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.py_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "True".into() } else { conds.join(" and ") };
            let kw = if ri == 0 { "if" } else { "elif" };
            let cells: Vec<String> = row
                .cells
                .iter()
                .map(cell_src)
                .chain(row.outs.iter().map(out_src))
                .collect();
            o.push_str(&format!("    {kw} {cond}:  # {}\n", self.row_head(t, ri, &cells.join(" | "))));
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let ty = self.ty_of(&oc.name.text);
                        self.int_lit(n, &ty, self.scale(&oc.name.text))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "True".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "False".into(),
                        Lit::Word(w) => self.py_value(w),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        Lit::Str(x) => str_lit(x),
                        _ => "0".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "True".into()
                        } else if w == crate::kw::FALSE {
                            "False".into()
                        } else if self.value_names.contains_key(w) {
                            self.py_value(w)
                        } else {
                            self.rescaled(w, &oc.name.text, local(w))
                        }
                    }
                    None => "0".into(),
                };
                o.push_str(&format!("        {} = {v}\n", self.ident(&oc.name.text)));
            }
            let (tn, rn, lbl) = self.fired_of(t, ri);
            if lbl.is_empty() {
                o.push_str(&format!("        {trace}.append(Fired({tn:?}, {rn}))\n"));
            } else {
                o.push_str(&format!("        {trace}.append(Fired({tn:?}, {rn}, {lbl:?}))\n"));
            }
        }
        o.push_str(&format!(
            "    else:\n        raise AssertionError(\"{}\")\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        o.push_str(&self.guards(t, local, Lang::Py, "    ", |name, i, j| {
            format!(
                "        raise RuleContradictionError(\"{}\")\n",
                tr!("表 {name}: {i} と {j} が同時に当てはまりました", "table {name}: {i} and {j} matched at the same time")
            )
        }));
        o
    }

    /// The door for the `constraint` lines (§15.55).
    ///
    /// The checker was told these combinations do not happen, and it believed it: it demanded
    /// no row for them. So a caller that sends one reaches branches that have nothing to
    /// answer with. Refusing at the door is the same line as the range and the integer checks
    /// — the entry guard enforces exactly what the proof assumed (§15.43, §15.45).
    ///
    /// Both sides are inputs of the same type, so their wire values share a scale and compare
    /// directly.
    fn constraint_guards(
        &self,
        local: &dyn Fn(&str) -> String,
        lang: Lang,
        indent: &str,
        raise: impl Fn(&str) -> String,
    ) -> String {
        let sp = lang.spelling();
        let mut o = String::new();
        for k in &self.f.constraints {
            let (a, b) = (local(&k.left), local(&k.right));
            let op = k.op.word();
            let said = format!("{} {op} {}", k.left, k.right);
            o.push_str(&format!(
                "{indent}{} {}\n",
                sp.comment,
                tr!("制約: {said}", "constraint: {said}")
            ));
            o.push_str(&format!("{indent}{}\n", (sp.if_head)(&format!("{}({a} {op} {b})", lang.not()))));
            o.push_str(&raise(&tr!("制約が成り立ちません: {said}", "the constraint does not hold: {said}")));
            if !sp.close.is_empty() {
                o.push_str(&format!("{indent}{}\n", sp.close));
            }
        }
        o
    }

    /// Guards (§8.1). Emitted only for row pairs whose exclusivity could not be proven statically.
    /// No vector exercises this branch: had such an input been constructible, it would have been
    /// an E105.
    fn guards(
        &self,
        t: &Table,
        local: &dyn Fn(&str) -> String,
        lang: Lang,
        indent: &str,
        raise: impl Fn(&str, &str, &str) -> String,
    ) -> String {
        let Some(pairs) = self.w114.get(&t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default()) else {
            return String::new();
        };
        let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        let guard = tr!("ガード", "guard");
        let mut o = String::new();
        for (i, j) in pairs {
            let (ni, nj) = (self.row_name(t, *i), self.row_name(t, *j));
            let mut conds: Vec<String> = Vec::new();
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                let ty = self.ty_of(col);
                let sc = self.scale(col);
                for r in [*i, *j] {
                    let Some(cell) = t.rows[r].cells.get(ci) else { continue };
                    let c = self.cell(lang, cell, &local(col), &ty, sc);
                    if let Some(c) = c {
                        if !conds.contains(&c) {
                            conds.push(c);
                        }
                    }
                }
            }
            if conds.is_empty() {
                continue;
            }
            let sp = lang.spelling();
            let joined = conds.join(sp.and);
            o.push_str(&format!(
                "{indent}{} {guard}: {}\n",
                sp.comment,
                tr!(
                    "W114（表 {name} {ni} × {nj}）。重ならないことを静的に証明できなかった行の対",
                    "W114 (table {name}, {ni} × {nj}): a pair of rows whose exclusivity could not be proven statically"
                )
            ));
            o.push_str(&format!("{indent}{}\n", (sp.if_head)(&joined)));
            o.push_str(&raise(&name, &ni, &nj));
            if !sp.close.is_empty() {
                o.push_str(&format!("{indent}{}\n", sp.close));
            }
        }
        o
    }
}

/// The source spelling of a cell, used in diagnostics and in generated comments.
fn cell_src(c: &Cell) -> String {
    match c {
        Cell::DontCare => "-".into(),
        Cell::Nothing => crate::kw::NONE.into(),
        Cell::Lit(l) => lit_src(l),
        Cell::Set(ls) => ls.iter().map(lit_src).collect::<Vec<_>>().join(", "),
        Cell::Not(ls) => format!("{}: {}", crate::kw::NOT, ls.iter().map(lit_src).collect::<Vec<_>>().join(", ")),
        Cell::Prefix(ps) => format!("{} {}", crate::kw::STARTS_WITH, ps.iter().map(|p| format!("\"{p}\"")).collect::<Vec<_>>().join(", ")),
        Cell::Cmp(cs) => cs
            .iter()
            .map(|(o, l)| {
                let op = match o {
                    CmpOp::Le => "<=",
                    CmpOp::Ge => ">=",
                    CmpOp::Lt => "<",
                    CmpOp::Gt => ">",
                };
                format!("{op}{}", lit_src(l))
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn out_src(o: &OutCell) -> String {
    match o {
        OutCell::Lit(l) => lit_src(l),
        OutCell::Name(n) => n.clone(),
    }
}

/// A string literal for the generated code. Every target but SQL writes a string between
/// double quotes and escapes `"` and `\\` the same way, and a `.rule` string cannot hold a
/// newline (the lexer reads it on one line), so one helper serves all of them.
///
/// Until §15.88 there was none: the lowering of an output cell had no arm for `Lit::Str`, so
/// every string fell through to `_ => "0"` and a `string` output answered **0** in every
/// language, with the row's own comment beside it printing the value it should have had.
pub(crate) fn str_lit(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn lit_src(l: &Lit) -> String {
    match l {
        Lit::Num(n) => n.raw.clone(),
        Lit::Word(w) => w.clone(),
        Lit::Str(s) => format!("\"{s}\""),
        Lit::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
    }
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

impl<'a> Gen<'a> {
    fn go_value(&self, v: &str) -> String {
        match self.value_names.get(v) {
            Some((ty, alias)) => format!("{ty}{alias}"),
            None => format!("{v:?}"),
        }
    }

    /// Go's zero value for a return type, for the early return of a guard. `return 0` does
    /// not compile when the rule answers with a boolean or a string.
    fn go_zero(&self, ty: &Ty) -> String {
        match ty {
            Ty::Bool => "false".into(),
            Ty::Str => "\"\"".into(),
            Ty::Opt(_) => "nil".into(),
            _ => "0".into(),
        }
    }

    /// A blank assignment for a value nothing downstream reads. The value is still computed,
    /// so that the generated code and the rule stay line for line, but Go will not compile a
    /// local that is never read — so say out loud that it is on purpose. W111 has already
    /// named the declaration.
    fn go_unread(&self, name: &str) -> String {
        if self.is_read(name) {
            String::new()
        } else {
            format!("\t_ = {}\n", self.ident(name))
        }
    }

    /// Whether anything downstream reads a table's output column. A column nothing reads is
    /// still assigned, so that the branch and the row stay 1:1 (§8.2), but Go refuses to
    /// compile a local that is never read. W111 reports the column itself.
    fn is_read(&self, name: &str) -> bool {
        self.c.used.contains(name) || self.f.outputs.iter().any(|o| o.name.text == name)
    }

    fn go_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => self.enum_names.get(n).cloned().unwrap_or_else(|| "string".into()),
            Ty::Bool => "bool".into(),
            Ty::Number | Ty::Date => "int64".into(),
            Ty::Str => "string".into(),
            Ty::Opt(t) => format!("*{}", self.go_ty(t)),
            _ => brand_of(ty),
        }
    }

    fn go_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        // An optional column is a pointer in Go, so every test but `none` reads through it
        // and needs the nil guard the type system asks for. Without it the package did not
        // compile at all: `in.Memo == KindA` is `*Kind` against `Kind` (DESIGN §15.88). The
        // guard flips for a negated cell, because `not 甲` holds of the absent value — `T?`
        // is the enum with one more value, and that is how the checker reads it too.
        let optional = matches!(ty, Ty::Opt(_));
        let raw = var.to_string();
        let deref = if optional { format!("(*{var})") } else { raw.clone() };
        let var = deref.as_str();
        let guarded = |c: String, neg: bool| -> String {
            if !optional {
                c
            } else if neg {
                format!("({raw} == nil || {c})")
            } else {
                format!("({raw} != nil && {c})")
            }
        };
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.go_value(w),
                Lit::Num(n) => self.int_lit(n, inner, col_scale),
                Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => format!("{s:?}"),
            }
        };
        let set = |ls: &Vec<Lit>, neg: bool| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.go_value(m)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            let op = if neg { "!=" } else { "==" };
            let join = if neg { " && " } else { " || " };
            format!(
                "({})",
                out.iter().map(|v| format!("{var} {op} {v}")).collect::<Vec<_>>().join(join)
            )
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => return Some(format!("{raw} == nil")),
            Cell::Prefix(ps) => ps
                .iter()
                .map(|p| format!("strings.HasPrefix({var}, {})", format!("{p:?}")))
                .collect::<Vec<_>>()
                .join(" || "),
            // A cell that names one group calls the predicate the package already declares,
            // rather than writing the members out again — otherwise that function is dead.
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                guarded(format!("is{}({var})", pascal(&self.ident(w))), false)
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => guarded(var.to_string(), false),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => guarded(format!("!{var}"), false),
            Cell::Lit(l) => guarded(format!("{var} == {}", lit(l)), false),
            Cell::Set(ls) => guarded(set(ls, false), false),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                guarded(format!("!is{}({var})", pascal(&self.ident(w))), true)
            }
            Cell::Not(ls) => guarded(set(ls, true), true),
            Cell::Cmp(cs) => cs
                .iter()
                .map(|(o, l)| {
                    let op = match o {
                        CmpOp::Le => "<=",
                        CmpOp::Ge => ">=",
                        CmpOp::Lt => "<",
                        CmpOp::Gt => ">",
                    };
                    format!("{var} {op} {}", lit(l))
                })
                .collect::<Vec<_>>()
                .join(" && "),
        })
    }

    pub fn go(&self) -> String {
        let alias = pub_name(&self.f.name);
        let pkg = alias.replace('_', "").to_lowercase();
        let mut o = self.header("//");
        o.push_str(&format!("\npackage {pkg}\n\n{IMPORT_MARK}"));

        let mut brands: BTreeMap<String, String> = BTreeMap::new();
        for v in self.f.inputs.iter().map(|i| &i.name.text).chain(self.f.outputs.iter().map(|o| &o.name.text)) {
            let ty = self.ty_of(v);
            // A number gets no brand: it is a plain integer on purpose, and branding it
            // would shadow the language's own `int`.
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                brands.insert(brand_of(&ty), format!("{ty}"));
            }
        }
        for (b, doc) in &brands {
            // A defined type (not an alias): the compiler rejects YenInclTax + YenExclTax.
            o.push_str(&format!("type {b} int64{CELL}// {doc}\n"));
        }
        if !brands.is_empty() {
            o.push('\n');
        }

        let mut emitted: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if emitted.contains(ascii) {
                continue;
            }
            emitted.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            o.push_str(&format!("type {ascii} int\n\nconst (\n"));
            for (i, v) in vals.iter().enumerate() {
                let n = self.go_value(v);
                if i == 0 {
                    o.push_str(&format!("\t{n}{CELL}{ascii} = iota{CELL}// {v}\n"));
                } else {
                    o.push_str(&format!("\t{n}{CELL}{CELL}// {v}\n"));
                }
            }
            o.push_str(")\n\n");
            o.push_str(&format!(
                "func (v {ascii}) Valid() bool {{ return v >= 0 && v < {} }}\n\n",
                vals.len()
            ));
            o.push_str(&format!("func (v {ascii}) String() string {{\n\tswitch v {{\n"));
            for v in vals {
                o.push_str(&format!("\tcase {}:\n\t\treturn {:?}\n", self.go_value(v), v));
            }
            o.push_str("\t}\n\treturn \"?\"\n}\n\n");
            // The wire format of §10 uses the Japanese names, so provide a way back from them.
            o.push_str(&format!("func Parse{ascii}(s string) ({ascii}, bool) {{\n\tswitch s {{\n"));
            for v in vals {
                o.push_str(&format!("\tcase {:?}:\n\t\treturn {}, true\n", v, self.go_value(v)));
            }
            o.push_str(&format!("\t}}\n\treturn {}(0), false\n}}\n\n", ascii));
        }

        // Groups. They are unexported, so the Japanese identifiers can stay (§8.1).
        for g in &self.f.groups {
            let ty = self
                .c
                .groups
                .get(&g.name.text)
                .and_then(|(owner, _)| self.enum_names.get(owner).cloned())
                .unwrap_or_else(|| "int".into());
            o.push_str(&format!(
                "func is{}(v {ty}) bool {{\n\tswitch v {{\n\tcase ",
                pascal(&self.ident(&g.name.text))
            ));
            let ms: Vec<String> = g.members.iter().map(|m| self.go_value(&m.text)).collect();
            o.push_str(&ms.join(", "));
            o.push_str(":\n\t\treturn true\n\t}\n\treturn false\n}\n\n");
        }

        o.push_str(&format!("// Fired {}\ntype Fired struct {{\n\tTable{CELL}string\n\tRow{CELL}int\n\tLabel{CELL}string\n}}\n\n", fired_doc()));
        o.push_str(&round_go());
        o.push_str(&self.go_fn());
        o.push('\n');
        o.push_str(&self.go_record());
        // The record function uses fmt whatever the rule looks like, so the import is
        // unconditional now. `strings` comes in only where a cell tests a prefix
        // (§15.101) — an unused import does not compile in Go.
        let imports = if o.contains("strings.HasPrefix(") {
            "import (\n\t\"fmt\"\n\t\"strings\"\n)\n\n"
        } else {
            "import \"fmt\"\n\n"
        };
        let o = o.replace(IMPORT_MARK, imports);
        align(&o)
    }

    /// The rule's items as Go, at the base indentation (§15.56).
    fn go_items(&self, local: &dyn Fn(&str) -> String, trace: &str, phase: Phase) -> String {
        let mut o = String::new();
        for it in &self.f.items {
            if !self.in_phase(it, phase) {
                continue;
            }
            match it {
                Item::Derived(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!("\t{} := {}{CELL}// {}\n", self.ident(&d.name.text), go_typed(&go_expr(&e.text)), tr!("導出", "derived value")));
                    o.push_str(&self.go_unread(&d.name.text));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!("\t{} := {}{CELL}// {}\n", self.ident(&d.name.text), go_typed(&go_expr(&e.text)), tr!("定義", "definition")));
                    o.push_str(&self.go_unread(&d.name.text));
                }
                Item::Agg(_) => {}
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        o.push_str(&self.go_table(t, local, trace));
                    }
                }
            }
        }
        o
    }

    /// The walk, in Go (§15.56).
    fn go_walk(&self, fold: &crate::ast::FoldDecl, trace: &str, zero: &str) -> String {
        let el = self.f.elements.as_ref().expect("a fold has elements");
        let seq = format!("in.{}", pascal(&pub_name(&el.name)));
        let fields: BTreeMap<String, String> =
            el.fields.iter().map(|fd| (fd.name.text.clone(), pascal(&pub_name(&fd.name)))).collect();
        let (answer, stopped, taken, kept, best) = self.fold_locals();
        let e = self.temp("elem");
        let held = self.temp("held");
        // Go has no `None`, so each accumulator carries its own "was it ever set" flag.
        let has_taken = self.temp("tookOne");
        let has_kept = self.temp("keptOne");
        let has_best = self.temp("bestSet");
        // Inside the walk a field is read off the element, and `held` is the accumulator.
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => {
                    let v = format!("{e}.{f}");
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                        format!("int64({v})")
                    } else {
                        v
                    }
                }
                None if n == crate::kw::HELD => held.clone(),
                None => self.ident(n),
            }
        };
        let out_name = self.f.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
        let out_ty = self.ty_of(&out_name);
        let acc_ty = if out_ty.is_numeric() { "int64".to_string() } else { self.go_ty(&out_ty) };
        let acc_zero = if out_ty.is_numeric() { "0".to_string() } else { self.go_zero(&out_ty) };
        let text = |x: &Option<Expr>| -> String {
            x.as_ref().map(|x| go_expr(&self.expr(x, &local).text)).unwrap_or_else(|| "0".to_string())
        };
        // Go refuses to compile a local nothing reads, so only the arms that are written get one.
        let takes = fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::Take { .. }));
        let keeps = fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::KeepMax { .. }));

        let mut o = String::new();
        o.push_str(&format!("\tvar {answer} {acc_ty} = {}{CELL}// {}\n", text(&fold.empty), tr!("要素ゼロ件の答え", "the answer for no elements")));
        o.push_str(&format!("\t{stopped} := false\n"));
        if takes {
            o.push_str(&format!("\tvar {taken} {acc_ty} = {acc_zero}\n\t{has_taken} := false\n"));
        }
        if keeps {
            o.push_str(&format!("\tvar {kept} {acc_ty} = {acc_zero}\n\t{has_kept} := false\n"));
            o.push_str(&format!("\tvar {best} int64 = 0\n\t{has_best} := false\n"));
        }
        o.push_str(&format!("\tfor _, {e} := range {seq} {{\n"));

        let mut body = self.go_element_guards(&local, zero);
        body.push_str(&self.go_items(&local, trace, Phase::All));
        // The verdict the table wrote for this element, and what the walk does about it.
        let v = local(&fold.verdict);
        for (name, arm, _) in &fold.arms {
            body.push_str(&format!("\tif {} == {} {{ // {}\n", v, self.go_value(&name.text), name.text));
            match arm {
                Arm::Next => body.push_str(&format!("\t\t// {}\n", crate::kw::NEXT)),
                Arm::Stop(None) => body.push_str("\t\tbreak\n"),
                Arm::Stop(Some(x)) => {
                    body.push_str(&format!("\t\t{answer} = {}\n", go_expr(&self.expr(x, &local).text)));
                    body.push_str(&format!("\t\t{stopped} = true\n\t\tbreak\n"));
                }
                Arm::Take { expr, unique } => {
                    if *unique {
                        body.push_str(&format!("\t\tif {has_taken} {{\n"));
                        body.push_str(&format!(
                            "\t\t\treturn {zero}, nil, &RuleContradictionError{{What: \"{}\"}}\n\t\t}}\n",
                            tr!(
                                "畳み込み {}: take_unique に二件当たりました",
                                "fold {}: two elements matched a take_unique",
                                fold.verdict
                            )
                        ));
                        body.push_str(&format!("\t\t{taken} = {}\n\t\t{has_taken} = true\n", go_expr(&self.expr(expr, &local).text)));
                    } else {
                        body.push_str(&format!("\t\tif !{has_taken} {{\n"));
                        body.push_str(&format!("\t\t\t{taken} = {}\n\t\t\t{has_taken} = true\n\t\t}}\n", go_expr(&self.expr(expr, &local).text)));
                    }
                }
                Arm::KeepMax { expr, key } => {
                    let k = self.temp("key");
                    body.push_str(&format!("\t\t{k} := {}\n", go_expr(&self.expr(key, &local).text)));
                    body.push_str(&format!("\t\tif !{has_best} || {k} > {best} {{\n"));
                    body.push_str(&format!("\t\t\t{best} = {k}\n\t\t\t{has_best} = true\n"));
                    body.push_str(&format!("\t\t\t{kept} = {}\n\t\t\t{has_kept} = true\n\t\t}}\n", go_expr(&self.expr(expr, &local).text)));
                }
            }
            body.push_str("\t}\n");
        }
        o.push_str(&Self::indent_block(&body, "\t"));
        o.push_str("\t}\n");

        // A sequence with nothing in it answers `empty` and never reaches `exhausted`; a walk
        // that ended on `stop with` has its answer already.
        o.push_str(&format!("\tif len({seq}) > 0 && !{stopped} {{\n"));
        o.push_str(&format!("\t\tvar {held} {acc_ty} = {}\n", text(&fold.empty)));
        if takes {
            o.push_str(&format!("\t\tif {has_taken} {{\n\t\t\t{held} = {taken}\n\t\t}}\n"));
        }
        if keeps {
            let els = if takes { " else" } else { "" };
            if takes {
                // Chain onto the `if` just written, so a taken value wins over a kept one.
                o = o.trim_end_matches("\t\t}\n").to_string();
                o.push_str(&format!("\t\t}}{els} if {has_kept} {{\n\t\t\t{held} = {kept}\n\t\t}}\n"));
            } else {
                o.push_str(&format!("\t\tif {has_kept} {{\n\t\t\t{held} = {kept}\n\t\t}}\n"));
            }
        }
        let ex = text(&fold.exhausted);
        if !ex.contains(&held) {
            o.push_str(&format!("\t\t_ = {held}\n"));
        }
        o.push_str(&format!("\t\t{answer} = {ex}\n\t}}\n"));
        o.push_str(&format!("\t{} := {answer}\n", self.ident(&out_name)));
        o.push_str(&self.go_unread(&out_name));
        o
    }

    /// The counting walk in Go. The shape is the one `py_count_walk` writes (§15.58).
    fn go_count_walk(&self, outer: &dyn Fn(&str) -> String, trace: &str, zero: &str) -> String {
        let el = self.f.elements.as_ref().expect("a count has elements");
        let seq = format!("in.{}", pascal(&pub_name(&el.name)));
        let fields: BTreeMap<String, String> =
            el.fields.iter().map(|fd| (fd.name.text.clone(), pascal(&pub_name(&fd.name)))).collect();
        let e = self.temp("elem");
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => {
                    let v = format!("{e}.{f}");
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                        format!("int64({v})")
                    } else {
                        v
                    }
                }
                None => self.ident(n),
            }
        };
        let mut o = String::new();
        if let Some(cap) = self.count_cap() {
            o.push_str(&format!(
                "\tif len({seq}) > {cap} {{\n\t\treturn {zero}, nil, &RuleInputError{{What: \"{}\", Value: int64(len({seq})), HasValue: true}}\n\t}}\n",
                tr!("{} の要素が多すぎます（上限 {cap}）", "{} has too many elements (at most {cap})", el.name.text)
            ));
        }
        for d in self.counts() {
            o.push_str(&format!("\tvar {} int64 = 0{CELL}// {}\n", self.ident(&d.name.text), self.agg_word(d)));
        }
        o.push_str(&format!("\tfor _, {e} := range {seq} {{\n"));
        let mut body = self.go_element_guards(&local, zero);
        body.push_str(&self.go_items(&local, trace, Phase::Walk));
        for d in self.counts() {
            let v = local(&d.column.text);
            if d.kind == crate::ast::AggKind::Sum {
                body.push_str(&format!("\t{} += {v} // {}\n", self.ident(&d.name.text), d.name.text));
                continue;
            }
            let test = match self.count_member(d) {
                Some(w) => format!("{v} == {}", self.go_value(&w.text)),
                None if self.count_negated(d) => format!("!{v}"),
                None => v,
            };
            body.push_str(&format!("\tif {test} {{ // {}\n\t\t{}++\n\t}}\n", d.name.text, self.ident(&d.name.text)));
        }
        o.push_str(&Self::indent_block(&body, "\t"));
        o.push_str("\t}\n");
        for (d, cap) in self.sum_caps() {
            let n = self.ident(&d.name.text);
            o.push_str(&format!(
                "\tif {n} > {cap} {{\n\t\treturn {zero}, nil, &RuleInputError{{What: \"{}\", Value: {n}, HasValue: true}}\n\t}}\n",
                tr!("{} が範囲の外です", "{} is out of range", d.name.text)
            ));
        }
        for d in self.counts() {
            o.push_str(&self.go_unread(&d.name.text));
        }
        o.push_str(&self.go_items(outer, trace, Phase::Main));
        o
    }

    /// The entry guards of one element, inside the walk: the same checks an input gets, on the
    /// values the caller filled in for this element.
    fn go_element_guards(&self, local: &dyn Fn(&str) -> String, zero: &str) -> String {
        let mut o = String::new();
        for i in self.element_fields() {
            let v = local(&i.name.text);
            let val = go_i64(&v);
            let ty = self.ty_of(&i.name.text);
            match &ty {
                Ty::Enum(_) => o.push_str(&format!(
                    "\tif !{v}.Valid() {{\n\t\treturn {zero}, nil, &RuleInputError{{What: \"{}\", Value: {val}, HasValue: true}}\n\t}}\n",
                    tr!("{} が列挙の値ではありません", "{} is not a value of the enum", i.name.text)
                )),
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                    if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) {
                        let sc = self.c.wire_scale(&i.name.text);
                        o.push_str(&format!(
                            "\tif {v} < {} || {v} > {} {{\n\t\treturn {zero}, nil, &RuleInputError{{What: \"{}\", Value: {val}, HasValue: true}}\n\t}}\n",
                            crate::types::wire_int(lo, sc),
                            crate::types::wire_int(hi, sc),
                            tr!("{} が範囲の外です", "{} is out of range", i.name.text)
                        ));
                    }
                }
                _ => {}
            }
        }
        o
    }

    fn go_fn(&self) -> String {
        let fname = pascal(&pub_name(&self.f.name));
        let outs = &self.f.outputs;
        let mut o = String::new();

        o.push_str(&format!(
            "// {}\ntype Input struct {{\n",
            tr!(
                "Input は入力をまとめて受ける。同型の int が並ぶのを避けるため（§8.3）。",
                "Input bundles the inputs, so that a parameter list of same-typed ints is avoided (§8.3)."
            )
        ));
        for i in &self.f.inputs {
            // The field holds the wire integer, so the range is shown in the same units as
            // the guard below it, not in the units the declaration was written in.
            let sc = self.c.wire_scale(&i.name.text);
            let doc = match self.c.ranges.get(&i.name.text) {
                Some((Some(lo), Some(hi))) => tr!(
                    "// {} 範囲 {}..{}",
                    "// {} range {}..{}",
                    i.name.text,
                    crate::types::wire_int(*lo, sc),
                    crate::types::wire_int(*hi, sc)
                ),
                _ => format!("// {}", i.name.text),
            };
            o.push_str(&format!("\t{}{CELL}{}{CELL}{doc}\n", pascal(&pub_name(&i.name)), self.go_ty(&self.ty_of(&i.name.text))));
        }
        // The sequence a walk reads is a field like the rest (§15.56).
        if let Some(el) = &self.f.elements {
            o.push_str(&format!(
                "\t{}{CELL}[]Element{CELL}// {}\n",
                pascal(&pub_name(&el.name)),
                el.name.text
            ));
        }
        o.push_str("}\n\n");
        // One element is a row of inputs, so it is a struct like `Input`.
        if let Some(el) = &self.f.elements {
            o.push_str(&format!("// {}\ntype Element struct {{\n", tr!("{} の一件", "one of {}", el.name.text)));
            for fd in &el.fields {
                o.push_str(&format!(
                    "\t{}{CELL}{}{CELL}// {}\n",
                    pascal(&pub_name(&fd.name)),
                    self.go_ty(&self.ty_of(&fd.name.text)),
                    fd.name.text
                ));
            }
            o.push_str("}\n\n");
        }

        let ret = if outs.len() == 1 {
            self.go_ty(&self.ty_of(&outs[0].name.text))
        } else {
            o.push_str("type Output struct {\n");
            for od in outs {
                o.push_str(&format!("\t{}{CELL}{}\n", pascal(&pub_name(&od.name)), self.go_ty(&self.ty_of(&od.name.text))));
            }
            o.push_str("}\n\n");
            "Output".into()
        };
        let zero = if outs.len() == 1 {
            self.go_zero(&self.ty_of(&outs[0].name.text))
        } else {
            "Output{}".to_string()
        };

        // Go's defined types cannot be mixed in arithmetic. Brands are enforced on the public
        // surface (Input and the return value) while internal arithmetic runs on plain int64 (the
        // flip side of the asymmetry in §8.3).
        let local = |n: &str| -> String {
            match self.f.inputs.iter().find(|i| i.name.text == n) {
                Some(i) => {
                    let f = format!("in.{}", pascal(&pub_name(&i.name)));
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                        format!("int64({f})")
                    } else {
                        f
                    }
                }
                None => self.ident(n),
            }
        };

        // The public function keeps the plain signature; the branches live in its traced
        // twin, which also returns the rows that matched (§15.33).
        let traced = format!("{fname}Traced");
        o.push_str(&format!("// {fname} {}\n", plain_doc(&self.f.name.text, &self.f.version, &traced)));
        o.push_str(&format!(
            "func {fname}(in Input) ({ret}, error) {{\n\tout, _, err := {traced}(in)\n\treturn out, err\n}}\n\n"
        ));
        o.push_str(&format!("// {traced} {}\n", traced_doc(&self.f.name.text, &self.f.version)));
        o.push_str(&format!("func {traced}(in Input) ({ret}, []Fired, error) {{\n"));

        for i in &self.f.inputs {
            let v = local(&i.name.text);
            let val = go_i64(&v);
            let ty = self.ty_of(&i.name.text);
            match &ty {
                Ty::Enum(_) => o.push_str(&format!(
                    "\tif !{v}.Valid() {{\n\t\treturn {zero}, nil, &RuleInputError{{What: \"{}\", Value: {val}, HasValue: true}}\n\t}}\n",
                    tr!("{} が列挙の値ではありません", "{} is not a value of the enum", i.name.text)
                )),
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                    if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text) {
                        let sc = self.c.wire_scale(&i.name.text);
                        o.push_str(&format!(
                            "\tif {v} < {} || {v} > {} {{\n\t\treturn {zero}, nil, &RuleInputError{{What: \"{}\", Value: {val}, HasValue: true}}\n\t}}\n",
                            crate::types::wire_int(*lo, sc),
                            crate::types::wire_int(*hi, sc),
                            tr!("{} が範囲の外です", "{} is out of range", i.name.text)
                        ));
                    }
                }
                _ => {}
            }
        }

        o.push_str(&self.constraint_guards(&local, Lang::Go, "\t", |m| {
            format!("\t\treturn {zero}, nil, &RuleInputError{{What: \"{m}\"}}\n")
        }));

        let trace = self.temp("trace");
        o.push_str(&format!("\tvar {trace} []Fired\n"));

        match &self.f.fold {
            Some(fold) => o.push_str(&self.go_walk(fold, &trace, &zero)),
            None if self.counts().is_empty() => o.push_str(&self.go_items(&local, &trace, Phase::All)),
            None => o.push_str(&self.go_count_walk(&local, &trace, &zero)),
        }

        // Every output takes the same three steps: its source — the `result` expression for
        // the first output, the binding of its own name otherwise — brought to the wire
        // scale, then its declared rounding applied exactly once (§15.16).
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let ty = self.ty_of(out_name);
            finals.push(match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os {
                        let raw = self.temp(&raw_base(oi));
                        o.push_str(&format!(
                            // The cast needs its own parentheses: an expression that begins with
                            // a call rather than a `(` glued itself to the type name.
                            "\t{raw} := int64({}){CELL}// {}\n",
                            go_expr(&res.text),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("round{}({raw}, {}) / {}", pascal(mode_fn(m)), grid_i, res.scale / os)
                    } else {
                        format!("round{}(int64({}), {})", pascal(mode_fn(m)), go_expr(&res.text), grid_i)
                    }
                }
                // Only a number needs the widening cast; `int64(可否)` does not compile.
                None if !ty.is_numeric() => go_expr(&res.text),
                None => format!("int64({})", go_expr(&res.text)),
            });
        }
        if outs.len() == 1 {
            o.push_str(&format!("\treturn {ret}({}), {trace}, nil\n}}\n", finals[0]));
        } else {
            // Each output gets its own rounding, applied exactly once (§7.2).
            let fields: Vec<String> = outs
                .iter()
                .zip(&finals)
                .map(|(od, body)| {
                    format!("{}: {}({body})", pascal(&pub_name(&od.name)), self.go_ty(&self.ty_of(&od.name.text)))
                })
                .collect();
            o.push_str(&format!("\treturn Output{{{}}}, {trace}, nil\n}}\n", fields.join(", ")));
        }
        o
    }

    fn go_table(&self, t: &Table, local: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = format!("\t// {}\n", self.table_head(t));
        // In Go a variable declared inside an if does not escape it, so declare them up front.
        for oc in &t.outputs {
            let t2 = self.ty_of(&oc.name.text);
            let ty = if matches!(t2, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                "int64".to_string()
            } else {
                self.go_ty(&t2)
            };
            o.push_str(&format!("\tvar {} {ty}\n", self.ident(&oc.name.text)));
        }
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.go_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            // A set cell comes in parentheses so that it can sit beside other conditions; alone
            // on the `if`, gofmt strips them, so they are stripped here.
            let cond: String = match conds.as_slice() {
                [] => "true".to_string(),
                [one] => unparen(one).to_string(),
                many => many.join(" && "),
            };
            let kw = if ri == 0 { "\tif" } else { " else if" };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            let line = self.row_head(t, ri, &cells.join(" | "));
            if ri == 0 {
                o.push_str(&format!("{kw} {cond} {{ // {line}\n"));
            } else {
                o.push_str(&format!("\t}}{kw} {cond} {{ // {line}\n"));
            }
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let ty = self.ty_of(&oc.name.text);
                        self.int_lit(n, &ty, self.scale(&oc.name.text))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.go_value(w),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        Lit::Str(x) => str_lit(x),
                        _ => "0".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "true".into()
                        } else if w == crate::kw::FALSE {
                            "false".into()
                        } else if self.value_names.contains_key(w) {
                            self.go_value(w)
                        } else {
                            self.rescaled(w, &oc.name.text, local(w))
                        }
                    }
                    None => "0".into(),
                };
                o.push_str(&format!("\t\t{} = {v}\n", self.ident(&oc.name.text)));
            }
            let (tn, rn, lbl) = self.fired_of(t, ri);
            o.push_str(&format!("\t\t{trace} = append({trace}, Fired{{{tn:?}, {rn}, {lbl:?}}})\n"));
        }
        o.push_str(&format!(
            "\t}} else {{\n\t\tpanic(\"{}\")\n\t}}\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        for oc in &t.outputs {
            o.push_str(&self.go_unread(&oc.name.text));
        }
        o.push_str(&self.guards(t, local, Lang::Go, "\t", |name, i, j| {
            format!(
                "\t\treturn {}, nil, &RuleContradictionError{{What: \"{}\"}}\n",
                if self.f.outputs.len() == 1 {
                    self.go_zero(&self.ty_of(&self.f.outputs[0].name.text))
                } else {
                    "Output{}".into()
                },
                tr!("表 {name}: {i} と {j} が同時に当てはまりました", "table {name}: {i} and {j} matched at the same time")
            )
        }));
        o
    }
}

/// Adapt an expression built for Python to Go's spelling. Only integer division differs.
/// A definition that is one literal (`define 介護保険料率 = 1.62%`) would be declared `int`
/// by `:=`, and the first `+` with an `int64` neighbour would not compile (§15.72).
fn go_typed(s: &str) -> String {
    if s.parse::<i128>().is_ok() { format!("int64({s})") } else { s.to_string() }
}

fn go_expr(s: &str) -> String {
    let t = s
        .replace(" // ", " / ")
        .replace("True", "true")
        .replace("False", "false")
        .replace("_round_down(", "roundDown(")
        .replace("_round_up(", "roundUp(")
        .replace("_round_half_down(", "roundHalfDown(")
        .replace("_round_half(", "roundHalf(")
        .replace("_round_bankers(", "roundBankers(")
        .replace("_min(", "minInt(")
        .replace("_max(", "maxInt(");
    tighten_nested_products(&t)
}

/// Match one habit of `gofmt`: a product written inside a group that sits two or more call
/// arguments deep loses the spaces around its operator.
///
/// The generator formats its own Go rather than shelling out to `gofmt` (§8.5: the output
/// must not depend on the version of a tool installed on the machine), and a test then holds
/// it to `gofmt -l` being empty. That test is what turned this up: `roundDown(minInt(x, (int64(in.Cap) * 100)), (1 * 100))`
/// comes back from `gofmt` with the first product tightened and the second left alone. The
/// rule below reproduces that on the shapes the generator emits — at one call deep the spaces
/// stay, which is what `gofmt` does too.
fn tighten_nested_products(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    // For every open paren still on the stack: was it a call's, or a grouping's?
    let mut stack: Vec<bool> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == '(' {
            let call = i > 0 && (b[i - 1].is_alphanumeric() || b[i - 1] == '_');
            stack.push(call);
        } else if c == ')' {
            stack.pop();
        }
        let calls = stack.iter().filter(|x| **x).count();
        let in_group = stack.last() == Some(&false);
        if in_group && calls >= 2 && c == ' ' && i + 2 < b.len() && (b[i + 1] == '*' || b[i + 1] == '/') && b[i + 2] == ' ' {
            out.push(b[i + 1]);
            i += 3;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// The value in a Go error is an `int64`; a wire value already is one, so the cast is
/// written only when it is not (an enum, a length).
fn go_i64(v: &str) -> String {
    if v.starts_with("int64(") && v.ends_with(')') { v.to_string() } else { format!("int64({v})") }
}

fn round_go() -> String {
    tr!(
        "// §7.3 の五モード。負の向きと半分ちょうどまで仕様どおりに固定する。\n\
         // 言語の素の除算に任せない（Python の // は −∞ 方向、Go は 0 方向）。\n",
        "// The five modes of §7.3. Negative values and exact halves are pinned to the spec as well;\n\
         // nothing is left to the language's native division (Python's // goes toward −∞, Go's toward 0).\n"
    ) + ROUND_GO_BODY
}

const ROUND_GO_BODY: &str = r#"
// RuleInputError is a contract violation by the caller: outside the declared input domain.
// The sentence and the value travel apart (§15.95), so a caller can react to which input
// was refused without parsing the text back.
type RuleInputError struct {
	What     string
	Value    int64
	HasValue bool
}

func (e *RuleInputError) Error() string {
	if !e.HasValue {
		return e.What
	}
	return fmt.Sprintf("%s: %d", e.What, e.Value)
}

// RuleContradictionError is a contradiction in the rule itself, not a mistake by the caller.
type RuleContradictionError struct {
	What string
}

func (e *RuleContradictionError) Error() string {
	return e.What
}
func absMod(x, g int64) (int64, int64, bool) {
	neg := x < 0
	if neg {
		x = -x
	}
	return x / g, x % g, neg
}

func roundDown(x, g int64) int64 {
	q, _, neg := absMod(x, g)
	v := q * g
	if neg {
		return -v
	}
	return v
}

func roundUp(x, g int64) int64 {
	q, r, neg := absMod(x, g)
	if r != 0 {
		q++
	}
	v := q * g
	if neg {
		return -v
	}
	return v
}

func roundHalf(x, g int64) int64 {
	q, r, neg := absMod(x, g)
	if 2*r >= g {
		q++
	}
	v := q * g
	if neg {
		return -v
	}
	return v
}

func roundHalfDown(x, g int64) int64 {
	q, r, neg := absMod(x, g)
	if 2*r > g {
		q++
	}
	v := q * g
	if neg {
		return -v
	}
	return v
}

func minInt(a, b int64) int64 {
	if a < b {
		return a
	}
	return b
}

func maxInt(a, b int64) int64 {
	if a > b {
		return a
	}
	return b
}

func roundBankers(x, g int64) int64 {
	q, r, neg := absMod(x, g)
	if 2*r > g || (2*r == g && q%2 == 1) {
		q++
	}
	v := q * g
	if neg {
		return -v
	}
	return v
}

"#;

/// Our own column alignment, matching gofmt's tabwriter.
///
/// Running gofmt as a later stage would make the generated code depend on whichever gofmt is
/// installed in that environment, breaking the determinism of §8.5. Alignment is local to a run
/// of declarations, so doing it ourselves is enough. The control character used as a separator
/// marks consecutive lines with the same number of columns as one block to align together.
const CELL: char = '\u{1f}';
const IMPORT_MARK: &str = "\u{1e}IMPORTS\u{1e}";

/// Column width is measured in runes. That is how Go's text/tabwriter counts, so measuring by
/// display width (`diag::width`, which counts a full-width character as 2) would drift from gofmt
/// by one character wherever Japanese identifiers line up. §8.5 requires `gofmt -l` to print
/// nothing, so we follow gofmt's logic rather than the visual one.
fn cells_wide(s: &str) -> usize {
    s.chars().count()
}

/// Drop the parentheses wrapping the whole expression. The generator parenthesizes every
/// subexpression uniformly, which leaves one redundant outer pair on the right-hand side of an
/// assignment. Python's formatter wants it gone, so to keep `ruff format --check` green we drop
/// it at generation time (honoring §8.5's "do not rely on a later formatting stage" on the Python
/// side too). Go is left alone, since gofmt does not remove it.
fn unparen(s: &str) -> &str {
    let t = s.trim();
    let Some(inner) = t.strip_prefix('(').and_then(|x| x.strip_suffix(')')) else { return s };
    // Drop the outer pair only when it really matches itself. `(a) + (b)` is left alone.
    let mut depth = 0i32;
    for ch in inner.chars() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return s;
                }
            }
            _ => {}
        }
    }
    if depth == 0 { inner } else { s }
}

/// Fix up PEP 8 blank lines (E301–E305) in one place. Output assembly is scattered across the
/// generator, and forgetting a single blank line turns the formatter red. Rather than being
/// careful at every emitting site, run one pass at the end: two blank lines around top-level
/// `class` / `def` / decorators, at most one before any other top-level statement.
fn pep8_blanks(src: &str) -> String {
    let lines: Vec<&str> = src.trim_end().split('\n').collect();
    let mut out: Vec<String> = Vec::new();
    let mut in_block = false; // whether the previous top-level construct was a def / class body
    for line in lines {
        let top = !line.is_empty() && !line.starts_with(char::is_whitespace);
        if line.trim().is_empty() {
            continue; // blank lines are dropped and reinserted where needed
        }
        if top {
            let starts = line.starts_with("class ")
                || line.starts_with("def ")
                || line.starts_with('@');
            let want = if starts || in_block { 2 } else { 1 };
            if !out.is_empty() {
                let have = out.iter().rev().take_while(|l| l.is_empty()).count();
                for _ in have..want {
                    out.push(String::new());
                }
            }
            if starts {
                in_block = true;
            } else if !line.starts_with('#') {
                in_block = false;
            }
        }
        out.push(line.to_string());
    }
    // Drop blank lines at the start of the file.
    while out.first().is_some_and(|l| l.is_empty()) {
        out.remove(0);
    }
    out.push(String::new());
    out.join("\n")
}

fn align(src: &str) -> String {
    let lines: Vec<&str> = src.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        if !lines[i].contains(CELL) {
            out.push(lines[i].to_string());
            i += 1;
            continue;
        }
        let n = lines[i].matches(CELL).count();
        let start = i;
        while i < lines.len() && lines[i].contains(CELL) && lines[i].matches(CELL).count() == n {
            i += 1;
        }
        let rows: Vec<Vec<&str>> = lines[start..i].iter().map(|l| l.split(CELL).collect()).collect();
        let mut w = vec![0usize; n];
        for r in &rows {
            for (k, c) in r.iter().take(n).enumerate() {
                w[k] = w[k].max(cells_wide(c));
            }
        }
        for r in rows {
            let mut line = String::new();
            for (k, c) in r.iter().enumerate() {
                line.push_str(c);
                if k < n && r[k + 1..].iter().any(|x| !x.trim().is_empty()) {
                    line.push_str(&" ".repeat(w[k] - cells_wide(c) + 1));
                }
            }
            out.push(line.trim_end().to_string());
        }
    }
    out.join("\n")
}

// ---------------------------------------------------------------------------
// Runners (§9.3: feed the vectors through every generated language and compare the answers)
// ---------------------------------------------------------------------------

impl<'a> Gen<'a> {
    /// A Python runner that reads JSONL from stdin and prints just the outputs, one record
    /// per line. It is also the worked example of the calling convention, so it constructs
    /// each branded argument the way a caller has to — `mypy --strict` checks both (§15.22).
    pub fn python_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let mut args: Vec<String> = Vec::new();
        // One reader, used for the rule's own inputs and for an element's fields. There used
        // to be a second copy inline below, and it was the one that ran — which is how the
        // optional input got no arm anywhere and came out as `int(d[…])` (§15.88).
        fn read_arg(g: &Gen, ty: &Ty, expr: String) -> String {
            match ty {
                Ty::Enum(n) => {
                    let cls = g.enum_names.get(n).cloned().unwrap_or_default();
                    format!("m.{cls}({expr})")
                }
                Ty::Bool => format!("bool({expr})"),
                Ty::Date => format!("_ord({expr})"),
                Ty::Str => format!("str({expr})"),
                // A branded input has to be constructed, exactly as a caller must.
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => {
                    format!("m.{}(int({expr}))", brand_of(ty))
                }
                // The absent value is `null` on the wire (§10.2), and `None` in Python.
                Ty::Opt(inner) => {
                    format!("(None if {expr} is None else {})", read_arg(g, inner, expr.clone()))
                }
                _ => format!("int({expr})"),
            }
        }
        let read = |ty: &Ty, expr: String| -> String { read_arg(self, ty, expr) };
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            args.push(read(&ty, format!("d[{:?}]", i.name.text)));
        }
        // The sequence a walk reads is built the same way, one element at a time (§15.56).
        let mut prelude = String::new();
        if let Some(el) = &self.f.elements {
            let jp = &el.name.text;
            let fields: Vec<String> = el
                .fields
                .iter()
                .map(|fd| read(&self.ty_of(&fd.name.text), format!("e[{:?}]", fd.name.text)))
                .collect();
            prelude = format!(
                "    rows = [m.Element({}) for e in d[{jp:?}]]\n",
                if fields.len() == 1 { format!("{},", fields[0]) } else { fields.join(", ") }
            );
            args.push("rows".to_string());
        }
        // The runner prints the record the module itself writes, so the agreement test
        // holds the record function — the wire form of every input, dates included — to
        // the reference evaluator in every language (§15.35).
        format!(
            "# Code generated by rulec {}. DO NOT EDIT.\n\
             import datetime\n\
             import json\n\
             import sys\n\n\
             import {alias} as m\n\n\
             def _ord(s: str) -> int:\n    \
                 y, mo, d = (int(x) for x in s.split(\"-\"))\n    \
                 return (datetime.date(y, mo, d) - datetime.date(1970, 1, 1)).days\n\n\
             for line in sys.stdin:\n    \
                 line = line.strip()\n    \
                 if not line:\n        \
                     continue\n    \
                 d = json.loads(line)[\"in\"]\n\
                 {}    \
                 args = ({})\n    \
                 r, trace = m.{alias}_traced(*args)\n    \
                 print(m.{alias}_record(*args, r, trace))\n",
            env!("CARGO_PKG_VERSION"),
            prelude,
            if args.len() == 1 { format!("{},", args[0]) } else { args.join(", ") }
        )
    }

    /// A Go runner that does the same. encoding/json is in the standard library.
    pub fn go_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let pkg = alias.replace('_', "").to_lowercase();
        let fname = pascal(&alias);
        // One field of a JSON object into one field of a struct. The element loop below reads
        // the same shapes out of its own object, so the two stay one piece of code.
        let field = |name: &crate::ast::Name, ty: &Ty, into: &str, src: &str, ind: &str| -> String {
            let jp = &name.text;
            let g = pascal(&pub_name(name));
            match ty {
                Ty::Enum(n) => {
                    let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                    format!("{ind}v{g}, _ := r.Parse{cls}(str({src}[{jp:?}]))\n{ind}{into}.{g} = v{g}\n")
                }
                Ty::Bool => format!("{ind}{into}.{g} = {src}[{jp:?}] == true\n"),
                Ty::Date => format!("{ind}{into}.{g} = ord(str({src}[{jp:?}]))\n"),
                Ty::Str => format!("{ind}{into}.{g} = str({src}[{jp:?}])\n"),
                // A number is a plain int64, not a type the package declares, so it must not
                // be qualified with the package name.
                Ty::Number => format!("{ind}{into}.{g} = int64(num({src}[{jp:?}]))\n"),
                // `null` on the wire is a nil pointer. It used to fall through to the
                // branded number, so the runner would not compile at all (§15.88).
                Ty::Opt(inner) => {
                    let inner_ty = inner.as_ref();
                    let take = match inner_ty {
                        Ty::Enum(n) => {
                            let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                            format!("{ind}\tv{g}, _ := r.Parse{cls}(str({src}[{jp:?}]))\n")
                        }
                        Ty::Bool => format!("{ind}\tv{g} := {src}[{jp:?}] == true\n"),
                        Ty::Date => format!("{ind}\tv{g} := ord(str({src}[{jp:?}]))\n"),
                        Ty::Str => format!("{ind}\tv{g} := str({src}[{jp:?}])\n"),
                        Ty::Number => format!("{ind}\tv{g} := int64(num({src}[{jp:?}]))\n"),
                        other => format!("{ind}\tv{g} := r.{}(num({src}[{jp:?}]))\n", self.go_ty(other)),
                    };
                    format!(
                        "{ind}if {src}[{jp:?}] != nil {{\n{take}{ind}\t{into}.{g} = &v{g}\n{ind}}}\n"
                    )
                }
                _ => format!("{ind}{into}.{g} = r.{}(num({src}[{jp:?}]))\n", self.go_ty(ty)),
            }
        };
        let mut fields: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            fields.push(field(&i.name, &self.ty_of(&i.name.text), "in", "d", "\t\t"));
        }
        // The sequence arrives as an array of objects (§10.2), one per element.
        if let Some(el) = &self.f.elements {
            let jp = &el.name.text;
            let g = pascal(&pub_name(&el.name));
            let mut body = format!("\t\tfor _, ev := range arr(d[{jp:?}]) {{\n\t\t\te := obj(ev)\n\t\t\tvar el r.Element\n");
            for fd in &el.fields {
                body.push_str(&field(&fd.name, &self.ty_of(&fd.name.text), "el", "e", "\t\t\t"));
            }
            body.push_str(&format!("\t\t\tin.{g} = append(in.{g}, el)\n\t\t}}\n"));
            fields.push(body);
        }
        format!(
            "// Code generated by rulec {}. DO NOT EDIT.\n\
             package main\n\n\
             import (\n\t\"bufio\"\n\t\"encoding/json\"\n\t\"fmt\"\n\t\"os\"\n\t\"time\"\n\n\tr \"{pkg}\"\n)\n\n\
             func str(v any) string {{ s, _ := v.(string); return s }}\n\n\
             func num(v any) int64 {{ f, _ := v.(float64); return int64(f) }}\n\n\
             func arr(v any) []any {{ a, _ := v.([]any); return a }}\n\n\
             func obj(v any) map[string]any {{ m, _ := v.(map[string]any); return m }}\n\n\
             func ord(s string) int64 {{\n\t\
                 t, _ := time.Parse(\"2006-01-02\", s)\n\t\
                 return int64(t.Sub(time.Date(1970, 1, 1, 0, 0, 0, 0, time.UTC)).Hours() / 24)\n}}\n\n\
             func main() {{\n\t\
                 sc := bufio.NewScanner(os.Stdin)\n\t\
                 sc.Buffer(make([]byte, 1<<20), 1<<20)\n\t\
                 for sc.Scan() {{\n\t\t\
                     if len(sc.Bytes()) == 0 {{\n\t\t\tcontinue\n\t\t}}\n\t\t\
                     var rec struct {{\n\t\t\tIn map[string]any `json:\"in\"`\n\t\t}}\n\t\t\
                     if err := json.Unmarshal(sc.Bytes(), &rec); err != nil {{\n\t\t\tpanic(err)\n\t\t}}\n\t\t\
                     d := rec.In\n\t\t\
                     var in r.Input\n{}\t\t\
                     got, trace, err := r.{fname}Traced(in)\n\t\t\
                     if err != nil {{\n\t\t\tpanic(err)\n\t\t}}\n\t\t\
                     fmt.Println(r.{fname}Record(in, got, trace, \"\"))\n\t}}\n}}\n",
            env!("CARGO_PKG_VERSION"),
            fields.join("")
        )
    }
}


// ---------------------------------------------------------------------------
// TypeScript (§8.3)
// ---------------------------------------------------------------------------

/// Turn the shared expression text into TypeScript. The same post-processing shape as
/// `go_expr`: one grammar is emitted once and each language adjusts the spellings it does
/// not share.
///
/// Every integer becomes a `bigint` literal. The overflow proof (E108) is against int64, and
/// a JavaScript `number` is exact only to 2^53, so using one would put a silent wrong answer
/// above nine quadrillion into a tool whose whole claim is that it does not do that.
fn ts_expr(s: &str) -> String {
    let t = s
        .replace(" // ", " / ")
        .replace("True", "true")
        .replace("False", "false")
        .replace("_round_down(", "_roundDown(")
        .replace("_round_up(", "_roundUp(")
        .replace("_round_half_down(", "_roundHalfDown(")
        .replace("_round_half(", "_roundHalf(")
        .replace("_round_bankers(", "_roundBankers(")
        .replace("_min(", "_min(")
        .replace("_max(", "_max(");
    bigint_literals(&t)
}

/// Append `n` to every integer literal, and only to those. A run of digits that touches a
/// letter, `_` or `.` on either side belongs to a name (`SizeClass.S60`, `項目1`) and is left
/// alone.
fn bigint_literals(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let part = |c: char| c.is_alphanumeric() || c == '_' || c == '.';
    let mut out = String::with_capacity(s.len() + 8);
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !part(b[i - 1])) {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let touches_name = i < b.len() && part(b[i]);
            out.extend(&b[start..i]);
            if !touches_name {
                out.push('n');
            }
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// The five modes of §7.3 on `bigint`. `/` on a bigint truncates toward zero, the same as
/// Go's integer division, so these mirror the Go helpers rather than the Python ones.
fn round_ts() -> String {
    format!(
        r#"
export const NO_VALUE = Symbol("no value");

export class RuleInputError extends Error {{
  readonly what: string;
  readonly value: unknown;
  constructor(what: string, value: unknown = NO_VALUE) {{
    super(value === NO_VALUE ? what : `${{what}}: ${{String(value)}}`);
    this.name = "RuleInputError";
    this.what = what;
    this.value = value;
  }}
}}

export class RuleContradictionError extends Error {{
  readonly what: string;
  constructor(what: string) {{
    super(what);
    this.name = "RuleContradictionError";
    this.what = what;
  }}
}}

function _min(a: bigint, b: bigint): bigint {{
  return a < b ? a : b;
}}

function _max(a: bigint, b: bigint): bigint {{
  return a > b ? a : b;
}}

/** {down} */
function _roundDown(x: bigint, g: bigint): bigint {{
  const a = x < 0n ? -x : x;
  const v = (a / g) * g;
  return x < 0n ? -v : v;
}}

/** {up} */
function _roundUp(x: bigint, g: bigint): bigint {{
  const a = x < 0n ? -x : x;
  const v = a % g === 0n ? (a / g) * g : (a / g + 1n) * g;
  return x < 0n ? -v : v;
}}

/** {half} */
function _roundHalf(x: bigint, g: bigint): bigint {{
  const a = x < 0n ? -x : x;
  const v = 2n * (a % g) >= g ? (a / g + 1n) * g : (a / g) * g;
  return x < 0n ? -v : v;
}}

/** {half_down} */
function _roundHalfDown(x: bigint, g: bigint): bigint {{
  const a = x < 0n ? -x : x;
  const v = 2n * (a % g) > g ? (a / g + 1n) * g : (a / g) * g;
  return x < 0n ? -v : v;
}}

/** {bankers} */
function _roundBankers(x: bigint, g: bigint): bigint {{
  const a = x < 0n ? -x : x;
  let q = a / g;
  const r = a % g;
  if (2n * r > g || (2n * r === g && q % 2n === 1n)) {{
    q += 1n;
  }}
  const v = q * g;
  return x < 0n ? -v : v;
}}
"#,
        down = tr!("0 へ寄せる。-4.8円 → -4円。", "Toward zero: -4.8 yen → -4 yen."),
        up = tr!("0 から遠ざける。-4.2円 → -5円。", "Away from zero: -4.2 yen → -5 yen."),
        half = tr!("半分ちょうどは 0 から遠ざける。", "An exact half goes away from zero."),
        half_down = tr!("半分ちょうどは 0 へ寄せる。", "An exact half goes toward zero."),
        bankers = tr!("半分ちょうどは偶数へ。", "An exact half goes to the even neighbor."),
    )
}

/// Which language a shared emitter is writing for. `guards` is the one body three languages
/// share, and it used to tell Python from Go by whether the indent was a tab — which quietly
/// handed TypeScript the Python spelling.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Py,
    Go,
    Ts,
    Rs,
    Rb,
    Sw,
    Sql,
    Php,
    Java,
}

/// The few spellings shared code needs to know per language. Keeping them in one row each
/// is what lets `guards()` be written once: it used to carry three separate `match`es, and
/// a sixth language meant finding all three (§15.20).
struct Spelling {
    /// How a line comment starts.
    comment: &'static str,
    /// How two conditions are joined.
    and: &'static str,
    /// The condition as an `if` head, without the indent.
    if_head: fn(&str) -> String,
    /// What closes the block, or "" for a language that closes by indentation.
    close: &'static str,
}

impl Lang {
    /// How the language spells "not" in front of a parenthesised condition.
    fn not(self) -> &'static str {
        match self {
            Lang::Py => "not ",
            Lang::Sql => "NOT ",
            _ => "!",
        }
    }

    fn spelling(self) -> Spelling {
        match self {
            Lang::Py => Spelling {
                comment: "#",
                and: " and ",
                if_head: |c| format!("if {c}:"),
                close: "",
            },
            Lang::Rb => Spelling {
                comment: "#",
                and: " && ",
                if_head: |c| format!("if {c}"),
                close: "end",
            },
            // PHP is spelled like TypeScript here — braces, `&&`, `//` — and differs only
            // where §15.77 says it does: `intdiv` for division, and typed parameters.
            Lang::Ts | Lang::Php | Lang::Java => Spelling {
                comment: "//",
                and: " && ",
                if_head: |c| format!("if ({c}) {{"),
                close: "}",
            },
            Lang::Sql => Spelling {
                comment: "--",
                and: " AND ",
                if_head: |c| format!("WHEN {c} THEN"),
                close: "",
            },
            Lang::Go | Lang::Rs | Lang::Sw => Spelling {
                comment: "//",
                and: " && ",
                if_head: |c| format!("if {c} {{"),
                close: "}",
            },
        }
    }
}

impl<'a> Gen<'a> {
    /// The TypeScript type for a value. Numbers are branded `bigint`s: a brand costs nothing
    /// at runtime and still refuses `YenInclTax` where `YenExclTax` was meant.
    fn ts_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => self.enum_names.get(n).cloned().unwrap_or_else(|| "string".into()),
            Ty::Bool => "boolean".into(),
            Ty::Str => "string".into(),
            Ty::Number | Ty::Date => "bigint".into(),
            Ty::Opt(t) => format!("{} | null", self.ts_ty(t)),
            _ => brand_of(ty),
        }
    }

    fn ts_value(&self, v: &str) -> String {
        match self.value_names.get(v) {
            Some((ty, alias)) => format!("{ty}.{}", alias.to_uppercase()),
            None => format!("{v:?}"),
        }
    }

    /// Render a cell as a TypeScript condition. A don't-care yields None (no condition).
    fn ts_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        let numeric = inner.is_numeric() || matches!(inner, Ty::Date);
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.ts_value(w),
                Lit::Num(n) => format!("{}n", self.int_lit(n, inner, col_scale)),
                Lit::Date(y, m, d) => format!("{}n", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => format!("{s:?}"),
            }
        };
        let members = |ls: &Vec<Lit>| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.ts_value(m)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            format!("[{}]", out.join(", "))
        };
        // `===` on a bigint and on a string are both value comparisons, so one spelling does
        // for every type here.
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!("{var} === null"),
            Cell::Prefix(ps) => ps
                .iter()
                .map(|p| format!("{var}.startsWith({})", format!("{p:?}")))
                .collect::<Vec<_>>()
                .join(" || "),
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                format!("_{}.has({var})", self.ident(w))
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("!{var}"),
            Cell::Lit(l) => format!("{var} === {}", lit(l)),
            Cell::Set(ls) => format!("{}.includes({var})", members(ls)),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                format!("!_{}.has({var})", self.ident(w))
            }
            Cell::Not(ls) => format!("!{}.includes({var})", members(ls)),
            Cell::Cmp(cs) => cs
                .iter()
                .map(|(o, l)| {
                    let op = match o {
                        CmpOp::Le => "<=",
                        CmpOp::Ge => ">=",
                        CmpOp::Lt => "<",
                        CmpOp::Gt => ">",
                    };
                    let _ = numeric;
                    format!("{var} {op} {}", lit(l))
                })
                .collect::<Vec<_>>()
                .join(" && "),
        })
    }

    pub fn typescript(&self) -> String {
        let mut o = self.header("//");
        o.push('\n');

        // Brands. A branded bigint is still a bigint at runtime; the brand exists only for
        // the type checker, exactly as `NewType` does on the Python side.
        let mut brands: BTreeMap<String, String> = BTreeMap::new();
        for v in self.f.inputs.iter().map(|i| &i.name.text).chain(self.f.outputs.iter().map(|o| &o.name.text)) {
            let ty = self.ty_of(v);
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                brands.insert(brand_of(&ty), format!("{ty}"));
            }
        }
        for (b, doc) in &brands {
            o.push_str(&format!("export type {b} = bigint & {{ readonly __rulec: \"{b}\" }}; // {doc}\n"));
        }
        if !brands.is_empty() {
            o.push('\n');
        }

        o.push_str(&format!("/** {} */\nexport type Fired = {{ readonly table: string; readonly row: number; readonly label?: string }};\n\n", fired_doc()));

        // The error classes come first: the enum parsers below throw them.
        o.push_str(&round_ts());
        o.push('\n');

        // Enums. A frozen object plus a union type, not `enum`: that keeps the file to
        // erasable syntax, so `node file.ts` runs it with no build step at all.
        let mut emitted: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if emitted.contains(ascii) {
                continue;
            }
            emitted.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            o.push_str(&format!("export const {ascii} = {{\n"));
            for v in vals {
                let name = self.value_names.get(v).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| v.clone());
                o.push_str(&format!("  {name}: {v:?},\n"));
            }
            o.push_str("} as const;\n");
            o.push_str(&format!(
                "export type {ascii} = (typeof {ascii})[keyof typeof {ascii}];\n\n"
            ));
            o.push_str(&format!(
                "export function parse{ascii}(s: string): {ascii} {{\n  \
                 const v = Object.values({ascii}).find((x) => x === s);\n  \
                 if (v === undefined) {{\n    \
                 throw new RuleInputError(`{}`);\n  }}\n  \
                 return v;\n}}\n\n",
                tr!("${{s}} は列挙 {ascii} の値ではありません", "${{s}} is not a value of enum {ascii}")
            ));
        }

        for g in &self.f.groups {
            let ms: Vec<String> = g.members.iter().map(|m| self.ts_value(&m.text)).collect();
            o.push_str(&format!(
                "const _{}: ReadonlySet<string> = new Set([{}]);\n",
                self.ident(&g.name.text),
                ms.join(", ")
            ));
        }
        if !self.f.groups.is_empty() {
            o.push('\n');
        }

        o.push_str(&self.ts_fn());
        o.push('\n');
        o.push_str(&self.ts_record());
        o
    }


    /// The rule's items as TypeScript, at the base indentation. `local` decides how a name
    /// is spelled, which is where a walk puts the element's own fields (§15.56).
    /// The walk, in TypeScript (§15.56). The shape is the one `py_walk` writes: accumulators,
    /// one pass per element with the body inside it, and the answer bound to the output's own
    /// name so the rounding and the return below are the ones every rule uses.
    fn ts_walk(&self, fold: &crate::ast::FoldDecl, trace: &str) -> String {
        let el = self.f.elements.as_ref().expect("a fold has elements");
        let seq = pub_name(&el.name);
        let fields: BTreeMap<String, String> =
            el.fields.iter().map(|fd| (fd.name.text.clone(), pub_name(&fd.name))).collect();
        let (answer, stopped, taken, kept, best) = self.fold_locals();
        let e = self.temp("elem");
        let held = self.temp("held");
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => format!("{e}.{f}"),
                None if n == crate::kw::HELD => held.clone(),
                None => self.ident(n),
            }
        };
        let out_name = self.f.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
        let out_ty = self.ty_of(&out_name);
        let acc_ty = if out_ty.is_numeric() { "bigint".to_string() } else { self.ts_ty(&out_ty) };
        let text = |x: &Option<Expr>| -> String {
            x.as_ref().map(|x| ts_expr(unparen(&self.expr(x, &local).text)).to_string()).unwrap_or_else(|| "0n".to_string())
        };

        let mut o = String::new();
        o.push_str(&format!("  let {answer}: {acc_ty} = {}; // {}\n", text(&fold.empty), tr!("要素ゼロ件の答え", "the answer for no elements")));
        o.push_str(&format!("  let {stopped} = false;\n"));
        o.push_str(&format!("  let {taken}: {acc_ty} | null = null;\n"));
        o.push_str(&format!("  let {kept}: {acc_ty} | null = null;\n"));
        o.push_str(&format!("  let {best}: bigint | null = null;\n"));
        o.push_str(&format!("  for (const {e} of {seq}) {{\n"));

        let mut body = self.ts_element_guards(&local);
        body.push_str(&self.ts_items(&local, trace, Phase::All));
        let v = local(&fold.verdict);
        for (name, arm, _) in &fold.arms {
            let cls = self.ts_ty(&self.ty_of(&fold.verdict));
            let member = self.value_names.get(&name.text).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| name.text.clone());
            body.push_str(&format!("  if ({v} === {cls}.{member}) {{ // {}\n", name.text));
            match arm {
                Arm::Next => body.push_str("    // next\n"),
                Arm::Stop(None) => body.push_str("    break;\n"),
                Arm::Stop(Some(x)) => {
                    body.push_str(&format!("    {answer} = {};\n", ts_expr(unparen(&self.expr(x, &local).text))));
                    body.push_str(&format!("    {stopped} = true;\n    break;\n"));
                }
                Arm::Take { expr, unique } => {
                    if *unique {
                        body.push_str(&format!("    if ({taken} !== null) {{\n"));
                        body.push_str(&format!(
                            "      throw new RuleContradictionError(`{}`);\n    }}\n",
                            tr!("畳み込み {}: take_unique に二件当たりました", "fold {}: two elements matched a take_unique", fold.verdict)
                        ));
                        body.push_str(&format!("    {taken} = {};\n", ts_expr(unparen(&self.expr(expr, &local).text))));
                    } else {
                        body.push_str(&format!("    if ({taken} === null) {{\n"));
                        body.push_str(&format!("      {taken} = {};\n    }}\n", ts_expr(unparen(&self.expr(expr, &local).text))));
                    }
                }
                Arm::KeepMax { expr, key } => {
                    let k = self.temp("key");
                    body.push_str(&format!("    const {k} = {};\n", ts_expr(unparen(&self.expr(key, &local).text))));
                    body.push_str(&format!("    if ({best} === null || {k} > {best}) {{\n"));
                    body.push_str(&format!("      {best} = {k};\n"));
                    body.push_str(&format!("      {kept} = {};\n    }}\n", ts_expr(unparen(&self.expr(expr, &local).text))));
                }
            }
            body.push_str("  }\n");
        }
        o.push_str(&Self::indent_block(&body, "  "));
        o.push_str("  }\n");

        o.push_str(&format!("  if ({seq}.length > 0 && !{stopped}) {{\n"));
        o.push_str(&format!(
            "    const {held} = {taken} !== null ? {taken} : ({kept} !== null ? {kept} : {});\n",
            text(&fold.empty)
        ));
        o.push_str(&format!("    {answer} = {};\n  }}\n", text(&fold.exhausted)));
        o.push_str(&format!("  const {} = {answer};\n", self.ident(&out_name)));
        o
    }

    /// The entry guards of one element, inside the walk.
    /// The counting walk in TypeScript (§15.58).
    fn ts_count_walk(&self, outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let el = self.f.elements.as_ref().expect("a count has elements");
        let seq = pub_name(&el.name);
        let fields: BTreeMap<String, String> =
            el.fields.iter().map(|fd| (fd.name.text.clone(), pub_name(&fd.name))).collect();
        let e = self.temp("elem");
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => format!("{e}.{f}"),
                None => self.ident(n),
            }
        };
        let mut o = String::new();
        if let Some(cap) = self.count_cap() {
            o.push_str(&format!(
                "  if ({seq}.length > {cap}) {{\n    throw new RuleInputError(`{}`, {seq}.length);\n  }}\n",
                tr!("{} の要素が多すぎます（上限 {cap}）", "{} has too many elements (at most {cap})", el.name.text)
            ));
        }
        for d in self.counts() {
            o.push_str(&format!("  let {}: bigint = 0n; // {}\n", self.ident(&d.name.text), tr!("数え上げ", "count")));
        }
        o.push_str(&format!("  for (const {e} of {seq}) {{\n"));
        let mut body = self.ts_element_guards(&local);
        body.push_str(&self.ts_items(&local, trace, Phase::Walk));
        for d in self.counts() {
            let v = local(&d.column.text);
            if d.kind == crate::ast::AggKind::Sum {
                body.push_str(&format!("  {} += {v}; // {}\n", self.ident(&d.name.text), d.name.text));
                continue;
            }
            let test = match self.count_member(d) {
                Some(w) => {
                    let cls = self.ts_ty(&self.ty_of(&d.column.text));
                    let member = self.value_names.get(&w.text).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| w.text.clone());
                    format!("{v} === {cls}.{member}")
                }
                None if self.count_negated(d) => format!("!{v}"),
                None => v,
            };
            body.push_str(&format!("  if ({test}) {{ // {}\n    {} += 1n;\n  }}\n", d.name.text, self.ident(&d.name.text)));
        }
        o.push_str(&Self::indent_block(&body, "  "));
        o.push_str("  }\n");
        for (d, cap) in self.sum_caps() {
            let n = self.ident(&d.name.text);
            o.push_str(&format!(
                "  if ({n} > {cap}n) {{\n    throw new RuleInputError(`{}`, {n});\n  }}\n",
                tr!("{} が範囲の外です", "{} is out of range", d.name.text)
            ));
        }
        o.push_str(&self.ts_items(outer, trace, Phase::Main));
        o
    }

    fn ts_element_guards(&self, local: &dyn Fn(&str) -> String) -> String {
        let mut o = String::new();
        for i in self.element_fields() {
            let v = local(&i.name.text);
            let ty = self.ty_of(&i.name.text);
            match &ty {
                Ty::Enum(n) => {
                    let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                    o.push_str(&format!(
                        "  if (!Object.values({cls}).includes({v})) {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                        tr!("{} が列挙 {cls} の値ではありません", "{} is not a value of enum {cls}", i.name.text)
                    ));
                }
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                    o.push_str(&format!(
                        "  if (typeof {v} !== \"bigint\") {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                        tr!("{} が整数ではありません", "{} is not an integer", i.name.text)
                    ));
                    if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) {
                        let sc = self.c.wire_scale(&i.name.text);
                        o.push_str(&format!(
                            "  if ({v} < {}n || {v} > {}n) {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                            crate::types::wire_int(lo, sc),
                            crate::types::wire_int(hi, sc),
                            tr!("{} が範囲の外です", "{} is out of range", i.name.text)
                        ));
                    }
                }
                _ => {}
            }
        }
        o
    }

    fn ts_items(&self, local: &dyn Fn(&str) -> String, trace: &str, phase: Phase) -> String {
        let mut o = String::new();
        for it in &self.f.items {
            if !self.in_phase(it, phase) {
                continue;
            }
            match it {
                Item::Derived(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!(
                        "  const {} = {}; // {}\n",
                        self.ident(&d.name.text),
                        ts_expr(unparen(&e.text)),
                        tr!("導出", "derived value")
                    ));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!(
                        "  const {} = {}; // {}\n",
                        self.ident(&d.name.text),
                        ts_expr(unparen(&e.text)),
                        tr!("定義", "definition")
                    ));
                }
                Item::Agg(_) => {}
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        o.push_str(&self.ts_table(t, local, trace));
                    }
                }
            }
        }
        o
    }

    fn ts_fn(&self) -> String {
        let fname = pub_name(&self.f.name);
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.ts_ty(&self.ty_of(&i.name.text))))
            .collect();
        if let Some(el) = &self.f.elements {
            params.push(format!("{}: readonly Element[]", pub_name(&el.name)));
        }
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 {
            self.ts_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let mut o = String::new();

        // One element is a row of inputs, so it gets the same shape an `Output` gets.
        if let Some(el) = &self.f.elements {
            o.push_str(&format!("/** {} */\n", tr!("{} の一件", "one of {}", el.name.text)));
            o.push_str("export interface Element {\n");
            for fd in &el.fields {
                o.push_str(&format!(
                    "  {}: {};\n",
                    pub_name(&fd.name),
                    self.ts_ty(&self.ty_of(&fd.name.text))
                ));
            }
            o.push_str("}\n\n");
        }

        if outs.len() > 1 {
            o.push_str("export interface Output {\n");
            for od in outs {
                o.push_str(&format!(
                    "  {}: {};\n",
                    pub_name(&od.name),
                    self.ts_ty(&self.ty_of(&od.name.text))
                ));
            }
            o.push_str("}\n\n");
        }

        // The public function keeps the plain signature; the branches live in its traced
        // twin, which also returns the rows that matched (§15.33).
        let mut args: Vec<String> = self.f.inputs.iter().map(|i| pub_name(&i.name)).collect();
        if let Some(el) = &self.f.elements {
            args.push(pub_name(&el.name));
        }
        let traced = format!("{fname}_traced");
        o.push_str(&format!("/** {} */\n", plain_doc(&self.f.name.text, &self.f.version, &traced)));
        o.push_str(&format!(
            "export function {fname}({}): {ret} {{\n  return {traced}({})[0];\n}}\n\n",
            params.join(", "),
            args.join(", ")
        ));
        o.push_str(&format!("/** {} */\n", traced_doc(&self.f.name.text, &self.f.version)));
        o.push_str(&format!("export function {traced}({}): [{ret}, Fired[]] {{\n", params.join(", ")));

        let local = |n: &str| -> String { self.ident(n) };
        for i in &self.f.inputs {
            let v = pub_name(&i.name);
            let ty = self.ty_of(&i.name.text);
            match &ty {
                Ty::Enum(n) => {
                    let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                    o.push_str(&format!(
                        "  if (!Object.values({cls}).includes({v})) {{\n    \
                         throw new RuleInputError(`{}`, {v});\n  }}\n",
                        tr!(
                            "{} が列挙 {cls} の値ではありません",
                            "{} is not a value of enum {cls}",
                            i.name.text
                        )
                    ));
                }
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                    // The type says bigint, but a JavaScript caller can pass anything, and a
                    // number would fail somewhere inside the arithmetic instead of here
                    // (§15.43).
                    o.push_str(&format!(
                        "  if (typeof {v} !== \"bigint\") {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                        tr!("{} が整数ではありません", "{} is not an integer", i.name.text)
                    ));
                    if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) {
                        let sc = self.c.wire_scale(&i.name.text);
                        o.push_str(&format!(
                            "  if ({v} < {}n || {v} > {}n) {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                            crate::types::wire_int(lo, sc),
                            crate::types::wire_int(hi, sc),
                            tr!("{} が範囲の外です", "{} is out of range", i.name.text)
                        ));
                    }
                }
                _ => {}
            }
        }

        o.push_str(&self.constraint_guards(&local, Lang::Ts, "  ", |m| {
            format!("    throw new RuleInputError(`{m}`);\n")
        }));

        let trace = self.temp("trace");
        o.push_str(&format!("  const {trace}: Fired[] = [];\n"));

        match &self.f.fold {
            Some(fold) => o.push_str(&self.ts_walk(fold, &trace)),
            None if self.counts().is_empty() => o.push_str(&self.ts_items(&local, &trace, Phase::All)),
            None => o.push_str(&self.ts_count_walk(&local, &trace)),
        }

        let cast = |ty: &Ty, body: String| -> String {
            match ty {
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => format!("({body}) as {}", self.ts_ty(ty)),
                _ => body,
            }
        };
        // Every output takes the same three steps: its source — the `result` expression for
        // the first output, the binding of its own name otherwise — brought to the wire
        // scale, then its declared rounding applied exactly once (§15.16).
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let ty = self.ty_of(out_name);
            let body = match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os {
                        let raw = self.temp(&raw_base(oi));
                        o.push_str(&format!(
                            "  const {raw} = {}; // {}\n",
                            ts_expr(unparen(&res.text)),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("_round{}({raw}, {grid_i}n) / {}n", pascal(mode_fn(m)), res.scale / os)
                    } else {
                        format!("_round{}({}, {grid_i}n)", pascal(mode_fn(m)), ts_expr(&res.text))
                    }
                }
                None => ts_expr(&res.text),
            };
            finals.push(cast(&ty, body));
        }
        if outs.len() == 1 {
            o.push_str(&format!("  return [{}, {trace}];\n}}\n", finals[0]));
        } else {
            let fields: Vec<String> = outs
                .iter()
                .zip(&finals)
                .map(|(od, body)| format!("{}: {body}", pub_name(&od.name)))
                .collect();
            o.push_str(&format!("  return [{{ {} }}, {trace}];\n}}\n", fields.join(", ")));
        }
        o
    }

    fn ts_table(&self, t: &Table, local: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = format!("  // {}\n", self.table_head(t));
        // `let` up front: a binding made inside a branch does not leave it.
        for oc in &t.outputs {
            let ty = self.ty_of(&oc.name.text);
            let decl = match ty {
                Ty::Bool => "boolean".to_string(),
                Ty::Str => "string".to_string(),
                Ty::Enum(_) => self.ts_ty(&ty),
                _ => "bigint".to_string(),
            };
            o.push_str(&format!("  let {}: {decl};\n", self.ident(&oc.name.text)));
        }
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.ts_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "true".into() } else { conds.join(" && ") };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            let head = if ri == 0 { "  if" } else { " else if" };
            let line = self.row_head(t, ri, &cells.join(" | "));
            if ri == 0 {
                o.push_str(&format!("{head} ({cond}) {{ // {line}\n"));
            } else {
                o.push_str(&format!("  }}{head} ({cond}) {{ // {line}\n"));
            }
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let oty = self.ty_of(&oc.name.text);
                        format!("{}n", self.int_lit(n, &oty, self.scale(&oc.name.text)))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.ts_value(w),
                        Lit::Date(y, m, d) => format!("{}n", crate::types::date_ord(*y, *m, *d).num),
                        Lit::Str(x) => str_lit(x),
                        _ => "0n".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "true".into()
                        } else if w == crate::kw::FALSE {
                            "false".into()
                        } else if self.value_names.contains_key(w) {
                            self.ts_value(w)
                        } else {
                            ts_expr(&self.rescaled(w, &oc.name.text, local(w)))
                        }
                    }
                    None => "0n".into(),
                };
                o.push_str(&format!("    {} = {v};\n", self.ident(&oc.name.text)));
            }
            let (tn, rn, lbl) = self.fired_of(t, ri);
            if lbl.is_empty() {
                o.push_str(&format!("    {trace}.push({{ table: {tn:?}, row: {rn} }});\n"));
            } else {
                o.push_str(&format!("    {trace}.push({{ table: {tn:?}, row: {rn}, label: {lbl:?} }});\n"));
            }
        }
        o.push_str(&format!(
            "  }} else {{\n    throw new Error(\"{}\");\n  }}\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        o.push_str(&self.guards(t, local, Lang::Ts, "  ", |name, i, j| {
            format!(
                "      throw new RuleContradictionError(\"{}\");\n",
                tr!("表 {name}: {i} と {j} が同時に当てはまりました", "table {name}: {i} and {j} matched at the same time")
            )
        }));
        o
    }

    pub fn ts_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let d = runner_local("d", &alias);
        let r = runner_local("r", &alias);
        let line = runner_local("line", &alias);
        let lines = runner_local("lines", &alias);
        let trace = runner_local("trace", &alias);
        let a = runner_local("a", &alias);
        let mut args: Vec<String> = Vec::new();
        let mut imports: Vec<String> = vec![format!("{alias}_traced"), format!("{alias}_record")];
        // A brand is a type, and node's type stripping can only erase a whole `import type`
        // statement — a type name mixed into a value import is a syntax error there.
        let mut type_imports: Vec<String> = Vec::new();
        fn ts_read(
            g: &Gen,
            ty: &Ty,
            src: String,
            imports: &mut Vec<String>,
            type_imports: &mut Vec<String>,
        ) -> String {
            match ty {
                Ty::Enum(n) => {
                    let cls = g.enum_names.get(n).cloned().unwrap_or_default();
                    if !imports.contains(&format!("parse{cls}")) {
                        imports.push(format!("parse{cls}"));
                    }
                    format!("parse{cls}(String({src}))")
                }
                Ty::Bool => format!("{src} === true"),
                Ty::Date => format!("_ord(String({src}))"),
                Ty::Str => format!("String({src})"),
                Ty::Number => format!("BigInt({src} as number)"),
                // The absent value is `null` on the wire (§10.2). Its brand is a compound
                // type (`Kind | null`), which was being pushed into an `import type {…}` as
                // if it were a name — a syntax error before anything ran (§15.88).
                Ty::Opt(inner) => {
                    let one = ts_read(g, inner, src.clone(), imports, type_imports);
                    format!("({src} === null ? null : {one})")
                }
                _ => {
                    let brand = g.ts_ty(ty);
                    if !type_imports.contains(&brand) {
                        type_imports.push(brand.clone());
                    }
                    format!("BigInt({src} as number) as {brand}")
                }
            }
        }
        let read = |ty: &Ty, src: String, imports: &mut Vec<String>, type_imports: &mut Vec<String>| -> String {
            ts_read(self, ty, src, imports, type_imports)
        };
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            let jp = &i.name.text;
            let src = format!("{d}[{jp:?}]");
            args.push(read(&ty, src, &mut imports, &mut type_imports));
        }
        // The sequence a walk reads, one element at a time (§15.56).
        let mut prelude = String::new();
        if let Some(el) = &self.f.elements {
            let jp = &el.name.text;
            let e = runner_local("e", &alias);
            let fields: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    let src = format!("({e} as Record<string, unknown>)[{:?}]", fd.name.text);
                    format!("{}: {}", pub_name(&fd.name), read(&ty, src, &mut imports, &mut type_imports))
                })
                .collect();
            let rows = runner_local("rows", &alias);
            prelude = format!(
                "  const {rows} = ({d}[{jp:?}] as unknown[]).map(({e}) => ({{ {} }}));\n",
                fields.join(", ")
            );
            args.push(rows);
        }
        format!(
            "// Code generated by rulec {}. DO NOT EDIT.\n\
             import {{ readFileSync }} from \"node:fs\";\n\
             import {{ {} }} from \"./{alias}.ts\";\n{}\n\
             function _ord(s: string): bigint {{\n  \
                 const [y, m, d] = s.split(\"-\").map(Number);\n  \
                 return BigInt(Math.round(Date.UTC(y, m - 1, d) / 86400000));\n}}\n\n\
             const {lines} = readFileSync(0, \"utf8\").split(\"\\n\");\n\
             for (const {line} of {lines}) {{\n  \
                 if ({line}.trim() === \"\") {{\n    continue;\n  }}\n  \
                 const {d} = JSON.parse({line}).in as Record<string, unknown>;\n\
                 {}  \
                 const {a} = [{}] as const;\n  \
                 const [{r}, {trace}] = {alias}_traced(...{a});\n  \
                 console.log({alias}_record(...{a}, {r}, {trace}));\n}}\n",
            env!("CARGO_PKG_VERSION"),
            imports.join(", "),
            if type_imports.is_empty() {
                String::new()
            } else {
                format!("import type {{ {} }} from \"./{alias}.ts\";\n", type_imports.join(", "))
            },
            prelude,
            args.join(", ")
        )
    }
}


// ---------------------------------------------------------------------------
// Rust (§8.3)
// ---------------------------------------------------------------------------

/// Turn the shared expression text into Rust. The same post-processing shape as `go_expr`
/// and `ts_expr`. Integer division truncates toward zero here, exactly as it does in Go and
/// on a JavaScript bigint, so the rounding helpers are the Go ones transliterated.
fn rs_expr(s: &str) -> String {
    s.replace(" // ", " / ")
        .replace("True", "true")
        .replace("False", "false")
        .replace("_round_down(", "round_down(")
        .replace("_round_up(", "round_up(")
        .replace("_round_half_down(", "round_half_down(")
        .replace("_round_half(", "round_half(")
        .replace("_round_bankers(", "round_bankers(")
        .replace("_min(", "min_i64(")
        .replace("_max(", "max_i64(")
}

/// The error type and the five modes of §7.3.
fn round_rs() -> String {
    format!(
        r#"
/// {err}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {{
    /// {input}
    Input {{ what: &'static str, value: Option<i64> }},
    /// {contra}
    Contradiction {{ what: &'static str }},
}}

impl std::fmt::Display for RuleError {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        match self {{
            RuleError::Input {{ what, value: Some(v) }} => write!(f, "{{what}}: {{v}}"),
            RuleError::Input {{ what, .. }} | RuleError::Contradiction {{ what }} => f.write_str(what),
        }}
    }}
}}

impl std::error::Error for RuleError {{}}

pub(crate) fn min_i64(a: i64, b: i64) -> i64 {{
    if a < b {{ a }} else {{ b }}
}}

pub(crate) fn max_i64(a: i64, b: i64) -> i64 {{
    if a > b {{ a }} else {{ b }}
}}

/// {down}
pub(crate) fn round_down(x: i64, g: i64) -> i64 {{
    let v = x.abs() / g * g;
    if x < 0 {{ -v }} else {{ v }}
}}

/// {up}
pub(crate) fn round_up(x: i64, g: i64) -> i64 {{
    let a = x.abs();
    let v = if a % g == 0 {{ a / g * g }} else {{ (a / g + 1) * g }};
    if x < 0 {{ -v }} else {{ v }}
}}

/// {half}
pub(crate) fn round_half(x: i64, g: i64) -> i64 {{
    let a = x.abs();
    let v = if 2 * (a % g) >= g {{ (a / g + 1) * g }} else {{ a / g * g }};
    if x < 0 {{ -v }} else {{ v }}
}}

/// {half_down}
pub(crate) fn round_half_down(x: i64, g: i64) -> i64 {{
    let a = x.abs();
    let v = if 2 * (a % g) > g {{ (a / g + 1) * g }} else {{ a / g * g }};
    if x < 0 {{ -v }} else {{ v }}
}}

/// {bankers}
pub(crate) fn round_bankers(x: i64, g: i64) -> i64 {{
    let a = x.abs();
    let (mut q, r) = (a / g, a % g);
    if 2 * r > g || (2 * r == g && q % 2 == 1) {{
        q += 1;
    }}
    let v = q * g;
    if x < 0 {{ -v }} else {{ v }}
}}
"#,
        err = tr!("この規則が返しうる誤り。", "Everything this rule can go wrong with."),
        input = tr!("宣言した範囲の外。呼び出し側の契約違反。", "Outside the declared input domain: a contract violation by the caller."),
        contra = tr!("規則そのものの矛盾。呼び出し側の誤りではない。", "A contradiction in the rule itself, not a mistake by the caller."),
        down = tr!("0 へ寄せる。-4.8円 → -4円。", "Toward zero: -4.8 yen → -4 yen."),
        up = tr!("0 から遠ざける。-4.2円 → -5円。", "Away from zero: -4.2 yen → -5 yen."),
        half = tr!("半分ちょうどは 0 から遠ざける。", "An exact half goes away from zero."),
        half_down = tr!("半分ちょうどは 0 へ寄せる。", "An exact half goes toward zero."),
        bankers = tr!("半分ちょうどは偶数へ。", "An exact half goes to the even neighbor."),
    )
}

impl<'a> Gen<'a> {
    /// The Rust type for a value. A unit is a newtype over `i64`: the compiler refuses
    /// `YenExclTax` where `YenInclTax` was meant, and it costs nothing at run time.
    fn rs_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => self.enum_names.get(n).cloned().unwrap_or_else(|| "String".into()),
            Ty::Bool => "bool".into(),
            Ty::Str => "String".into(),
            Ty::Number | Ty::Date => "i64".into(),
            Ty::Opt(t) => format!("Option<{}>", self.rs_ty(t)),
            _ => brand_of(ty),
        }
    }

    fn rs_value(&self, v: &str) -> String {
        match self.value_names.get(v) {
            Some((ty, alias)) => format!("{ty}::{}", pascal(alias)),
            None => format!("{v:?}"),
        }
    }

    /// Render a cell as a Rust condition. A don't-care yields None (no condition).
    /// `Copy` for a struct of these fields, or just `Clone`. A `String` is not `Copy`, so a
    /// rule with two outputs one of which is a string did not compile at all — and no rule
    /// with a string output was ever generated until §15.88.
    fn rs_derive(&self, mut names: impl Iterator<Item = String>) -> &'static str {
        let owned = names.any(|n| matches!(self.ty_of(&n), Ty::Str | Ty::Opt(_)));
        if owned {
            "#[derive(Clone, PartialEq, Eq, Debug)]"
        } else {
            "#[derive(Clone, Copy, PartialEq, Eq, Debug)]"
        }
    }

    fn rs_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        // An optional column is an `Option<T>`, and every test but `none` is about the value
        // inside it. `memo == Kind::A` does not compile against `Option<Kind>`, so the module
        // itself would not build (DESIGN §15.88). The tests below are written against a bound
        // `__v` and wrapped in `matches!`; a negated cell is the negation of the positive
        // one, because `not 甲` holds of the absent value — `T?` is the enum with one more
        // value, and that is how the checker reads it too.
        let optional = matches!(ty, Ty::Opt(_));
        let raw = var.to_string();
        let bound = if optional { "__v".to_string() } else { raw.clone() };
        let var = bound.as_str();
        let wrap = |c: String, neg: bool| -> String {
            if !optional {
                return c;
            }
            let m = format!("matches!({raw}, Some(__v) if {c})");
            if neg { format!("!{m}") } else { m }
        };
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.rs_value(w),
                Lit::Num(n) => self.int_lit(n, inner, col_scale),
                Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => format!("{s:?}"),
            }
        };
        let any_of = |ls: &Vec<Lit>, neg: bool| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.rs_value(m)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            let (op, join) = if neg { ("!=", " && ") } else { ("==", " || ") };
            format!("({})", out.iter().map(|v| format!("{var} {op} {v}")).collect::<Vec<_>>().join(join))
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => return Some(format!("{raw}.is_none()")),
            Cell::Prefix(ps) => ps
                .iter()
                .map(|p| format!("{var}.starts_with({})", format!("{p:?}")))
                .collect::<Vec<_>>()
                .join(" || "),
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                wrap(format!("is_{}({var})", self.ident(w)), false)
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => wrap(var.to_string(), false),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => wrap(format!("!{var}"), false),
            Cell::Lit(l) => wrap(format!("{var} == {}", lit(l)), false),
            Cell::Set(ls) => wrap(any_of(ls, false), false),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                if optional {
                    wrap(format!("is_{}({var})", self.ident(w)), true)
                } else {
                    format!("!is_{}({var})", self.ident(w))
                }
            }
            Cell::Not(ls) => {
                if optional { wrap(any_of(ls, false), true) } else { any_of(ls, true) }
            }
            Cell::Cmp(cs) => cs
                .iter()
                .map(|(o, l)| {
                    let op = match o {
                        CmpOp::Le => "<=",
                        CmpOp::Ge => ">=",
                        CmpOp::Lt => "<",
                        CmpOp::Gt => ">",
                    };
                    format!("{var} {op} {}", lit(l))
                })
                .collect::<Vec<_>>()
                .join(" && "),
        })
    }

    pub fn rust(&self) -> String {
        let mut o = self.header("//");
        o.push_str(
            "\n#![allow(non_snake_case, uncommon_codepoints, unused_parens)]\n\n",
        );

        // Brands: a newtype over i64, which is what the overflow proof is stated in.
        let mut brands: BTreeMap<String, String> = BTreeMap::new();
        for v in self.f.inputs.iter().map(|i| &i.name.text).chain(self.f.outputs.iter().map(|o| &o.name.text)) {
            let ty = self.ty_of(v);
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                brands.insert(brand_of(&ty), format!("{ty}"));
            }
        }
        for (b, doc) in &brands {
            o.push_str(&format!(
                "/// {doc}\n#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]\npub struct {b}(pub i64);\n\n"
            ));
        }
        o.push_str(&format!(
            "/// {}\n#[derive(Clone, Copy, PartialEq, Eq, Debug)]\npub struct Fired {{\n    pub table: &'static str,\n    pub row: u32,\n    pub label: &'static str,\n}}\n\n",
            fired_doc()
        ));

        // Enums. Each carries its Japanese name, which is what the wire format uses (§10).
        let mut emitted: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if emitted.contains(ascii) {
                continue;
            }
            emitted.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            let members: Vec<String> = vals
                .iter()
                .map(|v| self.value_names.get(v).map(|(_, a)| pascal(a)).unwrap_or_else(|| v.clone()))
                .collect();
            o.push_str(&format!(
                "#[derive(Clone, Copy, PartialEq, Eq, Debug)]\npub enum {ascii} {{\n{}}}\n\n",
                members.iter().map(|m| format!("    {m},\n")).collect::<String>()
            ));
            o.push_str(&format!("impl {ascii} {{\n    pub fn as_str(self) -> &'static str {{\n        match self {{\n"));
            for (m, v) in members.iter().zip(vals) {
                o.push_str(&format!("            {ascii}::{m} => {v:?},\n"));
            }
            o.push_str("        }\n    }\n\n");
            o.push_str(&format!("    pub fn parse(s: &str) -> Option<{ascii}> {{\n        match s {{\n"));
            for (m, v) in members.iter().zip(vals) {
                o.push_str(&format!("            {v:?} => Some({ascii}::{m}),\n"));
            }
            o.push_str("            _ => None,\n        }\n    }\n}\n\n");
        }

        o.push_str(round_rs().trim_start_matches('\n'));
        o.push('\n');

        for g in &self.f.groups {
            let ty = self
                .c
                .groups
                .get(&g.name.text)
                .and_then(|(owner, _)| self.enum_names.get(owner).cloned())
                .unwrap_or_else(|| "i64".into());
            let ms: Vec<String> = g.members.iter().map(|m| self.rs_value(&m.text)).collect();
            o.push_str(&format!(
                "pub(crate) fn is_{}(v: {ty}) -> bool {{\n    matches!(v, {})\n}}\n\n",
                self.ident(&g.name.text),
                ms.join(" | ")
            ));
        }

        o.push_str(&self.rs_fn());
        o.push('\n');
        o.push_str(&self.rs_record());
        o
    }

    /// The rule's items as Rust, at the base indentation (§15.56).
    fn rs_items(&self, local: &dyn Fn(&str) -> String, trace: &str, phase: Phase) -> String {
        let mut o = String::new();
        for it in &self.f.items {
            if !self.in_phase(it, phase) {
                continue;
            }
            match it {
                Item::Derived(d) => o.push_str(&format!(
                    "    let {} = {}; // {}\n",
                    self.ident(&d.name.text),
                    rs_expr(unparen(&self.expr(&d.expr, local).text)),
                    tr!("導出", "derived value")
                )),
                Item::Define(d) => o.push_str(&format!(
                    "    let {} = {}; // {}\n",
                    self.ident(&d.name.text),
                    rs_expr(unparen(&self.expr(&d.expr, local).text)),
                    tr!("定義", "definition")
                )),
                Item::Agg(_) => {}
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        o.push_str(&self.rs_table(t, local, trace));
                    }
                }
            }
        }
        o
    }

    /// The walk, in Rust (§15.56). Same shape as the other backends: accumulators, one pass
    /// per element, and the answer bound to the output's own name.
    fn rs_walk(&self, fold: &crate::ast::FoldDecl, trace: &str) -> String {
        let el = self.f.elements.as_ref().expect("a fold has elements");
        let seq = pub_name(&el.name);
        let fields: BTreeMap<String, String> =
            el.fields.iter().map(|fd| (fd.name.text.clone(), pub_name(&fd.name))).collect();
        let (answer, stopped, taken, kept, best) = self.fold_locals();
        let e = self.temp("elem");
        let held = self.temp("held");
        // A unit is a newtype here, so a field read in an expression is unwrapped exactly as
        // an input is (§8.1).
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => {
                    let v = format!("{e}.{f}");
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                        format!("{v}.0")
                    } else {
                        v
                    }
                }
                None if n == crate::kw::HELD => held.clone(),
                None => self.ident(n),
            }
        };
        let out_name = self.f.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
        let out_ty = self.ty_of(&out_name);
        let acc_ty = if out_ty.is_numeric() { "i64".to_string() } else { self.rs_ty(&out_ty) };
        let text = |x: &Option<Expr>| -> String {
            x.as_ref().map(|x| rs_expr(unparen(&self.expr(x, &local).text)).to_string()).unwrap_or_else(|| "0".to_string())
        };

        let mut o = String::new();
        o.push_str(&format!("    let mut {answer}: {acc_ty} = {}; // {}\n", text(&fold.empty), tr!("要素ゼロ件の答え", "the answer for no elements")));
        o.push_str(&format!("    let mut {stopped} = false;\n"));
        o.push_str(&format!("    let mut {taken}: Option<{acc_ty}> = None;\n"));
        o.push_str(&format!("    let mut {kept}: Option<{acc_ty}> = None;\n"));
        o.push_str(&format!("    let mut {best}: Option<i64> = None;\n"));
        o.push_str(&format!("    for {e} in {seq} {{\n"));

        let mut body = self.rs_element_guards(&local);
        body.push_str(&self.rs_items(&local, trace, Phase::All));
        let v = local(&fold.verdict);
        for (name, arm, _) in &fold.arms {
            let cls = self.rs_ty(&self.ty_of(&fold.verdict));
            let member = self.value_names.get(&name.text).map(|(_, a)| a.clone()).unwrap_or_else(|| name.text.clone());
            body.push_str(&format!("    if {v} == {cls}::{member} {{ // {}\n", name.text));
            match arm {
                Arm::Next => body.push_str("        // next\n"),
                Arm::Stop(None) => body.push_str("        break;\n"),
                Arm::Stop(Some(x)) => {
                    body.push_str(&format!("        {answer} = {};\n", rs_expr(unparen(&self.expr(x, &local).text))));
                    body.push_str(&format!("        {stopped} = true;\n        break;\n"));
                }
                Arm::Take { expr, unique } => {
                    if *unique {
                        body.push_str(&format!("        if {taken}.is_some() {{\n"));
                        body.push_str(&format!(
                            "            return Err(RuleError::Contradiction {{ what: \"{}\" }});\n        }}\n",
                            tr!("畳み込み {}: take_unique に二件当たりました", "fold {}: two elements matched a take_unique", fold.verdict)
                        ));
                        body.push_str(&format!("        {taken} = Some({});\n", rs_expr(unparen(&self.expr(expr, &local).text))));
                    } else {
                        body.push_str(&format!("        if {taken}.is_none() {{\n"));
                        body.push_str(&format!("            {taken} = Some({});\n        }}\n", rs_expr(unparen(&self.expr(expr, &local).text))));
                    }
                }
                Arm::KeepMax { expr, key } => {
                    let k = self.temp("key");
                    body.push_str(&format!("        let {k} = {};\n", rs_expr(unparen(&self.expr(key, &local).text))));
                    body.push_str(&format!("        if {best}.is_none() || {k} > {best}.unwrap() {{\n"));
                    body.push_str(&format!("            {best} = Some({k});\n"));
                    body.push_str(&format!("            {kept} = Some({});\n        }}\n", rs_expr(unparen(&self.expr(expr, &local).text))));
                }
            }
            body.push_str("    }\n");
        }
        o.push_str(&Self::indent_block(&body, "    "));
        o.push_str("    }\n");

        o.push_str(&format!("    if !{seq}.is_empty() && !{stopped} {{\n"));
        o.push_str(&format!(
            "        let {held} = {taken}.or({kept}).unwrap_or({});\n",
            text(&fold.empty)
        ));
        o.push_str(&format!("        {answer} = {};\n    }}\n", text(&fold.exhausted)));
        o.push_str(&format!("    let {} = {answer};\n", self.ident(&out_name)));
        o
    }

    /// The entry guards of one element, inside the walk.
    /// The counting walk in Rust (§15.58).
    fn rs_count_walk(&self, outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let el = self.f.elements.as_ref().expect("a count has elements");
        let seq = pub_name(&el.name);
        let fields: BTreeMap<String, String> =
            el.fields.iter().map(|fd| (fd.name.text.clone(), pub_name(&fd.name))).collect();
        let e = self.temp("elem");
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => {
                    let v = format!("{e}.{f}");
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                        format!("{v}.0")
                    } else {
                        v
                    }
                }
                None => self.ident(n),
            }
        };
        let mut o = String::new();
        if let Some(cap) = self.count_cap() {
            o.push_str(&format!(
                "    if {seq}.len() > {cap} {{\n        return Err(RuleError::Input {{ what: \"{}\", value: Some({seq}.len() as i64) }});\n    }}\n",
                tr!("{} の要素が多すぎます（上限 {cap}）", "{} has too many elements (at most {cap})", el.name.text)
            ));
        }
        for d in self.counts() {
            o.push_str(&format!("    let mut {}: i64 = 0; // {}\n", self.ident(&d.name.text), tr!("数え上げ", "count")));
        }
        o.push_str(&format!("    for {e} in {seq} {{\n"));
        let mut body = self.rs_element_guards(&local);
        body.push_str(&self.rs_items(&local, trace, Phase::Walk));
        for d in self.counts() {
            let v = local(&d.column.text);
            if d.kind == crate::ast::AggKind::Sum {
                body.push_str(&format!("    {} += {v}; // {}\n", self.ident(&d.name.text), d.name.text));
                continue;
            }
            let test = match self.count_member(d) {
                Some(w) => {
                    let cls = self.rs_ty(&self.ty_of(&d.column.text));
                    let member = self.value_names.get(&w.text).map(|(_, a)| a.clone()).unwrap_or_else(|| w.text.clone());
                    format!("{v} == {cls}::{member}")
                }
                None if self.count_negated(d) => format!("!{v}"),
                None => v,
            };
            body.push_str(&format!("    if {test} {{ // {}\n        {} += 1;\n    }}\n", d.name.text, self.ident(&d.name.text)));
        }
        o.push_str(&Self::indent_block(&body, "    "));
        o.push_str("    }\n");
        for (d, cap) in self.sum_caps() {
            let n = self.ident(&d.name.text);
            o.push_str(&format!(
                "    if {n} > {cap} {{\n        return Err(RuleError::Input {{ what: \"{}\", value: Some({n}) }});\n    }}\n",
                tr!("{} が範囲の外です", "{} is out of range", d.name.text)
            ));
        }
        o.push_str(&self.rs_items(outer, trace, Phase::Main));
        o
    }

    fn rs_element_guards(&self, local: &dyn Fn(&str) -> String) -> String {
        let mut o = String::new();
        for i in self.element_fields() {
            let v = local(&i.name.text);
            let ty = self.ty_of(&i.name.text);
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) {
                    let sc = self.c.wire_scale(&i.name.text);
                    o.push_str(&format!(
                        "    if {v} < {} || {v} > {} {{\n        return Err(RuleError::Input {{ what: \"{}\", value: Some({v}) }});\n    }}\n",
                        crate::types::wire_int(lo, sc),
                        crate::types::wire_int(hi, sc),
                        tr!("{} が範囲の外です", "{} is out of range", i.name.text)
                    ));
                }
            }
        }
        o
    }

    fn rs_fn(&self) -> String {
        let fname = pub_name(&self.f.name);
        let outs = &self.f.outputs;
        let mut o = String::new();

        // One element is a row of inputs, so it is a struct like `Output`.
        if let Some(el) = &self.f.elements {
            o.push_str(&format!("/// {}\n", tr!("{} の一件", "one of {}", el.name.text)));
            o.push_str(&format!(
                "{}\npub struct Element {{\n",
                self.rs_derive(el.fields.iter().map(|fd| fd.name.text.clone()))
            ));
            for fd in &el.fields {
                o.push_str(&format!(
                    "    pub {}: {},\n",
                    pub_name(&fd.name),
                    self.rs_ty(&self.ty_of(&fd.name.text))
                ));
            }
            o.push_str("}\n\n");
        }

        if outs.len() > 1 {
            o.push_str(&format!(
                "{}\npub struct Output {{\n",
                self.rs_derive(outs.iter().map(|od| od.name.text.clone()))
            ));
            for od in outs {
                o.push_str(&format!(
                    "    pub {}: {},\n",
                    pub_name(&od.name),
                    self.rs_ty(&self.ty_of(&od.name.text))
                ));
            }
            o.push_str("}\n\n");
        }

        let ret = if outs.len() == 1 {
            self.rs_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.rs_ty(&self.ty_of(&i.name.text))))
            .collect();
        if let Some(el) = &self.f.elements {
            params.push(format!("{}: &[Element]", pub_name(&el.name)));
        }

        // The public function keeps the plain signature; the branches live in its traced
        // twin, which also returns the rows that matched (§15.33).
        let mut args: Vec<String> = self.f.inputs.iter().map(|i| pub_name(&i.name)).collect();
        if let Some(el) = &self.f.elements {
            args.push(pub_name(&el.name));
        }
        let traced = format!("{fname}_traced");
        o.push_str(&format!("/// {}\n", plain_doc(&self.f.name.text, &self.f.version, &format!("`{traced}`"))));
        o.push_str(&format!(
            "pub fn {fname}({}) -> Result<{ret}, RuleError> {{\n    {traced}({}).map(|(out, _)| out)\n}}\n\n",
            params.join(", "),
            args.join(", ")
        ));
        o.push_str(&format!("/// {}\n", traced_doc(&self.f.name.text, &self.f.version)));
        o.push_str(&format!(
            "pub fn {traced}({}) -> Result<({ret}, Vec<Fired>), RuleError> {{\n",
            params.join(", ")
        ));

        // A branded input is an i64 inside; unwrap it once, where it is read.
        let local = |n: &str| -> String {
            match self.f.inputs.iter().find(|i| i.name.text == n) {
                Some(i) => {
                    let v = pub_name(&i.name);
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                        format!("{v}.0")
                    } else {
                        v
                    }
                }
                None => self.ident(n),
            }
        };
        // Entry guards. An enum needs none: in Rust a value of an enum type is one of its
        // variants by construction, so the check the other three languages have to make at
        // run time is already made by the compiler.
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                continue;
            }
            let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) else {
                continue;
            };
            let sc = self.c.wire_scale(&i.name.text);
            let v = local(&i.name.text);
            // Rust's inline format arguments take a name, not a field access, so the value
            // goes in as a positional argument.
            o.push_str(&format!(
                "    if {v} < {} || {v} > {} {{\n        return Err(RuleError::Input {{ what: \"{}\", value: Some({v}) }});\n    }}\n",
                crate::types::wire_int(lo, sc),
                crate::types::wire_int(hi, sc),
                tr!("{} が範囲の外です", "{} is out of range", i.name.text)
            ));
        }

        o.push_str(&self.constraint_guards(&local, Lang::Rs, "    ", |m| {
            format!("        return Err(RuleError::Input {{ what: \"{m}\", value: None }});\n")
        }));

        let trace = self.temp("trace");
        let has_table = self.f.items.iter().any(|i| matches!(i, Item::Table(_)));
        o.push_str(&format!("    let {}{trace}: Vec<Fired> = Vec::new();\n", if has_table { "mut " } else { "" }));

        match &self.f.fold {
            Some(fold) => o.push_str(&self.rs_walk(fold, &trace)),
            None if self.counts().is_empty() => o.push_str(&self.rs_items(&local, &trace, Phase::All)),
            None => o.push_str(&self.rs_count_walk(&local, &trace)),
        }

        let wrap = |ty: &Ty, body: String| -> String {
            match ty {
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => format!("{}({body})", self.rs_ty(ty)),
                _ => body,
            }
        };
        // Every output takes the same three steps: its source — the `result` expression for
        // the first output, the binding of its own name otherwise — brought to the wire
        // scale, then its declared rounding applied exactly once (§15.16).
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let ty = self.ty_of(out_name);
            let body = match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os {
                        let raw = self.temp(&raw_base(oi));
                        o.push_str(&format!(
                            "    let {raw} = {}; // {}\n",
                            rs_expr(unparen(&res.text)),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("round_{}({raw}, {grid_i}) / {}", mode_fn(m), res.scale / os)
                    } else {
                        format!("round_{}({}, {grid_i})", mode_fn(m), rs_expr(&res.text))
                    }
                }
                None => rs_expr(&res.text),
            };
            finals.push(wrap(&ty, body));
        }
        if outs.len() == 1 {
            o.push_str(&format!("    Ok(({}, {trace}))\n}}\n", finals[0]));
        } else {
            let fields: Vec<String> = outs
                .iter()
                .zip(&finals)
                .map(|(od, body)| format!("{}: {body}", pub_name(&od.name)))
                .collect();
            o.push_str(&format!("    Ok((Output {{ {} }}, {trace}))\n}}\n", fields.join(", ")));
        }
        o
    }

    fn rs_table(&self, t: &Table, local: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = format!("    // {}\n", self.table_head(t));
        for oc in &t.outputs {
            let ty = self.ty_of(&oc.name.text);
            let decl = match ty {
                Ty::Bool => "bool".to_string(),
                Ty::Str => "String".to_string(),
                Ty::Enum(_) => self.rs_ty(&ty),
                _ => "i64".to_string(),
            };
            o.push_str(&format!("    let {}: {decl};\n", self.ident(&oc.name.text)));
        }
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.rs_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "true".into() } else { conds.join(" && ") };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            let line = self.row_head(t, ri, &cells.join(" | "));
            let head = if ri == 0 { "    if" } else { " else if" };
            if ri == 0 {
                o.push_str(&format!("{head} {cond} {{ // {line}\n"));
            } else {
                o.push_str(&format!("    }}{head} {cond} {{ // {line}\n"));
            }
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let oty = self.ty_of(&oc.name.text);
                        self.int_lit(n, &oty, self.scale(&oc.name.text))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.rs_value(w),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        // Rust's string output is a `String`, so the literal is owned here
                        // rather than at every use.
                        Lit::Str(x) => format!("{}.to_string()", str_lit(x)),
                        _ => "0".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "true".into()
                        } else if w == crate::kw::FALSE {
                            "false".into()
                        } else if self.value_names.contains_key(w) {
                            self.rs_value(w)
                        } else {
                            rs_expr(&self.rescaled(w, &oc.name.text, local(w)))
                        }
                    }
                    None => "0".into(),
                };
                o.push_str(&format!("        {} = {v};\n", self.ident(&oc.name.text)));
            }
            let (tn, rn, lbl) = self.fired_of(t, ri);
            o.push_str(&format!("        {trace}.push(Fired {{ table: {tn:?}, row: {rn}, label: {lbl:?} }});\n"));
        }
        o.push_str(&format!(
            "    }} else {{\n        unreachable!(\"{}\");\n    }}\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        o.push_str(&self.guards(t, local, Lang::Rs, "    ", |name, i, j| {
            format!(
                "        return Err(RuleError::Contradiction {{ what: \"{}\" }});\n",
                tr!("表 {name}: {i} と {j} が同時に当てはまりました", "table {name}: {i} and {j} matched at the same time")
            )
        }));
        o
    }

    /// The runner. Rust has no JSON in its standard library, and the generated code takes no
    /// dependencies, so the reader below is written out here: the wire format is one flat
    /// object of numbers, strings and booleans (§10.2), which is small enough to scan.
    /// The proof harnesses, for the Kani Rust Verifier (§15.95). A file of its own: a person
    /// reading the rule should not have to read the scaffolding, and `rustc` never compiles
    /// it. `kani <alias>_proof.rs` — or `rulec test`, when kani is on PATH.
    ///
    /// Three of the five things `check` proves are already written into the generated Rust
    /// as assertions, so the harness only has to reach them: the `unreachable!` that closes
    /// every table is completeness, the guard that returns `Contradiction` is the W114 pair,
    /// and every arithmetic operation is the int64 claim, which Kani checks by default. The
    /// fourth, the overlap, is *not* in the artifact — the if/else chain has already settled
    /// the priority — so the rows are counted here instead.
    pub fn rs_proof(&self) -> String {
        let alias = pub_name(&self.f.name);
        let mut o = self.header("//");
        o.push_str(&format!(
            "//\n// {}\n//\n// {}\n//   * {}\n//   * {}\n//   * {}\n//   * {}\n//\n// {}\n\n",
            tr!(
                "Kani（https://model-checking.github.io/kani/）の証明ハーネス。`kani {alias}_proof.rs`、または `rulec test --proofs`。すべて `#[cfg(kani)]` の中なので rustc は読まない。",
                "Proof harnesses for Kani (https://model-checking.github.io/kani/): `kani {alias}_proof.rs`, or `rulec test --proofs`. Everything is behind `#[cfg(kani)]`, so rustc never reads it."
            ),
            tr!("宣言した範囲のすべての入力について、次を検査する。", "Over every input in the declared domain, these are checked:"),
            tr!(
                "完全性（E101）— 表の末尾の `unreachable!`。Kani は到達可能な panic を既定で探す。",
                "completeness (E101) — the `unreachable!` that closes every table; Kani looks for a reachable panic by default."
            ),
            tr!(
                "W114 のガード — `RuleError::Contradiction` が返らないこと。静的に閉じなかった行対が、ここで閉じる（あるいは反例が出る）。",
                "the W114 guards — that `RuleError::Contradiction` is never returned. A pair the checker could not close either closes here, or a counterexample comes back."
            ),
            tr!(
                "int64（§7.4）— あふれ。Kani は算術のあふれを既定で検査する。",
                "int64 (§7.4) — overflow. Kani checks arithmetic overflow by default."
            ),
            tr!(
                "重なり（E105）— `rows_*` が、当たる行を数える。`unique` はちょうど一つ、`first` は一つ以上。",
                "overlap (E105) and completeness from the rows' side — `rows_*` counts the rows that match: exactly one for `unique`, at least one for `first`."
            ),
            tr!(
                "証明の対象はこの Rust であって、表でも検査器でもない（§15.47）。",
                "What is proved is this Rust, not the table and not the checker (§15.47)."
            ),
        ));
        o.push_str("#![allow(non_snake_case, uncommon_codepoints, unused_parens, unused_mut, unused_variables)]\n\n");
        o.push_str(&format!("#[cfg(kani)]\n#[path = \"{alias}.rs\"]\nmod r;\n\n#[cfg(kani)]\nmod proof {{\n    use super::r::*;\n\n"));

        // The inputs, made symbolic once: the same lines serve every harness in the file.
        let mut decls = String::new();
        let mut args: Vec<String> = Vec::new();
        let mut params: Vec<String> = Vec::new();
        let mut whole = true;
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            params.push(format!("{}: {}", pub_name(&i.name), self.rs_ty(&ty)));
            match self.rs_any(&i.name.text, &pub_name(&i.name), &ty, "        ") {
                Some((d, e)) => {
                    decls.push_str(&d);
                    args.push(e);
                }
                None => whole = false,
            }
        }
        // A `constraint` (§15.55) says a combination the rule was never asked to answer, and
        // the generated code refuses it at the door. It is part of the declared domain, so
        // the harness assumes it too; without that, the harness hands the rule an input its
        // own entry guard turns away. Both sides are inputs (E018), and an input here is the
        // plain i64, before it is branded for the call.
        for k in &self.f.constraints {
            let name = |n: &String| self.f.inputs.iter().find(|i| &i.name.text == n).map(|i| pub_name(&i.name));
            if let (Some(a), Some(b)) = (name(&k.left), name(&k.right)) {
                decls.push_str(&format!("        kani::assume({a} {} {b});\n", k.op.word()));
            }
        }
        let mut unwind = String::new();
        if let Some(el) = &self.f.elements {
            let cap = self.count_cap().unwrap_or(0);
            let seq = pub_name(&el.name);
            let mut lines = String::new();
            let mut inits: Vec<String> = Vec::new();
            for fd in &el.fields {
                let ty = self.ty_of(&fd.name.text);
                match self.rs_any(&fd.name.text, &format!("{}_v", pub_name(&fd.name)), &ty, "            ") {
                    Some((d, e)) => {
                        lines.push_str(&d);
                        inits.push(format!("{}: {e}", pub_name(&fd.name)));
                    }
                    None => whole = false,
                }
            }
            // The cap is the entry guard's, so the walk is covered whole rather than up to
            // some length picked here: `unwind` one past it leaves nothing bounded away.
            decls.push_str(&format!(
                "        let n: usize = kani::any();\n        kani::assume(n <= {cap});\n        \
                 let {seq}: [Element; {cap}] = core::array::from_fn(|_| {{\n{lines}            Element {{ {} }}\n        }});\n",
                inits.join(", ")
            ));
            args.push(format!("&{seq}[..n]"));
            unwind = format!("    #[kani::unwind({})]\n", cap + 2);
        }

        if !whole {
            o.push_str(&format!(
                "    // {}\n}}\n",
                tr!(
                    "この規則には string の入力があり、記号として置けないので、ハーネスは出していない。",
                    "This rule takes an input a harness cannot quantify over (a string), so none is written."
                )
            ));
            return o;
        }

        // The rows of each table, where the rule is a straight line. A rule that walks a
        // sequence has tables whose columns belong to one element and tables that read what
        // the walk counted, and neither can be replayed outside the walk; the harness above
        // still reaches the `unreachable!` of both.
        let mut harnesses = String::new();
        if self.f.elements.is_none() {
            for (k, it) in self.f.items.iter().enumerate() {
                let Item::Table(t0) = it else { continue };
                let Some(t) = self.c.table_at(t0) else { continue };
                let Some(rows) = self.rs_rows_fn(k, t, &params.join(", ")) else { continue };
                let tn = t.name.as_ref().map(pub_name).unwrap_or_else(|| format!("t{k}"));
                o.push_str(&rows);
                let (want, doc) = if t.policy == crate::ast::Policy::Unique {
                    (
                        format!("        assert_eq!(rows_{tn}({}), Ok(1));\n", args.join(", ")),
                        tr!("表 {} の行は、宣言した範囲を隙間なく重なりなく覆う。", "The rows of {} cover the declared domain with no gap and no overlap.", t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default()),
                    )
                } else {
                    (
                        format!("        assert!(matches!(rows_{tn}({}), Ok(n) if n >= 1));\n", args.join(", ")),
                        tr!("表 {} の行は、宣言した範囲を覆う（`first` は、重なったときは上の行が勝つ）。", "The rows of {} cover the declared domain (`first` settles an overlap by order).", t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default()),
                    )
                };
                harnesses.push_str(&format!(
                    "    /// {doc}\n    #[kani::proof]\n{unwind}    fn {tn}_rows() {{\n{decls}{want}    }}\n\n"
                ));
            }
        }

        o.push_str(&format!(
            "    /// {}\n    #[kani::proof]\n{unwind}    fn {alias}_answers() {{\n{decls}        assert!({alias}({}).is_ok());\n    }}\n\n",
            tr!(
                "宣言した範囲のどの入力にも、この規則は答えを返す。返らないのは、完全性に穴があるとき、W114 の対が実在したとき、入口のガードに当たったときの三つ。三つめが起きないことは、ここで仮定した範囲が宣言した範囲そのものだ、という確認でもある。",
                "For every input in the declared domain the rule answers. There are three ways not to — a gap in the completeness, a W114 pair that was real, and the entry guard — and the third not happening is also the check that the domain assumed here is the declared one."
            ),
            args.join(", ")
        ));
        o.push_str(&harnesses);
        o.push_str("}\n");
        o
    }

    /// The names of the harnesses `rs_proof` writes, in the order they appear.
    fn proof_harnesses(&self) -> Vec<String> {
        let alias = pub_name(&self.f.name);
        if self.f.inputs.iter().any(|i| matches!(self.ty_of(&i.name.text), Ty::Str)) {
            return Vec::new();
        }
        let mut v = vec![format!("{alias}_answers")];
        if self.f.elements.is_none() {
            for (k, it) in self.f.items.iter().enumerate() {
                let Item::Table(t0) = it else { continue };
                let Some(t) = self.c.table_at(t0) else { continue };
                v.push(format!("{}_rows", t.name.as_ref().map(pub_name).unwrap_or_else(|| format!("t{k}"))));
            }
        }
        v
    }

    /// The members of an enum, as the paths the generated Rust writes.
    fn enum_members(&self, n: &str) -> Vec<String> {
        if let Some(e) = self.f.enums.iter().find(|e| e.name.text == n) {
            return e.values.iter().map(|v| self.rs_value(&v.text)).collect();
        }
        if n == "都道府県" {
            return crate::prelude::PREFECTURES.iter().map(|(j, _)| self.rs_value(j)).collect();
        }
        Vec::new()
    }

    /// One value made symbolic for a harness: the lines that declare it, and the expression
    /// that goes in as the argument. `None` for a type there is no useful way to quantify
    /// over (a string), which drops the harness rather than pretending (§15.95).
    fn rs_any(&self, name: &str, ident: &str, ty: &Ty, ind: &str) -> Option<(String, String)> {
        match ty {
            Ty::Bool => Some((format!("{ind}let {ident}: bool = kani::any();\n"), ident.into())),
            Ty::Str => None,
            Ty::Enum(n) => {
                let vs = self.enum_members(n);
                if vs.is_empty() {
                    return None;
                }
                let arms: Vec<String> = vs
                    .iter()
                    .enumerate()
                    .map(|(k, v)| if k + 1 == vs.len() { format!("_ => {v}") } else { format!("{k} => {v}") })
                    .collect();
                Some((
                    format!("{ind}let {ident} = match kani::any::<u8>() {{ {} }};\n", arms.join(", ")),
                    ident.into(),
                ))
            }
            Ty::Opt(t) => {
                let (d, e) = self.rs_any(name, &format!("{ident}_v"), t, ind)?;
                Some((
                    format!("{d}{ind}let {ident} = if kani::any::<bool>() {{ Some({e}) }} else {{ None }};\n"),
                    ident.into(),
                ))
            }
            // Money, quantities, rates, numbers and dates all travel as an i64 (§10.2), and
            // the declared range is what the completeness proof quantified over, so it is
            // exactly what the harness assumes.
            _ => {
                let mut d = format!("{ind}let {ident}: i64 = kani::any();\n");
                if let Some((lo, hi)) = self.c.ranges.get(name) {
                    let sc = self.c.wire_scale(name);
                    let mut b: Vec<String> = Vec::new();
                    if let Some(lo) = lo {
                        b.push(format!("{} <= {ident}", crate::types::wire_int(*lo, sc)));
                    }
                    if let Some(hi) = hi {
                        b.push(format!("{ident} <= {}", crate::types::wire_int(*hi, sc)));
                    }
                    if !b.is_empty() {
                        d.push_str(&format!("{ind}kani::assume({});\n", b.join(" && ")));
                    }
                }
                let brand = self.rs_ty(ty);
                let e = if brand == "i64" { ident.to_string() } else { format!("{brand}({ident})") };
                Some((d, e))
            }
        }
    }

    /// How many rows of one table match, as a function of the rule's inputs. Everything the
    /// rule computes before the table is computed here the same way, so the dependencies
    /// between derived values — the ones the axis space drops, and W114 with them — are
    /// present as arithmetic (§15.95).
    fn rs_rows_fn(&self, upto: usize, t: &Table, params: &str) -> Option<String> {
        // The same reading of a name as the rule's own body: a branded input is an i64
        // inside, unwrapped where it is read.
        let local = |n: &str| -> String {
            match self.f.inputs.iter().find(|i| i.name.text == n) {
                Some(i) => {
                    let v = pub_name(&i.name);
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                        format!("{v}.0")
                    } else {
                        v
                    }
                }
                None => self.ident(n),
            }
        };
        let trace = self.temp("trace");
        let mut body = String::new();
        for (k, it) in self.f.items.iter().enumerate() {
            if k >= upto {
                break;
            }
            match it {
                Item::Derived(d) => body.push_str(&format!(
                    "    let {} = {};\n",
                    self.ident(&d.name.text),
                    rs_expr(unparen(&self.expr(&d.expr, &local).text))
                )),
                Item::Define(d) => body.push_str(&format!(
                    "    let {} = {};\n",
                    self.ident(&d.name.text),
                    rs_expr(unparen(&self.expr(&d.expr, &local).text))
                )),
                Item::Agg(_) => {}
                Item::Table(t) => {
                    let t = self.c.table_at(t)?;
                    body.push_str(&self.rs_table(t, &local, &trace));
                }
            }
        }
        let tname = t.name.as_ref().map(pub_name).unwrap_or_else(|| format!("t{upto}"));
        let mut o = format!(
            "    /// {}\n    fn rows_{tname}({params}) -> Result<u32, RuleError> {{\n",
            tr!(
                "表 {} の、当たる行の数。{}",
                "How many rows of {} match. {}",
                t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default(),
                if t.policy == crate::ast::Policy::Unique {
                    tr!("`policy unique` なら、どの入力でもちょうど一行。", "`policy unique` says exactly one.")
                } else {
                    tr!("`policy first` なら、どの入力でも一行以上。", "`policy first` says at least one.")
                }
            )
        );
        if !body.is_empty() {
            o.push_str(&format!("        let mut {trace}: Vec<Fired> = Vec::new();\n"));
            o.push_str(&Self::indent_block(&body, "    "));
        }
        o.push_str("        Ok(0\n");
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.rs_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "true".into() } else { conds.join(" && ") };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            o.push_str(&format!("            + (({cond}) as u32) // {}\n", self.row_head(t, ri, &cells.join(" | "))));
        }
        o.push_str("        )\n    }\n\n");
        Some(o)
    }
    pub fn rs_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let mut args: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            let jp = &i.name.text;
            args.push(match &ty {
                Ty::Enum(n) => {
                    let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                    format!("r::{cls}::parse(s(&d, {jp:?})).expect({jp:?})")
                }
                Ty::Bool => format!("b(&d, {jp:?})"),
                Ty::Date => format!("ord(s(&d, {jp:?}))"),
                Ty::Number => format!("n(&d, {jp:?})"),
                Ty::Str => format!("s(&d, {jp:?}).to_string()"),
                Ty::Opt(inner) => {
                    // `null` on the wire is `None`. The brand of an optional is `Option<Kind>`,
                    // which used to be pasted in as if it were a constructor (§15.88).
                    let one = match inner.as_ref() {
                        Ty::Enum(n) => {
                            let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                            format!("r::{cls}::parse(s(&d, {jp:?})).expect({jp:?})")
                        }
                        Ty::Bool => format!("b(&d, {jp:?})"),
                        Ty::Date => format!("ord(s(&d, {jp:?}))"),
                        Ty::Number => format!("n(&d, {jp:?})"),
                        Ty::Str => format!("s(&d, {jp:?}).to_string()"),
                        other => format!("r::{}(n(&d, {jp:?}))", self.rs_ty(other)),
                    };
                    // The runner's own scanner keeps an unquoted token as its text, so a JSON
    // null arrives as the four letters. An optional string input cannot occur
    // (a string is not a table column, E110), so nothing else can look like it.
    format!("if s(&d, {jp:?}) != \"null\" {{ Some({one}) }} else {{ None }}")
                }
                _ => format!("r::{}(n(&d, {jp:?}))", self.rs_ty(&ty)),
            });
        }
        // The sequence a walk reads, one element at a time (§15.56).
        if let Some(el) = &self.f.elements {
            let jp = &el.name.text;
            let fields: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let k = &fd.name.text;
                    let ty = self.ty_of(k);
                    let body = match &ty {
                        Ty::Enum(n) => {
                            let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                            format!("r::{cls}::parse(s(e, {k:?})).expect({k:?})")
                        }
                        Ty::Bool => format!("b(e, {k:?})"),
                        Ty::Date => format!("ord(s(e, {k:?}))"),
                        Ty::Number => format!("n(e, {k:?})"),
                        Ty::Str => format!("s(e, {k:?}).to_string()"),
                        Ty::Opt(inner) => {
                            // `null` on the wire is `None`. The brand of an optional is `Option<Kind>`,
                            // which used to be pasted in as if it were a constructor (§15.88).
                            let one = match inner.as_ref() {
                                Ty::Enum(n) => {
                                    let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                                    format!("r::{cls}::parse(s(e, {k:?})).expect({k:?})")
                                }
                                Ty::Bool => format!("b(e, {k:?})"),
                                Ty::Date => format!("ord(s(e, {k:?}))"),
                                Ty::Number => format!("n(e, {k:?})"),
                                Ty::Str => format!("s(e, {k:?}).to_string()"),
                                other => format!("r::{}(n(e, {k:?}))", self.rs_ty(other)),
                            };
                            format!("if s(e, {k:?}) != \"null\" {{ Some({one}) }} else {{ None }}")
                        }
                        _ => format!("r::{}(n(e, {k:?}))", self.rs_ty(&ty)),
                    };
                    format!("{}: {body}", pub_name(&fd.name))
                })
                .collect();
            args.push(format!(
                "rows(s(&d, {jp:?})).iter().map(|e| r::Element {{ {} }}).collect::<Vec<_>>()",
                fields.join(", ")
            ));
        }
        // The inputs are bound once and handed to both calls. Everything is `Copy` except a
        // string, which the first call takes as a clone.
        let binds: String = args.iter().enumerate().map(|(i, a)| format!("        let a{i} = {a};\n")).collect();
        let n_ins = self.f.inputs.len();
        let has_seq = self.f.elements.is_some();
        let pass = |clone: bool| -> String {
            let mut v: Vec<String> = self
                .f
                .inputs
                .iter()
                .enumerate()
                .map(|(i, inp)| {
                    if clone && matches!(self.ty_of(&inp.name.text), Ty::Str) {
                        format!("a{i}.clone()")
                    } else {
                        format!("a{i}")
                    }
                })
                .collect();
            if has_seq {
                v.push(format!("&a{n_ins}"));
            }
            v.join(", ")
        };
        let (first, second) = (pass(true), pass(false));
        format!(
            r#"// Code generated by rulec {ver}. DO NOT EDIT.
#![allow(non_snake_case, uncommon_codepoints, unused_parens)]

#[path = "{alias}.rs"]
mod r;

use std::io::Read;

{helpers}fn main() {{
    let mut src = String::new();
    std::io::stdin().read_to_string(&mut src).unwrap();
    for line in src.lines() {{
        if line.trim().is_empty() {{
            continue;
        }}
        let d = fields(line);
{binds}        let (got, trace) = r::{alias}_traced({first}).unwrap();
        println!("{{}}", r::{alias}_record({second}, got, &trace, ""));
    }}
}}
"#,
            ver = env!("CARGO_PKG_VERSION"),
            helpers = RS_JSON_HELPERS,
            binds = binds,
            first = first,
            second = second
        )
    }

    /// The rule as one Wasm module for any host (§15.64): the module `rust/` gets, behind the
    /// canonical ABI of `call: func(input: string) -> string`. A JavaScript host drives the
    /// core module through `cabi_realloc`, `call` and `cabi_post_call`; with the `.wit` beside
    /// it, `wasm-tools component new` makes a component of the same file without a change.
    pub fn rs_wasm(&self) -> String {
        let alias = pub_name(&self.f.name);
        // An unknown enum value is answered as an error, where the runner would panic: a host
        // gets a line it can read, not a trap.
        let bind_of = |src: &str, name: &str, ty: &Ty| -> String {
            match ty {
                Ty::Enum(n) => {
                    let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                    format!(
                        "match r::{cls}::parse(s({src}, {name:?})) {{ Some(v) => v, None => return error(&format!(\"{name}: {{:?}}\", s({src}, {name:?}))) }}"
                    )
                }
                Ty::Bool => format!("b({src}, {name:?})"),
                Ty::Date => format!("ord(s({src}, {name:?}))"),
                Ty::Number => format!("n({src}, {name:?})"),
                Ty::Str => format!("s({src}, {name:?}).to_string()"),
                // `null` on the wire is `None`; the scanner keeps an unquoted token as its
                // text, so the four letters are the test (DESIGN §15.88).
                Ty::Opt(inner) => {
                    let one = match inner.as_ref() {
                        Ty::Enum(n) => {
                            let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                            format!(
                                "match r::{cls}::parse(s({src}, {name:?})) {{ Some(v) => v, None => return error(&format!(\"{name}: {{:?}}\", s({src}, {name:?}))) }}"
                            )
                        }
                        Ty::Bool => format!("b({src}, {name:?})"),
                        Ty::Date => format!("ord(s({src}, {name:?}))"),
                        Ty::Number => format!("n({src}, {name:?})"),
                        Ty::Str => format!("s({src}, {name:?}).to_string()"),
                        other => format!("r::{}(n({src}, {name:?}))", self.rs_ty(other)),
                    };
                    format!("if s({src}, {name:?}) != \"null\" {{ Some({one}) }} else {{ None }}")
                }
                _ => format!("r::{}(n({src}, {name:?}))", self.rs_ty(ty)),
            }
        };
        let mut binds = String::new();
        for (i, inp) in self.f.inputs.iter().enumerate() {
            binds.push_str(&format!("    let a{i} = {};\n", bind_of("&d", &inp.name.text, &self.ty_of(&inp.name.text))));
        }
        let n_ins = self.f.inputs.len();
        let has_seq = self.f.elements.is_some();
        if let Some(el) = &self.f.elements {
            let fields: Vec<String> = el
                .fields
                .iter()
                .map(|fd| format!("{}: {}", pub_name(&fd.name), bind_of("e", &fd.name.text, &self.ty_of(&fd.name.text))))
                .collect();
            binds.push_str(&format!(
                "    let mut a{n_ins}: Vec<r::Element> = Vec::new();\n    for e in &rows(s(&d, {:?})) {{\n        a{n_ins}.push(r::Element {{ {} }});\n    }}\n",
                el.name.text,
                fields.join(", ")
            ));
        }
        let pass = |clone: bool| -> String {
            let mut v: Vec<String> = self
                .f
                .inputs
                .iter()
                .enumerate()
                .map(|(i, inp)| {
                    if clone && matches!(self.ty_of(&inp.name.text), Ty::Str) {
                        format!("a{i}.clone()")
                    } else {
                        format!("a{i}")
                    }
                })
                .collect();
            if has_seq {
                v.push(format!("&a{n_ins}"));
            }
            v.join(", ")
        };
        let (first, second) = (pass(true), pass(false));
        let mut o = format!(
            "// Code generated by rulec {}. DO NOT EDIT.\n#![allow(non_snake_case, uncommon_codepoints, unused_parens, dead_code)]\n\n#[path = \"{alias}.rs\"]\nmod r;\n\nuse std::alloc::{{alloc, realloc, Layout}};\n\n",
            env!("CARGO_PKG_VERSION")
        );
        o.push_str(RS_JSON_HELPERS);
        o.push_str(RS_JSON_STR);
        o.push_str(RS_WASM_ABI);
        o.push_str(&format!(
            "/// The inputs read, the rule called, the record line written.\nfn answer(text: &str) -> String {{\n    let chars: Vec<char> = text.chars().collect();\n    let top = pairs_from(&chars, 0).0;\n    // A record carries the inputs under \"in\"; a bare object is the inputs themselves.\n    let inner: Option<String> = top.iter().find(|(k, _)| k == \"in\").map(|(_, v)| v.clone());\n    let d = match inner {{\n        Some(v) => pairs_from(&v.chars().collect::<Vec<_>>(), 0).0,\n        None => top,\n    }};\n{binds}    match r::{alias}_traced({first}) {{\n        Ok((got, trace)) => r::{alias}_record({second}, got, &trace, \"\"),\n        Err(e) => error(&e.to_string()),\n    }}\n}}\n"
        ));
        o
    }

    /// The rule's interface for the component model (§15.64): one world exporting
    /// `call: func(input: string) -> string`, the JSON wire on both sides, with the inputs and
    /// outputs in the doc comments so that a reader of the `.wit` knows what to send.
    pub fn wit(&self) -> String {
        let alias = pub_name(&self.f.name);
        let world = wit_name(&alias);
        let mut o = format!("package rulec:{world}@{};\n\n", wit_version(&self.f.version));
        o.push_str(&format!("/// Rule {} v{}", self.f.name.text, self.f.version));
        if let Some(d) = &self.f.description {
            o.push_str(&format!(": {}", d.replace('\n', " ")));
        }
        o.push_str(&format!("\nworld {world} {{\n"));
        o.push_str(
            "    /// One call. `input` is a JSON object with the rule's inputs by their names, in the\n    /// wire form (whole numbers in the declared unit, enum values by name, dates as\n    /// YYYY-MM-DD); a record with them under \"in\" is read the same way. Back comes the\n    /// record line {\"in\":…,\"out\":…,\"trace\":[…]}, or {\"error\":\"…\"} for an input outside\n    /// the contract.\n    ///\n    /// Inputs:\n",
        );
        for i in &self.f.inputs {
            o.push_str(&format!("    ///   {}: {}\n", i.name.text, self.ty_of(&i.name.text)));
        }
        if let Some(el) = &self.f.elements {
            let fields: Vec<String> =
                el.fields.iter().map(|fd| format!("{}: {}", fd.name.text, self.ty_of(&fd.name.text))).collect();
            o.push_str(&format!("    ///   {}: a list of objects, each with {}\n", el.name.text, fields.join(", ")));
        }
        o.push_str("    /// Outputs:\n");
        for od in &self.f.outputs {
            o.push_str(&format!("    ///   {}: {}\n", od.name.text, self.ty_of(&od.name.text)));
        }
        o.push_str("    export call: func(input: string) -> string;\n}\n");
        o
    }

    /// The `wasi` entry, inside the Rust entry of `rulec api`: the *other* shape of Wasm
    /// (§15.63) — the runner itself built as a command that reads stdin and writes stdout, the
    /// shape Fastly Compute, Spin or a batch step in a sandbox takes. It sits under the
    /// language and not at the top level because the registry carries it as a property of a
    /// backend (`Backend::wasi`), so a second language that ever earned the column would get
    /// the same key.
    pub fn api_wasi(&self) -> String {
        let alias = pub_name(&self.f.name);
        crate::json::Obj::new()
            .str("source", &format!("{alias}_runner.rs"))
            .str("module", &format!("{alias}_runner.wasm"))
            .str(
                "build",
                &format!("rustc --edition 2021 {WASI_RUSTC_FLAGS} {alias}_runner.rs -o {alias}_runner.wasm"),
            )
            .str("run", &format!("wasmtime {alias}_runner.wasm"))
            .str("wire", "one vectors line on stdin, one fixtures record per line on stdout")
            .raw("needs", crate::json::strs(&["wasmtime", "rustup target add wasm32-wasip1"]))
            .finish()
    }

    /// The `wasm` entry of `rulec api`: the files, the build line, and the names of the
    /// exports a host needs.
    pub fn api_wasm(&self) -> String {
        let alias = pub_name(&self.f.name);
        let world = wit_name(&alias);
        crate::json::Obj::new()
            .str("source", &format!("{alias}_wasm.rs"))
            .str("module", &format!("{alias}.wasm"))
            .str("build", &format!("rustc --edition 2021 {WASM_RUSTC_FLAGS} {alias}_wasm.rs -o {alias}.wasm"))
            .str("wit", &format!("{alias}.wit"))
            .str("package", &format!("rulec:{world}@{}", wit_version(&self.f.version)))
            .str("world", &world)
            .str("call", "call")
            .str("call_signature", "call: func(input: string) -> string")
            .str("post_return", "cabi_post_call")
            .str("realloc", "cabi_realloc")
            .str("memory", "memory")
            .str("runner", &format!("{alias}_runner.mjs"))
            .str(
                "component",
                &format!("wasm-tools component embed {alias}.wit {alias}.wasm -o {alias}.embedded.wasm && wasm-tools component new {alias}.embedded.wasm -o {alias}.component.wasm"),
            )
            .finish()
    }
}

/// The JSON string escaper the generated Rust module and the Wasm entry share.
pub(crate) const RS_JSON_STR: &str = "fn json_str(s: &str) -> String {\n    let mut o = String::from(\"\\\"\");\n    for ch in s.chars() {\n        match ch {\n            '\"' => o.push_str(\"\\\\\\\"\"),\n            '\\\\' => o.push_str(\"\\\\\\\\\"),\n            c if (c as u32) < 32 => o.push_str(&format!(\"\\\\u{:04x}\", c as u32)),\n            c => o.push(c),\n        }\n    }\n    o.push('\"');\n    o\n}\n\n";

/// The flags the Wasm module is built with, as one line for `rulec api` and as arguments for
/// `rulec test`: size over speed, no unwinding, no symbols — 22 KB for a tariff rather than
/// 1.4 MB.
pub const WASM_RUSTC_FLAGS: &str =
    "-C opt-level=s -C lto -C panic=abort -C strip=symbols --target wasm32-unknown-unknown --crate-type cdylib";

/// The flags the Rust runner is built with as a WASI command (§15.63), as one line for
/// `rulec api` and as arguments for `rulec test`. Nothing is tuned for size here: this is the
/// same program the native runner is, reached through stdin and stdout.
pub const WASI_RUSTC_FLAGS: &str = "-O --target wasm32-wasip1";

/// The JSON reading the generated Rust runner and the Wasm entry share: one flat object as
/// (key, value) pairs, values kept as their source text, and a reader for each wire type.
pub(crate) const RS_JSON_HELPERS: &str = r##"/// One flat JSON object as (key, value) pairs, values kept as their source text.
fn fields(line: &str) -> Vec<(String, String)> {
    let b: Vec<char> = line.chars().collect();
    let mut i = 0usize;
    // Step into the object under "in" and read the pairs of the one level below it.
    while i < b.len() && !(b[i] == '"' && b[i..].starts_with(&['"', 'i', 'n', '"'])) {
        i += 1;
    }
    pairs_from(&b, i).0
}

/// The pairs of the object starting at or after `i`, and the index just past its closing
/// brace. A value that is itself an object or an array is kept whole, as its source text.
fn pairs_from(b: &[char], mut i: usize) -> (Vec<(String, String)>, usize) {
    let mut out = Vec::new();
    while i < b.len() && b[i] != '{' {
        i += 1;
    }
    i += 1;
    while i < b.len() && b[i] != '}' {
        while i < b.len() && b[i] != '"' && b[i] != '}' {
            i += 1;
        }
        if i >= b.len() || b[i] == '}' {
            break;
        }
        let (k, ni) = string_at(b, i);
        i = ni;
        while i < b.len() && b[i] != ':' {
            i += 1;
        }
        i += 1;
        while i < b.len() && b[i] == ' ' {
            i += 1;
        }
        let v = if b[i] == '"' {
            let (v, ni) = string_at(b, i);
            i = ni;
            v
        } else if b[i] == '[' || b[i] == '{' {
            let (v, ni) = balanced(b, i);
            i = ni;
            v
        } else {
            let s = i;
            while i < b.len() && b[i] != ',' && b[i] != '}' {
                i += 1;
            }
            b[s..i].iter().collect::<String>().trim().to_string()
        };
        out.push((k, v));
    }
    (out, i + 1)
}

/// The source text of one bracketed value, quotes respected.
fn balanced(b: &[char], i: usize) -> (String, usize) {
    let (open, close) = if b[i] == '[' { ('[', ']') } else { ('{', '}') };
    let (mut depth, mut j) = (0i32, i);
    while j < b.len() {
        if b[j] == '"' {
            let (_, nj) = string_at(b, j);
            j = nj;
            continue;
        }
        if b[j] == open {
            depth += 1;
        }
        if b[j] == close {
            depth -= 1;
            if depth == 0 {
                j += 1;
                break;
            }
        }
        j += 1;
    }
    (b[i..j].iter().collect(), j)
}

/// An array of flat objects, as the pairs of each (§15.56).
fn rows(v: &str) -> Vec<Vec<(String, String)>> {
    let b: Vec<char> = v.chars().collect();
    let (mut i, mut out) = (0usize, Vec::new());
    while i < b.len() {
        if b[i] == '{' {
            let (p, ni) = pairs_from(&b, i);
            out.push(p);
            i = ni;
        } else {
            i += 1;
        }
    }
    out
}

/// The string starting at `i`, and the index just past its closing quote.
fn string_at(b: &[char], i: usize) -> (String, usize) {
    let (mut i, mut s) = (i + 1, String::new());
    while i < b.len() && b[i] != '"' {
        if b[i] == '\\' && i + 1 < b.len() {
            i += 1;
            s.push(match b[i] {
                'n' => '\n',
                't' => '\t',
                c => c,
            });
        } else {
            s.push(b[i]);
        }
        i += 1;
    }
    (s, i + 1)
}

fn get<'a>(d: &'a [(String, String)], k: &str) -> &'a str {
    d.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str()).unwrap_or("")
}

fn n(d: &[(String, String)], k: &str) -> i64 {
    get(d, k).parse().unwrap_or(0)
}

fn s<'a>(d: &'a [(String, String)], k: &str) -> &'a str {
    get(d, k)
}

fn b(d: &[(String, String)], k: &str) -> bool {
    get(d, k) == "true"
}

/// A date as its day number from 1970-01-01, by the same civil-date arithmetic the tool uses.
fn ord(s: &str) -> i64 {
    let p: Vec<i64> = s.split('-').map(|x| x.parse().unwrap_or(0)).collect();
    let (y, m, d) = (p[0], p[1], p[2]);
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

"##;

/// The canonical ABI of `call: func(input: string) -> string`, written once for every rule.
pub(crate) const RS_WASM_ABI: &str = r##"/// The canonical ABI's allocator: the host allocates the input string here, and the module
/// frees it once read.
#[no_mangle]
pub unsafe extern "C" fn cabi_realloc(old: *mut u8, old_size: usize, align: usize, new_size: usize) -> *mut u8 {
    if new_size == 0 {
        return align as *mut u8;
    }
    let p = if old_size == 0 {
        alloc(Layout::from_size_align_unchecked(new_size, align))
    } else {
        realloc(old, Layout::from_size_align_unchecked(old_size, align), new_size)
    };
    if p.is_null() {
        std::process::abort();
    }
    p
}

/// `call: func(input: string) -> string`: the input is (pointer, length) of UTF-8 in this
/// module's memory, and what comes back points at a (pointer, length) pair for the answer,
/// which `cabi_post_call` frees.
#[no_mangle]
pub unsafe extern "C" fn call(ptr: *mut u8, len: usize) -> *mut [u32; 2] {
    let input = String::from_raw_parts(ptr, len, len);
    let out = answer(&input);
    drop(input);
    let s = out.into_boxed_str();
    let n = s.len();
    let p = Box::into_raw(s) as *mut u8;
    Box::into_raw(Box::new([p as u32, n as u32]))
}

#[no_mangle]
pub unsafe extern "C" fn cabi_post_call(ret: *mut [u32; 2]) {
    let r = Box::from_raw(ret);
    let (p, n) = (r[0] as *mut u8, r[1] as usize);
    drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(p, n) as *mut str));
}

fn error(message: &str) -> String {
    format!("{{\"error\":{}}}", json_str(message))
}

"##;

/// A WIT identifier from an ASCII alias: lowercase kebab-case, starting with a letter.
fn wit_name(alias: &str) -> String {
    let mut s: String = alias.chars().map(|c| if c == '_' { '-' } else { c.to_ascii_lowercase() }).collect();
    if !s.starts_with(|c: char| c.is_ascii_lowercase()) {
        s.insert_str(0, "r-");
    }
    s
}

/// `1` as `1.0.0`: a WIT package version is a semver.
fn wit_version(v: &str) -> String {
    let mut parts: Vec<String> = v
        .split('.')
        .map(|p| p.chars().filter(|c| c.is_ascii_digit()).collect::<String>())
        .map(|p| if p.is_empty() { "0".to_string() } else { p })
        .collect();
    parts.truncate(3);
    while parts.len() < 3 {
        parts.push("0".to_string());
    }
    parts.join(".")
}

/// The Node runner `rulec test` drives: the module built from `<alias>_wasm.rs`, one vector per
/// line of stdin through `call`, the record line out.
pub fn wasm_runner_js(alias: &str) -> String {
    format!("// Code generated by rulec {}. DO NOT EDIT.\n{}", env!("CARGO_PKG_VERSION"), WASM_RUNNER_JS.replace("ALIAS", alias))
}

const WASM_RUNNER_JS: &str = r#"import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const { instance } = await WebAssembly.instantiate(readFileSync(join(here, "ALIAS.wasm")), {});
const ex = instance.exports;
const enc = new TextEncoder();
const dec = new TextDecoder();

// One call through the canonical ABI of `call: func(input: string) -> string`. The views on
// the memory are taken after the call, because the memory may have grown during it.
function call(text) {
  const bytes = enc.encode(text);
  const ptr = ex.cabi_realloc(0, 0, 1, bytes.length);
  new Uint8Array(ex.memory.buffer, ptr, bytes.length).set(bytes);
  const ret = ex.call(ptr, bytes.length);
  const [p, n] = new Uint32Array(ex.memory.buffer, ret, 2);
  const out = dec.decode(new Uint8Array(ex.memory.buffer, p, n));
  ex.cabi_post_call(ret);
  return out;
}

for (const line of readFileSync(0, "utf8").split("\n")) {
  if (!line.trim()) continue;
  const out = call(line);
  if (out.startsWith('{"error":')) {
    console.error(out);
    process.exit(1);
  }
  console.log(out);
}
"#;

/// The rounding helpers as a module of their own, so that the Wasm side runs the same unit
/// vectors as every other language (§8.5).
pub fn round_wasm_rust() -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n// {}\n#![allow(dead_code, unused_parens)]\n",
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の五モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five modes of §7.3, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(round_rs().trim_start_matches('\n'));
    for m in ["down", "up", "half", "half_down", "bankers"] {
        o.push_str(&format!("\n#[no_mangle]\npub extern \"C\" fn rulec_round_{m}(x: i64, g: i64) -> i64 {{\n    round_{m}(x, g)\n}}\n"));
    }
    o
}

pub fn round_tests_wasm_js() -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n// {}\nimport {{ readFileSync }} from \"node:fs\";\nimport {{ dirname, join }} from \"node:path\";\nimport {{ fileURLToPath }} from \"node:url\";\n\nconst here = dirname(fileURLToPath(import.meta.url));\nconst {{ instance }} = await WebAssembly.instantiate(readFileSync(join(here, \"_round.wasm\")), {{}});\nconst ex = instance.exports;\n\nconst CASES = [\n",
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の五モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five modes of §7.3, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("  [\"{}\", {x}n, {g}n, {want}n],\n", mode_fn(m)));
    }
    o.push_str(
        "];\n\nlet bad = 0;\nfor (const [mode, x, g, want] of CASES) {\n  const got = ex[\"rulec_round_\" + mode](x, g);\n  if (got !== want) {\n    console.log(`NG ${mode}(${x}, ${g}) = ${got}, want ${want}`);\n    bad += 1;\n  }\n}\nif (bad > 0) {\n  process.exit(1);\n}\n",
    );
    let line = tr!("ok {{}} 件", "ok {{}} cases").replace("{}", "${CASES.length}");
    o.push_str(&format!("console.log(`{line}`);\n"));
    o
}

pub fn round_tests_rust() -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n// {}\n#![allow(unused_parens)]\n",
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の五モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five modes of §7.3, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(round_rs().trim_start_matches('\n'));
    o.push_str("\nconst CASES: &[(&str, i64, i64, i64)] = &[\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("    (\"{}\", {x}, {g}, {want}),\n", mode_fn(m)));
    }
    o.push_str(
        "];\n\nfn main() {\n    let mut bad = 0;\n    for (mode, x, g, want) in CASES {\n        \
         let got = match *mode {\n            \"down\" => round_down(*x, *g),\n            \
         \"up\" => round_up(*x, *g),\n            \"half\" => round_half(*x, *g),\n            \
         \"half_down\" => round_half_down(*x, *g),\n            _ => round_bankers(*x, *g),\n        };\n        if got != *want {\n            \
         println!(\"NG {mode}({x}, {g}) = {got}, want {want}\");\n            bad += 1;\n        \
         }\n    }\n    if bad > 0 {\n        std::process::exit(1);\n    }\n",
    );
    let line = tr!("ok {{}} 件", "ok {{}} cases");
    o.push_str(&format!("    println!(\"{line}\", CASES.len());\n}}\n"));
    o
}

// ---------------------------------------------------------------------------
// Unit vectors for the rounding helpers (§8.5)
// ---------------------------------------------------------------------------

/// Five modes × grids × values. Negative values and exact halves are always included.
/// Table-level agreement alone would hide a broken helper on a table that never produces a
/// fraction.
fn round_cases() -> Vec<(RoundMode, i128, i128, i128)> {
    let mut out = Vec::new();
    for m in [RoundMode::Down, RoundMode::Up, RoundMode::Half, RoundMode::HalfDown, RoundMode::Bankers] {
        for g in [1i128, 10, 100] {
            for x in [-25i128, -20, -15, -11, -10, -5, -1, 0, 1, 5, 10, 11, 15, 20, 25, 105, 150, 250] {
                let want = Rat::int(x).round_to(m, Rat::int(g));
                out.push((m, x, g, want.num / want.den));
            }
        }
    }
    out
}

pub fn round_tests_python() -> String {
    let mut o = format!(
        "# Code generated by rulec {}. DO NOT EDIT.\n\
         # {}\n\
         import sys\n\n",
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の五モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five modes of §7.3, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(round_py().trim_start_matches('\n'));
    o.push_str("\nCASES = [\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("    (\"{}\", {x}, {g}, {want}),\n", mode_fn(m)));
    }
    o.push_str("]\n\nFN = {\n    \"down\": _round_down,\n    \"up\": _round_up,\n    \"half\": _round_half,\n    \"half_down\": _round_half_down,\n    \"bankers\": _round_bankers,\n}\n\n");
    o.push_str(
        "bad = 0\nfor mode, x, g, want in CASES:\n    \
         got = FN[mode](x, g)\n    \
         if got != want:\n        \
             print(f\"NG {mode}({x}, {g}) = {got}, want {want}\")\n        \
             bad += 1\nif bad:\n    sys.exit(1)\n",
    );
    o.push_str(&format!("print(f\"{}\")\n", tr!("ok {{len(CASES)}} 件", "ok {{len(CASES)}} cases")));
    o
}

pub fn round_tests_typescript() -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n// {}\n",
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の五モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five modes of §7.3, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(round_ts().trim_start_matches('\n'));
    o.push_str("\nconst CASES: [string, bigint, bigint, bigint][] = [\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("  [\"{}\", {x}n, {g}n, {want}n],\n", mode_fn(m)));
    }
    o.push_str(
        "];\n\nconst FN: Record<string, (x: bigint, g: bigint) => bigint> = {\n  \
         down: _roundDown,\n  up: _roundUp,\n  half: _roundHalf,\n  half_down: _roundHalfDown,\n  bankers: _roundBankers,\n};\n\n\
         let bad = 0;\n\
         for (const [mode, x, g, want] of CASES) {\n  \
             const got = FN[mode](x, g);\n  \
             if (got !== want) {\n    \
                 console.log(`NG ${mode}(${x}, ${g}) = ${got}, want ${want}`);\n    \
                 bad += 1;\n  }\n}\n\
         if (bad > 0) {\n  process.exit(1);\n}\n",
    );
    o.push_str(&format!("console.log(`{}`);\n", tr!("ok ${{CASES.length}} 件", "ok ${{CASES.length}} cases")));
    o
}

pub fn round_tests_go(pkg: &str) -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n\
         // {}\n\
         package {pkg}\n\nimport \"testing\"\n\n\
         func TestRoundingModes(t *testing.T) {{\n\t\
         cases := []struct {{\n\t\tmode{CELL}string\n\t\tx, g, want{CELL}int64\n\t}}{{\n",
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の五モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five modes of §7.3, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("\t\t{{{:?}, {x}, {g}, {want}}},\n", mode_fn(m)));
    }
    o.push_str(
        "\t}\n\tfor _, c := range cases {\n\t\tvar got int64\n\t\tswitch c.mode {\n\t\t\
         case \"down\":\n\t\t\tgot = roundDown(c.x, c.g)\n\t\t\
         case \"up\":\n\t\t\tgot = roundUp(c.x, c.g)\n\t\t\
         case \"half\":\n\t\t\tgot = roundHalf(c.x, c.g)\n\t\t\
         case \"half_down\":\n\t\t\tgot = roundHalfDown(c.x, c.g)\n\t\t\
         case \"bankers\":\n\t\t\tgot = roundBankers(c.x, c.g)\n\t\t}\n\t\t\
         if got != c.want {\n\t\t\tt.Errorf(\"%s(%d, %d) = %d, want %d\", c.mode, c.x, c.g, got, c.want)\n\t\t}\n\t}\n}\n",
    );
    align(&o)
}


// ── The API inventory (`rulec api`) ──────────────────────────────────────
//
// What a caller has to know to invoke the generated code: the module and function names,
// the parameters in order with their brands, units and ranges, the outputs with their
// rounding, the enum members under the spelling the generated code gives them, and the
// errors it can raise.
//
// It lives here, next to the generator, on purpose. Anywhere else it would be a second
// description of the same thing and would start drifting the first time a name changes; here
// it is built from the same `pub_name`, `pascal`, `py_ty` and `go_ty` the emitters use, and a
// test runs the generated code of every language against it.

impl Gen<'_> {
    /// The unit as it is written in the rule (`円`, `g`, `%`), or absent for a type that has
    /// none.
    fn unit_of(ty: &Ty) -> Option<String> {
        match ty {
            Ty::Money { cur, .. } => Some(cur.clone()),
            Ty::Qty { unit, .. } => Some(unit.clone()),
            Ty::Rate => Some("%".into()),
            Ty::Opt(t) => Self::unit_of(t),
            _ => None,
        }
    }

    /// The range **the generated guard actually enforces**, not a second computation of it.
    /// Both entry guards above take `lo.num` and `hi.num`, so this does too: an inventory that
    /// disagreed with the guard would send a caller values the code then rejects. A test holds
    /// these two numbers to the text of the generated guard (§8.6).
    fn range_json(&self, name: &str) -> Option<String> {
        let (lo, hi) = self.c.ranges.get(name)?;
        let (lo, hi) = (lo.as_ref()?, hi.as_ref()?);
        let sc = self.c.wire_scale(name);
        Some(
            crate::json::Obj::new()
                .int("min", crate::types::wire_int(*lo, sc))
                .int("max", crate::types::wire_int(*hi, sc))
                .finish(),
        )
    }

    /// `{"mode":"down","grid":1}` — the grid as an integer in the canonical unit, so it can be
    /// compared against a value straight away.
    fn rounding_json(&self, od: &OutDecl) -> Option<String> {
        let rd = od.rounding.as_ref()?;
        let ty = self.ty_of(&od.name.text);
        let g = crate::types::lit_value_in_pub(&rd.grid, &ty)?;
        Some(
            crate::json::Obj::new()
                .str("mode", &rd.mode)
                .int("grid", g.num / g.den)
                .finish(),
        )
    }

    /// One parameter or field, described the same way on both sides.
    fn value_json(&self, jp: &str, alias: &str, ty_name: &str, ty: &Ty) -> String {
        crate::json::Obj::new()
            .str("name", jp)
            .str("alias", alias)
            .str("type", ty_name)
            .opt_str("unit", Self::unit_of(ty))
            .opt_raw("range", self.range_json(jp))
            .bool("optional", matches!(ty, Ty::Opt(_)))
            .finish()
    }

    /// The sequence a walk reads, as one entry of the inventory's parameter list: the
    /// parameter itself, and the fields one element carries, each stated the way an input is
    /// (§15.56). A caller that reads the inventory rather than the code needs both.
    fn elements_json(
        &self,
        ty_name: &str,
        alias: impl Fn(&crate::ast::Name) -> String,
        fty: impl Fn(&Ty) -> String,
    ) -> Option<String> {
        let el = self.f.elements.as_ref()?;
        let fields: Vec<String> = el
            .fields
            .iter()
            .map(|fd| {
                let ty = self.ty_of(&fd.name.text);
                self.value_json(&fd.name.text, &alias(&fd.name), &fty(&ty), &ty)
            })
            .collect();
        let head = crate::json::Obj::new()
            .str("name", &el.name.text)
            .str("alias", &alias(&el.name))
            .str("type", ty_name)
            .bool("optional", false)
            .finish();
        Some(format!("{},\"elements\":{}}}", head.trim_end_matches('}'), crate::json::arr(&fields)))
    }

    /// The enums, under the spelling each language gives their members.
    fn enums_json(&self, member: impl Fn(&str, &str) -> String) -> String {
        let mut out: Vec<String> = Vec::new();
        let mut done: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if done.contains(ascii) {
                continue;
            }
            done.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            let members: Vec<String> = vals
                .iter()
                .map(|v| {
                    let a = self.value_names.get(v).map(|(_, a)| a.clone()).unwrap_or_else(|| v.clone());
                    crate::json::Obj::new()
                        .str("name", v)
                        .str("alias", member(ascii, &a))
                        .finish()
                })
                .collect();
            out.push(
                crate::json::Obj::new()
                    .str("name", jp)
                    .str("alias", ascii)
                    .raw("values", crate::json::arr(&members))
                    .finish(),
            );
        }
        crate::json::arr(&out)
    }

    /// `rulec api <file.rule> --format json`. Language independent throughout: every field is
    /// a name or a number that the generated code really uses.
    pub fn api(&self) -> String {
        let alias = pub_name(&self.f.name);
        let pkg = alias.replace('_', "").to_lowercase();
        let outs = &self.f.outputs;

        // The record function's own parameter names (§15.35): a temporary that no declared
        // name answers to, so an input aliased `out` does not collide.
        let (r_out, r_trace, r_tag) = (self.temp("out"), self.temp("trace"), self.temp("tag"));

        // --- Python
        let params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &pub_name(&i.name), &self.py_ty(&ty), &ty)
            })
            .chain(self.elements_json("list[Element]", |n| pub_name(n), |t| self.py_ty(t)))
            .collect();
        let py_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &self.py_ty(&ty), &ty);
                // An output carries its rounding as well; `value_json` is shared with the
                // inputs, so it is spliced in here rather than made optional there.
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let py_ret = if outs.len() == 1 {
            self.py_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        // The sequence a walk reads is the last parameter (§15.56), in the inventory as in
        // the code, so an argument list built from here is the one the module takes.
        let py_args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.py_ty(&self.ty_of(&i.name.text))))
            .chain(self.f.elements.iter().map(|el| format!("{}: list[Element]", pub_name(&el.name))))
            .collect();
        let py_sig = format!("def {alias}({}) -> {py_ret}:", py_args.join(", "));
        let py_traced = py_sig
            .replacen(&format!("def {alias}("), &format!("def {alias}_traced("), 1)
            .replace(&format!(") -> {py_ret}:"), &format!(") -> tuple[{py_ret}, list[Fired]]:"));
        let python = crate::json::Obj::new()
            .str("module", &alias)
            .str("mcp", &format!("{alias}_mcp.py"))
            .str("page", &format!("{alias}_page.html"))
            .str("function", &alias)
            .str("signature", &py_sig)
            .str("traced", &format!("{alias}_traced"))
            .str("traced_signature", &py_traced)
            .str("record", &format!("{alias}_record"))
            .str("record_signature", &format!(
                "def {alias}_record({}, {r_out}: {py_ret}, {r_trace}: _Trace, {r_tag}: str = \"\") -> str:",
                py_args.join(", ")
            ))
            .raw("params", crate::json::arr(&params))
            .str("returns", &py_ret)
            .raw("outputs", crate::json::arr(&py_outs))
            // A Python enum member is the alias in upper case (`CouponKind.PERCENT`).
            .raw("enums", self.enums_json(|_, a| a.to_uppercase()))
            .raw("errors", crate::json::strs(&["RuleInputError", "RuleContradictionError"]))
            .finish();

        // --- TypeScript
        let ts_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &pub_name(&i.name), &self.ts_ty(&ty), &ty)
            })
            .chain(self.elements_json("readonly Element[]", |n| pub_name(n), |t| self.ts_ty(t)))
            .collect();
        let ts_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &self.ts_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let ts_ret = if outs.len() == 1 {
            self.ts_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let ts_args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.ts_ty(&self.ty_of(&i.name.text))))
            .chain(self.f.elements.iter().map(|el| format!("{}: readonly Element[]", pub_name(&el.name))))
            .collect();
        let ts_sig = format!("export function {alias}({}): {ts_ret}", ts_args.join(", "));
        let ts_traced = ts_sig
            .replacen(&format!("export function {alias}("), &format!("export function {alias}_traced("), 1)
            .replace(&format!("): {ts_ret}"), &format!("): [{ts_ret}, Fired[]]"));
        let typescript = crate::json::Obj::new()
            .str("module", &format!("{alias}.ts"))
            .str("mcp", &format!("{alias}_mcp.ts"))
            .str("page", &format!("{alias}_page.html"))
            .str("function", &alias)
            .str("signature", &ts_sig)
            .str("traced", &format!("{alias}_traced"))
            .str("traced_signature", &ts_traced)
            .str("record", &format!("{alias}_record"))
            .str("record_signature", &format!(
                "export function {alias}_record({}, {r_out}: {ts_ret}, {r_trace}: Fired[], {r_tag} = \"\"): string",
                ts_args.join(", ")
            ))
            .raw("params", crate::json::arr(&ts_in))
            .str("returns", &ts_ret)
            .raw("outputs", crate::json::arr(&ts_outs))
            // A TypeScript enum member is the alias in upper case on a frozen object
            // (`CouponKind.PERCENT`), the same spelling Python uses.
            .raw("enums", self.enums_json(|_, a| a.to_uppercase()))
            .raw("errors", crate::json::strs(&["RuleInputError", "RuleContradictionError"]))
            .finish();

        // --- JavaScript: the TypeScript names without the types (§15.36). A value is what it
        // is at run time there: a bigint for every number and date, a string for an enum
        // member (its name) and for a string, a boolean.
        let js_ty = |ty: &Ty| -> String {
            match ty {
                Ty::Enum(_) | Ty::Str => "string".into(),
                Ty::Bool => "boolean".into(),
                Ty::Opt(t) => format!("{} | null", match **t {
                    Ty::Enum(_) | Ty::Str => "string",
                    Ty::Bool => "boolean",
                    _ => "bigint",
                }),
                _ => "bigint".into(),
            }
        };
        let js_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &pub_name(&i.name), &js_ty(&ty), &ty)
            })
            .chain(self.elements_json("Element[]", |n| pub_name(n), &js_ty))
            .collect();
        let js_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &js_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let js_params = self
            .f
            .inputs
            .iter()
            .map(|i| pub_name(&i.name))
            .chain(self.f.elements.iter().map(|el| pub_name(&el.name)))
            .collect::<Vec<_>>()
            .join(", ");
        let js_ret = if outs.len() == 1 { js_ty(&self.ty_of(&outs[0].name.text)) } else { "Output".into() };
        let javascript = crate::json::Obj::new()
            .str("module", &format!("{alias}.mjs"))
            .str("mcp", &format!("{alias}_mcp.mjs"))
            .str("page", &format!("{alias}_page.html"))
            .str("function", &alias)
            .str("signature", &format!("export function {alias}({js_params})"))
            .str("traced", &format!("{alias}_traced"))
            .str("traced_signature", &format!("export function {alias}_traced({js_params})"))
            .str("record", &format!("{alias}_record"))
            .str("record_signature", &format!("export function {alias}_record({js_params}, {r_out}, {r_trace}, {r_tag} = \"\")"))
            .raw("params", crate::json::arr(&js_in))
            .str("returns", &js_ret)
            .raw("outputs", crate::json::arr(&js_outs))
            .raw("enums", self.enums_json(|_, a| a.to_uppercase()))
            .raw("errors", crate::json::strs(&["RuleInputError", "RuleContradictionError"]))
            .finish();

        // --- Rust
        let rs_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &pub_name(&i.name), &self.rs_ty(&ty), &ty)
            })
            .chain(self.elements_json("&[Element]", |n| pub_name(n), |t| self.rs_ty(t)))
            .collect();
        let rs_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &self.rs_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let rs_ret = if outs.len() == 1 {
            self.rs_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let rs_args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.rs_ty(&self.ty_of(&i.name.text))))
            .chain(self.f.elements.iter().map(|el| format!("{}: &[Element]", pub_name(&el.name))))
            .collect();
        let rs_sig = format!("pub fn {alias}({}) -> Result<{rs_ret}, RuleError>", rs_args.join(", "));
        let rs_traced = rs_sig
            .replacen(&format!("pub fn {alias}("), &format!("pub fn {alias}_traced("), 1)
            .replace(&format!(") -> Result<{rs_ret}, RuleError>"), &format!(") -> Result<({rs_ret}, Vec<Fired>), RuleError>"));
        let rust = crate::json::Obj::new()
            .str("module", &format!("{alias}.rs"))
            .str("function", &alias)
            .str("signature", &rs_sig)
            .str("traced", &format!("{alias}_traced"))
            .str("traced_signature", &rs_traced)
            .str("record", &format!("{alias}_record"))
            .str("record_signature", &format!(
                "pub fn {alias}_record({}, {r_out}: {rs_ret}, {r_trace}: &[Fired], {r_tag}: &str) -> String",
                rs_args.join(", ")
            ))
            .raw("params", crate::json::arr(&rs_in))
            .str("returns", &rs_ret)
            .raw("outputs", crate::json::arr(&rs_outs))
            // A Rust enum member is the alias in PascalCase on the enum's own type.
            .raw("enums", self.enums_json(|_, a| pascal(a)))
            .raw("errors", crate::json::strs(&["RuleError::Input", "RuleError::Contradiction"]))
            // The proof harnesses beside the module (§15.95): the file, and the name of
            // every harness in it, so `kani --harness <name>` can be written from here.
            .str("proof", &format!("{alias}_proof.rs"))
            .raw("harnesses", crate::json::strs(&self.proof_harnesses().iter().map(|s| s.as_str()).collect::<Vec<_>>()))
            .raw("wasi", self.api_wasi())
            .finish();

        // --- Ruby. The unit is not in the type here (§15.20), so the entry states it
        // the way the others do and the generated comment repeats it for a reader.
        let rb_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &pub_name(&i.name), &self.rb_api_ty(&ty), &ty)
            })
            .chain(self.elements_json("Array[Element]", |n| pub_name(n), |t| self.rb_api_ty(t)))
            .collect();
        let rb_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &self.rb_api_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let rb_ret = if outs.len() == 1 {
            self.rb_api_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let rb_args = self
            .f
            .inputs
            .iter()
            .map(|i| pub_name(&i.name))
            .chain(self.f.elements.iter().map(|el| pub_name(&el.name)))
            .collect::<Vec<_>>()
            .join(", ");
        let rb_sig = format!("{}.{alias}({rb_args})", self.rb_module());
        let rb_traced = rb_sig.replacen(&format!(".{alias}("), &format!(".{alias}_traced("), 1);
        let ruby = crate::json::Obj::new()
            .str("module", &self.rb_module())
            .str("function", &alias)
            .str("signature", &rb_sig)
            .str("traced", &format!("{alias}_traced"))
            .str("traced_signature", &rb_traced)
            .str("record", &format!("{alias}_record"))
            .str("record_signature", &format!(
                "{}.{alias}_record({rb_args}, {r_out}, {r_trace}, {r_tag} = \"\")",
                self.rb_module()
            ))
            // The signature file that ships with it; `steep` reads this, not the entry.
            .str("rbs", &format!("sig/{alias}.rbs"))
            .raw("params", crate::json::arr(&rb_in))
            .str("returns", &rb_ret)
            .raw("outputs", crate::json::arr(&rb_outs))
            // A Ruby enum member is a constant under the type's module (`Band::SHORT`).
            .raw("enums", self.enums_json(|_, a| a.to_uppercase()))
            .raw("errors", crate::json::strs(&["RuleInputError", "RuleContradictionError"]))
            .finish();

        // --- PHP. The type is written down (§15.77) but carries no unit, so a param's
        // `type` is what the signature says and the `unit` field beside it says the rest.
        let php_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &pub_name(&i.name), &self.php_api_ty(&ty), &ty)
            })
            // The alias here is the declared name, as in Python and Ruby: PHP takes its
            // arguments by position, and `$` belongs to the spelling, not to the name.
            .chain(self.elements_json("Element[]", |n| pub_name(n), |t| self.php_api_ty(t)))
            .collect();
        let php_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &self.php_api_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let php_fname = self.php_fname();
        let php_ret = if outs.len() == 1 {
            self.php_api_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let php_args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                format!(
                    "{} {}",
                    self.php_api_ty(&self.ty_of(&i.name.text)),
                    crate::codegen::php::php_var_pub(&pub_name(&i.name))
                )
            })
            .chain(
                self.f
                    .elements
                    .iter()
                    .map(|el| format!("array {}", crate::codegen::php::php_var_pub(&pub_name(&el.name)))),
            )
            .collect();
        let php_sig = format!("function {php_fname}({}): {php_ret}", php_args.join(", "));
        let php_traced = format!("function {php_fname}_traced({}): array", php_args.join(", "));
        let php = crate::json::Obj::new()
            .str("module", &format!("{alias}.php"))
            // The namespace every name below sits in: a caller writes `use` for it, or
            // qualifies the call, and nothing here needs an autoloader.
            .str("namespace", &self.php_ns())
            .str("function", &php_fname)
            .str("signature", &php_sig)
            .str("traced", &format!("{php_fname}_traced"))
            .str("traced_signature", &php_traced)
            .str("record", &format!("{php_fname}_record"))
            .str("record_signature", &format!(
                "function {php_fname}_record({}, {php_ret} {}, array {}, string {} = ''): string",
                php_args.join(", "),
                crate::codegen::php::php_var_pub(&r_out),
                crate::codegen::php::php_var_pub(&r_trace),
                crate::codegen::php::php_var_pub(&r_tag)
            ))
            .raw("params", crate::json::arr(&php_in))
            .str("returns", &php_ret)
            .raw("outputs", crate::json::arr(&php_outs))
            // A PHP enum member is a case on the enum's own type, spelled in upper case as
            // Python and TypeScript spell theirs (`CouponKind::PERCENT`).
            .raw("enums", self.enums_json(|t, a| format!("{}::{}", crate::codegen::php::php_name(t), crate::codegen::php::php_name(&a.to_uppercase()))))
            .raw("errors", crate::json::strs(&["RuleInputError", "RuleContradictionError"]))
            .finish();

        // --- Swift. The unit is in the type here, as it is in Rust, so a param's `type`
        // is the brand and the `unit` field beside it repeats what that brand stands for.
        let sw_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &sw_name(&pub_name(&i.name)), &self.sw_api_ty(&ty), &ty)
            })
            .chain(self.elements_json("[Element]", |n| sw_name(&pub_name(n)), |t| self.sw_api_ty(t)))
            .collect();
        let sw_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &sw_name(&pub_name(&od.name)), &self.sw_api_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let sw_fname = sw_name(&pub_name(&self.f.name));
        let sw_ret = if outs.len() == 1 {
            self.sw_api_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let sw_args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", sw_name(&pub_name(&i.name)), self.sw_ty(&self.ty_of(&i.name.text))))
            .chain(self.f.elements.iter().map(|el| format!("{}: [Element]", sw_name(&pub_name(&el.name)))))
            .collect();
        let sw_sig = format!("func {sw_fname}({}) throws -> {sw_ret}", sw_args.join(", "));
        let sw_traced = sw_sig
            .replacen(&format!("func {sw_fname}("), &format!("func {sw_fname}Traced("), 1)
            .replace(&format!(") throws -> {sw_ret}"), &format!(") throws -> ({sw_ret}, [Fired])"));
        let swift = crate::json::Obj::new()
            .str("module", &format!("{alias}.swift"))
            .str("function", &sw_fname)
            .str("signature", &sw_sig)
            .str("traced", &format!("{sw_fname}Traced"))
            .str("traced_signature", &sw_traced)
            .str("record", &format!("{sw_fname}Record"))
            .str("record_signature", &format!(
                "func {sw_fname}Record({}, {}: {sw_ret}, {}: [Fired], {}: String = \"\") -> String",
                sw_args.join(", "),
                sw_name(&r_out),
                sw_name(&r_trace),
                sw_name(&r_tag)
            ))
            .raw("params", crate::json::arr(&sw_in))
            .str("returns", &sw_ret)
            .raw("outputs", crate::json::arr(&sw_outs))
            // A Swift enum member is the alias in the language's own spelling, on the
            // enum's own type (`Band.short`).
            .raw("enums", self.enums_json(|_, a| sw_name(a)))
            .raw("errors", crate::json::strs(&["RuleError.input", "RuleError.contradiction"]))
            .finish();

        // --- Java. `long` is the int64 the proof is about, and no brand rides in the
        // type (§15.78), so a param's `type` is what the signature says and `unit` says the
        // rest.
        let jv_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &self.java_name_of(&pub_name(&i.name)), &self.java_api_ty(&ty), &ty)
            })
            .chain(self.elements_json("List<Element>", |n| crate::codegen::java::java_name_pub(&pub_name(n)), |t| self.java_api_ty(t)))
            .collect();
        let jv_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &self.java_api_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let jv_cls = self.java_class();
        let jv_fname = self.java_fname();
        let jv_ret = if outs.len() == 1 {
            self.java_api_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let jv_args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{} {}", self.java_api_ty(&self.ty_of(&i.name.text)), self.java_name_of(&pub_name(&i.name))))
            .chain(self.f.elements.iter().map(|el| format!("List<Element> {}", self.java_name_of(&pub_name(&el.name)))))
            .collect();
        let java = crate::json::Obj::new()
            .str("module", &format!("{jv_cls}.java"))
            .str("class", &jv_cls)
            .str("function", &jv_fname)
            .str("signature", &format!("public static {jv_ret} {jv_fname}({})", jv_args.join(", ")))
            .str("traced", &format!("{jv_fname}Traced"))
            .str("traced_signature", &format!("public static Traced {jv_fname}Traced({})", jv_args.join(", ")))
            .str("record", &format!("{jv_fname}Record"))
            .str("record_signature", &format!(
                "public static String {jv_fname}Record({}, {jv_ret} {}, List<Fired> {}, String {})",
                jv_args.join(", "),
                self.java_name_of(&r_out),
                self.java_name_of(&r_trace),
                self.java_name_of(&r_tag)
            ))
            // Everything is nested in the one class, so a caller says `Rule.Member` where
            // Python says `Module.Member`.
            .str("build", "javac --release 17 -encoding UTF-8 -d classes *.java")
            .raw("params", crate::json::arr(&jv_in))
            .str("returns", &jv_ret)
            .raw("outputs", crate::json::arr(&jv_outs))
            .raw("enums", self.enums_json(|t, a| format!("{}.{}", crate::codegen::java_class(t), a.to_uppercase())))
            .raw("errors", crate::json::strs(&["RuleInputError", "RuleContradictionError"]))
            .finish();

        // --- Go
        let go_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(
                    &i.name.text,
                    &pascal(&pub_name(&i.name)),
                    &self.go_ty(&ty),
                    &ty,
                )
            })
            .chain(self.elements_json("[]Element", |n| pascal(&pub_name(n)), |t| self.go_ty(t)))
            .collect();
        let go_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pascal(&pub_name(&od.name)), &self.go_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let go_fn = pascal(&alias);
        let go_ret = if outs.len() == 1 {
            self.go_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let go_sig = format!("func {go_fn}(in Input) ({go_ret}, error)");
        let go_traced = format!("func {go_fn}Traced(in Input) ({go_ret}, []Fired, error)");
        let go = crate::json::Obj::new()
            .str("package", &pkg)
            .str("func", &go_fn)
            .str("signature", &go_sig)
            .str("traced", &format!("{go_fn}Traced"))
            .str("traced_signature", &go_traced)
            .str("record", &format!("{go_fn}Record"))
            .str("record_signature", &format!("func {go_fn}Record(in Input, out {go_ret}, trace []Fired, tag string) string"))
            .str("input_type", "Input")
            .raw("input_fields", crate::json::arr(&go_in))
            .str("output_type", &go_ret)
            .raw("output_fields", crate::json::arr(&go_outs))
            // A Go enum member is the type name followed by the alias (`CouponKindPercent`).
            .raw("enums", self.enums_json(|t, a| format!("{t}{a}")))
            .finish();

        crate::json::Obj::new()
            .str("rule", &self.f.name.text)
            .str("alias", &alias)
            .str("version", &self.f.version)
            .str("source_sha256", &self.src_hash)
            .raw("applies", crate::json::arr(&self.f.applies.iter().map(|a| {
                crate::json::Obj::new()
                    .str("name", &a.name.text)
                    .str("path", &a.path)
                    .str("sha256", a.hash.as_deref().unwrap_or(""))
                    .finish()
            }).collect::<Vec<_>>()))
            // The documents transcribed, as the header names them (§15.71).
            .raw("sources", crate::json::arr(&self.f.sources.iter().map(|s| {
                let o = crate::json::Obj::new().str("name", &s.name.text);
                match &s.kind {
                    SourceKind::Law { id, asof } => o
                        .str("kind", crate::kw::LAW)
                        .str("id", id)
                        .str("asof", asof)
                        .raw("pins", crate::json::arr(&s.pins.iter().map(|p| {
                            crate::json::Obj::new().str("fragment", &p.fragment).str("sha256", &p.hash).finish()
                        }).collect::<Vec<_>>()))
                        .finish(),
                    SourceKind::File { path, url, hash } => o
                        .str("kind", crate::kw::FILE)
                        .str("path", path)
                        .str("url", url.as_deref().unwrap_or(""))
                        .str("sha256", hash.as_deref().unwrap_or(""))
                        .raw("pins", crate::json::arr(&s.pins.iter().map(|p| {
                            crate::json::Obj::new().str("fragment", &p.fragment).str("sha256", &p.hash).finish()
                        }).collect::<Vec<_>>()))
                        .finish(),
                }
            }).collect::<Vec<_>>()))
            .raw("python", python)
            .raw("typescript", typescript)
            .raw("javascript", javascript)
            .raw("rust", rust)
            .raw("ruby", ruby)
            .raw("php", php)
            .raw("go", go)
            .raw("swift", swift)
            .raw("java", java)
            .raw("numpy", self.api_numpy())
            .raw("sql", self.api_sql())
            .raw("wasm", self.api_wasm())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Ruby (§15.20). The closest relative of the Python backend: dynamically typed,
// and `/` on Integers rounds toward −∞ exactly as Python's `//` does. What is
// different is the constant rule — a Ruby constant must begin with an uppercase
// ASCII letter, so a Japanese name cannot be one and groups take a prefix.

/// Turn a name into something Ruby will accept as a constant.
fn rb_const(n: &str) -> String {
    if n.starts_with(|c: char| c.is_ascii_uppercase()) {
        n.to_string()
    } else {
        format!("C_{n}")
    }
}

fn rb_expr(s: &str) -> String {
    // `//` becomes `/`: Ruby's integer division already floors toward −∞.
    s.replace(" // ", " / ")
        .replace("True", "true")
        .replace("False", "false")
}

/// The five modes of §7.3, and the two comparisons. Nothing is left to Ruby's own
/// division: `abs` first, then the sign is carried back, so every mode means the
/// same thing it means in the other four.
fn round_rb() -> String {
    format!(
        r#"
  def self._min(a, b)
    a < b ? a : b
  end

  def self._max(a, b)
    a > b ? a : b
  end

  # {down}
  def self._round_down(x, g)
    v = x.abs / g * g
    x < 0 ? -v : v
  end

  # {up}
  def self._round_up(x, g)
    q, r = x.abs / g, x.abs % g
    v = r.zero? ? q * g : (q + 1) * g
    x < 0 ? -v : v
  end

  # {half}
  def self._round_half(x, g)
    q, r = x.abs / g, x.abs % g
    v = 2 * r >= g ? (q + 1) * g : q * g
    x < 0 ? -v : v
  end

  # {half_down}
  def self._round_half_down(x, g)
    q, r = x.abs / g, x.abs % g
    v = 2 * r > g ? (q + 1) * g : q * g
    x < 0 ? -v : v
  end

  # {bankers}
  def self._round_bankers(x, g)
    q, r = x.abs / g, x.abs % g
    q += 1 if 2 * r > g || (2 * r == g && q.odd?)
    v = q * g
    x < 0 ? -v : v
  end

  private_class_method :_min, :_max, :_round_down, :_round_up, :_round_half, :_round_half_down, :_round_bankers
"#,
        down = tr!("0 へ寄せる。-4.8円 → -4円。", "Toward zero: -4.8 yen -> -4 yen."),
        up = tr!("0 から遠ざける。-4.2円 → -5円。", "Away from zero: -4.2 yen -> -5 yen."),
        half = tr!("半分ちょうどは 0 から遠ざける。", "An exact half goes away from zero."),
        half_down = tr!("半分ちょうどは 0 へ寄せる。", "An exact half goes toward zero."),
        bankers = tr!("半分ちょうどは偶数へ。", "An exact half goes to the even neighbor."),
    )
}

impl<'a> Gen<'a> {
    /// An enum value, written as the constant the module declares for it.
    fn rb_value(&self, v: &str) -> String {
        match self.value_names.get(v) {
            Some((ty, alias)) => format!("{}::{}", rb_const(ty), rb_const(&alias.to_uppercase())),
            None => format!("{v:?}"),
        }
    }

    /// Render a cell as a Ruby condition. A don't-care yields None (no condition).
    fn rb_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.rb_value(w),
                Lit::Num(n) => self.int_lit(n, inner, col_scale),
                Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => format!("{s:?}"),
            }
        };
        // A set is compared value by value. `[A, B].include?(x)` would do, but steep types
        // that literal as `Array[A | B]` and refuses `x`, whose type is the whole enum.
        let any_of = |ls: &Vec<Lit>, neg: bool| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.rb_value(m)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            let (op, join) = if neg { ("!=", " && ") } else { ("==", " || ") };
            format!("({})", out.iter().map(|v| format!("{var} {op} {v}")).collect::<Vec<_>>().join(join))
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!("{var}.nil?"),
            Cell::Prefix(ps) => ps
                .iter()
                .map(|p| format!("{var}.start_with?({})", format!("{p:?}")))
                .collect::<Vec<_>>()
                .join(" || "),
            // A cell naming one group calls the constant the module already declares,
            // rather than writing the members out again.
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                format!("GROUP_{}.include?({var})", self.rb_group(w))
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("!{var}"),
            Cell::Lit(l) => format!("{var} == {}", lit(l)),
            Cell::Set(ls) => any_of(ls, false),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                format!("!GROUP_{}.include?({var})", self.rb_group(w))
            }
            Cell::Not(ls) => any_of(ls, true),
            Cell::Cmp(cs) => cs
                .iter()
                .map(|(o, l)| {
                    let op = match o {
                        CmpOp::Le => "<=",
                        CmpOp::Ge => ">=",
                        CmpOp::Lt => "<",
                        CmpOp::Gt => ">",
                    };
                    format!("{var} {op} {}", lit(l))
                })
                .collect::<Vec<_>>()
                .join(" && "),
        })
    }
}

impl<'a> Gen<'a> {
    /// The module name: the rule's ASCII alias in PascalCase, like the Go package.
    fn rb_module(&self) -> String {
        rb_const(&pascal(&pub_name(&self.f.name)))
    }

    /// How a value's type is written for a reader. Ruby has no zero-cost brand, so the
    /// unit is documented rather than enforced (§15.20); `Integer` is exact at any size.
    fn rb_doc_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => rb_const(&self.enum_names.get(n).cloned().unwrap_or_else(|| "String".into())),
            Ty::Bool => "true/false".into(),
            Ty::Str => "String".into(),
            Ty::Opt(t) => format!("{} | nil", self.rb_doc_ty(t)),
            _ => format!("Integer  # {ty}"),
        }
    }

    pub fn ruby(&self) -> String {
        let mut o = self.header("#");
        o.insert_str(0, "# frozen_string_literal: true\n");
        o.push_str(&format!("module {} {}\n", self.rb_module(), "".trim()));
        // `module X` on its own line; the format above keeps it simple.
        o = o.replace(&format!("module {} \n", self.rb_module()), &format!("module {}\n", self.rb_module()));

        // Enums. A frozen constant per member, carrying the source name, which is also the
        // wire value (§10.2) — so the runner needs no conversion in either direction.
        let mut emitted: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if emitted.contains(ascii) {
                continue;
            }
            emitted.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            o.push_str(&format!("  module {}\n", rb_const(ascii)));
            let mut names: Vec<String> = Vec::new();
            for v in vals {
                let name = rb_const(
                    &self.value_names.get(v).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| v.clone()),
                );
                o.push_str(&format!("    {name} = {v:?}\n"));
                names.push(name);
            }
            o.push_str(&format!("    ALL = [{}].freeze\n  end\n\n", names.join(", ")));
        }

        o.push_str(&format!(
            "  # {}\n  #\n  # {}\n  class RuleInputError < ArgumentError\n\
             \x20   NO_VALUE = Object.new\n\n\
             \x20   attr_reader :what, :value\n\n\
             \x20   def initialize(what, value = NO_VALUE)\n\
             \x20     @what = what\n\
             \x20     @value = value\n\
             \x20     super(value.equal?(NO_VALUE) ? what : \"#{{what}}: #{{value.inspect}}\")\n\
             \x20   end\n  end\n\n",
            tr!("宣言した範囲の外。呼び出し側の契約違反。", "Outside the declared input domain: a contract violation by the caller."),
            tr!("文と値は別々に持つ（§15.95）。", "The sentence and the value travel apart (§15.95).")
        ));
        o.push_str(&format!(
            "  # {}\n  class RuleContradictionError < RuntimeError\n\
             \x20   attr_reader :what\n\n\
             \x20   def initialize(what)\n\
             \x20     @what = what\n\
             \x20     super\n\
             \x20   end\n  end\n\n",
            tr!("規則そのものの矛盾。呼び出し側の誤りではない。", "A contradiction in the rule itself, not a mistake by the caller.")
        ));
        o.push_str(&format!("  # {}\n  Fired = Struct.new(:table, :row, :label)\n\n", fired_doc()));

        // Groups. A Ruby constant has to begin with an uppercase ASCII letter, so a
        // Japanese group name cannot be one on its own; the prefix is what makes it legal.
        for g in &self.f.groups {
            let ms: Vec<String> = g.members.iter().map(|m| self.rb_value(&m.text)).collect();
            o.push_str(&format!("  GROUP_{} = [{}].freeze\n", self.rb_group(&g.name.text), ms.join(", ")));
        }
        if !self.f.groups.is_empty() {
            o.push('\n');
        }

        o.push_str(&round_rb());
        o.push('\n');
        o.push_str(&self.rb_fn());
        o.push('\n');
        o.push_str(&self.rb_record());
        o.push_str("end\n");
        o
    }

    /// The rule's items as Ruby, at the base indentation (§15.56).
    fn rb_items(&self, local: &dyn Fn(&str) -> String, trace: &str, phase: Phase) -> String {
        let mut o = String::new();
        for it in &self.f.items {
            if !self.in_phase(it, phase) {
                continue;
            }
            match it {
                Item::Derived(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!("    {} = {}  # {}\n", self.ident(&d.name.text), rb_expr(unparen(&e.text)), tr!("導出", "derived value")));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!("    {} = {}  # {}\n", self.ident(&d.name.text), rb_expr(unparen(&e.text)), tr!("定義", "definition")));
                }
                Item::Agg(_) => {}
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        o.push_str(&self.rb_table(t, local, trace));
                    }
                }
            }
        }
        o
    }

    /// The walk, in Ruby (§15.56).
    fn rb_walk(&self, fold: &crate::ast::FoldDecl, trace: &str) -> String {
        let el = self.f.elements.as_ref().expect("a fold has elements");
        let seq = pub_name(&el.name);
        let fields: BTreeMap<String, String> =
            el.fields.iter().map(|fd| (fd.name.text.clone(), pub_name(&fd.name))).collect();
        let (answer, stopped, taken, kept, best) = self.fold_locals();
        let e = self.temp("elem");
        let held = self.temp("held");
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => format!("{e}.{f}"),
                None if n == crate::kw::HELD => held.clone(),
                None => self.ident(n),
            }
        };
        let out_name = self.f.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
        let text = |x: &Option<Expr>| -> String {
            x.as_ref().map(|x| rb_expr(unparen(&self.expr(x, &local).text)).to_string()).unwrap_or_else(|| "0".to_string())
        };

        let mut o = String::new();
        o.push_str(&format!("    {answer} = {}  # {}\n", text(&fold.empty), tr!("要素ゼロ件の答え", "the answer for no elements")));
        o.push_str(&format!("    {stopped} = false\n"));
        // steep reads the first assignment as the whole type of a local, so a `nil` that
        // later holds a value needs the annotation (§15.23). The value is the rule's first
        // output; a `keep_max` key is ordered, so it is always an integer.
        let acc = self
            .f
            .outputs
            .first()
            .map(|od| self.rbs_ty(&self.ty_of(&od.name.text)))
            .unwrap_or_else(|| "untyped".into());
        o.push_str(&format!(
            "    {taken} = nil #: {acc}?\n    {kept} = nil #: {acc}?\n    {best} = nil #: Integer?\n"
        ));
        o.push_str(&format!("    {seq}.each do |{e}|\n"));

        let mut body = self.rb_element_guards(&local);
        body.push_str(&self.rb_items(&local, trace, Phase::All));
        let v = local(&fold.verdict);
        for (name, arm, _) in &fold.arms {
            let val = self.rb_value(&name.text);
            body.push_str(&format!("    if {v} == {val}  # {}\n", name.text));
            match arm {
                Arm::Next => body.push_str("      # next\n"),
                Arm::Stop(None) => body.push_str("      break\n"),
                Arm::Stop(Some(x)) => {
                    body.push_str(&format!("      {answer} = {}\n", rb_expr(unparen(&self.expr(x, &local).text))));
                    body.push_str(&format!("      {stopped} = true\n      break\n"));
                }
                Arm::Take { expr, unique } => {
                    if *unique {
                        body.push_str(&format!(
                            "      raise RuleContradictionError.new(\"{}\") unless {taken}.nil?\n",
                            tr!("畳み込み {}: take_unique に二件当たりました", "fold {}: two elements matched a take_unique", fold.verdict)
                        ));
                        body.push_str(&format!("      {taken} = {}\n", rb_expr(unparen(&self.expr(expr, &local).text))));
                    } else {
                        body.push_str(&format!("      {taken} = {} if {taken}.nil?\n", rb_expr(unparen(&self.expr(expr, &local).text))));
                    }
                }
                Arm::KeepMax { expr, key } => {
                    let k = self.temp("key");
                    body.push_str(&format!("      {k} = {}\n", rb_expr(unparen(&self.expr(key, &local).text))));
                    body.push_str(&format!("      if {best}.nil? || {k} > {best}\n"));
                    body.push_str(&format!("        {best} = {k}\n"));
                    body.push_str(&format!("        {kept} = {}\n      end\n", rb_expr(unparen(&self.expr(expr, &local).text))));
                }
            }
            body.push_str("    end\n");
        }
        o.push_str(&Self::indent_block(&body, "    "));
        o.push_str("    end\n");

        o.push_str(&format!("    unless {seq}.empty? || {stopped}\n"));
        o.push_str(&format!("      {held} = {taken} || {kept} || {}\n", text(&fold.empty)));
        o.push_str(&format!("      {answer} = {}\n    end\n", text(&fold.exhausted)));
        o.push_str(&format!("    {} = {answer}\n", self.ident(&out_name)));
        o
    }

    /// The entry guards of one element, inside the walk.
    /// The counting walk in Ruby (§15.58).
    fn rb_count_walk(&self, outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let el = self.f.elements.as_ref().expect("a count has elements");
        let seq = pub_name(&el.name);
        let fields: BTreeMap<String, String> =
            el.fields.iter().map(|fd| (fd.name.text.clone(), pub_name(&fd.name))).collect();
        let e = self.temp("elem");
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => format!("{e}.{f}"),
                None => self.ident(n),
            }
        };
        let mut o = String::new();
        if let Some(cap) = self.count_cap() {
            o.push_str(&format!(
                "    raise RuleInputError.new(\"{}\", {seq}.length) if {seq}.length > {cap}\n",
                tr!("{} の要素が多すぎます（上限 {cap}）", "{} has too many elements (at most {cap})", el.name.text)
            ));
        }
        for d in self.counts() {
            o.push_str(&format!("    {} = 0  # {}\n", self.ident(&d.name.text), tr!("数え上げ", "count")));
        }
        o.push_str(&format!("    {seq}.each do |{e}|\n"));
        let mut body = self.rb_element_guards(&local);
        body.push_str(&self.rb_items(&local, trace, Phase::Walk));
        for d in self.counts() {
            let v = local(&d.column.text);
            if d.kind == crate::ast::AggKind::Sum {
                body.push_str(&format!("    {} += {v}  # {}\n", self.ident(&d.name.text), d.name.text));
                continue;
            }
            let test = match self.count_member(d) {
                Some(w) => format!("{v} == {}", self.rb_value(&w.text)),
                None if self.count_negated(d) => format!("!{v}"),
                None => v,
            };
            body.push_str(&format!("    if {test}  # {}\n      {} += 1\n    end\n", d.name.text, self.ident(&d.name.text)));
        }
        o.push_str(&Self::indent_block(&body, "    "));
        o.push_str("    end\n");
        for (d, cap) in self.sum_caps() {
            let n = self.ident(&d.name.text);
            o.push_str(&format!(
                "    raise RuleInputError.new(\"{}\", {n}) if {n} > {cap}\n",
                tr!("{} が範囲の外です", "{} is out of range", d.name.text)
            ));
        }
        o.push_str(&self.rb_items(outer, trace, Phase::Main));
        o
    }

    fn rb_element_guards(&self, local: &dyn Fn(&str) -> String) -> String {
        let mut o = String::new();
        for i in self.element_fields() {
            let v = local(&i.name.text);
            let ty = self.ty_of(&i.name.text);
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                o.push_str(&format!(
                    "    raise RuleInputError.new(\"{}\", {v}) unless {v}.is_a?(Integer)\n",
                    tr!("{} が整数ではありません", "{} is not an integer", i.name.text)
                ));
                if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) {
                    let sc = self.c.wire_scale(&i.name.text);
                    o.push_str(&format!(
                        "    raise RuleInputError.new(\"{}\", {v}) unless ({}..{}).cover?({v})\n",
                        tr!("{} が範囲の外です", "{} is out of range", i.name.text),
                        crate::types::wire_int(lo, sc),
                        crate::types::wire_int(hi, sc),
                    ));
                }
            }
        }
        o
    }

    fn rb_fn(&self) -> String {
        let fname = pub_name(&self.f.name);
        let outs = &self.f.outputs;
        let mut o = String::new();

        // One element is a row of inputs, so it is a Struct like `Output`.
        if let Some(el) = &self.f.elements {
            let fs: Vec<String> = el.fields.iter().map(|fd| format!(":{}", pub_name(&fd.name))).collect();
            o.push_str(&format!("  # {}\n", tr!("{} の一件", "one of {}", el.name.text)));
            o.push_str(&format!("  Element = Struct.new({})\n\n", fs.join(", ")));
        }

        // Multiple outputs come back as a Struct; one output is the value itself (§8.5).
        if outs.len() > 1 {
            let fs: Vec<String> = outs.iter().map(|od| format!(":{}", pub_name(&od.name))).collect();
            o.push_str(&format!("  Output = Struct.new({})\n\n", fs.join(", ")));
        }

        // The signature, with each parameter's declared type as a comment: this is where
        // the unit lives, since Ruby cannot hold it (§15.20).
        let traced = format!("{fname}_traced");
        o.push_str(&format!("  # {}\n", plain_doc(&self.f.name.text, &self.f.version, &traced)));
        for i in &self.f.inputs {
            o.push_str(&format!(
                "  #   {} : {}\n",
                pub_name(&i.name),
                self.rb_doc_ty(&self.ty_of(&i.name.text)).replace("Integer  # ", "")
            ));
        }
        for od in outs {
            o.push_str(&format!(
                "  # -> {} : {}\n",
                pub_name(&od.name),
                self.rb_doc_ty(&self.ty_of(&od.name.text)).replace("Integer  # ", "")
            ));
        }

        // The public method keeps the plain shape; the branches live in its traced twin,
        // which also returns the rows that matched (§15.33).
        let mut params: Vec<String> = self.f.inputs.iter().map(|i| pub_name(&i.name)).collect();
        if let Some(el) = &self.f.elements {
            params.push(pub_name(&el.name));
        }
        o.push_str(&format!(
            "  def self.{fname}({})\n    {traced}({})[0]\n  end\n\n",
            params.join(", "),
            params.join(", ")
        ));
        o.push_str(&format!("  # {}\n", traced_doc(&self.f.name.text, &self.f.version)));
        o.push_str(&format!("  def self.{traced}({})\n", params.join(", ")));

        // Entry guards (§8.5).
        let local = |n: &str| -> String { self.ident(n) };
        for i in &self.f.inputs {
            let v = pub_name(&i.name);
            let ty = self.ty_of(&i.name.text);
            match &ty {
                Ty::Enum(n) => {
                    let cls = rb_const(&self.enum_names.get(n).cloned().unwrap_or_default());
                    o.push_str(&format!(
                        "    raise RuleInputError.new(\"{}\", {v}) unless {cls}::ALL.include?({v})\n",
                        tr!("{} が列挙 {} の値ではありません", "{} is not a value of enum {}", i.name.text, cls)
                    ));
                }
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                    // `cover?` is true of a Float inside the range, so the kind is checked
                    // first (§15.43).
                    o.push_str(&format!(
                        "    raise RuleInputError.new(\"{}\", {v}) unless {v}.is_a?(Integer)\n",
                        tr!("{} が整数ではありません", "{} is not an integer", i.name.text)
                    ));
                    if let Some((lo, hi)) = self.c.ranges.get(&i.name.text) {
                        if let (Some(lo), Some(hi)) = (lo, hi) {
                            let sc = self.c.wire_scale(&i.name.text);
                            o.push_str(&format!(
                                "    raise RuleInputError.new(\"{}\", {v}) unless ({}..{}).cover?({v})\n",
                                tr!("{} が範囲の外です", "{} is out of range", i.name.text),
                                crate::types::wire_int(*lo, sc),
                                crate::types::wire_int(*hi, sc),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }

        o.push_str(&self.constraint_guards(&local, Lang::Rb, "    ", |m| {
            format!("      raise RuleInputError.new(\"{m}\")\n")
        }));

        let trace = self.temp("trace");
        o.push_str(&format!("    {trace} = [] #: Array[Fired]\n"));

        match &self.f.fold {
            Some(fold) => o.push_str(&self.rb_walk(fold, &trace)),
            None if self.counts().is_empty() => o.push_str(&self.rb_items(&local, &trace, Phase::All)),
            None => o.push_str(&self.rb_count_walk(&local, &trace)),
        }

        // Every output: its source, brought to the wire scale, then its rounding once.
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let ty = self.ty_of(out_name);
            finals.push(match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os {
                        let raw = self.temp(&raw_base(oi));
                        o.push_str(&format!(
                            "    {raw} = {}  # {}\n",
                            rb_expr(unparen(&res.text)),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("_round_{}({raw}, {}) / {}", mode_fn(m), grid_i, res.scale / os)
                    } else {
                        format!("_round_{}({}, {})", mode_fn(m), rb_expr(&res.text), grid_i)
                    }
                }
                None => rb_expr(&res.text),
            });
        }
        if outs.len() == 1 {
            o.push_str(&format!("    [{}, {trace}]\n", finals[0]));
        } else {
            o.push_str(&format!("    [Output.new({}), {trace}]\n", finals.join(", ")));
        }
        o.push_str("  end\n");
        o
    }

    fn rb_table(&self, t: &Table, local: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = format!("    # {}\n", self.table_head(t));
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.rb_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "true".into() } else { conds.join(" && ") };
            let kw = if ri == 0 { "if" } else { "elsif" };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            o.push_str(&format!("    {kw} {cond}  # {}\n", self.row_head(t, ri, &cells.join(" | "))));
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let ty = self.ty_of(&oc.name.text);
                        self.int_lit(n, &ty, self.scale(&oc.name.text))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.rb_value(w),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        Lit::Str(x) => str_lit(x),
                        _ => "0".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "true".into()
                        } else if w == crate::kw::FALSE {
                            "false".into()
                        } else if self.value_names.contains_key(w) {
                            self.rb_value(w)
                        } else {
                            rb_expr(&self.rescaled(w, &oc.name.text, local(w)))
                        }
                    }
                    None => "0".into(),
                };
                o.push_str(&format!("      {} = {v}\n", self.ident(&oc.name.text)));
            }
            let (tn, rn, lbl) = self.fired_of(t, ri);
            if lbl.is_empty() {
                o.push_str(&format!("      {trace} << Fired.new({tn:?}, {rn})\n"));
            } else {
                o.push_str(&format!("      {trace} << Fired.new({tn:?}, {rn}, {lbl:?})\n"));
            }
        }
        o.push_str(&format!(
            "    else\n      raise RuleContradictionError.new(\"{}\")\n    end\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        o.push_str(&self.guards(t, local, Lang::Rb, "    ", |name, i, j| {
            format!(
                "      raise RuleContradictionError.new(\"{}\")\n",
                tr!("表 {name}: {i} と {j} が同時に当てはまりました", "table {name}: {i} and {j} matched at the same time")
            )
        }));
        o
    }
}

impl<'a> Gen<'a> {
    /// A Ruby runner that reads JSONL from stdin and prints just the outputs, one record
    /// per line. `json` and `date` are both standard library, so this needs no gem.
    pub fn ruby_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let m = self.rb_module();
        let mut args: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let jp = &i.name.text;
            args.push(match &self.ty_of(&i.name.text) {
                // An enum member is the source string, which is what the wire carries.
                Ty::Enum(_) | Ty::Str => format!("d[{jp:?}]"),
                Ty::Bool => format!("d[{jp:?}] ? true : false"),
                Ty::Date => format!("_ord(d[{jp:?}])"),
                // `null` on the wire is `nil` here. It used to fall to `.to_i`, which turns
                // `nil` into 0 — a value no row of the table matches, so the module raised
                // "unreachable" on a case it answers perfectly well (§15.88).
                Ty::Opt(_) => format!("d[{jp:?}]"),
                _ => format!("d[{jp:?}].to_i"),
            });
        }
        // The sequence a walk reads, one element at a time (§15.56).
        if let Some(el) = &self.f.elements {
            let jp = &el.name.text;
            let fields: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let k = &fd.name.text;
                    match &self.ty_of(k) {
                        Ty::Enum(_) | Ty::Str => format!("e[{k:?}]"),
                        Ty::Bool => format!("e[{k:?}] ? true : false"),
                        Ty::Date => format!("_ord(e[{k:?}])"),
                        Ty::Opt(_) => format!("e[{k:?}]"),
                        _ => format!("e[{k:?}].to_i"),
                    }
                })
                .collect();
            args.push(format!(
                "d[{jp:?}].map {{ |e| {m}::Element.new({}) }}",
                fields.join(", ")
            ));
        }
        format!(
            "# Code generated by rulec {}. DO NOT EDIT.\n\
             # frozen_string_literal: true\n\n\
             require \"date\"\n\
             require \"json\"\n\
             require_relative \"{alias}\"\n\n\
             def _ord(s)\n  \
               Date.iso8601(s).jd - Date.new(1970, 1, 1).jd\n\
             end\n\n\
             STDIN.each_line do |line|\n  \
               line = line.strip\n  \
               next if line.empty?\n  \
               d = JSON.parse(line)[\"in\"]\n  \
               args = [{}]\n  \
               r, trace = {m}.{alias}_traced(*args)\n  \
               puts {m}.{alias}_record(*args, r, trace)\n\
             end\n",
            env!("CARGO_PKG_VERSION"),
            args.join(", ")
        )
    }
}

/// The five modes of §7.3 in Ruby, checked against the same reference cases the other
/// others are checked against. `rulec test` runs it beside the generated module.
pub fn round_tests_ruby() -> String {
    let mut o = format!(
        "# Code generated by rulec {}. DO NOT EDIT.\n\
         # frozen_string_literal: true\n\
         # {}\n\n\
         module R\n",
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の五モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five modes of §7.3, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(round_rb().trim_start_matches('\n'));
    o.push_str("  public_class_method :_round_down, :_round_up, :_round_half, :_round_half_down, :_round_bankers\nend\n\nCASES = [\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("  [{:?}, {x}, {g}, {want}],\n", mode_fn(m)));
    }
    o.push_str("].freeze\n\nbad = 0\nCASES.each do |mode, x, g, want|\n  ");
    o.push_str("got = R.send(\"_round_#{mode}\", x, g)\n  next if got == want\n  ");
    o.push_str("puts \"NG #{mode}(#{x}, #{g}) = #{got}, want #{want}\"\n  bad += 1\nend\n");
    o.push_str("exit(1) unless bad.zero?\n");
    o.push_str(&format!("puts \"{}\"\n", tr!("ok #{{CASES.size}} 件", "ok #{{CASES.size}} cases")));
    o
}

impl<'a> Gen<'a> {
    /// The type as the inventory states it for Ruby. There are no brands here (§15.20), so
    /// a number is `Integer` and the unit travels in the entry's own `unit` field.
    fn rb_api_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => rb_const(&self.enum_names.get(n).cloned().unwrap_or_else(|| "String".into())),
            Ty::Bool => "Boolean".into(),
            Ty::Str => "String".into(),
            Ty::Opt(t) => format!("{} | nil", self.rb_api_ty(t)),
            _ => "Integer".into(),
        }
    }
}

impl<'a> Gen<'a> {
    /// A cell as a condition in one language. The one place that maps a `Lang` to its cell
    /// renderer, so that shared code never has to know the set (§15.20).
    fn cell(&self, lang: Lang, cell: &Cell, var: &str, ty: &Ty, scale: i128) -> Option<String> {
        match lang {
            Lang::Py => self.py_cell(cell, var, ty, scale),
            Lang::Go => self.go_cell(cell, var, ty, scale),
            Lang::Ts => self.ts_cell(cell, var, ty, scale),
            Lang::Rs => self.rs_cell(cell, var, ty, scale),
            Lang::Rb => self.rb_cell(cell, var, ty, scale),
            Lang::Sw => self.sw_cell(cell, var, ty, scale),
            Lang::Sql => self.sql_cell(cell, var, ty, scale),
            Lang::Php => self.php_cell(cell, var, ty, scale),
            Lang::Java => self.java_cell(cell, var, ty, scale),
        }
    }
}

impl<'a> Gen<'a> {
    /// The RBS type name for a value. There is no newtype in RBS either, so a unit is not
    /// expressible and every number is `Integer` (§15.23) — but an enum **is**, as the
    /// union of its own values, which closes the set at the type level.
    fn rbs_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => self
                .enum_names
                .get(n)
                .map(|a| snake(a))
                .unwrap_or_else(|| "String".into()),
            Ty::Bool => "bool".into(),
            Ty::Str => "String".into(),
            Ty::Opt(t) => format!("{}?", self.rbs_ty(t)),
            _ => "Integer".into(),
        }
    }

    /// The signature file that ships beside the generated Ruby. `steep` reads it and
    /// refuses a caller that passes the wrong kind, the wrong count, or a string that is
    /// not one of an enum's values. It cannot refuse grams where yen were meant — RBS has
    /// no newtype, and a type alias is the same type (§15.23).
    pub fn rbs(&self) -> String {
        let m = self.rb_module();
        let mut o = self.header("#");
        o.push_str(&format!("\nmodule {m}\n"));

        // Enums, as a union of the values themselves.
        let mut emitted: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if emitted.contains(ascii) {
                continue;
            }
            emitted.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            let lits: Vec<String> = vals.iter().map(|v| format!("{v:?}")).collect();
            o.push_str(&format!("  type {} = {}\n\n", snake(ascii), lits.join(" | ")));
            o.push_str(&format!("  module {}\n", rb_const(ascii)));
            let mut names: Vec<String> = Vec::new();
            for v in vals {
                let name = rb_const(
                    &self.value_names.get(v).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| v.clone()),
                );
                o.push_str(&format!("    {name}: {v:?}\n"));
                names.push(name);
            }
            o.push_str(&format!("    ALL: Array[{}]\n  end\n\n", snake(ascii)));
        }

        o.push_str(
            "  class RuleInputError < ArgumentError\n    NO_VALUE: Object\n    attr_reader what: String\n    \
             attr_reader value: untyped\n    def initialize: (String, ?untyped) -> void\n  end\n\n",
        );
        o.push_str(
            "  class RuleContradictionError < RuntimeError\n    attr_reader what: String\n    \
             def initialize: (String) -> void\n  end\n\n",
        );

        for g in &self.f.groups {
            let el = self
                .c
                .groups
                .get(&g.name.text)
                .map(|(ty, _)| self.rbs_ty(&Ty::Enum(ty.clone())))
                .unwrap_or_else(|| "String".into());
            o.push_str(&format!("  GROUP_{}: Array[{el}]\n", self.rb_group(&g.name.text)));
        }
        if !self.f.groups.is_empty() {
            o.push('\n');
        }

        let outs = &self.f.outputs;
        if outs.len() > 1 {
            let tys: Vec<String> = outs.iter().map(|od| self.rbs_ty(&self.ty_of(&od.name.text))).collect();
            o.push_str("  class Output < Struct[untyped]\n");
            for (od, t) in outs.iter().zip(&tys) {
                o.push_str(&format!("    attr_reader {}: {t}\n", pub_name(&od.name)));
            }
            o.push_str(&format!("    def self.new: ({}) -> Output\n  end\n\n", tys.join(", ")));
        }

        // One element is a row of inputs, and the Ruby module makes it a Struct; without
        // this the signature has no name for what the caller passes (§15.56).
        if let Some(el) = &self.f.elements {
            let tys: Vec<String> =
                el.fields.iter().map(|fd| self.rbs_ty(&self.ty_of(&fd.name.text))).collect();
            o.push_str("  class Element < Struct[untyped]\n");
            for (fd, t) in el.fields.iter().zip(&tys) {
                o.push_str(&format!("    attr_reader {}: {t}\n", pub_name(&fd.name)));
            }
            o.push_str(&format!("    def self.new: ({}) -> Element\n  end\n\n", tys.join(", ")));
        }

        o.push_str(
            "  class Fired < Struct[untyped]\n    attr_reader table: String\n    attr_reader row: Integer\n    attr_reader label: String?\n    def self.new: (String, Integer, ?String) -> Fired\n  end\n\n",
        );

        // The rounding helpers are private at run time; declaring them keeps `steep` from
        // warning about a method the module defines and the signature does not mention.
        o.push_str(&format!("  # {}\n", tr!("丸めの補助。実行時は私有。", "The rounding helpers, private at run time.")));
        for h in ["_min", "_max", "_round_down", "_round_up", "_round_half", "_round_half_down", "_round_bankers"] {
            o.push_str(&format!("  private def self.{h}: (Integer, Integer) -> Integer\n"));
        }
        o.push('\n');

        let params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{} {}", self.rbs_ty(&self.ty_of(&i.name.text)), pub_name(&i.name)))
            .chain(self.f.elements.iter().map(|el| format!("Array[Element] {}", pub_name(&el.name))))
            .collect();
        let ret = if outs.len() == 1 {
            self.rbs_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        o.push_str(&format!("  def self.{}: ({}) -> {ret}\n", pub_name(&self.f.name), params.join(", ")));
        o.push_str(&format!("  def self.{}_traced: ({}) -> [{ret}, Array[Fired]]\n", pub_name(&self.f.name), params.join(", ")));
        let (out, trace, tag) = (self.temp("out"), self.temp("trace"), self.temp("tag"));
        o.push_str(&format!(
            "  def self.{}_record: ({}, {ret} {out}, Array[Fired] {trace}, ?String {tag}) -> String\n",
            pub_name(&self.f.name),
            params.join(", ")
        ));
        o.push_str("  private def self._json_str: (String) -> String\n");
        if civil_needed(self.f, self.c) {
            o.push_str("  private def self._civil: (Integer) -> String\n");
        }
        o.push_str("end\n");
        o
    }
}

/// PascalCase to snake_case, for an RBS type alias — those have to start lowercase.
fn snake(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                o.push('_');
            }
            o.push(c.to_ascii_lowercase());
        } else {
            o.push(c);
        }
    }
    o
}

// ---------------------------------------------------------------------------
// Swift (§15.20). The second target after Rust that can hold a unit in the type: a
// single-field struct over `Int64` is laid out as the integer itself, so a compiler that
// refuses `YenExclTax` where `YenInclTax` was meant costs nothing at run time. Integer
// division truncates toward zero, as it does in Go, in Rust and on a JavaScript bigint, so
// the rounding helpers are those transliterated rather than Python's.
//
// Two spellings are Swift's own. Identifiers are lowerCamelCase — the convention its
// linters enforce, and the same reason the Go backend writes `pascal` — and a name that
// lands on a keyword goes in backticks, which is the one shape a rule can declare
// (`alias: in`) that no other target has to escape.

/// The reserved words that have to be written in backticks.
const SWIFT_KEYWORDS: &[&str] = &[
    "Any", "Protocol", "Self", "Type", "as", "associatedtype", "await", "break", "case",
    "catch", "class", "continue", "default", "defer", "deinit", "do", "else", "enum",
    "extension", "fallthrough", "false", "fileprivate", "for", "func", "guard", "if",
    "import", "in", "init", "inout", "internal", "is", "let", "nil", "operator", "private",
    "protocol", "public", "repeat", "rethrows", "return", "self", "static", "struct",
    "subscript", "super", "switch", "throw", "throws", "true", "try", "typealias", "var",
    "where", "while",
];

/// Lower the leading run of capitals: all of it, unless the run is longer than one and the
/// character after it is lowercase, in which case that last capital begins the next word.
fn lower_lead(w: &str) -> String {
    let cs: Vec<char> = w.chars().collect();
    let run = cs.iter().take_while(|c| c.is_uppercase()).count();
    let keep = if run > 1 && cs.get(run).is_some_and(|c| c.is_lowercase()) { run - 1 } else { run };
    let mut out = String::with_capacity(w.len());
    for (i, c) in cs.iter().enumerate() {
        if i < keep {
            out.extend(c.to_lowercase());
        } else {
            out.push(*c);
        }
    }
    out
}

/// To lowerCamelCase, the way Swift's own guidelines do it: `Basic` → `basic`, `EUR` →
/// `eur`, `URLSession` → `urlSession`, `pay_rate` → `payRate`. A name with no capitals and
/// no underscores comes back unchanged, so an alias already written in Swift's spelling is
/// left alone.
///
/// Leading and trailing underscores survive, and that is not cosmetic: [`Gen::temp`] makes
/// a generated temporary unique by appending one, so a `camel` that dropped it would hand
/// `raw_` and `raw` the same Swift identifier — the collision `temp` exists to prevent.
fn camel(s: &str) -> String {
    let lead = s.len() - s.trim_start_matches('_').len();
    let end = s.trim_end_matches('_').len();
    if end <= lead {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    out.push_str(&s[..lead]);
    for (i, w) in s[lead..end].split('_').filter(|p| !p.is_empty()).enumerate() {
        if i == 0 {
            out.push_str(&lower_lead(w));
            continue;
        }
        let mut ch = w.chars();
        if let Some(c) = ch.next() {
            out.extend(c.to_uppercase());
            out.push_str(ch.as_str());
        }
    }
    out.push_str(&s[end..]);
    out
}

/// A declared name as Swift writes a value: lowerCamelCase, in backticks when that is one
/// of the language's own words.
fn sw_name(s: &str) -> String {
    let c = camel(s);
    if SWIFT_KEYWORDS.contains(&c.as_str()) {
        format!("`{c}`")
    } else {
        c
    }
}

/// The same name as an **argument label at a call site**, where the rule is the other way
/// around: Swift takes a keyword there as it stands, and warns that the backticks were not
/// needed. `inout` is the one that still has to be escaped, because it is also a parameter
/// modifier and the parser cannot tell the two apart.
fn sw_label(s: &str) -> String {
    let c = camel(s);
    if c == "inout" {
        format!("`{c}`")
    } else {
        c
    }
}

/// The helper for one rounding mode, named as the generated module spells it.
fn sw_round_fn(m: RoundMode) -> String {
    format!("_round{}", pascal(mode_fn(m)))
}

/// Turn the shared expression text into Swift. The same post-processing shape as `go_expr`,
/// `ts_expr` and `rs_expr` — and the `//` substitution is load-bearing twice over here,
/// since in Swift it would otherwise start a comment and swallow the rest of the line.
///
/// `_min` and `_max` stay our own rather than becoming the standard library's `min` and
/// `max`. A rule may declare an input named `min` — `クーポン割引.rule` does — and a
/// parameter shadows a global in Swift, so the call would resolve to the money the caller
/// passed in.
/// The Swift twin of [`go_typed`]: a bare literal would be an `Int`, not an `Int64`.
fn sw_typed(s: &str) -> String {
    if s.parse::<i128>().is_ok() { format!("Int64({s})") } else { s.to_string() }
}

fn sw_expr(s: &str) -> String {
    s.replace(" // ", " / ")
        .replace("True", "true")
        .replace("False", "false")
        .replace("_round_down(", "_roundDown(")
        .replace("_round_up(", "_roundUp(")
        .replace("_round_half_down(", "_roundHalfDown(")
        .replace("_round_half(", "_roundHalf(")
        .replace("_round_bankers(", "_roundBankers(")
}

/// The error type and the five modes of §7.3. `private` at file scope in Swift is what
/// `fileprivate` is, so the same text serves the generated module — where nothing outside
/// may call these — and `_round_test.swift`, where the cases sit in the same file.
fn round_sw() -> String {
    format!(
        r#"
/// {err}
public enum RuleError: Error, CustomStringConvertible {{
    /// {input}
    case input(what: String, value: Int64?)
    /// {contra}
    case contradiction(what: String)

    public var description: String {{
        switch self {{
        case .input(let what, .some(let v)): return "\(what): \(v)"
        case .input(let what, .none), .contradiction(let what): return what
        }}
    }}
}}

private func _min(_ a: Int64, _ b: Int64) -> Int64 {{
    a < b ? a : b
}}

private func _max(_ a: Int64, _ b: Int64) -> Int64 {{
    a > b ? a : b
}}

/// {down}
private func _roundDown(_ x: Int64, _ g: Int64) -> Int64 {{
    let v = abs(x) / g * g
    return x < 0 ? -v : v
}}

/// {up}
private func _roundUp(_ x: Int64, _ g: Int64) -> Int64 {{
    let a = abs(x)
    let v = a % g == 0 ? a / g * g : (a / g + 1) * g
    return x < 0 ? -v : v
}}

/// {half}
private func _roundHalf(_ x: Int64, _ g: Int64) -> Int64 {{
    let a = abs(x)
    let v = 2 * (a % g) >= g ? (a / g + 1) * g : a / g * g
    return x < 0 ? -v : v
}}

/// {half_down}
private func _roundHalfDown(_ x: Int64, _ g: Int64) -> Int64 {{
    let a = abs(x)
    let v = 2 * (a % g) > g ? (a / g + 1) * g : a / g * g
    return x < 0 ? -v : v
}}

/// {bankers}
private func _roundBankers(_ x: Int64, _ g: Int64) -> Int64 {{
    let a = abs(x)
    var q = a / g
    let r = a % g
    if 2 * r > g || (2 * r == g && q % 2 == 1) {{
        q += 1
    }}
    let v = q * g
    return x < 0 ? -v : v
}}
"#,
        err = tr!("この規則が返しうる誤り。", "Everything this rule can go wrong with."),
        input = tr!("宣言した範囲の外。呼び出し側の契約違反。", "Outside the declared input domain: a contract violation by the caller."),
        contra = tr!("規則そのものの矛盾。呼び出し側の誤りではない。", "A contradiction in the rule itself, not a mistake by the caller."),
        down = tr!("0 へ寄せる。-4.8円 → -4円。", "Toward zero: -4.8 yen -> -4 yen."),
        up = tr!("0 から遠ざける。-4.2円 → -5円。", "Away from zero: -4.2 yen -> -5 yen."),
        half = tr!("半分ちょうどは 0 から遠ざける。", "An exact half goes away from zero."),
        half_down = tr!("半分ちょうどは 0 へ寄せる。", "An exact half goes toward zero."),
        bankers = tr!("半分ちょうどは偶数へ。", "An exact half goes to the even neighbor."),
    )
}

impl<'a> Gen<'a> {
    /// The Swift type for a value. A unit is a struct over `Int64`, which is what the
    /// overflow proof (E108) is stated in — `Int` is the platform's word and only
    /// happens to be 64 bits.
    fn sw_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => self.enum_names.get(n).cloned().unwrap_or_else(|| "String".into()),
            Ty::Bool => "Bool".into(),
            Ty::Str => "String".into(),
            Ty::Number | Ty::Date => "Int64".into(),
            Ty::Opt(t) => format!("{}?", self.sw_ty(t)),
            _ => brand_of(ty),
        }
    }

    /// The identifier a declared name gets, in Swift's spelling.
    fn sw_ident(&self, n: &str) -> String {
        sw_name(&self.ident(n))
    }

    /// The name of the set a group is declared as. The underscore goes on *before* the
    /// keyword escaping: `_` + `` `internal` `` is not an identifier, and `_internal` needs
    /// no backticks in the first place.
    fn sw_group(&self, n: &str) -> String {
        sw_name(&format!("_{}", self.ident(n)))
    }

    /// A blank assignment for a value nothing downstream reads, the same device the Go
    /// backend needs. The value is still computed, so the generated code and the rule stay
    /// line for line, but Swift warns about a binding that is never read — and the module
    /// says DO NOT EDIT, so nobody can quiet the warning afterwards. W111 has already named
    /// the declaration.
    fn sw_unread(&self, name: &str) -> String {
        if self.is_read(name) {
            String::new()
        } else {
            format!("    _ = {}\n", self.sw_ident(name))
        }
    }

    fn sw_value(&self, v: &str) -> String {
        match self.value_names.get(v) {
            Some((ty, alias)) => format!("{ty}.{}", sw_name(alias)),
            None => format!("{v:?}"),
        }
    }

    /// Render a cell as a Swift condition. A don't-care yields None (no condition).
    fn sw_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.sw_value(w),
                Lit::Num(n) => self.int_lit(n, inner, col_scale),
                Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => format!("{s:?}"),
            }
        };
        let members = |ls: &Vec<Lit>| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.sw_value(m)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            format!("[{}]", out.join(", "))
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!("{var} == nil"),
            Cell::Prefix(ps) => ps
                .iter()
                .map(|p| format!("{var}.hasPrefix({})", format!("{p:?}")))
                .collect::<Vec<_>>()
                .join(" || "),
            // A cell naming one group uses the set the module already declares, rather than
            // writing the members out a second time.
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                format!("{}.contains({var})", self.sw_group(w))
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("!{var}"),
            Cell::Lit(l) => format!("{var} == {}", lit(l)),
            Cell::Set(ls) => format!("{}.contains({var})", members(ls)),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                format!("!{}.contains({var})", self.sw_group(w))
            }
            Cell::Not(ls) => format!("!{}.contains({var})", members(ls)),
            Cell::Cmp(cs) => cs
                .iter()
                .map(|(o, l)| {
                    let op = match o {
                        CmpOp::Le => "<=",
                        CmpOp::Ge => ">=",
                        CmpOp::Lt => "<",
                        CmpOp::Gt => ">",
                    };
                    format!("{var} {op} {}", lit(l))
                })
                .collect::<Vec<_>>()
                .join(" && "),
        })
    }

    pub fn swift(&self) -> String {
        let mut o = self.header("//");
        o.push('\n');

        // Brands. A struct with one stored property is laid out as that property, so the
        // type costs nothing at run time — the same bargain the Rust backend makes.
        let mut brands: BTreeMap<String, String> = BTreeMap::new();
        for v in self.f.inputs.iter().map(|i| &i.name.text).chain(self.f.outputs.iter().map(|o| &o.name.text)) {
            let ty = self.ty_of(v);
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                brands.insert(brand_of(&ty), format!("{ty}"));
            }
        }
        for (b, doc) in &brands {
            o.push_str(&format!(
                "/// {doc}\n\
                 public struct {b}: Hashable, Comparable, Sendable {{\n    \
                     public var value: Int64\n\n    \
                     public init(_ value: Int64) {{ self.value = value }}\n\n    \
                     public static func < (lhs: {b}, rhs: {b}) -> Bool {{ lhs.value < rhs.value }}\n\
                 }}\n\n"
            ));
        }
        o.push_str(&format!(
            "/// {}\npublic struct Fired: Hashable, Sendable {{\n    public let table: String\n    public let row: Int\n    public let label: String\n\n    \
             public init(table: String, row: Int, label: String = \"\") {{\n        self.table = table\n        self.row = row\n        self.label = label\n    }}\n}}\n\n",
            fired_doc()
        ));

        // Enums. The raw value is the source name, which is also what the wire carries
        // (§10.2), so `rawValue` and `init(rawValue:)` are the whole conversion and no
        // hand-written parser is needed on either side.
        let mut emitted: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if emitted.contains(ascii) {
                continue;
            }
            emitted.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            o.push_str(&format!("public enum {ascii}: String, CaseIterable, Sendable {{\n"));
            for v in vals {
                let name = self
                    .value_names
                    .get(v)
                    .map(|(_, a)| sw_name(a))
                    .unwrap_or_else(|| sw_name(v));
                o.push_str(&format!("    case {name} = {v:?}\n"));
            }
            o.push_str("}\n\n");
        }

        o.push_str(round_sw().trim_start_matches('\n'));
        o.push('\n');

        // Groups. A `Set` needs Hashable, which a raw-value enum already is.
        for g in &self.f.groups {
            let ty = self
                .c
                .groups
                .get(&g.name.text)
                .and_then(|(owner, _)| self.enum_names.get(owner).cloned())
                .unwrap_or_else(|| "Int64".into());
            let ms: Vec<String> = g.members.iter().map(|m| self.sw_value(&m.text)).collect();
            o.push_str(&format!(
                "private let {}: Set<{ty}> = [{}]\n",
                self.sw_group(&g.name.text),
                ms.join(", ")
            ));
        }
        if !self.f.groups.is_empty() {
            o.push('\n');
        }

        o.push_str(&self.sw_fn());
        o.push('\n');
        o.push_str(&self.sw_record());
        o
    }

    /// The rule's items as Swift, at the base indentation (§15.56).
    fn sw_items(&self, local: &dyn Fn(&str) -> String, trace: &str, phase: Phase) -> String {
        let mut o = String::new();
        for it in &self.f.items {
            if !self.in_phase(it, phase) {
                continue;
            }
            match it {
                Item::Derived(d) => {
                    o.push_str(&format!(
                        "    let {} = {}  // {}\n",
                        self.sw_ident(&d.name.text),
                        sw_typed(&sw_expr(unparen(&self.expr(&d.expr, local).text))),
                        tr!("導出", "derived value")
                    ));
                    o.push_str(&self.sw_unread(&d.name.text));
                }
                Item::Define(d) => {
                    o.push_str(&format!(
                        "    let {} = {}  // {}\n",
                        self.sw_ident(&d.name.text),
                        sw_typed(&sw_expr(unparen(&self.expr(&d.expr, local).text))),
                        tr!("定義", "definition")
                    ));
                    o.push_str(&self.sw_unread(&d.name.text));
                }
                Item::Agg(_) => {}
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        o.push_str(&self.sw_table(t, local, trace));
                    }
                }
            }
        }
        o
    }

    /// The walk, in Swift (§15.56).
    fn sw_walk(&self, fold: &crate::ast::FoldDecl, trace: &str) -> String {
        let el = self.f.elements.as_ref().expect("a fold has elements");
        let seq = sw_name(&pub_name(&el.name));
        let fields: BTreeMap<String, String> =
            el.fields.iter().map(|fd| (fd.name.text.clone(), sw_name(&pub_name(&fd.name)))).collect();
        let (answer, stopped, taken, kept, best) = self.fold_locals();
        let (answer, stopped, taken, kept, best) =
            (sw_name(&answer), sw_name(&stopped), sw_name(&taken), sw_name(&kept), sw_name(&best));
        let e = sw_name(&self.temp("elem"));
        let held = sw_name(&self.temp("held"));
        // Inside the walk a field is read off the element, and `held` is the accumulator.
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => {
                    let v = format!("{e}.{f}");
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                        format!("{v}.value")
                    } else {
                        v
                    }
                }
                None if n == crate::kw::HELD => held.clone(),
                None => self.sw_ident(n),
            }
        };
        let out_name = self.f.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
        let out_ty = self.ty_of(&out_name);
        let acc_ty = if out_ty.is_numeric() { "Int64".to_string() } else { self.sw_ty(&out_ty) };
        let text = |x: &Option<Expr>| -> String {
            x.as_ref().map(|x| sw_expr(unparen(&self.expr(x, &local).text))).unwrap_or_else(|| "0".to_string())
        };
        let takes = fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::Take { .. }));
        let keeps = fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::KeepMax { .. }));

        let mut o = String::new();
        o.push_str(&format!("    var {answer}: {acc_ty} = {}  // {}\n", text(&fold.empty), tr!("要素ゼロ件の答え", "the answer for no elements")));
        o.push_str(&format!("    var {stopped} = false\n"));
        if takes {
            o.push_str(&format!("    var {taken}: {acc_ty}? = nil\n"));
        }
        if keeps {
            o.push_str(&format!("    var {kept}: {acc_ty}? = nil\n    var {best}: Int64? = nil\n"));
        }
        o.push_str(&format!("    for {e} in {seq} {{\n"));

        let mut body = self.sw_element_guards(&local);
        body.push_str(&self.sw_items(&local, trace, Phase::All));
        // The verdict the table wrote for this element, and what the walk does about it.
        let v = local(&fold.verdict);
        for (name, arm, _) in &fold.arms {
            body.push_str(&format!("    if {v} == {} {{  // {}\n", self.sw_value(&name.text), name.text));
            match arm {
                Arm::Next => body.push_str(&format!("        // {}\n", crate::kw::NEXT)),
                Arm::Stop(None) => body.push_str("        break\n"),
                Arm::Stop(Some(x)) => {
                    body.push_str(&format!("        {answer} = {}\n", sw_expr(unparen(&self.expr(x, &local).text))));
                    body.push_str(&format!("        {stopped} = true\n        break\n"));
                }
                Arm::Take { expr, unique } => {
                    if *unique {
                        body.push_str(&format!("        if {taken} != nil {{\n"));
                        body.push_str(&format!(
                            "            throw RuleError.contradiction(what: \"{}\")\n        }}\n",
                            tr!(
                                "畳み込み {}: take_unique に二件当たりました",
                                "fold {}: two elements matched a take_unique",
                                fold.verdict
                            )
                        ));
                        body.push_str(&format!("        {taken} = {}\n", sw_expr(unparen(&self.expr(expr, &local).text))));
                    } else {
                        body.push_str(&format!("        if {taken} == nil {{\n"));
                        body.push_str(&format!("            {taken} = {}\n        }}\n", sw_expr(unparen(&self.expr(expr, &local).text))));
                    }
                }
                Arm::KeepMax { expr, key } => {
                    let k = sw_name(&self.temp("key"));
                    body.push_str(&format!("        let {k} = {}\n", sw_expr(unparen(&self.expr(key, &local).text))));
                    body.push_str(&format!("        if {best} == nil || {k} > {best}! {{\n"));
                    body.push_str(&format!("            {best} = {k}\n"));
                    body.push_str(&format!("            {kept} = {}\n        }}\n", sw_expr(unparen(&self.expr(expr, &local).text))));
                }
            }
            body.push_str("    }\n");
        }
        o.push_str(&Self::indent_block(&body, "    "));
        o.push_str("    }\n");

        // A sequence with nothing in it answers `empty` and never reaches `exhausted`; a walk
        // that ended on `stop with` has its answer already.
        o.push_str(&format!("    if !{seq}.isEmpty && !{stopped} {{\n"));
        let mut fallback = text(&fold.empty);
        if keeps {
            fallback = format!("({kept} ?? {fallback})");
        }
        if takes {
            fallback = format!("({taken} ?? {fallback})");
        }
        o.push_str(&format!("        let {held}: {acc_ty} = {fallback}\n"));
        let ex = text(&fold.exhausted);
        if !ex.contains(&held) {
            o.push_str(&format!("        _ = {held}\n"));
        }
        o.push_str(&format!("        {answer} = {ex}\n    }}\n"));
        o.push_str(&format!("    let {} = {answer}\n", self.sw_ident(&out_name)));
        o.push_str(&self.sw_unread(&out_name));
        o
    }

    /// The entry guards of one element, inside the walk. An enum needs none, for the reason
    /// the inputs' own guards give.
    /// The counting walk in Swift (§15.58).
    fn sw_count_walk(&self, outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let el = self.f.elements.as_ref().expect("a count has elements");
        let seq = sw_name(&pub_name(&el.name));
        let fields: BTreeMap<String, String> =
            el.fields.iter().map(|fd| (fd.name.text.clone(), sw_name(&pub_name(&fd.name)))).collect();
        let e = self.temp("elem");
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => {
                    let v = format!("{e}.{f}");
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                        format!("{v}.value")
                    } else {
                        v
                    }
                }
                None => self.sw_ident(n),
            }
        };
        let mut o = String::new();
        if let Some(cap) = self.count_cap() {
            o.push_str(&format!(
                "    if {seq}.count > {cap} {{\n        throw RuleError.input(what: \"{}\", value: Int64({seq}.count))\n    }}\n",
                tr!("{} の要素が多すぎます（上限 {cap}）", "{} has too many elements (at most {cap})", el.name.text)
            ));
        }
        for d in self.counts() {
            o.push_str(&format!("    var {}: Int64 = 0  // {}\n", self.sw_ident(&d.name.text), tr!("数え上げ", "count")));
        }
        o.push_str(&format!("    for {e} in {seq} {{\n"));
        let mut body = self.sw_element_guards(&local);
        body.push_str(&self.sw_items(&local, trace, Phase::Walk));
        for d in self.counts() {
            let v = local(&d.column.text);
            if d.kind == crate::ast::AggKind::Sum {
                body.push_str(&format!("    {} += {v}  // {}\n", self.sw_ident(&d.name.text), d.name.text));
                continue;
            }
            let test = match self.count_member(d) {
                Some(w) => format!("{v} == {}", self.sw_value(&w.text)),
                None if self.count_negated(d) => format!("!{v}"),
                None => v,
            };
            body.push_str(&format!("    if {test} {{  // {}\n        {} += 1\n    }}\n", d.name.text, self.sw_ident(&d.name.text)));
        }
        o.push_str(&Self::indent_block(&body, "    "));
        o.push_str("    }\n");
        for (d, cap) in self.sum_caps() {
            let n = self.sw_ident(&d.name.text);
            o.push_str(&format!(
                "    if {n} > {cap} {{\n        throw RuleError.input(what: \"{}\", value: Int64({n}))\n    }}\n",
                tr!("{} が範囲の外です", "{} is out of range", d.name.text)
            ));
        }
        o.push_str(&self.sw_items(outer, trace, Phase::Main));
        o
    }

    fn sw_element_guards(&self, local: &dyn Fn(&str) -> String) -> String {
        let mut o = String::new();
        for i in self.element_fields() {
            let ty = self.ty_of(&i.name.text);
            if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                continue;
            }
            let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) else {
                continue;
            };
            let sc = self.c.wire_scale(&i.name.text);
            let v = local(&i.name.text);
            o.push_str(&format!(
                "    if {v} < {} || {v} > {} {{\n        throw RuleError.input(what: \"{}\", value: Int64({v}))\n    }}\n",
                crate::types::wire_int(lo, sc),
                crate::types::wire_int(hi, sc),
                tr!("{} が範囲の外です", "{} is out of range", i.name.text)
            ));
        }
        o
    }

    fn sw_fn(&self) -> String {
        let fname = sw_name(&pub_name(&self.f.name));
        let outs = &self.f.outputs;
        let mut o = String::new();

        // One element is a row of inputs, so it is a struct like an `Output`, and needs its
        // initializer declared for the same reason.
        if let Some(el) = &self.f.elements {
            let fields: Vec<(String, String)> = el
                .fields
                .iter()
                .map(|fd| (sw_name(&pub_name(&fd.name)), self.sw_ty(&self.ty_of(&fd.name.text))))
                .collect();
            o.push_str(&format!("/// {}\npublic struct Element: Hashable, Sendable {{\n", tr!("{} の一件", "one of {}", el.name.text)));
            for (n, t) in &fields {
                o.push_str(&format!("    public var {n}: {t}\n"));
            }
            o.push_str(&format!(
                "\n    public init({}) {{\n",
                fields.iter().map(|(n, t)| format!("{n}: {t}")).collect::<Vec<_>>().join(", ")
            ));
            for (n, _) in &fields {
                o.push_str(&format!("        self.{n} = {n}\n"));
            }
            o.push_str("    }\n}\n\n");
        }

        // Multiple outputs come back as a struct. The memberwise initializer Swift writes
        // for a public struct is internal, so a caller in another module would not be able
        // to build one; this one is declared.
        if outs.len() > 1 {
            let fields: Vec<(String, String)> = outs
                .iter()
                .map(|od| (sw_name(&pub_name(&od.name)), self.sw_ty(&self.ty_of(&od.name.text))))
                .collect();
            o.push_str("public struct Output: Hashable, Sendable {\n");
            for (n, t) in &fields {
                o.push_str(&format!("    public var {n}: {t}\n"));
            }
            o.push_str(&format!(
                "\n    public init({}) {{\n",
                fields.iter().map(|(n, t)| format!("{n}: {t}")).collect::<Vec<_>>().join(", ")
            ));
            for (n, _) in &fields {
                o.push_str(&format!("        self.{n} = {n}\n"));
            }
            o.push_str("    }\n}\n\n");
        }

        let ret = if outs.len() == 1 {
            self.sw_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", sw_name(&pub_name(&i.name)), self.sw_ty(&self.ty_of(&i.name.text))))
            .collect();

        // The public function keeps the plain signature; the branches live in its traced
        // twin, which also returns the rows that matched (§15.33).
        let mut args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", sw_label(&pub_name(&i.name)), sw_name(&pub_name(&i.name))))
            .collect();
        // The sequence a walk reads is a parameter like the rest (§15.56).
        if let Some(el) = &self.f.elements {
            params.push(format!("{}: [Element]", sw_name(&pub_name(&el.name))));
            args.push(format!("{}: {}", sw_label(&pub_name(&el.name)), sw_name(&pub_name(&el.name))));
        }
        let traced = format!("{fname}Traced");
        o.push_str(&format!("/// {}\n", plain_doc(&self.f.name.text, &self.f.version, &format!("`{traced}`"))));
        o.push_str(&format!(
            "public func {fname}({}) throws -> {ret} {{\n    try {traced}({}).0\n}}\n\n",
            params.join(", "),
            args.join(", ")
        ));
        o.push_str(&format!("/// {}\n", traced_doc(&self.f.name.text, &self.f.version)));
        o.push_str(&format!(
            "public func {traced}({}) throws -> ({ret}, [Fired]) {{\n",
            params.join(", ")
        ));

        // A branded input is an Int64 inside; unwrap it once, where it is read.
        let local = |n: &str| -> String {
            match self.f.inputs.iter().find(|i| i.name.text == n) {
                Some(i) => {
                    let v = sw_name(&pub_name(&i.name));
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                        format!("{v}.value")
                    } else {
                        v
                    }
                }
                None => self.sw_ident(n),
            }
        };
        // Entry guards. An enum needs none: a value of an enum type is one of its cases by
        // construction, so the check Python, TypeScript, Ruby and Go make at run time is
        // already made by the compiler, exactly as in Rust.
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                continue;
            }
            let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) else {
                continue;
            };
            let sc = self.c.wire_scale(&i.name.text);
            let v = local(&i.name.text);
            o.push_str(&format!(
                "    if {v} < {} || {v} > {} {{\n        throw RuleError.input(what: \"{}\", value: Int64({v}))\n    }}\n",
                crate::types::wire_int(lo, sc),
                crate::types::wire_int(hi, sc),
                tr!("{} が範囲の外です", "{} is out of range", i.name.text)
            ));
        }

        o.push_str(&self.constraint_guards(&local, Lang::Sw, "    ", |m| {
            format!("        throw RuleError.input(what: \"{m}\", value: nil)\n")
        }));

        let trace = self.temp("trace");
        let has_table = self.f.items.iter().any(|i| matches!(i, Item::Table(_)));
        o.push_str(&format!("    {} {trace}: [Fired] = []\n", if has_table { "var" } else { "let" }));

        match &self.f.fold {
            Some(fold) => o.push_str(&self.sw_walk(fold, &trace)),
            None if self.counts().is_empty() => o.push_str(&self.sw_items(&local, &trace, Phase::All)),
            None => o.push_str(&self.sw_count_walk(&local, &trace)),
        }

        let wrap = |ty: &Ty, body: String| -> String {
            match ty {
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => format!("{}({body})", self.sw_ty(ty)),
                _ => body,
            }
        };
        // Every output takes the same three steps: its source — the `result` expression for
        // the first output, the binding of its own name otherwise — brought to the wire
        // scale, then its declared rounding applied exactly once (§15.16).
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let ty = self.ty_of(out_name);
            let body = match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os {
                        let raw = self.temp(&raw_base(oi));
                        o.push_str(&format!(
                            "    let {} = {}  // {}\n",
                            sw_name(&raw),
                            sw_expr(unparen(&res.text)),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("{}({}, {grid_i}) / {}", sw_round_fn(m), sw_name(&raw), res.scale / os)
                    } else {
                        format!("{}({}, {grid_i})", sw_round_fn(m), sw_expr(&res.text))
                    }
                }
                None => sw_expr(&res.text),
            };
            finals.push(wrap(&ty, body));
        }
        if outs.len() == 1 {
            o.push_str(&format!("    return ({}, {trace})\n}}\n", finals[0]));
        } else {
            let fields: Vec<String> = outs
                .iter()
                .zip(&finals)
                .map(|(od, body)| format!("{}: {body}", sw_name(&pub_name(&od.name))))
                .collect();
            o.push_str(&format!("    return (Output({}), {trace})\n}}\n", fields.join(", ")));
        }
        o
    }

    fn sw_table(&self, t: &Table, local: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = format!("    // {}\n", self.table_head(t));
        // Swift lets a `let` be assigned once on every path; the throwing `else` below is a
        // path that assigns nothing, and the compiler accepts it because it does not return.
        for oc in &t.outputs {
            let ty = self.ty_of(&oc.name.text);
            let decl = match ty {
                Ty::Bool => "Bool".to_string(),
                Ty::Str => "String".to_string(),
                Ty::Enum(_) => self.sw_ty(&ty),
                _ => "Int64".to_string(),
            };
            o.push_str(&format!("    let {}: {decl}\n", self.sw_ident(&oc.name.text)));
        }
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.sw_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "true".into() } else { conds.join(" && ") };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            let line = self.row_head(t, ri, &cells.join(" | "));
            let head = if ri == 0 { "    if" } else { "    } else if" };
            o.push_str(&format!("{head} {cond} {{ // {line}\n"));
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let oty = self.ty_of(&oc.name.text);
                        self.int_lit(n, &oty, self.scale(&oc.name.text))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.sw_value(w),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        Lit::Str(x) => str_lit(x),
                        _ => "0".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "true".into()
                        } else if w == crate::kw::FALSE {
                            "false".into()
                        } else if self.value_names.contains_key(w) {
                            self.sw_value(w)
                        } else {
                            sw_expr(&self.rescaled(w, &oc.name.text, local(w)))
                        }
                    }
                    None => "0".into(),
                };
                o.push_str(&format!("        {} = {v}\n", self.sw_ident(&oc.name.text)));
            }
            let (tn, rn, lbl) = self.fired_of(t, ri);
            if lbl.is_empty() {
                o.push_str(&format!("        {trace}.append(Fired(table: {tn:?}, row: {rn}))\n"));
            } else {
                o.push_str(&format!("        {trace}.append(Fired(table: {tn:?}, row: {rn}, label: {lbl:?}))\n"));
            }
        }
        o.push_str(&format!(
            "    }} else {{\n        throw RuleError.contradiction(what: \"{}\")\n    }}\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        for oc in &t.outputs {
            o.push_str(&self.sw_unread(&oc.name.text));
        }
        o.push_str(&self.guards(t, local, Lang::Sw, "    ", |name, i, j| {
            format!(
                "        throw RuleError.contradiction(what: \"{}\")\n",
                tr!("表 {name}: {i} と {j} が同時に当てはまりました", "table {name}: {i} and {j} matched at the same time")
            )
        }));
        o
    }

    /// The runner. Foundation's `JSONSerialization` is to Swift what `json` is to Ruby —
    /// part of the toolchain rather than a dependency — and it keeps an integer that fits
    /// in `Int64` exact, which is what the vectors need. The answer is written out by hand
    /// rather than serialized, because the comparison is byte-for-byte and the order of the
    /// fields is the order the rule declares its outputs in.
    ///
    /// `@main` needs the runner not to be the module's main file, which is why it is
    /// `{alias}_runner.swift` and not `main.swift`.
    pub fn swift_runner(&self) -> String {
        // The module takes its name from the binary, so the call has to be the function's
        // own spelling: a plain `shipping_fee(...)` resolves to the *module* and fails to
        // compile, which is how this was found.
        let fname = sw_name(&pub_name(&self.f.name));
        let d = runner_local("d", &fname);
        let got = runner_local("got", &fname);
        let line = runner_local("line", &fname);
        let root = runner_local("root", &fname);
        let trace = runner_local("trace", &fname);
        let mut args: Vec<String> = Vec::new();
        let mut binds: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            let jp = &i.name.text;
            let label = sw_label(&pub_name(&i.name));
            let v = match &ty {
                Ty::Enum(n) => {
                    let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                    format!("{cls}(rawValue: _s({d}, {jp:?}))!")
                }
                Ty::Str => format!("_s({d}, {jp:?})"),
                Ty::Bool => format!("_b({d}, {jp:?})"),
                Ty::Date => format!("_ord(_s({d}, {jp:?}))"),
                Ty::Number => format!("_n({d}, {jp:?})"),
                // `null` on the wire is `nil`. The brand of an optional is `Kind?`, which
                // used to be pasted in as if it were an initialiser (DESIGN §15.88).
                Ty::Opt(inner) => {
                    let one = match inner.as_ref() {
                        Ty::Enum(n) => {
                            let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                            format!("{cls}(rawValue: _s({d}, {jp:?}))!")
                        }
                        Ty::Bool => format!("_b({d}, {jp:?})"),
                        Ty::Date => format!("_ord(_s({d}, {jp:?}))"),
                        Ty::Number => format!("_n({d}, {jp:?})"),
                        Ty::Str => format!("_s({d}, {jp:?})"),
                        other => format!("{}(_n({d}, {jp:?}))", self.sw_ty(other)),
                    };
                    format!("({d}[{jp:?}] is NSNull || {d}[{jp:?}] == nil ? nil : {one})")
                }
                _ => format!("{}(_n({d}, {jp:?}))", self.sw_ty(&ty)),
            };
            let local = runner_local(&format!("a{}", args.len()), &fname);
            binds.push(format!("            let {local} = {v}\n"));
            args.push(format!("{label}: {local}"));
        }
        // The sequence arrives as an array of objects (§10.2), one per element.
        if let Some(el) = &self.f.elements {
            let jp = &el.name.text;
            let label = sw_label(&pub_name(&el.name));
            let local = runner_local(&format!("a{}", args.len()), &fname);
            let ev = runner_local("e", &fname);
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    let jf = &fd.name.text;
                    let v = match &ty {
                        Ty::Enum(n) => {
                            let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                            format!("{cls}(rawValue: _s({ev}, {jf:?}))!")
                        }
                        Ty::Str => format!("_s({ev}, {jf:?})"),
                        Ty::Bool => format!("_b({ev}, {jf:?})"),
                        Ty::Date => format!("_ord(_s({ev}, {jf:?}))"),
                        Ty::Number => format!("_n({ev}, {jf:?})"),
                        _ => format!("{}(_n({ev}, {jf:?}))", self.sw_ty(&ty)),
                    };
                    format!("{}: {v}", sw_name(&pub_name(&fd.name)))
                })
                .collect();
            binds.push(format!(
                "            let {local} = ({d}[{jp:?}] as! [[String: Any]]).map {{ {ev} in Element({}) }}\n",
                inner.join(", ")
            ));
            args.push(format!("{label}: {local}"));
        }
        // The record function's own parameter names, which are the call's labels.
        let (p_out, p_trace) = (sw_name(&self.temp("out")), sw_name(&self.temp("trace")));
        format!(
            r#"// Code generated by rulec {ver}. DO NOT EDIT.
import Foundation

@main
enum Runner {{
    static func main() throws {{
        while let {line} = readLine(strippingNewline: true) {{
            if {line}.trimmingCharacters(in: .whitespaces).isEmpty {{
                continue
            }}
            let {root} = try JSONSerialization.jsonObject(with: Data({line}.utf8)) as! [String: Any]
            let {d} = {root}["in"] as! [String: Any]
{binds}            let ({got}, {trace}) = try {fname}Traced({args})
            print({fname}Record({args}, {p_out}: {got}, {p_trace}: {trace}))
        }}
    }}

    /// A number, as NSNumber on both Darwin and the corelibs Foundation on Linux.
    static func _n(_ d: [String: Any], _ k: String) -> Int64 {{
        (d[k] as? NSNumber)?.int64Value ?? 0
    }}

    static func _s(_ d: [String: Any], _ k: String) -> String {{
        d[k] as? String ?? ""
    }}

    static func _b(_ d: [String: Any], _ k: String) -> Bool {{
        (d[k] as? NSNumber)?.boolValue ?? false
    }}

    /// A date as its day number from 1970-01-01, by the same civil-date arithmetic the tool
    /// uses. Foundation's own parsers carry a calendar and a time zone; this carries none.
    static func _ord(_ s: String) -> Int64 {{
        let p = s.split(separator: "-").map {{ Int64($0) ?? 0 }}
        let (y, m, d) = (p[0], p[1], p[2])
        let y2 = m <= 2 ? y - 1 : y
        let era = (y2 >= 0 ? y2 : y2 - 399) / 400
        let yoe = y2 - era * 400
        let mp = (m + 9) % 12
        let doy = (153 * mp + 2) / 5 + d - 1
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy
        return era * 146097 + doe - 719468
    }}

}}
"#,
            ver = env!("CARGO_PKG_VERSION"),
            args = args.join(", "),
            binds = binds.join("")
        )
    }

    /// The type as the inventory states it for Swift.
    fn sw_api_ty(&self, ty: &Ty) -> String {
        self.sw_ty(ty)
    }
}

/// The five modes of §7.3 in Swift, checked against the same reference cases the other five
/// are. A lone file compiled by `swiftc` is the module's main file, so this one is written
/// as top-level code and needs no `@main`.
pub fn round_tests_swift() -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n// {}\nimport Foundation\n",
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の五モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five modes of §7.3, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(round_sw().trim_start_matches('\n'));
    o.push_str("\nlet cases: [(String, Int64, Int64, Int64)] = [\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("    (\"{}\", {x}, {g}, {want}),\n", mode_fn(m)));
    }
    o.push_str("]\n\nvar bad = 0\nfor (mode, x, g, want) in cases {\n    let got: Int64\n    switch mode {\n");
    o.push_str("    case \"down\": got = _roundDown(x, g)\n    case \"up\": got = _roundUp(x, g)\n");
    o.push_str("    case \"half\": got = _roundHalf(x, g)\n    case \"half_down\": got = _roundHalfDown(x, g)\n    default: got = _roundBankers(x, g)\n    }\n");
    o.push_str("    if got != want {\n        print(\"NG \\(mode)(\\(x), \\(g)) = \\(got), want \\(want)\")\n        bad += 1\n    }\n}\n");
    o.push_str("if bad > 0 {\n    exit(1)\n}\n");
    o.push_str(&format!("print(\"{}\")\n", tr!("ok \\(cases.count) 件", "ok \\(cases.count) cases")));
    o
}


// ---------------------------------------------------------------------------
// The record function (§15.35): one call as one line of the fixtures format.
// ---------------------------------------------------------------------------

/// Howard Hinnant's civil_from_days, as each language spells it. Only emitted when the rule
/// has a date anywhere on its surface.
fn civil_needed(f: &RuleFile, c: &Checked) -> bool {
    f.inputs
        .iter()
        .map(|i| &i.name.text)
        .chain(f.outputs.iter().map(|o| &o.name.text))
        .any(|n| matches!(c.ty_of(n), Some(Ty::Date) | Some(Ty::Opt(_))))
}

impl<'a> Gen<'a> {
    /// The inputs of the rule with the expression the record function reads each from, in
    /// the language's own spelling, and the outputs likewise.
    fn record_fields(&self, input: impl Fn(&str, &Ty) -> String, output: impl Fn(usize, &str, &Ty) -> String) -> (Vec<(String, String, Wire)>, Vec<(String, String, Wire)>) {
        let ins = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                (i.name.text.clone(), input(&pub_name(&i.name), &ty), wire_of(&ty))
            })
            .collect();
        let outs = self
            .f
            .outputs
            .iter()
            .enumerate()
            .map(|(k, o)| {
                let ty = self.ty_of(&o.name.text);
                (o.name.text.clone(), output(k, &pub_name(&o.name), &ty), wire_of(&ty))
            })
            .collect();
        (ins, outs)
    }

    // ── Python
    fn py_wire(e: &str, w: &Wire) -> String {
        match w {
            Wire::Enum => format!("_json_str({e}.value)"),
            Wire::Bool => format!("(\"true\" if {e} else \"false\")"),
            Wire::Int | Wire::Brand => format!("f\"{{{e}}}\""),
            Wire::Date => format!("_json_str(_civil({e}))"),
            Wire::Str => format!("_json_str({e})"),
            Wire::Opt(w) => format!("(\"null\" if {e} is None else {})", Self::py_wire(e, w)),
        }
    }

    pub fn py_record(&self) -> String {
        let fname = pub_name(&self.f.name);
        let single = self.f.outputs.len() == 1;
        let (out, trace, tag) = (self.temp("out"), self.temp("trace"), self.temp("tag"));
        let (ins, obs) = self.record_fields(
            |a, _| a.to_string(),
            |_, a, _| if single { out.clone() } else { format!("{out}.{a}") },
        );
        let ret = if single { self.py_ty(&self.ty_of(&self.f.outputs[0].name.text)) } else { "Output".into() };
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.py_ty(&self.ty_of(&i.name.text))))
            .collect();
        if let Some(el) = &self.f.elements {
            params.push(format!("{}: list[Element]", pub_name(&el.name)));
        }
        let mut o = String::new();
        o.push_str(
            "def _json_str(s: str) -> str:\n    o = '\"'\n    for ch in s:\n        if ch == '\"' or ch == \"\\\\\":\n            o += \"\\\\\" + ch\n        elif ord(ch) < 32:\n            o += \"\\\\u%04x\" % ord(ch)\n        else:\n            o += ch\n    return o + '\"'\n\n\n",
        );
        if civil_needed(self.f, self.c) {
            o.push_str(
                "def _civil(days: int) -> str:\n    z = days + 719468\n    era = (z if z >= 0 else z - 146096) // 146097\n    doe = z - era * 146097\n    yoe = (doe - doe // 1460 + doe // 36524 - doe // 146096) // 365\n    doy = doe - (365 * yoe + yoe // 4 - yoe // 100)\n    mp = (5 * doy + 2) // 153\n    d = doy - (153 * mp + 2) // 5 + 1\n    m = mp + 3 if mp < 10 else mp - 9\n    y = yoe + era * 400 + (1 if m <= 2 else 0)\n    return f\"{y:04d}-{m:02d}-{d:02d}\"\n\n\n",
            );
        }
        o.push_str(&format!(
            "def {fname}_record({}, {out}: {ret}, {trace}: _Trace, {tag}: str = \"\") -> str:\n    \"\"\"{}\"\"\"\n",
            params.join(", "),
            record_doc()
        ));
        let (v_ins, v_obs, v_rows, v_head) = (self.temp("ins"), self.temp("obs"), self.temp("rows"), self.temp("head"));
        let field = |(jp, e, w): &(String, String, Wire)| format!("        '\"{jp}\":' + {},\n", Self::py_wire(e, w));
        // The sequence goes in first, and one element is written the way one record's inputs
        // are: the same wire, one level in (§15.56).
        let mut seq_first = String::new();
        if let Some(el) = &self.f.elements {
            let seq = self.temp("seq");
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    format!(
                        "'\"{}\":' + {}",
                        fd.name.text,
                        Self::py_wire(&format!("e.{}", pub_name(&fd.name)), &wire_of(&ty))
                    )
                })
                .collect();
            o.push_str(&format!(
                "    {seq} = '\"{}\":[' + \",\".join(\"{{\" + {} + \"}}\" for e in {}) + \"]\"\n",
                el.name.text,
                inner.join(" + \",\" + "),
                pub_name(&el.name)
            ));
            seq_first = format!("        {seq},\n");
        }
        o.push_str(&format!("    {v_ins} = [\n{seq_first}{}    ]\n", ins.iter().map(field).collect::<String>()));
        o.push_str(&format!("    {v_obs} = [\n{}    ]\n", obs.iter().map(field).collect::<String>()));
        o.push_str(&format!(
            "    {v_rows} = ['{{\"table\":' + _json_str(f.table) + ',\"row\":' + f\"{{f.row}}\" + (',\"label\":' + _json_str(f.label) if f.label else \"\") + \"}}\" for f in {trace}]\n"
        ));
        o.push_str(&format!("    {v_head} = '{{\"tag\":' + _json_str({tag}) + \",\" if {tag} else \"{{\"\n"));
        o.push_str(&format!(
            "    return {v_head} + '\"in\":{{' + \",\".join({v_ins}) + '}},\"observed\":{{' + \",\".join({v_obs}) + '}},\"trace\":[' + \",\".join({v_rows}) + \"]}}\"\n"
        ));
        o
    }

    // ── TypeScript
    fn ts_wire(e: &str, w: &Wire) -> String {
        match w {
            Wire::Enum | Wire::Str => format!("JSON.stringify({e})"),
            Wire::Bool | Wire::Int | Wire::Brand => format!("String({e})"),
            Wire::Date => format!("JSON.stringify(_civil({e}))"),
            Wire::Opt(w) => format!("({e} === null ? \"null\" : {})", Self::ts_wire(e, w)),
        }
    }

    pub fn ts_record(&self) -> String {
        let fname = pub_name(&self.f.name);
        let single = self.f.outputs.len() == 1;
        let (out, trace, tag) = (self.temp("out"), self.temp("trace"), self.temp("tag"));
        let (ins, obs) = self.record_fields(
            |a, _| a.to_string(),
            |_, a, _| if single { out.clone() } else { format!("{out}.{a}") },
        );
        let ret = if single { self.ts_ty(&self.ty_of(&self.f.outputs[0].name.text)) } else { "Output".into() };
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.ts_ty(&self.ty_of(&i.name.text))))
            .collect();
        if let Some(el) = &self.f.elements {
            params.push(format!("{}: readonly Element[]", pub_name(&el.name)));
        }
        let mut o = String::new();
        if civil_needed(self.f, self.c) {
            o.push_str(
                "function _civil(days: bigint): string {\n  const z = days + 719468n;\n  const era = (z >= 0n ? z : z - 146096n) / 146097n;\n  const doe = z - era * 146097n;\n  const yoe = (doe - doe / 1460n + doe / 36524n - doe / 146096n) / 365n;\n  const doy = doe - (365n * yoe + yoe / 4n - yoe / 100n);\n  const mp = (5n * doy + 2n) / 153n;\n  const d = doy - (153n * mp + 2n) / 5n + 1n;\n  const m = mp < 10n ? mp + 3n : mp - 9n;\n  const y = yoe + era * 400n + (m <= 2n ? 1n : 0n);\n  return String(y).padStart(4, \"0\") + \"-\" + String(m).padStart(2, \"0\") + \"-\" + String(d).padStart(2, \"0\");\n}\n\n",
            );
        }
        o.push_str(&format!("/** {} */\n", record_doc()));
        o.push_str(&format!(
            "export function {fname}_record({}, {out}: {ret}, {trace}: Fired[], {tag} = \"\"): string {{\n",
            params.join(", ")
        ));
        let (v_ins, v_obs, v_rows, v_head) = (self.temp("ins"), self.temp("obs"), self.temp("rows"), self.temp("head"));
        let field = |(jp, e, w): &(String, String, Wire)| format!("    '\"{jp}\":' + {},\n", Self::ts_wire(e, w));
        // The sequence goes in first, written the way one record's inputs are (§15.56).
        let mut seq_first = String::new();
        if let Some(el) = &self.f.elements {
            let e = self.temp("elem");
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    format!(
                        "'\"{}\":' + {}",
                        fd.name.text,
                        Self::ts_wire(&format!("{e}.{}", pub_name(&fd.name)), &wire_of(&ty))
                    )
                })
                .collect();
            seq_first = format!(
                "    '\"{}\":[' + {}.map(({e}) => \"{{\" + {} + \"}}\").join(\",\") + \"]\",\n",
                el.name.text,
                pub_name(&el.name),
                inner.join(" + \",\" + ")
            );
        }
        o.push_str(&format!("  const {v_ins} = [\n{seq_first}{}  ].join(\",\");\n", ins.iter().map(field).collect::<String>()));
        o.push_str(&format!("  const {v_obs} = [\n{}  ].join(\",\");\n", obs.iter().map(field).collect::<String>()));
        o.push_str(&format!(
            "  const {v_rows} = {trace}.map((f) => '{{\"table\":' + JSON.stringify(f.table) + ',\"row\":' + String(f.row) + (f.label ? ',\"label\":' + JSON.stringify(f.label) : \"\") + \"}}\").join(\",\");\n"
        ));
        o.push_str(&format!("  const {v_head} = {tag} === \"\" ? \"{{\" : '{{\"tag\":' + JSON.stringify({tag}) + \",\";\n"));
        o.push_str(&format!(
            "  return {v_head} + '\"in\":{{' + {v_ins} + '}},\"observed\":{{' + {v_obs} + '}},\"trace\":[' + {v_rows} + \"]}}\";\n}}\n"
        ));
        o
    }

    // ── Rust
    fn rs_wire(e: &str, w: &Wire) -> String {
        match w {
            Wire::Enum => format!("json_str({e}.as_str())"),
            Wire::Bool | Wire::Int => format!("{e}.to_string()"),
            Wire::Brand => format!("{e}.0.to_string()"),
            Wire::Date => format!("json_str(&civil({e}))"),
            Wire::Str => format!("json_str(&{e})"),
            Wire::Opt(w) => format!("{e}.map(|v| {}).unwrap_or_else(|| \"null\".to_string())", Self::rs_wire("v", w)),
        }
    }

    pub fn rs_record(&self) -> String {
        let fname = pub_name(&self.f.name);
        let single = self.f.outputs.len() == 1;
        let (out, trace, tag) = (self.temp("out"), self.temp("trace"), self.temp("tag"));
        let (ins, obs) = self.record_fields(
            |a, _| a.to_string(),
            |_, a, _| if single { out.clone() } else { format!("{out}.{a}") },
        );
        let ret = if single { self.rs_ty(&self.ty_of(&self.f.outputs[0].name.text)) } else { "Output".into() };
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.rs_ty(&self.ty_of(&i.name.text))))
            .collect();
        if let Some(el) = &self.f.elements {
            params.push(format!("{}: &[Element]", pub_name(&el.name)));
        }
        let mut o = String::new();
        o.push_str(RS_JSON_STR);
        if civil_needed(self.f, self.c) {
            o.push_str(
                "fn civil(days: i64) -> String {\n    let z = days + 719468;\n    let era = if z >= 0 { z } else { z - 146096 } / 146097;\n    let doe = z - era * 146097;\n    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;\n    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);\n    let mp = (5 * doy + 2) / 153;\n    let d = doy - (153 * mp + 2) / 5 + 1;\n    let m = if mp < 10 { mp + 3 } else { mp - 9 };\n    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };\n    format!(\"{y:04}-{m:02}-{d:02}\")\n}\n\n",
            );
        }
        o.push_str(&format!("/// {}\n", record_doc()));
        o.push_str(&format!(
            "pub fn {fname}_record({}, {out}: {ret}, {trace}: &[Fired], {tag}: &str) -> String {{\n",
            params.join(", ")
        ));
        let (v_ins, v_obs, v_rows, v_head) = (self.temp("ins"), self.temp("obs"), self.temp("rows"), self.temp("head"));
        let field = |(jp, e, w): &(String, String, Wire)| format!("        String::from(\"\\\"{jp}\\\":\") + &{},\n", Self::rs_wire(e, w));
        // The sequence goes in first, each element written the way one record's inputs are.
        let mut seq_first = String::new();
        if let Some(el) = &self.f.elements {
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    format!(
                        "String::from(\"\\\"{}\\\":\") + &{}",
                        fd.name.text,
                        Self::rs_wire(&format!("e.{}", pub_name(&fd.name)), &wire_of(&ty))
                    )
                })
                .collect();
            seq_first = format!(
                "        String::from(\"\\\"{}\\\":[\") + &{}.iter().map(|e| String::from(\"{{\") + &({}) + \"}}\").collect::<Vec<_>>().join(\",\") + \"]\",\n",
                el.name.text,
                pub_name(&el.name),
                inner.join(" + \",\" + &")
            );
        }
        o.push_str(&format!("    let {v_ins} = [\n{seq_first}{}    ]\n    .join(\",\");\n", ins.iter().map(field).collect::<String>()));
        o.push_str(&format!("    let {v_obs} = [\n{}    ]\n    .join(\",\");\n", obs.iter().map(field).collect::<String>()));
        o.push_str(&format!(
            "    let {v_rows} = {trace}\n        .iter()\n        .map(|f| String::from(\"{{\\\"table\\\":\") + &json_str(f.table) + \",\\\"row\\\":\" + &f.row.to_string() + &(if f.label.is_empty() {{ String::new() }} else {{ String::from(\",\\\"label\\\":\") + &json_str(f.label) }}) + \"}}\")\n        .collect::<Vec<_>>()\n        .join(\",\");\n"
        ));
        o.push_str(&format!(
            "    let {v_head} = if {tag}.is_empty() {{ String::from(\"{{\") }} else {{ String::from(\"{{\\\"tag\\\":\") + &json_str({tag}) + \",\" }};\n"
        ));
        o.push_str(&format!(
            "    {v_head} + \"\\\"in\\\":{{\" + &{v_ins} + \"}},\\\"observed\\\":{{\" + &{v_obs} + \"}},\\\"trace\\\":[\" + &{v_rows} + \"]}}\"\n}}\n"
        ));
        o
    }

    // ── Go
    fn go_wire(e: &str, w: &Wire) -> String {
        match w {
            Wire::Enum => format!("jsonStr({e}.String())"),
            Wire::Bool => format!("fmt.Sprint({e})"),
            Wire::Int | Wire::Brand => format!("fmt.Sprint(int64({e}))"),
            Wire::Date => format!("jsonStr(civil(int64({e})))"),
            Wire::Str => format!("jsonStr({e})"),
            // Handled as a statement by the caller: a pointer has to be tested first.
            Wire::Opt(w) => Self::go_wire(&format!("(*{e})"), w),
        }
    }

    pub fn go_record(&self) -> String {
        let fname = pascal(&pub_name(&self.f.name));
        let single = self.f.outputs.len() == 1;
        let (ins, obs) = self.record_fields(
            |a, _| format!("in.{}", pascal(a)),
            |_, a, _| if single { "out".to_string() } else { format!("out.{}", pascal(a)) },
        );
        let ret = if single { self.go_ty(&self.ty_of(&self.f.outputs[0].name.text)) } else { "Output".into() };
        let mut o = String::new();
        o.push_str(
            "func jsonStr(s string) string {\n\tb := []byte{'\"'}\n\tfor _, r := range s {\n\t\tswitch {\n\t\tcase r == '\"' || r == '\\\\':\n\t\t\tb = append(b, '\\\\', byte(r))\n\t\tcase r < 32:\n\t\t\tb = append(b, []byte(fmt.Sprintf(\"\\\\u%04x\", r))...)\n\t\tdefault:\n\t\t\tb = append(b, []byte(string(r))...)\n\t\t}\n\t}\n\treturn string(append(b, '\"'))\n}\n\n",
        );
        o.push_str("func joinComma(xs []string) string {\n\ts := \"\"\n\tfor i, x := range xs {\n\t\tif i > 0 {\n\t\t\ts += \",\"\n\t\t}\n\t\ts += x\n\t}\n\treturn s\n}\n\n");
        if civil_needed(self.f, self.c) {
            o.push_str(
                "func civil(days int64) string {\n\tz := days + 719468\n\tera := z\n\tif z < 0 {\n\t\tera = z - 146096\n\t}\n\tera /= 146097\n\tdoe := z - era*146097\n\tyoe := (doe - doe/1460 + doe/36524 - doe/146096) / 365\n\tdoy := doe - (365*yoe + yoe/4 - yoe/100)\n\tmp := (5*doy + 2) / 153\n\td := doy - (153*mp+2)/5 + 1\n\tm := mp - 9\n\tif mp < 10 {\n\t\tm = mp + 3\n\t}\n\ty := yoe + era*400\n\tif m <= 2 {\n\t\ty++\n\t}\n\treturn fmt.Sprintf(\"%04d-%02d-%02d\", y, m, d)\n}\n\n",
            );
        }
        o.push_str(&format!("// {fname}Record {}\n", record_doc()));
        o.push_str(&format!("func {fname}Record(in Input, out {ret}, trace []Fired, tag string) string {{\n"));
        // The sequence goes in first, and one element is written the way one record's inputs
        // are: the same wire, one level in (§15.56).
        let mut seq_first = String::new();
        if let Some(el) = &self.f.elements {
            let seq = self.temp("seq");
            let g = pascal(&pub_name(&el.name));
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    format!(
                        "\"\\\"{}\\\":\" + {}",
                        fd.name.text,
                        Self::go_wire(&format!("e.{}", pascal(&pub_name(&fd.name))), &wire_of(&ty))
                    )
                })
                .collect();
            o.push_str(&format!("\t{seq} := \"\\\"{}\\\":[\"\n", el.name.text));
            o.push_str(&format!("\tfor i, e := range in.{g} {{\n\t\tif i > 0 {{\n\t\t\t{seq} += \",\"\n\t\t}}\n"));
            o.push_str(&format!("\t\t{seq} += \"{{\" + {} + \"}}\"\n\t}}\n", inner.join(" + \",\" + ")));
            o.push_str(&format!("\t{seq} += \"]\"\n"));
            seq_first = seq;
        }
        let mut list = |name: &str, fields: &[(String, String, Wire)]| {
            o.push_str(&format!("\t{name} := []string{{}}\n"));
            if name == "ins" && !seq_first.is_empty() {
                o.push_str(&format!("\tins = append(ins, {seq_first})\n"));
            }
            for (jp, e, w) in fields {
                match w {
                    Wire::Opt(_) => o.push_str(&format!(
                        "\tif {e} == nil {{\n\t\t{name} = append({name}, \"\\\"{jp}\\\":null\")\n\t}} else {{\n\t\t{name} = append({name}, fmt.Sprintf(\"\\\"{jp}\\\":%s\", {}))\n\t}}\n",
                        Self::go_wire(e, w)
                    )),
                    _ => o.push_str(&format!("\t{name} = append({name}, fmt.Sprintf(\"\\\"{jp}\\\":%s\", {}))\n", Self::go_wire(e, w))),
                }
            }
        };
        list("ins", &ins);
        list("obs", &obs);
        o.push_str("\trows := []string{}\n\tfor _, f := range trace {\n\t\tlbl := \"\"\n\t\tif f.Label != \"\" {\n\t\t\tlbl = fmt.Sprintf(\",\\\"label\\\":%s\", jsonStr(f.Label))\n\t\t}\n\t\trows = append(rows, fmt.Sprintf(\"{\\\"table\\\":%s,\\\"row\\\":%d%s}\", jsonStr(f.Table), f.Row, lbl))\n\t}\n");
        o.push_str("\thead := \"{\"\n\tif tag != \"\" {\n\t\thead = fmt.Sprintf(\"{\\\"tag\\\":%s,\", jsonStr(tag))\n\t}\n");
        o.push_str("\treturn fmt.Sprintf(\"%s\\\"in\\\":{%s},\\\"observed\\\":{%s},\\\"trace\\\":[%s]}\", head, joinComma(ins), joinComma(obs), joinComma(rows))\n}\n");
        o
    }

    // ── Ruby
    fn rb_wire(e: &str, w: &Wire) -> String {
        match w {
            Wire::Enum | Wire::Str => format!("_json_str({e})"),
            Wire::Bool | Wire::Int | Wire::Brand => e.to_string(),
            Wire::Date => format!("_json_str(_civil({e}))"),
            Wire::Opt(w) => format!("({e}.nil? ? \"null\" : {})", Self::rb_wire(e, w)),
        }
    }

    pub fn rb_record(&self) -> String {
        let fname = pub_name(&self.f.name);
        let single = self.f.outputs.len() == 1;
        let (out, trace, tag) = (self.temp("out"), self.temp("trace"), self.temp("tag"));
        let (ins, obs) = self.record_fields(
            |a, _| a.to_string(),
            |_, a, _| if single { out.clone() } else { format!("{out}.{a}") },
        );
        let mut params: Vec<String> = self.f.inputs.iter().map(|i| pub_name(&i.name)).collect();
        if let Some(el) = &self.f.elements {
            params.push(pub_name(&el.name));
        }
        let mut o = String::new();
        o.push_str(
            "  def self._json_str(s)\n    o = +\"\\\"\"\n    s.each_char do |ch|\n      if ch == \"\\\"\" || ch == \"\\\\\"\n        o << \"\\\\\" << ch\n      elsif ch.ord < 32\n        o << format(\"\\\\u%04x\", ch.ord)\n      else\n        o << ch\n      end\n    end\n    o << \"\\\"\"\n  end\n\n",
        );
        if civil_needed(self.f, self.c) {
            o.push_str(
                "  def self._civil(days)\n    z = days + 719468\n    era = (z >= 0 ? z : z - 146096) / 146097\n    doe = z - era * 146097\n    yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365\n    doy = doe - (365 * yoe + yoe / 4 - yoe / 100)\n    mp = (5 * doy + 2) / 153\n    d = doy - (153 * mp + 2) / 5 + 1\n    m = mp < 10 ? mp + 3 : mp - 9\n    y = yoe + era * 400 + (m <= 2 ? 1 : 0)\n    format(\"%04d-%02d-%02d\", y, m, d)\n  end\n\n",
            );
        }
        o.push_str(&format!("  # {}\n", record_doc()));
        o.push_str(&format!(
            "  def self.{fname}_record({}, {out}, {trace}, {tag} = \"\")\n",
            params.join(", ")
        ));
        let (v_ins, v_obs, v_rows, v_head) = (self.temp("ins"), self.temp("obs"), self.temp("rows"), self.temp("head"));
        let field = |(jp, e, w): &(String, String, Wire)| format!("      \"\\\"{jp}\\\":#{{{}}}\",\n", Self::rb_wire(e, w));
        // The sequence goes in first (§15.56).
        let mut seq_first = String::new();
        if let Some(el) = &self.f.elements {
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    format!(
                        "\\\"{}\\\":#{{{}}}",
                        fd.name.text,
                        Self::rb_wire(&format!("e.{}", pub_name(&fd.name)), &wire_of(&ty))
                    )
                })
                .collect();
            seq_first = format!(
                "      \"\\\"{}\\\":[\" + {}.map {{ |e| \"{{{}}}\" }}.join(\",\") + \"]\",\n",
                el.name.text,
                pub_name(&el.name),
                inner.join(",")
            );
        }
        o.push_str(&format!("    {v_ins} = [\n{seq_first}{}    ].join(\",\")\n", ins.iter().map(field).collect::<String>()));
        o.push_str(&format!("    {v_obs} = [\n{}    ].join(\",\")\n", obs.iter().map(field).collect::<String>()));
        o.push_str(&format!(
            "    {v_rows} = {trace}.map {{ |f| lbl = f.label.to_s; \"{{\\\"table\\\":#{{_json_str(f.table)}},\\\"row\\\":#{{f.row}}#{{lbl.empty? ? \"\" : \",\\\"label\\\":#{{_json_str(lbl)}}\"}}}}\" }}.join(\",\")\n"
        ));
        o.push_str(&format!("    {v_head} = {tag}.empty? ? \"{{\" : \"{{\\\"tag\\\":#{{_json_str({tag})}},\"\n"));
        o.push_str(&format!(
            "    \"#{{{v_head}}}\\\"in\\\":{{#{{{v_ins}}}}},\\\"observed\\\":{{#{{{v_obs}}}}},\\\"trace\\\":[#{{{v_rows}}}]}}\"\n  end\n"
        ));
        o
    }

    // ── Swift
    fn sw_wire(e: &str, w: &Wire) -> String {
        match w {
            Wire::Enum => format!("_jsonStr({e}.rawValue)"),
            Wire::Bool | Wire::Int => format!("String({e})"),
            Wire::Brand => format!("String({e}.value)"),
            Wire::Date => format!("_jsonStr(_civil({e}))"),
            Wire::Str => format!("_jsonStr({e})"),
            Wire::Opt(w) => format!("({e}.map {{ {} }} ?? \"null\")", Self::sw_wire("$0", w)),
        }
    }

    pub fn sw_record(&self) -> String {
        let fname = sw_name(&pub_name(&self.f.name));
        let single = self.f.outputs.len() == 1;
        let (out, trace, tag) = (self.temp("out"), self.temp("trace"), self.temp("tag"));
        let (ins, obs) = self.record_fields(
            |a, _| sw_name(a),
            |_, a, _| if single { sw_name(&out) } else { format!("{}.{}", sw_name(&out), sw_name(a)) },
        );
        let ret = if single { self.sw_ty(&self.ty_of(&self.f.outputs[0].name.text)) } else { "Output".into() };
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", sw_name(&pub_name(&i.name)), self.sw_ty(&self.ty_of(&i.name.text))))
            .collect();
        if let Some(el) = &self.f.elements {
            params.push(format!("{}: [Element]", sw_name(&pub_name(&el.name))));
        }
        let mut o = String::new();
        o.push_str(
            "func _jsonStr(_ s: String) -> String {\n    var o = \"\\\"\"\n    for u in s.unicodeScalars {\n        if u == \"\\\"\" {\n            o += \"\\\\\\\"\"\n        } else if u == \"\\\\\" {\n            o += \"\\\\\\\\\"\n        } else if u.value < 32 {\n            let h = String(u.value, radix: 16)\n            o += \"\\\\u\" + String(repeating: \"0\", count: 4 - h.count) + h\n        } else {\n            o.unicodeScalars.append(u)\n        }\n    }\n    return o + \"\\\"\"\n}\n\n",
        );
        if civil_needed(self.f, self.c) {
            o.push_str(
                "func _pad(_ v: Int64, _ w: Int) -> String {\n    let s = String(v)\n    return s.count >= w ? s : String(repeating: \"0\", count: w - s.count) + s\n}\n\nfunc _civil(_ days: Int64) -> String {\n    let z = days + 719468\n    let era = (z >= 0 ? z : z - 146096) / 146097\n    let doe = z - era * 146097\n    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365\n    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100)\n    let mp = (5 * doy + 2) / 153\n    let d = doy - (153 * mp + 2) / 5 + 1\n    let m = mp < 10 ? mp + 3 : mp - 9\n    let y = yoe + era * 400 + (m <= 2 ? 1 : 0)\n    return _pad(y, 4) + \"-\" + _pad(m, 2) + \"-\" + _pad(d, 2)\n}\n\n",
            );
        }
        o.push_str(&format!("/// {}\n", record_doc()));
        o.push_str(&format!(
            "public func {fname}Record({}, {}: {ret}, {}: [Fired], {}: String = \"\") -> String {{\n",
            params.join(", "),
            sw_name(&out),
            sw_name(&trace),
            sw_name(&tag)
        ));
        let (v_ins, v_obs, v_rows, v_head) = (self.temp("ins"), self.temp("obs"), self.temp("rows"), self.temp("head"));
        let field = |(jp, e, w): &(String, String, Wire)| format!("        \"\\\"{jp}\\\":\\({})\",\n", Self::sw_wire(e, w));
        // The sequence goes in first, and one element is written the way one record's inputs
        // are: the same wire, one level in (§15.56).
        let mut seq_first = String::new();
        if let Some(el) = &self.f.elements {
            let seq = sw_name(&self.temp("seq"));
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    format!(
                        "\\\"{}\\\":\\({})",
                        fd.name.text,
                        Self::sw_wire(&format!("$0.{}", sw_name(&pub_name(&fd.name))), &wire_of(&ty))
                    )
                })
                .collect();
            o.push_str(&format!(
                "    let {seq} = \"\\\"{}\\\":[\" + {}.map {{ \"{{{}}}\" }}.joined(separator: \",\") + \"]\"\n",
                el.name.text,
                sw_name(&pub_name(&el.name)),
                inner.join(",")
            ));
            seq_first = format!("        {seq},\n");
        }
        o.push_str(&format!("    let {v_ins} = [\n{seq_first}{}    ].joined(separator: \",\")\n", ins.iter().map(field).collect::<String>()));
        o.push_str(&format!("    let {v_obs} = [\n{}    ].joined(separator: \",\")\n", obs.iter().map(field).collect::<String>()));
        o.push_str(&format!(
            "    let {v_rows} = {}.map {{ f in\n        let l = f.label.isEmpty ? \"\" : \",\\\"label\\\":\" + _jsonStr(f.label)\n        return \"{{\\\"table\\\":\\(_jsonStr(f.table)),\\\"row\\\":\\(f.row)\\(l)}}\"\n    }}.joined(separator: \",\")\n",
            sw_name(&trace)
        ));
        o.push_str(&format!(
            "    let {v_head} = {}.isEmpty ? \"{{\" : \"{{\\\"tag\\\":\\(_jsonStr({})),\"\n",
            sw_name(&tag),
            sw_name(&tag)
        ));
        o.push_str(&format!(
            "    return \"\\({v_head})\\\"in\\\":{{\\({v_ins})}},\\\"observed\\\":{{\\({v_obs})}},\\\"trace\\\":[\\({v_rows})]}}\"\n}}\n"
        ));
        o
    }
}


// ---------------------------------------------------------------------------
// JavaScript (§15.36): the TypeScript with its types taken off.
// ---------------------------------------------------------------------------

/// The generated TypeScript is erasable syntax by design (§8.3), so the JavaScript target
/// is the same file with the annotations removed: the same branches, the same helpers, held
/// to the same vectors by `rulec test`. A transform rather than a second emitter, because
/// two emitters of the same code drift apart; the agreement check is what keeps the
/// transform honest.
pub fn strip_types(ts: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_interface = false;
    for line in ts.lines() {
        if in_interface {
            if line == "}" {
                in_interface = false;
            }
            continue;
        }
        let t = line.trim_start();
        if t.starts_with("export type ") || t.starts_with("import type ") {
            continue;
        }
        if t.starts_with("export interface ") {
            in_interface = true;
            continue;
        }
        if is_field_decl(t) {
            continue;
        }
        // Comment lines carry prose, and prose says "as" too. An import's `as` is a
        // binding, not a cast, and stays.
        if t.starts_with("//") || t.starts_with("/*") || t.starts_with("* ") || t == "*/" || t.starts_with("import ") {
            out.push(line.to_string());
            continue;
        }
        let mut l = line.replace(" as const;", ";");
        l = strip_signature(&l);
        l = strip_declaration(&l);
        l = strip_casts(&l);
        out.push(l);
    }
    out.join("\n") + "\n"
}

/// `readonly what: string;` — a class property declared and not assigned. TypeScript needs
/// the line; JavaScript gets the property from the constructor, so the line is dropped
/// rather than rewritten. An object literal's `k: v,` ends in a comma and is left alone.
fn is_field_decl(t: &str) -> bool {
    let Some(rest) = t.strip_suffix(';') else { return false };
    let rest = rest.strip_prefix("readonly ").unwrap_or(rest);
    let Some((name, ty)) = rest.split_once(": ") else { return false };
    !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        && !ty.is_empty()
        && !ty.contains('=')
        && !ty.contains('(')
}

/// `function f(a: T, b: U): R {` → `function f(a, b) {`. Also a class constructor.
fn strip_signature(l: &str) -> String {
    let t = l.trim_start();
    if !(t.contains("function ") || t.starts_with("constructor(")) {
        return l.to_string();
    }
    let Some(open) = l.find('(') else { return l.to_string() };
    let Some(close) = l.rfind(')') else { return l.to_string() };
    if close < open {
        return l.to_string();
    }
    let params: Vec<String> = split_params(&l[open + 1..close])
        .into_iter()
        .filter(|p| !p.is_empty())
        .map(|p| match p.find(": ") {
            // A default value outlives its annotation: `value: unknown = NO_VALUE` keeps
            // the `= NO_VALUE`, or JavaScript would see a different sentinel (§15.95).
            Some(i) => match p[i..].find(" = ") {
                Some(j) => format!("{}{}", &p[..i], &p[i + j..]),
                None => p[..i].to_string(),
            },
            None => p.to_string(),
        })
        .collect();
    let mut rest = &l[close + 1..];
    if let Some(r) = rest.strip_prefix(": ") {
        // The return type runs up to the block's brace.
        rest = match r.rfind(" {") {
            Some(i) => &r[i..],
            None => "",
        };
    }
    format!("{}({}){}", &l[..open], params.join(", "), rest)
}

/// `let x: T;`, `const x: T = …` → without the annotation. Destructuring has none.
/// A parameter list, cut at the commas that separate parameters — not at the ones inside a
/// type. `Record<string, string>` is one type and not two parameters.
fn split_params(list: &str) -> Vec<&str> {
    let (mut depth, mut start) = (0i32, 0usize);
    let mut out = Vec::new();
    for (i, c) in list.char_indices() {
        match c {
            '<' | '(' | '[' | '{' => depth += 1,
            '>' | ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                out.push(list[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(list[start..].trim());
    out
}

fn strip_declaration(l: &str) -> String {
    let t = l.trim_start();
    let indent = &l[..l.len() - t.len()];
    let Some(rest) = t.strip_prefix("let ").or_else(|| t.strip_prefix("const ")) else {
        return l.to_string();
    };
    let kw = if t.starts_with("let ") { "let " } else { "const " };
    let name_end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(rest.len());
    if name_end == 0 {
        return l.to_string();
    }
    let after = &rest[name_end..];
    let Some(a) = after.strip_prefix(": ") else { return l.to_string() };
    // The annotation ends at the assignment or at the semicolon, whichever comes first.
    let end = match (a.find(" = "), a.find(';')) {
        (Some(i), Some(j)) => i.min(j),
        (Some(i), None) => i,
        (None, Some(j)) => j,
        (None, None) => return l.to_string(),
    };
    format!("{indent}{kw}{}{}", &rest[..name_end], &a[end..])
}

/// ` as YenInclTax`, ` as number`, ` as Record<string, unknown>` → gone.
fn strip_casts(l: &str) -> String {
    let mut o = String::new();
    let mut rest = l;
    while let Some(i) = rest.find(" as ") {
        o.push_str(&rest[..i]);
        let after = &rest[i + 4..];
        let mut n = after.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(after.len());
        if n == 0 {
            // Not a cast (nothing that looks like a type follows); keep the text.
            o.push_str(" as ");
            rest = after;
            continue;
        }
        if after[n..].starts_with('<') {
            if let Some(k) = after[n..].find('>') {
                n += k + 1;
            }
        }
        // An array type (`as unknown[]`) is one more pair of brackets on the end of the name.
        while after[n..].starts_with("[]") {
            n += 2;
        }
        rest = &after[n..];
    }
    o.push_str(rest);
    o
}

impl<'a> Gen<'a> {
    /// The JavaScript module: the TypeScript one without its types (§15.36).
    pub fn javascript(&self) -> String {
        strip_types(&self.typescript())
    }

    /// The runner, likewise. The import points at the `.mjs` beside it.
    pub fn js_runner(&self) -> String {
        strip_types(&self.ts_runner()).replace(".ts\";", ".mjs\";")
    }
}

pub fn round_tests_javascript() -> String {
    strip_types(&round_tests_typescript())
}
