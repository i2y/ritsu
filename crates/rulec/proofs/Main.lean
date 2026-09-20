/-
  `rulec-recheck`: holds a certificate to its claims, and says which of them it proved.

  Every test this program runs is a function from `RulecCert.Check`, `RulecCert.Sieve` or
  `RulecCert.Values`, and every one of those has a theorem in `RulecCert.Sound` saying what
  a `true` from it settles. So the program is not a second opinion about the same rule: it
  is the proofs, run. What it cannot settle it says out loud — a pair W114 left undecided,
  a leaf resting on a table above, a value whose interval this arithmetic gives up on —
  because a checker that printed "ok" over those would be worth less than no checker.
-/
import Lean
import RulecCert

open Lean RulecCert

/-- Everything the run has to say, and whether anything failed. -/
structure Report where
  lines : Array String := #[]
  bad : Array String := #[]

def Report.say (r : Report) (s : String) : Report := { r with lines := r.lines.push s }
def Report.fail (r : Report) (s : String) : Report := { r with bad := r.bad.push s }

def declaresAs (want got : Ty) : Bool :=
  (unify want got).isSome ||
    (want == Ty.rate && got == Ty.number) || (want == Ty.number && got == Ty.rate)

def ratText (q : Rat) : String := toString q

/-- The units and int64 claims, over the `values` section. -/
def checkValues (cert : Json) (r : Report) : Report × (String → Option Span2) := Id.run do
  let typesJson := (field cert "types").getD Json.null
  let types : String → Option Ty := fun n =>
    (objPairs typesJson).find? (fun p => p.1 == n) |>.bind (fun p => str p.2 >>= tyOfString)
  let declared : String → Option Span2 := fun n =>
    match (objPairs ((field cert "ranges").getD Json.null)).find? (fun p => p.1 == n) with
    | some (_, v) => do
        let a ← arr v
        let lo ← a[0]? >>= optRat
        let hi ← a[1]? >>= optRat
        some (lo, hi)
    | none => none
  let vals := fieldArr cert "values"
  -- A derived value's own interval is what the axis it feeds can reach, so the values are
  -- read before the tables and the two are the same number.
  let mut got : List (String × Span2) := []
  let mut r := r
  let mut typed := 0
  let mut held := 0
  let mut skipped := 0
  for v in vals do
    let name := fieldStr v "name"
    let some e := (field v "expr" >>= exprOfJson)
      | r := r.fail s!"{name}: an expression this program cannot read"; continue
    -- units (E103)
    match typeOf types e with
    | none => r := r.fail s!"{name}: the units do not meet in its own expression"
    | some τ =>
      match (field v "type" >>= str >>= tyOfString) with
      | none => r := r.fail s!"{name}: the type it declares cannot be read"
      | some want =>
        if declaresAs want τ then typed := typed + 1
        else r := r.fail s!"{name}: the expression gives a different type from the declared one"
    -- the interval, and int64 (E108)
    let ranges : String → Option Span2 := fun n =>
      match declared n with
      | some I => some I
      | none => (got.find? (fun p => p.1 == n)).map (fun p => p.2)
    match interval ranges e with
    | none => skipped := skipped + 1
    | some I =>
      got := got ++ [(name, I)]
      let stated : Option Span2 := do
        let a ← field v "interval" >>= arr
        let lo ← a[0]? >>= optRat
        let hi ← a[1]? >>= optRat
        some (lo, hi)
      match stated with
      | none => r := r.fail s!"{name}: the interval it states cannot be read"
      | some S =>
        if S.1 ≤ I.1 && I.2 ≤ S.2 then
          let scale := ((field v "scale" >>= optRat)).getD 1
          let smax := ((field v "stored_max" >>= optRat)).getD 0
          if storedFits I scale smax then held := held + 1
          else r := r.fail s!"{name}: stored at that scale it does not fit int64"
        else
          r := r.fail s!"{name}: the interval here is wider than the one it states"
  if vals.size > 0 then
    r := r.say s!"  values: {typed} typed, {held} held to int64, {skipped} not re-checked"
  let reachOf : String → Option Span2 := fun n => (got.find? (fun p => p.1 == n)).map (·.2)
  return (r, reachOf)

/-- **The box each row states is the box its own cells describe** (§6.2). A box widened
    without touching the cell it was read from is caught here and nowhere else, and
    `RulecCert.mem_boxOf_cmp_iff` is why reading it off the coordinates says what the cell
    says — provided no value a cell compares against falls inside a coordinate, which
    `axisSplits` checks. -/
def checkBoxes (t : ReadTable) (r : Report) : Report := Id.run do
  let mut r := r
  let mut read := 0
  if t.rowsRaw.length != t.cert.rows.length then
    -- A row whose cells this program cannot read would otherwise vanish from every check
    -- below, which is the one way a reader could be shown fewer claims than were made.
    r := r.fail s!"{t.name}: {t.cert.rows.length - t.rowsRaw.length} rows state a cell this program cannot read"
  for row in t.rowsRaw do
    if row.tests.length != t.columns.length || row.accepts.length != t.columns.length then
      r := r.fail s!"{t.name}: row {row.index} does not state one cell per column"
    else
      for ai in [0 : t.columns.length] do
        let cell := row.tests[ai]!
        let labels := t.labels[ai]!
        let coords := t.spans[ai]!
        if !axisSplits coords cell then
          r := r.fail s!"{t.name}: row {row.index}, {t.columns[ai]!}: the cell compares against a value that falls inside a coordinate, so the axis does not stand for it"
        else if boxOf labels coords cell != row.accepts[ai]! then
          r := r.fail s!"{t.name}: row {row.index}, {t.columns[ai]!}: the box it states is not the one its cell describes"
        else
          read := read + 1
  if read > 0 then r := r.say s!"    {read} boxes read back from the cells they were written as"
  return r

/-- **The certificate quotes the file.** Every cell is read back out of the `.rule` text at
    the byte span the certificate names. -/
def checkSpans (t : ReadTable) (lines : Array ByteArray) (r : Report) : Report × Nat × Nat :=
  Id.run do
  let mut r := r
  let mut read := 0
  let mut apart := 0
  for row in t.rowsRaw do
    match row.source with
    | none => apart := apart + 1
    | some spans =>
      if spans.length != t.columns.length then
        r := r.fail s!"{t.name}: row {row.index} names {spans.length} cells, the table has {t.columns.length} columns"
      else
        for ai in [0 : t.columns.length] do
          match spans[ai]! with
          | none =>
            if !(row.tests[ai]! matches CellTest.any) then
              r := r.fail s!"{t.name}: row {row.index}, {t.columns[ai]!}: nothing is written there, and the certificate reads a cell"
          | some sp =>
            let line := if 0 < sp.line && sp.line ≤ lines.size then lines[sp.line - 1]! else .empty
            let got := if sp.col + sp.len ≤ line.size then
                String.fromUTF8? (line.extract sp.col (sp.col + sp.len)) else none
            if got != some sp.text then
              r := r.fail s!"{t.name}: row {row.index}, {t.columns[ai]!}: the file does not say `{sp.text}` there"
            else
              read := read + 1
  return (r, read, apart)

def checkTable (t : ReadTable) (r : Report) : Report := Id.run do
  let mut r := r
  let C := t.cert
  let claimed := C.rows.filter (fun row => !t.unused.contains row.index)
  let mut notes : Array String := #[]
  -- E101
  if t.upstream then
    notes := notes.push "cover rests on a table above (not re-checked)"
  else if C.coverChecks then
    notes := notes.push "complete"
  else
    r := r.fail s!"{t.name}: the cover does not tile the space"
  -- E102
  if C.reachChecks claimed then
    notes := notes.push s!"{claimed.length} rows reached"
  else if C.reachWeakChecks claimed then
    notes := notes.push s!"{claimed.length} rows reached (the sieve does not exclude the point; the values behind it are not shown)"
  else
    r := r.fail s!"{t.name}: a row has no point, or the point it has is one the sieve excludes"
  if t.unused.length > 0 then
    notes := notes.push s!"{t.unused.length} unused (not re-checked)"
  -- E105
  if C.policy == Policy.unique then
    if C.pairChecks then
      let undec := (C.rows.flatMap (fun a => C.rows.filterMap (fun b =>
        if C.undecided a.index b.index then some (a.index, b.index) else none)))
      if undec.isEmpty then notes := notes.push "no two rows meet"
      else notes := notes.push s!"{undec.length} pairs undecided (not re-checked)"
    else
      r := r.fail s!"{t.name}: two rows are neither proved apart nor named as undecided"
  r := r.say s!"  {t.name}: {C.rows.length} rows — {", ".intercalate notes.toList}"
  r := checkBoxes t r
  return r

/-- The file a certificate is about, split into lines the way a span counts them. -/
def readLines (bs : ByteArray) : Array ByteArray := Id.run do
  let mut out : Array ByteArray := #[]
  let mut start := 0
  for i in [0 : bs.size] do
    if bs[i]! == 10 then
      out := out.push (bs.extract start i)
      start := i + 1
  return out.push (bs.extract start bs.size)

def run (text : String) (rule : Option (String × ByteArray)) : IO UInt32 := do
  match Json.parse text with
  | .error e => IO.println s!"FAILED: the certificate is not JSON: {e}"; return 1
  | .ok cert =>
    let mut r : Report := {}
    r := r.say s!"{fieldStr cert "rule"} ({fieldStr cert "rulec"}), re-checked against the Lean proofs"
    let (r', reachOf) := checkValues cert r
    r := r'
    let groupsJson := (field cert "groups").getD Json.null
    let groups : String → List String := fun n =>
      match (objPairs groupsJson).find? (fun p => p.1 == n) with
      | some (_, v) => ((arr v).getD #[]).toList.filterMap str
      | none => []
    let mut tables : Array ReadTable := #[]
    for tj in fieldArr cert "tables" do
      match readTable reachOf groups tj with
      | none => r := r.fail s!"{fieldStr tj "table"}: this program cannot read the table"
      | some t => r := checkTable t r; tables := tables.push t
    match rule with
    | none => r := r.say "  (no `--rule`: the certificate is not held to any text)"
    | some (path, bs) =>
      if Sha256.hex bs != fieldStr cert "source_sha256" then
        r := r.fail s!"the certificate is about another text than {path}"
      else
        let lines := readLines bs
        let mut read := 0
        let mut apart := 0
        for t in tables do
          let (r', n, a) := checkSpans t lines r
          r := r'; read := read + n; apart := apart + a
        let rest := if apart > 0 then s!", {apart} rows written in an applied rule" else ""
        r := r.say s!"  the digest is {path}'s, and {read} cells are read back out of it{rest}"
    for l in r.lines do IO.println l
    if r.bad.isEmpty then
      IO.println "OK: every claim this program states was proved, by the theorems in RulecCert.Sound."
      return 0
    else
      for b in r.bad do IO.println s!"FAILED: {b}"
      return 1

def main (args : List String) : IO UInt32 := do
  let rulePath := match args.dropWhile (fun a => a != "--rule") with
    | _ :: p :: _ => some p
    | _ => none
  let rest := match args.dropWhile (fun a => a != "--rule") with
    | _ :: _ :: tl => args.takeWhile (fun a => a != "--rule") ++ tl
    | _ => args
  let text ← match rest with
    | [] => do let s ← IO.getStdin; s.readToEnd
    | f :: _ => IO.FS.readFile f
  let rule ← match rulePath with
    | none => pure none
    | some p => do let bs ← IO.FS.readBinFile p; pure (some (p, bs))
  run text rule
