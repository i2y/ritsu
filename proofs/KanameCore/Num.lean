/-
  Numbers as kaname holds them (kaname's DESIGN 3.3, 5.6, 8): an exact rational counted in the
  unit of its expression's type, held to the rule of int64, and rounded one of rulec's five ways.

  The rule of int64: a value, as a reduced fraction, has a numerator from −2⁶³ to 2⁶³ − 1 and a
  denominator up to 2⁶³ − 1 (`fits`). Lean's `Rat` is always reduced, as ritsu-units' `Rat` is, so
  `fits` reads the same numerator and denominator kaname's `num::fits` reads.

  The rounding is the one integer procedure of DESIGN 3.3, which the reference interpreter and the
  generated code follow (`roundInt`): the value `n/d` on the step `gn/gd` is `|n·gd|` divided by
  `d·gn`, the quotient taken or one past it by the way of rounding, the sign put back and the step
  multiplied in. `roundQ` says the same of rationals, with `floor` and `ceil`; `Round.lean` proves
  that the two agree and what any of the five ways gives (rulec's `roundsTo`).
-/
import RulecCert.Carried

namespace KanameCore

/-- How deep the functions over nested values go: far deeper than any value of a module nests (a
    record holds a list of records, and so on). Past it, `join` says anything and `has` says no. -/
def D : Nat := 1000

/-- The smallest and largest numerators the rule of int64 lets in. -/
def i64min : Int := -9223372036854775808
def i64max : Int := 9223372036854775807

/-- Whether a value keeps the rule of int64 (DESIGN 5.6). -/
def fits (r : Rat) : Bool :=
  decide (i64min ≤ r.num) && decide (r.num ≤ i64max) && decide ((r.den : Int) ≤ i64max)

/-- Why an operation has no answer: a value that leaves int64, a division by 0. -/
inductive Arith where
  | overflow
  | zero
  deriving DecidableEq, Repr

/-- A value held to the rule of int64. -/
def held (r : Rat) : Except Arith Rat :=
  if fits r then .ok r else .error .overflow

def addN (a b : Rat) : Except Arith Rat := held (a + b)
def subN (a b : Rat) : Except Arith Rat := held (a - b)
def mulN (a b : Rat) : Except Arith Rat := held (a * b)
def divN (a b : Rat) : Except Arith Rat :=
  if b = 0 then .error .zero else held (a / b)
def negN (a : Rat) : Except Arith Rat := held (-a)

/-- The five ways of rounding, rulec's. -/
inductive Way where
  /-- Away from 0. -/
  | up
  /-- Toward 0. -/
  | down
  /-- Half away from 0. -/
  | halfUp
  /-- Half toward 0. -/
  | halfDown
  /-- Half to the even neighbour. -/
  | halfEven
  deriving DecidableEq, Repr

/-- Whether the whole number `q` of steps goes one past, with the remainder `r` of `y`: what each
    way decides once the magnitude is divided. -/
def past (way : Way) (q r y : Nat) : Bool :=
  r != 0 &&
    match way with
    | .down => false
    | .up => true
    | .halfUp => decide (2 * r ≥ y)
    | .halfDown => decide (2 * r > y)
    | .halfEven => decide (2 * r > y) || (decide (2 * r = y) && q % 2 == 1)

/-- How many steps the integer procedure takes: the quotient `q` of the magnitude by the step, and
    one more where the way goes past it with the remainder `r` of `y`. -/
def stepsN (way : Way) (q r y : Nat) : Nat := q + if past way q r y then 1 else 0

/-- `x` rounded to a whole number of the positive step `g`, by the integer procedure of DESIGN 3.3
    (kaname's `num::round` before it holds the answer to int64). -/
def roundInt (way : Way) (x g : Rat) : Rat :=
  let top : Int := x.num * g.den
  let y : Nat := x.den * g.num.natAbs
  let k : Int := stepsN way (top.natAbs / y) (top.natAbs % y) y
  ((if top < 0 then -k else k : Int) : Rat) * g

/-- The magnitude of a value. -/
def mag (x : Rat) : Rat := if x < 0 then -x else x

/-- How many steps a magnitude `t` (in steps) rounds to, each way, said of rationals. -/
def stepsQ (way : Way) (t : Rat) : Int :=
  let f : Int := t.floor
  match way with
  | .down => f
  | .up => if t = f then f else f + 1
  | .halfUp => if t - f < 1/2 then f else f + 1
  | .halfDown => if t - f ≤ 1/2 then f else f + 1
  | .halfEven =>
    if t - f < 1/2 then f
    else if t - f > 1/2 then f + 1
    else if f % 2 = 0 then f else f + 1

/-- The same rounding, said of rationals: the magnitude in steps, rounded by the way, the sign put
    back. -/
def roundQ (way : Way) (x g : Rat) : Rat :=
  let k := stepsQ way (mag x / g)
  ((if x < 0 then -k else k : Int) : Rat) * g

/-- `round(x, g)` as the reference interpreter answers it: the procedure, held to int64. -/
def roundN (way : Way) (x g : Rat) : Except Arith Rat := held (roundInt way x g)

/-- `allocate(t, c, s)`: `t × c / s` rounded toward 0 to a whole unit, each value on the way
    held to int64 (rulec's §15.102). -/
def allocateN (t c s : Rat) : Except Arith Rat := do
  let p ← mulN t c
  let q ← divN p s
  roundN .down q 1

/-- `mod(x, n)`: the remainder of the whole part of `x` by the positive whole `n`. -/
def modN (x : Rat) (n : Int) : Rat := ((x.floor % n : Int) : Rat)

/-- `isqrt(x)`: the largest whole number whose square is not above `x`; none below 0. -/
def isqrtN (x : Rat) : Option Rat :=
  if x.num < 0 then none else some ((x.floor.toNat.sqrt : Nat) : Rat)

/-- Whether `v` is a whole number of the step `s` (a step of 0: `v` is 0). -/
def onStep (s v : Rat) : Bool :=
  if s = 0 then v == 0 else (v / s).den == 1

end KanameCore
