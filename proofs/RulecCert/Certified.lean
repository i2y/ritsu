/-
  One table's certificate, and what follows from it (DESIGN §15.97).

  `RulecCert.Sound` states each theorem against the check it belongs to; this file puts one
  table's worth of them together, so that a single `true` from `Certified.checks` is a
  statement about the table itself. It is also the shape `RulecMain` builds from the JSON: what
  the program reads becomes a `Certified`, and what it prints is which of the three
  theorems below apply to it.
-/
import RulecCert.Sieve

namespace RulecCert

/-- Everything one table's certificate carries, in the form the theorems want it. -/
structure Certified where
  arities : List Arity
  rows : List Row
  policy : Policy
  sieve : Sieve
  cover : Cover
  /-- The axis the certificate names for a pair of rows, by row numbers. -/
  told : Nat → Nat → Option Nat
  /-- The point and the values behind it, by row number. -/
  witness : Nat → Option (Point × List Rat)
  /-- The pairs the axes do not part — a W114 the tool could not settle, a pair a
  declared precedence orders, a pair the sieve ruled out. A certificate that names one is not claiming the
      two rows are apart, and `Certified.disjoint` does not apply to it. -/
  undecided : Nat → Nat → Bool
  /-- The pairs the axes do not part and the linear model does, each with the refutation
      that says nothing both rows take is asked about (§15.141). -/
  refuted : Nat → Nat → Option (List (Ref × Rat)) := fun _ _ => none
  /-- The cover's leaves the linear model rules out, by the path to each, with the
      refutation for the box below it. -/
  farkasAt : Point → Option (List (Ref × Rat)) := fun _ => none
  /-- The pairs the axes do not part and koyomi's days do (rulec's §15.174): the axis of days
      on which every coordinate both rows take holds none of the days. -/
  daysApart : Nat → Nat → Option Nat := fun _ _ => none
  /-- The cover's leaves the rows of a table above rule out (rulec's §15.195), by the path to
      each: the axis of the column that table decides, and a reason for each row of it that
      writes the value the path holds there. -/
  aboveAt : Point → Option (Nat × List RowOut) := fun _ => none
  /-- The pairs the axes do not part and the rows of a table above do (rulec's §15.196): the axis
      of the column that table decides, and for each value both rows take there, a reason for each
      row of it that writes the value. -/
  aboveApart : Nat → Nat → Option (Nat × List (Nat × List RowOut)) := fun _ _ => none

/-- The table the certificate is about. Its `asked` is the sieve's: a combination counts
    when the values behind it satisfy everything the rule declares. -/
def Certified.table (C : Certified) : Table where
  arities := C.arities
  rows := C.rows
  policy := C.policy
  asked := C.sieve.asked

/-- What rules a box of the cover out: the sieve, the refutation the linear model gives for it
    (§15.141), or the rows of a table above with a reason for each (rulec's §15.195). -/
def Certified.ruledOut (C : Certified) (path : Point) : Bool :=
  boxRuledOut C.sieve C.arities path ||
    (match C.farkasAt path with
     | some refs => farkasRuledOut C.sieve (pathBox C.arities path) refs
     | none => false) ||
    (match C.aboveAt path with
     | some (axis, outs) => aboveRuledOut C.sieve (pathBox C.arities path) axis outs
     | none => false)

theorem Certified.not_asked_of_ruledOut (C : Certified) {path p : Point}
    (h : C.ruledOut path = true) (hpre : path <+: p) (hsp : inSpace C.arities p = true) :
    ¬ C.sieve.asked p := by
  unfold Certified.ruledOut at h
  simp only [Bool.or_eq_true] at h
  rcases h with (h | h) | h
  · exact not_asked_of_boxRuledOut h hpre hsp
  · split at h
    · exact not_asked_of_farkas h (inBox_pathBox hpre hsp)
    · exact absurd h (by simp)
  · split at h
    · exact not_asked_of_aboveRuledOut h (inBox_pathBox hpre hsp)
    · exact absurd h (by simp)

/-- The completeness check: the rows are shaped like the axes, and the walk goes through.
    A box the tables above rule out is no longer outside this — the facts they give are part
    of the sieve, so such a leaf is checked like any other impossible box (§15.115). -/
def Certified.coverChecks (C : Certified) : Bool :=
  rowsShaped C.arities C.rows &&
    coverOk C.arities C.rows C.ruledOut C.cover []

/-- The reachability check, for the rows the certificate claims a point for. -/
def Certified.reachChecks (C : Certified) (claimed : List Row) : Bool :=
  reachOk C.arities claimed C.rows C.policy (witnessOk C.sieve) C.witness

/-- The same, without the values: the point only has to be one the sieve does not
    exclude. What a certificate falls back on when it cannot hand over the values. -/
def Certified.reachWeakChecks (C : Certified) (claimed : List Row) : Bool :=
  reachOk C.arities claimed C.rows C.policy (fun p _ => !pointRuledOut C.sieve p) C.witness

/-- The overlap check, which only a `unique` table makes. -/
def Certified.pairChecks (C : Certified) : Bool :=
  rowsDistinct C.rows && pairsPart C.rows C.told C.undecided

/-- **The overlap check, on the points the rule is asked about** (§15.141). For each pair of
    rows: an axis they part on, or a refutation the linear model gives for what both take,
    or the pair is named as undecided. -/
def pairsApart (s : Sieve) (rows : List Row) (told : Nat → Nat → Option Nat)
    (refuted : Nat → Nat → Option (List (Ref × Rat))) (undecided : Nat → Nat → Bool)
    (daysApart : Nat → Nat → Option Nat := fun _ _ => none)
    (aboveApart : Nat → Nat → Option (Nat × List (Nat × List RowOut)) := fun _ _ => none) : Bool :=
  rows.all (fun r =>
    rows.all (fun q =>
      if r.index < q.index then
        match told r.index q.index with
        | some axis => partsOn r.box q.box axis
        | none =>
          match refuted r.index q.index with
          | some refs => farkasRuledOut s (pairBox r.box q.box) refs
          | none =>
            match daysApart r.index q.index with
            | some axis => daysPart s r.box q.box axis
            | none =>
              match aboveApart r.index q.index with
              | some e => abovePart s r.box q.box e.1 e.2
              | none => undecided r.index q.index
      else true))

theorem disjointAsked_of_pairsApart {s : Sieve} {rows : List Row} {told : Nat → Nat → Option Nat}
    {refuted : Nat → Nat → Option (List (Ref × Rat))} {undecided : Nat → Nat → Bool}
    {daysApart : Nat → Nat → Option Nat} {aboveApart : Nat → Nat → Option (Nat × List (Nat × List RowOut))}
    (hshape : rowsDistinct rows = true) (hnone : ∀ a b, undecided a b = false)
    (h : pairsApart s rows told refuted undecided daysApart aboveApart = true) :
    ∀ p, s.asked p → ((rows.filter (fun r => inBox r.box p)).length ≤ 1) := by
  intro p hask
  refine length_filter_le_one ?_
  have hdis : rows.Pairwise (fun r q => r.index ≠ q.index) := of_decide_eq_true hshape
  have hall := List.all_eq_true.1 h
  refine hdis.imp_of_mem ?_
  intro r q hr hq hne
  rintro ⟨hrp, hqp⟩
  have key : ∀ x y : Row, x ∈ rows → y ∈ rows → x.index < y.index →
      inBox x.box p = true → inBox y.box p = true → False := by
    intro x y hx hy hlt hxp hyp
    have hxy := List.all_eq_true.1 (hall x hx) y hy
    rw [ite_eq_left_of_eq_true _ _ (by simp [hlt])] at hxy
    cases hax : told x.index y.index with
    | some axis => rw [hax] at hxy; exact not_both_of_partsOn hxy hxp hyp
    | none =>
      rw [hax] at hxy
      cases hrf : refuted x.index y.index with
      | some refs =>
        rw [hrf] at hxy
        exact not_asked_of_farkas hxy (inBox_pairBox hxp hyp) hask
      | none =>
        rw [hrf] at hxy
        cases hda : daysApart x.index y.index with
        | some axis =>
          rw [hda] at hxy
          exact not_asked_of_daysPart hxy hxp hyp hask
        | none =>
          rw [hda] at hxy
          cases hab : aboveApart x.index y.index with
          | some e =>
            rw [hab] at hxy
            exact not_asked_of_abovePart hxy hxp hyp hask
          | none => rw [hab, hnone] at hxy; exact absurd hxy (by simp)
  rcases Nat.lt_or_ge r.index q.index with hlt | hge
  · exact key r q hr hq hlt hrp hqp
  · exact key q r hq hr (Nat.lt_of_le_of_ne hge (Ne.symm hne)) hqp hrp

/-- The overlap check on the points the rule is asked about, for a table's certificate. -/
def Certified.pairAskedChecks (C : Certified) : Bool :=
  rowsDistinct C.rows && pairsApart C.sieve C.rows C.told C.refuted C.undecided C.daysApart C.aboveApart

theorem Certified.complete (C : Certified) (h : C.coverChecks = true) :
    C.table.completeHolds := by
  simp only [Certified.coverChecks, Bool.and_eq_true] at h
  obtain ⟨hshape, hcov⟩ := h
  refine complete_of_coverOk (t := C.table) hshape ?_ hcov
  intro path p hro hpre hsp
  exact C.not_asked_of_ruledOut hro hpre hsp

theorem Certified.reached (C : Certified) (claimed : List Row)
    (h : C.reachChecks claimed = true) : C.table.reachedHoldsFor claimed :=
  reached_of_reachOk (t := C.table) (fun _ _ hw => asked_of_witnessOk hw) h

theorem Certified.notExcluded (C : Certified) (claimed : List Row)
    (h : C.reachWeakChecks claimed = true) :
    C.table.notExcludedFor (pointRuledOut C.sieve) claimed :=
  notExcluded_of_reachOk h

theorem Certified.disjoint (C : Certified) (h : C.pairChecks = true)
    (hu : ∀ a b, C.undecided a b = false) : C.table.disjointHolds := by
  simp only [Certified.pairChecks, Bool.and_eq_true] at h
  exact fun p => disjoint_of_pairsPart h.1 hu h.2 p

theorem Certified.disjointAsked (C : Certified) (h : C.pairAskedChecks = true)
    (hu : ∀ a b, C.undecided a b = false) : C.table.disjointAskedHolds := by
  simp only [Certified.pairAskedChecks, Bool.and_eq_true] at h
  intro p _ hask
  exact disjointAsked_of_pairsApart h.1 hu h.2 p hask

/-- **What a `unique` table's certificate says, all told**: every combination the rule is
    asked about is answered by exactly one row. The pairs may be parted by an axis or by the
    linear model; either way no point the rule is asked about lies in two rows. -/
theorem Certified.unique (C : Certified) (hc : C.coverChecks = true)
    (hp : C.pairAskedChecks = true) (hu : ∀ a b, C.undecided a b = false) :
    C.table.uniqueHolds := by
  intro p hsp hask
  have h1 := C.complete hc p hsp hask
  have h2 := C.disjointAsked hp hu p hsp hask
  have h3 : (C.table.firing p).length ≠ 0 := fun h => h1 (List.eq_nil_of_length_eq_zero h)
  omega

/-! ## A row's box, less the values no table above writes (rulec's §15.253)

rulec reads every box of a table without the coordinates of a column decided above whose value
no row of the deciding table writes: nothing brings the column there. The certificate states
each such coordinate as a `never` fact, and a row's box is the box its cells describe with those
coordinates left out of their axes — `RulecMain` holds it to exactly that. The theorems above are
about the boxes as stated; the ones below carry each claim over to the table whose rows take the
boxes their cells describe. A point the rule is asked about stands on no `never` coordinate
(`Sieve.asked` says so), and there the two boxes take the same points. -/

/-- One axis of a box, without the coordinates `never` names on axis `i`. -/
def dropNeverAt (never : List (Nat × Nat)) (i : Nat) (xs : List Nat) : List Nat :=
  xs.filter (fun c => !never.any (fun q => q.1 == i && q.2 == c))

/-- A box from axis `i` on, each axis without the coordinates `never` names on it. -/
def dropNeverFrom (never : List (Nat × Nat)) : Nat → Box → Box
  | _, [] => []
  | i, xs :: rest => dropNeverAt never i xs :: dropNeverFrom never (i + 1) rest

/-- A box without the coordinates `never` names, axis by axis. -/
def dropNever (never : List (Nat × Nat)) (box : Box) : Box := dropNeverFrom never 0 box

theorem contains_dropNeverAt {never : List (Nat × Nat)} {i c : Nat} {xs : List Nat}
    (h : ∀ q ∈ never, ¬(q.1 = i ∧ q.2 = c)) :
    (dropNeverAt never i xs).contains c = xs.contains c := by
  have hc : never.any (fun q => q.1 == i && q.2 == c) = false := by
    rw [List.any_eq_false]
    intro q hq hqc
    simp only [Bool.and_eq_true, beq_iff_eq] at hqc
    exact h q hq hqc
  apply Bool.eq_iff_iff.2
  simp only [dropNeverAt, List.contains_iff_mem, List.mem_filter, hc, Bool.not_false, and_true]

theorem inBox_dropNeverFrom {never : List (Nat × Nat)} :
    ∀ (i : Nat) (box : Box) (p : Point),
      (∀ (k c : Nat), p[k]? = some c → ∀ q ∈ never, ¬(q.1 = i + k ∧ q.2 = c)) →
      inBox (dropNeverFrom never i box) p = inBox box p := by
  intro i box
  induction box generalizing i with
  | nil => intro p _; cases p <;> rfl
  | cons xs rest ih =>
    intro p h
    cases p with
    | nil => rfl
    | cons c cs =>
      simp only [dropNeverFrom, inBox]
      rw [contains_dropNeverAt (fun q hq hqc => h 0 c (by simp) q hq ⟨by omega, hqc.2⟩)]
      rw [ih (i + 1) cs (fun k d hd q hq hqd => h (k + 1) d (by simpa using hd) q hq ⟨by omega, hqd.2⟩)]

/-- **Leaving the `never` coordinates out changes nothing at a point that stands on none of
    them.** -/
theorem inBox_dropNever {never : List (Nat × Nat)} {p : Point}
    (h : ∀ q ∈ never, p[q.1]? ≠ some q.2) (b : Box) :
    inBox (dropNever never b) p = inBox b p :=
  inBox_dropNeverFrom 0 b p (fun k c hk q hq hqk => h q hq (by rw [hqk.1, Nat.zero_add, hk, hqk.2]))

/-- A point the rule is asked about stands on no coordinate the tables above never write. -/
theorem Sieve.asked_off_never {s : Sieve} {p : Point} (h : s.asked p) :
    ∀ q ∈ s.never, p[q.1]? ≠ some q.2 := by
  obtain ⟨_, _, _, _, hn, _⟩ := h
  exact hn

/-- Nor does a point the sieve does not exclude: a `never` coordinate is one of its reasons. -/
theorem off_never_of_pointRuledOut {s : Sieve} {p : Point} (h : pointRuledOut s p = false) :
    ∀ q ∈ s.never, p[q.1]? ≠ some q.2 := by
  intro q hq hpq
  have hn : neverRulesOut s p = true := by
    simp only [neverRulesOut, List.any_eq_true, beq_iff_eq]
    exact ⟨q, hq, hpq⟩
  simp [pointRuledOut, hn] at h

/-- A row, its box stated without the `never` coordinates. -/
def Row.dropNever (never : List (Nat × Nat)) (r : Row) : Row := { r with box := RulecCert.dropNever never r.box }

/-- The rows a certificate states are the rows its cells describe, each without the coordinates
    of its `never` facts — what `RulecMain.checkBoxes` holds every row to, axis by axis. -/
def Certified.statesCells (C : Certified) (cells : List Row) : Prop :=
  C.rows = cells.map (Row.dropNever C.sieve.never)

/-- The table the cells describe: the same axes, policy and points asked about, and each row
    taking the box its cells describe. -/
def Certified.cellTable (C : Certified) (cells : List Row) : Table where
  arities := C.arities
  rows := cells
  policy := C.policy
  asked := C.sieve.asked

theorem Certified.firing_of_off_never (C : Certified) {cells : List Row} (hc : C.statesCells cells)
    {p : Point} (hoff : ∀ q ∈ C.sieve.never, p[q.1]? ≠ some q.2) :
    C.table.firing p = ((C.cellTable cells).firing p).map (Row.dropNever C.sieve.never) := by
  have hrows : C.rows = cells.map (Row.dropNever C.sieve.never) := hc
  simp only [Table.firing, Certified.table, Certified.cellTable, hrows, List.filter_map]
  congr 1
  apply List.filter_congr
  intro r _
  simp [Row.dropNever, inBox_dropNever hoff]

/-- **Completeness, of the rows as their cells take them.** -/
theorem Certified.cells_complete (C : Certified) {cells : List Row} (hc : C.statesCells cells)
    (h : C.table.completeHolds) : (C.cellTable cells).completeHolds := by
  intro p hsp hask hnil
  have := h p hsp hask
  rw [C.firing_of_off_never hc (Sieve.asked_off_never hask), hnil] at this
  exact this rfl

/-- **Exactly one row, of the rows as their cells take them.** -/
theorem Certified.cells_unique (C : Certified) {cells : List Row} (hc : C.statesCells cells)
    (h : C.table.uniqueHolds) : (C.cellTable cells).uniqueHolds := by
  intro p hsp hask
  have := h p hsp hask
  rwa [C.firing_of_off_never hc (Sieve.asked_off_never hask), List.length_map] at this

theorem Certified.cells_disjointAsked (C : Certified) {cells : List Row} (hc : C.statesCells cells)
    (h : C.table.disjointAskedHolds) : (C.cellTable cells).disjointAskedHolds := by
  intro p hsp hask
  have := h p hsp hask
  rwa [C.firing_of_off_never hc (Sieve.asked_off_never hask), List.length_map] at this

theorem Certified.cells_holdsFor (C : Certified) {cells : List Row} (hc : C.statesCells cells)
    {P : Point → Prop} (hP : ∀ p, P p → ∀ q ∈ C.sieve.never, p[q.1]? ≠ some q.2)
    (claimed : List Row) (h : C.table.holdsFor P (claimed.map (Row.dropNever C.sieve.never))) :
    (C.cellTable cells).holdsFor P claimed := by
  intro r hr
  obtain ⟨p, hsp, hp, hin, hfirst⟩ := h (r.dropNever C.sieve.never) (List.mem_map_of_mem hr)
  refine ⟨p, hsp, hp, ?_, ?_⟩
  · rw [← inBox_dropNever (hP p hp)]; exact hin
  · intro hpol q hq hlt
    have hq' : q.dropNever C.sieve.never ∈ C.table.rows := by
      have hrows : C.rows = cells.map (Row.dropNever C.sieve.never) := hc
      show q.dropNever C.sieve.never ∈ C.rows
      rw [hrows]; exact List.mem_map_of_mem hq
    rw [← inBox_dropNever (hP p hp)]
    exact hfirst hpol _ hq' hlt

/-- **Every claimed row reached, as its cells take it**: the point that reaches the stated box
    reaches the cells' box, and under `first` no earlier row's cells take it either. -/
theorem Certified.cells_reached (C : Certified) {cells : List Row} (hc : C.statesCells cells)
    (claimed : List Row) (h : C.table.reachedHoldsFor (claimed.map (Row.dropNever C.sieve.never))) :
    (C.cellTable cells).reachedHoldsFor claimed :=
  C.cells_holdsFor hc (fun _ hp => Sieve.asked_off_never hp) claimed h

/-- The weaker reading of the same, for points that come with no values behind them. -/
theorem Certified.cells_notExcluded (C : Certified) {cells : List Row} (hc : C.statesCells cells)
    (claimed : List Row)
    (h : C.table.notExcludedFor (pointRuledOut C.sieve) (claimed.map (Row.dropNever C.sieve.never))) :
    (C.cellTable cells).notExcludedFor (pointRuledOut C.sieve) claimed :=
  C.cells_holdsFor hc (fun _ hp => off_never_of_pointRuledOut hp) claimed h

end RulecCert
