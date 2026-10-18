/-
  The values of kaname (kaname's DESIGN 8), the failures an evaluation ends in (DESIGN 4.2, 4.4),
  and the abstract values of the analysis of ranges (DESIGN 5.1; `ranges.rs`, `Abs`).

  A number is an exact rational in the unit of its expression's type; a date its day number; a
  value of an enum the name it is declared with; a record its fields by name in the order declared;
  `none` the one value that is not there (a value that is there is itself, not wrapped); a transfer
  the operation it calls and its arguments.

  The failures are kaname's six (`input` and the five kinds of the effects) and one more, `type`:
  what the check of types rules out (a value off the step its type says, an argument outside the
  values of its type, a `match` no arm of which takes the value). Rust's evaluation never meets
  one, because its types hold; the model has no types, so it says where it leans on them, and the
  theorems say that every failure of a module whose places are shown is one of these.
-/
import KanameCore.Iv

namespace KanameCore

/-- A value. -/
inductive Val where
  | num (r : Rat)
  | bool (b : Bool)
  | str (s : String)
  | date (d : Int)
  | enum (v : String)
  | record (fs : List (String × Val))
  | list (xs : List Val)
  | none
  | transfer (id : String) (args : List (String × Val))
  deriving Repr, Inhabited

mutual
def Val.beq : Val → Val → Bool
  | .num a, .num b => a == b
  | .bool a, .bool b => a == b
  | .str a, .str b => a == b
  | .date a, .date b => a == b
  | .enum a, .enum b => a == b
  | .record a, .record b => Val.beqFields a b
  | .list a, .list b => Val.beqList a b
  | .none, .none => true
  | .transfer i a, .transfer j b => i == j && Val.beqFields a b
  | _, _ => false
def Val.beqFields : List (String × Val) → List (String × Val) → Bool
  | [], [] => true
  | (n, x) :: xs, (m, y) :: ys => n == m && Val.beq x y && Val.beqFields xs ys
  | _, _ => false
def Val.beqList : List Val → List Val → Bool
  | [], [] => true
  | x :: xs, y :: ys => Val.beq x y && Val.beqList xs ys
  | _, _ => false
end

instance : BEq Val := ⟨Val.beq⟩

/-- The value of a field of a record, by its declared name. -/
def Val.field? : Val → String → Option Val
  | .record fs, n => (fs.find? (·.1 == n)).map (·.2)
  | _, _ => Option.none

/-- A failure's kind. -/
inductive Kind where
  | input
  | rule
  | date
  | range
  | overflow
  | zero
  | type
  deriving DecidableEq, Repr, Inhabited

def Kind.word : Kind → String
  | .input => "input"
  | .rule => "rule"
  | .date => "date"
  | .range => "range"
  | .overflow => "overflow"
  | .zero => "zero"
  | .type => "type"

/-- One of the five kinds of the effects (DESIGN 4.2): what a place that is not shown can end in. -/
def Kind.ofEffect : Kind → Bool
  | .rule | .date | .range | .overflow | .zero => true
  | _ => false

/-- Why an evaluation ends with no answer: a failure at a line, a call deeper than the interpreter
    goes (DESIGN 8), or no more steps to take (`fuel`: never met in a run, and no failure). -/
inductive Stop where
  | failed (k : Kind) (line : Nat)
  | diverged
  | fuel
  deriving DecidableEq, Repr, Inhabited

/-- How two fields of a record are known to compare. -/
inductive Rel where
  | lt
  | le
  | eq
  deriving DecidableEq, Repr, Inhabited

/-- What an expression's values can be (DESIGN 5.1). A set is kept sorted, as Rust's `BTreeSet`. -/
inductive Abs where
  /-- No value: what is never reached. -/
  | never
  /-- Anything. -/
  | any
  | num (i : Iv)
  | date (i : Iv)
  /-- The values an enum or a bool can be (`true`, `false`); none for any. -/
  | set (s : Option (List String))
  | str
  /-- A value that may be none, and what it is when it is there. -/
  | opt (n : Bool) (a : Abs)
  /-- A list: the interval of its count, and what each element can be. -/
  | list (c : Iv) (e : Abs)
  /-- A record's fields, and what is known of how they compare. -/
  | record (fields : List (String × Abs)) (rels : List (String × Rel × String))
  | transfer (args : List (String × Abs))
  deriving Repr, Inhabited

mutual
def Abs.beq : Abs → Abs → Bool
  | .never, .never => true
  | .any, .any => true
  | .num a, .num b => a == b
  | .date a, .date b => a == b
  | .set a, .set b => a == b
  | .str, .str => true
  | .opt n a, .opt m b => n == m && Abs.beq a b
  | .list c a, .list d b => c == d && Abs.beq a b
  | .record f r, .record g s => Abs.beqFields f g && r == s
  | .transfer f, .transfer g => Abs.beqFields f g
  | _, _ => false
def Abs.beqFields : List (String × Abs) → List (String × Abs) → Bool
  | [], [] => true
  | (n, x) :: xs, (m, y) :: ys => n == m && Abs.beq x y && Abs.beqFields xs ys
  | _, _ => false
end

instance : BEq Abs := ⟨Abs.beq⟩

/-- A sorted set of strings, with one more inserted. -/
def insertSorted (x : String) : List String → List String
  | [] => [x]
  | y :: ys => if x < y then x :: y :: ys else if x = y then y :: ys else y :: insertSorted x ys

def setOf (xs : List String) : List String := xs.foldl (fun s x => insertSorted x s) []

def Abs.bools : Abs := .set (some ["false", "true"])

def Abs.none' : Abs := .opt true .never

/-- A bool as the set of the one value. -/
def boolAbs (b : Bool) : Abs := .set (some [if b then "true" else "false"])

/-- `bools(t, f)`: the truths a condition can have. -/
def bools (t f : Bool) : Abs :=
  .set (some ((if f then ["false"] else []) ++ (if t then ["true"] else [])))

/-- Whether two numbers keep a relation. -/
def Rel.holds : Rel → Rat → Rat → Bool
  | .lt, a, b => decide (a < b)
  | .le, a, b => decide (a ≤ b)
  | .eq, a, b => a == b

namespace Abs

/-- The interval of a number or a date, through an optional. -/
def iv : Abs → Option Iv
  | num i => some i
  | date i => some i
  | opt _ x => x.iv
  | _ => none

/-- The value when one is there. -/
def present : Abs → Abs
  | opt _ x => x
  | x => x

def mayBeNone : Abs → Bool
  | opt true _ => true
  | _ => false

def field : Abs → String → Abs
  | record fs _, n => match fs.find? (·.1 == n) with
    | some (_, a) => a
    | Option.none => any
  | opt _ x, n => x.field n
  | never, _ => never
  | _, _ => any

/-- The abstract value of the first field of a record named `n`. -/
def fieldOf (fs : List (String × Abs)) (n : String) : Option Abs := (fs.find? (·.1 == n)).map (·.2)

/-- The members of either set; none (every member) where either is none. -/
def setUnion : Option (List String) → Option (List String) → Option (List String)
  | some x, some y => some (y.foldl (fun s v => insertSorted v s) x)
  | _, _ => Option.none

/-- A field joined with the same field of the second record (anything where the second has none). -/
def joinField (g : Abs → Abs → Abs) (fb : List (String × Abs)) (n : String) (x : Abs) : Abs :=
  match fieldOf fb n with
  | some y => g x y
  | Option.none => any

/-- The fields of the first, each joined with the same field of the second. -/
def joinFields (g : Abs → Abs → Abs) (fb : List (String × Abs)) : List (String × Abs) → List (String × Abs)
  | [] => []
  | (n, x) :: rest => (n, joinField g fb n x) :: joinFields g fb rest

/-- Both: every value of either. -/
def joinD : Nat → Abs → Abs → Abs
  | 0, _, _ => any
  | d + 1, a, b =>
    match a, b with
    | never, x => x
    | x, never => x
    | any, _ => any
    | _, any => any
    | num a, num b => num (a.hull b)
    | date a, date b => date (a.hull b)
    | set a, set b => set (setUnion a b)
    | str, str => str
    | opt n x, opt m y => opt (n || m) (joinD d x y)
    | opt n x, y => opt n (joinD d x y)
    | y, opt n x => opt n (joinD d x y)
    | list c x, list e y => list (c.hull e) (joinD d x y)
    | record a ra, record b rb => record (joinFields (joinD d) b a) (ra.filter (fun r => rb.contains r))
    | transfer a, transfer b => transfer (joinFields (joinD d) b a)
    | _, _ => any

def join (a b : Abs) : Abs := joinD D a b

/-- Whether a value is among these (`ranges::Abs::has`), the relations of a record's fields too. -/
def hasD : Nat → Abs → Val → Bool
  | 0, _, _ => false
  | d + 1, a, v =>
    match a, v with
    | never, _ => false
    | any, _ => true
    | opt n _, .none => n
    | opt _ x, v => hasD d x v
    | _, .none => false
    | num i, .num r => i.has r
    | date i, .date x => i.has x
    | set Option.none, .enum _ => true
    | set Option.none, .bool _ => true
    | set (some s), .enum x => s.contains x
    | set (some s), .bool b => s.contains (if b then "true" else "false")
    | str, .str _ => true
    | list c e, .list xs => c.has xs.length && xs.all (fun x => hasD d e x)
    | record fs rels, .record vs =>
      vs.all (fun (n, x) => match fieldOf fs n with
        | some a => hasD d a x
        | Option.none => true) &&
      rels.all (fun (a, r, b) =>
        match ((vs.find? (·.1 == a)).map (·.2) : Option Val), ((vs.find? (·.1 == b)).map (·.2) : Option Val) with
        | some (Val.num x), some (Val.num y) => r.holds x y
        | _, _ => false)
    | transfer as, .transfer _ vs =>
      vs.all (fun (n, x) => match fieldOf as n with
        | some a => hasD d a x
        | Option.none => true)
    | _, _ => false

def has (a : Abs) (v : Val) : Bool := hasD D a v

/-- The same values, the numbers read in another unit: times `f`, plus `shift`. -/
def scaled (a : Abs) (f shift : Rat) : Abs :=
  match a with
  | num i =>
    let x := i.scale f
    let x := if shift.num = 0 then x else x.plus (Iv.exact shift)
    num { x with den := i.den.map (· * f.den * shift.den) }
  | opt n x => opt n (x.scaled f shift)
  | list c x => list c (x.scaled f shift)
  | x => x

end Abs

end KanameCore
