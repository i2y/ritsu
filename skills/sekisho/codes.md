# Diagnostic codes

Written by `sekisho explain --all --format markdown`; do not edit.

<a id="e001"></a>

## E001 — Something cannot be read as a word of the language

**When**: A character the language does not have, a string not closed, a full-width space outside a string, a date of the wrong shape, or a number with a unit the table of units does not have. Cedar's `==`, `!=`, `&&`, `||` and `!` are not words of sekisho either.

**Fix**: Correct it where it points. The same is `is`, not the same is `is not`, and the others are `and`, `or` and `not`.

**Example**:

```gate
gate t v1
description "not closed
```

See also: [E005](#e005)

<a id="e002"></a>

## E002 — A keyword is used as a name

**When**: A declared name or alias is one of the words conditions, computed values and types are written with (`and`, `or`, `not`, `in`, `is`, `principal`, `resource`, `workflow`, `true`, `false`, `any`, `today`, `bool`, `date`, `number`, `rate`); or where a name goes, something that is no name is written (a string, a number).

**Fix**: Choose another name.

**Example**:

```gate
gate t v1

role is
```

See also: [E007](#e007)

<a id="e003"></a>

## E003 — A section or a line is out of order, repeated, or missing

**When**: The file does not start with a `gate` line; the sections are out of their order; a section written once (`description`, `namespace`, `today`) or a line written once in a block is written twice; a line a block needs is missing (an action's `principal` and `resource`, the `action` of a policy or an expectation, the `actions` of a `separate`); or `attributes`, `input` or `context` has nothing under it.

**Fix**: A file goes: the heading (`gate`), `description`, `namespace`, `use`, `today`, `enum`, `role`, `principal`, `workflow`, `resource`, `action`, `permit` and `forbid`, `expect`, `separate`; the order of a block's lines is in the diagnostic's note.

**Example**:

```gate
gate t v1

resource Order

role clerk
```

See also: [E005](#e005)

<a id="e004"></a>

## E004 — The indentation does not line up

**When**: The indentation has a tab, the lines of one block are not indented alike, or an indented line follows a line that takes none.

**Fix**: Indent with spaces, every line of a block by the same amount; the lines under `attributes`, `input` and `context` one step further.

**Example**:

```gate
gate t v1

role clerk
	description "Answers customers"
```

See also: [E003](#e003)

<a id="e005"></a>

## E005 — A line that cannot be written there

**When**: A block does not take the line's first word (`roles` in a `role`, `when` at the left margin), or the line is not of the form its first word takes (an unknown word after `use`, a `today` without `offset`, `action any` listed with other actions, `actions` with one action).

**Fix**: Write it in the form the note gives.

**Example**:

```gate
gate t v1

role clerk
  roles manager
```

See also: [E003](#e003), [E004](#e004)

<a id="e006"></a>

## E006 — A name is declared twice

**When**: A name or an alias is declared twice among the things of one kind (the types and the enums, the roles, an enum's values, a type's attributes, the workflows, the actions, an action's inputs and computed values, the policies, the expectations, the separations, the names of the `use` lines), also against what a file read with `use gate` declares; or an input of a rule is given twice, or `actions` lists an action twice.

**Fix**: Rename one of them, or give it another alias.

**Example**:

```gate
gate t v1

role clerk
role clerk
```

See also: [E007](#e007)

<a id="e007"></a>

## E007 — A name that goes to Cedar has no ASCII alias, or one of the wrong form

**When**: A name that goes to Cedar (the file, an enum and its values, a role, a principal's or a resource's type, an attribute, a workflow, an action, an input, a computed value, a policy) is not ASCII of the form of an alias and has no alias in parentheses; or its alias is not of the form; or the namespace is not an ASCII identifier. A type's alias is `[A-Z][A-Za-z0-9]*`, any other `[a-z][a-z0-9_]*`. An expectation and a separation do not go to Cedar, and need none.

**Fix**: Give it an alias in parentheses, like `返金する(refund_order)`.

**Example**:

```gate
gate t v1

role Clerk
```

See also: [E002](#e002), [E008](#e008)

<a id="e008"></a>

## E008 — An alias is a reserved word of Cedar or of the generated code

**When**: An alias (or an ASCII name with none) is a reserved word of Cedar (`if`, `then`, `else`, `has`, `like`) or of TypeScript, Python or Go; or a type's name is Cedar's `Action` or one of its builtin types, or `Role` or `Workflow`, which sekisho declares.

**Fix**: Choose another alias.

**Example**:

```gate
gate t v1

role has
```

See also: [E007](#e007)

<a id="e101"></a>

## E101 — A name names nothing

**When**: A role, a type, an attribute, a value of an enum, an action, an input, a computed value, the name of a `use`, a rule's output or input, a date function or a unit that is referred to is not there; or a policy or an expectation names an action of another file.

**Fix**: Correct the spelling, or declare it; the note lists the names there are.

**Example**:

```gate
gate t v1

role clerk

principal User
  roles clerk

resource Order

action view
  principal User
  resource Order

permit clerks_view
  principal in clerck
  action view
```

See also: [E006](#e006)

<a id="e102"></a>

## E102 — A type that does not fit

**When**: True or false compared with a value of an enum, an enum with a number, numbers of different units; a value that is not true or false written alone as a condition; a range on a type that takes none; a value of another type given to a rule's input.

**Fix**: Make the value fit the type of what it is compared with; a number has to be of the same unit (`JPY` and `円` are the same; with tax and without are not).

**Example**:

```gate
gate t v1

principal User
  attributes
    suspended : bool

resource Order

action view
  principal User
  resource Order

forbid suspended_users
  principal is User
  action view
  when principal.suspended is yes
```

See also: [E103](#e103)

<a id="e103"></a>

## E103 — A range that is wrong

**When**: A number or a date (an attribute, an input) has no `range`, or one end of it only, or a range no value is in; a constant does not come to a whole number in its unit, or is past ±(2⁵³ − 1); a constant a condition compares with is outside the range.

**Fix**: Write both ends, like `range >=1GBP <=10_000GBP`, and constants that come to whole numbers in the unit.

**Example**:

```gate
gate t v1

principal User
  attributes
    refund_limit : money[GBP, incl_tax]
```

See also: [E102](#e102)

<a id="e104"></a>

## E104 — A relation v1 does not write

**When**: An attribute is followed by another (`resource.order.customer`); two attributes of different types are compared; an attribute is compared with the principal when no principal of its type comes; a number is compared with another value.

**Fix**: Keep a relation to one step (`resource.customer is principal`, `resource.tenant is principal.tenant`, `principal in resource.team`); an answer that goes two steps can be a true-or-false output of a rule.

**Example**:

```gate
gate t v1

principal User

resource Order

action view
  principal User
  resource Order

permit owners_view
  principal is User
  action view
  when resource.order.owner is principal
```

See also: [E102](#e102)

<a id="e105"></a>

## E105 — A computed value that is wrong

**When**: A computed value calls what is neither a rule nor a date (the name of a `use openapi`); a rule's output is neither an enum nor true or false; the rule walks a list of elements; a computed value or an entity is given to the input of a rule or a date; an input is given nothing.

**Fix**: A computed value is an enum or a true-or-false output of a rule, a test of today against a date (`today <= …`), or a business day (`today is open in …`); an input takes an attribute of the principal or the resource, an input of the action, a constant, or `today`.

**Example**:

```gate
gate t v1

use openapi shop from "shop.json"

principal User

resource Order

action view
  principal User
  resource Order
  context
    allowed = shop(id: 1).ok
```

`shop.json`:

```
{"openapi": "3.1.0", "info": {"title": "shop", "version": "1"}, "paths": {}}
```

See also: [E101](#e101), [E102](#e102)

<a id="e106"></a>

## E106 — A principal the actions never take

**When**: No principal a policy's or an expectation's `principal` line picks (`is <type>`, `is workflow <name>`, `in <role>`) comes to an action it lists; with `action any`, to no action at all.

**Fix**: Correct the `principal` line, or add the type to the action's `principal` line; for a role, write it in the `roles` of a type that holds it.

**Example**:

```gate
gate t v1

principal User

principal Customer

resource Order

action refund
  principal User
  resource Order

permit customers_refund
  principal is Customer
  action refund
```

See also: [E101](#e101)

<a id="e107"></a>

## E107 — Today without its line, or with a time zone's name

**When**: A computed value or a rule's input uses `today`, and there is no `today` line; or the `offset` is a time zone's name, or not `±HH:MM`.

**Fix**: Write the range and the offset the day changes at, as a number: `today range >=2026-10-01 <=2028-10-31 offset +00:00`.

**Example**:

```gate
gate t v1

today range >=2026-10-01 <=2026-12-31 offset Europe/London
```

See also: [E103](#e103)

<a id="e108"></a>

## E108 — The roles' `includes` go round in a circle

**When**: Following the roles' `includes` comes back to the role it started from (`clerk → manager → clerk`); a role that includes itself is a circle too. Each circle is said once, at the `includes` of its first role, with its roles in order.

**Fix**: Remove an `includes` of the circle. `includes` says whoever holds this role holds that one too: in a circle, which role includes which is not decided, and the parents of Cedar's roles would go round too.

**Example**:

```gate
gate t v1

role clerk
  includes manager

role manager
  includes clerk
```

See also: [E101](#e101)

<a id="e201"></a>

## E201 — A file a `use` reads does not pass its language's check, cannot be read, or is outside the root

**When**: A rule of a `use rule` does not pass rulec's check, or a dates file of a `use dates` koyomi's, or it cannot be read (the note says what the language says); or a file of a `use gate` does not pass sekisho's check, cannot be read, or the `use gate` lines go round in a circle; or the file of a `use` is outside the root. A file a gate reads is named by a reference whose path is from the root: the operations its actions guard, the rules, dates and calendars the `@doc` of the generated Cedar names, and what the other languages name of it.

**Fix**: Correct that file until its language's check passes it. For a file outside the root, give `--root` a directory that holds it.

**Example**:

```gate
gate t v1

use gate "people.gate"
```

See also: [E209](#e209)

<a id="e202"></a>

## E202 — `guards` names an operation the contract does not have

**When**: The operation of a `guards` line is not an `operationId` (or a method and a path) or an operation's key of the `use openapi` or `use asyncapi` document, a service and method of the `use proto`, or an operation of a transfer of the `use book`.

**Fix**: Write one of the operations the note lists.

**Example**:

```gate
gate shop v1
description "A shop's orders"

use openapi orders from "orders.json"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  guards orders refundOrders
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order
```

See also: [E203](#e203), [E205](#e205)

<a id="e203"></a>

## E203 — An input is not what the operation takes

**When**: A name under `input` is neither a parameter nor a field of the body of an operation the action guards; or its type is not what the operation takes (a number taken as one with a fraction or as a string, other values of an enum); or its range goes past what the operation takes; or the operation does not require it and it has no `?`.

**Fix**: Write what the operation takes, by the same name and type, with a range within the operation's.

**Example**:

```gate
gate shop v1
description "A shop's orders"

use openapi orders from "orders.json"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  guards orders refundOrder
  principal User
  resource Order from orderId
  input
    amount : money[GBP]  range >=1GBP <=20_000GBP

permit clerks_refund
  description "A clerk refunds up to 50 pounds"
  principal in clerk
  action refund_order
  when amount <= 50GBP
```

See also: [E202](#e202), [E204](#e204)

<a id="e204"></a>

## E204 — `from` names no parameter of the operation's path or query

**When**: The argument of `resource … from <argument>` is not a parameter of the path or the query of an operation the action guards (for a `.proto`, a field of the request); the generated code reads the resource by it.

**Fix**: Write the name of a parameter of the operation's path or query.

**Example**:

```gate
gate shop v1
description "A shop's orders"

use openapi orders from "orders.json"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  guards orders refundOrder
  principal User
  resource Order from order_id

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order
```

See also: [E203](#e203)

<a id="e205"></a>

## E205 — Two actions guard one operation

**When**: Two actions write the same operation of the same contract under `guards`: which of them decides is not settled.

**Fix**: Guard the operation with one action.

**Example**:

```gate
gate shop v1
description "A shop's orders"

use openapi orders from "orders.json"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  guards orders refundOrder
  principal User
  resource Order

action refund_again
  description "Refund an order once more"
  guards orders refundOrder
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order, refund_again
```

See also: [E202](#e202)

<a id="e206"></a>

## E206 — A value given to a rule or a date can be outside its input's range, or break a precondition of the rule

**When**: The range of an attribute, an input or a constant a computed value gives to a rule or a date is not within the range of the input it is given to; or, over the ranges given, an example breaks a precondition (`constraint`) of the rule.

**Fix**: Narrow the ranges of what is given to the input's, or to ones that keep the precondition.

**Example**: put the files below in one directory, and run `ritsu sekisho check example.gate` there.

`example.gate`:

```
gate shop v1
description "A shop's orders"

use rule limit from "limit.rule"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk
  attributes
    refund_limit : money[GBP]  range >=0GBP <=200GBP

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal User
  resource Order
  input
    amount : money[GBP]  range >=1GBP <=100GBP
  context
    band = limit(amount: amount, limit: principal.refund_limit).band

permit clerks_refund_within_their_limit
  description "A clerk refunds up to the clerk's limit"
  principal in clerk
  action refund_order
  when band is within_limit
```

`limit.rule`:

```rule
rule limit v1
description "Whether a refund is within a limit of at most 100 pounds"

enum band = within_limit | over_limit

inputs
  amount : money[GBP]  range >=1GBP <=100GBP
  limit  : money[GBP]  range >=0GBP <=100GBP

outputs
  band : band

derive excess : money[GBP] = amount - limit  range >=-99GBP <=100GBP

table decide
policy unique
| excess | -> band : band |
| <=0GBP | within_limit   |
| >0GBP  | over_limit     |
```

See also: [W201](#w201), [E201](#e201)

<a id="w201"></a>

## W201 — Whether the values given to a rule keep its precondition cannot be decided

**When**: rulec cannot decide whether a precondition (`constraint`) of the rule holds over the ranges given; the generated code checks it when it runs, and denies the request when it breaks.

**Fix**: Narrowing the ranges given can let it be decided.

**Example**: No example yet: the check that prints it comes in a later stage.

See also: [E206](#e206)

<a id="e207"></a>

## E207 — A calendar does not cover every day of `today`

**When**: The days the data of the calendar of `today is open in <calendar>` covers (the days its tables know) do not cover the range of `today`.

**Fix**: Narrow today's range to the data's, or take the calendar's copy again when a newer table is out.

**Example**: put the files below in one directory, and run `ritsu sekisho check example.gate` there.

`example.gate`:

```
gate shop v1
description "A shop's orders"

use calendar days from "closed_days.cal"

today range >=2026-10-01 <=2027-03-31 offset +00:00

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal User
  resource Order
  context
    business_day = today is open in days

permit clerks_refund_on_business_days
  description "A clerk refunds on a business day"
  principal in clerk
  action refund_order
  when business_day
```

`closed_days.cal`:

```cal
calendar closed_days v1

source holidays = file "holidays.csv" sha256:899aee90fcd554a9
  format csv
  covers 2026-01-01..2026-12-31

closed weekly sat, sun
closed holidays
```

`holidays.csv`:

```
2026-01-01,New Year's Day
2026-05-04,Greenery Day
```

See also: [E107](#e107)

<a id="e208"></a>

## E208 — A workflow's `.flow` does not pass dandori's check, cannot be read, or is outside the root

**When**: The flow of `workflow <name> from "<.flow>"` does not pass dandori's check, or cannot be read; or the flow is outside the root (the `@doc` of the generated Cedar and the other languages name it by a reference whose path is from the root).

**Fix**: Fix the flow until dandori's check passes; the notes say what dandori says. For a flow outside the root, give `--root` a directory that holds it.

**Example**: put the files below in one directory, and run `ritsu sekisho check example.gate` there.

`example.gate`:

```
gate shop v1
description "A shop's orders"

workflow returns from "returns.flow"
  description "Refunds a returned order"

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal Workflow
  resource Order

permit returns_refunds
  description "The returns workflow refunds"
  principal is workflow returns
  action refund_order
```

`returns.flow`:

```flow
workflow returns v1
description "Calls a task it does not declare"

inputs
  order_id : string

outputs
  refund : string

flow
  let r = refund_order(orderId: order_id)
  succeed refund = r.id
```

See also: [E201](#e201)

<a id="e209"></a>

## E209 — A file that reads another language, checked with a sekisho that reads none

**When**: A file with `use rule`, `use dates`, `use calendar`, `use book` or `workflow … from`, checked with the binary of sekisho's own crate (`sekisho`), which holds none of rulec, koyomi, chobo and dandori. It is said once, at the first such line, and nothing else is said. The exit code is 2: it is how the command is run, not what the file says.

**Fix**: Run the same command as `ritsu sekisho` (`ritsu sekisho check refunds.gate`), which reads the rules, the dates files, the calendars, the books and the flows in the same process.

**Example**:

```gate
gate t v1

use rule limit from "limit.rule"
```

See also: [E201](#e201)

<a id="e210"></a>

## E210 — A file read with `use gate` is in another namespace

**When**: The Cedar namespace of a file read with `use gate` is not the namespace of the file that reads it. A file with no `namespace` line is in its alias in Pascal case (`Refunds` for `refunds`), so files that read one another with `use gate` take the same `namespace` line.

**Fix**: Write the same `namespace` line in both files (`namespace Shop`).

**Example**:

```gate
gate refunds v1
namespace Shop

use gate "people.gate"
```

`people.gate`:

```
gate people v1
namespace People

role clerk
```

See also: [E211](#e211), [E201](#e201)

<a id="e211"></a>

## E211 — Two files read with `use gate` declare one thing

**When**: Two files read with `use gate` (with what they read in turn) declare a type, an enum, a role or a workflow of the same name or alias: what is read stands in one Cedar namespace, where it would be two things of one name. A file that two of them both read counts once. It is said at the `use gate` line of the file read later.

**Fix**: Declare a thing of one name in one file, and have the other files read that one with `use gate`.

**Example**:

```gate
gate refunds v1
namespace Shop

use gate "people.gate"
use gate "staff.gate"
```

`people.gate`:

```
gate people v1
namespace Shop

role clerk
```

`staff.gate`:

```
gate staff v1
namespace Shop

role clerk
```

See also: [E210](#e210), [E006](#e006)

<a id="e301"></a>

## E301 — No permit allows the action in any combination

**When**: In every combination of the action (the principal's type and roles, the attributes, the inputs, the computed values), no permit applies or a forbid denies it: no one can do the action.

**Fix**: Write a permit for it; if no one is meant to do it, write `nobody "<why>"` under the action.

**Example**:

```gate
gate shop v1
description "A shop's orders"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal User
  resource Order
```

See also: [E302](#e302), [E303](#e303)

<a id="e302"></a>

## E302 — A forbid denies every combination a permit would allow

**When**: Every combination a permit applies to, a forbid applies to as well: the permit allows nothing.

**Fix**: Narrow the forbid, or remove the permit if no one is meant to be allowed this.

**Example**:

```gate
gate shop v1
description "A shop's orders"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order

forbid nobody_refunds
  description "No one refunds"
  action refund_order
```

See also: [E301](#e301), [E303](#e303)

<a id="e303"></a>

## E303 — A permit or a forbid applies to no combination

**When**: The `principal` line and the `when` and `unless` lines of a policy never hold together, in any combination.

**Fix**: Remove the line that cannot be met, or the policy itself.

**Example**:

```gate
gate shop v1
description "A shop's orders"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk
  attributes
    suspended : bool

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order

forbid suspended_and_not
  description "Asks for a suspended user who is not suspended"
  action refund_order
  when principal.suspended
  unless principal.suspended
```

See also: [E302](#e302)

<a id="e304"></a>

## E304 — An expectation does not hold

**When**: A combination an `expect allow` or `expect deny` picks is answered otherwise: how many, and one of them, are shown. So is an action under `nobody` that some combination allows.

**Fix**: Fix the policies, or the expectation if it says more than is meant.

**Example**:

```gate
gate shop v1
description "A shop's orders"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order

expect deny clerks_never_refund
  description "No clerk refunds"
  principal in clerk
  action refund_order
```

See also: [E305](#e305)

<a id="e305"></a>

## E305 — A separation does not hold

**When**: One principal (of one type, holding one set of roles, with one value of each attribute) is allowed two of the actions a `separate` lists; the resource and the context of each may differ.

**Fix**: Add a forbid that keeps whoever is allowed one from the other, or narrow a permit.

**Example**:

```gate
gate shop v1
description "A shop's orders"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal User
  resource Order

action export_refunds
  description "Export the refunds"
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order

permit clerks_export
  description "A clerk exports the refunds"
  principal in clerk
  action export_refunds

separate refunding_and_exporting
  description "Whoever refunds does not export the refunds"
  actions refund_order, export_refunds
```

See also: [E304](#e304)

<a id="e306"></a>

## E306 — A role is allowed an action its `can` does not list

**When**: A principal holding the role alone (and the roles it includes) is allowed, in some combination, an action its `can` line does not list.

**Fix**: Add the action to `can`, or narrow the permit that allows it.

**Example**:

```gate
gate shop v1
description "A shop's orders"

role clerk
  description "Answers customers"
  can view_order

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action view_order
  description "Look at an order"
  principal User
  resource Order

action refund_order
  description "Refund an order"
  principal User
  resource Order

permit clerks_view_and_refund
  description "A clerk looks at an order and refunds it"
  principal in clerk
  action view_order, refund_order
```

See also: [W302](#w302)

<a id="e307"></a>

## E307 — An action comes to more combinations than the budget

**When**: The combinations of an action (with the pairs of today and values its dates are walked over) come to more than the budget (10⁸ by default, `--budget` to change it). The check does not sample: the action is not checked, and nothing is generated.

**Fix**: The note says what makes the most. For roles, split the roles of a type or check each role with `can`; for attributes and constants, have the conditions read fewer values. `--budget` raises the budget.

**Example**:

```gate
gate shop v1
description "Fourteen attributes of four values each: more than 10^8 combinations"

enum level = a | b | c | d

principal User
  description "A member of the staff"
  attributes
    x1 : level
    x2 : level
    x3 : level
    x4 : level
    x5 : level
    x6 : level
    x7 : level
    x8 : level
    x9 : level
    x10 : level
    x11 : level
    x12 : level
    x13 : level
    x14 : level

resource Order
  description "An order"

action read_order
  description "Look at an order"
  principal User
  resource Order

permit level_a_reads
  description "Whoever is at level a everywhere reads"
  action read_order
  when principal.x1 is a and principal.x2 is a and principal.x3 is a and principal.x4 is a and principal.x5 is a and principal.x6 is a and principal.x7 is a
  when principal.x8 is a and principal.x9 is a and principal.x10 is a and principal.x11 is a and principal.x12 is a and principal.x13 is a and principal.x14 is a
```

<a id="w301"></a>

## W301 — Another single permit allows everything a permit allows

**When**: Every combination a permit allows, another single permit allows too: removing it changes no answer. Of two that cover each other, only the one written later is named.

**Fix**: Remove the permit, or narrow the one that covers it if the first says what is meant.

**Example**:

```gate
gate shop v1
description "A shop's orders"

enum status = paid | refunded

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"
  attributes
    status : status

action refund_order
  description "Refund an order"
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order

permit clerks_refund_paid_orders
  description "A clerk refunds a paid order"
  principal in clerk
  action refund_order
  when resource.status is paid
```

See also: [E302](#e302)

<a id="w302"></a>

## W302 — A role alone is never allowed an action its `can` lists

**When**: A principal holding the role alone (and the roles it includes) is denied, in every combination, an action its `can` line lists.

**Fix**: Remove the action from `can`, or write the permit that allows it.

**Example**:

```gate
gate shop v1
description "A shop's orders"

role clerk
  description "Answers customers"

role auditor
  description "Checks the refunds"
  can refund_order

principal User
  description "A member of the staff"
  roles clerk, auditor

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order
```

See also: [E306](#e306)

<a id="w303"></a>

## W303 — An answer rests on a value no language can vouch for, and cannot be decided

**When**: A rule cannot say exactly what its output comes to over the ranges asked (a table over derived values whose rows whole numbers may not reach), and an answer that comes from the values counted in case is given by no concrete input tried; or a Cedar policy read has an expression that is not finite.

**Fix**: Narrow what is given to the rule, or write the rule's table with rows whole numbers reach.

**Example**: put the files below in one directory, and run `ritsu sekisho check example.gate` there.

`example.gate`:

```
gate shop v1
description "A shop's orders"

use rule halves from "halves.rule"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk
  attributes
    first  : number  range >=0 <=3
    second : number  range >=0 <=3

resource Order
  description "An order"

action read_order
  description "Look at an order"
  principal User
  resource Order
  context
    kind = halves(first: principal.first, second: principal.second).answer

permit clerks_read
  description "A clerk reads"
  principal in clerk
  action read_order

permit halves_read
  description "Whoever is at one and a half twice reads"
  principal in clerk
  action read_order
  when kind is odd_half
```

`halves.rule`:

```rule
rule halves v1
description "A row that asks for a sum of 3 and a difference of 0: real numbers reach it, whole numbers do not"

enum kind = odd_half | other

inputs
  first  : number  range >=0 <=3
  second : number  range >=0 <=3

outputs
  answer : kind

derive total : number = first + second  range >=0 <=6
derive gap   : number = first - second  range >=-3 <=3

table t
policy first
| total | gap | -> answer : kind |
| 3     | 0   | odd_half         |
| -     | -   | other            |
```

See also: [W201](#w201)

<a id="w304"></a>

## W304 — An expectation picks no combination

**When**: The `principal`, `when` and `unless` lines of an `expect` hold together in no combination of its actions: the expectation holds, and checks nothing (most often a line is written wrong).

**Fix**: Correct the lines that cannot be met together, or remove the expectation.

**Example**:

```gate
gate shop v1
description "A shop's orders"

enum order_status = paid | refunded

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"
  attributes
    status : order_status

action refund_order
  description "Refund an order"
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds an order not yet refunded"
  principal in clerk
  action refund_order
  unless resource.status is refunded

expect deny nothing_paid_and_refunded_is_refunded
  description "No one refunds an order that is paid and refunded at once"
  action refund_order
  when resource.status is paid
  when resource.status is refunded
```

See also: [E304](#e304), [E303](#e303)

<a id="w401"></a>

## W401 — Over a quota of Verified Permissions

**When**: Under `sekisho gen --authorizer avp`, a generated policy or the schema is over a quota of Verified Permissions (10,000 bytes a policy, 100,000 bytes the schema as JSON, 100 transitive parents of an entity). The roles counted are every role a type can hold, with the roles they include.

**Fix**: Split the policy, spread the types and the actions over several gates, or make the roles nest less deep.

**Example**: put the files below in one directory, and run `ritsu sekisho gen example.gate --target cedar --authorizer avp` there.

`example.gate`:

```
gate deep v1
namespace Shop

role r0
role r1
  includes r0
role r2
  includes r1
role r3
  includes r2
role r4
  includes r3
role r5
  includes r4
role r6
  includes r5
role r7
  includes r6
role r8
  includes r7
role r9
  includes r8
role r10
  includes r9
role r11
  includes r10
role r12
  includes r11
role r13
  includes r12
role r14
  includes r13
role r15
  includes r14
role r16
  includes r15
role r17
  includes r16
role r18
  includes r17
role r19
  includes r18
role r20
  includes r19
role r21
  includes r20
role r22
  includes r21
role r23
  includes r22
role r24
  includes r23
role r25
  includes r24
role r26
  includes r25
role r27
  includes r26
role r28
  includes r27
role r29
  includes r28
role r30
  includes r29
role r31
  includes r30
role r32
  includes r31
role r33
  includes r32
role r34
  includes r33
role r35
  includes r34
role r36
  includes r35
role r37
  includes r36
role r38
  includes r37
role r39
  includes r38
role r40
  includes r39
role r41
  includes r40
role r42
  includes r41
role r43
  includes r42
role r44
  includes r43
role r45
  includes r44
role r46
  includes r45
role r47
  includes r46
role r48
  includes r47
role r49
  includes r48
role r50
  includes r49
role r51
  includes r50
role r52
  includes r51
role r53
  includes r52
role r54
  includes r53
role r55
  includes r54
role r56
  includes r55
role r57
  includes r56
role r58
  includes r57
role r59
  includes r58
role r60
  includes r59
role r61
  includes r60
role r62
  includes r61
role r63
  includes r62
role r64
  includes r63
role r65
  includes r64
role r66
  includes r65
role r67
  includes r66
role r68
  includes r67
role r69
  includes r68
role r70
  includes r69
role r71
  includes r70
role r72
  includes r71
role r73
  includes r72
role r74
  includes r73
role r75
  includes r74
role r76
  includes r75
role r77
  includes r76
role r78
  includes r77
role r79
  includes r78
role r80
  includes r79
role r81
  includes r80
role r82
  includes r81
role r83
  includes r82
role r84
  includes r83
role r85
  includes r84
role r86
  includes r85
role r87
  includes r86
role r88
  includes r87
role r89
  includes r88
role r90
  includes r89
role r91
  includes r90
role r92
  includes r91
role r93
  includes r92
role r94
  includes r93
role r95
  includes r94
role r96
  includes r95
role r97
  includes r96
role r98
  includes r97
role r99
  includes r98
role r100
  includes r99

principal User
  roles r100

resource Doc

action read
  principal User
  resource Doc

permit everyone_reads
  action read
```

<a id="w901"></a>

## W901 — A value in the shape of a key is written

**When**: Somewhere in a `.gate`, in a string or a comment, there is a value in a shape its provider fixes for a key: an AWS access key ID, a key or token of GitHub, Slack, Stripe, OpenAI, Anthropic or Google, a Slack incoming webhook URL, a PEM private key. The diagnostic gives the kind of key, its prefix and its length, and never the key nor its line.

**Fix**: Keep the key where the code runs (an environment variable, the platform's connection or secret store) and read it from there. If it is real, revoke it with its provider first: taking it out of the file leaves it in the history of the repository. If it is a value for tests, write `ritsu: test secret` in a comment on the same line.

**Example**:

```gate
gate maps v1
# the maps API key of the staging site: AIzaSyD-ritsu-fake-key-for-tests-000000
```

<a id="w910"></a>

## W910 — A policy reads a value the contract marks secret

**When**: An `input` a policy reads is a parameter or a field the contract of the operation marks secret (`x-data-classification`, `x-sensitive-data` or `format: password` on an OpenAPI schema, `debug_redact` in a `.proto`). An input a policy reads goes into the request Cedar is asked, and stays in the record of the decision (the answer the generated code gives, Verified Permissions' logs). An input only a computed value reads does not go to Cedar, and is not told.

**Fix**: Give the policies what they need of it as a value a rule or a date computes (`context`), or take it out of the policies' conditions.

**Example**:

```gate
gate loans v1

use openapi loans from "loans.json"

role officer

principal User
  roles officer

resource Loan

action open_loan
  guards loans openLoan
  principal User
  resource Loan
  input
    credit_score : number  range >=0 <=999

permit officers_open_loans_for_good_scores
  principal in officer
  action open_loan
  when credit_score >= 600
```

See also: [W901](#w901)
