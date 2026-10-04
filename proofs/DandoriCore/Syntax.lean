/-
  The core of a dandori flow (dandori's DESIGN 1.5, 2): the statements, the tasks and rules they
  call, and the cases that follow state machines — as `crates/dandori/src/model.rs` has them once a
  `.flow` is checked. How some answers are read before the flow sees them — Jev's, a Claude agent's
  enum values, protobuf's zero values of a Connect service or of a service the flow implements, a
  rule's answer from its Connect service — is not modelled: a run is handed what dandori reads
  each such answer as (`St.reads`), and the model holds what the flow does with it.

  Two things are built on it. `DandoriCore.Replay` runs a flow against a scenario, as dandori's
  reference interpreter does, and `ritsu-model` compares the two. `DandoriCore.Check` and
  `DandoriCore.Sound` are the check of what a case is left in when the flow ends (E020) and the
  proof that it settles what it says.
-/
import Lean.Data.Json

namespace DandoriCore
open Lean (Json)

/-- A type, as far as the values that come in are held to it (`render::value_fits`). -/
inductive Ty where
  | int
  | str
  | bool
  | timestamp
  | json
  | enum (values : List String)
  | record (r : Nat)
  | list (t : Ty)
  | opt (t : Ty)
  deriving Repr, Inhabited

structure Range where
  lo : Option Int
  hi : Option Int
  deriving Repr, Inhabited

structure Field where
  name : String
  ty : Ty
  range : Option Range
  deriving Repr, Inhabited

/-- An expression: a variable and the fields read from it, a value written out, a record, a list,
    or a string with values put in (its parts, each a literal string or a value). -/
inductive Expr where
  | var (name : String) (fields : List String)
  | lit (v : Json)
  | record (fields : List (String × Expr))
  | list (items : List Expr)
  | interp (parts : List Expr)
  deriving Inhabited

/-- An error a handler takes: one the task declares, `timeout`, or `failure` (any). A declared
    error carries every name it is seen by on the platform. -/
inductive HErr where
  | declared (names : List String)
  | timeout
  | failure
  deriving Repr, Inhabited

/-- Whether a handler's error takes an error of this kind. -/
def HErr.hits (kind : String) : HErr → Bool
  | .failure => true
  | .timeout => kind == "timeout"
  | .declared names => names.contains kind

inductive Target where
  | letVar (x : String)
  | case (c : Nat)
  deriving Repr, Inhabited

inductive Callee where
  | task (t : Nat)
  | rule (r : Nat)
  deriving Repr, Inhabited, DecidableEq

mutual
  inductive Stmt where
    /-- `[let x =] t(…)` or `c <- t(…)`, and the handlers under it. -/
    | call (site line : Nat) (target : Option Target) (callee : Callee) (handlers : List Handler)
    | assign (site : Nat) (name : String) (e : Expr)
    /-- `match`: what it reads, as the flow writes it, and the arms. -/
    | matchOn (site line : Nat) (e : Expr) (shown : String) (arms : List Arm)
    /-- `wait`: nothing a run on Temporal reports. -/
    | wait (site : Nat)
    | repeat (site : Nat) (times : Nat) (body : List Stmt)
    /-- `for x in <list> at most <max>`; `parallel`, the variables each round sets for itself when
        the rounds run in parallel. -/
    | forEach (site line : Nat) (var : String) (list : Expr) (max : Nat) (body : List Stmt)
        (result : Option (String × Expr)) (parallel : Option (List String))
    | brk (site : Nat)
    | pass (site : Nat)
    | succeed (site : Nat) (fields : List (String × Expr))
    | fail (site : Nat) (error : String) (cause : Option Expr) (leaving : List Nat)
  inductive Handler where
    | mk (errors : List HErr) (body : List Stmt)
  inductive Arm where
    /-- the values it names, whether it is the `none` arm, the name a `some y` arm gives the value -/
    | mk (values : List String) (none : Bool) (some : Option String) (body : List Stmt)
end

instance : Inhabited Stmt := ⟨.pass 0⟩

/-- What a call does to the case it is made on (dandori's DESIGN 1.4, 2.1). -/
inductive Effect where
  | none
  | starts (thenEvents : List String)
  | sends (event : String)
  | observes
  deriving Repr, Inhabited

/-- `retry`: the error names it takes and how many tries it gives. -/
structure Retrier where
  names : List String
  max : Nat
  deriving Repr, Inhabited

structure Task where
  name : String
  /-- What it answers; none for a task with no `->`, read as `json`. -/
  result : Option Ty
  range : Option Range
  retriers : List Retrier
  effect : Effect
  /-- `refused as`: the error the call comes back with when the machine refuses its event. -/
  refusedAs : Option String
  deriving Repr, Inhabited

structure Rule where
  name : String
  /-- The record of its outputs. -/
  outputs : Nat
  retriers : List Retrier
  deriving Repr, Inhabited

structure Case where
  name : String
  stateField : String
  deriving Repr, Inhabited

structure Flow where
  records : List (List Field)
  inputs : List Field
  tasks : List Task
  rules : List Rule
  cases : List Case
  /-- For a call on a case, by its site: the states its answer may carry. -/
  monitors : List (Nat × List String)
  flow : List Stmt
  onFailure : Option (List Stmt)
  onCancel : Option (List Stmt)
  /-- The flow implements a service: a run with no outputs ends with `{}`, which protobuf's JSON
      reads a response from. -/
  service : Bool := false
  deriving Inhabited

end DandoriCore
