/-
  The decisions of the borders a workflow crosses into koyomi and chobo (ritsu's DESIGN 7.5 (a),
  7.6–7.8: X3 (a), X4, X5, X6; `crates/ritsu-cross/src/borders.rs`), each over what the languages
  hand over through their ports.

  - `daysFit`, `daysGiven` (X3 (a), X6): every day a value can be lies inside a range — a rule's
    date input's, or a koyomi date's input's (`inputRange`). The value can be the day of koyomi
    dates, each of whose days koyomi counts exactly, or come from somewhere that says nothing of
    what day it is. The example is the first day outside, of the first koyomi date that has one.
  - `amountFits`, `amountsGiven` (X4): every amount a value can be is one chobo takes, 0 to
    2⁶³ − 1: the outputs of rules (their fewest and most, as rulec counts them) and the numbers
    dandori knows the range of. `amountsHull` is what chobo's search for the refusals is held to.
  - `refusalsMet` (X4): the refusals chobo's search finds that turn on the amounts (the reasons of
    the book's bounds) against the errors the task handles.
  - `heldUntil` (X5): a hold is still held when a call on it comes, from the fewest and the most
    seconds the call can come after the hold, against the seconds it expires after.

  Each is written as `borders.rs` writes it, and each theorem below says what an answer settles.
  `amountFits_chobo_takes` and `amountFits_fails_chobo` tie X4 to `ChoboModel` (an amount the check
  lets through is one chobo's interpreter takes, and the amount it gives as an example is one the
  interpreter does not take), and
  `heldUntil_fails_expired`, `heldUntil_holds_held` tie X5 to it (a hold the check says expires is
  one `ChoboModel.expiredAt` says has expired when the call comes, and the other way round).
-/
import RitsuCross.Answer
import ChoboModel

namespace RitsuCross

/-! ## X3 (a): the days of a koyomi date, against a rule's range -/

/-- `borders::days_fit`. The days are koyomi's set, in its order (ascending: a `BTreeSet`). -/
def daysFit (days : Found (List Int)) (range : Range) : Answer Int :=
  match days with
  | .undecided => .undecided
  | .value set =>
    match set.find? range.outside with
    | some d => .fails d
    | none => .holds

/-- **When X3 (a) holds**, every day of the set lies inside the rule's range. -/
theorem daysFit_holds {set : List Int} {range : Range} (h : daysFit (.value set) range = .holds) :
    ∀ d ∈ set, range.mem d := by
  simp only [daysFit] at h
  split at h
  · exact absurd h (by simp)
  · next hn =>
    intro d hd
    have := List.find?_eq_none.1 hn d hd
    exact Range.mem_of_outside_false (by simpa using this)

/-- **When X3 (a) gives an example**, it is a day of the set outside the rule's range, and every
    day the set has before it lies inside. -/
theorem daysFit_fails {set : List Int} {range : Range} {d : Int}
    (h : daysFit (.value set) range = .fails d) :
    d ∈ set ∧ ¬ range.mem d ∧ ∃ before after, set = before ++ d :: after ∧ ∀ e ∈ before, range.mem e := by
  simp only [daysFit] at h
  split at h
  · next d' hs =>
    simp only [Answer.fails.injEq] at h
    subst h
    obtain ⟨hp, before, after, heq, hb⟩ := List.find?_eq_some_iff_append.1 hs
    refine ⟨List.mem_of_find?_eq_some hs, Range.not_mem_of_outside hp, before, after, heq, ?_⟩
    intro e he
    exact Range.mem_of_outside_false (by simpa using hb e he)
  · exact absurd h (by simp)

/-- X3 (a) is undecided only when koyomi could not count the days. -/
theorem daysFit_undecided {days : Found (List Int)} {range : Range}
    (h : daysFit days range = .undecided) : days = .undecided := by
  cases days with
  | undecided => rfl
  | value set =>
    simp only [daysFit] at h
    split at h <;> exact absurd h (by simp)

/-- `borders::days_given` (X3 (a), X6): the days of each koyomi date the value can be the day of,
    in order, and whether it can also come from a place that says nothing of what day it is
    (`other`). The example is the first koyomi date with a day outside, by its place, and the day. -/
def daysGiven (dates : List (Found (List Int))) (other : Bool) (range : Range) : Answer (Nat × Int) :=
  go dates 0 other
where
  go : List (Found (List Int)) → Nat → Bool → Answer (Nat × Int)
    | [], _, und => if und then .undecided else .holds
    | d :: ds, i, und =>
      match daysFit d range with
      | .fails day => .fails (i, day)
      | .undecided => go ds (i + 1) true
      | .holds => go ds (i + 1) und

theorem daysGiven_go_holds {range : Range} :
    ∀ {dates : List (Found (List Int))} {i : Nat} {und : Bool},
      daysGiven.go range dates i und = .holds →
        und = false ∧ ∀ d ∈ dates, ∃ set, d = .value set ∧ ∀ x ∈ set, range.mem x
  | [], i, und, h => by
    simp only [daysGiven.go] at h
    cases und <;> simp_all
  | d :: ds, i, und, h => by
    simp only [daysGiven.go] at h
    split at h
    · exact absurd h (by simp)
    · obtain ⟨hu, _⟩ := daysGiven_go_holds h
      exact absurd hu (by simp)
    · next hd =>
      obtain ⟨hu, hall⟩ := daysGiven_go_holds h
      refine ⟨hu, fun x hx => ?_⟩
      rcases List.mem_cons.1 hx with rfl | hx'
      · cases x with
        | undecided => simp [daysFit] at hd
        | value set => exact ⟨set, rfl, daysFit_holds hd⟩
      · exact hall x hx'

/-- **When X3 (a) or X6 holds**, the value can only be the day of koyomi dates, and every day each
    of them comes to lies inside the range. -/
theorem daysGiven_holds {dates : List (Found (List Int))} {other : Bool} {range : Range}
    (h : daysGiven dates other range = .holds) :
    other = false ∧ ∀ d ∈ dates, ∃ set, d = .value set ∧ ∀ x ∈ set, range.mem x :=
  daysGiven_go_holds h

theorem daysGiven_go_fails {range : Range} :
    ∀ {dates : List (Found (List Int))} {i j : Nat} {und : Bool} {day : Int},
      daysGiven.go range dates i und = .fails (j, day) →
        ∃ set, dates[j - i]? = some (.value set) ∧ i ≤ j ∧ day ∈ set ∧ ¬ range.mem day
  | [], i, j, und, day, h => by
    simp only [daysGiven.go] at h
    cases und <;> simp_all
  | d :: ds, i, j, und, day, h => by
    simp only [daysGiven.go] at h
    split at h
    · next day' hd =>
      simp only [Answer.fails.injEq, Prod.mk.injEq] at h
      obtain ⟨rfl, rfl⟩ := h
      cases d with
      | undecided => simp [daysFit] at hd
      | value set =>
        obtain ⟨hm, hout, -⟩ := daysFit_fails hd
        exact ⟨set, by simp, Nat.le_refl _, hm, hout⟩
    · obtain ⟨set, hs, hij, hm, hout⟩ := daysGiven_go_fails h
      refine ⟨set, ?_, by omega, hm, hout⟩
      have : j - i = (j - (i + 1)) + 1 := by omega
      rw [this]
      simpa using hs
    · obtain ⟨set, hs, hij, hm, hout⟩ := daysGiven_go_fails h
      refine ⟨set, ?_, by omega, hm, hout⟩
      have : j - i = (j - (i + 1)) + 1 := by omega
      rw [this]
      simpa using hs

/-- **When X3 (a) or X6 gives an example**, it is a day of the koyomi date it names, which the value
    can be the day of, and the day lies outside the range. -/
theorem daysGiven_fails {dates : List (Found (List Int))} {other : Bool} {range : Range} {j : Nat}
    {day : Int} (h : daysGiven dates other range = .fails (j, day)) :
    ∃ set, dates[j]? = some (.value set) ∧ day ∈ set ∧ ¬ range.mem day := by
  obtain ⟨set, hs, -, hm, hout⟩ := daysGiven_go_fails h
  exact ⟨set, by simpa using hs, hm, hout⟩

/-! ## X4: a rule's output as an amount -/

/-- The largest amount chobo takes, 2⁶³ − 1 (chobo's DESIGN 1.5; `ChoboModel.maxAmount`). -/
def maxAmount : Int := 9223372036854775807

/-- What a numeric output of a rule comes to (`ritsu_ports::OutputValues`), on the wire: its fewest
    and its most, every number its rows write when each writes one, and for some of those values
    an input that comes to it (`α`, which this check only hands on). -/
structure OutputValues (α : Type) where
  min : Option Int
  max : Option Int
  values : Option (List Int)
  examples : List (Int × α)

/-- `borders::amount_fits`: an amount below 1 is looked for first among the numbers the rows
    write, then the low end; an amount above 2⁶³ − 1 is the high end. -/
def amountFits {α : Type} (out : Found (OutputValues α)) : Answer (Int × Option α) :=
  match out with
  | .undecided => .undecided
  | .value o =>
    match o.min, o.max with
    | some lo, some hi =>
      let bad : Option Int :=
        if lo < 0 then some ((o.values.bind (fun vs => vs.find? (fun v => decide (v < 0)))).getD lo)
        else if hi > maxAmount then some hi
        else none
      match bad with
      | none => .holds
      | some v => .fails (v, (o.examples.find? (fun e => e.1 == v)).map (·.2))
    | _, _ => .undecided

/-- **X4's amounts hold exactly when** both ends are known, the low one is at least 1 and the high
    one at most 2⁶³ − 1. -/
theorem amountFits_holds_iff {α : Type} {o : OutputValues α} :
    amountFits (.value o) = .holds ↔
      ∃ lo hi, o.min = some lo ∧ o.max = some hi ∧ 0 ≤ lo ∧ hi ≤ maxAmount := by
  simp only [amountFits]
  constructor
  · intro h
    split at h
    · next lo hi hlo hhi =>
      refine ⟨lo, hi, hlo, hhi, ?_⟩
      split at h
      · next hb =>
        split at hb
        · exact absurd hb (by simp)
        · next h1 =>
          split at hb
          · exact absurd hb (by simp)
          · next h2 => exact ⟨by omega, by omega⟩
      · exact absurd h (by simp)
    · exact absurd h (by simp)
  · rintro ⟨lo, hi, hlo, hhi, h1, h2⟩
    simp only [hlo, hhi]
    have e1 : ¬ lo < 0 := by omega
    have e2 : ¬ hi > maxAmount := by omega
    simp [e1, e2]

/-- **When X4's amounts hold**, every value of the output's range is an amount from 0 to 2⁶³ − 1. -/
theorem amountFits_holds {α : Type} {o : OutputValues α} {lo hi : Int}
    (h : amountFits (.value o) = .holds) (hlo : o.min = some lo) (hhi : o.max = some hi) :
    ∀ v, lo ≤ v → v ≤ hi → 0 ≤ v ∧ v ≤ maxAmount := by
  obtain ⟨lo', hi', h1, h2, h3, h4⟩ := amountFits_holds_iff.1 h
  rw [hlo] at h1
  rw [hhi] at h2
  cases h1
  cases h2
  intro v hv1 hv2
  exact ⟨by omega, by omega⟩

/-- **When X4's amounts give an example**, it is outside 0 to 2⁶³ − 1, it is one of the numbers the
    rows write or an end of the output's range, and the input beside it is the one the rule's
    vectors give for it. -/
theorem amountFits_fails {α : Type} {o : OutputValues α} {v : Int} {ex : Option α}
    (h : amountFits (.value o) = .fails (v, ex)) :
    (v < 0 ∨ maxAmount < v) ∧
      ((∃ vs, o.values = some vs ∧ v ∈ vs) ∨ o.min = some v ∨ o.max = some v) ∧
      ex = (o.examples.find? (fun e => e.1 == v)).map (·.2) := by
  simp only [amountFits] at h
  split at h
  · next lo hi hlo hhi =>
    split at h
    · exact absurd h (by simp)
    · next b hb =>
      simp only [Answer.fails.injEq, Prod.mk.injEq] at h
      obtain ⟨rfl, rfl⟩ := h
      refine ⟨?_, ?_, rfl⟩
      · split at hb
        · next h1 =>
          simp only [Option.some.injEq] at hb
          subst hb
          cases hv : o.values.bind (fun vs => vs.find? (fun v => decide (v < 0))) with
          | none => simp only [Option.getD_none]; exact Or.inl h1
          | some w =>
            simp only [Option.getD_some]
            left
            cases hvs : o.values with
            | none => rw [hvs] at hv; simp at hv
            | some vs =>
              rw [hvs] at hv
              simpa using List.find?_some hv
        · split at hb
          · next h2 =>
            simp only [Option.some.injEq] at hb
            subst hb
            right; omega
          · exact absurd hb (by simp)
      · split at hb
        · simp only [Option.some.injEq] at hb
          subst hb
          cases hv : o.values.bind (fun vs => vs.find? (fun v => decide (v < 0))) with
          | none => simp only [Option.getD_none]; exact Or.inr (Or.inl hlo)
          | some w =>
            simp only [Option.getD_some]
            left
            cases hvs : o.values with
            | none => rw [hvs] at hv; simp at hv
            | some vs =>
              rw [hvs] at hv
              exact ⟨vs, rfl, List.mem_of_find?_eq_some hv⟩
        · split at hb
          · simp only [Option.some.injEq] at hb
            subst hb
            exact Or.inr (Or.inr hhi)
          · exact absurd hb (by simp)
  · exact absurd h (by simp)

/-- **X4, held to chobo's interpreter**: an amount in the range of an output the check lets through
    is one `ChoboModel.fits` takes as an amount argument (from 0 to 2⁶³ − 1). -/
theorem amountFits_chobo_takes {α : Type} {o : OutputValues α} {lo hi a : Int}
    (h : amountFits (.value o) = .holds) (hlo : o.min = some lo) (hhi : o.max = some hi)
    (h1 : lo ≤ a) (h2 : a ≤ hi) :
    (decide (0 ≤ a) && decide (a ≤ ChoboModel.maxAmount)) = true := by
  have := amountFits_holds h hlo hhi a h1 h2
  simp only [maxAmount] at this
  have h0 : 0 ≤ a := by omega
  have hm : a ≤ ChoboModel.maxAmount := by unfold ChoboModel.maxAmount; omega
  simp [h0, hm]

/-- **X4, held to chobo's interpreter, the other way**: the amount X4 gives as an example is one
    `ChoboModel.fits` does not take as an amount argument (below 0 or above 2⁶³ − 1), so the check
    refuses exactly what the interpreter does not take. -/
theorem amountFits_fails_chobo {α : Type} {o : OutputValues α} {v : Int} {ex : Option α}
    (h : amountFits (.value o) = .fails (v, ex)) :
    (decide (0 ≤ v) && decide (v ≤ ChoboModel.maxAmount)) = false := by
  obtain ⟨hv, -, -⟩ := amountFits_fails h
  simp only [maxAmount] at hv
  cases h1 : decide (0 ≤ v) with
  | false => rfl
  | true =>
    have h0 : 0 ≤ v := of_decide_eq_true h1
    have hm : ¬ v ≤ ChoboModel.maxAmount := by unfold ChoboModel.maxAmount; omega
    simp [hm]

/-- Where the amount of an example of `amountsGiven` comes from: an output of a rule, by its place,
    with an input that comes to it; or a range dandori knows, by its place. -/
inductive AmountFrom (α : Type) where
  | output (i : Nat) (ex : Option α)
  | range (i : Nat)
  deriving Repr

/-- `borders::amounts_given` (X4): the outputs of rules the value can be, in order, the numbers
    dandori knows the range of, and whether it can also come from a place that says nothing. The
    example is the first offending amount, of the first output with one, else of the first range
    with one; a range with an open end leaves it undecided. -/
def amountsGiven {α : Type} (outputs : List (Found (OutputValues α))) (ranges : List (Option Int × Option Int))
    (other : Bool) : Answer (Int × AmountFrom α) :=
  goOut outputs 0 other
where
  goOut : List (Found (OutputValues α)) → Nat → Bool → Answer (Int × AmountFrom α)
    | [], _, und => goRange ranges 0 und
    | o :: os, i, und =>
      match amountFits o with
      | .fails (v, ex) => .fails (v, .output i ex)
      | .undecided => goOut os (i + 1) true
      | .holds => goOut os (i + 1) und
  goRange : List (Option Int × Option Int) → Nat → Bool → Answer (Int × AmountFrom α)
    | [], _, und => if und then .undecided else .holds
    | r :: rs, i, und =>
      match r with
      | (some lo, _) =>
        if lo < 0 then .fails (lo, .range i)
        else match r.2 with
          | some hi => if hi > maxAmount then .fails (hi, .range i) else goRange rs (i + 1) und
          | none => goRange rs (i + 1) true
      | (none, some hi) => if hi > maxAmount then .fails (hi, .range i) else goRange rs (i + 1) true
      | (none, none) => goRange rs (i + 1) true

theorem amountsGiven_goRange_holds {α : Type} {ranges0 : List (Option Int × Option Int)} :
    ∀ {rs : List (Option Int × Option Int)} {i : Nat} {und : Bool},
      amountsGiven.goRange (α := α) rs i und = .holds →
        und = false ∧ ∀ r ∈ rs, ∃ lo hi, r = (some lo, some hi) ∧ 0 ≤ lo ∧ hi ≤ maxAmount
  | [], i, und, h => by
    simp only [amountsGiven.goRange] at h
    cases und <;> simp_all
  | r :: rs, i, und, h => by
    simp only [amountsGiven.goRange] at h
    split at h
    · next lo hi' =>
      split at h
      · exact absurd h (by simp)
      · next hlo =>
        split at h
        · next hi hhi =>
          split at h
          · exact absurd h (by simp)
          · next hmax =>
            obtain ⟨hu, hall⟩ := amountsGiven_goRange_holds (ranges0 := ranges0) h
            refine ⟨hu, fun x hx => ?_⟩
            rcases List.mem_cons.1 hx with rfl | hx'
            · simp only at hhi
              subst hhi
              exact ⟨lo, hi, rfl, by omega, by omega⟩
            · exact hall x hx'
        · obtain ⟨hu, _⟩ := amountsGiven_goRange_holds (ranges0 := ranges0) h
          exact absurd hu (by simp)
    · split at h
      · exact absurd h (by simp)
      · obtain ⟨hu, _⟩ := amountsGiven_goRange_holds (ranges0 := ranges0) h
        exact absurd hu (by simp)
    · obtain ⟨hu, _⟩ := amountsGiven_goRange_holds (ranges0 := ranges0) h
      exact absurd hu (by simp)

theorem amountsGiven_goOut_holds {α : Type} {ranges : List (Option Int × Option Int)} :
    ∀ {os : List (Found (OutputValues α))} {i : Nat} {und : Bool},
      amountsGiven.goOut ranges os i und = .holds →
        und = false ∧ (∀ o ∈ os, amountFits o = .holds) ∧
          ∀ r ∈ ranges, ∃ lo hi, r = (some lo, some hi) ∧ 0 ≤ lo ∧ hi ≤ maxAmount
  | [], i, und, h => by
    simp only [amountsGiven.goOut] at h
    obtain ⟨hu, hall⟩ := amountsGiven_goRange_holds (ranges0 := ranges) h
    exact ⟨hu, by simp, hall⟩
  | o :: os, i, und, h => by
    simp only [amountsGiven.goOut] at h
    split at h
    · exact absurd h (by simp)
    · obtain ⟨hu, _⟩ := amountsGiven_goOut_holds h
      exact absurd hu (by simp)
    · next ho =>
      obtain ⟨hu, hall, hr⟩ := amountsGiven_goOut_holds h
      refine ⟨hu, fun x hx => ?_, hr⟩
      rcases List.mem_cons.1 hx with rfl | hx'
      · exact ho
      · exact hall x hx'

/-- **When X4's amounts hold**, the value comes from nowhere that says nothing, and every amount it
    can be — any value of each output's range, any number of each range dandori knows — is one
    chobo takes, from 0 to 2⁶³ − 1. -/
theorem amountsGiven_holds {α : Type} {outputs : List (Found (OutputValues α))}
    {ranges : List (Option Int × Option Int)} {other : Bool}
    (h : amountsGiven outputs ranges other = .holds) :
    other = false ∧
      (∀ o ∈ outputs, ∃ out lo hi, o = .value out ∧ out.min = some lo ∧ out.max = some hi ∧
        ∀ v, lo ≤ v → v ≤ hi → 0 ≤ v ∧ v ≤ maxAmount) ∧
      (∀ r ∈ ranges, ∃ lo hi, r = (some lo, some hi) ∧ ∀ v, lo ≤ v → v ≤ hi → 0 ≤ v ∧ v ≤ maxAmount) := by
  obtain ⟨hu, hout, hr⟩ := amountsGiven_goOut_holds h
  refine ⟨hu, fun o ho => ?_, fun r hr' => ?_⟩
  · have hf := hout o ho
    cases o with
    | undecided => simp [amountFits] at hf
    | value out =>
      obtain ⟨lo, hi, hlo, hhi, -, -⟩ := amountFits_holds_iff.1 hf
      exact ⟨out, lo, hi, rfl, hlo, hhi, amountFits_holds hf hlo hhi⟩
  · obtain ⟨lo, hi, rfl, h1, h2⟩ := hr r hr'
    exact ⟨lo, hi, rfl, fun v hv1 hv2 => ⟨by omega, by omega⟩⟩

/-- `borders::amounts_hull` (X4): the fewest and the most of every amount the value can be, held to
    the amounts chobo takes — what chobo's search for the refusals is held to. None when one has an
    open end or is not counted, or no amount chobo takes is left. -/
def amountsHull {α : Type} (outputs : List (Found (OutputValues α))) (ranges : List (Option Int × Option Int)) :
    Option (Int × Int) :=
  let fromOut : Option (List (Int × Int)) := outputs.mapM (fun o => match o with
    | .value out => match out.min, out.max with
      | some lo, some hi => some (lo, hi)
      | _, _ => none
    | .undecided => none)
  let fromRange : Option (List (Int × Int)) := ranges.mapM (fun r => match r with
    | (some lo, some hi) => some (lo, hi)
    | _ => none)
  match fromOut, fromRange with
  | some a, some b =>
    match (a ++ b).map (·.1), (a ++ b).map (·.2) with
    | l :: ls, h :: hs =>
      let lo := max (ls.foldl min l) 0
      let hi := min (hs.foldl max h) maxAmount
      if lo ≤ hi then some (lo, hi) else none
    | _, _ => none
  | _, _ => none

theorem foldl_min_le {l : Int} : ∀ {ls : List Int} {x : Int}, x ∈ l :: ls → ls.foldl min l ≤ x
  | [], x, hx => by simp at hx; subst hx; simp
  | y :: ys, x, hx => by
    simp only [List.foldl_cons]
    rcases List.mem_cons.1 hx with rfl | hx'
    · exact Int.le_trans (foldl_min_le (l := min x y) (ls := ys) List.mem_cons_self) (Int.min_le_left _ _)
    · rcases List.mem_cons.1 hx' with rfl | hx''
      · exact Int.le_trans (foldl_min_le (l := min l x) (ls := ys) List.mem_cons_self) (Int.min_le_right _ _)
      · exact foldl_min_le (l := min l y) (List.mem_cons_of_mem _ hx'')

theorem le_foldl_max {h : Int} : ∀ {hs : List Int} {x : Int}, x ∈ h :: hs → x ≤ hs.foldl max h
  | [], x, hx => by simp at hx; subst hx; simp
  | y :: ys, x, hx => by
    simp only [List.foldl_cons]
    rcases List.mem_cons.1 hx with rfl | hx'
    · exact Int.le_trans (Int.le_max_left _ _) (le_foldl_max (h := max x y) (hs := ys) List.mem_cons_self)
    · rcases List.mem_cons.1 hx' with rfl | hx''
      · exact Int.le_trans (Int.le_max_right _ _) (le_foldl_max (h := max h x) (hs := ys) List.mem_cons_self)
      · exact le_foldl_max (h := max h y) (List.mem_cons_of_mem _ hx'')

theorem mapM_some_mem {β γ : Type} {f : β → Option γ} :
    ∀ {l : List β} {ys : List γ}, l.mapM f = some ys → ∀ x ∈ l, ∃ y, f x = some y ∧ y ∈ ys
  | [], _, _, x, hx => by simp at hx
  | a :: l, ys, h, x, hx => by
    rw [List.mapM_cons] at h
    cases ha : f a with
    | none => rw [ha] at h; simp at h
    | some b =>
      rw [ha] at h
      cases hl : l.mapM f with
      | none => rw [hl] at h; simp at h
      | some bs =>
        rw [hl] at h
        simp only [Option.bind_eq_bind, Option.bind_some, Option.pure_def, Option.some.injEq] at h
        subst h
        rcases List.mem_cons.1 hx with rfl | hx'
        · exact ⟨b, ha, List.mem_cons_self⟩
        · obtain ⟨y, hy, hm⟩ := mapM_some_mem hl x hx'
          exact ⟨y, hy, List.mem_cons_of_mem _ hm⟩

/-- **What chobo's search is held to covers every amount chobo takes that the value can be**: the
    hull lies within 0 to 2⁶³ − 1, and any value of an output's range or a range dandori knows that
    chobo takes lies inside it. -/
theorem amountsHull_covers {α : Type} {outputs : List (Found (OutputValues α))}
    {ranges : List (Option Int × Option Int)} {lo hi : Int} (h : amountsHull outputs ranges = some (lo, hi)) :
    0 ≤ lo ∧ hi ≤ maxAmount ∧
      (∀ o ∈ outputs, ∃ out a b, o = .value out ∧ out.min = some a ∧ out.max = some b ∧
        ∀ v, a ≤ v → v ≤ b → 0 ≤ v → v ≤ maxAmount → lo ≤ v ∧ v ≤ hi) ∧
      (∀ r ∈ ranges, ∃ a b, r = (some a, some b) ∧
        ∀ v, a ≤ v → v ≤ b → 0 ≤ v → v ≤ maxAmount → lo ≤ v ∧ v ≤ hi) := by
  unfold amountsHull at h
  simp only at h
  split at h
  · next A B hA hB =>
    split at h
    · next l ls hh hs heqL heqH =>
      split at h
      · next hle =>
        simp only [Option.some.injEq, Prod.mk.injEq] at h
        obtain ⟨rfl, rfl⟩ := h
        have hlo : ∀ p ∈ A ++ B, max (ls.foldl min l) 0 ≤ max p.1 0 := by
          intro p hp
          have : p.1 ∈ l :: ls := by rw [← heqL]; exact List.mem_map.2 ⟨p, hp, rfl⟩
          have := foldl_min_le this
          omega
        have hhi : ∀ p ∈ A ++ B, min p.2 maxAmount ≤ min (hs.foldl max hh) maxAmount := by
          intro p hp
          have : p.2 ∈ hh :: hs := by rw [← heqH]; exact List.mem_map.2 ⟨p, hp, rfl⟩
          have := le_foldl_max this
          omega
        refine ⟨by omega, by omega, fun o ho => ?_, fun r hr => ?_⟩
        · obtain ⟨y, hy, hm⟩ := mapM_some_mem hA o ho
          cases o with
          | undecided => simp at hy
          | value out =>
            simp only at hy
            split at hy
            · next a b ha hb =>
              simp only [Option.some.injEq] at hy
              subst hy
              have h1 := hlo (a, b) (List.mem_append.2 (Or.inl hm))
              have h2 := hhi (a, b) (List.mem_append.2 (Or.inl hm))
              exact ⟨out, a, b, rfl, ha, hb, fun v hv1 hv2 hv3 hv4 => ⟨by simp only at h1; omega, by simp only at h2; omega⟩⟩
            · simp at hy
        · obtain ⟨y, hy, hm⟩ := mapM_some_mem hB r hr
          split at hy
          · next a b =>
            simp only [Option.some.injEq] at hy
            subst hy
            have h1 := hlo (a, b) (List.mem_append.2 (Or.inr hm))
            have h2 := hhi (a, b) (List.mem_append.2 (Or.inr hm))
            exact ⟨a, b, rfl, fun v hv1 hv2 hv3 hv4 => ⟨by simp only at h1; omega, by simp only at h2; omega⟩⟩
          · simp at hy
      · exact absurd h (by simp)
    · exact absurd h (by simp)
  · exact absurd h (by simp)

/-! ## X4: the refusals of a transfer, against the errors a task handles -/

/-- What `refusalsMet` found missing on either side. -/
structure Unmet where
  /-- Refusals the operation can come to that the task does not handle. -/
  unhandled : List String
  /-- Errors the task handles that the search found no run for. -/
  unfound : List String
  deriving Repr, DecidableEq

/-- `borders::refusals_met`: the reasons chobo's search finds for the operation `op` (the first
    entry for it), against the errors the task handles, both held to `bounds` (the reasons of the
    book's bounds: the refusals that turn on the amounts). An example is a refusal the operation can
    come to that the task does not handle; when the task only handles more than the search finds,
    it is undecided (the search goes only as deep as chobo's check does). -/
def refusalsMet (found : Found (List (String × List String))) (op : String) (handled bounds : List String) :
    Answer Unmet :=
  match found with
  | .undecided => .undecided
  | .value ops =>
    match ops.find? (fun e => e.1 == op) with
    | none => .undecided
    | some (_, reasons) =>
      let unhandled := reasons.filter (fun r => bounds.contains r && !handled.contains r)
      let unfound := handled.filter (fun h => bounds.contains h && !reasons.contains h)
      if !unhandled.isEmpty then .fails ⟨unhandled, unfound⟩
      else if unfound.isEmpty then .holds else .undecided

theorem mem_of_contains_ne_false {l : List String} {a : String} (h : ¬ l.contains a = false) : a ∈ l := by
  cases hc : l.contains a with
  | false => exact absurd hc h
  | true => exact List.contains_iff_mem.1 hc

theorem nil_of_isEmpty_ne {l : List String} (h : ¬ (!l.isEmpty) = true) : l = [] := by
  cases l with
  | nil => rfl
  | cons _ _ => simp at h

/-- **When X4's refusals hold**, of the refusals that turn on the amounts, the task handles every
    one the search finds for the operation, and the search finds every one the task handles. -/
theorem refusalsMet_holds {ops : List (String × List String)} {op : String} {handled bounds : List String}
    (h : refusalsMet (.value ops) op handled bounds = .holds) :
    ∃ reasons, (op, reasons) ∈ ops ∧ (∀ r ∈ reasons, r ∈ bounds → r ∈ handled) ∧
      (∀ x ∈ handled, x ∈ bounds → x ∈ reasons) := by
  simp only [refusalsMet] at h
  split at h
  · exact absurd h (by simp)
  · next o reasons hf =>
    have hop : o = op := by simpa using List.find?_some hf
    subst hop
    refine ⟨reasons, List.mem_of_find?_eq_some hf, ?_⟩
    split at h
    · exact absurd h (by simp)
    · next h1 =>
      split at h
      · next h2 =>
        have e1 := nil_of_isEmpty_ne h1
        have e2 : List.filter (fun h => bounds.contains h && !reasons.contains h) handled = [] :=
          List.isEmpty_iff.1 h2
        refine ⟨fun r hr hb => ?_, fun x hx hb => ?_⟩
        · refine mem_of_contains_ne_false (fun hc => ?_)
          have : r ∈ List.filter (fun r => bounds.contains r && !handled.contains r) reasons :=
            List.mem_filter.2 ⟨hr, by simp only [Bool.and_eq_true, Bool.not_eq_eq_eq_not, Bool.not_true]; exact ⟨List.contains_iff_mem.2 hb, hc⟩⟩
          rw [e1] at this
          cases this
        · refine mem_of_contains_ne_false (fun hc => ?_)
          have : x ∈ List.filter (fun h => bounds.contains h && !reasons.contains h) handled :=
            List.mem_filter.2 ⟨hx, by simp only [Bool.and_eq_true, Bool.not_eq_eq_eq_not, Bool.not_true]; exact ⟨List.contains_iff_mem.2 hb, hc⟩⟩
          rw [e2] at this
          cases this
      · exact absurd h (by simp)

/-- **When X4's refusals give an example**, it names exactly the refusals that turn on the amounts
    that the search finds and the task does not handle — there is one at least — and those the task
    handles that the search does not find. -/
theorem refusalsMet_fails {ops : List (String × List String)} {op : String} {handled bounds : List String}
    {u : Unmet} (h : refusalsMet (.value ops) op handled bounds = .fails u) :
    ∃ reasons, (op, reasons) ∈ ops ∧ u.unhandled ≠ [] ∧
      (∀ r, r ∈ u.unhandled ↔ r ∈ reasons ∧ r ∈ bounds ∧ r ∉ handled) ∧
      (∀ x, x ∈ u.unfound ↔ x ∈ handled ∧ x ∈ bounds ∧ x ∉ reasons) := by
  simp only [refusalsMet] at h
  split at h
  · exact absurd h (by simp)
  · next o reasons hf =>
    have hop : o = op := by simpa using List.find?_some hf
    subst hop
    refine ⟨reasons, List.mem_of_find?_eq_some hf, ?_⟩
    split at h
    · next h1 =>
      simp only [Answer.fails.injEq] at h
      subst h
      refine ⟨?_, fun r => by simp, fun x => by simp⟩
      intro he
      dsimp only at he
      rw [he] at h1
      exact absurd h1 (by simp)
    · split at h
      · exact absurd h (by simp)
      · exact absurd h (by simp)

/-- When X4's refusals are undecided over what the search found, the task handles a refusal that
    turns on the amounts which the search does not find, and every one the search finds is handled. -/
theorem refusalsMet_undecided {ops : List (String × List String)} {op : String} {handled bounds : List String}
    {reasons : List String} (hf : ops.find? (fun e => e.1 == op) = some (op, reasons))
    (h : refusalsMet (.value ops) op handled bounds = .undecided) :
    (∀ r ∈ reasons, r ∈ bounds → r ∈ handled) ∧ ∃ x ∈ handled, x ∈ bounds ∧ x ∉ reasons := by
  simp only [refusalsMet, hf] at h
  split at h
  · exact absurd h (by simp)
  · next h1 =>
    split at h
    · exact absurd h (by simp)
    · next h2 =>
      have e1 := nil_of_isEmpty_ne h1
      refine ⟨fun r hr hb => ?_, ?_⟩
      · refine mem_of_contains_ne_false (fun hc => ?_)
        have : r ∈ List.filter (fun r => bounds.contains r && !handled.contains r) reasons :=
          List.mem_filter.2 ⟨hr, by simp only [Bool.and_eq_true, Bool.not_eq_eq_eq_not, Bool.not_true]; exact ⟨List.contains_iff_mem.2 hb, hc⟩⟩
        rw [e1] at this
        cases this
      · cases hl : List.filter (fun h => bounds.contains h && !reasons.contains h) handled with
        | nil => rw [hl] at h2; exact absurd rfl h2
        | cons x xs =>
          have hx : x ∈ List.filter (fun h => bounds.contains h && !reasons.contains h) handled := by
            rw [hl]; exact List.mem_cons_self
          obtain ⟨hxh, hp⟩ := List.mem_filter.1 hx
          simp only [Bool.and_eq_true, Bool.not_eq_eq_eq_not, Bool.not_true] at hp
          refine ⟨x, hxh, List.contains_iff_mem.1 hp.1, fun hm => ?_⟩
          rw [List.contains_iff_mem.2 hm] at hp
          exact absurd hp.2 (by simp)

/-! ## X6: the day given to a koyomi date -/

/-- An input of a dates file (`ritsu_ports::DateInput`): its name, whether it is a date (else an
    integer), and its range, both ends in. -/
structure DateInput where
  name : String
  isDate : Bool
  min : Int
  max : Int
  deriving Repr

/-- `borders::input_range` (X6): the range of the date input `input` of a dates file, which every
    day given to it is held to with `daysGiven`. koyomi's own check holds every input of that range
    to the days its calendar has data for (koyomi's E203), so the range is all there is to hold a
    day to. -/
def inputRange (inputs : List DateInput) (input : String) : Option (Int × Int) :=
  (inputs.find? (fun i => i.name == input && i.isDate)).map (fun i => (i.min, i.max))

/-- The range X6 holds a day to is the range of a date input of that name. -/
theorem inputRange_some {inputs : List DateInput} {input : String} {lo hi : Int}
    (h : inputRange inputs input = some (lo, hi)) :
    ∃ i ∈ inputs, i.name = input ∧ i.isDate = true ∧ i.min = lo ∧ i.max = hi := by
  unfold inputRange at h
  obtain ⟨i, hi', he⟩ := Option.map_eq_some_iff.1 h
  have hp := List.find?_some hi'
  simp only [Bool.and_eq_true, beq_iff_eq] at hp
  simp only [Prod.mk.injEq] at he
  exact ⟨i, List.mem_of_find?_eq_some hi', hp.1, hp.2, he.1, he.2⟩

/-! ## X5: a hold, against when it expires -/

/-- `borders::held_until` (X5): whether a hold that expires `expiry` seconds after it is made is
    still held when a call on it comes, `least` to `most` seconds after the hold (`most` none when
    nothing bounds it). A hold whose expiry the clock reaches has expired. The example is the fewest
    seconds, when the call always comes at or after the expiry. -/
def heldUntil (least : Nat) (most : Option Nat) (expiry : Nat) : Answer Nat :=
  if least ≥ expiry then .fails least
  else match most with
    | some m => if m < expiry then .holds else .undecided
    | none => .undecided

/-- **When X5 holds**, every call the flow can make comes before the hold expires. -/
theorem heldUntil_holds {least expiry : Nat} {most : Option Nat}
    (h : heldUntil least most expiry = .holds) : ∃ m, most = some m ∧ ∀ t, t ≤ m → t < expiry := by
  unfold heldUntil at h
  split at h
  · exact absurd h (by simp)
  · split at h
    · next m =>
      split at h
      · next hm => exact ⟨m, rfl, fun t ht => by omega⟩
      · exact absurd h (by simp)
    · exact absurd h (by simp)

/-- **When X5 gives an example**, every call the flow can make comes at or after the hold expires. -/
theorem heldUntil_fails {least expiry v : Nat} {most : Option Nat}
    (h : heldUntil least most expiry = .fails v) : v = least ∧ ∀ t, least ≤ t → expiry ≤ t := by
  unfold heldUntil at h
  split at h
  · next hl =>
    simp only [Answer.fails.injEq] at h
    exact ⟨h.symm, fun t ht => by omega⟩
  · split at h
    · split at h <;> exact absurd h (by simp)
    · exact absurd h (by simp)

/-- **X5, held to chobo's interpreter**: a hold made at `t0` that expires `expiry` seconds later
    (`ChoboModel.deadlineOf` of `pending expires after`) has expired, as `ChoboModel` says, when a
    call comes `t` seconds after it and X5 says the hold always expires first — so chobo refuses the
    post or the void with `expired`. -/
theorem heldUntil_fails_expired {least expiry v t0 t : Nat} {most : Option Nat} {h : ChoboModel.Hold}
    (hf : heldUntil least most expiry = .fails v) (hd : h.deadline = some (t0 + expiry)) (ht : least ≤ t) :
    ChoboModel.expiredAt h (t0 + t) = true := by
  obtain ⟨-, hall⟩ := heldUntil_fails hf
  have := hall t ht
  simp only [ChoboModel.expiredAt, hd, decide_eq_true_eq]
  omega

/-- **X5, held to chobo's interpreter, the other way**: when X5 holds, the hold has not expired when
    any call the flow can make comes. -/
theorem heldUntil_holds_held {least expiry t0 t : Nat} {most : Option Nat} {h : ChoboModel.Hold}
    (hh : heldUntil least most expiry = .holds) (hd : h.deadline = some (t0 + expiry)) :
    ∃ m, most = some m ∧ (t ≤ m → ChoboModel.expiredAt h (t0 + t) = false) := by
  obtain ⟨m, hm, hall⟩ := heldUntil_holds hh
  refine ⟨m, hm, fun ht => ?_⟩
  have := hall t ht
  simp only [ChoboModel.expiredAt, hd, decide_eq_false_iff_not]
  omega

end RitsuCross
