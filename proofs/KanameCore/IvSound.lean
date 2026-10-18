/-
  Each operation of the intervals holds the values it is about (kaname's DESIGN 5.2, 15.2.2): where
  `a` is in `i` and `b` in `o`, `a + b` is in `i.plus o`, `a * b` in `i.times o`, `a / b` in
  `i.over o` for `b` not 0, the value rounded in `i.round`, and so on; a comparison the intervals
  decide comes out as they say; `fits` of an interval says `fits` of its values.

  An interval holds a value when the value is between its ends and its denominator is within its
  bound (`Iv.In`, what `Iv.has` tests).
-/
import KanameCore.Round
import KanameCore.Iv

namespace KanameCore

-- ── Rationals ───────────────────────────────────────────────────────────────

theorem den_mul_le (a b : Rat) : (a * b).den ≤ a.den * b.den := by
  rw [Rat.mul_def, Rat.normalize_eq]; exact Nat.div_le_self _ _

theorem den_add_le (a b : Rat) : (a + b).den ≤ a.den * b.den := by
  rw [Rat.add_def, Rat.normalize_eq]; exact Nat.div_le_self _ _

theorem den_neg (a : Rat) : (-a).den = a.den := Rat.neg_den a

theorem den_sub_le (a b : Rat) : (a - b).den ≤ a.den * b.den := by
  rw [Rat.sub_eq_add_neg]; have := den_add_le a (-b); rw [den_neg] at this; exact this

theorem den_inv_le (b : Rat) (hb : b ≠ 0) : (b⁻¹).den ≤ b.num.natAbs := by
  rw [Rat.den_inv]
  have : b.num ≠ 0 := fun h => hb (Rat.num_eq_zero.mp h)
  simp [this]

theorem den_div_le (a b : Rat) (hb : b ≠ 0) : (a / b).den ≤ a.den * b.num.natAbs := by
  rw [Rat.div_def]
  exact Nat.le_trans (den_mul_le a b⁻¹) (Nat.mul_le_mul_left _ (den_inv_le b hb))

theorem den_intCast_mul_le (k : Int) (g : Rat) : ((k : Rat) * g).den ≤ g.den := by
  have := den_mul_le (k : Rat) g
  rw [Rat.den_intCast, Nat.one_mul] at this
  exact this

theorem den_pos' (a : Rat) : 0 < a.den := a.den_pos

theorem rlt_of_lt_of_le {a b c : Rat} (h1 : a < b) (h2 : b ≤ c) : a < c := by grind
theorem rlt_of_le_of_lt {a b c : Rat} (h1 : a ≤ b) (h2 : b < c) : a < c := by grind

/-- A value whose denominator is at most `d`, as a whole number of `1/d!`… the magnitude of its
    numerator: `|a.num| = |a| · a.den`. -/
theorem natAbs_num_eq (a : Rat) : ((a.num.natAbs : Nat) : Rat) = mag a * (a.den : Rat) := by
  rw [mag_eq]
  have := den_ne_zero a
  grind

-- ── Membership ──────────────────────────────────────────────────────────────

/-- An end below a value, an end above it, a bound on its denominator. -/
def EndLe : End → Rat → Prop
  | none, _ => True
  | some l, v => l ≤ v

def EndGe : End → Rat → Prop
  | none, _ => True
  | some h, v => v ≤ h

def DenLe : Option Nat → Rat → Prop
  | none, _ => True
  | some d, v => v.den ≤ d

/-- An interval holds a value. -/
structure Iv.In (i : Iv) (v : Rat) : Prop where
  lo : EndLe i.lo v
  hi : EndGe i.hi v
  den : DenLe i.den v

theorem Iv.has_iff (i : Iv) (v : Rat) : i.has v = true ↔ i.In v := by
  unfold Iv.has
  rcases i with ⟨lo, hi, den⟩
  constructor
  · intro h
    simp only [Bool.and_eq_true] at h
    obtain ⟨⟨h1, h2⟩, h3⟩ := h
    refine ⟨?_, ?_, ?_⟩
    · cases lo <;> simp_all [EndLe]
    · cases hi <;> simp_all [EndGe]
    · cases den <;> simp_all [DenLe]
  · rintro ⟨h1, h2, h3⟩
    simp only [Bool.and_eq_true]
    refine ⟨⟨?_, ?_⟩, ?_⟩
    · cases lo <;> simp_all [EndLe]
    · cases hi <;> simp_all [EndGe]
    · cases den <;> simp_all [DenLe]

theorem div_le_div_of_le {x y k : Rat} (h : x ≤ y) (hk : 0 < k) : x / k ≤ y / k := by
  rw [Rat.div_def, Rat.div_def]
  exact Rat.mul_le_mul_of_nonneg_right h (Rat.le_of_lt (Rat.inv_pos.2 hk))

theorem floorGrid_le (v : Rat) (d : Nat) (hd : 0 < d) : floorGrid v d ≤ v := by
  unfold floorGrid
  have hdr : (0 : Rat) < d := Rat.natCast_pos.mpr hd
  have h := div_le_div_of_le (Rat.floor_le (v * d)) hdr
  rw [Rat.mul_div_cancel (Rat.ne_of_gt hdr)] at h
  exact h

theorem le_ceilGrid (v : Rat) (d : Nat) (hd : 0 < d) : v ≤ ceilGrid v d := by
  unfold ceilGrid
  have hdr : (0 : Rat) < d := Rat.natCast_pos.mpr hd
  have h := div_le_div_of_le (Rat.le_ceil (x := v * d)) hdr
  rw [Rat.mul_div_cancel (Rat.ne_of_gt hdr)] at h
  exact h

theorem FINE_pos : 0 < FINE := by unfold FINE; decide

theorem normLo_le {lo : End} {v : Rat} (h : EndLe lo v) : EndLe (normLo lo) v := by
  cases lo with
  | none => trivial
  | some l =>
    simp only [normLo]
    by_cases hb : tooBig l = true
    · simp [hb, EndLe]
    · simp only [hb, Bool.false_eq_true, ite_false, EndLe] at h ⊢
      by_cases hf : l.den > FINE
      · simp only [hf, ite_true]; exact Rat.le_trans (floorGrid_le l FINE FINE_pos) h
      · simp only [hf, ite_false]; exact h

theorem normHi_ge {hi : End} {v : Rat} (h : EndGe hi v) : EndGe (normHi hi) v := by
  cases hi with
  | none => trivial
  | some l =>
    simp only [normHi]
    by_cases hb : tooBig l = true
    · simp [hb, EndGe]
    · simp only [hb, Bool.false_eq_true, ite_false, EndGe] at h ⊢
      by_cases hf : l.den > FINE
      · simp only [hf, ite_true]; exact Rat.le_trans h (le_ceilGrid l FINE FINE_pos)
      · simp only [hf, ite_false]; exact h

theorem DenLe_filter {d : Option Nat} {v : Rat} (h : DenLe d v) : DenLe (d.filter (· ≥ 1)) v := by
  cases d with
  | none => trivial
  | some d =>
    simp only [Option.filter]
    split <;> simp_all [DenLe]

theorem In_mk' {lo hi : End} {den : Option Nat} {v : Rat} (h1 : EndLe lo v) (h2 : EndGe hi v) (h3 : DenLe den v) :
    (Iv.mk' lo hi den).In v :=
  ⟨normLo_le h1, normHi_ge h2, DenLe_filter h3⟩

theorem In_exact (v : Rat) : (Iv.exact v).In v :=
  In_mk' (Rat.le_refl) (Rat.le_refl) (Nat.le_refl _)

theorem In_ints {lo hi : Int} {v : Int} (h1 : lo ≤ v) (h2 : v ≤ hi) : (Iv.ints lo hi).In (v : Rat) :=
  In_mk' (Rat.intCast_le_intCast.mpr h1) (Rat.intCast_le_intCast.mpr h2) (by simp [DenLe, Rat.den_intCast])

theorem In_withDen {i : Iv} {d : Option Nat} {v : Rat} (h : i.In v) (hd : DenLe d v) : (i.withDen d).In v := by
  refine ⟨h.lo, h.hi, ?_⟩
  unfold Iv.withDen
  have := h.den
  rcases hi : i.den with _ | a <;> rcases d with _ | b <;> simp_all [DenLe]
  omega

theorem In_hull_left {i o : Iv} {v : Rat} (h : i.In v) : (i.hull o).In v := by
  obtain ⟨h1, h2, h3⟩ := h
  refine ⟨?_, ?_, ?_⟩
  · unfold Iv.hull minLo; dsimp only
    rcases hl : i.lo with _ | a <;> rcases o.lo with _ | b <;> simp_all [EndLe]
    split <;> grind
  · unfold Iv.hull maxHi; dsimp only
    rcases hl : i.hi with _ | a <;> rcases o.hi with _ | b <;> simp_all [EndGe]
    split <;> grind
  · unfold Iv.hull; dsimp only
    rcases hl : i.den with _ | a <;> rcases o.den with _ | b <;> simp_all [DenLe]
    omega

theorem hull_comm_lo (i o : Iv) : (i.hull o).lo = (o.hull i).lo := by
  unfold Iv.hull minLo; dsimp only
  rcases i.lo with _ | a <;> rcases o.lo with _ | b <;> simp
  split <;> split <;> grind

theorem hull_comm_hi (i o : Iv) : (i.hull o).hi = (o.hull i).hi := by
  unfold Iv.hull maxHi; dsimp only
  rcases i.hi with _ | a <;> rcases o.hi with _ | b <;> simp
  split <;> split <;> grind

theorem hull_comm_den (i o : Iv) : (i.hull o).den = (o.hull i).den := by
  unfold Iv.hull; dsimp only
  rcases i.den with _ | a <;> rcases o.den with _ | b <;> simp
  omega

theorem In_hull_right {i o : Iv} {v : Rat} (h : o.In v) : (i.hull o).In v := by
  have := In_hull_left (o := i) h
  exact ⟨by rw [hull_comm_lo]; exact this.lo, by rw [hull_comm_hi]; exact this.hi, by rw [hull_comm_den]; exact this.den⟩

/-- Two intervals that hold one value meet, in an interval that holds it. -/
theorem In_meet {i o : Iv} {v : Rat} (hi : i.In v) (ho : o.In v) : ∃ m, i.meet o = some m ∧ m.In v := by
  have lo_ok : EndLe (maxLo i.lo o.lo) v := by
    have a := hi.lo; have b := ho.lo
    unfold maxLo
    rcases hl : i.lo with _ | x <;> rcases hm : o.lo with _ | y <;> simp_all [EndLe]
    split <;> assumption
  have hi_ok : EndGe (minHi i.hi o.hi) v := by
    have a := hi.hi; have b := ho.hi
    unfold minHi
    rcases hl : i.hi with _ | x <;> rcases hm : o.hi with _ | y <;> simp_all [EndGe]
    split <;> assumption
  have den_ok : DenLe (i.withDen o.den).den v := (In_withDen hi ho.den).den
  unfold Iv.meet
  dsimp only
  generalize hL : maxLo i.lo o.lo = L at lo_ok
  generalize hH : minHi i.hi o.hi = H at hi_ok
  cases L with
  | none => exact ⟨_, rfl, ⟨lo_ok, hi_ok, den_ok⟩⟩
  | some a =>
    cases H with
    | none => exact ⟨_, rfl, ⟨lo_ok, hi_ok, den_ok⟩⟩
    | some b =>
      simp only [EndLe, EndGe] at lo_ok hi_ok
      have : ¬ b < a := fun h => absurd (Rat.le_trans lo_ok hi_ok) (Rat.not_le.mpr h)
      simp only [this, ite_false]
      exact ⟨_, rfl, ⟨lo_ok, hi_ok, den_ok⟩⟩

theorem In_cut {i : Iv} {mn mx : End} {v : Rat} (h : i.In v) (h1 : EndLe mn v) (h2 : EndGe mx v) : (i.cut mn mx).In v := by
  unfold Iv.cut
  obtain ⟨m, hm, hv⟩ := In_meet h (o := ⟨mn, mx, none⟩) ⟨h1, h2, trivial⟩
  rw [hm]; exact hv

/-- An interval within a range holds only values within it. -/
theorem within_sound {i : Iv} {mn mx : End} {v : Rat} (h : i.In v) (hw : i.within mn mx = true) : EndLe mn v ∧ EndGe mx v := by
  unfold Iv.within at hw
  simp only [Bool.and_eq_true] at hw
  obtain ⟨w1, w2⟩ := hw
  constructor
  · cases mn with
    | none => trivial
    | some m =>
      simp only [Option.all_some] at w1
      have := h.lo
      rcases hl : i.lo with _ | l <;> simp_all [EndLe]
      exact Rat.le_trans w1 this
  · cases mx with
    | none => trivial
    | some m =>
      simp only [Option.all_some] at w2
      have := h.hi
      rcases hl : i.hi with _ | l <;> simp_all [EndGe]
      exact Rat.le_trans this w2

/-- **`fits` of an interval says `fits` of its values** (DESIGN 5.6): a value `n/d` with `d` at
    most the bound `D` has `|n| = |v|·d`, at most `hi·D` above zero and `lo·D` below it. -/
theorem fits_sound {i : Iv} {v : Rat} (h : i.In v) (hf : i.fits = true) : KanameCore.fits v = true := by
  unfold Iv.fits at hf
  rcases hlo : i.lo with _ | lo <;> rcases hhi : i.hi with _ | hi <;> rcases hd : i.den with _ | d <;>
    simp only [hlo, hhi, hd] at hf <;> try contradiction
  simp only [Bool.and_eq_true, Bool.or_eq_true, decide_eq_true_eq] at hf
  obtain ⟨⟨hD, hL⟩, hH⟩ := hf
  have h1 : lo ≤ v := by have := h.lo; rw [hlo] at this; exact this
  have h2 : v ≤ hi := by have := h.hi; rw [hhi] at this; exact this
  have h3 : v.den ≤ d := by have := h.den; rw [hd] at this; exact this
  unfold KanameCore.fits
  simp only [Bool.and_eq_true, decide_eq_true_eq]
  have hden : (v.den : Int) ≤ i64max := by omega
  have hvd := rat_eq_num_div_den v
  have hdpos : (0 : Rat) < v.den := Rat.natCast_pos.mpr v.den_pos
  have hdd : (v.den : Rat) ≤ (d : Rat) := Rat.natCast_le_natCast.mpr h3
  -- v.num = v · v.den
  have hnum : ((v.num : Int) : Rat) = v * (v.den : Rat) := by
    have := den_ne_zero v
    conv => rhs; rw [hvd]
    grind
  refine ⟨⟨?_, ?_⟩, hden⟩
  · -- the numerator from below
    rcases Rat.le_total (a := 0) (b := v) with hv | hv
    · have : (0 : Rat) ≤ v * (v.den : Rat) := Rat.mul_nonneg hv (Rat.le_of_lt hdpos)
      have : (0 : Int) ≤ v.num := by rw [← hnum] at this; exact_mod_cast this
      unfold i64min; omega
    · rcases hL with hL | hL
      · -- lo ≥ 0 and v ≤ 0: v = 0
        have : (0 : Rat) ≤ lo := (@Rat.num_nonneg lo).mp hL
        have hv0 : v = 0 := Rat.le_antisymm hv (Rat.le_trans this h1)
        subst hv0; simp [i64min]
      · -- v·den ≥ lo·d ≥ i64min
        have hd0 : (0 : Rat) ≤ d := Rat.le_of_lt (rlt_of_lt_of_le hdpos hdd)
        have a1 : lo * (d : Rat) ≤ v * (d : Rat) := Rat.mul_le_mul_of_nonneg_right h1 hd0
        have a2 : v * (d : Rat) ≤ v * (v.den : Rat) := by
          have := Rat.mul_le_mul_of_nonneg_left hdd (Rat.le_refl (a := 0))
          have hn : 0 ≤ -v := by grind
          have := Rat.mul_le_mul_of_nonneg_left hdd hn
          grind
        have : ((i64min : Int) : Rat) ≤ ((v.num : Int) : Rat) := by
          rw [hnum]; exact Rat.le_trans hL (Rat.le_trans a1 a2)
        exact Rat.intCast_le_intCast.mp this
  · rcases Rat.le_total (a := v) (b := 0) with hv | hv
    · have : v * (v.den : Rat) ≤ 0 := by
        have := Rat.mul_le_mul_of_nonneg_right hv (Rat.le_of_lt hdpos); grind
      have : v.num ≤ (0 : Int) := by rw [← hnum] at this; exact_mod_cast this
      unfold i64max; omega
    · rcases hH with hH | hH
      · have : hi ≤ 0 := by
          rcases Rat.le_total (a := hi) (b := 0) with h' | h'
          · exact h'
          · have := (@Rat.num_nonneg hi).mpr h'; have : hi.num = 0 := by omega
            rw [Rat.num_eq_zero.mp this]; exact Rat.le_refl
        have hv0 : v = 0 := Rat.le_antisymm (Rat.le_trans h2 this) hv
        subst hv0; simp [i64max]
      · have hd0 : (0 : Rat) ≤ d := Rat.le_of_lt (rlt_of_lt_of_le hdpos hdd)
        have a1 : v * (d : Rat) ≤ hi * (d : Rat) := Rat.mul_le_mul_of_nonneg_right h2 hd0
        have a2 : v * (v.den : Rat) ≤ v * (d : Rat) := Rat.mul_le_mul_of_nonneg_left hdd hv
        have : ((v.num : Int) : Rat) ≤ ((i64max : Int) : Rat) := by
          rw [hnum]; exact Rat.le_trans a2 (Rat.le_trans a1 hH)
        exact Rat.intCast_le_intCast.mp this

-- ── Sums and differences ────────────────────────────────────────────────────

theorem In_plus {i o : Iv} {a b : Rat} (ha : i.In a) (hb : o.In b) : (i.plus o).In (a + b) := by
  unfold Iv.plus
  apply In_mk'
  · have h1 := ha.lo; have h2 := hb.lo
    rcases hl : i.lo with _ | x <;> rcases hm : o.lo with _ | y <;> simp_all [EndLe]
    grind
  · have h1 := ha.hi; have h2 := hb.hi
    rcases hl : i.hi with _ | x <;> rcases hm : o.hi with _ | y <;> simp_all [EndGe]
    grind
  · have h1 := ha.den; have h2 := hb.den
    unfold Iv.denMul
    rcases hl : i.den with _ | x <;> rcases hm : o.den with _ | y <;> simp_all [DenLe]
    exact Nat.le_trans (den_add_le a b) (Nat.mul_le_mul h1 h2)

theorem In_negated {i : Iv} {a : Rat} (ha : i.In a) : i.negated.In (-a) := by
  unfold Iv.negated
  apply In_mk'
  · have := ha.hi
    rcases hl : i.hi with _ | x <;> simp_all [EndLe, EndGe]
  · have := ha.lo
    rcases hl : i.lo with _ | x <;> simp_all [EndLe, EndGe]
  · have := ha.den
    rcases hl : i.den with _ | x <;> simp_all [DenLe]

theorem In_minus {i o : Iv} {a b : Rat} (ha : i.In a) (hb : o.In b) : (i.minus o).In (a - b) := by
  unfold Iv.minus
  rw [Rat.sub_eq_add_neg]
  exact In_plus ha (In_negated hb)

-- ── The order and the products of ends ──────────────────────────────────────

namespace X

theorem le_refl (p : X) : p.le p = true := by
  cases p <;> simp [le]

theorem le_trans {p q r : X} (h1 : p.le q = true) (h2 : q.le r = true) : p.le r = true := by
  cases p <;> cases q <;> cases r <;> simp_all [le]
  exact Rat.le_trans h1 h2

theorem le_total (p q : X) : p.le q = true ∨ q.le p = true := by
  cases p <;> cases q <;> simp [le]
  exact Rat.le_total

theorem f_le_f {a b : Rat} : (X.f a).le (X.f b) = true ↔ a ≤ b := by simp [le]

theorem ofSign_neg : ofSign (-1) = neg := by simp [ofSign]
theorem ofSign_pos : ofSign 1 = pos := by simp [ofSign]
theorem ofSign_zero : ofSign 0 = f 0 := by simp [ofSign]

theorem sign_f_pos {r : Rat} (h : 0 < r) : (f r).sign = 1 := by
  simp only [sign]; have : ¬ r < 0 := Rat.not_lt.mpr (Rat.le_of_lt h); simp [this, h]
theorem sign_f_neg {r : Rat} (h : r < 0) : (f r).sign = -1 := by simp [sign, h]
theorem sign_f_zero : (f 0).sign = 0 := by simp [sign]

/-- The products of an end with a value, by the value's sign. -/
theorem mul_neg_fpos {b : Rat} (h : 0 < b) : neg.mul (f b) = neg := by
  simp only [mul, sign_f_pos h]; simp [sign, ofSign]
theorem mul_neg_fneg {b : Rat} (h : b < 0) : neg.mul (f b) = pos := by
  simp only [mul, sign_f_neg h]; simp [sign, ofSign]
theorem mul_pos_fpos {b : Rat} (h : 0 < b) : pos.mul (f b) = pos := by
  simp only [mul, sign_f_pos h]; simp [sign, ofSign]
theorem mul_pos_fneg {b : Rat} (h : b < 0) : pos.mul (f b) = neg := by
  simp only [mul, sign_f_neg h]; simp [sign, ofSign]
theorem mul_fpos_neg {a : Rat} (h : 0 < a) : (f a).mul neg = neg := by
  simp only [mul, sign_f_pos h]; simp [sign, ofSign]
theorem mul_fneg_neg {a : Rat} (h : a < 0) : (f a).mul neg = pos := by
  simp only [mul, sign_f_neg h]; simp [sign, ofSign]
theorem mul_fpos_pos {a : Rat} (h : 0 < a) : (f a).mul pos = pos := by
  simp only [mul, sign_f_pos h]; simp [sign, ofSign]
theorem mul_fneg_pos {a : Rat} (h : a < 0) : (f a).mul pos = neg := by
  simp only [mul, sign_f_neg h]; simp [sign, ofSign]
theorem mul_zero_right (p : X) : p.mul (f 0) = f 0 := by
  cases p <;> simp [mul, sign, ofSign]
theorem mul_zero_left (q : X) : (f 0).mul q = f 0 := by
  cases q <;> simp [mul, sign, ofSign]
theorem mul_ff (a b : Rat) : (f a).mul (f b) = f (a * b) := rfl
theorem mul_neg_neg : neg.mul neg = pos := by simp [mul, sign, ofSign]
theorem mul_neg_pos : neg.mul pos = neg := by simp [mul, sign, ofSign]
theorem mul_pos_neg : pos.mul neg = neg := by simp [mul, sign, ofSign]
theorem mul_pos_pos : pos.mul pos = pos := by simp [mul, sign, ofSign]

theorem trichotomy (r : Rat) : r < 0 ∨ r = 0 ∨ 0 < r := by
  rcases Rat.le_total (a := r) (b := 0) with h | h
  · rcases Rat.le_iff_lt_or_eq.mp h with h | h
    · exact Or.inl h
    · exact Or.inr (Or.inl h)
  · rcases Rat.le_iff_lt_or_eq.mp h with h | h
    · exact Or.inr (Or.inr h)
    · exact Or.inr (Or.inl h.symm)

/-- (M1) A nonnegative value as the right factor keeps the order of the left. -/
theorem mul_mono_left_nonneg {p q : X} {b : Rat} (hb : 0 ≤ b) (h : p.le q = true) :
    (p.mul (f b)).le (q.mul (f b)) = true := by
  rcases Rat.le_iff_lt_or_eq.mp hb with hb | hb
  · cases p <;> cases q <;>
      simp only [mul_neg_fpos hb, mul_pos_fpos hb, mul_ff] <;> simp_all [le]
    exact Rat.mul_le_mul_of_nonneg_right h (Rat.le_of_lt hb)
  · subst hb; rw [mul_zero_right, mul_zero_right]; exact le_refl _

/-- (M2) A negative value as the right factor turns the order of the left round. -/
theorem mul_anti_left_neg {p q : X} {b : Rat} (hb : b < 0) (h : p.le q = true) :
    (q.mul (f b)).le (p.mul (f b)) = true := by
  cases p <;> cases q <;>
    simp only [mul_neg_fneg hb, mul_pos_fneg hb, mul_ff] <;> simp_all [le]
  have := Rat.mul_le_mul_of_nonneg_right h (show (0 : Rat) ≤ -b by grind)
  grind

theorem sign_mono {p q : X} (h : p.le q = true) : p.sign ≤ q.sign := by
  cases p <;> cases q <;> simp_all [le, sign]
  all_goals (try split) <;> (try split) <;> (try split) <;> (try split) <;> (try omega)
  all_goals grind

theorem ofSign_le {s t : Int} (h : s ≤ t) : (ofSign s).le (ofSign t) = true := by
  unfold ofSign
  by_cases a : s < 0 <;> by_cases b : t < 0 <;> by_cases c : s > 0 <;> by_cases d : t > 0 <;>
    simp [a, b, c, d, le] <;> omega

theorem mul_pos_left (q : X) : pos.mul q = ofSign q.sign := by
  cases q <;> simp [mul, sign, ofSign]

theorem mul_neg_left (q : X) : neg.mul q = ofSign (-q.sign) := by
  cases q <;> simp [mul, sign, ofSign]

/-- (M3) A left factor that is not negative keeps the order of the right. -/
theorem mul_mono_right {p q1 q2 : X} (hp : 0 ≤ p.sign) (h : q1.le q2 = true) :
    (p.mul q1).le (p.mul q2) = true := by
  cases p with
  | neg => simp [sign] at hp
  | pos => rw [mul_pos_left, mul_pos_left]; exact ofSign_le (sign_mono h)
  | f r =>
    rcases trichotomy r with hr | hr | hr
    · simp [sign_f_neg hr] at hp
    · subst hr; rw [mul_zero_left, mul_zero_left]; exact le_refl _
    · cases q1 <;> cases q2 <;>
        simp only [mul_fpos_neg hr, mul_fpos_pos hr, mul_ff] <;> simp_all [le]
      exact Rat.mul_le_mul_of_nonneg_left h (Rat.le_of_lt hr)

/-- (M4) A negative left factor turns the order of the right round. -/
theorem mul_anti_right {p q1 q2 : X} (hp : p.sign < 0) (h : q1.le q2 = true) :
    (p.mul q2).le (p.mul q1) = true := by
  cases p with
  | pos => simp [sign] at hp
  | neg => rw [mul_neg_left, mul_neg_left]; exact ofSign_le (by have := sign_mono h; omega)
  | f r =>
    rcases trichotomy r with hr | hr | hr
    · cases q1 <;> cases q2 <;>
        simp only [mul_fneg_neg hr, mul_fneg_pos hr, mul_ff] <;> simp_all [le]
      have := Rat.mul_le_mul_of_nonneg_left h (show (0 : Rat) ≤ -r by grind)
      grind
    · subst hr; simp [sign] at hp
    · simp [sign_f_pos hr] at hp

end X

-- ── Products of intervals ───────────────────────────────────────────────────

theorem loHi_bounds (cs : List X) (acc : X × X) :
    (cs.foldl Iv.loHi acc).1.le acc.1 = true ∧ acc.2.le (cs.foldl Iv.loHi acc).2 = true ∧
      ∀ c ∈ cs, (cs.foldl Iv.loHi acc).1.le c = true ∧ c.le (cs.foldl Iv.loHi acc).2 = true := by
  induction cs generalizing acc with
  | nil => exact ⟨X.le_refl _, X.le_refl _, by simp⟩
  | cons c rest ih =>
    simp only [List.foldl_cons]
    have ⟨i1, i2, i3⟩ := ih (Iv.loHi acc c)
    have s1 : (Iv.loHi acc c).1.le acc.1 = true ∧ (Iv.loHi acc c).1.le c = true := by
      unfold Iv.loHi; dsimp only
      split
      · exact ⟨by assumption, X.le_refl _⟩
      · rename_i h
        refine ⟨X.le_refl _, ?_⟩
        rcases X.le_total c acc.1 with h' | h'
        · exact absurd h' h
        · exact h'
    have s2 : acc.2.le (Iv.loHi acc c).2 = true ∧ c.le (Iv.loHi acc c).2 = true := by
      unfold Iv.loHi; dsimp only
      split
      · exact ⟨by assumption, X.le_refl _⟩
      · rename_i h
        refine ⟨X.le_refl _, ?_⟩
        rcases X.le_total acc.2 c with h' | h'
        · exact absurd h' h
        · exact h'
    refine ⟨X.le_trans i1 s1.1, X.le_trans s2.1 i2, ?_⟩
    intro d hd
    rcases List.mem_cons.mp hd with hd | hd
    · subst hd; exact ⟨X.le_trans i1 s1.2, X.le_trans s2.2 i2⟩
    · exact i3 d hd

theorem times_corner {xlo xhi ylo yhi : X} {a b : Rat}
    (h1 : xlo.le (X.f a) = true) (h2 : (X.f a).le xhi = true) (h3 : ylo.le (X.f b) = true) (h4 : (X.f b).le yhi = true) :
    (∃ c ∈ [xlo.mul ylo, xlo.mul yhi, xhi.mul ylo, xhi.mul yhi], c.le (X.f (a * b)) = true) ∧
    (∃ c ∈ [xlo.mul ylo, xlo.mul yhi, xhi.mul ylo, xhi.mul yhi], (X.f (a * b)).le c = true) := by
  have e : X.f (a * b) = (X.f a).mul (X.f b) := rfl
  rw [e]
  rcases Rat.le_total (a := 0) (b := b) with hb | hb'
  · refine ⟨?_, ?_⟩
    · have l1 := X.mul_mono_left_nonneg hb h1
      by_cases s : 0 ≤ xlo.sign
      · exact ⟨_, by simp, X.le_trans (X.mul_mono_right s h3) l1⟩
      · exact ⟨_, by simp, X.le_trans (X.mul_anti_right (by omega) h4) l1⟩
    · have u1 := X.mul_mono_left_nonneg hb h2
      by_cases s : 0 ≤ xhi.sign
      · exact ⟨_, by simp, X.le_trans u1 (X.mul_mono_right s h4)⟩
      · exact ⟨_, by simp, X.le_trans u1 (X.mul_anti_right (by omega) h3)⟩
  · rcases Rat.le_iff_lt_or_eq.mp hb' with hb | hb
    · refine ⟨?_, ?_⟩
      · have l1 := X.mul_anti_left_neg hb h2
        by_cases s : 0 ≤ xhi.sign
        · exact ⟨_, by simp, X.le_trans (X.mul_mono_right s h3) l1⟩
        · exact ⟨_, by simp, X.le_trans (X.mul_anti_right (by omega) h4) l1⟩
      · have u1 := X.mul_anti_left_neg hb h1
        by_cases s : 0 ≤ xlo.sign
        · exact ⟨_, by simp, X.le_trans u1 (X.mul_mono_right s h4)⟩
        · exact ⟨_, by simp, X.le_trans u1 (X.mul_anti_right (by omega) h3)⟩
    · subst hb
      refine ⟨?_, ?_⟩
      · have l1 := X.mul_mono_left_nonneg (Rat.le_refl) h1
        by_cases s : 0 ≤ xlo.sign
        · exact ⟨_, by simp, X.le_trans (X.mul_mono_right s h3) l1⟩
        · exact ⟨_, by simp, X.le_trans (X.mul_anti_right (by omega) h4) l1⟩
      · have u1 := X.mul_mono_left_nonneg (Rat.le_refl) h2
        by_cases s : 0 ≤ xhi.sign
        · exact ⟨_, by simp, X.le_trans u1 (X.mul_mono_right s h4)⟩
        · exact ⟨_, by simp, X.le_trans u1 (X.mul_anti_right (by omega) h3)⟩

theorem xLo_le {i : Iv} {a : Rat} (h : i.In a) : i.xLo.le (X.f a) = true := by
  have := h.lo; unfold Iv.xLo
  rcases hl : i.lo with _ | l <;> simp_all [EndLe, X.le]

theorem le_xHi {i : Iv} {a : Rat} (h : i.In a) : (X.f a).le i.xHi = true := by
  have := h.hi; unfold Iv.xHi
  rcases hl : i.hi with _ | l <;> simp_all [EndGe, X.le]

theorem toEnd_le {p : X} {v : Rat} (h : p.le (X.f v) = true) : EndLe p.toEnd v := by
  cases p <;> simp_all [X.toEnd, EndLe, X.le]

theorem toEnd_ge {p : X} {v : Rat} (h : (X.f v).le p = true) : EndGe p.toEnd v := by
  cases p <;> simp_all [X.toEnd, EndGe, X.le]

theorem In_ofX {lo hi : X} {den : Option Nat} {v : Rat} (h1 : lo.le (X.f v) = true) (h2 : (X.f v).le hi = true)
    (h3 : DenLe den v) : (Iv.ofX lo hi den).In v :=
  In_mk' (toEnd_le h1) (toEnd_ge h2) h3

theorem DenLe_denMul {d1 d2 : Option Nat} {a b : Rat} (h1 : DenLe d1 a) (h2 : DenLe d2 b) :
    DenLe (Iv.denMul d1 d2) (a * b) := by
  unfold Iv.denMul
  rcases d1 with _ | x <;> rcases d2 with _ | y <;> simp_all [DenLe]
  exact Nat.le_trans (den_mul_le a b) (Nat.mul_le_mul h1 h2)

/-- **The product of two intervals holds every product of their values.** -/
theorem In_times {i o : Iv} {a b : Rat} (ha : i.In a) (hb : o.In b) : (i.times o).In (a * b) := by
  unfold Iv.times
  dsimp only
  have ⟨lw, up⟩ := times_corner (xLo_le ha) (le_xHi ha) (xLo_le hb) (le_xHi hb)
  have ⟨_, _, hall⟩ := loHi_bounds [i.xLo.mul o.xLo, i.xLo.mul o.xHi, i.xHi.mul o.xLo, i.xHi.mul o.xHi]
    (i.xLo.mul o.xLo, i.xLo.mul o.xLo)
  apply In_ofX _ _ (DenLe_denMul ha.den hb.den)
  · obtain ⟨c, hc, hcl⟩ := lw
    exact X.le_trans (hall c hc).1 hcl
  · obtain ⟨c, hc, hcu⟩ := up
    exact X.le_trans hcu (hall c hc).2

theorem In_scale {i : Iv} {a : Rat} (f : Rat) (ha : i.In a) : (i.scale f).In (a * f) :=
  In_times ha (In_exact f)

-- ── Reciprocals and quotients ───────────────────────────────────────────────

theorem inv_mul_self {a : Rat} (h : a ≠ 0) : a * a⁻¹ = 1 := Rat.mul_inv_cancel a h

theorem inv_le_inv_of_pos {a b : Rat} (ha : 0 < a) (h : a ≤ b) : b⁻¹ ≤ a⁻¹ := by
  have hb : 0 < b := by grind
  have hai := Rat.inv_pos.mpr ha
  have hbi := Rat.inv_pos.mpr hb
  have h1 : a * b⁻¹ ≤ b * b⁻¹ := Rat.mul_le_mul_of_nonneg_right h (Rat.le_of_lt hbi)
  have h2 : a⁻¹ * (a * b⁻¹) ≤ a⁻¹ * (b * b⁻¹) := Rat.mul_le_mul_of_nonneg_left h1 (Rat.le_of_lt hai)
  have e1 : a⁻¹ * (a * b⁻¹) = b⁻¹ := by
    rw [← Rat.mul_assoc, Rat.mul_comm a⁻¹ a, Rat.mul_inv_cancel a (Rat.ne_of_gt ha), Rat.one_mul]
  have e2 : a⁻¹ * (b * b⁻¹) = a⁻¹ := by
    rw [Rat.mul_inv_cancel b (Rat.ne_of_gt hb), Rat.mul_one]
  rw [e1, e2] at h2
  exact h2

theorem neg_inv (a : Rat) : (-a)⁻¹ = -(a⁻¹) := by
  by_cases h : a = 0
  · subst h; simp
  · have e := inv_mul_self h
    have hn : -a ≠ 0 := by grind
    have e2 := inv_mul_self hn
    -- both are the inverse of -a
    have : (-a)⁻¹ = (-a)⁻¹ * (a * a⁻¹) := by rw [e]; grind
    grind

theorem inv_le_inv_of_neg {a b : Rat} (hb : b < 0) (h : a ≤ b) : b⁻¹ ≤ a⁻¹ := by
  have := inv_le_inv_of_pos (show 0 < -b by grind) (show -b ≤ -a by grind)
  rw [neg_inv, neg_inv] at this
  grind

theorem inv_pos_of_pos {a : Rat} (h : 0 < a) : 0 < a⁻¹ := Rat.inv_pos.mpr h

theorem inv_neg_of_neg {a : Rat} (h : a < 0) : a⁻¹ < 0 := by
  have := Rat.inv_pos.mpr (show 0 < -a by grind)
  rw [neg_inv] at this
  grind

/-- The reciprocals of an interval of positive values. -/
theorem In_recip_pos {o : Iv} {b : Rat} (hb : o.In b) (hpos : o.lo.any (fun l => l.num > 0) = true) :
    o.recip.In b⁻¹ := by
  obtain ⟨l, hl, hln⟩ : ∃ l, o.lo = some l ∧ 0 < l := by
    rcases h : o.lo with _ | l
    · simp [h] at hpos
    · refine ⟨l, rfl, ?_⟩
      simp [h] at hpos
      have := (@Rat.num_nonneg l).mp (by omega)
      rcases Rat.le_iff_lt_or_eq.mp this with h' | h'
      · exact h'
      · subst h'; simp at hpos
  have hlb : l ≤ b := by have := hb.lo; rw [hl] at this; exact this
  have hb0 : 0 < b := by grind
  unfold Iv.recip
  apply In_ofX
  · unfold Iv.xHi
    rcases hh : o.hi with _ | h
    · simp [X.le]; exact Rat.le_of_lt (inv_pos_of_pos hb0)
    · have hbh : b ≤ h := by have := hb.hi; rw [hh] at this; exact this
      have hh0 : 0 < h := by grind
      have : h.num ≠ 0 := fun e => by have := Rat.num_eq_zero.mp e; grind
      simp [this, X.le]
      exact inv_le_inv_of_pos hb0 hbh
  · unfold Iv.xLo
    rw [hl]
    have : l.num ≠ 0 := fun e => by have := Rat.num_eq_zero.mp e; grind
    simp [this, X.le]
    exact inv_le_inv_of_pos hln hlb
  · trivial

/-- The reciprocals of an interval of negative values. -/
theorem In_recip_neg {o : Iv} {b : Rat} (hb : o.In b) (hneg : o.hi.any (fun h => h.num < 0) = true) :
    o.recip.In b⁻¹ := by
  obtain ⟨h, hh, hhn⟩ : ∃ h, o.hi = some h ∧ h < 0 := by
    rcases e : o.hi with _ | h
    · simp [e] at hneg
    · refine ⟨h, rfl, ?_⟩
      simp [e] at hneg
      exact (num_neg_iff h).mp hneg
  have hbh : b ≤ h := by have := hb.hi; rw [hh] at this; exact this
  have hb0 : b < 0 := by grind
  unfold Iv.recip
  apply In_ofX
  · unfold Iv.xHi
    rw [hh]
    have : h.num ≠ 0 := fun e => by have := Rat.num_eq_zero.mp e; grind
    simp [this, X.le]
    exact inv_le_inv_of_neg hhn hbh
  · unfold Iv.xLo
    rcases hl : o.lo with _ | l
    · simp [X.le]; exact Rat.le_of_lt (inv_neg_of_neg hb0)
    · have hlb : l ≤ b := by have := hb.lo; rw [hl] at this; exact this
      have : l.num ≠ 0 := fun e => by have := Rat.num_eq_zero.mp e; grind
      simp [this, X.le]
      exact inv_le_inv_of_neg hb0 hlb
  · trivial

theorem mag_le_magnitude {o : Iv} {b m : Rat} (hb : o.In b) (hm : o.magnitude = some m) : mag b ≤ m := by
  unfold Iv.magnitude at hm
  rcases hl : o.lo with _ | lo <;> rcases hh : o.hi with _ | hi <;> simp [hl, hh] at hm
  have h1 : lo ≤ b := by have := hb.lo; rw [hl] at this; exact this
  have h2 : b ≤ hi := by have := hb.hi; rw [hh] at this; exact this
  unfold mag
  subst hm
  split <;> split <;> split <;> grind

/-- A value that is not 0, on a denominator up to `d`, is at least `1/d` away from 0. -/
theorem near_zero {b : Rat} {d : Nat} (hb : b ≠ 0) (hd : b.den ≤ d) : 1 / (d : Rat) ≤ mag b := by
  have hn : 1 ≤ b.num.natAbs := by
    have : b.num ≠ 0 := fun e => hb (Rat.num_eq_zero.mp e)
    omega
  have hdpos : (0 : Rat) < b.den := Rat.natCast_pos.mpr b.den_pos
  have hdd : (b.den : Rat) ≤ d := Rat.natCast_le_natCast.mpr hd
  have hd0 : (0 : Rat) < d := by grind
  have hm := natAbs_num_eq b
  have h1 : (1 : Rat) ≤ (b.num.natAbs : Rat) := by
    have := Rat.natCast_le_natCast.mpr hn; simpa using this
  -- 1 ≤ |b|·den ≤ |b|·d
  have hmag : 0 ≤ mag b := by unfold mag; split <;> grind
  have h2 : mag b * (b.den : Rat) ≤ mag b * (d : Rat) := Rat.mul_le_mul_of_nonneg_left hdd hmag
  have h3 : (1 : Rat) ≤ mag b * (d : Rat) := by grind
  have := div_le_div_of_le h3 hd0
  rw [Rat.mul_div_cancel (Rat.ne_of_gt hd0)] at this
  exact this

/-- The bound `over` puts on the denominator of a quotient. -/
theorem DenLe_over {i o : Iv} {a b : Rat} (ha : i.In a) (hb : o.In b) (hb0 : b ≠ 0) :
    DenLe (Iv.overDen i o) (a / b) := by
  unfold Iv.overDen
  rcases hm : o.magnitude with _ | m <;> rcases hd : o.den with _ | d <;> rcases hx : i.den with _ | x <;>
    simp only [DenLe]
  have hbd : b.den ≤ d := by have := hb.den; rw [hd] at this; exact this
  have hax : a.den ≤ x := by have := ha.den; rw [hx] at this; exact this
  have hmb := mag_le_magnitude hb hm
  -- |b.num| = |b|·den ≤ m·d, a whole number, so at most ⌊m·d⌋
  have hnum : (b.num.natAbs : Rat) ≤ m * (d : Rat) := by
    rw [natAbs_num_eq]
    have hdpos : (0 : Rat) ≤ b.den := Rat.le_of_lt (Rat.natCast_pos.mpr b.den_pos)
    have hmag : 0 ≤ mag b := by unfold mag; split <;> grind
    have h1 : mag b * (b.den : Rat) ≤ m * (b.den : Rat) := Rat.mul_le_mul_of_nonneg_right hmb hdpos
    have hm0 : 0 ≤ m := Rat.le_trans hmag hmb
    have h2 : m * (b.den : Rat) ≤ m * (d : Rat) := Rat.mul_le_mul_of_nonneg_left (Rat.natCast_le_natCast.mpr hbd) hm0
    exact Rat.le_trans h1 h2
  have hfl : (b.num.natAbs : Int) ≤ (m * (d : Rat)).floor := Rat.le_floor_iff.mpr (by rw [Rat.intCast_natCast]; exact hnum)
  have : b.num.natAbs ≤ (m * (d : Rat)).floor.toNat := by omega
  exact Nat.le_trans (den_div_le a b hb0) (Nat.mul_le_mul hax (by omega))

/-- The ends of an interval hold a value. -/
def Iv.Ends (q : Iv) (v : Rat) : Prop := EndLe q.lo v ∧ EndGe q.hi v

theorem mergeParts_left {l : Iv} {upper : Option Iv} {v : Rat} (h : l.In v) :
    ∃ q, Iv.mergeParts (some l) upper = some q ∧ q.Ends v := by
  cases upper with
  | none => exact ⟨l, rfl, h.lo, h.hi⟩
  | some u => exact ⟨_, rfl, (In_hull_left h).lo, (In_hull_left h).hi⟩

theorem mergeParts_right {lower : Option Iv} {u : Iv} {v : Rat} (h : u.In v) :
    ∃ q, Iv.mergeParts lower (some u) = some q ∧ q.Ends v := by
  cases lower with
  | none => exact ⟨u, rfl, h.lo, h.hi⟩
  | some l => exact ⟨_, rfl, (In_hull_right h).lo, (In_hull_right h).hi⟩

/-- Where the divisor holds 0, the quotient is on one side of it or the other. -/
theorem overSplit_In {i o : Iv} {a b : Rat} {d : Nat} (ha : i.In a) (hb : o.In b) (hb0 : b ≠ 0)
    (hod : o.den = some d) : ∃ q, Iv.overSplit i o d = some q ∧ q.Ends (a * b⁻¹) := by
  have hbd : b.den ≤ d := by have := hb.den; rw [hod] at this; exact this
  have hnear := near_zero hb0 hbd
  have hd0 : (0 : Rat) < d := by
    have := Rat.natCast_le_natCast.mpr hbd
    have := Rat.natCast_pos.mpr b.den_pos
    grind
  have hn0 : (0 : Rat) < 1 / (d : Rat) := by
    have := Rat.inv_pos.mpr hd0; rw [Rat.div_def, Rat.one_mul]; exact this
  unfold Iv.overSplit
  dsimp only
  rcases X.trichotomy b with hneg | hz | hpos
  · have hmag : mag b = -b := by unfold mag; simp [hneg]
    have hle : b ≤ 0 - 1 / (d : Rat) := by
      rw [hmag] at hnear
      have := Rat.neg_le_neg hnear
      rw [Rat.neg_neg] at this
      have e : (0 : Rat) - 1 / (d : Rat) = -(1 / (d : Rat)) := by grind
      rw [e]; exact this
    have hlo : o.lo.all (fun l => l.num < 0) = true := by
      rcases hl : o.lo with _ | l
      · rfl
      · have := hb.lo; rw [hl] at this
        simp only [Option.all_some, decide_eq_true_eq]
        exact (num_neg_iff l).mpr (rlt_of_le_of_lt this hneg)
    have hpart : (⟨o.lo, some (0 - 1 / (d : Rat)), o.den⟩ : Iv).In b := ⟨hb.lo, hle, hb.den⟩
    have hr := In_recip_neg hpart (by
      simp only [Option.any_some, decide_eq_true_eq]
      exact (num_neg_iff _).mpr (by grind))
    have hq := In_times ha hr
    simp only [hlo, ite_true]
    exact mergeParts_left hq
  · exact absurd hz hb0
  · have hmag : mag b = b := by unfold mag; simp [show ¬ b < 0 by grind]
    have hge : 1 / (d : Rat) ≤ b := by rw [hmag] at hnear; exact hnear
    have hhi : o.hi.all (fun h => h.num > 0) = true := by
      rcases hh : o.hi with _ | h
      · rfl
      · have := hb.hi; rw [hh] at this
        simp only [Option.all_some, decide_eq_true_eq]
        have hh0 : 0 < h := rlt_of_lt_of_le hpos this
        have := (@Rat.num_nonneg h).mpr (Rat.le_of_lt hh0)
        have : h.num ≠ 0 := fun e => by have := Rat.num_eq_zero.mp e; subst this; exact Rat.lt_irrefl hh0
        omega
    have hpart : (⟨some (1 / (d : Rat)), o.hi, o.den⟩ : Iv).In b := ⟨hge, hb.hi, hb.den⟩
    have hr := In_recip_pos hpart (by
      simp only [Option.any_some, decide_eq_true_eq]
      have := (@Rat.num_nonneg (1 / (d : Rat))).mpr (Rat.le_of_lt hn0)
      have : (1 / (d : Rat)).num ≠ 0 := fun e => by have := Rat.num_eq_zero.mp e; grind
      omega)
    have hq := In_times ha hr
    simp only [hhi, ite_true]
    exact mergeParts_right hq

/-- **The quotient of two intervals holds every quotient of their values by one that is not 0.** -/
theorem In_over {i o : Iv} {a b : Rat} (ha : i.In a) (hb : o.In b) (hb0 : b ≠ 0) : (i.over o).In (a / b) := by
  have hden := DenLe_over ha hb hb0
  unfold Iv.over
  split
  · rename_i hsign
    have hr : o.recip.In b⁻¹ := by
      rcases Bool.or_eq_true_iff.mp hsign with h | h
      · exact In_recip_pos hb h
      · exact In_recip_neg hb h
    have := In_times ha hr
    rw [Rat.div_def]
    exact ⟨this.lo, this.hi, by rw [← Rat.div_def]; exact hden⟩
  · split
    · exact ⟨trivial, trivial, trivial⟩
    · rename_i d hod
      obtain ⟨q, hq, h1, h2⟩ := overSplit_In ha hb hb0 hod
      rw [hq]
      rw [Rat.div_def]
      exact ⟨h1, h2, by rw [← Rat.div_def]; exact hden⟩

-- ── Rounding, and what an interval says of its values ───────────────────────

theorem roundInt_den_le (way : Way) (a g : Rat) : (roundInt way a g).den ≤ g.den := by
  unfold roundInt; dsimp only
  exact den_intCast_mul_le _ g

/-- **Rounding an interval holds every value of it rounded** (DESIGN 5.2): each way is monotone,
    and an end that rounds past int64 falls back to a whole number on its own side of the end. -/
theorem In_round {i : Iv} {a g : Rat} (way : Way) (ha : i.In a) (hg : 0 < g) :
    (i.round way g).In (roundInt way a g) := by
  unfold Iv.round
  apply In_mk'
  · have := ha.lo
    rcases hl : i.lo with _ | l
    · trivial
    · rw [hl] at this
      simp only [Option.bind_some, Iv.roundEnd, Bool.false_eq_true, ite_false]
      by_cases f1 : fits (roundInt way l g) = true
      · simp only [f1, ite_true]; exact roundInt_mono way hg this
      · simp only [f1, Bool.false_eq_true, ite_false]
        by_cases f2 : fits (roundInt way ((l.floor : Int) : Rat) g) = true
        · simp only [f2, ite_true]
          exact roundInt_mono way hg (Rat.le_trans (Rat.floor_le l) this)
        · simp only [f2, Bool.false_eq_true, ite_false]; trivial
  · have := ha.hi
    rcases hh : i.hi with _ | h
    · trivial
    · rw [hh] at this
      simp only [Option.bind_some, Iv.roundEnd, ite_true]
      by_cases f1 : fits (roundInt way h g) = true
      · simp only [f1, ite_true]; exact roundInt_mono way hg this
      · simp only [f1, Bool.false_eq_true, ite_false]
        by_cases f2 : fits (roundInt way ((h.ceil : Int) : Rat) g) = true
        · simp only [f2, ite_true]
          exact roundInt_mono way hg (Rat.le_trans this (Rat.le_ceil (x := h)))
        · simp only [f2, Bool.false_eq_true, ite_false]; trivial
  · exact roundInt_den_le way a g

theorem isExact_sound {i : Iv} {a k : Rat} (ha : i.In a) (h : i.isExact = some k) : a = k := by
  unfold Iv.isExact at h
  rcases hl : i.lo with _ | l <;> rcases hh : i.hi with _ | u <;> simp [hl, hh] at h
  obtain ⟨e, rfl⟩ := h
  have h1 := ha.lo; have h2 := ha.hi
  rw [hl] at h1; rw [hh] at h2
  simp only [EndLe, EndGe] at h1 h2
  subst e
  exact Rat.le_antisymm h2 h1

theorem num_pos_iff (x : Rat) : 0 < x.num ↔ 0 < x := by
  constructor
  · intro h
    have h0 := (@Rat.num_nonneg x).mp (Int.le_of_lt h)
    rcases Rat.le_iff_lt_or_eq.mp h0 with h' | h'
    · exact h'
    · subst h'; simp at h
  · intro h
    have := (@Rat.num_nonneg x).mpr (Rat.le_of_lt h)
    have : x.num ≠ 0 := fun e => by have := Rat.num_eq_zero.mp e; subst this; exact Rat.lt_irrefl h
    omega

theorem hasZero_sound {i : Iv} {a : Rat} (ha : i.In a) (h : i.hasZero = false) : a ≠ 0 := by
  intro e
  subst e
  unfold Iv.hasZero at h
  simp only [Bool.and_eq_false_iff] at h
  rcases h with h | h
  · rcases hl : i.lo with _ | l
    · simp [hl] at h
    · simp only [hl, Option.all_some, decide_eq_false_iff_not, Int.not_le] at h
      have h1 := ha.lo; rw [hl] at h1
      have := (num_pos_iff l).mp h
      exact absurd h1 (Rat.not_le.mpr this)
  · rcases hh : i.hi with _ | u
    · simp [hh] at h
    · simp only [hh, Option.all_some, decide_eq_false_iff_not, Int.not_le] at h
      have h2 := ha.hi; rw [hh] at h2
      have := (num_neg_iff u).mp (by omega)
      exact absurd h2 (Rat.not_le.mpr this)

theorem below_sound {x y : Iv} {a b : Rat} (ha : x.In a) (hb : y.In b) (h : x.below y = true) : a < b := by
  unfold Iv.below at h
  rcases hh : x.hi with _ | u <;> rcases hl : y.lo with _ | l <;> simp [hh, hl] at h
  have h1 := ha.hi; have h2 := hb.lo
  rw [hh] at h1; rw [hl] at h2
  simp only [EndLe, EndGe] at h1 h2
  exact rlt_of_le_of_lt h1 (rlt_of_lt_of_le h h2)

end KanameCore
