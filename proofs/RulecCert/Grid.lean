/-
  The grid a value the rule computes sits on (rulec's §15.322).

  A rounding puts its value on the multiples of its step, whatever it rounds: `down(x, 10)` is a
  whole number of tens. A sum of two such values is on a grid both sit on, a multiple or a quotient
  by a constant on that multiple or quotient of the grid, and `min` and `max` answer one of their two
  arguments. So a value can sit on a grid that leaves a coordinate of a table's axis empty even where
  the interval the value is forced into (`interval`) crosses it: `down(x, 10)` never lands strictly
  between 50 and 60. `gridOf` works the grid out from the expression, and `eval_onGridAt_gridOf`
  says every value the expression evaluates to sits on it; `gridMisses` decides that no value of a
  grid lies on a coordinate inside an interval, and `gridMisses_sound` says what a `true` settles.
-/
import RulecCert.Values

namespace RulecCert

/-- A value sits on the grid `(o, s)` when it is `o` and a whole number of `s`. A step of zero is
    the one value `o`. -/
def onGridAt (g : Rat × Rat) (v : Rat) : Prop := onGrid g.2 (v - g.1)

/-- A step both `a` and `b` are whole numbers of: the largest, worked out from their numerators and
    denominators, and held to that by a test, so that nothing below rests on the arithmetic. -/
def gridCandidate (a b : Rat) : Rat :=
  if a == 0 then absR b else if b == 0 then absR a
  else ((Int.gcd (a.num * b.den) (b.num * a.den) : Nat) : Rat) / ((a.den * b.den : Nat) : Rat)

def commonGrid (a b : Rat) : Option Rat :=
  if gridB (gridCandidate a b) a && gridB (gridCandidate a b) b then some (gridCandidate a b) else none

/-- The number a literal of the certificate stands for, and nothing for any other node. -/
def litOf : Expr → Option Rat
  | .lit v _ => some v
  | _ => none

/-- The grid of one node: the one value `pt` pins the node to, where it pins it (rulec's §15.325),
    and what the node's own rule works out otherwise. -/
def gridPick (pt : Expr → Option Rat) (e : Expr) (g : Option (Rat × Rat)) : Option (Rat × Rat) :=
  match pt e with
  | some v => some (v, 0)
  | none => g

/-- **The grid an expression's values sit on** (rulec's §15.322): `(origin, step)`. A node `pt` pins
    to one value is that value (§15.325: what the held names leave a part of the expression). Otherwise
    a literal is one value; a name is on the grid `names` gives it; a sum or a difference is on a step
    both sides' steps are whole numbers of; a product where one side is one value, on that multiple of
    the other side's grid, and a quotient by a constant on that quotient of the grid; `min` and `max`
    on a step both steps and the distance between the two origins are whole numbers of; a rounding
    whose mode the certificate names on the multiples of its step. Nothing for anything else. -/
def gridOf (pt : Expr → Option Rat) (names : String → Option (Rat × Rat)) : Expr → Option (Rat × Rat)
  | .name n => gridPick pt (.name n) (names n)
  | .lit v t => gridPick pt (.lit v t) (some (v, 0))
  | .add a b => gridPick pt (.add a b) (
    match gridOf pt names a, gridOf pt names b with
    | some (o, s), some (p, t) => (commonGrid s t).map (fun g => (o + p, g))
    | _, _ => none)
  | .sub a b => gridPick pt (.sub a b) (
    match gridOf pt names a, gridOf pt names b with
    | some (o, s), some (p, t) => (commonGrid s t).map (fun g => (o - p, g))
    | _, _ => none)
  | .mul a b => gridPick pt (.mul a b) (
    match gridOf pt names a, gridOf pt names b with
    | some (o, s), some (p, q) =>
      if q == 0 then some (o * p, absR (s * p))
      else if s == 0 then some (o * p, absR (q * o))
      else none
    | _, _ => none)
  | .divc a k kt => gridPick pt (.divc a k kt) (
    if k == 0 then none else (gridOf pt names a).map (fun g => (g.1 / k, absR (g.2 / k))))
  | .minOf a b => gridPick pt (.minOf a b) (
    match gridOf pt names a, gridOf pt names b with
    | some (o, s), some (p, t) =>
      match commonGrid s t with
      | some g => (commonGrid g (o - p)).map (fun h => (o, h))
      | none => none
    | _, _ => none)
  | .maxOf a b => gridPick pt (.maxOf a b) (
    match gridOf pt names a, gridOf pt names b with
    | some (o, s), some (p, t) =>
      match commonGrid s t with
      | some g => (commonGrid g (o - p)).map (fun h => (o, h))
      | none => none
    | _, _ => none)
  | .roundTo a g (some m) => gridPick pt (.roundTo a g (some m)) (if 0 < g then some (0, g) else none)
  | .divRound a b g tw (some m) => gridPick pt (.divRound a b g tw (some m)) (if 0 < g then some (0, g) else none)
  | e => gridPick pt e none

/-- **Whether no value of the grid `g` lies on coordinate `c` inside the closed interval `I`**
    (rulec's §15.322). A point is missed when it is outside `I` or off the grid; an interval when the
    whole numbers of steps that would put a value of the grid above both its low ends and below both
    its high ends run out before they start. -/
def gridMisses (g : Rat × Rat) (c : Coord) (I : Rat × Rat) : Bool :=
  match c with
  | .exactly a => !(decide (I.1 ≤ a) && decide (a ≤ I.2) && gridB g.2 (a - g.1))
  | .between lo hi =>
    if g.2 == 0 then !(coordHoldsB c g.1 && decide (I.1 ≤ g.1) && decide (g.1 ≤ I.2))
    else if g.2 < 0 then false
    else
      let kl0 : Int := rceil ((I.1 - g.1) / g.2)
      let kl : Int := match lo with
        | some a => max kl0 (rfloor ((a - g.1) / g.2) + 1)
        | none => kl0
      let kr0 : Int := rfloor ((I.2 - g.1) / g.2)
      let kr : Int := match hi with
        | some b => min kr0 (rceil ((b - g.1) / g.2) - 1)
        | none => kr0
      decide (kr < kl)

/-! ### What the grid settles -/

theorem onGrid_add {g a b : Rat} (ha : onGrid g a) (hb : onGrid g b) : onGrid g (a + b) := by
  obtain ⟨j, rfl⟩ := ha
  obtain ⟨k, rfl⟩ := hb
  exact ⟨j + k, by rw [Rat.intCast_add, Rat.add_mul]⟩

theorem onGrid_neg {g a : Rat} (ha : onGrid g a) : onGrid g (-a) := by
  obtain ⟨j, rfl⟩ := ha
  exact ⟨-j, by rw [Rat.intCast_neg, Rat.neg_mul]⟩

theorem onGrid_sub {g a b : Rat} (ha : onGrid g a) (hb : onGrid g b) : onGrid g (a - b) := by
  rw [Rat.sub_eq_add_neg]
  exact onGrid_add ha (onGrid_neg hb)

theorem onGrid_mul_right {g a : Rat} (k : Rat) (ha : onGrid g a) : onGrid (g * k) (a * k) := by
  obtain ⟨j, rfl⟩ := ha
  exact ⟨j, by rw [Rat.mul_assoc]⟩

theorem onGrid_absR {g a : Rat} (ha : onGrid g a) : onGrid (absR g) a := by
  unfold absR
  split
  · exact ha
  · obtain ⟨j, rfl⟩ := ha
    exact ⟨-j, by rw [Rat.intCast_neg]; grind⟩

theorem onGrid_zero (g : Rat) : onGrid g 0 := ⟨0, by simp⟩

theorem commonGrid_spec {a b g : Rat} (h : commonGrid a b = some g) : onGrid g a ∧ onGrid g b := by
  unfold commonGrid at h
  split at h
  · rename_i hb
    simp only [Bool.and_eq_true] at hb
    simp only [Option.some.injEq] at h
    subst h
    exact ⟨onGrid_of_gridB hb.1, onGrid_of_gridB hb.2⟩
  · exact absurd h (by simp)

theorem gridB_of_onGrid {g v : Rat} (h : onGrid g v) : gridB g v = true := by
  obtain ⟨k, rfl⟩ := h
  unfold gridB
  by_cases hg : g = 0
  · subst hg
    simp
  · have : (k : Rat) * g / g = (k : Rat) := Rat.mul_div_cancel hg
    rw [this, Rat.num_intCast]
    simp

/-- A step of zero is the one value: whatever sits on it is zero away from the origin. -/
theorem onGrid_zero_eq {d : Rat} (h : onGrid 0 d) : d = 0 := by
  obtain ⟨k, hk⟩ := h
  rw [hk, Rat.mul_zero]

/-- **What `gridPick` settles**: where `pt` pins a node only to the value the node evaluates to, the
    node's value sits on the grid `gridPick` picks, if it sits on the one the node's own rule gives. -/
theorem gridPick_sound {pt : Expr → Option Rat} {e : Expr} {g0 : Option (Rat × Rat)} {g : Rat × Rat}
    {v : Rat} (hpt : ∀ w, pt e = some w → v = w) (h0 : ∀ g', g0 = some g' → onGridAt g' v)
    (hg : gridPick pt e g0 = some g) : onGridAt g v := by
  unfold gridPick at hg
  split at hg
  · rename_i w hw
    simp only [Option.some.injEq] at hg
    subst hg
    show onGrid 0 (v - w)
    rw [hpt w hw, Rat.sub_self]
    exact onGrid_zero 0
  · exact h0 g hg

/-- **Every value an expression evaluates to sits on the grid `gridOf` works out for it**, where
    every name sits on the grid `names` gives it, and `pt` pins a node only to the value it
    evaluates to. -/
theorem eval_onGridAt_gridOf {pt : Expr → Option Rat} {names : String → Option (Rat × Rat)}
    {env : String → Option UVal} {rnd : Rat → Rat → Rat}
    (hpt : ∀ e w v t, pt e = some w → eval rnd env e = some (v, t) → v = w)
    (henv : ∀ n g v t, names n = some g → env n = some (v, t) → onGridAt g v) :
    ∀ (e : Expr) (g : Rat × Rat) (v : Rat) (t : Ty),
      gridOf pt names e = some g → eval rnd env e = some (v, t) → onGridAt g v := by
  intro e
  induction e with
  | name n =>
    intro g v t hg hv
    simp only [gridOf] at hg
    exact gridPick_sound (fun w hw => hpt _ w v t hw hv) (fun g' h => henv n g' v t h hv) hg
  | lit w tw =>
    intro g v t hg hv
    simp only [gridOf] at hg
    refine gridPick_sound (fun x hx => hpt _ x v t hx hv) ?_ hg
    intro g' hg'
    simp only [Option.some.injEq] at hg'
    simp only [eval, Option.some.injEq, Prod.mk.injEq] at hv
    subst hg'
    obtain ⟨rfl, -⟩ := hv
    show onGrid 0 (w - w)
    rw [Rat.sub_self]
    exact onGrid_zero 0
  | add a b iha ihb =>
    intro g v t hg hv
    simp only [gridOf] at hg
    refine gridPick_sound (fun x hx => hpt _ x v t hx hv) ?_ hg
    intro g hg
    split at hg
    · rename_i o s p q ha hb
      simp only [Option.map_eq_some_iff] at hg
      obtain ⟨c, hc, rfl⟩ := hg
      simp only [eval] at hv
      split at hv
      · rename_i x y hx hy
        simp only [uadd, Option.map_eq_some_iff, Prod.mk.injEq] at hv
        obtain ⟨_, _, rfl, -⟩ := hv
        have h1 := iha (o, s) x.1 x.2 ha hx
        have h2 := ihb (p, q) y.1 y.2 hb hy
        obtain ⟨hs, hq⟩ := commonGrid_spec hc
        show onGrid c (x.1 + y.1 - (o + p))
        have : x.1 + y.1 - (o + p) = (x.1 - o) + (y.1 - p) := by grind
        rw [this]
        exact onGrid_add (onGrid_trans hs h1) (onGrid_trans hq h2)
      · exact absurd hv (by simp)
    · exact absurd hg (by simp)
  | sub a b iha ihb =>
    intro g v t hg hv
    simp only [gridOf] at hg
    refine gridPick_sound (fun x hx => hpt _ x v t hx hv) ?_ hg
    intro g hg
    split at hg
    · rename_i o s p q ha hb
      simp only [Option.map_eq_some_iff] at hg
      obtain ⟨c, hc, rfl⟩ := hg
      simp only [eval] at hv
      split at hv
      · rename_i x y hx hy
        simp only [usub, Option.map_eq_some_iff, Prod.mk.injEq] at hv
        obtain ⟨_, _, rfl, -⟩ := hv
        have h1 := iha (o, s) x.1 x.2 ha hx
        have h2 := ihb (p, q) y.1 y.2 hb hy
        obtain ⟨hs, hq⟩ := commonGrid_spec hc
        show onGrid c (x.1 - y.1 - (o - p))
        have : x.1 - y.1 - (o - p) = (x.1 - o) - (y.1 - p) := by grind
        rw [this]
        exact onGrid_sub (onGrid_trans hs h1) (onGrid_trans hq h2)
      · exact absurd hv (by simp)
    · exact absurd hg (by simp)
  | mul a b iha ihb =>
    intro g v t hg hv
    simp only [gridOf] at hg
    refine gridPick_sound (fun x hx => hpt _ x v t hx hv) ?_ hg
    intro g hg
    simp only [eval] at hv
    split at hv
    · rename_i x y hx hy
      simp only [umul, Option.map_eq_some_iff, Prod.mk.injEq] at hv
      obtain ⟨_, _, rfl, -⟩ := hv
      split at hg
      · rename_i o s p q ha hb
        have h1 := iha (o, s) x.1 x.2 ha hx
        have h2 := ihb (p, q) y.1 y.2 hb hy
        split at hg
        · rename_i hq
          simp only [Option.some.injEq] at hg
          subst hg
          have hq0 : q = 0 := by simpa using hq
          have h2' : onGrid 0 (y.1 - p) := hq0 ▸ h2
          have hy0 : y.1 = p := by have := onGrid_zero_eq h2'; grind
          show onGrid (absR (s * p)) (x.1 * y.1 - o * p)
          rw [hy0]
          have : x.1 * p - o * p = (x.1 - o) * p := by grind
          rw [this]
          exact onGrid_absR (onGrid_mul_right p h1)
        · split at hg
          · rename_i _ hs
            simp only [Option.some.injEq] at hg
            subst hg
            have hs0 : s = 0 := by simpa using hs
            have h1' : onGrid 0 (x.1 - o) := hs0 ▸ h1
            have hx0 : x.1 = o := by have := onGrid_zero_eq h1'; grind
            show onGrid (absR (q * o)) (x.1 * y.1 - o * p)
            rw [hx0]
            have : o * y.1 - o * p = (y.1 - p) * o := by grind
            rw [this]
            exact onGrid_absR (onGrid_mul_right o h2)
          · exact absurd hg (by simp)
      · exact absurd hg (by simp)
    · exact absurd hv (by simp)
  | divc a k kt iha =>
    intro g v t hg hv
    simp only [gridOf] at hg
    refine gridPick_sound (fun x hx => hpt _ x v t hx hv) ?_ hg
    intro g hg
    split at hg
    · exact absurd hg (by simp)
    · rename_i hk
      simp only [Option.map_eq_some_iff] at hg
      obtain ⟨g0, hg0, rfl⟩ := hg
      simp only [eval] at hv
      split at hv
      · rename_i x hx
        simp only [udiv, Option.map_eq_some_iff, Prod.mk.injEq] at hv
        obtain ⟨_, _, rfl, -⟩ := hv
        have h1 := iha g0 x.1 x.2 hg0 hx
        show onGrid (absR (g0.2 / k)) (x.1 / k - g0.1 / k)
        have : x.1 / k - g0.1 / k = (x.1 - g0.1) * k⁻¹ := by
          rw [Rat.div_def, Rat.div_def]; grind
        rw [this, Rat.div_def]
        exact onGrid_absR (onGrid_mul_right k⁻¹ h1)
      · exact absurd hv (by simp)
  | minOf a b iha ihb =>
    intro g v t hg hv
    simp only [gridOf] at hg
    refine gridPick_sound (fun x hx => hpt _ x v t hx hv) ?_ hg
    intro g hg
    split at hg
    · rename_i o s p q ha hb
      split at hg
      · rename_i c hc
        simp only [Option.map_eq_some_iff] at hg
        obtain ⟨d, hd, rfl⟩ := hg
        simp only [eval] at hv
        split at hv
        · rename_i x y hx hy
          simp only [umin, Option.map_eq_some_iff, Prod.mk.injEq] at hv
          obtain ⟨_, _, rfl, -⟩ := hv
          have h1 := iha (o, s) x.1 x.2 ha hx
          have h2 := ihb (p, q) y.1 y.2 hb hy
          obtain ⟨hs, hq⟩ := commonGrid_spec hc
          obtain ⟨hcd, hop⟩ := commonGrid_spec hd
          show onGrid d ((if x.1 ≤ y.1 then x.1 else y.1) - o)
          split
          · exact onGrid_trans hcd (onGrid_trans hs h1)
          · have : y.1 - o = (y.1 - p) - (o - p) := by grind
            rw [this]
            exact onGrid_sub (onGrid_trans hcd (onGrid_trans hq h2)) hop
        · exact absurd hv (by simp)
      · exact absurd hg (by simp)
    · exact absurd hg (by simp)
  | maxOf a b iha ihb =>
    intro g v t hg hv
    simp only [gridOf] at hg
    refine gridPick_sound (fun x hx => hpt _ x v t hx hv) ?_ hg
    intro g hg
    split at hg
    · rename_i o s p q ha hb
      split at hg
      · rename_i c hc
        simp only [Option.map_eq_some_iff] at hg
        obtain ⟨d, hd, rfl⟩ := hg
        simp only [eval] at hv
        split at hv
        · rename_i x y hx hy
          simp only [umax, Option.map_eq_some_iff, Prod.mk.injEq] at hv
          obtain ⟨_, _, rfl, -⟩ := hv
          have h1 := iha (o, s) x.1 x.2 ha hx
          have h2 := ihb (p, q) y.1 y.2 hb hy
          obtain ⟨hs, hq⟩ := commonGrid_spec hc
          obtain ⟨hcd, hop⟩ := commonGrid_spec hd
          show onGrid d ((if x.1 ≤ y.1 then y.1 else x.1) - o)
          split
          · have : y.1 - o = (y.1 - p) - (o - p) := by grind
            rw [this]
            exact onGrid_sub (onGrid_trans hcd (onGrid_trans hq h2)) hop
          · exact onGrid_trans hcd (onGrid_trans hs h1)
        · exact absurd hv (by simp)
      · exact absurd hg (by simp)
    · exact absurd hg (by simp)
  | roundTo a w mo iha =>
    intro g v t hg hv
    cases mo with
    | none =>
      simp only [gridOf] at hg
      exact gridPick_sound (fun x hx => hpt _ x v t hx hv) (fun _ h => by simp at h) hg
    | some m =>
      simp only [gridOf] at hg
      refine gridPick_sound (fun x hx => hpt _ x v t hx hv) ?_ hg
      intro g hg
      split at hg
      · simp only [Option.some.injEq] at hg
        subst hg
        simp only [eval] at hv
        split at hv
        · simp only [Option.some.injEq, Prod.mk.injEq] at hv
          obtain ⟨rfl, -⟩ := hv
          show onGrid w (rndBy (some m) rnd w _ - 0)
          rw [show ∀ x : Rat, x - 0 = x from fun x => by grind]
          exact onGrid_intMul _ w
        · exact absurd hv (by simp)
      · exact absurd hg (by simp)
  | divRound a b w tw mo iha ihb =>
    intro g v t hg hv
    cases mo with
    | none =>
      simp only [gridOf] at hg
      exact gridPick_sound (fun x hx => hpt _ x v t hx hv) (fun _ h => by simp at h) hg
    | some m =>
      simp only [gridOf] at hg
      refine gridPick_sound (fun x hx => hpt _ x v t hx hv) ?_ hg
      intro g hg
      split at hg
      · simp only [Option.some.injEq] at hg
        subst hg
        simp only [eval] at hv
        split at hv
        · simp only [Option.map_eq_some_iff, Prod.mk.injEq] at hv
          obtain ⟨_, _, rfl, -⟩ := hv
          show onGrid w (rndBy (some m) rnd w _ - 0)
          rw [show ∀ x : Rat, x - 0 = x from fun x => by grind]
          exact onGrid_intMul _ w
        · exact absurd hv (by simp)
      · exact absurd hg (by simp)
  | _ =>
    intro g v t hg hv
    simp only [gridOf] at hg
    exact gridPick_sound (fun x hx => hpt _ x v t hx hv) (fun _ h => by simp at h) hg

/-- **What a `true` from `gridMisses` settles**: no value of the grid inside the interval stands on
    the coordinate. -/
theorem gridMisses_sound {g : Rat × Rat} {c : Coord} {I : Rat × Rat} {v : Rat}
    (h : gridMisses g c I = true) (hv : onGridAt g v) (h1 : I.1 ≤ v) (h2 : v ≤ I.2) : ¬ c.holds v := by
  intro hc
  obtain ⟨o, s⟩ := g
  obtain ⟨lo1, hi2⟩ := I
  simp only at h1 h2
  cases c with
  | exactly a =>
    simp only [Coord.holds] at hc
    subst hc
    have hb := gridB_of_onGrid hv
    simp [gridMisses, h1, h2, hb] at h
  | between lo hi =>
    obtain ⟨k, hk⟩ := hv
    simp only at hk
    by_cases hs0 : s = 0
    · subst hs0
      have hvo : v = o := by rw [Rat.mul_zero] at hk; grind
      subst hvo
      have hcb := coordHoldsB_of_holds hc
      simp [gridMisses, hcb, h1, h2] at h
    · by_cases hsn : s < 0
      · simp [gridMisses, hs0, hsn] at h
      · have hs : 0 < s := by grind
        have hq : (v - o) / s = (k : Rat) := by rw [hk]; exact Rat.mul_div_cancel hs0
        -- The low end of the interval: `rceil ((lo1 - o) / s) ≤ k`.
        have hx : (lo1 - o) / s ≤ (k : Rat) := by
          rw [← hq]; exact div_le_div_right (by grind) hs
        have hkl0 : rceil ((lo1 - o) / s) ≤ k := by
          unfold rceil
          have : (-k : Int) ≤ (-((lo1 - o) / s)).floor :=
            Rat.le_floor_iff.mpr (by rw [Rat.intCast_neg]; exact Rat.neg_le_neg hx)
          omega
        -- The high end of the interval: `k ≤ rfloor ((hi2 - o) / s)`.
        have hy : (k : Rat) ≤ (hi2 - o) / s := by
          rw [← hq]; exact div_le_div_right (by grind) hs
        have hkr0 : k ≤ rfloor ((hi2 - o) / s) := Rat.le_floor_iff.mpr hy
        -- The coordinate's own ends.
        have hlo : ∀ a, lo = some a → rfloor ((a - o) / s) + 1 ≤ k := by
          intro a ha
          have hav := hc.1 a ha
          have : (a - o) / s < (k : Rat) := (Rat.div_lt_iff hs).mpr (by rw [← hk]; grind)
          have := Rat.floor_lt_iff.mpr this
          unfold rfloor
          omega
        have hhi : ∀ b, hi = some b → k ≤ rceil ((b - o) / s) - 1 := by
          intro b hb
          have hvb := hc.2 b hb
          have : (k : Rat) < (b - o) / s := (Rat.lt_div_iff hs).mpr (by rw [← hk]; grind)
          have h3 : (-((b - o) / s)).floor < -k :=
            Rat.floor_lt_iff.mpr (by rw [Rat.intCast_neg]; exact Rat.neg_lt_neg this)
          unfold rceil
          omega
        have hs0' : (s == 0) = false := by simp [hs0]
        simp only [gridMisses, hs0', hsn, Bool.false_eq_true, ite_false, decide_eq_true_eq] at h
        cases lo with
        | none =>
          cases hi with
          | none => simp only at h; omega
          | some b => have := hhi b rfl; simp only at h; omega
        | some a =>
          have := hlo a rfl
          cases hi with
          | none => simp only at h; omega
          | some b => have := hhi b rfl; simp only at h; omega

end RulecCert
