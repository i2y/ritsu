# payment_terms

Where the dates of koyomi's example `payment_20th_close_next_10th.cal` come from. The payment day (close on the 20th, pay on the 10th of the next month) is a rule decided for this example; the business days come from the table of national holidays the calendar file pins, borrowed from it (`source holidays = koyomi "calendars/tokyo_business_days.cal" source national_holidays`). What checks them is koyomi's claims, which koyomi holds on every day of the input's range; the scope is every date of the `.cal`.

`payment_terms.req` is in English, and `payment_terms.ja.req` is the same project in Japanese, over koyomi's Japanese version of the file. Each `.req` is a project of its own.

Copied from the other languages of ritsu (`crates/<language>/` in this repository), as they were on 2026-10-04:

- `payment_20th_close_next_10th.cal`: koyomi, `examples/payment_20th_close_next_10th.cal`
- `payment_20th_close_next_10th.ja.cal`: koyomi, `examples/payment_20th_close_next_10th.ja.cal`
- `calendars/tokyo_business_days.cal`: koyomi, `examples/calendars/tokyo_business_days.cal`
- `calendars/東京の営業日.cal`: koyomi, `examples/calendars/東京の営業日.cal`
- `calendars/data/syukujitsu.csv`: koyomi, `examples/calendars/data/syukujitsu.csv` (the Cabinet Office's table of national holidays; see THIRD_PARTY_NOTICES.md)

`reviewed/` and the `reviewed` and `approved` lines were written by `yuen review --date 2026-10-04`.

```console
$ ritsu yuen check examples/payment_terms/payment_terms.req
$ ritsu yuen check examples/payment_terms/payment_terms.ja.req
```
