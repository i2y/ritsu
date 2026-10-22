/-
  Strings (kaname's DESIGN 25, `text.rs`): a string is the list of its code points (Lean's `Char`, a
  Unicode scalar value), and every builtin of strings counts, cuts, finds and compares in code points.
  The steps are kaname's own, as the reference interpreter and the helper modules of the generated
  code take them: white space is the 25 code points of Unicode's White_Space, upper and lower case
  are A to Z and a to z alone, and a number and a date are read in one form only. Lean's
  `String.trim` takes off the ASCII white space alone, so the model keeps its own table.

  Nothing here fails: a cut past either end is cut to the string, a place that is not found and a
  number or a date that cannot be read are none. The one bound is how long a string padding makes
  (`LONGEST`), past which the evaluation stops (`Eval.lean`), as a list of more numbers does.
-/
import KanameCore.Num

namespace KanameCore

namespace Text

/-- The code points of Unicode's White_Space (DESIGN 25.3). -/
def SPACES : List Nat :=
  [0x9, 0xA, 0xB, 0xC, 0xD, 0x20, 0x85, 0xA0, 0x1680, 0x2000, 0x2001, 0x2002, 0x2003, 0x2004, 0x2005,
    0x2006, 0x2007, 0x2008, 0x2009, 0x200A, 0x2028, 0x2029, 0x202F, 0x205F, 0x3000]

def isSpace (c : Char) : Bool := SPACES.contains c.toNat

/-- A whole number between 0 and `n`, where one past either end is cut to it. -/
def cut (n : Nat) (i : Int) : Nat := if i < 0 then 0 else min i.toNat n

/-- The code points from `a` up to before `b`, both cut to the string. -/
def slice (cs : List Char) (a b : Int) : List Char :=
  let i := cut cs.length a
  let j := cut cs.length b
  if i < j then (cs.drop i).take (j - i) else []

def take (cs : List Char) (n : Int) : List Char := cs.take (cut cs.length n)

def drop (cs : List Char) (n : Int) : List Char := cs.drop (cut cs.length n)

/-- Where `t` first stands in what is left, counted from `i`. -/
def findFrom (t : List Char) (i : Nat) : List Char → Option Nat
  | [] => if t.isPrefixOf [] then some i else none
  | c :: rest => if t.isPrefixOf (c :: rest) then some i else findFrom t (i + 1) rest

/-- Where `t` first stands in `cs`, from 0; 0 for an empty `t`, none where it is not there. -/
def indexOf (cs t : List Char) : Option Nat := findFrom t 0 cs

def contains (cs t : List Char) : Bool := (indexOf cs t).isSome

def endsWith (cs t : List Char) : Bool := t.isSuffixOf cs

/-- `cs` cut at each `sep` (not empty) from the left, none of two cuts overlapping: the piece so far,
    backwards, and the pieces after it. -/
def splitAux (sep : List Char) (hs : sep ≠ []) (cur : List Char) : List Char → List (List Char)
  | [] => [cur.reverse]
  | c :: rest =>
    if sep.isPrefixOf (c :: rest) then cur.reverse :: splitAux sep hs [] ((c :: rest).drop sep.length)
    else splitAux sep hs (c :: cur) rest
termination_by cs => cs.length
decreasing_by
  · have : 0 < sep.length := List.length_pos_iff.mpr hs
    simp only [List.length_drop, List.length_cons]
    omega
  · simp

/-- The pieces of `cs` between the `sep`s, the empty ones at the ends kept; one for each code point
    where `sep` is empty. -/
def split (cs sep : List Char) : List (List Char) :=
  if h : sep = [] then cs.map fun c => [c] else splitAux sep h [] cs

/-- The runs of what is not white space: the run so far, backwards, and the runs after it. -/
def wordsAux (cur : List Char) : List Char → List (List Char)
  | [] => if cur.isEmpty then [] else [cur.reverse]
  | c :: rest =>
    if isSpace c then (if cur.isEmpty then wordsAux [] rest else cur.reverse :: wordsAux [] rest)
    else wordsAux (c :: cur) rest

def words (cs : List Char) : List (List Char) := wordsAux [] cs

/-- The pieces one after another, `sep` between two. -/
def joinWith (sep : List Char) : List (List Char) → List Char
  | [] => []
  | [x] => x
  | x :: rest => x ++ sep ++ joinWith sep rest

/-- Every `old` from the left made `new`, none of two overlapping; `cs` as it is where `old` is
    empty. -/
def replace (cs old new : List Char) : List Char :=
  if old = [] then cs else joinWith new (split cs old)

def trimStart (cs : List Char) : List Char := cs.dropWhile isSpace

def trimEnd (cs : List Char) : List Char := (cs.reverse.dropWhile isSpace).reverse

def trim (cs : List Char) : List Char := trimEnd (trimStart cs)

/-- A to Z made a to z, nothing else changed (Lean's `Char.toLower` changes the ASCII letters alone). -/
def lowerAscii (cs : List Char) : List Char := cs.map Char.toLower

def upperAscii (cs : List Char) : List Char := cs.map Char.toUpper

/-- `k` code points of `fill`, repeated from its start. -/
def padding (fill : List Char) (k : Nat) : List Char :=
  (List.range k).map fun i => fill.getD (i % fill.length) ' '

/-- `fill` repeated before `cs` (after it, with `atEnd`) to make it `width` long, the last repetition
    cut; `cs` as it is where `fill` is empty or `cs` is that long already. -/
def pad (cs : List Char) (width : Int) (fill : List Char) (atEnd : Bool) : List Char :=
  if fill = [] ∨ width ≤ (cs.length : Int) then cs
  else
    let added := padding fill (width - cs.length).toNat
    if atEnd then cs ++ added else added ++ cs

/-- The longest string padding makes (`text::LONGEST`): a width past it stops the evaluation. -/
def LONGEST : Nat := 1000000

/-- The number the ASCII digits write, after `acc`; none where one is no digit. -/
def digitsVal : List Char → Nat → Option Nat
  | [], acc => some acc
  | c :: rest, acc => if '0' ≤ c ∧ c ≤ '9' then digitsVal rest (acc * 10 + (c.toNat - '0'.toNat)) else none

/-- The number `cs` writes: one `+` or `-` at most, then one ASCII digit or more, within int64
    (DESIGN 25.4). -/
def parseNumber (cs : List Char) : Option Int :=
  let (neg, ds) : Bool × List Char := match cs with
    | '+' :: r => (false, r)
    | '-' :: r => (true, r)
    | _ => (false, cs)
  if ds = [] then none
  else match digitsVal ds 0 with
    | none => none
    | some v =>
      let x : Int := if neg then -(v : Int) else (v : Int)
      if i64min ≤ x ∧ x ≤ i64max then some x else none

/-- The day numbers of 0001-01-01 and 9999-12-31. -/
def DAY_MIN : Int := -719162
def DAY_MAX : Int := 2932896

def leap (y : Int) : Bool := (y % 4 == 0 && y % 100 != 0) || y % 400 == 0

def monthLen (y m : Int) : Int :=
  if m = 2 then (if leap y then 29 else 28)
  else if m = 4 ∨ m = 6 ∨ m = 9 ∨ m = 11 then 30 else 31

/-- The day number of a date: Howard Hinnant's `days_from_civil`, with Rust's division. -/
def daysFromCivil (y m d : Int) : Int :=
  let y := if m ≤ 2 then y - 1 else y
  let era := (if y ≥ 0 then y else y - 399).tdiv 400
  let yoe := y - era * 400
  let doy := (153 * (m + (if m > 2 then -3 else 9)) + 2).tdiv 5 + d - 1
  let doe := yoe * 365 + yoe.tdiv 4 - yoe.tdiv 100 + doy
  era * 146097 + doe - 719468

/-- The date `cs` writes: `YYYY-MM-DD` alone, a day of 0001-01-01 to 9999-12-31 (DESIGN 25.4). -/
def parseDate (cs : List Char) : Option Int :=
  match cs with
  | [y1, y2, y3, y4, '-', m1, m2, '-', d1, d2] =>
    match digitsVal [y1, y2, y3, y4] 0, digitsVal [m1, m2] 0, digitsVal [d1, d2] 0 with
    | some y, some m, some d =>
      if 1 ≤ y ∧ 1 ≤ m ∧ m ≤ 12 ∧ 1 ≤ d ∧ (d : Int) ≤ monthLen y m then
        let z := daysFromCivil y m d
        if DAY_MIN ≤ z ∧ z ≤ DAY_MAX then some z else none
      else none
    | _, _, _ => none
  | _ => none

/-- Whether a number is a Unicode scalar value: 0 to U+10FFFF, but for the surrogates. -/
def isScalar (n : Int) : Bool := 0 ≤ n && n ≤ 0x10FFFF && !(0xD800 ≤ n && n ≤ 0xDFFF)

/-- The code points these numbers are; none where one is no scalar value. -/
def ofCodePoints (ns : List Int) : Option (List Char) :=
  ns.mapM fun n => if isScalar n then some (Char.ofNat n.toNat) else none

end Text

end KanameCore
