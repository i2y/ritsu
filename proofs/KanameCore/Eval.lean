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
import KanameCore.Text

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

/-- What a collection of `n` holds where `n` is within the count a place declares; none where it is
    outside. -/
def countedIn (count : Option Range) (n : Nat) (xs : List Val) : Option (List Val) :=
  match count with
  | some c => if outRange c n then Option.none else some xs
  | Option.none => some xs

/-- The values a place holds to its range: a list's elements, a set's members, a dict's values (none
    where the count of the elements, the members or the pairs is outside), or the value itself. -/
def elemsOf (list : Bool) (count : Option Range) (v : Val) : Option (List Val) :=
  match list, v with
  | true, .list xs => countedIn count xs.length xs
  | true, .set xs => countedIn count xs.length xs
  | true, .dict ps => countedIn count ps.length (ps.map (·.2))
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

/-- Whether a field of a value of an enum does not keep what it declares, each field of the value
    by its place (`out`): the fields of the value it is, where the place holds that value. -/
def variantOutside (out : Slot → Val → Bool) (values : List (String × List (String × Slot))) : Val → Bool
  | .variant t fs => match (values.find? (·.1 == t)).map (·.2) with
    | some fss => fss.any fun x => match fs.find? (·.1 == x.1) with
      | some (_, fv) => out x.2 fv
      | Option.none => false
    | Option.none => false
  | _ => false

/-- Whether a key of a dict does not keep what the place of the keys declares (`out`). -/
def keysOut (out : Slot → Val → Bool) (keys : Option Slot) : Val → Bool
  | .dict ps => match keys with
    | some ks => ps.any fun p => out ks p.1
    | Option.none => false
  | _ => false

/-- Whether a string a place holds itself (no list) has a count of code points outside the count
    the place declares (DESIGN 25.1). -/
def strOut (list : Bool) (count : Option Range) : Val → Bool
  | .str s => !list && count.any fun c => outRange c s.length
  | _ => false

/-- Whether a value does not keep what its place declares (`interp::outside`): the count of a
    list, the range of its numbers and dates (with `reached`, a rate with a step and no range held to
    0% to 100%), a string's count of code points, and with `reached` the fields of the records of a
    module it holds and the fields of the values of an enum it holds (DESIGN 23.6). -/
def Slot.outsideD : Nat → Slot → Bool → Val → Bool
  | 0, _, _, _ => false
  | d + 1, .mk list count range unit01 record fields values keys _, reached, v =>
    match v with
    | .none => false
    | _ =>
      -- the keys of a dict where a parameter reaches them, each to the whole of its type
      (reached && keysOut (fun s v => Slot.outsideD d s true v) keys v) ||
      match elemsOf list count v with
      | Option.none => true
      | some xs =>
        xs.any fun x => match x with
          | .none => false
          | _ =>
            pointOut (rangeOf range unit01 reached) x || strOut list count x ||
            (reached && record && match x with
              | .record fs => fields.any fun (f, s) => match fs.find? (·.1 == f) with
                | some (_, fv) => Slot.outsideD d s true fv
                | Option.none => false
              | _ => false) ||
            (reached && variantOutside (fun s v => Slot.outsideD d s true v) values x)

def Slot.outside (s : Slot) (reached : Bool) (v : Val) : Bool := s.outsideD D reached v

/-- How deep a host's argument may nest the values of records and enums that hold themselves
    (kaname's `interp::NESTED`, DESIGN 28.1). -/
def NESTED : Nat := 200

/-- Whether a value a host gives is outside what its place declares (`interp::check_entry`, DESIGN
    4.4, 28.1): what `Slot.outsideD` holds a parameter's value to, and inside the values of records
    and enums that hold themselves too, each held to its type's place in the module's table (`table`),
    one deeper than those over it (`above`): one more than `NESTED` deep is outside. -/
def Slot.enterD (table : List Slot) : Nat → Nat → Slot → Val → Bool
  | 0, _, _, _ => false
  | d + 1, above, .mk list count range unit01 record fields values keys ref, v =>
    match v with
    | .none => false
    | _ =>
      keysOut (fun s v => Slot.enterD table d above s v) keys v ||
      match elemsOf list count v with
      | Option.none => true
      | some xs =>
        xs.any fun x => match x with
          | .none => false
          | _ =>
            pointOut (rangeOf range unit01 true) x || strOut list count x ||
            (record && match x with
              | .record fs => fields.any fun (f, s) => match fs.find? (·.1 == f) with
                | some (_, fv) => Slot.enterD table d above s fv
                | Option.none => false
              | _ => false) ||
            variantOutside (fun s v => Slot.enterD table d above s v) values x ||
            match ref with
            | some n => decide (NESTED < above + 1) || match table[n]? with
              | some t => Slot.enterD table d (above + 1) t x
              | Option.none => false
            | Option.none => false

def Slot.enter (table : List Slot) (s : Slot) (v : Val) : Bool := s.enterD table D 0 v

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

/-- What `text` is given, as the check of types holds it (DESIGN 25.6): a number a whole number of
    its unit within int64 (a rounded value), a date a day of the years 1 to 9999. -/
def textOK : TextFmt → Val → Bool
  | .number, v | .unit _, v => match v with
    | .num r => r.den == 1 && decide (i64min ≤ r.num) && decide (r.num ≤ i64max)
    | _ => false
  | .plain, .date d =>
    let (y, m, dd) := civil d
    decide (1 ≤ y) && decide (y ≤ 9999) && decide (1 ≤ m) && decide (m ≤ 12) && decide (1 ≤ dd) && decide (dd ≤ 31)
  | _, _ => true

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
  -- strings by their code points (DESIGN 25.1)
  | .str p, .str q => .ok (.bool (op.holds (compare p q)))
  | .enum p, .enum q =>
    match op, ord with
    | .eq, _ => .ok (.bool (p == q))
    | .ne, _ => .ok (.bool (p != q))
    | _, .enum vs => .ok (.bool (op.holds (cmpInt (rank vs p) (rank vs q))))
    | _, _ => .ok (.bool (op.holds .eq))
  | .enum _, .bool _ => failK .type line
  | .bool _, .enum _ => failK .type line
  | x, y =>
    match op with
    -- two values compared whole (DESIGN 23.5), each read in the unit they are compared in
    | .eq | .ne => do
      let x ← coerce ca x
      let y ← coerce cb y
      let same := x == y
      .ok (.bool (if op == .ne then !same else same))
    | _ =>
      let same := x == y
      .ok (.bool (if op == .ne then !same else same))

-- ── The order of keys ───────────────────────────────────────────────────────

/-- The place of a value of an enum in the order it is declared (after every value, where it is
    not one of them). -/
def keyRank (values : List String) (v : String) : Nat := (values.idxOf? v).getD values.length

def cmpBool (a b : Bool) : Ordering := if a == b then .eq else if a then .gt else .lt

mutual
/-- Two keys in the order of their type (DESIGN 24.1; `value.rs`'s `key_cmp`): numbers from the
    least, dates from the earliest, strings by their code points, `false` before `true`, the values
    of an enum in the order declared, a record field by field. -/
def keyCmp : Key → Val → Val → Ordering
  | _, .num a, .num b => cmpRat a b
  | _, .date a, .date b => cmpInt a b
  | _, .str a, .str b => compare a b
  | _, .bool a, .bool b => cmpBool a b
  | .enum vs, .enum a, .enum b => compare (keyRank vs a) (keyRank vs b)
  | _, .enum a, .enum b => compare a b
  | .record ks, .record xs, .record ys => keyCmpFields ks xs ys
  | _, _, _ => .eq
/-- Two records' fields, in the order declared: the first that differs decides. -/
def keyCmpFields : List Key → List (String × Val) → List (String × Val) → Ordering
  | k :: ks, (_, x) :: xs, (_, y) :: ys =>
    match keyCmp k x y with
    | .eq => keyCmpFields ks xs ys
    | o => o
  | _, _, _ => .eq
end

/-- Whether two keys are one: the same value. For the keys a dict and a set can have (a number, a
    date, a string, a bool, a value of an enum whose values have no fields), it is what `key_cmp`
    says, which tells them apart by nothing else. -/
def sameKey (a b : Val) : Bool := a == b

/-- The pairs with the later of two pairs of one key kept. -/
def lastOfEach : List (Val × Val) → List (Val × Val)
  | [] => []
  | p :: rest => if rest.any (fun q => sameKey q.1 p.1) then lastOfEach rest else p :: lastOfEach rest

/-- The values not among `seen`, each the first time it comes. -/
def firstOfEachAux (seen : List Val) : List Val → List Val
  | [] => []
  | x :: rest => if seen.any (fun y => sameKey y x) then firstOfEachAux seen rest else x :: firstOfEachAux (x :: seen) rest

/-- The values with the first of two that are one kept, in their order. -/
def firstOfEach (xs : List Val) : List Val := firstOfEachAux [] xs

/-- In the order of the keys: a sort, stable, by `keyCmp`. -/
def byKey (k : Key) (a b : Val) : Bool := keyCmp k a b != .gt

/-- The pairs of a dict in the order of their keys, the later of two pairs of one key kept
    (`value.rs`'s `dict_of`). -/
def dictFrom (k : Key) (ps : List (Val × Val)) : List (Val × Val) :=
  (lastOfEach ps).mergeSort fun p q => byKey k p.1 q.1

/-- The members of a set in the order of their keys, each once (`value.rs`'s `set_of`). -/
def setFrom (k : Key) (xs : List Val) : List Val := (firstOfEach xs).mergeSort (byKey k)

/-- The elements of each key, in the order of the list, the keys in their order (`group_by`), from
    pairs of a key and an element. -/
def groupsOf (k : Key) (ps : List (Val × Val)) : List (Val × Val) :=
  ((firstOfEach (ps.map (·.1))).map fun key => (key, .list ((ps.filter fun p => sameKey p.1 key).map (·.2)))).mergeSort
    fun p q => byKey k p.1 q.1

/-- The element of the greatest key, or of the least (`want`), the first of one key, from pairs of a
    key and an element. -/
def bestBy (k : Key) (want : Ordering) : Option (Val × Val) → List (Val × Val) → Option (Val × Val)
  | best, [] => best
  | none, p :: ps => bestBy k want (some p) ps
  | some b, p :: ps => bestBy k want (if keyCmp k p.1 b.1 == want then some p else some b) ps

/-- An entry of a dict: a record of its key and its value (DESIGN 24.3). -/
def entryV (p : Val × Val) : Val := .record [("key", p.1), ("value", p.2)]

/-- How many numbers `numbers` makes before the evaluation stops (`interp::NUMBERS`): a list that
    long is what runs out of memory, which is outside the language's meaning, as a call too deep is. -/
def NUMBERS : Nat := 1000000

/-- A whole number between 0 and `n`, where one past either end is cut to it (DESIGN 24.3). -/
def clampTo (n : Nat) (i : Int) : Nat := if i < 0 then 0 else min i.toNat n

/-- A number a builtin takes as a place or a count (`at`, `take`, `numbers`): a whole number, as the
    check of types holds it to (E109). -/
def wholeOf : Val → Option Int
  | .num r => if r.den = 1 then some r.num else Option.none
  | _ => Option.none

/-- A list, as the check of types holds the lists `concat` and `flatten` join to. -/
def listOf : Val → Option (List Val)
  | .list xs => some xs
  | _ => Option.none

/-- A string, as the check of types holds what `join` and `concat` put together to. -/
def textVal : Val → Option String
  | .str s => some s
  | _ => Option.none

/-- A builtin of DESIGN 24.3 that takes no function, from its arguments read in their places, the
    dict, the set or the list first. What the check of types rules out fails `type`. -/
def primVal (line : Nat) (op : Prim) (k : Key) (vs : List Val) : R Val :=
  match op, vs with
  -- a dict
  | .get, [.dict ps, x] => .ok (((ps.find? fun p => sameKey p.1 x).map (·.2)).getD .none)
  | .hasD, [.dict ps, x] => .ok (.bool (ps.any fun p => sameKey p.1 x))
  | .put, [.dict ps, x, y] => .ok (.dict (dictFrom k (ps ++ [(x, y)])))
  | .removeD, [.dict ps, x] => .ok (.dict (dictFrom k (ps.filter fun p => !sameKey p.1 x)))
  | .keys, [.dict ps] => .ok (.list (ps.map (·.1)))
  | .values, [.dict ps] => .ok (.list (ps.map (·.2)))
  | .entries, [.dict ps] => .ok (.list (ps.map entryV))
  | .merge, [.dict ps, .dict qs] => .ok (.dict (dictFrom k (ps ++ qs)))
  -- a set
  | .hasS, [.set ms, x] => .ok (.bool (ms.any fun y => sameKey y x))
  | .add, [.set ms, x] => .ok (.set (setFrom k (ms ++ [x])))
  | .removeS, [.set ms, x] => .ok (.set (setFrom k (ms.filter fun y => !sameKey y x)))
  | .union, [.set ms, .set os] => .ok (.set (setFrom k (ms ++ os)))
  | .intersect, [.set ms, .set os] => .ok (.set (setFrom k (ms.filter fun y => os.any fun z => sameKey y z)))
  | .difference, [.set ms, .set os] => .ok (.set (setFrom k (ms.filter fun y => !os.any fun z => sameKey y z)))
  | .toList, [.set ms] => .ok (.list ms)
  -- a list
  | .first, [.list xs] => .ok (xs.head?.getD .none)
  | .last, [.list xs] => .ok (xs.getLast?.getD .none)
  | .append, [.list xs, x] => .ok (.list (xs ++ [x]))
  | .at, [.list xs, i] =>
    match wholeOf i with
    | some j => .ok (if 0 ≤ j ∧ j < (xs.length : Int) then (xs[j.toNat]?).getD .none else .none)
    | Option.none => failK .type line
  | .take, [.list xs, n] =>
    match wholeOf n with
    | some j => .ok (.list (xs.take (clampTo xs.length j)))
    | Option.none => failK .type line
  | .drop, [.list xs, n] =>
    match wholeOf n with
    | some j => .ok (.list (xs.drop (clampTo xs.length j)))
    | Option.none => failK .type line
  | .slice, [.list xs, a, b] =>
    match wholeOf a, wholeOf b with
    | some a, some b =>
      let (i, j) := (clampTo xs.length a, clampTo xs.length b)
      .ok (.list (if i < j then (xs.drop i).take (j - i) else []))
    | _, _ => failK .type line
  | .reverse, [.list xs] => .ok (.list xs.reverse)
  | .sort, [.list xs] => .ok (.list (xs.mergeSort (byKey k)))
  | .distinct, [.list xs] => .ok (.list (firstOfEach xs))
  | .contains, [.list xs, x] => .ok (.bool (xs.any (· == x)))
  | .flatten, [.list xs] =>
    match xs.mapM listOf with
    | some yss => .ok (.list yss.flatten)
    | Option.none => failK .type line
  | .indexed, [.list xs] =>
    .ok (.list (xs.zipIdx.map fun (x, i) => .record [("index", .num (i : Rat)), ("item", x)]))
  | .toSet, [.list xs] => .ok (.set (setFrom k xs))
  | .join, [.list xs, .str sep] =>
    match xs.mapM textVal with
    | some ss => .ok (.str (sep.intercalate ss))
    | Option.none => failK .type line
  -- lists one after another, each read in the type of the answer
  | .concat, ls =>
    match ls.mapM listOf with
    | some yss => .ok (.list yss.flatten)
    | Option.none => failK .type line
  -- `numbers(a, b)`: a to b, both in, none where b is below a; the count held to int64, and a list
  -- past `NUMBERS` stops the evaluation
  | .numbers, [a, b] =>
    match wholeOf a, wholeOf b with
    | some a, some b =>
      if b < a then .ok (.list [])
      else
        let count := b - a + 1
        if i64max < count then failK .overflow line
        else if (NUMBERS : Int) < count then .error .diverged
        else .ok (.list ((List.range count.toNat).map fun (i : Nat) => .num ((a + (i : Int) : Int) : Rat)))
    | _, _ => failK .type line
  -- a string (DESIGN 25.2), each in code points (`text.rs`)
  | .chars, [.str s] => .ok (.list (s.toList.map fun c => .str (String.ofList [c])))
  | .codePoints, [.str s] => .ok (.list (s.toList.map fun c => .num ((c.toNat : Int) : Rat)))
  | .toText, [.list xs] =>
    match xs.mapM wholeOf with
    | some ns => .ok (match Text.ofCodePoints ns with
      | some cs => .str (String.ofList cs)
      | Option.none => .none)
    | Option.none => failK .type line
  | .sliceT, [.str s, a, b] =>
    match wholeOf a, wholeOf b with
    | some a, some b => .ok (.str (String.ofList (Text.slice s.toList a b)))
    | _, _ => failK .type line
  | .takeT, [.str s, n] =>
    match wholeOf n with
    | some n => .ok (.str (String.ofList (Text.take s.toList n)))
    | Option.none => failK .type line
  | .dropT, [.str s, n] =>
    match wholeOf n with
    | some n => .ok (.str (String.ofList (Text.drop s.toList n)))
    | Option.none => failK .type line
  | .indexOf, [.str s, .str t] =>
    .ok (match Text.indexOf s.toList t.toList with
      | some i => .num ((i : Int) : Rat)
      | Option.none => .none)
  | .containsT, [.str s, .str t] => .ok (.bool (Text.contains s.toList t.toList))
  | .endsWith, [.str s, .str t] => .ok (.bool (Text.endsWith s.toList t.toList))
  -- the pieces of a string of more than 2⁶³ − 1 code points: the model stops on the string, as on a
  -- list that long (`sized`)
  | .split, [.str s, .str sep] =>
    if i64max < s.length then .error .diverged
    else .ok (.list ((Text.split s.toList sep.toList).map fun p => .str (String.ofList p)))
  | .words, [.str s] =>
    if i64max < s.length then .error .diverged
    else .ok (.list ((Text.words s.toList).map fun p => .str (String.ofList p)))
  | .replace, [.str s, .str a, .str b] => .ok (.str (String.ofList (Text.replace s.toList a.toList b.toList)))
  | .trim, [.str s] => .ok (.str (String.ofList (Text.trim s.toList)))
  | .trimStart, [.str s] => .ok (.str (String.ofList (Text.trimStart s.toList)))
  | .trimEnd, [.str s] => .ok (.str (String.ofList (Text.trimEnd s.toList)))
  | .lowerAscii, [.str s] => .ok (.str (String.ofList (Text.lowerAscii s.toList)))
  | .upperAscii, [.str s] => .ok (.str (String.ofList (Text.upperAscii s.toList)))
  -- a padding past `LONGEST` stops the evaluation, as a list of more numbers does
  | .padStart, [.str s, w, .str f] =>
    match wholeOf w with
    | some w =>
      if (Text.LONGEST : Int) < w && f.toList ≠ [] && (s.length : Int) < w then .error .diverged
      else .ok (.str (String.ofList (Text.pad s.toList w f.toList false)))
    | Option.none => failK .type line
  | .padEnd, [.str s, w, .str f] =>
    match wholeOf w with
    | some w =>
      if (Text.LONGEST : Int) < w && f.toList ≠ [] && (s.length : Int) < w then .error .diverged
      else .ok (.str (String.ofList (Text.pad s.toList w f.toList true)))
    | Option.none => failK .type line
  | .parseNumber, [.str s] =>
    .ok (match Text.parseNumber s.toList with
      | some x => .num (x : Rat)
      | Option.none => .none)
  | .parseDate, [.str s] =>
    .ok (match Text.parseDate s.toList with
      | some z => .date z
      | Option.none => .none)
  -- `char(n)`: a scalar value, as the check of types holds the constant to (E109)
  | .charOf, [n] =>
    match wholeOf n with
    | some n => if Text.isScalar n then .ok (.str (String.ofList [Char.ofNat n.toNat])) else failK .type line
    | Option.none => failK .type line
  | _, _ => failK .type line

/-- A list, a set or a dict of more than 2⁶³ − 1 elements, members or pairs: what runs out of memory
    long before it is made, outside the language's meaning as a call too deep is (DESIGN 24.3), where
    the model stops (the interpreter cannot make one). -/
def sized (v : Val) : R Val :=
  match v with
  | .list xs => if i64max < xs.length then .error .diverged else .ok v
  | .set xs => if i64max < xs.length then .error .diverged else .ok v
  | .dict ps => if i64max < ps.length then .error .diverged else .ok v
  | .str s => if i64max < s.length then .error .diverged else .ok v
  | v => .ok v

/-- The arguments of a builtin read in their places, in the order written. -/
def coerceAll : List (Expr × Option Conv) → List Val → R (List Val)
  | (_, c) :: rest, v :: vs => do
    let v ← coerce c v
    let more ← coerceAll rest vs
    .ok (v :: more)
  | _, _ => .ok []

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

/-- The word a value of an enum or a bool is (`true`, `false`): what a pattern names and a set of
    the analysis holds. -/
def strOf : Val → Option String
  | .enum e => some e
  | .bool b => some (if b then "true" else "false")
  | _ => none

/-- Whether a pattern takes the value, and the name it gives it there. A value of an enum and a bool
    are taken by the word they are (the check of types holds a pattern to the type it splits); a
    value of an enum that has fields by its name, with or without its fields; a literal by being the
    same value; `else` takes every value. -/
def Pat.takes : Pat → Val → Option (Option String)
  | .none, .none => Option.some Option.none
  | .some _, .none => Option.none
  | .some x, _ => Option.some (Option.some x)
  | .bool b, v => if strOf v == Option.some (if b then "true" else "false") then Option.some Option.none else Option.none
  | .enum (Option.some n), v => if strOf v == Option.some n then Option.some Option.none else Option.none
  | .value (Option.some n) b, v => if v.tag? == Option.some n then Option.some b else Option.none
  | .lit l, v => if l == v then Option.some Option.none else Option.none
  | .rest, _ => Option.some Option.none
  | _, _ => Option.none

def takesAny (v : Val) : List Pat → Option (Option String)
  | [] => Option.none
  | p :: ps => match p.takes v with
    | some b => some b
    | Option.none => takesAny v ps

/-- A pattern of a value of an enum or a bool, given no name. -/
def Pat.isName : Pat → Bool
  | .bool _ | .enum _ | .value _ Option.none => true
  | _ => false

/-- A literal. -/
def Pat.isLit : Pat → Bool
  | .lit _ => true
  | _ => false

/-- An arm the check of types lets through: `none` alone, `some x` alone, `else` alone, a value of
    an enum given a name alone, values of an enum and bools, or literals (E106 holds `some x`,
    `else` and a value given a name to an arm of their own, E102 the patterns to the value's type). -/
def armOK : List Pat → Bool
  | [.none] => true
  | [.some _] => true
  | [.rest] => true
  | [.value _ (Option.some _)] => true
  | ps => ps.all Pat.isName || ps.all Pat.isLit

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

/-- What a builtin of DESIGN 24.3 that applies a function answers, from the collection it walks
    (each element of a list, each entry or value of a dict) and what the function answered for each
    (for `last`, from the end). -/
def foldK (w : WalkK) (k : Key) (xs answers : List Val) : Val :=
  match w with
  | .last => (((xs.zip answers).find? fun p => isTrue p.2).map (·.1)).getD .none
  | .sortBy => .list (((answers.zip xs).mergeSort fun p q => byKey k p.1 q.1).map (·.2))
  | .sortByDesc => .list (((answers.zip xs).mergeSort fun p q => byKey k q.1 p.1).map (·.2))
  | .maxBy => ((bestBy k .gt none (answers.zip xs)).map (·.2)).getD .none
  | .minBy => ((bestBy k .lt none (answers.zip xs)).map (·.2)).getD .none
  | .flatMap => .list ((answers.filterMap listOf).flatten)
  | .filterMap => .list (answers.filter fun a => match a with | .none => false | _ => true)
  | .groupBy => .dict (groupsOf k (answers.zip xs))
  -- a dict's walks are folded where they are evaluated
  | .filterD | .mapValues => .none

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

/-- The value written last for each field of a record: where none is, the field of the value it is
    made from (`..x`), else none. -/
def recordVals (fields : List (String × Slot × Bool)) (given : List (Nat × Val × Nat)) (src : Option Val) : List Val :=
  fields.zipIdx.map fun ((f, _), k) => ((given.reverse.find? (·.1 == k)).map (·.2.1)).getD
    ((src.bind (·.field? f)).getD .none)

/-- The line each field of a record is written on (the record's where it is not written). -/
def recordLines (line : Nat) (fields : List (String × Slot × Bool)) (given : List (Nat × Val × Nat)) : List Nat :=
  fields.zipIdx.map fun (_, k) => ((given.reverse.find? (·.1 == k)).map (·.2.2)).getD line

/-- The line of the first field that does not keep what it declares. -/
def outsideLine (entries : List (((String × Slot × Bool) × Val) × Nat)) : Option Nat :=
  (entries.find? (fun (((_, s, _), v), _) => s.outside false v)).map (·.2)

/-- What a record is built as: a record of its fields, or a value of an enum (its name alone where
    it has no fields). -/
def built (tag : Option String) (fs : List (String × Val)) : Val :=
  match tag with
  | some t => if fs.isEmpty then .enum t else .variant t fs
  | Option.none => .record fs

/-- Whether a field not written is one the value it is made from does not have: what the check of
    types rules out (`..x` is of the type built). -/
def baseMissing (fields : List (String × Slot × Bool)) (given : List (Nat × Val × Nat)) : Option Val → Bool
  | some w => fields.zipIdx.any fun ((f, _), k) => (given.find? (·.1 == k)).isNone && (w.field? f).isNone
  | Option.none => false

/-- A record built: its fields in the order declared, each the value written last for it (the field
    of the value it is made from, or none, where none is), held to what it declares where it is a
    module's. -/
def recordVal (line : Nat) (module : Bool) (fields : List (String × Slot × Bool)) (given : List (Nat × Val × Nat))
    (src : Option Val) (tag : Option String) : R Val :=
  if baseMissing fields given src then failK .type line
  else match module, outsideLine ((fields.zip (recordVals fields given src)).zip (recordLines line fields given)) with
  | true, some l => failK .range l
  | _, _ => .ok (built tag ((fields.map (·.1)).zip (recordVals fields given src)))

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
    | .record _ line module fields inits tag base => do
      -- the value the fields not written are taken from, before the fields (DESIGN 23.6)
      let src ← match base with
        | some b => (some ·) <$> eval C n dep ρ b
        | Option.none => .ok Option.none
      let vals ← evalInits C n dep ρ inits
      let given ← convInits inits vals
      recordVal line module fields given src tag
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
        | .str p, .str q => compare p q
        | .enum p, .enum q => match ord with
          | .enum vs => cmpInt (rank vs p) (rank vs q)
          | _ => .eq
        | _, _ => .eq
      let _ := line
      let first := if isMin then o != .gt else o != .lt
      .ok (if first then x else y)
    | .modOp _ line x k => do
      let v ← eval C n dep ρ x
      let d ← eval C n dep ρ k
      -- two numbers, as the check of types holds `mod` to (as `isqrt` and the rest); the remainder
      -- from 0 to one less than the divisor's size, `zero` where the divisor is 0 (DESIGN 24.3)
      let v ← numOf v 0
      let d ← numOf d 0
      if d = 0 then failK .zero line
      -- a whole number, as the check of types holds the divisor to (E109)
      else if d.den ≠ 1 then failK .type line
      else .ok (.num (modN v d.floor))
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
      .ok (.num (match v with
        | .list xs => (xs.length : Rat)
        | .dict ps => (ps.length : Rat)
        | .set xs => (xs.length : Rat)
        | .str s => (s.length : Rat)
        | _ => 0))
    | .startsWith _ s p => do
      let sv ← eval C n dep ρ s
      let pv ← eval C n dep ρ p
      .ok (.bool (match sv, pv with
        | .str a, .str b => a.startsWith b
        | _, _ => false))
    | .concat _ args => do
      let vs ← evalList C n dep ρ args
      match vs.mapM textVal with
      | some ss => sized (.str (String.join ss))
      | Option.none => failK .type 0
    | .text _ x fmt => do
      let v ← eval C n dep ρ x
      if textOK fmt v then
        match textOf fmt v with
        | some s => .ok (.str s)
        | Option.none => failK .type 0
      else failK .type 0
    | .dict _ key pairs => do
      let ps ← evalPairs C n dep ρ pairs
      sized (.dict (dictFrom key ps))
    | .prim _ line op key args top => do
      let vs ← evalList C n dep ρ (args.map (·.1))
      let vs ← coerceAll args vs
      let v ← primVal line op key vs
      let v ← sized v
      match top with
      | some t => if t.has v then .ok v else failK .type line
      | Option.none => .ok v
    | .walkK _ line w coll x body c key => do
      let cv ← eval C n dep ρ coll
      match w, cv with
      -- the entries of a dict, each given to the condition in the order of the keys
      | .filterD, .dict ps => do
        let answers ← evalEach C n dep ρ x body c .map (ps.map entryV)
        answersOK .filter line answers
        sized (.dict (dictFrom key (((ps.zip answers).filter fun p => isTrue p.2).map (·.1))))
      -- the values of a dict, each given to the function, its answer read in the dict's values' type
      | .mapValues, .dict ps => do
        let answers ← evalEach C n dep ρ x body c .map (ps.map (·.2))
        sized (.dict (dictFrom key ((ps.zip answers).map fun (p, u) => (p.1, u))))
      | .filterD, _ => failK .type line
      | .mapValues, _ => failK .type line
      -- `last` walks the list from its end, and stops at the element that holds
      | .last, .list xs => do
        let answers ← evalEach C n dep ρ x body c .first xs.reverse
        answersOK .first line answers
        .ok (foldK w key xs.reverse answers)
      | _, .list xs => do
        let answers ← evalEach C n dep ρ x body c .map xs
        -- the lists `flat_map`'s function answers, as the check of types holds them
        if w == .flatMap && !answers.all (fun a => (listOf a).isSome) then failK .type line
        -- a group can be as long as the list: a list held to 2⁶³ − 1 elements, as `sized` holds one
        else if w == .groupBy && i64max < (xs.length : Int) then .error .diverged
        else sized (foldK w key xs answers)
      | _, _ => failK .type line
    | .toDict _ line key list x kb kc y vb vc => do
      let lv ← eval C n dep ρ list
      match lv with
      | .list xs => do
        let ps ← evalEach2 C n dep ρ x kb kc y vb vc xs
        sized (.dict (dictFrom key ps))
      | _ => failK .type line
    | .mergeWith _ line key a b x y body c => do
      let av ← eval C n dep ρ a
      let bv ← eval C n dep ρ b
      match av, bv with
      | .dict ps, .dict qs => do
        let more ← evalMerge C n dep ρ key ps x y body c qs
        sized (.dict (dictFrom key (ps ++ more)))
      | _, _ => failK .type line
    -- a lambda made a value, with the values of the names in scope (DESIGN 27.3)
    | .lam id => .ok (.closure id ρ)
    -- a function value called: what it is, then its arguments from the left, each read in its
    -- parameter's unit, then the call, one more call deep (DESIGN 27.3): the lambda's body with the
    -- names it caught and its parameters, its answer read in the type of the lambda's answer
    | .callV _ line f args _ => do
      let fv ← eval C n dep ρ f
      let vs ← evalList C n dep ρ (args.map (·.1))
      let vs ← coerceAll args vs
      match fv with
      | .closure i env =>
        match C.M.lams.find? (·.id == i) with
        | some lam =>
          if DEPTH ≤ dep then .error .diverged
          else do
            let r ← eval C n (dep + 1) (((lam.params.map (·.1)).zip vs).reverse ++ env) lam.body
            coerce lam.conv r
        | Option.none => failK .type line
      | _ => failK .type line
    | .bad _ line _ => failK .type line
termination_by structural fuel _ _ _ => fuel

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
termination_by structural fuel _ _ _ => fuel

def evalArgs (C : Ctx) : Nat → Nat → Env → List (Nat × Expr × Option Conv) → R (List Val)
  | 0, _, _, _ => .error .fuel
  | _ + 1, _, _, [] => .ok []
  | n + 1, dep, ρ, (_, x, _) :: rest => do
    let v ← eval C n dep ρ x
    let vs ← evalArgs C n dep ρ rest
    .ok (v :: vs)
termination_by structural fuel _ _ _ => fuel

def evalArgs4 (C : Ctx) : Nat → Nat → Env → List (String × Expr × Option Conv × Nat × Bool) → R (List Val)
  | 0, _, _, _ => .error .fuel
  | _ + 1, _, _, [] => .ok []
  | n + 1, dep, ρ, (_, x, _) :: rest => do
    let v ← eval C n dep ρ x
    let vs ← evalArgs4 C n dep ρ rest
    .ok (v :: vs)
termination_by structural fuel _ _ _ => fuel

def evalInits (C : Ctx) : Nat → Nat → Env → List (Nat × Expr × Option Conv × Nat) → R (List Val)
  | 0, _, _, _ => .error .fuel
  | _ + 1, _, _, [] => .ok []
  | n + 1, dep, ρ, (_, x, _) :: rest => do
    let v ← eval C n dep ρ x
    let vs ← evalInits C n dep ρ rest
    .ok (v :: vs)
termination_by structural fuel _ _ _ => fuel

def evalElems (C : Ctx) : Nat → Nat → Env → List (Expr × Option Conv) → R (List Val)
  | 0, _, _, _ => .error .fuel
  | _ + 1, _, _, [] => .ok []
  | n + 1, dep, ρ, (x, c) :: rest => do
    let v ← eval C n dep ρ x
    let v ← coerce c v
    let vs ← evalElems C n dep ρ rest
    .ok (v :: vs)
termination_by structural fuel _ _ _ => fuel

def evalList (C : Ctx) : Nat → Nat → Env → List Expr → R (List Val)
  | 0, _, _, _ => .error .fuel
  | _ + 1, _, _, [] => .ok []
  | n + 1, dep, ρ, x :: rest => do
    let v ← eval C n dep ρ x
    let vs ← evalList C n dep ρ rest
    .ok (v :: vs)
termination_by structural fuel _ _ _ => fuel

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
termination_by structural fuel _ _ _ _ _ _ _ => fuel

/-- The pairs of a dict written out: each key and then its value, each read in its type as it comes. -/
def evalPairs (C : Ctx) : Nat → Nat → Env → List (Expr × Option Conv × Expr × Option Conv) → R (List (Val × Val))
  | 0, _, _, _ => .error .fuel
  | _ + 1, _, _, [] => .ok []
  | n + 1, dep, ρ, (x, cx, y, cy) :: rest => do
    let a ← eval C n dep ρ x
    let a ← coerce cx a
    let b ← eval C n dep ρ y
    let b ← coerce cy b
    let more ← evalPairs C n dep ρ rest
    .ok ((a, b) :: more)
termination_by structural fuel _ _ _ => fuel

/-- `to_dict`'s two functions, for each element in turn its key and then its value, each read in
    the dict's type. -/
def evalEach2 (C : Ctx) : Nat → Nat → Env → String → Expr → Option Conv → String → Expr → Option Conv → List Val →
    R (List (Val × Val))
  | 0, _, _, _, _, _, _, _, _, _ => .error .fuel
  | _ + 1, _, _, _, _, _, _, _, _, [] => .ok []
  | n + 1, dep, ρ, x, kb, kc, y, vb, vc, el :: rest => do
    let a ← eval C n dep ((x, el) :: ρ) kb
    let a ← coerce kc a
    let b ← eval C n dep ((y, el) :: ρ) vb
    let b ← coerce vc b
    let more ← evalEach2 C n dep ρ x kb kc y vb vc rest
    .ok ((a, b) :: more)
termination_by structural fuel _ _ _ _ _ _ _ _ _ => fuel

/-- `merge_with`'s function, for each pair of the second dict in the order of the keys whose key the
    first has: given the first's value and the second's, its answer read in the values' type; the
    second's value where the first has no such key. -/
def evalMerge (C : Ctx) : Nat → Nat → Env → Key → List (Val × Val) → String → String → Expr → Option Conv →
    List (Val × Val) → R (List (Val × Val))
  | 0, _, _, _, _, _, _, _, _, _ => .error .fuel
  | _ + 1, _, _, _, _, _, _, _, _, [] => .ok []
  | n + 1, dep, ρ, key, ps, x, y, body, c, (k, w) :: rest => do
    let v ← match ps.find? (fun p => sameKey p.1 k) with
      | some (_, va) => do
        let r ← eval C n dep ((y, w) :: (x, va) :: ρ) body
        coerce c r
      | Option.none => .ok w
    let more ← evalMerge C n dep ρ key ps x y body c rest
    .ok ((k, v) :: more)
termination_by structural fuel _ _ _ _ _ _ _ _ _ => fuel

/-- `reduce`'s function laid over each element, the running value read in the type of `first`. -/
def evalFold (C : Ctx) : Nat → Nat → Env → String → String → Expr → Option Conv → List Val → Val → R Val
  | 0, _, _, _, _, _, _, _, _ => .error .fuel
  | _ + 1, _, _, _, _, _, _, [], a => .ok a
  | n + 1, dep, ρ, acc, x, body, c, el :: rest, a => do
    let v ← eval C n dep ((x, el) :: (acc, a) :: ρ) body
    let a ← coerce c v
    evalFold C n dep ρ acc x body c rest a
termination_by structural fuel _ _ _ _ _ _ _ _ => fuel
end

end KanameCore
