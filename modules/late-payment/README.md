# ritsu-late-payment

What a supplier in the United Kingdom may claim when a commercial invoice is paid late, under the
Late Payment of Commercial Debts (Interest) Act 1998, for a module of ritsu to use: a rule, a
calendar, and the code of kaname that joins them. Each is here in English and in Japanese, and the
two give the same answers.

| In English | In Japanese | What it answers |
|---|---|---|
| `dates/payment_terms.cal` | `dates/支払期限.cal` | The day an invoice falls due when no day was agreed: the last of the 30 days from the day the buyer had it (section 4) |
| `rules/late_payment_charges.rule` | `rules/遅延の請求額.rule` | The fixed sum for the cost of recovering the debt (section 5A), and the interest at 8% over the reference rate, a day at a time |
| `late_payment.kn` | `late_payment.ja.kn` | The whole claim on an invoice: the day it fell due, how many days late it was paid, the fixed sum and the interest, or nothing when it was paid in time |

The reference rate is an input: pass the Bank of England base rate that applies. The rules read the
Act and GOV.UK's guide as they stood on 18 October 2026, to be read against them. They are not legal
advice. Where the guide does not say which way a fraction of a penny goes, a comment in the rule says
so, and half up is a placeholder.

## Use

Add the module, add the line that checks its signature to your `ritsu.mod`, and fetch it:

```console
$ ritsu mod add late_payment ghcr.io/i2y/ritsu-late-payment@1.0.0
```

```
signature "ghcr.io/i2y/ritsu-late-payment" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

```console
$ ritsu mod fetch
```

Then read it from your own code of kaname or dandori:

```
use rule charges from "@late_payment/rules/late_payment_charges.rule"
use dates terms from "@late_payment/dates/payment_terms.cal"
```

## Sources

`THIRD_PARTY_NOTICES.md` lists the copies under `rules/sources/`. Contains public sector information
licensed under the Open Government Licence v3.0.

The rule, the calendar and the code are under the MIT License or the Apache License, version 2.0, at
your option, as ritsu is.

## Versions

A version that follows a change in the law raises the minor version. A version that renames a file
or removes one, or changes the inputs or outputs of a rule, raises the major version.
