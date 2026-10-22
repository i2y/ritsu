/-
  Strings (kaname's DESIGN 25.6, 15.2.2): what a builtin of a string answers is among what the
  analysis says of it, the count of its code points in the interval the analysis gives; `concat`
  and `text` likewise. Each count is a fact of the lists of `Text.lean`: a cut is no longer than
  what it cuts, a piece of a string cut at a separator no longer than the string, the pieces no more
  than one past its count, a padding as long as the width, a whole number of int64 written in 1 to
  20 characters, a day of the years 1 to 9999 in 10.

  The assembly of every builtin that takes no function (`primAbs_sound`) is here, for it takes the
  strings' cases with those of the dicts, the sets and the lists (`SoundPrim.lean`).
-/
import KanameCore.SoundPrim

namespace KanameCore

-- ── Counts of code points ───────────────────────────────────────────────────

/-- The count of a string is among what its abstract value says of it. -/
theorem strCount_sound {s : String} : ∀ {a : Abs}, Mem a (.str s) → (strCount a).In (s.length : Rat)
  | _, .any _ => by simp only [strCount]; exact anyCount_In _
  | _, .optSome _ hx => by simp only [strCount]; exact strCount_sound hx
  | _, .str hc => by simp only [strCount]; exact hc

/-- A string `sized` lets through has at most 2⁶³ − 1 code points. -/
theorem sized_str {s : String} {v' : Val} (h : sized (.str s) = .ok v') : (s.length : Int) ≤ i64max := by
  simp only [sized] at h
  split at h
  · cases h
  · omega

/-- A string of the code points `cs` is among those of a count holding their number. -/
theorem Mem_str_ofList {c : Iv} {cs : List Char} (h : c.In (cs.length : Rat)) : Mem (.str c) (.str (String.ofList cs)) :=
  Mem.str (by rw [String.length_ofList]; exact h)

theorem mapM_some_length {α β : Type} {f : α → Option β} :
    ∀ {xs : List α} {ys : List β}, xs.mapM f = some ys → ys.length = xs.length
  | [], ys, h => by simp at h; subst h; rfl
  | x :: rest, ys, h => by
    simp only [List.mapM_cons, Option.bind_eq_bind] at h
    rcases hx : f x with _ | y
    · rw [hx] at h; simp at h
    · rw [hx] at h
      simp only [Option.bind_some] at h
      rcases hr : rest.mapM f with _ | ys'
      · rw [hr] at h; simp at h
      · rw [hr] at h
        simp only [Option.bind_some, Option.pure_def, Option.some.injEq] at h
        subst h
        simp [mapM_some_length hr]

theorem mapM_textVal : ∀ {xs : List Val} {ss : List String}, xs.mapM textVal = some ss → xs = ss.map .str
  | [], ss, h => by simp at h; subst h; rfl
  | x :: rest, ss, h => by
    simp only [List.mapM_cons, Option.bind_eq_bind] at h
    rcases hx : textVal x with _ | t
    · rw [hx] at h; simp at h
    · rw [hx] at h
      simp only [Option.bind_some] at h
      rcases hr : rest.mapM textVal with _ | ss'
      · rw [hr] at h; simp at h
      · rw [hr] at h
        simp only [Option.bind_some, Option.pure_def, Option.some.injEq] at h
        subst h
        cases x <;> simp [textVal] at hx
        subst hx
        simp [mapM_textVal hr]

-- ── Cuts: counted as the cuts of a list as long ─────────────────────────────

/-- A list as long as the code points `cs`, of nothing in particular: a cut of a string is counted
    as the same cut of it (`take_sound`, `drop_sound`, `slice_sound`). -/
def blanks (cs : List Char) : List Val := cs.map fun _ => .none

theorem Mem_blanks {a : Abs} {s : String} (h : Mem a (.str s)) :
    Mem (.list (strCount a) .any) (.list (blanks s.toList)) :=
  Mem.list (by simp only [blanks, List.length_map, String.length_toList]; exact strCount_sound h) (fun x _ => Mem.any x)

theorem str_of_blanks {c : Iv} {e : Abs} {ys : List Val} {cs : List Char} (h : Mem (.list c e) (.list ys))
    (hl : ys.length = cs.length) : Mem (.str c) (.str (String.ofList cs)) := by
  cases h with
  | list hc _ => exact Mem.str (by rw [String.length_ofList, ← hl]; exact hc)

theorem blanks_take (cs : List Char) (j : Int) :
    ((blanks cs).take (clampTo (blanks cs).length j)).length = (Text.take cs j).length := by
  simp [blanks, Text.take, Text.cut, clampTo]

theorem blanks_drop (cs : List Char) (j : Int) :
    ((blanks cs).drop (clampTo (blanks cs).length j)).length = (Text.drop cs j).length := by
  simp [blanks, Text.drop, Text.cut, clampTo]

theorem length_cut {α β : Type} (f : α → β) (l : List α) (i j : Nat) :
    (if i < j then ((l.map f).drop i).take (j - i) else []).length =
      (if i < j then (l.drop i).take (j - i) else []).length := by
  split <;> simp

theorem blanks_slice (cs : List Char) (a b : Int) :
    (if clampTo (blanks cs).length a < clampTo (blanks cs).length b then
      ((blanks cs).drop (clampTo (blanks cs).length a)).take (clampTo (blanks cs).length b - clampTo (blanks cs).length a)
      else []).length = (Text.slice cs a b).length := by
  simp only [blanks, Text.slice, Text.cut, clampTo, List.length_map]
  exact length_cut _ _ _ _

theorem takeT_sound {o : Out} {id : Nat} {a0 a1 : Abs} {s : String} {r : Rat} (h0 : Mem a0 (.str s))
    (h1 : Mem a1 (.num r)) (hr : r.den = 1) (hsz : ((Text.take s.toList r.num).length : Int) ≤ i64max) :
    Mem (primAbs o id .takeT [a0, a1]).1 (.str (String.ofList (Text.take s.toList r.num))) := by
  have h := take_sound (Mem_blanks h0) h1 hr (by rw [blanks_take]; exact hsz)
  simp only [listParts] at h
  simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
  exact str_of_blanks h (blanks_take _ _)

theorem dropT_sound {o : Out} {id : Nat} {a0 a1 : Abs} {s : String} {r : Rat} (h0 : Mem a0 (.str s))
    (h1 : Mem a1 (.num r)) (hr : r.den = 1) (hsz : ((Text.drop s.toList r.num).length : Int) ≤ i64max) :
    Mem (primAbs o id .dropT [a0, a1]).1 (.str (String.ofList (Text.drop s.toList r.num))) := by
  have h := drop_sound (Mem_blanks h0) h1 hr (by rw [blanks_drop]; exact hsz)
  simp only [listParts] at h
  simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
  exact str_of_blanks h (blanks_drop _ _)

theorem sliceT_sound {o : Out} {id : Nat} {a0 a1 a2 : Abs} {s : String} {r q : Rat} (h0 : Mem a0 (.str s))
    (h1 : Mem a1 (.num r)) (h2 : Mem a2 (.num q)) (hr : r.den = 1) (hq : q.den = 1)
    (hsz : ((Text.slice s.toList r.num q.num).length : Int) ≤ i64max) :
    Mem (primAbs o id .sliceT [a0, a1, a2]).1 (.str (String.ofList (Text.slice s.toList r.num q.num))) := by
  have h := slice_sound (Mem_blanks h0) h1 h2 hr hq (by rw [blanks_slice]; exact hsz)
  simp only [listParts] at h
  simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
  exact str_of_blanks h (blanks_slice _ _ _)

-- ── Lists of a string and a string of a list ────────────────────────────────

theorem char_toNat_le (c : Char) : c.toNat ≤ 1114111 := by
  have h := c.valid
  simp only [UInt32.isValidChar, Nat.isValidChar] at h
  rw [← Char.toNat_val]
  omega

theorem chars_sound {o : Out} {id : Nat} {a0 : Abs} {s : String} (h0 : Mem a0 (.str s)) :
    Mem (primAbs o id .chars [a0]).1 (.list (s.toList.map fun c => .str (String.ofList [c]))) := by
  simp only [primAbs, List.headD]
  refine Mem.list (by rw [List.length_map, String.length_toList]; exact strCount_sound h0) (fun x hx => ?_)
  obtain ⟨c, _, rfl⟩ := List.mem_map.mp hx
  exact Mem_str_ofList (In_ints_nat (by simp) (by simp))

theorem codePoints_sound {o : Out} {id : Nat} {a0 : Abs} {s : String} (h0 : Mem a0 (.str s)) :
    Mem (primAbs o id .codePoints [a0]).1 (.list (s.toList.map fun c => .num ((c.toNat : Int) : Rat))) := by
  simp only [primAbs, List.headD]
  refine Mem.list (by rw [List.length_map, String.length_toList]; exact strCount_sound h0) (fun x hx => ?_)
  obtain ⟨c, _, rfl⟩ := List.mem_map.mp hx
  exact Mem.num (In_ints (by omega) (by have := char_toNat_le c; omega))

theorem toText_sound {o : Out} {id : Nat} {a0 : Abs} {xs : List Val} {ns : List Int} (h0 : Mem a0 (.list xs))
    (hns : xs.mapM wholeOf = some ns) :
    Mem (primAbs o id .toText [a0]).1 (match Text.ofCodePoints ns with
      | some cs => .str (String.ofList cs)
      | Option.none => .none) := by
  simp only [primAbs, List.headD]
  obtain ⟨hc, _⟩ := listParts_sound h0
  split
  · rename_i cs hcs
    unfold Text.ofCodePoints at hcs
    have l1 := mapM_some_length hns
    have l2 := mapM_some_length hcs
    exact Mem_optOf (Mem_str_ofList (by rw [l2, l1]; exact hc))
  · exact Mem_optOf_none rfl

theorem charOf_sound {o : Out} {id k : Nat} {avals : List Abs} :
    Mem (primAbs o id .charOf avals).1 (.str (String.ofList [Char.ofNat k])) := by
  simp only [primAbs]
  exact Mem_str_ofList (In_ints_nat (by simp) (by simp))

-- ── Strings no longer than the string ───────────────────────────────────────

theorem trimStart_length (cs : List Char) : (Text.trimStart cs).length ≤ cs.length :=
  (List.dropWhile_suffix _).length_le

theorem trimEnd_length (cs : List Char) : (Text.trimEnd cs).length ≤ cs.length := by
  unfold Text.trimEnd
  rw [List.length_reverse]
  exact Nat.le_trans (List.dropWhile_suffix _).length_le (by simp)

theorem trim_length (cs : List Char) : (Text.trim cs).length ≤ cs.length :=
  Nat.le_trans (trimEnd_length _) (trimStart_length _)

/-- A string no longer than `s` is among the strings of 0 code points to as many as `s` can have. -/
theorem shorter_sound {c : Iv} {s : String} {cs : List Char} (h : c.In (s.length : Rat)) (hl : cs.length ≤ s.length)
    (hsz : (cs.length : Int) ≤ i64max) : Mem (.str (countIv (some 0) c.hi)) (.str (String.ofList cs)) := by
  refine Mem_str_ofList (countIv_sound endLe_zero (fun hv hh => Or.inl ?_) hsz)
  have := h.hi
  rw [hh] at this
  simp only [EndGe] at this
  exact Rat.le_trans (by exact_mod_cast hl) this

theorem trim_sound {o : Out} {id : Nat} {a0 : Abs} {s : String} (h0 : Mem a0 (.str s))
    (hsz : ((Text.trim s.toList).length : Int) ≤ i64max) :
    Mem (primAbs o id .trim [a0]).1 (.str (String.ofList (Text.trim s.toList))) := by
  simp only [primAbs, List.headD]
  exact shorter_sound (strCount_sound h0) (by have := trim_length s.toList; rwa [String.length_toList] at this) hsz

theorem trimStart_sound {o : Out} {id : Nat} {a0 : Abs} {s : String} (h0 : Mem a0 (.str s))
    (hsz : ((Text.trimStart s.toList).length : Int) ≤ i64max) :
    Mem (primAbs o id .trimStart [a0]).1 (.str (String.ofList (Text.trimStart s.toList))) := by
  simp only [primAbs, List.headD]
  exact shorter_sound (strCount_sound h0) (by have := trimStart_length s.toList; rwa [String.length_toList] at this) hsz

theorem trimEnd_sound {o : Out} {id : Nat} {a0 : Abs} {s : String} (h0 : Mem a0 (.str s))
    (hsz : ((Text.trimEnd s.toList).length : Int) ≤ i64max) :
    Mem (primAbs o id .trimEnd [a0]).1 (.str (String.ofList (Text.trimEnd s.toList))) := by
  simp only [primAbs, List.headD]
  exact shorter_sound (strCount_sound h0) (by have := trimEnd_length s.toList; rwa [String.length_toList] at this) hsz

theorem lower_sound {o : Out} {id : Nat} {a0 : Abs} {s : String} (h0 : Mem a0 (.str s)) :
    Mem (primAbs o id .lowerAscii [a0]).1 (.str (String.ofList (Text.lowerAscii s.toList))) := by
  simp only [primAbs, List.headD]
  exact Mem_str_ofList (by rw [Text.lowerAscii, List.length_map, String.length_toList]; exact strCount_sound h0)

theorem upper_sound {o : Out} {id : Nat} {a0 : Abs} {s : String} (h0 : Mem a0 (.str s)) :
    Mem (primAbs o id .upperAscii [a0]).1 (.str (String.ofList (Text.upperAscii s.toList))) := by
  simp only [primAbs, List.headD]
  exact Mem_str_ofList (by rw [Text.upperAscii, List.length_map, String.length_toList]; exact strCount_sound h0)

theorem replace_sound {o : Out} {id : Nat} {avals : List Abs} {cs : List Char} (hsz : (cs.length : Int) ≤ i64max) :
    Mem (primAbs o id .replace avals).1 (.str (String.ofList cs)) := by
  simp only [primAbs]
  exact Mem_str_ofList (countIv_sound endLe_zero (fun h hh => by cases hh; exact Or.inl (by exact_mod_cast hsz)) hsz)

theorem num_pos_of {l : Rat} (h : 1 ≤ l.num) : 0 < l := by
  rcases (Rat.le_total : l ≤ 0 ∨ 0 ≤ l) with hl | hl
  · exfalso
    rcases Rat.le_iff_lt_or_eq.mp hl with hlt | heq
    · have := (num_neg_iff l).mpr hlt; omega
    · rw [heq] at h; simp at h
  · rcases Rat.le_iff_lt_or_eq.mp hl with hlt | heq
    · exact hlt
    · rw [← heq] at h; simp at h

/-- A string whose count is said to be above 0 at least is not empty. -/
theorem nonempty_of_lo {c : Iv} {s : String} (h : c.In (s.length : Rat)) (hlo : c.lo.any (·.num ≥ 1) = true) :
    s.toList ≠ [] := by
  intro he
  have el := h.lo
  cases hl : c.lo with
  | none => rw [hl] at hlo; simp at hlo
  | some l =>
    rw [hl] at hlo el
    simp only [Option.any_some, decide_eq_true_eq] at hlo
    simp only [EndLe] at el
    have h0' : s.length = 0 := by rw [← String.length_toList, he]; rfl
    rw [h0'] at el
    have := num_pos_of hlo
    simp at el
    grind

-- ── Where a string is found ─────────────────────────────────────────────────

theorem findFrom_bound (t : List Char) : ∀ (k : Nat) (cs : List Char) (i : Nat), Text.findFrom t k cs = some i →
    k ≤ i ∧ i + t.length ≤ k + cs.length
  | k, [], i, h => by
    simp only [Text.findFrom] at h
    split at h
    · rename_i hp
      cases h
      have := (List.isPrefixOf_iff_prefix.mp hp).length_le
      simp only [List.length_nil] at this ⊢
      omega
    · cases h
  | k, c :: rest, i, h => by
    simp only [Text.findFrom] at h
    split at h
    · rename_i hp
      cases h
      have := (List.isPrefixOf_iff_prefix.mp hp).length_le
      exact ⟨Nat.le_refl _, by omega⟩
    · have := findFrom_bound t (k + 1) rest i h
      simp only [List.length_cons]
      omega

theorem indexOf_sound {o : Out} {id : Nat} {a0 a1 : Abs} {s t : String} (h0 : Mem a0 (.str s)) (h1 : Mem a1 (.str t)) :
    Mem (primAbs o id .indexOf [a0, a1]).1 (match Text.indexOf s.toList t.toList with
      | some i => .num ((i : Int) : Rat)
      | Option.none => .none) := by
  have hn := strCount_sound h0
  have ht := strCount_sound h1
  simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
  have hleast : ((strCount a1).lo.map nonNeg).getD 0 ≤ (t.length : Rat) := by
    have el := ht.lo
    cases hl : (strCount a1).lo with
    | none => simp only [Option.map_none, Option.getD_none]; exact_mod_cast Nat.zero_le _
    | some l =>
      rw [hl] at el
      simp only [EndLe] at el
      simp only [Option.map_some, Option.getD_some, nonNeg_eq]
      split
      · exact_mod_cast Nat.zero_le _
      · exact el
  generalize ((strCount a1).lo.map nonNeg).getD 0 = least at hleast
  rcases hf : Text.indexOf s.toList t.toList with _ | i
  · split <;> exact Mem.optNone
  · have hb := findFrom_bound t.toList 0 s.toList i hf
    rw [String.length_toList, String.length_toList] at hb
    have hi' : ((i : Nat) : Rat) + (t.length : Rat) ≤ (s.length : Rat) := by exact_mod_cast (by omega : i + t.length ≤ s.length)
    have hi0 : (0 : Rat) ≤ ((i : Nat) : Rat) := by exact_mod_cast Nat.zero_le i
    have hcast : (((i : Nat) : Int) : Rat) = ((i : Nat) : Rat) := Rat.intCast_natCast i
    split
    · rename_i hneg
      exfalso
      cases hh : (strCount a0).hi with
      | none => rw [hh] at hneg; simp at hneg
      | some h =>
        rw [hh] at hneg
        simp only [Option.map_some, Option.any_some, decide_eq_true_eq] at hneg
        have eh := hn.hi
        rw [hh] at eh
        simp only [EndGe] at eh
        have hlt := (num_neg_iff (h - least)).mp hneg
        grind
    · refine Mem.optSome (by simp) (Mem.num (In_mk' ?_ ?_ ?_))
      · simp only [EndLe]; rw [hcast]; exact hi0
      · cases hh : (strCount a0).hi with
        | none => trivial
        | some h =>
          have eh := hn.hi
          rw [hh] at eh
          simp only [EndGe] at eh
          simp only [Option.map_some, EndGe]
          rw [hcast]
          grind
      · simp [DenLe]

-- ── Pieces ──────────────────────────────────────────────────────────────────

theorem splitAux_facts (sep : List Char) (hs : sep ≠ []) : ∀ (n : Nat) (cur cs : List Char), cs.length ≤ n →
    1 ≤ (Text.splitAux sep hs cur cs).length ∧ (Text.splitAux sep hs cur cs).length ≤ cs.length + 1 ∧
      ∀ p ∈ Text.splitAux sep hs cur cs, p.length ≤ cur.length + cs.length
  | _, cur, [], _ => by
    rw [Text.splitAux]
    simp
  | 0, cur, c :: rest, hn => by simp at hn
  | n + 1, cur, c :: rest, hn => by
    have hr : rest.length ≤ n := by simp at hn; omega
    rw [Text.splitAux]
    split
    · have hsep : 0 < sep.length := List.length_pos_iff.mpr hs
      have hlen : ((c :: rest).drop sep.length).length ≤ n := by simp; omega
      obtain ⟨_, i2, i3⟩ := splitAux_facts sep hs n [] ((c :: rest).drop sep.length) hlen
      refine ⟨by simp, ?_, ?_⟩
      · simp at i2 ⊢; omega
      · intro p hp
        simp only [List.mem_cons] at hp
        rcases hp with rfl | hp
        · simp
        · have := i3 p hp; simp at this ⊢; omega
    · obtain ⟨i1, i2, i3⟩ := splitAux_facts sep hs n (c :: cur) rest hr
      refine ⟨i1, by simp at i2 ⊢; omega, fun p hp => by have := i3 p hp; simp at this ⊢; omega⟩

theorem split_facts (cs sep : List Char) :
    (Text.split cs sep).length ≤ cs.length + 1 ∧ (sep ≠ [] → 1 ≤ (Text.split cs sep).length) ∧
      (sep = [] → (Text.split cs sep).length = cs.length) ∧ ∀ p ∈ Text.split cs sep, p.length ≤ cs.length := by
  unfold Text.split
  split
  · rename_i h
    refine ⟨by simp, fun h' => absurd h h', fun _ => by simp, fun p hp => ?_⟩
    obtain ⟨c, hc, rfl⟩ := List.mem_map.mp hp
    simp only [List.length_singleton]
    exact List.length_pos_of_mem hc
  · rename_i h
    obtain ⟨i1, i2, i3⟩ := splitAux_facts sep h cs.length [] cs (Nat.le_refl _)
    exact ⟨i2, fun _ => i1, fun h' => absurd h' h, fun p hp => by have := i3 p hp; simpa using this⟩

theorem wordsAux_facts : ∀ (cur cs : List Char),
    (Text.wordsAux cur cs).length ≤ cs.length + 1 ∧ ∀ p ∈ Text.wordsAux cur cs, p.length ≤ cur.length + cs.length
  | cur, [] => by
    simp only [Text.wordsAux]
    split <;> simp
  | cur, c :: rest => by
    simp only [Text.wordsAux]
    obtain ⟨i1, i2⟩ := wordsAux_facts [] rest
    obtain ⟨j1, j2⟩ := wordsAux_facts (c :: cur) rest
    split
    · split
      · exact ⟨by simp at i1 ⊢; omega, fun p hp => by have := i2 p hp; simp at this ⊢; omega⟩
      · refine ⟨by simp at i1 ⊢; omega, fun p hp => ?_⟩
        simp only [List.mem_cons] at hp
        rcases hp with rfl | hp
        · simp
        · have := i2 p hp; simp at this ⊢; omega
    · exact ⟨by simp at j1 ⊢; omega, fun p hp => by have := j2 p hp; simp at this ⊢; omega⟩

/-- A count from one past the string's count, below by `lo`. -/
theorem pieces_count {n : Iv} {s : String} {lo : End} {L : Nat} (hn : n.In (s.length : Rat)) (hl : EndLe lo (L : Rat))
    (hL : L ≤ s.length + 1) (hsz : (L : Int) ≤ i64max) : (countIv lo (plusOne n.hi)).In (L : Rat) := by
  refine countIv_sound hl (fun h hh => Or.inl ?_) hsz
  cases hhi : n.hi with
  | none => rw [hhi] at hh; simp [plusOne] at hh
  | some u =>
    rw [hhi] at hh
    simp only [plusOne, Option.map_some, Option.some.injEq] at hh
    subst hh
    have eh := hn.hi
    rw [hhi] at eh
    simp only [EndGe] at eh
    have : (L : Rat) ≤ (s.length : Rat) + 1 := by exact_mod_cast hL
    grind

theorem split_sound {o : Out} {id : Nat} {a0 a1 : Abs} {s sep : String} (h0 : Mem a0 (.str s)) (h1 : Mem a1 (.str sep))
    (hs : (s.length : Int) ≤ i64max) (hsz : ((Text.split s.toList sep.toList).length : Int) ≤ i64max) :
    Mem (primAbs o id .split [a0, a1]).1 (.list ((Text.split s.toList sep.toList).map fun p => .str (String.ofList p))) := by
  have hn := strCount_sound h0
  have hsp := strCount_sound h1
  obtain ⟨f1, f2, f3, f4⟩ := split_facts s.toList sep.toList
  rw [String.length_toList] at f1 f3 f4
  simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
  refine Mem.list ?_ (fun x hx => ?_)
  · rw [List.length_map]
    refine pieces_count hn ?_ f1 hsz
    split
    · rename_i hge
      have hne : sep.toList ≠ [] := nonempty_of_lo hsp hge
      simp only [EndLe]
      exact_mod_cast f2 hne
    · cases hl : (strCount a0).lo with
      | none => simp [minLo, EndLe]
      | some l =>
        simp only [minLo, EndLe]
        have el := hn.lo
        rw [hl] at el
        simp only [EndLe] at el
        by_cases he : sep.toList = []
        · rw [f3 he]
          split
          · exact el
          · rename_i hgt; grind
        · have : (1 : Rat) ≤ ((Text.split s.toList sep.toList).length : Rat) := by exact_mod_cast f2 he
          split
          · grind
          · exact this
  · obtain ⟨p, hp, rfl⟩ := List.mem_map.mp hx
    exact shorter_sound hn (f4 p hp) (by have := f4 p hp; omega)

theorem words_sound {o : Out} {id : Nat} {a0 : Abs} {s : String} (h0 : Mem a0 (.str s))
    (hs : (s.length : Int) ≤ i64max) (hsz : ((Text.words s.toList).length : Int) ≤ i64max) :
    Mem (primAbs o id .words [a0]).1 (.list ((Text.words s.toList).map fun p => .str (String.ofList p))) := by
  have hn := strCount_sound h0
  obtain ⟨f1, f2⟩ := wordsAux_facts [] s.toList
  rw [String.length_toList] at f1 f2
  simp only [primAbs, List.headD]
  refine Mem.list (by rw [List.length_map]; exact pieces_count hn endLe_zero f1 hsz) (fun x hx => ?_)
  obtain ⟨p, hp, rfl⟩ := List.mem_map.mp hx
  have := f2 p hp
  simp only [List.length_nil, Nat.zero_add] at this
  exact shorter_sound hn this (by omega)

-- ── Padding ─────────────────────────────────────────────────────────────────

theorem pad_length (cs fill : List Char) (w : Int) (atEnd : Bool) :
    ((Text.pad cs w fill atEnd).length : Int) = if fill = [] ∨ w ≤ cs.length then (cs.length : Int) else w := by
  unfold Text.pad
  split
  · simp
  · rename_i h
    simp only [not_or] at h
    have hw : (cs.length : Int) < w := by omega
    cases atEnd <;> simp [Text.padding] <;> omega

theorem padded_sound {n w fl : Iv} {s f : String} {j : Int} {cs : List Char} (hn : n.In (s.length : Rat))
    (hw : w.In (j : Rat)) (hf : fl.In (f.length : Rat))
    (hl : (cs.length : Int) = if f.toList = [] ∨ j ≤ s.toList.length then (s.toList.length : Int) else j)
    (hsz : (cs.length : Int) ≤ i64max) :
    Mem (.str (countIv (if fl.lo.any (·.num ≥ 1) then maxLo n.lo w.lo else n.lo) (maxHi n.hi w.hi)))
      (.str (String.ofList cs)) := by
  rw [String.length_toList] at hl
  have hge : (s.length : Int) ≤ cs.length := by split at hl <;> omega
  have hgw : f.toList ≠ [] → j ≤ (cs.length : Int) := by
    intro hne
    split at hl
    · rename_i h; rcases h with h | h
      · exact absurd h hne
      · omega
    · omega
  have hle : (cs.length : Int) = s.length ∨ (cs.length : Int) = j := by split at hl <;> omega
  have hs' : (s.length : Rat) ≤ (cs.length : Rat) := by exact_mod_cast hge
  have hnl : EndLe n.lo (cs.length : Rat) := by
    have := hn.lo
    cases hh : n.lo with
    | none => trivial
    | some a => rw [hh] at this; simp only [EndLe] at this ⊢; exact Rat.le_trans this hs'
  refine Mem_str_ofList (countIv_sound ?_ ?_ hsz)
  · split
    · rename_i hfl
      have hj : (j : Rat) ≤ (cs.length : Rat) := by
        have := hgw (nonempty_of_lo hf hfl)
        have h2 : ((j : Int) : Rat) ≤ (((cs.length : Nat) : Int) : Rat) := by exact_mod_cast this
        rwa [Rat.intCast_natCast] at h2
      have hwl : EndLe w.lo (cs.length : Rat) := by
        have := hw.lo
        cases hh : w.lo with
        | none => trivial
        | some a => rw [hh] at this; simp only [EndLe] at this ⊢; exact Rat.le_trans this hj
      cases ha : n.lo with
      | none =>
        cases hb : w.lo with
        | none => simp [maxLo, EndLe]
        | some b => rw [hb] at hwl; simpa [maxLo] using hwl
      | some a =>
        cases hb : w.lo with
        | none => rw [ha] at hnl; simpa [maxLo] using hnl
        | some b =>
          rw [ha] at hnl; rw [hb] at hwl
          simp only [EndLe] at hnl hwl
          simp only [maxLo, EndLe]
          split <;> assumption
    · exact hnl
  · intro h hh
    left
    cases ha : n.hi with
    | none => rw [ha] at hh; simp [maxHi] at hh
    | some a =>
      cases hb : w.hi with
      | none => rw [ha, hb] at hh; simp [maxHi] at hh
      | some b =>
        rw [ha, hb] at hh
        simp only [maxHi, Option.some.injEq] at hh
        subst hh
        have e1 := hn.hi; rw [ha] at e1; simp only [EndGe] at e1
        have e2 := hw.hi; rw [hb] at e2; simp only [EndGe] at e2
        rcases hle with h1 | h1
        · have : (cs.length : Rat) = (s.length : Rat) := by exact_mod_cast h1
          rw [this]
          split
          · exact e1
          · rename_i hba; grind
        · have : (cs.length : Rat) = (j : Rat) := by
            have h2 : (((cs.length : Nat) : Int) : Rat) = ((j : Int) : Rat) := by exact_mod_cast h1
            rwa [Rat.intCast_natCast] at h2
          rw [this]
          split
          · rename_i hba; grind
          · exact e2

theorem padStart_sound {o : Out} {id : Nat} {a0 a1 a2 : Abs} {s f : String} {r : Rat} (h0 : Mem a0 (.str s))
    (h1 : Mem a1 (.num r)) (h2 : Mem a2 (.str f)) (hr : r.den = 1)
    (hsz : ((Text.pad s.toList r.num f.toList false).length : Int) ≤ i64max) :
    Mem (primAbs o id .padStart [a0, a1, a2]).1 (.str (String.ofList (Text.pad s.toList r.num f.toList false))) := by
  have hw := In_ivD h1
  rw [rat_of_den_one hr] at hw
  simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
  exact padded_sound (strCount_sound h0) hw (strCount_sound h2) (pad_length _ _ _ _) hsz

theorem padEnd_sound {o : Out} {id : Nat} {a0 a1 a2 : Abs} {s f : String} {r : Rat} (h0 : Mem a0 (.str s))
    (h1 : Mem a1 (.num r)) (h2 : Mem a2 (.str f)) (hr : r.den = 1)
    (hsz : ((Text.pad s.toList r.num f.toList true).length : Int) ≤ i64max) :
    Mem (primAbs o id .padEnd [a0, a1, a2]).1 (.str (String.ofList (Text.pad s.toList r.num f.toList true))) := by
  have hw := In_ivD h1
  rw [rat_of_den_one hr] at hw
  simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
  exact padded_sound (strCount_sound h0) hw (strCount_sound h2) (pad_length _ _ _ _) hsz

-- ── A number and a date read ────────────────────────────────────────────────

theorem parseNumber_range {cs : List Char} {x : Int} (h : Text.parseNumber cs = some x) : i64min ≤ x ∧ x ≤ i64max := by
  unfold Text.parseNumber at h
  repeat' split at h
  all_goals first | (cases h; done) | skip
  all_goals (simp only at h; split at h <;> (cases h; first | done | (constructor <;> omega) | assumption))

theorem parseDate_range {cs : List Char} {z : Int} (h : Text.parseDate cs = some z) :
    Text.DAY_MIN ≤ z ∧ z ≤ Text.DAY_MAX := by
  unfold Text.parseDate at h
  repeat' split at h
  all_goals first | (cases h; done) | skip
  all_goals (simp only at h; split at h <;> (cases h; first | done | (constructor <;> omega) | assumption))

theorem parseNumber_sound {o : Out} {id : Nat} {avals : List Abs} {cs : List Char} :
    Mem (primAbs o id .parseNumber avals).1 (match Text.parseNumber cs with
      | some x => .num (x : Rat)
      | Option.none => .none) := by
  simp only [primAbs]
  rcases hp : Text.parseNumber cs with _ | x
  · exact Mem.optNone
  · obtain ⟨h1, h2⟩ := parseNumber_range hp
    exact Mem.optSome (by simp) (Mem.num (In_mk' (by simp only [EndLe]; exact_mod_cast h1)
      (by simp only [EndGe]; exact_mod_cast h2) (by simp [DenLe])))

theorem parseDate_sound {o : Out} {id : Nat} {avals : List Abs} {cs : List Char} :
    Mem (primAbs o id .parseDate avals).1 (match Text.parseDate cs with
      | some z => .date z
      | Option.none => .none) := by
  simp only [primAbs]
  rcases hp : Text.parseDate cs with _ | z
  · exact Mem.optNone
  · obtain ⟨h1, h2⟩ := parseDate_range hp
    exact Mem.optSome (by simp) (Mem.date (In_mk' (by simp only [EndLe]; exact_mod_cast h1)
      (by simp only [EndGe]; exact_mod_cast h2) (by simp [DenLe])))

-- ── Strings put together ────────────────────────────────────────────────────

theorem intercalate_length (sep : String) : ∀ (ss : List String),
    (sep.intercalate ss).length = (ss.map String.length).sum + (ss.length - 1) * sep.length
  | [] => by simp
  | [t] => by simp [String.intercalate_singleton]
  | t :: u :: l => by
    rw [String.intercalate_cons_cons, String.length_append, String.length_append, intercalate_length sep (u :: l)]
    simp only [List.map_cons, List.sum_cons, List.length_cons, Nat.add_sub_cancel, Nat.add_mul, Nat.one_mul]
    omega

theorem nonNeg_nonneg (v : Rat) : 0 ≤ nonNeg v := by
  rw [nonNeg_eq]; split
  · exact Rat.le_refl
  · rename_i h; exact Rat.not_lt.mp h

theorem nonNeg_le {v : Rat} {n : Nat} (h : v ≤ (n : Rat)) : nonNeg v ≤ (n : Rat) := by
  rw [nonNeg_eq]; split
  · exact_mod_cast Nat.zero_le n
  · exact h

theorem sum_ge {ss : List String} {e : Rat} (he : ∀ t ∈ ss, e ≤ (t.length : Rat)) :
    (ss.length : Rat) * e ≤ (((ss.map String.length).sum : Nat) : Rat) := by
  induction ss with
  | nil => simp
  | cons t rest ih =>
    have h1 := he t (by simp)
    have h2 := ih (fun u hu => he u (by simp [hu]))
    simp only [List.length_cons, List.map_cons, List.sum_cons]
    push_cast
    rw [Rat.add_mul, Rat.one_mul]
    grind

theorem sum_le {ss : List String} {e : Rat} (he : ∀ t ∈ ss, (t.length : Rat) ≤ e) :
    (((ss.map String.length).sum : Nat) : Rat) ≤ (ss.length : Rat) * e := by
  induction ss with
  | nil => simp
  | cons t rest ih =>
    have h1 := he t (by simp)
    have h2 := ih (fun u hu => he u (by simp [hu]))
    simp only [List.length_cons, List.map_cons, List.sum_cons]
    push_cast
    rw [Rat.add_mul, Rat.one_mul]
    grind

theorem gap_le {c b : Rat} {k g : Nat} (hck : c ≤ (k : Rat)) (hb0 : 0 ≤ b) (hbg : b ≤ (g : Rat)) :
    (if c.num > 0 then c - 1 else 0) * b ≤ ((k - 1 : Nat) : Rat) * (g : Rat) := by
  have hR : (0 : Rat) ≤ ((k - 1 : Nat) : Rat) * (g : Rat) :=
    Rat.mul_nonneg (by exact_mod_cast Nat.zero_le _) (by exact_mod_cast Nat.zero_le _)
  split
  · rcases (Rat.le_total : c - 1 ≤ 0 ∨ 0 ≤ c - 1) with h | h
    · have := Rat.mul_le_mul_of_nonneg_right h hb0
      rw [Rat.zero_mul] at this
      exact Rat.le_trans this hR
    · have hk1 : 1 ≤ k := by
        have : (1 : Rat) ≤ (k : Rat) := by grind
        exact_mod_cast this
      rw [natSub_cast hk1]
      push_cast
      have h1 : (c - 1) * b ≤ ((k : Rat) - 1) * b := Rat.mul_le_mul_of_nonneg_right (by grind) hb0
      have h2 : ((k : Rat) - 1) * b ≤ ((k : Rat) - 1) * (g : Rat) := Rat.mul_le_mul_of_nonneg_left hbg (by grind)
      exact Rat.le_trans h1 h2
  · rw [Rat.zero_mul]; exact hR

theorem gap_ge {c b : Rat} {k g : Nat} (hk : 1 ≤ k) (hck : (k : Rat) ≤ c) (hgb : (g : Rat) ≤ b) :
    ((k - 1 : Nat) : Rat) * (g : Rat) ≤ (if c.num > 0 then c - 1 else 0) * b := by
  have hk1 : (1 : Rat) ≤ (k : Rat) := by exact_mod_cast hk
  have hc : 0 < c := by grind
  have hp : c.num > 0 := num_pos_of_pos hc
  simp only [hp, ↓reduceIte]
  rw [natSub_cast hk]
  push_cast
  have hb0 : 0 ≤ b := Rat.le_trans (by exact_mod_cast Nat.zero_le g) hgb
  have h1 : ((k : Rat) - 1) * (g : Rat) ≤ ((k : Rat) - 1) * b := Rat.mul_le_mul_of_nonneg_left hgb (by grind)
  have h2 : ((k : Rat) - 1) * b ≤ (c - 1) * b := Rat.mul_le_mul_of_nonneg_right (by grind) hb0
  exact Rat.le_trans h1 h2

/-- The least count of strings put together: the fewest of the shortest, the gaps of the shortest
    separator. -/
theorem joined_lo {cl el sl : Rat} {ss : List String} {g : Nat} (hc : cl ≤ (ss.length : Rat))
    (he : ∀ t ∈ ss, el ≤ (t.length : Rat)) (hs : sl ≤ (g : Rat)) :
    nonNeg cl * nonNeg el + (if (nonNeg cl).num > 0 then nonNeg cl - 1 else 0) * nonNeg sl ≤
      (((ss.map String.length).sum + (ss.length - 1) * g : Nat) : Rat) := by
  have h1 : (ss.length : Rat) * nonNeg el ≤ (((ss.map String.length).sum : Nat) : Rat) :=
    sum_ge (fun t ht => nonNeg_le (he t ht))
  have h2 : nonNeg cl * nonNeg el ≤ (ss.length : Rat) * nonNeg el :=
    Rat.mul_le_mul_of_nonneg_right (nonNeg_le hc) (nonNeg_nonneg el)
  have h3 := gap_le (nonNeg_le hc) (nonNeg_nonneg sl) (nonNeg_le hs)
  rw [Rat.natCast_add, Rat.natCast_mul]
  grind

/-- The greatest count of one string or more put together: the most of the longest, the gaps of the
    longest separator. -/
theorem joined_hi {ch eh sh : Rat} {ss : List String} {g : Nat} (hk : 1 ≤ ss.length) (hc : (ss.length : Rat) ≤ ch)
    (he : ∀ t ∈ ss, (t.length : Rat) ≤ eh) (hs : (g : Rat) ≤ sh) :
    (((ss.map String.length).sum + (ss.length - 1) * g : Nat) : Rat) ≤
      nonNeg ch * eh + (if (nonNeg ch).num > 0 then nonNeg ch - 1 else 0) * sh := by
  have hk1 : (1 : Rat) ≤ (ss.length : Rat) := by exact_mod_cast hk
  have hch : nonNeg ch = ch := by rw [nonNeg_eq]; split <;> grind
  rw [hch]
  obtain ⟨t0, ht0⟩ : ∃ t, t ∈ ss := by
    cases ss with
    | nil => simp at hk
    | cons t _ => exact ⟨t, by simp⟩
  have he0 : 0 ≤ eh := Rat.le_trans (by exact_mod_cast Nat.zero_le _) (he t0 ht0)
  have h1 : (((ss.map String.length).sum : Nat) : Rat) ≤ (ss.length : Rat) * eh := sum_le he
  have h2 : (ss.length : Rat) * eh ≤ ch * eh := Rat.mul_le_mul_of_nonneg_right hc he0
  have h3 := gap_ge hk hc hs
  rw [Rat.natCast_add, Rat.natCast_mul]
  grind

theorem join_sound {o : Out} {id : Nat} {a0 a1 : Abs} {ss : List String} {sep : String}
    (h0 : Mem a0 (.list (ss.map .str))) (h1 : Mem a1 (.str sep)) (hsz : ((sep.intercalate ss).length : Int) ≤ i64max) :
    Mem (primAbs o id .join [a0, a1]).1 (.str (sep.intercalate ss)) := by
  obtain ⟨hc, he⟩ := listParts_sound h0
  rw [List.length_map] at hc
  have het : ∀ t ∈ ss, (strCount (listParts a0).2).In (t.length : Rat) :=
    fun t ht => strCount_sound (he _ (List.mem_map_of_mem ht))
  have hsp := strCount_sound h1
  have hL := intercalate_length sep ss
  simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
  refine Mem.str (countIv_sound ?_ ?_ hsz)
  · rw [hL]
    cases hcl : (listParts a0).1.lo with
    | none => simp only [Option.map_none, joinedEnd, Option.or, EndLe]; exact_mod_cast Nat.zero_le _
    | some cl =>
      cases hel : (strCount (listParts a0).2).lo with
      | none => simp only [Option.map_none, Option.map_some, joinedEnd, Option.or, EndLe]; exact_mod_cast Nat.zero_le _
      | some el =>
        cases hsl : (strCount a1).lo with
        | none => simp only [Option.map_none, Option.map_some, joinedEnd, Option.or, EndLe]; exact_mod_cast Nat.zero_le _
        | some sl =>
          simp only [Option.map_some, joinedEnd, Option.or, EndLe]
          have hc' := hc.lo
          rw [hcl] at hc'
          simp only [EndLe] at hc'
          have he' : ∀ t ∈ ss, el ≤ (t.length : Rat) := fun t ht => by
            have := (het t ht).lo
            rw [hel] at this
            exact this
          have hs' := hsp.lo
          rw [hsl] at hs'
          simp only [EndLe] at hs'
          exact joined_lo hc' he' hs'
  · intro h hh
    rw [hL]
    by_cases hk : ss.length = 0
    · right
      have : ss = [] := List.length_eq_zero_iff.mp hk
      subst this
      simp
    · left
      cases hch : (listParts a0).1.hi with
      | none => rw [hch] at hh; simp [joinedEnd] at hh
      | some ch =>
        cases heh : (strCount (listParts a0).2).hi with
        | none => rw [hch, heh] at hh; simp [joinedEnd] at hh
        | some eh =>
          cases hsh : (strCount a1).hi with
          | none => rw [hch, heh, hsh] at hh; simp [joinedEnd] at hh
          | some sh =>
            rw [hch, heh, hsh] at hh
            simp only [Option.map_some, joinedEnd, Option.some.injEq] at hh
            subst hh
            have hc' := hc.hi
            rw [hch] at hc'
            simp only [EndGe] at hc'
            have he' : ∀ t ∈ ss, (t.length : Rat) ≤ eh := fun t ht => by
              have := (het t ht).hi
              rw [heh] at this
              exact this
            have hs' := hsp.hi
            rw [hsh] at hs'
            simp only [EndGe] at hs'
            exact joined_hi (by omega) hc' he' hs'

-- ── What `text` writes ──────────────────────────────────────────────────────

/-- A whole number of int64 is written in 1 to 20 characters. -/
theorem int_text_length {x : Int} (h1 : i64min ≤ x) (h2 : x ≤ i64max) :
    1 ≤ (toString x).length ∧ (toString x).length ≤ 20 := by
  show 1 ≤ (Int.repr x).length ∧ (Int.repr x).length ≤ 20
  simp only [i64min, i64max] at h1 h2
  cases x with
  | ofNat m =>
    simp only [Int.repr]
    refine ⟨Nat.length_repr_pos, (Nat.length_repr_le_iff (by decide)).mpr ?_⟩
    have : (m : Int) ≤ 9223372036854775807 := h2
    omega
  | negSucc m =>
    simp only [Int.repr, String.length_append, Nat.succ_eq_add_one]
    have hm : (m + 1 : Nat) < 10 ^ 19 := by
      have : Int.negSucc m = -((m : Int) + 1) := rfl
      omega
    have := (Nat.length_repr_le_iff (by decide)).mpr hm
    have h1' : ("-" : String).length = 1 := rfl
    rw [h1']
    omega

theorem ratText_whole {r : Rat} (h : r.den = 1) : ratText r = toString r.num := by
  unfold ratText; simp [h]

theorem padNat_length {w n : Nat} (hw : 0 < w) (h : n < 10 ^ w) : (padNat w n).length = w := by
  unfold padNat
  simp only [String.length_append, String.length_ofList, List.length_replicate]
  have := (Nat.length_repr_le_iff hw).mpr h
  show w - (Nat.repr n).length + (Nat.repr n).length = w
  omega

theorem dateText_length {d : Int} (h : textOK .plain (.date d) = true) : (dateText d).length = 10 := by
  simp only [textOK] at h
  unfold dateText
  rcases hc : civil d with ⟨y, m, dd⟩
  rw [hc] at h
  simp only [Bool.and_eq_true, decide_eq_true_eq] at h
  obtain ⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩ := h
  simp only [String.length_append]
  rw [padNat_length (by decide) (by omega), padNat_length (by decide) (by omega), padNat_length (by decide) (by omega)]
  rfl

/-- The name of a value of an enum or of a bool, as long as the shortest name at least and the
    longest at most. -/
theorem names_count {names : List String} {x : String} (hx : x ∈ names) :
    (match (names.map String.length).min?, (names.map String.length).max? with
      | some lo, some hi => Iv.ints lo hi
      | _, _ => Iv.ints 0 0).In (x.length : Rat) := by
  have hmem : x.length ∈ names.map String.length := List.mem_map_of_mem hx
  rcases h1 : (names.map String.length).min? with _ | lo
  · exact absurd (List.min?_eq_none_iff.mp h1) (List.ne_nil_of_mem hmem)
  rcases h2 : (names.map String.length).max? with _ | hi
  · exact absurd (List.max?_eq_none_iff.mp h2) (List.ne_nil_of_mem hmem)
  have l1 := (List.min?_eq_some_iff.mp h1).2 _ hmem
  have l2 := (List.max?_eq_some_iff.mp h2).2 _ hmem
  exact In_ints_nat (by omega) (by omega)

/-- The count of the code points `text` writes is among those the analysis says (DESIGN 25.6). -/
theorem text_sound {fmt : TextFmt} {a : Abs} {v : Val} {s : String} (hm : Mem a v) (hok : textOK fmt v = true)
    (ht : textOf fmt v = some s) : (textCount fmt a).In (s.length : Rat) := by
  cases fmt with
  | rate => exact anyCount_In _
  | number =>
    cases v with
    | num r =>
      simp only [textOK, Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at hok
      obtain ⟨⟨hr, h1⟩, h2⟩ := hok
      simp only [textOf, Option.some.injEq] at ht
      subst ht
      rw [ratText_whole hr]
      obtain ⟨l1, l2⟩ := int_text_length h1 h2
      exact In_ints_nat (by omega) (by omega)
    | _ => simp [textOK] at hok
  | unit u =>
    cases v with
    | num r =>
      simp only [textOK, Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at hok
      obtain ⟨⟨hr, h1⟩, h2⟩ := hok
      simp only [textOf, Option.some.injEq] at ht
      subst ht
      rw [String.length_append, ratText_whole hr]
      obtain ⟨l1, l2⟩ := int_text_length h1 h2
      exact In_ints_nat (by omega) (by omega)
    | _ => simp [textOK] at hok
  | plain =>
    cases hm with
    | any => exact anyCount_In _
    | optNone => simp [textOf] at ht
    | optSome _ _ => exact anyCount_In _
    | num _ => exact anyCount_In _
    | date _ =>
      simp only [textOf, Option.some.injEq] at ht
      subst ht
      rw [dateText_length hok]
      exact In_ints_nat (by omega) (by omega)
    | enumAll => exact anyCount_In _
    | boolAll => exact anyCount_In _
    | enum hx =>
      simp only [textOf, Option.some.injEq] at ht
      subst ht
      exact names_count hx
    | bool hx =>
      simp only [textOf, Option.some.injEq] at ht
      subst ht
      exact names_count hx
    | str hc =>
      simp only [textOf, Option.some.injEq] at ht
      subst ht
      exact hc
    | list _ _ => simp [textOf] at ht
    | record _ _ => simp [textOf] at ht
    | transfer _ => simp [textOf] at ht
    | variantEnum _ => exact anyCount_In _
    | variant _ _ _ _ => simp [textOf] at ht
    | dict _ _ _ _ => simp [textOf] at ht
    | members _ _ _ => simp [textOf] at ht
    | closAny => simp [textOf] at ht
    | clos _ _ _ => simp [textOf] at ht

-- ── Every builtin that takes no function ────────────────────────────────────

set_option maxHeartbeats 4000000 in
/-- **What a builtin of a dict, a set, a list or a string answers** is among what the analysis says
    of it, the arguments each among theirs (DESIGN 24.5, 25.6). -/
theorem primAbs_sound {o : Out} {id line : Nat} {op : Prim} {key : Key} {avals : List Abs} {vs : List Val} {v v' : Val}
    (hall : AllMem avals vs) (hv : primVal line op key vs = .ok v) (hsz : sized v = .ok v') :
    Mem (primAbs o id op avals).1 v' := by
  obtain rfl := sized_eq hsz
  cases op <;> simp only [primVal] at hv <;> split at hv <;> (try contradiction) <;> (try exact absurd hv failK_ne_ok)
  -- get
  · rename_i ps x; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain ⟨a1, as2, rfl, _, h2⟩ := allMem_cons h1
    obtain rfl := allMem_nil h2
    simp only [primAbs, List.headD]; exact get_sound h0
  -- put
  · rename_i ps x y; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain ⟨a1, as2, rfl, hx, h2⟩ := allMem_cons h1
    obtain ⟨a2, as3, rfl, hy, h3⟩ := allMem_cons h2
    obtain rfl := allMem_nil h3
    simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
    exact put_sound h0 hx hy (sized_dict hsz)
  -- removeD
  · rename_i ps x; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain ⟨a1, as2, rfl, _, h2⟩ := allMem_cons h1
    obtain rfl := allMem_nil h2
    simp only [primAbs, List.headD]; exact removeD_sound h0 (sized_dict hsz)
  -- hasD
  · cases hv; simp only [primAbs]; exact Mem_allBools _
  -- keys
  · rename_i ps; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    simp only [primAbs, List.headD]; exact keys_sound h0
  -- values
  · rename_i ps; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    simp only [primAbs, List.headD]; exact values_sound h0
  -- entries
  · rename_i ps; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    simp only [primAbs, List.headD]; exact entries_sound h0
  -- merge
  · rename_i ps qs; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain ⟨a1, as2, rfl, hq, h2⟩ := allMem_cons h1
    obtain rfl := allMem_nil h2
    simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
    exact merge_sound h0 hq (sized_dict hsz)
  -- add
  · rename_i ms x; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain ⟨a1, as2, rfl, hx, h2⟩ := allMem_cons h1
    obtain rfl := allMem_nil h2
    simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
    exact add_sound h0 hx (sized_set hsz)
  -- removeS
  · rename_i ms x; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain ⟨a1, as2, rfl, _, h2⟩ := allMem_cons h1
    obtain rfl := allMem_nil h2
    simp only [primAbs, List.headD]; exact removeS_sound h0 (sized_set hsz)
  -- hasS
  · cases hv; simp only [primAbs]; exact Mem_allBools _
  -- union
  · rename_i ms os; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain ⟨a1, as2, rfl, ho, h2⟩ := allMem_cons h1
    obtain rfl := allMem_nil h2
    simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
    exact union_sound h0 ho (sized_set hsz)
  -- intersect
  · rename_i ms os; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain ⟨a1, as2, rfl, ho, h2⟩ := allMem_cons h1
    obtain rfl := allMem_nil h2
    simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
    exact intersect_sound h0 ho (sized_set hsz)
  -- difference
  · rename_i ms os; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain ⟨a1, as2, rfl, ho, h2⟩ := allMem_cons h1
    obtain rfl := allMem_nil h2
    simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
    exact difference_sound h0 ho (sized_set hsz)
  -- toList
  · rename_i ms; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    simp only [primAbs, List.headD]; exact toList_sound h0
  -- first
  · rename_i xs; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    simp only [primAbs, List.headD]; exact first_sound h0
  -- last
  · rename_i xs; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    simp only [primAbs, List.headD]; exact last_sound h0
  -- at
  · split at hv
    · cases hv
      obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
      obtain ⟨a1, as2, rfl, _, h2⟩ := allMem_cons h1
      obtain rfl := allMem_nil h2
      simp only [primAbs, List.headD]; exact at_sound h0
    · exact absurd hv failK_ne_ok
  -- append
  · rename_i xs x; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain ⟨a1, as2, rfl, hx, h2⟩ := allMem_cons h1
    obtain rfl := allMem_nil h2
    simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
    exact append_sound h0 hx (sized_list hsz)
  -- take
  · split at hv
    · rename_i j hj; cases hv
      obtain ⟨r, rfl, hr, rfl⟩ := wholeOf_some hj
      obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
      obtain ⟨a1, as2, rfl, hn, h2⟩ := allMem_cons h1
      obtain rfl := allMem_nil h2
      simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
      exact take_sound h0 hn hr (sized_list hsz)
    · exact absurd hv failK_ne_ok
  -- drop
  · split at hv
    · rename_i j hj; cases hv
      obtain ⟨r, rfl, hr, rfl⟩ := wholeOf_some hj
      obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
      obtain ⟨a1, as2, rfl, hn, h2⟩ := allMem_cons h1
      obtain rfl := allMem_nil h2
      simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
      exact drop_sound h0 hn hr (sized_list hsz)
    · exact absurd hv failK_ne_ok
  -- slice
  · split at hv
    · rename_i i j hi hj; cases hv
      obtain ⟨r, rfl, hr, rfl⟩ := wholeOf_some hi
      obtain ⟨q, rfl, hq, rfl⟩ := wholeOf_some hj
      obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
      obtain ⟨a1, as2, rfl, ha, h2⟩ := allMem_cons h1
      obtain ⟨a2, as3, rfl, hb, h3⟩ := allMem_cons h2
      obtain rfl := allMem_nil h3
      simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
      exact slice_sound h0 ha hb hr hq (sized_list hsz)
    · exact absurd hv failK_ne_ok
  -- reverse
  · rename_i xs; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    simp only [primAbs, List.headD]; exact reverse_sound h0
  -- sort
  · rename_i xs; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    simp only [primAbs, List.headD]; exact sort_sound h0
  -- distinct
  · rename_i xs; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    simp only [primAbs, List.headD]; exact distinct_sound h0 (sized_list hsz)
  -- contains
  · cases hv; simp only [primAbs]; exact Mem_allBools _
  -- flatten
  · rename_i xs
    split at hv
    · rename_i yss hys; cases hv
      obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
      obtain rfl := allMem_nil h1
      simp only [primAbs, List.headD]; exact flatten_sound h0 hys (sized_list hsz)
    · exact absurd hv failK_ne_ok
  -- indexed
  · rename_i xs; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    simp only [primAbs, List.headD]; exact indexed_sound h0
  -- toSet
  · rename_i xs; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    simp only [primAbs, List.headD]; exact toSet_sound h0 (sized_set hsz)
  -- join
  · split at hv
    · rename_i xs sep ss hss; cases hv
      have := mapM_textVal hss; subst this
      obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
      obtain ⟨a1, as2, rfl, h1', h2⟩ := allMem_cons h1
      obtain rfl := allMem_nil h2
      exact join_sound h0 h1' (sized_str hsz)
    · exact absurd hv failK_ne_ok
  -- concat
  · rename_i yss hys; cases hv
    have hls := mapM_listOf hys
    subst hls
    have hlen : avals.length = yss.length := by have := AllMem.length hall; simpa using this
    simp only [primAbs]
    have := concat_fold_sound avals yss (Iv.ints 0 0, .never) [] hlen (allMem_zip hall)
      (In_ints (Int.le_refl 0) (Int.le_refl 0)) (fun x hx => by simp at hx) (by simpa using sized_list hsz)
    simp only [List.nil_append] at this
    exact Mem.list this.1 this.2
  -- numbers
  · split at hv
    · rename_i i j hi hj
      obtain ⟨r, rfl, hr, rfl⟩ := wholeOf_some hi
      obtain ⟨q, rfl, hq, rfl⟩ := wholeOf_some hj
      obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
      obtain ⟨a1, as2, rfl, h1', h2⟩ := allMem_cons h1
      obtain rfl := allMem_nil h2
      exact numbers_sound h0 h1' hr hq hv
    · exact absurd hv failK_ne_ok

  -- chars
  · rename_i s; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    exact chars_sound h0
  -- codePoints
  · rename_i s; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    exact codePoints_sound h0
  -- toText
  · split at hv
    · rename_i xs ns hns; cases hv
      obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
      obtain rfl := allMem_nil h1
      exact toText_sound h0 hns
    · exact absurd hv failK_ne_ok
  -- sliceT
  · split at hv
    · rename_i i j hi hj; cases hv
      obtain ⟨r, rfl, hr, rfl⟩ := wholeOf_some hi
      obtain ⟨q, rfl, hq, rfl⟩ := wholeOf_some hj
      obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
      obtain ⟨a1, as2, rfl, ha, h2⟩ := allMem_cons h1
      obtain ⟨a2, as3, rfl, hb, h3⟩ := allMem_cons h2
      obtain rfl := allMem_nil h3
      exact sliceT_sound h0 ha hb hr hq (by have := sized_str hsz; rwa [String.length_ofList] at this)
    · exact absurd hv failK_ne_ok
  -- takeT
  · split at hv
    · rename_i j hj; cases hv
      obtain ⟨r, rfl, hr, rfl⟩ := wholeOf_some hj
      obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
      obtain ⟨a1, as2, rfl, hn, h2⟩ := allMem_cons h1
      obtain rfl := allMem_nil h2
      exact takeT_sound h0 hn hr (by have := sized_str hsz; rwa [String.length_ofList] at this)
    · exact absurd hv failK_ne_ok
  -- dropT
  · split at hv
    · rename_i j hj; cases hv
      obtain ⟨r, rfl, hr, rfl⟩ := wholeOf_some hj
      obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
      obtain ⟨a1, as2, rfl, hn, h2⟩ := allMem_cons h1
      obtain rfl := allMem_nil h2
      exact dropT_sound h0 hn hr (by have := sized_str hsz; rwa [String.length_ofList] at this)
    · exact absurd hv failK_ne_ok
  -- indexOf
  · rename_i s t; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain ⟨a1, as2, rfl, h1', h2⟩ := allMem_cons h1
    obtain rfl := allMem_nil h2
    exact indexOf_sound h0 h1'
  -- containsT
  · cases hv; simp only [primAbs]; exact Mem_allBools _
  -- endsWith
  · cases hv; simp only [primAbs]; exact Mem_allBools _
  -- split
  · split at hv
    · cases hv
    · rename_i s sep hlong; cases hv
      obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
      obtain ⟨a1, as2, rfl, h1', h2⟩ := allMem_cons h1
      obtain rfl := allMem_nil h2
      exact split_sound h0 h1' (by omega) (by have := sized_list hsz; rwa [List.length_map] at this)
  -- words
  · split at hv
    · cases hv
    · rename_i s hlong; cases hv
      obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
      obtain rfl := allMem_nil h1
      exact words_sound h0 (by omega) (by have := sized_list hsz; rwa [List.length_map] at this)
  -- replace
  · cases hv; exact replace_sound (by have := sized_str hsz; rwa [String.length_ofList] at this)
  -- trim
  · cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    exact trim_sound h0 (by have := sized_str hsz; rwa [String.length_ofList] at this)
  -- trimStart
  · cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    exact trimStart_sound h0 (by have := sized_str hsz; rwa [String.length_ofList] at this)
  -- trimEnd
  · cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    exact trimEnd_sound h0 (by have := sized_str hsz; rwa [String.length_ofList] at this)
  -- lowerAscii
  · rename_i s; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    exact lower_sound h0
  -- upperAscii
  · rename_i s; cases hv
    obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
    obtain rfl := allMem_nil h1
    exact upper_sound h0
  -- padStart
  · split at hv
    · rename_i j hj
      split at hv
      · cases hv
      · cases hv
        obtain ⟨r, rfl, hr, rfl⟩ := wholeOf_some hj
        obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
        obtain ⟨a1, as2, rfl, hw, h2⟩ := allMem_cons h1
        obtain ⟨a2, as3, rfl, hf, h3⟩ := allMem_cons h2
        obtain rfl := allMem_nil h3
        exact padStart_sound h0 hw hf hr (by have := sized_str hsz; rwa [String.length_ofList] at this)
    · exact absurd hv failK_ne_ok
  -- padEnd
  · split at hv
    · rename_i j hj
      split at hv
      · cases hv
      · cases hv
        obtain ⟨r, rfl, hr, rfl⟩ := wholeOf_some hj
        obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
        obtain ⟨a1, as2, rfl, hw, h2⟩ := allMem_cons h1
        obtain ⟨a2, as3, rfl, hf, h3⟩ := allMem_cons h2
        obtain rfl := allMem_nil h3
        exact padEnd_sound h0 hw hf hr (by have := sized_str hsz; rwa [String.length_ofList] at this)
    · exact absurd hv failK_ne_ok
  -- parseNumber
  · cases hv; exact parseNumber_sound
  -- parseDate
  · cases hv; exact parseDate_sound
  -- charOf
  · split at hv
    · split at hv
      · cases hv; exact charOf_sound
      · exact absurd hv failK_ne_ok
    · exact absurd hv failK_ne_ok

-- ── `concat` and `text` ─────────────────────────────────────────────────────

/-- The counts of strings put together, added one after another from those so far. -/
theorem concat_counts : ∀ (avals : List Abs) (ss : List String) (acc : Iv) (pre : Nat),
    AllMem avals (ss.map .str) → acc.In (pre : Rat) → ((pre + (ss.map String.length).sum : Nat) : Int) ≤ i64max →
    (avals.foldl (fun acc v => addCounts acc (strCount v)) acc).In ((pre + (ss.map String.length).sum : Nat) : Rat)
  | [], [], acc, pre, _, hc, _ => by simpa using hc
  | a :: as, t :: rest, acc, pre, hall, hc, hsz => by
    simp only [List.map_cons, AllMem] at hall
    obtain ⟨ha, hrest⟩ := hall
    simp only [List.foldl_cons]
    have e : pre + ((t :: rest).map String.length).sum = (pre + t.length) + (rest.map String.length).sum := by
      simp only [List.map_cons, List.sum_cons]; omega
    rw [e] at hsz ⊢
    have h1 : ((pre + t.length : Nat) : Int) ≤ i64max := by omega
    exact concat_counts as rest _ (pre + t.length) hrest (addCounts_sound hc (strCount_sound ha) h1) hsz
  | [], _ :: _, _, _, hall, _, _ => by simp [AllMem] at hall
  | _ :: _, [], _, _, hall, _, _ => by simp [AllMem] at hall

section
variable {C : Ctx} {res : List (Option Abs)} {n : Nat}

theorem case_concat (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id : Nat}
    {args : List Expr} (hG : Good C res A) (hS : Sat A ρ) (hw : (evI A m (.concat id args) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.concat id args)) (evI A m (.concat id args) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    split at hw
    · rename_i hnone
      simp only [AR.of] at hw ⊢
      obtain ⟨a1, a2⟩ := IH.list m A ρ args o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨vals, hvals, _⟩ := bind_ok hv
        obtain ⟨avs, havs, _⟩ := a1 vals hvals
        rw [hnone] at havs; cases havs
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨vals, hvals, _⟩
        · exact a2 k l hv hk
        · obtain ⟨avs, havs, _⟩ := a1 vals hvals
          rw [hnone] at havs; cases havs
    · rename_i avs hsome
      simp only [AR.of] at hw ⊢
      obtain ⟨a1, a2⟩ := IH.list m A ρ args o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨vals, hvals, hv1⟩ := bind_ok hv
        obtain ⟨avs', havs, hall, hs⟩ := a1 vals hvals
        rw [hsome] at havs; cases havs
        split at hv1
        · rename_i ss hss
          have := mapM_textVal hss; subst this
          obtain rfl := sized_eq hv1
          have hsz := sized_str hv1
          rw [String.length_join] at hsz
          refine ⟨Mem.str ?_, hs, fun _ _ => trivial⟩
          rw [String.length_join]
          have := concat_counts avs ss (Iv.ints 0 0) 0 hall (In_ints (Int.le_refl 0) (Int.le_refl 0))
            (by simpa using hsz)
          simpa using this
        · exact absurd hv1 failK_ne_ok
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨vals, _, hv1⟩
        · exact a2 k l hv hk
        · split at hv1
          · simp only [sized] at hv1
            split at hv1 <;> cases hv1
          · rw [fail_type hv1] at hk; cases hk

theorem case_text (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id : Nat} {x : Expr}
    {fmt : TextFmt} (hG : Good C res A) (hS : Sat A ρ) (hw : (evI A m (.text id x fmt) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.text id x fmt)) (evI A m (.text id x fmt) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    split at hw
    · rename_i hnev
      simp only [AR.of] at hw ⊢
      obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨xv, hxv, _⟩ := bind_ok hv
        have := (h1 xv hxv).1; rw [hnev] at this; exact absurd this Mem.not_never
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨xv, hxv, _⟩
        · exact h2 k l hv hk
        · have := (h1 xv hxv).1; rw [hnev] at this; exact absurd this Mem.not_never
    · rename_i a hne
      simp only [AR.of] at hw ⊢
      obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨xv, hxv, hv1⟩ := bind_ok hv
        obtain ⟨hm, hs, _⟩ := h1 xv hxv
        split at hv1
        · rename_i hok
          split at hv1
          · rename_i s hs'
            cases hv1
            exact ⟨Mem.str (text_sound hm hok hs'), hs, fun _ _ => trivial⟩
          · exact absurd hv1 failK_ne_ok
        · exact absurd hv1 failK_ne_ok
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨xv, _, hv1⟩
        · exact h2 k l hv hk
        · split at hv1
          · split at hv1
            · cases hv1
            · rw [fail_type hv1] at hk; cases hk
          · rw [fail_type hv1] at hk; cases hk

end

end KanameCore
