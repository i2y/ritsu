//! What a module's names name (DESIGN 2.5, 2.6, 3.9): the files its `use` lines read, each through
//! its language's port (rulec, koyomi, chobo) or checked as a module of its own; and its
//! declarations, with their types resolved and what each declares of its values (a range, a count,
//! a step).
//!
//! - The files: E201 when one cannot be read or does not pass its language's check, with what that
//!   language says as notes; E202 when the run does not join the language (the binary of kaname's
//!   own crate); E203 when the modules read each other in a ring.
//! - The names: E005 when a name the generated code takes is not ASCII and has no alias, E006 when a
//!   name is given twice (a module's declarations and its `use` names are one space, and a builtin's
//!   name is taken), E007 when the generated code cannot take a name, E101 when a type names
//!   nothing.
//! - The declarations: E108 when a range, a count or a step is missing where one is needed, or does
//!   not hold (an empty range, a record that holds itself); E102 and E104 when an end of a range is
//!   not a value of its type. A range is needed on the numbers and dates of a function's parameters,
//!   and of every field a parameter reaches (through lists, optionals and other records); a field
//!   of a record only answered with may leave it out (DESIGN 3.9).

use crate::ast::*;
use crate::check::Checked;
use crate::diag::Diag;
use crate::kw;
use crate::ty::*;
use ritsu_base::text::Text;
use ritsu_ports::{BookFacts, Column, ColumnType, DateFacts, DateKind, RuleFacts};
use ritsu_units::{Dim, Rat, Unit};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// What a `use` line reads, once it is read.
#[derive(Clone)]
pub enum Target {
    /// A rule: the file as the port is asked with it, the file on the disk, and what rulec knows.
    Rule {
        file: PathBuf,
        canon: PathBuf,
        facts: Rc<RuleFacts>,
    },
    Dates {
        file: PathBuf,
        canon: PathBuf,
        facts: Rc<DateFacts>,
    },
    Book {
        file: PathBuf,
        canon: PathBuf,
        facts: Rc<BookFacts>,
    },
    /// Another module, checked.
    Module(Rc<Checked>),
}

impl Target {
    pub fn canon(&self) -> &Path {
        match self {
            Target::Rule { canon, .. } | Target::Dates { canon, .. } | Target::Book { canon, .. } => canon,
            Target::Module(c) => &c.canon,
        }
    }
}

/// Every file a module reaches, through its own `use` lines and those of the modules it reads, by
/// the file on the disk: what a type of another file is looked up in.
#[derive(Clone, Default)]
pub struct World {
    pub modules: BTreeMap<PathBuf, Rc<Checked>>,
    pub rules: BTreeMap<PathBuf, Rc<RuleFacts>>,
    pub dates: BTreeMap<PathBuf, Rc<DateFacts>>,
    pub books: BTreeMap<PathBuf, Rc<BookFacts>>,
}

impl World {
    pub fn add(&mut self, t: &Target) {
        match t {
            Target::Rule { canon, facts, .. } => {
                self.rules.insert(canon.clone(), facts.clone());
            }
            Target::Dates { canon, facts, .. } => {
                self.dates.insert(canon.clone(), facts.clone());
            }
            Target::Book { canon, facts, .. } => {
                self.books.insert(canon.clone(), facts.clone());
            }
            Target::Module(c) => {
                self.modules.insert(c.canon.clone(), c.clone());
                for (k, v) in &c.world.modules {
                    self.modules.insert(k.clone(), v.clone());
                }
                for (k, v) in &c.world.rules {
                    self.rules.insert(k.clone(), v.clone());
                }
                for (k, v) in &c.world.dates {
                    self.dates.insert(k.clone(), v.clone());
                }
                for (k, v) in &c.world.books {
                    self.books.insert(k.clone(), v.clone());
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct EnumInfo {
    pub name: Name,
    pub ordered: bool,
    pub values: Vec<Name>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct FieldInfo {
    pub name: Name,
    pub slot: Slot,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct RecordInfo {
    pub name: Name,
    pub fields: Vec<FieldInfo>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct ParamInfo {
    pub name: Name,
    pub slot: Slot,
}

#[derive(Clone, Debug)]
pub struct FnInfo {
    pub name: Name,
    pub params: Vec<ParamInfo>,
    pub ret: Slot,
    pub line: usize,
}

/// What a module declares, its types resolved, in the order written.
#[derive(Clone, Debug, Default)]
pub struct Decls {
    pub enums: Vec<EnumInfo>,
    pub records: Vec<RecordInfo>,
    pub fns: Vec<FnInfo>,
}

/// Where a file a module reads is: beside the module, the path written from its directory.
pub fn beside(module: &str, written: &str) -> PathBuf {
    let dir = Path::new(module).parent().unwrap_or(Path::new(""));
    if dir.as_os_str().is_empty() { PathBuf::from(written) } else { dir.join(written) }
}

/// A file as one is told from another: what the file system says, or as written.
pub fn canonical(p: &Path) -> PathBuf {
    ritsu_base::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// What a language said of a file it would not answer for, as notes: each with its code first and
/// its place after it (sekisho's way), the first three.
pub fn said_notes(said: &[ritsu_ports::Said]) -> Vec<Text> {
    said.iter()
        .take(3)
        .map(|s| {
            let at = match s.line {
                Some(l) => format!("{}:{l}", s.file),
                None => s.file.clone(),
            };
            let (ja, en) = (&s.message.ja, ritsu_base::text::capitalize(&s.message.en));
            match (s.code.as_str(), at.is_empty()) {
                ("", true) => s.message.clone(),
                ("", false) => tr!("{ja}（{at}）", "{en} ({at})"),
                (code, true) => tr!("{code}: {ja}", "{code}: {en}"),
                (code, false) => tr!("{code}: {ja}（{at}）", "{code}: {en} ({at})"),
            }
        })
        .collect()
}

/// The type of a rule's input, output or field of an element, as kaname holds it.
pub fn column_ty(t: &ColumnType, rule: &Path) -> Ty {
    match t {
        ColumnType::Bool => Ty::Bool,
        ColumnType::Str => Ty::Str,
        ColumnType::Date => Ty::Date,
        ColumnType::Enum { name: n, .. } => Ty::Enum(EnumId { owner: Owner::Rule(rule.to_path_buf()), name: n.clone() }),
        ColumnType::Num { unit: Some(u), .. } => Ty::Num(NumTy::of(u.clone())),
        ColumnType::Num { unit: None, .. } => Ty::Error,
        ColumnType::Opt(t) => Ty::Opt(Box::new(column_ty(t, rule))),
    }
}

/// The place a rule's input or field is: its type, and its range as values of its unit (the
/// integers on the wire are counted in steps for a rate).
pub fn column_slot(c: &Column, rule: &Path) -> Slot {
    let ty = column_ty(&c.ty, rule);
    let mut inner = &c.ty;
    while let ColumnType::Opt(t) = inner {
        inner = t;
    }
    let range = match inner {
        ColumnType::Num { unit: Some(u), min, max, .. } if min.is_some() || max.is_some() => {
            let step = unit_step(u);
            Some(Range { min: min.map(|v| Rat::int(v).mul(step)), max: max.map(|v| Rat::int(v).mul(step)) })
        }
        _ => None,
    };
    Slot { ty, range, count: None }
}

/// The values an input of a rule of an ordered enum lets in, where it holds the enum to a range
/// (`campaign : tier  range >=basic <=gold`, rulec's §15.210): the rule's names of them in the enum's
/// order, as the port carries them (ritsu's DESIGN 3.2). None for an input of any other type, an
/// input with no range, an output and a field of an element. A slot's range is one of numbers and
/// dates, which a module declares too; a range of an enum's values only a rule declares (E108), so it
/// stays with the rule's column, and the analysis of ranges holds the value given to it (DESIGN 5.4).
pub fn column_values(c: &Column) -> Option<Vec<String>> {
    let mut t = &c.ty;
    while let ColumnType::Opt(x) = t {
        t = x;
    }
    match t {
        ColumnType::Enum { range, .. } => range.clone(),
        _ => None,
    }
}

/// The place of an input of a dates file: a date, or a whole number, with its range.
pub fn date_input_slot(facts: &DateFacts, name: &str) -> Option<Slot> {
    let i = facts.inputs.iter().find(|i| i.name == name)?;
    let ty = match i.kind {
        DateKind::Date => Ty::Date,
        DateKind::Int => Ty::Num(NumTy::number()),
    };
    Some(Slot { ty, range: Some(Range { min: Some(Rat::int(i.min as i128)), max: Some(Rat::int(i.max as i128)) }), count: None })
}

/// The unit an amount parameter of a transfer is counted in, as kaname types it: the book's unit as
/// ritsu shares it (`unit USD scale 2` is `money[USDc]`), and a count with a name and nothing else
/// as a `number` (DESIGN 3.8).
pub fn book_unit(facts: &BookFacts, unit: &str) -> Option<Unit> {
    let u = facts.unit(unit)?;
    Some(match u.dim {
        Dim::Count(_) => Unit::number(),
        _ => u.clone(),
    })
}

/// The words the generated code cannot take as a name (E007): the reserved words of TypeScript's
/// strict mode, Python's keywords, Go's keywords, and kaname's own helpers.
pub fn taken_word(w: &str) -> bool {
    ritsu_emit::words::typescript::NAMES.contains(w) || ritsu_emit::words::python::KEYWORDS.contains(&w) || ritsu_emit::words::go::KEYWORDS.contains(&w) || kw::HELPERS.contains(&w)
}

/// The declarations of a module and the names they give, checked: what [`declare`] answers.
pub struct Declared {
    pub decls: Decls,
    pub diags: Vec<Diag>,
}

/// The checks of a module's declarations, over the files it reads.
pub struct Dc<'a> {
    pub path: &'a str,
    pub src: &'a str,
    pub canon: &'a Path,
    pub module: &'a Module,
    /// The `use` lines read, by their names: the line's place in the module and what it read.
    pub uses: &'a BTreeMap<String, (usize, Target)>,
    pub world: &'a World,
    pub diags: Vec<Diag>,
}

/// One end of a range, read.
type End = (Cmp, Rat, Span);

impl<'a> Dc<'a> {
    fn err(&mut self, code: &'static str, at: Span, msg: Text) -> &mut Diag {
        self.diags.push(Diag::at(code, self.path, at.line, at.col, msg).source(self.src));
        self.diags.last_mut().expect("just pushed")
    }

    fn at_line(&mut self, code: &'static str, line: usize, msg: Text) -> &mut Diag {
        self.diags.push(Diag::at(code, self.path, line, 0, msg).source(self.src));
        self.diags.last_mut().expect("just pushed")
    }

    // ── Names ───────────────────────────────────────────────────────────────────

    /// E005 and E007 for a name the generated code takes.
    fn public_name(&mut self, n: &Name, what: &Text) {
        match &n.alias {
            None if !ritsu_emit::ident::is_ascii_ident(&n.text) => {
                let t = &n.text;
                self.err("E005", n.span, tr!("{} `{t}` に、ASCII の別名がありません", "The {} `{t}` has no ASCII alias", what.ja; what.en)).notes.push(tr!(
                    "生成するコードに出る名前（モジュール、関数、引数、レコード、フィールド、列挙とその値）が ASCII でなければ、`{t}(name)` のように丸括弧で ASCII の別名を付けてください。",
                    "A name the generated code takes (a module, a function, a parameter, a record, a field, an enum and its values) that is not ASCII takes an ASCII alias in parentheses: `{t}(name)`."
                ));
            }
            Some((a, sp)) if !ritsu_emit::ident::is_ascii_ident(a) => {
                let sp = *sp;
                self.err("E005", sp, tr!("別名 `{a}` が ASCII の名前ではありません", "The alias `{a}` is not an ASCII name"))
                    .notes
                    .push(tr!("別名は、英字か `_` で始まり、英字・数字・`_` だけでできた名前にしてください。", "An alias starts with a letter or `_` and has only letters, digits and `_`."));
            }
            _ => {
                let a = n.ascii().to_string();
                if taken_word(&a) {
                    let sp = n.alias.as_ref().map(|(_, s)| *s).unwrap_or(n.span);
                    self.err("E007", sp, tr!("生成するコードは、`{a}` を名前に使えません", "The generated code cannot take `{a}` as a name")).notes.push(tr!(
                        "`{a}` は、生成する TypeScript、Python、Go のどれかの予約語か、kaname の補助のモジュールの名前です。別の名前か別名にしてください。",
                        "`{a}` is a reserved word of the TypeScript, Python or Go the module is generated in, or a name of kaname's helper module; choose another name or alias."
                    ));
                }
            }
        }
    }

    /// E006 for names given twice in one space: by the name and by the alias.
    fn unique<'n>(&mut self, names: impl IntoIterator<Item = (&'n Name, Text)>) {
        let mut seen: BTreeMap<String, (Text, usize)> = BTreeMap::new();
        for (n, what) in names {
            let mut keys = vec![n.text.clone()];
            if let Some((a, _)) = &n.alias
                && a != &n.text
            {
                keys.push(a.clone());
            }
            for k in keys {
                if let Some((first, line)) = seen.get(&k).cloned() {
                    self.err(
                        "E006",
                        n.span,
                        tr!("`{k}` が二度あります（{line} 行目の{}と、この{}）", "`{k}` is given twice (the {} on line {line}, and this {})", first.ja, what.ja; first.en, what.en),
                    )
                    .notes
                    .push(tr!("どちらかの名前を替えてください。別名も名前と同じく、ほかと重なれません。", "Rename one of them; an alias may not be another's name or alias either."));
                    break;
                }
                seen.insert(k, (what.clone(), n.span.line));
            }
        }
    }

    /// The names of the module: one space for its `use` names and its declarations.
    pub fn names(&mut self) {
        let m = self.module;
        self.public_name(&m.name, &tr!("モジュール", "module"));
        let mut all: Vec<(Name, Text)> = Vec::new();
        for u in &m.uses {
            all.push((Name { text: u.name.clone(), alias: None, span: u.name_span }, tr!("`use` の名前", "`use` name")));
        }
        for e in &m.enums {
            all.push((e.name.clone(), tr!("列挙", "enum")));
        }
        for r in &m.records {
            all.push((r.name.clone(), tr!("レコード", "record")));
        }
        for f in &m.fns {
            all.push((f.name.clone(), tr!("関数", "function")));
        }
        self.unique(all.iter().map(|(n, w)| (n, w.clone())));
        // a function or a `use` name spelled like a builtin: which one a call names would not be
        // clear to a reader (DESIGN 2.2)
        for (n, what) in &all {
            if kw::is_builtin(&n.text) && (what.en == "function" || what.en == "`use` name") {
                let t = &n.text;
                self.err("E006", n.span, tr!("`{t}` は組み込みの関数の名前です", "`{t}` is the name of a builtin function")).notes.push(tr!(
                    "`{t}(…)` と書いたとき、どちらを呼ぶのかが読む人に分からなくなります。別の名前にしてください。",
                    "A reader of `{t}(…)` could not tell which one it calls; choose another name."
                ));
            }
        }
        for e in &m.enums {
            self.public_name(&e.name, &tr!("列挙", "enum"));
            self.unique(e.values.iter().map(|v| (&v.name, tr!("値", "value"))));
            for v in &e.values {
                self.public_name(&v.name, &tr!("列挙の値", "value"));
            }
        }
        for r in &m.records {
            self.public_name(&r.name, &tr!("レコード", "record"));
            self.unique(r.fields.iter().map(|f| (&f.name, tr!("フィールド", "field"))));
            for f in &r.fields {
                self.public_name(&f.name, &tr!("フィールド", "field"));
            }
        }
        for f in &m.fns {
            self.public_name(&f.name, &tr!("関数", "function"));
            self.unique(f.params.iter().map(|p| (&p.name, tr!("引数", "parameter"))));
            for p in &f.params {
                self.public_name(&p.name, &tr!("引数", "parameter"));
            }
        }
    }

    // ── Types ───────────────────────────────────────────────────────────────────

    /// The module enum or record a word names, by its name or its alias.
    fn own_type(&self, w: &str) -> Option<Ty> {
        let file = self.canon.to_path_buf();
        if let Some(e) = self.module.enums.iter().find(|e| e.name.is(w)) {
            return Some(Ty::Enum(EnumId { owner: Owner::Module(file), name: e.name.text.clone() }));
        }
        self.module.records.iter().find(|r| r.name.is(w)).map(|r| Ty::Record(RecId::Module { file, name: r.name.text.clone() }))
    }

    /// The type a name of words with dots between them names (`Invoice`, `charges.band`,
    /// `ledger.capture.do`, `pricing.Quote`); E101 when it names none.
    pub fn named(&mut self, path: &[(String, Span)]) -> Ty {
        let words: Vec<&str> = path.iter().map(|(w, _)| w.as_str()).collect();
        // a type of a file that could not be read: that is said once, at its `use` line
        if words.len() > 1 && !self.uses.contains_key(words[0]) && self.module.uses.iter().any(|u| u.name == words[0]) {
            return Ty::Error;
        }
        let shown = words.join(".");
        let at = path[0].1.to(path[path.len() - 1].1);
        let found = match words.as_slice() {
            [w] => self.own_type(w),
            [u, rest @ ..] => match self.uses.get(*u).map(|(_, t)| t) {
                Some(Target::Module(c)) => match rest {
                    [w] => {
                        let file = c.canon.clone();
                        c.module
                            .enums
                            .iter()
                            .find(|e| e.name.is(w))
                            .map(|e| Ty::Enum(EnumId { owner: Owner::Module(file.clone()), name: e.name.text.clone() }))
                            .or_else(|| c.module.records.iter().find(|r| r.name.is(w)).map(|r| Ty::Record(RecId::Module { file, name: r.name.text.clone() })))
                    }
                    _ => None,
                },
                Some(Target::Rule { canon, facts, .. }) => match rest {
                    [w] if *w == "outputs" => Some(Ty::Record(RecId::Outputs(canon.clone()))),
                    [w] => facts
                        .enums
                        .iter()
                        .find(|e| rule_enum_is(e, w))
                        .map(|e| Ty::Enum(EnumId { owner: Owner::Rule(canon.clone()), name: e.name.clone() }))
                        .or_else(|| facts.elements.iter().find(|(n, a, _)| n == w || a == w).map(|(n, _, _)| Ty::Record(RecId::Element(canon.clone(), n.clone())))),
                    _ => None,
                },
                Some(Target::Book { canon, facts, .. }) => match rest {
                    [t, op] => facts.transfers.iter().find(|x| x.name == *t).and_then(|x| {
                        let ops: &[&str] = if x.pending.is_some() { &["hold", "post", "void"] } else { &["do"] };
                        ops.contains(op).then(|| Ty::Transfer(TransferId { book: canon.clone(), transfer: x.name.clone(), op: op.to_string() }))
                    }),
                    _ => None,
                },
                Some(Target::Dates { .. }) | None => None,
            },
            [] => None,
        };
        match found {
            Some(t) => t,
            None => {
                self.err("E101", at, tr!("`{shown}` という型はありません", "There is no type `{shown}`")).notes.push(tr!(
                    "型に書けるのは、`number`、`bool`、`string`、`date`、`rate`、単位の型（`money[USD]`、`mass[kg]`）、`list[…]`、このモジュールのレコードと列挙、`use` で読んだもの（`<規則>.outputs`、`<規則>.<列挙>`、`<規則>.<並び>`、`<帳簿>.<振替>.<操作>`、`<モジュール>.<レコード>`）です。",
                    "A type is `number`, `bool`, `string`, `date`, `rate`, a type of a unit (`money[USD]`, `mass[kg]`), `list[…]`, a record or an enum of this module, or one a `use` reads (`<rule>.outputs`, `<rule>.<enum>`, `<rule>.<list>`, `<book>.<transfer>.<operation>`, `<module>.<record>`)."
                ));
                Ty::Error
            }
        }
    }

    /// The type a type as written names.
    pub fn resolve(&mut self, t: &TypeRef) -> Ty {
        let base = match &t.kind {
            TypeKind::Number => Ty::Num(NumTy::number()),
            TypeKind::Bool => Ty::Bool,
            TypeKind::Str => Ty::Str,
            TypeKind::Date => Ty::Date,
            TypeKind::Rate(None) => Ty::Num(NumTy { unit: Unit::rate(None), step: None, konst: None, ratio: false }),
            TypeKind::Rate(Some(n)) => {
                if n.unit != "%" || n.value.num <= 0 {
                    let raw = &n.raw;
                    self.err("E104", t.span, tr!("率の刻み `{raw}` は正の百分率ではありません", "The step `{raw}` of the rate is not a positive percentage")).notes.push(tr!(
                        "率の刻みは `rate[step 0.01%]` のように、`%` を付けた正の数で書きます（rulec と同じです）。",
                        "A rate's step is a positive number with `%`, as in `rate[step 0.01%]` (as in rulec)."
                    ));
                    return Ty::Error;
                }
                Ty::Num(NumTy::of(Unit::rate(Some(n.value.div(Rat::int(100))))))
            }
            TypeKind::Unit { dim, unit, tax, unit_span } => {
                let written = match tax {
                    Some(x) => format!("{dim}[{unit}, {x}]"),
                    None => format!("{dim}[{unit}]"),
                };
                match Unit::parse(&written) {
                    Ok(u) => Ty::Num(NumTy::of(u)),
                    Err(p) => {
                        let sp = *unit_span;
                        self.err("E101", sp, tr!("型 `{written}` を読めません", "The type `{written}` cannot be read")).notes.push(p.text());
                        return Ty::Error;
                    }
                }
            }
            TypeKind::List(inner) => Ty::List(Box::new(self.resolve(inner))),
            TypeKind::Running(item, weight) => {
                let (i, w) = (self.resolve(item), self.resolve(weight));
                if !matches!(w, Ty::Num(_) | Ty::Error) {
                    self.err("E102", weight.span, tr!("`running` の重みは数の型です", "The weight of `running` is a type of numbers"));
                    return Ty::Error;
                }
                Ty::Record(RecId::Running(Box::new(i), Box::new(w)))
            }
            TypeKind::Named(path) => self.named(path),
        };
        if t.optional && !base.is_error() { Ty::Opt(Box::new(base)) } else { base }
    }

    /// The type and what is declared of a place as written: a parameter, a field, a result.
    pub fn slot(&mut self, t: &TypeRef, b: &Bounds) -> Slot {
        let ty = self.resolve(t);
        let mut slot = Slot { ty: ty.clone(), range: None, count: None };
        if ty.is_error() {
            return slot;
        }
        // the range is of the numbers or the dates it holds, through optionals and a list
        let mut inner = &ty;
        let mut list = false;
        loop {
            match inner {
                Ty::Opt(t) => inner = t,
                Ty::List(t) if !list => {
                    list = true;
                    inner = t;
                }
                _ => break,
            }
        }
        // a range or a count written and refused is still written: what reaches it is not told again
        // that it declares none
        let refused = Range::default();
        if let Some((ends, sp)) = &b.range {
            match inner {
                Ty::Num(n) => {
                    let n = n.clone();
                    slot.range = Some(self.ends(ends, *sp, &Kind::Num(Box::new(n))).unwrap_or(refused.clone()));
                }
                Ty::Date => slot.range = Some(self.ends(ends, *sp, &Kind::Date).unwrap_or(refused.clone())),
                _ => {
                    self.err("E108", *sp, tr!("`range` を書けるのは、数と日付だけです", "A `range` is declared on numbers and dates only")).notes.push(tr!(
                        "範囲は、数の型（`number`、`money[…]`、`mass[…]`、`rate`）と `date` と、それを要素に持つリストに書きます。",
                        "A range goes on a type of numbers (`number`, `money[…]`, `mass[…]`, `rate`), on `date`, and on a list of them."
                    ));
                }
            }
        }
        if let Some((ends, sp)) = &b.count {
            let mut outer = &ty;
            while let Ty::Opt(t) = outer {
                outer = t;
            }
            if matches!(outer, Ty::List(_)) {
                slot.count = Some(self.ends(ends, *sp, &Kind::Count).unwrap_or(Range { min: None, max: Some(Rat::zero()) }));
            } else {
                self.err("E108", *sp, tr!("`count` を書けるのはリストだけです", "A `count` is declared on a list only"))
                    .notes
                    .push(tr!("`count` は、リストの要素の数の範囲です（`list[Line] count >=1 <=30`）。", "`count` bounds the number of the elements of a list (`list[Line] count >=1 <=30`)."));
            }
        }
        slot
    }

    /// The ends of a range or a count, as values of what they bound, both in; E102 and E104 for an
    /// end that is not a value of it, E108 for one written twice or a range with nothing in it.
    fn ends(&mut self, ends: &[Bound], sp: Span, kind: &Kind) -> Option<Range> {
        let mut read: Vec<End> = Vec::new();
        for b in ends {
            let v = match (&b.value, kind) {
                (BoundValue::Date(d), Kind::Date) => Rat::int(*d as i128),
                (BoundValue::Num(n, neg), Kind::Num(t)) => {
                    let raw = format!("{}{}", if *neg { "-" } else { "" }, n.raw);
                    let sign = |v: Rat| if *neg { Rat::zero().sub(v) } else { v };
                    match lit_in(n, &t.unit) {
                        LitIn::Value(v) => {
                            let step = t.step.unwrap_or(Rat::int(1));
                            if t.step.is_some() && !on_step(Some(v), step) {
                                let (shown, unit) = (step_text(step, &t.unit), unit_text(&t.unit));
                                self.err("E104", b.span, tr!("範囲の端 `{raw}` は、{unit} の刻み {shown} の上にありません", "The end `{raw}` of the range is not on the step {shown} of {unit}"));
                                return None;
                            }
                            sign(v)
                        }
                        LitIn::NotWhole(v) => {
                            let unit = unit_text(&t.unit);
                            let v = v.to_string();
                            self.err("E104", b.span, tr!("範囲の端 `{raw}` は、{unit} の整数になりません（{v}）", "The end `{raw}` of the range is no whole number of {unit} ({v})"));
                            return None;
                        }
                        LitIn::Other => {
                            let unit = unit_text(&t.unit);
                            self.err("E102", b.span, tr!("範囲の端 `{raw}` は {unit} の値ではありません", "The end `{raw}` of the range is not a value of {unit}"));
                            return None;
                        }
                    }
                }
                (BoundValue::Num(n, neg), Kind::Count) => {
                    if !n.unit.is_empty() || !n.value.is_int() || *neg {
                        let raw = &n.raw;
                        self.err("E102", b.span, tr!("要素の数 `{raw}` は、0 以上の整数で書いてください", "Write the count `{raw}` as a whole number, zero or more"));
                        return None;
                    }
                    n.value
                }
                _ => {
                    let what = match kind {
                        Kind::Date => tr!("日付", "a date"),
                        Kind::Count => tr!("要素の数", "a count"),
                        Kind::Num(_) => tr!("数", "a number"),
                    };
                    self.err("E102", b.span, tr!("範囲の端は{}で書いてください", "Write the end of the range as {}", what.ja; what.en));
                    return None;
                }
            };
            read.push((b.op, v, b.span));
        }
        let step = match kind {
            Kind::Num(t) => t.step,
            _ => Some(Rat::int(1)),
        };
        let mut r = Range::default();
        for (op, v, at) in read {
            let lower = matches!(op, Cmp::Ge | Cmp::Gt);
            let v = match (op, step) {
                (Cmp::Gt, Some(s)) => v.add(s),
                (Cmp::Lt, Some(s)) => v.sub(s),
                _ => v,
            };
            let slot = if lower { &mut r.min } else { &mut r.max };
            if slot.is_some() {
                let which = if lower { tr!("下の端", "lower end") } else { tr!("上の端", "upper end") };
                self.err("E108", at, tr!("範囲の{}が二つあります", "The range has two {}s", which.ja; which.en));
                return None;
            }
            *slot = Some(v);
        }
        if let (Some(lo), Some(hi)) = (r.min, r.max)
            && lo.cmp_to(hi) == std::cmp::Ordering::Greater
        {
            self.err("E108", sp, tr!("範囲に値が一つもありません（下の端が上の端より大きい）", "The range holds no value: its lower end is above its upper end"));
            return None;
        }
        Some(r)
    }

    // ── Declarations ────────────────────────────────────────────────────────────

    /// The enums, the records and the functions of the module, their types resolved.
    pub fn declarations(&mut self) -> Decls {
        let m = self.module;
        let mut d = Decls::default();
        for e in &m.enums {
            d.enums.push(EnumInfo { name: e.name.clone(), ordered: e.ordered, values: e.values.iter().map(|v| v.name.clone()).collect(), line: e.line });
        }
        for r in &m.records {
            let fields = r.fields.iter().map(|f| FieldInfo { name: f.name.clone(), slot: self.slot(&f.ty, &f.bounds), line: f.line }).collect();
            d.records.push(RecordInfo { name: r.name.clone(), fields, line: r.line });
        }
        for f in &m.fns {
            let params = f.params.iter().map(|p| ParamInfo { name: p.name.clone(), slot: self.slot(&p.ty, &p.bounds) }).collect();
            let ret = self.slot(&f.ret, &f.ret_bounds);
            d.fns.push(FnInfo { name: f.name.clone(), params, ret, line: f.line });
        }
        self.holds_itself(&d);
        self.reached(&d);
        self.steps(&d);
        d
    }

    /// E108 for a rate with no step where a value goes out of a function: its answer, a field of a
    /// record (DESIGN 3.1, 3.9). A rate travels as a count of its step, and a value is rounded to its
    /// place only against a step; a `let` alone may leave the step out. A parameter that leaves it out
    /// is said where what it reaches is (`reached`).
    fn steps(&mut self, d: &Decls) {
        fn stepless(t: &Ty) -> bool {
            match t {
                Ty::Num(n) => n.is_rate() && n.step.is_none(),
                Ty::Opt(x) | Ty::List(x) => stepless(x),
                Ty::Record(RecId::Running(i, w)) => stepless(i) || stepless(w),
                _ => false,
            }
        }
        let why = tr!(
            "率は刻みの数で受け渡し、丸めていない値かどうかも刻みで決まります。そのため、関数の答えとレコードのフィールドの率には刻みを書いてください。刻みを省けるのは `let` だけです。",
            "A rate travels as a count of its step, and whether a value is rounded is told against the step; so the rate of a function's answer and of a field of a record declares its step. Only a `let` may leave it out."
        );
        for (ri, r) in d.records.iter().enumerate() {
            for (fi, f) in r.fields.iter().enumerate() {
                if stepless(&f.slot.ty) {
                    let (record, n) = (&r.name.text, &f.name.text);
                    let sp = self.module.records[ri].fields[fi].ty.span;
                    self.err("E108", sp, tr!("レコード `{record}` のフィールド `{n}` に、率の刻み（`rate[step 0.01%]`）がありません", "The field `{n}` of the record `{record}` declares no step of the rate (`rate[step 0.01%]`)"))
                        .notes
                        .push(why.clone());
                }
            }
        }
        for (i, f) in d.fns.iter().enumerate() {
            if stepless(&f.ret.ty) {
                let fun = &f.name.text;
                let sp = self.module.fns[i].ret.span;
                self.err("E108", sp, tr!("関数 `{fun}` の答えの率に、刻み（`rate[step 0.01%]`）がありません", "The rate `{fun}` answers declares no step (`rate[step 0.01%]`)")).notes.push(why.clone());
            }
        }
    }

    /// The fields of a record of this module, or of a module it reaches: the record, and the path
    /// of the other module as it is shown (None for this one's).
    fn fields_of<'d>(&'d self, d: &'d Decls, file: &Path, name: &str) -> Option<(&'d RecordInfo, Option<&'d str>)> {
        if file == self.canon {
            return d.records.iter().find(|r| r.name.text == name).map(|r| (r, None));
        }
        let c = self.world.modules.get(file)?;
        c.decls.records.iter().find(|r| r.name.text == name).map(|r| (r, Some(c.path.as_str())))
    }

    /// E108 for a record of the module that holds itself, through its fields, lists and optionals.
    fn holds_itself(&mut self, d: &Decls) {
        for r in &d.records {
            let me = RecId::Module { file: self.canon.to_path_buf(), name: r.name.text.clone() };
            let mut seen: BTreeSet<String> = BTreeSet::new();
            let mut stack: Vec<(&Ty, String)> = r.fields.iter().map(|f| (&f.slot.ty, f.name.text.clone())).collect();
            let mut found: Option<String> = None;
            while let Some((t, via)) = stack.pop() {
                match t {
                    Ty::List(x) | Ty::Opt(x) => stack.push((x, via)),
                    Ty::Record(id @ RecId::Module { file, name }) => {
                        if *id == me {
                            found = Some(via);
                            break;
                        }
                        if seen.insert(name.clone())
                            && let Some((other, _)) = self.fields_of(d, file, name)
                        {
                            for f in &other.fields {
                                stack.push((&f.slot.ty, format!("{via}.{}", f.name.text)));
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let Some(via) = found {
                let n = r.name.text.clone();
                self.err("E108", r.name.span, tr!("レコード `{n}` は、自分自身を含みます（`{n}.{via}` を通して）", "The record `{n}` holds itself (through `{n}.{via}`)")).notes.push(tr!(
                    "kaname の値は、深さの決まった木です。ベクタも範囲の解析も型をたどって値を作るので、自分を含む型ではたどる処理が終わりません。",
                    "A value of kaname has a depth that is known: the vectors and the analysis of ranges build values by walking the types, which would not end for a type that holds itself."
                ));
            }
        }
    }

    /// E108 for what a function's parameters reach without the range, the count or the step it
    /// needs (DESIGN 3.9).
    fn reached(&mut self, d: &Decls) {
        let mut done: BTreeSet<(PathBuf, String)> = BTreeSet::new();
        for (fi, f) in d.fns.iter().enumerate() {
            for (pi, p) in f.params.iter().enumerate() {
                let origin = Origin { function: f.name.text.clone(), param: p.name.text.clone(), span: self.module.fns[fi].params[pi].name.span };
                self.needs(d, &p.slot, None, &origin, &mut done);
            }
        }
    }

    /// What a place reached from a parameter needs. `field` is the field it is, with its record and
    /// the path of the record's module when it is another's; None for the parameter itself.
    fn needs(&mut self, d: &Decls, slot: &Slot, field: Option<(&FieldInfo, &str, Option<&str>)>, origin: &Origin, done: &mut BTreeSet<(PathBuf, String)>) {
        let mut t = &slot.ty;
        let mut through_list = false;
        loop {
            match t {
                Ty::Opt(x) => t = x,
                Ty::List(x) => {
                    if through_list {
                        self.missing(field, origin, Missing::ListOfLists);
                        return;
                    }
                    if slot.count.as_ref().is_none_or(|c| c.max.is_none()) {
                        self.missing(field, origin, Missing::Count);
                    }
                    through_list = true;
                    t = x;
                }
                _ => break,
            }
        }
        match t {
            Ty::Num(n) if n.is_rate() => {
                // a field's step is said for every record (`steps`)
                if n.step.is_none() && field.is_none() {
                    self.missing(field, origin, Missing::Step);
                }
            }
            Ty::Num(_) | Ty::Date => {
                if slot.range.is_none() {
                    self.missing(field, origin, Missing::Range);
                }
            }
            Ty::Record(RecId::Module { file, name }) => {
                let key = (file.clone(), name.clone());
                if !done.insert(key) {
                    return;
                }
                let Some((r, shown)) = self.fields_of(d, file, name) else { return };
                let (r, shown) = (r.clone(), shown.map(str::to_string));
                for f in &r.fields {
                    self.needs(d, &f.slot, Some((f, &r.name.text, shown.as_deref())), origin, done);
                }
            }
            _ => {}
        }
    }

    fn missing(&mut self, field: Option<(&FieldInfo, &str, Option<&str>)>, origin: &Origin, what: Missing) {
        let (fun, param, pline) = (&origin.function, &origin.param, origin.span.line);
        let need = match what {
            Missing::Range => tr!("範囲（`range`）", "range (`range`)"),
            Missing::Count => tr!("要素の数の上限（`count`）", "upper bound on its count (`count`)"),
            Missing::Step => tr!("率の刻み（`rate[step 0.01%]`）", "step of the rate (`rate[step 0.01%]`)"),
            Missing::ListOfLists => tr!("要素の数の範囲（リストのリストには書けません）", "count of its elements (which a list of lists cannot declare)"),
        };
        let why = tr!(
            "関数の引数の値はホストから入ります。そのため、数と日付の引数と、引数が届くレコードの数と日付のフィールドには範囲を、リストには要素の数の上限を、率には刻みを書きます。範囲は、ホストとの約束で、範囲の解析の前提になります。",
            "A function's parameters come in from the host, so the numbers and dates of a parameter and of every field it reaches declare a range, a list the most elements it holds, and a rate its step: they are the promise to the host, and what the analysis of ranges starts from."
        );
        match field {
            None => {
                let sp = origin.span;
                let d = self.err("E108", sp, tr!("関数 `{fun}` の引数 `{param}` に、{}がありません", "The parameter `{param}` of `{fun}` declares no {}", need.ja; need.en));
                d.notes.push(why);
            }
            Some((f, record, None)) => {
                let n = &f.name.text;
                let line = f.line;
                let d = self.at_line("E108", line, tr!("レコード `{record}` のフィールド `{n}` に、{}がありません", "The field `{n}` of the record `{record}` declares no {}", need.ja; need.en));
                d.notes.push(tr!("関数 `{fun}` の引数 `{param}`（{pline} 行目）が、このフィールドに届きます。", "The parameter `{param}` of `{fun}` (line {pline}) reaches this field."));
                d.notes.push(why);
            }
            Some((f, record, Some(file))) => {
                let n = &f.name.text;
                let at = format!("{file}:{}", f.line);
                let sp = origin.span;
                let d = self.err(
                    "E108",
                    sp,
                    tr!("引数 `{param}` が届くフィールド `{record}.{n}` に、{}がありません", "The field `{record}.{n}`, which the parameter `{param}` reaches, declares no {}", need.ja; need.en),
                );
                d.notes.push(tr!("そのフィールドは {at} にあります。", "The field is at {at}."));
                d.notes.push(why);
            }
        }
    }
}

/// What a range's ends are of.
enum Kind {
    Num(Box<NumTy>),
    Date,
    Count,
}

/// The parameter a place is reached from.
struct Origin {
    function: String,
    param: String,
    span: Span,
}

enum Missing {
    Range,
    Count,
    Step,
    ListOfLists,
}

/// A literal read in a unit.
pub enum LitIn {
    /// Its value in the unit, a whole number of it (for a rate, any value).
    Value(Rat),
    /// Its value in the unit, which is no whole number of it.
    NotWhole(Rat),
    /// A literal of another dimension, tax or kind.
    Other,
}

/// A literal (`15lb`, `3.49%`, `2`) read as a value of `unit` (DESIGN 3.2: a literal is read in the
/// unit of where it is written).
pub fn lit_in(n: &crate::lex::Num, unit: &Unit) -> LitIn {
    let written = match n.unit.as_str() {
        "" => Unit::number(),
        "%" => Unit::rate(None),
        u => match ritsu_units::table::unit(u) {
            Some((Dim::Money(_), _)) => match Unit::money(u, None) {
                Some(x) => x,
                None => return LitIn::Other,
            },
            Some((d, _)) => Unit { dim: d, unit: u.to_string(), tax: None, step: None },
            None => return LitIn::Other,
        },
    };
    let v = if n.unit == "%" { n.value.div(Rat::int(100)) } else { n.value };
    // zero is zero of anything
    if v.num == 0 && unit.dim != Dim::Rate && written.dim == Dim::Number {
        return LitIn::Value(v);
    }
    match convert(v, &written, unit) {
        Some(x) if unit.dim == Dim::Rate || x.is_int() => LitIn::Value(x),
        Some(x) => LitIn::NotWhole(x),
        None => LitIn::Other,
    }
}

/// A step as a person reads it, in its unit: `0.01%`, `1`, `100`.
pub fn step_text(step: Rat, unit: &Unit) -> String {
    if unit.dim == Dim::Rate { format!("{}%", step.mul(Rat::int(100))) } else { step.to_string() }
}

/// Whether a word names a rule's enum: by the rule's name of it (`緊急度`), or by its alias as the
/// rule writes it in parentheses (`urgency`), which the port gives as the enum's public name.
pub fn rule_enum_is(e: &ritsu_ports::RuleEnum, w: &str) -> bool {
    e.name == w || e.public == w
}

/// Whether two enums of rules are one type: the same enum of the same rule, or the same enum of the
/// same `enums` file read whole by two rules (`import enum … from`, DESIGN 3.5), with the same
/// values. An enum is otherwise the type of the place that declares it.
pub fn rule_enums_same(a: &ritsu_ports::RuleEnum, b: &ritsu_ports::RuleEnum) -> bool {
    match (&a.origin, &b.origin) {
        (Some((fa, na)), Some((fb, nb))) => na == nb && canonical(fa) == canonical(fb) && a.values.iter().map(|v| &v.name).eq(b.values.iter().map(|v| &v.name)),
        _ => false,
    }
}
