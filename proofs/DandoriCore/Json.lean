/-
  Reading a flow as the Rust tests write it, and answering one scenario a line. Plumbing: it
  proves nothing. A field it cannot read is an error, and an error is a line the comparison
  reports, never a quiet pass.
-/
import Lean.Data.Json
import DandoriCore.Replay

namespace DandoriCore
open Lean (Json)

def field (j : Json) (k : String) : Except String Json := j.getObjVal? k

def optField (j : Json) (k : String) : Option Json :=
  match j.getObjVal? k with
  | .ok v => if v.isNull then none else some v
  | .error _ => none

def arrOf (j : Json) (k : String) : Except String (List Json) := do
  return (← (← field j k).getArr?).toList

partial def readTy (j : Json) : Except String Ty :=
  match j.getStr? with
  | .ok "int" => pure .int
  | .ok "string" => pure .str
  | .ok "bool" => pure .bool
  | .ok "timestamp" => pure .timestamp
  | .ok "json" => pure .json
  | .ok other => throw s!"unknown type {other}"
  | .error _ =>
    if let some e := optField j "enum" then do
      return .enum (← (← e.getArr?).toList.mapM (·.getStr?))
    else if let some r := optField j "record" then do
      return .record (← r.getNat?)
    else if let some t := optField j "list" then do
      return .list (← readTy t)
    else if let some t := optField j "opt" then do
      return .opt (← readTy t)
    else throw "unknown type"

def readRange (j : Option Json) : Except String (Option Range) :=
  match j with
  | none => pure none
  | some r => do
    let lo ← match optField r "lo" with
      | some v => some <$> v.getInt?
      | none => pure none
    let hi ← match optField r "hi" with
      | some v => some <$> v.getInt?
      | none => pure none
    return some { lo, hi }

def readField (j : Json) : Except String Field := do
  return { name := ← (← field j "name").getStr?, ty := ← readTy (← field j "ty"), range := ← readRange (optField j "range") }

partial def readExpr (j : Json) : Except String Expr := do
  if let some x := optField j "var" then
    return .var (← x.getStr?) (← (← arrOf j "fields").mapM (·.getStr?))
  if let .ok v := j.getObjVal? "lit" then
    return .lit v
  if let some fs := optField j "record" then
    return .record (← (← fs.getArr?).toList.mapM (fun p => do
      let xs ← p.getArr?
      return (← (xs[0]?.getD Json.null).getStr?, ← readExpr (xs[1]?.getD Json.null))))
  if let some xs := optField j "list" then
    return .list (← (← xs.getArr?).toList.mapM readExpr)
  if let some xs := optField j "interp" then
    return .interp (← (← xs.getArr?).toList.mapM readExpr)
  throw "unknown expression"

def readHErr (j : Json) : Except String HErr :=
  match j.getStr? with
  | .ok "timeout" => pure .timeout
  | .ok "failure" => pure .failure
  | _ => do return .declared (← (← (← field j "declared").getArr?).toList.mapM (·.getStr?))

def readRetrier (j : Json) : Except String Retrier := do
  return { names := ← (← arrOf j "names").mapM (·.getStr?), max := ← (← field j "max").getNat? }

mutual
  partial def readStmt (j : Json) : Except String Stmt := do
    let site ← (← field j "site").getNat?
    let line ← (← field j "line").getNat?
    if let some c := optField j "call" then
      let target ← match optField c "target" with
        | none => pure none
        | some t => match optField t "let" with
          | some x => do pure (some (Target.letVar (← x.getStr?)))
          | none => do pure (some (Target.case (← (← field t "case").getNat?)))
      let cl ← field c "callee"
      let callee ← match optField cl "task" with
        | some t => Callee.task <$> t.getNat?
        | none => do pure (Callee.rule (← (← field cl "rule").getNat?))
      let handlers ← (← arrOf c "handlers").mapM (fun h => do
        return Handler.mk (← (← arrOf h "errors").mapM readHErr) (← readBlock (← field h "body")))
      return .call site line target callee handlers
    if let some a := optField j "assign" then
      return .assign site (← (← field a "name").getStr?) (← readExpr (← field a "expr"))
    if let some m := optField j "match" then
      let arms ← (← arrOf m "arms").mapM (fun a => do
        let some_ ← match optField a "some" with
          | some y => some <$> y.getStr?
          | none => pure none
        return Arm.mk (← (← arrOf a "values").mapM (·.getStr?)) (← (← field a "none").getBool?) some_
          (← readBlock (← field a "body")))
      return .matchOn site line (← readExpr (← field m "expr")) (← (← field m "shown").getStr?) arms
    if (optField j "wait").isSome then
      return .wait site
    if let some r := optField j "repeat" then
      return .repeat site (← (← field r "times").getNat?) (← readBlock (← field r "body"))
    if let some fr := optField j "for" then
      let result ← match optField fr "result" with
        | some r => do pure (some ((← (← field r "name").getStr?), (← readExpr (← field r "yield"))))
        | none => pure none
      let parallel ← match optField fr "parallel" with
        | some ls => do pure (some (← (← ls.getArr?).toList.mapM (·.getStr?)))
        | none => pure none
      return .forEach site line (← (← field fr "var").getStr?) (← readExpr (← field fr "list"))
        (← (← field fr "max").getNat?) (← readBlock (← field fr "body")) result parallel
    if (optField j "break").isSome then
      return .brk site
    if (optField j "pass").isSome then
      return .pass site
    if let some s := optField j "succeed" then
      return .succeed site (← (← arrOf s "fields").mapM (fun p => do
        let xs ← p.getArr?
        return (← (xs[0]?.getD Json.null).getStr?, ← readExpr (xs[1]?.getD Json.null))))
    if let some fl := optField j "fail" then
      let cause ← match optField fl "cause" with
        | some c => some <$> readExpr c
        | none => pure none
      return .fail site (← (← field fl "error").getStr?) cause (← (← arrOf fl "leaving").mapM (·.getNat?))
    throw "unknown statement"

  partial def readBlock (j : Json) : Except String (List Stmt) := do
    (← j.getArr?).toList.mapM readStmt
end

def readFlow (j : Json) : Except String Flow := do
  let onFailure ← match optField j "on_failure" with
    | some b => some <$> readBlock b
    | none => pure none
  let onCancel ← match optField j "on_cancel" with
    | some b => some <$> readBlock b
    | none => pure none
  return {
    records := ← (← arrOf j "records").mapM (fun r => do (← r.getArr?).toList.mapM readField),
    inputs := ← (← arrOf j "inputs").mapM readField,
    tasks := ← (← arrOf j "tasks").mapM (fun t => do
      return ({ name := ← (← field t "name").getStr?,
                result := ← (match optField t "result" with
                  | some r => some <$> readTy r
                  | none => pure none),
                range := ← readRange (optField t "range"),
                retriers := ← (← arrOf t "retriers").mapM readRetrier,
                effect := .none, refusedAs := none } : Task)),
    rules := ← (← arrOf j "rules").mapM (fun r => do
      return ({ name := ← (← field r "name").getStr?, outputs := ← (← field r "outputs").getNat?,
                retriers := ← (← arrOf r "retriers").mapM readRetrier } : Rule)),
    cases := ← (← arrOf j "cases").mapM (fun c => do
      return ({ name := ← (← field c "name").getStr?, stateField := ← (← field c "state_field").getStr? } : Case)),
    monitors := ← (← arrOf j "monitors").mapM (fun m => do
      let xs ← m.getArr?
      return (← (xs[0]?.getD Json.null).getNat?, ← (← (xs[1]?.getD Json.null).getArr?).toList.mapM (·.getStr?))),
    flow := ← readBlock (← field j "flow"),
    onFailure, onCancel,
    service := ((optField j "service").bind (fun b => b.getBool?.toOption)).getD false }

/-- What dandori reads the answers as, where it reads them before the flow sees them:
    `[place, {"task": t} | {"rule": r}, {"value": …} | {"error": …, "cause": …}]`. -/
def readReads (sc : Json) : List ((Nat × Callee) × Json) :=
  match sc.getObjValD "reads" with
  | .arr xs => xs.toList.filterMap (fun x => do
      let a ← x.getArr?.toOption
      let place ← (a[0]?.getD Json.null).getNat?.toOption
      let c := a[1]?.getD Json.null
      let callee ← match optField c "task", optField c "rule" with
        | some t, _ => (t.getNat?.toOption).map Callee.task
        | _, some r => (r.getNat?.toOption).map Callee.rule
        | _, _ => none
      pure ((place, callee), a[2]?.getD Json.null))
  | _ => []

/-- A scenario as `dandori scenarios` writes it — the input and the answers — with what dandori
    reads some answers as. -/
def answer (f : Flow) (sc : Json) : Json :=
  let answers := match sc.getObjValD "answers" with
    | .arr xs => xs.toList
    | _ => []
  match run f (sc.getObjValD "input") answers (readReads sc) with
  | .ok st => runJson f st
  | .error e => Json.mkObj [("error", Json.str e)]

end DandoriCore
