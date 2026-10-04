/-
  Linear inequalities, and the multipliers that refute a system of them (DESIGN §15.139,
  §15.141).

  A refutation is a list of inequalities, each with a multiplier at least zero, whose sum
  cancels every value and leaves a constant that is false — positive, or zero where a strict
  inequality took part. Checking one is addition. What this file proves is the one thing a
  checker needs from that addition: when it comes out false, no values satisfy all of the
  inequalities at once. Nothing is assumed about how the multipliers were found, which is the
  point of handing them over rather than the conclusion.

  The values are a list, numbered from zero, and an inequality's coefficients are a list in
  the same numbering. A value the list does not reach counts as zero, and so does a
  coefficient; the lemmas below hold whatever the two lengths are, so nothing downstream has
  to keep them equal.
-/

namespace RulecCert

/-! ## Two lists taken together -/

/-- `a · x`: the sum of the products of the two lists, position by position. What is left
    over when one is longer counts for nothing. -/
def dot : List Rat → List Rat → Rat
  | a :: as, x :: xs => a * x + dot as xs
  | _, _ => 0

/-- Two lists of coefficients added position by position; the longer one's tail stays. -/
def vadd : List Rat → List Rat → List Rat
  | a :: as, b :: bs => (a + b) :: vadd as bs
  | [], bs => bs
  | as, [] => as

/-- Every coefficient times the same number. -/
def vscale (y : Rat) (as : List Rat) : List Rat := as.map (y * ·)

theorem dot_vadd : ∀ (a b x : List Rat), dot (vadd a b) x = dot a x + dot b x
  | [], b, x => by simp only [vadd, dot]; grind
  | a :: as, [], x => by simp only [vadd, dot]; grind
  | a :: as, b :: bs, [] => by simp only [vadd, dot]; grind
  | a :: as, b :: bs, x :: xs => by
    simp only [vadd, dot, dot_vadd as bs xs]
    grind

theorem dot_vscale (y : Rat) : ∀ (a x : List Rat), dot (vscale y a) x = y * dot a x
  | [], x => by simp [vscale, dot]
  | a :: as, [] => by simp [vscale, dot]
  | a :: as, x :: xs => by
    have := dot_vscale y as xs
    simp only [vscale, List.map_cons, dot] at this ⊢
    rw [this]
    grind

theorem dot_zero : ∀ (a x : List Rat), (∀ c ∈ a, c = 0) → dot a x = 0
  | [], x, _ => by simp [dot]
  | a :: as, [], _ => by simp [dot]
  | a :: as, x :: xs, h => by
    simp only [dot]
    rw [h a (by simp), dot_zero as xs (fun c hc => h c (by simp [hc]))]
    grind

/-- The coefficients that pick out one value: `c` at position `i`, nothing elsewhere. -/
def unitAt (i : Nat) (c : Rat) : List Rat := List.replicate i 0 ++ [c]

theorem dot_unitAt : ∀ (i : Nat) (c : Rat) (v : List Rat), dot (unitAt i c) v = c * v.getD i 0
  | 0, c, [] => by simp [unitAt, dot]
  | 0, c, x :: xs => by simp only [unitAt, List.replicate, List.nil_append, dot]; simp; grind
  | i + 1, c, [] => by simp [unitAt, List.replicate, dot]
  | i + 1, c, x :: xs => by
    have := dot_unitAt i c xs
    simp only [unitAt, List.replicate, List.cons_append, dot] at this ⊢
    rw [this]
    simp
    grind

/-! ## One inequality -/

/-- One inequality over numbered values: `coeffs · v + k ≤ 0`, or `< 0` when strict. -/
structure LinIneq where
  coeffs : List Rat
  k : Rat
  strict : Bool
  deriving Repr, Inhabited

/-- The left-hand side at a list of values. -/
def LinIneq.lhs (q : LinIneq) (v : List Rat) : Rat := dot q.coeffs v + q.k

def LinIneq.holds (q : LinIneq) (v : List Rat) : Prop :=
  if q.strict then q.lhs v < 0 else q.lhs v ≤ 0

/-- The same as a test, for a checker that holds a point's values to it. -/
def LinIneq.holdsB (q : LinIneq) (v : List Rat) : Bool :=
  if q.strict then decide (q.lhs v < 0) else decide (q.lhs v ≤ 0)

theorem LinIneq.holds_of_holdsB {q : LinIneq} {v : List Rat} (h : q.holdsB v = true) : q.holds v := by
  unfold holdsB at h
  unfold holds
  cases hs : q.strict <;> simp only [hs, ite_true, ite_false, Bool.false_eq_true] at h ⊢ <;> simpa using h

theorem LinIneq.le_of_holds {q : LinIneq} {v : List Rat} (h : q.holds v) : q.lhs v ≤ 0 := by
  unfold holds at h
  cases hs : q.strict <;> simp only [hs] at h
  · exact h
  · exact Rat.le_of_lt h

/-! ## The sum a refutation makes -/

/-- The sum of multiplier × inequality: its coefficients, its constant, and whether a strict
    inequality took part with a multiplier above zero. -/
def combine : List (Rat × LinIneq) → List Rat × Rat × Bool
  | [] => ([], 0, false)
  | (y, q) :: rest =>
    let c := combine rest
    (vadd (vscale y q.coeffs) c.1, y * q.k + c.2.1, (q.strict && decide (0 < y)) || c.2.2)

/-- **The refutation check.** Every multiplier is at least zero, every coefficient of the sum
    is zero, and what is left is false: positive, or zero where a strict inequality took
    part. -/
def farkasOk (ps : List (Rat × LinIneq)) : Bool :=
  ps.all (fun p => decide (0 ≤ p.1)) &&
    (combine ps).1.all (fun x => decide (x = 0)) &&
    (decide (0 < (combine ps).2.1) || (decide ((combine ps).2.1 = 0) && (combine ps).2.2))

theorem mul_nonpos_of {y l : Rat} (hy : 0 ≤ y) (hl : l ≤ 0) : y * l ≤ 0 := by
  have h := Rat.mul_nonneg hy (show 0 ≤ -l by grind)
  rw [Rat.mul_neg] at h
  grind

theorem mul_neg_of {y l : Rat} (hy : 0 < y) (hl : l < 0) : y * l < 0 := by
  have h := Rat.mul_pos hy (show 0 < -l by grind)
  rw [Rat.mul_neg] at h
  grind

/-- Added up with their multipliers, inequalities that hold give a sum at most zero — below
    zero where a strict one took part. The sum is the combination's own left-hand side. -/
theorem combine_le (v : List Rat) : ∀ (ps : List (Rat × LinIneq)),
    (∀ p ∈ ps, 0 ≤ p.1) → (∀ p ∈ ps, p.2.holds v) →
    dot (combine ps).1 v + (combine ps).2.1 ≤ 0 ∧
      ((combine ps).2.2 = true → dot (combine ps).1 v + (combine ps).2.1 < 0)
  | [], _, _ => by simp only [combine, dot]; exact ⟨by grind, by simp⟩
  | (y, q) :: rest, hy, hh => by
    have ih := combine_le v rest (fun p hp => hy p (by simp [hp])) (fun p hp => hh p (by simp [hp]))
    have hy0 : 0 ≤ y := hy (y, q) (by simp)
    have hq : q.holds v := hh (y, q) (by simp)
    simp only [combine]
    rw [dot_vadd, dot_vscale]
    have hle := mul_nonpos_of hy0 (LinIneq.le_of_holds hq)
    unfold LinIneq.lhs at hle
    refine ⟨by grind, ?_⟩
    intro hs
    simp only [Bool.or_eq_true, Bool.and_eq_true, decide_eq_true_eq] at hs
    rcases hs with ⟨hst, hpos⟩ | hr
    · unfold LinIneq.holds at hq
      simp only [hst, ite_true] at hq
      have := mul_neg_of hpos hq
      unfold LinIneq.lhs at this
      grind
    · have := ih.2 hr
      grind

/-- **A refutation that checks leaves no values.** If `farkasOk` accepts the multipliers,
    there is no list of values at which every inequality holds. -/
theorem farkas_sound {ps : List (Rat × LinIneq)} (h : farkasOk ps = true) (v : List Rat)
    (hold : ∀ p ∈ ps, p.2.holds v) : False := by
  simp only [farkasOk, Bool.and_eq_true, Bool.or_eq_true, List.all_eq_true, decide_eq_true_eq] at h
  obtain ⟨⟨hy, hz⟩, hk⟩ := h
  obtain ⟨hle, hlt⟩ := combine_le v ps hy hold
  have h0 : dot (combine ps).1 v = 0 := dot_zero _ v hz
  rw [h0] at hle hlt
  rcases hk with hk | ⟨hk, hs⟩
  · grind
  · have := hlt hs
    grind

end RulecCert
