# ritsu-late-payment

英国の事業者間の請求書の支払いが遅れたとき、売り手が請求できるものを、ritsu のモジュールにしたものです。根拠は Late Payment of Commercial Debts (Interest) Act 1998 です。規則と暦と、それをつなぐ kaname のコードがあり、どれにも英語の版と日本語の版があって、二つの答えは同じです。

| 英語の版 | 日本語の版 | 答えること |
|---|---|---|
| `dates/payment_terms.cal` | `dates/支払期限.cal` | 支払いの日を決めていないときの期日。買い手が請求書を受けた日から数えて 30 日の、最後の日（第 4 条） |
| `rules/late_payment_charges.rule` | `rules/遅延の請求額.rule` | 取り立ての費用としての定額（第 5A 条）と、基準金利に 8% を足した率で一日ずつ数える利息 |
| `late_payment.kn` | `late_payment.ja.kn` | 請求書一通の請求の全体。期日、遅れた日数、定額、利息。期日までに払われていれば何も無し |

基準金利は入力です。当てはまるイングランド銀行の政策金利を渡してください。規則は 2026 年 10 月 18 日時点の法律と GOV.UK の案内を読んだもので、原文と見比べて読むためのものです。法律の助言ではありません。1 ペニー未満の端数をどちらに丸めるかは案内に書かれていないので、規則のコメントにそう書き、仮に四捨五入としました。

## 使い方

モジュールを依存に足し、署名を確かめる行を `ritsu.mod` に書いて、取ってきます。

```console
$ ritsu mod add late_payment ghcr.io/i2y/ritsu-late-payment@1.0.0
```

```
signature "ghcr.io/i2y/ritsu-late-payment" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

```console
$ ritsu mod fetch
```

あとは、自分の kaname や dandori のコードから読みます。

```
use rule charges from "@late_payment/rules/late_payment_charges.rule"
use dates terms from "@late_payment/dates/payment_terms.cal"
```

## 出典

`rules/sources/` のコピーの一覧は `THIRD_PARTY_NOTICES.md` にあります。Contains public sector information licensed under the Open Government Licence v3.0.

規則と暦とコードは、ritsu と同じく、MIT License か Apache License 2.0 の好きなほうで使えます。

## 版

法令の変更に合わせる版は、マイナーの版を上げます。ファイルの名前を変える版、ファイルを消す版、規則の入力や出力を変える版は、メジャーの版を上げます。
