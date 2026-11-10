//! What a page says, in words, beside the notation of a rule (§15.300).
//!
//! The default page keeps each cell, condition and expression as the source writes it (§1.6)
//! and adds in parentheses what it says, where the words say something the notation does not:
//! the ends of a range, `not:`, `starts_with`, `none`, a flag's `true` and `false`, the
//! values a comparison on an ordered enum takes, `always`, a comparison, and the operations of
//! an expression. The customer page writes the words alone. Every word here is built from the
//! parsed rule and the fixed words of this file, never from prose of its own, so the tests can
//! hold a page to its source: what is not a fixed word is a name or a value the source writes,
//! written as the source writes it (§15.180: a rule reads `1_000_000JPY` and `100万円`, not
//! `1,000,000JPY`). The customer page groups the digits of what it writes with [`number`].

use crate::ast::{BinOp, Cell, CmpOp, Expr, Lit, Table};
use crate::types::{Checked, Ty};

/// A number as the customer page writes it: the source's `_` as `,`, and a run of four digits
/// or more grouped by three (`1_234_567円` → `1,234,567円`, `999999999999円` →
/// `999,999,999,999円`). A date, the digits after a decimal point, the digits of a name
/// (`fy2026`, `税率2025`; a name starts with a letter or `_`) and what a string or a prefix
/// quotes (`"0120"`, 「0120」) stay as they are.
pub(crate) fn number(raw: &str) -> String {
    let cs: Vec<char> = raw.chars().collect();
    let mut out = String::new();
    let mut quoted: Option<char> = None;
    let mut i = 0;
    while i < cs.len() {
        let ch = cs[i];
        match (quoted, ch) {
            (None, '"') => quoted = Some('"'),
            (None, '「') => quoted = Some('」'),
            (Some(end), c) if c == end => quoted = None,
            _ => {}
        }
        if quoted.is_some() || !ch.is_ascii_digit() {
            out.push(ch);
            i += 1;
            continue;
        }
        let start = i;
        while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '_') {
            i += 1;
        }
        let run: String = cs[start..i].iter().collect();
        let after_point = start > 0 && cs[start - 1] == '.';
        let in_name = start > 0 && (cs[start - 1].is_alphabetic() || cs[start - 1] == '_');
        let dash_digit = |k: usize| cs.get(k) == Some(&'-') && cs.get(k + 1).is_some_and(|c| c.is_ascii_digit());
        let in_date = dash_digit(i) || (start >= 2 && cs[start - 1] == '-' && cs[start - 2].is_ascii_digit());
        if after_point || in_name || in_date {
            out.push_str(&run);
        } else if run.contains('_') {
            out.push_str(&run.replace('_', ","));
        } else if run.len() >= 4 {
            let n = run.len();
            for (k, ch) in run.chars().enumerate() {
                if k > 0 && (n - k) % 3 == 0 {
                    out.push(',');
                }
                out.push(ch);
            }
        } else {
            out.push_str(&run);
        }
    }
    out
}

/// A literal as the words write it.
fn lit(l: &Lit) -> String {
    match l {
        Lit::Num(n) => n.raw.clone(),
        Lit::Word(w) => w.clone(),
        Lit::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
        Lit::Str(s) => format!("\"{s}\""),
    }
}

fn sep() -> &'static str {
    if crate::i18n::ja() { "、" } else { ", " }
}

/// The ends of a range, the lower one first, as the statute says them: 「10万円以上 50万円以下」,
/// 「2025-04-01 から 2026-03-31 まで」, "from 10 to 50", "on or after 2026-04-01".
pub(crate) fn bounds(ops: &[(CmpOp, String)], date: bool) -> String {
    let lower = ops.iter().find(|(o, _)| matches!(o, CmpOp::Ge | CmpOp::Gt));
    let upper = ops.iter().find(|(o, _)| matches!(o, CmpOp::Le | CmpOp::Lt));
    let ja = crate::i18n::ja();
    // One end alone, or the lower end with an upper one after it (`then`).
    let one = |o: CmpOp, v: &str, then: bool| -> String {
        match (ja, date, o) {
            (true, false, CmpOp::Ge) => format!("{v}以上"),
            (true, false, CmpOp::Gt) if then => format!("{v}より大きく"),
            (true, false, CmpOp::Gt) => format!("{v}より大きい"),
            (true, false, CmpOp::Le) => format!("{v}以下"),
            (true, false, CmpOp::Lt) => format!("{v}未満"),
            (true, true, CmpOp::Ge) => format!("{v} から"),
            (true, true, CmpOp::Gt) => format!("{v} より後"),
            (true, true, CmpOp::Le) => format!("{v} まで"),
            (true, true, CmpOp::Lt) => format!("{v} より前"),
            (false, false, CmpOp::Ge) => format!("{v} or more"),
            (false, false, CmpOp::Gt) => format!("more than {v}"),
            (false, false, CmpOp::Le) => format!("{v} or less"),
            (false, false, CmpOp::Lt) => format!("less than {v}"),
            (false, true, CmpOp::Ge) => format!("on or after {v}"),
            (false, true, CmpOp::Gt) => format!("after {v}"),
            (false, true, CmpOp::Le) => format!("on or before {v}"),
            (false, true, CmpOp::Lt) => format!("before {v}"),
        }
    };
    match (lower, upper) {
        (Some((CmpOp::Ge, lv)), Some((CmpOp::Le, hv))) if !ja => format!("from {lv} to {hv}"),
        (Some((lo, lv)), Some((ho, hv))) => {
            if ja {
                format!("{} {}", one(*lo, lv, true), one(*ho, hv, false))
            } else {
                format!("{} and {}", one(*lo, lv, true), one(*ho, hv, false))
            }
        }
        (Some((o, v)), None) | (None, Some((o, v))) => one(*o, v, false),
        (None, None) => String::new(),
    }
}

/// What a cell of an input column says, where the notation does not say it plainly; `None`
/// where the cell is a value or a list of values, which read as they are, and for `-`, which
/// the page says once under the table.
pub(crate) fn cell(ty: Option<&Ty>, cell: &Cell) -> Option<String> {
    let ty = ty.map(|t| t.present());
    let date = matches!(ty, Some(Ty::Date));
    match cell {
        Cell::Lit(Lit::Word(w)) if matches!(ty, Some(Ty::Bool)) => yes_no(w),
        Cell::Not(ls) => Some(tr!("{}以外", "other than {}", ls.iter().map(lit).collect::<Vec<_>>().join(sep()))),
        Cell::Cmp(ops) => {
            let ops: Vec<(CmpOp, String)> = ops.iter().map(|(o, l)| (*o, lit(l))).collect();
            Some(bounds(&ops, date))
        }
        Cell::Prefix(ps) => {
            let ps: Vec<String> = ps.iter().map(|p| tr!("「{p}」", "\"{p}\"")).collect();
            Some(tr!("{}で始まる", "starting with {}", ps.join(if crate::i18n::ja() { "か" } else { " or " })))
        }
        Cell::Nothing => Some(tr!("値なし", "no value")),
        Cell::Ranked { values, .. } => Some(tr!("{}のどれか", "one of {}", values.join(sep()))),
        _ => None,
    }
}

fn yes_no(w: &str) -> Option<String> {
    if w == crate::kw::TRUE {
        Some(tr!("はい", "yes"))
    } else if w == crate::kw::FALSE {
        Some(tr!("いいえ", "no"))
    } else {
        None
    }
}

/// A cell as the words write it where it has no words of its own: its values.
fn cell_text(cell: &Cell) -> String {
    match cell {
        Cell::Lit(l) => lit(l),
        Cell::Set(ls) => ls.iter().map(lit).collect::<Vec<_>>().join(sep()),
        _ => String::new(),
    }
}

/// A clause's `when` as a sentence: 「政令の国税 が はい」, "by_order is yes", the parts joined
/// as all of them hold; `always`, or a condition whose every part is `-`, is 「どの場合も」, "in
/// every case". A part that is `-` says nothing and is left out.
pub(crate) fn condition(c: &Checked, t: &Table) -> Option<String> {
    let row = t.rows.first()?;
    let parts: Vec<String> = t
        .inputs
        .iter()
        .zip(&row.cells)
        .filter(|(_, x)| !matches!(x, Cell::DontCare))
        .map(|((col, _), x)| {
            let w = match x {
                Cell::Set(_) => tr!("{} のどれか", "one of {}", cell_text(x)),
                _ => cell(c.ty_of(col).as_ref(), x).unwrap_or_else(|| cell_text(x)),
            };
            tr!("{col} が {w}", "{col} is {w}")
        })
        .collect();
    if parts.is_empty() {
        return Some(tr!("どの場合も", "in every case"));
    }
    // Every part holds: 「A が はいで、B が 1万円未満」, "A is yes and B is less than 10000".
    Some(parts.join(if crate::i18n::ja() { "で、" } else { " and " }))
}

/// The rounding of a value to a grid: 「1000円単位で切り捨て」, "rounded down to 1000JPY".
pub(crate) fn rounding(mode: &str, grid: &str) -> String {
    let g = grid;
    match mode {
        m if m == crate::kw::UP => tr!("{g}単位で切り上げ", "rounded up to {g}"),
        m if m == crate::kw::DOWN => tr!("{g}単位で切り捨て", "rounded down to {g}"),
        m if m == crate::kw::HALF_UP => tr!("{g}単位で四捨五入", "rounded half up to {g}"),
        m if m == crate::kw::HALF_DOWN => tr!("{g}単位で丸め、ちょうど半分は切り捨て", "rounded half down to {g}"),
        m if m == crate::kw::HALF_EVEN => tr!("{g}単位で偶数への丸め", "rounded half to even at {g}"),
        m => format!("{m}({grid})"),
    }
}

/// An expression as the words write a part of it: the notation, with `×` and `÷`.
fn text(e: &Expr) -> String {
    match e {
        Expr::Name(n, _) => n.clone(),
        Expr::Lit(l, _) => lit(l),
        Expr::Bin(l, op, r, _) => {
            let o = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "×",
                BinOp::Div => "÷",
                BinOp::Le => "<=",
                BinOp::Lt => "<",
                BinOp::Ge => ">=",
                BinOp::Gt => ">",
                BinOp::Eq => "=",
            };
            let side = |x: &Expr| match x {
                Expr::Bin(..) => format!("({})", text(x)),
                _ => text(x),
            };
            format!("{} {o} {}", side(l), side(r))
        }
        Expr::Call(name, args, _) => format!("{name}({})", args.iter().map(text).collect::<Vec<_>>().join(", ")),
    }
}

/// What an expression does, where it calls an operation or compares: 「金額 を 1000円単位で切り捨て」,
/// 「made が 2027-03-31 以前」. `None` for arithmetic, a name or a value, which read as written.
pub(crate) fn expr(e: &Expr) -> Option<String> {
    match e {
        Expr::Call(name, args, _) => call(name, args),
        Expr::Bin(l, op, r, _) => compare(l, *op, r),
        _ => None,
    }
}

fn call(name: &str, args: &[Expr]) -> Option<String> {
    if let Some((n, d)) = crate::types::var_division(name, args) {
        let Some(Expr::Lit(Lit::Num(g), _)) = args.get(1) else { return None };
        return Some(tr!("{} を {} で割って、{}", "{} divided by {}, {}", text(n), text(d), rounding(name, &g.raw)));
    }
    match (name, args) {
        (m, [x, Expr::Lit(Lit::Num(g), _)]) if crate::num::RoundMode::parse(m).is_some() => {
            Some(tr!("{} を {}", "{} {}", text(x), rounding(m, &g.raw)))
        }
        (crate::kw::MIN, [a, b]) => Some(tr!("{} と {} の小さい方", "the smaller of {} and {}", text(a), text(b))),
        (crate::kw::MAX, [a, b]) => Some(tr!("{} と {} の大きい方", "the larger of {} and {}", text(a), text(b))),
        (crate::kw::MOD, [x, k]) => Some(tr!("{} を {} で割った余り", "the remainder of {} divided by {}", text(x), text(k))),
        (crate::kw::ISQRT, [x]) => Some(tr!("{} の平方根の整数の部分", "the whole part of the square root of {}", text(x))),
        _ => None,
    }
}

fn compare(l: &Expr, op: BinOp, r: &Expr) -> Option<String> {
    let date = [l, r].iter().any(|x| matches!(x, Expr::Lit(Lit::Date(..), _)));
    let (a, b) = (text(l), text(r));
    let ja = crate::i18n::ja();
    Some(match (ja, date, op) {
        (true, false, BinOp::Le) => format!("{a} が {b}以下"),
        (true, false, BinOp::Ge) => format!("{a} が {b}以上"),
        (true, false, BinOp::Lt) => format!("{a} が {b}未満"),
        (true, false, BinOp::Gt) => format!("{a} が {b}より大きい"),
        (true, false, BinOp::Eq) => format!("{a} が {b} と等しい"),
        (true, true, BinOp::Le) => format!("{a} が {b} 以前"),
        (true, true, BinOp::Ge) => format!("{a} が {b} 以後"),
        (true, true, BinOp::Lt) => format!("{a} が {b} より前"),
        (true, true, BinOp::Gt) => format!("{a} が {b} より後"),
        (true, true, BinOp::Eq) => format!("{a} が {b} と同じ日"),
        (false, false, BinOp::Le) => format!("{a} is at most {b}"),
        (false, false, BinOp::Ge) => format!("{a} is at least {b}"),
        (false, false, BinOp::Lt) => format!("{a} is less than {b}"),
        (false, false, BinOp::Gt) => format!("{a} is more than {b}"),
        (false, false, BinOp::Eq) => format!("{a} equals {b}"),
        (false, true, BinOp::Le) => format!("{a} is on or before {b}"),
        (false, true, BinOp::Ge) => format!("{a} is on or after {b}"),
        (false, true, BinOp::Lt) => format!("{a} is before {b}"),
        (false, true, BinOp::Gt) => format!("{a} is after {b}"),
        (false, true, BinOp::Eq) => format!("{a} is {b}"),
        (_, _, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div) => return None,
    })
}

/// The notation with its words after it, where there are words: 「always（どの場合も）」,
/// "always (in every case)". The words come out escaped for markdown by the caller.
pub(crate) fn beside(notation: &str, words: Option<String>) -> String {
    match words {
        Some(w) if !w.is_empty() && w != notation => tr!("{notation}（{w}）", "{notation} ({w})"),
        _ => notation.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{with, Lang};

    #[test]
    fn numbers_take_commas_and_dates_do_not() {
        assert_eq!(number("1_234_567円"), "1,234,567円");
        assert_eq!(number("999999999999円"), "999,999,999,999円");
        assert_eq!(number("1000万円"), "1,000万円");
        assert_eq!(number("300円"), "300円");
        assert_eq!(number("2026-04-01"), "2026-04-01");
        assert_eq!(number("0円 〜 999999999999円"), "0円 〜 999,999,999,999円");
        assert_eq!(number("0.0005%"), "0.0005%");
        assert_eq!(number("12345.6789円"), "12,345.6789円");
        assert_eq!(number("-12000円"), "-12,000円");
        assert_eq!(number("fy2026 税率2025 rate_1000"), "fy2026 税率2025 rate_1000");
        assert_eq!(number("「0120」で始まる、\"98765\" 以外、12345円"), "「0120」で始まる、\"98765\" 以外、12,345円");
    }

    fn r(o: &[(CmpOp, &str)], date: bool) -> String {
        bounds(&o.iter().map(|(a, b)| (*a, b.to_string())).collect::<Vec<_>>(), date)
    }

    #[test]
    fn the_ends_of_a_range_read_lower_first() {
        with(Lang::Ja, || {
            assert_eq!(r(&[(CmpOp::Ge, "10万円"), (CmpOp::Le, "50万円")], false), "10万円以上 50万円以下");
            assert_eq!(r(&[(CmpOp::Le, "50万円"), (CmpOp::Gt, "10万円")], false), "10万円より大きく 50万円以下");
            assert_eq!(r(&[(CmpOp::Lt, "5kg")], false), "5kg未満");
            assert_eq!(r(&[(CmpOp::Ge, "2025-04-01"), (CmpOp::Le, "2026-03-31")], true), "2025-04-01 から 2026-03-31 まで");
            assert_eq!(r(&[(CmpOp::Gt, "2026-03-31")], true), "2026-03-31 より後");
        });
        with(Lang::En, || {
            assert_eq!(r(&[(CmpOp::Ge, "10"), (CmpOp::Le, "50")], false), "from 10 to 50");
            assert_eq!(r(&[(CmpOp::Ge, "10"), (CmpOp::Lt, "50")], false), "10 or more and less than 50");
            assert_eq!(r(&[(CmpOp::Le, "2027-03-31")], true), "on or before 2027-03-31");
        });
    }

    #[test]
    fn rounding_says_half_down_too() {
        with(Lang::Ja, || {
            assert_eq!(rounding("down", "1000円"), "1000円単位で切り捨て");
            assert_eq!(rounding("half_down", "1円"), "1円単位で丸め、ちょうど半分は切り捨て");
        });
        with(Lang::En, || assert_eq!(rounding("half_down", "1JPY"), "rounded half down to 1JPY"));
    }

    #[test]
    fn beside_adds_words_only_where_they_differ() {
        with(Lang::Ja, || {
            assert_eq!(beside("always", Some("どの場合も".into())), "always（どの場合も）");
            assert_eq!(beside("一般", None), "一般");
        });
        with(Lang::En, || assert_eq!(beside("true", Some("yes".into())), "true (yes)"));
    }
}
