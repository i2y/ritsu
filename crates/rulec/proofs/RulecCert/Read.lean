/-
  Reading a certificate (DESIGN §15.97).

  Everything in this file is plumbing: JSON in, the structures `RulecCert.Certified` and
  `RulecCert.Values` are stated over, out. It proves nothing and is trusted to the extent
  that a misreading can only make the checker **refuse** a certificate it should accept —
  every field it cannot make sense of becomes a `none`, and a `none` reaching a check makes
  that check fail. The one thing a misreading here could do is check the wrong document, and
  that is what the digest and the cell spans are for.
-/
import Lean
import RulecCert.Certified
import RulecCert.Cells
import RulecCert.Values

namespace RulecCert
open Lean

/-! ## Numbers -/

/-- A rational as the certificate writes it: `"7/2"`, `"3"`, `"-5"`. -/
def ratOfString (s : String) : Option Rat :=
  match s.splitOn "/" with
  | [a] => (a.trimAscii.toInt?).map (fun n => (n : Rat))
  | [a, b] => do
      let n ← a.trimAscii.toInt?
      let d ← b.trimAscii.toInt?
      if d = 0 then none else some ((n : Rat) / (d : Rat))
  | _ => none

/-- A number as JSON writes it, or as a string. Booleans and enum names have no number,
    and the reader gives them zero — nothing in the sieve reads a coordinate that has no
    span, so the value is never looked at. -/
def ratOfJson : Json → Option Rat
  | .num n => some ((n.mantissa : Rat) / ((10 ^ n.exponent : Nat) : Rat))
  | .str s => ratOfString s
  | .bool _ => some 0
  | .null => none
  | _ => none

def optRat : Json → Option Rat
  | .null => none
  | j => ratOfJson j

/-! ## Small helpers over `Lean.Json` -/

def field (j : Json) (k : String) : Option Json := (j.getObjVal? k).toOption
def arr (j : Json) : Option (Array Json) := (j.getArr?).toOption
def str (j : Json) : Option String := (j.getStr?).toOption
def nat (j : Json) : Option Nat := (j.getNat?).toOption

def fieldArr (j : Json) (k : String) : Array Json :=
  (field j k >>= arr).getD #[]

def fieldStr (j : Json) (k : String) : String := (field j k >>= str).getD ""

def fieldNat (j : Json) (k : String) : Option Nat := field j k >>= nat

/-- The pairs of an object, in the order it was written. -/
def objPairs (j : Json) : List (String × Json) :=
  match j with
  | .obj m => m.toArray.toList.map (fun p => (p.1, p.2))
  | _ => []

/-! ## Types -/

/-- `"money[円,税込]"`, `"qty[個]"`, `"rate"`, `"number"`. Anything else is unknown, and a
    value whose type cannot be read is reported rather than checked. -/
def tyOfString (s : String) : Option Ty :=
  if s == "rate" then some .rate
  else if s == "number" then some .number
  else if s.startsWith "money[" && s.endsWith "]" then
    let inner := ((s.drop 6).dropEnd 1).toString
    match inner.splitOn "," with
    | [c] => some (.money c none)
    | [c, t] => some (.money c (some t))
    | _ => none
  else if s.contains '[' && s.endsWith "]" then some (.dim s)
  -- `bool`, `date`, `string`, the name of an enum: no dimension, no interval, and it meets
  -- only a type written the same way.
  else if s.isEmpty then none
  else some (.other s)

/-! ## Expressions -/

def cmpOfString (s : String) : Option Cmp :=
  if s == "<=" then some .le else if s == "<" then some .lt
  else if s == ">=" then some .ge else if s == ">" then some .gt
  else if s == "=" then some .eq else none


partial def exprOfJson (j : Json) : Option Expr :=
  match field j "name" >>= str with
  | some n => some (.name n)
  | none =>
    match field j "num" with
    | some _ => do
        let v ← (field j "value" >>= optRat)
        let t := (field j "type" >>= str >>= tyOfString).getD Ty.number
        some (.lit v t)
    | none =>
    match field j "lit" with
    | some _ => some (match field j "type" >>= str >>= tyOfString with
                      | some t => Expr.typed t
                      | none => Expr.unread)
    | none =>
      match field j "op" >>= str with
      | some op => do
          let l ← field j "l" >>= exprOfJson
          let r ← field j "r"
          if op == "+" then some (.add l (← exprOfJson r))
          else if op == "-" then some (.sub l (← exprOfJson r))
          else if op == "*" then some (.mul l (← exprOfJson r))
          else if op == "/" then do
            -- §2.3 (E115): the divisor is a constant, so the checker only knows that form.
            let k ← field r "value" >>= optRat
            let kt := (field r "type" >>= str >>= tyOfString).getD Ty.number
            some (.divc l k kt)
          else
            match cmpOfString (if op == "==" then "=" else op) with
            | some c => some (.compare c l (← exprOfJson r))
            | none => none
      | none =>
        match field j "call" >>= str with
        | some c => do
            let args := fieldArr j "args"
            let a ← args[0]? >>= exprOfJson
            if c == "min" && args.size == 2 then some (.minOf a (← args[1]? >>= exprOfJson))
            else if c == "max" && args.size == 2 then some (.maxOf a (← args[1]? >>= exprOfJson))
            -- A share (§15.102). Its three arguments are names — E117 accepts nothing
            -- else — but nothing here depends on that: what the interval needs is the
            -- guarantee, and that is looked up by name where the two are names.
            else if c == "allocate" && args.size == 3 then do
              let b ← args[1]? >>= exprOfJson
              let w ← args[2]? >>= exprOfJson
              some (.alloc a b w)
            else if args.size == 2 then do
              let g ← args[1]? >>= fun x => field x "value" >>= optRat
              some (.roundTo a g)
            else none
        | none => none

/-- A truth value, as JSON writes one. -/
def boolOf : Json → Option Bool
  | .bool b => some b
  | _ => none

/-- One inequality of a refutation and its multiplier (§15.141): a fact of the model by its
    index, or an end of the coordinates a box allows on an axis. -/
def refOfJson (j : Json) : Option (Ref × Rat) := do
  let y ← field j "y" >>= optRat
  match fieldNat j "fact" with
  | some i => some (.fact i, y)
  | none => do
    let c ← field j "coord"
    let ai ← fieldNat c "axis"
    let hi ← field c "hi" >>= boolOf
    let at_ ← field c "at" >>= optRat
    let opn ← field c "open" >>= boolOf
    some (.coord ai hi at_ opn, y)

/-- A refutation, as the certificate writes it. -/
def refsOfJson (j : Json) : Option (List (Ref × Rat)) :=
  (arr j) >>= (fun a => a.toList.mapM refOfJson)

/-- The cover's leaves the linear model rules out, with the path to each. A leaf whose
    refutation cannot be read is left out, and then nothing rules its box out: the cover
    check fails there rather than passing over it. -/
partial def farkasLeaves (c : Json) (path : Point) : List (Point × List (Ref × Rat)) :=
  match field c "split" >>= arr with
  | some kids => ((List.range kids.size).zip kids.toList).flatMap (fun (i, k) => farkasLeaves k (path ++ [i]))
  | none =>
    match field c "farkas" >>= refsOfJson with
    | some rs => [(path, rs)]
    | none => []

/-- Coefficients over numbered values, from a linear form over names: `idx` says which number
    a name has, and a name with none leaves the form unreadable. -/
def denseOf (idx : String → Option Nat) (terms : List (String × Rat)) : Option (List Rat) :=
  terms.foldlM (fun acc t => do let i ← idx t.1; some (vadd acc (unitAt i t.2))) []

/-! ## One table -/

/-- The span a pair of bounds describes: equal ends are one value, unequal ends the open
    interval between them (§6.2). A `null` is a coordinate with no number at all. -/
def coordOfJson (j : Json) : Option Coord :=
  match arr j with
  | some a =>
    match a[0]?, a[1]? with
    | some lo, some hi =>
      let l := optRat lo
      let h := optRat hi
      match l, h with
      | some x, some y => if x = y then some (.exactly x) else some (.between l h)
      | _, _ => some (.between l h)
    | _, _ => none
  | none => none

/-- Whether the cover written in the document rests anywhere on the tables above. The
    reading below turns such a leaf into an ordinary impossible box, settled by the facts
    this table states about those tables, so the shape alone no longer says it. -/
partial def leansAbove : Json → Bool
  | c =>
    match field c "split" >>= arr with
    | some kids => kids.toList.any leansAbove
    | none => (field c "upstream").isSome

mutual

/-- The cover as the certificate writes it. The three impossible leaves — a constraint, a
    derive out of reach, every point ruled out one at a time — become the same node here:
    the checker recomputes which of them holds rather than taking the hint. -/
partial def coverOfJson : Option Json → Option Cover
  | none => none
  | some c =>
    match field c "split" >>= arr with
    | some kids => (kidsOfJson kids.toList).map Cover.split
    | none =>
      match fieldNat c "row" with
      | some i => some (.row i)
      | none =>
        if (field c "upstream").isSome || (field c "constraint").isSome
          || (field c "derived_axis").isSome || (field c "every_point_ruled_out").isSome
          || (field c "farkas").isSome
        then some .impossible else none

partial def kidsOfJson : List Json → Option Kids
  | [] => some .nil
  | k :: ks => do
      let a ← coverOfJson (some k)
      let b ← kidsOfJson ks
      some (.cons a b)

end

/-- Where a cell stands in the file, and what the certificate says stands there. -/
structure SrcSpan where
  line : Nat
  col : Nat
  len : Nat
  text : String

/-- One row, as the cell check reads it. -/
structure ReadRow where
  index : Nat
  origin : String
  /-- The line the row itself is written on; 0 for one an `apply` brought in. -/
  line : Nat
  tests : List CellTest
  accepts : List (List Nat)
  /-- `none` for a row an `apply` brought in; `none` inside for a column the row has no
      cell in. -/
  source : Option (List (Option SrcSpan))

structure ReadTable where
  name : String
  /-- How many columns the table writes to the right of `->`. -/
  outputs : Nat
  cert : Certified
  /-- Column names, in axis order; the report and the cell check use them. -/
  columns : List String
  /-- What each coordinate of each axis is written as. -/
  labels : List (List String)
  /-- What each coordinate of each axis stands for, where it stands for a number. -/
  spans : List (List (Option Coord))
  rowsRaw : List ReadRow
  /-- Rows for which the certificate states no point because the tool called them unused. -/
  unused : List Nat
  /-- Rows the sieve rules out entirely. Stated, not proved. -/
  ruledOut : List Nat
  /-- Whether every reach point comes with the values behind it. Without them the claim
      falls back to the weaker reading, and the program says which one it made. -/
  valuesStated : Bool
  /-- Per axis: what the column is, the grid its values sit on, and the range the rule
      declares for it. The tiling check needs all three. -/
  kinds : List String
  steps : List (Option Rat)
  /-- Per axis: the prefix each coordinate stands for, on a string axis; empty elsewhere. -/
  prefixes : List (List (Option (List Char)))
  declared : List (Option Span2)
  /-- Leaves the cover rests on an upstream table for. -/
  upstream : Bool

/-- The words a cell names, with the members of any group among them — a cell may write
    the group's name, and the certificate carries what it stands for. -/
def cellWords (groups : String → List String) (j : Json) : List String :=
  let ws := (fieldArr j "words").toList.filterMap str
  ws ++ ws.flatMap groups

def cellOfJson (groups : String → List String) (j : Json) : Option CellTest :=
  match fieldStr j "cell" with
  | "any" => some CellTest.any
  | "none" => some CellTest.nothing
  | "is" => some (CellTest.isIn (cellWords groups j))
  | "not" => some (CellTest.notIn (cellWords groups j))
  | "prefix" => some (CellTest.prefixOf ((fieldArr j "words").toList.filterMap str |>.map String.toList))
  | "cmp" => do
      let ts ← (fieldArr j "tests").toList.mapM (fun x => do
        let op ← cmpOfString (fieldStr x "op")
        let v ← field x "value" >>= optRat
        some (op, v))
      some (CellTest.cmp ts)
  | _ => none

def srcOfJson (j : Json) : Option (List (Option SrcSpan)) :=
  match j with
  | .null => none
  | _ => (arr j).map (fun a => a.toList.map (fun x =>
      match x with
      | .null => none
      | _ => do
          let line ← fieldNat x "line"
          let col ← fieldNat x "col"
          let len ← fieldNat x "len"
          some { line := line, col := col, len := len, text := fieldStr x "text" }))

def readTable (rangesOf : String → Option Span2) (groups : String → List String)
    (declaredOf : String → Option Span2)
    (factOf : Json → Option (List (String × Rat) × Rat × Bool)) (j : Json) : Option ReadTable := do
  let name := fieldStr j "table"
  let policy ← (if fieldStr j "policy" == "unique" then some Policy.unique
                else if fieldStr j "policy" == "first" then some Policy.first else none)
  let axes := fieldArr j "axes"
  let columns := axes.toList.map (fun a => fieldStr a "column")
  let arities := axes.toList.map (fun a => (fieldArr a "coords").size)
  let coords : List (List (Option Coord)) := axes.toList.map (fun a =>
    let bs := fieldArr a "bounds"
    (List.range (fieldArr a "coords").size).map (fun c =>
      match bs[c]? with
      | some b => coordOfJson b
      | none => none))
  -- A constraint counts for this table only when both its columns are axes of it.
  let cons : List Constraint := (fieldArr j "constraints").toList.filterMap (fun k => do
    let l ← columns.idxOf? (fieldStr k "left")
    let r ← columns.idxOf? (fieldStr k "right")
    let op ← cmpOfString (fieldStr k "op")
    some { left := l, op := op, right := r })
  -- A derived column can only produce values its own expression can reach.
  let reach : List (Option Ival) := columns.map (fun n =>
    if (axes.toList.find? (fun a => fieldStr a "column" == n)).map
        (fun a => fieldStr a "kind") == some "derived" then
      (rangesOf n).map (fun I => (some I.1, some I.2))
    else none)
  let rows : List Row := (fieldArr j "rows").toList.filterMap (fun r => do
    let i ← fieldNat r "row"
    let box := (fieldArr r "accepts").toList.map (fun xs =>
      (arr xs).getD #[] |>.toList.filterMap nat)
    some { index := i, box := box })
  let told : List (Nat × Nat × Nat) := (fieldArr j "disjoint").toList.filterMap (fun d => do
    let a ← fieldNat d "a"; let b ← fieldNat d "b"; let ax ← fieldNat d "axis"
    some (a, b, ax))
  let undec : List (Nat × Nat) := (fieldArr j "undecided").toList.filterMap (fun d => do
    let a ← fieldNat d "a"; let b ← fieldNat d "b"; some (a, b))
  -- The table's linear model (§15.141): its facts, each built again from where the
  -- certificate says it comes from, over the axes and then the model's other names.
  let lin := (field j "linear").getD Json.null
  let extra := (fieldArr lin "extra").toList.filterMap str
  let idx : String → Option Nat := fun n =>
    match columns.idxOf? n with
    | some i => some i
    | none => (extra.idxOf? n).map (columns.length + ·)
  let facts ← (fieldArr lin "facts").toList.mapM (fun f => do
    let (terms, k, strict) ← factOf f
    let coeffs ← denseOf idx terms
    some ({ coeffs := coeffs, k := k, strict := strict } : LinIneq))
  let refuted : List (Nat × Nat × List (Ref × Rat)) := (fieldArr j "refuted").toList.filterMap (fun d => do
    let a ← fieldNat d "a"; let b ← fieldNat d "b"
    let rs ← field d "farkas" >>= refsOfJson
    some (a, b, rs))
  let leaves := match field j "cover" with
    | some c => farkasLeaves c []
    | none => []
  let wit : List (Nat × Point × List Rat) := (fieldArr j "reach").toList.filterMap (fun w => do
    let i ← fieldNat w "row"
    let at_ := (fieldArr w "at").toList.filterMap nat
    -- `at_values` is the machine-readable half of the witness: one number per axis, on
    -- the axis's own scale, or `null` for a coordinate that stands for no number. The
    -- model's other names follow, in the order `linear.extra` gives them.
    let vs := (fieldArr w "at_values").toList.map (fun v => (optRat v).getD 0)
    let xs := (fieldArr w "extra_values").toList.map (fun v => (optRat v).getD 0)
    some (i, at_, vs ++ xs))
  let cover ← coverOfJson (field j "cover")
  let rowsRaw : List ReadRow := (fieldArr j "rows").toList.filterMap (fun r => do
    let i ← fieldNat r "row"
    let tests ← (fieldArr r "tests").toList.mapM (cellOfJson groups)
    let accepts := (fieldArr r "accepts").toList.map (fun xs =>
      (arr xs).getD #[] |>.toList.filterMap nat)
    some { index := i, origin := fieldStr r "origin", line := (fieldNat r "line").getD 0
           tests := tests, accepts := accepts, source := (field r "source").bind srcOfJson })
  some {
    name := name
    outputs := (fieldNat j "outputs").getD 0
    columns := columns
    labels := axes.toList.map (fun a => (fieldArr a "coords").toList.filterMap str)
    spans := coords
    rowsRaw := rowsRaw
    unused := (fieldArr j "unused").toList.filterMap nat
    ruledOut := (fieldArr j "unreachable").toList.filterMap nat
    -- The strong reading needs a value behind every coordinate that stands for one. A
    -- `null` where the axis is numeric is the certificate declining to show it, and the
    -- claim falls back to the weaker reading rather than failing.
    valuesStated := (fieldArr j "reach").toList.all (fun w =>
      let vs := fieldArr w "at_values"
      let xs := fieldArr w "extra_values"
      vs.size == axes.size &&
        (List.range axes.size).all (fun ai =>
          match (coords[ai]?).getD [] with
          | [] => true
          | cs => cs.all (·.isNone) || !(vs[ai]!).isNull) &&
        (facts.isEmpty || (xs.size == extra.length && xs.all (fun x => !x.isNull))))
    kinds := axes.toList.map (fun a => fieldStr a "kind")
    steps := axes.toList.map (fun a => field a "step" >>= optRat)
    prefixes := axes.toList.map (fun a =>
      match field a "prefixes" >>= arr with
      | some ps => ps.toList.map (fun x => (str x).map String.toList)
      | none => [])
    declared := columns.map declaredOf
    upstream := leansAbove ((field j "cover").getD Json.null)
    cert := {
      arities := arities, rows := rows, policy := policy
      sieve := { coords := coords, cons := cons, reach := reach
                 never := (fieldArr ((field j "above").getD Json.null) "never").toList.filterMap
                   (fun f => do let a ← fieldNat f "axis"; let c ← fieldNat f "coord"; some (a, c))
                 apart := (fieldArr ((field j "above").getD Json.null) "apart").toList.filterMap
                   (fun f => do
                     let a ← field f "a"; let b ← field f "b"
                     let ai ← fieldNat a "axis"; let ac ← fieldNat a "coord"
                     let bi ← fieldNat b "axis"; let bc ← fieldNat b "coord"
                     some ((ai, ac), (bi, bc)))
                 facts := facts }
      cover := cover
      told := fun a b => (told.find? (fun t => t.1 == a && t.2.1 == b)).map (fun t => t.2.2)
      witness := fun i => (wit.find? (fun w => w.1 == i)).map (fun w => (w.2.1, w.2.2))
      undecided := fun a b => (undec.find? (fun u => u.1 == a && u.2 == b)).isSome
      refuted := fun a b => (refuted.find? (fun t => t.1 == a && t.2.1 == b)).map (fun t => t.2.2)
      farkasAt := fun p => (leaves.find? (fun l => l.1 == p)).map (fun l => l.2) } }

end RulecCert
