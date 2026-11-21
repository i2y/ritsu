/-
  The machine's code (`ticket.kn`), its lines 1990–3151: the `on` lines, a request is made, an event,
  each event and `act`, `next`. Each function as the machine's code writes it, by its name there, its
  arguments in the order it declares them.

  Where this differs from the machine's code:
  - `on` answers `Except Refusal (List OnLine)`, read with `←`: an answer the test has not given is
    refused, not taken for some other answer.
  - Lean wants a function written before it is called, so four come before the place the machine's
    code writes them: `event_word` and `no_reason` before `reasoned`, `not_approver` before `seats`,
    `counted_days` before `unpause`.
  - `closes`, `without`, `on_line` and `on_ending` are in `RingiCore.Stages`, before `finish`, which
    calls `on_ending`; they are not written again here.
  - The arguments, locals and fields `at`, `by`, `open`, `until` and `when` are `at_`, `by_`, `open_`,
    `until_` and `when_`.
  - Added: `Status.word` (the name of a state, as the machine's `text` writes it) and `maxOf` (the
    machine's `max_of`).
-/
import RingiCore.Stages

namespace RingiCore

/-- A state's name, as the machine's `text` writes it. -/
def Status.word : Status → String
  | .in_progress => "in_progress"
  | .paused => "paused"
  | .with_requester => "with_requester"
  | .approved => "approved"
  | .rejected => "rejected"
  | .withdrawn => "withdrawn"
  | .expired => "expired"

/-- The largest value `f` gives of the elements, none for no element (the machine's `max_of`). -/
def maxOf {α} (l : List α) (f : α → Int) : Option Int :=
  l.foldl (fun acc x =>
    match acc with
    | none => some (f x)
    | some b => if f x > b then some (f x) else some b) none

-- ── the `on` lines ─────────────────────────────────────────────────────────

-- `closes`, `without`, `on_line` and `on_ending`, the first four functions of this section, are in
-- `RingiCore.Stages`, before `finish`, which calls `on_ending`.

/-- The ticket ends: its clocks are cancelled, what its `on` lines say of the ending is done, and the requester is told. `proxy_of` is whom `by` ended it for -/
def end_ticket (cx : Cx) (s : Step) (status : Status) (ending : Ending) (did : String)
    (by_ : Option String) (proxy_of : Option String) (note : Option String)
    (on : Ending → Ref → Org → Except Refusal (List OnLine)) : Except Refusal Step := do
  let s1 := cancel_back (cancel_pause (cancel_clocks s))
  let stage := s1.t.stage
  let s2 := note_down s1 { rec_of did with stage := stage, by_ := by_, proxy_of := proxy_of, note := note }
  let t := { s2.t with status := status, stage := none, paused_at := none, sent_back_by := none, ended := some ({ at_ := cx.now, stage := stage, by_ := by_ } : Ended) }
  let s3 ← on_ending cx { s2 with t := t } ending on
  pure (put s3 (.notify (theirs (who_of s3.t)) .decided none))

-- ── a request is made ──────────────────────────────────────────────────────

/-- The requester is in the organization, and the one who makes the request in their name is a proxy of theirs that day: who made it for them, if anyone did -/
def may_submit (org : Org) (requester : String) (by_ : Option String) : Except Refusal (Option String) :=
  if in_org org requester then
    match by_ with
    | none => pure none
    | some b =>
      if b == requester then pure none
      else
        if in_org org b then
          if org.proxies.any (fun x => x.kind == .submit && x.person == requester && x.proxy == b) then pure (some b)
          else throw (refusal .person (who_en org b ++ " is no proxy of " ++ who_en org requester ++ " to submit the request on " ++ dateText org.day ++ " (no `submit` row of `proxies.csv`)") (who_ja org b ++ "は、" ++ dateText org.day ++ " に " ++ who_ja org requester ++ "の代わりにこの申請を出せません（`proxies.csv` に `submit` の行がありません）"))
        else throw (not_in b org)
  else throw (not_in requester org)

/-- What a person wrote with an event, as the machine keeps it: none for an empty one, and refused (`comment`) past 2,000 characters -/
def said (c : Option String) : Except Refusal (Option String) :=
  match c with
  | none => pure none
  | some x =>
    if chars x == 0 then pure none
    else
      if chars x > 2000 then throw (refusal .comment ("the comment is " ++ chars_long (chars x) ++ ", past the limit (2000)") ("コメントは" ++ textInt (chars x) ++ "文字で、上限（2000）を超えます"))
      else pure (some x)

/-- Whether the request asks a reason with the event (`comment required for`): a `reject` or a `send back` it names -/
def reason_asked (spec : Spec) (ev : Event) : Bool :=
  match ev with
  | .reject _ => spec.comment_required.any (fun x => x == "reject")
  | .send_back _ => spec.comment_required.any (fun x => x == "send_back")
  | _ => false

-- (`event_word` is written here, before `no_reason`, which calls it; the machine's code writes it
-- under "an event".)
/-- The event as a refusal names it: its name in the contract, its words apart (`send back`) -/
def event_word (ev : Event) : String :=
  match ev with
  | .resubmit _ => "resubmit"
  | .approve _ => "approve"
  | .reject _ => "reject"
  | .send_back _ => "send back"
  | .retract _ => "retract"
  | .pause _ => "pause"
  | .resume _ => "resume"
  | .withdraw _ => "withdraw"
  | .discard _ => "discard"
  | .edit _ => "edit"
  | .add_approver _ => "add approver"
  | .hand_over _ => "hand over"
  | .comment _ => "comment"
  | .remind => "remind"
  | .late => "late"
  | .resubmit_late => "resubmit late"
  | .pause_ends => "pause ends"
  | .agent_answered _ => "agent answered"
  | .note_ready _ => "note ready"
  | .flow_failed _ => "flow failed"

def no_reason (ev : Event) : Refusal :=
  refusal .comment ("a `" ++ event_word ev ++ "` of this request needs a reason: write it in the comment") ("この申請の `" ++ event_word ev ++ "` には理由が要ります。コメントに書いてください")

/-- What a person wrote with an event the request asks a reason with, as `said` keeps it: refused (`comment`) when it is none or white space alone, before who gives the event is asked -/
def reasoned (spec : Spec) (ev : Event) (c : Option String) : Except Refusal (Option String) :=
  if reason_asked spec ev then
    match c with
    | none => throw (no_reason ev)
    | some x =>
      if chars (trimSpace x) == 0 then throw (no_reason ev)
      else pure c
  else pure c

/-- A request is made: the ticket at its first stage, with what is to do; or why it is not made. What the requester wrote with it is on the line of the history it makes -/
def start (spec : Spec) (id : String) (requester : String) (made_by : Option String) (form : Form)
    (vals : List Arg) (stages : List Stage) (org : Org) (now : Int) (comment : Option String)
    (attempt : String) (clocks : String → Int → Int → Int → Except Refusal Clocks)
    (open_after : String → Int → Int → Except Refusal Int)
    (on : Ending → Ref → Org → Form → Except Refusal (List OnLine)) : Except Refusal Moved := do
  let cx : Cx := { spec := spec, org := org, now := now }
  let pause : Int :=
    match spec.pause with
    | none => 0
    | some d => d
  let t : Ticket := { id := id, request := spec.request, version := spec.version, requester := requester, made_by := made_by, sent_back_by := none, status := .in_progress, round := 1, attempt := attempt, serial := 0, revision := { round := 1, stage := none, edits := 0 }, form := form, stage := none, stages := stages, backs := 0, edits := 0, pause_left := pause, paused_at := none, pause_ends := none, resubmit_due := none, holds := [], notes := [], ended := none }
  let by_ : String :=
    match made_by with
    | none => requester
    | some b => b
  let proxy_of : Option String :=
    match made_by with
    | none => none
    | some _ => some requester
  let s0 := note_down ({ t := t, fx := [], vals := vals } : Step) { rec_of "submitted" with by_ := some by_, proxy_of := proxy_of, organization := some org.version }
  let s1 ← on_ending cx s0 .submitted (fun e r o => on e r o form)
  let s2 ← advance cx s1 0 false clocks open_after (fun e r o => on e r o form)
  pure (done (commented s2 comment))

-- ── an event ───────────────────────────────────────────────────────────────

def clock_list (st : Stage) : List Rung :=
  let r : List Rung :=
    match st.remind with
    | none => []
    | some x => [{ timer := "remind", stage := some st.name, at_ := x }]
  let d : List Rung :=
    match st.due with
    | none => []
    | some x => [{ timer := "late", stage := some st.name, at_ := x }]
  r ++ d

def own_clocks (t : Ticket) : List Rung :=
  if t.status == .in_progress then
    match at_of t with
    | none => []
    | some i =>
      match at? t.stages i with
      | none => []
      | some st => clock_list st
  else []

/-- The first clock of the ticket that is set and has rung -/
def rung_first (t : Ticket) (now : Int) : Option Rung :=
  let pause : List Rung :=
    match t.pause_ends with
    | none => []
    | some x => [{ timer := "pause ends", stage := t.stage, at_ := x }]
  let back : List Rung :=
    match t.resubmit_due with
    | none => []
    | some x => [{ timer := "resubmit late", stage := none, at_ := x }]
  first? (sortBy (pause ++ back ++ own_clocks t) (fun r => r.at_)) (fun r => r.at_ <= now)

/-- A clock that is set and has rung; else refused, with the time it is set for and the time it is, in the offset the clock is read in -/
def rung (spec : Spec) (at_ : Option Int) (what : String) (stage : Option String) (now : Int) : Except Refusal Bool :=
  match at_ with
  | none => throw (refusal .clock ("no clock is set for `" ++ what ++ "`") ("`" ++ what ++ "` の時計は置かれていません"))
  | some x =>
    if now < x then
      let offset := clock_offset spec what stage
      let a := time_text x offset
      let n := time_text now offset
      throw (refusal .clock ("the clock of `" ++ what ++ "` is set for " ++ a ++ ", and it is " ++ n) ("`" ++ what ++ "` の時計は " ++ a ++ " に鳴ります。今は " ++ n ++ " です"))
    else pure true

def ended (s : Status) : Bool :=
  match s with
  | .approved | .rejected | .withdrawn | .expired => true
  | _ => false

def is_clock (ev : Event) : Bool :=
  match ev with
  | .remind | .late | .resubmit_late | .pause_ends => true
  | _ => false

/-- Whether the event is made on a revision of the ticket: every event but a clock's and a comment, which moves nothing -/
def revised (ev : Event) : Bool :=
  match ev with
  | .remind | .late | .resubmit_late | .pause_ends | .comment _ => false
  | _ => true

/-- What the person wrote with the event: none for a clock's and a flow's -/
def comment_of (ev : Event) : Option String :=
  match ev with
  | .resubmit x => x.comment
  | .approve x => x.comment
  | .reject x => x.comment
  | .send_back x => x.comment
  | .retract x => x.comment
  | .pause x => x.comment
  | .resume x => x.comment
  | .withdraw x => x.comment
  | .discard x => x.comment
  | .edit x => x.comment
  | .add_approver x => x.comment
  | .hand_over x => x.comment
  | .comment x => x.comment
  | .remind | .late | .resubmit_late | .pause_ends | .agent_answered _ | .note_ready _ | .flow_failed _ => none

/-- The revision of the ticket the event says it was made on: none for a clock's event and a comment, which say none -/
def seen (ev : Event) : Option Revision :=
  match ev with
  | .resubmit x => x.revision
  | .approve x => x.revision
  | .reject x => x.revision
  | .send_back x => x.revision
  | .retract x => x.revision
  | .pause x => x.revision
  | .resume x => x.revision
  | .withdraw x => x.revision
  | .discard x => x.revision
  | .edit x => x.revision
  | .add_approver x => x.revision
  | .hand_over x => x.revision
  | .agent_answered x => x.revision
  | .note_ready x => x.revision
  | .flow_failed revision => revision
  | .remind | .late | .resubmit_late | .pause_ends | .comment _ => none

/-- The revision as a refusal says it: round 1, stage 2 of the ticket, 1 edit -/
def revision_text (r : Revision) : String :=
  let stage :=
    match r.stage with
    | none => "at no stage"
    | some i => "stage " ++ textInt (i + 1) ++ " of the ticket"
  let edits := if r.edits == 1 then "1 edit" else textInt r.edits ++ " edits"
  "round " ++ textInt r.round ++ ", " ++ stage ++ ", " ++ edits

/-- The revision as a refusal says it in Japanese: 1回目、2番目のステップ、直し1回 -/
def revision_text_ja (r : Revision) : String :=
  let stage :=
    match r.stage with
    | none => "ステップの外"
    | some i => textInt (i + 1) ++ "番目のステップ"
  textInt r.round ++ "回目、" ++ stage ++ "、直し" ++ textInt r.edits ++ "回"

/-- Whether an event a person or a flow gives was made on the ticket's revision: else it is refused, and the one who made it is to see the ticket again. A clock's event and a comment say none, and are not asked -/
def fresh (t : Ticket) (ev : Event) : Except Refusal Bool :=
  let now := revision_of t
  if revised ev then
    match seen ev with
    | none => throw (refusal .stale ("`" ++ event_word ev ++ "` does not say which revision of the ticket it was made on") ("`" ++ event_word ev ++ "` が、どの版のチケットに対するものかを言っていません"))
    | some r =>
      if r == now then pure true
      else throw (refusal .stale ("`" ++ event_word ev ++ "` was made on the ticket at " ++ revision_text r ++ ", and it is at " ++ revision_text now ++ " now: the ticket is to be shown again") ("`" ++ event_word ev ++ "` はチケットの" ++ revision_text_ja r ++ "の時のものですが、チケットは今" ++ revision_text_ja now ++ "です。チケットを見せ直してください"))
  else pure true

/-- The person is in the organization and did not make the request (where self approval is not allowed): one who may answer a stage of it -/
def may_answer (cx : Cx) (t : Ticket) (by_ : String) : Except Refusal Bool :=
  if in_org cx.org by_ then
    if left_out cx.spec (who_of t) by_ then throw (refusal .person (who_en cx.org by_ ++ " made the request, and does not answer it (`self approval not allowed`)") (who_ja cx.org by_ ++ "は、この申請を出した人なので、答えられません（`self approval not allowed`）"))
    else pure true
  else throw (not_in by_ cx.org)

def free (p : Part) (by_ : String) (seat : String) : Bool :=
  p.state == .active && !(p.answers.any (fun a => a.by_ == by_ || a.seat == seat)) && p.approvers.contains seat

def proxy_seat (org : Org) (p : Indexed Part) (by_ : String) : Option Seat :=
  match first? p.item.approvers (fun a => free p.item by_ a && is_proxy org by_ a) with
  | none => none
  | some a => some { part := p.index, seat := a }

-- (`not_approver` is written here, before `seats` and `approver`, which call it.)
def not_approver (cx : Cx) (st : Stage) (by_ : String) : Refusal :=
  refusal .person (who_en cx.org by_ ++ " is not an approver of the stage `" ++ stage_written cx.spec st.name ++ "`, nor a proxy of one") (who_ja cx.org by_ ++ "は、ステップ `" ++ stage_written cx.spec st.name ++ "` の承認者でも、その代理でもありません")

/-- The seats an answer takes at the stage: each part the person is an approver of, in their own right; else the first approver they are a proxy of who has yet to answer -/
def seats (cx : Cx) (t : Ticket) (st : Stage) (by_ : String) : Except Refusal (List Seat) := do
  let _may ← may_answer cx t by_
  let own := ((indexed st.parts).filter (fun p => free p.item by_ by_)).map (fun p => ({ part := p.index, seat := by_ } : Seat))
  if (own.length : Int) == 0 then
    match ((indexed st.parts).filterMap (fun p => proxy_seat cx.org p by_)).head? with
    | some x => pure [x]
    | none =>
      if st.parts.any (fun p => p.answers.any (fun a => a.by_ == by_ || a.seat == by_)) then
        throw (refusal .person (who_en cx.org by_ ++ " has answered the stage `" ++ stage_written cx.spec st.name ++ "` already") (who_ja cx.org by_ ++ "は、ステップ `" ++ stage_written cx.spec st.name ++ "` にもう答えています"))
      else throw (not_approver cx st by_)
  else pure own

/-- An approver of the stage, or a proxy of one: who may pause it and edit the request there. Not one whose part waits for a later stage -/
def approver (cx : Cx) (t : Ticket) (st : Stage) (by_ : String) : Except Refusal Bool := do
  let _may ← may_answer cx t by_
  if (st.parts.filter (fun p => p.state != .deferred)).any (fun p => p.approvers.any (fun a => a == by_ || is_proxy cx.org by_ a)) then pure true
  else throw (not_approver cx st by_)

/-- The requester, the one who made the request for them, or a proxy who may submit for them that day -/
def theirs_to_do (cx : Cx) (t : Ticket) (by_ : String) (ev : String) : Except Refusal Bool :=
  if in_org cx.org by_ then
    if by_ == t.requester || t.made_by == some by_ || cx.org.proxies.any (fun x => x.kind == .submit && x.person == t.requester && x.proxy == by_) then pure true
    else throw (refusal .person ("`" ++ ev ++ "` is the requester's to do, and " ++ who_en cx.org by_ ++ " is neither the requester nor a proxy of theirs") ("`" ++ ev ++ "` は申請者がします。" ++ who_ja cx.org by_ ++ "は、この申請の申請者でも、その代理でもありません"))
  else throw (not_in by_ cx.org)

def answered_seat (x : Indexed Part) (seats : List Seat) (by_ : String) (yes : Bool) (now : Int) : Part :=
  let mine := (seats.filter (fun s => s.part == x.index)).map (fun s => ({ by_ := by_, seat := s.seat, yes := yes, at_ := now } : Answer))
  let p := { x.item with answers := x.item.answers ++ mine }
  if yes && p.state == .active && countIf p.answers (fun a => a.yes) >= p.needed then { p with state := .passed }
  else p

def proxy_of_seats (seats : List Seat) (by_ : String) : Option String :=
  match seats.head? with
  | none => none
  | some x => if x.seat == by_ then none else some x.seat

/-- Whom one on the requester's side acts for: the requester, when the one who made the request for them or a proxy of theirs does it -/
def for_requester (t : Ticket) (by_ : String) : Option String :=
  if by_ == t.requester then none else some t.requester

/-- Whom one acts for at the stage as an approver does: nobody where they are an approver of it, else the first approver they are a proxy of that day -/
def acting_for (org : Org) (st : Stage) (by_ : String) : Option String :=
  let parts := st.parts.filter (fun p => p.state != .deferred)
  if parts.any (fun p => p.approvers.contains by_) then none
  else first? (parts.flatMap (fun p => p.approvers)) (fun a => is_proxy org by_ a)

def state_refusal (t : Status) (ev : String) : Refusal :=
  refusal .state ("the ticket is `" ++ Status.word t ++ "`, and takes no `" ++ ev ++ "`") ("チケットは `" ++ Status.word t ++ "` です。`" ++ ev ++ "` は受け付けません")

/-- The stage the ticket is at, when its state is one the event is taken in -/
def stage_in (t : Ticket) (ev : String) (states : List Status) : Except Refusal Stage :=
  if states.contains t.status then
    match at_of t with
    | none => throw (state_refusal t.status ev)
    | some i =>
      match at? t.stages i with
      | none => throw (state_refusal t.status ev)
      | some st => pure st
  else throw (state_refusal t.status ev)

/-- The place of the stage the ticket is at -/
def index_in (t : Ticket) : Int :=
  match at_of t with
  | none => 0
  | some i => max (min i 1000000) 0

/-- The approver of the next stage, whom this stage's approver chooses, is chosen -/
def chosen_next (cx : Cx) (s : Step) (i : Int) : Except Refusal Bool := do
  match at? s.t.stages (i + 1) with
  | none => pure true
  | some nx =>
    let o ← occ_of cx.spec nx
    match first? (o.ways.filterMap (fun w => chosen_by_previous cx.spec w)) (fun c => !(present (pick s.vals c.field))) with
    | none => pure true
    | some c => throw (refusal_at .form ("`" ++ c.shown ++ "` is empty, and the next stage `" ++ stage_written cx.spec nx.name ++ "` is answered by the person chosen in it: fill it in with `edit` first") ("`" ++ c.shown ++ "` が空です。次のステップ `" ++ stage_written cx.spec nx.name ++ "` は、そこに指名した人が答えます。先に `edit` で入れてください") c.field)

/-- The stage has every approval it needs: the ticket moves on. The parts that waited for it are done with the approval given here; a part of its own that waits for a later stage leaves it waiting -/
def passed_on (cx : Cx) (s : Step) (i : Int)
    (clocks : String → Int → Int → Int → Except Refusal Clocks)
    (open_after : String → Int → Int → Except Refusal Int)
    (on : Ending → Ref → Org → Except Refusal (List OnLine)) : Except Refusal Step :=
  let s1 := cancel_clocks s
  let s2 := if cx.spec.once == .last then answered s1 i else s1
  match at? s2.t.stages i with
  | none => pure s2
  | some st =>
    let phase := if st.parts.any (fun p => p.state == .deferred) then Phase.deferred else Phase.answered
    advance cx (set_phase s2 i phase) (min (i + 1) 1000000) false clocks open_after on

def answer (cx : Cx) (s : Step) (by_ : String) (yes : Bool)
    (clocks : String → Int → Int → Int → Except Refusal Clocks)
    (open_after : String → Int → Int → Except Refusal Int)
    (on : Ending → Ref → Org → Except Refusal (List OnLine)) : Except Refusal Step := do
  let ev := if yes then "approve" else "reject"
  let st ← stage_in s.t ev [Status.in_progress]
  let i := index_in s.t
  let sts ← seats cx s.t st by_
  let parts := (indexed st.parts).map (fun x => answered_seat x sts by_ yes cx.now)
  let did := if yes then "approved" else "rejected"
  let s1 := note_down { s with t := with_stage s.t i { st with parts := parts } } { rec_of did with stage := some st.name, by_ := some by_, proxy_of := proxy_of_seats sts by_, organization := st.org }
  if yes then
    if parts.all (fun p => part_done p) then
      let _chosen ← chosen_next cx s1 i
      passed_on cx s1 i clocks open_after on
    else pure s1
  else
    if parts.any (fun p => p.state == .active && max ((p.approvers.length : Int) - countIf p.answers (fun a => !a.yes)) 0 < p.needed) then
      end_ticket cx s1 .rejected .rejected "ticket rejected" (some by_) (proxy_of_seats sts by_) none on
    else pure s1

-- (`counted_days` is written here, before `unpause`, which calls it.)
def counted_days (cx : Cx) (at_ : Int) (whole : Bool) (left : Int)
    (open_after : String → Int → Int → Except Refusal Int) : Except Refusal Int :=
  if whole then pure left
  else open_between (day_where cx.spec at_ "pause ends") (day_where cx.spec cx.now "pause ends") left open_after

/-- The business days of the pause the ticket is in are taken from what it has left: all of them when the pause ran out -/
def unpause (cx : Cx) (s : Step) (whole : Bool)
    (open_after : String → Int → Int → Except Refusal Int) : Except Refusal Unpaused := do
  match s.t.paused_at with
  | none => pure { s := s, used := 0 }
  | some at_ =>
    let counted ← counted_days cx at_ whole s.t.pause_left open_after
    let used := max (min counted s.t.pause_left) 0
    pure { s := { s with t := { s.t with paused_at := none, pause_left := max (s.t.pause_left - used) 0 } }, used := used }

/-- The requester's turn has a clock: the end of the day the request is to be made again by -/
def due_back (cx : Cx) (s : Step)
    (open_after : String → Int → Int → Except Refusal Int) : Except Refusal Step := do
  match cx.spec.resubmit with
  | none => pure s
  | some n =>
    let day ← open_after "resubmit" (day_where cx.spec cx.now "resubmit late") n
    let due ← end_of_day day (clock_offset cx.spec "resubmit late" none)
    pure (put { s with t := { s.t with resubmit_due := some due } } (.set_timer .resubmit_late none due))

/-- The ticket goes to the requester: sent back by an approver or a proxy of the approver `proxy_of` (`sent_by`, kept while it is with the requester), or withdrawn by the requester's side (none) -/
def to_requester (cx : Cx) (s : Step) (did : String) (by_ : String) (sent_by : Option String)
    (proxy_of : Option String) (tell_to : List String)
    (open_after : String → Int → Int → Except Refusal Int)
    (on : Ending → Ref → Org → Except Refusal (List OnLine)) : Except Refusal Step := do
  let u ← unpause cx (cancel_pause (cancel_clocks s)) false open_after
  let s1 := note_down u.s { rec_of did with stage := u.s.t.stage, by_ := some by_, proxy_of := proxy_of }
  let s2 ← on_ending cx { s1 with t := { s1.t with backs := min (s1.t.backs + 1) 1000000, status := .with_requester, stage := none, sent_back_by := sent_by } } .sent_back on
  let s3 ← due_back cx s2 open_after
  pure (tell s3 tell_to .returned none)

def times_of (b : Option Back) : Int :=
  match b with
  | none => 0
  | some x => x.times

def answered_once (p : Phase) : Bool :=
  match p with
  | .answered | .once | .deferred => true
  | _ => false

def do_send_back (cx : Cx) (s : Step) (by_ : String) (to : Option String)
    (clocks : String → Int → Int → Int → Except Refusal Clocks)
    (open_after : String → Int → Int → Except Refusal Int)
    (on : Ending → Ref → Org → Except Refusal (List OnLine)) : Except Refusal Step := do
  let st ← stage_in s.t "send back" [Status.in_progress]
  let i := index_in s.t
  let sts ← seats cx s.t st by_
  match cx.spec.back with
  | none => throw (refusal .event "the request has no `send back` line" "この申請の種類には `send back` の行がありません")
  | some b =>
    if s.t.backs >= b.times then
      throw (refusal .limit ("the ticket has gone back " ++ textInt b.times ++ " times already (`send back … at most " ++ textInt b.times ++ " times`): approve it or reject it") ("差し戻しと取り下げは合わせて" ++ textInt b.times ++ "回までです（`send back … at most " ++ textInt b.times ++ " times`）。承認か却下を選んでください"))
    else
      match to with
      | none => to_requester cx s "sent back" by_ (some by_) (proxy_of_seats sts by_) (theirs (who_of s.t)) open_after on
      | some k =>
        if b.to_requester then
          throw (refusal .event "the request goes back to the requester alone (`send back to the requester`)" "この申請の種類は、申請者へだけ戻せます（`send back to the requester`）")
        else
          match first? (indexed (takeI s.t.stages i)) (fun x => x.item.name == k) with
          | none => throw (refusal .event ("`" ++ stage_written cx.spec k ++ "` is no stage of the ticket before `" ++ stage_written cx.spec st.name ++ "`") ("`" ++ stage_written cx.spec k ++ "` は、このチケットの `" ++ stage_written cx.spec st.name ++ "` より前のステップではありません"))
          | some j =>
            if answered_once j.item.phase then
              let s1 := cancel_clocks s
              let s2 := note_down { s1 with t := { s1.t with backs := min (s1.t.backs + 1) 1000000 } } { rec_of "sent back" with stage := some st.name, by_ := some by_, proxy_of := proxy_of_seats sts by_, organization := st.org, to := [k] }
              let back := max (min j.index 1000000) 0
              advance cx { s2 with t := unwind cx s2.t back none } back true clocks open_after on
            else throw (refusal .event ("nobody answered the stage `" ++ stage_written cx.spec k ++ "` on this ticket") ("ステップ `" ++ stage_written cx.spec k ++ "` には、このチケットでだれも答えていません"))

def position (order : List String) (n : String) : Option Int :=
  match first? (indexed order) (fun x => x.item == n) with
  | none => none
  | some x => some x.index

/-- The stage `n` comes after the stage `un` in the route -/
def past (order : List String) (n : String) (un : String) : Bool :=
  match position order n with
  | none => false
  | some a =>
    match position order un with
    | none => false
    | some b => a > b

/-- The ticket has come past the stage a request is withdrawn until: the stage it is at is after it in the route, or one it came to before is, and no part waits for it -/
def until_past (cx : Cx) (t : Ticket) (st : Stage) (i : Int) (until_ : Option String) : Bool :=
  match until_ with
  | none => false
  | some un =>
    let waits := st.name == un || (waiting t.stages i).any (fun p => name_at t.stages p.stage == un)
    let before := (takeI t.stages i).any (fun x => x.name == un || past cx.spec.order x.name un)
    (past cx.spec.order st.name un || before) && !waits

def do_withdraw (cx : Cx) (s : Step) (by_ : String)
    (open_after : String → Int → Int → Except Refusal Int)
    (on : Ending → Ref → Org → Except Refusal (List OnLine)) : Except Refusal Step := do
  let st ← stage_in s.t "withdraw" [Status.in_progress, Status.paused]
  let i := index_in s.t
  let _mine ← theirs_to_do cx s.t by_ "withdraw"
  match cx.spec.withdraw with
  | none => throw (refusal .event "the request has no `withdraw` line" "この申請の種類には `withdraw` の行がありません")
  | some wl =>
    if until_past cx s.t st i wl.until_ then
      let un : String :=
        match wl.until_ with
        | none => ""
        | some u => stage_written cx.spec u
      let cur := stage_written cx.spec st.name
      throw (refusal .state ("the request is withdrawn until `" ++ un ++ "` answers (`withdraw until " ++ un ++ "`), and the ticket is at `" ++ cur ++ "`") ("取り下げられるのは `" ++ un ++ "` が答えるまでです（`withdraw until " ++ un ++ "`）。チケットは `" ++ cur ++ "` にあります"))
    else
      if s.t.backs >= times_of cx.spec.back then
        let u ← unpause cx s false open_after
        end_ticket cx u.s .withdrawn .withdrawn "ticket withdrawn" (some by_) (for_requester s.t by_) none on
      else to_requester cx s "withdrawn" by_ none (for_requester s.t by_) (waited st cx.org) open_after on

def do_discard (cx : Cx) (s : Step) (by_ : String)
    (on : Ending → Ref → Org → Except Refusal (List OnLine)) : Except Refusal Step := do
  if s.t.status == .with_requester then
    let _mine ← theirs_to_do cx s.t by_ "discard"
    end_ticket cx s .withdrawn .withdrawn "ticket withdrawn" (some by_) (for_requester s.t by_) none on
  else throw (state_refusal s.t.status "discard")

/-- The names an event lists, none for a list it leaves out -/
def listed (names : Option (List String)) : List String :=
  match names with
  | none => []
  | some ns => ns

def do_resubmit (cx : Cx) (s : Step) (by_ : String) (form : Form) (clear : Option (List String))
    (attempt : String) (clocks : String → Int → Int → Int → Except Refusal Clocks)
    (open_after : String → Int → Int → Except Refusal Int)
    (on : Ending → Ref → Org → Form → Except Refusal (List OnLine))
    (reform : Form → Form → List String → Bool → Org → Except Refusal Formed) : Except Refusal Step := do
  if s.t.status == .with_requester then
    let _mine ← theirs_to_do cx s.t by_ "resubmit"
    let _u ← usable_on cx.spec cx.org.day
    let f ← reform s.t.form form (listed clear) false cx.org
    let s1 := cancel_back s
    let s2 := note_down { s1 with t := { s1.t with round := min (s1.t.round + 1) 1000000, attempt := attempt, form := f.form, stages := f.stages, sent_back_by := none }, vals := f.vals } { rec_of "resubmitted" with by_ := some by_, proxy_of := for_requester s1.t by_, organization := some cx.org.version, cleared := f.cleared }
    let s3 ← on_ending cx s2 .submitted (fun e r o => on e r o f.form)
    advance cx s3 0 false clocks open_after (fun e r o => on e r o f.form)
  else throw (state_refusal s.t.status "resubmit")

def after_cur (order : List String) (n : String) (cur : Option Int) : Bool :=
  match position order n with
  | none => false
  | some p =>
    match cur with
    | none => true
    | some c => p > c

def agent_of (p : Part) : Option String :=
  if p.state == .agent then p.of else none

def ask_again (cx : Cx) (who : Who) (vals : List Arg) (stage : String) (flow : String) : List What :=
  [.ask_agent flow stage (flow_args who vals), hist { rec_of "agent asked" with stage := some stage, organization := some cx.org.version, note := some (flow_shown cx.spec flow) }]

def do_edit (cx : Cx) (s : Step) (by_ : String) (fields : Form) (clear : Option (List String))
    (reform : Form → Form → List String → Bool → Org → Except Refusal Formed) : Except Refusal Step := do
  let st ← stage_in s.t "edit" [Status.in_progress]
  let i := index_in s.t
  let _who ← approver cx s.t st by_
  let f ← reform s.t.form fields (listed clear) true cx.org
  let cur := maxOf ((takeI s.t.stages (i + 1)).filterMap (fun x => position cx.spec.order x.name)) (fun p => p)
  let stages := takeI s.t.stages (i + 1) ++ f.stages.filter (fun x => after_cur cx.spec.order x.name cur)
  let t := { s.t with form := f.form, stages := stages, edits := min (s.t.edits + 1) 1000000 }
  let s1 := note_down { s with t := t, vals := f.vals } { rec_of "edited" with stage := some st.name, by_ := some by_, proxy_of := acting_for cx.org st by_, to := f.named, cleared := f.cleared }
  pure (put_all s1 ((st.parts.filterMap (fun p => agent_of p)).flatMap (fun flow => ask_again cx (who_of s1.t) s1.vals st.name flow)))

def waits_answer (p : Part) : Bool :=
  p.state == .active || p.state == .stopped

def do_add_approver (cx : Cx) (s : Step) (by_ : Option String) (approver : String)
    (which : Option String) : Except Refusal Step := do
  let st ← stage_in s.t "add approver" [Status.in_progress, Status.paused]
  let i := index_in s.t
  let _may ← may_answer cx s.t approver
  let found : Option (Indexed Part) :=
    match which with
    | some w => first? (indexed st.parts) (fun x => x.item.way == w && waits_answer x.item)
    | none =>
      match first? (indexed st.parts) (fun x => x.item.state == .stopped) with
      | some x => some x
      | none => first? (indexed st.parts) (fun x => waits_answer x.item)
  match found with
  | none => throw (refusal .event ("the stage `" ++ stage_written cx.spec st.name ++ "` has no part that waits for an approver") ("ステップ `" ++ stage_written cx.spec st.name ++ "` に、承認者を足せる所がありません"))
  | some x =>
    if x.item.approvers.contains approver then
      throw (refusal .event (approver ++ " is an approver of the stage `" ++ stage_written cx.spec st.name ++ "` already") (approver ++ " は、もうステップ `" ++ stage_written cx.spec st.name ++ "` の承認者です"))
    else
      let p := x.item
      let approvers := p.approvers ++ [approver]
      let n := max p.needed 1
      let state := if p.state == .stopped && (approvers.length : Int) >= n then PartState.active else p.state
      let sole := if (approvers.length : Int) == 1 && n == 1 then Sole.by_ approver else Sole.other
      let st2 := { st with parts := replace_part st.parts (max (min x.index 1000000) 0) { p with approvers := approvers, state := state, needed := n, sole := sole } }
      let s1 := note_down { s with t := with_stage s.t i st2 } { rec_of "approver added" with stage := some st.name, by_ := by_, to := [approver] }
      if s.t.status == .paused then pure s1
      else pure (put s1 (.notify [approver] .waiting (some st.name)))

def hands (p : Part) (of : String) (to : String) : Bool :=
  p.state == .active && p.approvers.contains of && !(p.answers.any (fun a => a.seat == of)) && !(p.approvers.contains to)

def swapped (s : Sole) (of : String) (to : String) : Sole :=
  match s with
  | .by_ person => if person == of then .by_ to else s
  | _ => s

def handed (p : Part) (of : String) (to : String) : Part :=
  if hands p of to then { p with approvers := p.approvers.map (fun a => if a == of then to else a), sole := swapped p.sole of to }
  else p

def do_hand_over (cx : Cx) (s : Step) (by_ : Option String) (of : String) (to : String) : Except Refusal Step := do
  let st ← stage_in s.t "hand over" [Status.in_progress, Status.paused]
  let i := index_in s.t
  let _may ← may_answer cx s.t to
  if st.parts.any (fun p => hands p of to) then
    let st2 := { st with parts := st.parts.map (fun p => handed p of to) }
    let s1 := note_down { s with t := with_stage s.t i st2 } { rec_of "handed over" with stage := some st.name, by_ := by_, to := [of, to] }
    if s.t.status == .paused then pure s1
    else pure (put s1 (.notify [to] .waiting (some st.name)))
  else throw (refusal .event (of ++ " is no approver of the stage `" ++ stage_written cx.spec st.name ++ "` who has yet to answer (or " ++ to ++ " is one already)") (of ++ " は、ステップ `" ++ stage_written cx.spec st.name ++ "` でまだ答えていない承認者ではありません（または " ++ to ++ " がもう承認者です）"))

/-- A comment: a line of the history, and a notice to the requester, the one who made the request for them, the one who sent it back to the requester while it is with them, and the ones the stage waits for, past the one who wrote it. The ticket's state, clocks and revision do not move. Who may write one is the gate's to say -/
def do_comment (cx : Cx) (s : Step) (by_ : String) (text : Option String) : Except Refusal Step :=
  match text with
  | none => throw (refusal .comment "the text of `comment` is empty" "`comment` の文が空です")
  | some x =>
    let back : List String :=
      match s.t.sent_back_by with
      | none => []
      | some b => [b]
    let waits : List String :=
      match at_of s.t with
      | none => []
      | some i =>
        match at? s.t.stages i with
        | none => []
        | some st => waited st cx.org
    let to := (distinct (theirs (who_of s.t) ++ back ++ waits)).filter (fun p => p != by_)
    let s1 := note_down s { rec_of "commented" with stage := s.t.stage, by_ := some by_, comment := some x }
    pure (tell s1 to .comment s.t.stage)

def back_step (t : Ticket) (s : Scan) (j : Int) : Scan :=
  if s.done then s
  else
    match at? t.stages j with
    | none => { found := none, done := true }
    | some st =>
      match st.phase with
      | .answered => { found := some j, done := true }
      | .deferred => if present st.started then { found := some j, done := true } else s
      | .skipped | .once | .circulated => s
      | _ => { found := none, done := true }

/-- The stage an approval to retract was given at: the last one before `i` that people answered, past the ones that approval did, the ones nobody answers and the circulations -/
def back_to (t : Ticket) (i : Int) : Option Int :=
  let n := max (min i 1000000) 0
  ((numbers 1 n).foldl (fun s k => back_step t s (n - k)) ({ found := none, done := false } : Scan)).found

def unanswered (p : Part) (by_ : String) : Part :=
  let answers := p.answers.filter (fun a => a.by_ != by_)
  let q := { p with answers := answers }
  if p.state == .passed && countIf answers (fun a => a.yes) < p.needed then { q with state := .active }
  else q

def do_retract (cx : Cx) (s : Step) (by_ : String)
    (clocks : String → Int → Int → Int → Except Refusal Clocks)
    (open_after : String → Int → Int → Except Refusal Int) : Except Refusal Step := do
  let cur ← stage_in s.t "retract" [Status.in_progress]
  let i := index_in s.t
  if cx.spec.retract then
    if cur.parts.any (fun p => (p.answers.length : Int) > 0 || p.state == .checked) then
      throw (refusal .state ("the stage `" ++ stage_written cx.spec cur.name ++ "` has an answer already, and the approval before it is not retracted") ("ステップ `" ++ stage_written cx.spec cur.name ++ "` に、もう答えがあります。前の承認は取り消せません"))
    else
      match back_to s.t i with
      | none => throw (refusal .state ("no approval before `" ++ stage_written cx.spec cur.name ++ "` is there to retract") ("`" ++ stage_written cx.spec cur.name ++ "` の前に、取り消せる承認がありません"))
      | some j0 =>
        let j := max (min j0 1000000) 0
        match at? s.t.stages j with
        | none => throw (refusal .state ("no approval before `" ++ stage_written cx.spec cur.name ++ "` is there to retract") ("`" ++ stage_written cx.spec cur.name ++ "` の前に、取り消せる承認がありません"))
        | some sj =>
          match first? (sj.parts.flatMap (fun p => p.answers)) (fun a => a.by_ == by_ && a.yes) with
          | none => throw (refusal .person (who_en cx.org by_ ++ " did not approve the stage `" ++ stage_written cx.spec sj.name ++ "`") (who_ja cx.org by_ ++ "は、ステップ `" ++ stage_written cx.spec sj.name ++ "` を承認していません"))
          | some given =>
            let proxy : Option String := if given.seat == by_ then none else some given.seat
            let s1 := cancel_clocks s
            let t1 := with_stage s1.t j { sj with parts := sj.parts.map (fun p => unanswered p by_), phase := .active }
            let t2 := { unwind cx t1 (min (j + 1) 1000000) (some j) with stage := some sj.name }
            let s2 := note_down { s1 with t := t2 } { rec_of "retracted" with stage := some sj.name, by_ := some by_, proxy_of := proxy }
            let s3 ← set_clocks cx s2 j false clocks open_after
            match at? s3.t.stages j with
            | none => pure s3
            | some sj2 => pure (tell s3 (waited sj2 cx.org) .waiting (some sj.name))
  else throw (refusal .event "the request has no `retract approval` line" "この申請の種類には `retract approval` の行がありません")

def do_pause (cx : Cx) (s : Step) (by_ : String)
    (open_after : String → Int → Int → Except Refusal Int) : Except Refusal Step := do
  let st ← stage_in s.t "pause" [Status.in_progress]
  let _who ← approver cx s.t st by_
  match cx.spec.pause with
  | none => throw (refusal .event "the request has no `pause` line" "この申請の種類には `pause` の行がありません")
  | some most =>
    if s.t.pause_left == 0 then
      throw (refusal .limit ("the ticket has been paused for the " ++ textInt most ++ " business days it may be (`pause at most " ++ textInt most ++ " business days`)") ("保留は合わせて" ++ textInt most ++ "営業日までで、もう残っていません（`pause at most " ++ textInt most ++ " business days`）"))
    else
      let day ← open_after "pause" (day_where cx.spec cx.now "pause ends") s.t.pause_left
      let ends ← end_of_day day (clock_offset cx.spec "pause ends" none)
      let s1 := cancel_clocks s
      let t := { s1.t with status := .paused, paused_at := some cx.now, pause_ends := some ends }
      let s2 := note_down { s1 with t := t } { rec_of "paused" with stage := some st.name, by_ := some by_, proxy_of := acting_for cx.org st by_ }
      let s3 := put s2 (.notify (theirs (who_of t)) .paused (some st.name))
      pure (put s3 (.set_timer .pause_ends (some st.name) ends))

def resumed_by (cx : Cx) (t : Ticket) (st : Stage) (by_ : Option String) (ev : String) : Except Refusal Bool :=
  match by_ with
  | some b => approver cx t st b
  | none => rung cx.spec t.pause_ends ev (some st.name) cx.now

/-- `resume` by an approver, or the clock of the pause (`by` none): the stage's clocks are set again, later by the business days it was paused for -/
def do_resume (cx : Cx) (s : Step) (by_ : Option String)
    (clocks : String → Int → Int → Int → Except Refusal Clocks)
    (open_after : String → Int → Int → Except Refusal Int) : Except Refusal Step := do
  let ev : String :=
    match by_ with
    | none => "pause ends"
    | some _ => "resume"
  let st ← stage_in s.t ev [Status.paused]
  let i := index_in s.t
  let _who ← resumed_by cx s.t st by_ ev
  let u ← unpause cx s (!(present by_)) open_after
  let s1 := if present by_ && present u.s.t.pause_ends then put u.s (.cancel_timer .pause_ends (some st.name)) else u.s
  let st1 := { st with held := min (st.held + u.used) 100000 }
  let used := textInt u.used ++ " business days"
  let proxy : Option String :=
    match by_ with
    | none => none
    | some b => acting_for cx.org st b
  let s2 := note_down { s1 with t := { with_stage s1.t i st1 with pause_ends := none, status := .in_progress } } { rec_of "resumed" with stage := some st.name, by_ := by_, proxy_of := proxy, note := some used }
  let s3 ← set_clocks cx s2 i false clocks open_after
  match at? s3.t.stages i with
  | none => pure s3
  | some st2 => pure (put s3 (.notify (theirs (who_of s3.t) ++ waited st2 cx.org) .resumed (some st.name)))

def do_remind (cx : Cx) (s : Step) : Except Refusal Step := do
  let st ← stage_in s.t "remind" [Status.in_progress]
  let i := index_in s.t
  let _r ← rung cx.spec st.remind "remind" (some st.name) cx.now
  let st1 := { st with remind := none, reminded := true }
  let to := waited st1 cx.org
  let s1 := note_down { s with t := with_stage s.t i st1 } { rec_of "reminded" with stage := some st.name, to := to }
  pure (tell s1 to .reminder (some st.name))

def do_resubmit_late (cx : Cx) (s : Step)
    (on : Ending → Ref → Org → Except Refusal (List OnLine)) : Except Refusal Step := do
  if s.t.status == .with_requester then
    let _r ← rung cx.spec s.t.resubmit_due "resubmit late" none cx.now
    end_ticket cx { s with t := { s.t with resubmit_due := none } } .expired .expired "ticket expired" none none (some "resubmit late") on
  else throw (state_refusal s.t.status "resubmit late")

/-- What the stage does at this deadline: what `when late` says first, then what it says after `then` -/
def late_act (o : Occ) (up : Int) : Option LateAct :=
  match o.late with
  | none => none
  | some l =>
    match up with
    | 0 =>
      let first := l.first
      some first
    | 1 => l.after
    | _ => none

def then_stays (o : Occ) : Bool :=
  match o.late with
  | none => false
  | some l =>
    match l.after with
    | none => false
    | some a =>
      match a with
      | .stay => true
      | _ => false

def head_not (spec : Spec) (who : Who) (team : Team) (p : String) : Option String :=
  match team.head with
  | none => none
  | some h => if h == p || left_out spec who h then none else some h

/-- The first head above the person who answers in their place: from their primary team up, each team's head who is not them and not left out -/
def above_person (cx : Cx) (who : Who) (p : String) : Option String :=
  ((chain cx.org (primary cx.org p)).filterMap (fun x => head_not cx.spec who x p)).head?

def above_move (cx : Cx) (who : Who) (k : Int) (p : Part) : Option Move :=
  let to : Option String :=
    match p.team with
    | some tm => up_from cx.spec who cx.org (.above tm)
    | none =>
      match p.approvers.head? with
      | none => none
      | some a => above_person cx who a
  match to with
  | none => none
  | some h => some { part := k, people := [h], way := none, team := none }

/-- Where a table sends a late part: the people of the way it names, or the head above where the way finds nobody -/
def sent_move (cx : Cx) (who : Who) (vals : List Arg) (k : Int) (way : String) : Option Move :=
  if is_chosen cx.spec way then none
  else
    match solve cx.spec who vals cx.org way with
    | .people ps team =>
      if (ps.length : Int) == 0 then none
      else some { part := k, people := ps, way := some way, team := team }
    | .nobody _ up _ =>
      match up_from cx.spec who cx.org up with
      | none => none
      | some h => some { part := k, people := [h], way := some way, team := none }

def table_move (cx : Cx) (who : Who) (vals : List Arg) (k : Int) (p : Part) (sends : List Send) : Option Move :=
  match first? sends (fun x => x.key == p.key) with
  | none => none
  | some sd => (sd.to.filterMap (fun w => sent_move cx who vals k w)).head?

/-- Where a part that still waits goes up to when it is late: an agent that has not answered, with no table, to the way after `else` -/
def move_of (cx : Cx) (who : Who) (vals : List Arg) (o : Occ) (x : Indexed Part) (table : Bool)
    (sends : List Send) : Option Move :=
  if x.item.state == .agent && !table then
    match at? o.ways x.index with
    | none => none
    | some way =>
      match solve cx.spec who vals cx.org way with
      | .people ps team => some { part := x.index, people := ps, way := none, team := team }
      | .nobody _ _ _ => none
  else
    if table then table_move cx who vals x.index x.item sends
    else above_move cx who x.index x.item

def moved (x : Indexed Part) (moves : List Move) : Part :=
  match first? moves (fun m => m.part == x.index) with
  | none => x.item
  | some m =>
    let p := x.item
    let n : Int :=
      match m.team with
      | none => 1
      | some _ => min (max p.needed 1) (max (min (m.people.length : Int) 1000000) 1)
    let sole : Sole :=
      match m.people.head? with
      | none => Sole.other
      | some a =>
        if (m.people.length : Int) == 1 && n == 1 && p.state != .agent then Sole.by_ a
        else Sole.other
    let key : String :=
      match m.way with
      | none => p.key
      | some w => w
    { p with needed := n, sole := sole, key := key, answers := [], state := .active, of := none, approvers := m.people, team := m.team }

def gathered (acc : List String) (m : Move) : List String :=
  acc ++ m.people.filter (fun p => !(acc.contains p))

def expire (cx : Cx) (s : Step) (note : String)
    (on : Ending → Ref → Org → Except Refusal (List OnLine)) : Except Refusal Step :=
  end_ticket cx s .expired .expired "ticket expired" none none (some note) on

/-- The stage goes up for being late: each part that still waits to where it goes up, with a deadline and a reminder counted from the time the deadline passed, the reminder not yet rung -/
def go_up (cx : Cx) (s : Step) (i : Int) (st : Stage) (o : Occ) (table : Bool) (sends : List Send)
    (clocks : String → Int → Int → Int → Except Refusal Clocks)
    (_open_after : String → Int → Int → Except Refusal Int)
    (on : Ending → Ref → Org → Except Refusal (List OnLine)) : Except Refusal Step := do
  let tried := ((indexed st.parts).filter (fun x => !(part_done x.item))).map (fun x => move_of cx (who_of s.t) s.vals o x table sends)
  if tried.any (fun m => !(present m)) then
    if st.up == 0 && then_stays o then pure (note_down s { rec_of "late" with stage := some st.name, note := some "nowhere to go up; it waits on" })
    else expire cx s "late; nowhere to go up" on
  else
    let moves := tried.filterMap (fun m => m)
    let none_yet : List String := []
    let to := takeI (moves.foldl (fun acc m => gathered (takeI acc 9223372036854775807) m) none_yet) 9223372036854775807
    let up := min (st.up + 1) 255
    let started : Int :=
      match st.started with
      | none => cx.now
      | some x => x
    let c ← clocks st.name started st.held up
    let d : Option Int :=
      match c.due with
      | none => none
      | some x => some (max x cx.now)
    let r : Option Int :=
      match c.remind with
      | none => none
      | some x => before (max x cx.now) d
    let st2 := { st with parts := (indexed st.parts).map (fun x => moved x moves), up := up, remind := r, reminded := false, due := d }
    let s1 := note_down { s with t := with_stage s.t i st2 } { rec_of "went up" with stage := some st.name, to := to, organization := some cx.org.version, note := some "late" }
    let s2 := tell s1 (waited st2 cx.org) .went_up (some st.name)
    pure (timer_at (timer_at s2 .remind st.name r) .late st.name d)

/-- The deadline of the ticket's stage passed -/
def do_late (cx : Cx) (s : Step)
    (clocks : String → Int → Int → Int → Except Refusal Clocks)
    (open_after : String → Int → Int → Except Refusal Int)
    (on : Ending → Ref → Org → Except Refusal (List OnLine)) : Except Refusal Step := do
  let st ← stage_in s.t "late" [Status.in_progress]
  let i := index_in s.t
  let _r ← rung cx.spec st.due "late" (some st.name) cx.now
  let o ← occ_of cx.spec st
  let st1 := { st with due := none, remind := none }
  let s0 := { s with t := with_stage s.t i st1 }
  let s1 : Step :=
    match st.remind with
    | none => s0
    | some _ => put s0 (.cancel_timer .remind (some st.name))
  match o.shape with
  | .note _ =>
    let s2 := note_down s1 { rec_of "note missing" with stage := some st.name, note := some "late" }
    advance cx (set_phase s2 i .noted) (min (i + 1) 1000000) false clocks open_after on
  | _ =>
    match late_act o st.up with
    | none => expire cx s1 "late" on
    | some a =>
      match a with
      | .expire => expire cx s1 "late" on
      | .stay => pure (note_down s1 { rec_of "late" with stage := some st.name, note := some "it waits on" })
      | .escalate e => go_up cx s1 i st1 o e.table e.sends clocks open_after on

/-- The stage is done by a flow's answer while the ticket is paused: the pause ends with it, the business days it took counted -/
def lift (cx : Cx) (s : Step) (i : Int)
    (open_after : String → Int → Int → Except Refusal Int) : Except Refusal Step := do
  if s.t.status == .paused then
    let name := name_at s.t.stages i
    let u ← unpause cx s false open_after
    let used := textInt u.used ++ " business days"
    let s1 := note_down (cancel_pause u.s) { rec_of "resumed" with stage := some name, note := some used }
    let s2 := put s1 (.notify (theirs (who_of s1.t)) .resumed (some name))
    match at? s2.t.stages i with
    | none => pure { s2 with t := { s2.t with status := .in_progress } }
    | some st => pure { s2 with t := { with_stage s2.t i { st with remind := none, due := none } with status := .in_progress } }
  else pure s

/-- The agent doubts (`why` is its reason), or its flow failed: the way after `else` answers the part -/
def doubted (cx : Cx) (s : Step) (i : Int) (st : Stage) (o : Occ) (x : Indexed Part) (note : String)
    (why : Option String) (failed : Bool) : Except Refusal Step :=
  let did := if failed then "agent failed" else "agent doubt"
  let s1 := note_down s { rec_of did with stage := some st.name, note := some note, comment := why }
  match at? o.ways x.index with
  | none => throw (refusal .unread ("the stage `" ++ st.name ++ "` has no such way") ("ステップ `" ++ st.name ++ "` の決め方がありません"))
  | some way =>
    let np := part_of cx.spec (who_of s1.t) s1.vals o st way (quorum_of o) cx.org false
    let s2 := put_all { s1 with t := with_stage s1.t i { st with parts := replace_part st.parts (max (min x.index 1000000) 0) np.part } } np.made
    let to : List String := if np.part.state == .active then np.part.approvers else []
    let s3 := tell s2 to .waiting (some st.name)
    if np.part.state == .stopped then pure (put s3 (.notify [] .stopped (some st.name)))
    else pure s3

/-- An agent's flow answered `ok`, or a doubt, with its reason, or failed, which is a doubt; a summary's flow that failed leaves the ticket to go on without it. A flow answers when it is done, so a paused ticket takes its answer too. The history names the flow in its note, and keeps the reason as its comment -/
def do_agent (cx : Cx) (s : Step) (yes : Bool) (reason : String) (failed : Bool)
    (clocks : String → Int → Int → Int → Except Refusal Clocks)
    (open_after : String → Int → Int → Except Refusal Int)
    (on : Ending → Ref → Org → Except Refusal (List OnLine)) : Except Refusal Step := do
  let ev := if failed then "flow failed" else "agent answered"
  let st ← stage_in s.t ev [Status.in_progress, Status.paused]
  let i := index_in s.t
  let o ← occ_of cx.spec st
  match o.shape with
  | .note _ =>
    if failed then
      let s1 := note_down s { rec_of "note missing" with stage := some st.name, note := some "flow failed" }
      let s2 ← lift cx s1 i open_after
      advance cx (set_phase (cancel_clocks s2) i .noted) (min (i + 1) 1000000) false clocks open_after on
    else throw (refusal .event ("the stage `" ++ stage_written cx.spec st.name ++ "` is a summary, and takes `note ready`") ("ステップ `" ++ stage_written cx.spec st.name ++ "` は要約です。`note ready` を受け付けます"))
  | _ =>
    match first? (indexed st.parts) (fun x => x.item.state == .agent) with
    | none => throw (refusal .state ("the stage `" ++ stage_written cx.spec st.name ++ "` waits for no agent's answer") ("ステップ `" ++ stage_written cx.spec st.name ++ "` は、エージェントの答えを待っていません"))
    | some x =>
      let flow : String :=
        match x.item.of with
        | none => ""
        | some f => flow_shown cx.spec f
      let why : Option String := if reason == "" then none else some reason
      let s1 : Except Refusal Step :=
        if yes then .ok (note_down { s with t := with_stage s.t i { st with parts := replace_part st.parts (max (min x.index 1000000) 0) { x.item with state := .checked } } } { rec_of "agent ok" with stage := some st.name, note := some flow, comment := why })
        else doubted cx s i st o x flow why failed
      let s2 ← s1
      match at? s2.t.stages i with
      | none => pure s2
      | some st2 =>
        if st2.parts.all (fun p => part_done p) then
          let s3 ← lift cx s2 i open_after
          passed_on cx s3 i clocks open_after on
        else pure s2

def do_note (cx : Cx) (s : Step) (outputs : List Arg)
    (clocks : String → Int → Int → Int → Except Refusal Clocks)
    (open_after : String → Int → Int → Except Refusal Int)
    (on : Ending → Ref → Org → Except Refusal (List OnLine)) : Except Refusal Step := do
  let st ← stage_in s.t "note ready" [Status.in_progress, Status.paused]
  let i := index_in s.t
  let o ← occ_of cx.spec st
  match o.shape with
  | .note _ =>
    let s1 := note_down { s with t := { s.t with notes := s.t.notes ++ [({ stage := st.name, outputs := outputs } : Note)] } } { rec_of "note attached" with stage := some st.name }
    let s2 ← lift cx s1 i open_after
    advance cx (set_phase (cancel_clocks s2) i .noted) (min (i + 1) 1000000) false clocks open_after on
  | _ => throw (refusal .state ("the stage `" ++ stage_written cx.spec st.name ++ "` is no summary") ("ステップ `" ++ stage_written cx.spec st.name ++ "` は要約ではありません"))

def act (cx : Cx) (t : Ticket) (ev : Event) (c : Option String) (vals : List Arg)
    (clocks : String → Int → Int → Int → Except Refusal Clocks)
    (open_after : String → Int → Int → Except Refusal Int)
    (on : Ending → Ref → Org → Form → Except Refusal (List OnLine))
    (reform : Form → Form → List String → Bool → Org → Except Refusal Formed) : Except Refusal Step :=
  let s : Step := { t := t, fx := [], vals := vals }
  match ev with
  | .approve a => answer cx s a.by_ true clocks open_after (fun e r o => on e r o t.form)
  | .reject a => answer cx s a.by_ false clocks open_after (fun e r o => on e r o t.form)
  | .send_back a => do_send_back cx s a.by_ a.to clocks open_after (fun e r o => on e r o t.form)
  | .retract a => do_retract cx s a.by_ clocks open_after
  | .pause a => do_pause cx s a.by_ open_after
  | .resume a => do_resume cx s (some a.by_) clocks open_after
  | .pause_ends => do_resume cx s none clocks open_after
  | .withdraw a => do_withdraw cx s a.by_ open_after (fun e r o => on e r o t.form)
  | .discard a => do_discard cx s a.by_ (fun e r o => on e r o t.form)
  | .resubmit a => do_resubmit cx s a.by_ a.form a.clear a.attempt clocks open_after on reform
  | .edit a => do_edit cx s a.by_ a.fields a.clear reform
  | .add_approver a => do_add_approver cx s a.by_ a.approver a.part
  | .hand_over a => do_hand_over cx s a.by_ a.of a.to
  | .comment a => do_comment cx s a.by_ c
  | .remind => do_remind cx s
  | .late => do_late cx s clocks open_after (fun e r o => on e r o t.form)
  | .resubmit_late => do_resubmit_late cx s (fun e r o => on e r o t.form)
  | .agent_answered a => do_agent cx s (a.verdict == .ok) a.reason false clocks open_after (fun e r o => on e r o t.form)
  | .flow_failed _ => do_agent cx s false "" true clocks open_after (fun e r o => on e r o t.form)
  | .note_ready a => do_note cx s a.outputs clocks open_after (fun e r o => on e r o t.form)

/-- One event of a ticket: the ticket as it is after it, with what is to do; or why the event is not taken. Asked in order: a clock that has rung, the revision the event was made on, the state of the ticket, the comment and the reason the request asks with it, then who gives it. What a person wrote with it is on the line of the history of what they did -/
def next (spec : Spec) (t : Ticket) (ev : Event) (org : Org) (now : Int) (vals : List Arg)
    (clocks : String → Int → Int → Int → Except Refusal Clocks)
    (open_after : String → Int → Int → Except Refusal Int)
    (on : Ending → Ref → Org → Form → Except Refusal (List OnLine))
    (reform : Form → Form → List String → Bool → Org → Except Refusal Formed) : Except Refusal Moved := do
  let _day ← on_day org now spec.offset
  let r : Option Rung := if is_clock ev then none else rung_first t now
  match r with
  | some x =>
    let a := time_text x.at_ (clock_offset spec x.timer x.stage)
    let of_en : String :=
      match x.stage with
      | none => ""
      | some st => " (`" ++ stage_written spec st ++ "`)"
    let of_ja : String :=
      match x.stage with
      | none => ""
      | some st => "（`" ++ stage_written spec st ++ "`）"
    throw (refusal .clock ("the clock of `" ++ x.timer ++ "`" ++ of_en ++ " rang at " ++ a ++ ": its event comes before `" ++ event_word ev ++ "`") ("`" ++ x.timer ++ "` の時計" ++ of_ja ++ "が " ++ a ++ " に鳴っています。`" ++ event_word ev ++ "` より先に、その時計のイベントを渡してください"))
  | none =>
    let _same ← fresh t ev
    if ended t.status then
      throw (refusal .state ("the ticket has ended (`" ++ Status.word t.status ++ "`), and takes no `" ++ event_word ev ++ "`") ("チケットは `" ++ Status.word t.status ++ "` で終わっています。`" ++ event_word ev ++ "` は受け付けません"))
    else
      let written ← said (comment_of ev)
      let c ← reasoned spec ev written
      let s ← act { spec := spec, org := org, now := now } t ev c vals clocks open_after on reform
      pure (done (commented s c))

end RingiCore
