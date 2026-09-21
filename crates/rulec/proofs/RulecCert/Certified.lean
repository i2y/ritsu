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

/-- The table the certificate is about. Its `asked` is the sieve's: a combination counts
    when the values behind it satisfy everything the rule declares. -/
def Certified.table (C : Certified) : Table where
  arities := C.arities
  rows := C.rows
  policy := C.policy
  asked := C.sieve.asked

/-- The completeness check: the rows are shaped like the axes, the cover leans on nothing
    upstream, and the walk goes through. -/
def Certified.coverChecks (C : Certified) : Bool :=
  rowsShaped C.arities C.rows && !C.cover.leansOnUpstream &&
    coverOk C.arities C.rows (boxRuledOut C.sieve C.arities) C.cover []

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

theorem Certified.complete (C : Certified) (h : C.coverChecks = true) :
    C.table.completeHolds := by
  simp only [Certified.coverChecks, Bool.and_eq_true, Bool.not_eq_true'] at h
  obtain ⟨⟨hshape, hup⟩, hcov⟩ := h
  refine complete_of_coverOk (t := C.table) hshape ?_ hup hcov
  intro path p hro hpre hsp
  exact not_asked_of_boxRuledOut hro hpre hsp

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

/-- **What a `unique` table's certificate says, all told**: every combination the rule is
    asked about is answered by exactly one row. -/
theorem Certified.unique (C : Certified) (hc : C.coverChecks = true)
    (hp : C.pairChecks = true) (hu : ∀ a b, C.undecided a b = false) :
    C.table.uniqueHolds := by
  intro p hsp hask
  have h1 := C.complete hc p hsp hask
  have h2 := C.disjoint hp hu p
  have h3 : (C.table.firing p).length ≠ 0 := fun h => h1 (List.eq_nil_of_length_eq_zero h)
  omega

end RulecCert
