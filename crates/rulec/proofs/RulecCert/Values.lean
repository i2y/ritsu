/-
  The values a rule computes: what they mean, what type they keep, and how far they can go
  (§2.1, §2.3, §7.1, DESIGN §15.97).

  The two claims in this file are about one expression rather than one table. **Units**
  (E103): the type the rule declares for a value is the type its own expression gives it.
  **int64** (E108): every value a rule computes, at the scale it is stored, fits in a signed
  64-bit integer in every target language.

  Both are proved against one evaluation. `eval` carries a type alongside the number and
  refuses to go on where the units do not meet, so "the derived type is right" and "the
  evaluation does not get stuck" are one statement; and the interval the checker computes
  is shown to hold that same evaluation's number. Rounding is the one operation left
  abstract: which way a mode breaks a tie is not the subject here, so it arrives as a
  function with the only property the bounds need — the result sits on the grid, between
  rounding down and rounding up.
-/
import RulecCert.Sieve

namespace RulecCert

/-! ## Types -/

/-- A type as §2.1 writes it. A money type may carry a tag — `税込` and the like — and two
    amounts of the same currency meet whether or not one of them is tagged. -/
inductive Ty where
  | number
  | rate
  | money : String → Option String → Ty
  /-- Any other dimension the rule writes as `kind[unit]`: `qty[個]`, `area[m2]`. Two of
      them meet only when they are written the same way. -/
  | dim : String → Ty
  /-- Everything with no dimension and no arithmetic: `bool`, `date`, `string`, the name of
      an enum. Two of them meet only when they are written the same way, and none of them
      carries an interval — `check` makes no int64 claim about a truth value. -/
  | other : String → Ty
  deriving DecidableEq, Repr, Inhabited

/-- Whether the type carries a dimension. A rate and a count do not. -/
def Ty.dimensioned : Ty → Bool
  | .money _ _ => true
  | .dim _ => true
  | _ => false

/-- Whether a value of this type is stored as an integer, and so has an int64 claim to
    make. A certificate that states no interval for one of these is refused. -/
def Ty.numeric : Ty → Bool
  | .other _ => false
  | _ => true

/-- Two types meet when they are the same, or when one is an amount with no tag and the
    other the same currency with one (§2.1). -/
def unify : Ty → Ty → Option Ty
  | .money c₁ t₁, .money c₂ t₂ =>
    if c₁ = c₂ then
      match t₁, t₂ with
      | none, t => some (.money c₁ t)
      | t, none => some (.money c₁ t)
      | some a, some b => if a = b then some (.money c₁ (some a)) else none
    else none
  | a, b => if a = b then some a else none

/-! ## Expressions

Exactly the forms a certificate can carry. A divisor is a constant and a rounding grid is a
literal, both by §2.3 — E115 refuses the rest — so neither needs to be an expression, and
the interval arithmetic below is spared the cases that would come with them. -/

inductive Expr where
  | name : String → Expr
  | lit : Rat → Ty → Expr
  | add : Expr → Expr → Expr
  | sub : Expr → Expr → Expr
  | mul : Expr → Expr → Expr
  /-- Divided by a constant: its value and its type. -/
  | divc : Expr → Rat → Ty → Expr
  | minOf : Expr → Expr → Expr
  | maxOf : Expr → Expr → Expr
  /-- Rounded to a literal grid. Which way the mode breaks a tie is not modelled. -/
  | roundTo : Expr → Rat → Expr
  /-- A comparison. Its value is a truth value, and it has a unit claim of its own: the two
      sides have to meet, which is what makes `注文金額 >= 3万円` well typed and
      `注文金額 >= 3個` not. -/
  | compare : Cmp → Expr → Expr → Expr
  /-- A leaf with a type and no number: a date, a string. It carries a unit claim and no
      interval claim, which is what `check` says about it too. -/
  | typed : Ty → Expr
  /-- A leaf this program cannot type — a word whose type only the context gives. A value
      whose expression holds one is reported as not re-checked, never as passed. -/
  | unread : Expr
  deriving Repr, Inhabited

/-- Whether anything in the expression is a leaf this program cannot type. -/
def Expr.unreadable : Expr → Bool
  | .name _ => false
  | .lit _ _ => false
  | .typed _ => false
  | .unread => true
  | .add a b => a.unreadable || b.unreadable
  | .sub a b => a.unreadable || b.unreadable
  | .mul a b => a.unreadable || b.unreadable
  | .divc a _ _ => a.unreadable
  | .minOf a b => a.unreadable || b.unreadable
  | .maxOf a b => a.unreadable || b.unreadable
  | .roundTo a _ => a.unreadable
  | .compare _ a b => a.unreadable || b.unreadable

/-! ## What each operation does to a type

Factored out so that the derivation and the evaluation below say the same thing by
construction: `eval` computes the number and asks these for the type, and `typeOf` asks
these and nothing else. -/

def mulTy (x y : Ty) : Option Ty :=
  if x.dimensioned && y.dimensioned then none
  else if x.dimensioned then some x
  else if y.dimensioned then some y
  else if x = Ty.rate || y = Ty.rate then some Ty.rate
  else some x

/-- Dividing by a constant of the same unit cancels it and leaves a plain count; a
    dimensionless divisor leaves the left side as it was (§2.3). -/
def divTy (x kt : Ty) : Option Ty :=
  match unify x kt with
  | some _ => some Ty.number
  | none => if kt.dimensioned then none else some x

/-- `min` and `max` need their two sides to meet, and give back the first one's type. -/
def pickTy (x y : Ty) : Option Ty := (unify x y).map (fun _ => x)

/-- **The type of an expression**, derived from the leaves up by the rules of §2.3. A
    certificate that states a different one for a value is refused; so is one whose units
    do not meet at an operation, which is what E103 says. -/
def typeOf (types : String → Option Ty) : Expr → Option Ty
  | .name n => types n
  | .lit _ t => some t
  | .add a b => match typeOf types a, typeOf types b with
                | some x, some y => unify x y
                | _, _ => none
  | .sub a b => match typeOf types a, typeOf types b with
                | some x, some y => unify x y
                | _, _ => none
  | .mul a b => match typeOf types a, typeOf types b with
                | some x, some y => mulTy x y
                | _, _ => none
  | .divc a _ kt => match typeOf types a with
                    | some x => divTy x kt
                    | none => none
  | .minOf a b => match typeOf types a, typeOf types b with
                  | some x, some y => pickTy x y
                  | _, _ => none
  | .maxOf a b => match typeOf types a, typeOf types b with
                  | some x, some y => pickTy x y
                  | _, _ => none
  | .roundTo a _ => typeOf types a
  | .compare _ a b => match typeOf types a, typeOf types b with
                      | some x, some y => (unify x y).map (fun _ => Ty.other "bool")
                      | _, _ => none
  | .typed t => some t
  | .unread => none

/-! ## Evaluation

A value is a number and the type it keeps. An operation whose units do not meet has no
value — that is what makes the units claim a statement about a computation and not about a
derivation talking to itself. -/

abbrev UVal := Rat × Ty

def uadd (x y : UVal) : Option UVal := (unify x.2 y.2).map (fun t => (x.1 + y.1, t))
def usub (x y : UVal) : Option UVal := (unify x.2 y.2).map (fun t => (x.1 - y.1, t))
def umul (x y : UVal) : Option UVal := (mulTy x.2 y.2).map (fun t => (x.1 * y.1, t))
def udiv (x : UVal) (k : Rat) (kt : Ty) : Option UVal :=
  (divTy x.2 kt).map (fun t => (x.1 / k, t))
def umin (x y : UVal) : Option UVal :=
  (pickTy x.2 y.2).map (fun t => (if x.1 ≤ y.1 then x.1 else y.1, t))
def umax (x y : UVal) : Option UVal :=
  (pickTy x.2 y.2).map (fun t => (if x.1 ≤ y.1 then y.1 else x.1, t))

/-- Evaluation. `rnd g v` is the rounding: only its bounds matter here, and they arrive as
    a hypothesis on the theorem that needs them. -/
def eval (rnd : Rat → Rat → Rat) (env : String → Option UVal) : Expr → Option UVal
  | .name n => env n
  | .lit v t => some (v, t)
  | .add a b => match eval rnd env a, eval rnd env b with
                | some x, some y => uadd x y
                | _, _ => none
  | .sub a b => match eval rnd env a, eval rnd env b with
                | some x, some y => usub x y
                | _, _ => none
  | .mul a b => match eval rnd env a, eval rnd env b with
                | some x, some y => umul x y
                | _, _ => none
  | .divc a k kt => match eval rnd env a with
                    | some x => udiv x k kt
                    | none => none
  | .minOf a b => match eval rnd env a, eval rnd env b with
                  | some x, some y => umin x y
                  | _, _ => none
  | .maxOf a b => match eval rnd env a, eval rnd env b with
                  | some x, some y => umax x y
                  | _, _ => none
  | .roundTo a g => match eval rnd env a with
                    | some x => some (rnd g x.1, x.2)
                    | none => none
  | .compare op a b => match eval rnd env a, eval rnd env b with
                       | some x, some y =>
                         (unify x.2 y.2).map (fun _ =>
                           ((if cmpHolds op x.1 y.1 then 1 else 0 : Rat), Ty.other "bool"))
                       | _, _ => none
  | .typed t => some (0, t)
  | .unread => none

/-! ## The units claim -/

/-- **E103 is settled by the derivation.** Where the checker derives a type, the value is
    there and keeps that very type: no operation in the expression is left with units that
    do not meet. -/
theorem eval_type_of_typeOf {types : String → Option Ty} {env : String → Option UVal}
    {rnd : Rat → Rat → Rat} (henv : ∀ n t, types n = some t → ∃ v, env n = some (v, t)) :
    ∀ (e : Expr) (τ : Ty), typeOf types e = some τ → ∃ v, eval rnd env e = some (v, τ) := by
  intro e
  induction e with
  | name n => intro τ h; exact henv n τ h
  | lit v t => intro τ h; simp only [typeOf, Option.some.injEq] at h; exact ⟨v, by simp [eval, h]⟩
  | add a b iha ihb =>
    intro τ h
    simp only [typeOf] at h
    split at h
    case _ x y hx hy =>
      obtain ⟨va, hva⟩ := iha x hx
      obtain ⟨vb, hvb⟩ := ihb y hy
      exact ⟨va + vb, by simp [eval, hva, hvb, uadd, h]⟩
    case _ => exact absurd h (by simp)
  | sub a b iha ihb =>
    intro τ h
    simp only [typeOf] at h
    split at h
    case _ x y hx hy =>
      obtain ⟨va, hva⟩ := iha x hx
      obtain ⟨vb, hvb⟩ := ihb y hy
      exact ⟨va - vb, by simp [eval, hva, hvb, usub, h]⟩
    case _ => exact absurd h (by simp)
  | mul a b iha ihb =>
    intro τ h
    simp only [typeOf] at h
    split at h
    case _ x y hx hy =>
      obtain ⟨va, hva⟩ := iha x hx
      obtain ⟨vb, hvb⟩ := ihb y hy
      exact ⟨va * vb, by simp [eval, hva, hvb, umul, h]⟩
    case _ => exact absurd h (by simp)
  | divc a k kt iha =>
    intro τ h
    simp only [typeOf] at h
    split at h
    case _ x hx =>
      obtain ⟨va, hva⟩ := iha x hx
      exact ⟨va / k, by simp [eval, hva, udiv, h]⟩
    case _ => exact absurd h (by simp)
  | minOf a b iha ihb =>
    intro τ h
    simp only [typeOf] at h
    split at h
    case _ x y hx hy =>
      obtain ⟨va, hva⟩ := iha x hx
      obtain ⟨vb, hvb⟩ := ihb y hy
      exact ⟨if va ≤ vb then va else vb, by simp [eval, hva, hvb, umin, h]⟩
    case _ => exact absurd h (by simp)
  | maxOf a b iha ihb =>
    intro τ h
    simp only [typeOf] at h
    split at h
    case _ x y hx hy =>
      obtain ⟨va, hva⟩ := iha x hx
      obtain ⟨vb, hvb⟩ := ihb y hy
      exact ⟨if va ≤ vb then vb else va, by simp [eval, hva, hvb, umax, h]⟩
    case _ => exact absurd h (by simp)
  | roundTo a g iha =>
    intro τ h
    obtain ⟨va, hva⟩ := iha τ (by simpa [typeOf] using h)
    exact ⟨rnd g va, by simp [eval, hva]⟩
  | compare op a b iha ihb =>
    intro τ h
    simp only [typeOf] at h
    split at h
    case _ x y hx hy =>
      obtain ⟨va, hva⟩ := iha x hx
      obtain ⟨vb, hvb⟩ := ihb y hy
      refine ⟨if cmpHolds op va vb then 1 else 0, ?_⟩
      simp only [eval, hva, hvb, Option.map_eq_some_iff] at h ⊢
      obtain ⟨u, hu, hτ⟩ := h
      exact ⟨u, hu, by simpa using hτ⟩
    case _ => exact absurd h (by simp)
  | typed t => intro τ h; simp only [typeOf, Option.some.injEq] at h; exact ⟨0, by simp [eval, h]⟩
  | unread => intro τ h; exact absurd h (by simp [typeOf])


/-! ## How far a value can go

The interval arithmetic, and the proof that it holds. Core states enough about `Rat` to do
this without a library; `rmin` and `rmax` are spelled out here so that the six facts the
proof needs about them are in one place. -/

def rmin (a b : Rat) : Rat := if a ≤ b then a else b
def rmax (a b : Rat) : Rat := if a ≤ b then b else a

theorem rmin_le_left (a b : Rat) : rmin a b ≤ a := by
  unfold rmin; split
  · exact Rat.le_refl
  · rcases (Rat.le_total : a ≤ b ∨ b ≤ a) with h | h
    · exact absurd h (by assumption)
    · exact h
theorem rmin_le_right (a b : Rat) : rmin a b ≤ b := by
  unfold rmin; split
  · assumption
  · exact Rat.le_refl
theorem le_rmin {a b c : Rat} (h1 : c ≤ a) (h2 : c ≤ b) : c ≤ rmin a b := by
  unfold rmin; split <;> assumption
theorem left_le_rmax (a b : Rat) : a ≤ rmax a b := by
  unfold rmax; split
  · assumption
  · exact Rat.le_refl
theorem right_le_rmax (a b : Rat) : b ≤ rmax a b := by
  unfold rmax; split
  · exact Rat.le_refl
  · rcases (Rat.le_total : a ≤ b ∨ b ≤ a) with h | h
    · exact absurd h (by assumption)
    · exact h
theorem rmax_le {a b c : Rat} (h1 : a ≤ c) (h2 : b ≤ c) : rmax a b ≤ c := by
  unfold rmax; split <;> assumption

def rmin4 (a b c d : Rat) : Rat := rmin (rmin a b) (rmin c d)
def rmax4 (a b c d : Rat) : Rat := rmax (rmax a b) (rmax c d)

/-- Multiplying a bounded value by anything keeps it between the two products of the
    bounds — whichever way round the sign of the other factor puts them. -/
theorem mul_between {a b x : Rat} (h1 : a ≤ x) (h2 : x ≤ b) (y : Rat) :
    rmin (a * y) (b * y) ≤ x * y ∧ x * y ≤ rmax (a * y) (b * y) := by
  rcases (Rat.le_total : (0:Rat) ≤ y ∨ y ≤ 0) with hy | hy
  · exact ⟨Rat.le_trans (rmin_le_left _ _) (Rat.mul_le_mul_of_nonneg_right h1 hy),
      Rat.le_trans (Rat.mul_le_mul_of_nonneg_right h2 hy) (right_le_rmax _ _)⟩
  · have hny : (0 : Rat) ≤ -y := by
      have := Rat.neg_le_neg hy
      simpa using this
    have k1 : a * -y ≤ x * -y := Rat.mul_le_mul_of_nonneg_right h1 hny
    have k2 : x * -y ≤ b * -y := Rat.mul_le_mul_of_nonneg_right h2 hny
    rw [Rat.mul_neg, Rat.mul_neg] at k1 k2
    have k1' : x * y ≤ a * y := by simpa using Rat.neg_le_neg k1
    have k2' : b * y ≤ x * y := by simpa using Rat.neg_le_neg k2
    exact ⟨Rat.le_trans (rmin_le_right _ _) k2', Rat.le_trans k1' (left_le_rmax _ _)⟩

/-- The product of two bounded values stays inside the four products of their bounds. -/
theorem mul_box {a b c d x y : Rat} (hxa : a ≤ x) (hxb : x ≤ b) (hyc : c ≤ y) (hyd : y ≤ d) :
    rmin4 (a * c) (a * d) (b * c) (b * d) ≤ x * y ∧
      x * y ≤ rmax4 (a * c) (a * d) (b * c) (b * d) := by
  obtain ⟨lo, hi⟩ := mul_between hxa hxb y
  have ha : rmin (a * c) (a * d) ≤ a * y ∧ a * y ≤ rmax (a * c) (a * d) := by
    have := mul_between hyc hyd a
    rw [Rat.mul_comm c a, Rat.mul_comm d a, Rat.mul_comm y a] at this
    exact this
  have hb : rmin (b * c) (b * d) ≤ b * y ∧ b * y ≤ rmax (b * c) (b * d) := by
    have := mul_between hyc hyd b
    rw [Rat.mul_comm c b, Rat.mul_comm d b, Rat.mul_comm y b] at this
    exact this
  refine ⟨Rat.le_trans ?_ lo, Rat.le_trans hi ?_⟩
  · exact le_rmin (Rat.le_trans (rmin_le_left _ _) ha.1) (Rat.le_trans (rmin_le_right _ _) hb.1)
  · exact rmax_le (Rat.le_trans ha.2 (left_le_rmax _ _)) (Rat.le_trans hb.2 (right_le_rmax _ _))

theorem add_le_add {a b c d : Rat} (h1 : a ≤ b) (h2 : c ≤ d) : a + c ≤ b + d :=
  Rat.le_trans (Rat.add_le_add_right.2 h1) (Rat.add_le_add_left.2 h2)

/-! ### Rounding to a grid

`rnd` is any rounding: the certificate does not say which mode a value uses, and the
bounds do not depend on it. What they do depend on is that the result sits between
rounding down to the grid and rounding up to it, and that both of those are monotone. -/

def rfloor (x : Rat) : Int := x.floor
def rceil (x : Rat) : Int := -((-x).floor)

theorem rceil_monotone {x y : Rat} (h : x ≤ y) : rceil x ≤ rceil y := by
  have : -y ≤ -x := Rat.neg_le_neg h
  exact Int.neg_le_neg (Rat.floor_monotone this)

theorem le_rceil (x : Rat) : x ≤ (rceil x : Rat) := by
  have := Rat.floor_le (-x)
  have h2 : -((-x).floor : Rat) ≥ x := by
    have := Rat.neg_le_neg this
    simpa using this
  simpa [rceil] using h2

def floorTo (g x : Rat) : Rat := (rfloor (x / g) : Int) * g
def ceilTo (g x : Rat) : Rat := (rceil (x / g) : Int) * g

theorem div_le_div_right {x y k : Rat} (h : x ≤ y) (hk : 0 < k) : x / k ≤ y / k := by
  rw [Rat.div_def, Rat.div_def]
  exact Rat.mul_le_mul_of_nonneg_right h (Rat.le_of_lt (Rat.inv_pos.2 hk))

theorem floorTo_monotone {g x y : Rat} (hg : 0 < g) (h : x ≤ y) : floorTo g x ≤ floorTo g y := by
  refine Rat.mul_le_mul_of_nonneg_right ?_ (Rat.le_of_lt hg)
  exact Rat.intCast_le_intCast.2 (Rat.floor_monotone (div_le_div_right h hg))

theorem ceilTo_monotone {g x y : Rat} (hg : 0 < g) (h : x ≤ y) : ceilTo g x ≤ ceilTo g y := by
  refine Rat.mul_le_mul_of_nonneg_right ?_ (Rat.le_of_lt hg)
  exact Rat.intCast_le_intCast.2 (rceil_monotone (div_le_div_right h hg))

/-- What any rounding to a positive grid has to satisfy, and all the bounds below use. -/
def RoundsWell (rnd : Rat → Rat → Rat) : Prop :=
  ∀ g v, 0 < g → floorTo g v ≤ rnd g v ∧ rnd g v ≤ ceilTo g v

/-! ### The interval a declared range forces -/

abbrev Span2 := Rat × Rat

def inSpan (I : Span2) (v : Rat) : Prop := I.1 ≤ v ∧ v ≤ I.2

/-- **The interval an expression is forced into**, recomputed from the declared ranges. A
    `none` says this program does not know — an unbounded name, a zero divisor, a grid that
    is not positive — and the claim over that value is then reported as not re-checked
    rather than passed. -/
def interval (ranges : String → Option Span2) : Expr → Option Span2
  | .name n => ranges n
  | .lit v _ => some (v, v)
  | .add a b => match interval ranges a, interval ranges b with
                | some x, some y => some (x.1 + y.1, x.2 + y.2)
                | _, _ => none
  | .sub a b => match interval ranges a, interval ranges b with
                | some x, some y => some (x.1 - y.2, x.2 - y.1)
                | _, _ => none
  | .mul a b => match interval ranges a, interval ranges b with
                | some x, some y =>
                  some (rmin4 (x.1 * y.1) (x.1 * y.2) (x.2 * y.1) (x.2 * y.2),
                        rmax4 (x.1 * y.1) (x.1 * y.2) (x.2 * y.1) (x.2 * y.2))
                | _, _ => none
  | .divc a k _ => if k = 0 then none else
      match interval ranges a with
      | some x => some (rmin (x.1 / k) (x.2 / k), rmax (x.1 / k) (x.2 / k))
      | none => none
  | .minOf a b => match interval ranges a, interval ranges b with
                  | some x, some y => some (rmin x.1 y.1, rmin x.2 y.2)
                  | _, _ => none
  | .maxOf a b => match interval ranges a, interval ranges b with
                  | some x, some y => some (rmax x.1 y.1, rmax x.2 y.2)
                  | _, _ => none
  | .roundTo a g => if 0 < g then
      match interval ranges a with
      | some x => some (floorTo g x.1, ceilTo g x.2)
      | none => none
    else none
  -- A truth value is not stored as an integer, so there is no interval to state and none
  -- is claimed. `Ty.numeric` is what refuses a certificate that leaves one out elsewhere.
  | .compare _ _ _ => none
  | .typed _ => none
  | .unread => none

/-- **E108 rests on this.** Where the checker states an interval, the value really is
    inside it, whatever the inputs and whichever way each rounding breaks its ties. -/
theorem eval_mem_interval {ranges : String → Option Span2} {env : String → Option UVal}
    {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd)
    (henv : ∀ n I w t, ranges n = some I → env n = some (w, t) → inSpan I w) :
    ∀ (e : Expr) (I : Span2) (v : Rat) (t : Ty),
      interval ranges e = some I → eval rnd env e = some (v, t) → inSpan I v := by
  intro e
  induction e with
  | name n => intro I v t hI hv; exact henv n I v t hI hv
  | lit w t =>
    intro I v t' hI hv
    simp only [interval, Option.some.injEq] at hI
    simp only [eval, Option.some.injEq, Prod.mk.injEq] at hv
    subst hI; rw [← hv.1]
    exact ⟨Rat.le_refl, Rat.le_refl⟩
  | add a b iha ihb =>
    intro I v t hI hv
    simp only [interval] at hI; simp only [eval] at hv
    split at hI
    case _ x y hx hy =>
      split at hv
      case _ p q hp hq =>
        obtain ⟨hp1, hp2⟩ := iha x p.1 p.2 hx (by rw [hp])
        obtain ⟨hq1, hq2⟩ := ihb y q.1 q.2 hy (by rw [hq])
        simp only [uadd, Option.map_eq_some_iff] at hv
        obtain ⟨u, _, hu⟩ := hv
        simp only [Option.some.injEq, Prod.mk.injEq] at hI hu
        subst hI
        rw [← hu.1]
        exact ⟨add_le_add hp1 hq1, add_le_add hp2 hq2⟩
      case _ => exact absurd hv (by simp)
    case _ => exact absurd hI (by simp)
  | sub a b iha ihb =>
    intro I v t hI hv
    simp only [interval] at hI; simp only [eval] at hv
    split at hI
    case _ x y hx hy =>
      split at hv
      case _ p q hp hq =>
        obtain ⟨hp1, hp2⟩ := iha x p.1 p.2 hx (by rw [hp])
        obtain ⟨hq1, hq2⟩ := ihb y q.1 q.2 hy (by rw [hq])
        simp only [usub, Option.map_eq_some_iff] at hv
        obtain ⟨u, _, hu⟩ := hv
        simp only [Option.some.injEq, Prod.mk.injEq] at hI hu
        subst hI
        rw [← hu.1, Rat.sub_eq_add_neg, Rat.sub_eq_add_neg, Rat.sub_eq_add_neg]
        exact ⟨add_le_add hp1 (Rat.neg_le_neg hq2), add_le_add hp2 (Rat.neg_le_neg hq1)⟩
      case _ => exact absurd hv (by simp)
    case _ => exact absurd hI (by simp)
  | mul a b iha ihb =>
    intro I v t hI hv
    simp only [interval] at hI; simp only [eval] at hv
    split at hI
    case _ x y hx hy =>
      split at hv
      case _ p q hp hq =>
        obtain ⟨hp1, hp2⟩ := iha x p.1 p.2 hx (by rw [hp])
        obtain ⟨hq1, hq2⟩ := ihb y q.1 q.2 hy (by rw [hq])
        simp only [umul, Option.map_eq_some_iff] at hv
        obtain ⟨u, _, hu⟩ := hv
        simp only [Option.some.injEq, Prod.mk.injEq] at hI hu
        subst hI
        rw [← hu.1]
        exact mul_box hp1 hp2 hq1 hq2
      case _ => exact absurd hv (by simp)
    case _ => exact absurd hI (by simp)
  | divc a k kt iha =>
    intro I v t hI hv
    simp only [interval] at hI
    split at hI
    case _ => exact absurd hI (by simp)
    case _ hk =>
      simp only [eval] at hv
      split at hI
      case _ x hx =>
        split at hv
        case _ p hp =>
          obtain ⟨hp1, hp2⟩ := iha x p.1 p.2 hx (by rw [hp])
          simp only [udiv, Option.map_eq_some_iff] at hv
          obtain ⟨u, _, hu⟩ := hv
          simp only [Option.some.injEq, Prod.mk.injEq] at hI hu
          subst hI
          rw [← hu.1, Rat.div_def, Rat.div_def, Rat.div_def]
          exact mul_between hp1 hp2 k⁻¹
        case _ => exact absurd hv (by simp)
      case _ => exact absurd hI (by simp)
  | minOf a b iha ihb =>
    intro I v t hI hv
    simp only [interval] at hI; simp only [eval] at hv
    split at hI
    case _ x y hx hy =>
      split at hv
      case _ p q hp hq =>
        obtain ⟨hp1, hp2⟩ := iha x p.1 p.2 hx (by rw [hp])
        obtain ⟨hq1, hq2⟩ := ihb y q.1 q.2 hy (by rw [hq])
        simp only [umin, Option.map_eq_some_iff] at hv
        obtain ⟨u, _, hu⟩ := hv
        simp only [Option.some.injEq, Prod.mk.injEq] at hI hu
        subst hI
        rw [← hu.1]
        by_cases hc : p.1 ≤ q.1
        · rw [ite_eq_left_of_eq_true _ _ (by simp [hc])]
          exact ⟨Rat.le_trans (rmin_le_left _ _) hp1,
            le_rmin hp2 (Rat.le_trans hc hq2)⟩
        · rw [ite_eq_right_of_eq_false _ _ (by simp [hc])]
          have hq : q.1 ≤ p.1 := Rat.le_of_lt (Rat.not_le.1 hc)
          exact ⟨Rat.le_trans (rmin_le_right _ _) hq1,
            le_rmin (Rat.le_trans hq hp2) hq2⟩
      case _ => exact absurd hv (by simp)
    case _ => exact absurd hI (by simp)
  | maxOf a b iha ihb =>
    intro I v t hI hv
    simp only [interval] at hI; simp only [eval] at hv
    split at hI
    case _ x y hx hy =>
      split at hv
      case _ p q hp hq =>
        obtain ⟨hp1, hp2⟩ := iha x p.1 p.2 hx (by rw [hp])
        obtain ⟨hq1, hq2⟩ := ihb y q.1 q.2 hy (by rw [hq])
        simp only [umax, Option.map_eq_some_iff] at hv
        obtain ⟨u, _, hu⟩ := hv
        simp only [Option.some.injEq, Prod.mk.injEq] at hI hu
        subst hI
        rw [← hu.1]
        by_cases hc : p.1 ≤ q.1
        · rw [ite_eq_left_of_eq_true _ _ (by simp [hc])]
          exact ⟨rmax_le (Rat.le_trans hp1 hc) hq1, Rat.le_trans hq2 (right_le_rmax _ _)⟩
        · rw [ite_eq_right_of_eq_false _ _ (by simp [hc])]
          have hq : q.1 ≤ p.1 := Rat.le_of_lt (Rat.not_le.1 hc)
          exact ⟨rmax_le hp1 (Rat.le_trans hq1 hq), Rat.le_trans hp2 (left_le_rmax _ _)⟩
      case _ => exact absurd hv (by simp)
    case _ => exact absurd hI (by simp)
  | compare op a b _ _ => intro I v t hI _; exact absurd hI (by simp [interval])
  | typed _ => intro I v t hI _; exact absurd hI (by simp [interval])
  | unread => intro I v t hI _; exact absurd hI (by simp [interval])
  | roundTo a g iha =>
    intro I v t hI hv
    simp only [interval] at hI
    split at hI
    case _ hg =>
      simp only [eval] at hv
      split at hI
      case _ x hx =>
        split at hv
        case _ p hp =>
          obtain ⟨hp1, hp2⟩ := iha x p.1 p.2 hx (by rw [hp])
          simp only [Option.some.injEq, Prod.mk.injEq] at hI hv
          subst hI
          rw [← hv.1]
          obtain ⟨lo, hi⟩ := hrnd g p.1 hg
          exact ⟨Rat.le_trans (floorTo_monotone hg hp1) lo,
            Rat.le_trans hi (ceilTo_monotone hg hp2)⟩
        case _ => exact absurd hv (by simp)
      case _ => exact absurd hI (by simp)
    case _ => exact absurd hI (by simp)

/-! ## int64 -/

def absR (v : Rat) : Rat := if 0 ≤ v then v else -v

def I64_MAX : Rat := 9223372036854775807

/-- **The int64 check.** The widest the value can be, times the scale it is stored at, has
    to stay inside what the certificate claims, and that claim inside int64. -/
def storedFits (I : Span2) (scale storedMax : Rat) : Bool :=
  decide (0 ≤ scale) && decide (rmax (absR I.1) (absR I.2) * scale ≤ storedMax) &&
    decide (storedMax ≤ I64_MAX)

theorem le_absR (x : Rat) : x ≤ absR x := by
  unfold absR; split
  · exact Rat.le_refl
  · rename_i h
    have hx : x ≤ 0 := Rat.le_of_lt (Rat.not_le.1 h)
    exact Rat.le_trans hx (by simpa using Rat.neg_le_neg hx)

theorem neg_le_absR (x : Rat) : -x ≤ absR x := by
  unfold absR; split
  · rename_i h
    exact Rat.le_trans (by simpa using Rat.neg_le_neg h) h
  · exact Rat.le_refl

theorem absR_le_of_inSpan {I : Span2} {v : Rat} (h : inSpan I v) :
    absR v ≤ rmax (absR I.1) (absR I.2) := by
  rcases (Rat.le_total : (0:Rat) ≤ v ∨ v ≤ 0) with hv | hv
  · have : absR v = v := by unfold absR; rw [ite_eq_left_of_eq_true _ _ (by simp [hv])]
    rw [this]
    exact Rat.le_trans h.2 (Rat.le_trans (le_absR I.2) (right_le_rmax _ _))
  · have : absR v ≤ -v := by
      unfold absR; split
      · rename_i h0
        exact Rat.le_trans hv (by simpa using Rat.neg_le_neg hv)
      · exact Rat.le_refl
    exact Rat.le_trans this
      (Rat.le_trans (Rat.neg_le_neg h.1) (Rat.le_trans (neg_le_absR I.1) (left_le_rmax _ _)))

/-- **E108 is settled by the interval.** Every value the rule computes, at the scale it is
    stored, fits in a signed 64-bit integer. -/
theorem stored_in_i64 {I : Span2} {v scale storedMax : Rat} (hv : inSpan I v)
    (h : storedFits I scale storedMax = true) : absR v * scale ≤ I64_MAX := by
  simp only [storedFits, Bool.and_eq_true, decide_eq_true_eq] at h
  obtain ⟨⟨hs, hm⟩, hmx⟩ := h
  exact Rat.le_trans
    (Rat.le_trans (Rat.mul_le_mul_of_nonneg_right (absR_le_of_inSpan hv) hs) hm) hmx

end RulecCert
