/-
  Reading a book and a scenario as the Rust tests write them, and writing a result as chobo's
  `scenario::result_json` does. Plumbing: it proves nothing. A field it cannot read is an error,
  and an error is a line the comparison reports, never a quiet pass.
-/
import Lean.Data.Json
import ChoboModel.Scenario

namespace ChoboModel
open Lean

def field (j : Json) (k : String) : Except String Json := j.getObjVal? k

def arrOf (j : Json) (k : String) : Except String (List Json) := do
  return (← (← field j k).getArr?).toList

def optField (j : Json) (k : String) : Option Json :=
  match j.getObjVal? k with
  | .ok v => if v.isNull then none else some v
  | .error _ => none

def readBound (j : Json) : Except String Bound := do
  return { value := ← (← field j "value").getInt?, refusal := ← (← field j "refusal").getStr? }

def readAccount (j : Json) : Except String AccountKind := do
  let lower ← match optField j "lower" with
    | some b => some <$> readBound b
    | none => pure none
  let upper ← match optField j "upper" with
    | some b => some <$> readBound b
    | none => pure none
  return { name := ← (← field j "name").getStr?, lower, upper }

def readArg (j : Json) : Except String Arg :=
  match optField j "param" with
  | some i => Arg.param <$> i.getNat?
  | none => do return .lit (← (← field j "lit").getStr?)

def readAmount (j : Json) : Except String Amount :=
  match optField j "param" with
  | some i => Amount.param <$> i.getNat?
  | none => do return .lit (← (← field j "lit").getInt?)

def readRef (j : Json) : Except String Ref := do
  return { kind := ← (← field j "kind").getNat?, args := ← (← arrOf j "args").mapM readArg }

def readMove (j : Json) : Except String Move := do
  return { amount := ← readAmount (← field j "amount"), src := ← readRef (← field j "src"),
           dst := ← readRef (← field j "dst") }

def readTransfer (j : Json) : Except String TransferKind := do
  let pending ← match optField j "pending" with
    | none => pure none
    | some p => match p.getStr? with
      | .ok "never" => pure (some Expiry.never)
      | _ => do pure (some (Expiry.after (← (← field p "after").getNat?)))
  return { name := ← (← field j "name").getStr?,
           amounts := ← (← arrOf j "amounts").mapM (·.getBool?),
           key := ← (← arrOf j "key").mapM (·.getNat?),
           pending,
           moves := ← (← arrOf j "moves").mapM readMove }

def readBook (j : Json) : Except String Book := do
  return { accounts := ← (← arrOf j "accounts").mapM readAccount,
           transfers := ← (← arrOf j "transfers").mapM readTransfer }

def readVal (j : Json) : Except String (Option Val) :=
  if j.isNull then pure none
  else match optField j "str" with
    | some s => do return some (.str (← s.getStr?))
    | none => do return some (.amt (← (← field j "amt").getInt?))

def readOp (s : String) : Except String Op :=
  match s with
  | "do" => pure .«do»
  | "hold" => pure .hold
  | "post" => pure .post
  | "void" => pure .void
  | other => throw s!"unknown op {other}"

def readCall (j : Json) : Except String Call := do
  let amounts ← match optField j "amounts" with
    | none => pure none
    | some a => do
      let pairs ← (← a.getArr?).toList.mapM (fun p => do
        let xs ← p.getArr?
        let i ← (xs[0]?.getD Json.null).getNat?
        let v ← (xs[1]?.getD Json.null).getInt?
        return (i, v))
      pure (some pairs)
  return { op := ← readOp (← (← field j "op").getStr?), kind := ← (← field j "kind").getNat?,
           args := ← (← arrOf j "args").mapM readVal, amounts }

def readStep (j : Json) : Except String Step :=
  match optField j "call" with
  | some c => Step.call <$> readCall c
  | none => match optField j "pass" with
    | some p => Step.pass <$> p.getNat?
    | none => do
      let callers ← (← (← field j "together").getArr?).toList.mapM (fun c => do
        (← c.getArr?).toList.mapM readCall)
      return .together callers

def readSteps (j : Json) : Except String (List Step) := do
  (← arrOf j "steps").mapM readStep

/-! ## Writing a result -/

def outcomeJson (b : Book) (c : Call) (o : Outcome) : Json :=
  let kind := ((b.transfers[c.kind]?).map (·.name)).getD ""
  let base : List (String × Json) := [("op", Json.str c.op.name), ("kind", Json.str kind)]
  match o with
  | .done => Json.mkObj (base ++ [("result", Json.str "done")])
  | .doneBefore => Json.mkObj (base ++ [("result", Json.str "done_before")])
  | .refused r => Json.mkObj (base ++ [("result", Json.str "refused"), ("reason", Json.str r)])

def stepJson (b : Book) : Step → StepOut → Json
  | .call c, .call o => outcomeJson b c o
  | .pass _, _ => Json.mkObj [("op", Json.str "pass")]
  | .together cs, .together os =>
    Json.mkObj [("op", Json.str "together"),
      ("callers", Json.arr ((cs.zip os).map (fun (cc, oo) =>
        Json.arr ((cc.zip oo).map (fun (c, o) => outcomeJson b c o)).toArray)).toArray)]
  | _, _ => Json.null

def strings (xs : List String) : Json := Json.arr (xs.map Json.str).toArray

def resultJson (b : Book) (steps : List Step) (r : Run) : Json :=
  let accounts := (namedAccounts b steps).map (fun a =>
    let bal := r.state.balance a
    let name := ((b.accounts[a.kind]?).map (·.name)).getD ""
    Json.mkObj [("account", Json.str name), ("args", strings a.args), ("posted", toJson bal.posted),
      ("held_in", toJson bal.heldIn), ("held_out", toJson bal.heldOut)])
  let holds := (holdsOf r.state).map (fun (k, h) =>
    let name := ((b.transfers[k.1]?).map (·.name)).getD ""
    Json.mkObj [("kind", Json.str name), ("key", strings k.2), ("state", Json.str h.state.name)])
  Json.mkObj [("steps", Json.arr ((steps.zip r.steps).map (fun (st, o) => stepJson b st o)).toArray),
    ("accounts", Json.arr accounts.toArray), ("holds", Json.arr holds.toArray)]

def hasTogether (steps : List Step) : Bool :=
  steps.any (fun st => match st with
    | .together _ => true
    | _ => false)

/-- What `chobo`'s `scenario::run_json` answers: one result, or with `together`, every distinct
    one (the comparison reads them as a set). -/
def answer (b : Book) (line : Json) : Json :=
  match readSteps line with
  | .error e => Json.mkObj [("error", Json.str s!"unreadable scenario: {e}")]
  | .ok steps =>
    match runScenario b steps with
    | .error e => Json.mkObj [("error", Json.str e)]
    | .ok runs =>
      if hasTogether steps then
        let outs := runs.map (resultJson b steps)
        let distinct := outs.foldl (fun acc j => if acc.any (fun k => k.compress == j.compress) then acc else acc ++ [j]) []
        Json.mkObj [("outcomes", Json.arr distinct.toArray)]
      else match runs with
        | r :: _ => resultJson b steps r
        | [] => Json.mkObj [("error", Json.str "no run")]

end ChoboModel
