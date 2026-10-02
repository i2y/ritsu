//! Saying an operation in words (DESIGN 7.1). Two forms, from one table:
//!
//! - the sentence the approver's page writes for a line of the `.cal` (stage D), and
//! - the short label a step of a computation carries in `eval` and in a diagnostic.
//!
//! In English the label is the line of the `.cal` itself, which is English already; in
//! Japanese it is the short form of the sentence (`翌月 10 日`, `月末締め`). No other file
//! words an operation.

use crate::ast::{At, Conv};
use crate::date::Missing;
use crate::i18n::Text;
use crate::resolve::{A, Model, ROp};

/// A number as the sentence says it: the value written, or the input's name.
fn n_of(m: &Model, a: &A) -> String {
    match a {
        A::Lit(n) => n.to_string(),
        A::Input(k) => m.inputs[*k].name.clone(),
    }
}

/// `+1` and the like as a count of months away, when it is written out.
fn months_away(m: &Model, sign: i64, k: &A) -> Result<i64, String> {
    match k {
        A::Lit(n) => Ok(sign * n),
        A::Input(i) => Err(if sign < 0 { format!("-{}", m.inputs[*i].name) } else { m.inputs[*i].name.clone() }),
    }
}

/// `翌月`, `2 か月後の`, `前月`, `同じ月の` …, before a day of the month.
fn month_ja(k: &Result<i64, String>) -> String {
    match k {
        Ok(0) => "同じ月の ".into(),
        Ok(1) => "翌月 ".into(),
        Ok(-1) => "前月 ".into(),
        Ok(n) if *n > 0 => format!("{n} か月後の "),
        Ok(n) => format!("{} か月前の ", -n),
        Err(name) => format!("{name} か月後の "),
    }
}

fn month_en(k: &Result<i64, String>) -> String {
    match k {
        Ok(0) => "the same month".into(),
        Ok(1) => "the next month".into(),
        Ok(-1) => "the month before".into(),
        Ok(n) if *n > 0 => format!("the month {n} months later"),
        Ok(n) => format!("the month {} months earlier", -n),
        Err(name) => format!("the month {name} months later"),
    }
}

fn ordinal(n: &str) -> String {
    match n.parse::<u32>() {
        Ok(v) => {
            let suffix = match (v % 10, v % 100) {
                (1, x) if x != 11 => "st",
                (2, x) if x != 12 => "nd",
                (3, x) if x != 13 => "rd",
                _ => "th",
            };
            format!("{v}{suffix}")
        }
        Err(_) => format!("day {n}"),
    }
}

/// The sentence of DESIGN 7.1 for an operation, for the approver's page.
pub fn sentence(m: &Model, op: &ROp) -> Text {
    match op {
        ROp::Days(s, n) => {
            let v = n_of(m, n);
            match (*s > 0, n) {
                (true, A::Lit(1)) => tr!("翌日", "the day after"),
                (false, A::Lit(1)) => tr!("前日", "the day before"),
                (true, _) => tr!("{v} 日後", "{v} days later"),
                (false, _) => tr!("{v} 日前", "{v} days earlier"),
            }
        }
        ROp::Business(fwd, n) => {
            let v = n_of(m, n);
            if *fwd { tr!("{v} 営業日後", "{v} business days later") } else { tr!("{v} 営業日前", "{v} business days earlier") }
        }
        ROp::Months { sign, n, per, missing } => {
            let v = n_of(m, n);
            let (unit_ja, unit_en) = if *per == 12 { ("年", "year") } else { ("か月", "month") };
            let base = match (*sign > 0, n) {
                (true, A::Lit(1)) => tr!("1 {unit_ja}後の同じ日", "the same day a {unit_en} later"),
                (false, A::Lit(1)) => tr!("1 {unit_ja}前の同じ日", "the same day a {unit_en} earlier"),
                (true, _) => tr!("{v} {unit_ja}後の同じ日", "the same day {v} {unit_en}s later"),
                (false, _) => tr!("{v} {unit_ja}前の同じ日", "the same day {v} {unit_en}s earlier"),
            };
            with_missing(base, *missing)
        }
        ROp::DayOfMonth { n, sign, k, missing } => {
            let v = n_of(m, n);
            let k = months_away(m, *sign, k);
            let base = tr!("{}{v} 日", "the {} of {}", month_ja(&k); ordinal(&v), month_en(&k));
            match missing {
                Some(_) => with_missing(base, *missing),
                None => base,
            }
        }
        ROp::StartOfMonth(sign, k) => {
            let k = months_away(m, *sign, k);
            tr!("{}1 日", "the first of {}", month_ja(&k); month_en(&k))
        }
        ROp::EndOfMonth(sign, k) => {
            let k = months_away(m, *sign, k);
            match &k {
                Ok(0) => tr!("月末", "the end of the month"),
                Ok(1) => tr!("翌月末", "the end of the next month"),
                Ok(-1) => tr!("前月末", "the end of the month before"),
                _ => tr!("{}月末", "the end of {}", month_ja(&k).trim_end(); month_en(&k)),
            }
        }
        ROp::CloseDay(n, missing) => {
            let v = n_of(m, n);
            let base = match n {
                A::Lit(x) if *x < 31 => {
                    let next = x + 1;
                    tr!(
                        "{v} 日締め：前の月の {next} 日からその月の {v} 日までの期間の、締め日",
                        "the closing day of its period, closing on the {} (the {} to the {})", ;
                        ordinal(&v),
                        ordinal(&next.to_string()),
                        ordinal(&v)
                    )
                }
                _ => tr!("{v} 日締め：その日を含む期間の締め日", "the closing day of its period, closing on the {}", ; ordinal(&v)),
            };
            match missing {
                Some(_) => with_missing(base, *missing),
                None => base,
            }
        }
        ROp::CloseEndOfMonth => tr!("月末締め：その日を含む月の末日", "the end of its month"),
        ROp::Roll(c) => roll_sentence(*c),
        ROp::IfClosed(inner) => {
            let s = sentence(m, inner);
            match inner.as_ref() {
                ROp::Days(sg, A::Lit(1)) if *sg > 0 => tr!(
                    "休みなら翌日（翌日が休みでも、それ以上は動かさない）",
                    "if it is closed, the day after (once, whatever that day is)"
                ),
                _ => tr!("休みなら{}（一度だけ）", "if it is closed, {} (once)", s.ja; s.en),
            }
        }
    }
}

fn with_missing(base: Text, missing: Option<Missing>) -> Text {
    match missing {
        Some(Missing::EndOfMonth) => tr!("{}。その日が無ければ、その月の末日", "{}; the end of that month when it has no such day", base.ja; base.en),
        Some(Missing::StartOfNextMonth) => {
            tr!("{}。その日が無ければ、その次の月の 1 日", "{}; the first of the following month when it has no such day", base.ja; base.en)
        }
        Some(Missing::Reject) => tr!(
            "{}（範囲のどの日でも、その日があることを確かめた）",
            "{} (it exists for every day of the range, as checked)",
            base.ja;
            base.en
        ),
        _ => base,
    }
}

fn roll_sentence(c: Conv) -> Text {
    match c {
        Conv::Following => tr!("休みなら翌営業日", "if it is closed, the next business day"),
        Conv::Preceding => tr!("休みなら前営業日", "if it is closed, the business day before"),
        Conv::ModifiedFollowing => tr!(
            "休みなら翌営業日。月が変わるなら前営業日",
            "if it is closed, the next business day, unless that is in the next month; then the business day before"
        ),
        Conv::ModifiedPreceding => tr!(
            "休みなら前営業日。月が変わるなら翌営業日",
            "if it is closed, the business day before, unless that is in the month before; then the next business day"
        ),
    }
}

/// The sentence for `at`.
pub fn at_sentence(at: At, offset: Option<&str>) -> Text {
    let off = offset.unwrap_or("");
    match at {
        At::Time(t) => {
            let hm = format!("{:02}:{:02}", t / 60, t % 60);
            tr!("時刻はその日の {hm}（{off}）", "at {hm} ({off}) on that day")
        }
        At::EndOfDay => tr!("時刻はその日の終わり（次の日の 00:00、{off}）", "at the end of that day (00:00 of the next, {off})"),
    }
}

/// The short label of a step: the `.cal` text in English, the short form in Japanese. The
/// numbers taken from integer inputs are shown with their values.
pub fn label(m: &Model, op: &ROp, text: &str, vals: &[i64]) -> Text {
    let ja = short_ja(m, op, vals);
    let mut en = text.to_string();
    let used = inputs_of(op);
    if !used.is_empty() {
        let vs: Vec<String> = used.iter().map(|k| format!("{} = {}", m.inputs[*k].name, vals[*k])).collect();
        en = format!("{en} ({})", vs.join(", "));
    }
    Text { ja, en }
}

fn inputs_of(op: &ROp) -> Vec<usize> {
    let mut out = Vec::new();
    let mut push = |a: &A| {
        if let A::Input(k) = a
            && !out.contains(k)
        {
            out.push(*k);
        }
    };
    match op {
        ROp::Days(_, n) | ROp::Business(_, n) | ROp::StartOfMonth(_, n) | ROp::EndOfMonth(_, n) | ROp::CloseDay(n, _) => push(n),
        ROp::Months { n, .. } => push(n),
        ROp::DayOfMonth { n, k, .. } => {
            push(n);
            push(k);
        }
        ROp::CloseEndOfMonth | ROp::Roll(_) => {}
        ROp::IfClosed(inner) => return inputs_of(inner),
    }
    out
}

fn value(a: &A, vals: &[i64]) -> i64 {
    match a {
        A::Lit(n) => *n,
        A::Input(k) => vals[*k],
    }
}

fn short_ja(m: &Model, op: &ROp, vals: &[i64]) -> String {
    // What the parentheses after the label hold: the values taken from integer inputs, and
    // what happens to a missing day.
    let mut more: Vec<String> = Vec::new();
    let mut named = |a: &A| {
        if let A::Input(k) = a {
            let s = format!("{} = {}", m.inputs[*k].name, vals[*k]);
            if !more.contains(&s) {
                more.push(s);
            }
        }
    };
    let else_ja = |p: &Option<Missing>| match p {
        Some(Missing::EndOfMonth) => Some("無ければ月末".to_string()),
        Some(Missing::StartOfNextMonth) => Some("無ければ次の月の 1 日".to_string()),
        Some(Missing::Reject) => Some("無ければ断る".to_string()),
        _ => None,
    };
    let (base, missing) = match op {
        ROp::Days(s, n) => {
            named(n);
            let v = value(n, vals);
            let t = match (*s > 0, v) {
                (true, 1) => "翌日".to_string(),
                (false, 1) => "前日".to_string(),
                (true, _) => format!("{v} 日後"),
                (false, _) => format!("{v} 日前"),
            };
            (t, None)
        }
        ROp::Business(fwd, n) => {
            named(n);
            let v = value(n, vals);
            (if *fwd { format!("{v} 営業日後") } else { format!("{v} 営業日前") }, None)
        }
        ROp::Months { sign, n, per, missing } => {
            named(n);
            let v = value(n, vals);
            let unit = if *per == 12 { "年" } else { "か月" };
            (if *sign > 0 { format!("{v} {unit}後の同じ日") } else { format!("{v} {unit}前の同じ日") }, else_ja(missing))
        }
        ROp::DayOfMonth { n, sign, k, missing } => {
            named(n);
            named(k);
            let nv = value(n, vals);
            let kv = sign * value(k, vals);
            (format!("{}{nv} 日", month_ja(&Ok(kv))), else_ja(missing))
        }
        ROp::StartOfMonth(sign, k) => {
            named(k);
            let kv = sign * value(k, vals);
            (format!("{}1 日", month_ja(&Ok(kv))), None)
        }
        ROp::EndOfMonth(sign, k) => {
            named(k);
            let kv = sign * value(k, vals);
            let t = match kv {
                0 => "月末".to_string(),
                1 => "翌月末".to_string(),
                -1 => "前月末".to_string(),
                _ => format!("{}月末", month_ja(&Ok(kv)).trim_end()),
            };
            (t, None)
        }
        ROp::CloseDay(n, missing) => {
            named(n);
            (format!("{} 日締め", value(n, vals)), else_ja(missing))
        }
        ROp::CloseEndOfMonth => ("月末締め".into(), None),
        ROp::Roll(c) => (
            match c {
                Conv::Following => "休みなら翌営業日",
                Conv::Preceding => "休みなら前営業日",
                Conv::ModifiedFollowing => "休みなら翌営業日（月が変わるなら前営業日）",
                Conv::ModifiedPreceding => "休みなら前営業日（月が変わるなら翌営業日）",
            }
            .to_string(),
            None,
        ),
        ROp::IfClosed(inner) => {
            let s = short_ja(m, inner, vals);
            return if s.starts_with("休みなら") {
                s
            } else if s.starts_with(|c: char| c.is_ascii_digit()) {
                format!("休みなら {s}")
            } else {
                format!("休みなら{s}")
            };
        }
    };
    more.extend(missing);
    if more.is_empty() { base } else { format!("{base}（{}）", more.join("。")) }
}
