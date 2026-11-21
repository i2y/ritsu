/-
  The records and enums the machine's code keeps to itself while it works a move out: how far up
  the organization a climb came, who a ticket is of, why nobody answers a way, what a stage is
  waiting for, a walk over the stages. Each as the machine's code declares it, in its order.
-/
import RingiCore.Prelude

namespace RingiCore

/-! The organization. -/

structure Climb where
  at_ : Option String
  teams : List Team
  deriving DecidableEq, Repr, Inhabited

structure Who where
  id : String
  requester : String
  made_by : Option String
  deriving DecidableEq, Repr, Inhabited

/-! Who answers. -/

inductive Why where
  | person_not_in (id : String)
  | team_not_in (id : String)
  | head_empty (team : String)
  | only_requester (id : String)
  | no_team_of_kind (team_kind : String)
  | none_above
  | no_members (team : String)
  deriving DecidableEq, Repr, Inhabited

inductive Up where
  | at_ (team : String)
  | above (team : String)
  | cannot
  deriving DecidableEq, Repr, Inhabited

inductive Solved where
  | people (ps : List String) (team : Option String)
  | nobody (why : String) (up : Up) (cannot : Bool)
  deriving DecidableEq, Repr, Inhabited

/-! What is made. -/

structure Chosen where
  field : String
  shown : String
  deriving DecidableEq, Repr, Inhabited

/-! The stages. -/

structure Wait where
  person : String
  to : String
  deriving DecidableEq, Repr, Inhabited

structure Place where
  stage : Int
  part : Int
  deriving DecidableEq, Repr, Inhabited

structure Change where
  stage : Int
  part : Int
  p : Part
  made : List What
  deriving Inhabited

structure Parted where
  part : Part
  made : List What
  deriving Inhabited

/-! `same approver approves once`. -/

inductive Out where
  | passing
  | by_ (person : String)
  | other
  deriving DecidableEq, Repr, Inhabited

inductive Look where
  | passed_over
  | other
  | parts (outs : List Out) (once : Bool)
  deriving DecidableEq, Repr, Inhabited

structure Scan where
  found : Option Int
  done : Bool
  deriving DecidableEq, Repr, Inhabited

structure Parts where
  s : Step
  parts : List Part
  deriving Inhabited

inductive Settled where
  | clear | started | passed
  deriving DecidableEq, Repr, Inhabited

structure Settle where
  s : Step
  how : Settled
  deriving Inhabited

structure Started where
  part : Int
  p : Part
  deriving DecidableEq, Repr, Inhabited

inductive Fate where
  | waits | starts
  deriving DecidableEq, Repr, Inhabited

structure Decided where
  stage : Int
  part : Int
  fate : Fate
  p : Part
  made : List What
  deriving Inhabited

structure Try where
  stage : Int
  r : Except Refusal (Option Decided)

/-! Coming to a stage and moving on. -/

structure Walk where
  s : Step
  at_ : Int
  over : Bool
  end_ : Bool
  stop : Option Refusal
  deriving Inhabited

structure Lap where
  stages : List Stage
  stage : Option String
  fx : List Made
  at_ : Int
  over : Bool
  end_ : Bool
  stop : Option Refusal
  deriving Inhabited

/-! The `on` lines. -/

structure Booked where
  holds : List Held
  made : List What
  stop : Option Refusal
  deriving Inhabited

/-! An event. -/

structure Rung where
  timer : String
  stage : Option String
  at_ : Int
  deriving DecidableEq, Repr, Inhabited

structure Seat where
  part : Int
  seat : String
  deriving DecidableEq, Repr, Inhabited

structure Unpaused where
  s : Step
  used : Int
  deriving Inhabited

structure Move where
  part : Int
  people : List String
  way : Option String
  team : Option String
  deriving DecidableEq, Repr, Inhabited

end RingiCore
