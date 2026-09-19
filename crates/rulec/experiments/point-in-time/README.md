# Point in time: how the statutes behind the corpus actually change

Whether a rule should carry an enforcement date — a "version" with a period it is in force —
was decided by counting (DESIGN.md §15.71). This directory holds what was counted: every
revision of five laws from the e-Gov statute API v2, the cited fragments at every enforcement
date, the diffs between neighbouring dates, and the two rules written to try both shapes of a
change (one folded file, and two version files bundled by a date table). Everything here was
fetched and run on 2026-09-19; the API keeps history from 2017-04-01 and the scheduled
amendments up to 2030.

The laws: 印紙税法 (342AC0000000023), 租税特別措置法 (332AC0000000026), 所得税法
(340AC0000000033), 健康保険法 (211AC0000000070), 厚生年金保険法 (329AC0000000115). The
fragments: the ones the corpus cites (別表第一, 第91条) and the ones a payroll rule would
(所得税法 第28・83・83の2・84・84の2・86・89・185・186条, 別表第二・第四; 健康保険法
第3・40・45・160・161条; 厚生年金保険法 第12・20・24の4・81・82条).

## What is here

- `revisions/rev_<law>.json` — the `law_revisions` answer for each law: 423 revisions, 305
  distinct enforcement dates.
- `fragments/index.tsv` — one line per (law, fragment, enforcement date): the revision id
  e-Gov served and the digest of the text. 1,213 pairs; `-` means the element did not exist
  yet (第84条の2 before 2025-12-01). `fragments/<law>_<elm>_<digest>.xml` are the 117
  distinct texts behind them, kept plain so they can be read and diffed.
- `laws/full_<law>.xml.gz` — the consolidated text of each law as served on 2026-09-19, for
  the supplementary provisions (附則) of every amending law.
- `reports/report_all.txt` — every change between neighbouring dates, with the text diff and a
  heuristic kind; `reports/suppl_counts.json` — the transitional wording tally;
  `reports/fold.log` and `reports/bundle.log` — the two experiments run with the fixed tool;
  `reports/suppl.log` — a rule citing an amending law's supplementary provision, fetched,
  pinned, checked and rendered; `reports/outdated-suppl.log` and `reports/outdated-2024.log` —
  `rulec source outdated` on that rule and on the stamp tax rule read as of 2024-01-01, with
  the diffs and the landing lines it prints since §15.71; `reports/library.log` and
  `reports/library-outdated.log` — the rules of `../library` checked, generated and run in
  the nine targets, and asked about later amendments (the pension cap's three staged raises
  come back with their diffs).
- `rules/` — `厚生年金の上限改定.rule` (the three staged raises of the pension cap in one file,
  keyed on 月分), `束.rule` with its two callees (two versions bundled, a date table picking
  the output), and `給与所得控除の経過措置.rule` (the supplementary-provision citation).

## Counts (DESIGN.md §15.71 quotes these)

| | |
|---|---|
| byte-level differences between neighbouring dates | 103 |
| of which markup only, no text change | 38 |
| text changes | 65 = wording/references 35, values (amounts, dates, rows) 19, shape (columns, inputs, outputs, enum values) 11 |
| what keys the 30 value/shape changes | 作成日 5, 年分 10, 支払日 6, 月分 3, the enforcement date itself (insured status) 6, the processing date 0 |
| where that key is written | an amending law's 附則 23, the main text 4 (措置法 第91条), the enforcement date alone 3 |
| shape changes that still fold into one file | 10 of 11 (the one that does not: the 2019 renaming of the 第9号 document kinds) |
| 附則 of the five laws | 1,391; 従前の例による 428; keyed on a transaction date 173; enforcement date only 960 |

The tables the corpus transcribes did not move: 別表第一 第1号 (0 in nine years), 所得税法
第89条 (0), 健康保険法 第40条 (0), 厚生年金保険法 第20条 (0, three scheduled). 措置法 第91条
changed five times, four of them the closing date alone.

## Reproducing

```console
$ python3 unpack.py                       # cache/<law>/<date>/<elm>.xml from the archive
$ python3 report.py 342AC0000000023 …     # the diffs and the heuristic kinds
$ python3 suppl.py                        # the transitional wording tally
$ python3 fusoku.py 332AC0000000026 '令和六年.*法律第八号$' '印紙税'   # one amending law's 附則
$ python3 finish.py                       # fetch whatever the cache lacks (network; e-Gov throttles)
```

`xt.py` strips the law XML to lines the same way `src/sources.rs` does; the heuristic kind in
`report.py` is only a first sort — the classification in §15.71 was read from the diffs.

## What the runs found

Both shapes check, cover and agree across the nine targets (`reports/*.log`). Four defects
came out of running them and were fixed the same day: the expansion of `apply` gave a renamed
output the identifier of the callee's own output column, so TypeScript, JavaScript, Go and
Swift declared it twice; the approver's page printed a date range as day numbers;
`rulec source fetch|outdated` compared bytes, so a revision that only re-marked the XML read
as an amendment (38 of the 103 differences above); and the customer page garbled a cell with
two bounds. One more thing the runs showed: e-Gov numbers a law's supplementary provisions in
document order, and that order shifts by a few between dates (the 2025 income tax act's sat at
349 as of 2025-12-01 and at 350 as of 2027-04-01), which is why `outdated` probes the
neighbouring positions before reading the whole law.
