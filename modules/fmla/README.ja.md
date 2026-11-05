# ritsu-fmla

米国の家族・医療休暇法（FMLA）を、その規則である 29 CFR Part 825 のとおりに、ritsu の五つの言語で書いたものです。どのファイルも同じ休暇の一部分に答え、`ritsu check` がそれをつなげて確かめます。フローは、規則と期限と帳簿を、それぞれが宣言した型で呼びます。

| ファイル | 言語 | 持つもの | 節 |
|---|---|---|---|
| `fmla_eligibility.rule` | rulec | 従業員に休暇を取る資格があるか、無ければ最初の理由 | 825.110(a) |
| `fmla_entitlement.rule` | rulec | 理由ごとに取れる週数と、それが本人の 1 週間の働き方で何分にあたるか | 825.200、825.205(b) |
| `business_days.cal`、`fmla_deadlines.cal` | koyomi | 各通知と診断書の期限の日。祝日はモジュール `holidays` の連邦の祝日 | 825.300(b)、(d)、825.305(b) |
| `fmla_leave.book` | chobo | 12 か月の期間に残っている休暇。ゼロを下回らず、指定した分は取るまで押さえておく | 825.200、825.205 |
| `fmla_request.flow` | dandori | 申し出から休暇が終わるまで。使用者が出す通知、診断書、休暇の指定 | 825.300、825.305 |
| `fmla_records.gate` | sekisho | 医療の記録からだれが何を知ってよいか、だれが医療者に連絡してよいか | 825.500(g)、825.307(a) |

2026-01-01 時点の節を転記したもので、原文と見比べて読むためのものです。法律の助言ではありません。

## ここで決めて読んだところ

節が決めていないところは、どう決めたかを各ファイルのコメントに書きました。

- 営業日は、月曜から金曜のうち連邦の祝日を除いた日です。Part 825 は営業日を定めておらず、使用者によって違うことがあります。
- 期限は時刻ではなく日です。節は時刻を言わず、使用者は米国のどのタイムゾーンにもいます。また「やむを得ない事情」が無いときの日です。
- フローは、申し出のあった日に診断書を求めます。825.305(b) が認める日のうち、いちばん早い日です。
- 825.500(g) は、記録をだれが持つかを言っていません。ゲートでは、休暇の担当者と人事の専門職が読みます。

## 入っていないもの

- 使用者が法の対象か（825.104）、病状が「重い健康状態」にあたるか（825.113）。
- 航空機の乗務員（825.801〜825.803）、米国の外で働く従業員（825.105(b)）、学校の従業員の特則（825.600〜825.604）。
- 同じ使用者に勤める配偶者が上限を分け合う場合（825.120(a)(3)、825.127(f)）。
- 825.200(g) の合計の上限。帳簿は一つの期間に一つの上限を持ちます。825.200(f) の、対象となる軍人を介護するための 12 か月は、使用者が別の期間として名前を付けます。
- 825.200(b)(4) の、さかのぼって数える 12 か月。毎日動くので、別の形の帳簿が要ります。
- 有給休暇への振り替え（825.207）と、州の休暇法。
- 従業員が自分の記録をどこまで見られるか。この項は言っていません。

## 使い方

モジュールを依存に足し、このモジュールと `holidays` の署名を確かめる行を `ritsu.mod` に書いて、取ってきます。

```console
$ ritsu mod add fmla ghcr.io/i2y/ritsu-fmla@1.0.0
```

```
signature "ghcr.io/i2y/ritsu-fmla" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
signature "ghcr.io/i2y/ritsu-holidays" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

```console
$ ritsu mod fetch
```

あとは、自分のファイルから読みます。

```
use rule eligibility from "@fmla/fmla_eligibility.rule"
use dates deadlines from "@fmla/fmla_deadlines.cal"
use book leave from "@fmla/fmla_leave.book"
```

フローのキューを通るタスクは Temporal で動きます。`ritsu gen --target temporal-typescript`（`temporal-python`、`temporal-go` も）で書き出せます。ゲートの決まりは `ritsu gen --target cedar` で Cedar になります。

## 出典

`sources/law/` に、eCFR が配ったままの節を置いています。規則は読む三つの節を SHA-256 で固定し、`rulec source outdated <ファイル>` が新しい改正が出ていないかを eCFR に問い合わせます。koyomi、chobo、dandori、sekisho のファイルには eCFR の節を固定する行がありません。コメントに節の名前を書き、横のコピーがそのとき読んだものです。一覧は `THIRD_PARTY_NOTICES.md` にあります。

ライセンスは、ritsu と同じく MIT License か Apache License 2.0 のどちらかを選べます。

## 版

法令の改正に合わせた版はマイナーバージョンを上げます。言語のファイルの名前を変える版や消す版、ファイルが受け取るものや返すものを変える版は、メジャーバージョンを上げます。
