/-
  The machine's code from its days and times to what a move makes (`ticket.kn`, from "days and
  times" through "what is made"): a day of a time and the time of a day, the part of the
  organization (its teams, members, positions and proxies), the values of the form, who answers a
  way, and the list of what is to do. Each function as the machine's code writes it, in its order.

  Where the names differ from the machine's code:
  - the arguments `at`, `by` and `open` are `at_`, `by_` and `open_`, and the local `local`
    (`day_number`, `time_text`) is `local_`, as Lean takes those words for its own;
  - `day_where` comes before `moved_time`, which calls it;
  - `done` is not here: it calls `revision_of`, which the machine's code writes after it, among the
    stages.
-/
import RingiCore.Records

namespace RingiCore

-- ── days and times ─────────────────────────────────────────────────────────

/-- The day the time is in, counted from 1970-01-01, as the request reads its days in its offset (minutes east of UTC) -/
def day_number (now : Int) (offset : Int) : Int :=
  let local_ := now + offset * 60
  (local_ - local_ % 86400) / 86400

/-- The day a time is in, as the request reads its days -/
def day_at (at_ : Int) (offset : Int) : Int :=
  max (min (day_number at_ offset) 2932896) 0

/-- The time of a day: `minutes` after its midnight (1440 is the end of the day) in the offset -/
def instant (day : Int) (minutes : Int) (offset : Int) : Int :=
  day * 86400 + minutes * 60 - offset * 60

/-- The UTC offset the clock is read in: the offset of the calendar it counts in, the request's where that calendar writes none -/
def clock_offset (spec : Spec) (timer : String) (stage : Option String) : Int :=
  match first? spec.offsets (fun x => x.timer == timer && x.stage == stage) with
  | some x => x.offset
  | none =>
    match first? spec.offsets (fun x => x.timer == timer && x.stage == none) with
    | some x => x.offset
    | none => spec.offset

/-- A time as a refusal says it: its day and its hours and minutes in the offset (`2026-11-20 09:00`) -/
def time_text (at_ : Int) (offset : Int) : String :=
  let local_ := at_ + offset * 60
  let minutes := (local_ % 86400 - local_ % 60) / 60
  let hours := (minutes - minutes % 60) / 60
  dateText (day_at at_ offset) ++ " " ++ padStart (textInt hours) 2 '0' ++ ":" ++ padStart (textInt (minutes % 60)) 2 '0'

/-- A time, when it is one a ticket holds: 1970-01-01 to 9999-12-31 -/
def time_in (x : Int) : Option Int :=
  if x >= 0 && x <= 253402300799 then
    some x
  else
    none

/-- The end of the day, its 24:00, as a time a ticket holds -/
def end_of_day (day : Int) (offset : Int) : Except Refusal Int := do
  match time_in (instant day 1440 offset) with
  | none => throw (refusal .data ("the end of " ++ dateText day ++ " is no time a ticket holds") (dateText day ++ " の終わりは、チケットが持てる時刻ではありません"))
  | some x => pure x

/-- The part of the organization is of the day the event is on -/
def on_day (org : Org) (now : Int) (offset : Int) : Except Refusal Int := do
  if org.day == day_number now offset then
    pure org.day
  else
    throw (refusal .data ("the part of the organization is of " ++ dateText org.day ++ ", and the event is on another day") ("組織の部分が " ++ dateText org.day ++ " のものです。イベントは別の日に起きました"))

/-- The day is one the request can be used on -/
def usable_on (spec : Spec) (day : Int) : Except Refusal Bool := do
  let early :=
    match spec.usable_from with
    | none => false
    | some f => decide (day < f)
  let late :=
    match spec.usable_to with
    | none => false
    | some u => decide (day > u)
  if early || late then
    throw (refusal .data ("the request cannot be used on " ++ dateText day) ("この申請の種類は " ++ dateText day ++ " には使えません"))
  else
    pure true

def refusal_of_day (r : Except Refusal Int) : Option Refusal :=
  match r with
  | .error e => some e
  | .ok _ => none

def on_or_before (r : Except Refusal Int) (b : Int) : Bool :=
  match r with
  | .error _ => false
  | .ok d => decide (d <= b)

/-- The business days of the pause's calendar after `a`, through `b`: at most `most` of them -/
def open_between (a : Int) (b : Int) (most : Int) (open_after : String → Int → Int → Except Refusal Int) : Except Refusal Int := do
  let tried := (numbers 1 most).map (fun k => open_after "pause" a k)
  match (tried.filterMap (fun r => refusal_of_day r)).head? with
  | some e => throw e
  | none => pure (countIf tried (fun r => on_or_before r b))

-- `day_where` is written here, before `moved_time`, which calls it (the machine's code writes it after
-- `unmoved_time`).

/-- The day a time is in where the calendar of a clock counts (`pause ends`, `resubmit late`): each calendar reads its own day of a time, in its offset -/
def day_where (spec : Spec) (at_ : Int) (timer : String) : Int :=
  day_at at_ (clock_offset spec timer none)

/-- The time a stage's clocks count from: the time it started, later by the days the business days it was paused for take in the pause's calendar (`open`), counted from the day it started there. Each calendar counts from its own day of it -/
def moved_time (spec : Spec) (at_ : Int) (held : Int) (open_ : Int → Int → Except Refusal Int) : Except Refusal Int := do
  if held == 0 then
    pure at_
  else
    let begun := day_where spec at_ "pause ends"
    match open_ begun held with
    | .error e => throw e
    | .ok d => pure (at_ + (d - begun) * 86400)

/-- The time a stage's clocks count from, in a request with no pause: the time it started -/
def unmoved_time (at_ : Int) (held : Int) : Except Refusal Int := do
  if held == 0 then
    pure at_
  else
    throw (refusal .unread "the request counts no business days for `pause`" "この申請の種類は、`pause` の営業日を数えません")

-- ── the organization ───────────────────────────────────────────────────────

def in_org (org : Org) (id : String) : Bool :=
  org.people.any (fun p => p.id == id)

/-- A word a Japanese sentence goes on after: followed by a space where it ends in a character of ASCII -/
def ja_word (s : String) : String :=
  match ((s.drop (chars s - 1).toNat).copy.toList.head?).map (fun ch => (ch.toNat : Int)) with
  | none => s
  | some c =>
    if c <= 127 then
      s ++ " "
    else
      s

/-- A person as a refusal names them in English: their ID and name (`e3101 (Head of Sales Department 1)`), the ID alone where the part of the organization has no name of theirs -/
def who_en (org : Org) (id : String) : String :=
  match first? org.people (fun p => p.id == id) with
  | none => id
  | some p =>
    if p.name == "" || p.name == id then
      id
    else
      id ++ " (" ++ p.name ++ ")"

/-- The same in Japanese, the Japanese name where there is one (`e3101（営業部長 1）`), as a Japanese sentence goes on after it (`e1201 は`) -/
def who_ja (org : Org) (id : String) : String :=
  let named :=
    match first? org.people (fun p => p.id == id) with
    | none => id
    | some p =>
      let n := if p.name_ja == "" then p.name else p.name_ja
      if n == "" || n == id then
        id
      else
        id ++ "（" ++ n ++ "）"
  ja_word named

/-- A stage as the request writes it (`決裁`), by the name the ticket gives it (`decide`) -/
def stage_written (spec : Spec) (name : String) : String :=
  match first? spec.named (fun x => x.name == name) with
  | none => name
  | some x => x.written

def has_team (org : Org) (id : String) : Bool :=
  org.teams.any (fun t => t.id == id)

/-- The master has the row on the day: its rows in effect are what the part of the organization carries -/
def has_row (org : Org) (master : String) (key : String) : Bool :=
  org.rows.any (fun r => r.master == master && r.key == key)

def team_of (org : Org) (id : String) : Option Team :=
  first? org.teams (fun t => t.id == id)

/-- The person's primary team -/
def primary (org : Org) (person : String) : Option String :=
  match first? org.members (fun m => m.person == person && m.primary) with
  | none => none
  | some m => some m.team

def climb (org : Org) (c : Climb) : Climb :=
  match c.at_ with
  | none => c
  | some id =>
    match team_of org id with
    | none => { at_ := none, teams := c.teams }
    | some t => { at_ := t.parent, teams := c.teams ++ [t] }

/-- The team and the teams above it, nearest first, at most 30 of them -/
def chain (org : Org) (team : Option String) : List Team :=
  ((numbers 1 30).foldl (fun c _ => climb org c) ({ at_ := team, teams := [] } : Climb)).teams

/-- The team of the kind that the person's primary team is or is below: `requester.department` -/
def kind_above (org : Org) (person : String) (kind : String) : Option String :=
  match first? (chain org (primary org person)) (fun t => t.kind == kind) with
  | none => none
  | some t => some t.id

/-- The person is a member of the team -/
def member_of (org : Org) (person : String) (team : String) : Bool :=
  org.members.any (fun m => m.person == person && m.team == team)

/-- The person's primary team is the team, or one below it -/
def under (org : Org) (person : String) (team : String) : Bool :=
  (chain org (primary org person)).any (fun t => t.id == team)

/-- The place of a position in the organization's positions -/
def rank (org : Org) (position : String) : Option Int :=
  match first? (indexed org.positions) (fun x => x.item == position) with
  | none => none
  | some x => some x.index

/-- The place of the person's position in their primary team -/
def position_of (org : Org) (person : String) : Option Int :=
  match first? org.members (fun m => m.person == person && m.primary) with
  | none => none
  | some m => rank org m.position

/-- Who the ticket is for: its ID, its requester and the one who made it for them -/
def who_of (t : Ticket) : Who :=
  { id := t.id, requester := t.requester, made_by := t.made_by }

/-- The person is left out of every way: the requester or the one who made the request, with self approval not allowed -/
def left_out (spec : Spec) (who : Who) (id : String) : Bool :=
  !spec.self_approval && (who.requester == id || who.made_by == some id)

def not_in (id : String) (org : Org) : Refusal :=
  refusal .person (id ++ " is not in the organization from " ++ dateText org.version) (id ++ " は、" ++ dateText org.version ++ " からの組織にいません")

/-- The one is a proxy who may answer for the person that day -/
def is_proxy (org : Org) (by_ : String) (person : String) : Bool :=
  org.proxies.any (fun x => x.kind == ProxyKind.approve && x.person == person && x.proxy == by_)

-- ── the form ───────────────────────────────────────────────────────────────

/-- Whether a value is filled in -/
def present {α} (v : Option α) : Bool :=
  match v with
  | none => false
  | some _ => true

/-- The value where it is kept, none where it is dropped -/
def kept {α} (v : Option α) (keep : Bool) : Option α :=
  if keep then v else none

def chars_long (n : Int) : String :=
  if n == 1 then "1 character long" else textInt n ++ " characters long"

def rows_many (n : Int) : String :=
  if n == 1 then "1 row" else textInt n ++ " rows"

-- The refusals are written as records here, not through `refusal_at`: a record a function answers,
-- made an optional one, is copied in the Rust kaname writes (`Some((&r).clone())`), which clippy's
-- `needless_borrow` refuses in the package's crate.
/-- Why the form does not take a field shown or asked for on a condition: a value of a field it does not show, or none of a field it asks for -/
def untaken (has : Bool) (shown : Bool) (asked : Bool) (name : String) (at_ : String) (missing : String) (missing_ja : String) : Option Refusal :=
  if shown then
    if has || !asked then
      none
    else
      some { refused := .form, text := missing, text_ja := missing_ja, at_ := some at_ }
  else
    if has then
      some { refused := .form, text := "`" ++ name ++ "` is not shown for this request (its `shown when` does not hold), and takes no value", text_ja := "`" ++ name ++ "` は、この申請では見せないフィールドです（`shown when` が成り立たない）。値は送りません", at_ := some at_ }
    else
      none

/-- The value of a field of the form that names a person or a row, when it is filled in -/
def pick (vals : List Arg) (field : String) : Option String :=
  match first? vals (fun a => a.name == field) with
  | none => none
  | some a =>
    match a.value with
    | .str v => if v == "" then none else some v
    | _ => none

def arg_str (name : String) (v : String) : Arg :=
  Arg.mk name (.str v)

def arg_num (name : String) (v : Int) : Arg :=
  Arg.mk name (.num v)

def arg_truth (name : String) (v : Bool) : Arg :=
  Arg.mk name (.truth v)

def arg_rows (name : String) (v : List (List Arg)) : Arg :=
  Arg.mk name (.rows v)

/-- A text, or nothing where it is not filled in -/
def arg_maybe_str (name : String) (v : Option String) : Arg :=
  match v with
  | none => Arg.mk name .nothing
  | some x => arg_str name x

/-- A number, or nothing where it is not filled in -/
def arg_maybe_num (name : String) (v : Option Int) : Arg :=
  match v with
  | none => Arg.mk name .nothing
  | some x => arg_num name x

/-- A truth, or nothing where it is not filled in -/
def arg_maybe_truth (name : String) (v : Option Bool) : Arg :=
  match v with
  | none => Arg.mk name .nothing
  | some x => arg_truth name x

/-- The person holds the position in their primary team, or, with `higher`, one above it: `submitted by position manager or higher` -/
def at_position (org : Org) (person : String) (position : String) (higher : Bool) : Bool :=
  match rank org position with
  | none => false
  | some r =>
    match position_of org person with
    | none => false
    | some p => p == r || (higher && decide (p > r))

/-- What a flow the ticket asks is given: the ticket's ID, and the values of its form -/
def flow_args (who : Who) (vals : List Arg) : List Arg :=
  [arg_str "request_id" who.id] ++ vals

-- ── who answers ────────────────────────────────────────────────────────────

def why_note (why : Why) : String :=
  match why with
  | .person_not_in id => id ++ " is not in the organization"
  | .team_not_in id => "the team " ++ id ++ " is not in the organization"
  | .head_empty team => "the head of " ++ team ++ " is empty"
  | .only_requester _ => "it comes to the requester alone"
  | .no_team_of_kind team_kind => "no team of the kind " ++ team_kind ++ " is above the requester"
  | .none_above => "no head is above the requester"
  | .no_members team => team ++ " has no member"

def nobody (why : Why) (up : Up) : Solved :=
  let cannot :=
    match up with
    | .cannot =>
      match why with
      | .person_not_in _ | .team_not_in _ => true
      | _ => false
    | _ => false
  .nobody (why_note why) up cannot

def cannot (note : String) : Solved :=
  .nobody note .cannot true

/-- The head of the team, when there is one and it is neither the requester nor left out -/
def head_for (spec : Spec) (who : Who) (team : Team) : Option String :=
  match team.head with
  | none => none
  | some h =>
    if h == who.requester || left_out spec who h then
      none
    else
      some h

/-- The head of the team, when there is one and it is not left out -/
def head_kept (spec : Spec) (who : Who) (team : Team) : Option String :=
  match team.head with
  | none => none
  | some h =>
    if left_out spec who h then
      none
    else
      some h

def manager_of (spec : Spec) (who : Who) (org : Org) : Solved :=
  match primary org who.requester with
  | none => nobody .none_above .cannot
  | some p =>
    match ((chain org (some p)).filterMap (fun x => head_for spec who x)).head? with
    | none => nobody .none_above .cannot
    | some h => .people [h] none

def head_of_kind (spec : Spec) (who : Who) (org : Org) (kind : String) : Solved :=
  match primary org who.requester with
  | none => nobody .none_above .cannot
  | some p =>
    match first? (chain org (some p)) (fun x => x.kind == kind) with
    | none => nobody (.no_team_of_kind kind) (.at_ p)
    | some tm =>
      match tm.head with
      | none => nobody (.head_empty tm.id) (.above tm.id)
      | some h =>
        if left_out spec who h then
          nobody (.only_requester h) (.above tm.id)
        else
          .people [h] none

/-- The head of the team, when they hold the position or a higher one there, and are neither the requester nor left out -/
def senior (spec : Spec) (who : Who) (org : Org) (team : Team) (want : Int) : Option String :=
  match head_for spec who team with
  | none => none
  | some h =>
    match first? org.members (fun m => m.person == h && m.team == team.id) with
    | none => none
    | some m =>
      match rank org m.position with
      | none => none
      | some r =>
        if r >= want then
          some h
        else
          none

def nearest_way (spec : Spec) (who : Who) (org : Org) (position : String) (written : String) : Solved :=
  match primary org who.requester with
  | none => nobody .none_above .cannot
  | some p =>
    match rank org position with
    | none => cannot ("the organization does not decide the way to choose " ++ written)
    | some want =>
      match ((chain org (some p)).filterMap (fun x => senior spec who org x want)).head? with
      | none => nobody .none_above .cannot
      | some h => .people [h] none

def team_way (spec : Spec) (who : Who) (org : Org) (id : String) : Solved :=
  match team_of org id with
  | none => nobody (.team_not_in id) .cannot
  | some _ =>
    let all := (org.members.filter (fun m => m.team == id)).map (fun m => m.person)
    let people := all.filter (fun p => !left_out spec who p)
    match all.head? with
    | none => nobody (.no_members id) (.above id)
    | some a =>
      if (people.length : Int) == 0 then
        nobody (.only_requester a) (.above id)
      else
        .people people (some id)

def person_way (spec : Spec) (who : Who) (org : Org) (id : String) : Solved :=
  if in_org org id then
    if left_out spec who id then
      match primary org id with
      | none => nobody (.only_requester id) .cannot
      | some p => nobody (.only_requester id) (.at_ p)
    else
      .people [id] none
  else
    nobody (.person_not_in id) .cannot

/-- The person in the field of the row of the master that a field of the form chose, on the day -/
def lookup_way (spec : Spec) (who : Who) (vals : List Arg) (org : Org) (master : String) (field : String) (by_ : String) (shown : String) (column : String) (by_name : String) : Solved :=
  match pick vals by_ with
  | none => cannot (by_name ++ " is empty, and no row of " ++ shown ++ " is looked up")
  | some key =>
    match first? org.rows (fun r => r.master == master && r.key == key) with
    | none => cannot (shown ++ " has no row " ++ key ++ " in effect on " ++ dateText org.day)
    | some row =>
      match first? row.values (fun c => c.name == field && c.value != "") with
      | none => cannot ("the row " ++ key ++ " of " ++ shown ++ " has no " ++ column)
      | some c => person_way spec who org c.value

def solve_way (spec : Spec) (who : Who) (vals : List Arg) (org : Org) (w : Way) : Solved :=
  match w.how with
  | .manager_of => manager_of spec who org
  | .head_of team_kind => head_of_kind spec who org team_kind
  | .nearest position => nearest_way spec who org position w.written
  | .team id => team_way spec who org id
  | .person id => person_way spec who org id
  | .lookup x => lookup_way spec who vals org x.master x.field x.by_ x.shown x.column x.by_name
  | .chosen_by x =>
    match pick vals x.field with
    | none => cannot ("nobody is chosen in " ++ x.shown)
    | some id => person_way spec who org id
  | .agent _ => cannot ("the organization does not decide the way to choose " ++ w.written)

/-- The way to choose, solved for the ticket in the part of the organization: the way after `else` for an agent's -/
def solve (spec : Spec) (who : Who) (vals : List Arg) (org : Org) (way : String) : Solved :=
  match first? spec.ways (fun w => w.name == way) with
  | none => cannot ("the way to choose " ++ way ++ " is not declared")
  | some w =>
    match w.how with
    | .agent a =>
      match first? spec.ways (fun x => x.name == a.otherwise) with
      | none => cannot ("the way to choose " ++ a.otherwise ++ " is not declared")
      | some o => solve_way spec who vals org o
    | _ => solve_way spec who vals org w

/-- Where a stage with nobody goes up to: the first head above who is not left out -/
def up_from (spec : Spec) (who : Who) (org : Org) (origin : Up) : Option String :=
  match origin with
  | .cannot => none
  | .at_ team => ((chain org (some team)).filterMap (fun x => head_kept spec who x)).head?
  | .above team => ((dropI (chain org (some team)) 1).filterMap (fun x => head_kept spec who x)).head?

def written (spec : Spec) (way : String) : String :=
  match first? spec.ways (fun w => w.name == way) with
  | none => way
  | some w => w.written

/-- A flow as the request writes it, which the history names -/
def flow_shown (spec : Spec) (flow : String) : String :=
  match first? spec.flows (fun f => f.name == flow) with
  | none => flow
  | some f => f.shown

/-- The flow of an agent's way -/
def agent_flow (spec : Spec) (way : String) : Option String :=
  match first? spec.ways (fun w => w.name == way) with
  | none => none
  | some w =>
    match w.how with
    | .agent a => some a.flow
    | _ => none

/-- Whether the way is a person chosen in a field of the form -/
def is_chosen (spec : Spec) (way : String) : Bool :=
  match first? spec.ways (fun w => w.name == way) with
  | none => false
  | some w =>
    match w.how with
    | .chosen_by _ => true
    | _ => false

/-- The field the approver before chooses the person of the way in -/
def chosen_by_previous (spec : Spec) (way : String) : Option Chosen :=
  match first? spec.ways (fun w => w.name == way) with
  | none => none
  | some w =>
    match w.how with
    | .chosen_by c =>
      if c.previous then
        some { field := c.field, shown := c.shown }
      else
        none
    | _ => none

-- ── what is made ───────────────────────────────────────────────────────────

def rank_of (what : What) : Int :=
  match what with
  | .history .. => 0
  | .cancel_timer .. => 1
  | .post .. => 2
  | .release .. => 3
  | .hold .. => 4
  | .launch .. => 5
  | .ask_agent .. | .ask_note .. => 6
  | .notify .. => 7
  | .set_timer .. => 8

def put (s : Step) (what : What) : Step :=
  { s with fx := s.fx ++ [({ round := s.t.round, what := what } : Made)] }

def put_all (s : Step) (made : List What) : Step :=
  { s with fx := s.fx ++ made.map (fun w => ({ round := s.t.round, what := w } : Made)) }

def rec_of (did : String) : Rec :=
  { did := did, stage := none, by_ := none, proxy_of := none, organization := none, once_with := none, to := [], note := none, comment := none, cleared := [] }

def hist (r : Rec) : What :=
  .history { did := r.did, stage := r.stage, by_ := r.by_, proxy_of := r.proxy_of, organization := r.organization, once_with := r.once_with, to := r.to, note := r.note, comment := r.comment, cleared := r.cleared }

def note_down (s : Step) (r : Rec) : Step :=
  put s (hist r)

def is_history (w : What) : Bool :=
  match w with
  | .history _ => true
  | _ => false

def with_comment (w : What) (c : String) : What :=
  match w with
  | .history h => .history { did := h.did, stage := h.stage, by_ := h.by_, proxy_of := h.proxy_of, organization := h.organization, once_with := h.once_with, to := h.to, note := h.note, comment := some c, cleared := h.cleared }
  | _ => w

/-- What a person wrote with what they did, on the first line of the history the move made: the line of what they did -/
def commented (s : Step) (c : Option String) : Step :=
  match c with
  | none => s
  | some x =>
    match first? (indexed s.fx) (fun m => is_history m.item.what) with
    | none => s
    | some h => { s with fx := (indexed s.fx).map (fun m => if m.index == h.index then { m.item with what := with_comment m.item.what x } else m.item) }

/-- A notice to the ones named: none to nobody -/
def tell (s : Step) (to : List String) (notice : Notice) (stage : Option String) : Step :=
  if (to.length : Int) == 0 then
    s
  else
    put s (.notify to notice stage)

def keyed (t : Ticket) (x : Indexed Made) : Effect :=
  { key := { ticket := t.id, round := x.item.round, serial := min (t.serial + max (min x.index 1000000) 0 + 1) 1000000000 }, what := x.item.what }

-- `done` (the last function of "what is made") calls `revision_of`, which the machine's code writes
-- among the stages, after it; it goes there, after `revision_of`.

end RingiCore
