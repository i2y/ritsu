/-
  The check of what a case is left in when a flow ends (dandori's DESIGN 2.1–2.5, E020), as a
  function, for the core.

  A case follows a state machine (rulec's), given here as what it says: the state each event
  leads to from each state, whether it refuses an event in a state, what the events on the other
  side lead to, and which states are final. Two things are true of a case at a point of a run:
  what the flow last heard of it (the state its record says, or nothing yet), and the state it is
  really in on the other side, which external events can have moved on since, and which a call
  that failed may or may not have moved. The check keeps, for each case, the pairs (what the flow
  last heard, a state the case was in when it last heard or since) that a run can bring to the
  point; the true state is then that state or one external events lead to from it.

  At every place the flow ends — `succeed`, `fail` (less the cases it hands over with `leaving`),
  the end of the flow, the end of `on failure` — every state a started case can be in, the
  external events' included, has to be final; otherwise the check fails, as E020 does. Where it
  cannot say — a loop that does not settle within its rounds, a machine whose closure does not
  close, an event sent to a case not surely started, a case started twice, `for … in parallel` —
  it fails too, so that a pass always means what `DandoriCore.Sound` proves.
-/
import DandoriCore.Syntax

namespace DandoriCore

/-- A case's state machine, as the check reads it. States are numbers; `names` gives each one's
    name, the value a case's record carries. -/
structure Machine where
  names : List String
  initial : Nat
  finals : List Nat
  /-- what an event leads to from a state, when the machine takes it there -/
  next : Nat → String → List Nat
  /-- whether the machine refuses the event in the state -/
  refuses : Nat → String → Bool
  /-- what the events on the other side lead to from a state -/
  ext : Nat → List Nat
  /-- what any event leads to: a case the flow finds already started can be anywhere these lead -/
  any : Nat → List Nat

/-- What a check knows: the flow, each case's machine, and how many cases there are. -/
structure Env where
  flow : Flow
  ms : Nat → Machine
  n : Nat

/-! ## Following steps -/

/-- `t` reaches `v` by zero or more of the steps. -/
inductive Reach (step : Nat → List Nat) : Nat → Nat → Prop
  | refl (t : Nat) : Reach step t t
  | tail {t u v : Nat} : Reach step t u → v ∈ step u → Reach step t v

def grow (step : Nat → List Nat) (cur : List Nat) : List Nat :=
  cur ++ (cur.flatMap step).filter (fun x => !cur.contains x)

def iterate (step : Nat → List Nat) : Nat → List Nat → List Nat
  | 0, cur => cur
  | k + 1, cur => iterate step k (grow step cur)

def closedUnder (step : Nat → List Nat) (cl : List Nat) : Bool :=
  cl.all (fun t => (step t).all (fun u => cl.contains u))

/-- `cl`, when it holds `b` and the steps lead nowhere out of it. -/
def checkClosed (step : Nat → List Nat) (b : Nat) (cl : List Nat) : Option (List Nat) :=
  if closedUnder step cl && cl.contains b then some cl else none

/-- Every state the steps lead to from `b`, when 64 rounds of following them close; a
    machine of rulec has far fewer states than that. -/
def closure (step : Nat → List Nat) (b : Nat) : Option (List Nat) :=
  checkClosed step b (iterate step 64 [b])

/-! ## What the check keeps -/

/-- What the flow last heard of a case (nothing yet, or a state), and a state the case was in
    then or since (none: not started). -/
abbrev Pair := Option Nat × Option Nat

/-- For each case, the pairs a run can bring to a point. -/
abbrev Abs := Nat → List Pair

/-- What can come out of a statement: the pairs where it ends and the next one begins, where a
    `break` leaves it, where a task's error nothing handles leaves it; none where nothing does. -/
structure Res where
  normal : Option Abs
  brk : Option Abs
  raised : Option Abs

def Res.none : Res := { normal := .none, brk := .none, raised := .none }

def setAbs (A : Abs) (c : Nat) (P : List Pair) : Abs := fun i => if i = c then P else A i

def unionL (a b : List Pair) : List Pair := a ++ b.filter (fun p => !a.contains p)

def unionA (a b : Abs) : Abs := fun i => unionL (a i) (b i)

def joinO : Option Abs → Option Abs → Option Abs
  | .none, b => b
  | a, .none => a
  | some a, some b => some (unionA a b)

def Res.join (r s : Res) : Res :=
  { normal := joinO r.normal s.normal, brk := joinO r.brk s.brk, raised := joinO r.raised s.raised }

def subL (a b : List Pair) : Bool := a.all (fun p => b.contains p)

def subA (n : Nat) (a b : Abs) : Bool := (List.range n).all (fun i => subL (a i) (b i))

/-! ## A call on a case -/

/-- The states the true state can be in from a state the case was in: its closure under the
    events on the other side (the check makes sure it closes before it reads it). -/
def cl (m : Machine) (b : Nat) : List Nat := (closure m.ext b).getD []

def closuresOk (m : Machine) (P : List Pair) : Bool :=
  P.all (fun p => match p.2 with
    | some b => (closure m.ext b).isSome
    | none => true)

def allStarted (P : List Pair) : Bool := P.all (fun p => p.2.isSome)
def allUnstarted (P : List Pair) : Bool := P.all (fun p => p.2.isNone)

/-- What `sends e` can leave when the call is answered: the state the case went to, heard. -/
def sendOk (m : Machine) (e : String) (P : List Pair) : List Pair :=
  P.flatMap (fun p => match p.2 with
    | some b => (cl m b).flatMap (fun s => (m.next s e).map (fun s' => (some s', some s')))
    | none => [])

/-- …and when it fails: the event may or may not have happened, and the flow heard nothing. -/
def sendErr (m : Machine) (e : String) (P : List Pair) : List Pair :=
  P ++ P.flatMap (fun p => match p.2 with
    | some b => (cl m b).flatMap (fun s => (m.next s e).map (fun s' => (p.1, some s')))
    | none => [])

/-- The states `starts … then e1, e2, …` can leave a new case in. -/
def thenStates (m : Machine) (es : List String) : List Nat :=
  es.foldl (fun cur e => cur.flatMap (fun s => m.next s e)) [m.initial]

def startOk (m : Machine) (es : List String) : List Pair :=
  (thenStates m es).map (fun s => (some s, some s))

def startErr (m : Machine) (es : List String) (P : List Pair) : List Pair :=
  P ++ P.flatMap (fun p => (thenStates m es).map (fun s => (p.1, some s)))

/-- Every state a case the flow finds already started can be in. -/
def clAny (m : Machine) : List Nat := (closure m.any m.initial).getD []

/-- What `observes` can leave when it is answered: the state the case is in, heard. -/
def observeOk (m : Machine) (P : List Pair) : List Pair :=
  P.flatMap (fun p => match p.2 with
    | some b => (cl m b).map (fun s => (some s, some s))
    | none => (clAny m).map (fun s => (some s, some s)))

/-- What a call can leave its case in, answered and failed; none where the check cannot say. -/
def callAbs (env : Env) (tgt : Option Target) (callee : Callee) (A : Abs) : Option (Abs × Abs) :=
  match tgt, callee with
  | some (.case c), .task t =>
    if c < env.n then
      let m := env.ms c
      let P := A c
      match ((env.flow.tasks[t]?).map Task.effect : Option Effect) with
      | some (.sends e) =>
        if allStarted P && closuresOk m P then some (setAbs A c (sendOk m e P), setAbs A c (sendErr m e P)) else .none
      | some (.starts es) =>
        if allUnstarted P then some (setAbs A c (startOk m es), setAbs A c (startErr m es P)) else .none
      | some .observes =>
        if closuresOk m P && (allStarted P || (closure m.any m.initial).isSome) then some (setAbs A c (observeOk m P), A)
        else .none
      | _ => .none
    else .none
  | _, _ => some (A, A)

/-- Whether a handler takes every error: one that names `failure`. -/
def catchesAll : List Handler → Bool
  | [] => false
  | (.mk errs _) :: rest => errs.any (fun e => match e with
      | .failure => true
      | _ => false) || catchesAll rest

/-! ## A match on a case's state -/

/-- The case whose state a `match` reads, if it reads one. -/
def caseOf (env : Env) (e : Expr) : Option Nat :=
  match e with
  | .var x [f] => (env.flow.cases.zipIdx.find? (fun p => p.1.name == x && p.1.stateField == f)).map (·.2)
  | _ => .none

/-- Whether an arm takes what the flow last heard of the case. -/
def armHits (names : List String) : Arm → Option Nat → Bool
  | .mk values isNone someName _, k =>
    (isNone && k.isNone) || (someName.isSome && k.isSome) ||
      (match k with
       | some s => values.contains (names.getD s "")
       | .none => false)

def narrow (env : Env) (e : Expr) (arm : Arm) (A : Abs) : Abs :=
  match caseOf env e with
  | some c => setAbs A c ((A c).filter (fun p => armHits (env.ms c).names arm p.1))
  | .none => A

/-! ## The check -/

/-- Every place a flow ends checks the cases: each one not handed over, if it can be started, can
    only be in final states, the events on the other side included. -/
def endOk (env : Env) (A : Abs) (leaving : List Nat) : Bool :=
  (List.range env.n).all (fun c => leaving.contains c || (A c).all (fun p => match p.2 with
    | .none => true
    | some b => match closure (env.ms c).ext b with
      | some cl => cl.all (fun s => (env.ms c).finals.contains s)
      | .none => false))

def endOkO (env : Env) (A : Option Abs) (leaving : List Nat) : Bool :=
  match A with
  | .none => true
  | some a => endOk env a leaving

/-- A loop's head: from where it starts, joined with where each round ends, until a round adds
    nothing. -/
def fix (n : Nat) (bd : Abs → Option Res) : Nat → Abs → Option Abs
  | 0, _ => .none
  | k + 1, H =>
    match bd H with
    | .none => .none
    | some rb =>
      match rb.normal with
      | .none => some H
      | some N => if subA n N H then some H else fix n bd k (unionA H N)

def loopRes (n : Nat) (bd : Abs → Option Res) (A : Abs) : Option Res :=
  match fix n bd 64 A with
  | .none => .none
  | some H =>
    match bd H with
    | .none => .none
    | some rb => some { normal := joinO (some H) rb.brk, brk := .none, raised := rb.raised }

def chkHandlers (rec : List Stmt → Abs → Option Res) (A : Abs) : List Handler → Option Res
  | [] => some Res.none
  | (.mk _ body) :: rest =>
    match rec body A, chkHandlers rec A rest with
    | some r, some rs => some (r.join rs)
    | _, _ => .none

def chkArms (env : Env) (rec : List Stmt → Abs → Option Res) (e : Expr) (A : Abs) : List Arm → Option Res
  | [] => some Res.none
  | arm :: rest =>
    match arm with
    | .mk _ _ _ body =>
      match rec body (narrow env e arm A), chkArms env rec e A rest with
      | some r, some rs => some (r.join rs)
      | _, _ => .none

/-- One statement, the blocks under it checked by `rec`. -/
def chkHead (env : Env) (rec : List Stmt → Abs → Option Res) (s : Stmt) (A : Abs) : Option Res :=
  match s with
  | .pass _ | .assign .. | .wait _ => some { normal := some A, brk := .none, raised := .none }
  | .brk _ => some { normal := .none, brk := some A, raised := .none }
  | .succeed .. => if endOk env A [] then some Res.none else .none
  | .fail _ _ _ leaving => if endOk env A leaving then some Res.none else .none
  | .matchOn _ _ e _ arms =>
    match caseOf env e with
    | some c => if c < env.n then chkArms env rec e A arms else .none
    | .none => chkArms env rec e A arms
  | .repeat _ _ body => loopRes env.n (rec body) A
  | .forEach _ _ _ _ _ body _ parallel => if parallel.isSome then .none else loopRes env.n (rec body) A
  | .call _ _ tgt callee hs =>
    match callAbs env tgt callee A with
    | .none => .none
    | some (aok, aerr) =>
      match chkHandlers rec aerr hs with
      | .none => .none
      | some rh =>
        some { normal := joinO (some aok) rh.normal, brk := rh.brk,
               raised := joinO (if catchesAll hs then .none else some aerr) rh.raised }

/-- A block of statements. The fuel bounds how deep blocks nest, no more. -/
def chk (env : Env) : Nat → List Stmt → Abs → Option Res
  | 0, _, _ => .none
  | _ + 1, [], A => some { normal := some A, brk := .none, raised := .none }
  | f + 1, s :: rest, A =>
    match chkHead env (chk env f) s A with
    | .none => .none
    | some r1 =>
      match r1.normal with
      | .none => some { r1 with normal := .none }
      | some N =>
        match chk env f rest N with
        | .none => .none
        | some r2 => some { normal := r2.normal, brk := joinO r1.brk r2.brk, raised := joinO r1.raised r2.raised }

/-- No case started yet. -/
def start : Abs := fun _ => [(.none, .none)]

/-- **The check of a flow**: the flow, its end, and `on failure` and its end. -/
def chkFlow (env : Env) (fuel : Nat) : Bool :=
  match chk env fuel env.flow.flow start with
  | .none => false
  | some r =>
    endOkO env r.normal [] &&
    match r.raised, env.flow.onFailure with
    | some ar, some blk =>
      match chk env fuel blk ar with
      | .none => false
      | some r2 => endOkO env r2.normal []
    | _, _ => true

end DandoriCore
