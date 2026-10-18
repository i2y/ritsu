/-
  The evaluation (kaname's DESIGN 8, `interp.rs`): what a module means, as the reference
  interpreter computes it. The theorems of `Sound.lean` are about this function, and `ritsu-model`
  runs it on the vectors the Rust test makes.

  - The order is the interpreter's: the arguments and the fields as written, from left to right,
    then each read in its place's unit, then the places held to what they declare, in the order
    declared; the `let` lines from the top down; `and` and `or` from the left, the right not
    evaluated once the left decides; `any`, `all` and `first` stop at the element that decides them.
  - The places: a value given to a parameter of a function, a function's answer and a field of a
    record of a module are held to what they declare (`range`), a number to int64 (`overflow`), a
    divisor to not 0 (`zero`), `isqrt`'s argument to 0 and above (`range`), an amount of a transfer
    to 0 and above (`range`), an input of a rule or a date to its range (`rule`, `date`).
  - A rule answers what the oracle says it answers for the values of its inputs, or refuses them
    (`rule`); a date of a dates file likewise (`date`). A transfer is a value that is built.
  - Where the module leans on what the check of types holds (a value on the step its type says, an
    argument among the values of its parameter's type, an answer among the values of the result's
    type, an arm that takes the value), the evaluation looks, and fails `type` where it does not
    hold: never, for a module that passes the check.
  - A call deeper than 200 does not stop (`diverged`, DESIGN 8). Every step takes one of `fuel`;
    a run is given more than any evaluation of the tests takes.
-/
import KanameCore.Syntax

namespace KanameCore

abbrev Env := List (String × Val)

def Env.get? (ρ : Env) (x : String) : Option Val := (ρ.find? (·.1 == x)).map (·.2)

/-- How deep the calls of kaname's functions go before the evaluation stops (DESIGN 8). -/
def DEPTH : Nat := 200

abbrev R := Except Stop

def failK {α : Type} (k : Kind) (line : Nat) : R α := .error (.failed k line)

def liftA (line : Nat) : Except Arith Rat → R Rat
  | .ok v => .ok v
  | .error .overflow => .error (.failed .overflow line)
  | .error .zero => .error (.failed .zero line)

/-- A number read in another unit, held to int64. -/
def convN : Option Conv → Rat → R Rat
  | none, r => .ok r
  | some c, r => liftA c.line (held (r * c.f + c.shift))

mutual
/-- A value read in another unit: a number, and every number of a list (of lists); anything else
    is itself (`interp::coerce`). -/
def coerceV (c : Conv) : Val → R Val
  | .num r => (.num ·) <$> convN (some c) r
  | .list xs => (.list ·) <$> coerceL c xs
  | v => .ok v
def coerceL (c : Conv) : List Val → R (List Val)
  | [] => .ok []
  | x :: xs => do
    let y ← coerceV c x
    let ys ← coerceL c xs
    .ok (y :: ys)
end

def coerce : Option Conv → Val → R Val
  | none, v => .ok v
  | some c, v => coerceV c v

/-- A number held to the rule of int64 (`x in u` is). -/
def heldV (line : Nat) (v : Val) : R Val :=
  match v with
  | .num r => (.num ·) <$> liftA line (held r)
  | v => .ok v

/-- Whether a value is outside what a range declares. -/
def outRange (r : Range) (p : Rat) : Bool :=
  (r.min.any fun lo => p < lo) || (r.max.any fun hi => hi < p)

/-- The number or the day a range is held to. -/
def Val.point : Val → Option Rat
  | .num r => Option.some r
  | .date d => Option.some d
  | _ => Option.none

/-- The values a place holds to its range: a list's elements (none where its count is outside),
    or the value itself. -/
def elemsOf (list : Bool) (count : Option Range) (v : Val) : Option (List Val) :=
  match list, v with
  | true, .list xs =>
    match count with
    | some c => if outRange c xs.length then Option.none else some xs
    | Option.none => some xs
  | _, _ => some [v]

/-- The range a place holds its values to: what it declares, or 0% to 100% for a rate with a step
    and no range, where a parameter reaches it. -/
def rangeOf (range : Option Range) (unit01 reached : Bool) : Option Range :=
  match range with
  | some r => some r
  | Option.none => if reached && unit01 then some ⟨some 0, some 1⟩ else Option.none

/-- Whether a number or a date is outside a range. -/
def pointOut (r : Option Range) (x : Val) : Bool :=
  match r, x.point with
  | some r, some p => outRange r p
  | _, _ => false

/-- Whether a value does not keep what its place declares (`interp::outside`): the count of a
    list, the range of its numbers and dates (with `reached`, a rate with a step and no range held to
    0% to 100%), and with `reached` the fields of the records of a module it holds. -/
def Slot.outsideD : Nat → Slot → Bool → Val → Bool
  | 0, _, _, _ => false
  | d + 1, .mk list count range unit01 record fields, reached, v =>
    match v with
    | .none => false
    | _ =>
      match elemsOf list count v with
      | Option.none => true
      | some xs =>
        xs.any fun x => match x with
          | .none => false
          | _ =>
            pointOut (rangeOf range unit01 reached) x ||
            (reached && record && match x with
              | .record fs => fields.any fun (f, s) => match fs.find? (·.1 == f) with
                | some (_, fv) => Slot.outsideD d s true fv
                | Option.none => false
              | _ => false)

def Slot.outside (s : Slot) (reached : Bool) (v : Val) : Bool := s.outsideD D reached v

-- ── Text ────────────────────────────────────────────────────────────────────

/-- A whole number padded with zeros to `w` digits. -/
def padNat (w : Nat) (n : Nat) : String :=
  let s := toString n
  String.ofList (List.replicate (w - s.length) '0') ++ s

/-- A rational as ritsu-units writes it: a whole number as it is, a decimal where the denominator
    is made of 2s and 5s (and the decimal fits in 128 bits), a fraction otherwise. -/
def ratText (r : Rat) : String :=
  if r.den = 1 then toString r.num
  else
    let sign := if r.num < 0 then "-" else ""
    let num := r.num.natAbs
    let den := r.den
    let rec strip (p : Nat) (n : Nat) (fuel : Nat) : Nat × Nat :=
      match fuel with
      | 0 => (n, 0)
      | k + 1 => if n % p = 0 && n > 0 then let (m, c) := strip p (n / p) k; (m, c + 1) else (n, 0)
    let (rest, twos) := strip 2 den 128
    let (rest, fives) := strip 5 rest 128
    let places := max twos fives
    let scale := 10 ^ places
    if rest = 1 && scale < 2 ^ 128 && num * scale < 2 ^ 128 then
      let v := num * scale / den
      sign ++ toString (v / scale) ++ "." ++ padNat places (v % scale)
    else sign ++ toString num ++ "/" ++ toString den

/-- The year, month and day of a day number: Hinnant's `civil_from_days`, with Rust's division. -/
def civil (day : Int) : Int × Int × Int :=
  let z := day + 719468
  let era := (if z ≥ 0 then z else z - 146096).tdiv 146097
  let doe := z - era * 146097
  let yoe := (doe - doe.tdiv 1460 + doe.tdiv 36524 - doe.tdiv 146096).tdiv 365
  let y := yoe + era * 400
  let doy := doe - (365 * yoe + yoe.tdiv 4 - yoe.tdiv 100)
  let mp := (5 * doy + 2).tdiv 153
  let d := doy - (153 * mp + 2).tdiv 5 + 1
  let m := if mp < 10 then mp + 3 else mp - 9
  (if m ≤ 2 then y + 1 else y, m, d)

def dateText (day : Int) : String :=
  let (y, m, d) := civil day
  padNat 4 y.toNat ++ "-" ++ padNat 2 m.toNat ++ "-" ++ padNat 2 d.toNat

def textOf (fmt : TextFmt) : Val → Option String
  | .num r => some (match fmt with
      | .rate => ratText (r * 100) ++ "%"
      | .unit u => ratText r ++ u
      | _ => ratText r)
  | .str s => some s
  | .bool b => some (if b then "true" else "false")
  | .date d => some (dateText d)
  | .enum v => some v
  | _ => none

-- ── Comparisons ─────────────────────────────────────────────────────────────

/-- The place of a value in the order of its enum (the first, where it is not one of them). -/
def rank (values : List String) (v : String) : Nat := (values.idxOf? v).getD 0

def CmpOp.holds (op : CmpOp) (o : Ordering) : Bool :=
  match op with
  | .eq => o == .eq
  | .ne => o != .eq
  | .lt => o == .lt
  | .le => o != .gt
  | .gt => o == .gt
  | .ge => o != .lt

def cmpRat (a b : Rat) : Ordering := if a < b then .lt else if b < a then .gt else .eq
def cmpInt (a b : Int) : Ordering := if a < b then .lt else if b < a then .gt else .eq

/-- Two values compared, each number read in the unit they are compared in. A value of an enum and
    a bool are not compared: the check of types holds them apart. -/
def cmpVals (line : Nat) (op : CmpOp) (ca cb : Option Conv) (ord : Order) (x y : Val) : R Val :=
  match x, y with
  | .num p, .num q => do
    let p ← convN ca p
    let q ← convN cb q
    .ok (.bool (op.holds (cmpRat p q)))
  | .date p, .date q => .ok (.bool (op.holds (cmpInt p q)))
  | .enum p, .enum q =>
    match op, ord with
    | .eq, _ => .ok (.bool (p == q))
    | .ne, _ => .ok (.bool (p != q))
    | _, .enum vs => .ok (.bool (op.holds (cmpInt (rank vs p) (rank vs q))))
    | _, _ => .ok (.bool (op.holds .eq))
  | .enum _, .bool _ => failK .type line
  | .bool _, .enum _ => failK .type line
  | x, y =>
    let same := x == y
    .ok (.bool (if op == .ne then !same else same))

/-- A value given to a place whose type says a step: on it, or the type does not hold. -/
def stepped (s : Option Rat) (v : Val) (line : Nat) : R Unit :=
  match s, v with
  | some s, .num r => if onStep s r then .ok () else failK .type line
  | _, _ => .ok ()

def asBool (v : Val) (line : Nat) : R Bool :=
  match v with
  | .bool b => .ok b
  | _ => failK .type line

def asNum (v : Val) : Rat := match v with | .num r => r | _ => 0

/-- A number where the check of types holds a number. -/
def numOf (v : Val) (line : Nat) : R Rat :=
  match v with
  | .num r => .ok r
  | _ => failK .type line

/-- A value `max_of` and `min_of` can order: a number, a date, a value of an enum. -/
def ordered (v : Val) (line : Nat) : R Unit :=
  match v with
  | .num _ | .date _ | .enum _ => .ok ()
  | _ => failK .type line

/-- The answer of `max_of` or `min_of` so far, and the next: whether the next is better. -/
def better (ord : Order) (isMax : Bool) (best v : Val) : Bool :=
  let want : Ordering := if isMax then .gt else .lt
  match best, v with
  | .num b, .num n => cmpRat n b == want
  | .date b, .date n => cmpInt n b == want
  | .enum b, .enum n => match ord with
    | .enum vs => cmpInt (rank vs n) (rank vs b) == want
    | _ => false
  | _, _ => false

/-- The first argument value of each place of a call, in its order: none where no argument goes. -/
def place (n : Nat) (given : List (Nat × Val)) : List Val :=
  (List.range n).map fun k => ((given.find? (·.1 == k)).map (·.2)).getD .none

-- ── Pieces of the evaluation ────────────────────────────────────────────────

/-- Whether a pattern takes the value, and the name it gives it there. -/
def Pat.takes : Pat → Val → Option (Option String)
  | .none, .none => Option.some Option.none
  | .some _, .none => Option.none
  | .some x, _ => Option.some (Option.some x)
  | .bool b, .bool c => if b == c then Option.some Option.none else Option.none
  | .enum (Option.some n), .enum v => if n == v then Option.some Option.none else Option.none
  | _, _ => Option.none

def takesAny (v : Val) : List Pat → Option (Option String)
  | [] => Option.none
  | p :: ps => match p.takes v with
    | some b => some b
    | Option.none => takesAny v ps

/-- An arm the check of types lets through: `none` alone, `some x` alone, or values of an enum or
    bools (E106 holds `some x` to an arm of its own, E102 the patterns to the value's type). -/
def armOK : List Pat → Bool
  | [.none] => true
  | [.some _] => true
  | ps => ps.all fun p => match p with
    | .bool _ | .enum _ => true
    | _ => false

/-- The arm of a `match` that takes the value: its block, its answer's reading, and the name
    `some` gives the value there. An arm the check of types does not let through takes nothing, and
    the `match` stops there. -/
def pick (v : Val) : List (List Pat × Expr × Option Conv) → Option (Expr × Option Conv × Option String)
  | [] => Option.none
  | (ps, body, c) :: rest =>
    if !armOK ps then Option.none
    else match takesAny v ps with
      | some b => some (body, c, b)
      | Option.none => pick v rest

/-- The exact answer of `+`, `-`, `*` or `/` on two numbers, each side of a sum or a quotient read
    in the unit they are added or divided in (`combine`): a division by 0 fails. -/
def arithR (line : Nat) (op : ArOp) (ca cb : Option Conv) (p q : Rat) : R Rat :=
  match op with
  | .mul => .ok (p * q)
  | .add => do let p ← convN ca p; let q ← convN cb q; .ok (p + q)
  | .sub => do let p ← convN ca p; let q ← convN cb q; .ok (p - q)
  | .div => do
    let p ← convN ca p
    let q ← convN cb q
    if q = 0 then failK .zero line else .ok (p / q)

/-- The arguments of a call read in their places' units, in the order written. -/
def convArgs : List (Nat × Expr × Option Conv) → List Val → R (List (Nat × Val))
  | (k, _, c) :: rest, v :: vs => do
    let v ← coerce c v
    let more ← convArgs rest vs
    .ok ((k, v) :: more)
  | _, _ => .ok []

/-- Whether a value is a number below 0. -/
def negNum (v : Val) : Bool :=
  match v with
  | .num r => decide (r < 0)
  | _ => false

/-- The arguments of a transfer read in their parameters' units, an amount below 0 the failure
    `range` at its line (chobo takes none). -/
def convTArgs : List (String × Expr × Option Conv × Nat × Bool) → List Val → R (List (String × Val))
  | (p, _, c, line, amount) :: rest, v :: vs => do
    let v ← coerce c v
    if amount && negNum v then failK .range line
    else do
      let more ← convTArgs rest vs
      .ok ((p, v) :: more)
  | _, _ => .ok []

/-- The fields of a record as written, read in their places' units, each with its line. -/
def convInits : List (Nat × Expr × Option Conv × Nat) → List Val → R (List (Nat × Val × Nat))
  | (k, _, c, line) :: rest, v :: vs => do
    let v ← coerce c v
    let more ← convInits rest vs
    .ok ((k, v, line) :: more)
  | _, _ => .ok []

/-- Whether rulec refuses a field of an element of a list it walks for the field's range. -/
def colRefuses (vs : List (String × Val)) (col : String × Option Range) : Bool :=
  match col.2, ((vs.find? (·.1 == col.1)).map (·.2) : Option Val) with
  | some r, some (Val.num x) => outRange r x
  | _, _ => false

/-- Whether rulec refuses a value of an enum for the range its input holds the enum to: a value the
    range does not let in. An input with no numeric range and no range of days holds one (an input
    of an ordered enum: ritsu-ports' `ColumnType::Enum`). -/
def RuleInput.enumOut (i : RuleInput) : Val → Bool
  | .enum x =>
    match i.range, i.dateRanged, i.values with
    | Option.none, false, some vs => !vs.contains x
    | _, _, _ => false
  | _ => false

/-- Whether rulec refuses the value of an input for its range: a number outside the input's
    range, a value of an enum the range of its input does not let in, a field of an element of a
    list it walks outside the field's. -/
def RuleInput.refuses (i : RuleInput) (v : Val) : Bool :=
  match i.elements with
  | Option.none =>
    (match i.range, v.point with
    | some r, some x => outRange r x
    | _, _ => false) || i.enumOut v
  | some fs =>
    match v with
    | .list els => els.any fun el => match el with
      | .record vs => fs.any (colRefuses vs)
      | _ => false
    | _ => false

def sumN (line : Nat) : Rat → List Val → R Rat
  | acc, [] => .ok acc
  | acc, v :: vs => do
    let acc ← liftA line (addN acc (asNum v))
    sumN line acc vs

/-- The records of `running_totals`: each element, the total before it, up to it, and of all. The
    sums on the way are those `sumN` held to int64 already. -/
def runningN (total : Rat) : Rat → List (Val × Val) → List Val
  | _, [] => []
  | before, (x, w) :: rest =>
    let upto := before + asNum w
    .record [("item", x), ("before", .num before), ("upto", .num upto), ("total", .num total)] :: runningN total upto rest

def bestOf (ord : Order) (isMax : Bool) : Option Val → List Val → Option Val
  | best, [] => best
  | none, v :: vs => bestOf ord isMax (some v) vs
  | some b, v :: vs => bestOf ord isMax (if better ord isMax b v then some v else some b) vs

def isTrue (v : Val) : Bool := match v with | .bool true => true | _ => false

/-- What the function given to a walk answers, as the check of types holds it: a bool where it
    is a condition, a value that can be ordered for `max_of` and `min_of`, a number to add up. -/
def answersOK (w : Walk) (line : Nat) (answers : List Val) : R Unit :=
  match w with
  | .count | .any | .all | .filter | .first => answers.forM fun a => do let _ ← asBool a line
  | .maxOf | .minOf => answers.forM fun a => ordered a line
  | .sum | .running => answers.forM fun a => do let _ ← numOf a line
  | .map => .ok ()

/-- A walk of a list, from the list and what the function given answered for its elements. -/
def fold (w : Walk) (line : Nat) (ord : Order) (step : Option Rat) (xs answers : List Val) : R Val :=
  match w with
  | .count => .ok (.num ((answers.filter isTrue).length : Rat))
  | .any => .ok (.bool (answers.any isTrue))
  | .all => .ok (.bool (answers.all isTrue))
  | .map => .ok (.list answers)
  | .filter => .ok (.list (((xs.zip answers).filter (fun p => isTrue p.2)).map (·.1)))
  | .first => .ok (match (xs.zip answers).find? (fun p => isTrue p.2) with
      | some (x, _) => x
      | none => .none)
  | .sum => do
    answers.forM fun a => stepped step a line
    let total ← sumN line 0 answers
    .ok (.num total)
  | .maxOf => .ok ((bestOf ord true none answers).getD .none)
  | .minOf => .ok ((bestOf ord false none answers).getD .none)
  | .running => do
    answers.forM fun a => stepped step a line
    let total ← sumN line 0 answers
    .ok (.list (runningN total 0 (xs.zip answers)))

-- ── The evaluation ──────────────────────────────────────────────────────────

structure Ctx where
  M : Module
  O : Oracle

/-- Whether a value given to the parameter `k` of `fn` is outside what the parameter declares. -/
def paramOutside (fn : Fn) (k : Nat) (v : Val) : Bool :=
  match fn.params[k]? with
  | some p => p.slot.outside true v
  | Option.none => false

/-- Whether rulec refuses a value given to the input `k` of a rule. -/
def inputRefuses (rd : RuleD) (k : Nat) (v : Val) : Bool :=
  match rd.inputs[k]? with
  | some i => i.refuses v
  | Option.none => false

/-- A call's arguments in its parameters' places: each held to what its place declares (`range`),
    and to its parameter's type. -/
def callArgs (M : Module) (line f : Nat) (given : List (Nat × Val)) : R (List Val) :=
  match M.fns[f]? with
  | Option.none => failK .type line
  | some fn =>
    let ps := place fn.params.length given
    if given.any (fun (k, v) => paramOutside fn k v) then failK .range line
    else if (fn.params.zip ps).all (fun (p, v) => p.abs.has v) then .ok ps
    else failK .type line

/-- What a rule answers for the values of its inputs (`rule` where it refuses them). -/
def ruleAnswer (C : Ctx) (line r : Nat) (given : List (Nat × Val)) : R Val :=
  match C.M.rules[r]? with
  | Option.none => failK .type line
  | some rd =>
    let ins := place rd.inputs.length given
    if !(rd.inputs.zip ins).all (fun (i, v) => match i.step, v with
        | some s, .num x => onStep s x
        | _, _ => true) then failK .type line
    else if given.any (fun (k, v) => inputRefuses rd k v) then failK .rule line
    else match C.O.rule r ins with
      | Option.none => failK .rule line
      | some outs =>
        if outs.length != rd.outputs.length then failK .type line
        else if outs.any (fun v => match v with | .num x => !fits x | _ => false) then failK .overflow line
        else .ok (.record (rd.outputs.map (·.1) |>.zip outs))

/-- The day of a date, or a whole number. -/
def intOf (v : Val) : Option Int :=
  match v with
  | .date d => some d
  | .num x => if x.den = 1 then some x.num else Option.none
  | _ => Option.none

/-- What a date of a dates file is for the values of its inputs (`date` where it stops). -/
def dateAnswer (C : Ctx) (line file : Nat) (date : String) (given : List (Nat × Val)) : R Val :=
  match C.M.dates[file]? with
  | Option.none => failK .type line
  | some fd =>
    let named := given.filterMap fun (k, v) => (fd.inputs[k]?).map fun i => (i, v)
    -- each a date or a whole number, as the check of types holds them
    match named.mapM (fun (i, v) => (intOf v).map fun d => (i.name, d)) with
    | Option.none => failK .type line
    | some ints =>
      if (named.zip ints).any (fun ((i, _), (_, d)) => d < i.min || i.max < d) then failK .date line
      else match C.O.date file date ints with
        | some d => .ok (.date d)
        | Option.none => failK .date line

/-- A transfer built: its parameters in the order declared, each with the value given to it. -/
def transferVal (tid : String) (params : List String) (given : List (String × Val)) : Val :=
  .transfer tid (params.filterMap fun p => (given.find? (·.1 == p)).map fun (_, v) => (p, v))

/-- The value written last for each field of a record (none where none is). -/
def recordVals (fields : List (String × Slot × Bool)) (given : List (Nat × Val × Nat)) : List Val :=
  fields.zipIdx.map fun (_, k) => ((given.reverse.find? (·.1 == k)).map (·.2.1)).getD .none

/-- The line each field of a record is written on (the record's where it is not written). -/
def recordLines (line : Nat) (fields : List (String × Slot × Bool)) (given : List (Nat × Val × Nat)) : List Nat :=
  fields.zipIdx.map fun (_, k) => ((given.reverse.find? (·.1 == k)).map (·.2.2)).getD line

/-- The line of the first field that does not keep what it declares. -/
def outsideLine (entries : List (((String × Slot × Bool) × Val) × Nat)) : Option Nat :=
  (entries.find? (fun (((_, s, _), v), _) => s.outside false v)).map (·.2)

/-- A record built: its fields in the order declared, each the value written last for it (none
    where none is), held to what it declares where it is a module's. -/
def recordVal (line : Nat) (module : Bool) (fields : List (String × Slot × Bool)) (given : List (Nat × Val × Nat)) : R Val :=
  match module, outsideLine ((fields.zip (recordVals fields given)).zip (recordLines line fields given)) with
  | true, some l => failK .range l
  | _, _ => .ok (.record ((fields.map (·.1)).zip (recordVals fields given)))

/-- Whether the function given to a walk answered what decides it, so that the walk stops there:
    `true` for `any` and `first`, `false` for `all`. -/
def stopAt (w : Walk) (v : Val) : Bool :=
  match w, v with
  | .any, .bool true => true
  | .first, .bool true => true
  | .all, .bool false => true
  | _, _ => false

mutual
def eval (C : Ctx) : Nat → Nat → Env → Expr → R Val
  | 0, _, _, _ => .error .fuel
  | n + 1, dep, ρ, e =>
    match e with
    | .num _ line v => (.num ·) <$> liftA line (held v)
    | .date _ d => .ok (.date d)
    | .str _ s => .ok (.str s)
    | .bool _ b => .ok (.bool b)
    | .none _ => .ok .none
    | .enum _ v => .ok (.enum v)
    | .var _ line x => match ρ.get? x with
      | some v => .ok v
      | Option.none => failK .type line
    | .field _ line base f => do
      let b ← eval C n dep ρ base
      match b.field? f with
      | some v => .ok v
      | Option.none => failK .type line
    | .neg _ line x => do
      let v ← eval C n dep ρ x
      match v with
      | .num r => (.num ·) <$> liftA line (negN r)
      | v => .ok v
    | .not _ line x => do
      let v ← eval C n dep ρ x
      let b ← asBool v line
      .ok (.bool !b)
    | .inUnit _ line x c => do
      let v ← eval C n dep ρ x
      let v ← coerce c v
      heldV line v
    | .arith _ line op a b ca cb step => do
      let x ← eval C n dep ρ a
      let y ← eval C n dep ρ b
      match x, y with
      | .num p, .num q =>
        let r ← arithR line op ca cb p q
        -- the exact answer is on the step its type says (what the check of types promises, which
        -- the reference interpreter does not test), then held to int64 (`num::add` and the rest)
        stepped step (.num r) line
        let r ← liftA line (held r)
        .ok (.num r)
      | _, _ => failK .type line
    | .ddiff _ a b => do
      let x ← eval C n dep ρ a
      let y ← eval C n dep ρ b
      match x, y with
      | .date p, .date q => .ok (.num (p - q))
      | _, _ => failK .type 0
    | .cmp _ line op a b ca cb ord sa sb => do
      let x ← eval C n dep ρ a
      let y ← eval C n dep ρ b
      stepped sa x line
      stepped sb y line
      cmpVals line op ca cb ord x y
    | .and _ line a b => do
      let l ← eval C n dep ρ a
      let lb ← asBool l line
      if !lb then .ok (.bool false)
      else do
        let r ← eval C n dep ρ b
        let rb ← asBool r line
        .ok (.bool rb)
    | .or _ line a b => do
      let l ← eval C n dep ρ a
      let lb ← asBool l line
      if lb then .ok (.bool true)
      else do
        let r ← eval C n dep ρ b
        let rb ← asBool r line
        .ok (.bool rb)
    | .ite _ c t f ct cf => do
      let cv ← eval C n dep ρ c
      let go ← asBool cv 0
      let v ← eval C n dep ρ (if go then t else f)
      coerce (if go then ct else cf) v
    | .mtch _ line s arms => do
      let v ← eval C n dep ρ s
      match pick v arms with
      | some (body, c, bind) =>
        let ρ' := match bind with | some x => (x, v) :: ρ | Option.none => ρ
        let r ← eval C n dep ρ' body
        coerce c r
      | Option.none => failK .type line
    | .letIn x v c body => do
      let a ← eval C n dep ρ v
      let a ← coerce c a
      eval C n dep ((x, a) :: ρ) body
    | .block _ b c => do
      let v ← eval C n dep ρ b
      coerce c v
    | .call _ line f args => do
      let vals ← evalArgs C n dep ρ args
      -- a function, rule or dates file the module does not have: what the check of types rules out
      match C.M.fns[f]? with
      | Option.none => failK .type line
      | some _ => do
        let given ← convArgs args vals
        let ps ← callArgs C.M line f given
        callFn C n dep f ps
    | .rule _ line r args => do
      let vals ← evalArgs C n dep ρ args
      match C.M.rules[r]? with
      | Option.none => failK .type line
      | some _ => do
        let given ← convArgs args vals
        ruleAnswer C line r given
    | .dateCall _ line file date args => do
      let vals ← evalArgs C n dep ρ args
      match C.M.dates[file]? with
      | Option.none => failK .type line
      | some _ => do
        let given ← convArgs args vals
        dateAnswer C line file date given
    | .transfer _ tid params args => do
      let vals ← evalArgs4 C n dep ρ args
      let given ← convTArgs args vals
      .ok (transferVal tid params given)
    | .record _ line module fields inits => do
      let vals ← evalInits C n dep ρ inits
      let given ← convInits inits vals
      recordVal line module fields given
    | .list _ elems => do
      let vs ← evalElems C n dep ρ elems
      .ok (.list vs)
    | .walk _ line w list x body c ord step => do
      let lv ← eval C n dep ρ list
      match lv with
      | .list xs => do
        let answers ← evalEach C n dep ρ x body c w xs
        answersOK w line answers
        fold w line ord step xs answers
      | _ => failK .type line
    | .reduce _ line list first acc x body c out => do
      let lv ← eval C n dep ρ list
      match lv with
      | .list xs => do
        let a ← eval C n dep ρ first
        let a ← evalFold C n dep ρ acc x body c xs a
        coerce out a
      | _ => failK .type line
    | .minmax _ line isMin a b ca cb ord => do
      let x ← eval C n dep ρ a
      let y ← eval C n dep ρ b
      let x ← coerce ca x
      let y ← coerce cb y
      let o : Ordering := match x, y with
        | .num p, .num q => cmpRat p q
        | .date p, .date q => cmpInt p q
        | .enum p, .enum q => match ord with
          | .enum vs => cmpInt (rank vs p) (rank vs q)
          | _ => .eq
        | _, _ => .eq
      let _ := line
      let first := if isMin then o != .gt else o != .lt
      .ok (if first then x else y)
    | .modOp _ x k => do
      let v ← eval C n dep ρ x
      let d ← eval C n dep ρ k
      -- two numbers, as the check of types holds `mod` to (as `isqrt` and the rest)
      let v ← numOf v 0
      let d ← numOf d 0
      .ok (.num (if d.den = 1 && d.num > 0 then modN v d.num else 0))
    | .isqrt _ line x => do
      let v ← eval C n dep ρ x
      let v ← numOf v line
      match isqrtN v with
      | some r => .ok (.num r)
      | Option.none => failK .range line
    | .allocate _ line t c s cc cs out => do
      let tv ← eval C n dep ρ t
      let cv ← eval C n dep ρ c
      let sv ← eval C n dep ρ s
      let tv ← numOf tv line
      let cv ← numOf cv line
      let sv ← numOf sv line
      let c ← convN cc cv
      let s ← convN cs sv
      let v ← liftA line (allocateN tv c s)
      let v ← convN out v
      .ok (.num v)
    | .round _ line way x g cg out top => do
      let xv ← eval C n dep ρ x
      let gv ← eval C n dep ρ g
      let xv ← numOf xv line
      let gv ← numOf gv line
      let g ← convN cg gv
      if g ≤ 0 then failK .type line
      else do
        let v ← liftA line (roundN way xv g)
        let v ← convN out v
        if top.has (.num v) then .ok (.num v) else failK .type line
    | .count1 _ list => do
      let v ← eval C n dep ρ list
      .ok (.num (match v with | .list xs => (xs.length : Rat) | _ => 0))
    | .startsWith _ s p => do
      let sv ← eval C n dep ρ s
      let pv ← eval C n dep ρ p
      .ok (.bool (match sv, pv with
        | .str a, .str b => a.startsWith b
        | _, _ => false))
    | .concat _ args => do
      let vs ← evalList C n dep ρ args
      .ok (.str (String.join (vs.filterMap fun v => match v with | .str s => some s | _ => Option.none)))
    | .text _ x fmt => do
      let v ← eval C n dep ρ x
      match textOf fmt v with
      | some s => .ok (.str s)
      | Option.none => failK .type 0
    | .bad _ line _ => failK .type line

/-- A function of the module called with values for its parameters, from depth `dep`: its answer,
    read in its result's type and held to its result's place. -/
def callFn (C : Ctx) : Nat → Nat → Nat → List Val → R Val
  | 0, _, _, _ => .error .fuel
  | n + 1, dep, f, args =>
    match C.M.fns[f]? with
    | Option.none => failK .type 0
    | some fn =>
      if DEPTH ≤ dep then .error .diverged
      else do
        let v ← eval C n (dep + 1) ((fn.params.map (·.name)).zip args |>.reverse) fn.body
        let v ← coerce fn.conv v
        if fn.ret.outside false v then failK .range fn.answerLine
        else if fn.declared.has v then .ok v
        else failK .type fn.answerLine

def evalArgs (C : Ctx) : Nat → Nat → Env → List (Nat × Expr × Option Conv) → R (List Val)
  | 0, _, _, _ => .error .fuel
  | _ + 1, _, _, [] => .ok []
  | n + 1, dep, ρ, (_, x, _) :: rest => do
    let v ← eval C n dep ρ x
    let vs ← evalArgs C n dep ρ rest
    .ok (v :: vs)

def evalArgs4 (C : Ctx) : Nat → Nat → Env → List (String × Expr × Option Conv × Nat × Bool) → R (List Val)
  | 0, _, _, _ => .error .fuel
  | _ + 1, _, _, [] => .ok []
  | n + 1, dep, ρ, (_, x, _) :: rest => do
    let v ← eval C n dep ρ x
    let vs ← evalArgs4 C n dep ρ rest
    .ok (v :: vs)

def evalInits (C : Ctx) : Nat → Nat → Env → List (Nat × Expr × Option Conv × Nat) → R (List Val)
  | 0, _, _, _ => .error .fuel
  | _ + 1, _, _, [] => .ok []
  | n + 1, dep, ρ, (_, x, _) :: rest => do
    let v ← eval C n dep ρ x
    let vs ← evalInits C n dep ρ rest
    .ok (v :: vs)

def evalElems (C : Ctx) : Nat → Nat → Env → List (Expr × Option Conv) → R (List Val)
  | 0, _, _, _ => .error .fuel
  | _ + 1, _, _, [] => .ok []
  | n + 1, dep, ρ, (x, c) :: rest => do
    let v ← eval C n dep ρ x
    let v ← coerce c v
    let vs ← evalElems C n dep ρ rest
    .ok (v :: vs)

def evalList (C : Ctx) : Nat → Nat → Env → List Expr → R (List Val)
  | 0, _, _, _ => .error .fuel
  | _ + 1, _, _, [] => .ok []
  | n + 1, dep, ρ, x :: rest => do
    let v ← eval C n dep ρ x
    let vs ← evalList C n dep ρ rest
    .ok (v :: vs)

/-- The function given to a walk, applied to each element in turn, its answer read in its place,
    until an answer decides the walk. -/
def evalEach (C : Ctx) : Nat → Nat → Env → String → Expr → Option Conv → Walk → List Val → R (List Val)
  | 0, _, _, _, _, _, _, _ => .error .fuel
  | _ + 1, _, _, _, _, _, _, [] => .ok []
  | n + 1, dep, ρ, x, body, c, w, el :: rest => do
    let a ← eval C n dep ((x, el) :: ρ) body
    let a ← coerce c a
    if stopAt w a then .ok [a]
    else do
      let as ← evalEach C n dep ρ x body c w rest
      .ok (a :: as)

/-- `reduce`'s function laid over each element, the running value read in the type of `first`. -/
def evalFold (C : Ctx) : Nat → Nat → Env → String → String → Expr → Option Conv → List Val → Val → R Val
  | 0, _, _, _, _, _, _, _, _ => .error .fuel
  | _ + 1, _, _, _, _, _, _, [], a => .ok a
  | n + 1, dep, ρ, acc, x, body, c, el :: rest, a => do
    let v ← eval C n dep ((x, el) :: (acc, a) :: ρ) body
    let a ← coerce c v
    evalFold C n dep ρ acc x body c rest a
end

end KanameCore
