# Third-party notices

The examples and the tests carry copies of public data, kept as their sources served them. Each
keeps the terms of its source, as below. What `yuen doc` and `yuen trace` show from a copy (the
text of an article, quoted on a page or under a requirement) is made from these copies, and says
which copy, as of which date and which version.

The other files the examples and the tests copy from the rest of ritsu (the rules `.rule`, the
calendars `.cal`, the books `.book`, the specs `.geas` and the server `server.py`, the workflows
`.flow`, the maps `.ctx`, the `.proto` files) are by the same author, under the same license as
yuen (MIT OR Apache-2.0). Each example's `README.md` says where in this repository its files were
copied from (`crates/rulec/tests/corpus/`, `crates/koyomi/examples/`, `crates/chobo/examples/`,
`crates/geas/examples/`). ReqIF's schemas, which the tests validate the ReqIF export against, are
not in the repository: `tools/reqif/fetch.sh` fetches them.

## The national holidays of Japan, from the Cabinet Office

`calendars/data/syukujitsu.csv` under [examples/payment_terms](examples/payment_terms),
[examples/civil_code_periods](examples/civil_code_periods) and
[examples/civil_code_periods_reread](examples/civil_code_periods_reread), and under the fixtures
`tests/fixtures/koyomi` and `tests/fixtures/calendar_sources`, is, byte for byte, the file
「昭和30年（1955年）から令和9年（2027年）国民の祝日（csv形式）」
(<https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv>) that the Cabinet Office of Japan
publishes on its page 「国民の祝日」について (<https://www8.cao.go.jp/chosei/shukujitsu/gaiyou.html>),
as koyomi's examples keep it (taken on 2026-10-02). Its SHA-256 is
`cec37a743c96995cdb9cb52b685c9003634682a9b0e1a640a6b9b96881fe964a`; it is in Shift_JIS, as served.

The Cabinet Office's terms of use for its site (内閣府ホームページ利用規約,
<https://www.cao.go.jp/notice/rule.html>) put its content under the Public Data License, version
1.0 (公共データ利用規約（第1.0版）, PDL1.0,
<https://www.digital.go.jp/resources/open_data/public_data_license_v1.0>), which lets anyone copy,
adapt and use the content, commercially too, and asks that the source be given, in the form of
its examples:

```
出典：内閣府ホームページ「「国民の祝日」について」（https://www8.cao.go.jp/chosei/shukujitsu/gaiyou.html）の「昭和30年（1955年）から令和9年（2027年）国民の祝日（csv形式）」（https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv）、PDL1.0（https://www.digital.go.jp/resources/open_data/public_data_license_v1.0）（2026年10月2日に取得）
```

PDL1.0 also asks that content which was edited or processed say so, and by whom, apart from the
source, and that it not be presented as if the government had made it as it is. The copies here
are not edited.

## The bank holidays of the United Kingdom, from GOV.UK

`tests/fixtures/calendar_sources/calendars/data/bank-holidays.json` is, byte for byte,
<https://www.gov.uk/bank-holidays.json>, as koyomi's examples keep it (taken on 2026-10-02).

GOV.UK says that all its content is available under the Open Government Licence v3.0, except
where otherwise stated
(<https://www.nationalarchives.gov.uk/doc/open-government-licence/version/3/>). The licence asks
that the source be acknowledged with the attribution statement the provider gives, or else with
this one, and, where possible, a link to the licence:

```
Contains public sector information licensed under the Open Government Licence v3.0.
```

## Articles of laws of Japan, from e-Gov

As e-Gov's law API, version 2, served them (`law_data`, one element at a time), with e-Gov's
version of each law in `revision.txt` beside the copies:

- the Civil Code (民法, 明治二十九年法律第八十九号, law ID `129AC0000000089`), Articles 140 to 143,
  as of 2026-10-01, in `sources/law/129AC0000000089@2026-10-01/` under
  [examples/civil_code_periods](examples/civil_code_periods),
  [examples/civil_code_periods_reread](examples/civil_code_periods_reread), the fixtures
  `tests/fixtures/period` and `tests/fixtures/koyomi`, and the Japanese mutants under
  `tests/mutants/` (as koyomi's examples keep them, taken on 2026-10-02);
- the Stamp Tax Act (印紙税法, 昭和四十二年法律第二十三号, law ID `342AC0000000023`), Appended Table 1
  (別表第一), and the Act on Special Measures Concerning Taxation (租税特別措置法,
  昭和三十二年法律第二十六号, law ID `332AC0000000026`), Article 91, as of 2026-04-01, in
  `sources/law/` under [examples/stamp_tax](examples/stamp_tax) and the fixture
  `tests/fixtures/rulec` (as rulec's test corpus keeps them).

The terms of use of the e-Gov Law Search (e-Gov 法令検索 利用規約, <https://laws.e-gov.go.jp/terms/>)
put its content under PDL1.0 unless stated otherwise, and ask that the source be given, in the
form of their examples:

```
出典：e-Gov法令検索（https://laws.e-gov.go.jp/）の「民法」（明治二十九年法律第八十九号）第百四十条から第百四十三条（2026年10月1日時点、版 129AC0000000089_20260624_508AC0000000045）、法令API Version2 で取得、PDL1.0
出典：e-Gov法令検索（https://laws.e-gov.go.jp/）の「印紙税法」（昭和四十二年法律第二十三号）別表第一、「租税特別措置法」（昭和三十二年法律第二十六号）第九十一条（2026年4月1日時点）、法令API Version2 で取得、PDL1.0
```

They also ask that content which was edited or processed say so, apart from the source, and that
it not be presented as if the government had made it. The copies here are not edited, but for
the mutants that change one character of an article on purpose to show a diagnostic (their
directory names say what they change); the pages `yuen doc` writes quote the text of an article
from its copy with the XML tags removed, and say which copy, as of which date and which version.

## Sections of the Code of Federal Regulations, from the eCFR

As the eCFR's versioner API served them:

- 29 CFR 1910.157 (Portable fire extinguishers) as of 2026-01-01, in
  `sources/law/29-CFR-1910@2026-01-01/1910.157.xml` under [examples/osha](examples/osha) and the
  fixtures `tests/fixtures/ecfr` and `tests/fixtures/rulec` (as rulec's test corpus keeps it);
- 37 CFR 1.6, 1.7, 1.8, 1.10, 1.17 and 1.27 as of 2026-01-01, in
  `sources/law/37-CFR-1@2026-01-01/` under the fixtures `tests/fixtures/period_of_months` and
  `tests/fixtures/fee_rules`, and the English mutants under `tests/mutants/` (fetched with
  `yuen source fetch`).

The Code of Federal Regulations is a work of the United States government and is not subject to
copyright in the United States (17 U.S.C. 105). The eCFR is published by the Office of the Federal
Register and the Government Publishing Office (<https://www.ecfr.gov/>); it is an editorial
compilation, not an official legal edition of the CFR. The copies here are not edited, but for the
mutants that change a word of a section on purpose.
