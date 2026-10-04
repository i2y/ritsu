/-
  A flow run against a scenario — the execution's input and the answers the calls get, in order —
  as dandori's reference interpreter runs it for Temporal (`crates/dandori/src/interp.rs`, the view
  in which an error is known by its kind). What a run says is what it went through (every
  statement, arm, answer, handler and round, in order), how it ended, and the state each case's
  record says at the end; `ritsu-model` prints exactly that and the Rust test compares it with
  `interp::run_visits` and `interp::cases_at_end`.

  The run is bounded by a fuel that no scenario comes near; running out is an error, never a
  quiet answer. The functions that walk the statements are `partial`: nothing is proved about this
  file — the theorems of `DandoriCore.Sound` are about the check — and what holds it to dandori's
  interpreter is the comparison.
-/
import DandoriCore.Syntax

namespace DandoriCore
open Lean (Json JsonNumber)

/-! ## Values -/

def lookupVar (vars : List (String × Json)) (x : String) : Json :=
  match vars.find? (fun p => p.1 == x) with
  | some (_, v) => v
  | none => Json.null

/-- A value put into a string: a string as it is, nothing for null, anything else as JSON. -/
def valueText (v : Json) : String :=
  match v with
  | .str s => s
  | .null => ""
  | other => other.compress

mutual
  /-- The value of an expression over the variables. A field that is not there reads as null. -/
  def eval (vars : List (String × Json)) : Expr → Json
    | .var x fs => fs.foldl (fun v f => v.getObjValD f) (lookupVar vars x)
    | .lit v => v
    | .record fs => Json.mkObj (evalFields vars fs)
    | .list items => Json.arr (evalList vars items).toArray
    | .interp parts => Json.str (String.join ((evalList vars parts).map valueText))
  def evalFields (vars : List (String × Json)) : List (String × Expr) → List (String × Json)
    | [] => []
    | (f, e) :: rest => (f, eval vars e) :: evalFields vars rest
  def evalList (vars : List (String × Json)) : List Expr → List Json
    | [] => []
    | e :: rest => eval vars e :: evalList vars rest
end

/-- RFC 3339 in UTC, `Z` at the end, as `render::is_timestamp` reads it. -/
def isTimestamp (s : String) : Bool :=
  let b := s.toList
  let digits (lo hi : Nat) : Bool := (List.range (hi - lo)).all (fun i => (b[lo + i]?.map Char.isDigit).getD false)
  let atIs (i : Nat) (c : Char) : Bool := b[i]? == some c
  if b.length < 20 || !digits 0 4 || !atIs 4 '-' || !digits 5 7 || !atIs 7 '-' || !digits 8 10 || !atIs 10 'T' ||
     !digits 11 13 || !atIs 13 ':' || !digits 14 16 || !atIs 16 ':' || !digits 17 19 then false
  else
    let rest := b.drop 19
    match rest.getLast? with
    | some 'Z' =>
      let frac := rest.dropLast
      frac.isEmpty || (frac.length > 1 && frac.head? == some '.' && frac.tail.all Char.isDigit)
    | _ => false

/-- A JSON number that is a whole number. -/
def numInt (n : JsonNumber) : Option Int :=
  let p : Int := 10 ^ n.exponent
  if n.mantissa % p == 0 then some (n.mantissa / p) else none

def inRange (rg : Option Range) (i : Int) : Bool :=
  match rg with
  | none => true
  | some r => r.lo.all (fun lo => decide (lo ≤ i)) && r.hi.all (fun hi => decide (i ≤ hi))

/-- Whether a value is a well-formed value of the type, in the range (`render::value_fits`). -/
def fits (recs : List (List Field)) : Nat → Json → Ty → Option Range → Bool
  | 0, _, _, _ => false
  | n + 1, v, t, rg =>
    match t with
    | .list inner => match v with
      | .arr xs => xs.all (fun x => fits recs n x inner rg)
      | _ => false
    | .opt inner => v.isNull || fits recs n v inner rg
    | .json => true
    | .str => match v with
      | .str _ => true
      | _ => false
    | .bool => match v with
      | .bool _ => true
      | _ => false
    | .timestamp => match v with
      | .str s => isTimestamp s
      | _ => false
    | .int => match v with
      | .num k => match numInt k with
        | some i => inRange rg i
        | none => false
      | _ => false
    | .enum vals => match v with
      | .str s => vals.contains s
      | _ => false
    | .record r => match v with
      | .obj _ => (recs[r]?.getD []).all (fun fd => match v.getObjVal? fd.name with
        | .ok x => fits recs n x fd.ty fd.range
        | .error _ => match fd.ty with
          | .opt _ => true
          | .json => true
          | _ => false)
      | _ => false

/-! ## A run -/

/-- A parallel round's failure, kept until every round is done: a task's error nothing handled,
    or a deliberate end. -/
inductive Pending where
  | task (kind cause : String)
  | fail (error : String) (cause : Json)
  deriving Inhabited

structure St where
  vars : List (String × Json) := []
  answers : List Json
  taken : Nat := 0
  visits : Array String := #[]
  ended : Option Json := none
  error : Option String := none
  inOnFailure : Bool := false
  inOnCancel : Bool := false
  cancelled : Bool := false
  /-- how many `for … in parallel` rounds run around the statement -/
  parDepth : Nat := 0
  pending : Option Pending := none
  /-- What dandori reads an answer as, by the answer's place and the callee that takes it, where it
      reads one before the flow sees it: `{"value": …}`, or `{"error": …, "cause": …}` for one Jev is
      less sure of than the task asks. -/
  reads : List ((Nat × Callee) × Json) := []
  deriving Inhabited

inductive Ctl where
  | next
  | brk
  | stop
  deriving DecidableEq, Inhabited

def St.visit (st : St) (v : String) : St := { st with visits := st.visits.push v }
def St.set (st : St) (x : String) (v : Json) : St := { st with vars := (x, v) :: st.vars }
def failJson (error : String) (cause : Json) : Json :=
  Json.mkObj [("fail", Json.mkObj [("error", Json.str error), ("cause", cause)])]
/-- End the run as failed; inside a parallel round, end the round and keep the failure. -/
def St.failWith (st : St) (error : String) (cause : Json) : St :=
  if st.parDepth > 0 then { st with pending := some (.fail error cause) }
  else { st with ended := some (failJson error cause) }
def St.halted (st : St) : Bool := st.ended.isSome || st.error.isSome || st.pending.isSome || st.cancelled
def St.oops (st : St) (e : String) : St := { st with error := some e }

/-- `interp::matches_error`. -/
def matchesError (names : List String) (err : String) : Bool :=
  names.any (fun n => n == err || (n == "States.ALL" && err != "States.Runtime" && err != "States.DataLimitExceeded"))

def Flow.calleeName (f : Flow) : Callee → String
  | .task t => ((f.tasks[t]?).map (·.name)).getD ""
  | .rule r => ((f.rules[r]?).map (·.name)).getD ""

def Flow.retriers (f : Flow) : Callee → List Retrier
  | .task t => ((f.tasks[t]?).map (·.retriers)).getD []
  | .rule r => ((f.rules[r]?).map (·.retriers)).getD []

/-- What the callee answers, and its range. -/
def Flow.answerTy (f : Flow) : Callee → Ty × Option Range
  | .task t => match f.tasks[t]? with
    | some tk => (tk.result.getD .json, tk.range)
    | none => (.json, none)
  | .rule r => (.record (((f.rules[r]?).map (·.outputs)).getD 0), none)

/-- What a run that ends with no outputs ends with: null, or `{}` for a flow that implements a
    service. -/
def Flow.noOutput (f : Flow) : Json := if f.service then Json.mkObj [] else Json.null

/-- How a call's tries came out. -/
inductive Tried where
  | ok (v : Json)
  | err (kind cause : String)
  | cancel
  | stop

/-- Take answers until the call is settled: answered, failed with an error no retrier takes
    again, or cancelled. The first retrier whose errors match decides. -/
def tries (site : Nat) (callee : Callee) (cname : String) (retr : List Retrier) : Nat → List Nat → St → Tried × St
  | 0, _, st => (.stop, st.oops "a call tried more often than any scenario has answers")
  | fuel + 1, counts, st =>
    match st.answers with
    | [] => (.stop, st.oops s!"the scenario has no answer for call {st.taken + 1} ({cname})")
    | ans :: rest =>
      let place := st.taken
      let st : St := { st with answers := rest, taken := st.taken + 1 }
      let isCancel := (ans.getObjVal? "cancel").toOption.isSome
      let okv := (ans.getObjVal? "ok").toOption
      let kind := if isCancel then "cancel" else if okv.isSome then "ok"
        else ((ans.getObjValD "error").getStr?.toOption.getD "failure")
      let st := st.visit s!"r{site}:{kind}"
      if isCancel then (.cancel, { st with cancelled := true }) else
      match okv with
      | some v =>
        match st.reads.find? (fun p => p.1.1 == place && p.1.2 == callee) with
        | some (_, r) =>
          match (r.getObjVal? "error").toOption with
          | some e => (.err (e.getStr?.toOption.getD "") ((r.getObjValD "cause").getStr?.toOption.getD ""), st)
          | none => (.ok (r.getObjValD "value"), st)
        | none => (.ok v, st)
      | none =>
        match (retr.zipIdx).find? (fun (r, _) => matchesError r.names kind) with
        | some (r, i) =>
          if counts.getD i 0 < r.max then tries site callee cname retr fuel (counts.set i (counts.getD i 0 + 1)) st
          else (.err kind "scripted", st)
        | none => (.err kind "scripted", st)

def fuelLimit : Nat := 1000000

/-- The first arm that takes the value: `none` for an absent value, `some y` for one that is
    there, a value it names. -/
def pick (v : Json) : List Arm → Nat → Option (Nat × Arm)
  | [], _ => none
  | (.mk values isNone someName body) :: rest, i =>
    let named := values.any (fun x => match v with
      | .str s => s == x
      | .bool b => toString b == x
      | _ => false)
    if (isNone && v.isNull) || (someName.isSome && !v.isNull) || named then some (i, .mk values isNone someName body)
    else pick v rest (i + 1)

mutual
  partial def block (f : Flow) : Nat → List Stmt → St → Ctl × St
    | 0, _, st => (.stop, st.oops "the run went deeper than any flow does")
    | _ + 1, [], st => (.next, st)
    | fuel + 1, s :: rest, st =>
      let (c, st) := stmt f fuel s st
      if c != .next then (c, st)
      else if st.halted then (.stop, st)
      else block f fuel rest st

  partial def stmt (f : Flow) : Nat → Stmt → St → Ctl × St
    | 0, _, st => (.stop, st.oops "the run went deeper than any flow does")
    | fuel + 1, s, st =>
      if st.taken > 20000 then (.stop, st.oops "the run took more than 20000 steps") else
      match s with
      | .pass site => (.next, st.visit s!"s{site}")
      | .brk site => (.brk, st.visit s!"s{site}")
      | .wait site => (.next, st.visit s!"s{site}")
      | .succeed site fields =>
        let st := st.visit s!"s{site}"
        let out := if fields.isEmpty then f.noOutput else Json.mkObj (evalFields st.vars fields)
        (.stop, { st with ended := some (Json.mkObj [("succeed", out)]) })
      | .fail site error cause _ =>
        let st := st.visit s!"s{site}"
        (.stop, st.failWith error ((cause.map (eval st.vars)).getD Json.null))
      | .assign site x e =>
        let st := st.visit s!"s{site}"
        (.next, st.set x (eval st.vars e))
      | .forEach site line x list max body result parallel =>
        let st := st.visit s!"s{site}"
        let items := match eval st.vars list with
          | .arr xs => xs.toList
          | _ => []
        if items.length > max then
          (.stop, st.failWith "Dandori.TooManyItems" (Json.str s!"line {line}: the list has more than {max} items"))
        else match parallel with
        | some locals =>
          match parRounds f fuel site x body result locals items 0 #[] none st with
          | (.stop, _, st) => (.stop, st)
          | (_, out, st) =>
            let st := locals.foldl (fun st l => st.set l Json.null) st
            let st := match result with
              | some (r, _) => st.set r (Json.arr out)
              | none => st
            (.next, st)
        | none =>
          match rounds f fuel site x body result items 0 #[] st with
          | (.stop, _, _, st) => (.stop, st)
          | (_, broke, out, st) =>
            let st := if broke then st else st.visit s!"d{site}"
            let st := match result with
              | some (r, _) => st.set r (Json.arr out)
              | none => st
            (.next, st)
      | .repeat site times body =>
        let st := st.visit s!"s{site}"
        repeatRounds f fuel site body times 0 st
      | .matchOn site line e shown arms =>
        let st := st.visit s!"s{site}"
        let v := eval st.vars e
        match pick v arms 0 with
        | some (i, .mk _ _ someName body) =>
          let st := st.visit s!"a{site}.{i}"
          let st := match someName with
            | some n => st.set n v
            | none => st
          block f fuel body st
        | none => (.stop, st.failWith "Dandori.UnexpectedValue" (Json.str s!"line {line}: {shown} took a value that no arm names"))
      | .call site line target callee handlers =>
        let st := st.visit s!"s{site}"
        let cname := f.calleeName callee
        match tries site callee cname (f.retriers callee) fuelLimit ((f.retriers callee).map (fun _ => 0)) st with
        | (.stop, st) => (.stop, st)
        | (.cancel, st) => (.stop, st)
        | (.ok v, st) =>
          let var := match target with
            | some (.letVar x) => some x
            | some (.case c) => (f.cases[c]?).map Case.name
            | none => none
          match var with
          | none => (.next, st)
          | some x =>
            let (ty, rg) := f.answerTy callee
            if !fits f.records 64 v ty rg then
              (.stop, st.failWith "Dandori.BadResponse" (Json.str s!"line {line}: the answer from {cname} does not have the declared shape"))
            else
              let monitor : Option (List String × String) := match target with
                | some (.case c) => match (f.monitors.find? (fun (p : Nat × List String) => p.1 == site)), f.cases[c]? with
                  | some (_, allowed), some cs => some (allowed, cs.stateField)
                  | _, _ => none
                | _ => none
              match monitor with
              | some (allowed, field) =>
                let stateNow := ((v.getObjValD field).getStr?.toOption).getD ""
                if !allowed.contains stateNow then
                  (.stop, st.failWith "Dandori.UnexpectedState"
                    (Json.str s!"line {line}: {cname} answered with a state the machine does not lead to here (expected one of {", ".intercalate allowed})"))
                else (.next, st.set x v)
              | none => (.next, st.set x v)
        | (.err kind cause, st) =>
          match handlers.zipIdx.find? (fun (h, _) => match h with
              | .mk errors _ => errors.any (HErr.hits kind)) with
          | some (.mk _ body, j) => block f fuel body (st.visit s!"h{site}.{j}")
          | none => raise f fuel kind cause (st.visit s!"u{site}")

  /-- The rounds of a `for`, one item after another: whether it ended or stopped, whether a
      `break` ended it, what the rounds yielded. -/
  partial def rounds (f : Flow) : Nat → Nat → String → List Stmt → Option (String × Expr) → List Json → Nat →
      Array Json → St → Ctl × Bool × Array Json × St
    | 0, _, _, _, _, _, _, out, st => (.stop, false, out, st.oops "the run went deeper than any flow does")
    | _ + 1, _, _, _, _, [], _, out, st => (.next, false, out, st)
    | fuel + 1, site, x, body, result, item :: rest, i, out, st =>
      let st := (st.visit s!"o{site}.{i}").set x item
      let (c, st) := block f fuel body st
      let out := match c, result with
        | .next, some (_, y) => if st.ended.isNone && st.error.isNone && st.pending.isNone then out.push (eval st.vars y) else out
        | _, _ => out
      match c with
      | .brk => (.next, true, out, st)
      | .stop => (.stop, false, out, st)
      | .next => rounds f fuel site x body result rest (i + 1) out st

  /-- The rounds of a `for … in parallel`, one after another in the list's order: every round runs
      to its end; then, if any failed, the loop fails as the first of them did. A round does not
      see another round's variables. -/
  partial def parRounds (f : Flow) : Nat → Nat → String → List Stmt → Option (String × Expr) → List String →
      List Json → Nat → Array Json → Option Pending → St → Ctl × Array Json × St
    | 0, _, _, _, _, _, _, _, out, _, st => (.stop, out, st.oops "the run went deeper than any flow does")
    | fuel + 1, site, _, _, _, locals, [], _, out, first, st =>
      let st := locals.foldl (fun st l => st.set l Json.null) st
      match first with
      | some p => match raisePending f fuel p st with
        | (c, st) => (c, out, st)
      | none => (.next, out, st.visit s!"d{site}")
    | fuel + 1, site, x, body, result, locals, item :: rest, i, out, first, st =>
      let st := locals.foldl (fun st l => st.set l Json.null) st
      let st := (st.visit s!"o{site}.{i}").set x item
      let st := { st with parDepth := st.parDepth + 1 }
      let (_, st) := block f fuel body st
      let st := { st with parDepth := st.parDepth - 1 }
      if st.error.isSome || st.cancelled then (.stop, out, st) else
      let p := st.pending
      let st := { st with pending := none }
      match p with
      | some p => parRounds f fuel site x body result locals rest (i + 1) out (first.orElse (fun _ => some p)) st
      | none =>
        let out := match result with
          | some (_, y) => out.push (eval st.vars y)
          | none => out
        parRounds f fuel site x body result locals rest (i + 1) out first st

  /-- A failure the rounds kept, raised once they are done. -/
  partial def raisePending (f : Flow) : Nat → Pending → St → Ctl × St
    | 0, _, st => (.stop, st.oops "the run went deeper than any flow does")
    | fuel + 1, p, st =>
      if st.parDepth > 0 then (.stop, { st with pending := some p }) else
      match p with
      | .fail error cause => (.stop, { st with ended := some (failJson error cause) })
      | .task kind cause => raise f fuel kind cause st

  /-- The rounds of a `repeat`: a `break` ends it as the next statement's business. -/
  partial def repeatRounds (f : Flow) : Nat → Nat → List Stmt → Nat → Nat → St → Ctl × St
    | 0, _, _, _, _, st => (.stop, st.oops "the run went deeper than any flow does")
    | fuel + 1, site, body, times, n, st =>
      if n < times then
        let (c, st) := block f fuel body (st.visit s!"o{site}.{n}")
        match c with
        | .brk => (.next, st)
        | .stop => (.stop, st)
        | .next => repeatRounds f fuel site body times (n + 1) st
      else (.next, st.visit s!"d{site}")

  /-- A task's error nothing took: `on failure` runs, unless the run is in it or in `on cancel`
      already, and the run fails as the error did. -/
  partial def raise (f : Flow) : Nat → String → String → St → Ctl × St
    | 0, _, _, st => (.stop, st.oops "the run went deeper than any flow does")
    | fuel + 1, kind, cause, st =>
      if st.parDepth > 0 then (.stop, { st with pending := some (.task kind cause) }) else
      match f.onFailure with
      | some blk =>
        if !st.inOnFailure && !st.inOnCancel then
          let st := { st with inOnFailure := true }
          let st := st.set "dd_error" (Json.mkObj [("Error", Json.str kind), ("Cause", Json.str cause)])
          let st := st.visit "of"
          match block f fuel blk st with
          | (.stop, st) => (.stop, st)
          | (_, st) =>
            if st.ended.isNone then (.stop, (st.visit "ofe").failWith kind (Json.str cause))
            else (.stop, st)
        else (.stop, st.failWith kind (Json.str cause))
      | none => (.stop, st.failWith kind (Json.str cause))
end

/-- **One run**: the input checked against its declared shape, the flow, and `on cancel` when a
    cancellation stopped it. Err when the scenario does not fit the flow (an answer missing). -/
def run (f : Flow) (input : Json) (answers : List Json) (reads : List ((Nat × Callee) × Json)) :
    Except String St := Id.run do
  let mut st : St := { answers := answers, reads := reads }
  let mut ok := true
  for fd in f.inputs do
    let v := input.getObjValD fd.name
    if !fits f.records 64 v fd.ty fd.range then ok := false
    st := st.set fd.name v
  if !ok then
    st := st.failWith "Dandori.BadInput" (Json.str "the execution's input does not have the declared shape")
  else
    let (c, st1) := block f fuelLimit f.flow st
    st := st1
    if c == .next && st.ended.isNone then
      st := { (st.visit "fe") with ended := some (Json.mkObj [("succeed", f.noOutput)]) }
    if st.cancelled && st.error.isNone then
      st := { st with cancelled := false, inOnCancel := true }
      match f.onCancel with
      | some blk =>
        st := st.visit "oc"
        let (_, st2) := block f fuelLimit blk st
        st := st2
        if st.ended.isNone then st := st.visit "oce"
      | none => pure ()
      if st.ended.isNone then st := { st with ended := some (Json.mkObj [("cancel", Json.null)]) }
  match st.error with
  | some e => return .error e
  | none => return .ok st

/-- What `ritsu-model` prints for a run: what it went through, how it ended, and each case's
    state at the end (null for a case it did not start). -/
def runJson (f : Flow) (st : St) : Json :=
  Json.mkObj [
    ("visits", Json.arr (st.visits.map Json.str)),
    ("end", st.ended.getD Json.null),
    ("cases", Json.mkObj (f.cases.map (fun c => (c.name, (lookupVar st.vars c.name).getObjValD c.stateField))))]

end DandoriCore
