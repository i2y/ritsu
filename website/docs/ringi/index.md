---
hide:
  - toc
---

# ringi

**A small language for the requests a company's departments make and approve: the form of one kind
of request, the route it takes and who answers each stage, its deadlines, and what happens when it
ends, written in one `.ringi` file. ringi checks it against the organization's data, for every
person who can submit it, and ritsu writes from it the code, the pages and the tests a server of
requests runs on.**

Every department has requests that go round for approval: an expense or a trip, leave, a purchase,
a contract to sign, access to a system, a change to a price list. Each kind has a form, and a route
that depends on what the form says and on who is asking: the requester's manager, the head of the
department for a long leave, legal for a contract, the board over an amount. A kind of request is a
`.ringi` file (a *request type*), and the files around it say what several of them share: a route,
the shape of the organization, the lists a form chooses from.

ringi keeps one request as a ticket. What the ticket does next is a function of four things: the
ticket as it is, the event that came (an approval, a send-back, a reminder's clock, the answer of a
flow), the organization at that time, and the time. The check, the reference interpreter, the
simulation, the pages and the generated code all stand on that one function.

## Who each page is for

| You are | Read |
|---|---|
| an agent that writes `.ringi`, or an engineer who asks one to | [Writing a request type](language.md), then [What the check guarantees](checks.md) |
| an administrator who looks after a kind of request, or an approver who wants to know where a request goes | [Reading a request type](reading.md) |
| someone who builds or runs the server the requests go through | [What is generated, and what the server does](server.md) |
| writing the calendar, the budget, the gate or the flows a request reads | [With the other languages](together.md) |

## A request type

A leave request (`crates/ringi/examples/leave.ringi`), approved by the requester's manager, and by
the head of the department too for more than five business days. The rules are fictional, as every
example's are:

<!-- crates/ringi/examples/leave.ringi -->
```ringi
request leave v1 "Leave request"
…
use organization org from "shared/company.ringi"
use calendar office from "shared/office.cal"
use book leave from "shared/leave.book"
use gate "shared/company.gate"
use route standard from "shared/manager_approval.ringi"

form
  kind : choice "Kind of leave"
    paid    "Paid leave"
    sick    "Sick leave"
    special "Special leave"
  first_day : date "First day"  range >=2026-01-01 <=2027-12-31  open in office
  days      : number "Business days"  range >=1 <=20
  back_on   : date "Back at work on" = first_day + days business days in office
  reason    : long text "Reason"  count <=200  required when kind is special
  cover     : person "Who covers for you"  optional

route standard(long: days > 5)

ticket
…
  submitted by   everyone except team board
  self approval  not allowed
…
  same approver  approves once
  send back      to the requester, at most 2 times
…
  comment required  for reject, send back
  resubmit       within 5 business days
  withdraw       until it is decided
  on submitted   hold leave.take(request: ticket.id, attempt: ticket.attempt, person: requester.id, days: days)  when kind is paid
  on approved    post leave.take(request: ticket.id, attempt: ticket.attempt)  when kind is paid
  on sent back, rejected, withdrawn, expired  void leave.take(request: ticket.id, attempt: ticket.attempt)  when kind is paid
```

The route is one that several requests share. Who answers each stage is written as a relation in
the organization (`manager of requester`), not as a person, so the route follows the organization
as it changes:

<!-- crates/ringi/examples/shared/manager_approval.ringi -->
```ringi
route manager_approval v1 "Approval by the manager"
…
inputs
  long : bool

route
  manager  manager
  head     dept_head  when long

who
  manager   = manager of requester
  dept_head = head of requester.department

every stage
  due in 2 business days at end of day
  remind after 1 business day at 09:00
  when late escalate, then expire
…
  when empty escalate, then skip
```

## What the check says

`ritsu ringi check` reads the organization's data of every version, and finds who answers each
stage for each person who may submit the request. Here, in the organization from 2027-04-01, the
legal team has no head, so two lawyers' long leaves go one level up:

```console
$ cd crates/ringi/examples
$ ritsu ringi check leave.ringi
warning[W502]: leave.ringi:22:7: In the organization from 2027-04-01, the stage `head` goes elsewhere or is skipped for lack of an approver
    22 | route standard(long: days > 5)
  = `dept_head` comes to the team legal (Legal), whose head is empty.
  = The stage is `when empty escalate, then skip`, and it goes up.
  = It goes up to e0102 (Head of Corporate Division) for 2 requesters: e9002 (Lawyer 2) and e9003 (Lawyer 3).
  = Correct the data of the version, or make sure this is meant.
ok leave.ringi: a request, 6 fields, 2 stages
  used from 2026-04-01: the first version of the organization `shared/company.ringi` (data `../org`)
  used with no end: no data it reads ends
  ends within 40 business days at the most (3 × 10 at its stages, 2 × 5 with the requester)
  submitted by 19 of 24 people in the organization from 2026-04-01, and 18 of 23 from 2027-04-01 (`everyone except team board`)
  requesters whose stages (`head`) go elsewhere or are skipped by the shape of the organization: 9 of 19 in the organization from 2026-04-01, and 8 of 18 from 2027-04-01
  requesters whose late stages (`manager`, `head`) have nowhere to go up to by the shape of the organization: 2 of 19 in the organization from 2026-04-01, and 2 of 18 from 2027-04-01
  who they are, and where their stages go: `ritsu ringi simulate leave.ringi --every-requester`
```

The warning says what is so and lets the request pass: the route was written to go up when a stage
has nobody (`when empty escalate`). Under the line that says the file passes, the check says from
when the request can be used, the longest a ticket can take, and how many people it leaves out or
sends elsewhere. `ritsu check` runs the same check with every other language of the project, and
checks what passes between them: [With the other languages](together.md).

## What it does when it runs

A scenario writes the events of one ticket, a row each, and what the ticket comes to after each.
`ritsu ringi run` gives each row to the reference interpreter at the time it is written at, with
the organization and the books, and shows whom the ticket waits for and what it had the books do:

```console
$ cd crates/ringi/examples
$ ritsu ringi run leave.ringi --scenario a_long_paid_leave
2026-11-16 09:00  submit   by e1203  -> manager (e1101), due 2026-11-18 end of day; hold leave.take 8 day
2026-11-16 17:00  approve  by e1101  -> head (e3101), due 2026-11-18 end of day
2026-11-17 09:00  remind             -> head (e3101), due 2026-11-18 end of day; reminded e3101
2026-11-19 00:00  late               -> head (e0101), due 2026-11-23 end of day; head went up to e0101
2026-11-19 10:00  approve  by e0101  -> approved; post leave.take
the book `leave` at the end: leave_left(e1203) 12 day, granted -20 day, taken 8 day
  the hold `take` (T-1, attempt-1): posted
scenario `a_long_paid_leave`: as written (5 rows)
```

The head of the department did not answer by the deadline, so the stage went up to the head of the
division, as the shared route says. `ritsu ringi check` runs every scenario too, and fails when a
row's stage or status is not what the ticket comes to.

## What is made of it

- **Pages for people.** `ritsu ringi doc` draws a request type on one page: the form, the route with
  who answers each stage, the deadlines, and what a ticket does. [Reading a request type](reading.md)
  shows the pages of the examples.
- **Code and contracts.** `ritsu gen` writes the ticket machine and each request type's code in
  TypeScript, Python, Go and Rust, and with `--target requests` the screens, the OpenAPI document,
  the tools of an agent and the vectors a server is tested with:
  [What is generated, and what the server does](server.md).
- **A server to run.** [ringi-server](https://github.com/i2y/ringi-server), a framework in Go, serves
  the requests: the pages people use in a browser, the API and the tools of an agent.
  `ritsu ringi init` writes the first files of [a project on it](server.md#a-server-on-ringi-server).

[The playground](../playground/#project=ringi/leave) opens the leave request with every file it
reads, in the browser. Edit the request or the shared route and the check answers at once; open
*generated code* for what `ringi api`, `ringi run`, `ringi extract` and `ringi vectors` print or
write, or *the page for people*.

## Read more

- [ringi's README](https://github.com/i2y/ritsu/blob/main/crates/ringi/README.md): the language at
  a glance, its commands, and how it is tested.
- [The diagnostics](https://github.com/i2y/ritsu/blob/main/crates/ringi/docs/codes.md): for each
  code, when it comes, how to fix it, and the smallest file that gives it (`ritsu ringi explain`
  prints one).
- [DESIGN.md](https://github.com/i2y/ritsu/blob/main/crates/ringi/DESIGN.md), in Japanese: why a
  request is a ticket moved by events, what ringi lowers to the other languages, and where the
  server begins.
- [The skill](https://github.com/i2y/ritsu/blob/main/skills/ringi/SKILL.md), for an agent that
  writes or fixes a request type.
- [ringi-server's site](https://i2y.github.io/ringi-server/): a tutorial that makes a server of
  requests, how it works, guides for what you want to do, and how to run it for your staff.
