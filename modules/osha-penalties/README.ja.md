# ritsu-osha-penalties

米国の労働安全衛生局（OSHA）が違反に提案できる制裁金の上限と、故意の違反の下限を、rulec の規則にしたものです。規則は 29 CFR 1903.15(d) を読みます。その節は eCFR が配ったままの形で規則の横に置き、規則が SHA-256 で固定しています。

| ファイル | 名前 | 持つもの | 節 |
|---|---|---|---|
| `osha_penalty.rule` | `osha_penalty` | 違反の種類ごとの、制裁金の上限と下限、違反ごとに数えるか日ごとに数えるか | 29 CFR 1903.15(d) |
| `osha_penalties.req` | `penalty_amounts` | 額を述べた一文、この版が従う改正、それぞれのリンクを読んだ人 | 29 CFR 1903.15(d) |

この版は 2024-02-01 時点の節を転記したもので、2024 年 1 月 15 日より後に提案する制裁金の額です。原文と見比べて読むためのもので、法律の助言ではありません。節が決めているのは提案できる範囲です。一件ごとの額は、事業の規模など 1903.15(b) の事情でも変わります。

## 使い方

モジュールを依存に足し、署名を確かめる行を `ritsu.mod` に書いて、取ってきます。

```console
$ ritsu mod add osha_penalties ghcr.io/i2y/ritsu-osha-penalties@1.1.0
```

```
signature "ghcr.io/i2y/ritsu-osha-penalties" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

```console
$ ritsu mod fetch
```

あとは、自分の kaname や dandori のコードから読みます。

```
use rule penalty from "@osha_penalties/osha_penalty.rule"
```

## 出典

`rulec source outdated <ファイル>` は、節を変える新しい改正が出ていないかを eCFR に問い合わせます。`sources/law/` の下のコピーは `THIRD_PARTY_NOTICES.md` に挙げています。

規則のライセンスは、ritsu と同じく MIT License か Apache License 2.0 のどちらかを選べます。

## 要件

`osha_penalties.req` は、節の額を一文で述べ、この版がどの改正に従うかを書き、節と文、文と規則を結びます。どのリンクにも、両端を読んだ人と日の行があり、`yuen review` が書いたものです。`reviewed/` には読んだときの両端の中身を残していて、そのあとで端が動けば `ritsu check` が止まります。モジュールのページと `ritsu mod diff` は、この言葉を新しい版の答えの横に出します。これはメンテナーが読んで決めたことで、ritsu が確かめるのはそのあとで動いていないことまでです。正しいかどうかは確かめません。

## 版

米国労働省は毎年 1 月に、この節を改正して額をインフレに合わせます。この改正に合わせた版はマイナーバージョンを上げるので、二つの版の差が、その改正で動いたものです。`ritsu mod diff ghcr.io/i2y/ritsu-osha-penalties@<旧> ghcr.io/i2y/ritsu-osha-penalties@<新>` で見られます。規則のファイルの名前を変える版や消す版、規則の入力や出力を変える版は、メジャーバージョンを上げます。

| 版 | この日より後に提案する制裁金 | 故意と繰り返しの上限 | ほかの上限 | 故意の下限 | 官報（Federal Register） |
|---|---|---|---|---|---|
| 1.0.0 | 2023 年 1 月 15 日 | 156,259 ドル | 15,625 ドル | 11,162 ドル | 88 FR 2217 |
| 1.1.0 | 2024 年 1 月 15 日 | 161,323 ドル | 16,131 ドル | 11,524 ドル | 89 FR 1817 |
