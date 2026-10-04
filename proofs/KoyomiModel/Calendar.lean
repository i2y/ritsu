/-
  Calendars and business days (koyomi's DESIGN 1.4, 1.8, 2.2, 2.4; `crates/koyomi/src/calendar.rs`).

  Which days are closed is data — the tables of holidays, the closed days of the week, the yearly
  and the one-off closings — and `Rules.isOpen` reads it the way `Calendar::is_open` does. The
  business-day operations take any `Cal`, the answer to "is this day open", so what is proved of
  them in `KoyomiModel.Business` holds whatever the calendar says.
-/
import KoyomiModel.Date

namespace KoyomiModel

/-- Whether a day is open, or the error of asking: a day outside the calendar's data. -/
abbrev Cal := Int → Except Fail Bool

/-- A yearly closing, `12-29..01-03` going round the year's end when `from` is after `to`. -/
structure Every where
  fromMD : Nat × Nat
  toMD : Nat × Nat
  deriving Repr

def leMD (a b : Nat × Nat) : Bool := a.1 < b.1 || (a.1 == b.1 && a.2 ≤ b.2)

def Every.covers (e : Every) (m d : Nat) : Bool :=
  if leMD e.fromMD e.toMD then leMD e.fromMD (m, d) && leMD (m, d) e.toMD
  else leMD e.fromMD (m, d) || leMD (m, d) e.toMD

/-- A calendar as koyomi has it once its files are read. -/
structure Rules where
  /-- The days it knows. -/
  dataFrom : Int
  dataTo : Int
  /-- Minutes east of UTC. -/
  offset : Option Int
  /-- Closed days of the week, Monday first. -/
  weekly : Array Bool
  every : List Every
  days : List (Int × Int)
  opens : List (Int × Int)
  /-- The days its tables close, in order. -/
  holidays : Array Int
  deriving Repr

/-- Whether a sorted array holds `x`, by halving: at most 64 halvings for any array there is. -/
def memSorted (a : Array Int) (x : Int) : Bool := go 64 0 a.size
where
  go : Nat → Nat → Nat → Bool
    | 0, _, _ => false
    | fuel + 1, lo, hi =>
      if lo < hi then
        let mid := (lo + hi) / 2
        let v := a[mid]!
        if v = x then true else if v < x then go fuel (mid + 1) hi else go fuel lo mid
      else false

def within (spans : List (Int × Int)) (z : Int) : Bool := spans.any (fun s => decide (s.1 ≤ z) && decide (z ≤ s.2))

def Rules.closedByRules (c : Rules) (z : Int) : Bool :=
  c.weekly[weekday z]!.or <|
  (memSorted c.holidays z).or <|
  (let (_, m, d) := civilFromDays z; c.every.any (·.covers m d)).or <|
  within c.days z

/-- A day is open when an `open` line names it, or when no `closed` line does; outside the data
    the calendar does not know. -/
def Rules.isOpen (c : Rules) : Cal := fun z =>
  if z < c.dataFrom ∨ z > c.dataTo then .error (.outside z)
  else if within c.opens z then .ok true
  else .ok (!c.closedByRules z)

/-- Enough steps to walk across every day there is. -/
def fuel : Nat := (maxDay - minDay + 2).toNat

def step (forward : Bool) : Int := if forward then 1 else -1

/-- The first business day on or after `z` (or on or before it). -/
def seek (cal : Cal) (forward : Bool) : Nat → Int → Except Fail Int
  | 0, _ => .error (.bug "a walk longer than every day there is")
  | n + 1, z =>
    match cal z with
    | .error e => .error e
    | .ok true => .ok z
    | .ok false =>
      match plus z (step forward) with
      | .error e => .error e
      | .ok w => seek cal forward n w

/-- Counting business days after (before) `z`: `k` found so far, `n` wanted. -/
def count (cal : Cal) (forward : Bool) (n : Nat) : Nat → Int → Nat → Except Fail Int
  | 0, _, _ => .error (.bug "a walk longer than every day there is")
  | f + 1, z, k =>
    match plus z (step forward) with
    | .error e => .error e
    | .ok w =>
      match cal w with
      | .error e => .error e
      | .ok true => if k + 1 = n then .ok w else count cal forward n f w (k + 1)
      | .ok false => count cal forward n f w k

/-- `+ n business days` (`forward`) and `- n business days`: the `n`-th business day after
    (before) `z`, counting from the day after it whether `z` is open or not; with `n` = 0, `z`
    rolled to a business day in the same direction (DESIGN 1.8). -/
def addBusiness (cal : Cal) (z : Int) (n : Nat) (forward : Bool) : Except Fail Int :=
  if n = 0 then seek cal forward fuel z else count cal forward n fuel z 0

def sameMonth (a b : Int) : Bool :=
  let (ya, ma, _) := civilFromDays a
  let (yb, mb, _) := civilFromDays b
  ya == yb && ma == mb

inductive Conv where
  | following
  | preceding
  | modifiedFollowing
  | modifiedPreceding
  deriving DecidableEq, Repr

/-- The four conventions. -/
def roll (cal : Cal) (z : Int) : Conv → Except Fail Int
  | .following => seek cal true fuel z
  | .preceding => seek cal false fuel z
  | .modifiedFollowing =>
    match seek cal true fuel z with
    | .error e => .error e
    | .ok f => if sameMonth f z then .ok f else seek cal false fuel z
  | .modifiedPreceding =>
    match seek cal false fuel z with
    | .error e => .error e
    | .ok p => if sameMonth p z then .ok p else seek cal true fuel z

end KoyomiModel
