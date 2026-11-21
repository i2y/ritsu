/-
  The JSON of the contract (ringi's DESIGN 10.4, 12.4), read into the model's types and written
  back out: a ticket, an event, the part of the organization, what a request is made with, what is
  to do and a refusal, as the vectors hold them; and what the test gives the model of the request
  (the data of its glue) and the answers it gives to what the model asks.

  Nothing here decides anything: it is the shape of the lines the comparison passes.
-/
import Lean.Data.Json
import RingiCore.Records

open Lean

namespace RingiCore.Json

abbrev R := Except String

def field (j : Json) (k : String) : R Json := j.getObjVal? k

/-- A key that may be missing: none for a missing key and for `null`. -/
def opt? (j : Json) (k : String) : Option Json :=
  match j.getObjVal? k with
  | .ok .null => none
  | .ok v => some v
  | .error _ => none

def str (j : Json) (k : String) : R String := do (← field j k).getStr?
def int (j : Json) (k : String) : R Int := do (← field j k).getInt?
def bool (j : Json) (k : String) : R Bool := do (← field j k).getBool?
def arr (j : Json) (k : String) : R (List Json) := do return (← (← field j k).getArr?).toList

def optStr (j : Json) (k : String) : R (Option String) :=
  match opt? j k with
  | none => pure none
  | some v => do return some (← v.getStr?)

def optInt (j : Json) (k : String) : R (Option Int) :=
  match opt? j k with
  | none => pure none
  | some v => do return some (← v.getInt?)

def strs (j : Json) (k : String) : R (List String) := do
  (← arr j k).mapM (·.getStr?)

def dateOf (v : Json) : R Int := do
  let s ← v.getStr?
  match parseDate s with
  | some d => pure d
  | none => throw s!"not a day: {s}"

def date (j : Json) (k : String) : R Int := do dateOf (← field j k)

def optDate (j : Json) (k : String) : R (Option Int) :=
  match opt? j k with
  | none => pure none
  | some v => do return some (← dateOf v)

/-- The one key of an object that is an enum's value with fields, and what it holds. -/
def tagged (j : Json) : R (String × Json) :=
  match j with
  | .str s => pure (s, Json.null)
  | .obj kvs =>
    match kvs.toArray.toList with
    | [⟨k, v⟩] => pure (k, v)
    | _ => throw s!"not one key: {j.compress}"
  | _ => throw s!"not an enum's value: {j.compress}"

/-! Reading. -/

def readStatus : String → R Status
  | "in_progress" => pure .in_progress | "paused" => pure .paused
  | "with_requester" => pure .with_requester | "approved" => pure .approved
  | "rejected" => pure .rejected | "withdrawn" => pure .withdrawn | "expired" => pure .expired
  | s => throw s!"status {s}"

def readPhase : String → R Phase
  | "ahead" => pure .ahead | "active" => pure .active | "answered" => pure .answered
  | "skipped" => pure .skipped | "circulated" => pure .circulated | "noted" => pure .noted
  | "once" => pure .once | "deferred" => pure .deferred
  | s => throw s!"phase {s}"

def readPartState : String → R PartState
  | "active" => pure .active | "passed" => pure .passed | "skipped" => pure .skipped
  | "stopped" => pure .stopped | "agent" => pure .agent | "checked" => pure .checked
  | "once" => pure .once | "deferred" => pure .deferred
  | s => throw s!"part state {s}"

def readKind : String → R Kind
  | "state" => pure .state | "person" => pure .person | "submit" => pure .submit
  | "form" => pure .form | "comment" => pure .comment | "limit" => pure .limit
  | "clock" => pure .clock | "stale" => pure .stale | "event" => pure .event
  | "data" => pure .data | "book" => pure .book | "unread" => pure .unread
  | s => throw s!"kind {s}"

def readSole (j : Json) : R Sole := do
  match ← tagged j with
  | ("passing", _) => pure .passing
  | ("other", _) => pure .other
  | ("by", v) => return .by_ (← str v "person")
  | (s, _) => throw s!"sole {s}"

def readAnswer (j : Json) : R Answer := do
  return { by_ := ← str j "by", seat := ← str j "seat", yes := ← bool j "yes", at_ := ← int j "at" }

def readPart (j : Json) : R Part := do
  return { way := ← str j "way", key := ← str j "key", approvers := ← strs j "approvers",
           team := ← optStr j "team", needed := ← int j "needed",
           answers := ← (← arr j "answers").mapM readAnswer,
           state := ← readPartState (← str j "state"), of := ← optStr j "of",
           sole := ← readSole (← field j "sole") }

def readStage (j : Json) : R Stage := do
  return { name := ← str j "name", value := ← str j "value", phase := ← readPhase (← str j "phase"),
           started := ← optInt j "started", held := ← int j "held", remind := ← optInt j "remind",
           reminded := ← bool j "reminded", due := ← optInt j "due", up := ← int j "up",
           org := ← optDate j "org", again := ← bool j "again",
           parts := ← (← arr j "parts").mapM readPart }

def readHeld (j : Json) : R Held := do
  return { book := ← str j "book", transfer := ← str j "transfer", round := ← int j "round" }

def readEnded (j : Json) : R Ended := do
  return { at_ := ← int j "at", stage := ← optStr j "stage", by_ := ← optStr j "by" }

/-- A value a flow or a book is given; rows within rows as deep as `fuel` reaches. -/
def readVal : Nat → Json → R Val
  | 0, _ => throw "values nested too deep"
  | fuel + 1, j => do
    match ← tagged j with
    | ("nothing", _) => pure .nothing
    | ("str", v) => return .str (← str v "v")
    | ("num", v) => return .num (← int v "v")
    | ("truth", v) => return .truth (← bool v "v")
    | ("rows", v) =>
      let rows ← arr v "v"
      let read ← rows.mapM fun r => do
        let cells ← r.getArr?
        cells.toList.mapM fun c => do
          return Arg.mk (← str c "name") (← readVal fuel (← field c "value"))
      return .rows read
    | (s, _) => throw s!"value {s}"

def readArg (j : Json) : R Arg := do
  return Arg.mk (← str j "name") (← readVal 8 (← field j "value"))

def readArgs (j : Json) : R (List Arg) := do (← j.getArr?).toList.mapM readArg

def readNote (j : Json) : R Note := do
  return { stage := ← str j "stage", outputs := ← readArgs (← field j "outputs") }

def readRevision (j : Json) : R Revision := do
  return { round := ← int j "round", stage := ← optInt j "stage", edits := ← int j "edits" }

def optRevision (j : Json) : R (Option Revision) :=
  match opt? j "revision" with
  | none => pure none
  | some v => do return some (← readRevision v)

def readTicket (j : Json) : R Ticket := do
  return { id := ← str j "id", request := ← str j "request", version := ← int j "version",
           requester := ← str j "requester", made_by := ← optStr j "made_by",
           sent_back_by := ← optStr j "sent_back_by", status := ← readStatus (← str j "status"),
           round := ← int j "round", attempt := ← str j "attempt", serial := ← int j "serial",
           revision := ← readRevision (← field j "revision"), form := ← field j "form",
           stage := ← optStr j "stage", stages := ← (← arr j "stages").mapM readStage,
           backs := ← int j "backs", edits := ← int j "edits", pause_left := ← int j "pause_left",
           paused_at := ← optInt j "paused_at", pause_ends := ← optInt j "pause_ends",
           resubmit_due := ← optInt j "resubmit_due", holds := ← (← arr j "holds").mapM readHeld,
           notes := ← (← arr j "notes").mapM readNote,
           ended := ← match opt? j "ended" with
             | none => pure none
             | some v => (some <$> readEnded v) }

def readTeam (j : Json) : R Team := do
  return { id := ← str j "id", kind := ← str j "kind", parent := ← optStr j "parent", head := ← optStr j "head" }

def readMember (j : Json) : R Member := do
  return { person := ← str j "person", team := ← str j "team", position := ← str j "position",
           primary := ← bool j "primary" }

def readProxy (j : Json) : R Proxy := do
  let kind ← match ← str j "kind" with
    | "approve" => pure ProxyKind.approve
    | "submit" => pure ProxyKind.submit
    | s => throw s!"proxy kind {s}"
  return { person := ← str j "person", proxy := ← str j "proxy", kind := kind }

def readRow (j : Json) : R Row := do
  let cells ← (← arr j "values").mapM fun c => do
    return ({ name := ← str c "name", value := ← str c "value" } : Cell)
  return { master := ← str j "master", key := ← str j "key", values := cells }

def readPerson (j : Json) : R Person := do
  return { id := ← str j "id", name := ← str j "name", name_ja := ← str j "name_ja" }

def readOrg (j : Json) : R Org := do
  return { day := ← date j "day", version := ← date j "version", positions := ← strs j "positions",
           teams := ← (← arr j "teams").mapM readTeam, people := ← (← arr j "people").mapM readPerson,
           members := ← (← arr j "members").mapM readMember,
           proxies := ← (← arr j "proxies").mapM readProxy, rows := ← (← arr j "rows").mapM readRow }

def readBy (v : Json) : R ByEv := do
  return { by_ := ← str v "by", revision := ← optRevision v, comment := ← optStr v "comment" }

def optStrs (j : Json) (k : String) : R (Option (List String)) :=
  match opt? j k with
  | none => pure none
  | some v => do return some (← (← v.getArr?).toList.mapM (·.getStr?))

def readEvent (j : Json) : R Event := do
  match ← tagged j with
  | ("approve", v) => return .approve (← readBy v)
  | ("reject", v) => return .reject (← readBy v)
  | ("retract", v) => return .retract (← readBy v)
  | ("pause", v) => return .pause (← readBy v)
  | ("resume", v) => return .resume (← readBy v)
  | ("withdraw", v) => return .withdraw (← readBy v)
  | ("discard", v) => return .discard (← readBy v)
  | ("send_back", v) =>
    return .send_back { by_ := ← str v "by", to := ← optStr v "to", revision := ← optRevision v,
                        comment := ← optStr v "comment" }
  | ("resubmit", v) =>
    return .resubmit { by_ := ← str v "by", form := ← field v "form", clear := ← optStrs v "clear",
                       revision := ← optRevision v, comment := ← optStr v "comment",
                       attempt := ← str v "attempt" }
  | ("edit", v) =>
    return .edit { by_ := ← str v "by", fields := ← field v "fields", clear := ← optStrs v "clear",
                   revision := ← optRevision v, comment := ← optStr v "comment" }
  | ("add_approver", v) =>
    return .add_approver { by_ := ← optStr v "by", approver := ← str v "approver",
                           part := ← optStr v "part", revision := ← optRevision v,
                           comment := ← optStr v "comment" }
  | ("hand_over", v) =>
    return .hand_over { by_ := ← optStr v "by", of := ← str v "of", to := ← str v "to",
                        revision := ← optRevision v, comment := ← optStr v "comment" }
  | ("comment", v) => return .comment { by_ := ← str v "by", comment := ← optStr v "comment" }
  | ("remind", _) => pure .remind
  | ("late", _) => pure .late
  | ("resubmit_late", _) => pure .resubmit_late
  | ("pause_ends", _) => pure .pause_ends
  | ("agent_answered", v) =>
    let verdict ← match ← str v "verdict" with
      | "ok" => pure Verdict.ok
      | "doubt" => pure Verdict.doubt
      | s => throw s!"verdict {s}"
    return .agent_answered { verdict := verdict, reason := ← str v "reason", revision := ← optRevision v }
  | ("note_ready", v) =>
    return .note_ready { outputs := ← readArgs (← field v "outputs"), revision := ← optRevision v }
  | ("flow_failed", v) => return .flow_failed (← optRevision v)
  | (s, _) => throw s!"event {s}"

def readSubmit (v : Json) : R Submit := do
  return { id := ← str v "id", requester := ← str v "requester", by_ := ← optStr v "by",
           form := ← field v "form", comment := ← optStr v "comment", attempt := ← str v "attempt" }

def readRefusal (j : Json) : R Refusal := do
  return { refused := ← readKind (← str j "refused"), text := ← str j "text", text_ja := ← str j "text_ja",
           at_ := ← optStr j "at" }

/-- `{"ok": x}` or `{"err": refusal}`. -/
def readResult {α} (read : Json → R α) (j : Json) : R (Except Refusal α) := do
  match opt? j "ok", opt? j "err" with
  | _, some e => return .error (← readRefusal e)
  | some v, _ => return .ok (← read v)
  | none, none =>
    -- an `ok` of null: an answer that is none
    match j.getObjVal? "ok" with
    | .ok v => return .ok (← read v)
    | .error _ => throw s!"neither ok nor err: {j.compress}"

/-! The request, as its glue gives it. -/

def readHow (j : Json) : R How := do
  match ← tagged j with
  | ("manager_of", _) => pure .manager_of
  | ("head_of", v) => return .head_of (← str v "team_kind")
  | ("nearest", v) => return .nearest (← str v "position")
  | ("team", v) => return .team (← str v "id")
  | ("person", v) => return .person (← str v "id")
  | ("lookup", v) =>
    return .lookup { master := ← str v "master", field := ← str v "field", by_ := ← str v "by",
                     shown := ← str v "shown", column := ← str v "column", by_name := ← str v "by_name" }
  | ("chosen_by", v) =>
    return .chosen_by { field := ← str v "field", shown := ← str v "shown", previous := ← bool v "previous" }
  | ("agent", v) => return .agent { flow := ← str v "flow", otherwise := ← str v "otherwise" }
  | (s, _) => throw s!"how {s}"

def readQuorum (j : Json) : R Quorum := do
  match ← tagged j with
  | ("all", _) => pure .all
  | ("at_least", v) => return .at_least (← int v "n")
  | ("percent", v) => return .percent (← int v "p")
  | (s, _) => throw s!"quorum {s}"

def readShape (j : Json) : R Shape := do
  match ← tagged j with
  | ("who", _) => pure .who
  | ("quorum", v) => return .quorum (← readQuorum (← field v "q"))
  | ("rule_output", _) => pure .rule_output
  | ("circulate", v) => return .circulate (← str v "way")
  | ("note", v) => return .note (← str v "flow")
  | (s, _) => throw s!"shape {s}"

def readAtTop (j : Json) : R AtTop := do
  match ← tagged j with
  | ("stay", _) => pure .stay
  | ("skip", _) => pure .skip
  | ("to", v) => return .to (← str v "way")
  | (s, _) => throw s!"at top {s}"

def readEmpty (j : Json) : R WhenEmpty := do
  match ← tagged j with
  | ("stay", _) => pure .stay
  | ("skip", _) => pure .skip
  | ("escalate", v) =>
    match opt? v "top" with
    | none => pure (.escalate none)
    | some t => return .escalate (some (← readAtTop t))
  | (s, _) => throw s!"when empty {s}"

def readLateAct (j : Json) : R LateAct := do
  match ← tagged j with
  | ("expire", _) => pure .expire
  | ("stay", _) => pure .stay
  | ("escalate", v) =>
    let sends ← (← arr v "sends").mapM fun s => do
      return ({ key := ← str s "key", to := ← strs s "to" } : Send)
    return .escalate { table := ← bool v "table", sends := sends }
  | (s, _) => throw s!"late {s}"

def readOcc (j : Json) : R Occ := do
  let late ← match opt? j "late" with
    | none => pure none
    | some l => do
      let after ← match opt? l "after" with
        | none => pure none
        | some a => (some <$> readLateAct a)
      let first ← readLateAct (← field l "first")
      pure (some ({ first := first, after := after } : LateLine))
  return { stage := ← str j "stage", value := ← str j "value", shape := ← readShape (← field j "shape"),
           ways := ← strs j "ways", empty := ← readEmpty (← field j "empty"), late := late }

def readSpec (j : Json) : R Spec := do
  let once ← match ← str j "once" with
    | "off" => pure Once.off | "before" => pure Once.before | "last" => pure Once.last
    | s => throw s!"once {s}"
  let back ← match opt? j "back" with
    | none => pure none
    | some b => do
      let tr ← bool b "to_requester"
      let n ← int b "times"
      pure (some ({ to_requester := tr, times := n } : Back))
  let withdraw ← match opt? j "withdraw" with
    | none => pure none
    | some w => do
      let u ← optStr w "until"
      pure (some ({ until_ := u } : Withdraw))
  return { request := ← str j "request", version := ← int j "version", offset := ← int j "offset",
           usable_from := ← optDate j "usable_from", usable_to := ← optDate j "usable_to",
           self_approval := ← bool j "self_approval", once := once, pause := ← optInt j "pause",
           back := back, resubmit := ← optInt j "resubmit", withdraw := withdraw,
           retract := ← bool j "retract", order := ← strs j "order",
           ways := ← (← arr j "ways").mapM fun w => do
             return ({ name := ← str w "name", written := ← str w "written", how := ← readHow (← field w "how") } : Way),
           occs := ← (← arr j "occs").mapM readOcc,
           flows := ← (← arr j "flows").mapM fun f => do
             return ({ name := ← str f "name", shown := ← str f "shown" } : Flow),
           named := ← (← arr j "named").mapM fun n => do
             return ({ name := ← str n "name", written := ← str n "written" } : StageName),
           offsets := ← (← arr j "offsets").mapM fun o => do
             return ({ timer := ← str o "timer", stage := ← optStr o "stage", offset := ← int o "offset" } : ClockOffset),
           comment_required := ← strs j "comment_required" }

/-! Writing. -/

def jstr (s : String) : Json := .str s
def jint (n : Int) : Json := toJson n
def jopt {α} (f : α → Json) : Option α → Json
  | none => .null
  | some x => f x
def jlist {α} (f : α → Json) (l : List α) : Json := .arr (l.map f).toArray
def jdate (d : Int) : Json := .str (dateText d)
def obj (kvs : List (String × Json)) : Json := Json.mkObj kvs
def tag (k : String) (v : Json) : Json := obj [(k, v)]

def statusWord : Status → String
  | .in_progress => "in_progress" | .paused => "paused" | .with_requester => "with_requester"
  | .approved => "approved" | .rejected => "rejected" | .withdrawn => "withdrawn" | .expired => "expired"

def phaseWord : Phase → String
  | .ahead => "ahead" | .active => "active" | .answered => "answered" | .skipped => "skipped"
  | .circulated => "circulated" | .noted => "noted" | .once => "once" | .deferred => "deferred"

def partStateWord : PartState → String
  | .active => "active" | .passed => "passed" | .skipped => "skipped" | .stopped => "stopped"
  | .agent => "agent" | .checked => "checked" | .once => "once" | .deferred => "deferred"

def kindWord : Kind → String
  | .state => "state" | .person => "person" | .submit => "submit" | .form => "form"
  | .comment => "comment" | .limit => "limit" | .clock => "clock" | .stale => "stale"
  | .event => "event" | .data => "data" | .book => "book" | .unread => "unread"

def timerWord : Timer → String
  | .remind => "remind" | .late => "late" | .resubmit_late => "resubmit_late" | .pause_ends => "pause_ends"

def noticeWord : Notice → String
  | .waiting => "waiting" | .reminder => "reminder" | .went_up => "went_up" | .returned => "returned"
  | .decided => "decided" | .circulation => "circulation" | .paused => "paused" | .resumed => "resumed"
  | .stopped => "stopped" | .comment => "comment"

def endingWord : Ending → String
  | .submitted => "submitted" | .approved => "approved" | .sent_back => "sent_back"
  | .rejected => "rejected" | .withdrawn => "withdrawn" | .expired => "expired"

def writeSole : Sole → Json
  | .passing => jstr "passing"
  | .other => jstr "other"
  | .by_ p => tag "by" (obj [("person", jstr p)])

def writeAnswer (a : Answer) : Json :=
  obj [("by", jstr a.by_), ("seat", jstr a.seat), ("yes", .bool a.yes), ("at", jint a.at_)]

def writePart (p : Part) : Json :=
  obj [("way", jstr p.way), ("key", jstr p.key), ("approvers", jlist jstr p.approvers),
       ("team", jopt jstr p.team), ("needed", jint p.needed), ("answers", jlist writeAnswer p.answers),
       ("state", jstr (partStateWord p.state)), ("of", jopt jstr p.of), ("sole", writeSole p.sole)]

def writeStage (s : Stage) : Json :=
  obj [("name", jstr s.name), ("value", jstr s.value), ("phase", jstr (phaseWord s.phase)),
       ("started", jopt jint s.started), ("held", jint s.held), ("remind", jopt jint s.remind),
       ("reminded", .bool s.reminded), ("due", jopt jint s.due), ("up", jint s.up),
       ("org", jopt jdate s.org), ("again", .bool s.again), ("parts", jlist writePart s.parts)]

/-- A value as the JSON holds it; rows within rows as deep as `fuel` reaches. -/
def writeValF : Nat → Val → Json
  | 0, _ => jstr "nothing"
  | _ + 1, .nothing => jstr "nothing"
  | _ + 1, .str v => tag "str" (obj [("v", jstr v)])
  | _ + 1, .num v => tag "num" (obj [("v", jint v)])
  | _ + 1, .truth v => tag "truth" (obj [("v", .bool v)])
  | f + 1, .rows v =>
    tag "rows" (obj [("v", .arr (v.map fun r => .arr (r.map fun a => obj [("name", jstr a.name), ("value", writeValF f a.value)]).toArray).toArray)])

def writeVal (v : Val) : Json := writeValF 8 v

def writeArg (a : Arg) : Json := obj [("name", jstr a.name), ("value", writeVal a.value)]

def writeRevision (r : Revision) : Json :=
  obj [("round", jint r.round), ("stage", jopt jint r.stage), ("edits", jint r.edits)]

def writeTicket (t : Ticket) : Json :=
  obj [("id", jstr t.id), ("request", jstr t.request), ("version", jint t.version),
       ("requester", jstr t.requester), ("made_by", jopt jstr t.made_by),
       ("sent_back_by", jopt jstr t.sent_back_by), ("status", jstr (statusWord t.status)),
       ("round", jint t.round), ("attempt", jstr t.attempt), ("serial", jint t.serial),
       ("revision", writeRevision t.revision), ("form", t.form), ("stage", jopt jstr t.stage),
       ("stages", jlist writeStage t.stages), ("backs", jint t.backs), ("edits", jint t.edits),
       ("pause_left", jint t.pause_left), ("paused_at", jopt jint t.paused_at),
       ("pause_ends", jopt jint t.pause_ends), ("resubmit_due", jopt jint t.resubmit_due),
       ("holds", jlist (fun h => obj [("book", jstr h.book), ("transfer", jstr h.transfer), ("round", jint h.round)]) t.holds),
       ("notes", jlist (fun n => obj [("stage", jstr n.stage), ("outputs", jlist writeArg n.outputs)]) t.notes),
       ("ended", jopt (fun e => obj [("at", jint e.at_), ("stage", jopt jstr e.stage), ("by", jopt jstr e.by_)]) t.ended)]

def writeRec (r : Rec) : Json :=
  obj [("did", jstr r.did), ("stage", jopt jstr r.stage), ("by", jopt jstr r.by_),
       ("proxy_of", jopt jstr r.proxy_of), ("organization", jopt jdate r.organization),
       ("once_with", jopt jstr r.once_with), ("to", jlist jstr r.to), ("note", jopt jstr r.note),
       ("comment", jopt jstr r.comment), ("cleared", jlist jstr r.cleared)]

def writeWhat : What → Json
  | .history r => tag "history" (writeRec r)
  | .cancel_timer t s => tag "cancel_timer" (obj [("timer", jstr (timerWord t)), ("stage", jopt jstr s)])
  | .post b t a => tag "post" (obj [("book", jstr b), ("transfer", jstr t), ("args", jlist writeArg a)])
  | .release b t a => tag "release" (obj [("book", jstr b), ("transfer", jstr t), ("args", jlist writeArg a)])
  | .hold b t a => tag "hold" (obj [("book", jstr b), ("transfer", jstr t), ("args", jlist writeArg a)])
  | .launch f a => tag "launch" (obj [("flow", jstr f), ("args", jlist writeArg a)])
  | .ask_agent f s a => tag "ask_agent" (obj [("flow", jstr f), ("stage", jstr s), ("args", jlist writeArg a)])
  | .ask_note f s a => tag "ask_note" (obj [("flow", jstr f), ("stage", jstr s), ("args", jlist writeArg a)])
  | .notify to n s => tag "notify" (obj [("to", jlist jstr to), ("notice", jstr (noticeWord n)), ("stage", jopt jstr s)])
  | .set_timer t s a => tag "set_timer" (obj [("timer", jstr (timerWord t)), ("stage", jopt jstr s), ("at", jint a)])

def writeEffect (e : Effect) : Json :=
  obj [("key", obj [("ticket", jstr e.key.ticket), ("round", jint e.key.round), ("serial", jint e.key.serial)]),
       ("what", writeWhat e.what)]

def writeRefusal (r : Refusal) : Json :=
  obj [("refused", jstr (kindWord r.refused)), ("text", jstr r.text), ("text_ja", jstr r.text_ja), ("at", jopt jstr r.at_)]

def writeMoved (r : Except Refusal Moved) : Json :=
  match r with
  | .ok m => tag "ok" (obj [("ticket", writeTicket m.ticket), ("effects", jlist writeEffect m.effects)])
  | .error e => tag "err" (writeRefusal e)

/-! What the model is given back: the answers to what it asked. -/

def readClocks (j : Json) : R Clocks := do
  return { remind := ← optInt j "remind", due := ← optInt j "due" }

def readAction : String → R Action
  | "hold" => pure .hold | "post" => pure .post | "release" => pure .release | "launch" => pure .launch
  | s => throw s!"action {s}"

def readOnLine (j : Json) : R OnLine := do
  return { action := ← readAction (← str j "action"), book := ← str j "book", transfer := ← str j "transfer",
           when_ := ← readResult (·.getBool?) (← field j "when"),
           args := ← readResult readArgs (← field j "args") }

def readFormed (j : Json) : R Formed := do
  return { form := ← field j "form", stages := ← (← arr j "stages").mapM readStage,
           vals := ← readArgs (← field j "vals"), named := ← strs j "named", cleared := ← strs j "cleared" }

end RingiCore.Json
