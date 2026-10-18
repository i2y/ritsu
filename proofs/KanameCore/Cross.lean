/-
  X2's relation stage at a call of kaname, put together (kaname's DESIGN 14.3, 15.2): what a call of a
  rule keeps between the inputs it gives holds between their values (`callRels_sound`), and where
  that says the precondition, the precondition holds (`RitsuCross.kept_sound`). So where X2 says, by
  the relations kaname gives, that a precondition holds at a call of kaname, it holds between the
  values the call gives the two inputs, in every environment the scope of the call holds
  (`kept_holds`).

  The relations are between the values as kaname has them, in their own unit: rulec holds the two
  sides of a `constraint` to one type (its E103), and the values are read in that one unit, which
  keeps their order (the factor of a unit is above 0).
-/
import KanameCore.Verdicts
import RitsuCross.ProgramSound

namespace KanameCore

/-- A relation as kaname's port writes it (`Rel::shown`). -/
def Rel.word : Rel → String
  | .lt => "<"
  | .le => "<="
  | .eq => "="

/-- What a call keeps, as `Programs` hands it to ritsu-cross. -/
def keptWire (rels : List (String × Rel × String)) : List RitsuCross.Kept :=
  rels.map fun (a, r, b) => (a, r.word, b)

/-- A value as the rational it is: a number, a date as its day number. -/
def Val.q : Val → Option Rat
  | .num r => some r
  | .date d => some (d : Rat)
  | _ => Option.none

/-- `x op y` between two rationals. -/
def holdsQ : RitsuCross.Op → Rat → Rat → Prop
  | .le, x, y => x ≤ y
  | .lt, x, y => x < y
  | .ge, x, y => y ≤ x
  | .gt, x, y => y < x

theorem le_of_eqQ {p q : Rat} (h : p = q) : p ≤ q := h ▸ Rat.le_refl

theorem relSem_lt {x y : Val} {p q : Rat} (h : RelSem .lt x y) (hx : x.q = some p) (hy : y.q = some q) : p < q := by
  cases x <;> cases y <;> simp only [RelSem] at h <;> simp only [Val.q, Option.some.injEq] at hx hy <;> subst hx hy
  · exact h
  · exact Rat.intCast_lt_intCast.mpr h

theorem relSem_le {x y : Val} {p q : Rat} (h : RelSem .le x y) (hx : x.q = some p) (hy : y.q = some q) : p ≤ q := by
  cases x <;> cases y <;> simp only [RelSem] at h <;> simp only [Val.q, Option.some.injEq] at hx hy <;> subst hx hy
  · exact h
  · exact Rat.intCast_le_intCast.mpr h

theorem relSem_eq {x y : Val} {p q : Rat} (h : RelSem .eq x y) (hx : x.q = some p) (hy : y.q = some q) : p = q := by
  simp only [RelSem] at h
  subst h
  rw [hx] at hy
  exact Option.some.inj hy

/-- **Where the relations a call of a rule keeps say a precondition, the precondition holds between
    the values the call gives its two inputs**, in every environment the scope of the call holds:
    the names given to `left` and to `right` have values that keep `left op right`. -/
theorem kept_holds {A : ACtx} {ρ : Env} (h : Sat A ρ) {rd : RuleD} {args : List (Nat × Expr × Option Conv)}
    {op : RitsuCross.Op} {left right : String}
    (hk : RitsuCross.kept (keptWire (callRels A rd args)) op left right = true) :
    ∃ xl xr sl sr, (∃ k c, (k, xl, c) ∈ args ∧ (rd.inputs[k]?).map (·.name) = some left) ∧
      (∃ k c, (k, xr, c) ∈ args ∧ (rd.inputs[k]?).map (·.name) = some right) ∧
      sym xl = some sl ∧ sym xr = some sr ∧
      ∀ x y p q, symVal ρ sl = some x → symVal ρ sr = some y → x.q = some p → y.q = some q → holdsQ op p q := by
  unfold RitsuCross.kept at hk
  obtain ⟨⟨a, w, b⟩, hm, hs⟩ := List.any_eq_true.mp hk
  simp only [keptWire, List.mem_map] at hm
  obtain ⟨⟨a', r, b'⟩, hmr, he⟩ := hm
  simp only [Prod.mk.injEq] at he
  obtain ⟨rfl, rfl, rfl⟩ := he
  obtain ⟨xa, xb, sa, sb, ha, hb, hsa, hsb, hrel⟩ := callRels_sound h hmr
  cases op <;> cases r <;>
    simp only [RitsuCross.Op.sides, RitsuCross.says, Rel.word, beq_iff_eq, Bool.and_eq_true, Bool.or_eq_true,
      Bool.not_true, Bool.not_false, String.reduceEq, ite_true, ite_false, and_true,
      and_false, Bool.false_eq_true] at hs
  -- `<=`: from `<`, `<=` and `=` either way round
  · obtain ⟨rfl, rfl⟩ := hs
    exact ⟨xa, xb, sa, sb, ha, hb, hsa, hsb, fun x y p q hx hy hp hq => Rat.le_of_lt (relSem_lt (hrel x y hx hy) hp hq)⟩
  · obtain ⟨rfl, rfl⟩ := hs
    exact ⟨xa, xb, sa, sb, ha, hb, hsa, hsb, fun x y p q hx hy hp hq => relSem_le (hrel x y hx hy) hp hq⟩
  · rcases hs with ⟨rfl, rfl⟩ | ⟨rfl, rfl⟩
    · exact ⟨xa, xb, sa, sb, ha, hb, hsa, hsb, fun x y p q hx hy hp hq => le_of_eqQ (relSem_eq (hrel x y hx hy) hp hq)⟩
    · exact ⟨xb, xa, sb, sa, hb, ha, hsb, hsa, fun x y p q hx hy hp hq => le_of_eqQ (relSem_eq (hrel y x hy hx) hq hp).symm⟩
  -- `<`: from `<` only
  · obtain ⟨rfl, rfl⟩ := hs
    exact ⟨xa, xb, sa, sb, ha, hb, hsa, hsb, fun x y p q hx hy hp hq => relSem_lt (hrel x y hx hy) hp hq⟩
  -- `>=`: the right below the left
  · obtain ⟨rfl, rfl⟩ := hs
    exact ⟨xb, xa, sb, sa, hb, ha, hsb, hsa, fun x y p q hx hy hp hq => Rat.le_of_lt (relSem_lt (hrel y x hy hx) hq hp)⟩
  · obtain ⟨rfl, rfl⟩ := hs
    exact ⟨xb, xa, sb, sa, hb, ha, hsb, hsa, fun x y p q hx hy hp hq => relSem_le (hrel y x hy hx) hq hp⟩
  · rcases hs with ⟨rfl, rfl⟩ | ⟨rfl, rfl⟩
    · exact ⟨xb, xa, sb, sa, hb, ha, hsb, hsa, fun x y p q hx hy hp hq => le_of_eqQ (relSem_eq (hrel y x hy hx) hq hp)⟩
    · exact ⟨xa, xb, sa, sb, ha, hb, hsa, hsb, fun x y p q hx hy hp hq => le_of_eqQ (relSem_eq (hrel x y hx hy) hp hq).symm⟩
  -- `>`: from `<` only, the right below the left
  · obtain ⟨rfl, rfl⟩ := hs
    exact ⟨xb, xa, sb, sa, hb, ha, hsb, hsa, fun x y p q hx hy hp hq => relSem_lt (hrel y x hy hx) hq hp⟩

end KanameCore
