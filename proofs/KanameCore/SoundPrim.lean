/-
  The builtins of dicts, sets and lists (kaname's DESIGN 24, 15.2.2). What each answers is among what
  the analysis says of it, and where one fails with one of the five kinds (`numbers`' count past
  int64, a value read in another unit), the analysis kept a place.

  A dict holds each key once and a set each member once (`Mem`), and every builtin that builds one
  keeps it so (`dictFrom`, `setFrom`, `groupsOf`): so the counts the analysis says hold. A pair put in
  is one more pair, or a key the dict has; two dicts or two sets put together have at least as many as
  the larger; one taken from another, at least as many as the first has less the second's.
-/
import KanameCore.SoundArith

namespace KanameCore

instance : LawfulBEq Val where
  eq_of_beq h := Val.eq_of_beq h
  rfl := Val.beq_refl _

theorem sameKey_iff {a b : Val} : sameKey a b = true ↔ a = b := Val.beq_iff

-- ── Lists that hold each value once ─────────────────────────────────────────

/-- A list that holds each value once, all of whose values another holds, is no longer. -/
theorem length_le_of_sub {α : Type} [BEq α] [LawfulBEq α] : ∀ {xs ys : List α}, xs.Nodup → (∀ x ∈ xs, x ∈ ys) →
    xs.length ≤ ys.length
  | [], _, _, _ => Nat.zero_le _
  | x :: xs, ys, hn, hs => by
    have hx : x ∈ ys := hs x List.mem_cons_self
    obtain ⟨hnx, hn'⟩ := List.nodup_cons.mp hn
    have h1 : xs.length ≤ (ys.erase x).length := length_le_of_sub hn' (fun y hy => by
      have hne : y ≠ x := fun h => hnx (h ▸ hy)
      exact (List.mem_erase_of_ne hne).mpr (hs y (List.mem_cons_of_mem _ hy)))
    rw [List.length_erase_of_mem hx] at h1
    have : 0 < ys.length := List.length_pos_of_mem hx
    simp only [List.length_cons]; omega

theorem any_sameKey_iff {xs : List Val} {x : Val} : (xs.any fun y => sameKey y x) = true ↔ x ∈ xs := by
  constructor
  · intro h
    obtain ⟨y, hy, he⟩ := List.any_eq_true.mp h
    rw [sameKey_iff] at he; subst he; exact hy
  · intro h
    exact List.any_eq_true.mpr ⟨x, h, sameKey_iff.mpr rfl⟩

-- ── The first of each ───────────────────────────────────────────────────────

theorem firstOfEachAux_sublist : ∀ (seen xs : List Val), (firstOfEachAux seen xs).Sublist xs
  | _, [] => by simp [firstOfEachAux]
  | seen, x :: rest => by
    simp only [firstOfEachAux]
    split
    · exact (firstOfEachAux_sublist seen rest).cons x
    · exact (firstOfEachAux_sublist (x :: seen) rest).cons_cons x

theorem firstOfEachAux_fresh : ∀ (seen xs : List Val) (y : Val), y ∈ firstOfEachAux seen xs → y ∉ seen
  | _, [], _, h => by simp [firstOfEachAux] at h
  | seen, x :: rest, y, h => by
    simp only [firstOfEachAux] at h
    split at h
    · exact firstOfEachAux_fresh seen rest y h
    · rename_i hx
      rcases List.mem_cons.mp h with rfl | h
      · intro hy; exact hx (any_sameKey_iff.mpr hy)
      · exact fun hy => firstOfEachAux_fresh (x :: seen) rest y h (List.mem_cons_of_mem _ hy)

theorem firstOfEachAux_nodup : ∀ (seen xs : List Val), (firstOfEachAux seen xs).Nodup
  | _, [] => by simp [firstOfEachAux]
  | seen, x :: rest => by
    simp only [firstOfEachAux]
    split
    · exact firstOfEachAux_nodup seen rest
    · exact List.nodup_cons.mpr ⟨fun h => firstOfEachAux_fresh (x :: seen) rest x h List.mem_cons_self,
        firstOfEachAux_nodup (x :: seen) rest⟩

theorem firstOfEachAux_complete : ∀ (seen xs : List Val) (y : Val), y ∈ xs → y ∉ seen → y ∈ firstOfEachAux seen xs
  | _, [], _, h, _ => by simp at h
  | seen, x :: rest, y, h, hs => by
    simp only [firstOfEachAux]
    split
    · rename_i hx
      rcases List.mem_cons.mp h with rfl | h
      · exact absurd (any_sameKey_iff.mp hx) hs
      · exact firstOfEachAux_complete seen rest y h hs
    · by_cases hy : y = x
      · subst hy; exact List.mem_cons_self
      · rcases List.mem_cons.mp h with rfl | h
        · exact absurd rfl hy
        · refine List.mem_cons_of_mem _ (firstOfEachAux_complete (x :: seen) rest y h ?_)
          intro hm
          rcases List.mem_cons.mp hm with rfl | hm
          · exact hy rfl
          · exact hs hm

theorem firstOfEach_sublist (xs : List Val) : (firstOfEach xs).Sublist xs := firstOfEachAux_sublist [] xs
theorem firstOfEach_nodup (xs : List Val) : (firstOfEach xs).Nodup := firstOfEachAux_nodup [] xs
theorem firstOfEach_mem {xs : List Val} {y : Val} : y ∈ firstOfEach xs ↔ y ∈ xs :=
  ⟨fun h => (firstOfEach_sublist xs).subset h, fun h => firstOfEachAux_complete [] xs y h (by simp)⟩

theorem firstOfEach_length_le (xs : List Val) : (firstOfEach xs).length ≤ xs.length := (firstOfEach_sublist xs).length_le

theorem firstOfEach_ne_nil {xs : List Val} (h : xs ≠ []) : firstOfEach xs ≠ [] := by
  obtain ⟨x, hx⟩ := List.exists_mem_of_ne_nil xs h
  intro he
  have := firstOfEach_mem.mpr hx
  rw [he] at this; simp at this

/-- A list that holds each value once, all among another's, is no longer than the first of each of the other's. -/
theorem firstOfEach_length_ge {xs ys : List Val} (hn : xs.Nodup) (hs : ∀ x ∈ xs, x ∈ ys) :
    xs.length ≤ (firstOfEach ys).length :=
  length_le_of_sub hn (fun x hx => firstOfEach_mem.mpr (hs x hx))

-- ── The last pair of each key ───────────────────────────────────────────────

theorem lastOfEach_sublist : ∀ (ps : List (Val × Val)), (lastOfEach ps).Sublist ps
  | [] => by simp [lastOfEach]
  | p :: rest => by
    simp only [lastOfEach]
    split
    · exact (lastOfEach_sublist rest).cons p
    · exact (lastOfEach_sublist rest).cons_cons p

theorem lastOfEach_keys_nodup : ∀ (ps : List (Val × Val)), ((lastOfEach ps).map (·.1)).Nodup
  | [] => by simp [lastOfEach]
  | p :: rest => by
    simp only [lastOfEach]
    split
    · exact lastOfEach_keys_nodup rest
    · rename_i hp
      simp only [List.map_cons]
      refine List.nodup_cons.mpr ⟨fun h => ?_, lastOfEach_keys_nodup rest⟩
      obtain ⟨q, hq, he⟩ := List.mem_map.mp h
      have hq' := (lastOfEach_sublist rest).subset hq
      exact hp (List.any_eq_true.mpr ⟨q, hq', sameKey_iff.mpr he⟩)

theorem lastOfEach_keys : ∀ (ps : List (Val × Val)) (p : Val × Val), p ∈ ps → ∃ q ∈ lastOfEach ps, q.1 = p.1
  | [], _, h => by simp at h
  | r :: rest, p, h => by
    simp only [lastOfEach]
    split
    · rename_i hr
      rcases List.mem_cons.mp h with rfl | h
      · obtain ⟨q, hq, he⟩ := List.any_eq_true.mp hr
        rw [sameKey_iff] at he
        obtain ⟨q', hq', he'⟩ := lastOfEach_keys rest q hq
        exact ⟨q', hq', he'.trans he⟩
      · exact lastOfEach_keys rest p h
    · rcases List.mem_cons.mp h with rfl | h
      · exact ⟨p, List.mem_cons_self, rfl⟩
      · obtain ⟨q, hq, he⟩ := lastOfEach_keys rest p h
        exact ⟨q, List.mem_cons_of_mem _ hq, he⟩

-- ── A dict built ─────────────────────────────────────────────────────────────

theorem dictFrom_perm (k : Key) (ps : List (Val × Val)) : (dictFrom k ps).Perm (lastOfEach ps) :=
  List.mergeSort_perm _ _

theorem dictFrom_mem {k : Key} {ps : List (Val × Val)} {q : Val × Val} (h : q ∈ dictFrom k ps) : q ∈ ps :=
  (lastOfEach_sublist ps).subset ((dictFrom_perm k ps).mem_iff.mp h)

theorem dictFrom_nodup (k : Key) (ps : List (Val × Val)) : ((dictFrom k ps).map (·.1)).Nodup :=
  ((dictFrom_perm k ps).map (·.1)).nodup_iff.mpr (lastOfEach_keys_nodup ps)

theorem dictFrom_keys {k : Key} {ps : List (Val × Val)} {p : Val × Val} (h : p ∈ ps) :
    p.1 ∈ (dictFrom k ps).map (·.1) := by
  obtain ⟨q, hq, he⟩ := lastOfEach_keys ps p h
  exact List.mem_map.mpr ⟨q, (dictFrom_perm k ps).mem_iff.mpr hq, he⟩

theorem dictFrom_length_le (k : Key) (ps : List (Val × Val)) : (dictFrom k ps).length ≤ ps.length := by
  rw [(dictFrom_perm k ps).length_eq]; exact (lastOfEach_sublist ps).length_le

/-- Pairs of keys each once, all among a dict's pairs built, are no more than its pairs. -/
theorem dictFrom_length_ge {k : Key} {ps qs : List (Val × Val)} (hn : (qs.map (·.1)).Nodup) (hs : ∀ q ∈ qs, q ∈ ps) :
    qs.length ≤ (dictFrom k ps).length := by
  have : (qs.map (·.1)).length ≤ ((dictFrom k ps).map (·.1)).length := length_le_of_sub hn (fun x hx => by
    obtain ⟨q, hq, rfl⟩ := List.mem_map.mp hx
    exact dictFrom_keys (k := k) (hs q hq))
  simpa using this

theorem dictFrom_ne_nil {k : Key} {ps : List (Val × Val)} (h : ps ≠ []) : dictFrom k ps ≠ [] := by
  obtain ⟨p, hp⟩ := List.exists_mem_of_ne_nil ps h
  intro he
  have := dictFrom_keys (k := k) hp
  rw [he] at this; simp at this

-- ── A set built ──────────────────────────────────────────────────────────────

theorem setFrom_perm (k : Key) (xs : List Val) : (setFrom k xs).Perm (firstOfEach xs) := List.mergeSort_perm _ _

theorem setFrom_mem {k : Key} {xs : List Val} {y : Val} : y ∈ setFrom k xs ↔ y ∈ xs :=
  ((setFrom_perm k xs).mem_iff).trans firstOfEach_mem

theorem setFrom_nodup (k : Key) (xs : List Val) : (setFrom k xs).Nodup :=
  (setFrom_perm k xs).nodup_iff.mpr (firstOfEach_nodup xs)

theorem setFrom_length_le (k : Key) (xs : List Val) : (setFrom k xs).length ≤ xs.length := by
  rw [(setFrom_perm k xs).length_eq]; exact firstOfEach_length_le xs

theorem setFrom_length_ge {k : Key} {xs ys : List Val} (hn : xs.Nodup) (hs : ∀ x ∈ xs, x ∈ ys) :
    xs.length ≤ (setFrom k ys).length := by
  rw [(setFrom_perm k ys).length_eq]; exact firstOfEach_length_ge hn hs

theorem setFrom_ne_nil {k : Key} {xs : List Val} (h : xs ≠ []) : setFrom k xs ≠ [] := by
  obtain ⟨x, hx⟩ := List.exists_mem_of_ne_nil xs h
  intro he
  have := setFrom_mem (k := k) |>.mpr hx
  rw [he] at this; simp at this

/-- A set's members that are no member of another: none of them, and every one of the first. -/
theorem nodup_filter {xs : List Val} {p : Val → Bool} (h : xs.Nodup) : (xs.filter p).Nodup :=
  h.sublist List.filter_sublist

-- ── Counts ───────────────────────────────────────────────────────────────────

theorem capCount_nonneg (v : Rat) : 0 ≤ capCount v := by
  unfold capCount
  split
  · exact Rat.le_refl
  · split
    · exact_mod_cast (show (0 : Int) ≤ i64max by decide)
    · rename_i h1 _
      have : ¬ v < 0 := fun h => h1 ((num_neg_iff v).mpr h)
      grind

/-- **A count the analysis says**: a count of `L` (at most 2⁶³ − 1) between the ends, where an upper
    end below 0 says none and 0 is the count. -/
theorem countIv_sound {lo hi : End} {L : Nat} (hl : EndLe lo (L : Rat))
    (hh : ∀ h, hi = some h → (L : Rat) ≤ h ∨ L = 0) (hm : (L : Int) ≤ i64max) : (countIv lo hi).In (L : Rat) := by
  have hL0 : (0 : Rat) ≤ (L : Rat) := by exact_mod_cast Nat.zero_le L
  have hLm : (L : Rat) ≤ ((i64max : Int) : Rat) := by exact_mod_cast hm
  -- the lower end
  have hlo : (lo.map capCount).getD 0 ≤ (L : Rat) := by
    cases lo with
    | none => simpa using hL0
    | some lv =>
      simp only [EndLe] at hl
      simp only [Option.map_some, Option.getD_some]
      unfold capCount
      split
      · exact hL0
      · split
        · rename_i _ h2
          exact absurd (rlt_of_lt_of_le h2 (Rat.le_trans hl hLm)) (fun h => Rat.lt_irrefl h)
        · exact hl
  unfold countIv
  refine In_mk' (by simpa [EndLe] using hlo) ?_ (by simp [DenLe])
  cases hi with
  | none => simp [EndGe]
  | some hv =>
    have hcap : (L : Rat) ≤ capCount hv := by
      rcases hh hv rfl with h | h
      · unfold capCount
        split
        · rename_i hn
          have : hv < 0 := (num_neg_iff hv).mp hn
          grind
        · split
          · exact hLm
          · exact h
      · subst h
        simpa using capCount_nonneg hv
    simp only [Option.map_some, EndGe]
    split
    · rename_i hlt
      exact Rat.le_of_lt (rlt_of_le_of_lt hcap hlt)
    · exact hcap

-- ── The parts of what a builtin is given ────────────────────────────────────

theorem anyCount_In (n : Nat) : anyCount.In (n : Rat) :=
  In_mk' (by simp only [EndLe]; exact_mod_cast Nat.zero_le n) trivial (by simp [DenLe])

theorem anyCount_lo : anyCount.lo = some 0 := by
  simp [anyCount, Iv.mk', normLo, tooBig, FINE]

theorem listParts_sound {xs : List Val} : ∀ {a : Abs}, Mem a (.list xs) →
    (listParts a).1.In xs.length ∧ ∀ x ∈ xs, Mem (listParts a).2 x
  | _, .any _ => ⟨anyCount_In _, fun x _ => Mem.any x⟩
  | _, .optSome _ hx => by simp only [listParts]; exact listParts_sound hx
  | _, .list hc he => ⟨hc, he⟩

theorem dictParts_sound {ps : List (Val × Val)} : ∀ {a : Abs}, Mem a (.dict ps) →
    (dictParts a).1.In ps.length ∧ (∀ p ∈ ps, Mem (dictParts a).2.1 p.1) ∧ (∀ p ∈ ps, Mem (dictParts a).2.2 p.2) ∧
      ((ps.map (·.1)).Nodup ∨ (dictParts a).1.lo = some 0)
  | _, .any _ => ⟨anyCount_In _, fun p _ => Mem.any _, fun p _ => Mem.any _, Or.inr anyCount_lo⟩
  | _, .optSome _ hx => by simp only [dictParts]; exact dictParts_sound hx
  | _, .dict hc hk hv hn => ⟨hc, hk, hv, Or.inl hn⟩

theorem membersParts_sound {xs : List Val} : ∀ {a : Abs}, Mem a (.set xs) →
    (membersParts a).1.In xs.length ∧ (∀ x ∈ xs, Mem (membersParts a).2 x) ∧
      (xs.Nodup ∨ (membersParts a).1.lo = some 0)
  | _, .any _ => ⟨anyCount_In _, fun x _ => Mem.any _, Or.inr anyCount_lo⟩
  | _, .optSome _ hx => by simp only [membersParts]; exact membersParts_sound hx
  | _, .members hc he hn => ⟨hc, he, Or.inl hn⟩

theorem Mem_optOf {may : Bool} {a : Abs} {v : Val} (h : Mem a v) : Mem (optOf may a) v := by
  cases a with
  | opt n x =>
    simp only [optOf]
    cases h with
    | optNone => simp only [Bool.or_true]; exact Mem.optNone
    | optSome hne hx => exact Mem.optSome hne hx
  | any => simp only [optOf]; exact h
  | _ =>
    simp only [optOf]
    have hne : v ≠ .none := by
      intro hv; subst hv
      rcases Mem.none_cases h with he | ⟨x, he⟩ <;> cases he
    exact Mem.optSome hne h

theorem Mem_optOf_none {may : Bool} {a : Abs} (h : may = true) : Mem (optOf may a) .none := by
  subst h
  cases a with
  | opt n x => simp only [optOf, Bool.true_or]; exact Mem.optNone
  | any => exact Mem.any _
  | _ => exact Mem.optNone

/-- A record of two fields, each among what it can be. -/
theorem Mem_record2 {n1 n2 : String} {a1 a2 : Abs} {v1 v2 : Val} (h1 : Mem a1 v1) (h2 : Mem a2 v2) :
    Mem (.record [(n1, a1), (n2, a2)] []) (.record [(n1, v1), (n2, v2)]) := by
  refine Mem.record (fun n x hx a ha => ?_) (fun a r b h => by simp at h)
  simp only [fieldV, Abs.fieldOf, List.find?] at hx ha
  by_cases e1 : (n1 == n) = true
  · rw [e1] at hx ha
    simp only [Option.map_some, Option.some.injEq] at hx ha
    rw [← hx, ← ha]; exact h1
  · rw [Bool.not_eq_true] at e1
    rw [e1] at hx ha
    by_cases e2 : (n2 == n) = true
    · rw [e2] at hx ha
      simp only [Option.map_some, Option.some.injEq] at hx ha
      rw [← hx, ← ha]; exact h2
    · rw [Bool.not_eq_true] at e2
      rw [e2] at hx
      simp at hx

theorem Mem_entry {k v : Abs} {p : Val × Val} (hk : Mem k p.1) (hv : Mem v p.2) : Mem (entryAbs k v) (entryV p) :=
  Mem_record2 hk hv

/-- What `sized` lets through is what it is given. -/
theorem sized_eq {v v' : Val} (h : sized v = .ok v') : v' = v := by
  unfold sized at h
  split at h <;> (try split at h) <;> first | (cases h; rfl) | cases h

/-- A list, a set, a dict `sized` lets through has at most 2⁶³ − 1 elements, members, pairs. -/
theorem sized_list {xs : List Val} {v' : Val} (h : sized (.list xs) = .ok v') : (xs.length : Int) ≤ i64max := by
  simp only [sized] at h
  split at h
  · cases h
  · omega

theorem sized_set {xs : List Val} {v' : Val} (h : sized (.set xs) = .ok v') : (xs.length : Int) ≤ i64max := by
  simp only [sized] at h
  split at h
  · cases h
  · omega

theorem sized_dict {ps : List (Val × Val)} {v' : Val} (h : sized (.dict ps) = .ok v') : (ps.length : Int) ≤ i64max := by
  simp only [sized] at h
  split at h
  · cases h
  · omega

theorem endLe_atLeastOne {lo : End} {L : Nat} (h1 : 1 ≤ L) (h : EndLe lo (L : Rat)) : EndLe (atLeastOne lo) (L : Rat) := by
  cases lo with
  | none => trivial
  | some l =>
    simp only [atLeastOne, Option.map_some, EndLe] at h ⊢
    split
    · exact_mod_cast h1
    · exact h

theorem endLe_mono {lo : End} {a b : Nat} (h : EndLe lo (a : Rat)) (hab : a ≤ b) : EndLe lo (b : Rat) := by
  cases lo with
  | none => trivial
  | some l => simp only [EndLe] at h ⊢; exact Rat.le_trans h (by exact_mod_cast hab)

theorem endGe_mono {hi : End} {a b : Nat} (h : EndGe hi (a : Rat)) (hab : b ≤ a) : EndGe hi (b : Rat) := by
  cases hi with
  | none => trivial
  | some u => simp only [EndGe] at h ⊢; exact Rat.le_trans (by exact_mod_cast hab) h

theorem endLe_zero {L : Nat} : EndLe (some 0) (L : Rat) := by simp only [EndLe]; exact_mod_cast Nat.zero_le L

/-- The lower end of a count that is a dict's, a set's, where its keys are each once or it says none. -/
theorem endLe_count {lo : End} {n L : Nat} (h : EndLe lo (n : Rat)) (hge : n ≤ L ∨ lo = some 0) : EndLe lo (L : Rat) := by
  rcases hge with hge | hz
  · exact endLe_mono h hge
  · subst hz; exact endLe_zero

-- ── A dict ───────────────────────────────────────────────────────────────────

/-- A key taken out of a dict that holds each key once: one pair fewer at most. -/
theorem length_filter_key {ps : List (Val × Val)} {x : Val} (hn : (ps.map (·.1)).Nodup) :
    ps.length ≤ (ps.filter fun p => !sameKey p.1 x).length + 1 := by
  induction ps with
  | nil => simp
  | cons p rest ih =>
    simp only [List.map_cons] at hn
    obtain ⟨hp, hn'⟩ := List.nodup_cons.mp hn
    simp only [List.filter_cons]
    split
    · simp only [List.length_cons]; have := ih hn'; omega
    · rename_i hpx
      simp only [Bool.not_eq_true', Bool.not_eq_false] at hpx
      rw [sameKey_iff] at hpx
      -- the rest has no pair of the key
      have hall : (rest.filter fun q => !sameKey q.1 x) = rest := by
        rw [List.filter_eq_self]
        intro q hq
        simp only [Bool.not_eq_true', sameKey]
        cases h : (q.1 == x)
        · rfl
        · rw [Val.beq_iff] at h
          exact absurd (List.mem_map.mpr ⟨q, hq, h.trans hpx.symm⟩) hp
      rw [hall]; simp

theorem dict_count_mem {key : Key} {qs : List (Val × Val)} {c : Iv} {k v : Abs}
    (hc : c.In ((dictFrom key qs).length : Rat)) (hk : ∀ q ∈ qs, Mem k q.1) (hv : ∀ q ∈ qs, Mem v q.2) :
    Mem (.dict c k v) (.dict (dictFrom key qs)) :=
  Mem.dict hc (fun p hp => hk p (dictFrom_mem hp)) (fun p hp => hv p (dictFrom_mem hp)) (dictFrom_nodup key qs)

/-- The length of a dict built from pairs whose keys are each once: theirs. -/
theorem dictFrom_length_eq {key : Key} {qs : List (Val × Val)} (hn : (qs.map (·.1)).Nodup) :
    (dictFrom key qs).length = qs.length :=
  Nat.le_antisymm (dictFrom_length_le key qs) (dictFrom_length_ge hn (fun _ h => h))

theorem put_sound {key : Key} {a0 a1 a2 : Abs} {ps : List (Val × Val)} {x y : Val} (h0 : Mem a0 (.dict ps))
    (h1 : Mem a1 x) (h2 : Mem a2 y) (hsz : ((dictFrom key (ps ++ [(x, y)])).length : Int) ≤ i64max) :
    Mem (.dict (countIv (atLeastOne (dictParts a0).1.lo) (plusOne (dictParts a0).1.hi))
      ((dictParts a0).2.1.join a1) ((dictParts a0).2.2.join a2)) (.dict (dictFrom key (ps ++ [(x, y)]))) := by
  obtain ⟨hc, hk, hv, hnd⟩ := dictParts_sound h0
  refine dict_count_mem ?_ (fun q hq => ?_) (fun q hq => ?_)
  · have hL1 : 1 ≤ (dictFrom key (ps ++ [(x, y)])).length :=
      List.length_pos_iff.mpr (dictFrom_ne_nil (by simp))
    have hLle := dictFrom_length_le key (ps ++ [(x, y)])
    simp only [List.length_append, List.length_singleton] at hLle
    refine countIv_sound (endLe_atLeastOne hL1 (endLe_count hc.lo ?_)) (fun h hh => Or.inl ?_) hsz
    · rcases hnd with hnd | hz
      · exact Or.inl (dictFrom_length_ge hnd (fun q hq => List.mem_append_left _ hq))
      · exact Or.inr hz
    · have := hc.hi
      cases hhi : (dictParts a0).1.hi with
      | none => simp [plusOne, hhi] at hh
      | some u =>
        simp only [plusOne, hhi, Option.map_some, Option.some.injEq] at hh
        subst hh
        rw [hhi] at this; simp only [EndGe] at this
        have : ((dictFrom key (ps ++ [(x, y)])).length : Rat) ≤ (ps.length : Rat) + 1 := by exact_mod_cast hLle
        grind
  · rcases List.mem_append.mp hq with hq | hq
    · exact mem_join_left (hk q hq)
    · simp at hq; subst hq; exact mem_join_right h1
  · rcases List.mem_append.mp hq with hq | hq
    · exact mem_join_left (hv q hq)
    · simp at hq; subst hq; exact mem_join_right h2

theorem removeD_sound {key : Key} {a0 : Abs} {ps : List (Val × Val)} {x : Val} (h0 : Mem a0 (.dict ps))
    (hsz : ((dictFrom key (ps.filter fun p => !sameKey p.1 x)).length : Int) ≤ i64max) :
    Mem (.dict (countIv ((dictParts a0).1.lo.map (· - 1)) (dictParts a0).1.hi) (dictParts a0).2.1 (dictParts a0).2.2)
      (.dict (dictFrom key (ps.filter fun p => !sameKey p.1 x))) := by
  obtain ⟨hc, hk, hv, hnd⟩ := dictParts_sound h0
  refine dict_count_mem ?_ (fun q hq => hk q (List.mem_filter.mp hq).1) (fun q hq => hv q (List.mem_filter.mp hq).1)
  have hLle : (dictFrom key (ps.filter fun p => !sameKey p.1 x)).length ≤ ps.length :=
    Nat.le_trans (dictFrom_length_le _ _) (List.length_filter_le _ _)
  refine countIv_sound ?_ (fun h hh => Or.inl ?_) hsz
  · have hl := hc.lo
    cases hlo : (dictParts a0).1.lo with
    | none => trivial
    | some l =>
      rw [hlo] at hl
      simp only [Option.map_some, EndLe] at hl ⊢
      rcases hnd with hnd | hz
      · have hf := length_filter_key (x := x) hnd
        have heq := dictFrom_length_eq (key := key) (qs := ps.filter fun p => !sameKey p.1 x)
          (hnd.sublist (List.Sublist.map _ (List.filter_sublist (p := fun (p : Val × Val) => !sameKey p.1 x))))
        rw [heq]
        have : (ps.length : Rat) ≤ ((ps.filter fun p => !sameKey p.1 x).length : Rat) + 1 := by exact_mod_cast hf
        grind
      · rw [hlo] at hz; cases hz
        have : (0 : Rat) ≤ ((dictFrom key (ps.filter fun p => !sameKey p.1 x)).length : Rat) := by
          exact_mod_cast Nat.zero_le _
        grind
  · have := hc.hi; rw [hh] at this; simp only [EndGe] at this
    exact Rat.le_trans (by exact_mod_cast hLle) this

/-- Two dicts put together: at least as many pairs as the larger has (each holding each key once),
    at most as many as both. -/
theorem mergedC_sound {key : Key} {ca cb : Iv} {ps qs : List (Val × Val)}
    (hc : ca.In (ps.length : Rat)) (hnd : (ps.map (·.1)).Nodup ∨ ca.lo = some 0)
    (hc' : cb.In (qs.length : Rat)) (hnd' : (qs.map (·.1)).Nodup ∨ cb.lo = some 0)
    (hsz : ((dictFrom key (ps ++ qs)).length : Int) ≤ i64max) :
    (mergedC ca cb).In ((dictFrom key (ps ++ qs)).length : Rat) := by
  have hLle := dictFrom_length_le key (ps ++ qs)
  simp only [List.length_append] at hLle
  have hge0 : ps.length ≤ (dictFrom key (ps ++ qs)).length ∨ ca.lo = some 0 := by
    rcases hnd with hnd | hz
    · exact Or.inl (dictFrom_length_ge hnd (fun q hq => List.mem_append_left _ hq))
    · exact Or.inr hz
  have hge1 : qs.length ≤ (dictFrom key (ps ++ qs)).length ∨ cb.lo = some 0 := by
    rcases hnd' with hnd' | hz
    · exact Or.inl (dictFrom_length_ge hnd' (fun q hq => List.mem_append_right _ hq))
    · exact Or.inr hz
  have l0 := endLe_count hc.lo hge0
  have l1 := endLe_count hc'.lo hge1
  unfold mergedC
  refine countIv_sound ?_ (fun h hh => Or.inl ?_) hsz
  · split
    · rename_i x y hx hy
      rw [hx] at l0; rw [hy] at l1
      simp only [EndLe] at l0 l1 ⊢
      split <;> assumption
    · rename_i hxy
      cases hx : ca.lo with
      | some x => rw [hx] at l0; simp only [Option.some_or]; exact l0
      | none => simp only [Option.none_or]; rw [hx] at l0; exact l1
  · cases hx : ca.hi with
    | none => simp [hx] at hh
    | some u =>
      cases hy : cb.hi with
      | none => simp [hx, hy] at hh
      | some w =>
        simp only [hx, hy, Option.some.injEq] at hh
        subst hh
        have e0 := hc.hi; rw [hx] at e0
        have e1 := hc'.hi; rw [hy] at e1
        simp only [EndGe] at e0 e1
        have : ((dictFrom key (ps ++ qs)).length : Rat) ≤ (ps.length : Rat) + (qs.length : Rat) := by exact_mod_cast hLle
        grind

theorem merge_sound {key : Key} {a0 a1 : Abs} {ps qs : List (Val × Val)} (h0 : Mem a0 (.dict ps)) (h1 : Mem a1 (.dict qs))
    (hsz : ((dictFrom key (ps ++ qs)).length : Int) ≤ i64max) :
    Mem (.dict (mergedC (dictParts a0).1 (dictParts a1).1) ((dictParts a0).2.1.join (dictParts a1).2.1)
      ((dictParts a0).2.2.join (dictParts a1).2.2)) (.dict (dictFrom key (ps ++ qs))) := by
  obtain ⟨hc, hk, hv, hnd⟩ := dictParts_sound h0
  obtain ⟨hc', hk', hv', hnd'⟩ := dictParts_sound h1
  refine dict_count_mem (mergedC_sound hc hnd hc' hnd' hsz) (fun q hq => ?_) (fun q hq => ?_)
  · rcases List.mem_append.mp hq with hq | hq
    · exact mem_join_left (hk q hq)
    · exact mem_join_right (hk' q hq)
  · rcases List.mem_append.mp hq with hq | hq
    · exact mem_join_left (hv q hq)
    · exact mem_join_right (hv' q hq)

-- ── A set ────────────────────────────────────────────────────────────────────

theorem set_count_mem {key : Key} {xs : List Val} {c : Iv} {e : Abs}
    (hc : c.In ((setFrom key xs).length : Rat)) (he : ∀ x ∈ xs, Mem e x) : Mem (.members c e) (.set (setFrom key xs)) :=
  Mem.members hc (fun y hy => he y (setFrom_mem.mp hy)) (setFrom_nodup key xs)

theorem setFrom_length_eq {key : Key} {xs : List Val} (hn : xs.Nodup) : (setFrom key xs).length = xs.length :=
  Nat.le_antisymm (setFrom_length_le key xs) (setFrom_length_ge hn (fun _ h => h))

theorem length_filter_split {α : Type} (p : α → Bool) : ∀ (xs : List α),
    xs.length = (xs.filter p).length + (xs.filter fun x => !p x).length
  | [] => rfl
  | x :: rest => by
    simp only [List.filter_cons, List.length_cons]
    have := length_filter_split p rest
    cases hp : p x <;> simp <;> omega

/-- A member taken out of a set that holds each once: one fewer at most. -/
theorem length_filter_member {ms : List Val} {x : Val} (hn : ms.Nodup) :
    ms.length ≤ (ms.filter fun y => !sameKey y x).length + 1 := by
  have hs := length_filter_split (fun y => !sameKey y x) ms
  have hle : (ms.filter fun y => !!sameKey y x).length ≤ 1 := by
    have hsub : ∀ y ∈ ms.filter (fun y => !!sameKey y x), y ∈ [x] := by
      intro y hy
      simp only [List.mem_filter, Bool.not_not, sameKey_iff] at hy
      simp [hy.2]
    have := length_le_of_sub (hn.sublist List.filter_sublist) hsub
    simpa using this
  omega

/-- The members of a set in another, each once: no more than the other has. -/
theorem length_inter_le {ms os : List Val} (hn : ms.Nodup) :
    (ms.filter fun y => os.any fun z => sameKey y z).length ≤ os.length := by
  refine length_le_of_sub (hn.sublist List.filter_sublist) (fun y hy => ?_)
  simp only [List.mem_filter, List.any_eq_true] at hy
  obtain ⟨_, z, hz, he⟩ := hy
  rw [sameKey_iff] at he; subst he; exact hz

theorem add_sound {key : Key} {a0 a1 : Abs} {ms : List Val} {x : Val} (h0 : Mem a0 (.set ms)) (h1 : Mem a1 x)
    (hsz : ((setFrom key (ms ++ [x])).length : Int) ≤ i64max) :
    Mem (.members (countIv (atLeastOne (membersParts a0).1.lo) (plusOne (membersParts a0).1.hi)) ((membersParts a0).2.join a1))
      (.set (setFrom key (ms ++ [x]))) := by
  obtain ⟨hc, he, hnd⟩ := membersParts_sound h0
  refine set_count_mem ?_ (fun y hy => ?_)
  · have hL1 : 1 ≤ (setFrom key (ms ++ [x])).length := List.length_pos_iff.mpr (setFrom_ne_nil (by simp))
    have hLle := setFrom_length_le key (ms ++ [x])
    simp only [List.length_append, List.length_singleton] at hLle
    refine countIv_sound (endLe_atLeastOne hL1 (endLe_count hc.lo ?_)) (fun h hh => Or.inl ?_) hsz
    · rcases hnd with hnd | hz
      · exact Or.inl (setFrom_length_ge hnd (fun q hq => List.mem_append_left _ hq))
      · exact Or.inr hz
    · have := hc.hi
      cases hhi : (membersParts a0).1.hi with
      | none => simp [plusOne, hhi] at hh
      | some u =>
        simp only [plusOne, hhi, Option.map_some, Option.some.injEq] at hh
        subst hh
        rw [hhi] at this; simp only [EndGe] at this
        have : ((setFrom key (ms ++ [x])).length : Rat) ≤ (ms.length : Rat) + 1 := by exact_mod_cast hLle
        grind
  · rcases List.mem_append.mp hy with hy | hy
    · exact mem_join_left (he y hy)
    · simp at hy; subst hy; exact mem_join_right h1

theorem removeS_sound {key : Key} {a0 : Abs} {ms : List Val} {x : Val} (h0 : Mem a0 (.set ms))
    (hsz : ((setFrom key (ms.filter fun y => !sameKey y x)).length : Int) ≤ i64max) :
    Mem (.members (countIv ((membersParts a0).1.lo.map (· - 1)) (membersParts a0).1.hi) (membersParts a0).2)
      (.set (setFrom key (ms.filter fun y => !sameKey y x))) := by
  obtain ⟨hc, he, hnd⟩ := membersParts_sound h0
  refine set_count_mem ?_ (fun y hy => he y (List.mem_filter.mp hy).1)
  have hLle : (setFrom key (ms.filter fun y => !sameKey y x)).length ≤ ms.length :=
    Nat.le_trans (setFrom_length_le _ _) (List.length_filter_le _ _)
  refine countIv_sound ?_ (fun h hh => Or.inl ?_) hsz
  · have hl := hc.lo
    cases hlo : (membersParts a0).1.lo with
    | none => trivial
    | some l =>
      rw [hlo] at hl
      simp only [Option.map_some, EndLe] at hl ⊢
      rcases hnd with hnd | hz
      · have hf := length_filter_member (x := x) hnd
        rw [setFrom_length_eq (hn := hnd.sublist List.filter_sublist)]
        have : (ms.length : Rat) ≤ ((ms.filter fun y => !sameKey y x).length : Rat) + 1 := by exact_mod_cast hf
        grind
      · rw [hlo] at hz; cases hz
        have : (0 : Rat) ≤ ((setFrom key (ms.filter fun y => !sameKey y x)).length : Rat) := by
          exact_mod_cast Nat.zero_le _
        grind
  · have := hc.hi; rw [hh] at this; simp only [EndGe] at this
    exact Rat.le_trans (by exact_mod_cast hLle) this

theorem union_sound {key : Key} {a0 a1 : Abs} {ms os : List Val} (h0 : Mem a0 (.set ms)) (h1 : Mem a1 (.set os))
    (hsz : ((setFrom key (ms ++ os)).length : Int) ≤ i64max) :
    Mem (.members (mergedC (membersParts a0).1 (membersParts a1).1) ((membersParts a0).2.join (membersParts a1).2))
      (.set (setFrom key (ms ++ os))) := by
  obtain ⟨hc, he, hnd⟩ := membersParts_sound h0
  obtain ⟨hc', he', hnd'⟩ := membersParts_sound h1
  refine set_count_mem ?_ (fun y hy => ?_)
  · have hLle := setFrom_length_le key (ms ++ os)
    simp only [List.length_append] at hLle
    have hge0 : ms.length ≤ (setFrom key (ms ++ os)).length ∨ (membersParts a0).1.lo = some 0 := by
      rcases hnd with hnd | hz
      · exact Or.inl (setFrom_length_ge hnd (fun q hq => List.mem_append_left _ hq))
      · exact Or.inr hz
    have hge1 : os.length ≤ (setFrom key (ms ++ os)).length ∨ (membersParts a1).1.lo = some 0 := by
      rcases hnd' with hnd' | hz
      · exact Or.inl (setFrom_length_ge hnd' (fun q hq => List.mem_append_right _ hq))
      · exact Or.inr hz
    have l0 := endLe_count hc.lo hge0
    have l1 := endLe_count hc'.lo hge1
    unfold mergedC
    refine countIv_sound ?_ (fun h hh => Or.inl ?_) hsz
    · split
      · rename_i x y hx hy
        rw [hx] at l0; rw [hy] at l1
        simp only [EndLe] at l0 l1 ⊢
        split <;> assumption
      · rename_i hxy
        cases hx : (membersParts a0).1.lo with
        | some x => rw [hx] at l0; simp only [Option.some_or]; exact l0
        | none => simp only [Option.none_or]; rw [hx] at l0; exact l1
    · cases hx : (membersParts a0).1.hi with
      | none => simp [hx] at hh
      | some u =>
        cases hy : (membersParts a1).1.hi with
        | none => simp [hx, hy] at hh
        | some w =>
          simp only [hx, hy, Option.some.injEq] at hh
          subst hh
          have e0 := hc.hi; rw [hx] at e0
          have e1 := hc'.hi; rw [hy] at e1
          simp only [EndGe] at e0 e1
          have : ((setFrom key (ms ++ os)).length : Rat) ≤ (ms.length : Rat) + (os.length : Rat) := by exact_mod_cast hLle
          grind
  · rcases List.mem_append.mp hy with hy | hy
    · exact mem_join_left (he y hy)
    · exact mem_join_right (he' y hy)

theorem intersect_sound {key : Key} {a0 a1 : Abs} {ms os : List Val} (h0 : Mem a0 (.set ms)) (h1 : Mem a1 (.set os))
    (hsz : ((setFrom key (ms.filter fun y => os.any fun z => sameKey y z)).length : Int) ≤ i64max) :
    Mem (.members (countIv (some 0) (minHi (membersParts a0).1.hi (membersParts a1).1.hi)) (membersParts a0).2)
      (.set (setFrom key (ms.filter fun y => os.any fun z => sameKey y z))) := by
  obtain ⟨hc, he, _⟩ := membersParts_sound h0
  obtain ⟨hc', _, _⟩ := membersParts_sound h1
  refine set_count_mem ?_ (fun y hy => he y (List.mem_filter.mp hy).1)
  have hL0 : (setFrom key (ms.filter fun y => os.any fun z => sameKey y z)).length ≤ ms.length :=
    Nat.le_trans (setFrom_length_le _ _) (List.length_filter_le _ _)
  -- the members of the answer are among the other's, each once
  have hL1 : (setFrom key (ms.filter fun y => os.any fun z => sameKey y z)).length ≤ os.length := by
    refine length_le_of_sub (setFrom_nodup _ _) (fun y hy => ?_)
    have := List.mem_filter.mp (setFrom_mem.mp hy)
    obtain ⟨z, hz, he⟩ := List.any_eq_true.mp this.2
    rw [sameKey_iff] at he; subst he; exact hz
  refine countIv_sound endLe_zero (fun h hh => Or.inl ?_) hsz
  have e0 := hc.hi
  have e1 := hc'.hi
  cases hx : (membersParts a0).1.hi with
  | none =>
    rw [hx] at hh
    cases hy : (membersParts a1).1.hi with
    | none => rw [hy] at hh; simp [minHi] at hh
    | some w =>
      rw [hy] at hh; simp only [minHi, Option.some.injEq] at hh; subst hh
      rw [hy] at e1; simp only [EndGe] at e1
      exact Rat.le_trans (by exact_mod_cast hL1) e1
  | some u =>
    cases hy : (membersParts a1).1.hi with
    | none =>
      rw [hx, hy] at hh; simp only [minHi, Option.some.injEq] at hh; subst hh
      rw [hx] at e0; simp only [EndGe] at e0
      exact Rat.le_trans (by exact_mod_cast hL0) e0
    | some w =>
      rw [hx, hy] at hh; simp only [minHi, Option.some.injEq] at hh; subst hh
      rw [hx] at e0; rw [hy] at e1; simp only [EndGe] at e0 e1
      split
      · exact Rat.le_trans (by exact_mod_cast hL0) e0
      · exact Rat.le_trans (by exact_mod_cast hL1) e1

theorem difference_sound {key : Key} {a0 a1 : Abs} {ms os : List Val} (h0 : Mem a0 (.set ms)) (h1 : Mem a1 (.set os))
    (hsz : ((setFrom key (ms.filter fun y => !os.any fun z => sameKey y z)).length : Int) ≤ i64max) :
    Mem (.members (countIv (match (membersParts a0).1.lo, (membersParts a1).1.hi with
        | some l, some h => some (l - h)
        | _, _ => some 0) (membersParts a0).1.hi) (membersParts a0).2)
      (.set (setFrom key (ms.filter fun y => !os.any fun z => sameKey y z))) := by
  obtain ⟨hc, he, hnd⟩ := membersParts_sound h0
  obtain ⟨hc', _, _⟩ := membersParts_sound h1
  refine set_count_mem ?_ (fun y hy => he y (List.mem_filter.mp hy).1)
  have hL0 : (setFrom key (ms.filter fun y => !os.any fun z => sameKey y z)).length ≤ ms.length :=
    Nat.le_trans (setFrom_length_le _ _) (List.length_filter_le _ _)
  refine countIv_sound ?_ (fun h hh => Or.inl ?_) hsz
  · split
    · rename_i l h hl hh
      simp only [EndLe]
      rcases hnd with hnd | hz
      · -- the members taken out are among the other's, each once
        have hs := length_filter_split (fun y => os.any fun z => sameKey y z) ms
        have hi := length_inter_le (os := os) hnd
        rw [setFrom_length_eq (hn := hnd.sublist List.filter_sublist)]
        have e0 := hc.lo; rw [hl] at e0
        have e1 := hc'.hi; rw [hh] at e1
        simp only [EndLe, EndGe] at e0 e1
        have : (ms.length : Rat) ≤ ((ms.filter fun y => !os.any fun z => sameKey y z).length : Rat) + (os.length : Rat) := by
          exact_mod_cast (show ms.length ≤ _ + os.length by omega)
        grind
      · rw [hl] at hz; cases hz
        have e1 := hc'.hi; rw [hh] at e1; simp only [EndGe] at e1
        have : (0 : Rat) ≤ (os.length : Rat) := by exact_mod_cast Nat.zero_le _
        have : (0 : Rat) ≤ ((setFrom key (ms.filter fun y => !os.any fun z => sameKey y z)).length : Rat) := by
          exact_mod_cast Nat.zero_le _
        grind
    · exact endLe_zero
  · have := hc.hi; rw [hh] at this; simp only [EndGe] at this
    exact Rat.le_trans (by exact_mod_cast hL0) this

-- ── A list ───────────────────────────────────────────────────────────────────

theorem list_count_mem {c : Iv} {e : Abs} {xs : List Val} (hc : c.In (xs.length : Rat)) (he : ∀ x ∈ xs, Mem e x) :
    Mem (.list c e) (.list xs) := Mem.list hc he

theorem lo_le_zero {c : Iv} (hc : c.In ((0 : Nat) : Rat)) : ((c.lo.map (·.num)).getD 0 ≤ 0) := by
  have := hc.lo
  cases hlo : c.lo with
  | none => simp
  | some l =>
    rw [hlo] at this; simp only [EndLe] at this
    simp only [Option.map_some, Option.getD_some]
    have hl : l ≤ 0 := by simpa using this
    by_cases h0 : l < 0
    · exact Int.le_of_lt ((num_neg_iff l).mpr h0)
    · have : l = 0 := by grind
      subst this; decide

theorem first_sound {a0 : Abs} {xs : List Val} (h0 : Mem a0 (.list xs)) :
    Mem (optOf (decide (((listParts a0).1.lo.map (·.num)).getD 0 ≤ 0)) (listParts a0).2) (xs.head?.getD .none) := by
  obtain ⟨hc, he⟩ := listParts_sound h0
  cases xs with
  | nil => exact Mem_optOf_none (by simpa using lo_le_zero hc)
  | cons x rest => exact Mem_optOf (he x List.mem_cons_self)

theorem last_sound {a0 : Abs} {xs : List Val} (h0 : Mem a0 (.list xs)) :
    Mem (optOf (decide (((listParts a0).1.lo.map (·.num)).getD 0 ≤ 0)) (listParts a0).2) (xs.getLast?.getD .none) := by
  obtain ⟨hc, he⟩ := listParts_sound h0
  rcases hl : xs.getLast? with _ | x
  · rw [List.getLast?_eq_none_iff] at hl; subst hl
    exact Mem_optOf_none (by simpa using lo_le_zero hc)
  · exact Mem_optOf (he x (List.mem_of_mem_getLast? hl))

theorem at_sound {a0 : Abs} {xs : List Val} {j : Int} (h0 : Mem a0 (.list xs)) :
    Mem (optOf true (listParts a0).2) (if 0 ≤ j ∧ j < (xs.length : Int) then (xs[j.toNat]?).getD .none else .none) := by
  obtain ⟨_, he⟩ := listParts_sound h0
  split
  · rcases hx : xs[j.toNat]? with _ | x
    · exact Mem_optOf_none rfl
    · exact Mem_optOf (he x (List.mem_of_getElem? hx))
  · exact Mem_optOf_none rfl

theorem append_sound {a0 a1 : Abs} {xs : List Val} {x : Val} (h0 : Mem a0 (.list xs)) (h1 : Mem a1 x)
    (hsz : ((xs ++ [x]).length : Int) ≤ i64max) :
    Mem (.list (countIv (plusOne (listParts a0).1.lo) (plusOne (listParts a0).1.hi)) ((listParts a0).2.join a1))
      (.list (xs ++ [x])) := by
  obtain ⟨hc, he⟩ := listParts_sound h0
  refine Mem.list ?_ (fun y hy => ?_)
  · refine countIv_sound ?_ (fun h hh => Or.inl ?_) hsz
    · have := hc.lo
      cases hlo : (listParts a0).1.lo with
      | none => simp [plusOne, EndLe]
      | some l =>
        rw [hlo] at this; simp only [EndLe] at this
        simp only [plusOne, Option.map_some, EndLe, List.length_append, List.length_singleton]
        push_cast; grind
    · have := hc.hi
      cases hhi : (listParts a0).1.hi with
      | none => simp [plusOne, hhi] at hh
      | some u =>
        simp only [plusOne, hhi, Option.map_some, Option.some.injEq] at hh
        subst hh
        rw [hhi] at this; simp only [EndGe] at this
        simp only [List.length_append, List.length_singleton]; push_cast; grind
  · rcases List.mem_append.mp hy with hy | hy
    · exact mem_join_left (he y hy)
    · simp at hy; subst hy; exact mem_join_right h1

theorem nonNeg_eq (v : Rat) : nonNeg v = if v < 0 then 0 else v := by
  unfold nonNeg
  by_cases h : v < 0
  · simp [h, (num_neg_iff v).mpr h]
  · have : ¬ v.num < 0 := fun h' => h ((num_neg_iff v).mp h')
    simp [h, this]

theorem clampTo_cast (n : Nat) (j : Int) :
    ((clampTo n j : Nat) : Rat) = if j < 0 then 0 else if (n : Int) ≤ j then (n : Rat) else (j : Rat) := by
  unfold clampTo
  by_cases h : j < 0
  · simp [h]
  · simp only [h, ite_false]
    by_cases hn : (n : Int) ≤ j
    · simp only [hn, ite_true]
      rw [Nat.min_eq_right (by omega)]
    · simp only [hn, ite_false]
      rw [Nat.min_eq_left (by omega)]
      have : ((j.toNat : Nat) : Int) = j := Int.toNat_of_nonneg (by omega)
      exact_mod_cast this

theorem natSub_cast {a b : Nat} (h : a ≤ b) : (((b - a : Nat)) : Rat) = (b : Rat) - (a : Rat) := by
  have e : ((b - a : Nat) : Int) = (b : Int) - (a : Int) := by omega
  have e2 : (((b - a : Nat) : Int) : Rat) = ((((b : Int) - (a : Int)) : Int) : Rat) := by rw [e]
  rw [Rat.intCast_sub] at e2
  have l : ((((b - a : Nat) : Int)) : Rat) = (((b - a : Nat)) : Rat) := Rat.intCast_natCast _
  have lb : (((b : Int)) : Rat) = ((b : Nat) : Rat) := Rat.intCast_natCast _
  have la : (((a : Int)) : Rat) = ((a : Nat) : Rat) := Rat.intCast_natCast _
  rw [l, lb, la] at e2
  exact e2

theorem clampTo_le (n : Nat) (j : Int) : clampTo n j ≤ n := by
  unfold clampTo; split <;> omega

theorem cast_lt_zero {j : Int} : ((j : Rat) < 0) ↔ j < 0 := by
  constructor <;> intro h <;> exact_mod_cast h

theorem cast_le_cast {n : Nat} {j : Int} : ((n : Rat) ≤ (j : Rat)) ↔ (n : Int) ≤ j := by
  constructor <;> intro h <;> exact_mod_cast h

theorem take_sound {a0 a1 : Abs} {xs : List Val} {r : Rat} (h0 : Mem a0 (.list xs)) (h1 : Mem a1 (.num r))
    (hr : r.den = 1) (hsz : ((xs.take (clampTo xs.length r.num)).length : Int) ≤ i64max) :
    Mem (.list (countIv (minLo (listParts a0).1.lo (((a1.iv.getD Iv.top).lo.map nonNeg).or (some 0)))
        (minHi (listParts a0).1.hi ((a1.iv.getD Iv.top).hi.map nonNeg))) (listParts a0).2)
      (.list (xs.take (clampTo xs.length r.num))) := by
  obtain ⟨hc, he⟩ := listParts_sound h0
  have hn := In_ivD h1
  have hrq := rat_of_den_one hr
  refine Mem.list ?_ (fun y hy => he y (List.mem_of_mem_take hy))
  have hlen : (xs.take (clampTo xs.length r.num)).length = clampTo xs.length r.num := by
    rw [List.length_take]; exact Nat.min_eq_left (clampTo_le _ _)
  rw [hlen] at hsz ⊢
  have hcast := clampTo_cast xs.length r.num
  have h0' : (0 : Rat) ≤ (xs.length : Rat) := by exact_mod_cast Nat.zero_le _
  have hj := @cast_lt_zero r.num
  have hnj := @cast_le_cast xs.length r.num
  rw [← hrq] at hj hnj
  refine countIv_sound ?_ (fun h hh => Or.inl ?_) hsz
  · -- the lower end: the fewer of the list's and the number's
    cases hcl : (listParts a0).1.lo with
    | none => simp [minLo, EndLe]
    | some l =>
      have el := hc.lo; rw [hcl] at el; simp only [EndLe] at el
      cases hnl : (a1.iv.getD Iv.top).lo with
      | none =>
        simp only [minLo, Option.map_none, Option.none_or, EndLe]
        rw [hcast]
        split <;> split <;> (try split) <;> grind
      | some nl =>
        have en := hn.lo; rw [hnl] at en; simp only [EndLe] at en
        simp only [minLo, Option.map_some, Option.some_or, EndLe, nonNeg_eq]
        rw [hcast]
        split <;> split <;> (try split) <;> (try split) <;> grind
  · -- the upper end: the fewer of the list's and the number's
    have eh := hc.hi
    have en := hn.hi
    rw [hcast]
    cases hch : (listParts a0).1.hi with
    | none =>
      rw [hch] at hh
      cases hnh : (a1.iv.getD Iv.top).hi with
      | none => rw [hnh] at hh; simp [minHi] at hh
      | some u =>
        rw [hnh] at hh en; simp only [minHi, Option.map_some, Option.some.injEq, nonNeg_eq] at hh
        simp only [EndGe] at en
        subst hh
        split <;> (try split) <;> (try split) <;> grind
    | some w =>
      rw [hch] at hh eh; simp only [EndGe] at eh
      cases hnh : (a1.iv.getD Iv.top).hi with
      | none =>
        rw [hnh] at hh; simp only [minHi, Option.map_none, Option.some.injEq] at hh
        subst hh
        split <;> (try split) <;> grind
      | some u =>
        rw [hnh] at hh en; simp only [minHi, Option.map_some, Option.some.injEq, nonNeg_eq] at hh
        simp only [EndGe] at en
        subst hh
        split <;> (try split) <;> (try split) <;> (try split) <;> grind

theorem drop_sound {a0 a1 : Abs} {xs : List Val} {r : Rat} (h0 : Mem a0 (.list xs)) (h1 : Mem a1 (.num r))
    (hr : r.den = 1) (hsz : ((xs.drop (clampTo xs.length r.num)).length : Int) ≤ i64max) :
    Mem (.list (countIv (match (listParts a0).1.lo, (a1.iv.getD Iv.top).hi.map nonNeg with
          | some l, some h => some (l - h)
          | _, _ => some 0)
        ((listParts a0).1.hi.map (· - ((a1.iv.getD Iv.top).lo.map nonNeg).getD 0))) (listParts a0).2)
      (.list (xs.drop (clampTo xs.length r.num))) := by
  obtain ⟨hc, he⟩ := listParts_sound h0
  have hn := In_ivD h1
  have hrq := rat_of_den_one hr
  refine Mem.list ?_ (fun y hy => he y (List.mem_of_mem_drop hy))
  have hlen : ((xs.drop (clampTo xs.length r.num)).length : Rat) = (xs.length : Rat) - ((clampTo xs.length r.num : Nat) : Rat) := by
    rw [List.length_drop]; exact natSub_cast (clampTo_le _ _)
  have hcast := clampTo_cast xs.length r.num
  have h0' : (0 : Rat) ≤ (xs.length : Rat) := by exact_mod_cast Nat.zero_le _
  have hj := @cast_lt_zero r.num
  have hnj := @cast_le_cast xs.length r.num
  rw [← hrq] at hj hnj
  refine countIv_sound ?_ (fun h hh => ?_) hsz
  · split
    · rename_i l u hl hu
      simp only [EndLe]
      have el := hc.lo; rw [hl] at el; simp only [EndLe] at el
      cases hnh : (a1.iv.getD Iv.top).hi with
      | none => rw [hnh] at hu; simp at hu
      | some w =>
        rw [hnh] at hu; simp only [Option.map_some, Option.some.injEq, nonNeg_eq] at hu
        have en := hn.hi; rw [hnh] at en; simp only [EndGe] at en
        rw [hlen, hcast, ← hu]
        split <;> (try split) <;> (try split) <;> grind
    · exact endLe_zero
  · -- the upper end: the list's less the fewest taken off
    cases hch : (listParts a0).1.hi with
    | none => rw [hch] at hh; simp at hh
    | some w =>
      have eh := hc.hi; rw [hch] at eh; simp only [EndGe] at eh
      rw [hch] at hh; simp only [Option.map_some, Option.some.injEq] at hh
      subst hh
      by_cases hz : clampTo xs.length r.num = xs.length
      · right; rw [List.length_drop, hz]; simp
      · left
        rw [hlen, hcast]
        cases hnl : (a1.iv.getD Iv.top).lo with
        | none => simp only [Option.map_none, Option.getD_none]; split <;> (try split) <;> grind
        | some l =>
          have en := hn.lo; rw [hnl] at en; simp only [EndLe] at en
          simp only [Option.map_some, Option.getD_some, nonNeg_eq]
          have hz' : ¬ (((clampTo xs.length r.num : Nat) : Rat) = (xs.length : Rat)) := by
            intro h; exact hz (by exact_mod_cast h)
          rw [hcast] at hz'
          split <;> (try split) <;> (try split) <;> grind

theorem slice_sound {a0 a1 a2 : Abs} {xs : List Val} {r q : Rat} (h0 : Mem a0 (.list xs)) (h1 : Mem a1 (.num r))
    (h2 : Mem a2 (.num q)) (hr : r.den = 1) (hq : q.den = 1)
    (hsz : ((if clampTo xs.length r.num < clampTo xs.length q.num then
      (xs.drop (clampTo xs.length r.num)).take (clampTo xs.length q.num - clampTo xs.length r.num) else []).length : Int) ≤ i64max) :
    Mem (.list (countIv (some 0) (minHi (listParts a0).1.hi ((a2.iv.getD Iv.top).hi.map fun h =>
        nonNeg h - (((a1.iv.getD Iv.top).lo.map nonNeg).getD 0)))) (listParts a0).2)
      (.list (if clampTo xs.length r.num < clampTo xs.length q.num then
        (xs.drop (clampTo xs.length r.num)).take (clampTo xs.length q.num - clampTo xs.length r.num) else [])) := by
  obtain ⟨hc, he⟩ := listParts_sound h0
  have hna := In_ivD h1
  have hnb := In_ivD h2
  have hrq := rat_of_den_one hr
  have hqq := rat_of_den_one hq
  refine Mem.list ?_ (fun y hy => ?_)
  · refine countIv_sound endLe_zero (fun h hh => ?_) hsz
    split
    · rename_i hij
      have hi := clampTo_le xs.length r.num
      have hj := clampTo_le xs.length q.num
      have hlen : ((xs.drop (clampTo xs.length r.num)).take (clampTo xs.length q.num - clampTo xs.length r.num)).length =
          clampTo xs.length q.num - clampTo xs.length r.num := by
        rw [List.length_take, List.length_drop]; omega
      rw [hlen]
      left
      have hcr := clampTo_cast xs.length r.num
      have hcq := clampTo_cast xs.length q.num
      have hsub : (((clampTo xs.length q.num - clampTo xs.length r.num : Nat)) : Rat) =
          ((clampTo xs.length q.num : Nat) : Rat) - ((clampTo xs.length r.num : Nat) : Rat) := by
        exact natSub_cast (Nat.le_of_lt hij)
      have hij' : ((clampTo xs.length r.num : Nat) : Rat) < ((clampTo xs.length q.num : Nat) : Rat) := by exact_mod_cast hij
      have hjn : ((clampTo xs.length q.num : Nat) : Rat) ≤ (xs.length : Rat) := by exact_mod_cast hj
      rw [hsub]
      have h0' : (0 : Rat) ≤ (xs.length : Rat) := by exact_mod_cast Nat.zero_le _
      have ha1 := @cast_lt_zero r.num
      have ha2 := @cast_le_cast xs.length r.num
      have hb1 := @cast_lt_zero q.num
      have hb2 := @cast_le_cast xs.length q.num
      rw [← hrq] at ha1 ha2 hcr
      rw [← hqq] at hb1 hb2 hcq
      have eh := hc.hi
      cases hch : (listParts a0).1.hi with
      | none =>
        rw [hch] at hh
        cases hbh : (a2.iv.getD Iv.top).hi with
        | none => rw [hbh] at hh; simp [minHi] at hh
        | some u =>
          rw [hbh] at hh; simp only [minHi, Option.map_some, Option.some.injEq] at hh
          have en := hnb.hi; rw [hbh] at en; simp only [EndGe] at en
          subst hh
          cases hal : (a1.iv.getD Iv.top).lo with
          | none =>
            simp only [Option.map_none, Option.getD_none, nonNeg_eq]
            rw [hcr, hcq] at hij' ⊢; rw [hcq] at hjn
            split <;> (try split) <;> (try split) <;> (try split) <;> (try split) <;> grind
          | some l =>
            have el := hna.lo; rw [hal] at el; simp only [EndLe] at el
            simp only [Option.map_some, Option.getD_some, nonNeg_eq]
            rw [hcr, hcq] at hij' ⊢; rw [hcq] at hjn
            split <;> (try split) <;> (try split) <;> (try split) <;> (try split) <;> (try split) <;> grind
      | some w =>
        rw [hch] at hh eh; simp only [EndGe] at eh
        have hle : ((clampTo xs.length q.num : Nat) : Rat) - ((clampTo xs.length r.num : Nat) : Rat) ≤ w := by
          have : (0 : Rat) ≤ ((clampTo xs.length r.num : Nat) : Rat) := by exact_mod_cast Nat.zero_le _
          grind
        cases hbh : (a2.iv.getD Iv.top).hi with
        | none =>
          rw [hbh] at hh; simp only [minHi, Option.map_none, Option.some.injEq] at hh
          subst hh; exact hle
        | some u =>
          rw [hbh] at hh; simp only [minHi, Option.map_some, Option.some.injEq] at hh
          have en := hnb.hi; rw [hbh] at en; simp only [EndGe] at en
          subst hh
          split
          · exact hle
          · cases hal : (a1.iv.getD Iv.top).lo with
            | none =>
              simp only [Option.map_none, Option.getD_none, nonNeg_eq]
              rw [hcr, hcq] at hij' ⊢; rw [hcq] at hjn
              split <;> (try split) <;> (try split) <;> (try split) <;> (try split) <;> grind
            | some l =>
              have el := hna.lo; rw [hal] at el; simp only [EndLe] at el
              simp only [Option.map_some, Option.getD_some, nonNeg_eq]
              rw [hcr, hcq] at hij' ⊢; rw [hcq] at hjn
              split <;> (try split) <;> (try split) <;> (try split) <;> (try split) <;> (try split) <;> grind
    · right; rfl
  · split at hy
    · exact he y (List.mem_of_mem_drop (List.mem_of_mem_take hy))
    · simp at hy

theorem reverse_sound {a0 : Abs} {xs : List Val} (h0 : Mem a0 (.list xs)) :
    Mem (.list (listParts a0).1 (listParts a0).2) (.list xs.reverse) := by
  obtain ⟨hc, he⟩ := listParts_sound h0
  exact Mem.list (by rw [List.length_reverse]; exact hc) (fun y hy => he y (List.mem_reverse.mp hy))

theorem sort_sound {a0 : Abs} {xs : List Val} {key : Key} (h0 : Mem a0 (.list xs)) :
    Mem (.list (listParts a0).1 (listParts a0).2) (.list (xs.mergeSort (byKey key))) := by
  obtain ⟨hc, he⟩ := listParts_sound h0
  have hp := List.mergeSort_perm xs (byKey key)
  exact Mem.list (by rw [hp.length_eq]; exact hc) (fun y hy => he y (hp.mem_iff.mp hy))

/-- A rational whose numerator is at most 1 is at most 1. -/
theorem rat_le_one {l : Rat} (h : l.num ≤ 1) : l ≤ 1 := by
  have hd0 : (0 : Rat) < (l.den : Rat) := by exact_mod_cast l.den_pos
  have hd1 : (1 : Rat) ≤ (l.den : Rat) := by exact_mod_cast l.den_pos
  have hmul : ((l.num : Int) : Rat) = l * (l.den : Rat) := by
    have : (l.num : Rat) / l.den * l.den = l.num := Rat.div_mul_cancel (den_ne_zero l)
    rw [← rat_eq_num_div_den l] at this
    exact this.symm
  have hn : (l.num : Rat) ≤ 1 := by exact_mod_cast h
  by_cases hl : l ≤ 1
  · exact hl
  · have hgt : 1 < l := by grind
    have : (1 : Rat) * (l.den : Rat) < l * (l.den : Rat) := Rat.mul_lt_mul_of_pos_right hgt hd0
    grind

/-- **From one to the count**: what a list of `n` makes, from one where it has one to `n`. -/
theorem oneUp_sound {c : Iv} {n L : Nat} (hc : c.In (n : Rat)) (hL : L ≤ n) (h1 : 0 < n → 0 < L)
    (hm : (L : Int) ≤ i64max) : (oneUp c).In (L : Rat) := by
  unfold oneUp
  refine countIv_sound ?_ (fun h hh => Or.inl ?_) hm
  · have el := hc.lo
    cases hl : c.lo with
    | none => trivial
    | some l =>
      rw [hl] at el; simp only [EndLe] at el
      simp only [Option.map_some, EndLe]
      split
      · rename_i hgt
        have : (0 : Rat) < l := by
          have : 0 ≤ l.num := by omega
          have h0 := (@Rat.num_nonneg l).mp this
          have hne : l ≠ 0 := by intro h; subst h; simp at hgt
          grind
        have hn0 : 0 < n := by
          have : (0 : Rat) < (n : Rat) := rlt_of_lt_of_le this el
          exact_mod_cast this
        have := h1 hn0
        exact_mod_cast this
      · rename_i hle
        by_cases hn0 : n = 0
        · subst hn0
          have : L = 0 := by omega
          subst this; simpa using el
        · have hL1 : 1 ≤ L := h1 (by omega)
          have hl1 : l ≤ 1 := rat_le_one (by omega)
          have : (1 : Rat) ≤ (L : Rat) := by exact_mod_cast hL1
          grind
  · have := hc.hi; rw [hh] at this; simp only [EndGe] at this
    exact Rat.le_trans (by exact_mod_cast hL) this

theorem distinct_sound {a0 : Abs} {xs : List Val} (h0 : Mem a0 (.list xs)) (hsz : ((firstOfEach xs).length : Int) ≤ i64max) :
    Mem (.list (oneUp (listParts a0).1) (listParts a0).2) (.list (firstOfEach xs)) := by
  obtain ⟨hc, he⟩ := listParts_sound h0
  refine Mem.list (oneUp_sound hc (firstOfEach_length_le xs) (fun hn => ?_) hsz) (fun y hy => he y (firstOfEach_mem.mp hy))
  exact List.length_pos_iff.mpr (firstOfEach_ne_nil (List.ne_nil_of_length_pos hn))

theorem toSet_sound {a0 : Abs} {xs : List Val} {key : Key} (h0 : Mem a0 (.list xs))
    (hsz : ((setFrom key xs).length : Int) ≤ i64max) :
    Mem (.members (oneUp (listParts a0).1) (listParts a0).2) (.set (setFrom key xs)) := by
  obtain ⟨hc, he⟩ := listParts_sound h0
  refine set_count_mem (oneUp_sound hc (setFrom_length_le key xs) (fun hn => ?_) hsz) he
  exact List.length_pos_iff.mpr (setFrom_ne_nil (List.ne_nil_of_length_pos hn))

theorem indexed_sound {a0 : Abs} {xs : List Val} (h0 : Mem a0 (.list xs)) :
    Mem (.list (listParts a0).1 (.record [("index", .num (let index := Iv.mk' (some 0) ((listParts a0).1.hi.map (· - 1)) (some 1);
        if index.hi.any (·.num < 0) then Iv.ints 0 0 else index)), ("item", (listParts a0).2)] []))
      (.list (xs.zipIdx.map fun (x, i) => .record [("index", .num (i : Rat)), ("item", x)])) := by
  obtain ⟨hc, he⟩ := listParts_sound h0
  refine Mem.list (by rw [List.length_map, List.length_zipIdx]; exact hc) (fun y hy => ?_)
  obtain ⟨⟨x, i⟩, hxi, rfl⟩ := List.mem_map.mp hy
  obtain ⟨_, hi, hx⟩ := List.mem_zipIdx hxi
  simp only [Nat.zero_add, Nat.sub_zero] at hi hx
  refine Mem_record2 (Mem.num ?_) (by rw [hx]; exact he _ (List.getElem_mem _))
  have eh := hc.hi
  dsimp only
  split
  · rename_i hneg
    exfalso
    cases hh : (listParts a0).1.hi with
    | none => simp [hh, Iv.mk', normHi] at hneg
    | some u =>
      rw [hh] at eh; simp only [EndGe] at eh
      rw [hh] at hneg
      have hi' : ((i : Nat) : Rat) + 1 ≤ (xs.length : Rat) := by exact_mod_cast hi
      have hi0 : (0 : Rat) ≤ ((i : Nat) : Rat) := by exact_mod_cast Nat.zero_le i
      have hu : (0 : Rat) ≤ u - 1 := by grind
      have hn := normHi_ge (hi := some (u - 1)) (v := 0) (by simpa [EndGe] using hu)
      simp only [Iv.mk', Option.map_some] at hneg
      cases hw : normHi (some (u - 1)) with
      | none => rw [hw] at hneg; simp at hneg
      | some w =>
        rw [hw] at hneg hn
        simp only [Option.any_some, decide_eq_true_eq] at hneg
        simp only [EndGe] at hn
        have := (num_neg_iff w).mp hneg
        grind
  · refine In_mk' (by simp only [EndLe]; exact_mod_cast Nat.zero_le i) ?_ (by simp [DenLe])
    cases hh : (listParts a0).1.hi with
    | none => trivial
    | some u =>
      rw [hh] at eh; simp only [EndGe] at eh
      simp only [Option.map_some, EndGe]
      have hi' : ((i : Nat) : Rat) + 1 ≤ (xs.length : Rat) := by exact_mod_cast hi
      grind

-- ── Lists of lists ───────────────────────────────────────────────────────────

theorem mapM_listOf : ∀ {xs : List Val} {yss : List (List Val)}, xs.mapM listOf = some yss → xs = yss.map .list
  | [], yss, h => by simp at h; subst h; rfl
  | x :: rest, yss, h => by
    simp only [List.mapM_cons, Option.bind_eq_bind] at h
    rcases hx : listOf x with _ | ys
    · rw [hx] at h; simp at h
    · rw [hx] at h
      simp only [Option.bind_some] at h
      rcases hr : rest.mapM listOf with _ | yss'
      · rw [hr] at h; simp at h
      · rw [hr] at h
        simp only [Option.bind_some, Option.pure_def, Option.some.injEq] at h
        subst h
        cases x <;> simp [listOf] at hx
        subst hx
        simp [mapM_listOf hr]

theorem flatten_len_ge {y : Rat} : ∀ (yss : List (List Val)), (∀ ys ∈ yss, y ≤ (ys.length : Rat)) →
    (yss.length : Rat) * y ≤ (yss.flatten.length : Rat)
  | [], _ => by simp
  | ys :: rest, h => by
    have ih := flatten_len_ge rest (fun z hz => h z (List.mem_cons_of_mem _ hz))
    have h1 := h ys List.mem_cons_self
    simp only [List.length_cons, List.flatten_cons, List.length_append]
    push_cast
    rw [Rat.add_mul, Rat.one_mul]
    grind

theorem flatten_len_le {w : Rat} : ∀ (yss : List (List Val)), (∀ ys ∈ yss, (ys.length : Rat) ≤ w) →
    (yss.flatten.length : Rat) ≤ (yss.length : Rat) * w
  | [], _ => by simp
  | ys :: rest, h => by
    have ih := flatten_len_le rest (fun z hz => h z (List.mem_cons_of_mem _ hz))
    have h1 := h ys List.mem_cons_self
    simp only [List.length_cons, List.flatten_cons, List.length_append]
    push_cast
    rw [Rat.add_mul, Rat.one_mul]
    grind

/-- **The lists of a list one after another**: a count times a count. -/
theorem timesCount_sound {c ci : Iv} {yss : List (List Val)} (hc : c.In (yss.length : Rat))
    (hci : ∀ ys ∈ yss, ci.In (ys.length : Rat)) (hm : (yss.flatten.length : Int) ≤ i64max) :
    (timesCount c ci).In (yss.flatten.length : Rat) := by
  unfold timesCount
  refine countIv_sound ?_ (fun h hh => ?_) hm
  · split
    · rename_i x y hx hy
      simp only [EndLe]
      have ex := hc.lo; rw [hx] at ex; simp only [EndLe] at ex
      have hn0 : (0 : Rat) ≤ (yss.length : Rat) := by exact_mod_cast Nat.zero_le _
      have hx' : nonNeg x ≤ (yss.length : Rat) := by rw [nonNeg_eq]; split <;> grind
      have hy0 : 0 ≤ nonNeg y := by rw [nonNeg_eq]; split <;> grind
      have hy' : ∀ ys ∈ yss, nonNeg y ≤ (ys.length : Rat) := by
        intro ys hys
        have ey := (hci ys hys).lo; rw [hy] at ey; simp only [EndLe] at ey
        have : (0 : Rat) ≤ (ys.length : Rat) := by exact_mod_cast Nat.zero_le _
        rw [nonNeg_eq]; split <;> grind
      have h1 : nonNeg x * nonNeg y ≤ (yss.length : Rat) * nonNeg y := Rat.mul_le_mul_of_nonneg_right hx' hy0
      exact Rat.le_trans h1 (flatten_len_ge yss hy')
    · trivial
  · cases yss with
    | nil => right; rfl
    | cons ys rest =>
      left
      split at hh
      · rename_i u w hu hw
        simp only [Option.some.injEq] at hh; subst hh
        have eu := hc.hi; rw [hu] at eu; simp only [EndGe] at eu
        have hw' : ∀ z ∈ ys :: rest, (z.length : Rat) ≤ w := by
          intro z hz; have := (hci z hz).hi; rw [hw] at this; exact this
        have hw0 : (0 : Rat) ≤ w := Rat.le_trans (by exact_mod_cast Nat.zero_le _) (hw' ys List.mem_cons_self)
        have h1 := flatten_len_le (ys :: rest) hw'
        have h2 : ((ys :: rest).length : Rat) * w ≤ u * w := Rat.mul_le_mul_of_nonneg_right eu hw0
        exact Rat.le_trans h1 h2
      · simp at hh

theorem flatten_sound {a0 : Abs} {xs : List Val} {yss : List (List Val)} (h0 : Mem a0 (.list xs))
    (hys : xs.mapM listOf = some yss) (hsz : (yss.flatten.length : Int) ≤ i64max) :
    Mem (.list (timesCount (listParts a0).1 (listParts (listParts a0).2).1) (listParts (listParts a0).2).2) (.list yss.flatten) := by
  obtain ⟨hc, he⟩ := listParts_sound h0
  have hxs := mapM_listOf hys
  subst hxs
  have hin : ∀ ys ∈ yss, (listParts (listParts a0).2).1.In (ys.length : Rat) ∧
      ∀ y ∈ ys, Mem (listParts (listParts a0).2).2 y :=
    fun ys hys => listParts_sound (he (.list ys) (List.mem_map.mpr ⟨ys, hys, rfl⟩))
  refine Mem.list (timesCount_sound (by simpa using hc) (fun ys hys => (hin ys hys).1) hsz) (fun y hy => ?_)
  obtain ⟨ys, hys, hy⟩ := List.mem_flatten.mp hy
  exact (hin ys hys).2 y hy

-- ── Lists one after another ─────────────────────────────────────────────────

theorem addCounts_sound {a b : Iv} {m k : Nat} (ha : a.In (m : Rat)) (hb : b.In (k : Rat)) (hm : ((m + k : Nat) : Int) ≤ i64max) :
    (addCounts a b).In ((m + k : Nat) : Rat) := by
  unfold addCounts
  refine countIv_sound ?_ (fun h hh => Or.inl ?_) hm
  · split
    · rename_i x y hx hy
      have ex := ha.lo; rw [hx] at ex
      have ey := hb.lo; rw [hy] at ey
      simp only [EndLe] at ex ey ⊢; push_cast; grind
    · trivial
  · split at hh
    · rename_i x y hx hy
      simp only [Option.some.injEq] at hh; subst hh
      have ex := ha.hi; rw [hx] at ex
      have ey := hb.hi; rw [hy] at ey
      simp only [EndGe] at ex ey; push_cast; grind
    · simp at hh

theorem concat_fold_sound : ∀ (avals : List Abs) (yss : List (List Val)) (acc : Iv × Abs) (pre : List Val),
    avals.length = yss.length → (∀ p ∈ (avals.zip yss), Mem p.1 (.list p.2)) →
    acc.1.In (pre.length : Rat) → (∀ x ∈ pre, Mem acc.2 x) →
    ((pre ++ yss.flatten).length : Int) ≤ i64max →
    (avals.foldl (fun (acc : Iv × Abs) v => let (c, el) := listParts v; (addCounts acc.1 c, acc.2.join el)) acc).1.In
        ((pre ++ yss.flatten).length : Rat) ∧
      ∀ x ∈ pre ++ yss.flatten, Mem (avals.foldl (fun (acc : Iv × Abs) v => let (c, el) := listParts v;
        (addCounts acc.1 c, acc.2.join el)) acc).2 x
  | [], [], acc, pre, _, _, hc, he, _ => by simp only [List.foldl_nil, List.flatten_nil, List.append_nil]; exact ⟨hc, he⟩
  | a :: as, ys :: rest, acc, pre, hl, hm, hc, he, hsz => by
    simp only [List.foldl_cons]
    have hma := hm (a, ys) (by simp)
    obtain ⟨hca, hea⟩ := listParts_sound hma
    have e : pre ++ (ys :: rest).flatten = (pre ++ ys) ++ rest.flatten := by simp
    rw [e] at hsz ⊢
    have hlen : (((pre ++ ys).length : Nat) : Int) ≤ i64max := by
      have : (pre ++ ys).length ≤ ((pre ++ ys) ++ rest.flatten).length := by simp
      omega
    have hc' : (addCounts acc.1 (listParts a).1).In ((pre ++ ys).length : Rat) := by
      rw [List.length_append]
      exact addCounts_sound hc hca (by rw [List.length_append] at hlen; exact hlen)
    exact concat_fold_sound as rest _ (pre ++ ys) (by simpa using hl)
      (fun p hp => hm p (List.mem_cons_of_mem _ hp)) hc'
      (fun x hx => by
        rcases List.mem_append.mp hx with hx | hx
        · exact mem_join_left (he x hx)
        · exact mem_join_right (hea x hx)) hsz
  | [], _ :: _, _, _, hl, _, _, _, _ => by simp at hl
  | _ :: _, [], _, _, hl, _, _, _, _ => by simp at hl

-- ── Reading a dict and a set ────────────────────────────────────────────────

theorem get_sound {a0 : Abs} {ps : List (Val × Val)} {x : Val} (h0 : Mem a0 (.dict ps)) :
    Mem (optOf true (dictParts a0).2.2) (((ps.find? fun p => sameKey p.1 x).map (·.2)).getD .none) := by
  obtain ⟨_, _, hv, _⟩ := dictParts_sound h0
  rcases hf : ps.find? (fun p => sameKey p.1 x) with _ | p
  · rw [hf]; exact Mem_optOf_none rfl
  · rw [hf]; exact Mem_optOf (hv p (List.mem_of_find?_eq_some hf))

theorem keys_sound {a0 : Abs} {ps : List (Val × Val)} (h0 : Mem a0 (.dict ps)) :
    Mem (.list (dictParts a0).1 (dictParts a0).2.1) (.list (ps.map (·.1))) := by
  obtain ⟨hc, hk, _, _⟩ := dictParts_sound h0
  refine Mem.list (by rw [List.length_map]; exact hc) (fun y hy => ?_)
  obtain ⟨p, hp, rfl⟩ := List.mem_map.mp hy
  exact hk p hp

theorem values_sound {a0 : Abs} {ps : List (Val × Val)} (h0 : Mem a0 (.dict ps)) :
    Mem (.list (dictParts a0).1 (dictParts a0).2.2) (.list (ps.map (·.2))) := by
  obtain ⟨hc, _, hv, _⟩ := dictParts_sound h0
  refine Mem.list (by rw [List.length_map]; exact hc) (fun y hy => ?_)
  obtain ⟨p, hp, rfl⟩ := List.mem_map.mp hy
  exact hv p hp

theorem entries_sound {a0 : Abs} {ps : List (Val × Val)} (h0 : Mem a0 (.dict ps)) :
    Mem (.list (dictParts a0).1 (entryAbs (dictParts a0).2.1 (dictParts a0).2.2)) (.list (ps.map entryV)) := by
  obtain ⟨hc, hk, hv, _⟩ := dictParts_sound h0
  refine Mem.list (by rw [List.length_map]; exact hc) (fun y hy => ?_)
  obtain ⟨p, hp, rfl⟩ := List.mem_map.mp hy
  exact Mem_entry (hk p hp) (hv p hp)

theorem toList_sound {a0 : Abs} {ms : List Val} (h0 : Mem a0 (.set ms)) :
    Mem (.list (membersParts a0).1 (membersParts a0).2) (.list ms) := by
  obtain ⟨hc, he, _⟩ := membersParts_sound h0
  exact Mem.list hc he

-- ── The numbers between two ─────────────────────────────────────────────────

/-- `numbers(a, b)`'s count and the analysis' interval of it before it is held to a count. -/
theorem numbers_raw {ia ib : Iv} {a b : Int} (ha : ia.In (a : Rat)) (hb : ib.In (b : Rat)) :
    ((ib.minus ia).plus (Iv.exact 1)).In (((b - a + 1 : Int)) : Rat) := by
  have := In_plus (In_minus hb ha) (In_exact 1)
  push_cast; exact this

theorem numbers_count {ia ib : Iv} {a b : Int} (ha : ia.In (a : Rat)) (hb : ib.In (b : Rat)) (hab : a ≤ b)
    (hm : b - a + 1 ≤ i64max) :
    (countIv (((ib.minus ia).plus (Iv.exact 1)).lo.map nonNeg) ((ib.minus ia).plus (Iv.exact 1)).hi).In
      (((b - a + 1).toNat : Nat) : Rat) := by
  have hr := numbers_raw ha hb
  have hc : (((b - a + 1).toNat : Nat) : Rat) = (((b - a + 1 : Int)) : Rat) := by
    have : (((b - a + 1).toNat : Nat) : Int) = b - a + 1 := Int.toNat_of_nonneg (by omega)
    exact_mod_cast this
  refine countIv_sound ?_ (fun h hh => Or.inl ?_) (by rw [Int.toNat_of_nonneg (by omega)]; exact hm)
  · rw [hc]
    cases hlo : ((ib.minus ia).plus (Iv.exact 1)).lo with
    | none => trivial
    | some l =>
      have el := hr.lo; rw [hlo] at el; simp only [EndLe] at el
      simp only [Option.map_some, EndLe, nonNeg_eq]
      have : (0 : Rat) ≤ (((b - a + 1 : Int)) : Rat) := by exact_mod_cast (show (0 : Int) ≤ b - a + 1 by omega)
      split <;> grind
  · rw [hc]
    have eh := hr.hi; rw [hh] at eh; exact eh

-- ── What a builtin answers ──────────────────────────────────────────────────

theorem wholeOf_some {v : Val} {j : Int} (h : wholeOf v = some j) : ∃ r, v = .num r ∧ r.den = 1 ∧ r.num = j := by
  cases v with
  | num r =>
    simp only [wholeOf] at h
    split at h
    · rename_i hr; cases h; exact ⟨r, rfl, hr, rfl⟩
    · cases h
  | _ => simp [wholeOf] at h

theorem allMem_zip : ∀ {avals : List Abs} {yss : List (List Val)}, AllMem avals (yss.map .list) →
    ∀ p ∈ avals.zip yss, Mem p.1 (.list p.2)
  | [], [], _, _, h => by simp at h
  | a :: as, ys :: rest, h, p, hp => by
    simp only [List.map_cons, AllMem] at h
    simp only [List.zip_cons_cons, List.mem_cons] at hp
    rcases hp with rfl | hp
    · exact h.1
    · exact allMem_zip h.2 p hp
  | [], _ :: _, h, _, _ => by simp [AllMem] at h
  | _ :: _, [], h, _, _ => by simp [AllMem] at h

/-- The numbers from `a` to `b`, each among the numbers from the least `a` to the greatest `b`. -/
theorem numbers_sound {o : Out} {id line : Nat} {a0 a1 : Abs} {r q : Rat} (h0 : Mem a0 (.num r)) (h1 : Mem a1 (.num q))
    (hr : r.den = 1) (hq : q.den = 1) {v : Val}
    (hv : (if q.num < r.num then Except.ok (Val.list [])
      else
        let count := q.num - r.num + 1
        if i64max < count then failK .overflow line
        else if (NUMBERS : Int) < count then .error .diverged
        else .ok (.list ((List.range count.toNat).map fun (i : Nat) => .num ((r.num + (i : Int) : Int) : Rat))) : R Val) = .ok v) :
    Mem (primAbs o id .numbers [a0, a1]).1 v := by
  have ha := In_ivD h0
  have hb := In_ivD h1
  rw [rat_of_den_one hr] at ha
  rw [rat_of_den_one hq] at hb
  have hraw := numbers_raw ha hb
  have hcountOf := fun (hab : r.num ≤ q.num) (hm : q.num - r.num + 1 ≤ i64max) => numbers_count ha hb hab hm
  simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
  generalize ((a1.iv.getD Iv.top).minus (a0.iv.getD Iv.top)).plus (Iv.exact 1) = raw at hraw hcountOf ⊢
  split at hv
  · -- `b` below `a`: none
    rename_i hlt; cases hv
    split
    · exact Mem.list (In_ints (Int.le_refl 0) (Int.le_refl 0)) (fun x hx => by simp at hx)
    · refine Mem.list (countIv_sound ?_ (fun _ _ => Or.inr rfl) (by decide)) (fun x hx => by simp at hx)
      cases hlo : raw.lo with
      | none => trivial
      | some l =>
        have el := hraw.lo; rw [hlo] at el; simp only [EndLe] at el
        simp only [Option.map_some, EndLe, nonNeg_eq]
        have : (((q.num - r.num + 1 : Int)) : Rat) ≤ 0 := by exact_mod_cast (show q.num - r.num + 1 ≤ 0 by omega)
        have hz : ((([] : List Val).length : Nat) : Rat) = 0 := rfl
        rw [hz]
        split <;> grind
  · rename_i hge
    simp only at hv
    split at hv
    · exact absurd hv failK_ne_ok
    · split at hv
      · cases hv
      · rename_i hm _; cases hv
        have hm' : q.num - r.num + 1 ≤ i64max := by omega
        have hcount := hcountOf (by omega) hm'
        have hlen : ((List.range (q.num - r.num + 1).toNat).map fun (i : Nat) => Val.num ((r.num + (i : Int) : Int) : Rat)).length =
            (q.num - r.num + 1).toNat := by simp
        split
        · -- an upper end at most 0 says no number: there is one
          rename_i hneg
          exfalso
          have eh := hcount.hi
          generalize countIv (Option.map nonNeg raw.lo) raw.hi = c at hneg eh
          cases hh : c.hi with
          | none => rw [hh] at hneg; simp at hneg
          | some u =>
            rw [hh] at hneg eh; simp only [Option.any_some, decide_eq_true_eq] at hneg
            simp only [EndGe] at eh
            have h1 : (1 : Rat) ≤ (((q.num - r.num + 1).toNat : Nat) : Rat) := by
              exact_mod_cast (show 1 ≤ (q.num - r.num + 1).toNat by omega)
            have hu : u ≤ 0 := by
              rcases Int.lt_or_eq_of_le hneg with hlt | heq
              · have := (num_neg_iff u).mp hlt; grind
              · rw [Rat.num_eq_zero.mp heq]; grind
            grind
        · refine Mem.list (by rw [hlen]; exact hcount) (fun x hx => ?_)
          obtain ⟨i, hi, rfl⟩ := List.mem_map.mp hx
          simp only [List.mem_range] at hi
          refine Mem.num (In_mk' ?_ ?_ (by simp only [DenLe, Rat.den_intCast, Nat.le_refl]))
          · have el := ha.lo
            cases hl : (a0.iv.getD Iv.top).lo with
            | none => trivial
            | some l =>
              rw [hl] at el; simp only [EndLe] at el ⊢
              have : ((r.num : Int) : Rat) ≤ ((r.num + (i : Int) : Int) : Rat) := by exact_mod_cast (show r.num ≤ r.num + i by omega)
              exact Rat.le_trans el this
          · have eh := hb.hi
            cases hh : (a1.iv.getD Iv.top).hi with
            | none => trivial
            | some u =>
              rw [hh] at eh; simp only [EndGe] at eh ⊢
              have : ((r.num + (i : Int) : Int) : Rat) ≤ ((q.num : Int) : Rat) := by
                exact_mod_cast (show r.num + i ≤ q.num by omega)
              exact Rat.le_trans this eh

theorem allMem_cons {as : List Abs} {v : Val} {vs : List Val} (h : AllMem as (v :: vs)) :
    ∃ a as', as = a :: as' ∧ Mem a v ∧ AllMem as' vs := by
  cases as with
  | nil => simp [AllMem] at h
  | cons a as' => exact ⟨a, as', rfl, h.1, h.2⟩

theorem allMem_nil {as : List Abs} (h : AllMem as []) : as = [] := by
  cases as with
  | nil => rfl
  | cons _ _ => simp [AllMem] at h

end KanameCore
