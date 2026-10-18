/-
  Rounding (kaname's DESIGN 3.3, 15.2.1).

  `roundInt` is the integer procedure the reference interpreter and the generated code follow: the
  magnitude of `x` in steps of `g` divided as whole numbers, the quotient taken or one past it by
  the way of rounding. `roundQ` says the same of rationals with `floor`. The theorems:

  - `roundInt_eq_roundQ`: the procedure gives what the rational definition says, for every way and
    every positive step (what `tests/rounding.rs` checks against rulec on 360,600 cases);
  - `roundInt_onGrid`, `roundInt_roundsTo`: the answer is a whole number of steps, between the
    value rounded down to the grid and rounded up to it, the form rulec's certificates hold a
    rounding to (`RulecCert.Carried.roundsTo`);
  - `roundInt_mono`: every way is monotone, which the analysis of ranges rounds the ends of an
    interval by (DESIGN 5.2).
-/
import KanameCore.Num

namespace KanameCore

open RulecCert

theorem rat_eq_num_div_den (x : Rat) : x = (x.num : Rat) / (x.den : Rat) := by
  rw [← Rat.mkRat_eq_div, Rat.mkRat_self]

theorem num_neg_iff (x : Rat) : x.num < 0 ↔ x < 0 := by
  have h := @Rat.num_nonneg x
  constructor
  · intro hn
    rcases Rat.le_total (a := 0) (b := x) with h0 | h0
    · have := h.mpr h0; omega
    · rcases Rat.le_iff_lt_or_eq.mp h0 with hlt | heq
      · exact hlt
      · subst heq; simp at hn
  · intro hx
    rcases Int.lt_or_le x.num 0 with hn | hn
    · exact hn
    · have := h.mp hn
      grind

theorem num_pos_of_pos {g : Rat} (hg : 0 < g) : 0 < g.num := by
  have h1 := (@Rat.num_nonneg g).mpr (Rat.le_of_lt hg)
  rcases Int.lt_or_eq_of_le h1 with h | h
  · exact h
  · have : g = 0 := Rat.num_eq_zero.mp h.symm
    subst this
    exact absurd hg (Rat.lt_irrefl)

theorem den_ne_zero (x : Rat) : (x.den : Rat) ≠ 0 :=
  Rat.ne_of_gt (Rat.natCast_pos.mpr x.den_pos)

theorem natAbs_cast_of_neg {a : Int} (h : a < 0) : ((a.natAbs : Nat) : Rat) = -((a : Int) : Rat) := by
  have e : ((a.natAbs : Nat) : Int) = -a := by omega
  rw [← Rat.intCast_natCast, e, Rat.intCast_neg]

theorem natAbs_cast_of_nonneg {a : Int} (h : 0 ≤ a) : ((a.natAbs : Nat) : Rat) = ((a : Int) : Rat) := by
  have e : ((a.natAbs : Nat) : Int) = a := by omega
  rw [← Rat.intCast_natCast, e]

theorem mag_eq (x : Rat) : mag x = (x.num.natAbs : Rat) / (x.den : Rat) := by
  unfold mag
  have hx := rat_eq_num_div_den x
  have hd := den_ne_zero x
  split
  · rename_i h
    rw [natAbs_cast_of_neg ((num_neg_iff x).mpr h)]
    grind
  · rename_i h
    rw [natAbs_cast_of_nonneg (by have hn : ¬ (x.num < 0) := fun hn => h ((num_neg_iff x).mp hn); omega)]
    exact hx

/-- The magnitude in steps, as the procedure divides it: `|n·gd| / (d·gn)`. -/
theorem steps_eq (x g : Rat) (hg : 0 < g) :
    mag x / g = (((x.num * g.den).natAbs : Nat) : Rat) / ((x.den * g.num.natAbs : Nat) : Rat) := by
  have hgn := num_pos_of_pos hg
  have hg' : g = ((g.num.natAbs : Nat) : Rat) / (g.den : Rat) := by
    rw [natAbs_cast_of_nonneg (Int.le_of_lt hgn)]
    exact rat_eq_num_div_den g
  rw [mag_eq, Int.natAbs_mul]
  have hgd : (g.den : Rat) ≠ 0 := den_ne_zero g
  have hxd : (x.den : Rat) ≠ 0 := den_ne_zero x
  have hgnr : ((g.num.natAbs : Nat) : Rat) ≠ 0 := by
    have : 0 < g.num.natAbs := by omega
    exact Rat.ne_of_gt (Rat.natCast_pos.mpr this)
  have e0 : ((g.den : Int).natAbs : Nat) = g.den := by simp
  rw [e0, Rat.natCast_mul, Rat.natCast_mul]
  conv => lhs; rw [hg']
  grind

/-- The floor of a quotient of whole numbers, as `q` and `r` divide it. -/
theorem floor_div_eq (T y q r : Nat) (hy : 0 < y) (hT : T = y * q + r) (hr : r < y) :
    ((T : Rat) / (y : Rat)).floor = (q : Int) := by
  have hyr : (0 : Rat) < y := Rat.natCast_pos.mpr hy
  have hTr : (T : Rat) = (y : Rat) * (q : Rat) + (r : Rat) := by
    rw [hT, Rat.natCast_add, Rat.natCast_mul]
  have hrr : (r : Rat) < (y : Rat) := Rat.natCast_lt_natCast.mpr hr
  have hr0 : (0 : Rat) ≤ (r : Rat) := by have := Rat.natCast_le_natCast.mpr (Nat.zero_le r); simpa using this
  apply Int.le_antisymm
  · have : (T : Rat) / (y : Rat) < (((q + 1 : Nat) : Int) : Rat) := by
      rw [Rat.div_lt_iff hyr, Rat.intCast_natCast, Rat.natCast_add]
      have : ((1 : Nat) : Rat) = 1 := rfl
      rw [this]
      grind
    have := Rat.floor_lt_iff.mpr this
    omega
  · apply Rat.le_floor_iff.mpr
    have : ¬ ((T : Rat) / (y : Rat) < ((q : Int) : Rat)) := by
      rw [Rat.div_lt_iff hyr, Rat.intCast_natCast]
      grind
    exact Rat.not_lt.mp this

/-- What is left of a quotient of whole numbers past its floor: the remainder over the divisor. -/
theorem sub_floor_eq (T y q r : Nat) (hy : 0 < y) (hT : T = y * q + r) :
    (T : Rat) / (y : Rat) - (q : Rat) = (r : Rat) / (y : Rat) := by
  have hyr : (y : Rat) ≠ 0 := Rat.ne_of_gt (Rat.natCast_pos.mpr hy)
  have hTr : (T : Rat) = (y : Rat) * (q : Rat) + (r : Rat) := by
    rw [hT, Rat.natCast_add, Rat.natCast_mul]
  rw [hTr]
  grind

/-- A remainder over its divisor against a half. -/
theorem half_lt_iff (r y : Nat) (hy : 0 < y) : (r : Rat) / (y : Rat) < 1 / 2 ↔ 2 * r < y := by
  have hyr : (0 : Rat) < y := Rat.natCast_pos.mpr hy
  rw [Rat.div_lt_iff hyr]
  have e : ((2 * r : Nat) : Rat) = 2 * (r : Rat) := by rw [Rat.natCast_mul]; rfl
  constructor
  · intro h
    have : ((2 * r : Nat) : Rat) < (y : Rat) := by rw [e]; grind
    exact Rat.natCast_lt_natCast.mp this
  · intro h
    have := Rat.natCast_lt_natCast.mpr h
    rw [e] at this
    grind

theorem half_gt_iff (r y : Nat) (hy : 0 < y) : 1 / 2 < (r : Rat) / (y : Rat) ↔ y < 2 * r := by
  have hyr : (0 : Rat) < y := Rat.natCast_pos.mpr hy
  rw [Rat.lt_div_iff hyr]
  have e : ((2 * r : Nat) : Rat) = 2 * (r : Rat) := by rw [Rat.natCast_mul]; rfl
  constructor
  · intro h
    have : (y : Rat) < ((2 * r : Nat) : Rat) := by rw [e]; grind
    exact Rat.natCast_lt_natCast.mp this
  · intro h
    have := Rat.natCast_lt_natCast.mpr h
    rw [e] at this
    grind

theorem frac_zero_iff (r y : Nat) (hy : 0 < y) : (r : Rat) / (y : Rat) = 0 ↔ r = 0 := by
  have hyr : (y : Rat) ≠ 0 := Rat.ne_of_gt (Rat.natCast_pos.mpr hy)
  constructor
  · intro h
    have h2 : (r : Rat) = 0 := by
      have := congrArg (· * (y : Rat)) h
      rw [Rat.div_mul_cancel hyr] at this
      grind
    have : ((r : Nat) : Rat) = ((0 : Nat) : Rat) := by rw [h2]; rfl
    exact Rat.natCast_inj.mp this
  · intro h; subst h; grind

/-- **The steps the procedure takes are the steps the rational definition takes.** -/
theorem stepsN_eq_stepsQ (way : Way) (T y q r : Nat) (hy : 0 < y) (hT : T = y * q + r) (hr : r < y) :
    ((stepsN way q r y : Nat) : Int) = stepsQ way ((T : Rat) / (y : Rat)) := by
  have hfl := floor_div_eq T y q r hy hT hr
  have hfr := sub_floor_eq T y q r hy hT
  unfold stepsQ stepsN
  rw [hfl]
  simp only [Rat.intCast_natCast, hfr]
  cases way with
  | down => simp [past]
  | up =>
    by_cases h0 : r = 0
    · have : (T : Rat) / (y : Rat) = (q : Rat) := by
        have := hfr; rw [h0] at this; simp at this; grind
      simp [past, h0, this]
    · have hne : ¬ ((T : Rat) / (y : Rat) = (q : Rat)) := by
        intro h
        have : (r : Rat) / (y : Rat) = 0 := by rw [← hfr, h]; grind
        exact h0 ((frac_zero_iff r y hy).mp this)
      simp [past, h0, hne]
  | halfUp =>
    by_cases hh : 2 * r < y
    · have : past .halfUp q r y = false := by simp [past]; omega
      simp [this, (half_lt_iff _ _ hy).mpr hh]
    · have hp : past .halfUp q r y = true := by simp [past]; omega
      have hn : ¬ ((r : Rat) / (y : Rat) < 1 / 2) := fun h => hh ((half_lt_iff _ _ hy).mp h)
      simp [hp, hn]
  | halfDown =>
    by_cases hh : y < 2 * r
    · have hp : past .halfDown q r y = true := by simp [past]; omega
      have hn : ¬ ((r : Rat) / (y : Rat) ≤ 1 / 2) := fun h =>
        absurd ((half_gt_iff _ _ hy).mpr hh) (Rat.not_lt.mpr h)
      simp [hp, hn]
    · have hp : past .halfDown q r y = false := by simp [past]; omega
      have hl : (r : Rat) / (y : Rat) ≤ 1 / 2 := Rat.not_lt.mp fun h => hh ((half_gt_iff _ _ hy).mp h)
      simp [hp, hl]
  | halfEven =>
    by_cases hl : 2 * r < y
    · have : past .halfEven q r y = false := by simp [past]; omega
      simp [this, (half_lt_iff _ _ hy).mpr hl]
    · have hnl : ¬ ((r : Rat) / (y : Rat) < 1 / 2) := fun h => hl ((half_lt_iff _ _ hy).mp h)
      by_cases hg2 : y < 2 * r
      · have hp : past .halfEven q r y = true := by simp [past]; omega
        have hgt : (1 : Rat) / 2 < (r : Rat) / (y : Rat) := (half_gt_iff _ _ hy).mpr hg2
        simp [hp, hnl, hgt]
      · have hngt : ¬ ((1 : Rat) / 2 < (r : Rat) / (y : Rat)) := fun h => hg2 ((half_gt_iff _ _ hy).mp h)
        by_cases ho : q % 2 = 1
        · have hp : past .halfEven q r y = true := by simp [past]; omega
          have : ¬ ((q : Int) % 2 = 0) := by omega
          simp [hp, hnl, hngt, this]
        · have hp : past .halfEven q r y = false := by simp [past]; omega
          have : ((q : Int) % 2 = 0) := by omega
          simp [hp, hnl, hngt, this]

theorem sign_iff (x g : Rat) : x.num * (g.den : Int) < 0 ↔ x < 0 := by
  have hd : (0 : Int) < g.den := by have := g.den_pos; omega
  constructor
  · intro h
    apply (num_neg_iff x).mp
    rcases Int.lt_or_le x.num 0 with hn | hn
    · exact hn
    · have := Int.mul_nonneg hn (Int.le_of_lt hd); omega
  · intro h
    exact Int.mul_neg_of_neg_of_pos ((num_neg_iff x).mpr h) hd

/-- **The integer procedure is the rational definition.** -/
theorem roundInt_eq_roundQ (way : Way) (x g : Rat) (hg : 0 < g) : roundInt way x g = roundQ way x g := by
  have hgn := num_pos_of_pos hg
  have hy : 0 < x.den * g.num.natAbs := Nat.mul_pos x.den_pos (by omega)
  unfold roundInt roundQ
  dsimp only
  rw [steps_eq x g hg]
  rw [← stepsN_eq_stepsQ way _ _ _ _ hy (Nat.div_add_mod _ _).symm (Nat.mod_lt _ hy)]
  by_cases h : x < 0
  · simp [h, (sign_iff x g).mpr h]
  · have : ¬ (x.num * (g.den : Int) < 0) := fun h' => h ((sign_iff x g).mp h')
    simp [h, this]

-- ── What any way gives ──────────────────────────────────────────────────────

theorem floor_eq_of (a : Rat) (n : Int) (h1 : (n : Rat) ≤ a) (h2 : a < (n : Rat) + 1) : a.floor = n := by
  apply Int.le_antisymm
  · have : a < ((n + 1 : Int) : Rat) := by rw [Rat.intCast_add]; exact h2
    have := Rat.floor_lt_iff.mpr this
    omega
  · exact Rat.le_floor_iff.mpr h1

theorem floor_props (a : Rat) : ((a.floor : Int) : Rat) ≤ a ∧ a < ((a.floor : Int) : Rat) + 1 := by
  refine ⟨Rat.floor_le a, ?_⟩
  have := Rat.lt_floor_add_one a
  rw [Rat.intCast_add] at this
  exact this

/-- `-⌊-t⌋`: the whole part, or one more where `t` is not whole. -/
theorem neg_floor_neg (t : Rat) :
    -((-t).floor) = t.floor + (if t = ((t.floor : Int) : Rat) then 0 else 1) := by
  have ⟨h1, h2⟩ := floor_props t
  split
  · rename_i h
    have : (-t).floor = -t.floor := by
      apply floor_eq_of
      · rw [Rat.intCast_neg]; grind
      · rw [Rat.intCast_neg]; grind
    omega
  · rename_i h
    have hlt : ((t.floor : Int) : Rat) < t := by
      rcases Rat.le_iff_lt_or_eq.mp h1 with h' | h'
      · exact h'
      · exact absurd h'.symm h
    have : (-t).floor = -t.floor - 1 := by
      apply floor_eq_of
      · rw [Rat.intCast_sub, Rat.intCast_neg]; grind
      · rw [Rat.intCast_sub, Rat.intCast_neg]; grind
    omega

/-- Every way takes the whole part of the steps, or one more, and the whole part where the steps
    are whole. -/
theorem stepsQ_bounds (way : Way) (t : Rat) :
    t.floor ≤ stepsQ way t ∧ stepsQ way t ≤ -((-t).floor) := by
  rw [neg_floor_neg]
  have ⟨h1, h2⟩ := floor_props t
  unfold stepsQ
  cases way with
  | down => simp only; split <;> omega
  | up => simp only; split <;> constructor <;> omega
  | halfUp =>
    simp only
    split
    · split
      · rename_i ht _
        constructor <;> omega
      · constructor <;> omega
    · split
      · rename_i hh ht
        exfalso; apply hh; rw [← ht]; grind
      · constructor <;> omega
  | halfDown =>
    simp only
    split
    · split <;> constructor <;> omega
    · split
      · rename_i hh ht
        exfalso; apply hh; rw [← ht]; grind
      · constructor <;> omega
  | halfEven =>
    simp only
    split
    · split <;> constructor <;> omega
    · split
      · split
        · rename_i _ hh ht
          exfalso; rw [← ht] at hh; grind
        · constructor <;> omega
      · split
        · split
          · rename_i _ _ _ ht
            exfalso
            rename_i h3 h4 _
            rw [← ht] at h3
            grind
          · constructor <;> omega
        · split
          · rename_i _ _ _ ht
            exfalso
            rename_i h3 h4 _
            rw [← ht] at h3
            grind
          · constructor <;> omega

theorem int_mul_le_mul {a b : Int} {g : Rat} (h : a ≤ b) (hg : 0 < g) : (a : Rat) * g ≤ (b : Rat) * g :=
  Rat.mul_le_mul_of_nonneg_right (Rat.intCast_le_intCast.mpr h) (Rat.le_of_lt hg)

theorem div_neg_eq (x g : Rat) : -x / g = -(x / g) := by grind

/-- **What any way gives** (rulec's `roundsTo`): a whole number of steps, between the value rounded
    down to the grid and rounded up to it. -/
theorem roundInt_roundsTo (way : Way) (x g : Rat) (hg : 0 < g) : Carried.roundsTo g x (roundInt way x g) := by
  rw [roundInt_eq_roundQ way x g hg]
  unfold roundQ Carried.roundsTo floorTo ceilTo rfloor rceil mag
  dsimp only
  have e1 : -x / g = -(x / g) := by grind
  have e2 : -(-(x / g)) = x / g := by grind
  refine ⟨⟨_, rfl⟩, ?_, ?_⟩
  · by_cases h : x < 0
    · simp only [h, ite_true]
      have hb := stepsQ_bounds way (-x / g)
      generalize stepsQ way (-x / g) = k at hb ⊢
      rw [e1, e2] at hb
      apply int_mul_le_mul _ hg; omega
    · simp only [h, ite_false]
      have hb := stepsQ_bounds way (x / g)
      generalize stepsQ way (x / g) = k at hb ⊢
      apply int_mul_le_mul _ hg; omega
  · by_cases h : x < 0
    · simp only [h, ite_true]
      have hb := stepsQ_bounds way (-x / g)
      generalize stepsQ way (-x / g) = k at hb ⊢
      rw [e1] at hb
      apply int_mul_le_mul _ hg; omega
    · simp only [h, ite_false]
      have hb := stepsQ_bounds way (x / g)
      generalize stepsQ way (x / g) = k at hb ⊢
      apply int_mul_le_mul _ hg; omega

theorem roundInt_onGrid (way : Way) (x g : Rat) : onGrid g (roundInt way x g) := ⟨_, rfl⟩

-- ── Every way is monotone ───────────────────────────────────────────────────

theorem floor_mono {a b : Rat} (h : a ≤ b) : a.floor ≤ b.floor := Rat.floor_monotone h

theorem stepsQ_up (t : Rat) : stepsQ .up t = -((-t).floor) := by
  rw [neg_floor_neg]; unfold stepsQ; simp only; split <;> omega

theorem stepsQ_halfUp (t : Rat) : stepsQ .halfUp t = (t + 1 / 2).floor := by
  have ⟨h1, h2⟩ := floor_props t
  unfold stepsQ; simp only
  split
  · rename_i h
    exact (floor_eq_of _ _ (by grind) (by grind)).symm
  · rename_i h
    exact (floor_eq_of _ (t.floor + 1) (by rw [Rat.intCast_add]; grind) (by rw [Rat.intCast_add]; grind)).symm

theorem stepsQ_halfDown (t : Rat) : stepsQ .halfDown t = -((1 / 2 - t).floor) := by
  have ⟨h1, h2⟩ := floor_props t
  unfold stepsQ; simp only
  split
  · rename_i h
    have : (1 / 2 - t).floor = -t.floor := floor_eq_of _ _ (by rw [Rat.intCast_neg]; grind) (by rw [Rat.intCast_neg]; grind)
    omega
  · rename_i h
    have : (1 / 2 - t).floor = -t.floor - 1 :=
      floor_eq_of _ _ (by rw [Rat.intCast_sub, Rat.intCast_neg]; grind) (by rw [Rat.intCast_sub, Rat.intCast_neg]; grind)
    omega

/-- Half to the even neighbour lies between half toward 0 and half away from it. -/
theorem stepsQ_halfEven_between (t : Rat) :
    -((1 / 2 - t).floor) ≤ stepsQ .halfEven t ∧ stepsQ .halfEven t ≤ (t + 1 / 2).floor := by
  rw [← stepsQ_halfDown, ← stepsQ_halfUp]
  unfold stepsQ; simp only
  by_cases h1 : t - ((t.floor : Int) : Rat) < 1 / 2
  · have h2 : t - ((t.floor : Int) : Rat) ≤ 1 / 2 := Rat.le_of_lt h1
    simp only [h1, h2, ite_true]; omega
  · by_cases h3 : t - ((t.floor : Int) : Rat) > 1 / 2
    · have h4 : ¬ (t - ((t.floor : Int) : Rat) ≤ 1 / 2) := fun h => absurd h3 (Rat.not_lt.mpr h)
      simp only [h1, h3, h4, ite_true, ite_false]; omega
    · have h4 : t - ((t.floor : Int) : Rat) ≤ 1 / 2 := Rat.not_lt.mp h3
      simp only [h1, h3, h4, ite_true, ite_false]
      split <;> omega

theorem stepsQ_mono (way : Way) {t1 t2 : Rat} (h : t1 ≤ t2) : stepsQ way t1 ≤ stepsQ way t2 := by
  cases way with
  | down => exact floor_mono h
  | up =>
    rw [stepsQ_up, stepsQ_up]
    have := floor_mono (Rat.neg_le_neg h)
    omega
  | halfUp =>
    rw [stepsQ_halfUp, stepsQ_halfUp]
    exact floor_mono (by grind)
  | halfDown =>
    rw [stepsQ_halfDown, stepsQ_halfDown]
    have : (1 / 2 - t2).floor ≤ (1 / 2 - t1).floor := floor_mono (by grind)
    omega
  | halfEven =>
    rcases Rat.le_iff_lt_or_eq.mp h with hlt | heq
    · have ⟨_, a2⟩ := stepsQ_halfEven_between t1
      have ⟨b1, _⟩ := stepsQ_halfEven_between t2
      -- ⌊t1 + 1/2⌋ ≤ ⌈t2 - 1/2⌉ where t1 < t2
      have hn := floor_props (t1 + 1 / 2)
      have hm := floor_props (1 / 2 - t2)
      have : (t1 + 1 / 2).floor ≤ -((1 / 2 - t2).floor) := by
        have : ((t1 + 1 / 2).floor : Rat) + ((1 / 2 - t2).floor : Rat) < 1 := by grind
        have : (t1 + 1 / 2).floor + (1 / 2 - t2).floor < 1 := by
          have h' : (((t1 + 1 / 2).floor + (1 / 2 - t2).floor : Int) : Rat) < ((1 : Int) : Rat) := by
            rw [Rat.intCast_add]; exact this
          exact Rat.intCast_lt_intCast.mp h'
        omega
      omega
    · rw [heq]; exact Int.le_refl _

theorem stepsQ_nonneg (way : Way) {t : Rat} (h : 0 ≤ t) : 0 ≤ stepsQ way t := by
  have h0 : (0 : Int) ≤ t.floor := Rat.le_floor_iff.mpr (by simpa using h)
  have := (stepsQ_bounds way t).1
  omega

/-- **Every way is monotone**: a larger value rounds to no smaller one. -/
theorem roundInt_mono (way : Way) {x y g : Rat} (hg : 0 < g) (h : x ≤ y) : roundInt way x g ≤ roundInt way y g := by
  rw [roundInt_eq_roundQ way x g hg, roundInt_eq_roundQ way y g hg]
  unfold roundQ mag
  dsimp only
  have hdiv : ∀ {a b : Rat}, a ≤ b → a / g ≤ b / g := fun hab => div_le_div_right hab hg
  apply int_mul_le_mul _ hg
  by_cases hx : x < 0
  · by_cases hy : y < 0
    · simp only [hx, hy, ite_true]
      have := stepsQ_mono way (hdiv (Rat.neg_le_neg h))
      omega
    · simp only [hx, hy, ite_true, ite_false]
      have h1 := stepsQ_nonneg way (t := -x / g) (by have := hdiv (show (0 : Rat) ≤ -x by grind); grind)
      have h2 := stepsQ_nonneg way (t := y / g) (by have := hdiv (show (0 : Rat) ≤ y by grind); grind)
      omega
  · have hy : ¬ y < 0 := by grind
    simp only [hx, hy, ite_false]
    exact stepsQ_mono way (hdiv h)

end KanameCore
