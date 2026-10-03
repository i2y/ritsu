//! The month tables of the approver's page (DESIGN 7): every day of a month, Monday first,
//! with what closes it, whether a claim fails on it as an input, and whether an edge case
//! starts from it. Which months are shown is decided here too, without today's date, so a
//! page does not change with the day it is made.

use crate::calendar::{Calendar, Reason};
use crate::date::{self, Day};
use ritsu_base::text::Text;
use std::collections::BTreeMap;

/// A year and a month.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ym {
    pub y: i32,
    pub m: u32,
}

impl Ym {
    pub fn of(d: Day) -> Ym {
        let (y, m, _) = d.ymd();
        Ym { y, m }
    }

    /// `2026-05`, or None when it is not one.
    pub fn parse(s: &str) -> Option<Ym> {
        let (y, m) = s.split_once('-')?;
        if y.len() != 4 || m.len() != 2 || !y.bytes().all(|c| c.is_ascii_digit()) || !m.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let (y, m): (i32, u32) = (y.parse().ok()?, m.parse().ok()?);
        ((1..=9999).contains(&y) && (1..=12).contains(&m)).then_some(Ym { y, m })
    }

    pub fn next(self) -> Ym {
        if self.m == 12 { Ym { y: self.y + 1, m: 1 } } else { Ym { y: self.y, m: self.m + 1 } }
    }

    pub fn first(self) -> Day {
        Day::from_ymd(self.y as i64, self.m, 1).expect("a month of 0001..9999")
    }

    pub fn last(self) -> Day {
        Day::from_ymd(self.y as i64, self.m, date::month_len(self.y as i64, self.m)).expect("a month of 0001..9999")
    }

    /// The months from `self` to `to`, both included.
    pub fn through(self, to: Ym) -> Vec<Ym> {
        let mut out = Vec::new();
        let mut x = self;
        while x <= to {
            out.push(x);
            if x.y == 9999 && x.m == 12 {
                break;
            }
            x = x.next();
        }
        out
    }

    /// The count of months from `self` to `to`, both included.
    pub fn count(self, to: Ym) -> i64 {
        (to.y as i64 * 12 + to.m as i64) - (self.y as i64 * 12 + self.m as i64) + 1
    }
}

impl std::fmt::Display for Ym {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:04}-{:02}", self.y, self.m)
    }
}

/// `--months 2026-01..2027-12`.
pub fn parse_span(s: &str) -> Option<(Ym, Ym)> {
    let (a, b) = s.split_once("..")?;
    let (a, b) = (Ym::parse(a)?, Ym::parse(b)?);
    (a <= b).then_some((a, b))
}

/// A name a closed or open day carries in the table: a row of a table of holidays, or a
/// line of the `.cal`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Named {
    /// A row of a table of holidays (`憲法記念日`).
    Holiday(String),
    /// A `closed every`, `closed <days>` or `open` line of the `.cal` (`年末年始`).
    Rule(String),
}

impl Named {
    pub fn text(&self) -> &str {
        match self {
            Named::Holiday(s) | Named::Rule(s) => s,
        }
    }
}

/// One day of a month table.
#[derive(Clone, Debug)]
pub struct Cell {
    pub day: Day,
    /// Inside the days the calendar knows (always, without a calendar).
    pub known: bool,
    pub closed: bool,
    /// Why it is closed, every reason, as `eval` gives them.
    pub reasons: Vec<Text>,
    /// The names it carries: holidays, the `.cal`'s named closings, an `open` line.
    pub names: Vec<Named>,
    /// An `open` line opens it.
    pub opened: bool,
    /// The claims that fail on it as an input, each with the integer inputs it fails for.
    pub fails: Vec<Text>,
    /// Why an edge case starts from it.
    pub edges: Vec<Text>,
}

/// One month: weeks of seven days from Monday, None outside the month.
#[derive(Clone, Debug)]
pub struct Grid {
    pub ym: Ym,
    pub weeks: Vec<[Option<Cell>; 7]>,
}

impl Grid {
    pub fn cells(&self) -> impl Iterator<Item = &Cell> {
        self.weeks.iter().flat_map(|w| w.iter().flatten())
    }
}

/// What the day cells are lit with: the claims failing on each input day, and the edge cases.
#[derive(Default)]
pub struct Marks {
    pub fails: BTreeMap<Day, Vec<Text>>,
    pub edges: BTreeMap<Day, Vec<Text>>,
}

fn names_of(cal: &Calendar, d: Day) -> Vec<Named> {
    let mut out = Vec::new();
    if let Some(o) = cal.opened_by(d) {
        out.push(Named::Rule(o.name.clone().unwrap_or_else(|| o.text())));
        return out;
    }
    for r in cal.reasons(d) {
        match r {
            Reason::Weekday(_) => {}
            Reason::Holiday { name, table } => out.push(Named::Holiday(if name.is_empty() { table } else { name })),
            Reason::Every { name, text } | Reason::Day { name, text } => out.push(Named::Rule(name.unwrap_or(text))),
        }
    }
    out
}

/// The table of one month.
pub fn grid(ym: Ym, cal: Option<&Calendar>, marks: &Marks) -> Grid {
    let first = ym.first();
    let last = ym.last();
    let mut weeks: Vec<[Option<Cell>; 7]> = Vec::new();
    let mut week: [Option<Cell>; 7] = Default::default();
    let mut d = first;
    loop {
        let w = d.weekday() as usize;
        let (known, closed, reasons, names, opened) = match cal {
            None => (true, false, vec![], vec![], false),
            Some(c) => match c.is_open(d) {
                Err(_) => (false, false, vec![], vec![], false),
                Ok(open) => (true, !open, c.reasons(d).iter().map(Reason::text).collect(), names_of(c, d), c.opened_by(d).is_some()),
            },
        };
        week[w] = Some(Cell {
            day: d,
            known,
            closed,
            reasons,
            names,
            opened,
            fails: marks.fails.get(&d).cloned().unwrap_or_default(),
            edges: marks.edges.get(&d).cloned().unwrap_or_default(),
        });
        if w == 6 || d == last {
            weeks.push(std::mem::take(&mut week));
        }
        if d == last {
            break;
        }
        d = Day(d.0 + 1);
    }
    Grid { ym, weeks }
}

/// The named days of a month, the same names on days in a row together:
/// `(3, 3, 憲法記念日)`, `(29, 31, 年末年始)`. A day with two names has them joined the way the
/// language lists two things (`元日と年末年始`, `元日 and 年末年始`).
pub fn named_runs(g: &Grid, lang: ritsu_base::text::Lang) -> Vec<(u32, u32, String)> {
    let mut out: Vec<(u32, u32, String)> = Vec::new();
    for c in g.cells() {
        if c.names.is_empty() {
            continue;
        }
        let names: Vec<String> = c.names.iter().map(|n| n.text().to_string()).collect();
        let text = match lang {
            ritsu_base::text::Lang::Ja => crate::doc::ja_list(&names),
            ritsu_base::text::Lang::En => ritsu_base::text::Text::list(&names.iter().map(|x| ritsu_base::text::Text::same(x.clone())).collect::<Vec<_>>()).en,
        };
        let (_, _, dd) = c.day.ymd();
        match out.last_mut() {
            Some((_, b, t)) if *t == text && *b + 1 == dd => *b = dd,
            _ => out.push((dd, dd, text)),
        }
    }
    out
}
