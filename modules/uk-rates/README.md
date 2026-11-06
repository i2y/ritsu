# ritsu-uk-rates

Income Tax, Stamp Duty Land Tax and the minimum wage of the United Kingdom, as rules of rulec, for a
module of ritsu to use. Each rule reads a page of GOV.UK, kept beside it and pinned by its SHA-256.

| File | Rule | What it answers | As of |
|---|---|---|---|
| `uk_income_tax.rule` | `uk_income_tax` | The Personal Allowance, and the Income Tax band an income falls in, for England, Wales and Northern Ireland | The tax year 6 April 2026 to 5 April 2027 |
| `uk_stamp_duty.rule` | `uk_stamp_duty` | The SDLT rate band of a residential purchase in England or Northern Ireland, with the relief for first-time buyers and the surcharge on a second home | The page on 21 September 2026 |
| `uk_minimum_wage.rule` | `uk_minimum_wage` | The hourly minimum wage by age, and the rate of an apprentice | From 1 April 2026 |

The rules transcribe the pages, to be read against them. They are not tax or legal advice. The first
gives the band an income falls in and not the tax, and the second the band a price falls in and not
the duty, as the tax is worked out slice by slice and the pages tabulate no total.

## Use

Add the module, add the line that checks its signature to your `ritsu.mod`, and fetch it:

```console
$ ritsu mod add uk_rates ghcr.io/i2y/ritsu-uk-rates@1.0.1
```

```
signature "ghcr.io/i2y/ritsu-uk-rates" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

```console
$ ritsu mod fetch
```

Then read it from your own code of kaname or dandori:

```
use rule income_tax from "@uk_rates/uk_income_tax.rule"
```

## Sources

`THIRD_PARTY_NOTICES.md` lists the copies under `sources/`, taken on 21 September 2026. Contains
public sector information licensed under the Open Government Licence v3.0.

The rules are under the MIT License or the Apache License, version 2.0, at your option, as ritsu is.

## Versions

A version that follows a change in the law raises the minor version. A version that renames a file
or removes one, or changes the inputs or outputs of a rule, raises the major version.
A version that changes only the words of this README raises the patch version.
