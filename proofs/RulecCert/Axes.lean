/-
  A numeric axis tiles the values its column takes (§6.2, rulec's §15.201, §15.236).

  The claims of `RulecCert.Sound` are about points of the space of coordinates, and `RulecCert.Cells`
  ties a coordinate to the values it stands for. What is left between the two is that every value a
  column can take stands on **one** coordinate of its axis: none between two coordinates, none on two.
  `checkAxes` in `RulecMain.lean` held the coordinates to the declared range and to each other, but
  no theorem said what that settles, and the check let two things through that it should not have:
  two intervals that meet at a value, which leaves that value out of both, and the same point twice,
  which puts one value on two coordinates. `axisTilesB` is the check as it now stands, and
  `axisTilesB_cover` and `axisTilesB_unique` are what a pass settles.

  With them, what a walk answers — a `max`, a `min`, a fold's answer — stands on a coordinate of the
  axis a table below reads it on (`extreme_on_axis`, `foldAnswer_on_axis`): `extreme_mem_stated` and
  `foldAnswer_mem_stated` put the answer inside the range the certificate states, and the axis tiles
  that range. A point made of such coordinates is a point of the table's space, and the table's own
  certificate answers it there (`Certified.answers_at`).
-/
import RulecCert.Certified
import RulecCert.Cells

namespace RulecCert

/-! ## The check -/

/-- A coordinate is cut on the grid `st`: its ends lie on the grid, and an interval with two ends runs
    forwards. -/
def Coord.onGridB (st : Rat) : Coord → Bool
  | .exactly a => gridB st a
  | .between lo hi =>
    (match lo with | some a => gridB st a | none => true) &&
    (match hi with | some b => gridB st b | none => true) &&
    (match lo, hi with | some a, some b => decide (a < b) | _, _ => true)

/-- Two coordinates side by side on an axis cut on the grid `st`, with no value of the grid between
    them and none in both: two points a step apart, or a point and an interval that begins or ends
    at it. Two intervals that meet leave the value they meet at out of both, and one point written
    twice puts a value on two coordinates, so neither is side by side. -/
def nextTo (st : Rat) : Coord → Coord → Bool
  | .exactly a, .exactly b => b == a + st
  | .exactly a, .between (some b) _ => b == a
  | .between _ (some a), .exactly b => b == a
  | _, _ => false

/-- The first coordinate begins where the range does: on a point at its low end, or running on below
    where the range has none. -/
def beginsAt : Option Rat → Coord → Bool
  | some l, .exactly a => a == l
  | none, .between none _ => true
  | _, _ => false

/-- The last coordinate ends where the range does: on a point at its high end, or running on above
    where the range has none. -/
def endsAt : Option Rat → Coord → Bool
  | some h, .exactly a => a == h
  | none, .between _ none => true
  | _, _ => false

/-- From this coordinate on, each is cut on the grid and side by side with the next, and the last
    ends where the range does. -/
def tilesOn (st : Rat) (hi : Option Rat) : List Coord → Bool
  | [] => false
  | [x] => x.onGridB st && endsAt hi x
  | x :: y :: rest => x.onGridB st && nextTo st x y && tilesOn st hi (y :: rest)

/-- **The tiling check** of a numeric axis over the range `lo`–`hi` (an end `none` where the column has
    none): a grid that moves, the first coordinate beginning where the range does, and from there each
    side by side with the next up to the last, which ends where the range does. -/
def axisTilesB (st : Rat) (lo hi : Option Rat) (cs : List Coord) : Bool :=
  decide (0 < st) &&
    (match cs with
     | [] => false
     | x :: _ => beginsAt lo x) &&
    tilesOn st hi cs

/-! ## What a pass settles: every value of the range stands on a coordinate -/

/-- A value at or above where a coordinate begins: its point or above, past its low end, or anything
    where it runs on below. -/
def startsBelow (x : Coord) (v : Rat) : Prop :=
  match x with
  | .exactly a => a ≤ v
  | .between (some a) _ => a < v
  | .between none _ => True

theorem tilesOn_cover {st : Rat} (hst : 0 < st) {hi : Option Rat} {v : Rat} (hv : onGrid st v)
    (hhi : ∀ u, hi = some u → v ≤ u) :
    ∀ (cs : List Coord) (x : Coord), tilesOn st hi (x :: cs) = true → startsBelow x v →
      ∃ (c : Nat) (y : Coord), (x :: cs)[c]? = some y ∧ y.holds v
  | [], x, h, hs => by
    simp only [tilesOn, Bool.and_eq_true] at h
    obtain ⟨_, hend⟩ := h
    match x, hi, hend with
    | .exactly a, some u, hend =>
      have hau : a = u := eq_of_beq hend
      have hle : v ≤ u := hhi u rfl
      have hge : a ≤ v := hs
      refine ⟨0, .exactly a, rfl, ?_⟩
      show v = a
      subst hau
      exact Rat.le_antisymm hle hge
    | .between lo none, none, _ =>
      refine ⟨0, .between lo none, rfl, ?_⟩
      refine ⟨fun a ha => ?_, fun b hb => by cases hb⟩
      subst ha
      exact hs
  | y :: cs, x, h, hs => by
    simp only [tilesOn, Bool.and_eq_true] at h
    obtain ⟨⟨hgx, hnext⟩, hrest⟩ := h
    have shift : (∃ (c : Nat) (z : Coord), (y :: cs)[c]? = some z ∧ z.holds v) →
        ∃ (c : Nat) (z : Coord), (x :: y :: cs)[c]? = some z ∧ z.holds v := by
      rintro ⟨c, z, hc, hz⟩
      exact ⟨c + 1, z, by simpa using hc, hz⟩
    match x, y, hnext with
    | .exactly a, .exactly b, hnext =>
      have hb : b = a + st := eq_of_beq hnext
      by_cases hva : v = a
      · exact ⟨0, .exactly a, rfl, hva⟩
      · have hlt : a < v := Rat.lt_of_le_of_ne hs (fun h => hva h.symm)
        have hga : onGrid st a := onGrid_of_gridB (by simpa [Coord.onGridB] using hgx)
        have hstep := grid_succ_le hst hga hv hlt
        exact shift (tilesOn_cover hst hv hhi cs (.exactly b) hrest (show b ≤ v by rw [hb]; exact hstep))
    | .exactly a, .between (some b) e, hnext =>
      have hb : b = a := eq_of_beq hnext
      by_cases hva : v = a
      · exact ⟨0, .exactly a, rfl, hva⟩
      · have hlt : a < v := Rat.lt_of_le_of_ne hs (fun h => hva h.symm)
        exact shift (tilesOn_cover hst hv hhi cs (.between (some b) e) hrest (show b < v by rw [hb]; exact hlt))
    | .between lo (some a), .exactly b, hnext =>
      have hb : b = a := eq_of_beq hnext
      by_cases hva : v < a
      · refine ⟨0, .between lo (some a), rfl, ?_⟩
        refine ⟨fun l hl => ?_, fun u hu => ?_⟩
        · cases lo with
          | none => cases hl
          | some l' =>
            cases hl
            exact hs
        · cases hu
          exact hva
      · have hge : a ≤ v := Rat.not_lt.1 hva
        exact shift (tilesOn_cover hst hv hhi cs (.exactly b) hrest (show b ≤ v by rw [hb]; exact hge))

/-- **Every value of the range stands on a coordinate.** Where the tiling check passes, a value on the
    grid inside the range is held by one of the axis's coordinates. -/
theorem axisTilesB_cover {st : Rat} {lo hi : Option Rat} {cs : List Coord}
    (h : axisTilesB st lo hi cs = true) {v : Rat} (hv : onGrid st v)
    (hlo : ∀ l, lo = some l → l ≤ v) (hhi : ∀ u, hi = some u → v ≤ u) :
    ∃ (c : Nat) (x : Coord), cs[c]? = some x ∧ x.holds v := by
  simp only [axisTilesB, Bool.and_eq_true, decide_eq_true_eq] at h
  obtain ⟨⟨hst, hbeg⟩, htile⟩ := h
  match cs, hbeg with
  | x :: rest, hbeg =>
    refine tilesOn_cover hst hv hhi rest x htile ?_
    match lo, x, hbeg with
    | some l, .exactly a, hbeg =>
      have : a = l := eq_of_beq hbeg
      show a ≤ v
      rw [this]
      exact hlo l rfl
    | none, .between none _, _ => trivial

/-! ## What a pass settles: no value stands on two coordinates -/

/-- A value past where a coordinate begins, for the coordinates that come after it. -/
def aboveStart (x : Coord) (w : Rat) : Prop :=
  match x with
  | .exactly a => a ≤ w
  | .between (some a) _ => a < w
  | .between none _ => True

theorem holds_aboveStart {x : Coord} {w : Rat} (h : x.holds w) : aboveStart x w := by
  match x with
  | .exactly a =>
    show a ≤ w
    have : w = a := h
    rw [this]
    exact Rat.le_refl
  | .between (some a) _ => exact h.1 a rfl
  | .between none _ => trivial

/-- Every value a coordinate from here on holds is past where this one begins. -/
theorem tilesOn_aboveStart {st : Rat} (hst : 0 < st) {hi : Option Rat} :
    ∀ (cs : List Coord) (y : Coord), tilesOn st hi (y :: cs) = true →
      ∀ z ∈ y :: cs, ∀ w, z.holds w → aboveStart y w
  | [], y, _, z, hz, w, hw => by
    simp only [List.mem_singleton] at hz
    subst hz
    exact holds_aboveStart hw
  | y' :: cs, y, h, z, hz, w, hw => by
    simp only [tilesOn, Bool.and_eq_true] at h
    obtain ⟨⟨hgy, hnext⟩, hrest⟩ := h
    rcases List.mem_cons.1 hz with rfl | hz'
    · exact holds_aboveStart hw
    · have ih := tilesOn_aboveStart hst cs y' hrest z hz' w hw
      match y, y', hnext with
      | .exactly a, .exactly b, hnext =>
        have hb : b = a + st := eq_of_beq hnext
        have h1 : a + st ≤ w := hb ▸ ih
        show a ≤ w
        exact Rat.le_trans (Rat.le_of_lt (lt_add_pos hst)) h1
      | .exactly a, .between (some b) _, hnext =>
        have hb : b = a := eq_of_beq hnext
        have h1 : b < w := ih
        show a ≤ w
        rw [← hb]
        exact Rat.le_of_lt h1
      | .between lo (some a), .exactly b, hnext =>
        have hb : b = a := eq_of_beq hnext
        have h1 : b ≤ w := ih
        cases lo with
        | none => trivial
        | some l =>
          have hla : l < a := by
            have := hgy
            simp only [Coord.onGridB, Bool.and_eq_true, decide_eq_true_eq] at this
            exact this.2
          show l < w
          exact ltle hla (hb ▸ h1)
where
  lt_add_pos {a s : Rat} (hs : 0 < s) : a < a + s := by
    have := (Rat.add_lt_add_right (c := a)).2 hs
    rwa [Rat.zero_add, Rat.add_comm] at this

/-- Every value a coordinate after this one holds is above every value this one holds. -/
theorem tilesOn_before {st : Rat} (hst : 0 < st) {hi : Option Rat} {x : Coord} {cs : List Coord}
    (h : tilesOn st hi (x :: cs) = true) {v : Rat} (hv : x.holds v) :
    ∀ z ∈ cs, ∀ w, z.holds w → v < w := by
  intro z hz w hw
  match cs, hz, h with
  | y :: rest, hz, h =>
    simp only [tilesOn, Bool.and_eq_true] at h
    obtain ⟨⟨_, hnext⟩, hrest⟩ := h
    have hy := tilesOn_aboveStart hst rest y hrest z hz w hw
    match x, y, hnext with
    | .exactly a, .exactly b, hnext =>
      have hb : b = a + st := eq_of_beq hnext
      have hva : v = a := hv
      have h1 : a + st ≤ w := hb ▸ hy
      rw [hva]
      exact lelt (Rat.le_refl) (ltle (tilesOn_aboveStart.lt_add_pos hst) h1)
    | .exactly a, .between (some b) _, hnext =>
      have hb : b = a := eq_of_beq hnext
      have hva : v = a := hv
      have h1 : b < w := hy
      rw [hva, ← hb]
      exact h1
    | .between lo (some a), .exactly b, hnext =>
      have hb : b = a := eq_of_beq hnext
      have h1 : b ≤ w := hy
      exact ltle (hv.2 a rfl) (hb ▸ h1)

/-- **No value stands on two coordinates.** Where the coordinates from `x` on are side by side, the
    one that holds a value is one only. -/
theorem tilesOn_unique {st : Rat} (hst : 0 < st) {hi : Option Rat} :
    ∀ (cs : List Coord), tilesOn st hi cs = true →
      ∀ (c d : Nat) (x y : Coord) (v : Rat), cs[c]? = some x → x.holds v → cs[d]? = some y → y.holds v → c = d
  | [], h, _, _, _, _, _, _, _, _, _ => by simp [tilesOn] at h
  | x :: rest, h, c, d, y, z, v, hc, hy, hd, hz => by
    have hrest : rest ≠ [] → tilesOn st hi rest = true := by
      intro hne
      match rest, hne, h with
      | _ :: _, _, h =>
        simp only [tilesOn, Bool.and_eq_true] at h
        exact h.2
    cases c with
    | zero =>
      cases d with
      | zero => rfl
      | succ d =>
        simp only [List.getElem?_cons_zero, Option.some.injEq] at hc
        subst hc
        have hmem : z ∈ rest := List.mem_of_getElem? (by simpa using hd)
        exact absurd (tilesOn_before hst h hy z hmem v hz) (Rat.lt_irrefl)
    | succ c =>
      cases d with
      | zero =>
        simp only [List.getElem?_cons_zero, Option.some.injEq] at hd
        subst hd
        have hmem : y ∈ rest := List.mem_of_getElem? (by simpa using hc)
        exact absurd (tilesOn_before hst h hz y hmem v hy) (Rat.lt_irrefl)
      | succ d =>
        have hne : rest ≠ [] := by
          intro he
          subst he
          simp at hc
        have := tilesOn_unique hst rest (hrest hne) c d y z v (by simpa using hc) hy (by simpa using hd) hz
        rw [this]

theorem axisTilesB_unique {st : Rat} {lo hi : Option Rat} {cs : List Coord}
    (h : axisTilesB st lo hi cs = true) {c d : Nat} {x y : Coord} {v : Rat}
    (hc : cs[c]? = some x) (hx : x.holds v) (hd : cs[d]? = some y) (hy : y.holds v) : c = d := by
  simp only [axisTilesB, Bool.and_eq_true, decide_eq_true_eq] at h
  exact tilesOn_unique h.1.1 cs h.2 c d x y v hc hx hd hy

/-! ## What a walk answers stands on the axis that reads it (rulec's §15.236)

A table below reads what a walk leaves behind on an axis of kind `walk`, cut over the range the
certificate states for it, and the tiling check holds the axis to that range as it holds an input's to
the range the rule declares. `extreme_mem_stated` and `foldAnswer_mem_stated` put whatever the walk
answers inside the stated range; the tiling puts every value of the range on one coordinate. -/

/-- **A `max` or a `min` answers a value that stands on a coordinate of the axis that reads it**: the
    extremes check holds the stated range to the column's interval and the `empty` value, the axis tiles
    the stated range, and the answer — on the grid the axis is cut on — is held by one coordinate. -/
theorem extreme_on_axis {beats : Rat → Rat → Bool} {C S : Span2} {z : Rat} {xs : List (Bool × Rat)}
    {st : Rat} {cs : List Coord}
    (hex : extremeOk C z S = true) (hxs : ∀ x ∈ xs, x.1 = true → inSpan C x.2)
    (htile : axisTilesB st (some S.1) (some S.2) cs = true)
    (hgrid : onGrid st (extremeOf beats z xs)) :
    ∃ (c : Nat) (x : Coord), cs[c]? = some x ∧ x.holds (extremeOf beats z xs) := by
  obtain ⟨h1, h2⟩ := extreme_mem_stated (beats := beats) hex hxs
  exact axisTilesB_cover htile hgrid (fun l hl => by cases hl; exact h1) (fun u hu => by cases hu; exact h2)

/-- **A fold's answer stands on a coordinate of the axis that reads it**: the range check holds the
    stated range to every value the fold answers with, the axis tiles the stated range, and the answer —
    on the grid the axis is cut on — is held by one coordinate. -/
theorem foldAnswer_on_axis {ranges : String → Option Span2} {within : Expr → Expr → Bool}
    {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd) {S : Span2} {answers : List Expr}
    (hS : writesWithin ranges within S (answers.map (fun e => ({ expr := e } : Writes))) = true)
    {env0 : String → Option UVal} (hg0 : GoodEnv ranges within rnd env0) {empty : Expr}
    {exhausted : Option Expr} (hempty : empty ∈ answers) (hexh : ∀ e, exhausted = some e → e ∈ answers)
    {steps : List (FoldStep × (String → Option UVal))}
    (hgood : ∀ p ∈ steps, GoodEnv ranges within rnd p.2)
    (hans : ∀ p ∈ steps, ∀ e, p.1 = .hold e ∨ p.1 = .stop (some e) → e ∈ answers)
    {v : Rat} {t : Ty} (h : foldAnswer rnd env0 empty exhausted steps = some (v, t))
    {st : Rat} {cs : List Coord} (htile : axisTilesB st (some S.1) (some S.2) cs = true)
    (hgrid : onGrid st v) :
    ∃ (c : Nat) (x : Coord), cs[c]? = some x ∧ x.holds v := by
  obtain ⟨h1, h2⟩ := foldAnswer_mem_stated hrnd hS hg0 hempty hexh hgood hans h
  exact axisTilesB_cover htile hgrid (fun l hl => by cases hl; exact h1) (fun u hu => by cases hu; exact h2)

/-! ## A point made of the coordinates the values stand on -/

/-- One coordinate per axis, each below its axis's arity, is a point of the space. -/
theorem inSpace_of_lt : ∀ {ns : List Arity} {p : Point}, p.length = ns.length →
    (∀ (i c n : Nat), p[i]? = some c → ns[i]? = some n → c < n) → inSpace ns p = true
  | [], [], _, _ => rfl
  | [], _ :: _, hl, _ => by simp at hl
  | _ :: _, [], hl, _ => by simp at hl
  | n :: ns, c :: cs, hl, h => by
    simp only [inSpace, Bool.and_eq_true, decide_eq_true_eq]
    exact ⟨h 0 c n rfl rfl, inSpace_of_lt (by simpa using hl) (fun i c' n' hc hn => h (i + 1) c' n' (by simpa using hc) (by simpa using hn))⟩

/-- **The table answers a call whose values stand on its axes.** Where the cover check passes, a point
    made of a coordinate of each axis — the one each column's value stands on, which the tiling gives —
    is a point of the space, and a row takes it wherever the rule is asked about it. -/
theorem Certified.answers_at (C : Certified) (hc : C.coverChecks = true) {p : Point}
    (hlen : p.length = C.arities.length)
    (hlt : ∀ (i c n : Nat), p[i]? = some c → C.arities[i]? = some n → c < n)
    (hask : C.sieve.asked p) : C.table.firing p ≠ [] :=
  C.complete hc p (inSpace_of_lt hlen hlt) hask

end RulecCert
