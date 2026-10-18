/-
  X2 where a module of kaname calls a rule (ritsu's DESIGN 7.4; kaname's DESIGN 14.3;
  `crates/ritsu-cross/src/programs.rs`): the relation stage first, then the corner of the ranges
  kaname finds, then the runs of the function's vectors.

  With each call of a rule, kaname hands ritsu-cross the relations it knows hold there between two of
  the inputs it gives (`(input, "<" | "<=" | "=", input)`: a condition in force, the running totals of
  `running_totals`). A relation that says the precondition `left op right` shows it (`kept`): the same
  two inputs in its direction, `<` saying `<=` too, `=` saying `<=` and `>=`. Else the precondition is
  asked of rulec over kaname's ranges as for a workflow (`x2`); where that does not show it, the first
  run of the function's vectors that gives the two inputs values that break it is the example, and
  with none it is undecided (`x2k`).

  A wire integer `n` of an input at the scale `s` stands for `n / s` (`Relation.lean`): a relation
  between two inputs is between the values they stand for (`relAt`, as `Op.at` is).

  The theorems:

  - `kept_sound` (F.5 of kaname's PLAN): where the relations kaname gives say the precondition, it
    holds for any values the call gives that keep the relations.
  - `x2k_holds`: where the decision at a call of kaname holds, every pair of values the call can give
    that keeps the relations kaname gives and lies in the ranges kaname finds keeps the precondition.
  - `x2k_fails`: the example is one of the runs, and breaks the precondition; no relation said it.
  - `x2k_undecided`: undecided only where no relation, no corner and no run decides it.

  The theorems are in `ProgramSound.lean`; `KanameCore/Cross.lean` puts `kept_sound` together with
  what kaname's analysis shows of the relations it gives (`callRels_sound`).
-/
import RitsuCross.Relation

namespace RitsuCross

/-- A relation kaname keeps between two inputs of a call, as its port writes it: `(input, "<",
    input)`, `"<="` or `"="` (`ProgramRuleCall::relations`). -/
abbrev Kept := String × String × String

/-- The relation `r` (`"<"`, `"<="`, `"="`) between two wire integers, the first at the scale `sx`
    and the second at `sy`: between the values they stand for. -/
def relAt (r : String) (x : Int) (sx : Nat) (y : Int) (sy : Nat) : Bool :=
  if r = "<" then Op.lt.at x sx y sy
  else if r = "<=" then Op.le.at x sx y sy
  else if r = "=" then decide (x * (sy : Int) = y * (sx : Int))
  else false

/-- The two inputs of `left op right` in the direction of the comparison, the smaller first, and
    whether it is strict. -/
def Op.sides (op : Op) (left right : String) : String × String × Bool :=
  match op with
  | .lt => (left, right, true)
  | .le => (left, right, false)
  | .gt => (right, left, true)
  | .ge => (right, left, false)

/-- Whether a relation kaname keeps says `small < large` (strict) or `small <= large`: `<` says both,
    `<=` the second, `=` the second either way round. -/
def says (small large : String) (strict : Bool) : Kept → Bool
  | (a, r, b) =>
    if r = "<" then a == small && b == large
    else if r = "<=" then a == small && b == large && !strict
    else if r = "=" then ((a == small && b == large) || (a == large && b == small)) && !strict
    else false

/-- `programs.rs`'s `kept`: a relation kaname keeps at the call says the precondition. -/
def kept (rels : List Kept) (op : Op) (left right : String) : Bool :=
  rels.any (says (op.sides left right).1 (op.sides left right).2.1 (op.sides left right).2.2)

/-- X2 at one call of a module of kaname, for a relation between two inputs (`programs.rs`'s
    `precondition`): the relations kaname gives, then the corner of kaname's ranges as for a
    workflow, then the first of the runs (the values each gives the two inputs) that breaks it. -/
def x2k (op : Op) (left right : String) (rels : List Kept) (arg : String → Option Arg) (scale : String → Nat)
    (runs : List (Int × Int)) : Answer Example :=
  if kept rels op left right then .holds
  else match x2 (.relation op left right) arg scale with
    | .holds => .holds
    | _ => match runs.find? (fun ab => !op.at ab.1 (scale left) ab.2 (scale right)) with
      | some ab => .fails (.at ab.1 ab.2)
      | none => .undecided

end RitsuCross
