# Properties and proofs

An `examples` line gives the answer for one input. A **property** says something about every input
of a function: the shares of a refund come to the refund, no line gets a negative share. kaname
tries a property on the inputs it chooses, and writes it as a statement for Lean, where a proof
shows it for all of them. kaname does not search for the proof. Someone writes it, a person or an
agent, in a file beside the `.kn`, and Lean checks it.

## A property

The example is `crates/kaname/examples/refund_split`: a refund spread over the lines of an order in
proportion to what is still unpaid on each, with `allocate` and `running_totals`.

```kn
fn split(lines: list[Line] count >=1 <=10, refund: money[USDc] range >=0USDc <=10_000_000USDc) -> list[Share] total
  description "Each line gets the refund up to its end less the refund before it"
  map(running_totals(lines, l => l.left), r =>
    Share { sku: r.item.sku, left: r.item.left, share: allocate(refund, r.upto, r.total) - allocate(refund, r.before, r.total) })

property shares_add_up(lines: list[Line] count >=1 <=10, refund: money[USDc] range >=0USDc <=10_000_000USDc)
  description "The shares come to the refund"
  sum(split(lines: lines, refund: refund), s => s.share) = refund
```

A property is written like a function, with parameters that declare their ranges. It reads: for
every input in those ranges, the last line is `true`. A `when` line narrows the inputs it speaks of:

```kn
property no_share_is_more_than_its_left(lines: list[Line] count >=1 <=10, refund: money[USDc] range >=0USDc <=10_000_000USDc)
  description "A refund no larger than what is left in all gives no line more than what is left on it"
  when refund <= sum(lines, l => l.left)
  all(split(lines: lines, refund: refund), s => s.share <= s.left)
```

A property is not called, and does not appear in any generated code. It is total, like a function
written `total`: a body that can fail is E401, because a failure could be read as "false" and as
"does not apply". A parameter cannot be a function (E605).

## What `check` says

`kaname check` and `ritsu check` run every property on the vectors of its parameters, the inputs
`kaname vectors` chooses, and call rules and dates for real, as `examples` do. A property that is
false on one of them is an error that names the input:

```console
$ cd crates/kaname
$ ritsu kaname check tests/properties/shares.kn
error[E603]: tests/properties/shares.kn:12:1: The property `every_line_gets_a_share` is false for lines: [Line { sku: "a", left: 500000USDc }, Line { sku: "a", left: 500000USDc }], refund: 1USDc
    12 | property every_line_gets_a_share(lines: list[Line] count >=1 <=10, refund: money[USDc] range >=0USDc <=10_000_000USDc)
  = Line 15: `shares` is [0USDc, 1USDc]
  = `check` runs a property in the reference interpreter on the vectors of its parameters: on this one the `when` lines hold and the last line is false.
  = Correct the function, narrow the property with `when` or the ranges of its parameters, or correct the property.
```

A refund of one cent over two lines gives the first line nothing. Either the function is wrong or
the property is, and which is for a person to say.

A property with no counterexample is **not shown** by that: the vectors are a sample. `check` says so
with a warning, and why. A property whose `when` lines hold on none of the vectors is also said, since
it could be proved whatever its last line says:

```console
$ cd crates/kaname
$ ritsu kaname check tests/properties/thresholds.kn
warning[W603]: tests/properties/thresholds.kn:4:1: The property `past_the_threshold` held on all 3 of its 6 vectors on which its `when` lines are true, but it is not shown: there is no proof of it (thresholds.lean holds no theorem of it)
     4 | property past_the_threshold(x: number range >=0 <=100)
  = `ritsu gen --target lean` writes `samples/thresholds.lean`, the theorems' names with `sorry` for their proofs: put it beside the .kn as thresholds.lean, and write the proofs in place of `sorry`.
warning[W603]: tests/properties/thresholds.kn:9:1: The property `past_the_end` is not shown: there is no proof of it (thresholds.lean holds no theorem of it)
     9 | property past_the_end(x: number range >=0 <=100)
  = `ritsu gen --target lean` writes `samples/thresholds.lean`, the theorems' names with `sorry` for their proofs: put it beside the .kn as thresholds.lean, and write the proofs in place of `sorry`.
warning[W605]: tests/properties/thresholds.kn:9:1: The `when` lines of the property `past_the_end` hold on none of its 3 vectors
     9 | property past_the_end(x: number range >=0 <=100)
  = A property whose `when` lines hold for no input can be proved whatever it says: look again at its `when` lines and the ranges of its parameters.
ok tests/properties/thresholds.kn: 0 functions
```

A property has one of three answers:

| Answer | When | What `check` prints |
|---|---|---|
| counterexample | the `when` lines hold and the last line is false on a vector | E603, with the input |
| shown | `ritsu check --lean` took a proof of it | nothing |
| not decided | no counterexample, and not shown | W603 (or W604) with the reason |

W604 is a property that uses what is not yet written for Lean: dicts and sets, the builtins of
strings other than `=`, `sort_by`, `distinct`, `max_by`, `min_by`, `to_dict`, `group_by`, and a type
parameter held to keys. It is still tried on its vectors. A property that rests on what a rule or a
dates file answers is W603 too, with that as the reason: a proof may use only the facts the rule's
port tells.

## The statement, in Lean

`ritsu gen --target lean` writes each function as a definition in Lean 4, and each property as the
statement of a theorem, beside the other targets:

```console
$ cd crates/kaname/examples/refund_split
$ ritsu gen --root . --out generated --target lean
generated: generated/lean/lean-toolchain
generated: generated/lean/lakefile.toml
generated: generated/lean/.gitignore
generated: generated/lean/Kaname/Prelude.lean
generated: generated/lean/Kaname/refund_split_ja.lean
generated: generated/lean/Kaname/refund_split.lean
generated: generated/lean/samples/refund_split_ja.lean
generated: generated/lean/samples/refund_split.lean
```

`Kaname/refund_split.lean` holds the definitions and one statement for each property. An `Int`
counts amounts in the unit of their type, and `Within` is the range of a record's fields:

```lean
abbrev shares_add_up : Prop :=
  ∀ (lines : List Line) (refund : Int),
    1 ≤ lines.length → lines.length ≤ 10 → (∀ e₁ ∈ lines, e₁.Within) →
    0 ≤ refund → refund ≤ 10000000 →
    ((List.map (fun (s : Share) => s.share) (split lines refund)).sum) = refund
```

`samples/refund_split.lean` is a draft: the name of every theorem, and `sorry` for each proof.

## The proof

The proofs go in `refund_split.lean`, beside the `.kn`. Copy the draft there without its first three
lines, and put a proof in place of each `sorry`. A theorem is named `Proofs.refund_split.<property>`
and its type is the statement itself. Lean's own `omega`, `simp`, `decide` and `grind` do the work,
with the lemmas of `Kaname/Prelude.lean` for sums, `allocate` and `running_totals`. Mathlib is not
used, so `ring`, `linarith` and `set` are not there: `omega` does what `linarith` would, and the first
proof below names a sum with `obtain` where `set` would. To try a proof as it is written, run
`lake build` in `generated/lean/`. The Lean kaname writes builds with no warning, so what it warns of
is in the proof. `--lean` needs no such build: it writes the package again from the `.kn`.

The proof file of the example, whole:

```lean
import Kaname.refund_split

namespace Proofs.refund_split

open Kaname Kaname.refund_split

/-- The shares, each the refund up to its line's end less the refund before it, telescope to the
refund of the whole. -/
theorem shares_add_up : Kaname.refund_split.shares_add_up := by
  intro lines refund hlen _ hwithin _ _
  obtain ⟨S, hS⟩ : ∃ S, S = (lines.map fun l => l.left).sum := ⟨_, rfl⟩
  have hT : 0 < S := hS ▸ sum_pos hlen fun l hl => (hwithin l hl).1
  have key : (split lines refund).map (fun s => s.share)
      = (runningTotals lines (fun l => l.left)).map (fun r => allocate refund r.upto S - allocate refund r.before S) := by
    unfold split
    rw [List.map_map]
    apply List.map_congr_left
    intro r hr
    simp only [Function.comp, runningTotals_total _ _ r hr, hS]
  rw [key, runningTotals_telescope lines (fun l => l.left) (fun x => allocate refund x S), ← hS]
  rw [allocate_self refund hT, allocate_zero]
  omega

/-- A share is the refund up to its line's end less the refund before it, and `allocate` does not go
down as its share grows. -/
theorem no_share_is_negative : Kaname.refund_split.no_share_is_negative := by
  intro lines refund hlen _ hwithin hr0 _ s hs
  have hT : 0 < (lines.map fun l => l.left).sum := sum_pos hlen fun l hl => (hwithin l hl).1
  unfold split at hs
  rw [List.mem_map] at hs
  obtain ⟨r, hr, rfl⟩ := hs
  have hw : ∀ l ∈ lines, 0 ≤ (fun (l : Line) => l.left) l := fun l hl => by
    have := (hwithin l hl).1
    show 0 ≤ l.left
    omega
  have ⟨hwide, _, _⟩ := mem_runningTotals hw r hr
  have htot := runningTotals_total _ _ r hr
  have hmono := allocate_mono (t := refund) hr0 (htot ▸ hT) (show r.before ≤ r.upto by
    have hitem : r.item ∈ lines := by
      have := List.mem_map_of_mem (f := (·.item)) hr
      rwa [runningTotals_items] at this
    have := (hwithin r.item hitem).1
    omega)
  show allocate refund r.upto r.total - allocate refund r.before r.total ≥ 0
  omega

/-- What one line gets is at most its width of the whole, when the refund is no more than the whole. -/
theorem no_share_is_more_than_its_left : Kaname.refund_split.no_share_is_more_than_its_left := by
  intro lines refund hlen _ hwithin hr0 _ hle s hs
  have hT : 0 < (lines.map fun l => l.left).sum := sum_pos hlen fun l hl => (hwithin l hl).1
  unfold split at hs
  rw [List.mem_map] at hs
  obtain ⟨r, hr, rfl⟩ := hs
  have hw : ∀ l ∈ lines, 0 ≤ (fun (l : Line) => l.left) l := fun l hl => by
    have := (hwithin l hl).1
    show 0 ≤ l.left
    omega
  have ⟨hwide, _, _⟩ := mem_runningTotals hw r hr
  have htot := runningTotals_total _ _ r hr
  have hitem : r.item ∈ lines := by
    have := List.mem_map_of_mem (f := (·.item)) hr
    rwa [runningTotals_items] at this
  have := (hwithin r.item hitem).1
  have hsub := allocate_sub_le (t := refund) hr0 (htot ▸ hle) (htot ▸ hT) (show r.before ≤ r.upto by omega)
  show allocate refund r.upto r.total - allocate refund r.before r.total ≤ r.item.left
  omega

end Proofs.refund_split
```

`ritsu check --lean` is what says a property is shown. It builds the Lean package again from the
`.kn`, so that nothing in `generated/lean/` counts, reads the proof file as text, has Lean build it,
passes the proofs through the kernel again, and then checks for each property that the theorem is
there, that its type is the statement `ritsu gen` wrote, and that it rests on no axiom but Lean's
three (`propext`, `Classical.choice`, `Quot.sound`). A `sorry`, an axiom of the proof's own, or
`native_decide` is E604.

With every proof in place, `--lean` gives no W603:

```console
$ cd crates/kaname
$ ritsu check --lean examples/refund_split
ok examples/refund_split/refund_split.ja.kn: 1 function, total
ok examples/refund_split/refund_split.kn: 1 function, total
ritsu check: 2 files (kaname 2): all pass; borders between the languages: 0 checked, 0 undecided
```

Left as `sorry`, the proof of `no_share_is_negative` is E604: it rests on `sorryAx`, besides
`propext`, `Classical.choice` and `Quot.sound`.

The proof file is read as text before anything is built, because a proof written by someone else and
built in CI would run what it holds. It cannot import from outside `Kaname`, run code (`#eval`,
`run_cmd`), add syntax (`macro`, `syntax`, `elab`), use `native_decide`, or set options and attributes
that switch the checks off.

## Lean

Only `--lean` runs Lean. Install [elan](https://github.com/leanprover/elan), Lean's version manager;
the first `lake` it runs downloads the toolchain the proofs use, `leanprover/lean4:v4.34.0`, once. It
is about 2.7 GB with its library. Without `lake`, `--lean` stops with exit code 2 and says why.

kaname is not yet in the playground on this site. When it is, a property will run there on its
vectors without Lean, and say W603: a proof is checked by `--lean` on your machine.

The reference has [the rest](https://github.com/i2y/ritsu/blob/main/crates/kaname/docs/reference.md#properties):
the names, what is not written for Lean, the proof file's limits and the exit codes. An agent can
write the proofs with [the skill](https://github.com/i2y/ritsu/blob/main/skills/kaname/SKILL.md).
