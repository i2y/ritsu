# ritsu-us-income-tax

米国の連邦所得税のうち、独身の個人の税額を、ritsu のモジュールが使える rulec の規則にしたものです。課税年度ごとに規則が一本あります。どの規則も内国歳入庁（IRS）の税率表を読み、その表を横に置いて SHA-256 で固定しています。

| ファイル | 規則 | 課税年度の始まり | 表 |
|---|---|---|---|
| `us_income_tax_2026.rule` | `us_income_tax_2026` | 2026 年 | Rev. Proc. 2025-32 の 4.01 節、Table 3 |
| `us_income_tax_2025.rule` | `us_income_tax_2025` | 2025 年 | Rev. Proc. 2024-40 の 3.01 節、Table 3 |

表は Section 1(j)(2)(C) のもので、配偶者と死別した人と世帯主を除く、未婚の個人が対象です。規則は課税所得をセント単位で受け取り、税額を答えます。課税所得の計算は含みません。規則は表を転記したもので、原文と見比べて読むためのものです。税の助言ではありません。

## 使い方

モジュールを依存に足し、署名を確かめる行を `ritsu.mod` に書いて、取ってきます。

```console
$ ritsu mod add us_income_tax ghcr.io/i2y/ritsu-us-income-tax@1.0.1
```

```
signature "ghcr.io/i2y/ritsu-us-income-tax" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

```console
$ ritsu mod fetch
```

あとは、自分の kaname や dandori のコードから読みます。

```
use rule tax from "@us_income_tax/us_income_tax_2026.rule"
```

## 出典

`sources/` のコピーの一覧は `THIRD_PARTY_NOTICES.md` にあります。

規則のファイルは、ritsu と同じく、MIT License か Apache License 2.0 の好きなほうで使えます。

## 版

新しい課税年度の規則を足す版は、マイナーの版を上げます。ファイルの名前を変える版、ファイルを消す版、規則の入力や出力を変える版は、メジャーの版を上げます。
この README の言葉だけを直す版は、パッチの版を上げます。
