# ritsu-osha

米国の労働安全基準の上限と条件を、ritsu のモジュールが使える rulec の規則にしたものです。どの規則も、連邦規則集（CFR）第 29 編の一つの節を読みます。その節は eCFR が配ったままの形で規則の横に置き、規則が SHA-256 で固定しています。

| ファイル | 規則 | 答えること | 節 |
|---|---|---|---|
| `osha_extinguisher.rule` | `osha_extinguisher` | 持ち運び式の消火器まで、従業員が歩く距離の上限 | 29 CFR 1910.157(d) |
| `osha_noise.rule` | `osha_noise` | 連続する騒音の大きさごとの、許される時間 | 29 CFR 1910.95 の Table G-16 |
| `osha_excavation.rule` | `osha_excavation` | 掘削に崩落を防ぐ設備が要るか | 29 CFR 1926.652(a)(1) |

規則は 2026-01-01 時点の節を転記したもので、原文と見比べて読むためのものです。法律の助言ではありません。節が決めていないところは、どう決めたかを規則のコメントに書きました。たとえば `osha_noise.rule` は、表の二つの値のあいだの騒音に、短いほうの時間を当てます。

## 使い方

モジュールを依存に足し、署名を確かめる行を `ritsu.mod` に書いて、取ってきます。

```console
$ ritsu mod add osha ghcr.io/i2y/ritsu-osha@1.0.1
```

```
signature "ghcr.io/i2y/ritsu-osha" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

```console
$ ritsu mod fetch
```

あとは、自分の kaname や dandori のコードから読みます。

```
use rule noise from "@osha/osha_noise.rule"
```

## 出典

`rulec source outdated <ファイル>` で、節がその後に改正されたかを eCFR に問い合わせられます。`sources/law/` のコピーの一覧は `THIRD_PARTY_NOTICES.md` にあります。

規則のファイルは、ritsu と同じく、MIT License か Apache License 2.0 の好きなほうで使えます。

## 版

法令の変更に合わせる版は、マイナーの版を上げます。ファイルの名前を変える版、ファイルを消す版、規則の入力や出力を変える版は、メジャーの版を上げます。
この README の言葉だけを直す版は、パッチの版を上げます。
