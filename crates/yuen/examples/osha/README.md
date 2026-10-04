# osha

An English example that reads a regulation: where the distances of rulec's rule `osha_extinguisher.rule` come from. The source is 29 CFR 1910.157, read from the eCFR as of 2026-01-01, copied and pinned by yuen itself; the copy is the same bytes as the copy the rule pins, so the check finds the two the same (E107 does not come). The requirement is met by the rule's table `distance` and checked by rulec's own check of the rule (its examples and its completeness).

Written as an example; it does not say how the regulation is to be read.

Copied from the other languages of ritsu (`crates/<language>/` in this repository), as they were on 2026-10-04:

- `osha_extinguisher.rule`: rulec, `tests/corpus/osha_extinguisher.rule`
- `sources/law/29-CFR-1910@2026-01-01/1910.157.xml`: rulec, `tests/corpus/sources/law/29-CFR-1910@2026-01-01/1910.157.xml` (the eCFR's XML of the section, as fetched)

`reviewed/` and the `reviewed` and `approved` lines were written by `yuen review --date 2026-10-04`.

```console
$ ritsu yuen check examples/osha/osha.req
```
