/-
  Which combinations the rule is asked about, and how the certificate settles it (§6.2,
  §15.55, §15.98).

  `RulecCert.Semantics` leaves `Table.asked` abstract, and `RulecCert.Sound` takes it on
  hypothesis: a box the cover calls impossible really is one no input reaches. This file is
  where that hypothesis is discharged. A point is asked about when there are values behind
  its coordinates — one per axis, each inside the coordinate it names — that satisfy every
  `constraint` the rule declares, put every derived column and every column of a `define` of a
  number inside the interval its own expression is forced into, and, where a column decided by
  a table above holds a value the certificate rests on the rows of, let one of the rows that
  write that value fire (rulec's §15.195).

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

/-- One row of a table above that writes the value a column of this table holds (rulec's
    §15.195), read on this table: what its box says about the values this table numbers — for
    each numbered value it bounds, the coordinates the row takes on that value's axis in its own
    table — and about this table's own axes of words both tables cut — for each, the coordinates
    of this table's axis the row lets in. The row fires only where both hold, so a value decided
    above arrives only where one of its rows does. -/
structure AboveRow where
  /-- The row's number in its own table, for a reader. -/
  index : Nat
  nums : List (Nat × List Coord)
  words : List (Nat × List Nat)
  deriving Repr, Inhabited

/-- A column a table above decides, at one of its values: the coordinate `coord` of axis `axis`
    holds only where one of `rows` — every row of that table that writes the value — fires. -/
structure AboveCol where
  axis : Nat
  coord : Nat
  rows : List AboveRow
  deriving Repr, Inhabited

/-- A boolean `define` of one comparison, read on its axis (rulec's §15.196): the comparison, the
    intervals its two sides are forced into (`RulecCert.interval`, or a date's day number), and the
    coordinate of the axis that stands for `true`. The define is true exactly where its two sides
    compare that way. -/
structure Truth where
  op : Cmp
  l : Rat × Rat
  r : Rat × Rat
  trueAt : Nat
  deriving Repr, Inhabited

/-- Everything about a table that decides whether a combination can arrive: the values each
    coordinate stands for, the constraints, and — for a column the rule computes, a derive or
    a `define` of a number — the interval its expression is forced into. -/
structure Sieve where
  /-- One entry per axis, one per coordinate. `none` where the coordinate stands for no
      number — an enum, a flag, the absent value of an optional column — which is also
      every coordinate no constraint and no derive can speak about. -/
  coords : List (List (Option Coord))
  cons : List Constraint
  /-- One entry per axis: the interval the expression of a derived or `define` column is forced
      into (`RulecCert.interval`, which `eval_mem_interval` shows holds the value), `none` on an
      axis of inputs or of a value decided above. -/
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
  /-- One entry per axis: the days an input takes when its range is a date of a koyomi file
      (`range from koyomi`, rulec's §15.174), as day numbers; `none` on every other axis. A
      value no day of the list equals is one no caller sends: the generated code refuses it. -/
  days : List (Option (List Rat)) := []
  /-- The values decided above that the certificate rests on the rows of (rulec's §15.195):
      at each, the rows of the table above that write it, read on this table. -/
  above : List AboveCol := []
  /-- One entry per axis: for a boolean `define` of one comparison whose two sides have intervals,
      the comparison read on the axis (rulec's §15.196); `none` on every other axis. -/
  truth : List (Option Truth) := []

def Sieve.coordAt (s : Sieve) (i c : Nat) : Option Coord :=
  match s.coords[i]? with
  | none => none
  | some cs => match cs[c]? with
               | none => none
               | some x => x

/-- A row of a table above fires on these values at this point: every value it bounds lies in
    one of the coordinates it takes, and this table's axis of words holds a word it lets in. -/
def AboveRow.fires (r : AboveRow) (p : Point) (v : List Rat) : Prop :=
  (∀ q ∈ r.nums, ∃ w, v[q.1]? = some w ∧ ∃ x ∈ q.2, x.holds w) ∧
  (∀ q ∈ r.words, ∃ c, p[q.1]? = some c ∧ c ∈ q.2)

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
    (∀ q ∈ s.facts, q.holds v) ∧
    (∀ (i : Nat) (D : List Rat), s.days[i]? = some (some D) → ∃ w, v[i]? = some w ∧ w ∈ D) ∧
    (∀ e ∈ s.above, p[e.axis]? = some e.coord → ∃ r ∈ e.rows, r.fires p v) ∧
    (∀ (i : Nat) (T : Truth), s.truth[i]? = some (some T) → ∀ c, p[i]? = some c →
      ∃ x y, T.l.1 ≤ x ∧ x ≤ T.l.2 ∧ T.r.1 ≤ y ∧ y ≤ T.r.2 ∧ (c = T.trueAt ↔ T.op.holds x y))

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

/-- A derived or `define` column whose coordinate lies outside what its own expression can
    produce. An interval coordinate leaves its ends out, so one that starts where the reach ends
    lies outside it too (rulec's §15.195). -/
def derivedRulesOut (s : Sieve) (i : Nat) (p : Point) : Bool :=
  match p[i]?, s.reach[i]? with
  | some c, some (some I) =>
    match s.coordAt i c with
    | some x =>
      (match x.span.2, I.1 with | some b, some a => decide (b < a) || (x.opn && decide (b ≤ a)) | _, _ => false) ||
      (match x.span.1, I.2 with | some a, some b => decide (b < a) || (x.opn && decide (b ≤ a)) | _, _ => false)
    | none => false
  | _, _ => false

/-- The comparison holds of every pair of values the two sides can take. -/
def Truth.always (T : Truth) : Bool :=
  match T.op with
  | .le => decide (T.l.2 ≤ T.r.1)
  | .lt => decide (T.l.2 < T.r.1)
  | .ge => decide (T.r.2 ≤ T.l.1)
  | .gt => decide (T.r.2 < T.l.1)
  | .eq => false

/-- The comparison holds of no pair of values the two sides can take. -/
def Truth.never (T : Truth) : Bool :=
  match T.op with
  | .le => decide (T.r.2 < T.l.1)
  | .lt => decide (T.r.2 ≤ T.l.1)
  | .ge => decide (T.l.2 < T.r.1)
  | .gt => decide (T.l.2 ≤ T.r.1)
  | .eq => false

theorem Truth.always_sound {T : Truth} {x y : Rat} (h : T.always = true)
    (hx2 : x ≤ T.l.2) (hx1 : T.l.1 ≤ x) (hy1 : T.r.1 ≤ y) (hy2 : y ≤ T.r.2) : T.op.holds x y := by
  cases hop : T.op with
  | le =>
    simp only [Truth.always, hop, decide_eq_true_eq] at h
    exact Rat.le_trans hx2 (Rat.le_trans h hy1)
  | lt =>
    simp only [Truth.always, hop, decide_eq_true_eq] at h
    exact lelt hx2 (ltle h hy1)
  | ge =>
    simp only [Truth.always, hop, decide_eq_true_eq] at h
    exact Rat.le_trans hy2 (Rat.le_trans h hx1)
  | gt =>
    simp only [Truth.always, hop, decide_eq_true_eq] at h
    exact lelt hy2 (ltle h hx1)
  | eq => simp [Truth.always, hop] at h

theorem Truth.never_sound {T : Truth} {x y : Rat} (h : T.never = true)
    (hx1 : T.l.1 ≤ x) (hx2 : x ≤ T.l.2) (hy1 : T.r.1 ≤ y) (hy2 : y ≤ T.r.2) : ¬ T.op.holds x y := by
  cases hop : T.op with
  | le =>
    simp only [Truth.never, hop, decide_eq_true_eq] at h
    exact fun hxy => no_room (Rat.le_trans hx1 hxy) hy2 h
  | lt =>
    simp only [Truth.never, hop, decide_eq_true_eq] at h
    exact fun hxy => Rat.lt_irrefl (ltle (lelt hx1 (ltle hxy hy2)) h)
  | ge =>
    simp only [Truth.never, hop, decide_eq_true_eq] at h
    exact fun hyx => no_room (Rat.le_trans hy1 hyx) hx2 h
  | gt =>
    simp only [Truth.never, hop, decide_eq_true_eq] at h
    exact fun hyx => Rat.lt_irrefl (ltle (lelt hy1 (ltle hyx hx2)) h)
  | eq => simp [Truth.never, hop] at h

/-- The axis of a boolean `define` at the truth value its comparison never takes over the
    intervals of its two sides (rulec's §15.196). -/
def truthRulesOut (s : Sieve) (i : Nat) (p : Point) : Bool :=
  match p[i]?, s.truth[i]? with
  | some c, some (some T) => if c = T.trueAt then T.never else T.always
  | _, _ => false

/-- **A boolean `define` never stands at a truth value its comparison cannot take.** -/
theorem not_asked_of_truth {s : Sieve} {i : Nat} {p : Point}
    (h : truthRulesOut s i p = true) : ¬ s.asked p := by
  rintro ⟨v, _, _, _, _, _, _, _, _, ht⟩
  unfold truthRulesOut at h
  split at h
  case _ c T hp hT =>
    obtain ⟨x, y, hx1, hx2, hy1, hy2, hiff⟩ := ht i T hT c hp
    by_cases hc : c = T.trueAt
    · simp only [hc, ↓reduceIte] at h
      exact Truth.never_sound h hx1 hx2 hy1 hy2 (hiff.1 hc)
    · simp only [hc, ↓reduceIte] at h
      exact hc (hiff.2 (Truth.always_sound h hx2 hx1 hy1 hy2))
  case _ => exact absurd h (by simp)

/-- A coordinate no table above ever writes. -/
def neverRulesOut (s : Sieve) (p : Point) : Bool :=
  s.never.any (fun q => p[q.1]? == some q.2)

/-- Two coordinates the tables above cannot hold at once, both of them held here. -/
def apartRulesOut (s : Sieve) (p : Point) : Bool :=
  s.apart.any (fun qr => p[qr.1.1]? == some qr.1.2 && p[qr.2.1]? == some qr.2.2)

/-- Whether a coordinate takes this value, read exactly: one value, or strictly between the
    two boundaries. -/
def coordHoldsB : Coord → Rat → Bool
  | .exactly a, v => decide (v = a)
  | .between lo hi, v =>
      (match lo with | some a => decide (a < v) | none => true) &&
      (match hi with | some b => decide (v < b) | none => true)

theorem coordHoldsB_of_holds {x : Coord} {v : Rat} (h : x.holds v) : coordHoldsB x v = true := by
  cases x with
  | exactly a =>
    simp only [Coord.holds] at h
    simp [coordHoldsB, h]
  | between lo hi =>
    obtain ⟨h1, h2⟩ := h
    simp only [coordHoldsB, Bool.and_eq_true]
    constructor
    · cases lo with
      | none => rfl
      | some a => simpa using h1 a rfl
    · cases hi with
      | none => rfl
      | some b => simpa using h2 b rfl

/-- An axis of koyomi's days whose coordinate takes none of them (rulec's §15.174). -/
def daysRulesOut (s : Sieve) (i : Nat) (p : Point) : Bool :=
  match p[i]?, s.days[i]? with
  | some c, some (some D) =>
    match s.coordAt i c with
    | some x => D.all (fun d => !coordHoldsB x d)
    | none => false
  | _, _ => false

theorem not_asked_of_days {s : Sieve} {i : Nat} {p : Point}
    (h : daysRulesOut s i p = true) : ¬ s.asked p := by
  rintro ⟨v, hf, _, _, _, _, _, hd, _⟩
  unfold daysRulesOut at h
  split at h
  case _ c D hp hD =>
    split at h
    case _ x hx =>
      obtain ⟨w, hw, hh⟩ := hf i c x hp hx
      obtain ⟨w', hw', hmem⟩ := hd i D hD
      rw [hw] at hw'
      obtain rfl : w = w' := Option.some.inj hw'
      have hall := List.all_eq_true.1 h w hmem
      rw [coordHoldsB_of_holds hh] at hall
      exact absurd hall (by simp)
    case _ => exact absurd h (by simp)
  case _ => exact absurd h (by simp)

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
      obtain ⟨w, hw, hh⟩ := hf i c x hp hc
      have hb := Coord.holds_span hh
      obtain ⟨w', hw', hI⟩ := hr i I hri
      rw [hw] at hw'
      obtain rfl : w = w' := Option.some.inj hw'
      simp only [Bool.or_eq_true] at h
      rcases h with h | h
      · split at h
        case _ b a hs hl =>
          simp only [Bool.or_eq_true, Bool.and_eq_true, decide_eq_true_eq] at h
          rcases h with h | ⟨ho, h⟩
          · exact no_room (hI.1 a hl) (hb.2 b hs) h
          · -- a ≤ w < b ≤ a
            exact Rat.lt_irrefl (lelt (hI.1 a hl) (ltle (Coord.holds_hi_lt hh ho hs) h))
        case _ => exact absurd h (by simp)
      · split at h
        case _ a b hs hl =>
          simp only [Bool.or_eq_true, Bool.and_eq_true, decide_eq_true_eq] at h
          rcases h with h | ⟨ho, h⟩
          · exact no_room (hb.1 a hs) (hI.2 b hl) h
          · -- b ≤ a < w ≤ b
            exact Rat.lt_irrefl (ltle (lelt h (Coord.holds_lo_lt hh ho hs)) (hI.2 b hl))
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
    neverRulesOut s p || apartRulesOut s p ||
    (List.range p.length).any (fun i => daysRulesOut s i p) ||
    (List.range p.length).any (fun i => truthRulesOut s i p)

theorem not_asked_of_pointRuledOut {s : Sieve} {p : Point} (h : pointRuledOut s p = true) :
    ¬ s.asked p := by
  simp only [pointRuledOut, Bool.or_eq_true, List.any_eq_true] at h
  rcases h with ((((⟨k, hk, hkr⟩ | ⟨i, _, hir⟩) | hn) | hab) | ⟨i, _, hdy⟩) | ⟨i, _, htr⟩
  · exact not_asked_of_constraint hk hkr
  · exact not_asked_of_derived hir
  · exact not_asked_of_never (by simpa [neverRulesOut] using hn)
  · exact not_asked_of_apart (by simpa [apartRulesOut] using hab)
  · exact not_asked_of_days hdy
  · exact not_asked_of_truth htr


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

theorem daysRulesOut_mono {s : Sieve} {i : Nat} {path p : Point}
    (hpre : path <+: p) (h : daysRulesOut s i path = true) :
    daysRulesOut s i p = true := by
  unfold daysRulesOut at h ⊢
  split at h
  case _ c D hp hD => rw [prefix_getElem? hpre (lt_of_getElem? hp), hp, hD]; exact h
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

theorem truthRulesOut_mono {s : Sieve} {i : Nat} {path p : Point}
    (hpre : path <+: p) (h : truthRulesOut s i path = true) :
    truthRulesOut s i p = true := by
  unfold truthRulesOut at h ⊢
  split at h
  case _ c T hp hT => rw [prefix_getElem? hpre (lt_of_getElem? hp), hp, hT]; exact h
  case _ => exact absurd h (by simp)

theorem pointRuledOut_mono {s : Sieve} {path p : Point} (hpre : path <+: p)
    (h : pointRuledOut s path = true) : pointRuledOut s p = true := by
  simp only [pointRuledOut, Bool.or_eq_true, List.any_eq_true, List.mem_range] at h ⊢
  rcases h with ((((⟨k, hk, hkr⟩ | ⟨i, hi, hir⟩) | hn) | hab) | ⟨i, hi, hdy⟩) | ⟨i, hi, htr⟩
  · exact Or.inl (Or.inl (Or.inl (Or.inl (Or.inl ⟨k, hk, constraintRulesOut_mono hpre hkr⟩))))
  · exact Or.inl (Or.inl (Or.inl (Or.inl (Or.inr ⟨i, Nat.lt_of_lt_of_le hi hpre.length_le, derivedRulesOut_mono hpre hir⟩))))
  · exact Or.inl (Or.inl (Or.inl (Or.inr (by simpa [neverRulesOut] using neverRulesOut_mono hpre (by simpa [neverRulesOut] using hn)))))
  · exact Or.inl (Or.inl (Or.inr (by simpa [apartRulesOut] using apartRulesOut_mono hpre (by simpa [apartRulesOut] using hab))))
  · exact Or.inl (Or.inr ⟨i, Nat.lt_of_lt_of_le hi hpre.length_le, daysRulesOut_mono hpre hdy⟩)
  · exact Or.inr ⟨i, Nat.lt_of_lt_of_le hi hpre.length_le, truthRulesOut_mono hpre htr⟩

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

/-- `AboveRow.fires` as a test. -/
def AboveRow.firesB (r : AboveRow) (p : Point) (v : List Rat) : Bool :=
  r.nums.all (fun q => match v[q.1]? with
    | some w => q.2.any (fun x => coordHolds x w)
    | none => false) &&
  r.words.all (fun q => match p[q.1]? with
    | some c => q.2.contains c
    | none => false)

theorem AboveRow.fires_of_firesB {r : AboveRow} {p : Point} {v : List Rat}
    (h : r.firesB p v = true) : r.fires p v := by
  simp only [AboveRow.firesB, Bool.and_eq_true, List.all_eq_true] at h
  obtain ⟨hn, hw⟩ := h
  refine ⟨?_, ?_⟩
  · intro q hq
    have hq' := hn q hq
    cases hv : v[q.1]? with
    | none => rw [hv] at hq'; exact absurd hq' (by simp)
    | some w =>
      rw [hv] at hq'
      simp only [List.any_eq_true] at hq'
      obtain ⟨x, hx, hxw⟩ := hq'
      exact ⟨w, rfl, x, hx, holds_of_coordHolds hxw⟩
  · intro q hq
    have hq' := hw q hq
    cases hc : p[q.1]? with
    | none => rw [hc] at hq'; exact absurd hq' (by simp)
    | some c =>
      rw [hc] at hq'
      exact ⟨c, rfl, by simpa using hq'⟩

/-- Some pair of values the two sides can take gives the comparison the truth value `b`: the two
    intervals are not empty, and the pair at the ends that suits `b` does. -/
def Truth.can (T : Truth) (b : Bool) : Bool :=
  decide (T.l.1 ≤ T.l.2) && decide (T.r.1 ≤ T.r.2) &&
  match T.op, b with
  | .le, true => decide (T.l.1 ≤ T.r.2)
  | .le, false => decide (T.r.1 < T.l.2)
  | .lt, true => decide (T.l.1 < T.r.2)
  | .lt, false => decide (T.r.1 ≤ T.l.2)
  | .ge, true => decide (T.r.1 ≤ T.l.2)
  | .ge, false => decide (T.l.1 < T.r.2)
  | .gt, true => decide (T.r.1 < T.l.2)
  | .gt, false => decide (T.l.1 ≤ T.r.2)
  | .eq, _ => false

theorem Truth.exists_of_can {T : Truth} {b : Bool} (h : T.can b = true) :
    ∃ x y, T.l.1 ≤ x ∧ x ≤ T.l.2 ∧ T.r.1 ≤ y ∧ y ≤ T.r.2 ∧ (b = true ↔ T.op.holds x y) := by
  unfold Truth.can at h
  simp only [Bool.and_eq_true, decide_eq_true_eq] at h
  obtain ⟨⟨hl, hr⟩, h⟩ := h
  cases hop : T.op <;> cases b <;> simp only [hop, decide_eq_true_eq] at h
  -- `le`, false: the high end of the left over the low end of the right
  · refine ⟨T.l.2, T.r.1, hl, Rat.le_refl, Rat.le_refl, hr, ?_⟩
    simp only [Cmp.holds, Bool.false_eq_true, false_iff]
    exact fun hle => Rat.lt_irrefl (ltle h hle)
  · exact ⟨T.l.1, T.r.2, Rat.le_refl, hl, hr, Rat.le_refl, by simp [Cmp.holds, h]⟩
  · refine ⟨T.l.2, T.r.1, hl, Rat.le_refl, Rat.le_refl, hr, ?_⟩
    simp only [Cmp.holds, Bool.false_eq_true, false_iff]
    exact fun hlt => Rat.lt_irrefl (lelt h hlt)
  · exact ⟨T.l.1, T.r.2, Rat.le_refl, hl, hr, Rat.le_refl, by simp [Cmp.holds, h]⟩
  · refine ⟨T.l.1, T.r.2, Rat.le_refl, hl, hr, Rat.le_refl, ?_⟩
    simp only [Cmp.holds, Bool.false_eq_true, false_iff]
    exact fun hle => Rat.lt_irrefl (ltle h hle)
  · exact ⟨T.l.2, T.r.1, hl, Rat.le_refl, Rat.le_refl, hr, by simp [Cmp.holds, h]⟩
  · refine ⟨T.l.1, T.r.2, Rat.le_refl, hl, hr, Rat.le_refl, ?_⟩
    simp only [Cmp.holds, Bool.false_eq_true, false_iff]
    exact fun hlt => Rat.lt_irrefl (lelt h hlt)
  · exact ⟨T.l.2, T.r.1, hl, Rat.le_refl, Rat.le_refl, hr, by simp [Cmp.holds, h]⟩
  · exact absurd h (by simp)
  · exact absurd h (by simp)

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
  s.facts.all (fun q => q.holdsB v) &&
  (List.range s.days.length).all (fun i =>
    match s.days[i]?, v[i]? with
    | some (some D), some w => decide (w ∈ D)
    | some none, _ => true
    | _, _ => false) &&
  s.above.all (fun e => !(p[e.axis]? == some e.coord) || e.rows.any (fun r => r.firesB p v)) &&
  (List.range p.length).all (fun i =>
    match p[i]?, s.truth[i]? with
    | some c, some (some T) => T.can (c == T.trueAt)
    | _, _ => true)

theorem asked_of_witnessOk {s : Sieve} {p : Point} {v : List Rat}
    (h : witnessOk s p v = true) : s.asked p := by
  simp only [witnessOk, Bool.and_eq_true] at h
  obtain ⟨⟨⟨⟨⟨⟨⟨⟨hco, hre⟩, hcs⟩, hnv⟩, hap⟩, hfa⟩, hdy⟩, hab⟩, htr⟩ := h
  refine ⟨v, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
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
  · intro i D hD
    have hi := List.all_eq_true.1 hdy i (List.mem_range.2 (lt_of_getElem? hD))
    cases hv : v[i]? with
    | none => simp [hD, hv] at hi
    | some w =>
      simp only [hD, hv] at hi
      exact ⟨w, rfl, of_decide_eq_true hi⟩
  · intro e he hp
    have he' := List.all_eq_true.1 hab e he
    rw [hp] at he'
    simp only [beq_self_eq_true, Bool.not_true, Bool.false_or, List.any_eq_true] at he'
    obtain ⟨r, hr, hf⟩ := he'
    exact ⟨r, hr, AboveRow.fires_of_firesB hf⟩
  · intro i T hT c hp
    have hi := List.all_eq_true.1 htr i (List.mem_range.2 (lt_of_getElem? hp))
    simp only [hp, hT] at hi
    obtain ⟨x, y, h1, h2, h3, h4, hiff⟩ := Truth.exists_of_can hi
    rw [beq_iff_eq] at hiff
    exact ⟨x, y, h1, h2, h3, h4, hiff⟩

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
  /-- An end of the coordinates a row of a table above takes on the axis of the numbered value
      `value` in its own table (rulec's §15.195), read the same way. Only a refutation about one
      such row may name one. -/
  | cond : (value : Nat) → (hi : Bool) → (at_ : Rat) → (opn : Bool) → Ref
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

/-- Whether an end holds of every coordinate a row above takes on the axis of the numbered value
    `j`, read from the row's entry for it. -/
def condHolds (conds : List (Nat × List Coord)) (j : Nat) (hi : Bool) (at_ : Rat) (opn : Bool) : Bool :=
  match conds.find? (fun q => q.1 == j) with
  | some q => q.2.all (fun x => beyondEnd x hi at_ opn)
  | none => false

/-- The inequalities a refutation names, built again from the sieve, the box and — for a
    refutation about a row of a table above — what that row takes on the numbered values
    (`conds`); `none` where a fact is not there or an end does not hold. -/
def refIneqsIn (s : Sieve) (box : Box) (conds : List (Nat × List Coord)) :
    List (Ref × Rat) → Option (List (Rat × LinIneq))
  | [] => some []
  | (r, y) :: rest =>
    match refIneqsIn s box conds rest with
    | none => none
    | some qs =>
      match r with
      | .fact i => (s.facts[i]?).map (fun q => (y, q) :: qs)
      | .coord i hi at_ opn =>
        if endHolds s box i hi at_ opn then some ((y, endIneq i hi at_ opn) :: qs) else none
      | .cond j hi at_ opn =>
        if condHolds conds j hi at_ opn then some ((y, endIneq j hi at_ opn) :: qs) else none

/-- The inequalities of a refutation of the model in a box: no row above is in question, so an
    end of one is not there. -/
def refIneqs (s : Sieve) (box : Box) (refs : List (Ref × Rat)) : Option (List (Rat × LinIneq)) :=
  refIneqsIn s box [] refs

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

/-- Every inequality `refIneqsIn` builds holds at the values behind a point of the box that the
    rule is asked about, where the row above whose entries are `conds` fires on those values. -/
theorem refIneqsIn_hold {s : Sieve} {box : Box} {conds : List (Nat × List Coord)} {p : Point}
    {v : List Rat}
    (hin : inBox box p = true)
    (hf : ∀ (i c : Nat) (x : Coord), p[i]? = some c → s.coordAt i c = some x →
      ∃ w, v[i]? = some w ∧ x.holds w)
    (hfa : ∀ q ∈ s.facts, q.holds v)
    (hc : ∀ q ∈ conds, ∃ w, v[q.1]? = some w ∧ ∃ x ∈ q.2, x.holds w) :
    ∀ (refs : List (Ref × Rat)) (ps : List (Rat × LinIneq)),
      refIneqsIn s box conds refs = some ps → ∀ q ∈ ps, q.2.holds v
  | [], ps, h => by simp only [refIneqsIn, Option.some.injEq] at h; subst h; simp
  | (r, y) :: rest, ps, h => by
    simp only [refIneqsIn] at h
    split at h
    · exact absurd h (by simp)
    · rename_i qs hqs
      have ih := refIneqsIn_hold hin hf hfa hc rest qs hqs
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
      | cond j hi at_ opn =>
        by_cases hend : condHolds conds j hi at_ opn = true
        · simp only [hend, ite_true, Option.some.injEq] at h
          subst h
          intro e he
          simp only [List.mem_cons] at he
          rcases he with rfl | he
          · unfold condHolds at hend
            split at hend
            · rename_i q hq
              have hmem := List.mem_of_find?_eq_some hq
              have hj : q.1 = j := by simpa using List.find?_some hq
              obtain ⟨w, hw, x, hx, hxw⟩ := hc q hmem
              have hall := List.all_eq_true.1 hend x hx
              rw [hj] at hw
              exact endIneq_holds hall hxw (getD_of_getElem? hw)
            · exact absurd hend (by simp)
          · exact ih e he
        · simp only [hend, Bool.false_eq_true, ite_false] at h
          exact absurd h (by simp)

/-- Every inequality `refIneqs` builds holds at the values behind a point of the box that the
    rule is asked about. -/
theorem refIneqs_hold {s : Sieve} {box : Box} {p : Point} {v : List Rat}
    (hin : inBox box p = true)
    (hf : ∀ (i c : Nat) (x : Coord), p[i]? = some c → s.coordAt i c = some x →
      ∃ w, v[i]? = some w ∧ x.holds w)
    (hfa : ∀ q ∈ s.facts, q.holds v) :
    ∀ (refs : List (Ref × Rat)) (ps : List (Rat × LinIneq)), refIneqs s box refs = some ps →
      ∀ q ∈ ps, q.2.holds v :=
  refIneqsIn_hold hin hf hfa (by simp)

/-- **A box the linear model leaves no values in holds no point the rule is asked about.** -/
theorem not_asked_of_farkas {s : Sieve} {box : Box} {refs : List (Ref × Rat)} {p : Point}
    (h : farkasRuledOut s box refs = true) (hin : inBox box p = true) : ¬ s.asked p := by
  rintro ⟨v, hf, _, _, _, _, hfa, _⟩
  unfold farkasRuledOut at h
  split at h
  · rename_i ps hps
    exact farkas_sound h v (refIneqs_hold hin hf hfa refs ps hps)
  · exact absurd h (by simp)

/-! ## A box the rows of a table above leave no values in (rulec's §15.195)

A column a table above decides holds a value only where a row of that table that writes it
fires. The certificate names the column's axis on a box that fixes it; every row that writes
the value comes with a reason it fires nowhere in the box — the box takes none of the words the
row lets in on a column both tables cut, or a refutation that adds the model's facts, the ends of
the box and the ends of the row's own box up to a contradiction. The rows themselves are read
from the table above when the sieve is built, not from the leaf. -/

/-- Why a row of a table above fires nowhere in a box. -/
inductive RowOut where
  /-- On this axis the box takes none of the words the row lets in. -/
  | clash : Nat → RowOut
  /-- The multipliers of a refutation that may name the row's own ends. -/
  | farkas : List (Ref × Rat) → RowOut
  deriving Repr, Inhabited

def rowOutOk (s : Sieve) (box : Box) (r : AboveRow) : RowOut → Bool
  | .clash j =>
    match r.words.find? (fun q => q.1 == j), box[j]? with
    | some q, some cs => cs.all (fun c => !q.2.contains c)
    | _, _ => false
  | .farkas refs =>
    match refIneqsIn s box r.nums refs with
    | some ps => farkasOk ps
    | none => false

/-- **The rows above leave the box no values**: the box fixes the axis to one coordinate, the
    sieve has the rows that write its value, and each of them comes with a reason that holds. -/
def aboveRuledOut (s : Sieve) (box : Box) (axis : Nat) (outs : List RowOut) : Bool :=
  match box[axis]? with
  | some [coord] =>
    match s.above.find? (fun e => e.axis == axis && e.coord == coord) with
    | some e => e.rows.length == outs.length && (e.rows.zip outs).all (fun ro => rowOutOk s box ro.1 ro.2)
    | none => false
  | _ => false

theorem rowOut_sound {s : Sieve} {box : Box} {r : AboveRow} {o : RowOut} {p : Point} {v : List Rat}
    (h : rowOutOk s box r o = true) (hin : inBox box p = true)
    (hf : ∀ (i c : Nat) (x : Coord), p[i]? = some c → s.coordAt i c = some x →
      ∃ w, v[i]? = some w ∧ x.holds w)
    (hfa : ∀ q ∈ s.facts, q.holds v) (hr : r.fires p v) : False := by
  cases o with
  | clash j =>
    simp only [rowOutOk] at h
    cases hq : r.words.find? (fun q => q.1 == j) with
    | none => rw [hq] at h; exact absurd h (by simp)
    | some q =>
      cases hbj : box[j]? with
      | none => rw [hq, hbj] at h; exact absurd h (by simp)
      | some cs =>
        rw [hq, hbj] at h
        have hmem := List.mem_of_find?_eq_some hq
        have hj : q.1 = j := by simpa using List.find?_some hq
        obtain ⟨c, hpc, hcok⟩ := hr.2 q hmem
        obtain ⟨c', hpc', hc'⟩ := inBox_getElem hin hbj
        rw [hj, hpc'] at hpc
        obtain rfl : c' = c := Option.some.inj hpc
        have hall := List.all_eq_true.1 h c' (List.contains_iff_mem.1 hc')
        simp only [Bool.not_eq_true'] at hall
        have hin' : q.2.contains c' = true := List.contains_iff_mem.2 hcok
        rw [hall] at hin'
        exact absurd hin' (by simp)
  | farkas refs =>
    simp only [rowOutOk] at h
    cases hps : refIneqsIn s box r.nums refs with
    | none => rw [hps] at h; exact absurd h (by simp)
    | some ps =>
      rw [hps] at h
      exact farkas_sound h v (refIneqsIn_hold hin hf hfa hr.1 refs ps hps)

/-- **A box the rows of a table above leave no values in holds no point the rule is asked
    about.** -/
theorem not_asked_of_aboveRuledOut {s : Sieve} {box : Box} {axis : Nat} {outs : List RowOut}
    {p : Point} (h : aboveRuledOut s box axis outs = true) (hin : inBox box p = true) :
    ¬ s.asked p := by
  rintro ⟨v, hf, _, _, _, _, hfa, _, hab, _⟩
  unfold aboveRuledOut at h
  split at h
  · rename_i coord hbx
    split at h
    · rename_i e he
      simp only [Bool.and_eq_true, beq_iff_eq, List.all_eq_true] at h
      obtain ⟨hlen, hall⟩ := h
      have hmem : e ∈ s.above := List.mem_of_find?_eq_some he
      have hpred := List.find?_some he
      simp only [Bool.and_eq_true, beq_iff_eq] at hpred
      obtain ⟨hax, hco⟩ := hpred
      obtain ⟨c, hpc, hcc⟩ := inBox_getElem hin hbx
      have hc : c = coord := by simpa using hcc
      obtain ⟨r, hr, hfires⟩ := hab e hmem (by rw [hax, hco, ← hc]; exact hpc)
      obtain ⟨i, hi, hri⟩ := List.mem_iff_getElem.1 hr
      have hio : i < outs.length := by rw [← hlen]; exact hi
      have hzl : i < (e.rows.zip outs).length := by simp [List.length_zip]; omega
      have hz : (e.rows.zip outs)[i] ∈ e.rows.zip outs := List.getElem_mem hzl
      have hok := hall _ hz
      simp only [List.getElem_zip] at hok
      rw [hri] at hok
      exact rowOut_sound hok hin hf hfa hfires
    · exact absurd h (by simp)
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

/-! ## Two rows koyomi's days part (rulec's §15.174) -/

/-- On the axis `i`, the coordinate `c` holds no day of the axis's set: no value it stands for is
    one of the days. -/
def daysCoordOut (s : Sieve) (i c : Nat) : Bool :=
  match s.days[i]?, s.coordAt i c with
  | some (some D), some x => D.all (fun d => !coordHoldsB x d)
  | _, _ => false

/-- Two boxes part on koyomi's days of axis `i`: every coordinate both take there holds none of
    the days. -/
def daysPart (s : Sieve) (a b : Box) (i : Nat) : Bool :=
  match a[i]?, b[i]? with
  | some xs, some ys => xs.all (fun c => !ys.contains c || daysCoordOut s i c)
  | _, _ => false

theorem daysRulesOut_of_coord {s : Sieve} {i c : Nat} {p : Point} (hp : p[i]? = some c)
    (h : daysCoordOut s i c = true) : daysRulesOut s i p = true := by
  unfold daysCoordOut at h
  unfold daysRulesOut
  split at h
  case _ D x hD hx => rw [hp, hD]; simp only [hx]; exact h
  case _ => exact absurd h (by simp)

/-- **No point the rule is asked about lies in two boxes koyomi's days part.** -/
theorem not_asked_of_daysPart {s : Sieve} {a b : Box} {i : Nat} {p : Point}
    (h : daysPart s a b i = true) (ha : inBox a p = true) (hb : inBox b p = true) : ¬ s.asked p := by
  unfold daysPart at h
  cases hx : a[i]? with
  | none => rw [hx] at h; simp at h
  | some xs =>
    cases hy : b[i]? with
    | none => rw [hx, hy] at h; simp at h
    | some ys =>
      rw [hx, hy] at h
      obtain ⟨c, hc, hcx⟩ := inBox_getElem ha hx
      obtain ⟨d, hd, hdy⟩ := inBox_getElem hb hy
      have : c = d := by rw [hc] at hd; exact Option.some.inj hd
      subst this
      have hall := List.all_eq_true.1 h c (by simpa using hcx)
      simp only [Bool.or_eq_true, Bool.not_eq_true'] at hall
      rcases hall with hno | hout
      · rw [hdy] at hno; exact absurd hno (by simp)
      · exact not_asked_of_days (daysRulesOut_of_coord hc hout)

/-! ## Two rows the rows of a table above part (rulec's §15.196) -/

theorem box_length_of_inBox {b : Box} {p : Point} (h : inBox b p = true) : b.length = p.length := by
  induction b generalizing p with
  | nil => cases p with
    | nil => rfl
    | cons _ _ => simp [inBox] at h
  | cons x xs ih => cases p with
    | nil => simp [inBox] at h
    | cons c cs =>
      simp only [inBox, Bool.and_eq_true] at h
      simp [ih h.2]

/-- A point of a box is a point of the box with one axis narrowed to the coordinate it takes there. -/
theorem inBox_set {b : Box} {p : Point} {i c : Nat} (h : inBox b p = true) (hp : p[i]? = some c) :
    inBox (b.set i [c]) p = true := by
  have hlen := box_length_of_inBox h
  apply inBox_of_getElem
  · simp [hlen]
  · intro j xs d hb hd
    by_cases hji : i = j
    · subst hji
      have hi : i < b.length := by rw [hlen]; exact lt_of_getElem? hp
      rw [List.getElem?_set_self hi] at hb
      rw [hp] at hd
      simp only [Option.some.injEq] at hb hd
      subst hb; subst hd
      simp
    · rw [List.getElem?_set_ne hji] at hb
      obtain ⟨e, he, hm⟩ := inBox_getElem h hb
      rw [hd] at he
      simp only [Option.some.injEq] at he
      subst he
      exact hm

/-- Two boxes the rows of a table above part: for every coordinate both take on the axis of the
    column that table decides, the box they share narrowed to it is one `aboveRuledOut` settles,
    with the reasons given for that coordinate. -/
def abovePart (s : Sieve) (a b : Box) (axis : Nat) (at_ : List (Nat × List RowOut)) : Bool :=
  match (pairBox a b)[axis]? with
  | some cs => cs.all (fun c => match at_.find? (fun q => q.1 == c) with
      | some q => aboveRuledOut s ((pairBox a b).set axis [c]) axis q.2
      | none => false)
  | none => false

/-- **No point the rule is asked about lies in two boxes the rows of a table above part.** -/
theorem not_asked_of_abovePart {s : Sieve} {a b : Box} {axis : Nat} {at_ : List (Nat × List RowOut)}
    {p : Point} (h : abovePart s a b axis at_ = true) (ha : inBox a p = true) (hb : inBox b p = true) :
    ¬ s.asked p := by
  have hpb := inBox_pairBox ha hb
  unfold abovePart at h
  split at h
  case _ cs hcs =>
    obtain ⟨c, hpc, hc⟩ := inBox_getElem hpb hcs
    have hall := List.all_eq_true.1 h c (List.contains_iff_mem.1 hc)
    split at hall
    case _ q _ => exact not_asked_of_aboveRuledOut hall (inBox_set hpb hpc)
    case _ => exact absurd hall (by simp)
  case _ => exact absurd h (by simp)

end RulecCert
