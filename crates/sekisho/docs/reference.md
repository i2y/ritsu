# The sekisho reference

sekisho writes who may do what. A `.gate` file declares the principals, the roles, the resources
and the actions of one part of a service, and the permits and forbids that decide each request, with
conditions on roles, attributes and relations, on the answers of rulec's rules and on koyomi's
dates. `sekisho check` walks every combination the file declares and decides each as Cedar does;
`sekisho gen` writes the Cedar schema and policies, and the code that builds each action's request
and asks Cedar. Cedar, not sekisho, decides the requests of a running service.

This page is the whole language and the commands. [codes.md](codes.md) has every diagnostic, with
the smallest file that prints it.

## Files

A `.gate` is UTF-8 text. `#` starts a comment that runs to the end of the line, outside a string.
The lines of a block are indented with spaces under the line that opens it, and line up (E004; a
tab is E004 too).

The parts of a file come in this order (E003 otherwise):

| Part | What it is |
|---|---|
| `gate <name> v<N>` | the heading: the file's name and version |
| `description "…"` | what the file is for, once |
| `namespace <Name>` | Cedar's namespace (`Shop`, or `Shop::Refunds`), in ASCII, once; without it, the file's alias in Pascal case |
| `use …` | the files the gate reads, any number |
| `today range … offset …` | the days a request may come on, once |
| `enum` | enums |
| `role` | roles |
| `principal`, `workflow` | who asks |
| `resource` | what is asked about |
| `action` | what is asked to be done |
| `permit`, `forbid` | the policies, in any order between them |
| `expect` | what a person expects of the answers |
| `separate` | duties kept apart |

### Names and aliases

A name may be written in any language, and Cedar takes ASCII: a name that is not an ASCII
identifier carries an alias in parentheses, and the alias is what goes into Cedar (E007 without
one). A reference may use either. The words a condition, a computed value or a type is made of
cannot be a name or an alias (E002): `and`, `or`, `not`, `in`, `is`, `principal`, `resource`,
`workflow`, `true`, `false`, `any`, `today`, `bool`, `date`, `number`, `rate`. Every other keyword
stands at the start of a line or where nothing else can, so a name may be spelled like one: an
enum's value `open`, an attribute `description`. An alias that is a reserved word of Cedar or of the
generated code, and a type named like one of Cedar's own types (or `Role` and `Workflow`, which
sekisho declares), are E008. Two declarations of one name or alias among things of one kind are
E006.

## The heading

```gate
gate refunds v1
description "Who may look at an order of the shop, refund it, and export the record of refunds. A sketch of a shop's own terms, written as sekisho's example"
namespace Shop
```

`v1` is for people: it goes into the head of what `gen` writes and into `api`.

## Reading other files

```gate
use rule refund_limit from "rules/refund_limit.rule"
use dates refund_terms from "dates/refund_terms.cal"
use calendar uk from "calendars/england_and_wales.cal"
use openapi orders from "api/orders.json"
```

| Line | What it reads | What the gate does with it |
|---|---|---|
| `use rule <name> from "<file.rule>"` | a rule of rulec | a computed value from one of its outputs |
| `use dates <name> from "<file.cal>"` | koyomi's dates | a computed value that compares today with a date |
| `use calendar <name> from "<file.cal>"` | a koyomi calendar | `today is open in <name>` |
| `use openapi <name> from "<file>"` | an OpenAPI 3.0, 3.1 or 3.2 document, in JSON or YAML | the operations its actions guard |
| `use proto <name> from "<file.proto>"` | the services of a `.proto` | the methods its actions guard |
| `use asyncapi <name> from "<file>"` | an AsyncAPI 3.0 or 3.1 document | the operations its actions guard |
| `use book <name> from "<file.book>"` | a chobo book | the operations of the transfers its actions guard |
| `use gate "<file.gate>"` | another gate | its enums, roles, principals, workflows and resources, and its forbids that name `action any` |

A path is written from the directory of the `.gate`. A rule, a dates file, a calendar or a gate
read that does not pass its own check is E201, with what its check said in the notes; a flow that
does not pass dandori's is E208. The binary of sekisho's own crate holds no other language: a file
that reads a rule, a dates file, a calendar, a book or a flow stops there with E209, which says to
run the same command as `ritsu sekisho`, where every language is joined.

The gates a file reads with `use gate` share its namespace (E210), and no two of them declare one
name (E211); reading in a circle is E201. A permit of a gate read has no effect on the file that
reads it; a forbid that names `action any` holds for its actions too, and `gen` writes it among the
file's own policies, as `<file's alias>/<alias of the gate read>/<policy's alias>`.

```gate
gate papers v1
description "The papers of the lab: the forbid people.gate holds for every action holds for this file's, under the id papers/people/suspended_do_nothing"
namespace Lab

use gate "people.gate"
```

### The root

What a gate names of another file is written from the root of the project, so that sekisho, yuen,
sakai and ritsu name one thing alike: the operations an action guards (`openapi "api/orders.json"
operation refundOrder`), the rules and the dates a computed value reads, the flow of a workflow. The
root is `--root`, else the nearest directory above the first file given that holds `.git`, else the
directory of that file; `ritsu check` gives the project's root. A file read from outside the root
is E201 at its `use` line, and a flow outside it E208.

## Today

```gate
today range >=2026-10-01 <=2028-10-31 offset +00:00
```

`today` is the day of the request, from the clock of the server that asks the gate, the day
changing at the offset given. A time zone, whose offset changes with summer time, is E107: choose
when the day changes, and write it where a reader sees it. The check walks every day of the range,
and the generated code refuses a request on a day outside it for an action whose computed values
read `today`; an action that reads no date answers on any day. A file that reads `today` and has no
`today` line is E107 too.

## Enums

```gate
enum order_status = paid | shipped | returned | refunded
```

## Roles

```gate
role manager
  description "Refunds what a clerk cannot, and after the refund period on a business day"
  includes clerk
  can view_order, refund_order
```

A role is an entity of Cedar's type `Role`. `includes clerk` says that whoever holds `manager`
holds `clerk` too: `clerk` is the parent of `manager` in Cedar, written by the generated code, never
read from the service's data. Roles that include each other in a circle are E108. `can` lists the
actions that someone who holds this role alone (with the roles it includes) is allowed in some
combination: allowed an action `can` does not list is E306, and never allowed one it lists is W302.

## Principals and workflows

```gate
principal User
  description "A member of the shop's staff, signed in"
  roles clerk, manager, auditor
  attributes
    refund_limit : money[GBP, incl_tax]  range >=0GBP <=10_000GBP
    suspended    : bool

principal Customer
  description "A customer of the shop, signed in"

workflow returns from "flows/returns.flow"
  description "Refunds a returned order once the item is back at the warehouse"
```

A principal is an entity type of Cedar: a person signed in, or a service calling under its own
credentials. `roles` lists the roles one may hold, which the service's data gives; the check counts
every set of them, none and several. A workflow is an entity of Cedar's type `Workflow`, named as
the gate names it, calling under its own credentials; its `.flow` has to pass dandori's check
(E208). `ritsu check` holds what the flow calls to what the gate allows the workflow (X16, below).

## Resources

```gate
resource Order
  description "An order of the shop"
  attributes
    status   : order_status
    paid_on  : date  range >=2026-01-01 <=2028-09-30
    customer : Customer
```

### Attributes

Every attribute has finitely many values to count:

| Type | In Cedar | What the check counts |
|---|---|---|
| `bool` | `Bool` | true and false |
| an enum | `String`, the value's alias | every value |
| a number: one of rulec's unit types (`money[GBP, incl_tax]`, `mass[kg]`, …), `number` or `rate`, with a `range` | `Long`, in whole units of the type | the stretches the constants the policies compare it with cut its range into |
| `date`, with a `range` | none | what the computed values that read it can be |
| an entity type (`customer : Customer`) | that type | which entities of one type are the same entity (relations) |
| `T?` | an optional attribute | its absence, as one more value |

There are no free strings: there would be no end of values to count. A number or a date needs both
ends of its range (E103). Only what the generated policies read goes into Cedar's schema and
entities: an attribute only a computed value reads (`refund_limit`, `paid_on`) stays out of Cedar.

## Actions

```gate
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
```

- `guards <document> <operation>`: an operation of a contract the action guards. In an OpenAPI
  document, its `operationId`, or its method and path as a string (`"POST /orders/{orderId}/refunds"`);
  in a `.proto`, `"Service/Method"`; in an AsyncAPI document, the operation's key; in a book, a
  transfer's operation as `<transfer>.<operation>`. An action may guard several operations (the
  same one over REST and gRPC); an operation the document does not have is E202, and one guarded
  by two actions E205. An action with no `guards` guards no operation a contract writes, and its
  page says so.
- `principal` and `resource` list the types a request of the action may bring: Cedar's
  `appliesTo`. A policy whose `principal` line picks no principal the actions it names take is
  E106.
- `from orderId`: the operation's parameter the resource's id is read from (E204 if the operation
  has none of that name). Without it, the generated code takes the id as its own argument.
- `nobody "<why>"`, after the `resource` line, says that no one is meant to be allowed the action;
  without it, an action no permit allows is E301, and with it, an action some combination allows
  is E304.
- `input`: the arguments of the operation that a policy or a computed value reads, under the names
  the operation gives them, with types and ranges the operation accepts (E203).
- `context`: the computed values, each an attribute of Cedar's context of the action.

### Computed values

A computed value is an answer of another language. The generated code computes it from the
service's data, the input and the clock, and puts it in the request; a caller cannot hand one in.

| Form | What it is |
|---|---|
| `<rule>(<input>: <value>, …).<output>` | an output of a rule of rulec, an enum or a bool (E105 for another) |
| `today <op> <dates>.<date>(<input>: <value>, …)` | whether today is before, on or after a date koyomi computes; `<op>` is `<`, `<=`, `>`, `>=` or `is` |
| `today <op> resource.<attribute>` | the same, with a date of the principal or the resource |
| `today is open in <calendar>` | whether today is a business day of a koyomi calendar |

A value given to a rule or a date is an attribute of the principal or the resource, an input, a
constant or `today`; another computed value cannot be (E105). Its range has to fit within what the
rule or the date takes, and a precondition of the rule (its `constraint`) has to hold over it
(E206; W201 when rulec cannot decide). A calendar has to have data for every day of `today`'s range
and every day the dates can come to (E207). What a rule's output can be (over the stretches the
policies cut its number inputs into, at each value of an enum or a bool the policies read too or
the gate writes as a constant, and over only the values of an enum of the gate's that has fewer
than the rule's), and which answers of the dates can come together on one day, sekisho asks rulec
and koyomi, so that the check counts what can happen and nothing else.

## Policies

```gate
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
```

The meaning is Cedar's: a request no policy applies to is denied, and a forbid that applies wins
over every permit. Every policy has a name, and its alias makes its `@id`:
`<file's alias>/<policy's alias>`, which stays the same when other policies come and go.

- `principal in <role>, …` (any of the roles, through `includes`), `principal is <type>`, or
  `principal is workflow <name>`; without the line, every principal of the actions.
- `action <action>, …`, or `action any`, every action of the file.
- `when` and `unless`, any number: the policy applies when every `when` holds and no `unless`
  does. One line can join conditions with `and`, `or`, `not` and parentheses.

### Conditions

| Condition | What it says | In Cedar |
|---|---|---|
| `principal in clerk` | holds the role | `principal in Shop::Role::"clerk"` |
| `principal is Customer` | the principal's type | `principal is Shop::Customer` |
| `principal is workflow returns` | that workflow | `principal == Shop::Workflow::"returns"` |
| `resource.status is returned` | an enum's value | `resource.status == "returned"` |
| `principal.suspended` | a bool | `principal.suspended` |
| `amount <= 50GBP` | a number against a constant: `<`, `<=`, `>`, `>=`, `is` | `context.amount <= 50` |
| `resource.customer is principal` | the attribute is the principal | `resource.customer == principal` |
| `resource.tenant is principal.tenant` | two attributes are one entity | `resource.tenant == principal.tenant` |
| `principal in resource.team` | the principal is a member of it | `principal in resource.team` |
| `refund_band is within_limit` | a computed value's value | `context has refund_band && context.refund_band == "within_limit"` |
| `in_period` | a computed bool | `context.in_period` |

`x is not v` reads as `not (x is v)`, and holds when `x` has no value. A relation goes one step: an
attribute of an attribute (`resource.project.owner`) is E104. A computed value that a principal
type of the action cannot have (it reads an attribute the type does not have) is optional in the
context, and the generated condition asks `has` first, as above.

## Expectations and separations

```gate
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

`expect allow` and `expect deny` pick combinations as a policy picks requests, and say how every
one of them is answered: when one is answered otherwise, E304 says how many and shows one; when
none is picked, the expectation holds and checks nothing, which is W304. `separate` says that no
principal (one type, one set of roles, one value of each attribute) is allowed two of the actions,
on any resource and context each (E305).

## What `check` says

`sekisho check` reads the file, its names and its types, asks the other languages what it reads of
them, then walks each action's combinations: the principal's type, the set of roles, every value of
the bool and enum attributes, the stretches of the numbers, the relations, the outputs a rule can
give and the answers of the dates that can come together. It decides each as Cedar does. In the
example, that is 1,078 combinations over three actions. Over them it says:

| Code | What it finds |
|---|---|
| E301 | no permit allows the action in any combination: no one can do it |
| E302 | a permit allows nothing: a forbid denies every combination it would allow |
| E303 | the conditions of a policy never hold together |
| W301 | a permit adds nothing: another permit alone allows every combination it allows |
| E304 | an expectation does not hold, with a combination that breaks it |
| W304 | an expectation picks no combination |
| E305 | a separation does not hold: a principal is allowed two of its actions |
| E306 | a role is allowed an action its `can` does not list |
| W302 | a role is never allowed an action its `can` lists |
| W303 | a combination where an answer of rulec or koyomi could not be decided, with why |
| E307 | an action has more combinations than `--budget` (100,000,000 by default): it is not checked, and nothing is generated |
| W910 | an input a policy reads that the contract of the operation marks secret: its value goes into the request Cedar is asked, and stays in the record of the decision |
| W901 | a key of a provider, written in the file |

```console
$ sekisho check examples/refunds/refunds.gate --root examples/refunds
examples/refunds/refunds.gate: ok — 3 actions, 10 policies (7 permits, 3 forbids), 3 expectations, 1 separation
```

With the two `unless` lines taken out of the expectation that a manager refunds in the period:

```text
error[E304]: tests/mutants/E304_expect_fails.gate:147:1: The expectation `managers_refund_in_period` does not hold: 144 of the 192 combinations it picks are denied
   147 | expect allow managers_refund_in_period
  = For example: User holding manager (suspended: yes), Order (status: paid), amount: 1GBP to 50GBP, refund_band: within_limit, in_period: yes, business_day: no; denied by `suspended_staff_do_nothing`.
  = Fix the policies, or the expectation if it says more than is meant.
```

Every check comes to one of three answers: it holds, here is a combination where it does not, or it
cannot be decided, and then it says why. A rule or a date whose answers rulec or koyomi cannot
decide over a stretch is counted with every value its output has, and what holds over those
combinations still holds; a finding that rests on such a combination alone is tried with real
inputs through the rule's evaluator, and when none brings it about it is W303.

## What `gen` writes

`sekisho gen <file.gate> --target cedar` writes four files under `<out>/cedar/` (`generated` by
default), named by the gate's alias:

| File | What it is |
|---|---|
| `<alias>.cedar` | the policies, each with its `@id` and its `description` as `@doc` |
| `<alias>.cedarschema` | the schema: the entity types, the attributes the policies read, the actions with their `appliesTo` and context |
| `<alias>.cedarschema.json` | the schema in Cedar's JSON form |
| `<alias>.policies.json` | the policies in Cedar's JSON form, keyed by `@id` |

```cedar
@id("refunds/clerks_refund_within_their_limit")
@doc("A clerk refunds up to the clerk's own limit, while the refund period lasts")
permit (
  principal in Shop::Role::"clerk",
  action == Shop::Action::"refund_order",
  resource is Shop::Order
)
when { context has refund_band && context.refund_band == "within_limit" }
when { context.in_period };
```

The schema says, beside each computed value, what it is computed from, and that the caller never
gives it; beside each action, the operation it guards (`@guards`), named from the root:

```cedarschema
  @doc("Refund an order, in part or in whole")
  @guards("openapi \"api/orders.json\" operation refundOrder")
  action "refund_order" appliesTo {
    principal: [User, Workflow],
    resource: [Order],
    context: {
      @doc("An argument of the operation: money[GBP, incl_tax], 1 to 10000")
      amount: Long,
```

A name of the gate written in Japanese is kept as `@name` beside its alias. What sekisho writes in
its own words (the head, the `@doc` of a computed value, an input, a role or a workflow) is in the
language of `--lang`; a `description` goes in as it is.

`--target typescript`, `python` and `go` write, for each action, a function that builds its
request from the service's data and asks Cedar:

| Target | The file | Cedar it asks |
|---|---|---|
| `typescript` | `<out>/typescript/authz/<alias>.ts` | `@cedar-policy/cedar-wasm` 4.13.0, or Verified Permissions through `@aws-sdk/client-verifiedpermissions` |
| `python` | `<out>/python/authz/<alias>.py` | `cedarpy` 4.12.1, or Verified Permissions through `boto3` |
| `go` | `<out>/go/authz/<package>/<package>.go` | `github.com/cedar-policy/cedar-go` v1.8.0, or Verified Permissions through `aws-sdk-go-v2` |

A function takes the service's `Store` (each type's attributes by id), the principal as the
authentication decided it, the input as the handler parsed it, and the time. It reads the principal
and the resource from the store, computes the computed values with the code rulec and koyomi
generate (the package's `rules/` and `dates/`, as `ritsu gen` lays them out), and asks Cedar. It
refuses without asking when what it reads is not what the gate declared, and says which: the
`principal`, the `resource` or the `input` is not there or outside its range or its enum, `today`
is outside the range the gate was checked over, or the code of a `rule` or a `date` failed; `cedar`
when Cedar met an error or could not be asked. `--authorizer avp` asks Amazon Verified Permissions
instead, and says W401 of what is over its quotas. `ritsu gen` writes the same into the packages of
a project, with the Cedar once in `<out>/cedar/`.

## Across the languages

`ritsu check` reads the gates of a project with the other languages, and adds two checks across
them (ritsu's codes, which `ritsu explain` looks up):

- **X15**: every operation a context of the map opens (sakai's `open host service`) is guarded by
  an action of a gate, unless its contract says anyone may call it (`security: []`). E907 for an
  operation no action guards in a context where others are guarded, W907 for a context none of
  whose operations is guarded yet. It looks only when the project holds a gate.
- **X16**: a workflow is allowed the operations its flow calls, and calls what it is allowed. E908
  for a call its gate allows in no combination; W909 for a call that can be denied, or where
  whether it is allowed cannot be decided, while the task declares no error for a denial (status
  403, or Connect's `permission_denied`); W908 for an action the workflow is allowed and never
  calls. A flow's calls of a book's transfers (dandori's `book` tasks) are not among them: the code
  dandori writes calls chobo's client without asking a gate, and chobo refuses for no reason that
  stands for a denial. An action that guards a transfer's operation holds where the service that
  keeps the book asks the gate before it moves the book.

A file of Cedar written by hand, with its schema, takes part in the same checks when the map
(`owns`) or a requirement names it, and its schema says with `@guards` which operation an action
guards.

## The keywords

| Where | Words |
|---|---|
| at the start of a line | `gate` `description` `namespace` `use` `today` `enum` `role` `principal` `workflow` `resource` `action` `permit` `forbid` `expect` `separate` |
| after `use` | `rule` `dates` `calendar` `openapi` `proto` `asyncapi` `book` `gate` `from` |
| in a block | `description` `includes` `can` `roles` `attributes` `guards` `principal` `resource` `from` `nobody` `input` `context` `action` `actions` `when` `unless` |
| in a condition | `in` `is` `not` `and` `or` `any` `workflow` `open` `today` `allow` `deny` `principal` `resource` `true` `false` |
| types and ranges | `bool` `date` `number` `rate` `money` `mass` `length` `area` `volume` `duration` `temperature` `sound` `range` `offset` |

The symbols are `=` `:` `,` `.` `(` `)` `<` `<=` `>` `>=` `|` `?`, and `#` for a comment. Cedar's
`==`, `!=`, `&&`, `||` and `!` are not sekisho's: E001 says what to write instead (`is`, `is not`,
`and`, `or`, `not`).

## Commands

```console
$ sekisho check examples/refunds/refunds.gate
$ sekisho gen examples/refunds/refunds.gate --target cedar
$ sekisho doc examples/refunds/refunds.gate --root examples/refunds --format html
$ sekisho vectors examples/refunds/refunds.gate --action refund_order
$ sekisho api examples/refunds/refunds.gate --root examples/refunds
$ sekisho explain E302
```

- `check <file.gate>... [--format json] [--budget <n>] [--root <dir>]`: the checks above. `--budget`
  is the most combinations one action may have.
- `gen <file.gate>... --target cedar|typescript|python|go [--authorizer cedar|avp] [--module <path>]
  [--out <dir>] [--check] [--root <dir>]`: what is above. `--module` is the Go module the code of
  `go` imports the rules' and the dates' packages from (`generated` by default, as `ritsu gen`).
  `--check` writes nothing and exits 1 when a file on disk differs from what `gen` would write.
  Nothing is written from a file that does not pass check.
- `doc <file.gate> [--format markdown|html] [--root <dir>]`: the page for people: each action's
  table of the combinations allowed and denied, the policies beside the Cedar they compile to, the
  computed values with the pages rulec and koyomi draw of their rules and dates, the expectations,
  the separations and the roles, the workflows and what their flows call, and the operations
  guarded. The Markdown is for a pull request; the HTML is one file that reads nothing from outside.
- `vectors <file.gate> [--action <action>] [--root <dir>]`: every combination as a test of `cedar
  run-tests`: the request, the entities, and the decision and the deciding policies by sekisho's
  reference evaluation, at both ends of each stretch of a number.
- `api <file.gate> [--root <dir>]`: JSON for other tools (below).
- `explain <code> | --all [--format markdown|json]`: when a diagnostic is printed, how to fix it,
  and the smallest file that prints it.

`doc`, `vectors` and `api` take one file, which has to pass check; else they print its diagnostics
and exit 1. Every command takes `--lang ja|en` (else `SEKISHO_LANG`, then `RITSU_LANG`, else
English) and `--help`.

### Exit codes

| Code | When |
|---|---|
| 0 | no errors (there may be warnings); generated; printed |
| 1 | at least one error; `gen --check` found a file that differs or is missing |
| 2 | bad arguments, a file that cannot be read or written, or a file that reads another language run with sekisho's own binary (E209) |

## JSON

`check --format json` prints one object a file:

```json
{
  "file": "examples/refunds/refunds.gate",
  "ok": true,
  "summary": "3 actions, 10 policies (7 permits, 3 forbids), 3 expectations, 1 separation",
  "diagnostics": []
}
```

`api` prints what the gate declares, each with its line: `sekisho` (the version), `name`, `alias`,
`version`, `source_sha256`, `description`, `namespace`, `uses`, `today`, `cedar` (the four files
`gen` writes), `roles` (each role's entity in Cedar, `includes`, `can`), `types` (each type's
entity type, its roles, its attributes and whether each goes into Cedar), `workflows`, `actions`
(the operations each guards, as references written from the root, the types it takes, its input,
its context and how each computed value is computed), `policies` (each `@id`), `expects` and
`separations`. A reference is an object of `text`, `tool`, `path` and `items`, as sakai's and
yuen's are.

`vectors` prints a JSON array, a test of `cedar run-tests` a combination: `name`, `request`,
`entities`, `decision`, `reason` (the deciding policies' `@id`) and `num_errors`.

## Environment

| Variable | What it does |
|---|---|
| `SEKISHO_LANG` | `ja` or `en`, the language of the text when `--lang` is not given |
| `RITSU_LANG` | the same, for every language of ritsu, after `SEKISHO_LANG` |
