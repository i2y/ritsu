# ritsu-osha

Limits from the occupational safety standards of the United States, as rules of rulec, for a module
of ritsu to use. Each rule reads one section of Title 29 of the Code of Federal Regulations. The
section is kept beside the rule as the eCFR served it, and the rule pins it by its SHA-256.

| File | Rule | What it answers | Section |
|---|---|---|---|
| `osha_extinguisher.rule` | `osha_extinguisher` | How far an employee may have to walk to a portable fire extinguisher | 29 CFR 1910.157(d) |
| `osha_noise.rule` | `osha_noise` | How long a workplace may expose an employee to a level of continuous noise | 29 CFR 1910.95, Table G-16 |
| `osha_excavation.rule` | `osha_excavation` | Whether an excavation needs a protective system against cave-ins | 29 CFR 1926.652(a)(1) |

The rules transcribe the sections as of 2026-01-01, to be read against them. They are not legal
advice. Where a section leaves something open, a comment in the rule says how it was decided:
`osha_noise.rule` gives a level between two listed ones the shorter of their two times.

## Use

Add the module, add the line that checks its signature to your `ritsu.mod`, and fetch it:

```console
$ ritsu mod add osha ghcr.io/i2y/ritsu-osha@1.0.0
```

```
signature "ghcr.io/i2y/ritsu-osha" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

```console
$ ritsu mod fetch
```

Then read it from your own code of kaname or dandori:

```
use rule noise from "@osha/osha_noise.rule"
```

## Sources

`rulec source outdated <file>` asks the eCFR whether a later amendment touched a section.
`THIRD_PARTY_NOTICES.md` lists the copies under `sources/law/`.

The rules are under the MIT License or the Apache License, version 2.0, at your option, as ritsu is.

## Versions

A version that follows a change in the law raises the minor version. A version that renames a file
or removes one, or changes the inputs or outputs of a rule, raises the major version.
