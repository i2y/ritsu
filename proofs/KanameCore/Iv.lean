/-
  Intervals (kaname's DESIGN 5.1, 5.2; `ranges.rs`, `Iv`): the values a number or a date can be,
  both ends in, an end none where the interval goes on, and the largest denominator its values have
  in lowest terms (1 for whole numbers; none where it is not known).

  Each operation is the one `ranges.rs` makes, written again: the arithmetic of the ends, the ends
  past `CAP` let go and those finer than `FINE` moved outward, the product of two ends with no end
  (`X`), the quotient over the values of a divisor that are not 0, rounding end by end. Rust works
  the ends in `i128` and lets an end go where a product leaves it; here the ends are exact, which
  says the same wherever the Rust stays in `i128` (the comparison holds it to that).
-/
import KanameCore.Num

namespace KanameCore

/-- An end of an interval: a value, or none where the interval goes on without end. -/
abbrev End := Option Rat

structure Iv where
  lo : End
  hi : End
  den : Option Nat
  deriving DecidableEq, Repr, Inhabited

/-- The magnitude past which an end is no end (`1 << 70`). -/
def CAP : Nat := 2 ^ 70
/-- The finest denominator an end keeps (`1 << 24`). -/
def FINE : Nat := 2 ^ 24

/-- The largest value on the grid of `1/d` at or below `v`, and the smallest at or above it. -/
def floorGrid (v : Rat) (d : Nat) : Rat := ((v * d).floor : Rat) / d
def ceilGrid (v : Rat) (d : Nat) : Rat := ((v * d).ceil : Rat) / d

/-- Whether the whole part of a value, toward 0, is past `CAP`. -/
def tooBig (v : Rat) : Bool := (v.num.tdiv v.den).natAbs > CAP

def normLo : End → End
  | none => none
  | some v => if tooBig v then none else some (if v.den > FINE then floorGrid v FINE else v)

def normHi : End → End
  | none => none
  | some v => if tooBig v then none else some (if v.den > FINE then ceilGrid v FINE else v)

def minLo : End → End → End
  | some x, some y => some (if x ≤ y then x else y)
  | _, _ => none

def maxHi : End → End → End
  | some x, some y => some (if y ≤ x then x else y)
  | _, _ => none

/-- The larger of two lower ends (a narrowing): an end that is none is the other. -/
def maxLo : End → End → End
  | some x, some y => some (if y ≤ x then x else y)
  | x, none => x
  | none, y => y

def minHi : End → End → End
  | some x, some y => some (if x ≤ y then x else y)
  | x, none => x
  | none, y => y

/-- An end in the arithmetic: below every value, a value, above every value. -/
inductive X where
  | neg
  | f (r : Rat)
  | pos
  deriving DecidableEq, Repr

namespace X

def sign : X → Int
  | neg => -1
  | pos => 1
  | f r => if r < 0 then -1 else if r > 0 then 1 else 0

def ofSign (s : Int) : X := if s < 0 then neg else if s > 0 then pos else f 0

def mul : X → X → X
  | f a, f b => f (a * b)
  | a, b => ofSign (a.sign * b.sign)

def le : X → X → Bool
  | neg, _ => true
  | _, pos => true
  | pos, _ => false
  | _, neg => false
  | f a, f b => decide (a ≤ b)

def toEnd : X → End
  | f r => some r
  | _ => none

end X

namespace Iv

def top : Iv := ⟨none, none, none⟩

def mk' (lo hi : End) (den : Option Nat) : Iv :=
  ⟨normLo lo, normHi hi, den.filter (· ≥ 1)⟩

def exact (v : Rat) : Iv := mk' (some v) (some v) (some v.den)

def ints (lo hi : Int) : Iv := mk' (some lo) (some hi) (some 1)

def withDen (i : Iv) (d : Option Nat) : Iv :=
  let den := match i.den, d with
    | some a, some b => some (min a b)
    | a, b => a.or b
  { i with den }

def hull (i o : Iv) : Iv :=
  let den := match i.den, o.den with
    | some a, some b => some (max a b)
    | _, _ => none
  ⟨minLo i.lo o.lo, maxHi i.hi o.hi, den⟩

/-- The values in both; none when there are none. -/
def meet (i o : Iv) : Option Iv :=
  let lo := maxLo i.lo o.lo
  let hi := minHi i.hi o.hi
  match lo, hi with
  | some a, some b => if b < a then none else some ⟨lo, hi, (i.withDen o.den).den⟩
  | _, _ => some ⟨lo, hi, (i.withDen o.den).den⟩

/-- The interval cut to a declared range. -/
def cut (i : Iv) (min max : End) : Iv := (i.meet ⟨min, max, none⟩).getD i

def isExact (i : Iv) : Option Rat :=
  match i.lo, i.hi with
  | some a, some b => if a = b then some a else none
  | _, _ => none

def hasZero (i : Iv) : Bool := (i.lo.all fun l => l.num ≤ 0) && (i.hi.all fun h => h.num ≥ 0)

/-- Whether a value is in the interval and keeps its bound on the denominator. -/
def has (i : Iv) (v : Rat) : Bool :=
  (i.lo.all fun l => l ≤ v) && (i.hi.all fun h => v ≤ h) && (i.den.all fun d => v.den ≤ d)

/-- Whether every value is within a declared range. -/
def within (i : Iv) (min max : End) : Bool :=
  (min.all fun m => i.lo.any fun l => m ≤ l) && (max.all fun m => i.hi.any fun h => h ≤ m)

/-- Whether every value keeps the rule of int64. -/
def fits (i : Iv) : Bool :=
  match i.lo, i.hi, i.den with
  | some lo, some hi, some d =>
    decide ((d : Int) ≤ i64max) &&
      (decide (lo.num ≥ 0) || decide ((i64min : Rat) ≤ lo * d)) &&
      (decide (hi.num ≤ 0) || decide (hi * d ≤ (i64max : Rat)))
  | _, _, _ => false

def xLo (i : Iv) : X := match i.lo with | some r => .f r | none => .neg
def xHi (i : Iv) : X := match i.hi with | some r => .f r | none => .pos

def ofX (lo hi : X) (den : Option Nat) : Iv := mk' lo.toEnd hi.toEnd den

def denMul : Option Nat → Option Nat → Option Nat
  | some a, some b => some (a * b)
  | _, _ => none

def plus (i o : Iv) : Iv :=
  let lo := match i.lo, o.lo with | some a, some b => some (a + b) | _, _ => none
  let hi := match i.hi, o.hi with | some a, some b => some (a + b) | _, _ => none
  mk' lo hi (denMul i.den o.den)

def negated (i : Iv) : Iv := mk' (i.hi.map (- ·)) (i.lo.map (- ·)) i.den

def minus (i o : Iv) : Iv := i.plus o.negated

/-- The lowest and the highest so far, and one more candidate. -/
def loHi (acc : X × X) (c : X) : X × X :=
  (if c.le acc.1 then c else acc.1, if acc.2.le c then c else acc.2)

/-- The four products of the ends, the lowest and the highest taken in the order Rust takes them. -/
def times (i o : Iv) : Iv :=
  let c1 := i.xLo.mul o.xLo
  let c2 := i.xLo.mul o.xHi
  let c3 := i.xHi.mul o.xLo
  let c4 := i.xHi.mul o.xHi
  let r := [c1, c2, c3, c4].foldl loHi (c1, c1)
  ofX r.1 r.2 (denMul i.den o.den)

/-- The reciprocals of an interval that holds no 0 (an end with no end, or 0, gives 0). -/
def recip (i : Iv) : Iv :=
  let r := fun (v : X) => match v with
    | .f x => if x.num ≠ 0 then X.f x⁻¹ else X.f 0
    | _ => X.f 0
  ofX (r i.xHi) (r i.xLo) none

/-- The largest magnitude a value has. -/
def magnitude (i : Iv) : Option Rat :=
  match i.lo, i.hi with
  | some lo, some hi =>
    let a := fun (r : Rat) => if r < 0 then -r else r
    some (if a hi ≤ a lo then a lo else a hi)
  | _, _ => none

/-- The bound on the denominator of a quotient: the dividend's, times one more than the largest
    numerator a value of the divisor can have. -/
def overDen (i o : Iv) : Option Nat :=
  match o.magnitude, o.den, i.den with
  | some m, some d, some a => some (a * ((m * d).floor.toNat + 1))
  | _, _, _ => none

/-- The quotients over the values of a divisor that holds 0, on either side of it: a value that is
    not 0 is at least `1/d` away from it. -/
def mergeParts : Option Iv → Option Iv → Option Iv
  | some a, some b => some (a.hull b)
  | some a, none => some a
  | none, b => b

def overSplit (i o : Iv) (d : Nat) : Option Iv :=
  let near : Rat := 1 / (d : Rat)
  let lower : Option Iv :=
    if o.lo.all (fun l => l.num < 0) then some (i.times (⟨o.lo, some (0 - near), o.den⟩ : Iv).recip) else none
  let upper : Option Iv :=
    if o.hi.all (fun h => h.num > 0) then some (i.times (⟨some near, o.hi, o.den⟩ : Iv).recip) else none
  mergeParts lower upper

/-- `i / o` over the values of `o` that are not 0: a value of `o` that is not 0 is at least `1/D`
    away from it, `D` its bound on the denominator. -/
def over (i o : Iv) : Iv :=
  if (o.lo.any fun l => l.num > 0) || (o.hi.any fun h => h.num < 0) then
    { i.times o.recip with den := overDen i o }
  else
    match o.den with
    | none => top
    | some d =>
      match overSplit i o d with
      | some q => { q with den := overDen i o }
      | none => ⟨none, none, overDen i o⟩

def scale (i : Iv) (f : Rat) : Iv := i.times (exact f)

/-- One end rounded: the end, or else a whole number beside it (below a lower end, above an upper
    one), rounded and held to int64. `ranges.rs` takes the whole part below for both ends; for an
    upper end that is no bound (rounding `-199/7` down to tenths gives `-28.4`, past a small
    stand-in for int64, while `-29` fits, and `-1201/42` rounds to `-28.5`, which fits too and is
    above it), so the model takes the whole part above. -/
def roundEnd (way : Way) (g : Rat) (upper : Bool) (x : Rat) : End :=
  let v := roundInt way x g
  if KanameCore.fits v then some v
  else
    let w := roundInt way ((if upper then x.ceil else x.floor : Int) : Rat) g
    if KanameCore.fits w then some w else none

/-- Rounded to the step `g` one of the five ways: each way is monotone, so the ends rounded are
    the ends of the values rounded. -/
def round (i : Iv) (way : Way) (g : Rat) : Iv :=
  let lo := i.lo.bind (roundEnd way g false)
  let hi := i.hi.bind (roundEnd way g true)
  mk' lo hi (some g.den)

/-- Whether every value of `i` is below every value of `o`. -/
def below (i o : Iv) : Bool :=
  match i.hi, o.lo with
  | some h, some l => decide (h < l)
  | _, _ => false

end Iv

end KanameCore
