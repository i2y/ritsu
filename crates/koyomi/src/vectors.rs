//! `koyomi vectors` (DESIGN 6.3): the reference interpreter's result for every input of the
//! range, one JSON object a line, with the inputs just outside the range, where the generated
//! code has to refuse. The generated code is held to these lines (DESIGN 6.4).
//!
//! The rows come in the order the check walks the inputs (`check::Inputs`: the integer inputs
//! outside, the last declared turning fastest, the date inside), so the n-th line is the n-th
//! input the check computed. The rows outside the range follow.

use crate::ast::{At, Ty};
use crate::calendar::Calendar;
use crate::check::Inputs;
use crate::date::{self, Day};
use crate::interp::{self, OpFail};
use crate::resolve::Model;

/// What one input gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expect {
    /// Every date of the file in the order it declares them, each followed by its time (RFC
    /// 3339 in UTC) when it has `at`.
    Values(Vec<String>),
    /// A calendar: whether the day is a business day.
    Open(bool),
    /// The kind of error the generated code raises: `range`, `data`, `reject` or `date`.
    Error(&'static str),
}

/// One line of the vectors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Every input's value, by index (a date as its day number). For a calendar, the day.
    pub vals: Vec<i64>,
    pub expect: Expect,
}

/// The kind of error a computation that stopped gives, as the generated code names it.
pub fn kind_of(f: &OpFail) -> &'static str {
    match f {
        OpFail::Missing { .. } => "reject",
        OpFail::Outside(_) => "data",
        OpFail::OutOfRange => "date",
        OpFail::Bug(_) => "bug",
    }
}

/// The keys of a dates file's `out`, in the order the values come: each date's name, and
/// `<name>.at` after a date with `at`.
pub fn out_keys(m: &Model) -> Vec<String> {
    let timed = m.cal.as_ref().and_then(|c| c.offset).is_some();
    let mut keys = Vec::new();
    for d in &m.dates {
        keys.push(d.name.clone());
        if d.at.is_some() && timed {
            keys.push(format!("{}.at", d.name));
        }
    }
    keys
}

/// What the reference interpreter gives for one input.
pub fn expect(m: &Model, vals: &[i64], out: &mut [Day]) -> Expect {
    let mut evs = Vec::new();
    if let Err(stop) = interp::run(m, vals, out, &mut evs) {
        return Expect::Error(kind_of(&stop.fail));
    }
    let offset = m.cal.as_ref().and_then(|c| c.offset);
    let mut values = Vec::with_capacity(m.dates.len() * 2);
    for (k, d) in m.dates.iter().enumerate() {
        values.push(out[k].to_string());
        if let (Some((at, _)), Some(off)) = (d.at, offset) {
            match interp::time(out[k], at, off) {
                Ok(t) => values.push(t.utc),
                Err(f) => return Expect::Error(kind_of(&f)),
            }
        }
    }
    Expect::Values(values)
}

/// The inputs just outside the range, each refused by the generated code's guard: the day
/// before the date's range and the day after it (the integers at their least), then for each
/// integer input one below its range and one above it (the date at its first day). An input
/// no date takes has no guard in the generated code, so it gets no row.
pub fn outside(m: &Model) -> Vec<Vec<i64>> {
    let mut rows = Vec::new();
    if m.dates.is_empty() {
        return rows;
    }
    let base: Vec<i64> = m.inputs.iter().map(|i| i.lo).collect();
    let taken = |k: usize| m.dates.iter().any(|d| d.params.contains(&k));
    for (k, i) in m.inputs.iter().enumerate() {
        if !taken(k) {
            continue;
        }
        for v in [i.lo - 1, i.hi + 1] {
            if i.ty == Ty::Date && (v < date::MIN.0 as i64 || v > date::MAX.0 as i64) {
                continue;
            }
            let mut r = base.clone();
            r[k] = v;
            rows.push(r);
        }
    }
    // The date input first, as DESIGN 6.3 lists it.
    rows.sort_by_key(|r| r.iter().zip(&base).position(|(a, b)| a != b).map(|k| if k == m.date_input { 0 } else { k + 1 }));
    rows
}

/// Every row of a dates file: the range, then the inputs just outside it.
pub struct DatesRows<'a> {
    m: &'a Model,
    inputs: Inputs<'a>,
    outside: std::vec::IntoIter<Vec<i64>>,
    out: Vec<Day>,
}

impl<'a> DatesRows<'a> {
    pub fn new(m: &'a Model) -> DatesRows<'a> {
        DatesRows { m, inputs: Inputs::new(m), outside: outside(m).into_iter(), out: vec![Day(0); m.dates.len()] }
    }
}

impl Iterator for DatesRows<'_> {
    type Item = Row;

    fn next(&mut self) -> Option<Row> {
        if let Some(vals) = self.inputs.next_input() {
            let vals = vals.to_vec();
            let expect = expect(self.m, &vals, &mut self.out);
            return Some(Row { vals, expect });
        }
        let vals = self.outside.next()?;
        Some(Row { vals, expect: Expect::Error("range") })
    }
}

/// The days a calendar's vectors cover: its data range, or 1900–2100 when it reads no table
/// and knows every day (DESIGN 6.3).
pub fn calendar_span(c: &Calendar) -> (Day, Day, bool) {
    if c.data == (date::MIN, date::MAX) {
        (Day::from_ymd(1900, 1, 1).unwrap(), Day::from_ymd(2100, 12, 31).unwrap(), false)
    } else {
        (c.data.0, c.data.1, true)
    }
}

/// Every row of a calendar file: each day of its span, then the day before and the day after
/// the data range, which the generated `is_open` refuses (`data`).
pub fn calendar_rows(c: &Calendar) -> impl Iterator<Item = Row> + '_ {
    let (from, to, bounded) = calendar_span(c);
    let inside = (from.0..=to.0).map(move |z| Row {
        vals: vec![z as i64],
        expect: match c.is_open(Day(z)) {
            Ok(open) => Expect::Open(open),
            Err(_) => Expect::Error("data"),
        },
    });
    let mut edges = Vec::new();
    if bounded {
        for z in [c.data.0.0 as i64 - 1, c.data.1.0 as i64 + 1] {
            if z >= date::MIN.0 as i64 && z <= date::MAX.0 as i64 {
                edges.push(Row { vals: vec![z], expect: Expect::Error("data") });
            }
        }
    }
    inside.chain(edges)
}

/// A JSON string, for the names used as keys.
fn quoted(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

/// Writes the lines of a dates file's vectors.
pub struct DatesLines {
    keys_in: Vec<String>,
    tys: Vec<Ty>,
    keys_out: Vec<String>,
}

impl DatesLines {
    pub fn new(m: &Model) -> DatesLines {
        DatesLines {
            keys_in: m.inputs.iter().map(|i| quoted(&i.name)).collect(),
            tys: m.inputs.iter().map(|i| i.ty).collect(),
            keys_out: out_keys(m).iter().map(|k| quoted(k)).collect(),
        }
    }

    /// `{"in":{"受領日":"2026-04-01"},"out":{"締め日":"2026-04-20",…}}`.
    pub fn line(&self, r: &Row) -> String {
        let mut s = String::with_capacity(160);
        s.push_str("{\"in\":{");
        for (k, v) in r.vals.iter().enumerate() {
            if k > 0 {
                s.push(',');
            }
            s.push_str(&self.keys_in[k]);
            s.push(':');
            match self.tys[k] {
                Ty::Date => {
                    s.push('"');
                    s.push_str(&Day(*v as i32).to_string());
                    s.push('"');
                }
                Ty::Int => s.push_str(&v.to_string()),
            }
        }
        s.push('}');
        match &r.expect {
            Expect::Values(vs) => {
                s.push_str(",\"out\":{");
                for (k, v) in vs.iter().enumerate() {
                    if k > 0 {
                        s.push(',');
                    }
                    s.push_str(&self.keys_out[k]);
                    s.push_str(":\"");
                    s.push_str(v);
                    s.push('"');
                }
                s.push('}');
            }
            Expect::Error(e) => {
                s.push_str(",\"error\":\"");
                s.push_str(e);
                s.push('"');
            }
            Expect::Open(_) => unreachable!("a dates file's row"),
        }
        s.push('}');
        s
    }
}

/// `{"in":{"date":"2026-05-04"},"out":{"open":false}}`.
pub fn calendar_line(r: &Row) -> String {
    let day = Day(r.vals[0] as i32);
    match &r.expect {
        Expect::Open(o) => format!("{{\"in\":{{\"date\":\"{day}\"}},\"out\":{{\"open\":{o}}}}}"),
        Expect::Error(e) => format!("{{\"in\":{{\"date\":\"{day}\"}},\"error\":\"{e}\"}}"),
        Expect::Values(_) => unreachable!("a calendar's row"),
    }
}

/// The time an `at` gives, as the vectors and the generated code write it.
pub fn at_minutes(at: At) -> u32 {
    match at {
        At::Time(t) => t,
        At::EndOfDay => 1440,
    }
}
