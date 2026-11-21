# With the other languages

*For whoever writes the files a request type reads: the company's calendar, a department's budget,
the rules of the whole company, the table of authority, the flows an agent or the work after an
approval runs. And for the engineer who puts them together.*

ringi gives a meaning to four things only: the form, the route and who answers it, the ticket, and
the organization. Everything else a request needs is a file of another of ritsu's languages, which
the request reads with `use`, and which its own language checks:

| A request reads | Language | What for |
|---|---|---|
| `use calendar office from "shared/office.cal"` | koyomi | the business days its deadlines, reminders and dates count |
| `use book budget from "shared/budget.book"` | chobo | a budget or a leave balance it holds when submitted, and posts or lets go when it ends |
| `use gate "shared/company.gate"` | sekisho | the rules of the whole company: who may do what to a ticket, and who administers a kind of request |
| `use rule authority from "shared/authority.rule"` | rulec | a table several requests share, such as the table of authority |
| `use flow pay from "shared/pay.flow"` | dandori | the work after an approval, an agent that answers a stage, a summary for the approvers |

What a request writes inside it is lowered the same way: the route's table and the form's
computations to rules of rulec, the deadlines to a dates file of koyomi, and the ticket of each
request to a module of kaname. A requirement of yuen (`shared/expense.req`) points at the lines of
a request that carry out an article of the regulations.

## What `ritsu check` checks between them

Each language checks its own file alone. Where a request and the file it reads are written by
different people, each can pass and the two still not fit. `ritsu check` checks those borders too:

- **The gate and the route.** Every operation the server asks the gate about has an action that
  the gate declares; the approvers the route finds may approve; and a requester may not decide their
  own request where the route says so. A gate that lets them is an error (E911).
- **The book and the ticket.** A hold made when a request is submitted lasts longer than the longest
  a ticket can keep it open, and every requester has the values its arguments read (E209).
- **The flows and the ticket.** What a request gives a flow is in the range and the type the flow
  takes; an agent's flow answers `ok` or `doubt` with a reason; and a flow that submits a request
  through the server's API calls an operation of the request as it is now (E210). The `http` task
  that makes such a call writes `key`: the server asks an `Idempotency-Key` header of every POST,
  which dandori fills from it.

The requests of the examples pass every border, each with every file it reads:

```console
$ cd crates/ringi/examples
$ ritsu check expense.ringi purchase.ringi leave.ringi account.ringi
ok account.ringi: a request, 5 fields, 4 stages
…
ritsu check: 4 files (ringi 4): all pass (8 warnings); borders between the languages: 38 checked, 0 undecided
```

## A border that does not hold

The budget's hold, set to expire after 90 days instead of 365:

```diff
   key request, attempt
-  pending expires after 365 days
+  pending expires after 90 days
```

The book passes its own check, and the purchase passes ringi's. `ritsu check purchase.ringi` checks
the two together:

```text
…
error[ritsu E209]: purchase.ringi:77:26: The hold `budget.reserve` can expire before a ticket of `purchase` posts or voids it
    77 |   on submitted      hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: requester.department, amount: amount)  when requester.department is not none
  = The hold is made when a request is submitted and stays open until its round ends: 216 business days at the most, as ringi's check counts them (a round of 54, walked 4 times where a stage sends the ticket back to an earlier stage).
  = A request submitted on 2030-03-01 can stay open through 2031-01-06, 216 business days later in the calendar `office`: 312 days.
  = The hold expires 90 days after it is made (`reserve` of "crates/ringi/examples/shared/budget.book").
  = The book refuses a post or a void of an expired hold with `expired`: the ticket would be approved with nothing held for it.
  = Make the hold last longer (the transfer's `pending expires after`), or give the stages shorter deadlines.
ritsu check: 1 file (ringi 1): 1 fail (1 error, 1 warning); borders between the languages: 8 checked, 0 undecided
```

A purchase can be sent back to any earlier stage three times, and the hold stays open while it is.
The error says the longest it can stay open, from the day that makes it the longest, and what to
change. `ritsu explain E209` prints the smallest files that give it, as it does for E911 and E210.

## One package

`ritsu gen` writes the code of every language of the project into one package for each of
TypeScript, Python, Go and Rust: the rules, the dates, the books' clients, the gate, the flows, the
modules of kaname, and the tickets of the requests. A server calls them in one process, and the
same rule answers the screen and the server ([What is generated, and what the server does](server.md)).
