//! The deadlines lowered to `terms.cal` (DESIGN 5.10, 6.4): from the day a stage starts, when it is
//! due and when its reminder goes, counted in business days of the calendar, and, where a late
//! stage goes up (`when late escalate`), the deadline it gets then, counted again from the time it
//! goes up (`due_escalated`; `due_escalated_twice` for `escalate, then escalate`), with the
//! reminder of the ones it goes up to, counted from that time as the stage's reminder is from its
//! start (`remind_escalated`, `remind_escalated_twice`). A stage that writes a deadline or a
//! reminder of its own has dates of its own. The claim that every stage reminds before it is due
//! is koyomi's claim `remind < due`, for each stage that writes either. A calendar other than the
//! file's first has a file of its own (`terms_<calendar>.cal`).
//!
//! Each file counts from the day of its own calendar: the time a stage starts, read in that
//! calendar's UTC offset (DESIGN 7.5); the glue gives each the day it reads. That day is checked over
//! the days of the calendar the request can be used on, within the days the calendar's data covers
//! ([`crate::usable`] sets them in `Ctx::windows`, each calendar's own days); where nothing ends
//! them, over a whole turn of the Gregorian calendar (400 years) from the day it is first used, or
//! from 2000-01-01.

use super::lines::Out;
use super::words::Names;
use super::{Ctx, LFile, Part, what};
use crate::ast::{At, ClaimBody, Deadline, Deadlines, Late, Route, UseKind};
use ritsu_base::text::Text;
use ritsu_ports::LoweredTool;

/// A deadline or a reminder of a stage: its business days, the calendar, the time, and its line;
/// for one after the stage goes up, the date it is counted from (the deadline before it), and
/// whether that deadline is at the end of its day (the stage goes up at the next day's 00:00, and
/// the count starts on that day).
#[derive(Clone)]
struct Count {
    days: u32,
    calendar: Option<String>,
    at: Option<At>,
    line: usize,
    from: Option<String>,
    next_day: bool,
}

/// How many times a late stage goes up: `when late escalate` once, `escalate, then escalate` twice.
fn escalations(d: &Deadlines) -> Option<(u32, usize)> {
    d.lines.iter().find_map(|l| match l {
        Deadline::Late { first, then, line } => {
            let up = |x: &Late| matches!(x, Late::Escalate(_));
            let n = match (up(first), then.as_ref().is_some_and(up)) {
                (false, _) => 0,
                (true, false) => 1,
                (true, true) => 2,
            };
            Some((n, *line))
        }
        _ => None,
    })
}

/// The `every stage` block and the stage lines a request's deadlines are read from: its own, else
/// those of the shared route it uses.
fn blocks<'f>(cx: &'f Ctx) -> (Option<&'f Deadlines>, Vec<(String, String, &'f Deadlines)>) {
    blocks_of(cx.s)
}

/// [`blocks`], of a request's scope.
pub(crate) fn blocks_of<'f>(s: &'f crate::names::Scope) -> (Option<&'f Deadlines>, Vec<(String, String, &'f Deadlines)>) {
    let mut own = s.file.every_stage.as_ref();
    let mut stages: Vec<(String, String, &Deadlines)> = Vec::new();
    let add_lines = |f: &'f crate::ast::File, stages: &mut Vec<(String, String, &'f Deadlines)>| {
        if let Some(Route::Stages(lines)) = &f.route {
            for l in lines {
                if let Some(d) = &l.deadlines {
                    stages.push((l.name.text.clone(), l.name.ascii().to_string(), d));
                }
            }
        }
    };
    add_lines(s.file, &mut stages);
    if let Some(Route::Shared { alias, .. }) = &s.file.route
        && let Some(shared) = s.ringi(&alias.text, UseKind::Route)
    {
        if own.is_none() {
            own = shared.every_stage.as_ref();
        }
        add_lines(shared, &mut stages);
    }
    (own, stages)
}

fn due(d: &Deadlines) -> Option<Count> {
    d.lines.iter().find_map(|l| match l {
        Deadline::Due { days, calendar, at, line } => Some(Count { days: *days, calendar: calendar.as_ref().map(|c| c.text.clone()), at: Some(*at), line: *line, from: None, next_day: false }),
        _ => None,
    })
}

fn remind(d: &Deadlines) -> Option<Count> {
    d.lines.iter().find_map(|l| match l {
        Deadline::Remind { days, calendar, at, line } => Some(Count { days: *days, calendar: calendar.as_ref().map(|c| c.text.clone()), at: Some(At::Time(*at)), line: *line, from: None, next_day: false }),
        _ => None,
    })
}

/// The deadlines after a stage goes up, each counted from the one before: from the time the
/// deadline passes, the same business days, at the same time.
fn escalated(names: &mut Names, base_name: &str, base: &Count, times: u32, line: usize, whose: &Text) -> Vec<(String, Count, Text)> {
    let mut out = Vec::new();
    let mut from = base_name.to_string();
    for i in 0..times {
        let n = names.take(&if i == 0 { format!("{base_name}_escalated") } else { format!("{base_name}_escalated_twice") });
        let w = if i == 0 {
            what(format!("{}が過ぎて上へ回したあとの期限", whose.ja), format!("the deadline after {} passes and the stage goes up", whose.en))
        } else {
            what(format!("{}が過ぎて二度目に上へ回したあとの期限", whose.ja), format!("the deadline after {} passes and the stage goes up a second time", whose.en))
        };
        out.push((n.clone(), Count { from: Some(from.clone()), next_day: base.at == Some(At::EndOfDay), line, ..base.clone() }, w));
        from = n;
    }
    out
}

/// The reminders after a stage goes up, one for each time it does: the stage's reminder counted
/// from the time the stage goes up (the deadline before it, `chain` in order), the same business
/// days at the same time. None where the reminder does not come before the deadline (it would not
/// come before the deadline after going up either, and is not set), or is counted in another
/// calendar than the deadline (a file counts in one calendar).
fn reminders_after(names: &mut Names, base_name: &str, rem: &Count, due: &Count, chain: &[String], same_calendar: bool, whose: &Text) -> Vec<(String, Count, Text)> {
    let before = rem.days < due.days
        || (rem.days == due.days
            && match (rem.at, due.at) {
                (Some(At::Time(r)), Some(At::Time(d))) => r < d,
                (Some(At::Time(_)), Some(At::EndOfDay)) => true,
                _ => false,
            });
    if !before || !same_calendar {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (i, from) in chain.iter().take(chain.len().saturating_sub(1)).enumerate() {
        let n = names.take(&if i == 0 { format!("{base_name}_escalated") } else { format!("{base_name}_escalated_twice") });
        let w = if i == 0 {
            what(format!("{}が過ぎて上へ回したあとの催促", whose.ja), format!("the reminder after {} passes and the stage goes up", whose.en))
        } else {
            what(format!("{}が過ぎて二度目に上へ回したあとの催促", whose.ja), format!("the reminder after {} passes and the stage goes up a second time", whose.en))
        };
        out.push((n, Count { from: Some(from.clone()), next_day: due.at == Some(At::EndOfDay), ..rem.clone() }, w));
    }
    out
}

/// `terms.cal` (and a file for each other calendar), when the request writes deadlines.
pub fn terms(cx: &Ctx) -> Vec<LFile> {
    let s = cx.s;
    let (every, stages) = blocks(cx);
    // every date: its name, its count, what it is, and the stage it is of (None for every stage)
    let mut dates: Vec<(String, Count, Text)> = Vec::new();
    let mut pairs: Vec<(Option<String>, String, String)> = Vec::new();
    let mut names = Names::koyomi();
    let started = names.take("started");
    let every_remind = every.and_then(remind);
    let every_due = every.and_then(due);
    let every_up = every.and_then(escalations);
    if let Some(r) = &every_remind {
        let n = names.take("remind");
        dates.push((n, r.clone(), what("`every stage` の催促", "the reminder of every stage")));
    }
    // a reminder and a deadline counted in one calendar are dates of one file
    let key_of = |c: &Count| c.calendar.clone().filter(|n| cx.calendars.first().is_none_or(|f| f.alias != *n));
    let same = |a: &Count, b: &Count| key_of(a) == key_of(b);
    // the names of a deadline and of the deadlines after it goes up, in order
    let chain = |n: &str, up: &[(String, Count, Text)]| -> Vec<String> { std::iter::once(n.to_string()).chain(up.iter().map(|(g, ..)| g.clone())).collect() };
    // the deadline of every stage and the ones after it goes up, by the names the file gives them
    let mut every_chain: Vec<String> = Vec::new();
    if let Some(d) = &every_due {
        let n = names.take("due");
        dates.push((n.clone(), d.clone(), what("`every stage` の期限", "the deadline of every stage")));
        every_chain = vec![n.clone()];
        if let Some((times, line)) = every_up {
            let whose = Text::new("`every stage` の期限", "the deadline of every stage");
            let up = escalated(&mut names, &n, d, times, line, &whose);
            every_chain = chain(&n, &up);
            let after = every_remind.as_ref().map(|r| reminders_after(&mut names, "remind", r, d, &every_chain, same(r, d), &whose)).unwrap_or_default();
            dates.extend(up);
            dates.extend(after);
        }
    }
    if let (Some(_), Some(_)) = (&every_remind, &every_due) {
        pairs.push((None, "remind".into(), "due".into()));
    }
    for (stage, ascii, d) in &stages {
        let (r, du) = (remind(d), due(d));
        // a stage that writes only how it goes up, with the deadline of every stage
        let own_up = escalations(d);
        if du.is_none()
            && let (Some((times, line)), Some(ed)) = (own_up, &every_due)
            && own_up.map(|u| u.0) != every_up.map(|u| u.0)
        {
            let whose = Text::new(format!("ステップ `{stage}` の期限"), format!("the deadline of the stage `{stage}`"));
            let base = names.take(&format!("{ascii}_due"));
            dates.push((base.clone(), ed.clone(), what(format!("ステップ `{stage}` の期限"), format!("the deadline of the stage `{stage}`"))));
            let up = escalated(&mut names, &base, ed, times, line, &whose);
            let after = r.as_ref().or(every_remind.as_ref()).map(|rm| reminders_after(&mut names, &format!("{ascii}_remind"), rm, ed, &chain(&base, &up), same(rm, ed), &whose)).unwrap_or_default();
            dates.extend(up);
            dates.extend(after);
        }
        if r.is_none() && du.is_none() {
            continue;
        }
        let rn = match &r {
            Some(r) => {
                let n = names.take(&format!("{ascii}_remind"));
                dates.push((n.clone(), r.clone(), what(format!("ステップ `{stage}` の催促"), format!("the reminder of the stage `{stage}`"))));
                Some(n)
            }
            None => every_remind.as_ref().map(|_| "remind".to_string()),
        };
        let dn = match &du {
            Some(dd) => {
                let n = names.take(&format!("{ascii}_due"));
                dates.push((n.clone(), dd.clone(), what(format!("ステップ `{stage}` の期限"), format!("the deadline of the stage `{stage}`"))));
                if let Some((times, line)) = own_up.or(every_up) {
                    let whose = Text::new(format!("ステップ `{stage}` の期限"), format!("the deadline of the stage `{stage}`"));
                    let up = escalated(&mut names, &n, dd, times, line, &whose);
                    let after = r.as_ref().or(every_remind.as_ref()).map(|rm| reminders_after(&mut names, &format!("{ascii}_remind"), rm, dd, &chain(&n, &up), same(rm, dd), &whose)).unwrap_or_default();
                    dates.extend(up);
                    dates.extend(after);
                }
                Some(n)
            }
            None => {
                // a reminder of its own with the deadline of every stage: after it goes up as
                // every stage does, its own reminder is counted from the deadlines of every stage
                if let (Some(rm), Some(ed), false) = (&r, &every_due, own_up.is_some_and(|u| Some(u.0) != every_up.map(|e| e.0))) {
                    let whose = Text::new("`every stage` の期限", "the deadline of every stage");
                    dates.extend(reminders_after(&mut names, &format!("{ascii}_remind"), rm, ed, &every_chain, same(rm, ed), &whose));
                }
                every_due.as_ref().map(|_| "due".to_string())
            }
        };
        if let (Some(rn), Some(dn)) = (rn, dn) {
            pairs.push((Some(ascii.clone()), rn, dn));
        }
    }
    if dates.is_empty() {
        return Vec::new();
    }
    let claim = s.file.claims.iter().find(|c| matches!(c.body, ClaimBody::RemindsBeforeDue));
    // the calendars, the first one first
    let mut cals: Vec<Option<String>> = Vec::new();
    for (_, c, _) in &dates {
        let key = c.calendar.clone().filter(|n| cx.calendars.first().is_none_or(|f| f.alias != *n));
        if !cals.contains(&key) {
            cals.push(key);
        }
    }
    cals.sort_by_key(|c| c.is_some());
    let mut files = Vec::new();
    for key in cals {
        let Some(cal) = cx.calendar(key.as_deref()) else { continue };
        let mine: Vec<&(String, Count, Text)> = dates.iter().filter(|(_, c, _)| c.calendar.clone().filter(|n| cx.calendars.first().is_none_or(|f| f.alias != *n)) == key).collect();
        let none_w = Text::default();
        let mut out = Out::default();
        let suffix = key.as_ref().map(|_| format!("_{}", cal.file)).unwrap_or_default();
        out.put(format!("dates {} v1", super::words::koyomi_file(&format!("{}_terms{suffix}", cx.name()))), 0, &none_w);
        out.put(format!("description \"What ringi writes from the deadlines of {}: from the day a stage starts, when it is due and when its reminder goes\"", cx.shown()), 0, &none_w);
        out.put(format!("use calendar \"{}\"", cal.path), 0, &none_w);
        out.blank();
        out.put("inputs", 0, &none_w);
        let (lo, hi) = window(cx, &cal.alias, cal.data);
        out.put(format!("  {started} : date  range >={} <={}", crate::lex::date_text(lo), crate::lex::date_text(hi)), 0, &none_w);
        for (n, c, w) in &mine {
            out.blank();
            match &c.from {
                None => out.put(format!("date {n} = {started}"), c.line, w),
                Some(base) => {
                    out.put(format!("date {n} = {base}"), c.line, w);
                    // a deadline at the end of the day passes at the next day's 00:00: the stage
                    // goes up on the next day, and its new deadline and reminder are counted from
                    // there
                    if c.next_day {
                        out.put("  + 1 day", c.line, w);
                    }
                }
            }
            let unit = if c.days == 1 { "business day" } else { "business days" };
            out.put(format!("  + {} {unit}", c.days), c.line, w);
            match c.at {
                Some(At::EndOfDay) => out.put("  at end of day", c.line, w),
                Some(At::Time(t)) => out.put(format!("  at {}", crate::lex::time_text(t)), c.line, w),
                None => {}
            }
        }
        if let Some(cl) = claim {
            let mine_names: Vec<&str> = mine.iter().map(|(n, ..)| n.as_str()).collect();
            let ps: Vec<&(Option<String>, String, String)> = pairs.iter().filter(|(_, r, d)| mine_names.contains(&r.as_str()) && mine_names.contains(&d.as_str())).collect();
            if !ps.is_empty() {
                out.blank();
                out.put("claims", cl.line, &what(format!("主張 `{}`", cl.name.text), format!("the claim `{}`", cl.name.text)));
                for (stage, r, d) in ps {
                    let cname = match stage {
                        None => cl.name.text.clone(),
                        Some(st) => format!("{}_{st}", cl.name.text),
                    };
                    let w = what(format!("主張 `{}`", cl.name.text), format!("the claim `{}`", cl.name.text));
                    out.put(format!("  {cname} : {r} < {d}"), cl.line, &w);
                }
            }
        }
        let name = match &key {
            None => "terms.cal".to_string(),
            Some(_) => format!("terms{suffix}.cal"),
        };
        files.push(LFile { name, tool: LoweredTool::Koyomi, part: Part::Terms(key.unwrap_or_else(|| cx.calendars.first().map(|c| c.alias.clone()).unwrap_or_default())), text: out.text, lines: out.lines, renamed: names.renamed(), given: Vec::new(), reads: Vec::new() });
    }
    files
}

/// The days a stage is taken to start on: the ones [`crate::usable`] set for the calendar; else a
/// whole turn of the Gregorian calendar from 2000-01-01, within the calendar's data.
pub(crate) fn window(cx: &Ctx, alias: &str, data: Option<(i64, i64)>) -> (i64, i64) {
    let (lo, hi) = match cx.windows.iter().find(|(a, _)| a == alias) {
        Some((_, w)) => *w,
        None => {
            let (mut lo, mut hi) = (crate::usable::FIRST, crate::usable::FIRST + crate::usable::TURN - 1);
            if let Some((a, b)) = data {
                lo = lo.max(a);
                hi = hi.min(b);
            }
            (lo, hi)
        }
    };
    if hi < lo { (lo, lo) } else { (lo, hi) }
}
