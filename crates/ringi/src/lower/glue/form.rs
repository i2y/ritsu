//! The form in a request's glue (DESIGN 4.3, 4.4, 12.1): the values a requester gives (`Given`,
//! each of them optional), the form the ticket holds (`Form`), each value held to what its field
//! declares as the reference interpreter holds it ([`crate::taken`]), what the form works out of them
//! by the copy of `form.rule` the generated code reads (`values.rule`, [`crate::lower::form::values`]),
//! the stages the route comes to for them (`formed`), the values a flow is given (`vals`), and the
//! form given again or edited (`reform`).
//!
//! The copy works out every computation over all the values what it reads can take, and the glue
//! holds each computed number to its range itself, in the order the door of `form.rule` holds them
//! ([`crate::lower::form::held`]): the computations before the rows are walked, those of a row (by
//! their most and their least over the rows), then the totals and what reads them. The refusal is
//! the reference interpreter's ([`crate::lower::form::out_of_range_said`]). `worked` answers what the
//! form works out and the first value outside its range at once, for a screen as much as for
//! `given`; whether a field is shown and asked for is read from the copy's outputs (`asked`).
//!
//! A value is held where kaname can narrow it: a range and a count with an `if` before the value is
//! put in the record that writes them. A choice is held as the text of its value, as the JSON carries
//! it, and given to a rule as the rule's enum (`choice_<field>`). Every refusal of the form is of the
//! kind `form`, as the reference interpreter's are, in the reference's words.
//!
//! The form a ticket holds (`Form`, and its rows, `<Rows>Row`) writes no range: the machine hands it
//! on through its parameters of a function type, where kaname keeps no range of it. The ranges are
//! held where a value is given (`<Rows>Checked` is a row held to them, which the copy reads), and
//! again where the route reads the form (`formed`), which narrows each value it reads with an `if`.
//!
//! A form given again by the requester (`resubmit`) or edited by an approver (`edit`) is the form the
//! ticket holds with each value given in its place, as the reference merges them: a value the event
//! gives is held to its field and to who gives it (an approvers' field is the approvers' alone, and an
//! approver edits only the fields `approvers edit` names), then the whole form is held again. A value
//! the event does not give stays as the ticket holds it: a value of `null` is read as none given.

mod worked;

use super::{not_yet, quoted};
use crate::ast::{Field, TicketLine, TypeExpr};
use crate::engine::Machine;
use crate::lower::words::{self, Names};
use crate::lower::{LFile, Part};
use ritsu_base::text::Text;
use ritsu_ports::{ColumnType, RuleFacts};
use ritsu_units::{Dim, Rat, Unit};

/// The place of the value in a check, until it is written over a name.
const X: &str = "\u{1}";

/// The place of the name of a field of a row in the words of its checks, until it is written over
/// its place in the row ([`placed`]).
const NAME: &str = "\u{3}";

/// The place of a row in the element `worked` gives the copy, until it is written over the name of
/// the row.
const ROW: &str = "\u{4}";

/// The expression of kaname `e` with `mark`, which it writes in its strings, written over `inner`,
/// what goes in a string there: the string split around it (`"`items[", text(no - 1), "].price` is
/// …"`), and a string alone made a `concat` of its parts.
fn splice(e: &str, mark: &str, inner: &str) -> String {
    if !e.contains(mark) {
        return e.to_string();
    }
    let out = e.replace(mark, inner);
    if e.starts_with('"') { format!("concat({out})") } else { out }
}

/// A check of a field of a row, the name in its words written over the field's place in the row:
/// `items[1].price`, the rows counted from 0 (`no` is the row's number, from 1), as the reference
/// says it and as `at` names it.
fn placed(o: Over, rows: &str, cell: &str) -> Over {
    let safe = |n: &str| n.replace('"', "'").replace('\n', " ");
    let inner = format!("{}[\", text(no - 1), \"].{}", safe(rows), safe(cell));
    Over { bad: o.bad, en: splice(&o.en, NAME, &inner), ja: splice(&o.ja, NAME, &inner), at: o.at }
}

/// A value a field does not take, as the glue holds it: the condition a value that is not taken
/// meets, and the refusal in English and in Japanese, each an expression of kaname over [`X`].
#[derive(Clone)]
struct Check {
    bad: String,
    said: String,
    said_ja: String,
}

impl Check {
    /// The check written over the value `v`, its refusal naming the place `at` (an expression of
    /// kaname: the field's name quoted, or the cell of a row).
    fn over(&self, v: &str, at: &str) -> Over {
        Over { bad: self.bad.replace(X, v), en: self.said.replace(X, v), ja: self.said_ja.replace(X, v), at: at.to_string() }
    }
}

/// A check written over a value: the condition a value that is not taken meets, the refusal in
/// English and in Japanese, and the place of the form it names (`at`, DESIGN 12.4), each an
/// expression of kaname.
#[derive(Clone)]
struct Over {
    bad: String,
    en: String,
    ja: String,
    at: String,
}

impl Over {
    /// The refusal of the form the check makes.
    fn refused(&self) -> String {
        format!("err(refused(text: {}, text_ja: {}, at: {}))", self.en, self.ja, self.at)
    }
}

/// The place of a field of the form, as a refusal names it: its ASCII name, quoted.
fn at_field(name: &str) -> String {
    quoted(name)
}

/// The place of a cell of a row, as a refusal names it: `items[1].price`, the rows counted from 0
/// (`no` is the row's number, from 1, and at most one past the most rows the glue walks).
fn at_cell(rows: &str, cell: &str) -> String {
    format!("concat({}, text(no - 1), {})", quoted(&format!("{rows}[")), quoted(&format!("].{cell}")))
}

/// What a value of a field is, as the JSON of a flow's arguments holds it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Vk {
    /// A number: the whole number of its steps.
    Num,
    /// A text, a choice's value, a person, a team, an attachment.
    Str,
    /// A date, as its text.
    Date,
    Bool,
    Rows,
}

/// A field of the form as the glue reads it.
pub(super) struct Fld {
    /// Its ASCII name: the field of the records, and the key of the JSON.
    pub(super) name: String,
    /// The name a function of the glue takes its value by (a parameter, a `let`): its ASCII name,
    /// unless the glue reads the name as something else in the same place (`g`, a `use` line's
    /// name), when `_value` follows it ([`FormOf::name_locals`]).
    local: String,
    /// Its name as written, which a refusal says.
    pub(super) shown: String,
    /// The type of its value in kaname.
    pub(super) ty: String,
    kind: Kind,
    pub(super) vk: Vk,
    /// The unit of a number.
    pub(super) unit: Option<Unit>,
    /// Whether the form holds a value of it for every request: no `optional`, no condition, not the
    /// approvers' (and, in a row, not an attachment). Else its type in the form is optional.
    pub(super) required: bool,
    /// Whether only the approvers give it: the requester gives it no value.
    approvers: bool,
    optional: bool,
    shown_when: bool,
    required_when: bool,
    /// What a value given is held to besides its range: its length, a choice's values, a master's
    /// row, the organization's person or team.
    checks: Vec<Check>,
    /// Whether holding a value reads the part of the organization.
    org: bool,
    /// The range of a number or a date, as the record writes it (`range >0 <10`), and its check.
    range: Option<(String, Check)>,
    /// What a date `open in` a calendar is held to after its range: a day outside the calendar's
    /// data, then a day it is closed on; and the calendar, which the glue reads a file of koyomi's
    /// of (`opened_<calendar>.cal`).
    open: Vec<Check>,
    opened: Option<Opened>,
    /// The values of a choice, as the JSON names them; with the rule and its enum when the field is a
    /// `choice from` a rule's enum.
    choice: Option<(Vec<String>, Option<(String, String)>)>,
    /// The names the glue declares for the field ([`FormOf::name_functions`]).
    fns: Fns,
}

/// The names the glue declares for a field, each made of the field's name (`given_amount`,
/// `cell_lines_price`, `LinesRow`), unless another name the glue declares is the same, or the same
/// in Pascal case (`given_a_b` and `given_aB` are both `GivenAB`, which kaname names a type of the
/// generated code after), when `_value` follows it ([`FormOf::name_functions`]).
#[derive(Clone, Debug, Default)]
struct Fns {
    /// `given_<field>`: the value given, held to what the form declares of it.
    given: String,
    /// `regiven_<field>`: the value given again by a resubmit or an edit.
    regiven: String,
    /// `val_<field>` (`val_<rows>_<field>` for a field of a row): a value that may be empty as a
    /// flow's arguments.
    val: String,
    /// `cell_<rows>_<field>`: a field of a row held to what the form declares of it.
    cell: String,
    /// `choice_<field>`: a choice as the enum of `choices.rule`.
    choice: String,
    /// `choice_<field>_<rule>`: a `choice from` a rule's enum as that rule's enum.
    choice_from: String,
    /// The functions of rows: `row_vals_<rows>`, `row_<rows>`, `add_<rows>`, `no_<rows>` and
    /// `held_<rows>`.
    row_vals: String,
    row: String,
    add: String,
    no: String,
    held: String,
    /// The records of rows: as given (`LinesGiven`), as held to what the form declares
    /// (`LinesChecked`), and a row as the ticket holds it (`LinesRow`).
    given_rec: String,
    checked_rec: String,
    row_rec: String,
}

impl Fns {
    /// The names of a field of the form, `rows` the ASCII name of the rows it is a field of, each
    /// taken from `names` in turn; `from` the alias the glue names the rule of a `choice from` by.
    fn taken(names: &mut Names, x: &Fld, rows: Option<&str>, from: Option<&str>) -> Fns {
        let n = &x.name;
        let mut f = Fns::default();
        match (rows, &x.kind) {
            (Some(r), _) => {
                f.cell = names.take(&format!("cell_{r}_{n}"));
                f.val = names.take(&format!("val_{r}_{n}"));
            }
            (None, Kind::Rows { .. }) => {
                let (given, checked) = row_records(n);
                f.given_rec = names.take(&given);
                f.checked_rec = names.take(&checked);
                f.row_rec = names.take(&row_held(n));
                f.given = names.take(&format!("given_{n}"));
                f.regiven = names.take(&format!("regiven_{n}"));
                f.held = names.take(&format!("held_{n}"));
                f.row = names.take(&format!("row_{n}"));
                f.add = names.take(&format!("add_{n}"));
                f.no = names.take(&format!("no_{n}"));
                f.row_vals = names.take(&format!("row_vals_{n}"));
            }
            (None, Kind::Given) => {
                f.given = names.take(&format!("given_{n}"));
                f.regiven = names.take(&format!("regiven_{n}"));
                f.val = names.take(&format!("val_{n}"));
                if x.choice.is_some() {
                    f.choice = names.take(&format!("choice_{n}"));
                }
                if let Some(a) = from {
                    f.choice_from = names.take(&format!("choice_{n}_{a}"));
                }
            }
            (None, Kind::Computed { .. } | Kind::Counted) => f.val = names.take(&format!("val_{n}")),
        }
        f
    }
}

impl Fld {
    /// A value of the field as the JSON holds it, a number the whole number of its steps
    /// (`form.amount / 1JPY`, `form.hours / 0.5h`), `v` being the value in kaname.
    pub(super) fn steps(&self, v: &str) -> String {
        match &self.unit {
            // a rate over a rate is a rate in kaname: a function of the glue gives its steps
            Some(u) if u.dim == Dim::Rate => format!("rate_steps_{}(v: {v})", rate_tag(u)),
            Some(u) if u.dim != Dim::Number || !words::step(u).is_int() || words::step(u).num != 1 => format!("{v} / {}", words::number(words::step(u), u)),
            _ => v.to_string(),
        }
    }

    /// What follows the field's type where the glue writes no range of the field's: for a rate,
    /// every value the JSON holds ([`rate_open`]).
    fn open(&self) -> String {
        rate_open(self.unit.as_ref())
    }

    /// A number of the field in its unit, as a number of kaname (`form.amount / 1JPY`): what an
    /// argument of a book's transfer is counted from.
    pub(super) fn in_unit(&self, v: &str) -> String {
        match &self.unit {
            Some(u) if u.dim != Dim::Number => format!("{v} / {}", words::number(Rat::int(1), u)),
            _ => v.to_string(),
        }
    }

    /// The least a number of the field moves by, in its unit: its step.
    pub(super) fn step(&self) -> Rat {
        self.unit.as_ref().map(words::step).unwrap_or(Rat::int(1))
    }

    /// Whether its unit is a rate, which a book's amount is not counted from.
    pub(super) fn is_rate(&self) -> bool {
        self.unit.as_ref().is_some_and(|u| u.dim == Dim::Rate)
    }

    /// The check of a value outside the range of a number or a date, given or worked out.
    fn range_check(&self) -> Option<&Check> {
        match &self.kind {
            Kind::Computed { range } => range.as_ref().map(|(_, c)| c),
            _ => self.range.as_ref().map(|(_, c)| c),
        }
    }
}

/// A calendar a date of the form is `open in`, as the glue reads it: its alias in ASCII, the path of
/// its file from the lowered directory, and the days a date of the form is read over in it (the
/// field's range within the calendar's data).
#[derive(Clone)]
struct Opened {
    /// The word the calendar's file is named by (`opened_<word>.cal`; [`crate::lower::UsedCal::file`]).
    file: String,
    /// The alias the glue reads the file by (`opened_office`).
    alias: String,
    calendar: String,
    path: String,
    lo: i64,
    hi: i64,
}

enum Kind {
    Given,
    /// Rows: the fields of a row a person gives, and the count of the rows as the record writes it
    /// (`count >=2 <=3`), with its checks, the limit first.
    Rows { children: Vec<Fld>, count: Option<(String, Vec<Check>)> },
    /// A number the form works out, with its range in the form (what it can come to within the range
    /// it writes, as the route reads it) and the refusal of a value outside it.
    Computed { range: Option<(String, Check)> },
    /// A date the form counts in business days (`form.cal`), held to its range and its calendar as
    /// a date given is.
    Counted,
}

/// The form of a request as its glue reads it: its fields, the copy of its computations when it has
/// one, the lowered route, and the rules the route reads an output of.
pub(super) struct FormOf<'x, 's, 'a> {
    m: &'x Machine<'s, 'a>,
    fields: Vec<Fld>,
    /// `values.rule`, with what rulec knows of it and the numbers held to their ranges.
    values: Option<(&'x LFile, RuleFacts, Vec<crate::lower::form::Held>)>,
    /// `form.cal`, with what koyomi knows of it: the dates the form counts in business days.
    dates: Option<(&'x LFile, ritsu_ports::DateFacts)>,
    /// The calendars the dates of the form are `open in`, each read over the days of all of them.
    opened: Vec<Opened>,
    /// What rulec knows of `route.rule`.
    route: Option<RuleFacts>,
    /// The rules the route reads an output of: the alias the request uses each by, the alias the
    /// glue names it by, its path from the lowered directory, and what rulec knows of it.
    used: Vec<(String, String, String, RuleFacts)>,
    /// The names of the glue's `use` lines and of its functions for each calendar, taken so far: the
    /// names it declares whatever the request is ([`super::DECLARED`]), the aliases of the rules
    /// above and of the calendars' open days, then the deadlines' and the calendars' business days
    /// (`terms_1`, `open_1`, `open_office`), which the glue takes after them.
    pub(super) aliases: Names,
    /// The names the glue declares at the top of its module, each name it takes for a field and for
    /// the route's helpers (`out_…`, `way_of_…`) free of the others ([`FormOf::name_functions`]).
    names: std::cell::RefCell<Names>,
    /// The functions a value that may be empty is read through (`in_…`, `fill_…`, `choice_opt_…`),
    /// each made the first time something reads it: what it is of, its name, and its text until it
    /// is written out ([`FormOf::helpers`]).
    helpers: std::cell::RefCell<Vec<(String, String, Option<String>)>>,
}

/// The name of a record of the glue for a rows field: `lines` is `Lines`, `due_items` `DueItems`.
fn pascal(ascii: &str) -> String {
    let p: String = ascii.split('_').filter(|p| !p.is_empty()).map(|p| p[..1].to_ascii_uppercase() + &p[1..]).collect();
    if p.is_empty() { "Rows".to_string() } else { p }
}

/// The records of the rows of `lines`: as given (`LinesGiven`) and as held to what the form declares
/// (`LinesChecked`).
fn row_records(rows: &str) -> (String, String) {
    let p = pascal(rows);
    (format!("{p}Given"), format!("{p}Checked"))
}

/// The record of a row of `lines` as the ticket holds it (`LinesRow`), its fields of no range.
fn row_held(rows: &str) -> String {
    format!("{}Row", pascal(rows))
}

/// Text lines, each moved right by `pad` spaces.
fn indent(text: &str, pad: usize) -> String {
    text.lines().map(|l| if l.is_empty() { "\n".to_string() } else { format!("{}{l}\n", " ".repeat(pad)) }).collect()
}

/// `if <bad> err(…) else if … else <then>`: each check over the value it is written over, the first
/// a value meets refusing it.
fn chain(checks: &[Over], then: &str, pad: usize) -> String {
    chain_to(checks, then, pad, &Over::refused)
}

/// [`chain`], each check a value meets ending in what `bad` makes of it.
fn chain_to(checks: &[Over], then: &str, pad: usize, bad: &dyn Fn(&Over) -> String) -> String {
    if checks.is_empty() {
        return indent(then, pad);
    }
    let p = " ".repeat(pad);
    let mut out = String::new();
    for (i, c) in checks.iter().enumerate() {
        let kw = if i == 0 { "if" } else { "else if" };
        out.push_str(&format!("{p}{kw} {}\n{p}  {}\n", c.bad, bad(c)));
    }
    out.push_str(&format!("{p}else\n"));
    out.push_str(&indent(then, pad + 2));
    out
}

/// The refusal's words around a value, as kaname writes them: `concat("`", text(x), "` is …")`.
fn around(before: &str, value: &str, after: &str) -> String {
    format!("concat({}, {value}, {})", quoted(before), quoted(after))
}

/// The range of a number or a date: as the record writes it, and the check of a value outside it,
/// in the reference's words (`taken::holds`). None for a field that writes none.
fn range_check(f: &Field, shown: &str) -> Option<(String, Check)> {
    let b = f.range.as_ref()?;
    let mut parts = Vec::new();
    for (bound, lo) in [(b.lo.as_ref(), true), (b.hi.as_ref(), false)] {
        let Some(bound) = bound else { continue };
        let v = words::constant_in(&bound.value, &f.ty)?;
        let op = match (lo, bound.inclusive) {
            (true, true) => "<",
            (true, false) => "<=",
            (false, true) => ">",
            (false, false) => ">=",
        };
        parts.push(format!("{X} {op} {}", words::written(v, &f.ty)));
    }
    if parts.is_empty() {
        return None;
    }
    let r = words::range_text(f);
    // kaname's `text` writes no rate, and no whole number's digits in threes: functions of the glue
    // write them as the reference does (`12_345%`, `1_000_001JPY`)
    let value = match &f.ty {
        TypeExpr::Num(u) if u.dim == Dim::Rate => format!("rate_text_{}(v: {X})", rate_tag(u)),
        TypeExpr::Num(u) => format!("grouped(t: text({X}), unit: {})", quoted(if u.dim == Dim::Number { "" } else { &u.unit })),
        _ => format!("text({X})"),
    };
    let said = around("`", &value, &format!("` is outside the range of `{shown}` (`{r}`)"));
    let said_ja = around("`", &value, &format!("` は、`{shown}` の範囲（`{r}`）の外です"));
    Some((format!("range {r}"), Check { bad: parts.join(" or "), said, said_ja }))
}

/// The count of a text or of rows, as written (`count >=1 <=100`), and the conditions a value outside
/// it meets, the limit first, in the reference's words.
fn count_checks(s: &crate::names::Scope, f: &Field, shown: &str, rows: bool) -> Option<(String, Vec<Check>)> {
    let b = f.count.as_ref()?;
    let (lo, hi) = crate::taken::count_ends(b);
    let c = crate::types::written(s.src, b.span);
    let len = format!("{X}.count()");
    let said = |end: &str| {
        if rows {
            around(&format!("`{shown}` has "), &format!("machine.rows_many(n: {len})"), &format!(", {end} of its rows (`{c}`)"))
        } else {
            format!("concat(\"`\", {X}, \"` is \", machine.chars_long(n: {len}), {})", quoted(&format!(", {end} of `{shown}` (`{c}`)")))
        }
    };
    let said_ja = |end: &str| {
        if rows {
            around(&format!("`{shown}` は"), &format!("text({len})"), &format!("行で、行の数の{end}"))
        } else {
            format!("concat(\"`\", {X}, \"` は\", text({len}), {})", quoted(&format!("文字で、`{shown}` の{end}")))
        }
    };
    let mut out = Vec::new();
    if let Some(h) = hi.filter(|h| h.is_int()) {
        out.push(Check { bad: format!("{len} > {}", words::grouped(h.num)), said: said("past the limit"), said_ja: said_ja(&format!("上限（`{c}`）を超えます")) });
    }
    if let Some(l) = lo.filter(|l| l.is_int() && l.num > 0) {
        out.push(Check { bad: format!("{len} < {}", words::grouped(l.num)), said: said("short of the least"), said_ja: said_ja(&format!("下限（`{c}`）に足りません")) });
    }
    Some((c, out))
}

/// One part of a list of values: one value, or a list.
enum Seg {
    One(String),
    Many(String),
}

/// The parts joined into one list: the values next to each other written in one list, the lists
/// concatenated after it.
fn joined(segs: Vec<Seg>) -> String {
    let mut lists: Vec<String> = Vec::new();
    let mut ones: Vec<String> = Vec::new();
    for s in segs {
        match s {
            Seg::One(x) => ones.push(x),
            Seg::Many(x) => {
                if !ones.is_empty() {
                    lists.push(format!("[{}]", ones.join(", ")));
                    ones.clear();
                }
                lists.push(x);
            }
        }
    }
    if !ones.is_empty() {
        lists.push(format!("[{}]", ones.join(", ")));
    }
    match lists.split_first() {
        None => "[]".to_string(),
        Some((first, [])) => first.clone(),
        Some((first, rest)) => format!("{first}.concat({})", rest.join(", ")),
    }
}

/// A value of the field as a flow's argument (`machine.Arg`), `v` being the value in kaname.
pub(super) fn arg_of(x: &Fld, v: &str) -> String {
    let n = quoted(&x.name);
    match x.vk {
        Vk::Num => format!("machine.arg_num(name: {n}, v: {})", x.steps(v)),
        Vk::Str => format!("machine.arg_str(name: {n}, v: {v})"),
        Vk::Date => format!("machine.arg_str(name: {n}, v: text({v}))"),
        Vk::Bool => format!("machine.arg_truth(name: {n}, v: {v})"),
        Vk::Rows => format!("machine.arg_rows(name: {n}, v: {v}.map(y => {}(r: y)))", x.fns.row_vals),
    }
}

/// A function that gives a value of a field that may be empty as a flow's arguments: none of it
/// when it is not filled in, as the reference gives none.
fn val_fn(name: &str, x: &Fld) -> String {
    format!("fn {name}(v: {}?{}) -> list[machine.Arg] total\n  match v\n    none => []\n    some x => [{}]\n\n", x.ty, x.open(), arg_of(x, "x"))
}

impl<'x, 's, 'a> FormOf<'x, 's, 'a> {
    /// The form of the request the machine is of. Err with what of it the glue does not lower yet.
    pub(super) fn of(m: &'x Machine<'s, 'a>) -> Result<Self, Text> {
        let s = m.s;
        // the names of the `use` lines of what the form reads, taken before the rest: the rules the
        // route reads an output of, each by its alias, then for each calendar a date is `open in`,
        // the days it is open on (`opened_office`); none of them a word kaname reserves, a name the
        // glue declares itself (`given`, `values`, `open_after`), another of them, or the same as
        // another in Pascal case
        let mut aliases = Names::kaname();
        for n in super::DECLARED {
            aliases.keep(n);
        }
        let mut reads: Vec<(String, String)> = Vec::new();
        for (r, col, _) in m.lowered.route.iter().flat_map(|r| &r.inputs) {
            // a rule the route applies is the route's to work out (DESIGN 6.4)
            if m.lowered.route.as_ref().is_some_and(|n| n.worked.contains(col)) {
                continue;
            }
            let [alias, _] = r.as_slice() else { continue };
            if !reads.iter().any(|u| u.0 == *alias) {
                reads.push((alias.clone(), aliases.take(&words::alias_ascii(s, alias))));
            }
        }
        let cx = crate::lower::context(s, m.suite);
        let mut opened_as: Vec<(String, String)> = Vec::new();
        for f in s.fields().iter().flat_map(|f| std::iter::once(f).chain(&f.children)) {
            let Some(c) = &f.open_in else { continue };
            let Some(used) = cx.calendars.iter().find(|u| u.alias == c.text) else { continue };
            if !opened_as.iter().any(|(a, _)| *a == c.text) {
                opened_as.push((c.text.clone(), aliases.take(&format!("opened_{}", used.file))));
            }
        }
        let mut fields = Vec::new();
        for f in s.fields() {
            fields.push(field(m, f, None, &opened_as)?);
        }
        let facts_of = |lf: &LFile| -> Result<RuleFacts, Text> {
            let Some(r) = &m.suite.rules else { return Err(not_yet("a form, with no rulec joined".to_string())) };
            m.fs(|| r.facts(&m.lowered.path(lf))).map_err(|_| not_yet(format!("the form, whose `{}` does not read", lf.name)))
        };
        let values = match m.lowered.files.iter().find(|f| f.part == Part::Values) {
            Some(lf) => Some((lf, facts_of(lf)?, crate::lower::form::held(s, lf))),
            None => None,
        };
        let route = match m.lowered.files.iter().find(|f| f.part == Part::Route) {
            Some(lf) => Some(facts_of(lf)?),
            None => None,
        };
        let dates = match m.lowered.files.iter().find(|f| f.part == Part::FormDates) {
            Some(lf) => {
                let Some(d) = &m.suite.dates else { return Err(not_yet("a date the form counts, with no koyomi joined".to_string())) };
                Some((lf, m.fs(|| d.facts(&m.lowered.path(lf))).map_err(|_| not_yet("the dates the form counts, whose `form.cal` does not read".to_string()))?))
            }
            None => None,
        };
        if fields.iter().any(|x| matches!(x.kind, Kind::Counted)) && dates.is_none() {
            return Err(not_yet("a date the form counts that is not lowered to `form.cal`".to_string()));
        }
        // each calendar a date is open in, over the days of every date of it
        let mut opened: Vec<Opened> = Vec::new();
        let rows = fields.iter().flat_map(|x| match &x.kind {
            Kind::Rows { children, .. } => children.iter().collect::<Vec<_>>(),
            _ => Vec::new(),
        });
        for o in fields.iter().chain(rows).filter_map(|x| x.opened.clone()) {
            match opened.iter_mut().find(|y| y.file == o.file) {
                Some(y) => {
                    y.lo = y.lo.min(o.lo);
                    y.hi = y.hi.max(o.hi);
                }
                None => opened.push(o),
            }
        }
        // the rules the route reads an output of, each by the alias taken for it above
        let mut used: Vec<(String, String, String, RuleFacts)> = Vec::new();
        let dir = ritsu_base::paths::opened(&m.lowered.dir);
        for (alias, ascii) in reads {
            let Some(rules) = &m.suite.rules else { return Err(not_yet("a route, with no rulec joined".to_string())) };
            let Some(p) = s.other_path(&alias) else { return Err(not_yet(format!("the rule `{alias}`, which is not read"))) };
            let facts = m.fs(|| rules.facts(p)).map_err(|_| not_yet(format!("the rule `{alias}`, which does not read")))?;
            let rel = ritsu_base::paths::between(&dir, &ritsu_base::paths::opened(p));
            used.push((alias, ascii, rel, facts));
        }
        let mut form = FormOf { m, fields, values, dates, opened, route, used, aliases, names: std::cell::RefCell::new(Names::kaname()), helpers: std::cell::RefCell::new(Vec::new()) };
        form.name_functions();
        Ok(form)
    }

    /// The names the glue declares for each field ([`Fns`]), taken in the order of the form, after
    /// the names it declares whatever the request is ([`super::DECLARED`]) and the names of its `use`
    /// lines: each as it is made where that is free, else with `_value` after it, so that no two are
    /// the same, or the same in Pascal case (a field `x_y` and a field `y` of the rows `x` both make
    /// `val_x_y`; `given_a_b` and `given_aB` are both `GivenAB` in the generated code). The type of
    /// rows is the record named so.
    fn name_functions(&mut self) {
        let mut names = Names::kaname();
        for n in super::DECLARED {
            names.keep(n);
        }
        for n in self.use_names() {
            names.keep(&n);
        }
        let used = &self.used;
        for x in &mut self.fields {
            let from = x.choice.as_ref().and_then(|(_, f)| f.as_ref()).and_then(|(rule, _)| used.iter().find(|u| u.0 == *rule)).map(|u| u.1.clone());
            x.fns = Fns::taken(&mut names, x, None, from.as_deref());
            let r = x.name.clone();
            if let Kind::Rows { children, .. } = &mut x.kind {
                for c in children.iter_mut() {
                    c.fns = Fns::taken(&mut names, c, Some(&r), None);
                }
                x.ty = format!("list[{}]", x.fns.checked_rec);
            }
        }
        self.names = std::cell::RefCell::new(names);
    }

    /// The names the functions of the glue take the values of the form by: each field's ASCII name,
    /// unless a function that takes the field's value reads the name as something else (its
    /// parameters `g`, `org`, `form` and `approver`, the rows' `y`) or the glue reads a file by it
    /// (`uses`, the names of its `use` lines), when `_value` follows it. The name given in place of
    /// another is no field's ASCII name. (No field is named with a word kaname reserves: E002.)
    pub(super) fn name_locals(&mut self, uses: &[String]) {
        let all: std::collections::BTreeSet<String> = self.fields.iter().map(|x| x.name.clone()).collect();
        let mut names = Names::kaname().avoiding(&all);
        for n in ["g", "org", "form", "approver", "y"].iter().copied().chain(uses.iter().map(String::as_str)) {
            names.keep(n);
        }
        for x in &mut self.fields {
            x.local = names.take(&x.name);
        }
    }

    /// The names of the `use` lines of what the form reads.
    pub(super) fn use_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.values.is_some() {
            out.push("values".to_string());
        }
        out.extend(self.used.iter().map(|u| u.1.clone()));
        if self.dates.is_some() {
            out.push("form_dates".to_string());
        }
        out.extend(self.opened.iter().map(|o| o.alias.clone()));
        out
    }

    /// The `use` lines of what the form reads: the copy of its computations, and the rules the route
    /// reads an output of.
    pub(super) fn uses(&self) -> String {
        let mut out = String::new();
        if self.values.is_some() {
            out.push_str("use rule values from \"values.rule\"\n");
        }
        for (_, ascii, rel, _) in &self.used {
            out.push_str(&format!("use rule {ascii} from \"{rel}\"\n"));
        }
        if let Some((lf, _)) = &self.dates {
            out.push_str(&format!("use dates form_dates from \"{}\"\n", lf.name));
        }
        for o in &self.opened {
            out.push_str(&format!("use dates {} from \"opened_{}.cal\"\n", o.alias, o.file));
        }
        out
    }

    /// The files of koyomi's the form reads besides the lowered ones, each its name in the lowered
    /// directory and its text: for each calendar a date of the form is `open in`, the day a date
    /// comes to as the calendar's next open day (the date itself where the calendar is open on it).
    pub(super) fn files(&self) -> Vec<(String, String)> {
        let name = self.m.s.file.name.ascii();
        self.opened
            .iter()
            .map(|o| {
                (
                    format!("opened_{}.cal", o.file),
                    format!(
                        "dates {file} v1\ndescription \"The days the calendar `{c}` is open on, as the request {name} holds the dates of its form that are `open in` it: the day itself, or the next day the calendar is open on where it is closed\"\nuse calendar \"{}\"\n\ninputs\n  given : date  range >={} <={}\n\ndate opened = given\n  + 0 business days\n",
                        o.path,
                        crate::lex::date_text(o.lo),
                        crate::lex::date_text(o.hi),
                        file = words::koyomi_file(&format!("{name}_opened_{}", o.file)),
                        c = o.calendar
                    ),
                )
            })
            .collect()
    }

    /// A field at the top of the form, by its name as written or its ASCII name.
    pub(super) fn fld(&self, name: &str) -> Option<&Fld> {
        let s = self.m.s;
        let f = s.field(name).or_else(|| s.fields().iter().find(|f| f.name.ascii() == name))?;
        self.fields.iter().find(|x| x.name == f.name.ascii())
    }

    /// A function a value that may be empty is read through, made the first time something reads it
    /// (`key` says what it is of), under a name taken from `base` free of the others; its name.
    fn helper(&self, key: String, base: &str, text: impl FnOnce(&str) -> String) -> String {
        if let Some((_, n, _)) = self.helpers.borrow().iter().find(|(k, ..)| *k == key) {
            return n.clone();
        }
        let n = self.names.borrow_mut().take(base);
        let t = text(&n);
        self.helpers.borrow_mut().push((key, n.clone(), Some(t)));
        n
    }

    /// The functions made for the values that may be empty ([`FormOf::helper`]) not written out yet.
    pub(super) fn helpers(&self) -> String {
        self.helpers.borrow_mut().iter_mut().filter_map(|(_, _, t)| t.take()).collect()
    }

    /// `v`, a value of the field `x` that may be empty, as a value of its range or none
    /// (`in_<field>`): the value was held to the range where it was given or worked out, so the
    /// none of a value outside it never comes. `v` itself for a field of no range.
    fn in_range(&self, x: &Fld, rows: Option<&str>, v: &str) -> String {
        let Some(c) = x.range_check() else { return v.to_string() };
        let range = match &x.kind {
            Kind::Computed { range: Some((r, _)) } => r.clone(),
            _ => x.range.as_ref().map(|(r, _)| r.clone()).unwrap_or_default(),
        };
        let whole = rows.map_or_else(|| x.name.clone(), |r| format!("{r}_{}", x.name));
        let (ty, open, bad, shown) = (&x.ty, x.open(), c.bad.replace(X, "y"), &x.shown);
        let n = self.helper(format!("in {whole}"), &format!("in_{whole}"), |n| {
            format!("fn {n}(v: {ty}?{open}) -> {ty}?  {range} total\n  description \"A value of `{shown}` that may be empty, within its range\"\n  match v\n    none => none\n    some y =>\n      if {bad}\n        none\n      else\n        y\n\n")
        });
        format!("{n}(v: {v})")
    }

    /// `v`, a value of the field `x` (written `f`) that may be empty that a computation reads, as a
    /// value of its range (`fill_<field>`): itself, or where it is empty (or, which never comes,
    /// outside its range) a value of its range no answer reads, which the rule takes with whether
    /// the field is filled in (DESIGN 4.9), as the reference gives it.
    fn filled(&self, x: &Fld, f: &Field, rows: Option<&str>, v: &str) -> String {
        let whole = rows.map_or_else(|| x.name.clone(), |r| format!("{r}_{}", x.name));
        let at = crate::lower::stand_in(f);
        let stand = match &f.ty {
            TypeExpr::Bool => "false".to_string(),
            _ => words::written(at, &f.ty),
        };
        let (ty, open, shown) = (&x.ty, x.open(), &x.shown);
        let ret = match x.range.as_ref() {
            Some((r, _)) => format!("{ty}  {r}"),
            None => format!("{ty}{open}"),
        };
        // a number with no unit is a whole number to the glue's Rust, where a `match` that answers a
        // constant for none and the value for a value is what clippy reads as `unwrap_or` and asks to
        // be written so (`manual_unwrap_or`): it answers the value narrowed by an `if`, which kaname
        // does not warn of where it answers a number (its W501)
        let plain = matches!(&f.ty, TypeExpr::Num(u) if u.dim == ritsu_units::Dim::Number);
        let body = match x.range_check() {
            Some(c) if plain => format!("match v\n    none => {stand}\n    some y =>\n      if {}\n        {stand}\n      else\n        y\n", c.bad.replace(X, "y")),
            // the value within its range, or none: a `match` that only splits none from a value
            _ => format!("match {}\n    none => {stand}\n    some y => y\n", self.in_range(x, rows, "v")),
        };
        let n = self.helper(format!("fill {whole}"), &format!("fill_{whole}"), |n| {
            format!("fn {n}(v: {ty}?{open}) -> {ret} total\n  description \"A value of `{shown}` that a computation reads, a value of its range where it is empty, which no answer reads\"\n  {body}\n")
        });
        format!("{n}(v: {v})")
    }

    /// `v`, a choice that may be empty, as the rule's enum or none (`choice_opt_<field>`).
    fn choice_opt(&self, x: &Fld, v: &str) -> Result<String, Text> {
        let ty = self.choice_type(x).ok_or_else(|| not_yet(format!("the choice `{}`, which no lowered file reads", x.shown)))?;
        let one = &x.fns.choice;
        let n = self.helper(format!("choice_opt {}", x.name), &format!("choice_opt_{}", x.name), |n| format!("fn {n}(v: string?) -> {ty}? total\n  match v\n    none => none\n    some y => {one}(x: y)\n\n"));
        Ok(format!("{n}(v: {v})"))
    }

    /// The enum a choice is given to the lowered files as (`values.kind`, `route.kind`): the one
    /// `choice_<field>` gives ([`FormOf::choices`]).
    fn choice_type(&self, x: &Fld) -> Option<String> {
        let s = self.m.s;
        let cx = crate::lower::context(s, self.m.suite);
        let f = s.fields().iter().find(|f| f.name.ascii() == x.name)?;
        let en = crate::lower::enum_of(&cx, f).unwrap_or_else(|| x.name.clone());
        let lowered = [self.values.as_ref().map(|v| ("values", &v.1)), self.route.as_ref().map(|r| ("route", r))].into_iter().flatten().find(|(_, f)| f.enums.iter().any(|e| e.public == en || e.name == en));
        let (alias, facts) = lowered?;
        let e = facts.enums.iter().find(|e| e.public == en || e.name == en)?;
        Some(format!("{alias}.{}", e.public))
    }

    /// Whether the computed field `x` has a value, over the values of the fields it reads that may be
    /// empty, each by its name in `local` (DESIGN 4.9): a kaname expression; `true` where it always
    /// has one.
    fn has_value(&self, x: &Fld, local: &dyn Fn(&Fld) -> String) -> String {
        let s = self.m.s;
        let Some(f) = s.fields().iter().find(|f| f.name.ascii() == x.name) else { return "true".to_string() };
        let each: Vec<String> = crate::lower::empty_reads(s, f).iter().filter_map(|g| self.fields.iter().find(|y| y.name == g.name.ascii())).map(|y| present(y.unit.as_ref(), &local(y))).collect();
        if each.is_empty() { "true".to_string() } else { each.join(" and ") }
    }

    /// The fields of the form a person gives (at the top, not computed).
    fn given(&self) -> impl Iterator<Item = &Fld> {
        self.fields.iter().filter(|x| matches!(x.kind, Kind::Given | Kind::Rows { .. }))
    }

    /// The records: the rows of each rows field as given, as held to what the form declares and as
    /// the ticket holds them, `Given`, `Form`, and what the form works out (`Outside`, `Worked`) when
    /// it works something out.
    pub(super) fn records(&self) -> String {
        let mut out = String::new();
        for x in &self.fields {
            let Kind::Rows { children, .. } = &x.kind else { continue };
            out.push_str(&record(&x.fns.given_rec, children.iter().map(|c| (c.name.clone(), format!("{}?{}", c.ty, c.open())))));
            out.push_str(&record(&x.fns.checked_rec, children.iter().map(|c| (c.name.clone(), held_type(c)))));
            out.push_str(&record(&x.fns.row_rec, children.iter().map(|c| (c.name.clone(), wire_type(c)))));
        }
        out.push_str(&record("Given", self.given().map(|x| (x.name.clone(), format!("{}?{}", given_type(x), x.open())))));
        out.push_str(&record("Form", self.fields.iter().map(|x| (x.name.clone(), wire_type(x)))));
        if let Some((lf, facts, _)) = &self.values {
            out.push_str(&record("Outside", ["field", "text", "text_ja", "at"].iter().map(|n| (n.to_string(), "string".to_string()))));
            let mut fs: Vec<(String, String)> = Vec::new();
            for o in &facts.outputs {
                if let Some(n) = self.worked_name(lf, &o.name) {
                    // a computation that may have no value (DESIGN 4.9)
                    let opt = if self.fields.iter().any(|x| x.name == n && matches!(x.kind, Kind::Computed { .. }) && !x.required) { "?" } else { "" };
                    let ty = match rate_of(&o.ty) {
                        Some(u) => format!("{}{opt}{}", kn_type(u), rate_open(Some(u))),
                        None => format!("{}{opt}", column_type(&o.ty)),
                    };
                    fs.push((n, ty));
                }
            }
            fs.push(("outside".to_string(), "Outside?".to_string()));
            out.push_str(&record("Worked", fs.into_iter()));
        }
        let hid = self.hidable();
        if !hid.is_empty() {
            out.push_str(&record("Shown", hid.iter().map(|x| (x.name.clone(), "bool".to_string()))));
        }
        out
    }

    /// The name `Worked` gives an output of the copy: the field it works out, or whether a field is
    /// shown or asked for (`trip_to_shown`). None for the most and the least of a computation of a
    /// row, which only the check of its range reads.
    fn worked_name(&self, lf: &LFile, output: &str) -> Option<String> {
        let want = asked(lf, output);
        if self.fields.iter().any(|x| x.name == want && matches!(x.kind, Kind::Computed { .. })) {
            return Some(want);
        }
        self.m.s.fields().iter().any(|f| want == format!("{}_shown", f.name.ascii()) || want == format!("{}_required", f.name.ascii())).then_some(want)
    }

    /// The functions of the form: each value given held to its field (`given_<field>`, and for rows
    /// `cell_…`, `row_…`, `add_…`, `no_…`, `given_…`), a choice as a rule's enum (`choice_…`), what
    /// the form works out (`worked`), the fields asked for on a condition (`asked`), the form of the
    /// values given (`form_of`) and `given`, which a host's `start` calls.
    pub(super) fn functions(&self) -> Result<String, Text> {
        let mut out = String::new();
        for x in &self.fields {
            match &x.kind {
                Kind::Given => out.push_str(&given_fn(x)),
                Kind::Rows { children, count } => out.push_str(&rows_fns(x, children, count.as_ref())),
                Kind::Computed { .. } | Kind::Counted => {}
            }
        }
        let mut rates: Vec<&Unit> = Vec::new();
        for x in &self.fields {
            let children = match &x.kind {
                Kind::Rows { children, .. } => children.as_slice(),
                _ => &[],
            };
            for u in std::iter::once(x).chain(children).filter_map(|x| x.unit.as_ref()).filter(|u| u.dim == Dim::Rate) {
                if !rates.iter().any(|r| rate_tag(r) == rate_tag(u)) {
                    rates.push(u);
                }
            }
        }
        let numbers = self.fields.iter().any(|x| match &x.kind {
            Kind::Rows { children, .. } => x.unit.is_some() || children.iter().any(|c| c.unit.is_some()),
            _ => x.unit.is_some(),
        });
        if numbers {
            out.push_str(GROUPED);
        }
        for u in rates {
            out.push_str(&rate_fns(u));
        }
        out.push_str(&self.choices()?);
        if self.values.is_some() {
            out.push_str(&self.worked()?);
        }
        out.push_str(&self.asked());
        out.push_str(&self.shown()?);
        out.push_str(&self.form_of()?);
        out.push_str(&self.given_all());
        Ok(out)
    }

    /// For each choice a rule reads, its value as the rule's enum: `choice_<field>` for the enum of
    /// `choices.rule` (one type in every lowered rule that reads it, under the name the lowered files
    /// give it, each value by the name they write it under), `choice_<field>_<rule>` for the enum of a
    /// rule the route reads an output of.
    fn choices(&self) -> Result<String, Text> {
        let s = self.m.s;
        let cx = crate::lower::context(s, self.m.suite);
        let mut out = String::new();
        for x in self.given() {
            let Some((vs, from)) = &x.choice else { continue };
            let Some(f) = s.fields().iter().find(|f| f.name.ascii() == x.name) else { continue };
            let en = crate::lower::enum_of(&cx, f).unwrap_or_else(|| x.name.clone());
            let lowered = [self.values.as_ref().map(|v| ("values", &v.1)), self.route.as_ref().map(|r| ("route", r))].into_iter().flatten().find(|(_, f)| f.enums.iter().any(|e| e.public == en || e.name == en));
            if let Some((alias, facts)) = lowered {
                let e = facts.enums.iter().find(|e| e.public == en || e.name == en).expect("the enum is there");
                let values: Vec<String> = e.values.iter().map(|v| v.public.clone()).collect();
                let pairs: Vec<(String, String)> = vs.iter().map(|v| (v.clone(), if from.is_none() { words::choice_value(s, f, v) } else { v.clone() })).collect();
                out.push_str(&choice_fn(&x.fns.choice, &format!("{alias}.{}", e.public), &pairs, &values)?);
            }
            if let Some((rule, en)) = from
                && let Some((_, ascii, _, facts)) = self.used.iter().find(|u| u.0 == *rule)
                && let Some(e) = facts.enums.iter().find(|e| e.name == *en || e.public == *en)
            {
                let values: Vec<String> = e.values.iter().map(|v| v.public.clone()).collect();
                let pairs: Vec<(String, String)> = vs.iter().map(|v| (v.clone(), v.clone())).collect();
                out.push_str(&choice_fn(&x.fns.choice_from, &format!("{ascii}.{}", e.public), &pairs, &values)?);
            }
        }
        Ok(out)
    }

    /// What `worked` takes of the copy's inputs: each input, the field it reads, how the copy is
    /// given its value over [`X`], and whether it is whether the field is filled in (a condition's
    /// `cover is none`, a field a computation reads that may be empty), which `worked` takes as yes
    /// or no by the input's name. A choice is given as its enum; a value that may be empty as one
    /// that may be none (`in_<field>`, `choice_opt_<field>`), or, where a computation reads it, as a
    /// value of its range beside whether it is filled in (`fill_<field>`, DESIGN 4.9).
    fn worked_inputs(&self) -> Result<Vec<(String, String, String, bool)>, Text> {
        let Some((lf, facts, _)) = &self.values else { return Ok(Vec::new()) };
        let s = self.m.s;
        let mut out = Vec::new();
        for i in &facts.inputs {
            if let Some((_, field)) = lf.given.iter().find(|(col, _)| *col == i.name) {
                out.push((i.name.clone(), field.clone(), X.to_string(), true));
                continue;
            }
            let want = asked(lf, &i.name);
            let Some(x) = self.fields.iter().find(|x| x.name == want && matches!(x.kind, Kind::Given)) else {
                return Err(not_yet(format!("a computation or a condition of the form that reads `{want}`")));
            };
            let arg = match (&i.ty, &x.choice) {
                (ColumnType::Enum { .. }, Some(_)) if x.required => format!("{}(x: {X})", x.fns.choice),
                _ if x.required => X.to_string(),
                (ColumnType::Opt(t), Some(_)) if matches!(t.as_ref(), ColumnType::Enum { .. }) => self.choice_opt(x, X)?,
                (ColumnType::Opt(_), _) => self.in_range(x, None, X),
                _ => {
                    let f = s.fields().iter().find(|f| f.name.ascii() == x.name).ok_or_else(|| not_yet(format!("a computation of the form that reads `{}`", x.shown)))?;
                    self.filled(x, f, None, X)
                }
            };
            out.push((i.name.clone(), x.name.clone(), arg, false));
        }
        Ok(out)
    }

    /// The rows fields the copy walks, by its elements: each element's name and the field.
    fn worked_rows(&self) -> Vec<(String, String)> {
        let Some((lf, facts, _)) = &self.values else { return Vec::new() };
        facts.elements.iter().map(|(el, ..)| (el.clone(), asked(lf, el))).collect()
    }

    /// `worked`: what the form works out of the values given (the copy's outputs), and the first
    /// value outside its range, in the order rulec's door would hold them.
    fn worked(&self) -> Result<String, Text> {
        let (lf, facts, held) = self.values.as_ref().expect("the form works something out");
        let mut names = Names::default();
        let mut params = Vec::new();
        let mut call = Vec::new();
        let inputs = self.worked_inputs()?;
        // whether a field is filled in, by the input's name, which is no field's (lowering avoids
        // the names the request declares)
        for (input, ..) in inputs.iter().filter(|i| i.3) {
            names.keep(input);
        }
        for (input, field, arg, given) in inputs {
            if given {
                params.push(format!("{input}: bool"));
                call.push(format!("{input}: {input}"));
                continue;
            }
            let x = self.fields.iter().find(|x| x.name == field).expect("a field");
            let l = &x.local;
            names.keep(l);
            // a value that may be empty is taken as it is, and narrowed where the copy is called
            let ty = if x.required { format!("{}{}", x.ty, x.range.as_ref().map_or_else(|| x.open(), |(r, _)| format!(" {r}"))) } else { format!("{}?{}", x.ty, x.open()) };
            params.push(format!("{l}: {ty}"));
            call.push(format!("{input}: {}", arg.replace(X, l)));
        }
        let rows = self.worked_rows();
        for (_, f) in &rows {
            if let Some(x) = self.fields.iter().find(|x| x.name == *f) {
                names.keep(&x.local);
            }
        }
        // each rows field the copy walks: its name, its element's, its local name, and the element the
        // copy takes of a row written as [`ROW`]
        let mut elements: Vec<(String, String, String, String)> = Vec::new();
        for ((el, f), (_, _, cols)) in rows.iter().zip(&facts.elements) {
            let Some(x) = self.fields.iter().find(|x| x.name == *f) else { return Err(not_yet(format!("the rows `{f}` the form walks"))) };
            let Kind::Rows { children, count } = &x.kind else { return Err(not_yet(format!("the rows `{f}` the form walks"))) };
            let count = count.as_ref().map(|(c, _)| format!(" {c}")).unwrap_or_default();
            let l = &x.local;
            params.push(format!("{l}: list[{}]{count}", x.fns.checked_rec));
            let row = names.take("row");
            let mut cells = Vec::new();
            let rows_field = self.m.s.fields().iter().find(|g| g.name.ascii() == *f);
            for c in cols {
                // whether a cell is filled in, which a field of the element says (DESIGN 4.9)
                if let Some((_, cell)) = lf.given.iter().find(|(col, cell)| *col == c.name && cell.contains('.')) {
                    let cw = cell.rsplit('.').next().unwrap_or_default();
                    let Some(child) = children.iter().find(|y| y.name == cw) else { return Err(not_yet(format!("the field `{f}.{cw}` the form reads"))) };
                    cells.push(format!("{}: {}", c.name, present(child.unit.as_ref(), &format!("{ROW}.{cw}"))));
                    continue;
                }
                let cw = asked(lf, &c.name);
                let Some(child) = children.iter().find(|y| y.name == cw) else { return Err(not_yet(format!("the field `{f}.{cw}` the form reads"))) };
                if child.required {
                    cells.push(format!("{}: {ROW}.{cw}", c.name));
                    continue;
                }
                let Some(g) = rows_field.and_then(|r| r.children.iter().find(|g| g.name.ascii() == cw)) else { return Err(not_yet(format!("the field `{f}.{cw}` the form reads"))) };
                cells.push(format!("{}: {}", c.name, self.filled(child, g, Some(f), &format!("{ROW}.{cw}"))));
            }
            let element = format!("values.{el} {{ {} }}", cells.join(", "));
            call.push(format!("{el}: {l}.map({row} => {})", element.replace(ROW, &row)));
            elements.push((f.clone(), el.clone(), l.clone(), element));
        }
        let got = names.take("got");
        let outside = names.take("outside");
        let mut body = format!("let {got} = values({})\n", call.join(", "));
        // the first value outside its range
        let mut checks = Vec::new();
        // the functions that find the first row a computation of the rows is outside its range on
        let mut finders = String::new();
        // whether a computation has a value, by the values of the fields it reads that may be empty,
        // as `worked` takes them
        let local = |y: &Fld| y.local.clone();
        // a value of `h` outside its range, over the copy's outputs `got`: its value, or for a
        // computation of a row, its most or its least over the rows
        let past = |h: &crate::lower::form::Held, got: &str| -> Vec<String> {
            let mut bad = Vec::new();
            if let Some((n, v)) = &h.lo {
                bad.push(format!("{got}.{n} < {}", words::written(words::Val::Num(*v), &h.ty)));
            }
            if let Some((n, v)) = &h.hi {
                bad.push(format!("{got}.{n} > {}", words::written(words::Val::Num(*v), &h.ty)));
            }
            bad
        };
        let mut i = 0;
        while i < held.len() {
            let h = &held[i];
            let Some(r) = &h.rows else {
                i += 1;
                let mut bad = past(h, &got);
                // a computation that has no value is held to no range (DESIGN 4.9)
                let has = self.fields.iter().find(|x| x.name == h.field && !x.required).map(|x| self.has_value(x, &local));
                if let Some(has) = has.filter(|h| h != "true") {
                    let all = bad.join(" or ");
                    bad = vec![format!("{has} and ({all})")];
                }
                checks.push((bad.join(" or "), format!("Outside {{ field: {}, text: {}, text_ja: {}, at: {} }}", quoted(&h.field), quoted(&h.said.en), quoted(&h.said.ja), quoted(&h.field))));
                continue;
            };
            // the computations of a row of the same rows, which the door holds on each row in turn:
            // where the most or the least of one over the rows is outside its range, the first row one
            // of them is outside on, worked out by the copy over that row alone (which its computations
            // read nothing else of), and the first such computation of the row
            let n = held[i..].iter().take_while(|g| g.rows.as_ref() == Some(r)).count();
            let group = &held[i..i + n];
            i += n;
            let Some((_, el, l, element)) = elements.iter().find(|(f, ..)| f == r) else { return Err(not_yet(format!("the rows `{r}` the form walks"))) };
            let bad: Vec<String> = group.iter().flat_map(|g| past(g, &got)).collect();
            let finder = self.names.borrow_mut().take(&format!("outside_{r}"));
            let at = names.take("at");
            let one: Vec<String> = call.iter().map(|c| if c.starts_with(&format!("{el}: ")) { format!("{el}: [{}]", element.replace(ROW, &format!("{at}.item"))) } else { c.clone() }).collect();
            let index = format!("\", text({at}.index), \"");
            let mut each = Vec::new();
            for g in group {
                let said = |t: &str| splice(&quoted(t), crate::lower::form::INDEX, &index);
                let place = format!("concat({}, text({at}.index), {})", quoted(&format!("{r}[")), quoted(&format!("].{}", g.field)));
                each.push((past(g, &got).join(" or "), format!("Outside {{ field: {}, text: {}, text_ja: {}, at: {place} }}", quoted(&format!("{r}.{}", g.field)), said(&g.said.en), said(&g.said.ja))));
            }
            let mut f = format!("let {got} = values({})\n", one.join(", "));
            for (k, (bad, then)) in each.iter().enumerate() {
                let kw = if k == 0 { "if" } else { "else if" };
                f.push_str(&format!("{kw} {bad}\n  {then}\n"));
            }
            f.push_str("else\n  none\n");
            let row_rec = self.fields.iter().find(|x| x.name == *r).map(|x| x.fns.checked_rec.clone()).unwrap_or_default();
            // what the copy reads besides the rows: every value `worked` takes but the rows
            let mut fparams: Vec<String> = params.iter().filter(|p| !p.starts_with(&format!("{l}: "))).cloned().collect();
            let args: Vec<String> = fparams.iter().map(|p| p.split(':').next().unwrap_or_default().trim().to_string()).map(|p| format!("{p}: {p}")).chain([format!("{at}: {at}")]).collect();
            fparams.push(format!("{at}: indexed[{row_rec}]"));
            finders.push_str(&format!(
                "fn {finder}({}) -> Outside? total\n  description \"The first computation of the row of `{r}` that is outside its range on it, as the form works the row out alone\"\n{}\n",
                fparams.join(", "),
                indent(&f, 2)
            ));
            checks.push((bad.join(" or "), format!("{l}.indexed().filter_map({at} => {finder}({})).first()", args.join(", "))));
        }
        if checks.is_empty() {
            body.push_str(&format!("let {outside} : Outside? = none\n"));
        } else {
            // the first row a computation of the rows is outside on is found, or none
            let ty = if finders.is_empty() { "" } else { " : Outside?" };
            body.push_str(&format!("let {outside}{ty} =\n"));
            for (i, (bad, then)) in checks.iter().enumerate() {
                let kw = if i == 0 { "if" } else { "else if" };
                body.push_str(&format!("  {kw} {bad}\n    {then}\n"));
            }
            body.push_str("  else\n    none\n");
        }
        let mut fs = Vec::new();
        for o in &facts.outputs {
            if let Some(n) = self.worked_name(lf, &o.name) {
                let v = match rate_of(&o.ty) {
                    // the rule rounds the rate to its step (`round down(0.25%)`), which kaname does
                    // not read across into the rule's answer: rounded again, it is the same value
                    Some(u) => format!("down({got}.{}, {})", o.name, words::number(words::step(u), u)),
                    None => format!("{got}.{}", o.name),
                };
                // a computation that reads a field that is empty has no value (DESIGN 4.9)
                let v = match self.fields.iter().find(|x| x.name == n && matches!(x.kind, Kind::Computed { .. }) && !x.required) {
                    Some(x) => kept(x.unit.as_ref(), &v, &self.has_value(x, &local)),
                    None => v,
                };
                fs.push(format!("{n}: {v}"));
            }
        }
        fs.push(format!("outside: {outside}"));
        body.push_str(&format!("Worked {{ {} }}\n", fs.join(", ")));
        Ok(format!(
            "{finders}fn worked({}) -> Worked total\n  description \"What the form works out of the values given: its totals and computations, whether each field is shown and asked for, and the first value it works out that is outside its range\"\n{}\n",
            params.join(", "),
            indent(&body, 2)
        ))
    }

    /// The fields shown or asked for on a condition, which `asked` holds: an approvers' field shown
    /// on a condition too, whose value the form does not take where it does not show it.
    fn conditioned(&self) -> Vec<&Fld> {
        if self.values.is_none() {
            return Vec::new();
        }
        self.given().filter(|x| x.shown_when || (!x.approvers && x.required_when)).collect()
    }

    /// The fields whose value the form drops where it does not show them, when a resubmit or an
    /// edit does not give it (DESIGN 7.3): the ones shown on a condition.
    fn hidable(&self) -> Vec<&Fld> {
        if self.values.is_none() {
            return Vec::new();
        }
        self.given().filter(|x| x.shown_when && matches!(x.kind, Kind::Given)).collect()
    }

    /// `asked`: the form, when it has every field it asks for on a condition and none it does not
    /// show, as the form works the conditions out (`taken::missing`).
    fn asked(&self) -> String {
        let conds = self.conditioned();
        if conds.is_empty() {
            return String::new();
        }
        let mut each = Vec::new();
        for x in conds {
            let shown = if x.shown_when { format!("w.{}_shown", x.name) } else { "true".to_string() };
            // an approvers' field is asked for of the approvers, not of the form
            let asked = if x.approvers {
                "false".to_string()
            } else if x.required_when {
                format!("w.{}_required", x.name)
            } else {
                (!x.optional).to_string()
            };
            let (says, says_ja) = if x.required_when {
                (format!("`{}` is not filled in, and this request must have it (its `required when` holds)", x.shown), format!("`{}` がありません。この申請では必須です（`required when` が成り立つ）", x.shown))
            } else {
                (format!("`{}` is not filled in, and the form asks for it", x.shown), format!("`{}` がありません。必須のフィールドなので、入れてください", x.shown))
            };
            each.push(format!(
                "machine.untaken(has: {}, shown: {shown}, asked: {asked}, name: {}, at: {}, missing: {}, missing_ja: {})",
                present(x.unit.as_ref(), &format!("form.{}", x.name)),
                quoted(&x.shown),
                at_field(&x.name),
                quoted(&says),
                quoted(&says_ja)
            ));
        }
        format!(
            "fn asked(form: Form, w: Worked) -> result[Form, machine.Refusal] total\n  description \"The form, when it has every field it asks for on a condition and none it does not show\"\n  match [{}].filter_map(r => r).first()\n    none => ok(form)\n    some r => err(r)\n\n",
            each.join(", ")
        )
    }

    /// What `form_of` and `shown` take, each value of the form given as `form_of` takes it, and the
    /// names their bodies keep from the values': the parameters, the checks of the ranges and the
    /// counts of the values given where kaname narrows them, and the names.
    fn formed_params(&self) -> (Vec<String>, Vec<Over>, Names) {
        let mut names = Names::default();
        let mut params = Vec::new();
        for x in self.given() {
            names.keep(&x.local);
            params.push(format!("{}: {}{}", x.local, formed_type(x), x.open()));
        }
        for x in self.fields.iter().filter(|x| matches!(x.kind, Kind::Counted)) {
            names.keep(&x.local);
        }
        // the ranges and the counts of the values given
        let mut checks = Vec::new();
        for x in self.given().filter(|x| x.required) {
            match &x.kind {
                Kind::Rows { count: Some((_, cs)), .. } => checks.extend(cs.iter().map(|c| c.over(&x.local, &at_field(&x.name)))),
                Kind::Given => checks.extend(x.range.iter().map(|(_, c)| c.over(&x.local, &at_field(&x.name)))),
                _ => {}
            }
        }
        (params, checks, names)
    }

    /// The arguments of `worked` over the values `form_of` takes: a value that may be empty as it
    /// is, which `worked` reads as the copy takes it (DESIGN 4.9).
    fn worked_args(&self) -> Result<Vec<String>, Text> {
        let mut args = Vec::new();
        for (input, f, _, given) in self.worked_inputs()? {
            let x = self.fields.iter().find(|x| x.name == f).expect("a field");
            let l = &x.local;
            if given {
                let filled = if x.required { "true".to_string() } else { present(x.unit.as_ref(), l) };
                args.push(format!("{input}: {filled}"));
                continue;
            }
            args.push(format!("{l}: {l}"));
        }
        args.extend(self.worked_rows().iter().filter_map(|(_, f)| self.fields.iter().find(|x| x.name == *f)).map(|x| format!("{l}: {l}", l = x.local)));
        Ok(args)
    }

    /// `shown`: whether each field the form shows on a condition is shown for the values given
    /// (`Shown`, by the fields' names); none where the form's computations are not worked out for
    /// them (a value given is outside its range, or one they work out is), where the reference drops
    /// no more of them (DESIGN 7.3). A value they read that is empty is read as none (DESIGN 4.9).
    fn shown(&self) -> Result<String, Text> {
        let hid = self.hidable();
        if hid.is_empty() {
            return Ok(String::new());
        }
        let (params, checks, mut names) = self.formed_params();
        let w = names.take("w");
        let o = names.take("o");
        let args = self.worked_args()?;
        let each: Vec<String> = hid.iter().map(|x| format!("{}: {w}.{}_shown", x.name, x.name)).collect();
        let then = format!("let {w} = worked({})\nmatch {w}.outside\n  some {o} => none\n  none => Shown {{ {} }}\n", args.join(", "), each.join(", "));
        Ok(format!(
            "fn shown({}) -> Shown? total\n  description \"Whether each field shown on a condition is shown for the values given; none where the form's computations are not worked out for them\"\n{}\n",
            params.join(", "),
            chain_to(&checks, &then, 2, &|_| "none".to_string())
        ))
    }

    /// `form_of`: the form of the values given, each held to its range and its count where kaname
    /// narrows it, with what the form works out of them held to its range.
    fn form_of(&self) -> Result<String, Text> {
        let (params, checks, mut names) = self.formed_params();
        let w = names.take("w");
        let o = names.take("o");
        let args = self.worked_args()?;
        let fields: Vec<String> = self
            .fields
            .iter()
            .map(|x| match &x.kind {
                Kind::Computed { .. } => format!("{}: {w}.{}", x.name, x.name),
                Kind::Counted => format!("{}: {}", x.name, x.local),
                // the rows as the ticket holds them
                Kind::Rows { children, .. } => {
                    let cells: Vec<String> = children.iter().map(|c| format!("{}: y.{}", c.name, c.name)).collect();
                    let cells = if cells.is_empty() { "nothing: none".to_string() } else { cells.join(", ") };
                    format!("{}: {}.map(y => {} {{ {cells} }})", x.name, x.local, x.fns.row_rec)
                }
                Kind::Given => format!("{}: {}", x.name, x.local),
            })
            .collect();
        let made = if fields.is_empty() { "Form { nothing: none }".to_string() } else { format!("Form {{ {} }}", fields.join(", ")) };
        let done = if self.conditioned().is_empty() { format!("ok({made})") } else { format!("asked(form: {made}, w: {w})") };
        let done = self.counted(&w, &done, &mut names)?;
        let then = match &self.values {
            None => done,
            Some(_) => {
                // what the form works out, held to the ranges the form writes; one that may have no
                // value is held to its range by `worked`, where it has one (DESIGN 4.9)
                let computed: Vec<Over> = self
                    .fields
                    .iter()
                    .filter(|x| x.required)
                    .filter_map(|x| match &x.kind {
                        Kind::Computed { range: Some((_, c)) } => Some(c.over(&format!("{w}.{}", x.name), &at_field(&x.name))),
                        _ => None,
                    })
                    .collect();
                format!(
                    "let {w} = worked({})\nmatch {w}.outside\n  some {o} => err(refused(text: {o}.text, text_ja: {o}.text_ja, at: {o}.at))\n  none =>\n{}",
                    args.join(", "),
                    chain(&computed, &done, 4),
                )
            }
        };
        Ok(format!(
            "fn form_of({}) -> result[Form, machine.Refusal] total\n  description \"The form of the values given: each held to its range, with what the form works out of them held to its range\"\n{}\n",
            params.join(", "),
            chain(&checks, &then, 2)
        ))
    }

    /// The dates the form counts, around `done` (what reads them), each counted by `form.cal` from
    /// the values it reads, then held to its range and its calendar, as the reference holds a date it
    /// counts. A value a date is counted from is narrowed first to the range `form.cal` reads it in
    /// where that is narrower than its field's (the days of the calendar's data a date is counted
    /// from), refused outside it in koyomi's words, as the reference refuses it. A date counted from
    /// a value that is empty has no value (DESIGN 4.9).
    fn counted(&self, w: &str, done: &str, names: &mut Names) -> Result<String, Text> {
        let Some((lf, facts)) = &self.dates else { return Ok(done.to_string()) };
        let s = self.m.s;
        let first_x = facts.functions.first().and_then(|f| self.fields.iter().find(|x| x.name == asked(lf, &f.name)));
        let (first, first_at) = first_x.map(|x| (x.shown.clone(), at_field(&x.name))).unwrap_or_else(|| (String::new(), "none".to_string()));
        let mut inner = done.to_string();
        for x in self.fields.iter().filter(|x| matches!(x.kind, Kind::Counted)).collect::<Vec<_>>().into_iter().rev() {
            let Some(f) = facts.functions.iter().find(|f| asked(lf, &f.name) == x.name) else { return Err(not_yet(format!("the date `{}`, which `form.cal` does not count", x.shown))) };
            let mut args = Vec::new();
            let mut narrow = Vec::new();
            // the values it is counted from that may be empty, each with the name its value is bound to
            let mut opens: Vec<(String, String)> = Vec::new();
            for p in &f.params {
                let Some(i) = facts.inputs.iter().find(|i| i.name == *p) else { return Err(not_yet(format!("the date `{}`, counted from `{p}`", x.shown))) };
                let want = asked(lf, p);
                let Some(y) = self.fields.iter().find(|y| y.name == want) else { return Err(not_yet(format!("the date `{}`, counted from `{want}`", x.shown))) };
                let field = s.fields().iter().find(|g| g.name.ascii() == y.name);
                let (v, (lo, hi)) = match (&y.kind, field) {
                    (Kind::Given | Kind::Counted, Some(g)) if y.required => (y.local.clone(), words::ends(g)),
                    (Kind::Computed { .. }, Some(g)) if y.required => (format!("{w}.{}", y.name), crate::lower::route::reach_or_range(s, g)),
                    (Kind::Given | Kind::Computed { .. }, Some(_)) => {
                        let of = if matches!(y.kind, Kind::Computed { .. }) { format!("{w}.{}", y.name) } else { y.local.clone() };
                        let b = names.take(&format!("{}_v", y.name));
                        opens.push((of, b.clone()));
                        (b, (None, None))
                    }
                    _ => return Err(not_yet(format!("the date `{}`, counted from `{}`", x.shown, y.shown))),
                };
                let (ilo, ihi) = (Rat::int(i128::from(i.min)), Rat::int(i128::from(i.max)));
                let shown = |r: Rat| if matches!(y.vk, Vk::Date) { crate::lex::date_text(r.num as i64) } else { words::grouped(r.num) };
                let mut bad = Vec::new();
                if lo.is_none_or(|l| l.cmp_to(ilo) == std::cmp::Ordering::Less) {
                    bad.push(format!("{v} < {}", shown(ilo)));
                }
                if hi.is_none_or(|h| h.cmp_to(ihi) == std::cmp::Ordering::Greater) {
                    bad.push(format!("{v} > {}", shown(ihi)));
                }
                if !bad.is_empty() {
                    let en = around(&format!("`{first}` cannot be counted: "), &format!("text({v})"), &format!(" of the input `{want}` is outside its range"));
                    let ja = around(&format!("`{first}` を数えられません: 入力 `{want}` の "), &format!("text({v})"), " が範囲の外です");
                    narrow.push(Over { bad: bad.join(" or "), en, ja, at: first_at.clone() });
                }
                args.push(format!("{p}: {v}"));
            }
            let held: Vec<Over> = x.range.iter().map(|(_, c)| c).chain(&x.open).map(|c| c.over(&x.local, &at_field(&x.name))).collect();
            let counted = format!("let {} = form_dates.{}({})\n{}", x.local, f.name, args.join(", "), chain(&held, &inner, 0));
            let mut one = chain(&narrow, &counted, 0);
            // where a value it is counted from is empty, it has none
            let none = format!("let {} : date? = none\n{inner}", x.local);
            for (of, b) in opens.iter().rev() {
                one = format!("match {of}\n  none =>\n{}  some {b} =>\n{}", indent(&none, 4), indent(&one, 4));
            }
            inner = one;
        }
        Ok(inner)
    }

    /// `given`: the values the requester gives, held to what the form declares of each, in the order
    /// of the form, then the form of them.
    fn given_all(&self) -> String {
        let mut out = String::from("fn given(g: Given, org: machine.Org) -> result[Form, machine.Refusal] total\n  description \"The values the requester gives, held to what the form declares of each\"\n");
        let mut names = Names::default();
        names.take("g");
        names.take("org");
        let mut args = Vec::new();
        for x in self.given() {
            let local = names.take(&x.local);
            let org = if needs_org(x) { ", org: org" } else { "" };
            out.push_str(&format!("  let {local} = try {}(v: g.{}{org})\n", x.fns.given, x.name));
            args.push(format!("{}: {local}", x.local));
        }
        out.push_str(&format!("  form_of({})\n\n", args.join(", ")));
        out
    }

    /// `vals`: the values of the form a flow is given, by their names in the JSON, in the order of
    /// the form (what was given, then what the form works out), each as the JSON holds it (a number
    /// as the whole number of its steps, a date as its text, rows as lists of their values). A field
    /// that is not filled in is not given, as the reference gives none of it.
    pub(super) fn vals(&self) -> String {
        let mut out = String::new();
        for x in &self.fields {
            let Kind::Rows { children, .. } = &x.kind else { continue };
            let row = &x.fns.row_rec;
            let mut segs = Vec::new();
            for c in children {
                if c.required {
                    segs.push(Seg::One(arg_of(c, &format!("r.{}", c.name))));
                } else {
                    let f = &c.fns.val;
                    out.push_str(&val_fn(f, c));
                    segs.push(Seg::Many(format!("{f}(v: r.{})", c.name)));
                }
            }
            out.push_str(&format!(
                "fn {}(r: {row}) -> list[machine.Arg] total\n  description \"The values of a row of `{}`, by their names in the JSON\"\n  {}\n\n",
                x.fns.row_vals,
                x.name,
                joined(segs)
            ));
        }
        let mut segs = Vec::new();
        // what the form works out after what was given: its numbers, then the dates it counts
        let computed = self.fields.iter().filter(|x| matches!(x.kind, Kind::Computed { .. }));
        let counted = self.fields.iter().filter(|x| matches!(x.kind, Kind::Counted));
        for x in self.given().chain(computed).chain(counted) {
            if x.required {
                segs.push(Seg::One(arg_of(x, &format!("form.{}", x.name))));
            } else {
                let f = &x.fns.val;
                out.push_str(&val_fn(f, x));
                segs.push(Seg::Many(format!("{f}(v: form.{})", x.name)));
            }
        }
        out.push_str(&format!(
            "fn vals(form: Form) -> list[machine.Arg] total\n  description \"The values of the form a flow is given, by their names in the JSON: what was given, then what the form works out\"\n  {}\n\n",
            joined(segs)
        ));
        out
    }

    /// `reform`: the form given again by the requester (`resubmit`) or edited by an approver
    /// (`edit`), as the reference makes it (DESIGN 7.3): the names `clear` lists held to the form
    /// (`clearable`), each value given held to its field and to who gives it (`regiven_<field>`), the
    /// fields `clear` names made empty and the others as the ticket holds them; then the value of each
    /// field the form does not show for the values dropped where the event gives it none, every one
    /// hidden at once, again for what is left, as many times as there are such fields (`shown`); then
    /// the form of them, the stages the route comes to for it, the values a flow is given, the fields
    /// given (`named`), which the history says an approver edited, and the fields that lost their
    /// value (`lost`), which the history says too.
    pub(super) fn reform(&self) -> Result<String, Text> {
        let s = self.m.s;
        let editable: Vec<String> = s.file.ticket.iter().flat_map(|t| &t.lines).find_map(|l| if let TicketLine::ApproversEdit { fields, .. } = l { Some(fields.iter().map(|w| w.text.clone()).collect()) } else { None }).unwrap_or_default();
        let mut out = String::new();
        for x in self.given() {
            out.push_str(&regiven_fn(x, editable.contains(&x.shown)));
        }
        let each: Vec<String> = self.given().map(|x| format!("name_if(has: {}, name: {})", present(x.unit.as_ref(), &format!("g.{}", x.name)), quoted(&x.name))).collect();
        out.push_str("fn name_if(has: bool, name: string) -> string? total\n  match has\n    true => name\n    false => none\n\n");
        let named = if each.is_empty() { "[]".to_string() } else { format!("[{}].filter_map(x => x)", each.join(", ")) };
        out.push_str(&format!("fn named(g: Given) -> list[string] total\n  description \"The fields given, by their names in the JSON: what the history says an approver edited\"\n  {named}\n\n"));
        out.push_str(&self.clearable());
        out.push_str(&self.lost());
        let mut names = Names::default();
        for n in ["form", "g", "clear", "approver", "org", "clearable", "shown", "form_of", "formed", "vals", "named", "lost"] {
            names.take(n);
        }
        let c = names.take("cleared");
        let mut body = format!("  let {c} = try clearable(names: clear)\n");
        let mut locals: Vec<(&Fld, String)> = Vec::new();
        for x in self.given() {
            let l = names.take(&x.local);
            let org = if x.org { ", org: org" } else { "" };
            body.push_str(&format!("  let {l} = try {}(v: g.{}, was: form.{}, approver: approver, clear: clear.contains({}){org})\n", x.fns.regiven, x.name, x.name, quoted(&x.name)));
            locals.push((x, l));
        }
        // the fields the form does not show for the values, dropped: as many times as there are of
        // them, each time all of them hidden then, unless the event gives one a value
        let hid: Vec<String> = self.hidable().iter().map(|x| x.name.clone()).collect();
        let sv = names.take("s");
        for round in 1..=hid.len() {
            let sh = names.take(&format!("shown_{round}"));
            let args: Vec<String> = locals.iter().map(|(x, l)| format!("{}: {l}", x.local)).collect();
            body.push_str(&format!("  let {sh} = shown({})\n", args.join(", ")));
            for (x, l) in locals.iter_mut().filter(|(x, _)| hid.contains(&x.name)) {
                let next = names.take(&format!("{}_{round}", x.local));
                body.push_str(&format!(
                    "  let {next} =\n    match {sh}\n      none => {l}\n      some {sv} => {}\n",
                    kept(x.unit.as_ref(), l, &format!("{sv}.{} or {}", x.name, present(x.unit.as_ref(), &format!("g.{}", x.name))))
                ));
                *l = next;
            }
        }
        let args: Vec<String> = locals.iter().map(|(x, l)| format!("{}: {l}", x.local)).collect();
        let f = names.take("f");
        let st = names.take("stages");
        body.push_str(&format!(
            "  let {f} = try form_of({})\n  let {st} = try formed(form: {f})\n  ok(machine.Formed {{ form: {f}, stages: {st}, vals: vals(form: {f}), named: named(g: g), cleared: lost(was: form, now: {f}) }})\n\n",
            args.join(", ")
        ));
        out.push_str(&format!(
            "fn reform(form: Form, g: Given, clear: list[string], approver: bool, org: machine.Org) -> result[machine.Formed[Form], machine.Refusal] total\n  description \"The form given again by the requester or edited by an approver: the fields made empty held to the form, each value given in its place, held to its field and to who gives it, the fields the form does not show dropped, then the form held again and the stages the route comes to for it\"\n{body}"
        ));
        Ok(out)
    }

    /// `clearable`: the names a resubmit or an edit lists to make empty (`clear`), when each is a
    /// field of the form that is given (not computed); else the first that is not, refused as the
    /// reference refuses it (`taken::cleared`). `unclearable` says why as the refusal's text in
    /// English, `unclearable_ja` in Japanese, empty for a field that is given: an optional refusal
    /// made in each arm is written in Rust as a borrow clippy refuses (`Some((&r).clone())`), and an
    /// error made of a text is not. The refusal names the field, unless the form has none of the
    /// name.
    fn clearable(&self) -> String {
        let mut arms: Vec<String> = self.given().map(|x| format!("    {} => \"\"\n", quoted(&x.name))).collect();
        let mut arms_ja = arms.clone();
        for x in self.fields.iter().filter(|x| matches!(x.kind, Kind::Computed { .. } | Kind::Counted)) {
            arms.push(format!("    {} => {}\n", quoted(&x.name), quoted(&format!("`{}` is computed, and is not given", x.shown))));
            arms_ja.push(format!("    {} => {}\n", quoted(&x.name), quoted(&format!("`{}` は計算するフィールドなので、渡せません", x.shown))));
        }
        arms.push("    else => concat(\"the form has no field `\", n, \"`\")\n".to_string());
        arms_ja.push("    else => concat(\"フォームに `\", n, \"` というフィールドはありません\")\n".to_string());
        format!(
            "fn unclearable(n: string) -> string total\n  description \"Why the form does not make the field named empty, as the refusal says it: empty for a field that is given\"\n  match n\n{}\nfn unclearable_ja(n: string) -> string total\n  description \"Why the form does not make the field named empty, in Japanese: empty for a field that is given\"\n  match n\n{}\nfn clearable(names: list[string]) -> result[list[string], machine.Refusal] total\n  description \"The names a resubmit or an edit makes empty, when each is a field of the form that is given\"\n  match names.first(n => unclearable(n: n) != \"\")\n    none => ok(names)\n    some n =>\n      match unclearable(n: n) = concat(\"the form has no field `\", n, \"`\")\n        true => err(refused(text: unclearable(n: n), text_ja: unclearable_ja(n: n), at: none))\n        false => err(refused(text: unclearable(n: n), text_ja: unclearable_ja(n: n), at: n))\n\n",
            arms.concat(),
            arms_ja.concat()
        )
    }

    /// `lost`: the fields that held a value before a resubmit or an edit and hold none after it, by
    /// their names in the JSON, in the order of the form: what the line of the history says
    /// (`cleared`). A field the form holds a value of for every request is never one.
    fn lost(&self) -> String {
        let each: Vec<String> = self
            .given()
            .filter(|x| !x.required)
            .map(|x| format!("name_if(has: {} and not {}, name: {})", present(x.unit.as_ref(), &format!("was.{}", x.name)), present(x.unit.as_ref(), &format!("now.{}", x.name)), quoted(&x.name)))
            .collect();
        let lost = if each.is_empty() { "[]".to_string() } else { format!("[{}].filter_map(x => x)", each.join(", ")) };
        format!("fn lost(was: Form, now: Form) -> list[string] total\n  description \"The fields that held a value before and hold none now, by their names in the JSON: what the history says was made empty or dropped\"\n  {lost}\n\n")
    }

    /// `formed`: the stages the route comes to for the values of the form. Each input of the lowered
    /// route is a field, whether a field is filled in, or an output of a rule the route reads, called
    /// with the fields it takes; each output is a stage of the value it takes, none of a stage the
    /// request does not come to. A function that gives a rule's enum as the route's goes before it.
    pub(super) fn formed(&self, routed: bool) -> Result<String, Text> {
        let m = self.m;
        let s = m.s;
        let head = "fn formed(form: Form) -> result[list[machine.Stage], machine.Refusal] total\n  description \"The stages the route comes to for the values of the form\"\n";
        let route = m.lowered.route.clone().unwrap_or_default();
        if !routed || route.stages.is_empty() {
            return Ok(format!("{head}  ok([])\n\n"));
        }
        let facts = self.route.as_ref();
        let field_of = |n: &str| s.field(n).or_else(|| s.fields().iter().find(|f| f.name.ascii() == n));
        let mut names = Names::default();
        names.take("form");
        let mut helpers = String::new();
        // each helper by what it is of: (the rule's alias, the output's alias), and its name
        let mut made_helpers: Vec<((String, String), String)> = Vec::new();
        let mut lets = Vec::new();
        let mut args = Vec::new();
        let mut called: Vec<(String, String)> = Vec::new();
        // each value of a range the route reads, narrowed to it: the form the ticket holds writes none
        let (mut pre, mut checks, mut bound) = (Vec::new(), Vec::new(), Vec::new());
        // the values a rule the route reads takes that may be empty, each with the name its value is
        // bound to and the refusal where it is empty, as the reference refuses it
        let mut opens: Vec<(String, String, String)> = Vec::new();
        for (reads, col, _) in &route.inputs {
            if route.worked.contains(col) {
                continue;
            }
            match reads.as_slice() {
                [one] if one.ends_with('?') => {
                    let f = field_of(one.trim_end_matches('?')).ok_or_else(|| not_yet(format!("a route that reads `{one}`")))?;
                    let unit = match &f.ty {
                        TypeExpr::Num(u) => Some(u),
                        _ => None,
                    };
                    args.push(format!("{col}: {}", present(unit, &format!("form.{}", f.name.ascii()))));
                }
                [one] => {
                    let x = field_of(one).and_then(|f| self.fields.iter().find(|x| x.name == f.name.ascii())).ok_or_else(|| not_yet(format!("a route that reads `{one}`")))?;
                    if !x.required {
                        // a value that may be empty, as one that may be none (DESIGN 4.9)
                        let v = format!("form.{}", x.name);
                        args.push(format!("{col}: {}", if x.choice.is_some() { self.choice_opt(x, &v)? } else { self.in_range(x, None, &v) }));
                        continue;
                    }
                    let v = narrowed(x, &mut names, &mut pre, &mut checks, &mut bound);
                    args.push(format!("{col}: {}", if x.choice.is_some() { format!("{}(x: {v})", x.fns.choice) } else { v }));
                }
                [alias, out] => {
                    let (_, ascii, _, rf) = self.used.iter().find(|u| u.0 == *alias).ok_or_else(|| not_yet(format!("a route that reads `{alias}.{out}`")))?;
                    let local = match called.iter().find(|(a, _)| a == alias) {
                        Some((_, l)) => l.clone(),
                        None => {
                            let l = names.take(&format!("{ascii}_out"));
                            let mut call = Vec::new();
                            for i in &rf.inputs {
                                let x = field_of(&i.name).or_else(|| field_of(&i.alias)).and_then(|f| self.fields.iter().find(|x| x.name == f.name.ascii()));
                                let x = x.ok_or_else(|| not_yet(format!("the rule `{alias}`, which reads `{}`", i.name)))?;
                                let v = if x.required {
                                    narrowed(x, &mut names, &mut pre, &mut checks, &mut bound)
                                } else {
                                    // the rule takes a value: where the field is empty, the ticket does not
                                    // take the form, as the reference does not
                                    let b = match opens.iter().find(|(of, ..)| *of == format!("form.{}", x.name)) {
                                        Some((_, b, _)) => b.clone(),
                                        None => {
                                            let b = names.take(&format!("{}_v", x.name));
                                            let n = &i.name;
                                            let at = if field_of(n).is_some() { at_field(&x.name) } else { "none".to_string() };
                                            let (en, ja) = (format!("the form's computations read `{n}`, which is not filled in"), format!("フォームの計算が読む `{n}` がありません"));
                                            opens.push((format!("form.{}", x.name), b.clone(), format!("err(refused(text: {}, text_ja: {}, at: {at}))", quoted(&en), quoted(&ja))));
                                            if let Some(c) = x.range_check() {
                                                checks.push(c.over(&b, &at_field(&x.name)));
                                            }
                                            b
                                        }
                                    };
                                    b
                                };
                                let v = match (&i.ty, &x.choice) {
                                    (ColumnType::Enum { .. }, Some((_, Some(_)))) => format!("{}(x: {v})", x.fns.choice_from),
                                    (ColumnType::Enum { .. }, _) => return Err(not_yet(format!("the rule `{alias}`, which reads `{}` as its enum", i.name))),
                                    _ => v,
                                };
                                call.push(format!("{}: {v}", i.name));
                            }
                            lets.push(format!("let {l} = {ascii}({})", call.join(", ")));
                            called.push((alias.clone(), l.clone()));
                            l
                        }
                    };
                    let o = rf.outputs.iter().find(|o| o.name == *out || o.alias == *out).ok_or_else(|| not_yet(format!("the output `{alias}.{out}`")))?;
                    let v = format!("{local}.{}", o.name);
                    // an output that may be none is read as one (DESIGN 4.9)
                    let (oty, opt) = match &o.ty {
                        ColumnType::Opt(t) => (t.as_ref(), true),
                        t => (t, false),
                    };
                    let v = match oty {
                        ColumnType::Enum { name: en, .. } => {
                            // the rule's enum, as the route's copy of it in `choices.rule`
                            let missing = || not_yet(format!("the output `{alias}.{out}`"));
                            let e = rf.enums.iter().find(|e| e.name == *en).ok_or_else(missing)?;
                            let to = facts.and_then(|f| f.inputs.iter().find(|i| i.name == *col)).and_then(|i| match &i.ty {
                                ColumnType::Enum { name, .. } => facts.and_then(|f| f.enums.iter().find(|e| e.name == *name)),
                                ColumnType::Opt(t) => match t.as_ref() {
                                    ColumnType::Enum { name, .. } => facts.and_then(|f| f.enums.iter().find(|e| e.name == *name)),
                                    _ => None,
                                },
                                _ => None,
                            });
                            let to = to.ok_or_else(missing)?;
                            let key = (ascii.clone(), o.alias.clone());
                            let fname = match made_helpers.iter().find(|(k, _)| *k == key) {
                                Some((_, f)) => f.clone(),
                                None => {
                                    let fname = self.names.borrow_mut().take(&format!("out_{ascii}_{}", o.alias));
                                    if opt {
                                        let arms: Vec<String> = e.values.iter().map(|v| format!("          {} => route.{}.{}", v.public, to.public, v.public)).collect();
                                        helpers.push_str(&format!("fn {fname}(x: {ascii}.{}?) -> route.{}? total\n  match x\n    none => none\n    some y =>\n      match y\n{}\n\n", e.public, to.public, arms.join("\n").replace("          ", "        ")));
                                    } else {
                                        let arms: Vec<String> = e.values.iter().map(|v| format!("    {} => route.{}.{}", v.public, to.public, v.public)).collect();
                                        helpers.push_str(&format!("fn {fname}(x: {ascii}.{}) -> route.{} total\n  match x\n{}\n\n", e.public, to.public, arms.join("\n")));
                                    }
                                    made_helpers.push((key, fname.clone()));
                                    fname
                                }
                            };
                            format!("{fname}(x: {v})")
                        }
                        _ => v,
                    };
                    args.push(format!("{col}: {v}"));
                }
                _ => return Err(not_yet(format!("a route that reads `{}`", reads.join(".")))),
            }
        }
        let r = names.take("r");
        let mut stages = Vec::new();
        let mut after = Vec::new();
        let rf = m.lowered.files.iter().find(|f| f.part == Part::Route);
        for (stage, output, values) in &route.stages {
            let optional = facts.and_then(|f| f.outputs.iter().find(|c| c.name == *output)).is_some_and(|c| matches!(c.ty, ColumnType::Opt(_)));
            // a value the lowered rule writes under another name (`after_approver`) is the ticket's
            // by the request's (`after`)
            let back = rf.and_then(|rf| way_of(facts?, rf, output, values, &mut self.names.borrow_mut()));
            if let Some((_, text)) = &back {
                helpers.push_str(text);
            }
            let made = |v: &str| match &back {
                Some((f, _)) => format!("machine.new_stage(name: {}, value: {f}(x: {v}))", quoted(m.names.stage(stage))),
                None => format!("machine.new_stage(name: {}, value: text({v}))", quoted(m.names.stage(stage))),
            };
            if optional {
                let l = names.take(&format!("{output}_stage"));
                let v = names.take("v");
                after.push(format!("let {l} =\n  match {r}.{output}\n    some {v} => [{}]\n    none => []", made(&v)));
                stages.push(l);
            } else {
                stages.push(format!("[{}]", made(&format!("{r}.{output}"))));
            }
        }
        let mut rest = String::new();
        for l in &lets {
            rest.push_str(&indent(l, 0));
        }
        rest.push_str(&format!("let {r} = route({})\n", args.join(", ")));
        for l in &after {
            rest.push_str(&indent(l, 0));
        }
        let all = match stages.split_first() {
            None => "[]".to_string(),
            Some((first, [])) => first.clone(),
            Some((first, more)) => format!("{first}.concat({})", more.join(", ")),
        };
        rest.push_str(&format!("ok({all})\n"));
        let mut body = String::new();
        for p in &pre {
            body.push_str(&format!("  {p}\n"));
        }
        let mut pad = 2;
        for (of, b, refused) in &opens {
            let p = " ".repeat(pad);
            body.push_str(&format!("{p}match {of}\n{p}  none => {refused}\n{p}  some {b} =>\n"));
            pad += 4;
        }
        body.push_str(&chain(&checks, &rest, pad));
        body.push('\n');
        Ok(format!("{helpers}{head}{body}"))
    }
}

/// For a stage's output whose values the lowered route writes under other names than the request's
/// (`after_approver` for the way `after`), the function that gives the request's name of a value
/// (`way_of_<output>`), and its text. None where the route writes every value by the request's name.
fn way_of(facts: &RuleFacts, rf: &LFile, output: &str, values: &[(String, String)], names: &mut Names) -> Option<(String, String)> {
    let back: Vec<(&String, String)> = values.iter().map(|(v, _)| (v, asked(rf, v))).filter(|(v, w)| **v != *w).collect();
    if back.is_empty() {
        return None;
    }
    let c = facts.outputs.iter().find(|c| c.name == output)?;
    let ty = match &c.ty {
        ColumnType::Opt(t) => t.as_ref(),
        t => t,
    };
    let ColumnType::Enum { name, .. } = ty else { return None };
    let e = facts.enums.iter().find(|e| e.name == *name)?;
    let f = names.take(&format!("way_of_{output}"));
    let mut text = format!("fn {f}(x: route.{}) -> string total\n  description \"The way to choose an approver a value of the stage's output names, by the request's name of it\"\n", e.public);
    match back.as_slice() {
        // an enum of one value: kaname writes a `match` of one arm in Rust as one that binds nothing,
        // which clippy refuses, so the name is given back from the value's text
        [(v, w)] if e.values.len() == 1 => text.push_str(&format!("  text(x).replace({}, {})\n", quoted(v), quoted(w))),
        // every value has an arm, and none reads `text(x)`: kaname writes `else => text(x)` in Python
        // as the value of an enum mypy has narrowed, which it reads as `Any` (mypy's `no-any-return`)
        _ => {
            text.push_str("  match x\n");
            for v in &e.values {
                text.push_str(&format!("    {} => {}\n", v.public, quoted(&asked(rf, &v.public))));
            }
        }
    }
    text.push('\n');
    Some((f, text))
}

/// A value of the form the route reads: the field itself, or, for a field of a range, a name it is
/// bound to (`amount_in`), narrowed to the range before the route is read (`pre`, `checks`); once
/// for each field (`bound`).
fn narrowed(x: &Fld, names: &mut Names, pre: &mut Vec<String>, checks: &mut Vec<Over>, bound: &mut Vec<(String, String)>) -> String {
    let Some(c) = x.range_check() else { return format!("form.{}", x.name) };
    if let Some((_, l)) = bound.iter().find(|(n, _)| *n == x.name) {
        return l.clone();
    }
    let l = names.take(&format!("{}_in", x.name));
    pre.push(format!("let {l} = form.{}", x.name));
    checks.push(c.over(&l, &at_field(&x.name)));
    bound.push((x.name.clone(), l.clone()));
    l
}

/// A field as the glue reads it; `rows` the rows field it is a field of, `opened` the alias the glue
/// reads the open days of each calendar by, by the calendar's alias as written.
fn field(m: &Machine, f: &Field, rows: Option<&Field>, opened: &[(String, String)]) -> Result<Fld, Text> {
    let s = m.s;
    let shown = f.name.text.clone();
    let whole = match rows {
        Some(r) => format!("{}.{}", r.name.text, shown),
        None => shown.clone(),
    };
    // the name the words of a check say: a field of a row at its place in the row ([`placed`])
    let named = if rows.is_some() { NAME.to_string() } else { shown.clone() };
    let required = f.optional.is_none() && f.required_when.is_none() && f.shown_when.is_none() && f.approvers_only.is_none() && !(rows.is_some() && f.ty == TypeExpr::Attachment);
    let mut x = Fld {
        name: f.name.ascii().to_string(),
        local: f.name.ascii().to_string(),
        shown: shown.clone(),
        ty: String::new(),
        kind: Kind::Given,
        vk: Vk::Str,
        unit: None,
        required,
        approvers: f.approvers_only.is_some(),
        optional: f.optional.is_some(),
        shown_when: f.shown_when.is_some(),
        required_when: f.required_when.is_some(),
        checks: Vec::new(),
        org: false,
        range: None,
        open: Vec::new(),
        opened: None,
        choice: None,
        fns: Fns::default(),
    };
    if f.computed.is_some() && f.ty == TypeExpr::Date {
        // a date the form counts in business days, held to its range and its calendar as the
        // reference holds it once it is counted
        if rows.is_some() {
            return Err(not_yet(format!("the computed field `{whole}`, a date counted in a row")));
        }
        x.ty = "date".into();
        x.vk = Vk::Date;
        // counted from a value that is empty, it has none (DESIGN 4.9)
        x.required = !crate::lower::may_be_none(s, f);
        x.range = range_check(f, &shown);
        x.kind = Kind::Counted;
        open_in(m, f, &whole, &shown, &mut x, opened)?;
        return Ok(x);
    }
    if f.computed.is_some() {
        let TypeExpr::Num(u) = &f.ty else { return Err(not_yet(format!("the computed field `{whole}`, which is not a number"))) };
        x.ty = kn_type(u);
        x.vk = Vk::Num;
        x.unit = Some(u.clone());
        // reading a value that is empty, it has none (DESIGN 4.9)
        x.required = !crate::lower::may_be_none(s, f);
        // the range the route reads it within, which the form holds it to
        let (lo, hi) = crate::lower::route::reach_or_range(s, f);
        let mut bad = Vec::new();
        if let Some(l) = lo {
            bad.push(format!("{X} < {}", words::written(words::Val::Num(l), &f.ty)));
        }
        if let Some(h) = hi {
            bad.push(format!("{X} > {}", words::written(words::Val::Num(h), &f.ty)));
        }
        let r = crate::lower::route::range_of(f, lo, hi);
        let values = m.lowered.files.iter().find(|lf| lf.part == Part::Values);
        let t = crate::lower::form::out_of_range_said(s, values, f, rows, &whole);
        let (said, said_ja) = (quoted(&t.en), quoted(&t.ja));
        x.kind = Kind::Computed { range: (!bad.is_empty()).then(|| (format!("range {r}"), Check { bad: bad.join(" or "), said, said_ja })) };
        return Ok(x);
    }
    let one_of = |vs: &[String]| Check {
        bad: format!("not ({})", vs.iter().map(|v| format!("{X} = {}", quoted(v))).collect::<Vec<_>>().join(" or ")),
        said: quoted(&format!("the value of `{named}` does not fit the field's type")),
        said_ja: quoted(&format!("`{named}` の値が、フィールドの型に合いません")),
    };
    match &f.ty {
        TypeExpr::Text | TypeExpr::LongText => {
            x.ty = "string".into();
            if let Some((_, cs)) = count_checks(s, f, &named, false) {
                x.checks.extend(cs);
            }
        }
        TypeExpr::Attachment => x.ty = "string".into(),
        TypeExpr::Bool => {
            x.ty = "bool".into();
            x.vk = Vk::Bool;
        }
        TypeExpr::Num(u) => {
            x.ty = kn_type(u);
            x.vk = Vk::Num;
            x.unit = Some(u.clone());
            x.range = range_check(f, &named);
        }
        TypeExpr::Date => {
            x.ty = "date".into();
            x.vk = Vk::Date;
            x.range = range_check(f, &named);
            open_in(m, f, &whole, &named, &mut x, opened)?;
        }
        TypeExpr::Choice => {
            x.ty = "string".into();
            let vs: Vec<String> = f.options.iter().map(|o| o.name.ascii().to_string()).collect();
            x.checks.push(one_of(&vs));
            x.choice = Some((vs, None));
        }
        TypeExpr::ChoiceFrom(p) if p.len() == 2 => {
            x.ty = "string".into();
            if s.ringi(&p[0].text, crate::ast::UseKind::Masters).is_some() {
                let master = m.names.master(&format!("{}.{}", p[0].text, p[1].text)).to_string();
                x.org = true;
                x.checks.push(Check {
                    bad: format!("not machine.has_row(org: org, master: {}, key: {X})", quoted(&master)),
                    said: format!("concat(\"`\", {X}, {}, text(org.day))", quoted(&format!("` of `{named}` is not a row of the master `{}` to choose on ", p[1].text))),
                    said_ja: format!("concat({}, {X}, \"` は、\", text(org.day), {})", quoted(&format!("`{named}` の `")), quoted(&format!(" に選べるマスタ `{}` の行ではありません", p[1].text))),
                });
            } else {
                let cx = crate::lower::context(s, m.suite);
                let e = cx.rule(&p[0].text).and_then(|r| r.facts.enums.iter().find(|e| e.name == p[1].text || e.public == p[1].text).cloned());
                let Some(e) = e else { return Err(not_yet(format!("the field `{whole}`, a choice from a rule rulec does not answer for"))) };
                let vs: Vec<String> = e.values.iter().map(|v| v.public.clone()).collect();
                x.checks.push(one_of(&vs));
                x.choice = Some((vs, Some((p[0].text.clone(), e.name.clone()))));
            }
        }
        TypeExpr::Person => {
            x.ty = "string".into();
            x.org = true;
            x.checks.push(Check {
                bad: format!("not machine.in_org(org: org, id: {X})"),
                said: format!("concat({X}, {}, text(org.version))", quoted(&format!(" of `{named}` is not in the organization from "))),
                said_ja: format!("concat({}, {X}, \" は、\", text(org.version), \" からの組織にいません\")", quoted(&format!("`{named}` の "))),
            });
        }
        TypeExpr::Team => {
            x.ty = "string".into();
            x.org = true;
            x.checks.push(Check {
                bad: format!("not machine.has_team(org: org, id: {X})"),
                said: format!("concat(\"the team `\", {X}, {}, text(org.version))", quoted(&format!("` of `{named}` is not in the organization from "))),
                said_ja: format!("concat({}, {X}, \"` は、\", text(org.version), \" からの組織にありません\")", quoted(&format!("`{named}` のチーム `"))),
            });
        }
        TypeExpr::Rows if rows.is_none() => {
            if !x.required {
                return Err(not_yet(format!("the rows `{whole}`, which may be empty")));
            }
            let mut children = Vec::new();
            for c in &f.children {
                if c.computed.is_some() {
                    // a computation of a row is worked out by the copy, and is no value of the form
                    if !matches!(c.ty, TypeExpr::Num(_)) {
                        return Err(not_yet(format!("the computed field `{whole}.{}`, which is not a number", c.name.text)));
                    }
                    continue;
                }
                let y = field(m, c, Some(f), opened)?;
                x.org |= y.org;
                children.push(y);
            }
            x.ty = format!("list[{}]", row_records(&x.name).1);
            x.vk = Vk::Rows;
            x.kind = Kind::Rows { children, count: count_checks(s, f, &shown, true) };
        }
        other => return Err(not_yet(format!("the field `{whole}` of the type `{}`", other.word()))),
    }
    Ok(x)
}

/// What a date `open in` a calendar is held to after its range, as the reference holds it
/// (`taken::holds`): a day outside the calendar's data, then a day the calendar is closed on, which
/// the glue reads as a day the calendar's next open day comes after (`opened_<calendar>.cal`). The
/// days the glue reads it over are the field's range within the calendar's data.
fn open_in(m: &Machine, f: &Field, whole: &str, named: &str, x: &mut Fld, opened: &[(String, String)]) -> Result<(), Text> {
    let Some(c) = &f.open_in else { return Ok(()) };
    let cal = &c.text;
    let cx = crate::lower::context(m.s, m.suite);
    let Some(used) = cx.calendars.iter().find(|u| u.alias == *cal) else { return Err(not_yet(format!("the field `{whole}`, open in the calendar `{cal}`, which is not read"))) };
    let Some((_, facts)) = m.calendars.iter().find(|(a, _)| a == cal) else { return Err(not_yet(format!("the field `{whole}`, open in the calendar `{cal}`, which koyomi does not answer for"))) };
    let (lo, hi) = words::ends(f);
    let (Some(lo), Some(hi)) = (lo.filter(|r| r.is_int()), hi.filter(|r| r.is_int())) else { return Err(not_yet(format!("the field `{whole}`, open in a calendar with no range"))) };
    let (lo, hi) = (lo.num as i64, hi.num as i64);
    let (dlo, dhi) = facts.data;
    let n = named;
    // a day outside the calendar's data, where the field's range goes past it
    let mut bad = Vec::new();
    if lo < dlo {
        bad.push(format!("{X} < {}", crate::lex::date_text(dlo)));
    }
    if hi > dhi {
        bad.push(format!("{X} > {}", crate::lex::date_text(dhi)));
    }
    if !bad.is_empty() {
        let (a, b) = (crate::lex::date_text(dlo), crate::lex::date_text(dhi));
        let after = format!("` is outside the data of the calendar `{cal}` ({a} to {b})");
        let after_ja = format!("` は、カレンダー `{cal}` のデータ（{a} から {b}）の外です");
        x.open.push(Check { bad: bad.join(" or "), said: around("`", &format!("text({X})"), &after), said_ja: around("`", &format!("text({X})"), &after_ja) });
    }
    let (lo, hi) = (lo.max(dlo), hi.min(dhi));
    if lo > hi {
        return Err(not_yet(format!("the field `{whole}`, whose range is outside the data of the calendar `{cal}`")));
    }
    let Some((_, alias)) = opened.iter().find(|(c, _)| c == cal) else { return Err(not_yet(format!("the field `{whole}`, open in the calendar `{cal}`, which is not read"))) };
    x.open.push(Check {
        bad: format!("{alias}.opened(given: {X}) > {X}"),
        said: around("`", &format!("text({X})"), &format!("` is a day the calendar `{cal}` is closed on, and `{n}` is `open in {cal}`")),
        said_ja: around("`", &format!("text({X})"), &format!("` は、カレンダー `{cal}` の休みの日です。`{n}` は `open in {cal}` です")),
    });
    x.opened = Some(Opened { file: used.file.clone(), alias: alias.clone(), calendar: cal.clone(), path: used.path.clone(), lo, hi });
    Ok(())
}

/// Whether holding the field's values reads the part of the organization.
fn needs_org(x: &Fld) -> bool {
    x.org && !x.approvers
}

/// The type of a value given for a field: rows as given.
fn given_type(x: &Fld) -> String {
    match &x.kind {
        Kind::Rows { .. } => format!("list[{}]", x.fns.given_rec),
        _ => x.ty.clone(),
    }
}

/// The type of a value `form_of` takes for a field: rows as held, a value the form holds for every
/// request with no range, any other optional.
fn formed_type(x: &Fld) -> String {
    match &x.kind {
        Kind::Rows { .. } => format!("list[{}]", x.fns.checked_rec),
        _ if x.required => x.ty.clone(),
        _ => format!("{}?", x.ty),
    }
}

/// The type a field's value is held as in the form a ticket holds (or in a row of it): of no range,
/// optional where the form does not hold a value of it for every request.
fn wire_type(x: &Fld) -> String {
    match &x.kind {
        Kind::Rows { .. } => format!("list[{}]", x.fns.row_rec),
        _ if x.required => format!("{}{}", x.ty, x.open()),
        _ => format!("{}?{}", x.ty, x.open()),
    }
}

/// The type a field's value is held as in a row held to what the form declares: with its range when
/// the row holds a value of it for every request, else optional.
fn held_type(x: &Fld) -> String {
    if !x.required {
        return format!("{}?{}", x.ty, x.open());
    }
    let bound = match &x.kind {
        Kind::Rows { count, .. } => count.as_ref().map(|(c, _)| format!("  {c}")),
        Kind::Computed { range } => range.as_ref().map(|(r, _)| format!("  {r}")),
        Kind::Given | Kind::Counted => x.range.as_ref().map(|(r, _)| format!("  {r}")),
    };
    format!("{}{}", x.ty, bound.unwrap_or_else(|| x.open()))
}

/// `given_<field>`: the value given for a field at the top of the form, held to what the form
/// declares of it. An approvers' field takes none from the requester.
fn given_fn(x: &Fld) -> String {
    let n = &x.name;
    let ty = &x.ty;
    let org = if needs_org(x) { ", org: machine.Org" } else { "" };
    let ret = if x.required { ty.clone() } else { format!("{ty}?") };
    let mut out = format!("fn {}(v: {ty}?{}{org}) -> result[{ret}, machine.Refusal] total\n  description \"The value given for `{n}`, held to what the form declares of it\"\n  match v\n", x.fns.given, x.open());
    let at = at_field(n);
    if x.approvers {
        out.push_str(&format!("    none => ok(none)\n    some x => {}\n\n", approvers_only(x).refused()));
        return out;
    }
    if x.required {
        out.push_str(&format!("    none => {}\n", unfilled(x).refused()));
    } else {
        out.push_str("    none => ok(none)\n");
    }
    out.push_str("    some x =>\n");
    let checks: Vec<Over> = x.checks.iter().chain(x.range.iter().map(|(_, c)| c)).chain(&x.open).map(|c| c.over("x", &at)).collect();
    out.push_str(&chain(&checks, "ok(x)", 6));
    out.push('\n');
    out
}

/// The refusal of a value the requester gives for an approvers' field (`bad`: true where it is
/// written as a branch of its own).
fn approvers_only(x: &Fld) -> Over {
    Over {
        bad: "not approver".to_string(),
        en: quoted(&format!("`{}` is a field of the approvers only, which the requester does not fill in", x.shown)),
        ja: quoted(&format!("`{}` は承認者だけのフィールドなので、申請者は入れません", x.shown)),
        at: at_field(&x.name),
    }
}

/// The refusal of a field the form asks for on every request, given no value.
fn unfilled(x: &Fld) -> Over {
    Over {
        bad: "true".to_string(),
        en: quoted(&format!("`{}` is not filled in, and the form asks for it", x.shown)),
        ja: quoted(&format!("`{}` がありません。必須のフィールドなので、入れてください", x.shown)),
        at: at_field(&x.name),
    }
}

/// `regiven_<field>`: the value an event gives for a field in place of the one the ticket holds
/// (`was`, kept where the event gives none, none where it makes the field empty: `clear`), held to
/// who gives it, then to what the form declares of it, then to `clear`, which does not name a field
/// given a value; a field made empty held to who gives it, and refused where the form asks for it
/// on every request, as one not filled in. `editable`: whether `approvers edit` names the field.
fn regiven_fn(x: &Fld, editable: bool) -> String {
    let n = &x.name;
    let org = if x.org { ", org: machine.Org" } else { "" };
    let org_a = if x.org { ", org: org" } else { "" };
    let (vty, ret) = (given_type(x), formed_type(x));
    let at = at_field(n);
    let mut who: Vec<Over> = Vec::new();
    if x.approvers {
        who.push(approvers_only(x));
    } else if !editable {
        who.push(Over {
            bad: "approver".to_string(),
            en: quoted(&format!("`{}` is not a field approvers may edit (it is not in `approvers edit`)", x.shown)),
            ja: quoted(&format!("`{}` は、承認者が直せるフィールドではありません（`approvers edit` に無い）", x.shown)),
            at: at.clone(),
        });
    }
    let both = Over {
        bad: "true".to_string(),
        en: quoted(&format!("`{}` is given a value, and is in `clear` too", x.shown)),
        ja: quoted(&format!("`{}` に値を渡していて、`clear` にも挙げています", x.shown)),
        at: at.clone(),
    }
    .refused();
    let mut checks = who.clone();
    let (kept, then) = match &x.kind {
        // the rows the ticket holds are held to the form again: the copy reads them held
        Kind::Rows { children, .. } => {
            let (given, held) = (&x.fns.given_rec, &x.fns.held);
            let cells: Vec<String> = children.iter().map(|c| format!("{}: y.{}", c.name, c.name)).collect();
            let cells = if cells.is_empty() { "nothing: none".to_string() } else { cells.join(", ") };
            (format!("{held}(gs: was.map(y => {given} {{ {cells} }}){org_a})"), format!("{held}(gs: x{org_a})"))
        }
        _ => {
            checks.extend(x.checks.iter().chain(x.range.iter().map(|(_, c)| c)).chain(&x.open).map(|c| c.over("x", &at)));
            ("ok(was)".to_string(), "ok(x)".to_string())
        }
    };
    let then = format!("match clear\n  true => {both}\n  false => {then}\n");
    // a field the form holds a value of for every request is not made empty
    let empty = if x.required { unfilled(x).refused() } else { "ok(none)".to_string() };
    format!(
        "fn {}(v: {vty}?{}, was: {}, approver: bool, clear: bool{org}) -> result[{ret}, machine.Refusal] total\n  description \"The value given for `{n}` again, held to who gives it and to what the form declares of it; the one the ticket holds where none is given, and none where the field is made empty\"\n  match v\n    none =>\n      match clear\n        false => {kept}\n        true =>\n{}    some x =>\n{}\n",
        x.fns.regiven,
        x.open(),
        wire_type(x),
        chain(&who, &empty, 10),
        chain(&checks, &then, 6)
    )
}

/// The functions of a rows field: `cell_<rows>_<field>` for each field of a row whose value is held
/// to something, `row_<rows>` for a row, `add_<rows>` and `no_<rows>` to walk the rows given, and
/// `given_<rows>`, which holds the count of the rows first, as the reference does.
fn rows_fns(x: &Fld, children: &[Fld], count: Option<&(String, Vec<Check>)>) -> String {
    let r = &x.name;
    let f = &x.fns;
    let (given, row) = (&f.given_rec, &f.checked_rec);
    let org_p = if x.org { ", org: machine.Org" } else { "" };
    let org_a = if x.org { ", org: org" } else { "" };
    let mut out = String::new();
    for c in children {
        // a cell's refusal names it at its place in the row, as the place the refusal names
        let checks: Vec<Over> = c.checks.iter().chain(c.range.iter().map(|(_, k)| k)).chain(&c.open).map(|k| placed(k.over("x", &at_cell(r, &c.name)), &x.shown, &c.shown)).collect();
        if checks.is_empty() {
            continue;
        }
        let corg = if c.org { ", org: machine.Org" } else { "" };
        out.push_str(&format!(
            "fn {}(v: {ty}?{}, no: number range >=1 <=1_000_001{corg}) -> result[{ty}?, machine.Refusal] total\n  description \"The value given for `{n}` in a row of `{r}`, held to what the form declares of it\"\n  match v\n    none => ok(none)\n    some x =>\n{}\n",
            c.fns.cell,
            c.open(),
            chain(&checks, "ok(x)", 6),
            n = c.name,
            ty = c.ty
        ));
    }
    // a row: each value held, then each field it must have unwrapped in turn, then each range narrowed
    let mut names = Names::default();
    for n in ["g", "no", "org"] {
        names.take(n);
    }
    let mut body = String::new();
    let mut vals: Vec<String> = Vec::new();
    for c in children {
        if c.checks.is_empty() && c.range.is_none() && c.open.is_empty() {
            vals.push(format!("g.{}", c.name));
            continue;
        }
        let l = names.take(&format!("{}_c", c.name));
        let corg = if c.org { ", org: org" } else { "" };
        body.push_str(&format!("let {l} = try {}(v: g.{}, no: no{corg})\n", c.fns.cell, c.name));
        vals.push(l);
    }
    let mut fields = Vec::new();
    let mut ranges = Vec::new();
    let mut depth = 0;
    for (c, v) in children.iter().zip(&vals) {
        if !c.required {
            fields.push(format!("{}: {v}", c.name));
            continue;
        }
        let u = names.take(&format!("{}_v", c.name));
        let p = "  ".repeat(depth);
        let none = placed(
            Over {
                bad: "true".to_string(),
                en: quoted(&format!("`{NAME}` is not filled in, and the form asks for it")),
                ja: quoted(&format!("`{NAME}` がありません。必須のフィールドなので、入れてください")),
                at: at_cell(r, &c.name),
            },
            &x.shown,
            &c.shown,
        );
        body.push_str(&format!("{p}match {v}\n{p}  none => {}\n{p}  some {u} =>\n", none.refused()));
        depth += 2;
        if let Some((_, k)) = &c.range {
            ranges.push(placed(k.over(&u, &at_cell(r, &c.name)), &x.shown, &c.shown));
        }
        fields.push(format!("{}: {u}", c.name));
    }
    let made = if fields.is_empty() { format!("ok({row} {{ nothing: none }})") } else { format!("ok({row} {{ {} }})", fields.join(", ")) };
    body.push_str(&chain(&ranges, &made, depth * 2));
    out.push_str(&format!(
        "fn {}(g: {given}, no: number range >=1 <=1_000_001{org_p}) -> result[{row}, machine.Refusal] total\n  description \"A row of `{r}` as given, held to what the form declares of its fields\"\n{}\n",
        f.row,
        indent(&body, 2)
    ));
    out.push_str(&format!(
        "fn {add}(acc: result[list[{row}], machine.Refusal], g: {given}{org_p}) -> result[list[{row}], machine.Refusal] total\n  match acc\n    err e => err(e)\n    ok xs =>\n      match {one}(g: g, no: min(xs.count(), 1_000_000) + 1{org_a})\n        err e => err(e)\n        ok x => ok(concat(xs, [x]))\n\n",
        add = f.add,
        one = f.row
    ));
    out.push_str(&format!("fn {}() -> result[list[{row}], machine.Refusal] total\n  ok([])\n\n", f.no));
    let checks: Vec<Over> = count.map(|(_, cs)| cs.iter().map(|c| c.over("gs", &at_field(r))).collect()).unwrap_or_default();
    out.push_str(&format!(
        "fn {}(gs: list[{given}]{org_p}) -> result[list[{row}], machine.Refusal] total\n  description \"Rows of `{r}`, held to what the form declares of them: their count, then each row\"\n{}\n",
        f.held,
        chain(&checks, &format!("reduce(gs, {}(), (acc, g) => {}(acc: acc, g: g{org_a}))", f.no, f.add), 2)
    ));
    out.push_str(&format!(
        "fn {}(v: list[{given}]?{org_p}) -> result[list[{row}], machine.Refusal] total\n  description \"The rows given for `{r}`, held to what the form declares of them\"\n  match v\n    none => {}\n    some gs => {}(gs: gs{org_a})\n\n",
        f.given,
        unfilled(x).refused(),
        f.held
    ));
    out
}

/// A function that gives a choice's value as a rule's enum: each value the field takes, by its name
/// in the JSON, as the enum's value the lowered files write for it (`vs`, each with the enum's
/// value). The field takes no other (`given_…`), and the last is the rest.
fn choice_fn(name: &str, ty: &str, vs: &[(String, String)], enum_values: &[String]) -> Result<String, Text> {
    if let Some((v, _)) = vs.iter().find(|(_, e)| !enum_values.contains(e)) {
        return Err(not_yet(format!("the value `{v}`, which the rule's enum does not have")));
    }
    let ((_, last), rest) = vs.split_last().ok_or_else(|| not_yet("a choice of no value".to_string()))?;
    let mut out = format!("fn {name}(x: string) -> {ty} total\n  match x\n");
    for (v, e) in rest {
        out.push_str(&format!("    {} => {ty}.{e}\n", quoted(v)));
    }
    out.push_str(&format!("    else => {ty}.{last}\n\n"));
    Ok(out)
}

/// The name the request gives an output or an input of a lowered file (`days` for `days_value`).
fn asked(lf: &LFile, given: &str) -> String {
    lf.renamed.iter().find(|(g, _)| g == given).map(|(_, w)| w.clone()).unwrap_or_else(|| given.to_string())
}

/// A rule's output as a type of kaname: a number with no range, a truth, a date, a text.
fn column_type(t: &ColumnType) -> String {
    match t {
        ColumnType::Bool => "bool".into(),
        ColumnType::Date { .. } => "date".into(),
        ColumnType::Str | ColumnType::Enum { .. } => "string".into(),
        ColumnType::Num { written, .. } => written.clone(),
        ColumnType::Opt(t) => format!("{}?", column_type(t)),
    }
}

/// The type of a number of the unit as the glue writes it: as the request writes it, but a rate
/// with no step as one of 1%, the step the JSON counts it in (`words::step`), for kaname takes no
/// rate without a step (its E108).
fn kn_type(u: &Unit) -> String {
    match (&u.dim, u.step) {
        (Dim::Rate, None) => "rate[step 1%]".to_string(),
        _ => crate::ast::unit_type(u),
    }
}

/// The unit of a rule's output that is a rate, which a computed field of the form is: what the glue
/// writes its type with, and rounds it to.
fn rate_of(t: &ColumnType) -> Option<&Unit> {
    match t {
        ColumnType::Num { unit: Some(u), .. } if u.dim == Dim::Rate => Some(u),
        _ => None,
    }
}

/// The word the functions of a rate's step are named by: the step in percent, `p` for the point
/// (`rate_steps_0p25` for `rate[step 0.25%]`, `rate_steps_1` for `rate`, whose step is 1%).
fn rate_tag(u: &Unit) -> String {
    crate::lex::decimal(words::step(u).mul(Rat::int(100))).replace('.', "p")
}

/// The range the glue writes after a rate's type where it writes none of the field's: every value
/// the JSON can hold, the whole of int64 in the rate's steps (`range >=-2_305_843_009_213_693_952%
/// <=2305843009213693951.75%` for `rate[step 0.25%]`). kaname reads a rate with no range as one from
/// 0% to 100% and refuses any other at its door, where the reference holds a rate to the field's
/// range alone, as any number. Nothing for any other unit, whose range is the whole of int64 where
/// none is written.
fn rate_open(u: Option<&Unit>) -> String {
    let Some(u) = u.filter(|u| u.dim == Dim::Rate) else { return String::new() };
    let end = |n: i64| words::number(Rat::int(i128::from(n)).mul(words::step(u)), u);
    format!("  range >={} <={}", end(i64::MIN), end(i64::MAX))
}

/// Whether `e`, a value of the unit that may be none, is filled in: `machine.present`, but for a
/// rate the glue's `rate_present_<step>` ([`rate_fns`]).
pub(super) fn present(u: Option<&Unit>, e: &str) -> String {
    match u.filter(|u| u.dim == Dim::Rate) {
        Some(u) => format!("rate_present_{}(v: {e})", rate_tag(u)),
        None => format!("machine.present(v: {e})"),
    }
}

/// `e`, a value of the unit that may be none, where `keep` holds, and none where it does not:
/// `machine.kept`, but for a rate the glue's `rate_kept_<step>` ([`rate_fns`]).
fn kept(u: Option<&Unit>, e: &str, keep: &str) -> String {
    match u.filter(|u| u.dim == Dim::Rate) {
        Some(u) => format!("rate_kept_{}(v: {e}, keep: {keep})", rate_tag(u)),
        None => format!("machine.kept(v: {e}, keep: {keep})"),
    }
}

/// The glue's `grouped`: a number as the reference writes it in a refusal (`words::number`), from what
/// kaname's `text` writes and the unit it ends in: a whole number's digits in threes from the right
/// with `_` between them, where there are more than four (`123456JPY` is `123_456JPY`); a number
/// with decimals as it is. int64 has 19 digits at most, seven threes.
const GROUPED: &str = "fn grouped(t: string, unit: string) -> string total\n  description \"A number as the reference writes it in a refusal: the digits of a whole number in threes from the right with `_` between them, where there are more than four\"\n  let n = t.take(t.count() - unit.count())\n  let digits = n.replace(\"-\", \"\")\n  if digits.count() <= 4 or n.contains(\".\") or n.contains(\" \")\n    t\n  else\n    let p = digits.pad_start(21, \" \")\n    let g = concat(p.take(3), \"_\", p.slice(3, 6), \"_\", p.slice(6, 9), \"_\", p.slice(9, 12), \"_\", p.slice(12, 15), \"_\", p.slice(15, 18), \"_\", p.drop(18))\n    concat(n.take(n.count() - digits.count()), g.replace(\"_\", \" \").trim_start().replace(\" \", \"_\"), unit)\n\n";

/// The functions of a rate's step. `rate_steps_<step>`: a rate as the JSON holds it, the whole
/// number of its steps (a rate over a rate is a rate in kaname, and a number is what a flow's
/// argument is). `rate_text_<step>`: a rate as the reference writes it in a refusal (`31.25%`,
/// `-0.5%`, `100%`, `12_345%`: the digits of its decimals that are not zeros, a whole number's in
/// threes, [`GROUPED`]), which kaname's `text` does not write; a value past what int64 holds in the last digit of its decimals, which no form takes, is
/// written as the number of its steps (`922337203685477581 × 0.25%`). `rate_present_<step>` and
/// `rate_kept_<step>`: the ticket module's `present` and `kept` for a rate, whose parameters kaname
/// would read as rates from 0% to 100%, as it reads the type of a generic function's parameter
/// with no range.
fn rate_fns(u: &Unit) -> String {
    let (tag, ty, open) = (rate_tag(u), kn_type(u), rate_open(Some(u)));
    let step = words::number(words::step(u), u);
    // the step in the last of its decimals: 25 and 2 for 0.25%
    let (mut k, mut d) = (words::step(u).mul(Rat::int(100)), 0usize);
    while !k.is_int() && d < 18 {
        k = k.mul(Rat::int(10));
        d += 1;
    }
    let k = k.num;
    let mut out = format!(
        "fn rate_steps_{tag}(v: {ty}{open}) -> number total\n  description \"A rate in steps of {step} as the JSON holds it: the whole number of its steps\"\n  let n : number = v / {step}\n  n\n\n"
    );
    out.push_str(&format!(
        "fn rate_present_{tag}(v: {ty}?{open}) -> bool total\n  description \"Whether a rate in steps of {step} is filled in\"\n  match v\n    none => false\n    some x => true\n\nfn rate_kept_{tag}(v: {ty}?{open}, keep: bool) -> {ty}? total\n  description \"A rate in steps of {step} where it is kept, none where it is dropped\"\n  match keep\n    true => v\n    false => none\n\n"
    ));
    out.push_str(&format!("fn rate_text_{tag}(v: {ty}{open}) -> string total\n  description \"A rate in steps of {step} as the reference writes it in a refusal: in percent, with the digits of its decimals that are not zeros\"\n  let n : number = v / {step}\n"));
    let text = |pad: &str| {
        let t = if k == 1 { "text(n)".to_string() } else { format!("text(n * {k})") };
        if d == 0 {
            return format!("{pad}grouped(t: concat({t}, \"%\"), unit: \"%\")\n");
        }
        format!(
            "{pad}let t = {t}\n{pad}let digits = t.replace(\"-\", \"\").pad_start({w}, \"0\")\n{pad}let whole = digits.take(digits.count() - {d})\n{pad}let frac = digits.drop(digits.count() - {d}).replace(\"0\", \" \").trim_end().replace(\" \", \"0\")\n{pad}grouped(t: concat(t.take(t.count() - t.replace(\"-\", \"\").count()), concat(whole, \".\", frac).replace(\".\", \" \").trim_end().replace(\" \", \".\"), \"%\"), unit: \"%\")\n",
            w = d + 1
        )
    };
    if k == 1 {
        out.push_str(&text("  "));
    } else {
        let limit = words::grouped(i128::from(i64::MAX) / k);
        out.push_str(&format!("  if n < -{limit} or n > {limit}\n    concat(text(n), \" × {step}\")\n  else\n"));
        out.push_str(&text("    "));
    }
    out.push('\n');
    out
}

/// A record of kaname, its fields lined up; one with no field holds `nothing`.
fn record(name: &str, fields: impl Iterator<Item = (String, String)>) -> String {
    let mut fields: Vec<(String, String)> = fields.collect();
    if fields.is_empty() {
        fields.push(("nothing".to_string(), "bool?".to_string()));
    }
    let width = fields.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
    let mut out = format!("record {name}\n");
    for (n, t) in &fields {
        out.push_str(&format!("  {n:width$} : {t}\n"));
    }
    out.push('\n');
    out
}
