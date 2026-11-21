# Diagnostic codes

Written by `ringi explain --all --format markdown`; do not edit.

<a id="e001"></a>

## E001 — A line that does not read

**When**: A line does not follow how ringi is written: a word or a sign ringi does not have, a string or a bracket not closed, a tab, a full-width space, a date that does not exist, blocks out of their order, a block the kind of file cannot do without missing (a request's `form`, `route` and `ticket`), a `choice` with no values, or a master with no `key` or no `fields`. A line that says `step` is told to say `stage`.

**Fix**: Correct it where it points, as the message and the note say. A stage of a route is written `stage`.

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

every step
  due in 3 business days at end of day

ticket
  self approval  not allowed
```

See also: [E006](#e006)

<a id="e002"></a>

## E002 — A name that cannot be used

**When**: A name is one of the seven words conditions are written with (`and` `or` `not` `is` `none` `true` `false`); a field is named `requester` or `ticket`, which expressions read; a way to choose an approver is named `skip` or `stay`, the words of `when empty escalate, then …`; a name that is not ASCII has no ASCII name in brackets after it, or one in brackets that is not an ASCII name; a name is a reserved word of a language the code is generated in (TypeScript, Python, Go); a field of the form or of a row has as its ASCII name a word kaname reserves (`then` `match` `some` and others); a name is one the code kaname generates keeps for its helper module and its types (`kaname` `KanameError` `Q` `Date` `Rat` `glue`); an ASCII name is `_` alone; the ASCII name of a kind of request starts or ends with `_`, or has two `_` in a row; two fields of the form, or of one row, have ASCII names that are one name in the code generated for Go (`a_b` and `aB` are both `AB`); an input column of a named table has the name of a field, and its rows name ways to choose an approver; or a way to choose an approver has the name of the value a stage takes for several ways or a share of one (`a and b` is `a_and_b`, `all of legal` is `all_of_legal`). In the last three, one of the names is a key or a value of a ticket's JSON, which ringi cannot write under another name.

**Fix**: Choose another name. Give a name that is not ASCII an ASCII name in brackets right after it, as in `金額(amount)`: the generated code and the JSON take that name. For a column of a table, give the column of the ways to choose a name no field has. Start the name of a kind of request with a letter, end it with a letter or a digit, and write no two `_` in a row: it begins the names of the files it is lowered to and of the generated code.

**Example**:

```ringi
request example v1 "Example"

form
  ticket : text "Ticket number"  count <=20

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
```

See also: [E003](#e003)

<a id="e003"></a>

## E003 — A name declared twice

**When**: A name, or an ASCII name, is declared twice in one place names are kept: the fields of the form, the values of a choice, the columns of rows, the stages of the route, `who`, the tables and their columns, the aliases of the `use` lines, the claims, the scenarios, the positions and the kinds of team. A field named as a `use` line's alias, `labels` of one language written twice, and one item labelled twice in a `labels` are this too.

**Fix**: Rename one of the two, or make them one.

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance
  finance = team accounting

ticket
  self approval  not allowed
```

See also: [E002](#e002), [E004](#e004)

<a id="e004"></a>

## E004 — A name that is not declared

**When**: A name is read that nothing declares: none of the fields, the values to choose from (a `choice`'s, and those of the rule's enum a `choice from` chooses from), the stages, the ways to choose approvers in `who`, the tables and their columns, the aliases of the `use` lines, the masters and their fields, the positions and the kinds of team of the organization. Something that reads the organization (`manager of requester`, `requester.department`) in a file with no `use organization`, and business days counted where no calendar can be told, are this too. A name of one kind written in several places is said once, where it is first written, and the note gives the lines of the others (a column, once for each rows; a field of a master, for each master).

**Fix**: Choose one of the names the note lists, or declare it. To read the organization, write `use organization`; to count business days, `use calendar`.

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  accounting

who
  finance = team finance

ticket
  self approval  not allowed
```

See also: [E003](#e003), [E308](#e308)

<a id="e005"></a>

## E005 — A `use` that cannot be read

**When**: The file of a `use` line cannot be read: it is not there or cannot be opened, its extension is not that of the kind of `use` (`use calendar` with a `.book`), the first word of a `.ringi` or a `.cal` is not that of the kind (`use organization` with a request), the path leads out of the project, or it is an absolute path (one that starts with `/` or a drive letter).

**Fix**: Correct the path or the kind. A path is written from the directory of the file it is in, and the kinds read: `organization`, `masters` and `route` a `.ringi`, `calendar` and `dates` a `.cal`, `book` a `.book`, `flow` a `.flow`, `rule` a `.rule`, `gate` a `.gate`.

**Example**:

```ringi
request example v1 "Example"

use calendar office from "budget.book"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
```

See also: [E202](#e202)

<a id="e006"></a>

## E006 — A block this kind of file does not take

**When**: A block is written that the kind of file does not have (`route` in an organization, `form` in masters). A block a file takes once written twice, a line a `ticket` takes once (any but `on`: `self approval`, `submitted by` and the rest) written twice, a second line saying the kind of file, and a shared route that reads another route with `use route` are this too.

**Fix**: Move the block to a kind of file that has it, or take it out; the note lists the blocks of the kind at hand, in their order. Routes are put together in a request's file. A block or a line of `ticket` written twice is made one.

**Example**:

```ringi
organization company v1 "Company"

positions
  staff    "Staff"
  manager  "Manager"

team kinds
  department "Department"

route
  check  finance
```

See also: [E001](#e001)

<a id="e007"></a>

## E007 — A label for nothing

**When**: An item of `labels` is for something the file does not have: a field, a value to choose from or a column of rows there is not, a stage the route does not have (`stage …`), a way to choose approvers `who` does not declare (`who …`), or `request` in a file that is not a request.

**Fix**: Make the item name what the file names, or take the line out.

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed

labels ja
  amount         "金額"
  stage check    "確かめ"
  stage approve  "承認"
```

See also: [E004](#e004), [E703](#e703)

<a id="e008"></a>

## E008 — A file that needs another language, given to ringi's own binary

**When**: A file that needs another language is given to the binary of ringi's own crate, which holds none. A request is lowered to a module of kaname's, its form and route to rulec's rules and its deadlines to koyomi's dates, so it needs them; so does a file whose `use` lines read a rule, a calendar, a book, a flow or a gate. It is said once for each language needed, at the first line that needs it, and the exit is 2. An organization and masters need no other language.

**Fix**: Run the same command through ritsu, with every language joined: `ritsu ringi check <file>`.

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
```

<a id="e009"></a>

## E009 — A value that does not fit where it is written

**When**: A value is not of the type where it is written: numbers of different units compared or added (`amount > 30_000` is an amount of yen and a number with no unit), a condition that is not true or false, `range` on a text, `count` on a number, `open in` on what is not a date, an end of a range that is no value or one of another type, or a column of rows read without `sum`.

**Fix**: Write a value of the type the place takes. An amount is written with its unit (`30_000JPY`).

**Example**:

```ringi
request example v1 "Example"

form
  amount  : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  receipt : attachment "Receipt"  required when amount > 30_000

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
```

See also: [E004](#e004), [E101](#e101)

<a id="e101"></a>

## E101 — A number, an amount or a date with no range

**When**: A number, an amount or a date someone fills in has no `range`, or one without its two ends; or `rows` have no upper limit on their number. The range is what the checks reason over and what the screens and the generated code take. A computed field needs none.

**Fix**: Write both ends, as in `range >=1 <=20`; for rows, the most there may be, as in `count >=1 <=20`.

**Example**:

```ringi
request example v1 "Example"

form
  days : number "Days"

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
```

See also: [E102](#e102), [E009](#e009)

<a id="e102"></a>

## E102 — A text with no limit on its length

**When**: A text someone fills in (`text`, `long text`) has no upper limit in `count`: with none, a text of any length is taken.

**Fix**: Write the most characters it takes, as in `count <=100`.

**Example**:

```ringi
request example v1 "Example"

form
  purpose : long text "Purpose"

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
```

See also: [E101](#e101)

<a id="e103"></a>

## E103 — A range a computed field never comes to

**When**: The range written on a computed field and the values its computation can come to (which ringi works out from the ranges of what it reads) do not meet at all (`total = price * quantity` comes to 10,000 yen at most, and the range says `>=20_000JPY <=30_000JPY`). The range written on a computed field is the ends of what it takes: a request whose computed value falls outside it cannot be made, and the screens and the generated code refuse it as they refuse a value outside the range of a field someone fills in; what reads the field (the route's table, the conditions, the other computations, the claims) looks only inside the range. A range wider than what the computation comes to, or one that meets it in part, is no mistake: only a range no request can meet is this error.

**Fix**: Move the range so that it meets what the computation comes to, or correct the ranges of what it reads.

**Example**:

```ringi
request example v1 "Example"

form
  price    : money[JPY] "Unit price"  range >=1JPY <=1_000JPY
  quantity : number "Quantity"  range >=1 <=10
  total    : money[JPY] "Total" = price * quantity  range >=20_000JPY <=30_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
```

See also: [E101](#e101), [E104](#e104)

<a id="e104"></a>

## E104 — A computed amount that does not say how it rounds

**When**: A computed field's expression can come to a value finer than the field's step, and the field has no `round`: an amount of yen times `3%`, a whole number divided.

**Fix**: Say how it rounds and to what, as in `round down(1JPY)` (`up`, `down`, `half_up`, `half_down`, `half_even`).

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  fee    : money[JPY] "Fee" = amount * 3%

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
```

See also: [E009](#e009), [E103](#e103)

<a id="e105"></a>

## E105 — A required field that may be hidden

**When**: A required field can be required while its `shown when` hides it (`required when amount > 30_000JPY` and `shown when category is travel`): the requester cannot fill in what is not shown. rulec is asked through a rule the two conditions are lowered to, and such a request is given. When rulec cannot settle it, W201 says so.

**Fix**: Make the field required only where it is shown, or show it more widely.

**Example**:

```ringi
request example v1 "Example"

form
  category : choice "Category"
    travel  "Travel"
    expense "Expenses"
  amount  : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  receipt : attachment "Receipt"  shown when category is travel  required when amount > 30_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
```

See also: [E106](#e106), [W201](#w201)

<a id="e106"></a>

## E106 — A condition that reads a field before it is filled

**When**: What is decided when the request is made (when a field is shown or required, the route's table and the conditions of its stages, what a shared route is given, the `on submitted` lines) reads a field only approvers fill in (`approvers only`), itself or through a computed field. When the request is made, that field is empty.

**Fix**: Have what is decided when the request is made read only fields the requester fills in; what approvers decide belongs where they choose, at a later stage.

**Example**:

```ringi
request example v1 "Example"

form
  amount  : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  account : text "Account code"  count <=10  approvers only
  receipt : attachment "Receipt"  required when account is "6100"

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
```

See also: [W108](#w108), [E305](#e305)

<a id="e107"></a>

## E107 — A choice from a master with no row on some day

**When**: On some day of the range the request is used in, no row of a master it chooses from or looks up (`choice from`, `lookup`) is in effect: the account codes end in June 2026, and the next row starts in August. The range runs from `effective from` (else the first day every piece of data it reads is there) to the last day it is all there. The diagnostic gives the days of the hole, and how the rows before and after it could be made to fill it. The end of the data is no mistake: `check` says through which day the request can be used. With `check --through <day>`, a master whose rows end before that day is this error too.

**Fix**: Add a row in effect on those days, or extend the `valid_to` or the `valid_from` of the rows around them; for `--through`, add rows in effect through that day.

**Example**:

```ringi
request example v1 "Example"

use masters m from "masters.ringi"

form
  account : choice from m.accounts "Account code"

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
```

`masters.ringi`:

```
masters company_masters v1
description "The account codes a request chooses from (fictional)"

master accounts "Account codes"
  key code
  fields
    code  : text "Code"  count <=10
    label : text "Name"  count <=60
  data "accounts.csv"
```

`accounts.csv`:

```
code,label,valid_from,valid_to
5103,Supplies,2026-01-01,2026-06-30
5110,Supplies and small equipment,2026-08-01,
```

See also: [E404](#e404), [E505](#e505)

<a id="w108"></a>

## W108 — A field approvers may edit that the route reads

**When**: A field approvers may edit (`approvers edit`) is read by the route, itself or through a computed field: by a column of the table, the condition of a stage, what a shared route is given, `lookup … by` or `chosen by requester in`. When it is edited, the route after the stage at hand is decided again with the new value, and the answers of the stages before it stay.

**Fix**: If the route may change, leave it; if it may not, take the field out of `approvers edit`.

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check   finance
  decide  heads  when amount > 50_000JPY

who
  finance = team finance
  heads   = team finance_heads

ticket
  self approval  not allowed
  approvers edit amount
```

See also: [E106](#e106)

<a id="w109"></a>

## W109 — A field that asks for a secret

**When**: The name, the ASCII name or the title of a text field has a word for a secret in it (`password`, `secret`, `token`, `api key`, and their Japanese words). What is filled in stays in the ticket's history, where approvers and whoever reads the pages and the logs see it.

**Fix**: Do not take a secret through a request: pass it through what is made for it (a secret store, a screen the person fills in themselves). If the value is no secret, name and title the field so that it does not read as one.

**Example**:

```ringi
request example v1 "Example"

form
  system   : text "System"  count <=30
  password : text "Password"  count <=50

route
  check  it_desk

who
  it_desk = team it_desk

ticket
  self approval  not allowed
```

See also: [W901](#w901)

<a id="e201"></a>

## E201 — Something written inside does not pass the language it is lowered to

**When**: The rules or the dates a file is lowered to give an error ringi has no code of its own for (a computed field whose value does not fit in int64: rulec's E108); the language's code and words are in the notes. A file on the disk where ringi lays what it lowers (`<name>.lowered/` beside the file) is this too. So is anything kaname says of the code `ritsu gen` lays for a request (its glue, and the machine of the tickets), a warning or a function that is not total too, said at the request's first line: that is a bug in ringi, and the notes give kaname's codes and lines alone.

**Fix**: Read the language's code and words in the notes, and correct the line it points to. For a file in `.lowered/`, remove it or rename it. For the code `ritsu gen` lays, leave the request as it is and report a bug in ringi, with the request and the version of ritsu.

**Example**:

```ringi
request example v1 "Example"

form
  price    : money[JPY] "Unit price"  range >=1JPY <=9_000_000_000_000_000JPY
  quantity : number "Quantity"  range >=1 <=10_000
  total    : money[JPY] "Total" = price * quantity

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
```

See also: [W201](#w201), [E202](#e202)

<a id="w201"></a>

## W201 — The language something written inside is lowered to warns of it

**When**: The rules or the dates a file is lowered to warn of something ringi has no code of its own for (the ASCII name of a field is a keyword of a language the code is generated in, and the generated code writes it with `_` after it: rulec's W121); the language's code and words are in the notes. A claim, or whether a field may be asked for while it is hidden (the question of E105), that the rule it is lowered to cannot settle is this too.

**Fix**: Read the notes, and see that this is what is meant; if not, correct the line it points to (for a name, choose another ASCII name).

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  impl   : choice "Implementation"
    inhouse "In house"
    vendor  "Vendor"

route
| impl    | amount       | -> decide |
| inhouse | <=50_000JPY  | manager   |
| inhouse | >50_000JPY   | head      |
| vendor  | -            | head      |

who
  manager = team managers
  head    = team heads

ticket
  self approval  not allowed
```

See also: [E201](#e201), [E105](#e105)

<a id="e202"></a>

## E202 — A file read with `use` does not pass its check

**When**: A file a `use` line reads does not pass its language's check: ringi's for a file of ringi's (an organization, masters, a shared route), rulec's for a rule, koyomi's for a calendar or a dates file, chobo's for a book, dandori's for a flow, sekisho's for a gate. Up to three of its errors are in the notes. In `ritsu check`, this line is one that waits for that file to pass, and it is printed under the mistake in that file.

**Fix**: Correct the file read until its language's check passes.

**Example**:

```ringi
request example v1 "Example"

use organization org from "company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
```

`company.ringi`:

```
organization company v1 "Company"

team kinds
  department "Department"
```

See also: [E005](#e005)

<a id="e203"></a>

## E203 — A value of a rule's output that names no approver

**When**: A route takes its approvers from the output of a rule it reads with `use rule`, and a value of that output names no way to choose approvers in `who` (the value `cfo` of `authority.decider`). The output's type is read from the rule: an output that names approvers and is no enum is E009, and an output the rule does not have E004.

**Fix**: Add a way to choose approvers to `who`, by that value's name.

**Example**:

```ringi
request example v1 "Example"

use rule authority from "authority.rule"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  decide  authority.decider

who
  manager = team managers

ticket
  self approval  not allowed
```

`authority.rule`:

```
rule authority v1
description "Who decides, by the amount (fictional)"

enum decider = manager "Manager" | cfo "Chief financial officer"

inputs
  amount : money[JPY]  range >=1JPY <=100_000JPY

outputs
  decider : decider

table decider
| amount      | -> decider |
| <=50_000JPY | manager    |
| >50_000JPY  | cfo        |
```

See also: [E004](#e004)

<a id="e301"></a>

## E301 — No row of the route takes some requests

**When**: Some requests fall in no row of the route's table (no row for purchases over one million yen). rulec finds it in the rule the table is lowered to. The requests no row takes are said by the fields' names and units, with one of them as an example, and the rows to add are shown in the table's form: their input cells are cut only at the bounds the table already writes and at the ends of the ranges, and the cells of the stages (of the route, in a table that chooses one) are `?`. A `?` does not pass the check, so a row pasted as it is stops there until its answers are written. Past eight parts, the rows of the first eight are shown, and the next check names the rest.

**Fix**: Add the rows shown to the table, writing in place of each `?` who answers that stage (the shared route, in a table that chooses one); split a row where the answer changes inside it.

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=1_000_000JPY

route
| amount       | -> check | decide  |
| <=100_000JPY | finance  | manager |

who
  finance = team finance
  manager = team managers

ticket
  self approval  not allowed
```

See also: [E302](#e302), [E303](#e303)

<a id="e302"></a>

## E302 — Two rows of the route take one request

**When**: One request falls in two rows of the route's table (`<=100_000JPY` and `>=100_000JPY`). rulec finds it in the rule the table is lowered to, and an example is given. Where the two share more than one request, what they share is said too (or that the two take the same requests, or that one takes every request the other does).

**Fix**: Make one of the ends `<` or `>`, so that the rows do not overlap.

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=1_000_000JPY

route
| amount        | -> decide |
| <=100_000JPY  | manager   |
| >=100_000JPY  | head      |

who
  manager = team managers
  head    = team heads

ticket
  self approval  not allowed
```

See also: [E301](#e301)

<a id="e303"></a>

## E303 — A row of the route no request reaches

**When**: No request falls in a row of the route's table (a row `>1_000_000JPY`, where `amount` goes up to one million yen). rulec finds it in the rule the table is lowered to.

**Fix**: Correct the row's bounds, or take the row out.

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=1_000_000JPY

route
| amount                     | -> decide |
| <=100_000JPY               | manager   |
| >100_000JPY <=1_000_000JPY | head      |
| >1_000_000JPY              | head      |

who
  manager = team managers
  head    = team heads

ticket
  self approval  not allowed
```

See also: [E301](#e301), [W303](#w303)

<a id="w303"></a>

## W303 — An approver of a stage no request reaches

**When**: No request reaches an approver of a stage: the stage's condition never holds within the ranges of what it reads (`when amount > 200_000JPY`, where `amount` goes up to 100,000 yen), or only an `only when` claim names the approver. rulec finds it in the rule the route is lowered to. An approver only a `possible` or an example names is told by the claim or the example that does not hold (E306, E601).

**Fix**: Correct the conditions of the rows or of the stage, or take out of the route what no request reaches.

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance
  head   heads  when amount > 200_000JPY

who
  finance = team finance
  heads   = team heads

ticket
  self approval  not allowed
```

See also: [E303](#e303), [E306](#e306)

<a id="e304"></a>

## E304 — A quorum larger than the approvers it counts

**When**: A quorum of two or more (`at least 2 of …`) counts a way to choose that names one person (`person`, `manager of requester`, `head of`, `nearest`, `chosen by`, `lookup`); or `at least …%` is more than 100%.

**Fix**: Count a quorum over a team (`team legal`); for one person to answer, write the way to choose alone.

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  decide  at least 2 of president

who
  president = person "e0101"

ticket
  self approval  not allowed
```

<a id="e305"></a>

## E305 — A choice the stage before cannot make

**When**: A stage `chosen by previous approver` has no stage before it that always approves (it is the first). A person the approver before chooses in a field the requester fills in, and one the requester chooses in a field only approvers fill in, are this too.

**Fix**: For the approver before to choose, put an approving stage before it and write `approvers only` on the field chosen in; for the requester to choose, write `chosen by requester in …`.

**Example**:

```ringi
request example v1 "Example"

form
  amount   : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  reviewer : person "Reviewer"  approvers only

route
  review  chosen_reviewer

who
  chosen_reviewer = chosen by previous approver in reviewer

ticket
  self approval  not allowed
```

See also: [E106](#e106)

<a id="e306"></a>

## E306 — A claim about the route that does not hold

**When**: An `only when` or a `possible` of the `claims` does not hold of the route (`decide is manager only when amount <= 50_000JPY`, with a row that has the manager decide 50,001 yen). rulec is asked through the rule the route is lowered to, and a counterexample is given.

**Fix**: Read the example, and correct whichever is wrong: the table or the claim.

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=1_000_000JPY

route
| amount       | -> decide |
| <=100_000JPY | manager   |
| >100_000JPY  | head      |

who
  manager = team managers
  head    = team heads

ticket
  self approval  not allowed

claims
  small_by_manager : decide is manager only when amount <= 50_000JPY
```

See also: [E601](#e601), [W303](#w303)

<a id="e307"></a>

## E307 — A shared route given what it does not take

**When**: What a shared route is given in `route name(…)` does not match its `inputs`: a name it does not take, a name given twice, a value of another type, or an input not given that is not `optional`. A cell of the route's table that uses a shared route taking inputs is this too: a cell gives it nothing.

**Fix**: Give what the note lists, by its names and types; use a shared route that takes inputs on a line of its own, as in `route name(input: …)`.

**Example**:

```ringi
request example v1 "Example"

use route standard from "standard.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route standard(days: 5)

ticket
  self approval  not allowed
```

`standard.ringi`:

```
route standard v1 "Standard approval"

inputs
  long : bool

route
  check  finance
  head   heads  when long

who
  finance = team finance
  heads   = team finance_heads
```

See also: [E009](#e009)

<a id="e308"></a>

## E308 — A stage the route does not have

**When**: `withdraw until`, a claim, a column of the `examples` or a line of a scenario names a stage the route does not have (`withdraw until counsel`, and no `counsel` in the route). A name written in several places is said once, where it is first written, and the note gives the lines of the others.

**Fix**: Choose one of the stages the note lists.

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
  withdraw       until counsel
```

See also: [E004](#e004)

<a id="e309"></a>

## E309 — A condition the route or the form cannot read

**When**: A condition of the route (a stage's `when`, a column of the route's table, what a shared route is given, an `only when` claim) or of the form (`shown when`, `required when`) reads what a table cannot hold: the requester (`requester.department`, `requester.id`), the ticket (`ticket.round`), a field of text, of a person or chosen from a master, a computed field that is not a number or a date, two values compared with each other (`amount > limit`), or a computation in the condition (`sum(items.price) > 100_000JPY`). The conditions are lowered to a table of the values they read, which rulec checks, and what is no column of a table cannot be read. What may have no value (a field that may be empty, an output of a rule that may be none, an input of a shared route the request does not give) is read with `none` as one of its values (DESIGN 4.9). A request that passes the check always has its route lowered.

**Fix**: Put a comparison of two numbers, or a computation, in a computed field, and compare that field with a value (`difference : money[JPY] = amount - limit`, then `when difference > 0JPY`). To decide by a field of text, make it a `choice`. To let only some people submit the request, write `submitted by` under `ticket`; to have someone approve the requests of the person at the top of the organization, write where a stage goes when there is no one above (`when empty escalate, then board`, with `board = team board` in `who`).

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  limit  : money[JPY] "Limit"  range >=1JPY <=100_000JPY

route
  check  finance
  audit  board    when amount > limit

who
  finance = team finance
  board   = team board

ticket
  self approval  not allowed
```

See also: [E009](#e009), [E307](#e307), [E502](#e502)

<a id="e401"></a>

## E401 — A ticket that may not end within what the claim says

**When**: `every ticket ends within … business days` does not hold. The most a ticket can take is counted: a stage takes its deadline, and the deadline again each time it goes up when it is late (with the day it goes up on, where the deadline is at the end of a day); the stages of the longest path of the route are added, a stage that comes on a condition counted as coming; that is taken once more for each time the ticket may go back, with the requester's `resubmit within` each time; and the `pause at most` is added. The count is given with how it is made. It is the most that can be, so a claim it meets holds for every ticket; a ticket that takes every one of those days may not exist. A ticket that may never end is this too: a stage that stays when it is late, a stage with no deadline, a ticket that goes back with no `resubmit within`.

**Fix**: Write the count in the claim, or shorten the deadlines, the times it goes back or the pause; give a wait that has no end one (`expire` or `escalate`, `due in …`, `resubmit within …`).

**Example**:

```ringi
request example v1 "Example"

use calendar office from "office.cal"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check   finance
  decide  board

who
  finance = team finance
  board   = team board

every stage
  due in 3 business days at end of day
  when late escalate, then expire

ticket
  self approval  not allowed
  send back      to the requester, at most 2 times
  resubmit       within 5 business days
  pause          at most 4 business days

claims
  ends_in_time : every ticket ends within 30 business days
```

`office.cal`:

```
calendar office v1
description "Saturdays and Sundays are closed (fictional)"
offset +09:00

closed weekly sat, sun
```

See also: [W402](#w402)

<a id="w402"></a>

## W402 — A stage that may wait for ever

**When**: A stage stays when it is late (`when late stay`, `when late escalate, then stay`): it has a deadline, and the ticket does not end at it. With a claim that every ticket ends within some days, it is E401. A stage with no deadline and a ticket that goes back with no `resubmit within` wait with no end too; no warning is given for them, and the line under `ok` says so (`may never end: …`).

**Fix**: If it may wait, leave it; for it to end, write `expire` or `escalate` in place of `stay`.

**Example**:

```ringi
request example v1 "Example"

use calendar office from "office.cal"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

every stage
  due in 3 business days at end of day
  when late stay

ticket
  self approval  not allowed
```

`office.cal`:

```
calendar office v1
description "Saturdays and Sundays are closed (fictional)"
offset +09:00

closed weekly sat, sun
```

See also: [E401](#e401)

<a id="e403"></a>

## E403 — A reminder that does not come before its deadline

**When**: `claims` say `every stage reminds before it is due`, and a reminder may come at its deadline or after it (`remind after 3 business days at 10:00` with `due in 3 business days at 09:00`). koyomi checks it in the dates the deadlines are lowered to, for every day a stage can start on, and the day farthest off is given. A stage whose reminder and deadline are counted in two calendars (`due in 2 business days in office at end of day` with `remind after 2 business days in plant at 09:00`) is checked by ringi, which counts both for every day a stage can start on; the first such day and how many days are so are given.

**Fix**: Make the reminder earlier, or the deadline later; where the two are counted in two calendars, counting them in one is a fix too.

**Example**:

```ringi
request example v1 "Example"

use calendar office from "office.cal"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

every stage
  due in 3 business days at 09:00
  remind after 3 business days at 10:00

ticket
  self approval  not allowed

claims
  reminded_in_time : every stage reminds before it is due
```

`office.cal`:

```
calendar office v1
description "Saturdays and Sundays are closed (fictional)"
offset +09:00

closed weekly sat, sun
```

See also: [E404](#e404)

<a id="e404"></a>

## E404 — Days of use the calendar's data does not cover

**When**: A day the deadlines or a date of the form are counted from in business days can come before the calendar's table of closed days starts: `effective from` before the first day of the table, or a date of the form counted in business days (`back_on = first_day + days business days in office`) whose date counted from may come before it (the table starts in 2026, and `first_day` in October 2025). The end of the table is no mistake: `check` says through which day the request can be used, and which calendar decides it (a stage that starts later, or a date of the form counted from a later day, is refused with what data is missing). With `check --through <day>`, a table that ends before that day is this error too.

**Fix**: Extend the calendar's table of closed days back to that day, or make `effective from` or the range of the date counted from start when the table does; for `--through`, extend the table past that day.

**Example**:

```ringi
request example v1 "Example"

use calendar office from "office.cal"

form
  first_day : date "First day"  range >=2025-10-01 <=2027-12-31
  days      : number "Business days"  range >=1 <=20
  back_on   : date "Back at work on" = first_day + days business days in office

route
  check  manager

who
  manager = team managers

ticket
  self approval  not allowed
```

`office.cal`:

```
calendar office v1
description "Saturdays, Sundays and the holidays of 2026 and 2027 are closed (fictional)"
offset +09:00

source holidays = file "holidays.csv" sha256:5927e83156e00064
  format csv
  covers 2026-01-01..2027-12-31

closed weekly sat, sun
closed holidays
```

`holidays.csv`:

```
2026-01-01,New Year's Day
2027-01-01,New Year's Day
```

See also: [E403](#e403), [E107](#e107)

<a id="e405"></a>

## E405 — A hold an ending may leave open, or an edit may leave unlike the request

**When**: A hold made with `on submitted hold` is neither posted nor voided by some ending the ticket can come to while the hold is open: approved, rejected, sent back, withdrawn, expired (there is no `void` on `expired`). A `post` or a `void` with a `when` of its own counts only where the hold has the same `when`. A `hold` on any ending but `submitted` is this too: no line closes it. So is a hold whose condition (`when`) or whose argument reads a field approvers may edit (`approvers edit`), itself or through a computed field or an output of a rule. A hold is made once, with the values the request is submitted with, and is not made again when an approver edits them: an amount edited is not the amount posted, and once the condition fails, a `post` or a `void` on the same condition is not done and the hold is left open.

**Fix**: Add a `post` or a `void` line for that ending. Where a hold reads a field approvers may edit, take the field out of `approvers edit`.

**Example**:

```ringi
request example v1 "Example"

use calendar office from "office.cal"
use book budget from "budget.book"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  dept   : text "Department"  count >=1 <=20

route
  check  finance

who
  finance = team finance

every stage
  due in 3 business days at end of day
  when late expire

ticket
  self approval  not allowed
  on submitted   hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: dept, amount: amount)
  on approved    post budget.reserve(request: ticket.id, attempt: ticket.attempt)
  on rejected    void budget.reserve(request: ticket.id, attempt: ticket.attempt)
```

`office.cal`:

```
calendar office v1
description "Saturdays and Sundays are closed (fictional)"
offset +09:00

closed weekly sat, sun
```

See also: [E406](#e406), [E407](#e407)

<a id="e406"></a>

## E406 — A post or a void with no hold before it

**When**: A `post` or a `void` has no `hold` before it (`on approved post`, and no `on submitted hold`).

**Fix**: Add `on submitted hold`, or take the line out.

**Example**:

```ringi
request example v1 "Example"

use book budget from "budget.book"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
  on approved    post budget.reserve(request: ticket.id, attempt: ticket.attempt)
```

See also: [E405](#e405)

<a id="e407"></a>

## E407 — An action on an ending this ticket never has

**When**: An `on` line is for an ending the ticket never has: `sent back` with no `send back` line; `withdrawn` with neither a `withdraw` line nor a `send back` line (a ticket that has gone back can be given up by its requester, so `send back` alone allows it); `expired` where no stage expires at a deadline and no turn of the requester at `resubmit within`; `rejected` where no stage is answered by a person.

**Fix**: Take the line out, or allow that ending.

**Example**:

```ringi
request example v1 "Example"

use book budget from "budget.book"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  dept   : text "Department"  count >=1 <=20

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
  on submitted   hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: dept, amount: amount)
  on approved    post budget.reserve(request: ticket.id, attempt: ticket.attempt)
  on rejected, withdrawn  void budget.reserve(request: ticket.id, attempt: ticket.attempt)
```

See also: [E405](#e405)

<a id="e408"></a>

## E408 — A hold keyed by nothing that tells one try from another

**When**: The key of the transfer a hold makes takes nothing that tells one try from another (`ticket.attempt`): it is the request and its `round` alone. A book remembers its answer for each key, a refusal too, and holds nothing again for a key it voided. Where a server keeps the book in a database apart from the tickets, a refusal of the book stays though the ticket is not saved: keyed by the request and its `round`, a request made again with other values, or once the budget has grown, is refused by the same key for the rest of the round, and a later try comes to the key of a hold voided because its ticket was not saved, and holds nothing. `ticket.attempt` is the name of the try the server gives a `submit` and a `resubmit`: the same name for the same operation sent again, a new one for a new operation. Checked where chobo reads the book.

**Fix**: Add a parameter for the try to the transfer and to its key (`attempt: string`, `key request, attempt`), and pass `attempt: ticket.attempt` to the `hold`, the `post` and the `void`.

**Example**:

```ringi
request example v1 "Example"

use book budget from "budget.book"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  dept   : text "Department"  count >=1 <=20

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
  on submitted   hold budget.reserve(request: ticket.id, round: ticket.round, dept: dept, amount: amount)
  on approved    post budget.reserve(request: ticket.id, round: ticket.round)
  on rejected    void budget.reserve(request: ticket.id, round: ticket.round)
```

See also: [E405](#e405), [E406](#e406)

<a id="e409"></a>

## E409 — A reason asked with an operation this ticket never has

**When**: `comment required for` names an operation the ticket never has: `send back` with no `send back` line; `reject` where no stage is answered by a person. Decided as E407 decides an ending.

**Fix**: Take the operation out of the line, or write the line that allows it.

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
  comment required  for reject, send back
```

See also: [E407](#e407), [E006](#e006)

<a id="w500"></a>

## W500 — No organization data, so missing approvers are not decided

**When**: The organization a request reads has no `data` line, or no data where it points, and no `--org` is given: a stage with no approver, a quorum out of reach and one person at two stages are left undecided, the other checks are made, and this warning says so rather than letting it pass as checked. A file of masters whose fields of people cannot be checked against an organization (it reads none, or one with no data) is this too.

**Fix**: Write a `data` line in the organization's file and put the data there, or give the data's directory with `--org`.

**Example**:

```ringi
request example v1 "Example"

use organization org from "company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  manager

who
  manager = manager of requester

ticket
  self approval  not allowed
```

`company.ringi`:

```
organization company v1 "Company"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"
```

See also: [E501](#e501)

<a id="e501"></a>

## E501 — Organization data that does not read

**When**: The organization's data does not read: a column missing, a position or a kind of team that is not declared, a row naming a team or a person that is not there, a ring of `parent`, a head who is not in the team, a person with no primary team or two, an ID twice, a version's directory not named by a date; also a value over its limit (IDs and master keys are 255 bytes, names 200 characters, a master's values 1,000, addresses 254 bytes), a character the server refuses (control, format, line or paragraph separators), an address of the wrong shape, two addresses of one version that fold to the same, a key `organization.json` does not know at its top or inside an element, and a CSV the reader refuses (a row of a different number of values, a row too big, a misplaced quote). It is said at the row of the data's file, and the requests that read the organization wait for it to pass (E202). A master's data that does not read (a column missing, two rows of one key on one day, a value its field does not take) is this error too, at the row of the master's file.

**Fix**: Correct the rows of the data it points to.

**Example**:

```ringi
organization example v1 "Example"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,section,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

See also: [W500](#w500), [E202](#e202)

<a id="e502"></a>

## E502 — A stage with no approver in a version of the organization

**When**: In a version of the organization, for some requester, no one answers a stage, and the ticket would wait there: the stage is `when empty stay` (the default), or `when empty escalate` with nobody above to go up to (the top of the organization), where it says neither `then skip` nor `then <way>`, or the way it names finds nobody either. `manager of requester` for the president, a team whose head is empty, the requester alone where self approval is not allowed, someone not in the organization. One diagnostic is said for each version and stage, with how many requesters it comes to and up to three of them by name. No version in effect on the day of `effective from` is this too.

**Fix**: Correct the data of the version, or write `when empty escalate` (go up) or `when empty skip` for the stage. Where nobody is above, write `when empty escalate, then skip` if the stage may be skipped; if someone must see the request, write after `then` the way to choose who answers it (`when empty escalate, then board`, with `board = team board` in `who`). Where the way after `then` finds nobody, make it a way that has someone in every version.

**Example**:

```ringi
request example v1 "Example"

use organization org from "company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  manager

who
  manager = manager of requester

ticket
  self approval  not allowed
```

`company.ringi`:

```
organization company v1 "Company"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

See also: [W502](#w502), [E503](#e503)

<a id="w502"></a>

## W502 — A stage that goes elsewhere or is skipped for lack of an approver

**When**: In a version of the organization, for some requester, a stage with no approver because the data has a gap goes up to a head above with `when empty escalate`, or is skipped with `when empty skip`; or, for the seat of the head it goes up to is empty, it is skipped with `then skip`, or goes to the way of `then <way>`. A gap in the data is a team whose head is empty, a team with nobody in it, a person or a team the version does not have, or the empty seat of a head on the way up. Whether the seat is to be filled as written or the data is wrong is for a person to say, so it is a warning. A stage that goes elsewhere or is skipped by the shape of the organization (the only approver is the requester, and self approval is not allowed; the requester is at the top; there is no team of the kind above) goes as it is written, and is not warned of: the line under `ok` says for how many requesters, and `ringi simulate --every-requester` shows whose requests go where. The requesters are said by the person the stage goes to.

**Fix**: Correct the data of the version, or make sure this is meant.

**Example**:

```ringi
request example v1 "Example"

use organization org from "company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  head

who
  head = head of requester.department

every stage
  when empty escalate

ticket
  self approval  allowed
```

`company.ringi`:

```
organization company v1 "Company"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

See also: [E502](#e502), [W510](#w510)

<a id="e503"></a>

## E503 — A stage that goes up names someone a version does not have

**When**: The way to choose the approver of a `when empty escalate` stage names a person (`person "e09"`) or a team (`team legal`) a version of the organization does not have: there is nothing to go up from. It is said at the line of the way to choose. With `when empty stay` it is E502, with `skip` W502; with `then skip` or `then <way>` it is this too, for both hold only where nobody is above. A person of a master's row who is not in the organization is E505.

**Fix**: Correct the ID, or add the person (the team) to the data of the version.

**Example**:

```ringi
request example v1 "Example"

use organization org from "company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  decide  counsel

who
  counsel = person "e09"

every stage
  when empty escalate

ticket
  self approval  not allowed
```

`company.ringi`:

```
organization company v1 "Company"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

See also: [E502](#e502), [E505](#e505)

<a id="e504"></a>

## E504 — A quorum some requesters cannot reach

**When**: In a version of the organization, for some requester, the approvers a quorum counts (`at least 2 of sales`) are fewer than it needs: the team, without the requester, is too small.

**Fix**: Lower the quorum, or add people to the team.

**Example**:

```ringi
request example v1 "Example"

use organization org from "company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  review  at least 2 of sales

who
  sales = team sales

ticket
  self approval  not allowed
```

`company.ringi`:

```
organization company v1 "Company"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

See also: [E304](#e304)

<a id="e505"></a>

## E505 — A proxy or a master row for someone not in the organization

**When**: The person or the proxy of a row of the proxies (`proxies.csv`), or the person of a field of people of a master's row, is not in a version of the organization in effect in the row's days (a proxy for someone who has left). It is said at the row of the data's file, for each row and version. A master's rows are checked when its file reads the organization with `use organization`.

**Fix**: Correct the row, or end it.

**Example**:

```ringi
organization example v1 "Example"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

`org/proxies.csv`:

```
person,proxy,kind,from,to,requests
e09,e02,approve,2026-04-01,2026-06-30,
```

See also: [E503](#e503)

<a id="w506"></a>

## W506 — One person at two stages of one ticket

**When**: In a version of the organization, for some requester, one person alone answers two stages chosen in different ways (the manager is the head of the department too; the only member of the team is the one who decides). One person checking twice is not two pairs of eyes. The requesters are said by the one person. Whom a stage goes up to, and a stage any one of several may answer, are not counted. In a request whose `ticket` says `same approver approves once`, a pair whose later stage is done with the approval of the stage right before it is not said, and a pair it does not do is said with why (a stage between them, another part of the earlier stage, an agent's stage). In one that says `same approver approves once, at the last stage`, the earlier stage waits for the approval of the later, and only a pair with a stage an agent answers first is said. Where neither is written, or the line written does not make it one approval, the pair is said with the writing that does.

**Fix**: Change the way to choose one of the stages; if one approval is enough, write `same approver approves once` under `ticket` (done with the approval of the stage right before) or `same approver approves once, at the last stage` (one answer at the last of the stages, whatever comes between them); or make sure it is meant.

**Example**:

```ringi
request example v1 "Example"

use organization org from "company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check   finance
  decide  head

who
  finance = team finance
  head    = person "e04"

ticket
  self approval  allowed
```

`company.ringi`:

```
organization company v1 "Company"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

See also: [E502](#e502)

<a id="w507"></a>

## W507 — A late stage with nowhere to go up to

**When**: In a version of the organization, for some requester, a stage that goes up when it is late (`when late escalate`) cannot go up because the data has a gap: the seat of the head above the one who answers it is empty, or the way to choose a table of `escalate by <table>` sends it to finds nobody for a gap in the data (a team whose head is empty, a team with nobody in it, a person or a team the version does not have). The one who answers is also the one a stage with no approver went to with `when empty escalate` (a head above, or the people of the way of `then <way>`). The second time it goes up (`escalate, then escalate`) is looked at too. A stage that cannot go up does at that deadline what is written after `escalate`: with `then expire`, or nothing, it expires; with `then stay`, it waits on. A stage that cannot go up by the shape of the organization, as when the one who answers is at the top, ends at its deadline as it is written, and is not warned of: the line under `ok` says for how many requesters, and `ringi simulate` says what the stage does when it is late. The requesters are said by the one who is late.

**Fix**: Correct the data of the version; for it to go to someone else, say where it goes in the table of `when late escalate by <table>`.

**Example**:

```ringi
request example v1 "Example"

use organization org from "company.ringi"
use calendar office from "office.cal"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  decide  sales_head

who
  sales_head = person "e02"

every stage
  due in 3 business days at end of day
  when late escalate, then expire

ticket
  self approval  allowed
```

`company.ringi`:

```
organization company v1 "Company"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

`office.cal`:

```
calendar office v1
description "Saturdays and Sundays are closed (fictional)"
offset +09:00

closed weekly sat, sun
```

See also: [W502](#w502), [W402](#w402)

<a id="e508"></a>

## E508 — `submitted by` names a team or a person a version does not have

**When**: The `submitted by` of `ticket` names a team (`everyone in legal`, `team legal`, `position … in legal`) or a person (`person "e09"`) a version of the organization does not have. It is an error even when the other groups have people, so that a section gone in a version, and the people who should submit left unable to, are found before the switch. It is said where the team or the person is written, the versions together; in such a version E509 is not said.

**Fix**: Correct the ID, or add the team (the person) to the data of the version.

**Example**:

```ringi
request example v1 "Example"

use organization org from "company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  submitted by   everyone in legal
  self approval  not allowed
```

`company.ringi`:

```
organization company v1 "Company"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

See also: [E509](#e509), [E503](#e503)

<a id="e509"></a>

## E509 — No one may submit the request in a version of the organization

**When**: In a version of the organization, `submitted by` comes to nobody, or `except` leaves everyone out: no one may submit the request in that version. Where a team or a person it names is not in the version, E508 alone is said.

**Fix**: Widen `submitted by`, or correct the data of the version.

**Example**:

```ringi
request example v1 "Example"

use organization org from "company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  submitted by   team finance except position dept_head
  self approval  not allowed
```

`company.ringi`:

```
organization company v1 "Company"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

See also: [E508](#e508)

<a id="w510"></a>

## W510 — Requesters for whom no stage has anyone to answer it

**When**: In a version of the organization, for some requester, every stage with no approver is skipped (`when empty skip`, `when empty escalate, then skip`), and no other stage has anyone to answer it: a ticket of theirs is approved as soon as it is submitted, with nobody answering it. The stage of the manager, skipped with `then skip` for the president's request because nobody is above, is one. Each stage may be skipped as it is written, by the shape of the organization, and still nobody sees the request, so it is a warning. Only the people who may submit the request (`submitted by`) are looked at. It is said at the line of the first stage that approves, the requesters together for the stages that are skipped.

**Fix**: If someone must see their requests, write after `then` the way to choose who answers them (`when empty escalate, then board`, with `board = team board` in `who`), or correct the data of the version; if they do not make the request, leave them out with `submitted by … except …`.

**Example**:

```ringi
request example v1 "Example"

use organization org from "company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  manager

who
  manager = manager of requester

every stage
  when empty escalate, then skip

ticket
  self approval  not allowed
```

`company.ringi`:

```
organization company v1 "Company"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

See also: [W502](#w502), [E502](#e502), [E509](#e509)

<a id="e601"></a>

## E601 — An example of the route that does not hold

**When**: A row of the `examples` comes to other stages than the route gives. rulec checks it in the rule the route is lowered to, and the route's answer is given. A cell of a row with a value the form does not take is this too: a number or a date outside its range (for a computed field, outside what its computation can come to), a number that is not a multiple of its unit's step, a text whose length is outside its `count`, a key no row of its master has. It is said at the cell, whether the route reads the column or not; a value that is not one to choose from is E004, and a value of another type E009.

**Fix**: Correct whichever is wrong: the example or the table. A value the form does not take is written as one it takes: a row of the examples is a request that can be made.

**Example**:

```ringi
request example v1 "Example"

form
  amount : money[JPY] "Amount"  range >=1JPY <=1_000_000JPY

route
| amount       | -> decide |
| <=100_000JPY | manager   |
| >100_000JPY  | head      |

who
  manager = team managers
  head    = team heads

ticket
  self approval  not allowed

examples
| amount    | -> decide |
| 50_000JPY | head      |
```

See also: [E306](#e306), [E004](#e004), [E009](#e009)

<a id="e602"></a>

## E602 — A scenario that goes another way

**When**: After a row of a scenario, the stage or the state is not what the ticket comes to; the real ones are given, with the clocks that are set. `ringi check` runs every scenario on the ticket machine, and `ringi run` prints each row as far as it goes.

**Fix**: Correct whichever is wrong: the scenario, or the route and the ticket.

**Example**:

```ringi
request example v1 "Example"

use organization org from "company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check   finance
  decide  sales

who
  finance = team finance
  sales   = team sales

ticket
  self approval  allowed

scenario a_small_expense
  requester e03
  form amount = 5_000JPY
| at               | event   | by  | -> stage | status      |
| 2026-06-01 09:00 | submit  | e03 | check    | in progress |
| 2026-06-01 10:00 | approve | e04 |          | approved    |
```

`company.ringi`:

```
organization company v1 "Company"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

See also: [E603](#e603)

<a id="e603"></a>

## E603 — An event of a scenario the ticket does not take

**When**: The ticket does not take a row of a scenario: an event the machine refuses (an `approve` by someone who is not an approver, anything after the ticket has ended, a comment past 2,000 characters), a hold the book refuses, a second `submit`, a time before the row above, an argument the event does not take, a value the form does not take, something in the `by` or the `comment` cell of a clock's or a flow's row.

**Fix**: Correct the event, who makes it, or its time.

**Example**:

```ringi
request example v1 "Example"

use organization org from "company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check   finance
  decide  sales

who
  finance = team finance
  sales   = team sales

ticket
  self approval  allowed

scenario a_small_expense
  requester e03
  form amount = 5_000JPY
| at               | event   | by  | -> stage | status      |
| 2026-06-01 09:00 | submit  | e03 | check    | in progress |
| 2026-06-01 10:00 | approve | e02 | decide   | in progress |
```

`company.ringi`:

```
organization company v1 "Company"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

See also: [E602](#e602)

<a id="e604"></a>

## E604 — A timer at another time

**When**: A clock's row of a scenario (`remind`, `late`, `resubmit late`, `pause ends`) is not at the time the clock is set for (`late` at 23:59, and the deadline at the end of the day), or no such clock is set; or a row is written at the time a clock rings or after it with no row for the clock. The clocks that are set are given, and the row to write. The times are the lowered dates'; a scenario holds none of its own.

**Fix**: Write the clock's row at the time given.

**Example**:

```ringi
request example v1 "Example"

use organization org from "company.ringi"
use calendar office from "office.cal"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

every stage
  due in 3 business days at end of day
  when late expire

ticket
  self approval  allowed

scenario nobody_answers
  requester e03
  form amount = 5_000JPY
| at               | event  | by  | -> stage | status      |
| 2026-06-01 09:00 | submit | e03 | check    | in progress |
| 2026-06-04 23:59 | late   |     |          | expired     |
```

`company.ringi`:

```
organization company v1 "Company"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

`office.cal`:

```
calendar office v1
description "Saturdays and Sundays are closed (fictional)"
offset +09:00

closed weekly sat, sun
```

See also: [E602](#e602)

<a id="e701"></a>

## E701 — A button sends a value the screen does not show

**When**: A button of a screen `ritsu gen --target requests` writes sends a value no part of the screen shows (an approval card shows a category's label and sends its code). Every screen is checked for it before the templates are written. The templates are made from what the request's file declares, so it is said only of a bug in ringi, never of how a file is written.

**Fix**: Leave the request's file as it is, and report it as a bug in ringi, with the request and the version of ritsu (`ritsu --version`).

**Example**: No example: ringi writes no template that gives it, and `ritsu gen` says it only of a bug in ringi.

See also: [W702](#w702)

<a id="w702"></a>

## W702 — A number shown or read in other steps than it is sent in

**When**: A screen `ritsu gen --target requests` writes shows or reads a number a button sends in other steps than it is sent in (fractions of a yen shown as whole yen, a rate in steps of 0.25% read in steps of 0.01%). As with E701, it is said only of a bug in ringi.

**Fix**: Leave the request's file as it is, and report it as a bug in ringi, with the request and the version of ritsu (`ritsu --version`).

**Example**: No example: ringi writes no template that gives it, and `ritsu gen` says it only of a bug in ringi.

See also: [E701](#e701)

<a id="e703"></a>

## E703 — An item a screen shows has no words in a language

**When**: An item a screen shows has no words in a language it needs. Either a language `labels` declares has no label for it (`labels ja` without `items.price`); the items are `request`, the fields (a column of rows as `<rows>.<column>`), the values to choose from (`<field>.<value>`, a value of a rule's enum too), `stage <name>` and `who <name>`. Or an item that comes from another file has no title in the file's language: a value of a rule's enum with no line under its field that titles it and no display name the rule writes in the file's language (said at the field's line), or a stage or a way to choose an approver of a shared route written in another language, which the route's `labels <the file's language>` does not title (said at the `use route` line). `ritsu gen --target requests` says it before it makes the screens, and writes nothing.

**Fix**: Add the item's label to that language's `labels`, title the enum's value under its field (`goods "Goods and services"`), or add the title to the `labels` of the shared route's file.

**Example**:

```ringi
request example v1 "Example"

use organization org from "company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  manager

who
  manager = manager of requester

every stage
  when empty escalate, then skip

ticket
  self approval  not allowed

labels ja
  request      "例"
  amount       "金額"
  who manager  "上長"
```

`company.ringi`:

```
organization company v1 "Company"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

See also: [E007](#e007)

<a id="e704"></a>

## E704 — A number's range a screen cannot hold exactly

**When**: An end of a number field's range is past 2^53 − 1 (9_007_199_254_740_991) counted in the steps of its unit, whether the requester gives the field, an approver does, the form computes it or it is a column of rows. A screen holds a number as JavaScript does, so a value past it would be rounded, and checked and sent as another. `ritsu gen --target requests` says it at the field's line before it makes the screens, and writes nothing.

**Fix**: Narrow the range to within ±9_007_199_254_740_991 steps.

**Example**:

```ringi
request example v1 "Example"

use organization org from "company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=10_000_000_000_000_000JPY

route
  check  manager

who
  manager = manager of requester

every stage
  when empty escalate, then skip

ticket
  self approval  not allowed
```

`company.ringi`:

```
organization company v1 "Company"
description "A small company (fictional)"

positions
  staff     "Staff"
  dept_head "Head of department"
  president "President"

team kinds
  company    "Company"
  department "Department"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

See also: [E101](#e101)

<a id="w901"></a>

## W901 — A value shaped like a key is written in the file

**When**: Somewhere in a `.ringi`, in a string or a comment, there is a value in a shape its provider fixes for a key: an AWS access key ID, a key or token of GitHub, Slack, Stripe, OpenAI, Anthropic or Google, a Slack incoming webhook URL, a PEM private key. It is said of a file that does not read too. The diagnostic gives the kind of key, its prefix and its length, and never the key nor its line.

**Fix**: Keep the key where the code runs (an environment variable, the platform's connection or secret store) and read it from there. If it is real, revoke it with its provider first: taking it out of the file leaves it in the history of the repository. If it is a value for tests, write `ritsu: test secret` in a comment on the same line.

**Example**:

```ringi
request example v1 "Example"
description "Read the maps of the offices with the API key of the staging site: AIzaSyD-ritsu-fake-key-for-tests-000000"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
```

See also: [W109](#w109)
