# Third-party notices

sakai itself carries no third-party code; its one outside dependency is serde_json, under MIT OR Apache-2.0.
The examples carry one copy of public data, which koyomi's calendars in them read, kept as its source served it.
It keeps the terms of its source, as below.
Every other file of the examples is ritsu's own, under the license of the repository: the rules, workflows, calendars, book and `.proto` files copied from the examples of ritsu's languages, and what was written for these examples.

## The national holidays of Japan, from the Cabinet Office

[examples/shop/calendars/data/syukujitsu.csv](examples/shop/calendars/data/syukujitsu.csv) and [examples/shop.ja/calendars/data/syukujitsu.csv](examples/shop.ja/calendars/data/syukujitsu.csv) are, byte for byte, the file 「昭和30年（1955年）から令和9年（2027年）国民の祝日（csv形式）」 (<https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv>) that the Cabinet Office of Japan publishes on its page 「国民の祝日」について (<https://www8.cao.go.jp/chosei/shukujitsu/gaiyou.html>), as koyomi's examples copied it on 2026-10-02.
Its SHA-256 is `cec37a743c96995cdb9cb52b685c9003634682a9b0e1a640a6b9b96881fe964a`; it is in Shift_JIS, as served.

The Cabinet Office's terms of use for its site (内閣府ホームページ利用規約, as of 2025-03-25, <https://www.cao.go.jp/notice/rule.html>) put its content under the Public Data License, version 1.0 (公共データ利用規約（第1.0版）, PDL1.0, <https://www.digital.go.jp/resources/open_data/public_data_license_v1.0>), which lets anyone copy, adapt and use the content, commercially too, and asks that the source be given, in the form of its examples:

```
出典：内閣府ホームページ「「国民の祝日」について」（https://www8.cao.go.jp/chosei/shukujitsu/gaiyou.html）の「昭和30年（1955年）から令和9年（2027年）国民の祝日（csv形式）」（https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv）、PDL1.0（https://www.digital.go.jp/resources/open_data/public_data_license_v1.0）（2026年10月2日に取得）
```

PDL1.0 also asks that content which was edited or processed say so, and by whom, apart from the source, and that it not be presented as if the government had made it as it is.
The copies here are not edited.
