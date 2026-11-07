//! The port of `fed by` (DESIGN 3.2, 7.16): the places a file says where their value comes from, and
//! what a place of a file holds, for a check across the languages to hold the two together. The
//! language of the place says what it takes; the source's language is asked what can come from it
//! through its own port ([`crate::Rules`], [`crate::Books`], [`crate::Dates`]), or by this port for a
//! place that declares what it holds: a workflow's output, a record's field, and a place that took
//! the value first. No generated code reads any of it.

use crate::{ColumnType, Day, DaySet, Ports, RuleFacts, Said};
use ritsu_base::naming::Name;
use ritsu_base::text::Text;
use ritsu_units::Unit;
use std::path::Path;

/// A value of an enum, by both the names it goes by.
#[derive(Clone, Debug, PartialEq)]
pub struct Spelled {
    /// As its file writes it.
    pub written: String,
    /// Its public name, which is what goes on the wire; the same as `written` where a file gives it
    /// no other.
    pub public: String,
}

impl Spelled {

    /// Whether it goes by the name.
    pub fn is(&self, name: &str) -> bool {
        self.written == name || self.public == name
    }

    /// Whether the two go by a name in common: the same value, however each file spells it for the
    /// wire. Hand-written code carries the value, and turning one spelling into the other is its part.
    pub fn same(&self, other: &Spelled) -> bool {
        self.is(&other.written) || self.is(&other.public)
    }
}

/// What a place takes, or what a place of a language holds: the type with the range written on it.
#[derive(Clone, Debug, PartialEq)]
pub enum Carried {
    Bool,
    Str,
    /// A calendar day, with its range; None at an end not written.
    Date { min: Option<Day>, max: Option<Day> },
    /// A calendar day that has to be one of these (an input with `range from koyomi`).
    Days(DaySet),
    /// An enum: its name, and the values taken.
    Enum { name: String, values: Vec<Spelled> },
    /// A number: the type as its language spells it, the unit (None where the spelling has none in
    /// the table of units), and its range as the integers on the wire; None at an end not written.
    Num { written: String, unit: Option<Unit>, min: Option<i128>, max: Option<i128> },
    /// `T?`: the value may be none.
    Opt(Box<Carried>),
    /// Anything no value of a source is compared with (a record, a list, a moment), as its language
    /// spells it.
    Other(String),
}

impl Carried {
    /// What a rule's input, output or field holds, as the rule's facts say it: an enum's values,
    /// those its range lets in. A date has its range from [`crate::Rules::date_range`]: the facts do
    /// not say it.
    pub fn of_column(facts: &RuleFacts, ty: &ColumnType) -> Carried {
        match ty {
            ColumnType::Bool => Carried::Bool,
            ColumnType::Str => Carried::Str,
            ColumnType::Date => Carried::Date { min: None, max: None },
            ColumnType::Enum { name, range } => {
                let values = facts.enums.iter().find(|e| &e.name == name).map(|e| &e.values[..]).unwrap_or(&[]);
                let taken = values.iter().filter(|v| range.as_ref().is_none_or(|r| r.contains(&v.name)));
                Carried::Enum { name: name.clone(), values: taken.map(|v| Spelled { written: v.name.clone(), public: v.public.clone() }).collect() }
            }
            ColumnType::Num { written, unit, min, max } => Carried::Num { written: written.clone(), unit: unit.clone(), min: *min, max: *max },
            ColumnType::Opt(t) => Carried::Opt(Box::new(Carried::of_column(facts, t))),
        }
    }

    /// A plain whole number with no unit, as a language that has only that spells it.
    pub fn whole(written: &str, min: Option<i128>, max: Option<i128>) -> Carried {
        Carried::Num { written: written.to_string(), unit: Some(Unit::number()), min, max }
    }

    /// The type under `T?`, with whether it may be none.
    pub fn present(&self) -> (&Carried, bool) {
        match self {
            Carried::Opt(t) => (t, true),
            t => (t, false),
        }
    }
}

/// A place that says where its value comes from: `food : number  range >=0 <=60  fed by …`.
#[derive(Clone, Debug, PartialEq)]
pub struct Fed {
    /// The line it is written on, from 1.
    pub line: usize,
    /// The place, as its language names it (`rulec "a.rule" input food`, `dandori "a.flow" record R
    /// field f`). A parameter of kaname's is shown the same way (`kaname "a.kn" function f parameter
    /// p`), though the table of names has no such kind: a parameter is no thing another file names.
    pub place: Name,
    /// What it takes.
    pub takes: Carried,
    /// What it names, from the root; why not, when the path it writes cannot be joined to the file.
    pub source: Result<Name, Text>,
    /// The naming as the line writes it.
    pub written: String,
}

/// What a file's places say of where their values come from. `root` is the root of the project and
/// `file` the file, from the root; `ports` are the languages a file reads others through, as a
/// workflow and a module read rules. The default says nothing, for a language with no `fed by`.
pub trait Feeds {
    /// Each place of the file that says it, in the order written, when the file passes what its
    /// language asks of it before it answers; else what the language says.
    fn feeds(&self, root: &Path, file: &str, ports: &Ports) -> Result<Vec<Fed>, Vec<Said>> {
        let _ = (root, file, ports);
        Ok(Vec::new())
    }

    /// What the place `thing` of the file holds: None when the file holds no such place. Only a
    /// language whose places a `fed by` can name answers: dandori's input, output and record's field,
    /// kaname's record's field, rulec's and koyomi's input, and sekisho's attribute and action's input.
    /// A place that takes a value also holds what can come from it, to the next place the code gives
    /// it to.
    fn holds(&self, root: &Path, file: &str, thing: &Name, ports: &Ports) -> Result<Option<Carried>, Vec<Said>> {
        let _ = (root, file, thing, ports);
        Ok(None)
    }
}
