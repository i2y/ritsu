# Reading a request type

*For the administrator who looks after a kind of request, and for an approver who wants to know
where a request goes and why. Nothing here asks you to read a `.ringi` file.*

A `.ringi` file is written by an agent and checked by ringi. What a person checks is whether it is
the company's rule: who answers each stage, for whom, and by when. ringi draws that on a page, and
answers three questions about it: who answers a given person's request, what changes when the file
changes, and what changes when the organization does.

## The page of a request type

`ritsu ringi doc` draws a request type on one page, in English or in Japanese (`--lang ja`): the
form, the route as a table of who answers each stage and when, the deadlines and what happens when
one passes, what people may do with a ticket, the claims and what was shown of them, and the
scenarios. The pages of the examples, as `ritsu ringi doc <file> --format html` draws them:

| Request type | Department | The page |
|---|---|---|
| An expense, a trip or a purchase | accounting | [expense](doc/expense.html) |
| Leave | HR | [leave](doc/leave.html) |
| A purchase over the table of authority | general affairs | [purchase](doc/purchase.html) |
| A contract to sign | legal | [contract](doc/contract.html) |
| Access to a system | IT | [account](doc/account.html) |

A file several requests share has a page too. The page of a shared route lists the requests that
take it; the page of the organization, its positions, its kinds of teams and its versions; the page
of the lists, each list with its rows: [the route by the manager](doc/manager_approval.html),
[the organization](doc/company.html), [the lists](doc/masters.html).

Without `--format html`, the page is Markdown, which a repository shows as it is
(`ritsu ringi doc leave.ringi > leave.md`).

## Who answers a person's request

`ritsu ringi simulate` picks a requester, the values of the form and a day, and says who answers
each stage and when the first one is due. Here, a member of the sales staff asks for eight days of
paid leave:

```console
$ cd crates/ringi/examples
$ ritsu ringi simulate leave.ringi --requester e1203 --form kind=paid,first_day=2026-12-21,days=8 --on 2026-11-16
leave.ringi on 2026-11-16, in the organization from 2026-04-01
  computed: back_on = 2027-01-06
requester e1203 (Sales staff 3)
  manager  manager: e1101 (Manager of Section 1A)
           due 2026-11-18 at the end of the day, reminder on 2026-11-17 09:00
  head     dept_head: e3101 (Head of Sales Department 1)
           due 2 business days after it starts, at the end of the day
```

With `--every-requester`, it goes through every person of the organization, and puts together the
ones whose requests come to the same approvers. It says who may not submit the request at all, and
whose stages go elsewhere because nobody is there to answer:

```console
$ cd crates/ringi/examples
$ ritsu ringi simulate leave.ringi --every-requester --on 2026-11-16
leave.ringi on 2026-11-16, in the organization from 2026-04-01
  without `--form`: every stage the route can come to, by each way to choose its approver; which come depends on the form
  5 people may not submit it (`submitted by everyone except team board`): e0001 (President (fictional)), e0201 (Director 1), e0202 (Director 2) and 2 more
requesters e0101 (Head of Sales Division), e0102 (Head of Corporate Division) (2 requesters)
  manager  manager: e0001 (President (fictional)); when it is late, it cannot go up, and expires at the deadline
  head     dept_head: no team of the kind `department` is above the requester; it goes up to e0001 (President (fictional)); done with the approval of `manager` (`same approver approves once`)
requesters e1001 (Head of Accounting), e4001 (Head of Finance), e5001 (Head of General Affairs) and 3 more (6 requesters)
  manager  manager: e0102 (Head of Corporate Division)
  head     dept_head: it comes to the requester alone; it goes up to e0102 (Head of Corporate Division); done with the approval of `manager` (`same approver approves once`)
requester e1002 (Accountant)
  manager  manager: e1001 (Head of Accounting)
  head     dept_head: e1001 (Head of Accounting); done with the approval of `manager` (`same approver approves once`)
requester e1101 (Manager of Section 1A)
  manager  manager: e3101 (Head of Sales Department 1)
  head     dept_head: e3101 (Head of Sales Department 1); done with the approval of `manager` (`same approver approves once`)
requesters e1201 (Sales staff 1), e1203 (Sales staff 3), e1203 (Sales staff 3) [made by e1201 (Sales staff 1)] (3 requesters)
  manager  manager: e1101 (Manager of Section 1A)
  head     dept_head: e3101 (Head of Sales Department 1)
requester e3101 (Head of Sales Department 1)
  manager  manager: e0101 (Head of Sales Division)
  head     dept_head: it comes to the requester alone; it goes up to e0101 (Head of Sales Division); done with the approval of `manager` (`same approver approves once`)
requesters e7101 (Manager of IT Security), e7201 (Manager of IT Operations) (2 requesters)
  manager  manager: e7001 (Head of Information Systems)
  head     dept_head: e7001 (Head of Information Systems); done with the approval of `manager` (`same approver approves once`)
requester e7102 (Security engineer)
  manager  manager: e7101 (Manager of IT Security)
  head     dept_head: e7001 (Head of Information Systems)
requester e7202 (Operator)
  manager  manager: e7201 (Manager of IT Operations)
  head     dept_head: e7001 (Head of Information Systems)
requesters e9002 (Lawyer 2), e9003 (Lawyer 3) (2 requesters)
  manager  manager: e9001 (General counsel)
  head     dept_head: e9001 (General counsel); done with the approval of `manager` (`same approver approves once`)
```

## What changes when the organization does

The organization's data has a version for each day it changes. `ritsu ringi diff --versions` reads
one request type in two of them, and names every requester whose approvers change, and how:

```console
$ cd crates/ringi/examples
$ ritsu ringi diff --versions 2026-04-01 2027-04-01 leave.ringi
  The request `leave` (`crates/ringi/examples/leave.ringi`), read in the organization in effect on 2026-04-01 (the version from 2026-04-01) and on 2027-04-01 (the version from 2027-04-01).
Approvers:
  The approvers change for 4 of the 18 requesters both versions have.
    e1201 (Sales staff 1), manager: manager: e1101 (Manager of Section 1A) → manager: e3101 (Head of Sales Department 1)
    e1203 (Sales staff 3), manager: manager: e1101 (Manager of Section 1A) → manager: e3101 (Head of Sales Department 1)
    e9002 (Lawyer 2), manager: manager: e9001 (General counsel) → manager: e0102 (Head of Corporate Division)
    e9002 (Lawyer 2), head: dept_head: e9001 (General counsel) → dept_head: the head of legal (Legal) is empty; it goes up to e0102 (Head of Corporate Division)
    e9003 (Lawyer 3), manager: manager: e9001 (General counsel) → manager: e0102 (Head of Corporate Division)
    e9003 (Lawyer 3), head: dept_head: e9001 (General counsel) → dept_head: the head of legal (Legal) is empty; it goes up to e0102 (Head of Corporate Division)
Requesters:
  - Only in the version from 2026-04-01: e9001 (General counsel), e1203 (Sales staff 3) [made by e1201 (Sales staff 1)]
Stages nobody answers:
  How many requesters each stage finds nobody for, by what happens then as the request says, where the two versions differ:
    head, goes up or elsewhere: 9 → 10
Tickets in progress:
  A stage whose approvers are decided keeps them on and after 2027-04-01; each stage that starts on or after 2027-04-01 takes its approvers from the version from 2027-04-01. Where an approver a stage waits for is no longer in the organization, an administrator hands the ticket over; a late stage goes up as the version in effect then says.
```

From 2027-04-01, the first section's staff answer to the head of the department, and the legal team
has no head of its own, so its lawyers' stages go up to the head of the division. What happens to a
ticket already in progress is said last.

## What changes when the file does

`ritsu ringi diff <old> <new>` compares two versions of a request type: which requests come to other
stages, and the approvers of whom change, in each version of the organization. A leave that asks
the head of the department from four business days on, not six:

```diff
-request leave v1 "Leave request"
+request leave v2 "Leave request"
…
-route standard(long: days > 5)
+route standard(long: days > 3)
```

Saved beside the first as `leave.v2.ringi`, `ritsu ringi diff leave.ringi leave.v2.ringi` says:

```text
  The request `leave` (v1 → v2): `crates/ringi/examples/leave.ringi` → `crates/ringi/examples/leave.v2.ringi`
Route:
  Requests where “Business days” is from 4 to 5: the stage “head” (dept_head) comes.
  In the organization from 2026-04-01, the approvers change for 20 of 20 requesters.
    e0101 (Head of Sales Division), head: does not come → dept_head: no team of the kind `department` is above the requester; it goes up to e0001 (President (fictional))
…
  And 8 more: `--format json` lists them all.
  Every other request comes to the same stages.
Tickets in progress:
  A ticket finishes with the version of the request it was submitted with: tickets submitted with version 1 finish as version 1 says, and what changes here applies to requests submitted with version 2.
```

The page `ritsu ringi diff --format html` draws says the same for people, and `ritsu mod diff`
says it of each request type when a module of them gets a new version.

## The longest a ticket can take

`ritsu ringi doc --longest` draws the ticket that takes the longest: every stage late to its last
day, every send-back, every resubmission at its last day, and how many business days that makes.
The check holds it to the claim `every ticket ends within`.
