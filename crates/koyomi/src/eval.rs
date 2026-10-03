//! `koyomi eval` (DESIGN 5): one input, every date, a step a line; or, for a calendar, one
//! day and whether it is open.

use crate::ast::Ty;
use crate::calendar::Calendar;
use crate::date::{self, Day};
use crate::diag::{day_with_weekday, render_steps, steps_json};
use ritsu_base::text::{Lang, Text};
use crate::interp;
use crate::resolve::{CK, Model};
use serde_json::{Value, json};

/// The input values from `name=value` arguments (a name or its alias).
pub fn parse_inputs(m: &Model, args: &[String]) -> Result<Vec<i64>, Text> {
    let mut vals: Vec<Option<i64>> = vec![None; m.inputs.len()];
    for a in args {
        let Some((k, v)) = a.split_once('=') else {
            return Err(tr!("`{a}` は `<名前>=<値>` の形ではありません", "`{a}` is not `<name>=<value>`"));
        };
        let Some(ix) = m.inputs.iter().position(|i| i.name == k || i.alias == k) else {
            let names: Vec<String> = m.inputs.iter().map(|i| format!("{}({})", i.name, i.alias)).collect();
            let names = names.join(", ");
            return Err(tr!("入力 `{k}` はありません（あるのは {names}）", "there is no input `{k}` (there are {names})"));
        };
        let i = &m.inputs[ix];
        let x = match i.ty {
            Ty::Date => date::parse(v).map(|d| d.0 as i64),
            Ty::Int => v.parse::<i64>().ok(),
        };
        let Some(x) = x else {
            let n = &i.name;
            return Err(match i.ty {
                Ty::Date => tr!("{n}の値 `{v}` は日付（`2026-04-01` の形）ではありません", "the value `{v}` of {n} is not a date (`2026-04-01`)"),
                Ty::Int => tr!("{n}の値 `{v}` は整数ではありません", "the value `{v}` of {n} is not an integer"),
            });
        };
        if vals[ix].is_some() {
            let n = &i.name;
            return Err(tr!("{n}が二度書かれています", "{n} is given twice"));
        }
        vals[ix] = Some(x);
    }
    let missing: Vec<String> = m.inputs.iter().zip(&vals).filter(|(_, v)| v.is_none()).map(|(i, _)| format!("{}=…", i.name)).collect();
    if !missing.is_empty() {
        let list = missing.join(" ");
        return Err(tr!("入力が足りません: {list}", "inputs are missing: {list}"));
    }
    Ok(vals.into_iter().map(|v| v.unwrap()).collect())
}

/// What `eval` prints for a dates file, and whether the computation went through.
pub fn eval_dates(m: &Model, vals: &[i64], lang: Lang, as_json: bool) -> (String, bool) {
    // The range is the guard at the entrance of the generated code; eval keeps it.
    for (k, i) in m.inputs.iter().enumerate() {
        if vals[k] < i.lo || vals[k] > i.hi {
            let (v, lo, hi) = (i.show(vals[k]), i.show(i.lo), i.show(i.hi));
            let msg = tr!("{} {v} は範囲 {lo}〜{hi} の外です", "{} {v} is outside its range {lo}..{hi}", i.name; i.name);
            if as_json {
                return (json!({"error": "range", "message": msg.get(lang)}).to_string() + "\n", false);
            }
            return (format!("{}\n", msg.get(lang)), false);
        }
    }
    let all: Vec<usize> = (0..m.dates.len()).collect();
    let t = interp::trace(m, vals, &all, true);
    let mut claims: Vec<(String, Option<bool>, Text)> = Vec::new();
    if t.stop.is_none() {
        let dates: Vec<Day> = t.dates.iter().map(|d| d.unwrap_or(Day(0))).collect();
        for c in &m.claims {
            let (holds, why) = match &c.kind {
                CK::Monotonic(k) => {
                    if vals[m.date_input] <= m.date_in().lo {
                        (None, tr!("範囲の最初の日なので、比べる前の日がありません", "the first day of the range has no day before it to compare with"))
                    } else {
                        let mut before = vals.to_vec();
                        before[m.date_input] -= 1;
                        let b = interp::trace(m, &before, &[*k], false);
                        match (b.dates[*k], dates.get(*k)) {
                            (Some(p), Some(q)) => {
                                let d0 = Day(before[m.date_input] as i32);
                                let n = &m.dates[*k].name;
                                (Some(*q >= p), tr!("前の日 {d0} なら{n}は {p}", "for the day before, {d0}, {n} is {p}"))
                            }
                            _ => (None, tr!("前の日の計算が止まります", "the day before does not compute")),
                        }
                    }
                }
                kind => match interp::claim(m, kind, vals, &dates) {
                    Ok(v) => {
                        let why = match (kind, v.sides) {
                            (CK::Compare(a, cmp, b), Some((l, r))) => crate::check::compare_text(m, a, *cmp, b, l, r, vals),
                            _ => Text::default(),
                        };
                        (Some(v.holds), why)
                    }
                    Err(f) => (None, interp::fail_text(&f)),
                },
            };
            claims.push((c.name.clone(), holds, why));
        }
    }
    let ok = t.stop.is_none();
    if as_json {
        let dates: serde_json::Map<String, Value> = m
            .dates
            .iter()
            .enumerate()
            .map(|(k, d)| (d.name.clone(), t.dates[k].map(|x| json!(x.to_string())).unwrap_or(Value::Null)))
            .collect();
        let times: serde_json::Map<String, Value> = m
            .dates
            .iter()
            .enumerate()
            .filter_map(|(k, d)| t.times[k].as_ref().map(|x| (d.name.clone(), json!({"utc": x.utc, "local": x.local}))))
            .collect();
        let v = json!({
            "inputs": Value::Object(m.inputs.iter().enumerate().map(|(k, i)| (i.name.clone(), match i.ty {
                Ty::Date => json!(i.show(vals[k])),
                Ty::Int => json!(vals[k]),
            })).collect()),
            "dates": dates,
            "times": times,
            "steps": crate::diag::value(&steps_json(&t.steps, lang)),
            "claims": claims.iter().map(|(n, h, w)| json!({"name": n, "holds": h, "note": w.get(lang)})).collect::<Vec<_>>(),
            "error": t.stop.as_ref().map(|s| interp::fail_text(&s.fail).get(lang).to_string()),
        });
        return (format!("{v}\n"), ok);
    }
    let mut o = String::new();
    for l in render_steps(&t.steps, lang, true) {
        o.push_str(&l);
        o.push('\n');
    }
    if !claims.is_empty() {
        o.push('\n');
        for (n, h, w) in &claims {
            let line = match (h, lang) {
                (Some(true), Lang::Ja) => format!("条件「{n}」は成り立つ"),
                (Some(false), Lang::Ja) => format!("条件「{n}」は成り立たない"),
                (None, Lang::Ja) => format!("条件「{n}」はこの入力だけでは決まらない"),
                (Some(true), Lang::En) => format!("claim {n}: holds"),
                (Some(false), Lang::En) => format!("claim {n}: fails"),
                (None, Lang::En) => format!("claim {n}: not decided by this input alone"),
            };
            if w.is_empty() {
                o.push_str(&format!("{line}\n"));
            } else if lang == Lang::Ja {
                o.push_str(&format!("{line}（{}）\n", w.ja));
            } else {
                o.push_str(&format!("{line} ({})\n", w.en));
            }
        }
    }
    (o, ok)
}

/// What `eval` prints for a calendar and one day.
pub fn eval_calendar(cal: &Calendar, day: Day, lang: Lang, as_json: bool) -> (String, bool) {
    let d = day_with_weekday(day, lang);
    match cal.is_open(day) {
        Err(_) => {
            let (lo, hi) = cal.data;
            let msg = tr!(
                "{d} は、カレンダーが知っている {lo}〜{hi} の外なので、営業日かは分かりません",
                "{d} is outside {lo}..{hi}, the days the calendar knows, so whether it is open is not known"
            );
            if as_json {
                return (json!({"date": day.to_string(), "error": "data", "message": msg.get(lang)}).to_string() + "\n", false);
            }
            (format!("{}\n", msg.get(lang)), false)
        }
        Ok(open) => {
            let reasons: Vec<Text> = cal.reasons(day).iter().map(|r| r.text()).collect();
            let opened = cal.opened_by(day).map(|s| s.name.clone().unwrap_or_else(|| s.text()));
            if as_json {
                return (
                    json!({
                        "date": day.to_string(),
                        "open": open,
                        "reasons": reasons.iter().map(|r| r.get(lang).to_string()).collect::<Vec<_>>(),
                        "opened_by": opened,
                    })
                    .to_string()
                        + "\n",
                    true,
                );
            }
            let t = match (open, &opened) {
                (true, Some(n)) => tr!("{d}  営業日（`open` の {n}）", "{d}  open ({n}, under `open`)"),
                (true, None) => tr!("{d}  営業日", "{d}  open"),
                (false, _) => {
                    let r = Text::join(&reasons, "、", ", ");
                    tr!("{d}  休み: {}", "{d}  closed: {}", r.ja; r.en)
                }
            };
            (format!("{}\n", t.get(lang)), true)
        }
    }
}
