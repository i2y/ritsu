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
    /// A value of an enum, by the name it is declared with.
    Enum(String),
    /// A record: each field by the name it is declared with, in the order declared.
    Record(Vec<(String, Value)>),
    List(Vec<Value>),
    /// The value that is not there. A value that is there is itself, not wrapped.
    None,
    /// A call of a transfer of a book, built to be made by the host (DESIGN 6.3).
    Transfer(Box<TransferValue>),
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

    /// The value of a field of a record, by its declared name.
    pub fn field(&self, name: &str) -> Option<&Value> {
        match self {
            Value::Record(fs) => fs.iter().find(|(n, _)| n == name).map(|(_, v)| v),
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

    /// The fields of a record: each its name (with its alias) and its place.
    pub fn fields(&self, id: &RecId) -> Vec<(crate::ast::Name, Slot)> {
        let named = |n: &str, alias: &str| crate::ast::Name { text: n.to_string(), alias: (alias != n).then(|| (alias.to_string(), crate::ast::Span::default())), span: crate::ast::Span::default() };
        match id {
            RecId::Module { file, name } => self
                .modules
                .get(file)
                .and_then(|c| c.decls.records.iter().find(|r| r.name.text == *name))
                .map(|r| r.fields.iter().map(|f| (f.name.clone(), f.slot.clone())).collect())
                .unwrap_or_default(),
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
        }
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
            Ty::Enum(id) => self.named_by(m, &id.owner, &id.name),
            Ty::Record(RecId::Module { file, name }) => self.named_by(m, &Owner::Module(file.clone()), name),
            Ty::Record(RecId::Outputs(f)) => self.named_by(m, &Owner::Rule(f.clone()), "outputs"),
            Ty::Record(RecId::Element(f, s)) => self.named_by(m, &Owner::Rule(f.clone()), s),
            Ty::Record(RecId::Running(a, b)) => format!("running[{}, {}]", self.show_ty(m, a), self.show_ty(m, b)),
            Ty::List(x) => format!("list[{}]", self.show_ty(m, x)),
            Ty::Opt(x) => format!("{}?", self.show_ty(m, x)),
            Ty::Transfer(id) => format!("{}.{}.{}", self.book_word(m, &id.book), id.transfer, id.op),
            Ty::None => "none".into(),
            Ty::Empty => "[]".into(),
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
            (v, Ty::Opt(inner)) => self.show(m, v, inner),
            (Value::Num(r), Ty::Num(n)) => num_text(*r, &n.unit),
            (Value::Num(r), _) => r.to_string(),
            (Value::Bool(b), _) => b.to_string(),
            (Value::Str(s), _) => format!("\"{s}\""),
            (Value::Date(d), _) => crate::date::text(*d),
            (Value::Enum(e), _) => e.clone(),
            (Value::List(xs), t) => {
                let inner = match t {
                    Ty::List(x) => (**x).clone(),
                    _ => Ty::Error,
                };
                format!("[{}]", xs.iter().map(|x| self.show(m, x, &inner)).collect::<Vec<_>>().join(", "))
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
                    RecId::Outputs(_) => format!("{{ {body} }}"),
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
            (Value::List(xs), t) => {
                let inner = match t {
                    Ty::List(x) => (**x).clone(),
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
        let wrong = |want: &str| -> Text {
            let got = j.kind();
            tr!("`{path}` は {want} です。渡されたのは{}です", "`{path}` is {want}; it is given {}", got.ja; got.en)
        };
        match t {
            Ty::Opt(inner) => {
                if j.is_null() {
                    Ok(Value::None)
                } else {
                    self.from_json(m, j, inner, path)
                }
            }
            Ty::Num(n) => {
                let shown = self.show_ty(m, t);
                let i = match j {
                    Json::Int(i) => *i,
                    Json::Frac(x) => return Err(tr!("`{path}` の {x} は整数ではありません（{shown} は整数で渡します）", "`{path}` is {x}, no whole number ({shown} is given as a whole number)")),
                    _ => return Err(wrong(&shown)),
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
            Ty::Bool => j.as_bool().map(Value::Bool).ok_or_else(|| wrong("bool")),
            Ty::Str => j.as_str().map(|s| Value::Str(s.to_string())).ok_or_else(|| wrong("string")),
            Ty::Date => {
                let s = j.as_str().ok_or_else(|| wrong("date"))?;
                crate::date::parse(s)
                    .map(Value::Date)
                    .ok_or_else(|| tr!("`{path}` の \"{s}\" は日付ではありません（`YYYY-MM-DD` で、0001-01-01 から 9999-12-31 まで）", "`{path}` is \"{s}\", no date (`YYYY-MM-DD`, from 0001-01-01 to 9999-12-31)"))
            }
            Ty::Enum(id) => {
                let en = self.show_ty(m, t);
                let s = j.as_str().ok_or_else(|| wrong(&en))?;
                let (vs, _) = self.values(id);
                match vs.iter().find(|v| v.is(s)) {
                    Some(v) => Ok(Value::Enum(v.text.clone())),
                    None => {
                        let all = vs.iter().map(|v| v.text.as_str()).collect::<Vec<_>>().join(", ");
                        Err(tr!("`{path}` の \"{s}\" は、列挙 `{en}` の値ではありません（値は {all}）", "`{path}` is \"{s}\", no value of the enum `{en}` (its values are {all})"))
                    }
                }
            }
            Ty::List(inner) => {
                let xs = j.as_arr().ok_or_else(|| wrong(&self.show_ty(m, t)))?;
                xs.iter().enumerate().map(|(i, x)| self.from_json(m, x, inner, &format!("{path}[{i}]"))).collect::<Result<Vec<_>, _>>().map(Value::List)
            }
            Ty::Record(id) => {
                let shown = self.show_ty(m, t);
                let obj = j.as_obj().ok_or_else(|| wrong(&shown))?;
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
            Ty::Transfer(id) => {
                let shown = self.show_ty(m, t);
                let (book, params) = self.transfer(id).ok_or_else(|| wrong(&shown))?;
                let at = |k: &str| j.get(k).and_then(Json::as_str);
                if at("book") != Some(book.name.as_str()) || at("transfer") != Some(id.transfer.as_str()) || at("op") != Some(id.op.as_str()) {
                    return Err(wrong(&shown));
                }
                let args = j.get("args").and_then(Json::as_obj).ok_or_else(|| wrong(&shown))?;
                let mut out = Vec::new();
                for (p, pt) in &params {
                    if let Some((_, x)) = args.iter().find(|(k, _)| k == p) {
                        out.push((p.clone(), self.from_json(m, x, pt, &format!("{path}.args.{p}"))?));
                    }
                }
                Ok(Value::Transfer(Box::new(TransferValue { id: id.clone(), args: out })))
            }
            Ty::None | Ty::Empty | Ty::Error => Err(wrong("?")),
        }
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
