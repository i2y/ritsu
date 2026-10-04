/-
  From the cells a rule writes to the boxes the claims are about (§6.2, DESIGN §15.97).

  Everything in `RulecCert.Sound` is stated over boxes: sets of coordinates, one set per
  axis. A rule is not written that way — it is written as cells, `<=2000g` and `not: 遠隔地`
  — and §6.2 compresses a column to the boundary values its cells compare against. This
  file is where that compression is shown to be faithful, and it is what makes the five
  claims claims about the rule and not only about its compressed shadow.

  The case that needs a proof is a comparison against a numeric column. A coordinate there
  stands for a whole open interval, and "the row takes this coordinate" is only the same as
  "the row takes every value in it" because **the value compared against is itself a
  boundary**: nothing a cell mentions falls strictly inside a coordinate. That is a
  property of the axis the certificate carries, so `splitsAt` below checks it rather than
  assuming it, and `admits_iff` is the theorem that rests on it.
-/
import RulecCert.Sieve

namespace RulecCert

/-- Whether the value a cell compares against falls outside this coordinate, so that the
    comparison is either true throughout it or false throughout it. -/
def splitsAt (x : Coord) (w : Rat) : Bool :=
  match x with
  | .exactly _ => true
  | .between lo hi =>
    (match hi with | some b => decide (b ≤ w) | none => false) ||
    (match lo with | some a => decide (w ≤ a) | none => false)

/-- Whether a whole coordinate satisfies one comparison. -/
def admitsCmp (x : Coord) (op : Cmp) (w : Rat) : Bool :=
  match x with
  | .exactly v => cmpHolds op v w
  | .between lo hi =>
    match op with
    | .le | .lt => (match hi with | some b => decide (b ≤ w) | none => false)
    | .ge | .gt => (match lo with | some a => decide (w ≤ a) | none => false)
    -- A whole interval is never one value, and `splitsAt` puts that value outside it.
    | .eq => false

/-- **The compression is faithful.** Where the value compared against is a boundary of the
    axis, "the coordinate is admitted" and "the value satisfies the comparison" are the
    same statement — so a box read off the coordinates says exactly what the cell says. -/
theorem admits_iff {x : Coord} {v w : Rat} {op : Cmp}
    (hsplit : splitsAt x w = true) (hv : x.holds v) :
    admitsCmp x op w = true ↔ op.holds v w := by
  cases x with
  | exactly u =>
    subst hv
    cases op <;> simp [admitsCmp, cmpHolds, Cmp.holds]
  | between lo hi =>
    obtain ⟨hlo, hhi⟩ := hv
    simp only [splitsAt, Bool.or_eq_true] at hsplit
    cases op
    · -- v ≤ w
      simp only [admitsCmp, Cmp.holds]
      constructor
      · intro h
        revert h
        cases hb : hi with
        | none => intro h; simp at h
        | some b => intro h; simp only [decide_eq_true_eq] at h; exact Rat.le_of_lt (ltle (hhi b hb) h)
      · intro hvw
        rcases hsplit with h | h
        · revert h; cases hb : hi with
          | none => intro h; simp at h
          | some b => intro h; simpa using h
        · revert h; cases ha : lo with
          | none => intro h; simp at h
          | some a =>
            intro h
            simp only [decide_eq_true_eq] at h
            exact absurd (Rat.le_trans hvw h) (Rat.not_le.2 (hlo a ha))
    · -- v < w
      simp only [admitsCmp, Cmp.holds]
      constructor
      · intro h
        revert h
        cases hb : hi with
        | none => intro h; simp at h
        | some b => intro h; simp only [decide_eq_true_eq] at h; exact ltle (hhi b hb) h
      · intro hvw
        rcases hsplit with h | h
        · revert h; cases hb : hi with
          | none => intro h; simp at h
          | some b => intro h; simpa using h
        · revert h; cases ha : lo with
          | none => intro h; simp at h
          | some a =>
            intro h
            simp only [decide_eq_true_eq] at h
            exact absurd (Rat.le_of_lt (ltle hvw h)) (Rat.not_le.2 (hlo a ha))
    · -- w ≤ v
      simp only [admitsCmp, Cmp.holds]
      constructor
      · intro h
        revert h
        cases ha : lo with
        | none => intro h; simp at h
        | some a => intro h; simp only [decide_eq_true_eq] at h; exact Rat.le_of_lt (lelt h (hlo a ha))
      · intro hwv
        rcases hsplit with h | h
        · revert h; cases hb : hi with
          | none => intro h; simp at h
          | some b =>
            intro h
            simp only [decide_eq_true_eq] at h
            exact absurd (Rat.le_trans h hwv) (Rat.not_le.2 (hhi b hb))
        · revert h; cases ha : lo with
          | none => intro h; simp at h
          | some a => intro h; simpa using h
    · -- w < v
      simp only [admitsCmp, Cmp.holds]
      constructor
      · intro h
        revert h
        cases ha : lo with
        | none => intro h; simp at h
        | some a => intro h; simp only [decide_eq_true_eq] at h; exact lelt h (hlo a ha)
      · intro hwv
        rcases hsplit with h | h
        · revert h; cases hb : hi with
          | none => intro h; simp at h
          | some b =>
            intro h
            simp only [decide_eq_true_eq] at h
            exact absurd (Rat.le_of_lt (lelt h hwv)) (Rat.not_le.2 (hhi b hb))
        · revert h; cases ha : lo with
          | none => intro h; simp at h
          | some a => intro h; simpa using h
    · -- v = w
      simp only [admitsCmp, Cmp.holds]
      constructor
      · intro h; exact absurd h (by simp)
      · intro hvw
        subst hvw
        rcases hsplit with h | h
        · revert h; cases hb : hi with
          | none => intro h; simp at h
          | some b =>
            intro h
            simp only [decide_eq_true_eq] at h
            exact absurd h (Rat.not_le.2 (hhi b hb))
        · revert h; cases ha : lo with
          | none => intro h; simp at h
          | some a =>
            intro h
            simp only [decide_eq_true_eq] at h
            exact absurd h (Rat.not_le.2 (hlo a ha))

/-! ## The box a cell describes

One cell of one row, read on its own column. The box a certificate states for a row is
held to this — a box widened without touching the cell it was read from is caught here and
nowhere else. -/

/-- A cell, as far as the box depends on it. -/
inductive CellTest where
  | any
  | nothing
  | isIn : List String → CellTest
  | notIn : List String → CellTest
  | cmp : List (Cmp × Rat) → CellTest
  /-- `starts_with "ABC"` on a column of strings (§15.101). A finite set of prefixes cuts
      the strings into finitely many classes, which is all §6.2 asks of a column. -/
  | prefixOf : List (List Char) → CellTest
  /-- `100, 200` on a column of numbers: the values, each a point of the axis (§15.143). -/
  | inVals : List Rat → CellTest
  /-- `not: 100, 200`. -/
  | notInVals : List Rat → CellTest
  deriving Repr, Inhabited

/-- The coordinates a cell takes. An axis arrives as its labels — what each coordinate is
    written as — and, where it is numeric, the values each stands for. -/
def boxOf (labels : List String) (coords : List (Option Coord)) : CellTest → List Nat
  | .any => List.range labels.length
  | .nothing => if labels[0]? == some "none" then [0] else []
  | .isIn ws => (List.range labels.length).filter (fun c =>
      match labels[c]? with | some l => ws.contains l | none => false)
  | .notIn ws => (List.range labels.length).filter (fun c =>
      match labels[c]? with | some l => !ws.contains l | none => false)
  | .cmp ts => (List.range labels.length).filter (fun c =>
      match coords[c]? with
      | some (some x) => ts.all (fun t => admitsCmp x t.1 t.2)
      | _ => false)
  | .prefixOf _ => []
  | .inVals ws => (List.range labels.length).filter (fun c =>
      match coords[c]? with
      | some (some x) => ws.any (fun w => admitsCmp x .eq w)
      | _ => false)
  | .notInVals ws => (List.range labels.length).filter (fun c =>
      match coords[c]? with
      | some (some x) => !ws.any (fun w => admitsCmp x .eq w)
      | _ => false)

/-! Every value a cell compares against falls outside every coordinate of the axis, so no
    coordinate is split by it. This is §6.2's construction, checked rather than assumed. -/
def axisSplits (coords : List (Option Coord)) : CellTest → Bool
  | .cmp ts => coords.all (fun oc =>
      match oc with
      | some x => ts.all (fun t => splitsAt x t.2)
      | none => true)
  | .inVals ws | .notInVals ws => coords.all (fun oc =>
      match oc with
      | some x => ws.all (fun w => splitsAt x w)
      | none => true)
  | _ => true

/-- The coordinates a cell on a string column takes: the ones whose own prefix extends one
    the cell names. -/
def boxOfPrefix (prefixes : List (Option (List Char))) (ws : List (List Char)) : List Nat :=
  (List.range prefixes.length).filter (fun c =>
    match prefixes[c]? with
    | some (some p) => ws.any (fun w => w.isPrefixOf p)
    | _ => false)

/-- Every prefix the cell names is itself a coordinate of the axis. That is how §6.2 builds
    the axis — the coordinates *are* the prefixes the cells name — and it is what makes
    "the coordinate is taken" and "every string in it satisfies the cell" the same
    statement. Checked, not assumed. -/
def axisCovers (prefixes : List (Option (List Char))) (ws : List (List Char)) : Bool :=
  ws.all (fun w => prefixes.any (fun p => p == some w))

/-- **A prefix cell's box says exactly what the cell says.** For a string that sits in
    coordinate `c` — it starts with that coordinate's prefix, and no longer coordinate
    covers it — being in the box and starting with one of the cell's prefixes are the same
    thing. -/
theorem mem_boxOf_prefix_iff {prefixes : List (Option (List Char))} {ws : List (List Char)}
    {c : Nat} {p s : List Char}
    (hc : prefixes[c]? = some (some p)) (hcov : axisCovers prefixes ws = true)
    (hs : p <+: s)
    (hlong : ∀ (i : Nat) (q : List Char), prefixes[i]? = some (some q) → q <+: s →
      q.length ≤ p.length) :
    c ∈ boxOfPrefix prefixes ws ↔ ws.any (fun w => w.isPrefixOf s) = true := by
  have hlen : c < prefixes.length := lt_of_getElem? hc
  simp only [boxOfPrefix, List.mem_filter, List.mem_range, hc, hlen, true_and,
    List.any_eq_true]
  constructor
  · rintro ⟨w, hw, hwp⟩
    exact ⟨w, hw, by simpa using (List.isPrefixOf_iff_prefix.1 (by simpa using hwp)).trans hs⟩
  · rintro ⟨w, hw, hws⟩
    have hws : w <+: s := List.isPrefixOf_iff_prefix.1 (by simpa using hws)
    -- `w` is one of the axis's own coordinates, so it cannot be longer than the one `s`
    -- actually sits in.
    have hmem : some w ∈ prefixes := by
      have := List.all_eq_true.1 hcov w hw
      obtain ⟨q, hq, hqw⟩ := List.any_eq_true.1 this
      have : q = some w := by simpa using hqw
      exact this ▸ hq
    obtain ⟨i, hi⟩ := List.getElem?_of_mem hmem
    exact ⟨w, hw, by
      simpa using List.isPrefixOf_iff_prefix.2 (List.prefix_of_prefix_length_le hws hs (hlong i w hi hws))⟩

/-- The values a set cell names split no coordinate of its axis. -/
theorem splits_of_axisSplits_vals {coords : List (Option Coord)} {ws : List Rat} {c : Nat} {x : Coord}
    (hc : coords[c]? = some (some x))
    (hsplit : coords.all (fun oc => match oc with | some x => ws.all (fun w => splitsAt x w) | none => true) = true) :
    ∀ w ∈ ws, splitsAt x w = true := by
  intro w hw
  have hmem : some x ∈ coords := by
    have := List.mem_of_getElem? hc
    simpa using this
  exact List.all_eq_true.1 (List.all_eq_true.1 hsplit (some x) hmem) w hw

/-- **A set cell's box says exactly what the cell says.** For a coordinate whose values the
    axis knows, being in the box and being one of the cell's values are the same thing. -/
theorem mem_boxOf_in_iff {labels : List String} {coords : List (Option Coord)}
    {ws : List Rat} {c : Nat} {x : Coord} {v : Rat}
    (hlen : c < labels.length) (hc : coords[c]? = some (some x))
    (hsplit : axisSplits coords (.inVals ws) = true) (hv : x.holds v) :
    c ∈ boxOf labels coords (.inVals ws) ↔ ∃ w ∈ ws, v = w := by
  have hx := splits_of_axisSplits_vals hc hsplit
  simp only [boxOf, List.mem_filter, List.mem_range, hc, hlen, true_and, List.any_eq_true]
  constructor
  · rintro ⟨w, hw, h⟩
    exact ⟨w, hw, (admits_iff (hx w hw) hv).1 h⟩
  · rintro ⟨w, hw, h⟩
    exact ⟨w, hw, (admits_iff (hx w hw) hv).2 h⟩

/-- And its complement: in the box exactly when the value is none of the cell's. -/
theorem mem_boxOf_notIn_iff {labels : List String} {coords : List (Option Coord)}
    {ws : List Rat} {c : Nat} {x : Coord} {v : Rat}
    (hlen : c < labels.length) (hc : coords[c]? = some (some x))
    (hsplit : axisSplits coords (.notInVals ws) = true) (hv : x.holds v) :
    c ∈ boxOf labels coords (.notInVals ws) ↔ ∀ w ∈ ws, v ≠ w := by
  have hx := splits_of_axisSplits_vals hc hsplit
  simp only [boxOf, List.mem_filter, List.mem_range, hc, hlen, true_and, Bool.not_eq_true',
    List.any_eq_false]
  constructor
  · intro h w hw hvw
    exact h w hw ((admits_iff (hx w hw) hv).2 hvw)
  · intro h w hw hadm
    exact h w hw ((admits_iff (hx w hw) hv).1 hadm)

/-- **A comparison cell's box says exactly what the cell says.** For a coordinate whose
    values the axis knows, being in the box and satisfying every comparison in the cell are
    the same thing. -/
theorem mem_boxOf_cmp_iff {labels : List String} {coords : List (Option Coord)}
    {ts : List (Cmp × Rat)} {c : Nat} {x : Coord} {v : Rat}
    (hlen : c < labels.length) (hc : coords[c]? = some (some x))
    (hsplit : axisSplits coords (.cmp ts) = true) (hv : x.holds v) :
    c ∈ boxOf labels coords (.cmp ts) ↔ ∀ t ∈ ts, t.1.holds v t.2 := by
  have hx : ∀ t ∈ ts, splitsAt x t.2 = true := by
    intro t ht
    have hmem : some x ∈ coords := by
      have := List.mem_of_getElem? hc
      simpa using this
    exact List.all_eq_true.1 (List.all_eq_true.1 hsplit (some x) hmem) t ht
  simp only [boxOf, List.mem_filter, List.mem_range, hc, hlen, true_and]
  constructor
  · intro h t ht
    exact (admits_iff (hx t ht) hv).1 (List.all_eq_true.1 h t ht)
  · intro h
    refine List.all_eq_true.2 ?_
    intro t ht
    exact (admits_iff (hx t ht) hv).2 (h t ht)

end RulecCert
