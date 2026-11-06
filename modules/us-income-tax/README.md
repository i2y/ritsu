# ritsu-us-income-tax

The federal income tax of an unmarried individual in the United States, as rules of rulec, for a
module of ritsu to use: a rule for each taxable year. Each reads the rate table of the Internal
Revenue Service, kept beside it and pinned by its SHA-256.

| File | Rule | Taxable years beginning in | Table |
|---|---|---|---|
| `us_income_tax_2026.rule` | `us_income_tax_2026` | 2026 | Rev. Proc. 2025-32, section 4.01, Table 3 |
| `us_income_tax_2025.rule` | `us_income_tax_2025` | 2025 | Rev. Proc. 2024-40, section 3.01, Table 3 |

The tables are of Section 1(j)(2)(C): unmarried individuals, other than surviving spouses and heads
of households. A rule takes the taxable income, in cents, and gives the tax. Working out the taxable
income is not in it. The rules transcribe the tables, to be read against them. They are not tax
advice.

## Use

Add the module, add the line that checks its signature to your `ritsu.mod`, and fetch it:

```console
$ ritsu mod add us_income_tax ghcr.io/i2y/ritsu-us-income-tax@1.0.1
```

```
signature "ghcr.io/i2y/ritsu-us-income-tax" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

```console
$ ritsu mod fetch
```

Then read it from your own code of kaname or dandori:

```
use rule tax from "@us_income_tax/us_income_tax_2026.rule"
```

## Sources

`THIRD_PARTY_NOTICES.md` lists the copies under `sources/`.

The rules are under the MIT License or the Apache License, version 2.0, at your option, as ritsu is.

## Versions

A version that adds the rule of a new taxable year raises the minor version. A version that renames
a file or removes one, or changes the inputs or outputs of a rule, raises the major version.
A version that changes only the words of this README raises the patch version.
