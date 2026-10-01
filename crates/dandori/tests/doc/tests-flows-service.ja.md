# 出荷の受付 v1

サービスを実装する（どのプラットフォームでも）：protos/shipping.proto のサービスの入口で、入力とコールバックの応答に、protobuf の JSON が省くゼロ値を埋めてから読む。箱のリストの中の品のリスト、中のメッセージ、ゼロ値が値でもある列挙を通る。値が設定されていない google.protobuf.Value は省かれるので、json の入力は無いことがあり、無ければ null として読む。出力は無く、実行は {} で終わる（protobuf の JSON を読む側は、null からメッセージを読まない）

`tests/flows/service.flow` を `dandori doc` で描いたものです。入力は `注文ID: string`, `箱: list[箱]`, `宛先: 宛先`, `区分: 受付.Kind`, `付帯: json` です。このワークフローは `shipping.v1.ShippingService`（`protos/shipping.proto`）を実装します。

## flow

```mermaid
flowchart TD
    start(["出荷の受付 v1"])
    subgraph L1 ["for 一箱 in 箱 at most 3"]
        subgraph L2 ["for 一品 in 一箱.品 at most 3"]
            s3["記録する(…)<br>lambda shipping-log"]
        end
    end
    s4[/"済み = 梱包を待つ(…)<br>lambda packing · callback<br>timeout 1 day"/]
    s5(["fail 梱包の遅れ<br>#quot;一日たっても梱包の知らせがありません#quot;"])
    s6{{"match 済み.検品済み"}}
    s8(["fail 検品漏れ<br>#quot;注文 {注文ID} の検品が済んでいません#quot;"])
    s9{{"match 区分"}}
    s10["知らせる(…)<br>lambda shipping-notice"]
    s11(["succeed"])
    fin(["終わり（成功）"])
    start --> s3
    s3 -->|"次のイテレーション"| s3
    L2 -->|"最後の項目のあと"| s3
    L1 -->|"最後の項目のあと"| s4
    s4 -.->|"on timeout"| s5
    s4 --> s6
    s6 -->|"false"| s8
    s6 -->|"true"| s9
    s9 -->|"normal"| s10
    s9 -->|"express"| s11
    s10 --> fin
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s11 ok
    class s5,s8 bad
```

四角はタスク、両脇に線のある四角は規則、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。

## サービス

サービスのメソッドと、それぞれが実行に対して何をするかです。

| メソッド | 何をするか | タスク |
|---|---|---|
| `Ship` | 実行を始めます。失敗の名前は `梱包の遅れ`・`検品漏れ` | — |
| `AnswerPacking` | コールバックに応答します | `梱包を待つ` |

## 呼び出し

| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき |
|---:|---|---|---|---|---|
| 48 | `記録する(…)` | `lambda shipping-log`, `idempotent` | — | — | `timeout`, `failure` → ワークフローが失敗する |
| 49 | `済み = 梱包を待つ(…)` | `lambda packing · callback` | — | 1 日 | `timeout` → 50 行目<br>`failure` → ワークフローが失敗する |
| 55 | `知らせる(…)` | `lambda shipping-notice`, `idempotent` | — | — | `timeout`, `failure` → ワークフローが失敗する |

## 終わり方

ワークフローの終わり方のすべてです。

| 行 | 終わり方 |
|---:|---|
| 50 | `fail 梱包の遅れ` "一日たっても梱包の知らせがありません" |
| 53 | `fail 検品漏れ` "注文 {注文ID} の検品が済んでいません" |
| 56 | `succeed` |
| 56 | flow が最後まで走り、ワークフローは成功する |

