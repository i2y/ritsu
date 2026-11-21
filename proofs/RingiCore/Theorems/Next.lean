/-
  One event of a ticket (ringi's DESIGN 7.1–7.3, 7.6, 17 items 1 and 5). Before an event does its
  own work, `next` asks, in this order, whether the part of the organization is of the event's day
  (`data`), whether a clock has rung whose event the ticket has not taken (`clock`), whether the
  event was made on the ticket's revision (`stale`), and whether the ticket has ended (`state`), then
  reads what the person wrote with the event (`comment`): `next_day`, `next_rung_first`, `next_fresh`
  and `next_ended_refused` say what each refuses, `next_ok` what an event `next` takes passed, and
  `next_error` where a refusal comes from. From them:

  - an ended ticket takes no event (`next_ended`), and is refused as `state` once the day, the
    clocks and the revision pass (`next_ended_state`); a ticket `end_ticket` ends keeps no clock that
    could ring first (`end_ticket_clocks`, `rung_first_ended`);
  - a clock that has rung comes before every event that is not a clock's (`next_rung_first`);
  - a comment moves nothing (`next_comment_ticket`, `next_comment_same`, `next_comment_effects`);
  - an event made on another revision, or on none, is refused as `stale` (`next_stale`);
  - with self approval not allowed, the requester and the one who made the request for them answer
    no stage, in their own seat or a proxy's (`seats_self`, `answer_self`, `next_approve_self`,
    `next_reject_self`), are no approver who may pause the stage or edit the request there
    (`approver_self`), and are not made approvers (`do_add_approver_self`, `do_hand_over_self`); one
    on the requester's side who did not make the request, a proxy who may submit for the requester,
    is not left out (`may_answer_submit_proxy`).

  The theorems hold whatever the request's own code answers (`clocks`, `open_after`, `on`, `reform`)
  and whatever values the form gives (`vals`).
-/
import RingiCore.Glue

namespace RingiCore

variable {spec : Spec} {t : Ticket} {ev : Event} {org : Org} {now : Int} {vals : List Arg}
  {clocks : String → Int → Int → Int → Except Refusal Clocks}
  {open_after : String → Int → Int → Except Refusal Int}
  {on : Ending → Ref → Org → Form → Except Refusal (List OnLine)}
  {reform : Form → Form → List String → Bool → Org → Except Refusal Formed}

-- ── Binding a result ─────────────────────────────────────────────────────────

private theorem bind_ok {ε α β : Type} {x : Except ε α} {f : α → Except ε β} {b : β}
    (h : (x >>= f) = .ok b) : ∃ a, x = .ok a ∧ f a = .ok b := by
  cases x with
  | error e => cases h
  | ok a => exact ⟨a, rfl, h⟩

private theorem bind_error {ε α β : Type} {x : Except ε α} {f : α → Except ε β} {e : ε}
    (h : (x >>= f) = .error e) : x = .error e ∨ ∃ a, x = .ok a ∧ f a = .error e := by
  cases x with
  | error e' => cases h; exact Or.inl rfl
  | ok a => exact Or.inr ⟨a, rfl, h⟩

-- ── What each check refuses ──────────────────────────────────────────────────

/-- The part of the organization of another day is refused as `data` (DESIGN 7.5). -/
theorem on_day_error_kind {offset : Int} {e : Refusal} (h : on_day org now offset = .error e) :
    e.refused = .data := by
  unfold on_day at h
  split at h <;> cases h <;> rfl

/-- An event made on another revision of the ticket, or on none, is refused as `stale`
    (DESIGN 7.2). -/
theorem fresh_error_kind {e : Refusal} (h : fresh t ev = .error e) : e.refused = .stale := by
  unfold fresh at h
  split at h
  · split at h
    · cases h; rfl
    · dsimp only at h
      split at h
      · cases h
      · cases h; rfl
  · cases h

/-- An event a clock's or a comment, or one made on the ticket's revision, is fresh (DESIGN 7.2). -/
theorem fresh_ok (h : revised ev = false ∨ seen ev = some (revision_of t)) : fresh t ev = .ok true := by
  unfold fresh
  rcases h with h | h
  · simp only [h]
    rfl
  · split
    · rw [h]
      simp only [beq_self_eq_true, ite_true]
      rfl
    · rfl

/-- An event made on a revision of the ticket is refused as `stale` where it says none, or another
    than the ticket's as it is worked out (DESIGN 7.1, 7.2). -/
theorem fresh_stale (hrev : revised ev = true) (hseen : seen ev ≠ some (revision_of t)) :
    ∃ e, fresh t ev = .error e ∧ e.refused = .stale := by
  cases h : fresh t ev with
  | error e => exact ⟨e, rfl, fresh_error_kind h⟩
  | ok b =>
    exfalso
    unfold fresh at h
    split at h
    · split at h
      · cases h
      · rename_i r hr
        dsimp only at h
        split at h
        · rename_i heq
          exact hseen (by rw [hr, eq_of_beq heq])
        · cases h
    · rename_i hn
      exact hn hrev

private theorem said_error_kind {c : Option String} {e : Refusal} (h : said c = .error e) :
    e.refused = .comment := by
  unfold said at h
  split at h
  · cases h
  · split at h
    · cases h
    · split at h
      · cases h; rfl
      · cases h

private theorem reasoned_error_kind {c : Option String} {e : Refusal}
    (h : reasoned spec ev c = .error e) : e.refused = .comment := by
  unfold reasoned at h
  split at h
  · split at h
    · cases h; rfl
    · split at h
      · cases h; rfl
      · cases h
  · cases h

/-- An event taken in no state the ticket is in, or at no stage of it, is refused as `state`
    (DESIGN 7.2). -/
theorem stage_in_error_kind {name : String} {states : List Status} {r : Refusal}
    (h : stage_in t name states = .error r) : r.refused = .state := by
  unfold stage_in at h
  split at h
  · split at h
    · cases h; rfl
    · split at h
      · cases h; rfl
      · cases h
  · cases h; rfl

/-- An event made on a revision is not a clock's (DESIGN 7.2). -/
theorem not_clock_of_revised (h : revised ev = true) : is_clock ev = false := by
  cases ev <;> simp_all [revised, is_clock]

-- ── The head of `next` ───────────────────────────────────────────────────────

/-- `next` refuses an event the part of the organization of another day comes with, as `data`, first
    of all (DESIGN 7.3, 7.5). -/
theorem next_day {e : Refusal} (h : on_day org now spec.offset = .error e) :
    next spec t ev org now vals clocks open_after on reform = .error e := by
  unfold next
  rw [h]
  rfl

/-- On the event's day, with no clock to come first, `next` refuses an event `fresh` refuses, with
    its refusal (DESIGN 7.3). -/
theorem next_fresh {d : Int} {e : Refusal} (hday : on_day org now spec.offset = .ok d)
    (hclock : is_clock ev = true ∨ rung_first t now = none) (hf : fresh t ev = .error e) :
    next spec t ev org now vals clocks open_after on reform = .error e := by
  unfold next
  rw [hday]
  simp only [bind, Except.bind]
  have hr : (if is_clock ev = true then none else rung_first t now) = none := by
    rcases hclock with h | h <;> simp [h]
  rw [hr]
  dsimp only
  rw [hf]

/-- On the event's day, with no clock to come first and the event fresh, `next` refuses every event
    of an ended ticket as `state` (DESIGN 7.3). -/
theorem next_ended_refused {d : Int} {b : Bool} (hday : on_day org now spec.offset = .ok d)
    (hclock : is_clock ev = true ∨ rung_first t now = none) (hf : fresh t ev = .ok b)
    (h : ended t.status = true) :
    ∃ r, next spec t ev org now vals clocks open_after on reform = .error r ∧ r.refused = .state := by
  unfold next
  rw [hday]
  simp only [bind, Except.bind]
  have hr : (if is_clock ev = true then none else rung_first t now) = none := by
    rcases hclock with h | h <;> simp [h]
  rw [hr]
  dsimp only
  rw [hf]
  simp only [h, ite_true]
  exact ⟨_, rfl, rfl⟩

/-- An event `next` takes passed every check of its head: the day, no clock to come first, its
    revision, a ticket not ended, what the person wrote with it; and the move is the event's own
    work, with what they wrote on its line of the history (DESIGN 7.3). -/
theorem next_ok {m : Moved} (h : next spec t ev org now vals clocks open_after on reform = .ok m) :
    (∃ d, on_day org now spec.offset = .ok d) ∧ (is_clock ev = true ∨ rung_first t now = none) ∧
      fresh t ev = .ok true ∧ ended t.status = false ∧
      ∃ w, said (comment_of ev) = .ok w ∧ ∃ c, reasoned spec ev w = .ok c ∧
        ∃ s, act { spec := spec, org := org, now := now } t ev c vals clocks open_after on reform = .ok s ∧
          m = done (commented s c) := by
  unfold next at h
  obtain ⟨d, hd, h⟩ := bind_ok h
  dsimp only at h
  split at h
  · cases h
  · rename_i hr
    obtain ⟨b, hf, h⟩ := bind_ok h
    split at h
    · cases h
    · rename_i hne
      obtain ⟨w, hs, h⟩ := bind_ok h
      obtain ⟨c, hc, h⟩ := bind_ok h
      obtain ⟨s, ha, h⟩ := bind_ok h
      cases h
      have hb : b = true := by
        unfold fresh at hf
        split at hf
        · split at hf
          · cases hf
          · dsimp only at hf
            split at hf
            · cases hf; rfl
            · cases hf
        · cases hf; rfl
      subst hb
      refine ⟨⟨d, hd⟩, ?_, hf, by simpa using hne, w, hs, c, hc, s, ha, rfl⟩
      by_cases hic : is_clock ev = true
      · exact Or.inl hic
      · exact Or.inr (by simpa [hic] using hr)

/-- A refusal of `next` is one of its head (`data`, `clock`, `stale`, `state`, `comment`), or the
    event's own work refused it (DESIGN 7.2, 7.3). -/
theorem next_error {r : Refusal} (h : next spec t ev org now vals clocks open_after on reform = .error r) :
    r.refused = .data ∨ r.refused = .clock ∨ r.refused = .stale ∨ r.refused = .state ∨
      r.refused = .comment ∨
      ∃ c, act { spec := spec, org := org, now := now } t ev c vals clocks open_after on reform = .error r := by
  unfold next at h
  rcases bind_error h with hd | ⟨d, -, h⟩
  · exact Or.inl (on_day_error_kind hd)
  dsimp only at h
  split at h
  · cases h
    exact Or.inr (Or.inl rfl)
  rcases bind_error h with hf | ⟨b, -, h⟩
  · exact Or.inr (Or.inr (Or.inl (fresh_error_kind hf)))
  split at h
  · cases h
    exact Or.inr (Or.inr (Or.inr (Or.inl rfl)))
  rcases bind_error h with hs | ⟨w, -, h⟩
  · exact Or.inr (Or.inr (Or.inr (Or.inr (Or.inl (said_error_kind hs)))))
  rcases bind_error h with hc | ⟨c, -, h⟩
  · exact Or.inr (Or.inr (Or.inr (Or.inr (Or.inl (reasoned_error_kind hc)))))
  rcases bind_error h with ha | ⟨s, -, h⟩
  · exact Or.inr (Or.inr (Or.inr (Or.inr (Or.inr ⟨c, ha⟩))))
  · cases h

-- ── 1. An ended ticket takes no event ────────────────────────────────────────

/-- **An ended ticket takes no event** (DESIGN 17 item 1, 7.1, 7.2): whatever the event, the part of
    the organization, the time, the values and what the request's own code answers, `next` refuses
    it, as `data`, `clock`, `stale` or `state`. -/
theorem next_ended (h : ended t.status = true) :
    ∃ r, next spec t ev org now vals clocks open_after on reform = .error r ∧
      (r.refused = .data ∨ r.refused = .clock ∨ r.refused = .stale ∨ r.refused = .state) := by
  unfold next
  cases hd : on_day org now spec.offset with
  | error e => exact ⟨e, rfl, Or.inl (on_day_error_kind hd)⟩
  | ok d =>
    simp only [bind, Except.bind]
    split
    · exact ⟨_, rfl, Or.inr (Or.inl rfl)⟩
    · cases hf : fresh t ev with
      | error e => exact ⟨e, rfl, Or.inr (Or.inr (Or.inl (fresh_error_kind hf)))⟩
      | ok b =>
        simp only [h, ite_true]
        exact ⟨_, rfl, Or.inr (Or.inr (Or.inr rfl))⟩

/-- **An ended ticket is refused as `state`** (DESIGN 17 item 1, 7.3) once the day is the event's,
    no clock comes first and the event is fresh. -/
theorem next_ended_state {d : Int} (h : ended t.status = true) (hday : on_day org now spec.offset = .ok d)
    (hclock : is_clock ev = true ∨ rung_first t now = none)
    (hrev : revised ev = false ∨ seen ev = some (revision_of t)) :
    ∃ r, next spec t ev org now vals clocks open_after on reform = .error r ∧ r.refused = .state :=
  next_ended_refused hday hclock (fresh_ok hrev) h

private theorem own_clocks_not_in_progress (h : ended t.status = true) : own_clocks t = [] := by
  unfold own_clocks
  have : t.status ≠ .in_progress := by
    intro he
    rw [he] at h
    cases h
  simp [this]

/-- An ended ticket without a clock of a pause or of a resubmission set has no clock to ring
    (DESIGN 7.6): its stage's clocks are not read once it has ended. -/
theorem rung_first_ended (h : ended t.status = true) (hp : t.pause_ends = none)
    (hb : t.resubmit_due = none) : rung_first t now = none := by
  unfold rung_first
  simp [hp, hb, own_clocks_not_in_progress h, sortBy, first?]

private theorem on_ending_t {cx : Cx} {s s' : Step} {ending : Ending}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)} (h : on_ending cx s ending on = .ok s') :
    s'.t = { s.t with holds := s'.t.holds } := by
  unfold on_ending at h
  obtain ⟨lines, -, h⟩ := bind_ok h
  dsimp only at h
  split at h
  · cases h
  · cases h; rfl

private theorem cancel_pause_ends (s : Step) : (cancel_pause s).t.pause_ends = none := by
  unfold cancel_pause
  split
  · assumption
  · rfl

private theorem cancel_back_ends (s : Step) : (cancel_back s).t.pause_ends = s.t.pause_ends := by
  unfold cancel_back
  split <;> rfl

private theorem cancel_back_due (s : Step) : (cancel_back s).t.resubmit_due = none := by
  unfold cancel_back
  split
  · assumption
  · rfl

/-- A ticket `end_ticket` ends (DESIGN 7.3: rejected, withdrawn, expired) is in the state it is given,
    with neither the clock of a pause nor that of a resubmission set. -/
theorem end_ticket_clocks {cx : Cx} {s s' : Step} {status : Status} {ending : Ending} {did : String}
    {by_ proxy_of note : Option String} {on : Ending → Ref → Org → Except Refusal (List OnLine)}
    (h : end_ticket cx s status ending did by_ proxy_of note on = .ok s') :
    s'.t.status = status ∧ s'.t.pause_ends = none ∧ s'.t.resubmit_due = none := by
  unfold end_ticket at h
  obtain ⟨s3, h3, hs⟩ := bind_ok h
  cases hs
  have ht := on_ending_t h3
  refine ⟨?_, ?_, ?_⟩
  · show s3.t.status = status
    rw [ht]
  · show s3.t.pause_ends = none
    rw [ht]
    exact (cancel_back_ends _).trans (cancel_pause_ends _)
  · show s3.t.resubmit_due = none
    rw [ht]
    exact cancel_back_due _

-- ── 2. A clock that has rung comes first ─────────────────────────────────────

/-- **A clock that has rung comes first** (DESIGN 7.6): on the event's day, while a clock of the
    ticket has rung, `next` refuses every event that is not a clock's as `clock`, whatever the
    ticket's state and the event. -/
theorem next_rung_first {d : Int} {x : Rung} (hday : on_day org now spec.offset = .ok d)
    (hc : is_clock ev = false) (hr : rung_first t now = some x) :
    ∃ r, next spec t ev org now vals clocks open_after on reform = .error r ∧ r.refused = .clock := by
  unfold next
  rw [hday]
  simp only [bind, Except.bind, hc, hr]
  exact ⟨_, rfl, rfl⟩

-- ── 3. A comment moves nothing ───────────────────────────────────────────────

private theorem mem_indexed {α : Type} {l : List α} {x : Indexed α} (h : x ∈ indexed l) : x.item ∈ l := by
  unfold indexed at h
  rw [List.mem_map] at h
  obtain ⟨⟨a, i⟩, hm, rfl⟩ := h
  exact List.fst_mem_of_mem_zipIdx hm

private theorem done_effects_mem {s : Step} {e : Effect} (h : e ∈ (done s).effects) :
    ∃ x ∈ s.fx, e.what = x.what := by
  unfold done at h
  simp only [List.mem_map] at h
  obtain ⟨x, hx, rfl⟩ := h
  exact ⟨x.item, List.mem_mergeSort.mp (mem_indexed hx), rfl⟩

private theorem commented_t (s : Step) (c : Option String) : (commented s c).t = s.t := by
  unfold commented
  split
  · rfl
  · split <;> rfl

private theorem commented_fx_mem {s : Step} {c : Option String} {y : Made} (h : y ∈ (commented s c).fx) :
    ∃ z ∈ s.fx, y.what = z.what ∨ ∃ x, y.what = with_comment z.what x := by
  unfold commented at h
  split at h
  · exact ⟨y, h, Or.inl rfl⟩
  · rename_i x
    split at h
    · exact ⟨y, h, Or.inl rfl⟩
    · simp only [List.mem_map] at h
      obtain ⟨m, hm, rfl⟩ := h
      refine ⟨m.item, mem_indexed hm, ?_⟩
      split
      · exact Or.inr ⟨x, rfl⟩
      · exact Or.inl rfl

private theorem tell_t (s : Step) (to : List String) (n : Notice) (st : Option String) :
    (tell s to n st).t = s.t := by
  unfold tell put
  split <;> rfl

private theorem tell_fx_mem {s : Step} {to : List String} {n : Notice} {st : Option String} {y : Made}
    (h : y ∈ (tell s to n st).fx) : y ∈ s.fx ∨ y.what = .notify to n st := by
  unfold tell put at h
  split at h
  · exact Or.inl h
  · simp only [List.mem_append, List.mem_singleton] at h
    rcases h with h | rfl
    · exact Or.inl h
    · exact Or.inr rfl

/-- What a comment makes: its line of the history, or its notice. -/
private def CommentMade (w : What) : Prop :=
  (∃ r, w = .history r ∧ r.did = "commented") ∨ (∃ to st, w = .notify to .comment st)

private theorem CommentMade.with_comment {w : What} (h : CommentMade w) (x : String) :
    CommentMade (with_comment w x) := by
  rcases h with ⟨r, rfl, hr⟩ | ⟨to, st, rfl⟩
  · exact Or.inl ⟨_, rfl, hr⟩
  · exact Or.inr ⟨to, st, rfl⟩

private theorem do_comment_ok {cx : Cx} {s s' : Step} {by_ : String} {text : Option String}
    (h : do_comment cx s by_ text = .ok s') : s'.t = s.t ∧ ∀ y ∈ s'.fx, y ∈ s.fx ∨ CommentMade y.what := by
  unfold do_comment at h
  split at h
  · cases h
  · cases h
    refine ⟨by rw [tell_t]; rfl, fun y hy => ?_⟩
    rcases tell_fx_mem hy with hy | hy
    · simp only [note_down, put, List.mem_append, List.mem_singleton] at hy
      rcases hy with hy | rfl
      · exact Or.inl hy
      · exact Or.inr (Or.inl ⟨_, rfl, rfl⟩)
    · exact Or.inr (Or.inr ⟨_, _, hy⟩)

private theorem next_comment_ok {a : CommentEv} {m : Moved}
    (h : next spec t (.comment a) org now vals clocks open_after on reform = .ok m) :
    ∃ c s, (s.t = t ∧ ∀ y ∈ s.fx, CommentMade y.what) ∧ m = done (commented s c) := by
  obtain ⟨-, -, -, -, w, -, c, -, s, ha, rfl⟩ := next_ok h
  obtain ⟨hst, hfx⟩ := do_comment_ok ha
  refine ⟨c, s, ⟨hst, fun y hy => ?_⟩, rfl⟩
  rcases hfx y hy with hy | hy
  · cases hy
  · exact hy

/-- **A comment moves nothing** (DESIGN 7.2, 7.3): the ticket after a comment `next` takes is the
    ticket before it but for its serial, which moves past what is to do, and its revision, worked out
    of it as it is. -/
theorem next_comment_ticket {a : CommentEv} {m : Moved}
    (h : next spec t (.comment a) org now vals clocks open_after on reform = .ok m) :
    m.ticket = { t with serial := m.ticket.serial, revision := revision_of t } := by
  obtain ⟨c, s, ⟨hst, -⟩, rfl⟩ := next_comment_ok h
  simp only [done, commented_t, hst]

/-- **A comment moves nothing** (DESIGN 7.2): of a ticket that holds its revision as it is worked
    out, a comment changes the serial alone. -/
theorem next_comment_same {a : CommentEv} {m : Moved}
    (h : next spec t (.comment a) org now vals clocks open_after on reform = .ok m)
    (hrev : t.revision = revision_of t) : m.ticket = { t with serial := m.ticket.serial } := by
  rw [next_comment_ticket h, ← hrev]

/-- **A comment makes its line of the history and its notice alone** (DESIGN 7.2, 7.3, 7.4): what is
    to do after a comment is lines of the history of `commented` and notices of `comment`; no clock
    is set or cancelled, nothing is held, posted or launched. -/
theorem next_comment_effects {a : CommentEv} {m : Moved}
    (h : next spec t (.comment a) org now vals clocks open_after on reform = .ok m) :
    ∀ e ∈ m.effects, (∃ r, e.what = .history r ∧ r.did = "commented") ∨
      (∃ to st, e.what = .notify to .comment st) := by
  obtain ⟨c, s, ⟨-, hfx⟩, rfl⟩ := next_comment_ok h
  intro e he
  obtain ⟨x, hx, hw⟩ := done_effects_mem he
  obtain ⟨z, hz, hxz⟩ := commented_fx_mem hx
  show CommentMade e.what
  rw [hw]
  rcases hxz with hxz | ⟨y, hxz⟩
  · rw [hxz]; exact hfx z hz
  · rw [hxz]; exact (hfx z hz).with_comment y

-- ── 4. An event made on another revision ─────────────────────────────────────

/-- **An event made on another revision is refused, and moves nothing** (DESIGN 7.1, 7.2): on the
    event's day, with no clock rung, an event that says a revision and says none, or another than the
    ticket's as it is worked out, is refused as `stale`. -/
theorem next_stale {d : Int} (hday : on_day org now spec.offset = .ok d) (hr : rung_first t now = none)
    (hrev : revised ev = true) (hseen : seen ev ≠ some (revision_of t)) :
    ∃ r, next spec t ev org now vals clocks open_after on reform = .error r ∧ r.refused = .stale := by
  obtain ⟨e, he, hk⟩ := fresh_stale hrev hseen
  exact ⟨e, next_fresh hday (Or.inr hr) he, hk⟩

-- ── 5. Self approval, on the side of the machine ─────────────────────────────

private theorem left_out_self {spec : Spec} {who : Who} {x : String} (hs : spec.self_approval = false)
    (hby : who.requester = x ∨ who.made_by = some x) : left_out spec who x = true := by
  unfold left_out
  rcases hby with h | h <;> simp [hs, h]

private theorem may_answer_left_out {cx : Cx} {p : String} (h : left_out cx.spec (who_of t) p = true) :
    ∃ r, may_answer cx t p = .error r ∧ r.refused = .person := by
  unfold may_answer
  split
  · exact ⟨_, rfl, rfl⟩
  · exact ⟨_, rfl, rfl⟩

/-- **The requester answers in no seat** (DESIGN 17 item 5, 5.5, 5.8): with self approval not allowed,
    the requester and the one who made the request for them take no seat at a stage, neither their
    own nor that of an approver they are a proxy of; the answer is refused as `person`. -/
theorem seats_self {cx : Cx} {st : Stage} {by_ : String} (hs : cx.spec.self_approval = false)
    (hby : t.requester = by_ ∨ t.made_by = some by_) :
    ∃ r, seats cx t st by_ = .error r ∧ r.refused = .person := by
  obtain ⟨r, hr, hk⟩ := may_answer_left_out (left_out_self (who := who_of t) hs hby)
  refine ⟨r, ?_, hk⟩
  unfold seats
  rw [hr]
  rfl

/-- **The requester is no approver** (DESIGN 17 item 5, 5.5, 5.8): with self approval not allowed,
    the requester and the one who made the request for them are refused, as `person`, as an approver
    or a proxy of one, who may pause the stage (`pause`, `resume`) and edit the request there. -/
theorem approver_self {cx : Cx} {st : Stage} {by_ : String} (hs : cx.spec.self_approval = false)
    (hby : t.requester = by_ ∨ t.made_by = some by_) :
    ∃ r, approver cx t st by_ = .error r ∧ r.refused = .person := by
  obtain ⟨r, hr, hk⟩ := may_answer_left_out (left_out_self (who := who_of t) hs hby)
  refine ⟨r, ?_, hk⟩
  unfold approver
  rw [hr]
  rfl

/-- **The requester's answer passes no stage** (DESIGN 17 item 5, 5.5): with self approval not
    allowed, an approval or a rejection by the requester or by the one who made the request for them
    is refused, as `state` where the ticket takes no answer, else as `person`. -/
theorem answer_self {cx : Cx} {s : Step} {by_ : String} {yes : Bool}
    {on : Ending → Ref → Org → Except Refusal (List OnLine)} (hs : cx.spec.self_approval = false)
    (hby : s.t.requester = by_ ∨ s.t.made_by = some by_) :
    ∃ r, answer cx s by_ yes clocks open_after on = .error r ∧ (r.refused = .state ∨ r.refused = .person) := by
  unfold answer
  dsimp only
  cases hst : stage_in s.t (if yes = true then "approve" else "reject") [Status.in_progress] with
  | error e => exact ⟨e, rfl, Or.inl (stage_in_error_kind hst)⟩
  | ok st =>
    obtain ⟨r, hr, hk⟩ := seats_self (st := st) hs hby
    refine ⟨r, ?_, Or.inr hk⟩
    show (seats cx s.t st by_ >>= _) = _
    rw [hr]
    rfl

/-- **The requester is not made an approver** (DESIGN 17 item 5, 5.5, 5.9): with self approval not
    allowed, `add approver` of the requester or of the one who made the request for them is refused,
    as `state` where the ticket takes no such event, else as `person`. -/
theorem do_add_approver_self {cx : Cx} {s : Step} {by_ : Option String} {who : String}
    {which : Option String} (hs : cx.spec.self_approval = false)
    (hby : s.t.requester = who ∨ s.t.made_by = some who) :
    ∃ r, do_add_approver cx s by_ who which = .error r ∧ (r.refused = .state ∨ r.refused = .person) := by
  unfold do_add_approver
  cases hst : stage_in s.t "add approver" [Status.in_progress, Status.paused] with
  | error e => exact ⟨e, rfl, Or.inl (stage_in_error_kind hst)⟩
  | ok st =>
    obtain ⟨r, hr, hk⟩ := may_answer_left_out (left_out_self (who := who_of s.t) hs hby)
    refine ⟨r, ?_, Or.inr hk⟩
    show (may_answer cx s.t who >>= _) = _
    rw [hr]
    rfl

/-- **The requester is not handed a seat** (DESIGN 17 item 5, 5.5, 5.9): with self approval not
    allowed, `hand over` to the requester or to the one who made the request for them is refused, as
    `state` where the ticket takes no such event, else as `person`. -/
theorem do_hand_over_self {cx : Cx} {s : Step} {by_ : Option String} {of to : String}
    (hs : cx.spec.self_approval = false) (hby : s.t.requester = to ∨ s.t.made_by = some to) :
    ∃ r, do_hand_over cx s by_ of to = .error r ∧ (r.refused = .state ∨ r.refused = .person) := by
  unfold do_hand_over
  cases hst : stage_in s.t "hand over" [Status.in_progress, Status.paused] with
  | error e => exact ⟨e, rfl, Or.inl (stage_in_error_kind hst)⟩
  | ok st =>
    obtain ⟨r, hr, hk⟩ := may_answer_left_out (left_out_self (who := who_of s.t) hs hby)
    refine ⟨r, ?_, Or.inr hk⟩
    show (may_answer cx s.t to >>= _) = _
    rw [hr]
    rfl

private theorem next_answer_self {a : ByEv} {yes : Bool} {ev : Event}
    (hev : ∀ c, act { spec := spec, org := org, now := now } t ev c vals clocks open_after on reform =
      answer { spec := spec, org := org, now := now } { t := t, fx := [], vals := vals } a.by_ yes clocks
        open_after (fun e r o => on e r o t.form))
    (hs : spec.self_approval = false) (hby : t.requester = a.by_ ∨ t.made_by = some a.by_) :
    ∃ r, next spec t ev org now vals clocks open_after on reform = .error r ∧
      (r.refused = .data ∨ r.refused = .clock ∨ r.refused = .stale ∨ r.refused = .state ∨
        r.refused = .comment ∨ r.refused = .person) := by
  obtain ⟨r0, hr0, hk0⟩ := answer_self (cx := { spec := spec, org := org, now := now })
    (s := { t := t, fx := [], vals := vals }) (yes := yes) (clocks := clocks) (open_after := open_after)
    (on := fun e r o => on e r o t.form) hs hby
  cases hn : next spec t ev org now vals clocks open_after on reform with
  | ok m =>
    obtain ⟨-, -, -, -, w, -, c, -, s, ha, -⟩ := next_ok hn
    rw [hev, hr0] at ha
    cases ha
  | error r =>
    refine ⟨r, rfl, ?_⟩
    rcases next_error hn with h1 | h1 | h1 | h1 | h1 | ⟨c, ha⟩
    · exact Or.inl h1
    · exact Or.inr (Or.inl h1)
    · exact Or.inr (Or.inr (Or.inl h1))
    · exact Or.inr (Or.inr (Or.inr (Or.inl h1)))
    · exact Or.inr (Or.inr (Or.inr (Or.inr (Or.inl h1))))
    · rw [hev, hr0] at ha
      cases ha
      rcases hk0 with h1 | h1
      · exact Or.inr (Or.inr (Or.inr (Or.inl h1)))
      · exact Or.inr (Or.inr (Or.inr (Or.inr (Or.inr h1))))

/-- **The requester's approval passes no stage** (DESIGN 17 item 5, 5.5): with self approval not
    allowed, `next` refuses every `approve` by the requester or by the one who made the request for
    them, whatever the ticket, the part of the organization and the time. -/
theorem next_approve_self {a : ByEv} (hs : spec.self_approval = false)
    (hby : t.requester = a.by_ ∨ t.made_by = some a.by_) :
    ∃ r, next spec t (.approve a) org now vals clocks open_after on reform = .error r ∧
      (r.refused = .data ∨ r.refused = .clock ∨ r.refused = .stale ∨ r.refused = .state ∨
        r.refused = .comment ∨ r.refused = .person) :=
  next_answer_self (yes := true) (fun _ => rfl) hs hby

/-- **The requester's rejection is refused too** (DESIGN 17 item 5, 5.5): with self approval not
    allowed, `next` refuses every `reject` by the requester or by the one who made the request for
    them. -/
theorem next_reject_self {a : ByEv} (hs : spec.self_approval = false)
    (hby : t.requester = a.by_ ∨ t.made_by = some a.by_) :
    ∃ r, next spec t (.reject a) org now vals clocks open_after on reform = .error r ∧
      (r.refused = .data ∨ r.refused = .clock ∨ r.refused = .stale ∨ r.refused = .state ∨
        r.refused = .comment ∨ r.refused = .person) :=
  next_answer_self (yes := false) (fun _ => rfl) hs hby

/-- **A proxy who may submit, and did not make the request, is not left out** (DESIGN 5.5, 5.8): one
    on the requester's side (`theirs_to_do` takes them) who is neither the requester nor the one who
    made the request for them, a proxy who may submit for the requester, may answer a stage of it. -/
theorem may_answer_submit_proxy {cx : Cx} {p name : String} {b : Bool}
    (hside : theirs_to_do cx t p name = .ok b) (hr : p ≠ t.requester) (hm : t.made_by ≠ some p) :
    may_answer cx t p = .ok true := by
  have hin : in_org cx.org p = true := by
    unfold theirs_to_do at hside
    split at hside
    · assumption
    · cases hside
  have hl : left_out cx.spec (who_of t) p = false := by
    unfold left_out who_of
    have h1 : (t.requester == p) = false := by simpa using fun h => hr h.symm
    have h2 : (t.made_by == some p) = false := by simpa using hm
    simp [h1, h2]
  unfold may_answer
  simp only [hin, hl, ite_true]
  rfl

end RingiCore
