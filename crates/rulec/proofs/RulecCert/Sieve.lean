/-
  Which combinations the rule is asked about, and how the certificate settles it (§6.2,
  §15.55, §15.98).

  `RulecCert.Semantics` leaves `Table.asked` abstract, and `RulecCert.Sound` takes it on
  hypothesis: a box the cover calls impossible really is one no input reaches. This file is
  where that hypothesis is discharged. A point is asked about when there are values behind
  its coordinates — one per axis, each inside the coordinate it names — that satisfy every
  `constraint` the rule declares and put every derived column inside the interval its own
  expression is forced into.

  Read that definition twice before trusting the word "impossible". It is **consistency
  with what the rule declares**, not "some real input produces this": whether a derived
  column can actually take a value its interval allows is not decided here, and `rulec`
  does not decide it either (§6.2 calls the reading loose in the safe direction). What is
  proved below is that the certificate's leaves imply *this* notion, and every claim that
  rests on it means what this definition says and no more.
-/
import RulecCert.Sound
import RulecCert.Linear

namespace RulecCert

/-! ## Values behind a coordinate -/

/-- A coordinate stands for a set of values: one value, or an interval strictly between two
    boundaries, either of which may run on (§6.2). -/
inductive Coord where
  | exactly : Rat → Coord
  | between : Option Rat → Option Rat → Coord
  deriving Repr, Inhabited

/-- An interval with open ends, as the declared ranges and the derives both carry it. -/
abbrev Ival := Option Rat × Option Rat

def inIval (i : Ival) (v : Rat) : Prop :=
  (∀ a, i.1 = some a → a ≤ v) ∧ (∀ b, i.2 = some b → v ≤ b)

def Coord.holds : Coord → Rat → Prop
  | .exactly a, v => v = a
  | .between lo hi, v =>
      (∀ a, lo = some a → a < v) ∧ (∀ b, hi = some b → v < b)

/-- The closed interval a coordinate cannot leave. Wider than the coordinate itself where
    the ends are open, which is the safe side for every use below: a box ruled out on this
    reading is ruled out on the true one. -/
def Coord.span : Coord → Ival
  | .exactly a => (some a, some a)
  | .between lo hi => (lo, hi)

/-- Whether a coordinate leaves its ends out: an interval does, a single value does not.
    Where two ends meet, this is what decides whether a pair can meet there (§15.140). -/
def Coord.opn : Coord → Bool
  | .exactly _ => false
  | .between _ _ => true

theorem Coord.holds_lo_lt {x : Coord} {v a : Rat} (h : x.holds v) (ho : x.opn = true)
    (ha : x.span.1 = some a) : a < v := by
  cases x with
  | exactly _ => simp [Coord.opn] at ho
  | between lo hi => exact h.1 a ha

theorem Coord.holds_hi_lt {x : Coord} {v b : Rat} (h : x.holds v) (ho : x.opn = true)
    (hb : x.span.2 = some b) : v < b := by
  cases x with
  | exactly _ => simp [Coord.opn] at ho
  | between lo hi => exact h.2 b hb

/-! Three steps of rational arithmetic that core states in other words. Everything below
leans on these and on nothing else. -/

theorem lelt {a b c : Rat} (h1 : a ≤ b) (h2 : b < c) : a < c :=
  Rat.not_le.1 (fun hca => (Rat.not_le.2 h2) (Rat.le_trans hca h1))

theorem ltle {a b c : Rat} (h1 : a < b) (h2 : b ≤ c) : a < c :=
  Rat.not_le.1 (fun hca => (Rat.not_le.2 h1) (Rat.le_trans h2 hca))

/-- Nothing sits above `a` and below `b` when `b` is already below `a`. -/
theorem no_room {a b w : Rat} (h1 : a ≤ w) (h2 : w ≤ b) (h3 : b < a) : False :=
  Rat.lt_irrefl (lelt (Rat.le_trans h1 h2) h3)

/-- The same, one step tighter: the pair in the middle is strictly ordered instead. -/
theorem no_room' {a b x y : Rat} (h1 : a ≤ x) (hxy : x < y) (h2 : y ≤ b) (h3 : b ≤ a) : False :=
  Rat.lt_irrefl (lelt h1 (ltle (ltle hxy h2) h3))

theorem Coord.holds_span {x : Coord} {v : Rat} (h : x.holds v) : inIval x.span v := by
  cases x with
  | exactly a =>
    subst h
    exact ⟨fun b hb => by cases hb; exact Rat.le_refl, fun b hb => by cases hb; exact Rat.le_refl⟩
  | between lo hi =>
    obtain ⟨h1, h2⟩ := h
    exact ⟨fun a ha => Rat.le_of_lt (h1 a ha), fun b hb => Rat.le_of_lt (h2 b hb)⟩

/-! ## What the rule declares -/

inductive Cmp where
  | le | lt | ge | gt
  /-- A cell that writes a bare number is this: `10000円` on a numeric column. No
      `constraint` uses it, so it never rules a box out — but a cell does. -/
  | eq
  deriving Repr, DecidableEq, Inhabited

def Cmp.holds : Cmp → Rat → Rat → Prop
  | .le, x, y => x ≤ y
  | .lt, x, y => x < y
  | .ge, x, y => y ≤ x
  | .gt, x, y => y < x
  | .eq, x, y => x = y

/-- `constraint 甲 <= 乙`, read on the axes of one table. -/
structure Constraint where
  left : Nat
  op : Cmp
  right : Nat
  deriving Repr, Inhabited

/-- Everything about a table that decides whether a combination can arrive: the values each
    coordinate stands for, the constraints, and — for a derived column — the interval its
    expression is forced into. -/
structure Sieve where
  /-- One entry per axis, one per coordinate. `none` where the coordinate stands for no
      number — an enum, a flag, the absent value of an optional column — which is also
      every coordinate no constraint and no derive can speak about. -/
  coords : List (List (Option Coord))
  cons : List Constraint
  reach : List (Option Ival)
  /-- `(axis, coordinate)` pairs no table above ever writes: not one row of the table that
      decides the column puts it there. -/
  never : List (Nat × Nat)
  /-- Pairs of `(axis, coordinate)` the tables above cannot hold at the same time.
      Two columns decided above can be cut from one input at different thresholds, and then
      each value arrives on its own while the pair never does.

      Where the pairs come from: a table above writes a value only in some of its rows, and
      a row fires only inside its box, so the union of those boxes contains every input on
      which the column holds the value. Two such unions that do not meet are a pair that
      cannot stand together. That reading is arithmetic on the very rows the certificate
      carries, so the checker earns each pair back rather than believing it; what is settled
      **here** is the step from the pair to the box, which is the step the cover rests on. -/
  apart : List ((Nat × Nat) × (Nat × Nat))
  /-- The rule's linear model around the table (§15.141): each `derive`'s equation, the
      declared ranges and the `constraint`s, over numbered values — the axes first, in axis
      order, then the names the model uses that are not columns of the table. Values that
      satisfy the coordinates and not these are values no input produces. -/
  facts : List LinIneq := []

def Sieve.coordAt (s : Sieve) (i c : Nat) : Option Coord :=
  match s.coords[i]? with
  | none => none
  | some cs => match cs[c]? with
               | none => none
               | some x => x

/-- **The point is asked about**: values exist behind its coordinates that satisfy
    everything the rule declares. -/
def Sieve.asked (s : Sieve) (p : Point) : Prop :=
  ∃ v : List Rat,
    (∀ (i c : Nat) (x : Coord), p[i]? = some c → s.coordAt i c = some x →
      ∃ w, v[i]? = some w ∧ x.holds w) ∧
    (∀ k ∈ s.cons, ∃ x y, v[k.left]? = some x ∧ v[k.right]? = some y ∧ k.op.holds x y) ∧
    (∀ (i : Nat) (I : Ival), s.reach[i]? = some (some I) → ∃ w, v[i]? = some w ∧ inIval I w) ∧
    (∀ q ∈ s.never, p[q.1]? ≠ some q.2) ∧
    (∀ qr ∈ s.apart, ¬(p[qr.1.1]? = some qr.1.2 ∧ p[qr.2.1]? = some qr.2.2)) ∧
    (∀ q ∈ s.facts, q.holds v)

/-! ## One constraint rules a box out -/

/-- Whether the two coordinates this point fixes leave the comparison no room. Both ends
    are read off the closed spans, so a `true` here is a fact about every value pair the
    box allows. Where the two ends of a `≤` or a `≥` meet, the pair is still out of reach
    when one of the two coordinates leaves its end out (§15.140). -/
def constraintRulesOut (s : Sieve) (k : Constraint) (p : Point) : Bool :=
  match p[k.left]?, p[k.right]? with
  | some cl, some cr =>
    match s.coordAt k.left cl, s.coordAt k.right cr with
    | some xl, some xr =>
      match k.op with
      | .le => match xl.span.1, xr.span.2 with
        | some a, some b => decide (b < a) || (decide (a = b) && (xl.opn || xr.opn))
        | _, _ => false
      | .lt => match xl.span.1, xr.span.2 with | some a, some b => decide (b ≤ a) | _, _ => false
      | .ge => match xl.span.2, xr.span.1 with
        | some a, some b => decide (a < b) || (decide (a = b) && (xl.opn || xr.opn))
        | _, _ => false
      | .gt => match xl.span.2, xr.span.1 with | some a, some b => decide (a ≤ b) | _, _ => false
      | .eq => false
    | _, _ => false
  | _, _ => false

/-- A derived column whose coordinate lies outside what its own expression can produce. -/
def derivedRulesOut (s : Sieve) (i : Nat) (p : Point) : Bool :=
  match p[i]?, s.reach[i]? with
  | some c, some (some I) =>
    match s.coordAt i c with
    | some x =>
      (match x.span.2, I.1 with | some b, some a => decide (b < a) | _, _ => false) ||
      (match x.span.1, I.2 with | some a, some b => decide (b < a) | _, _ => false)
    | none => false
  | _, _ => false

/-- A coordinate no table above ever writes. -/
def neverRulesOut (s : Sieve) (p : Point) : Bool :=
  s.never.any (fun q => p[q.1]? == some q.2)

/-- Two coordinates the tables above cannot hold at once, both of them held here. -/
def apartRulesOut (s : Sieve) (p : Point) : Bool :=
  s.apart.any (fun qr => p[qr.1.1]? == some qr.1.2 && p[qr.2.1]? == some qr.2.2)

/-- The reading of a point the two tests above share: pull the value out of the assignment
    and bound it by the coordinate's span. -/
private theorem value_at {s : Sieve} {p : Point} {v : List Rat} {i c : Nat} {x : Coord}
    (hf : ∀ (i c : Nat) (x : Coord), p[i]? = some c → s.coordAt i c = some x →
      ∃ w, v[i]? = some w ∧ x.holds w)
    (hp : p[i]? = some c) (hx : s.coordAt i c = some x) :
    ∃ w, v[i]? = some w ∧ inIval x.span w := by
  obtain ⟨w, hw, hh⟩ := hf i c x hp hx
  exact ⟨w, hw, Coord.holds_span hh⟩

theorem not_asked_of_constraint {s : Sieve} {k : Constraint} {p : Point}
    (hk : k ∈ s.cons) (h : constraintRulesOut s k p = true) : ¬ s.asked p := by
  rintro ⟨v, hf, hc, _, _, _⟩
  obtain ⟨x, y, hx, hy, hop⟩ := hc k hk
  unfold constraintRulesOut at h
  split at h
  case _ cl cr hpl hpr =>
    split at h
    case _ xl xr hcl hcr =>
      obtain ⟨wl, hwl, hl⟩ := hf _ _ xl hpl hcl
      obtain ⟨wr, hwr, hr⟩ := hf _ _ xr hpr hcr
      have hbl := Coord.holds_span hl
      have hbr := Coord.holds_span hr
      rw [hx] at hwl; rw [hy] at hwr
      obtain rfl : x = wl := Option.some.inj hwl
      obtain rfl : y = wr := Option.some.inj hwr
      cases hop' : k.op <;> simp only [hop', Cmp.holds] at h hop
      · split at h
        case _ a b ha hb =>
          simp only [Bool.or_eq_true, Bool.and_eq_true, decide_eq_true_eq] at h
          rcases h with h | ⟨rfl, ho | ho⟩
          · exact no_room (hbl.1 a ha) (Rat.le_trans hop (hbr.2 b hb)) h
          · -- a < x ≤ y ≤ a
            exact Rat.lt_irrefl (ltle (Coord.holds_lo_lt hl ho ha) (Rat.le_trans hop (hbr.2 a hb)))
          · -- a ≤ x ≤ y < a
            exact Rat.lt_irrefl (lelt (Rat.le_trans (hbl.1 a ha) hop) (Coord.holds_hi_lt hr ho hb))
        case _ => exact absurd h (by simp)
      · split at h
        case _ a b ha hb =>
          simp only [decide_eq_true_eq] at h
          exact no_room' (hbl.1 a ha) hop (hbr.2 b hb) h
        case _ => exact absurd h (by simp)
      · split at h
        case _ a b ha hb =>
          simp only [Bool.or_eq_true, Bool.and_eq_true, decide_eq_true_eq] at h
          rcases h with h | ⟨rfl, ho | ho⟩
          · exact no_room (Rat.le_trans (hbr.1 b hb) hop) (hbl.2 a ha) h
          · -- a ≤ y ≤ x < a
            exact Rat.lt_irrefl (lelt (Rat.le_trans (hbr.1 a hb) hop) (Coord.holds_hi_lt hl ho ha))
          · -- a < y ≤ x ≤ a
            exact Rat.lt_irrefl (ltle (Coord.holds_lo_lt hr ho hb) (Rat.le_trans hop (hbl.2 a ha)))
        case _ => exact absurd h (by simp)
      · split at h
        case _ a b ha hb =>
          simp only [decide_eq_true_eq] at h
          exact no_room' (hbr.1 b hb) hop (hbl.2 a ha) h
        case _ => exact absurd h (by simp)
      · exact absurd h (by simp)
    case _ => exact absurd h (by simp)
  case _ => exact absurd h (by simp)

theorem not_asked_of_derived {s : Sieve} {i : Nat} {p : Point}
    (h : derivedRulesOut s i p = true) : ¬ s.asked p := by
  rintro ⟨v, hf, _, hr, _, _⟩
  unfold derivedRulesOut at h
  split at h
  case _ c I hp hri =>
    split at h
    case _ x hc =>
      obtain ⟨w, hw, hb⟩ := value_at hf hp hc
      obtain ⟨w', hw', hI⟩ := hr i I hri
      rw [hw] at hw'
      obtain rfl : w = w' := Option.some.inj hw'
      simp only [Bool.or_eq_true] at h
      rcases h with h | h
      · split at h
        case _ b a hs hl =>
          simp only [decide_eq_true_eq] at h
          exact no_room (hI.1 a hl) (hb.2 b hs) h
        case _ => exact absurd h (by simp)
      · split at h
        case _ a b hs hl =>
          simp only [decide_eq_true_eq] at h
          exact no_room (hb.1 a hs) (hI.2 b hl) h
        case _ => exact absurd h (by simp)
    case _ => exact absurd h (by simp)
  case _ => exact absurd h (by simp)

/-- **One point is ruled out**: some constraint, or some derived column, leaves it no
    values. This is the test §15.98 is about — the sieve asks about a point, and a leaf of
    the cover stands for a whole box. -/
theorem not_asked_of_never {s : Sieve} {p : Point} (h : neverRulesOut s p = true) :
    ¬ s.asked p := by
  simp only [neverRulesOut, List.any_eq_true, beq_iff_eq] at h
  rcases h with ⟨q, hq, hpq⟩
  rintro ⟨_, _, _, _, hnv, _⟩
  exact hnv q hq hpq

theorem not_asked_of_apart {s : Sieve} {p : Point} (h : apartRulesOut s p = true) :
    ¬ s.asked p := by
  simp only [apartRulesOut, List.any_eq_true, Bool.and_eq_true, beq_iff_eq] at h
  rcases h with ⟨qr, hqr, hq, hr⟩
  rintro ⟨_, _, _, _, _, hap, _⟩
  exact hap qr hqr ⟨hq, hr⟩

def pointRuledOut (s : Sieve) (p : Point) : Bool :=
  s.cons.any (fun k => constraintRulesOut s k p) ||
    (List.range p.length).any (fun i => derivedRulesOut s i p) ||
    neverRulesOut s p || apartRulesOut s p

theorem not_asked_of_pointRuledOut {s : Sieve} {p : Point} (h : pointRuledOut s p = true) :
    ¬ s.asked p := by
  simp only [pointRuledOut, Bool.or_eq_true, List.any_eq_true] at h
  rcases h with ((⟨k, hk, hkr⟩ | ⟨i, _, hir⟩) | hn) | hab
  · exact not_asked_of_constraint hk hkr
  · exact not_asked_of_derived hir
  · exact not_asked_of_never (by simpa [neverRulesOut] using hn)
  · exact not_asked_of_apart (by simpa [apartRulesOut] using hab)


/-! ## A whole box, not one corner of it

A leaf of the cover stands for every point below a path, and the sieve is a question about
a whole point. Two of its answers survive the gap — a constraint or a derive that already
bites on the coordinates the path has fixed bites on every point below it — and where
neither does, the points are asked about one at a time. That last case is §15.98: reading
the leaf as a claim about the path padded with zeros is what made a gap look covered. -/

theorem lt_of_getElem? {α : Type _} {l : List α} {i : Nat} {a : α} (h : l[i]? = some a) :
    i < l.length := by
  rcases Nat.lt_or_ge i l.length with h' | h'
  · exact h'
  · rw [List.getElem?_eq_none h'] at h; simp at h

theorem constraintRulesOut_mono {s : Sieve} {k : Constraint} {path p : Point}
    (hpre : path <+: p) (h : constraintRulesOut s k path = true) :
    constraintRulesOut s k p = true := by
  unfold constraintRulesOut at h ⊢
  split at h
  case _ cl cr hpl hpr =>
    rw [prefix_getElem? hpre (lt_of_getElem? hpl), prefix_getElem? hpre (lt_of_getElem? hpr),
      hpl, hpr]
    exact h
  case _ => exact absurd h (by simp)

theorem derivedRulesOut_mono {s : Sieve} {i : Nat} {path p : Point}
    (hpre : path <+: p) (h : derivedRulesOut s i path = true) :
    derivedRulesOut s i p = true := by
  unfold derivedRulesOut at h ⊢
  split at h
  case _ c I hp hri => rw [prefix_getElem? hpre (lt_of_getElem? hp), hp, hri]; exact h
  case _ => exact absurd h (by simp)

theorem neverRulesOut_mono {s : Sieve} {path p : Point} (hpre : path <+: p)
    (h : neverRulesOut s path = true) : neverRulesOut s p = true := by
  simp only [neverRulesOut, List.any_eq_true, beq_iff_eq] at h ⊢
  rcases h with ⟨q, hq, hpq⟩
  exact ⟨q, hq, by rw [prefix_getElem? hpre (lt_of_getElem? hpq)]; exact hpq⟩

theorem apartRulesOut_mono {s : Sieve} {path p : Point} (hpre : path <+: p)
    (h : apartRulesOut s path = true) : apartRulesOut s p = true := by
  simp only [apartRulesOut, List.any_eq_true, Bool.and_eq_true, beq_iff_eq] at h ⊢
  rcases h with ⟨qr, hqr, hq, hr⟩
  exact ⟨qr, hqr, by rw [prefix_getElem? hpre (lt_of_getElem? hq)]; exact hq,
         by rw [prefix_getElem? hpre (lt_of_getElem? hr)]; exact hr⟩

theorem pointRuledOut_mono {s : Sieve} {path p : Point} (hpre : path <+: p)
    (h : pointRuledOut s path = true) : pointRuledOut s p = true := by
  simp only [pointRuledOut, Bool.or_eq_true, List.any_eq_true, List.mem_range] at h ⊢
  rcases h with ((⟨k, hk, hkr⟩ | ⟨i, hi, hir⟩) | hn) | hab
  · exact Or.inl (Or.inl (Or.inl ⟨k, hk, constraintRulesOut_mono hpre hkr⟩))
  · exact Or.inl (Or.inl (Or.inr ⟨i, Nat.lt_of_lt_of_le hi hpre.length_le, derivedRulesOut_mono hpre hir⟩))
  · exact Or.inl (Or.inr (by simpa [neverRulesOut] using neverRulesOut_mono hpre (by simpa [neverRulesOut] using hn)))
  · exact Or.inr (by simpa [apartRulesOut] using apartRulesOut_mono hpre (by simpa [apartRulesOut] using hab))

/-- Every point the path opens onto. The walk stops where the axes do, so the list is the
    box below the path and nothing else. -/
def completions (arities : List Arity) (path : Point) : List Point :=
  match h : arities[path.length]? with
  | none => [path]
  | some n => (List.range n).flatMap (fun c => completions arities (path ++ [c]))
termination_by arities.length - path.length
decreasing_by
  have hlt : path.length < arities.length := lt_of_getElem? h
  simp only [List.length_append, List.length_cons, List.length_nil]
  omega

theorem mem_completions {arities : List Arity} {path p : Point}
    (hpre : path <+: p) (hsp : inSpace arities p = true) : p ∈ completions arities path := by
  fun_induction completions arities path with
  | case1 path h =>
    have hp : p.length = arities.length := inSpace_length hsp
    have hge : arities.length ≤ path.length := by
      rcases Nat.lt_or_ge path.length arities.length with h' | h'
      · rw [List.getElem?_eq_some_iff.2 ⟨h', rfl⟩] at h; exact absurd h (by simp)
      · exact h'
    have hle : path.length ≤ p.length := hpre.length_le
    have : path.length = p.length := by omega
    rw [hpre.eq_of_length this]
    simp
  | case2 path n h ih =>
    obtain ⟨c, hpc, hcn⟩ := inSpace_getElem hsp h
    simp only [List.mem_flatMap]
    exact ⟨c, List.mem_range.2 hcn, ih c (prefix_extend hpre hpc)⟩

/-- **A box is ruled out**: either what the path has already fixed rules it out on its own,
    or every point below the path is ruled out one at a time. -/
def boxRuledOut (s : Sieve) (arities : List Arity) (path : Point) : Bool :=
  pointRuledOut s path || (completions arities path).all (pointRuledOut s)

theorem not_asked_of_boxRuledOut {s : Sieve} {arities : List Arity} {path p : Point}
    (h : boxRuledOut s arities path = true) (hpre : path <+: p)
    (hsp : inSpace arities p = true) : ¬ s.asked p := by
  simp only [boxRuledOut, Bool.or_eq_true, List.all_eq_true] at h
  rcases h with h | h
  · exact not_asked_of_pointRuledOut (pointRuledOut_mono hpre h)
  · exact not_asked_of_pointRuledOut (h p (mem_completions hpre hsp))


/-! ## A point the rule really is asked about

`Table.asked` is an existential, so a reach witness has to *exhibit* the values, not just
name the coordinates. The certificate carries them, and the check below is the definition
read as a test. -/

def coordHolds : Coord → Rat → Bool
  | .exactly a, v => decide (v = a)
  | .between lo hi, v =>
      (match lo with | some a => decide (a < v) | none => true) &&
      (match hi with | some b => decide (v < b) | none => true)

theorem holds_of_coordHolds {x : Coord} {v : Rat} (h : coordHolds x v = true) : x.holds v := by
  cases x with
  | exactly a => simpa [coordHolds, Coord.holds] using h
  | between lo hi =>
    simp only [coordHolds, Bool.and_eq_true] at h
    refine ⟨?_, ?_⟩
    · intro a ha; rw [ha] at h; simpa using h.1
    · intro b hb; rw [hb] at h; simpa using h.2

def ivalHolds (I : Ival) (v : Rat) : Bool :=
  (match I.1 with | some a => decide (a ≤ v) | none => true) &&
  (match I.2 with | some b => decide (v ≤ b) | none => true)

theorem inIval_of_ivalHolds {I : Ival} {v : Rat} (h : ivalHolds I v = true) : inIval I v := by
  simp only [ivalHolds, Bool.and_eq_true] at h
  refine ⟨?_, ?_⟩
  · intro a ha; rw [ha] at h; simpa using h.1
  · intro b hb; rw [hb] at h; simpa using h.2

def cmpHolds : Cmp → Rat → Rat → Bool
  | .le, x, y => decide (x ≤ y)
  | .lt, x, y => decide (x < y)
  | .ge, x, y => decide (y ≤ x)
  | .gt, x, y => decide (y < x)
  | .eq, x, y => decide (x = y)

theorem holds_of_cmpHolds {op : Cmp} {x y : Rat} (h : cmpHolds op x y = true) : op.holds x y := by
  cases op <;> simpa [cmpHolds, Cmp.holds] using h

/-- **The values behind a point check out**: each one sits in the coordinate it stands for
    and inside its column's reach, and together they satisfy every constraint. -/
def witnessOk (s : Sieve) (p : Point) (v : List Rat) : Bool :=
  (List.range p.length).all (fun i =>
    match p[i]?, v[i]? with
    | some c, some w =>
      match s.coordAt i c with
      | some x => coordHolds x w
      | none => true
    | _, _ => false) &&
  (List.range s.reach.length).all (fun i =>
    match s.reach[i]?, v[i]? with
    | some (some I), some w => ivalHolds I w
    | some none, _ => true
    | _, _ => false) &&
  s.cons.all (fun k =>
    match v[k.left]?, v[k.right]? with
    | some x, some y => cmpHolds k.op x y
    | _, _ => false) &&
  s.never.all (fun q => !(p[q.1]? == some q.2)) &&
  s.apart.all (fun qr => !(p[qr.1.1]? == some qr.1.2 && p[qr.2.1]? == some qr.2.2)) &&
  s.facts.all (fun q => q.holdsB v)

theorem asked_of_witnessOk {s : Sieve} {p : Point} {v : List Rat}
    (h : witnessOk s p v = true) : s.asked p := by
  simp only [witnessOk, Bool.and_eq_true] at h
  obtain ⟨⟨⟨⟨⟨hco, hre⟩, hcs⟩, hnv⟩, hap⟩, hfa⟩ := h
  refine ⟨v, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro i c x hp hx
    have hi := List.all_eq_true.1 hco i (List.mem_range.2 (lt_of_getElem? hp))
    cases hv : v[i]? with
    | none => simp [hp, hv] at hi
    | some w =>
      simp only [hp, hv, hx] at hi
      exact ⟨w, rfl, holds_of_coordHolds hi⟩
  · intro k hk
    have hkk := List.all_eq_true.1 hcs k hk
    cases hl : v[k.left]? with
    | none => simp [hl] at hkk
    | some x =>
      cases hr : v[k.right]? with
      | none => simp [hl, hr] at hkk
      | some y =>
        simp only [hl, hr] at hkk
        exact ⟨x, y, rfl, rfl, holds_of_cmpHolds hkk⟩
  · intro i I hI
    have hi := List.all_eq_true.1 hre i (List.mem_range.2 (lt_of_getElem? hI))
    cases hv : v[i]? with
    | none => simp [hI, hv] at hi
    | some w =>
      simp only [hI, hv] at hi
      exact ⟨w, rfl, inIval_of_ivalHolds hi⟩
  · intro q hq
    have := List.all_eq_true.1 hnv q hq
    simpa using this
  · intro qr hqr hboth
    have := List.all_eq_true.1 hap qr hqr
    simp only [Bool.not_eq_true', Bool.and_eq_false_iff, beq_eq_false_iff_ne] at this
    rcases this with h1 | h1
    · exact h1 hboth.1
    · exact h1 hboth.2
  · intro q hq
    exact LinIneq.holds_of_holdsB (List.all_eq_true.1 hfa q hq)

/-! ## A box the linear model leaves no values in (§15.141)

A refutation names its inequalities by where each comes from — a fact of the sieve's model,
or one end of the coordinates the box allows on an axis — and gives each a multiplier. The
inequalities are built again here, from the sieve and the box, and `farkasOk` adds them up.
A fact holds at every point the rule is asked about because `asked` says so; an end holds
at every point of the box because every coordinate the box allows on that axis is checked
against it. -/

/-- Where one inequality of a refutation comes from. -/
inductive Ref where
  /-- The fact of the model at this index. -/
  | fact : Nat → Ref
  /-- An end of the coordinates the box allows on an axis: `v[axis] ≥ at` below, or
      `v[axis] ≤ at` above, strict where the end is left out. -/
  | coord : (axis : Nat) → (hi : Bool) → (at_ : Rat) → (opn : Bool) → Ref
  deriving Repr, Inhabited

/-- Whether every value of a coordinate lies on the right side of an end. -/
def beyondEnd (x : Coord) (hi : Bool) (at_ : Rat) (opn : Bool) : Bool :=
  match x, hi with
  | .exactly a, false => decide (at_ < a) || (decide (a = at_) && !opn)
  | .exactly a, true => decide (a < at_) || (decide (a = at_) && !opn)
  | .between (some l) _, false => decide (at_ ≤ l)
  | .between _ (some h), true => decide (h ≤ at_)
  | _, _ => false

/-- The inequality an end stands for: `−v[i] + at ≤ 0` below, `v[i] − at ≤ 0` above, strict
    where the end is left out. -/
def endIneq (i : Nat) (hi : Bool) (at_ : Rat) (opn : Bool) : LinIneq :=
  if hi then { coeffs := unitAt i 1, k := -at_, strict := opn }
  else { coeffs := unitAt i (-1), k := at_, strict := opn }

/-- Whether an end holds of every coordinate a box allows on its axis. -/
def endHolds (s : Sieve) (box : Box) (i : Nat) (hi : Bool) (at_ : Rat) (opn : Bool) : Bool :=
  match box[i]? with
  | some cs => cs.all (fun c => match s.coordAt i c with
      | some x => beyondEnd x hi at_ opn
      | none => false)
  | none => false

/-- The inequalities a refutation names, built again from the sieve and the box; `none`
    where a fact is not there or an end does not hold. -/
def refIneqs (s : Sieve) (box : Box) : List (Ref × Rat) → Option (List (Rat × LinIneq))
  | [] => some []
  | (r, y) :: rest =>
    match refIneqs s box rest with
    | none => none
    | some qs =>
      match r with
      | .fact i => (s.facts[i]?).map (fun q => (y, q) :: qs)
      | .coord i hi at_ opn =>
        if endHolds s box i hi at_ opn then some ((y, endIneq i hi at_ opn) :: qs) else none

/-- **The linear model leaves the box no values**: the inequalities the refutation names are
    what it says they are, and `farkasOk` accepts their sum. -/
def farkasRuledOut (s : Sieve) (box : Box) (refs : List (Ref × Rat)) : Bool :=
  match refIneqs s box refs with
  | some ps => farkasOk ps
  | none => false

theorem getD_of_getElem? {v : List Rat} {i : Nat} {w : Rat} (h : v[i]? = some w) : v.getD i 0 = w := by
  simp [List.getD, h]

/-- An end that holds of the coordinate a value stands in holds of the value. -/
theorem endIneq_holds {x : Coord} {w at_ : Rat} {hi opn : Bool} {i : Nat} {v : List Rat}
    (hb : beyondEnd x hi at_ opn = true) (hx : x.holds w) (hv : v.getD i 0 = w) :
    (endIneq i hi at_ opn).holds v := by
  unfold endIneq
  cases hi <;> simp only [ite_true, Bool.false_eq_true, ite_false] <;>
    unfold LinIneq.holds LinIneq.lhs <;> simp only [dot_unitAt, hv] <;>
    cases x with
    | exactly a =>
      simp only [Coord.holds] at hx
      subst hx
      simp only [beyondEnd, Bool.or_eq_true, Bool.and_eq_true, decide_eq_true_eq, Bool.not_eq_true'] at hb
      cases opn <;> simp only [ite_true, Bool.false_eq_true, ite_false] <;> rcases hb with hb | ⟨hb, ho⟩ <;> grind
    | between lo up =>
      obtain ⟨hlo, hup⟩ := hx
      first
      | (cases lo with
          | none => simp [beyondEnd] at hb
          | some l =>
            simp only [beyondEnd, decide_eq_true_eq] at hb
            have := hlo l rfl
            cases opn <;> simp only [ite_true, Bool.false_eq_true, ite_false] <;> grind)
      | (cases up with
          | none => simp [beyondEnd] at hb
          | some h =>
            simp only [beyondEnd, decide_eq_true_eq] at hb
            have := hup h rfl
            cases opn <;> simp only [ite_true, Bool.false_eq_true, ite_false] <;> grind)

/-- Every inequality `refIneqs` builds holds at the values behind a point of the box that the
    rule is asked about. -/
theorem refIneqs_hold {s : Sieve} {box : Box} {p : Point} {v : List Rat}
    (hin : inBox box p = true)
    (hf : ∀ (i c : Nat) (x : Coord), p[i]? = some c → s.coordAt i c = some x →
      ∃ w, v[i]? = some w ∧ x.holds w)
    (hfa : ∀ q ∈ s.facts, q.holds v) :
    ∀ (refs : List (Ref × Rat)) (ps : List (Rat × LinIneq)), refIneqs s box refs = some ps →
      ∀ q ∈ ps, q.2.holds v
  | [], ps, h => by simp only [refIneqs, Option.some.injEq] at h; subst h; simp
  | (r, y) :: rest, ps, h => by
    simp only [refIneqs] at h
    split at h
    · exact absurd h (by simp)
    · rename_i qs hqs
      have ih := refIneqs_hold hin hf hfa rest qs hqs
      cases r with
      | fact i =>
        simp only [Option.map_eq_some_iff] at h
        obtain ⟨q, hq, rfl⟩ := h
        intro e he
        simp only [List.mem_cons] at he
        rcases he with rfl | he
        · exact hfa q (List.mem_of_getElem? hq)
        · exact ih e he
      | coord i hi at_ opn =>
        by_cases hend : endHolds s box i hi at_ opn = true
        · simp only [hend, ite_true, Option.some.injEq] at h
          subst h
          intro e he
          simp only [List.mem_cons] at he
          rcases he with rfl | he
          · unfold endHolds at hend
            split at hend
            · rename_i cs hcs
              obtain ⟨c, hpc, hmem⟩ := inBox_getElem hin hcs
              have hall := List.all_eq_true.1 hend c (List.contains_iff_mem.1 hmem)
              split at hall
              · rename_i x hx
                obtain ⟨w, hw, hxw⟩ := hf i c x hpc hx
                exact endIneq_holds hall hxw (getD_of_getElem? hw)
              · exact absurd hall (by simp)
            · exact absurd hend (by simp)
          · exact ih e he
        · simp only [hend, Bool.false_eq_true, ite_false] at h
          exact absurd h (by simp)

/-- **A box the linear model leaves no values in holds no point the rule is asked about.** -/
theorem not_asked_of_farkas {s : Sieve} {box : Box} {refs : List (Ref × Rat)} {p : Point}
    (h : farkasRuledOut s box refs = true) (hin : inBox box p = true) : ¬ s.asked p := by
  rintro ⟨v, hf, _, _, _, _, hfa⟩
  unfold farkasRuledOut at h
  split at h
  · rename_i ps hps
    exact farkas_sound h v (refIneqs_hold hin hf hfa refs ps hps)
  · exact absurd h (by simp)

/-! ## The boxes a refutation speaks about -/

/-- The box a path fixes: the coordinate it takes on each axis it has fixed, every one on the
    axes it has not. -/
def pathBox (arities : List Arity) (path : Point) : Box :=
  path.map (fun c => [c]) ++ (arities.drop path.length).map List.range

theorem inBox_pathBox {arities : List Arity} {path p : Point} (hpre : path <+: p)
    (hsp : inSpace arities p = true) : inBox (pathBox arities path) p = true := by
  have hlen := inSpace_length hsp
  have hle : path.length ≤ arities.length := by have := hpre.length_le; omega
  apply inBox_of_getElem
  · simp [pathBox]; omega
  · intro i xs c hb hc
    unfold pathBox at hb
    rcases Nat.lt_or_ge i path.length with hi | hi
    · rw [List.getElem?_append_left (by simpa using hi)] at hb
      simp only [List.getElem?_map] at hb
      have hpi := prefix_getElem? hpre hi
      rw [hc] at hpi
      rw [← hpi] at hb
      simp only [Option.map_some, Option.some.injEq] at hb
      subst hb
      simp
    · rw [List.getElem?_append_right (by simpa using hi)] at hb
      simp only [List.getElem?_map, List.length_map, List.getElem?_drop] at hb
      have hj : path.length + (i - path.length) = i := by omega
      rw [hj] at hb
      cases hn : arities[i]? with
      | none => rw [hn] at hb; simp at hb
      | some n =>
        rw [hn] at hb
        simp only [Option.map_some, Option.some.injEq] at hb
        subst hb
        obtain ⟨c', hc', hlt⟩ := inSpace_getElem hsp hn
        rw [hc] at hc'
        simp only [Option.some.injEq] at hc'
        subst hc'
        simpa using hlt

/-- The coordinates two boxes both take on each axis. -/
def pairBox (a b : Box) : Box :=
  List.zipWith (fun xs ys => xs.filter (fun c => ys.contains c)) a b

theorem inBox_pairBox : ∀ {a b : Box} {p : Point}, inBox a p = true → inBox b p = true →
    inBox (pairBox a b) p = true
  | [], [], [], _, _ => by simp [pairBox, inBox]
  | x :: xs, y :: ys, c :: cs, ha, hb => by
    simp only [inBox, Bool.and_eq_true] at ha hb
    simp only [pairBox, List.zipWith_cons_cons, inBox, Bool.and_eq_true]
    refine ⟨?_, inBox_pairBox ha.2 hb.2⟩
    simp only [List.contains_iff_mem, List.mem_filter] at ha hb ⊢
    exact ⟨ha.1, by simpa using hb.1⟩
  | [], [], _ :: _, ha, _ => by simp [inBox] at ha
  | [], _ :: _, [], _, hb => by simp [inBox] at hb
  | [], _ :: _, _ :: _, ha, _ => by simp [inBox] at ha
  | _ :: _, [], [], ha, _ => by simp [inBox] at ha
  | _ :: _, [], _ :: _, _, hb => by simp [inBox] at hb
  | _ :: _, _ :: _, [], ha, _ => by simp [inBox] at ha

end RulecCert
