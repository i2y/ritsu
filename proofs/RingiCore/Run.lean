/-
  The comparison's side of the model (`ritsu-model ringi`): a vector of the reference interpreter
  run through `glue_start` or `glue_next`, with what the request's own code answers given by the
  test a question at a time.

  The model asks each question it needs as it comes to it. An answer the test has not given yet is
  a refusal the model does not make itself (`unread`, `need`, the question as its place): the line
  is answered with the question, and the test gives the line again with the answer added.
-/
import RingiCore.Glue
import RingiCore.Json

open Lean

namespace RingiCore.Run

open RingiCore.Json

/-- What the test gives the model of a request: the data of its glue and its `submitted by`. -/
structure Request where
  spec : Spec
  sub : Option Submitters

def readGroup (j : Json) : R Group := do
  match ← tagged j with
  | ("everyone", v) => return .everyone (← optStr v "within")
  | ("team", v) => return .team (← str v "id")
  | ("position", v) => return .position (← str v "position") (← bool v "higher") (← optStr v "within")
  | ("person", v) => return .person (← str v "id")
  | (s, _) => throw s!"group {s}"

def readRequest (j : Json) : R Request := do
  let sub ← match opt? j "submitters" with
    | none => pure none
    | some s => do
      let gs ← (← arr s "groups").mapM readGroup
      let xs ← (← arr s "except").mapM readGroup
      let file ← str s "file"
      let shown ← str s "shown"
      pure (some ({ groups := gs, except_ := xs, file := file, shown := shown } : Submitters))
  return { spec := ← readSpec j, sub := sub }

/-- The refusal that stands for an answer not given yet. -/
def need (q : Json) : Refusal :=
  { refused := .unread, text := "need", text_ja := "need", at_ := some q.compress }

def unreadable (q : Json) (e : String) : Refusal :=
  { refused := .unread, text := s!"the answer to {q.compress} cannot be read: {e}", text_ja := "", at_ := none }

/-- The answer the test gave to the question, read with `read`. -/
def asked {α} (answers : List (Json × Json)) (q : Json) (read : Json → R (Except Refusal α)) :
    Except Refusal α :=
  match answers.find? (fun (k, _) => k.compress == q.compress) with
  | none => .error (need q)
  | some (_, a) =>
    match read a with
    | .ok r => r
    | .error e => .error (unreadable q e)

def plain {α} (read : Json → R α) (j : Json) : R (Except Refusal α) := do return .ok (← read j)

def writeRef (r : Ref) : Json :=
  obj [("id", jstr r.id), ("round", jint r.round), ("attempt", jstr r.attempt),
       ("requester", jstr r.requester), ("made_by", jopt jstr r.made_by)]

def readPrepared (j : Json) : R Prepared := do
  return { form := ← field j "form", stages := ← (← arr j "stages").mapM readStage,
           vals := ← readArgs (← field j "vals") }

/-- The request's own code, as the answers the test gave. -/
def oracle (answers : List (Json × Json)) : Oracle where
  clocks stage started held up :=
    asked answers (tag "clocks" (obj [("stage", jstr stage), ("started", jint started), ("held", jint held), ("up", jint up)]))
      (readResult readClocks)
  open_after key day n :=
    asked answers (tag "open_after" (obj [("key", jstr key), ("day", jint day), ("n", jint n)]))
      (readResult (·.getInt?))
  on e r _ form :=
    asked answers (tag "on" (obj [("ending", jstr (endingWord e)), ("ref", writeRef r), ("form", form)]))
      (plain fun j => do (← j.getArr?).toList.mapM readOnLine)
  reform form given clear approver _ :=
    asked answers (tag "reform" (obj [("form", form), ("given", given), ("clear", jlist jstr clear), ("approver", .bool approver)]))
      (readResult readFormed)
  prepare given _ :=
    asked answers (tag "prepare" (obj [("given", given)])) (readResult readPrepared)
  vals form :=
    asked answers (tag "vals" (obj [("form", form)])) (plain readArgs)

/-- One line: `{"vector": …, "answers": [{"q": …, "a": …}, …]}`, answered with `{"need": q}` or
    `{"answer": {"ok": …} | {"err": …}}`. -/
def answer (req : Request) (line : Json) : Json :=
  let run : R (Except Refusal Moved) := do
    let v ← field line "vector"
    let answers ← (← arr line "answers").mapM fun x => do return (← field x "q", ← field x "a")
    let o := oracle answers
    let org ← readOrg (← field v "organization")
    let now ← int v "now"
    let ev ← field v "event"
    match tagged ev with
    | .ok ("submit", s) => return glue_start req.spec req.sub o (← readSubmit s) org now
    | _ =>
      let t ← readTicket (← field v "ticket")
      return glue_next req.spec o t (← readEvent ev) org now
  match run with
  | .error e => obj [("error", jstr e)]
  | .ok r =>
    match r with
    | .error x =>
      match x.refused, x.text, x.at_ with
      | .unread, "need", some q =>
        match Json.parse q with
        | .ok qj => tag "need" qj
        | .error e => obj [("error", jstr e)]
      | _, _, _ => tag "answer" (writeMoved r)
    | .ok _ => tag "answer" (writeMoved r)

end RingiCore.Run
