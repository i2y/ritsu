# civil_code_periods

Where the dates and claims of koyomi's example `civil_code_period_end.ja.cal` (the periods of Japan's Civil Code, articles 140 to 143, written as they read) come from, and who decided how article 142's "the following day" is read. The source is the copy of the Civil Code the `.cal` pins, borrowed from it. The project is in Japanese only: the law is Japan's, and only e-Gov serves it (the English examples that read a law are `osha` and the test fixtures over 37 CFR 1).

Written as an example; it does not say how the law is to be read.

Copied from the other languages of ritsu (`crates/<language>/` in this repository), as they were on 2026-10-04:

- `civil_code_period_end.ja.cal`: koyomi, `examples/civil_code_period_end.ja.cal`
- `calendars/民法142条の休日.cal`: koyomi, `examples/calendars/民法142条の休日.cal`
- `calendars/data/syukujitsu.csv`: koyomi, `examples/calendars/data/syukujitsu.csv` (the Cabinet Office's table of national holidays)
- `sources/law/129AC0000000089@2026-10-01/`: koyomi, `examples/sources/law/129AC0000000089@2026-10-01/` (e-Gov's XML of articles 140 to 143, as fetched)

`reviewed/` and the `reviewed` and `approved` lines were written by `yuen review --date 2026-10-04`.

```console
$ ritsu yuen check examples/civil_code_periods/civil_code_periods.ja.req
```
