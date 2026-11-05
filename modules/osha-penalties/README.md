# ritsu-osha-penalties

The most the Occupational Safety and Health Administration of the United States may propose for a
violation, and the least for a willful one, as a rule of rulec. The rule reads 29 CFR 1903.15(d). The
section is kept beside the rule as the eCFR served it, and the rule pins it by its SHA-256.

| File | Name | What it holds | Section |
|---|---|---|---|
| `osha_penalty.rule` | `osha_penalty` | For a kind of violation, the most and the least of a penalty, and whether it counts by the violation or by the day | 29 CFR 1903.15(d) |
| `osha_penalties.req` | `penalty_amounts` | The amounts in a sentence, the amendment this version follows, and who read each link | 29 CFR 1903.15(d) |

This version transcribes the section as of 2023-02-01, for penalties proposed after January 15, 2023. It is to be
read against the section, and is not legal advice. The section states what may be proposed: what is
proposed for a violation also turns on the matters of 1903.15(b), such as the size of the business.

## Use

Add the module, add the line that checks its signature to your `ritsu.mod`, and fetch it:

```console
$ ritsu mod add osha_penalties ghcr.io/i2y/ritsu-osha-penalties@1.0.0
```

```
signature "ghcr.io/i2y/ritsu-osha-penalties" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

```console
$ ritsu mod fetch
```

Then read it from your own code of kaname or dandori:

```
use rule penalty from "@osha_penalties/osha_penalty.rule"
```

## Sources

`rulec source outdated <file>` asks the eCFR whether a later amendment touched the section.
`THIRD_PARTY_NOTICES.md` lists the copy under `sources/law/`.

The rule is under the MIT License or the Apache License, version 2.0, at your option, as ritsu is.

## Requirements

`osha_penalties.req` states the section's amounts in a sentence, says which amendment this version
follows, and links the section to the sentence and the sentence to the rule. Each link has a line
saying who read both ends and when, written by `yuen review`. `reviewed/` keeps the text of each end
as it was read, and `ritsu check` stops if an end has moved since. A module's page and `ritsu mod diff`
show these words beside what the new version answers. They are what the maintainer read and decided:
ritsu checks that they have not moved since, not that they are right.

## Versions

The Department of Labor adjusts the amounts for inflation by an amendment of the section each
January. A version that follows such a change raises the minor version, so that the difference between
two versions is what the amendment moved: `ritsu mod diff ghcr.io/i2y/ritsu-osha-penalties@<old>
ghcr.io/i2y/ritsu-osha-penalties@<new>` says it. A version that renames the rule's file or removes it, or
changes the inputs or outputs of the rule, raises the major version.

| Version | Penalties proposed after | Willful or repeated, at most | Others, at most | Willful, at least | Federal Register |
|---|---|---|---|---|---|
| 1.0.0 | January 15, 2023 | 156,259 USD | 15,625 USD | 11,162 USD | 88 FR 2217 |
