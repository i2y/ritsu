/-
  The ticket machine of ringi (ringi's DESIGN 7): what a ticket, an event, the part of the
  organization, what is to do and a refusal are. The shapes are the ones the machine's code writes
  (`ticket.kn`, the module the generated code of every request runs), and the JSON of the contract
  (ringi's DESIGN 10.4, 12.4) is these written out.

  A number is an `Int`; a day is an `Int` too, the days since 1970-01-01. The form of a ticket and
  the values an event gives are kept as the JSON they come as (`Form`): the machine does not read
  them itself, it asks what the request says of them (`Oracle`, in `RingiCore.Machine`).
-/
import Lean.Data.Json

namespace RingiCore

/-- The values of a form, as the JSON of the contract holds them. -/
abbrev Form := Lean.Json

inductive Status where
  | in_progress | paused | with_requester | approved | rejected | withdrawn | expired
  deriving DecidableEq, Repr, Inhabited

inductive Phase where
  | ahead | active | answered | skipped | circulated | noted | once | deferred
  deriving DecidableEq, Repr, Inhabited

inductive PartState where
  | active | passed | skipped | stopped | agent | checked | once | deferred
  deriving DecidableEq, Repr, Inhabited

/-- Who alone answers a part, as `same approver approves once` reads it. -/
inductive Sole where
  | passing
  | by_ (person : String)
  | other
  deriving DecidableEq, Repr, Inhabited

structure Answer where
  by_ : String
  seat : String
  yes : Bool
  at_ : Int
  deriving DecidableEq, Repr, Inhabited

structure Part where
  way : String
  key : String
  approvers : List String
  team : Option String
  needed : Int
  answers : List Answer
  state : PartState
  of : Option String
  sole : Sole
  deriving DecidableEq, Repr, Inhabited

structure Stage where
  name : String
  value : String
  phase : Phase
  started : Option Int
  held : Int
  remind : Option Int
  reminded : Bool
  due : Option Int
  up : Int
  org : Option Int
  again : Bool
  parts : List Part
  deriving DecidableEq, Repr, Inhabited

structure Held where
  book : String
  transfer : String
  round : Int
  deriving DecidableEq, Repr, Inhabited

structure Ended where
  at_ : Int
  stage : Option String
  by_ : Option String
  deriving DecidableEq, Repr, Inhabited

mutual
  /-- A value a flow or a book is given. -/
  inductive Val where
    | str (v : String)
    | num (v : Int)
    | truth (v : Bool)
    | rows (v : List (List Arg))
    | nothing
  /-- A named value. -/
  inductive Arg where
    | mk (name : String) (value : Val)
end

instance : Inhabited Val := ⟨.nothing⟩
instance : Inhabited Arg := ⟨.mk "" .nothing⟩

def Arg.name : Arg → String
  | .mk n _ => n

def Arg.value : Arg → Val
  | .mk _ v => v

structure Note where
  stage : String
  outputs : List Arg
  deriving Inhabited

/-- The revision of a ticket: its round, the place in its stages of the stage it is at (none while
    it is with the requester and once it has ended), and the approvers' edits that went through. -/
structure Revision where
  round : Int
  stage : Option Int
  edits : Int
  deriving DecidableEq, Repr, Inhabited

structure Ticket where
  id : String
  request : String
  version : Int
  requester : String
  made_by : Option String
  sent_back_by : Option String
  status : Status
  round : Int
  attempt : String
  serial : Int
  revision : Revision
  form : Form
  stage : Option String
  stages : List Stage
  backs : Int
  edits : Int
  pause_left : Int
  paused_at : Option Int
  pause_ends : Option Int
  resubmit_due : Option Int
  holds : List Held
  notes : List Note
  ended : Option Ended
  deriving Inhabited

/-! The part of the organization an event comes with. -/

structure Team where
  id : String
  kind : String
  parent : Option String
  head : Option String
  deriving DecidableEq, Repr, Inhabited

structure Member where
  person : String
  team : String
  position : String
  primary : Bool
  deriving DecidableEq, Repr, Inhabited

inductive ProxyKind where
  | approve | submit
  deriving DecidableEq, Repr, Inhabited

structure Proxy where
  person : String
  proxy : String
  kind : ProxyKind
  deriving DecidableEq, Repr, Inhabited

structure Cell where
  name : String
  value : String
  deriving DecidableEq, Repr, Inhabited

structure Row where
  master : String
  key : String
  values : List Cell
  deriving DecidableEq, Repr, Inhabited

structure Person where
  id : String
  name : String
  name_ja : String
  deriving DecidableEq, Repr, Inhabited

structure Org where
  day : Int
  version : Int
  positions : List String
  teams : List Team
  people : List Person
  members : List Member
  proxies : List Proxy
  rows : List Row
  deriving DecidableEq, Repr, Inhabited

/-! The events. -/

inductive Verdict where
  | ok | doubt
  deriving DecidableEq, Repr, Inhabited

/-- `approve`, `reject`, `retract`, `pause`, `resume`, `withdraw`, `discard`. -/
structure ByEv where
  by_ : String
  revision : Option Revision
  comment : Option String
  deriving Inhabited

structure SendBackEv where
  by_ : String
  to : Option String
  revision : Option Revision
  comment : Option String
  deriving Inhabited

structure ResubmitEv where
  by_ : String
  form : Form
  clear : Option (List String)
  revision : Option Revision
  comment : Option String
  attempt : String
  deriving Inhabited

structure EditEv where
  by_ : String
  fields : Form
  clear : Option (List String)
  revision : Option Revision
  comment : Option String
  deriving Inhabited

structure AddApproverEv where
  by_ : Option String
  approver : String
  part : Option String
  revision : Option Revision
  comment : Option String
  deriving Inhabited

structure HandOverEv where
  by_ : Option String
  of : String
  to : String
  revision : Option Revision
  comment : Option String
  deriving Inhabited

structure CommentEv where
  by_ : String
  comment : Option String
  deriving Inhabited

structure AgentEv where
  verdict : Verdict
  reason : String
  revision : Option Revision
  deriving Inhabited

structure NoteEv where
  outputs : List Arg
  revision : Option Revision
  deriving Inhabited

inductive Event where
  | resubmit (a : ResubmitEv)
  | approve (a : ByEv)
  | reject (a : ByEv)
  | send_back (a : SendBackEv)
  | retract (a : ByEv)
  | pause (a : ByEv)
  | resume (a : ByEv)
  | withdraw (a : ByEv)
  | discard (a : ByEv)
  | edit (a : EditEv)
  | add_approver (a : AddApproverEv)
  | hand_over (a : HandOverEv)
  | comment (a : CommentEv)
  | remind
  | late
  | resubmit_late
  | pause_ends
  | agent_answered (a : AgentEv)
  | note_ready (a : NoteEv)
  | flow_failed (revision : Option Revision)
  deriving Inhabited

/-- What a request is made with (`submit`). -/
structure Submit where
  id : String
  requester : String
  by_ : Option String
  form : Form
  comment : Option String
  attempt : String
  deriving Inhabited

/-! What is to do. -/

inductive Timer where
  | remind | late | resubmit_late | pause_ends
  deriving DecidableEq, Repr, Inhabited

inductive Notice where
  | waiting | reminder | went_up | returned | decided | circulation | paused | resumed | stopped | comment
  deriving DecidableEq, Repr, Inhabited

/-- A line of the history. -/
structure Rec where
  did : String
  stage : Option String
  by_ : Option String
  proxy_of : Option String
  organization : Option Int
  once_with : Option String
  to : List String
  note : Option String
  comment : Option String
  cleared : List String
  deriving DecidableEq, Repr, Inhabited

inductive What where
  | history (r : Rec)
  | cancel_timer (timer : Timer) (stage : Option String)
  | post (book : String) (transfer : String) (args : List Arg)
  | release (book : String) (transfer : String) (args : List Arg)
  | hold (book : String) (transfer : String) (args : List Arg)
  | launch (flow : String) (args : List Arg)
  | ask_agent (flow : String) (stage : String) (args : List Arg)
  | ask_note (flow : String) (stage : String) (args : List Arg)
  | notify (to : List String) (notice : Notice) (stage : Option String)
  | set_timer (timer : Timer) (stage : Option String) (at_ : Int)
  deriving Inhabited

structure Key where
  ticket : String
  round : Int
  serial : Int
  deriving DecidableEq, Repr, Inhabited

structure Effect where
  key : Key
  what : What
  deriving Inhabited

inductive Kind where
  | state | person | submit | form | comment | limit | clock | stale | event | data | book | unread
  deriving DecidableEq, Repr, Inhabited

structure Refusal where
  refused : Kind
  text : String
  text_ja : String
  at_ : Option String
  deriving Inhabited

def refusal (kind : Kind) (en ja : String) : Refusal :=
  { refused := kind, text := en, text_ja := ja, at_ := none }

def refusal_at (kind : Kind) (en ja at_ : String) : Refusal :=
  { refused := kind, text := en, text_ja := ja, at_ := some at_ }

structure Moved where
  ticket : Ticket
  effects : List Effect
  deriving Inhabited

/-! What the request says: data its glue gives the machine. -/

inductive Once where
  | off | before | last
  deriving DecidableEq, Repr, Inhabited

structure LookupHow where
  master : String
  field : String
  by_ : String
  shown : String
  column : String
  by_name : String
  deriving DecidableEq, Repr, Inhabited

structure ChosenByHow where
  field : String
  shown : String
  previous : Bool
  deriving DecidableEq, Repr, Inhabited

structure AgentHow where
  flow : String
  otherwise : String
  deriving DecidableEq, Repr, Inhabited

inductive How where
  | manager_of
  | head_of (team_kind : String)
  | nearest (position : String)
  | team (id : String)
  | person (id : String)
  | lookup (x : LookupHow)
  | chosen_by (x : ChosenByHow)
  | agent (x : AgentHow)
  deriving DecidableEq, Repr, Inhabited

structure Way where
  name : String
  written : String
  how : How
  deriving DecidableEq, Repr, Inhabited

inductive AtTop where
  | stay
  | skip
  | to (way : String)
  deriving DecidableEq, Repr, Inhabited

inductive WhenEmpty where
  | stay
  | skip
  | escalate (top : Option AtTop)
  deriving DecidableEq, Repr, Inhabited

inductive Quorum where
  | all
  | at_least (n : Int)
  | percent (p : Int)
  deriving DecidableEq, Repr, Inhabited

/-- What a stage of the route is: the ways it names (a quorum of one), a quorum, a rule's output
    that names its way, a circulation, or a summary a flow makes. -/
inductive Shape where
  | who
  | quorum (q : Quorum)
  | rule_output
  | circulate (way : String)
  | note (flow : String)
  deriving DecidableEq, Repr, Inhabited

/-- The ways a table of `when late escalate by <table>` sends a late stage of the way `key` to. -/
structure Send where
  key : String
  to : List String
  deriving DecidableEq, Repr, Inhabited

structure EscalateAct where
  table : Bool
  sends : List Send
  deriving DecidableEq, Repr, Inhabited

inductive LateAct where
  | expire
  | stay
  | escalate (e : EscalateAct)
  deriving DecidableEq, Repr, Inhabited

structure LateLine where
  first : LateAct
  after : Option LateAct
  deriving DecidableEq, Repr, Inhabited

structure Occ where
  stage : String
  value : String
  shape : Shape
  ways : List String
  empty : WhenEmpty
  late : Option LateLine
  deriving DecidableEq, Repr, Inhabited

structure Back where
  to_requester : Bool
  times : Int
  deriving DecidableEq, Repr, Inhabited

structure Withdraw where
  until_ : Option String
  deriving DecidableEq, Repr, Inhabited

structure Flow where
  name : String
  shown : String
  deriving DecidableEq, Repr, Inhabited

structure StageName where
  name : String
  written : String
  deriving DecidableEq, Repr, Inhabited

structure ClockOffset where
  timer : String
  stage : Option String
  offset : Int
  deriving DecidableEq, Repr, Inhabited

/-- What the machine is given of the request. -/
structure Spec where
  request : String
  version : Int
  offset : Int
  usable_from : Option Int
  usable_to : Option Int
  self_approval : Bool
  once : Once
  pause : Option Int
  back : Option Back
  resubmit : Option Int
  withdraw : Option Withdraw
  retract : Bool
  order : List String
  ways : List Way
  occs : List Occ
  flows : List Flow
  named : List StageName
  offsets : List ClockOffset
  comment_required : List String
  deriving DecidableEq, Repr, Inhabited

inductive Ending where
  | submitted | approved | sent_back | rejected | withdrawn | expired
  deriving DecidableEq, Repr, Inhabited

inductive Action where
  | hold | post | release | launch
  deriving DecidableEq, Repr, Inhabited

/-- An `on` line of the request, as the request reads it for a ticket: what it does, and its
    condition and arguments, each worked out or refused. -/
structure OnLine where
  action : Action
  book : String
  transfer : String
  when_ : Except Refusal Bool
  args : Except Refusal (List Arg)

structure Ref where
  id : String
  round : Int
  attempt : String
  requester : String
  made_by : Option String
  deriving DecidableEq, Repr, Inhabited

structure Clocks where
  remind : Option Int
  due : Option Int
  deriving DecidableEq, Repr, Inhabited

/-- The form of a ticket made again or edited: the form, the stages its route comes to, its values
    as a flow is given them, the fields the event gave, and the fields that lost their value. -/
structure Formed where
  form : Form
  stages : List Stage
  vals : List Arg
  named : List String
  cleared : List String
  deriving Inhabited

/-! A move in the making. -/

structure Cx where
  spec : Spec
  org : Org
  now : Int
  deriving Inhabited

structure Made where
  round : Int
  what : What
  deriving Inhabited

structure Step where
  t : Ticket
  fx : List Made
  vals : List Arg
  deriving Inhabited

end RingiCore
