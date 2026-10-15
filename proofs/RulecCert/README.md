# The proofs behind a rulec certificate

`rulec certificate <file.rule>` prints the evidence for the five things `rulec check`
proves. This library says what that evidence **means**, and proves that a certificate
which passes the checks really does settle the claims. It is `RulecCert`, one library of
ritsu's Lean package (`proofs/`, the directory above this one, with no dependencies beyond
the toolchain its `lean-toolchain` pins), and `rulec-recheck` is one of its two programs.
The paths below are from `proofs/`, and the commands run there.

```console
$ lake build                                   # checks the proofs, builds the programs
$ rulec certificate rules/shipping_fee.rule > cert.json
$ .lake/build/bin/rulec-recheck --rule rules/shipping_fee.rule cert.json
```

A contribution's certificate is the certificate of the rule it is merged into, so `--rule` names
that rule; the files of the contributions are read from the paths the certificate gives for them,
from that rule's directory.

## What is in it

| file | |
|---|---|
| `RulecCert/Semantics.lean` | what a table claims. Points, boxes, rows, a policy, and the three propositions `uniqueHolds`, `completeHolds`, `reachedHolds` |
| `RulecCert/Check.lean` | the checks, as functions: the pairs part, the cover tiles the space, every row has a point |
| `RulecCert/Sound.lean` | the theorems. A `true` from each check settles the matching proposition |
| `RulecCert/Sieve.lean` | which combinations the rule is **asked about**, from the `constraint` lines, the reach of each derived column and each `define` of a number, the truth value a boolean `define` of one comparison can take, and the rows of a table above that write a value — and the proof that a box the cover calls impossible really is one (`not_asked_of_derived`, `not_asked_of_farkas`, `not_asked_of_aboveRuledOut`, `not_asked_of_truth`), and that two rows the rows of a table above part share no point the rule is asked about (`not_asked_of_abovePart`) |
| `RulecCert/Linear.lean` | linear inequalities and the multipliers that refute a system of them: when the sum `farkasOk` checks comes out false, no values satisfy them all (`farkas_sound`) |
| `RulecCert/Contract.lean` | what a contract lets through, opened into cases, held to what the rule's door asks: when the proof for every case passes, any values the contract admits are ones the door takes (`included_sound`) |
| `RulecCert/Values.lean` | expressions, their evaluation, the units (E103) and int64 (E108) claims, and the proofs — and, for the share `allocate` works out, that a run of them hands out the amount exactly (`runTotal_exact`); and, for a column a table writes numbers into, that every value its rows write where they fire lies in the hull this program works out (`eval_mem_hull`) and in the range the certificate states once that range holds every row's interval (`eval_mem_stated`) — a row that writes one of its own table's columns inside the cut its own cell puts on the value (rulec's §15.213); and the grid facts a strict comparison and a carried value rest on (`grid_succ_le`, `grid_above`, `grid_below`); what a `max` or a `min` answers, inside the hull of its column and its `empty` value (`extreme_mem_stated`), and what a fold answers, one of the values its arms, `empty` and `exhausted` name (`foldAnswer_mem_stated`, rulec's §15.233); and the evaluation where every rounding meets a value already on its grid, which is the value under every mode (`evalExact_sound`); and what an open value comes to — its terms added, multiplied, the largest or the smallest — inside the range it declares once that range holds the interval the terms' intervals work out to (`openValue_mem_stated`, rulec's §15.243) |
| `RulecCert/Machine.lean` | a rule that is one step of a state machine: a case is what the rows allow from the initial state, and the checks on the reach set, the final states and the `never` and `once` lines hold of every sequence of calls (`reaches_mem`, `final_stays`, `never_after`, `once_below_two`); from the calls a certificate hands over, every state a case can reach can still finish (`reachable_finishes`) |
| `RulecCert/Cells.lean` | from the cells a rule writes to the boxes the claims are about: the compression of §6.2, shown faithful; the cut a row's own cell puts on the value it writes into a column of numbers, which holds at every point the row's box takes (`cut_of_box`, `written_mem_hull`, rulec's §15.213); and the axis of an input of an enum, written as the enum's values or those its `range` lets in, has a coordinate for exactly the values the input takes (`enumAxis_coords`, rulec's §15.210) |
| `RulecCert/Carried.lean` | a machine that carries more than its state (rulec's §15.217): a case is a configuration — the state and each carried value — and a call that really happens moves it as the row's answer and the output's rounding say. The check reasons over places, coordinates of the carried values; every call from a configuration at a place is one the check counts from there (`Model.simulates`), so places closed under those calls hold every configuration a case can reach (`Sys.reaches_mem`), whether the walk took every value or cut a number into intervals, and the claims laid on them hold of the configurations: `Sys.final_stays`, `Sys.never_after`, `Sys.once_below_two`, `Model.always_holds`, `Model.range_holds`. That a case can still finish rests on calls that really happen (`Model.fires`, rulec's §15.233): a path the program runs call by call is one (`Model.fires_of_callOk`, `Model.finishes_of_pathOk`), and with the places closed every configuration a case can reach can finish (`Model.reachable_finishes`) |
| `RulecCert/Prereq.lean` | which values of an enum need which before them (rulec's §15.222): from the order the certificate gives, no value needs itself however far its needs are followed (`no_cycle`); a case that starts with `start` and takes the values in the other order comes to hold them all (`reached_holds`), and none of those no case can take (`never_held`); and an `ordered by` table's answers do not go down along the needs (`order_along`) |
| `RulecCert/Axes.lean` | a numeric axis tiles what its column takes (rulec's §15.243): where the check passes, every value of the range on the axis's grid stands on exactly one coordinate (`axisTilesB_cover`, `axisTilesB_unique`); what a `max`, a `min` or a fold answers stands on a coordinate of the axis a table below reads it on (`extreme_on_axis`, `foldAnswer_on_axis`); and a point made of such coordinates is one a row takes (`Certified.answers_at`) |
| `RulecCert/Chances.lean` | the claims of `chances` (rulec's §15.224–§15.226, §15.243): wherever the inputs that are not random take values the claim's cells let in, the share of the draws whose outputs pass its output cells passes the chance it writes, under every rounding. Counted box by box — the coordinates of a random axis tile what its input is drawn from (`tiles_flatMap`, `countP_cart_flatMap`), inside a box the tables answer every draw alike (`cellOf_sound`, `Claim.box_const`), and a class's hits over its points are the share (`Claim.count_share`) — it is `Claim.holds`; run point by point, `RunClaim.holds`; over a pick by weight, the walk run at every draw, `WeightedClaim.holds` |
| `RulecCert/Certified.lean` | one table's certificate, and the three theorems put together |
| `RulecCert/Read.lean`, `RulecCert/Sha256.lean`, `RulecMain.lean` | reading the JSON, the digest, and the program that runs the checks |

Nothing is left open: rulec's `crates/rulec/tests/lean.rs` fails if a `sorry`, an `axiom`,
or a `native_decide` appears anywhere in the package, and asks Lean what the theorems the
claims rest on stand on; `crates/ritsu-model/tests/proofs.rs` asks it of every declaration
of every library.

## What it does not say

The theorems are about the **document**, not about rulec. That `rulec certificate` produces
a true certificate is still evidence — the corpus, the mutants, the second opinion from a model checker — and not a
proof. What changes is where the trust sits: a certificate that passes now means something
exact, and that meaning is written down in `Semantics.lean` rather than in prose.

Four things the document states and this program cannot re-check. It names them in a line
of its own rather than printing a clean "ok": the row pairs the axes do not part, rows an
`apply` brought in from another file, rows the sieve rules out entirely, and a reach point
handed over with no values behind it. A certificate from before rulec's §15.190, which states
no `scales`, adds the step an axis is cut at where the step of its column's values cannot be
worked out without them; one from before rulec's §15.196, which states no `inputs` and no
`writes`, adds the kinds of its columns and the ranges of the columns its tables write numbers
into, and an answer cell written in a unit this program does not convert adds itself. A contract adds three: a condition with parts that
could not be read and were taken as true, one that opens into too many cases, and a thing
the door asks that the document gives no proof for. A machine adds one: a claim the document
lists under `uncertified`, which it could not lay on the rows — among them, for one that carries
more than its state, that a case can still finish where a carried number was cut into intervals: a
place is then many configurations, and no path of calls is given from each. One that carries more
than its state adds, in a certificate that does not say what grid a carried output rounds to
(`round`), its claims over the carried values; and a certificate from before rulec's §15.233 adds
where a case starts, which no `initial_source` ties to the file. A claim of `chances` adds itself
where `check` left it undecided within its budget (W129), or where this program cannot run it as
the check did: a value the rule computes that reads one listed after it, a rounding that decides
the answer at a draw, a column of strings, more points than it runs.

Beyond those, the document's own account of the rule — the declared ranges, the types, the
groups, the enums, the constraints, each value's expression and scale, the scale of every name,
the grid a carried output rounds to, what a `max` or a `min` reads and the values a fold answers
with, the graph of a `prerequisites` block, the cell a `once` line tests, the terms of an open value,
the grid each output a claim of `chances` reads rounds to, the arms of a pick by weight and what
each takes — is its word. What is worked out from it is checked: the step a numeric axis is cut at has to
divide the step its column's values take, worked out from the expressions and the scales the
way rulec works it out (`checkSteps` in `RulecMain.lean`), so an axis of tenths of a pound cut
every whole pound is refused; and the interval each value the rule computes is forced into is
worked out from its expression (`interval`), which is where the reach of a derived or `define`
column and the ranges of those values in a linear model come from, not the `ranges` the
document states for them (rulec's §15.195). A column a table writes numbers into is bounded the
same way, by the hull of what its rows write, each row's answer cell an expression of its own
(`hullOf`): it is what a value over the column is worked out with and what the column's range in
a linear model is, and the range the document states for it is held to it (rulec's §15.196). The
truth value a boolean `define` of one comparison can take is worked out from its comparison and
the intervals of its two sides, and the kind of every column from `inputs`, `walks`, `values` and
`decides`. Where a leaf or a pair rests on the rows of a table above that write a value, those
rows are counted from that table's part of the document, not from the leaf. The
digest ties the certificate to one text — and every contribution merged into the rule to its own
file — and every cell is read back from its own line and column in it — the answers a row writes
into a column of numbers among them, the words it writes, out of the file the row is written in
where a table is written in several (rulec's §15.243), what a machine's moves and `once` lines read
in each row's own answer cell, where a case starts, out of the `initial` lines (rulec's §15.233),
and the cells and the chance of every claim of `chances` and of every `sequence` a pick by weight
walks (§15.243); going behind the rest would take a parser for the rule, and a checker that
reads a rule the way rulec reads it is not independent of it. A contract's section is the
same: its digest ties it to the file beside the rule, and which inputs the contract feeds,
at what scale, and the atoms and cases are the document's reading of that file, since this
program reads neither CEL nor a schema.

"Asked about" is consistency with everything the rule declares, which is what rulec decides
— not "some real input produces this".
