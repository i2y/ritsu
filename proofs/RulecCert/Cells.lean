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
import RulecCert.Values

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
  /-- `>=medieval <industrial` on a column of an ordered enum (rulec's §15.210): the enum's
      values in their order, and each comparison with the place of the value it is against. -/
  | rank : List String → List (Cmp × Nat) → CellTest
  deriving Repr, Inhabited

/-- One comparison between two places in an ordered enum's order. -/
def rankHolds : Cmp → Nat → Nat → Bool
  | .le, x, y => decide (x ≤ y)
  | .lt, x, y => decide (x < y)
  | .ge, x, y => decide (y ≤ x)
  | .gt, x, y => decide (y < x)
  | .eq, x, y => decide (x = y)

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
      -- The absent value of an optional input is none of the values, so `not:` takes it
      -- (rulec's §15.201).
      | some none => labels[c]? == some "none"
      | _ => false)
  -- A value is taken when it has a place in the order and the place passes every comparison;
  -- the absent value has none (rulec's §15.210).
  | .rank order ts => (List.range labels.length).filter (fun c =>
      match labels[c]? with
      | some l => l != "none" && (match order.idxOf? l with
          | some i => ts.all (fun t => rankHolds t.1 i t.2)
          | none => false)
      | none => false)

/-! ## The absent value

An optional input (`T?`) takes one value more than the type it wraps: none at all, which the
cell `none` tests. On the axis it is the first coordinate, written `none`; on a numeric axis
it stands for no number (rulec's §15.201). -/

/-- Whether a cell takes the absent value: `-` and `none` do, `not:` does (the absent value is
    none of the values it names), a set of words does when it names `none`, and a comparison,
    a set of numbers and a prefix do not — they are about a value that is there. This is how
    rulec's reference evaluator reads a cell (its `eval.rs`). -/
def CellTest.takesAbsent : CellTest → Bool
  | .any => true
  | .nothing => true
  | .isIn ws => ws.contains "none"
  | .notIn ws => !ws.contains "none"
  | .cmp _ => false
  | .prefixOf _ => false
  | .inVals _ => false
  | .notInVals _ => true
  | .rank _ _ => false

/-- **The box says what the cell says of the absent value.** On an axis that starts with the
    absent value — written `none`, standing for no number — the first coordinate is in a
    cell's box exactly when the cell takes the absent value. -/
theorem mem_boxOf_absent_iff {labels : List String} {coords : List (Option Coord)}
    (hl : labels[0]? = some "none") (hc : coords[0]? = some none) (t : CellTest)
    (ht : ∀ ps, t ≠ .prefixOf ps) :
    0 ∈ boxOf labels coords t ↔ t.takesAbsent = true := by
  have hlen : 0 < labels.length := by
    cases labels with
    | nil => simp at hl
    | cons _ _ => simp
  have h0 : labels[0]'hlen = "none" := by
    rw [List.getElem?_eq_getElem hlen] at hl
    exact Option.some.inj hl
  cases t with
  | any => simp [boxOf, CellTest.takesAbsent, hlen]
  | nothing => simp [boxOf, CellTest.takesAbsent, hl]
  | isIn ws => simp [boxOf, CellTest.takesAbsent, hlen, h0]
  | notIn ws => simp [boxOf, CellTest.takesAbsent, hlen, h0]
  | cmp ts => simp [boxOf, CellTest.takesAbsent, hc]
  | prefixOf ps => exact absurd rfl (ht ps)
  | inVals ws => simp [boxOf, CellTest.takesAbsent, hc]
  | notInVals ws => simp [boxOf, CellTest.takesAbsent, hlen, hc, h0]
  | rank o ts => simp [boxOf, CellTest.takesAbsent, hl]

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

/-- **A comparison on an ordered enum takes the values whose place passes it** (rulec's
    §15.210). The coordinate of a value is in the cell's box exactly when the value has a place
    in the enum's order and that place satisfies every comparison the cell writes — `<medieval`
    takes the coordinates of the values whose place is before medieval's. -/
theorem mem_boxOf_rank_iff {labels : List String} {coords : List (Option Coord)}
    {order : List String} {ts : List (Cmp × Nat)} {c : Nat} {l : String}
    (hl : labels[c]? = some l) (hn : l ≠ "none") :
    c ∈ boxOf labels coords (.rank order ts) ↔
      ∃ i, order.idxOf? l = some i ∧ ∀ t ∈ ts, rankHolds t.1 i t.2 = true := by
  have hlen : c < labels.length := lt_of_getElem? hl
  have hne : (l != "none") = true := by simpa [bne_iff_ne] using hn
  simp only [boxOf, List.mem_filter, List.mem_range, hlen, true_and, hl, hne, Bool.true_and]
  cases h : order.idxOf? l with
  | none => simp
  | some i =>
    simp only [List.all_eq_true]
    constructor
    · intro hall
      exact ⟨i, rfl, hall⟩
    · rintro ⟨j, hj, hall⟩
      cases hj
      exact hall

/-! ## What a row's own cell lets through the value it writes (rulec's §15.213)

A row that writes one of its own table's columns — `| >=1 | units |` writes `units` — answers only
at the points its box takes, and its box on that column is the cell's (`checkBoxes` in
`RulecMain.lean` holds the two together). So the value it writes is one its cell lets through. That
is the row's **cut**: the bounds of a comparison cell, a strict end moved in to the next point of the
grid the value sits on, or the ends of a set of numbers. -/

/-- The bounds one comparison puts on the value it compares, where that value sits on the grid `g`:
    `>w` lets through the grid's first point above `w`, `<w` its last point below it. -/
def cmpBound (g : Rat) : Cmp → Rat → Ival
  | .ge, w => (some w, none)
  | .gt, w => (some (floorTo g w + g), none)
  | .le, w => (none, some w)
  | .lt, w => (none, some (ceilTo g w - g))
  | .eq, w => (some w, some w)

/-- The tighter of two low ends, and of two high ends. -/
def meetLo : Option Rat → Option Rat → Option Rat
  | some a, some b => some (rmax a b)
  | some a, none => some a
  | none, b => b

def meetHi : Option Rat → Option Rat → Option Rat
  | some a, some b => some (rmin a b)
  | some a, none => some a
  | none, b => b

theorem inIval_meet {I J : Ival} {v : Rat} (hI : inIval I v) (hJ : inIval J v) :
    inIval (meetLo I.1 J.1, meetHi I.2 J.2) v := by
  obtain ⟨hI1, hI2⟩ := hI
  obtain ⟨hJ1, hJ2⟩ := hJ
  constructor
  · intro a ha
    cases h1 : I.1 with
    | none => simp only [meetLo, h1] at ha; exact hJ1 a ha
    | some x =>
      cases h2 : J.1 with
      | none =>
        simp only [meetLo, h1, h2, Option.some.injEq] at ha
        exact ha ▸ hI1 x h1
      | some y =>
        simp only [meetLo, h1, h2, Option.some.injEq] at ha
        exact ha ▸ rmax_le (hI1 x h1) (hJ1 y h2)
  · intro b hb
    cases h1 : I.2 with
    | none => simp only [meetHi, h1] at hb; exact hJ2 b hb
    | some x =>
      cases h2 : J.2 with
      | none =>
        simp only [meetHi, h1, h2, Option.some.injEq] at hb
        exact hb ▸ hI2 x h1
      | some y =>
        simp only [meetHi, h1, h2, Option.some.injEq] at hb
        exact hb ▸ le_rmin (hI2 x h1) (hJ2 y h2)

/-- The bounds a comparison cell puts on its value: all of its comparisons' together. -/
def cellBounds (g : Rat) : List (Cmp × Rat) → Ival
  | [] => (none, none)
  | t :: ts =>
    let b := cmpBound g t.1 t.2
    let r := cellBounds g ts
    (meetLo b.1 r.1, meetHi b.2 r.2)

theorem inIval_cmpBound {g v w : Rat} {op : Cmp} (hg : 0 < g) (hv : onGrid g v)
    (h : op.holds v w) : inIval (cmpBound g op w) v := by
  cases op with
  | ge =>
    refine ⟨fun a ha => ?_, fun b hb => by simp [cmpBound] at hb⟩
    simp only [cmpBound, Option.some.injEq] at ha
    exact ha ▸ h
  | gt =>
    refine ⟨fun a ha => ?_, fun b hb => by simp [cmpBound] at hb⟩
    simp only [cmpBound, Option.some.injEq] at ha
    exact ha ▸ grid_above hg hv h
  | le =>
    refine ⟨fun a ha => by simp [cmpBound] at ha, fun b hb => ?_⟩
    simp only [cmpBound, Option.some.injEq] at hb
    exact hb ▸ h
  | lt =>
    refine ⟨fun a ha => by simp [cmpBound] at ha, fun b hb => ?_⟩
    simp only [cmpBound, Option.some.injEq] at hb
    exact hb ▸ grid_below hg hv h
  | eq =>
    have h' : v = w := h
    refine ⟨fun a ha => ?_, fun b hb => ?_⟩
    · simp only [cmpBound, Option.some.injEq] at ha
      rw [← ha, h']
      exact Rat.le_refl
    · simp only [cmpBound, Option.some.injEq] at hb
      rw [← hb, h']
      exact Rat.le_refl

/-- **A value on its grid that passes every comparison of a cell lies inside the cell's bounds.** -/
theorem inIval_cellBounds {g v : Rat} (hg : 0 < g) (hv : onGrid g v) :
    ∀ {ts : List (Cmp × Rat)}, (∀ t ∈ ts, t.1.holds v t.2) → inIval (cellBounds g ts) v
  | [], _ => ⟨fun a ha => by simp [cellBounds] at ha, fun b hb => by simp [cellBounds] at hb⟩
  | t :: ts, h =>
    inIval_meet (inIval_cmpBound hg hv (h t (by simp)))
      (inIval_cellBounds hg hv (fun t' ht' => h t' (List.mem_cons_of_mem _ ht')))

/-- The ends of a set of numbers. -/
def valsBounds : List Rat → Ival
  | [] => (none, none)
  | w :: ws =>
    match valsBounds ws with
    | (some a, some b) => (some (rmin w a), some (rmax w b))
    | _ => (some w, some w)

theorem valsBounds_ends : ∀ (ws : List Rat), ws ≠ [] →
    ∃ a b, valsBounds ws = (some a, some b) ∧ ∀ w ∈ ws, a ≤ w ∧ w ≤ b
  | [], h => absurd rfl h
  | [w], _ => ⟨w, w, rfl, fun x hx => by
      simp only [List.mem_singleton] at hx
      subst hx
      exact ⟨Rat.le_refl, Rat.le_refl⟩⟩
  | w :: w2 :: ws, _ => by
    obtain ⟨a, b, hab, hall⟩ := valsBounds_ends (w2 :: ws) (by simp)
    have hv : valsBounds (w :: w2 :: ws) = match valsBounds (w2 :: ws) with
        | (some a, some b) => (some (rmin w a), some (rmax w b))
        | _ => (some w, some w) := rfl
    refine ⟨rmin w a, rmax w b, by rw [hv, hab], fun x hx => ?_⟩
    simp only [List.mem_cons] at hx
    rcases hx with rfl | hx
    · exact ⟨rmin_le_left _ _, left_le_rmax _ _⟩
    · obtain ⟨h1, h2⟩ := hall x (by simpa [List.mem_cons] using hx)
      exact ⟨Rat.le_trans (rmin_le_right _ _) h1, Rat.le_trans h2 (right_le_rmax _ _)⟩

/-- **A value that is one of a set's lies inside the set's ends.** -/
theorem inIval_valsBounds {ws : List Rat} {v : Rat} (h : ∃ w ∈ ws, v = w) : inIval (valsBounds ws) v := by
  obtain ⟨w, hw, rfl⟩ := h
  obtain ⟨a, b, hab, hall⟩ := valsBounds_ends ws (List.ne_nil_of_mem hw)
  rw [hab]
  obtain ⟨h1, h2⟩ := hall _ hw
  exact ⟨fun x hx => by simp only [Option.some.injEq] at hx; exact hx ▸ h1,
    fun x hx => by simp only [Option.some.injEq] at hx; exact hx ▸ h2⟩

/-- The cut a row's cell on a column of numbers puts on the value there, on the grid `g` the value
    sits on: a comparison's bounds or a set's ends. Other cells cut nothing. -/
def cellCut (g : Rat) : CellTest → Option Ival
  | .cmp ts => some (cellBounds g ts)
  | .inVals ws => some (valsBounds ws)
  | _ => none

/-- **The cut holds at every point the row's box takes** (rulec's §15.213). Where the coordinate the
    point has on the column is one the cell's box takes, the value behind it — a value on its grid
    — is inside the cut: being in the box is passing the cell (`mem_boxOf_cmp_iff`,
    `mem_boxOf_in_iff`), and passing the cell is being inside its bounds. -/
theorem cut_of_box {labels : List String} {coords : List (Option Coord)} {cell : CellTest}
    {c : Nat} {x : Coord} {v g : Rat} {J : Ival} (hcut : cellCut g cell = some J)
    (hlen : c < labels.length) (hc : coords[c]? = some (some x))
    (hsplit : axisSplits coords cell = true) (hv : x.holds v)
    (hbox : c ∈ boxOf labels coords cell) (hg : 0 < g) (hgrid : onGrid g v) : inIval J v := by
  cases cell with
  | cmp ts =>
    simp only [cellCut, Option.some.injEq] at hcut
    subst hcut
    exact inIval_cellBounds hg hgrid ((mem_boxOf_cmp_iff hlen hc hsplit hv).1 hbox)
  | inVals ws =>
    simp only [cellCut, Option.some.injEq] at hcut
    subst hcut
    exact inIval_valsBounds ((mem_boxOf_in_iff hlen hc hsplit hv).1 hbox)
  | any => simp [cellCut] at hcut
  | nothing => simp [cellCut] at hcut
  | isIn _ => simp [cellCut] at hcut
  | notIn _ => simp [cellCut] at hcut
  | prefixOf _ => simp [cellCut] at hcut
  | notInVals _ => simp [cellCut] at hcut
  | rank _ _ => simp [cellCut] at hcut

/-- **What a table writes into a column of numbers lies in the hull, at every point one of its rows
    fires** (rulec's §15.196, §15.213). The row that fires writes its expression's value; where it
    writes one of its own table's columns, that column's value is behind the point's coordinate on
    it, sits on its grid, and the coordinate is one the row's cell takes — so it is inside the row's
    cut, and `eval_mem_hull` applies. -/
theorem written_mem_hull {ranges : String → Option Span2} {env : String → Option UVal}
    {within : Expr → Expr → Bool} {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd)
    (henv : ∀ n I w t, ranges n = some I → env n = some (w, t) → inSpan I w)
    (hwithin : ∀ a b va vb ta tb, within a b = true →
      eval rnd env a = some (va, ta) → eval rnd env b = some (vb, tb) → va ≤ vb)
    {ws : List Writes} {H : Span2} (hH : hullOf ranges within ws = some H)
    {w : Writes} (hw : w ∈ ws) {v : Rat} {t : Ty} (hv : eval rnd env w.expr = some (v, t))
    (hfires : ∀ J, w.cut = some J → ∃ (labels : List String) (coords : List (Option Coord))
      (cell : CellTest) (c : Nat) (x : Coord) (g : Rat), cellCut g cell = some J ∧
      c < labels.length ∧ coords[c]? = some (some x) ∧ axisSplits coords cell = true ∧
      x.holds v ∧ c ∈ boxOf labels coords cell ∧ 0 < g ∧ onGrid g v) :
    inSpan H v := by
  refine eval_mem_hull hrnd henv hwithin ws H hH w hw v t hv (fun J hJ => ?_)
  obtain ⟨labels, coords, cell, c, x, g, hcut, hlen, hc, hsplit, hx, hbox, hg, hgrid⟩ := hfires J hJ
  exact cut_of_box hcut hlen hc hsplit hx hbox hg hgrid

/-! ## The axis of an input of an enum (rulec's §15.210)

The cover tiles the space of coordinates, so completeness and every other claim speak of the values
an axis has a coordinate for. For an input of an enum those have to be the values the input can take:
the enum's, or the ones an ordered enum's `range` lets in, after the absent value of an optional
input. A value left off the axis would be one no row was asked to answer; a value on it the door
turns away would be a point a row is "reached" at that no call makes. -/

/-- The words an input of an enum takes, in order: the absent value first where the input is
    optional, then the enum's values — those its range lets in, where it declares one. -/
def enumAxis (vals : List String) (range : Option (List String)) (optional : Bool) : List String :=
  (if optional then ["none"] else []) ++
    (match range with
     | some r => vals.filter (fun w => r.contains w)
     | none => vals)

/-- A word the input can take. -/
def EnumTakes (vals : List String) (range : Option (List String)) (optional : Bool) (w : String) :
    Prop :=
  (optional = true ∧ w = "none") ∨ (w ∈ vals ∧ ∀ r, range = some r → w ∈ r)

/-- **The axis written as `enumAxis` has a coordinate for exactly the words the input takes.** -/
theorem mem_enumAxis_iff {vals : List String} {range : Option (List String)} {optional : Bool}
    {w : String} : w ∈ enumAxis vals range optional ↔ EnumTakes vals range optional w := by
  unfold enumAxis EnumTakes
  cases optional <;> cases range <;> simp [List.mem_filter]

/-- **Every word the input takes is a coordinate of its axis, and every coordinate is one it
    takes**, when the axis is written as `enumAxis` says. -/
theorem enumAxis_coords {labels vals : List String} {range : Option (List String)} {optional : Bool}
    (h : labels = enumAxis vals range optional) :
    (∀ w, EnumTakes vals range optional w → ∃ c : Nat, labels[c]? = some w) ∧
    (∀ (c : Nat) (w : String), labels[c]? = some w → EnumTakes vals range optional w) := by
  subst h
  refine ⟨fun w hw => List.mem_iff_getElem?.1 (mem_enumAxis_iff.2 hw), fun c w hc => ?_⟩
  exact mem_enumAxis_iff.1 (List.mem_of_getElem? hc)

end RulecCert
