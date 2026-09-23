# How it is checked

rulec makes a small number of claims and spends a lot of machinery on each one. This page is
the map: every layer, what it reaches, where it stops, and the command that runs it. Nothing
here asks you to take the layer above on trust — each one is a different program, and several
of them share no code with the checker at all.

The short version is the order they run in. A rule that fails a layer never reaches the next.

![Which joint each of the nine layers watches. The spine is four things — the business as it is, the document it was copied from, the table as written, and the generated code — each a copy of the one before it. Between the document and the table stands layer 9, the source check (E116, E119, W120); between the table and the code stand layers 4, 5 and 6, the vectors, the coverage criteria and the model checker. The table itself is watched by layers 1, 2 and 3, the five proofs, the two declarations and the examples. At the leftmost joint, between the business and the document, there is no layer at all: that is where a person read and decided. Layers 7 and 8, the certificate with its two re-checkers and the repository's own tests, are aimed at the tool rather than at a joint.](images/assurance.svg#only-dark)
![Which joint each of the nine layers watches. The spine is four things — the business as it is, the document it was copied from, the table as written, and the generated code — each a copy of the one before it. Between the document and the table stands layer 9, the source check (E116, E119, W120); between the table and the code stand layers 4, 5 and 6, the vectors, the coverage criteria and the model checker. The table itself is watched by layers 1, 2 and 3, the five proofs, the two declarations and the examples. At the leftmost joint, between the business and the document, there is no layer at all: that is where a person read and decided. Layers 7 and 8, the certificate with its two re-checkers and the repository's own tests, are aimed at the tool rather than at a joint.](images/assurance-light.svg#only-light)

Drawn out, the layers fall into place. **The spine is four things**, each a copy of the one
above it, and most of the checking lives at a **joint**, holding two of them to each other.
The table produces one more thing — **the certificate**, the JSON the five proofs rest on —
and layer 7 is the two programs that read it back. Only two layers stand at no joint: 1, 2
and 3, which look at the table on its own, and 8, which is aimed at the tool. **At the
topmost joint there is no layer at all**: a person read the world and wrote the document
down, and nothing here can check that reading.

| | what it settles | run it with |
|---|---|---|
| **1. The five proofs** | over **every** input the declarations allow: no gap, no overlap, no dead row, no unit confusion, no overflow | `rulec check` |
| **2. The two declarations** | rounding is written down, not guessed; an overlap the proof could not settle becomes a runtime guard that refuses rather than picks | `rulec check` |
| **3. The examples** | the cases a person wrote by hand still hold | `rulec check` |
| **4. The vectors** | the generated code answers like the reference evaluator, in twelve languages, byte for byte | `rulec test` |
| **5. The five coverage criteria** | the vector suite actually reaches every row, every boundary pair, every shadowed pair, every rounding tie and every fold transition | `rulec coverage` |
| **6. The model checker** | the generated Rust, over every input in the declared domain, read by a tool that shares no code with rulec | `rulec test --proofs` |
| **7. The certificate** | the evidence, small enough to hand over, re-checked by two programs that share no code with rulec — one of them carrying machine-checked proofs | `rulec certificate` |
| **8. The repository's own tests** | 94 deliberately broken rules each produce the diagnostic they should; 46 rules are checked, generated and run on every commit | `cargo test` |
| **9. The source** | an amount that disagrees with the document the row cites fails | `rulec source fetch`, then `rulec check` |

---

## 1. The five proofs

Completeness (E101), overlap (E105), dead rows (E102), units (E103) and int64 (E108) are
decided over **the whole declared input space**, not over samples of it. What makes that
finite is the compression of §6.2: a column is cut at the boundaries its own cells name, and
everything between two boundaries behaves alike, so a space of 10<sup>18</sup> combinations
becomes a few hundred boxes that can be walked.

**Where it stops:** at the table as written. A tariff transcribed wrong passes every one of
the five. That is what layer 9 is for, and even that only ties the table to a copy.

[What it proves](checks.md){ .md-button }

## 2. The two declarations

**Rounding is not inferred.** A numeric output without a `round` is E104, and the message
shows the money the choice moves. Which way to round is a business decision, so the tool
refuses to make it.

**An overlap that could not be settled becomes a guard.** Where neither an input matching two
rows nor a proof that none exists could be constructed, W114 names the pair and the generated
code returns an error instead of quietly preferring one row. It is the one place with no
static answer, and it is visible in the output rather than buried.

## 3. The examples

`examples` is an executable specification: every row runs through the reference evaluator on
every `rulec check`, and a row that does not hold is E107, reported with the rows that fired.

They exist because the layers below them can all agree on the same mistake. That happened
once: rounding for multiple outputs was missing in the reference evaluator *and* in every
generated language at the same time, and the agreement check stayed green throughout. The
only thing that can catch that is an answer a person wrote down.

## 4. The vectors, and twelve languages

`rulec gen` writes a vector suite beside the code: cases built from the boundaries of the
rule, each with the answer the reference evaluator gives. `rulec test` then runs every
generated language over them and compares **byte for byte** — not "close enough", and not
only the answer: the rows that fired are compared too.

Twelve languages, and more doors than languages: the module itself, the MCP server, the same
server over HTTP, the Rust runner compiled to WASI, the Wasm component, the SQL query and the
same query as a PostgreSQL function. Each door is a separate run. A rule that refuses an
input has a suite for that too — the refusals are cases, and a door that answers where it
should refuse fails.

**Where it stops:** this is a test, not a proof of equivalence. It is strong evidence over
a suite designed to be adversarial, which is a different thing from a theorem.

[Generate and call](generate.md){ .md-button }

## 5. The five coverage criteria

A suite that runs is not a suite that reaches. `rulec coverage` states the obligations the
rule itself implies and says which are met: every **row** wins somewhere, every **boundary
pair** has the two cases on either side of it, every **shadowed pair** under `policy first`
is exercised, every **rounding tie** lands on the exact half, and every **fold transition**
is taken. The obligations come from the rule, never from the suite — an auditor that says
"all satisfied" of an empty set says nothing, and a test in the repository holds it to that.

## 6. The model checker

Beside the generated Rust, `gen` writes proof harnesses for
[Kani](https://model-checking.github.io/kani/), behind `#[cfg(kani)]` so `rustc` never reads
them. They hold the generated code over **every** input in the declared domain: no table
falls through, no runtime guard of layer 2 fires, nothing overflows an `i64`, and the rows of
each `unique` table cover the domain exactly once.

It is worth running because the model checker and the checker that proved the table share no
code. Where they agree, two unrelated tools say the same thing; where they disagree, one of
them is wrong and the input that shows it comes back.

**Where it stops:** at Rust. And two kinds of rule get no harness at all, the file saying
which: one with a `string` input, which a harness cannot quantify over, and one that works
out a share with `allocate`, where two divisions by a value rather than by a constant do not
come back from the model checker.

## 7. The certificate, and two re-checkers

`rulec certificate` prints what the five proofs rest on: the tree that tiles the input space
with the row covering each box, the axis on which each pair of rows parts, a point that
reaches every row with the values behind it, the interval and the type every computed value
is forced into — and where every cell stands in your file, down to the byte.

Two programs read it, and neither shares code with rulec:

- **`tools/recheck.py`** — one file, no dependencies. It recomputes each box from the cell it
  was read from, reads every cell back out of the `.rule` text at the byte span the
  certificate names, and holds the rest to those claims. Milliseconds.
- **`proofs/`** — a Lean 4 development in which the meaning of a rule, the checks a
  certificate has to pass, and the theorems that each check settles its claim are all written
  down and machine-checked. The program `lake build` produces runs those very check
  functions, so what it prints is the theorems applied to one document. There is no `sorry`
  and no `axiom` anywhere in it, and `#print axioms` on the results shows only the three
  axioms Lean itself rests on.

Writing the second one, and reading it back adversarially, found a soundness bug in the
completeness check, a witness that could break the rule's own `constraint`, and eleven ways a
forged certificate got past a re-checker.

**Where it stops:** the certificate is tied to one text by a digest and, cell by cell, by
those byte spans. Everything else in it — the declared ranges, the types, the groups, the
constraints, each value's expression — is the document's own word, and going behind that
would take a parser for the rule. A checker that reads a rule the way rulec reads it is not
independent of it.

[Formats](formats.md){ .md-button }

## 8. The repository's own tests

The five proofs come out of rulec's implementation, and **that implementation has not been
proved correct**. What stands in for a proof is evidence, and it is kept deliberately:

- **94 deliberately broken rules**, each producing the diagnostic it should — and the
  expected codes are pinned, so a mutant that starts reporting something else fails.
- **46 rules** — 36 transcribed from real published terms, 10 written to reach the corners of
  the language — checked, generated and run in every language on every commit.
- **The documents are held to the tool.** The diagnostic ledger is regenerated from the code,
  the generated-code page is held to the tool's own output, and the examples on this site are
  held to the corpus files they came from. A page that drifts is a failing test.
- **Every diagnostic is in the ledger**: 83 codes, each with a smallest reproduction that is
  run on every commit to check it still produces that code.

## 9. The source a rule was transcribed from

A rule can cite the document it came from: a law in the government's own database
(`source 法 = law "342AC0000000023" asof 2026-04-01`) or a file beside it, pinned by digest.
Rows, tables, clauses and derivations then carry `@法 別表第一`, and `rulec check` compares
the amounts in the table with the amounts in the copy. One that is nowhere in the copy is
E116; a value of the copy that no row uses is W120, which is what usually comes with a
mistyped digit. A threshold is compared too, in the one way it can be: the copy's `60cm以下`
puts 60cm in the band below, `<60cm` puts it in the band above, and that is E119.

**Where it stops:** agreement with the copy, not with the world. And a rule with no citation
gets none of it — transcribe the tariff wrong and everything stays green.

---

## What none of it reaches

Stating this plainly is part of the job.

- **Whether the table says what the business means.** Every layer above is about the table as
  written. Layer 9 narrows it to "agrees with the copy we cited", and that is as far as any
  of this goes.
- **Whether rulec itself is right.** Layers 6, 7 and 8 attack it from three directions — an
  unrelated model checker, two independent re-checkers, and mutants — but none of them is a
  proof that the checker is correct. What *is* proved is the step after it: that a
  certificate passing its checks settles the claims it makes.
- **The pairs W114 could not settle.** They are named, and they become a runtime error rather
  than a silent choice.
- **Anything a rule does not declare.** The proofs quantify over the declared ranges. Widen
  an input's range and the claims are about the wider space; narrow it and the generated
  guard refuses what falls outside.

---

[What it proves](checks.md){ .md-button .md-button--primary }
[Generate and call](generate.md){ .md-button }
[Formats](formats.md){ .md-button }
