# ritsu-japan-tax

日本の所得税と印紙税を、ritsu のモジュールが使える rulec の規則にしたものです。どの規則にも英語の版と日本語の版があり、二つの答えは同じです。

| 英語の版 | 日本語の版 | 答えること | 出典 |
|---|---|---|---|
| `japan_income_tax.rule` | `所得税.rule` | 課税所得に対する所得税。速算表で計算し、2.1% の上乗せも出す | 国税庁タックスアンサー No.2260（令和 8 年 4 月 1 日現在法令等） |
| `japan_stamp_duty.rule` | `印紙税.rule` | 不動産の譲渡に関する契約書（第 1 号文書）の印紙税。令和 9 年 3 月 31 日までに作成したものの軽減税率も | 印紙税法の別表第一と租税特別措置法の第 91 条（2026-04-01 時点、固定あり） |
| `receipt_stamp_duty.rule` | `領収書の印紙税.rule` | 売上代金の受取書（第 17 号の 1 文書）の印紙税 | 国税庁タックスアンサー No.7141（令和 8 年 4 月 1 日現在法令等） |

規則は出典を転記したもので、原文と見比べて読むためのものです。税の助言ではありません。令和 8 年度の税制改正で、所得税の上乗せの内訳は 2027 年分から変わります。規則が読んでいるのは、令和 8 年 4 月 1 日時点のページです。

## 使い方

モジュールを依存に足し、署名を確かめる行を `ritsu.mod` に書いて、取ってきます。

```console
$ ritsu mod add japan_tax ghcr.io/i2y/ritsu-japan-tax@1.0.1
```

```
signature "ghcr.io/i2y/ritsu-japan-tax" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

```console
$ ritsu mod fetch
```

あとは、自分の kaname や dandori のコードから読みます。

```
use rule 印紙税 from "@japan_tax/印紙税.rule"
```

## 出典

`rulec source outdated <ファイル>` で、印紙税の規則が固定している条がその後に改正されたかを e-Gov に問い合わせられます。`sources/law/` のコピーの一覧は `THIRD_PARTY_NOTICES.md` にあります。タックスアンサーを読む規則は、そのページの名前をコメントに書いていて、コピーは持ちません。

規則のファイルは、ritsu と同じく、MIT License か Apache License 2.0 の好きなほうで使えます。

## 版

法令の変更に合わせる版は、マイナーの版を上げます。ファイルの名前を変える版、ファイルを消す版、規則の入力や出力を変える版は、メジャーの版を上げます。
この README の言葉だけを直す版は、パッチの版を上げます。
