//! How a ticket ends (DESIGN 7.6, 9.5): whether it ends at all and the most business days it takes
//! (E401, W402), which endings it has (E407), and that none of them leaves a hold open (E405,
//! E406).
//!
//! Every state a ticket waits in has a clock that takes it out: a stage its deadline, the
//! requester's turn `resubmit within`, a pause the days it has left. The count adds the most each
//! can take. A stage due in `n` business days takes `n`; one that goes up when it is late takes `n`
//! again each time, and, where the deadline is at the end of a day, the day it goes up on (the
//! clock rings at the next day's 00:00, and the new deadline is counted from there). A round of the
//! ticket takes what the stages of the longest path of the route take together, a stage that comes
//! on a condition counted as coming. The ticket goes back as often as `send back … at most` lets
//! it, each time for a round more and the requester's turn, and is paused for what `pause at most`
//! gives it. The count is the most that can be: it holds whatever the calendar closes, and a
//! claim it meets holds for every ticket.
//!
//! With `same approver approves once, at the last stage`, a way of a stage can wait for a later
//! stage, and be started after all when that stage no longer comes to its one approver: the stage
//! comes again then, with its clocks counted anew. A stage with one way starts once, there or
//! later; one with several (`a and b`) can start once for each, and is counted as many times.
//!
//! What has no clock has no end: a stage with no deadline, one that stays when it is late, a
//! ticket that goes back to a requester with no `resubmit within`.

use crate::ast::{ActionKind, At, ClaimBody, Ending, Expr, File, FileKind, Late, Reasoned, Span, Spec, TicketLine};
use crate::diag::Diag;
use crate::names::Scope;
use crate::org::listing::{self, Occ};
use ritsu_base::text::Text;
use std::collections::BTreeMap;

/// The most business days a stage takes, by how it is counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Takes {
    /// Nobody is waited for: an empty cell, a circulation.
    Nothing,
    /// At its deadline the ticket expires, or goes on without the summary.
    Due(u32),
    /// Late, it goes up `times` times, each with the deadline again; `day` where the deadline is at
    /// the end of a day, and the stage goes up on the next.
    Up { days: u32, times: u32, day: bool },
    /// It waits on when it is late.
    Stays,
    /// It has no deadline.
    NoDeadline,
}

impl Takes {
    /// The business days; None where it has no end.
    pub fn days(self) -> Option<u64> {
        match self {
            Takes::Nothing => Some(0),
            Takes::Due(n) => Some(u64::from(n)),
            Takes::Up { days, times, day } => Some(u64::from(days) * (u64::from(times) + 1) + if day { u64::from(times) } else { 0 }),
            Takes::Stays | Takes::NoDeadline => None,
        }
    }

    /// Whether the ticket can expire at the stage: its last deadline passes, or it has nowhere to
    /// go up to.
    fn expires(self, o: &Occ) -> bool {
        matches!(self, Takes::Due(_) | Takes::Up { .. }) && !matches!(o.spec, Spec::Note(_))
    }
}

/// How many times the stage `o` can start in a round: once; and with `same approver approves once,
/// at the last stage`, once for each of its ways to choose (`a and b`), each of which can wait for a
/// later stage of its own and be started when the others have answered.
pub fn starts(s: &Scope, o: &Occ) -> u64 {
    match o.spec {
        Spec::Who(ws) if listing::once(s.file) == listing::Once::Last => ws.len().max(1) as u64,
        _ => 1,
    }
}

/// The most business days the stage `o` takes in a round: what it takes each time it starts.
fn stage_days(s: &Scope, o: &Occ) -> Option<u64> {
    Some(takes(o).days()?.saturating_mul(starts(s, o)))
}

/// What the stage `o` takes.
pub fn takes(o: &Occ) -> Takes {
    if matches!(o.spec, Spec::Empty | Spec::Circulate(_) | Spec::Route(_)) {
        return Takes::Nothing;
    }
    let Some(d) = o.due else { return Takes::NoDeadline };
    // a summary that does not come: the ticket goes on without it
    if matches!(o.spec, Spec::Note(_)) {
        return Takes::Due(d.days);
    }
    let day = d.at == At::EndOfDay;
    match o.late.map(|l| (l.first, l.then)) {
        None | Some((Late::Expire, _)) => Takes::Due(d.days),
        Some((Late::Stay, _)) | Some((Late::Escalate(_), Some(Late::Stay))) => Takes::Stays,
        Some((Late::Escalate(_), Some(Late::Escalate(_)))) => Takes::Up { days: d.days, times: 2, day },
        Some((Late::Escalate(_), _)) => Takes::Up { days: d.days, times: 1, day },
    }
}

/// The count of a request (or of a shared route's stages).
pub struct Count<'f> {
    pub paths: Vec<Vec<Occ<'f>>>,
    /// The path a round takes the longest on, where every stage has an end.
    pub longest: Option<usize>,
    /// The most business days of a round.
    pub round: Option<u64>,
    /// How many times the ticket goes back (`send back … at most N times`), and the line.
    pub backs: Option<(u32, usize)>,
    /// The business days the requester has (`resubmit within`), and the line.
    pub resubmit: Option<(u32, usize)>,
    /// The business days of a pause (`pause at most`), and the line.
    pub pause: Option<(u32, usize)>,
    /// The most business days of a ticket; None where it may never end.
    pub total: Option<u64>,
    /// The most business days a hold made at `submit` stays open (DESIGN 9.3, X19): one round and
    /// the pause; and where `send back` can send a ticket to an earlier stage, which starts no new
    /// round and leaves the hold as it is, each time it walks the round again. None where a stage
    /// has no end.
    pub held: Option<u64>,
}

impl<'f> Count<'f> {
    pub fn of(s: &'f Scope) -> Count<'f> {
        let paths = listing::paths(s);
        let sums: Vec<Option<u64>> = paths.iter().map(|p| p.iter().try_fold(0u64, |a, o| Some(a.saturating_add(stage_days(s, o)?)))).collect();
        let round = sums.iter().copied().collect::<Option<Vec<u64>>>().map(|v| v.into_iter().max().unwrap_or(0));
        // the first of the longest
        let longest = round.and_then(|r| sums.iter().position(|x| *x == Some(r)));
        let line = |pick: &dyn Fn(&TicketLine) -> Option<(u32, usize)>| s.file.ticket.as_ref().and_then(|t| t.lines.iter().find_map(pick));
        let backs = line(&|l| if let TicketLine::SendBack { times, line, .. } = l { Some((*times, *line)) } else { None }).filter(|(n, _)| *n > 0);
        let resubmit = line(&|l| if let TicketLine::Resubmit { days, line, .. } = l { Some((*days, *line)) } else { None });
        let pause = line(&|l| if let TicketLine::Pause { days, line, .. } = l { Some((*days, *line)) } else { None });
        let k = backs.map_or(0, |(n, _)| u64::from(n));
        let turn = match (k, resubmit) {
            (0, _) => Some(0),
            (_, Some((r, _))) => Some(k.saturating_mul(u64::from(r))),
            (_, None) => None,
        };
        let total = match (round, turn) {
            (Some(r), Some(t)) => Some((k + 1).saturating_mul(r).saturating_add(t).saturating_add(pause.map_or(0, |(p, _)| u64::from(p)))),
            _ => None,
        };
        let to_a_stage = line(&|l| if let TicketLine::SendBack { to_requester: false, times, line } = l { Some((*times, *line)) } else { None }).is_some_and(|(n, _)| n > 0);
        let held = round.map(|r| (if to_a_stage { k + 1 } else { 1 }).saturating_mul(r).saturating_add(pause.map_or(0, |(p, _)| u64::from(p))));
        Count { paths, longest, round, backs, resubmit, pause, total, held }
    }

    /// Whether the ticket can be with its requester: it goes back at least once.
    fn goes_back(&self) -> bool {
        self.backs.is_some()
    }

    /// The stages that have no end, each once, by the reason: the ones that stay when they are
    /// late, by their `when late` line, and the ones with no deadline.
    fn endless(&self) -> (Vec<(&Occ<'f>, Vec<String>)>, Vec<&Occ<'f>>) {
        let mut stays: Vec<(&Occ<'f>, Vec<String>)> = Vec::new();
        let mut bare: Vec<&Occ<'f>> = Vec::new();
        for o in self.paths.iter().flatten() {
            match takes(o) {
                Takes::Stays => {
                    let l = o.late.expect("a stage that stays has a `when late` line");
                    match stays.iter_mut().find(|(x, _)| x.late.is_some_and(|y| std::ptr::eq(y.file, l.file) && y.line == l.line)) {
                        Some((_, names)) => {
                            if !names.contains(&o.stage) {
                                names.push(o.stage.clone());
                            }
                        }
                        None => stays.push((o, vec![o.stage.clone()])),
                    }
                }
                Takes::NoDeadline => {
                    if !bare.iter().any(|x| x.stage == o.stage) {
                        bare.push(o);
                    }
                }
                _ => {}
            }
        }
        (stays, bare)
    }

    /// The line under `ok` that says how long a ticket takes (DESIGN 7.6); None where it has no
    /// end, which W402 says.
    pub fn fact(&self) -> Option<Text> {
        let total = self.total?;
        let (round, k) = (self.round?, self.backs.map_or(0, |(n, _)| u64::from(n)));
        let mut ja = Vec::new();
        let mut en = Vec::new();
        if k > 0 {
            let r = self.resubmit.map_or(0, |(r, _)| r);
            ja.push(format!("ステップに {} × {round}、申請者のもとで {k} × {r}", k + 1));
            en.push(format!("{} × {round} at its stages, {k} × {r} with the requester", k + 1));
        }
        if let Some((p, _)) = self.pause {
            if k == 0 {
                ja.push(format!("ステップに {round}"));
                en.push(format!("{round} at its stages"));
            }
            ja.push(format!("保留に {p}"));
            en.push(format!("{p} paused"));
        }
        Some(if en.is_empty() {
            tr!("長くても {total} 営業日で終わります", "ends within {total} business days at the most")
        } else {
            tr!("長くても {total} 営業日で終わります（{}）", "ends within {total} business days at the most ({})", ja.join("、"); en.join(", "))
        })
    }
}

fn names(v: &[String]) -> Text {
    Text::list(&v.iter().map(|n| Text::same(format!("`{n}`"))).collect::<Vec<_>>())
}

/// The first word of a line, as a place to point at.
fn line_span(src: &str, line: usize) -> Span {
    let text = src.lines().nth(line.saturating_sub(1)).unwrap_or("");
    let col = text.chars().take_while(|c| c.is_whitespace()).count() + 1;
    Span { line, col, end: text.trim_end().chars().count() + 1 }
}

/// A `when late` line as it is written.
fn late_written(l: &listing::LateLine) -> String {
    match l.then {
        Some(t) => format!("when late {}, then {}", l.first.written(), t.written()),
        None => format!("when late {}", l.first.written()),
    }
}

/// A `due in` line as it is written.
fn due_written(d: &listing::DueLine) -> String {
    let unit = if d.days == 1 { "business day" } else { "business days" };
    let cal = d.calendar.map(|c| format!(" in {}", c.text)).unwrap_or_default();
    match d.at {
        At::EndOfDay => format!("due in {} {unit}{cal} at end of day", d.days),
        At::Time(t) => format!("due in {} {unit}{cal} at {}", d.days, crate::lex::time_text(t)),
    }
}

/// Where a line of a stage's deadlines is, when it is in the shared route the request uses.
fn elsewhere(s: &Scope, f: &File, line: usize) -> Option<Text> {
    (!std::ptr::eq(f, s.file)).then(|| tr!("共有の経路の {line} 行目に書いてあります。", "It is written on line {line} of the shared route."))
}

/// A wait that has no end: where it is said, what it is, where its line is when that is in another
/// file, whether it is a `stay` (which W402 says where no claim is made), what gives it an end, and
/// whether it is a wait of a round (false for the requester's turn between two).
struct NoEnd {
    at: Span,
    text: Text,
    more: Option<Text>,
    stay: bool,
    fix: Text,
    in_round: bool,
}

/// What has no end, each as a sentence: the stages that stay, the stages with no deadline, the
/// requester's turn.
fn no_ends<'c, 'f>(s: &Scope, c: &'c Count<'f>) -> Vec<NoEnd> {
    let mut out = Vec::new();
    let (stays, bare) = c.endless();
    for (o, stages) in &stays {
        let l = o.late.expect("a stage that stays has a `when late` line");
        let (w, ns) = (late_written(&l), names(stages));
        let own = std::ptr::eq(l.file, s.file);
        let at = if own { line_span(s.src, l.line) } else { Span { line: o.line, col: o.col, end: o.col + 1 } };
        let text = if stages.len() == 1 {
            tr!("ステップ {} は、期限を過ぎても待ち続けます（`{w}`）", "the stage {} waits on when it is late (`{w}`)", ns.ja; ns.en)
        } else {
            tr!("ステップ {} は、期限を過ぎても待ち続けます（`{w}`）", "the stages {} wait on when they are late (`{w}`)", ns.ja; ns.en)
        };
        out.push(NoEnd { at, text, more: elsewhere(s, l.file, l.line), stay: true, fix: tr!("`stay` を `expire` か `escalate` にしてください", "write `expire` or `escalate` in place of `stay`"), in_round: true });
    }
    if let Some(first) = bare.first() {
        let ns = names(&bare.iter().map(|o| o.stage.clone()).collect::<Vec<_>>());
        let text = if bare.len() == 1 {
            tr!("ステップ {} には期限がありません（`due in …`）。答えが来るまで待ちます", "the stage {} has no deadline (`due in …`), and waits until it is answered", ns.ja; ns.en)
        } else {
            tr!("ステップ {} には期限がありません（`due in …`）。答えが来るまで待ちます", "the stages {} have no deadline (`due in …`), and wait until they are answered", ns.ja; ns.en)
        };
        out.push(NoEnd { at: Span { line: first.line, col: first.col, end: first.col + 1 }, text, more: None, stay: false, fix: tr!("期限（`due in …`）を書いてください", "write a deadline (`due in …`)"), in_round: true });
    }
    if let (Some((_, line)), None) = (c.backs, c.resubmit)
        && s.file.kind == FileKind::Request
    {
        out.push(NoEnd {
            at: line_span(s.src, line),
            text: tr!("戻ったチケットは、申請者が出し直すまで待ち続けます（`resubmit within …` の行がありません）", "a ticket that goes back waits for its requester with no end (there is no `resubmit within …` line)"),
            more: None,
            stay: false,
            fix: tr!("`resubmit within …` を書いてください", "write `resubmit within …`"),
            in_round: false,
        });
    }
    out
}

/// How the stages of the longest round are counted, as notes: the round, each kind of stage, and
/// the ticket's own lines.
fn counted(s: &Scope, c: &Count) -> Vec<Text> {
    let mut notes = Vec::new();
    let (Some(i), Some(round), Some(total)) = (c.longest, c.round, c.total) else { return notes };
    let path: Vec<(&Occ, Takes)> = c.paths[i].iter().map(|o| (o, takes(o))).filter(|(_, t)| *t != Takes::Nothing).collect();
    let each = Text::list(
        &path
            .iter()
            .map(|(o, t)| {
                Text::same(match starts(s, o) {
                    1 => format!("`{}` {}", o.stage, t.days().unwrap_or(0)),
                    n => format!("`{}` {n} × {}", o.stage, t.days().unwrap_or(0)),
                })
            })
            .collect::<Vec<_>>(),
    );
    // a row of the route's table, where the route is one
    let row = match &s.file.route {
        Some(crate::ast::Route::Table(_)) => c.paths[i].first().map(|o| o.line),
        _ => None,
    };
    let of_row = match row {
        Some(l) => tr!("（経路の {l} 行目）", " (the row of the route on line {l})"),
        None => Text::default(),
    };
    let cond = if c.paths[i].iter().any(|o| o.cond) { tr!("条件で来るステップは、来るものとして数えます。", " A stage that comes on a condition is counted as coming.") } else { Text::default() };
    notes.push(tr!(
        "一つの `round` は、長くて {round} 営業日です: {}{}。{}",
        "A round takes {round} business days at the most: {}{}.{}",
        each.ja, of_row.ja, cond.ja;
        each.en, of_row.en, cond.en
    ));
    // each way a stage is counted, once, with the stages counted so
    let mut kinds: Vec<(Takes, String, Option<String>, bool, Vec<String>)> = Vec::new();
    for (o, t) in &path {
        let due = o.due.map(|d| due_written(&d)).unwrap_or_default();
        let late = o.late.map(|l| late_written(&l));
        let note = matches!(o.spec, Spec::Note(_));
        match kinds.iter_mut().find(|k| k.0 == *t && k.1 == due && k.2 == late && k.3 == note) {
            Some(k) => {
                if !k.4.contains(&o.stage) {
                    k.4.push(o.stage.clone());
                }
            }
            None => kinds.push((*t, due, late, note, vec![o.stage.clone()])),
        }
    }
    for (t, due, late, note, stages) in &kinds {
        let ns = names(stages);
        let n = t.days().unwrap_or(0);
        let late = late.clone().unwrap_or_default();
        notes.push(match *t {
            Takes::Due(_) if *note => tr!("{}: `{due}`。要約が来なければ、そこで先へ進みます。{n} 営業日。", "{}: `{due}`, and the ticket goes on without the summary then: {n}.", ns.ja; ns.en),
            Takes::Due(_) if late.is_empty() => tr!("{}: `{due}`。過ぎれば期限切れで終わります。{n} 営業日。", "{}: `{due}`, and the ticket expires then: {n}.", ns.ja; ns.en),
            Takes::Due(_) => tr!("{}: `{due}`。過ぎれば期限切れで終わります（`{late}`）。{n} 営業日。", "{}: `{due}`, and the ticket expires then (`{late}`): {n}.", ns.ja; ns.en),
            Takes::Up { days, times, day: true } => {
                let sum = std::iter::once(days.to_string()).chain((0..times).flat_map(|_| ["1".to_string(), days.to_string()])).collect::<Vec<_>>().join(" + ");
                if times == 1 {
                    tr!(
                        "{}: `{due}`。過ぎると次の日に上へ回り、そこから同じ日数の期限が付きます（`{late}`）。{sum} = {n} 営業日。",
                        "{}: `{due}`; late, it goes up on the next day, and is due again counted from there (`{late}`): {sum} = {n}.",
                        ns.ja; ns.en
                    )
                } else {
                    tr!(
                        "{}: `{due}`。過ぎると次の日に上へ回り、そこから同じ日数の期限が付きます。これが {times} 度あります（`{late}`）。{sum} = {n} 営業日。",
                        "{}: `{due}`; late, it goes up on the next day, and is due again counted from there, {times} times (`{late}`): {sum} = {n}.",
                        ns.ja; ns.en
                    )
                }
            }
            Takes::Up { days, times, day: false } => {
                let sum = std::iter::repeat_n(days.to_string(), times as usize + 1).collect::<Vec<_>>().join(" + ");
                if times == 1 {
                    tr!(
                        "{}: `{due}`。過ぎると上へ回り、そこから同じ日数の期限が付きます（`{late}`）。{sum} = {n} 営業日。",
                        "{}: `{due}`; late, it goes up, and is due again counted from there (`{late}`): {sum} = {n}.",
                        ns.ja; ns.en
                    )
                } else {
                    tr!(
                        "{}: `{due}`。過ぎると上へ回り、そこから同じ日数の期限が付きます。これが {times} 度あります（`{late}`）。{sum} = {n} 営業日。",
                        "{}: `{due}`; late, it goes up, and is due again counted from there, {times} times (`{late}`): {sum} = {n}.",
                        ns.ja; ns.en
                    )
                }
            }
            Takes::Nothing | Takes::Stays | Takes::NoDeadline => continue,
        });
    }
    // the stages counted once for each of their ways
    let mut several: Vec<String> = Vec::new();
    for (o, _) in path.iter().filter(|(o, _)| starts(s, o) > 1) {
        if !several.contains(&o.stage) {
            several.push(o.stage.clone());
        }
    }
    if !several.is_empty() {
        let ns = names(&several);
        notes.push(tr!(
            "{} は、決め方の数だけ数えます。`same approver approves once, at the last stage` では、決め方の一つが後のステップを待ち、ほかの決め方の答えがそろったあとで始まることがあります。",
            "{}: counted once for each of its ways to choose. With `same approver approves once, at the last stage`, a way can wait for a later stage, and be started after the others have answered.",
            ns.ja; ns.en
        ));
    }
    let k = c.backs.map_or(0, |(n, _)| u64::from(n));
    let mut ja = Vec::new();
    let mut en = Vec::new();
    let mut sum = if k > 0 { format!("{} × {round}", k + 1) } else { round.to_string() };
    if k > 0 {
        let r = c.resubmit.map_or(0, |(r, _)| r);
        ja.push(format!("チケットは {k} 回まで戻るので（`send back … at most {k} times`）、`round` は {} 回まであり、戻るたびに、申請者の出し直しを {r} 営業日まで待ちます（`resubmit within {r} business days`）", k + 1));
        en.push(format!("The ticket goes back {k} times at the most (`send back … at most {k} times`), for {} rounds, and its requester has {r} business days each time (`resubmit within {r} business days`)", k + 1));
        sum.push_str(&format!(" + {k} × {r}"));
    }
    if let Some((p, _)) = c.pause {
        ja.push(format!("保留は合わせて {p} 営業日までです（`pause at most {p} business days`）"));
        en.push(format!("{} is paused for {p} business days at the most (`pause at most {p} business days`)", if en.is_empty() { "The ticket" } else { "it" }));
        sum.push_str(&format!(" + {p}"));
    }
    if !en.is_empty() {
        notes.push(tr!("{}。{sum} = {total} 営業日。", "{}: {sum} = {total}.", ja.join("。"); en.join("; ")));
    }
    notes
}

/// E401 and W402 (DESIGN 7.6): the claim `every ticket ends within N business days` against the
/// count, and, with no such claim, each stage that stays when it is late. What it answers is the
/// line under `ok`: within how long a ticket ends, or what it may wait for with no end (a stage
/// with no deadline and a requester with no `resubmit within` are no mistakes, and are said there).
pub fn check(s: &Scope, out: &mut Vec<Diag>) -> Option<Text> {
    let c = Count::of(s);
    let claim = s.file.claims.iter().find_map(|cl| if let ClaimBody::EndsWithin { days } = cl.body { Some((cl, days)) } else { None });
    let none = no_ends(s, &c);
    let Some((cl, days)) = claim else {
        for n in none.iter().filter(|n| n.stay) {
            let mut d = s.warn("W402", n.at, tr!("{}。チケットが終わらないことがあります", "{}: a ticket may never end", n.text.ja; ritsu_base::text::capitalize(&n.text.en)));
            if let Some(m) = &n.more {
                d = d.note(m.clone());
            }
            out.push(d.note(tr!("待ってよいなら、そのままでかまいません。終わらせるなら、`stay` を `expire` か `escalate` にしてください。", "If it may wait, leave it; for it to end, write `expire` or `escalate` in place of `stay`.")));
        }
        if none.is_empty() {
            return c.fact();
        }
        let all = Text::join(&none.iter().map(|n| n.text.clone()).collect::<Vec<_>>(), "。", "; ");
        return Some(tr!("終わらないことがあります: {}", "may never end: {}", all.ja; all.en));
    };
    let name = &cl.name.text;
    if !none.is_empty() {
        let mut d = s.err("E401", cl.name.span, tr!("主張 `{name}` は、どのチケットも {days} 営業日のうちに終わると言いますが、終わらないチケットがあります", "The claim `{name}` says every ticket ends within {days} business days, and a ticket may never end"));
        let mut fixes: Vec<Text> = Vec::new();
        for n in none {
            let l = n.at.line;
            d = d.note(tr!("{l} 行目: {}。", "Line {l}: {}.", n.text.ja; n.text.en));
            if let Some(m) = n.more {
                d = d.note(m);
            }
            if !fixes.contains(&n.fix) {
                fixes.push(n.fix);
            }
        }
        let fix = Text::join(&fixes, "。", ", and ");
        out.push(d.note(tr!("終わらせるなら、{}。待ってよいなら、主張を消してください。", "For it to end, {}; if it may wait, take the claim out.", fix.ja; fix.en)));
        return None;
    }
    let total = c.total.unwrap_or(0);
    if total > u64::from(days) {
        let mut d = s.err("E401", cl.name.span, tr!("主張 `{name}` は、どのチケットも {days} 営業日のうちに終わると言いますが、長いと {total} 営業日かかります", "The claim `{name}` says every ticket ends within {days} business days, and a ticket can take {total}"));
        for n in counted(s, &c) {
            d = d.note(n);
        }
        out.push(d.note(tr!("主張を {total} 営業日にするか、期限、戻せる回数、保留の日数を減らしてください。", "Write {total} business days in the claim, or shorten the deadlines, the times it goes back or the pause.")));
        return None;
    }
    c.fact()
}

/// The endings a ticket of the request has (DESIGN 7.3), besides `submitted` and `approved`, which
/// every ticket has.
struct Endings {
    /// A person answers a stage, and can reject.
    rejected: bool,
    /// A `send back` line lets it go back.
    sent_back: bool,
    /// The requester withdraws it for good: with a `withdraw` line, when it cannot go back any
    /// more.
    withdrawn_open: bool,
    /// The requester discards it while it is with them, or withdraws it for good.
    withdrawn: bool,
    /// A stage's deadline expires it.
    expired_open: bool,
    /// A stage's deadline, or `resubmit within`, expires it.
    expired: bool,
}

impl Endings {
    fn of(s: &Scope, c: &Count) -> Endings {
        let all = || c.paths.iter().flatten();
        let withdraw = s.file.ticket.as_ref().is_some_and(|t| t.lines.iter().any(|l| matches!(l, TicketLine::Withdraw { .. })));
        let expired_open = all().any(|o| takes(o).expires(o));
        Endings {
            rejected: all().any(|o| o.spec.approves()),
            sent_back: c.goes_back(),
            withdrawn_open: withdraw,
            withdrawn: withdraw || c.goes_back(),
            expired_open,
            expired: expired_open || (c.goes_back() && c.resubmit.is_some()),
        }
    }

    fn has(&self, e: Ending) -> bool {
        match e {
            Ending::Submitted | Ending::Approved => true,
            Ending::Rejected => self.rejected,
            Ending::SentBack => self.sent_back,
            Ending::Withdrawn => self.withdrawn,
            Ending::Expired => self.expired,
        }
    }

    /// The endings a hold of the round is open at.
    fn open_at(&self) -> Vec<Ending> {
        let mut v = vec![Ending::Approved];
        if self.sent_back {
            v.push(Ending::SentBack);
        }
        if self.rejected {
            v.push(Ending::Rejected);
        }
        if self.withdrawn_open {
            v.push(Ending::Withdrawn);
        }
        if self.expired_open {
            v.push(Ending::Expired);
        }
        v
    }
}

/// What the port says of how a ticket ends (`Requests::ticket`, DESIGN 9.4), as the check counts
/// it: the count, the endings a ticket has besides `submitted` (in the order `approved`, `sent
/// back`, `rejected`, `withdrawn`, `expired`), and what has no end, each as W402 and the line under
/// `ok` say it, with what gives it an end.
pub fn summary<'f>(s: &'f Scope) -> (Count<'f>, Vec<Ending>, Vec<ritsu_ports::TicketNoEnd>) {
    let c = Count::of(s);
    let e = Endings::of(s, &c);
    let endings = [Ending::Approved, Ending::SentBack, Ending::Rejected, Ending::Withdrawn, Ending::Expired].into_iter().filter(|x| e.has(*x)).collect();
    let none = no_ends(s, &c).into_iter().map(|n| ritsu_ports::TicketNoEnd { text: n.text, fix: n.fix, in_round: n.in_round }).collect();
    (c, endings, none)
}

/// The name an expression of an `on` line reads a field of the form through.
enum Via {
    Itself,
    /// A computed field, which is computed from it.
    Computed(String),
    /// An output of a rule (`authority.limit`), which takes it.
    Rule(String),
}

/// The fields of the form an expression of an `on` line reads, each with the name it is read
/// through: the field itself, a computed field it is computed from (however far), an output of a
/// rule that takes it. What the machine reads when it evaluates the expression
/// ([`crate::engine`]'s `on`), and no more: the requester and the ticket are no fields.
fn form_reads(s: &Scope, e: &Expr) -> BTreeMap<String, Via> {
    let mut read: BTreeMap<String, Via> = BTreeMap::new();
    for p in e.paths() {
        let first = p[0].text.as_str();
        if let Some(f) = s.field(first) {
            read.entry(f.name.text.clone()).or_insert(Via::Itself);
        } else if let (Some(out), Some(facts)) = (p.get(1), s.rule_facts(first)) {
            let written = format!("{first}.{}", out.text);
            for f in s.fields() {
                let a = f.name.ascii();
                if facts.inputs.iter().any(|c| c.name == a || c.alias == a) || facts.elements.iter().any(|(n, al, _)| n == a || al == a) {
                    read.entry(f.name.text.clone()).or_insert_with(|| Via::Rule(written.clone()));
                }
            }
        }
    }
    loop {
        let mut more: Vec<(String, Via)> = Vec::new();
        for x in s.fields() {
            let (Some(via), Some(c)) = (read.get(&x.name.text), &x.computed) else { continue };
            for p in c.paths() {
                let Some(f) = s.field(&p[0].text) else { continue };
                if read.contains_key(&f.name.text) || more.iter().any(|(n, _)| *n == f.name.text) {
                    continue;
                }
                more.push((f.name.text.clone(), match via {
                    Via::Itself => Via::Computed(x.name.text.clone()),
                    Via::Computed(v) => Via::Computed(v.clone()),
                    Via::Rule(r) => Via::Rule(r.clone()),
                }));
            }
        }
        if more.is_empty() {
            return read;
        }
        read.extend(more);
    }
}

/// E405, E406, E407 and E409 (DESIGN 9.5): a hold some ending leaves open, a `post` or a `void` of
/// what is not held, an `on` line for an ending the ticket never has, and a reason asked with an
/// operation the ticket never has.
pub fn holds(s: &Scope, out: &mut Vec<Diag>) {
    let Some(t) = &s.file.ticket else { return };
    let c = Count::of(s);
    let ends = Endings::of(s, &c);
    struct On<'x> {
        endings: &'x [(Ending, Span)],
        action: ActionKind,
        call: &'x crate::ast::Call,
        when: Option<String>,
        cond: Option<&'x Expr>,
        line: usize,
    }
    let ons: Vec<On> = t
        .lines
        .iter()
        .filter_map(|l| match l {
            TicketLine::On { endings, action, call, when, line } => {
                Some(On { endings, action: *action, call, when: when.as_ref().map(|w| crate::types::written(s.src, w.span).split_whitespace().collect::<Vec<_>>().join(" ")), cond: when.as_ref(), line: *line })
            }
            _ => None,
        })
        .collect();
    let target = |c: &crate::ast::Call| c.target.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(".");
    // E407: an ending the ticket never has
    for o in &ons {
        for (e, span) in o.endings.iter().filter(|(e, _)| !ends.has(*e)) {
            let why = match e {
                Ending::Rejected => tr!("経路に、人が答えるステップがありません", "no stage of the route is answered by a person"),
                Ending::SentBack => tr!("`send back` の行がありません", "there is no `send back` line"),
                Ending::Withdrawn => tr!("`withdraw` の行も `send back` の行もありません", "there is no `withdraw` line and no `send back` line"),
                _ => tr!("期限で終わるステップも、`resubmit within` で終わる申請者の番もありません", "no stage expires at a deadline, and no turn of the requester at `resubmit within`"),
            };
            let w = e.word();
            out.push(
                s.err("E407", *span, tr!("このチケットが `{w}` になることはありません。{}", "The ticket is never `{w}`: {}", why.ja; why.en))
                    .note(tr!("その行を消すか、その終わり方を許す行を書いてください。", "Take the line out, or write the line that allows that ending.")),
            );
        }
    }
    // E409: a reason asked for an operation the ticket never has, as E407 tells its ending
    for l in &t.lines {
        let TicketLine::CommentRequired { ops, .. } = l else { continue };
        for (op, span) in ops {
            let (ending, why) = match op {
                Reasoned::Reject => (Ending::Rejected, tr!("経路に、人が答えるステップがありません", "no stage of the route is answered by a person")),
                Reasoned::SendBack => (Ending::SentBack, tr!("`send back` の行がありません", "there is no `send back` line")),
            };
            if ends.has(ending) {
                continue;
            }
            let (w, line) = (op.word(), crate::kw::COMMENT_REQUIRED);
            out.push(
                s.err("E409", *span, tr!("`{line}` が理由を求める `{w}` は、このチケットにはありません。{}", "`{line}` asks a reason with `{w}`, which this ticket never has: {}", why.ja; why.en))
                    .note(tr!("その操作を行から消すか、その操作を許す行を書いてください。", "Take the operation out of the line, or write the line that allows it.")),
            );
        }
    }
    let held = |o: &On| o.action == ActionKind::Hold && o.endings.iter().any(|(e, _)| *e == Ending::Submitted);
    // E406: a post or a void of what is not held
    for o in ons.iter().filter(|o| matches!(o.action, ActionKind::Post | ActionKind::Void)) {
        let (what, tg) = (o.action.word(), target(o.call));
        if !ons.iter().any(|h| held(h) && target(h.call) == tg) {
            out.push(
                s.err("E406", o.call.target[0].span, tr!("`{what} {tg}` より前に、仮押さえがありません（`on submitted hold {tg}(…)` の行がありません）", "`{what} {tg}` has no hold before it: there is no `on submitted hold {tg}(…)` line"))
                    .note(tr!("`on submitted hold {tg}(…)` を足すか、この行を消してください。", "Add `on submitted hold {tg}(…)`, or take this line out.")),
            );
        }
    }
    // E405: a hold some ending leaves open
    for h in ons.iter().filter(|o| o.action == ActionKind::Hold) {
        let tg = target(h.call);
        let at = h.call.target[0].span;
        // a hold made where the ticket ends, or goes back: nothing closes it
        let after: Vec<Text> = h.endings.iter().filter(|(e, _)| *e != Ending::Submitted).map(|(e, _)| Text::same(format!("`{}`", e.word()))).collect();
        if !after.is_empty() {
            let l = Text::list(&after);
            out.push(
                s.err("E405", at, tr!("仮押さえ `{tg}` を {} で押さえても、確定も取消もされません", "The hold `{tg}` made on {} is never posted or voided", l.ja; l.en))
                    .note(tr!("仮押さえは `on submitted` で押さえ、終わり方ごとに `post` か `void` を書きます。", "A hold is made `on submitted`, and each ending posts it or voids it.")),
            );
        }
        if !held(h) {
            continue;
        }
        let mut left: Vec<Ending> = Vec::new();
        let mut notes: Vec<Text> = Vec::new();
        for e in ends.open_at() {
            let closing: Vec<&On> = ons.iter().filter(|o| matches!(o.action, ActionKind::Post | ActionKind::Void) && target(o.call) == tg && o.endings.iter().any(|(x, _)| *x == e)).collect();
            // a line closes it for sure when it has no condition, or the hold's own
            if closing.iter().any(|o| o.when.is_none() || o.when == h.when) {
                continue;
            }
            left.push(e);
            for o in closing {
                let (what, l, w, ew) = (o.action.word(), o.line, o.when.clone().unwrap_or_default(), e.word());
                notes.push(match &h.when {
                    Some(hw) => tr!("{l} 行目の `{what}` は `{ew}` のとき `when {w}` でだけ行い、仮押さえは `when {hw}` で押さえます。", "The `{what}` on line {l} is done on `{ew}` only `when {w}`, and the hold is made `when {hw}`."),
                    None => tr!("{l} 行目の `{what}` は `{ew}` のとき `when {w}` でだけ行い、仮押さえは条件なしで押さえます。", "The `{what}` on line {l} is done on `{ew}` only `when {w}`, and the hold is made whatever the form holds."),
                });
            }
        }
        if left.is_empty() {
            continue;
        }
        let l = Text::join(&left.iter().map(|e| Text::same(format!("`{}`", e.word()))).collect::<Vec<_>>(), "、", " or ");
        let words = left.iter().map(|e| e.word()).collect::<Vec<_>>().join(", ");
        let mut d = s.err("E405", at, tr!("仮押さえ `{tg}` が、チケットが {} になるとき、確定も取消もされずに残ります", "The hold `{tg}` is left neither posted nor voided when the ticket is {}", l.ja; l.en));
        for n in notes {
            d = d.note(n);
        }
        out.push(d.note(tr!("`on {words}  void {tg}(…)` のように、その終わり方に `post` か `void` の行を足してください。", "Add a `post` or a `void` line for it, as in `on {words}  void {tg}(…)`.")));
    }
    // E405: a hold that reads what an approver may edit. It is made once, with the values the
    // request is submitted with, and an edit does not make it again: the amount held is no longer
    // the amount the request says, and a condition that no longer holds leaves the hold open
    let editable: Vec<(&crate::ast::Word, usize)> = t
        .lines
        .iter()
        .filter_map(|l| match l {
            TicketLine::ApproversEdit { fields, line } => Some(fields.iter().map(move |w| (w, *line))),
            _ => None,
        })
        .flatten()
        .collect();
    for h in ons.iter().filter(|o| held(o)) {
        let tg = target(h.call);
        // where the hold reads the form: its condition, then each argument
        let mut places: Vec<(bool, Text, &Expr)> = Vec::new();
        if let (Some(c), Some(w)) = (h.cond, &h.when) {
            places.push((true, tr!("仮押さえの条件（`when {w}`）", "The condition of the hold (`when {w}`)"), c));
        }
        for a in &h.call.args {
            let n = &a.name.text;
            places.push((false, tr!("仮押さえの引数 `{n}` ", "The argument `{n}` of the hold"), &a.value));
        }
        for (w, line) in &editable {
            let Some(f) = s.field(&w.text) else { continue };
            let n = &f.name.text;
            let hits: Vec<(bool, &Text, &Expr, Via)> = places.iter().filter_map(|(cond, what, e)| form_reads(s, e).remove(n).map(|via| (*cond, what, *e, via))).collect();
            let Some((_, _, first, _)) = hits.first() else { continue };
            let mut d = s.err("E405", first.span, tr!("仮押さえ `{tg}` が、承認者の直せるフィールド `{n}` を読みます", "The hold `{tg}` reads `{n}`, a field approvers may edit"));
            for (_, what, _, via) in &hits {
                d = d.note(match via {
                    Via::Itself => tr!("{}が `{n}` を読み、{line} 行目の `approvers edit` が、承認者に `{n}` を直させます。", "{} reads `{n}`, and `approvers edit` on line {line} lets an approver edit `{n}`.", what.ja; what.en),
                    Via::Computed(v) => tr!(
                        "{}が `{v}` を読み、`{v}` は `{n}` から計算します。{line} 行目の `approvers edit` が、承認者に `{n}` を直させます。",
                        "{} reads `{v}`, which is computed from `{n}`, and `approvers edit` on line {line} lets an approver edit `{n}`.",
                        what.ja;
                        what.en
                    ),
                    Via::Rule(r) => tr!(
                        "{}が `{r}` を読み、規則はそれを `{n}` から決めます。{line} 行目の `approvers edit` が、承認者に `{n}` を直させます。",
                        "{} reads `{r}`, which the rule decides from `{n}`, and `approvers edit` on line {line} lets an approver edit `{n}`.",
                        what.ja;
                        what.en
                    ),
                });
            }
            d = d.note(tr!("仮押さえは、申請を出したときの値で一度だけ行い、承認者が直しても押さえ直しません。", "A hold is made once, with the values the request is submitted with, and is not made again when an approver edits them."));
            if hits.iter().any(|(cond, ..)| !*cond) {
                d = d.note(tr!("確定されるのは押さえたときの値で、直したあとの申請の値と食い違います。", "What is posted is what was held, which is no longer what the request says."));
            }
            if hits.iter().any(|(cond, ..)| *cond) {
                d = d.note(tr!("直して条件が成り立たなくなると、同じ条件の `post` と `void` は行われず、仮押さえが開いたまま終わります。", "Once an edit makes the condition fail, a `post` or a `void` on the same condition is not done, and the hold is left open."));
            }
            out.push(d.note(tr!("`approvers edit` から `{n}` を外してください（ほかのフィールドが無ければ、行ごと）。", "Take `{n}` out of `approvers edit` (the line itself, where it lists no other field).")));
        }
    }
}

/// E408: a hold whose book keys it by nothing that tells one try from another (DESIGN 12.4). A
/// book remembers what it answered for each key, a refusal too, and a key it voided holds nothing
/// again: keyed by the request and its round, a try the book refused, or a hold voided because the
/// ticket that made it was not saved, stops every later try of the round. The key takes the try
/// (`ticket.attempt`, which the server gives a `submit` and a `resubmit`) where one of its
/// parameters is given `ticket.attempt`. Said where chobo reads the book (`books`).
pub fn keyed(s: &Scope, books: &[(String, ritsu_ports::BookFacts)], out: &mut Vec<Diag>) {
    let Some(t) = &s.file.ticket else { return };
    for l in &t.lines {
        let TicketLine::On { action: ActionKind::Hold, call, .. } = l else { continue };
        let [alias, transfer] = call.target.as_slice() else { continue };
        let Some((_, facts)) = books.iter().find(|(a, _)| *a == alias.text) else { continue };
        let Some(tr) = facts.transfers.iter().find(|x| x.name == transfer.text) else { continue };
        let attempt = |e: &Expr| matches!(&e.kind, crate::ast::ExprKind::Path(p) if p.len() == 2 && p[0].text == "ticket" && p[1].text == "attempt");
        if tr.key.iter().any(|k| call.args.iter().any(|a| a.name.text == *k && attempt(&a.value))) {
            continue;
        }
        let (tg, key) = (format!("{}.{}", alias.text, transfer.text), tr.key.iter().map(|k| format!("`{k}`")).collect::<Vec<_>>());
        let (key_en, key_ja) = (key.join(", "), key.join("、"));
        out.push(
            s.err(
                "E408",
                alias.span,
                tr!(
                    "`{tg}` のキー（{key_ja}）が `ticket.attempt` を受け取りません。帳簿が断った試みと、チケットを保存できずに取り消した仮押さえが、その回のあとの試みを止めます",
                    "The key of `{tg}` ({key_en}) does not take `ticket.attempt`: a try the book refused, or a hold voided because its ticket was not saved, stops every later try of the round"
                ),
            )
            .note(tr!(
                "振替のキーに試みの引数を足し（`attempt: string` と `key …, attempt`）、`hold`、`post`、`void` に `attempt: ticket.attempt` を渡してください。",
                "Give the transfer a parameter for the try in its key (`attempt: string`, `key …, attempt`), and pass `attempt: ticket.attempt` to the `hold`, the `post` and the `void`."
            )),
        );
    }
}
