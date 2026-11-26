# What the check guarantees

*For whoever relies on a request type: the agent and the engineer who write it, and the
administrator who signs it off before it is used.*

`ritsu ringi check` holds a request type to three things: what ringi knows of its own (the form,
the route, the ticket, the organization), what the languages it is lowered to know (a table and a
condition are rulec's, a deadline is koyomi's), and the organization's data, version by version
and person by person. `ritsu check` runs it on every `.ringi` of a project, with the other
languages' files. This page shows what the check says of the examples, and what it says when one of
them is edited.

## What ringi checks of its own

- **The words and the names.** A word in the wrong place, a name declared twice or never, a `use`
  that does not read, a block the kind of file does not take.
- **The form.** Every number, amount and date has a range, every text a limit on its length (E101,
  E102). A computed field's range meets what the computation can come to (E103), and an amount says
  how it rounds (E104). A required field is never hidden (E105), and a condition reads only fields
  filled in before it (E106).
- **The route.** A quorum is no larger than the approvers it counts (E304), a choice comes from a
  stage that can make it (E305), a shared route is given what it takes (E307), and a stage named
  anywhere is one the route has (E308).
- **The ticket.** Where the claims ask it, every reminder comes before its deadline (E403), and
  every ticket ends within the days they give (E401). A hold in a book is posted or let go on
  every ending (E405), and keyed by the try, so a request made again holds again (E408).
- **The words of the pages.** Each language `labels` declares has a label for every field, value,
  stage and way the pages show, and what comes from another file has a title in the file's
  language (E703).

## What the languages it is lowered to check

ringi does not give a table, a condition, a computation or a deadline a meaning of its own. It
writes each as a file of the language that has one, in memory, and has that language check it:
the route's table and the form's computations become rules of rulec, the deadlines a dates file of
koyomi. What they find is said again in ringi's words, at ringi's line:

- every value the form can take comes to exactly one row of the route (E301, E302), and no row is
  out of reach (E303);
- each claim holds for every value of the form, or a value is shown where it does not (E306);
- each example comes out as written (E601).

## What the organization's data shows

With the organization's data, the check finds who answers each stage, for each person who may
submit the request, in each version of the organization:

- the data itself, read as strictly as the server reads it: a column missing, a key
  `organization.json` does not know, a value over its limit or a row of the wrong shape is said
  at the row of its file (E501);
- a stage with nobody to answer it, where the route does not say what to do then (E502), and one
  that goes up or is skipped as it says (W502);
- a quorum some requesters cannot reach (E504), one person at two stages of a ticket (W506), and a
  late stage with nowhere to go up to (W507);
- a person a version does not have, named where a stage was to go up from them (E503);
- the team in charge of the request (`owner`), where a version does not have it (E511);
- the days the request can be used on: from the first version of the organization, until the end
  of what the calendar, the organization and the lists cover.

The contract review (`contract.ringi`) passes in the organization as it is now, and not in the one
that comes into effect on 2027-04-01. From that day, the general counsel is no longer in the data,
and the legal team keeps one lawyer besides the requester, while a contract over 5,000,000 yen asks
two of them:

```console
$ cd crates/ringi/examples
$ ritsu ringi check contract.ringi
error[E504]: contract.ringi:31:12: In the organization from 2027-04-01, the stage `second` cannot get 2 approvals for some requesters
    31 |   second   at least 2 of legal  when value > 5_000_000JPY
  = `legal` has only 1 member besides the requester.
  = For 2 requesters: e9002 (Lawyer 2) and e9003 (Lawyer 3).
  = Add people to the team, or change the quorum.
warning[W506]: contract.ringi:33:12: In the organization from 2026-04-01, one person answers two stages of a ticket
    33 |   counsel  counsel              when value > 10_000_000JPY
  = The stage `manager` is `manager` and the stage `counsel` is `counsel`, and both come to one person.
  = For 2 requesters, e9001 (General counsel) answers both: e9002 (Lawyer 2) and e9003 (Lawyer 3).
  = If one approval is enough, write `same approver approves once, at the last stage` under `ticket`: `manager` then waits for the approval of `counsel`, after the stages between them.
  = One person who checks twice is not two pairs of eyes: change the way to choose one of the stages, or make sure it is meant.
error[E503]: contract.ringi:41:3: In the organization from 2027-04-01, `counsel` names someone who is not there
    41 |   counsel  = person "e9001"
  = `counsel` names e9001, who is not in the organization from 2027-04-01.
  = The stage `counsel` is `when empty escalate`, and it cannot go up from someone the organization does not have (what `then` says holds only where nobody is above).
  = Correct the ID, or add the person (the team) to the data of the version.
```

The file has not changed; the organization has. The check reads every version of the data, so the
error is said now, before the day it would stop a ticket. Read only in the version in effect on a
day (`--at`), the contract passes, and the lines under `ok` say what the data comes to:

```console
$ cd crates/ringi/examples
$ ritsu ringi check contract.ringi --at 2026-11-16
warning[W506]: contract.ringi:33:12: In the organization from 2026-04-01, one person answers two stages of a ticket
    33 |   counsel  counsel              when value > 10_000_000JPY
  = The stage `manager` is `manager` and the stage `counsel` is `counsel`, and both come to one person.
  = For 2 requesters, e9001 (General counsel) answers both: e9002 (Lawyer 2) and e9003 (Lawyer 3).
  = If one approval is enough, write `same approver approves once, at the last stage` under `ticket`: `manager` then waits for the approval of `counsel`, after the stages between them.
  = One person who checks twice is not two pairs of eyes: change the way to choose one of the stages, or make sure it is meant.
ok contract.ringi: a request, 8 fields, 7 stages
  used from 2026-04-01: the first version of the organization `shared/company.ringi` (data `../org`)
  used with no end: no data it reads ends
  ends within 192 business days at the most (4 × 38 at its stages, 3 × 10 with the requester, 10 paused)
  requesters whose stages (`manager`, `counsel`) go elsewhere or are skipped by the shape of the organization: 2 of 24 in the organization from 2026-04-01
  requesters whose late stages (`manager`) have nowhere to go up to by the shape of the organization: 6 of 24 in the organization from 2026-04-01
  who they are, and where their stages go: `ritsu ringi simulate contract.ringi --every-requester`
```

## The scenarios

Each scenario is run on the reference interpreter, row by row, at the time each row is written at.
A row whose stage or status is not what the ticket comes to fails the check (E602), and so does an
event the ticket does not take at that point (E603).

## An edit and what the check says

The table of the expense request, with the row of the travel between 30,000 and 100,000 yen taken
out:

```diff
 | travel   | <=30_000JPY                |             |         |                          | travel_agent  |
-| travel   | >30_000JPY <=100_000JPY    |             |         |                          | manager       |
 | travel   | >100_000JPY                |             | manager |                          | dept_head     |
```

`ritsu ringi check expense.ringi`:

```text
error[E301]: expense.ringi:29:1: No row of the route takes some requests
    29 | route
  = The requests no row takes: category = travel, amount from 30_001JPY to 100_000JPY
  = A request no row takes: category = travel, amount = 30_001JPY
  = Add this row to the table, writing in place of each `?` who answers that stage (a name in `who`; a blank cell where those requests skip it): | travel | >30_000JPY <=100_000JPY | ? | ? | ? | ? |
  = The cells are cut only at the bounds the table already writes and at the ends of the ranges, so the rows close the gap as they are; where the approvers change inside a row, split it there.
  = 4 more diagnostics follow from this mistake (the examples and the claims that cannot hold because of it); correct it, then check again.
…
```

rulec finds the travel no row takes, and ringi says it as a row to add, in the table's own words.

The claim that the agent decides a trip only up to 30,000 yen, written for 20,000:

```diff
-  the_agent_decides_up_to_30000_yen : decide is travel_agent only when amount <= 30_000JPY
+  the_agent_decides_up_to_30000_yen : decide is travel_agent only when amount <= 20_000JPY
```

`ritsu ringi check expense.ringi`:

```text
…
error[E306]: expense.ringi:82:3: The claim the_agent_decides_up_to_30000_yen does not hold: some request outside the condition comes to `travel_agent` at the stage decide
    82 |   the_agent_decides_up_to_30000_yen : decide is travel_agent only when amount <= 20_000JPY
  = Such a request: category = travel, amount = 20_001JPY
  = Correct the route, or the condition.
```

A reminder after the deadline it reminds of:

```diff
   due in 3 business days at end of day
-  remind after 2 business days at 09:00
+  remind after 3 business days at 09:00
   when late escalate by next_up, then expire
```

`ritsu ringi check expense.ringi`:

```text
…
error[E403]: expense.ringi:84:1: A reminder does not come before its deadline
    84 |   reminder_before_due               : every stage reminds before it is due
  = As koyomi says it: The claim reminder_before_due fails for 146,097 of the 146,097 days of started
  = It fails on 2026-04-01..2426-03-31
  = The farthest is started 2026-04-01, where remind 2026-04-06 is the same day as due
  = Make the reminder come after fewer business days than the deadline.
```

Every diagnostic has a page in [the list of codes](https://github.com/i2y/ritsu/blob/main/crates/ringi/docs/codes.md),
with the smallest file that gives it; `ritsu ringi explain <code>` prints the same.

## What the check does not say

- **Whether the data is the company as it is.** The check reads the organization's data as it is
  given. Who belongs where, and who is whose manager, is what the data says.
- **Whether the rules are the company's rules.** A route that passes is complete and has no two
  rows for one request; whether it is the route the regulations ask for is for a person to read, on
  [the page for people](reading.md).
- **What the server does.** The check holds the request type and the code made of it. The server
  that runs the tickets is held to the same answers by the vectors:
  [What is generated, and what the server does](server.md).
