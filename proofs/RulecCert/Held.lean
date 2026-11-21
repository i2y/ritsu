/-
  A value read as a linear form over the names a box holds (rulec's §15.325).

  Interval arithmetic reads each place a name stands on its own, so `max(amount, 35) - amount` with
  `amount` from 115 to 337 comes to anything from −222 to 222. Read as a linear form — each held
  name's value times a coefficient, plus a remainder somewhere in an interval — the two places
  cancel, and `max` is the argument the other never passes once the difference of the two forms
  keeps one sign with every held name in its span: the value is 0. `formOf` reads an expression
  that way, `formRange` says where a form can go with each held name in its span, and
  `eval_mem_heldInterval` says what both together settle: the value lies in the interval
  arithmetic's interval met with that range.
-/
import RulecCert.Values

namespace RulecCert

/-- A linear form: the coefficients, in the order the held names are listed, and the two ends of the
    remainder. A list shorter than the names leaves the rest at zero, as `dot` reads it. -/
abbrev Form := List Rat × Rat × Rat

/-- A form that reads no held name: the remainder alone. -/
def formConst (I : Span2) : Form := ([], I.1, I.2)

def formAdd (a b : Form) : Form := (vadd a.1 b.1, a.2.1 + b.2.1, a.2.2 + b.2.2)

def formScale (k : Rat) (a : Form) : Form :=
  (vscale k a.1, rmin (a.2.1 * k) (a.2.2 * k), rmax (a.2.1 * k) (a.2.2 * k))

def formSub (a b : Form) : Form := formAdd a (formScale (-1) b)

/-- The one value a form stands for, where it reads no held name and its remainder is a point. -/
def formPoint (a : Form) : Option Rat :=
  if a.1.all (· == 0) && a.2.1 == a.2.2 then some a.2.1 else none

/-- Where `dot cs vals` can go with each value in its span, the spans listed as the coefficients
    are. -/
def dotRange : List Rat → List Span2 → Span2
  | c :: cs, s :: ss =>
    let r := dotRange cs ss
    (rmin (s.1 * c) (s.2 * c) + r.1, rmax (s.1 * c) (s.2 * c) + r.2)
  | _, _ => (0, 0)

/-- Where a form can go with each held name in its span. -/
def formRange (spans : List Span2) (a : Form) : Span2 :=
  let r := dotRange a.1 spans
  (r.1 + a.2.1, r.2 + a.2.2)

/-- **An expression read as a linear form over the held names** (rulec's §15.325). A held name is
    itself; a sum, a difference, a multiple of a constant and a quotient by one keep the form; `min`
    and `max` are the argument the other never passes, where the difference of the two forms keeps
    one sign over the held spans; a rounding to a grid `g` is its argument with `[-g, g]` added to
    the remainder, since every rounding lands within one step of what it rounds. Anything else is
    the interval arithmetic's interval, as the remainder of a form that reads no held name. -/
def formOf (held : List (String × Span2)) (look : String → Option Span2) (within : Expr → Expr → Bool) :
    Expr → Option Form
  | .name n => match held.findIdx? (·.1 == n) with
    | some i => some (unitAt i 1, 0, 0)
    | none => (look n).map formConst
  | .lit v _ => some (formConst (v, v))
  | .add a b => match formOf held look within a, formOf held look within b with
    | some x, some y => some (formAdd x y)
    | _, _ => (interval look within (.add a b)).map formConst
  | .sub a b => match formOf held look within a, formOf held look within b with
    | some x, some y => some (formSub x y)
    | _, _ => (interval look within (.sub a b)).map formConst
  | .mul a b => match formOf held look within a, formOf held look within b with
    | some x, some y => match formPoint y, formPoint x with
      | some k, _ => some (formScale k x)
      | none, some k => some (formScale k y)
      | none, none => (interval look within (.mul a b)).map formConst
    | _, _ => (interval look within (.mul a b)).map formConst
  | .divc a k kt =>
    if k = 0 then (interval look within (.divc a k kt)).map formConst else
    match formOf held look within a with
    | some x => some (formScale k⁻¹ x)
    | none => (interval look within (.divc a k kt)).map formConst
  | .minOf a b => match formOf held look within a, formOf held look within b with
    | some x, some y =>
      let d := formRange (held.map (·.2)) (formSub x y)
      if 0 ≤ d.1 then some y
      else if d.2 ≤ 0 then some x
      else (interval look within (.minOf a b)).map formConst
    | _, _ => (interval look within (.minOf a b)).map formConst
  | .maxOf a b => match formOf held look within a, formOf held look within b with
    | some x, some y =>
      let d := formRange (held.map (·.2)) (formSub x y)
      if 0 ≤ d.1 then some x
      else if d.2 ≤ 0 then some y
      else (interval look within (.maxOf a b)).map formConst
    | _, _ => (interval look within (.maxOf a b)).map formConst
  | .roundTo a g mo =>
    if 0 < g then
      match formOf held look within a with
      | some x => some (x.1, x.2.1 - g, x.2.2 + g)
      | none => (interval look within (.roundTo a g mo)).map formConst
    else (interval look within (.roundTo a g mo)).map formConst
  | e => (interval look within e).map formConst

/-- **What a box holds a value to** (rulec's §15.325): the interval arithmetic's interval, met with
    the range of the value's linear form over the held names. -/
def heldInterval (held : List (String × Span2)) (look : String → Option Span2)
    (within : Expr → Expr → Bool) (e : Expr) : Option Span2 :=
  match interval look within e, formOf held look within e with
  | some I, some F =>
    let R := formRange (held.map (·.2)) F
    some (rmax I.1 R.1, rmin I.2 R.2)
  | some I, none => some I
  | none, _ => none

/-- An expression with each name `inline` gives an expression for put in its place, through those
    expressions `fuel` deep: a value worked out again from what it reads, as `heldLook` reads it. -/
def substWith (f : String → Expr) : Expr → Expr
  | .name n => f n
  | .lit v t => .lit v t
  | .add a b => .add (substWith f a) (substWith f b)
  | .sub a b => .sub (substWith f a) (substWith f b)
  | .mul a b => .mul (substWith f a) (substWith f b)
  | .divc a k kt => .divc (substWith f a) k kt
  | .minOf a b => .minOf (substWith f a) (substWith f b)
  | .maxOf a b => .maxOf (substWith f a) (substWith f b)
  | .roundTo a g mo => .roundTo (substWith f a) g mo
  | .divRound a b g t mo => .divRound (substWith f a) (substWith f b) g t mo
  | .modc a n => .modc (substWith f a) n
  | .isqrt a => .isqrt (substWith f a)
  | .alloc a b c => .alloc (substWith f a) (substWith f b) (substWith f c)
  | .compare op a b => .compare op (substWith f a) (substWith f b)
  | .typed t => .typed t
  | .unread => .unread

def substInline (inline : List (String × Expr)) : Nat → Expr → Expr
  | 0, e => e
  | k + 1, e => substWith (fun n => match inline.find? (·.1 == n) with
      | some ie => substInline inline k ie.2
      | none => .name n) e

/-! ### What a form settles -/

theorem dot_mem_dotRange : ∀ (cs : List Rat) (spans : List Span2) (vals : List Rat),
    vals.length = spans.length → (∀ i s, spans[i]? = some s → inSpan s (vals.getD i 0)) →
    inSpan (dotRange cs spans) (dot cs vals)
  | [], spans, vals, _, _ => by
    have h1 : dotRange [] spans = (0, 0) := by cases spans <;> rfl
    have h2 : dot [] vals = 0 := by cases vals <;> rfl
    rw [h1, h2]; exact ⟨Rat.le_refl, Rat.le_refl⟩
  | c :: cs, [], vals, hl, _ => by
    have hv : vals = [] := List.eq_nil_of_length_eq_zero (by simpa using hl)
    subst hv
    exact ⟨Rat.le_refl, Rat.le_refl⟩
  | c :: cs, s :: ss, [], hl, _ => by simp at hl
  | c :: cs, s :: ss, v :: vs, hl, h => by
    have hv : inSpan s v := by simpa using h 0 s rfl
    have ih := dot_mem_dotRange cs ss vs (by simpa using hl)
      (fun i s' hs => by simpa using h (i + 1) s' (by simpa using hs))
    obtain ⟨m1, m2⟩ := mul_between hv.1 hv.2 c
    obtain ⟨i1, i2⟩ := ih
    simp only [dotRange, dot]
    rw [Rat.mul_comm c v]
    exact ⟨add_le_add m1 i1, add_le_add m2 i2⟩

/-- A form's value: what the held values give it, and a remainder within its two ends. -/
def FormHolds (vals : List Rat) (F : Form) (v : Rat) : Prop := inSpan (F.2.1, F.2.2) (v - dot F.1 vals)

theorem formRange_mem {spans : List Span2} {vals : List Rat} (hl : vals.length = spans.length)
    (hs : ∀ i s, spans[i]? = some s → inSpan s (vals.getD i 0)) {F : Form} {v : Rat}
    (h : FormHolds vals F v) : inSpan (formRange spans F) v := by
  obtain ⟨d1, d2⟩ := dot_mem_dotRange F.1 spans vals hl hs
  obtain ⟨r1, r2⟩ := h
  simp only [formRange]
  constructor <;> grind

theorem formConst_holds {vals : List Rat} {I : Span2} {v : Rat} (h : inSpan I v) :
    FormHolds vals (formConst I) v := by
  have : dot [] vals = 0 := by cases vals <;> rfl
  simp only [FormHolds, formConst, this]
  obtain ⟨h1, h2⟩ := h
  constructor <;> grind

theorem formAdd_holds {vals : List Rat} {A B : Form} {p q : Rat}
    (ha : FormHolds vals A p) (hb : FormHolds vals B q) : FormHolds vals (formAdd A B) (p + q) := by
  obtain ⟨a1, a2⟩ := ha
  obtain ⟨b1, b2⟩ := hb
  simp only [FormHolds, formAdd, dot_vadd]
  constructor <;> grind

theorem formScale_holds {vals : List Rat} {A : Form} {p : Rat} (k : Rat)
    (ha : FormHolds vals A p) : FormHolds vals (formScale k A) (p * k) := by
  obtain ⟨a1, a2⟩ := ha
  obtain ⟨m1, m2⟩ := mul_between a1 a2 k
  simp only [FormHolds, formScale, dot_vscale]
  have e : p * k - k * dot A.1 vals = (p - dot A.1 vals) * k := by grind
  rw [e]
  exact ⟨m1, m2⟩

theorem formSub_holds {vals : List Rat} {A B : Form} {p q : Rat}
    (ha : FormHolds vals A p) (hb : FormHolds vals B q) : FormHolds vals (formSub A B) (p - q) := by
  have h := formAdd_holds ha (formScale_holds (-1) hb)
  have e : p + q * -1 = p - q := by grind
  rw [e] at h
  exact h

theorem formPoint_eq {vals : List Rat} {A : Form} {p k : Rat} (hk : formPoint A = some k)
    (ha : FormHolds vals A p) : p = k := by
  simp only [formPoint] at hk
  split at hk
  case isTrue hc =>
    simp only [Option.some.injEq] at hk
    simp only [Bool.and_eq_true, List.all_eq_true, beq_iff_eq] at hc
    obtain ⟨hz, he⟩ := hc
    have h0 : dot A.1 vals = 0 := dot_zero A.1 vals hz
    obtain ⟨a1, a2⟩ := ha
    rw [h0] at a1 a2
    subst hk
    rw [he] at a1
    grind
  case isFalse => exact absurd hk (by simp)

theorem sub_le_floorTo {g : Rat} (hg : 0 < g) (v : Rat) : v - g ≤ floorTo g v := by
  have hl : v / g < ((rfloor (v / g) : Int) : Rat) + 1 := (RoundMode.floor_props (v / g)).2
  have hm := Rat.mul_le_mul_of_nonneg_right (Rat.le_of_lt hl) (Rat.le_of_lt hg)
  rw [Rat.div_mul_cancel (Ne.symm (Rat.ne_of_lt hg))] at hm
  simp only [floorTo]
  grind

theorem ceilTo_le_add {g : Rat} (hg : 0 < g) (v : Rat) : ceilTo g v ≤ v + g := by
  have hl : -(v / g) < (((-(v / g)).floor : Int) : Rat) + 1 := (RoundMode.floor_props (-(v / g))).2
  have hc : ((rceil (v / g) : Int) : Rat) = -(((-(v / g)).floor : Int) : Rat) := by
    simp [rceil]
  have hl' : ((rceil (v / g) : Int) : Rat) < v / g + 1 := by rw [hc]; grind
  have hm := Rat.mul_le_mul_of_nonneg_right (Rat.le_of_lt hl') (Rat.le_of_lt hg)
  have e : (v / g + 1) * g = v + g := by
    rw [Rat.add_mul, Rat.div_mul_cancel (Ne.symm (Rat.ne_of_lt hg))]
    grind
  rw [e] at hm
  simp only [ceilTo]
  exact hm

/-- A value of `findIdx?` names a place whose element passes the test. -/
theorem findIdx?_get {α : Type} {p : α → Bool} : ∀ {xs : List α} {i : Nat},
    xs.findIdx? p = some i → ∃ x, xs[i]? = some x ∧ p x = true
  | [], i, h => by simp at h
  | x :: xs, i, h => by
    rw [List.findIdx?_cons] at h
    split at h
    case isTrue hx =>
      simp only [Option.some.injEq] at h
      subst h
      exact ⟨x, rfl, hx⟩
    case isFalse =>
      simp only [Option.map_eq_some_iff] at h
      obtain ⟨j, hj, rfl⟩ := h
      obtain ⟨y, hy, hp⟩ := findIdx?_get hj
      exact ⟨y, by simpa using hy, hp⟩

/-- The held spans give each held value's span, place by place. -/
theorem held_spans {held : List (String × Span2)} {env : String → Option UVal} {vals : List Rat}
    (hheld : ∀ i n s, held[i]? = some (n, s) →
      (∃ t, env n = some (vals.getD i 0, t)) ∧ inSpan s (vals.getD i 0)) :
    ∀ i s, (held.map (·.2))[i]? = some s → inSpan s (vals.getD i 0) := by
  intro i s h
  simp only [List.getElem?_map, Option.map_eq_some_iff] at h
  obtain ⟨⟨n, s'⟩, hx, rfl⟩ := h
  exact (hheld i n s' hx).2

/-- **A form holds the value** (rulec's §15.325): where the held names' values are `vals`, each in
    its span, and every name `look` gives an interval for is inside it, the value an expression
    evaluates to is what its form says — the held values times the coefficients, and a remainder
    within the form's two ends. -/
theorem eval_formOf {held : List (String × Span2)} {look : String → Option Span2}
    {within : Expr → Expr → Bool} {env : String → Option UVal} {rnd : Rat → Rat → Rat}
    (hrnd : RoundsWell rnd)
    (henv : ∀ n I w t, look n = some I → env n = some (w, t) → inSpan I w)
    (hwithin : ∀ a b va vb ta tb, within a b = true →
      eval rnd env a = some (va, ta) → eval rnd env b = some (vb, tb) → va ≤ vb)
    {vals : List Rat} (hlen : vals.length = held.length)
    (hheld : ∀ i n s, held[i]? = some (n, s) →
      (∃ t, env n = some (vals.getD i 0, t)) ∧ inSpan s (vals.getD i 0)) :
    ∀ (e : Expr) (F : Form) (v : Rat) (t : Ty),
      formOf held look within e = some F → eval rnd env e = some (v, t) → FormHolds vals F v := by
  -- Where the form is the interval arithmetic's interval alone.
  have fall : ∀ (e : Expr) (F : Form) (v : Rat) (t : Ty),
      (interval look within e).map formConst = some F → eval rnd env e = some (v, t) →
      FormHolds vals F v := by
    intro e F v t hF hv
    simp only [Option.map_eq_some_iff] at hF
    obtain ⟨I, hI, rfl⟩ := hF
    exact formConst_holds (eval_mem_interval hrnd henv hwithin e I v t hI hv)
  have hl' : vals.length = (held.map (·.2)).length := by simp [hlen]
  have hs := held_spans hheld
  intro e
  induction e with
  | name n =>
    intro F v t hF hv
    simp only [formOf] at hF
    split at hF
    case _ i hi =>
      simp only [Option.some.injEq] at hF
      subst hF
      obtain ⟨⟨n', s⟩, hx, hp⟩ := findIdx?_get hi
      simp only [beq_iff_eq] at hp
      subst hp
      obtain ⟨⟨t', ht⟩, _⟩ := hheld i n' s hx
      simp only [eval] at hv
      rw [ht] at hv
      simp only [Option.some.injEq, Prod.mk.injEq] at hv
      simp only [FormHolds, dot_unitAt, inSpan]
      rw [← hv.1]
      constructor <;> grind
    case _ =>
      simp only [Option.map_eq_some_iff] at hF
      obtain ⟨I, hI, rfl⟩ := hF
      simp only [eval] at hv
      exact formConst_holds (henv n I v t hI hv)
  | lit w ty =>
    intro F v t hF hv
    simp only [formOf, Option.some.injEq] at hF
    subst hF
    simp only [eval, Option.some.injEq, Prod.mk.injEq] at hv
    rw [← hv.1]
    exact formConst_holds ⟨Rat.le_refl, Rat.le_refl⟩
  | add a b iha ihb =>
    intro F v t hF hv
    simp only [formOf] at hF
    split at hF
    case _ x y hx hy =>
      simp only [Option.some.injEq] at hF
      subst hF
      simp only [eval] at hv
      split at hv
      case _ p q hp hq =>
        simp only [uadd, Option.map_eq_some_iff] at hv
        obtain ⟨u, _, hu⟩ := hv
        simp only [Prod.mk.injEq] at hu
        rw [← hu.1]
        exact formAdd_holds (iha x p.1 p.2 hx (by rw [hp])) (ihb y q.1 q.2 hy (by rw [hq]))
      case _ => exact absurd hv (by simp)
    case _ => exact fall _ F v t hF hv
  | sub a b iha ihb =>
    intro F v t hF hv
    simp only [formOf] at hF
    split at hF
    case _ x y hx hy =>
      simp only [Option.some.injEq] at hF
      subst hF
      simp only [eval] at hv
      split at hv
      case _ p q hp hq =>
        simp only [usub, Option.map_eq_some_iff] at hv
        obtain ⟨u, _, hu⟩ := hv
        simp only [Prod.mk.injEq] at hu
        rw [← hu.1]
        exact formSub_holds (iha x p.1 p.2 hx (by rw [hp])) (ihb y q.1 q.2 hy (by rw [hq]))
      case _ => exact absurd hv (by simp)
    case _ => exact fall _ F v t hF hv
  | mul a b iha ihb =>
    intro F v t hF hv
    have hv0 := hv
    simp only [formOf] at hF
    split at hF
    case _ x y hx hy =>
      simp only [eval] at hv
      split at hv
      case _ p q hp hq =>
        have hpx := iha x p.1 p.2 hx (by rw [hp])
        have hqy := ihb y q.1 q.2 hy (by rw [hq])
        simp only [umul, Option.map_eq_some_iff] at hv
        obtain ⟨u, _, hu⟩ := hv
        simp only [Prod.mk.injEq] at hu
        cases hky : formPoint y with
        | some k =>
          simp only [hky, Option.some.injEq] at hF
          subst hF
          rw [← hu.1, formPoint_eq hky hqy]
          exact formScale_holds k hpx
        | none =>
          cases hkx : formPoint x with
          | some k =>
            simp only [hky, hkx, Option.some.injEq] at hF
            subst hF
            rw [← hu.1, formPoint_eq hkx hpx, Rat.mul_comm]
            exact formScale_holds k hqy
          | none =>
            simp only [hky, hkx] at hF
            exact fall _ F v t hF hv0
      case _ => exact absurd hv (by simp)
    case _ => exact fall _ F v t hF hv
  | divc a k kt iha =>
    intro F v t hF hv
    simp only [formOf] at hF
    split at hF
    case isTrue => exact fall _ F v t hF hv
    case isFalse =>
      split at hF
      case _ x hx =>
        simp only [Option.some.injEq] at hF
        subst hF
        simp only [eval] at hv
        split at hv
        case _ p hp =>
          simp only [udiv, Option.map_eq_some_iff] at hv
          obtain ⟨u, _, hu⟩ := hv
          simp only [Prod.mk.injEq] at hu
          rw [← hu.1, Rat.div_def]
          exact formScale_holds k⁻¹ (iha x p.1 p.2 hx (by rw [hp]))
        case _ => exact absurd hv (by simp)
      case _ => exact fall _ F v t hF hv
  | minOf a b iha ihb =>
    intro F v t hF hv
    have hv0 := hv
    simp only [formOf] at hF
    split at hF
    case _ x y hx hy =>
      simp only [eval] at hv
      split at hv
      case _ p q hp hq =>
        have hpx := iha x p.1 p.2 hx (by rw [hp])
        have hqy := ihb y q.1 q.2 hy (by rw [hq])
        have hd := formRange_mem hl' hs (formSub_holds hpx hqy)
        simp only [umin, Option.map_eq_some_iff] at hv
        obtain ⟨u, _, hu⟩ := hv
        simp only [Prod.mk.injEq] at hu
        split at hF
        case isTrue h0 =>
          simp only [Option.some.injEq] at hF
          subst hF
          rw [← hu.1]
          have hqp : q.1 ≤ p.1 := by have := Rat.le_trans h0 hd.1; grind
          by_cases hc : p.1 ≤ q.1
          · have he : p.1 = q.1 := by grind
            simp only [hc, ↓reduceIte]; rw [he]; exact hqy
          · simp only [hc, ↓reduceIte]; exact hqy
        case isFalse =>
          split at hF
          case isTrue h1 =>
            simp only [Option.some.injEq] at hF
            subst hF
            rw [← hu.1]
            have hpq : p.1 ≤ q.1 := by have := Rat.le_trans hd.2 h1; grind
            simp only [hpq, ↓reduceIte]; exact hpx
          case isFalse => exact fall _ F v t hF hv0
      case _ => exact absurd hv (by simp)
    case _ => exact fall _ F v t hF hv
  | maxOf a b iha ihb =>
    intro F v t hF hv
    have hv0 := hv
    simp only [formOf] at hF
    split at hF
    case _ x y hx hy =>
      simp only [eval] at hv
      split at hv
      case _ p q hp hq =>
        have hpx := iha x p.1 p.2 hx (by rw [hp])
        have hqy := ihb y q.1 q.2 hy (by rw [hq])
        have hd := formRange_mem hl' hs (formSub_holds hpx hqy)
        simp only [umax, Option.map_eq_some_iff] at hv
        obtain ⟨u, _, hu⟩ := hv
        simp only [Prod.mk.injEq] at hu
        split at hF
        case isTrue h0 =>
          simp only [Option.some.injEq] at hF
          subst hF
          rw [← hu.1]
          have hqp : q.1 ≤ p.1 := by have := Rat.le_trans h0 hd.1; grind
          by_cases hc : p.1 ≤ q.1
          · have he : q.1 = p.1 := by grind
            simp only [hc, ↓reduceIte]; rw [he]; exact hpx
          · simp only [hc, ↓reduceIte]; exact hpx
        case isFalse =>
          split at hF
          case isTrue h1 =>
            simp only [Option.some.injEq] at hF
            subst hF
            rw [← hu.1]
            have hpq : p.1 ≤ q.1 := by have := Rat.le_trans hd.2 h1; grind
            simp only [hpq, ↓reduceIte]; exact hqy
          case isFalse => exact fall _ F v t hF hv0
      case _ => exact absurd hv (by simp)
    case _ => exact fall _ F v t hF hv
  | roundTo a g mo iha =>
    intro F v t hF hv
    simp only [formOf] at hF
    split at hF
    case isTrue hg =>
      split at hF
      case _ x hx =>
        simp only [Option.some.injEq] at hF
        subst hF
        simp only [eval] at hv
        split at hv
        case _ p hp =>
          simp only [Option.some.injEq, Prod.mk.injEq] at hv
          rw [← hv.1]
          obtain ⟨a1, a2⟩ := iha x p.1 p.2 hx (by rw [hp])
          obtain ⟨r1, r2⟩ := rndBy_roundsWell hrnd mo g p.1 hg
          have f1 := sub_le_floorTo hg p.1
          have c1 := ceilTo_le_add hg p.1
          simp only [FormHolds, inSpan] at a1 a2 ⊢
          constructor <;> grind
        case _ => exact absurd hv (by simp)
      case _ => exact fall _ F v t hF hv
    case isFalse => exact fall _ F v t hF hv
  | divRound a b g ty mo _ _ =>
    intro F v t hF hv
    simp only [formOf] at hF
    exact fall _ F v t hF hv
  | modc a n _ =>
    intro F v t hF hv
    simp only [formOf] at hF
    exact fall _ F v t hF hv
  | isqrt a _ =>
    intro F v t hF hv
    simp only [formOf] at hF
    exact fall _ F v t hF hv
  | alloc a b c _ _ _ =>
    intro F v t hF hv
    simp only [formOf] at hF
    exact fall _ F v t hF hv
  | compare op a b _ _ =>
    intro F v t hF hv
    simp only [formOf] at hF
    exact fall _ F v t hF hv
  | typed ty =>
    intro F v t hF hv
    simp only [formOf] at hF
    exact fall _ F v t hF hv
  | unread =>
    intro F v t hF hv
    simp only [formOf] at hF
    exact fall _ F v t hF hv

/-- **What a box holds a value to, settled** (rulec's §15.325): the same premises, and the value lies
    in `heldInterval` — the interval arithmetic's interval met with its form's range. -/
theorem eval_mem_heldInterval {held : List (String × Span2)} {look : String → Option Span2}
    {within : Expr → Expr → Bool} {env : String → Option UVal} {rnd : Rat → Rat → Rat}
    (hrnd : RoundsWell rnd)
    (henv : ∀ n I w t, look n = some I → env n = some (w, t) → inSpan I w)
    (hwithin : ∀ a b va vb ta tb, within a b = true →
      eval rnd env a = some (va, ta) → eval rnd env b = some (vb, tb) → va ≤ vb)
    {vals : List Rat} (hlen : vals.length = held.length)
    (hheld : ∀ i n s, held[i]? = some (n, s) →
      (∃ t, env n = some (vals.getD i 0, t)) ∧ inSpan s (vals.getD i 0)) :
    ∀ (e : Expr) (I : Span2) (v : Rat) (t : Ty),
      heldInterval held look within e = some I → eval rnd env e = some (v, t) → inSpan I v := by
  intro e I v t hI hv
  have hl' : vals.length = (held.map (·.2)).length := by simp [hlen]
  simp only [heldInterval] at hI
  split at hI
  case _ J F hJ hF =>
    simp only [Option.some.injEq] at hI
    subst hI
    have h1 := eval_mem_interval hrnd henv hwithin e J v t hJ hv
    have h2 := formRange_mem hl' (held_spans hheld)
      (eval_formOf hrnd henv hwithin hlen hheld e F v t hF hv)
    exact ⟨rmax_le h1.1 h2.1, le_rmin h1.2 h2.2⟩
  case _ J hJ _ =>
    simp only [Option.some.injEq] at hI
    subst hI
    exact eval_mem_interval hrnd henv hwithin e J v t hJ hv
  case _ => exact absurd hI (by simp)

/-- **A part the held names pin to one value is that value** (rulec's §15.325): where `heldInterval`
    comes to a single point, the expression evaluates to it. `gridOf` reads a node so pinned as that
    value alone (`gridPick`). -/
theorem eval_eq_heldPoint {held : List (String × Span2)} {look : String → Option Span2}
    {within : Expr → Expr → Bool} {env : String → Option UVal} {rnd : Rat → Rat → Rat}
    (hrnd : RoundsWell rnd)
    (henv : ∀ n I w t, look n = some I → env n = some (w, t) → inSpan I w)
    (hwithin : ∀ a b va vb ta tb, within a b = true →
      eval rnd env a = some (va, ta) → eval rnd env b = some (vb, tb) → va ≤ vb)
    {vals : List Rat} (hlen : vals.length = held.length)
    (hheld : ∀ i n s, held[i]? = some (n, s) →
      (∃ t, env n = some (vals.getD i 0, t)) ∧ inSpan s (vals.getD i 0))
    {e : Expr} {a v : Rat} {t : Ty} (hI : heldInterval held look within e = some (a, a))
    (hv : eval rnd env e = some (v, t)) : v = a := by
  have h := eval_mem_heldInterval hrnd henv hwithin hlen hheld e (a, a) v t hI hv
  exact Rat.le_antisymm h.2 h.1

end RulecCert
