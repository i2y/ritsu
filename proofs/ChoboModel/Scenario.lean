/-
  Scenarios (chobo's DESIGN 2.6, `crates/chobo/src/scenario.rs`): calls one after another, time
  passing, and `together` — the operations of several callers, run in every order that keeps each
  caller's own order. The result of a scenario is every distinct way it can come out.
-/
import ChoboModel.Ledger

namespace ChoboModel

inductive Step where
  | call (c : Call)
  | pass (secs : Nat)
  /-- each caller's operations, in order -/
  | together (callers : List (List Call))
  deriving Repr

/-- What one step came to. -/
inductive StepOut where
  | call (o : Outcome)
  | pass
  | together (outs : List (List Outcome))
  deriving DecidableEq, Repr

/-- One way a scenario came out: what each step came to, and the state at the end. -/
structure Run where
  steps : List StepOut
  state : State
  deriving DecidableEq, Repr

/-- Every order the callers' operations can come in, each caller's kept: a list of callers, one
    for each operation. The first caller that still has one goes first, as in `scenario.rs`. -/
def orders : Nat → List Nat → List (List Nat)
  | 0, _ => [[]]
  | n + 1, left =>
    (List.range left.length).flatMap (fun i =>
      match left[i]? with
      | some (k + 1) => (orders n (left.set i k)).map (i :: ·)
      | _ => [])

/-- One order of a `together`: each caller's outcomes, in its own order. -/
def runOrder (b : Book) (callers : List (List Call)) :
    List Nat → State → List (List Outcome) → Except String (State × List (List Outcome))
  | [], s, outs => .ok (s, outs)
  | i :: rest, s, outs =>
    let done := (outs[i]?.getD []).length
    match (callers[i]?.getD [])[done]? with
    | none => .error "an order names an operation a caller does not have"
    | some c =>
      match s.apply b c with
      | .error e => .error e
      | .ok (o, s') => runOrder b callers rest s' (outs.set i ((outs[i]?.getD []) ++ [o]))

def dedup {α : Type} [DecidableEq α] : List α → List α
  | [] => []
  | x :: xs => if x ∈ dedup xs then dedup xs else x :: dedup xs

/-- One step, from every run so far. -/
def stepRuns (b : Book) (st : Step) (runs : List Run) : Except String (List Run) := do
  let mut next : List Run := []
  for r in runs do
    match st with
    | .call c =>
      let (o, s') ← r.state.apply b c
      next := next ++ [{ steps := r.steps ++ [.call o], state := s' }]
    | .pass secs =>
      next := next ++ [{ steps := r.steps ++ [.pass], state := r.state.pass secs }]
    | .together callers =>
      let total := (callers.map List.length).foldl (· + ·) 0
      for order in orders total (callers.map List.length) do
        let (s', outs) ← runOrder b callers order r.state (callers.map (fun _ => []))
        next := next ++ [{ steps := r.steps ++ [.together outs], state := s' }]
  return dedup next

/-- Every way a scenario can come out, from an empty book. -/
def runScenario (b : Book) (steps : List Step) : Except String (List Run) :=
  steps.foldlM (fun runs st => stepRuns b st runs) [{ steps := [], state := {} }]

/-! ## What a result shows -/

/-- Every account a `do` or a `hold` names, in the order of its moves. -/
def callAccounts (b : Book) (c : Call) : List AccountId :=
  match b.transfers[c.kind]? with
  | none => []
  | some t =>
    if c.op == .«do» || c.op == .hold then
      (c.moves t).foldl (fun acc m =>
        let acc := if m.src ∈ acc then acc else acc ++ [m.src]
        if m.dst ∈ acc then acc else acc ++ [m.dst]) []
    else []

def ltStrings : List String → List String → Bool
  | [], [] => false
  | [], _ :: _ => true
  | _ :: _, [] => false
  | a :: as, b :: bs => if a < b then true else if a = b then ltStrings as bs else false

/-- Accounts in the order of their kinds, then of their arguments: `BTreeMap<AccountId, _>`'s. -/
def AccountId.le (a b : AccountId) : Bool :=
  a.kind < b.kind || (a.kind == b.kind && !ltStrings b.args a.args)

def HoldKey.le (a b : HoldKey) : Bool :=
  a.1 < b.1 || (a.1 == b.1 && !ltStrings b.2 a.2)

/-- Every account the scenario's `do` and `hold` name, whatever came of them, in order. -/
def namedAccounts (b : Book) (steps : List Step) : List AccountId :=
  let calls := steps.flatMap (fun st => match st with
    | .call c => [c]
    | .pass _ => []
    | .together cs => cs.flatten)
  let all := calls.foldl (fun acc c => (callAccounts b c).foldl (fun acc a => if a ∈ acc then acc else acc ++ [a]) acc) []
  all.mergeSort AccountId.le

/-- The holds of a state, each once, in the order of their keys. -/
def holdsOf (s : State) : List (HoldKey × Hold) :=
  (latest s.holds).mergeSort (fun a b => HoldKey.le a.1 b.1)

end ChoboModel
