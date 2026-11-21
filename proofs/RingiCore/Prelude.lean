/-
  The few things the machine's code reads from its language that Lean does not say the same way:
  a list's first element that holds, the elements with their places, a stable sort, the numbers
  from one to another, a day written as text, white space.
-/
import RingiCore.Types

namespace RingiCore

/-- The first element that holds. -/
def first? {α} (l : List α) (p : α → Bool) : Option α := l.find? p

/-- The last element that holds. -/
def last? {α} (l : List α) (p : α → Bool) : Option α := l.reverse.find? p

/-- How many elements hold. -/
def countIf {α} (l : List α) (p : α → Bool) : Int := (l.filter p).length

/-- An element with its place in its list, counted from 0. -/
structure Indexed (α : Type) where
  index : Int
  item : α

def indexed {α} (l : List α) : List (Indexed α) :=
  (l.zipIdx).map (fun (x, i) => { index := i, item := x })

/-- The element at a place, none past either end. -/
def at? {α} (l : List α) (i : Int) : Option α :=
  if i < 0 then none else l[i.toNat]?

/-- The first `n` elements (all of them for more, none for fewer than one). -/
def takeI {α} (l : List α) (n : Int) : List α := l.take n.toNat

/-- The elements after the first `n`. -/
def dropI {α} (l : List α) (n : Int) : List α := l.drop n.toNat

/-- The elements, each once, where it first comes. -/
def distinct {α} [BEq α] (l : List α) : List α :=
  l.foldl (fun acc x => if acc.contains x then acc else acc ++ [x]) []

/-- A stable sort by a key: the elements of one key keep their order. -/
def sortBy {α} (l : List α) (key : α → Int) : List α :=
  l.mergeSort (fun a b => decide (key a ≤ key b))

/-- The numbers from `a` to `b`, `a` and `b` among them. -/
def numbers (a b : Int) : List Int :=
  if b < a then [] else (List.range (b - a + 1).toNat).map (fun (k : Nat) => a + Int.ofNat k)

/-- A number as text. -/
def textInt (n : Int) : String := toString n

/-- A text of at least `n` characters, filled from the start with `c`. -/
def padStart (s : String) (n : Nat) (c : Char) : String :=
  String.ofList (List.replicate (n - s.length) c) ++ s

/-! Days: the days since 1970-01-01, as Hinnant's `civil_from_days` and `days_from_civil` count them. -/

/-- The year, month and day of a day number. -/
def civil (z : Int) : Int × Int × Int :=
  let z := z + 719468
  let era := (if z ≥ 0 then z else z - 146096) / 146097
  let doe := z - era * 146097
  let yoe := (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365
  let y := yoe + era * 400
  let doy := doe - (365 * yoe + yoe / 4 - yoe / 100)
  let mp := (5 * doy + 2) / 153
  let d := doy - (153 * mp + 2) / 5 + 1
  let m := if mp < 10 then mp + 3 else mp - 9
  (if m ≤ 2 then y + 1 else y, m, d)

/-- The day number of a year, month and day. -/
def daysFromCivil (y m d : Int) : Int :=
  let y := if m ≤ 2 then y - 1 else y
  let era := (if y ≥ 0 then y else y - 399) / 400
  let yoe := y - era * 400
  let mp := if m > 2 then m - 3 else m + 9
  let doy := (153 * mp + 2) / 5 + d - 1
  let doe := yoe * 365 + yoe / 4 - yoe / 100 + doy
  era * 146097 + doe - 719468

/-- A day as the JSON and the history write it: `2026-11-16`. -/
def dateText (day : Int) : String :=
  let (y, m, d) := civil day
  s!"{padStart (toString y) 4 '0'}-{padStart (toString m) 2 '0'}-{padStart (toString d) 2 '0'}"

/-- A day written `YYYY-MM-DD`, as its day number. -/
def parseDate (s : String) : Option Int :=
  match s.splitOn "-" with
  | [y, m, d] =>
    match y.toNat?, m.toNat?, d.toNat? with
    | some y, some m, some d =>
      if m ≥ 1 ∧ m ≤ 12 ∧ d ≥ 1 ∧ d ≤ 31 then some (daysFromCivil y m d) else none
    | _, _, _ => none
  | _ => none

/-! Text. -/

/-- The code points of Unicode's White_Space, which the machine's `trim` takes for white space. -/
def isSpace (c : Char) : Bool :=
  let n := c.toNat
  (n ≥ 0x9 ∧ n ≤ 0xD) ∨ n = 0x20 ∨ n = 0x85 ∨ n = 0xA0 ∨ n = 0x1680 ∨ (n ≥ 0x2000 ∧ n ≤ 0x200A) ∨
    n = 0x2028 ∨ n = 0x2029 ∨ n = 0x202F ∨ n = 0x205F ∨ n = 0x3000

/-- The text without the white space at either end. -/
def trimSpace (s : String) : String :=
  String.ofList ((s.toList.dropWhile isSpace).reverse.dropWhile isSpace).reverse

/-- The number of code points. -/
def chars (s : String) : Int := s.length

end RingiCore
