# stamp_tax

Where the stamp duty of rulec's rule `印紙税の本則と軽減.rule` (the stamp duty on a contract for the transfer of real estate) comes from, as one requirement in two versions: v1 while the reduced rates of the Act on Special Measures Concerning Taxation, article 91, apply (2014-04-01 to 2027-03-31, the start of the rule's input `作成日` and the end of its `define 軽減期間`), and v2 after them (from 2027-04-01). The sources are copied and pinned by yuen itself; the copies are the same bytes as the rule's, so the check finds them the same (E107 does not come). The project is in Japanese only: the laws are Japan's, and only e-Gov serves them.

Written as an example; it does not say how the law is to be read.

Copied from the other languages of ritsu (`crates/<language>/` in this repository), as they were on 2026-10-04:

- `印紙税の本則と軽減.rule`: rulec, `tests/corpus/印紙税の本則と軽減.rule`
- `sources/law/342AC0000000023@2026-04-01/`: rulec, `tests/corpus/sources/law/342AC0000000023@2026-04-01/` (e-Gov's XML of the Stamp Tax Act's Appended Table 1, as fetched)
- `sources/law/332AC0000000026@2026-04-01/`: rulec, `tests/corpus/sources/law/332AC0000000026@2026-04-01/` (e-Gov's XML of article 91 of the Act on Special Measures Concerning Taxation, as fetched)

`reviewed/` and the `reviewed` and `approved` lines were written by `yuen review --date 2026-10-04`.

```console
$ ritsu yuen check examples/stamp_tax/stamp_tax.ja.req
```
