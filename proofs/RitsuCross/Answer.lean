/-
  What a check across a border comes to (ritsu's DESIGN 7.1, P5), and the ranges the checks
  compare: `ritsu_ports::Answer`, `ritsu_ports::Found`, and a range with both ends in.

  The checks of `ritsu-cross` say one of three things of a border: it holds for every value it is
  about, here is a value where it does not, or it cannot be decided. The models in this library
  are those decisions written again as functions; their theorems say what each of the first two
  answers settles. Why a check could not decide is prose in the Rust (in both languages) and is
  left out here: a model answers `undecided` where the Rust does, and says nothing of it.
-/

namespace RitsuCross

/-- What a check across a border comes to. -/
inductive Answer (ε : Type) where
  /-- Shown, for every value the check is about. -/
  | holds
  /-- A value where it does not hold. -/
  | fails (e : ε)
  /-- Not decided. -/
  | undecided
  deriving Repr, Inhabited

/-- What a question that asks for a value comes to: the value, or why it cannot be had. -/
inductive Found (α : Type) where
  | value (v : α)
  | undecided
  deriving Repr, Inhabited

/-- A range of integers with both ends in, `none` at an open end: a range dandori carries on the
    wire, the range a rule declares, the range of a koyomi input. -/
structure Range where
  lo : Option Int
  hi : Option Int
  deriving Repr, DecidableEq, Inhabited

/-- The value lies in the range. -/
def Range.mem (r : Range) (v : Int) : Prop :=
  (∀ a, r.lo = some a → a ≤ v) ∧ (∀ b, r.hi = some b → v ≤ b)

/-- The range has a value in it where both its ends are given: the low one is not above the high
    one. An end that is open leaves it nonempty. -/
def Range.Ok (r : Range) : Prop :=
  ∀ a b, r.lo = some a → r.hi = some b → a ≤ b

/-- A value below the low end or above the high one, as `borders.rs` tests it. -/
def Range.outside (r : Range) (v : Int) : Bool :=
  (match r.lo with
   | some a => decide (v < a)
   | none => false) ||
  (match r.hi with
   | some b => decide (b < v)
   | none => false)

theorem Range.mem_of_outside_false {r : Range} {v : Int} (h : r.outside v = false) : r.mem v := by
  unfold Range.outside at h
  simp only [Bool.or_eq_false_iff] at h
  obtain ⟨h1, h2⟩ := h
  refine ⟨fun a ha => ?_, fun b hb => ?_⟩
  · rw [ha] at h1
    simp only [decide_eq_false_iff_not, Int.not_lt] at h1
    exact h1
  · rw [hb] at h2
    simp only [decide_eq_false_iff_not, Int.not_lt] at h2
    exact h2

theorem Range.not_mem_of_outside {r : Range} {v : Int} (h : r.outside v = true) : ¬ r.mem v := by
  intro hm
  unfold Range.outside at h
  simp only [Bool.or_eq_true] at h
  rcases h with h | h
  · cases hlo : r.lo with
    | none => rw [hlo] at h; exact absurd h (by simp)
    | some a =>
      rw [hlo] at h
      have := hm.1 a hlo
      simp only [decide_eq_true_eq] at h
      omega
  · cases hhi : r.hi with
    | none => rw [hhi] at h; exact absurd h (by simp)
    | some b =>
      rw [hhi] at h
      have := hm.2 b hhi
      simp only [decide_eq_true_eq] at h
      omega

end RitsuCross
