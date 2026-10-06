# sekisho

A small language for who may do what: principals, roles, attributes and relations, with the answers of business rules and dates as conditions, checked on every combination it declares and compiled to [Cedar](https://www.cedarpolicy.com/).
sekisho is one of the eight languages of [ritsu](../../README.md): it reads the rules of rulec, the dates and calendars of koyomi and the workflows of dandori through ritsu, and the contracts services keep in OpenAPI, AsyncAPI and `.proto` files.

Authorization is usually a set of policies nobody can read whole: which role may do what is spread over the policies, and a condition such as "while the refund period lasts" or "up to the clerk's own limit" is computed somewhere in the service and handed in as a fact.
sekisho writes the policies of one part of a service in one `.gate` file, beside the contract whose operations they guard, and takes the conditions from the languages that decide them:

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
…
permit clerks_refund_within_their_limit
  description "A clerk refunds up to the clerk's own limit, while the refund period lasts"
  principal in clerk
  action refund_order
  when refund_band is within_limit
  when in_period
…
forbid suspended_staff_do_nothing
  description "A suspended member of the staff does nothing"
  principal is User
  action any
  when principal.suspended
…
expect deny clerks_never_refund_over_their_limit
  description "A clerk who is not a manager never refunds more than the clerk's own limit"
  principal in clerk
  action refund_order
  unless principal in manager
  when refund_band is over_limit
```

`refund_band` is what a rule of rulec answers, `in_period` and `business_day` what koyomi answers of a dates file and of the bank holidays of England and Wales.
`sekisho check` asks rulec which bands each stretch of amounts can come to, and koyomi which answers can come together on one day, and walks every combination of roles, attributes, amounts and answers that can happen:

```console
$ sekisho check examples/refunds/refunds.gate --root examples/refunds
examples/refunds/refunds.gate: ok — 3 actions, 10 policies (7 permits, 3 forbids), 3 expectations, 1 separation
```

That is 1,078 combinations over the three actions, each decided as Cedar decides it: a request no policy allows is denied, and a forbid that applies wins over every permit.
The three expectations hold on every combination they pick, the separation of duties holds, and each role is allowed exactly what its `can` line lists.

## What the checker says

A forbid aimed at clerks also hits managers, because a manager includes clerk.
Nothing in Cedar stops it, and the permits for managers silently allow nothing:

```text
error[E302]: tests/mutants/E302_forbid_covers_permit.gate:94:1: The permit `managers_refund_in_period` allows nothing: a forbid denies every combination it would allow
    94 | permit managers_refund_in_period
  = It would allow 256 combinations; `clerks_do_not_refund` denies 256 of them, `auditors_do_not_refund` 128, `suspended_staff_do_nothing` 128, `no_second_refund` 64.
  = For example: User holding manager (suspended: no), Order (status: paid), amount: 1GBP to 50GBP, refund_band: within_limit, in_period: yes, business_day: no; denied by `clerks_do_not_refund`.
  = Narrow the forbid, or remove the permit if no one is meant to be allowed this.
```

An expectation is what a person has decided must hold, written beside the policies.
With two of its lines taken out, the one that says a manager refunds in the period no longer holds, and the check shows a combination where it does not:

```text
error[E304]: tests/mutants/E304_expect_fails.gate:147:1: The expectation `managers_refund_in_period` does not hold: 144 of the 192 combinations it picks are denied
   147 | expect allow managers_refund_in_period
  = For example: User holding manager (suspended: yes), Order (status: paid), amount: 1GBP to 50GBP, refund_band: within_limit, in_period: yes, business_day: no; denied by `suspended_staff_do_nothing`.
  = Fix the policies, or the expectation if it says more than is meant.
```

The other checks: an action no permit allows, a policy whose conditions never hold together, a permit another one already covers, two duties a `separate` keeps apart that one principal is allowed, a role allowed more or less than its `can` line, an operation the contract does not have, an input the operation does not take, a value given to a rule outside what it takes or breaking its precondition, a calendar without data for every day the gate can be asked on.
Every check comes to one of three answers: it holds, here is a combination where it does not, or it cannot be decided, and then it says why.
Every code is in [docs/codes.md](docs/codes.md), with when it comes, how to fix it, and the smallest file that prints it (`sekisho explain E302`).

## Cedar, and the code that asks it

Only a gate that passes its check is generated.
`gen --target cedar` writes Cedar's schema and policies, in its text and in its JSON:

```console
$ sekisho gen examples/refunds/refunds.gate --target cedar --root examples/refunds --out generated
generated: generated/cedar/refunds.cedar
generated: generated/cedar/refunds.cedarschema
generated: generated/cedar/refunds.cedarschema.json
generated: generated/cedar/refunds.policies.json
```

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

Cedar trusts what a request tells it.
If the caller sent `refund_band`, the caller could allow their own refund.
So `gen --target typescript`, `python` or `go` writes, for each action, a function that reads the principal and the order from the service's own data, computes the band and the dates with the code rulec and koyomi generate, builds the request and asks Cedar: cedar-wasm, cedarpy or cedar-go in the process, or Amazon Verified Permissions with `--authorizer avp`.
The function takes no computed value from the caller, and refuses before asking Cedar when the data is not what the gate declared or the day is outside the range the gate was checked over.
`ritsu gen` writes the same into the package of a project, beside the code of the rules and dates it reads.

## Across the languages

`ritsu check` reads the gates of a project with every other language, and adds two checks across them.
Every operation a context of sakai's map opens is guarded by an action, unless its contract says anyone may call it (ritsu's E907 and W907).
A workflow is allowed the operations its flow calls, handles a denial where it can be denied, and is allowed nothing it never calls (E908, W909, W908).
In the example, the workflow `returns` calls `refundOrder`, which the gate allows it for a returned order up to 50 pounds, and its task declares the error of a denial:

```console
$ ritsu check examples/refunds
ok examples/refunds/rules/refund_limit.rule
ok examples/refunds/rules/返金の上限.rule
examples/refunds/calendars/england_and_wales.cal: ok — calendar england_and_wales: table bank_holidays 83 rows, data range 2019-01-01..2028-12-31
examples/refunds/dates/refund_terms.cal: ok — 3 claims hold on all 1,060 days of paid_on (2026-01-01..2028-11-25); 2 examples match
examples/refunds/dates/返金の期限.cal: ok — 3 claims hold on all 1,060 days of 支払日 (2026-01-01..2028-11-25); 2 examples match
examples/refunds/flows/returns.flow: ok
examples/refunds/refunds.gate: ok — 3 actions, 10 policies (7 permits, 3 forbids), 3 expectations, 1 separation
examples/refunds/refunds.ja.gate: ok — 3 actions, 10 policies (7 permits, 3 forbids), 3 expectations, 1 separation
ritsu check: 8 files (rulec 2, koyomi 3, dandori 1, sekisho 2): all pass; borders between the languages: 2 checked, 0 undecided
```

A file of Cedar written by hand takes part too, when the map or a requirement names it and its schema says with `@guards` which operation each action guards.

## The page for people

`sekisho doc` writes a page for the people who decide who may do what: each action's table of the combinations allowed and denied, with the policies that decide them; each policy beside the Cedar it becomes; the computed values, with the pages rulec and koyomi draw of their rule and dates; the expectations, the separations and the roles; the workflows and what their flows call; and the operations guarded.
The Markdown is for a pull request; the HTML is one file that reads nothing from outside.
[tests/golden/doc/refunds.en.md](tests/golden/doc/refunds.en.md) is the example's page, with the pages of the rule and the dates left out.

## For AI agents

[skills/sekisho](../../skills/sekisho/SKILL.md) is a skill for an agent that writes or fixes a gate: the loop, the language on one page, what to ask a person, and how to fix each diagnostic, sekisho's and ritsu's across the languages.
Its pages besides `SKILL.md` are copies of `docs/`, made by `skills/sync.sh`.

## Install

sekisho comes with ritsu:

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

Run it as `ritsu sekisho …`.
Called through a link named `sekisho`, ritsu is sekisho, with every language sekisho reads joined; the archives of a release carry the link, and `ln -s ritsu sekisho` beside the binary makes one.
The binary of sekisho's own crate joins no other language: it stops with E209 at a gate that reads a rule, a dates file, a calendar, a book or a flow.

## Commands

```console
$ sekisho check examples/refunds/refunds.gate --root examples/refunds
$ sekisho gen examples/refunds/refunds.gate --target typescript --root examples/refunds
$ sekisho doc examples/refunds/refunds.gate --root examples/refunds --format html
$ sekisho vectors examples/refunds/refunds.gate --root examples/refunds --action refund_order
$ sekisho api examples/refunds/refunds.gate --root examples/refunds
$ sekisho explain E302
```

`--root` is the directory the paths of what a gate names are written from (else the nearest one above the first file that holds `.git`), so that sekisho, yuen and sakai name one operation alike.
`vectors` prints every combination as a test of `cedar run-tests`.
Every command takes `--lang ja|en` (else `SEKISHO_LANG`, then `RITSU_LANG`, else English) and `--help`.
The exit code is 0 for no errors, 1 for errors, and 2 for bad arguments, a file that cannot be read, or a language that is not joined.
[docs/reference.md](docs/reference.md) has the whole language and every command.

## Examples

- [examples/refunds](examples/refunds): who may look at an order of a shop, refund it and export the record of refunds, with a rule of rulec for the clerk's limit, a dates file of koyomi for the refund period, the bank holidays of England and Wales, an OpenAPI document of the operations, and a dandori workflow that refunds returned orders.
  `refunds.ja.gate` is the same gate with Japanese names, reading the Japanese versions of the rule and the dates file.

## How it is checked

`cargo test -p sekisho` runs the language on its mutants and its examples, and runs the real Cedar.

- 120 mutants, each the example or a fixture with one change, are held to the diagnostics they give, in English and in Japanese (60 of them with Japanese names, each with an English twin).
- The Cedar generated from every gate of the examples and the tests that passes check goes through the official Cedar CLI 4.13.0: `cedar validate` in strict mode finds nothing, `cedar format --check` passes, `cedar run-tests` answers every combination as sekisho's reference evaluation does, with the same deciding policies, and the JSON forms are what `cedar translate-schema` and `translate-policy` write.
- The code generated for TypeScript, Python and Go builds the requests of every combination from raw data and asks cedar-wasm 4.13.0, cedarpy 4.12.1 and cedar-go v1.8.0, which answer as the reference evaluation; `tsc --strict`, `mypy --strict`, `go vet` and gofmt pass it.
- The pages of `doc` are golden files, the pages of the rules and dates in them are what rulec and koyomi draw, and Chrome opens the HTML.
- The outputs, diagnostics and the lines of `.gate` and Cedar in this README, `docs/` and the skill are real: `tests/docs.rs` runs and compares them, and ritsu's `tests/sekisho.rs` runs the `ritsu check` above.

## Status

The language, `check`, `gen` for Cedar, TypeScript, Python and Go, `doc`, `vectors`, `api` and `explain` are done, as [DESIGN.md](DESIGN.md) says.
Not yet: relations of more than one step, numbers a rule outputs as conditions, the code of `--authorizer avp` run against a real Verified Permissions (it is type-checked only), and the workflows of Cedar written by hand (DESIGN 15).

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or [MIT license](../../LICENSE-MIT), at your option.
