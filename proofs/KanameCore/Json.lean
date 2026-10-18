/-
  Reading a module as the Rust test writes it, and answering one line at a time
  (`ritsu-model kaname <module.json>`). Plumbing: it proves nothing.

  A line is a run of a function or the analysis of the module:

  - `{"run": f, "input": [v…], "rules": [[r, [v…], [v…] | null]…], "dates": [[file, date, [[name, n]…], n | null]…]}`
    answers `{"value": v}`, `{"fails": [kind, line]}`, `{"diverged": true}` or `{"fuel": true}`: the
    evaluation of the function with that input, the rules and the dates answering what the
    reference interpreter's rulec and koyomi answered on the same run (a refusal, a date that stops:
    `null`);
  - `{"analysis": true}` answers what the analysis finds of every function: what it answers, its
    places (expression, key, kind, values, whether shown, whether ritsu-cross decides it) and its
    effect, and the values of every expression.

  A rational is written `"p/q"` or `"p"`; an interval `[lo, hi, den]` with `null` for an end or a
  bound that is not there; a value tagged by its kind (`{"n": …}`, `{"d": …}`, `{"r": …}`, `null`
  for none).
-/
import Lean.Data.Json
import KanameCore.Analysis

namespace KanameCore
open Lean

-- ── Reading ─────────────────────────────────────────────────────────────────

def jget (j : Json) (k : String) : Except String Json := j.getObjVal? k

def jarr (j : Json) : Except String (List Json) := do return (← j.getArr?).toList

def jat (xs : List Json) (i : Nat) : Json := xs[i]?.getD Json.null

def readRat (j : Json) : Except String Rat := do
  let s ← j.getStr?
  match s.splitOn "/" with
  | [n] => match n.toInt? with
    | some n => return (n : Rat)
    | none => throw s!"no rational: {s}"
  | [n, d] => match n.toInt?, d.toNat? with
    | some n, some d => if d = 0 then throw s!"no rational: {s}" else return ((n : Rat) / (d : Rat))
    | _, _ => throw s!"no rational: {s}"
  | _ => throw s!"no rational: {s}"

def readOptRat (j : Json) : Except String (Option Rat) :=
  if j.isNull then return none else some <$> readRat j

def readOptNat (j : Json) : Except String (Option Nat) :=
  if j.isNull then return none else some <$> j.getNat?

def readIv (j : Json) : Except String Iv := do
  let xs ← jarr j
  return ⟨← readOptRat (jat xs 0), ← readOptRat (jat xs 1), ← readOptNat (jat xs 2)⟩

def readRange (j : Json) : Except String Range := do
  let xs ← jarr j
  return ⟨← readOptRat (jat xs 0), ← readOptRat (jat xs 1)⟩

def readOptRange (j : Json) : Except String (Option Range) :=
  if j.isNull then return none else some <$> readRange j

def readRel (s : String) : Except String Rel :=
  match s with
  | "lt" => pure .lt
  | "le" => pure .le
  | "eq" => pure .eq
  | _ => throw s!"no relation: {s}"

partial def readAbs (j : Json) : Except String Abs := do
  match j with
  | .str "never" => return .never
  | .str "any" => return .any
  | .str "str" => return .str
  | _ =>
    if let .ok x := j.getObjVal? "num" then return .num (← readIv x)
    if let .ok x := j.getObjVal? "date" then return .date (← readIv x)
    if let .ok x := j.getObjVal? "set" then
      if x.isNull then return .set none
      return .set (some (← (← jarr x).mapM (·.getStr?)))
    if let .ok x := j.getObjVal? "opt" then
      let xs ← jarr x
      return .opt (← (jat xs 0).getBool?) (← readAbs (jat xs 1))
    if let .ok x := j.getObjVal? "list" then
      let xs ← jarr x
      return .list (← readIv (jat xs 0)) (← readAbs (jat xs 1))
    if let .ok x := j.getObjVal? "rec" then
      let xs ← jarr x
      let fs ← (← jarr (jat xs 0)).mapM fun f => do
        let p ← jarr f
        return ((← (jat p 0).getStr?), ← readAbs (jat p 1))
      let rels ← (← jarr (jat xs 1)).mapM fun r => do
        let p ← jarr r
        return ((← (jat p 0).getStr?), ← readRel (← (jat p 1).getStr?), ← (jat p 2).getStr?)
      return .record fs rels
    if let .ok x := j.getObjVal? "tr" then
      let fs ← (← jarr x).mapM fun f => do
        let p ← jarr f
        return ((← (jat p 0).getStr?), ← readAbs (jat p 1))
      return .transfer fs
    throw s!"no abstract value: {j.compress}"

partial def readSlot (j : Json) : Except String Slot := do
  let fields ← (← jarr (← jget j "fields")).mapM fun f => do
    let p ← jarr f
    return ((← (jat p 0).getStr?), ← readSlot (jat p 1))
  return .mk (← (← jget j "list").getBool?) (← readOptRange (← jget j "count")) (← readOptRange (← jget j "range"))
    (← (← jget j "unit01").getBool?) (← (← jget j "record").getBool?) fields

def readConv (j : Json) : Except String (Option Conv) := do
  if j.isNull then return none
  let xs ← jarr j
  return some ⟨← readRat (jat xs 0), ← readRat (jat xs 1), ← (jat xs 2).getNat?, ← (jat xs 3).getNat?⟩

def readOrder (j : Json) : Except String Order := do
  match j with
  | .str "num" => return .num
  | .str "date" => return .date
  | .str "eq" => return .eq
  | _ => return .enum (← (← jarr (← jget j "enum")).mapM (·.getStr?))

def readCmp (s : String) : Except String CmpOp :=
  match s with
  | "=" => pure .eq | "!=" => pure .ne | "<" => pure .lt | "<=" => pure .le | ">" => pure .gt | ">=" => pure .ge
  | _ => throw s!"no comparison: {s}"

def readAr (s : String) : Except String ArOp :=
  match s with
  | "+" => pure .add | "-" => pure .sub | "*" => pure .mul | "/" => pure .div
  | _ => throw s!"no operation: {s}"

def readWay (s : String) : Except String Way :=
  match s with
  | "up" => pure .up | "down" => pure .down | "half_up" => pure .halfUp | "half_down" => pure .halfDown
  | "half_even" => pure .halfEven
  | _ => throw s!"no rounding: {s}"

def readWalk (s : String) : Except String Walk :=
  match s with
  | "count" => pure .count | "sum" => pure .sum | "any" => pure .any | "all" => pure .all | "map" => pure .map
  | "filter" => pure .filter | "first" => pure .first | "max_of" => pure .maxOf | "min_of" => pure .minOf
  | "running_totals" => pure .running
  | _ => throw s!"no builtin: {s}"

def readPat (j : Json) : Except String Pat := do
  match j with
  | .str "none" => return .none
  | _ =>
    if let .ok x := j.getObjVal? "some" then return .some (← x.getStr?)
    if let .ok x := j.getObjVal? "bool" then return .bool (← x.getBool?)
    if let .ok x := j.getObjVal? "enum" then
      if x.isNull then return .enum none
      return .enum (some (← x.getStr?))
    throw s!"no pattern: {j.compress}"

def readFmt (j : Json) : Except String TextFmt := do
  match j with
  | .str "rate" => return .rate
  | .str "number" => return .number
  | .str "plain" => return .plain
  | _ => return .unit (← (← jget j "unit").getStr?)

def nat (xs : List Json) (i : Nat) : Except String Nat := (jat xs i).getNat?
def str (xs : List Json) (i : Nat) : Except String String := (jat xs i).getStr?

partial def readExpr (j : Json) : Except String Expr := do
  let tag ← match j with
    | .obj kvs => match kvs.toArray.toList with
      | [(k, _)] => pure k
      | _ => throw s!"no expression: {j.compress}"
    | _ => throw s!"no expression: {j.compress}"
  let body ← jget j tag
  let xs : List Json := match body with
    | .arr a => a.toList
    | x => [x]
  let e (i : Nat) : Except String Expr := readExpr (jat xs i)
  let args3 (i : Nat) : Except String (List (Nat × Expr × Option Conv)) := do
    (← jarr (jat xs i)).mapM fun a => do
      let p ← jarr a
      return ((← (jat p 0).getNat?), ← readExpr (jat p 1), ← readConv (jat p 2))
  match tag with
  | "num" => return .num (← nat xs 0) (← nat xs 1) (← readRat (jat xs 2))
  | "date" => return .date (← nat xs 0) (← (jat xs 1).getInt?)
  | "str" => return .str (← nat xs 0) (← str xs 1)
  | "bool" => return .bool (← nat xs 0) (← (jat xs 1).getBool?)
  | "none" => return .none (← nat xs 0)
  | "enum" => return .enum (← nat xs 0) (← str xs 1)
  | "var" => return .var (← nat xs 0) (← nat xs 1) (← str xs 2)
  | "field" => return .field (← nat xs 0) (← nat xs 1) (← e 2) (← str xs 3)
  | "neg" => return .neg (← nat xs 0) (← nat xs 1) (← e 2)
  | "not" => return .not (← nat xs 0) (← nat xs 1) (← e 2)
  | "in" => return .inUnit (← nat xs 0) (← nat xs 1) (← e 2) (← readConv (jat xs 3))
  | "arith" => return (.arith (← nat xs 0) (← nat xs 1) (← readAr (← str xs 2)) (← e 3) (← e 4)
      (← readConv (jat xs 5)) (← readConv (jat xs 6)) (← readOptRat (jat xs 7)))
  | "ddiff" => return .ddiff (← nat xs 0) (← e 1) (← e 2)
  | "cmp" => return (.cmp (← nat xs 0) (← nat xs 1) (← readCmp (← str xs 2)) (← e 3) (← e 4)
      (← readConv (jat xs 5)) (← readConv (jat xs 6)) (← readOrder (jat xs 7)) (← readOptRat (jat xs 8))
      (← readOptRat (jat xs 9)))
  | "and" => return .and (← nat xs 0) (← nat xs 1) (← e 2) (← e 3)
  | "or" => return .or (← nat xs 0) (← nat xs 1) (← e 2) (← e 3)
  | "if" => return .ite (← nat xs 0) (← e 1) (← e 2) (← e 3) (← readConv (jat xs 4)) (← readConv (jat xs 5))
  | "match" =>
    let arms ← (← jarr (jat xs 3)).mapM fun a => do
      let p ← jarr a
      return ((← (← jarr (jat p 0)).mapM readPat), ← readExpr (jat p 1), ← readConv (jat p 2))
    return .mtch (← nat xs 0) (← nat xs 1) (← e 2) arms
  | "let" => return .letIn (← str xs 0) (← e 1) (← readConv (jat xs 2)) (← e 3)
  | "block" => return .block (← nat xs 0) (← e 1) (← readConv (jat xs 2))
  | "call" => return .call (← nat xs 0) (← nat xs 1) (← nat xs 2) (← args3 3)
  | "rule" => return .rule (← nat xs 0) (← nat xs 1) (← nat xs 2) (← args3 3)
  | "datecall" => return .dateCall (← nat xs 0) (← nat xs 1) (← nat xs 2) (← str xs 3) (← args3 4)
  | "transfer" =>
    let args ← (← jarr (jat xs 3)).mapM fun a => do
      let p ← jarr a
      return ((← (jat p 0).getStr?), ← readExpr (jat p 1), ← readConv (jat p 2), ← (jat p 3).getNat?,
        ← (jat p 4).getBool?)
    return .transfer (← nat xs 0) (← str xs 1) (← (← jarr (jat xs 2)).mapM (·.getStr?)) args
  | "record" =>
    let fields ← (← jarr (jat xs 3)).mapM fun f => do
      let p ← jarr f
      return ((← (jat p 0).getStr?), ← readSlot (jat p 1), ← (jat p 2).getBool?)
    let inits ← (← jarr (jat xs 4)).mapM fun a => do
      let p ← jarr a
      return ((← (jat p 0).getNat?), ← readExpr (jat p 1), ← readConv (jat p 2), ← (jat p 3).getNat?)
    return .record (← nat xs 0) (← nat xs 1) (← (jat xs 2).getBool?) fields inits
  | "list" =>
    let elems ← (← jarr (jat xs 1)).mapM fun a => do
      let p ← jarr a
      return ((← readExpr (jat p 0)), ← readConv (jat p 1))
    return .list (← nat xs 0) elems
  | "walk" => return (.walk (← nat xs 0) (← nat xs 1) (← readWalk (← str xs 2)) (← e 3) (← str xs 4) (← e 5)
      (← readConv (jat xs 6)) (← readOrder (jat xs 7)) (← readOptRat (jat xs 8)))
  | "reduce" => return (.reduce (← nat xs 0) (← nat xs 1) (← e 2) (← e 3) (← str xs 4) (← str xs 5) (← e 6)
      (← readConv (jat xs 7)) (← readConv (jat xs 8)))
  | "minmax" => return (.minmax (← nat xs 0) (← nat xs 1) (← (jat xs 2).getBool?) (← e 3) (← e 4)
      (← readConv (jat xs 5)) (← readConv (jat xs 6)) (← readOrder (jat xs 7)))
  | "mod" => return .modOp (← nat xs 0) (← e 1) (← e 2)
  | "isqrt" => return .isqrt (← nat xs 0) (← nat xs 1) (← e 2)
  | "allocate" => return (.allocate (← nat xs 0) (← nat xs 1) (← e 2) (← e 3) (← e 4) (← readConv (jat xs 5))
      (← readConv (jat xs 6)) (← readConv (jat xs 7)))
  | "round" => return (.round (← nat xs 0) (← nat xs 1) (← readWay (← str xs 2)) (← e 3) (← e 4)
      (← readConv (jat xs 5)) (← readConv (jat xs 6)) (← readAbs (jat xs 7)))
  | "count1" => return .count1 (← nat xs 0) (← e 1)
  | "starts_with" => return .startsWith (← nat xs 0) (← e 1) (← e 2)
  | "concat" => return .concat (← nat xs 0) (← (← jarr (jat xs 1)).mapM readExpr)
  | "text" => return .text (← nat xs 0) (← e 1) (← readFmt (jat xs 2))
  | "bad" => return .bad (← nat xs 0) (← nat xs 1) (← (← jarr (jat xs 2)).mapM readExpr)
  | _ => throw s!"no expression: {tag}"

def readParam (j : Json) : Except String Param := do
  return ⟨← (← jget j "name").getStr?, ← readSlot (← jget j "slot"), ← (← jget j "scalar").getBool?,
    ← readAbs (← jget j "abs")⟩

def readFn (j : Json) : Except String Fn := do
  return { name := ← (← jget j "name").getStr?, line := ← (← jget j "line").getNat?,
           params := ← (← jarr (← jget j "params")).mapM readParam, ret := ← readSlot (← jget j "ret"),
           retRanged := ← (← jget j "ranged").getBool?, declared := ← readAbs (← jget j "declared"),
           body := ← readExpr (← jget j "body"), answer := ← (← jget j "answer").getNat?,
           answerLine := ← (← jget j "answer_line").getNat?, conv := ← readConv (← jget j "conv") }

def readRuleInput (j : Json) : Except String RuleInput := do
  let els ← jget j "elements"
  let elements ← if els.isNull then pure none else some <$> (← jarr els).mapM fun c => do
    let p ← jarr c
    return ((← (jat p 0).getStr?), ← readOptRange (jat p 1))
  -- the values an input of an enum held to a range lets in (none where the module says none)
  let vals := (j.getObjVal? "values").toOption.getD Json.null
  let values ← if vals.isNull then pure none else some <$> (← jarr vals).mapM (·.getStr?)
  return { name := ← (← jget j "name").getStr?, range := ← readOptRange (← jget j "range"),
           step := ← readOptRat (← jget j "step"), opt := ← (← jget j "opt").getBool?,
           dateRanged := ← (← jget j "date_ranged").getBool?, elements, values }

def readRule (j : Json) : Except String RuleD := do
  return { name := ← (← jget j "name").getStr?, inputs := ← (← jarr (← jget j "inputs")).mapM readRuleInput,
           outputs := ← (← jarr (← jget j "outputs")).mapM fun o => do
             let p ← jarr o
             return ((← (jat p 0).getStr?), ← readAbs (jat p 1)),
           pres := ← (← jarr (← jget j "pres")).mapM (·.getStr?) }

def readPair (j : Json) : Except String (Option (Int × Int)) := do
  if j.isNull then return none
  let xs ← jarr j
  return some (← (jat xs 0).getInt?, ← (jat xs 1).getInt?)

def readDates (j : Json) : Except String DatesD := do
  return { inputs := ← (← jarr (← jget j "inputs")).mapM fun i => do
             return ({ name := ← (← jget i "name").getStr?, isDate := ← (← jget i "date").getBool?,
                       min := ← (← jget i "min").getInt?, max := ← (← jget i "max").getInt? } : DateInput),
           dates := ← (← jarr (← jget j "dates")).mapM fun d => do
             return ({ name := ← (← jget d "name").getStr?, reads := ← (← jarr (← jget d "reads")).mapM (·.getStr?),
                       hull := ← readPair (← jget d "hull"), days := ← readPair (← jget d "days") } : DateD) }

/-- The verdicts of the checks across the languages, where the module has them: each place by its
    expression and its key, and whether the check holds there. -/
def readVerdicts (j : Json) : Except String (List (Nat × String × Bool)) :=
  match j.getObjVal? "verdicts" with
  | .ok vs => do
    (← jarr vs).mapM fun v => do
      let p ← jarr v
      return ((← (jat p 0).getNat?), ← (jat p 1).getStr?, ← (jat p 2).getBool?)
  | .error _ => return []

def readModule (j : Json) : Except String Module := do
  return { fns := ← (← jarr (← jget j "fns")).mapM readFn,
           rules := ← (← jarr (← jget j "rules")).mapM readRule,
           dates := ← (← jarr (← jget j "dates")).mapM readDates,
           rings := ← (← jarr (← jget j "rings")).mapM fun r => do
             let p ← jarr r
             return ((← (← jarr (jat p 0)).mapM (·.getNat?)), ← (jat p 1).getBool?),
           verdicts := ← readVerdicts j }

partial def readVal (j : Json) : Except String Val := do
  if j.isNull then return .none
  if let .ok x := j.getObjVal? "n" then return .num (← readRat x)
  if let .ok x := j.getObjVal? "b" then return .bool (← x.getBool?)
  if let .ok x := j.getObjVal? "s" then return .str (← x.getStr?)
  if let .ok x := j.getObjVal? "d" then return .date (← x.getInt?)
  if let .ok x := j.getObjVal? "e" then return .enum (← x.getStr?)
  if let .ok x := j.getObjVal? "r" then
    return .record (← (← jarr x).mapM fun f => do
      let p ← jarr f
      return ((← (jat p 0).getStr?), ← readVal (jat p 1)))
  if let .ok x := j.getObjVal? "l" then return .list (← (← jarr x).mapM readVal)
  if let .ok x := j.getObjVal? "t" then
    let p ← jarr x
    return .transfer (← (jat p 0).getStr?) (← (← jarr (jat p 1)).mapM fun f => do
      let q ← jarr f
      return ((← (jat q 0).getStr?), ← readVal (jat q 1)))
  throw s!"no value: {j.compress}"

-- ── Writing ─────────────────────────────────────────────────────────────────

def ratJ (r : Rat) : Json := Json.str (if r.den = 1 then toString r.num else s!"{r.num}/{r.den}")

def endJ : End → Json
  | some r => ratJ r
  | none => Json.null

def ivJ (i : Iv) : Json := Json.arr #[endJ i.lo, endJ i.hi, match i.den with | some d => Json.num d | none => Json.null]

def relJ : Rel → Json
  | .lt => "lt" | .le => "le" | .eq => "eq"

partial def absJ : Abs → Json
  | .never => "never"
  | .any => "any"
  | .str => "str"
  | .num i => Json.mkObj [("num", ivJ i)]
  | .date i => Json.mkObj [("date", ivJ i)]
  | .set none => Json.mkObj [("set", Json.null)]
  | .set (some s) => Json.mkObj [("set", Json.arr (s.map Json.str).toArray)]
  | .opt n a => Json.mkObj [("opt", Json.arr #[Json.bool n, absJ a])]
  | .list c a => Json.mkObj [("list", Json.arr #[ivJ c, absJ a])]
  | .record fs rels => Json.mkObj [("rec", Json.arr #[
      Json.arr (fs.map fun (n, a) => Json.arr #[Json.str n, absJ a]).toArray,
      Json.arr (rels.map fun (x, r, y) => Json.arr #[Json.str x, relJ r, Json.str y]).toArray])]
  | .transfer fs => Json.mkObj [("tr", Json.arr (fs.map fun (n, a) => Json.arr #[Json.str n, absJ a]).toArray)]

partial def valJ : Val → Json
  | .num r => Json.mkObj [("n", ratJ r)]
  | .bool b => Json.mkObj [("b", Json.bool b)]
  | .str s => Json.mkObj [("s", Json.str s)]
  | .date d => Json.mkObj [("d", Json.num d)]
  | .enum v => Json.mkObj [("e", Json.str v)]
  | .record fs => Json.mkObj [("r", Json.arr (fs.map fun (n, v) => Json.arr #[Json.str n, valJ v]).toArray)]
  | .list xs => Json.mkObj [("l", Json.arr (xs.map valJ).toArray)]
  | .none => Json.null
  | .transfer id args => Json.mkObj [("t", Json.arr #[Json.str id,
      Json.arr (args.map fun (n, v) => Json.arr #[Json.str n, valJ v]).toArray])]

def kindsJ (ks : List Kind) : Json := Json.arr (ks.map fun k => Json.str k.word).toArray

-- ── Answering ───────────────────────────────────────────────────────────────

/-- How many steps a run may take: more than any vector of the tests takes. -/
def RUNFUEL : Nat := 100000000

/-- The oracle of one run: what the reference interpreter's rulec and koyomi answered. -/
def readOracle (j : Json) : Except String Oracle := do
  let rules ← (← jarr (← jget j "rules")).mapM fun r => do
    let p ← jarr r
    let outs := jat p 2
    return ((← (jat p 0).getNat?), ← (← jarr (jat p 1)).mapM readVal,
      ← if outs.isNull then pure none else some <$> (← jarr outs).mapM readVal)
  let dates ← (← jarr (← jget j "dates")).mapM fun d => do
    let p ← jarr d
    let given ← (← jarr (jat p 2)).mapM fun g => do
      let q ← jarr g
      return ((← (jat q 0).getStr?), ← (jat q 1).getInt?)
    let day := jat p 3
    return ((← (jat p 0).getNat?), ← (jat p 1).getStr?, given, ← if day.isNull then pure none else some <$> day.getInt?)
  let rule := fun (r : Nat) (ins : List Val) =>
    (rules.find? fun (q, i, _) => q == r && Val.beqList i ins).bind (·.2.2)
  let date := fun (f : Nat) (d : String) (given : List (String × Int)) =>
    (dates.find? fun (g, e, gv, _) => g == f && e == d && gv == given).bind (·.2.2.2)
  return ⟨rule, date⟩

def runLine (M : Module) (j : Json) : Except String Json := do
  let f ← (← jget j "run").getNat?
  let input ← (← jarr (← jget j "input")).mapM readVal
  let O ← readOracle j
  let C : Ctx := ⟨M, O⟩
  let fn := M.fns[f]?.getD default
  let r : R Val :=
    if (fn.params.zip input).any (fun (p, v) => p.slot.outside true v) then failK .input fn.line
    else callFn C RUNFUEL 0 f input
  return match r with
  | .ok v => Json.mkObj [("value", valJ v)]
  | .error (.failed k line) => Json.mkObj [("fails", Json.arr #[Json.str k.word, Json.num line])]
  | .error .diverged => Json.mkObj [("diverged", Json.bool true)]
  | .error .fuel => Json.mkObj [("fuel", Json.bool true)]

def placeJ (p : Place) : Json :=
  Json.arr #[Json.num p.expr, Json.str p.key, Json.str p.kind.word, absJ p.value, Json.bool p.shown,
    Json.bool p.pending]

/-- The relations a call of a rule keeps, as `Programs` hands them over: `(input, "<=", input)`. -/
def keptJ (rels : List (String × Rel × String)) : Json :=
  Json.arr (rels.map fun (a, r, b) =>
    Json.arr #[Json.str a, Json.str (match r with | .lt => "<" | .le => "<=" | .eq => "="), Json.str b]).toArray

/-- The analysis of the module with the verdicts it was given taken (`analyzeWith`): per function its
    result, its places, its effect, and the relations each of its calls of a rule keeps; and the
    values of every expression. -/
def analysisJ (M : Module) : Json :=
  let (facts, effects, vals) := analyzeWith M
  Json.mkObj [
    ("fns", Json.arr ((facts.zip effects).map fun (ff, e) => Json.mkObj [
      ("result", absJ ff.result),
      ("places", Json.arr (ff.places.map placeJ).toArray),
      ("effect", Json.arr #[kindsJ e.fails, kindsJ e.decided, Json.bool e.diverges]),
      ("kept", Json.arr (ff.kept.map fun (i, rels) => Json.arr #[Json.num i, keptJ rels]).toArray)]).toArray),
    ("vals", Json.arr (vals.map fun (i, a) => Json.arr #[Json.num i, absJ a]).toArray)]

def answerLine (M : Module) (j : Json) : Json :=
  let out : Except String Json :=
    if (j.getObjVal? "analysis").isOk then pure (analysisJ M) else runLine M j
  match out with
  | .ok x => x
  | .error e => Json.mkObj [("error", Json.str e)]

end KanameCore
