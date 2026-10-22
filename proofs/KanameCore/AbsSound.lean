/-
  The abstract values hold the values they are about (kaname's DESIGN 5.1, 15.2.2).

  `Mem a v` says the value `v` is among what `a` says: a number in its interval, a value of an enum
  or a bool in its set, `none` where the value may be none, a list whose count is in the interval
  and whose every element is among what each element can be, a record whose every field is among
  what that field can be and whose fields keep the relations the record says, a value of an enum
  that has fields one of the values it can be, each of its fields one the value has and among what
  that field can be (DESIGN 23.7), a dict whose count of pairs is in the interval and whose every key
  and value is among what each can be, a set likewise of its members (DESIGN 24.5), a function value
  one of the lambdas it can be, each name it caught with a value among what the lambda caught of it,
  or any where it can be one the analysis does not see (DESIGN 27.4). The evaluation's own test
  (`Abs.has`) says no more than `Mem`: `has_sound`.

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
  /-- A string: its count of code points in the interval (DESIGN 25.6). -/
  | str {c : Iv} {s : String} : c.In s.length → Mem (.str c) (.str s)
  | optNone {x : Abs} : Mem (.opt true x) .none
  | optSome {n : Bool} {x : Abs} {v : Val} : v ≠ .none → Mem x v → Mem (.opt n x) v
  | list {c : Iv} {e : Abs} {xs : List Val} : c.In xs.length → (∀ x ∈ xs, Mem e x) → Mem (.list c e) (.list xs)
  | record {fs : List (String × Abs)} {rels : List (String × Rel × String)} {vs : List (String × Val)} :
      (∀ n x, fieldV vs n = some x → ∀ a, Abs.fieldOf fs n = some a → Mem a x) → RelsHold rels vs →
      Mem (.record fs rels) (.record vs)
  | transfer {as : List (String × Abs)} {id : String} {vs : List (String × Val)} :
      (∀ n x, fieldV vs n = some x → ∀ a, Abs.fieldOf as n = some a → Mem a x) → Mem (.transfer as) (.transfer id vs)
  /-- A value of an enum by its name alone, one of the values it can be. -/
  | variantEnum {vs : List (String × List (String × Abs))} {x : String} :
      (Abs.recOf vs x).isSome = true → Mem (.variants vs) (.enum x)
  /-- A value of an enum with its fields (some), one of the values it can be: each field one that
      value has, and among what it can be. -/
  | variant {vs : List (String × List (String × Abs))} {x : String} {r : List (String × Abs)} {fs : List (String × Val)} :
      Abs.recOf vs x = some r → fs ≠ [] →
      (∀ n y, fieldV fs n = some y → (Abs.fieldOf r n).isSome = true) →
      (∀ n y, fieldV fs n = some y → ∀ a, Abs.fieldOf r n = some a → Mem a y) →
      Mem (.variants vs) (.variant x fs)
  /-- A dict: its count of pairs in the interval, each key and each value among what they can be. -/
  | dict {c : Iv} {k v : Abs} {ps : List (Val × Val)} : c.In ps.length → (∀ p ∈ ps, Mem k p.1) →
      (∀ p ∈ ps, Mem v p.2) → (ps.map (·.1)).Nodup → Mem (.dict c k v) (.dict ps)
  /-- A set: its count of members in the interval, each among what a member can be, each once. -/
  | members {c : Iv} {e : Abs} {xs : List Val} : c.In xs.length → (∀ x ∈ xs, Mem e x) → xs.Nodup →
      Mem (.members c e) (.set xs)
  /-- A function value where one the analysis does not see is said: any (DESIGN 27.4). -/
  | closAny {fs : List (Nat × List (String × Abs) × Bool)} {w : Option (List Kind × Bool)} {i : Nat}
      {env : List (String × Val)} : Mem (.fns fs (some w)) (.closure i env)
  /-- A lambda the function value can be: each name it caught, as the analysis read it where the
      lambda was made, has a value, among what it read (`Sat`'s environment, kept with the lambda). -/
  | clos {fs : List (Nat × List (String × Abs) × Bool)} {u : Option (Option (List Kind × Bool))} {i : Nat}
      {ae : List (String × Abs)} {own : Bool} {env : List (String × Val)} : (i, ae, own) ∈ fs →
      (∀ x a, (ae.find? (·.1 == x)).map (·.2) = some a → (Env.get? env x).isSome = true) →
      (∀ x a v, (ae.find? (·.1 == x)).map (·.2) = some a → Env.get? env x = some v → Mem a v) →
      Mem (.fns fs u) (.closure i env)

-- ── Two values that are one ────────────────────────────────────────────────

mutual
theorem Val.beq_refl : ∀ (a : Val), Val.beq a a = true
  | .num r => by simp [Val.beq]
  | .bool b => by simp [Val.beq]
  | .str s => by simp [Val.beq]
  | .date d => by simp [Val.beq]
  | .enum v => by simp [Val.beq]
  | .record fs => by simp only [Val.beq]; exact Val.beqFields_refl fs
  | .list xs => by simp only [Val.beq]; exact Val.beqList_refl xs
  | .none => by simp [Val.beq]
  | .transfer id args => by simp only [Val.beq, beq_self_eq_true, Bool.true_and]; exact Val.beqFields_refl args
  | .variant t fs => by simp only [Val.beq, beq_self_eq_true, Bool.true_and]; exact Val.beqFields_refl fs
  | .dict ps => by simp only [Val.beq]; exact Val.beqPairs_refl ps
  | .set xs => by simp only [Val.beq]; exact Val.beqList_refl xs
  | .closure i env => by simp only [Val.beq, beq_self_eq_true, Bool.true_and]; exact Val.beqFields_refl env
theorem Val.beqFields_refl : ∀ (xs : List (String × Val)), Val.beqFields xs xs = true
  | [] => by simp [Val.beqFields]
  | (n, x) :: xs => by
    simp only [Val.beqFields, beq_self_eq_true, Bool.true_and]
    rw [Val.beq_refl x, Val.beqFields_refl xs]; rfl
theorem Val.beqList_refl : ∀ (xs : List Val), Val.beqList xs xs = true
  | [] => by simp [Val.beqList]
  | x :: xs => by simp only [Val.beqList]; rw [Val.beq_refl x, Val.beqList_refl xs]; rfl
theorem Val.beqPairs_refl : ∀ (ps : List (Val × Val)), Val.beqPairs ps ps = true
  | [] => by simp [Val.beqPairs]
  | (k, x) :: ps => by simp only [Val.beqPairs]; rw [Val.beq_refl k, Val.beq_refl x, Val.beqPairs_refl ps]; rfl
end

mutual
theorem Val.eq_of_beq : ∀ {a b : Val}, Val.beq a b = true → a = b
  | .num r, b, h => by cases b <;> simp_all [Val.beq]
  | .bool x, b, h => by cases b <;> simp_all [Val.beq]
  | .str x, b, h => by cases b <;> simp_all [Val.beq]
  | .date x, b, h => by cases b <;> simp_all [Val.beq]
  | .enum x, b, h => by cases b <;> simp_all [Val.beq]
  | .none, b, h => by cases b <;> simp_all [Val.beq]
  | .record fs, b, h => by
    cases b with
    | record gs => simp only [Val.beq] at h; rw [Val.eq_of_beqFields h]
    | _ => simp [Val.beq] at h
  | .list xs, b, h => by
    cases b with
    | list ys => simp only [Val.beq] at h; rw [Val.eq_of_beqList h]
    | _ => simp [Val.beq] at h
  | .transfer i a, b, h => by
    cases b with
    | transfer j c =>
      simp only [Val.beq, Bool.and_eq_true, beq_iff_eq] at h
      rw [h.1, Val.eq_of_beqFields h.2]
    | _ => simp [Val.beq] at h
  | .variant t a, b, h => by
    cases b with
    | variant u c =>
      simp only [Val.beq, Bool.and_eq_true, beq_iff_eq] at h
      rw [h.1, Val.eq_of_beqFields h.2]
    | _ => simp [Val.beq] at h
  | .dict ps, b, h => by
    cases b with
    | dict qs => simp only [Val.beq] at h; rw [Val.eq_of_beqPairs h]
    | _ => simp [Val.beq] at h
  | .set xs, b, h => by
    cases b with
    | set ys => simp only [Val.beq] at h; rw [Val.eq_of_beqList h]
    | _ => simp [Val.beq] at h
  | .closure i a, b, h => by
    cases b with
    | closure j c =>
      simp only [Val.beq, Bool.and_eq_true, beq_iff_eq] at h
      rw [h.1, Val.eq_of_beqFields h.2]
    | _ => simp [Val.beq] at h
theorem Val.eq_of_beqFields : ∀ {xs ys : List (String × Val)}, Val.beqFields xs ys = true → xs = ys
  | [], [], _ => rfl
  | (n, x) :: xs, (m, y) :: ys, h => by
    simp only [Val.beqFields, Bool.and_eq_true, beq_iff_eq] at h
    obtain ⟨⟨h1, h2⟩, h3⟩ := h
    rw [h1, Val.eq_of_beq h2, Val.eq_of_beqFields h3]
  | [], _ :: _, h => by simp [Val.beqFields] at h
  | _ :: _, [], h => by simp [Val.beqFields] at h
theorem Val.eq_of_beqList : ∀ {xs ys : List Val}, Val.beqList xs ys = true → xs = ys
  | [], [], _ => rfl
  | x :: xs, y :: ys, h => by
    simp only [Val.beqList, Bool.and_eq_true] at h
    rw [Val.eq_of_beq h.1, Val.eq_of_beqList h.2]
  | [], _ :: _, h => by simp [Val.beqList] at h
  | _ :: _, [], h => by simp [Val.beqList] at h
theorem Val.eq_of_beqPairs : ∀ {xs ys : List (Val × Val)}, Val.beqPairs xs ys = true → xs = ys
  | [], [], _ => rfl
  | (k, x) :: xs, (l, y) :: ys, h => by
    simp only [Val.beqPairs, Bool.and_eq_true] at h
    obtain ⟨⟨h1, h2⟩, h3⟩ := h
    rw [Val.eq_of_beq h1, Val.eq_of_beq h2, Val.eq_of_beqPairs h3]
  | [], _ :: _, h => by simp [Val.beqPairs] at h
  | _ :: _, [], h => by simp [Val.beqPairs] at h
end

/-- `==` on values is their equality. -/
theorem Val.beq_iff {a b : Val} : (a == b) = true ↔ a = b :=
  ⟨Val.eq_of_beq, fun h => h ▸ Val.beq_refl a⟩

mutual
theorem Abs.eq_of_beq : ∀ {a b : Abs}, Abs.beq a b = true → a = b
  | .never, b, h => by cases b <;> simp_all [Abs.beq]
  | .any, b, h => by cases b <;> simp_all [Abs.beq]
  | .num i, b, h => by cases b <;> simp_all [Abs.beq]
  | .date i, b, h => by cases b <;> simp_all [Abs.beq]
  | .set s, b, h => by cases b <;> simp_all [Abs.beq]
  | .str i, b, h => by cases b <;> simp_all [Abs.beq]
  | .opt n a, b, h => by
    cases b with
    | opt m b' =>
      simp only [Abs.beq, Bool.and_eq_true, beq_iff_eq] at h
      rw [h.1, Abs.eq_of_beq h.2]
    | _ => simp [Abs.beq] at h
  | .list c a, b, h => by
    cases b with
    | list d b' =>
      simp only [Abs.beq, Bool.and_eq_true, beq_iff_eq] at h
      rw [h.1, Abs.eq_of_beq h.2]
    | _ => simp [Abs.beq] at h
  | .record f r, b, h => by
    cases b with
    | record g s =>
      simp only [Abs.beq, Bool.and_eq_true, beq_iff_eq] at h
      rw [Abs.eq_of_beqFields h.1, h.2]
    | _ => simp [Abs.beq] at h
  | .transfer f, b, h => by
    cases b with
    | transfer g =>
      simp only [Abs.beq] at h
      rw [Abs.eq_of_beqFields h]
    | _ => simp [Abs.beq] at h
  | .variants f, b, h => by
    cases b with
    | variants g =>
      simp only [Abs.beq] at h
      rw [Abs.eq_of_beqVariants h]
    | _ => simp [Abs.beq] at h
  | .dict c k a, b, h => by
    cases b with
    | dict d l b' =>
      simp only [Abs.beq, Bool.and_eq_true, beq_iff_eq] at h
      rw [h.1.1, Abs.eq_of_beq h.1.2, Abs.eq_of_beq h.2]
    | _ => simp [Abs.beq] at h
  | .members c a, b, h => by
    cases b with
    | members d b' =>
      simp only [Abs.beq, Bool.and_eq_true, beq_iff_eq] at h
      rw [h.1, Abs.eq_of_beq h.2]
    | _ => simp [Abs.beq] at h
  | .fns a u, b, h => by
    cases b with
    | fns b' w =>
      simp only [Abs.beq, Bool.and_eq_true, beq_iff_eq] at h
      rw [Abs.eq_of_beqLams h.1, h.2]
    | _ => simp [Abs.beq] at h
theorem Abs.eq_of_beqLams : ∀ {xs ys : List (Nat × List (String × Abs) × Bool)}, Abs.beqLams xs ys = true → xs = ys
  | [], [], _ => rfl
  | (i, e, o) :: xs, (j, f, p) :: ys, h => by
    simp only [Abs.beqLams, Bool.and_eq_true, beq_iff_eq] at h
    obtain ⟨⟨⟨h1, h2⟩, h3⟩, h4⟩ := h
    rw [h1, Abs.eq_of_beqFields h2, h3, Abs.eq_of_beqLams h4]
  | [], _ :: _, h => by simp [Abs.beqLams] at h
  | _ :: _, [], h => by simp [Abs.beqLams] at h
theorem Abs.eq_of_beqFields : ∀ {xs ys : List (String × Abs)}, Abs.beqFields xs ys = true → xs = ys
  | [], [], _ => rfl
  | (n, x) :: xs, (m, y) :: ys, h => by
    simp only [Abs.beqFields, Bool.and_eq_true, beq_iff_eq] at h
    obtain ⟨⟨h1, h2⟩, h3⟩ := h
    rw [h1, Abs.eq_of_beq h2, Abs.eq_of_beqFields h3]
  | [], _ :: _, h => by simp [Abs.beqFields] at h
  | _ :: _, [], h => by simp [Abs.beqFields] at h
theorem Abs.eq_of_beqVariants : ∀ {xs ys : List (String × List (String × Abs))}, Abs.beqVariants xs ys = true → xs = ys
  | [], [], _ => rfl
  | (t, f) :: xs, (u, g) :: ys, h => by
    simp only [Abs.beqVariants, Bool.and_eq_true, beq_iff_eq] at h
    obtain ⟨⟨h1, h2⟩, h3⟩ := h
    rw [h1, Abs.eq_of_beqFields h2, Abs.eq_of_beqVariants h3]
  | [], _ :: _, h => by simp [Abs.beqVariants] at h
  | _ :: _, [], h => by simp [Abs.beqVariants] at h
end

theorem nodupB_sound : ∀ {xs : List Val}, nodupB xs = true → xs.Nodup
  | [], _ => List.nodup_nil
  | x :: rest, h => by
    simp only [nodupB, Bool.and_eq_true, Bool.not_eq_true', List.any_eq_false] at h
    refine List.nodup_cons.mpr ⟨fun hx => ?_, nodupB_sound h.2⟩
    have := h.1 x hx
    exact this (Val.beq_iff.mpr rfl)

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

/-- A function value where one the analysis does not see is said. -/
theorem mem_closAny {fs : List (Nat × List (String × Abs) × Bool)} {u : Option (Option (List Kind × Bool))} {i : Nat}
    {env : List (String × Val)} (h : u.isSome = true) : Mem (.fns fs u) (.closure i env) := by
  cases u with
  | none => simp at h
  | some w => exact Mem.closAny

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
    | str c =>
      cases v <;> simp only [Abs.hasD] at h <;> (try contradiction)
      exact Mem.str ((Iv.has_iff _ _).mp h)
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
    | variants vs =>
      cases v <;> simp only [Abs.hasD] at h <;> (try contradiction)
      · exact Mem.variantEnum h
      · rename_i x fs
        rcases hr : Abs.recOf vs x with _ | r <;> rw [hr] at h
        · simp at h
        · simp only [Bool.and_eq_true, Bool.not_eq_true', List.isEmpty_eq_false_iff] at h
          obtain ⟨hne, hall⟩ := h
          have each : ∀ n y, fieldV fs n = some y → ∃ a, Abs.fieldOf r n = some a ∧ Mem a y := by
            intro n y hy
            have := List.all_eq_true.mp hall (n, y) (fieldV_mem hy)
            simp only at this
            rcases ha : Abs.fieldOf r n with _ | a <;> rw [ha] at this
            · simp at this
            · exact ⟨a, rfl, has_sound d a y this⟩
          refine Mem.variant hr hne (fun n y hy => ?_) (fun n y hy a ha => ?_)
          · obtain ⟨a, ha, _⟩ := each n y hy; rw [ha]; rfl
          · obtain ⟨a', ha', hm⟩ := each n y hy; rw [ha'] at ha; cases ha; exact hm
    | dict c k x =>
      cases v <;> simp only [Abs.hasD] at h <;> (try contradiction)
      rename_i ps
      simp only [Bool.and_eq_true] at h
      obtain ⟨⟨h1, h2⟩, h3⟩ := h
      refine Mem.dict ((Iv.has_iff _ _).mp h1) (fun p hp => ?_) (fun p hp => ?_) (nodupB_sound h3)
      · have := List.all_eq_true.mp h2 p hp
        simp only [Bool.and_eq_true] at this
        exact has_sound d k _ this.1
      · have := List.all_eq_true.mp h2 p hp
        simp only [Bool.and_eq_true] at this
        exact has_sound d x _ this.2
    | members c e =>
      cases v <;> simp only [Abs.hasD] at h <;> (try contradiction)
      rename_i xs
      simp only [Bool.and_eq_true] at h
      obtain ⟨⟨h1, h2⟩, h3⟩ := h
      exact Mem.members ((Iv.has_iff _ _).mp h1) (fun x hx => has_sound d _ x (List.all_eq_true.mp h2 x hx))
        (nodupB_sound h3)
    | fns fs u =>
      cases v <;> simp only [Abs.hasD] at h <;> (try contradiction)
      exact mem_closAny h

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

-- ── The values of an enum that has fields ────────────────────────────────────

theorem recOf_mem {vs : List (String × List (String × Abs))} {t : String} {r : List (String × Abs)}
    (h : Abs.recOf vs t = some r) : (t, r) ∈ vs := by
  unfold Abs.recOf at h
  rcases hf : vs.find? (·.1 == t) with _ | ⟨k, s⟩ <;> rw [hf] at h <;> simp at h
  subst h
  have hm := List.mem_of_find?_eq_some hf
  have hk := List.find?_some hf
  simp at hk; subst hk; exact hm

theorem fieldOf_append (l1 l2 : List (String × Abs)) (n : String) :
    Abs.fieldOf (l1 ++ l2) n = (Abs.fieldOf l1 n).or (Abs.fieldOf l2 n) := by
  unfold Abs.fieldOf
  rw [List.find?_append]
  cases List.find? (fun p => p.1 == n) l1 <;> simp

theorem fieldOf_filter_none {s : List (String × Abs)} {n : String} (hs : Abs.fieldOf s n = none) :
    ∀ (r : List (String × Abs)), Abs.fieldOf (r.filter (fun p => (Abs.fieldOf s p.1).isNone)) n = Abs.fieldOf r n
  | [] => rfl
  | (m, a) :: rest => by
    by_cases hm : m = n
    · subst hm
      rw [List.filter_cons_of_pos (by simp [hs])]
      simp [Abs.fieldOf, List.find?]
    · have hmn : (m == n) = false := by simp [hm]
      have ih := fieldOf_filter_none hs rest
      unfold Abs.fieldOf at ih ⊢
      simp only [List.filter]
      split <;> simp only [List.find?, hmn] <;> exact ih

theorem fieldOf_joinRecV_left {g : Abs → Abs → Abs} {s r : List (String × Abs)} {n : String} {a : Abs}
    (h : Abs.fieldOf s n = some a) : Abs.fieldOf (Abs.joinRecV g s r) n = some (Abs.joinField g r n a) := by
  unfold Abs.joinRecV
  rw [fieldOf_append, fieldOf_joinFields, h]
  rfl

theorem fieldOf_joinRecV_right {g : Abs → Abs → Abs} {s r : List (String × Abs)} {n : String} {b : Abs}
    (h : Abs.fieldOf r n = some b) :
    Abs.fieldOf (Abs.joinRecV g s r) n = some b ∨ ∃ a, Abs.fieldOf (Abs.joinRecV g s r) n = some (g a b) := by
  unfold Abs.joinRecV
  rw [fieldOf_append, fieldOf_joinFields]
  rcases hs : Abs.fieldOf s n with _ | a
  · left
    simp only [Option.map_none, Option.none_or]
    rw [fieldOf_filter_none hs, h]
  · right
    refine ⟨a, ?_⟩
    simp only [Option.map_some, Option.some_or]
    unfold Abs.joinField
    rw [h]

theorem recOf_vIns {v : String} {r : List (String × Abs)} :
    ∀ (out : List (String × List (String × Abs))), out.any (·.1 == v) = false → ∀ t,
      Abs.recOf (Abs.vIns v r out) t = if t = v then some r else Abs.recOf out t
  | [], _, t => by
    by_cases ht : t = v
    · subst ht; simp [Abs.vIns, Abs.recOf]
    · have : (v == t) = false := by simp; exact fun e => ht e.symm
      simp [Abs.vIns, Abs.recOf, List.find?, this, ht]
  | (w, s) :: rest, hno, t => by
    simp only [List.any_cons, Bool.or_eq_false_iff] at hno
    obtain ⟨hw, hrest⟩ := hno
    have hwv : w ≠ v := by simpa using hw
    unfold Abs.vIns
    split
    · by_cases ht : t = v
      · subst ht; simp [Abs.recOf, List.find?]
      · have : (v == t) = false := by simp; exact fun e => ht e.symm
        simp only [Abs.recOf, List.find?, this, ht, ite_false]
    · have ih := recOf_vIns (v := v) (r := r) rest hrest t
      by_cases htw : t = w
      · subst htw
        have : t ≠ v := hwv
        simp [Abs.recOf, List.find?, this]
      · have hwt : (w == t) = false := by simp; exact fun e => htw e.symm
        unfold Abs.recOf at ih ⊢
        simp only [List.find?, hwt]
        exact ih

theorem recOf_map_put {g : Abs → Abs → Abs} {v : String} {r : List (String × Abs)} :
    ∀ (out : List (String × List (String × Abs))) (t : String),
      Abs.recOf (out.map (fun (w, s) => if w == v then (w, Abs.joinRecV g s r) else (w, s))) t =
        (Abs.recOf out t).map (fun s => if t = v then Abs.joinRecV g s r else s)
  | [], _ => rfl
  | (w, s) :: rest, t => by
    have ih := recOf_map_put (g := g) (v := v) (r := r) rest t
    by_cases htw : w = t
    · subst htw
      by_cases hv : w = v
      · subst hv; simp [Abs.recOf, List.find?]
      · simp [Abs.recOf, List.find?, hv]
    · have hwt : (w == t) = false := by simp [htw]
      by_cases hv : w = v
      · subst hv
        unfold Abs.recOf at ih ⊢
        simp only [List.map_cons, beq_self_eq_true, ite_true, List.find?, hwt]
        exact ih
      · have hwv : (w == v) = false := by simp [hv]
        unfold Abs.recOf at ih ⊢
        simp only [List.map_cons, hwv, Bool.false_eq_true, ite_false, List.find?, hwt]
        exact ih

/-- Where one value of an enum is put, the values it can be: that one, joined with the same value
    where it was there; the others as they were. -/
theorem recOf_vPut {g : Abs → Abs → Abs} {v : String} {r : List (String × Abs)}
    (out : List (String × List (String × Abs))) (t : String) :
    Abs.recOf (Abs.vPut g v r out) t =
      if t = v then some (match Abs.recOf out v with | some s => Abs.joinRecV g s r | none => r)
      else Abs.recOf out t := by
  unfold Abs.vPut
  split
  · rename_i hany
    rw [recOf_map_put]
    by_cases ht : t = v
    · subst ht
      rcases hr : Abs.recOf out t with _ | s
      · exfalso
        obtain ⟨⟨w, s⟩, hm, hw⟩ := List.any_eq_true.mp hany
        simp at hw; subst hw
        unfold Abs.recOf at hr
        have hn : List.find? (fun x => x.1 == w) out = none := by simpa using hr
        rw [List.find?_eq_none] at hn
        exact absurd (by simp) (hn _ hm)
      · simp
    · simp only [ht, ite_false]
      cases Abs.recOf out t <;> simp
  · rename_i hany
    rw [recOf_vIns out (Bool.eq_false_iff.mpr hany) t]
    by_cases ht : t = v
    · subst ht
      have : Abs.recOf out t = none := by
        unfold Abs.recOf
        have : List.find? (fun x => x.1 == t) out = none := by
          rw [List.find?_eq_none]
          intro x hx hxt
          exact hany (List.any_eq_true.mpr ⟨x, hx, hxt⟩)
        simp [this]
      simp [this]
    · simp [ht]

/-- What a value of an enum keeps of the values it can be, where one more is put among them: the
    same, where the value put joins what the value's fields are with more. -/
theorem mem_vPut_left {g : Abs → Abs → Abs} (hg : ∀ a b v, Mem a v → Mem (g a b) v)
    {v : String} {r : List (String × Abs)} {out : List (String × List (String × Abs))} {x : Val}
    (h : Mem (.variants out) x) : Mem (.variants (Abs.vPut g v r out)) x := by
  cases h with
  | variantEnum hs =>
    rename_i t
    apply Mem.variantEnum
    rw [recOf_vPut]
    split <;> simp_all
  | variant hr hne hsome hmem =>
    rename_i t s fs
    by_cases ht : t = v
    · subst ht
      refine Mem.variant (by rw [recOf_vPut, ite_eq_left_of_eq_true _ _ (eq_self t), hr]) hne (fun n y hy => ?_) (fun n y hy a ha => ?_)
      · obtain ⟨b, hb⟩ := Option.isSome_iff_exists.mp (hsome n y hy)
        rw [fieldOf_joinRecV_left hb]; rfl
      · obtain ⟨b, hb⟩ := Option.isSome_iff_exists.mp (hsome n y hy)
        rw [fieldOf_joinRecV_left hb] at ha
        cases ha
        have hm := hmem n y hy b hb
        unfold Abs.joinField
        split
        · exact hg _ _ _ hm
        · exact Mem.any _
    · exact Mem.variant (by rw [recOf_vPut]; simp only [ht, ↓reduceIte, hr]) hne hsome hmem

theorem mem_fold_left {g : Abs → Abs → Abs} (hg : ∀ a b v, Mem a v → Mem (g a b) v) {x : Val} :
    ∀ (b : List (String × List (String × Abs))) (out : List (String × List (String × Abs))),
      Mem (.variants out) x → Mem (.variants (b.foldl (fun out (v, r) => Abs.vPut g v r out) out)) x
  | [], _, h => h
  | (v, r) :: rest, out, h => by
    simp only [List.foldl_cons]
    exact mem_fold_left hg rest _ (mem_vPut_left hg h)

theorem recOf_cons_ne {t w : String} {r : List (String × Abs)} {rest : List (String × List (String × Abs))}
    (h : t ≠ w) : Abs.recOf ((w, r) :: rest) t = Abs.recOf rest t := by
  have : (w == t) = false := by simp; exact fun e => h e.symm
  simp [Abs.recOf, List.find?, this]

theorem recOf_cons_eq {t : String} {r : List (String × Abs)} {rest : List (String × List (String × Abs))} :
    Abs.recOf ((t, r) :: rest) t = some r := by
  simp [Abs.recOf, List.find?]

/-- A value of an enum that one side can be, the other side's values put among it: still one it
    can be. -/
theorem mem_fold_right {g : Abs → Abs → Abs} (hgl : ∀ a b v, Mem a v → Mem (g a b) v)
    (hgr : ∀ a b v, Mem b v → Mem (g a b) v) {x : Val} :
    ∀ (b : List (String × List (String × Abs))) (out : List (String × List (String × Abs))),
      Mem (.variants b) x → Mem (.variants (b.foldl (fun out (v, r) => Abs.vPut g v r out) out)) x
  | [], _, h => by cases h <;> simp_all [Abs.recOf]
  | (w, r) :: rest, out, h => by
    simp only [List.foldl_cons]
    cases h with
    | variantEnum hs =>
      rename_i t
      by_cases ht : t = w
      · subst ht
        apply mem_fold_left hgl
        apply Mem.variantEnum
        rw [recOf_vPut]; simp
      · rw [recOf_cons_ne ht] at hs
        exact mem_fold_right hgl hgr rest _ (Mem.variantEnum hs)
    | variant hr hne hsome hmem =>
      rename_i t s fs
      by_cases ht : t = w
      · subst ht
        rw [recOf_cons_eq] at hr; cases hr
        apply mem_fold_left hgl
        refine Mem.variant (by rw [recOf_vPut, ite_eq_left_of_eq_true _ _ (eq_self t)]) hne (fun n y hy => ?_) (fun n y hy a ha => ?_)
        · obtain ⟨b, hb⟩ := Option.isSome_iff_exists.mp (hsome n y hy)
          split
          · rename_i s0 _
            rcases fieldOf_joinRecV_right (g := g) (s := s0) hb with e | ⟨a, e⟩ <;> rw [e] <;> rfl
          · rw [hb]; rfl
        · obtain ⟨b, hb⟩ := Option.isSome_iff_exists.mp (hsome n y hy)
          have hm := hmem n y hy b hb
          split at ha
          · rename_i s0 _
            rcases fieldOf_joinRecV_right (g := g) (s := s0) hb with e | ⟨a', e⟩ <;> rw [e] at ha <;> cases ha
            · exact hm
            · exact hgr _ _ _ hm
          · rw [hb] at ha; cases ha; exact hm
      · rw [recOf_cons_ne ht] at hr
        exact mem_fold_right hgl hgr rest _ (Mem.variant hr hne hsome hmem)

/-- Two lambdas `lamBeq` says are one are one. -/
theorem lamBeq_eq {x y : Nat × List (String × Abs) × Bool} (h : Abs.lamBeq x y = true) : x = y := by
  obtain ⟨i, e, o⟩ := x
  obtain ⟨j, f, p⟩ := y
  simp only [Abs.lamBeq, Bool.and_eq_true, beq_iff_eq] at h
  obtain ⟨⟨h1, h2⟩, h3⟩ := h
  rw [h1, Abs.eq_of_beqFields h2, h3]

theorem unionStep_keeps : ∀ (b acc : List (Nat × List (String × Abs) × Bool)) (x : Nat × List (String × Abs) × Bool),
    x ∈ acc → x ∈ b.foldl (fun acc x => if acc.any (Abs.lamBeq x) then acc else acc ++ [x]) acc
  | [], _, _, h => h
  | _ :: b, acc, x, h => by
    simp only [List.foldl_cons]
    apply unionStep_keeps b
    split
    · exact h
    · exact List.mem_append_left _ h

theorem unionStep_has : ∀ (b acc : List (Nat × List (String × Abs) × Bool)) (x : Nat × List (String × Abs) × Bool),
    x ∈ b → x ∈ b.foldl (fun acc x => if acc.any (Abs.lamBeq x) then acc else acc ++ [x]) acc
  | [], _, _, h => by cases h
  | y :: b, acc, x, h => by
    simp only [List.foldl_cons]
    rcases List.mem_cons.mp h with rfl | h
    · apply unionStep_keeps b
      split
      · rename_i ha
        obtain ⟨z, hz, hb⟩ := List.any_eq_true.mp ha
        rw [lamBeq_eq hb]; exact hz
      · exact List.mem_append_right _ (List.mem_singleton_self _)
    · exact unionStep_has b _ x h

/-- The lambdas of either are among the lambdas of both. -/
theorem mem_unionLams_left {a b : List (Nat × List (String × Abs) × Bool)} {x : Nat × List (String × Abs) × Bool}
    (h : x ∈ a) : x ∈ Abs.unionLams a b := unionStep_keeps b a x h

theorem mem_unionLams_right {a b : List (Nat × List (String × Abs) × Bool)} {x : Nat × List (String × Abs) × Bool}
    (h : x ∈ b) : x ∈ Abs.unionLams a b := unionStep_has b a x h

/-- A function the analysis does not see, joined with anything, is one. -/
theorem joinUnseen_left {u w : Option (Option (List Kind × Bool))} (h : u.isSome = true) : (Abs.joinUnseen u w).isSome = true := by
  rcases u with _ | _ | ⟨ka, da⟩
  · simp at h
  · rcases w with _ | _ | ⟨kb, db⟩ <;> simp [Abs.joinUnseen]
  · rcases w with _ | _ | ⟨kb, db⟩ <;> simp [Abs.joinUnseen]

theorem joinUnseen_right {u w : Option (Option (List Kind × Bool))} (h : w.isSome = true) : (Abs.joinUnseen u w).isSome = true := by
  rcases w with _ | _ | ⟨kb, db⟩
  · simp at h
  · rcases u with _ | _ | ⟨ka, da⟩ <;> simp [Abs.joinUnseen]
  · rcases u with _ | _ | ⟨ka, da⟩ <;> simp [Abs.joinUnseen]

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
      cases b <;> cases h <;> simp only [Abs.joinD, Abs.joinVariants]
      all_goals first
        | exact Mem.any _
        | exact Mem.variantEnum (by assumption)
        | exact Mem.variant (by assumption) (by assumption) (by assumption) (by assumption)
        | exact mem_fold_left ihl _ _ (Mem.variantEnum (by assumption))
        | exact mem_fold_left ihl _ _ (Mem.variant (by assumption) (by assumption) (by assumption) (by assumption))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.variantEnum (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.variant (by assumption) (by assumption) (by assumption) (by assumption)))
        | exact Mem.num (In_hull_left (by assumption))
        | exact Mem.date (In_hull_left (by assumption))
        | exact Mem.num (by assumption)
        | exact Mem.date (by assumption)
        | exact Mem.enumAll
        | exact Mem.boolAll
        | exact Mem.str (In_hull_left (by assumption))
        | exact Mem.str (by assumption)
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
        | exact Mem.dict (In_hull_left (by assumption)) (fun p hp => ihl _ _ _ ((by assumption : ∀ p ∈ _, Mem _ p.1) p hp))
            (fun p hp => ihl _ _ _ ((by assumption : ∀ p ∈ _, Mem _ p.2) p hp)) (by assumption)
        | exact Mem.dict (by assumption) (by assumption) (by assumption) (by assumption)
        | exact Mem.members (In_hull_left (by assumption)) (fun x hx => ihl _ _ _ ((by assumption : ∀ x ∈ _, Mem _ x) x hx))
            (by assumption)
        | exact Mem.members (by assumption) (by assumption) (by assumption)
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.dict (by assumption) (by assumption) (by assumption) (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.members (by assumption) (by assumption) (by assumption)))
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
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.str (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.list (by assumption) (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.record (by assumption) (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.transfer (by assumption)))
        | exact Mem.closAny
        | exact Mem.clos (by assumption) (by assumption) (by assumption)
        | exact mem_closAny (joinUnseen_left rfl)
        | exact Mem.clos (mem_unionLams_left (by assumption)) (by assumption) (by assumption)
        | exact Mem.optSome (by simp) (ihr _ _ _ Mem.closAny)
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.clos (by assumption) (by assumption) (by assumption)))
    · intro a b v h
      cases a <;> cases h <;> simp only [Abs.joinD, Abs.joinVariants]
      all_goals first
        | exact Mem.any _
        | exact Mem.variantEnum (by assumption)
        | exact Mem.variant (by assumption) (by assumption) (by assumption) (by assumption)
        | exact mem_fold_right ihl ihr _ _ (Mem.variantEnum (by assumption))
        | exact mem_fold_right ihl ihr _ _ (Mem.variant (by assumption) (by assumption) (by assumption) (by assumption))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.variantEnum (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.variant (by assumption) (by assumption) (by assumption) (by assumption)))
        | exact Mem.optSome (by assumption) (ihl _ _ _ (Mem.variantEnum (by assumption)))
        | exact Mem.optSome (by assumption) (ihl _ _ _ (Mem.variant (by assumption) (by assumption) (by assumption) (by assumption)))
        | exact Mem.num (In_hull_right (by assumption))
        | exact Mem.date (In_hull_right (by assumption))
        | exact Mem.num (by assumption)
        | exact Mem.date (by assumption)
        | exact Mem.enumAll
        | exact Mem.boolAll
        | exact Mem.str (In_hull_right (by assumption))
        | exact Mem.str (by assumption)
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
        | exact Mem.dict (In_hull_right (by assumption)) (fun p hp => ihr _ _ _ ((by assumption : ∀ p ∈ _, Mem _ p.1) p hp))
            (fun p hp => ihr _ _ _ ((by assumption : ∀ p ∈ _, Mem _ p.2) p hp)) (by assumption)
        | exact Mem.dict (by assumption) (by assumption) (by assumption) (by assumption)
        | exact Mem.members (In_hull_right (by assumption)) (fun x hx => ihr _ _ _ ((by assumption : ∀ x ∈ _, Mem _ x) x hx))
            (by assumption)
        | exact Mem.members (by assumption) (by assumption) (by assumption)
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.dict (by assumption) (by assumption) (by assumption) (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.members (by assumption) (by assumption) (by assumption)))
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
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.str (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.list (by assumption) (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.record (by assumption) (by assumption)))
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.transfer (by assumption)))
        | exact Mem.closAny
        | exact Mem.clos (by assumption) (by assumption) (by assumption)
        | exact mem_closAny (joinUnseen_right rfl)
        | exact Mem.clos (mem_unionLams_right (by assumption)) (by assumption) (by assumption)
        | exact Mem.optSome (by simp) (ihr _ _ _ Mem.closAny)
        | exact Mem.optSome (by simp) (ihr _ _ _ (Mem.clos (by assumption) (by assumption) (by assumption)))

theorem mem_join_left {a b : Abs} {v : Val} (h : Mem a v) : Mem (a.join b) v := (join_mem D).1 a b v h
theorem mem_join_right {a b : Abs} {v : Val} (h : Mem b v) : Mem (a.join b) v := (join_mem D).2 a b v h

-- ── Fields, intervals, optionals ─────────────────────────────────────────────

/-- The field `f` of every value of an enum that has it, joined: a value of the field of one of them
    is in it. -/
theorem mem_variantField_acc {f : String} {w : Val} : ∀ (vs : List (String × List (String × Abs))) (acc : Abs),
    (Mem acc w ∨ ∃ t r a, (t, r) ∈ vs ∧ Abs.fieldOf r f = some a ∧ Mem a w) →
    Mem (vs.foldl (fun acc (_, fs) => match Abs.fieldOf fs f with
      | some a => acc.join a
      | Option.none => acc) acc) w
  | [], acc, h => by
    rcases h with h | ⟨_, _, _, hm, _⟩
    · exact h
    · simp at hm
  | (t, r) :: rest, acc, h => by
    simp only [List.foldl_cons]
    apply mem_variantField_acc rest
    rcases h with h | ⟨t', r', a, hm, ha, hw⟩
    · left
      split
      · exact mem_join_left h
      · exact h
    · simp only [List.mem_cons] at hm
      rcases hm with e | hm
      · simp only [Prod.mk.injEq] at e
        obtain ⟨rfl, rfl⟩ := e
        left
        rw [ha]
        exact mem_join_right hw
      · right; exact ⟨t', r', a, hm, ha, hw⟩

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
  | @variant vs x r fs hr _ hsome hmem _ =>
    intro f w hw
    have hw' : fieldV fs f = some w := by simpa [Val.field?, fieldV] using hw
    obtain ⟨a, ha⟩ := Option.isSome_iff_exists.mp (hsome f w hw')
    simp only [Abs.field, Abs.variantField]
    exact mem_variantField_acc vs .never (Or.inr ⟨x, r, a, recOf_mem hr, ha, hmem f w hw' a ha⟩)
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
  | .str _, r, h, i, hi => by simp [ivDeep] at hi
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
  | .str _, r, h, hb => by simp only [numPart] at h; cases h
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
    | dict hc hk hv hnd =>
      cases hb with
      | dict hc' hk' hv' _ =>
        simp only [cutD]
        obtain ⟨m, hm, hin⟩ := In_meet hc hc'
        rw [hm]; simp only [Option.getD_some]
        exact Mem.dict hin (fun p hp => cutD_sound d _ _ _ (hk p hp) (hk' p hp))
          (fun p hp => cutD_sound d _ _ _ (hv p hp) (hv' p hp)) hnd
      | any => simp only [cutD]; exact Mem.dict hc hk hv hnd
      | optSome _ hy => simp only [cutD]; exact Mem.dict hc hk hv hnd
    | members hc he hnd =>
      cases hb with
      | members hc' he' _ =>
        simp only [cutD]
        obtain ⟨m, hm, hin⟩ := In_meet hc hc'
        rw [hm]; simp only [Option.getD_some]
        exact Mem.members hin (fun x hx => cutD_sound d _ _ x (he x hx) (he' x hx)) hnd
      | any => simp only [cutD]; exact Mem.members hc he hnd
      | optSome _ hy => simp only [cutD]; exact Mem.members hc he hnd
    | str hc =>
      cases hb with
      | str hc' =>
        simp only [cutD]
        obtain ⟨m, hm, hin⟩ := In_meet hc hc'
        rw [hm]; simp only [Option.getD_some]
        exact Mem.str hin
      | any => simp only [cutD]; exact Mem.str hc
      | optSome _ hy => simp only [cutD]; exact Mem.str hc
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
