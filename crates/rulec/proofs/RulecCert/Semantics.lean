/-
  The meaning of a rule, as far as a certificate speaks about it (DESIGN §15.70, §15.97).

  This file is the referent of the word "proved". Everything `RulecCert.Check` decides is a
  statement *about these definitions*, and `RulecCert.Sound` is the proof that deciding it
  settles the statement. Nothing here reads a `.rule` file or a JSON document: a rule
  arrives already as the finite, coordinate-shaped object §6.2 compresses it to, and what a
  certificate has to make believable is that the compression is faithful — which is the
  `cells` half of the document and the subject of `RulecCert.Cells`, not of this one.

  The shape, in one paragraph. An **axis** is a finite list of coordinates; a **point** picks
  one coordinate per axis; a **box** takes a set of coordinates per axis, and a point is in
  the box when every one of its coordinates is taken. A **row** is a box with a number; a
  **table** is a list of rows and a policy. Under `unique` a point must be in exactly one
  row's box; under `first` the row that fires is the earliest whose box takes it. A point
  the rule is never **asked** about — no input produces it — is outside every claim.

  `asked` is a `Prop`, not a `Bool`, on purpose. Whether a combination can arrive is a
  question about the values behind the coordinates, and no finite test settles it in
  general; the certificate's leaves make the cases it *can* settle decidable, and
  `RulecCert.Sieve` is where those tests are shown to imply this predicate.
-/

namespace RulecCert

/-- An axis is known by how many coordinates it has. -/
abbrev Arity := Nat

/-- A point of the space: one coordinate per axis, in axis order. -/
abbrev Point := List Nat

/-- A box: the coordinates it takes on each axis, in axis order. -/
abbrev Box := List (List Nat)

/-- A point lies in a box when every coordinate it picks is one the box takes. The two
    lists walk together; a box that names fewer axes than the point takes nothing, which is
    the safe reading of a malformed certificate. -/
def inBox : Box → Point → Bool
  | [], [] => true
  | b :: bs, c :: cs => b.contains c && inBox bs cs
  | _, _ => false

/-- A point of the space: each coordinate is below its axis's arity, and the point names
    exactly as many axes as there are. Nothing enumerates the space — the checks walk the
    cover, the pairs and the rows — so a predicate is all that is needed. -/
def inSpace : List Arity → Point → Bool
  | [], [] => true
  | n :: ns, c :: cs => decide (c < n) && inSpace ns cs
  | _, _ => false

theorem inSpace_length {ns : List Arity} {p : Point} (h : inSpace ns p = true) :
    p.length = ns.length := by
  induction ns generalizing p with
  | nil => cases p with
    | nil => rfl
    | cons _ _ => simp [inSpace] at h
  | cons n ns ih => cases p with
    | nil => simp [inSpace] at h
    | cons c cs =>
      simp [inSpace] at h
      simp [ih h.2]

/-- Every coordinate of a point of the space is below its axis's arity. -/
theorem inSpace_getElem {ns : List Arity} {p : Point} (h : inSpace ns p = true)
    {i : Nat} {n : Arity} (hn : ns[i]? = some n) : ∃ c, p[i]? = some c ∧ c < n := by
  induction ns generalizing p i with
  | nil => simp at hn
  | cons m ms ih =>
    cases p with
    | nil => simp [inSpace] at h
    | cons c cs =>
      simp only [inSpace, Bool.and_eq_true, decide_eq_true_eq] at h
      cases i with
      | zero =>
        simp only [List.getElem?_cons_zero, Option.some.injEq] at hn
        exact ⟨c, by simp, hn ▸ h.1⟩
      | succ j =>
        obtain ⟨d, hd, hlt⟩ := ih h.2 (by simpa using hn)
        exact ⟨d, by simpa using hd, hlt⟩

/-- How a table settles which row answers. -/
inductive Policy where
  | unique
  | first
  deriving DecidableEq, Repr

/-- A row: where it sits in the table (1-based, as the rule writes it) and the box it takes. -/
structure Row where
  index : Nat
  box : Box
  deriving Repr

/-- A table, as a certificate describes it: the axes' arities, the rows, the policy, and
    which points the rule is asked about — the combinations some input really produces
    (§6.2), which excludes the ones a `constraint` forbids (§15.55). -/
structure Table where
  arities : List Arity
  rows : List Row
  policy : Policy
  /-- The points some input reaches. Every claim below is made about these and no others. -/
  asked : Point → Prop

namespace Table

/-- The rows whose box takes this point. -/
def firing (t : Table) (p : Point) : List Row :=
  t.rows.filter (fun r => inBox r.box p)

/-- **What `unique` claims**: every point the rule is asked about is taken by exactly one
    row — no gap and no overlap, which is E101 and E105 together. -/
def uniqueHolds (t : Table) : Prop :=
  ∀ p, inSpace t.arities p = true → t.asked p → (t.firing p).length = 1

/-- **What completeness claims**: every point the rule is asked about is taken by at least
    one row. This is what `policy first` needs, and what `unique` needs before decisiveness
    (E101). -/
def completeHolds (t : Table) : Prop :=
  ∀ p, inSpace t.arities p = true → t.asked p → (t.firing p) ≠ []

/-- **What disjointness claims**: no point is taken by two rows at once (E105). -/
def disjointHolds (t : Table) : Prop :=
  ∀ p, (t.firing p).length ≤ 1

/-- **The same, for the points the rule is asked about.** Two rows whose boxes meet only
    where no input arrives — which the linear model can show where the axes cannot — answer
    one point each all the same (§15.141). -/
def disjointAskedHolds (t : Table) : Prop :=
  ∀ p, inSpace t.arities p = true → t.asked p → (t.firing p).length ≤ 1

/-- **What reachability claims**, for the rows it is claimed of: each of them answers
    somewhere (E102). Under `first` a row that an earlier row always beats answers nowhere,
    so the point has to be one no earlier row takes — and "earlier" is read against the
    whole table, not against the rows the claim is made for.

    The claim is made for a list rather than for the table because a row an `apply` brought
    in and this rule's bindings leave unused answers nowhere on purpose (§15.69). Those
    rows are named in the certificate, and left out here rather than quietly passed. -/
def holdsFor (t : Table) (P : Point → Prop) (claimed : List Row) : Prop :=
  ∀ r ∈ claimed, ∃ p, inSpace t.arities p = true ∧ P p ∧ inBox r.box p = true ∧
      (t.policy = Policy.first → ∀ q ∈ t.rows, q.index < r.index → inBox q.box p = false)

/-- Each of these rows holds a point the rule is asked about. -/
def reachedHoldsFor (t : Table) (claimed : List Row) : Prop := t.holdsFor t.asked claimed

/-- The weaker reading, for when the certificate hands over no values behind the point:
    each of these rows holds a point **the sieve does not exclude**. That is what `rulec`
    itself decides — the sieve is a necessary condition for a combination to arrive, not a
    sufficient one — so a certificate that cannot say more says this. -/
def notExcludedFor (t : Table) (excluded : Point → Bool) (claimed : List Row) : Prop :=
  t.holdsFor (fun p => excluded p = false) claimed

/-- Every row of the table answers somewhere. -/
def reachedHolds (t : Table) : Prop := t.reachedHoldsFor t.rows

end Table

/-! ## Reading a box coordinate by coordinate

The checks state their business as "on axis `i` the box takes / does not take this
coordinate", and the claims are about `inBox`. These two lemmas are the bridge, and they
are used in every proof in `RulecCert.Sound`. -/

/-- What a point in a box says about one axis. -/
theorem inBox_getElem {b : Box} {p : Point} (h : inBox b p = true) {i : Nat} {xs : List Nat}
    (hb : b[i]? = some xs) : ∃ c, p[i]? = some c ∧ xs.contains c = true := by
  induction b generalizing p i with
  | nil => simp at hb
  | cons y ys ih =>
    cases p with
    | nil => simp [inBox] at h
    | cons c cs =>
      simp only [inBox, Bool.and_eq_true] at h
      cases i with
      | zero =>
        simp only [List.getElem?_cons_zero, Option.some.injEq] at hb
        exact ⟨c, by simp, hb ▸ h.1⟩
      | succ j =>
        obtain ⟨d, hd, hmem⟩ := ih h.2 (by simpa using hb)
        exact ⟨d, by simpa using hd, hmem⟩

/-- The same length, and the same reading on every axis, is enough to be in the box. -/
theorem inBox_of_getElem {b : Box} {p : Point} (hlen : b.length = p.length)
    (h : ∀ (i : Nat) (xs : List Nat) (c : Nat), b[i]? = some xs → p[i]? = some c → xs.contains c = true) :
    inBox b p = true := by
  induction b generalizing p with
  | nil => cases p with
    | nil => rfl
    | cons _ _ => simp at hlen
  | cons y ys ih =>
    cases p with
    | nil => simp at hlen
    | cons c cs =>
      simp only [inBox, Bool.and_eq_true]
      refine ⟨h 0 y c (by simp) (by simp), ih (by simpa using hlen) ?_⟩
      intro i xs d hx hd
      exact h (i + 1) xs d (by simpa using hx) (by simpa using hd)

end RulecCert
