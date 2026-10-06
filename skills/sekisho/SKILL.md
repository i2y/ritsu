---
name: sekisho
description: Write, check and fix sekisho files (`.gate`), the authorization of a service written as who may do what — principals, roles, attributes and relations, the actions that guard the operations of an OpenAPI, AsyncAPI or `.proto` contract, and the permits, forbids, expectations and separations of duties that decide them, with the answers of rulec's rules and koyomi's dates as conditions — checked on every combination the file declares and compiled to Cedar. Use when who may call which operation has to be written down or changed; when Cedar policies have to be generated, or the code that builds a request and asks Cedar (TypeScript, Python, Go, or Amazon Verified Permissions); when a sekisho diagnostic (E001-E307, W201-W910) or ritsu's E907-W909 has to be fixed; or when the table of who may do what has to be shown to the people who decide it.
compatibility: Requires the `ritsu` binary on PATH (`cargo install --git https://github.com/i2y/ritsu --locked ritsu`); run sekisho as `ritsu sekisho <command>`, or as `sekisho <command>` through a link to ritsu named for it. Cedar itself decides the requests of the running service, through the code sekisho generates.
license: MIT OR Apache-2.0
---

## When this applies

The job is **the authorization of a service, written where it can be checked**: which principal
(a person signed in, a service, a workflow) may do which action on which resource, under which
conditions, and which operation of the service's contract each action guards. sekisho writes it as
a `.gate`, checks every combination of roles, attributes, inputs and the answers of the rules and
dates it reads, and generates Cedar and the code that asks Cedar.

It does not apply to authentication (who the caller is: the service's own login, tokens, mTLS),
to the balance of an account (that is a bound of chobo's book, kept in the write itself), or to a
relation that goes more than one step (`resource.project.owner`): sekisho writes none of those.

The files bundled with this skill are `reference.md` (the whole language and the commands) and
`codes.md` (every diagnostic). Read them when you need them, not all up front.

---

# Working with sekisho

Your part is to write the gate, get it past `ritsu sekisho check`, generate what the service runs,
and show the page to the people who decide who may do what. Three things stay with people:

1. **Who may do what.** The roles, the permits and the forbids are a decision of the business and
   of security. Never add a permit, widen a role or drop a forbid to make the check pass.
2. **What is expected to hold.** An `expect` and a `separate` say what a person has decided must
   never or always happen; when one does not hold, the policies are wrong or the expectation is,
   and only a person can say which.
3. **That no one is meant to do an action** (`nobody "<why>"`).

## 1. The loop

1. Read what is there: the contract of the service (an OpenAPI or AsyncAPI document, a `.proto`),
   the rules and dates the conditions will read, the workflows that call the service.
2. Write the gate: the heading, the `use` lines, `today` if a date is read, the enums, the roles,
   the principals and workflows, the resources, the actions with what they guard, then the
   policies, the expectations and the separations.
3. Run the check and fix what it says, one diagnostic at a time, from the top; `ritsu sekisho
   explain <code>` says how. Then `ritsu check` on the project, which adds the checks across the
   languages (X15, X16).
4. Generate the Cedar and the code that asks it, into the service's package (`ritsu gen`, or
   `ritsu sekisho gen --target …`), and have CI run `gen --check`.
5. Write the page with `ritsu sekisho doc`, and show it to the people who decide.

```console
$ ritsu sekisho check examples/refunds/refunds.gate --root examples/refunds
$ ritsu sekisho explain E302
$ ritsu sekisho gen examples/refunds/refunds.gate --target cedar --root examples/refunds
$ ritsu sekisho doc examples/refunds/refunds.gate --root examples/refunds --format html
```

Run every command as `ritsu sekisho <command>` (through a link to ritsu named sekisho, `sekisho
<command>` is the same): it reads the rules, the dates, the calendars and the flows a gate names in
the same process, where sekisho built alone from its crate reads no other language and stops with
E209. `--root` is the directory the paths of what the gate names are written from (else the nearest
directory above that holds `.git`). Add `--lang ja` for Japanese.

## 2. The language on one page

```gate
gate refunds v1
description "Who may look at an order of the shop, refund it, and export the record of refunds. A sketch of a shop's own terms, written as sekisho's example"
namespace Shop

use rule refund_limit from "rules/refund_limit.rule"
use dates refund_terms from "dates/refund_terms.cal"
use calendar uk from "calendars/england_and_wales.cal"
use openapi orders from "api/orders.json"

today range >=2026-10-01 <=2028-10-31 offset +00:00

enum order_status = paid | shipped | returned | refunded

role manager
  description "Refunds what a clerk cannot, and after the refund period on a business day"
  includes clerk
  can view_order, refund_order

principal User
  description "A member of the shop's staff, signed in"
  roles clerk, manager, auditor
  attributes
    refund_limit : money[GBP, incl_tax]  range >=0GBP <=10_000GBP
    suspended    : bool

workflow returns from "flows/returns.flow"
  description "Refunds a returned order once the item is back at the warehouse"

resource Order
  description "An order of the shop"
  attributes
    status   : order_status
    paid_on  : date  range >=2026-01-01 <=2028-09-30
    customer : Customer

action refund_order
  description "Refund an order, in part or in whole"
  guards orders refundOrder
  principal User, Workflow
  resource Order from orderId
  input
    amount : money[GBP, incl_tax]  range >=1GBP <=10_000GBP
  context
    refund_band  = refund_limit(amount: amount, limit: principal.refund_limit).band
    in_period    = today <= refund_terms.last_day(paid_on: resource.paid_on)
    business_day = today is open in uk

permit clerks_refund_within_their_limit
  description "A clerk refunds up to the clerk's own limit, while the refund period lasts"
  principal in clerk
  action refund_order
  when refund_band is within_limit
  when in_period

forbid suspended_staff_do_nothing
  description "A suspended member of the staff does nothing"
  principal is User
  action any
  when principal.suspended

expect deny clerks_never_refund_over_their_limit
  description "A clerk who is not a manager never refunds more than the clerk's own limit"
  principal in clerk
  action refund_order
  unless principal in manager
  when refund_band is over_limit

separate refunding_and_auditing
  description "The people who refund are not the people who audit the refunds"
  actions refund_order, export_refunds
```

- The parts come in this order (E003), and the lines of a block are indented with spaces.
- Cedar's meaning: what no policy allows is denied, and a forbid that applies wins over every
  permit. `principal in <role>` follows `includes`.
- A name that is not ASCII carries an alias for Cedar: `返金する(refund_order)`.
- Every attribute counts finitely: `bool`, an enum, a number of a unit type with both ends of its
  `range`, a `date` with a range, an entity type, `T?`. There are no free strings.
- `guards` names an operation of a document a `use` line reads: an `operationId` (or `"POST
  /orders/{orderId}/refunds"`), `"Service/Method"` of a `.proto`, an AsyncAPI operation's key, a
  chobo transfer's `<transfer>.<operation>`.
- A computed value is the output of a rule (`<rule>(…).<output>`, an enum or a bool), `today`
  against a date of koyomi, or `today is open in <calendar>`. It is given only attributes, inputs,
  constants and `today`; the generated code computes it, and a caller cannot hand it in.
- Conditions: `principal in`, `principal is`, `principal is workflow`, `x is v`, `x is not v`, a
  bool, a number against a constant, `resource.a is principal`, `resource.a is principal.b`,
  `principal in resource.a`, joined with `and`, `or`, `not` and parentheses.
- A policy's alias is its `@id`, `<file's alias>/<policy's alias>`: renaming a policy changes the
  `@id` that the records of decisions and Verified Permissions hold.

## 3. What to ask a person

- Who may do each action, and on which conditions, before writing a permit.
- What the operations of the contract mean, when an action's name or what it guards is not plain
  from the document.
- What an expectation should say, when E304 shows a combination where it does not hold: whether
  the policies or the expectation are wrong.
- Whether a role's reach (`can`) is wrong or the policies are, at E306 and W302.
- Whether two duties must be kept apart (`separate`), and what to do at E305.
- Whether an operation of a context that no action guards is open to anyone on purpose (then its
  contract says `security: []`), at ritsu's E907 and W907.
- What a workflow should do when it is denied, at ritsu's W909: the task declares the error
  (`errors denied = 403`), and the flow handles it.

## 4. From a diagnostic to a fix

`ritsu sekisho explain <code>` (and `codes.md`) gives each code's cause, its fix and its smallest
reproduction. The usual ones:

| Code | What it means | What to do |
|---|---|---|
| E001-E005 | a word, a line or the indentation the language does not read | fix the line; E001 says what sekisho writes for Cedar's `==`, `&&` and the rest |
| E101 | a name names nothing | fix the spelling, or declare it |
| E103 | a number or a date with no range, or a constant outside it | give both ends of the range |
| E105 | a computed value from what it cannot be computed from | give a rule only attributes, inputs, constants and `today`; read an enum or bool output |
| E201 | a file a `use` reads does not pass its check, cannot be read, or is outside the root | fix that file with its language, or `--root` |
| E202-E205 | an operation the contract does not have, an input it does not take, a `from` it has no parameter for, an operation two actions guard | follow the contract; ask which action decides an operation |
| E206, W201 | a value given to a rule or a date outside what it takes, or breaking its precondition | narrow the range, or ask whether the rule should take more |
| E207 | a calendar without data for every day `today` can be | narrow `today`'s range, or bring the calendar's data |
| E208 | a workflow's flow does not pass dandori's check | fix the flow with dandori |
| E209 | the command was run without the other languages | run it as `ritsu sekisho` |
| E301 | no permit allows the action | ask who may do it; if no one, `nobody "<why>"` after the `resource` line |
| E302 | a permit a forbid covers entirely | narrow the forbid, or ask whether the permit should go |
| E303 | a policy whose conditions never hold together | fix the conditions |
| E304, W304 | an expectation that does not hold, or picks nothing | ask whether the policies or the expectation are wrong |
| E305 | a principal is allowed two duties a `separate` keeps apart | ask; usually a forbid on one of them for the holders of the other |
| E306, W302 | a role's reach is not its `can` | ask whether `can` or the policies are right |
| E307 | too many combinations | split the roles, or narrow the ranges; never raise `--budget` without saying so |
| W301 | a permit another one already covers | ask whether it can go |
| W303 | an answer no concrete input could be found for | read why; a rule whose output cannot be told exactly may need its table made plain |
| W910 | a policy reads an input the contract marks secret | ask whether the policy needs it; the value goes to Cedar and into the record of the decision |
| W901 | a key written in the file | take it out and keep it where the service runs; tell the person to revoke it if it is real |

`ritsu check` adds ritsu's codes across the languages, which `ritsu explain <code>` looks up:

| Code | What it means | What to do |
|---|---|---|
| E907, W907 | an operation a context opens that no action guards; a context none of whose operations is guarded yet | guard it with an action, or say in its contract that anyone may call it (`security: []`), after asking |
| E908 | a workflow calls an operation its gate never allows it | ask whether the gate or the flow is wrong |
| W908 | a workflow is allowed an action whose operation it never calls | narrow the permit to what the flow calls |
| W909 | a workflow's call can be denied, and its task declares no error for it | declare the error on the task (`errors denied = 403`) and handle it in the flow |

Fix the first diagnostic first: an error of words or names stops the checks after it.

## 5. What it generates, and the page

`ritsu sekisho gen <file> --target cedar` writes the schema and the policies, each policy with its
`@id` and its description, the schema with what each computed value is computed from and the
operation each action guards. `--target typescript|python|go` writes, for each action, a function
that reads the principal and the resource from the service's `Store`, computes the computed values
with the code rulec and koyomi generate, and asks Cedar (cedar-wasm, cedarpy, cedar-go), or
Amazon Verified Permissions with `--authorizer avp`. It refuses before asking Cedar when what it
reads is not what the gate declared. `ritsu gen` puts the same into the packages of a project,
beside the rules and dates the code reads. The handler calls the function with the operation's
arguments as it parsed them, and performs the operation with those same arguments.

`ritsu sekisho doc` writes the page for people: each action's table of who is allowed and who is
denied, the policies beside the Cedar they become, the pages of the rules and dates the conditions
read, the expectations, the separations and the roles, the workflows and what their flows call.
The Markdown is for a pull request; the HTML is one file that reads nothing from outside. Show it
to the people who decide, and change the gate, not the page.
