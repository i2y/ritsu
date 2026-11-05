# ritsu-japan-social-insurance

日本の健康保険料と厚生年金保険料を、ritsu のモジュールが使える rulec の規則にしたものです。どの規則にも英語の版と日本語の版があり、二つの答えは同じです。規則は報酬月額から等級の表で標準報酬月額を引き、料率を掛けて折半します。そこから、給与から控除する額と、現金で納める額を出します。二つは 1 円未満の端数の扱いが違います。

| 英語の版 | 日本語の版 | 保険料 | 表 |
|---|---|---|---|
| `health_insurance_premium.rule` | `健康保険料.rule` | 協会けんぽ（全国健康保険協会）の健康保険 | 協会の保険料額表、令和 8 年 3 月分（4 月納付分）から、東京支部 |
| `pension_insurance_premium.rule` | `厚生年金保険料.rule` | 厚生年金保険 | 日本年金機構の保険料額表、令和 2 年 9 月分（10 月納付分）から、令和 8 年度版（固定あり） |

料率は入力です。健康保険料率は都道府県と年度で変わり、厚生年金は基金によって違うので、規則には持たせていません。当てはまる料率を渡してください。規則は表を書き写したもので、原文と見比べて読むためのものです。法律や税の助言ではありません。

## 使い方

モジュールを依存に足し、署名を確かめる行を `ritsu.mod` に書いて、取ってきます。

```console
$ ritsu mod add japan_social_insurance ghcr.io/i2y/ritsu-japan-social-insurance@1.0.0
```

```
signature "ghcr.io/i2y/ritsu-japan-social-insurance" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

```console
$ ritsu mod fetch
```

あとは、自分の kaname や dandori のコードから読みます。

```
use rule pension from "@japan_social_insurance/pension_insurance_premium.rule"
```

## 出典

`sources/nenkin/` のコピーの出どころは `THIRD_PARTY_NOTICES.md` に書きました。健康保険料の規則は、表の名前をコメントに書いていて、コピーは持ちません。

規則のファイルは、ritsu と同じく、MIT License か Apache License 2.0 の好きなほうで使えます。

## 版

法令の変更に合わせる版は、マイナーの版を上げます。ファイルの名前を変える版、ファイルを消す版、規則の入力や出力を変える版は、メジャーの版を上げます。
