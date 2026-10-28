# ritsu-holidays

英国と日本の祝日を、ritsu のモジュールが使える koyomi のカレンダーにしたものです。どのカレンダーも、祝日だけを休みにします。ほかに何曜日を休むか（土曜と日曜、年末年始など）は、このカレンダーを読むカレンダーの側で書いてください。

| ファイル | カレンダー | 休みにする日 | データで分かる範囲 |
|---|---|---|---|
| `uk/england_and_wales.cal` | `england_and_wales` | イングランドとウェールズのバンクホリデー | 2019-01-01〜2028-12-31 |
| `uk/scotland.cal` | `scotland` | スコットランドのバンクホリデー | 2019-01-01〜2028-12-31 |
| `uk/northern_ireland.cal` | `northern_ireland` | 北アイルランドのバンクホリデー | 2019-01-01〜2028-12-31 |
| `japan/national_holidays.cal` | `national_holidays` | 国民の祝日と、国民の祝日に関する法律に規定する休日 | 1955-01-01〜2027-12-31 |
| `japan/国民の祝日.cal` | `国民の祝日` | 同じもの（日本語の版） | 1955-01-01〜2027-12-31 |

英国のカレンダーにはオフセットがありません。三つの地域には夏時間があるので、日付だけを決めます。日本のカレンダーのオフセットは `+09:00` です。データが知っている範囲の外の日が祝日かどうかは分からないので、koyomi は推し量らずに、分からないと言います。

## 使い方

モジュールを依存に足し、署名を確かめる行を書きます。

```console
$ ritsu mod add holidays ghcr.io/i2y/ritsu-holidays@1.0.0
```

```
require holidays oci "ghcr.io/i2y/ritsu-holidays" 1.0.0
signature "ghcr.io/i2y/ritsu-holidays" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

自分のカレンダーでこのカレンダーを読み、その上に自分の休みを足します。

```cal
calendar 事務所(office) v1
description "東京の事務所。土日と国民の祝日を休む"
offset +09:00
use calendar "@holidays/japan/国民の祝日.cal"

closed weekly sat, sun
```

## データ

データのファイルは、出している先が配るファイルとバイトまで同じで、カレンダーが SHA-256 で固定しています。どこから、いつ、どの条件で取ったかは `THIRD_PARTY_NOTICES.md` に書きました。

- `uk/data/bank-holidays.json`：GOV.UK のバンクホリデー（<https://www.gov.uk/bank-holidays.json>）。Open Government Licence v3.0 で使えます。Contains public sector information licensed under the Open Government Licence v3.0.
- `japan/data/syukujitsu.csv`：内閣府の国民の祝日の表（<https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv>）。配られたままの Shift_JIS です。公共データ利用規約（第1.0版）（PDL1.0）で使えます。

カレンダーのファイルは、ritsu と同じく、MIT License か Apache License 2.0 の好きなほうで使えます。

## 版

新しい年の祝日を入れる版は、マイナーの版を上げます。ファイルの名前を変える版と、ファイルを消す版は、メジャーの版を上げます。
