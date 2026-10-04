# The models behind ritsu's languages

This directory says, in Lean 4, what the core of three of ritsu's languages means, proves what
their checks settle, and builds a program that runs the same functions so that the Rust
implementations can be held to them (ritsu's DESIGN 11). It is a Lake package with no
dependencies beyond the toolchain `lean-toolchain` pins — the same Lean as rulec's
`crates/rulec/proofs`, which proves what a rulec certificate says and stays where it is for now.

```console
$ lake build                                  # checks the proofs, builds ritsu-model
$ cargo test -p ritsu-model -- --nocapture    # from the root, RITSU_TEST_LEVEL=tools
```

## What is in it

| library | what it models | what it proves |
|---|---|---|
| `ChoboModel` | chobo's transfers (chobo's DESIGN 2): accounts with posted, held-in and held-out amounts, the moves of a `do` or a `hold` made in the order written and each checked against the bounds, every move or none, the keys, how a hold ends, time passing, `together` | `apply_kept`, `pass_kept`, `within_stays`: no call and no time passing takes an account across a bound in the direction the bound forbids, counting what is held pessimistically; `refused_keeps_balances`, `doneBefore_keeps_state`, `again_doneBefore`: a refusal changes no balance, and the same call a second time is `done_before` and changes nothing |
| `KoyomiModel` | koyomi's dates (koyomi's DESIGN 2): Hinnant's day numbers with Rust's division, month arithmetic and the three ways with a missing day, closing days, the calendar's rules, rolling and counting business days, the times `at` gives | `monotone_of_adjacentOk`, `monotone_on_range`: comparing every day with the next is enough for `is monotonic` over the whole range; `seek_following`, `seek_preceding`: `roll following` gives the first business day on or after the day, `roll preceding` the last on or before it; `roll_open`, `addBusiness_open`, `ifClosed_open`: every convention and every count of business days, `± 0` included, gives a business day |
| `DandoriCore` | dandori's core (dandori's DESIGN 1.5, 1.16, 2): the statements, `match`, `for` and `repeat` with their bounds, the calls, their arguments and what comes of them, `on failure` and `on cancel`; the dates of koyomi's dates files (`use dates`), the operations of chobo's books and the holds a case follows (`use book`, `case … follows`), and `now`; a run against a scenario, as dandori's reference interpreter runs it for Temporal; and the check of what a case is left in when the flow ends (E020), with the cases' true states on the other side | `exec_sound`, `chkFlow_sound`: a flow the check passes ends, in every run where the check looks — the end of the flow, `succeed`, `fail`, the end of `on failure` — with every case it does not hand over with `leaving` either not started or in a final state, and in one the events on the other side cannot take it out of |

`Main.lean` is `ritsu-model`: `ritsu-model chobo <book.json>`, `koyomi <file.json>` or
`dandori <flow.json>` reads the file a test wrote (a book, a dates or calendar file, a flow,
resolved the way the model reads them) and answers every line of its standard input with one line:
a scenario's result, what an input of the range gives, a run.

## How the Rust side is held to it

`crates/ritsu-model` makes the inputs and compares the answers a line at a time, and fails with
the first five lines that differ, each with its input:

- chobo: every scenario chobo writes for every book of its examples and tests, and those written
  by hand beside them — 339 scenarios of 14 books, every outcome, balance and hold;
- koyomi: every line of `koyomi vectors` for every `.cal` of its examples and fixtures that
  resolves — 5,493,872 lines of 20 files, every input of every range and those just outside it;
- dandori: every scenario `dandori scenarios` writes for every flow of its examples and tests, the
  ones with dates and books too — 1,195 scenarios of 52 flows, everything a run goes through, the
  arguments of the calls whose wire carries them as they are (137 calls: a rule's or a date's, a
  task's own activity or Lambda function), how it ends and what each case's record says. A book's
  operation answers the hold its arguments make, which the model makes itself. How some answers are
  read before the flow sees them (Jev's, a Claude agent's enum values, protobuf's zero values of a
  Connect service or of a service the flow implements, a rule's answer from its Connect service) is
  not the core: the test hands the model what dandori's own functions read each answer as, and the
  model holds what the flow does with it.

`tests/proofs.rs` there reads every file here for `sorry`, `axiom`, `native_decide` and
`implemented_by`, and asks Lean what each declaration of the three libraries stands on: every one
stands on `propext`, `Classical.choice` and `Quot.sound` at most, the axioms Lean itself stands on.

`DandoriCore/Examples.lean` works the check out on two flows when the library builds. In the first,
a capture that times out may still have happened, the flow matches on the record (which still says
`pending`) and voids the payment, and the void's `on unexpected_state` ends the workflow. The check
refuses it — the payment can be left `processing` — and passes it once the handler hands the payment
over. The second makes the same mistake in `on failure`, as the hotel booking of dandori's examples
did. dandori's check says the same of both (its fixtures `capture_timeout.flow` and
`capture_timeout_handed_over.flow`).

## What it does not say

The theorems are about the models. That the Rust implementations compute the same functions is
evidence — every input above, compared — and not a proof. dandori's run against a scenario
(`DandoriCore.Replay`) is written with `partial` functions and nothing is proved of it; it is what
the comparison holds dandori's interpreter to. The theorem of `DandoriCore.Sound` is about the
check in `DandoriCore.Check`, which keeps, for each case, pairs of what the flow last heard and a
state the case was in since; it is not dandori's `flow.rs`, and it gives up where it cannot say
(a loop that does not settle, `for … in parallel`, an event sent to a case not surely started, a
case started twice). Its runs take one call to be one event on the other side however many times
it is tried, as a task's `key` makes it (a book's operation, as the book's key makes it), and leave
out a cancellation (Temporal's `on cancel`). dandori's check keeps the same pairs, and differs from
this one in two places, where this one refuses more: a call that starts a case and fails may have
made the case on the other side, which this check counts and dandori's leaves to W103; and dandori's
check takes a refusal's error to come back only from a refusal, so that a handler of the refusal
alone sees only the states that refuse, where this check gives every handler every state an error
can leave.
