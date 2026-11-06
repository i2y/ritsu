# ritsu-uk-rates

英国の所得税（Income Tax）、不動産取得の印紙土地税（Stamp Duty Land Tax、SDLT）、最低賃金を、ritsu のモジュールが使える rulec の規則にしたものです。どの規則も GOV.UK のページを読み、そのページを横に置いて SHA-256 で固定しています。

| ファイル | 規則 | 答えること | 時点 |
|---|---|---|---|
| `uk_income_tax.rule` | `uk_income_tax` | 基礎控除（Personal Allowance）と、所得が入る税率の帯。イングランド、ウェールズ、北アイルランド | 2026 年 4 月 6 日から 2027 年 4 月 5 日までの税年度 |
| `uk_stamp_duty.rule` | `uk_stamp_duty` | イングランドと北アイルランドの住宅の購入で、価格が入る SDLT の帯。初めて買う人の軽減と、二軒目の上乗せも | 2026 年 9 月 21 日のページ |
| `uk_minimum_wage.rule` | `uk_minimum_wage` | 年齢ごとの最低時給と、見習いの時給 | 2026 年 4 月 1 日から |

規則はページを転記したもので、原文と見比べて読むためのものです。税や法律の助言ではありません。税額は帯ごとに分けて計算し、ページに合計の表が無いので、一つ目は税額ではなく所得の入る帯を、二つ目は税額ではなく価格の入る帯を答えます。

## 使い方

モジュールを依存に足し、署名を確かめる行を `ritsu.mod` に書いて、取ってきます。

```console
$ ritsu mod add uk_rates ghcr.io/i2y/ritsu-uk-rates@1.0.1
```

```
signature "ghcr.io/i2y/ritsu-uk-rates" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

```console
$ ritsu mod fetch
```

あとは、自分の kaname や dandori のコードから読みます。

```
use rule income_tax from "@uk_rates/uk_income_tax.rule"
```

## 出典

`sources/` のコピーは 2026 年 9 月 21 日に取ったもので、一覧は `THIRD_PARTY_NOTICES.md` にあります。Contains public sector information licensed under the Open Government Licence v3.0.

規則のファイルは、ritsu と同じく、MIT License か Apache License 2.0 の好きなほうで使えます。

## 版

法令の変更に合わせる版は、マイナーの版を上げます。ファイルの名前を変える版、ファイルを消す版、規則の入力や出力を変える版は、メジャーの版を上げます。
この README の言葉だけを直す版は、パッチの版を上げます。
