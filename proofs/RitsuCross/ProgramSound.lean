/-
  What X2 at a call of kaname settles (`Program.lean`): the relation stage (`kept_sound`, F.5 of
  kaname's PLAN) and the decision at the call (`x2k_holds`, `x2k_fails`, `x2k_undecided`).
-/
import RitsuCross.Program

namespace RitsuCross

/-- A relation that says `small < large` (or `<=`), held between the values: the comparison it
    says. -/
theorem says_sound {small large : String} {strict : Bool} {k : Kept} (hs : says small large strict k = true)
    {val : String → Int} {scale : String → Nat}
    (hr : relAt k.2.1 (val k.1) (scale k.1) (val k.2.2) (scale k.2.2) = true) :
    (if strict then decide (val small * (scale large : Int) < val large * (scale small : Int))
     else decide (val small * (scale large : Int) ≤ val large * (scale small : Int))) = true := by
  obtain ⟨a, r, b⟩ := k
  simp only [says] at hs
  simp only at hr
  by_cases h1 : r = "<"
  · subst h1
    simp only [ite_true, Bool.and_eq_true, beq_iff_eq] at hs
    obtain ⟨rfl, rfl⟩ := hs
    simp only [relAt, ite_true, Op.at, Op.test, decide_eq_true_eq] at hr
    cases strict
    · simp only [Bool.false_eq_true, ite_false, decide_eq_true_eq]; exact Int.le_of_lt hr
    · simp only [ite_true, decide_eq_true_eq]; exact hr
  · by_cases h2 : r = "<="
    · subst h2
      simp only [h1, ite_false, ite_true, Bool.and_eq_true, beq_iff_eq, Bool.not_eq_true'] at hs
      obtain ⟨⟨rfl, rfl⟩, rfl⟩ := hs
      simp only [relAt, h1, ite_false, ite_true, Op.at, Op.test, decide_eq_true_eq] at hr
      simp only [Bool.false_eq_true, ite_false, decide_eq_true_eq]; exact hr
    · by_cases h3 : r = "="
      · subst h3
        simp only [h1, h2, ite_false, ite_true, Bool.and_eq_true, Bool.or_eq_true, beq_iff_eq,
          Bool.not_eq_true'] at hs
        obtain ⟨hab, rfl⟩ := hs
        simp only [relAt, h1, h2, ite_false, ite_true, decide_eq_true_eq] at hr
        simp only [Bool.false_eq_true, ite_false, decide_eq_true_eq]
        rcases hab with ⟨rfl, rfl⟩ | ⟨rfl, rfl⟩
        · exact Int.le_of_eq hr
        · exact Int.le_of_eq hr.symm
      · simp [h1, h2, h3] at hs

/-- **Where the relations kaname gives say the precondition, it holds** for any values the call
    gives that keep the relations (F.5 of kaname's PLAN; the first stage of X2 at a call of kaname,
    kaname's DESIGN 14.3). -/
theorem kept_sound {rels : List Kept} {op : Op} {left right : String} (hk : kept rels op left right = true)
    {val : String → Int} {scale : String → Nat}
    (hr : ∀ a r b, (a, r, b) ∈ rels → relAt r (val a) (scale a) (val b) (scale b) = true) :
    op.at (val left) (scale left) (val right) (scale right) = true := by
  unfold kept at hk
  obtain ⟨⟨a, r, b⟩, hm, hs⟩ := List.any_eq_true.mp hk
  have h := says_sound hs (hr a r b hm)
  cases op <;> simp only [Op.sides, Op.at, Op.test, ite_true, Bool.false_eq_true, ite_false,
    decide_eq_true_eq] at h ⊢ <;> exact of_decide_eq_true h

/-- **What X2 says at a call of kaname when the precondition holds**: every pair of values the call
    can give that keeps the relations kaname gives and lies in the ranges kaname finds (the same value
    to both where kaname gives both the same, at one scale) keeps it. -/
theorem x2k_holds {op : Op} {left right : String} {rels : List Kept} {arg : String → Option Arg}
    {scale : String → Nat} {runs : List (Int × Int)} (h : x2k op left right rels arg scale runs = .holds)
    (val : String → Int)
    (hr : ∀ a r b, (a, r, b) ∈ rels → relAt r (val a) (scale a) (val b) (scale b) = true)
    (hg : ∀ l r, arg left = some l → arg right = some r →
      (l.shown = r.shown → scale left = scale right) ∧ Gives l r (val left) (val right)) :
    op.at (val left) (scale left) (val right) (scale right) = true := by
  unfold x2k at h
  split at h
  · rename_i hk; exact kept_sound hk hr
  · split at h
    · rename_i hx
      obtain ⟨l, r, hl, hr', hall⟩ := x2_holds hx
      obtain ⟨hs, hgv⟩ := hg l r hl hr'
      exact hall hs _ _ hgv
    · split at h <;> simp at h

/-- **What X2 says at a call of kaname when it gives an example**: no relation kaname gives said the
    precondition, and the example is one of the runs of the function's vectors, whose values break
    it. -/
theorem x2k_fails {op : Op} {left right : String} {rels : List Kept} {arg : String → Option Arg}
    {scale : String → Nat} {runs : List (Int × Int)} {e : Example} (h : x2k op left right rels arg scale runs = .fails e) :
    kept rels op left right = false ∧ ∃ a b, e = .at a b ∧ (a, b) ∈ runs ∧ op.at a (scale left) b (scale right) = false := by
  unfold x2k at h
  split at h
  · simp at h
  · rename_i hk
    refine ⟨by simpa using hk, ?_⟩
    split at h
    · simp at h
    · split at h
      · rename_i ab hab
        simp only [Answer.fails.injEq] at h
        subst h
        refine ⟨ab.1, ab.2, rfl, List.mem_of_find?_eq_some hab, ?_⟩
        have := List.find?_some hab
        simpa using this
      · simp at h

/-- **X2 at a call of kaname is undecided** only where no relation kaname gives says the
    precondition, the corner of kaname's ranges does not show it, and no run breaks it. -/
theorem x2k_undecided {op : Op} {left right : String} {rels : List Kept} {arg : String → Option Arg}
    {scale : String → Nat} {runs : List (Int × Int)} (h : x2k op left right rels arg scale runs = .undecided) :
    kept rels op left right = false ∧ (∀ e, x2 (.relation op left right) arg scale = e → e ≠ .holds) ∧
      ∀ ab ∈ runs, op.at ab.1 (scale left) ab.2 (scale right) = true := by
  unfold x2k at h
  split at h
  · simp at h
  · rename_i hk
    refine ⟨by simpa using hk, ?_, ?_⟩
    · intro e he hh
      subst hh
      rw [he] at h
      simp at h
    · split at h
      · simp at h
      · split at h
        · simp at h
        · rename_i hn
          intro ab hab
          have := List.find?_eq_none.mp hn ab hab
          simpa using this

end RitsuCross
