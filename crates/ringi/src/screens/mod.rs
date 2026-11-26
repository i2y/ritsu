//! The screens of the kinds of request (DESIGN 13): what `ritsu gen --target requests` writes into
//! `<out>/requests/a2ui/` beside the description of the kinds. For each kind, six screens in
//! A2UI 1.0 messages, in English and in Japanese ([`a2ui`]): the form a requester fills in and makes
//! again, the card an approver answers on, the list of the tickets that wait for the person, the
//! list of the person's own requests, the history of one, and the notice of the day a request of
//! the kind may be made from, for a kind no version of which is in effect yet. Once for
//! all the kinds, the catalog of the parts and functions the screens use beside the basic catalog
//! ([`catalog`]), and the functions themselves, an ES module a browser reads as it is
//! ([`functions`]). Every screen is made from the form and the route as the files declare them,
//! and before anything is written each is held to what it sends ([`bindings`]): a button sends
//! only what the screen shows (E701), rounded as it is shown (W702). A language the file declares
//! labels in has a label for everything the screens show, and the file's own language a title for
//! what comes from elsewhere: a value of a rule's enum, a stage or a way of a shared route written
//! in another language (E703). Every number's range is one a screen holds exactly (E704).

pub mod a2ui;
pub mod bindings;
pub mod catalog;
pub mod functions;

use crate::ast::{Field, File, LabelKey, TicketLine, TypeExpr, UseKind};
use crate::r#gen::{Fld, Kind, Named};
use ritsu_base::json::Json;
use ritsu_base::text::Text;

/// The basic catalog of A2UI 1.0, which the surfaces default to: its ID, as the catalog says it.
pub const BASIC: &str = "https://a2ui.org/specification/v1_0/catalogs/basic/catalog.json";

/// The six screens of a kind, in the order they are written.
pub const SCREENS: [&str; 6] = ["form", "card", "list", "mine", "history", "offered"];

/// The languages each screen is written in: English first.
pub const LANGS: [&str; 2] = ["en", "ja"];

/// What the screens are made with besides the kinds.
#[derive(Clone, Debug)]
pub struct Given {
    /// The ID of the catalog of ritsu's parts (`ritsu gen --catalog`): a name for the project's
    /// catalog, written as a URL the project owns, which need not be where `a2ui/catalog.json` is
    /// served, and the same in every environment.
    pub catalog: String,
    /// Each kind's `values.rule` as rulec writes it in JavaScript for a page (one module, the enums
    /// it reads inside it), by the kind's ASCII name. A kind with nothing to work out has none.
    pub rules: Vec<(String, String)>,
}

/// What a kind's screens need of its files besides its description (DESIGN 13).
#[derive(Clone, Debug, Default)]
pub struct Screen {
    /// Each `labels <language>` the file writes, by its language, with its line.
    pub declared: Vec<(String, usize)>,
    /// What the screens show with no title in the file's own language (E703, [`untitled`]): each
    /// by its key as `labels` writes it, with the line it is said at.
    pub untitled: Vec<(String, usize)>,
    /// Whether a ticket is sent back to any earlier stage (`send back to any earlier stage`), not to
    /// the requester alone.
    pub back_to_stages: bool,
    /// Whether the file writes `submitted by`: whether some people may not make the request.
    pub submitted_by: bool,
    /// What the form works out with the request's `values.rule`; None when it has nothing to work out.
    pub worked: Option<Worked>,
}

/// What the form works out, as the screens call it: the function rulec writes for `values.rule`,
/// called for one output at a time (DESIGN 13.7). An output is called with the form's values of
/// what it reads, and a value of its range for each argument it does not read, which cannot change
/// it; it is worked out once the fields it reads are there.
#[derive(Clone, Debug)]
pub struct Worked {
    /// Its name in the code (`expense_values`).
    pub function: String,
    /// What it takes, in its order.
    pub params: Vec<Param>,
    /// What each argument is given where the output called does not read it, in the order of
    /// `params`, as the API takes a value: a value of the field's range (its least, else its most,
    /// as the lowered files take where a field is empty, [`crate::lower::stand_in`]), no value for an
    /// input that may have none, no for whether a field is filled in, and for rows as many rows as
    /// the field asks for at least, each of such values (with the value of each field of a row,
    /// for a field of a row an output does not read). Each passes the rule's door, and the rule
    /// answers whatever an output reads: its tables are complete, it holds no computation to a
    /// range, and a total's range is what the total can reach (DESIGN 4.4, 13.7).
    pub stand_ins: Vec<Json>,
    /// Each output, by the request's name of it (`amount`, `trip_to_shown`), with its name in the code.
    pub outputs: Vec<(String, String)>,
    /// What each output reads, by its name in the code ([`crate::lower::LFile::reads`]): each
    /// argument, by its place in `params`, with the fields of a row it reads, by their names in the
    /// code.
    pub reads: Vec<(String, Vec<(usize, Vec<String>)>)>,
    /// Each computed output, by its name in the code, with the fields that may be empty it reads
    /// (DESIGN 4.9): it has no value while one of them is empty.
    pub value_when: Vec<(String, Vec<String>)>,
    /// The computations held to their ranges, as the glue holds them (DESIGN 4.4).
    pub held: Vec<Hold>,
}

/// One argument of what the form works out.
#[derive(Clone, Debug)]
pub enum Param {
    /// A field's value; for a choice, each of its values the lowered files write under another name
    /// (`after_value` for `after`, DESIGN 6.4), with that name, which the rule takes it by; and what
    /// the rule is given while the field is empty or hidden.
    Value(String, Vec<(String, String)>, Empty),
    /// Whether a field is filled in (a condition's `cover is none`).
    Given(String),
    /// The rows of a field: each field of a row the rule takes, by its name in the code, with the
    /// row's field, and whether it says whether that field is filled in.
    Rows(String, Vec<(String, String, bool)>),
}

/// What a rule is given for a field the form has no value of: one that is empty, or hidden, which
/// the machine reads as empty (DESIGN 4.9).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Empty {
    /// No value: the rule takes one that may be absent.
    None,
    /// A value of its range: the rule takes whether the field is filled in beside it, and no answer
    /// reads the value then.
    StandIn,
    /// Nothing: a field the requester has to fill in. What reads it is not worked out until it is
    /// filled in (the check of the field says so).
    Wait,
}

impl Empty {
    /// As the functions read it.
    pub fn word(self) -> &'static str {
        match self {
            Empty::None => "none",
            Empty::StandIn => "standin",
            Empty::Wait => "wait",
        }
    }
}

/// A computation held to its range: the field (`subtotal` of the rows `lines`), the outputs that
/// are its least and its most with the ends they may not pass, and the refusal's words; and the
/// fields that may be empty it reads, where it has no value, and is not held (DESIGN 4.9).
#[derive(Clone, Debug)]
pub struct Hold {
    pub field: String,
    pub rows: Option<String>,
    pub lo: Option<(String, i128)>,
    pub hi: Option<(String, i128)>,
    pub said: Text,
    pub when: Vec<String>,
}

impl Hold {
    /// The field as the form names it (`lines.subtotal`).
    pub fn whole(&self) -> String {
        match &self.rows {
            Some(r) => format!("{r}.{}", self.field),
            None => self.field.clone(),
        }
    }
}

impl Screen {
    /// What the screens need of the checked request on the machine `m`.
    pub fn of(m: &crate::engine::Machine) -> Screen {
        let f = m.s.file;
        let declared = f.labels.iter().map(|b| (b.lang.text.clone(), b.line)).collect();
        let lines: &[TicketLine] = f.ticket.as_ref().map(|t| t.lines.as_slice()).unwrap_or_default();
        let back_to_stages = lines.iter().any(|l| matches!(l, TicketLine::SendBack { to_requester: false, .. }));
        let submitted_by = crate::org::submit::of(f).is_some();
        Screen { declared, untitled: untitled(m.s), back_to_stages, submitted_by, worked: worked(m) }
    }
}

/// What the screens of the request `s` would show with no title in the file's own language (E703):
/// a value of a rule's enum a `choice from` shows, which no line under the field titles and the
/// rule gives no display name in that language (said at the field's line), and a stage or a way of a shared route written in another language, which
/// the route's `labels` of the file's language does not title (said at the `use route` line).
/// Each by its key as `labels` writes it (`kind.goods`, `stage head`).
pub fn untitled(s: &crate::names::Scope) -> Vec<(String, usize)> {
    let lang = crate::r#gen::written_in(s.file);
    let mut out = Vec::new();
    let mut fields: Vec<(String, &Field)> = Vec::new();
    for f in &s.file.form {
        fields.push((f.name.text.clone(), f));
        fields.extend(f.children.iter().map(|c| (format!("{}.{}", f.name.text, c.name.text), c)));
    }
    for (key, f) in fields {
        let Some(e) = crate::names::enum_of(s, f) else { continue };
        // the display names the rule writes count where it writes them in the file's language
        let ruled = matches!(&f.ty, TypeExpr::ChoiceFrom(p) if s.rule_facts(&p[0].text).is_some_and(|r| crate::r#gen::rule_written_in(r) == lang));
        for v in e.values.iter().filter(|v| !(ruled && v.title.is_some() || f.value_titles.iter().any(|t| t.name.text == v.name || t.name.text == v.public || t.name.ascii() == v.public))) {
            out.push((format!("{key}.{}", v.name), f.line));
        }
    }
    let used = s.shared_routes();
    for u in s.file.uses.iter().filter(|u| u.kind == UseKind::Route) {
        let Some(r) = u.alias.as_ref().and_then(|a| s.ringi(&a.text, UseKind::Route)) else { continue };
        if crate::r#gen::written_in(r) == lang || !used.iter().flatten().any(|x| std::ptr::eq(*x, r)) {
            continue;
        }
        let titled = |key: &LabelKey| r.labels.iter().filter(|b| b.lang.text == lang).flat_map(|b| &b.entries).any(|e| same_key(&e.key, key));
        for n in crate::names::own_stages(r) {
            if !titled(&LabelKey::Stage(crate::ast::Word { text: n.text.clone(), span: n.span })) {
                out.push((format!("stage {}", n.text), u.line));
            }
        }
        for w in &r.who {
            if !titled(&LabelKey::Who(crate::ast::Word { text: w.name.text.clone(), span: w.name.span })) {
                out.push((format!("who {}", w.name.text), u.line));
            }
        }
    }
    out
}

/// Whether two keys of `labels` name the same thing.
fn same_key(a: &LabelKey, b: &LabelKey) -> bool {
    match (a, b) {
        (LabelKey::Request, LabelKey::Request) => true,
        (LabelKey::Field(x), LabelKey::Field(y)) => x.len() == y.len() && x.iter().zip(y).all(|(x, y)| x.text == y.text),
        (LabelKey::Stage(x), LabelKey::Stage(y)) | (LabelKey::Who(x), LabelKey::Who(y)) => x.text == y.text,
        _ => false,
    }
}

/// The title a stage or a way to choose an approver (`key`, as `labels` writes it: `stage head`,
/// `who dept_head`; named `name`) is shown by in the request `s`'s own language, and its labels in
/// the others (DESIGN 4.8, 5.11). The labels are the file's own, then, for a language the file gives
/// none in, those of the shared route that declares it. One of a shared route written in another
/// language is titled by the route's `labels` of the file's language (where the route has none,
/// E703 says so, and it is titled by its name); one of the file, or of a route written in the
/// file's language, is titled by its name.
pub fn titled(s: &crate::names::Scope, key: &LabelKey, name: &str) -> (String, Vec<(String, String)>) {
    let of = |f: &File| -> Vec<(String, String)> { f.labels.iter().filter_map(|b| b.entries.iter().find(|e| same_key(&e.key, key)).map(|e| (b.lang.text.clone(), e.text.clone()))).collect() };
    let lang = crate::r#gen::written_in(s.file);
    let declares = |r: &File| match key {
        LabelKey::Stage(w) => crate::names::own_stages(r).iter().any(|n| n.text == w.text),
        LabelKey::Who(w) => r.who.iter().any(|x| x.name.text == w.text),
        _ => false,
    };
    let mut labels = of(s.file);
    let Some(r) = s.shared_routes().into_iter().flatten().find(|r| declares(r)) else {
        return (name.to_string(), labels);
    };
    let theirs = of(r);
    let title = match theirs.iter().find(|(l, _)| l == lang) {
        Some((_, t)) if crate::r#gen::written_in(r) != lang => t.clone(),
        _ => name.to_string(),
    };
    for (l, t) in theirs.into_iter().filter(|(l, _)| l != lang) {
        if !labels.iter().any(|(x, _)| *x == l) {
            labels.push((l, t));
        }
    }
    (title, labels)
}

/// What the form works out, read from the request's `values.rule` as the glue reads it.
fn worked(m: &crate::engine::Machine) -> Option<Worked> {
    let lf = m.lowered.files.iter().find(|f| f.part == crate::lower::Part::Values)?;
    let rules = m.suite.rules.as_ref()?;
    let facts = m.fs(|| rules.facts(&m.lowered.path(lf))).ok()?;
    // the request's name of a name the file gives
    let back = |given: &str| lf.renamed.iter().find(|(g, _)| g == given).map(|(_, w)| w.clone()).unwrap_or_else(|| given.to_string());
    let field = |name: &str| m.s.fields().iter().find(|f| f.name.ascii() == name);
    let code = |c: &ritsu_ports::Column| if c.alias.is_empty() { c.name.clone() } else { c.alias.clone() };
    let mut params = Vec::new();
    let mut stand_ins = Vec::new();
    // each argument by the file's name of it, and for rows its element's names and its fields'
    let mut names: Vec<(Vec<String>, Vec<(String, String)>)> = Vec::new();
    for p in &facts.typescript.params {
        if let Some((n, a, cols)) = facts.elements.iter().find(|(n, a, _)| *n == p.name || *a == p.name) {
            let rows_name = back(&p.name);
            let given = |c: &ritsu_ports::Column| lf.given.iter().find(|(g, _)| *g == c.name).map(|(_, w)| w.rsplit('.').next().unwrap_or(w).to_string());
            let taken: Vec<(String, String, bool)> = cols.iter().map(|c| match given(c) {
                Some(child) => (code(c), child, true),
                None => (code(c), back(&c.name), false),
            }).collect();
            stand_ins.push(field(&rows_name).map(|f| rows_stand_in(m, f, &taken)).unwrap_or(Json::Null));
            names.push((vec![n.clone(), a.clone()], cols.iter().map(|c| (c.name.clone(), code(c))).collect()));
            params.push(Param::Rows(rows_name, taken));
        } else if let Some((_, f)) = lf.given.iter().find(|(n, _)| *n == p.name) {
            stand_ins.push(Json::Bool(false));
            names.push((vec![p.name.clone()], Vec::new()));
            params.push(Param::Given(f.clone()));
        } else {
            let name = back(&p.name);
            let named = m.s.fields().iter().find(|f| f.name.ascii() == name && matches!(f.ty, TypeExpr::Choice)).map(|f| crate::lower::words::lowered_values(m.s, f).into_iter().filter(|(a, l)| a != l).collect()).unwrap_or_default();
            let opt = facts.inputs.iter().any(|i| i.name == p.name && matches!(i.ty, ritsu_ports::ColumnType::Opt(_)));
            let paired = lf.given.iter().any(|(_, f)| *f == name);
            let empty = if opt { Empty::None } else if paired { Empty::StandIn } else { Empty::Wait };
            stand_ins.push(match (empty, field(&name)) {
                (Empty::None, _) | (_, None) => Json::Null,
                (_, Some(f)) => stand_in(m, f),
            });
            names.push((vec![p.name.clone()], Vec::new()));
            params.push(Param::Value(name, named, empty));
        }
    }
    let outputs: Vec<(String, String)> = facts.typescript.outputs.iter().map(|o| (back(&o.name), o.alias.clone())).collect();
    let code_of = |name: &str| facts.typescript.outputs.iter().find(|o| o.name == name).map(|o| o.alias.clone()).unwrap_or_else(|| name.to_string());
    // what each output reads, by the arguments: an input by its name, a field of an element by both
    let mut reads = Vec::new();
    for o in &facts.typescript.outputs {
        let mut taken: Vec<(usize, Vec<String>)> = Vec::new();
        for r in lf.reads.iter().find(|(n, _)| *n == o.name).map(|(_, r)| r.as_slice()).unwrap_or_default() {
            let (whole, col) = match r.split_once('.') {
                Some((el, c)) => (el, Some(c)),
                None => (r.as_str(), None),
            };
            let Some(i) = names.iter().position(|(ns, _)| ns.iter().any(|n| n == whole)) else { continue };
            let at = match taken.iter().position(|(j, _)| *j == i) {
                Some(at) => at,
                None => {
                    taken.push((i, Vec::new()));
                    taken.len() - 1
                }
            };
            if let Some(c) = col.and_then(|c| names[i].1.iter().find(|(n, _)| n == c)).map(|(_, code)| code.clone())
                && !taken[at].1.contains(&c)
            {
                taken[at].1.push(c);
            }
        }
        taken.sort();
        reads.push((o.alias.clone(), taken));
    }
    // a computed output that reads a field that may be empty has no value while it is empty
    let value_when = facts
        .typescript
        .outputs
        .iter()
        .filter_map(|o| {
            let f = field(&back(&o.name)).filter(|f| f.computed.is_some())?;
            let when: Vec<String> = crate::lower::empty_reads(m.s, f).iter().map(|g| g.name.ascii().to_string()).collect();
            (!when.is_empty()).then(|| (o.alias.clone(), when))
        })
        .collect();
    let end = |e: &Option<(String, ritsu_units::Rat)>| e.as_ref().filter(|(_, r)| r.is_int()).map(|(n, r)| (code_of(n), r.num));
    let held = crate::lower::form::held(m.s, lf)
        .into_iter()
        .map(|h| {
            let when = match (&h.rows, field(&h.field)) {
                (None, Some(f)) => crate::lower::empty_reads(m.s, f).iter().map(|g| g.name.ascii().to_string()).collect(),
                _ => Vec::new(),
            };
            Hold { lo: end(&h.lo), hi: end(&h.hi), field: h.field, rows: h.rows, said: h.said, when }
        })
        .collect();
    Some(Worked { function: facts.typescript.function.clone(), params, stand_ins, outputs, reads, value_when, held })
}

/// A value of the range of the field `f` as the API takes it, which the rule is given for it where
/// the output called does not read it: the one the lowered files take where it is empty
/// ([`crate::lower::stand_in`]), a choice's first value, no for yes or no.
fn stand_in(m: &crate::engine::Machine, f: &Field) -> Json {
    match &f.ty {
        TypeExpr::Num(u) => match crate::lower::stand_in(f) {
            crate::lower::words::Val::Num(r) => Some(r.div(crate::lower::words::step(u))).filter(|q| q.is_int()).map(|q| Json::int(q.num)).unwrap_or(Json::Null),
            crate::lower::words::Val::Day(d) => Json::int(i128::from(d)),
        },
        TypeExpr::Date => match crate::lower::stand_in(f) {
            crate::lower::words::Val::Day(d) => Json::str(crate::lex::date_text(d)),
            crate::lower::words::Val::Num(r) => Json::str(crate::lex::date_text(r.num as i64)),
        },
        TypeExpr::Choice => f.options.first().map(|o| Json::str(o.name.ascii())).unwrap_or(Json::Null),
        TypeExpr::ChoiceFrom(p) if p.len() == 2 => m.s.rule_facts(&p[0].text).and_then(|r| r.enums.iter().find(|e| e.name == p[1].text || e.public == p[1].text)).and_then(|e| e.values.first()).map(|v| Json::str(&v.public)).unwrap_or(Json::Null),
        TypeExpr::Bool => Json::Bool(false),
        _ => Json::str(""),
    }
}

/// The rows the rule is given for the rows field `f` where the output called does not read them:
/// as many as the field asks for at least (`count`), each with a value of the range of each field the
/// rule takes and no for whether a field is filled in (`cells`, by the rule's names of them as
/// [`Param::Rows`] holds them); and that value of each, which a row the form holds is given for a
/// field the output does not read.
fn rows_stand_in(m: &crate::engine::Machine, f: &Field, taken: &[(String, String, bool)]) -> Json {
    let cells = taken.iter().map(|(code, child, given)| {
        let v = if *given { Json::Bool(false) } else { f.children.iter().find(|c| c.name.ascii() == child).map(|c| stand_in(m, c)).unwrap_or(Json::Null) };
        (code.as_str(), v)
    });
    let least = crate::ports::field(f).lo.unwrap_or(0).max(0);
    Json::obj([("count", Json::int(least)), ("cells", Json::obj(cells))])
}

/// A field's label in the language `lang`: its `labels <lang>` text, else its title as written.
pub fn label(n: &Named, lang: &str) -> String {
    n.labels.iter().find(|(l, _)| l == lang).map(|(_, t)| t.clone()).unwrap_or_else(|| n.label.clone())
}

/// One step of a number of the type `ty` (what one integer on the wire counts), in the unit people
/// count in: a yen, a dollar and a kilogram for `money[JPY]`, `money[USD]` and `mass[kg]`, 1/100 of
/// a yen and of a dollar for `money[銭]` and `money[USDc]`, 1/4 of a percent for `rate[step 0.25%]`,
/// a percent for `rate` (DESIGN 13.1).
pub fn step_in_unit(ty: &TypeExpr) -> ritsu_units::Rat {
    let one = ritsu_units::Rat::int(1);
    let TypeExpr::Num(u) = ty else { return one };
    match &u.dim {
        ritsu_units::Dim::Money(_) => ritsu_units::table::money(&u.unit).map_or(one, |(_, f)| f),
        ritsu_units::Dim::Rate => crate::lower::words::step(u).mul(ritsu_units::Rat::int(100)),
        _ => one,
    }
}

/// The digits after the point a screen writes a number of the type with, in the unit people count
/// in: the fewest that write any number of its steps exactly (none for a whole unit, 2 for
/// `money[USDc]` and for `rate[step 0.25%]`, 1 for `rate[step 0.1%]` and `rate[step 2.5%]`).
pub fn decimals(ty: &TypeExpr) -> u32 {
    let mut den = step_in_unit(ty).den;
    let (mut twos, mut fives) = (0, 0);
    while den % 2 == 0 {
        den /= 2;
        twos += 1;
    }
    while den % 5 == 0 {
        den /= 5;
        fives += 1;
    }
    twos.max(fives)
}

/// One step of a number of the type, counted in the last of its [`decimals`]: 1 where a step is one
/// of that digit, 25 for `rate[step 0.25%]` (2 decimals), 25 for `rate[step 2.5%]` (1 decimal).
pub fn step(ty: &TypeExpr) -> i128 {
    let s = step_in_unit(ty).mul(ritsu_units::Rat::int(10i128.pow(decimals(ty))));
    if s.is_int() { s.num } else { 1 }
}

/// The ISO 4217 code a screen formats money in: the unit as written, `円` and `銭` as `JPY`, `USDc`
/// as `USD`. None for a number that is no money.
pub fn currency(ty: &TypeExpr) -> Option<String> {
    let TypeExpr::Num(u) = ty else { return None };
    let ritsu_units::Dim::Money(cur) = &u.dim else { return None };
    Some(if cur == "円" || u.unit == "円" { "JPY".to_string() } else { cur.clone() })
}

/// Whether a screen shows a number of the type with the basic catalog's formats, as it always
/// has: a number with no unit (`formatNumber`), and money counted in whole units of its currency
/// (`formatCurrency`). Any other number, counted in steps of a part of its unit, is shown with
/// ritsu's `formatSteps` (DESIGN 13.1).
pub fn basic_format(ty: &TypeExpr) -> bool {
    let TypeExpr::Num(u) = ty else { return true };
    match &u.dim {
        ritsu_units::Dim::Number => true,
        ritsu_units::Dim::Money(_) => step_in_unit(ty).is_int() && step_in_unit(ty).num == 1,
        _ => false,
    }
}

/// What a screen in `lang` writes after the digits of a number of the type, the space before it
/// included: ` USD` (and ` JPY`, `円` in Japanese), `%`, ` kg` (`kg` in Japanese); nothing for a
/// number with no unit.
pub fn unit_after(ty: &TypeExpr, lang: &str) -> String {
    let TypeExpr::Num(u) = ty else { return String::new() };
    match (&u.dim, currency(ty)) {
        (ritsu_units::Dim::Number, _) => String::new(),
        (ritsu_units::Dim::Rate, _) => "%".to_string(),
        (_, Some(c)) if c == "JPY" && lang == "ja" => "円".to_string(),
        (_, Some(c)) => format!(" {c}"),
        (_, None) if lang == "ja" => u.unit.clone(),
        (_, None) => format!(" {}", u.unit),
    }
}

/// `n` steps of a number of the type as a screen in `lang` writes them: its [`decimals`] after the
/// point, zeros too, a whole part of five digits or more grouped by commas, and the unit
/// ([`unit_after`]): `1.25 USD`, `1.25円`, `31.25%`, `-0.5%`, `12,000 kg` (`12,000kg` in
/// Japanese). What `formatSteps` writes in the screens, and the checks say.
pub fn amount_text(ty: &TypeExpr, n: i128, lang: &str) -> String {
    let d = decimals(ty);
    let v = n.saturating_mul(step(ty));
    let digits = format!("{:0>w$}", v.unsigned_abs(), w = d as usize + 1);
    let (whole, frac) = digits.split_at(digits.len() - d as usize);
    let whole = whole.parse::<i128>().map(|w| crate::lower::words::grouped(w).replace('_', ",")).unwrap_or_else(|_| whole.to_string());
    let sign = if v < 0 { "-" } else { "" };
    let point = if d > 0 { format!(".{frac}") } else { String::new() };
    format!("{sign}{whole}{point}{}", unit_after(ty, lang))
}

/// A label the screens show, for E703: its key as `labels` writes it, and whether a language has it.
struct Key<'a> {
    key: String,
    named: &'a Named,
}

/// Every label the screens of `k` show, in the order the file writes the things they name: the
/// request, the fields (a field's values and the fields of its rows after it), the stages, the ways.
fn keys(k: &Kind) -> Vec<Key<'_>> {
    fn field<'a>(f: &'a Fld, at: &str, out: &mut Vec<Key<'a>>) {
        let key = if at.is_empty() { f.named.written.clone() } else { format!("{at}.{}", f.named.written) };
        out.push(Key { key: key.clone(), named: &f.named });
        if matches!(f.ty, TypeExpr::Choice | TypeExpr::ChoiceFrom(_)) {
            for c in &f.choices {
                out.push(Key { key: format!("{key}.{}", c.written), named: c });
            }
        }
        for c in &f.fields {
            field(c, &key, out);
        }
    }
    let mut out = vec![Key { key: "request".to_string(), named: &k.named }];
    for f in &k.form {
        field(f, "", &mut out);
    }
    out.extend(k.stages.iter().map(|s| Key { key: format!("stage {}", s.written), named: s }));
    out.extend(k.ways.iter().map(|w| Key { key: format!("who {}", w.written), named: w }));
    out
}

/// What is said of a kind's screens before they are written: the code, the line, the words.
#[derive(Clone, Debug)]
pub struct Said {
    pub code: &'static str,
    pub line: usize,
    pub message: Text,
    pub notes: Vec<Text>,
}

/// E703: each language the file declares labels in that lacks a label the screens show, at its
/// `labels` line, with the keys it lacks.
pub fn labels_said(k: &Kind) -> Vec<Said> {
    let all: Vec<(String, Vec<String>)> = keys(k).into_iter().map(|x| (x.key, x.named.labels.iter().map(|(l, _)| l.clone()).collect())).collect();
    missing_labels(&all, &k.screen.declared, &k.screen.untitled, &k.language)
}

/// E703 of a request as `check` says it: what [`labels_said`] says of the kind `ritsu gen --target
/// requests` makes of the file, from the file alone ([`label_keys`]), so that a label the screens
/// lack is said where the file is checked, and not first when the screens are written.
pub fn labels_checked(s: &crate::names::Scope) -> Vec<Said> {
    let declared: Vec<(String, usize)> = s.file.labels.iter().map(|b| (b.lang.text.clone(), b.line)).collect();
    missing_labels(&label_keys(s), &declared, &untitled(s), crate::r#gen::written_in(s.file))
}

/// Every label the screens of a request show, as [`keys`] lists them of its kind, from the file: the
/// request, the fields (a field's values and the fields of its rows after it), the stages and the
/// ways, each with the languages that label it.
pub fn label_keys(s: &crate::names::Scope) -> Vec<(String, Vec<String>)> {
    let f = s.file;
    let langs = |is: &dyn Fn(&LabelKey) -> bool| -> Vec<String> { f.labels.iter().filter(|b| b.entries.iter().any(|e| is(&e.key))).map(|b| b.lang.text.clone()).collect() };
    let key_is = |words: &[crate::ast::Word], want: &[String]| words.len() == want.len() && words.iter().zip(want).all(|(w, x)| w.text == *x);
    fn walk(s: &crate::names::Scope, x: &Field, at: &[String], out: &mut Vec<(String, Vec<String>)>, langs: &dyn Fn(&dyn Fn(&LabelKey) -> bool) -> Vec<String>, key_is: &dyn Fn(&[crate::ast::Word], &[String]) -> bool) {
        let mut here = at.to_vec();
        here.push(x.name.text.clone());
        out.push((here.join("."), langs(&|k| matches!(k, LabelKey::Field(ws) if key_is(ws, &here)))));
        let values: Vec<(String, Option<String>)> = match &x.ty {
            TypeExpr::Choice => x.options.iter().map(|o| (o.name.text.clone(), None)).collect(),
            TypeExpr::ChoiceFrom(p) if p.len() == 2 => s
                .rule_facts(&p[0].text)
                .and_then(|r| r.enums.iter().find(|e| e.name == p[1].text || e.public == p[1].text))
                .map(|e| e.values.iter().map(|v| (v.name.clone(), Some(v.public.clone()))).collect())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        for (name, public) in values {
            let (mut by_name, mut by_ascii) = (here.clone(), here.clone());
            by_name.push(name.clone());
            by_ascii.push(public.unwrap_or_else(|| name.clone()));
            out.push((by_name.join("."), langs(&|k| matches!(k, LabelKey::Field(ws) if key_is(ws, &by_name) || key_is(ws, &by_ascii)))));
        }
        for c in &x.children {
            walk(s, c, &here, out, langs, key_is);
        }
    }
    let mut out = vec![("request".to_string(), langs(&|k| matches!(k, LabelKey::Request)))];
    for x in &f.form {
        walk(s, x, &[], &mut out, &langs, &key_is);
    }
    let word = |n: &crate::ast::Name| crate::ast::Word { text: n.text.clone(), span: n.span };
    let of = |labels: Vec<(String, String)>| labels.into_iter().map(|(l, _)| l).collect::<Vec<_>>();
    out.extend(s.stages().unwrap_or_default().into_iter().map(|n| (format!("stage {}", n.text), of(titled(s, &LabelKey::Stage(word(n)), &n.text).1))));
    out.extend(s.who_all().iter().map(|w| &w.who.name).map(|n| (format!("who {}", n.text), of(titled(s, &LabelKey::Who(word(n)), &n.text).1))));
    out
}

/// E703 of the labels a kind's screens show, each with the languages that label it: each language
/// the file declares labels in (`declared`, with its line) that lacks one, and what comes from
/// elsewhere with no title in the file's language `lang` (`untitled`, with the line it is said at).
fn missing_labels(all: &[(String, Vec<String>)], declared: &[(String, usize)], untitled: &[(String, usize)], lang: &str) -> Vec<Said> {
    let mut out = Vec::new();
    for (lang, line) in declared {
        let missing: Vec<String> = all.iter().filter(|(_, ls)| !ls.iter().any(|l| l == lang)).map(|(key, _)| format!("`{key}`")).collect();
        if missing.is_empty() {
            continue;
        }
        let (en, ja) = (missing.join(", "), missing.join("、"));
        let n = missing.len();
        let things = ritsu_base::text::plural(n, "thing", "things");
        out.push(Said {
            code: "E703",
            line: *line,
            message: tr!("`labels {lang}` に、画面に出る項目のうち {n} 個の文言がありません: {ja}", "`labels {lang}` has no label for {things} the screens show: {en}"),
            notes: vec![tr!("画面は、この言語の文言が無い所に、ファイルの言語の題を出してしまいます。", "Where it has none, a screen in this language would show the title as the file writes it.")],
        });
    }
    // the file's own language: what comes from elsewhere, said at each line it is given at
    let mut lines: Vec<usize> = Vec::new();
    for (_, l) in untitled {
        if !lines.contains(l) {
            lines.push(*l);
        }
    }
    for line in lines {
        let missing: Vec<String> = untitled.iter().filter(|(_, l)| *l == line).map(|(key, _)| format!("`{key}`")).collect();
        let (en, ja) = (missing.join(", "), missing.join("、"));
        let n = missing.len();
        let things = ritsu_base::text::plural(n, "thing", "things");
        // a line is a `use route` line, or the line of a field that chooses from a rule's enum
        let routed = untitled.iter().any(|(key, l)| *l == line && (key.starts_with("stage ") || key.starts_with("who ")));
        let note = if routed {
            tr!(
                "共有の経路のステップと決め方の題は、その経路のファイルの `labels {lang}` に書きます。無ければ、画面は経路が別の言語で書いた名前を出してしまいます。",
                "Title them in `labels {lang}` of the shared route's file: without it, a screen would show the names the route writes in another language."
            )
        } else {
            tr!(
                "規則の列挙から選ぶ値の題は、フィールドの下に `goods \"物品・サービス\"` のように書きます。規則がファイルの言語（`{lang}`）で書いてあれば、規則が値に書いた表示名も題になります。どちらも無ければ、画面は規則が書いた値の名前を出してしまいます。",
                "Title each value under the field, as `goods \"Goods and services\"`; where the rule is written in the file's language (`{lang}`), the display name it writes for a value titles it too. Without either, a screen would show the value's name as the rule writes it."
            )
        };
        out.push(Said {
            code: "E703",
            line,
            message: tr!("画面に出る項目のうち {n} 個に、ファイルの言語（`{lang}`）の題がありません: {ja}", "The screens show {things} with no title in the file's language (`{lang}`): {en}"),
            notes: vec![note],
        });
    }
    out
}

/// The largest whole number a screen holds exactly: a screen holds a number as JavaScript does
/// (`Number.MAX_SAFE_INTEGER`, 2^53 − 1).
pub const SAFE: i128 = (1 << 53) - 1;

/// E704: each number field whose range reaches past what a screen holds exactly, `SAFE` steps of
/// its unit either way, at its line: a field the requester gives, an approver's, a computed one and
/// a field of rows alike, for a screen shows, checks or sends each. Past it a value would be
/// rounded, and checked and sent as another.
pub fn numbers_said(k: &Kind) -> Vec<Said> {
    fn walk(f: &Fld, at: &str, out: &mut Vec<Said>) {
        let name = if at.is_empty() { f.named.written.clone() } else { format!("{at}.{}", f.named.written) };
        if let TypeExpr::Num(u) = &f.ty
            && let Some(end) = [f.min, f.max].into_iter().flatten().find(|n| n.abs() > SAFE)
        {
            let in_unit = |n: i128| crate::lower::words::number(ritsu_units::Rat::int(n).mul(crate::lower::words::step(u)), u);
            let (end, limit) = (in_unit(end), in_unit(SAFE));
            out.push(Said {
                code: "E704",
                line: f.line,
                message: tr!("`{name}` の範囲が `{end}` まであり、画面が正確に持てる数（`{limit}`）を越えます", "the range of `{name}` reaches `{end}`, past what a screen holds exactly (`{limit}`)"),
                notes: vec![tr!(
                    "画面は数を JavaScript と同じに持ち、正確なのは `-{limit}` から `{limit}` までです。それを越える値は丸まり、別の値として確かめて送ってしまいます。範囲をその中に狭めてください。",
                    "A screen holds a number as JavaScript does, exactly only from `-{limit}` to `{limit}`: a value past it would be rounded, and checked and sent as another. Narrow the range to within it."
                )],
            });
        }
        for c in &f.fields {
            walk(c, &name, out);
        }
    }
    let mut out = Vec::new();
    for f in &k.form {
        walk(f, "", &mut out);
    }
    out
}

/// Every file of `a2ui/` for the kinds, in the order written: each kind's six screens in English
/// and in Japanese, then the catalog and the functions; each with its path under `requests/`, its
/// text and the request it is written from. Err with why, when a kind's screens are not what they
/// send (a bug in ringi: what [`said`] says before).
pub fn files(kinds: &[Kind], given: &Given) -> Result<Vec<(String, String, Option<String>)>, Text> {
    let mut out = Vec::new();
    for k in kinds {
        for (screen, lang, j) in a2ui::screens(k, &given.catalog) {
            if let Some(b) = bindings::check(k, &j).into_iter().find(|b| b.code == "E701") {
                let (code, at) = (b.code, &b.path);
                return Err(tr!("{} の {screen}.{lang} が {code}（{at}）", "{}'s {screen}.{lang} gives {code} ({at})", k.named.name; k.named.name));
            }
            out.push((format!("a2ui/{}/{screen}.{lang}.json", k.named.name), format!("{}\n", j.pretty()), Some(k.file.clone())));
        }
    }
    out.push(("a2ui/catalog.json".to_string(), format!("{}\n", catalog::json(kinds, &given.catalog).pretty()), None));
    out.push(("a2ui/functions.js".to_string(), functions::module(kinds, given)?, None));
    Ok(out)
}

/// What is said of the screens of `k` before anything is written (DESIGN 13.3): E704 of the ranges
/// of its numbers, E703 of its labels, and E701 and W702 of each screen, at the request's first
/// line. The templates are made from the declarations, so an E701 or a W702 is a bug in ringi, and
/// says so.
pub fn said(k: &Kind, given: &Given) -> Vec<Said> {
    let mut out = numbers_said(k);
    out.extend(labels_said(k));
    for (screen, lang, j) in a2ui::screens(k, &given.catalog) {
        for b in bindings::check(k, &j) {
            let (file, at) = (format!("{screen}.{lang}.json"), &b.path);
            let (shown, sent) = b.why.split_once(" / ").unwrap_or_default();
            let message = match b.code {
                "E701" => tr!("画面 `{file}` のボタンが、画面に見せていない `{at}` を送ります", "A button of the screen `{file}` sends `{at}`, which the screen does not show"),
                _ => tr!("画面 `{file}` が、`{at}` を {shown} 刻みで見せますが、送るのは {sent} 刻みです", "The screen `{file}` shows `{at}` in steps of {shown}, and it is sent in steps of {sent}"),
            };
            out.push(Said {
                code: b.code,
                line: 1,
                message,
                notes: vec![tr!(
                    "テンプレートは宣言から作るので、申請を直す必要はありません。この申請と ritsu の版（`ritsu --version`）を添えて、ringi の不具合として知らせてください。",
                    "The templates are made from the declarations, so nothing in the request is to be corrected for it: report it as a bug in ringi, with this request and the version of ritsu (`ritsu --version`)."
                )],
            });
        }
    }
    out
}
