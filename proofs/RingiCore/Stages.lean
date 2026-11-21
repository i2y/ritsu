/-
  The stages of the ticket machine (`ticket.kn`, its sections "the stages", "`same approver
  approves once`" and "coming to a stage and moving on"): a stage as the ticket comes to it, the
  parts of a stage and who answers them, `same approver approves once` before and at the last
  stage, the clocks of a stage, and the walk over the stages as a ticket moves on.

  Each function is the machine's function of its name, its arguments in their order. Where it is
  not so:
  - `decide` is `decide_once` (`decide` is Lean's).
  - The functions are in the order Lean needs, each after the ones it calls: `is_rule_output` before
    `occ_of`; `at_top`, `empty_part` and `solved_part` before `part_of`; `timer_at`, `before`,
    `set_times`, `set_clocks` and `wait_at` (of "coming to a stage and moving on") before
    `come_again`.
  - `done`, the last function of "what is made" (before the stages), calls `revision_of`; it is
    written here, after `revision_of`.
  - `finish` calls `on_ending`, of the section "the `on` lines" after these. `closes`, `without`,
    `on_line` and `on_ending` are written here, before `finish`, and not again with the `on` lines.
  - The arguments and locals `at`, `by` and `open` are `at_`, `by_` and `open_` (and the fields
    `at`, `by`, `end` and `when` of the records, `at_`, `by_`, `end_` and `when_`).
-/
import RingiCore.Org

set_option linter.unusedVariables false

namespace RingiCore

-- ── the stages ─────────────────────────────────────────────────────────────

/-- A stage of the route as the ticket comes to it: by its name and the value of its output in the lowered route -/
def new_stage (name : String) (value : String) : Stage :=
  { name := name, value := value, phase := .ahead, started := none, held := 0, remind := none,
    reminded := false, due := none, up := 0, org := none, again := false, parts := [] }

/-- The stage as it is before the ticket comes to it -/
def reset (st : Stage) : Stage :=
  new_stage st.name st.value

def with_stage (t : Ticket) (i : Int) (st : Stage) : Ticket :=
  { t with stages := takeI t.stages i ++ [st] ++ dropI t.stages (i + 1) }

/-- The parts with the one at `k` as it is now -/
def replace_part (ps : List Part) (k : Int) (p : Part) : List Part :=
  takeI ps k ++ [p] ++ dropI ps (k + 1)

def set_phase (s : Step) (i : Int) (phase : Phase) : Step :=
  match at? s.t.stages i with
  | none => s
  | some st => { s with t := with_stage s.t i { st with phase := phase } }

def name_at (stages : List Stage) (k : Int) : String :=
  match at? stages k with
  | none => ""
  | some st => st.name

/-- The stage the ticket is at: the last of its name -/
def at_of (t : Ticket) : Option Int :=
  match t.stage with
  | none => none
  | some name =>
    match last? (indexed t.stages) (fun x => x.item.name == name) with
    | none => none
    | some x => some x.index

/-- The ticket's revision, worked out from it: the one place it is made -/
def revision_of (t : Ticket) : Revision :=
  let stage : Option Int :=
    match at_of t with
    | none => none
    | some i => some (max (min i 1000000) 0)
  { round := t.round, stage := stage, edits := t.edits }

-- `done` is the last function of "what is made", before the stages; it calls `revision_of`, so it is here.
/-- The list in its order, keyed from the ticket's serial on; the ticket's serial moves past them, and its revision is worked out of it as it is now -/
def done (s : Step) : Moved :=
  let effects := (indexed (sortBy s.fx (fun m => rank_of m.what))).map (fun x => keyed s.t x)
  let serial := min (s.t.serial + min (effects.length : Int) 1000000) 1000000000
  { ticket := { s.t with serial := serial, revision := revision_of s.t }, effects := effects }

def is_rule_output (o : Occ) : Bool :=
  match o.shape with
  | .rule_output => true
  | _ => false

/-- The stage of the route a ticket's stage is: by its name and the value of its output; a rule's output's value names its way -/
def occ_of (spec : Spec) (st : Stage) : Except Refusal Occ :=
  match first? spec.occs (fun o => o.stage == st.name && o.value == st.value) with
  | some o => pure o
  | none =>
    match first? spec.occs (fun o => o.stage == st.name && is_rule_output o) with
    | some o => pure { o with ways := [st.value] }
    | none => throw (refusal .unread ("the route has no stage `" ++ st.name ++ "` of `" ++ st.value ++ "`") ("ステップ `" ++ st.name ++ "`（`" ++ st.value ++ "`）が経路にありません"))

def quorum_of (o : Occ) : Option Quorum :=
  match o.shape with
  | .quorum x =>
    let q := x
    some q
  | _ => none

/-- How many approvals a part of `n` people needs -/
def needed (q : Option Quorum) (n : Int) : Int :=
  match q with
  | none => 1
  | some x =>
    match x with
    | .all => n
    | .at_least a => a
    | .percent c => (n * c + 100 - 1) / 100

/-- What a part comes to as `same approver approves once` reads it: one person alone, or others -/
def sole_of (ps : List String) (needed : Int) (alone : Bool) : Sole :=
  match ps.head? with
  | none => .other
  | some p =>
    if alone && (ps.length : Int) == 1 && needed == 1 then .by_ p else .other

def part_done (p : Part) : Bool :=
  match p.state with
  | .passed | .skipped | .checked | .once | .deferred => true
  | _ => false

def passed_phase (parts : List Part) : Phase :=
  if parts.any (fun p => p.state == PartState.deferred) then .deferred
  else
    if parts.any (fun p => p.state == PartState.once) then .once
    else .skipped

/-- The one person a part waits for a later stage's approval of, and that stage -/
def waits_for (p : Part) : Option Wait :=
  if p.state == PartState.deferred then
    match p.sole with
    | .by_ b =>
      match p.of with
      | none => none
      | some to => some { person := b, to := to }
    | _ => none
  else none

/-- The one person who alone answers a part that waits for them -/
def alone_by (p : Part) : Option String :=
  if p.state == PartState.active then
    match p.sole with
    | .by_ b => some b
    | _ => none
  else none

/-- The stage is the last of its name on the ticket -/
def live (stages : List Stage) (a : Int) (name : String) : Bool :=
  !(dropI stages (max (min a 1000000) 0 + 1)).any (fun s => s.name == name)

def places_of (x : Indexed Stage) : List Place :=
  ((indexed x.item.parts).filter (fun p => present (waits_for p.item))).map
    (fun p => { stage := x.index, part := p.index })

/-- The parts that wait for a later stage's approval, among the stages before `i` that are the last of their names -/
def waiting (stages : List Stage) (i : Int) : List Place :=
  ((indexed stages).filter (fun x => x.index < i && live stages x.index x.item.name)).flatMap
    (fun x => places_of x)

def part_now (p : Indexed Part) (cs : List Change) : Part :=
  match first? cs (fun c => c.part == p.index) with
  | none => p.item
  | some c => c.p

/-- A stage with the parts of it the changes name as they are now -/
def changed (x : Indexed Stage) (cs : List Change) : Stage :=
  let mine := cs.filter (fun c => c.stage == x.index)
  if (mine.length : Int) == 0 then x.item
  else { x.item with parts := (indexed x.item.parts).map (fun p => part_now p mine) }

def at_stage (did : String) (st : Stage) (org : Org) (to : List String) (note : String) : What :=
  hist { rec_of did with stage := some st.name, organization := some org.version, to := to, note := some note }

def at_top (spec : Spec) (who : Who) (vals : List Arg) (st : Stage) (p : Part) (why : String) (top : Option AtTop) (org : Org) (alone : Bool) : Parted :=
  let stopped : Parted := { part := { p with state := .stopped }, made := [at_stage "stopped" st org [] (why ++ "; nobody is above to go up to")] }
  match top with
  | none => stopped
  | some a =>
    match a with
    | .stay => stopped
    | .skip => { part := { p with state := .skipped }, made := [at_stage "skipped" st org [] (why ++ "; nobody is above to go up to")] }
    | .to w =>
      let name := written spec w
      match solve spec who vals org w with
      | .people ps team => { part := { p with key := w, approvers := ps, team := team, sole := sole_of ps 1 alone }, made := [at_stage "went to" st org ps (why ++ "; nobody is above to go up to: " ++ name)] }
      | .nobody nwhy _ _ => { part := { p with state := .stopped }, made := [at_stage "stopped" st org [] (why ++ "; nobody is above to go up to, and " ++ name ++ " has nobody: " ++ nwhy)] }

def empty_part (spec : Spec) (who : Who) (vals : List Arg) (o : Occ) (st : Stage) (p : Part) (why : String) (origin : Up) (cannot : Bool) (org : Org) (alone : Bool) : Parted :=
  match o.empty with
  | .stay => { part := { p with state := .stopped }, made := [at_stage "stopped" st org [] why] }
  | .skip => { part := { p with state := .skipped }, made := [at_stage "skipped" st org [] why] }
  | .escalate top =>
    if cannot then
      { part := { p with state := .stopped }, made := [at_stage "stopped" st org [] (why ++ "; it cannot go up")] }
    else
      match up_from spec who org origin with
      | some h => { part := { p with approvers := [h], sole := sole_of [h] 1 alone }, made := [at_stage "went up" st org [h] why] }
      | none => at_top spec who vals st p why top org alone

def solved_part (spec : Spec) (who : Who) (vals : List Arg) (o : Occ) (st : Stage) (way : String) (q : Option Quorum) (org : Org) (p : Part) (alone : Bool) : Parted :=
  match solve spec who vals org way with
  | .people ps team =>
    let n := needed q (max (min (ps.length : Int) 1000000) 0)
    let q2 := { p with approvers := ps, team := team, needed := n, sole := sole_of ps n alone }
    if (ps.length : Int) < n then
      { part := { q2 with state := .stopped }, made := [at_stage "stopped" st org ps (textInt (ps.length : Int) ++ " of " ++ written spec way ++ " cannot give the " ++ textInt n ++ " approvals the stage needs")] }
    else { part := q2, made := [] }
  | .nobody why up cannot =>
    if in_org org who.requester then
      empty_part spec who vals o st p why up cannot org alone
    else
      { part := { p with state := .stopped }, made := [at_stage "stopped" st org [] (why ++ "; the requester " ++ who.requester ++ " is not in the organization from " ++ dateText org.version)] }

/-- One part of a stage as it starts: who answers the way, or what the stage's `when empty` does with nobody, with its line of the history; an agent's way asks its flow when `ask` -/
def part_of (spec : Spec) (who : Who) (vals : List Arg) (o : Occ) (st : Stage) (way : String) (q : Option Quorum) (org : Org) (ask : Bool) : Parted :=
  let flow := agent_flow spec way
  let alone := !present flow && !is_chosen spec way
  let p : Part := { way := way, key := way, approvers := [], team := none, needed := 1, answers := [], state := .active, of := none, sole := .other }
  let asked : Option Parted :=
    match flow with
    | none => none
    | some f =>
      if ask then
        some { part := { p with state := .agent, of := some f }, made := [.ask_agent f st.name (flow_args who vals), hist { rec_of "agent asked" with stage := some st.name, organization := some org.version, note := some (flow_shown spec f) }] }
      else none
  match asked with
  | some x => x
  | none => solved_part spec who vals o st way q org p alone

/-- Who a stage waits for: its approvers yet to answer, and the proxies of theirs in effect that day -/
def waited (st : Stage) (org : Org) : List String :=
  let own := distinct ((st.parts.filter (fun p => p.state == PartState.active)).flatMap
    (fun p => p.approvers.filter (fun a => !p.answers.any (fun x => x.seat == a))))
  let proxies := own.flatMap (fun a => (org.proxies.filter (fun x => x.kind == ProxyKind.approve && x.person == a)).map (fun x => x.proxy))
  distinct (own ++ proxies)

def theirs (who : Who) : List String :=
  match who.made_by with
  | none => [who.requester]
  | some b => [who.requester, b]

-- ── `same approver approves once` ──────────────────────────────────────────

def out_of (p : Part) : Out :=
  if p.state == PartState.skipped then .passing
  else
    match p.sole with
    | .passing => .passing
    | .by_ b => .by_ b
    | .other => .other

/-- A stage before, as `same approver approves once` reads it: what it came to on the ticket -/
def look_of (st : Stage) : Look :=
  match st.phase with
  | .circulated | .skipped | .ahead => .passed_over
  | .noted | .active | .deferred => .other
  | .answered => .parts (st.parts.map (fun p => out_of p)) false
  | .once => .parts (st.parts.map (fun p => out_of p)) true

def is_passing (o : Out) : Bool :=
  match o with
  | .passing => true
  | _ => false

def is_by (o : Out) (x : String) : Bool :=
  match o with
  | .by_ b => b == x
  | _ => false

def scan_step (looks : List Look) (s : Scan) (k : Int) (x : String) : Scan :=
  if s.done then s
  else
    match at? looks k with
    | none => { found := none, done := true }
    | some l =>
      match l with
      | .passed_over => s
      | .other => { found := none, done := true }
      | .parts outs _ =>
        if outs.all (fun o => is_passing o) then s
        else
          if outs.any (fun o => !is_passing o && !is_by o x) then { found := none, done := true }
          else { found := some k, done := true }

/-- The stage before `ib` whose approval does a part `x` alone answers: the first one back that anyone answers, when only `x` answers it -/
def before_on (looks : List Look) (ib : Int) (x : String) : Option Int :=
  let n := max (min ib 1000000) 0
  ((numbers 1 n).foldl (fun (s : Scan) j => scan_step looks s (n - j) x) { found := none, done := false }).found

def origin_step (looks : List Look) (k : Int) (x : String) : Int :=
  match at? looks k with
  | none => k
  | some l =>
    match l with
    | .parts _ o =>
      match o with
      | true =>
        match before_on looks k x with
        | none => k
        | some m => m
      | false => k
    | _ => k

/-- The stage `x` approved that `same approver approves once` does the stage `k` with -/
def origin_on (looks : List Look) (k : Int) (x : String) : Int :=
  (numbers 1 (max (min k 1000000) 0)).foldl (fun (acc : Int) j => origin_step looks acc x) k

/-- The persons who alone answer a part of the stage `k`, were it to start now -/
def alone_at (cx : Cx) (who : Who) (stages : List Stage) (vals : List Arg) (k : Int) : List String :=
  match at? stages k with
  | none => []
  | some st =>
    match occ_of cx.spec st with
    | .error e => []
    | .ok o =>
      distinct ((o.ways.map (fun way => (part_of cx.spec who vals o st way (quorum_of o) cx.org true).part)).filterMap
        (fun p => alone_by p))

/-- The last stage at or after `first` that `x` alone answers a part of, as the stages would start now -/
def last_of (cx : Cx) (who : Who) (stages : List Stage) (vals : List Arg) (first : Int) (x : String) : Option Int :=
  match last? (indexed stages) (fun y => y.index >= first && (alone_at cx who stages vals y.index).contains x) with
  | none => none
  | some y => some y.index

def done_before (cx : Cx) (stages : List Stage) (looks : List Look) (i : Int) (st : Stage) (p : Part) : Parted :=
  match alone_by p with
  | none => { part := p, made := [] }
  | some x =>
    match before_on looks i x with
    | none => { part := p, made := [] }
    | some k =>
      let at_ := origin_on looks k x
      let origin := name_at stages at_
      let given : Option Answer :=
        match at? stages at_ with
        | none => none
        | some so => first? (so.parts.flatMap (fun q => q.answers)) (fun a => a.yes && a.seat == x)
      let by_ :=
        match given with
        | none => x
        | some a => a.by_
      let proxy : Option String :=
        if by_ == x then none else some x
      { part := { p with state := .once, of := some origin },
        made := [hist { rec_of "approved" with stage := some st.name, by_ := some by_, proxy_of := proxy, organization := some cx.org.version, once_with := some origin }] }

/-- `same approver approves once`: a part one person alone answers is done with the approval of the stage before it that they alone approved -/
def once_with_before (cx : Cx) (s : Step) (i : Int) (st : Stage) (parts : List Part) : Parts :=
  if cx.spec.once == Once.before && !st.again then
    let looks := (takeI s.t.stages i).map (fun x => look_of x)
    let made := parts.map (fun p => done_before cx s.t.stages looks i st p)
    { s := put_all s (made.flatMap (fun x => x.made)), parts := made.map (fun x => x.part) }
  else { s := s, parts := parts }

def defer_own (cx : Cx) (who : Who) (stages : List Stage) (vals : List Arg) (i : Int) (st : Stage) (p : Part) : Parted :=
  match alone_by p with
  | none => { part := p, made := [] }
  | some x =>
    match last_of cx who stages vals (i + 1) x with
    | none => { part := p, made := [] }
    | some k =>
      let to := name_at stages k
      { part := { p with state := .deferred, of := some to },
        made := [hist { rec_of "deferred" with stage := some st.name, organization := some cx.org.version, once_with := some to, to := [x] }] }

/-- A part before that waited for the stage `name`: where its one person waits for a later stage here too, it waits for that one -/
def waits_too (stages : List Stage) (pl : Place) (name : String) (parts : List Part) : Option Change :=
  match at? stages pl.stage with
  | none => none
  | some sa =>
    match at? sa.parts pl.part with
    | none => none
    | some pa =>
      match waits_for pa with
      | none => none
      | some w =>
        if w.to == name then
          match first? (parts.filterMap (fun p => waits_for p)) (fun y => y.person == w.person) with
          | none => none
          | some later =>
            some { stage := pl.stage, part := pl.part, p := { pa with of := some later.to },
                   made := [hist { rec_of "deferred" with stage := some sa.name, organization := sa.org, once_with := some later.to, to := [w.person], note := some "the stage it waited for waits too" }] }
        else none

/-- `same approver approves once, at the last stage`: a part one person alone answers, who alone answers a part of a stage ahead too, waits for the last of them; the parts that waited for this stage wait on with it -/
def once_at_last (cx : Cx) (s : Step) (i : Int) (st : Stage) (parts : List Part) : Parts :=
  if cx.spec.once == Once.last then
    let own : List Parted :=
      if st.again then parts.map (fun p => { part := p, made := [] })
      else parts.map (fun p => defer_own cx (who_of s.t) s.t.stages s.vals i st p)
    let s1 := put_all s (own.flatMap (fun x => x.made))
    let ps := own.map (fun x => x.part)
    let cs := (waiting s1.t.stages i).filterMap (fun pl => waits_too s1.t.stages pl st.name ps)
    let t := { s1.t with stages := (indexed s1.t.stages).map (fun x => changed x cs) }
    { s := put_all { s1 with t := t } (cs.flatMap (fun c => c.made)), parts := ps }
  else { s := s, parts := parts }

-- ── coming to a stage and moving on: the clocks (`come_again` calls `wait_at`) ──

def timer_at (s : Step) (timer : Timer) (stage : String) (at_ : Option Int) : Step :=
  match at_ with
  | none => s
  | some x => put s (.set_timer timer (some stage) x)

def before (r : Int) (due : Option Int) : Option Int :=
  match due with
  | none => none
  | some d => if r < d then some r else none

/-- The clocks of the stage, set at `now`: neither before it, and the reminder only where it comes before the deadline -/
def set_times (s : Step) (i : Int) (st : Stage) (remind : Option Int) (due : Option Int) (now : Int) : Step :=
  let d : Option Int :=
    match due with
    | none => none
    | some x => some (max x now)
  let r : Option Int :=
    match remind with
    | none => none
    | some x => before (max x now) d
  let t := with_stage s.t i { st with remind := r, due := d }
  timer_at (timer_at { s with t := t } .remind st.name r) .late st.name d

/-- The clocks of the stage, counted from the time it started and the days it was paused for and set, none before now, the reminder only where it comes before the deadline. A stage that starts (`fresh`) sets both; a stage come back to, resumed or come back to by a retracted approval, counts again the clocks it has not heard ring: the deadline, where it is set, and the reminder, where it has not rung (`reminded`), set before or not -/
def set_clocks (cx : Cx) (s : Step) (i : Int) (fresh : Bool) (clocks : String → Int → Int → Int → Except Refusal Clocks) (open_after : String → Int → Int → Except Refusal Int) : Except Refusal Step := do
  match at? s.t.stages i with
  | none => pure s
  | some st =>
    match st.started with
    | none => pure s
    | some started =>
      let c ← clocks st.name started st.held st.up
      let remind := if fresh || !st.reminded then c.remind else none
      let due := if fresh || present st.due then c.due else none
      pure (set_times s i st remind due cx.now)

/-- The ticket waits at the stage: it starts now, the ones it waits for are told, and its clocks are set -/
def wait_at (cx : Cx) (s : Step) (i : Int) (clocks : String → Int → Int → Int → Except Refusal Clocks) (open_after : String → Int → Int → Except Refusal Int) : Except Refusal Step := do
  let u ← usable_on cx.spec cx.org.day
  match at? s.t.stages i with
  | none => pure s
  | some st0 =>
    let st := { st0 with phase := .active, started := some cx.now }
    let s1 := tell { s with t := { with_stage s.t i st with stage := some st.name } } (waited st cx.org) .waiting (some st.name)
    let s2 := if st.parts.any (fun p => p.state == PartState.stopped) then put s1 (.notify [] .stopped (some st.name)) else s1
    set_clocks cx s2 i true clocks open_after

-- ── the stages, on: a part that waits for a later stage, decided again ──

/-- A part that waits for a later stage, as the ticket comes to the stage `i`: none while that stage is still one to wait for; else it is decided again, and waits on for the last stage ahead the one who answers it now alone answers, or starts -/
def decide_once (cx : Cx) (who : Who) (stages : List Stage) (vals : List Arg) (pl : Place) (i : Int) (ahead : List String) (here : List String) : Except Refusal (Option Decided) := do
  match at? stages pl.stage with
  | none => pure none
  | some sa =>
    match at? sa.parts pl.part with
    | none => pure none
    | some pa =>
      match waits_for pa with
      | none => pure none
      | some w =>
        let why : String :=
          match first? (indexed ahead) (fun y => y.item == w.to) with
          | some y =>
            if y.index == 0 && !here.contains w.person then "the stage it waited for does not come to " ++ w.person ++ " alone"
            else ""
          | none =>
            if (takeI stages i).any (fun x => x.name == w.to) then "the stage it waited for was answered without " ++ w.person
            else "the stage it waited for is not on the route"
        if why == "" then pure none
        else
          let o ← occ_of cx.spec sa
          if o.ways.contains pa.way then
            let np := part_of cx.spec who vals o sa pa.way (quorum_of o) cx.org true
            let later : Option Wait :=
              match alone_by np.part with
              | none => none
              | some y =>
                match last_of cx who stages vals i y with
                | none => none
                | some k2 => some { person := y, to := name_at stages k2 }
            match later with
            | some lw =>
              pure (some { stage := pl.stage, part := pl.part, fate := .waits, p := { np.part with state := .deferred, of := some lw.to },
                           made := np.made ++ [hist { rec_of "deferred" with stage := some sa.name, organization := some cx.org.version, once_with := some lw.to, to := [lw.person], note := some why }] })
            | none =>
              pure (some { stage := pl.stage, part := pl.part, fate := .starts, p := np.part,
                           made := np.made ++ [hist { rec_of "deferral ended" with stage := some sa.name, organization := some cx.org.version, once_with := some w.to, to := np.part.approvers, note := some why }] })
          else
            throw (refusal .unread ("the stage `" ++ sa.name ++ "` has no way `" ++ pa.way ++ "`") ("ステップ `" ++ sa.name ++ "` の決め方 `" ++ pa.way ++ "` がありません"))

def start_of (x : Try) : Option Int :=
  match x.r with
  | .error e => none
  | .ok d =>
    match d with
    | none => none
    | some y => if y.fate == Fate.starts then some x.stage else none

/-- A decision made before the stage that comes again, or at it: the ones after it are not made -/
def tried_by (x : Try) (first : Option Int) : Bool :=
  match first with
  | none => true
  | some a => x.stage <= a

def refused (x : Try) : Option Refusal :=
  match x.r with
  | .error e => some e
  | .ok d => none

def decided_of (x : Try) : Option Decided :=
  match x.r with
  | .error e => none
  | .ok d => d

/-- A stage before, with the parts of it that wait on for another stage; one that never started takes the part of the organization of now -/
def settled_stage (cx : Cx) (x : Indexed Stage) (ds : List Decided) : Stage :=
  let cs := (ds.filter (fun d => d.stage == x.index && d.fate == Fate.waits)).map
    (fun d => ({ stage := d.stage, part := d.part, p := d.p, made := [] } : Change))
  if (cs.length : Int) == 0 then x.item
  else
    let org : Option Int :=
      match x.item.started with
      | none => some cx.org.version
      | some at_ => x.item.org
    { changed x cs with org := org }

def restarted (x : Indexed Part) (started : List Started) : Part :=
  match first? started (fun y => y.part == x.index) with
  | none => x.item
  | some y => y.p

/-- The stage comes again, here: the parts started now, and the ones of it that wait on -/
def come_again (cx : Cx) (s : Step) (a : Int) (i : Int) (started : List Started) (clocks : String → Int → Int → Int → Except Refusal Clocks) (open_after : String → Int → Int → Except Refusal Int) : Except Refusal Settle := do
  match at? s.t.stages a with
  | none => pure { s := s, how := .clear }
  | some sa =>
    let parts := ((indexed sa.parts).filter (fun x => present (waits_for x.item))).map (fun x => restarted x started)
    let again : Stage := { new_stage sa.name sa.value with again := true, org := some cx.org.version, parts := parts }
    let s1 : Step := { s with t := { s.t with stages := takeI s.t.stages i ++ [again] ++ dropI s.t.stages i } }
    if parts.all (fun p => part_done p) then
      let phase := if parts.any (fun p => p.state == PartState.deferred) then Phase.deferred else Phase.skipped
      pure { s := set_phase s1 i phase, how := .passed }
    else
      let s2 ← wait_at cx s1 i clocks open_after
      pure { s := s2, how := .started }

/-- The parts that wait for a later stage, as the ticket comes to the stage `i` or to the end of its route: a part whose stage is no longer one to wait for is decided again, stage by stage, and the first stage before with a part that starts comes again here -/
def settle (cx : Cx) (s : Step) (i : Int) (clocks : String → Int → Int → Int → Except Refusal Clocks) (open_after : String → Int → Int → Except Refusal Int) : Except Refusal Settle := do
  let at_ := max (min i (s.t.stages.length : Int)) 0
  let w := waiting s.t.stages at_
  if (w.length : Int) == 0 then pure { s := s, how := .clear }
  else
    let ahead := (dropI s.t.stages at_).map (fun x => x.name)
    let here := alone_at cx (who_of s.t) s.t.stages s.vals at_
    let tried := w.map (fun pl => ({ stage := pl.stage, r := decide_once cx (who_of s.t) s.t.stages s.vals pl at_ ahead here } : Try))
    let first := (tried.filterMap (fun x => start_of x)).head?
    let upto := tried.filter (fun x => tried_by x first)
    match (upto.filterMap (fun x => refused x)).head? with
    | some e => throw e
    | none =>
      let ds := upto.filterMap (fun x => decided_of x)
      let t := { s.t with stages := (indexed s.t.stages).map (fun x => settled_stage cx x ds) }
      let s1 := put_all { s with t := t } (ds.flatMap (fun d => d.made))
      match first with
      | none => pure { s := s1, how := .clear }
      | some a =>
        come_again cx s1 a at_ ((ds.filter (fun d => d.stage == a && d.fate == Fate.starts)).map (fun d => ({ part := d.part, p := d.p } : Started))) clocks open_after

/-- A part before that waited for the stage `st`: done with the approval given at it in the seat of its one approver -/
def answered_at (stages : List Stage) (pl : Place) (st : Stage) : Option Change :=
  match at? stages pl.stage with
  | none => none
  | some sa =>
    match at? sa.parts pl.part with
    | none => none
    | some pa =>
      match waits_for pa with
      | none => none
      | some w =>
        if w.to == st.name then
          match first? (st.parts.flatMap (fun p => p.answers)) (fun x => x.yes && x.seat == w.person) with
          | none => none
          | some ans =>
            let proxy : Option String :=
              if ans.by_ == w.person then none else some w.person
            some { stage := pl.stage, part := pl.part, p := { pa with state := .once, of := some st.name },
                   made := [hist { rec_of "approved" with stage := some sa.name, by_ := some ans.by_, proxy_of := proxy, organization := sa.org, once_with := some st.name }] }
        else none

/-- A stage before with the parts done by an approval given later: answered when none of its parts waits any more -/
def answered_stage (x : Indexed Stage) (cs : List Change) : Stage :=
  if cs.any (fun c => c.stage == x.index) then
    let st := changed x cs
    let phase :=
      if st.parts.any (fun p => p.state == PartState.deferred) then st.phase
      else
        if present st.started then Phase.answered
        else Phase.once
    { st with phase := phase }
  else x.item

/-- The stage `i` has every approval it needs: each part that waited for it is done with the approval given at it in the seat of the part's one approver -/
def answered (s : Step) (i : Int) : Step :=
  match at? s.t.stages i with
  | none => s
  | some st =>
    let cs := (waiting s.t.stages i).filterMap (fun pl => answered_at s.t.stages pl st)
    let t := { s.t with stages := (indexed s.t.stages).map (fun x => answered_stage x cs) }
    put_all { s with t := t } (cs.flatMap (fun c => c.made))

def once_in (p : Part) (undone : List String) : Bool :=
  if p.state == PartState.once then
    match p.of with
    | none => false
    | some x => undone.contains x
  else false

/-- A stage before: the parts an approval given at a stage undone did wait for it again -/
def rewait (st : Stage) (undone : List String) : Stage :=
  let waits := st.parts.any (fun p => once_in p undone)
  let parts := st.parts.map (fun p => if once_in p undone then { p with state := .deferred } else p)
  let phase :=
    if waits && (st.phase == Phase.answered || st.phase == Phase.once) then Phase.deferred
    else st.phase
  { st with parts := parts, phase := phase }

/-- The stages from `first` on are as they were before the ticket came to them: a stage that came again there is taken off, and the parts before `first` that an approval given there or at the stage `reopened` did wait for it again -/
def unwind (cx : Cx) (t : Ticket) (first : Int) (reopened : Option Int) : Ticket :=
  let back : List String :=
    match reopened with
    | none => []
    | some j => [name_at t.stages j]
  let undone := (dropI t.stages first).map (fun x => x.name) ++ back
  let head := takeI t.stages first
  let rest := dropI t.stages first
  let tail := ((indexed rest).filter (fun x => !head.any (fun y => y.name == x.item.name) && !(takeI rest x.index).any (fun y => y.name == x.item.name))).map
    (fun x => reset x.item)
  let stages := head ++ tail
  if cx.spec.once == Once.last then
    { t with stages := (indexed stages).map (fun x => if x.index < first then rewait x.item undone else x.item) }
  else { t with stages := stages }

-- ── coming to a stage and moving on ────────────────────────────────────────

def cancel_remind (s : Step) (st : Stage) : Step :=
  match st.remind with
  | none => s
  | some r => put s (.cancel_timer .remind (some st.name))

def cancel_due (s : Step) (st : Stage) : Step :=
  match st.due with
  | none => s
  | some r => put s (.cancel_timer .late (some st.name))

/-- The clocks of the ticket's stage that are set are cancelled; the stage keeps their times. A paused ticket has none set -/
def cancel_clocks (s : Step) : Step :=
  if s.t.status == Status.paused then s
  else
    match at_of s.t with
    | none => s
    | some i =>
      match at? s.t.stages i with
      | none => s
      | some st => cancel_due (cancel_remind s st) st

def cancel_pause (s : Step) : Step :=
  match s.t.pause_ends with
  | none => s
  | some x => put { s with t := { s.t with pause_ends := none } } (.cancel_timer .pause_ends s.t.stage)

def cancel_back (s : Step) : Step :=
  match s.t.resubmit_due with
  | none => s
  | some x => put { s with t := { s.t with resubmit_due := none } } (.cancel_timer .resubmit_late none)

/-- A summary: its one part waits for the flow, which is asked for it -/
def ask_note (cx : Cx) (s : Step) (i : Int) (st : Stage) (flow : String) (clocks : String → Int → Int → Int → Except Refusal Clocks) (open_after : String → Int → Int → Except Refusal Int) : Except Refusal Step := do
  let u ← usable_on cx.spec cx.org.day
  let part : Part := { way := "", key := "", approvers := [], team := none, needed := 0, answers := [], state := .agent, of := some flow, sole := .other }
  let t := { with_stage s.t i { st with parts := [part], phase := .active, started := some cx.now } with stage := some st.name }
  let s1 := note_down { s with t := t } { rec_of "note asked" with stage := some st.name, note := some (flow_shown cx.spec flow) }
  let s2 := put s1 (.ask_note flow st.name (flow_args (who_of t) s.vals))
  set_clocks cx s2 i true clocks open_after

def people_stage (cx : Cx) (w : Walk) (i : Int) (o : Occ) (st : Stage) (clocks : String → Int → Int → Int → Except Refusal Clocks) (open_after : String → Int → Int → Except Refusal Int) : Walk :=
  let made := o.ways.map (fun way => part_of cx.spec (who_of w.s.t) w.s.vals o st way (quorum_of o) cx.org true)
  let s1 := put_all w.s (made.flatMap (fun x => x.made))
  let b := once_with_before cx s1 i st (made.map (fun x => x.part))
  let l := once_at_last cx b.s i st b.parts
  let s2 := { l.s with t := with_stage l.s.t i { st with parts := l.parts } }
  if l.parts.all (fun p => part_done p) then
    { w with s := set_phase s2 i (passed_phase l.parts), at_ := min (i + 1) 1000000 }
  else
    match wait_at cx s2 i clocks open_after with
    | .error e => { w with s := s2, over := true, stop := some e }
    | .ok s3 => { w with s := s3, over := true }

def circulated (cx : Cx) (to : Solved) : List String :=
  match to with
  | .people ps _ => ps
  | .nobody _ _ _ => []

def walk_stage (cx : Cx) (w : Walk) (clocks : String → Int → Int → Int → Except Refusal Clocks) (open_after : String → Int → Int → Except Refusal Int) : Walk :=
  let i := w.at_
  match at? w.s.t.stages i with
  | none => { w with over := true, end_ := true }
  | some st0 =>
    match occ_of cx.spec st0 with
    | .error e => { w with over := true, stop := some e }
    | .ok o =>
      let st := { reset st0 with again := st0.again, org := some cx.org.version }
      let s := { w.s with t := with_stage w.s.t i st }
      match o.shape with
      | .circulate way =>
        let to := circulated cx (solve cx.spec (who_of s.t) s.vals cx.org way)
        let s1 := note_down s { rec_of "circulated" with stage := some st.name, organization := some cx.org.version, to := to }
        { w with s := set_phase (tell s1 to .circulation (some st.name)) i .circulated, at_ := min (i + 1) 1000000 }
      | .note flow =>
        match ask_note cx s i st flow clocks open_after with
        | .error e => { w with s := s, over := true, stop := some e }
        | .ok s1 => { w with s := s1, over := true }
      | _ => people_stage cx { w with s := s } i o st clocks open_after

def walk (cx : Cx) (w : Walk) (clocks : String → Int → Int → Int → Except Refusal Clocks) (open_after : String → Int → Int → Except Refusal Int) : Walk :=
  if w.over then w
  else
    if cx.spec.once == Once.last then
      match settle cx w.s w.at_ clocks open_after with
      | .error e => { w with over := true, stop := some e }
      | .ok x =>
        match x.how with
        | .started => { w with s := x.s, over := true }
        | .passed => { w with s := x.s, at_ := min (w.at_ + 1) 1000000 }
        | .clear => walk_stage cx { w with s := x.s } clocks open_after
    else walk_stage cx w clocks open_after

/-- One stage of the walk, from the ticket's stages and effects so far (`walk`) -/
def lap (cx : Cx) (s : Step) (x : Lap) (clocks : String → Int → Int → Int → Except Refusal Clocks) (open_after : String → Int → Int → Except Refusal Int) : Lap :=
  if x.over then x
  else
    let w := walk cx { s := { s with t := { s.t with stages := x.stages, stage := x.stage }, fx := x.fx }, at_ := x.at_, over := false, end_ := false, stop := none } clocks open_after
    { stages := w.s.t.stages, stage := w.s.t.stage, fx := w.s.fx, at_ := w.at_, over := w.over, end_ := w.end_, stop := w.stop }

-- ── the `on` lines, as far as `finish` calls them (`on_ending`) ────────────

def closes (l : OnLine) : Bool :=
  match l.action with
  | .post | .release => true
  | _ => false

/-- The holds without the one at `k` -/
def without (holds : List Held) (k : Int) : List Held :=
  ((indexed holds).filter (fun x => x.index != k)).map (fun x => x.item)

/-- One `on` line: a post or a void is given only while the hold of the round is open, and nothing is asked of it otherwise -/
def on_line (t : Ticket) (b : Booked) (l : OnLine) : Booked :=
  match b.stop with
  | some e => b
  | none =>
    let open_ := first? (indexed b.holds) (fun h => h.item.book == l.book && h.item.transfer == l.transfer && h.item.round == t.round)
    if closes l && !present open_ then b
    else
      match l.when_ with
      | .error e => { b with stop := some e }
      | .ok yes =>
        if yes then
          match l.args with
          | .error e => { b with stop := some e }
          | .ok args =>
            match l.action with
            | .launch =>
              let w : What := .launch l.book args
              { b with made := b.made ++ [w] }
            | .hold =>
              let w : What := .hold l.book l.transfer args
              { b with holds := b.holds ++ [{ book := l.book, transfer := l.transfer, round := t.round }], made := b.made ++ [w] }
            | .post =>
              match open_ with
              | none => b
              | some h =>
                let w : What := .post l.book l.transfer args
                { b with holds := without b.holds h.index, made := b.made ++ [w] }
            | .release =>
              match open_ with
              | none => b
              | some h =>
                let w : What := .release l.book l.transfer args
                { b with holds := without b.holds h.index, made := b.made ++ [w] }
        else b

-- `on` is the request's `on` lines of the form the ticket holds: `start`, `act` and a resubmission make
-- it of the form they hold, so that the lines read that form as the glue gave it however deep the
-- calls that come to an ending go, past the four calls kaname reads a generic function again in.
/-- What the ticket does at the ending: each `on` line of it whose condition holds, in the order written -/
def on_ending (cx : Cx) (s : Step) (ending : Ending) (on : Ending → Ref → Org → Except Refusal (List OnLine)) : Except Refusal Step := do
  let lines ← on ending { id := s.t.id, round := s.t.round, attempt := s.t.attempt, requester := s.t.requester, made_by := s.t.made_by } cx.org
  let most : Int := 9223372036854775807
  let b := lines.foldl (fun (x : Booked) l => on_line s.t { x with holds := takeI x.holds most, made := takeI x.made most } l) { holds := s.t.holds, made := [], stop := none }
  match b.stop with
  | some e => throw e
  | none => pure (put_all { s with t := { s.t with holds := takeI b.holds most } } (takeI b.made most))

-- ── coming to a stage and moving on: the end of the walk ───────────────────

/-- No stage is left: the ticket is approved -/
def finish (cx : Cx) (s : Step) (on : Ending → Ref → Org → Except Refusal (List OnLine)) : Except Refusal Step := do
  let t := { s.t with stage := none, status := .approved, ended := some { at_ := cx.now, stage := none, by_ := none } }
  let s1 := note_down { s with t := t } (rec_of "ticket approved")
  let s2 ← on_ending cx s1 .approved on
  pure (put s2 (.notify (theirs (who_of s2.t)) .decided none))

/-- The ticket moves on from the stage `first`: the first stage at or after it that waits for someone is started, the ones before it that nobody answers, that circulate and that another stage's approval does are passed; with none left, the ticket is approved. `again` starts `first` as a stage the ticket was sent back to -/
def advance (cx : Cx) (s : Step) (first : Int) (again : Bool) (clocks : String → Int → Int → Int → Except Refusal Clocks) (open_after : String → Int → Int → Except Refusal Int) (on : Ending → Ref → Org → Except Refusal (List OnLine)) : Except Refusal Step := do
  let t0 := { s.t with status := .in_progress }
  let t1 :=
    if again then
      match at? t0.stages first with
      | none => t0
      | some st => with_stage t0 first { st with again := true }
    else t0
  let most : Int := 9223372036854775807
  let s0 := { s with t := t1, fx := [] }
  -- Each lap passes one stage or ends the walk. A stage comes again (`come_again`) for a part that
  -- waits, which is decided then, so the walk ends within as many laps as the stages and the parts,
  -- and the refusal below is not reached.
  let laps := min (t1.stages.length : Int) 1000000 + min ((t1.stages.flatMap (fun y => y.parts)).length : Int) 1000000 + 2
  let x := (numbers 1 laps).foldl
    (fun (y : Lap) k => lap cx s0 { y with stages := takeI y.stages most, fx := takeI y.fx most, at_ := max (min y.at_ 1000000) 0 } clocks open_after)
    { stages := t1.stages, stage := t1.stage, fx := [], at_ := first, over := false, end_ := false, stop := none }
  let s1 := { s with t := { t1 with stages := takeI x.stages most, stage := x.stage }, fx := s.fx ++ takeI x.fx most }
  match x.stop with
  | some e => throw e
  | none =>
    if x.end_ then finish cx s1 on
    else
      if x.over then pure s1
      else throw (refusal .unread "the route has more stages than one move walks" "経路のステップが、一度に進める数を超えます")

end RingiCore
