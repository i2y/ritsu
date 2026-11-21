//! The clocks of a ticket (DESIGN 5.6, 5.7, 5.10, 7.5): the day an instant is, the deadline and the
//! reminder of a stage, and the business days of a pause and of the requester's turn.
//!
//! An instant is seconds since 1970-01-01T00:00:00Z. An event has one instant, and each calendar
//! its own day of it, read in the calendar's UTC offset ([`Machine::day_in`]): the request reads
//! the day of an event (the version of the organization, the days it can be used on) in the offset
//! of its first calendar that writes one, and a calendar counts its business days from its own day
//! of the instant, and rings its clocks at their times in its offset ([`Machine::clock_offset`]).
//! A stage's reminder and deadline are the dates of the lowered `terms.cal` (a file for each
//! calendar), counted by koyomi (`Dates::eval`) from the day the stage starts where the file's
//! calendar is; after the stage goes up for being late, its deadline and its reminder are the dates
//! the file counts from the deadline before it (`due_escalated`, `remind_escalated`;
//! `_escalated_twice` the second time), which is the day the clock rang; a reminder counted in
//! another calendar than its deadline is in no such date, and is counted here from the day the
//! deadline passed where the reminder's calendar is ([`crate::apart`]). A reminder is set where it
//! comes before its deadline. The time a stage's clocks count from moves later by the days the
//! business days it was paused for take in the pause's calendar, counted from the day it started
//! there ([`Machine::moved`]). The business days of a pause and of `resubmit within` are counted on
//! the days the calendar closes (`Dates::calendar`), from its own day of the event.

use super::{Machine, Refusal, Ticket};
use crate::ast::{TicketLine, Word};
use crate::lower::Part;
use ritsu_ports::{CalendarFacts, DateValue};
use std::rc::Rc;

impl Machine<'_, '_> {
    /// The day the instant `now` is in, as the request reads its days.
    pub fn day(&self, now: i64) -> i64 {
        crate::simulate::day_at(now, self.offset)
    }

    /// The instant of a time of a day as the request reads its days: `minutes` after its midnight
    /// (1440 is the end of the day, the next day's 00:00).
    pub fn instant(&self, day: i64, minutes: u32) -> i64 {
        Self::instant_in(day, minutes, self.offset)
    }

    /// The instant of a time of a day in the UTC offset `offset`.
    pub fn instant_in(day: i64, minutes: u32, offset: i32) -> i64 {
        day * 86_400 + i64::from(minutes) * 60 - i64::from(offset) * 60
    }

    /// The day the instant `now` is in where the calendar `alias` is (the request's first where it
    /// names none), read in its UTC offset (DESIGN 7.5): what the calendar counts business days from.
    pub(crate) fn day_in(&self, now: i64, alias: Option<&str>) -> i64 {
        crate::simulate::day_at(now, self.offset_in(alias))
    }

    /// The UTC offset of the calendar `alias` (the request's first where it names none): the one
    /// it writes, else the request's (DESIGN 7.5).
    pub(crate) fn offset_in(&self, alias: Option<&str>) -> i32 {
        let found = match alias {
            Some(a) => self.calendars.iter().find(|(x, _)| x == a),
            None => self.calendars.first(),
        };
        found.and_then(|(_, c)| c.offset).unwrap_or(self.offset)
    }

    /// The UTC offset a clock is read in (DESIGN 7.5): the offset of the calendar it counts in, the
    /// request's where that calendar writes none. `timer` is the clock: a stage's reminder and
    /// deadline (`remind`, `late`) by its first deadline as the lowered files hold it (the stage's own,
    /// else every stage's; a later one is counted in the same calendar), a pause's end (`pause
    /// ends`), the requester's turn (`resubmit late`). `stage` is the stage, as the request writes
    /// it; none asks of the stages the request writes no deadline of their own for.
    pub fn clock_offset(&self, timer: &str, stage: Option<&str>) -> i32 {
        match timer {
            "pause ends" => self.offset_in(self.pause_calendar()),
            "resubmit late" => self.offset_in(self.resubmit_calendar()),
            _ => {
                let ascii = stage.map(|st| self.s.stages().unwrap_or_default().iter().find(|n| n.text == st).map(|n| n.ascii().to_string()).unwrap_or_else(|| st.to_string()));
                let every = if timer == "late" { "due" } else { "remind" };
                let dates = self.term_calendars();
                let own = ascii.map(|a| format!("{a}_{every}"));
                let cal = own.and_then(|o| dates.iter().find(|(n, _)| *n == o)).or_else(|| dates.iter().find(|(n, _)| n == every)).map(|(_, c)| c.clone());
                match cal {
                    Some(c) => self.offset_in(Some(&c)),
                    None => self.offset,
                }
            }
        }
    }

    /// Each date of the lowered deadlines, by the request's name of it, with the calendar it counts
    /// in.
    fn term_calendars(&self) -> Vec<(String, String)> {
        let Some(dates) = &self.suite.dates else { return Vec::new() };
        let mut out = Vec::new();
        for tf in self.lowered.files.iter() {
            let Part::Terms(cal) = &tf.part else { continue };
            let Ok(facts) = self.fs(|| dates.facts(&self.lowered.path(tf))) else { continue };
            for f in &facts.functions {
                let want = tf.renamed.iter().find(|(g, _)| *g == f.name).map(|(_, w)| w.clone()).unwrap_or_else(|| f.name.clone());
                out.push((want, cal.clone()));
            }
        }
        out
    }

    /// The day is one the request can be used on (DESIGN 9.6): else refused, with the data that
    /// does not cover it, as the check found it.
    pub(crate) fn usable_on(&self, day: i64) -> Result<(), Refusal> {
        let Some(u) = self.usable else { return Ok(()) };
        let d = crate::lex::date_text(day);
        if let Some(f) = u.from.as_ref().filter(|f| day < f.day) {
            let (fd, by) = (crate::lex::date_text(f.day), crate::usable::said_by(&f.by));
            return Err(Refusal::new("data", tr!("この申請の種類は {fd} から使います。{d} には使えません（{}）", "the request is used from {fd}, not on {d} ({})", by.ja; by.en)));
        }
        if let Some(t) = u.through.as_ref().filter(|t| day > t.day) {
            let (td, by) = (crate::lex::date_text(t.day), crate::usable::said_by(&t.by));
            return Err(Refusal::new("data", tr!("この申請の種類は {td} まで使えます。{d} には使えません（{}）", "the request can be used through {td}, not on {d} ({})", by.ja; by.en)));
        }
        Ok(())
    }

    /// The calendar business days are counted in: the one named, else the request's first.
    fn calendar(&self, named: Option<&str>) -> Result<&(String, Rc<CalendarFacts>), Refusal> {
        let found = match named {
            Some(n) => self.calendars.iter().find(|(a, _)| a == n),
            None => self.calendars.first(),
        };
        found.ok_or_else(|| Refusal::new("data", tr!("営業日を数えるカレンダーがありません", "the request has no calendar to count business days in")))
    }

    fn past(alias: &str, c: &CalendarFacts, day: i64) -> Refusal {
        let (a, b, d) = (crate::lex::date_text(c.data.0), crate::lex::date_text(c.data.1), crate::lex::date_text(day));
        Refusal::new("data", tr!("カレンダー `{alias}` のデータは {a} から {b} までです。{d} からの営業日を数えられません", "the data of the calendar `{alias}` covers {a} to {b}: business days cannot be counted from {d}"))
    }

    /// The `n`th business day after `day` in the calendar `cal`; `day` itself for none.
    pub(crate) fn add_open(&self, cal: Option<&str>, day: i64, n: u32) -> Result<i64, Refusal> {
        let (alias, c) = self.calendar(cal)?;
        if day < c.data.0 || day > c.data.1 {
            return Err(Self::past(alias, c, day));
        }
        let (mut d, mut left) = (day, n);
        while left > 0 {
            d += 1;
            if d > c.data.1 {
                return Err(Self::past(alias, c, day));
            }
            if !c.closed.contains(&d) {
                left -= 1;
            }
        }
        Ok(d)
    }

    /// The business days of the calendar `cal` after `a`, through `b`.
    pub(crate) fn open_between(&self, cal: Option<&str>, a: i64, b: i64) -> Result<u32, Refusal> {
        let (alias, c) = self.calendar(cal)?;
        if a < c.data.0 || b > c.data.1 {
            return Err(Self::past(alias, c, a));
        }
        Ok(((a + 1)..=b).filter(|d| !c.closed.contains(d)).count() as u32)
    }

    /// The calendar a pause counts in, as the `pause` line names it (none for the request's first).
    pub(crate) fn pause_calendar(&self) -> Option<&str> {
        self.line(|l| if let TicketLine::Pause { calendar, .. } = l { Some(calendar.as_ref()) } else { None }).flatten().map(|w: &Word| w.text.as_str())
    }

    /// The calendar the requester's turn counts in, as the `resubmit` line names it.
    pub(crate) fn resubmit_calendar(&self) -> Option<&str> {
        self.line(|l| if let TicketLine::Resubmit { calendar, .. } = l { Some(calendar.as_ref()) } else { None }).flatten().map(|w: &Word| w.text.as_str())
    }

    /// The time the clocks of a stage that started at `started` and was paused for `held` business
    /// days count from: later by the days those business days take in the pause's calendar, counted
    /// from the day the stage started there (DESIGN 7.5). One time, which each calendar reads its
    /// own day of; with no pause, the time it started.
    pub(crate) fn moved(&self, started: i64, held: u32) -> Result<i64, Refusal> {
        if held == 0 {
            return Ok(started);
        }
        let cal = self.pause_calendar();
        let begun = self.day_in(started, cal);
        Ok(started + (self.add_open(cal, begun, held)? - begun) * 86_400)
    }

    /// The reminder and the deadline of the stage `stage`, as instants, when it started at the
    /// instant `started`, was paused for `held` business days and has gone up `up` times for being
    /// late: None where the stage has none (a stage with no deadline is never late, and has no
    /// reminder). Each file of the lowered deadlines counts from its calendar's day of the time
    /// [`Machine::moved`] says (DESIGN 7.5). After the stage goes up, both are counted from the time
    /// it went up. Which of them is set is [`Machine::set_at`]'s to say.
    pub(crate) fn clocks(&self, stage: &str, started: i64, held: u32, up: u8) -> Result<(Option<i64>, Option<i64>), Refusal> {
        let Some(dates) = &self.suite.dates else { return Ok((None, None)) };
        let moved = self.moved(started, held)?;
        let ascii = self.s.stages().unwrap_or_default().iter().find(|n| n.text == stage).map(|n| n.ascii().to_string()).unwrap_or_else(|| stage.to_string());
        // every date of the lowered deadlines, by the request's name of it, with its time of day, the
        // UTC offset of the calendar it counts in and the day it counts from
        let mut found: Vec<(String, DateValue, Option<u32>, i32, i64)> = Vec::new();
        for tf in self.lowered.files.iter() {
            let Part::Terms(cal) = &tf.part else { continue };
            let offset = self.offset_in(Some(cal));
            let start = crate::simulate::day_at(moved, offset);
            let p = self.lowered.path(tf);
            let d = crate::lex::date_text(start);
            let no = |m: &ritsu_base::text::Text| Refusal::new("data", tr!("ステップ `{stage}` の期限を {d} から数えられません: {}", "the deadlines of the stage `{stage}` cannot be counted from {d}: {}", tf.named_back(&m.ja); tf.named_back(&m.en)));
            let facts = self.fs(|| dates.facts(&p)).map_err(|e| no(&e.first().map(|x| x.message.clone()).unwrap_or_default()))?;
            let Some(started) = facts.inputs.first() else { continue };
            let got = self.fs(|| dates.eval(&p, &[(started.name.clone(), start)])).map_err(|e| no(&e.first().map(|x| x.message.clone()).unwrap_or_default()))?;
            for (n, v) in got {
                let want = tf.renamed.iter().find(|(g, _)| *g == n).map(|(_, w)| w.clone()).unwrap_or_else(|| n.clone());
                let at = facts.functions.iter().find(|f| f.name == n).and_then(|f| f.at);
                found.push((want, v, at, offset, start));
            }
        }
        let has = |name: &str| found.iter().any(|(n, ..)| n == name);
        let time = |name: &str| -> Result<Option<i64>, Refusal> {
            match found.iter().find(|(n, ..)| n == name) {
                None => Ok(None),
                Some((_, DateValue::Day(d), at, offset, _)) => Ok(Some(Self::instant_in(*d, at.unwrap_or(0), *offset))),
                Some((_, DateValue::Stopped(t), .., start)) => {
                    let d = crate::lex::date_text(*start);
                    Err(Refusal::new("data", tr!("ステップ `{stage}` の期限を {d} から数えられません: {}", "the deadlines of the stage `{stage}` cannot be counted from {d}: {}", t.ja; t.en)))
                }
            }
        };
        let after = match up {
            0 => "",
            1 => "_escalated",
            _ => "_escalated_twice",
        };
        let (own_due, own_remind) = (format!("{ascii}_due"), format!("{ascii}_remind"));
        let due_name = if has(&own_due) { own_due.as_str() } else { "due" };
        let due = time(&format!("{due_name}{after}"))?;
        // the stage's own reminder, else the one of every stage; after going up, a stage with a
        // deadline or a reminder of its own has reminders of its own, counted from its deadlines
        let remind = if up == 0 {
            time(if has(&own_remind) { &own_remind } else { "remind" })?
        } else if has(&own_due) || has(&own_remind) {
            time(&format!("{own_remind}{after}"))?
        } else {
            time(&format!("remind{after}"))?
        };
        // a reminder counted in another calendar than the deadline is no date of a lowered file
        // after the stage goes up (a file counts in one calendar): it is counted here, in the
        // reminder's calendar, from the day the deadline before passed where that calendar is, the
        // same business days at the same time
        let remind = match remind {
            None if up > 0 => {
                let before = if up == 1 { "" } else { "_escalated" };
                match (crate::apart::of(self.s, self.calendars.first().map(|(a, _)| a.as_str()), stage), time(&format!("{due_name}{before}"))?) {
                    (Some(a), Some(went_up)) => {
                        let cal = a.remind.1.map(|w| w.text.as_str());
                        Some(Self::instant_in(self.add_open(cal, self.day_in(went_up, cal), a.remind.0)?, a.remind.2, self.offset_in(cal)))
                    }
                    _ => None,
                }
            }
            r => r,
        };
        Ok((remind.filter(|_| due.is_some()), due))
    }

    /// The reminder and the deadline as they are set at `now`: neither before `now`, and the
    /// reminder only where it comes before the deadline (DESIGN 5.10).
    pub(crate) fn set_at(remind: Option<i64>, due: Option<i64>, now: i64) -> (Option<i64>, Option<i64>) {
        let due = due.map(|d| d.max(now));
        (remind.map(|r| r.max(now)).filter(|r| due.is_some_and(|d| *r < d)), due)
    }

    /// The clocks of the ticket's stage, counted from the day it started (and the days it was paused
    /// for) and set, none before `now`, the reminder only where it comes before the deadline. A
    /// stage that starts (`fresh`) sets both. A stage come back to, resumed or come back to by a
    /// retracted approval, counts again the clocks it has not heard ring: the deadline, where it is
    /// set, and the reminder, where it has not rung (`reminded`), set before or not (DESIGN 7.5).
    pub(crate) fn set_clocks(&self, t: &mut Ticket, i: usize, fresh: bool, now: i64, fx: &mut super::Effects) -> Result<(), Refusal> {
        use super::{Timer, What};
        let round = t.round;
        let st = &mut t.stages[i];
        let Some(started) = st.started else { return Ok(()) };
        let (remind, due) = self.clocks(&st.name, started, st.held, st.up)?;
        let (rung, had_due) = (st.reminded, st.due.is_some());
        (st.remind, st.due) = Self::set_at(remind.filter(|_| fresh || !rung), due.filter(|_| fresh || had_due), now);
        if let Some(at) = st.remind {
            fx.put(round, What::SetTimer { timer: Timer::Remind, stage: Some(st.name.clone()), at });
        }
        if let Some(at) = st.due {
            fx.put(round, What::SetTimer { timer: Timer::Late, stage: Some(st.name.clone()), at });
        }
        Ok(())
    }


    /// The clocks of the ticket's stage that are set are cancelled; the stage keeps their times,
    /// for them to be set again. A paused ticket has none of them set: the pause cancelled them.
    pub(crate) fn cancel_clocks(&self, t: &Ticket, fx: &mut super::Effects) {
        use super::{Timer, What};
        if t.status == super::Status::Paused {
            return;
        }
        let Some(st) = t.stage() else { return };
        if st.remind.is_some() {
            fx.put(t.round, What::CancelTimer { timer: Timer::Remind, stage: Some(st.name.clone()) });
        }
        if st.due.is_some() {
            fx.put(t.round, What::CancelTimer { timer: Timer::Late, stage: Some(st.name.clone()) });
        }
    }
}
