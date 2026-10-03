//! Rust (DESIGN 6.2): `rust/<alias>.rs`, a module with a `Date { year, month, day }`
//! (`FromStr`, `Display`, `Copy`, `Ord`) and functions that return `Result<_, KoyomiError>`.
//! It compiles warning-free on the 2021 and the 2024 editions, so it does not depend on the
//! edition of the crate it is put in.
//!
//! The runner (`<alias>_runner.rs`) reads the module with `#[path]`. The standard library has
//! no JSON, so the runner reads the one object `koyomi vectors` writes by itself: names come
//! without escapes, values are a date in quotes or a number.

use ritsu_emit::header::Comment;
use super::{Call, H, Msg, Num, Piece, Unit, at_doc, date_doc, day_text, is_open_doc, message};
use crate::ast::{Kind, Ty};
use crate::naming::Target;

const T: Target = Target::Rust;

fn lit(s: &str) -> String {
    ritsu_emit::lit::json(s)
}

/// A message as a `format!`.
fn msg(u: &Unit, m: Msg) -> String {
    let mut f = String::new();
    let mut args = Vec::new();
    for p in message(u.lang, m) {
        match p {
            Piece::Lit(t) => {
                let q = lit(t);
                f.push_str(&q[1..q.len() - 1].replace('{', "{{").replace('}', "}}"));
            }
            Piece::Var(v) => {
                f.push_str("{}");
                args.push(v);
            }
        }
    }
    if args.is_empty() {
        format!("\"{f}\".to_string()")
    } else {
        format!("format!(\"{f}\", {})", args.join(", "))
    }
}

fn num(u: &Unit, n: &Num) -> String {
    match n {
        Num::Lit(v) => v.to_string(),
        Num::Input(1, k) => u.inputs[*k].alias.clone(),
        Num::Input(-1, k) => format!("-{}", u.inputs[*k].alias),
        Num::Input(f, k) => format!("{f} * {}", u.inputs[*k].alias),
    }
}

fn call(u: &Unit, c: &Call) -> String {
    match c {
        Call::AddDays(n) => format!("_add_days(day, {})?", num(u, n)),
        Call::AddBusiness(n, fwd) => format!("_add_business(day, {}, {fwd})?", num(u, n)),
        Call::AddMonths(k, m) => format!("_add_months(day, {}, {})?", num(u, k), lit(m)),
        Call::DayOfMonth(n, k, m) => format!("_day_of_month(day, {}, {}, {})?", num(u, n), num(u, k), lit(m)),
        Call::StartOfMonth(k) => format!("_start_of_month(day, {})?", num(u, k)),
        Call::EndOfMonth(k) => format!("_end_of_month(day, {})?", num(u, k)),
        Call::CloseDay(n, m) => format!("_close_day(day, {}, {})?", num(u, n), lit(m)),
        Call::CloseEndOfMonth => "_close_end_of_month(day)".into(),
        Call::Roll(c) => format!("_roll(day, {})?", lit(c)),
        Call::IfClosed(inner) => call(u, inner),
    }
}

fn doc(lines: &[String], indent: &str) -> String {
    lines.iter().map(|l| format!("{indent}/// {l}\n")).collect()
}

fn calendar(u: &Unit) -> String {
    let Some(c) = &u.cal else { return String::new() };
    let mut o = format!("// {}\n", u.t(tr!("カレンダー「{}」", "The calendar {}", c.name)));
    o.push_str(&format!("const _DATA_FROM: i64 = {}; // {}\n", c.data.0, day_text(c.data.0)));
    o.push_str(&format!("const _DATA_TO: i64 = {}; // {}\n", c.data.1, day_text(c.data.1)));
    if c.weekly.iter().any(|w| *w) {
        let ws: Vec<String> = c.weekly.iter().map(|w| w.to_string()).collect();
        let names: Vec<&str> = (0..7).filter(|d| c.weekly[*d]).map(|d| crate::kw::WEEKDAYS[d]).collect();
        o.push_str(&format!("const _WEEKLY: [bool; 7] = [{}]; // {}\n", ws.join(", "), super::weekly_comment(u, &names)));
    }
    let pairs = |name: &str, v: &[(i64, i64, String)], o: &mut String| {
        if v.is_empty() {
            return;
        }
        o.push_str(&format!("const {name}: &[(i64, i64)] = &[\n"));
        for (a, b, t) in v {
            o.push_str(&format!("    ({a}, {b}), // {t}\n"));
        }
        o.push_str("];\n");
    };
    pairs("_OPENS", &c.opens.iter().map(|(a, b, t)| (*a as i64, *b as i64, format!("open {t}"))).collect::<Vec<_>>(), &mut o);
    pairs("_EVERY", &c.every.iter().map(|(a, b, t)| (*a as i64, *b as i64, format!("closed {t}"))).collect::<Vec<_>>(), &mut o);
    pairs("_DAYS", &c.days.iter().map(|(a, b, t)| (*a as i64, *b as i64, format!("closed {t}"))).collect::<Vec<_>>(), &mut o);
    if !c.holidays.is_empty() {
        o.push_str("const _HOLIDAYS: &[i64] = &[\n");
        for (z, t) in &c.holidays {
            o.push_str(&format!("    {z}, // {t}\n"));
        }
        o.push_str("];\n");
    }
    o.push('\n');
    o
}

fn helpers(u: &Unit) -> String {
    let mut o = String::new();
    o.push_str(&format!(
        "// {}\n",
        u.t(tr!(
            "日付は 1970-01-01 からの通算日で計算する。どの関数も koyomi の date.rs と calendar.rs の同じ名前の手順を写したもの",
            "Dates are computed as days since 1970-01-01. Each function follows the procedure of the same name in koyomi's date.rs and calendar.rs"
        ))
    ));
    o.push_str("const _MIN: i64 = -719162; // 0001-01-01\nconst _MAX: i64 = 2932896; // 9999-12-31\n\n");
    o.push_str(
        r#"fn _error(kind: &'static str, message: String) -> KoyomiError {
    KoyomiError { kind, message }
}

fn _days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn _civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn _is_leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

fn _month_len(y: i64, m: i64) -> i64 {
    match m {
        2 if _is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn _ymd(y: i64, m: i64, d: i64) -> String {
    format!("{y:04}-{m:02}-{d:02}")
}

fn _format_date(z: i64) -> String {
    let (y, m, d) = _civil_from_days(z);
    _ymd(y, m, d)
}

fn _date(z: i64) -> Date {
    let (y, m, d) = _civil_from_days(z);
    Date { year: y as i32, month: m as u32, day: d as u32 }
}

"#,
    );
    o.push_str(&format!(
        r#"fn _day_number(d: Date) -> Result<i64, KoyomiError> {{
    let (y, m, dd) = (d.year as i64, d.month as i64, d.day as i64);
    if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || dd < 1 || dd > _month_len(y, m) {{
        let text = _ymd(y, m, dd);
        return Err(_error("date", {not_a_date}));
    }}
    Ok(_days_from_civil(y, m, dd))
}}

fn _parse_date(text: &str) -> Result<i64, KoyomiError> {{
    let b = text.as_bytes();
    let digits = |r: std::ops::Range<usize>| -> Option<i64> {{
        b[r].iter().try_fold(0i64, |v, c| if c.is_ascii_digit() {{ Some(v * 10 + (c - b'0') as i64) }} else {{ None }})
    }};
    if b.len() == 10 && b[4] == b'-' && b[7] == b'-' {{
        if let (Some(y), Some(m), Some(d)) = (digits(0..4), digits(5..7), digits(8..10)) {{
            if y >= 1 && (1..=12).contains(&m) && d >= 1 && d <= _month_len(y, m) {{
                return Ok(_days_from_civil(y, m, d));
            }}
        }}
    }}
    Err(_error("date", {not_a_date}))
}}

"#,
        not_a_date = msg(u, Msg::NotADate)
    ));
    if u.kind == Kind::Dates && !u.dates.is_empty() {
        o.push_str(&format!(
            r#"fn _input_date(d: Date, name: &str, from: i64, to: i64) -> Result<i64, KoyomiError> {{
    let z = _day_number(d)?;
    if z < from || z > to {{
        let (value, lo, hi) = (_format_date(z), _format_date(from), _format_date(to));
        return Err(_error("range", {range}));
    }}
    Ok(z)
}}

"#,
            range = msg(u, Msg::Range)
        ));
    }
    if u.has_ints() {
        o.push_str(&format!(
            r#"fn _input_int(v: i64, name: &str, from: i64, to: i64) -> Result<(), KoyomiError> {{
    if v < from || v > to {{
        let (value, lo, hi) = (v, from, to);
        return Err(_error("range", {range}));
    }}
    Ok(())
}}

"#,
            range = msg(u, Msg::Range)
        ));
    }
    if u.has(H::ShiftMonth) {
        o.push_str(
            r#"fn _shift_month(y: i64, m: i64, k: i64) -> (i64, i64) {
    let t = y * 12 + (m - 1) + k;
    (t.div_euclid(12), t.rem_euclid(12) + 1)
}

"#,
        );
    }
    if u.has(H::Place) {
        o.push_str(&format!(
            r#"fn _place(y: i64, m: i64, d: i64, mode: &str) -> Result<i64, KoyomiError> {{
    if !(1..=9999).contains(&y) {{
        return Err(_error("date", {outside}));
    }}
    let n = _month_len(y, m);
    if d <= n {{
        return Ok(_days_from_civil(y, m, d));
    }}
    match mode {{
        "end_of_month" => Ok(_days_from_civil(y, m, n)),
        "start_of_next_month" => {{
            let (ny, nm) = _shift_month(y, m, 1);
            if !(1..=9999).contains(&ny) {{
                return Err(_error("date", {outside}));
            }}
            Ok(_days_from_civil(ny, nm, 1))
        }}
        "reject" => {{
            let day = _ymd(y, m, d);
            Err(_error("reject", {reject}))
        }}
        _ => panic!("{{}}", {bug}),
    }}
}}

"#,
            outside = msg(u, Msg::Outside),
            reject = msg(u, Msg::Reject),
            bug = msg(u, Msg::Bug)
        ));
    }
    if u.has(H::AddDays) {
        o.push_str(&format!(
            r#"fn _add_days(z: i64, n: i64) -> Result<i64, KoyomiError> {{
    let r = z + n;
    if !(_MIN..=_MAX).contains(&r) {{
        return Err(_error("date", {outside}));
    }}
    Ok(r)
}}

"#,
            outside = msg(u, Msg::Outside)
        ));
    }
    if u.has(H::AddMonths) {
        o.push_str(
            r#"fn _add_months(z: i64, k: i64, mode: &str) -> Result<i64, KoyomiError> {
    let (y, m, d) = _civil_from_days(z);
    let (y2, m2) = _shift_month(y, m, k);
    _place(y2, m2, d, mode)
}

"#,
        );
    }
    if u.has(H::DayOfMonth) {
        o.push_str(
            r#"fn _day_of_month(z: i64, n: i64, k: i64, mode: &str) -> Result<i64, KoyomiError> {
    let (y, m, _) = _civil_from_days(z);
    let (y2, m2) = _shift_month(y, m, k);
    _place(y2, m2, n, mode)
}

"#,
        );
    }
    if u.has(H::StartOfMonth) {
        o.push_str(
            r#"fn _start_of_month(z: i64, k: i64) -> Result<i64, KoyomiError> {
    let (y, m, _) = _civil_from_days(z);
    let (y2, m2) = _shift_month(y, m, k);
    _place(y2, m2, 1, "none")
}

"#,
        );
    }
    if u.has(H::EndOfMonth) {
        o.push_str(&format!(
            r#"fn _end_of_month(z: i64, k: i64) -> Result<i64, KoyomiError> {{
    let (y, m, _) = _civil_from_days(z);
    let (y2, m2) = _shift_month(y, m, k);
    if !(1..=9999).contains(&y2) {{
        return Err(_error("date", {outside}));
    }}
    Ok(_days_from_civil(y2, m2, _month_len(y2, m2)))
}}

"#,
            outside = msg(u, Msg::Outside)
        ));
    }
    if u.has(H::CloseDay) {
        o.push_str(&format!(
            r#"fn _close_day(z: i64, n: i64, mode: &str) -> Result<i64, KoyomiError> {{
    let (y, m, _) = _civil_from_days(z);
    let first = if mode == "start_of_next_month" {{ -1 }} else {{ 0 }};
    for k in first..=1 {{
        let (y2, m2) = _shift_month(y, m, k);
        if !(1..=9999).contains(&y2) {{
            if k < 0 {{
                continue;
            }}
            return Err(_error("date", {outside}));
        }}
        let c = _place(y2, m2, n, mode)?;
        if c >= z {{
            return Ok(c);
        }}
    }}
    panic!("{{}}", {bug})
}}

"#,
            outside = msg(u, Msg::Outside),
            bug = msg(u, Msg::Bug)
        ));
    }
    if u.has(H::CloseEndOfMonth) {
        o.push_str(
            r#"fn _close_end_of_month(z: i64) -> i64 {
    let (y, m, _) = _civil_from_days(z);
    _days_from_civil(y, m, _month_len(y, m))
}

"#,
        );
    }
    if u.has(H::Weekday) {
        o.push_str(
            r#"fn _weekday(z: i64) -> usize {
    (z + 3).rem_euclid(7) as usize
}

"#,
        );
    }
    if u.has(H::IsOpen) {
        let c = u.cal.as_ref().unwrap();
        let mut body = String::new();
        if !c.opens.is_empty() {
            body.push_str("    if _OPENS.iter().any(|(a, b)| *a <= z && z <= *b) {\n        return Ok(true);\n    }\n");
        }
        if c.weekly.iter().any(|w| *w) {
            body.push_str("    if _WEEKLY[_weekday(z)] {\n        return Ok(false);\n    }\n");
        }
        if !c.holidays.is_empty() {
            body.push_str("    if _HOLIDAYS.binary_search(&z).is_ok() {\n        return Ok(false);\n    }\n");
        }
        if !c.every.is_empty() {
            body.push_str("    let (_, m, d) = _civil_from_days(z);\n    let md = m * 100 + d;\n");
            body.push_str("    if _EVERY.iter().any(|(a, b)| if a <= b { *a <= md && md <= *b } else { md >= *a || md <= *b }) {\n        return Ok(false);\n    }\n");
        }
        if !c.days.is_empty() {
            body.push_str("    if _DAYS.iter().any(|(a, b)| *a <= z && z <= *b) {\n        return Ok(false);\n    }\n");
        }
        o.push_str(&format!(
            r#"fn _is_open(z: i64) -> Result<bool, KoyomiError> {{
    if z < _DATA_FROM || z > _DATA_TO {{
        let (day, lo, hi) = (_format_date(z), _format_date(_DATA_FROM), _format_date(_DATA_TO));
        return Err(_error("data", {data}));
    }}
{body}    Ok(true)
}}

"#,
            data = msg(u, Msg::Data)
        ));
    }
    if u.has(H::Seek) {
        o.push_str(
            r#"fn _seek(z: i64, forward: bool) -> Result<i64, KoyomiError> {
    let mut x = z;
    while !_is_open(x)? {
        x = _add_days(x, if forward { 1 } else { -1 })?;
    }
    Ok(x)
}

"#,
        );
    }
    if u.has(H::Roll) {
        o.push_str(
            r#"fn _same_month(a: i64, b: i64) -> bool {
    let (ya, ma, _) = _civil_from_days(a);
    let (yb, mb, _) = _civil_from_days(b);
    ya == yb && ma == mb
}

fn _roll(z: i64, convention: &str) -> Result<i64, KoyomiError> {
    match convention {
        "following" => _seek(z, true),
        "preceding" => _seek(z, false),
        "modified_following" => {
            let f = _seek(z, true)?;
            if _same_month(f, z) { Ok(f) } else { _seek(z, false) }
        }
        _ => {
            let p = _seek(z, false)?;
            if _same_month(p, z) { Ok(p) } else { _seek(z, true) }
        }
    }
}

"#,
        );
    }
    if u.has(H::AddBusiness) {
        o.push_str(
            r#"fn _add_business(z: i64, n: i64, forward: bool) -> Result<i64, KoyomiError> {
    if n == 0 {
        return _seek(z, forward);
    }
    let mut x = z;
    let mut k = 0;
    while k < n {
        x = _add_days(x, if forward { 1 } else { -1 })?;
        if _is_open(x)? {
            k += 1;
        }
    }
    Ok(x)
}

"#,
        );
    }
    if u.has(H::At) {
        o.push_str(&format!(
            r#"fn _at(day: Date, minutes: i64, offset: i64) -> Result<String, KoyomiError> {{
    let local = _day_number(day)? * 1440 + minutes;
    let utc = local - offset;
    for v in [local, utc] {{
        if !(_MIN..=_MAX).contains(&v.div_euclid(1440)) {{
            return Err(_error("date", {outside}));
        }}
    }}
    let (ud, um) = (utc.div_euclid(1440), utc.rem_euclid(1440));
    Ok(format!("{{}}T{{:02}}:{{:02}}:00Z", _format_date(ud), um / 60, um % 60))
}}

"#,
            outside = msg(u, Msg::Outside)
        ));
    }
    o
}

fn functions(u: &Unit) -> String {
    let mut o = String::new();
    for d in &u.dates {
        let params: Vec<(String, Ty)> = d.params.iter().map(|k| (u.inputs[*k].alias.clone(), u.inputs[*k].ty)).collect();
        let fname = T.function(&d.alias);
        o.push_str(&doc(&date_doc(u, d, None), ""));
        o.push_str(&format!("{} {{\n", T.signature(&u.alias, &d.alias, &params, false)));
        let first = &u.inputs[u.date_input()];
        let mutable = if d.steps.is_empty() { "" } else { "mut " };
        o.push_str(&format!("    let {mutable}day = _input_date({}, {}, {}, {})?; // {}\n", first.alias, lit(&first.name), first.lo, first.hi, first.range_text()));
        for k in &d.params {
            let i = &u.inputs[*k];
            if i.ty == Ty::Int {
                o.push_str(&format!("    _input_int({}, {}, {}, {})?; // {}\n", i.alias, lit(&i.name), i.lo, i.hi, i.range_text()));
            }
        }
        for s in &d.steps {
            match &s.call {
                Call::IfClosed(inner) => {
                    o.push_str(&format!("    if !_is_open(day)? {{\n        day = {}; // {}\n    }}\n", call(u, inner), s.comment));
                }
                c => o.push_str(&format!("    day = {}; // {}\n", call(u, c), s.comment)),
            }
        }
        o.push_str("    Ok(_date(day))\n}\n\n");
        if let Some((minutes, _)) = &d.at {
            let at = crate::naming::at_alias(&d.alias);
            let off = u.cal.as_ref().and_then(|c| c.offset).unwrap_or(0);
            o.push_str(&doc(&at_doc(u, d, None, &fname), ""));
            o.push_str(&format!("{} {{\n", T.signature(&u.alias, &at, &params, true)));
            let args: Vec<String> = params.iter().map(|(a, _)| a.clone()).collect();
            o.push_str(&format!("    _at({fname}({})?, {minutes}, {off})\n}}\n\n", args.join(", ")));
        }
    }
    if u.cal.is_some() {
        o.push_str(&doc(&is_open_doc(u, None), ""));
        o.push_str(&format!("{} {{\n    _is_open(_day_number(day)?)\n}}\n\n", T.is_open(&u.alias)));
    }
    while o.ends_with("\n\n") {
        o.pop();
    }
    o
}

/// `rust/<alias>.rs`.
pub fn module(u: &Unit) -> String {
    let mut o = String::new();
    for h in &u.header {
        o.push_str(&Comment::Slashes.line(h));
    }
    o.push('\n');
    let what = match u.kind {
        Kind::Dates => {
            let n = super::named(&u.name, &u.alias, false);
            u.t(tr!("{}の日付を計算する。", "The dates of {}.", n.ja; n.en))
        }
        Kind::Calendar => {
            let n = super::named(&u.name, &u.alias, true);
            u.t(tr!("ある日がカレンダー{}の営業日かを返す。", "Whether a day is a business day of the calendar {}.", n.ja; n.en))
        }
    };
    o.push_str(&format!("//! {what}\n\nuse std::fmt;\nuse std::str::FromStr;\n\n"));
    o.push_str(&doc(
        &[u.t(tr!(
            "先発グレゴリオ暦の日付（0001-01-01〜9999-12-31。タイムゾーンを持たない）。",
            "A day of the proleptic Gregorian calendar, 0001-01-01..9999-12-31, with no time zone."
        ))],
        "",
    ));
    o.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub struct Date {\n    pub year: i32,\n    pub month: u32,\n    pub day: u32,\n}\n\n");
    o.push_str(&doc(
        &[u.t(tr!(
            "この生成物の関数が返すエラー。kind は range（入力が範囲の外）、data（カレンダーが知らない日）、reject（else reject の無い日）、date（0001-01-01〜9999-12-31 の外か日付でない値）。",
            "What the functions of this module return. kind is range (an input outside its range), data (a day the calendar does not know), reject (a day the month does not have under else reject) or date (outside 0001-01-01..9999-12-31, or not a date)."
        ))],
        "",
    ));
    o.push_str("#[derive(Clone, Debug, PartialEq, Eq)]\npub struct KoyomiError {\n    pub kind: &'static str,\n    pub message: String,\n}\n\n");
    o.push_str("impl fmt::Display for KoyomiError {\n    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n        f.write_str(&self.message)\n    }\n}\n\nimpl std::error::Error for KoyomiError {}\n\n");
    o.push_str(&doc(&[u.t(tr!("YYYY-MM-DD を読む。", "Reads YYYY-MM-DD."))], ""));
    o.push_str("impl FromStr for Date {\n    type Err = KoyomiError;\n\n    fn from_str(s: &str) -> Result<Date, KoyomiError> {\n        Ok(_date(_parse_date(s)?))\n    }\n}\n\n");
    o.push_str(&doc(&[u.t(tr!("YYYY-MM-DD で書く。", "Writes YYYY-MM-DD."))], ""));
    o.push_str("impl fmt::Display for Date {\n    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n        write!(f, \"{:04}-{:02}-{:02}\", self.year, self.month, self.day)\n    }\n}\n\n");
    o.push_str(&calendar(u));
    o.push_str(&helpers(u));
    o.push_str(&functions(u));
    o
}

/// `rust/<alias>_runner.rs`.
pub fn runner(u: &Unit) -> String {
    let a = &u.alias;
    let mut o = String::new();
    o.push_str(&Comment::Slashes.line(&u.header[0]));
    o.push_str(&format!(
        "// {}\n\n",
        u.t(tr!(
            "koyomi vectors の行を標準入力から読み、関数の結果を一行ずつ書く（空白で区切る。エラーなら error <種類>）",
            "Reads the lines of koyomi vectors on standard input and writes what the functions give, a line each (separated by a space; error <kind> for an error)"
        ))
    ));
    o.push_str(&format!("// The runner uses only some of what the module offers.\n#[allow(dead_code)]\n#[path = \"{a}.rs\"]\nmod {a};\n\n"));
    // A file with no date gives an empty line for every input, and reads nothing of it.
    let reads = u.kind == Kind::Calendar || !u.outputs().is_empty();
    if reads {
        o.push_str(&format!("use {a}::{{Date, KoyomiError}};\nuse ::std::io::{{BufRead, BufWriter, Write}};\n\n"));
    } else {
        o.push_str(&format!("use {a}::KoyomiError;\nuse ::std::io::{{BufRead, BufWriter, Write}};\n\n"));
    }
    if reads {
        o.push_str(
            r#"/// The value of `key` in the line's `in` object: a string's contents, or a number as written.
fn input<'a>(line: &'a str, key: &str) -> &'a str {
    let start = line.find("\"in\":{").expect("an in object") + 6;
    let obj = &line[start..start + line[start..].find('}').expect("its end")];
    let pat = format!("\"{key}\":");
    let rest = &obj[obj.find(&pat).expect("the input") + pat.len()..];
    match rest.strip_prefix('"') {
        Some(s) => &s[..s.find('"').expect("a closing quote")],
        None => &rest[..rest.find(',').unwrap_or(rest.len())],
    }
}

"#,
        );
    }
    o.push_str(if reads { "fn run(line: &str) -> Result<String, KoyomiError> {\n" } else { "fn run(_line: &str) -> Result<String, KoyomiError> {\n" });
    if u.kind == Kind::Calendar {
        o.push_str(&format!("    let day: Date = input(line, \"date\").parse()?;\n    Ok({a}::is_open(day)?.to_string())\n}}\n"));
    } else {
        for (k, inp) in u.inputs.iter().enumerate() {
            if !u.dates.iter().any(|d| d.params.contains(&k)) {
                continue;
            }
            match inp.ty {
                Ty::Date => o.push_str(&format!("    let in{k}: Date = input(line, {}).parse()?;\n", lit(&inp.name))),
                Ty::Int => o.push_str(&format!("    let in{k}: i64 = input(line, {}).parse().expect(\"an integer\");\n", lit(&inp.name))),
            }
        }
        o.push_str("    let out: Vec<String> = vec![\n");
        for (k, at) in u.outputs() {
            let d = &u.dates[k];
            let f = if at { T.function(&crate::naming::at_alias(&d.alias)) } else { T.function(&d.alias) };
            let args: Vec<String> = d.params.iter().map(|p| format!("in{p}")).collect();
            if at {
                o.push_str(&format!("        {a}::{f}({})?,\n", args.join(", ")));
            } else {
                o.push_str(&format!("        {a}::{f}({})?.to_string(),\n", args.join(", ")));
            }
        }
        o.push_str("    ];\n    Ok(out.join(\" \"))\n}\n");
    }
    o.push_str(
        r#"
fn main() {
    let stdin = ::std::io::stdin();
    let mut w = BufWriter::new(::std::io::stdout().lock());
    for line in stdin.lock().lines() {
        let line = line.expect("a line");
        if line.is_empty() {
            continue;
        }
        match run(&line) {
            Ok(s) => writeln!(w, "{s}").expect("stdout"),
            Err(e) => writeln!(w, "error {}", e.kind).expect("stdout"),
        }
    }
    w.flush().expect("stdout");
}
"#,
    );
    o
}
