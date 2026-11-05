# ritsu-japan-tax

Income tax and stamp duty of Japan, as rules of rulec, for a module of ritsu to use. Each rule is
here in English and in Japanese, and the two give the same answers.

| In English | In Japanese | What it answers | Source |
|---|---|---|---|
| `japan_income_tax.rule` | `所得税.rule` | The income tax on a taxable income, from the quick-calculation table, with the 2.1% surtax | National Tax Agency, Tax Answer No. 2260, as of 1 April 2026 |
| `japan_stamp_duty.rule` | `印紙税.rule` | The stamp duty on a contract for the transfer of real estate (a Class 1 document), with the reduced rate of contracts made by 31 March 2027 | Appended Table 1 of the Stamp Tax Act and Article 91 of the Act on Special Measures Concerning Taxation, as of 1 April 2026, pinned |
| `receipt_stamp_duty.rule` | `領収書の印紙税.rule` | The stamp duty on a receipt for the price of goods sold (a Class 17, item 1 document) | National Tax Agency, Tax Answer No. 7141, as of 1 April 2026 |

The rules transcribe their sources, to be read against them. They are not tax advice. The tax
reform of fiscal 2026 changes how the income tax's surtax is made up from 2027: the rule reads the
page as it stood on 1 April 2026.

## Use

Add the module, add the line that checks its signature to your `ritsu.mod`, and fetch it:

```console
$ ritsu mod add japan_tax ghcr.io/i2y/ritsu-japan-tax@1.0.0
```

```
signature "ghcr.io/i2y/ritsu-japan-tax" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

```console
$ ritsu mod fetch
```

Then read it from your own code of kaname or dandori:

```
use rule stamp_duty from "@japan_tax/japan_stamp_duty.rule"
```

## Sources

`rulec source outdated <file>` asks e-Gov whether a later amendment touched an article the stamp
duty's rule pins. `THIRD_PARTY_NOTICES.md` lists the copies under `sources/law/`. The rules that read
the Tax Answer pages name them in a comment, and carry no copy.

The rules are under the MIT License or the Apache License, version 2.0, at your option, as ritsu is.

## Versions

A version that follows a change in the law raises the minor version. A version that renames a file
or removes one, or changes the inputs or outputs of a rule, raises the major version.
