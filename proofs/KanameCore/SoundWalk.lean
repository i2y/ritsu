/-
  The builtins that walk a list (kaname's DESIGN 5.5, 15.2.2): `count`, `sum`, `any`, `all`, `map`,
  `filter`, `first`, `max_of`, `min_of` and `running_totals`.

  The function given is analysed once, on what every element can be; each element the evaluation
  applies it to is among those, so each answer is among what the analysis says it answers
  (`AnsOK`), and where a condition holds of an element, the element is among those the condition
  narrows it to. A sum of up to `n` values each in `w` is in `sums`; `running_totals` keeps, in
  each record, the relations of its running totals where no value is below 0 (DESIGN 5.3).
-/
import KanameCore.SoundCalls

namespace KanameCore

-- ── Sums ─────────────────────────────────────────────────────────────────────

/-- The sum of a list, from the left, as the evaluation adds it up. -/
def psum (acc : Rat) (qs : List Rat) : Rat := qs.foldl (· + ·) acc

theorem psum_ge {l : Rat} : ∀ (qs : List Rat) (acc : Rat), (∀ q ∈ qs, l ≤ q) → acc + (qs.length : Rat) * l ≤ psum acc qs
  | [], acc, _ => by simp only [psum, List.foldl_nil, List.length_nil]; grind
  | q :: qs, acc, h => by
    simp only [psum, List.foldl_cons, List.length_cons]
    have ih := psum_ge qs (acc + q) (fun q' hq => h q' (by simp [hq]))
    have hq := h q (by simp)
    unfold psum at ih
    push_cast
    grind

theorem psum_le {u : Rat} : ∀ (qs : List Rat) (acc : Rat), (∀ q ∈ qs, q ≤ u) → psum acc qs ≤ acc + (qs.length : Rat) * u
  | [], acc, _ => by simp only [psum, List.foldl_nil, List.length_nil]; grind
  | q :: qs, acc, h => by
    simp only [psum, List.foldl_cons, List.length_cons]
    have ih := psum_le qs (acc + q) (fun q' hq => h q' (by simp [hq]))
    have hq := h q (by simp)
    unfold psum at ih
    push_cast
    grind

theorem mul_le_of_le_neg {j c l : Rat} (hj : j ≤ c) (hl : l < 0) : c * l ≤ j * l := by
  have : 0 ≤ (c - j) * (-l) := Rat.mul_nonneg (by grind) (by grind)
  grind

theorem mul_le_of_le_pos {j c l : Rat} (hj : c ≤ j) (hl : 0 ≤ l) : c * l ≤ j * l := Rat.mul_le_mul_of_nonneg_right hj hl

theorem adjLo_ok {lo : End} {b : Bool} {S : Rat} (h : EndLe lo S) : EndLe (adjLo b lo) S := by
  cases lo with
  | none => trivial
  | some l =>
    simp only [EndLe] at h
    simp only [adjLo]
    split
    · rename_i hpos
      simp only [Bool.and_eq_true, decide_eq_true_eq] at hpos
      have : 0 < l := (num_pos_iff l).mp hpos.2
      exact Rat.le_trans (Rat.le_of_lt this) h
    · exact h

theorem adjHi_ok {hi : End} {b : Bool} {S : Rat} (h : EndGe hi S) : EndGe (adjHi b hi) S := by
  cases hi with
  | none => trivial
  | some u =>
    simp only [EndGe] at h
    simp only [adjHi]
    split
    · rename_i hneg
      simp only [Bool.and_eq_true, decide_eq_true_eq] at hneg
      have : u < 0 := (num_neg_iff u).mp hneg.2
      exact Rat.le_trans h (Rat.le_of_lt this)
    · exact h

theorem sumLo_ok {count w : Iv} {qs : List Rat} (hc : count.In (qs.length : Rat)) (hw : ∀ q ∈ qs, w.In q) :
    EndLe (sumLo count w) (psum 0 qs) := by
  have hj0 : (0 : Rat) ≤ (qs.length : Rat) := by exact_mod_cast Nat.zero_le _
  unfold sumLo
  split
  · rename_i l hl
    have hge : (qs.length : Rat) * l ≤ psum 0 qs := by
      have := psum_ge (l := l) qs 0 (fun q hq => by have := (hw q hq).lo; rw [hl] at this; exact this)
      grind
    split
    · rename_i hneg
      have hl0 : l < 0 := (num_neg_iff l).mp hneg
      rcases hch : count.hi with _ | hc'
      · trivial
      · have hjc : (qs.length : Rat) ≤ hc' := by have := hc.hi; rw [hch] at this; exact this
        exact Rat.le_trans (mul_le_of_le_neg hjc hl0) hge
    · rename_i hpos
      have hl0 : 0 ≤ l := by
        have : ¬ l < 0 := fun h => hpos ((num_neg_iff l).mpr h)
        exact Rat.not_lt.mp this
      rcases hcl : count.lo with _ | cl
      · simp only [Option.getD_none, EndLe]
        exact Rat.le_trans (by have := Rat.mul_nonneg hj0 hl0; grind) hge
      · have hcj : cl ≤ (qs.length : Rat) := by have := hc.lo; rw [hcl] at this; exact this
        simp only [Option.getD_some, EndLe]
        exact Rat.le_trans (mul_le_of_le_pos hcj hl0) hge
  · trivial

theorem sumHi_ok {count w : Iv} {qs : List Rat} (hc : count.In (qs.length : Rat)) (hw : ∀ q ∈ qs, w.In q) :
    EndGe (sumHi count w) (psum 0 qs) := by
  have hj0 : (0 : Rat) ≤ (qs.length : Rat) := by exact_mod_cast Nat.zero_le _
  unfold sumHi
  split
  · rename_i h hh
    have hle : psum 0 qs ≤ (qs.length : Rat) * h := by
      have := psum_le (u := h) qs 0 (fun q hq => by have := (hw q hq).hi; rw [hh] at this; exact this)
      grind
    split
    · rename_i hpos
      have h0 : 0 < h := (num_pos_iff h).mp hpos
      rcases hch : count.hi with _ | hc'
      · trivial
      · have hjc : (qs.length : Rat) ≤ hc' := by have := hc.hi; rw [hch] at this; exact this
        exact Rat.le_trans hle (mul_le_of_le_pos hjc (Rat.le_of_lt h0))
    · rename_i hnp
      have h0 : h ≤ 0 := by
        have : ¬ 0 < h := fun hh' => hnp ((num_pos_iff h).mpr hh')
        exact Rat.not_lt.mp this
      rcases hcl : count.lo with _ | cl
      · simp only [Option.getD_none, EndGe]
        exact Rat.le_trans hle (by have := Rat.mul_nonneg hj0 (show (0 : Rat) ≤ -h by grind); grind)
      · have hcj : cl ≤ (qs.length : Rat) := by have := hc.lo; rw [hcl] at this; exact this
        simp only [Option.getD_some, EndGe]
        refine Rat.le_trans hle ?_
        have : 0 ≤ ((qs.length : Rat) - cl) * (-h) := Rat.mul_nonneg (by grind) (by grind)
        grind
  · trivial

/-- **The sums of up to `count` values each in `w`.** -/
theorem sums_sound {count w : Iv} {den : Option Nat} {qs : List Rat}
    (hc : count.In (qs.length : Rat)) (hw : ∀ q ∈ qs, w.In q) (hd : DenLe den (psum 0 qs)) :
    (sums count w den).In (psum 0 qs) :=
  In_mk' (adjLo_ok (sumLo_ok hc hw)) (adjHi_ok (sumHi_ok hc hw)) hd


-- ── The list walked ─────────────────────────────────────────────────────────

theorem walked_sound {a : Abs} {xs : List Val} (h : Mem a (.list xs)) :
    (walked a).1.In (xs.length : Rat) ∧ ∀ x ∈ xs, Mem (walked a).2 x := by
  cases a with
  | never => exact absurd h Mem.not_never
  | list c e =>
    cases h with
    | list hc he => exact ⟨hc, he⟩
  | opt n y =>
    cases h with
    | optSome _ hy =>
      cases y with
      | list c e =>
        cases hy with
        | list hc he => exact ⟨hc, he⟩
      | _ =>
        simp only [walked]
        exact ⟨In_mk' (by simp [EndLe]; exact_mod_cast Nat.zero_le _) trivial (by simp [DenLe]), fun x _ => Mem.any x⟩
  | _ =>
    simp only [walked]
    exact ⟨In_mk' (by simp [EndLe]; exact_mod_cast Nat.zero_le _) trivial (by simp [DenLe]), fun x _ => Mem.any x⟩

theorem nil_of_noneAtAll {count : Iv} {elem : Abs} {xs : List Val} (hc : count.In (xs.length : Rat))
    (he : ∀ x ∈ xs, Mem elem x) (h : noneAtAll count elem = true) : xs = [] := by
  unfold noneAtAll at h
  simp only [Bool.or_eq_true] at h
  rcases h with h | h
  · rcases hch : count.hi with _ | u <;> rw [hch] at h <;> simp at h
    have hle := hc.hi; rw [hch] at hle
    have hu : u ≤ 0 := by
      have : ¬ 0 < u := fun hp => by have := (num_pos_iff u).mpr hp; omega
      exact Rat.not_lt.mp this
    have : (xs.length : Rat) ≤ 0 := Rat.le_trans hle hu
    have : xs.length = 0 := by
      have h0 : (xs.length : Rat) = 0 := Rat.le_antisymm this (by exact_mod_cast Nat.zero_le _)
      exact_mod_cast h0
    exact List.length_eq_zero_iff.mp this
  · cases elem with
    | never =>
      cases xs with
      | nil => rfl
      | cons y ys => exact absurd (he y (by simp)) Mem.not_never
    | _ => simp at h

theorem mayBeEmpty_nil {count : Iv} (hc : count.In ((([] : List Val).length : Nat) : Rat)) :
    (count.lo.all fun l => l.num ≤ 0) = true := by
  have hlo := hc.lo
  rcases hcl : count.lo with _ | l
  · rfl
  · rw [hcl] at hlo
    simp only [EndLe, List.length_nil] at hlo
    simp only [Option.all_some, decide_eq_true_eq]
    have h0 : ¬ 0 < l := fun hp => absurd (show l ≤ 0 by simpa using hlo) (Rat.not_le.mpr hp)
    have : ¬ 0 < l.num := fun hp => h0 ((num_pos_iff l).mp hp)
    omega

/-- What `evalEach` gives back: an answer for each element up to the one that decides the walk. -/
theorem evalEach_shape {C : Ctx} : ∀ (n dep : Nat) (ρ : Env) (x : String) (body : Expr) (c : Option Conv) (w : Walk)
    (xs as : List Val), evalEach C n dep ρ x body c w xs = .ok as →
    as.length ≤ xs.length ∧ ((∀ a ∈ as, stopAt w a = false) → as.length = xs.length) ∧ (xs ≠ [] → as ≠ [])
  | 0, _, _, _, _, _, _, _, _, h => by simp [evalEach] at h
  | n + 1, dep, ρ, x, body, c, w, [], as, h => by simp [evalEach] at h; subst h; simp
  | n + 1, dep, ρ, x, body, c, w, el :: rest, as, h => by
    simp only [evalEach] at h
    obtain ⟨a0, _, h⟩ := bind_ok h
    obtain ⟨a, _, h⟩ := bind_ok h
    split at h
    · rename_i hs
      cases h
      refine ⟨by simp, fun hall => ?_, fun _ => by simp⟩
      exact absurd hs (by simp [hall a (by simp)])
    · obtain ⟨as', has', h⟩ := bind_ok h
      cases h
      obtain ⟨h1, h2, _⟩ := evalEach_shape n dep ρ x body c w rest as' has'
      refine ⟨by simp; omega, fun hall => ?_, fun _ => by simp⟩
      simp only [List.length_cons]
      rw [h2 (fun b hb => hall b (by simp [hb]))]

-- ── A walk with a condition ─────────────────────────────────────────────────

theorem length_filter_le {α : Type} (p : α → Bool) (l : List α) : (l.filter p).length ≤ l.length :=
  List.length_filter_le p l

theorem count_true_eq {as : List Val} (h : ∀ a ∈ as, a = .bool true) : (as.filter isTrue).length = as.length := by
  rw [List.filter_eq_self.mpr]
  intro a ha; rw [h a ha]; rfl

theorem count_true_zero {as : List Val} (h : ∀ a ∈ as, isTrue a = false) : (as.filter isTrue).length = 0 := by
  rw [List.filter_eq_nil_iff.mpr]
  · rfl
  · intro a ha; simp [h a ha]

theorem isTrue_iff {a : Val} : isTrue a = true ↔ a = .bool true := by
  cases a <;> simp [isTrue]
  rename_i b; cases b <;> simp

/-- **A walk with a condition answers within what its analysis says.** -/
theorem condResult_sound {w : Walk} {count : Iv} {elem : Abs} {t f : Bool} {held : Abs}
    {xs as : List Val} {line : Nat} {ord : Order} {step : Option Rat} {v : Val}
    (hcond : w.isCondition = true) (hcnt : count.In (xs.length : Rat)) (helem : ∀ el ∈ xs, Mem elem el)
    (hshape : as.length ≤ xs.length ∧ ((∀ a ∈ as, stopAt w a = false) → as.length = xs.length) ∧ (xs ≠ [] → as ≠ []))
    (hbool : ∀ a ∈ as, ∃ b, a = .bool b)
    (ht : ∀ a ∈ as, a = .bool true → t = true) (hf : ∀ a ∈ as, a = .bool false → f = true)
    (hheld : ∀ el a, (el, a) ∈ xs.zip as → a = .bool true → Mem held el)
    (hv : fold w line ord step xs as = .ok v) : Mem (condResult w count elem t f held) v := by
  obtain ⟨hlen, hfull, hne⟩ := hshape
  have hnil : noneAtAll count elem = true → xs = [] := nil_of_noneAtAll hcnt helem
  have hasnil : xs = [] → as = [] := by intro h; subst h; exact List.length_eq_zero_iff.mp (by simpa using hlen)
  cases w with
  | count =>
    simp only [fold] at hv; cases hv
    simp only [condResult]
    split
    · rename_i hn
      have := hasnil (hnil hn); subst this
      exact Mem.num (by simpa using In_ints (lo := 0) (hi := 0) (v := 0) (by omega) (by omega))
    · rename_i hn
      split
      · rename_i htf
        simp only [Bool.and_eq_true, Bool.not_eq_true'] at htf
        have hall : ∀ a ∈ as, a = .bool true := by
          intro a ha
          obtain ⟨b, rfl⟩ := hbool a ha
          cases b
          · have := hf _ ha rfl; rw [htf.2] at this; simp at this
          · rfl
        rw [count_true_eq hall, hfull (fun a ha => by rw [hall a ha]; rfl)]
        exact Mem.num hcnt
      · split
        · rename_i _ hnt
          have : ∀ a ∈ as, isTrue a = false := by
            intro a ha
            cases h : isTrue a
            · rfl
            · have := ht a ha (isTrue_iff.mp h); simp_all
          rw [count_true_zero this]
          exact Mem.num (by simpa using In_ints (lo := 0) (hi := 0) (v := 0) (by omega) (by omega))
        · refine Mem.num (In_mk' ?_ ?_ ?_)
          · simp only [EndLe]; exact_mod_cast Nat.zero_le _
          · have h1 := hcnt.hi
            rcases hch : count.hi with _ | u
            · trivial
            · rw [hch] at h1
              simp only [EndGe]
              have : ((as.filter isTrue).length : Rat) ≤ (xs.length : Rat) := by
                exact_mod_cast Nat.le_trans (length_filter_le _ _) hlen
              exact Rat.le_trans this h1
          · simp [DenLe]
  | any =>
    simp only [fold] at hv; cases hv
    simp only [condResult]
    refine Mem_bools ?_ ?_
    · intro hb
      obtain ⟨a, ha, hta⟩ := List.any_eq_true.mp hb
      have hxs : xs ≠ [] := by intro h; rw [hasnil h] at ha; simp at ha
      have hn : noneAtAll count elem = false := by
        cases h : noneAtAll count elem
        · rfl
        · exact absurd (hnil h) hxs
      simp [hn, ht a ha (isTrue_iff.mp hta)]
    · intro hb
      by_cases hxs : xs = []
      · subst hxs
        simp [mayBeEmpty_nil hcnt]
      · have hn : noneAtAll count elem = false := by
          cases h : noneAtAll count elem
          · rfl
          · exact absurd (hnil h) hxs
        obtain ⟨a, rest, has⟩ := List.exists_cons_of_ne_nil (hne hxs)
        have ha : a ∈ as := by rw [has]; simp
        have hnt : isTrue a = false := by
          have := List.any_eq_false.mp (by simpa using hb) a ha
          simpa using this
        obtain ⟨b, rfl⟩ := hbool a ha
        cases b
        · simp [hn, hf _ ha rfl]
        · simp [isTrue] at hnt
  | all =>
    simp only [fold] at hv; cases hv
    simp only [condResult]
    refine Mem_bools ?_ ?_
    · intro hb
      by_cases hxs : xs = []
      · subst hxs; simp [mayBeEmpty_nil hcnt]
      · have hn : noneAtAll count elem = false := by
          cases h : noneAtAll count elem
          · rfl
          · exact absurd (hnil h) hxs
        obtain ⟨a, rest, has⟩ := List.exists_cons_of_ne_nil (hne hxs)
        have ha : a ∈ as := by rw [has]; simp
        have := List.all_eq_true.mp hb a ha
        simp [hn, ht a ha (isTrue_iff.mp this)]
    · intro hb
      have hb' : as.all isTrue = false := hb
      obtain ⟨a, ha, hnt⟩ : ∃ a ∈ as, isTrue a = false := by
        apply Classical.byContradiction
        intro hc
        have : as.all isTrue = true := by
          rw [List.all_eq_true]
          intro a ha
          cases h : isTrue a
          · exact absurd ⟨a, ha, h⟩ hc
          · rfl
        rw [this] at hb'; simp at hb'
      have hxs : xs ≠ [] := by intro h; rw [hasnil h] at ha; simp at ha
      have hn : noneAtAll count elem = false := by
        cases h : noneAtAll count elem
        · rfl
        · exact absurd (hnil h) hxs
      obtain ⟨b, rfl⟩ := hbool a ha
      cases b
      · simp [hn, hf _ ha rfl]
      · simp [isTrue] at hnt
  | filter =>
    simp only [fold] at hv; cases hv
    simp only [condResult]
    refine Mem.list ?_ ?_
    · refine In_mk' ?_ ?_ ?_
      · simp only [EndLe]; exact_mod_cast Nat.zero_le _
      · have h1 := hcnt.hi
        rcases hch : count.hi with _ | u
        · trivial
        · rw [hch] at h1
          simp only [EndGe]
          have : (((xs.zip as).filter (fun p => isTrue p.2)).map (·.1)).length ≤ xs.length := by
            simp only [List.length_map]
            exact Nat.le_trans (length_filter_le _ _) (by rw [List.length_zip]; exact Nat.min_le_left _ _)
          exact Rat.le_trans (by exact_mod_cast this) h1
      · simp [DenLe]
    · intro y hy
      obtain ⟨⟨y', a⟩, hp, rfl⟩ := List.mem_map.mp hy
      obtain ⟨hpz, hpt⟩ := List.mem_filter.mp hp
      exact hheld y' a hpz (isTrue_iff.mp hpt)
  | first =>
    simp only [fold] at hv; cases hv
    simp only [condResult]
    split
    · rename_i y a hfind
      have hm := List.mem_of_find?_eq_some hfind
      have hp := List.find?_some hfind
      have hmy := hheld y a hm (isTrue_iff.mp hp)
      by_cases hyn : y = .none
      · subst hyn; exact Mem.optNone
      · exact Mem.optSome hyn (Mem_stripOpt hmy hyn)
    · exact Mem.optNone
  | _ => simp [Walk.isCondition] at hcond


-- ── Sums the evaluation adds up ─────────────────────────────────────────────

theorem forM_cons' {α : Type} {f : α → R Unit} {x : α} {xs : List α} :
    (x :: xs).forM f = (f x >>= fun _ => xs.forM f) := rfl

theorem forM_ok {α : Type} {f : α → R Unit} : ∀ {l : List α}, l.forM f = .ok () → ∀ a ∈ l, f a = .ok ()
  | [], _, a, h => by simp at h
  | x :: xs, h, a, ha => by
    rw [forM_cons'] at h
    obtain ⟨u, hu, h⟩ := bind_ok h
    rcases List.mem_cons.mp ha with rfl | ha
    · exact hu
    · exact forM_ok h a ha

theorem forM_err {α : Type} {f : α → R Unit} {e : Stop} : ∀ {l : List α}, l.forM f = .error e → ∃ a ∈ l, f a = .error e
  | [], h => by cases h
  | x :: xs, h => by
    rw [forM_cons'] at h
    rcases bind_err h with h | ⟨u, _, h⟩
    · exact ⟨x, by simp, h⟩
    · obtain ⟨a, ha, h⟩ := forM_err h
      exact ⟨a, by simp [ha], h⟩

theorem stepped_err {s : Option Rat} {v : Val} {line : Nat} {e : Stop} (h : stepped s v line = .error e) :
    e = .failed .type line := by
  unfold stepped at h
  split at h
  · split at h
    · simp at h
    · simp [failK] at h; exact h.symm
  · simp at h

theorem onStep_add {s a b : Rat} (ha : onStep s a = true) (hb : onStep s b = true) : onStep s (a + b) = true := by
  by_cases hs : s = 0
  · subst hs
    simp only [onStep, ite_true, beq_iff_eq] at ha hb ⊢
    rw [ha, hb]; grind
  · obtain ⟨k, rfl⟩ := onStep_int hs ha
    obtain ⟨j, rfl⟩ := onStep_int hs hb
    unfold onStep
    simp only [hs, ite_false, beq_iff_eq]
    rw [show (k : Rat) * s + (j : Rat) * s = ((k + j : Int) : Rat) * s by push_cast; grind,
      Rat.mul_div_cancel hs]
    exact Rat.den_intCast _

theorem onStep_zero (s : Rat) : onStep s 0 = true := by
  by_cases hs : s = 0
  · simp [onStep, hs]
  · unfold onStep
    simp only [hs, ite_false, beq_iff_eq]
    rw [show (0 : Rat) / s = ((0 : Int) : Rat) by grind]
    exact Rat.den_intCast _

theorem onStep_psum {s : Rat} : ∀ (qs : List Rat) (acc : Rat), onStep s acc = true → (∀ q ∈ qs, onStep s q = true) →
    onStep s (psum acc qs) = true
  | [], acc, h, _ => h
  | q :: qs, acc, h, hq => by
    simp only [psum, List.foldl_cons]
    exact onStep_psum qs (acc + q) (onStep_add h (hq q (by simp))) (fun q' h' => hq q' (by simp [h']))

theorem denLe_stepDen {step : Option Rat} {v : Rat} (h : ∀ s, step = some s → onStep s v = true) :
    DenLe (stepDen step) v := by
  cases step with
  | none => trivial
  | some s =>
    have hv := h s rfl
    simp only [stepDen, DenLe]
    by_cases hs : s = 0
    · simp only [hs, ite_true]
      simp only [onStep, hs, ite_true, beq_iff_eq] at hv
      subst hv; simp
    · simp only [hs, ite_false]
      obtain ⟨k, rfl⟩ := onStep_int hs hv
      exact den_intCast_mul_le k s

/-- The answers a sum adds up, as numbers. -/
theorem nums_of_ok {line : Nat} : ∀ {as : List Val}, as.forM (fun a => do let _ ← numOf a line) = .ok () →
    ∃ qs : List Rat, as = qs.map .num
  | [], _ => ⟨[], rfl⟩
  | a :: as, h => by
    rw [forM_cons'] at h
    obtain ⟨u, hu, h⟩ := bind_ok h
    obtain ⟨q, hq, _⟩ := bind_ok hu
    cases a <;> simp [numOf, failK] at hq
    subst hq
    obtain ⟨qs, rfl⟩ := nums_of_ok h
    exact ⟨_ :: qs, rfl⟩

theorem sumN_ok {line : Nat} : ∀ (qs : List Rat) (acc total : Rat), sumN line acc (qs.map .num) = .ok total →
    total = psum acc qs ∧ (qs = [] ∨ fits total = true)
  | [], acc, total, h => by simp [sumN] at h; subst h; exact ⟨rfl, Or.inl rfl⟩
  | q :: qs, acc, total, h => by
    simp only [List.map_cons, sumN, asNum] at h
    obtain ⟨acc', ha, h⟩ := bind_ok h
    have ha' := liftA_ok ha
    simp only [addN] at ha'
    obtain ⟨rfl, hfit⟩ := held_ok ha'
    obtain ⟨h1, h2⟩ := sumN_ok qs _ total h
    refine ⟨by simp [psum, h1], Or.inr ?_⟩
    rcases h2 with rfl | h2
    · simp [psum] at h1; subst h1; exact hfit
    · exact h2

theorem sumN_err {line : Nat} : ∀ (qs : List Rat) (acc : Rat) (e : Stop), sumN line acc (qs.map .num) = .error e →
    e = .failed .overflow line ∧ ∃ j, j ≤ qs.length ∧ fits (psum acc (qs.take j)) = false
  | [], acc, e, h => by simp [sumN] at h
  | q :: qs, acc, e, h => by
    simp only [List.map_cons, sumN, asNum] at h
    rcases bind_err h with h | ⟨acc', ha, h⟩
    · rcases liftA_err h with ⟨h1, h2⟩ | ⟨h1, h2⟩
      · simp only [addN] at h1
        have := held_err h1
        refine ⟨h2, 1, by simp, ?_⟩
        simp [psum, this.2]
      · simp [addN, held] at h1; split at h1 <;> simp at h1
    · have ha' := liftA_ok ha
      simp only [addN] at ha'
      obtain ⟨rfl, _⟩ := held_ok ha'
      obtain ⟨h1, j, hj, hf⟩ := sumN_err qs _ e h
      refine ⟨h1, j + 1, by simp; omega, ?_⟩
      simpa [psum] using hf


theorem psum_append (acc : Rat) (P Q : List Rat) : psum acc (P ++ Q) = psum (psum acc P) Q := by
  simp [psum, List.foldl_append]

theorem psum_mono {acc : Rat} {qs : List Rat} (h : ∀ q ∈ qs, 0 ≤ q) : acc ≤ psum acc qs := by
  have := psum_ge (l := 0) qs acc h
  grind

theorem runningN_length {T : Rat} : ∀ (before : Rat) (pairs : List (Val × Val)),
    (runningN T before pairs).length = pairs.length
  | _, [] => rfl
  | before, (x, w) :: rest => by simp [runningN, runningN_length _ rest]

/-- The running totals: each record of an element, its total before, up to it, and of all. -/
theorem running_mem {elem : Abs} {wv count : Iv} {dn : Option Nat} {T : Rat} {qs : List Rat}
    (hcnt : count.In (qs.length : Rat)) (hw : ∀ q ∈ qs, wv.In q)
    (hden : ∀ P, (∃ R, P ++ R = qs) → DenLe dn (psum 0 P)) (hT : T = psum 0 qs) :
    ∀ (P R : List Rat) (xs' : List Val),
      P ++ R = qs → xs'.length = R.length → (∀ x ∈ xs', Mem elem x) →
      ∀ r ∈ runningN T (psum 0 P) (xs'.zip (R.map .num)), Mem (.record [("item", elem),
        ("before", .num (sums (Iv.mk' (some 0) (count.hi.map fun h => if h - 1 < 0 then (0 : Rat) else h - 1) (some 1)) wv dn)),
        ("upto", .num (sums (Iv.mk' (some 1) count.hi (some 1)) wv dn)),
        ("total", .num (sums count wv dn))]
        (if wv.lo.any (fun l => l.num ≥ 0) then [("before", .le, "upto"), ("upto", .le, "total")] else [])) r
  | P, [], xs', hPR, hlen, hel => by
    cases xs' with
    | nil => intro r hr; simp [runningN] at hr
    | cons _ _ => simp at hlen
  | P, q :: R, [], _, hlen, _ => by simp at hlen
  | P, q :: R, x :: xs', hPR, hlen, hel => by
    simp only [List.map_cons, List.zip_cons_cons, runningN, asNum]
    have hP' : psum 0 P + q = psum 0 (P ++ [q]) := by rw [psum_append]; rfl
    intro r hr
    rcases List.mem_cons.mp hr with rfl | hr
    · -- this element's record
      have hPq : (P ++ [q]) ++ R = qs := by rw [← hPR]; simp
      have hlenP : (P ++ [q]).length ≤ qs.length := by rw [← hPq]; simp
      have hwq : ∀ q' ∈ P ++ [q], wv.In q' := fun q' h' => hw q' (by rw [← hPq]; exact List.mem_append_left _ h')
      have hwP : ∀ q' ∈ P, wv.In q' := fun q' h' => hwq q' (by simp [h'])
      have hhi := hcnt.hi
      have hP0 : (0 : Rat) ≤ (P.length : Rat) := by exact_mod_cast Nat.zero_le _
      have hP1 : (P.length : Rat) + 1 ≤ (qs.length : Rat) := by
        have : P.length + 1 ≤ qs.length := by rw [← hPR]; simp
        exact_mod_cast this
      refine Mem.record ?_ ?_
      · intro n v hv a ha
        simp only [fieldV, List.find?] at hv
        simp only [Abs.fieldOf, List.find?] at ha
        by_cases h1 : n = "item"
        · subst h1; simp at hv ha; subst hv; subst ha; exact hel x (by simp)
        · by_cases h2 : n = "before"
          · subst h2; simp at hv ha; subst hv; subst ha
            refine Mem.num (sums_sound ?_ hwP (hden P ⟨q :: R, hPR⟩))
            refine In_mk' (by simp only [EndLe]; exact_mod_cast Nat.zero_le _) ?_
              (by show ((P.length : Nat) : Rat).den ≤ 1; rw [Rat.den_natCast]; exact Nat.le_refl 1)
            rcases hch : count.hi with _ | u
            · trivial
            · rw [hch] at hhi
              simp only [EndGe] at hhi
              simp only [EndGe, Option.map_some]
              by_cases hu : u - 1 < 0
              · simp only [hu, ite_true]
                exfalso; grind
              · simp only [hu, ite_false]; grind
          · by_cases h3 : n = "upto"
            · subst h3; simp at hv ha; subst hv; subst ha
              rw [hP']
              refine Mem.num (sums_sound ?_ hwq (hden (P ++ [q]) ⟨R, hPq⟩))
              refine In_mk' ?_ ?_ (by show (((P ++ [q]).length : Nat) : Rat).den ≤ 1; rw [Rat.den_natCast]; exact Nat.le_refl 1)
              · show (1 : Rat) ≤ ((P ++ [q]).length : Rat)
                exact_mod_cast (show 1 ≤ (P ++ [q]).length by simp)
              · rcases hch : count.hi with _ | u
                · trivial
                · rw [hch] at hhi
                  simp only [EndGe]
                  exact Rat.le_trans (by exact_mod_cast hlenP) hhi
            · by_cases h4 : n = "total"
              · subst h4; simp at hv ha; subst hv; subst ha
                rw [hT]
                exact Mem.num (sums_sound hcnt hw (hden qs ⟨[], by simp⟩))
              · have e1 : ("item" == n) = false := by simp only [beq_eq_false_iff_ne]; exact fun e => h1 e.symm
                have e2 : ("before" == n) = false := by simp only [beq_eq_false_iff_ne]; exact fun e => h2 e.symm
                have e3 : ("upto" == n) = false := by simp only [beq_eq_false_iff_ne]; exact fun e => h3 e.symm
                have e4 : ("total" == n) = false := by simp only [beq_eq_false_iff_ne]; exact fun e => h4 e.symm
                simp [e1, e2, e3, e4] at hv
      · -- the running totals keep their order where no value is below 0
        intro a rr b hm
        split at hm
        · rename_i hnn
          rcases hl : wv.lo with _ | l
          · rw [hl] at hnn; simp at hnn
          have hl0 : l.num ≥ 0 := by rw [hl] at hnn; simpa using hnn
          have hl0' : (0 : Rat) ≤ l := (@Rat.num_nonneg l).mp hl0
          have hnn' : ∀ q' ∈ qs, 0 ≤ q' := fun q' h' => by
            have := (hw q' h').lo; rw [hl] at this; exact Rat.le_trans hl0' this
          simp only [List.mem_cons, Prod.mk.injEq, List.not_mem_nil, or_false] at hm
          rcases hm with ⟨rfl, rfl, rfl⟩ | ⟨rfl, rfl, rfl⟩
          · refine ⟨.num (psum 0 P), .num (psum 0 P + q), by simp [fieldV], by simp [fieldV], ?_⟩
            simp only [RelVals, Rel.holds, decide_eq_true_eq]
            have := hnn' q (by rw [← hPR]; simp)
            grind
          · refine ⟨.num (psum 0 P + q), .num T, by simp [fieldV], by simp [fieldV], ?_⟩
            simp only [RelVals, Rel.holds, decide_eq_true_eq]
            rw [hP', hT, ← hPq, psum_append 0 (P ++ [q]) R]
            exact psum_mono (fun q' h' => hnn' q' (by rw [← hPq]; exact List.mem_append_right _ h'))
        · simp at hm
    · -- the records after it
      rw [hP'] at hr
      exact running_mem hcnt hw hden hT (P ++ [q]) R xs' (by rw [← hPR]; simp) (by simpa using hlen)
        (fun x' h' => hel x' (by simp [h'])) r hr


-- ── A walk with a function ──────────────────────────────────────────────────

theorem bestOf_mem {ord : Order} {isMax : Bool} : ∀ (best : Option Val) (l : List Val) (b : Val),
    bestOf ord isMax best l = some b → best = some b ∨ b ∈ l
  | best, [], b, h => Or.inl h
  | none, v :: vs, b, h => by
    simp only [bestOf] at h
    rcases bestOf_mem (some v) vs b h with h | h
    · cases h; exact Or.inr (by simp)
    · exact Or.inr (by simp [h])
  | some b0, v :: vs, b, h => by
    simp only [bestOf] at h
    rcases bestOf_mem _ vs b h with h | h
    · split at h
      · cases h; exact Or.inr (by simp)
      · exact Or.inl h
    · exact Or.inr (by simp [h])

theorem bestOf_none {ord : Order} {isMax : Bool} : ∀ (l : List Val), bestOf ord isMax none l = none → l = []
  | [], _ => rfl
  | v :: vs, h => by
    simp only [bestOf] at h
    rcases hb : bestOf ord isMax (some v) vs with _ | b
    · have := bestOf_mem (ord := ord) (isMax := isMax) (some v) vs
      -- a walk that starts from a value ends with one
      exfalso
      clear this
      have key : ∀ (best : Option Val) (l : List Val), best.isSome → (bestOf ord isMax best l).isSome := by
        intro best l
        induction l generalizing best with
        | nil => intro h; exact h
        | cons w ws ih =>
          intro h
          cases best with
          | none => simp at h
          | some b0 => simp only [bestOf]; apply ih; split <;> rfl
      have := key (some v) vs rfl
      rw [hb] at this; simp at this
    · rw [hb] at h; simp at h

theorem ordered_ok {line : Nat} : ∀ {as : List Val}, as.forM (fun a => ordered a line) = .ok () →
    ∀ a ∈ as, a ≠ .none := by
  intro as h a ha
  have := forM_ok h a ha
  intro e; subst e; simp [ordered, failK] at this

theorem stepped_num {step : Option Rat} {q : Rat} {line : Nat} (h : stepped step (.num q) line = .ok ()) :
    ∀ s, step = some s → onStep s q = true := fun s hs => stepOK_of_stepped h s q hs rfl

/-- **A walk with a function answers within what its analysis says**, and where it fails, the
    place of its sums says so. -/
theorem lamResult_sound {C : Ctx} {w : Walk} {o : Out} {id : Nat} {count : Iv}
    {elem va : Abs} {xs as : List Val} {line : Nat} {ord : Order} {step : Option Rat}
    (hlam : w.isCondition = false) (hcnt : count.In (xs.length : Rat)) (helem : ∀ el ∈ xs, Mem elem el)
    (hshape : as.length ≤ xs.length ∧ ((∀ a ∈ as, stopAt w a = false) → as.length = xs.length) ∧ (xs ≠ [] → as ≠ []))
    (hmem : ∀ a ∈ as, Mem va a) (hok : answersOK w line as = .ok ()) :
    (∀ v, fold w line ord step xs as = .ok v → Mem (lamResult o id w count elem va step).1 v) ∧
    (∀ k l, fold w line ord step xs as = .error (.failed k l) → k.ofEffect = true →
      Wit C (lamResult o id w count elem va step).2) := by
  obtain ⟨hlen, hfull, hne⟩ := hshape
  have hnostop : ∀ a ∈ as, stopAt w a = false := by
    intro a _; cases w <;> simp [Walk.isCondition] at hlam <;> rfl
  have hfl := hfull hnostop
  have hnil : noneAtAll count elem = true → xs = [] := nil_of_noneAtAll hcnt helem
  have hasnil : xs = [] → as = [] := by intro h; subst h; exact List.length_eq_zero_iff.mp (by simpa using hlen)
  -- what a sum adds up: numbers in what the function answers, each on the step
  have sumFacts : (as.forM fun a => do let _ ← numOf a line) = .ok () →
      ∃ qs : List Rat, as = qs.map .num ∧ (∀ q ∈ qs, (va.iv.getD Iv.top).In q) := by
    intro h
    obtain ⟨qs, rfl⟩ := nums_of_ok h
    refine ⟨qs, rfl, fun q hq => ?_⟩
    have hm := hmem (.num q) (List.mem_map_of_mem hq)
    rcases hiv : va.iv with _ | i
    · exact ⟨trivial, trivial, trivial⟩
    · exact Mem_iv_num hm hiv
  have hlenq : ∀ qs : List Rat, as = qs.map .num → qs.length = xs.length := by
    intro qs h; have := hfl; rw [h, List.length_map] at this; exact this
  have heveryIn : ∀ qs : List Rat, as = qs.map .num → (∀ q ∈ qs, (va.iv.getD Iv.top).In q) →
      (∀ s, step = some s → ∀ q ∈ qs, onStep s q = true) →
      ∀ j, j ≤ qs.length → (sums (Iv.mk' (some 0) count.hi (some 1)) (va.iv.getD Iv.top) (stepDen step)).In (psum 0 (qs.take j)) := by
    intro qs has hw hst j hj
    refine sums_sound ?_ (fun q hq => hw q (List.mem_of_mem_take hq)) ?_
    · refine In_mk' (by simp only [EndLe]; exact_mod_cast Nat.zero_le _) ?_ ?_
      · have h1 := hcnt.hi
        rcases hch : count.hi with _ | u
        · trivial
        · rw [hch] at h1
          simp only [EndGe]
          have : (qs.take j).length ≤ xs.length := by rw [← hlenq qs has]; simp; exact Nat.min_le_right _ _
          exact Rat.le_trans (by exact_mod_cast this) h1
      · show (((qs.take j).length : Nat) : Rat).den ≤ 1; rw [Rat.den_natCast]; exact Nat.le_refl 1
    · exact denLe_stepDen (fun s hs => onStep_psum _ 0 (onStep_zero s)
        (fun q hq => hst s hs q (List.mem_of_mem_take hq)))
  cases w with
  | map =>
    refine ⟨fun v hv => ?_, fun k l hv => by simp [fold] at hv⟩
    simp only [fold] at hv; cases hv
    simp only [lamResult]
    refine Mem.list (by rw [hfl]; exact hcnt) ?_
    intro a ha
    split
    · rename_i hn; rw [hasnil (hnil hn)] at ha; simp at ha
    · exact hmem a ha
  | maxOf =>
    refine ⟨fun v hv => ?_, fun k l hv => by simp [fold] at hv⟩
    simp only [fold] at hv; cases hv
    simp only [lamResult]
    have hord : ∀ a ∈ as, a ≠ .none := by
      simp only [answersOK] at hok; exact ordered_ok hok
    rcases hb : bestOf ord true none as with _ | b
    · have := bestOf_none as hb
      subst this
      have hx : xs = [] := List.length_eq_zero_iff.mp (by simpa using hfl.symm)
      subst hx
      simp only [Option.getD_none]
      rw [mayBeEmpty_nil hcnt]; exact Mem.optNone
    · rcases bestOf_mem none as b hb with h | h
      · cases h
      · simp only [Option.getD_some]
        refine Mem.optSome (hord b h) ?_
        split
        · rename_i hn; rw [hasnil (hnil hn)] at h; simp at h
        · exact hmem b h
  | minOf =>
    refine ⟨fun v hv => ?_, fun k l hv => by simp [fold] at hv⟩
    simp only [fold] at hv; cases hv
    simp only [lamResult]
    have hord : ∀ a ∈ as, a ≠ .none := by
      simp only [answersOK] at hok; exact ordered_ok hok
    rcases hb : bestOf ord false none as with _ | b
    · have := bestOf_none as hb
      subst this
      have hx : xs = [] := List.length_eq_zero_iff.mp (by simpa using hfl.symm)
      subst hx
      simp only [Option.getD_none]
      rw [mayBeEmpty_nil hcnt]; exact Mem.optNone
    · rcases bestOf_mem none as b hb with h | h
      · cases h
      · simp only [Option.getD_some]
        refine Mem.optSome (hord b h) ?_
        split
        · rename_i hn; rw [hasnil (hnil hn)] at h; simp at h
        · exact hmem b h
  | sum =>
    simp only [answersOK] at hok
    obtain ⟨qs, has, hw⟩ := sumFacts hok
    subst has
    simp only [lamResult]
    constructor
    · intro v hv
      simp only [fold] at hv
      obtain ⟨u, hst, hv⟩ := bind_ok hv
      obtain ⟨total, htot, hv⟩ := bind_ok hv
      cases hv
      have hst' : ∀ s, step = some s → ∀ q ∈ qs, onStep s q = true :=
        fun s hs q hq => stepped_num (forM_ok hst (.num q) (List.mem_map_of_mem hq)) s hs
      obtain ⟨rfl, hfit⟩ := sumN_ok qs 0 total htot
      split
      · rename_i hn
        have := hasnil (hnil hn)
        rw [List.map_eq_nil_iff] at this; subst this
        exact Mem.num (by simpa [psum] using In_ints (lo := 0) (hi := 0) (v := 0) (by omega) (by omega))
      · have hf : fits (psum 0 qs) = true := by
          rcases hfit with rfl | hf
          · simp [psum]; decide
          · exact hf
        refine heldA_mem (Mem.num ?_) hf
        refine sums_sound (by rw [hlenq qs rfl]; exact hcnt) hw ?_
        exact denLe_stepDen (fun s hs => onStep_psum qs 0 (onStep_zero s) (hst' s hs))
    · intro k l hv hk
      simp only [fold] at hv
      rcases bind_err hv with hv | ⟨u, hst, hv⟩
      · obtain ⟨a, _, ha⟩ := forM_err hv
        have h1 := stepped_err ha
        simp only [Stop.failed.injEq] at h1
        rw [h1.1] at hk; simp [Kind.ofEffect] at hk
      · have hst' : ∀ s, step = some s → ∀ q ∈ qs, onStep s q = true :=
          fun s hs q hq => stepped_num (forM_ok hst (.num q) (List.mem_map_of_mem hq)) s hs
        rcases bind_err hv with hv | ⟨total, _, hv⟩
        · obtain ⟨_, j, hj, hf⟩ := sumN_err qs 0 _ hv
          refine Wit.grow (wit_place (what := .overflow) (Mem.num (heveryIn qs rfl hw hst' j hj)) ⟨_, rfl, hf⟩) (grows_heldA (Grows.refl _))
        · simp at hv
  | running =>
    simp only [answersOK] at hok
    obtain ⟨qs, has, hw⟩ := sumFacts hok
    subst has
    simp only [lamResult]
    constructor
    · intro v hv
      simp only [fold] at hv
      obtain ⟨u, hst, hv⟩ := bind_ok hv
      obtain ⟨total, htot, hv⟩ := bind_ok hv
      cases hv
      have hst' : ∀ s, step = some s → ∀ q ∈ qs, onStep s q = true :=
        fun s hs q hq => stepped_num (forM_ok hst (.num q) (List.mem_map_of_mem hq)) s hs
      obtain ⟨rfl, _⟩ := sumN_ok qs 0 total htot
      refine Mem.list ?_ ?_
      · have : (runningN (psum 0 qs) 0 (xs.zip (qs.map .num))).length = xs.length := by
          rw [runningN_length, List.length_zip, List.length_map, ← hlenq qs rfl, Nat.min_self]
        rw [this]; exact hcnt
      · intro r hr
        split
        · rename_i hn
          have := hasnil (hnil hn)
          have hx := hnil hn
          subst hx
          simp [runningN] at hr
        · exact running_mem (by rw [hlenq qs rfl]; exact hcnt) hw
            (fun P _ => denLe_stepDen (fun s hs => onStep_psum P 0 (onStep_zero s) (fun q hq => by
              obtain ⟨R, hR⟩ := ‹∃ R, P ++ R = qs›
              exact hst' s hs q (by rw [← hR]; exact List.mem_append_left _ hq))))
            rfl [] qs xs (by simp) (by rw [hlenq qs rfl]) helem r (by simpa [psum] using hr)
    · intro k l hv hk
      simp only [fold] at hv
      rcases bind_err hv with hv | ⟨u, hst, hv⟩
      · obtain ⟨a, _, ha⟩ := forM_err hv
        have h1 := stepped_err ha
        simp only [Stop.failed.injEq] at h1
        rw [h1.1] at hk; simp [Kind.ofEffect] at hk
      · have hst' : ∀ s, step = some s → ∀ q ∈ qs, onStep s q = true :=
          fun s hs q hq => stepped_num (forM_ok hst (.num q) (List.mem_map_of_mem hq)) s hs
        rcases bind_err hv with hv | ⟨total, _, hv⟩
        · obtain ⟨_, j, hj, hf⟩ := sumN_err qs 0 _ hv
          exact wit_place (what := .overflow) (Mem.num (heveryIn qs rfl hw hst' j hj)) ⟨_, rfl, hf⟩
        · simp at hv
  | _ => simp [Walk.isCondition] at hlam


-- ── The function given to a walk ────────────────────────────────────────────

theorem coerce_bool {c : Option Conv} {v : Val} {b : Bool} (h : coerce c v = .ok (.bool b)) : v = .bool b := by
  cases c with
  | none => simpa [coerce] using h
  | some c =>
    cases v with
    | num r =>
      simp only [coerce, coerceV, Functor.map, Except.map] at h
      split at h <;> simp at h
    | list xs =>
      simp only [coerce, coerceV, Functor.map, Except.map] at h
      split at h <;> simp at h
    | bool b' => simp only [coerce, coerceV] at h; cases h; rfl
    | _ => simp [coerce, coerceV] at h

theorem mem_zip_left {xs as : List Val} {a : Val} (hlen : as.length ≤ xs.length) (ha : a ∈ as) :
    ∃ el, (el, a) ∈ xs.zip as := by
  obtain ⟨i, hi⟩ := List.mem_iff_getElem?.mp ha
  have hia : i < as.length := by
    rcases Nat.lt_or_ge i as.length with h | h
    · exact h
    · rw [List.getElem?_eq_none h] at hi; simp at hi
  have hix : i < xs.length := Nat.lt_of_lt_of_le hia hlen
  refine ⟨xs[i], List.mem_iff_getElem?.mpr ⟨i, ?_⟩⟩
  rw [List.getElem?_zip_eq_some]
  exact ⟨List.getElem?_eq_getElem hix, hi⟩

/-- **A condition given to a walk**: what it can come out as, and the elements where it holds. -/
theorem condBody {C : Ctx} {res : List (Option Abs)} {n : Nat} (IH : SoundAt C res n) {m : Nat} {A : ACtx}
    {ρ : Env} {x : String} {elem : Abs} {body : Expr} {c : Option Conv} {o : Out} {dep : Nat} {w : Walk}
    {xs : List Val} (hG : Good C res A) (hS : Sat A ρ) (helem : ∀ el ∈ xs, Mem elem el)
    (hwid : (condition A m x elem body c o).2.widened = false) :
    (∀ as, evalEach C n dep ρ x body c w xs = .ok as →
      (∀ a ∈ as, a = .bool true → (condition A m x elem body c o).1.1 = true) ∧
      (∀ a ∈ as, a = .bool false → (condition A m x elem body c o).1.2.1 = true) ∧
      (∀ el a, (el, a) ∈ xs.zip as → a = .bool true → Mem (condition A m x elem body c o).1.2.2 el)) ∧
    (∀ k l, evalEach C n dep ρ x body c w xs = .error (.failed k l) → k.ofEffect = true →
      Wit C (condition A m x elem body c o).2) := by
  cases m with
  | zero =>
    simp only [condition]
    exact ⟨fun as _ => ⟨fun _ _ _ => trivial, fun _ _ _ => trivial, fun el a h _ => helem el (List.of_mem_zip h).1⟩,
      fun _ _ _ _ => wit_fuel⟩
  | succ m =>
    simp only [condition] at hwid ⊢
    have hG' : Good C res (A.bindNew x elem) := hG
    have hwid' : (ev (A.bindNew x elem) m body o).out.widened = false :=
      grows_not_widened (grows_convA (Grows.refl _)) hwid
    obtain ⟨h1, h2⟩ := IH.each m A ρ x body c w xs elem o dep hG hS helem hwid'
    refine ⟨fun as has => ?_, fun k l h hk => h2 k l h hk⟩
    obtain ⟨hlen, _, _, hans⟩ := h1 as has
    refine ⟨?_, ?_, ?_⟩
    · intro a ha hat
      obtain ⟨el, hel⟩ := mem_zip_left hlen ha
      obtain ⟨raw, hm, _, _, hc⟩ := hans el a hel
      subst hat
      exact truths_true (convA_mem hm hc)
    · intro a ha haf
      obtain ⟨el, hel⟩ := mem_zip_left hlen ha
      obtain ⟨raw, hm, _, _, hc⟩ := hans el a hel
      subst haf
      exact truths_false (convA_mem hm hc)
    · intro el a hel hat
      obtain ⟨raw, hm, hsat, hnar, hc⟩ := hans el a hel
      subst hat
      have hraw := coerce_bool hc
      subst hraw
      have hS' := Sat.apply _ true hsat (hnar true rfl)
      cases hl : ((ev (A.bindNew x elem) m body o).nar.apply true (ev (A.bindNew x elem) m body o).ctx).lookup x with
      | none => simp only [Option.getD_none]; exact helem el (List.of_mem_zip hel).1
      | some h' =>
        simp only [Option.getD_some]
        obtain ⟨v, hv, hmv⟩ := hS'.env x h' hl
        rw [get_cons] at hv; simp at hv; subst hv; exact hmv

/-- **A function given to a walk**: each answer among what it answers. -/
theorem lamBody {C : Ctx} {res : List (Option Abs)} {n : Nat} (IH : SoundAt C res n) {m : Nat} {A : ACtx}
    {ρ : Env} {x : String} {elem : Abs} {body : Expr} {c : Option Conv} {o : Out} {dep : Nat} {w : Walk}
    {xs : List Val} (hG : Good C res A) (hS : Sat A ρ) (helem : ∀ el ∈ xs, Mem elem el)
    (hwid : (lam1 A m x elem body c o).2.widened = false) :
    (∀ as, evalEach C n dep ρ x body c w xs = .ok as → ∀ a ∈ as, Mem (lam1 A m x elem body c o).1 a) ∧
    (∀ k l, evalEach C n dep ρ x body c w xs = .error (.failed k l) → k.ofEffect = true →
      Wit C (lam1 A m x elem body c o).2) := by
  cases m with
  | zero =>
    simp only [lam1]
    exact ⟨fun _ _ a _ => Mem.any a, fun _ _ _ _ => wit_fuel⟩
  | succ m =>
    simp only [lam1] at hwid ⊢
    have hwid' : (ev (A.bindNew x elem) m body o).out.widened = false :=
      grows_not_widened (grows_convA (Grows.refl _)) hwid
    obtain ⟨h1, h2⟩ := IH.each m A ρ x body c w xs elem o dep hG hS helem hwid'
    refine ⟨fun as has a ha => ?_, fun k l h hk => h2 k l h hk⟩
    obtain ⟨hlen, _, _, hans⟩ := h1 as has
    obtain ⟨el, hel⟩ := mem_zip_left hlen ha
    obtain ⟨raw, hm, _, _, hc⟩ := hans el a hel
    exact convA_mem hm hc


theorem asBool_err {a : Val} {line : Nat} {e : Stop} (h : asBool a line = .error e) : e = .failed .type line := by
  cases a <;> simp [asBool, failK] at h <;> exact h.symm

theorem numOf_err {a : Val} {line : Nat} {e : Stop} (h : numOf a line = .error e) : e = .failed .type line := by
  cases a <;> simp [numOf, failK] at h <;> exact h.symm

theorem ordered_err {a : Val} {line : Nat} {e : Stop} (h : ordered a line = .error e) : e = .failed .type line := by
  cases a <;> simp [ordered, failK] at h <;> exact h.symm

theorem answersOK_err {w : Walk} {line : Nat} {as : List Val} {e : Stop} (h : answersOK w line as = .error e) :
    e = .failed .type line := by
  cases w with
  | count | any | all | filter | first =>
    simp only [answersOK] at h
    obtain ⟨a, _, ha⟩ := forM_err h
    rcases bind_err ha with ha | ⟨_, _, ha⟩
    · exact asBool_err ha
    · cases ha
  | maxOf | minOf =>
    simp only [answersOK] at h
    obtain ⟨a, _, ha⟩ := forM_err h
    exact ordered_err ha
  | sum | running =>
    simp only [answersOK] at h
    obtain ⟨a, _, ha⟩ := forM_err h
    rcases bind_err ha with ha | ⟨_, _, ha⟩
    · exact numOf_err ha
    · cases ha
  | map => simp [answersOK] at h

theorem bools_of_ok {line : Nat} {as : List Val} (h : as.forM (fun a => do let _ ← asBool a line) = .ok ()) :
    ∀ a ∈ as, ∃ b, a = .bool b := by
  intro a ha
  have := forM_ok h a ha
  obtain ⟨b, hb, _⟩ := bind_ok this
  cases a <;> simp [asBool, failK] at hb
  exact ⟨_, rfl⟩

/-- **A walk of a list**: `count`, `sum`, `any`, `all`, `map`, `filter`, `first`, `max_of`,
    `min_of`, `running_totals`. -/
theorem walk_case {C : Ctx} {res : List (Option Abs)} {n : Nat} (IH : SoundAt C res n) {m : Nat} {A : ACtx}
    {ρ : Env} {o : Out} {dep id line : Nat} {w : Walk} {list : Expr} {x : String} {body : Expr} {c : Option Conv}
    {ord : Order} {step : Option Rat} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (walkAbs A m id w list x body c ord step o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.walk id line w list x body c ord step))
      (walkAbs A m id w list x body c ord step o) := by
  cases m with
  | zero => simp only [walkAbs]; exact concl_fuel hS
  | succ m =>
    have S := sat m
    have hG1 : Good C res (ev A m list o).ctx := hG.same (S.ev _ _ _).2
    -- what the analysis of the function given grows from
    have gcond : ∀ (A' : ACtx) (el : Abs) (o' : Out), Grows o' (condition A' m x el body c o').2 :=
      fun A' el o' => S.cond _ _ _ _ _ _
    have glam : ∀ (A' : ACtx) (el : Abs) (o' : Out), Grows o' (lam1 A' m x el body c o').2 :=
      fun A' el o' => S.lam1 _ _ _ _ _ _
    simp only [walkAbs] at hw ⊢
    -- the list walked
    have hwl : (ev A m list o).out.widened = false := by
      split at hw
      · exact hw
      · split at hw
        · exact grows_not_widened (gcond _ _ _) hw
        · exact grows_not_widened ((glam _ _ _).trans (lamResult_grows (Grows.refl _))) hw
    obtain ⟨hl1, hl2⟩ := IH.expr m A ρ list o dep hG hS hwl
    refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
    · simp only [eval] at hv
      obtain ⟨lv, hlv, hv⟩ := bind_ok hv
      obtain ⟨hml, hsl, _⟩ := hl1 lv hlv
      cases lv with
      | list xs =>
        simp only at hv
        obtain ⟨as, has, hv⟩ := bind_ok hv
        obtain ⟨u, hok, hv⟩ := bind_ok hv
        obtain ⟨hcnt, helem⟩ := walked_sound hml
        have hshape := evalEach_shape n dep ρ x body c w xs as has
        split
        · -- a count below 0: no list comes
          rename_i hneg
          exfalso
          have h1 := hcnt.hi
          rcases hch : (walked (ev A m list o).abs).1.hi with _ | u
          · rw [hch] at hneg; simp at hneg
          · rw [hch] at hneg h1
            simp only [Option.any_some, decide_eq_true_eq] at hneg
            have : u < 0 := (num_neg_iff u).mp hneg
            have h0 : (0 : Rat) ≤ (xs.length : Rat) := by exact_mod_cast Nat.zero_le _
            exact absurd (Rat.le_trans h0 h1) (Rat.not_le.mpr this)
        · rename_i hneg
          rw [ite_eq_right hneg] at hw
          split
          · rename_i hc
            rw [ite_eq_left hc] at hw
            simp only [AR.of] at hw ⊢
            obtain ⟨hb1, _⟩ := condBody IH hG1 hsl helem hw (w := w) (dep := dep)
            obtain ⟨ht, hf, hheld⟩ := hb1 as has
            have hbool : ∀ a ∈ as, ∃ b, a = .bool b := by
              cases w <;> simp [Walk.isCondition] at hc <;> simp only [answersOK] at hok <;> exact bools_of_ok hok
            exact ⟨condResult_sound hc hcnt helem hshape hbool ht hf hheld hv, hsl, fun _ _ => trivial⟩
          · rename_i hc
            rw [ite_eq_right hc] at hw
            simp only [AR.of] at hw ⊢
            have hc' : w.isCondition = false := by simpa using hc
            have hwl1 := grows_not_widened (lamResult_grows (Grows.refl _)) hw
            obtain ⟨hb1, _⟩ := lamBody IH hG1 hsl helem hwl1 (w := w) (dep := dep)
            exact ⟨(lamResult_sound (C := C) hc' hcnt helem hshape (hb1 as has) hok).1 v hv, hsl, fun _ _ => trivial⟩
      | _ => simp [failK] at hv
    · simp only [eval] at hv
      -- what the analysis keeps grows from what the list's analysis kept
      have hgrow : Grows (ev A m list o).out
          (if ((walked (ev A m list o).abs).1.hi.any fun h => h.num < 0) = true then
            AR.of .never (ev A m list o).out (ev A m list o).ctx
          else if w.isCondition = true then
            AR.of (condResult w (walked (ev A m list o).abs).1 (walked (ev A m list o).abs).2
              (condition (ev A m list o).ctx m x (walked (ev A m list o).abs).2 body c (ev A m list o).out).1.1
              (condition (ev A m list o).ctx m x (walked (ev A m list o).abs).2 body c (ev A m list o).out).1.2.1
              (condition (ev A m list o).ctx m x (walked (ev A m list o).abs).2 body c (ev A m list o).out).1.2.2)
              (condition (ev A m list o).ctx m x (walked (ev A m list o).abs).2 body c (ev A m list o).out).2
              (ev A m list o).ctx
          else
            AR.of (lamResult (lam1 (ev A m list o).ctx m x (walked (ev A m list o).abs).2 body c (ev A m list o).out).2 id w
                (walked (ev A m list o).abs).1 (walked (ev A m list o).abs).2
                (lam1 (ev A m list o).ctx m x (walked (ev A m list o).abs).2 body c (ev A m list o).out).1 step).1
              (lamResult (lam1 (ev A m list o).ctx m x (walked (ev A m list o).abs).2 body c (ev A m list o).out).2 id w
                (walked (ev A m list o).abs).1 (walked (ev A m list o).abs).2
                (lam1 (ev A m list o).ctx m x (walked (ev A m list o).abs).2 body c (ev A m list o).out).1 step).2
              (ev A m list o).ctx).out := by
        split
        · exact Grows.refl _
        · split
          · exact gcond _ _ _
          · exact (glam _ _ _).trans (lamResult_grows (Grows.refl _))
      rcases bind_err hv with hv | ⟨lv, hlv, hv⟩
      · exact Wit.grow (hl2 k l hv hk) hgrow
      · obtain ⟨hml, hsl, _⟩ := hl1 lv hlv
        cases lv with
        | list xs =>
          simp only at hv
          obtain ⟨hcnt, helem⟩ := walked_sound hml
          rcases bind_err hv with hv | ⟨as, has, hv⟩
          · -- the function given fails on an element
            split
            · rename_i hneg
              exfalso
              have h1 := hcnt.hi
              rcases hch : (walked (ev A m list o).abs).1.hi with _ | u
              · rw [hch] at hneg; simp at hneg
              · rw [hch] at hneg h1
                simp only [Option.any_some, decide_eq_true_eq] at hneg
                have : u < 0 := (num_neg_iff u).mp hneg
                have h0 : (0 : Rat) ≤ (xs.length : Rat) := by exact_mod_cast Nat.zero_le _
                exact absurd (Rat.le_trans h0 h1) (Rat.not_le.mpr this)
            · rename_i hneg
              rw [ite_eq_right hneg] at hw
              split
              · rename_i hc
                rw [ite_eq_left hc] at hw
                simp only [AR.of] at hw ⊢
                exact (condBody IH hG1 hsl helem hw (w := w) (dep := dep)).2 k l hv hk
              · rename_i hc
                rw [ite_eq_right hc] at hw
                simp only [AR.of] at hw ⊢
                have hwl1 := grows_not_widened (lamResult_grows (Grows.refl _)) hw
                exact Wit.grow ((lamBody IH hG1 hsl helem hwl1 (w := w) (dep := dep)).2 k l hv hk)
                  (lamResult_grows (Grows.refl _))
          · rcases bind_err hv with hv | ⟨u, hok, hv⟩
            · have h1 := answersOK_err hv
              simp only [Stop.failed.injEq] at h1
              rw [h1.1] at hk; simp [Kind.ofEffect] at hk
            · -- the walk itself fails: a sum that leaves int64
              have hshape := evalEach_shape n dep ρ x body c w xs as has
              split
              · rename_i hneg
                exfalso
                have h1 := hcnt.hi
                rcases hch : (walked (ev A m list o).abs).1.hi with _ | u
                · rw [hch] at hneg; simp at hneg
                · rw [hch] at hneg h1
                  simp only [Option.any_some, decide_eq_true_eq] at hneg
                  have : u < 0 := (num_neg_iff u).mp hneg
                  have h0 : (0 : Rat) ≤ (xs.length : Rat) := by exact_mod_cast Nat.zero_le _
                  exact absurd (Rat.le_trans h0 h1) (Rat.not_le.mpr this)
              · rename_i hneg
                rw [ite_eq_right hneg] at hw
                split
                · rename_i hc
                  exfalso
                  cases w <;> simp [Walk.isCondition] at hc <;> simp [fold] at hv
                · rename_i hc
                  rw [ite_eq_right hc] at hw
                  simp only [AR.of] at hw ⊢
                  have hc' : w.isCondition = false := by simpa using hc
                  have hwl1 := grows_not_widened (lamResult_grows (Grows.refl _)) hw
                  obtain ⟨hb1, _⟩ := lamBody IH hG1 hsl helem hwl1 (w := w) (dep := dep)
                  exact (lamResult_sound hc' hcnt helem hshape (hb1 as has) hok).2 k l hv hk
        | _ => have := failK_err hv; subst this; simp [Kind.ofEffect] at hk

end KanameCore
