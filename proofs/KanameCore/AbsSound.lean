/-
  The abstract values hold the values they are about (kaname's DESIGN 5.1, 15.2.2).

  `Mem a v` says the value `v` is among what `a` says: a number in its interval, a value of an enum
  or a bool in its set, `none` where the value may be none, a list whose count is in the interval
  and whose every element is among what each element can be, a record whose every field is among
  what that field can be and whose fields keep the relations the record says. The evaluation's own
  test (`Abs.has`) says no more than `Mem`: `has_sound`.

  The operations keep a value in: `join` holds what either holds; the field of a record holds the
  field of every value; a number read in another unit is in what its values read so are; a value
  cut to what is declared of it stays in where it is in both.
-/
import KanameCore.IvSound
import KanameCore.Analysis

namespace KanameCore

/-- The value of the first field of a record named `n` (what `Val.field?` reads). -/
def fieldV (vs : List (String × Val)) (n : String) : Option Val := (vs.find? (·.1 == n)).map (·.2)

/-- Two numbers that keep a relation. -/
def RelVals (r : Rel) : Val → Val → Prop
  | .num a, .num b => r.holds a b = true
  | _, _ => False

/-- The relations of a record hold between its fields. -/
def RelsHold (rels : List (String × Rel × String)) (vs : List (String × Val)) : Prop :=
  ∀ a r b, (a, r, b) ∈ rels → ∃ x y, fieldV vs a = some x ∧ fieldV vs b = some y ∧ RelVals r x y

/-- The value is among what the abstract value says. A record's field is read by its name, as the
    evaluation reads it (the first of the name). -/
inductive Mem : Abs → Val → Prop where
  | any (v : Val) : Mem .any v
  | num {i : Iv} {r : Rat} : i.In r → Mem (.num i) (.num r)
  | date {i : Iv} {d : Int} : i.In d → Mem (.date i) (.date d)
  | enumAll {x : String} : Mem (.set none) (.enum x)
  | boolAll {b : Bool} : Mem (.set none) (.bool b)
  | enum {s : List String} {x : String} : x ∈ s → Mem (.set (some s)) (.enum x)
  | bool {s : List String} {b : Bool} : (if b then "true" else "false") ∈ s → Mem (.set (some s)) (.bool b)
  | str {s : String} : Mem .str (.str s)
  | optNone {x : Abs} : Mem (.opt true x) .none
  | optSome {n : Bool} {x : Abs} {v : Val} : v ≠ .none → Mem x v → Mem (.opt n x) v
  | list {c : Iv} {e : Abs} {xs : List Val} : c.In xs.length → (∀ x ∈ xs, Mem e x) → Mem (.list c e) (.list xs)
  | record {fs : List (String × Abs)} {rels : List (String × Rel × String)} {vs : List (String × Val)} :
      (∀ n x, fieldV vs n = some x → ∀ a, Abs.fieldOf fs n = some a → Mem a x) → RelsHold rels vs →
      Mem (.record fs rels) (.record vs)
  | transfer {as : List (String × Abs)} {id : String} {vs : List (String × Val)} :
      (∀ n x, fieldV vs n = some x → ∀ a, Abs.fieldOf as n = some a → Mem a x) → Mem (.transfer as) (.transfer id vs)

theorem fieldV_mem {vs : List (String × Val)} {n : String} {x : Val} (h : fieldV vs n = some x) : (n, x) ∈ vs := by
  unfold fieldV at h
  rcases hf : vs.find? (·.1 == n) with _ | ⟨k, y⟩ <;> rw [hf] at h <;> simp at h
  subst h
  have hm := List.mem_of_find?_eq_some hf
  have hk := List.find?_some hf
  simp at hk; subst hk; exact hm

theorem Mem.not_never {v : Val} : ¬ Mem .never v := by intro h; cases h

theorem Mem.none_cases {a : Abs} (h : Mem a .none) : a = .any ∨ ∃ x, a = .opt true x := by
  cases h with
  | any => exact Or.inl rfl
  | optNone => exact Or.inr ⟨_, rfl⟩
  | optSome hv _ => exact absurd rfl hv

-- ── The evaluation's test says no more ──────────────────────────────────────

theorem rels_of_has {rels : List (String × Rel × String)} {vs : List (String × Val)}
    (h : rels.all (fun (a, r, b) =>
        match ((vs.find? (·.1 == a)).map (·.2) : Option Val), ((vs.find? (·.1 == b)).map (·.2) : Option Val) with
        | some (Val.num x), some (Val.num y) => r.holds x y
        | _, _ => false) = true) : RelsHold rels vs := by
  intro a r b hm
  have := List.all_eq_true.mp h (a, r, b) hm
  simp only at this
  split at this
  · rename_i x y hx hy
    exact ⟨_, _, hx, hy, this⟩
  · simp at this

/-- **What `has` says is so.** -/
theorem has_sound : ∀ (d : Nat) (a : Abs) (v : Val), Abs.hasD d a v = true → Mem a v
  | 0, _, _, h => by simp [Abs.hasD] at h
  | d + 1, a, v, h => by
    cases a with
    | never => simp [Abs.hasD] at h
    | any => exact Mem.any _
    | opt n x =>
      cases v with
      | none => simp only [Abs.hasD] at h; subst h; exact Mem.optNone
      | _ =>
        simp only [Abs.hasD] at h
        exact Mem.optSome (by simp) (has_sound d x _ h)
    | num i =>
      cases v <;> simp only [Abs.hasD] at h <;> (try contradiction)
      exact Mem.num ((Iv.has_iff _ _).mp h)
    | date i =>
      cases v <;> simp only [Abs.hasD] at h <;> (try contradiction)
      exact Mem.date ((Iv.has_iff _ _).mp h)
    | set s =>
      cases s with
      | none =>
        cases v <;> simp only [Abs.hasD] at h <;> (try contradiction)
        · exact Mem.boolAll
        · exact Mem.enumAll
      | some s =>
        cases v <;> simp only [Abs.hasD] at h <;> (try contradiction)
        · exact Mem.bool (List.contains_iff_mem.mp h)
        · exact Mem.enum (List.contains_iff_mem.mp h)
    | str => cases v <;> simp only [Abs.hasD] at h <;> (try contradiction); exact Mem.str
    | list c e =>
      cases v <;> simp only [Abs.hasD] at h <;> (try contradiction)
      rename_i xs
      simp only [Bool.and_eq_true] at h
      obtain ⟨h1, h2⟩ := h
      exact Mem.list ((Iv.has_iff _ _).mp h1) (fun x hx => has_sound d _ x (List.all_eq_true.mp h2 x hx))
    | record fs rels =>
      cases v <;> simp only [Abs.hasD] at h <;> (try contradiction)
      rename_i vs
      simp only [Bool.and_eq_true] at h
      obtain ⟨h1, h2⟩ := h
      refine Mem.record ?_ (rels_of_has h2)
      intro n x hx a ha
      have := List.all_eq_true.mp h1 (n, x) (fieldV_mem hx)
      simp only [ha] at this
      exact has_sound d a x this
    | transfer as =>
      cases v <;> simp only [Abs.hasD] at h <;> (try contradiction)
      rename_i id vs
      refine Mem.transfer ?_
      intro n x hx a ha
      have := List.all_eq_true.mp h (n, x) (fieldV_mem hx)
      simp only [ha] at this
      exact has_sound d a x this

theorem has_sound' {a : Abs} {v : Val} (h : a.has v = true) : Mem a v := has_sound D a v h

-- ── Join ─────────────────────────────────────────────────────────────────────

theorem mem_insertSorted {x y : String} : ∀ {s : List String}, x ∈ insertSorted y s ↔ x = y ∨ x ∈ s
  | [] => by simp [insertSorted]
  | z :: zs => by
    unfold insertSorted
    split
    · simp
    · split
      · rename_i h; subst h; simp
      · have := @mem_insertSorted x y zs
        simp only [List.mem_cons, this]
        constructor
        · rintro (h | h | h)
          · exact Or.inr (Or.inl h)
          · exact Or.inl h
          · exact Or.inr (Or.inr h)
        · rintro (h | h | h)
          · exact Or.inr (Or.inl h)
          · exact Or.inl h
          · exact Or.inr (Or.inr h)

theorem mem_union {x : String} : ∀ (b a : List String), x ∈ b.foldl (fun s v => insertSorted v s) a ↔ x ∈ a ∨ x ∈ b
  | [], a => by simp
  | y :: ys, a => by
    simp only [List.foldl_cons]
    rw [mem_union ys (insertSorted y a), mem_insertSorted]
    simp only [List.mem_cons]
    constructor
    · rintro ((h | h) | h)
      · exact Or.inr (Or.inl h)
      · exact Or.inl h
      · exact Or.inr (Or.inr h)
    · rintro (h | h | h)
      · exact Or.inl (Or.inr h)
      · exact Or.inl (Or.inl h)
      · exact Or.inr h

theorem fieldOf_joinFields {g : Abs → Abs → Abs} {fb : List (String × Abs)} {n : String} :
    ∀ (fa : List (String × Abs)), Abs.fieldOf (Abs.joinFields g fb fa) n =
      (Abs.fieldOf fa n).map (Abs.joinField g fb n)
  | [] => rfl
  | (k, x) :: rest => by
    by_cases h : k = n
    · subst h; simp [Abs.fieldOf, Abs.joinFields, List.find?]
    · have hk : (k == n) = false := by simp [h]
      have e1 : Abs.fieldOf ((k, x) :: rest) n = Abs.fieldOf rest n := by
        simp only [Abs.fieldOf, List.find?, hk]
      have e2 : Abs.fieldOf (Abs.joinFields g fb ((k, x) :: rest)) n = Abs.fieldOf (Abs.joinFields g fb rest) n := by
        simp only [Abs.fieldOf, Abs.joinFields, List.find?, hk]
      rw [e1, e2, fieldOf_joinFields rest]

theorem relsHold_filter {rels : List (String × Rel × String)} {vs : List (String × Val)} {p : _ → Bool}
    (h : RelsHold rels vs) : RelsHold (rels.filter p) vs := by
  intro a r b hm
  exact h a r b (List.mem_filter.mp hm).1

theorem relsHold_of_mem {rels rb : List (String × Rel × String)} {vs : List (String × Val)}
    (h : RelsHold rb vs) : RelsHold (rels.filter (fun r => rb.contains r)) vs := by
  intro a r b hm
  have := (List.mem_filter.mp hm).2
  exact h a r b (List.contains_iff_mem.mp this)

theorem mem_setUnion_left {a b : Option (List String)} {v : Val} (h : Mem (.set a) v) :
    Mem (.set (Abs.setUnion a b)) v := by
  cases h with
  | enumAll => exact Mem.enumAll
  | boolAll => exact Mem.boolAll
  | enum hx =>
    cases b with
    | none => exact Mem.enumAll
    | some y => exact Mem.enum ((mem_union _ _).mpr (Or.inl hx))
  | bool hx =>
    cases b with
    | none => exact Mem.boolAll
    | some y => exact Mem.bool ((mem_union _ _).mpr (Or.inl hx))

theorem mem_setUnion_right {a b : Option (List String)} {v : Val} (h : Mem (.set b) v) :
    Mem (.set (Abs.setUnion a b)) v := by
  cases h with
  | enumAll => cases a <;> exact Mem.enumAll
  | boolAll => cases a <;> exact Mem.boolAll
  | enum hx =>
    cases a with
    | none => exact Mem.enumAll
    | some y => exact Mem.enum ((mem_union _ _).mpr (Or.inr hx))
  | bool hx =>
    cases a with
    | none => exact Mem.boolAll
    | some y => exact Mem.bool ((mem_union _ _).mpr (Or.inr hx))

/-- **The join holds what either side holds.** -/
theorem join_mem : ∀ d, (∀ a b v, Mem a v → Mem (Abs.joinD d a b) v) ∧ (∀ a b v, Mem b v → Mem (Abs.joinD d a b) v)
  | 0 => ⟨fun _ _ v _ => by simp [Abs.joinD]; exact Mem.any v, fun _ _ v _ => by simp [Abs.joinD]; exact Mem.any v⟩
  | d + 1 => by
    have ⟨ihl, ihr⟩ := join_mem d
    have recL : ∀ (fa fb : List (String × Abs)) (vs : List (String × Val)),
        (∀ n x, fieldV vs n = some x → ∀ a, Abs.fieldOf fa n = some a → Mem a x) →
        ∀ n x, fieldV vs n = some x → ∀ a, Abs.fieldOf (Abs.joinFields (Abs.joinD d) fb fa) n = some a → Mem a x := by
      intro fa fb vs hf n x hx a ha
      rw [fieldOf_joinFields] at ha
      rcases hfa : Abs.fieldOf fa n with _ | xa <;> rw [hfa] at ha <;> simp at ha
      subst ha
      unfold Abs.joinField
      split
      · exact ihl _ _ _ (hf n x hx xa hfa)
      · exact Mem.any _
    have recR : ∀ (fa fb : List (String × Abs)) (vs : List (String × Val)),
        (∀ n x, fieldV vs n = some x → ∀ a, Abs.fieldOf fb n = some a → Mem a x) →
        ∀ n x, fieldV vs n = some x → ∀ a, Abs.fieldOf (Abs.joinFields (Abs.joinD d) fb fa) n = some a → Mem a x := by
      intro fa fb vs hf n x hx a ha
      rw [fieldOf_joinFields] at ha
      rcases hfa : Abs.fieldOf fa n with _ | xa <;> rw [hfa] at ha <;> simp at ha
      subst ha
      unfold Abs.joinField
      split
      · rename_i y hy
        exact ihr _ _ _ (hf n x hx y hy)
      · exact Mem.any _
    constructor
    · intro a b v h
      cases b <;> cases h <;> simp only [Abs.joinD]
      all_goals first
        | exact Mem.any _
        | exact Mem.num (In_hull_left (by assumption))
        | exact Mem.date (In_hull_left (by assumption))
        | exact Mem.num (by assumption)
        | exact Mem.date (by assumption)
        | exact Mem.enumAll
        | exact Mem.boolAll
        | exact Mem.str
        | exact Mem.optNone
        | (simp only [Bool.true_or]; exact Mem.optNone)
        | exact Mem.optSome (by assumption) (by assumption)
        | exact Mem.optSome (by assumption) (ihl _ _ _ (by assumption))
        | exact mem_setUnion_left (Mem.enum (by assumption))
        | exact mem_setUnion_left (Mem.bool (by assumption))
        | exact mem_setUnion_left Mem.enumAll
        | exact mem_setUnion_left Mem.boolAll
        | exact Mem.enum (by assumption)
        | exact Mem.bool (by assumption)
        | exact Mem.list (In_hull_left (by assumption)) (fun x hx => ihl _ _ _ ((by assumption : ∀ x ∈ _, Mem _ x) x hx))
        | exact Mem.list (by assumption) (by assumption)
        | exact Mem.record (recL _ _ _ (by assumption)) (relsHold_filter (by assumption))
        | exact Mem.record (by assumption) (by assumption)
        | exact Mem.transfer (recL _ _ _ (by assumption))
        | exact Mem.transfer (by assumption)
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.num (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.date (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ Mem.enumAll)
        | exact Mem.optSome (by simp) (ihr _ _ _ Mem.boolAll)
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.enum (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.bool (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ Mem.str)
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.list (by assumption) (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.record (by assumption) (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.transfer (by assumption)))
    · intro a b v h
      cases a <;> cases h <;> simp only [Abs.joinD]
      all_goals first
        | exact Mem.any _
        | exact Mem.num (In_hull_right (by assumption))
        | exact Mem.date (In_hull_right (by assumption))
        | exact Mem.num (by assumption)
        | exact Mem.date (by assumption)
        | exact Mem.enumAll
        | exact Mem.boolAll
        | exact Mem.str
        | exact Mem.optNone
        | (simp only [Bool.or_true]; exact Mem.optNone)
        | exact Mem.optSome (by assumption) (by assumption)
        | exact Mem.optSome (by assumption) (ihr _ _ _ (by assumption))
        | exact Mem.optSome (by assumption) (ihl _ _ _ (by assumption))
        | exact mem_setUnion_right (Mem.enum (by assumption))
        | exact mem_setUnion_right (Mem.bool (by assumption))
        | exact mem_setUnion_right Mem.enumAll
        | exact mem_setUnion_right Mem.boolAll
        | exact Mem.enum (by assumption)
        | exact Mem.bool (by assumption)
        | exact Mem.list (In_hull_right (by assumption)) (fun x hx => ihr _ _ _ ((by assumption : ∀ x ∈ _, Mem _ x) x hx))
        | exact Mem.list (by assumption) (by assumption)
        | exact Mem.record (recR _ _ _ (by assumption)) (relsHold_of_mem (by assumption))
        | exact Mem.record (by assumption) (by assumption)
        | exact Mem.transfer (recR _ _ _ (by assumption))
        | exact Mem.transfer (by assumption)
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.num (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.date (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ Mem.enumAll)
        | exact Mem.optSome (by simp) (ihr _ _ _ Mem.boolAll)
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.enum (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.bool (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ Mem.str)
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.list (by assumption) (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.record (by assumption) (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.transfer (by assumption)))

theorem mem_join_left {a b : Abs} {v : Val} (h : Mem a v) : Mem (a.join b) v := (join_mem D).1 a b v h
theorem mem_join_right {a b : Abs} {v : Val} (h : Mem b v) : Mem (a.join b) v := (join_mem D).2 a b v h

-- ── Fields, intervals, optionals ─────────────────────────────────────────────

/-- **The field of a record holds the field of every value.** -/
theorem Mem_field {a : Abs} {v : Val} (h : Mem a v) : ∀ {f : String} {w : Val}, v.field? f = some w → Mem (a.field f) w := by
  induction h with
  | any => intro f w _; exact Mem.any _
  | @record fs rels vs hf hr ih =>
    intro f w hw
    simp only [Val.field?] at hw
    simp only [Abs.field]
    rcases hfind : fs.find? (·.1 == f) with _ | ⟨k, b⟩
    · simp only [hfind]; exact Mem.any _
    · have : Abs.fieldOf fs f = some b := by simp [Abs.fieldOf, hfind]
      simp only [hfind]
      exact hf f w hw b this
  | optSome _ _ ih => intro f w hw; simp only [Abs.field]; exact ih hw
  | _ => intro f w hw; simp [Val.field?] at hw

theorem Mem_iv_num {a : Abs} {r : Rat} (h : Mem a (.num r)) : ∀ {i : Iv}, a.iv = some i → i.In r := by
  generalize hv : Val.num r = v at h
  induction h with
  | num hi => intro i h; cases hv; simp [Abs.iv] at h; subst h; exact hi
  | optSome _ _ ih => intro i h; exact ih hv h
  | any => intro i h; simp [Abs.iv] at h
  | _ => intro i h; first | cases hv | simp [Abs.iv] at h

theorem Mem_iv_date {a : Abs} {d : Int} (h : Mem a (.date d)) : ∀ {i : Iv}, a.iv = some i → i.In d := by
  generalize hv : Val.date d = v at h
  induction h with
  | date hi => intro i h; cases hv; simp [Abs.iv] at h; subst h; exact hi
  | optSome _ _ ih => intro i h; exact ih hv h
  | any => intro i h; simp [Abs.iv] at h
  | _ => intro i h; first | cases hv | simp [Abs.iv] at h

theorem Mem_present {a : Abs} {v : Val} (h : Mem a v) (hv : v ≠ .none) : Mem a.present v := by
  cases h with
  | optNone => exact absurd rfl hv
  | optSome _ hx => exact hx
  | _ => simp only [Abs.present]; first | (constructor <;> assumption) | constructor | assumption

-- ── Numbers read in another unit ─────────────────────────────────────────────

theorem den_conv_le (r f s : Rat) : (r * f + s).den ≤ r.den * f.den * s.den :=
  Nat.le_trans (den_add_le _ _) (Nat.mul_le_mul_right _ (den_mul_le _ _))

theorem In_scaled {i : Iv} {r f s : Rat} (h : i.In r) :
    ({ (if s.num = 0 then i.scale f else (i.scale f).plus (Iv.exact s)) with den := i.den.map (· * f.den * s.den) } : Iv).In (r * f + s) := by
  have h1 : (if s.num = 0 then i.scale f else (i.scale f).plus (Iv.exact s)).In (r * f + s) := by
    split
    · rename_i hs
      have : s = 0 := Rat.num_eq_zero.mp hs
      subst this; simp only [Rat.add_zero]; exact In_scale f h
    · exact In_plus (In_scale f h) (In_exact s)
  refine ⟨h1.lo, h1.hi, ?_⟩
  have hd := h.den
  rcases hi : i.den with _ | d
  · simp [DenLe]
  · rw [hi] at hd
    simp only [DenLe, Option.map_some] at hd ⊢
    exact Nat.le_trans (den_conv_le r f s) (Nat.mul_le_mul_right _ (Nat.mul_le_mul_right _ hd))

theorem Mem_scaled_num {a : Abs} {r : Rat} (f s : Rat) (h : Mem a (.num r)) : Mem (a.scaled f s) (.num (r * f + s)) := by
  generalize hv : Val.num r = v at h
  induction h with
  | any => exact Mem.any _
  | num hi => cases hv; simp only [Abs.scaled]; exact Mem.num (In_scaled hi)
  | optSome _ _ ih => cases hv; simp only [Abs.scaled]; exact Mem.optSome (by simp) (ih rfl)
  | _ => cases hv

theorem coerceL_ok {c : Conv} : ∀ {xs ys : List Val}, coerceL c xs = .ok ys →
    ys.length = xs.length ∧ ∀ y ∈ ys, ∃ x ∈ xs, coerceV c x = .ok y
  | [], ys, h => by simp [coerceL] at h; cases h; simp
  | x :: xs, ys, h => by
    simp only [coerceL, bind, Except.bind] at h
    rcases hx : coerceV c x with e | y <;> rw [hx] at h <;> simp at h
    rcases hxs : coerceL c xs with e | ys' <;> rw [hxs] at h <;> simp [] at h
    subst h
    have ⟨h1, h2⟩ := coerceL_ok hxs
    refine ⟨by simp [h1], ?_⟩
    intro z hz
    simp at hz
    rcases hz with rfl | hz
    · exact ⟨x, by simp, hx⟩
    · obtain ⟨w, hw, hw'⟩ := h2 z hz
      exact ⟨w, by simp [hw], hw'⟩

theorem coerceL_error {c : Conv} : ∀ {xs : List Val} {e : Stop}, coerceL c xs = .error e →
    ∃ x ∈ xs, coerceV c x = .error e
  | [], e, h => by simp [coerceL] at h
  | x :: xs, e, h => by
    simp only [coerceL, bind, Except.bind] at h
    rcases hx : coerceV c x with e' | y <;> rw [hx] at h
    · simp at h; subst h; exact ⟨x, by simp, hx⟩
    · simp only at h
      rcases hxs : coerceL c xs with e' | ys' <;> rw [hxs] at h <;> simp [] at h
      subst h
      obtain ⟨w, hw, hw'⟩ := coerceL_error hxs
      exact ⟨w, by simp [hw], hw'⟩

theorem coerceV_other {c : Conv} {v : Val} (h1 : ∀ r, v ≠ .num r) (h2 : ∀ xs, v ≠ .list xs) : coerceV c v = .ok v := by
  cases v <;> simp_all [coerceV]

theorem convN_ok {c : Conv} {r r' : Rat} (h : convN (some c) r = .ok r') : r' = r * c.f + c.shift ∧ fits r' = true := by
  simp only [convN, liftA, held] at h
  split at h <;> rename_i h'
  · split at h' <;> simp_all
  · split at h' <;> simp_all
  · split at h' <;> simp_all

theorem convN_error {c : Conv} {r : Rat} {e : Stop} (h : convN (some c) r = .error e) :
    e = .failed .overflow c.line ∧ fits (r * c.f + c.shift) = false := by
  simp only [convN, liftA, held] at h
  split at h <;> rename_i h'
  · split at h' <;> simp_all
  · split at h' <;> simp_all
  · split at h' <;> simp_all

theorem coerceV_ne_none {c : Conv} {v v' : Val} (hv : v ≠ .none) (h : coerceV c v = .ok v') : v' ≠ .none := by
  cases v with
  | num r =>
    simp only [coerceV, Functor.map, Except.map] at h
    rcases hc : convN (some c) r with e | r' <;> rw [hc] at h <;> simp at h
    subst h; simp
  | list xs =>
    simp only [coerceV, Functor.map, Except.map] at h
    rcases hcl : coerceL c xs with e | ys <;> rw [hcl] at h <;> simp at h
    subst h; simp
  | none => exact absurd rfl hv
  | _ => simp [coerceV] at h; subst h; simp

mutual
theorem coerceV_error (c : Conv) : ∀ (x : Val) (e : Stop), coerceV c x = .error e → e = .failed .overflow c.line
  | .num r, e, he => by
    simp only [coerceV, Functor.map, Except.map] at he
    rcases hc : convN (some c) r with e' | r' <;> rw [hc] at he <;> simp at he
    subst he
    exact (convN_error hc).1
  | .list xs, e, he => by
    simp only [coerceV, Functor.map, Except.map] at he
    rcases hcl : coerceL c xs with e' | ys <;> rw [hcl] at he <;> simp at he
    subst he
    exact coerceL_error' c xs e' hcl
  | .bool _, e, he => by simp [coerceV] at he
  | .str _, e, he => by simp [coerceV] at he
  | .date _, e, he => by simp [coerceV] at he
  | .enum _, e, he => by simp [coerceV] at he
  | .record _, e, he => by simp [coerceV] at he
  | .none, e, he => by simp [coerceV] at he
  | .transfer _ _, e, he => by simp [coerceV] at he
theorem coerceL_error' (c : Conv) : ∀ (xs : List Val) (e : Stop), coerceL c xs = .error e → e = .failed .overflow c.line
  | [], e, he => by simp [coerceL] at he
  | x :: xs, e, he => by
    simp only [coerceL, bind, Except.bind] at he
    rcases hx : coerceV c x with e' | y <;> rw [hx] at he
    · simp at he; subst he; exact coerceV_error c x e' hx
    · simp only at he
      rcases hxs : coerceL c xs with e' | ys' <;> rw [hxs] at he <;> simp [] at he
      subst he
      exact coerceL_error' c xs e' hxs
end

/-- **A value read in another unit is in its values read so.** -/
theorem Mem_coerceV {c : Conv} {a : Abs} {v : Val} (h : Mem a v) : ∀ {v' : Val}, coerceV c v = .ok v' →
    Mem (a.scaled c.f c.shift) v' := by
  induction h with
  | any => intro v' _; exact Mem.any _
  | num hi =>
    intro v' hv
    simp only [coerceV, Functor.map, Except.map] at hv
    rcases hc : convN (some c) _ with e | r' <;> rw [hc] at hv <;> simp at hv
    subst hv
    obtain ⟨rfl, _⟩ := convN_ok hc
    simp only [Abs.scaled]; exact Mem.num (In_scaled hi)
  | list hc' _ ih =>
    intro v' hv
    simp only [coerceV, Functor.map, Except.map] at hv
    rcases hcl : coerceL c _ with e | ys <;> rw [hcl] at hv <;> simp at hv
    subst hv
    obtain ⟨hlen, hys⟩ := coerceL_ok hcl
    simp only [Abs.scaled]
    refine Mem.list (by rw [hlen]; exact hc') ?_
    intro y hy
    obtain ⟨x, hx, hxy⟩ := hys y hy
    exact ih x hx hxy
  | optNone => intro v' hv; simp [coerceV] at hv; subst hv; simp only [Abs.scaled]; exact Mem.optNone
  | optSome hne _ ih =>
    intro v' hv
    simp only [Abs.scaled]
    exact Mem.optSome (coerceV_ne_none hne hv) (ih hv)
  | _ =>
    intro v' hv
    simp only [coerceV] at hv; cases hv
    simp only [Abs.scaled]
    first | (constructor <;> assumption) | constructor | assumption

theorem Mem_coerce {cv : Option Conv} {a : Abs} {v v' : Val} (h : Mem a v) (hc : coerce cv v = .ok v') :
    Mem (scaleBy cv a) v' := by
  cases cv with
  | none => simp [coerce] at hc; subst hc; exact h
  | some c => exact Mem_coerceV h hc

theorem Mem_numPart_num {a : Abs} {r : Rat} (h : Mem a (.num r)) : Mem (numPart a) (.num r) := by
  generalize hv : Val.num r = v at h
  induction h with
  | optSome hne _ ih => cases hv; simp only [numPart]; exact Mem.optSome hne (ih rfl)
  | list => cases hv
  | _ => first | (simp only [numPart]; first | (constructor <;> assumption) | constructor | assumption) | cases hv

theorem Mem_coerceV_error {c : Conv} {a : Abs} {v : Val} (h : Mem a v) : ∀ {e : Stop}, coerceV c v = .error e →
    e = .failed .overflow c.line ∧ ∃ r, Mem (numPart (a.scaled c.f c.shift)) (.num r) ∧ fits r = false := by
  induction h with
  | any =>
    intro e he
    refine ⟨coerceV_error c _ e he, (2 : Rat) ^ 64, Mem.any _, ?_⟩
    decide
  | num hi =>
    intro e he
    simp only [coerceV, Functor.map, Except.map] at he
    rcases hc : convN (some c) _ with e' | r' <;> rw [hc] at he <;> simp at he
    subst he
    obtain ⟨h1, h2⟩ := convN_error hc
    exact ⟨h1, _, by simp only [Abs.scaled, numPart]; exact Mem.num (In_scaled hi), h2⟩
  | list _ _ ih =>
    intro e he
    simp only [coerceV, Functor.map, Except.map] at he
    rcases hcl : coerceL c _ with e' | ys <;> rw [hcl] at he <;> simp at he
    subst he
    obtain ⟨x, hx, hx'⟩ := coerceL_error hcl
    obtain ⟨h1, r, hm, hr⟩ := ih x hx hx'
    exact ⟨h1, r, by simp only [Abs.scaled, numPart]; exact hm, hr⟩
  | optNone => intro e he; simp [coerceV] at he
  | optSome _ _ ih =>
    intro e he
    obtain ⟨h1, r, hm, hr⟩ := ih he
    exact ⟨h1, r, by simp only [Abs.scaled, numPart]; exact Mem.optSome (by simp) hm, hr⟩
  | _ => intro e he; simp [coerceV] at he

theorem ivDeep_numPart : ∀ {b : Abs} {r : Rat}, Mem (numPart b) (.num r) → ∀ {i : Iv}, ivDeep b = some i → i.In r
  | .num j, r, h, i, hi => by
    simp [ivDeep] at hi; subst hi; simp only [numPart] at h; cases h; assumption
  | .date j, r, h, i, _ => by simp only [numPart] at h; cases h
  | .opt n x, r, h, i, hi => by
    simp only [numPart] at h
    simp only [ivDeep] at hi
    cases h with
    | optSome _ hx => exact ivDeep_numPart hx hi
  | .list c x, r, h, i, hi => by
    simp only [numPart] at h; simp only [ivDeep] at hi; exact ivDeep_numPart h hi
  | .never, r, h, i, hi => by simp [ivDeep] at hi
  | .any, r, h, i, hi => by simp [ivDeep] at hi
  | .set _, r, h, i, hi => by simp [ivDeep] at hi
  | .str, r, h, i, hi => by simp [ivDeep] at hi
  | .record _ _, r, h, i, hi => by simp [ivDeep] at hi
  | .transfer _, r, h, i, hi => by simp [ivDeep] at hi

theorem hasAny_numPart : ∀ {b : Abs} {r : Rat}, Mem (numPart b) (.num r) → ivDeep b = none → hasAny b = true
  | .num j, r, h, hb => by simp [ivDeep] at hb
  | .date j, r, h, hb => by simp [ivDeep] at hb
  | .opt n x, r, h, hb => by
    simp only [numPart] at h; simp only [ivDeep] at hb; simp only [hasAny]
    cases h with
    | optSome _ hx => exact hasAny_numPart hx hb
  | .list c x, r, h, hb => by
    simp only [numPart] at h; simp only [ivDeep] at hb; simp only [hasAny]; exact hasAny_numPart h hb
  | .any, r, h, hb => rfl
  | .never, r, h, hb => by simp only [numPart] at h; cases h
  | .set _, r, h, hb => by simp only [numPart] at h; cases h
  | .str, r, h, hb => by simp only [numPart] at h; cases h
  | .record _ _, r, h, hb => by simp only [numPart] at h; cases h
  | .transfer _, r, h, hb => by simp only [numPart] at h; cases h

-- ── int64 ────────────────────────────────────────────────────────────────────

theorem okNum_sound {a : Abs} {r : Rat} (h : Mem a (.num r)) (hok : okNum a = true) : fits r = true := by
  generalize hv : Val.num r = v at h
  induction h with
  | num hi => cases hv; simp only [okNum] at hok; exact fits_sound hi hok
  | optSome _ _ ih => simp only [okNum] at hok; exact ih hok hv
  | any => simp [okNum] at hok
  | _ => cases hv


theorem neg_of_num_neg {x : Rat} (h : x.num < 0) : x < 0 := by
  have : ¬ (0 ≤ x) := fun h' => by have := Rat.num_nonneg.mpr h'; omega
  exact Rat.not_le.mp this

theorem num_neg_of_neg {x : Rat} (h : x < 0) : x.num < 0 := by
  have : ¬ (0 ≤ x.num) := fun h' => by have := Rat.num_nonneg.mp h'; exact absurd h (Rat.not_lt.mpr this)
  omega

theorem lo_fits_of_hull {lo lh : Rat} {d dh : Nat} (hl : lh ≤ lo) (hd : d ≤ dh)
    (h : lh.num ≥ 0 ∨ (i64min : Rat) ≤ lh * dh) : lo.num ≥ 0 ∨ (i64min : Rat) ≤ lo * d := by
  by_cases h0 : lo.num ≥ 0
  · exact Or.inl h0
  · right
    have hlo : lo < 0 := neg_of_num_neg (by omega)
    have hlh : lh < 0 := rlt_of_le_of_lt hl hlo
    have hlhn : ¬ lh.num ≥ 0 := by have := num_neg_of_neg hlh; omega
    have h' : (i64min : Rat) ≤ lh * dh := by rcases h with h | h; exact absurd h hlhn; exact h
    have hd0 : (0 : Rat) ≤ d := by exact_mod_cast Nat.zero_le d
    have hdd : (d : Rat) ≤ dh := by exact_mod_cast hd
    have a1 : lh * (d : Rat) ≤ lo * (d : Rat) := Rat.mul_le_mul_of_nonneg_right hl hd0
    have a2 : lh * (dh : Rat) ≤ lh * (d : Rat) := by
      have hn : 0 ≤ -lh := by grind
      have := Rat.mul_le_mul_of_nonneg_left hdd hn
      grind
    exact Rat.le_trans h' (Rat.le_trans a2 a1)

theorem hi_fits_of_hull {hi hh : Rat} {d dh : Nat} (hl : hi ≤ hh) (hd : d ≤ dh)
    (h : hh.num ≤ 0 ∨ hh * dh ≤ (i64max : Rat)) : hi.num ≤ 0 ∨ hi * d ≤ (i64max : Rat) := by
  by_cases h0 : hi.num ≤ 0
  · exact Or.inl h0
  · right
    have hhi : 0 < hi := by
      have : 0 < hi.num := by omega
      exact (num_pos_iff hi).mp this
    have hhh : 0 < hh := rlt_of_lt_of_le hhi hl
    have hhn : ¬ hh.num ≤ 0 := by have := (num_pos_iff hh).mpr hhh; omega
    have h' : hh * (dh : Rat) ≤ (i64max : Rat) := by rcases h with h | h; exact absurd h hhn; exact h
    have hd0 : (0 : Rat) ≤ d := by exact_mod_cast Nat.zero_le d
    have hdd : (d : Rat) ≤ dh := by exact_mod_cast hd
    have a1 : hi * (d : Rat) ≤ hh * (d : Rat) := Rat.mul_le_mul_of_nonneg_right hl hd0
    have a2 : hh * (d : Rat) ≤ hh * (dh : Rat) := Rat.mul_le_mul_of_nonneg_left hdd (Rat.le_of_lt hhh)
    exact Rat.le_trans a1 (Rat.le_trans a2 h')

/-- An interval wider than another fits only where the other does. -/
theorem fits_hull {i o : Iv} (h : (i.hull o).fits = true) : i.fits = true ∧ o.fits = true := by
  unfold Iv.hull at h
  unfold Iv.fits at h ⊢
  rcases hil : i.lo with _ | l1 <;> rcases hol : o.lo with _ | l2 <;>
    simp only [hil, hol, minLo] at h <;> try (simp at h; done)
  rcases hih : i.hi with _ | h1 <;> rcases hoh : o.hi with _ | h2 <;>
    simp only [hih, hoh, maxHi] at h <;> try (simp at h; done)
  rcases hid : i.den with _ | d1 <;> rcases hod : o.den with _ | d2 <;>
    simp only [hid, hod] at h <;> try (simp at h; done)
  simp only [Bool.and_eq_true, Bool.or_eq_true, decide_eq_true_eq] at h ⊢
  obtain ⟨⟨hD, hL⟩, hH⟩ := h
  have hd1 : d1 ≤ max d1 d2 := Nat.le_max_left _ _
  have hd2 : d2 ≤ max d1 d2 := Nat.le_max_right _ _
  have hlo1 : (if l1 ≤ l2 then l1 else l2) ≤ l1 := by split <;> grind
  have hlo2 : (if l1 ≤ l2 then l1 else l2) ≤ l2 := by split <;> grind
  have hhi1 : h1 ≤ (if h2 ≤ h1 then h1 else h2) := by split <;> grind
  have hhi2 : h2 ≤ (if h2 ≤ h1 then h1 else h2) := by split <;> grind
  have hD' : ((max d1 d2 : Nat) : Int) ≤ i64max := by exact_mod_cast hD
  refine ⟨⟨⟨by omega, lo_fits_of_hull hlo1 hd1 hL⟩, hi_fits_of_hull hhi1 hd1 hH⟩,
    ⟨⟨by omega, lo_fits_of_hull hlo2 hd2 hL⟩, hi_fits_of_hull hhi2 hd2 hH⟩⟩

/-- **A place for int64 not shown stays not shown when it takes more values.** -/
theorem okNum_join : ∀ (d : Nat) (a b : Abs), okNum (Abs.joinD d a b) = true → okNum a = true ∧ okNum b = true
  | 0, a, b, h => by simp [Abs.joinD, okNum] at h
  | d + 1, a, b, h => by
    have ih := okNum_join d
    cases a <;> cases b <;> simp [Abs.joinD, okNum] at h ⊢ <;>
      first
        | exact h
        | exact fits_hull h
        | exact ih _ _ h
        | exact ⟨(ih _ _ h).2, (ih _ _ h).1⟩
        | exact (ih _ _ h).1
        | exact (ih _ _ h).2

theorem okNum_join' {a b : Abs} (h : okNum (a.join b) = true) : okNum a = true := (okNum_join D a b h).1

-- ── Cut to what is declared ──────────────────────────────────────────────────

/-- **A value in both is in the one cut to the other.** -/
theorem cutD_sound : ∀ (d : Nat) (a b : Abs) (v : Val), Mem a v → Mem b v → Mem (cutD d a b) v
  | 0, a, b, v, ha, _ => by simp only [cutD]; exact ha
  | d + 1, a, b, v, ha, hb => by
    cases ha with
    | num hi =>
      cases hb with
      | num hj =>
        simp only [cutD]
        obtain ⟨m, hm, hin⟩ := In_meet hi hj
        rw [hm]; simp only [Option.map_some, Option.getD_some]
        exact Mem.num (In_withDen hin hi.den)
      | any => simp only [cutD]; exact Mem.num hi
      | optSome _ hx => simp only [cutD]; exact Mem.num hi
    | date hi =>
      cases hb with
      | date hj =>
        simp only [cutD]
        obtain ⟨m, hm, hin⟩ := In_meet hi hj
        rw [hm]; exact Mem.date hin
      | any => simp only [cutD]; exact Mem.date hi
      | optSome _ hx => simp only [cutD]; exact Mem.date hi
    | optNone =>
      cases hb with
      | optNone => simp only [cutD]; exact Mem.optNone
      | any => simp only [cutD]; exact Mem.optNone
      | optSome hne _ => exact absurd rfl hne
    | optSome hne hx =>
      cases hb with
      | optNone => exact absurd rfl hne
      | any => simp only [cutD]; exact Mem.optSome hne hx
      | optSome _ hy => simp only [cutD]; exact Mem.optSome hne (cutD_sound d _ _ v hx hy)
      | _ => simp only [cutD]; exact Mem.optSome hne hx
    | list hc he =>
      cases hb with
      | list hc' he' =>
        simp only [cutD]
        obtain ⟨m, hm, hin⟩ := In_meet hc hc'
        rw [hm]; simp only [Option.getD_some]
        exact Mem.list hin (fun x hx => cutD_sound d _ _ x (he x hx) (he' x hx))
      | any => simp only [cutD]; exact Mem.list hc he
      | optSome _ hy => simp only [cutD]; exact Mem.list hc he
    | _ => simp only [cutD]; first | (constructor <;> assumption) | constructor | assumption

-- ── Bools ────────────────────────────────────────────────────────────────────

theorem mem_setOf {x : String} {xs : List String} : x ∈ setOf xs ↔ x ∈ xs := by
  unfold setOf; rw [mem_union]; simp

theorem truths_true {a : Abs} (h : Mem a (.bool true)) : (truths a).1 = true := by
  cases h <;> simp_all [truths]

theorem truths_false {a : Abs} (h : Mem a (.bool false)) : (truths a).2 = true := by
  cases h <;> simp_all [truths]

theorem Mem_bools {t f b : Bool} (ht : b = true → t = true) (hf : b = false → f = true) :
    Mem (bools t f) (.bool b) := by
  unfold bools
  apply Mem.bool
  cases b <;> simp_all

theorem Mem_boolAbs (b : Bool) : Mem (boolAbs b) (.bool b) := Mem.bool (by simp)

theorem Mem_none' : Mem Abs.none' .none := Mem.optNone

theorem Mem_not {s : List String} {b : Bool} (h : Mem (.set (some s)) (.bool b)) :
    Mem (.set (some (setOf (s.map fun x => if x == "true" then "false" else "true")))) (.bool !b) := by
  cases h with
  | bool hb =>
    apply Mem.bool
    rw [mem_setOf, List.mem_map]
    exact ⟨_, hb, by cases b <;> simp⟩

end KanameCore
