# Rego as a tenth target: does the arithmetic survive?

`opa eval` is one static binary that runs offline and answers JSON with JSON, so unlike a
warehouse it could sit **inside** `rulec test` rather than beside it — the thing that decides
whether a target is in this tool's claim or outside it (§15.46). Whether it should turns on
one question that was not guessable from the documentation: a rule is exact integers with a
declared rounding, and Rego's numbers are JSON numbers with a `/` that returns a rational.

This directory is that question, measured. `report.txt` is what `run.sh` printed on
2026-09-19 against OPA 1.20.2 (Rego v1) and rulec 0.6.0. In short: **the arithmetic survives,
and the emitter has exactly one rule to follow.**

## What is here

- `round.rego` — the five rounding modes of §7.3, written the way the generated Python, Rust
  and Go helpers are written: on the magnitude, with the sign put back.
- `ec261.rego` — Article 7 of Regulation (EC) No 261/2004, transcribed by hand from
  `tests/corpus/ec261.rule`: one Rego rule per row of each of the three tables, an entry
  guard that returns an error object, and a second copy of the same arithmetic with the
  division transcribed literally, as a control.
- `cases.py` — the cases, taken out of the tool's own output rather than written here: the
  `CASES` of the generated `_round_test.py`, a stress set built from the same reference
  implementations over values that set does not reach, and the vectors of `ec261`.
- `run.sh` — the whole measurement, end to end; `report.txt` — what it printed.

```console
$ cargo build --release
$ sh run.sh                  # needs `opa` on the PATH; OPA=... points at one elsewhere
```

`opa` is one binary from <https://github.com/open-policy-agent/opa/releases> and is not
committed here. Nothing in this directory reaches the network when it runs.

## What it found

**Rego's numbers are exact rationals, not float64.** `floor((2^53 + 1) / 1)` comes back as
9007199254740993, `123456789 * 987654321` as 121932631112635269, and int64's maximum survives
a round trip. The first risk — that a tariff would quietly lose its last digits — is not
there.

**The five modes transcribe without loss.** All 270 of the vectors rulec generates for its own
rounding helpers agree, and so do 27,005 harder ones: |x| up to int64's maximum, grids from 1
to 1,000,000,007, both signs. Nothing was skipped, which matters in Rego, where a body that
fails is not an error but an absence — so the run counts what answered as well as what agreed.
It takes 0.4 seconds, which is well inside what a pass of `rulec test` can afford.

**The direction of the rounding was the risk that did not bite.** Rego's `floor` goes toward
−∞ and `round down` in a rule goes toward zero, which is a real difference — and it disappears
the moment the helper is written on the magnitude with the sign put back, the shape every
generated helper already has.

**One whole rule agrees.** All 94 vectors of `ec261` — three stacked tables, a rate held as a
count of 50% steps, a rounding at a scale — come back identical, and the entry guard refuses
an out-of-range input with an error object rather than an absence.

**`/` must never be emitted bare.** The generated SQL writes the scale out as
`((raw / 2) * 2) / 2`, which depends on integer division truncating. Rego's does not:
`(7 / 2) * 2` is 7. **And `ec261`'s own 94 vectors do not catch it** — every product there
happens to be even, so the literal transcription passes all of them. At a scale of 100 the
same code returns 123.45 where the answer is 123: a non-integer, in a tool whose claim is that
every value is an integer in its declared unit. What catches it is the rounding unit vectors,
which `gen` writes as a file of their own beside each language.

**An undefined answer is silent.** `_base("xl")` is not an error in Rego, it is no value at
all, and at the caller that is indistinguishable from a denial — which is the shape of the
accident this tool would be there to prevent. A generated policy has to return an object
(`{"compensation": …}` or `{"error": …}`), never a bare value, the way the Wasm target already
does.

## What is left

Not the arithmetic. The grammar: how a `fold` or a `count` walks a sequence in Rego, how
`overrides` puts one definition ahead of another, how a value outside an enum is refused at
the door. Those are transcription questions, and a wrong answer to one of them is a mistake
that shows up in the vectors — not, like a float, a wrong answer that looks right.
