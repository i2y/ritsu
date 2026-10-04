/-
  What can happen when a flow runs, with the cases' true states on the other side (dandori's
  DESIGN 2.1–2.5).

  A run here is any run the flow and the other side can make together, not one scenario: a call
  can be answered or fail with any error; a `match` on something other than a case's state can take
  any arm; a loop can run any number of rounds; the events on the other side can move a case on
  before any call. For each case it keeps what the flow last heard of it and the state it really
  is in. What the flow does to a case follows its machine:

  - `sends e`, answered: the case went to a state the machine leads to from its state by `e`, and
    the flow heard it. Refused: the case is in a state where the machine refuses `e`, and stays. Any
    other error: the event may have happened or not, and the flow heard nothing.
  - `starts … then …`, on a case not started: answered, it is in a state the events lead to from
    the machine's start; failed, it may or may not have started.
  - `observes`: answered, the flow hears the state the case is in — a case it did not start is
    found in some state the machine reaches from its start.

  One call is one event on the other side however many times it is tried (what a task's `key`
  gives), and a case is started once. `DandoriCore.Sound` proves that a flow `chkFlow` passes ends,
  in every run where the check looks, with no case left in a state that is not final.
-/
import DandoriCore.Check

namespace DandoriCore

/-- For each case: what the flow last heard (nothing, or a state), and the state it really is in
    on the other side (none: not started). -/
abbrev World := Nat → Option Nat × Option Nat

def setW (w : World) (c : Nat) (v : Option Nat × Option Nat) : World := fun i => if i = c then v else w i

/-- No case started yet. -/
def World.start : World := fun _ => (none, none)

/-- The events on the other side moved the cases' true states on, and nothing else changed. -/
def ExtW (env : Env) (w w' : World) : Prop :=
  ∀ i, (w' i).1 = (w i).1 ∧ (((w i).2 = none ∧ (w' i).2 = none) ∨
    ∃ t t', (w i).2 = some t ∧ (w' i).2 = some t' ∧ Reach (env.ms i).ext t t')

/-- How a call comes out. -/
inductive CallOut where
  | ok (w : World)
  | err (kind : String) (w : World)

/-- The case a call is made on, and what its task does to the case. -/
def caseCall (env : Env) (tgt : Option Target) (callee : Callee) : Option (Nat × Effect) :=
  match tgt, callee with
  | some (.case c), .task t => (env.flow.tasks[t]?).map (fun tk => (c, tk.effect))
  | _, _ => none

/-- The error a refused event comes back as. -/
def refusedName (env : Env) (callee : Callee) : String :=
  match callee with
  | .task t => ((env.flow.tasks[t]?).bind (·.refusedAs)).getD "failure"
  | .rule _ => "failure"

/-- **One call, on the other side.** -/
inductive CallStep (env : Env) (tgt : Option Target) (callee : Callee) : World → CallOut → Prop
  | plainOk {w} : caseCall env tgt callee = none → CallStep env tgt callee w (.ok w)
  | plainErr {w k} : caseCall env tgt callee = none → CallStep env tgt callee w (.err k w)
  | sendOk {w c e s s'} : caseCall env tgt callee = some (c, .sends e) → (w c).2 = some s →
      s' ∈ (env.ms c).next s e → CallStep env tgt callee w (.ok (setW w c (some s', some s')))
  | sendRefused {w c e s} : caseCall env tgt callee = some (c, .sends e) → (w c).2 = some s →
      (env.ms c).refuses s e = true → CallStep env tgt callee w (.err (refusedName env callee) w)
  | sendFailed {w c e k} : caseCall env tgt callee = some (c, .sends e) → CallStep env tgt callee w (.err k w)
  | sendFailedAfter {w c e k s s'} : caseCall env tgt callee = some (c, .sends e) → (w c).2 = some s →
      s' ∈ (env.ms c).next s e → CallStep env tgt callee w (.err k (setW w c ((w c).1, some s')))
  | startOk {w c es s} : caseCall env tgt callee = some (c, .starts es) → (w c).2 = none →
      s ∈ thenStates (env.ms c) es → CallStep env tgt callee w (.ok (setW w c (some s, some s)))
  | startFailed {w c es k} : caseCall env tgt callee = some (c, .starts es) → CallStep env tgt callee w (.err k w)
  | startFailedAfter {w c es k s} : caseCall env tgt callee = some (c, .starts es) → (w c).2 = none →
      s ∈ thenStates (env.ms c) es → CallStep env tgt callee w (.err k (setW w c ((w c).1, some s)))
  | observeOk {w c t} : caseCall env tgt callee = some (c, .observes) → (w c).2 = some t →
      CallStep env tgt callee w (.ok (setW w c (some t, some t)))
  | observeFound {w c t} : caseCall env tgt callee = some (c, .observes) → (w c).2 = none →
      Reach (env.ms c).any (env.ms c).initial t → CallStep env tgt callee w (.ok (setW w c (some t, some t)))
  | observeFailed {w c k} : caseCall env tgt callee = some (c, .observes) → CallStep env tgt callee w (.err k w)

def Handler.body : Handler → List Stmt
  | .mk _ body => body

/-- A handler takes an error of this kind. -/
def Handler.takes (h : Handler) (k : String) : Prop :=
  match h with
  | .mk errs _ => errs.any (HErr.hits k) = true

def Arm.body : Arm → List Stmt
  | .mk _ _ _ body => body

/-- An arm a run can take: on a case's state, one that takes what the flow last heard of it;
    on anything else, any. -/
def ArmMay (env : Env) (e : Expr) (arm : Arm) (w : World) : Prop :=
  match caseOf env e with
  | some c => armHits (env.ms c).names arm (w c).1 = true
  | none => True

/-- How a block comes out: it ends and the next statement begins, a `break` leaves it, a task's
    error nothing handled leaves it, or the run ends there — where the check looks, handing over
    `leaving`, or not (`none`: a match no arm takes, a list longer than a `for` allows). -/
inductive Out where
  | normal (w : World)
  | brk (w : World)
  | raised (w : World)
  | ended (leaving : Option (List Nat)) (w : World)

/-- The block stops without a `break`. -/
def Out.halts : Out → Prop
  | .raised _ => True
  | .ended _ _ => True
  | _ => False

/-- The block stops. -/
def Out.stops : Out → Prop
  | .normal _ => False
  | _ => True

/-- **A run of a block.** -/
inductive Exec (env : Env) : List Stmt → World → Out → Prop
  | nil {w} : Exec env [] w (.normal w)
  | pass {site rest w o} : Exec env rest w o → Exec env (.pass site :: rest) w o
  | assign {site x e rest w o} : Exec env rest w o → Exec env (.assign site x e :: rest) w o
  | wait {site rest w o} : Exec env rest w o → Exec env (.wait site :: rest) w o
  | brk {site rest w} : Exec env (.brk site :: rest) w (.brk w)
  | succeed {site fs rest w} : Exec env (.succeed site fs :: rest) w (.ended (some []) w)
  | fail {site err cause leaving rest w} :
      Exec env (.fail site err cause leaving :: rest) w (.ended (some leaving) w)
  | matchArm {site line e shown arms rest w arm w1 o} : arm ∈ arms → ArmMay env e arm w →
      Exec env arm.body w (.normal w1) → Exec env rest w1 o → Exec env (.matchOn site line e shown arms :: rest) w o
  | matchArmStop {site line e shown arms rest w arm o} : arm ∈ arms → ArmMay env e arm w →
      Exec env arm.body w o → o.stops → Exec env (.matchOn site line e shown arms :: rest) w o
  | matchNone {site line e shown arms rest w} : Exec env (.matchOn site line e shown arms :: rest) w (.ended none w)
  | repeatExit {site t body rest w o} : Exec env rest w o → Exec env (.repeat site t body :: rest) w o
  | repeatRound {site t body rest w w1 o} : Exec env body w (.normal w1) →
      Exec env (.repeat site t body :: rest) w1 o → Exec env (.repeat site t body :: rest) w o
  | repeatBreak {site t body rest w w1 o} : Exec env body w (.brk w1) → Exec env rest w1 o →
      Exec env (.repeat site t body :: rest) w o
  | repeatStop {site t body rest w o} : Exec env body w o → o.halts → Exec env (.repeat site t body :: rest) w o
  | forExit {site line x list max body result par rest w o} : Exec env rest w o →
      Exec env (.forEach site line x list max body result par :: rest) w o
  | forRound {site line x list max body result par rest w w1 o} : Exec env body w (.normal w1) →
      Exec env (.forEach site line x list max body result par :: rest) w1 o →
      Exec env (.forEach site line x list max body result par :: rest) w o
  | forBreak {site line x list max body result par rest w w1 o} : Exec env body w (.brk w1) → Exec env rest w1 o →
      Exec env (.forEach site line x list max body result par :: rest) w o
  | forStop {site line x list max body result par rest w o} : Exec env body w o → o.halts →
      Exec env (.forEach site line x list max body result par :: rest) w o
  | forTooMany {site line x list max body result par rest w} :
      Exec env (.forEach site line x list max body result par :: rest) w (.ended none w)
  | callOk {site line tgt callee hs rest w w1 w2 o} : ExtW env w w1 → CallStep env tgt callee w1 (.ok w2) →
      Exec env rest w2 o → Exec env (.call site line tgt callee hs :: rest) w o
  | callHandled {site line tgt callee hs rest w w1 w2 w3 o k h} : ExtW env w w1 → CallStep env tgt callee w1 (.err k w2) →
      h ∈ hs → h.takes k → Exec env h.body w2 (.normal w3) → Exec env rest w3 o →
      Exec env (.call site line tgt callee hs :: rest) w o
  | callHandledStop {site line tgt callee hs rest w w1 w2 o k h} : ExtW env w w1 → CallStep env tgt callee w1 (.err k w2) →
      h ∈ hs → h.takes k → Exec env h.body w2 o → o.stops → Exec env (.call site line tgt callee hs :: rest) w o
  | callRaised {site line tgt callee hs rest w w1 w2 k} : ExtW env w w1 → CallStep env tgt callee w1 (.err k w2) →
      (∀ h ∈ hs, ¬ h.takes k) → Exec env (.call site line tgt callee hs :: rest) w (.raised w2)

/-- **A run that ends where the check looks**, handing over the cases `leaving`: at the end of
    the flow, at a `succeed` or a `fail`, or at the end of `on failure` after a task's error
    nothing handled. A run that ends otherwise — an error nothing handles and no `on failure`
    (W101), a match no arm takes, a list longer than its `for` allows — is not one. -/
inductive Ends (env : Env) : List Nat → World → Prop
  | flowEnd {w} : Exec env env.flow.flow World.start (.normal w) → Ends env [] w
  | stopped {leaving w} : Exec env env.flow.flow World.start (.ended (some leaving) w) → Ends env leaving w
  | onFailureEnd {w1 blk w} : Exec env env.flow.flow World.start (.raised w1) → env.flow.onFailure = some blk →
      Exec env blk w1 (.normal w) → Ends env [] w
  | onFailureStopped {w1 blk leaving w} : Exec env env.flow.flow World.start (.raised w1) →
      env.flow.onFailure = some blk → Exec env blk w1 (.ended (some leaving) w) → Ends env leaving w

end DandoriCore
