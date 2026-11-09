/-!
The prelude of the Lean that kaname writes (kaname's DESIGN 38.4): the roundings of rulec's §3.3,
`allocate`, `isqrt`, `numbers`, the cuts of a list, the unreduced fractions a quotient by a value
is held as, the records `running_totals` and `indexed` answer, and the lemmas a proof of a property
leans on. Each definition does what kaname's reference interpreter does, by the same integer
procedure; the lemmas rest on `propext`, `Classical.choice` and `Quot.sound` alone.
-/

namespace Kaname

-- A result compares as its value does (a record that holds one compares so too).
deriving instance DecidableEq for Except

/-- The five ways of rounding (rulec's §3.3). -/
inductive Rounding where
  | up | down | half_up | half_down | half_even
  deriving Repr, DecidableEq

/-- `top / bottom` rounded to a whole number the way given, for a positive `bottom`: the whole
part of the magnitudes, one more where the remainder says so, and the sign of `top` put back. -/
def roundDiv (way : Rounding) (top bottom : Int) : Int :=
  let y := bottom.natAbs
  let q := top.natAbs / y
  let r := top.natAbs % y
  let past : Bool :=
    r != 0 &&
      match way with
      | .down => false
      | .up => true
      | .half_up => decide (y ≤ 2 * r)
      | .half_down => decide (y < 2 * r)
      | .half_even => decide (y < 2 * r) || (decide (2 * r = y) && q % 2 == 1)
  let k : Int := Int.ofNat (q + if past then 1 else 0)
  if top < 0 then -k else k

/-- `allocate(t, c, s)`: `t × c / s` rounded toward zero to a whole number (rulec's §15.102). -/
def allocate (t c s : Int) : Int := Int.tdiv (t * c) s

/-- `isqrt(x)`: the largest whole number whose square is not above `x`, for `x` not below zero. -/
def isqrt (x : Int) : Int := Int.ofNat (Nat.sqrt x.toNat)

/-- `numbers(a, b)`: `a` to `b`, both in; none where `b` is below `a`. -/
def numbers (a b : Int) : List Int := (List.range (b - a + 1).toNat).map fun (i : Nat) => a + (i : Int)

/-- `xs.at(i)`: the element at `i` from 0, none past either end. -/
def at? {α : Type} (xs : List α) (i : Int) : Option α := if i < 0 then none else xs[i.toNat]?

/-- `xs.slice(a, b)`: the elements from `a` up to `b`, each cut to the ends of the list. -/
def slice {α : Type} (xs : List α) (a b : Int) : List α :=
  let n : Int := xs.length
  let a' := (max 0 (min a n)).toNat
  let b' := (max 0 (min b n)).toNat
  (xs.drop a').take (b' - a')

/-- The first of the elements no later one is `better` than: `max_of` and `min_of`. -/
def best? {α : Type} (better : α → α → Bool) (xs : List α) : Option α :=
  xs.foldl (fun b x => match b with
    | none => some x
    | some y => if better x y then some x else some y) none

/-- A value of unknown step (a quotient by a value): `num / den`, unreduced, `den` above zero. -/
structure Frac where
  num : Int
  den : Int
  deriving Repr, DecidableEq, Inhabited

namespace Frac

def ofInt (n : Int) : Frac := ⟨n, 1⟩

/-- `n / d` for a `d` that is not zero, its sign put on `n`. -/
def mk' (n d : Int) : Frac := if d < 0 then ⟨-n, -d⟩ else ⟨n, d⟩

def add (x y : Frac) : Frac := ⟨x.num * y.den + y.num * x.den, x.den * y.den⟩
def sub (x y : Frac) : Frac := ⟨x.num * y.den - y.num * x.den, x.den * y.den⟩
def mul (x y : Frac) : Frac := ⟨x.num * y.num, x.den * y.den⟩
def neg (x : Frac) : Frac := ⟨-x.num, x.den⟩
def div (x y : Frac) : Frac := mk' (x.num * y.den) (x.den * y.num)
/-- `x` times `n / d`, for a positive `d`. -/
def scale (x : Frac) (n d : Int) : Frac := ⟨x.num * n, x.den * d⟩

def lt (x y : Frac) : Bool := decide (x.num * y.den < y.num * x.den)
def le (x y : Frac) : Bool := decide (x.num * y.den ≤ y.num * x.den)
def eq (x y : Frac) : Bool := x.num * y.den == y.num * x.den

/-- The whole number of `x`, where it is one. -/
def whole (x : Frac) : Int := x.num / x.den
/-- The greatest whole number not above `x`. -/
def floor (x : Frac) : Int := x.num / x.den
/-- `x` rounded the way given to a whole number of the step `d / n` (`n`, `d` positive). -/
def round (way : Rounding) (x : Frac) (n d : Int) : Int := roundDiv way (x.num * n) (x.den * d)

instance : Add Frac := ⟨add⟩
instance : Zero Frac := ⟨ofInt 0⟩

end Frac

/-- An element of what `running_totals` answers (kaname's DESIGN 3.6): the item, and the weights
before it, up to it and of the whole list. -/
structure Running (α : Type) where
  item : α
  before : Int
  upto : Int
  total : Int
  deriving Repr, DecidableEq, Inhabited

/-- The elements of `running_totals` from `before` on, of the whole list's weight `total`. -/
def runningFrom {α : Type} (w : α → Int) (total : Int) : List α → Int → List (Running α)
  | [], _ => []
  | x :: xs, before => ⟨x, before, before + w x, total⟩ :: runningFrom w total xs (before + w x)

/-- `running_totals(xs, w)`. -/
def runningTotals {α : Type} (xs : List α) (w : α → Int) : List (Running α) :=
  runningFrom w (xs.map w).sum xs 0

/-- An element of what `indexed` answers: the index from 0, and the item. -/
structure Indexed (α : Type) where
  index : Int
  item : α
  deriving Repr, DecidableEq, Inhabited

/-- `xs.indexed()`. -/
def indexed {α : Type} (xs : List α) : List (Indexed α) :=
  (xs.zipIdx).map fun p => ⟨Int.ofNat p.2, p.1⟩

/-! ## Lemmas -/

theorem allocate_self (t : Int) {s : Int} (h : 0 < s) : allocate t s s = t :=
  Int.mul_tdiv_cancel t (Int.ne_of_gt h)

theorem allocate_zero (t s : Int) : allocate t 0 s = 0 := by
  simp [allocate]

theorem allocate_mono {t c c' s : Int} (ht : 0 ≤ t) (hs : 0 < s) (hc : c ≤ c') :
    allocate t c s ≤ allocate t c' s :=
  Int.tdiv_le_tdiv hs (Int.mul_le_mul_of_nonneg_left hc ht)

/-- The remainder of `tdiv` keeps the sign of what is divided, and is smaller than the divisor. -/
private theorem tmod_bounds (a : Int) {s : Int} (hs : 0 < s) :
    (0 ≤ a → 0 ≤ a.tmod s ∧ a.tmod s < s) ∧ (a < 0 → a.tmod s ≤ 0 ∧ -s < a.tmod s) := by
  refine ⟨fun ha => ⟨Int.tmod_nonneg s ha, Int.tmod_lt_of_pos a hs⟩, fun ha => ⟨?_, Int.lt_tmod_of_pos a hs⟩⟩
  have h := Int.tmod_nonneg s (Int.neg_nonneg_of_nonpos (Int.le_of_lt ha))
  rw [Int.neg_tmod] at h
  omega

theorem allocate_sub_le {t c c' s : Int} (ht : 0 ≤ t) (hts : t ≤ s) (hs : 0 < s) (hc : c ≤ c') :
    allocate t c' s - allocate t c s ≤ c' - c := by
  unfold allocate
  have hA := Int.tmod_add_mul_tdiv (t * c) s
  have hB := Int.tmod_add_mul_tdiv (t * c') s
  have bA := tmod_bounds (t * c) hs
  have bB := tmod_bounds (t * c') hs
  have hle : t * c ≤ t * c' := Int.mul_le_mul_of_nonneg_left hc ht
  -- t (c' - c) ≤ s (c' - c)
  have hd : t * (c' - c) ≤ s * (c' - c) := Int.mul_le_mul_of_nonneg_right hts (by omega)
  -- the remainders differ by less than s
  have hr : (t * c).tmod s - (t * c').tmod s < s := by
    by_cases h1 : 0 ≤ t * c
    · have := (bA.1 h1); have := (bB.1 (Int.le_trans h1 hle)); omega
    · have h1' : t * c < 0 := by omega
      have := (bA.2 h1')
      by_cases h2 : 0 ≤ t * c'
      · have := (bB.1 h2); omega
      · have := (bB.2 (by omega)); omega
  have key : s * ((t * c').tdiv s - (t * c).tdiv s) < s * (c' - c + 1) := by
    rw [Int.mul_sub, Int.mul_add, Int.mul_one]
    rw [Int.mul_sub] at hd
    omega
  have := Int.lt_of_mul_lt_mul_left key (Int.le_of_lt hs)
  omega

theorem runningFrom_length {α : Type} (w : α → Int) (total : Int) :
    ∀ (xs : List α) (before : Int), (runningFrom w total xs before).length = xs.length
  | [], _ => rfl
  | _ :: xs, before => by simp [runningFrom, runningFrom_length w total xs]

theorem runningTotals_length {α : Type} (xs : List α) (w : α → Int) :
    (runningTotals xs w).length = xs.length :=
  runningFrom_length w _ xs 0

theorem runningFrom_items {α : Type} (w : α → Int) (total : Int) :
    ∀ (xs : List α) (before : Int), (runningFrom w total xs before).map (·.item) = xs
  | [], _ => rfl
  | _ :: xs, before => by simp [runningFrom, runningFrom_items w total xs]

theorem runningTotals_items {α : Type} (xs : List α) (w : α → Int) :
    (runningTotals xs w).map (·.item) = xs :=
  runningFrom_items w _ xs 0

theorem runningFrom_total {α : Type} (w : α → Int) (total : Int) :
    ∀ (xs : List α) (before : Int), ∀ r ∈ runningFrom w total xs before, r.total = total
  | [], _, r, hr => by simp [runningFrom] at hr
  | _ :: xs, _, r, hr => by
    simp only [runningFrom, List.mem_cons] at hr
    rcases hr with rfl | hr
    · rfl
    · exact runningFrom_total w total xs _ r hr

/-- Each element of what `running_totals` answers holds the weight of the whole list. -/
theorem runningTotals_total {α : Type} (xs : List α) (w : α → Int) :
    ∀ r ∈ runningTotals xs w, r.total = (xs.map w).sum :=
  runningFrom_total w _ xs 0

theorem runningFrom_telescope {α : Type} (w : α → Int) (total : Int) (f : Int → Int) :
    ∀ (xs : List α) (before : Int),
      ((runningFrom w total xs before).map fun r => f r.upto - f r.before).sum
        = f (before + (xs.map w).sum) - f before
  | [], before => by simp [runningFrom]
  | x :: xs, before => by
    simp only [runningFrom, List.map_cons, List.sum_cons]
    rw [runningFrom_telescope w total f xs (before + w x), Int.add_assoc]
    omega

/-- What `running_totals` answers telescopes: the differences of `f` at each element's ends add up
to `f` of the whole weight less `f 0`. -/
theorem runningTotals_telescope {α : Type} (xs : List α) (w : α → Int) (f : Int → Int) :
    ((runningTotals xs w).map fun r => f r.upto - f r.before).sum = f (xs.map w).sum - f 0 := by
  have := runningFrom_telescope w (xs.map w).sum f xs 0
  rw [Int.zero_add] at this
  exact this

private theorem sum_nonneg {α : Type} (w : α → Int) :
    ∀ xs : List α, (∀ x ∈ xs, 0 ≤ w x) → 0 ≤ (xs.map w).sum
  | [], _ => by simp
  | x :: xs, h => by
    simp only [List.map_cons, List.sum_cons]
    have h1 := h x (List.mem_cons_self ..)
    have h2 := sum_nonneg w xs (fun y hy => h y (List.mem_cons_of_mem _ hy))
    omega

theorem mem_runningFrom {α : Type} (w : α → Int) (total : Int) :
    ∀ (xs : List α) (before : Int), (∀ x ∈ xs, 0 ≤ w x) → 0 ≤ before →
      before + (xs.map w).sum ≤ total →
      ∀ r ∈ runningFrom w total xs before,
        r.upto - r.before = w r.item ∧ 0 ≤ r.before ∧ r.upto ≤ r.total
  | [], _, _, _, _, r, hr => by simp [runningFrom] at hr
  | x :: xs, before, hw, hb, ht, r, hr => by
    simp only [runningFrom, List.mem_cons] at hr
    simp only [List.map_cons, List.sum_cons] at ht
    have hx := hw x (List.mem_cons_self ..)
    have hrest : ∀ y ∈ xs, 0 ≤ w y := fun y hy => hw y (List.mem_cons_of_mem _ hy)
    have hs := sum_nonneg w xs hrest
    rcases hr with rfl | hr
    · simp only
      omega
    · exact mem_runningFrom w total xs (before + w x) hrest (by omega) (by omega) r hr

/-- With weights not below zero, each element of `running_totals` is its item's weight wide, and
lies between zero and the whole weight. -/
theorem mem_runningTotals {α : Type} {xs : List α} {w : α → Int} (hw : ∀ x ∈ xs, 0 ≤ w x) :
    ∀ r ∈ runningTotals xs w, r.upto - r.before = w r.item ∧ 0 ≤ r.before ∧ r.upto ≤ r.total :=
  mem_runningFrom w _ xs 0 hw (Int.le_refl 0) (by omega)

end Kaname
