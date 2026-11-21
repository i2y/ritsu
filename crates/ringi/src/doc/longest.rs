//! `ritsu ringi doc --longest` (DESIGN 11.3): the longest a ticket of a request can take (7.6), with
//! the dates of a ticket submitted on a chosen day, and what each deadline, the limit on sending
//! back, the requester's turn and the pause add to it.
//!
//! The dates walk the longest path of the route as the ticket machine counts its clocks
//! ([`Machine::clocks`]): a ticket submitted on the day, at every stage approved just before its
//! last deadline, after going up as often as `when late` lets it; sent back at the last stage as
//! often as `send back … at most` lets it, and submitted again on the last day `resubmit within`
//! gives; paused at its first stage for all `pause at most` gives. The day is the one `--on` says,
//! else the example day of the page: the first business day the request can be used on that comes
//! before a day the calendar closes.

use super::words::{Say, day_text, grouped};
use super::{Block, Page, h2};
use crate::ast::{At, Late, Route, TicketLine};
use crate::check::{Outcome, Run};
use crate::engine::Machine;
use crate::ends::{self, Count, Takes};
use crate::org::listing::Occ;
use ritsu_base::text::{Lang, Text};

/// The page of the longest ticket of the request `o`, submitted on `on` (else the example day).
pub fn page(o: &Outcome, run: &Run, say: &Say, shown: &str, sha: &str, on: Option<i64>) -> Result<Page, Text> {
    let title = say.title();
    let mut out = super::request::head(say, shown, sha);
    let count = Count::of(say.s);
    out.push(h2(say.t("The longest a ticket takes", "いちばん長くかかる日数"), "longest"));
    let (Some(total), Some(round), Some(at)) = (count.total, count.round, count.longest) else {
        no_end(say, &count, &mut out);
        return Ok(Page { file: shown.to_string(), sha: sha.to_string(), lang: say.lang, title: say.t(format!("{title}: the longest a ticket takes"), format!("{title}：いちばん長くかかる場合")), description: None, blocks: out });
    };
    let k = count.backs.map_or(0, |(n, _)| n);
    let r = count.resubmit.map_or(0, |(n, _)| n);
    let p = count.pause.map_or(0, |(n, _)| n);
    let path = &count.paths[at];
    out.push(Block::Para(say.t(
        format!("A ticket of this request ends within {} business days at the most:", grouped(total as i128)),
        format!("この申請の種類のチケットは、長くても {} 営業日で終わります。内訳は次のとおりです。", grouped(total as i128)),
    )));
    let mut rows = vec![vec![
        say.t(format!("The longest path of the route, {} times: once, and again each time it is sent back", k + 1), format!("経路のいちばん長い並びを {} 回（はじめの 1 回と、差し戻しのたびに 1 回）", k + 1)),
        say.t(format!("{} × {}", k + 1, grouped(round as i128)), format!("{} × {}", k + 1, grouped(round as i128))),
    ]];
    if k > 0 {
        rows.push(vec![say.t(format!("With the requester, to submit it again, {k} times"), format!("申請者の手元で出し直しを待つ（{k} 回）")), format!("{k} × {}", grouped(r as i128))]);
    }
    if p > 0 {
        rows.push(vec![say.t("Paused", "保留"), grouped(p as i128)]);
    }
    rows.push(vec![say.t("In all", "合計"), grouped(total as i128)]);
    out.push(Block::Table { head: vec![say.t("What", "内訳"), say.t("Business days", "営業日")], rows });
    path_section(say, path, &count, &mut out);
    let lowered = o.lowered.as_ref();
    match lowered {
        Some(l) => {
            let m = Machine::new(say.s, &run.suite, l, o.usable.as_ref());
            let day = match on {
                Some(d) => {
                    m.usable_on(d).map_err(|r| r.text)?;
                    Some(d)
                }
                None => example_day(&m, o),
            };
            if let Some(day) = day {
                dates(say, &m, path, &count, day, total, &mut out);
            }
        }
        None => {}
    }
    shorter(say, &count, total, &mut out);
    Ok(Page { file: shown.to_string(), sha: sha.to_string(), lang: say.lang, title: say.t(format!("{title}: the longest a ticket takes"), format!("{title}：いちばん長くかかる場合")), description: None, blocks: out })
}

/// What the page says where a ticket may never end: the stages that wait without end, and a turn
/// of the requester with no end.
fn no_end(say: &Say, count: &Count, out: &mut Vec<Block>) {
    out.push(Block::Para(say.t("A ticket of this request may never end, so it has no longest:", "この申請の種類のチケットは、終わらないことがあります。いちばん長い日数はありません。")));
    let mut items = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for o in count.paths.iter().flatten() {
        if seen.contains(&o.stage.as_str()) {
            continue;
        }
        let st = say.stage(&o.stage);
        match ends::takes(o) {
            Takes::Stays => items.push(say.t(format!("The stage “{st}” waits on when it is late."), format!("ステップ「{st}」は、期限を過ぎてもそのまま待ちます。"))),
            Takes::NoDeadline => items.push(say.t(format!("The stage “{st}” has no deadline."), format!("ステップ「{st}」には期限がありません。"))),
            _ => continue,
        }
        seen.push(&o.stage);
    }
    if count.backs.is_some() && count.resubmit.is_none() {
        items.push(say.t(
            "A ticket sent back to the requester waits for them with no end: the request says no `resubmit within`.",
            "申請者へ差し戻したチケットは、終わり無く申請者を待ちます。出し直しの期限（`resubmit within`）がありません。",
        ));
    }
    out.push(Block::List(items));
}

/// The longest path: the case it is, and each stage with who answers it, its deadline, what it does
/// when it is late and the business days it takes.
fn path_section(say: &Say, path: &[Occ], count: &Count, out: &mut Vec<Block>) {
    out.push(h2(say.t("The longest path", "いちばん長い並び"), "path"));
    if let Some(case) = case(say, path) {
        out.push(Block::Para(case));
    }
    let head = vec![say.t("Stage", "ステップ"), say.t("Who answers", "答える人"), say.t("Deadline", "期限"), say.t("When it is late", "期限を過ぎたら"), say.t("Business days at the most", "長くて何営業日")];
    let rows = path
        .iter()
        .filter(|o| !matches!(ends::takes(o), Takes::Nothing))
        .map(|o| {
            let days = ends::takes(o).days().unwrap_or(0) * ends::starts(say.s, o);
            vec![say.stage(&o.stage), say.spec(o.spec), due_text(say, o), late_text(say, o), grouped(days as i128)]
        })
        .collect();
    out.push(Block::Table { head, rows });
    if let Some(r) = count.round {
        out.push(Block::Para(say.t(format!("One round of this path takes {} business days at the most.", grouped(r as i128)), format!("この並びを一回通ると、長くて {} 営業日です。", grouped(r as i128)))));
    }
}

/// The case the longest path is: the row of the route's table it is, in words; for stage lines,
/// that every stage comes.
fn case(say: &Say, path: &[Occ]) -> Option<String> {
    match &say.s.file.route {
        Some(Route::Table(t)) => {
            let line = path.first()?.line;
            let row = t.rows.iter().find(|r| r.line == line)?;
            let parts: Vec<String> = t
                .inputs
                .iter()
                .zip(&row.tests)
                .filter(|(_, (test, _))| !matches!(test, crate::ast::Test::Any))
                .map(|(c, (test, _))| say.t(format!("{}: {}", say.column(c), say.test(c, test)), format!("{}：{}", say.column(c), say.test(c, test))))
                .collect();
            if parts.is_empty() {
                return Some(say.t("It is the path of every request.", "どの申請もこの並びです。"));
            }
            Some(say.t(format!("It is the path of the requests of this case: {}.", parts.join("; ")), format!("次の場合の申請の並びです。{}。", parts.join("、"))))
        }
        Some(Route::Stages(_)) | Some(Route::Shared { .. }) if path.iter().any(|o| o.cond) => Some(say.t("Every stage comes, those with a condition too.", "条件のあるステップも、どれも来るものとして数えます。")),
        _ => None,
    }
}

/// A stage's deadline in words: `3 business days, at the end of the day`.
fn due_text(say: &Say, o: &Occ) -> String {
    match o.due {
        None => "—".to_string(),
        Some(d) => {
            let n = say.days(d.days, d.calendar);
            match d.at {
                At::EndOfDay => say.t(format!("{n}, at the end of the day"), format!("{n}の日の終わり")),
                At::Time(t) => {
                    let t = crate::lex::time_text(t);
                    say.t(format!("{n}, at {t}"), format!("{n}の日の {t}"))
                }
            }
        }
    }
}

/// What a stage does when it is late, in words.
fn late_text(say: &Say, o: &Occ) -> String {
    let one = |l: &Late| match l {
        Late::Escalate(Some(t)) => say.t(format!("goes up as the table `{}` says", t.text), format!("表 `{}` のとおりに上へ回す", t.text)),
        Late::Escalate(None) => say.t("goes up to the head above", "上の長へ回す"),
        Late::Expire => say.t("expires", "期限切れで終わる"),
        Late::Stay => say.t("waits on", "そのまま待つ"),
    };
    match o.late {
        None => say.t("expires", "期限切れで終わる"),
        Some(l) => match l.then {
            Some(t) => say.t(format!("{}, then {}", one(l.first), one(t)), format!("{}。次は{}", one(l.first), one(t))),
            None => one(l.first),
        },
    }
}

/// The first business day the request can be used on that comes before a day the calendar closes
/// (the day DESIGN 11.3 submits on), in the request's first calendar; else its first business day.
/// None where the request reads no calendar.
pub fn example_day(m: &Machine, o: &Outcome) -> Option<i64> {
    let (_, cal) = m.calendars.first()?;
    let from = o.usable.as_ref().and_then(|u| u.from.as_ref()).map_or(cal.data.0, |e| e.day).max(cal.data.0);
    let through = o.usable.as_ref().and_then(|u| u.through.as_ref()).map_or(cal.data.1, |e| e.day).min(cal.data.1);
    let open = |d: i64| !cal.closed.contains(&d);
    let last = through.min(from + 366);
    (from..last).find(|d| open(*d) && !open(d + 1)).or_else(|| (from..=last).find(|d| open(*d)))
}

/// The instant a stage that starts on the day `day` of the request is taken to start at: its noon,
/// so that each calendar the deadlines count in reads the same day of it (DESIGN 7.5), whatever
/// its UTC offset within twelve hours of the request's.
pub(crate) fn started_on(m: &Machine, day: i64) -> i64 {
    m.instant(day, 720)
}

/// An instant as the page writes it, in the days of the request: `2026-04-08 (Wed) 17:00`; a
/// deadline at the end of a day, which rings at the next day's 00:00, as `2026-04-08 (Wed), at the
/// end of the day`.
pub fn instant_text(m: &Machine, at: i64, end_of_day: bool, lang: Lang) -> String {
    let day = m.day(at);
    let minutes = (at + i64::from(m.offset) * 60 - day * 86_400) / 60;
    if end_of_day && minutes == 0 {
        let d = day_text(day - 1, lang);
        return match lang {
            Lang::En => format!("{d}, at the end of the day"),
            Lang::Ja => format!("{d}の終わり"),
        };
    }
    let t = crate::lex::time_text(minutes as u32);
    match lang {
        Lang::En => format!("{} {t}", day_text(day, lang)),
        Lang::Ja => format!("{}{t}", day_text(day, lang)),
    }
}

/// The dates of the longest ticket submitted on `day`: a row for each thing that happens, and the
/// day it ends.
fn dates(say: &Say, m: &Machine, path: &[Occ], count: &Count, day: i64, total: u64, out: &mut Vec<Block>) {
    let d = day_text(day, say.lang);
    out.push(h2(say.t(format!("A ticket submitted on {d}"), format!("{d}に出したチケット")), "dates"));
    let mut how = vec![say.t("Each stage is approved just before its last deadline, after going up as often as it can.", "どのステップも、上へ回れるだけ回ったあと、最後の期限の直前に承認されます。")];
    if count.backs.is_some() {
        how.push(say.t(
            "The ticket is sent back to the requester at the last stage as often as it can be, and submitted again on the last day.",
            "チケットは、最後のステップから上限の回数まで申請者へ差し戻され、申請者は出し直しの期限の日に出し直します。",
        ));
    }
    if count.pause.is_some() {
        how.push(say.t("It is paused at its first stage for all the days it can be.", "保留は、最初のステップで使える日数を全部使います。"));
    }
    out.push(Block::Para(say.t(
        how.join(" "),
        how.concat(),
    )));
    let lines: &[TicketLine] = say.s.file.ticket.as_ref().map(|t| t.lines.as_slice()).unwrap_or(&[]);
    let resubmit_cal = lines.iter().find_map(|l| if let TicketLine::Resubmit { calendar, .. } = l { Some(calendar.as_ref()) } else { None }).flatten();
    let pause_cal = lines.iter().find_map(|l| if let TicketLine::Pause { calendar, .. } = l { Some(calendar.as_ref()) } else { None }).flatten();
    let k = count.backs.map_or(0, |(n, _)| n);
    let r = count.resubmit.map_or(0, |(n, _)| n);
    let p = count.pause.map_or(0, |(n, _)| n);
    let stages: Vec<&Occ> = path.iter().filter(|o| !matches!(ends::takes(o), Takes::Nothing)).collect();
    let mut rows: Vec<Vec<String>> = vec![vec![day_text(day, say.lang), say.t("The requester submits it.", "申請者が出します。")]];
    let mut at = day;
    let mut stopped: Option<Text> = None;
    'rounds: for round in 0..=k {
        for (i, o) in stages.iter().enumerate() {
            let st = say.stage(&o.stage);
            let last = i + 1 == stages.len();
            for start in 0..ends::starts(say.s, o) {
                let mut begins = at;
                rows.push(vec![day_text(at, say.lang), if start == 0 { say.t(format!("The stage “{st}” starts."), format!("ステップ「{st}」が始まります。")) } else { say.t(format!("The stage “{st}” starts again, for its next approver."), format!("ステップ「{st}」が、次の承認者のために、もう一度始まります。")) }]);
                if round == 0 && i == 0 && start == 0 && p > 0 {
                    match m.add_open(pause_cal.map(|w| w.text.as_str()), at, p) {
                        Ok(back) => {
                            rows.push(vec![day_text(at, say.lang), say.t(format!("An approver pauses it, for {}.", say.days(p, pause_cal)), format!("承認者が保留にします（{}）。", say.days(p, pause_cal)))]);
                            rows.push(vec![day_text(back, say.lang), say.t("The pause runs out and the ticket goes on by itself: its deadlines move later by the days it was paused.", "保留の日数が尽き、チケットは自動で戻ります。期限は、保留にした営業日だけ後ろへずれます。")]);
                            begins = back;
                        }
                        Err(e) => {
                            stopped = Some(e.text);
                            break 'rounds;
                        }
                    }
                }
                let times = match ends::takes(o) {
                    Takes::Up { times, .. } => times,
                    _ => 0,
                };
                let end_of_day = o.due.is_some_and(|d| d.at == At::EndOfDay);
                for up in 0..times {
                    match m.clocks(&o.stage, started_on(m, begins), 0, up as u8) {
                        Ok((_, Some(due))) => rows.push(vec![instant_text(m, due, end_of_day, say.lang), say.t(format!("The deadline of the stage “{st}” passes, and it goes up."), format!("ステップ「{st}」が期限を過ぎ、上へ回ります。"))]),
                        Ok((_, None)) => {}
                        Err(e) => {
                            stopped = Some(e.text);
                            break 'rounds;
                        }
                    }
                }
                let due = match m.clocks(&o.stage, started_on(m, begins), 0, times as u8) {
                    Ok((_, Some(due))) => due,
                    Ok((_, None)) => continue,
                    Err(e) => {
                        stopped = Some(e.text);
                        break 'rounds;
                    }
                };
                let when = instant_text(m, due, end_of_day, say.lang);
                let back = last && round < k && start + 1 == ends::starts(say.s, o);
                let what = match (back, last && round == k && start + 1 == ends::starts(say.s, o)) {
                    (true, _) => say.t(format!("The stage “{st}” sends it back to the requester just before its deadline, which is {when}."), format!("ステップ「{st}」が、期限の直前に申請者へ差し戻します。期限は{when}です。")),
                    (false, true) => say.t(format!("The stage “{st}” approves it just before its deadline, which is {when}: the ticket ends approved."), format!("ステップ「{st}」が、期限の直前に承認します。期限は{when}です。チケットは承認で終わります。")),
                    _ => say.t(format!("The stage “{st}” approves it just before its deadline, which is {when}."), format!("ステップ「{st}」が、期限の直前に承認します。期限は{when}です。")),
                };
                // approved one minute before the deadline: the day the next stage starts on
                at = m.day(due - 60);
                rows.push(vec![day_text(at, say.lang), what]);
                if back {
                    match m.add_open(resubmit_cal.map(|w| w.text.as_str()), at, r) {
                        Ok(again) => {
                            at = again;
                            rows.push(vec![day_text(at, say.lang), say.t(format!("On the last day of the {} to submit it again, the requester submits it again.", say.days(r, resubmit_cal)), format!("出し直しの期限（{}）の最後の日に、申請者が出し直します。", say.days(r, resubmit_cal)))]);
                        }
                        Err(e) => {
                            stopped = Some(e.text);
                            break 'rounds;
                        }
                    }
                }
            }
        }
    }
    out.push(Block::Table { head: vec![say.t("Day", "日"), say.t("What happens", "起きること")], rows });
    if let Some(t) = stopped {
        out.push(Block::Note(say.t(format!("The dates stop here: {}.", t.en), format!("日付はここまでです。{}。", t.ja))));
        return;
    }
    let walked = m.open_between(None, day, at).unwrap_or(0);
    let (a, b) = (day_text(day, say.lang), day_text(at, say.lang));
    out.push(Block::Para(say.t(format!("From {a} to {b}: {} business days.", grouped(i128::from(walked))), format!("{a}から{b}まで、{} 営業日です。", grouped(i128::from(walked))))));
    if u64::from(walked) < total {
        out.push(Block::Note(say.t(
            format!("No ticket takes longer than {} business days, whatever the days the calendar closes; a ticket that takes all of them need not be there. On these dates it takes fewer: where a deadline at the end of a day runs out on the eve of a day the calendar closes, the stage goes up on the closed day, which is not a business day, and so takes one business day less.", grouped(total as i128)),
            format!("カレンダーの休みがどうであっても、{} 営業日より長くかかるチケットはありません。ただし、ちょうどそれだけかかるチケットがあるとは限りません。この日付では短くなります。日の終わりの期限が休みの前の日に切れると、ステップは休みの日に上へ回ります。回す日が営業日に入らないので、そのステップは 1 営業日短くなります。", grouped(total as i128)),
        )));
    }
}

/// What shortens the longest ticket: a business day less of each deadline on the longest path, one
/// time less of sending back, a day less of the requester's turn and of the pause; each with what
/// the longest becomes, counted as 7.6 counts it (another path may become the longest).
fn shorter(say: &Say, count: &Count, total: u64, out: &mut Vec<Block>) {
    let k = count.backs.map_or(0, |(n, _)| u64::from(n));
    let r = count.resubmit.map_or(0, |(n, _)| u64::from(n));
    let p = count.pause.map_or(0, |(n, _)| u64::from(n));
    // the deadlines of the longest path, each line once, in the order the path meets them
    let mut lines: Vec<(*const crate::ast::File, usize, u32)> = Vec::new();
    let mut named: Vec<String> = Vec::new();
    let path = count.longest.map(|i| &count.paths[i]);
    for o in path.into_iter().flatten() {
        let Some(d) = o.due else { continue };
        if matches!(ends::takes(o), Takes::Nothing) || d.days < 2 {
            continue;
        }
        let key = (d.file as *const crate::ast::File, d.line, d.days);
        if lines.contains(&key) {
            continue;
        }
        // whose deadline the line is: every stage's (of the request or of its shared route), or a stage's own
        let every = d.file.every_stage.as_ref().is_some_and(|e| e.lines.iter().any(|l| matches!(l, crate::ast::Deadline::Due { line, .. } if *line == d.line)));
        let whose = match (every, std::ptr::eq(d.file, say.s.file)) {
            (true, true) => say.t("Every stage", "どのステップも"),
            (true, false) => say.t(format!("Every stage of the shared route `{}`", d.file.name.text), format!("共有の経路 `{}` のどのステップも", d.file.name.text)),
            (false, _) => {
                let st = say.stage(&o.stage);
                say.t(format!("The stage “{st}”"), format!("ステップ「{st}」は"))
            }
        };
        let n = d.days;
        named.push(say.t(format!("{whose} due in {} instead of {}", say.days(n - 1, d.calendar), say.days(n, d.calendar)), format!("{whose}期限を {}に（今は {}）", say.days(n - 1, d.calendar), say.days(n, d.calendar))));
        lines.push(key);
    }
    let mut rows = Vec::new();
    for (key, change) in lines.iter().zip(&named) {
        let less = |o: &Occ| -> Option<u64> {
            let mut t = ends::takes(o);
            if o.due.is_some_and(|d| (d.file as *const crate::ast::File, d.line, d.days) == *key) {
                t = match t {
                    Takes::Due(n) => Takes::Due(n - 1),
                    Takes::Up { days, times, day } => Takes::Up { days: days - 1, times, day },
                    t => t,
                };
            }
            Some(t.days()? * ends::starts(say.s, o))
        };
        if let Some(t) = recount(count, &less, k, r, p) {
            rows.push(vec![change.clone(), grouped(t as i128), grouped(total.saturating_sub(t) as i128)]);
        }
    }
    let same = |o: &Occ| -> Option<u64> { Some(ends::takes(o).days()? * ends::starts(say.s, o)) };
    if k > 0
        && let Some(t) = recount(count, &same, k - 1, r, p)
    {
        let change = say.t(format!("Sent back at most {} instead of {}", times(k - 1, Lang::En), times(k, Lang::En)), format!("差し戻しを {} 回までに（今は {} 回）", k - 1, k));
        rows.push(vec![change, grouped(t as i128), grouped(total.saturating_sub(t) as i128)]);
    }
    if k > 0
        && r > 1
        && let Some(t) = recount(count, &same, k, r - 1, p)
    {
        let change = say.t(format!("Submitted again within {} instead of {}", say.days((r - 1) as u32, None), say.days(r as u32, None)), format!("出し直しの期限を {}に（今は {}）", say.days((r - 1) as u32, None), say.days(r as u32, None)));
        rows.push(vec![change, grouped(t as i128), grouped(total.saturating_sub(t) as i128)]);
    }
    if p > 0
        && let Some(t) = recount(count, &same, k, r, p - 1)
    {
        let change = say.t(format!("Paused at most {} instead of {}", say.days((p - 1) as u32, None), say.days(p as u32, None)), format!("保留を {}までに（今は {}）", say.days((p - 1) as u32, None), say.days(p as u32, None)));
        rows.push(vec![change, grouped(t as i128), grouped(total.saturating_sub(t) as i128)]);
    }
    if rows.is_empty() {
        return;
    }
    out.push(h2(say.t("What shortens it", "短くするには"), "shorter"));
    out.push(Block::Para(say.t(
        "Each row changes one thing of the request, and says what the longest becomes, counted the same way: where another path becomes the longest, it is counted on that path.",
        "一行ごとに、申請の種類の一つを変えたときの、いちばん長い日数です。数え方は同じで、ほかの並びのほうが長くなれば、その並びで数えます。",
    )));
    out.push(Block::Table { head: vec![say.t("Change", "変えること"), say.t("The longest becomes", "いちばん長い日数"), say.t("Shorter by", "短くなる日数")], rows });
}

/// The longest ticket, counted as 7.6 counts it, with what each stage takes (`stage`), the times
/// it goes back (`k`), the requester's turn (`r`) and the pause (`p`).
fn recount(count: &Count, stage: &dyn Fn(&Occ) -> Option<u64>, k: u64, r: u64, p: u64) -> Option<u64> {
    let round = count.paths.iter().map(|path| path.iter().try_fold(0u64, |a, o| Some(a + stage(o)?))).collect::<Option<Vec<u64>>>()?.into_iter().max().unwrap_or(0);
    Some((k + 1) * round + k * r + p)
}

/// `at most 3 times` in English; the Japanese is written by the caller.
fn times(n: u64, lang: Lang) -> String {
    match lang {
        Lang::En => format!("{n} {}", if n == 1 { "time" } else { "times" }),
        Lang::Ja => format!("{n} 回"),
    }
}
