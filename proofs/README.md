# The Lean layer of ritsu

This directory says, in Lean 4, what a rulec certificate means and what the core of four of
ritsu's languages means, what the checks across the languages decide, and proves that each check
settles what it says (ritsu's DESIGN 11). It builds two programs that run the same functions: one
holds a rulec certificate to its claims, the other holds the Rust implementations to the models.
It is one Lake package with no dependencies beyond the toolchain `lean-toolchain` pins.

```console
$ lake build                                  # checks the proofs, builds rulec-recheck and ritsu-model
$ cargo test -p ritsu-model -- --nocapture    # from the root, RITSU_TEST_LEVEL=tools
```

## What is in it

| library | what it models | what it proves |
|---|---|---|
| `RulecCert` | what a rulec certificate says of a rule: the space of a table, its rows and policy, which combinations the rule is asked about, the values and their units, a contract, a state machine and the values it carries besides its state, the range each output answers in once rounded (rulec's DESIGN §15.97, §15.217, §15.269) | a certificate that passes the checks settles its claims: completeness, no overlap, the rows that answer, units, int64, the contract, the machine's claims, over the carried values too, and the range of each output, which the other languages read the rule's answer in (`output_mem_stated`, `written_output_mem_stated`). [RulecCert/README.md](RulecCert/README.md) has the whole of it |
| `ChoboModel` | chobo's transfers (chobo's DESIGN 2): accounts with posted, held-in and held-out amounts, the moves of a `do` or a `hold` made in the order written and each checked against the bounds, every move or none, the keys, how a hold ends, time passing, `together` | `apply_kept`, `pass_kept`, `within_stays`: no call and no time passing takes an account across a bound in the direction the bound forbids, counting what is held pessimistically; `refused_keeps_balances`, `doneBefore_keeps_state`, `again_doneBefore`: a refusal changes no balance, and the same call a second time is `done_before` and changes nothing |
| `KoyomiModel` | koyomi's dates (koyomi's DESIGN 2): Hinnant's day numbers with Rust's division, month arithmetic and the three ways with a missing day, closing days, the calendar's rules, rolling and counting business days, the times `at` gives | `monotone_of_adjacentOk`, `monotone_on_range`: comparing every day with the next is enough for `is monotonic` over the whole range; `seek_following`, `seek_preceding`: `roll following` gives the first business day on or after the day, `roll preceding` the last on or before it; `roll_open`, `addBusiness_open`, `ifClosed_open`: every convention and every count of business days, `± 0` included, gives a business day |
| `DandoriCore` | dandori's core (dandori's DESIGN 1.5, 1.16, 2): the statements, `match`, `for` and `repeat` with their bounds, the calls, their arguments and what comes of them, `on failure` and `on cancel`; the dates of koyomi's dates files (`use dates`), the operations of chobo's books and the holds a case follows (`use book`, `case … follows`), and `now`; a run against a scenario, as dandori's reference interpreter runs it for Temporal; and the check of what a case is left in when the flow ends (E020), with the cases' true states on the other side | `exec_sound`, `chkFlow_sound`: a flow the check passes ends, in every run where the check looks — the end of the flow, `succeed`, `fail`, the end of `on failure` — with every case it does not hand over with `leaving` either not started or in a final state, and in one the events on the other side cannot take it out of |
| `RitsuCross` | the checks across the languages (ritsu's DESIGN 7, `crates/ritsu-cross`): a rule's precondition where a workflow calls it (X2) — rulec's answer for a relation over two ranges, and the days of a koyomi date a value given to a date input can be — and where a module of kaname calls it, the relations kaname gives first, then the corner of its ranges, then the runs of the function's vectors (kaname's DESIGN 14.3); the days a value can be against a rule's range (X3 (a)) and as a rule's range (X3 (b)); the amounts a value can be as a transfer's amount, what chobo's search is held to, and the refusals of the transfer against the errors a task handles (X4); a hold against when it expires (X5); the day given to a koyomi date (X6); and the set of days a koyomi date comes to, computed over every combination of its inputs | each answer settles what it says: `corner_holds`, `corner_fails`, `x2_holds`, `x2_fails`, `x2_days_holds`, `x2_days_fails` (every value the call can give keeps the precondition, or the example breaks it); `daysFit_holds`, `daysFit_fails`, `daysGiven_holds`, `daysGiven_fails`, `amountFits_holds_iff`, `amountFits_fails`, `amountFits_fails_first`, `amountFits_undecided` (the example of an amount is an input rulec gives from the rule's vectors, the one furthest out, and with none every example is an amount chobo takes; rulec's examples hold the fewest and the most the vectors come to, rulec's DESIGN §15.270), `amountsGiven_holds`, `amountsHull_covers`, `refusalsMet_holds`, `refusalsMet_fails`, `refusalsMet_undecided`, `heldUntil_holds`, `heldUntil_fails`, `inputRange_some`. Across the border itself: `x3a_holds`, `x3a_fails` (for every input of koyomi's ranges the date is computed and lies in the rule's range; the example is a day koyomi computes); `x3b_complete`, `x3b_unique` (a rule whose certificate takes koyomi's days and passes its checks answers, for every input of koyomi's ranges, every point with koyomi's day, with exactly one row); `amountFits_chobo_takes`, `amountFits_fails_chobo` (an amount the check lets through is one `ChoboModel.fits` takes, and the amount it gives as an example is one it does not take: both take 0 to 2⁶³ − 1); `heldUntil_fails_expired`, `heldUntil_holds_held` (a hold X5 says always expires first has expired, as `ChoboModel.expiredAt` says, when the call comes, and one X5 says is still held has not); at a call of kaname, `kept_sound` (where the relations kaname gives say the precondition, every value that keeps them keeps it), `x2k_holds`, `x2k_fails`, `x2k_undecided` |
| `KanameCore` | kaname's evaluation and its analysis of ranges (kaname's DESIGN 5, 8, 15): numbers as exact rationals held to int64, the five ways of rounding as one integer procedure, every form of expression and builtin, the places a value is held to what it declares (an input of a rule's ordered enum among them, held to the values its range lets in), a rule's and a date's answers from outside (what rulec and koyomi show of their own: rulec refuses a number outside its input's range, a value of an enum the range does not let in and a field of an element outside its range), and the analysis of `ranges.rs` written again — intervals with a bound on the denominator, narrowing by conditions, `match` and calls, the relations a condition and `running_totals` keep — among them that the `total` a running total carries is the `sum` of the same field of the same list, a relation between terms that add a list up as well as names — the places and whether each is shown, the relations each call of a rule keeps between its inputs, the rings of calls and the effects, and the verdicts of the checks across the languages taken after the analysis | `roundInt_eq_roundQ`, `roundInt_roundsTo`, `roundInt_mono`, `In_round`: the integer procedure is the rational rounding, on the grid between the value rounded down and up, monotone, and an interval rounds to one that holds every value of it rounded; `ranges_sound`: where the environment is among those the analysis says, an answer of an expression is among its abstract values, the narrowed scope still holds it, and a failure of one of the five kinds is a place not shown or a function called that can fail; `analyzeFn_sound`, `module_sound`: the same of each function's answer, for any values its parameters take; `no_failure`, `fn_noFail`, `total_sound`: where every place is shown and every function called does not fail, there is no failure, and a function the analysis infers total fails for no values its parameters take; `summed_eval`, `totals_eval`, `Sat.bindLam`: a `sum` of a field answers the value of its term, every element `running_totals` answers carries that value as its `total`, and the relation the two keep holds where a function walks those elements; `callRels_sound`: every relation a call of a rule keeps holds between the values of the two names given, in every environment the scope of the call holds, and `kept_holds`, with RitsuCross's `kept_sound`: where those say a precondition, it holds there; `analyzeWith_nil`, `decideAcross_other`, `total_with_sound`: a verdict changes only a place that waits for one, with none the analysis is the one above, and a function the effects with the verdicts call total fails for no values its parameters take, where what each verdict that holds says is so. For an analysis that let no running value of a `reduce` go |

`RulecMain.lean` is `rulec-recheck`: it reads a certificate and the rule it is about and runs the
checks of `RulecCert` on them (rulec's `tests/lean.rs`, and over the corpus in CI).

`Main.lean` is `ritsu-model`: `ritsu-model chobo <book.json>`, `koyomi <file.json>`,
`dandori <flow.json>`, `cross <file.json>` or `kaname <module.json>` reads the file a test wrote (a
book, a dates or calendar file, a flow, a module of kaname, resolved the way the model reads them;
for `cross`, a dates file whose days the lines ask for, or nothing) and answers every line of its
standard input with one line: a scenario's result, what an input of the range gives, a run, a
check's answer, a function's answer or the analysis of the module.

## How the Rust side is held to it

`crates/ritsu-model` makes the inputs and compares the answers a line at a time, and fails with
the first five lines that differ, each with its input:

- chobo: every scenario chobo writes for every book of its examples and tests, and those written
  by hand beside them — every outcome, balance and hold;
- koyomi: every line of `koyomi vectors` for every `.cal` of its examples and fixtures that
  resolves — every input of every range and those just outside it;
- dandori: every scenario `dandori scenarios` writes for every flow of its examples and tests, the
  ones with dates and books too — everything a run goes through, the arguments of the calls whose
  wire carries them as they are (a rule's or a date's, a task's own activity or Lambda function),
  how it ends and what each case's record says. A book's operation answers the hold its arguments
  make, which the model makes itself. How some answers are read before the flow sees them (Jev's, a
  Claude agent's enum values, protobuf's zero values of a Connect service or of a service the flow
  implements, a rule's answer from its Connect service) is not the core: the test hands the model
  what dandori's own functions read each answer as, and the model holds what the flow does with it;
- the checks across the languages: rulec's answer for relations over every pair of ranges a grid
  of ends makes, at one scale and at two; X2 as `ritsu check` decides it on small projects (a
  relation, and a date input over the days of a koyomi date), from the values dandori's port says a
  call gives; the days koyomi's port hands over for every date of every dates file that passes its
  check, and those a rulec certificate carries for an input whose range is a koyomi date, against
  the days the model computes; `days_fit` and `days_given`, `amount_fits`, `amounts_given` and
  `amounts_hull` (every numeric output of rulec's corpus, and three rules written for it: a refund the rule's vectors reach, a difference a `constraint` keeps at 0 or more, and the same difference less 5, whose example is the fewest the vectors come to), `refusals_met` (what chobo's search
  finds for every transfer, held to each book's bounds), `held_until` and `input_range` (every
  input of the dates files), over inputs around each answer;
- kaname: every input `kaname vectors` chooses for every function of kaname's examples (in English
  and Japanese), of its test modules (but `calls`, which calls the functions of another module) and
  of modules made from the examples so that the checks across the languages come to each of their
  answers, checked as `ritsu kaname` checks them, with the answers the reference interpreter got
  from the rules and dates it called — every answer or failure; and the analysis, with ritsu-cross's
  verdicts taken where kaname took them — each function's result, places, effect and the relations
  each of its calls of a rule keeps, and the abstract value of every expression; every value the
  reference interpreter sees is within what the analysis says. And X2 at a call of kaname: small
  modules that call a rule with a `constraint`, each given the relations, ranges and runs kaname's
  port hands over, against the verdict ritsu-cross gives.

`-- --nocapture` prints how many of each it compared. `tests/proofs.rs` there reads every file
here for `sorry`, `axiom`, `native_decide` and `implemented_by`, and asks Lean what each
declaration of the six libraries stands on: every one stands on `propext`, `Classical.choice` and
`Quot.sound` at most, the axioms Lean itself stands on.

`DandoriCore/Examples.lean` works the check out on four flows when the library builds. In the
first, a capture that times out may still have happened, the flow matches on the record (which
still says `pending`) and voids the payment, and the void's `on unexpected_state` ends the
workflow. The check refuses it — the payment can be left `processing` — and passes it once the
handler hands the payment over. The second makes the same mistake in `on failure`, as the hotel
booking of dandori's examples did. dandori's check says the same of both (its fixtures
`capture_timeout.flow` and `capture_timeout_handed_over.flow`). The last two pass where the check
reads a run as dandori's does: the call that opens a payment fails and `on failure` passes on a
record that says nothing; a void's refusal comes back only once the hold has expired, and its
handler passes.

## What it does not say

The theorems are about the models. That the Rust implementations compute the same functions is
evidence — every input above, compared — and not a proof. dandori's run against a scenario
(`DandoriCore.Replay`) is written with `partial` functions and nothing is proved of it; it is what
the comparison holds dandori's interpreter to. The theorem of `DandoriCore.Sound` is about the
check in `DandoriCore.Check`, which keeps, for each case, pairs of what the flow last heard and a
state the case was in since; it is not dandori's `flow.rs`, and it gives up where it cannot say
(a loop that does not settle, `for … in parallel`, an event sent to a case not surely started, a
case started twice). Its runs leave out a cancellation (Temporal's `on cancel`), and take four
things for granted, as dandori's check does: one call is one event on the other side however many
times it is tried, as a task's `key` makes it (a book's operation, as the book's key makes it); a
case is started once; a call that starts a case and fails has not started it (dandori asks for a
task's `key`, W103, so that a retry finds the case the other side made); and a refusal's error —
the task's `refused as`, or a reason a book refuses with — comes back only from a refusal, so a
handler of the refusal alone sees only the states that refuse.

`RitsuCross` takes what the languages hand over as given: the ranges dandori gathers for a call,
the interval rulec holds an output to, the refusals chobo's search finds (which goes only as deep
as chobo's check), the facts koyomi gives of a file. Where X2 gives both inputs one value, it
holds that `<=` and `>=` keep the precondition and `<` and `>` break it when the two inputs travel
at one scale, which the theorems take as their premise and dandori's E003 makes so (a value of one
step is not given to an input of another). X3 (b) says what a rule's certificate settles over the
days when they are koyomi's set; that the certificate carries koyomi's set is compared, not proved.
X5 takes the fewest and the most seconds dandori counts along the flow as given, and says what
follows of the hold's expiry in chobo's interpreter.

`KanameCore` takes a rule's and a date's answers from outside, as rulec and koyomi show them of
their own (`OracleOK`), and its evaluation leans on what kaname's check of types holds — a value on
the step its type says, an argument among the values of its type, an arm that takes the value —
by failing `type` where it does not hold, which no module that passes the check meets. The
theorems leave out an analysis that lets the ends of a `reduce`'s running value go (kaname's
DESIGN 5.5): every function's facts must say `widened` is false. They say nothing of an evaluation
that runs out of steps or calls too deep. The model is `ranges.rs` and `interp.rs` written again;
where it is not as they are (the reference interpreter leaves a value's step to the check of
types; which place a verdict is taken at is Rust's to find; the model reads one module), kaname's
DESIGN 15.3 lists it. `total_with_sound` takes it as given that what each verdict that holds says
is so: that is what RitsuCross proves of each check, and only the relation stage of X2 is tied to
the model's evaluation (`kept_holds`).
