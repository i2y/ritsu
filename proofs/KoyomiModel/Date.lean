/-
  koyomi's dates and the operations on them that need no calendar (koyomi's DESIGN 2.1, 2.2;
  `crates/koyomi/src/date.rs`).

  A date is a day of the proleptic Gregorian calendar from 0001-01-01 to 9999-12-31, held as the
  number of days since 1970-01-01. The conversions are Howard Hinnant's `days_from_civil` and
  `civil_from_days`, written with the division Rust's `/` and `%` do on `i64` (towards zero,
  `Int.tdiv` and `Int.tmod`), and `div_euclid` and `rem_euclid` where Rust uses those
  (`Int.ediv` and `Int.emod`, the same for a positive divisor). Nothing here is cleverer than
  `date.rs`: the point is to be the same function, written a second time.
-/

namespace KoyomiModel

/-- 0001-01-01. -/
def minDay : Int := -719162
/-- 9999-12-31. -/
def maxDay : Int := 2932896

/-- Why an operation gives no day (`interp.rs`'s `OpFail`). -/
inductive Fail where
  /-- `else reject` on a day the month does not have (E202). -/
  | reject (y : Int) (m d : Nat)
  /-- The calendar was asked about a day outside its data (E203). -/
  | outside (day : Int)
  /-- Outside 0001-01-01..9999-12-31 (E204). -/
  | outOfRange
  /-- What the check said cannot happen. -/
  | bug (what : String)
  deriving Repr

/-- The kind of error the generated code raises for it (`vectors.rs`'s `kind_of`). -/
def Fail.kind : Fail → String
  | .reject .. => "reject"
  | .outside _ => "data"
  | .outOfRange => "date"
  | .bug _ => "bug"

/-- What an operation does when it lands on a day the month does not have (DESIGN 1.7). -/
inductive Missing where
  | endOfMonth
  | startOfNextMonth
  | reject
  /-- The operation cannot land there, as the check worked out. -/
  | never
  deriving DecidableEq, Repr

/-- Days from 1970-01-01 to the given date. -/
def daysFromCivil (y : Int) (m d : Nat) : Int :=
  let y := if m ≤ 2 then y - 1 else y
  let era := (if y ≥ 0 then y else y - 399).tdiv 400
  let yoe := y - era * 400
  let mp := ((m : Int) + 9).tmod 12
  let doy := (153 * mp + 2).tdiv 5 + (d : Int) - 1
  let doe := yoe * 365 + yoe.tdiv 4 - yoe.tdiv 100 + doy
  era * 146097 + doe - 719468

/-- The year, month and day of a day number. -/
def civilFromDays (z : Int) : Int × Nat × Nat :=
  let z := z + 719468
  let era := (if z ≥ 0 then z else z - 146096).tdiv 146097
  let doe := z - era * 146097
  let yoe := (doe - doe.tdiv 1460 + doe.tdiv 36524 - doe.tdiv 146096).tdiv 365
  let y := yoe + era * 400
  let doy := doe - (365 * yoe + yoe.tdiv 4 - yoe.tdiv 100)
  let mp := (5 * doy + 2).tdiv 153
  let d := (doy - (153 * mp + 2).tdiv 5 + 1).toNat
  let m := (if mp < 10 then mp + 3 else mp - 9).toNat
  (if m ≤ 2 then y + 1 else y, m, d)

def isLeap (y : Int) : Bool :=
  y.tmod 4 == 0 && (y.tmod 100 != 0 || y.tmod 400 == 0)

def monthLen (y : Int) (m : Nat) : Nat :=
  match m with
  | 1 | 3 | 5 | 7 | 8 | 10 | 12 => 31
  | 4 | 6 | 9 | 11 => 30
  | _ => if isLeap y then 29 else 28

/-- The month `k` months after `(y, m)`; `k` may be negative. -/
def shiftMonth (y : Int) (m : Nat) (k : Int) : Int × Nat :=
  let total := y * 12 + ((m : Int) - 1) + k
  (total.ediv 12, (total.emod 12 + 1).toNat)

def inYears (y : Int) : Bool := decide (1 ≤ y) && decide (y ≤ 9999)

/-- Monday is 0, Sunday 6. 1970-01-01 was a Thursday. -/
def weekday (z : Int) : Nat := ((z + 3).emod 7).toNat

/-- `n` days later, inside the range dates have. -/
def plus (z n : Int) : Except Fail Int :=
  let w := z + n
  if w < minDay ∨ w > maxDay then .error .outOfRange else .ok w

/-- The day a month has on day `d`, or what `p` says to do when it has none. -/
def place (y : Int) (m d : Nat) (p : Missing) : Except Fail Int :=
  if !inYears y then .error .outOfRange else
  let len := monthLen y m
  if d ≤ len then .ok (daysFromCivil y m d) else
  match p with
  | .endOfMonth => .ok (daysFromCivil y m len)
  | .startOfNextMonth =>
    let (ny, nm) := shiftMonth y m 1
    if !inYears ny then .error .outOfRange else .ok (daysFromCivil ny nm 1)
  | .reject => .error (.reject y m d)
  | .never => .error (.bug "a day the check said could not be reached")

/-- `+ n months else p`. -/
def addMonths (z k : Int) (p : Missing) : Except Fail Int :=
  let (y, m, d) := civilFromDays z
  let (y2, m2) := shiftMonth y m k
  place y2 m2 d p

/-- `day n of month +k else p`. -/
def dayOfMonth (z : Int) (n : Nat) (k : Int) (p : Missing) : Except Fail Int :=
  let (y, m, _) := civilFromDays z
  let (y2, m2) := shiftMonth y m k
  place y2 m2 n p

/-- `start of month +k`. -/
def startOfMonth (z k : Int) : Except Fail Int :=
  let (y, m, _) := civilFromDays z
  let (y2, m2) := shiftMonth y m k
  place y2 m2 1 .never

/-- `end of month +k`. -/
def endOfMonth (z k : Int) : Except Fail Int :=
  let (y, m, _) := civilFromDays z
  let (y2, m2) := shiftMonth y m k
  if !inYears y2 then .error .outOfRange else place y2 m2 (monthLen y2 m2) .never

/-- `close day n else p`: of the months' closing days, the earliest on or after `z`. The month
    before matters only under `start_of_next_month`; the month after always closes after `z`. -/
def closeDay (z : Int) (n : Nat) (p : Missing) : Except Fail Int :=
  let (y, m, _) := civilFromDays z
  let ks : List Int := if p = .startOfNextMonth then [-1, 0, 1] else [0, 1]
  go y m ks
where
  go (y : Int) (m : Nat) : List Int → Except Fail Int
    | [] => .error (.bug "no closing day found")
    | k :: rest =>
      let (y2, m2) := shiftMonth y m k
      if !inYears y2 then
        if k < 0 then go y m rest else .error .outOfRange
      else match place y2 m2 n p with
        | .error e => .error e
        | .ok c => if c ≥ z then .ok c else go y m rest

/-- `close end of month`: the last day of `z`'s month. -/
def closeEndOfMonth (z : Int) : Int :=
  let (y, m, _) := civilFromDays z
  daysFromCivil y m (monthLen y m)

def pad (n : Nat) (w : Nat) : String :=
  let s := toString n
  String.ofList (List.replicate (w - s.length) '0') ++ s

/-- `2026-04-01`. -/
def showDay (z : Int) : String :=
  let (y, m, d) := civilFromDays z
  s!"{pad y.toNat 4}-{pad m 2}-{pad d 2}"

end KoyomiModel
