/-
  Every ending closes the open holds (ringi's DESIGN 17 item 3, 7.4, 7.6), where the `on` lines of
  the ending are there for them.

  `on_ending` reads the `on` lines of an ending in their order (`on_line`): a post or a void is
  given only while a hold of its book and transfer of the ticket's round is open, and closes the
  first such hold; it is skipped, its condition and arguments not read, where none is open (7.4);
  a line whose condition is false is skipped, its arguments not read; and a condition or arguments
  refused stop the reading, and the ending is refused.

  The lines are there for the holds (`HoldsCovered`) when, for each book and transfer, the open
  holds of the round are no more than the lines that post or void one of them with a condition
  that is not false. Then, with no `hold` among the lines:

  - the ending is refused, or no hold of the round is left open, and every hold left was open
    before (`on_ending_closes`): a refused condition or argument never leaves a hold open in
    silence;
  - with every condition and argument worked out, the ending is not refused (`on_ending_ok`), and
    no hold of the round is left (`on_ending_ok_closes`);
  - a line that is read whatever is open (a `hold` or a `launch`) whose condition, or arguments
    where it holds, are refused refuses the ending (`on_ending_refused`), as does a refusal of the
    lines themselves (`on_ending_on_refused`).

  The same holds of the three ways a ticket's round ends: `end_ticket` (rejected, withdrawn,
  expired), `finish` (approved) and `to_requester` (sent back, or withdrawn to the requester).

  The theorems hold whatever the request's `on` lines answer: they are given as `on`, and the
  theorems take what they answer for the ticket as it is before the ending.
-/
import RingiCore.Glue

namespace RingiCore

-- ── Counting the holds ───────────────────────────────────────────────────────

/-- The holds of the round `r` open for a book and a transfer. -/
def open_holds (holds : List Held) (r : Int) (book transfer : String) : Nat :=
  holds.countP (fun h => h.book == book && h.transfer == transfer && h.round == r)

private theorem indexed_items {α} (l : List α) : (indexed l).map (·.item) = l := by
  have : ((fun (x : Indexed α) => x.item) ∘
      fun (x : α × Nat) => ({ index := ↑x.snd, item := x.fst } : Indexed α)) = Prod.fst := by
    funext x; rfl
  simp [indexed, this]

private theorem without_sublist (holds : List Held) (k : Int) : (without holds k).Sublist holds := by
  unfold without
  conv => rhs; rw [← indexed_items holds]
  exact List.filter_sublist.map _

private theorem countP_split {α} (f g : α → Bool) : ∀ l : List α,
    l.countP f = l.countP (fun x => f x && g x) + l.countP (fun x => f x && !g x)
  | [] => rfl
  | a :: l => by
    simp only [List.countP_cons, countP_split f g l]
    cases f a <;> cases g a <;> simp <;> omega

private theorem countP_without (holds : List Held) (q : Held → Bool) (h : Indexed Held)
    (hm : h ∈ indexed holds) (hq : q h.item = true) :
    (without holds h.index).countP q + 1 ≤ holds.countP q := by
  have e1 : (without holds h.index).countP q =
      (indexed holds).countP (fun x => q x.item && x.index != h.index) := by
    unfold without
    rw [List.countP_map, List.countP_filter]
    congr 1
  have e2 : holds.countP q = (indexed holds).countP (fun x => q x.item) := by
    conv => lhs; rw [← indexed_items holds]
    rw [List.countP_map]; rfl
  rw [e1, e2, countP_split (fun x => q x.item) (fun x => x.index != h.index) (indexed holds)]
  have : 0 < (indexed holds).countP (fun x => q x.item && !(x.index != h.index)) :=
    List.countP_pos_iff.mpr ⟨h, hm, by simp [hq]⟩
  omega

-- ── The `on` lines of an ending ──────────────────────────────────────────────

/-- A condition that is not false: it holds, or it is refused. -/
def not_false : Except Refusal Bool → Bool
  | .ok false => false
  | _ => true

/-- An `on` line posts or voids a hold of a book and a transfer, with a condition that is not
    false. -/
def line_closes (book transfer : String) (l : OnLine) : Bool :=
  closes l && l.book == book && l.transfer == transfer && not_false l.when_

/-- The `on` lines that post or void a hold of a book and a transfer with a condition that is not
    false. -/
def closing_lines (lines : List OnLine) (book transfer : String) : Nat :=
  lines.countP (line_closes book transfer)

/-- The lines are there for the holds of the round `r`: for each book and transfer, the open holds
    are no more than the lines that post or void one of them with a condition that is not false. -/
def HoldsCovered (holds : List Held) (r : Int) (lines : List OnLine) : Prop :=
  ∀ book transfer, open_holds holds r book transfer ≤ closing_lines lines book transfer

/-- An `on` line whose condition is worked out, and whose arguments are where it holds. -/
abbrev LineWorkedOut (l : OnLine) : Prop :=
  (∃ w, l.when_ = .ok w) ∧ (l.when_ = .ok true → ∃ a, l.args = .ok a)

/-- An `on` line read whatever holds are open (a `hold` or a `launch`), whose condition is refused,
    or holds with its arguments refused. -/
abbrev LineRefused (l : OnLine) : Prop :=
  closes l = false ∧ ∃ e, l.when_ = .error e ∨ (l.when_ = .ok true ∧ l.args = .error e)

/-- The first open hold of the round of the line's book and transfer, as `on_line` looks for it. -/
private abbrev opens (t : Ticket) (l : OnLine) (h : Indexed Held) : Bool :=
  h.item.book == l.book && h.item.transfer == l.transfer && h.item.round == t.round

-- ── One `on` line ────────────────────────────────────────────────────────────

private theorem on_line_stopped (t : Ticket) (x : Booked) (l : OnLine) (h : x.stop ≠ none) :
    on_line t x l = x := by
  unfold on_line
  split
  · rfl
  · next hn => exact absurd hn h

private theorem on_line_sublist (t : Ticket) (x : Booked) (l : OnLine) (hl : l.action ≠ .hold) :
    (on_line t x l).holds.Sublist x.holds := by
  unfold on_line; dsimp only
  repeat' split
  all_goals first | exact List.Sublist.refl _ | exact without_sublist _ _ | exact absurd ‹_› hl

private theorem open_zero (holds : List Held) (t : Ticket) (l : OnLine)
    (h : first? (indexed holds) (opens t l) = none) :
    open_holds holds t.round l.book l.transfer = 0 := by
  unfold open_holds
  rw [List.countP_eq_zero]
  intro a ha hp
  rw [← indexed_items holds, List.mem_map] at ha
  obtain ⟨y, hy, rfl⟩ := ha
  have := List.find?_eq_none.mp h _ hy
  simp_all

private theorem key_false (l : OnLine) (b tr : String) (hk : ¬(l.book = b ∧ l.transfer = tr)) :
    line_closes b tr l = false := by
  simp only [line_closes, Bool.and_eq_false_iff, beq_eq_false_iff_ne]
  by_cases h1 : l.book = b
  · exact Or.inl (Or.inr (fun h2 => hk ⟨h1, h2⟩))
  · exact Or.inl (Or.inl (Or.inr h1))

/-- One line keeps the lines after it there for the holds: a post or a void that counts for its book
    and transfer closes one of them, or none is open. -/
private theorem on_line_covered (t : Ticket) (x : Booked) (l : OnLine) (rest : List OnLine)
    (hx : x.stop = none) (hl : l.action ≠ .hold) (hc : HoldsCovered x.holds t.round (l :: rest)) :
    ∀ y, on_line t x l = y → y.stop = none → HoldsCovered y.holds t.round rest := by
  intro y hyeq hys b tr
  have hcb := hc b tr
  show open_holds y.holds t.round b tr ≤ closing_lines rest b tr
  unfold closing_lines at hcb ⊢
  rw [List.countP_cons] at hcb
  -- the holds are those before: the line counts for this book and transfer only where none is open
  have same : ∀ (hz : y.holds = x.holds),
      (line_closes b tr l = true → open_holds x.holds t.round l.book l.transfer = 0) →
      open_holds y.holds t.round b tr ≤ rest.countP (line_closes b tr) := by
    intro hz hk
    rw [hz]
    by_cases hK : line_closes b tr l = true
    · have h0 := hk hK
      simp only [line_closes, Bool.and_eq_true, beq_iff_eq] at hK
      obtain ⟨⟨⟨_, rfl⟩, rfl⟩, _⟩ := hK
      omega
    · simp only [Bool.not_eq_true] at hK
      simp only [hK, Bool.false_eq_true, ↓reduceIte] at hcb
      omega
  -- the holds are those before without the first open one of the line's book and transfer
  have less : ∀ (h : Indexed Held), first? (indexed x.holds) (opens t l) = some h →
      closes l = true → not_false l.when_ = true → y.holds = without x.holds h.index →
      open_holds y.holds t.round b tr ≤ rest.countP (line_closes b tr) := by
    intro h hh hcl hnf hz
    rw [hz]
    obtain ⟨hm, hp⟩ := And.intro (List.mem_of_find?_eq_some hh) (List.find?_some hh)
    by_cases hk : l.book = b ∧ l.transfer = tr
    · obtain ⟨rfl, rfl⟩ := hk
      have := countP_without x.holds
        (fun h => h.book == l.book && h.transfer == l.transfer && h.round == t.round) h hm hp
      simp only [line_closes, hcl, beq_self_eq_true, hnf, Bool.and_self, ↓reduceIte] at hcb
      unfold open_holds at hcb ⊢
      omega
    · simp only [key_false l b tr hk, Bool.false_eq_true, ↓reduceIte] at hcb
      have := (without_sublist x.holds h.index).countP_le
        (p := fun h => h.book == b && h.transfer == tr && h.round == t.round)
      unfold open_holds at hcb ⊢
      omega
  unfold on_line at hyeq
  simp only [hx] at hyeq
  generalize ho : first? (indexed x.holds) (opens t l) = o at hyeq
  cases o with
  | none =>
    by_cases hcl : closes l = true
    · simp only [present, hcl, Bool.not_false, Bool.and_self, ↓reduceIte] at hyeq
      subst hyeq
      exact same rfl (fun _ => open_zero x.holds t l ho)
    · simp only [Bool.not_eq_true] at hcl
      simp only [present, hcl, Bool.false_and, Bool.false_eq_true, ↓reduceIte] at hyeq
      have nk : line_closes b tr l = true → open_holds x.holds t.round l.book l.transfer = 0 := by
        intro hK; simp [line_closes, hcl] at hK
      cases hw : l.when_ with
      | error e => simp only [hw] at hyeq; subst hyeq; simp at hys
      | ok yes =>
        cases yes with
        | false => simp only [hw, Bool.false_eq_true, ↓reduceIte] at hyeq; subst hyeq; exact same rfl nk
        | true =>
          cases ha : l.args with
          | error e => simp only [hw, ha, ↓reduceIte] at hyeq; subst hyeq; simp at hys
          | ok args =>
            cases hac : l.action with
            | launch => simp only [hw, ha, hac, ↓reduceIte] at hyeq; subst hyeq; exact same rfl nk
            | hold => exact absurd hac hl
            | post => simp [closes, hac] at hcl
            | release => simp [closes, hac] at hcl
  | some h =>
    simp only [present, Bool.not_true, Bool.and_false, Bool.false_eq_true, ↓reduceIte] at hyeq
    cases hw : l.when_ with
    | error e => simp only [hw] at hyeq; subst hyeq; simp at hys
    | ok yes =>
      cases yes with
      | false =>
        simp only [hw, Bool.false_eq_true, ↓reduceIte] at hyeq; subst hyeq
        exact same rfl (fun hK => by simp [line_closes, hw, not_false] at hK)
      | true =>
        cases ha : l.args with
        | error e => simp only [hw, ha, ↓reduceIte] at hyeq; subst hyeq; simp at hys
        | ok args =>
          have hnf : not_false l.when_ = true := by simp [hw, not_false]
          cases hac : l.action with
          | launch =>
            simp only [hw, ha, hac, ↓reduceIte] at hyeq; subst hyeq
            exact same rfl (fun hK => by simp [line_closes, closes, hac] at hK)
          | hold => exact absurd hac hl
          | post =>
            simp only [hw, ha, hac, ↓reduceIte] at hyeq; subst hyeq
            exact less h ho (by simp [closes, hac]) hnf rfl
          | release =>
            simp only [hw, ha, hac, ↓reduceIte] at hyeq; subst hyeq
            exact less h ho (by simp [closes, hac]) hnf rfl


private theorem on_line_ok (t : Ticket) (x : Booked) (l : OnLine) (hx : x.stop = none)
    (hw : ∃ w, l.when_ = .ok w) (ha : l.when_ = .ok true → ∃ a, l.args = .ok a) :
    (on_line t x l).stop = none := by
  obtain ⟨w, hw⟩ := hw
  unfold on_line
  simp only [hx]
  split
  · exact hx
  · rw [hw]
    cases w with
    | false => exact hx
    | true =>
      obtain ⟨a, ha⟩ := ha hw
      simp only [ha, ↓reduceIte]
      split
      · rfl
      · rfl
      · split
        · exact hx
        · rfl
      · split
        · exact hx
        · rfl

private theorem on_line_refuses (t : Ticket) (x : Booked) (l : OnLine) (hx : x.stop = none)
    (hc : closes l = false)
    (he : ∃ e, l.when_ = .error e ∨ (l.when_ = .ok true ∧ l.args = .error e)) :
    (on_line t x l).stop ≠ none := by
  obtain ⟨e, he⟩ := he
  unfold on_line
  simp only [hx, hc, Bool.false_and, Bool.false_eq_true, ↓reduceIte]
  rcases he with he | ⟨hw, ha⟩
  · simp [he]
  · simp [hw, ha]

/-- The `on` lines as `on_ending` reads them, one after another. -/
private abbrev booked (t : Ticket) (m : Int) (lines : List OnLine) (x : Booked) : Booked :=
  lines.foldl
    (fun (x : Booked) l => on_line t { x with holds := takeI x.holds m, made := takeI x.made m } l) x

private theorem booked_stopped (t : Ticket) (m : Int) :
    ∀ (lines : List OnLine) (x : Booked), x.stop ≠ none → (booked t m lines x).stop ≠ none
  | [], _, h => h
  | l :: rest, x, h => by
    unfold booked
    rw [List.foldl_cons]
    apply booked_stopped t m rest
    rw [on_line_stopped t { x with holds := takeI x.holds m, made := takeI x.made m } l h]
    exact h

private theorem booked_sublist (t : Ticket) (m : Int) :
    ∀ (lines : List OnLine) (x : Booked), (∀ l ∈ lines, l.action ≠ .hold) →
      (booked t m lines x).holds.Sublist x.holds
  | [], _, _ => List.Sublist.refl _
  | l :: rest, x, hh => by
    unfold booked
    rw [List.foldl_cons]
    refine (booked_sublist t m rest _ (fun l' hl' => hh l' (List.mem_cons_of_mem _ hl'))).trans ?_
    exact (on_line_sublist t _ l (hh l List.mem_cons_self)).trans (List.take_sublist _ _)

private theorem covered_sublist {h1 h2 : List Held} {r : Int} {lines : List OnLine}
    (hs : h1.Sublist h2) (hc : HoldsCovered h2 r lines) : HoldsCovered h1 r lines := fun b tr =>
  Nat.le_trans hs.countP_le (hc b tr)

private theorem booked_closes (t : Ticket) (m : Int) :
    ∀ (lines : List OnLine) (x : Booked), (∀ l ∈ lines, l.action ≠ .hold) →
      (x.stop = none → HoldsCovered x.holds t.round lines) →
      (booked t m lines x).stop = none →
        ∀ b tr, open_holds (booked t m lines x).holds t.round b tr = 0
  | [], x, _, hc, hs => fun b tr => by
    have := hc hs b tr
    simp only [closing_lines, List.countP_nil] at this
    exact Nat.le_zero.mp this
  | l :: rest, x, hh, hc, hs => by
    unfold booked at hs ⊢
    rw [List.foldl_cons] at hs ⊢
    apply booked_closes t m rest _ (fun l' hl' => hh l' (List.mem_cons_of_mem _ hl')) _ hs
    intro hy
    by_cases hx : x.stop = none
    · exact on_line_covered t { x with holds := takeI x.holds m, made := takeI x.made m } l rest hx
        (hh l List.mem_cons_self) (covered_sublist (List.take_sublist _ _) (hc hx)) _ rfl hy
    · rw [on_line_stopped t { x with holds := takeI x.holds m, made := takeI x.made m } l hx] at hy
      exact absurd hy hx

private theorem booked_ok (t : Ticket) (m : Int) :
    ∀ (lines : List OnLine) (x : Booked), x.stop = none →
      (∀ l ∈ lines, LineWorkedOut l) →
      (booked t m lines x).stop = none
  | [], _, h, _ => h
  | l :: rest, x, hx, hr => by
    unfold booked
    rw [List.foldl_cons]
    exact booked_ok t m rest _
      (on_line_ok t { x with holds := takeI x.holds m, made := takeI x.made m } l hx
        (hr l List.mem_cons_self).1 (hr l List.mem_cons_self).2)
      (fun l' hl' => hr l' (List.mem_cons_of_mem _ hl'))

private theorem booked_refuses (t : Ticket) (m : Int) :
    ∀ (lines : List OnLine) (x : Booked),
      (∃ l ∈ lines, LineRefused l) →
      (booked t m lines x).stop ≠ none
  | [], _, ⟨_, hl, _⟩ => absurd hl List.not_mem_nil
  | l :: rest, x, ⟨l', hl', hc, he⟩ => by
    unfold booked
    rw [List.foldl_cons]
    rcases List.mem_cons.mp hl' with rfl | hl'
    · apply booked_stopped
      by_cases hx : x.stop = none
      · exact on_line_refuses t { x with holds := takeI x.holds m, made := takeI x.made m } l' hx
          hc he
      · rw [on_line_stopped t { x with holds := takeI x.holds m, made := takeI x.made m } l' hx]
        exact hx
    · exact booked_refuses t m rest _ ⟨l', hl', hc, he⟩

-- ── `on_ending` ──────────────────────────────────────────────────────────────

/-- What `on_ending` asks a ticket's `on` lines with. -/
def ticket_ref (t : Ticket) : Ref :=
  { id := t.id, round := t.round, attempt := t.attempt, requester := t.requester, made_by := t.made_by }

private theorem on_ending_cases {cx : Cx} {s : Step} {ending : Ending}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)} {lines : List OnLine}
    (hon : on ending (ticket_ref s.t) cx.org = .ok lines) :
    ∃ b : Booked,
      b = booked s.t 9223372036854775807 lines { holds := s.t.holds, made := [], stop := none } ∧
      ((b.stop ≠ none ∧ ∃ e, on_ending cx s ending on = .error e) ∨
       (b.stop = none ∧ on_ending cx s ending on =
          .ok (put_all { s with t := { s.t with holds := takeI b.holds 9223372036854775807 } }
            (takeI b.made 9223372036854775807)))) := by
  refine ⟨_, rfl, ?_⟩
  unfold on_ending
  simp only [ticket_ref] at hon
  simp only [bind, Except.bind, hon]
  unfold booked
  split
  · next e he => exact Or.inl ⟨by rw [he]; simp, e, rfl⟩
  · next he => exact Or.inr ⟨he, rfl⟩

/-- An ending whose `on` lines have no `hold` and are there for the holds of the round (DESIGN 17
    item 3, 7.6) is refused, or leaves no hold of the round open: every hold left was open before,
    and the ticket is otherwise as it was. -/
theorem on_ending_closes {cx : Cx} {s s' : Step} {ending : Ending}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)} {lines : List OnLine}
    (hon : on ending (ticket_ref s.t) cx.org = .ok lines) (hhold : ∀ l ∈ lines, l.action ≠ .hold)
    (hcov : HoldsCovered s.t.holds s.t.round lines) (h : on_ending cx s ending on = .ok s') :
    (∀ hd ∈ s'.t.holds, hd ∈ s.t.holds ∧ hd.round ≠ s.t.round) ∧
      s'.t = { s.t with holds := s'.t.holds } := by
  obtain ⟨b, rfl, ⟨-, e, he⟩ | ⟨hs, he⟩⟩ := on_ending_cases (ending := ending) hon
  · rw [he] at h; cases h
  · rw [he] at h
    cases h
    refine ⟨?_, rfl⟩
    intro hd hm
    simp only [put_all] at hm
    have hm' := (List.take_sublist _ _).subset hm
    refine ⟨(booked_sublist s.t _ lines _ hhold).subset hm', ?_⟩
    intro hr
    have h0 := booked_closes s.t _ lines _ hhold (fun _ => hcov) hs hd.book hd.transfer
    unfold open_holds at h0
    rw [List.countP_eq_zero] at h0
    exact h0 hd hm' (by simp [hr])

/-- An ending whose `on` lines have every condition, and the arguments of every line whose condition
    holds, worked out is not refused (DESIGN 7.4). -/
theorem on_ending_ok {cx : Cx} {s : Step} {ending : Ending}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)} {lines : List OnLine}
    (hon : on ending (ticket_ref s.t) cx.org = .ok lines)
    (hread : ∀ l ∈ lines, LineWorkedOut l) :
    ∃ s', on_ending cx s ending on = .ok s' := by
  obtain ⟨b, rfl, ⟨hs, -⟩ | ⟨-, he⟩⟩ := on_ending_cases (ending := ending) hon
  · exact absurd (booked_ok s.t _ lines _ rfl hread) hs
  · exact ⟨_, he⟩

/-- An ending is refused where a line it reads whatever is open (a `hold` or a `launch`) has its
    condition refused, or its condition holding and its arguments refused (DESIGN 7.2, 7.4). -/
theorem on_ending_refused {cx : Cx} {s : Step} {ending : Ending}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)} {lines : List OnLine}
    (hon : on ending (ticket_ref s.t) cx.org = .ok lines)
    (hbad : ∃ l ∈ lines, LineRefused l) :
    ∃ e, on_ending cx s ending on = .error e := by
  obtain ⟨b, rfl, ⟨-, he⟩ | ⟨hs, -⟩⟩ := on_ending_cases (ending := ending) hon
  · exact he
  · exact absurd hs (booked_refuses s.t _ lines _ hbad)

/-- An ending is refused where its `on` lines are. -/
theorem on_ending_on_refused {cx : Cx} {s : Step} {ending : Ending}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)} {e : Refusal}
    (hon : on ending (ticket_ref s.t) cx.org = .error e) : on_ending cx s ending on = .error e := by
  unfold on_ending
  simp only [ticket_ref] at hon
  simp only [bind, Except.bind, hon]

/-- An ending whose `on` lines have no `hold`, every condition and the arguments of every line
    whose condition holds worked out, and as many posts and voids whose condition holds as there are
    open holds of the round of their book and transfer, is not refused, and leaves no hold of the
    round open (DESIGN 17 item 3). -/
theorem on_ending_ok_closes {cx : Cx} {s : Step} {ending : Ending}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)} {lines : List OnLine}
    (hon : on ending (ticket_ref s.t) cx.org = .ok lines) (hhold : ∀ l ∈ lines, l.action ≠ .hold)
    (hread : ∀ l ∈ lines, LineWorkedOut l)
    (hcov : HoldsCovered s.t.holds s.t.round lines) :
    ∃ s', on_ending cx s ending on = .ok s' ∧
      ∀ hd ∈ s'.t.holds, hd ∈ s.t.holds ∧ hd.round ≠ s.t.round := by
  obtain ⟨s', hs'⟩ := on_ending_ok (ending := ending) hon hread
  exact ⟨s', hs', (on_ending_closes hon hhold hcov hs').1⟩

-- ── The endings ──────────────────────────────────────────────────────────────

/-- What a step on the way to the `on` lines keeps of the ticket: its holds, its round, and what
    the lines are asked with. -/
private def Keeps (t t' : Ticket) : Prop :=
  t'.holds = t.holds ∧ t'.round = t.round ∧ ticket_ref t' = ticket_ref t

private theorem Keeps.trans {t1 t2 t3 : Ticket} (h1 : Keeps t1 t2) (h2 : Keeps t2 t3) : Keeps t1 t3 :=
  ⟨h2.1.trans h1.1, h2.2.1.trans h1.2.1, h2.2.2.trans h1.2.2⟩

private theorem keeps_cancel_clocks (s : Step) : Keeps s.t (cancel_clocks s).t := by
  unfold cancel_clocks cancel_due cancel_remind
  split
  · exact ⟨rfl, rfl, rfl⟩
  · split
    · exact ⟨rfl, rfl, rfl⟩
    · split
      · exact ⟨rfl, rfl, rfl⟩
      · split <;> split <;> exact ⟨rfl, rfl, rfl⟩

private theorem keeps_cancel_pause (s : Step) : Keeps s.t (cancel_pause s).t := by
  unfold cancel_pause; split <;> exact ⟨rfl, rfl, rfl⟩

private theorem keeps_cancel_back (s : Step) : Keeps s.t (cancel_back s).t := by
  unfold cancel_back; split <;> exact ⟨rfl, rfl, rfl⟩

private theorem keeps_end (s : Step) :
    Keeps s.t (cancel_back (cancel_pause (cancel_clocks s))).t :=
  (keeps_cancel_clocks s).trans ((keeps_cancel_pause _).trans (keeps_cancel_back _))

private theorem keeps_unpause {cx : Cx} {s : Step} {whole : Bool}
    {open_after : String → Int → Int → Except Refusal Int} {u : Unpaused}
    (h : unpause cx s whole open_after = .ok u) : Keeps s.t u.s.t := by
  unfold unpause at h
  split at h
  · cases h; exact ⟨rfl, rfl, rfl⟩
  · simp only [bind, Except.bind] at h
    split at h
    · cases h
    · cases h; exact ⟨rfl, rfl, rfl⟩

private theorem keeps_back {cx : Cx} {s : Step} {open_after : String → Int → Int → Except Refusal Int}
    {u : Unpaused} (h : unpause cx (cancel_pause (cancel_clocks s)) false open_after = .ok u) :
    Keeps s.t u.s.t :=
  ((keeps_cancel_clocks s).trans (keeps_cancel_pause _)).trans (keeps_unpause h)

private theorem due_back_t {cx : Cx} {s s' : Step}
    {open_after : String → Int → Int → Except Refusal Int}
    (h : due_back cx s open_after = .ok s') : ∃ rd, s'.t = { s.t with resubmit_due := rd } := by
  unfold due_back at h
  split at h
  · cases h; exact ⟨_, rfl⟩
  · simp only [bind, Except.bind] at h
    split at h
    · cases h
    · split at h
      · cases h
      · cases h; exact ⟨_, rfl⟩

private theorem on_ending_closes_of {cx : Cx} {s X s' : Step} {ending : Ending}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)} {lines : List OnLine}
    (hon : on ending (ticket_ref s.t) cx.org = .ok lines) (hhold : ∀ l ∈ lines, l.action ≠ .hold)
    (hcov : HoldsCovered s.t.holds s.t.round lines) (h : on_ending cx X ending on = .ok s')
    (hk : Keeps s.t X.t) :
    (∀ hd ∈ s'.t.holds, hd ∈ s.t.holds ∧ hd.round ≠ s.t.round) ∧
      s'.t = { X.t with holds := s'.t.holds } := by
  obtain ⟨h1, h2, h3⟩ := hk
  rw [← h3] at hon
  rw [← h1, ← h2] at hcov
  have := on_ending_closes hon hhold hcov h
  rw [h1, h2] at this
  exact this

private theorem on_ending_ok_of {cx : Cx} {s X : Step} {ending : Ending}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)} {lines : List OnLine}
    (hon : on ending (ticket_ref s.t) cx.org = .ok lines)
    (hread : ∀ l ∈ lines, LineWorkedOut l)
    {e : Refusal} (h : on_ending cx X ending on = .error e) (hk : Keeps s.t X.t) : False := by
  rw [← hk.2.2] at hon
  obtain ⟨s', hs'⟩ := on_ending_ok (s := X) (ending := ending) hon hread
  rw [hs'] at h; cases h

private theorem on_ending_refused_of {cx : Cx} {s X s' : Step} {ending : Ending}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)} {lines : List OnLine}
    (hon : on ending (ticket_ref s.t) cx.org = .ok lines)
    (hbad : ∃ l ∈ lines, LineRefused l)
    (h : on_ending cx X ending on = .ok s') (hk : Keeps s.t X.t) : False := by
  rw [← hk.2.2] at hon
  obtain ⟨e, he⟩ := on_ending_refused (s := X) (ending := ending) hon hbad
  rw [he] at h; cases h

/-- A ticket that ends (`end_ticket`: rejected, withdrawn or expired) whose `on` lines of the ending
    have no `hold` and are there for the holds of the round (DESIGN 17 item 3, 7.6) is refused, or
    ends in the state it is given with no hold of the round left open. -/
theorem end_ticket_closes {cx : Cx} {s s' : Step} {status : Status} {ending : Ending}
    {did : String}
    {by_ proxy_of note : Option String} {on : Ending → Ref → Org → Except Refusal (List OnLine)}
    {lines : List OnLine}
    (hon : on ending (ticket_ref s.t) cx.org = .ok lines) (hhold : ∀ l ∈ lines, l.action ≠ .hold)
    (hcov : HoldsCovered s.t.holds s.t.round lines)
    (h : end_ticket cx s status ending did by_ proxy_of note on = .ok s') :
    s'.t.status = status ∧ ∀ hd ∈ s'.t.holds, hd ∈ s.t.holds ∧ hd.round ≠ s.t.round := by
  unfold end_ticket at h
  simp only [bind, Except.bind] at h
  split at h
  · cases h
  · next s3 h3 =>
    cases h
    obtain ⟨hc, he⟩ := on_ending_closes_of hon hhold hcov h3 ((keeps_end s).trans ⟨rfl, rfl, rfl⟩)
    exact ⟨by simp only [put]; rw [he]; try rfl, hc⟩

/-- A ticket ends (`end_ticket`) where its `on` lines of the ending have every condition, and the
    arguments of every line whose condition holds, worked out (DESIGN 7.4). -/
theorem end_ticket_ok {cx : Cx} {s : Step} {status : Status} {ending : Ending} {did : String}
    {by_ proxy_of note : Option String} {on : Ending → Ref → Org → Except Refusal (List OnLine)}
    {lines : List OnLine}
    (hon : on ending (ticket_ref s.t) cx.org = .ok lines)
    (hread : ∀ l ∈ lines, LineWorkedOut l) :
    ∃ s', end_ticket cx s status ending did by_ proxy_of note on = .ok s' := by
  unfold end_ticket
  simp only [bind, Except.bind]
  split
  · next e he =>
    exact (on_ending_ok_of hon hread he ((keeps_end s).trans ⟨rfl, rfl, rfl⟩)).elim
  · exact ⟨_, rfl⟩

/-- A ticket does not end (`end_ticket`) where a `hold` or a `launch` line of the ending has its
    condition, or its arguments where it holds, refused (DESIGN 7.2, 7.4). -/
theorem end_ticket_refused {cx : Cx} {s : Step} {status : Status} {ending : Ending}
    {did : String}
    {by_ proxy_of note : Option String} {on : Ending → Ref → Org → Except Refusal (List OnLine)}
    {lines : List OnLine}
    (hon : on ending (ticket_ref s.t) cx.org = .ok lines)
    (hbad : ∃ l ∈ lines, LineRefused l) :
    ∃ e, end_ticket cx s status ending did by_ proxy_of note on = .error e := by
  unfold end_ticket
  simp only [bind, Except.bind]
  split
  · exact ⟨_, rfl⟩
  · next s3 h3 =>
    exact (on_ending_refused_of hon hbad h3 ((keeps_end s).trans ⟨rfl, rfl, rfl⟩)).elim

/-- A ticket ends (`end_ticket`) with no hold of the round left open where its `on` lines of the
    ending have no `hold`, every condition and the arguments of every line whose condition holds
    worked out, and as many posts and voids whose condition holds as there are open holds of the
    round of their book and transfer (DESIGN 17 item 3). -/
theorem end_ticket_ok_closes {cx : Cx} {s : Step} {status : Status} {ending : Ending}
    {did : String}
    {by_ proxy_of note : Option String} {on : Ending → Ref → Org → Except Refusal (List OnLine)}
    {lines : List OnLine}
    (hon : on ending (ticket_ref s.t) cx.org = .ok lines) (hhold : ∀ l ∈ lines, l.action ≠ .hold)
    (hread : ∀ l ∈ lines, LineWorkedOut l)
    (hcov : HoldsCovered s.t.holds s.t.round lines) :
    ∃ s', end_ticket cx s status ending did by_ proxy_of note on = .ok s' ∧ s'.t.status = status ∧
      ∀ hd ∈ s'.t.holds, hd ∈ s.t.holds ∧ hd.round ≠ s.t.round := by
  obtain ⟨s', hs'⟩ := end_ticket_ok (status := status) (did := did) (by_ := by_)
    (proxy_of := proxy_of) (note := note) hon hread
  exact ⟨s', hs', end_ticket_closes hon hhold hcov hs'⟩

/-- An approved ticket (`finish`) whose `on approved` lines have no `hold` and are there for the
    holds of the round (DESIGN 17 item 3, 7.6) is refused, or approved with no hold of the round
    left open. -/
theorem finish_closes {cx : Cx} {s s' : Step}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)} {lines : List OnLine}
    (hon : on .approved (ticket_ref s.t) cx.org = .ok lines) (hhold : ∀ l ∈ lines, l.action ≠ .hold)
    (hcov : HoldsCovered s.t.holds s.t.round lines) (h : finish cx s on = .ok s') :
    s'.t.status = .approved ∧ ∀ hd ∈ s'.t.holds, hd ∈ s.t.holds ∧ hd.round ≠ s.t.round := by
  unfold finish at h
  simp only [bind, Except.bind] at h
  split at h
  · cases h
  · next s3 h3 =>
    cases h
    obtain ⟨hc, he⟩ := on_ending_closes_of hon hhold hcov h3 ⟨rfl, rfl, rfl⟩
    exact ⟨by simp only [put]; rw [he]; try rfl, hc⟩

/-- A ticket is approved (`finish`) where its `on approved` lines have every condition, and the
    arguments of every line whose condition holds, worked out (DESIGN 7.4). -/
theorem finish_ok {cx : Cx} {s : Step} {on : Ending → Ref → Org → Except Refusal (List OnLine)}
    {lines : List OnLine}
    (hon : on .approved (ticket_ref s.t) cx.org = .ok lines)
    (hread : ∀ l ∈ lines, LineWorkedOut l) :
    ∃ s', finish cx s on = .ok s' := by
  unfold finish
  simp only [bind, Except.bind]
  split
  · next e he => exact (on_ending_ok_of hon hread he ⟨rfl, rfl, rfl⟩).elim
  · exact ⟨_, rfl⟩

/-- A ticket is not approved (`finish`) where a `hold` or a `launch` line of `on approved` has its
    condition, or its arguments where it holds, refused (DESIGN 7.2, 7.4). -/
theorem finish_refused {cx : Cx} {s : Step} {on : Ending → Ref → Org → Except Refusal (List OnLine)}
    {lines : List OnLine}
    (hon : on .approved (ticket_ref s.t) cx.org = .ok lines)
    (hbad : ∃ l ∈ lines, LineRefused l) :
    ∃ e, finish cx s on = .error e := by
  unfold finish
  simp only [bind, Except.bind]
  split
  · exact ⟨_, rfl⟩
  · next s3 h3 => exact (on_ending_refused_of hon hbad h3 ⟨rfl, rfl, rfl⟩).elim

/-- A ticket that goes to the requester (`to_requester`: sent back, or withdrawn to them) whose
    `on sent back` lines have no `hold` and are there for the holds of the round (DESIGN 17 item 3,
    7.6) is refused, or is with the requester with no hold of the round left open. -/
theorem to_requester_closes {cx : Cx} {s s' : Step} {did by_ : String}
    {sent_by proxy_of : Option String}
    {tell_to : List String} {open_after : String → Int → Int → Except Refusal Int}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)} {lines : List OnLine}
    (hon : on .sent_back (ticket_ref s.t) cx.org = .ok lines)
    (hhold : ∀ l ∈ lines, l.action ≠ .hold)
    (hcov : HoldsCovered s.t.holds s.t.round lines)
    (h : to_requester cx s did by_ sent_by proxy_of tell_to open_after on = .ok s') :
    s'.t.status = .with_requester ∧ ∀ hd ∈ s'.t.holds, hd ∈ s.t.holds ∧ hd.round ≠ s.t.round := by
  unfold to_requester at h
  simp only [bind, Except.bind] at h
  split at h
  · cases h
  · next u hu =>
    split at h
    · cases h
    · next s2 h2 =>
      split at h
      · cases h
      · next s3 h3 =>
        cases h
        obtain ⟨hc, he⟩ := on_ending_closes_of hon hhold hcov h2
          ((keeps_back hu).trans ⟨rfl, rfl, rfl⟩)
        obtain ⟨rd, hrd⟩ := due_back_t h3
        have ht : (tell s3 tell_to .returned none).t = s3.t := by unfold tell; split <;> rfl
        rw [ht, hrd]
        exact ⟨by simp only; rw [he]; try rfl, hc⟩

/-- A ticket does not go to the requester (`to_requester`) where a `hold` or a `launch` line of
    `on sent back` has its condition, or its arguments where it holds, refused (DESIGN 7.2, 7.4). -/
theorem to_requester_refused {cx : Cx} {s : Step} {did by_ : String}
    {sent_by proxy_of : Option String}
    {tell_to : List String} {open_after : String → Int → Int → Except Refusal Int}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)} {lines : List OnLine}
    (hon : on .sent_back (ticket_ref s.t) cx.org = .ok lines)
    (hbad : ∃ l ∈ lines, LineRefused l) :
    ∃ e, to_requester cx s did by_ sent_by proxy_of tell_to open_after on = .error e := by
  unfold to_requester
  simp only [bind, Except.bind]
  split
  · exact ⟨_, rfl⟩
  · next u hu =>
    split
    · exact ⟨_, rfl⟩
    · next s2 h2 =>
      exact (on_ending_refused_of hon hbad h2
        ((keeps_back hu).trans ⟨rfl, rfl, rfl⟩)).elim

end RingiCore