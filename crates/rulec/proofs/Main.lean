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
  /-- Things the certificate states and this program does not prove. A run with any of
      these is not a clean pass, and the closing line has to say so rather than let a
      reader take "OK" for "everything was checked". -/
  stated : Array String := #[]

def Report.say (r : Report) (s : String) : Report := { r with lines := r.lines.push s }
def Report.fail (r : Report) (s : String) : Report := { r with bad := r.bad.push s }
def Report.state (r : Report) (s : String) : Report := { r with stated := r.stated.push s }

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
  let mut got : List (String × Span2) := []
  let mut r := r
  let mut typed := 0
  let mut held := 0
  let mut unread := 0
  let mut noInterval := 0
  for v in vals do
    let name := fieldStr v "name"
    let some e := (field v "expr" >>= exprOfJson)
      | r := r.fail s!"{name}: an expression this program cannot read"; continue
    let want := field v "type" >>= str >>= tyOfString
    -- units (E103)
    if e.unreadable then
      r := r.state s!"{name}'s units rest on a leaf this program cannot type"
      unread := unread + 1
    else
      match typeOf types e with
      | none => r := r.fail s!"{name}: the units do not meet in its own expression"
      | some τ =>
        match want with
        | none => r := r.fail s!"{name}: the type it declares cannot be read"
        | some w =>
          if declaresAs w τ then typed := typed + 1
          else r := r.fail s!"{name}: the expression gives a different type from the declared one"
    -- What bounds a name: the interval this program computed for it where there is one,
    -- and the declared range otherwise. Taking the declared range in preference would let
    -- a forged `ranges` entry override the arithmetic — the theorem needs the bound that
    -- really holds of the value, and for a computed value that is the computed one.
    let ranges : String → Option Span2 := fun n =>
      match (got.find? (fun p => p.1 == n)).map (fun p => p.2) with
      | some I => some I
      | none => declared n
    let stated : Option Span2 := do
      let a ← field v "interval" >>= arr
      let lo ← a[0]? >>= optRat
      let hi ← a[1]? >>= optRat
      some (lo, hi)
    match interval ranges e with
    | none =>
      -- A value with no interval of its own. `check` makes an int64 claim about every
      -- value that is stored as an integer, so a numeric one that states none is refused;
      -- a truth value or an enum states none because there is none to make.
      match stated, want with
      | none, some w =>
        if w.numeric then r := r.fail s!"{name}: it states no interval, and a value of its type is stored as an integer"
        else noInterval := noInterval + 1
      | none, none => noInterval := noInterval + 1
      | some _, _ => r := r.fail s!"{name}: it states an interval this program cannot derive"
    | some I =>
      -- The declared range has to contain what the expression reaches (E112); without
      -- that the value could leave the range the generated code guards on.
      match declared name with
      | some D =>
        if !(D.1 ≤ I.1 && I.2 ≤ D.2) then
          r := r.fail s!"{name}: the range the rule declares is narrower than what its expression reaches"
      | none => pure ()
      got := got ++ [(name, I)]
      match stated with
      | none => r := r.fail s!"{name}: it states no interval, and this program derives one"
      | some S =>
        if S.1 ≤ I.1 && I.2 ≤ S.2 then
          let scale := ((field v "scale" >>= optRat)).getD 1
          let smax := ((field v "stored_max" >>= optRat)).getD 0
          if storedFits I scale smax then held := held + 1
          else r := r.fail s!"{name}: stored at that scale it does not fit int64"
        else
          r := r.fail s!"{name}: the interval here is wider than the one it states"
  if vals.size > 0 then
    let notes := (if unread > 0 then s!", {unread} whose units rest on a leaf this program cannot type" else "")
      ++ (if noInterval > 0 then s!", {noInterval} with no interval to claim" else "")
    r := r.say s!"  values: {typed} typed, {held} held to int64{notes}"
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
        let pres := t.prefixes[ai]!
        -- A string column: the box is read off the prefixes, and every prefix the cell
        -- names has to be one of the axis's own (§15.101).
        if let CellTest.prefixOf ws := cell then
          if !axisCovers pres ws then
            r := r.fail s!"{t.name}: row {row.index}, {t.columns[ai]!}: the cell names a prefix the axis has no coordinate for"
          else if boxOfPrefix pres ws != row.accepts[ai]! then
            r := r.fail s!"{t.name}: row {row.index}, {t.columns[ai]!}: the box it states is not the one its cell describes"
          else
            read := read + 1
        else if !axisSplits coords cell then
          r := r.fail s!"{t.name}: row {row.index}, {t.columns[ai]!}: the cell compares against a value that falls inside a coordinate, so the axis does not stand for it"
        else if boxOf labels coords cell != row.accepts[ai]! then
          r := r.fail s!"{t.name}: row {row.index}, {t.columns[ai]!}: the box it states is not the one its cell describes"
        else
          read := read + 1
  if read > 0 then r := r.say s!"    {read} boxes read back from the cells they were written as"
  return r

/-- **A numeric axis tiles.** Its coordinates run from the declared range's low end to its
    high end, each touching the next or one grid step past it, with nothing between —
    §6.2's construction, checked rather than assumed. Without this a coordinate could be
    quietly removed and the gap under it covered by nothing. -/
def checkAxes (t : ReadTable) (r : Report) : Report × Nat := Id.run do
  let mut r := r
  let mut tiled := 0
  for ai in [0 : t.columns.length] do
    let bs := t.spans[ai]!
    if bs.isEmpty then
      r := r.fail s!"{t.name}: {t.columns[ai]!} has no coordinates, so it stands for nothing"
    else if bs.all (·.isNone) then
      pure ()  -- an enum or a flag: no grid, and nothing to tile
    else
      match t.steps[ai]! with
      | none => r := r.fail s!"{t.name}: {t.columns[ai]!} has coordinates and no grid to read them on"
      | some step =>
        if step ≤ 0 then
          r := r.fail s!"{t.name}: {t.columns[ai]!} has a grid that does not move"
        else
          let mut ok := true
          for c in [0 : bs.length] do
            match bs[c]! with
            | none =>
              r := r.fail s!"{t.name}: {t.columns[ai]!} mixes coordinates that stand for a number with ones that do not"
              ok := false
            | some x =>
              let (lo, hi) := x.span
              if (lo.isNone && c > 0) || (hi.isNone && c + 1 < bs.length) then
                r := r.fail s!"{t.name}: {t.columns[ai]!} has an open end away from its ends"
                ok := false
          if ok then
            for c in [0 : bs.length - 1] do
              match bs[c]!, bs[c + 1]! with
              | some x, some y =>
                match x.span.2, y.span.1 with
                | some hi, some lo =>
                  if !(lo == hi || lo == hi + step) then
                    r := r.fail s!"{t.name}: {t.columns[ai]!} has a gap between two coordinates"
                | _, _ => pure ()
              | _, _ => pure ()
            if t.kinds[ai]! == "input" then
              match t.declared[ai]!, bs[0]!, bs[bs.length - 1]! with
              | some d, some first, some last =>
                if first.span.1 != some d.1 || last.span.2 != some d.2 then
                  r := r.fail s!"{t.name}: {t.columns[ai]!} does not run the whole of the range the rule declares"
              | _, _, _ => pure ()
            tiled := tiled + 1
  return (r, tiled)

/-- The byte ranges a table row's `|` separators cut the line into, each trimmed. What
    follows the last `|` is a comment, not a cell. -/
def fieldsOf (line : ByteArray) : Array (Nat × Nat) := Id.run do
  let mut bars : Array Nat := #[]
  for i in [0 : line.size] do
    if line[i]! == 124 then bars := bars.push i
  let mut out : Array (Nat × Nat) := #[]
  for k in [0 : bars.size - 1] do
    let mut a := bars[k]! + 1
    let mut b := bars[k + 1]!
    while a < b && (line[a]! == 32 || line[a]! == 9) do a := a + 1
    while b > a && (line[b - 1]! == 32 || line[b - 1]! == 9) do b := b - 1
    out := out.push (a, b)
  return out

/-- **The certificate quotes the file.** Every cell is read back out of the `.rule` text at
    the byte span the certificate names — and the span has to be that cell's place, not
    somewhere else on the page that happens to read the same. A row is one line; its cells
    are that line's own `|`-separated fields, all of them; the rows of one table are
    written one under another with no other table's rows between them; and a column no row
    is written with is a column this table does not have. -/
def checkSpans (t : ReadTable) (lines : Array ByteArray) (r : Report) : Report × Nat × Nat :=
  Id.run do
  let mut r := r
  let mut read := 0
  let mut apart := 0
  let mut shape : List (String × List Bool) := []
  let mut written : List (String × Bool) := []
  let mut lineOf : List (String × (Nat × Nat)) := []
  let mut prevLine : List (String × Nat) := []
  let mut seenAxis : Array Bool := Array.replicate t.columns.length false
  for row in t.rowsRaw do
    let o := row.origin
    let has := row.source.isSome
    match written.lookup o with
    | some w => if w != has then
        r := r.fail s!"{t.name}: some rows of `{o}` are written in this file and some are not, which no one table is"
    | none => written := written ++ [(o, has)]
    match row.source with
    | none =>
      if row.line != 0 then
        r := r.fail s!"{t.name}: row {row.index} names a line in this file and no cells"
      apart := apart + 1
    | some spans =>
      if spans.length != t.columns.length then
        r := r.fail s!"{t.name}: row {row.index} names {spans.length} cells, the table has {t.columns.length} columns"
      else
        let pattern := spans.map (·.isNone)
        match shape.lookup o with
        | some p => if p != pattern then
            r := r.fail s!"{t.name}: rows of `{o}` disagree about which columns they have a cell in"
        | none => shape := shape ++ [(o, pattern)]
        let here := spans.filterMap id
        if row.line == 0 then
          r := r.fail s!"{t.name}: row {row.index} is written in this file and says no line"
        else if here.any (fun sp => sp.line != row.line) then
          r := r.fail s!"{t.name}: row {row.index} has a cell away from the line it is written on"
        else
          let ln := row.line
          match prevLine.lookup o with
          | some q => if ln ≤ q then
              r := r.fail s!"{t.name}: row {row.index} of `{o}` is written at line {ln}, at or above the row before it"
          | none => pure ()
          prevLine := (prevLine.filter (·.1 != o)) ++ [(o, ln)]
          let (lo, hi) := (lineOf.lookup o).getD (ln, ln)
          lineOf := (lineOf.filter (·.1 != o)) ++ [(o, (min lo ln, max hi ln))]
          for ai in [0 : t.columns.length] do
            if (spans[ai]!).isSome then seenAxis := seenAxis.set! ai true
          -- The cells of the row are the cells of the line, in order and with none left out.
          let line := if 0 < ln && ln ≤ lines.size then lines[ln - 1]! else .empty
          let got := (here.map (fun sp => (sp.col, sp.col + sp.len))).toArray.qsort (fun a b => a.1 < b.1)
          for k in [0 : got.size - 1] do
            if (got[k]!).2 > (got[k + 1]!).1 then
              r := r.fail s!"{t.name}: row {row.index} names two cells that overlap"
          let bars := fieldsOf line
          if here.isEmpty && bars.size > 0 then
            -- A row with no cell at all is a `clause` whose `when` names no column. A
            -- table row cannot become one by having its cells left out of the
            -- certificate: the line it sits on has cells on it.
            r := r.fail s!"{t.name}: row {row.index} is written on a line with cells on it, and the certificate names none of them"
          if bars.size > 0 && !here.isEmpty then
            if bars.extract 0 got.size != got then
              r := r.fail s!"{t.name}: row {row.index}'s cells are not the cells of the line it is written on"
            else if bars.size != got.size + t.outputs then
              r := r.fail s!"{t.name}: row {row.index} is written with {bars.size} cells, and the certificate accounts for {got.size} of them"
          for ai in [0 : t.columns.length] do
            match spans[ai]! with
            | none =>
              if !(row.tests[ai]! matches CellTest.any) then
                r := r.fail s!"{t.name}: row {row.index}, {t.columns[ai]!}: nothing is written there, and the certificate reads a cell"
            | some sp =>
              if sp.len == 0 || sp.text.isEmpty then
                r := r.fail s!"{t.name}: row {row.index}, {t.columns[ai]!}: an empty cell"
              else
                let out := if sp.col + sp.len ≤ line.size then
                    String.fromUTF8? (line.extract sp.col (sp.col + sp.len)) else none
                if out != some sp.text then
                  r := r.fail s!"{t.name}: row {row.index}, {t.columns[ai]!}: the file does not say `{sp.text}` there"
                else
                  read := read + 1
  -- No two tables of a merged set are written among each other.
  for (a, (alo, ahi)) in lineOf do
    for (b, (blo, bhi)) in lineOf do
      if a < b && alo ≤ bhi && blo ≤ ahi then
        r := r.fail s!"{t.name}: rows of `{a}` and `{b}` are written among each other, which no two tables are"
  if !lineOf.isEmpty then
    for ai in [0 : t.columns.length] do
      if !seenAxis[ai]! then
        r := r.fail s!"{t.name}: no row is written with a cell in {t.columns[ai]!}"
  -- A row called unused is one an `apply` brought in, not one written here.
  for u in t.unused do
    match (t.rowsRaw.find? (fun row => row.index == u)).map (·.source) with
    | some (some _) =>
      r := r.fail s!"{t.name}: row {u} is called unused, and it is written in this file rather than brought in by an `apply`"
    | _ => pure ()
  return (r, read, apart)

def checkTable (t : ReadTable) (r : Report) : Report := Id.run do
  let mut r := r
  let C := t.cert
  let claimed := C.rows.filter (fun row => !t.unused.contains row.index && !t.ruledOut.contains row.index)
  let mut notes : Array String := #[]
  -- E101
  if t.upstream && !t.kinds.contains "upstream" then
    r := r.fail s!"{t.name}: the cover rests on a table above, and no column of this table comes from one"
  if t.upstream then
    r := r.state s!"{t.name}'s cover rests on a table above"
    notes := notes.push "cover rests on a table above (not re-checked)"
  else if C.coverChecks then
    notes := notes.push "complete"
  else
    r := r.fail s!"{t.name}: the cover does not tile the space"
  -- E102. The values behind the point are what makes the strong reading; where the
  -- certificate hands over none, the weak one is used and named. A certificate that does
  -- hand them over and fails is wrong, not weak.
  if t.valuesStated then
    if C.reachChecks claimed then
      notes := notes.push s!"{claimed.length} rows reached"
    else
      r := r.fail s!"{t.name}: a row has no point, or the values behind its point do not hold"
  else if C.reachWeakChecks claimed then
    r := r.state s!"{t.name}'s points come with no values behind them"
    notes := notes.push s!"{claimed.length} rows reached (the sieve does not exclude the point; the values behind it are not shown)"
  else
    r := r.fail s!"{t.name}: a row has no point, or the point it has is one the sieve excludes"
  if t.unused.length > 0 then
    r := r.state s!"{t.unused.length} rows of {t.name} an `apply` brought in and this rule leaves unused"
    notes := notes.push s!"{t.unused.length} unused (not re-checked)"
  if t.ruledOut.length > 0 then
    r := r.state s!"{t.ruledOut.length} rows of {t.name} the sieve rules out"
    notes := notes.push s!"{t.ruledOut.length} the sieve rules out (stated, not proved)"
  -- E105
  if C.policy == Policy.unique then
    if C.pairChecks then
      let undec := (C.rows.flatMap (fun a => C.rows.filterMap (fun b =>
        if C.undecided a.index b.index then some (a.index, b.index) else none)))
      if undec.isEmpty then notes := notes.push "no two rows meet"
      else
        r := r.state s!"{undec.length} pairs of {t.name} W114 could not settle"
        notes := notes.push s!"{undec.length} pairs undecided (not re-checked)"
    else
      r := r.fail s!"{t.name}: two rows are neither proved apart nor named as undecided"
  let (r', tiled) := checkAxes t r
  r := r'
  if tiled > 0 then notes := notes.push s!"{tiled} axes tiled"
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
    let declaredRange : String → Option Span2 := fun n =>
      match (objPairs ((field cert "ranges").getD Json.null)).find? (fun p => p.1 == n) with
      | some (_, v) => do
          let a ← arr v
          let lo ← a[0]? >>= optRat
          let hi ← a[1]? >>= optRat
          some (lo, hi)
      | none => none
    let groupsJson := (field cert "groups").getD Json.null
    let groups : String → List String := fun n =>
      match (objPairs groupsJson).find? (fun p => p.1 == n) with
      | some (_, v) => ((arr v).getD #[]).toList.filterMap str
      | none => []
    let mut tables : Array ReadTable := #[]
    for tj in fieldArr cert "tables" do
      match readTable reachOf groups declaredRange tj with
      | none => r := r.fail s!"{fieldStr tj "table"}: this program cannot read the table"
      | some t => r := checkTable t r; tables := tables.push t
    match rule with
    | none =>
      r := r.state "the certificate is held to no text: pass `--rule <file.rule>`"
      r := r.say "  (no `--rule`: the certificate is not held to any text)"
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
        if apart > 0 then
          r := r.state s!"{apart} rows written in an applied rule, and read back in its own certificate"
        r := r.say s!"  the digest is {path}'s, and {read} cells are read back out of it{rest}"
    for l in r.lines do IO.println l
    if r.bad.isEmpty then
      if r.stated.isEmpty then
        IO.println "OK: every claim this program states was proved, by the theorems in RulecCert.Sound."
      else
        let n := if r.stated.size == 1 then "one thing is" else s!"{r.stated.size} things are"
        IO.println s!"OK, and {n} stated rather than proved: {", ".intercalate r.stated.toList}."
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
