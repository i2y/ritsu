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
//! - The declarations: E108 when a rate declares no step where a value of it comes in or goes out (a
//!   parameter, a function's answer, a field of a record), or a declaration does not hold (an empty
//!   range, an end written twice, a range or a count on what cannot take one, a record or an enum that
//!   holds itself and has no value that does not, or gives the types of its cycle type arguments that
//!   grow); E102 and E104 when an end of a range is not a value of its type. A range and a count may
//!   be left out anywhere: a value whose range is not written takes the whole of its type (DESIGN 3.9,
//!   22.1).

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

/// A type parameter of a generic function, record or enum (DESIGN 26.1): its name and alias, and
/// whether it takes only the types that can be keys (`K: key`).
#[derive(Clone, Debug, PartialEq)]
pub struct TParamInfo {
    pub name: Name,
    pub key: bool,
}

/// The names the types of a declaration's type parameters are written by (`Ty::Param`), in their order.
pub fn tnames(ps: &[TParamInfo]) -> Vec<String> {
    ps.iter().map(|p| p.name.text.clone()).collect()
}

#[derive(Clone, Debug)]
pub struct EnumInfo {
    pub name: Name,
    /// Written `private` (DESIGN 28.2): only its module uses it.
    pub private: bool,
    /// The type parameters (DESIGN 26.1); none for an enum that takes no type arguments.
    pub tparams: Vec<TParamInfo>,
    pub ordered: bool,
    pub values: Vec<Name>,
    /// The fields of each value, in the order of `values`: none for a value that has none (DESIGN
    /// 23.1).
    pub fields: Vec<Vec<FieldInfo>>,
    pub line: usize,
    /// Whether it holds itself (DESIGN 28.1): a value of it can hold another value of it, through its
    /// values' fields, or through other records and enums of the module that hold it.
    pub recursive: bool,
}

impl EnumInfo {
    /// Whether a value has fields: the enum is a sum, and each of its values has a type of its own
    /// (DESIGN 23.1, 23.2).
    pub fn is_sum(&self) -> bool {
        self.fields.iter().any(|f| !f.is_empty())
    }

    /// The fields of a value, by its declared name.
    pub fn fields_of(&self, value: &str) -> Option<&[FieldInfo]> {
        self.values.iter().position(|v| v.text == value).map(|i| self.fields[i].as_slice())
    }
}

/// The name a value of a sum has in the generated code, from the enum's alias and the value's
/// (`Order.placed` is `OrderPlaced`, DESIGN 23.8): in Pascal case, as a record's.
pub fn variant_type_name(enum_alias: &str, value_alias: &str) -> String {
    format!("{}{}", ritsu_emit::ident::pascal(enum_alias), ritsu_emit::ident::pascal(value_alias))
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
    /// Written `private` (DESIGN 28.2).
    pub private: bool,
    /// The type parameters (DESIGN 26.1).
    pub tparams: Vec<TParamInfo>,
    pub fields: Vec<FieldInfo>,
    pub line: usize,
    /// Whether it holds itself (DESIGN 28.1), through its fields or through other records and enums
    /// of the module that hold it.
    pub recursive: bool,
}

#[derive(Clone, Debug)]
pub struct ParamInfo {
    pub name: Name,
    pub slot: Slot,
}

#[derive(Clone, Debug)]
pub struct FnInfo {
    pub name: Name,
    /// Written `private` (DESIGN 28.2): only its module calls it.
    pub private: bool,
    /// The type parameters (DESIGN 26.1).
    pub tparams: Vec<TParamInfo>,
    pub params: Vec<ParamInfo>,
    pub ret: Slot,
    pub line: usize,
}

impl FnInfo {
    /// Whether the function is an inner one (DESIGN 21.3): it takes type parameters or a function, or
    /// is written `private`, so no host calls it, and it has no check of what a host gives and no
    /// vectors; the functions of the modules call it.
    pub fn inner(&self) -> bool {
        !self.tparams.is_empty() || self.private || self.takes_fn()
    }

    /// Whether the analysis of ranges reads its body again where it is called, with the values the
    /// call gives (DESIGN 26.5, 27.4): a function that takes type parameters or a function.
    pub fn reread(&self) -> bool {
        !self.tparams.is_empty() || self.takes_fn()
    }

    /// Whether a parameter is of a function type (DESIGN 27.1).
    pub fn takes_fn(&self) -> bool {
        self.params.iter().any(|p| matches!(p.slot.ty, Ty::Fn(..)))
    }

    /// Why no host calls an inner function (DESIGN 21.3), as `run` and `vectors` say it.
    pub fn inner_why(&self) -> Text {
        if !self.tparams.is_empty() {
            tr!("型の引数を持つ中の関数", "takes type parameters, an inner function")
        } else if self.takes_fn() {
            tr!("関数を取る中の関数", "takes a function, an inner function")
        } else {
            tr!("`private` の中の関数", "is `private`, an inner function")
        }
    }

    /// The type of the function as a value (DESIGN 27.1): its parameters' types and its answer's.
    pub fn fn_type(&self) -> Ty {
        Ty::Fn(self.params.iter().map(|p| p.slot.ty.clone()).collect(), Box::new(self.ret.ty.clone()), None)
    }
}

/// Where a type is written, for whether it may be a function type (DESIGN 27.1): a parameter's and a
/// `let`'s may; a function's answer, a field, a type inside another type and a type argument may not
/// (E113, and E112 for a type argument).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Within {
    Value,
    Answer,
    Field,
    Part,
    Arg,
}

/// What E113 says of where a function value goes (DESIGN 27.1).
pub fn fn_value_note() -> Text {
    tr!(
        "関数の値は、関数と組み込みの関数の引数に渡すこと、`let` で名前を付けること、呼ぶことにだけ使えます。関数の答え、レコードと列挙の値のフィールド、リスト・辞書・集合の要素、値の無いことがある値、`=` と `!=` には置けません。関数を答える代わりに、引数を足した関数を書くか、呼ぶ側で `x => f(x: x, n: n)` と書いてください。",
        "A function value is given as an argument of a function or a builtin, named by a `let`, and called, and goes nowhere else: not to a function's answer, a field of a record or of a value of an enum, an element of a list, a dict or a set, a value that may be none, nor `=` and `!=`. Instead of a function that answers a function, write one that takes the argument too, or write `x => f(x: x, n: n)` where it is called."
    )
}

/// What E101 says of a `private` thing of another module (DESIGN 28.2).
pub fn private_note(what: &Text, name: &str, module: &str) -> Text {
    tr!(
        "{} `{name}` は `private` なので、モジュール `{module}` の中でだけ使えます。",
        "The {} `{name}` is `private`: only the module `{module}` uses it.",
        what.ja; what.en
    )
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
        ColumnType::Enum { name: n, .. } => Ty::Enum(EnumId::plain(Owner::Rule(rule.to_path_buf()), n.clone())),
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
    /// The type parameters in scope where a type is read: those of the declaration it is written in
    /// (DESIGN 26.1).
    pub scope: Vec<TParamInfo>,
}

/// One end of a range, read.
type End = (Cmp, Rat, Span);

impl<'a> Dc<'a> {
    fn err(&mut self, code: &'static str, at: Span, msg: Text) -> &mut Diag {
        self.diags.push(Diag::at(code, self.path, at.line, at.col, msg).source(self.src));
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
            let sum = e.values.iter().any(|v| !v.fields.is_empty());
            for v in &e.values {
                self.public_name(&v.name, &tr!("列挙の値", "value"));
                self.unique(v.fields.iter().map(|f| (&f.name, tr!("フィールド", "field"))));
                for f in &v.fields {
                    self.public_name(&f.name, &tr!("フィールド", "field"));
                    // TypeScript tells the values of a sum apart by the field `kind` (DESIGN 23.8)
                    if sum && f.name.ascii() == "kind" {
                        let sp = f.name.alias.as_ref().map(|(_, s)| *s).unwrap_or(f.name.span);
                        let (en, vn) = (&e.name.text, &v.name.text);
                        self.err("E007", sp, tr!("生成するコードは、`{en}.{vn}` のフィールドの名前に `kind` を使えません", "The generated code cannot take `kind` as the name of a field of `{en}.{vn}`")).notes.push(tr!(
                            "生成する TypeScript は、フィールドを持つ値のある列挙の値を、フィールド `kind` で見分けます。別の名前か別名にしてください。",
                            "The generated TypeScript tells the values of an enum whose values have fields apart by the field `kind`; choose another name or alias."
                        ));
                    }
                }
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
        // the type parameters: in the space of the types, apart from the module's records and enums
        // and from each other, and named in the generated code (DESIGN 26.1)
        let decls: Vec<(&[TypeParam], &Name)> =
            m.records.iter().map(|r| (r.tparams.as_slice(), &r.name)).chain(m.enums.iter().map(|e| (e.tparams.as_slice(), &e.name))).chain(m.fns.iter().map(|f| (f.tparams.as_slice(), &f.name))).collect();
        for (tps, _) in &decls {
            self.unique(tps.iter().map(|p| (&p.name, tr!("型の引数", "type parameter"))));
            for p in tps.iter() {
                self.public_name(&p.name, &tr!("型の引数", "type parameter"));
                let taken = m.records.iter().map(|r| (&r.name, tr!("レコード", "record"))).chain(m.enums.iter().map(|e| (&e.name, tr!("列挙", "enum")))).find(|(n, _)| {
                    n.is(&p.name.text) || p.name.alias.as_ref().is_some_and(|(a, _)| n.is(a))
                });
                if let Some((n, what)) = taken {
                    let (t, line) = (&p.name.text, n.span.line);
                    self.err("E006", p.name.span, tr!("`{t}` が二度あります（{line} 行目の{}と、この型の引数）", "`{t}` is given twice (the {} on line {line}, and this type parameter)", what.ja; what.en))
                        .notes
                        .push(tr!("型の引数は、モジュールのレコードと列挙と同じ型の名前の中にあります。別の名前にしてください。", "A type parameter is a name of a type, as the module's records and enums are: choose another name."));
                }
            }
        }
        // the types the generated code names for the values of a sum (`OrderPlaced`, DESIGN 23.8), held
        // apart from the records' and the enums' and from each other
        let mut types: BTreeMap<String, Text> = BTreeMap::new();
        for r in &m.records {
            let n = &r.name.text;
            types.insert(ritsu_emit::ident::pascal(r.name.ascii()), tr!("レコード `{n}`", "the record `{n}`"));
        }
        for e in &m.enums {
            let n = &e.name.text;
            types.entry(ritsu_emit::ident::pascal(e.name.ascii())).or_insert(tr!("列挙 `{n}`", "the enum `{n}`"));
        }
        for e in m.enums.iter().filter(|e| e.values.iter().any(|v| !v.fields.is_empty())) {
            for v in &e.values {
                let ty = variant_type_name(e.name.ascii(), v.name.ascii());
                let (en, vn) = (&e.name.text, &v.name.text);
                let what = tr!("値 `{en}.{vn}`", "the value `{en}.{vn}`");
                match types.get(&ty).cloned() {
                    Some(first) => {
                        self.err(
                            "E006",
                            v.name.span,
                            tr!("生成するコードの型の名前 `{ty}` が二度あります（{} と、{}）", "The generated code would name two types `{ty}` ({} and {})", first.ja, what.ja; first.en, what.en),
                        )
                        .notes
                        .push(tr!(
                            "フィールドを持つ値のある列挙の値の型は、列挙の名前と値の名前をつなげた名前（`Order.placed` は `OrderPlaced`）になります。どちらかの名前か別名を替えてください。",
                            "The type of a value of an enum whose values have fields is named by the enum's name and the value's, joined (`Order.placed` is `OrderPlaced`); rename one of them, or give it another alias."
                        ));
                    }
                    None => {
                        types.insert(ty, what);
                    }
                }
            }
        }
    }

    // ── Types ───────────────────────────────────────────────────────────────────

    /// The module enum or record a word names, by its name or its alias, and its type parameters.
    fn own_type(&self, w: &str) -> Option<(Ty, Vec<TParamInfo>)> {
        let file = self.canon.to_path_buf();
        let info = |ps: &[TypeParam]| ps.iter().map(|p| TParamInfo { name: p.name.clone(), key: p.key }).collect::<Vec<_>>();
        if let Some(e) = self.module.enums.iter().find(|e| e.name.is(w)) {
            return Some((Ty::Enum(EnumId::plain(Owner::Module(file), e.name.text.clone())), info(&e.tparams)));
        }
        self.module.records.iter().find(|r| r.name.is(w)).map(|r| (Ty::Record(RecId::record(file, r.name.text.clone())), info(&r.tparams)))
    }

    /// The type parameter in scope a word names, by its name or its alias.
    fn tparam(&self, w: &str) -> Option<&TParamInfo> {
        self.scope.iter().find(|p| p.name.is(w))
    }

    /// The type arguments of a generic record or enum, read and held to its type parameters (DESIGN
    /// 26.1): E112 where their number is not the parameters', E114 where one of a parameter of keys
    /// cannot be one; None where one is said wrong.
    fn type_args(&mut self, shown: &str, at: Span, params: &[TParamInfo], args: &[TypeRef]) -> Option<Vec<Ty>> {
        let tys: Vec<Ty> = args.iter().map(|a| self.resolve_in(a, Within::Arg)).collect();
        if tys.len() != params.len() {
            let (n, k) = (params.len(), tys.len());
            let msg = if n == 0 {
                tr!("`{shown}` は型の引数を取りません", "`{shown}` takes no type arguments")
            } else {
                {
                    let en_n = ritsu_base::text::plural(n, "type argument", "type arguments");
                    tr!("`{shown}` は型の引数を {n} 個取ります。書いてあるのは {k} 個です", "`{shown}` takes {en_n}, and is given {k}")
                }
            };
            let d = self.err("E112", at, msg);
            if n > 0 {
                let ps = params.iter().map(|p| format!("`{}`", p.name.text)).collect::<Vec<_>>().join(", ");
                d.notes.push(tr!("型の引数（{ps}）に当てる型を、`{shown}[…]` のように順に書いてください。", "Write the types its type parameters ({ps}) take, in their order: `{shown}[…]`."));
            }
            return None;
        }
        for ((p, t), a) in params.iter().zip(&tys).zip(args) {
            if p.key && !t.is_error() && !self.key_ok(t, a.span) {
                return None;
            }
            // a type whose values may be none: `T?` would hold two kinds of none (DESIGN 26.2)
            if let Ty::Opt(_) = t {
                let (n, ty) = (&p.name.text, self.show_type(t));
                self.err("E112", a.span, tr!("`{shown}` の型の引数 `{n}` に、値の無いことがある型 {ty} が当たっています", "The type parameter `{n}` of `{shown}` is given {ty}, a type whose values may be none")).notes.push(tr!(
                    "型の引数には、値の無いことがある型は当たりません（総称の中の `{n}?` が、二つの none を持つことになるためです）。`?` の無い型を当て、値の無いことがあるところは `{n}?` と書いてください。",
                    "A type parameter takes no type whose values may be none (its `{n}?` would hold two kinds of none): give it the type with no `?`, and write `{n}?` where the value may be none."
                ));
                return None;
            }
        }
        if tys.iter().any(Ty::is_error) {
            return None;
        }
        Some(tys)
    }

    /// The type of a value of an enum of this module whose values have fields (`Order.placed`,
    /// DESIGN 23.2): Some(Ok) for one, Some(Err) with why for a word that names an enum of the module
    /// and no such value, None for a word that names no enum of it.
    fn own_value_type(&self, en: &str, v: &str) -> Option<(Result<Ty, Text>, Vec<TParamInfo>)> {
        let e = self.module.enums.iter().find(|e| e.name.is(en))?;
        let ps = e.tparams.iter().map(|p| TParamInfo { name: p.name.clone(), key: p.key }).collect();
        Some((variant_type(&self.canon.to_path_buf(), e.name.text.as_str(), e.values.iter().map(|x| (&x.name, !x.fields.is_empty())), v), ps))
    }

    /// The type a name of words with dots between them names (`Invoice`, `charges.band`,
    /// `ledger.capture.do`, `pricing.Quote`, a type parameter `T`), with the type arguments written
    /// after it (`Page[Invoice]`, DESIGN 26.1); E101 when it names none, E112 when the type arguments
    /// are not what it takes.
    pub fn named(&mut self, path: &[(String, Span)], args: &[TypeRef]) -> Ty {
        let words: Vec<&str> = path.iter().map(|(w, _)| w.as_str()).collect();
        let shown = words.join(".");
        let at = path[0].1.to(path[path.len() - 1].1);
        let Some((t, ps)) = self.lookup(path, !args.is_empty()) else { return Ty::Error };
        let Some(targs) = self.type_args(&shown, at, &ps, args) else { return Ty::Error };
        with_targs(t, targs)
    }

    /// The type a record or a value of a sum is built as (`Page { … }`, `Outcome.done { … }`, DESIGN
    /// 26.2): what `named` finds, its type arguments left to the check of the fields, and its type
    /// parameters; Ty::Error where it names none (said).
    pub fn named_open(&mut self, path: &[(String, Span)]) -> (Ty, Vec<TParamInfo>) {
        self.lookup(path, false).unwrap_or((Ty::Error, Vec::new()))
    }

    /// What `named` and `named_open` find: the type with no type arguments, and its type parameters;
    /// None where a type of a file that could not be read is named, or one that is said to be none.
    fn lookup(&mut self, path: &[(String, Span)], with_args: bool) -> Option<(Ty, Vec<TParamInfo>)> {
        let words: Vec<&str> = path.iter().map(|(w, _)| w.as_str()).collect();
        // a type of a file that could not be read: that is said once, at its `use` line
        if words.len() > 1 && !self.uses.contains_key(words[0]) && self.module.uses.iter().any(|u| u.name == words[0]) {
            return None;
        }
        let shown = words.join(".");
        let at = path[0].1.to(path[path.len() - 1].1);
        // a type parameter of the declaration the type is written in
        if let [w] = words.as_slice()
            && let Some(p) = self.tparam(w).cloned()
        {
            if with_args {
                // a type parameter takes no type arguments: said as one that takes none
                return Some((Ty::Param(p.name.text), Vec::new()));
            }
            return Some((Ty::Param(p.name.text), Vec::new()));
        }
        // a `private` record or enum of another module (DESIGN 28.2): E101, with why
        if let [u, w, ..] = words.as_slice()
            && let Some(Target::Module(c)) = self.uses.get(*u).map(|(_, t)| t)
        {
            let found = c.decls.enums.iter().find(|e| e.name.is(w)).map(|e| (e.private, tr!("列挙", "enum"), e.name.text.clone())).or_else(|| c.decls.records.iter().find(|r| r.name.is(w)).map(|r| (r.private, tr!("レコード", "record"), r.name.text.clone())));
            if let Some((true, what, name)) = found {
                let note = private_note(&what, &name, &c.module.name.text);
                self.err("E101", at, tr!("`{shown}` という型はありません", "There is no type `{shown}`")).notes.push(note);
                return None;
            }
        }
        // the type of a value of an enum whose values have fields: `Order.placed`, `orders.Order.placed`
        let value_type = match words.as_slice() {
            [en, v] if !self.uses.contains_key(*en) => self.own_value_type(en, v),
            [u, en, v] => match self.uses.get(*u).map(|(_, t)| t) {
                Some(Target::Module(c)) => c
                    .decls
                    .enums
                    .iter()
                    .find(|e| e.name.is(en))
                    .map(|e| (variant_type(&c.canon, &e.name.text, e.values.iter().zip(&e.fields).map(|(x, f)| (x, !f.is_empty())), v), e.tparams.clone())),
                _ => None,
            },
            _ => None,
        };
        match value_type {
            Some((Ok(t), ps)) => return Some((t, ps)),
            Some((Err(why), _)) => {
                self.err("E101", at, tr!("`{shown}` という型はありません", "There is no type `{shown}`")).notes.push(why);
                return None;
            }
            None => {}
        }
        let info = |ps: &[TParamInfo]| ps.to_vec();
        let found: Option<(Ty, Vec<TParamInfo>)> = match words.as_slice() {
            [w] => self.own_type(w),
            [u, rest @ ..] => match self.uses.get(*u).map(|(_, t)| t) {
                Some(Target::Module(c)) => match rest {
                    [w] => {
                        let file = c.canon.clone();
                        c.decls
                            .enums
                            .iter()
                            .find(|e| e.name.is(w))
                            .map(|e| (Ty::Enum(EnumId::plain(Owner::Module(file.clone()), e.name.text.clone())), info(&e.tparams)))
                            .or_else(|| c.decls.records.iter().find(|r| r.name.is(w)).map(|r| (Ty::Record(RecId::record(file, r.name.text.clone())), info(&r.tparams))))
                    }
                    _ => None,
                },
                Some(Target::Rule { canon, facts, .. }) => match rest {
                    // a rule with no outputs (an `enums` file): read for its enums alone (DESIGN 2.7)
                    [w] if *w == "outputs" && facts.outputs.is_empty() => {
                        let u = u.to_string();
                        self.err("E201", at, tr!("規則 `{u}` には出力が無いので、呼べません", "The rule `{u}` has no outputs, and is not called")).notes.push(crate::types::no_outputs_note(&u));
                        Some((Ty::Error, Vec::new()))
                    }
                    [w] if *w == "outputs" => Some((Ty::Record(RecId::Outputs(canon.clone())), Vec::new())),
                    [w] => facts
                        .enums
                        .iter()
                        .find(|e| rule_enum_is(e, w))
                        .map(|e| (Ty::Enum(EnumId::plain(Owner::Rule(canon.clone()), e.name.clone())), Vec::new()))
                        .or_else(|| facts.elements.iter().find(|(n, a, _)| n == w || a == w).map(|(n, _, _)| (Ty::Record(RecId::Element(canon.clone(), n.clone())), Vec::new()))),
                    _ => None,
                },
                Some(Target::Book { canon, facts, .. }) => match rest {
                    [t, op] => facts.transfers.iter().find(|x| x.name == *t).and_then(|x| {
                        let ops: &[&str] = if x.pending.is_some() { &["hold", "post", "void"] } else { &["do"] };
                        ops.contains(op).then(|| (Ty::Transfer(TransferId { book: canon.clone(), transfer: x.name.clone(), op: op.to_string() }), Vec::new()))
                    }),
                    _ => None,
                },
                Some(Target::Dates { .. }) | None => None,
            },
            [] => None,
        };
        match found {
            Some(x) => Some(x),
            None => {
                self.err("E101", at, tr!("`{shown}` という型はありません", "There is no type `{shown}`")).notes.push(tr!(
                    "型に書けるのは、`number`、`bool`、`string`、`date`、`rate`、単位の型（`money[USD]`、`mass[kg]`）、`list[…]`、このモジュールのレコードと列挙、フィールドを持つ値の型（`<列挙>.<値>`）、`use` で読んだもの（`<規則>.outputs`、`<規則>.<列挙>`、`<規則>.<並び>`、`<帳簿>.<振替>.<操作>`、`<モジュール>.<レコード>`）、型の引数、`result[…]` です。",
                    "A type is `number`, `bool`, `string`, `date`, `rate`, a type of a unit (`money[USD]`, `mass[kg]`), `list[…]`, a record or an enum of this module, the type of a value of an enum whose values have fields (`<enum>.<value>`), one a `use` reads (`<rule>.outputs`, `<rule>.<enum>`, `<rule>.<list>`, `<book>.<transfer>.<operation>`, `<module>.<record>`), a type parameter or `result[…]`."
                ));
                None
            }
        }
    }

    /// The type a type as written names, where a function type is not written (inside another type).
    pub fn resolve(&mut self, t: &TypeRef) -> Ty {
        self.resolve_in(t, Within::Part)
    }

    /// The type a type as written names, written at `within` (DESIGN 27.1): a function type where none
    /// goes is E113 (E112 for a type argument), and so is one with `?`.
    pub fn resolve_in(&mut self, t: &TypeRef, within: Within) -> Ty {
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
            TypeKind::Dict(k, v) => {
                let (kt, vt) = (self.resolve(k), self.resolve(v));
                if !self.key_ok(&kt, k.span) {
                    return Ty::Error;
                }
                Ty::Dict(Box::new(kt), Box::new(vt))
            }
            TypeKind::Set(x) => {
                let xt = self.resolve(x);
                if !self.key_ok(&xt, x.span) {
                    return Ty::Error;
                }
                Ty::Set(Box::new(xt))
            }
            TypeKind::Entry(k, v) => {
                let (kt, vt) = (self.resolve(k), self.resolve(v));
                if !self.key_ok(&kt, k.span) {
                    return Ty::Error;
                }
                Ty::Record(RecId::Entry(Box::new(kt), Box::new(vt)))
            }
            TypeKind::Indexed(x) => Ty::Record(RecId::Indexed(Box::new(self.resolve(x)))),
            TypeKind::Result(a, e) => {
                let (at, et) = (self.resolve(a), self.resolve(e));
                if at.is_error() || et.is_error() {
                    return Ty::Error;
                }
                Ty::Result(Box::new(at), Box::new(et))
            }
            TypeKind::Named(path, args) => self.named(path, args),
            TypeKind::Fn(ps, r, eff) => {
                if within != Within::Value || t.optional {
                    self.fn_type_here(t, within);
                    return Ty::Error;
                }
                let pts: Vec<Ty> = ps.iter().map(|p| self.resolve(p)).collect();
                let rt = self.resolve(r);
                if pts.iter().any(Ty::is_error) || rt.is_error() {
                    return Ty::Error;
                }
                Ty::Fn(pts, Box::new(rt), eff.as_ref().map(FnEffect::of))
            }
        };
        if t.optional && !base.is_error() { Ty::Opt(Box::new(base)) } else { base }
    }

    /// E113 (E112 for a type argument) for a function type written where none goes (DESIGN 27.1).
    fn fn_type_here(&mut self, t: &TypeRef, within: Within) {
        let at = t.span;
        if t.optional {
            self.err("E113", at, tr!("関数の型に `?` は付けられません", "A function type takes no `?`")).notes.push(fn_value_note());
            return;
        }
        match within {
            Within::Arg => {
                self.err("E112", at, tr!("関数の型は、型の引数に当たりません", "A function type is given to no type parameter")).notes.push(tr!(
                    "型の引数の値は、入れて持ち、比べることのできる値です。関数の値は渡すことと呼ぶことにだけ使うので、型の引数には当たりません。関数を取る引数は、関数の型で書いてください（`f: (T) -> U`）。",
                    "A value of a type parameter is held and compared, and a function value is only passed and called, so no type parameter takes a function type: write a parameter that takes a function with its function type (`f: (T) -> U`)."
                ));
            }
            Within::Answer => {
                self.err("E113", at, tr!("関数の答えに、関数の型は書けません", "The answer of a function cannot be of a function type")).notes.push(fn_value_note());
            }
            Within::Field => {
                self.err("E113", at, tr!("フィールドの型に、関数の型は書けません", "A field cannot be of a function type")).notes.push(fn_value_note());
            }
            Within::Part | Within::Value => {
                self.err("E113", at, tr!("関数の型は、ほかの型の中に書けません", "A function type is not written inside another type")).notes.push(fn_value_note());
            }
        }
    }

    /// Whether an enum of this module, or of one it reaches, has values with fields.
    pub fn is_sum(&self, id: &EnumId) -> bool {
        // `result` is a sum the language gives (DESIGN 26.3); its type is `Ty::Result`, not an enum
        match &id.owner {
            Owner::Module(f) if f == self.canon => self.module.enums.iter().any(|e| e.name.text == id.name && e.values.iter().any(|v| !v.fields.is_empty())),
            Owner::Module(f) => self.world.modules.get(f).is_some_and(|c| c.decls.enums.iter().any(|e| e.name.text == id.name && e.is_sum())),
            Owner::Rule(_) => false,
        }
    }

    /// E114 for the type of the keys of a dict or the members of a set that cannot be a key (DESIGN
    /// 24.1): false then.
    fn key_ok(&mut self, t: &Ty, at: Span) -> bool {
        // a type parameter is a key where it is held to keys (`K: key`, DESIGN 26.2)
        if let Ty::Param(n) = t {
            if self.scope.iter().any(|p| p.name.text == *n && p.key) {
                return true;
            }
            let n = n.clone();
            self.err("E114", at, tr!("型の引数 `{n}` は鍵にできません", "The type parameter `{n}` cannot be a key")).notes.push(param_key_note(&n));
            return false;
        }
        if is_key(t, |id| self.is_sum(id)) {
            return true;
        }
        let shown = self.show_type(t);
        self.err("E114", at, tr!("{shown} は鍵にできません", "{shown} cannot be a key")).notes.push(key_note());
        false
    }

    /// A type as this module writes it, for a message.
    fn show_type(&self, t: &Ty) -> String {
        match t {
            Ty::Num(n) => unit_text(&n.unit),
            Ty::Bool => "bool".into(),
            Ty::Str => "string".into(),
            Ty::Date => "date".into(),
            Ty::Enum(id) => with_args(id.name.clone(), &id.args.iter().map(|a| self.show_type(a)).collect::<Vec<_>>()),
            Ty::Record(RecId::Module { name, args, .. }) => with_args(name.clone(), &args.iter().map(|a| self.show_type(a)).collect::<Vec<_>>()),
            Ty::Record(RecId::Variant { name, value, args, .. }) => with_args(format!("{name}.{value}"), &args.iter().map(|a| self.show_type(a)).collect::<Vec<_>>()),
            Ty::Param(n) => n.clone(),
            Ty::Result(a, e) => format!("result[{}, {}]", self.show_type(a), self.show_type(e)),
            Ty::Record(RecId::Entry(k, v)) => format!("entry[{}, {}]", self.show_type(k), self.show_type(v)),
            Ty::Record(RecId::Indexed(x)) => format!("indexed[{}]", self.show_type(x)),
            Ty::Record(RecId::Running(a, b)) => format!("running[{}, {}]", self.show_type(a), self.show_type(b)),
            Ty::Record(_) => "record".into(),
            Ty::List(x) => format!("list[{}]", self.show_type(x)),
            Ty::Dict(k, v) => format!("dict[{}, {}]", self.show_type(k), self.show_type(v)),
            Ty::Set(x) => format!("set[{}]", self.show_type(x)),
            Ty::Opt(x) => format!("{}?", self.show_type(x)),
            Ty::Transfer(id) => format!("{}.{}", id.transfer, id.op),
            Ty::None => "none".into(),
            Ty::Empty => "[]".into(),
            Ty::EmptyDict => "[:]".into(),
            Ty::Fn(ps, r, eff) => fn_text(&ps.iter().map(|p| self.show_type(p)).collect::<Vec<_>>(), &self.show_type(r), eff.as_ref()),
            Ty::Error => "?".into(),
        }
    }

    /// The type and what is declared of a place as written at `within`: a parameter, a field, a
    /// result.
    pub fn slot(&mut self, t: &TypeRef, b: &Bounds, within: Within) -> Slot {
        let ty = self.resolve_in(t, within);
        let mut slot = Slot { ty: ty.clone(), range: None, count: None };
        if ty.is_error() {
            return slot;
        }
        // the range is of the numbers or the dates it holds, through optionals and a list (a set's
        // members, a dict's values)
        let mut inner = &ty;
        let mut list = false;
        loop {
            match inner {
                Ty::Opt(t) => inner = t,
                Ty::List(t) | Ty::Set(t) if !list => {
                    list = true;
                    inner = t;
                }
                Ty::Dict(_, t) if !list => {
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
                // a value of a type parameter is of no one type, and takes no range (DESIGN 26.2)
                Ty::Param(n) => {
                    let n = n.clone();
                    self.err("E108", *sp, tr!("型の引数 `{n}` の値には、範囲を書けません", "A value of the type parameter `{n}` declares no range")).notes.push(tr!(
                        "型の引数の値は、どの型の値になるかが呼ぶところで決まるので、範囲を書けません。範囲を書くのは、呼ぶ側の具体的な型の引数とフィールドです。型の引数のリストには `count` を書けます。",
                        "What type a value of a type parameter is of is said where the function is called, so it takes no range: write the range where the type is one, at the caller. A list of a type parameter takes a `count`."
                    ));
                    slot.range = Some(refused.clone());
                }
                _ => {
                    self.err("E108", *sp, tr!("`range` を書けるのは、数と日付だけです", "A `range` is declared on numbers and dates only")).notes.push(tr!(
                        "範囲は、数の型（`number`、`money[…]`、`mass[…]`、`rate`）と `date` と、それを要素に持つリストと集合、それを値に持つ辞書に書きます。",
                        "A range goes on a type of numbers (`number`, `money[…]`, `mass[…]`, `rate`), on `date`, on a list and a set of them, and on a dict whose values they are."
                    ));
                }
            }
        }
        if let Some((ends, sp)) = &b.count {
            let mut outer = &ty;
            while let Ty::Opt(t) = outer {
                outer = t;
            }
            // a string's count is of its code points (DESIGN 25.1)
            if matches!(outer, Ty::List(_) | Ty::Dict(..) | Ty::Set(_) | Ty::Str) {
                slot.count = Some(self.ends(ends, *sp, &Kind::Count).unwrap_or(Range { min: None, max: Some(Rat::zero()) }));
            } else {
                self.err("E108", *sp, tr!("`count` を書けるのは、リスト、辞書、集合、文字列だけです", "A `count` is declared on a list, a dict, a set and a string only")).notes.push(tr!(
                    "`count` は、リストの要素の数、辞書の組の数、集合の要素の数、文字列のコードポイントの数の範囲です（`list[Line] count >=1 <=30`、`string count <=40`）。",
                    "`count` bounds the number of the elements of a list, the pairs of a dict, the members of a set, the code points of a string (`list[Line] count >=1 <=30`, `string count <=40`)."
                ));
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
        let info = |ps: &[TypeParam]| ps.iter().map(|p| TParamInfo { name: p.name.clone(), key: p.key }).collect::<Vec<_>>();
        for e in &m.enums {
            self.scope = info(&e.tparams);
            let fields = e.values.iter().map(|v| v.fields.iter().map(|f| FieldInfo { name: f.name.clone(), slot: self.slot(&f.ty, &f.bounds, Within::Field), line: f.line }).collect()).collect();
            d.enums.push(EnumInfo { name: e.name.clone(), private: e.private, tparams: self.scope.clone(), ordered: e.ordered, values: e.values.iter().map(|v| v.name.clone()).collect(), fields, line: e.line, recursive: false });
        }
        for r in &m.records {
            self.scope = info(&r.tparams);
            let fields = r.fields.iter().map(|f| FieldInfo { name: f.name.clone(), slot: self.slot(&f.ty, &f.bounds, Within::Field), line: f.line }).collect();
            d.records.push(RecordInfo { name: r.name.clone(), private: r.private, tparams: self.scope.clone(), fields, line: r.line, recursive: false });
        }
        for f in &m.fns {
            self.scope = info(&f.tparams);
            let params = f.params.iter().map(|p| ParamInfo { name: p.name.clone(), slot: self.slot(&p.ty, &p.bounds, Within::Value) }).collect();
            let ret = self.slot(&f.ret, &f.ret_bounds, Within::Answer);
            d.fns.push(FnInfo { name: f.name.clone(), private: f.private, tparams: self.scope.clone(), params, ret, line: f.line });
        }
        self.scope.clear();
        self.recursion(&mut d);
        self.reached(&d);
        self.steps(&d);
        self.private_types(&d);
        d
    }

    /// E108 where what other modules or a host use names a `private` record or enum of the module
    /// (DESIGN 28.2): a parameter or the answer of a function that is not `private`, a field of a record
    /// or of a value of an enum that is not. They could not make a value of it.
    fn private_types(&mut self, d: &Decls) {
        let file = self.canon.to_path_buf();
        let private = |t: &Ty| -> Option<(Text, String)> {
            let mut found = None;
            let mut stack = vec![t];
            while let Some(t) = stack.pop() {
                match t {
                    Ty::List(x) | Ty::Set(x) | Ty::Opt(x) => stack.push(x),
                    Ty::Dict(a, b) | Ty::Result(a, b) => {
                        stack.push(a);
                        stack.push(b);
                    }
                    Ty::Fn(ps, r, _) => {
                        stack.extend(ps.iter());
                        stack.push(r);
                    }
                    Ty::Record(RecId::Module { file: f, name, args }) | Ty::Record(RecId::Variant { file: f, name, args, .. }) if *f == file => {
                        let rec = matches!(t, Ty::Record(RecId::Module { .. }));
                        let hidden = if rec { d.records.iter().any(|r| r.name.text == *name && r.private) } else { d.enums.iter().any(|e| e.name.text == *name && e.private) };
                        if hidden && found.is_none() {
                            found = Some((if rec { tr!("レコード", "record") } else { tr!("列挙", "enum") }, name.clone()));
                        }
                        stack.extend(args.iter());
                    }
                    Ty::Enum(EnumId { owner: Owner::Module(f), name, args }) if *f == file => {
                        if d.enums.iter().any(|e| e.name.text == *name && e.private) && found.is_none() {
                            found = Some((tr!("列挙", "enum"), name.clone()));
                        }
                        stack.extend(args.iter());
                    }
                    Ty::Record(RecId::Running(a, b)) | Ty::Record(RecId::Entry(a, b)) => {
                        stack.push(a);
                        stack.push(b);
                    }
                    Ty::Record(RecId::Indexed(x)) => stack.push(x),
                    _ => {}
                }
            }
            found
        };
        let note = tr!(
            "`private` のレコードと列挙は、そのモジュールの中でだけ使えます。ほかのモジュールやホストは、その型の値を作れません。使うものも `private` にするか、型の `private` を外してください。",
            "A `private` record or enum is used in its module alone: no other module and no host can make a value of it. Make what uses it `private` too, or take the `private` off the type."
        );
        let mut said: Vec<(Span, Text)> = Vec::new();
        for (fi, f) in d.fns.iter().enumerate() {
            if f.private {
                continue;
            }
            let fun = f.name.text.clone();
            for (pi, p) in f.params.iter().enumerate() {
                if let Some((what, name)) = private(&p.slot.ty) {
                    let pn = &p.name.text;
                    said.push((self.module.fns[fi].params[pi].ty.span, tr!("`private` でない関数 `{fun}` の引数 `{pn}` の型に、`private` の{} `{name}` があります", "The parameter `{pn}` of `{fun}`, which is not `private`, is of the `private` {} `{name}`", what.ja; what.en)));
                }
            }
            if let Some((what, name)) = private(&f.ret.ty) {
                said.push((self.module.fns[fi].ret.span, tr!("`private` でない関数 `{fun}` の答えの型に、`private` の{} `{name}` があります", "The answer of `{fun}`, which is not `private`, is of the `private` {} `{name}`", what.ja; what.en)));
            }
        }
        for (ri, r) in d.records.iter().enumerate() {
            if r.private {
                continue;
            }
            for (fi, f) in r.fields.iter().enumerate() {
                if let Some((what, name)) = private(&f.slot.ty) {
                    let (rn, fl) = (&r.name.text, &f.name.text);
                    said.push((self.module.records[ri].fields[fi].ty.span, tr!("`private` でないレコード `{rn}` のフィールド `{fl}` の型に、`private` の{} `{name}` があります", "The field `{fl}` of the record `{rn}`, which is not `private`, is of the `private` {} `{name}`", what.ja; what.en)));
                }
            }
        }
        for (ei, e) in d.enums.iter().enumerate() {
            if e.private {
                continue;
            }
            for (vi, fs) in e.fields.iter().enumerate() {
                for (fi, f) in fs.iter().enumerate() {
                    if let Some((what, name)) = private(&f.slot.ty) {
                        let (en, vn, fl) = (&e.name.text, &e.values[vi].text, &f.name.text);
                        said.push((self.module.enums[ei].values[vi].fields[fi].ty.span, tr!("`private` でない `{en}.{vn}` のフィールド `{fl}` の型に、`private` の{} `{name}` があります", "The field `{fl}` of `{en}.{vn}`, which is not `private`, is of the `private` {} `{name}`", what.ja; what.en)));
                    }
                }
            }
        }
        for (at, msg) in said {
            self.err("E108", at, msg).notes.push(note.clone());
        }
    }

    /// E108 for a rate with no step where a value goes out of a function: its answer, a field of a
    /// record (DESIGN 3.1, 3.9). A rate travels as a count of its step, and a value is rounded to its
    /// place only against a step; a `let` alone may leave the step out. A parameter that leaves it out
    /// is said where what it reaches is (`reached`).
    fn steps(&mut self, d: &Decls) {
        fn stepless(t: &Ty) -> bool {
            match t {
                Ty::Num(n) => n.is_rate() && n.step.is_none(),
                Ty::Opt(x) | Ty::List(x) | Ty::Set(x) | Ty::Record(RecId::Indexed(x)) => stepless(x),
                Ty::Record(RecId::Running(i, w)) | Ty::Record(RecId::Entry(i, w)) | Ty::Dict(i, w) => stepless(i) || stepless(w),
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
        for (ei, e) in d.enums.iter().enumerate() {
            for (vi, fs) in e.fields.iter().enumerate() {
                for (fi, f) in fs.iter().enumerate() {
                    if stepless(&f.slot.ty) {
                        let (en, vn, n) = (&e.name.text, &e.values[vi].text, &f.name.text);
                        let sp = self.module.enums[ei].values[vi].fields[fi].ty.span;
                        self.err("E108", sp, tr!("`{en}.{vn}` のフィールド `{n}` に、率の刻み（`rate[step 0.01%]`）がありません", "The field `{n}` of `{en}.{vn}` declares no step of the rate (`rate[step 0.01%]`)"))
                            .notes
                            .push(tr!(
                                "率は刻みの数で受け渡し、丸めていない値かどうかも刻みで決まります。そのため、列挙の値のフィールドの率にも刻みを書いてください。刻みを省けるのは `let` だけです。",
                                "A rate travels as a count of its step, and whether a value is rounded is told against the step; so the rate of a field of a value of an enum declares its step too. Only a `let` may leave it out."
                            ));
                    }
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

    /// The records and the enums whose values have fields that hold themselves (DESIGN 28.1): each one
    /// in a cycle of the module's declarations, through its fields (its values' fields), lists, dicts,
    /// sets, optionals, results and the type arguments it gives. A cycle is in one module: the modules
    /// a module reads cannot read it back (E203), and theirs reach its types only through type
    /// arguments. Each is marked `recursive`. E108 is said of one that has no value but those that
    /// hold another of its cycle (`record A` with the field `a : A` alone), which no value can be made
    /// of; and of one that gives a type of its cycle a type argument made from a type parameter
    /// (`Wrap[list[T]]` in `record Wrap[T]`), whose types would not end (`Wrap[list[list[T]]]`, …).
    fn recursion(&mut self, d: &mut Decls) {
        let file = self.canon.to_path_buf();
        // each record, then each enum whose values have fields, a node: its name, whether it is a
        // record, its type parameters, and its fields as a path names them (`placed.on` for a field of
        // a value), each with its place and where its type is written
        let mut nodes: Vec<RecNode> = Vec::new();
        for (ri, r) in d.records.iter().enumerate() {
            let fields = r.fields.iter().enumerate().map(|(fi, f)| (f.name.text.clone(), f.slot.clone(), self.module.records[ri].fields[fi].ty.span, None)).collect();
            nodes.push(RecNode { name: r.name.clone(), record: true, tparams: tnames(&r.tparams), fields, values: Vec::new(), index: ri });
        }
        for (ei, e) in d.enums.iter().enumerate() {
            if !e.is_sum() {
                continue;
            }
            let mut fields = Vec::new();
            for (vi, (v, fs)) in e.values.iter().zip(&e.fields).enumerate() {
                for (fi, f) in fs.iter().enumerate() {
                    fields.push((format!("{}.{}", v.text, f.name.text), f.slot.clone(), self.module.enums[ei].values[vi].fields[fi].ty.span, Some(vi)));
                }
            }
            let values = (0..e.values.len()).collect();
            nodes.push(RecNode { name: e.name.clone(), record: false, tparams: tnames(&e.tparams), fields, values, index: ei });
        }
        if nodes.is_empty() {
            return;
        }
        // the node a type names, a record of the module, an enum of it whose values have fields, or the
        // type of one of its values
        let node_of = |t: &Ty| -> Option<usize> {
            match t {
                Ty::Record(RecId::Module { file: f, name, .. }) if *f == file => nodes.iter().position(|n| n.record && n.name.text == *name),
                Ty::Record(RecId::Variant { file: f, name, .. }) if *f == file => nodes.iter().position(|n| !n.record && n.name.text == *name),
                Ty::Enum(EnumId { owner: Owner::Module(f), name, .. }) if *f == file => nodes.iter().position(|n| !n.record && n.name.text == *name),
                _ => None,
            }
        };
        // the nodes each field reaches, with the type arguments given there
        let mut edges: Vec<Vec<(usize, usize, Vec<Ty>)>> = vec![Vec::new(); nodes.len()];
        for (ni, n) in nodes.iter().enumerate() {
            for (fi, (_, slot, _, _)) in n.fields.iter().enumerate() {
                let mut found = Vec::new();
                named_types(&slot.ty, &mut found);
                for t in found {
                    if let Some(m) = node_of(&t) {
                        let args = match &t {
                            Ty::Record(RecId::Module { args, .. } | RecId::Variant { args, .. }) | Ty::Enum(EnumId { args, .. }) => args.clone(),
                            _ => Vec::new(),
                        };
                        edges[ni].push((m, fi, args));
                    }
                }
            }
        }
        // the cycles: the strongly connected parts of the graph (Tarjan's way), a node in one with
        // another, or reaching itself
        let parts = strong_parts(nodes.len(), &edges);
        let mut part_of = vec![0usize; nodes.len()];
        for (pi, p) in parts.iter().enumerate() {
            for &n in p {
                part_of[n] = pi;
            }
        }
        let recursive: Vec<bool> = (0..nodes.len()).map(|n| parts[part_of[n]].len() > 1 || edges[n].iter().any(|(m, _, _)| *m == n)).collect();
        for (ni, n) in nodes.iter().enumerate() {
            if n.record {
                d.records[n.index].recursive = recursive[ni];
            } else {
                d.enums[n.index].recursive = recursive[ni];
            }
        }
        // a type of the cycle given a type argument made from a type parameter: its types do not end
        let mut said: Vec<(Span, Text)> = Vec::new();
        for (ni, n) in nodes.iter().enumerate() {
            if !recursive[ni] {
                continue;
            }
            for (m, fi, args) in &edges[ni] {
                if part_of[*m] != part_of[ni] {
                    continue;
                }
                let grows = args.iter().find(|a| !matches!(a, Ty::Param(p) if n.tparams.contains(p)) && a.has_param());
                if let Some(a) = grows {
                    let (me, them) = (n.name.text.clone(), nodes[*m].name.text.clone());
                    let shown = self.show_type(&Ty::Record(RecId::Module { file: file.clone(), name: them.clone(), args: args.clone() }));
                    let arg = self.show_type(a);
                    let (_, _, at, _) = &n.fields[*fi];
                    if said.iter().any(|(sp, _)| sp == at) {
                        continue;
                    }
                    said.push((
                        *at,
                        tr!(
                            "`{me}` は、自分を含むところ（`{shown}`）で、型の引数から作った型 `{arg}` を渡しています",
                            "Where `{me}` holds itself (`{shown}`), it gives `{arg}`, a type argument made from a type parameter"
                        ),
                    ));
                }
            }
        }
        let note = tr!(
            "型の引数が、含むたびに一段ずつ深くなり（`list[list[T]]`、…）、型が限りなく増えます。生成した Go もこの形を書けません。自分を含むところでは、型の引数をそのまま渡すか、型の引数を含まない型を渡してください。",
            "Each time the types go round, the type argument grows one deeper (`list[list[T]]`, …), and the types never end; nor can the generated Go write them. Where a type holds itself, give it the type parameters as they are, or types that name none."
        );
        for (at, msg) in said {
            self.err("E108", at, msg).notes.push(note.clone());
        }
        // what can be made with no value of the cycle in it but those its fields give: a node is made
        // where every field can be (a record), or every field of one of its values (an enum); a type
        // outside the cycle counts as one that can be made (it is said where it is declared, if not)
        let world = self.world;
        for (pi, p) in parts.iter().enumerate() {
            if !p.iter().any(|&n| recursive[n]) {
                continue;
            }
            let mut made = vec![false; nodes.len()];
            loop {
                let mut more = false;
                for &n in p {
                    if made[n] {
                        continue;
                    }
                    let ok = |slot: &Slot| makes(&slot.ty, slot.count.as_ref(), &file, &world, d, &|t| node_of(t).filter(|m| part_of[*m] == pi).map(|m| made[m]), 0);
                    let can = if nodes[n].record {
                        nodes[n].fields.iter().all(|(_, s, _, _)| ok(s))
                    } else {
                        nodes[n].values.iter().any(|vi| nodes[n].fields.iter().filter(|(_, _, _, v)| *v == Some(*vi)).all(|(_, s, _, _)| ok(s)))
                    };
                    if can {
                        made[n] = true;
                        more = true;
                    }
                }
                if !more {
                    break;
                }
            }
            for &n in p {
                if made[n] {
                    continue;
                }
                // the way it holds another of its cycle, as a path names it, for the message
                let via = cycle_path(n, &nodes, &edges, &part_of);
                let t = nodes[n].name.text.clone();
                let msg = if nodes[n].record {
                    tr!("レコード `{t}` の値は、どれも `{t}.{via}` を通して別の `{t}` を含むので、一つも作れません", "Every value of the record `{t}` holds another `{t}` (through `{t}.{via}`), so none can be made")
                } else {
                    tr!("列挙 `{t}` の値は、どれも `{t}.{via}` を通して別の `{t}` を含むので、一つも作れません", "Every value of the enum `{t}` holds another `{t}` (through `{t}.{via}`), so none can be made")
                };
                let at = nodes[n].name.span;
                self.err("E108", at, msg).notes.push(tr!(
                    "自分を含む型には、自分を含まない値が一つは要ります。自分を含むフィールドをオプショナル（`T?`）か空にできるリストにするか、列挙に自分を含まない値を足してください。",
                    "A type that holds itself needs a value that does not: make a field that holds it optional (`T?`) or a list that may be empty, or give the enum a value that does not hold it."
                ));
            }
        }
    }

    /// E108 for a rate a function's parameter takes with no step (DESIGN 3.9): a rate comes in as a
    /// count of its step. A field's step is said for every record (`steps`); a range and a count may
    /// be left out, the type's whole taken (DESIGN 22.1).
    fn reached(&mut self, d: &Decls) {
        let mut done: BTreeSet<(PathBuf, String)> = BTreeSet::new();
        for (fi, f) in d.fns.iter().enumerate() {
            for (pi, p) in f.params.iter().enumerate() {
                let origin = Origin { function: f.name.text.clone(), param: p.name.text.clone(), span: self.module.fns[fi].params[pi].name.span };
                self.needs(d, &p.slot, None, &origin, &mut done);
            }
        }
    }

    /// What a place reached from a parameter needs: a rate of the parameter itself, its step (a field's
    /// is said for every record), and so on into the records it reaches. `field` is the field it is,
    /// with its record and the path of the record's module when it is another's; None for the
    /// parameter itself.
    fn needs(&mut self, d: &Decls, slot: &Slot, field: Option<(&FieldInfo, &str, Option<&str>)>, origin: &Origin, done: &mut BTreeSet<(PathBuf, String)>) {
        let mut t = &slot.ty;
        while let Ty::Opt(x) | Ty::List(x) | Ty::Set(x) = t {
            t = x;
        }
        // a dict: its keys, then its values
        if let Ty::Dict(k, v) = t {
            let (k, v) = (Slot::of((**k).clone()), Slot::of((**v).clone()));
            self.needs(d, &k, field, origin, done);
            self.needs(d, &v, field, origin, done);
            return;
        }
        match t {
            Ty::Num(n) if n.is_rate() => {
                if n.step.is_none() && field.is_none() {
                    self.stepless(origin);
                }
            }
            Ty::Record(RecId::Module { file, name, .. }) => {
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

    /// E108 for a parameter's rate with no step.
    fn stepless(&mut self, origin: &Origin) {
        let (fun, param) = (&origin.function, &origin.param);
        let d = self.err(
            "E108",
            origin.span,
            tr!("関数 `{fun}` の引数 `{param}` に、率の刻み（`rate[step 0.01%]`）がありません", "The parameter `{param}` of `{fun}` declares no step of the rate (`rate[step 0.01%]`)"),
        );
        d.notes.push(tr!(
            "率は刻みの数で受け渡すので、引数の率には刻みを書いてください。範囲は省けて、省けば 0% から 100% です。",
            "A rate is given as a count of its step, so a parameter's rate declares its step; it may leave its range out, and is then 0% to 100%."
        ));
    }
}

/// What a key can be (E114, DESIGN 24.1): said where a type cannot be one.
pub fn key_note() -> Text {
    tr!(
        "鍵にできるのは、数、お金、量、刻みのある率、日付、文字列、bool と、フィールドの無い列挙の値です。kaname は鍵を型ごとに決めた順で並べるので、辞書と集合は、どの出力先でも同じ順で回ります。",
        "A key is a number, an amount, a quantity, a rate with a step, a date, a string, a bool, or a value of an enum whose values have no fields: kaname puts keys in the order it gives each such type, so that a dict and a set go round in one order in every target."
    )
}

/// A record's, an enum's or a value's type with these type arguments (DESIGN 26.1).
pub fn with_targs(t: Ty, targs: Vec<Ty>) -> Ty {
    match t {
        Ty::Enum(id) => Ty::Enum(EnumId { args: targs, ..id }),
        Ty::Record(RecId::Module { file, name, .. }) => Ty::Record(RecId::Module { file, name, args: targs }),
        Ty::Record(RecId::Variant { file, name, value, .. }) => Ty::Record(RecId::Variant { file, name, value, args: targs }),
        t => t,
    }
}

/// A type's name with its type arguments, as a module writes it: `Page[Invoice]`.
pub fn with_args(name: String, args: &[String]) -> String {
    if args.is_empty() { name } else { format!("{name}[{}]", args.join(", ")) }
}

/// What a type parameter that is not held to keys cannot be (E114, DESIGN 26.2).
pub fn param_key_note(n: &str) -> Text {
    tr!(
        "型の引数の値を鍵にしたり、大小で比べたり、並べたりするには、型の引数に `key` を付けてください（`[{n}: key]`）。鍵にできる型だけが渡せるようになります。",
        "To make a value of a type parameter a key, to compare it by `<` or to sort by it, hold the parameter to keys: `[{n}: key]`. Only the types that can be keys are then given to it."
    )
}

/// The type of the value `v` of the enum `name` of the module at `file`, its values each with whether
/// it has fields (DESIGN 23.2): Err with why where the enum's values have no fields, or it has no
/// such value.
fn variant_type<'n>(file: &Path, name: &str, values: impl Iterator<Item = (&'n Name, bool)>, v: &str) -> Result<Ty, Text> {
    let values: Vec<(&Name, bool)> = values.collect();
    if !values.iter().any(|(_, f)| *f) {
        return Err(tr!(
            "列挙 `{name}` の値にはフィールドが無いので、値は型になりません。型は `{name}` です。",
            "The values of the enum `{name}` have no fields, so a value of it is no type: its type is `{name}`."
        ));
    }
    match values.iter().find(|(x, _)| x.is(v)) {
        Some((x, _)) => Ok(Ty::Record(RecId::Variant { file: file.to_path_buf(), name: name.to_string(), value: x.text.clone(), args: Vec::new() })),
        None => {
            let all: Vec<String> = values.iter().map(|(x, _)| format!("`{}`", x.text)).collect();
            let (ja, en) = (all.join("、"), all.join(", "));
            Err(tr!("列挙 `{name}` に値 `{v}` はありません。値は {ja} です。", "The enum `{name}` has no value `{v}`; its values are {en}."))
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

/// A record or an enum whose values have fields, as the check of what holds itself reads it (DESIGN
/// 28.1).
struct RecNode {
    name: Name,
    record: bool,
    tparams: Vec<String>,
    /// Each field, as a path names it (`placed.on` for a field of a value), its place, where its type
    /// is written, and the value it is of (an enum's).
    fields: Vec<(String, Slot, Span, Option<usize>)>,
    /// The values, by their places (an enum's).
    values: Vec<usize>,
    /// Its place among the module's records, or its enums.
    index: usize,
}

/// The records and the enums a type names, with their type arguments, and those its type arguments
/// name (`list[Box[Category]]` names `Box[Category]` and `Category`).
fn named_types(t: &Ty, out: &mut Vec<Ty>) {
    match t {
        Ty::List(x) | Ty::Set(x) | Ty::Opt(x) => named_types(x, out),
        Ty::Dict(a, b) | Ty::Result(a, b) => {
            named_types(a, out);
            named_types(b, out);
        }
        Ty::Fn(ps, r, _) => {
            for p in ps {
                named_types(p, out);
            }
            named_types(r, out);
        }
        Ty::Enum(id) => {
            out.push(t.clone());
            for a in &id.args {
                named_types(a, out);
            }
        }
        Ty::Record(RecId::Module { args, .. } | RecId::Variant { args, .. }) => {
            out.push(t.clone());
            for a in args {
                named_types(a, out);
            }
        }
        Ty::Record(RecId::Running(a, b) | RecId::Entry(a, b)) => {
            named_types(a, out);
            named_types(b, out);
        }
        Ty::Record(RecId::Indexed(x)) => named_types(x, out),
        _ => {}
    }
}

/// The strongly connected parts of a graph of `n` nodes (Tarjan's way), each a list of its nodes: the
/// cycles of the types (here) and of the calls (`types.rs`, DESIGN 26.2). An edge is the node it goes
/// to, with what the caller keeps of it.
pub(crate) fn strong_parts(n: usize, edges: &[Vec<(usize, usize, Vec<Ty>)>]) -> Vec<Vec<usize>> {
    struct T<'e> {
        edges: &'e [Vec<(usize, usize, Vec<Ty>)>],
        index: Vec<Option<usize>>,
        low: Vec<usize>,
        on: Vec<bool>,
        stack: Vec<usize>,
        next: usize,
        parts: Vec<Vec<usize>>,
    }
    fn visit(t: &mut T, v: usize) {
        t.index[v] = Some(t.next);
        t.low[v] = t.next;
        t.next += 1;
        t.stack.push(v);
        t.on[v] = true;
        for k in 0..t.edges[v].len() {
            let w = t.edges[v][k].0;
            match t.index[w] {
                None => {
                    visit(t, w);
                    t.low[v] = t.low[v].min(t.low[w]);
                }
                Some(i) if t.on[w] => t.low[v] = t.low[v].min(i),
                Some(_) => {}
            }
        }
        if Some(t.low[v]) == t.index[v] {
            let mut part = Vec::new();
            while let Some(w) = t.stack.pop() {
                t.on[w] = false;
                part.push(w);
                if w == v {
                    break;
                }
            }
            part.sort();
            t.parts.push(part);
        }
    }
    let mut t = T { edges, index: vec![None; n], low: vec![0; n], on: vec![false; n], stack: Vec::new(), next: 0, parts: Vec::new() };
    for v in 0..n {
        if t.index[v].is_none() {
            visit(&mut t, v);
        }
    }
    t.parts
}

/// The way the node `n` holds another of its cycle, as a path names it from the node (`a`, `b.a`,
/// `node.left`): the shortest, the fields in the order they are declared.
fn cycle_path(n: usize, nodes: &[RecNode], edges: &[Vec<(usize, usize, Vec<Ty>)>], part_of: &[usize]) -> String {
    let mut seen = vec![false; nodes.len()];
    let mut queue: std::collections::VecDeque<(usize, String)> = std::collections::VecDeque::new();
    queue.push_back((n, String::new()));
    seen[n] = true;
    while let Some((at, path)) = queue.pop_front() {
        for (m, fi, _) in &edges[at] {
            if part_of[*m] != part_of[n] {
                continue;
            }
            let step = &nodes[at].fields[*fi].0;
            let p = if path.is_empty() { step.clone() } else { format!("{path}.{step}") };
            if *m == n {
                return p;
            }
            if !seen[*m] {
                seen[*m] = true;
                queue.push_back((*m, p));
            }
        }
    }
    String::new()
}

/// Whether a value of `t` can be made (DESIGN 28.1), `count` the count written on it: a list, a dict
/// or a set that may be empty can; an optional can (none); a result where `ok` or `err` can; a record
/// where each field can; an enum whose values have fields where every field of one of its values can;
/// the type of one of its values where each of its fields can. `cycle` answers for the types of the
/// cycle looked at (whether each can be made, as far as is known yet). Any other type that takes no
/// type arguments can (where it cannot, that is said where it is declared); one that takes them is
/// looked into with them put in, the module's own from `d` and another's from `world`.
#[allow(clippy::too_many_arguments)]
fn makes(t: &Ty, count: Option<&Range>, file: &Path, world: &World, d: &Decls, cycle: &dyn Fn(&Ty) -> Option<bool>, depth: usize) -> bool {
    if depth > 32 {
        return false;
    }
    let empty = count.is_none_or(|c| c.min.is_none_or(|m| m.num <= 0));
    let go = |x: &Ty| makes(x, None, file, world, d, cycle, depth + 1);
    match t {
        Ty::Opt(_) => true,
        Ty::List(x) | Ty::Set(x) => empty || go(x),
        Ty::Dict(_, v) => empty || go(v),
        Ty::Result(a, e) => go(a) || go(e),
        Ty::Record(RecId::Running(a, b) | RecId::Entry(a, b)) => go(a) && go(b),
        Ty::Record(RecId::Indexed(x)) => go(x),
        _ => {
            let records = |f: &Path| -> Option<&[RecordInfo]> { if f == file { Some(&d.records) } else { world.modules.get(f).map(|c| c.decls.records.as_slice()) } };
            let enums = |f: &Path| -> Option<&[EnumInfo]> { if f == file { Some(&d.enums) } else { world.modules.get(f).map(|c| c.decls.enums.as_slice()) } };
            let each = |fields: &[FieldInfo], ps: &[String], args: &[Ty]| fields.iter().all(|f| {
                let s = subst_slot(&f.slot, ps, args);
                makes(&s.ty, s.count.as_ref(), file, world, d, cycle, depth + 1)
            });
            // the type of one value of an enum of the cycle: that value's own fields, not whether
            // another value of the enum can be made
            if let Ty::Record(RecId::Variant { file: f, name, value, args }) = t
                && cycle(t).is_some()
            {
                return match enums(f).and_then(|es| es.iter().find(|e| e.name.text == *name)) {
                    Some(e) => e.fields_of(value).is_none_or(|fs| each(fs, &tnames(&e.tparams), args)),
                    None => true,
                };
            }
            if let Some(m) = cycle(t) {
                return m;
            }
            match t {
                Ty::Record(RecId::Module { file: f, name, args }) if !args.is_empty() => match records(f).and_then(|rs| rs.iter().find(|r| r.name.text == *name)) {
                    Some(r) => each(&r.fields, &tnames(&r.tparams), args),
                    None => true,
                },
                Ty::Enum(EnumId { owner: Owner::Module(f), name, args }) if !args.is_empty() => match enums(f).and_then(|es| es.iter().find(|e| e.name.text == *name)) {
                    Some(e) => e.fields.iter().any(|fs| each(fs, &tnames(&e.tparams), args)),
                    None => true,
                },
                Ty::Record(RecId::Variant { file: f, name, value, args }) if !args.is_empty() => match enums(f).and_then(|es| es.iter().find(|e| e.name.text == *name)) {
                    Some(e) => e.fields_of(value).is_none_or(|fs| each(fs, &tnames(&e.tparams), args)),
                    None => true,
                },
                _ => true,
            }
        }
    }
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
