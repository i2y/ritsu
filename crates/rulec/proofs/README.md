# The proofs behind a rulec certificate

`rulec certificate <file.rule>` prints the evidence for the five things `rulec check`
proves. This directory says what that evidence **means**, and proves that a certificate
which passes the checks really does settle the claims. It is a Lean 4 package with no
dependencies beyond the toolchain `lean-toolchain` pins.

```console
$ lake build                                   # checks the proofs, builds the program
$ rulec certificate rules/送料.rule > cert.json
$ .lake/build/bin/rulec-recheck --rule rules/送料.rule cert.json
```

## What is in it

| file | |
|---|---|
| `RulecCert/Semantics.lean` | what a table claims. Points, boxes, rows, a policy, and the three propositions `uniqueHolds`, `completeHolds`, `reachedHolds` |
| `RulecCert/Check.lean` | the checks, as functions: the pairs part, the cover tiles the space, every row has a point |
| `RulecCert/Sound.lean` | the theorems. A `true` from each check settles the matching proposition |
| `RulecCert/Sieve.lean` | which combinations the rule is **asked about**, from the `constraint` lines and the reach of each derived column — and the proof that a box the cover calls impossible really is one |
| `RulecCert/Values.lean` | expressions, their evaluation, the units (E103) and int64 (E108) claims, and the proofs |
| `RulecCert/Cells.lean` | from the cells a rule writes to the boxes the claims are about: the compression of §6.2, shown faithful |
| `RulecCert/Certified.lean` | one table's certificate, and the three theorems put together |
| `RulecCert/Read.lean`, `RulecCert/Sha256.lean`, `Main.lean` | reading the JSON, the digest, and the program that runs the checks |

Nothing is left open: `tests/lean.rs` in the repository above fails if a `sorry`, an
`axiom`, or a `native_decide` appears anywhere here.

## What it does not say

The theorems are about the **document**, not about rulec. That `rulec certificate` produces
a true certificate is still evidence — the corpus, the mutants, the second opinion from a model checker — and not a
proof. What changes is where the trust sits: a certificate that passes now means something
exact, and that meaning is written down in `Semantics.lean` rather than in prose.

Five things the document states and this program cannot re-check. It names them in a line
of its own rather than printing a clean "ok": a cover leaf that rests on a table above, the
row pairs W114 could not settle, rows an `apply` brought in from another file, rows the
sieve rules out entirely, and a reach point handed over with no values behind it.

Beyond those, the document's own account of the rule — the declared ranges, the types, the
groups, the constraints, each value's expression and scale — is its word. The digest ties
the certificate to one text and every cell is read back from its own line and column in it;
going behind the rest would take a parser for the rule, and a checker that reads a rule the
way rulec reads it is not independent of it.

"Asked about" is consistency with everything the rule declares, which is what rulec decides
— not "some real input produces this".
