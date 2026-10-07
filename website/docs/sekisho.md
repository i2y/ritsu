---
hide:
  - toc
---

# sekisho

**Who may do what, with the answer of a business rule as a condition. sekisho checks a gate on
every combination it declares, then compiles it to [Cedar](https://www.cedarpolicy.com/), and
Cedar decides each request.**

A clerk of a shop may refund an order up to the clerk's own limit. In a gate (a `.gate` file),
whether an amount is within that limit is a rule's answer, and a permit reads it like a role:

```gate
use rule refund_limit from "rules/refund_limit.rule"
…
action refund_order
  …
  input
    amount : money[GBP, incl_tax]  range >=1GBP <=10_000GBP
  context
    refund_band  = refund_limit(amount: amount, limit: principal.refund_limit).band
…
permit clerks_refund_within_their_limit
  description "A clerk refunds up to the clerk's own limit, while the refund period lasts"
  principal in clerk
  action refund_order
  when refund_band is within_limit
  when in_period
```

`refund_band` is computed for every request: the output `band` of the rule `refund_limit`, given
the amount the operation is asked to refund and the limit the service keeps for the member of the
staff who asks. The rule is one of [rulec](rulec/):

```rule
rule refund_limit v1
…
enum refund_band = within_limit | over_limit

inputs
  amount : money[GBP, incl_tax]  range >=1GBP <=10_000GBP
  limit  : money[GBP, incl_tax]  range >=0GBP <=10_000GBP

outputs
  band : refund_band

derive excess : money[GBP, incl_tax] = amount - limit  range >=-9_999GBP <=10_000GBP

table decide
policy unique
| excess | -> band : refund_band |
| <=0GBP | within_limit          |
| >0GBP  | over_limit            |
…
```

`sekisho check` asks rulec which answers the rule can give, and holds the policies to every one of
them. Only a gate that passes compiles: into Cedar, and into code that computes the answer with the
code rulec generates before it asks Cedar.

## Cedar decides

sekisho decides no request. A service asks Cedar, in its own process (cedar-wasm, cedarpy,
cedar-go, the Rust crate) or as Amazon Verified Permissions, and Cedar answers from the policies,
the schema and the request. sekisho writes those, and checks them before it writes them. Cedar is a
standard: small, typed, with its semantics formalised in Lean. What sekisho writes is plain Cedar,
which `cedar validate` passes in strict mode.

What Cedar does not do alone is what sekisho is for:

- **Conditions from rules and dates.** A Cedar policy reads the request and the entities. It
  cannot look a value up in a table, count business days or add a month. rulec and koyomi compute
  those answers, and the request carries them.
- **Every combination, as a table.** Cedar's analysis tells whether two sets of policies allow the
  same requests, or which allows more. sekisho walks every combination its conditions can come to,
  and draws for each action the table of who is allowed what, for the people who decide it.
- **The request.** Cedar believes what a request says. The code sekisho generates builds the
  request itself, and takes nothing the policies read from the caller.
- **The rest of the project.** An action names the operation of the contract it guards, and a
  workflow can be a principal, so `ritsu check` holds the gate to the contracts, the map and the
  workflows.

The Cedar it writes keeps to a small part of the language (no extension types, no arithmetic, no
`like`), so that every implementation answers it alike. Its tests run every combination of the
examples through the official Cedar CLI, and through the three libraries the generated code asks.

## What the check asks the rule

The amount runs from 1 to 10,000 pounds, and the limit from 0 to 10,000; the check does not try
them one by one. It asks rulec what the rule can answer on each stretch of amounts the policies
tell apart. A permit of the gate compares the amount with 50 pounds, so there are two stretches,
and over every limit a member of the staff can have, both bands can come in each. With the roles,
the attributes and the answers of the dates, that makes 1,078 combinations over the gate's three
actions, and the check decides each as Cedar does: a request no policy allows is denied, and a
forbid that applies wins over every permit.

```console
$ cd crates/sekisho/examples/refunds
$ sekisho check refunds.gate
refunds.gate: ok — 3 actions, 10 policies (7 permits, 3 forbids), 3 expectations, 1 separation
```

Beside the policies, a gate writes what a person has decided must hold:

```gate
expect deny clerks_never_refund_over_their_limit
  description "A clerk who is not a manager never refunds more than the clerk's own limit"
  principal in clerk
  action refund_order
  unless principal in manager
  when refund_band is over_limit
```

Take the condition on the rule's answer out of the clerks' permit:

```diff
 permit clerks_refund_within_their_limit
   description "A clerk refunds up to the clerk's own limit, while the refund period lasts"
   principal in clerk
   action refund_order
-  when refund_band is within_limit
   when in_period
```

and `sekisho check` finds clerks refunding over their limit:

```text
warning[W301]: refunds.gate:93:1: Every combination the permit `managers_refund_in_period` allows, `clerks_refund_within_their_limit` allows too
    93 | permit managers_refund_in_period
  = It allows 48 combinations, and `clerks_refund_within_their_limit` allows each of them: removing `managers_refund_in_period` changes no answer.
  = Remove `managers_refund_in_period`, or narrow `clerks_refund_within_their_limit` if `managers_refund_in_period` says what is meant.
error[E304]: refunds.gate:134:1: The expectation `clerks_never_refund_over_their_limit` does not hold: 12 of the 128 combinations it picks are allowed
   134 | expect deny clerks_never_refund_over_their_limit
  = For example: User holding clerk (suspended: no), Order (status: paid), amount: 1GBP to 50GBP, refund_band: over_limit, in_period: yes, business_day: no; allowed by `clerks_refund_within_their_limit`.
  = Fix the policies, or the expectation if it says more than is meant.
```

The example is an amount of 50 pounds or less that the rule answers `over_limit` for, which it does
when the clerk's own limit is lower. The warning comes from the same edit: a manager includes
clerk, so the clerks' permit now allows everything the managers' permit for the refund period does.

Every check comes to one of three answers: it holds, here is a combination where it does not, or it
cannot be decided, and then it says why. The others find an action no permit allows (E301), a
permit that allows nothing because a forbid denies all it would allow (E302), a policy whose
conditions never hold together (E303), a separation of duties that one principal breaks (E305), a
role allowed an action its `can` line does not list (E306) or never allowed one it lists (W302), an
expectation that picks nothing (W304), a value given to a rule outside the range of its input or
breaking its precondition (E206), and a finding that rests on an answer of rulec or koyomi that
cannot be decided (W303). `sekisho explain E304` tells when a code comes, how to fix it, and the
smallest gate that gives it.

## What it generates

### Cedar

```console
$ cd crates/sekisho/examples/refunds
$ sekisho gen refunds.gate --target cedar --out generated
generated: generated/cedar/refunds.cedar
generated: generated/cedar/refunds.cedarschema
generated: generated/cedar/refunds.cedarschema.json
generated: generated/cedar/refunds.policies.json
```

The clerks' permit, in Cedar:

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

To Cedar, the rule's answer is a string in the context of the request. The schema says where it
comes from:

```cedarschema
      @doc("rulec \"crates/sekisho/examples/refunds/rules/refund_limit.rule\" output band, … Computed by the generated code, never taken from the caller")
      refund_band?: String
```

The `?` says it may be absent: a workflow, the other principal of the action, has no limit to give
the rule.

### The code that asks Cedar

If the caller could send `refund_band`, the caller could allow their own refund. So for each
action, the code sekisho generates for TypeScript, Python and Go reads the principal and the order
from the service's own data, computes the band with the code rulec generates and the dates with the
code koyomi generates, builds the request, and asks Cedar. `ritsu gen` writes it into one package
with the code of the rules and the dates it reads:

```console
$ cd crates/sekisho/examples/refunds
$ ritsu gen --target python --out generated
generated: generated/python/generated/rules/refund_limit.py
…
generated: generated/python/generated/dates/refund_terms.py
…
generated: generated/python/generated/authz/refunds.py
…
```

```python
from ..dates import england_and_wales as _dates_england_and_wales
from ..dates import refund_terms as _dates_refund_terms
from ..rules import refund_limit as _rules_refund_limit
…
def refund_order_request(store: Store, principal: Principal, input: RefundOrderInput, now: datetime | None = None) -> Request:
    …
    in_amount = _int('input', 'amount', input.get('amount'), 1, 10000)
    …
        p_user = _read_user(store, principal.id, ('refund_limit', 'suspended',))
    …
    # refund_band  = refund_limit(amount: amount, limit: principal.refund_limit).band
    if p_user is not None:
        try:
            v_refund_band = _rules_refund_limit.refund_limit(_rules_refund_limit.GBPInclTax(in_amount), _rules_refund_limit.GBPInclTax(p_user.refund_limit))
```

The function takes no computed value from its caller. When the data is not what the gate declares,
or today is outside the range the gate was checked over, it refuses before it asks Cedar. The
Python asks cedarpy in the process, the TypeScript cedar-wasm and the Go cedar-go; with
`--authorizer avp`, each asks Amazon Verified Permissions instead.

## The page for people

`sekisho doc` writes a page for the people who decide who may do what, in Markdown for a pull
request or, with `--format html`, as one HTML file. Each action has its table of the combinations
allowed and denied, with the policies that decide them, and the rule's answer is a column like the
roles. Who may refund, from the example's page (`any` is every value of the column, and `-` a
column that does not apply to the row):

| principal | clerk | manager | auditor | workflow | suspended | status | amount | refund_band | in_period | business_day | Deciding policies |
|---|---|---|---|---|---|---|---|---|---|---|---|
| User | yes | no | no | - | no | not refunded | any | within_limit | yes | any | clerks_refund_within_their_limit |
| User | yes | yes | no | - | no | not refunded | any | within_limit | yes | any | clerks_refund_within_their_limit, managers_refund_in_period |
| User | yes | yes | no | - | no | not refunded | any | over_limit | yes | any | managers_refund_in_period |
| User | yes | yes | no | - | no | not refunded | any | any | no | yes | managers_refund_late_on_business_days |
| Workflow | - | - | - | returns | - | returned | <=50GBP | - | any | any | returns_refunds_returned_orders |

The page also sets each policy beside the Cedar it becomes, and opens the pages rulec and koyomi
draw of the rule and the dates. [The example's page](https://github.com/i2y/ritsu/blob/main/crates/sekisho/tests/golden/doc/refunds.en.md)
is in the repository, with the pages of the rule and the dates left out.

## With the other languages

The same action reads a dates file and a calendar of koyomi and a contract, and a workflow of
dandori calls it:

```gate
use dates refund_terms from "dates/refund_terms.cal"
use calendar uk from "calendars/england_and_wales.cal"
use openapi orders from "api/orders.json"
…
today range >=2026-10-01 <=2028-10-31 offset +00:00
…
action refund_order
  description "Refund an order, in part or in whole"
  guards orders refundOrder
  …
    in_period    = today <= refund_terms.last_day(paid_on: resource.paid_on)
    business_day = today is open in uk
```

### Dates

`in_period` says whether today is on or before the last day of the refund period, which a dates
file of koyomi computes from the day the order was paid: 30 days later, moved to the next business
day in England and Wales. `business_day` says whether today is a business day, by a calendar of
koyomi that is closed on Saturdays, Sundays and the bank holidays of England and Wales, as GOV.UK
lists them. The check asks koyomi which answers can come together on one day, over every day of
`today`'s range, and a calendar without data for every day the gate can be asked on is an error
(E207). Today is taken from the server's clock, and the generated code refuses a day outside the
range.

### The contract

`guards orders refundOrder` names the operation of the OpenAPI document `api/orders.json` that the
action guards. The operation has to be there (E202), the input has to be what it takes (E203), and
the order's id is read from its argument `orderId` (E204).

### A workflow

The workflow `returns` refunds a returned order through the same operation, calling as itself:

```flow
task refund_order(orderId: string, amount: money[GBP, incl_tax] range >=1 <=10000) -> Refund
  http POST orders "/orders/{orderId}/refunds"
  errors denied = 403
  key
```

```gate
permit returns_refunds_returned_orders
  description "The returns workflow refunds a returned order, up to 50 pounds"
  principal is workflow returns
  action refund_order
  when resource.status is returned
  when amount <= 50GBP
```

`ritsu check` holds every call a flow makes to what its gate allows the workflow. A call allowed in
no combination is an error (E908); a call that can be denied, from a task that declares no error for
a denial, a warning (W909); and an action the workflow is allowed and never calls, a warning too
(W908). Here the task declares it, and the call is one border for each version of the gate:

```console
$ cd crates/sekisho/examples/refunds
$ ritsu check .
ok rules/refund_limit.rule
ok rules/返金の上限.rule
calendars/england_and_wales.cal: ok — calendar england_and_wales: table bank_holidays 83 rows, data range 2019-01-01..2028-12-31
dates/refund_terms.cal: ok — 3 claims hold on all 1,060 days of paid_on (2026-01-01..2028-11-25); 2 examples match
dates/返金の期限.cal: ok — 3 claims hold on all 1,060 days of 支払日 (2026-01-01..2028-11-25); 2 examples match
flows/returns.flow: ok
refunds.gate: ok — 3 actions, 10 policies (7 permits, 3 forbids), 3 expectations, 1 separation
refunds.ja.gate: ok — 3 actions, 10 policies (7 permits, 3 forbids), 3 expectations, 1 separation
ritsu check: 8 files (rulec 2, koyomi 3, dandori 1, sekisho 2): all pass; borders between the languages: 2 checked, 0 undecided
```

### The map

When sakai's map says a context opens an operation to the others (`open host service`),
`ritsu check` holds the operation to an action of a gate that guards it, unless its contract says
anyone may call it. An operation no action guards is an error (E907) where the context's other
operations are guarded, and a warning (W907) while none of them is. The example has no map; the
shop in the browser has one.

## Cedar written by hand

ritsu reads Cedar that people write by hand, too: a file of policies with its schema beside it
(`refunds.cedar` and `refunds.cedarschema`), when the map (`owns cedar "…"`) or a requirement of
yuen names it, and the schema says with `@guards` which operation each action guards. Its actions
are held to the operations of the map as a gate's are. sekisho counts its policies over the part of
Cedar its own conditions are made of (the scope, roles, attributes compared with constants, `has`,
and the like), and its tests hold every combination it counts to the answer of the official Cedar
CLI. Where a policy goes beyond that part (`like`, arithmetic, sets, the functions of the
extensions), sekisho says the question cannot be decided, and which part of which policy. The
calls of workflows are held to gates only: Cedar has no way to say which flow a workflow is.

## Try it in the browser

[The playground](playground/#project=sekisho/refunds) opens this example. Press *generate* on the
gate for its Cedar, or open *the page for people*. The [small shop](playground/#project=shop&file=gates/orders.gate)
has two gates: `gates/orders.gate` asks the rule the workflow follows whether an order can be
cancelled at all, and `gates/warehouse.gate` guards the operations the workflow calls. With the
shop's map, every operation ordering and the warehouse open to the other contexts is guarded by an
action of one of the two.

## Read more

- [sekisho's README](https://github.com/i2y/ritsu/blob/main/crates/sekisho/README.md): the language
  at a glance, its commands, and how it is tested.
- [The reference](https://github.com/i2y/ritsu/blob/main/crates/sekisho/docs/reference.md): the
  whole language, what `check` says and what `gen` writes.
- [The diagnostics](https://github.com/i2y/ritsu/blob/main/crates/sekisho/docs/codes.md): for each
  code, when it comes, how to fix it, and the smallest gate that gives it (`sekisho explain` prints
  one).
- [DESIGN.md](https://github.com/i2y/ritsu/blob/main/crates/sekisho/DESIGN.md), in Japanese: why
  Cedar, how the answers of the other languages reach it, and what is not done yet.
- [The skill](https://github.com/i2y/ritsu/blob/main/skills/sekisho/SKILL.md), for an agent that
  writes or fixes a gate.
