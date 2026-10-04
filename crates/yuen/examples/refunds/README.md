# refunds

"A refund does not exceed the sale of its order", a rule decided for this example, met by an account and two transfers of chobo's example book and checked by chobo's check of the book. The end of each artifact is the definition of that one account or transfer, so a change to another part of the book marks nothing here.

`refunds.req` is in English, over `refunds.book`; `refunds.ja.req` is the same project in Japanese, over `refunds.ja.book`. Each `.req` is a project of its own.

Copied from the other languages of ritsu (`crates/<language>/` in this repository), as they were on 2026-10-04:

- `refunds.book`: chobo, `examples/refunds/refunds.book`
- `refunds.ja.book`: chobo, `examples/refunds/refunds.ja.book`

`reviewed/` and the `reviewed` and `approved` lines were written by `yuen review --date 2026-10-04`.

```console
$ ritsu yuen check examples/refunds/refunds.req
$ ritsu yuen check examples/refunds/refunds.ja.req
```
