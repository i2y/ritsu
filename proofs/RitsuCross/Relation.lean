/-
  X2: a rule's precondition, where a workflow calls it (ritsu's DESIGN 7.4;
  `crates/ritsu-cross/src/preconditions.rs`, and rulec's answer in `crates/rulec/src/ports.rs`).

  rulec writes what the shape of an input cannot say as preconditions; the one the check decides
  is a relation between two inputs, `constraint <left> <op> <right>`. dandori knows the range of
  every value a call gives a rule, on the wire. The check asks rulec whether the relation holds
  for every pair the two ranges allow, and rulec decides it at one corner of the box the ranges
  make (`corner`): the left as large as it gets against the right as small for `<` and `<=`, the
  other way round for `>` and `>=`. A value given to both inputs is the same value on both sides
  (`x2`). A precondition that a date input takes only the days of a koyomi date (`range from
  koyomi`) is decided over the days the value given can be: the days of the koyomi dates it can be
  the day of, each of which must be one of the rule's (`daysKept`). A bound on a list's total or
  length is not decided at the call: dandori knows no list's length.

  A wire integer `n` of an input that travels at the scale `s` stands for `n / s` (rulec's
  `wire_scale`: 1, or for a rate the number of its steps in 1). So `l op r` between two wire
  integers is `l * sr op r * sl`, both brought to one scale (`Op.at`).

  The theorems:

  - `corner_holds`: when rulec says the relation holds, it holds for every pair the ranges allow.
  - `corner_fails`: when rulec gives an example, both values lie in their ranges (where a range
    has a value at all) and the relation does not hold between them.
  - `corner_undecided`: rulec leaves it undecided only where the corner has an open end.
  - `x2_holds`, `x2_fails`: the same, of the decision at the call. A value given to both inputs
    holds them to one value; that `<=` and `>=` then hold and `<` and `>` do not is true when the
    two inputs travel at one scale, which `x2_holds` and `x2_fails` take as a premise.
  - `x2_days_holds`, `x2_days_fails`: when the days are kept, the value can only be the day of
    koyomi dates and every day each comes to is one of the rule's; the example is a day of a koyomi
    date the value can be the day of, which is not one of the rule's.
-/
import RitsuCross.Answer

namespace RitsuCross

/-- The comparisons a `constraint` writes between two inputs (rulec's §15.55). -/
inductive Op where
  | le
  | lt
  | ge
  | gt
  deriving Repr, DecidableEq, Inhabited

/-- `x op y`, between two integers. -/
def Op.test : Op → Int → Int → Bool
  | .le, x, y => decide (x ≤ y)
  | .lt, x, y => decide (x < y)
  | .ge, x, y => decide (y ≤ x)
  | .gt, x, y => decide (y < x)

/-- `l op r` between two wire integers, the left at the scale `sl` and the right at `sr`. -/
def Op.at (op : Op) (l : Int) (sl : Nat) (r : Int) (sr : Nat) : Bool :=
  op.test (l * (sr : Int)) (r * (sl : Int))

/-- `<=` and `>=`: a value is in this relation with itself. -/
def Op.weak : Op → Bool
  | .le | .ge => true
  | .lt | .gt => false

/-- One side of a relation: the range of the wire integers given to the input, and the scale they
    travel at. -/
structure Side where
  range : Range
  scale : Nat
  deriving Repr

/-- rulec's answer for a relation over two ranges (`preconditions_hold`): the corner that tries it
    hardest decides it, and is the example when it does not hold. An open end at the corner leaves
    it undecided. -/
def corner (op : Op) (l r : Side) : Answer (Int × Int) :=
  let c : Option Int × Option Int := match op with
    | .le | .lt => (l.range.hi, r.range.lo)
    | .ge | .gt => (l.range.lo, r.range.hi)
  match c with
  | (some a, some b) => if op.at a l.scale b r.scale then .holds else .fails (a, b)
  | _ => .undecided

/-! ## Multiplying by a scale keeps the order -/

theorem mul_scale_le {x y : Int} (s : Nat) (h : x ≤ y) : x * (s : Int) ≤ y * (s : Int) :=
  Int.mul_le_mul_of_nonneg_right h (Int.natCast_nonneg s)

/-! ## rulec's answer -/

theorem corner_holds {op : Op} {l r : Side} (h : corner op l r = .holds) :
    ∀ a b, l.range.mem a → r.range.mem b → op.at a l.scale b r.scale = true := by
  intro a b ha hb
  cases op with
  | le =>
    unfold corner at h
    simp only at h
    split at h
    · next H L hc =>
      simp only [Prod.mk.injEq] at hc
      obtain ⟨hH, hL⟩ := hc
      split at h
      · next hc' =>
        simp only [Op.at, Op.test, decide_eq_true_eq] at hc' ⊢
        have h1 := mul_scale_le r.scale (ha.2 H hH)
        have h2 := mul_scale_le l.scale (hb.1 L hL)
        omega
      · exact absurd h (by simp)
    · exact absurd h (by simp)
  | lt =>
    unfold corner at h
    simp only at h
    split at h
    · next H L hc =>
      simp only [Prod.mk.injEq] at hc
      obtain ⟨hH, hL⟩ := hc
      split at h
      · next hc' =>
        simp only [Op.at, Op.test, decide_eq_true_eq] at hc' ⊢
        have h1 := mul_scale_le r.scale (ha.2 H hH)
        have h2 := mul_scale_le l.scale (hb.1 L hL)
        omega
      · exact absurd h (by simp)
    · exact absurd h (by simp)
  | ge =>
    unfold corner at h
    simp only at h
    split at h
    · next Lo Hi hc =>
      simp only [Prod.mk.injEq] at hc
      obtain ⟨hLo, hHi⟩ := hc
      split at h
      · next hc' =>
        simp only [Op.at, Op.test, decide_eq_true_eq] at hc' ⊢
        have h1 := mul_scale_le r.scale (ha.1 Lo hLo)
        have h2 := mul_scale_le l.scale (hb.2 Hi hHi)
        omega
      · exact absurd h (by simp)
    · exact absurd h (by simp)
  | gt =>
    unfold corner at h
    simp only at h
    split at h
    · next Lo Hi hc =>
      simp only [Prod.mk.injEq] at hc
      obtain ⟨hLo, hHi⟩ := hc
      split at h
      · next hc' =>
        simp only [Op.at, Op.test, decide_eq_true_eq] at hc' ⊢
        have h1 := mul_scale_le r.scale (ha.1 Lo hLo)
        have h2 := mul_scale_le l.scale (hb.2 Hi hHi)
        omega
      · exact absurd h (by simp)
    · exact absurd h (by simp)

/-- The high end of a range is in it, when the range has a value in it. -/
theorem Range.mem_hi {r : Range} {b : Int} (ok : r.Ok) (h : r.hi = some b) : r.mem b :=
  ⟨fun a ha => ok a b ha h, fun b' hb' => by rw [h] at hb'; cases hb'; exact Int.le_refl _⟩

/-- The low end of a range is in it, when the range has a value in it. -/
theorem Range.mem_lo {r : Range} {a : Int} (ok : r.Ok) (h : r.lo = some a) : r.mem a :=
  ⟨fun a' ha' => by rw [h] at ha'; cases ha'; exact Int.le_refl _, fun b hb => ok a b h hb⟩

theorem corner_fails {op : Op} {l r : Side} {a b : Int} (hl : l.range.Ok) (hr : r.range.Ok)
    (h : corner op l r = .fails (a, b)) :
    l.range.mem a ∧ r.range.mem b ∧ op.at a l.scale b r.scale = false := by
  cases op with
  | le | lt =>
    unfold corner at h
    simp only at h
    split at h
    · next H L hc =>
      simp only [Prod.mk.injEq] at hc
      obtain ⟨hH, hL⟩ := hc
      split at h
      · exact absurd h (by simp)
      · next hc' =>
        simp only [Answer.fails.injEq, Prod.mk.injEq] at h
        obtain ⟨rfl, rfl⟩ := h
        exact ⟨Range.mem_hi hl hH, Range.mem_lo hr hL, by simpa using hc'⟩
    · exact absurd h (by simp)
  | ge | gt =>
    unfold corner at h
    simp only at h
    split at h
    · next Lo Hi hc =>
      simp only [Prod.mk.injEq] at hc
      obtain ⟨hLo, hHi⟩ := hc
      split at h
      · exact absurd h (by simp)
      · next hc' =>
        simp only [Answer.fails.injEq, Prod.mk.injEq] at h
        obtain ⟨rfl, rfl⟩ := h
        exact ⟨Range.mem_lo hl hLo, Range.mem_hi hr hHi, by simpa using hc'⟩
    · exact absurd h (by simp)

/-- The ends of the two ranges at the corner `op` tries. -/
def cornerEnds (op : Op) (l r : Side) : Option Int × Option Int :=
  match op with
  | .le | .lt => (l.range.hi, r.range.lo)
  | .ge | .gt => (l.range.lo, r.range.hi)

theorem corner_undecided {op : Op} {l r : Side} (h : corner op l r = .undecided) :
    (cornerEnds op l r).1 = none ∨ (cornerEnds op l r).2 = none := by
  cases op <;>
  · unfold corner at h
    simp only at h
    unfold cornerEnds
    split at h
    · split at h <;> exact absurd h (by simp)
    · next hc =>
      simp only
      revert hc
      cases l.range.hi <;> cases r.range.lo <;> cases l.range.lo <;> cases r.range.hi <;> simp

/-! ## The decision at the call -/

/-- A kind of place a value can come from, as far as its days go (`ritsu_ports::Origin`): the day
    of a koyomi date, with the days koyomi counts it comes to; or a place that says nothing of what
    day it is (`now`, an input, a task's answer, a rule's output, a number). -/
inductive Origin where
  | day (set : Found (List Int))
  | other
  deriving Repr

/-- A value a call gives a rule (`ritsu_ports::CallArg`): as the flow writes it (`order.total`),
    the range of the wire integers it can be, when every place it comes from has one, and the
    kinds of place it can come from. -/
structure Arg where
  shown : String
  range : Option Range
  origins : List Origin := []
  deriving Repr

/-- A precondition, by kind (`ritsu_ports::Precondition`). -/
inductive Pre where
  /-- `constraint <left> <op> <right>`. -/
  | relation (op : Op) (left right : String)
  /-- A bound on the total of a column over the list the rule walks. -/
  | sum
  /-- A bound on how many elements the list may have. -/
  | length
  /-- The date input `input` takes only the days `days` of a koyomi date (`range from koyomi`). -/
  | days (input : String) (days : List Int)
  deriving Repr

/-- The example the decision gives. -/
inductive Example where
  /-- The corner: the value of the left input and of the right. -/
  | at (l r : Int)
  /-- One value given to both, which a strict relation does not hold of: the low end of its
      range, else the high end, when it has one. -/
  | same (v : Option Int)
  /-- A day the value given can be, which is not one of the rule's. -/
  | day (d : Int)
  deriving Repr, DecidableEq

/-- `preconditions.rs`'s `days_kept`: the kinds of place the value can come from, in order. The
    first day of a koyomi date it can be the day of that is not one of `days` is the example; a
    place that says nothing of what day it is, a koyomi date koyomi does not count, or no place at
    all, leaves it undecided. -/
def daysKept (origins : List Origin) (days : List Int) : Answer Int :=
  go origins origins.isEmpty
where
  go : List Origin → Bool → Answer Int
    | [], und => if und then .undecided else .holds
    | .day (.value set) :: rest, und =>
      match set.find? (fun x => !days.contains x) with
      | some d => .fails d
      | none => go rest und
    | .day .undecided :: rest, _ => go rest true
    | .other :: rest, _ => go rest true

/-- X2 at one call, for one precondition (`preconditions.rs`'s `decide`). `arg` is what the call
    gives each input, and `scale` the scale each input travels at. -/
def x2 (p : Pre) (arg : String → Option Arg) (scale : String → Nat) : Answer Example :=
  match p with
  | .relation op left right =>
    match arg left, arg right with
    | some l, some r =>
      if l.shown = r.shown then
        if op.weak then .holds else .fails (.same (l.range.bind (fun g => g.lo.or g.hi)))
      else
        match l.range, r.range with
        | some lr, some rr =>
          match corner op ⟨lr, scale left⟩ ⟨rr, scale right⟩ with
          | .holds => .holds
          | .fails (a, b) => .fails (.at a b)
          | .undecided => .undecided
        | _, _ => .undecided
    | _, _ => .undecided
  | .sum => .undecided
  | .length => .undecided
  | .days input days =>
    match arg input with
    | some a => match daysKept a.origins days with
      | .holds => .holds
      | .fails d => .fails (.day d)
      | .undecided => .undecided
    | none => .undecided

/-- The pairs of wire integers a call can give the two inputs, as dandori's E014 reads its ranges:
    each from every place it comes from, one value at a time; the same value to both when the flow
    gives both the same. -/
def Gives (l r : Arg) (a b : Int) : Prop :=
  (l.shown = r.shown → a = b) ∧
  (∀ g, l.range = some g → g.mem a) ∧
  (∀ g, r.range = some g → g.mem b)

/-- **What X2 says when the precondition holds**: every pair the call can give keeps it. Where the
    flow gives both inputs the same value, the two inputs travel at one scale. -/
theorem x2_holds {op : Op} {left right : String} {arg : String → Option Arg} {scale : String → Nat}
    (h : x2 (.relation op left right) arg scale = .holds) :
    ∃ l r, arg left = some l ∧ arg right = some r ∧
      ((l.shown = r.shown → scale left = scale right) →
        ∀ a b, Gives l r a b → op.at a (scale left) b (scale right) = true) := by
  simp only [x2] at h
  split at h
  · next l r hl hr =>
    refine ⟨l, r, hl, hr, fun hs a b hg => ?_⟩
    split at h
    · next he =>
      have hab := hg.1 he
      subst hab
      rw [hs he]
      split at h
      · next hw =>
        cases op <;> simp_all [Op.weak, Op.at, Op.test]
      · exact absurd h (by simp)
    · next he =>
      split at h
      · next lr rr hlr hrr =>
        split at h
        · next hc => exact corner_holds hc a b (hg.2.1 lr hlr) (hg.2.2 rr hrr)
        · exact absurd h (by simp)
        · exact absurd h (by simp)
      · exact absurd h (by simp)
  · exact absurd h (by simp)

/-- **What X2 says when it gives an example**: at the corner, two values the call can give that
    break the precondition (where the ranges have a value in them); one value given to both, a
    strict relation that no value is in with itself, and the low end of its range (or the high
    end) as the example. -/
theorem x2_fails {op : Op} {left right : String} {arg : String → Option Arg} {scale : String → Nat}
    {e : Example} (h : x2 (.relation op left right) arg scale = .fails e) :
    ∃ l r, arg left = some l ∧ arg right = some r ∧
      match e with
      | .at a b => l.shown ≠ r.shown ∧
          ((∀ g, l.range = some g → g.Ok) → (∀ g, r.range = some g → g.Ok) →
            Gives l r a b ∧ op.at a (scale left) b (scale right) = false)
      | .same v => l.shown = r.shown ∧ op.weak = false ∧
          v = l.range.bind (fun g => g.lo.or g.hi) ∧
          (scale left = scale right → ∀ a, op.at a (scale left) a (scale right) = false)
      | .day _ => False := by
  simp only [x2] at h
  split at h
  · next l r hl hr =>
    refine ⟨l, r, hl, hr, ?_⟩
    split at h
    · next he =>
      split at h
      · exact absurd h (by simp)
      · next hw =>
        simp only [Answer.fails.injEq] at h
        subst h
        refine ⟨he, by simpa using hw, rfl, fun hs a => ?_⟩
        rw [hs]
        cases op <;> simp_all [Op.weak, Op.at, Op.test]
    · next he =>
      split at h
      · next lr rr hlr hrr =>
        split at h
        · exact absurd h (by simp)
        · next a b hc =>
          simp only [Answer.fails.injEq] at h
          subst h
          refine ⟨he, fun okl okr => ?_⟩
          obtain ⟨ha, hb, hf⟩ := corner_fails (okl lr hlr) (okr rr hrr) hc
          refine ⟨⟨fun hs => absurd hs he, fun g hg => ?_, fun g hg => ?_⟩, hf⟩
          · rw [hlr] at hg; cases hg; exact ha
          · rw [hrr] at hg; cases hg; exact hb
        · exact absurd h (by simp)
      · exact absurd h (by simp)
  · exact absurd h (by simp)

/-- The preconditions the call cannot decide: dandori knows no list's length. -/
theorem x2_undecided_on_lists {arg : String → Option Arg} {scale : String → Nat} :
    x2 .sum arg scale = .undecided ∧ x2 .length arg scale = .undecided :=
  ⟨rfl, rfl⟩

theorem daysKept_go_holds {days : List Int} :
    ∀ {os : List Origin} {und : Bool}, daysKept.go days os und = .holds →
      und = false ∧ ∀ o ∈ os, ∃ set, o = .day (.value set) ∧ ∀ x ∈ set, x ∈ days
  | [], und, h => by
    simp only [daysKept.go] at h
    cases und <;> simp_all
  | o :: os, und, h => by
    cases o with
    | day f =>
      cases f with
      | value set =>
        simp only [daysKept.go] at h
        split at h
        · exact absurd h (by simp)
        · next hn =>
          obtain ⟨hu, hall⟩ := daysKept_go_holds h
          refine ⟨hu, fun x hx => ?_⟩
          rcases List.mem_cons.1 hx with rfl | hx'
          · refine ⟨set, rfl, fun y hy => ?_⟩
            have := List.find?_eq_none.1 hn y hy
            cases hc : days.contains y with
            | true => exact List.contains_iff_mem.1 hc
            | false => rw [hc] at this; exact absurd rfl this
          · exact hall x hx'
      | undecided =>
        simp only [daysKept.go] at h
        exact absurd (daysKept_go_holds h).1 (by simp)
    | other =>
      simp only [daysKept.go] at h
      exact absurd (daysKept_go_holds h).1 (by simp)

theorem daysKept_go_fails {days : List Int} :
    ∀ {os : List Origin} {und : Bool} {d : Int}, daysKept.go days os und = .fails d →
      ∃ set, Origin.day (.value set) ∈ os ∧ d ∈ set ∧ d ∉ days
  | [], und, d, h => by
    simp only [daysKept.go] at h
    cases und <;> simp_all
  | o :: os, und, d, h => by
    cases o with
    | day f =>
      cases f with
      | value set =>
        simp only [daysKept.go] at h
        split at h
        · next d' hs =>
          simp only [Answer.fails.injEq] at h
          subst h
          have hp := List.find?_some hs
          refine ⟨set, List.mem_cons_self, List.mem_of_find?_eq_some hs, fun hm => ?_⟩
          rw [List.contains_iff_mem.2 hm] at hp
          exact absurd hp (by simp)
        · obtain ⟨set', hm, hd, hn⟩ := daysKept_go_fails h
          exact ⟨set', List.mem_cons_of_mem _ hm, hd, hn⟩
      | undecided =>
        simp only [daysKept.go] at h
        obtain ⟨set', hm, hd, hn⟩ := daysKept_go_fails h
        exact ⟨set', List.mem_cons_of_mem _ hm, hd, hn⟩
    | other =>
      simp only [daysKept.go] at h
      obtain ⟨set', hm, hd, hn⟩ := daysKept_go_fails h
      exact ⟨set', List.mem_cons_of_mem _ hm, hd, hn⟩

/-- **What X2 says when the days of a koyomi date are kept**: the value given can only be the day
    of koyomi dates, and every day each of them comes to is one of the rule's. -/
theorem x2_days_holds {input : String} {days : List Int} {arg : String → Option Arg} {scale : String → Nat}
    (h : x2 (.days input days) arg scale = .holds) :
    ∃ a, arg input = some a ∧ a.origins ≠ [] ∧
      ∀ o ∈ a.origins, ∃ set, o = .day (.value set) ∧ ∀ x ∈ set, x ∈ days := by
  simp only [x2] at h
  split at h
  · next a ha =>
    split at h
    · next hk =>
      obtain ⟨hu, hall⟩ := daysKept_go_holds hk
      refine ⟨a, ha, fun he => ?_, hall⟩
      simp [he] at hu
    · exact absurd h (by simp)
    · exact absurd h (by simp)
  · exact absurd h (by simp)

/-- **What X2 says when it gives a day**: the value given can be the day of a koyomi date that comes
    to that day, and the day is not one of the rule's. -/
theorem x2_days_fails {input : String} {days : List Int} {arg : String → Option Arg} {scale : String → Nat}
    {d : Int} (h : x2 (.days input days) arg scale = .fails (.day d)) :
    ∃ a set, arg input = some a ∧ Origin.day (.value set) ∈ a.origins ∧ d ∈ set ∧ d ∉ days := by
  simp only [x2] at h
  split at h
  · next a ha =>
    split at h
    · exact absurd h (by simp)
    · next d' hk =>
      simp only [Answer.fails.injEq, Example.day.injEq] at h
      subst h
      obtain ⟨set, hm, hd, hn⟩ := daysKept_go_fails hk
      exact ⟨a, set, ha, hm, hd, hn⟩
    · exact absurd h (by simp)
  · exact absurd h (by simp)

end RitsuCross
