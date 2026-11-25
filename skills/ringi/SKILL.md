---
name: ringi
description: Write, check and fix ringi files (`.ringi`), a company's applications and approvals — a kind of request of any department (an expense, a purchase, a leave, an account on a system, a contract to review) with its form, its route and who approves each stage, its deadlines and reminders, and what is done in the books and the workflows as it ends, with the organization and the masters it reads — checked for every requester in every version of the organization, and generated as a ticket machine in TypeScript, Python, Go and Rust and as the API, the agents' tools and the screens a server serves. Use when an approval procedure written in words has to become a request type; when a ringi diagnostic (E001-E704, W108-W901) or ritsu's E209-W210 and E911-W911 has to be fixed; when who approves a person's request, or what a change moves, has to be shown to the people who run the requests; or when what a server of requests runs on has to be generated.
compatibility: Requires the `ritsu` binary on PATH (`cargo install --git https://github.com/i2y/ritsu --locked ritsu`); run ringi as `ritsu ringi <command>`, or as `ringi <command>` through a link to ritsu named for it. A server keeps the tickets, the clocks, the notices and the screens, from what ritsu generates.
license: MIT OR Apache-2.0
---

## When this applies

The job is **a kind of request and how it is approved, written where it can be checked**: the form
a requester fills in, the route the request takes and who answers each stage of it, the deadlines
and the reminders, who may submit it, and what is done in the books and the workflows as it ends.
Any department's requests are written the same way: accounting's expenses, general affairs'
purchases, HR's leave, IT's accounts, legal's contracts, and the rest. ringi checks the request
against the organization's data, for every person who may submit it, in every version of the data,
and `ritsu gen` writes from it what a server runs on.

It does not apply to the server itself (keeping the tickets, signing people in, sending mail and
serving the screens: [ringi-server](https://github.com/i2y/ringi-server), a framework in Go, does
that from what ritsu generates, and has its own documents), to a workflow
that runs after the approval (that is a flow of dandori the ticket launches), to the company-wide
rules of who may do what (that is the gate of sekisho the request reads), or to the balance of a
budget or of the days of leave (that is a book of chobo).

The files bundled with this skill are `reference.md` (the whole language and the commands) and
`codes.md` (every diagnostic). Read them when you need them, not all up front.

---

# Working with ringi

Your part is to turn the procedure a person describes into a request type, get it past `ritsu
ringi check` and `ritsu check`, try it on real people and real requests, and show it to the people
who run the requests until they say it is right. Three things stay with people:

1. **Who approves what.** The rows of the route, the ways approvers are chosen, the quorums, who
   may submit and self approval are the company's decisions, usually from its rules of authority.
   Never add a row, move a bound, change an approver or allow self approval to make the check pass.
2. **The organization's data.** Who is in which team, who heads it and who is whose proxy come from
   the people who keep that data. Never edit a version of the data to make a stage find someone.
3. **That a warning is meant.** A stage that goes elsewhere when its seat is empty (W502), one
   person at two stages of a ticket (W506), a late stage with nowhere to go up to (W507) are facts
   of the organization; ask whether each is meant, and change nothing until someone says.

## 1. The loop

1. Read what is there: the organization's file and its data, the masters, the calendar, the books,
   the gate and the flows the request will read, and the rules the procedure comes from.
2. Write the request type: the head and the `use` lines, the form, the route, `who`, `every stage`,
   `ticket`; then the claims, the examples of the route and the scenarios, from what the person said.
3. Run `ritsu ringi check` and fix what it says, one diagnostic at a time, from the top; `ritsu ringi
   explain <code>` says how. Then `ritsu check` on the project, which adds the checks across the
   languages (the gate, the books, the flows).
4. Try it: `simulate` with the people and the values of real requests, and `run` for the scenarios.
5. Show it to the people who run the requests (section 6), and change the file, never the page.
6. Generate what the server runs on (`ritsu gen --target <language> --target requests`), and have
   CI run the same `gen` with `--check`.
7. When the request changes, raise its version (`v2`): a ticket finishes with the version it was
   submitted with. Show the change with `diff`.

```console
$ ritsu ringi check examples/expense.ringi
$ ritsu ringi explain E301
$ ritsu ringi simulate examples/expense.ringi --every-requester --on 2026-11-16
$ ritsu ringi run examples/expense.ringi
$ ritsu ringi doc examples/expense.ringi --format html
$ ritsu ringi diff --versions 2026-04-01 2027-04-01 examples/expense.ringi
```

Run every command as `ritsu ringi <command>` (through a link to ritsu named ringi, `ringi
<command>` is the same): a request is lowered to rulec, koyomi and kaname and reads the calendar,
the books and the flows in the same process, where ringi built alone from its crate holds none of
them and stops with E008. Add `--lang ja` for Japanese.

## 2. From what a person says to the file

Each thing people say about a procedure has its place:

| They say | You write |
|---|---|
| "up to 100,000 yen the manager decides; above it the head of the department, after the manager" | a row of the route's table for each case, a column for each stage, and in each cell who answers it (a blank cell where those requests skip the stage) |
| "the manager", "the head of the department", "accounting", "the nearest head of division above" | a line of `who` for each: `manager of requester`, `head of requester.department`, `team accounting`, `nearest division_head above requester` |
| "both the head of the department and accounting" | `dept_head and accounting` in one cell |
| "if there is no manager, the one above" | `when empty escalate` (and `then <way>`, `then skip` or `then stay` for the top of the company) |
| "if nobody answers in three days, it goes up, and in the end it lapses" | `due in 3 business days at end of day` and `when late escalate, then expire` under `every stage` (or under one stage) |
| "nobody approves their own request" | `self approval  not allowed` |
| "the budget is reserved when it is submitted" | `on submitted hold …`, with the `post` on approval and the `void` on every other ending |
| "then accounting pays it" | `on approved launch <flow>(…)` |

The route of the expense request of the examples: each row is a case of the form, and a stage the
requests of a row skip has a blank cell. `who` says how each name in a cell finds its approver, and
`every stage` the deadline, the reminder, and where a stage goes when it is late or its seat is
empty:

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

every stage
  due in 3 business days at end of day
  remind after 2 business days at 09:00
  when late escalate by next_up, then expire
  when empty escalate, then board
```

A route can also be stages a line each, with a condition on a stage and what to do when its seat is
empty, as the purchase request writes it:

```ringi
route
  # The president has no manager: the stage is skipped for them.
  first     manager
    when empty escalate, then skip
  technical reviewer               when reviewer is not none
  …
  board     at least 60% of board  when authority.board
  …
  decide    authority.decider
    when empty escalate, then board
  inform    circulate to general_affairs
```

What the person said is also what the file is held to: write it down as claims, examples of the
route and scenarios, so that the check fails when the route stops saying it.

```ringi
claims
  the_agent_decides_up_to_30000_yen : decide is travel_agent only when amount <= 30_000JPY
  a_joint_second_stage              : possible second is dept_head and accounting
  reminder_before_due               : every stage reminds before it is due
  ends_in_time                      : every ticket ends within 93 business days

examples
| category | amount       | -> precheck | first   | second                   | decide        |
| travel   | 30_000JPY    |             |         |                          | travel_agent  |
| travel   | 30_001JPY    |             |         |                          | manager       |
…
```

A scenario is a story someone told, as the events of one ticket hour by hour, and where the ticket
is after each (the leave request's):

```ringi
scenario a_long_paid_leave
  requester e1203
  given leave.grant(person: "e1203", year: "2026", days: 20)
  form kind = paid, first_day = 2026-12-21, days = 8, reason = "A family visit"
| at               | event   | by    | -> stage | status      |
| 2026-11-16 09:00 | submit  | e1203 | manager  | in progress |
| 2026-11-16 17:00 | approve | e1101 | head     | in progress |
| 2026-11-17 09:00 | remind  |       | head     | in progress |
| 2026-11-19 00:00 | late    |       | head     | in progress |
| 2026-11-19 10:00 | approve | e0101 |          | approved    |
```

A request in Japanese writes its names in Japanese, each with the ASCII name the generated code
uses in brackets; the keywords stay as they are. This one takes its route from a shared route,
given the condition the shared route asks for:

```ringi
request 休暇(leave_ja) v1 "休暇の申請"
…
use route 標準(standard) from "shared/manager_approval.ringi"
…
form
  種類(kind) : choice "休暇の種類"
    有給(paid)    "有給休暇"
    病気(sick)    "病気休暇"
    特別(special) "特別休暇"
  初日(first_day)  : date "初日"  range >=2026-01-01 <=2027-12-31  open in 本社
  日数(days)       : number "営業日の日数"  range >=1 <=20
  復帰日(back_on)  : date "出社する日" = 初日 + 日数 business days in 本社
…
route 標準(long: 日数 > 5)
```

**The organization** is a file of its own: its positions from the lowest, its kinds of team from the
widest, and the folder of its data. Each version of the data is a folder named by the day it takes
effect (`org/2026-04-01/`, `org/2027-04-01/`), with `people.csv`, `teams.csv` and `members.csv`;
`proxies.csv` beside the versions says who may act for whom, when, for which kinds of request:

```ringi
organization company v1 "Company"
…
positions
  staff         "Staff"
  manager       "Manager"
  dept_head     "Head of department"
  division_head "Head of division"
  president     "President"

team kinds
  company    "Company"
  division   "Division"
  department "Department"
  section    "Section"
  committee  "Committee"

data "../org"
```

The data of a version: the teams with their kind, the team above and the head; the people; and who
is in which team at which position (`primary` is `yes` for a person's main team):

```csv
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e0001
sales_div,Sales Division,営業本部,division,company,e0101
sales1,Sales Department 1,営業一部,department,sales_div,e3101
```

```csv
person,team,position,primary
e0001,company,president,yes
e0001,board,president,no
e0101,sales_div,division_head,yes
```

`people.csv` has `id,name,name_ja,email,language`. Who may act for whom, beside the versions:

```csv
person,proxy,kind,from,to,requests
e3101,e1101,approve,2026-12-28,2027-01-08,expense expense_ja purchase purchase_ja
e1203,e1201,submit,2026-12-01,2026-12-31,leave leave_ja
```

Ask the people who keep the organization's data for these files; when they keep it in another
system, `organization/schema.json` and `columns.md` from `ritsu gen --target requests` say what a
server takes when the data is imported.

**The masters** (the lists a form chooses from: account codes, vendors, systems) are a `masters`
file, with a CSV for each list whose rows say from when and until when each may be chosen; a field
chooses from it with `choice from m.accounts`, and E107 says the days on which a list has nothing:

```ringi
master accounts "Account codes"
  key code
  fields
    code  : text "Code"  count <=10
    label : text "Name"  count <=60
  data "../masters/accounts.csv"
```

```csv
code,label,valid_from,valid_to
5101,Travel,2026-01-01,
```

**The gate** is a sekisho file every request reads (`use gate`). ringi hands it, for each stage,
the requester, the approvers the route resolved and whether the route allows self approval; the
gate says that only those approvers answer the stage, and that no one decides their own request
unless the route allows it. From the example's gate (`examples/shared/company.gate`, a resource `Stage` with `requester`, `approvers` and
`self_approval`):

```gate
permit approvers_answer_their_stage
  description "The approvers the route resolved for a stage answer it"
  principal is Staff
  action approve, send_back, reject, pause, retract
  when principal in resource.approvers

forbid no_one_decides_their_own
  description "No one approves, sends back or rejects their own request, unless its route allows it"
  principal is Staff
  action approve, send_back, reject
  when resource.requester is principal
  unless resource.self_approval
```

Start from that gate, or from the one `ritsu ringi init` writes; `ritsu check` holds it to the route
(X18, E911). Ask before you change who may do what in it: the gate is the company's, and every
request reads it.

## 3. The language on one page

- A request is one file, its blocks in this order (E001 otherwise): the heading with `description`,
  `owner` and `effective from`, `use` lines, `form`, `route`, `who`, `every stage`, `table`s,
  `ticket`, `claims`, `examples`, `scenario`s, `labels`. `form`, `route` and `ticket` are always there.
- Every number, amount and date of the form has a range (E101), every text a limit on its length
  (E102), every list of rows a most (`count >=1 <=20`). The types with a unit are ritsu's
  (`money[JPY]`, `rate[step 5%]`, `mass[kg]` and the other quantities), and a computed field can be
  of any of them; an amount divided, or multiplied by a rate, says how it rounds (E104).
- A name is ASCII, or a name in another language with its ASCII name in brackets right after it
  (`金額(amount)`): the generated code and the JSON take the ASCII name. The ASCII name of a kind of
  request starts with a letter, ends with a letter or a digit and has no two `_` in a row (E002): it
  begins the names of the files it is lowered to and of the generated code. `requester` and `ticket`
  name no field, and a reserved word of a language the code is generated in names nothing (E002).
- Every request comes to exactly one row of the route's table: a request no row takes is E301, one
  two rows take is E302. The cells of the table and the conditions are lowered to rulec, which proves
  both for every value of the form.
- A condition of the route and the form's `shown when` compare a field of the form with a value
  (E309). They never read the requester, the ticket, a text, two fields with each other or a
  computation: put a comparison in a computed field, make a text a `choice`, say who may submit with
  `submitted by`, and where the top of the company goes with `when empty escalate, then <way>`.
- A deadline counts business days of a calendar, and a reminder comes before it (E403). The check
  says the most business days a ticket can take, and a claim can hold it (`every ticket ends within
  93 business days`).
- What a ticket holds in a book on submission is posted on approval and let go on every other ending
  (E405, E406, E407), keyed by `ticket.id` and `ticket.attempt` (E408):

```ringi
  on submitted      hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: requester.department, amount: amount)  when requester.department is not none
  on approved       post budget.reserve(request: ticket.id, attempt: ticket.attempt)
  on approved       launch order(request_id: ticket.id, vendor: vendor, amount: amount, summary: summary)
  on sent back, rejected, withdrawn, expired  void budget.reserve(request: ticket.id, attempt: ticket.attempt)
```

- A field shown only in some cases (`shown when`) is never required while hidden (E105), and
  `simulate` refuses a value for a field that is not shown.
- `labels ja` gives the Japanese words of a request written in English, field by field and value by
  value; the pages, the screens and the API use them.

[reference.md](reference.md) has every form; [What the language does not have](reference.md#what-the-language-does-not-have)
says what to write instead of the words other tools use.

## 4. What to ask a person

- Who answers each stage, for each case of the form, before writing a row: the rules of authority
  if there are any, and what is done when the seat is empty or the stage is late.
- Where the bounds fall, when they say "up to" or "over": whether 100,000 yen itself is the
  manager's or the head's (E302 shows a request both rows take).
- Who may submit it (`submitted by`), and whether anyone may approve their own request.
- What is held in which book on submission, and which workflow starts on approval.
- Whether each warning about the organization is meant (W502, W506, W507, W510), at the names the
  note lists.
- Whether a version of the organization to come is right, when the check fails only in that version
  (E503, E504 at a date ahead): the data, or the route, is to change before that day.
- Whether one person who answers two stages (a manager who is the head of the department too)
  approves at each, or once: `same approver approves once` with the stage just before, `same
  approver approves once, at the last stage` at the last one, after the stages between.

## 5. From a diagnostic to a fix

The check names the row to add when a case has no row, with its cells cut at the bounds the table
already writes:

```console
$ ritsu ringi check tests/mutants/E301_no_row_over_100000_yen.ringi
error[E301]: tests/mutants/E301_no_row_over_100000_yen.ringi:6:1: No row of the route takes some requests
     6 | route
  = The requests no row takes: amount from 100_001JPY to 1_000_000JPY
  = A request no row takes: amount = 100_001JPY
  = Add this row to the table, writing in place of each `?` who answers that stage (a name in `who`; a blank cell where those requests skip it): | >100_000JPY | ? | ? |
  = The cells are cut only at the bounds the table already writes and at the ends of the ranges, so the rows close the gap as they are; where the approvers change inside a row, split it there.
```

Ask who answers before writing the `?`. Two rows that take one request name it:

```console
$ ritsu ringi check tests/mutants/E302_two_rows_at_100000_yen.ringi
error[E302]: tests/mutants/E302_two_rows_at_100000_yen.ringi:8:1: Two rows of the route take one request (lines 8, 9)
     8 | | <=100_000JPY  | manager   |
  = A request both rows take: amount = 100_000JPY
  = Decide which row is right, and correct the bounds so that they do not overlap.
```

A stage the organization gives nobody, for whom, and what to write for them:

```console
$ ritsu ringi check tests/mutants/E502_escalate_with_nobody_above.ringi
error[E502]: tests/mutants/E502_escalate_with_nobody_above.ringi:9:10: In the organization from 2026-04-01, the stage `check` has no approver
     9 |   check  manager
  = `manager` finds no head above the requester.
  = For 1 requester: e01 (President (fictional)).
  = The stage is `when empty escalate`, and nobody is above (the top of the organization): a ticket that comes to it waits there until an administrator adds an approver.
  = If the stage may be skipped for them, write `when empty escalate, then skip`; if someone must see their requests, write after `then` the way to choose who answers them (`when empty escalate, then board`, with `board = team board` in `who`).
```

Which of the two is right for the president is the company's to say. A quorum some requesters
cannot reach:

```console
$ ritsu ringi check tests/mutants/E504_two_of_a_team_of_two.ringi
error[E504]: tests/mutants/E504_two_of_a_team_of_two.ringi:9:11: In the organization from 2026-04-01, the stage `review` cannot get 2 approvals for some requesters
     9 |   review  at least 2 of sales
  = `sales` has only 1 member besides the requester.
  = For 2 requesters: e02 (Head of Sales) and e03 (Sales staff).
  = Add people to the team, or change the quorum.
```

A list the form chooses from with a gap in its days, and the rows that close it:

```console
$ ritsu ringi check tests/mutants/E107_a_hole_in_the_account_codes.ringi
error[E107]: tests/mutants/E107_a_hole_in_the_account_codes.ringi:6:3: The master `m.accounts` has no row to choose on 2026-07-01 to 2026-07-31
     6 |   account : choice from m.accounts "Account code"
  = The request is used from 2026-01-01 on, and the field `account` reads the master on each of those days.
  = The row before: `tests/mutants/shared/accounts_with_a_hole.csv`, line 2 (`5103`, from 2026-01-01 to 2026-06-30); making its `valid_to` 2026-07-31 fills it.
  = The row after: `tests/mutants/shared/accounts_with_a_hole.csv`, line 3 (`5110`, from 2026-08-01 with no end); making its `valid_from` 2026-07-01 fills it.
```

The usual codes (`codes.md` has every one, with its smallest reproduction):

| Code | What it means | What to do |
|---|---|---|
| E001-E006 | a line, a name or a block the language does not read, a name declared twice or not at all, a `use` that cannot be read | fix the line; E002 says why a name cannot be used |
| E101, E102 | a number, an amount or a date with no range; a text with no limit | write the range and the limit the business has; ask when you do not know them |
| E104 | a computed amount that does not say how it rounds | write the rounding the business uses (`round down(1JPY)`); ask which |
| E105, E106 | a field required while hidden; a condition that reads a field before it is filled | make the field shown whenever it is required; order the fields as they are filled |
| E107, E404 | a master with no row on some day the request is used; a calendar with no data for a day it counts | bring the rows or the data, or start the request later (`effective from`) |
| E201, W201 | a row, a condition or a deadline that does not pass the language it is lowered to | the note says which line; fix it there |
| E202 | a file a `use` reads does not pass its check | fix that file with its language |
| E203 | a value of a rule's output a stage reads that names no approver | add the way to `who`, or ask whether the rule is right |
| E301, E302 | a case no row takes; a case two rows take | ask who answers, and add the row, or move one bound |
| E303, W303 | a row, or an approver, no request reaches | ask whether the row should go or the range is wrong |
| E304, E309 | a quorum larger than the approvers it counts; a condition the route or the form cannot read | ask for the quorum; put the comparison in a computed field |
| E306, E601 | a claim or an example of the route that does not hold | ask whether the route or the claim is wrong |
| E401, W402 | a ticket that may take longer than its claim; a stage that may wait for ever | add a deadline or `when late`, or ask whether the claim is right |
| E403 | a reminder after its deadline | move the reminder before the deadline |
| E405-E408 | a hold an ending leaves open, a post or void with no hold, an ending the ticket never has, a hold with no key of the try | hold, post and void on every ending, keyed by `ticket.id` and `ticket.attempt` |
| E502, W502 | a stage with nobody in a version of the organization; one that goes elsewhere or is skipped | ask whether the data or the way to choose is wrong; never edit the data to pass |
| W500, E501 | no data of the organization, so missing approvers are not decided; data that does not read | bring the data (`data` in the organization's file), or fix the line the note names |
| E503, E504 | a stage that goes up to someone a version does not have; a quorum some requesters cannot reach | ask; the note names the version and the people |
| E505, E508 | a proxy or a master's row for someone not in the organization; `submitted by` naming a team or a person a version does not have | ask the people who keep the data, or correct the name |
| W506, W507 | one person at two stages of a ticket; a late stage with nowhere to go up to | ask whether one approval is meant (`same approver approves once`); ask where a late stage of the top goes |
| E509, W510 | no one may submit in a version; requesters for whom no stage has anyone | ask whether `submitted by` or the route is wrong |
| E602-E604 | a scenario that goes another way than it writes | the ticket is right and the scenario wrong, or the other way round: ask which |
| E701, W702, E703, E704 | a screen sends a value it does not show, shows a number in other steps, has no words in a language, or cannot hold a number's range | fix the field, its `labels`, or its range |
| W109, W901 | a field that asks for a secret; a key written in the file | take it out; a secret is never a field of a request |

`ritsu check` adds ritsu's codes across the languages, which `ritsu explain <code>` looks up:

| Code | What it means | What to do |
|---|---|---|
| E911, W911 | the gate does not say what the route says: an operation of the ticket with no action, an approver the gate does not let approve, a requester who may decide their own request where the route does not allow it | ask whether the gate or the route is right; fix the gate's policies where it is the gate |
| E209, W209 | a hold the ticket makes on submission can expire before the ticket ends; that cannot be decided (a stage with no deadline), or the hold reads a team some requesters do not have | ask about the book's expiry or the stages' deadlines; give every stage a deadline; `when requester.department is not none` on the line where some people have no department |
| E210, W210 | what a ticket gives a flow (`launch`, an agent's stage, a summary) does not fit the flow's inputs, or an agent's flow does not answer `verdict` and `reason`; the other way, a flow that makes a request calls an API written from the request as it was | hold the value to what the flow takes, or widen the flow's input (a person decides which); write the API again with `ritsu gen --target requests` |

Fix the first diagnostic first: an error of words or names stops the checks after it.

## 6. Showing it to the people who decide

The people who run the requests can say "this is right" without reading the file, from these, in
this order:

1. **The page.** `ritsu ringi doc <file> --format html` is the request in their words: who may
   submit it, the form, the route case by case, the deadlines with dates, what happens at each
   ending. Ask the owner of the request to read it against their rules.
2. **Their own people.** `simulate` with the person who asked, and two or three colleagues, with
   the values of a real request. They know who should approve it, and see at once when it is wrong:

```console
$ ritsu ringi simulate examples/expense.ringi --requester e1201 --on 2026-11-16 --form 'category = expense, items = [{what: "Books", price: 150_000JPY}]'
examples/expense.ringi on 2026-11-16, in the organization from 2026-04-01
  computed: amount = 150_000JPY
requester e1201 (Sales staff 1)
  precheck  check (manager when the agent doubts): e1101 (Manager of Section 1A)
            due 2026-11-19 at the end of the day, reminder on 2026-11-18 09:00
  first     manager: e1101 (Manager of Section 1A)
            due 3 business days after it starts, at the end of the day
  decide    dept_head: e3101 (Head of Sales Department 1)
            due 3 business days after it starts, at the end of the day
```

   `--every-requester` shows everyone at once, the people who come to the same approvers together;
   a value for a field the form hides is refused:

```console
$ ritsu ringi simulate examples/expense.ringi --requester e1201 --on 2026-11-16 --form 'category = expense, items = [{what: "Books", price: 150_000JPY}], trip_to = "Osaka"'
error: `trip_to` is not shown for this request (its `shown when` does not hold), and takes no value
```

3. **The stories they told.** `run` plays each scenario hour by hour: who it waits for, when it is
   due, what was held in the books and which flow started.
4. **A change.** `diff <old> <new>` says which cases' route changes and whose approvers change,
   and how the tickets in progress are affected; `diff --versions <day> <day> <file>` does the same
   for a change of the organization. `--longest` on `doc` shows the longest a ticket can take.

Change the file, never the page, and show the page again. What a person confirmed is kept with
yuen; `ritsu yuen review` is run by that person.

## 7. What it generates, and what the server adds

`ritsu gen --target typescript|python|go|rust` puts, beside the code of the rules, dates, books,
gates and flows a request reads, the code of the files it is lowered to, the ticket machine, and a
list of the kinds of request with `start` and `next` entries that take and return JSON. One server
runs every kind of request with none of their types; a call outside the contract is refused at the
entry with one sentence alike in the four languages.

`ritsu gen --target requests --catalog <url>` writes what a server and its screens read, once for
the project: the kinds of request, the server's API as OpenAPI 3.1, the tools an agent is given,
the screens as A2UI 1.0 messages with ritsu's catalog and the functions they call, the schema of
the organization's data, and the vectors a server is tested with. The catalog's ID is a URL you own,
the same in every environment (or the `catalog` line of `ritsu.mod`).

The server keeps the ticket as `next` returned it and its history, takes each event once by its
idempotency key, asks the gate before `next`, rings the clocks, keeps the books in the ticket's
transaction, tells people and starts flows after saving, hands over the part of the organization in
effect, serves the screens, and authenticates. [What the server adds](reference.md#what-the-server-adds)
has the whole table. [ringi-server](https://github.com/i2y/ringi-server), a framework in Go, does this,
and `ritsu ringi init <dir> --module <path>` writes the first files of a project on it: a request type,
its organization and gate, a server and its tests. When someone has no server yet, start there. The
project's README says how to run it, and ringi-server's documents say what the project gives the server
and how to run it for people.

## 8. What the language does not have

One word means one thing across ritsu's languages, so some words are not written: a stage is a
`stage` (not `step`), going up is `escalate` (not `up`), starting a flow is `launch` (not `start`),
taking an approval back is `retract approval` (not `cancel`), a file is an `attachment`, the texts in
another language are `labels`, a claim is `only when` or `possible` (not `never`, `always`), and a
group of people is a `team`. Write these, whatever words the person uses.

When a person asks for one of these, say that ringi does not write it, and what it writes instead:

- **A form or a route built on a screen**, or a route imported from a spreadsheet: never. An agent
  writes the `.ringi`, and a person reads its page.
- **A condition on the requester, the ticket or two fields in the route**: `submitted by` says who
  may submit, `when empty escalate, then <way>` where the top of the company goes, and a computed
  field holds a comparison.
- **A circulation that waits until everyone has read it, reminders on chosen days or repeated, a
  field held from the requester, one condition for several fields, a team looked up in a master, an
  approver adding people to circulate to, a flow that waits for a ticket it opened, the
  organization imported through another system's API**: not in this version. Write what comes
  closest (`circulate to`, one reminder, a condition on each field) and tell the person what is
  left out.
- **Running the requests on an outside workflow engine**: never. The ticket machine is ritsu's code,
  and a workflow after the approval is a flow of dandori, started with `on approved launch …`.
- **The server's part** (keeping the tickets and the attached files, the clocks, mail and chat,
  single sign-on, keeping the audit log, electronic signatures): outside ringi, in the server.

The pages on the site, <https://i2y.github.io/ritsu/ringi/>, go through writing a request type, what
the check guarantees, reading its pages, and what is generated and what the server does.
