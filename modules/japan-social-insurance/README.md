# ritsu-japan-social-insurance

The health insurance and employees' pension premiums of Japan, as rules of rulec, for a module of
ritsu to use. Each rule is here in English and in Japanese, and the two give the same answers. From
the monthly pay, a rule takes the standard monthly remuneration by the grade table, multiplies it by
the rate and halves it. It then gives the amount deducted from salary and the amount paid in cash,
which round a half of a yen differently.

| In English | In Japanese | Premium | Table |
|---|---|---|---|
| `health_insurance_premium.rule` | `健康保険料.rule` | The health insurance of the Japan Health Insurance Association (Kyokai Kenpo) | The association's premium table from March 2026 (paid in April), Tokyo branch |
| `pension_insurance_premium.rule` | `厚生年金保険料.rule` | The employees' pension insurance | The Japan Pension Service's premium table from September 2020 (paid in October), fiscal 2026 edition, pinned |

The rates are inputs. The rate of health insurance changes with the prefecture and the fiscal year,
and that of a pension with the fund, so a rule does not hold one: pass the rate that applies. The
rules transcribe the tables, to be read against them. They are not legal or tax advice.

## Use

Add the module, add the line that checks its signature to your `ritsu.mod`, and fetch it:

```console
$ ritsu mod add japan_social_insurance ghcr.io/i2y/ritsu-japan-social-insurance@1.0.0
```

```
signature "ghcr.io/i2y/ritsu-japan-social-insurance" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

```console
$ ritsu mod fetch
```

Then read it from your own code of kaname or dandori:

```
use rule pension from "@japan_social_insurance/pension_insurance_premium.rule"
```

## Sources

`THIRD_PARTY_NOTICES.md` says where the copy under `sources/nenkin/` comes from. The health
insurance's rule names its table in a comment, and carries no copy.

The rules are under the MIT License or the Apache License, version 2.0, at your option, as ritsu is.

## Versions

A version that follows a change in the law raises the minor version. A version that renames a file
or removes one, or changes the inputs or outputs of a rule, raises the major version.
