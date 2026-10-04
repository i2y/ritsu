/-
  X3 across the border itself: the days a koyomi date comes to, as `KoyomiModel` computes them,
  against a rule's range (X3 (a)) and against a rule's certificate that takes them as its range
  (X3 (b), `range from koyomi`, rulec's §15.174). ritsu's DESIGN 7.5.

  koyomi computes a date on every combination of its inputs' ranges, so the set of days it can
  come to is known exactly (`Dates::values` in koyomi's `ports.rs`). `daysOf` is that set computed
  by the model: every combination (`combos`), every date of the file computed in order
  (`KoyomiModel.DatesFile.run`), the date asked for read off; more combinations than the budget,
  or one where a date stops, leaves it undecided, as the port does.

  - `daysOf_mem`, `mem_daysOf`: a day is in the set exactly when some input of the ranges computes
    it — so the set is what koyomi's date really comes to, and the port that hands it over hands
    over the truth.
  - `x3a_holds`: when X3 (a) holds over that set, for every input of koyomi's ranges the date is
    computed and lies inside the rule's range.
  - `x3a_fails`: the example X3 (a) gives is a day koyomi computes for an input of its ranges, and
    it lies outside the rule's range.
  - `x3b_complete`, `x3b_unique`: a rule whose certificate takes the set as the days of a date axis
    (`Sieve.days`) and passes its checks answers, for every input of koyomi's ranges, every point
    the rule is asked about with koyomi's day on that axis: with a row (the cover), and with exactly
    one row (the cover and the pairs). This is the step from koyomi to the rule that X3 (b) counts
    as held: rulec's checks over the days, and `RulecCert` for what they settle.
-/
import RitsuCross.Borders
import KoyomiModel
import RulecCert

namespace RitsuCross

open KoyomiModel

/-! ## The inputs' ranges, every combination -/

/-- The integers from `lo` to `hi`, both in. -/
def intRange (lo hi : Int) : List Int :=
  (List.range (hi - lo + 1).toNat).map (fun n : Nat => lo + (n : Int))

theorem mem_intRange {lo hi v : Int} : v ∈ intRange lo hi ↔ lo ≤ v ∧ v ≤ hi := by
  unfold intRange
  simp only [List.mem_map, List.mem_range]
  constructor
  · rintro ⟨n, hn, rfl⟩
    omega
  · rintro ⟨h1, h2⟩
    exact ⟨(v - lo).toNat, by omega, by omega⟩

/-- Each value inside its input's range, one value for each input. -/
def InRanges : List KoyomiModel.Input → List Int → Prop
  | [], [] => True
  | i :: is, v :: vs => (i.lo ≤ v ∧ v ≤ i.hi) ∧ InRanges is vs
  | _, _ => False

/-- Every combination of the inputs' values. -/
def combos : List KoyomiModel.Input → List (List Int)
  | [] => [[]]
  | i :: is => (intRange i.lo i.hi).flatMap (fun v => (combos is).map (v :: ·))

theorem mem_combos : ∀ {ins : List KoyomiModel.Input} {vals : List Int},
    vals ∈ combos ins ↔ InRanges ins vals
  | [], vals => by cases vals <;> simp [combos, InRanges]
  | i :: is, vals => by
    cases vals with
    | nil => simp [combos, InRanges]
    | cons v vs =>
      simp only [combos, List.mem_flatMap, List.mem_map, InRanges, mem_intRange]
      constructor
      · rintro ⟨w, hw, us, hus, heq⟩
        simp only [List.cons.injEq] at heq
        obtain ⟨rfl, rfl⟩ := heq
        exact ⟨hw, mem_combos.1 hus⟩
      · rintro ⟨hv, hvs⟩
        exact ⟨v, hv, vs, mem_combos.2 hvs, rfl⟩

/-- How many combinations there are (`Model::combinations`). -/
def count (ins : List KoyomiModel.Input) : Nat :=
  ins.foldl (fun n i => n * (i.hi - i.lo + 1).toNat) 1

/-! ## `mapM` in `Except` -/

theorem mapM_ok_of_mem {α β ε : Type} {f : α → Except ε β} :
    ∀ {l : List α} {ys : List β}, l.mapM f = .ok ys → ∀ x ∈ l, ∃ y, f x = .ok y ∧ y ∈ ys
  | [], _, _, x, hx => by simp at hx
  | a :: l, ys, h, x, hx => by
    rw [List.mapM_cons] at h
    cases ha : f a with
    | error e => rw [ha] at h; exact absurd h (by simp [bind, Except.bind])
    | ok b =>
      rw [ha] at h
      cases hl : l.mapM f with
      | error e => rw [hl] at h; exact absurd h (by simp [bind, Except.bind])
      | ok bs =>
        rw [hl] at h
        simp only [bind, Except.bind, pure, Except.pure, Except.ok.injEq] at h
        subst h
        rcases List.mem_cons.1 hx with rfl | hx'
        · exact ⟨b, ha, List.mem_cons_self⟩
        · obtain ⟨y, hy, hm⟩ := mapM_ok_of_mem hl x hx'
          exact ⟨y, hy, List.mem_cons_of_mem _ hm⟩

theorem mem_of_mapM_ok {α β ε : Type} {f : α → Except ε β} :
    ∀ {l : List α} {ys : List β}, l.mapM f = .ok ys → ∀ y ∈ ys, ∃ x ∈ l, f x = .ok y
  | [], ys, h, y, hy => by
    rw [List.mapM_nil] at h
    simp only [pure, Except.pure, Except.ok.injEq] at h
    subst h
    simp at hy
  | a :: l, ys, h, y, hy => by
    rw [List.mapM_cons] at h
    cases ha : f a with
    | error e => rw [ha] at h; exact absurd h (by simp [bind, Except.bind])
    | ok b =>
      rw [ha] at h
      cases hl : l.mapM f with
      | error e => rw [hl] at h; exact absurd h (by simp [bind, Except.bind])
      | ok bs =>
        rw [hl] at h
        simp only [bind, Except.bind, pure, Except.pure, Except.ok.injEq] at h
        subst h
        rcases List.mem_cons.1 hy with rfl | hy'
        · exact ⟨a, List.mem_cons_self, ha⟩
        · obtain ⟨x, hx, hf⟩ := mem_of_mapM_ok hl y hy'
          exact ⟨x, List.mem_cons_of_mem _ hx, hf⟩

/-! ## The days a date comes to -/

/-- Every combination of a dates file's inputs, run: what every date comes to for each, when
    every one computes and there are no more combinations than `budget` (koyomi's `walk`). -/
def runAll (f : DatesFile) (budget : Nat) : Found (List (Array Int)) :=
  if count f.inputs.toList > budget then .undecided
  else
    match (combos f.inputs.toList).mapM (fun vals => f.run vals.toArray) with
    | .error _ => .undecided
    | .ok outs => .value outs

/-- The days the date `k` comes to, read off the runs. -/
def daysOfRuns (runs : Found (List (Array Int))) (k : Nat) : Found (List Int) :=
  match runs with
  | .undecided => .undecided
  | .value outs => .value (outs.map (fun out => out[k]!))

/-- `Dates::values`: every day the date `k` comes to, over every combination of the inputs. -/
def daysOf (f : DatesFile) (k budget : Nat) : Found (List Int) :=
  daysOfRuns (runAll f budget) k

theorem runAll_value {f : DatesFile} {budget : Nat} {outs : List (Array Int)}
    (h : runAll f budget = .value outs) :
    (combos f.inputs.toList).mapM (fun vals => f.run vals.toArray) = .ok outs := by
  unfold runAll at h
  split at h
  · exact absurd h (by simp)
  · split at h
    · exact absurd h (by simp)
    · next outs' heq =>
      simp only [Found.value.injEq] at h
      subst h
      exact heq

/-- **Every input of koyomi's ranges computes the date, and the day is in the set.** -/
theorem daysOf_mem {f : DatesFile} {k budget : Nat} {S : List Int} (h : daysOf f k budget = .value S)
    {vals : List Int} (hv : InRanges f.inputs.toList vals) :
    ∃ out, f.run vals.toArray = .ok out ∧ out[k]! ∈ S := by
  unfold daysOf daysOfRuns at h
  split at h
  · exact absurd h (by simp)
  · next outs hr =>
    simp only [Found.value.injEq] at h
    subst h
    obtain ⟨out, hout, hm⟩ := mapM_ok_of_mem (runAll_value hr) vals (mem_combos.2 hv)
    exact ⟨out, hout, List.mem_map.2 ⟨out, hm, rfl⟩⟩

/-- **Every day of the set is one an input of koyomi's ranges computes.** -/
theorem mem_daysOf {f : DatesFile} {k budget : Nat} {S : List Int} (h : daysOf f k budget = .value S)
    {d : Int} (hd : d ∈ S) :
    ∃ vals, InRanges f.inputs.toList vals ∧ ∃ out, f.run vals.toArray = .ok out ∧ out[k]! = d := by
  unfold daysOf daysOfRuns at h
  split at h
  · exact absurd h (by simp)
  · next outs hr =>
    simp only [Found.value.injEq] at h
    subst h
    obtain ⟨out, hm, rfl⟩ := List.mem_map.1 hd
    obtain ⟨vals, hvals, hrun⟩ := mem_of_mapM_ok (runAll_value hr) out hm
    exact ⟨vals, mem_combos.1 hvals, out, hrun, rfl⟩

/-! ## X3 (a), across the border -/

/-- **When X3 (a) holds over koyomi's days**, every input of koyomi's ranges computes the date, and
    the day lies inside the range the rule declares for its date input. -/
theorem x3a_holds {f : DatesFile} {k budget : Nat} {range : Range}
    (h : daysFit (daysOf f k budget) range = .holds) :
    ∀ vals, InRanges f.inputs.toList vals → ∃ out, f.run vals.toArray = .ok out ∧ range.mem out[k]! := by
  intro vals hv
  cases hd : daysOf f k budget with
  | undecided => rw [hd] at h; exact absurd h (by simp [daysFit])
  | value S =>
    rw [hd] at h
    obtain ⟨out, hout, hm⟩ := daysOf_mem hd hv
    exact ⟨out, hout, daysFit_holds h _ hm⟩

/-- **The example X3 (a) gives over koyomi's days** is a day koyomi computes for an input of its
    ranges, outside the range the rule declares. -/
theorem x3a_fails {f : DatesFile} {k budget : Nat} {range : Range} {d : Int}
    (h : daysFit (daysOf f k budget) range = .fails d) :
    ∃ vals, InRanges f.inputs.toList vals ∧ ∃ out, f.run vals.toArray = .ok out ∧ out[k]! = d ∧
      ¬ range.mem d := by
  cases hd : daysOf f k budget with
  | undecided => rw [hd] at h; exact absurd h (by simp [daysFit])
  | value S =>
    rw [hd] at h
    obtain ⟨hm, hout, -⟩ := daysFit_fails h
    obtain ⟨vals, hv, out, hrun, hk⟩ := mem_daysOf hd hm
    exact ⟨vals, hv, out, hrun, hk, hout⟩

/-! ## X3 (b), across the border: a rule over koyomi's days -/

open RulecCert

/-- Everything `Sieve.asked` asks of the values `v` behind the point `p`, but the days of the axis
    `i`: their coordinates, the constraints, the derived columns' reach, the tables above, the
    linear model, and the days of every other axis. -/
def AskedBut (s : Sieve) (i : Nat) (p : Point) (v : List Rat) : Prop :=
  (∀ (j c : Nat) (x : Coord), p[j]? = some c → s.coordAt j c = some x →
    ∃ w, v[j]? = some w ∧ x.holds w) ∧
  (∀ k ∈ s.cons, ∃ x y, v[k.left]? = some x ∧ v[k.right]? = some y ∧ k.op.holds x y) ∧
  (∀ (j : Nat) (I : Ival), s.reach[j]? = some (some I) → ∃ w, v[j]? = some w ∧ inIval I w) ∧
  (∀ q ∈ s.never, p[q.1]? ≠ some q.2) ∧
  (∀ qr ∈ s.apart, ¬(p[qr.1.1]? = some qr.1.2 ∧ p[qr.2.1]? = some qr.2.2)) ∧
  (∀ q ∈ s.facts, q.holds v) ∧
  (∀ (j : Nat) (D : List Rat), j ≠ i → s.days[j]? = some (some D) → ∃ w, v[j]? = some w ∧ w ∈ D)

/-- A point is asked about when its values satisfy the rest and the axis `i` holds a day of its
    set. -/
theorem asked_of_askedBut {s : Sieve} {i : Nat} {p : Point} {v : List Rat} {D : List Rat}
    {w : Rat} (hD : s.days[i]? = some (some D)) (hrest : AskedBut s i p v) (hw : v[i]? = some w)
    (hmem : w ∈ D) : s.asked p := by
  obtain ⟨h1, h2, h3, h4, h5, h6, h7⟩ := hrest
  refine ⟨v, h1, h2, h3, h4, h5, h6, fun j D' hj => ?_⟩
  by_cases hji : j = i
  · subst hji
    rw [hD] at hj
    simp only [Option.some.injEq] at hj
    subst hj
    exact ⟨w, hw, hmem⟩
  · exact h7 j D' hji hj

/-- The day koyomi computes, among the certificate's days when they are koyomi's set. -/
theorem day_in_certificate {f : DatesFile} {k budget : Nat} {S : List Int}
    (hS : daysOf f k budget = .value S) {vals : List Int} (hv : InRanges f.inputs.toList vals) :
    ∃ out, f.run vals.toArray = .ok out ∧ ((out[k]! : Int) : Rat) ∈ S.map (fun d : Int => (d : Rat)) := by
  obtain ⟨out, hout, hm⟩ := daysOf_mem hS hv
  exact ⟨out, hout, List.mem_map.2 ⟨_, hm, rfl⟩⟩

/-- **X3 (b), complete**: a table whose certificate takes koyomi's set as the days of the axis `i`
    and passes the cover check answers, for every input of koyomi's ranges, every point the rule is
    asked about with koyomi's day on that axis. -/
theorem x3b_complete (C : Certified) {f : DatesFile} {k budget i : Nat} {S : List Int}
    (hS : daysOf f k budget = .value S)
    (hD : C.sieve.days[i]? = some (some (S.map (fun d : Int => (d : Rat)))))
    (hc : C.coverChecks = true) {vals : List Int} (hv : InRanges f.inputs.toList vals) :
    ∃ out, f.run vals.toArray = .ok out ∧
      ∀ (p : Point) (v : List Rat), inSpace C.arities p = true → AskedBut C.sieve i p v →
        v[i]? = some ((out[k]! : Int) : Rat) → C.table.firing p ≠ [] := by
  obtain ⟨out, hout, hm⟩ := day_in_certificate hS hv
  refine ⟨out, hout, fun p v hsp hrest hw => ?_⟩
  exact C.complete hc p hsp (asked_of_askedBut hD hrest hw hm)

/-- **X3 (b), unique**: the same, with the pairs check: exactly one row answers. -/
theorem x3b_unique (C : Certified) {f : DatesFile} {k budget i : Nat} {S : List Int}
    (hS : daysOf f k budget = .value S)
    (hD : C.sieve.days[i]? = some (some (S.map (fun d : Int => (d : Rat)))))
    (hc : C.coverChecks = true) (hp : C.pairAskedChecks = true) (hu : ∀ a b, C.undecided a b = false)
    {vals : List Int} (hv : InRanges f.inputs.toList vals) :
    ∃ out, f.run vals.toArray = .ok out ∧
      ∀ (p : Point) (v : List Rat), inSpace C.arities p = true → AskedBut C.sieve i p v →
        v[i]? = some ((out[k]! : Int) : Rat) → (C.table.firing p).length = 1 := by
  obtain ⟨out, hout, hm⟩ := day_in_certificate hS hv
  refine ⟨out, hout, fun p v hsp hrest hw => ?_⟩
  exact C.unique hc hp hu p hsp (asked_of_askedBut hD hrest hw hm)

end RitsuCross
