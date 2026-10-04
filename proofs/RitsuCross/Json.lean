/-
  Reading the checks' inputs as the Rust tests write them, a JSON value a line, and writing what
  each check answers. Plumbing: it proves nothing. Every line is one object with one key, the
  check it asks for:

  ```
  {"corner": {"op": "<=", "left": [lo, hi], "right": [lo, hi], "scales": [sl, sr]}}
  {"x2": {"kind": "relation", "op": "<", "left": {"shown": "x", "range": [lo, hi]}, "right": …, "scales": [sl, sr]}}
  {"x2": {"kind": "days", "days": [d, …], "arg": {"shown": "due.day", "origins": [{"day": [d, …]}, "other", …]}}}
  {"days_fit": {"days": [d, …], "range": [lo, hi]}}
  {"days_given": {"dates": [[d, …], null, …], "other": false, "range": [lo, hi]}}
  {"values": k}
  {"amount_fits": {"out": {"min": n, "max": n, "values": [n, …], "examples": [[n, ex], …]}}}
  {"amounts_given": {"outputs": [out, …], "ranges": [[lo, hi], …], "other": false}}
  {"amounts_hull": {"outputs": [out, …], "ranges": [[lo, hi], …]}}
  {"refusals_met": {"found": [[op, [reason, …]], …], "op": "hold", "handled": [reason, …], "bounds": [reason, …]}}
  {"input_range": {"inputs": [[name, is_date, min, max], …], "input": "received"}}
  {"held_until": {"least": s, "most": s, "expiry": s}}
  ```

  `null` stands for an open end, a range not known, a question not answered (`Found::Undecided`).
  `values` asks for the days of the date `k` of the dates file the program was started with. The
  answers are `"holds"`, `"undecided"`, or `{"fails": …}` with the example; `values` answers
  `{"days": [d, …]}` (ascending, each once, as koyomi's set) or `"undecided"`.
-/
import Lean.Data.Json
import RitsuCross.Relation
import RitsuCross.Borders
import RitsuCross.Days

namespace RitsuCross
open Lean

def jfield (j : Json) (k : String) : Except String Json := j.getObjVal? k

/-- The value under `k`, or `none` when it is missing or `null`. -/
def joptField (j : Json) (k : String) : Option Json :=
  match j.getObjVal? k with
  | .ok v => if v.isNull then none else some v
  | .error _ => none

def joptInt (j : Json) : Except String (Option Int) :=
  if j.isNull then pure none else some <$> j.getInt?

def jarr (j : Json) : Except String (List Json) := do
  return (← j.getArr?).toList

/-- `[lo, hi]`, either end `null`. -/
def readRange (j : Json) : Except String Range := do
  let xs ← jarr j
  return { lo := ← joptInt (xs[0]?.getD Json.null), hi := ← joptInt (xs[1]?.getD Json.null) }

def readOp (s : String) : Except String Op :=
  match s with
  | "<=" => pure .le
  | "<" => pure .lt
  | ">=" => pure .ge
  | ">" => pure .gt
  | _ => throw s!"unknown comparison {s}"

def readScales (j : Json) : Except String (Nat × Nat) := do
  let xs ← jarr j
  return (← (xs[0]?.getD Json.null).getNat?, ← (xs[1]?.getD Json.null).getNat?)

def ints (j : Json) : Except String (List Int) := do
  (← jarr j).mapM (·.getInt?)

/-- `{"day": [d, …]}`, `{"day": null}` (koyomi does not count it), or `"other"`. -/
def readOrigin (j : Json) : Except String Origin := do
  match j.getStr? with
  | .ok _ => return .other
  | .error _ =>
    match joptField j "day" with
    | some d => return .day (.value (← ints d))
    | none => return .day .undecided

def readArg (j : Json) : Except String (Option Arg) := do
  if j.isNull then return none
  let range ← match joptField j "range" with
    | some r => some <$> readRange r
    | none => pure none
  let origins ← match joptField j "origins" with
    | some os => (← jarr os).mapM readOrigin
    | none => pure []
  return some { shown := ← (← jfield j "shown").getStr?, range, origins }

def strings (j : Json) : Except String (List String) := do
  (← jarr j).mapM (·.getStr?)

def intJ (n : Int) : Json := Json.num (JsonNumber.fromInt n)

def optIntJ : Option Int → Json
  | some n => intJ n
  | none => Json.null

def answerJ {ε : Type} (fails : ε → Json) : Answer ε → Json
  | .holds => Json.str "holds"
  | .undecided => Json.str "undecided"
  | .fails e => Json.mkObj [("fails", fails e)]

/-- The answer to one `corner` line: rulec's answer for a relation over two ranges. -/
def cornerLine (j : Json) : Except String Json := do
  let (sl, sr) ← readScales (← jfield j "scales")
  let a := corner (← readOp (← (← jfield j "op").getStr?))
    ⟨← readRange (← jfield j "left"), sl⟩ ⟨← readRange (← jfield j "right"), sr⟩
  return answerJ (fun (l, r) => Json.arr #[intJ l, intJ r]) a

def exampleJ : Example → Json
  | .at l r => Json.mkObj [("at", Json.arr #[intJ l, intJ r])]
  | .same v => Json.mkObj [("same", optIntJ v)]
  | .day d => Json.mkObj [("day", intJ d)]

/-- The answer to one `x2` line: the decision at a call, for one precondition. -/
def x2Line (j : Json) : Except String Json := do
  let kind ← (← jfield j "kind").getStr?
  let pre ← match kind with
    | "relation" => pure (Pre.relation (← readOp (← (← jfield j "op").getStr?)) "left" "right")
    | "sum" => pure Pre.sum
    | "length" => pure Pre.length
    | "days" => pure (Pre.days "input" (← ints (← jfield j "days")))
    | _ => throw s!"unknown precondition {kind}"
  let l ← readArg ((jfield j "left").toOption.getD Json.null)
  let r ← readArg ((jfield j "right").toOption.getD Json.null)
  let a ← readArg ((jfield j "arg").toOption.getD Json.null)
  let (sl, sr) ← match joptField j "scales" with
    | some sc => readScales sc
    | none => pure (1, 1)
  let arg : String → Option Arg := fun n =>
    if n == "left" then l else if n == "right" then r else if n == "input" then a else none
  let scale : String → Nat := fun n => if n == "left" then sl else sr
  return answerJ exampleJ (x2 pre arg scale)

/-- The answer to one `days_fit` line. -/
def daysFitLine (j : Json) : Except String Json := do
  let days ← match joptField j "days" with
    | some d => Found.value <$> ints d
    | none => pure Found.undecided
  return answerJ intJ (daysFit days (← readRange (← jfield j "range")))

/-- The answer to one `days_given` line. -/
def daysGivenLine (j : Json) : Except String Json := do
  let dates ← (← jarr (← jfield j "dates")).mapM (fun d =>
    if d.isNull then pure Found.undecided else Found.value <$> ints d)
  let other ← (← jfield j "other").getBool?
  return answerJ (fun (i, d) => Json.arr #[intJ i, intJ d]) (daysGiven dates other (← readRange (← jfield j "range")))

/-- One output of a rule, as `amount_fits` lines write it: null where rulec does not count it. -/
def readOut (o : Json) : Except String (Found (OutputValues Json)) := do
  if o.isNull then return Found.undecided
  let values ← match joptField o "values" with
    | some vs => some <$> ints vs
    | none => pure none
  let examples ← (← jarr (← jfield o "examples")).mapM (fun e => do
    let xs ← jarr e
    return ((← (xs[0]?.getD Json.null).getInt?), xs[1]?.getD Json.null))
  return Found.value { min := ← joptInt (← jfield o "min"), max := ← joptInt (← jfield o "max"), values, examples }

def readEnds (j : Json) : Except String (List (Option Int × Option Int)) := do
  (← jarr j).mapM (fun r => do
    let xs ← jarr r
    return (← joptInt (xs[0]?.getD Json.null), ← joptInt (xs[1]?.getD Json.null)))

/-- The answer to one `amounts_given` line. -/
def amountsGivenLine (j : Json) : Except String Json := do
  let outputs ← (← jarr (← jfield j "outputs")).mapM readOut
  let a := amountsGiven outputs (← readEnds (← jfield j "ranges")) (← (← jfield j "other").getBool?)
  return answerJ (fun (v, from_) => Json.arr #[intJ v, match from_ with
    | .output i ex => Json.mkObj [("output", Json.arr #[intJ i, ex.getD Json.null])]
    | .range i => Json.mkObj [("range", intJ i)]]) a

/-- The answer to one `amounts_hull` line. -/
def amountsHullLine (j : Json) : Except String Json := do
  let outputs ← (← jarr (← jfield j "outputs")).mapM readOut
  return match amountsHull outputs (← readEnds (← jfield j "ranges")) with
    | some (lo, hi) => Json.arr #[intJ lo, intJ hi]
    | none => Json.null

/-- The answer to one `input_range` line. -/
def inputRangeLine (j : Json) : Except String Json := do
  let inputs ← (← jarr (← jfield j "inputs")).mapM (fun e => do
    let xs ← jarr e
    return ({ name := ← (xs[0]?.getD Json.null).getStr?, isDate := ← (xs[1]?.getD Json.null).getBool?,
              min := ← (xs[2]?.getD Json.null).getInt?, max := ← (xs[3]?.getD Json.null).getInt? } : DateInput))
  return match inputRange inputs (← (← jfield j "input").getStr?) with
    | some (lo, hi) => Json.arr #[intJ lo, intJ hi]
    | none => Json.null

/-- The answer to one `held_until` line. -/
def heldUntilLine (j : Json) : Except String Json := do
  let most ← match joptField j "most" with
    | some m => some <$> m.getNat?
    | none => pure none
  return answerJ (fun (v : Nat) => intJ (v : Int)) (heldUntil (← (← jfield j "least").getNat?) most (← (← jfield j "expiry").getNat?))

/-- The answer to one `amount_fits` line. The examples are handed on as they came. -/
def amountFitsLine (j : Json) : Except String Json := do
  let out ← match joptField j "out" with
    | none => pure Found.undecided
    | some o => do
      let values ← match joptField o "values" with
        | some vs => some <$> ints vs
        | none => pure none
      let examples ← (← jarr (← jfield o "examples")).mapM (fun e => do
        let xs ← jarr e
        return ((← (xs[0]?.getD Json.null).getInt?), xs[1]?.getD Json.null))
      pure (Found.value ({ min := ← joptInt (← jfield o "min"), max := ← joptInt (← jfield o "max"),
                           values, examples } : OutputValues Json))
  return answerJ (fun (v, ex) => Json.arr #[intJ v, ex.getD Json.null]) (amountFits out)

/-- The answer to one `refusals_met` line. -/
def refusalsMetLine (j : Json) : Except String Json := do
  let found ← match joptField j "found" with
    | none => pure Found.undecided
    | some f => do
      let ops ← (← jarr f).mapM (fun e => do
        let xs ← jarr e
        return (← (xs[0]?.getD Json.null).getStr?, ← strings (xs[1]?.getD Json.null)))
      pure (Found.value ops)
  let a := refusalsMet found (← (← jfield j "op").getStr?) (← strings (← jfield j "handled"))
    (← strings (← jfield j "bounds"))
  return answerJ (fun u => Json.arr #[Json.arr (u.unhandled.map Json.str).toArray,
    Json.arr (u.unfound.map Json.str).toArray]) a

/-- Each value once, ascending: how koyomi's set is written. -/
def setOf (xs : List Int) : List Int :=
  let sorted := xs.mergeSort (fun a b => decide (a ≤ b))
  (sorted.foldl (fun (acc : List Int × Option Int) x =>
    match acc.2 with
    | some y => if x == y then acc else (x :: acc.1, some x)
    | none => (x :: acc.1, some x)) ([], none)).1.reverse

/-- The answer to one `values` line, from the runs of the file's dates. -/
def valuesLine (runs : Option (Found (List (Array Int)))) (j : Json) : Except String Json := do
  let k ← j.getNat?
  match runs with
  | none => throw "values asked of no dates file"
  | some r =>
    match daysOfRuns r k with
    | .undecided => return Json.str "undecided"
    | .value ds => return Json.mkObj [("days", Json.arr ((setOf ds).map intJ).toArray)]

/-- What a file of the `cross` program holds: a dates file, as `KoyomiModel.readFile` reads one,
    and koyomi's budget; or nothing, for lines that carry all they ask about. -/
def readCrossFile (j : Json) : Except String (Option (KoyomiModel.DatesFile × Nat)) := do
  match joptField j "dates" with
  | none => return none
  | some d =>
    match ← KoyomiModel.readFile d with
    | .dates f => return some (f, ← (← jfield j "budget").getNat?)
    | .calendar _ => throw "a calendar, not a dates file"

/-- The answer to one line. -/
def answerLine (runs : Option (Found (List (Array Int)))) (j : Json) : Json :=
  let go : Except String Json :=
    if let some x := joptField j "corner" then cornerLine x
    else if let some x := joptField j "x2" then x2Line x
    else if let some x := joptField j "days_fit" then daysFitLine x
    else if let some x := joptField j "values" then valuesLine runs x
    else if let some x := joptField j "days_given" then daysGivenLine x
    else if let some x := joptField j "amount_fits" then amountFitsLine x
    else if let some x := joptField j "amounts_given" then amountsGivenLine x
    else if let some x := joptField j "amounts_hull" then amountsHullLine x
    else if let some x := joptField j "refusals_met" then refusalsMetLine x
    else if let some x := joptField j "input_range" then inputRangeLine x
    else if let some x := joptField j "held_until" then heldUntilLine x
    else throw "unknown check"
  match go with
  | .ok a => a
  | .error e => Json.mkObj [("error", Json.str e)]

end RitsuCross
