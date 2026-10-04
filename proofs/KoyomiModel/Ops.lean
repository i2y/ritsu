/-
  The operations of a date and a file's dates computed in order (koyomi's DESIGN 1.6, 2.2;
  `crates/koyomi/src/interp.rs`), and what one input of the range gives (`vectors.rs`'s `expect`).
-/
import KoyomiModel.Calendar

namespace KoyomiModel

/-- A number an operation reads: written in the file, or an integer input. -/
inductive A where
  | lit (n : Int)
  | input (k : Nat)
  deriving Repr

def A.value (vals : Array Int) : A → Int
  | .lit n => n
  | .input k => vals[k]!

/-- One operation, as `resolve.rs`'s `ROp`. A sign is ±1. -/
inductive Op where
  | days (sign : Int) (n : A)
  | business (forward : Bool) (n : A)
  | months (sign : Int) (n : A) (per : Int) (missing : Missing)
  | dayOfMonth (n : A) (sign : Int) (k : A) (missing : Missing)
  | startOfMonth (sign : Int) (k : A)
  | endOfMonth (sign : Int) (k : A)
  | closeDay (n : A) (missing : Missing)
  | closeEndOfMonth
  | roll (c : Conv)
  | ifClosed (inner : Op)
  deriving Repr

/-- One operation on one day (DESIGN 2.2). `if closed` does its one operation only on a closed day. -/
def Op.apply (cal : Cal) (vals : Array Int) : Op → Int → Except Fail Int
  | .days s n, z => plus z (s * n.value vals)
  | .business fwd n, z => addBusiness cal z (n.value vals).toNat fwd
  | .months s n per p, z => KoyomiModel.addMonths z (s * per * n.value vals) p
  | .dayOfMonth n s k p, z => KoyomiModel.dayOfMonth z (n.value vals).toNat (s * k.value vals) p
  | .startOfMonth s k, z => KoyomiModel.startOfMonth z (s * k.value vals)
  | .endOfMonth s k, z => KoyomiModel.endOfMonth z (s * k.value vals)
  | .closeDay n p, z => KoyomiModel.closeDay z (n.value vals).toNat p
  | .closeEndOfMonth, z => .ok (KoyomiModel.closeEndOfMonth z)
  | .roll c, z => KoyomiModel.roll cal z c
  | .ifClosed inner, z =>
    match cal z with
    | .error e => .error e
    | .ok true => .ok z
    | .ok false => inner.apply cal vals z

def applyAll (cal : Cal) (vals : Array Int) : List Op → Int → Except Fail Int
  | [], z => .ok z
  | o :: os, z =>
    match o.apply cal vals z with
    | .error e => .error e
    | .ok w => applyAll cal vals os w

/-- Where a date starts: the date input, or a date computed before it. -/
inductive Start where
  | input
  | date (k : Nat)
  deriving Repr

structure DateDecl where
  start : Start
  ops : List Op
  /-- `at`, in minutes from the day's start (`at end of day` is 1440). -/
  at? : Option Int
  deriving Repr

structure Input where
  lo : Int
  hi : Int
  /-- Whether a date takes it: the generated code guards only those. -/
  taken : Bool
  deriving Repr

structure DatesFile where
  inputs : Array Input
  dateInput : Nat
  dates : Array DateDecl
  /-- Every date, each after the one it starts from. -/
  order : List Nat
  calendar : Option Rules
  deriving Repr

def DatesFile.cal (f : DatesFile) : Cal :=
  match f.calendar with
  | some c => c.isOpen
  | none => fun _ => .error (.bug "an operation asks a calendar the file does not have")

/-- Every date of one input, by index, in the order the file computes them. -/
def DatesFile.run (f : DatesFile) (vals : Array Int) : Except Fail (Array Int) :=
  f.order.foldlM (fun out k => do
    let some d := f.dates[k]? | .error (.bug "no such date")
    let z0 := match d.start with
      | .input => vals[f.dateInput]!
      | .date s => out[s]!
    let z ← applyAll f.cal vals d.ops z0
    pure (out.set! k z)) (Array.replicate f.dates.size 0)

def two (n : Int) : String := pad n.toNat 2

/-- A day's time at `at`, as RFC 3339 in UTC (`interp.rs`'s `time`): both the local and the UTC
    day have to be dates. -/
def timeUtc (z : Int) (atMin offset : Int) : Except Fail String :=
  let loc := z * 1440 + atMin
  let utc := loc - offset
  let dayOf (mins : Int) : Except Fail (Int × Int) :=
    let day := mins.ediv 1440
    if day < minDay ∨ day > maxDay then .error .outOfRange else .ok (day, mins.emod 1440)
  match dayOf loc, dayOf utc with
  | .ok _, .ok (ud, um) => .ok s!"{showDay ud}T{two (um.ediv 60)}:{two (um.emod 60)}:00Z"
  | .error e, _ => .error e
  | _, .error e => .error e

/-- What one input gives, as the vectors write it: every date in the order the file declares
    them, each followed by its time when it has `at` and the calendar an offset; or the kind of
    error. An input outside its range, of an input some date takes, is `range`. -/
def DatesFile.expect (f : DatesFile) (vals : Array Int) : Except String (List String) :=
  if (f.inputs.toList.zip vals.toList).any (fun (i, v) => i.taken && (v < i.lo || v > i.hi)) then .error "range"
  else match f.run vals with
    | .error e => .error e.kind
    | .ok out =>
      let offset := f.calendar.bind (·.offset)
      (f.dates.toList.zip out.toList).foldlM (fun acc (d, z) =>
        match d.at?, offset with
        | some m, some off =>
          match timeUtc z m off with
          | .ok t => .ok (acc ++ [showDay z, t])
          | .error e => .error e.kind
        | _, _ => .ok (acc ++ [showDay z])) []

end KoyomiModel
