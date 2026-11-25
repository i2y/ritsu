# The ringi reference

ringi is a small language for a company's applications and approvals. A `.ringi` file is one kind
of request: the form the requester fills in, the route it takes and who answers each stage of it,
the deadlines and the reminders, what is done in the books and the flows as it ends, and the rules
of its ticket (self approval, sending back, withdrawing, pausing, proxies). The same words serve
the applications of any department: the examples are an expense, a purchase, a leave, an account on
a system and a contract to review, each in English and with Japanese names. The organization (its
positions, its teams and the people in them, version by version) and the masters (the lists a form
chooses from) are files of their own, which every request reads.

What a request writes inside it, the table of its route, its conditions, its computations and its
deadlines, is lowered to `.rule`, `.cal` and `.kn` files in memory, and rulec, koyomi and kaname
check them; what they say comes back as ringi's diagnostics, at the lines of the `.ringi`. With the
organization's data, `check` finds who answers each stage for every requester in every version,
and says the stages nobody answers, the quorums out of reach and one person who answers two stages.
A request reads the calendars of koyomi, the rules of rulec, the books of chobo, the flows of
dandori and the gate of sekisho, and `ritsu check` holds it to them across the languages. `ritsu
gen` writes the ticket machine as TypeScript, Python, Go and Rust, and, with `--target requests`,
what a server and its screens read.

This page is the whole language and the commands. [codes.md](codes.md) has every diagnostic, with
the smallest file that prints it. DESIGN.md has the reasons.

## Files

A `.ringi` is in UTF-8. `#` starts a comment that runs to the end of the line, outside a string.
A block starts at the start of a line, and its lines are indented with spaces under it (a tab, or a
full-width space, is E001). The first word of the file says its kind:

| First line | What the file holds | Example |
|---|---|---|
| `request <name> v<N> "<title>"` | one kind of request | `examples/expense.ringi` |
| `route <name> v<N> "<title>"` | a route several requests share | `examples/shared/manager_approval.ringi` |
| `organization <name> v<N> "<title>"` | the shape of the organization: positions, kinds of team, where the data is | `examples/shared/company.ringi` |
| `masters <name> v<N>` | lists to choose from, and where their rows are | `examples/shared/masters.ringi` |

The blocks of a request come in this order (E001 otherwise); `form`, `route` and `ticket` cannot be
left out, and a block written twice, or one the kind of file does not take, is E006:

| Block | What it is |
|---|---|
| `request <name> v<N> "<title>"` | the heading |
| `description "<text>"`, `owner <team>`, `effective from <date>` | what it is for, the team in charge of it, the first day it is used |
| `use …` | the files it reads |
| `form` | the fields the requester fills in |
| `route` | the stages, as a table or a line a stage |
| `who` | the ways to choose an approver |
| `every stage` | the deadlines and reminders of every stage |
| `table <name>` | a table the deadlines read (any number) |
| `ticket` | the rules of the ticket, and what is done as it ends |
| `claims` | what the check proves of the route and the deadlines |
| `examples` | rows of requests and the approvers they come to |
| `scenario <name>` | a ticket from submission to its end (any number) |
| `labels <lang>` | the texts in another language (any number) |

The head of the expense example (the whole file is `examples/expense.ringi`):

```ringi
request expense v1 "Expense request"
description "An expense, a trip or a purchase, checked, approved and decided by the roles the Authority Rules (fictional), Article 12, Appendix 2 give its category and amount"
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

`description` says what the request is for, in the words of the people who make it; the pages and
the server show it. `owner` is the team in charge of the kind of request, whose
administrators may add an approver to a stage and hand a stage over (the gate decides who they
are). `effective from <date>` is the first day the request is used on; without it, the first day is
the first day all the data it reads is there.

### Names and aliases

A name in English is written as it is. A name in another language is written with an ASCII name in
brackets after it, `種別(category)`, and the ASCII name is the one the generated code, the JSON and
the vectors use; the pages, the diagnostics and `run` say the name as it is written. A request in
English and the same request with Japanese names (`expense.ja.ringi` beside `expense.ringi`) make
the same vectors but for the name of the kind of request, the texts people read and the texts
filled in the form (`tests/machine/desk.ringi` and `desk.ja.ringi`).

Names are kept in four places: the fields of the form, the stages, the ways to choose an approver
(`who`), and the tables. One name may be used in two places (`account.ringi` has a stage `manager`
and a way `manager`); twice in one place is E003. An expression reads the fields, the aliases of the
`use` lines, `requester` and `ticket`; a cell of a stage reads the ways of `who`.

A name cannot be one of the seven words conditions are made of, `and` `or` `not` `is` `none`
`true` `false` (E002). A field cannot be named `requester` or `ticket`, which an expression reads
(`requester.id`, `requester.department`, `ticket.id`, `ticket.round`, `ticket.attempt`). A way to
choose an approver cannot be named `skip` or `stay`, the words after `when empty escalate, then`.
The ASCII name of a field cannot be a word kaname reserves, nor a reserved word of TypeScript,
Python or Go, since it is a key of the ticket's JSON the generated code reads; [codes.md](codes.md)
lists the rest of E002.

The Japanese version of the expense example begins:

```ringi
request 経費(expense_ja) v1 "経費の申請"
description "経費・出張・購買の申請。種別と金額に応じて、決裁権限規程（架空）第 12 条の別表 2 が定める役が、確かめ、承認し、決裁する"
owner accounting

use organization 組織(org) from "shared/company.ringi"
use masters マスタ(m) from "shared/masters.ringi"
use calendar 本社(office) from "shared/office.cal"
use book 予算(budget) from "shared/budget.book"
use gate "shared/company.gate"
use flow 事前の確かめ(precheck_agent) from "shared/precheck.flow"
use flow 出張の決裁(travel_decision) from "shared/travel_decision.flow"
use flow 支払い(pay) from "shared/pay.flow"

form
  種別(category) : choice "種別"
    出張(travel)   "出張"
    経費(expense)  "経費"
    購買(purchase) "購買"
  明細(items) : rows "明細"  count >=1 <=20
    品目(what)  : text "品目"  count <=100
    金額(price) : money[円] "金額"  range >=1円 <=1_000_000円
  合計(amount)      : money[円] "合計" = sum(明細.金額)  range >=1円 <=10_000_000円
  目的(purpose)     : long text "目的"  count >=1 <=200
  使った日(used_on) : date "使った日"  range >=2026-01-01 <=2027-12-31
  行き先(trip_to)   : text "行き先"  count <=100  shown when 種別 is 出張
  領収書(receipt)   : attachment "領収書"  required when 合計 > 30_000円
  勘定科目(account) : choice from マスタ.accounts "勘定科目"  approvers only

route
| 種別 | 合計                     | -> 確かめ(precheck) | 一次(first) | 二次(second)    | 決裁(decide)   |
| 出張 | <=30_000円               |                    |             |                 | エージェント決裁 |
| 出張 | >30_000円 <=100_000円    |                    |             |                 | 上長           |
| 出張 | >100_000円               |                    | 上長        |                 | 部長           |
| 経費 | <=100_000円              | エージェント確認    |             |                 | 上長           |
| 経費 | >100_000円 <=1_000_000円 | エージェント確認    | 上長        |                 | 部長           |
| 経費 | >1_000_000円             |                    | 上長        | 部長            | 本部長         |
| 購買 | <=1_000_000円            |                    | 上長        | 経理            | 部長           |
| 購買 | >1_000_000円             |                    | 上長        | 部長 and 経理   | 本部長         |
```

## Reading other files

A `use` line names the kind of file, an alias, `from` and the path. The path is from the
directory of the file (`../` is the one above), or `@<module>/…` for a file of a module the project
depends on. A path from the root of the disk is E005 (copied elsewhere, the tree would read another
file), and so is a path that leads out of the project, or a file of another kind.

| `use` | Reads | For |
|---|---|---|
| `use organization <alias> from "<file>.ringi"` | an organization | the ways to choose an approver, `submitted by`, `requester.<kind of team>` |
| `use masters <alias> from "<file>.ringi"` | masters | `choice from <alias>.<master>`, `lookup <alias>.<master>.<field> by <field>` |
| `use calendar <alias> from "<file>.cal"` | a calendar of koyomi | business days: the deadlines, `open in`, `business days in` |
| `use dates <alias> from "<file>.cal"` | dates of koyomi that are not a calendar | dates the form computes |
| `use rule <alias> from "<file>.rule"` | a rule of rulec | `choice from <alias>.<enum>`, a stage chosen by an output (`authority.decider`), a condition on an output (`when authority.board`) |
| `use book <alias> from "<file>.book"` | a book of chobo | `hold`, `post` and `void` in the `on` lines, `given` in a scenario |
| `use flow <alias> from "<file>.flow"` | a flow of dandori | `agent`, `note from`, `launch` |
| `use route <alias> from "<file>.ringi"` | a shared route | `route <alias>(…)` |
| `use gate "<file>.gate"` | the gate of sekisho, which has no alias | who may do each operation on the ticket |

A request with two `use calendar` lines says which one the deadlines count in
(`due in 3 business days in office`); with one, it is the one.

## The form

`form` lists the fields, one a line: the name, `:`, the type, the title, and the marks after it.
A field with no mark is required.

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

### Types

| Type | Written | What it takes |
|---|---|---|
| text | `text "Title"  count <=100` | a line of text |
| long text | `long text "Title"  count >=1 <=200` | lines of text |
| number | `number "Title"  range >=1 <=20` | a whole number |
| a number with a unit | `money[JPY]`, `rate[step 5%]`, `mass[kg]`, and `length`, `area`, `volume`, `duration`, `temperature`, `sound` | a whole number of the unit's step, shown with its unit |
| date | `date "Title"  range >=2026-01-01 <=2027-12-31` | a day |
| choice | `choice "Title"`, its values on the lines under it | one of the values |
| a choice from a master | `choice from m.vendors "Title"` | the key of a row in effect on the day |
| a choice from a rule's enum | `choice from authority.kind "Title"`, the title of each value under it | one value of the enum |
| attachment | `attachment "Title"` | a file (the server keeps it) |
| person | `person "Title"` | a person of the organization |
| team | `team "Title"` | a team of the organization |
| bool | `bool "Title"` | true or false |
| rows | `rows "Title"  count >=1 <=20`, its fields indented under it | rows of the fields under it |

The types are ritsu's: an amount has a currency, a rate a step, and a value of one unit is not
compared with one of another (E009). There is no number with a fraction and no unit, since rulec
has none: such a value is written with a unit or as a rate. The value of a person or a team is
written `person "e9003"` and `team legal` in a scenario, an event and `simulate --form`.

### Marks

| Mark | What it does |
|---|---|
| `range >=a <=b` | the values taken; a number, an amount or a date someone fills in needs both ends (E101) |
| `count >=a <=b` | the length of a text, and the number of rows; a text needs an upper end (E102), and rows one too (E101) |
| `optional` | the field may be left empty |
| `required when <condition>` | required where the condition holds, and optional elsewhere |
| `shown when <condition>` | shown, and sent, only where the condition holds; a hidden field is never required |
| `approvers only` | filled in by an approver on the approval card, not by the requester |
| `open in <calendar>` | a date that has to be a business day of the calendar |
| `round <direction>(<step>)` | how a computed amount rounds: `up`, `down`, `half_up`, `half_down`, `half_even` |

A field that can be required while it is hidden (`required when` holds where `shown when` does not)
is E105: rulec is asked whether such a request exists, and the diagnostic gives one. When rulec
cannot settle it, W201 says so. Something decided when the request is made (`shown when`,
`required when`, the route, the `on submitted` lines) that reads a field approvers fill in is E106.
A text field whose name or title asks for a secret (`password`, `token`, …) is W109.

### Rows and sums

`rows` repeats the fields under it. Their number is held to `count` by the ticket machine, the
screens and the generated code. `sum(<rows>.<field>)` adds a column, and a computed field inside
the rows is worked out row by row:

```ringi
form
  kind   : choice from authority.kind "Kind"
    goods    "Goods and services"
    capex    "Capital expenditure"
    software "Software"
  vendor : choice from m.vendors "Vendor"
  lines  : rows "Lines"  count >=1 <=30
    item     : text "Item"  count <=100
    quantity : number "Quantity"  range >=1 <=10_000
    price    : money[JPY] "Unit price"  range >=1JPY <=50_000_000JPY
    subtotal : money[JPY] "Subtotal" = quantity * price  range >=1JPY <=100_000_000JPY
  amount   : money[JPY] "Amount" = sum(lines.subtotal)  range >=1JPY <=100_000_000JPY
  summary  : long text "What it is for"  count >=1 <=1000
  quote    : attachment "Quotation"
  advance  : rate[step 5%] "Paid in advance"  range >=0% <=100%
  others   : rows "Other quotations"  count >=0 <=3
    company : text "Company"  count >=1 <=100
    bid     : money[JPY] "Amount quoted"  range >=1JPY <=100_000_000JPY
  reviewer : person "Technical reviewer"  optional
```

### Computed fields

`= <expression>` makes a field the value of the expression: the requester does not fill it in. The
expression is written as rulec writes one, with `+ - * /`, `sum(…)`, the other fields and the
units; a date is computed with `<date> + <n> business days in <calendar>` (the leave example's
`back_on`). An amount divided, or multiplied by a rate, says how it rounds (`round down(1JPY)`);
without it, E104.

A computed field needs no range. One written is the ends of what it takes: a request whose value
falls outside it is refused, by the machine, the screens and the generated code alike, with the
field and the range named and the fields to change listed. What reads the field (the route, the
conditions, the claims) looks only inside the range. A range wider than what the computation comes
to is no mistake (the expense's `amount` comes to 20,000,000 yen at the most, and is held to
10,000,000); one that meets none of it is E103:

```ringi
form
  amount : money[JPY] "Amount"  range >=1JPY <=33JPY
  # 3% of 33 yen at the most is 0.99 yen, and rounded down it is 0 yen
  fee    : money[JPY] "Fee" = amount * 3%  round down(1JPY)  range >0JPY
```

```console
$ ritsu ringi check tests/mutants/E103_a_fee_rounded_down_to_nothing.ringi
error[E103]: tests/mutants/E103_a_fee_rounded_down_to_nothing.ringi:6:62: The range `>0JPY` of the field fee holds no value its computation comes to
     6 |   fee    : money[JPY] "Fee" = amount * 3%  round down(1JPY)  range >0JPY
  = The computation comes to `>=0JPY <=0JPY` (from the ranges of what it reads). With this range no request can be made.
  = Correct the range so that it meets what the computation comes to. A request whose value falls outside the range is refused; a range wider or narrower than that is no mistake.
```

### Conditions

`shown when`, `required when`, the `when` of a stage, the `when` of an `on` line and `only when`
compare a field with a value with `is`, `is not`, `<`, `<=`, `>`, `>=`, and join comparisons with
`and`, `or` and `not`. A bool output of a rule the request uses is a condition too (`when
authority.board`). What a condition can read is what a column of the route's table can: a choice,
a number, an amount, a date, a bool, a computed number, amount or date, an output of a rule, an
input of a shared route. Whether any field is filled in is read with `is none` and `is not none`.

A field with no value (an optional one left empty, a hidden one, one approvers have not filled in
yet) is the value `none`: `is none` holds, `is not <value>` holds, and `is <value>` and the
comparisons of size do not. A computed field that reads an empty field has no value either; a `sum`
always has one, and leaves out the rows whose cell is empty.

A condition that reads something else is E309, at its place: the requester (`requester.department`),
the ticket (`ticket.round`), a text, a person or a choice from a master, two fields compared with
each other (`amount > limit`), or a computation inside the condition. The note says how to write it:
a computed field compared with a value (`difference : money[JPY] = amount - limit`, then `when
difference > 0JPY`), a `choice` in place of a text, `submitted by` to say who may make the request,
`when empty escalate, then <way>` to say who answers the requests of the top of the organization.

### Choices, masters and rules

The values of a `choice` are the names on the lines under it, each with its title. `choice from
<alias>.<enum>` chooses a value of an enum of a rule the request reads, and the lines under it give
each value its title; a value the enum does not have is E004. `choice from <alias>.<master>`
chooses a row of a master in effect on the day the request is made: the ticket keeps its key, and
the pages and the screens show the row of that day. Its rows are the master's, so no line is written
under it (E001).

### Approvers' fields and edits

A field marked `approvers only` is not on the request: an approver fills it in on the card (an
account code, the reviewer of a contract, the risks legal notes). `approvers edit <field>, …` under
`ticket` lets an approver change a field the requester filled in. When one does, the route after the
stage at hand is decided again with the new value, and the answers of the stages before it stay; a
field the route reads that approvers may edit is W108. A field a hold reads (its condition or an
argument) that approvers may edit is E405: a hold is made once, with the values the request was
made with.

```ringi
form
  counterparty : text "Counterparty"  count >=1 <=100
  kind : choice "Kind of contract"
    nda     "Non-disclosure agreement"
    service "Service agreement"
    license "License agreement"
  value    : money[JPY] "Contract value"  range >=0JPY <=1_000_000_000JPY
  months   : number "Term in months"  range >=1 <=60
  document : attachment "Contract"
  text     : long text "Text of the contract"  count >=1 <=20_000
  reviewer : person "Reviewer"  approvers only
  # A non-disclosure agreement has no value to weigh: legal notes its risks on the others alone.
  risks    : long text "Risks legal notes"  count <=2_000  approvers only  optional  shown when kind is not nda
```

### Labels

The titles are written in the language of the file. `labels <lang>` (`ja` or `en`) gives the texts
in the other language, which the screens and the pages of that language use. The keys are `request`
(the title of the kind of request), a field, `<rows>.<field>`, `<field>.<value>`, `stage <name>` and
`who <name>`. A label for what the file does not have is E007. In a language a request declares, a
text the screens show with no label is E703, and `ritsu gen --target requests` writes nothing.

```ringi
labels ja
  request            "経費の申請"
  category           "種別"
  category.travel    "出張"
  category.expense   "経費"
  category.purchase  "購買"
  items              "明細"
  items.what         "品目"
  items.price        "金額"
  amount             "合計"
  purpose            "目的"
  used_on            "使った日"
  trip_to            "行き先"
  receipt            "領収書"
  account            "勘定科目"
  stage precheck     "事前の確かめ"
  stage first        "一次承認"
  stage second       "二次承認"
  stage decide       "決裁"
  who manager        "上長"
  who dept_head      "部長"
```

## The route

`route` is written as a table, or as stages one a line with a condition where one is needed.

### Stages

| Stage | Written | When it is passed |
|---|---|---|
| an approval | `first  manager` | one of the approvers approves (one person of a team) |
| a joint stage | `dept_head and accounting` (a cell of the table) | one of each way approves |
| a quorum | `board  at least 60% of board`, `at least 2 of legal`, `all of legal` | as many as the quorum asks approve; a share of the approvers is rounded up (`at least 60%` of a board of 5 is 3) |
| a circulation | `inform  circulate to general_affairs` | it is shown and the ticket goes on at once |
| an agent's answer | a stage whose way is `agent <flow>, else <way>` | the flow answers `ok`; on `doubt` or a failure the `else` way answers it |
| a summary | `summary  note from summarize` | the flow's summary is put on the cards of the later stages; it decides nothing |
| an output of a rule | `decide  authority.decider` | the way the rule's output names (each value a name in `who`, E203) |

A quorum stage that a rejection puts out of reach (the others can no longer approve enough) is
rejected there, and the ticket with it; while it can still be reached, it waits.

### The route as a table

The columns before `->` are the inputs, fields of the form or outputs of a rule; those after it are
the stages. A cell of a stage names a way of `who`, `a and b` is a joint stage, and an empty cell
is a stage the request does not come to. The cells of the inputs are rulec's (`-` is any value,
`not: …`, `none`).

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

The table is lowered to a rule, and rulec proves that every request the form takes comes to exactly
one row: a request no row takes is E301 (the note gives the row to add, with `?` for each
approver), two rows that take one request E302, a row no request comes to E303, and a way no row
gives a stage W303. A table whose output column is `-> route`, its cells the aliases of shared
routes, chooses the whole route.

### Stages a line each

```ringi
route
  # The president has no manager: the stage is skipped for them.
  first     manager
    when empty escalate, then skip
  technical reviewer               when reviewer is not none
  # Finance checks what the table of authority sends it, and a purchase over 30% paid in advance.
  finance   finance                when authority.finance or advance > 30%
  board     at least 60% of board  when authority.board
    due in 10 business days at end of day
    when late expire
  # The president is the top of the table of authority, and nobody above decides their own purchase:
  # it goes to the board, and one of the directors decides it.
  decide    authority.decider
    when empty escalate, then board
  inform    circulate to general_affairs
```

A stage is its name, how it is answered, and `when <condition>` where it comes to some requests
only. The lines indented under a stage are its own deadlines (`board` above), in place of those of
`every stage`.

### Who answers

`who` gives each way to choose an approver a name.

| Way | Written | Who it is |
|---|---|---|
| a person | `counsel = person "e9001"` | that person |
| the requester's manager | `manager = manager of requester` | the head of the requester's main team; where that is the requester or nobody, the head of the team above |
| the head of a kind of team | `dept_head = head of requester.department` | the head of the first team of that kind above the requester's main team |
| the nearest of a position | `division_head = nearest division_head above requester` | the first head above the requester whose position is that one or higher |
| a team | `accounting = team accounting` | any one of the team (a quorum counts them) |
| chosen by the requester | `reviewer = chosen by requester in reviewer` | the person the requester puts in a `person` field |
| chosen by the approver before | `reviewer = chosen by previous approver in reviewer` | the person the approver of the stage before puts in an `approvers only` field |
| looked up in a master | `system_owner = lookup m.systems.owner by system` | the person in the row the form chose |
| an agent | `check = agent precheck_agent, else manager` | the flow; where it doubts, the `else` way |

```ringi
who
  manager      = manager of requester
  system_owner = lookup m.systems.owner by system
  security     = team it_security
  operations   = team it_operations
```

A way is solved when its stage starts, in the version of the organization in effect then, and the
proxies in effect that day are added beside the people it finds. A stage chosen by someone needs
the field filled in before it: chosen by the requester in a field approvers fill in, by the
approver before in a field the requester fills in, or by the approver before in the first stage, is
E305.

### Examples of the route

`examples` is a table with the columns of the route: each row is a request and the approvers its
stages come to. rulec holds each row the route reads to the table (E601 where it answers
otherwise). A row is a request someone could make, so a value the form does not take is E601 at its
cell, whether the route reads the column or not.

```ringi
examples
| category | amount       | -> precheck | first   | second                   | decide        |
| travel   | 30_000JPY    |             |         |                          | travel_agent  |
| travel   | 30_001JPY    |             |         |                          | manager       |
| expense  | 100_000JPY   | check       |         |                          | manager       |
| expense  | 100_001JPY   | check       | manager |                          | dept_head     |
| purchase | 1_000_001JPY |             | manager | dept_head and accounting | division_head |
```

### Deadlines and reminders

`every stage` gives every stage its deadline, its reminder, what is done when it is late and what
is done when nobody answers it; lines indented under a stage change them for that stage.

```ringi
every stage
  due in 3 business days at end of day
  remind after 2 business days at 09:00
  when late escalate by next_up, then expire
  # The president has nobody above: for their own requests, a stage that asks the line above goes
  # to the board, and one of the directors answers it.
  when empty escalate, then board

table next_up
| approver      | -> goes_to    |
| check         | manager       |
| travel_agent  | manager       |
| manager       | dept_head     |
| dept_head     | division_head |
| accounting    | division_head |
| division_head | division_head |
| board         | board         |
```

- `due in N business days at end of day` (or `at 17:00`): N business days of the calendar from the
  day the stage starts, read in the calendar's UTC offset.
- `remind after N business days at 09:00`: one reminder to the approvers, set only where it comes
  before the deadline. It can count in another calendar (`remind after 1 business day in plant at
  09:00`).
- `when late escalate`: the part not yet answered goes up, with a deadline and a reminder of its
  own, counted from when the first deadline ran out. Up is the head above the approver who is late
  (for a team, the head of the team above); `by <table>` says where each way goes instead (the
  table `next_up`: its input column is the way that is late, `->` the way it goes to). `, then
  expire` ends the ticket at the second deadline, `, then escalate` goes up once more, `, then stay`
  waits.
- `when late expire` ends the ticket as `expired`; `when late stay` waits, and W402 says it may never
  end.
- A request whose deadlines run out on the head of the organization, or on a way that finds nobody
  above, expires at that deadline, as written; `check` counts them under `ok`, and W507 says where it
  is a hole in the data.

The deadlines are lowered to a `.cal` of koyomi, which counts them and proves the claim `every stage
reminds before it is due` (E403 where a reminder does not come first). A deadline the data of the
calendar cannot count is E404.

### When nobody answers a stage

A way can find nobody: a team with no head, a team with nobody in it, a person not in the version,
or only the requester where self approval is not allowed. `when empty` says what happens:

| Written | What happens |
|---|---|
| `when empty stay` (the default) | the ticket waits at the stage until an administrator adds an approver |
| `when empty skip` | the stage is skipped, and the history says why |
| `when empty escalate` | it goes up to the first head above who is not left out (the requester, and one who made the request for them); at the top of the organization it waits, as `then stay` does |
| `when empty escalate, then skip` | at the top, it is skipped |
| `when empty escalate, then board` | at the top, it goes to the way `board` of `who` |
| `when empty escalate, then stay` | at the top, it waits |

The way after `then` is solved once; where it has nobody either, the ticket waits. Up is tried for
as long as someone is above, so `when empty` has no second `escalate` (`then escalate` is E001, as
is `then` after `stay` or `skip`).

Where a ticket would stop and wait (E502), `check` says for which requesters, in which version, and
how to write it otherwise. What goes up or is skipped by the shape of the organization (the
president has no manager; the head of a department makes the request) is counted under `ok`; what
goes up for a hole in the data (an empty seat) is W502; a requester whose every stage is skipped,
whose ticket would be approved with nobody answering it, is W510.

### Shared routes

A route several requests use is a file of its own: its `inputs`, its route, `who` and `every
stage`, and `labels` for its stages and ways in another language.

```ringi
route manager_approval v1 "Approval by the manager"
description "A route several requests share (fictional rules): the requester's manager approves, and the head of the department too when the request says it is long"

use organization org from "company.ringi"
use calendar office from "office.cal"

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
  # This route asks the line above the requester. The person at the top of the organization has
  # nobody above to ask, and the stages are skipped for them.
  when empty escalate, then skip

labels ja
  stage manager  "上長"
  stage head     "部長"
  who manager    "上長"
  who dept_head  "部長"
```

A request reads it with `use route` and uses it with what it gives each input:

```ringi
use route standard from "shared/manager_approval.ringi"
…
route standard(long: days > 5)
```

The route is lowered with what the request gives: `when long` becomes `when days > 5`. An input
the request does not give is false for a bool, its computation for a computed one, and no value for
the rest. The types and the number of the inputs are checked (E307). `requester` is the requester
of the request that uses it, and an `every stage` of the request wins over the route's.

## The ticket

`ticket` holds the rules of the ticket. Each line but `on` is written once (E006).

```ringi
ticket
  self approval  not allowed
  # A stage right after one the same person approved is done with that approval. No stage waits: the manager answers before accounting.
  same approver  approves once
  send back      to the requester, at most 3 times
  resubmit       within 3 business days
  withdraw       until decide
  # The president, the heads of divisions and the directors are above every department: their
  # requests hold no department's budget.
  on submitted   hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: requester.department, amount: amount)  when requester.department is not none
  on approved    post budget.reserve(request: ticket.id, attempt: ticket.attempt)
  on approved    launch pay(request_id: ticket.id, requester: requester.id, amount: amount, purpose: purpose)
  on sent back, rejected, withdrawn, expired  void budget.reserve(request: ticket.id, attempt: ticket.attempt)
```

| Line | What it says |
|---|---|
| `self approval not allowed` (the default), `self approval allowed` | whether the requester, and one who made the request for them, may answer a stage of it |
| `same approver approves once` | a stage right after one the same person approved is done with that approval |
| `same approver approves once, at the last stage` | one who answers several stages alone answers once, at the last of them; the earlier ones wait |
| `submitted by <groups> [except <groups>]` | who may make the request |
| `send back to the requester, at most N times` | an approver may send it back to the requester |
| `send back to any earlier stage, at most N times` | to the requester, or to a stage someone answered |
| `resubmit within N business days` | how long the requester has to make it again |
| `withdraw until <stage>`, `withdraw until it is decided` | until when the requester may take it back |
| `retract approval until the next stage answers` | an approver may take their approval back while the next stage has not answered |
| `pause at most N business days` | an approver may pause the stage, its clocks stopped, for N business days in all |
| `approvers edit <field>, …` | fields an approver may change |
| `comment required for reject, send back` | a rejection or a send back needs a reason |
| `on <ending>, … hold\|post\|void <book>.<transfer>(…) [when …]` | what is done in a book as the ticket comes to an ending |
| `on <ending>, … launch <flow>(…) [when …]` | a flow started |

The endings are `submitted`, `approved`, `sent back`, `rejected`, `withdrawn` and `expired`.
`resubmit within` and `pause at most` count in the first calendar of the request, or in the one
named after them (`in plant`).

### Self approval and the same approver

`self approval not allowed` leaves the requester out of every way; with nobody left, `when empty`
decides. A company-wide rule is the gate's, and the ticket hands the gate each stage's
`self_approval`; a gate that lets a requester decide their own request where the route does not
is ritsu's E911 (Across the languages, below).

When one person answers two stages of a ticket (a manager who is the head of the department too),
`check` says W506 for each version and requester. `same approver approves once` does a stage with
the approval of the stage just before it, where no other person's stage, agent or summary comes
between them. `same approver approves once, at the last stage` has the person answer once, at the
last stage they answer alone, after the stages between: the earlier stage waits, sets no clock,
and is done with that approval. Both count a person only where a stage comes to one person (not a
team of two or more, a quorum, an agent or a stage chosen by someone). The purchase example writes
the second:

```ringi
ticket
  self approval     not allowed
  # One who answers two or more stages (a manager who also decides) answers once, at the last of
  # them: the decision is made after finance and the board have answered.
  same approver     approves once, at the last stage
  send back         to any earlier stage, at most 3 times
  resubmit          within 10 business days
  withdraw          until decide
  retract approval until the next stage answers
  approvers edit    summary
  # The president, the heads of divisions and the directors are above every department: their
  # requests hold no department's budget.
  on submitted      hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: requester.department, amount: amount)  when requester.department is not none
  on approved       post budget.reserve(request: ticket.id, attempt: ticket.attempt)
  on approved       launch order(request_id: ticket.id, vendor: vendor, amount: amount, summary: summary)
  on sent back, rejected, withdrawn, expired  void budget.reserve(request: ticket.id, attempt: ticket.attempt)
```

### Who may submit

`submitted by` narrows who may make the request; with no line, anyone in the version may.

| Group | Who |
|---|---|
| `everyone` | anyone in the version |
| `everyone in <team>` | those whose main team is the team or under it |
| `team <team>` | the members of the team (main team or not) |
| `position <position>`, `position <position> or higher`, `… in <team>` | by the position in the main team, in the order the organization declares |
| `person "<id>"` | that person |

Groups are joined with `,`; one who is in any of them may submit, and one in any group after
`except` may not. It is read when the request is made, in the version in effect: a ticket goes on
when its requester later moves out, and a resubmission is not refused. A team or a person not in the
version is E508; a version where nobody may submit is E509. The leave example:

```ringi
ticket
  # The leave rules are for the staff (fictional): the members of the board do not submit them.
  submitted by   everyone except team board
  self approval  not allowed
  # A manager who is also the head of the department approves once for both: the two stages follow each other.
  same approver  approves once
  send back      to the requester, at most 2 times
  # A reject and a send back say why, in the comment the requester reads.
  comment required  for reject, send back
  resubmit       within 5 business days
  withdraw       until it is decided
  on submitted   hold leave.take(request: ticket.id, attempt: ticket.attempt, person: requester.id, days: days)  when kind is paid
  on approved    post leave.take(request: ticket.id, attempt: ticket.attempt)  when kind is paid
  on sent back, rejected, withdrawn, expired  void leave.take(request: ticket.id, attempt: ticket.attempt)  when kind is paid
```

### Sending back, withdrawing, rejecting, retracting, pausing

- **Send back.** To the requester, the ticket goes `with requester`, its `round` goes up when it is
  made again, and the route starts again from the first stage. To an earlier stage, the answers from
  that stage on are emptied and the round stays. Send backs and withdrawals are counted together;
  at the limit an approver can only approve or reject.
- **Resubmit.** Only the fields that change are given; the others keep their values. A field is
  emptied by naming it (`resubmit(note: none)` in a scenario, `clear` in the JSON), and a field the
  new values hide loses its value. Past `resubmit within`, the ticket expires.
- **Withdraw.** The requester takes the ticket back until the stage `withdraw until` names answers;
  it goes back to them, to be made again or dropped (`discard`, which ends it as `withdrawn`). Where
  no send back is left, or the request has no `send back` line, it ends as `withdrawn` at once.
- **Reject.** The ticket ends as `rejected`, and cannot be made again.
- **Retract.** The approver of the stage before takes their approval back while the next stage has
  not answered; the ticket goes back to that stage with its deadline as it was, and where that has
  passed, its clock rings at once.
- **Pause.** The approver stops the stage's clocks; the pause ends by itself when the days run out
  (`pause ends`), and the deadline moves by the business days it was paused.

### Comments and reasons

Every operation of a person can carry a comment, up to 2,000 characters, kept in its row of the
history; the event `comment` writes one alone, and changes nothing else. `comment required for
reject, send back` refuses those operations with no comment, or one of white space only. Asking a
reason for an operation the ticket never has is E409. Who may write a comment is the gate's to say.

### Proxies

A proxy is a row of `proxies.csv` in the organization's data (below). An approving proxy answers
in the seat of the one they stand in for, and is counted once for that seat; a submitting proxy
makes the request for the one they stand in for, who is the requester. Every row of the history a
proxy makes says whom they stood in for (`proxy_of`). A proxy does not fill a stage that has nobody.

### Administrators' operations

An administrator of the team in charge (`owner`) can add an approver to the stage at hand (`add
approver`) and hand an approver's part over to someone else (`hand over`). Neither moves a
deadline. Who may do them is the gate's to say. A stage that stops (`when empty stay`, a quorum a
team that shrank cannot reach) waits for them:

```ringi
# A lawyer makes a contract request after the legal team has grown smaller (the organization from
# 2027-04-01). The second review needs two lawyers, and only one is left who is not the requester:
# the stage stops, and waits until an administrator adds an approver.
scenario a_lawyer_after_the_team_shrinks
  requester e9003
  form counterparty = "Example Printing (fictional)", kind = service, value = 6_000_000JPY, months = 12, document = "contract.pdf", text = "The contractor prints the catalogs of the company for twelve months (fictional)."
| at               | event                                                    | by    | -> stage | status      |
| 2027-04-12 10:00 | submit                                                   | e9003 | manager  | in progress |
| 2027-04-12 11:00 | approve                                                  | e0102 | summary  | in progress |
| 2027-04-12 11:05 | note ready(summary: "A one-year contract for printing") |       | intake   | in progress |
| 2027-04-12 14:00 | edit(reviewer: person "e9002")                           | e9002 | intake   | in progress |
| 2027-04-12 14:05 | approve                                                  | e9002 | review   | in progress |
| 2027-04-13 10:00 | approve                                                  | e9002 | second   | in progress |
| 2027-04-13 11:00 | add approver(approver: person "e0102")                   |       | second   | in progress |
| 2027-04-13 15:00 | approve                                                  | e9002 | second   | in progress |
| 2027-04-14 10:00 | approve                                                  | e0102 |          | approved    |
```

### Books and flows

An `on` line holds an amount in a book when the request is made (`hold`), posts it when it is
approved (`post`), lets it go on the other endings (`void`), or starts a flow (`launch`). `post`
and `void` act only on a hold that is open; a hold is keyed by the request and its attempt
(`ticket.attempt`), which every retry of one operation shares. `check` proves that every ending a
hold can reach posts or voids it (E405), that a `post` or a `void` has a hold (E406), that an
ending the ticket never has has no `on` line (E407), and that the key of the hold takes the attempt
(E408). The arguments are fields, `ticket.id`, `ticket.attempt`, `requester.id` and
`requester.<kind of team>`. A submission whose hold cannot be made (`dept: requester.department`,
for a requester with no department above them) is refused as `book`, and `ritsu check` says which
requesters who may submit would be (W209, below).

## Claims

```ringi
claims
  the_agent_decides_up_to_30000_yen : decide is travel_agent only when amount <= 30_000JPY
  a_joint_second_stage              : possible second is dept_head and accounting
  reminder_before_due               : every stage reminds before it is due
  ends_in_time                      : every ticket ends within 93 business days
```

| Claim | What it proves |
|---|---|
| `<stage> is <way> only when <condition>` | the stage comes to that way only where the condition holds (rulec, E306) |
| `possible <stage> is <answer>` | some request comes to it (rulec, E306) |
| `every stage reminds before it is due` | every reminder comes before its deadline (koyomi, E403) |
| `every ticket ends within N business days` | the longest a ticket can take is N business days or fewer (E401) |

## Scenarios

A scenario is a ticket from submission to its end: the requester, the transfers the books start
with (`given`), the form, and a table of events. Each row is the time, the event, who did it (empty
for a clock or a flow), what they wrote (a `comment` column, where a row writes one), and the stage
and the state the ticket is at after it.

```ringi
# The manager asks who covers, sends the request back with what to write, answers where to write it
# when the requester asks, and approves the request made again: each says why in the comment.
scenario a_leave_sent_back_with_a_comment
  requester e1203
  given leave.grant(person: "e1203", year: "2026", days: 20)
  form kind = paid, first_day = 2026-12-21, days = 3, reason = "A family visit"
| at               | event                           | by    | comment                                    | -> stage | status         |
| 2026-11-16 09:00 | submit                          | e1203 |                                            | manager  | in progress    |
| 2026-11-16 11:00 | comment                         | e1101 | "Who covers for you while you are away?"   | manager  | in progress    |
| 2026-11-16 13:00 | comment                         | e1203 | "e1201 does"                               | manager  | in progress    |
| 2026-11-16 17:00 | send back                       | e1101 | "Write e1201 as the cover, please"         |          | with requester |
| 2026-11-16 17:30 | comment                         | e1203 | "Where do I write the cover?"              |          | with requester |
| 2026-11-16 17:45 | comment                         | e1101 | "In the cover field of the form"           |          | with requester |
| 2026-11-17 09:00 | resubmit(cover: person "e1201") | e1203 | "e1201 is written as the cover"            | manager  | in progress    |
| 2026-11-17 10:00 | approve                         | e1101 | "Have a good trip"                         |          | approved       |
```

The arguments of an event are in brackets: `send back(to: first)`, `edit(reviewer: person
"e9003")`, `resubmit(cover: person "e1201")`, `add approver(approver: person "e0102")`, `hand
over(from: person "e1101", to: person "e3101")`, `agent answered(verdict: doubt, reason: "…")`,
`note ready(summary: "…")`. A `submit` by someone else than the requester is made by a proxy. The
clocks are rows too (`remind`, `late`, `resubmit late`, `pause ends`), written at the minute the
machine set them. The dates and the balances are not written: the machine works them out. `check`
runs the scenarios too, and a row that does not go as written is E602 (another stage or state),
E603 (an event refused, a row after the end) or E604 (a clock at another time, or a row after a
clock that rang).

## The ticket machine

A ticket is moved by one pure function, `next(ticket, event, organization, now)`, which answers the
next ticket and the things to do, or refuses the event; `start(submit, organization, now)` makes the
ticket. Both read nothing but what they are given. The reference interpreter in ringi's crate is
this machine, and the code `ritsu gen` writes (the kaname module `ticket`, with a module a request)
answers the same on every vector.

### States

| State | What it holds | Ended |
|---|---|---|
| `in progress` | the stage at hand, the answers, the approvers, the clocks, how many times it went up | no |
| `paused` | the same, and when the pause runs out | no |
| `with requester` | the day to resubmit by, and who sent it back | no |
| `approved` | when it ended | yes |
| `rejected` | when it ended, who rejected it, at which stage | yes |
| `withdrawn` | when it ended, who dropped it | yes |
| `expired` | when it ended, the stage that ran out of time | yes |

Every ticket holds the form, the version of the request, its `round`, its `attempt`, how many send
backs and edits it had, the pause left, the holds open, and its `revision` (`{round, stage,
edits}`), which moves when the stage moves, the ticket goes back or ends, or an edit is taken.

### Events

| Event | From | Taken in |
|---|---|---|
| `submit` | the requester or a proxy | (a new ticket) |
| `resubmit` | the requester's side | `with requester` |
| `approve`, `reject`, `send back` | an approver of the stage at hand, or a proxy | `in progress` |
| `retract` | who approved the stage before | `in progress`, before the next stage answers |
| `pause` | an approver of the stage at hand, or a proxy | `in progress` |
| `resume` | an approver of the stage at hand, or a proxy | `paused` |
| `withdraw` | the requester's side | `in progress`, `paused`, until the stage `withdraw until` names |
| `discard` | the requester's side | `with requester` |
| `edit` | an approver of the stage at hand | `in progress` |
| `add approver`, `hand over` | an administrator | `in progress`, `paused` |
| `comment` | whom the gate allows | `in progress`, `paused`, `with requester` |
| `remind`, `late` | a clock | `in progress` |
| `pause ends` | a clock | `paused` |
| `resubmit late` | a clock | `with requester` |
| `agent answered`, `note ready`, `flow failed` | a flow | `in progress`, `paused` |

The requester's side is the requester, one who made the request for them, and their submitting
proxy in effect that day. An event a person makes carries the ticket's revision they were shown;
one made on an older revision is refused as `stale`, and the screen is shown again.

### Refusals

An event the machine does not take leaves the ticket as it was and is refused with a kind and a
sentence in English and Japanese (`text`, `text_ja`):

| Kind | When |
|---|---|
| `state` | the state does not take it (an ended ticket; `approve` `with requester`; a withdrawal past its stage) |
| `clock` | a clock not set, or early; another event while a clock that rang has not been given |
| `stale` | the revision is not the ticket's |
| `person` | not an approver or a proxy, one who already answered, the requester (self approval), one not in the version |
| `event` | an operation the request does not have, a send back to a stage that cannot take it, an approver added twice |
| `limit` | no send back, withdrawal or pause left |
| `form` | a value the form does not take, a required field emptied, an approval with the next stage's person not chosen |
| `comment` | a comment over 2,000 characters, an empty `comment`, a reason asked for and not given |
| `submit` | a requester `submitted by` does not take |
| `data` | a day outside the days the request can be used on, a deadline the calendar cannot count, the organization of another day |
| `book` | a hold whose argument cannot be made (no department above the requester) |
| `unread` | a lowered file that cannot be read, a rule with no single answer |

### The things to do

`next` returns data, and the server does it, each with an idempotency key (the ticket, the round
and a serial number), so doing one twice is doing it once. In one transition they come in this
order: `record` (a row of the history), `cancel timer`, `post`, `void`, `hold`, `launch`, `ask
agent` and `ask note`, `notify` (`waiting`, `reminder`, `went up`, `returned`, `decided`,
`circulation`, `paused`, `resumed`, `stopped`, `comment`), and `set timer`.

### Time and the organization

Time is the instant of the event, in seconds of UTC. The day of the request is read in the UTC
offset of its first calendar that writes one (`utc_offset_minutes` in `types/<name>.json`), and
picks the version of the organization; each calendar counts its business days from its own day.
The organization is handed as the part the ticket and the event need, from the version in effect on
that day: the people they name (the requester, the approvers, who answered, the people the form
names), the rows of the masters the form chose and the people those name, what the kind needs for
every ticket (`organization.needs` in `types/<name>.json`: the people and the teams its routes and
`submitted by` name, and the teams with every member), and, of all these, the teams, the teams above,
their heads, and the proxies in effect. A part of another day is refused as `data`.

- A stage whose approvers are found keeps them across a change of the version; the next stage is
  decided in the version in effect when it starts.
- A requester gone from the version does not have their stages skipped: a stage whose way reads the
  requester waits for an administrator, whatever `when empty` says, and a ticket `with requester`
  can be neither made again nor dropped, and expires at its deadline.
- A new version of the request (`v2`) does not change a ticket in progress: it ends as the version
  it was made with says.

### Every ticket ends

Every state that is not an ending has a clock out: the deadline, the pause left, the day to
resubmit by. Send backs are limited, a retraction does not move a deadline, pauses are limited in
all. `check` counts the longest a ticket can take, in business days: a stage takes N, or 2N + e
going up once, 3N + 2e twice (e is 1 where the deadline is the end of a day); a round S is the
stages of the longest line added; and the ticket (K + 1) × S + K × R + P, with K send backs, R days
to resubmit and P days of pause. A stage that waits past its deadline, a stage with no deadline and
a send back with no `resubmit within` are waits with no end: `may never end: …` under `ok`, W402 for
`stay`, and E401 where a claim `every ticket ends within` is written.

## The organization

```ringi
organization company v1 "Company"
description "The shape of the company's organization (fictional): the positions from the lowest, the kinds of teams from the widest, and the folder that holds the data of each version"

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

`positions` are listed from the lowest, `team kinds` from the widest; `nearest <position> above
requester` and `position … or higher` compare in that order. `data` is the directory of the
versions, from the file. Under it, each version is a directory named by the day it takes effect,
holding three CSV files (or one `organization.json` with `teams`, `people` and `members`):

| File | Columns | Columns that may be left out |
|---|---|---|
| `teams.csv` | `id`, `name`, `kind`, `parent`, `head` | `name_ja` |
| `people.csv` | `id`, `name` | `name_ja`, `email`, `language` (`en` or `ja`) |
| `members.csv` | `person`, `team`, `position`, `primary` (`yes` for the main team) | |

```csv
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e0001
sales_div,Sales Division,営業本部,division,company,e0101
```

A version is in effect from its day to the day before the next one; a version dated ahead is a
change scheduled, and `check` reads every version laid there. `--at <date>` reads only the one in
effect that day:

```console
$ ritsu ringi check examples/shared/company.ringi --at 2026-11-16
ok examples/shared/company.ringi: an organization, 5 positions, 5 kinds of team
  data `../org`: the version from 2026-04-01, the one in effect on 2026-11-16 (`--at`), 14 teams and 24 people, 2 proxies
```

`--org <dir>` reads that directory in place of every `data` line (a next version
made elsewhere, checked before it replaces the data). Data that does not read (a column missing, an
ID twice, a kind or a position not declared, a team that is its own parent, a tree deeper than 30)
is E501 at its line, and a request that reads it waits for it (E202).

`proxies.csv`, beside the versions, lists the proxies, across the versions:

```csv
person,proxy,kind,from,to,requests
e3101,e1101,approve,2026-12-28,2027-01-08,expense expense_ja purchase purchase_ja
e1203,e1201,submit,2026-12-01,2026-12-31,leave leave_ja
```

`kind` is `approve` or `submit`; `from` and `to` take both days in (an empty `to` has no end);
`requests` lists the ASCII names of the kinds of request it is for, empty for every one. A proxy
or the one they stand in for not in a version the row overlaps is E505.

What `check` says of the organization, version by version and requester by requester:

| What happens | Said |
|---|---|
| a stage with nobody where the ticket stops and waits | E502 |
| a stage with nobody that goes up or is skipped by the shape of the organization | counted under `ok` |
| the same for a hole in the data (an empty seat, an empty team, someone not in the version) | W502 |
| `escalate` from a person or a team not in the version | E503 |
| a quorum the team cannot reach without the requester | E504 |
| one person at two stages, not done once by `same approver` | W506 |
| a late stage with nowhere to go up by the shape of the organization | counted under `ok` |
| the same for a hole in the data | W507 |
| `submitted by` naming a team or a person not in the version | E508 |
| nobody who may submit | E509 |
| requesters whose every stage is skipped | W510 |
| no data at all | W500: none of the above is decided |

## Masters

```ringi
masters company_masters v1
description "The lists the requests choose from or look up approvers in (fictional): account codes, vendors and the company's systems with their owners. Each row says from when and until when it may be chosen"
use organization org from "company.ringi"

master accounts "Account codes"
  key code
  fields
    code  : text "Code"  count <=10
    label : text "Name"  count <=60
  data "../masters/accounts.csv"

master vendors "Vendors"
  key id
  fields
    id   : text "Vendor ID"  count <=12
    name : text "Name"  count <=100
  data "../masters/vendors.csv"

master systems "Systems"
  key id
  fields
    id    : text "System"  count <=30
    name  : text "Name"  count <=100
    owner : person "Owner"
  data "../masters/systems.csv"
```

A master has a `key`, its `fields` and its `data`, a CSV with a column for each field and
`valid_from` and `valid_to`, the first and the last day a row may be chosen (both days in; empty is
no end):

```csv
id,name,owner,valid_from,valid_to
erp,Accounting system,e1001,2026-01-01,
crm,Customer system,e3101,2026-01-01,
```

A row that does not read is E501, a person a row names who is not in a version its days overlap
E505 (W500 where the masters read no organization), and a day of the request's days with no row to
choose is E107:

```console
$ ritsu ringi check tests/mutants/E107_a_hole_in_the_account_codes.ringi
error[E107]: tests/mutants/E107_a_hole_in_the_account_codes.ringi:6:3: The master `m.accounts` has no row to choose on 2026-07-01 to 2026-07-31
     6 |   account : choice from m.accounts "Account code"
  = The request is used from 2026-01-01 on, and the field `account` reads the master on each of those days.
  = The row before: `tests/mutants/shared/accounts_with_a_hole.csv`, line 2 (`5103`, from 2026-01-01 to 2026-06-30); making its `valid_to` 2026-07-31 fills it.
  = The row after: `tests/mutants/shared/accounts_with_a_hole.csv`, line 3 (`5110`, from 2026-08-01 with no end); making its `valid_from` 2026-07-01 fills it.
```

## The days a request can be used on

A request is used from a first day to a last one. The first is `effective from`, else the first day
every piece of data it reads is there (the first version of the organization, the first row of each
master it chooses from, the first day of each calendar's data). The last is the last day it all is:
the last row of a master, and the last day a calendar's data can count every deadline, reminder and
day to resubmit of a stage that starts on it (a pause moves it earlier by its days). A calendar
with no table of closed days has no end. `check` says both under `ok`, with the data that decides
them; a hole inside them is an error (E107 for a master, E502 for no version on `effective from`,
E404 for a calendar).

The end is no mistake: data ends. `--through <date>` asks that the request be usable through that
day, and says E107 or E404, with the day it can be used through and what to extend, where it is
not; `ritsu check --through <date>` asks it of every request of the project, as a CI does before a
new year. Outside its days, the machine refuses a new request, a resubmission and an event that
starts a stage as `data`.

## What `check` says

```console
$ ritsu ringi check examples/leave.ringi
warning[W502]: examples/leave.ringi:22:7: In the organization from 2027-04-01, the stage `head` goes elsewhere or is skipped for lack of an approver
    22 | route standard(long: days > 5)
  = `dept_head` comes to the team legal (Legal), whose head is empty.
  = The stage is `when empty escalate, then skip`, and it goes up.
  = It goes up to e0102 (Head of Corporate Division) for 2 requesters: e9002 (Lawyer 2) and e9003 (Lawyer 3).
  = Correct the data of the version, or make sure this is meant.
ok examples/leave.ringi: a request, 6 fields, 2 stages
  used from 2026-04-01: the first version of the organization `shared/company.ringi` (data `../org`)
  used with no end: no data it reads ends
  ends within 40 business days at the most (3 × 10 at its stages, 2 × 5 with the requester)
  submitted by 19 of 24 people in the organization from 2026-04-01, and 18 of 23 from 2027-04-01 (`everyone except team board`)
  requesters whose stages (`head`) go elsewhere or are skipped by the shape of the organization: 9 of 19 in the organization from 2026-04-01, and 8 of 18 from 2027-04-01
  requesters whose late stages (`manager`, `head`) have nowhere to go up to by the shape of the organization: 2 of 19 in the organization from 2026-04-01, and 2 of 18 from 2027-04-01
  who they are, and where their stages go: `ritsu ringi simulate examples/leave.ringi --every-requester`
```

A file that passes ends with its `ok` line, and under it what was found: the data it reads and the
days it can be used on, the longest a ticket takes, who may submit, the requesters whose stages go
elsewhere by the shape of the organization, and the command that lists them one by one. An
organization and masters say what data they read:

```console
$ ritsu ringi check examples/shared/company.ringi examples/shared/masters.ringi
ok examples/shared/company.ringi: an organization, 5 positions, 5 kinds of team
  data `../org`: 2 versions (2026-04-01, 2027-04-01), 13 teams and 23 people in the last, 2 proxies
ok examples/shared/masters.ringi: 3 masters
  data: accounts 4 rows, vendors 3 rows, systems 3 rows
```

A directory is every `.ringi` under it (but for directories of lowered files, `*.lowered`, and
names that start with `.`). A file given twice is checked once. `check` runs the scenarios of a
request too, but not with `--at`, since a scenario's rows are dated and `--at` reads one version.
With `--lang ja`, or `RINGI_LANG=ja`, it says the same in Japanese:

```console
$ ritsu ringi check examples/leave.ja.ringi --lang ja
警告[W502]: examples/leave.ja.ringi:22:7: 2027-04-01 からの組織では、ステップ `head` は承認者がいないので、ほかへ回るか飛ばされます
    22 | route 標準(long: 日数 > 5)
  = `dept_head` はチーム legal（法務部）に当たり、その長が空いています。
  = ステップは `when empty escalate, then skip` なので、上へ回ります。
  = e0102（管理本部長）へ回る申請者 2 人: e9002（弁護士 2）、e9003（弁護士 3）。
  = 版のデータを直すか、そのままでよいかを確かめてください。
ok examples/leave.ja.ringi: 申請の種類、フィールド 6 個、ステップ 2 個
  2026-04-01 から使えます: 組織 `shared/company.ringi` の最初の版（データ `../org`）
  終わり無く使えます: 読むデータに終わりがありません
  長くても 40 営業日で終わります（ステップに 3 × 10、申請者のもとで 2 × 5）
  出せる人: 2026-04-01 からの組織の 24 人のうち 19 人、2027-04-01 からの 23 人のうち 18 人（`everyone except team board`）
  組織の形で、ステップ（`head`）がほかへ回るか飛ばされる申請者: 2026-04-01 からの組織の 19 人のうち 9 人、2027-04-01 からの 18 人のうち 8 人
  組織の形で、期限を過ぎたステップ（`manager`、`head`）を上へ回せない申請者: 2026-04-01 からの組織の 19 人のうち 2 人、2027-04-01 からの 18 人のうち 2 人
  だれの申請の、どのステップがどこへ回るか: `ritsu ringi simulate examples/leave.ja.ringi --every-requester`
```

### A row missing from the table

```ringi
form
  amount : money[JPY] "Amount"  range >=1JPY <=1_000_000JPY

route
| amount       | -> check | decide  |
| <=100_000JPY | finance  | manager |
```

```console
$ ritsu ringi check tests/mutants/E301_no_row_over_100000_yen.ringi
error[E301]: tests/mutants/E301_no_row_over_100000_yen.ringi:6:1: No row of the route takes some requests
     6 | route
  = The requests no row takes: amount from 100_001JPY to 1_000_000JPY
  = A request no row takes: amount = 100_001JPY
  = Add this row to the table, writing in place of each `?` who answers that stage (a name in `who`; a blank cell where those requests skip it): | >100_000JPY | ? | ? |
  = The cells are cut only at the bounds the table already writes and at the ends of the ranges, so the rows close the gap as they are; where the approvers change inside a row, split it there.
```

The requests no row takes are said in the terms of the form, with one of them; the row to add is
written in the table's own columns, a `?` for each stage to fill in with a way of `who`, or to leave
empty where those requests skip it. E302 says the two rows that take one request, and the request.

### A stage with nobody

```ringi
route
  check  manager

who
  manager = manager of requester
```

```console
$ ritsu ringi check tests/mutants/E502_the_president_has_no_manager.ringi
error[E502]: tests/mutants/E502_the_president_has_no_manager.ringi:9:10: In the organization from 2026-04-01, the stage `check` has no approver
     9 |   check  manager
  = `manager` finds no head above the requester.
  = For 1 requester: e01 (President (fictional)).
  = The stage is `when empty stay`: a ticket that comes to it waits there until an administrator adds an approver.
  = Correct the data of the version, or change the way to choose the stage's approver or its `when empty`.
```

An organization's diagnostic says the version, the stage, why nobody is there, the requesters (as
many as three by name, the rest by number) and what to write; the same said of two versions is said
once. `--format json` gives every requester by ID in `witness`.

### What the languages a request is lowered to say

A request lowers its route and form to rules (`route.rule`, `form.rule`), its deadlines to dates
(`terms.cal`), and its wiring to a module of kaname; what those checks say is said as ringi's:

| Said of the lowered file | ringi's code |
|---|---|
| rulec E101: a request no row takes | E301 |
| rulec E105: two rows take one request | E302 |
| rulec E102: a row no request comes to | E303 |
| rulec W111: a way no row gives | W303 |
| rulec E104: a computation with no rounding | E104 |
| rulec E150: a computed range no value meets | E103 |
| rulec E146: a `possible` that does not happen | E306 |
| rulec E107, E019: an example that does not hold | E601 |
| koyomi E301: a reminder not before its deadline | E403 |
| koyomi E203: a day counted outside the calendar's data | E404 |
| rulec E108 of the copy the generated code reads: a computation past int64 | E201, with the fields to narrow |
| anything else | E201 (error) or W201 (warning), with the language, the code and its text in a note |

The note `From rulec: …` names the language and its code; `--format json` gives them in `from`,
and the example request in `witness`. The lowered files are written to the disk only by `extract`.

## Across the languages

`ritsu check` holds each request to the files it reads in other languages (the details are in the
ritsu skill's `between.md`):

| Check | What it proves | Code |
|---|---|---|
| X18, the gate and the route | every operation the server asks the gate has an action that takes no input of its own; the approvers the route gives may approve; a requester may not decide their own request where the route says `self approval not allowed`, and may where it allows it | ritsu E911, W911 |
| X19, the books and the ticket | a hold is not let expire before the longest its round can stay open; every requester who may submit has the value an argument reads (`requester.department`) | ritsu E209, W209 |
| X20, the flows and the ticket | what `launch`, an agent and a summary give a flow fits its inputs; an agent answers `verdict` (`ok`, `doubt`) and `reason`; a flow that opens a ticket through the API calls the operation the request writes now (`x-ritsu-request`), not an older one | ritsu E210, W210 |

They stop `ritsu check` (exit 1) and `ritsu gen` (nothing written); their warnings do not.
`ritsu explain E911` (and `E209`, `E210`) shows each. A requirement of yuen names what a request
holds with `satisfied by ringi "<file>" <kind> <name>`, and `verified by` a claim, an example or a
scenario of it.

```console
$ ritsu check examples/expense.ringi examples/leave.ringi
…
ritsu check: 2 files (ringi 2): all pass (7 warnings); borders between the languages: 22 checked, 0 undecided
```

## Running the scenarios (`run`)

`run` gives each row of a scenario to the machine at the time it is written at, and says where the
ticket is after it, whom it waits for, when it is due, and what it had the books and the flows do;
the balances of the books at the end; and whether it went as written.

```console
$ ritsu ringi run examples/leave.ringi --scenario a_long_paid_leave
2026-11-16 09:00  submit   by e1203  -> manager (e1101), due 2026-11-18 end of day; hold leave.take 8 day
2026-11-16 17:00  approve  by e1101  -> head (e3101), due 2026-11-18 end of day
2026-11-17 09:00  remind             -> head (e3101), due 2026-11-18 end of day; reminded e3101
2026-11-19 00:00  late               -> head (e0101), due 2026-11-23 end of day; head went up to e0101
2026-11-19 10:00  approve  by e0101  -> approved; post leave.take
the book `leave` at the end: leave_left(e1203) 12 day, granted -20 day, taken 8 day
  the hold `take` (T-1, attempt-1): posted
scenario `a_long_paid_leave`: as written (5 rows)
```

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

It runs every scenario without `--scenario`. A scenario stops at its first row that does not go as
written, with the diagnostic (E602 to E604) and where the ticket is.

## Trying a requester (`simulate`)

`simulate` picks a requester, the values of the form and a day, and shows who answers each stage
and when the first stage is due, in the version of that day:

```console
$ ritsu ringi simulate examples/leave.ringi --requester e1203 --on 2026-11-16 --form 'kind = paid, first_day = 2026-12-21, days = 8, reason = "A family visit"'
examples/leave.ringi on 2026-11-16, in the organization from 2026-04-01
  computed: back_on = 2027-01-06
requester e1203 (Sales staff 3)
  manager  manager: e1101 (Manager of Section 1A)
           due 2026-11-18 at the end of the day, reminder on 2026-11-17 09:00
  head     dept_head: e3101 (Head of Sales Department 1)
           due 2 business days after it starts, at the end of the day
```

`--form` is written as a scenario's `form` line. Without it, every stage the route can come to is
shown for each way. A stage with nobody says what happens to it (`it goes up to …`, `the stage is
skipped`, `the ticket waits until an administrator adds an approver`), a late stage with nowhere to
go up says so, and a stage done with an earlier approval, or waiting for a later one, says which.
`--every-requester` shows every person of the version, the ones who come to the same approvers
together: what an administrator reads on the day of a version laid ahead. A requester who may not
submit is refused:

```console
$ ritsu ringi simulate examples/leave.ringi --requester e0201 --on 2026-11-16
error: e0201 (Director 1) may not submit leave.ringi in the organization from 2026-04-01 (`submitted by everyone except team board`)
```

## Vectors (`vectors`)

`vectors` writes transitions of the machine, one JSON object a line: the ticket before, the event,
the part of the organization and the time, and what the machine answered. They are the rows of the
scenarios and what is made from them: approvals one by one up to a quorum, rejections, send backs to
their limit, clocks a minute early, comments up to 2,000 characters and one more, reasons asked
for, fields emptied, events on an older revision, a request made before closed days, every person
as the requester, the proxies, a ticket across two versions, the edges of the form's numbers and
rows, and the rows of the examples. A generated machine and a server, given the same, answer the
same. `--list` says each in a line:

```console
$ ritsu ringi vectors examples/leave.ringi --list
scenario a_long_paid_leave, line 47: submit by e1203 at 2026-11-16 09:00 -> manager, in progress
scenario a_long_paid_leave, line 48: approve by e1101 at 2026-11-16 17:00 -> head, in progress
scenario a_long_paid_leave, line 49: remind at 2026-11-17 09:00 -> head, in progress
scenario a_long_paid_leave, line 50: late at 2026-11-19 00:00 -> head, in progress
scenario a_long_paid_leave, line 51: approve by e0101 at 2026-11-19 10:00 -> approved
scenario a_long_paid_leave, after it ended: comment by e1203 at 2026-11-19 10:00 -> refused (state)
…
```

## The pages (`doc`, `diff`)

`doc` writes a file as one page in the words of the people who approve and manage the requests
(Markdown, or one HTML file with `--format html`): for a request, the days it can be used on, who
may submit it, the form, the route (each case with its stages), how the approvers are chosen, the
deadlines and reminders with dates, the rules of the ticket, the comments, the proxies, the books
and flows, the claims, the examples and the scenarios. A shared route, an organization and masters
each get a page too: a shared route lists the requests of the project that use it. `--longest`
shows the longest a ticket can take, dated from a day (`--on`), and what shortens it by how much.

```console
$ ritsu ringi doc examples/leave.ringi
…
# Leave request
…
Kept by the team Human Resources (`hr`).

The organization is read in each of its versions: from 2026-04-01 and 2027-04-01.

## When it can be used

- From 2026-04-01: the first version of the organization `shared/company.ringi` (data `../org`).
- With no end: nothing it reads ends.

## Who may submit it

It may be submitted by everyone, except the members of the team Board (`board`).

In numbers: 19 of 24 people in the organization from 2026-04-01, and 18 of 23 from 2027-04-01.
…
```

`diff <old> <new>` compares two versions of one file: for a request, the cases whose stages change,
said in the terms of the form, the requesters whose approvers change in each version of the
organization, and the changes of the form, the deadlines, the rules of the ticket and who may
submit, with how each reaches the tickets in progress. `diff --versions <date> <date> <file>` holds
one request to the two versions of the organization in effect on those days:

```console
$ ritsu ringi diff --versions 2026-04-01 2027-04-01 examples/leave.ringi
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
…
```

Both write text for the terminal (`diff`'s default), Markdown, HTML or JSON (`--format`), in the
language of `--lang`. `diff` exits 0 with no difference, 1 with one, and 2 where the two cannot be
compared (another kind or name, a version that does not pass its check).

## What `gen` writes

`ritsu gen --target typescript` (and `python`, `go`, `rust`) writes, beside the code of the rules,
dates, books, gates and flows the requests read: the code of the rules and dates each request is
lowered to, its wiring (a kaname module a request), the machine (the kaname module `ticket`), and a
list of the kinds of request with their `start`, `next` and `worked` entries taking and returning
JSON, so that one server runs every kind with none of their types:

| Language | File | What it holds |
|---|---|---|
| TypeScript | `ticket_kinds.ts` | `contract`, `lang`, `kinds`; `start`, `next`, `worked` |
| Python | `<package>/ticket_kinds.py` | `CONTRACT`, `LANG`, `KINDS` |
| Go | `ticketkinds/ticketkinds.go` | `Contract`, `Lang`, `Kinds` |
| Rust | `ticket_kinds.rs` | `CONTRACT`, `LANG`, `KINDS` |

A call that is not of the contract (a JSON of the wrong shape, a number past the machine's limits)
is refused at the entry with one sentence alike in the four, saying where, what was given and what
is needed (`The argument `submit.form.days` of `start` is "three", and must be an integer`).

`ritsu gen --target requests` writes, once for the project, under `<out>/requests/`:

```console
$ ritsu gen --target requests --catalog https://example.com/a2ui/catalog.json --out generated examples/leave.ringi
generated: generated/requests/types/leave.json
generated: generated/requests/openapi.json
generated: generated/requests/mcp/tools.json
generated: generated/requests/organization/schema.json
generated: generated/requests/organization/columns.md
generated: generated/requests/organization/find.jsonl
generated: generated/requests/vectors/leave.jsonl
generated: generated/requests/vectors/worked/leave.jsonl
generated: generated/requests/a2ui/leave/form.en.json
generated: generated/requests/a2ui/leave/form.ja.json
generated: generated/requests/a2ui/leave/card.en.json
generated: generated/requests/a2ui/leave/card.ja.json
generated: generated/requests/a2ui/leave/list.en.json
generated: generated/requests/a2ui/leave/list.ja.json
generated: generated/requests/a2ui/leave/mine.en.json
generated: generated/requests/a2ui/leave/mine.ja.json
generated: generated/requests/a2ui/leave/history.en.json
generated: generated/requests/a2ui/leave/history.ja.json
generated: generated/requests/a2ui/leave/offered.en.json
generated: generated/requests/a2ui/leave/offered.ja.json
generated: generated/requests/a2ui/catalog.json
generated: generated/requests/a2ui/functions.js
```

| File | What it is |
|---|---|
| `types/<name>.json` | the kind of request: its name, version, language and titles, the days it can be used on, its UTC offset, who may submit, self approval, the organization it reads (with what a server cuts the part of it by, `needs`), the form, the stages (with the operations that ask a reason), the ways, the operations of the gate, the flows |
| `openapi.json` | the server's API: every event a person makes, the lists (`/waiting`, `/mine`), a ticket with its history, the organization, the masters, finding people, teams and rows, the half-filled form (`worked`); the operations that take a form carry `x-ritsu-request` |
| `mcp/tools.json` | the tools an agent is given, and the ones it cannot undo |
| `organization/schema.json`, `organization/columns.md`, `organization/find.jsonl` | the JSON Schema of a version's data, its columns explained, and the vectors of finding people, teams and rows |
| `vectors/<name>.jsonl`, `vectors/worked/<name>.jsonl` | the vectors of the machine, and of the half-filled form |
| `a2ui/<name>/{form,card,list,mine,history,offered}.{en,ja}.json` | the screens, as A2UI 1.0 messages: the request form, the approval card, the lists, the history, and the notice of the day a request may be made from |
| `a2ui/catalog.json`, `a2ui/functions.js` | ritsu's catalog of components, and the functions the screens call to check and work out a form |

`requests` needs the ID of the catalog, from `--catalog <url>` or the `catalog` line of `ritsu.mod`.
A screen that sends a value it does not show is E701 (W702 where it shows it differently), a
number field whose range goes past what a screen holds exactly (2^53 − 1 in the steps of its unit)
E704, and a text with no label in a declared language E703; `ritsu gen` writes nothing then.

### What the server adds

ritsu writes the contract, and a server keeps it. [ringi-server](https://github.com/i2y/ringi-server),
a framework in Go, is one:

| The server | What it holds to |
|---|---|
| keeps the ticket and its history | the ticket as `next` returned it; the history, only added to |
| takes the events | the operations of `openapi.json`, each with an idempotency key, and the revision for all but the clocks and `comment`; the second time a key comes, the first answer |
| one event at a time | each event of a ticket on the ticket saved just before |
| asks the gate | before `next`, with the values ringi gives each stage (`requester`, `requesters`, `approvers`, `owner`, `self_approval`) |
| rings the clocks | `set timer` and `cancel timer` saved with the ticket; a clock that rang given before any other event |
| keeps the books | `hold`, `post` and `void` with chobo's client, in the transaction of the ticket where the book is in its database |
| tells people and starts flows | after saving, from the things saved, by their keys |
| hands over the organization | the part of the version in effect on the event's day |
| serves the screens | the A2UI templates filled with the ticket; a refused operation's sentence in `/refused` |
| shows a ticket | to the people on it and those the gate's `see` allows; 404 to the others |
| authenticates, logs, serves MCP | its own |

It is held to the contract by the vectors (each line gives the same ticket and things to do, or a
refusal of the same kind), by the vectors of finding, and by operations sent at once to one ticket.

## Starting a server project (`init`)

`ritsu ringi init [<dir>] --module <path>` writes the first files of a Go project that serves
requests and approvals on [ringi-server](https://github.com/i2y/ringi-server): one request type
(`purchase.ringi`) with the organization, the calendar and the gate it reads, the code `ritsu gen`
makes of them, a server, its tests and a README. `--mcp` adds the agents' tools, `--temporal` a request type with a flow and a worker, and
`--lang ja` writes the project with Japanese names and a Japanese README. The files are the user's
from then on: `init` writes them once, and writes nothing where any of them is already there. It
runs inside ritsu only, since it runs `ritsu gen`.

## What the language does not have

Some forms are not written, so that one word means one thing across ritsu's languages:

| Not written | Written |
|---|---|
| `step` for a stage (rulec and kaname write it in a rate's type) | `stage` |
| `up` to pass a stage up (a rounding's direction) | `escalate` |
| `start` to start a flow | `launch` |
| `cancel` for taking an approval back | `retract approval` |
| `file` for an attachment | `attachment` |
| `words` for the texts in another language | `labels` |
| `never`, `always` for a claim | `only when`, `possible` |
| `unit` for a team | `team` |
| a condition on the requester, the ticket or two fields | `submitted by`, `when empty escalate, then <way>`, a computed field |

Not in this version, and planned for a later one: a circulation that waits until everyone has read
it; reminders on chosen days or hours, or repeated; a field held from the requester; one condition
for several fields at once; a stage of a kind the table above does not have; a team looked up in a
master, the head of any team the requester belongs to, one of several teams; an approver adding
people to circulate to; importing the organization from another system through its API; an
agent's help for the requester (filling in the form from papers, pointing out what does not match,
finding the kind of request); a flow that waits for the end of a ticket it opened; attachments and
recipients on comments, and editing a comment; notices and approvals in chat, webhooks, single
sign-on, address limits, keeping the audit log for a set time; keeping the kinds and the masters of
a test environment and production alike.

Never: an editor that builds a form and a route on a screen (an agent writes the file, and a person
reads its page); importing a route from CSV (an agent writes it as `.ringi`); running the requests
on an outside workflow engine.

Outside ringi: the server (keeping the tickets, the clocks, the notices, the screens, the
authentication, the MCP server), which keeps the contract above; keeping the files attached and
sending mail; electronic signatures.

## The keywords

| Where | Words |
|---|---|
| at the start of a line | `request` `route` `organization` `masters` `description` `owner` `effective from` `use` `form` `who` `every stage` `table` `ticket` `claims` `examples` `scenario` `labels` `inputs` `positions` `team kinds` `data` `master` `key` `fields` |
| after `use` | `organization` `masters` `calendar` `book` `flow` `rule` `dates` `route` `gate` `from` |
| types | `text` `long text` `number` `money` `rate` `date` `choice` `choice from` `attachment` `person` `team` `bool` `rows`, the quantities `mass` `length` `area` `volume` `duration` `temperature` `sound`, and `step` in a rate's type |
| marks of a field | `range` `count` `optional` `required when` `shown when` `approvers only` `open in` `round`, and the directions `up` `down` `half_up` `half_down` `half_even` |
| stages | `at least` `of` `%` `all of` `circulate to` `note from` `when` `and` |
| ways to choose an approver | `person` `team` `manager of` `head of` `nearest … above` `requester` `chosen by` `previous approver` `in` `lookup … by` `agent` `else` |
| deadlines | `due in` `remind after` `business day(s)` `at end of day` `at` `when late` `when empty` `escalate` `by` `then` `expire` `stay` `skip` |
| `ticket` | `self approval` `allowed` `not allowed` `same approver approves once` `at the last stage` `submitted by` `everyone` `everyone in` `team` `position` `or higher` `in` `person` `except` `send back` `to the requester` `to any earlier stage` `at most` `times` `resubmit within` `withdraw until` `it is decided` `retract approval until the next stage answers` `pause at most` `approvers edit` `comment required for` `reject` `on` `hold` `post` `void` `launch` |
| endings | `submitted` `approved` `sent back` `rejected` `withdrawn` `expired` |
| claims | `only when` `possible` `is` `every stage reminds before it is due` `every ticket ends within` |
| scenarios | `requester` `given` `form`, the columns `at` `event` `by` `comment` `-> stage` `status` |
| events | `submit` `resubmit` `approve` `reject` `send back` `retract` `pause` `resume` `withdraw` `discard` `edit` `add approver` `hand over` `comment` `remind` `late` `resubmit late` `pause ends` `agent answered` `note ready` `flow failed` |
| states | `in progress` `paused` `with requester` `approved` `rejected` `withdrawn` `expired` |
| expressions | `is` `is not` `none` `and` `or` `not` `true` `false` `sum(…)` `+` `-` `*` `/` `business days in` |
| `labels` | the languages `ja` `en` |

Seven words are no name: `and` `or` `not` `is` `none` `true` `false`. A field is not named
`requester` or `ticket`. The symbols are `:` `=` `,` `.` `(` `)` `[` `]` `|` `->` `%` `<` `<=` `>`
`>=`, and `#` for a comment.

## Commands

```console
$ ritsu ringi check examples
$ ritsu ringi check examples/leave.ringi --format json --root examples
$ ritsu ringi simulate examples/expense.ringi --every-requester --on 2027-04-01
$ ritsu ringi run examples/purchase.ringi --scenario the_board_votes_it_down
$ ritsu ringi vectors examples/leave.ringi
$ ritsu ringi api examples/leave.ringi
$ ritsu ringi doc examples/expense.ringi --format html
$ ritsu ringi doc examples/contract.ringi --longest --on 2026-12-25
$ ritsu ringi diff examples/expense.ringi expense.v2.ringi --format markdown
$ ritsu ringi extract examples/expense.ringi --out shared/expense
$ ritsu ringi explain E301 E502
$ ritsu ringi init approvals --module example.com/acme/approvals
$ ritsu gen --target requests --catalog https://example.com/a2ui/catalog.json --out generated examples
```

- `check <path>... [--format json] [--root <dir>] [--org <dir>] [--at <date>] [--through <date>]`:
  everything above. A directory is every `.ringi` under it.
- `simulate <file> (--requester <id> | --every-requester) [--form <values>] [--on <date>] [--org
  <dir>] [--format json]`: who answers each stage, and when the first is due. `--on` is the day of the
  request, else today in the UTC offset of the request's calendars.
- `run <file> [--scenario <name>] [--org <dir>] [--format json]`: the scenarios.
- `vectors <file> [--list] [--org <dir>]`: the vectors, a JSON object a line, or a line of words each.
- `api <file> [--org <dir>] [--at <date>] [--through <date>] [--root <dir>]`: what the file declares,
  as JSON (below).
- `doc <file> [--format markdown|html] [--longest [--on <date>]] [--org <dir>] [--at <date>]
  [--through <date>] [--root <dir>]`: the page.
- `diff <old> <new> [--format text|markdown|html|json] [--org <dir>] [--at <date>] [--root <dir>]
  [--budget <n>]`, and `diff --versions <date> <date> <file>`: what changes. `--budget` caps the cells
  walked to compare the routes.
- `extract <file> --out <dir> [--org <dir>] [--at <date>] [--through <date>]`: the files a request is
  lowered to, on the disk (to move what is written inside a request into files of their own).
- `init [<dir>] --module <path> [--catalog <url>] [--mcp] [--temporal]`: a server project.
- `explain <code>... | --all [--format markdown|json]`: when a diagnostic is printed, how to fix it,
  and the smallest file that prints it.

Every command takes `--lang ja|en` and `--help` (`ritsu ringi help <cmd>` is the same page). A link
named `ringi` to the `ritsu` binary runs `ritsu ringi`. `doc`, `api`, `diff` and `init` write in
English unless `--lang` says otherwise, whatever the environment says; what they print, as every
command's text, follows `--lang`, else `RINGI_LANG`, then `RITSU_LANG`.

### Exit codes

| Code | When |
|---|---|
| 0 | no errors (there may be warnings); every scenario as written; written; no difference (`diff`) |
| 1 | at least one error, a claim or a scenario that fails; a difference (`diff`) |
| 2 | bad arguments or a file that cannot be read; no scenario, no such scenario or no data of the organization (`run`, `vectors`); two files that cannot be compared (`diff`); a file that needs another language given to ringi's own binary (E008) |

## JSON

The keys are English in either language. A name is written by its ASCII name; `check`, `simulate`
and `api` write the name as written beside it, as `written` beside `name` and `<key>_written` beside
another key (`stage_written`, `way_written`). `run` and the vectors, which the machine and the
generated code read, write the ASCII name alone.

`check --format json` writes an object a file: `ringi` (the version), `file`, `ok`, `summary` (the
`ok` line, or null), `facts` (the lines under `ok`, each a `text` and the `cases` it counts, one by
one), `usable` (`from`, `from_by`, `through`, `through_by`: the days it can be used on and the data
that decides them, or null) and `diagnostics`. A diagnostic is ritsu's (`code`, `severity`, `file`,
`line`, `col`, `message`, `notes`, `fix`); one said for a lowered file adds `witness` (the example
request), `rows` (the rows of the table it is about) and `from` (the language and its code, `rulec
E101`), and `fix` holds the rows to add. One of the organization gives the version, the stage and
every requester in `witness`.

`simulate --format json` writes `ringi`, `file`, `on`, `on_given`, `today_in`, `version`,
`computed`, `may_not_submit` and `requesters`, each with `requester`, `made_by` (a submitting
proxy), `holds_refused` (the holds their `submit` would be refused for) and `stages` (each with its
way, its condition, its deadline and the people of each way, proxies and all).

`api` writes what a file declares, for whoever builds a server or a screen: `ringi`, `file`, `kind`,
`name`, `written`, `version`, `title`, `description`, `owner`, `effective_from`, `uses`, `form` (each
field's type, range, count, and whether and when it is required or shown), `items` (each thing the
file holds, with its lines and its definition, as yuen names them), `usable`, `utc_offset_minutes`,
and `ticket`: `self_approval`, `gates` (the operations the server asks the gate), `operations`,
`endings`, `time` (the most business days), `holds` and `flows`.

`run --format json` writes `ringi`, `file` and `scenarios`, each with `scenario`, `as_written`, the
`status` and `stage` it ended at, the `timers` left, its `rows` (each what the machine was given and
what it answered, in the contract's shape), the `books` at the end, and where it `stopped`. A vector
is a line of `request`, `from` (the scenario and its line, or what it was made from), `now`,
`ticket`, `event`, `organization` and `answer` (`ok` with the ticket and the things to do, or `err`
with `refused`, `text`, `text_ja` and `at`).

## Environment

| Variable | What it does |
|---|---|
| `RINGI_LANG` | `ja` or `en`, the language of the text ringi prints when `--lang` is not given (not what `doc`, `api`, `diff`, `init` and the vectors write, which follow `--lang` alone) |
| `RITSU_LANG` | the same, for every language of ritsu, after `RINGI_LANG` |
