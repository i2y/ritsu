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

**Timeouts.** `timeout until` goes on a task that waits for an answer, an `event` or a `callback`, and
names one of the task's parameters, a `timestamp` (E007, E002, E003). Such a task's `retry` does not
retry a timeout, which past the time would only come again at once, and an `aws` callback cannot
have it, since its parameters are the API's own (E007).
[How long a call may take](tasks.md#how-long-a-call-may-take)

**Ranges.** A value given to a rule, a task, a record or an output fits the range there (E014), and
a value whose range nothing says is a warning (W104). So does a value of an ordered enum, held to a
range by its values (`range >=basic <=gold`) on a rule's input or on a place of the flow, and a
`match` on one takes the values its range lets in. A date or a time does too, held to a range of
days (`range >=2026-01-01 <=2027-12-31`, with `offset +09:00` for a time read at that offset), and so
does `now` in a workflow that says the period it runs in (`now range …`), which is a moment of that
period. [Ranges](tour.md#ranges)

**A rule's precondition.** Some of what a rule takes for granted its inputs' types cannot say: that
one input stays at or below another (`constraint asked <= paid`), that a combination does not happen
(`never has_tool = true, all_parts = true`), or that a day is one of the days a koyomi date comes to
(`range from koyomi`). `ritsu check` holds each such precondition to the values
the call can give (ritsu's E201, with an example that breaks it), and says when nothing decides it
(W201). What it cannot decide, the code that `ritsu dandori gen` writes checks when the workflow
runs, as soon as the values are made: right after the task whose answer they are, or later, where
the run can no longer go another way before the call. A run whose values break it fails there with
`Dandori.BrokenPrecondition`, on every platform.

**The period a workflow runs in.** The period is checked when the workflow runs, as a precondition
is, by the code that `gen` writes: a run that starts outside it calls nothing and fails with
`Dandori.OutOfPeriod`, and a statement that reads `now` checks the clock before it, going to
`on failure` when it is outside, as after a task's error nothing handles. The checker counts that
check as a place a run can fail from, as it counts a call that can fail: a workflow with no
`on failure` that can fail there with a case left open is W101. `ritsu check` reads the days `now` can be at a dates file's calendar and holds
them to the days a date takes (E205 when they can be outside, W205 when nothing says the period).
[The period a workflow runs in](dates-and-books.md#the-period-a-workflow-runs-in)

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

**The platform.** `dandori gen` also checks what only the platform decides: a run whose history
could outgrow the platform's limit (E040), and what the platform needs or cannot do (E050).

## Where a value comes from

Hand-written code starts a run with values it took from somewhere else: the amount from the request
the API took, what is left from the book. `fed by` at the end of an input's line, or of a field of a
record, says where, in the form the toolchain names everything
([examples/refund_request](https://github.com/i2y/ritsu/tree/main/crates/dandori/examples/refund_request)):

```flow
record Balance
  left : money[USDc]  range >=0USDc <=10000000USDc  fed by chobo "books/refunds.book" account remaining

inputs
  amount  : money[USDc]  range >=1USDc <=1000000USDc  fed by openapi "specs/refunds.json" schema RefundCreate property amount
```

The path goes from the directory of the `.flow`. A workflow names what it can be fed from as any
other language does:

| The value comes from | Named as |
|---|---|
| an output of a rule | `rulec "<file>" output <name>` |
| an account of a book | `chobo "<file>" account <name>` |
| a date of a dates file | `koyomi "<file>" date <name>` |
| a field of a `.proto` message | `proto "<file>" message <name> field <name>` |
| a property of an OpenAPI schema | `openapi "<file>" schema <name> property <name>` |
| a property of a JSON Schema | `jsonschema "<file>" def <name> property <name>`, or `jsonschema "<file>" pointer <pointer>` |
| an output of a workflow, or a field of its record | `dandori "<file>" output <name>`, `dandori "<file>" record <name> field <name>` |
| a field of a kaname record | `kaname "<file>" record <name> field <name>` |

`ritsu check` asks the language on each side, and comes to one of three answers:

- **It holds.** The place takes everything the source can come to. A place wider than its source is fine.
- **E207.** The source can come to a value the place refuses. The message gives the value, and the input that gives it when the source can say.
- **W207.** It cannot be decided, and the message says why: rulec counts an end of a rule's output that none of the rule's vectors reaches, or a contract carries a date as a string and lists no days.

E208 says the source is not there: its file, or the name in it.

Numbers are compared in one unit and one step, and another unit does not hold. An enum is compared by the public names of its values, a date by its days, a bool or a string by its type alone. An end the place does not write is not compared. A contract with no upper or no lower bound lets through a value beyond what the place takes, as with a rule's `from` (E122), and does not hold. So does an account of a book with no `at most`, which goes as far as chobo's limit (2⁶³ − 1).

This is the book that lost its `at most`, which a run would otherwise find as `Dandori.BadResponse`:

```console
$ ritsu check refund_request.flow books/refunds.book
books/refunds.book: ok
  sell.do    may be refused: key_conflict
  refund.do  may be refused: nothing_left (remaining(order) at least 0.00), key_conflict, already_refused
  key: sell once per order; a second call that differs only in amount is refused with key_conflict
  key: refund once per request; a second call that differs only in order or amount is refused with key_conflict
refund_request.flow: ok
error[ritsu E207]: refund_request.flow:9:1: `record Balance field left` takes money[USDc] >=0 <=10000000, and its source chobo "books/refunds.book" account remaining can be 10000001
     9 |   left : money[USDc]  range >=0USDc <=10000000USDc  fed by chobo "books/refunds.book" account remaining
  = The book takes it: an end with no `at least` or `at most` goes as far as chobo's limit (2⁶³ − 1).
  = Widen what this place takes, or narrow what the source gives. Which of the two is a person's decision.
ritsu check: 2 files (chobo 1, dandori 1): 1 fail (1 error); borders between the languages: 3 checked, 0 undecided
```

`dandori check` on its own says whether the form reads and stands where it can (E023), since the
other file is another language's to read. **Nothing checks that the hand-written code really carries
that value**, and the page of `dandori doc` says so. What dandori generates does not change on any
platform.

## The size of a run

A platform limits one run: Step Functions keeps 25,000 events of history, Temporal 51,200, and
Lambda durable functions 3,000 operations; on Argo Workflows, the Workflow object holds every node,
and dandori aims at 10,000 nodes at most. Loops say how far they may go (`repeat at most 12 times`,
`for x in xs at most 50`) and there is no recursion, so the build can count a run's largest
history. It counts generously, so near the limit it errs on the side of refusing.

On Temporal, a loop at the top of the flow goes on in a new run (Continue-As-New) once the history is
long, so such a loop counts only up to that point. What still does not fit is the part outside the
loop, or one round that is too large, and the diagnostic says which.

## Names in the code

A generator does not pick another name. When the code of a platform cannot keep a name of the `.flow`
or the `.proto`, because two names are one identifier there (`send_mail` and `sendMail` in Go) or the
code has the name for itself (`Transport` in the Go package, `Array` in TypeScript, `str` in Python),
`dandori check` warns and names the platforms (W105), and `dandori gen` writes nothing when it is to
write one of them (E051). Rename it, or leave that platform out of `--target`.

## Every code

[Diagnostics](reference/codes.md) lists all 38, with what each one finds.
