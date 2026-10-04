# civil_code_periods_reread

The example that stops on purpose. It is `civil_code_periods` with one line of the `.cal` rewritten after the decision was recorded: `満了日_142条` reads article 142's "the following day" as `roll following` (move on until the days off are over) instead of `if closed + 1 day`. The `.req` and `reviewed/` are the same bytes as in `civil_code_periods`, so `yuen check` stops with E303 on the link to that date and shows the difference from what was looked at. The other dates and the claims keep their definitions, so their links stay as they were.

Written as an example; it does not say how the law is to be read.

Copied from the other languages of ritsu (`crates/<language>/` in this repository), as they were on 2026-10-04:

- `civil_code_period_end.ja.cal`: koyomi, `examples/civil_code_period_end.ja.cal`, with line 20 changed from `if closed + 1 day` to `roll following`
- `calendars/民法142条の休日.cal`: koyomi, `examples/calendars/民法142条の休日.cal`
- `calendars/data/syukujitsu.csv`: koyomi, `examples/calendars/data/syukujitsu.csv`
- `sources/law/129AC0000000089@2026-10-01/`: koyomi, `examples/sources/law/129AC0000000089@2026-10-01/`
- `civil_code_periods_reread.ja.req`: this crate, `examples/civil_code_periods/civil_code_periods.ja.req`, byte for byte
- `reviewed/`: this crate, `examples/civil_code_periods/reviewed/`, byte for byte

`reviewed/` and the `reviewed` and `approved` lines were written by `yuen review --date 2026-10-04`.

```console
$ ritsu yuen check examples/civil_code_periods_reread/civil_code_periods_reread.ja.req
```
