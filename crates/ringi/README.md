# ringi

A small language for a company's applications and approvals: the form of a kind of request, the route it takes and who approves it at each stage, its deadlines and reminders, and what is done in the books and the workflows when it ends.
An expense, a purchase, a leave, an account on one of the company's systems and a contract to review are each one file, and accounting, general affairs, HR, IT and legal write theirs in the same words.
A request reads the company's organization from its data (people, teams and positions, version by version), the masters a form chooses from, the calendar, the balances its books keep, the gate and the workflows, and ringi checks it against them: that every request comes to a row of the route, that every stage of every requester has someone to answer it in every version of the organization, that every ticket ends, and that what is held in a book is posted or let go however the ticket ends.
The table of the route, the conditions, the computed fields and the deadlines are lowered to files of rulec, koyomi and kaname, which check them, and ringi says what they find at the lines of the `.ringi`.
A ticket, one request on its way, is moved by one function from its state, the event, that day's organization and the time to its next state and the things to do; `ritsu gen` writes that function in TypeScript, Python, Go and Rust, and, with `--target requests`, the API, the agents' tools and the screens a server serves.
ringi is not a server: keeping the tickets, the clock, the notices and the screens are the server's.
ringi is one of the ten languages of [ritsu](../../README.md).
On ritsu's site, **[the pages of ringi](https://i2y.github.io/ritsu/ringi/)** go through how a request type is written, what the check guarantees, how the people who run the requests read its pages, what is generated and what the server does, and the files of the other languages a request reads.

## A request

[examples/expense.ringi](examples/expense.ringi) is an expense, a trip or a purchase.
It names what it reads, then its form:

```ringi
request expense v1 "Expense request"
…
use organization org from "shared/company.ringi"
use masters m from "shared/masters.ringi"
use calendar office from "shared/office.cal"
use book budget from "shared/budget.book"
use gate "shared/company.gate"
use flow precheck_agent from "shared/precheck.flow"
…
form
  category : choice "Category"
    travel   "Travel"
    expense  "Expenses"
    purchase "Purchase"
  items : rows "Items"  count >=1 <=20
    what  : text "What"  count <=100
    price : money[JPY] "Price"  range >=1JPY <=1_000_000JPY
  amount  : money[JPY] "Amount" = sum(items.price)  range >=1JPY <=10_000_000JPY
  …
  trip_to : text "Destination"  count <=100  shown when category is travel
  receipt : attachment "Receipt"  required when amount > 30_000JPY
  account : choice from m.accounts "Account code"  approvers only
```

The route is a table: a row for each case of the form, and a column for each stage, naming who answers it, or blank where the requests of that row skip it.
`who` says how each one is found, from the organization of the day the stage starts:

```ringi
route
| category | amount                     | -> precheck | first   | second                   | decide        |
| travel   | <=30_000JPY                |             |         |                          | travel_agent  |
| travel   | >30_000JPY <=100_000JPY    |             |         |                          | manager       |
| travel   | >100_000JPY                |             | manager |                          | dept_head     |
| expense  | <=100_000JPY               | check       |         |                          | manager       |
| expense  | >100_000JPY <=1_000_000JPY | check       | manager |                          | dept_head     |
| expense  | >1_000_000JPY              |             | manager | dept_head                | division_head |
| purchase | <=1_000_000JPY             |             | manager | accounting               | dept_head     |
| purchase | >1_000_000JPY              |             | manager | dept_head and accounting | division_head |

who
  manager       = manager of requester
  dept_head     = head of requester.department
  accounting    = team accounting
  division_head = nearest division_head above requester
  check         = agent precheck_agent, else manager
  travel_agent  = agent travel_decision, else manager
  board         = team board
```

`check` and `travel_agent` are stages an agent answers, through a workflow of dandori, and the manager answers them when the agent is not sure.
Every stage has a deadline and a reminder in business days of the calendar, and says where it goes when it is late and when nobody is there to answer it.
The ticket says who may approve what, how often it can be sent back, and what is done in the budget's book and which workflow starts as it ends:

```ringi
every stage
  due in 3 business days at end of day
  remind after 2 business days at 09:00
  when late escalate by next_up, then expire
  …
  when empty escalate, then board
…
ticket
  self approval  not allowed
  …
  same approver  approves once
  send back      to the requester, at most 3 times
  resubmit       within 3 business days
  withdraw       until decide
  …
  on submitted   hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: requester.department, amount: amount)  when requester.department is not none
  on approved    post budget.reserve(request: ticket.id, attempt: ticket.attempt)
  on approved    launch pay(request_id: ticket.id, requester: requester.id, amount: amount, purpose: purpose)
  on sent back, rejected, withdrawn, expired  void budget.reserve(request: ticket.id, attempt: ticket.attempt)
```

The same file has its claims, examples of the route and scenarios, which the check holds it to.

## The organization

The organization is a file of its own, [examples/shared/company.ringi](examples/shared/company.ringi), which names its positions, its kinds of team and the folder of its data.
Each version of the data is a folder named by the day it takes effect, with the people, the teams and who is in which team, as CSV:

```csv
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e0001
sales_div,Sales Division,営業本部,division,company,e0101
sales1,Sales Department 1,営業一部,department,sales_div,e3101
sales1_a,Sales Section 1A,営業一課,section,sales1,e1101
…
```

The examples have the version from 2026-04-01 and one from 2027-04-01, in which Sales Section 1A is folded into its department and the head of legal leaves.
The proxies are one more file beside them, each row a person, the proxy, what the proxy does, the days and the kinds of request:

```csv
person,proxy,kind,from,to,requests
e3101,e1101,approve,2026-12-28,2027-01-08,expense expense_ja purchase purchase_ja
```

## What the checker says

`check` reads the organization in each of its versions, finds who answers each stage for every requester, and says what it finds:

```console
$ ritsu ringi check examples/expense.ringi
warning[W502]: examples/expense.ringi:33:93: In the organization from 2027-04-01, the stage `decide` goes elsewhere or is skipped for lack of an approver
    33 | | travel   | >100_000JPY                |             | manager |                          | dept_head     |
  = `dept_head` comes to the team legal (Legal), whose head is empty.
  = The stage is `when empty escalate, then board`, and it goes up.
  = It goes up to e0102 (Head of Corporate Division) for 2 requesters: e9002 (Lawyer 2) and e9003 (Lawyer 3).
  = Correct the data of the version, or make sure this is meant.
  = The same on the rows of lines 33, 35, 37.
…
ok examples/expense.ringi: a request, 8 fields, 4 stages
  used from 2026-04-01: the first version of the organization `shared/company.ringi` (data `../org`)
  used with no end: no data it reads ends
  ends within 93 business days at the most (4 × 21 at its stages, 3 × 3 with the requester)
  requesters whose stages (`precheck`, `first`, `second`, `decide`) go elsewhere or are skipped by the shape of the organization: 14 of 24 in the organization from 2026-04-01, and 13 of 23 from 2027-04-01
  requesters whose late stages (`second`) have nowhere to go up to by the shape of the organization: 1 of 24 in the organization from 2026-04-01, and 1 of 23 from 2027-04-01
  who they are, and where their stages go: `ritsu ringi simulate examples/expense.ringi --every-requester`
```

The warnings left out above say that one person answers two stages of some requesters' tickets (W506), where nothing comes between them to make one approval do for both.
A request no row of the route takes is an error, with the row to add, its cells cut at the bounds the table already writes:

```console
$ ritsu ringi check tests/mutants/E301_no_row_over_100000_yen.ringi
error[E301]: tests/mutants/E301_no_row_over_100000_yen.ringi:6:1: No row of the route takes some requests
     6 | route
  = The requests no row takes: amount from 100_001JPY to 1_000_000JPY
  = A request no row takes: amount = 100_001JPY
  = Add this row to the table, writing in place of each `?` who answers that stage (a name in `who`; a blank cell where those requests skip it): | >100_000JPY | ? | ? |
  = The cells are cut only at the bounds the table already writes and at the ends of the ranges, so the rows close the gap as they are; where the approvers change inside a row, split it there.
```

A stage that comes to nobody, and waits there, is an error too:

```console
$ ritsu ringi check tests/mutants/E502_the_president_has_no_manager.ringi
error[E502]: tests/mutants/E502_the_president_has_no_manager.ringi:9:10: In the organization from 2026-04-01, the stage `check` has no approver
     9 |   check  manager
  = `manager` finds no head above the requester.
  = For 1 requester: e01 (President (fictional)).
  = The stage is `when empty stay`: a ticket that comes to it waits there until an administrator adds an approver.
  = Correct the data of the version, or change the way to choose the stage's approver or its `when empty`.
```

What rulec, koyomi and kaname say of the files a request is lowered to comes back the same way, at the row of the route, the field or the deadline it came from.
[docs/codes.md](docs/codes.md) has every code, with the smallest file that prints it.

## Trying a requester

`simulate` shows who answers each stage for one requester on one day.
From 2026-12-28 to 2027-01-08, the organization's data names e1101 as the proxy of e3101 for the approvals of expense and purchase requests, so on 2026-12-29 either of them can answer the stages that come to the head of the department:

```console
$ ritsu ringi simulate examples/expense.ringi --requester e1201 --on 2026-12-29
examples/expense.ringi on 2026-12-29, in the organization from 2026-04-01
  without `--form`: every stage the route can come to, by each way to choose its approver; which come depends on the form
requester e1201 (Sales staff 1)
  precheck  check (manager when the agent doubts): e1101 (Manager of Section 1A)
  first     manager: e1101 (Manager of Section 1A); when the agent doubts, done with the approval of `precheck` where no other stage comes between them
  second    dept_head: e3101 (Head of Sales Department 1) [e1101 (Manager of Section 1A) too, as the proxy of e3101]
            accounting: one of e1001 (Head of Accounting), e1002 (Accountant)
  decide    travel_agent (manager when the agent doubts): e1101 (Manager of Section 1A)
            manager: e1101 (Manager of Section 1A); when the agent doubts, done with the approval of `precheck` where no other stage comes between them
            dept_head: e3101 (Head of Sales Department 1) [e1101 (Manager of Section 1A) too, as the proxy of e3101]
            division_head: e0101 (Head of Sales Division)
```

`--form` gives the values of the form, and only the stages of its row are shown; `--every-requester` shows every person of the organization, the ones who come to the same approvers together.

## Running the scenarios

A scenario is a table of events at their times, with where the ticket should be after each.
`run` gives them to the ticket machine, which reads the lowered rules for the stages, the lowered dates for the deadlines and the books for the balances, and shows what it did:

```console
$ ritsu ringi run examples/expense.ringi --scenario a_joint_purchase
2026-11-16 10:00  submit   by e1201  -> first (e1101), due 2026-11-19 end of day; hold budget.reserve 1_200_000 JPY
2026-11-16 15:00  approve  by e1101  -> second (dept_head: e3101; accounting: one of e1001, e1002), due 2026-11-19 end of day
2026-11-17 11:00  approve  by e1001  -> second (dept_head: e3101), due 2026-11-19 end of day
2026-11-17 16:00  approve  by e3101  -> decide (e0101), due 2026-11-20 end of day
2026-11-18 09:30  approve  by e0101  -> approved; launch pay; post budget.reserve
the book `budget` at the end: dept_budget(sales1) 3_800_000 JPY, allocations -5_000_000 JPY, payees 1_200_000 JPY
  the hold `reserve` (T-1, attempt-1): posted
scenario `a_joint_purchase`: as written (5 rows)
```

A leave sent back with a comment, talked over and submitted again; the leave book's days are held, let go and held again:

```console
$ ritsu ringi run examples/leave.ringi --scenario a_leave_sent_back_with_a_comment
2026-11-16 09:00  submit                             by e1203  -> manager (e1101), due 2026-11-18 end of day; hold leave.take 3 day
2026-11-16 11:00  comment                            by e1101  -> manager (e1101), due 2026-11-18 end of day; comment "Who covers for you while you are away?"
2026-11-16 13:00  comment                            by e1203  -> manager (e1101), due 2026-11-18 end of day; comment "e1201 does"
2026-11-16 17:00  send back                          by e1101  -> with requester, resubmit by 2026-11-23 end of day; comment "Write e1201 as the cover, please"; void leave.take
2026-11-16 17:30  comment                            by e1203  -> with requester, resubmit by 2026-11-23 end of day; comment "Where do I write the cover?"
2026-11-16 17:45  comment                            by e1101  -> with requester, resubmit by 2026-11-23 end of day; comment "In the cover field of the form"
2026-11-17 09:00  resubmit (cover = person "e1201")  by e1203  -> manager (e1101), due 2026-11-19 end of day; comment "e1201 is written as the cover"; hold leave.take 3 day
2026-11-17 10:00  approve                            by e1101  -> approved; comment "Have a good trip"; post leave.take
the book `leave` at the end: leave_left(e1203) 17 day, granted -20 day, taken 3 day
  the hold `take` (T-1, attempt-1): voided
  the hold `take` (T-1, attempt-2): posted
scenario `a_leave_sent_back_with_a_comment`: as written (8 rows)
```

A leave rejected with a reason:

```console
$ ritsu ringi run examples/leave.ringi --scenario a_leave_rejected_with_a_reason
2026-11-16 09:00  submit   by e1203  -> manager (e1101), due 2026-11-18 end of day; hold leave.take 8 day
2026-11-16 17:00  approve  by e1101  -> head (e3101), due 2026-11-18 end of day
2026-11-16 18:00  reject   by e3101  -> rejected; comment "The week before the year end is our busiest"; void leave.take
the book `leave` at the end: leave_left(e1203) 20 day, granted -20 day, taken 0 day
  the hold `take` (T-1, attempt-1): voided
scenario `a_leave_rejected_with_a_reason`: as written (3 rows)
```

A contract sent back for the counterparty's draft, which does not come before the days to submit it again run out:

```console
$ ritsu ringi run examples/contract.ringi --scenario a_draft_that_does_not_come
2026-11-16 10:00  submit         by e1201  -> manager (e1101), due 2026-11-19 end of day
2026-11-16 11:00  approve        by e1101  -> summary (flow summarize), due 2026-11-19 end of day; ask flow summarize
2026-11-16 11:05  note ready               -> intake (one of e9001, e9002, e9003), due 2026-11-19 end of day
2026-11-16 14:00  send back      by e9002  -> with requester, resubmit by 2026-11-30 end of day; comment "Attach the draft the counterparty sent: we check what both sign"
2026-12-01 00:00  resubmit late            -> expired; expired (resubmit late)
scenario `a_draft_that_does_not_come`: as written (5 rows)
```

`vectors` writes the machine's transitions, from the scenarios and from what is made of them (the edges of the quorums and of the form, every person as the requester, the proxies, a ticket across the versions of the organization), one JSON object a line; the generated code and a server, given the same, answer the same.

## The pages for people

`ritsu ringi doc` writes a request as one page in the words of the people who approve and manage the requests: the days it can be used on, who may submit it, its form, its route case by case, how its approvers are chosen, its deadlines with dates, the rules of its ticket, its books and flows, its claims, examples and scenarios.
`--longest` shows the longest a ticket can take, day by day, and what shortens it.
The Markdown is for a pull request; the HTML is one file that reads nothing from outside.
[The page of the expense request](tests/golden/doc/expense.md) and [its longest ticket](tests/golden/doc/expense.longest.md) are the example's.

`ritsu ringi diff` says where two versions of a request say or answer differently: the cases whose route changes, the requesters whose approvers change, and how each change reaches the tickets in progress ([a row of the route changed](tests/golden/diff/expense_row.txt)).
`--versions` holds one request to two versions of the organization:

```console
$ ritsu ringi diff --versions 2026-04-01 2027-04-01 examples/expense.ringi
  The request `expense` (`crates/ringi/examples/expense.ringi`), read in the organization in effect on 2026-04-01 (the version from 2026-04-01) and on 2027-04-01 (the version from 2027-04-01).
Approvers:
  The approvers change for 4 of the 23 requesters both versions have.
    e1201 (Sales staff 1), precheck: check (manager when the agent doubts): e1101 (Manager of Section 1A) → check (manager when the agent doubts): e3101 (Head of Sales Department 1)
…
  And 12 more: `--format json` lists them all.
Requesters:
  - Only in the version from 2026-04-01: e9001 (General counsel)
Stages nobody answers:
  How many requesters each stage finds nobody for, by what happens then as the request says, where the two versions differ:
    second (dept_head), goes up or elsewhere: 14 → 15
    second (dept_head and accounting), goes up or elsewhere: 14 → 15
    decide (dept_head), goes up or elsewhere: 14 → 15
Tickets in progress:
  A stage whose approvers are decided keeps them on and after 2027-04-01; each stage that starts on or after 2027-04-01 takes its approvers from the version from 2027-04-01. Where an approver a stage waits for is no longer in the organization, an administrator hands the ticket over; a late stage goes up as the version in effect then says.
```

For a change in a pull request, `ritsu impact` and `ritsu mod diff` say the same of the requests, beside what the other languages' files answer.

## Lowered to the other languages

What a request writes inside it is lowered to files of the languages that check such things: the route's table and the conditions to rules of rulec, the deadlines and reminders to a dates file of koyomi, and the joining of the request to the ticket machine to a module of kaname.
The check keeps them in memory; `extract` writes them out, to read them or to move what is written inside into files of their own:

```console
$ ritsu ringi extract examples/expense.ringi --out lowered
wrote lowered/choices.rule
wrote lowered/form.rule
wrote lowered/next_up.rule
wrote lowered/route.rule
wrote lowered/terms.cal
wrote lowered/values.rule
```

[tests/golden/lowered/expense](tests/golden/lowered/expense) holds them.

## Across the languages

`ritsu check` holds a request to the other files it reads:

- **The route and the gate** (E911, W911): every operation on a ticket has an action in the gate the request reads, the gate lets the approvers the route chooses approve and reject, and it lets a requester decide their own request only where the route allows self approval.
- **The hold and the book** (E209, W209): what a ticket holds when it is submitted does not expire before it is posted or let go, on the longest path of the route counted on the calendar, and every requester who may submit it has the values the hold reads.
- **The ticket and the workflows** (E210, W210): what a ticket hands a workflow fits the workflow's inputs, an agent's workflow answers with `verdict` and `reason`, and a workflow that reads the requests' API reads the one the requests are now.

## The code it generates

`ritsu gen --target typescript` (and `python`, `go`, `rust`) writes, beside the code of the rules, dates, books, gates and flows a request reads, the code of the files it is lowered to, the ticket machine (a module of kaname), and a list of the kinds of request with `start` and `next` entries that take and return JSON, so that one server runs every kind with none of their types.
A JSON outside the contract given to `start` or `next` is refused at the entry, before the machine runs, with one sentence the four languages write alike: where it is, what was given and what is needed (``The argument `submit.form.days` of `start` is "three", and must be an integer``).
A field of any of ritsu's types with a unit (an amount, a rate, a quantity) is generated, computed fields too, and the screens show it in the steps of its unit.
`ritsu gen --target requests` writes what a server and its screens read, once for the project: the kinds of request, the server's API as OpenAPI 3.1, the tools an agent is given, the screens as A2UI 1.0 messages with ritsu's catalog and the functions they call, the schema of the organization's data, and the vectors.
With `--extra wasm`, it builds the code of Rust into one module of WebAssembly, `requests/kinds.wasm`, which a server loads to run every kind with no build of its own; with `--extra bundle`, it puts `requests/` and the files the kinds are made from in one zip, `requests.zip`, and with `--diff-from` the pages of `ringi diff` against the bundle before.
[docs/reference.md](docs/reference.md#what-gen-writes) has what each file holds and what the server adds.

## The server

[ringi-server](https://github.com/i2y/ringi-server) is a framework in Go that runs what ritsu generates.
It keeps the tickets in PostgreSQL, signs people in, rings the clocks, and serves the pages people use in a browser, the API and the agents' tools, with the same code for every kind of request.
`ritsu ringi init` writes the first files of a project on it: one request type, the code `ritsu gen` makes of it, a server and its tests.

```sh
ritsu ringi init server --module example.com/requests
cd server
go mod tidy
go run ./cmd/server -dev-login
```

Then open http://127.0.0.1:8080/pages/ and choose who you are.
[Its site](https://i2y.github.io/ringi-server/) has a tutorial that goes on from here, guides for what you want to do, and how to run the server for your staff.

## For AI agents

[skills/ringi](../../skills/ringi/SKILL.md) is a skill for an agent that writes or changes a request, an organization or masters: the loop, what to ask the people who run the requests, and how to fix each diagnostic, ringi's and ritsu's across the languages.
Its pages besides `SKILL.md` are copies of `docs/`, made by `skills/sync.sh`.

## Install

ringi comes with ritsu:

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

Run it as `ritsu ringi …`.
Called through a link named `ringi`, ritsu is ringi, with the languages a request is lowered to and reads joined; the archives of a release carry the link, and `ln -s ritsu ringi` beside the binary makes one.
The binary of ringi's own crate joins no other language: it stops with E008 at a request, which is lowered to rulec, koyomi and kaname.

## Commands

```console
$ ritsu ringi check examples
$ ritsu ringi check examples/expense.ringi --at 2027-04-01
$ ritsu ringi simulate examples/leave.ringi --every-requester --on 2026-11-16
$ ritsu ringi run examples/leave.ringi
$ ritsu ringi vectors examples/leave.ringi
$ ritsu ringi api examples/leave.ringi
$ ritsu ringi doc examples/expense.ringi --format html
$ ritsu ringi diff --versions 2026-04-01 2027-04-01 examples/leave.ringi
$ ritsu ringi extract examples/leave.ringi --out lowered
$ ritsu ringi init server --module example.com/requests
$ ritsu ringi explain E301
```

Every command takes `--lang ja|en` (else `RINGI_LANG`, then `RITSU_LANG`, else English) and `--help`.
`check` takes `--at <date>` to read one version of the organization, `--through <date>` to say a request cannot be used through a day, and `--org <dir>` to read other data than the `data` line names.
The exit code is 0 for no errors, 1 for errors (for `diff`, for differences), and 2 for bad arguments, a file that cannot be read, or a language that is not joined.
[docs/reference.md](docs/reference.md#commands) has every command and flag.

## Examples

Each is in English, and with Japanese names beside it as `<name>.ja.ringi`; they read the organization, the masters, the calendar, the books, the gate and the flows under [examples/shared](examples/shared), and the organization's data under [examples/org](examples/org) and the masters' rows under [examples/masters](examples/masters):

- [examples/expense.ringi](examples/expense.ringi): an expense, a trip or a purchase, checked, approved and decided by the roles its category and amount come to, with an agent that checks expenses up to 1,000,000 yen and decides trips up to 30,000 yen; the budget is held when it is submitted and paid by a workflow once it is approved.
- [examples/purchase.ringi](examples/purchase.ringi): a purchase reviewed and decided by the people the company's table of authority names for its kind and amount, and ordered by a workflow once it is approved.
- [examples/leave.ringi](examples/leave.ringi): paid leave, sick leave or special leave, approved by the manager, and by the head of the department for more than five business days; paid leave takes its days from the leave book.
- [examples/account.ringi](examples/account.ringi): access to a company system, approved by the manager and the owner the list of systems gives, and by IT security for administrator access; IT operations see it once it is approved, and a workflow sets the account up.
- [examples/contract.ringi](examples/contract.ringi): a contract reviewed by legal: an agent summarizes it for the reviewers, a lawyer takes it in and names the reviewer, two lawyers review one over 5,000,000 yen, and the general counsel decides one over 10,000,000 yen.

## How it is checked

`cargo test -p ringi` runs the language on its examples and its tests.

- The five requests of the examples, in English and with Japanese names, and the shared organization, masters and route pass the check, and say the same at the same places in both; in the two versions of the organization they say only what the data of a version makes them say.
- Every diagnostic code has the smallest file that prints it, in English and in Japanese, and the fixes its notes show pass the check when written in.
- The ticket machine runs the scenarios of the examples and of 25 requests written for it, each as written, in English and in Japanese; for every scenario's form, it takes every person of both versions as the requester to the end of the ticket, and does what `simulate` and the check say, in no more business days than the check counts.
- The code `ritsu gen` writes in TypeScript, Python, Go and Rust, and the module of WebAssembly `--extra wasm` builds of it, answer the 11,141 vectors of the examples and the machine's requests as the machine does (ritsu's `tests/gen_requests.rs`); the code passes `tsc --strict`, `mypy --strict`, `go vet` and `cargo clippy -- -D warnings`.
- The screens are validated against A2UI 1.0 and ritsu's catalog, and the functions they call answer the vectors under Node as the machine does.
- The pages of `doc` and `diff` are golden files, and the lowered files of each example are golden files that pass the checks of their languages.
- The checks across the languages each have the shapes that hold, that do not and that cannot be decided, as golden files in English and in Japanese (ritsu's `tests/cross.rs`).
- The outputs, diagnostics and lines of `.ringi` and CSV in this README, `docs/` and the skill are real: `tests/docs.rs` compares them, and ritsu's `tests/ringi.rs` runs the commands.

## Status

The language, `check`, `simulate`, `run`, `vectors`, `api`, `doc`, `diff`, `extract`, `init` and `explain`, the checks across the languages, and the code `ritsu gen` writes are done, as [DESIGN.md](DESIGN.md) says.
Not yet: a circulation that waits until everyone has read it, reminders on chosen days or repeated, one condition for several fields at once, a team looked up in a master, an agent's help for the requester, a workflow that waits for the end of a ticket it opened, and attachments on comments ([docs/reference.md](docs/reference.md#what-the-language-does-not-have) has the whole list).
The server is outside ringi: it keeps the tickets, rings the clocks, tells people, serves the screens and authenticates, as the contract `--target requests` writes says.

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or [MIT license](../../LICENSE-MIT), at your option.
