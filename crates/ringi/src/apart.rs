//! A stage whose reminder is counted in another calendar than its deadline (DESIGN 5.10): `due in 3
//! business days in office` with `remind after 2 business days in plant`.
//!
//! A lowered dates file counts in one calendar, so the two are dates of two files (`terms.cal`,
//! `terms_plant.cal`), and koyomi neither compares them nor counts one from the other. ringi counts
//! them itself, on the days the calendars close, as it counts a pause:
//!
//! - the claim `every stage reminds before it is due` is checked over every day such a stage can
//!   start on ([`claim`], E403);
//! - after the stage goes up for being late, its reminder is counted in the reminder's calendar from
//!   the time it went up, as the lowered `remind_escalated` is where one calendar counts both (the
//!   machine's clocks, [`crate::engine`]);
//! - the last day such a reminder can be counted from is one of the last days the request can be
//!   used on ([`last_start`], [`crate::usable`]).
//!
//! Each calendar counts from its own day of a time, read in its UTC offset (DESIGN 7.5): a stage
//! started at one time may start on one day where the deadline's calendar is and on the day before
//! or after where the reminder's is, and the two clocks are compared as times.

use crate::ast::{At, ClaimBody, Deadline, Late, Word};
use crate::diag::Diag;
use crate::lower::Ctx;
use crate::names::Scope;
use crate::suite::Suite;
use ritsu_base::text::{Text, count};
use ritsu_ports::CalendarFacts;

/// Such a stage: its reminder, its deadline, and how many times it goes up when it is late.
#[derive(Clone, Debug)]
pub struct Apart<'f> {
    /// The stage as written; None for every stage that writes neither a reminder nor a deadline of
    /// its own.
    pub stage: Option<String>,
    /// The reminder: its business days, its calendar (None for the request's first), its time in
    /// minutes after midnight, and its line.
    pub remind: (u32, Option<&'f Word>, u32, usize),
    /// The deadline, the same way.
    pub due: (u32, Option<&'f Word>, At, usize),
    pub ups: u32,
}

/// The `n`th business day after `day` (`day` itself for none); None where the calendar's data does
/// not reach.
pub fn nth_open(c: &CalendarFacts, day: i64, n: u32) -> Option<i64> {
    if day < c.data.0 || day > c.data.1 {
        return None;
    }
    let (mut d, mut left) = (day, n);
    while left > 0 {
        d += 1;
        if d > c.data.1 {
            return None;
        }
        if !c.closed.contains(&d) {
            left -= 1;
        }
    }
    Some(d)
}

/// The `n`th business day after `day` (`day` itself for none), taking a day outside the calendar's
/// data as open: how far a count of business days reaches, where the data's end is another's to
/// say.
pub fn nth_open_loose(c: &CalendarFacts, day: i64, n: u32) -> i64 {
    let (mut d, mut left) = (day, n);
    while left > 0 {
        d += 1;
        if !c.closed.contains(&d) {
            left -= 1;
        }
    }
    d
}

fn minutes(at: At) -> i64 {
    match at {
        At::EndOfDay => 24 * 60,
        At::Time(t) => i64::from(t),
    }
}

/// The stages of the request whose reminder and deadline are counted in two calendars; `first` is
/// the alias of the request's first calendar, which a line that names none counts in.
pub fn stages<'f>(s: &'f Scope, first: Option<&str>) -> Vec<Apart<'f>> {
    let (every, own) = crate::lower::terms::blocks_of(s);
    let remind = |d: &'f crate::ast::Deadlines| d.lines.iter().find_map(|l| if let Deadline::Remind { days, calendar, at, line } = l { Some((*days, calendar.as_ref(), *at, *line)) } else { None });
    let due = |d: &'f crate::ast::Deadlines| d.lines.iter().find_map(|l| if let Deadline::Due { days, calendar, at, line } = l { Some((*days, calendar.as_ref(), *at, *line)) } else { None });
    let ups = |d: &'f crate::ast::Deadlines| {
        d.lines.iter().find_map(|l| match l {
            Deadline::Late { first, then, .. } => Some(match (matches!(first, Late::Escalate(_)), then.as_ref().is_some_and(|t| matches!(t, Late::Escalate(_)))) {
                (false, _) => 0,
                (true, false) => 1,
                (true, true) => 2,
            }),
            _ => None,
        })
    };
    let key = |c: Option<&Word>| c.map(|w| w.text.as_str()).filter(|n| Some(*n) != first).map(str::to_string);
    let mut out = Vec::new();
    let mut put = |stage: Option<String>, r: Option<(u32, Option<&'f Word>, u32, usize)>, d: Option<(u32, Option<&'f Word>, At, usize)>, ups: u32| {
        if let (Some(r), Some(d)) = (r, d)
            && key(r.1) != key(d.1)
        {
            out.push(Apart { stage, remind: r, due: d, ups });
        }
    };
    let (every_remind, every_due, every_ups) = (every.and_then(remind), every.and_then(due), every.and_then(ups));
    put(None, every_remind, every_due, every_ups.unwrap_or(0));
    for (stage, _, d) in own {
        let (r, du) = (remind(d), due(d));
        if r.is_none() && du.is_none() {
            continue;
        }
        put(Some(stage), r.or(every_remind), du.or(every_due), ups(d).or(every_ups).unwrap_or(0));
    }
    out
}

/// The stage `stage`, where its reminder and its deadline are counted in two calendars.
pub fn of<'f>(s: &'f Scope, first: Option<&str>, stage: &str) -> Option<Apart<'f>> {
    let all = stages(s, first);
    all.iter().find(|a| a.stage.as_deref() == Some(stage)).or_else(|| {
        // a stage with lines of its own that are counted in one calendar is not apart, whatever
        // every stage writes
        let own = crate::lower::terms::blocks_of(s).1.iter().any(|(n, _, d)| n == stage && d.lines.iter().any(|l| matches!(l, Deadline::Remind { .. } | Deadline::Due { .. })));
        all.iter().find(|a| a.stage.is_none()).filter(|_| !own)
    }).cloned()
}

/// Whether the calendar `alias` counts reminders and no deadline of the request (`first` is the
/// alias of the request's first calendar, which a line that names none counts in).
pub fn counts_reminders_alone(s: &Scope, first: Option<&str>, alias: &str) -> bool {
    let (every, own) = crate::lower::terms::blocks_of(s);
    let counts_in = |c: &Option<Word>| match c {
        Some(w) => w.text == alias,
        None => first == Some(alias),
    };
    let lines = || every.into_iter().chain(own.iter().map(|(_, _, d)| *d)).flat_map(|d| d.lines.iter());
    lines().any(|l| matches!(l, Deadline::Remind { calendar, .. } if counts_in(calendar))) && !lines().any(|l| matches!(l, Deadline::Due { calendar, .. } if counts_in(calendar)))
}

/// The calendar a line counts in: the one it names, else the request's first.
fn calendar<'c>(all: &'c [(String, std::rc::Rc<CalendarFacts>)], named: Option<&Word>) -> Option<&'c (String, std::rc::Rc<CalendarFacts>)> {
    match named {
        Some(n) => all.iter().find(|(a, _)| *a == n.text),
        None => all.first(),
    }
}

/// The time a clock counted to the day `day` at `at` rings, in a calendar at the UTC offset
/// `offset` (minutes east of UTC): seconds since 1970-01-01T00:00:00Z.
fn instant(day: i64, at: i64, offset: i32) -> i64 {
    day * 86_400 + at * 60 - i64::from(offset) * 60
}

/// The last day in `lo..=hi` of the deadline's calendar a stage can start on for every reminder
/// after it goes up to be counted in the reminder's calendar (`lo - 1` where there is none): the
/// stage goes up when its deadline passes, and the reminder is counted from the day that is where
/// the reminder's calendar is. `offsets` are the UTC offsets of the deadline's calendar and of the
/// reminder's.
pub fn last_start(a: &Apart, due_in: &CalendarFacts, remind_in: &CalendarFacts, offsets: (i32, i32), lo: i64, hi: i64) -> i64 {
    let counted = |start: i64| {
        let mut from = start;
        for _ in 0..a.ups {
            // a deadline the deadline's calendar cannot count is that calendar's to end the days by
            let Some(due) = nth_open(due_in, from, a.due.0) else { return true };
            from = if a.due.2 == At::EndOfDay { due + 1 } else { due };
            let passed = crate::simulate::day_at(instant(due, minutes(a.due.2), offsets.0), offsets.1);
            if nth_open(remind_in, passed, a.remind.0).is_none() {
                return false;
            }
        }
        true
    };
    let mut day = hi;
    while day >= lo && !counted(day) {
        day -= 1;
    }
    day
}

/// The claim `every stage reminds before it is due`, for the stages whose reminder and deadline are
/// counted in two calendars: over every day such a stage can start on, the reminder comes before
/// the deadline. Else E403, with the first such day.
///
/// The days are the reminder's calendar's. Where the two calendars read a time in two UTC offsets,
/// a stage started on one of its days starts, where the deadline's calendar is, on the day before or
/// after for some of its hours ([`crate::usable::spread`]); each is checked, and the clocks are
/// compared as times.
pub fn claim(cx: &Ctx, suite: &Suite, out: &mut Vec<Diag>) {
    let s = cx.s;
    let Some(cl) = s.file.claims.iter().find(|c| matches!(c.body, ClaimBody::RemindsBeforeDue)) else { return };
    let apart = stages(s, cx.calendars.first().map(|c| c.alias.as_str()));
    if apart.is_empty() {
        return;
    }
    let all = crate::engine::calendars(s, suite);
    let ours = cx.calendars.iter().find_map(|c| c.offset).unwrap_or(0);
    for a in &apart {
        let (Some((due_alias, due_in)), Some((remind_alias, remind_in))) = (calendar(&all, a.due.1), calendar(&all, a.remind.1)) else { continue };
        let data = |alias: &str| cx.calendars.iter().find(|c| c.alias == alias).and_then(|c| c.data);
        let ((lo_d, hi_d), (lo_r, hi_r)) = (crate::lower::terms::window(cx, due_alias, data(due_alias)), crate::lower::terms::window(cx, remind_alias, data(remind_alias)));
        let (o_d, o_r) = (due_in.offset.unwrap_or(ours), remind_in.offset.unwrap_or(ours));
        // the days of the deadline's calendar a time of one of the reminder's days is on
        let (k_lo, k_hi) = crate::usable::spread(o_r, o_d);
        let (lo, hi) = (lo_r.max(lo_d - k_hi), hi_r.min(hi_d - k_lo));
        // the days the reminder does not come before the deadline: how many, and the first
        let (mut fails, mut days, mut first) = (0usize, 0usize, None);
        for start in lo..=hi {
            let (mut counted, mut failed) = (false, None);
            for k in k_lo..=k_hi {
                let on = start + k;
                if on < lo_d || on > hi_d {
                    continue;
                }
                let (Some(r), Some(d)) = (nth_open(remind_in, start, a.remind.0), nth_open(due_in, on, a.due.0)) else { continue };
                counted = true;
                if failed.is_none() && instant(r, i64::from(a.remind.2), o_r) >= instant(d, minutes(a.due.2), o_d) {
                    failed = Some((start, on, r, d));
                }
            }
            if counted {
                days += 1;
            }
            if let Some(f) = failed {
                fails += 1;
                first.get_or_insert(f);
            }
        }
        let Some((start, on, r, d)) = first else { continue };
        let date = crate::lex::date_text;
        let whose = match &a.stage {
            Some(st) => tr!("ステップ `{st}` は", "The stage `{st}` is"),
            None => tr!("どのステップも", "Every stage is"),
        };
        let unit = |n: u32| if n == 1 { "business day" } else { "business days" };
        let (dd, rd) = (a.due.0, a.remind.0);
        let due_at = match a.due.2 {
            At::EndOfDay => tr!("の終わり", "at the end of the day"),
            At::Time(t) => {
                let t = crate::lex::time_text(t);
                tr!(" {t}", "at {t}")
            }
        };
        let remind_at = crate::lex::time_text(a.remind.2);
        let due_when = match a.due.2 {
            At::EndOfDay => tr!("{} の終わり", "at the end of {}", date(d); date(d)),
            At::Time(t) => {
                let t = crate::lex::time_text(t);
                tr!("{} {t}", "on {} at {t}", date(d); date(d))
            }
        };
        let (sd, rdate) = (date(start), date(r));
        let so = if fails == 1 { "is" } else { "are" };
        // a stage starts at one time, which is a day of each calendar
        let started = if o_d == o_r {
            tr!("{sd} に始まったステップは", "A stage started on {sd}")
        } else {
            let od = date(on);
            tr!(
                "カレンダー `{remind_alias}` のオフセットで読むと {sd}、`{due_alias}` のオフセットで読むと {od} の時刻に始まったステップは",
                "A stage started at a time read as {sd} in the offset of `{remind_alias}` and as {od} in that of `{due_alias}`"
            )
        };
        let whose_days = if o_d == o_r { tr!("", "") } else { tr!("カレンダー `{remind_alias}` のオフセットで読んだ日で、", ", as the offset of `{remind_alias}` reads them,") };
        let d = Diag::error("E403", s.path, cl.line, 1, tr!("期限より前に来ない催促があります", "A reminder does not come before its deadline"))
            .source(s.src)
            .note(Text {
                ja: format!("{}、期限をカレンダー `{due_alias}` の {dd} 営業日目{}に、催促をカレンダー `{remind_alias}` の {rd} 営業日目 {remind_at} に数えます。", whose.ja, due_at.ja),
                en: format!("{} due after {dd} {} in `{due_alias}` {}, and reminded after {rd} {} in `{remind_alias}` at {remind_at}.", whose.en, unit(dd), due_at.en, unit(rd)),
            })
            .note(Text {
                ja: format!("{}、期限が {}、催促が {rdate} {remind_at} になります。", started.ja, due_when.ja),
                en: format!("{} is due {}, and its reminder would go on {rdate} at {remind_at}.", started.en, due_when.en),
            })
            .note(tr!(
                "{}ステップが始まりうる {} 日のうち {} 日がそうで（{} から {} までを数えました）、その日に始まったステップには催促を置きません。",
                "{} of the {} days a stage can start on{} {so} so (counted from {} through {}), and a stage started on them is set no reminder.",
                whose_days.ja, count(days as u64), count(fails as u64), date(lo), date(hi); count(fails as u64), count(days as u64), whose_days.en, date(lo), date(hi)
            ))
            .note(tr!(
                "催促の営業日を減らすか、催促と期限を同じカレンダーで数えてください。",
                "Make the reminder come after fewer business days, or count the reminder and the deadline in one calendar."
            ));
        out.push(d);
    }
}
