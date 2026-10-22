//! The values of the reference interpreter (DESIGN 8), how a person reads them, and how they go on the
//! wire (DESIGN 9).
//!
//! A number is an exact rational counted in the unit of its type: money in its declared unit, a
//! quantity in its unit, a rate as the fraction it is (3.49% is 349/10000), a `number` as itself; a
//! date is its day number. Which unit that is comes from the type the check gave the expression, so
//! a value is read the same way the check typed it, and turned into another unit only where the
//! check says a value goes into a place of that unit (`15lb` beside `mass[oz]` is 240).
//!
//! On the wire (`run`, `vectors`, the inputs a host gives), a number is the whole number it travels
//! as, rulec's: money and quantities in their declared unit, a rate as a count of its step, a date as
//! `YYYY-MM-DD`, a value of an enum by its name, a record as an object of its fields by their names,
//! a list as an array, `none` as `null`, and a transfer as `{"book", "transfer", "op", "args"}`.
//! [`Program`] is what a module reaches, which both need to look up records, enums and transfers.

use crate::check::Checked;
use crate::names::{Target, book_unit, column_slot};
use crate::num::{self, Rat};
use crate::ty::*;
use ritsu_base::json::Json;
use ritsu_base::text::Text;
use ritsu_ports::{BookFacts, DateFacts, RuleFacts};
use ritsu_units::{Dim, Unit};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// A value of kaname.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A number in the unit of its type (DESIGN 8): a whole number for a rounded amount, any exact
    /// fraction for an unrounded one.
    Num(Rat),
    Bool(bool),
    Str(String),
    /// A day number.
    Date(i64),
    /// A value of an enum, by the name it is declared with: one with no fields.
    Enum(String),
    /// A value of an enum that has fields (DESIGN 23.6): its name, and each field by the name it is
    /// declared with, in the order declared.
    Variant(String, Vec<(String, Value)>),
    /// A record: each field by the name it is declared with, in the order declared.
    Record(Vec<(String, Value)>),
    List(Vec<Value>),
    /// A dict (DESIGN 24.4): its pairs, each a key and its value, in the order of its keys (24.1).
    Dict(Vec<(Value, Value)>),
    /// A set: its members, in the order of its keys.
    Set(Vec<Value>),
    /// The value that is not there. A value that is there is itself, not wrapped.
    None,
    /// A call of a transfer of a book, built to be made by the host (DESIGN 6.3).
    Transfer(Box<TransferValue>),
    /// A function value (DESIGN 27.3): a lambda, with the values of the names it caught.
    Closure(Rc<Closure>),
}

/// A lambda made a value (DESIGN 27.3): the module it is written in (on the disk) and its expression,
/// the values of the names in scope where it was made (they are never written again, so they are
/// what its body reads), and the function it was made in, with the types that function's type
/// parameters took: a call of it is a frame of that function's. A function value goes nowhere but
/// where it is passed and called (DESIGN 27.1), so it is never shown, put on the wire or compared.
#[derive(Debug, PartialEq)]
pub struct Closure {
    pub module: PathBuf,
    pub expr: usize,
    pub env: Vec<(String, Value)>,
    pub function: String,
    pub subst: Rc<(Vec<String>, Vec<Ty>)>,
}

/// A transfer built: which operation of which transfer of which book, and its arguments by the
/// transfer's names, in its order.
#[derive(Clone, Debug, PartialEq)]
pub struct TransferValue {
    pub id: TransferId,
    pub args: Vec<(String, Value)>,
}

impl Value {
    pub fn num(&self) -> Option<Rat> {
        match self {
            Value::Num(r) => Some(*r),
            _ => None,
        }
    }

    /// The value of a field of a record, or of a value of an enum that has fields, by its declared
    /// name.
    pub fn field(&self, name: &str) -> Option<&Value> {
        match self {
            Value::Record(fs) | Value::Variant(_, fs) => fs.iter().find(|(n, _)| n == name).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The name of a value of an enum, whether it has fields or not.
    pub fn tag(&self) -> Option<&str> {
        match self {
            Value::Enum(v) | Value::Variant(v, _) => Some(v),
            _ => None,
        }
    }
}

/// Everything a module reaches, by the file on the disk: itself and the modules it reads, and the
/// rules, the dates files and the books any of them reads.
pub struct Program {
    pub root: Rc<Checked>,
    pub modules: BTreeMap<PathBuf, Rc<Checked>>,
    pub rules: BTreeMap<PathBuf, Rc<RuleFacts>>,
    pub dates: BTreeMap<PathBuf, Rc<DateFacts>>,
    pub books: BTreeMap<PathBuf, Rc<BookFacts>>,
    /// A rule, a dates file or a book on the disk, as its port is asked about it.
    files: BTreeMap<PathBuf, PathBuf>,
}

impl Program {
    pub fn of(root: &Rc<Checked>) -> Program {
        let mut modules = root.world.modules.clone();
        modules.insert(root.canon.clone(), root.clone());
        let mut files = BTreeMap::new();
        for m in modules.values() {
            for t in &m.uses {
                match t {
                    Target::Rule { file, canon, .. } | Target::Dates { file, canon, .. } | Target::Book { file, canon, .. } => {
                        files.entry(canon.clone()).or_insert_with(|| file.clone());
                    }
                    Target::Module(_) => {}
                }
            }
        }
        Program { root: root.clone(), modules, rules: root.world.rules.clone(), dates: root.world.dates.clone(), books: root.world.books.clone(), files }
    }

    pub fn module(&self, canon: &Path) -> &Rc<Checked> {
        self.modules.get(canon).unwrap_or(&self.root)
    }

    /// The path a port is asked about a file with.
    pub fn file(&self, canon: &Path) -> PathBuf {
        self.files.get(canon).cloned().unwrap_or_else(|| canon.to_path_buf())
    }

    /// An enum a module of the program declares, with its values' fields.
    pub fn enum_info(&self, id: &EnumId) -> Option<&crate::names::EnumInfo> {
        let Owner::Module(file) = &id.owner else { return None };
        self.modules.get(file)?.decls.enums.iter().find(|e| e.name.text == id.name)
    }

    /// Whether an enum's values have fields (DESIGN 23.1).
    pub fn is_sum(&self, id: &EnumId) -> bool {
        self.enum_info(id).is_some_and(|e| e.is_sum())
    }

    /// Whether a value of `t` is one of a record or an enum that holds itself (DESIGN 28.1): a record
    /// of a module, an enum whose values have fields, or the type of one of its values, declared in a
    /// cycle of the module's declarations.
    pub fn recursive(&self, t: &Ty) -> bool {
        match t {
            Ty::Record(RecId::Module { file, name, .. }) => self.modules.get(file).and_then(|c| c.decls.records.iter().find(|r| r.name.text == *name)).is_some_and(|r| r.recursive),
            Ty::Record(r @ RecId::Variant { .. }) => r.sum().is_some_and(|id| self.enum_info(&id).is_some_and(|e| e.recursive)),
            Ty::Enum(id) => self.enum_info(id).is_some_and(|e| e.recursive),
            _ => false,
        }
    }

    /// Whether a value of `t` can hold a value of a record or an enum that holds itself (DESIGN 28.1):
    /// one itself, or through its elements, its fields and its values' fields.
    pub fn reaches_recursive(&self, t: &Ty) -> bool {
        self.reaches_in(t, &mut Vec::new())
    }

    fn reaches_in(&self, t: &Ty, seen: &mut Vec<Ty>) -> bool {
        if self.recursive(t) {
            return true;
        }
        match t {
            Ty::Opt(x) | Ty::List(x) | Ty::Set(x) | Ty::Record(RecId::Indexed(x)) => self.reaches_in(x, seen),
            Ty::Dict(a, b) | Ty::Result(a, b) | Ty::Record(RecId::Running(a, b) | RecId::Entry(a, b)) => self.reaches_in(a, seen) || self.reaches_in(b, seen),
            Ty::Record(id @ (RecId::Module { .. } | RecId::Variant { .. })) => {
                if seen.contains(t) {
                    return false;
                }
                seen.push(t.clone());
                self.fields(id).iter().any(|(_, s)| self.reaches_in(&s.ty, seen))
            }
            Ty::Enum(id) if self.is_sum(id) => {
                if seen.contains(t) {
                    return false;
                }
                seen.push(t.clone());
                self.values(id).0.iter().any(|v| self.value_fields(id, &v.text).iter().any(|(_, s)| self.reaches_in(&s.ty, seen)))
            }
            _ => false,
        }
    }

    /// How deep a value of `t` nests the values of records and enums that hold themselves (DESIGN
    /// 28.1): the most of them along a path from it, itself counted.
    pub fn nesting(&self, v: &Value, t: &Ty) -> usize {
        let mut t = t;
        while let Ty::Opt(x) = t {
            t = x;
        }
        if matches!(v, Value::None) {
            return 0;
        }
        let own = usize::from(self.recursive(t));
        let fields = |slots: Vec<(crate::ast::Name, Slot)>, fs: &[(String, Value)]| slots.iter().filter_map(|(f, s)| fs.iter().find(|(n, _)| *n == f.text).map(|(_, x)| self.nesting(x, &s.ty))).max().unwrap_or(0);
        own + match (t, v) {
            (Ty::List(x) | Ty::Set(x), Value::List(xs) | Value::Set(xs)) => xs.iter().map(|e| self.nesting(e, x)).max().unwrap_or(0),
            (Ty::Dict(k, x), Value::Dict(ps)) => ps.iter().map(|(a, b)| self.nesting(a, k).max(self.nesting(b, x))).max().unwrap_or(0),
            (Ty::Record(id), Value::Record(fs)) => fields(self.fields(id), fs),
            (Ty::Result(a, e), Value::Variant(tag, fs)) => fs.first().map(|(_, x)| self.nesting(x, if tag == "ok" { a } else { e })).unwrap_or(0),
            (Ty::Enum(id), Value::Variant(tag, fs)) => fields(self.value_fields(id, tag), fs),
            (Ty::Record(r @ RecId::Variant { .. }), Value::Variant(tag, fs)) => fields(r.sum().map(|id| self.value_fields(&id, tag)).unwrap_or_default(), fs),
            _ => 0,
        }
    }

    /// How deep the JSON of a value of `t` can nest its arrays and objects (DESIGN 11.1, 28.1), its
    /// values of records and enums that hold themselves nested `nested` deep at most along a path: an
    /// object for a record, `{"<value>": {…}}` for a value with fields, an array for a list and a set,
    /// two for a dict (`[[key, value], …]`), an object for a result, nothing more for an optional.
    /// What `run` reads a host's argument to.
    pub fn json_depth(&self, t: &Ty, nested: usize) -> usize {
        self.json_depth_in(t, nested, &mut std::collections::HashMap::new())
    }

    fn json_depth_in(&self, t: &Ty, left: usize, seen: &mut std::collections::HashMap<(Ty, usize), usize>) -> usize {
        let mut t = t;
        while let Ty::Opt(x) = t {
            t = x;
        }
        if let Some(d) = seen.get(&(t.clone(), left)) {
            return *d;
        }
        // a value of a type that holds itself is one deeper; none can be past the last
        let left_in = if self.recursive(t) {
            match left.checked_sub(1) {
                Some(l) => l,
                None => return 0,
            }
        } else {
            left
        };
        let fields = |me: &Self, slots: Vec<(crate::ast::Name, Slot)>, seen: &mut std::collections::HashMap<(Ty, usize), usize>| slots.iter().map(|(_, s)| me.json_depth_in(&s.ty, left_in, seen)).max().unwrap_or(0);
        let d = match t {
            Ty::List(x) | Ty::Set(x) => 1 + self.json_depth_in(x, left_in, seen),
            Ty::Dict(k, v) => 2 + self.json_depth_in(k, left_in, seen).max(self.json_depth_in(v, left_in, seen)),
            Ty::Result(a, e) => 1 + self.json_depth_in(a, left_in, seen).max(self.json_depth_in(e, left_in, seen)),
            Ty::Enum(id) if self.is_sum(id) => {
                let values = self.values(id).0;
                values.iter().map(|v| self.value_fields(id, &v.text)).filter(|fs| !fs.is_empty()).map(|fs| 2 + fields(self, fs, seen)).max().unwrap_or(0)
            }
            Ty::Record(r @ RecId::Variant { value, .. }) => match r.sum() {
                Some(id) => 2 + fields(self, self.value_fields(&id, value), seen),
                None => 0,
            },
            Ty::Record(id) => 1 + fields(self, self.fields(id), seen),
            _ => 0,
        };
        seen.insert((t.clone(), left), d);
        d
    }

    /// The type a value of a record or an enum that holds itself is counted as where such values nest
    /// in each other (DESIGN 28.1), its type arguments with it: a value of an enum and the type of one of
    /// its values are one. None for any other type.
    pub fn nest_key(&self, t: &Ty) -> Option<Ty> {
        if !self.recursive(t) {
            return None;
        }
        Some(match t {
            Ty::Record(r @ RecId::Variant { .. }) => Ty::Enum(r.sum()?),
            t => t.clone(),
        })
    }

    /// The fields of a value of an enum: each its name and its place, the enum's type arguments put in
    /// for its type parameters (DESIGN 26.1); none for a value with none.
    pub fn value_fields(&self, id: &EnumId, value: &str) -> Vec<(crate::ast::Name, Slot)> {
        let Some(e) = self.enum_info(id) else { return Vec::new() };
        let ps = crate::names::tnames(&e.tparams);
        e.fields_of(value).map(|fs| fs.iter().map(|f| (f.name.clone(), subst_slot(&f.slot, &ps, &id.args))).collect()).unwrap_or_default()
    }

    /// The fields of a record: each its name (with its alias) and its place.
    pub fn fields(&self, id: &RecId) -> Vec<(crate::ast::Name, Slot)> {
        let named = |n: &str, alias: &str| crate::ast::Name { text: n.to_string(), alias: (alias != n).then(|| (alias.to_string(), crate::ast::Span::default())), span: crate::ast::Span::default() };
        match id {
            RecId::Module { file, name, args } => self
                .modules
                .get(file)
                .and_then(|c| c.decls.records.iter().find(|r| r.name.text == *name))
                .map(|r| {
                    let ps = crate::names::tnames(&r.tparams);
                    r.fields.iter().map(|f| (f.name.clone(), subst_slot(&f.slot, &ps, args))).collect()
                })
                .unwrap_or_default(),
            RecId::Variant { value, .. } => id.sum().map(|e| self.value_fields(&e, value)).unwrap_or_default(),
            RecId::Outputs(file) => self.rules.get(file).map(|f| f.outputs.iter().map(|c| (named(&c.name, &c.alias), column_slot(c, file))).collect()).unwrap_or_default(),
            RecId::Element(file, seq) => self
                .rules
                .get(file)
                .and_then(|f| f.elements.iter().find(|(n, _, _)| n == seq))
                .map(|(_, _, cols)| cols.iter().map(|c| (named(&c.name, &c.alias), column_slot(c, file))).collect())
                .unwrap_or_default(),
            RecId::Running(item, weight) => ["item", "before", "upto", "total"]
                .iter()
                .map(|w| (named(w, w), Slot::of(if *w == "item" { (**item).clone() } else { (**weight).clone() })))
                .collect(),
            RecId::Entry(k, v) => vec![(named("key", "key"), Slot::of((**k).clone())), (named("value", "value"), Slot::of((**v).clone()))],
            RecId::Indexed(x) => vec![(named("index", "index"), Slot::of(Ty::Num(NumTy::number()))), (named("item", "item"), Slot::of((**x).clone()))],
        }
    }

    /// The order of two keys of type `t` (DESIGN 24.1): numbers from the least, dates from the
    /// earliest, strings by their code points, `false` before `true`, the values of an enum in the
    /// order declared; a record (a key of a sort) field by field, in the order its fields are declared.
    pub fn key_cmp(&self, t: &Ty, a: &Value, b: &Value) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (a, b) {
            (Value::Num(x), Value::Num(y)) => num::cmp(*x, *y),
            (Value::Date(x), Value::Date(y)) => x.cmp(y),
            // a string's bytes are UTF-8, whose order is the order of the code points
            (Value::Str(x), Value::Str(y)) => x.cmp(y),
            (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
            (Value::Enum(x), Value::Enum(y)) => match t.present() {
                Ty::Enum(id) => {
                    let (vs, _) = self.values(id);
                    let rank = |v: &str| vs.iter().position(|n| n.text == v).unwrap_or(usize::MAX);
                    rank(x).cmp(&rank(y))
                }
                _ => x.cmp(y),
            },
            (Value::Record(xs), Value::Record(ys)) => {
                let fields = match t.present() {
                    Ty::Record(id) => self.fields(id),
                    _ => Vec::new(),
                };
                for (f, slot) in &fields {
                    let (Some(x), Some(y)) = (a.field(&f.text), b.field(&f.text)) else { continue };
                    let o = self.key_cmp(&slot.ty, x, y);
                    if o != Ordering::Equal {
                        return o;
                    }
                }
                let _ = (xs, ys);
                Ordering::Equal
            }
            _ => Ordering::Equal,
        }
    }

    /// The pairs of a dict in the order of their keys, the later of two pairs of one key kept.
    pub fn dict_of(&self, k: &Ty, pairs: Vec<(Value, Value)>) -> Vec<(Value, Value)> {
        let mut out: Vec<(Value, Value)> = Vec::with_capacity(pairs.len());
        for (key, v) in pairs {
            match out.binary_search_by(|(x, _)| self.key_cmp(k, x, &key)) {
                Ok(i) => out[i].1 = v,
                Err(i) => out.insert(i, (key, v)),
            }
        }
        out
    }

    /// The members of a set in the order of their keys, each once.
    pub fn set_of(&self, t: &Ty, xs: Vec<Value>) -> Vec<Value> {
        let mut out: Vec<Value> = Vec::with_capacity(xs.len());
        for x in xs {
            if let Err(i) = out.binary_search_by(|y| self.key_cmp(t, y, &x)) {
                out.insert(i, x);
            }
        }
        out
    }

    /// The values of an enum, by their names and aliases, and whether they are in order.
    pub fn values(&self, id: &EnumId) -> (Vec<crate::ast::Name>, bool) {
        match &id.owner {
            Owner::Module(file) => self
                .modules
                .get(file)
                .and_then(|c| c.decls.enums.iter().find(|e| e.name.text == id.name))
                .map(|e| (e.values.clone(), e.ordered))
                .unwrap_or_default(),
            Owner::Rule(file) => self
                .rules
                .get(file)
                .and_then(|f| f.enums.iter().find(|e| e.name == id.name))
                .map(|e| {
                    let vs = e
                        .values
                        .iter()
                        .map(|v| crate::ast::Name { text: v.name.clone(), alias: (v.public != v.name).then(|| (v.public.clone(), crate::ast::Span::default())), span: crate::ast::Span::default() })
                        .collect();
                    (vs, e.ordered)
                })
                .unwrap_or_default(),
        }
    }

    /// The transfer a call builds, and the type of each of its arguments: a string, or an amount
    /// counted in the book's unit.
    pub fn transfer(&self, id: &TransferId) -> Option<(&BookFacts, Vec<(String, Ty)>)> {
        let book = self.books.get(&id.book)?;
        let t = book.transfers.iter().find(|t| t.name == id.transfer)?;
        let params = t
            .params
            .iter()
            .map(|p| {
                let ty = match &p.unit {
                    None => Ty::Str,
                    Some(u) => book_unit(book, u).map(|u| Ty::Num(NumTy::of(u))).unwrap_or(Ty::Error),
                };
                (p.name.clone(), ty)
            })
            .collect();
        Some((book, params))
    }

    /// The `use` name module `m` reads a file by.
    fn use_of<'m>(&self, m: &'m Checked, file: &Path) -> Option<&'m str> {
        m.module.uses.iter().zip(&m.uses).find(|(_, t)| t.canon() == file).map(|(u, _)| u.name.as_str())
    }

    fn named_by(&self, m: &Checked, owner: &Owner, name: &str) -> String {
        match owner {
            Owner::Module(f) if *f == m.canon => name.to_string(),
            Owner::Module(f) => match self.use_of(m, f) {
                Some(u) => format!("{u}.{name}"),
                None => self.modules.get(f).map(|c| format!("{}.{name}", c.module.name.text)).unwrap_or_else(|| name.to_string()),
            },
            Owner::Rule(f) => match self.use_of(m, f) {
                Some(u) => format!("{u}.{name}"),
                None => self.rules.get(f).map(|r| format!("{}.{name}", r.rule)).unwrap_or_else(|| name.to_string()),
            },
        }
    }

    /// A type as module `m` writes it.
    pub fn show_ty(&self, m: &Checked, t: &Ty) -> String {
        match t {
            Ty::Num(n) => unit_text(&n.unit),
            Ty::Bool => "bool".into(),
            Ty::Str => "string".into(),
            Ty::Date => "date".into(),
            Ty::Enum(id) => crate::names::with_args(self.named_by(m, &id.owner, &id.name), &id.args.iter().map(|a| self.show_ty(m, a)).collect::<Vec<_>>()),
            Ty::Record(RecId::Module { file, name, args }) => crate::names::with_args(self.named_by(m, &Owner::Module(file.clone()), name), &args.iter().map(|a| self.show_ty(m, a)).collect::<Vec<_>>()),
            Ty::Record(RecId::Variant { file, name, value, args }) => {
                crate::names::with_args(format!("{}.{value}", self.named_by(m, &Owner::Module(file.clone()), name)), &args.iter().map(|a| self.show_ty(m, a)).collect::<Vec<_>>())
            }
            Ty::Param(n) => n.clone(),
            Ty::Result(a, e) => format!("result[{}, {}]", self.show_ty(m, a), self.show_ty(m, e)),
            Ty::Record(RecId::Outputs(f)) => self.named_by(m, &Owner::Rule(f.clone()), "outputs"),
            Ty::Record(RecId::Element(f, s)) => self.named_by(m, &Owner::Rule(f.clone()), s),
            Ty::Record(RecId::Running(a, b)) => format!("running[{}, {}]", self.show_ty(m, a), self.show_ty(m, b)),
            Ty::Record(RecId::Entry(a, b)) => format!("entry[{}, {}]", self.show_ty(m, a), self.show_ty(m, b)),
            Ty::Record(RecId::Indexed(a)) => format!("indexed[{}]", self.show_ty(m, a)),
            Ty::List(x) => format!("list[{}]", self.show_ty(m, x)),
            Ty::Dict(k, v) => format!("dict[{}, {}]", self.show_ty(m, k), self.show_ty(m, v)),
            Ty::Set(x) => format!("set[{}]", self.show_ty(m, x)),
            Ty::EmptyDict => "[:]".into(),
            Ty::Opt(x) => format!("{}?", self.show_ty(m, x)),
            Ty::Transfer(id) => format!("{}.{}.{}", self.book_word(m, &id.book), id.transfer, id.op),
            Ty::None => "none".into(),
            Ty::Empty => "[]".into(),
            Ty::Fn(ps, r, eff) => fn_text(&ps.iter().map(|p| self.show_ty(m, p)).collect::<Vec<_>>(), &self.show_ty(m, r), eff.as_ref()),
            Ty::Error => "?".into(),
        }
    }

    /// A book as module `m` names it: its `use` name, else its own.
    fn book_word(&self, m: &Checked, book: &Path) -> String {
        self.use_of(m, book).map(str::to_string).unwrap_or_else(|| self.books.get(book).map(|b| b.name.clone()).unwrap_or_default())
    }

    // ── How a person reads a value ──────────────────────────────────────────────

    /// A value as module `m` would write it: `1150GBPc`, `0.5%`, `2026-04-01`, `Claim { due: … }`,
    /// `ledger.capture.do(payment: "pay_1", …)`. A rule's outputs are `{ fee: 398USDc }`.
    pub fn show(&self, m: &Checked, v: &Value, t: &Ty) -> String {
        match (v, t) {
            (Value::None, _) => "none".into(),
            // never shown where the check holds (DESIGN 27.1): a function as the type says it
            (Value::Closure(_), _) => format!("<{}>", self.show_ty(m, t)),
            (v, Ty::Opt(inner)) => self.show(m, v, inner),
            (Value::Num(r), Ty::Num(n)) => num_text(*r, &n.unit),
            (Value::Num(r), _) => r.to_string(),
            (Value::Bool(b), _) => b.to_string(),
            (Value::Str(s), _) => format!("\"{s}\""),
            (Value::Date(d), _) => crate::date::text(*d),
            (Value::Enum(e), _) => e.clone(),
            // a value of a `result`, as it is built: `ok(398USDc)`, `err("no item")` (DESIGN 26.3)
            (Value::Variant(v, fs), Ty::Result(a, e)) => {
                let inner = if v == "ok" { a } else { e };
                format!("{v}({})", fs.first().map(|(_, x)| self.show(m, x, inner)).unwrap_or_default())
            }
            // a value with fields, as it is built: `Order.placed { items: […], on: 2026-10-21 }`
            (Value::Variant(v, fs), t) => {
                let id = match t {
                    Ty::Enum(id) => Some(id.clone()),
                    Ty::Record(r) => r.sum(),
                    _ => None,
                };
                let slots = id.as_ref().map(|i| self.value_fields(i, v)).unwrap_or_default();
                let body = fs
                    .iter()
                    .map(|(n, x)| {
                        let ft = slots.iter().find(|(f, _)| f.text == *n).map(|(_, s)| s.ty.clone()).unwrap_or(Ty::Error);
                        format!("{n}: {}", self.show(m, x, &ft))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let head = match &id {
                    Some(i) => format!("{}.{v}", self.named_by(m, &i.owner, &i.name)),
                    None => v.clone(),
                };
                format!("{head} {{ {body} }}")
            }
            (Value::List(xs), t) => {
                let inner = match t {
                    Ty::List(x) => (**x).clone(),
                    _ => Ty::Error,
                };
                format!("[{}]", xs.iter().map(|x| self.show(m, x, &inner)).collect::<Vec<_>>().join(", "))
            }
            // a dict as its literal is written, `["a": 1, "b": 2]`; a set as it is made from a list
            (Value::Dict(ps), t) => {
                let (kt, vt) = match t {
                    Ty::Dict(k, v) => ((**k).clone(), (**v).clone()),
                    _ => (Ty::Error, Ty::Error),
                };
                if ps.is_empty() {
                    return "[:]".into();
                }
                format!("[{}]", ps.iter().map(|(k, v)| format!("{}: {}", self.show(m, k, &kt), self.show(m, v, &vt))).collect::<Vec<_>>().join(", "))
            }
            (Value::Set(xs), t) => {
                let inner = match t {
                    Ty::Set(x) => (**x).clone(),
                    _ => Ty::Error,
                };
                format!("[{}].to_set()", xs.iter().map(|x| self.show(m, x, &inner)).collect::<Vec<_>>().join(", "))
            }
            (Value::Record(fs), Ty::Record(id)) => {
                let slots = self.fields(id);
                let body = fs
                    .iter()
                    .map(|(n, x)| {
                        let ft = slots.iter().find(|(f, _)| f.text == *n).map(|(_, s)| s.ty.clone()).unwrap_or(Ty::Error);
                        format!("{n}: {}", self.show(m, x, &ft))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                match id {
                    RecId::Outputs(_) | RecId::Entry(..) | RecId::Indexed(_) => format!("{{ {body} }}"),
                    _ => format!("{} {{ {body} }}", self.show_ty(m, t)),
                }
            }
            (Value::Record(fs), _) => format!("{{ {} }}", fs.iter().map(|(n, x)| format!("{n}: {}", self.show(m, x, &Ty::Error))).collect::<Vec<_>>().join(", ")),
            (Value::Transfer(x), _) => {
                let params = self.transfer(&x.id).map(|(_, p)| p).unwrap_or_default();
                let args = x
                    .args
                    .iter()
                    .map(|(n, a)| {
                        let at = params.iter().find(|(p, _)| p == n).map(|(_, t)| t.clone()).unwrap_or(Ty::Error);
                        format!("{n}: {}", self.show(m, a, &at))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}.{}.{}({args})", self.book_word(m, &x.id.book), x.id.transfer, x.id.op)
            }
        }
    }

    // ── The wire ────────────────────────────────────────────────────────────────

    /// A value on the wire (DESIGN 9). A number that is not whole on the wire (an unrounded value,
    /// which only a `let` holds) is written as a person reads it, in a string.
    pub fn to_json(&self, v: &Value, t: &Ty) -> Json {
        match (v, t) {
            (Value::None, _) => Json::Null,
            // a function value is never put on the wire where the check holds (DESIGN 27.1)
            (Value::Closure(_), _) => Json::Null,
            (v, Ty::Opt(inner)) => self.to_json(v, inner),
            (Value::Num(r), Ty::Num(n)) => match wire_int(*r, n) {
                Some(i) => Json::Int(i),
                None => Json::Str(num_text(*r, &n.unit)),
            },
            (Value::Num(r), _) => match r.is_int() {
                true => Json::Int(r.num),
                false => Json::Str(r.to_string()),
            },
            (Value::Bool(b), _) => Json::Bool(*b),
            (Value::Str(s), _) => Json::str(s.clone()),
            (Value::Date(d), _) => Json::str(crate::date::text(*d)),
            (Value::Enum(e), _) => Json::str(e.clone()),
            // a value of a `result`: `{"ok": value}` or `{"err": value}` (DESIGN 26.4)
            (Value::Variant(v, fs), Ty::Result(a, e)) => {
                let inner = if v == "ok" { a } else { e };
                Json::obj([(v.clone(), fs.first().map(|(_, x)| self.to_json(x, inner)).unwrap_or(Json::Null))])
            }
            // a value with fields: an object of one pair, its name and its fields (DESIGN 23.6)
            (Value::Variant(v, fs), t) => {
                let id = match t {
                    Ty::Enum(id) => Some(id.clone()),
                    Ty::Record(r) => r.sum(),
                    _ => None,
                };
                let slots = id.as_ref().map(|i| self.value_fields(i, v)).unwrap_or_default();
                let fields = Json::obj(fs.iter().map(|(n, x)| {
                    let ft = slots.iter().find(|(f, _)| f.text == *n).map(|(_, s)| s.ty.clone()).unwrap_or(Ty::Error);
                    (n.clone(), self.to_json(x, &ft))
                }));
                Json::obj([(v.clone(), fields)])
            }
            (Value::List(xs), t) => {
                let inner = match t {
                    Ty::List(x) => (**x).clone(),
                    _ => Ty::Error,
                };
                Json::arr(xs.iter().map(|x| self.to_json(x, &inner)))
            }
            // a dict as the array of its pairs, each `[key, value]`, a set as the array of its members,
            // both in the order of their keys (DESIGN 24.4)
            (Value::Dict(ps), t) => {
                let (kt, vt) = match t {
                    Ty::Dict(k, v) => ((**k).clone(), (**v).clone()),
                    _ => (Ty::Error, Ty::Error),
                };
                Json::arr(ps.iter().map(|(k, v)| Json::arr([self.to_json(k, &kt), self.to_json(v, &vt)])))
            }
            (Value::Set(xs), t) => {
                let inner = match t {
                    Ty::Set(x) => (**x).clone(),
                    _ => Ty::Error,
                };
                Json::arr(xs.iter().map(|x| self.to_json(x, &inner)))
            }
            (Value::Record(fs), Ty::Record(id)) => {
                let slots = self.fields(id);
                Json::obj(fs.iter().map(|(n, x)| {
                    let ft = slots.iter().find(|(f, _)| f.text == *n).map(|(_, s)| s.ty.clone()).unwrap_or(Ty::Error);
                    (n.clone(), self.to_json(x, &ft))
                }))
            }
            (Value::Record(fs), _) => Json::obj(fs.iter().map(|(n, x)| (n.clone(), self.to_json(x, &Ty::Error)))),
            (Value::Transfer(x), _) => {
                let (book, params) = self.transfer(&x.id).map(|(b, p)| (b.name.clone(), p)).unwrap_or_default();
                let args = Json::obj(x.args.iter().map(|(n, a)| {
                    let at = params.iter().find(|(p, _)| p == n).map(|(_, t)| t.clone()).unwrap_or(Ty::Error);
                    (n.clone(), self.to_json(a, &at))
                }));
                Json::obj([("book", Json::str(book)), ("transfer", Json::str(x.id.transfer.clone())), ("op", Json::str(x.id.op.clone())), ("args", args)])
            }
        }
    }

    /// A value off the wire, read as a value of `t` (`path` names it in what is said of it): Err
    /// with what is wrong when it is not one. Its range is not looked at here.
    pub fn from_json(&self, m: &Checked, j: &Json, t: &Ty, path: &str) -> Result<Value, Text> {
        match t {
            Ty::Opt(inner) => {
                if j.is_null() {
                    Ok(Value::None)
                } else {
                    self.from_json(m, j, inner, path)
                }
            }
            Ty::Num(n) => self.num_json(m, j, t, n, path),
            Ty::Bool => j.as_bool().map(Value::Bool).ok_or_else(|| Self::wrong_json(j, path, "bool")),
            Ty::Str => j.as_str().map(|s| Value::Str(s.to_string())).ok_or_else(|| Self::wrong_json(j, path, "string")),
            Ty::Date => Self::date_json(j, path),
            Ty::Enum(id) if self.is_sum(id) => self.variant_json(m, j, id, t, path),
            Ty::Enum(id) => self.enum_json(m, j, id, t, path),
            // the type of a value of a sum: a value of the sum, that value
            Ty::Record(r @ RecId::Variant { value, .. }) => self.value_type_json(m, j, r, value, t, path),
            Ty::List(inner) => self.list_json(m, j, inner, t, path),
            // a dict's pairs and a set's members in any order, each key once (DESIGN 24.4)
            Ty::Dict(kt, vt) => self.dict_json(m, j, kt, vt, t, path),
            Ty::Set(inner) => self.set_json(m, j, inner, t, path),
            Ty::Record(id) => self.record_json(m, j, id, t, path),
            // `{"ok": value}` or `{"err": value}` (DESIGN 26.4)
            Ty::Result(a, e) => self.result_json(m, j, a, e, t, path),
            Ty::Transfer(id) => self.transfer_json(m, j, id, t, path),
            Ty::None | Ty::Empty | Ty::EmptyDict | Ty::Param(_) | Ty::Fn(..) | Ty::Error => Err(Self::wrong_json(j, path, "?")),
        }
    }

    /// A number, an amount, a quantity or a rate off the wire: a whole number, in int64 (a rate in its steps). One kind of type of [`Program::from_json`], kept out of it so that the recursion
    /// through a deep value holds a small frame for each level (DESIGN 28.1).
    #[inline(never)]
    fn num_json(&self, m: &Checked, j: &Json, t: &Ty, n: &NumTy, path: &str) -> Result<Value, Text> {
        let shown = self.show_ty(m, t);
        let i = match j {
            Json::Int(i) => *i,
            Json::Frac(x) => return Err(tr!("`{path}` の {x} は整数ではありません（{shown} は整数で渡します）", "`{path}` is {x}, no whole number ({shown} is given as a whole number)")),
            _ => return Err(Self::wrong_json(j, path, &shown)),
        };
        if i < i64::MIN as i128 || i > i64::MAX as i128 {
            return Err(tr!("`{path}` の {i} は int64 に収まりません", "`{path}` is {i}, which does not fit in int64"));
        }
        let v = match (n.is_rate(), n.step) {
            (true, Some(s)) => Rat::int(i).mul(s),
            _ => Rat::int(i),
        };
        num::held(Some(v)).map(Value::Num).map_err(|_| tr!("`{path}` の {i} は int64 に収まりません", "`{path}` is {i}, which does not fit in int64"))
    }

    /// A date off the wire: `YYYY-MM-DD`, from 0001-01-01 to 9999-12-31. One kind of type of [`Program::from_json`], kept out of it so that the recursion
    /// through a deep value holds a small frame for each level (DESIGN 28.1).
    #[inline(never)]
    fn date_json(j: &Json, path: &str) -> Result<Value, Text> {
        let s = j.as_str().ok_or_else(|| Self::wrong_json(j, path, "date"))?;
        crate::date::parse(s)
            .map(Value::Date)
            .ok_or_else(|| tr!("`{path}` の \"{s}\" は日付ではありません（`YYYY-MM-DD` で、0001-01-01 から 9999-12-31 まで）", "`{path}` is \"{s}\", no date (`YYYY-MM-DD`, from 0001-01-01 to 9999-12-31)"))
    }

    /// A value of an enum whose values have no fields, by its name or its alias. One kind of type of [`Program::from_json`], kept out of it so that the recursion
    /// through a deep value holds a small frame for each level (DESIGN 28.1).
    #[inline(never)]
    fn enum_json(&self, m: &Checked, j: &Json, id: &EnumId, t: &Ty, path: &str) -> Result<Value, Text> {
        let en = self.show_ty(m, t);
        let s = j.as_str().ok_or_else(|| Self::wrong_json(j, path, &en))?;
        let (vs, _) = self.values(id);
        match vs.iter().find(|v| v.is(s)) {
            Some(v) => Ok(Value::Enum(v.text.clone())),
            None => {
                let all = vs.iter().map(|v| v.text.as_str()).collect::<Vec<_>>().join(", ");
                Err(tr!("`{path}` の \"{s}\" は、列挙 `{en}` の値ではありません（値は {all}）", "`{path}` is \"{s}\", no value of the enum `{en}` (its values are {all})"))
            }
        }
    }

    /// A value of the type of one value of a sum (`Order.placed`): a value of the sum, that value. One kind of type of [`Program::from_json`], kept out of it so that the recursion
    /// through a deep value holds a small frame for each level (DESIGN 28.1).
    #[inline(never)]
    fn value_type_json(&self, m: &Checked, j: &Json, r: &RecId, value: &str, t: &Ty, path: &str) -> Result<Value, Text> {
        let id = r.sum().expect("a value's enum");
        let v = self.variant_json(m, j, &id, &Ty::Enum(id.clone()), path)?;
        if v.tag() == Some(value) {
            Ok(v)
        } else {
            let shown = self.show_ty(m, t);
            let got = v.tag().unwrap_or_default().to_string();
            Err(tr!("`{path}` は {shown} です。渡されたのは値 `{got}` です", "`{path}` is {shown}; it is given the value `{got}`"))
        }
    }

    /// A list: an array, each element read in its type. One kind of type of [`Program::from_json`], kept out of it so that the recursion
    /// through a deep value holds a small frame for each level (DESIGN 28.1).
    #[inline(never)]
    fn list_json(&self, m: &Checked, j: &Json, inner: &Ty, t: &Ty, path: &str) -> Result<Value, Text> {
        let xs = j.as_arr().ok_or_else(|| Self::wrong_json(j, path, &self.show_ty(m, t)))?;
        xs.iter().enumerate().map(|(i, x)| self.from_json(m, x, inner, &format!("{path}[{i}]"))).collect::<Result<Vec<_>, _>>().map(Value::List)
    }

    /// A dict: its pairs `[key, value]` in any order, each key once (DESIGN 24.4). One kind of type of [`Program::from_json`], kept out of it so that the recursion
    /// through a deep value holds a small frame for each level (DESIGN 28.1).
    #[inline(never)]
    fn dict_json(&self, m: &Checked, j: &Json, kt: &Ty, vt: &Ty, t: &Ty, path: &str) -> Result<Value, Text> {
        let shown = self.show_ty(m, t);
        let xs = j.as_arr().ok_or_else(|| Self::wrong_json(j, path, &shown))?;
        let mut pairs = Vec::with_capacity(xs.len());
        for (i, x) in xs.iter().enumerate() {
            let pair = x.as_arr().filter(|p| p.len() == 2).ok_or_else(|| {
                let got = x.kind();
                tr!("`{path}[{i}]` は、鍵と値の二つの配列（`[鍵, 値]`）です。渡されたのは{}です", "`{path}[{i}]` is an array of two, a key and its value (`[key, value]`); it is given {}", got.ja; got.en)
            })?;
            let k = self.from_json(m, &pair[0], kt, &format!("{path}[{i}][0]"))?;
            let v = self.from_json(m, &pair[1], vt, &format!("{path}[{i}][1]"))?;
            pairs.push((k, v));
        }
        let n = pairs.len();
        let out = self.dict_of(kt, pairs);
        if out.len() != n {
            let twice = self.twice(m, kt, xs.iter().filter_map(|x| x.as_arr().and_then(|p| p.first())).collect());
            return Err(tr!("`{path}` に、同じ鍵 {twice} が二度あります", "`{path}` has the key {twice} twice"));
        }
        Ok(Value::Dict(out))
    }

    /// A set: its members in any order, each once (DESIGN 24.4). One kind of type of [`Program::from_json`], kept out of it so that the recursion
    /// through a deep value holds a small frame for each level (DESIGN 28.1).
    #[inline(never)]
    fn set_json(&self, m: &Checked, j: &Json, inner: &Ty, t: &Ty, path: &str) -> Result<Value, Text> {
        let shown = self.show_ty(m, t);
        let xs = j.as_arr().ok_or_else(|| Self::wrong_json(j, path, &shown))?;
        let vals = xs.iter().enumerate().map(|(i, x)| self.from_json(m, x, inner, &format!("{path}[{i}]"))).collect::<Result<Vec<_>, _>>()?;
        let n = vals.len();
        let out = self.set_of(inner, vals);
        if out.len() != n {
            let twice = self.twice(m, inner, xs.iter().collect());
            return Err(tr!("`{path}` に、同じ要素 {twice} が二度あります", "`{path}` has the member {twice} twice"));
        }
        Ok(Value::Set(out))
    }

    /// A record: an object with a key for each field (its name or its alias), a field that may be none left out. One kind of type of [`Program::from_json`], kept out of it so that the recursion
    /// through a deep value holds a small frame for each level (DESIGN 28.1).
    #[inline(never)]
    fn record_json(&self, m: &Checked, j: &Json, id: &RecId, t: &Ty, path: &str) -> Result<Value, Text> {
        let shown = self.show_ty(m, t);
        let obj = j.as_obj().ok_or_else(|| Self::wrong_json(j, path, &shown))?;
        let fields = self.fields(id);
        for (k, _) in obj {
            if !fields.iter().any(|(f, _)| f.is(k)) {
                let all = fields.iter().map(|(f, _)| format!("`{}`", f.text)).collect::<Vec<_>>().join(", ");
                return Err(tr!("`{path}` の {shown} に、フィールド `{k}` はありません（フィールドは {all}）", "{shown} has no field `{k}` (`{path}`; its fields are {all})"));
            }
        }
        let mut out = Vec::new();
        for (f, slot) in &fields {
            let given: Vec<&Json> = obj.iter().filter(|(k, _)| f.is(k)).map(|(_, v)| v).collect();
            let fp = format!("{path}.{}", f.text);
            let v = match given.as_slice() {
                [] if matches!(slot.ty, Ty::Opt(_)) => Value::None,
                [] => {
                    let fname = &f.text;
                    return Err(tr!("`{path}` にフィールド `{fname}` がありません", "`{path}` has no field `{fname}`"));
                }
                [x] => self.from_json(m, x, &slot.ty, &fp)?,
                _ => {
                    let fname = &f.text;
                    return Err(tr!("`{path}` のフィールド `{fname}` が、名前と別名で二度渡されています", "`{path}` is given the field `{fname}` twice, by its name and by its alias"));
                }
            };
            out.push((f.text.clone(), v));
        }
        Ok(Value::Record(out))
    }

    /// A `result`: `{"ok": value}` or `{"err": value}` (DESIGN 26.4). One kind of type of [`Program::from_json`], kept out of it so that the recursion
    /// through a deep value holds a small frame for each level (DESIGN 28.1).
    #[inline(never)]
    fn result_json(&self, m: &Checked, j: &Json, a: &Ty, e: &Ty, t: &Ty, path: &str) -> Result<Value, Text> {
        let shown = self.show_ty(m, t);
        let Some(obj) = j.as_obj().filter(|o| o.len() == 1) else {
            let got = j.kind();
            return Err(tr!(
                "`{path}` は {shown} です。`{{\"ok\": 値}}` か `{{\"err\": 値}}` の形で渡します。渡されたのは{}です",
                "`{path}` is {shown}, given as `{{\"ok\": value}}` or `{{\"err\": value}}`; it is given {}",
                got.ja;
                got.en
            ));
        };
        let (k, x) = &obj[0];
        match k.as_str() {
            "ok" => Ok(Value::Variant("ok".into(), vec![("value".into(), self.from_json(m, x, a, &format!("{path}.ok"))?)])),
            "err" => Ok(Value::Variant("err".into(), vec![("error".into(), self.from_json(m, x, e, &format!("{path}.err"))?)])),
            _ => Err(tr!("`{path}` の \"{k}\" は、{shown} の値ではありません（値は ok と err）", "`{path}` is \"{k}\", no value of {shown} (its values are ok and err)")),
        }
    }

    /// A transfer: its book, transfer and operation, and its arguments. One kind of type of [`Program::from_json`], kept out of it so that the recursion
    /// through a deep value holds a small frame for each level (DESIGN 28.1).
    #[inline(never)]
    fn transfer_json(&self, m: &Checked, j: &Json, id: &TransferId, t: &Ty, path: &str) -> Result<Value, Text> {
        let shown = self.show_ty(m, t);
        let (book, params) = self.transfer(id).ok_or_else(|| Self::wrong_json(j, path, &shown))?;
        let at = |k: &str| j.get(k).and_then(Json::as_str);
        if at("book") != Some(book.name.as_str()) || at("transfer") != Some(id.transfer.as_str()) || at("op") != Some(id.op.as_str()) {
            return Err(Self::wrong_json(j, path, &shown));
        }
        let args = j.get("args").and_then(Json::as_obj).ok_or_else(|| Self::wrong_json(j, path, &shown))?;
        let mut out = Vec::new();
        for (p, pt) in &params {
            if let Some((_, x)) = args.iter().find(|(k, _)| k == p) {
                out.push((p.clone(), self.from_json(m, x, pt, &format!("{path}.args.{p}"))?));
            }
        }
        Ok(Value::Transfer(Box::new(TransferValue { id: id.clone(), args: out })))
    }

    /// What is said of a value off the wire that is not of the kind its place takes.
    #[cold]
    #[inline(never)]
    fn wrong_json(j: &Json, path: &str, want: &str) -> Text {
        let got = j.kind();
        tr!("`{path}` は {want} です。渡されたのは{}です", "`{path}` is {want}; it is given {}", got.ja; got.en)
    }

    /// The first key given twice among keys on the wire, as a person reads it.
    fn twice(&self, m: &Checked, t: &Ty, keys: Vec<&Json>) -> String {
        let mut seen: Vec<Value> = Vec::new();
        for k in keys {
            let Ok(v) = self.from_json(m, k, t, "") else { continue };
            if seen.iter().any(|x| self.key_cmp(t, x, &v) == std::cmp::Ordering::Equal) {
                return self.show(m, &v, t);
            }
            seen.push(v);
        }
        String::new()
    }
}

impl Program {
    /// A value of a sum off the wire (DESIGN 23.6): a value with no fields by its name, a value with
    /// fields as an object of one pair, its name and its fields.
    fn variant_json(&self, m: &Checked, j: &Json, id: &EnumId, t: &Ty, path: &str) -> Result<Value, Text> {
        let en = self.show_ty(m, t);
        let (vs, _) = self.values(id);
        let all = vs.iter().map(|v| v.text.as_str()).collect::<Vec<_>>().join(", ");
        let find = |s: &str| vs.iter().find(|v| v.is(s)).map(|v| v.text.clone());
        if let Some(s) = j.as_str() {
            let Some(v) = find(s) else {
                return Err(tr!("`{path}` の \"{s}\" は、列挙 `{en}` の値ではありません（値は {all}）", "`{path}` is \"{s}\", no value of the enum `{en}` (its values are {all})"));
            };
            if !self.value_fields(id, &v).is_empty() {
                return Err(tr!(
                    "`{path}` の \"{s}\" は、列挙 `{en}` のフィールドを持つ値です。`{{\"{v}\": {{…}}}}` の形で渡します",
                    "`{path}` is \"{s}\", a value of the enum `{en}` that has fields, given as `{{\"{v}\": {{…}}}}`"
                ));
            }
            return Ok(Value::Enum(v));
        }
        let Some(obj) = j.as_obj().filter(|o| o.len() == 1) else {
            let got = j.kind();
            return Err(tr!(
                "`{path}` は {en} です。値の名前か、名前とフィールドの一組のオブジェクトで渡します。渡されたのは{}です",
                "`{path}` is {en}, given as a value's name or an object of one pair, a value's name and its fields; it is given {}",
                got.ja;
                got.en
            ));
        };
        let (k, inner) = &obj[0];
        let Some(v) = find(k) else {
            return Err(tr!("`{path}` の \"{k}\" は、列挙 `{en}` の値ではありません（値は {all}）", "`{path}` is \"{k}\", no value of the enum `{en}` (its values are {all})"));
        };
        let slots = self.value_fields(id, &v);
        if slots.is_empty() {
            return Err(tr!("`{path}` の \"{v}\" はフィールドの無い値なので、名前だけで渡します", "`{path}` is \"{v}\", a value with no fields, given by its name alone"));
        }
        let fields = inner.as_obj().ok_or_else(|| {
            let got = inner.kind();
            tr!("`{path}.{v}` はフィールドのオブジェクトです。渡されたのは{}です", "`{path}.{v}` is an object of the fields; it is given {}", got.ja; got.en)
        })?;
        let shown = format!("{en}.{v}");
        for (k, _) in fields {
            if !slots.iter().any(|(f, _)| f.is(k)) {
                let names = slots.iter().map(|(f, _)| format!("`{}`", f.text)).collect::<Vec<_>>().join(", ");
                return Err(tr!("`{path}` の {shown} に、フィールド `{k}` はありません（フィールドは {names}）", "{shown} has no field `{k}` (`{path}`; its fields are {names})"));
            }
        }
        let mut out = Vec::new();
        for (f, slot) in &slots {
            let given: Vec<&Json> = fields.iter().filter(|(k, _)| f.is(k)).map(|(_, x)| x).collect();
            let fp = format!("{path}.{v}.{}", f.text);
            let x = match given.as_slice() {
                [] if matches!(slot.ty, Ty::Opt(_)) => Value::None,
                [] => {
                    let fname = &f.text;
                    return Err(tr!("`{path}` の {shown} にフィールド `{fname}` がありません", "`{path}`, {shown}, has no field `{fname}`"));
                }
                [x] => self.from_json(m, x, &slot.ty, &fp)?,
                _ => {
                    let fname = &f.text;
                    return Err(tr!("`{path}` のフィールド `{fname}` が、名前と別名で二度渡されています", "`{path}` is given the field `{fname}` twice, by its name and by its alias"));
                }
            };
            out.push((f.text.clone(), x));
        }
        Ok(Value::Variant(v, out))
    }
}

/// A number written as a literal of its unit: `1150GBPc`, `0.5%`, `30`; an unrounded one with its
/// fraction (`34.9USDc`, `1/3`).
pub fn num_text(r: Rat, unit: &Unit) -> String {
    match &unit.dim {
        Dim::Rate => format!("{}%", r.mul(Rat::int(100))),
        Dim::Number | Dim::Count(_) => r.to_string(),
        _ => format!("{r}{}", unit.unit),
    }
}

/// The whole number a value travels as on the wire: itself in its unit, or for a rate the count of
/// its type's step. None when it is not whole there.
pub fn wire_int(r: Rat, n: &NumTy) -> Option<i128> {
    let w = if n.is_rate() { r.checked_div(n.step?)? } else { r };
    w.is_int().then_some(w.num)
}

/// A range as kaname writes it: `>=1GBPc <=100000000GBPc`, `>=0% <=15%`, `>=2026-01-01`.
pub fn range_text(r: &Range, t: &Ty) -> String {
    let end = |v: Rat| -> String {
        match t {
            Ty::Num(n) => num_text(v, &n.unit),
            Ty::Date => crate::date::text(v.num as i64),
            _ => v.to_string(),
        }
    };
    let mut parts = Vec::new();
    if let Some(lo) = r.min {
        parts.push(format!(">={}", end(lo)));
    }
    if let Some(hi) = r.max {
        parts.push(format!("<={}", end(hi)));
    }
    parts.join(" ")
}
