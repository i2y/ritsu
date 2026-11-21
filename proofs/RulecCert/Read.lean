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

/-- Two spellings of one unit (rulec's §15.332): ritsu's table of units gives the two the same
    dimension, size and zero, and `check` compares them as one unit. A type is read here with the
    first of each pair as the second. rulec's `tests/units.rs` holds this list to the table. -/
def oneUnit : List (String × String) := [("JPY", "円"), ("mL", "cm3"), ("kL", "m3")]

/-- A type as written, with the unit in its brackets read as the one spelling `oneUnit` names:
    `money[JPY, 税込]` as `money[円, 税込]`, `volume[mL]?` as `volume[cm3]?`. Any other type as
    it is. -/
def spelledTy (s : String) : String :=
  let cs := s.toList
  match cs.idxOf? '[' with
  | none => s
  | some i =>
    let rest := cs.drop (i + 1)
    let unit := rest.takeWhile (fun c => c != ',' && c != ']')
    match oneUnit.find? (fun p => p.1.toList == unit) with
    | some (_, to) => String.ofList (cs.take (i + 1) ++ to.toList ++ rest.drop unit.length)
    | none => s

/-- `"money[円,税込]"`, `"qty[個]"`, `"rate"`, `"number"`. Anything else is unknown, and a
    value whose type cannot be read is reported rather than checked. Two spellings of one unit
    are read as one type (`spelledTy`). -/
def tyOfString (s : String) : Option Ty :=
  let s := spelledTy s
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

/-- One side of a `constraint` line: the names it adds up. A side of one name is written as the
    name, and a side that adds several as the list of them (rulec's §15.216); anything else is no
    side at all. -/
def sideNames (k : Json) (key : String) : List String :=
  match field k key with
  | some (.str s) => [s]
  | some (.arr xs) => xs.toList.filterMap str
  | _ => []

/-! ## Expressions -/

def cmpOfString (s : String) : Option Cmp :=
  if s == "<=" then some .le else if s == "<" then some .lt
  else if s == ">=" then some .ge else if s == ">" then some .gt
  else if s == "=" then some .eq else none


/-- An expression, as the certificate's tree writes it. With `dates`, a date written in the rule is read
    as its day number, on the scale `ranges` gives dates (rulec's §15.196, §15.233); without, as a
    leaf with a type and no number, which is what the units and int64 claims read. -/
partial def exprOfJson (j : Json) (dates : Bool := false) : Option Expr :=
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
    | some _ =>
      if dates && fieldStr j "type" == "date" then do
        let d ← field j "value" >>= optRat
        some (.lit d (Ty.other "date"))
      else
      some (match field j "type" >>= str >>= tyOfString with
            | some t => Expr.typed t
            | none => Expr.unread)
    | none =>
      match field j "op" >>= str with
      | some op => do
          let l ← field j "l" >>= (exprOfJson · dates)
          let r ← field j "r"
          if op == "+" then some (.add l (← exprOfJson r dates))
          else if op == "-" then some (.sub l (← exprOfJson r dates))
          else if op == "*" then some (.mul l (← exprOfJson r dates))
          else if op == "/" then do
            -- §2.3 (E115): the divisor is a constant, so the checker only knows that form.
            let k ← field r "value" >>= optRat
            let kt := (field r "type" >>= str >>= tyOfString).getD Ty.number
            some (.divc l k kt)
          else
            match cmpOfString (if op == "==" then "=" else op) with
            | some c => some (.compare c l (← exprOfJson r dates))
            | none => none
      | none =>
        match field j "call" >>= str with
        | some c => do
            let args := fieldArr j "args"
            -- A rounding of a quotient by a value (rulec's §15.213): the first argument divides by
            -- something that is not a number written in the rule, which is read here as one node.
            let quot : Option (Json × Json) := do
              let q ← args[0]?
              guard (["down", "up", "half_up", "half_down", "half_even"].contains c)
              guard (args.size == 2 && (field q "op" >>= str) == some "/" && (field q "r" >>= fun r => field r "num").isNone)
              let n ← field q "l"
              let d ← field q "r"
              some (n, d)
            match quot with
            | some (nj, dj) => do
              let n ← exprOfJson nj dates
              let d ← exprOfJson dj dates
              let gj ← args[1]?
              let g ← field gj "value" >>= optRat
              let gt := (field gj "type" >>= str >>= tyOfString).getD Ty.number
              some (.divRound n d g gt (RoundMode.ofString c))
            | none => do
            let a ← args[0]? >>= (exprOfJson · dates)
            -- A remainder by a positive whole constant, and a whole square root (rulec's §15.214,
            -- §15.215).
            if c == "mod" && args.size == 2 then do
              let k ← args[1]? >>= fun x => field x "value" >>= optRat
              guard (k.den == 1 && 0 < k.num)
              some (.modc a k.num.toNat)
            else if c == "isqrt" && args.size == 1 then some (.isqrt a) else
            if c == "min" && args.size == 2 then some (.minOf a (← args[1]? >>= (exprOfJson · dates)))
            else if c == "max" && args.size == 2 then some (.maxOf a (← args[1]? >>= (exprOfJson · dates)))
            -- A share (§15.102). Its three arguments are names — E117 accepts nothing
            -- else — but nothing here depends on that: what the interval needs is the
            -- guarantee, and that is looked up by name where the two are names.
            else if c == "allocate" && args.size == 3 then do
              let b ← args[1]? >>= (exprOfJson · dates)
              let w ← args[2]? >>= (exprOfJson · dates)
              some (.alloc a b w)
            -- A rounding, by the mode its name says (rulec's §15.303); one whose name this checker
            -- does not know is read as rounding any way.
            else if args.size == 2 then do
              let g ← args[1]? >>= fun x => field x "value" >>= optRat
              some (.roundTo a g (RoundMode.ofString c))
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

/-- One inequality of a refutation about a row of a table above (rulec's §15.195): what
    `refOfJson` reads, and an end of the row's own box on a numeric column of its table, which
    names the column — `idx` numbers it the way this table numbers its values. -/
def refOfJsonIn (idx : String → Option Nat) (j : Json) : Option (Ref × Rat) :=
  match field j "cond" with
  | some c => do
    let y ← field j "y" >>= optRat
    let n ← field c "name" >>= str
    let vi ← idx n
    let hi ← field c "hi" >>= boolOf
    let at_ ← field c "at" >>= optRat
    let opn ← field c "open" >>= boolOf
    some (.cond vi hi at_ opn, y)
  | none => refOfJson j

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

/-- The cover's leaves that rest on the rows of a table above (rulec's §15.195), with the path to
    each. -/
partial def aboveLeaves (c : Json) (path : Point) : List (Point × Json) :=
  match field c "split" >>= arr with
  | some kids => ((List.range kids.size).zip kids.toList).flatMap (fun (i, k) => aboveLeaves k (path ++ [i]))
  | none =>
    match field c "above_rows" with
    | some a => [(path, a)]
    | none => []

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
          || (field c "farkas").isSome || (field c "days_axis").isSome
          || (field c "define_axis").isSome || (field c "above_rows").isSome
          || (field c "truth_axis").isSome || (field c "never").isSome
          || (field c "narrowed").isSome
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

/-- What a row writes into a column of numbers (rulec's §15.196) or of dates (§15.325), as the cell
    check reads it back: the column, the cell, what the certificate says the cell is — the literal
    as written or the name; nothing for a date, which the certificate gives by its day — and, for a
    literal, the number it stands for. -/
structure WriteCell where
  column : String
  span : Option SrcSpan
  says : String
  value : Option Rat
  isName : Bool
  isDate : Bool := false

/-- One row, as the cell check reads it. -/
structure ReadRow where
  index : Nat
  origin : String
  /-- The file the row is written in, from the rule's directory, when it is not the rule's own: a
      row a contribution adds (rulec's §15.231). `""` for the rule's own file. -/
  file : String := ""
  /-- That file's SHA-256, which the cell check holds the text it reads to. -/
  fileSha : String := ""
  /-- The contribution the row comes from: its own names come into the rule as `<it>:<name>`. -/
  contribution : String := ""
  /-- The line the row itself is written on; 0 for one an `apply` brought in. -/
  line : Nat
  tests : List CellTest
  accepts : List (List Nat)
  /-- `none` for a row an `apply` brought in; `none` inside for a column the row has no
      cell in. -/
  source : Option (List (Option SrcSpan))
  /-- What the row writes into each column of numbers, in the order the table decides them. -/
  writes : List (Option WriteCell) := []
  /-- The word the row writes into each column, in the order the table decides them: an enum's
      value or a truth value; `none` for a number, a name the rule computes, or nothing. -/
  produces : List (Option String) := []
  /-- The axis of the column of its own table the row passes on into each column, where its answer
      cell names one (rulec's §15.261); `none` elsewhere. Empty where the certificate names none. -/
  passes : List (Option Nat) := []
  /-- Where the row writes each of those words, or the name of the column it passes on, for a table
      written in more than one place (rulec's §15.243): the answer cell of that column in the row's
      own file. Empty where the certificate names none. -/
  answers : List (Option SrcSpan) := []

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
  /-- Per axis: whether the column is an input the rule declares optional (`T?`), or a number or a
      date a table above writes or the rule works out that may be absent (§15.325). Its axis starts
      with the absent value, `none`, which stands for no number (rulec's §15.201). -/
  optional : List Bool := []
  /-- The cells of the rule's `never` lines this table reads, one for each axis (rulec's §15.282). -/
  neverCells : List (List CellTest) := []
  /-- Leaves the cover rests on an upstream table for. -/
  upstream : Bool
  /-- Whether a column of this table comes from a table above through a `define` that names its
      value, as an `apply` brings an output in (rulec's §15.322). -/
  fromAbove : Bool := false

/-- The words a cell names, with the members of any group among them — a cell may write
    the group's name, and the certificate carries what it stands for. -/
def cellWords (groups : String → List String) (j : Json) : List String :=
  let ws := (fieldArr j "words").toList.filterMap str
  ws ++ ws.flatMap groups

def cellOfJson (groups : String → List String) (j : Json) (order : List String := []) : Option CellTest :=
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
  | "in" => do
      let vs ← (fieldArr j "values").toList.mapM optRat
      some (CellTest.inVals vs)
  | "not_in" => do
      let vs ← (fieldArr j "values").toList.mapM optRat
      some (CellTest.notInVals vs)
  -- A comparison on an ordered enum (rulec's §15.210): each value it is against, by its place in
  -- the order the certificate gives the column's enum. A value the order does not have is no
  -- cell this program reads.
  | "rank" => do
      let ts ← (fieldArr j "tests").toList.mapM (fun x => do
        let op ← cmpOfString (fieldStr x "op")
        let i ← order.idxOf? (fieldStr x "value")
        some (op, i))
      some (CellTest.rank order ts)
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

/-- The table of a certificate that decides a column, and where the column sits among its outputs. -/
def deciderOf (all : Array Json) (column : String) : Option (Json × Nat) :=
  all.findSome? (fun u =>
    let ds := (fieldArr u "decides").toList.filterMap str
    (ds.findIdx? (· == column)).map (fun i => (u, i)))

/-- Where a column's value comes from (rulec's §15.322): the table that decides it, where the column
    sits among that table's outputs, and the roundings on the way, the innermost first. That is the
    table that decides the column, or — for a `define` that only names another value
    (`define x = a:y`) or puts one on a grid (`define x = down(a:y, 1)`), as an `apply` brings an
    output in — the table that decides that value. `none` for anything else, and past `fuel` steps. -/
def sourceOfIn (all values : Array Json) (opens : List String) :
    Nat → String → List (RoundMode × Rat) → Option (Json × Nat × List (RoundMode × Rat))
  | 0, _, _ => none
  | n + 1, name, rounds =>
    match deciderOf all name with
    | some (u, oi) => some (u, oi, rounds)
    | none =>
      if opens.contains name then none else
      match values.find? (fun v => fieldStr v "name" == name) with
      | none => none
      | some v =>
        if fieldStr v "of" != "define" then none else
        match field v "expr" with
        | none => none
        | some e =>
          match field e "name" >>= str with
          | some m => sourceOfIn all values opens n m rounds
          | none =>
            let args := fieldArr e "args"
            match RoundMode.ofString (fieldStr e "call"), args[0]? >>= (fun a => field a "name" >>= str), args[1]? with
            | some mode, some m, some g =>
              if args.size != 2 || (field g "num").isNone then none else
              match field g "value" >>= optRat with
              | some gv => if 0 < gv then sourceOfIn all values opens n m ((mode, gv) :: rounds) else none
              | none => none
            | _, _, _ => none

/-- `sourceOfIn` over a whole certificate: its tables, its values and its open values. -/
def sourceOf (cert : Json) (column : String) : Option (Json × Nat × List (RoundMode × Rat)) :=
  sourceOfIn (fieldArr cert "tables") (fieldArr cert "values")
    ((fieldArr cert "opens").toList.map (fun o => fieldStr o "name")) 16 column []

/-- What a row of a table above puts into a column (rulec's §15.322): the word it writes, a number
    literal put through the roundings on the way or a date literal as its day number (§15.325), the
    absent value of a column of numbers or dates, or something this program does not follow — a
    name, an expression — which may stand anywhere. -/
inductive Put where
  | word : String → Put
  | num : Rat → Put
  | absent : Put
  | any : Put
  deriving Inhabited

/-- Whether an expression of the certificate is a date written in the rule (rulec's §15.325). -/
def isDateLit (e : Json) : Bool :=
  fieldStr e "type" == "date" && (field e "lit").isSome

/-- What row `row` puts into its output `oi`, read as a column of numbers or not (`numeric`). -/
def putOf (row : Json) (oi : Nat) (rounds : List (RoundMode × Rat)) (numeric : Bool) : Put :=
  let w := (fieldArr row "produces")[oi]? >>= str
  if !numeric then (match w with | some x => .word x | none => .any) else
  if w == some "none" then .absent else
  match (fieldArr row "writes")[oi]? >>= (fun x => field x "expr") with
  | some e =>
    -- A date has no rounding; one said to go through a rounding is not followed.
    if isDateLit e then
      (if rounds.isEmpty then (match field e "value" >>= optRat with | some v => .num v | none => .any) else .any)
    else
    if (field e "num").isNone then .any else
    match field e "value" >>= optRat with
    | some v => .num (rounds.foldl (fun x mg => roundWith mg.1 mg.2 x) v)
    | none => .any
  | none => .any

/-- Whether what a row puts can stand on coordinate `x` of an axis: the coordinate's word, a number
    inside it (the one value of a point, strictly between the ends of an interval), the absent value
    on the coordinate of none. What this program does not follow, and any value of a string axis,
    stands anywhere. -/
def putTakes (ax : Json) (x : Nat) : Put → Bool
  | .any => true
  | p =>
    if !((field ax "prefixes").getD Json.null).isNull then true else
    let bs := fieldArr ax "bounds"
    if bs.any (fun b => !b.isNull) then
      match p, bs[x]? with
      | .absent, some b => b.isNull
      | .num v, some b => match coordOfJson b with
          | some c => coordHolds c v
          | none => false
      | _, _ => false
    else match p with
      | .word w => ((fieldArr ax "coords")[x]? >>= str) == some w
      | _ => false

/-- Every way row `row` of table `u` can fire (rulec's §15.322): the row, joined with each row of the
    table a column of `u`'s comes from — a column this table (`here`) does not cut — that puts what
    the row takes there, and so on above. Each way is the rows in order, each with its table, the
    tables in the order their columns come. `none` past `fuel` tables. -/
def waysIn (src : String → Option (Json × Nat × List (RoundMode × Rat))) (here : List String) :
    Nat → Json → Json → Option (List (List (Json × Json)))
  | 0, _, _ => none
  | n + 1, u, row =>
    let uaxes := (fieldArr u "axes").toList
    let accepts : List (List Nat) := (fieldArr row "accepts").toList.map (fun xs => ((arr xs).getD #[]).toList.filterMap nat)
    (List.range uaxes.length).foldlM (fun acc ua =>
      match uaxes[ua]? with
      | none => some acc
      | some ax =>
        let d := fieldStr ax "column"
        if here.contains d then some acc else
        match src d with
        | none => some acc
        | some (s, soi, rounds) =>
          let numeric := (fieldArr ax "bounds").any (fun b => !b.isNull)
          let mine := (accepts[ua]?).getD []
          match (fieldArr s "rows").toList.foldlM (fun alts r2 =>
              if mine.any (fun x => putTakes ax x (putOf r2 soi rounds numeric)) then
                (waysIn src here n s r2).map (alts ++ ·)
              else some alts) [] with
          | none => none
          | some alts => some (acc.flatMap (fun a => alts.map (a ++ ·)))) [[(u, row)]]

/-- The ways of `waysIn` for row `row` of the table `u` that decides the value, joined besides
    through each column of `u`'s that this table (`this`, cutting the columns `here`) cuts too and a
    table other than these two decides: with a row of that table that puts what the row takes
    there, one step and no further (rulec's §15.325). The box fixes such a column, and the row that
    puts it carries what it takes of the other columns. -/
def waysThrough (src : String → Option (Json × Nat × List (RoundMode × Rat))) (here : List String)
    (this : String) : Nat → Json → Json → Option (List (List (Json × Json)))
  | 0, _, _ => none
  | n + 1, u, row =>
    let uaxes := (fieldArr u "axes").toList
    let accepts : List (List Nat) := (fieldArr row "accepts").toList.map (fun xs => ((arr xs).getD #[]).toList.filterMap nat)
    (List.range uaxes.length).foldlM (fun acc ua =>
      match uaxes[ua]? with
      | none => some acc
      | some ax =>
        let d := fieldStr ax "column"
        match src d with
        | none => some acc
        | some (s, soi, rounds) =>
          let isHere := here.contains d
          if isHere && (fieldStr s "table" == this || fieldStr s "table" == fieldStr u "table") then some acc else
          let numeric := (fieldArr ax "bounds").any (fun b => !b.isNull)
          let mine := (accepts[ua]?).getD []
          match (fieldArr s "rows").toList.foldlM (fun alts r2 =>
              if mine.any (fun x => putTakes ax x (putOf r2 soi rounds numeric)) then
                (if isHere then some [[(s, r2)]] else waysIn src here n s r2).map (alts ++ ·)
              else some alts) [] with
          | none => none
          | some alts => some (acc.flatMap (fun a => alts.map (a ++ ·)))) [[(u, row)]]

/-- What one row of a table above says, read on this table: for each value this table numbers
    (`idx`, where the value's axis here is a number or the value is a name of the model), the
    coordinates the row takes on its axis there; for each axis of words both tables cut, the
    coordinates of this table's axis the row lets in. -/
def stepRead (u row : Json) (idx : String → Option Nat) (columns : List String) (labels : List (List String))
    (numeric : List Bool) : Option (List (Nat × List Coord) × List (Nat × List Nat)) := do
  let uaxes := (fieldArr u "axes").toList
  let accepts : List (List Nat) := (fieldArr row "accepts").toList.map (fun xs => ((arr xs).getD #[]).toList.filterMap nat)
  let nums ← ((List.range uaxes.length).filterMap (fun ua =>
    let ax := (uaxes[ua]?).getD Json.null
    let bs := fieldArr ax "bounds"
    if !bs.any (fun b => !b.isNull) then none else
    match idx (fieldStr ax "column") with
    | none => none
    | some vi =>
      let mine := (columns.idxOf? (fieldStr ax "column")).map (fun aj => (numeric[aj]?).getD false)
      if mine == some false then none else
      some ((((accepts[ua]?).getD []).mapM (fun x => bs[x]? >>= coordOfJson)).map (fun cs => (vi, cs))))).mapM id
  let words := (List.range uaxes.length).filterMap (fun ua => do
    let ax ← uaxes[ua]?
    if (fieldArr ax "bounds").any (fun b => !b.isNull) then none else
    let aj ← columns.idxOf? (fieldStr ax "column")
    if (numeric[aj]?).getD true then none else
    let ulabels := (fieldArr ax "coords").toList.filterMap str
    let lets := ((accepts[ua]?).getD []).filterMap (fun x => ulabels[x]?)
    let mineLabels := (labels[aj]?).getD []
    some (aj, (List.range mineLabels.length).filter (fun c => match mineLabels[c]? with
      | some l => lets.contains l
      | none => false)))
  some (nums, words)

/-- Every way the value of coordinate `ci` of this table's axis `tax` arrives (rulec's §15.195,
    §15.322): a row of the table the column comes from that puts it there — a word, or a number
    that lies in the coordinate once put through the roundings on the way — joined with the rows of
    the tables above it (`waysIn`), all of it read on this table as one row that fires where every
    row of the way does. Each comes with the number of the row that puts the value and the rows the
    way joins, `(table, row)` each, which a reason names, and the way itself. With `through`, the
    ways are joined besides through the columns both tables cut (`waysThrough`, rulec's §15.325).
    `none` when no table decides the column, or a row of it writes something this program cannot
    follow — the ways cannot then be counted. -/
def aboveRowsOf (src : String → Option (Json × Nat × List (RoundMode × Rat))) (this : String) (through : Bool)
    (tax : Json) (ci : Nat)
    (idx : String → Option Nat) (columns : List String) (labels : List (List String)) (numeric : List Bool) :
    Option (List (AboveRow × Nat × List (String × Nat) × List (Json × Json))) := do
  let (u, oi, rounds) ← src (fieldStr tax "column")
  let isNum := (fieldArr tax "bounds").any (fun b => !b.isNull)
  let rows := (fieldArr u "rows").toList
  let puts := rows.map (fun r => putOf r oi rounds isNum)
  if puts.any (fun p => match p with | .any => true | _ => false) then none else
  let writers := (rows.zip puts).filter (fun rp => putTakes tax ci rp.2)
  let ways ← writers.foldlM (fun acc rp =>
    (if through then waysThrough src columns this 32 u rp.1 else waysIn src columns 32 u rp.1).map (acc ++ ·)) []
  ways.mapM (fun way => do
    let (_, head) ← way.head?
    let i ← fieldNat head "row"
    let reads ← way.mapM (fun ur => stepRead ur.1 ur.2 idx columns labels numeric)
    let via ← (way.drop 1).mapM (fun ur => (fieldNat ur.2 "row").map (fun n => (fieldStr ur.1 "table", n)))
    some ({ index := i, nums := reads.flatMap (·.1), words := reads.flatMap (·.2) }, i, via, way))

/-- A coordinate of a numeric axis as a closed interval on the axis's own grid, as rulec's
    `coord_ends` reads it (rulec's §15.325): a point as itself, an interval without its ends, one
    grid step in. `some none` for the absent value; `none` when the coordinate or the grid cannot be
    read. -/
def axisEnds (ax : Json) (x : Nat) : Option (Option (Option Rat × Option Rat)) := do
  let b ← (fieldArr ax "bounds")[x]?
  if b.isNull then return none
  match ← coordOfJson b with
  | .exactly a => return some (some a, some a)
  | .between lo hi =>
    let st ← field ax "step" >>= optRat
    if st ≤ 0 then none else return some (lo.map (· + st), hi.map (· - st))

/-- Whether two closed intervals meet; a missing end runs on. -/
def endsMeet (x y : Option Rat × Option Rat) : Bool :=
  !((match x.2, y.1 with
     | some p, some q => decide (p < q)
     | _, _ => false) ||
    (match x.1, y.2 with
     | some p, some q => decide (q < p)
     | _, _ => false))

/-- The coordinates of axis `ta` a value can stand on that stands on one of the coordinates `mine`
    of axis `fa` — two tables cutting one column, as rulec's `carry` reads it (rulec's §15.325):
    words by the word, numbers where their closed intervals meet, the absent value with the absent
    value only. Every coordinate where either axis is of prefixes, or the two are not both numbers
    or both words. `none` when a coordinate cannot be read. -/
def carryAxis (fa : Json) (mine : List Nat) (ta : Json) : Option (List Bool) := do
  let n := (fieldArr ta "coords").size
  let isPre : Json → Bool := fun a => !((field a "prefixes").getD Json.null).isNull
  let isNum : Json → Bool := fun a => (fieldArr a "bounds").any (fun b => !b.isNull)
  if isPre fa || isPre ta || isNum fa != isNum ta then return List.replicate n true
  if !isNum ta then
    let fl := (fieldArr fa "coords").toList.map (fun c => (str c).getD "")
    let tl := (fieldArr ta "coords").toList.map (fun c => (str c).getD "")
    let words := mine.filterMap (fun c => fl[c]?)
    return (List.range n).map (fun x => match tl[x]? with
      | some w => words.contains w
      | none => false)
  let fends ← mine.mapM (axisEnds fa)
  (List.range n).mapM (fun x => do
    match ← axisEnds ta x with
    | none => return fends.any (·.isNone)
    | some t => return fends.any (fun f => match f with
        | some f => endsMeet f t
        | none => false))

/-- Whether the boxes together take every point of `part`, a box and the part each the coordinates
    it takes on every axis (rulec's §15.325): split on an axis where a box that meets the part takes
    some of the part's coordinates and not all, until one box takes the whole piece or none meets
    it. `none` past `fuel` splits deep. -/
def boxesCover : Nat → List (List Nat) → List (List (List Nat)) → Option Bool
  | 0, _, _ => none
  | fuel + 1, part, boxes =>
    if part.any (·.isEmpty) then some true else
    let meet := boxes.filter (fun b => b.length == part.length && (part.zip b).all (fun pq => pq.1.any (pq.2.contains ·)))
    if meet.isEmpty then some false
    else if meet.any (fun b => (part.zip b).all (fun pq => pq.1.all (pq.2.contains ·))) then some true
    else match meet.head? with
      | none => some false
      | some b =>
        match (List.range part.length).find? (fun a => ((part[a]?).getD []).any (fun x => !((b[a]?).getD []).contains x)) with
        | none => some true
        | some a => do
          let p := (part[a]?).getD []
          let q := (b[a]?).getD []
          let l ← boxesCover fuel (part.set a (p.filter (q.contains ·))) meet
          if !l then return false
          boxesCover fuel (part.set a (p.filter (fun x => !q.contains x))) meet

/-- What row `r` of table `s` takes, on each axis of its table, where the box `bx` of this table
    (its axes `axes`) lets a value stand: the box carried over a column both tables cut, every
    coordinate on one the box does not cut (rulec's §15.325). -/
def rowPart (axes : List Json) (bx : Box) (s r : Json) : Option (List (List Nat)) := do
  let saxes := (fieldArr s "axes").toList
  let cols := axes.map (fun a => fieldStr a "column")
  let acc : List (List Nat) := (fieldArr r "accepts").toList.map (fun xs => ((arr xs).getD #[]).toList.filterMap nat)
  (List.range saxes.length).mapM (fun ua => do
    let ax ← saxes[ua]?
    let lets ← match cols.idxOf? (fieldStr ax "column") with
      | some aj => do
        let fa ← axes[aj]?
        let mine ← bx[aj]?
        carryAxis fa mine ax
      | none => some (List.replicate (fieldArr ax "coords").size true)
    some (((acc[ua]?).getD []).filter (fun x => (lets[x]?).getD false)))

/-- The row of a way a reason names by its place, `step` (rulec's §15.325). -/
def stepAt (way : List (Json × Json)) (e : Json) : Option (Json × Json) := do
  let si ← fieldNat e "step"
  way[si]?

/-- Whether a row of the way takes nothing of the box `bx`: on some axis of its table, no coordinate
    it takes is one the box lets a value stand on (rulec's §15.325). -/
def missesOk (axes : List Json) (bx : Box) (way : List (Json × Json)) (e : Json) : Bool :=
  match stepAt way e >>= (fun sr => rowPart axes bx sr.1 sr.2) with
  | some part => part.any (·.isEmpty)
  | none => false

/-- Whether a row of the way, in a `policy first` table, never decides in the box `bx`: the rows the
    reason names, each above it and in order, take all of what it takes there (rulec's §15.325). -/
def earlierOk (axes : List Json) (bx : Box) (way : List (Json × Json)) (e : Json) : Bool :=
  match stepAt way e with
  | none => false
  | some (s, r) =>
    if fieldStr s "policy" != "first" then false else
    match (field e "rows" >>= arr).bind (fun xs => xs.toList.mapM nat), fieldNat r "row", rowPart axes bx s r with
    | some named, some ri, some part =>
      (named.zip (named.drop 1)).all (fun ab => decide (ab.1 < ab.2)) &&
      match named.mapM (fun x => if x < ri then (fieldArr s "rows").toList.find? (fun q => fieldNat q "row" == some x) else none) with
      | some found =>
        let boxes := found.map (fun q => (fieldArr q "accepts").toList.map (fun xs => ((arr xs).getD #[]).toList.filterMap nat))
        boxesCover (part.foldl (fun n p => n + p.length) 1) part boxes == some true
      | none => false
    | _, _, _ => false

/-- Whether a row of the way, in the table that decides a column this table cuts too, writes a value
    none of the coordinates the box `bx` takes there holds (rulec's §15.325): what the row puts, read
    as the column reads it through the roundings on the way (`src`, `putOf`, `putTakes`). -/
def writesOk (src : String → Option (Json × Nat × List (RoundMode × Rat))) (axes : List Json) (bx : Box)
    (way : List (Json × Json)) (e : Json) : Bool :=
  match stepAt way e, fieldNat e "axis" with
  | some (s, r), some aj =>
    match axes[aj]?, bx[aj]? with
    | some ax, some xs =>
      match src (fieldStr ax "column") with
      | some (u, oi, rounds) =>
        let p := putOf r oi rounds ((fieldArr ax "bounds").any (fun b => !b.isNull))
        fieldStr u "table" == fieldStr s "table" && (match p with | .any => false | _ => true)
          && !(xs.any (fun x => putTakes ax x p))
      | none => false
    | _, _ => false
  | _, _ => false

/-- Whether two rows of the way take no value of a column together (rulec's §15.325): the tables of
    the two steps the reason names, in order, both cut the column on the two axes it names, and no
    coordinate the later row takes there — of those the box `bx` lets a value stand on (`rowPart`) —
    meets one the earlier row takes, carried onto the later table's axis (`carryAxis`). -/
def disjointOk (axes : List Json) (bx : Box) (way : List (Json × Json)) (e : Json) : Bool :=
  match (field e "steps" >>= arr).bind (fun xs => xs.toList.mapM nat),
      (field e "axes" >>= arr).bind (fun xs => xs.toList.mapM nat) with
  | some [i1, i2], some [a1, a2] =>
    match way[i1]?, way[i2]? with
    | some (s1, r1), some (s2, r2) =>
      match (fieldArr s1 "axes")[a1]?, (fieldArr s2 "axes")[a2]?, rowPart axes bx s1 r1, rowPart axes bx s2 r2 with
      | some ax1, some ax2, some p1, some p2 =>
        match carryAxis ax1 ((p1[a1]?).getD []) ax2 with
        | some lets =>
          decide (i1 < i2) && fieldStr ax1 "column" == fieldStr ax2 "column"
            && !(((p2[a2]?).getD []).any (fun y => (lets[y]?).getD false))
        | none => false
      | _, _, _, _ => false
    | _, _ => false
  | _, _ => false

/-- Whether a row of the way cuts a value the rule works out at coordinates the value does not come
    to in the box `bx` (rulec's §15.325): the reason names the step, the axis of that step's table,
    the axes of this table held (`by`, each once), and where the span of one of them is cut into
    pieces, that axis and where each piece after the first starts (`cuts`). `reachOut` is asked with
    the value's name; each held axis with the coordinates the box takes on it and, where the step's
    table cuts the same column, that axis with the coordinates the row takes on it there (`rowPart`),
    and after them each axis of the step's table the reason holds (`row_by`: of numbers, a column
    this table does not cut) with the coordinates the row takes on it; this table's axis of the value
    with the box's coordinates on it, where it has one; the cut, by the held axis's place in `by`; and
    the step's axis of the value with the coordinates the row takes on it where the box lets a value
    stand. It works the value out the way a fact of `narrowed` is. -/
def worksOutOk (reachOut : String → List (Json × List Nat × Option (Json × List Nat)) →
      Option (Json × List Nat) → Option (Nat × List Rat) → Json → List Nat → Bool)
    (axes : List Json) (bx : Box) (way : List (Json × Json)) (e : Json) : Bool :=
  match stepAt way e, fieldNat e "axis", (field e "by" >>= arr).bind (fun xs => xs.toList.mapM nat) with
  | some (s, r), some ua, some by_ =>
    let saxes := (fieldArr s "axes").toList
    match saxes[ua]?, rowPart axes bx s r with
    | some sax, some part =>
      let col := fieldStr sax "column"
      let xs := (part[ua]?).getD []
      let numeric : Json → Bool := fun a => (fieldArr a "bounds").any (fun q => !q.isNull)
      -- What the row takes of a column on the step's table, where that table cuts it.
      let rowOn : String → Option (Json × List Nat) := fun c =>
        (saxes.zipIdx.find? (fun p => fieldStr p.1 "column" == c && numeric p.1)).map
          (fun p => (p.1, (part[p.2]?).getD []))
      let held := by_.mapM (fun j => do
        let ax ← axes[j]?
        let cs ← bx[j]?
        some (ax, cs, rowOn (fieldStr ax "column")))
      -- The axes of the row's own table it holds (`row_by`), each to what the row takes there: of
      -- numbers, of a column this table does not cut.
      let rowBy : Option (List Nat) := match field e "row_by" with
        | none => some []
        | some j => (arr j).bind (fun ys => ys.toList.mapM nat)
      let rowHeld := rowBy.bind (fun ys => ys.mapM (fun uj => do
        let ax ← saxes[uj]?
        if !numeric ax || axes.any (fun a => fieldStr a "column" == fieldStr ax "column") then none
        else some (ax, (part[uj]?).getD [], (none : Option (Json × List Nat)))))
      let own : Option (Json × List Nat) :=
        (axes.zipIdx.find? (fun p => fieldStr p.1 "column" == col && numeric p.1)).bind
          (fun p => (bx[p.2]?).map (fun cs => (p.1, cs)))
      let cuts : Option (Option (Nat × List Rat)) := match field e "cuts" with
        | none => some none
        | some cj => do
          let j ← fieldNat cj "axis"
          let k ← by_.idxOf? j
          let ats ← (fieldArr cj "at").toList.mapM optRat
          some (some (k, ats))
      match held, rowHeld, cuts with
      | some hs, some rs, some ct =>
        !xs.isEmpty && (by_.zip (List.range by_.length)).all (fun p => !(by_.drop (p.2 + 1)).contains p.1)
          && reachOut col (hs ++ rs) own ct sax xs
      | _, _, _ => false
    | _, _ => false
  | _, _, _ => false

def readTable (rangesOf : String → Option Span2) (groups : String → List String)
    (declaredOf : String → Option Span2)
    (factOf : Json → Option (List (String × Rat) × Rat × Bool))
    (daysOf : String → Option (List Rat))
    (truthOf : String → Option (Cmp × Span2 × Span2) := fun _ => none)
    (optionalOf : String → Bool := fun _ => false)
    (orderOf : String → List String := fun _ => [])
    (src : String → Option (Json × Nat × List (RoundMode × Rat)) := fun _ => none)
    (narrowed : List (List (Nat × List Nat)) := [])
    (reachOut : String → List (Json × List Nat × Option (Json × List Nat)) →
      Option (Json × List Nat) → Option (Nat × List Rat) → Json → List Nat → Bool := fun _ _ _ _ _ _ => false)
    (j : Json) : Option ReadTable := do
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
  -- A constraint counts for this table only when every column it names is an axis of it.
  let cons : List Constraint := (fieldArr j "constraints").toList.filterMap (fun k => do
    let l ← (sideNames k "left").mapM columns.idxOf?
    let r ← (sideNames k "right").mapM columns.idxOf?
    let op ← cmpOfString (fieldStr k "op")
    some { left := l, op := op, right := r })
  -- A column the rule computes — a derive, or a `define` of a number (rulec's §15.195) — can
  -- only produce values its own expression can reach: the interval worked out here from it.
  let reach : List (Option Ival) := columns.map (fun n =>
    if (axes.toList.find? (fun a => fieldStr a "column" == n)).map
        (fun a => fieldStr a "kind") != some "input" then
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
  -- The pairs koyomi's days part (rulec's §15.174), each with its axis of days.
  let byDays : List (Nat × Nat × Nat) := (fieldArr j "days_apart").toList.filterMap (fun d => do
    let a ← fieldNat d "a"; let b ← fieldNat d "b"; let ax ← fieldNat d "axis"
    some (a, b, ax))
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
  -- The leaves that rest on the rows of a table above (rulec's §15.195): the rows are read
  -- from that table's part of the document, and each leaf's reasons in the order of those rows.
  let labels := axes.toList.map (fun a => (fieldArr a "coords").toList.filterMap str)
  let numeric := axes.toList.map (fun a => (fieldArr a "bounds").any (fun b => !b.isNull))
  let aboveLs := match field j "cover" with
    | some c => aboveLeaves c []
    | none => []
  -- Whether a leaf's or a pair's reasons join the ways through the columns both tables cut
  -- (rulec's §15.325).
  let throughOf : Json → Bool := fun a => (field a "through" >>= boolOf) == some true
  -- Each value with the ways it arrives, read as rows of this table, and the key each way's reason
  -- is given under: the number of the row that puts the value, and the rows the way joins — with
  -- the way itself, which an `earlier` or a `misses` is read against.
  let aboveKeyed : List (AboveCol × List (Nat × List (String × Nat) × List (Json × Json))) := aboveLs.foldl (fun acc (path, a) =>
    match fieldNat a "axis" with
    | none => acc
    | some ai =>
      match path[ai]? with
      | none => acc
      | some ci =>
        let th := throughOf a
        if acc.any (fun e => e.1.axis == ai && e.1.coord == ci && e.1.through == th) then acc else
        match axes[ai]? with
        | some tax =>
          match aboveRowsOf src name th tax ci idx columns labels numeric with
          | some rows => acc ++ [({ axis := ai, coord := ci, rows := rows.map (·.1), through := th }, rows.map (·.2))]
          | none => acc
        | none => acc) []
  -- The pairs the rows of a table above part (rulec's §15.196): each value both rows take on the
  -- column that table decides, with the rows that write it read from that table, as for a leaf.
  let pairAbove := (fieldArr j "above_apart").toList
  let aboveKeyed : List (AboveCol × List (Nat × List (String × Nat) × List (Json × Json))) := pairAbove.foldl (fun acc d =>
    match fieldNat d "axis" with
    | none => acc
    | some ai => (fieldArr d "at").toList.foldl (fun acc x =>
        match fieldNat x "coord" with
        | none => acc
        | some ci =>
          let th := throughOf ((field x "above_rows").getD Json.null)
          if acc.any (fun e => e.1.axis == ai && e.1.coord == ci && e.1.through == th) then acc else
          match axes[ai]? with
          | some tax =>
            match aboveRowsOf src name th tax ci idx columns labels numeric with
            | some rows => acc ++ [({ axis := ai, coord := ci, rows := rows.map (·.1), through := th }, rows.map (·.2))]
            | none => acc
          | none => acc) acc) aboveKeyed
  -- The rows a way joins, as a reason names them (rulec's §15.322); none for a way of one row.
  let viaOf : Json → List (String × Nat) := fun g => (fieldArr g "via").toList.filterMap (fun x => do
    let v ← arr x
    let n ← v[0]? >>= str
    let r ← v[1]? >>= nat
    some (n, r))
  -- The reasons a leaf or a pair gives for its box `bx`, one for each way the value arrives, in the
  -- order of those ways, each found by the row that puts the value and the rows the way joins. An
  -- `earlier`, a `misses`, a `reach`, a `disjoint` or a `writes` is read only once this program has
  -- found it holds of the way in the box (rulec's §15.325), and stands for that box among those the way
  -- never decides in.
  let axesL := axes.toList
  let reasonsOf : Json → Box → AboveCol → Option (List RowOut) := fun a bx e => do
    let given := (fieldArr a "rows").toList
    let keys ← (aboveKeyed.find? (fun k => k.1.axis == e.axis && k.1.coord == e.coord && k.1.through == e.through)).map (·.2)
    keys.mapM (fun key => do
      let g ← given.find? (fun x => fieldNat x "row" == some key.1 && viaOf x == key.2.1)
      match fieldNat g "clash", field g "earlier", field g "misses", field g "reach", field g "disjoint",
          field g "writes" with
      | some aj, _, _, _, _, _ => some (RowOut.clash aj)
      | none, some ej, _, _, _, _ => if earlierOk axesL bx key.2.2 ej then some RowOut.taken else none
      | none, none, some mj, _, _, _ => if missesOk axesL bx key.2.2 mj then some RowOut.taken else none
      | none, none, none, some rj, _, _ => if worksOutOk reachOut axesL bx key.2.2 rj then some RowOut.taken else none
      | none, none, none, none, some dj, _ => if disjointOk axesL bx key.2.2 dj then some RowOut.taken else none
      | none, none, none, none, none, some wj => if writesOk src axesL bx key.2.2 wj then some RowOut.taken else none
      | none, none, none, none, none, none => do
        let refs ← (field g "farkas" >>= arr) >>= (fun xs => xs.toList.mapM (refOfJsonIn idx))
        some (RowOut.farkas refs))
  -- Each leaf and each value of a pair, with its box and the column of the value it reads.
  let leafBoxes : List (Point × Json × Nat × Nat × Box) := aboveLs.filterMap (fun (path, a) => do
    let ai ← fieldNat a "axis"
    let ci ← path[ai]?
    some (path, a, ai, ci, pathBox arities path))
  let pairBoxes : List (Nat × Nat × Nat × List (Nat × Json × Box)) := pairAbove.filterMap (fun d => do
    let a ← fieldNat d "a"
    let b ← fieldNat d "b"
    let ai ← fieldNat d "axis"
    let ra ← rows.find? (fun r => r.index == a)
    let rb ← rows.find? (fun r => r.index == b)
    let at_ ← (fieldArr d "at").toList.mapM (fun x => do
      let ci ← fieldNat x "coord"
      some (ci, (field x "above_rows").getD Json.null, (pairBox ra.box rb.box).set ai [ci]))
    some (a, b, ai, at_))
  -- The boxes each way never decides in, read off every leaf and pair whose reason for it says so
  -- and holds: `(axis, coordinate, through, the way's place, the box)`.
  let colOf : Nat → Nat → Bool → AboveCol := fun ai ci th =>
    ((aboveKeyed.find? (fun k => k.1.axis == ai && k.1.coord == ci && k.1.through == th)).map (·.1)).getD
      { axis := ai, coord := ci, rows := [], through := th }
  let takenIn : Nat → Nat → Json → Box → List (Nat × Nat × Bool × Nat × Box) := fun ai ci a bx =>
    let th := throughOf a
    match reasonsOf a bx (colOf ai ci th) with
    | some outs => ((List.range outs.length).zip outs).filterMap (fun (k, o) => match o with
        | .taken => some (ai, ci, th, k, bx)
        | _ => none)
    | none => []
  let takenAll : List (Nat × Nat × Bool × Nat × Box) :=
    leafBoxes.flatMap (fun (_, a, ai, ci, bx) => takenIn ai ci a bx) ++
    pairBoxes.flatMap (fun (_, _, ai, at_) => at_.flatMap (fun (ci, x, bx) => takenIn ai ci x bx))
  let aboveCols : List AboveCol := aboveKeyed.map (fun (e, _) =>
    { e with rows := ((List.range e.rows.length).zip e.rows).map (fun (k, r) =>
        { r with taken := takenAll.filterMap (fun t =>
            if t.1 == e.axis && t.2.1 == e.coord && t.2.2.1 == e.through && t.2.2.2.1 == k then some t.2.2.2.2 else none) }) })
  let aboveApartL : List (Nat × Nat × (Nat × List (Nat × Bool × List RowOut))) := pairBoxes.filterMap (fun (a, b, ai, at_) => do
    let outs ← at_.mapM (fun (ci, x, bx) => do
      let th := throughOf x
      let e ← aboveCols.find? (fun e => e.axis == ai && e.coord == ci && e.through == th)
      let rs ← reasonsOf x bx e
      some (ci, th, rs))
    some (a, b, (ai, outs)))
  let aboveAts : List (Point × (Nat × Bool × List RowOut)) := leafBoxes.filterMap (fun (path, a, ai, ci, bx) => do
    let th := throughOf a
    let e ← aboveCols.find? (fun e => e.axis == ai && e.coord == ci && e.through == th)
    let outs ← reasonsOf a bx e
    some (path, (ai, th, outs)))
  let rowsRaw : List ReadRow := (fieldArr j "rows").toList.filterMap (fun r => do
    let i ← fieldNat r "row"
    let tests ← ((fieldArr r "tests").toList.zip columns).mapM (fun (x, col) => cellOfJson groups x (orderOf col))
    let accepts := (fieldArr r "accepts").toList.map (fun xs =>
      (arr xs).getD #[] |>.toList.filterMap nat)
    let decides := (fieldArr j "decides").toList.filterMap str
    let writes := ((fieldArr r "writes").toList.zip (List.range (fieldArr r "writes").size)).map (fun (w, oi) => do
      let e ← field w "expr"
      let isName := (field e "name").isSome
      let isDate := !isName && (field e "num").isNone && isDateLit e
      let says ← if isName then field e "name" >>= str else if isDate then some "" else field e "num" >>= str
      let sp : Option SrcSpan := match field w "source" with
        | some x => do
            let line ← fieldNat x "line"
            let col ← fieldNat x "col"
            let len ← fieldNat x "len"
            some { line := line, col := col, len := len, text := fieldStr x "text" }
        | none => none
      some { column := (decides[oi]?).getD "", span := sp, says := says
             value := field e "value" >>= optRat, isName := isName, isDate := isDate })
    let fileJ := field r "file"
    some { index := i, origin := fieldStr r "origin", line := (fieldNat r "line").getD 0
           file := (fileJ.map (fun x => fieldStr x "path")).getD ""
           fileSha := (fileJ.map (fun x => fieldStr x "sha256")).getD ""
           contribution := (fileJ.map (fun x => fieldStr x "contribution")).getD ""
           tests := tests, accepts := accepts, source := (field r "source").bind srcOfJson
           writes := writes, produces := (fieldArr r "produces").toList.map str
           passes := (fieldArr r "passes").toList.map nat
           answers := (fieldArr r "answers").toList.map (fun x => do
             let line ← fieldNat x "line"
             let col ← fieldNat x "col"
             let len ← fieldNat x "len"
             some { line := line, col := col, len := len, text := fieldStr x "text" }) })
  -- The `never` lines this table reads (rulec's §15.282): each cell read the way a row's is, and
  -- what it takes on the axes it tests, read off the coordinates the way a row's box is.
  let neverCells : List (List CellTest) ← (fieldArr j "nevers").toList.mapM (fun n =>
    ((fieldArr n "tests").toList.zip columns).mapM (fun (x, col) => cellOfJson groups x (orderOf col)))
  let prefixesAt : List (List (Option (List Char))) := axes.toList.map (fun a =>
    (fieldArr a "prefixes").toList.map (fun p => (str p).map String.toList))
  let neverLines : List (List (Nat × List Nat)) := neverCells.map (fun cells =>
    ((List.range cells.length).zip cells).filterMap (fun (ai, cell) =>
      match cell with
      | .any => none
      | .prefixOf ws => some (ai, boxOfPrefix ((prefixesAt[ai]?).getD []) ws)
      | c => some (ai, boxOf ((labels[ai]?).getD []) ((coords[ai]?).getD []) c)))
  some {
    name := name
    outputs := (fieldNat j "outputs").getD 0
    columns := columns
    labels := axes.toList.map (fun a => (fieldArr a "coords").toList.filterMap str)
    spans := coords
    rowsRaw := rowsRaw
    neverCells := neverCells
    unused := (fieldArr j "unused").toList.filterMap nat
    ruledOut := (fieldArr j "unreachable").toList.filterMap nat
    -- The strong reading needs a value behind every coordinate that stands for one. A
    -- `null` where the axis is numeric is the certificate declining to show it, and the
    -- claim falls back to the weaker reading rather than failing.
    valuesStated := (fieldArr j "reach").toList.all (fun w =>
      let vs := fieldArr w "at_values"
      let xs := fieldArr w "extra_values"
      let atJ := fieldArr w "at"
      vs.size == axes.size &&
        (List.range axes.size).all (fun ai =>
          match (coords[ai]?).getD [] with
          | [] => true
          | cs => cs.all (·.isNone) || !(vs[ai]!).isNull ||
              -- The absent value of an optional input has no number behind it (rulec's §15.201).
              (match atJ[ai]? >>= nat with
               | some c => (match cs[c]? with | some none => true | _ => false) &&
                   ((labels[ai]?).getD [])[c]? == some "none"
               | none => false)) &&
        (facts.isEmpty || (xs.size == extra.length && xs.all (fun x => !x.isNull))))
    kinds := axes.toList.map (fun a => fieldStr a "kind")
    steps := axes.toList.map (fun a => field a "step" >>= optRat)
    prefixes := axes.toList.map (fun a =>
      match field a "prefixes" >>= arr with
      | some ps => ps.toList.map (fun x => (str x).map String.toList)
      | none => [])
    declared := columns.map declaredOf
    optional := axes.toList.map (fun a =>
      (fieldStr a "kind" == "input" || (fieldArr a "bounds").any (fun x => !x.isNull)) && optionalOf (fieldStr a "column"))
    upstream := leansAbove ((field j "cover").getD Json.null)
    fromAbove := columns.any (fun c => (src c).isSome)
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
                 facts := facts
                 -- The days an input takes from a koyomi file (rulec's §15.174), on its axis.
                 days := axes.toList.map (fun a =>
                   if fieldStr a "kind" == "input" then daysOf (fieldStr a "column") else none)
                 above := aboveCols
                 -- A boolean `define` of one comparison, read on its axis (rulec's §15.196): the
                 -- comparison and its two sides' intervals, worked out by the caller from the
                 -- define's expression, and the coordinate written `true`.
                 truth := (List.range columns.length).map (fun ai => do
                   let (op, L, R) ← truthOf ((columns[ai]?).getD "")
                   let ta ← ((labels[ai]?).getD []).idxOf? "true"
                   some { op := op, l := L, r := R, trueAt := ta })
                 -- What a `define` comes to with the axes it reads held (rulec's §15.322): each
                 -- fact the caller earned back from the define's expression, as a line.
                 lines := neverLines ++ narrowed }
      cover := cover
      told := fun a b => (told.find? (fun t => t.1 == a && t.2.1 == b)).map (fun t => t.2.2)
      witness := fun i => (wit.find? (fun w => w.1 == i)).map (fun w => (w.2.1, w.2.2))
      undecided := fun a b => (undec.find? (fun u => u.1 == a && u.2 == b)).isSome
      refuted := fun a b => (refuted.find? (fun t => t.1 == a && t.2.1 == b)).map (fun t => t.2.2)
      farkasAt := fun p => (leaves.find? (fun l => l.1 == p)).map (fun l => l.2)
      daysApart := fun a b => (byDays.find? (fun t => t.1 == a && t.2.1 == b)).map (fun t => t.2.2)
      aboveAt := fun p => (aboveAts.find? (fun l => l.1 == p)).map (fun l => l.2)
      aboveApart := fun a b => (aboveApartL.find? (fun t => t.1 == a && t.2.1 == b)).map (fun t => t.2.2) } }

end RulecCert
