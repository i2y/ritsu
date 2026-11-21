# Writing a request type

*For an agent that writes `.ringi`, and for the engineer who asks it to and reads what it wrote.*

A kind of request is one `.ringi` file: the form, the route and who answers each stage, the
deadlines, the rules of its ticket, and the claims and scenarios that hold it to what was meant.
What several kinds share goes in a file of its own, which they read with `use`. The first word of
a file says its kind:

| First line | What the file says | Example |
|---|---|---|
| `request <name> v<version> "<title>"` | one kind of request | `expense.ringi` |
| `route <name> v<version> "<title>"` | a route several requests take | `shared/manager_approval.ringi` |
| `organization <name> v<version> "<title>"` | the shape of the organization: positions, kinds of teams, where the data is | `shared/company.ringi` |
| `masters <name> v<version>` | the lists a form chooses from or looks an approver up in | `shared/masters.ringi` |

The examples are five kinds of request from five departments, each with a Japanese version beside
it: an expense (accounting), leave (HR), a purchase (general affairs), a contract review (legal)
and access to a system (IT), in `crates/ringi/examples`. Every line below is a line of them.

## The head of a request

A request starts with its name, its version and its title, then says what it is, which department
looks after it, and what it reads:

<!-- crates/ringi/examples/expense.ringi -->
```ringi
request expense v1 "Expense request"
…
owner accounting

use organization org from "shared/company.ringi"
use masters m from "shared/masters.ringi"
use calendar office from "shared/office.cal"
use book budget from "shared/budget.book"
use gate "shared/company.gate"
use flow precheck_agent from "shared/precheck.flow"
use flow travel_decision from "shared/travel_decision.flow"
use flow pay from "shared/pay.flow"
```

The calendar counts business days, the book holds the budget, the gate holds the rules of the whole
company (nobody approves their own request), and the flows are what an agent or the work after the
approval does. Each is a file of another of ritsu's languages: [With the other languages](together.md).

## The form

A field has a name, a type, a title, and the bounds of what a person can put in: a range for a
number and a date, a count for a text and for rows. A field with no bound is an error (E101,
E102), since the same bounds check the screen, the generated code and the rows of the route.

<!-- crates/ringi/examples/expense.ringi -->
```ringi
form
  category : choice "Category"
    travel   "Travel"
    expense  "Expenses"
    purchase "Purchase"
  items : rows "Items"  count >=1 <=20
    what  : text "What"  count <=100
    price : money[JPY] "Price"  range >=1JPY <=1_000_000JPY
  amount  : money[JPY] "Amount" = sum(items.price)  range >=1JPY <=10_000_000JPY
  purpose : long text "Purpose"  count >=1 <=200
  used_on : date "Date of use"  range >=2026-01-01 <=2027-12-31
  trip_to : text "Destination"  count <=100  shown when category is travel
  receipt : attachment "Receipt"  required when amount > 30_000JPY
  account : choice from m.accounts "Account code"  approvers only
```

| Type | Written | The screen shows |
|---|---|---|
| text | `text "Title"  count <=100` | a line to type in |
| long text | `long text "Title"  count >=1 <=200` | lines to type in |
| number | `number "Title"  range >=1 <=20` | a whole number |
| a number with its unit | `money[JPY]`, `rate[step 5%]`, `mass[kg]` and the other units of ritsu | a whole number with its unit |
| date | `date "Title"  range >=2026-01-01 <=2027-12-31` | a date |
| choice | `choice "Title"`, its values under it | one of the values |
| a choice from a list | `choice from m.vendors "Title"` | the rows of the list that are valid that day |
| a choice of a rule's values | `choice from authority.kind "Title"` | one of the values the rule declares |
| attachment | `attachment "Title"` | a file to attach |
| person, team | `person "Title"`, `team "Title"` | someone, or a team, of the organization |
| yes or no | `bool "Title"` | a box to tick |
| rows | `rows "Title"  count >=1 <=20`, its fields under it | rows to add and remove |

A field is required unless it says `optional`. `required when` makes it required only when the
condition holds, and `shown when` shows it only then; a hidden field is neither required nor sent.
A field written with `=` is computed (`amount`, the sum of the items' prices), and `approvers only`
is a field the approvers fill in. `open in office` takes only a day the calendar has open.

## The route

A route is the stages a request goes through, in order. Each stage has a name and who answers it,
and may hold only when a condition does. A purchase (`purchase.ringi`) shows each kind of stage:

<!-- crates/ringi/examples/purchase.ringi -->
```ringi
route
…
  first     manager
    when empty escalate, then skip
  technical reviewer               when reviewer is not none
…
  finance   finance                when authority.finance or advance > 30%
  board     at least 60% of board  when authority.board
    due in 10 business days at end of day
    when late expire
…
  decide    authority.decider
    when empty escalate, then board
  inform    circulate to general_affairs
```

| Stage | Written | It passes when |
|---|---|---|
| an approval | `first  manager` | one of its approvers approves |
| a quorum | `board  at least 60% of board`, `at least 2 of legal`, `all of legal`, `dept_head and accounting` in a table's cell | as many approve as the quorum asks; a reject that leaves the quorum out of reach rejects the request |
| a circulation | `inform  circulate to general_affairs` | at once: the people are told and may read it |
| an agent's answer | a way to choose an approver written `agent <flow>, else <approver>` | the flow answers `ok`; on a doubt or a failure, the stage goes to the approver after `else` |
| a summary | `summary  note from summarize` | the flow's summary is ready, and the approvers after it see it |

A stage may also be decided by a rule: `authority.decider` is the output of the company's table of
authority (a rule of rulec), whose values are ways to choose an approver.

When a route depends on two or three fields together, it is a table. Each row is a set of the
fields' values and the approvers of each stage for them; an empty cell is a stage the request skips.
ringi lowers the table to a rule of rulec, which checks that every value the form can take comes to
exactly one row:

<!-- crates/ringi/examples/expense.ringi -->
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
```

## Who answers

`who` names each way to choose an approver. Each is a relation in the organization, decided when
the stage starts, in the version of the organization in effect that day:

<!-- crates/ringi/examples/expense.ringi -->
```ringi
who
  manager       = manager of requester
  dept_head     = head of requester.department
  accounting    = team accounting
  division_head = nearest division_head above requester
  check         = agent precheck_agent, else manager
  travel_agent  = agent travel_decision, else manager
  board         = team board
```

| Way | Written | Who it is |
|---|---|---|
| a person | `counsel = person "e9001"` | that person |
| the requester's manager | `manager = manager of requester` | the head of the requester's main team, or of the team above when that is the requester or nobody |
| the head of the requester's team of a kind | `dept_head = head of requester.department` | the head of the first team of that kind above the requester |
| the nearest of a position | `division_head = nearest division_head above requester` | the first head above the requester whose position is that one or higher |
| a team | `accounting = team accounting` | anyone of the team |
| chosen by the requester | `reviewer = chosen by requester in reviewer` | the person the requester put in that field |
| chosen by the approver before | `reviewer = chosen by previous approver in reviewer` | the person the stage before put in a field of the approvers |
| looked up in a list | `system_owner = lookup m.systems.owner by system` | the person of the row the form's value finds |
| an agent | `check = agent precheck_agent, else manager` | a flow of dandori, and the approver after `else` when it doubts |

## Deadlines and empty stages

`every stage` says the deadline of each stage and its reminder, in business days of the calendar,
what happens when a stage is late, and what happens when it has nobody to answer it. A stage can
say its own under it, as the board does above:

<!-- crates/ringi/examples/expense.ringi -->
```ringi
every stage
  due in 3 business days at end of day
  remind after 2 business days at 09:00
  when late escalate by next_up, then expire
…
  when empty escalate, then board
```

`escalate` goes up the organization, or along a table (`by next_up`); `expire` ends the ticket;
`skip` passes the stage; `stay` waits. ringi checks every reminder comes before its deadline, and
says how long a ticket can take at the most.

## The ticket

`ticket` says what people may do with a request, and what the request does when it is submitted
and when it ends:

<!-- crates/ringi/examples/purchase.ringi -->
```ringi
ticket
  self approval     not allowed
…
  same approver     approves once, at the last stage
  send back         to any earlier stage, at most 3 times
  resubmit          within 10 business days
  withdraw          until decide
  retract approval until the next stage answers
  approvers edit    summary
…
  on submitted      hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: requester.department, amount: amount)  when requester.department is not none
  on approved       post budget.reserve(request: ticket.id, attempt: ticket.attempt)
  on approved       launch order(request_id: ticket.id, vendor: vendor, amount: amount, summary: summary)
  on sent back, rejected, withdrawn, expired  void budget.reserve(request: ticket.id, attempt: ticket.attempt)
```

The budget is held when the purchase is submitted, posted when it is approved, and let go when it
is sent back, rejected, withdrawn or expires; the order is placed by a flow once it is approved.
`submitted by` says who may submit it (`everyone except team board` in the leave request), and
`comment required for reject, send back` asks a reason of those who do either.

## Claims, examples and scenarios

A claim says what must hold of the route for every value of the form, and the check proves it or
shows a case where it does not. An example is a row of the route's table that must come out as
written. A scenario is the events of one ticket and what it comes to after each:

<!-- crates/ringi/examples/expense.ringi -->
```ringi
claims
  the_agent_decides_up_to_30000_yen : decide is travel_agent only when amount <= 30_000JPY
  a_joint_second_stage              : possible second is dept_head and accounting
  reminder_before_due               : every stage reminds before it is due
  ends_in_time                      : every ticket ends within 93 business days
```

<!-- crates/ringi/examples/leave.ringi -->
```ringi
scenario a_leave_rejected_with_a_reason
  requester e1203
  given leave.grant(person: "e1203", year: "2026", days: 20)
  form kind = paid, first_day = 2026-12-21, days = 8, reason = "A family visit"
| at               | event   | by    | comment                                         | -> stage | status      |
| 2026-11-16 09:00 | submit  | e1203 |                                                 | manager  | in progress |
| 2026-11-16 17:00 | approve | e1101 |                                                 | head     | in progress |
| 2026-11-16 18:00 | reject  | e3101 | "The week before the year end is our busiest"   |          | rejected    |
```

## The organization and the lists

The organization's file declares its positions from the lowest and its kinds of teams from the
widest, and where its data is. The data is a directory of CSV files for each version, named by the
day it comes into effect (`org/2026-04-01`, `org/2027-04-01`): the teams, the people, who belongs
where, and the proxies. A request decides its approvers in the version in effect when each stage
starts.

<!-- crates/ringi/examples/shared/company.ringi -->
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

A masters file declares the lists a form chooses from, each with its key, its fields and its rows'
CSV; each row says from when and until when it may be chosen:

<!-- crates/ringi/examples/shared/masters.ringi -->
```ringi
master systems "Systems"
  key id
  fields
    id    : text "System"  count <=30
    name  : text "Name"  count <=100
    owner : person "Owner"
  data "../masters/systems.csv"
```

## Names in another language

A name may be written in any script, with the ASCII name the code and the JSON use beside it in
brackets, and the titles may be given in another language under `labels`. The Japanese version of
the leave request (`leave.ja.ringi`) is the same request:

<!-- crates/ringi/examples/leave.ja.ringi -->
```ringi
request 休暇(leave_ja) v1 "休暇の申請"
…
form
  種類(kind) : choice "休暇の種類"
    有給(paid)    "有給休暇"
    病気(sick)    "病気休暇"
    特別(special) "特別休暇"
  初日(first_day)  : date "初日"  range >=2026-01-01 <=2027-12-31  open in 本社
  日数(days)       : number "営業日の日数"  range >=1 <=20
```

The keywords stay in English: an agent writes them, and a person reads the page `ritsu ringi doc`
draws, in English or in Japanese ([Reading a request type](reading.md)).
