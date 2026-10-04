/-
  The check on one flow, worked out by Lean when the library builds.

  A payment is opened (`pending`), then captured; the capture times out, and the flow goes on. On
  the other side the capture may have happened all the same, and the payment is `processing`. The
  flow then matches on the payment's record, which still says `pending`, and voids it. If the
  capture did happen, the machine refuses the void, the call comes back `unexpected_state`, and
  the handler ends the workflow with the payment still `processing`, which is not final.

  `chkFlow` refuses the flow: after the timeout it keeps both pairs (heard `pending`, really
  `pending`) and (heard `pending`, really `processing`), and the match on the record narrows what
  the flow heard, not where the payment is. With the handler handing the payment over
  (`fail … leaving p`), it passes. dandori's check says the same of the flow
  (`tests/fixtures/capture_timeout.flow`: E020 at the `succeed`; `capture_timeout_handed_over.flow`:
  ok).

  The second flow is the same mistake in `on failure`, as the hotel booking of dandori's examples
  made it: the capture fails with nothing to handle it, `on failure` reads the record, still
  `pending`, and voids the payment; a refusal that only passes ends `on failure`, and the workflow
  fails with the payment `processing`.
-/
import DandoriCore.Check

namespace DandoriCore.Examples

/-- pending —capture→ processing —settle→ done; pending —void→ voided; done and voided final. -/
def payment : Machine where
  names := ["pending", "processing", "done", "voided"]
  initial := 0
  finals := [2, 3]
  next := fun s e => match s, e with
    | 0, "capture" => [1]
    | 0, "void" => [3]
    | 1, "settle" => [2]
    | _, _ => []
  refuses := fun s e => match s, e with
    | 0, "capture" => false
    | 0, "void" => false
    | 1, "settle" => false
    | _, _ => true
  ext := fun _ => []
  any := fun s => ["capture", "settle", "void"].flatMap (fun e => match s, e with
    | 0, "capture" => [1]
    | 0, "void" => [3]
    | 1, "settle" => [2]
    | _, _ => [])

def task (n : String) (eff : Effect) : Task :=
  { name := n, result := none, range := none, retriers := [], effect := eff, refusedAs := some "unexpected_state" }

/-- The flow, with what the void's `on unexpected_state` does. -/
def flow (onRefused : List Stmt) : Flow where
  records := []
  inputs := []
  tasks := [task "open_payment" (.starts []), task "capture_payment" (.sends "capture"),
            task "void_payment" (.sends "void"), task "settle_payment" (.sends "settle")]
  rules := []
  cases := [{ name := "p", stateField := "status" }]
  monitors := []
  flow := [
    .call 1 1 (some (.case 0)) (.task 0) [] [],
    .call 2 2 (some (.case 0)) (.task 1) [] [.mk [.timeout] [.pass 3]],
    .matchOn 4 4 (.var "p" ["status"]) "p.status" [
      .mk ["pending"] false none
        [.call 5 6 (some (.case 0)) (.task 2) [] [.mk [.declared ["unexpected_state"]] onRefused]],
      .mk ["processing"] false none [.call 7 9 (some (.case 0)) (.task 3) [] []]]]
  onFailure := none
  onCancel := none

def env (onRefused : List Stmt) : Env := { flow := flow onRefused, ms := fun _ => payment, n := 1 }

/-- Ending the workflow there can leave the payment `processing`: refused. -/
example : chkFlow (env [.succeed 6 []]) 100 = false := by decide

/-- Handing the payment over as it is: passes. -/
example : chkFlow (env [.fail 6 "Stuck" none [0]]) 100 = true := by decide

/-- The capture fails with nothing to handle it; `on failure` voids what the record says is
    `pending`, with what `on unexpected_state` does. (The payment is opened with its failure handed
    over, so that only the capture's failure comes to `on failure`.) -/
def flowOnFailure (onRefused : List Stmt) : Flow where
  records := []
  inputs := []
  tasks := [task "open_payment" (.starts []), task "capture_payment" (.sends "capture"),
            task "void_payment" (.sends "void"), task "settle_payment" (.sends "settle")]
  rules := []
  cases := [{ name := "p", stateField := "status" }]
  monitors := []
  flow := [
    .call 1 1 (some (.case 0)) (.task 0) [] [.mk [.failure] [.fail 2 "NotOpened" none [0]]],
    .call 3 3 (some (.case 0)) (.task 1) [] [],
    .call 4 4 (some (.case 0)) (.task 3) [] []]
  onFailure := some [
    .matchOn 5 6 (.var "p" ["status"]) "p.status" [
      .mk ["pending"] false none
        [.call 6 8 (some (.case 0)) (.task 2) [] [.mk [.declared ["unexpected_state"]] onRefused]],
      .mk ["processing"] false none [.fail 8 "Unclear" none [0]]]]
  onCancel := none

def envOnFailure (onRefused : List Stmt) : Env := { flow := flowOnFailure onRefused, ms := fun _ => payment, n := 1 }

/-- A refusal that only passes: `on failure` can end with the payment `processing`. Refused. -/
example : chkFlow (envOnFailure [.pass 7]) 100 = false := by decide

/-- A refusal that hands the payment over: passes. -/
example : chkFlow (envOnFailure [.fail 7 "CleanupFailed" none [0]]) 100 = true := by decide

end DandoriCore.Examples
