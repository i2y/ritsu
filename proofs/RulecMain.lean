/-
  `rulec-recheck`: holds a certificate to its claims, and says which of them it proved.

  Every test this program runs is a function from `RulecCert.Check`, `RulecCert.Sieve`,
  `RulecCert.Linear`, `RulecCert.Values` or `RulecCert.Contract`, and every one of those has
  a theorem saying what a `true` from it settles. So the program is not a second opinion
  about the same rule: it is the proofs, run. What it cannot settle it says out loud — a pair W114 left undecided,
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

/-- Add a pair unless it is already there. -/
def addLe (ps : List (String × String)) (p : String × String) : List (String × String) :=
  if ps.any (fun q => q.1 == p.1 && q.2 == p.2) then ps else ps ++ [p]

/-- Close a list of `a ≤ b` pairs under chains. Two lines saying a running total never
    passes the next one, and that one never passes the whole, together say the first never
    passes the whole — and `rulec` reads them that way, so a re-checker that did not would
    refuse an honest certificate. Fuel is the length of the list: each round lengthens the
    longest chain it can see by one, and no chain is longer than the list. -/
def closeLe : Nat → List (String × String) → List (String × String)
  | 0, ps => ps
  | n + 1, ps =>
    let more := ps.flatMap (fun p => (ps.filter (fun q => q.1 == p.2)).map (fun q => (p.1, q.2)))
    let grown := more.foldl addLe ps
    if grown.length == ps.length then ps else closeLe n grown

/-- The `a ≤ b` pairs the certificate's `constraint` lines guarantee, as names. `<` and `>`
    count too: they say more, not less. -/
def guarantees (cert : Json) : List (String × String) :=
  -- A line with a sum on a side (rulec's §15.216) says nothing of one name against another.
  let raw : List (String × String) := (fieldArr cert "constraints").toList.filterMap (fun k => do
    let l ← field k "left" >>= str
    let r ← field k "right" >>= str
    match fieldStr k "op" with
    | "<=" => some (l, r)
    | "<" => some (l, r)
    | ">=" => some (r, l)
    | ">" => some (r, l)
    | _ => none)
  closeLe raw.length raw

/-- What a row says it writes into a column of numbers where its own cell narrows it (`inside`,
    rulec's §15.213): the row, the column, the name it writes, the interval the certificate says,
    and what the row writes as this program reads it, with the cut worked out from the cell. -/
structure InsideClaim where
  table : String
  row : Nat
  column : String
  name : String
  said : Option Span2
  writes : Writes

/-- A rational pair as JSON writes it: `["0","3"]`. -/
def ratPair (j : Json) : Option (Rat × Rat) := do
  let a ← arr j
  let lo ← a[0]? >>= optRat
  let hi ← a[1]? >>= optRat
  some (lo, hi)

/-- What one row writes into a column of numbers (rulec's §15.196): the expression of its answer
    cell, and — where the certificate says the row's own cell narrows it (`inside`, §15.213) — the
    cut that cell puts on the value, worked out here from the row's `tests` on the column the row
    writes and the grid the value sits on (`1/scale`, `Cells.cellCut`), never from the certificate's
    word for it. `Except` says why a write cannot be read that way. -/
def writesOf (cert tj rj : Json) (oi : Nat) : Except String (Option (Writes × Option (String × Option Span2))) := do
  let some w := (fieldArr rj "writes")[oi]? | return none
  let some ej := field w "expr" | return none
  let some e := exprOfJson ej | return none
  match field w "inside" with
  | none => return some ({ expr := e }, none)
  | some said =>
    let row := (fieldNat rj "row").getD 0
    let some name := field ej "name" >>= str
      | throw s!"{fieldStr tj "table"}: row {row} says it writes only inside a range, and what it writes is no name"
    let axes := fieldArr tj "axes"
    let some ai := axes.toList.findIdx? (fun a => fieldStr a "column" == name)
      | throw s!"{fieldStr tj "table"}: row {row} says its cell on {name} narrows what it writes, and {name} is no column of its table"
    let some cell := ((fieldArr rj "tests")[ai]?).bind (fun c => cellOfJson (fun _ => []) c)
      | throw s!"{fieldStr tj "table"}: row {row}: the cell on {name} cannot be read"
    let some sc := field ((field cert "scales").getD Json.null) name >>= nat
      | throw s!"{fieldStr tj "table"}: row {row}: no scale says what grid {name} sits on"
    if sc == 0 then throw s!"{fieldStr tj "table"}: row {row}: {name} is stored at a scale of 0"
    let some cut := cellCut (1 / (sc : Rat)) cell
      | throw s!"{fieldStr tj "table"}: row {row}: the cell on {name} says nothing an interval can"
    return some ({ expr := e, cut := some cut }, some (name, ratPair said))

/-- The columns a table writes numbers into, each with what every row of the table that decides it
    writes there (rulec's §15.196): `none` where a row writes something this program cannot read.
    And the columns whose rows state nothing they write — a certificate from before they did. And
    the rows that say their own cell narrows what they write (`inside`, §15.213), each with the cut
    worked out from its cell: `RulecCert.cut_of_box` is why the value a row writes where it fires is
    inside that cut, and `eval_mem_hull` why the column's values are then inside the hull. -/
def writtenCols (cert : Json) (types : String → Option Ty) :
    List (String × Option (List Writes)) × List String × List InsideClaim × List String := Id.run do
  let mut outs : List (String × Option (List Writes)) := []
  let mut unstated : List String := []
  let mut insides : List InsideClaim := []
  let mut bad : List String := []
  for t in fieldArr cert "tables" do
    let rows := fieldArr t "rows"
    let ds := (fieldArr t "decides").toList.filterMap str
    for (col, oi) in ds.zip (List.range ds.length) do
      let some ty := types col | continue
      if !ty.numeric then continue
      if !rows.any (fun r => (field r "writes").isSome) then
        if !unstated.contains col then unstated := unstated ++ [col]
        continue
      let mut ws : Option (List Writes) := some []
      for rj in rows do
        match writesOf cert t rj oi with
        | .error why => bad := bad ++ [why]; ws := none
        | .ok none => ws := none
        | .ok (some (w, said)) =>
          ws := ws.map (· ++ [w])
          if let some (name, st) := said then
            let claim : InsideClaim :=
              { table := fieldStr t "table", row := (fieldNat rj "row").getD 0, column := col,
                name := name, said := st, writes := w }
            insides := insides ++ [claim]
      outs := outs ++ [(col, ws)]
  return (outs, unstated, insides, bad)

/-- What `checkValues` works out: each value's interval, the hull of each column a table writes
    numbers into, and what bounds a name — the one of those that applies, and otherwise the range
    the rule declares, never the stated range of a column a table writes. -/
structure Worked where
  reachOf : String → Option Span2
  writtenOf : String → Option Span2
  rangesOf : String → Option Span2
  within : RulecCert.Expr → RulecCert.Expr → Bool

/-- The units and int64 claims, over the `values` section, and the ranges of the columns tables
    write numbers into (rulec's §15.196). -/
def checkValues (cert : Json) (r : Report) : Report × Worked := Id.run do
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
  -- `within a b` says the caller guarantees `a ≤ b`. Only names can be related this way:
  -- a `constraint` line names two inputs (§15.55).
  let le := guarantees cert
  let within : RulecCert.Expr → RulecCert.Expr → Bool := fun a b =>
    match a, b with
    | .name x, .name y => le.any (fun p => p.1 == x && p.2 == y)
    | _, _ => false
  let vals := fieldArr cert "values"
  let (outs, unstated, insides, badWrites) := writtenCols cert types
  let isOut : String → Bool := fun n => outs.any (fun o => o.1 == n)
  -- What bounds a name: the interval this program computed for it where there is one, and the
  -- declared range otherwise. Taking the declared range in preference would let a forged
  -- `ranges` entry override the arithmetic — the theorem needs the bound that really holds of
  -- the value, and for a computed value that is the computed one. A column a table writes
  -- numbers into is bounded by the hull of what its rows write, never by the stated range.
  let mut got : List (String × Span2) := []
  let mut wrote : List (String × Span2) := []
  -- What depends on what runs both ways — a `define` over a table's column, a table writing a
  -- `define` — so the two are worked out together until nothing more can be.
  for _ in [0 : vals.size + outs.length + 1] do
    let mut moved := false
    for v in vals do
      let name := fieldStr v "name"
      if got.any (fun p => p.1 == name) then continue
      let some e := (field v "expr" >>= exprOfJson) | continue
      let gotNow := got
      let wroteNow := wrote
      let ranges : String → Option Span2 := fun n =>
        match (gotNow.find? (fun p => p.1 == n)).map (fun p => p.2) with
        | some I => some I
        | none => match (wroteNow.find? (fun p => p.1 == n)).map (fun p => p.2) with
          | some I => some I
          | none => if isOut n then none else declared n
      match interval ranges within e with
      | some I => got := got ++ [(name, I)]; moved := true
      | none => pure ()
    for (col, es) in outs do
      if wrote.any (fun p => p.1 == col) then continue
      let some es := es | continue
      let gotNow := got
      let wroteNow := wrote
      let ranges : String → Option Span2 := fun n =>
        match (gotNow.find? (fun p => p.1 == n)).map (fun p => p.2) with
        | some I => some I
        | none => match (wroteNow.find? (fun p => p.1 == n)).map (fun p => p.2) with
          | some I => some I
          | none => if isOut n then none else declared n
      match hullOf ranges within es with
      | some H => wrote := wrote ++ [(col, H)]; moved := true
      | none => pure ()
    if !moved then break
  let gotF := got
  let wroteF := wrote
  let rangesF : String → Option Span2 := fun n =>
    match (gotF.find? (fun p => p.1 == n)).map (fun p => p.2) with
    | some I => some I
    | none => match (wroteF.find? (fun p => p.1 == n)).map (fun p => p.2) with
      | some I => some I
      | none => if isOut n then none else declared n
  let mut r := r
  for why in badWrites do
    r := r.fail why
  -- What a row says it writes where its own cell narrows it (§15.213) holds what this program works
  -- out from the cell: a row that claimed less would be the certificate narrowing the column past
  -- what the cell lets through.
  let mut cut := 0
  for c in insides do
    match c.writes.interval rangesF within, c.said with
    | some I, some S =>
      if S.1 ≤ I.1 && I.2 ≤ S.2 then cut := cut + 1
      else r := r.fail s!"{c.table}: row {c.row}'s cell lets {c.name} take [{ratText I.1}, {ratText I.2}], and the certificate says the row writes it inside [{ratText S.1}, {ratText S.2}]"
    | none, _ =>
      r := r.fail s!"{c.table}: row {c.row} says its cell narrows what it writes into {c.column}, and this program works out no interval for {c.name}"
    | _, none =>
      r := r.fail s!"{c.table}: row {c.row}: the interval it says it writes {c.column} inside cannot be read"
  -- The range the certificate states for a column a table writes numbers into holds every value
  -- its rows write where they fire (`eval_mem_stated`, `written_mem_hull`).
  let mut heldCols := 0
  for (col, es) in outs do
    match declared col, es with
    | some S, some es =>
      if wroteF.any (fun p => p.1 == col) then
        if writesWithin rangesF within S es then heldCols := heldCols + 1
        else r := r.fail s!"{col}: its rows write a value outside the range the certificate states for it"
      else
        r := r.fail s!"{col}: the certificate states a range for it, and this program cannot work one out from what its rows write"
    | some _, none =>
      r := r.fail s!"{col}: the certificate states a range for it, and a row writes something this program cannot read"
    | none, _ => pure ()
  if heldCols > 0 then
    let cutNote := if cut > 0 then s!", {cut} rows inside what their own cell lets through" else ""
    r := r.say s!"  ranges: {heldCols} columns a table writes numbers into hold every value their rows write{cutNote}"
  for col in unstated do
    r := r.state s!"the range of {col}: its rows state nothing they write"
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
    let stated : Option Span2 := do
      let a ← field v "interval" >>= arr
      let lo ← a[0]? >>= optRat
      let hi ← a[1]? >>= optRat
      some (lo, hi)
    match (gotF.find? (fun p => p.1 == name)).map (fun p => p.2) with
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
  let reachOf : String → Option Span2 := fun n => (gotF.find? (fun p => p.1 == n)).map (·.2)
  let writtenOf : String → Option Span2 := fun n => (wroteF.find? (fun p => p.1 == n)).map (·.2)
  return (r, { reachOf := reachOf, writtenOf := writtenOf, rangesOf := rangesF, within := within })

/-- What kind of column each name makes (rulec's §15.196): a `derive` is `derived`, a `define` of
    any type `define`, an input `input`, what a walk leaves behind `walk`; anything else — a column
    a table above decides, a field of the elements a fold walks — `upstream`. -/
def kindOf (cert : Json) (n : String) : String :=
  match (fieldArr cert "values").toList.find? (fun v => fieldStr v "name" == n) with
  | some v =>
    if fieldStr v "of" == "derive" then "derived"
    else if fieldStr v "of" == "define" then "define"
    else "upstream"
  | none =>
    if (fieldArr cert "inputs").toList.any (fun x => str x == some n) then "input"
    else if (fieldArr cert "walks").toList.any (fun x => str x == some n) then "walk"
    else "upstream"

/-- A boolean `define` of one comparison whose two sides have intervals (rulec's §15.196): the
    comparison and the two intervals — a date literal as its day number, any other side worked out
    the way every value is. -/
def truthOfValue (cert : Json) (w : Worked) (n : String) : Option (Cmp × Span2 × Span2) := do
  let v ← (fieldArr cert "values").toList.find? (fun v => fieldStr v "name" == n)
  if fieldStr v "type" != "bool" then none
  let e ← field v "expr"
  let op ← match fieldStr e "op" with
    | "<=" => some Cmp.le
    | "<" => some Cmp.lt
    | ">=" => some Cmp.ge
    | ">" => some Cmp.gt
    | _ => none
  let side : Json → Option Span2 := fun x =>
    if (field x "lit").isSome && fieldStr x "type" == "date" then do
      let d ← field x "value" >>= optRat
      some (d, d)
    else do
      let ex ← exprOfJson x
      interval w.rangesOf w.within ex
  let L ← field e "l" >>= side
  let R ← field e "r" >>= side
  some (op, L, R)

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
    let mut bs := t.spans[ai]!
    -- An optional input's axis starts with the absent value, which stands for no number; the
    -- values that are there tile the rest (rulec's §15.201). Without it, the cover leaves out
    -- an input the rule takes.
    if (t.optional[ai]?).getD false then
      let first := match bs[0]? with | some none => true | _ => false
      if (t.labels[ai]!)[0]? != some "none" || !first then
        r := r.fail s!"{t.name}: {t.columns[ai]!} is optional, and its axis has no coordinate for the absent value"
      bs := bs.drop 1
    if bs.isEmpty && (t.optional[ai]?).getD false then
      pure ()
    else if bs.isEmpty then
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
                -- An interval leaves its ends out, so an end of the declared range is covered
                -- only by a point on it (rulec's §15.201): an axis whose last point was taken
                -- away still reached the end with the interval before it.
                else if first.span.2 != some d.1 || last.span.1 != some d.2 then
                  r := r.fail s!"{t.name}: {t.columns[ai]!} does not end on the ends of the range the rule declares, so a value at an end is in no coordinate"
              | _, _, _ => pure ()
            tiled := tiled + 1
  return (r, tiled)

/-- The largest step two steps are both whole multiples of (rulec's §15.190): a tenth and a
    whole unit share a tenth. Zero, the step of a value that is always zero, divides nothing
    and is left out. -/
def shareStep (a b : Rat) : Rat :=
  if a == 0 then b
  else if b == 0 then a
  else ((Int.gcd (a.num * b.den) (b.num * a.den) : Nat) : Rat) / ((a.den * b.den : Nat) : Rat)

def ratAbs (a : Rat) : Rat := if a < 0 then -a else a

/-- The step every value of an expression is a multiple of, from its leaves up, as rulec works
    it out (its §15.190): a name is on its own step and a literal on its value; a sum, a
    difference, a `min` or a `max` on the step both sides share, a product on the product of
    the two, a quotient by a whole constant on its left side's over the constant; a rounding
    on its grid, a share on whole units. `none` for anything else. -/
def exprStep (stepOf : String → Option Rat) : RulecCert.Expr → Option Rat
  | .name n => stepOf n
  | .lit v _ => some (ratAbs v)
  | .add a b | .sub a b | .minOf a b | .maxOf a b => do
      let x ← exprStep stepOf a
      let y ← exprStep stepOf b
      pure (shareStep x y)
  | .mul a b => do
      let x ← exprStep stepOf a
      let y ← exprStep stepOf b
      pure (x * y)
  | .divc a k _ => do
      let x ← exprStep stepOf a
      if k.den == 1 && k > 0 then some (x / k) else none
  | .roundTo _ g => some (ratAbs g)
  | .alloc _ _ _ => some 1
  | _ => none

/-- The step every value of a column is a multiple of, from the document's own account: a
    value's expression read from its leaves up, and otherwise — and for a value whose
    expression gives no step — the scale the name is stored at (`scales`). `none` when
    neither says, which a certificate from before rulec's §15.190 does not. -/
partial def columnStep (cert : Json) (seen : List String) (name : String) : Option Rat :=
  if seen.contains name then none else
  let fromExpr : Option Rat := do
    let v ← (fieldArr cert "values").find? (fun v => fieldStr v "name" == name)
    let e ← field v "expr" >>= exprOfJson
    let g ← exprStep (columnStep cert (name :: seen)) e
    if g == 0 then none else some g
  -- What a walk answers (rulec's §15.219, §15.220): a fold's answer is the value of one of the
  -- expressions it answers with, and a `max` or a `min` a value of its column or its `empty` value,
  -- so each sits on the step those share.
  let fromWalk : Option Rat := do
    let step := columnStep cert (name :: seen)
    let g ← match (fieldArr cert "fold_answers").find? (fun fa => fieldStr fa "name" == name) with
      | some fa => do
        let es ← (fieldArr fa "answers").toList.mapM exprOfJson
        let gs ← es.mapM (exprStep step)
        some (gs.foldl shareStep 0)
      | none => do
        let x ← (fieldArr cert "extremes").find? (fun x => fieldStr x "name" == name)
        let c ← step (fieldStr x "of")
        let z ← field x "empty" >>= optRat
        some (shareStep c (ratAbs z))
    if g == 0 then none else some g
  match fromExpr, fromWalk with
  | some g, _ => some g
  | none, some g => some g
  | none, none => do
      let s ← field ((field cert "scales").getD Json.null) name >>= nat
      if s == 0 then none else some (1 / (s : Rat))

/-- **A numeric axis is cut on the step its values sit on** (rulec's §15.190). "One grid step
    past it, with nothing between" is only so on that grid: a column of tenths of a pound cut
    every whole pound has values between two coordinates, and no box covered them. The step
    the certificate gives has to divide the one the column's values are worked out to sit on,
    and every end of a coordinate has to lie on it. -/
def checkSteps (t : ReadTable) (stepOf : String → Option Rat) (r : Report) : Report := Id.run do
  let mut r := r
  for ai in [0 : t.columns.length] do
    let bs := t.spans[ai]!
    if bs.isEmpty || bs.all (·.isNone) then continue
    match t.steps[ai]! with
    | none => pure ()
    | some step =>
      if step ≤ 0 then continue
      let col := t.columns[ai]!
      match stepOf col with
      | none => r := r.state s!"the step {t.name}'s {col} is cut at, which no scale here bears out"
      | some g =>
        if (g / step).den != 1 then
          r := r.fail s!"{t.name}: {col} is cut every {ratText step}, and its values sit on a step of {ratText g}, so a value can fall between two coordinates"
        else
          let ends := bs.flatMap (fun x => match x with
            | some c => [c.span.1, c.span.2]
            | none => [])
          if ends.any (fun e => match e with
              | some v => (v / step).den != 1
              | none => false) then
            r := r.fail s!"{t.name}: {col} has a coordinate ending off its step of {ratText step}"
  return r

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

/-- The number a cell of digits stands for: `ds` read in base ten. -/
def digitsVal (ds : List Char) : Nat := ds.foldl (fun n c => n * 10 + (c.toNat - '0'.toNat)) 0

/-- The number an answer cell writes (rulec's §15.196), with the one multiplier §2.1 allows (`万`,
    `億`, `兆`) and its unit: `%` is a hundredth, no unit or the currency of a money column the
    number itself. `none` when the text is not a number; `some none` for a unit this program does
    not convert — a unit table is what it deliberately does not have. -/
def literalValue (text : String) (cur : Option String) : Option (Option Rat) :=
  let cs := ((text.toList.dropWhile Char.isWhitespace).reverse.dropWhile Char.isWhitespace).reverse
  let (neg, cs) := match cs with
    | '-' :: rest => (true, rest)
    | _ => (false, cs)
  let whole := (cs.takeWhile (fun c => c.isDigit || c == '_')).filter (· != '_')
  let rest := cs.dropWhile (fun c => c.isDigit || c == '_')
  if whole.isEmpty then none else
  let (frac, rest) := match rest with
    | '.' :: r => (r.takeWhile Char.isDigit, r.dropWhile Char.isDigit)
    | _ => ([], rest)
  let base : Rat := (digitsVal whole : Rat) + (digitsVal frac : Rat) / ((10 ^ frac.length : Nat) : Rat)
  let (mult, rest) : Rat × List Char := match rest with
    | '万' :: r => (10000, r)
    | '億' :: r => (100000000, r)
    | '兆' :: r => (1000000000000, r)
    | _ => (1, rest)
  let v := (if neg then -base else base) * mult
  let unit := ((rest.dropWhile Char.isWhitespace).reverse.dropWhile Char.isWhitespace).reverse
  if unit == ['%'] then some (some (v / 100))
  else if unit.isEmpty then some (some v)
  else match cur with
    | some c => if unit == c.toList then some (some v) else some none
    | none => some none

/-- **The certificate quotes the file.** Every cell is read back out of the `.rule` text at
    the byte span the certificate names — and the span has to be that cell's place, not
    somewhere else on the page that happens to read the same. A row is one line; its cells
    are that line's own `|`-separated fields, all of them; the rows of one table are
    written one under another with no other table's rows between them; and a column no row
    is written with is a column this table does not have. -/
def checkSpans (t : ReadTable) (lines : Array ByteArray) (r : Report)
    (curOf : String → Option String := fun _ => none) : Report × Nat × Nat :=
  Id.run do
  let mut r := r
  let mut read := 0
  let mut apart := 0
  let mut shape : List (String × List Bool) := []
  let mut written : List (String × Bool) := []
  let mut lineOf : List (String × (Nat × Nat)) := []
  let mut prevLine : List (String × Nat) := []
  let mut seenAxis : Array Bool := Array.replicate t.columns.length false
  -- A table written in one place, not a set of tables that decide the same column (§15.66): its
  -- answer cells stand in the order it decides its columns.
  let single := t.rowsRaw.all (fun x => x.origin == ((t.rowsRaw.head?.map (·.origin)).getD ""))
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
            else if single then
              -- What the row writes into a column of words is the cell in the file too (rulec's
              -- §15.233): the answer cell of that column says the word the certificate reads — what
              -- a machine moves on, what a `once` line counts, what an `ordered by` table answers,
              -- and what a table above writes.
              for (pj, j) in row.produces.zip (List.range row.produces.length) do
                if let some w := pj then
                  let txt : Option String := (bars[got.size + j]?).bind (fun ab =>
                    if ab.2 ≤ line.size then String.fromUTF8? (line.extract ab.1 ab.2) else none)
                  if txt == some w then read := read + 1
                  else r := r.fail s!"{t.name}: row {row.index}, answer {j + 1}: the file writes `{txt.getD ""}` there, and the certificate reads it as `{w}`"
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
          -- What the row writes into a column of numbers is the cell in the file (rulec's §15.196):
          -- one of the cells after the ones its tests read, on its own line — or, for a `clause`,
          -- on its `then` line — and the literal or the name the certificate says it is.
          for w in row.writes do
            match w with
            | none => pure ()
            | some wc =>
              match wc.span with
              | none =>
                r := r.fail s!"{t.name}: row {row.index}, -> {wc.column}: the row is written in this file, and no cell is named for what it writes"
              | some sp =>
                let answers := bars.extract got.size bars.size
                if bars.size > 0 && (sp.line != ln || !answers.contains (sp.col, sp.col + sp.len)) then
                  r := r.fail s!"{t.name}: row {row.index}, -> {wc.column}: the cell named is not one of the row's answer cells"
                else
                  let wline := if bars.size > 0 then line
                    else if 0 < sp.line && sp.line ≤ lines.size then lines[sp.line - 1]! else .empty
                  let out := if sp.col + sp.len ≤ wline.size then
                      String.fromUTF8? (wline.extract sp.col (sp.col + sp.len)) else none
                  if out != some sp.text || sp.text != wc.says then
                    r := r.fail s!"{t.name}: row {row.index}, -> {wc.column}: the file does not say `{wc.says}` there"
                  else if wc.isName then
                    read := read + 1
                  else
                    match literalValue sp.text (curOf wc.column) with
                    | none => r := r.fail s!"{t.name}: row {row.index}, -> {wc.column}: `{sp.text}` is not a number"
                    | some none =>
                      r := r.state s!"{t.name}: row {row.index}, -> {wc.column}: `{sp.text}` is in a unit this program does not convert"
                    | some (some x) =>
                      if wc.value == some x then read := read + 1
                      else r := r.fail s!"{t.name}: row {row.index}, -> {wc.column}: the file writes `{sp.text}`, and the certificate reads it as another number"
  -- No two tables of a merged set are written among each other.
  for (a, (alo, ahi)) in lineOf do
    for (b, (blo, bhi)) in lineOf do
      if a < b && alo ≤ bhi && blo ≤ ahi then
        r := r.fail s!"{t.name}: rows of `{a}` and `{b}` are written among each other, which no two tables are"
  if !lineOf.isEmpty then
    for ai in [0 : t.columns.length] do
      -- A column of a table an `apply` brought in has its cells in that rule's file, where its own
      -- certificate reads them back (rulec's §15.196): only a column no row has a cell in, here or
      -- there, is one no cell is written for.
      let applied := t.rowsRaw.any (fun row =>
        row.source.isNone && match row.tests[ai]? with
          | some CellTest.any => false
          | some _ => true
          | none => false)
      if !seenAxis[ai]! && !applied then
        r := r.fail s!"{t.name}: no row is written with a cell in {t.columns[ai]!}"
  -- A row called unused is one an `apply` brought in, not one written here.
  for u in t.unused do
    match (t.rowsRaw.find? (fun row => row.index == u)).map (·.source) with
    | some (some _) =>
      r := r.fail s!"{t.name}: row {u} is called unused, and it is written in this file rather than brought in by an `apply`"
    | _ => pure ()
  return (r, read, apart)

/-- A span of one column: an interval where the column is a number, the values it may take
    where it is not. -/
inductive Span where
  | num : Option Rat → Option Rat → Span
  | words : List String → Span
  deriving Inhabited

/-- Whether two spans of one column have anything in common. Two shapes that do not match
    are not compared: saying they meet is the half that declines to conclude. -/
def Span.meets : Span → Span → Bool
  | .num _ ah, .num bl _ =>
    !((match ah, bl with | some x, some y => decide (x < y) | _, _ => false) ||
      (match bl, ah with | some _, some _ => false | _, _ => false))
  | .words a, .words b => a.any (fun x => b.contains x)
  | _, _ => true

/-- Whether the first span holds all of the second. -/
def Span.holds : Span → Span → Bool
  | .num ol oh, .num il ih =>
    (match ol, il with | none, _ => true | some a, some b => decide (a ≤ b) | some _, none => false) &&
    (match oh, ih with | none, _ => true | some a, some b => decide (b ≤ a) | some _, none => false)
  | .words o, .words i => i.all (fun x => o.contains x)
  | _, _ => false

/-- One coordinate of one axis as a **closed** interval of true values. `bounds` writes an
    interval coordinate's ends, and the coordinate does not contain them: the values sit on
    the axis's grid, so `(10, 30)` on a grid of 1 is `[11, 29]`. -/
def closedAt (ax : Json) (ci : Nat) : Option Rat × Option Rat :=
  let bs := fieldArr ax "bounds"
  match bs[ci]? >>= arr with
  | none => (none, none)
  | some v =>
    let lo := v[0]? >>= optRat
    let hi := v[1]? >>= optRat
    match lo, hi with
    | some a, some c => if a == c then (lo, hi) else
      match (field ax "step") >>= optRat with
      | some st => (some (a + st), some (c - st))
      | none => (lo, hi)
    | _, _ =>
      match (field ax "step") >>= optRat with
      | some st => (lo.map (· + st), hi.map (· - st))
      | none => (lo, hi)

/-- One coordinate of one axis, as a span. -/
def spanAt (ax : Json) (ci : Nat) : Span :=
  if (fieldArr ax "bounds").any (fun b => !b.isNull) then
    let (lo, hi) := closedAt ax ci
    .num lo hi
  else .words (((fieldArr ax "coords")[ci]? >>= str).toList)

/-- The table that decides a column, and where the column sits among its outputs. -/
def decider (all : Array Json) (column : String) : Option (Json × Nat) :=
  all.findSome? (fun u =>
    let ds := (fieldArr u "decides").toList.filterMap str
    (ds.findIdx? (· == column)).map (fun i => (u, i)))

/-- Where a column another table decides can hold a value, read on one of that table's own
    columns.

    The rows that write the value, and the coordinates of `input` at which one of them can
    still fire. Under `first` a row's box is not where it fires — an earlier row may take
    all of it — so a row counts only while no single earlier row takes the whole of its box
    at that coordinate. That is loose the safe way: what drops out certainly cannot fire, so
    the span contains every input on which the column really holds the value. -/
def producedWhere (all : Array Json) (column value input : String) : Option Span := do
  let (u, oi) ← decider all column
  let axes := fieldArr u "axes"
  let ai ← axes.toList.findIdx? (fun a => fieldStr a "column" == input)
  let rows := (fieldArr u "rows").toList
  let outs ← rows.mapM (fun r => (fieldArr r "produces")[oi]? >>= str)
  let accepts : List (List (List Nat)) := rows.map (fun r =>
    (fieldArr r "accepts").toList.map (fun xs => ((arr xs).getD #[]).toList.filterMap nat))
  let first := fieldStr u "policy" == "first"
  let arity : Nat → Nat := fun a => (fieldArr ((axes[a]?).getD Json.null) "coords").size
  let nAx := axes.size
  let picked := (List.range rows.length).filter (fun i => (outs[i]?).getD "" == value)
  if picked.isEmpty then none else
  let boxOf : Nat → Nat → List Nat := fun i a => (((accepts[i]?).getD [])[a]?).getD []
  let takenEarlier : Nat → Nat → Bool := fun i x =>
    (List.range i).any (fun e =>
      (List.range nAx).all (fun a =>
        (List.range (arity a)).all (fun y =>
          !((boxOf i a).contains y && (a != ai || y == x)) || (boxOf e a).contains y)))
  let live := (List.range (arity ai)).filter (fun x =>
    picked.any (fun i => (boxOf i ai).contains x && (!first || !takenEarlier i x)))
  if live.isEmpty then none else
  let ax := (axes[ai]?).getD Json.null
  if (fieldArr ax "bounds").any (fun b => !b.isNull) then
    let ends := live.map (closedAt ax)
    let lo := if ends.any (fun e => e.1.isNone) then none
              else ends.foldl (fun acc e => match acc, e.1 with
                | none, v => v
                | some a, some b => some (if b < a then b else a)
                | some a, none => some a) none
    let hi := if ends.any (fun e => e.2.isNone) then none
              else ends.foldl (fun acc e => match acc, e.2 with
                | none, v => v
                | some a, some b => some (if a < b then b else a)
                | some a, none => some a) none
    some (.num lo hi)
  else some (.words (live.filterMap (fun x => (fieldArr ax "coords")[x]? >>= str)))

/-- A span as the certificate writes it, read against the shape of the one it is held to. -/
def statedSpan (j : Json) (like : Span) : Option Span :=
  match arr j, like with
  | some v, .num _ _ => if v.size == 2 then some (.num (v[0]? >>= optRat) (v[1]? >>= optRat)) else none
  | some v, .words _ => some (.words (v.toList.filterMap str))
  | none, _ => none

/-- **Every fact this table rests on, earned back from the tables that decide its columns.**
    A `never` fact is settled by reading those rows; an `apart` fact by recomputing both
    spans and finding they do not meet. The spans the certificate wrote have to contain the
    ones recomputed, so it cannot write a narrower span than the truth and call apart two
    things that meet. The verdict rests on the recomputed pair (§15.115). -/
def checkAbove (tj : Json) (all : Array Json) (r : Report) : Report := Id.run do
  let mut r := r
  let name := fieldStr tj "table"
  let axes := fieldArr tj "axes"
  let ab := (field tj "above").getD Json.null
  let named : Json → Option (String × String) := fun q => do
    let ai ← fieldNat q "axis"; let ci ← fieldNat q "coord"
    let ax ← axes[ai]?
    let v ← (fieldArr ax "coords")[ci]? >>= str
    some (fieldStr ax "column", v)
  for f in fieldArr ab "never" do
    match named f with
    | none => r := r.fail s!"{name}: a fact points at a coordinate that does not exist"
    | some (col, val) =>
      match decider all col with
      | none => r := r.fail s!"{name}: a fact says {col} is never {val}, and no table here decides {col}"
      | some (u, oi) =>
        for row in fieldArr u "rows" do
          match (fieldArr row "produces")[oi]? >>= str with
          | none => r := r.fail s!"{name}: a row of {fieldStr u "table"} writes something this program cannot read, so «{col} is never {val}» is not settled"
          | some w => if w == val then
              r := r.fail s!"{name}: {fieldStr u "table"} does write {col} = {val}, so it is not a value the tables above never reach"
  for f in fieldArr ab "apart" do
    let input := fieldStr f "input"
    let stated := fieldArr f "spans"
    let ends := #[(field f "a").getD Json.null, (field f "b").getD Json.null]
    let mut got : Array Span := #[]
    let mut ok := true
    for i in [0, 1] do
      match named (ends[i]!) with
      | none => r := r.fail s!"{name}: a fact points at a coordinate that does not exist"; ok := false
      | some (col, val) =>
        let ai := (axes.toList.findIdx? (fun a => fieldStr a "column" == input)).getD axes.size
        let real :=
          if col == input then
            (fieldNat (ends[i]!) "coord").map (spanAt (axes[ai]!))
          else producedWhere all col val input
        match real with
        | none => r := r.fail s!"{name}: this program cannot work out where {col} = {val} puts {input}"; ok := false
        | some sp =>
          match (stated[i]?).bind (fun j => statedSpan j sp) with
          | none => r := r.fail s!"{name}: a span for {input} is written in no shape this program reads"; ok := false
          | some st =>
            if !st.holds sp then
              r := r.fail s!"{name}: the span written for {input} while {col} = {val} is narrower than the one this certificate's own tables give"
              ok := false
            else got := got.push sp
    if ok && got.size == 2 && (got[0]!).meets (got[1]!) then
      r := r.fail s!"{name}: the two spans of {input} meet, so the pair is not apart"
  return r

/-- **The axis of an input of an enum is the values the input takes** (rulec's §15.210): the enum's
    values in their order — those its `range` lets in, where an ordered enum declares one
    (`enum_ranges`) — after the absent value of an optional input. `RulecCert.enumAxis_coords` says
    what that settles: every value the input can take has a coordinate the cover tiles, and every
    coordinate is a value the door lets in. Returns how many axes were held to it. -/
def checkEnumAxes (cert : Json) (t : ReadTable) (r : Report) : Report × Nat := Id.run do
  let mut r := r
  let mut held := 0
  let typesJ := (field cert "types").getD Json.null
  let enumsJ := (field cert "enums").getD Json.null
  let rangesJ := (field cert "enum_ranges").getD Json.null
  for ai in [0 : t.columns.length] do
    if t.kinds[ai]? != some "input" then continue
    let col := t.columns[ai]!
    let ty := fieldStr typesJ col
    let optional := ty.endsWith "?"
    let base := if optional then (ty.dropEnd 1).toString else ty
    let some vals := (field enumsJ base >>= arr).map (·.toList.filterMap str) | continue
    let range := (field rangesJ col >>= arr).map (·.toList.filterMap str)
    let want := enumAxis vals range optional
    if t.labels[ai]? == some want then held := held + 1
    else r := r.fail s!"{t.name}: {col}'s axis is not the values its enum lets in: {t.labels[ai]?.getD []} where {want} was declared"
  return (r, held)

def checkTable (t : ReadTable) (r : Report) : Report := Id.run do
  let mut r := r
  let C := t.cert
  let claimed := C.rows.filter (fun row => !t.unused.contains row.index && !t.ruledOut.contains row.index)
  let mut notes : Array String := #[]
  -- E101
  if t.upstream && !t.kinds.contains "upstream" then
    r := r.fail s!"{t.name}: the cover rests on a table above, and no column of this table comes from one"
  if C.coverChecks then
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
  -- E105. A pair the axes part meets at no point at all; a pair the linear model parts
  -- meets at no point the rule is asked about (§15.141), which is what `unique` needs.
  if C.policy == Policy.unique then
    if C.pairAskedChecks then
      let undec := (C.rows.flatMap (fun a => C.rows.filterMap (fun b =>
        if C.undecided a.index b.index then some (a.index, b.index) else none)))
      let byModel := (C.rows.flatMap (fun a => C.rows.filterMap (fun b =>
        if a.index < b.index && (C.told a.index b.index).isNone && (C.refuted a.index b.index).isSome
        then some (a.index, b.index) else none))).length
      let byDays := (C.rows.flatMap (fun a => C.rows.filterMap (fun b =>
        if a.index < b.index && (C.told a.index b.index).isNone && (C.refuted a.index b.index).isNone
          && (C.daysApart a.index b.index).isSome
        then some (a.index, b.index) else none))).length
      let apart := if byModel > 0 then s!" ({byModel} by the linear model)" else ""
      let apart := if byDays > 0 then apart ++ s!" ({byDays} by koyomi's days)" else apart
      if undec.isEmpty then notes := notes.push s!"no two rows meet{apart}"
      else
        r := r.state s!"{undec.length} pairs of {t.name} the axes do not part"
        notes := notes.push s!"{undec.length} pairs undecided (not re-checked){apart}"
    else
      r := r.fail s!"{t.name}: two rows are neither proved apart nor named as undecided"
  let (r', tiled) := checkAxes t r
  r := r'
  if tiled > 0 then notes := notes.push s!"{tiled} axes tiled"
  r := r.say s!"  {t.name}: {C.rows.length} rows — {", ".intercalate notes.toList}"
  r := checkBoxes t r
  return r

/-- **The contracts, held to the rule's door** (§15.142). The door is built again here from
    the rule's ranges, constraints and enums; what is read from the certificate is the reading
    of each contract and the proofs, and `included_sound` is what a pass settles. -/
def checkContracts (cert : Json) (r : Report) : Report := Id.run do
  let mut r := r
  let typesJson := (field cert "types").getD Json.null
  let typeOf : String → String := fun n =>
    ((objPairs typesJson).find? (fun p => p.1 == n)).bind (fun p => str p.2) |>.getD ""
  let enumsJson := (field cert "enums").getD Json.null
  let enumOf : String → Option (List String) := fun t =>
    ((objPairs enumsJson).find? (fun p => p.1 == t)).map (fun p => ((arr p.2).getD #[]).toList.filterMap str)
  let rangeEnd : String → Bool → Option Rat := fun n hi =>
    match (objPairs ((field cert "ranges").getD Json.null)).find? (fun p => p.1 == n) with
    | some (_, v) => (arr v) >>= (fun a => a[if hi then 1 else 0]?) >>= optRat
    | none => none
  let consJ := fieldArr cert "constraints"
  for k in fieldArr cert "contracts" do
    let name := fieldStr k "shape"
    let vars := (field k "vars").getD Json.null
    let nums := (fieldArr vars "num").toList.filterMap str
    let strs := (fieldArr vars "str").toList.filterMap str
    let bools := (fieldArr vars "bool").toList.filterMap str
    let atom : Json → Option CAtom := fun a =>
      match field a "num" with
      | some ts => do
          let terms := objPairs ts
          let coeffs ← terms.foldlM (fun acc t => do
            let i ← nums.idxOf? t.1
            let c ← optRat t.2
            some (vadd acc (unitAt i c))) []
          let kk ← field a "k" >>= optRat
          let q : LinIneq := { coeffs := coeffs, k := kk, strict := fieldStr a "rel" == "lt" }
          match fieldStr a "rel" with
          | "le" => some (.le q)
          | "lt" => some (.le q)
          | "eq" => some (.eq q)
          | _ => none
      | none =>
        match field a "str" >>= str with
        | some n => do
            let i ← strs.idxOf? n
            let yes ← field a "in" >>= boolOf
            some (.str i ((fieldArr a "values").toList.filterMap str) yes)
        | none => do
            let n ← field a "bool" >>= str
            let i ← bools.idxOf? n
            let b ← field a "value" >>= boolOf
            some (.bool i b)
    let why : Json → Option CaseWhy := fun p =>
      match field p "farkas" >>= arr with
      | some refs => do
          let rs ← refs.toList.mapM (fun ref => do
            let y ← field ref "y" >>= optRat
            if (field ref "door" >>= boolOf) == some true then some (CRef.door, y)
            else do
              let i ← fieldNat ref "atom"
              some (CRef.atom i ((fieldNat ref "part").getD 0), y))
          some (.farkas rs)
      | none =>
        match field p "clash" >>= str with
        | some n =>
          match strs.idxOf? n, bools.idxOf? n with
          | some i, _ => some (.clashStr i)
          | none, some i => some (.clashBool i)
          | none, none => none
        | none => if (field p "within" >>= boolOf) == some true then some .within else none
    let some atoms := (fieldArr k "atoms").toList.mapM atom
      | r := r.fail s!"contract {name}: a condition this program cannot read"; continue
    let casesJ := (field k "cases").getD Json.null
    if casesJ.isNull then
      r := r.state s!"contract {name}: its condition opens into too many cases to show anything about"
      r := r.say s!"  contract {name}: too many cases to open (not re-checked)"
      continue
    let cases : List (List Nat) := ((arr casesJ).getD #[]).toList.map (fun c => ((arr c).getD #[]).toList.filterMap nat)
    -- The door, from the rule: each numeric input's range at the scale it travels at, each
    -- `constraint` between two of them, each enum input's values.
    let inputs : List (String × Rat) := (fieldArr k "inputs").toList.filterMap (fun i => do
      let n ← field i "input" >>= str
      let sc ← field i "scale" >>= optRat
      some (n, sc))
    let mut doors : List (String × String × Door) := []
    for (n, sc) in inputs do
      match nums.idxOf? n with
      | none => r := r.fail s!"contract {name}: {n} is fed by the contract and missing from its values"
      | some i =>
        if let some lo := rangeEnd n false then
          doors := doors ++ [(s!"range {n} lo", s!"the low end of {n}'s range",
            .num { coeffs := unitAt i (-1), k := lo * sc, strict := false })]
        if let some hi := rangeEnd n true then
          doors := doors ++ [(s!"range {n} hi", s!"the high end of {n}'s range",
            .num { coeffs := unitAt i 1, k := -(hi * sc), strict := false })]
    for ci in [0 : consJ.size] do
      let c := consJ[ci]!
      let op := fieldStr c "op"
      -- Each side the inputs it adds up (rulec's §15.216), each at the scale it travels at.
      let term : Rat → String → Option (List Rat) := fun sign n => do
        let sc ← inputs.lookup n
        let i ← nums.idxOf? n
        some (unitAt i (sign / sc))
      let ls := sideNames c "left"
      let rs := sideNames c "right"
      let parts : Option (List (List Rat)) := (ls.map (term 1) ++ rs.map (term (-1))).mapM id
      match parts with
      | some ds =>
        if ls.isEmpty || rs.isEmpty then pure () else
        let d := ds.foldl vadd []
        let coeffs := if op == ">=" || op == ">" then vscale (-1) d else d
        doors := doors ++ [(s!"constraint {ci}", s!"constraint {" + ".intercalate ls} {op} {" + ".intercalate rs}",
          .num { coeffs := coeffs, k := 0, strict := op == "<" || op == ">" })]
      | none => pure ()
    for n in strs do
      match enumOf (typeOf n), strs.idxOf? n with
      | some vs, some i => doors := doors ++ [(s!"member {n}", s!"{n} being one of its enum's values", .member i vs)]
      | _, _ => pure ()
    -- The proofs the certificate gives, by the thing the door asks.
    let keyOf : Json → String := fun d =>
      match field d "range" >>= str with
      | some n => s!"range {n} {if (field d "hi" >>= boolOf) == some true then "hi" else "lo"}"
      | none =>
        match fieldNat d "constraint" with
        | some i => s!"constraint {i}"
        | none => s!"member {fieldStr d "member"}"
    let said : List (String × Json) := (fieldArr k "doors").toList.map (fun d => (keyOf d, (field d "proofs").getD Json.null))
    let mut checked : List (String × Door × List CaseWhy) := []
    let mut unshown : List String := []
    let mut bad := false
    for (key, text, door) in doors do
      match said.lookup key with
      | none => unshown := unshown ++ [text]
      | some pj =>
        if pj.isNull then unshown := unshown ++ [text]
        else
          match ((arr pj).getD #[]).toList.mapM why with
          | none => bad := true
          | some ws => checked := checked ++ [(text, door, ws)]
    if bad then
      r := r.fail s!"contract {name}: a proof this program cannot read"
    else if includedOk atoms cases (checked.map (·.2)) then
      r := r.say s!"  contract {name}: {checked.length} of {doors.length} things the door asks hold in all {cases.length} cases"
    else
      for (text, door, ws) in checked do
        if !includedOk atoms cases [(door, ws)] then
          r := r.fail s!"contract {name}: the proofs for {text} do not hold in every case"
    for text in unshown do
      r := r.state s!"contract {name}: {text} is not shown to hold"
    if (field k "unread" >>= boolOf) == some true then
      r := r.state s!"contract {name}: some of its rules could not be read and were taken as true"
  return r

/-- Which rows a `once` line counts: a row whose value there passes the line's test, and a row that
    writes a computed value (which may). -/
def onceCounts (on : Json) : Nat → Bool :=
  let test := (field on "test").getD Json.null
  let lo : Option Int := field test "lo" >>= (fun j => (j.getInt?).toOption)
  let hi : Option Int := field test "hi" >>= (fun j => (j.getInt?).toOption)
  let words := (fieldArr test "words").toList.filterMap str
  let isWords := (field test "words").isSome
  let flagOf : Json → Bool := fun v =>
    if v.isNull then true
    else if isWords then
      match boolOf v with
      | some bv => words.contains (if bv then "true" else "false")
      | none => words.contains ((str v).getD "")
    else
      match (v.getInt?).toOption with
      | some n => (lo.map (fun l => decide (l ≤ n))).getD true && (hi.map (fun h => decide (n ≤ h))).getD true
      | none => true
  let flags : List (Nat × Bool) := (fieldArr on "rows").toList.filterMap (fun x => do
    let i ← fieldNat x "row"
    some (i, flagOf ((field x "value").getD Json.null)))
  fun i => (flags.find? (fun x => x.1 == i)).map (·.2) |>.getD true

/-- The coordinates of a place as the certificate writes them, `-1` outside a range. -/
def coordsOf (j : Json) : Option (List (Option Nat)) := do
  let a ← arr j
  a.toList.mapM (fun x => do
    let i ← (x.getInt?).toOption
    some (if i < 0 then none else some i.toNat))

/-- A place, `[state, [coordinates]]`, and what follows it (a flag, a count). -/
def placeOf (j : Json) : Option ((Nat × List (Option Nat)) × Option Json) := do
  let a ← arr j
  let s ← a[0]? >>= nat
  let cc ← a[1]? >>= coordsOf
  some ((s, cc), a[2]?)

/-- One carried value of the section (rulec's §15.217), read into `Carried.CItem` and held to the
    table it is laid on: its column, its coordinates, the range the rule declares for its input, and
    what each row writes into it — read off the row's own answer (`writes`, `produces`), not taken
    from the section's word. A shape the proofs do not cover comes back as the reason, to be named
    as stated; anything that does not fit what the table says is refused. -/
def readCarriedItem (tname : String) (t : ReadTable) (itJ : Json) (decides : List String)
    (rowsJ : List Json) (valueNames : List String) (declared : String → Option (Rat × Rat)) :
    Except String (Carried.CItem × Option String) := do
  let name := fieldStr itJ "input"
  let out := fieldStr itJ "output"
  let some j := decides.findIdx? (· == out)
    | throw s!"carried values: {tname} does not decide {out}"
  let axis : Option Nat := field itJ "axis" >>= nat
  let named := (List.range t.columns.length).filter (fun i => t.columns[i]? == some name)
  let axisOk := match axis with
    | none => named.isEmpty
    | some ax => named.contains ax
  unless axisOk do throw s!"carried values: the column {tname} reads {name} in is not the one named"
  let some coords := (fieldArr itJ "coords").toList.mapM ratPair
    | throw s!"carried values: the coordinates of {name} cannot be read"
  let kindS := fieldStr itJ "kind"
  let labels := (fieldArr itJ "labels").toList.filterMap str
  let (kind, why) ← (if kindS == "number" then do
      let some (lo, hi) := field itJ "range" >>= ratPair
        | throw s!"carried values: the range of {name} cannot be read"
      if declared name != some (lo, hi) then
        throw s!"carried values: the range of {name} is not the one it declares"
      let some st := field itJ "step" >>= optRat
        | throw s!"carried values: {name} states no step"
      if st ≤ 0 then throw s!"carried values: {name} is cut on a step that does not move"
      unless Carried.tilesB st lo hi coords do
        throw s!"carried values: the coordinates of {name} do not run from one end of its range to the other on its step"
      match field itJ "round" >>= optRat with
      | some g =>
        if g ≤ 0 then throw s!"carried values: {out} rounds to a grid that does not move"
        if gridB st g then pure (Carried.CKind.num lo hi st g, none)
        else pure (Carried.CKind.num lo hi st g,
          some s!"the grid {out} rounds to, {ratText g}, is not a whole number of the step {name} is cut on, {ratText st}")
      | none => pure (Carried.CKind.num lo hi st 1, some s!"the certificate does not say what grid {out} rounds to")
    else if kindS == "bool" || kindS == "enum" then do
      if kindS == "bool" && labels != ["false", "true"] then
        throw s!"carried values: {name} is a truth value, and its coordinates are not false and true"
      unless coords == (List.range labels.length).map (fun (i : Nat) => (((i : Nat) : Rat), ((i : Nat) : Rat))) do
        throw s!"carried values: the coordinates of {name} are not its words' places"
      if let some ax := axis then
        if kindS == "enum" && t.labels[ax]? != some labels then
          throw s!"carried values: the coordinates of {name} are not its enum's values in order"
      pure (Carried.CKind.words labels, none)
    else throw s!"carried values: {name} is carried as `{kindS}`, which this program does not read" :
    Except String (Carried.CKind × Option String))
  let rowOf : Nat → Option Json := fun i => rowsJ.find? (fun x => fieldNat x "row" == some i)
  let mut moves : List (Nat × Carried.CMove) := []
  for mvJ in fieldArr itJ "moves" do
    let some i := fieldNat mvJ "row" | throw "carried values: a move names no row"
    let some rj := rowOf i | throw s!"carried values: row {i} is not in {tname}"
    let wrong := s!"carried values: what row {i} is said to write into {out} is not what the table says"
    let stay := (field mvJ "stay" >>= boolOf) == some true
    let mv ← (match kind with
      | .num .. => do
        let e := ((fieldArr rj "writes")[j]?).bind (fun w => field w "expr")
        let wname := e.bind (fun x => field x "name" >>= str)
        if stay then
          unless wname == some name do throw wrong
          pure Carried.CMove.stay
        else if let some n := field mvJ "expr" >>= str then
          unless wname == some n && valueNames.contains n do throw wrong
          pure (Carried.CMove.expr n)
        else
          let some v := e.bind (fun x => if (field x "num").isSome then field x "value" >>= optRat else none)
            | throw wrong
          let inside := (List.range coords.length).filter (fun ci => match coords[ci]? with
            | some (a, b) => decide (a ≤ v) && decide (v ≤ b)
            | none => false)
          if (field mvJ "out" >>= boolOf) == some true then
            unless inside.isEmpty do throw wrong
          else
            let some to := fieldNat mvJ "to" | throw wrong
            unless inside.contains to do throw wrong
          pure (Carried.CMove.lit v)
      | .words ws => do
        let produced : Option String :=
          (arr ((field rj "produces").getD Json.null)) >>= (fun a => a[j]?) >>= str
        if stay then
          unless produced.isNone do throw wrong
          pure Carried.CMove.stay
        else
          let some to := fieldNat mvJ "to" | throw wrong
          unless produced.isSome && produced == ws[to]? do throw wrong
          pure (Carried.CMove.lit ((to : Nat) : Rat)) : Except String Carried.CMove)
    moves := moves ++ [(i, mv)]
  for rj in rowsJ do
    let some i := fieldNat rj "row" | continue
    unless moves.any (·.1 == i) do throw s!"carried values: row {i} says nothing of {out}"
  let move : Nat → Option Carried.CMove := fun i => (moves.find? (·.1 == i)).map (·.2)
  return ({ input := name, axis := axis, kind := kind, coords := coords, move := move }, why)

/-- Which of the state's own claims the places settle: the line that closes the run does not name
    those as stated. -/
structure CarriedLaid where
  final : Bool := false
  allNever : Bool := false
  /-- The `once` lines laid on the places, by their place among the machine's `once` lines. -/
  onces : List Nat := []
  /-- That a case can still finish, laid on the places by paths of calls that really happen
      (rulec's §15.233). -/
  finish : Bool := false

/-- **The values a machine carries besides its state, held to the rows** (rulec's §15.217, §15.218).
    The section is read into `Carried.Model`; the places have to hold where a case starts and be
    closed under every call the check counts from them, which `Carried.Model.simulates` and
    `Carried.Sys.reaches_mem` make a statement about every configuration a case can reach; and each
    claim the section lays there is the check `Carried.Sys.finalOk`, `neverOk`, `onceOk`,
    `Carried.Model.alwaysOk` or `rangeOk`, whose theorems say what a pass settles. -/
def checkCarried (cert mj tj : Json) (t : ReadTable) (M : Machine) (init : Nat) (finals : List Nat)
    (states : List String) (r : Report) : Report × CarriedLaid := Id.run do
  let mut r := r
  let cm := (field mj "carried").getD Json.null
  if (field cm "items").isNone then
    r := r.state s!"the values the machine carries besides its state are not laid on the rows: {fieldStr cm "why"}"
    return (r, {})
  let tname := fieldStr mj "table"
  let decides := (fieldArr tj "decides").toList.filterMap str
  let rowsJ := (fieldArr tj "rows").toList
  let valuesJ := (fieldArr cert "values").toList
  let valueNames := valuesJ.map (fun v => fieldStr v "name")
  -- A date written in an expression is its day number here (rulec's §15.233): a call that really
  -- happens is worked out with the values, and a date input's value is its day number.
  let values : List (String × RulecCert.Expr) := valuesJ.filterMap (fun v => do
    let e ← field v "expr" >>= (exprOfJson · true)
    some (fieldStr v "name", e))
  let rangesJ := (field cert "ranges").getD Json.null
  let declared : String → Option (Rat × Rat) := fun n => field rangesJ n >>= ratPair
  let ranges : List (String × Span2) := ((fieldArr cert "inputs").toList.filterMap str).filterMap
    (fun n => (declared n).map (fun I => (n, I)))
  let mut items : List Carried.CItem := []
  let mut outs : List String := []
  for itJ in fieldArr cm "items" do
    match readCarriedItem tname t itJ decides rowsJ valueNames declared with
    | .error why => return (r.fail why, {})
    | .ok (it, none) =>
      items := items ++ [it]
      outs := outs ++ [fieldStr itJ "output"]
    | .ok (_, some why) =>
      r := r.state s!"the claims over the values the machine carries besides its state: {why}"
      return (r, {})
  -- The rule's inputs and its `constraint` lines, which say what the door lets through: what a call
  -- that really happens is made with (rulec's §15.233).
  let inputNames := (fieldArr cert "inputs").toList.filterMap str
  let cons : List (List String × Cmp × List String) := (fieldArr cert "constraints").toList.filterMap (fun k => do
    let op ← cmpOfString (fieldStr k "op")
    some (sideNames k "left", op, sideNames k "right"))
  let K : Carried.Model := { m := M, coords := t.spans, labels := t.labels, columns := t.columns,
                             ranges := ranges, values := values, items := items, inputs := inputNames,
                             cons := cons }
  if !K.okB then return (r.fail "carried values: the section is not one the proofs read", {})
  let some places := (fieldArr cm "places").toList.mapM (fun x => (placeOf x).map (·.1))
    | return (r.fail "carried values: a place cannot be read", {})
  let some startCs := field cm "start" >>= coordsOf
    | return (r.fail "carried values: where a case starts cannot be read", {})
  let start : Nat × List (Option Nat) := (init, startCs)
  -- The start has to be a place a case can be at (`Carried.Model.start_inhabited`): without one
  -- coordinate of each carried value, no case would start there, and every claim would hold of
  -- nothing.
  if !K.startOk startCs then
    return (r.fail "carried values: where a case starts is not one coordinate of each carried value", {})
  let shaped : List (Option Nat) → Bool := fun cc => cc.length == items.length &&
    (items.zip cc).all (fun ic => match ic.2 with
      | some ci => decide (ci < ic.1.coords.length)
      | none => match ic.1.kind with
        | .num .. => true
        | .words _ => false)
  if !places.all (fun p => shaped p.2) then
    return (r.fail "carried values: a place does not give one coordinate of each carried value", {})
  let S := K.sys
  if !places.contains start then return (r.fail "carried values: the places leave out where a case starts", {})
  for p in places do
    for e in S.succs p do
      if !places.contains e.2 then
        return (r.fail s!"carried values: from {(states[p.1]?).getD "?"}, row {e.1} leads outside the places", {})
  if !S.reachOk start places then return (r.fail "carried values: the places are not closed under a call", {})
  let mut claims : Array String := #[s!"{places.length} places a case can be"]
  let mut laid : CarriedLaid := {}
  if (field cm "final_certified" >>= boolOf) == some true then
    if S.finalOk finals places then
      claims := claims.push "no call leaves a final state"
      laid := { laid with final := true }
    else return (r.fail "carried values: a row leads out of a final state", {})
  else if places.any (fun p => finals.contains p.1) then
    r := r.state "carried values: that no call leaves a final state is not laid on the rows"
  let neverDefs := (fieldArr mj "never").toList
  let mut neverLaid := 0
  for ((nv, cj), li) in (neverDefs.zip (fieldArr cm "never").toList).zip (List.range neverDefs.length) do
    if cj.isNull then
      r := r.state s!"carried values: the `never` line {li + 1} is not laid on the rows"
      continue
    let a := (fieldArr nv "states").toList.filterMap nat
    let b := (fieldArr nv "after").toList.filterMap nat
    let some Q := ((arr cj).getD #[]).toList.mapM (fun x => do
        let (pl, rest) ← placeOf x
        let f ← rest >>= boolOf
        some (pl, f))
      | return (r.fail s!"carried values: the `never` set of line {li + 1} cannot be read", {})
    if S.neverOk start a b Q then
      claims := claims.push s!"`never` line {li + 1}"
      neverLaid := neverLaid + 1
    else return (r.fail s!"carried values: the `never` set of line {li + 1} is not closed, or holds a state after the states it is to come after", {})
  laid := { laid with allNever := !neverDefs.isEmpty && neverLaid == neverDefs.length }
  let onceDefs := (fieldArr mj "once").toList
  for ((on, cj), li) in (onceDefs.zip (fieldArr cm "once").toList).zip (List.range onceDefs.length) do
    if cj.isNull || (field on "test").isNone then
      r := r.state s!"carried values: `once {fieldStr on "output"}` is not laid on the rows"
      continue
    let some Q := ((arr cj).getD #[]).toList.mapM (fun x => do
        let (pl, rest) ← placeOf x
        let n ← rest >>= nat
        some (pl, n))
      | return (r.fail s!"carried values: the `once {fieldStr on "output"}` set cannot be read", {})
    if S.onceOk start (onceCounts on) Q then
      claims := claims.push s!"`once` line {li + 1} ({fieldStr on "output"})"
      laid := { laid with onces := laid.onces ++ [li] }
    else return (r.fail s!"carried values: the `once {fieldStr on "output"}` set is not closed, or counts two", {})
  let alwaysJ := (fieldArr cm "always").toList
  for (al, li) in alwaysJ.zip (List.range alwaysJ.length) do
    let when := (fieldArr al "when").toList.filterMap nat
    let some tests := (fieldArr al "tests").toList.mapM (fun x => do
        let k ← fieldNat x "item"
        let c ← field x "cell" >>= (fun cj => cellOfJson (fun _ => []) cj)
        some (k, c))
      | return (r.fail s!"carried values: the `always` line {li + 1} cannot be read", {})
    if (field al "holds" >>= boolOf) == some true then
      if K.alwaysOk places { when := when, tests := tests } then
        claims := claims.push s!"`always` line {li + 1}"
      else return (r.fail s!"carried values: the `always` line {li + 1} does not hold at every place it names", {})
    else
      r := r.state s!"carried values: the `always` line {li + 1} is not laid on the rows"
  for rg in fieldArr cm "ranges" do
    let some k := fieldNat rg "item" | return (r.fail "carried values: a range claim names no value", {})
    let out := (outs[k]?).getD "?"
    if (field rg "holds" >>= boolOf) == some true then
      if Carried.Model.rangeOk places k then claims := claims.push s!"{out} within its range"
      else return (r.fail s!"carried values: {out} is said to stay within its range, and a place leaves it", {})
    else
      r := r.state s!"carried values: that {out} stays within its range is not laid on the rows"
  -- That a case can still finish (rulec's §15.233): from every place not in a final state, a path of
  -- calls that really happen, each run here (`Carried.Model.callOk`); `Carried.Model.reachable_finishes`
  -- is what a pass settles. Where the section lays none, the claim is named as stated (by the caller).
  if !finals.isEmpty then
    if let some fj := field cm "finish" >>= arr then
      let typesJ := (field cert "types").getD Json.null
      let callOf : Json → Option Carried.FCall := fun x => do
        let row ← fieldNat x "row"
        let at_ := (fieldArr x "at").toList.filterMap nat
        let ins ← (objPairs ((field x "inputs").getD Json.null)).mapM (fun (n, v) => do
          let q ← optRat v
          let ty ← tyOfString (fieldStr typesJ n)
          some (n, ((q, ty) : UVal)))
        some { row := row, point := at_, inputs := ins }
      let some paths := fj.toList.mapM (fun x => do
          let (pl, _) ← (field x "place") >>= placeOf
          let calls ← (fieldArr x "path").toList.mapM callOf
          some (pl, calls))
        | return (r.fail "carried values: a path to a final state cannot be read", {})
      let pathsOf : Nat × List (Option Nat) → List Carried.FCall := fun pl =>
        ((paths.find? (fun q => q.1 == pl)).map (·.2)).getD []
      -- The values behind a point, as the table's sieve reads them: each column's, then the names of
      -- its linear model's that are not columns (`linear.extra`).
      let extras := (fieldArr ((field tj "linear").getD Json.null) "extra").toList.filterMap str
      let valOf : (String → Option UVal) → String → Rat := fun env n => ((env n).map (·.1)).getD 0
      let askedOk : Point → (String → Option UVal) → Bool := fun p env =>
        witnessOk t.cert.sieve p (t.columns.map (valOf env) ++ extras.map (valOf env))
      if K.finishOk askedOk finals places pathsOf then
        claims := claims.push "every place a case can be can still finish"
        laid := { laid with finish := true }
      else return (r.fail "carried values: a place has no path of calls to a final state that checks", {})
  r := r.say s!"  carried values over {tname}: {", ".intercalate claims.toList}"
  return (r, laid)

/-- **A machine, held to the rows of the table it is laid on** (§15.148). The moves are read
    back against what the table says each row writes; the transitions are rebuilt from the
    rows' boxes; and each claim is one of the checks in `RulecCert.Machine`, whose theorems say
    what a pass settles: `reaches_mem`, `final_stays`, `never_after`, `once_below_two`, and
    `reachable_finishes`. -/
def checkMachine (cert : Json) (tables : Array ReadTable) (r : Report) : Report := Id.run do
  let mut r := r
  let some mj := field cert "machine" | return r
  if mj.isNull then return r
  let tname := fieldStr mj "table"
  if tname.isEmpty then
    r := r.state s!"the machine is not laid on a table's certificate: {fieldStr mj "why"}"
    r := r.say s!"  machine: not certified here — {fieldStr mj "why"}"
    return r
  let some t := tables.find? (fun t => t.name == tname)
    | return r.fail s!"machine: the table {tname} has no certificate this program read"
  let some tj := (fieldArr cert "tables").find? (fun x => fieldStr x "table" == tname)
    | return r.fail s!"machine: the table {tname} is not in the certificate"
  let C := t.cert
  let states := (fieldArr mj "states").toList.filterMap str
  let carry := (field mj "carry").getD Json.null
  let axis : Option Nat := field mj "axis" >>= nat
  match axis with
  | some k =>
    if t.columns[k]? != some (fieldStr carry "input") then
      return r.fail s!"machine: axis {k} of {tname} is not the carried input {fieldStr carry "input"}"
    if t.labels[k]? != some states then
      return r.fail "machine: the state's axis is not the states in order"
  | none => pure ()
  let decides := (fieldArr tj "decides").toList.filterMap str
  let some jout := decides.findIdx? (· == fieldStr carry "output")
    | return r.fail s!"machine: {tname} does not decide {fieldStr carry "output"}"
  -- Each row's move, read back against what the table says the row writes.
  let mut moves : List (Nat × Move) := []
  for e in fieldArr mj "rows" do
    let some i := fieldNat e "row" | return r.fail "machine: a move names no row"
    let produced : Option String := ((fieldArr tj "rows").find? (fun x => fieldNat x "row" == some i)) >>=
      (fun x => (arr ((field x "produces").getD Json.null)) >>= (fun a => a[jout]?) >>= str)
    match fieldNat e "to", (field e "stay" >>= boolOf) with
    | some to, _ =>
      if produced != states[to]? then
        return r.fail s!"machine: row {i} is said to go to {to}, and the table says it writes something else"
      moves := moves ++ [(i, Move.goTo to)]
    | none, some true =>
      if produced.isSome then
        return r.fail s!"machine: row {i} is said to leave the state, and the table says it writes a value"
      moves := moves ++ [(i, Move.stay)]
    | _, _ => return r.fail s!"machine: row {i} has no move"
  let move : Nat → Option Move := fun i => (moves.find? (fun x => x.1 == i)).map (·.2)
  if !C.rows.all (fun row => (move row.index).isSome) then
    return r.fail "machine: a row of the table has no move"
  let M : Machine := { table := C.table, axis := axis, move := move }
  let init := (fieldNat mj "initial").getD 0
  let finals := (fieldArr mj "finals").toList.filterMap nat
  -- The worlds (§15.149): every combination of the held columns' coordinates, once each.
  let fixedInputs := (fieldArr mj "held_inputs").toList.filterMap str
  let fixed := (fieldArr mj "held").toList.filterMap nat
  let axesJ := fieldArr tj "axes"
  for fk in fixed do
    let ax := axesJ[fk]?.getD Json.null
    if fieldStr ax "kind" != "input" || !fixedInputs.contains (fieldStr ax "column") then
      return r.fail s!"machine: axis {fk} is not a held input's column"
  for fi in fixedInputs do
    if t.columns.contains fi && !(fixed.any (fun fk => t.columns[fk]? == some fi)) then
      return r.fail s!"machine: {fi} is held and a column of {tname}, and no world is cut by it"
  let want : List (List Nat) := fixed.foldl (fun acc fk =>
    acc.flatMap (fun w => (List.range (t.labels[fk]?.getD []).length).map (fun c => w ++ [c]))) [[]]
  let worldsJ := fieldArr mj "worlds"
  let got : List (List Nat) := worldsJ.toList.map (fun wj => (fieldArr wj "at").toList.filterMap nat)
  if got.length != want.length || !want.all (fun w => got.contains w) then
    return r.fail "machine: the worlds are not every combination of the held columns' coordinates, once each"
  -- A held input read by more than its own column: a path of calls at the world's coordinates
  -- is then not shown to be one case's, and none is taken as proof.
  let coupled := !fixedInputs.isEmpty &&
    (axesJ.any (fun ax => fieldStr ax "kind" != "input") ||
     (fieldArr cert "constraints").any (fun kc => (sideNames kc "left" ++ sideNames kc "right").any fixedInputs.contains))
  -- A machine that carries more than its state (§15.217): a path of the state's rows is not one
  -- case's, since each call moves the carried values too, and none is taken as proof. Its claims
  -- over the carried values are laid in `carried` and checked below (`checkCarried`, with the
  -- theorems of `RulecCert.Carried`); a claim of the state's own section that the rows could not
  -- carry, and the places do, is not named as stated at the end.
  let carriedJ := (field mj "carried").getD Json.null
  let carriesMore := !carriedJ.isNull
  let mut pending : Array (String × String) := #[]
  let neverDefs := (fieldArr mj "never").toList
  let onceDefs := (fieldArr mj "once").toList
  let mut claims : Array String := #[]
  let mut reached : List Nat := []
  let add : Array String → String → Array String := fun cs c => if cs.contains c then cs else cs.push c
  for wj in worldsJ do
    let w := (fieldArr wj "at").toList.filterMap nat
    let Mw := M.world fixed w
    let reach := (fieldArr wj "reach").toList.filterMap nat
    if Mw.reachOk init reach then
      reached := reached ++ reach.filter (fun x => !reached.contains x)
    else r := r.fail "machine: a reach set does not hold the start, or is not closed under a call"
    let finalsHere := finals.filter (fun x => reach.contains x)
    if (field wj "final_certified" >>= boolOf) == some true then
      if Mw.finalOk finalsHere then claims := add claims "no call leaves a final state"
      else r := r.fail "machine: a row leads out of a final state"
    else if !finalsHere.isEmpty then
      pending := pending.push ("final", "that no call leaves a final state is not laid on the rows")
    for (nv, cj) in neverDefs.zip (fieldArr wj "never").toList do
      if cj.isNull then
        pending := pending.push ("never", "a `never` line of the machine is not laid on the rows")
      else
        let a := (fieldArr nv "states").toList.filterMap nat
        let b := (fieldArr nv "after").toList.filterMap nat
        let P : List (Nat × Bool) := ((arr cj).getD #[]).toList.filterMap (fun x => do
          let xs ← arr x
          let st ← xs[0]? >>= nat
          let bb ← xs[1]? >>= boolOf
          some (st, bb))
        if Mw.neverOk init a b P then claims := add claims "a `never` line"
        else r := r.fail "machine: a `never` set is not closed, or holds a state it is never to reach"
    for ((on, cj), li) in (onceDefs.zip (fieldArr wj "once").toList).zip (List.range onceDefs.length) do
      if cj.isNull || (field on "test").isNone then
        pending := pending.push (s!"once:{li}", s!"`once {fieldStr on "output"}` is not laid on the rows")
      else
        let counts : Nat → Bool := onceCounts on
        let P : List (Nat × Nat) := ((arr cj).getD #[]).toList.filterMap (fun x => do
          let xs ← arr x
          let st ← xs[0]? >>= nat
          let n ← xs[1]? >>= nat
          some (st, n))
        if Mw.onceOk init counts P then claims := add claims s!"`once {fieldStr on "output"}`"
        else r := r.fail s!"machine: the `once {fieldStr on "output"}` set is not closed, or counts two"
    if !finals.isEmpty then
      let pathsJ := fieldArr wj "finish"
      if coupled then
        if pathsJ.size > 0 then
          r := r.fail "machine: paths to a final state are given, and a held input is read by more than its own column"
        else
          r := r.state "that a case can still finish is not laid on the rows (a held input is read by more than its own column)"
      else if carriesMore then
        -- Over the carried values, each place's path is checked in `checkCarried` (rulec's §15.233).
        if pathsJ.size > 0 then
          r := r.fail "machine: paths to a final state are given, and the machine carries more than its state"
        else
          pending := pending.push ("finish", "that a case can still finish is not laid on the rows (the machine carries more than its state)")
      else
        let callOf : Json → Option Machine.Call := fun x => do
          let st ← fieldNat x "state"
          let row ← fieldNat x "row"
          let at_ := (fieldArr x "at").toList.filterMap nat
          let vs := (fieldArr x "at_values").toList.map (fun v => (optRat v).getD 0)
          let xs := (fieldArr x "extra_values").toList.map (fun v => (optRat v).getD 0)
          some { state := st, row := row, point := at_, values := vs ++ xs }
        let paths : Nat → List Machine.Call := fun s =>
          match pathsJ.find? (fun x => fieldNat x "state" == some s) with
          | some x => (fieldArr x "path").toList.filterMap callOf
          | none => []
        -- Every call of a path is made with the world's coordinates on the held columns.
        let atWorld : Json → Bool := fun c =>
          let at_ := (fieldArr c "at").toList.filterMap nat
          (fixed.zip w).all (fun kc => at_[kc.1]? == some kc.2)
        if !(pathsJ.all (fun x => (fieldArr x "path").all atWorld)) then
          r := r.fail "machine: a call on a path is not made with the world's held coordinates"
        else
          -- The values behind each call, where the axis is one of numbers: without them the call
          -- is held to the weaker reading and the program says so, as it does for reach points.
          let stated := pathsJ.all (fun x => (fieldArr x "path").all (fun c =>
            let vs := fieldArr c "at_values"
            vs.size == t.columns.length &&
              (List.range t.columns.length).all (fun ai =>
                match t.spans[ai]? with
                | some cs => cs.all (·.isNone) || !(vs[ai]!).isNull
                | none => true)))
          if stated then
            if Mw.finishOk (witnessOk C.sieve) reach finals paths then
              claims := add claims "every state reached can still finish"
            else r := r.fail "machine: a state reached has no path of calls to a final state"
          else if Mw.finishOk (fun p _ => !pointRuledOut C.sieve p) reach finals paths then
            r := r.state "the machine's paths come with no values behind their calls"
            claims := add claims "every state reached can still finish (the calls are not shown to be asked about)"
          else r := r.fail "machine: a state reached has no path of calls to a final state"
  let inWorlds := if fixed.isEmpty then "" else s!" in {worldsJ.size} worlds"
  claims := #[s!"{reached.length} of {states.length} states reached{inWorlds}"] ++ claims
  r := r.say s!"  machine over {tname}: {", ".intercalate claims.toList}"
  let mut laid : CarriedLaid := {}
  if carriesMore then
    let (r', l) := checkCarried cert mj tj t M init finals states r
    r := r'
    laid := l
  -- A claim the state's own rows could not carry is laid when the places carry it.
  for (tag, msg) in pending do
    let settled := (tag == "final" && laid.final) || (tag == "never" && laid.allNever) ||
      (tag == "finish" && laid.finish) || (laid.onces.any (fun li => tag == s!"once:{li}"))
    if !settled then r := r.state msg
  return r

/-- The text the file has at a place a `source` names. -/
def textAt (lines : Array ByteArray) (src : Json) : Option String :=
  let ln := (fieldNat src "line").getD 0
  let col := (fieldNat src "col").getD 0
  let len := (fieldNat src "len").getD 0
  let line := if 0 < ln && ln ≤ lines.size then lines[ln - 1]! else .empty
  if col + len ≤ line.size then String.fromUTF8? (line.extract col (col + len)) else none

/-- Whether a `source` is the answer cell of column `j` (in the order the table decides its columns)
    of the row numbered `row`: on the row's own line, the cell `j` places after the cells its tests
    read. A row whose answers are not cells of its own line (a `clause`, whose `then` line holds them)
    has no such cell, and only what the file says there is held to the certificate. -/
def isAnswerCell (tj : Json) (lines : Array ByteArray) (row j : Nat) (src : Json) : Bool :=
  match (fieldArr tj "rows").find? (fun x => fieldNat x "row" == some row) with
  | none => false
  | some rj =>
    let ln := (fieldNat rj "line").getD 0
    let line := if 0 < ln && ln ≤ lines.size then lines[ln - 1]! else .empty
    let bars := fieldsOf line
    let tests := ((field rj "source").bind srcOfJson).map (fun ss => (ss.filterMap id).length)
    match tests with
    | none => false
    | some n =>
      if bars.size == 0 then true
      else
        let col := (fieldNat src "col").getD 0
        let len := (fieldNat src "len").getD 0
        fieldNat src "line" == some ln && bars[n + j]? == some (col, col + len)

/-- Whether an `initial` line stands at a place: the line reads `initial`, then `before` (the carried
    input's name, for a value carried besides the state), then the value at the place, and nothing
    after it but a comment. -/
def initialLineOk (lines : Array ByteArray) (src : Json) (before : List String) : Bool :=
  let ln := (fieldNat src "line").getD 0
  let col := (fieldNat src "col").getD 0
  let len := (fieldNat src "len").getD 0
  let line := if 0 < ln && ln ≤ lines.size then lines[ln - 1]! else .empty
  let head := (String.fromUTF8? (line.extract 0 col)).getD ""
  let tail := ((String.fromUTF8? (line.extract (col + len) line.size)).getD "").trimAscii.toString
  let words := (head.splitOn " ").filter (· != "") |>.map (·.trimAscii.toString) |>.filter (· != "")
  col + len ≤ line.size && words == "initial" :: before && (tail.isEmpty || tail.startsWith "#")

/-- **The machine's section says what the file says** (§15.148, rulec's §15.233). A row said to leave
    the state writes the carried input's own name there, and a row said to go to a state writes that
    state — each in the row's answer cell of the carried output. A value carried besides the state is
    the same: its input's name, a computed value's name, a word. A `once` line's value on each row is
    the one its answer cell writes: the word, or the number at the scale the wire counts it at (a name
    the rule computes counts wherever it might). And where a case starts is what the `initial` lines
    say: the state, and each value carried besides it, inside the coordinate the start gives it.
    Returns how many cells were read back. -/
def checkMachineSpans (cert : Json) (lines : Array ByteArray) (r : Report)
    (curOf : String → Option String := fun _ => none) : Report × Nat := Id.run do
  let mut r := r
  let mut read := 0
  let some mj := field cert "machine" | return (r, 0)
  if mj.isNull || (fieldStr mj "table").isEmpty then return (r, 0)
  let tname := fieldStr mj "table"
  let tj := ((fieldArr cert "tables").find? (fun x => fieldStr x "table" == tname)).getD Json.null
  let decides := (fieldArr tj "decides").toList.filterMap str
  let colOf : String → Nat := fun o => (decides.idxOf? o).getD decides.length
  let states := (fieldArr mj "states").toList.filterMap str
  let carry := (field mj "carry").getD Json.null
  let cin := fieldStr carry "input"
  let jout := colOf (fieldStr carry "output")
  for e in fieldArr mj "rows" do
    let row := (fieldNat e "row").getD 0
    let some src := field e "source"
      | r := r.fail s!"machine: row {row}'s move has no place in the file"
    let got := textAt lines src
    let want : Option String := match fieldNat e "to" with
      | some to => states[to]?
      | none => some cin
    if got != want then
      r := r.fail s!"machine: row {row}'s move is not what the file has there"
    else if !isAnswerCell tj lines row jout src then
      r := r.fail s!"machine: row {row}'s move is not read from the row's own cell of {fieldStr carry "output"}"
    else read := read + 1
  -- The values carried besides the state (§15.217): a row said to keep one writes the carried input's
  -- own name there, a row said to write a computed one writes its name, a word is the word.
  for it in fieldArr ((field mj "carried").getD Json.null) "items" do
    let labels := (fieldArr it "labels").toList.filterMap str
    let j := colOf (fieldStr it "output")
    for e in fieldArr it "moves" do
      let row := (fieldNat e "row").getD 0
      let some src := field e "source"
        | r := r.fail s!"carried values: row {row}'s move of {fieldStr it "output"} has no place in the file"
      let got := textAt lines src
      let want : Option String :=
        if (field e "stay" >>= boolOf) == some true then some (fieldStr it "input")
        else if let some n := field e "expr" >>= str then some n
        else if !labels.isEmpty then (fieldNat e "to").bind (fun i => labels[i]?)
        else some (fieldStr src "text")
      if got != want || got != some (fieldStr src "text") then
        r := r.fail s!"carried values: row {row} is said to write `{want.getD ""}` into {fieldStr it "output"}, and the file has `{got.getD ""}` there"
      else if !isAnswerCell tj lines row j src then
        r := r.fail s!"carried values: row {row}'s move of {fieldStr it "output"} is not read from the row's own cell of it"
      else read := read + 1
  -- What a `once` line reads on each row (rulec's §15.233): the value its answer cell writes.
  for on in fieldArr mj "once" do
    let some test := field on "test" | continue
    let out := fieldStr on "output"
    let j := colOf out
    let scale : Option Rat := field test "scale" >>= optRat
    let numeric := (field test "words").isNone
    for e in fieldArr on "rows" do
      let row := (fieldNat e "row").getD 0
      let some src := field e "source"
        | r := r.fail s!"machine: the value `once {out}` reads on row {row} has no place in the file"
      let got := textAt lines src
      if got != some (fieldStr src "text") then
        r := r.fail s!"machine: the value `once {out}` reads on row {row} is not what the file has there"
        continue
      if !isAnswerCell tj lines row j src then
        r := r.fail s!"machine: the value `once {out}` reads on row {row} is not read from the row's own cell of {out}"
        continue
      let v := (field e "value").getD Json.null
      let text := (got.getD "").trimAscii.toString
      -- A computed value counts wherever it might: nothing to hold it to.
      if v.isNull then
        read := read + 1
        continue
      let agrees : Option Bool :=
        if numeric then do
          let sc ← scale
          let w ← (v.getInt?).toOption
          let x ← (literalValue text (curOf out)).bind id
          some (x * sc == ((w : Int) : Rat))
        else
          match boolOf v with
          | some b => some (text == (if b then "true" else "false"))
          | none => some (str v == some text)
      match agrees with
      | some true => read := read + 1
      | some false => r := r.fail s!"machine: row {row} writes `{text}` into {out}, and the certificate reads another value for the `once` line"
      | none =>
        if numeric && scale.isNone then
          r := r.state s!"the values the `once {out}` line reads: the certificate does not say the scale they are counted at"
        else r := r.fail s!"machine: row {row} writes `{text}` into {out}, which is not the number the certificate reads for the `once` line"
  -- Where a case starts (rulec's §15.233): the state the `initial` line names, and for each value
  -- carried besides it, the value its own `initial` line gives, inside the coordinate the start gives
  -- it. A certificate from before it said where those lines are leaves the start its own word.
  match field mj "initial_source" with
  | none => r := r.state "where a case starts: the certificate does not say where the `initial` line is"
  | some src =>
    let init := (fieldNat mj "initial").getD 0
    if textAt lines src != states[init]? || textAt lines src != some (fieldStr src "text") then
      r := r.fail s!"machine: the `initial` line does not name {(states[init]?).getD "?"} there"
    else if !initialLineOk lines src [] then
      r := r.fail "machine: where a case starts is not read from an `initial` line"
    else read := read + 1
  let cm := (field mj "carried").getD Json.null
  let startJ := fieldArr cm "start"
  let items := fieldArr cm "items"
  for (it, k) in items.toList.zip (List.range items.size) do
    let input := fieldStr it "input"
    match field it "initial_source" with
    | none => r := r.state s!"where {input} starts: the certificate does not say where its `initial` line is"
    | some src =>
      let got := textAt lines src
      if got != some (fieldStr src "text") then
        r := r.fail s!"carried values: the `initial` line of {input} is not what the file has there"
      else if !initialLineOk lines src [input] then
        r := r.fail s!"carried values: where {input} starts is not read from its `initial` line"
      else
        let text := (got.getD "").trimAscii.toString
        let ci := (startJ[k]? >>= nat)
        let labels := (fieldArr it "labels").toList.filterMap str
        let coord : Option (Rat × Rat) := ci.bind (fun c => (fieldArr it "coords")[c]? >>= ratPair)
        let ok : Bool :=
          if !labels.isEmpty then (ci.bind (fun c => labels[c]?)) == some text
          else match (literalValue text (curOf input)).bind id, coord with
            | some x, some (a, b) => decide (a ≤ x) && decide (x ≤ b)
            | _, _ => false
        if ok then read := read + 1
        else r := r.fail s!"carried values: {input} starts at `{text}` by its `initial` line, and the certificate starts it elsewhere"
  return (r, read)

/-- **What a `max` or a `min` answers, held to its column and its `empty` value** (rulec's §15.220).
    The answer is a value of the column or the `empty` value, so the range the certificate states for
    it has to hold the column's interval — worked out here as every name's is (`Worked.rangesOf`) —
    and the `empty` value: `extremeOk`, and `extreme_mem_stated` is what a pass settles. One of an
    ordered enum answers one of its values, `empty` among them, and states no range. -/
def checkExtremes (cert : Json) (w : Worked) (r : Report) : Report := Id.run do
  let mut r := r
  let types := (field cert "types").getD Json.null
  let enums := (field cert "enums").getD Json.null
  let rangesJ := (field cert "ranges").getD Json.null
  let walks := (fieldArr cert "walks").toList.filterMap str
  let mut held := 0
  for e in fieldArr cert "extremes" do
    let name := fieldStr e "name"
    let col := fieldStr e "of"
    let kind := fieldStr e "kind"
    if kind != "max" && kind != "min" then
      r := r.fail s!"{name}: the certificate calls it a `{kind}`, which is neither `max` nor `min`"
      continue
    if !walks.contains name then
      r := r.fail s!"{name}: a `{kind}` is a value a walk leaves behind, and `walks` does not name it"
      continue
    let ty := fieldStr types col
    if ty.isEmpty || fieldStr types name != ty then
      r := r.fail s!"{name}: a `{kind}` of {col} answers with {col}'s type `{ty}`, and the certificate says `{fieldStr types name}`"
      continue
    let base := if ty.endsWith "?" then (ty.dropEnd 1).toString else ty
    match field enums base >>= arr with
    | some vs =>
      if !(vs.toList.filterMap str).contains (fieldStr e "empty") then
        r := r.fail s!"{name}: its `empty` value {fieldStr e "empty"} is not a value of {base}"
      else if (field rangesJ name).isSome then
        r := r.fail s!"{name}: a `{kind}` of an enum answers one of its values, and the certificate states a range for it"
      else held := held + 1
    | none =>
      match field rangesJ name with
      | none => held := held + 1  -- no range is claimed for the answer, so nothing reads it with one
      | some sj =>
        match ratPair sj with
        | none =>
          -- An end left open: `ranges` gives this program no interval for the name, so nothing here
          -- reads the answer with it; that it holds is the certificate's word.
          r := r.state s!"the range of {name}, the {kind} of {col}: an end of it is open"
        | some S =>
          match w.rangesOf col, field e "empty" >>= optRat with
          | none, _ =>
            r := r.fail s!"{name}: the certificate states a range for it, and this program cannot work out the range of {col}"
          | _, none =>
            r := r.fail s!"{name}: a `{kind}` of numbers or dates states no `empty` value"
          | some C, some z =>
            if extremeOk C z S then held := held + 1
            else
              let H := hullWith C z
              r := r.fail s!"{name}: a `{kind}` of {col} answers from {ratText H.1} to {ratText H.2} with its `empty` value, and the certificate states the range [{ratText S.1}, {ratText S.2}]"
  if held > 0 then
    r := r.say s!"  extremes: {held} `max` and `min` answers keep to their column's range and their `empty` value"
  return r

/-- One value a fold answers with, as an expression: a date written in the rule stands for its day
    number, on the scale `ranges` gives dates (rulec's §15.196), so it is read as that number. -/
def answerExprOfJson (j : Json) : Option RulecCert.Expr := exprOfJson j (dates := true)

/-- **What a fold answers, held to the values it answers with** (rulec's §15.219). The answer is the
    value of one of the expressions the certificate lists (`empty`, `exhausted` unless it is `held`, a
    `stop with`, what an arm takes or keeps), at the call or at one element, so the range the
    certificate states for it has to hold the interval of every one: `writesWithin`, and
    `foldAnswer_mem_stated` is what a pass settles. -/
def checkFoldAnswers (cert : Json) (w : Worked) (r : Report) : Report := Id.run do
  let mut r := r
  let rangesJ := (field cert "ranges").getD Json.null
  let mut held := 0
  for fa in fieldArr cert "fold_answers" do
    let name := fieldStr fa "name"
    let some S := field rangesJ name >>= ratPair
      | r := r.fail s!"{name}: the certificate lists the values the fold answers it with, and states no range for it"
        continue
    let some answers := (fieldArr fa "answers").toList.mapM answerExprOfJson
      | r := r.fail s!"{name}: a value the fold answers with cannot be read"
        continue
    if answers.isEmpty then
      r := r.fail s!"{name}: the fold answers with no value at all"
      continue
    let ws : List Writes := answers.map (fun e => { expr := e })
    if writesWithin w.rangesOf w.within S ws then held := held + 1
    else
      match hullOf w.rangesOf w.within ws with
      | some H =>
        r := r.fail s!"{name}: the fold answers with values from {ratText H.1} to {ratText H.2}, and the certificate states the range [{ratText S.1}, {ratText S.2}]"
      | none =>
        r := r.fail s!"{name}: one of the values the fold answers with has no interval this program can work out"
  if held > 0 then
    r := r.say s!"  fold answers: {held} ranges hold every value their fold answers with"
  return r

/-- Whether two lists hold the same values, each once. -/
def sameOnce (a b : List String) : Bool :=
  a.length == b.length && b.all (fun x => a.count x == 1)

/-- The answer each value of a block's enum gets from the `ordered by` table (rulec's §15.222), read
    off the table's rows: the first row whose box takes the value's coordinate on the table's one axis
    — the row that answers it, which the table's own certificate says is the only one under `unique`
    (`Certified.unique`) — and what it writes: an ordered enum's value by its place in the enum, a
    number by itself. The numbers a row writes are read back out of the file (`checkSpans`). -/
def orderedAnswers (cert ob : Json) (values : List String) :
    Except String (List (String × String) × (String → Option Rat)) := do
  let tname := fieldStr ob "table"
  let some t := (fieldArr cert "tables").find? (fun x => fieldStr x "table" == tname)
    | throw s!"{tname} is no table of the certificate"
  let axes := fieldArr t "axes"
  let decides := (fieldArr t "decides").toList.filterMap str
  unless axes.size == 1 && decides == [fieldStr ob "column"] do
    throw s!"{tname} is no table of one column of the block's enum that decides {fieldStr ob "column"}"
  unless ((fieldArr (axes[0]!) "coords").toList.filterMap str) == values do
    throw s!"the axis of {tname} is not the values of the block's enum"
  let kind := fieldStr ob "kind"
  let order : List String :=
    if kind == "enum" then
      ((field ((field cert "enums").getD Json.null) (fieldStr ob "enum")) >>= arr).map (·.toList.filterMap str) |>.getD []
    else []
  if kind == "enum" then
    unless !order.isEmpty && ((fieldArr ob "order").toList.filterMap str) == order &&
        ((fieldArr cert "ordered").toList.filterMap str).contains (fieldStr ob "enum") do
      throw s!"{fieldStr ob "enum"} is not an ordered enum of the certificate"
  else unless kind == "number" do
    throw s!"an order of kind `{kind}` cannot be read"
  let rows := (fieldArr t "rows").toList
  let mut shown : List (String × String) := []
  let mut keys : List (String × Rat) := []
  for (v, i) in values.zip (List.range values.length) do
    let takes : Json → Bool := fun row =>
      (((fieldArr row "accepts")[0]? >>= arr).map (fun a => (a.toList.filterMap nat).contains i)).getD false
    let some row := rows.find? takes | continue
    if kind == "enum" then
      let some w := (fieldArr row "produces")[0]? >>= str | continue
      let some k := order.idxOf? w
        | throw s!"{tname} writes {w} for {v}, which is not a value of {fieldStr ob "enum"}"
      shown := shown ++ [(v, w)]
      keys := keys ++ [(v, ((k : Nat) : Rat))]
    else
      let some e := ((fieldArr row "writes")[0]?).bind (fun w => field w "expr") | continue
      let some x := (if (field e "num").isSome then field e "value" >>= optRat else none)
        | throw s!"{tname} writes something other than a number for {v}"
      shown := shown ++ [(v, ratText x)]
      keys := keys ++ [(v, x)]
  let keysF := keys
  return (shown, fun v => keysF.lookup v)

/-- **A `prerequisites` block, held to what it claims** (rulec's §15.222). Its graph is the
    certificate's account of the block, as each value's expression is; what is held to it are the
    claims. Each value of the enum is said once — on `start`, on its own `requires` line, or among
    those no case can take — and named nowhere else. `topological` puts every value after every value
    it may need (`Prereq.topoOk`, so `Prereq.no_cycle`); `reached` takes the values one after another
    from `start` (`Prereq.reachedOk`, so `Prereq.reached_holds`), and it and those no case can take are
    the values of the enum, each once (so `Prereq.never_held` speaks of every other value). Where an
    `ordered by` names a table, each value's answer is read off the table's rows here, held to the
    certificate's `values`, and kept along every edge into a value a case can take (`Prereq.orderOk`,
    so `Prereq.order_along`). -/
def checkPrerequisites (cert p : Json) (r : Report) : Report := Id.run do
  let name := fieldStr p "name"
  let over := fieldStr p "over"
  let at_ := s!"prerequisites {name}"
  let some values := ((field ((field cert "enums").getD Json.null) over) >>= arr).map (·.toList.filterMap str)
    | return r.fail s!"{at_}: {over} is not an enum of the certificate"
  let start := (fieldArr p "start").toList.filterMap str
  let never := (fieldArr p "never").toList.filterMap str
  let reqJ := (field p "requires").getD Json.null
  let groupsOf : Json → Option (List (List String)) := fun gj => do
    let a ← arr gj
    a.toList.mapM (fun (g : Json) => (arr g).map (fun (xs : Array Json) => xs.toList.filterMap str))
  let some requires := ((objPairs reqJ).mapM (fun (p : String × Json) => (groupsOf p.2).map (p.1, ·)) :
      Option (List (String × List (List String))))
    | return r.fail s!"{at_}: a `requires` line cannot be read"
  let said := start ++ requires.map (·.1) ++ never
  if !said.all values.contains then
    return r.fail s!"{at_}: a value it names is not a value of {over}"
  if !sameOnce said values then
    return r.fail s!"{at_}: the values of {over} are not each said once (on start, in requires or in never)"
  for ((v, gs) : String × List (List String)) in requires do
    if gs.isEmpty || gs.any List.isEmpty then
      return r.fail s!"{at_}: {v} has an empty group"
    if !gs.all (·.all values.contains) then
      return r.fail s!"{at_}: {v} needs a value that is not a value of {over}"
  let G : Prereq.Graph := { start := start, requires := requires, never := never }
  let topo := (fieldArr p "topological").toList.filterMap str
  if !sameOnce topo values then
    return r.fail s!"{at_}: `topological` is not the values of {over}, each once"
  if !Prereq.topoOk G topo then
    return r.fail s!"{at_}: `topological` puts a value before one it needs"
  let reached := (fieldArr p "reached").toList.filterMap str
  if !sameOnce (reached ++ never) values then
    return r.fail s!"{at_}: `reached` and `never` are not the values of {over}, each once"
  if !Prereq.reachedOk G reached then
    return r.fail s!"{at_}: `reached` names a value before a value of each group it needs"
  let edges := (requires.map (fun (p : String × List (List String)) => (p.2.map List.length).sum)).sum
  let mut kept := ""
  let ob := (field p "ordered_by").getD Json.null
  if !ob.isNull then
    match orderedAnswers cert ob values with
    | .error why => return r.fail s!"{at_}: {why}"
    | .ok (shown, key) =>
      let stated : List (String × String) :=
        (objPairs ((field ob "values").getD Json.null)).filterMap (fun (v, j) => (str j).map (v, ·))
      let same := shown.length == stated.length && shown.all (fun (v, w) =>
        match stated.lookup v with
        | some w' => w == w' || (match ratOfString w, ratOfString w' with
            | some a, some b => a == b
            | _, _ => false)
        | none => false)
      if !same then
        return r.fail s!"{at_}: the answers of {fieldStr ob "table"} are not the ones its rows write"
      if !Prereq.orderOk G key then
        return r.fail s!"{at_}: {fieldStr ob "table"} puts a value before a value it needs"
      let had := (requires.map (fun (p : String × List (List String)) =>
        (p.2.map (fun g => (g.filter (fun x => !never.contains x)).length)).sum)).sum
      kept := s!", and {fieldStr ob "table"} keeps its order along {had} edges"
  return r.say s!"  prerequisites {name} over {over}: {values.length} values and {edges} edges with no cycle, {reached.length} reached from start and {never.length} no case can take{kept}"

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
    -- The shape this program reads is version 1. A certificate of another shape says so in
    -- `v`, and is refused here rather than read as something it is not.
    if (field cert "v").isSome && fieldNat cert "v" != some 1 then
      IO.println "FAILED: the certificate's format version (`v`) is not 1, the one this program reads"
      return 1
    let mut r : Report := {}
    r := r.say s!"{fieldStr cert "rule"} ({fieldStr cert "rulec"}), re-checked against the Lean proofs"
    let (r', worked) := checkValues cert r
    -- The currency a money column is written in, for reading an answer cell back (§15.196).
    let curOf : String → Option String := fun n =>
      match (objPairs ((field cert "types").getD Json.null)).find? (fun p => p.1 == n) with
      | some (_, j) => match str j with
        | some ty => if ty.startsWith "money[" then
            (((ty.drop 6).takeWhile (fun c => c != ',' && c != ']')).toString.trimAscii.toString : String) |> some
          else none
        | none => none
      | none => none
    let reachOf := worked.reachOf
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
    -- The facts of a table's linear model, each built again from where the certificate
    -- says it comes from (§15.141): a `derive`'s expression, or a `define`'s whose expression is
    -- linear (§15.209), one end of a declared range, a `constraint`. What cannot be built leaves
    -- the table unread rather than a fact missing.
    let typesJson := (field cert "types").getD Json.null
    let types : String → Option Ty := fun n =>
      (objPairs typesJson).find? (fun p => p.1 == n) |>.bind (fun p => str p.2 >>= tyOfString)
    let valuesJ := fieldArr cert "values"
    let consJ := fieldArr cert "constraints"
    let rangeEnd : String → Bool → Option Rat := fun n hi =>
      match (objPairs ((field cert "ranges").getD Json.null)).find? (fun p => p.1 == n) with
      | some (_, v) => (arr v) >>= (fun a => a[if hi then 1 else 0]?) >>= optRat
      | none => none
    -- The equation of a value the rule computes, `n − expr ≤ 0` (`le`) or `≥ 0`, built again from
    -- the value's own expression: a `derive`'s, or a `define`'s whose expression is linear (rulec's
    -- §15.209). `of` is what the document calls the value; one it calls otherwise is not read.
    let equation : String → String → Json → Option (List (String × Rat) × Rat × Bool) := fun of n f => do
      let v ← valuesJ.find? (fun v => fieldStr v "name" == n)
      guard (!((field v "of").isSome && fieldStr v "of" != of))
      let want ← field v "type" >>= str >>= tyOfString
      let e ← field v "expr" >>= exprOfJson
      let (terms, k) ← linOf types want e
      let le ← field f "le" >>= boolOf
      let d := (n, (1 : Rat)) :: terms.map (fun t => (t.1, -t.2))
      if le then some (d, -k, false) else some (d.map (fun t => (t.1, -t.2)), k, false)
    let factOf : Json → Option (List (String × Rat) × Rat × Bool) := fun f =>
      match field f "derive" >>= str, field f "define" >>= str with
      | some n, _ => equation "derive" n f
      | none, some n => equation "define" n f
      | none, none =>
        match field f "range" >>= str with
        | some n => do
            let hi ← field f "hi" >>= boolOf
            -- A value the rule computes is bounded by the interval worked out here from its
            -- expression, not by the document's word for it (rulec's §15.195); an input by its range.
            -- A column a table writes numbers into, by the hull of what its rows write (§15.196).
            let e ← match reachOf n with
              | some I => some (if hi then I.2 else I.1)
              | none => match worked.writtenOf n with
                | some H => some (if hi then H.2 else H.1)
                | none => rangeEnd n hi
            if hi then some ([(n, 1)], -e, false) else some ([(n, -1)], e, false)
        | none => do
            let i ← fieldNat f "constraint"
            let k ← consJ[i]?
            -- Each side the names it adds up (rulec's §15.216).
            let ls := sideNames k "left"
            let rs := sideNames k "right"
            guard (!ls.isEmpty && !rs.isEmpty)
            let d : List (String × Rat) := ls.map (fun n => (n, (1 : Rat))) ++ rs.map (fun n => (n, (-1 : Rat)))
            match fieldStr k "op" with
            | "<=" => some (d, 0, false)
            | "<" => some (d, 0, true)
            | ">=" => some (d.map (fun t => (t.1, -t.2)), 0, false)
            | ">" => some (d.map (fun t => (t.1, -t.2)), 0, true)
            | _ => none
    -- The days a date input takes from a koyomi file (rulec's §15.174): the document's word,
    -- as the ranges are, tied to the koyomi file by the SHA-256 the certificate writes beside them.
    let daysJ := (field cert "days").getD Json.null
    let daysOf : String → Option (List Rat) := fun n => do
      let d ← field daysJ n
      (fieldArr d "days").toList.mapM optRat
    -- The kind of every column, worked out from `inputs`, `walks` and `values` (§15.196); a
    -- certificate from before it carried them leaves the kinds its own word.
    let kindsKnown := (field cert "inputs").isSome
    if !kindsKnown then
      r := r.state "the kinds of the columns: the certificate does not say what the rule's inputs are"
    -- What a `max` or a `min` answers, and the answer of a fold the items below read (rulec's
    -- §15.219, §15.220): their ranges are worked out from the column's interval and the `empty`
    -- value, or from the values the fold answers with, rather than declared, and each is held to
    -- what it is worked out from here (`extreme_mem_stated`, `foldAnswer_mem_stated`).
    r := checkExtremes cert worked r
    r := checkFoldAnswers cert worked r
    let mut tables : Array ReadTable := #[]
    for tj in fieldArr cert "tables" do
      if kindsKnown then
        for a in fieldArr tj "axes" do
          let col := fieldStr a "column"
          let want := kindOf cert col
          if fieldStr a "kind" != want then
            r := r.fail s!"{fieldStr tj "table"}: {col} is a column of kind `{want}`, and the certificate calls it `{fieldStr a "kind"}`"
      -- An input the rule declares optional: its type, as the certificate states it, ends in `?`.
      let optionalOf : String → Bool := fun n => (fieldStr ((field cert "types").getD Json.null) n).endsWith "?"
      -- The order of the column's enum, where it is an ordered one (rulec's §15.210): the values
      -- as `enums` lists them, for an enum `ordered` names.
      let orderOf : String → List String := fun n =>
        let ty := (fieldStr ((field cert "types").getD Json.null) n).dropRightWhile (· == '?')
        if ((fieldArr cert "ordered").toList.filterMap str).contains ty then
          ((field ((field cert "enums").getD Json.null) ty).bind arr).map (·.toList.filterMap str) |>.getD []
        else []
      match readTable reachOf groups declaredRange factOf daysOf (fieldArr cert "tables") (truthOfValue cert worked) optionalOf orderOf tj with
      | none => r := r.fail s!"{fieldStr tj "table"}: this program cannot read the table"
      | some t =>
        r := checkAbove tj (fieldArr cert "tables") r
        r := checkTable t r
        r := checkSteps t (columnStep cert []) r
        let (r', enumAxes) := checkEnumAxes cert t r
        r := r'
        if enumAxes > 0 then
          r := r.say s!"    {enumAxes} axes of an enum's input run over the values it takes"
        tables := tables.push t
    r := checkContracts cert r
    r := checkMachine cert tables r
    -- Which values need which before them (rulec's §15.222): no cycle, every value but those no
    -- case can take reached from `start`, and an `ordered by` table's order kept along the edges
    -- (`Prereq.no_cycle`, `Prereq.reached_holds`, `Prereq.never_held`, `Prereq.order_along`).
    for p in fieldArr cert "prerequisites" do
      r := checkPrerequisites cert p r
    -- The claims of `chances` over the random inputs (rulec's §15.224): `tools/recheck.py` adds the
    -- boxes of the random axes up again and holds each claim to them; no theorem here says yet that
    -- the boxes tile the axes or that the chance follows from the rows, so each claim is named as
    -- stated rather than proved.
    if let some ch := field cert "chances" then
      for c in fieldArr ch "claims" do
        let t := (fieldNat c "chances").getD 0
        let row := (fieldNat c "row").getD 0
        r := r.state s!"chances {t + 1} row {row}: the chance of its outputs over the random inputs"
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
          let (r', n, a) := checkSpans t lines r curOf
          r := r'; read := read + n; apart := apart + a
        let (r', mread) := checkMachineSpans cert lines r curOf
        r := r'; read := read + mread
        let rest := if apart > 0 then s!", {apart} rows written in an applied rule" else ""
        if apart > 0 then
          r := r.state s!"{apart} rows written in an applied rule, and read back in its own certificate"
        r := r.say s!"  the digest is {path}'s, and {read} cells are read back out of it{rest}"
        -- Each contract's reading is tied to its text the same way (§15.142): the file is
        -- named relative to the rule, and its digest is in the certificate.
        let dir : System.FilePath := match (System.FilePath.mk path).parent with
          | some d => if d.toString.isEmpty then "." else d
          | none => "."
        for k in fieldArr cert "contracts" do
          let there := dir / fieldStr k "file"
          let got ← try pure (some (← IO.FS.readBinFile there)) catch _ => pure none
          match got with
          | none => r := r.state s!"contract {fieldStr k "shape"} is held to no text: {there} cannot be read"
          | some cb =>
            if Sha256.hex cb == fieldStr k "sha256" then
              r := r.say s!"  the digest of contract {fieldStr k "shape"} is {there}'s"
            else
              r := r.fail s!"contract {fieldStr k "shape"} is read from another text than {there}"
    for l in r.lines do IO.println l
    if r.bad.isEmpty then
      if r.stated.isEmpty then
        IO.println "OK: every claim this program states was proved, by the theorems of RulecCert."
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
