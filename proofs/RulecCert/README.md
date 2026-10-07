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

## What is in it

| file | |
|---|---|
| `RulecCert/Semantics.lean` | what a table claims. Points, boxes, rows, a policy, and the three propositions `uniqueHolds`, `completeHolds`, `reachedHolds` |
| `RulecCert/Check.lean` | the checks, as functions: the pairs part, the cover tiles the space, every row has a point |
| `RulecCert/Sound.lean` | the theorems. A `true` from each check settles the matching proposition |
| `RulecCert/Sieve.lean` | which combinations the rule is **asked about**, from the `constraint` lines, the reach of each derived column and each `define` of a number, the truth value a boolean `define` of one comparison can take, and the rows of a table above that write a value — and the proof that a box the cover calls impossible really is one (`not_asked_of_derived`, `not_asked_of_farkas`, `not_asked_of_aboveRuledOut`, `not_asked_of_truth`), and that two rows the rows of a table above part share no point the rule is asked about (`not_asked_of_abovePart`) |
| `RulecCert/Linear.lean` | linear inequalities and the multipliers that refute a system of them: when the sum `farkasOk` checks comes out false, no values satisfy them all (`farkas_sound`) |
| `RulecCert/Contract.lean` | what a contract lets through, opened into cases, held to what the rule's door asks: when the proof for every case passes, any values the contract admits are ones the door takes (`included_sound`) |
| `RulecCert/Values.lean` | expressions, their evaluation, the units (E103) and int64 (E108) claims, and the proofs — and, for the share `allocate` works out, that a run of them hands out the amount exactly (`runTotal_exact`); and, for a column a table writes numbers into, that every value its rows write lies in the hull this program works out (`eval_mem_hull`) and in the range the certificate states once that range holds every row's interval (`eval_mem_stated`) |
| `RulecCert/Machine.lean` | a rule that is one step of a state machine: a case is what the rows allow from the initial state, and the checks on the reach set, the final states and the `never` and `once` lines hold of every sequence of calls (`reaches_mem`, `final_stays`, `never_after`, `once_below_two`); from the calls a certificate hands over, every state a case can reach can still finish (`reachable_finishes`) |
| `RulecCert/Cells.lean` | from the cells a rule writes to the boxes the claims are about: the compression of §6.2, shown faithful |
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
lists under `uncertified`, which it could not lay on the rows.

Beyond those, the document's own account of the rule — the declared ranges, the types, the
groups, the enums, the constraints, each value's expression and scale, the scale of every name —
is its word. What is worked out from it is checked: the step a numeric axis is cut at has to
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
digest ties the certificate to one text and every cell is read back from its own line and
column in it — the answers a row writes into a column of numbers among them; going behind the rest would take a parser for the rule, and a checker that
reads a rule the way rulec reads it is not independent of it. A contract's section is the
same: its digest ties it to the file beside the rule, and which inputs the contract feeds,
at what scale, and the atoms and cases are the document's reading of that file, since this
program reads neither CEL nor a schema.

"Asked about" is consistency with everything the rule declares, which is what rulec decides
— not "some real input produces this".
