/-
  `is monotonic` (koyomi's DESIGN 1.10): a later input day never gives an earlier date.

  koyomi checks it by computing the date on every day of the range and comparing each day with
  the next, and nothing else. That this settles the claim for every pair of days, not only for
  adjacent ones, is the transitivity of ≤ — which the design says in a sentence and this file
  proves, about the check as a function.
-/
import KoyomiModel.Ops

namespace KoyomiModel

/-- **The check.** `f` is the date as a function of the date input, every integer input held at
    one value, and `none` where it gives no date. The range is `lo`, `lo + 1`, …, `lo + n`; every
    day of it is compared with the next. -/
def adjacentOk (f : Int → Option Int) (lo : Int) (n : Nat) : Bool :=
  (List.range n).all (fun i =>
    match f (lo + i), f (lo + i + 1) with
    | some a, some b => decide (a ≤ b)
    | _, _ => false)

/-- One pair the check compared. -/
theorem adjacentOk_step {f : Int → Option Int} {lo : Int} {n : Nat} (h : adjacentOk f lo n = true)
    {i : Nat} (hi : i < n) : ∃ a b, f (lo + i) = some a ∧ f (lo + i + 1) = some b ∧ a ≤ b := by
  have hall := List.all_eq_true.1 h i (List.mem_range.2 hi)
  cases ha : f (lo + i) with
  | none => rw [ha] at hall; simp at hall
  | some a =>
    cases hb : f (lo + i + 1) with
    | none => rw [ha, hb] at hall; simp at hall
    | some b =>
      rw [ha, hb] at hall
      exact ⟨a, b, rfl, rfl, by simpa using hall⟩

/-- **Adjacent days are enough.** When every day of the range passes against the next, any two
    days of the range, the earlier one first, give dates in the same order. -/
theorem monotone_of_adjacentOk {f : Int → Option Int} {lo : Int} {n : Nat}
    (h : adjacentOk f lo n = true) :
    ∀ (i j : Nat) (a b : Int), i ≤ j → j ≤ n → f (lo + i) = some a → f (lo + j) = some b → a ≤ b := by
  intro i j
  induction j with
  | zero =>
    intro a b hij _ ha hb
    have : i = 0 := by omega
    subst this
    rw [ha] at hb
    cases hb
    exact Int.le_refl _
  | succ j ih =>
    intro a b hij hjn ha hb
    rcases Nat.lt_or_ge i (j + 1) with hlt | hge
    · obtain ⟨c, d, hc, hd, hcd⟩ := adjacentOk_step h (i := j) (by omega)
      have h1 : a ≤ c := ih a c (by omega) (by omega) ha hc
      have e : lo + ((j + 1 : Nat) : Int) = lo + (j : Int) + 1 := by omega
      rw [e, hd] at hb
      cases hb
      exact Int.le_trans h1 hcd
    · have : i = j + 1 := by omega
      subst this
      rw [ha] at hb
      cases hb
      exact Int.le_refl _

/-- The same over the whole range, as the claim reads: a day `d` of the range and a later day
    `d'` of it give dates `X(d) ≤ X(d')`. -/
theorem monotone_on_range {f : Int → Option Int} {lo : Int} {n : Nat}
    (h : adjacentOk f lo n = true) {d d' a b : Int} (hd : lo ≤ d) (hdd : d ≤ d') (hd' : d' ≤ lo + n)
    (ha : f d = some a) (hb : f d' = some b) : a ≤ b := by
  have e1 : d = lo + ((d - lo).toNat : Int) := by omega
  have e2 : d' = lo + ((d' - lo).toNat : Int) := by omega
  rw [e1] at ha
  rw [e2] at hb
  exact monotone_of_adjacentOk h _ _ a b (by omega) (by omega) ha hb

end KoyomiModel
