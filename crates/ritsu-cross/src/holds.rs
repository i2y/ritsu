//! A hold's expiry against how long a workflow holds it (DESIGN 7.7, X5). dandori follows a hold of
//! a book as a case (`case … follows <book>.<transfer>`) and counts the expiry as an event that can
//! come at any time before a `post` or a `void` (its E022 asks the flow to handle `expired`). Here
//! the lengths are compared: dandori says how long after the hold each call on it can come, the
//! fewest and the most seconds, along every way the flow reaches the call (`HoldSpan`, with
//! koyomi's days for a wait until a date's time); chobo says when the hold expires (`pending
//! expires after`). Each call is a border ([`borders::held_until`]):
//!
//! - it comes before the hold expires on every way: shown to hold, and nothing is said. The flow
//!   still handles `expired`, which dandori asks for since it does not compare lengths.
//! - it comes after on every way (E206): the book refuses it with `expired` on every run, and the
//!   way on where the call goes through never runs. The fewest seconds, and what makes them up, are
//!   the example.
//! - otherwise undecided (W206): it can come on either side, or nothing bounds how long after it
//!   comes (a task with no `timeout`, a rule or a date, a wait until a time nothing bounds).

use crate::{borders, named, Borders, Flow};
use ritsu_base::diag::Diag;
use ritsu_base::text::Lang;
use ritsu_base::tr;
use ritsu_ports::{seconds_text, Answer, Books, Expiry, Finding};
use ritsu_project::{Joined, Project};

pub(crate) fn check(project: &Project, flows: &[Flow], joined: &Joined, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in flows {
        for span in &f.calls.holds {
            let Ok(facts) = joined.chobo.facts(&span.book) else { continue };
            let Some(Expiry::After(expiry)) = facts.transfers.iter().find(|t| t.name == span.transfer).and_then(|t| t.pending) else { continue };
            let (case, task, made) = (&span.case, &span.task, span.made);
            let (verb_ja, verb_en) = if span.op == "void" { ("取り消す", "voids") } else { ("確定する", "posts") };
            let (least, after) = (seconds_text(span.least), seconds_text(expiry));
            let book = named(project, &span.book);
            let transfer = &span.transfer;
            match borders::held_until(span.least, &span.most, expiry) {
                Answer::Holds => borders.held += 1,
                Answer::Fails(_) => {
                    borders.failed += 1;
                    let mut diag: Diag = Diag::at("E206", &f.file.shown, span.line, 1, tr!(
                        "`{task}` が案件 `{case}` の仮押さえを{verb_ja}ときには、期限が必ず切れています",
                        "The hold of the case `{case}` has always expired when `{task}` {verb_en} it"
                    ))
                    .source(&f.src)
                    .rel(&f.file.rel)
                    .note(tr!(
                        "`{task}` は、仮押さえを作ってから（{made} 行目）早くても {}あとに来ます。仮押さえは作ってから {}で期限が切れます（\"{book}\" の `{transfer}`）",
                        "`{task}` comes {} or more after the hold is made (line {made}), and the hold expires {} after it is made (`{transfer}` of \"{book}\")",
                        least.ja, after.ja; least.en, after.en
                    ));
                    for why in &span.least_why {
                        diag = diag.note(why.clone());
                    }
                    let diag = diag
                        .note(tr!(
                            "帳簿はどの実行でもこれを `expired` で断るので、`{task}` が通ったあとの流れは動きません。",
                            "The book refuses it with `expired` on every run, so what follows `{task}` going through never runs."
                        ))
                        .note(tr!(
                            "仮押さえの期限を延ばすか（振替の `pending expires after`）、もっと早く呼んでください。",
                            "Make the hold last longer (the transfer's `pending expires after`), or make the call sooner."
                        ));
                    out.push(Finding::of(&diag, Some(f.file.rel.clone()), lang));
                }
                Answer::Undecided(why) => {
                    borders.undecided += 1;
                    let _ = why;
                    let note = match &span.most {
                        Ok(most) => {
                            let most = seconds_text(*most);
                            tr!(
                                "`{task}` は仮押さえを作ってから（{made} 行目）{}から{}のあいだに来ます。仮押さえは作ってから {}で期限が切れます",
                                "`{task}` comes {} to {} after the hold is made (line {made}), and the hold expires {} after it is made",
                                least.ja, most.ja, after.ja; least.en, most.en, after.en
                            )
                        }
                        Err(open) if span.least == 0 => tr!(
                            "`{task}` は仮押さえを作ってから（{made} 行目）すぐに来ることもあり、遅いほうには上限がありません（{}）。仮押さえは作ってから {}で期限が切れます",
                            "`{task}` can come right after the hold is made (line {made}), and nothing bounds how long after: {}; the hold expires {} after it is made",
                            open.ja, after.ja; open.en, after.en
                        ),
                        Err(open) => tr!(
                            "`{task}` は仮押さえを作ってから（{made} 行目）早くても {}あとに来ますが、遅いほうには上限がありません（{}）。仮押さえは作ってから {}で期限が切れます",
                            "`{task}` comes {} or more after the hold is made (line {made}), and nothing bounds how much more: {}; the hold expires {} after it is made",
                            least.ja, open.ja, after.ja; least.en, open.en, after.en
                        ),
                    };
                    let diag: Diag = Diag::at("W206", &f.file.shown, span.line, 1, tr!(
                        "`{task}` が案件 `{case}` の仮押さえを{verb_ja}ときに期限が切れているかを決められません",
                        "Whether the hold of the case `{case}` has expired when `{task}` {verb_en} it cannot be decided"
                    ))
                    .source(&f.src)
                    .rel(&f.file.rel)
                    .note(note)
                    .note(tr!(
                        "期限の切れた仮押さえへの呼び出しは、帳簿が `expired` で断ります。フローはここでそれを処理しています。",
                        "The book refuses a call on an expired hold with `expired`, which the flow handles here."
                    ));
                    out.push(Finding::of(&diag, Some(f.file.rel.clone()), lang));
                }
            }
        }
    }
    out
}
