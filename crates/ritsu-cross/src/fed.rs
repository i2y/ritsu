//! X17 (DESIGN 7.2, 7.16): what can come from where a place says its value comes from (`fed by`),
//! held to what the place takes. Hand-written code carries the value between two worlds, and no
//! generated code reads the form. Each world answers for itself: the place's language says what it
//! takes (`Feeds::feeds`), and the source's says what can come from it, through its port (a rule's
//! output, a date, an account, a workflow's or a module's place) or through the one reader of its
//! format (a `.proto`, an OpenAPI document, a JSON Schema). Three answers (P5):
//!
//! - It holds: everything that can come from the source is taken (nothing is said).
//! - It does not, with an example (E207): a value, and the input that gives it where a rule has one.
//! - It cannot be decided (W207), with the reason.
//!
//! What a source leaves open is read by who is answering. A contract lets through what it does not
//! rule out, as E122 reads it, so its open end is an example; so does a book, up to chobo's limit of
//! 2⁶³ − 1. A rule's output with an end rulec does not count, or an end no input from its vectors
//! reaches, is undecided. A source that is not there is E208; a file its own language does not pass
//! is that language's to say, and undecided here. Nothing holds the hand-written code to carrying
//! the value, which is what a page says of it.

use crate::{Border, Borders, Came, Check, Side};
use ritsu_base::diag::Diag;
use ritsu_base::document::{self, Documents, Loaded};
use ritsu_base::json::Json;
use ritsu_base::naming::{Name, Tool};
use ritsu_base::openapi::{self, Number, Schema};
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_ports::{Answer, Books, Carried, ColumnType, Dates, DaySet, Fed, Finding, Found, OutputValues, Ports, RuleFacts, Rules, Value, Values, day_text};
use ritsu_project::{Joined, Project};
use ritsu_proto::validate::{self, Ints};
use ritsu_proto::{Label, Protos, Resolved, Type};
use ritsu_units::{Dim, Unit};
use std::path::{Path, PathBuf};

/// The most a chobo balance can be, and the least: 2⁶³ − 1 in the unit's smallest step.
const BOOK_LIMIT: i128 = i64::MAX as i128;

/// The numbers that can come from a source.
enum Numbers {
    /// A rule's output: what rulec counts of it, with an input for the values its vectors reach.
    Output(Found<OutputValues>),
    /// What a contract or a book lets through: whatever it does not rule out can come.
    Passes(Ints),
    /// A range a workflow or a module writes on a place: any number in it can come, and an end it
    /// does not write is one nothing says.
    Declared(Option<i128>, Option<i128>),
}

/// What can come from a source.
enum Gives {
    Bool,
    /// A string of a language.
    Str,
    /// A string of a contract: the ones it lists, or None for any. An enum of a `.proto`, by the names
    /// a rule calls its values, is a list; so is a date, which travels as a string (a contract says
    /// nothing of which days, as with E122).
    Wire(Option<Vec<String>>),
    /// Each day of a koyomi date, with where to ask for the input that gives a day.
    Days(Found<DaySet>, Option<(PathBuf, String)>),
    /// Days in a range: ends a rule says it can name (or None), or the range a workflow or a module
    /// writes on a place, where any day in it can come.
    DayRange { min: Option<i64>, max: Option<i64>, declared: bool },
    /// An enum, by its public names: all of them, and the ones its language says come, each with an
    /// input for a rule's.
    Enum { all: Vec<String>, reached: Found<Vec<(String, Values)>> },
    /// A number, with the spelling and the unit it is counted in; None for the integer of a contract,
    /// which is read as what goes on the wire of the place (as with E122, its unit is not compared).
    Num { unit: Option<(String, Option<Unit>)>, values: Numbers },
    /// A contract's fraction (`number`, `double`): E122 does not compare it.
    Frac,
    /// It can also come as no value at all, and how.
    Opt(Box<Gives>, Text),
    /// Anything a value is not compared with (an object, a list, a message), as its source calls it.
    Other(String),
}

/// What a source comes to, when asked.
enum Asked {
    Gives(Gives),
    /// It is not there (E208).
    Missing(Text),
    /// Its language does not answer (W207): a file it does not pass.
    Unanswered(Text),
}

/// An example of what a place does not take.
struct Against {
    /// The value; None where the types differ, or the source lets through any string.
    value: Option<String>,
    /// The input that gives it, where a rule or a dates file has one.
    at: Option<String>,
    /// What is wrong, for a note.
    why: Option<Text>,
}

impl Against {
    fn value(v: impl Into<String>, at: Option<String>) -> Against {
        Against { value: Some(v.into()), at, why: None }
    }

    fn none(why: Text) -> Against {
        Against { value: None, at: None, why: Some(why) }
    }
}

/// The ends of a range as a line writes them: `>=1 <=50`.
fn ends<T: std::fmt::Display>(lo: Option<T>, hi: Option<T>) -> String {
    [lo.map(|v| format!(">={v}")), hi.map(|v| format!("<={v}"))].into_iter().flatten().collect::<Vec<_>>().join(" ")
}

/// A spelling and what is written after it.
fn with_ends(word: &str, range: String) -> String {
    if range.is_empty() { word.to_string() } else { format!("{word} {range}") }
}

/// What a place takes, for a message and the record.
fn takes_text(c: &Carried) -> String {
    match c {
        Carried::Bool => "bool".into(),
        Carried::Str => "string".into(),
        Carried::Date { min, max } => with_ends("date", ends(min.map(day_text), max.map(day_text))),
        Carried::Days(set) => match (set.first(), set.last()) {
            (Some(a), Some(b)) => format!("date ({} days from {} to {})", set.len(), day_text(*a), day_text(*b)),
            _ => "date (no day)".into(),
        },
        Carried::Enum { name, values } => format!("enum {name} ({})", values.join(", ")),
        Carried::Num { written, min, max, .. } => with_ends(written, ends(*min, *max)),
        Carried::Opt(t) => format!("{}?", takes_text(t)),
        Carried::Other(w) => w.clone(),
    }
}

/// The integers a contract or a book lets through: `>=1 <=50`, `1, 2, 3`.
fn ints_text(s: &Ints) -> String {
    let one = |(a, b): &(Option<i128>, Option<i128>)| match (a, b) {
        (Some(a), Some(b)) if a == b => a.to_string(),
        (a, b) => ends(*a, *b),
    };
    s.0.iter().map(one).collect::<Vec<_>>().join(", ")
}

/// What can come from a source, for a message and the record.
fn gives_text(g: &Gives) -> String {
    match g {
        Gives::Bool => "bool".into(),
        Gives::Str => "string".into(),
        Gives::Wire(None) => "any string".into(),
        Gives::Wire(Some(list)) => format!("string ({})", list.join(", ")),
        Gives::Days(Found::Value(set), _) => match (set.first(), set.last()) {
            (Some(a), Some(b)) => format!("date ({} days from {} to {})", set.len(), day_text(*a), day_text(*b)),
            _ => "date (no day)".into(),
        },
        Gives::Days(Found::Undecided(_), _) => "date (days not computed)".into(),
        Gives::DayRange { min, max, .. } => with_ends("date", ends(min.map(day_text), max.map(day_text))),
        Gives::Enum { reached: Found::Value(vs), .. } => format!("enum ({})", vs.iter().map(|(v, _)| v.as_str()).collect::<Vec<_>>().join(", ")),
        Gives::Enum { all, .. } => format!("enum (of {})", all.join(", ")),
        Gives::Num { unit, values } => {
            let word = unit.as_ref().map_or("integer", |(w, _)| w.as_str());
            match values {
                Numbers::Output(Found::Value(o)) => with_ends(word, ends(o.min, o.max)),
                Numbers::Output(Found::Undecided(_)) => word.to_string(),
                Numbers::Passes(s) => with_ends(word, ints_text(s)),
                Numbers::Declared(a, b) => with_ends(word, ends(*a, *b)),
            }
        }
        Gives::Frac => "a fraction".into(),
        Gives::Opt(g, _) => format!("{}?", gives_text(g)),
        Gives::Other(w) => w.clone(),
    }
}

/// A rule's input as a message says it: `a = 1, b = "x"`.
fn inputs_text(v: &Values) -> Option<String> {
    let shown = |x: &Value| match x {
        Value::Int(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Str(s) => format!("\"{s}\""),
        Value::Date(d) | Value::Enum(d) => d.clone(),
        Value::None => "none".into(),
        Value::List(xs) => format!("[{} elements]", xs.len()),
    };
    (!v.is_empty()).then(|| v.iter().map(|(n, x)| format!("{n} = {}", shown(x))).collect::<Vec<_>>().join(", "))
}

/// Whether a value counted one way is counted the same way: the same unit, step and tax. A count
/// with a name and nothing else is a number, as dandori and kaname read it, and two counts are the
/// same when their names are. A spelling the table of units does not have is the same as itself.
fn same_unit(a: (&str, Option<&Unit>), b: (&str, Option<&Unit>)) -> bool {
    match (a.1, b.1) {
        (Some(x), Some(y)) => match (&x.dim, &y.dim) {
            (Dim::Count(m), Dim::Count(n)) => m == n,
            (Dim::Count(_), Dim::Number) | (Dim::Number, Dim::Count(_)) => true,
            _ => x.same(y),
        },
        (None, None) => a.0 == b.0,
        _ => false,
    }
}

/// Whether the numbers that can come are ones a range takes (an end not written takes anything).
fn numbers_fit(lo: Option<i128>, hi: Option<i128>, values: &Numbers) -> Answer<Against> {
    let outside = |v: i128| lo.is_some_and(|l| v < l) || hi.is_some_and(|h| v > h);
    match values {
        Numbers::Output(Found::Undecided(why)) => Answer::Undecided(why.clone()),
        Numbers::Output(Found::Value(o)) => {
            if lo.is_none_or(|l| o.min.is_some_and(|m| m >= l)) && hi.is_none_or(|h| o.max.is_some_and(|m| m <= h)) {
                return Answer::Holds;
            }
            // of the values outside that an input reaches, the smallest below, else the largest above
            let below = o.examples.iter().filter(|(v, _)| lo.is_some_and(|l| *v < l)).min_by_key(|(v, _)| *v);
            let above = o.examples.iter().filter(|(v, _)| outside(*v)).max_by_key(|(v, _)| *v);
            if let Some((v, inputs)) = below.or(above) {
                return Answer::Fails(Against::value(v.to_string(), inputs_text(inputs)));
            }
            let range = ends(o.min, o.max);
            Answer::Undecided(match (o.min, o.max) {
                (Some(_), Some(_)) => tr!(
                    "rulec の数えた範囲では出力が {range} で、規則のベクタには、受け取る側の範囲を外れる値になる入力がありません",
                    "rulec counts the output as {range}, and none of the rule's vectors gives it a value outside what this place takes"
                ),
                _ => tr!(
                    "rulec が出力の範囲の端を数えられません（{range}）。規則のベクタには、受け取る側の範囲を外れる値になる入力がありません",
                    "rulec does not count an end of the output's range ({range}), and none of the rule's vectors gives it a value outside what this place takes"
                ),
            })
        }
        Numbers::Passes(set) => {
            let below = lo.map_or(Ints(Vec::new()), |l| set.intersect(&Ints::between(None, Some(l.saturating_sub(1)))));
            let above = hi.map_or(Ints(Vec::new()), |h| set.intersect(&Ints::between(Some(h.saturating_add(1)), None)));
            match below.max().or(above.min()) {
                Some(v) => Answer::Fails(Against::value(v.to_string(), None)),
                None => Answer::Holds,
            }
        }
        Numbers::Declared(a, b) => {
            if let Some(v) = a.filter(|v| outside(*v)).or(b.filter(|v| outside(*v))) {
                return Answer::Fails(Against::value(v.to_string(), None));
            }
            if (lo.is_some() && a.is_none()) || (hi.is_some() && b.is_none()) {
                return Answer::Undecided(tr!("出どころの範囲に、受け取る側が書いた端がありません", "the source's range has no end where this place writes one"));
            }
            Answer::Holds
        }
    }
}

/// Whether the days that can come are ones a place takes, as a range or as a set of days.
fn days_fit(takes: &Carried, gives: &Gives, input: &dyn Fn(&Path, &str, i64) -> Option<String>) -> Answer<Against> {
    let takes_day = |d: i64| match takes {
        Carried::Date { min, max } => min.is_none_or(|m| d >= m) && max.is_none_or(|m| d <= m),
        Carried::Days(set) => set.contains(&d),
        _ => false,
    };
    let open = matches!(takes, Carried::Date { min: None, max: None });
    match gives {
        Gives::Days(Found::Undecided(why), _) => Answer::Undecided(why.clone()),
        Gives::Days(Found::Value(set), from) => match set.iter().copied().find(|d| !takes_day(*d)) {
            Some(d) => Answer::Fails(Against::value(day_text(d), from.as_ref().and_then(|(disk, date)| input(disk, date, d)))),
            None => Answer::Holds,
        },
        Gives::DayRange { min, max, declared } => {
            if open {
                return Answer::Holds;
            }
            let Carried::Date { min: lo, max: hi } = takes else {
                return Answer::Undecided(tr!("受け取る側の日は日付のファイルの日で、出どころが言うのは日の範囲だけです", "this place takes the days of a dates file, and the source says only a range of days"));
            };
            let out_lo = |d: i64| lo.is_some_and(|l| d < l) || hi.is_some_and(|h| d > h);
            if *declared && let Some(d) = min.filter(|d| out_lo(*d)).or(max.filter(|d| out_lo(*d))) {
                return Answer::Fails(Against::value(day_text(d), None));
            }
            let inside = lo.is_none_or(|l| min.is_some_and(|m| m >= l)) && hi.is_none_or(|h| max.is_some_and(|m| m <= h));
            if inside {
                return Answer::Holds;
            }
            Answer::Undecided(tr!("出どころの日の範囲が、受け取る側の範囲に収まるとは言えません", "the source's range of days cannot be said to lie in what this place takes"))
        }
        // The days a contract lists; a day it does not read as is no day this place takes.
        Gives::Wire(Some(list)) if !open => match list.iter().find(|v| !ritsu_ports::day_of(v).is_some_and(takes_day)) {
            Some(v) => Answer::Fails(Against::value(format!("\"{v}\""), None)),
            None => Answer::Holds,
        },
        // A date travels as a string, and a contract says nothing of which day (as with E122).
        _ if open => Answer::Holds,
        _ => Answer::Undecided(tr!("契約は日付を文字列として運ぶだけで、どの日が来るかを言いません", "a contract carries a date as a string, and does not say which days can come")),
    }
}

/// What the type of what can come is not, when it is not what a place takes.
fn types_differ(takes: &Carried, gives: &Gives) -> Answer<Against> {
    let (t, g) = (takes_text(takes), gives_text(gives));
    Answer::Fails(Against::none(tr!("受け取る側は {t}、出どころは {g} で、型が違います", "this place takes {t}, and the source is {g}: the types differ")))
}

/// Whether what can come from a source is what a place takes (DESIGN 7.16).
fn fits(takes: &Carried, gives: &Gives, input: &dyn Fn(&Path, &str, i64) -> Option<String>) -> Answer<Against> {
    if let Gives::Opt(g, how) = gives {
        return match takes {
            Carried::Opt(t) => fits(t, g, input),
            _ => Answer::Fails(Against { value: Some("none".into()), at: None, why: Some(how.clone()) }),
        };
    }
    let t = takes.present().0;
    match (t, gives) {
        (Carried::Other(a), Gives::Other(b)) => {
            Answer::Undecided(tr!("{a} と {b} は、値を比べません", "{a} and {b} are not compared value by value"))
        }
        (Carried::Bool, Gives::Bool) | (Carried::Str, Gives::Str | Gives::Wire(_) | Gives::Enum { .. }) => Answer::Holds,
        (Carried::Enum { values, .. }, Gives::Wire(Some(list))) => match list.iter().find(|v| !values.contains(v)) {
            Some(v) => Answer::Fails(Against::value(format!("\"{v}\""), None)),
            None => Answer::Holds,
        },
        (Carried::Enum { .. }, Gives::Wire(None)) => {
            Answer::Fails(Against::none(tr!("契約はどの文字列でも通しますが、受け取る側は列挙の値だけを受け取ります", "the contract lets any string through, and this place takes only the values of the enum")))
        }
        (Carried::Enum { values, .. }, Gives::Enum { all, reached }) => {
            if all.iter().all(|v| values.contains(v)) {
                return Answer::Holds;
            }
            match reached {
                Found::Undecided(why) => Answer::Undecided(why.clone()),
                Found::Value(vs) => match vs.iter().find(|(v, _)| !values.contains(v)) {
                    Some((v, inputs)) => Answer::Fails(Against::value(v.clone(), inputs_text(inputs))),
                    None => Answer::Holds,
                },
            }
        }
        (Carried::Date { .. } | Carried::Days(_), Gives::Days(..) | Gives::DayRange { .. } | Gives::Wire(_)) => days_fit(t, gives, input),
        (Carried::Num { written, unit, min, max }, Gives::Num { unit: from, values }) => {
            if let Some((w, u)) = from
                && !same_unit((written, unit.as_ref()), (w, u.as_ref()))
            {
                return Answer::Fails(Against::none(tr!("受け取る側の単位は {written}、出どころの単位は {w} です", "this place counts in {written}, and the source in {w}")));
            }
            numbers_fit(*min, *max, values)
        }
        // A rule's number is a whole number on the wire (§10.2), and a contract's fraction only a rate
        // can read; E122 does not compare it.
        (Carried::Num { unit: Some(u), min, max, .. }, Gives::Frac) if u.dim == Dim::Rate => {
            if min.is_none() && max.is_none() {
                Answer::Holds
            } else {
                Answer::Undecided(tr!("契約の小数と、刻みで数える率の範囲は比べません", "a contract's fraction is not compared with the range of a rate, which counts in its step"))
            }
        }
        _ => types_differ(t, gives),
    }
}

/// The floor and the ceiling of a number a document writes with a fraction or an exponent.
fn floor_ceil(s: &str) -> Option<(i128, i128)> {
    let (mantissa, exp) = s.split_once(['e', 'E']).unwrap_or((s, "0"));
    let exp: i32 = exp.parse().ok()?;
    let negative = mantissa.starts_with('-');
    let digits = mantissa.trim_start_matches(['-', '+']);
    let (whole, part) = digits.split_once('.').unwrap_or((digits, ""));
    let n: i128 = format!("{whole}{part}").parse().ok()?;
    let shift = exp - part.len() as i32;
    if shift >= 0 {
        let v = n.checked_mul(10i128.checked_pow(shift as u32)?)?;
        let v = if negative { -v } else { v };
        return Some((v, v));
    }
    let d = 10i128.checked_pow(shift.unsigned_abs())?;
    let (q, more) = (n / d, i128::from(n % d != 0));
    Some(if negative { (-q - more, -q) } else { (q, q + more) })
}

/// The integers a schema lets through (`minimum`, `maximum`, their exclusive forms, `const`, `enum`).
fn schema_ints(s: &Schema) -> Ints {
    let low = s.minimum.as_ref().and_then(|b| match &b.value {
        Number::Int(i) => Some(if b.exclusive { i + 1 } else { *i }),
        Number::Decimal(d) => floor_ceil(d).map(|(f, c)| if b.exclusive { f + 1 } else { c }),
    });
    let high = s.maximum.as_ref().and_then(|b| match &b.value {
        Number::Int(i) => Some(if b.exclusive { i - 1 } else { *i }),
        Number::Decimal(d) => floor_ceil(d).map(|(f, c)| if b.exclusive { c - 1 } else { f }),
    });
    let set = Ints::between(low, high);
    match &s.values {
        Some(vs) => set.intersect(&Ints::points(&vs.iter().filter_map(|v| v.parse().ok()).collect::<Vec<i128>>())),
        None => set,
    }
}

/// What a schema says can come.
fn schema_gives(s: &Schema) -> Gives {
    let ints = |s: &Schema| Gives::Num { unit: None, values: Numbers::Passes(schema_ints(s)) };
    match s.ty.as_deref() {
        Some("integer") => ints(s),
        Some("number") => Gives::Frac,
        Some("string") => Gives::Wire(s.values.clone()),
        Some("boolean") => Gives::Bool,
        Some(other) => Gives::Other(other.to_string()),
        None => match &s.values {
            Some(vs) if !vs.is_empty() && vs.iter().all(|v| v.parse::<i128>().is_ok()) => ints(s),
            Some(vs) => Gives::Wire(Some(vs.clone())),
            None => Gives::Other("a schema with no type".into()),
        },
    }
}

/// A rule's output of type `ty`.
fn output_gives(joined: &Joined, disk: &Path, output: &str, facts: &RuleFacts, ty: &ColumnType, under_opt: bool) -> Gives {
    match ty {
        ColumnType::Opt(t) => Gives::Opt(Box::new(output_gives(joined, disk, output, facts, t, true)), tr!("規則の出力は `none` になることがあります", "the rule's output can be `none`")),
        ColumnType::Bool => Gives::Bool,
        ColumnType::Str => Gives::Str,
        ColumnType::Date => {
            let (min, max) = joined.rulec.date_range(disk, output).unwrap_or((None, None));
            Gives::DayRange { min, max, declared: false }
        }
        ColumnType::Enum { name, .. } => {
            let all = facts.enums.iter().find(|e| &e.name == name).map(|e| e.values.iter().map(|v| v.public.clone()).collect()).unwrap_or_default();
            let unread = || tr!("rulec が出力の値を答えません", "rulec does not answer for the output's values");
            let reached = match joined.rulec.outputs_over(disk, output, &[]) {
                Ok(Found::Value(vs)) => Found::Value(
                    vs.into_iter()
                        .filter_map(|(v, inputs)| match v {
                            Value::Enum(x) => Some((x, inputs)),
                            _ => None,
                        })
                        .collect(),
                ),
                Ok(Found::Undecided(why)) => Found::Undecided(why),
                Err(_) => Found::Undecided(unread()),
            };
            Gives::Enum { all, reached }
        }
        ColumnType::Num { written, unit, min, max } => {
            // the values of an output that may be none are not counted; the range still is
            let values = if under_opt {
                Found::Value(OutputValues { min: *min, max: *max, values: None, examples: Vec::new() })
            } else {
                joined.rulec.output_values(disk, output).unwrap_or_else(|_| Found::Undecided(tr!("rulec が出力の値を答えません", "rulec does not answer for the output's values")))
            };
            Gives::Num { unit: Some((written.clone(), unit.clone())), values: Numbers::Output(values) }
        }
    }
}

/// What a workflow's or a module's place holds, as what can come from it.
fn carried_gives(c: Carried) -> Gives {
    match c {
        Carried::Bool => Gives::Bool,
        Carried::Str => Gives::Str,
        Carried::Date { min, max } => Gives::DayRange { min, max, declared: true },
        Carried::Days(set) => Gives::Days(Found::Value(set), None),
        Carried::Enum { values, .. } => {
            let reached = values.iter().map(|v| (v.clone(), Vec::new())).collect();
            Gives::Enum { all: values, reached: Found::Value(reached) }
        }
        Carried::Num { written, unit, min, max } => Gives::Num { unit: Some((written, unit)), values: Numbers::Declared(min, max) },
        Carried::Opt(t) => Gives::Opt(Box::new(carried_gives(*t)), tr!("出どころの値は無いことがあります", "the source may have no value")),
        Carried::Other(w) => Gives::Other(w),
    }
}

/// A `.proto`'s field.
fn proto_gives(ps: &Protos, file: &str, message: &str, field: &str) -> Asked {
    let Some(f) = ps.files.get(file) else {
        return Asked::Missing(tr!("{file} を .proto として読めません", "{file} does not read as a .proto"));
    };
    let Some(m) = f.messages.iter().find(|m| m.name == message) else {
        return Asked::Missing(tr!("{file} に message `{message}` がありません", "{file} has no message `{message}`"));
    };
    let Some(fd) = m.fields.iter().find(|x| x.name == field) else {
        return Asked::Missing(tr!("message `{message}` にフィールド `{field}` がありません", "the message `{message}` has no field `{field}`"));
    };
    let optional = fd.label == Label::Optional || fd.oneof.is_some();
    let gives = match (&fd.label, &fd.ty) {
        (Label::Repeated, _) => Gives::Other("a repeated field".into()),
        (_, Type::Map(..)) => Gives::Other("a map".into()),
        (_, Type::Scalar(s)) => match s.as_str() {
            "bool" => Gives::Bool,
            "string" => Gives::Wire(validate::field_strs(&fd.rules, optional)),
            "double" | "float" => Gives::Frac,
            "bytes" => Gives::Other("bytes".into()),
            kind => match (validate::field_ints(kind, &fd.rules, optional), validate::int_bounds(kind)) {
                // what the rules let through, up to what the kind can hold
                (Some(set), Some((lo, hi))) => Gives::Num { unit: None, values: Numbers::Passes(set.intersect(&Ints::between(Some(lo), Some(hi)))) },
                _ => Gives::Other(kind.to_string()),
            },
        },
        (_, Type::Named(n)) => match ps.resolve(file, &f.full(message), n) {
            Resolved::Found(s) if s.is_enum => {
                let e = ps.files.get(&s.file).and_then(|x| x.enums.iter().find(|e| e.name == s.name));
                Gives::Wire(Some(e.map(|e| validate::enum_names(e, &fd.rules.enum_values)).unwrap_or_default()))
            }
            Resolved::Found(s) => Gives::Other(s.full),
            Resolved::Known(k) => Gives::Other(k),
            Resolved::Unknown | Resolved::Missing => Gives::Other(n.clone()),
        },
    };
    Asked::Gives(if optional && !fd.rules.required {
        Gives::Opt(Box::new(gives), tr!("契約は、このフィールドを設定しないことを許します", "the contract lets this field be left unset"))
    } else {
        gives
    })
}

/// An element of an OpenAPI document or a JSON Schema, through its `$ref`s.
fn document_gives(root: &Path, source: &Name) -> Asked {
    let file = &source.path;
    let Ok(text) = ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(root, file)) else {
        return Asked::Missing(tr!("{file} を読めません", "{file} does not read"));
    };
    let top = match ritsu_base::yaml::read_file_text(file, &text) {
        Ok(top) => top,
        Err(e) => {
            let (line, col) = (e.line, e.col);
            return Asked::Missing(tr!("{file} の {line}:{col} を読めません: {}", "{file} does not read at {line}:{col}: {}", e.message.ja; e.message.en));
        }
    };
    if source.tool == Tool::Openapi && document::kind_of(&top) != Some(Tool::Openapi) {
        return Asked::Missing(tr!("{file} は OpenAPI の文書ではありません", "{file} is no OpenAPI document"));
    }
    let l = Loaded::reached(root, file, top);
    let shown = source.text();
    let Some(e) = document::find(&l, source) else {
        return Asked::Missing(tr!("{shown} を文書の中に見つけられません", "the document does not hold {shown}"));
    };
    let load = |p: &str| ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(root, p)).ok();
    let Some(s) = l.root(&e.file).and_then(|top| openapi::schema_at(&e.file, top, &e.pointer, &load)) else {
        return Asked::Missing(tr!("{shown} を文書の中に見つけられません", "the document does not hold {shown}"));
    };
    let gives = schema_gives(&s);
    Asked::Gives(if s.nullable {
        Gives::Opt(Box::new(gives), tr!("契約は null を通します", "the contract lets null through"))
    } else if e.required == Some(false) {
        Gives::Opt(Box::new(gives), tr!("契約の `required` にこのプロパティが無く、省略できます", "the contract's `required` does not list this property, so it can be left out"))
    } else {
        gives
    })
}

/// What a language says when a file does not pass it.
fn unanswered(tool: Tool, shown: &str) -> Asked {
    let w = tool.word();
    Asked::Unanswered(tr!("{w} が {shown} を通さないので、値を答えられません（誤りは {w} が言います）", "{w} does not pass {shown}, so it does not answer for its values ({w} says what is wrong)"))
}

/// Ask a source what can come from it.
fn ask(project: &Project, joined: &Joined, ports: &Ports, source: &Name) -> Asked {
    let root = &project.root;
    let file = &source.path;
    let disk = ritsu_base::paths::on_disk(root, file);
    if !ritsu_base::fs::exists(&disk) || ritsu_base::fs::is_dir(&disk) {
        return Asked::Missing(tr!("ファイル {file} がありません", "there is no file {file}"));
    }
    if let Some(ext) = source.tool.extension()
        && !file.ends_with(&format!(".{ext}"))
    {
        let w = source.tool.word();
        return Asked::Missing(tr!("{w} のファイルは .{ext} です（{file}）", "a file of {w} is a .{ext} file ({file})"));
    }
    let item = |i: usize| source.items.get(i).map_or("", |(_, v)| v.as_str());
    let names = |ns: Vec<&str>| ns.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ");
    match source.tool {
        Tool::Rulec => {
            let Ok(facts) = joined.rulec.facts(&disk) else { return unanswered(source.tool, file) };
            let output = item(0);
            match facts.outputs.iter().find(|c| c.name == output) {
                Some(c) => Asked::Gives(output_gives(joined, &disk, output, &facts, &c.ty, false)),
                None => {
                    let list = names(facts.outputs.iter().map(|c| c.name.as_str()).collect());
                    Asked::Missing(tr!("{file} に出力 `{output}` がありません（出力は {list}）", "{file} has no output `{output}` (its outputs are {list})"))
                }
            }
        }
        Tool::Koyomi => {
            let Ok(facts) = joined.koyomi.facts(&disk) else { return unanswered(source.tool, file) };
            let date = item(0);
            if !facts.functions.iter().any(|f| f.name == date) {
                let list = names(facts.functions.iter().map(|f| f.name.as_str()).collect());
                return Asked::Missing(tr!("{file} に日付 `{date}` がありません（日付は {list}）", "{file} has no date `{date}` (its dates are {list})"));
            }
            let days = joined.koyomi.values(&disk, date).unwrap_or_else(|_| Found::Undecided(tr!("koyomi が日付の日を答えません", "koyomi does not answer for the days of the date")));
            Asked::Gives(Gives::Days(days, Some((disk.clone(), date.to_string()))))
        }
        Tool::Chobo => {
            let Ok(facts) = joined.chobo.facts(&disk) else { return unanswered(source.tool, file) };
            let account = item(0);
            let Some(a) = facts.accounts.iter().find(|a| a.name == account) else {
                let list = names(facts.accounts.iter().map(|a| a.name.as_str()).collect());
                return Asked::Missing(tr!("{file} に勘定 `{account}` がありません（勘定は {list}）", "{file} has no account `{account}` (its accounts are {list})"));
            };
            let lo = a.lower.as_ref().map_or(-BOOK_LIMIT, |b| b.value);
            let hi = a.upper.as_ref().map_or(BOOK_LIMIT, |b| b.value);
            let unit = Some((a.unit.clone(), facts.unit(&a.unit).cloned()));
            Asked::Gives(Gives::Num { unit, values: Numbers::Passes(Ints::between(Some(lo), Some(hi))) })
        }
        Tool::Proto => match ritsu_proto::load_from(root, file, &[], &[]) {
            Ok((ps, _)) => proto_gives(&ps, file, item(0), item(1)),
            Err((f, e)) => {
                let why = e.message("ritsu");
                Asked::Missing(tr!("{f} を .proto として読めません: {}", "{f} does not read as a .proto: {}", why.ja; why.en))
            }
        },
        Tool::Openapi | Tool::Jsonschema => document_gives(root, source),
        Tool::Dandori | Tool::Kaname => {
            let Some(feeds) = joined.feeds(source.tool) else { return unanswered(source.tool, file) };
            match feeds.holds(root, file, source, ports) {
                Ok(Some(c)) => Asked::Gives(carried_gives(c)),
                Ok(None) => {
                    let shown = source.text();
                    Asked::Missing(tr!("{file} に {shown} がありません", "{file} does not hold {shown}"))
                }
                Err(_) => unanswered(source.tool, file),
            }
        }
        // what the form takes (`fed::shapes`): a language stops any other at its own check
        _ => unanswered(source.tool, file),
    }
}

/// The place, without its file: `input food`, `record R field f`.
fn about(place: &Name) -> String {
    place.items.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(" ")
}

/// The input of a dates file at which a date comes to a day, as `received = 2026-01-20`.
fn date_input(joined: &Joined, disk: &Path, date: &str, day: i64) -> Option<String> {
    let facts = joined.koyomi.facts(disk).ok()?;
    let inputs = joined.koyomi.input_for(disk, date, day).ok()??;
    let shown: Vec<String> = inputs
        .iter()
        .map(|(n, v)| match facts.inputs.iter().find(|i| i.name == *n).map(|i| i.kind) {
            Some(ritsu_ports::DateKind::Date) => format!("{n} = {}", day_text(*v)),
            _ => format!("{n} = {v}"),
        })
        .collect();
    Some(shown.join(", "))
}

/// What would decide a border that cannot be, by who answers for the source.
fn what_decides(tool: Tool) -> Option<Text> {
    match tool {
        Tool::Rulec => Some(tr!(
            "規則が出力の端を数えられるように書くか（`min(…, 上限)` など）、受け取る側の範囲を広げれば、決められます。",
            "Write the rule so that rulec counts the output's ends (`min(…, <the limit>)`, say), or widen what this place takes, and it can be decided."
        )),
        Tool::Openapi | Tool::Jsonschema | Tool::Proto => Some(tr!(
            "契約がとりうる値を並べれば（JSON Schema の `enum`、Protovalidate の `in`）、決められます。",
            "A contract that lists its values (`enum` in a schema, `in` in Protovalidate) can be decided."
        )),
        Tool::Dandori | Tool::Kaname => Some(tr!("出どころの行に範囲を書けば、決められます。", "Write the range on the source's line, and it can be decided.")),
        _ => None,
    }
}

/// X17 over each place of the project that says where its value comes from. A file its language
/// does not pass is that language's to say, and its places are not asked of.
pub fn check(project: &Project, joined: &Joined, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    let ports = joined.ports();
    let mut out = Vec::new();
    for f in &project.files {
        let Some(feeds) = joined.feeds(f.tool) else { continue };
        let Ok(places) = feeds.feeds(&project.root, &f.rel, &ports) else { continue };
        if places.is_empty() {
            continue;
        }
        let src = ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(&project.root, &f.rel)).unwrap_or_default();
        for fed in places {
            out.extend(one(project, joined, &ports, f, &src, &fed, lang, borders));
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn one(project: &Project, joined: &Joined, ports: &Ports, f: &ritsu_project::File, src: &str, fed: &Fed, lang: Lang, borders: &mut Borders) -> Option<Finding> {
    let what = about(&fed.place);
    let at = Side::at(f.tool, &f.rel, fed.line);
    let written = &fed.written;
    let takes = takes_text(&fed.takes);
    let base = |code: &'static str, text: Text| -> Diag { Diag::at(code, &f.shown, fed.line, 1, text).source(src).rel(&f.rel) };
    let record = |to: Option<Side>, came: Came, code: Option<&str>, gives: Json, example: Json, why: Json, source: Json| {
        let mut b = Border::new(Check::FedBy, what.clone(), at.clone(), to, came, code);
        b.detail = vec![("source".into(), source), ("takes".into(), Json::str(&takes)), ("gives".into(), gives), ("example".into(), example), ("why".into(), why)];
        b
    };
    let missing = |why: Text, to: Option<Side>, source: Json, borders: &mut Borders| {
        borders.add(record(to, Came::Failed, Some("E208"), Json::Null, Json::Null, Json::str(why.get(lang)), source));
        let d = base("E208", tr!("`{what}` の出どころ {written} がありません", "The source {written} of `{what}` is not there")).note(why).note(tr!(
            "パスは、書いたファイルのディレクトリからです。名前を直すか、出どころを足してください。",
            "A path goes from the directory of the file it is written in. Put the name right, or add what it names."
        ));
        Some(Finding::of(&d, Some(f.rel.clone()), lang))
    };
    let source = match &fed.source {
        Ok(n) => n,
        Err(why) => return missing(why.clone(), None, Json::str(written), borders),
    };
    let shown = source.text();
    let to = Side::new(source.tool, Some(source.path.clone()), None);
    let gives = match ask(project, joined, ports, source) {
        Asked::Gives(g) => g,
        Asked::Missing(why) => return missing(why, Some(to), Json::str(&shown), borders),
        Asked::Unanswered(why) => {
            borders.add(record(Some(to), Came::Undecided, Some("W207"), Json::Null, Json::Null, Json::str(why.get(lang)), Json::str(&shown)));
            let d = base("W207", tr!("`{what}` が出どころ {written} の値を受け取るかを決められません", "Whether `{what}` takes what can come from {written} cannot be decided")).note(why);
            return Some(Finding::of(&d, Some(f.rel.clone()), lang));
        }
    };
    let given = gives_text(&gives);
    let input = |disk: &Path, date: &str, day: i64| date_input(joined, disk, date, day);
    let answer = fits(&fed.takes, &gives, &input);
    let code = match &answer {
        Answer::Holds => None,
        Answer::Fails(_) => Some("E207"),
        Answer::Undecided(_) => Some("W207"),
    };
    let came = Came::of(&answer);
    let (example, why) = match &answer {
        Answer::Holds => (Json::Null, Json::Null),
        Answer::Fails(a) => (
            Json::obj([("value", Json::opt_str(a.value.clone())), ("at", Json::opt_str(a.at.clone()))]),
            a.why.as_ref().map_or(Json::Null, |w| Json::str(w.get(lang))),
        ),
        Answer::Undecided(w) => (Json::Null, Json::str(w.get(lang))),
    };
    borders.add(record(Some(to), came, code, Json::str(&given), example, why, Json::str(&shown)));
    match answer {
        Answer::Holds => None,
        Answer::Fails(a) => {
            let head = match &a.value {
                Some(v) => tr!("`{what}` は {takes} を受け取りますが、出どころ {written} は {v} になることがあります", "`{what}` takes {takes}, and its source {written} can be {v}"),
                None => tr!("`{what}` は {takes} を受け取りますが、出どころ {written} は {given} です", "`{what}` takes {takes}, and its source {written} is {given}"),
            };
            let mut d = base("E207", head);
            if let Some(w) = &a.why {
                d = d.note(w.clone());
            }
            if let (Some(v), Some(i)) = (&a.value, &a.at) {
                d = d.note(tr!("{written} は、{i} のとき {v} になります。", "{written} comes to {v} at {i}."));
            }
            if a.value.is_some() && a.why.is_none() {
                match source.tool {
                    Tool::Chobo => d = d.note(tr!(
                        "帳簿が受け付ける値です。`at least`・`at most` を書いていない端は、chobo の限度（2⁶³ − 1）まで受け付けます。",
                        "The book takes it: an end with no `at least` or `at most` goes as far as chobo's limit (2⁶³ − 1)."
                    )),
                    Tool::Proto | Tool::Openapi | Tool::Jsonschema => d = d.note(tr!("契約の検証は、この値を通します。", "The contract's validation lets it through.")),
                    _ => {}
                }
            }
            d = d.note(tr!(
                "受け取る側を広げるか、出どころを狭めてください。どちらにするかは人が決めることです。",
                "Widen what this place takes, or narrow what the source gives. Which of the two is a person's decision."
            ));
            Some(Finding::of(&d, Some(f.rel.clone()), lang))
        }
        Answer::Undecided(why) => {
            let mut d = base("W207", tr!("`{what}` が出どころ {written} の値を受け取るかを決められません", "Whether `{what}` takes what can come from {written} cannot be decided")).note(why);
            if let Some(n) = what_decides(source.tool) {
                d = d.note(n);
            }
            Some(Finding::of(&d, Some(f.rel.clone()), lang))
        }
    }
}
