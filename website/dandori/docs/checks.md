# What it checks

`dandori check` reads a `.flow` and the rules it uses, and follows the flow through every arm,
every error a call can come back with, and every event that can happen on the other side. Each
diagnostic comes with a short run that gets there.

## A case left unfinished

In a hotel booking that holds a card and captures at check-out
([tests/fixtures/hotel_naive.flow](https://github.com/i2y/ritsu/blob/main/crates/dandori/tests/fixtures/hotel_naive.flow),
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
(W101), which `on failure` settles. A `match` on a case's state reads its record, which says what
the workflow last heard of the case. After a call that failed, the event may have happened on the
other side all the same, and the case be further on than its record says: the checker keeps both,
so a `match` narrows the record, never where the case may be, and an arm for a state the record
cannot say there is dead (E011).

**Dates and books.** A date is given what its inputs take: a day, or a time when its calendar says
its UTC offset (E003). A task that runs an operation of a book takes what the operation takes,
answers the hold, and declares only reasons the book can refuse with (E016, E007). A hold followed as
a case can expire on the other side before it is posted, and each reason the book can refuse with
there must be handled (E022). [Dates and books](dates-and-books.md)

**Retries.** A call that changes something on the other side and is retried without an idempotency
`key` could change it twice (E030; W030 when it only may change something). A task that starts a
case without a `key` is a warning (W103).

**Ranges.** A value given to a rule, a task, a record or an output fits the range there (E014), and
a value whose range nothing says is a warning (W104). [Ranges](tour.md#ranges)

**A rule's precondition.** Some of what a rule takes for granted its inputs' types cannot say: that
one input stays at or below another (`constraint asked <= paid`), or that a day is one of the days a
koyomi date comes to (`range from koyomi`). `ritsu check` holds each such precondition to the values
the call can give (ritsu's E201, with an example that breaks it), and says when nothing decides it
(W201). What it cannot decide, the code that `ritsu dandori build` writes checks when the workflow
runs, as soon as the values are made: right after the task whose answer they are, or later, where
the run can no longer go another way before the call. A run whose values break it fails there with
`Dandori.BrokenPrecondition`, on every platform.

**What a task calls.** A task that runs another `.flow` fits the child's inputs, outputs and failures
(E015), and a task that calls a described API fits the description (E016). A decision task (`jev`) answers what
a decision model can answer (E007), and one that relies on how sure the model is names a model that
does not move to another by itself (W032); a refusal is declared on the Decisions API alone (E007).
[Decision models](jev.md)
[What a task calls](tasks.md)

**The service a workflow implements.** A workflow whose entry is a proto service fits it: its inputs
and outputs are the start's request and response, every name it fails with is listed, and the
events and callbacks the service names are tasks of the flow that read what the service sends (E017).
[Implement a service](services.md)

**Secrets and keys.** A value the contracts mark secret (`debug_redact` in a `.proto`,
`x-data-classification`, `x-sensitive-data` or `format: password` in an OpenAPI document, `secret`
in the `.flow`) is kept in the history of a run as an input, an output, an answer, an argument or a
`fail`'s reason (W904), and is sent outside the project only where the task says it `discloses` it
(E906). A key written in the `.flow` (W901) and a call over plain HTTP to another machine (W902) are
warnings, unless the line or the URL says why. [Secrets](secrets.md)

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

[Diagnostics](reference/codes.md) lists all 35, with what each one finds.
