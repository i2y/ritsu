# ritsu-holidays

The public holidays of the United Kingdom and of Japan, as calendars of koyomi, for a module of
ritsu to use. Each calendar closes the holidays and nothing else. Which other days a business is
closed (Saturdays and Sundays, the days between Christmas and the New Year) is for the calendar
that uses it to say.

| File | Calendar | Holidays | Known from the data |
|---|---|---|---|
| `uk/england_and_wales.cal` | `england_and_wales` | The bank holidays of England and Wales | 2019-01-01 to 2028-12-31 |
| `uk/scotland.cal` | `scotland` | The bank holidays of Scotland | 2019-01-01 to 2028-12-31 |
| `uk/northern_ireland.cal` | `northern_ireland` | The bank holidays of Northern Ireland | 2019-01-01 to 2028-12-31 |
| `japan/national_holidays.cal` | `national_holidays` | The national holidays of Japan and the other days off under the Act on National Holidays | 1955-01-01 to 2027-12-31 |
| `japan/国民の祝日.cal` | `国民の祝日` | The same, in Japanese | 1955-01-01 to 2027-12-31 |

The calendars of the United Kingdom have no offset: the three nations have daylight saving time,
so they give dates only. The calendars of Japan have the offset `+09:00`. Whether a day outside
the range its data knows is a holiday is not known, and koyomi says so rather than guessing.

## Use

Require the module, and the line that checks its signature:

```console
$ ritsu mod add holidays ghcr.io/i2y/ritsu-holidays@1.0.0
```

```
require holidays oci "ghcr.io/i2y/ritsu-holidays" 1.0.0
signature "ghcr.io/i2y/ritsu-holidays" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

Then read a calendar from your own, and close what your business closes on top of it:

```cal
calendar office v1
description "The office in London: closed on Saturdays, Sundays and the bank holidays of England and Wales"
use calendar "@holidays/uk/england_and_wales.cal"

closed weekly sat, sun
```

## Data

Each data file is, byte for byte, the file its publisher serves, and each calendar pins it by its
SHA-256. `THIRD_PARTY_NOTICES.md` says where each was taken from, when, and under which terms.

- `uk/data/bank-holidays.json`: GOV.UK's bank holidays (<https://www.gov.uk/bank-holidays.json>),
  under the Open Government Licence v3.0. Contains public sector information licensed under the
  Open Government Licence v3.0.
- `japan/data/syukujitsu.csv`: the Cabinet Office's table of the national holidays
  (<https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv>), in Shift_JIS as served, under the
  Public Data License, version 1.0 (PDL1.0).

The calendar files are under the MIT License or the Apache License, version 2.0, at your option,
as ritsu is.

## Versions

A version that takes in a new year's holidays raises the minor version. A version that renames a
file or removes one raises the major version.
