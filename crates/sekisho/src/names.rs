//! What each name refers to, and the types of the values (DESIGN 2, 3.1–3.3, 5.7, 10). A file that
//! passes this has a [`Scope`]: every reference finds what it names (in the file, or in a `.gate`
//! it reads with `use gate`), every value has a type, and what the rules and the dates files it
//! reads say of themselves is at hand.
//!
//! - E002: a name or an alias that is one of the words a condition is made of (`kw::RESERVED`).
//! - E006: a name or an alias declared twice where they share a namespace (the types and the enums,
//!   the roles, an enum's values, a type's attributes, the workflows, the actions, an action's
//!   inputs and computed values, the policies, the expectations, the separations, the names of the
//!   `use` lines), or an input of a rule given twice.
//! - E007, E008: a name that goes to Cedar without an ASCII alias of its form, or one Cedar or the
//!   generated code cannot take.
//! - E101–E107: a name that names nothing, a type that does not fit, a range, a relation v1 does not
//!   write, a computed value, a principal the actions never take, today.
//! - E201: a file a `use` reads that its language does not pass (asked through the port), or a `.gate`
//!   read with `use gate` that sekisho does not pass.
//! - E209: a rule, a dates file, a calendar, a book or a flow, read where the language is not joined.
//!
//! What only the other languages can decide over ranges (a value given to a rule against the
//! rule's range and its preconditions, E206; a calendar's data, E207; a flow, E208) and what the
//! contracts say (E202–E205) are the checks after this one.

use crate::ast::*;
use crate::diag::Diag;
use crate::kw;
use crate::suite::{Language, Suite};
use crate::types::{self, Miss};
use ritsu_base::text::{Text, capitalize};
use ritsu_ports::{ColumnType, DateFacts, DateKind, Day, RuleFacts, Said};
use ritsu_units::Unit;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path as FsPath, PathBuf};

/// What the names of a file come to: the diagnostics, and the scope when there is no error.
pub struct Named {
    pub diags: Vec<Diag>,
    pub scope: Option<Scope>,
}

/// A file a `use` names, as its language's port is asked with it (the `.gate`'s directory joined
/// to the path written), and what the language says of it.
#[derive(Clone, Debug)]
pub struct Read<T> {
    pub path: PathBuf,
    pub facts: T,
}

/// A file whose names hold together, with what it reads.
#[derive(Clone, Debug)]
pub struct Scope {
    /// The file checked, then the `.gate` files it reads with `use gate` (and what those read), in
    /// the order first read.
    pub files: Vec<File>,
    /// What rulec says of each rule the file reads, by the name of its `use rule`.
    pub rules: BTreeMap<String, Read<RuleFacts>>,
    /// What koyomi says of each dates file the file reads, by the name of its `use dates`.
    pub dates: BTreeMap<String, Read<DateFacts>>,
}

/// The type of an attribute or an input, once its names hold.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldType {
    Bool,
    /// The days of its range, both ends in.
    Date { lo: Day, hi: Day },
    /// A number of a unit, and its range counted in the unit (the integers on the wire), both ends in.
    Number { unit: Unit, lo: i128, hi: i128 },
    /// An enum of the gate, by what Cedar calls it.
    Enum(String),
    /// A principal's or a resource's type, by what Cedar calls it.
    Entity(String),
}

/// What a computed value can come to, as the rule or the date says.
#[derive(Clone, Debug, PartialEq)]
pub enum Domain {
    Bool,
    /// The values of the rule's enum, as rulec's port gives them, in the rule's order: each value's
    /// name in the rule (`within_limit`, `上限まで`) and its alias there, which is the member of the
    /// generated code (`WithinLimit`, for both).
    Enum(Vec<(String, String)>),
}

impl Scope {
    /// The file checked.
    pub fn file(&self) -> &File {
        &self.files[0]
    }

    pub fn role(&self, word: &str) -> Option<&RoleDecl> {
        self.files.iter().find_map(|f| f.role(word))
    }

    pub fn principal(&self, word: &str) -> Option<&EntityDecl> {
        self.files.iter().find_map(|f| f.principal(word))
    }

    pub fn resource(&self, word: &str) -> Option<&EntityDecl> {
        self.files.iter().find_map(|f| f.resource(word))
    }

    /// A principal's or a resource's type, or sekisho's `Workflow`.
    pub fn entity(&self, word: &str) -> Option<EntityRef<'_>> {
        if word == WORKFLOW_TYPE {
            return Some(EntityRef::Workflow);
        }
        self.principal(word).or_else(|| self.resource(word)).map(EntityRef::Declared)
    }

    pub fn enum_decl(&self, word: &str) -> Option<&EnumDecl> {
        self.files.iter().find_map(|f| f.enum_decl(word))
    }

    pub fn workflow(&self, word: &str) -> Option<&WorkflowDecl> {
        self.files.iter().find_map(|f| f.workflow(word))
    }

    /// The Cedar namespace: as written, or the file's alias in Pascal case (DESIGN 5.1).
    pub fn namespace(&self) -> String {
        namespace_of(self.file())
    }

    /// `today`: the first and the last day, and the offset in minutes east of UTC.
    pub fn today(&self) -> Option<(Day, Day, i32)> {
        let t = self.file().today.as_ref()?;
        match (&t.range.lo, &t.range.hi, offset(&t.offset.0)) {
            (Some((Lit::Date(lo), _)), Some((Lit::Date(hi), _)), Ok(m)) => Some((*lo, *hi, m)),
            _ => None,
        }
    }

    /// A file a `use` of the checked file names, from where sekisho runs.
    pub fn path_of(&self, written: &str) -> PathBuf {
        beside(&self.file().path, written)
    }

    /// The forbids of the files read with `use gate` that hold for every action (`action any`):
    /// they hold for the checked file's actions too (DESIGN 2.10).
    pub fn imported_forbids(&self) -> Vec<(&File, &Policy)> {
        self.files[1..].iter().flat_map(|f| f.policies.iter().filter(|p| p.effect == Effect::Forbid && matches!(p.body.action.0, What::Any)).map(move |p| (f, p))).collect()
    }

    /// The roles a principal that holds `role` holds: the role and every role it includes, by what
    /// Cedar calls them.
    pub fn held(&self, role: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut todo = vec![role.to_string()];
        while let Some(r) = todo.pop() {
            let Some(d) = self.role(&r) else { continue };
            if out.insert(d.name.ascii().to_string()) {
                todo.extend(d.includes.iter().map(|i| i.word.clone()));
            }
        }
        out
    }

    /// The type of an attribute or an input of a file that passed.
    pub fn field_type(&self, f: &Field) -> FieldType {
        let end = |e: &Option<(Lit, Span)>| e.as_ref().map(|(l, _)| l.clone());
        let range = f.range.as_ref();
        match &f.ty {
            Type::Bool => FieldType::Bool,
            Type::Date => {
                let day = |l: Option<Lit>| match l {
                    Some(Lit::Date(d)) => d,
                    _ => 0,
                };
                FieldType::Date { lo: day(range.and_then(|r| end(&r.lo))), hi: day(range.and_then(|r| end(&r.hi))) }
            }
            Type::Unit(u) => {
                let unit = types::unit(u).unwrap_or_else(|_| Unit::number());
                let count = |l: Option<Lit>| match l {
                    Some(Lit::Num(n)) => types::count(&n, &unit).unwrap_or(0),
                    _ => 0,
                };
                let (lo, hi) = (count(range.and_then(|r| end(&r.lo))), count(range.and_then(|r| end(&r.hi))));
                FieldType::Number { unit, lo, hi }
            }
            Type::Named(r) => match self.enum_decl(&r.word) {
                Some(e) => FieldType::Enum(e.name.ascii().to_string()),
                None => FieldType::Entity(match self.entity(&r.word) {
                    Some(EntityRef::Declared(e)) => e.name.ascii().to_string(),
                    _ => r.word.clone(),
                }),
            },
        }
    }

    /// What a computed value of a file that passed can come to.
    pub fn domain(&self, c: &Computed) -> Domain {
        match &c.value {
            Computation::Rule { rule, output, .. } => {
                let Some(read) = self.rules.get(&rule.word) else { return Domain::Bool };
                let facts = &read.facts;
                match facts.outputs.iter().find(|o| o.name == output.word || o.alias == output.word).map(|o| unopt(&o.ty)) {
                    Some(ColumnType::Enum(e)) => Domain::Enum(rule_values(facts, e)),
                    _ => Domain::Bool,
                }
            }
            Computation::Date { .. } | Computation::Open { .. } => Domain::Bool,
        }
    }
}

impl Scope {
    /// What rulec says of the rule a computed value calls.
    pub fn rule_of(&self, c: &Computed) -> Option<&Read<RuleFacts>> {
        match &c.value {
            Computation::Rule { rule, .. } => self.rules.get(&rule.word),
            _ => None,
        }
    }

    /// What koyomi says of the dates file a computed value calls.
    pub fn dates_of(&self, c: &Computed) -> Option<&Read<DateFacts>> {
        match &c.value {
            Computation::Date { date: DateValue::Call { dates, .. }, .. } => self.dates.get(&dates.word),
            _ => None,
        }
    }

    /// The principal types and the resource types of an action a computed value is computed for:
    /// those that have every attribute it reads (`principal.refund_limit` is no attribute of a
    /// workflow, and the value is not computed for one, DESIGN 3.7).
    pub fn computed_for(&self, a: &ActionDecl, c: &Computed) -> (Vec<String>, Vec<String>) {
        let paths = |args: &'_ [Arg]| -> Vec<Path> {
            args.iter()
                .filter_map(|x| match &x.value {
                    ArgValue::Path(p) => Some(p.clone()),
                    _ => None,
                })
                .collect()
        };
        let reads: Vec<Path> = match &c.value {
            Computation::Rule { args, .. } => paths(args),
            Computation::Date { date: DateValue::Call { args, .. }, .. } => paths(args),
            Computation::Date { date: DateValue::Attr(p), .. } => vec![p.clone()],
            Computation::Open { .. } => vec![],
        };
        let has = |ty: &str, principal: bool| {
            reads.iter().all(|p| match (p, principal) {
                (Path::Principal(r), true) | (Path::Resource(r), false) => match self.entity(ty) {
                    Some(EntityRef::Declared(e)) => e.attribute(&r.word).is_some(),
                    _ => false,
                },
                _ => true,
            })
        };
        let name = |r: &Ref| match self.entity(&r.word) {
            Some(EntityRef::Declared(e)) => e.name.ascii().to_string(),
            _ => r.word.clone(),
        };
        let ps = a.principals.0.iter().map(name).filter(|t| has(t, true)).collect();
        let rs = a.resources.0.iter().map(name).filter(|t| has(t, false)).collect();
        (ps, rs)
    }
}

/// The input of a rule a word names (its name or its alias): rulec's name of it is `.name`.
pub fn rule_input<'f>(facts: &'f RuleFacts, word: &str) -> Option<&'f ritsu_ports::Column> {
    facts.inputs.iter().find(|i| i.name == word || i.alias == word)
}

/// The value of a rule's enum a word names (its name or its alias).
pub fn rule_value<'f>(facts: &'f RuleFacts, enum_name: &str, word: &str) -> Option<&'f ritsu_ports::EnumValue> {
    facts.enums.iter().find(|e| e.name == enum_name || e.alias == enum_name)?.values.iter().find(|v| v.name == word || v.alias == word || v.public == word)
}

/// The input of a dates file a word names (its name or its alias).
pub fn date_input<'f>(facts: &'f DateFacts, word: &str) -> Option<&'f ritsu_ports::DateInput> {
    facts.inputs.iter().find(|i| i.name == word || i.alias == word)
}

/// A file's Cedar namespace: as written, or its alias in Pascal case (DESIGN 5.1).
pub fn namespace_of(f: &File) -> String {
    match &f.namespace {
        Some((segs, _)) => segs.join("::"),
        None => ritsu_emit::ident::pascal(f.alias()),
    }
}

fn unopt(t: &ColumnType) -> &ColumnType {
    match t {
        ColumnType::Opt(x) => unopt(x),
        t => t,
    }
}

/// The values of a rule's enum, as rulec's port gives them.
fn rule_enum_values<'f>(facts: &'f RuleFacts, name: &str) -> &'f [ritsu_ports::EnumValue] {
    facts.enums.iter().find(|e| e.name == name || e.alias == name).map(|e| e.values.as_slice()).unwrap_or(&[])
}

/// Whether a word names a value of a rule's enum: by the rule's name for it, by the member of the
/// generated code, or by the alias the `.rule` writes (the string Cedar is given).
fn is_rule_value(v: &ritsu_ports::EnumValue, word: &str) -> bool {
    v.name == word || v.alias == word || v.public == word
}

/// The values of a rule's enum, by rulec's name and alias.
fn rule_values(facts: &RuleFacts, name: &str) -> Vec<(String, String)> {
    facts.enums.iter().find(|e| e.name == name || e.alias == name).map(|e| e.values.iter().map(|v| (v.name.clone(), v.alias.clone())).collect()).unwrap_or_default()
}

/// A file named in a `.gate`, from where sekisho runs: the `.gate`'s directory joined to it.
fn beside(gate: &str, written: &str) -> PathBuf {
    let dir = FsPath::new(gate).parent().unwrap_or(FsPath::new(""));
    if dir.as_os_str().is_empty() { PathBuf::from(written) } else { dir.join(written) }
}

/// `+09:00` as minutes east of UTC, or why it is not one (E107): koyomi's way, since a `today`
/// changes days as a calendar does (koyomi's DESIGN 1.9).
pub fn offset(text: &str) -> Result<i32, (Text, Vec<Text>)> {
    let t = text.trim();
    let b = t.as_bytes();
    if b.len() == 6 && (b[0] == b'+' || b[0] == b'-') && b[1].is_ascii_digit() && b[2].is_ascii_digit() && b[3] == b':' && b[4].is_ascii_digit() && b[5].is_ascii_digit() {
        let h: i32 = t[1..3].parse().unwrap_or(99);
        let m: i32 = t[4..6].parse().unwrap_or(99);
        if h > 23 || m > 59 {
            return Err((tr!("{t} というオフセットはありません", "There is no offset {t}"), vec![]));
        }
        let v = h * 60 + m;
        return Ok(if b[0] == b'-' { -v } else { v });
    }
    let upper = t.to_ascii_uppercase();
    if upper == "UTC" || upper == "GMT" || upper == "Z" {
        return Err((tr!("オフセットは `±HH:MM` の形で書いてください（`{t}` ではなく `+00:00`）", "Write the offset as `±HH:MM`: `+00:00`, not `{t}`"), vec![]));
    }
    if t.contains('/') || t.chars().any(|c| c.is_ascii_alphabetic()) {
        return Err((
            tr!("オフセットにタイムゾーンの名前 `{t}` が書かれています。sekisho は固定の UTC オフセットだけを扱います", "The offset is a time zone's name, `{t}`; sekisho takes only a fixed UTC offset"),
            vec![
                tr!(
                    "夏時間のあるタイムゾーンでは、同じ時刻でも季節でオフセットが変わります。どのタイムゾーンに夏時間があるかを知るには tz データベースが要るので、sekisho はどの名前も受け付けません（koyomi と同じ）。",
                    "In a time zone with daylight saving time, the offset changes with the season, and knowing which zones have it takes the tz database; so no name is taken (as in koyomi)."
                ),
                tr!(
                    "日を変える時刻を一つに決めて、`offset +00:00` のように数で書いてください。夏時間のある地域では、夏のあいだ日の変わり目が現地の 0 時とずれることを、コメントに書いておくと読む人に伝わります。",
                    "Pick the one offset the day changes at, and write it as a number, like `offset +00:00`; where summer time moves the local midnight, say so in a comment for the reader."
                ),
            ],
        ));
    }
    Err((tr!("オフセット `{t}` は `±HH:MM` の形ではありません（`+00:00` のように書いてください）", "The offset `{t}` is not `±HH:MM` (write it like `+00:00`)"), vec![]))
}

/// What a declared name is, for its namespace, its alias's form and its messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    File,
    Enum,
    Value,
    Role,
    Type,
    Attribute,
    Workflow,
    Action,
    Input,
    Computed,
    Policy,
    Expect,
    Separate,
}

impl Kind {
    fn noun(self) -> Text {
        match self {
            Kind::File => tr!("ファイル", "file"),
            Kind::Enum => tr!("列挙", "enum"),
            Kind::Value => tr!("列挙の値", "value"),
            Kind::Role => tr!("役割", "role"),
            Kind::Type => tr!("型", "type"),
            Kind::Attribute => tr!("属性", "attribute"),
            Kind::Workflow => tr!("ワークフロー", "workflow"),
            Kind::Action => tr!("action", "action"),
            Kind::Input => tr!("入力", "input"),
            Kind::Computed => tr!("計算した値", "computed value"),
            Kind::Policy => tr!("ポリシー", "policy"),
            Kind::Expect => tr!("期待", "expectation"),
            Kind::Separate => tr!("職務の分離", "separation"),
        }
    }

    /// The noun with its article: `an attribute`, `a role`.
    fn a(self) -> String {
        let n = self.noun().en;
        if n.starts_with(['a', 'e', 'i', 'o', 'u']) { format!("an {n}") } else { format!("a {n}") }
    }

    /// Whether the name goes to Cedar or to the generated code, and needs an ASCII alias.
    fn to_cedar(self) -> bool {
        !matches!(self, Kind::Expect | Kind::Separate)
    }
}

/// `[a-z][a-z0-9_]*`.
fn is_lower(s: &str) -> bool {
    let mut cs = s.chars();
    matches!(cs.next(), Some('a'..='z')) && cs.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// `[A-Z][A-Za-z0-9]*`.
fn is_upper(s: &str) -> bool {
    let mut cs = s.chars();
    matches!(cs.next(), Some('A'..='Z')) && cs.all(|c| c.is_ascii_alphanumeric())
}

/// The targets whose reserved words have `alias` (DESIGN 10, E008): TypeScript's reserved words
/// and strict mode's, Python's keywords, Go's keywords (ritsu-emit's lists, from their standards).
fn targets_refusing(alias: &str) -> Vec<&'static str> {
    use ritsu_emit::words::{go, python, typescript};
    let mut out = Vec::new();
    if typescript::RESERVED.contains(&alias) || typescript::STRICT.contains(&alias) {
        out.push("TypeScript");
    }
    if python::KEYWORDS.contains(&alias) {
        out.push("Python");
    }
    if go::KEYWORDS.contains(&alias) {
        out.push("Go");
    }
    out
}

/// A name the form of an alias: lower case with `_` for what is not a letter or a digit.
fn suggest_lower(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

/// A value's type, as the checks compare them.
#[derive(Clone, Debug, PartialEq)]
enum Ty {
    Bool,
    Date,
    Num(Unit),
    /// An enum of the gate, by what Cedar calls it.
    Enum(String),
    /// A rule's enum: the `use rule`'s name, and the enum's name in the rule.
    RuleEnum(String, String),
    /// A principal's or a resource's type, by what Cedar calls it.
    Entity(String),
}

impl Ty {
    fn text(&self) -> Text {
        match self {
            Ty::Bool => tr!("真偽（bool）", "true or false (bool)"),
            Ty::Date => tr!("日付（date）", "a date"),
            Ty::Num(u) => tr!("数（{u}）", "a number ({u})"),
            Ty::Enum(e) => tr!("列挙 `{e}`", "the enum `{e}`"),
            Ty::RuleEnum(r, e) => tr!("規則 `{r}` の列挙 `{e}`", "the enum `{e}` of the rule `{r}`"),
            Ty::Entity(t) => tr!("型 `{t}`", "the type `{t}`"),
        }
    }
}

/// The names of the `use` lines that a file reads through a port of a language the suite does not
/// join, and the first line that does (E209).
fn unjoined(f: &File, suite: &Suite) -> Option<Diag> {
    let mut first: Option<(Span, Text)> = None;
    let mut take = |s: Span, t: Text| {
        if first.as_ref().is_none_or(|(at, _)| s < *at) {
            first = Some((s, t));
        }
    };
    for u in &f.uses {
        if let Some(l) = Language::of_use(u.kind)
            && !suite.joins(l)
        {
            let what = match u.kind {
                UseKind::Rule => tr!("規則", "rules"),
                UseKind::Dates => tr!("日付のファイル", "dates files"),
                UseKind::Calendar => tr!("カレンダー", "calendars"),
                _ => tr!("帳簿", "books"),
            };
            let name = u.name.as_ref().map(|(n, _)| n.clone()).unwrap_or_default();
            let tool = l.word();
            take(u.span, tr!("この sekisho は{}を読めません（{tool} が入っていません）。このファイルは `{name}` を読みます", "this sekisho cannot read {} ({tool} is not in it), and the file reads `{name}`", what.ja; what.en));
        }
    }
    if let Some(w) = f.workflows.first()
        && !suite.joins(Language::Dandori)
    {
        let name = &w.name.text;
        take(w.span, tr!("この sekisho は dandori のフローを読めません。このファイルはワークフロー `{name}` のフローを読みます", "this sekisho cannot read dandori's flows, and the file reads the flow of the workflow `{name}`"));
    }
    let (span, said) = first?;
    let cmd = crate::run::with_ritsu();
    Some(Diag::error("E209", &f.path, span.line, span.col, said).source(&f.src).note(tr!(
        "sekisho 単独のバイナリには、ほかの言語が入っていません。同じコマンドを、すべての言語をつないだ ritsu で、`{cmd}` のように走らせてください。",
        "The binary of sekisho's own crate holds no other language; run the same command with every language joined, through ritsu: `{cmd}`."
    )))
}

/// Check the names of a file, with the languages `suite` joins.
pub fn check(file: File, suite: &Suite) -> Named {
    let mut reading = vec![normal(&file.path)];
    check_reading(file, suite, &mut reading)
}

/// A path as one file is told from another: what the file system says, or as written.
fn normal(p: &str) -> PathBuf {
    ritsu_base::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p))
}

fn check_reading(file: File, suite: &Suite, reading: &mut Vec<PathBuf>) -> Named {
    if let Some(d) = unjoined(&file, suite) {
        return Named { diags: vec![d], scope: None };
    }
    let mut diags = Vec::new();
    let mut scope = Scope { files: vec![file], rules: BTreeMap::new(), dates: BTreeMap::new() };
    read_gates(&mut scope, suite, reading, &mut diags);
    // a file read with `use gate` reads what this run cannot: that alone is said (E209), as for this file
    if diags.iter().any(|d| d.code == "E209") {
        diags.retain(|d| d.code == "E209");
        return Named { diags, scope: None };
    }
    read_facts(&mut scope, suite, &mut diags);
    let mut ck = Ck { scope: &scope, diags: Vec::new(), attrs: BTreeMap::new(), locals: BTreeMap::new() };
    ck.declarations();
    ck.references();
    diags.append(&mut ck.diags);
    crate::diag::sort(&mut diags);
    let ok = !crate::diag::has_errors(&diags);
    Named { diags, scope: ok.then_some(scope) }
}

/// The `.gate` files the file reads with `use gate`, each checked as `sekisho check` checks it, and
/// what they read: into the scope, or E201 at the `use gate` line.
fn read_gates(scope: &mut Scope, suite: &Suite, reading: &mut Vec<PathBuf>, diags: &mut Vec<Diag>) {
    let f = &scope.files[0];
    let (path, src) = (f.path.clone(), f.src.clone());
    let ours = namespace_of(f);
    let uses: Vec<Use> = f.uses.iter().filter(|u| u.kind == UseKind::Gate).cloned().collect();
    // the `use gate` line each file read came by, in the order of `scope.files[1..]`
    let mut via: Vec<Span> = Vec::new();
    for u in uses {
        let target = beside(&path, &u.path.0);
        let shown = target.display().to_string();
        let at = |msg: Text| Diag::error("E201", &path, u.span.line, u.span.col, msg).source(&src);
        let key = normal(&shown);
        if reading.contains(&key) {
            diags.push(at(tr!("`{shown}` を読むと、このファイルに戻ってきます（`use gate` が輪になっています）", "reading `{shown}` comes back to this file (the `use gate` lines go round in a circle)")).note(tr!(
                "読む向きを一つにしてください。共通の役割や型は、両方が読む別のファイルに置けます。",
                "Make them read one way only; what both need can go in a third file both read."
            )));
            continue;
        }
        if scope.files.iter().any(|g| normal(&g.path) == key) {
            continue;
        }
        let text = match ritsu_base::fs::read_to_string(&target) {
            Ok(t) => t,
            Err(e) => {
                diags.push(at(tr!("`{shown}` を読めません: {e}", "cannot read `{shown}`: {e}")));
                continue;
            }
        };
        let parsed = crate::parse::parse(&shown, &text);
        let refused = |ds: &[Diag]| -> Diag {
            let mut d = at(tr!("`{shown}` は sekisho の検査を通りません", "`{shown}` does not pass sekisho's check"));
            // the code first and the place after the message, as dandori lists what rulec says: a
            // note that started with the file would have its first letter made a capital; the
            // message after the code starts a sentence, as it does on its own line
            for x in ds.iter().filter(|x| x.is_error()).take(3) {
                let (code, place) = (x.code, x.place());
                d = d.note(tr!("{code}: {}（{place}）", "{code}: {} ({place})", x.message.ja; capitalize(&x.message.en)));
            }
            d.note(tr!("`sekisho check {shown}` で、そのファイルの誤りを全部読めます。", "`sekisho check {shown}` says everything that is wrong with it."))
        };
        let Some(g) = parsed.file else {
            diags.push(refused(&parsed.diags));
            continue;
        };
        reading.push(key);
        let named = check_reading(g, suite, reading);
        reading.pop();
        match named.scope {
            Some(s) => {
                // the types and the roles of a file read are used in this file's namespace (E210)
                let theirs = namespace_of(&s.files[0]);
                if theirs != ours {
                    diags.push(Diag::error("E210", &path, u.span.line, u.span.col, tr!(
                        "`{shown}` の名前空間 `{theirs}` は、このファイルの名前空間 `{ours}` と違います",
                        "The namespace of `{shown}`, `{theirs}`, is not this file's, `{ours}`"
                    ))
                    .source(&src)
                    .note(tr!(
                        "`use gate` で読む型と役割は、読む側の Cedar の名前空間で使います。二つのファイルに同じ `namespace` の行を書いてください（`namespace {ours}`）。`namespace` を書かないファイルの名前空間は、ファイルの別名を Pascal case にしたもの（`refunds` なら `Refunds`）です。",
                        "The types and the roles read with `use gate` are used in the Cedar namespace of the file that reads them: write the same `namespace` line in both files (`namespace {ours}`). A file with no `namespace` line is in its alias in Pascal case (`Refunds` for `refunds`)."
                    )));
                }
                for g in s.files {
                    if !scope.files.iter().any(|x| normal(&x.path) == normal(&g.path)) {
                        scope.files.push(g);
                        via.push(u.span);
                    }
                }
            }
            None => match named.diags.iter().find(|d| d.code == "E209") {
                // what the file read reads cannot be read here: how the command is run, as for this file
                Some(inner) => {
                    let (code, place) = (inner.code, inner.place());
                    let cmd = crate::run::with_ritsu();
                    diags.push(
                        Diag::error("E209", &path, u.span.line, u.span.col, tr!("この sekisho は、`{shown}` が読むものを読めません", "this sekisho cannot read what `{shown}` reads"))
                            .source(&src)
                            .note(tr!("{code}: {}（{place}）", "{code}: {} ({place})", inner.message.ja; capitalize(&inner.message.en)))
                            .note(tr!(
                                "sekisho 単独のバイナリには、ほかの言語が入っていません。同じコマンドを、すべての言語をつないだ ritsu で、`{cmd}` のように走らせてください。",
                                "The binary of sekisho's own crate holds no other language; run the same command with every language joined, through ritsu: `{cmd}`."
                            )),
                    );
                }
                None => diags.push(refused(&named.diags)),
            },
        }
    }
    same_in_two_files(scope, &via, &path, &src, diags);
}

/// What two files read with `use gate` both declare (E211): a type or an enum, a role, a workflow,
/// by its name or its alias. They would be two things of one name in one Cedar namespace. Said at
/// the `use gate` line the later file came by.
fn same_in_two_files(scope: &Scope, via: &[Span], path: &str, src: &str, diags: &mut Vec<Diag>) {
    // one space for the types and the enums, one for the roles, one for the workflows: each name
    // and alias, with the file and the name of what declares it
    let mut spaces: [BTreeMap<String, (String, String)>; 3] = Default::default();
    for (k, g) in scope.files[1..].iter().enumerate() {
        let decls: [(usize, Text, Vec<&Name>); 3] = [
            (0, tr!("型", "type"), g.enums.iter().map(|e| &e.name).chain(g.principals.iter().chain(&g.resources).map(|e| &e.name)).collect()),
            (1, tr!("役割", "role"), g.roles.iter().map(|r| &r.name).collect()),
            (2, tr!("ワークフロー", "workflow"), g.workflows.iter().map(|w| &w.name).collect()),
        ];
        for (space, noun, names) in decls {
            for n in names {
                let mut keys = vec![n.text.clone()];
                if let Some((a, _)) = &n.alias
                    && *a != n.text
                {
                    keys.push(a.clone());
                }
                let other = keys.iter().find_map(|key| spaces[space].get(key).filter(|(file, _)| *file != g.path).cloned());
                if let Some((file, theirs)) = other {
                    let at = via.get(k).copied().unwrap_or_default();
                    let (this, name) = (&g.path, &n.text);
                    let (nj, ne) = (&noun.ja, &noun.en);
                    let mut d = Diag::error("E211", path, at.line, at.col, tr!("`{this}` と `{file}` が、どちらも{nj} `{name}` を宣言しています", "`{this}` and `{file}` both declare the {ne} `{name}`")).source(src);
                    if theirs != *name {
                        d = d.note(tr!("`{file}` では `{theirs}` という名前で、名前か別名が同じです。", "`{file}` declares it as `{theirs}`, of the same name or alias."));
                    }
                    diags.push(d.note(tr!(
                        "`use gate` で読んだものは一つの Cedar の名前空間に並ぶので、同じ名前のものは一つのファイルにだけ宣言し、ほかのファイルはそのファイルを `use gate` で読んでください。",
                        "What is read with `use gate` stands in one Cedar namespace: declare a thing of one name in one file, and have the other files read that one with `use gate`."
                    )));
                    continue;
                }
                for key in keys {
                    spaces[space].entry(key).or_insert_with(|| (g.path.clone(), n.text.clone()));
                }
            }
        }
    }
}

/// What rulec says of each `use rule`, and koyomi of each `use dates`: into the scope, or E201.
fn read_facts(scope: &mut Scope, suite: &Suite, diags: &mut Vec<Diag>) {
    let f = &scope.files[0];
    let (path, src) = (f.path.clone(), f.src.clone());
    let refused = |u: &Use, shown: &str, tool: &str, said: &[Said]| -> Diag {
        let mut d = Diag::error("E201", &path, u.path.1.line, u.path.1.col, tr!("`{shown}` は {tool} の検査を通りません", "`{shown}` does not pass {tool}'s check")).source(&src);
        // each with its code first and its place after it, as for a gate read (`read_gates`)
        for s in said.iter().take(3) {
            let at = match s.line {
                Some(l) => format!("{}:{l}", s.file),
                None => s.file.clone(),
            };
            let (ja, en) = (&s.message.ja, capitalize(&s.message.en));
            d = d.note(match (s.code.as_str(), at.is_empty()) {
                ("", true) => s.message.clone(),
                ("", false) => tr!("{ja}（{at}）", "{en} ({at})"),
                (code, true) => tr!("{code}: {ja}", "{code}: {en}"),
                (code, false) => tr!("{code}: {ja}（{at}）", "{code}: {en} ({at})"),
            });
        }
        d.note(tr!("そのファイルを、{tool} の検査が通るように直してください。", "Correct that file until {tool}'s check passes it."))
    };
    for u in f.uses.clone() {
        let Some((name, _)) = &u.name else { continue };
        let target = beside(&path, &u.path.0);
        let shown = target.display().to_string();
        match u.kind {
            UseKind::Rule => {
                let Some(rules) = &suite.rules else { continue };
                match rules.facts(&target) {
                    Ok(facts) => {
                        scope.rules.entry(name.clone()).or_insert(Read { path: target, facts });
                    }
                    Err(said) => diags.push(refused(&u, &shown, "rulec", &said)),
                }
            }
            UseKind::Dates => {
                let Some(dates) = &suite.dates else { continue };
                match dates.facts(&target) {
                    Ok(facts) => {
                        scope.dates.entry(name.clone()).or_insert(Read { path: target, facts });
                    }
                    Err(said) => diags.push(refused(&u, &shown, "koyomi", &said)),
                }
            }
            _ => {}
        }
    }
}

/// A value's place: the type or the action it belongs to, and its name, as Cedar calls them.
type Key = (String, String);

/// A number's range on the wire, both ends in; None for what is not a number.
type Ends = Option<(i128, i128)>;

/// The checks of the names, over a scope.
struct Ck<'a> {
    scope: &'a Scope,
    diags: Vec<Diag>,
    /// The type of each attribute, by (type, attribute) as Cedar calls them, and its range.
    attrs: BTreeMap<Key, (Ty, Ends)>,
    /// The type of each input and computed value, by (action, name) as Cedar calls them; its range
    /// for a number; whether it is computed.
    locals: BTreeMap<Key, (Ty, Ends, bool)>,
}

/// What a body's condition can read: the actions it is about, the principal types it picks, and
/// the resource types.
struct Env<'s> {
    actions: Vec<&'s ActionDecl>,
    principals: Vec<String>,
    resources: Vec<String>,
}

impl<'a> Ck<'a> {
    fn file(&self) -> &'a File {
        &self.scope.files[0]
    }

    /// A type as a message says it, by the names the file writes (`注文の状態`, not `order_status`).
    fn ty_text(&self, ty: &Ty) -> Text {
        match ty {
            Ty::Enum(e) => {
                let n = self.scope.enum_decl(e).map(|d| d.name.text.clone()).unwrap_or_else(|| e.clone());
                tr!("列挙 `{n}`", "the enum `{n}`")
            }
            Ty::Entity(t) => {
                let n = self.type_name(t);
                tr!("型 `{n}`", "the type `{n}`")
            }
            other => other.text(),
        }
    }

    /// A principal's or a resource's type as the file writes it (`職員`, not `User`).
    fn type_name(&self, ascii: &str) -> String {
        match self.scope.entity(ascii) {
            Some(EntityRef::Declared(e)) => e.name.text.clone(),
            _ => ascii.to_string(),
        }
    }

    /// Types as the file writes them, listed: `` `職員`、`顧客` ``.
    fn type_names(&self, ascii: &[String]) -> Text {
        let names: Vec<Text> = ascii.iter().map(|a| Text::same(format!("`{}`", self.type_name(a)))).collect();
        Text::list(&names)
    }

    fn err(&mut self, code: &'static str, span: Span, msg: Text) -> &mut Diag {
        let f = self.file();
        self.diags.push(Diag::error(code, &f.path, span.line, span.col, msg).source(&f.src));
        self.diags.last_mut().unwrap()
    }

    // ── Declared names ───────────────────────────────────────────────────────

    /// Every name the file declares: E002, E006, E007, E008.
    fn declarations(&mut self) {
        let f = self.file();
        self.name(&f.name, Kind::File);
        self.namespace();
        // the names of the `use` lines
        let mut uses: BTreeMap<String, Span> = BTreeMap::new();
        let mut gates: BTreeMap<String, Span> = BTreeMap::new();
        for u in &f.uses {
            match &u.name {
                Some((n, s)) => {
                    if kw::is_reserved(n) {
                        self.reserved(n, *s);
                    }
                    if let Some(first) = uses.get(n) {
                        let l = first.line;
                        self.err("E006", *s, tr!("`{n}` は {l} 行目の `use` でも使っている名前です", "`{n}` is the name of the `use` on line {l} too")).notes.push(tr!("`use` の名前は、読むものごとに別にしてください。", "Give each `use` a name of its own."));
                    } else {
                        uses.insert(n.clone(), *s);
                    }
                }
                None => {
                    if let Some(first) = gates.get(&u.path.0) {
                        let (p, l) = (u.path.0.clone(), first.line);
                        self.err("E006", u.span, tr!("`{p}` は {l} 行目でも読んでいます", "`{p}` is read on line {l} too"));
                    } else {
                        gates.insert(u.path.0.clone(), u.span);
                    }
                }
            }
        }
        // the types and the enums share a namespace: a field's type names either; so do the
        // declarations of the files read with `use gate`
        let mut types = Space::default();
        let mut roles = Space::default();
        let mut workflows = Space::default();
        for g in &self.scope.files[1..] {
            for e in &g.enums {
                types.insert_from(&e.name, &g.path);
            }
            for e in g.principals.iter().chain(&g.resources) {
                types.insert_from(&e.name, &g.path);
            }
            for r in &g.roles {
                roles.insert_from(&r.name, &g.path);
            }
            for w in &g.workflows {
                workflows.insert_from(&w.name, &g.path);
            }
        }
        for e in &f.enums {
            self.name(&e.name, Kind::Enum);
            self.unique(&mut types, &e.name, Kind::Enum);
            let mut values = Space::default();
            for v in &e.values {
                self.name(v, Kind::Value);
                self.unique(&mut values, v, Kind::Value);
            }
        }
        for r in &f.roles {
            self.name(&r.name, Kind::Role);
            self.unique(&mut roles, &r.name, Kind::Role);
        }
        for e in f.principals.iter().chain(&f.resources) {
            self.name(&e.name, Kind::Type);
            self.unique(&mut types, &e.name, Kind::Type);
            let mut attrs = Space::default();
            for a in &e.attributes {
                self.name(&a.name, Kind::Attribute);
                self.unique(&mut attrs, &a.name, Kind::Attribute);
            }
        }
        for w in &f.workflows {
            self.name(&w.name, Kind::Workflow);
            self.unique(&mut workflows, &w.name, Kind::Workflow);
        }
        let mut actions = Space::default();
        for a in &f.actions {
            self.name(&a.name, Kind::Action);
            self.unique(&mut actions, &a.name, Kind::Action);
            let mut locals = Space::default();
            for i in &a.input {
                self.name(&i.name, Kind::Input);
                self.unique(&mut locals, &i.name, Kind::Input);
            }
            for c in &a.context {
                self.name(&c.name, Kind::Computed);
                self.unique(&mut locals, &c.name, Kind::Computed);
            }
        }
        let mut policies = Space::default();
        for p in &f.policies {
            self.name(&p.name, Kind::Policy);
            self.unique(&mut policies, &p.name, Kind::Policy);
        }
        let mut expects = Space::default();
        for e in &f.expects {
            self.name(&e.name, Kind::Expect);
            self.unique(&mut expects, &e.name, Kind::Expect);
        }
        let mut separates = Space::default();
        for s in &f.separates {
            self.name(&s.name, Kind::Separate);
            self.unique(&mut separates, &s.name, Kind::Separate);
        }
    }

    fn reserved(&mut self, w: &str, s: Span) {
        let list = kw::RESERVED.join("、");
        let list_en = kw::RESERVED.join(", ");
        self.err("E002", s, tr!("`{w}` は sekisho のキーワードなので、名前にも別名にもできません", "`{w}` is a keyword of sekisho, and can be neither a name nor an alias")).notes.push(tr!(
            "条件と計算と型を書く語は、名前にできません（{list}）。条件が二通りに読めてしまうからです。別の名前にしてください。",
            "The words conditions, computed values and types are written with cannot be names ({list_en}): a condition could be read two ways. Choose another name."
        ));
    }

    /// One declared name: E002, E007, E008.
    fn name(&mut self, n: &Name, kind: Kind) {
        if n.text.is_empty() {
            return;
        }
        if kw::is_reserved(&n.text) {
            self.reserved(&n.text, n.span);
            return;
        }
        if let Some((a, s)) = &n.alias
            && kw::is_reserved(a)
        {
            self.reserved(a, *s);
            return;
        }
        if !kind.to_cedar() {
            return;
        }
        let noun = kind.noun();
        let upper = kind == Kind::Type;
        let (form, form_en) = if upper { ("`[A-Z][A-Za-z0-9]*`（大文字で始まり、英字と数字だけ）", "`[A-Z][A-Za-z0-9]*` (a capital, then letters and digits)") } else { ("`[a-z][a-z0-9_]*`（小文字で始まり、小文字・数字・`_` だけ）", "`[a-z][a-z0-9_]*` (a lower-case letter, then lower-case letters, digits and `_`)") };
        let fits = |s: &str| if upper { is_upper(s) } else { is_lower(s) };
        match &n.alias {
            Some((a, s)) => {
                if !fits(a) {
                    let an = kind.a();
                    self.err("E007", *s, tr!("{}の別名 `{a}` の形が違います", "The alias `{a}` of the {} is not of the form an alias takes", noun.ja; noun.en)).notes.push(tr!("{}の別名は {form}の形です。", "The alias of {an} is {form_en}.", noun.ja;));
                    return;
                }
            }
            None => {
                if !fits(&n.text) {
                    let t = &n.text;
                    if t.is_ascii() {
                        let s = if upper { ritsu_emit::ident::pascal(&suggest_lower(t)) } else { suggest_lower(t) };
                        self.err("E007", n.span, tr!("{}の名前 `{t}` は、Cedar に出る名前の形ではありません", "The {} name `{t}` is not of the form a name in Cedar takes", noun.ja; noun.en))
                            .notes
                            .push(tr!("{}の名前は {form}の形で書いてください。たとえば `{s}` です。", "The name of {} is of the form {form_en}: `{s}`, say.", noun.ja; kind.a()));
                    } else {
                        self.err("E007", n.span, tr!("{}の名前 `{t}` には ASCII の別名が要ります", "The {} name `{t}` needs an ASCII alias", noun.ja; noun.en)).notes.push(tr!(
                            "Cedar の名前は ASCII だけなので、`{t}(<別名>)` のように丸括弧で別名を付けてください。{}の別名は {form}の形です。",
                            "A name in Cedar is ASCII only: give it an alias in parentheses, like `{t}(<alias>)`. The alias of {} is {form_en}.",
                            noun.ja;
                            kind.a()
                        ));
                    }
                    return;
                }
            }
        }
        let (a, s) = match &n.alias {
            Some((a, s)) => (a.clone(), *s),
            None => (n.text.clone(), n.span),
        };
        if kw::CEDAR_RESERVED.contains(&a.as_str()) {
            self.err("E008", s, tr!("`{a}` は Cedar の予約語なので、Cedar の名前にできません", "`{a}` is a reserved word of Cedar, and cannot be a name in Cedar")).notes.push(tr!("別の名前か別名にしてください。", "Choose another name or alias."));
            return;
        }
        if upper && kw::CEDAR_TYPES.contains(&a.as_str()) {
            let note = if a == ROLE_TYPE || a == WORKFLOW_TYPE {
                tr!("`Role` と `Workflow` は、sekisho が役割とワークフローのために宣言する型です。", "`Role` and `Workflow` are the types sekisho declares for the roles and the workflows.")
            } else if a == "Action" {
                tr!("`Action` は Cedar が action のために持つ型で、スキーマに宣言できません。", "`Action` is Cedar's type of the actions, which a schema cannot declare.")
            } else {
                tr!("Cedar の組み込みの型（`Bool`・`Long`・`String`）と、JSON のスキーマが型に使う語（`Boolean`・`Entity`・`Extension`・`Record`・`Set`）は、型の名前にしません。", "Cedar's builtin types (`Bool`, `Long`, `String`) and the words its JSON schema writes types with (`Boolean`, `Entity`, `Extension`, `Record`, `Set`) are not taken as a type's name.")
            };
            self.err("E008", s, tr!("型の名前 `{a}` は、Cedar か sekisho がもう使っています", "The type name `{a}` is one Cedar or sekisho uses already")).notes.push(note);
            return;
        }
        let refusing = targets_refusing(&a);
        if !refusing.is_empty() {
            let ts = refusing.join("、");
            let ts_en = refusing.join(", ");
            self.err("E008", s, tr!("`{a}` は {ts} の予約語なので、生成するコードの名前にできません", "`{a}` is a reserved word of {ts_en}, and cannot be a name in the generated code")).notes.push(tr!("別の名前か別名にしてください。", "Choose another name or alias."));
        }
    }

    /// One declared name in its namespace: E006 when its name or alias is already there.
    fn unique(&mut self, space: &mut Space, n: &Name, kind: Kind) {
        if n.text.is_empty() {
            return;
        }
        let noun = kind.noun();
        let mut keys = vec![(n.text.clone(), n.span, false)];
        if let Some((a, s)) = &n.alias
            && *a != n.text
        {
            keys.push((a.clone(), *s, true));
        }
        for (k, s, alias) in &keys {
            if let Some((line, from, of)) = space.find(k) {
                let where_ja = match &from {
                    Some(p) => format!("`{p}` の {line} 行目"),
                    None => format!("{line} 行目"),
                };
                let where_en = match &from {
                    Some(p) => format!("line {line} of `{p}`"),
                    None => format!("line {line}"),
                };
                let t = &n.text;
                let msg = match (*alias, of == *k) {
                    (false, true) => tr!("{} `{k}` は、{where_ja}でも宣言されています", "The {} `{k}` is declared on {where_en} too", noun.ja; noun.en),
                    (false, false) => tr!("{} `{k}` は、{where_ja}の{} `{of}` の別名と同じです", "The {} `{k}` is the alias of the {} `{of}` on {where_en}", noun.ja, noun.ja; noun.en, noun.en),
                    (true, true) => tr!("{} `{t}` の別名 `{k}` は、{where_ja}で宣言した{}の名前と同じです", "The alias `{k}` of the {} `{t}` is the name of the {} declared on {where_en}", noun.ja, noun.ja; noun.en, noun.en),
                    (true, false) => tr!("{} `{t}` の別名 `{k}` は、{where_ja}の{} `{of}` の別名と同じです", "The alias `{k}` of the {} `{t}` is the alias of the {} `{of}` on {where_en} too", noun.ja, noun.ja; noun.en, noun.en),
                };
                self.err("E006", *s, msg).notes.push(tr!("名前と別名は、同じ種類のもののあいだで重ならないようにしてください。", "Names and aliases are each declared once among the things of one kind."));
                return;
            }
        }
        space.insert(n, None);
    }

    fn namespace(&mut self) {
        let Some((segs, s)) = &self.file().namespace else { return };
        for seg in segs {
            let ok = ritsu_emit::ident::is_ascii_ident(seg) && !seg.starts_with("__");
            if !ok {
                self.err("E007", *s, tr!("名前空間 `{seg}` は ASCII の識別子ではありません", "The namespace `{seg}` is not an ASCII identifier")).notes.push(tr!(
                    "名前空間は Cedar の名前なので、`Shop`、`Acme::Shop` のように ASCII で書いてください。",
                    "A namespace is a name in Cedar: write it in ASCII, like `Shop` or `Acme::Shop`."
                ));
                return;
            }
            if kw::CEDAR_RESERVED.contains(&seg.as_str()) {
                self.err("E008", *s, tr!("`{seg}` は Cedar の予約語なので、名前空間にできません", "`{seg}` is a reserved word of Cedar, and cannot be a namespace"));
                return;
            }
        }
    }

    // ── References and types ─────────────────────────────────────────────────

    fn references(&mut self) {
        let f = self.file();
        self.today_line();
        for e in &f.principals {
            for r in &e.roles {
                self.role_ref(r);
            }
        }
        self.roles();
        for e in f.principals.iter().chain(&f.resources) {
            let t = e.name.ascii().to_string();
            for a in &e.attributes {
                if let Some((ty, range)) = self.field(a, false) {
                    self.attrs.insert((t.clone(), a.name.ascii().to_string()), (ty, range));
                }
            }
        }
        // the attributes of the types of the files read with `use gate`, as they declare them
        for g in &self.scope.files[1..] {
            for e in g.principals.iter().chain(&g.resources) {
                for a in &e.attributes {
                    let ft = self.scope.field_type(a);
                    let (ty, range) = match ft {
                        FieldType::Bool => (Ty::Bool, None),
                        FieldType::Date { .. } => (Ty::Date, None),
                        FieldType::Number { unit, lo, hi } => (Ty::Num(unit), Some((lo, hi))),
                        FieldType::Enum(e) => (Ty::Enum(e), None),
                        FieldType::Entity(t) => (Ty::Entity(t), None),
                    };
                    self.attrs.insert((e.name.ascii().to_string(), a.name.ascii().to_string()), (ty, range));
                }
            }
        }
        for a in &f.actions {
            self.action(a);
        }
        for p in &f.policies {
            self.body(&p.body, Some(p.effect), false);
        }
        for e in &f.expects {
            self.body(&e.body, None, false);
        }
        for s in &f.separates {
            let mut seen: Vec<String> = Vec::new();
            for r in &s.actions.0 {
                if let Some(a) = self.action_ref(r) {
                    let k = a.name.ascii().to_string();
                    if seen.contains(&k) {
                        let w = &r.word;
                        self.err("E006", r.span, tr!("action `{w}` が二度並んでいます", "The action `{w}` is listed twice"));
                    }
                    seen.push(k);
                }
            }
        }
        // the forbids of the files read with `use gate` that hold for every action hold for these
        let imported: Vec<(&File, &Policy)> = self.scope.imported_forbids();
        for (g, p) in imported {
            let before = self.diags.len();
            self.body(&p.body, Some(p.effect), true);
            // what they say is said at their own lines, in their own file
            for d in &mut self.diags[before..] {
                d.file = g.path.clone();
                d.rel = g.path.clone();
                d.src = d.line.and_then(|l| g.src.lines().nth(l - 1)).map(|s| s.trim_end().to_string());
                d.notes.push(tr!(
                    "このポリシーは `{}` の forbid で、すべての action に効くので、読む側の `{}` の action にも効きます。",
                    "This is a forbid of `{}` that holds for every action, so it holds for the actions of `{}` that reads it too.",
                    g.path,
                    self.scope.files[0].path
                ));
            }
        }
    }

    /// `today`: both ends dates, in order, and the offset (E102, E103, E107).
    fn today_line(&mut self) {
        let Some(t) = &self.file().today else { return };
        let mut ends = Vec::new();
        for (e, word) in [(&t.range.lo, ">="), (&t.range.hi, "<=")] {
            match e {
                Some((Lit::Date(d), _)) => ends.push(*d),
                Some((l, s)) => {
                    let shown = lit_text(l);
                    self.err("E102", *s, tr!("`today` の範囲の端 `{shown}` は日付ではありません", "The end `{shown}` of the range of `today` is not a date"));
                }
                None => {
                    self.err("E103", t.range.span, tr!("`today` の範囲に `{word}` の端がありません", "The range of `today` has no `{word}` end")).notes.push(tr!(
                        "検査は範囲のすべての日を数えるので、両端が要ります: `today range >=2026-10-01 <=2028-10-31 offset +00:00`。",
                        "The check walks every day of the range, so it needs both ends: `today range >=2026-10-01 <=2028-10-31 offset +00:00`."
                    ));
                }
            }
        }
        if let [lo, hi] = ends[..]
            && lo > hi
        {
            let (a, b) = (types::day_text(lo), types::day_text(hi));
            self.err("E103", t.range.span, tr!("`today` の範囲 {a}..{b} に入る日はありません", "No day is in the range {a}..{b} of `today`"));
        }
        if let Err((msg, notes)) = offset(&t.offset.0) {
            let d = self.err("E107", t.offset.1, msg);
            d.notes.extend(notes);
        }
    }

    /// The roles: `includes` names roles, `can` names actions of the file, and no role includes
    /// itself (E005, E101).
    fn roles(&mut self) {
        let f = self.file();
        for r in &f.roles {
            for i in &r.includes {
                self.role_ref(i);
            }
            if let Some((cans, _)) = &r.can {
                for c in cans {
                    self.action_ref(c);
                }
            }
        }
        // a circle of `includes` is said once, at the `includes` of its first role in the file that
        // starts it, with the roles of the circle in order (E108)
        let mut said: BTreeSet<String> = BTreeSet::new();
        for r in &f.roles {
            if said.contains(r.name.ascii()) {
                continue;
            }
            let Some((at, circle)) = self.circle(r) else { continue };
            said.extend(circle.iter().map(|d| d.name.ascii().to_string()));
            let mut names: Vec<&str> = circle.iter().map(|d| d.name.text.as_str()).collect();
            names.push(&r.name.text);
            let shown = names.join(" → ");
            self.err("E108", at, tr!("役割の `includes` が輪になっています: {shown}", "The roles' `includes` go round in a circle: {shown}")).notes.push(tr!(
                "`includes` は「この役割を持つ人は、その役割も持つ」です。輪になると、どの役割がどれを含むかが決まらず、Cedar の役割の親子も輪になります。輪のどこかの `includes` を消してください。",
                "`includes` says whoever holds this role holds that one too; in a circle, which role includes which is not decided, and the parents of Cedar's roles would go round too. Remove an `includes` of the circle."
            ));
        }
    }

    /// The circle of `includes` a role is in, when it is in one: the `includes` of the role that
    /// starts it, and the roles of the circle from the role on (the shortest circle).
    fn circle(&self, start: &'a RoleDecl) -> Option<(Span, Vec<&'a RoleDecl>)> {
        let scope = self.scope;
        let me = start.name.ascii().to_string();
        // each role reached, the role it was reached from, and the `includes` of `start` it came by
        let mut prev: BTreeMap<String, String> = BTreeMap::new();
        let mut by: BTreeMap<String, Span> = BTreeMap::new();
        let mut queue: std::collections::VecDeque<String> = std::collections::VecDeque::new();
        for inc in &start.includes {
            let Some(d) = scope.role(&inc.word) else { continue };
            let k = d.name.ascii().to_string();
            if k == me {
                return Some((inc.span, vec![start]));
            }
            if !prev.contains_key(&k) {
                prev.insert(k.clone(), me.clone());
                by.insert(k.clone(), inc.span);
                queue.push_back(k);
            }
        }
        while let Some(k) = queue.pop_front() {
            let Some(d) = scope.role(&k) else { continue };
            for inc in &d.includes {
                let Some(n) = scope.role(&inc.word) else { continue };
                let nk = n.name.ascii().to_string();
                if nk == me {
                    // back along the way it was reached, to the role `start` includes
                    let mut chain = vec![k.clone()];
                    while let Some(p) = prev.get(chain.last().unwrap()) {
                        if *p == me {
                            break;
                        }
                        chain.push(p.clone());
                    }
                    chain.reverse();
                    let at = by.get(&chain[0]).copied().unwrap_or(start.span);
                    let mut roles = vec![start];
                    roles.extend(chain.iter().filter_map(|c| scope.role(c)));
                    return Some((at, roles));
                }
                if !prev.contains_key(&nk) {
                    prev.insert(nk.clone(), k.clone());
                    by.insert(nk.clone(), by.get(&k).copied().unwrap_or(start.span));
                    queue.push_back(nk);
                }
            }
        }
        None
    }

    fn role_ref(&mut self, r: &Ref) -> Option<&'a RoleDecl> {
        let scope = self.scope;
        match scope.role(&r.word) {
            Some(d) => Some(d),
            None => {
                let known = names_of(scope.files.iter().flat_map(|f| f.roles.iter().map(|r| &r.name)));
                let w = &r.word;
                self.err("E101", r.span, tr!("役割 `{w}` はありません", "There is no role `{w}`")).notes.push(known_note(&tr!("役割", "roles"), &known));
                None
            }
        }
    }

    /// An action of the file (a policy names only the file's own actions, DESIGN 2.7).
    fn action_ref(&mut self, r: &Ref) -> Option<&'a ActionDecl> {
        let f = self.file();
        match f.action(&r.word) {
            Some(a) => Some(a),
            None => {
                let w = &r.word;
                let other = self.scope.files[1..].iter().find(|g| g.action(w).is_some()).map(|g| g.path.clone());
                let known = names_of(f.actions.iter().map(|a| &a.name));
                let d = self.err("E101", r.span, tr!("action `{w}` はありません", "There is no action `{w}`"));
                if let Some(p) = other {
                    d.notes.push(tr!("`{p}` の action です。ポリシーと期待が書けるのは、このファイルの action だけです。", "It is an action of `{p}`; a policy or an expectation names only the actions of its own file."));
                } else {
                    d.notes.push(known_note(&tr!("action", "actions"), &known));
                }
                None
            }
        }
    }

    /// A principal's or a resource's type; `principal` says which kind is wanted.
    fn type_ref(&mut self, r: &Ref, principal: bool) -> Option<EntityRef<'a>> {
        let scope = self.scope;
        let w = &r.word;
        match scope.entity(w) {
            Some(EntityRef::Workflow) if principal => {
                if scope.files.iter().all(|f| f.workflows.is_empty()) {
                    self.err("E101", r.span, tr!("`Workflow` を書くには、`workflow` の行が要ります", "`Workflow` takes a `workflow` line")).notes.push(tr!(
                        "`Workflow` は、`workflow <名前> from \"<.flow>\"` で宣言したワークフローの型です。",
                        "`Workflow` is the type of the workflows declared with `workflow <name> from \"<.flow>\"`."
                    ));
                    return None;
                }
                Some(EntityRef::Workflow)
            }
            Some(EntityRef::Declared(e)) if (e.kind == EntityKind::Principal) == principal => Some(EntityRef::Declared(e)),
            Some(_) => {
                let (want, want_en, is, is_en) = if principal { ("principal の型", "a principal's type", "resource の型", "a resource's type") } else { ("resource の型", "a resource's type", "principal の型", "a principal's type") };
                self.err("E101", r.span, tr!("`{w}` は{is}です。ここには{want}を書いてください", "`{w}` is {is_en}; {want_en} goes here"));
                None
            }
            None => {
                let known = if principal {
                    let mut k = names_of(scope.files.iter().flat_map(|f| f.principals.iter().map(|e| &e.name)));
                    if scope.files.iter().any(|f| !f.workflows.is_empty()) {
                        k.push(WORKFLOW_TYPE.to_string());
                    }
                    k
                } else {
                    names_of(scope.files.iter().flat_map(|f| f.resources.iter().map(|e| &e.name)))
                };
                let what = if principal { tr!("principal の型", "principal types") } else { tr!("resource の型", "resource types") };
                self.err("E101", r.span, tr!("型 `{w}` はありません", "There is no type `{w}`")).notes.push(known_note(&what, &known));
                None
            }
        }
    }

    /// An attribute or an input: its type and its range (E101, E102, E103).
    fn field(&mut self, f: &Field, input: bool) -> Option<(Ty, Option<(i128, i128)>)> {
        let scope = self.scope;
        let n = &f.name.text;
        let no_range = |me: &mut Self, what: Text| {
            if let Some(r) = &f.range {
                me.err("E102", r.span, tr!("`{n}` は{}なので、範囲を書けません", "`{n}` is {}, and takes no range", what.ja; what.en)).notes.push(tr!("範囲を書けるのは、数と日付だけです。", "Only a number and a date take a range."));
            }
        };
        match &f.ty {
            Type::Bool => {
                no_range(self, tr!("真偽", "true or false"));
                Some((Ty::Bool, None))
            }
            Type::Date => {
                let (lo, hi) = self.ends(f, |me, l, s| match l {
                    Lit::Date(d) => Some(*d as i128),
                    other => {
                        let shown = lit_text(other);
                        me.err("E102", s, tr!("`{n}` は日付なので、範囲の端 `{shown}` も日付で書いてください", "`{n}` is a date, so the end `{shown}` is a date too"));
                        None
                    }
                })?;
                let _ = (lo, hi);
                Some((Ty::Date, None))
            }
            Type::Unit(u) => {
                let unit = match types::unit(u) {
                    Ok(unit) => unit,
                    Err(p) => {
                        let code = types::unit_code(&p);
                        self.err(code, f.ty_span, p.text()).notes.push(tr!(
                            "数の型は rulec と同じく、`money[GBP, incl_tax]`、`mass[kg]`、`rate[step 0.1%]`、`number` のように書きます。",
                            "A number's type is written as rulec writes it: `money[GBP, incl_tax]`, `mass[kg]`, `rate[step 0.1%]`, `number`."
                        ));
                        return None;
                    }
                };
                let (lo, hi) = self.ends(f, |me, l, s| match l {
                    Lit::Num(num) => match types::count(num, &unit) {
                        Ok(v) => Some(v),
                        Err(m) => {
                            me.miss(&m, s, num, &unit, n);
                            None
                        }
                    },
                    other => {
                        let shown = lit_text(other);
                        me.err("E102", s, tr!("`{n}` は {unit} の数なので、範囲の端 `{shown}` も数で書いてください", "`{n}` is a number of {unit}, so the end `{shown}` is a number too"));
                        None
                    }
                })?;
                Some((Ty::Num(unit), Some((lo, hi))))
            }
            Type::Named(r) => {
                let w = &r.word;
                if let Some(e) = scope.enum_decl(w) {
                    no_range(self, tr!("列挙", "an enum"));
                    return Some((Ty::Enum(e.name.ascii().to_string()), None));
                }
                match scope.entity(w) {
                    Some(EntityRef::Declared(e)) if !input => {
                        no_range(self, tr!("エンティティ", "an entity"));
                        Some((Ty::Entity(e.name.ascii().to_string()), None))
                    }
                    Some(_) if input => {
                        self.err("E102", r.span, tr!("入力 `{n}` の型が `{w}` です。入力に書けるのは、真偽、列挙、数、日付だけです", "The input `{n}` is of the type `{w}`; an input is true or false, an enum, a number or a date")).notes.push(tr!(
                            "操作が扱うエンティティは、`resource` の行で書きます（`resource Order from orderId`）。",
                            "The entity an operation acts on is written on the `resource` line (`resource Order from orderId`)."
                        ));
                        None
                    }
                    Some(_) => {
                        self.err("E102", r.span, tr!("`Workflow` は属性の型にできません", "`Workflow` is not the type of an attribute"));
                        None
                    }
                    None => {
                        let dim = [kw::MONEY, "mass", "length", "area", "volume", "duration", "temperature", "sound"].contains(&w.as_str());
                        let d = self.err("E101", r.span, tr!("型 `{w}` はありません", "There is no type `{w}`"));
                        if ["string", "String", "str", "text"].contains(&w.as_str()) {
                            d.notes.push(tr!(
                                "自由な文字列は、属性にも入力にもできません。比べる相手が限りなくあり、全部の組み合わせを数えられないからです。値が決まっているなら列挙（`enum`）にしてください。",
                                "A free string is neither an attribute nor an input: what it compares with has no end, and the combinations could not be counted. Where its values are known, make it an enum (`enum`)."
                            ));
                        } else if w == "Bool" || w == "Long" || w == "Boolean" {
                            let s = if w == "Long" { "number" } else { "bool" };
                            d.notes.push(tr!("Cedar の型ではなく、sekisho の型で書いてください（`{s}`。数なら `money[GBP, incl_tax]` のように単位も）。", "Write sekisho's type rather than Cedar's: `{s}` (for a number, with its unit, like `money[GBP, incl_tax]`)."));
                        } else if dim {
                            d.notes.push(tr!("単位の型は `{w}[…]` のように、単位を `[…]` の中に書きます（`money[GBP, incl_tax]`）。", "A unit's type has its unit in `[…]`: `{w}[…]` (`money[GBP, incl_tax]`)."));
                        } else {
                            d.notes.push(tr!(
                                "型に書けるのは、`bool`、`date`、数の型（`money[…]` など）、このファイルか `use gate` で読んだファイルの列挙と、principal と resource の型です。",
                                "A type is `bool`, `date`, a number's type (`money[…]` and the like), or an enum, a principal's or a resource's type of the file or of one it reads with `use gate`."
                            ));
                        }
                        None
                    }
                }
            }
        }
    }

    /// Both ends of a field's range, each read by `read` (E103 when the range or an end is missing,
    /// or when no value is in it).
    fn ends(&mut self, f: &Field, mut read: impl FnMut(&mut Self, &Lit, Span) -> Option<i128>) -> Option<(i128, i128)> {
        let n = &f.name.text;
        let Some(r) = &f.range else {
            self.err("E103", f.ty_span, tr!("`{n}` に範囲がありません", "`{n}` has no range")).notes.push(tr!(
                "検査は値を範囲の中で数えるので、数と日付には範囲が要ります（`range >=1GBP <=10_000GBP`、`range >=2026-01-01 <=2028-09-30`）。生成したコードは、範囲の外の値を Cedar に渡す前に拒みます。",
                "The check counts values within their range, so a number and a date take one (`range >=1GBP <=10_000GBP`, `range >=2026-01-01 <=2028-09-30`); the generated code refuses a value outside it before asking Cedar."
            ));
            return None;
        };
        let mut out = [None, None];
        let mut bad = false;
        for (k, (e, word)) in [(&r.lo, ">="), (&r.hi, "<=")].into_iter().enumerate() {
            match e {
                Some((l, s)) => match read(self, l, *s) {
                    Some(v) => out[k] = Some(v),
                    None => bad = true,
                },
                None => {
                    self.err("E103", r.span, tr!("`{n}` の範囲に `{word}` の端がありません", "The range of `{n}` has no `{word}` end")).notes.push(tr!("範囲には両端を書いてください。", "Write both ends of the range."));
                    bad = true;
                }
            }
        }
        if bad {
            return None;
        }
        let (lo, hi) = (out[0]?, out[1]?);
        if lo > hi {
            self.err("E103", r.span, tr!("`{n}` の範囲に入る値はありません", "No value is in the range of `{n}`"));
            return None;
        }
        Some((lo, hi))
    }

    /// A constant that is not a value of a unit.
    fn miss(&mut self, m: &Miss, s: Span, num: &Num, unit: &Unit, what: &str) {
        let raw = &num.raw;
        let msg = match m {
            Miss::NoUnit => tr!("`{what}` は {unit} の数なので、定数 `{raw}` にも単位を付けてください（`{raw}{}` など）", "`{what}` is a number of {unit}, so the constant `{raw}` takes a unit too (`{raw}{}`, say)", unit.unit; unit.unit),
            Miss::UnknownUnit => tr!("`{raw}` の単位 `{}` は単位の表にありません", "The unit `{}` of `{raw}` is not in the table of units", num.unit; num.unit),
            Miss::OtherUnit => tr!("定数 `{raw}` は、`{what}` の単位 {unit} の値ではありません", "The constant `{raw}` is not a value of {unit}, the unit of `{what}`"),
            Miss::NotWhole(_) => tr!("定数 `{raw}` を {unit} で数えると、整数になりません", "The constant `{raw}`, counted in {unit}, is not a whole number"),
            Miss::TooLarge(_) => tr!("定数 `{raw}` は、±(2⁵³ − 1) を超えます", "The constant `{raw}` is past ±(2⁵³ − 1)"),
        };
        let d = self.err(m.code(), s, msg);
        match m {
            Miss::NotWhole(_) => d.notes.push(tr!("値は型の単位で数えた整数として Cedar に渡るので、定数もその単位で整数にならなければなりません。", "A value goes to Cedar as a whole number in the unit of its type, so a constant has to come to one too.")),
            Miss::TooLarge(_) => d.notes.push(tr!("生成するコードは値を JSON の数として渡し、TypeScript の `number` が正確に表せる整数は 2⁵³ − 1 までです。", "The generated code passes a value as a JSON number, and TypeScript's `number` holds every integer only up to 2⁵³ − 1.")),
            _ => {}
        }
    }

    // ── Actions ──────────────────────────────────────────────────────────────

    fn action(&mut self, a: &'a ActionDecl) {
        let f = self.file();
        let me = a.name.ascii().to_string();
        for g in &a.guards {
            let w = &g.api.word;
            match f.used(w) {
                Some(u) if u.kind.has_operations() => {}
                Some(u) => {
                    let k = u.kind.word();
                    self.err("E101", g.api.span, tr!("`{w}` は `use {k}` なので、操作を守れません", "`{w}` is a `use {k}`, which has no operations to guard")).notes.push(tr!(
                        "`guards` が書けるのは、`use openapi`、`use proto`、`use asyncapi` の操作と、`use book` の振替の操作です。",
                        "`guards` names an operation of a `use openapi`, `use proto` or `use asyncapi`, or of a transfer of a `use book`."
                    ));
                }
                None => {
                    let known: Vec<String> = f.uses.iter().filter(|u| u.kind.has_operations()).filter_map(|u| u.name.as_ref().map(|(n, _)| n.clone())).collect();
                    self.err("E101", g.api.span, tr!("契約か帳簿 `{w}` はありません", "There is no contract or book `{w}`")).notes.push(known_note(&tr!("契約と帳簿（`use`）", "contracts and books (`use`)"), &known));
                }
            }
        }
        for r in &a.principals.0 {
            self.type_ref(r, true);
        }
        for r in &a.resources.0 {
            self.type_ref(r, false);
        }
        for i in &a.input {
            if let Some((ty, range)) = self.field(i, true) {
                self.locals.insert((me.clone(), i.name.ascii().to_string()), (ty, range, false));
            }
        }
        for c in &a.context {
            if let Some(ty) = self.computed(a, c) {
                self.locals.insert((me.clone(), c.name.ascii().to_string()), (ty, None, true));
            }
        }
    }

    /// The principal types and the resource types an action takes, as Cedar calls them.
    fn action_types(&self, a: &ActionDecl) -> (Vec<String>, Vec<String>) {
        let name = |r: &Ref| match self.scope.entity(&r.word) {
            Some(EntityRef::Declared(e)) => Some(e.name.ascii().to_string()),
            Some(EntityRef::Workflow) => Some(WORKFLOW_TYPE.to_string()),
            None => None,
        };
        (a.principals.0.iter().filter_map(name).collect(), a.resources.0.iter().filter_map(name).collect())
    }

    /// `today` is written, for what reads it (E107).
    fn needs_today(&mut self, s: Span) -> bool {
        if self.file().today.is_some() {
            return true;
        }
        self.err("E107", s, tr!("`today` を使うのに、`today` の行がありません", "`today` is used, and there is no `today` line")).notes.push(tr!(
            "`today range >=2026-10-01 <=2028-10-31 offset +00:00` のように、リクエストの日の範囲と、日を変えるオフセットを書いてください。",
            "Write the range of the request's day and the offset the day changes at, like `today range >=2026-10-01 <=2028-10-31 offset +00:00`."
        ));
        false
    }

    /// A computed value: what it calls, what it gives, and its type (E101, E102, E105, E107).
    fn computed(&mut self, a: &'a ActionDecl, c: &'a Computed) -> Option<Ty> {
        match &c.value {
            Computation::Rule { rule, args, output } => {
                let u = self.used_as(&rule.word, rule.span, UseKind::Rule)?;
                let name = u.name.as_ref().map(|(n, _)| n.clone()).unwrap_or_default();
                let read = self.scope.rules.get(&name)?;
                let facts = &read.facts;
                if facts.elements.is_some() {
                    let r = &rule.word;
                    self.err("E105", rule.span, tr!("規則 `{r}` は要素の並びをたどる規則なので、計算した値には使えません", "The rule `{r}` walks a list of elements, and cannot give a computed value")).notes.push(tr!(
                        "sekisho が規則に渡せるのは、一つずつの値だけです。",
                        "sekisho gives a rule single values only."
                    ));
                    return None;
                }
                let o = &output.word;
                let Some(out) = facts.outputs.iter().find(|x| x.name == *o || x.alias == *o) else {
                    let known: Vec<String> = facts.outputs.iter().map(|x| x.name.clone()).collect();
                    let r = &rule.word;
                    self.err("E101", output.span, tr!("規則 `{r}` に出力 `{o}` はありません", "The rule `{r}` has no output `{o}`")).notes.push(known_note(&tr!("出力", "outputs"), &known));
                    return None;
                };
                let ty = match unopt(&out.ty) {
                    ColumnType::Bool => Ty::Bool,
                    ColumnType::Enum(e) => Ty::RuleEnum(name.clone(), e.clone()),
                    other => {
                        let shown = column_text(other);
                        self.err("E105", output.span, tr!("規則の出力 `{o}` は {shown} です。計算した値にできるのは、列挙か真偽の出力だけです", "The rule's output `{o}` is {shown}; a computed value is an output that is an enum or true or false")).notes.push(tr!(
                            "条件は有限の値だけを見るので、数の出力は使えません（v1）。規則に、数を区分に分ける出力を足してください。",
                            "A condition reads finitely many values, so a numeric output is not taken (v1); give the rule an output that sorts the number into bands."
                        ));
                        return None;
                    }
                };
                let inputs: Vec<(String, String, ColumnType)> = facts.inputs.iter().map(|i| (i.name.clone(), i.alias.clone(), i.ty.clone())).collect();
                self.call_args(a, args, rule.span, &rule.word, &inputs, facts, &name);
                Some(ty)
            }
            Computation::Date { op: _, date } => {
                self.needs_today(c.span);
                match date {
                    DateValue::Attr(p) => {
                        let ty = self.attr_of_action(a, p)?;
                        if ty != Ty::Date {
                            let shown = p.text();
                            let t = self.ty_text(&ty);
                            self.err("E102", p.word().span, tr!("`{shown}` は{}で、日付ではありません", "`{shown}` is {}, not a date", t.ja; t.en));
                            return None;
                        }
                    }
                    DateValue::Call { dates, date, args } => {
                        let u = self.used_as(&dates.word, dates.span, UseKind::Dates)?;
                        let name = u.name.as_ref().map(|(n, _)| n.clone()).unwrap_or_default();
                        let read = self.scope.dates.get(&name)?;
                        let facts = read.facts.clone();
                        let d = &date.word;
                        let Some(func) = facts.functions.iter().find(|x| x.name == *d || x.alias == *d) else {
                            let known: Vec<String> = facts.functions.iter().map(|x| x.name.clone()).collect();
                            let n = &dates.word;
                            self.err("E101", date.span, tr!("日付のファイル `{n}` に日付 `{d}` はありません", "The dates file `{n}` has no date `{d}`")).notes.push(known_note(&tr!("日付", "dates"), &known));
                            return None;
                        };
                        let inputs: Vec<(String, String, ColumnType)> = func
                            .params
                            .iter()
                            .filter_map(|p| facts.inputs.iter().find(|i| i.name == *p))
                            .map(|i| (i.name.clone(), i.alias.clone(), if i.kind == DateKind::Date { ColumnType::Date } else { ColumnType::Num { written: "int".into(), unit: Some(Unit::number()), min: Some(i.min as i128), max: Some(i.max as i128) } }))
                            .collect();
                        let empty = RuleFacts { rule: String::new(), alias: String::new(), version: String::new(), sha256: String::new(), rulec: String::new(), inputs: vec![], outputs: vec![], elements: None, enums: vec![], machine: None, preconditions: vec![], connect: None, typescript: dummy_call(), python: dummy_call(), go: dummy_call() };
                        self.call_args(a, args, date.span, &date.word, &inputs, &empty, "");
                    }
                }
                Some(Ty::Bool)
            }
            Computation::Open { calendar } => {
                self.needs_today(c.span);
                self.used_as(&calendar.word, calendar.span, UseKind::Calendar)?;
                Some(Ty::Bool)
            }
        }
    }

    /// The `use` a computed value calls, of the kind it calls (E101, E105).
    fn used_as(&mut self, word: &str, s: Span, want: UseKind) -> Option<&'a Use> {
        let f = self.file();
        let (noun, line) = match want {
            UseKind::Rule => (tr!("規則", "rule"), "use rule"),
            UseKind::Dates => (tr!("日付のファイル", "dates file"), "use dates"),
            _ => (tr!("カレンダー", "calendar"), "use calendar"),
        };
        match f.used(word) {
            Some(u) if u.kind == want => Some(u),
            Some(u) => {
                let k = u.kind.word();
                self.err("E105", s, tr!("`{word}` は `use {k}` で、{}ではありません", "`{word}` is a `use {k}`, not a {}", noun.ja; noun.en)).notes.push(tr!(
                    "計算した値に使えるのは、規則の出力（`use rule`）、日付の関数（`use dates`）、カレンダーの営業日（`use calendar`）だけです。",
                    "A computed value is a rule's output (`use rule`), a date function (`use dates`), or a business day of a calendar (`use calendar`)."
                ));
                None
            }
            None => {
                let known: Vec<String> = f.uses.iter().filter(|u| u.kind == want).filter_map(|u| u.name.as_ref().map(|(n, _)| n.clone())).collect();
                self.err("E101", s, tr!("{} `{word}` はありません", "There is no {} `{word}`", noun.ja; noun.en)).notes.push(known_note(&Text::same(format!("`{line}`")), &known));
                None
            }
        }
    }

    /// The inputs given to a rule or a date function (E006, E101, E105), each held to its input by
    /// `fits`.
    #[allow(clippy::too_many_arguments)]
    fn call_args(&mut self, a: &'a ActionDecl, args: &'a [Arg], at: Span, callee: &str, inputs: &[(String, String, ColumnType)], facts: &RuleFacts, rule: &str) {
        let mut given: Vec<usize> = Vec::new();
        for arg in args {
            let w = &arg.name.word;
            let Some(k) = inputs.iter().position(|(n, al, _)| n == w || al == w) else {
                let known: Vec<String> = inputs.iter().map(|(n, ..)| n.clone()).collect();
                self.err("E101", arg.name.span, tr!("`{callee}` に入力 `{w}` はありません", "`{callee}` has no input `{w}`")).notes.push(known_note(&tr!("入力", "inputs"), &known));
                continue;
            };
            if given.contains(&k) {
                self.err("E006", arg.name.span, tr!("入力 `{w}` に二度値を渡しています", "The input `{w}` is given twice"));
                continue;
            }
            given.push(k);
            let s = arg_span(&arg.value, arg.name.span);
            self.fits_column(a, facts, rule, &inputs[k].2, &arg.value, s, &inputs[k].0);
        }
        for (k, (n, _, ct)) in inputs.iter().enumerate() {
            if !given.contains(&k) && !matches!(ct, ColumnType::Opt(_)) {
                self.err("E105", at, tr!("`{callee}` の入力 `{n}` に値を渡していません", "The input `{n}` of `{callee}` is given nothing")).notes.push(tr!(
                    "入力ごとに、`<入力>: <値>` で値を渡してください。渡せるのは、principal と resource の属性、action の入力、定数、`today` です。",
                    "Give each input a value as `<input>: <value>`: an attribute of the principal or the resource, an input of the action, a constant, or `today`."
                ));
            }
        }
    }

    /// One value given to an input of a rule or a date function, against the input's type (E101,
    /// E102, E103, E105, E107).
    #[allow(clippy::too_many_arguments)]
    fn fits_column(&mut self, a: &'a ActionDecl, facts: &RuleFacts, rule: &str, ct: &ColumnType, v: &ArgValue, s: Span, input: &str) {
        let a_ct = unopt(ct);
        let want = column_text(a_ct);
        match v {
            ArgValue::Today(ts) => {
                if !self.needs_today(*ts) {
                    return;
                }
                if *a_ct != ColumnType::Date {
                    self.err("E102", s, tr!("入力 `{input}` は {want} なので、`today` を渡せません", "The input `{input}` is {want}, and does not take `today`"));
                }
            }
            ArgValue::Lit(l, _) => match (a_ct, l) {
                (ColumnType::Bool, Lit::Bool(_)) | (ColumnType::Date, Lit::Date(_)) => {}
                (ColumnType::Num { unit: Some(u), .. }, Lit::Num(n)) => {
                    if let Err(m) = types::count(n, u) {
                        self.miss(&m, s, n, u, input);
                    }
                }
                (ColumnType::Num { unit: None, .. }, Lit::Num(_)) => {}
                (_, l) => {
                    let shown = lit_text(l);
                    self.err("E102", s, tr!("入力 `{input}` は {want} なので、`{shown}` を渡せません", "The input `{input}` is {want}, and does not take `{shown}`"));
                }
            },
            ArgValue::Path(p) => {
                // a bare word that names no input of the action: a value of the input's enum
                if let (Path::Local(r), ColumnType::Enum(e)) = (p, a_ct)
                    && a.local(&r.word).is_none()
                {
                    let values = rule_enum_values(facts, e);
                    if !values.iter().any(|v| is_rule_value(v, &r.word)) {
                        let known: Vec<String> = values.iter().map(|v| v.name.clone()).collect();
                        let w = &r.word;
                        self.err("E101", r.span, tr!("`{w}` は、action の入力でも、規則 `{rule}` の列挙 `{e}` の値でもありません", "`{w}` is neither an input of the action nor a value of the enum `{e}` of the rule `{rule}`")).notes.push(known_note(&tr!("列挙の値", "values"), &known));
                    }
                    return;
                }
                let Some(ty) = self.arg_path(a, p) else { return };
                let fits = match (a_ct, &ty) {
                    (ColumnType::Bool, Ty::Bool) | (ColumnType::Date, Ty::Date) => true,
                    (ColumnType::Num { unit: Some(u), .. }, Ty::Num(t)) => u.same(t),
                    (ColumnType::Num { unit: None, .. }, Ty::Num(_)) => true,
                    (ColumnType::Enum(e), Ty::Enum(g)) => {
                        // every value of the gate's enum is a value of the rule's
                        let values = rule_enum_values(facts, e);
                        let missing: Vec<String> = self
                            .scope
                            .enum_decl(g)
                            .map(|d| d.values.iter().filter(|x| !values.iter().any(|v| x.is(&v.name) || x.is(&v.alias) || x.is(&v.public))).map(|x| x.text.clone()).collect())
                            .unwrap_or_default();
                        if !missing.is_empty() {
                            let m = missing.join("、");
                            let m_en = missing.join(", ");
                            self.err("E102", s, tr!("列挙 `{g}` の値 {m} は、規則 `{rule}` の列挙 `{e}` にありません", "The values {m_en} of the enum `{g}` are not values of the enum `{e}` of the rule `{rule}`"));
                            return;
                        }
                        true
                    }
                    _ => false,
                };
                if !fits {
                    let shown = p.text();
                    let t = self.ty_text(&ty);
                    self.err("E102", s, tr!("入力 `{input}` は {want} ですが、`{shown}` は{}です", "The input `{input}` is {want}, and `{shown}` is {}", t.ja; t.en)).notes.push(tr!(
                        "数は単位まで同じでなければなりません（`JPY` と `円` は同じ、税込と税抜は別）。",
                        "A number has to be of the same unit (`JPY` and `円` are the same; with tax and without are not)."
                    ));
                }
            }
        }
    }

    /// The type of a value given to a rule or a date function: an attribute of the action's
    /// principal or resource, or an input of the action (a computed value is not given, E105; an
    /// entity is not, E105).
    fn arg_path(&mut self, a: &'a ActionDecl, p: &Path) -> Option<Ty> {
        match p {
            Path::Local(r) => {
                let w = &r.word;
                match a.local(w) {
                    Some(Local::Computed(_)) => {
                        self.err("E105", r.span, tr!("計算した値 `{w}` は、規則や日付の入力に渡せません", "The computed value `{w}` cannot be given to a rule or a date")).notes.push(tr!(
                            "規則と日付の入力に渡せるのは、principal と resource の属性、action の入力、定数、`today` です（v1。計算した値の順を考えずに済むように）。",
                            "A rule or a date is given an attribute of the principal or the resource, an input of the action, a constant, or `today` (v1, so that computed values need no order)."
                        ));
                        None
                    }
                    Some(Local::Input(i)) => self.locals.get(&(a.name.ascii().to_string(), i.name.ascii().to_string())).map(|x| x.0.clone()),
                    None => {
                        let n = &a.name.text;
                        let known = names_of(a.input.iter().map(|i| &i.name));
                        self.err("E101", r.span, tr!("action `{n}` に入力 `{w}` はありません", "The action `{n}` has no input `{w}`")).notes.push(known_note(&tr!("入力", "inputs"), &known));
                        None
                    }
                }
            }
            _ => {
                let ty = self.attr_of_action(a, p)?;
                if let Ty::Entity(t) = &ty {
                    let shown = p.text();
                    self.err("E105", p.word().span, tr!("`{shown}` は型 `{t}` のエンティティなので、規則や日付の入力に渡せません", "`{shown}` is an entity of the type `{t}`, which is not given to a rule or a date"));
                    return None;
                }
                Some(ty)
            }
        }
    }

    /// The type of `principal.x` or `resource.x` for an action's computed value: the attribute of
    /// one of the action's types (E101), the same type in each that has it (E102).
    fn attr_of_action(&mut self, a: &ActionDecl, p: &Path) -> Option<Ty> {
        let (ps, rs) = self.action_types(a);
        let (types, principal) = match p {
            Path::Principal(_) => (ps, true),
            Path::Resource(_) => (rs, false),
            Path::Local(_) => return None,
        };
        self.attr_in(&types, p, principal)
    }

    /// The type of an attribute among some types: E101 when none of them has it, E102 when two have
    /// it of different types.
    fn attr_in(&mut self, types: &[String], p: &Path, principal: bool) -> Option<Ty> {
        let w = &p.word().word;
        let mut found: Vec<(String, Ty)> = Vec::new();
        let mut declared = false;
        for t in types {
            let Some(EntityRef::Declared(e)) = self.scope.entity(t) else { continue };
            if let Some(field) = e.attribute(w) {
                declared = true;
                if let Some((ty, _)) = self.attrs.get(&(t.clone(), field.name.ascii().to_string())) {
                    found.push((t.clone(), ty.clone()));
                }
            }
        }
        // declared, with a type that is already said to be wrong: nothing more to say here
        if found.is_empty() && declared {
            return None;
        }
        let Some((_, first)) = found.first().cloned() else {
            let (whose, whose_en) = if principal { ("principal", "the principal") } else { ("resource", "the resource") };
            let names = self.type_names(types);
            let (ts, ts_en) = (names.ja, names.en);
            let shown = p.text();
            self.err("E101", p.word().span, tr!("`{shown}` はありません。{whose} の型（{ts}）のどれにも属性 `{w}` がありません", "There is no `{shown}`: none of the types of {whose_en} ({ts_en}) has an attribute `{w}`"));
            return None;
        };
        if let Some((t, other)) = found.iter().find(|(_, ty)| *ty != first) {
            let shown = p.text();
            let (a, b) = (self.ty_text(&first), self.ty_text(other));
            self.err("E102", p.word().span, tr!("`{shown}` の型が、型によって違います（{} と、`{t}` では {}）", "`{shown}` is of different types in different types ({}, and {} in `{t}`)", a.ja, b.ja; a.en, b.en));
            return None;
        }
        Some(first)
    }

    // ── Policies and expectations ───────────────────────────────────────────

    /// A policy's or an expectation's body: its scope against its actions (E101, E106), and its
    /// conditions (E101–E104). A forbid of a file read with `use gate` (`imported`) holds for the
    /// actions its principal line fits, and says nothing of the others.
    fn body(&mut self, b: &'a Body, effect: Option<Effect>, imported: bool) {
        let f = self.file();
        let mut actions: Vec<&'a ActionDecl> = match &b.action.0 {
            What::Any => f.actions.iter().collect(),
            What::Actions(refs) => refs.iter().filter_map(|r| self.action_ref(r)).collect(),
        };
        // the principal types of each action, narrowed by the principal line
        let mut per: Vec<Vec<String>> = actions.iter().map(|a| self.action_types(a).0).collect();
        if let Some((who, s)) = &b.principal {
            let Some(picked) = self.who(who, &actions, &b.action.0, *s, effect, imported) else { return };
            if imported {
                let mut fits = picked.iter().map(|p| !p.is_empty());
                actions.retain(|_| fits.next().unwrap_or(false));
            }
            per = picked.into_iter().filter(|p| !imported || !p.is_empty()).collect();
        }
        let mut principals: Vec<String> = Vec::new();
        for p in per.into_iter().flatten() {
            if !principals.contains(&p) {
                principals.push(p);
            }
        }
        // nothing to read the conditions over: no action it holds for, or a line already said wrong
        if actions.is_empty() || principals.is_empty() {
            return;
        }
        let mut resources: Vec<String> = Vec::new();
        for a in &actions {
            for r in self.action_types(a).1 {
                if !resources.contains(&r) {
                    resources.push(r);
                }
            }
        }
        let env = Env { actions, principals, resources };
        for c in &b.conds {
            self.expr(&env, &c.expr);
        }
    }

    /// For each action, its principal types a `principal` line picks: E106 for an action it picks
    /// none of (for `action any`, when it picks none of any), but for a forbid of another file; None
    /// when a name of the line names nothing (E101).
    #[allow(clippy::too_many_arguments)]
    fn who(&mut self, who: &Who, actions: &[&'a ActionDecl], what: &What, s: Span, effect: Option<Effect>, imported: bool) -> Option<Vec<Vec<String>>> {
        let scope = self.scope;
        let kind = match effect {
            Some(e) => Text::same(e.word()),
            None => tr!("期待", "expectation"),
        };
        let picks = |me: &Self, a: &ActionDecl| -> Vec<String> {
            let (ps, _) = me.action_types(a);
            match who {
                Who::Is(r) => match scope.entity(&r.word) {
                    Some(EntityRef::Declared(e)) => ps.into_iter().filter(|p| *p == e.name.ascii()).collect(),
                    Some(EntityRef::Workflow) => ps.into_iter().filter(|p| p == WORKFLOW_TYPE).collect(),
                    None => vec![],
                },
                Who::Workflow(_) => ps.into_iter().filter(|p| p == WORKFLOW_TYPE).collect(),
                Who::In(roles) => {
                    let wanted: Vec<String> = roles.iter().filter_map(|r| scope.role(&r.word).map(|d| d.name.ascii().to_string())).collect();
                    ps.into_iter()
                        .filter(|p| match scope.entity(p) {
                            Some(EntityRef::Declared(e)) => e.roles.iter().any(|held| scope.held(&held.word).iter().any(|h| wanted.contains(h))),
                            _ => false,
                        })
                        .collect()
                }
            }
        };
        // the names first
        let ok = match who {
            Who::Is(r) => self.type_ref(r, true).is_some(),
            Who::Workflow(r) => {
                if scope.workflow(&r.word).is_none() {
                    let w = &r.word;
                    let known = names_of(scope.files.iter().flat_map(|f| f.workflows.iter().map(|w| &w.name)));
                    self.err("E101", r.span, tr!("ワークフロー `{w}` はありません", "There is no workflow `{w}`")).notes.push(known_note(&tr!("ワークフロー", "workflows"), &known));
                    false
                } else {
                    true
                }
            }
            Who::In(roles) => {
                // every role is looked up, so that each unknown one is said
                let found: Vec<bool> = roles.iter().map(|r| self.role_ref(r).is_some()).collect();
                found.into_iter().all(|b| b)
            }
        };
        if !ok {
            return None;
        }
        let per: Vec<Vec<String>> = actions.iter().map(|a| picks(self, a)).collect();
        let say = |me: &mut Self, a: &ActionDecl| {
            let (ps, _) = me.action_types(a);
            let n = &a.name.text;
            let names = me.type_names(&ps);
            let (ts, ts_en) = (names.ja, names.en);
            let k = &kind;
            me.err("E106", s, tr!("この {} の `principal` の行に当たる principal は、action `{n}` には来ません", "No principal this {}'s `principal` line picks comes to the action `{n}`", k.ja; k.en)).notes.push(tr!(
                "action `{n}` に来る principal の型は {ts} です。役割で選ぶときは、その役割を持てる型（`roles`）が要ります。",
                "The principal types that come to `{n}` are {ts_en}; a role picks a type that can hold it (its `roles`)."
            ));
        };
        if !imported {
            match what {
                What::Actions(_) => {
                    for (a, p) in actions.iter().zip(&per) {
                        if p.is_empty() {
                            say(self, a);
                        }
                    }
                }
                What::Any => {
                    if per.iter().all(|p| p.is_empty())
                        && let Some(a) = actions.first()
                    {
                        say(self, a);
                    }
                }
            }
        }
        Some(per)
    }

    fn expr(&mut self, env: &Env<'a>, e: &'a Expr) {
        match e {
            Expr::Or(es) | Expr::And(es) => {
                for x in es {
                    self.expr(env, x);
                }
            }
            Expr::Not(x, _) => self.expr(env, x),
            Expr::Atom(a, _) => self.atom(env, a),
        }
    }

    fn atom(&mut self, env: &Env<'a>, a: &'a Atom) {
        let scope = self.scope;
        match a {
            Atom::InRole(r) => {
                self.role_ref(r);
            }
            Atom::IsType(r) => {
                self.type_ref(r, true);
            }
            Atom::IsWorkflow(r) => {
                if scope.workflow(&r.word).is_none() {
                    let w = &r.word;
                    self.err("E101", r.span, tr!("ワークフロー `{w}` はありません", "There is no workflow `{w}`"));
                }
            }
            Atom::InGroup(p) => {
                let Some(ty) = self.path_ty(env, p) else { return };
                if !matches!(ty, Ty::Entity(_)) {
                    let shown = p.text();
                    let t = self.ty_text(&ty);
                    self.err("E102", p.word().span, tr!("`{shown}` は{}なので、`principal in` で読めません", "`{shown}` is {}, which `principal in` does not read", t.ja; t.en)).notes.push(tr!(
                        "`principal in` のあとには、役割か、グループのエンティティを指す属性（`resource.team`）を書きます。",
                        "`principal in` is followed by a role, or an attribute that points to a group (`resource.team`)."
                    ));
                }
            }
            Atom::Holds(p) => {
                let Some(ty) = self.path_ty(env, p) else { return };
                if ty != Ty::Bool {
                    let shown = p.text();
                    let t = self.ty_text(&ty);
                    self.err("E102", p.word().span, tr!("`{shown}` は{}で、真偽ではありません", "`{shown}` is {}, not true or false", t.ja; t.en)).notes.push(tr!(
                        "真偽でない値は、`{shown} is <値>` のように比べてください。",
                        "Compare a value that is not true or false: `{shown} is <value>`."
                    ));
                }
            }
            Atom::Compare { left, op: _, right } => {
                let Some(ty) = self.path_ty(env, left) else { return };
                let shown = left.text();
                let Ty::Num(unit) = &ty else {
                    let t = self.ty_text(&ty);
                    self.err("E102", left.word().span, tr!("`{shown}` は{}なので、`<` や `>=` で比べられません", "`{shown}` is {}, which `<` and `>=` do not compare", t.ja; t.en)).notes.push(tr!("大きさで比べられるのは数だけです。", "Only a number compares by size."));
                    return;
                };
                self.constant(env, left, &right.0, right.1, unit);
            }
            Atom::Is { left, not: _, right } => {
                let Some(ty) = self.path_ty(env, left) else { return };
                self.is(env, left, &ty, right);
            }
        }
    }

    /// A constant compared with a number: in its unit, a whole number, within the number's range
    /// (E102, E103).
    fn constant(&mut self, env: &Env<'a>, left: &Path, l: &Lit, s: Span, unit: &Unit) {
        let shown = left.text();
        let Lit::Num(n) = l else {
            let c = lit_text(l);
            self.err("E102", s, tr!("`{shown}` は {unit} の数なので、`{c}` とは比べられません", "`{shown}` is a number of {unit}, which does not compare with `{c}`"));
            return;
        };
        let v = match types::count(n, unit) {
            Ok(v) => v,
            Err(m) => {
                self.miss(&m, s, n, unit, &shown);
                return;
            }
        };
        if let Some((lo, hi)) = self.path_range(env, left)
            && (v < lo || v > hi)
        {
            let raw = &n.raw;
            // the ends in the unit, when one integer counts one of it
            let u = if unit.step.is_none() && unit.dim != ritsu_units::Dim::Rate { unit.unit.clone() } else { String::new() };
            self.err("E103", s, tr!("定数 `{raw}` は、`{shown}` の範囲（{lo}{u}〜{hi}{u}）の外です", "The constant `{raw}` is outside the range of `{shown}` ({lo}{u} to {hi}{u})")).notes.push(tr!(
                "範囲の外の定数と比べると、どの値でも答えが同じになります。比べる定数か、`{shown}` の範囲を直してください。",
                "Against a constant outside the range, every value gives the same answer; correct the constant, or the range of `{shown}`."
            ));
        }
    }

    /// `<left> is <right>`.
    fn is(&mut self, env: &Env<'a>, left: &Path, ty: &Ty, right: &Rhs) {
        let shown = left.text();
        let mismatch = |me: &mut Self, s: Span, what: &str| {
            let t = me.ty_text(ty);
            me.err("E102", s, tr!("`{shown}` は{}なので、`{what}` とは比べられません", "`{shown}` is {}, which does not compare with `{what}`", t.ja; t.en));
        };
        match (ty, right) {
            (Ty::Bool, Rhs::Lit(Lit::Bool(_), _)) => {}
            (Ty::Bool, Rhs::Word(w)) => {
                mismatch(self, w.span, &w.word);
                if let Some(d) = self.diags.last_mut() {
                    d.notes.push(tr!("真偽は `when {shown}` か `unless {shown}` と書きます。", "True or false is written `when {shown}` or `unless {shown}`."));
                }
            }
            (Ty::Num(unit), Rhs::Lit(l, s)) => self.constant(env, left, l, *s, unit),
            (Ty::Enum(e), Rhs::Word(w)) => {
                let Some(d) = self.scope.enum_decl(e) else { return };
                if d.value(&w.word).is_none() {
                    let known = names_of(d.values.iter());
                    let (x, en) = (&w.word, &d.name.text);
                    self.err("E101", w.span, tr!("列挙 `{en}` に値 `{x}` はありません", "The enum `{en}` has no value `{x}`")).notes.push(known_note(&tr!("値", "values"), &known));
                }
            }
            (Ty::RuleEnum(rule, e), Rhs::Word(w)) => {
                let Some(read) = self.scope.rules.get(rule) else { return };
                let values = rule_enum_values(&read.facts, e);
                if !values.iter().any(|v| is_rule_value(v, &w.word)) {
                    let known: Vec<String> = values.iter().map(|v| v.name.clone()).collect();
                    let x = &w.word;
                    self.err("E101", w.span, tr!("規則 `{rule}` の列挙 `{e}` に値 `{x}` はありません", "The enum `{e}` of the rule `{rule}` has no value `{x}`")).notes.push(known_note(&tr!("値", "values"), &known));
                }
            }
            (Ty::Enum(e), Rhs::Path(p)) => {
                let Some(t) = self.path_ty(env, p) else { return };
                if t != Ty::Enum(e.clone()) {
                    let (a, b) = (self.ty_text(ty), self.ty_text(&t));
                    let other = p.text();
                    self.err("E104", p.word().span, tr!("`{shown}`（{}）と `{other}`（{}）は型が違うので、比べられません", "`{shown}` ({}) and `{other}` ({}) are of different types, and do not compare", a.ja, b.ja; a.en, b.en));
                }
            }
            (Ty::Entity(t), Rhs::Principal(s)) => {
                if !env.principals.contains(t) {
                    let names = self.type_names(&env.principals);
                    let (ps, ps_en) = (names.ja, names.en);
                    let tn = self.type_name(t);
                    self.err("E104", *s, tr!("`{shown}` は `{tn}` を指しますが、principal は {ps} です", "`{shown}` points to a `{tn}`, and the principal is {ps_en}")).notes.push(tr!(
                        "属性が principal そのものかを比べるには、属性の型が、その action に来る principal の型と同じでなければなりません。",
                        "Whether an attribute is the principal is asked of an attribute of the type of a principal that comes to the action."
                    ));
                }
            }
            (Ty::Entity(t), Rhs::Path(p)) => {
                let Some(other_ty) = self.path_ty(env, p) else { return };
                if other_ty != Ty::Entity(t.clone()) {
                    let (a, b) = (self.ty_text(ty), self.ty_text(&other_ty));
                    let other = p.text();
                    self.err("E104", p.word().span, tr!("`{shown}`（{}）と `{other}`（{}）は型が違うので、同じエンティティかを比べられません", "`{shown}` ({}) and `{other}` ({}) are of different types, and are not compared for one entity", a.ja, b.ja; a.en, b.en));
                }
            }
            (Ty::Date, _) => {
                let s = rhs_span(right, left.word().span);
                self.err("E102", s, tr!("日付 `{shown}` は条件の中で比べられません", "The date `{shown}` is not compared in a condition")).notes.push(tr!(
                    "日付は、`context` の計算した値にしてから条件に使います（`in_period = today <= resource.paid_on`）。",
                    "A date is compared in a computed value of `context` (`in_period = today <= resource.paid_on`), which a condition reads."
                ));
            }
            (_, Rhs::Word(w)) => mismatch(self, w.span, &w.word),
            (_, Rhs::Principal(s)) => mismatch(self, *s, "principal"),
            (_, Rhs::Path(p)) => {
                let other = p.text();
                let s = p.word().span;
                if matches!(ty, Ty::Num(_)) {
                    self.err("E104", s, tr!("数 `{shown}` は、ほかの値 `{other}` とは比べられません。数は定数とだけ比べます", "The number `{shown}` does not compare with another value, `{other}`; a number compares with a constant"));
                } else {
                    mismatch(self, s, &other);
                }
            }
            (_, Rhs::Lit(l, s)) => mismatch(self, *s, &lit_text(l)),
        }
    }

    /// The type of a value a condition reads (E101, E102).
    fn path_ty(&mut self, env: &Env<'a>, p: &Path) -> Option<Ty> {
        match p {
            Path::Principal(_) => self.attr_in(&env.principals.clone(), p, true),
            Path::Resource(_) => self.attr_in(&env.resources.clone(), p, false),
            Path::Local(r) => {
                let w = &r.word;
                let mut ty: Option<Ty> = None;
                for a in &env.actions {
                    let Some(l) = a.local(w) else {
                        let n = &a.name.text;
                        let d = self.err("E101", r.span, tr!("action `{n}` に、入力か計算した値 `{w}` はありません", "The action `{n}` has no input or computed value `{w}`"));
                        let known = names_of(a.input.iter().map(|i| &i.name).chain(a.context.iter().map(|c| &c.name)));
                        d.notes.push(known_note(&tr!("入力と計算した値", "inputs and computed values"), &known));
                        return None;
                    };
                    let key = match l {
                        Local::Input(i) => (a.name.ascii().to_string(), i.name.ascii().to_string()),
                        Local::Computed(c) => (a.name.ascii().to_string(), c.name.ascii().to_string()),
                    };
                    let t = self.locals.get(&key)?.0.clone();
                    match &ty {
                        None => ty = Some(t),
                        Some(prev) if *prev != t => {
                            let (x, y) = (self.ty_text(prev), self.ty_text(&t));
                            let n = &a.name.text;
                            self.err("E102", r.span, tr!("`{w}` の型が action によって違います（{} と、`{n}` では {}）", "`{w}` is of different types in different actions ({}, and {} in `{n}`)", x.ja, y.ja; x.en, y.en));
                            return None;
                        }
                        _ => {}
                    }
                }
                ty
            }
        }
    }

    /// The range of a number a condition reads, on the wire: the narrowest of the ranges it has in
    /// the types or the actions that have it.
    fn path_range(&self, env: &Env<'a>, p: &Path) -> Option<(i128, i128)> {
        let w = &p.word().word;
        let mut out: Option<(i128, i128)> = None;
        let mut join = |r: (i128, i128)| out = Some(out.map_or(r, |(a, b)| (a.min(r.0), b.max(r.1))));
        match p {
            Path::Principal(_) | Path::Resource(_) => {
                let types = if matches!(p, Path::Principal(_)) { &env.principals } else { &env.resources };
                for t in types {
                    let Some(EntityRef::Declared(e)) = self.scope.entity(t) else { continue };
                    if let Some(f) = e.attribute(w)
                        && let Some((_, Some(r))) = self.attrs.get(&(t.clone(), f.name.ascii().to_string()))
                    {
                        join(*r);
                    }
                }
            }
            Path::Local(_) => {
                for a in &env.actions {
                    if let Some(Local::Input(i)) = a.local(w)
                        && let Some((_, Some(r), _)) = self.locals.get(&(a.name.ascii().to_string(), i.name.ascii().to_string()))
                    {
                        join(*r);
                    }
                }
            }
        }
        out
    }
}

/// The names declared so far in one namespace: each name and alias, with its line, the file it is
/// declared in when it is another, and the declaration's name.
#[derive(Default)]
struct Space {
    keys: BTreeMap<String, (usize, Option<String>, String)>,
}

impl Space {
    fn insert(&mut self, n: &Name, from: Option<&str>) {
        let v = (n.span.line, from.map(str::to_string), n.text.clone());
        self.keys.entry(n.text.clone()).or_insert(v.clone());
        if let Some((a, _)) = &n.alias {
            self.keys.entry(a.clone()).or_insert(v);
        }
    }

    fn insert_from(&mut self, n: &Name, file: &str) {
        self.insert(n, Some(file));
    }

    fn find(&self, k: &str) -> Option<(usize, Option<String>, String)> {
        self.keys.get(k).cloned()
    }
}

fn names_of<'n>(names: impl Iterator<Item = &'n Name>) -> Vec<String> {
    names.map(|n| n.text.clone()).collect()
}

/// `書ける役割は `clerk`、`manager` です。`, or that there is none.
fn known_note(what: &Text, known: &[String]) -> Text {
    if known.is_empty() {
        return tr!("{}は一つも宣言されていません。", "No {} are declared.", what.ja; what.en);
    }
    let quoted: Vec<Text> = known.iter().map(|k| Text::same(format!("`{k}`"))).collect();
    let list = Text::list(&quoted);
    tr!("書ける{}は {} です。", "The {} there are {}.", what.ja, list.ja; what.en, list.en)
}

fn lit_text(l: &Lit) -> String {
    match l {
        Lit::Num(n) => n.raw.clone(),
        Lit::Date(d) => types::day_text(*d),
        Lit::Bool(b) => b.to_string(),
    }
}

fn column_text(t: &ColumnType) -> String {
    match t {
        ColumnType::Bool => "bool".into(),
        ColumnType::Str => "string".into(),
        ColumnType::Date => "date".into(),
        ColumnType::Enum(e) => e.clone(),
        ColumnType::Num { written, .. } => written.clone(),
        ColumnType::Opt(x) => format!("{}?", column_text(x)),
    }
}

fn arg_span(v: &ArgValue, at: Span) -> Span {
    match v {
        ArgValue::Path(p) => p.word().span,
        ArgValue::Today(s) | ArgValue::Lit(_, s) => *s,
    }
    .max(at)
}

fn rhs_span(r: &Rhs, at: Span) -> Span {
    match r {
        Rhs::Word(w) => w.span,
        Rhs::Principal(s) | Rhs::Lit(_, s) => *s,
        Rhs::Path(p) => p.word().span,
    }
    .max(at)
}

fn dummy_call() -> ritsu_ports::Call {
    ritsu_ports::Call { module: String::new(), function: String::new(), input_type: String::new(), params: vec![], outputs: vec![], enums: vec![] }
}
