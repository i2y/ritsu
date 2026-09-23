/-
  One table's certificate, and what follows from it (DESIGN §15.97).

  `RulecCert.Sound` states each theorem against the check it belongs to; this file puts one
  table's worth of them together, so that a single `true` from `Certified.checks` is a
  statement about the table itself. It is also the shape `Main` builds from the JSON: what
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

/-- The table the certificate is about. Its `asked` is the sieve's: a combination counts
    when the values behind it satisfy everything the rule declares. -/
def Certified.table (C : Certified) : Table where
  arities := C.arities
  rows := C.rows
  policy := C.policy
  asked := C.sieve.asked

/-- What rules a box of the cover out: the sieve, or the refutation the linear model gives
    for it (§15.141). -/
def Certified.ruledOut (C : Certified) (path : Point) : Bool :=
  boxRuledOut C.sieve C.arities path ||
    (match C.farkasAt path with
     | some refs => farkasRuledOut C.sieve (pathBox C.arities path) refs
     | none => false)

theorem Certified.not_asked_of_ruledOut (C : Certified) {path p : Point}
    (h : C.ruledOut path = true) (hpre : path <+: p) (hsp : inSpace C.arities p = true) :
    ¬ C.sieve.asked p := by
  unfold Certified.ruledOut at h
  simp only [Bool.or_eq_true] at h
  rcases h with h | h
  · exact not_asked_of_boxRuledOut h hpre hsp
  · split at h
    · exact not_asked_of_farkas h (inBox_pathBox hpre hsp)
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
    (refuted : Nat → Nat → Option (List (Ref × Rat))) (undecided : Nat → Nat → Bool) : Bool :=
  rows.all (fun r =>
    rows.all (fun q =>
      if r.index < q.index then
        match told r.index q.index with
        | some axis => partsOn r.box q.box axis
        | none =>
          match refuted r.index q.index with
          | some refs => farkasRuledOut s (pairBox r.box q.box) refs
          | none => undecided r.index q.index
      else true))

theorem disjointAsked_of_pairsApart {s : Sieve} {rows : List Row} {told : Nat → Nat → Option Nat}
    {refuted : Nat → Nat → Option (List (Ref × Rat))} {undecided : Nat → Nat → Bool}
    (hshape : rowsDistinct rows = true) (hnone : ∀ a b, undecided a b = false)
    (h : pairsApart s rows told refuted undecided = true) :
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
      | none => rw [hrf, hnone] at hxy; exact absurd hxy (by simp)
  rcases Nat.lt_or_ge r.index q.index with hlt | hge
  · exact key r q hr hq hlt hrp hqp
  · exact key q r hq hr (Nat.lt_of_le_of_ne hge (Ne.symm hne)) hqp hrp

/-- The overlap check on the points the rule is asked about, for a table's certificate. -/
def Certified.pairAskedChecks (C : Certified) : Bool :=
  rowsDistinct C.rows && pairsApart C.sieve C.rows C.told C.refuted C.undecided

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

end RulecCert
