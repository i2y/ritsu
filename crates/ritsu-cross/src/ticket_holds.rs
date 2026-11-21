//! A hold a ticket makes, held to the book it is made in (ringi's DESIGN 9.3, X19). A request holds
//! what it asks for when it is submitted (`on submitted hold budget.reserve(…)`), and the ending
//! of the round posts or voids the hold; the book says when a hold expires (`pending expires
//! after`). ringi's own check shows that every ending closes the hold (its E405); here the lengths
//! are compared, as X5 compares them for a workflow.
//!
//! ringi says how long a hold stays open, in business days (`Requests::ticket`: one round, and the
//! pause; where a stage can send the ticket back to an earlier stage, which starts no new round,
//! each time the round is walked again), the calendars the days are counted in, and the days a
//! request can be submitted on (`Requests::usable`); koyomi says which days each calendar closes
//! (`Dates::calendar`); chobo says when the hold expires (`Books::facts`). The business days are
//! laid on the calendar from every day a request can be submitted on, and the longest stretch is
//! taken: a hold made at 00:00 of the day, closed at the end of the last. Each hold is a border:
//!
//! - the longest stretch is shorter than the expiry, or the hold never expires: shown to hold.
//! - the longest stretch reaches the expiry (E209), with the day of the submission that makes it.
//!   The business days are what ringi's check counts, an upper bound it holds its own claims to.
//! - otherwise undecided (W209): a stage of the round has no end; the days are counted in several
//!   calendars, and the stretch over the days all of them open (which no ticket may take) reaches
//!   the expiry; or the calendar's data does not cover a round.
//!
//! An argument of the hold that reads where the requester is in the organization (`dept:
//! requester.department`) is a border too: the book takes a value, and not every requester has
//! one. Where every one who may submit has it, it holds; where some do not, their `submit` is
//! refused, which is said as a warning (W209), by the version of the organization and how many of
//! how many; where the organization has no data it is undecided (W209).

use crate::{Border, Borders, Came, Check, Side};
use ritsu_base::diag::Diag;
use ritsu_base::json::Json;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_ports::{Books, CalendarFacts, Dates, Day, Expiry, Finding, Found, HoldArg, RequestTicket, Requests, TicketHold, TicketTime, day_text, seconds_text};
use ritsu_project::{Joined, Project};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

/// A whole turn of the Gregorian calendar, in days: a calendar with no table of closed days
/// repeats after it, so the days a request is submitted on past one turn add nothing.
const TURN: i64 = 146_097;

/// The last day of a calendar with no table of closed days (koyomi's 9999-12-31).
const NO_TABLE_END: Day = 2_932_896;

/// The longest stretch a hold stays open, and the submission that makes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Longest {
    /// The day the request is submitted on.
    pub submitted: Day,
    /// The last day of the round: the `held`th open day after it.
    pub last: Day,
    /// From the first moment of the one to the end of the other, in days.
    pub days: i64,
}

/// The longest stretch `held` business days make, counted from a day a request is submitted on
/// (`from` through `through`) to the `held`th open day after it, both days in: `open` is the open
/// days, ascending, from `from` on. The first day that makes the longest. None where no day of the
/// range has `held` open days after it in `open`.
pub fn longest(open: &[Day], from: Day, through: Day, held: u64) -> Option<Longest> {
    if from > through || held == 0 {
        return None;
    }
    // the longest from each open day on is from the open day itself: a closed day after it is nearer
    // the same last day. So the days to try are the first of the range and each open day in it.
    let mut best: Option<Longest> = None;
    for d0 in std::iter::once(from).chain(open.iter().copied().filter(|d| *d > from && *d <= through)) {
        let next = open.partition_point(|o| *o <= d0);
        let Some(last) = open.get(next + held as usize - 1).copied() else { break };
        let days = last - d0 + 1;
        if best.is_none_or(|b| days > b.days) {
            best = Some(Longest { submitted: d0, last, days });
        }
    }
    best
}

/// What a hold's expiry comes to against the longest stretch it stays open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    /// The longest stretch ends before the hold expires.
    Held,
    /// It reaches the expiry, in one calendar (E209).
    Expires,
    /// It reaches the expiry over the days several calendars all open, which is more than a ticket
    /// may take (W209).
    MayExpire,
}

/// Whether a hold that expires `expiry` seconds after it is made is still held at the end of a
/// stretch of `days` days, counted in `calendars` calendars whose offsets are `spread` seconds
/// apart at the most. A hold whose expiry the clock reaches has expired (chobo's DESIGN 2.5), so a
/// stretch that ends at that very second does not hold.
pub fn judge(days: i64, spread: u64, expiry: u64, calendars: usize) -> Kept {
    let secs = (days.max(0) as u64).saturating_mul(86_400).saturating_add(spread);
    match (secs < expiry, calendars) {
        (true, _) => Kept::Held,
        (false, 1) => Kept::Expires,
        (false, _) => Kept::MayExpire,
    }
}

/// X19 over the project, through the ports `joined` holds.
pub(crate) fn check(project: &Project, joined: &Joined, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    check_with(project, &*joined.requests(), &*joined.chobo, &*joined.koyomi, lang, borders)
}

/// [`check`], with each port given: what ringi says of the tickets, chobo of the books and koyomi
/// of the calendars.
pub fn check_with(project: &Project, requests: &dyn Requests, books: &dyn Books, dates: &dyn Dates, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    let mut out = Vec::new();
    // each calendar is read once: its closed days are all the days of its data
    let calendars: Calendars = RefCell::default();
    for f in project.of(Tool::Ringi) {
        // by the path the file's own check was run with, which ringi keeps the check under
        let disk = std::path::Path::new(&f.shown);
        let Ok(Found::Value(t)) = requests.ticket(disk) else { continue };
        if t.holds.is_empty() {
            continue;
        }
        let src = ritsu_base::fs::read_to_string(disk).unwrap_or_default();
        let usable = requests.usable(disk).unwrap_or_default();
        let one = One { project, file: f, src: &src, t: &t, from: usable.from.map(|e| e.day), through: usable.through.map(|e| e.day), calendars: &calendars };
        for h in &t.holds {
            for a in &h.args {
                let (b, d) = one.argument(h, a);
                borders.add(b);
                out.extend(d.map(|d| Finding::of(&d, Some(f.rel.clone()), lang)));
            }
            let Ok(facts) = books.facts(&h.book) else { continue };
            let Some(expiry) = facts.transfers.iter().find(|x| x.name == h.transfer).and_then(|x| x.pending) else { continue };
            let Some((b, d)) = one.expiry(h, expiry, dates) else { continue };
            borders.add(b);
            out.extend(d.map(|d| Finding::of(&d, Some(f.rel.clone()), lang)));
        }
    }
    out
}

/// The calendars read in a run, by their paths; None for one that does not pass koyomi's check.
type Calendars = RefCell<BTreeMap<PathBuf, Option<Rc<CalendarFacts>>>>;

/// One request that makes a hold.
struct One<'a> {
    project: &'a Project,
    file: &'a ritsu_project::File,
    src: &'a str,
    t: &'a RequestTicket,
    /// The days a request can be submitted on; None at an end no data decides.
    from: Option<Day>,
    through: Option<Day>,
    calendars: &'a Calendars,
}

/// How the business days a hold stays open are made up, in words.
fn counted(t: &TicketTime, held: u64) -> Text {
    let (round, pause) = (t.round.unwrap_or(0), u64::from(t.pause.unwrap_or(0)));
    let times = if round > 0 { held.saturating_sub(pause) / round } else { 1 };
    let mut ja = format!("一つの `round` が {round} 営業日");
    let mut en = format!("a round of {round}");
    if times > 1 {
        ja.push_str(&format!("、前のステップへ差し戻すと同じ `round` を歩き直すので {times} 回分"));
        en.push_str(&format!(", walked {times} times where a stage sends the ticket back to an earlier stage"));
    }
    if pause > 0 {
        ja.push_str(&format!("、保留が {pause} 営業日"));
        en.push_str(&format!(", and a pause of {pause}"));
    }
    Text { ja, en }
}

/// A few people, as ringi's check lists the requesters of a finding: up to three by name, and how
/// many more (`e9002 (Lawyer 2) and e9003 (Lawyer 3)`, `e0101 (…), e0102 (…), e0201 (…) and 3
/// more`).
fn some(who: &[Text]) -> Text {
    let shown = &who[..who.len().min(3)];
    let more = who.len() - shown.len();
    if more == 0 {
        return Text::list(shown);
    }
    let (ja, en) = (shown.iter().map(|t| t.ja.as_str()).collect::<Vec<_>>().join("、"), shown.iter().map(|t| t.en.as_str()).collect::<Vec<_>>().join(", "));
    tr!("{ja}、ほか {more} 人", "{en} and {more} more")
}

impl One<'_> {
    fn at(&self, code: &'static str, line: usize, col: usize, message: Text) -> Diag {
        Diag::at(code, &self.file.shown, line, col, message).source(self.src).rel(&self.file.rel)
    }

    fn side(&self, h: &TicketHold) -> Side {
        Side::reached(self.project, Tool::Chobo, &h.book, None)
    }

    /// An argument of the hold that reads where the requester is in the organization.
    fn argument(&self, h: &TicketHold, a: &HoldArg) -> (Border, Option<Diag>) {
        let (request, hold, name, written) = (&self.t.named, format!("{}.{}", h.alias, h.transfer), &a.name, &a.written);
        let border = |came: Came, code: Option<&str>| Border::new(Check::TicketHold, format!("{hold}({name})"), Side::at(Tool::Ringi, &self.file.rel, a.line), Some(self.side(h)), came, code);
        match &a.without {
            Found::Undecided(why) => {
                let d = self
                    .at("W209", a.line, a.col, tr!(
                        "`{request}` のどの申請者にも、仮押さえ `{hold}` の引数 `{name}` に渡す値があるかを決められません",
                        "Whether every requester of `{request}` has a value for `{name}`, an argument of the hold `{hold}`, cannot be decided"
                    ))
                    .note(tr!("決められない理由: {}。", "Why it cannot be decided: {}.", why.ja; why.en))
                    .note(tr!(
                        "`{written}` に値の無い申請者では、仮押さえを作れず、`submit` が断られます。",
                        "The hold cannot be made for a requester with no `{written}`, whose `submit` is refused."
                    ))
                    .note(tr!(
                        "組織にデータを置くと（組織のファイルの `data \"<フォルダ>\"`）、値の無い人がいるかが決まります。",
                        "Give the organization its data (`data \"<folder>\"` in its file), and whether anyone has none is decided."
                    ));
                (border(Came::Undecided, Some("W209")), Some(d))
            }
            Found::Value(versions) if versions.iter().all(|v| v.without.is_empty()) => (border(Came::Held, None), None),
            Found::Value(versions) => {
                let mut d = self.at("W209", a.line, a.col, tr!(
                    "`{request}` の申請者の中に、仮押さえ `{hold}` の引数 `{name}` に渡す値の無い人がいます",
                    "Some requesters of `{request}` have no value for `{name}`, an argument of the hold `{hold}`"
                ));
                for v in versions.iter().filter(|v| !v.without.is_empty()) {
                    let (day, n, k, who) = (day_text(v.from), v.requesters, v.without.len(), some(&v.named));
                    let have = if k == 1 { "has" } else { "have" };
                    d = d.note(tr!(
                        "{day} からの組織では、出せる {n} 人のうち {k} 人に `{written}` がありません: {}。",
                        "In the organization from {day}, {k} of the {n} people who may submit the request {have} no `{written}`: {}.",
                        who.ja; who.en
                    ));
                }
                let shown = &self.file.shown;
                let d = d
                    .note(tr!("値が無いと仮押さえを作れないので、その人の `submit` は断られます。", "The hold cannot be made without it: their `submit` is refused."))
                    .note(tr!("だれかは `ritsu ringi simulate {shown} --every-requester` で見られます。", "Who they are: `ritsu ringi simulate {shown} --every-requester`."))
                    .note(tr!(
                        "その人が出さない申請なら `submitted by … except …` で外してください。出す申請なら、だれにもある値を引数に渡すか、行に `when {written} is not none` を書いて、その人の申請では押さえないようにしてください。",
                        "If they do not submit the request, leave them out (`submitted by … except …`). If they do, give the hold a value every requester has, or write `when {written} is not none` on the line, so that their requests hold nothing."
                    ));
                (border(Came::Undecided, Some("W209")), Some(d))
            }
        }
    }

    /// The hold's expiry against the longest it stays open. None where a calendar does not pass
    /// koyomi's check, which is koyomi's to say.
    fn expiry(&self, h: &TicketHold, expiry: Expiry, dates: &dyn Dates) -> Option<(Border, Option<Diag>)> {
        let (request, hold, transfer, time) = (&self.t.named, format!("{}.{}", h.alias, h.transfer), &h.transfer, &self.t.time);
        let book = crate::ticket_gates::partner(self.project, &h.book);
        let border = |came: Came, code: Option<&str>, detail: Vec<(String, Json)>| {
            let mut b = Border::new(Check::TicketHold, hold.clone(), Side::at(Tool::Ringi, &self.file.rel, h.line), Some(self.side(h)), came, code);
            b.detail = detail;
            b
        };
        let Expiry::After(expiry) = expiry else { return Some((border(Came::Held, None, Vec::new()), None)) };
        let after = seconds_text(expiry);
        let expires = tr!("仮押さえは、作ってから {}で期限が切れます（\"{book}\" の `{transfer}`）。", "The hold expires {} after it is made (`{transfer}` of \"{book}\").", after.ja; after.en);
        let refused = tr!(
            "期限の切れた仮押さえの確定と取消を、帳簿は `expired` で拒否します。チケットが承認されても、押さえた分はもうありません。",
            "The book refuses a post or a void of an expired hold with `expired`: the ticket would be approved with nothing held for it."
        );
        let undecided = |why: Text, fix: Option<Text>, detail: Vec<(String, Json)>| {
            let d = self
                .at("W209", h.line, h.col, tr!(
                    "`{request}` のチケットが仮押さえ `{hold}` を確定か取消にする前に、期限が切れるかを決められません",
                    "Whether the hold `{hold}` expires before a ticket of `{request}` posts or voids it cannot be decided"
                ))
                .note(why)
                .note(expires.clone())
                .note(refused.clone());
            let d = match fix {
                Some(fix) => d.note(fix),
                None => d,
            };
            Some((border(Came::Undecided, Some("W209"), detail), Some(d)))
        };
        let Some(held) = time.held else {
            // the waits of a round: the requester's turn between two rounds holds nothing
            let waits: Vec<&ritsu_ports::TicketNoEnd> = time.no_end.iter().filter(|n| n.in_round).collect();
            let what = Text::list(&waits.iter().map(|n| n.text.clone()).collect::<Vec<_>>());
            let fix = Text::list(&waits.iter().map(|n| n.fix.clone()).collect::<Vec<_>>());
            return undecided(
                tr!("`round` に終わりの無い待ちがあります: {}。", "A round of the request may never end: {}.", what.ja; what.en),
                Some(tr!(
                    "`round` に終わりがあれば、仮押さえが開いている長さが決まります: {}。",
                    "Where a round has an end, how long the hold stays open is known: {}.",
                    fix.ja; fix.en
                )),
                Vec::new(),
            );
        };
        let detail = |l: Option<Longest>| {
            let mut o = vec![("business_days".to_string(), Json::from(held)), ("expires_after_seconds".to_string(), Json::from(expiry))];
            if let Some(l) = l {
                o.extend([("submitted".to_string(), Json::str(&day_text(l.submitted))), ("through".to_string(), Json::str(&day_text(l.last))), ("days".to_string(), Json::from(l.days as u64))]);
            }
            o
        };
        if held == 0 {
            // the round ends when the request is submitted
            return Some((border(Came::Held, None, detail(None)), None));
        }
        if time.calendars.is_empty() {
            return undecided(tr!("営業日を数えるカレンダーが分かりません。", "The calendar the business days are counted in is not known."), None, detail(None));
        }
        // the days every calendar opens, within the data they all have
        let mut facts: Vec<Rc<CalendarFacts>> = Vec::new();
        for c in &time.calendars {
            let read = self.calendars.borrow_mut().entry(c.file.clone()).or_insert_with(|| dates.calendar(&c.file).ok().map(Rc::new)).clone();
            facts.push(read?);
        }
        let lo = facts.iter().map(|c| c.data.0).max()?;
        let hi = facts.iter().map(|c| c.data.1).min()?;
        let from = self.from.map_or(lo, |d| d.max(lo));
        let through = self.through.map_or(hi, |d| d.min(hi));
        // calendars that close by rule alone repeat after a turn: the days past one add nothing
        let through = if hi == NO_TABLE_END { through.min(from.saturating_add(TURN)) } else { through };
        let closed = |d: Day| facts.iter().any(|c| c.closed.contains(&d));
        let (mut open, mut past, mut d) = (Vec::new(), 0u64, from);
        while d <= hi && past < held {
            if !closed(d) {
                open.push(d);
                if d > through {
                    past += 1;
                }
            }
            d += 1;
        }
        let names = Text::list(&time.calendars.iter().map(|c| Text::same(format!("`{}`", c.alias))).collect::<Vec<_>>());
        let Some(l) = longest(&open, from, through, held) else {
            return undecided(
                tr!(
                    "カレンダー {} のデータでは、申請を出せるどの日からも {held} 営業日を数えられません。",
                    "The data of the calendar {} does not cover {held} business days from any day a request can be submitted on.",
                    names.ja; names.en
                ),
                Some(tr!("カレンダーの休みの表を、申請を出せる日から {held} 営業日先まで延ばしてください。", "Carry the calendar's table of closed days {held} business days past the days a request can be submitted on.")),
                detail(None),
            );
        };
        let offsets: Vec<i64> = facts.iter().map(|c| i64::from(c.offset.unwrap_or(0))).collect();
        let spread = (offsets.iter().max()? - offsets.iter().min()?) as u64 * 60;
        let how = counted(time, held);
        let stays = tr!(
            "仮押さえは申請を出したときに作り、その `round` が終わるまで開いています。ringi の検査の数え方で、長くて {held} 営業日です（{}）。",
            "The hold is made when a request is submitted and stays open until its round ends: {held} business days at the most, as ringi's check counts them ({}).",
            how.ja; how.en
        );
        let (on, last, days) = (day_text(l.submitted), day_text(l.last), l.days);
        match judge(l.days, spread, expiry, facts.len()) {
            Kept::Held => Some((border(Came::Held, None, detail(Some(l))), None)),
            Kept::Expires => {
                let d = self
                    .at("E209", h.line, h.col, tr!(
                        "`{request}` のチケットが仮押さえ `{hold}` を確定か取消にする前に、期限が切れることがあります",
                        "The hold `{hold}` can expire before a ticket of `{request}` posts or voids it"
                    ))
                    .note(stays)
                    .note(tr!(
                        "{on} に出した申請は、カレンダー {} で {held} 営業日あとの {last} まで開いていることがあり、{days} 日になります。",
                        "A request submitted on {on} can stay open through {last}, {held} business days later in the calendar {}: {days} days.",
                        names.ja; names.en
                    ))
                    .note(expires);
                // the two come to the same length: said, since the days alone read as enough
                let d = if (l.days.max(0) as u64).saturating_mul(86_400) == expiry {
                    d.note(tr!(
                        "仮押さえは、期限のちょうどその秒に切れています。{on} の 00:00 に作った仮押さえは、{last} が終わる時点で切れています。",
                        "A hold has expired at the second its expiry falls on: one made at 00:00 on {on} has expired when {last} ends."
                    ))
                } else {
                    d
                };
                let d = d
                    .note(refused)
                    .note(tr!(
                        "仮押さえの期限を延ばすか（振替の `pending expires after`）、ステップの期限を短くしてください。",
                        "Make the hold last longer (the transfer's `pending expires after`), or give the stages shorter deadlines."
                    ));
                Some((border(Came::Failed, Some("E209"), detail(Some(l))), Some(d)))
            }
            Kept::MayExpire => {
                let n = facts.len();
                let d = self
                    .at("W209", h.line, h.col, tr!(
                        "`{request}` のチケットが仮押さえ `{hold}` を確定か取消にする前に、期限が切れるかを決められません",
                        "Whether the hold `{hold}` expires before a ticket of `{request}` posts or voids it cannot be decided"
                    ))
                    .note(stays)
                    .note(tr!(
                        "営業日を {n} 個のカレンダー（{}）で数えています。どれかが休む日を休みとして数えると、{on} に出した申請は {last} まで、{days} 日になりますが、そこまでかかるチケットがあるとは限りません。",
                        "The business days are counted in {n} calendars ({}). Taking a day any of them closes as closed, a request submitted on {on} stays open through {last}, {days} days, which a ticket need not take.",
                        names.ja; names.en
                    ))
                    .note(expires)
                    .note(refused)
                    .note(tr!(
                        "仮押さえの期限をその日数より長くすると（振替の `pending expires after`）、切れないと示せます。",
                        "Make the hold last longer than those days (the transfer's `pending expires after`), and it is shown not to expire."
                    ));
                Some((border(Came::Undecided, Some("W209"), detail(Some(l))), Some(d)))
            }
        }
    }
}
