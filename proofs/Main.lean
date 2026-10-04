/-
  `ritsu-model`: the models of DESIGN 11.2, run (DESIGN 11.3).

  ```
  ritsu-model chobo <book.json>     each line of stdin a scenario; each line out its result
  ritsu-model koyomi <file.json>    each line of stdin an input; each line out what it gives
  ritsu-model dandori <flow.json>   each line of stdin a scenario; each line out the run
  ```

  The first argument names the model and the second is the file the Rust test wrote for it: a
  book, a dates or calendar file, a flow, resolved the way the model reads them. Every line in is
  answered by exactly one line out, in order, so the test can compare them a line at a time. What
  runs here is the functions the theorems of `ChoboModel`, `KoyomiModel` and `DandoriCore` are
  about; nothing in this file decides anything.
-/
import Lean.Data.Json
import ChoboModel
import KoyomiModel
import DandoriCore

open Lean

/-- Answer every line of stdin with `answer`, one line out for each. -/
partial def eachLine (answer : String → String) : IO Unit := do
  let stdin ← IO.getStdin
  let stdout ← IO.getStdout
  let rec loop : IO Unit := do
    let line ← stdin.getLine
    if line.isEmpty then return
    let l := line.trimAsciiEnd.toString
    unless l.isEmpty do
      stdout.putStrLn (answer l)
    loop
  loop
  stdout.flush

def jsonAnswer (f : Json → Json) (line : String) : String :=
  match Json.parse line with
  | .ok j => (f j).compress
  | .error e => (Json.mkObj [("error", Json.str s!"unreadable line: {e}")]).compress

def usage : String :=
  "ritsu-model chobo <book.json> | koyomi <file.json> | dandori <flow.json>, with the inputs on stdin, a JSON value a line"

def main (args : List String) : IO UInt32 := do
  match args with
  | [what, file] =>
    let text ← IO.FS.readFile file
    let j ← match Json.parse text with
      | .ok j => pure j
      | .error e => do
        IO.eprintln s!"ritsu-model: {file} is not JSON: {e}"
        return 2
    match what with
    | "chobo" =>
      match ChoboModel.readBook j with
      | .ok b => eachLine (jsonAnswer (ChoboModel.answer b)); return 0
      | .error e => IO.eprintln s!"ritsu-model: cannot read the book: {e}"; return 2
    | "koyomi" =>
      match KoyomiModel.readFile j with
      | .ok f => eachLine (KoyomiModel.answerLine f); return 0
      | .error e => IO.eprintln s!"ritsu-model: cannot read the file: {e}"; return 2
    | "dandori" =>
      match DandoriCore.readFlow j with
      | .ok f => eachLine (jsonAnswer (DandoriCore.answer f)); return 0
      | .error e => IO.eprintln s!"ritsu-model: cannot read the flow: {e}"; return 2
    | _ => IO.eprintln usage; return 2
  | _ => IO.eprintln usage; return 2
