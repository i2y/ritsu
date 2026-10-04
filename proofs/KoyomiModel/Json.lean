/-
  Reading a dates or calendar file as the Rust tests write it, and answering one input a line.
  Plumbing: it proves nothing. The inputs are many — every input of every range, millions of
  lines — so a line is read as the array of integers it is, without a JSON parser.
-/
import Lean.Data.Json
import KoyomiModel.Ops

namespace KoyomiModel
open Lean

def field (j : Json) (k : String) : Except String Json := j.getObjVal? k

def optField (j : Json) (k : String) : Option Json :=
  match j.getObjVal? k with
  | .ok v => if v.isNull then none else some v
  | .error _ => none

def arrOf (j : Json) (k : String) : Except String (List Json) := do
  return (← (← field j k).getArr?).toList

def pairOf (j : Json) : Except String (Int × Int) := do
  let xs ← j.getArr?
  return (← (xs[0]?.getD Json.null).getInt?, ← (xs[1]?.getD Json.null).getInt?)

def readRules (j : Json) : Except String Rules := do
  let (dataFrom, dataTo) ← pairOf (← field j "data")
  let offset ← match optField j "offset" with
    | some o => some <$> o.getInt?
    | none => pure none
  let every ← (← arrOf j "every").mapM (fun e => do
    let xs ← e.getArr?
    let n (i : Nat) : Except String Nat := (xs[i]?.getD Json.null).getNat?
    return ({ fromMD := (← n 0, ← n 1), toMD := (← n 2, ← n 3) } : Every))
  return { dataFrom, dataTo, offset,
           weekly := (← (← arrOf j "weekly").mapM (·.getBool?)).toArray,
           every,
           days := ← (← arrOf j "days").mapM pairOf,
           opens := ← (← arrOf j "opens").mapM pairOf,
           holidays := (← (← arrOf j "holidays").mapM (·.getInt?)).toArray }

def readA (j : Json) : Except String A :=
  match optField j "input" with
  | some k => A.input <$> k.getNat?
  | none => do return .lit (← (← field j "lit").getInt?)

def readMissing (j : Option Json) : Except String Missing :=
  match j with
  | none => pure .never
  | some m => match m.getStr? with
    | .ok "end_of_month" => pure .endOfMonth
    | .ok "start_of_next_month" => pure .startOfNextMonth
    | .ok "reject" => pure .reject
    | _ => throw "unknown else"

def readConv (s : String) : Except String Conv :=
  match s with
  | "following" => pure .following
  | "preceding" => pure .preceding
  | "modified_following" => pure .modifiedFollowing
  | "modified_preceding" => pure .modifiedPreceding
  | _ => throw s!"unknown convention {s}"

partial def readOp (j : Json) : Except String Op := do
  if let some o := optField j "days" then
    return .days (← (← field o "sign").getInt?) (← readA (← field o "n"))
  if let some o := optField j "business" then
    return .business (← (← field o "forward").getBool?) (← readA (← field o "n"))
  if let some o := optField j "months" then
    return .months (← (← field o "sign").getInt?) (← readA (← field o "n")) (← (← field o "per").getInt?)
      (← readMissing (optField o "missing"))
  if let some o := optField j "day_of_month" then
    return .dayOfMonth (← readA (← field o "n")) (← (← field o "sign").getInt?) (← readA (← field o "k"))
      (← readMissing (optField o "missing"))
  if let some o := optField j "start_of_month" then
    return .startOfMonth (← (← field o "sign").getInt?) (← readA (← field o "k"))
  if let some o := optField j "end_of_month" then
    return .endOfMonth (← (← field o "sign").getInt?) (← readA (← field o "k"))
  if let some o := optField j "close_day" then
    return .closeDay (← readA (← field o "n")) (← readMissing (optField o "missing"))
  if (optField j "close_end_of_month").isSome then
    return .closeEndOfMonth
  if let some c := optField j "roll" then
    return .roll (← readConv (← c.getStr?))
  if let some i := optField j "if_closed" then
    return .ifClosed (← readOp i)
  throw "unknown operation"

def readDate (j : Json) : Except String DateDecl := do
  let s ← field j "start"
  let start ← match optField s "date" with
    | some k => Start.date <$> k.getNat?
    | none => pure Start.input
  let at? ← match optField j "at" with
    | some a => some <$> a.getInt?
    | none => pure none
  return { start, ops := ← (← arrOf j "ops").mapM readOp, at? }

def readInput (j : Json) : Except String Input := do
  return { lo := ← (← field j "lo").getInt?, hi := ← (← field j "hi").getInt?,
           taken := ← (← field j "taken").getBool? }

/-- A dates file, or a calendar file whose inputs are days. -/
inductive File where
  | dates (f : DatesFile)
  | calendar (c : Rules)

def readFile (j : Json) : Except String File := do
  match ← (← field j "kind").getStr? with
  | "calendar" => return .calendar (← readRules (← field j "calendar"))
  | _ =>
    let calendar ← match optField j "calendar" with
      | some c => some <$> readRules c
      | none => pure none
    return .dates { inputs := (← (← arrOf j "inputs").mapM readInput).toArray,
                    dateInput := ← (← field j "date_input").getNat?,
                    dates := (← (← arrOf j "dates").mapM readDate).toArray,
                    order := ← (← arrOf j "order").mapM (·.getNat?),
                    calendar }

/-- `[20180,3]`: the integers of a line. -/
def readInts (line : String) : Option (Array Int) :=
  let body := ((line.trimAscii.toString.drop 1).dropEnd 1).toString
  if body.trimAscii.isEmpty then some #[] else
  (body.splitOn ",").foldlM (fun acc t => (t.trimAscii.toString.toInt?).map acc.push) #[]

def quote (s : String) : String := "\"" ++ s ++ "\""

/-- What one line gives: the values as a JSON array of strings, `{"open":…}` for a calendar, or
    `{"error":"<kind>"}`. -/
def answerLine (f : File) (line : String) : String :=
  match readInts line with
  | none => "{\"error\":\"unreadable line\"}"
  | some vals =>
    match f with
    | .calendar c =>
      match c.isOpen (vals[0]!) with
      | .ok o => s!"\{\"open\":{o}}"
      | .error e => s!"\{\"error\":\"{e.kind}\"}"
    | .dates d =>
      match d.expect vals with
      | .ok vs => "[" ++ ",".intercalate (vs.map quote) ++ "]"
      | .error k => s!"\{\"error\":\"{k}\"}"

end KoyomiModel
