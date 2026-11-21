/-
  A quorum only grows (ringi's DESIGN 17 item 4, 5.1, 5.6, 5.7, 5.12).

  - Counting: the condition a part passes on, its approvals (`countIf … yes`) at least what it
    needs, holds with more answers after the ones it has (`passes_append`).
  - `needed`: the number of approvals a part of `n` people needs grows with `n`, for every quorum
    whose percentage is not negative (`needed_mono`); `at least p%` is `p%` of `n` rounded up, the
    least number whose hundredfold reaches `n * p` (`needed_percent_ceil`, `needed_percent_least`).
  - An answer (`answered_seat`, `answer`): it puts its answers after the ones a part has, takes no
    answer away, and takes no part back from passed or done (`answered_seat_answers`,
    `answered_seat_passed`, `answered_seat_done`, `answered_parts_grown`). The ticket stays at its
    stage with those parts, or every one of them is done and the ticket moves on, or it is
    rejected (`answer_grows`, at the place `stage_in_at` reads the stage from).
  - Retracting (`do_retract`, 5.7): it takes away exactly the answers the one who retracts gave at
    the stage they approved, leaves the answers before it as they are, and leaves the stages after
    it as before the ticket came to them (`retract_answers`). With `same approver approves once, at
    the last stage` the parts the approval did wait for it again (`retract_rewaits`); without it,
    the stages before are as they were (`retract_keeps_before`).
  - `same approver approves once` (5.12): a part done with another stage's approval counts as done
    (`part_done_once`) and keeps its answers (`done_before_answers`); at the last stage it is done
    only with an approval given in its one approver's seat (`answered_at_seat`); and the stage it
    is done at is passed over looking for the approval to retract (`back_step_once`), so that
    retracting the approval undoes it as well (`retract_answers`, `retract_rewaits`).

  Retracting is not the only event the machine takes answers away with: in its code, a send back
  to an earlier stage takes them from that stage on (5.6), a resubmission starts a new round, and
  a late stage that goes up takes them from the parts that go up (5.10). "Monotone" is said here of
  the answer (`answer`), and of what a retraction takes away.

  `stage_in_at`, `retract_answers` and the theorems after it take a ticket of at most 1,000,000
  stages, the most the machine counts a stage's place up to (`index_in`, and `do_retract` unwinds
  from `min (j + 1) 1000000`).
-/
import RingiCore.Glue

namespace RingiCore

-- ── Counting ─────────────────────────────────────────────────────────────────

private theorem countIf_append {α} (l m : List α) (p : α → Bool) :
    countIf (l ++ m) p = countIf l p + countIf m p := by
  simp [countIf, List.filter_append]

private theorem countIf_nonneg {α} (l : List α) (p : α → Bool) : 0 ≤ countIf l p := by
  simp [countIf]

/-- The condition a part passes on, its approvals at least what it needs (DESIGN 5.1, 5.6), holds
    with more answers after the ones it has. -/
theorem passes_append (l m : List Answer) (n : Int) (h : countIf l (fun a => a.yes) ≥ n) :
    countIf (l ++ m) (fun a => a.yes) ≥ n := by
  have := countIf_nonneg m (fun a => a.yes)
  rw [countIf_append]
  omega

-- ── How many approvals a part needs ──────────────────────────────────────────

/-- A part of a stage that names its ways (no quorum) needs one approval (DESIGN 5.1). -/
theorem needed_none (n : Int) : needed none n = 1 := rfl

/-- `all of` needs every one of the part's people (DESIGN 5.1). -/
theorem needed_all (n : Int) : needed (some .all) n = n := rfl

/-- `at least n of` needs `n`, however many people the part has (DESIGN 5.1). -/
theorem needed_at_least (a n : Int) : needed (some (.at_least a)) n = a := rfl

/-- `at least p%` of `n` people is rounded up (DESIGN 5.1): its hundredfold reaches `n * p`, and
    one fewer does not. -/
theorem needed_percent_ceil (c n : Int) :
    n * c ≤ 100 * needed (some (.percent c)) n ∧ 100 * (needed (some (.percent c)) n - 1) < n * c := by
  simp only [needed]
  omega

/-- `at least p%` of `n` people is the least number whose hundredfold reaches `n * p`
    (DESIGN 5.1). -/
theorem needed_percent_least (c n k : Int) (h : n * c ≤ 100 * k) : needed (some (.percent c)) n ≤ k := by
  simp only [needed]
  omega

/-- DESIGN 5.1's example: `at least 60% of board` is 3 of a board of 5. -/
theorem needed_sixty_of_five : needed (some (.percent 60)) 5 = 3 := by decide

/-- The number of approvals a part needs grows with the number of its people (DESIGN 5.1), for
    every quorum whose percentage is not negative. -/
theorem needed_mono (q : Option Quorum) (hq : ∀ c, q = some (.percent c) → 0 ≤ c) {n m : Int}
    (h : n ≤ m) : needed q n ≤ needed q m := by
  match q with
  | none => simp [needed]
  | some .all => simpa [needed] using h
  | some (.at_least a) => simp [needed]
  | some (.percent c) =>
    have hc := hq c rfl
    have := Int.mul_le_mul_of_nonneg_right h hc
    simp only [needed]
    omega

-- ── An answer ────────────────────────────────────────────────────────────────

/-- The answers an answer puts at a part: one for each of its seats at the part, in their order. -/
def seat_answers (x : Indexed Part) (seats : List Seat) (by_ : String) (yes : Bool) (now : Int) :
    List Answer :=
  (seats.filter (fun s => s.part == x.index)).map
    (fun s => ({ by_ := by_, seat := s.seat, yes := yes, at_ := now } : Answer))

/-- An answer puts its answers after the ones the part has, and takes none away (DESIGN 5.8,
    17 item 4). -/
theorem answered_seat_answers (x : Indexed Part) (seats : List Seat) (by_ : String) (yes : Bool)
    (now : Int) :
    (answered_seat x seats by_ yes now).answers = x.item.answers ++ seat_answers x seats by_ yes now := by
  unfold answered_seat; dsimp only; split <;> rfl

/-- An answer leaves a part's people, quorum, way and the stage it waits for as they are. -/
theorem answered_seat_fields (x : Indexed Part) (seats : List Seat) (by_ : String) (yes : Bool)
    (now : Int) :
    (answered_seat x seats by_ yes now).approvers = x.item.approvers ∧
    (answered_seat x seats by_ yes now).needed = x.item.needed ∧
    (answered_seat x seats by_ yes now).way = x.item.way ∧
    (answered_seat x seats by_ yes now).of = x.item.of ∧
    (answered_seat x seats by_ yes now).sole = x.item.sole := by
  unfold answered_seat; dsimp only; split <;> exact ⟨rfl, rfl, rfl, rfl, rfl⟩

/-- A part is passed after an answer when it was, or when the answer is an approval and the part,
    waiting for answers, has as many approvals as it needs with it (DESIGN 5.1, 5.6). -/
theorem answered_seat_state (x : Indexed Part) (seats : List Seat) (by_ : String) (yes : Bool)
    (now : Int) :
    (answered_seat x seats by_ yes now).state = .passed ↔
      x.item.state = .passed ∨
        (yes = true ∧ x.item.state = .active ∧
          countIf (x.item.answers ++ seat_answers x seats by_ yes now) (fun a => a.yes) ≥ x.item.needed) := by
  unfold answered_seat seat_answers
  dsimp only
  split
  · next h =>
    simp only [Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at h
    exact ⟨fun _ => Or.inr ⟨h.1.1, h.1.2, h.2⟩, fun _ => rfl⟩
  · next h =>
    simp only [Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq, not_and] at h
    constructor
    · intro h1; exact Or.inl h1
    · rintro (h1 | ⟨h1, h2, h3⟩)
      · exact h1
      · exact absurd h3 (h ⟨h1, h2⟩)

private theorem answered_seat_state_or (x : Indexed Part) (seats : List Seat) (by_ : String)
    (yes : Bool) (now : Int) :
    (answered_seat x seats by_ yes now).state = x.item.state ∨
      (answered_seat x seats by_ yes now).state = .passed := by
  unfold answered_seat; dsimp only; split
  · exact Or.inr rfl
  · exact Or.inl rfl

/-- An answer takes no part back from passed (DESIGN 17 item 4). -/
theorem answered_seat_passed (x : Indexed Part) (seats : List Seat) (by_ : String) (yes : Bool)
    (now : Int) (h : x.item.state = .passed) : (answered_seat x seats by_ yes now).state = .passed :=
  (answered_seat_state x seats by_ yes now).mpr (Or.inl h)

/-- A rejection leaves a part's state as it is: whether it ends the ticket is `answer`'s to say
    (DESIGN 5.1, 5.7). -/
theorem answered_seat_reject (x : Indexed Part) (seats : List Seat) (by_ : String) (now : Int) :
    (answered_seat x seats by_ false now).state = x.item.state := by
  unfold answered_seat; simp

/-- An answer leaves a done part done (DESIGN 17 item 4): passed, nobody to answer it, checked by
    its agent, done with another stage's approval, or waiting for a later stage's. -/
theorem answered_seat_done (x : Indexed Part) (seats : List Seat) (by_ : String) (yes : Bool)
    (now : Int) (h : part_done x.item = true) : part_done (answered_seat x seats by_ yes now) = true := by
  rcases answered_seat_state_or x seats by_ yes now with h1 | h1
  · unfold part_done at h ⊢; rw [h1]; exact h
  · unfold part_done; rw [h1]

/-- A part after an answer, of the part before it: its answers are the ones before, then more; it is
    passed and done if it was; and it has the same people and needs as many approvals. -/
structure PartGrown (p q : Part) : Prop where
  answers : ∃ more, q.answers = p.answers ++ more
  passed : p.state = .passed → q.state = .passed
  done : part_done p = true → part_done q = true
  approvers : q.approvers = p.approvers
  needed : q.needed = p.needed

private theorem at?_indexed {α} (l : List α) (k : Int) :
    at? (indexed l) k = (at? l k).map (fun p => ({ index := k, item := p } : Indexed α)) := by
  unfold at? indexed
  split
  · rfl
  · next hk =>
    simp only [List.getElem?_map, List.getElem?_zipIdx, Option.map_map]
    congr 1
    funext a
    simp only [Function.comp_apply, Nat.zero_add]
    congr 1
    omega

private theorem at?_map' {α β} (f : α → β) (l : List α) (k : Int) : at? (l.map f) k = (at? l k).map f := by
  unfold at?; split <;> simp

/-- An answer grows every part of the stage, place by place (DESIGN 17 item 4). -/
theorem answered_parts_grown (ps : List Part) (seats : List Seat) (by_ : String) (yes : Bool)
    (now : Int) :
    ((indexed ps).map (fun x => answered_seat x seats by_ yes now)).length = ps.length ∧
      ∀ k q, at? ((indexed ps).map (fun x => answered_seat x seats by_ yes now)) k = some q →
        ∃ p, at? ps k = some p ∧ PartGrown p q := by
  refine ⟨by simp [indexed], ?_⟩
  intro k q h
  rw [at?_map', at?_indexed] at h
  cases hp : at? ps k with
  | none => rw [hp] at h; cases h
  | some p =>
    rw [hp] at h
    cases h
    let x : Indexed Part := { index := k, item := p }
    exact ⟨p, rfl,
      { answers := ⟨_, answered_seat_answers x seats by_ yes now⟩
        passed := answered_seat_passed x seats by_ yes now
        done := answered_seat_done x seats by_ yes now
        approvers := (answered_seat_fields x seats by_ yes now).1
        needed := (answered_seat_fields x seats by_ yes now).2.1 }⟩

private theorem on_ending_ticket {cx : Cx} {s s' : Step} {ending : Ending}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)} (h : on_ending cx s ending on = .ok s') :
    ∃ holds, s'.t = { s.t with holds := holds } := by
  unfold on_ending at h
  simp only [bind, Except.bind] at h
  split at h
  · cases h
  · split at h
    · cases h
    · cases h; exact ⟨_, rfl⟩

private theorem end_ticket_status {cx : Cx} {s s' : Step} {status : Status} {ending : Ending}
    {did : String} {by_ proxy_of note : Option String}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)}
    (h : end_ticket cx s status ending did by_ proxy_of note on = .ok s') : s'.t.status = status := by
  unfold end_ticket at h
  simp only [bind, Except.bind] at h
  split at h
  · cases h
  · next s3 h3 =>
    cases h
    obtain ⟨holds, e⟩ := on_ending_ticket h3
    simp [put, e]

/-- An approval or a rejection (`answer`, DESIGN 5.1, 5.8, 17 item 4) is taken at the stage the
    ticket is at, in the seats the one who gives it has there, and does one of three things: the
    ticket stays at the stage, whose parts are grown by it (`answered_parts_grown`); the approval
    leaves every part done, and the ticket moves on; or the rejection leaves the ticket rejected. -/
theorem answer_grows {cx : Cx} {s s' : Step} {by_ : String} {yes : Bool}
    {clocks : String → Int → Int → Int → Except Refusal Clocks}
    {open_after : String → Int → Int → Except Refusal Int}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)}
    (h : answer cx s by_ yes clocks open_after on = .ok s') :
    ∃ st sts, stage_in s.t (if yes then "approve" else "reject") [Status.in_progress] = .ok st ∧
      seats cx s.t st by_ = .ok sts ∧
      (s'.t = with_stage s.t (index_in s.t)
          { st with parts := (indexed st.parts).map (fun x => answered_seat x sts by_ yes cx.now) } ∨
       (yes = true ∧
          ((indexed st.parts).map (fun x => answered_seat x sts by_ yes cx.now)).all part_done = true) ∨
       (yes = false ∧ s'.t.status = .rejected)) := by
  unfold answer at h
  simp only [bind, Except.bind] at h
  split at h
  · cases h
  · next st hst =>
    split at h
    · cases h
    · next sts hsts =>
      refine ⟨st, sts, hst, hsts, ?_⟩
      cases yes
      · simp only [Bool.false_eq_true, ↓reduceIte] at h
        split at h
        · exact Or.inr (Or.inr ⟨rfl, end_ticket_status h⟩)
        · cases h; exact Or.inl rfl
      · simp only [↓reduceIte] at h
        split at h
        · next hall => exact Or.inr (Or.inl ⟨rfl, hall⟩)
        · cases h; exact Or.inl rfl

-- ── Lists of stages ──────────────────────────────────────────────────────────

private theorem at?_some {α} {l : List α} {i : Int} {x : α} (h : at? l i = some x) :
    0 ≤ i ∧ i.toNat < l.length ∧ l[i.toNat]? = some x := by
  unfold at? at h
  split at h
  · cases h
  · refine ⟨by omega, (List.getElem?_eq_some_iff.mp h).1, h⟩

private theorem split_at {α} {l : List α} {i : Int} {x : α} (h : at? l i = some x) :
    l = takeI l i ++ [x] ++ dropI l (i + 1) ∧ ((takeI l i).length : Int) = i := by
  obtain ⟨h0, hlt, hx⟩ := at?_some h
  have e : (i + 1).toNat = i.toNat + 1 := by omega
  refine ⟨?_, ?_⟩
  · unfold takeI dropI
    rw [e]
    conv => lhs; rw [← List.take_append_drop i.toNat l]
    rw [List.drop_eq_getElem_cons hlt]
    have : l[i.toNat] = x := by
      rw [List.getElem?_eq_getElem hlt] at hx; exact Option.some.inj hx
    simp [this]
  · unfold takeI
    simp [List.length_take]
    omega

private theorem takeI_mid {α} (A B : List α) (x : α) : takeI (A ++ [x] ++ B) (A.length : Int) = A := by
  simp [takeI]

private theorem takeI_mid_succ {α} (A B : List α) (x : α) :
    takeI (A ++ [x] ++ B) ((A.length : Int) + 1) = A ++ [x] := by
  unfold takeI
  have : ((A.length : Int) + 1).toNat = A.length + 1 := by omega
  rw [this, List.append_assoc, List.take_append]
  simp [List.take_of_length_le (Nat.le_succ A.length)]

private theorem dropI_mid {α} (A B : List α) (x : α) : dropI (A ++ [x] ++ B) (A.length : Int) = x :: B := by
  simp [dropI]

private theorem dropI_mid_succ {α} (A B : List α) (x : α) :
    dropI (A ++ [x] ++ B) ((A.length : Int) + 1) = B := by
  unfold dropI
  have : ((A.length : Int) + 1).toNat = A.length + 1 := by omega
  rw [this]; simp

private theorem at?_mid {α} (A B : List α) (x : α) : at? (A ++ [x] ++ B) (A.length : Int) = some x := by
  unfold at?; simp

private theorem at?_map {α β} (f : α → β) (l : List α) (k : Int) : at? (l.map f) k = (at? l k).map f := by
  unfold at?; split <;> simp

private theorem at?_append_left {α} (A B : List α) (k : Int) (hk : k < (A.length : Int)) :
    at? (A ++ B) k = at? A k := by
  unfold at?
  split
  · rfl
  · rw [List.getElem?_append_left (by omega)]

private theorem at?_takeI {α} (l : List α) (j k : Int) (hk : k < j) : at? (takeI l j) k = at? l k := by
  unfold at? takeI
  split
  · rfl
  · rw [List.getElem?_take]
    simp only [show k.toNat < j.toNat by omega, ↓reduceIte]

private theorem indexed_split_map {α} (A B : List α) (f : α → α) :
    (indexed (A ++ B)).map (fun y => if y.index < (A.length : Int) then f y.item else y.item) =
      A.map f ++ B := by
  unfold indexed
  rw [List.zipIdx_append, List.map_append, List.map_append, List.map_map, List.map_map]
  congr 1
  · conv => rhs; rw [← List.zipIdx_map_fst 0 A, List.map_map]
    apply List.map_congr_left
    intro ⟨a, i⟩ hm
    have := (List.mem_zipIdx hm).2.1
    simp only [Function.comp_apply, show ((i : Nat) : Int) < (A.length : Int) by omega, ↓reduceIte]
  · conv => rhs; rw [← List.zipIdx_map_fst (0 + A.length) B]
    apply List.map_congr_left
    intro ⟨a, i⟩ hm
    have := (List.mem_zipIdx hm).1
    simp only [Function.comp_apply, show ¬(((i : Nat) : Int) < (A.length : Int)) by omega, ↓reduceIte]

private theorem with_stage_mid (t : Ticket) (A B : List Stage) (x y : Stage)
    (hs : t.stages = A ++ [x] ++ B) : (with_stage t (A.length : Int) y).stages = A ++ [y] ++ B := by
  simp only [with_stage, hs, takeI_mid, dropI_mid_succ]

/-- The stage an event is taken at (`stage_in`) is the one at the place the event then writes to
    (`index_in`), on a ticket of at most 1,000,000 stages, in a state the event is taken in. -/
theorem stage_in_at {t : Ticket} {ev : String} {states : List Status} {st : Stage}
    (hlen : (t.stages.length : Int) ≤ 1000000) (h : stage_in t ev states = .ok st) :
    t.status ∈ states ∧ at? t.stages (index_in t) = some st := by
  unfold stage_in at h
  split at h
  · next hc =>
    split at h
    · cases h
    · next i hi =>
      split at h
      · cases h
      · next st' hst =>
        cases h
        refine ⟨by simpa using hc, ?_⟩
        have := at?_some hst
        have e : max (min i 1000000) 0 = i := by omega
        show at? t.stages (match at_of t with | none => 0 | some i => max (min i 1000000) 0) = some st
        rw [hi]
        show at? t.stages (max (min i 1000000) 0) = some st
        rw [e]; exact hst
  · cases h

-- ── The clocks of a stage ────────────────────────────────────────────────────

private theorem timer_at_t (s : Step) (timer : Timer) (stage : String) (at_ : Option Int) :
    (timer_at s timer stage at_).t = s.t := by
  unfold timer_at; split <;> rfl

private theorem set_times_t (s : Step) (i : Int) (st : Stage) (remind due : Option Int) (now : Int) :
    ∃ r d, (set_times s i st remind due now).t = with_stage s.t i { st with remind := r, due := d } := by
  unfold set_times
  dsimp only
  rw [timer_at_t, timer_at_t]
  exact ⟨_, _, rfl⟩

private theorem set_clocks_shape {cx : Cx} {s s' : Step} {i : Int} {fresh : Bool}
    {clocks : String → Int → Int → Int → Except Refusal Clocks}
    {open_after : String → Int → Int → Except Refusal Int}
    (h : set_clocks cx s i fresh clocks open_after = .ok s') :
    s'.t = s.t ∨
      ∃ st r d, at? s.t.stages i = some st ∧ s'.t = with_stage s.t i { st with remind := r, due := d } := by
  unfold set_clocks at h
  split at h
  · cases h; exact Or.inl rfl
  · next st hst =>
    split at h
    · cases h; exact Or.inl rfl
    · simp only [bind, Except.bind] at h
      split at h
      · cases h
      · cases h
        obtain ⟨r, d, e⟩ := set_times_t s i st _ _ cx.now
        exact Or.inr ⟨st, r, d, hst, e⟩

/-- Setting a stage's clocks changes no stage but its reminder and deadline. -/
private theorem set_clocks_mid {cx : Cx} {s s' : Step} {i : Int} {fresh : Bool}
    {clocks : String → Int → Int → Int → Except Refusal Clocks}
    {open_after : String → Int → Int → Except Refusal Int}
    (h : set_clocks cx s i fresh clocks open_after = .ok s') (A B : List Stage) (x : Stage)
    (hs : s.t.stages = A ++ [x] ++ B) (hi : i = (A.length : Int)) :
    s'.t.stage = s.t.stage ∧
      ∃ x', s'.t.stages = A ++ [x'] ++ B ∧ x'.parts = x.parts ∧ x'.phase = x.phase := by
  rcases set_clocks_shape h with e | ⟨st, r, d, hst, e⟩
  · rw [e]; exact ⟨rfl, x, hs, rfl, rfl⟩
  · rw [hs, hi, at?_mid] at hst
    cases hst
    rw [e, hi]
    exact ⟨rfl, _, with_stage_mid _ _ _ _ _ hs, rfl, rfl⟩

private theorem cancel_clocks_t (s : Step) : (cancel_clocks s).t = s.t := by
  unfold cancel_clocks cancel_due cancel_remind
  split
  · rfl
  · split
    · rfl
    · split
      · rfl
      · split <;> split <;> rfl

-- ── Retracting an approval ───────────────────────────────────────────────────

/-- The answers of each part of a stage. -/
def stage_answers (st : Stage) : List (List Answer) := st.parts.map (·.answers)

/-- A stage at or before the one an approval is retracted at, as `unwind` leaves it: with `same
    approver approves once, at the last stage`, its parts done with an approval undone wait for it
    again (`rewait`); else it is as it was. -/
def rewaited (cx : Cx) (undone : List String) (st : Stage) : Stage :=
  if cx.spec.once = .last then rewait st undone else st

/-- Taking a person's answers from a part takes exactly the answers they gave. -/
theorem unanswered_answers (p : Part) (by_ : String) :
    (unanswered p by_).answers = p.answers.filter (fun a => a.by_ != by_) := by
  unfold unanswered; dsimp only; split <;> rfl

/-- Taking a person's answers from a part takes it back from passed where its approvals left fall
    short of what it needs (DESIGN 5.7); it leaves every other state as it is. -/
theorem unanswered_state (p : Part) (by_ : String) :
    (unanswered p by_).state =
      if p.state = .passed ∧ countIf (p.answers.filter (fun a => a.by_ != by_)) (fun a => a.yes) < p.needed
      then .active else p.state := by
  unfold unanswered; dsimp only
  split <;> split <;> simp_all

private theorem unanswered_of (p : Part) (by_ : String) : (unanswered p by_).of = p.of := by
  unfold unanswered; dsimp only; split <;> rfl

private theorem rewaited_answers (cx : Cx) (u : List String) (st : Stage) :
    stage_answers (rewaited cx u st) = stage_answers st := by
  unfold rewaited stage_answers
  split
  · simp only [rewait, List.map_map]
    apply List.map_congr_left
    intro p _
    simp only [Function.comp_apply]
    split <;> rfl
  · rfl

private theorem unwind_shape (cx : Cx) (t : Ticket) (A B : List Stage) (x : Stage)
    (hs : t.stages = A ++ [x] ++ B) (first j : Int) (hf : first = (A.length : Int) + 1)
    (hj : j = (A.length : Int)) :
    ∃ tail, (unwind cx t first (some j)).stages =
        (A ++ [x]).map (rewaited cx (B.map (·.name) ++ [x.name])) ++ tail ∧
      ∀ st ∈ tail, st.parts = [] ∧ st.phase = .ahead := by
  subst hf hj
  unfold unwind
  dsimp only
  rw [hs, takeI_mid_succ, dropI_mid_succ]
  have hname : name_at (A ++ [x] ++ B) (A.length : Int) = x.name := by
    unfold name_at; rw [at?_mid]
  rw [hname]
  refine ⟨((indexed B).filter (fun y => !(A ++ [x]).any (fun z => z.name == y.item.name) &&
      !(takeI B y.index).any (fun z => z.name == y.item.name))).map (fun y => reset y.item), ?_, ?_⟩
  · unfold rewaited
    by_cases hl : cx.spec.once = .last
    · simp only [hl, beq_self_eq_true, ↓reduceIte]
      have := indexed_split_map (A ++ [x])
        (((indexed B).filter (fun y => !(A ++ [x]).any (fun z => z.name == y.item.name) &&
          !(takeI B y.index).any (fun z => z.name == y.item.name))).map (fun y => reset y.item))
        (fun st => rewait st (B.map (·.name) ++ [x.name]))
      simp only [List.length_append, List.length_singleton] at this
      push_cast at this
      exact this
    · have : (cx.spec.once == Once.last) = false := by simpa using hl
      simp only [this, hl, Bool.false_eq_true, ↓reduceIte, List.map_id']
  · intro st hst
    simp only [List.mem_map] at hst
    obtain ⟨y, _, rfl⟩ := hst
    exact ⟨rfl, rfl⟩

/-- What a retraction (`do_retract`, DESIGN 5.7) does to the stages, exactly: it is taken at a
    stage `j` the one who retracts approved; the ticket goes back to it; the stages before it are
    as `unwind` leaves them (`rewaited`); the stage `j` has the parts with that person's answers
    taken away (`unanswered`), and is waited at again; and every stage after it is as before the
    ticket came to it, with no part. -/
theorem retract_shape {cx : Cx} {s s' : Step} {by_ : String}
    {clocks : String → Int → Int → Int → Except Refusal Clocks}
    {open_after : String → Int → Int → Except Refusal Int}
    (hlen : (s.t.stages.length : Int) ≤ 1000000)
    (h : do_retract cx s by_ clocks open_after = .ok s') :
    ∃ j sj given sjx tail, at? s.t.stages j = some sj ∧
      first? (sj.parts.flatMap (fun p => p.answers)) (fun a => a.by_ == by_ && a.yes) = some given ∧
      s'.t.stage = some sj.name ∧
      s'.t.stages = (takeI s.t.stages j).map
          (rewaited cx ((dropI s.t.stages (j + 1)).map (·.name) ++ [sj.name])) ++ [sjx] ++ tail ∧
      sjx.parts = (rewaited cx ((dropI s.t.stages (j + 1)).map (·.name) ++ [sj.name])
                    { sj with parts := sj.parts.map (fun p => unanswered p by_), phase := .active }).parts ∧
      ∀ st ∈ tail, st.parts = [] ∧ st.phase = .ahead := by
  unfold do_retract at h
  simp only [bind, Except.bind] at h
  split at h
  · cases h
  · split at h
    · split at h
      · cases h
      · split at h
        · cases h
        · next j0 _ =>
          split at h
          · cases h
          · next sj hsj =>
            split at h
            · cases h
            · next given hgiven =>
              split at h
              · cases h
              · next s3 h3 =>
                generalize max (min j0 1000000) 0 = j at hsj h3 h
                obtain ⟨hsplit, hA⟩ := split_at hsj
                have hjl := at?_some hsj
                have hfirst : min (j + 1) 1000000 = ((takeI s.t.stages j).length : Int) + 1 := by omega
                have hreo : j = ((takeI s.t.stages j).length : Int) := hA.symm
                rw [cancel_clocks_t] at h3
                have hs1 : (with_stage s.t j
                      { sj with parts := sj.parts.map (fun p => unanswered p by_), phase := .active }).stages =
                    takeI s.t.stages j ++ [{ sj with parts := sj.parts.map (fun p => unanswered p by_), phase := .active }] ++
                      dropI s.t.stages (j + 1) := rfl
                obtain ⟨tail, hu, htail⟩ := unwind_shape cx _ _ _ _ hs1 _ _ hfirst hreo
                have hs' : s'.t = s3.t := by
                  split at h
                  · cases h; rfl
                  · cases h; unfold tell; split <;> rfl
                obtain ⟨hstage, x', hx', hxp, -⟩ := set_clocks_mid h3 _ tail _
                  (hu.trans (by rw [List.map_append]; rfl)) (by rw [List.length_map]; exact hreo)
                refine ⟨j, sj, given, x', tail, hsj, hgiven, ?_, ?_, hxp, htail⟩
                · rw [hs', hstage]; rfl
                · rw [hs', hx']
    · cases h

private theorem first?_mem {α} {l : List α} {p : α → Bool} {a : α} (h : first? l p = some a) :
    a ∈ l ∧ p a = true :=
  ⟨List.mem_of_find?_eq_some h, List.find?_some h⟩

/-- A retraction takes away exactly the answers the one who retracts gave at the stage `j` they
    approved, in every part of it, and no other answer at it or before it; the stages after it have
    none, being as before the ticket came to them, and the ticket goes back to `j` (DESIGN 5.7,
    17 item 4). -/
theorem retract_answers {cx : Cx} {s s' : Step} {by_ : String}
    {clocks : String → Int → Int → Int → Except Refusal Clocks}
    {open_after : String → Int → Int → Except Refusal Int}
    (hlen : (s.t.stages.length : Int) ≤ 1000000)
    (h : do_retract cx s by_ clocks open_after = .ok s') :
    ∃ j sj n, at? s.t.stages j = some sj ∧
      (∃ a ∈ sj.parts.flatMap (fun p => p.answers), a.by_ = by_ ∧ a.yes = true) ∧
      s'.t.stage = some sj.name ∧
      s'.t.stages.map stage_answers =
        (takeI s.t.stages j).map stage_answers ++
          [sj.parts.map (fun p => p.answers.filter (fun a => a.by_ != by_))] ++ List.replicate n [] := by
  obtain ⟨j, sj, given, sjx, tail, hsj, hgiven, hstage, hst, hx, htail⟩ := retract_shape hlen h
  obtain ⟨hmem, hp⟩ := first?_mem hgiven
  simp only [Bool.and_eq_true, beq_iff_eq] at hp
  refine ⟨j, sj, tail.length, hsj, ⟨given, hmem, hp⟩, hstage, ?_⟩
  rw [hst, List.map_append, List.map_append, List.map_map]
  congr 1
  · congr 1
    · apply List.map_congr_left
      intro st _
      exact rewaited_answers cx _ st
    · have : stage_answers sjx = stage_answers (rewaited cx ((dropI s.t.stages (j + 1)).map (·.name) ++ [sj.name])
          { sj with parts := sj.parts.map (fun p => unanswered p by_), phase := .active }) := by
        unfold stage_answers; rw [hx]
      rw [List.map_singleton, this, rewaited_answers]
      simp only [stage_answers, List.map_map]
      congr 1
      apply List.map_congr_left
      intro p _
      exact unanswered_answers p by_
  · rw [List.eq_replicate_iff]
    refine ⟨List.length_map .., ?_⟩
    intro b hb
    simp only [List.mem_map] at hb
    obtain ⟨st, hst, rfl⟩ := hb
    simp [stage_answers, (htail st hst).1]

private theorem once_in_of (p : Part) (u : List String) (x : String) (h1 : p.state = .once)
    (h2 : p.of = some x) (h3 : x ∈ u) : once_in p u = true := by
  unfold once_in
  simp [h1, h2, h3]

private theorem rewait_part (st : Stage) (u : List String) (m : Int) (p : Part)
    (hp : at? st.parts m = some p) (ho : once_in p u = true) :
    at? (rewait st u).parts m = some { p with state := .deferred } := by
  simp only [rewait, at?_map, hp, Option.map_some, ho, ↓reduceIte]

/-- With `same approver approves once, at the last stage` (DESIGN 5.12), a retraction undoes what
    the approval did: every part at or before the stage `j` it is taken at that was done with the
    approval of `j` or of a stage after it waits for that stage's approval again. -/
theorem retract_rewaits {cx : Cx} {s s' : Step} {by_ : String}
    {clocks : String → Int → Int → Int → Except Refusal Clocks}
    {open_after : String → Int → Int → Except Refusal Int}
    (hlen : (s.t.stages.length : Int) ≤ 1000000) (hlast : cx.spec.once = .last)
    (h : do_retract cx s by_ clocks open_after = .ok s') :
    ∃ j sj, at? s.t.stages j = some sj ∧
      (∃ a ∈ sj.parts.flatMap (fun p => p.answers), a.by_ = by_ ∧ a.yes = true) ∧
      ∀ k st m p x, k ≤ j → at? s.t.stages k = some st → at? st.parts m = some p →
        p.state = .once → p.of = some x → x ∈ (dropI s.t.stages j).map (·.name) →
        ∃ st' p', at? s'.t.stages k = some st' ∧ at? st'.parts m = some p' ∧ p'.state = .deferred := by
  obtain ⟨j, sj, given, sjx, tail, hsj, hgiven, -, hst, hx, -⟩ := retract_shape hlen h
  obtain ⟨hmem, hp⟩ := first?_mem hgiven
  simp only [Bool.and_eq_true, beq_iff_eq] at hp
  refine ⟨j, sj, hsj, ⟨given, hmem, hp⟩, ?_⟩
  intro k st m p x hk hstk hpm h1 h2 h3
  obtain ⟨hsplit, hA⟩ := split_at hsj
  have hu : x ∈ (dropI s.t.stages (j + 1)).map (·.name) ++ [sj.name] := by
    have : dropI s.t.stages j = sj :: dropI s.t.stages (j + 1) := by
      conv => lhs; rw [hsplit]
      conv => lhs; arg 2; rw [← hA]
      rw [dropI_mid]
    rw [this, List.map_cons, List.mem_cons] at h3
    rcases h3 with h3 | h3
    · exact List.mem_append_right _ (List.mem_singleton.mpr h3)
    · exact List.mem_append_left _ h3
  have hlenA : (((takeI s.t.stages j).map (rewaited cx ((dropI s.t.stages (j + 1)).map (·.name) ++ [sj.name]))).length : Int) = j := by
    rw [List.length_map]; exact hA
  rcases Int.lt_or_eq_of_le hk with hk | hk
  · have hk0 := (at?_some hstk).1
    refine ⟨rewait st ((dropI s.t.stages (j + 1)).map (·.name) ++ [sj.name]), { p with state := .deferred }, ?_,
      rewait_part st _ m p hpm (once_in_of p _ x h1 h2 hu), rfl⟩
    rw [hst, List.append_assoc, at?_append_left _ _ _ (by omega), at?_map, at?_takeI _ _ _ hk, hstk]
    simp [rewaited, hlast]
  · subst hk
    rw [hsj] at hstk
    cases hstk
    have hq : at? (sj.parts.map (fun p => unanswered p by_)) m = some (unanswered p by_) := by
      rw [at?_map, hpm]; rfl
    have hq1 : (unanswered p by_).state = .once := by
      rw [unanswered_state, h1]; simp
    refine ⟨sjx, { unanswered p by_ with state := .deferred }, ?_, ?_, rfl⟩
    · have := at?_mid ((takeI s.t.stages k).map
          (rewaited cx ((dropI s.t.stages (k + 1)).map (·.name) ++ [sj.name]))) tail sjx
      rw [hlenA] at this
      rw [hst]; exact this
    · rw [hx]
      simp only [rewaited, hlast, ↓reduceIte]
      exact rewait_part _ _ m _ hq (once_in_of _ _ x hq1 ((unanswered_of p by_).trans h2) hu)

/-- Without `same approver approves once, at the last stage`, a retraction leaves the stages before
    the one it is taken at as they were (DESIGN 5.7). -/
theorem retract_keeps_before {cx : Cx} {s s' : Step} {by_ : String}
    {clocks : String → Int → Int → Int → Except Refusal Clocks}
    {open_after : String → Int → Int → Except Refusal Int}
    (hlen : (s.t.stages.length : Int) ≤ 1000000) (hlast : cx.spec.once ≠ .last)
    (h : do_retract cx s by_ clocks open_after = .ok s') :
    ∃ j sj, at? s.t.stages j = some sj ∧ takeI s'.t.stages j = takeI s.t.stages j := by
  obtain ⟨j, sj, given, sjx, tail, hsj, -, -, hst, -, -⟩ := retract_shape hlen h
  refine ⟨j, sj, hsj, ?_⟩
  obtain ⟨-, hA⟩ := split_at hsj
  have : (takeI s.t.stages j).map (rewaited cx ((dropI s.t.stages (j + 1)).map (·.name) ++ [sj.name])) =
      takeI s.t.stages j := by
    conv => rhs; rw [← List.map_id (takeI s.t.stages j)]
    apply List.map_congr_left
    intro st _
    simp [rewaited, hlast]
  rw [hst, this]
  conv => lhs; arg 2; rw [← hA]
  rw [takeI_mid]

-- ── `same approver approves once` ────────────────────────────────────────────

/-- A part done with another stage's approval counts as done (DESIGN 5.12): the stage passes as if
    it were answered. -/
theorem part_done_once (p : Part) (h : p.state = .once) : part_done p = true := by
  unfold part_done; rw [h]

/-- `same approver approves once` takes no answer from a part: it leaves the part as it is, or
    done with the approval before it (DESIGN 5.12). -/
theorem done_before_answers (cx : Cx) (stages : List Stage) (looks : List Look) (i : Int) (st : Stage)
    (p : Part) :
    (done_before cx stages looks i st p).part.answers = p.answers ∧
      ((done_before cx stages looks i st p).part = p ∨ (done_before cx stages looks i st p).part.state = .once) := by
  unfold done_before
  split
  · exact ⟨rfl, Or.inl rfl⟩
  · split
    · exact ⟨rfl, Or.inl rfl⟩
    · exact ⟨rfl, Or.inr rfl⟩

/-- With `same approver approves once, at the last stage`, a part that waits for the stage `st` is
    done there only with an approval given at it in the seat of the part's one approver (DESIGN
    5.12): never without anyone's answer. -/
theorem answered_at_seat (stages : List Stage) (pl : Place) (st : Stage) (c : Change)
    (h : answered_at stages pl st = some c) :
    ∃ sa pa w, at? stages pl.stage = some sa ∧ at? sa.parts pl.part = some pa ∧ waits_for pa = some w ∧
      w.to = st.name ∧ c.p = { pa with state := .once, of := some st.name } ∧
      ∃ a ∈ st.parts.flatMap (fun p => p.answers), a.yes = true ∧ a.seat = w.person := by
  unfold answered_at at h
  split at h
  · cases h
  · next sa hsa =>
    split at h
    · cases h
    · next pa hpa =>
      split at h
      · cases h
      · next w hw =>
        split at h
        · next hto =>
          split at h
          · cases h
          · next ans hans =>
            cases h
            obtain ⟨hmem, hp⟩ := first?_mem hans
            simp only [Bool.and_eq_true, beq_iff_eq] at hp
            exact ⟨sa, pa, w, hsa, hpa, hw, by simpa using hto, rfl, ans, hmem, hp⟩
        · cases h

/-- Looking back for the approval to retract passes over a stage done with another stage's approval
    (DESIGN 5.12): retracting goes back to the approval that did it. -/
theorem back_step_once (t : Ticket) (s : Scan) (j : Int) (st : Stage) (hs : s.done = false)
    (hj : at? t.stages j = some st) (h : st.phase = .once) : back_step t s j = s := by
  unfold back_step
  simp [hs, hj, h]

end RingiCore
