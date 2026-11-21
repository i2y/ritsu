/-
  What joins a request to the machine (ringi's DESIGN 12.1, 12.2): who may submit it
  (`submitted by`), what the machine asks of the request's own code (`Oracle`), and the two doors
  a host calls, `start` for a request made and `next` for each event of a ticket, as the glue of
  every request writes them.

  `submitted by` is the request's data, and the model reads it here as the glue does. The rest of
  what the glue answers (the clocks of a stage, the business days, the conditions and arguments of
  the `on` lines, the form given again or edited, the form a request is made with and the stages
  its route comes to) is the request's rules, deadlines and calendars, which the other languages
  decide: the model is given them as functions, and the theorems hold whatever they answer.
-/
import RingiCore.Moves

namespace RingiCore

/-- A group of people a `submitted by` line names. -/
inductive Group where
  | everyone (within : Option String)
  | team (id : String)
  | position (position : String) (higher : Bool) (within : Option String)
  | person (id : String)
  deriving DecidableEq, Repr, Inhabited

/-- The `submitted by` line of a request: the groups that may submit it, the groups that may not,
    and the file and the line as the refusal names them. -/
structure Submitters where
  groups : List Group
  except_ : List Group
  file : String
  shown : String
  deriving DecidableEq, Repr, Inhabited

def in_group (org : Org) (person : String) : Group → Bool
  | .everyone none => true
  | .everyone (some t) => under org person t
  | .team t => member_of org person t
  | .position p h none => at_position org person p h
  | .position p h (some t) => at_position org person p h && under org person t
  | .person id => person == id

def any_group (org : Org) (person : String) (gs : List Group) : Bool :=
  gs.any (in_group org person)

/-- Whether the person may submit the request (`submitted by`); every person of the organization
    where the request has no such line. -/
def submitted_by (sub : Option Submitters) (org : Org) (person : String) : Except Refusal Bool :=
  match sub with
  | none => pure true
  | some l =>
    if in_org org person && any_group org person l.groups && !any_group org person l.except_ then pure true
    else throw (refusal .submit
      (who_en org person ++ s!" may not submit {l.file} in the organization from " ++ dateText org.version ++ s!" (`submitted by {l.shown}`)")
      (who_ja org person ++ "は、" ++ dateText org.version ++ s!" からの組織では {l.file} を出せません（`submitted by {l.shown}`）"))

/-- The form a request is made with, held to what the form declares, with the stages its route
    comes to and the values a flow is given. -/
structure Prepared where
  form : Form
  stages : List Stage
  vals : List Arg
  deriving Inhabited

/-- What the machine asks of the request's own code. -/
structure Oracle where
  clocks : String → Int → Int → Int → Except Refusal Clocks
  open_after : String → Int → Int → Except Refusal Int
  on : Ending → Ref → Org → Form → Except Refusal (List OnLine)
  reform : Form → Form → List String → Bool → Org → Except Refusal Formed
  prepare : Form → Org → Except Refusal Prepared
  vals : Form → Except Refusal (List Arg)

/-- A request is made: the ticket at its first stage with what is to do, or why it is not made.
    In the glue's order: the day, the days the request can be used on, the one who makes it in
    another's name, `submitted by`, the comment, the form, then the machine. -/
def glue_start (spec : Spec) (sub : Option Submitters) (o : Oracle) (submit : Submit) (org : Org)
    (now : Int) : Except Refusal Moved := do
  let day ← on_day org now spec.offset
  let _ ← usable_on spec day
  let made_by ← may_submit org submit.requester submit.by_
  let _ ← submitted_by sub org submit.requester
  let comment ← said submit.comment
  let p ← o.prepare submit.form org
  start spec submit.id submit.requester made_by p.form p.vals p.stages org now comment submit.attempt
    o.clocks o.open_after o.on

/-- One event of a ticket: the ticket after it with what is to do, or why the event is not taken. -/
def glue_next (spec : Spec) (o : Oracle) (t : Ticket) (ev : Event) (org : Org) (now : Int) :
    Except Refusal Moved := do
  let vals ← o.vals t.form
  next spec t ev org now vals o.clocks o.open_after o.on o.reform

end RingiCore
