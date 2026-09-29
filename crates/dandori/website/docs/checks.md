# What it checks

`dandori check` reads a `.flow` and the rules it uses, and follows the flow through every arm,
every error a call can come back with, and every event that can happen on the other side. Each
diagnostic comes with a short run that gets there.

## A case left unfinished

In a hotel booking that holds a card and captures at check-out
([tests/fixtures/hotel_naive.flow](https://github.com/i2y/dandori/blob/main/tests/fixtures/hotel_naive.flow),
a first draft of the example):

<div class="dd-term" markdown>

```text
error[E020]: tests/fixtures/hotel_naive.flow:95:1: the workflow can end here with the case `pi` in requires_payment_method, processing, which is not final (succeeded, canceled are)
    95 |   succeed outcome = stayed
  the run that gets there:
      81  quote = hold(…)
      84  match quote.handling: auto
      84  create_intent: pi starts in requires_confirmation
      85  confirm_intent: pi requires_confirmation → requires_capture
      90  match pi.status: requires_capture
      90  wait until booking.check_out
      93  capture_intent: pi requires_capture → processing
          `settle` happens on the other side: pi processing → requires_payment_method
      95  succeed
```

</div>

The transitions come from `payment_intent.rule`, a transcription of Stripe's documents into a
rulec state machine. The workflow says which events happen on their own
(`external authenticate, settle, expire`), and the checker follows them too: while the workflow
waits until check-out, the authorization can expire, and then the capture is refused; after the
capture, the bank can still decline the payment.

## What it looks at

**Types and names.** Every name is declared once and used with its type; a value that may be absent
is matched before it is used; the arguments and outputs are all there (E001–E006).

**Every arm, every place.** A `match` names every value (E010), and no arm is dead (E011). A
variable is read only where it has a value on every way there (E012). `yield`, `break`, `succeed`
and the calls on a case go only where they mean something (E009).

**Cases.** A task is called on a case only once it has started, and started only once (E013). An
event the state machine refuses in every state is an error (E021), and one it can refuse must be
handled (E022). When the workflow ends, every case it started is in a final state, unless the flow
hands it over with `fail … leaving` (E020). An error nothing handles can leave a case unfinished too
(W101), which `on failure` settles.

**Retries.** A call that changes something on the other side and is retried without an idempotency
`key` could change it twice (E030; W030 when it only may change something). A task that starts a
case without a `key` is a warning (W103).

**Ranges.** A value given to a rule, a task, a record or an output fits the range there (E014), and
a value whose range nothing says is a warning (W104). [Ranges](tour.md#ranges)

**What a task calls.** A task that runs another `.flow` fits the child's inputs, outputs and failures
(E015), and a task that calls a described API fits the description (E016). A Jev task answers what
Jev can answer (E007), and one that relies on how sure Jev is names the version it relies on (W032).
[Jev](jev.md)
[What a task calls](tasks.md)

**The platform.** `dandori build` also checks what only the platform decides: a run whose history
could outgrow the platform's limit (E040), and what the platform needs or cannot do (E050).

## The size of a run

A platform limits one run: Step Functions keeps 25,000 events of history, Temporal 51,200, and
Lambda durable functions 3,000 operations; on Argo Workflows, the Workflow object holds every node,
and dandori aims at 10,000 nodes at most. Loops say how far they may go (`repeat at most 12 times`,
`for x in xs at most 50`) and there is no recursion, so the build can count a run's largest
history. It counts generously, so near the limit it errs on the side of refusing.

On Temporal, a loop at the top of the flow goes on in a new run (Continue-As-New) once the history is
long, so such a loop counts only up to that point. What still does not fit is the part outside the
loop, or one round that is too large, and the diagnostic says which.

## Every code

[Diagnostics](reference/codes.md) lists all 29, with what each one finds.
