# 取消 v1

キャンセルのエッジケース：案件を片付ける on cancel、リトライとエラーの処理を素通りするキャンセル、並列のイテレーションの中のキャンセル、コールバックを待つあいだのキャンセル、on failure の最中のキャンセル。Temporal 向け

`tests/flows/cancel.flow` を `dandori doc` で描いたものです。入力は `注文ID: string`, `品目: list[string]` です。

## flow

```mermaid
flowchart TD
    start(["取消 v1"])
    s1["受注 ← 注文を見る(…)<br>自分で書くタスク<br>observes"]
    subgraph L2 ["let 引当 = for 一品 in 品目 at most 3 in parallel · yield r.id"]
        s3["r = 引き当てる(…)<br>自分で書くタスク<br>retry 1 times every 1 second"]
        s4(["fail NotReserved<br>#quot;{一品} を引き当てられませんでした#quot;<br>leaving 受注"])
    end
    s5[/"受注 ← 配達を待つ(…)<br>自分で書くタスク（応答はコールバック）<br>observes · timeout 7 days"/]
    s6(["fail DeliveryLate<br>#quot;七日たっても配達の知らせがありません#quot;<br>leaving 受注"])
    s7{{"match 受注.状態"}}
    s9(["fail NotDelivered<br>#quot;配達されていません#quot;<br>leaving 受注"])
    fin(["終わり（成功）"])
    start --> s1
    s1 --> s3
    s3 -.->|"on failure"| s4
    L2 -->|"すべてのイテレーションが終わったら"| s5
    s5 -.->|"on timeout"| s6
    s5 --> s7
    s7 -->|"受付, 入金済, 出荷済"| s9
    s7 -->|"配達済, 取消"| fin
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s4,s6,s9 bad
```

四角はタスク、両脇に線のある四角は規則、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。

## on failure

呼び出しが失敗し、そのエラーをその場で処理しないときに走ります（46・51 行目の呼び出しから）。最後まで走ると、ワークフローはそのエラーで失敗します。

```mermaid
flowchart TD
    onf(["on failure"])
    s10["知らせる(…)<br>自分で書くタスク"]
    s12(["fail Stopped<br>#quot;途中で止まりました#quot;<br>leaving 受注"])
    onf --> s10
    s10 --> s12
    s10 -.->|"on failure"| s12
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s12 bad
```

## on cancel

ワークフローがキャンセルされると、そのとき待っている呼び出しや wait から、ここに来ます。最後まで走ると、ワークフローはキャンセルで終わります。

```mermaid
flowchart TD
    onc(["on cancel"])
    s13{{"match 受注.状態"}}
    s15["受注 ← 取消を頼む(…)<br>自分で書くタスク<br>sends 取消依頼"]
    s17(["fail CancelFailed<br>#quot;注文 {受注.id} を取り消せませんでした#quot;<br>leaving 受注"])
    s18["知らせる(…)<br>自分で書くタスク"]
    s20(["fail ShippedAlready<br>#quot;注文 {受注.id} はもう出荷されています#quot;<br>leaving 受注"])
    oncEnd(["キャンセルで終わる"])
    onc --> s13
    s13 -->|"受付, 入金済"| s15
    s15 -.->|"on failure"| s17
    s15 --> s18
    s15 -.->|"on 断られた"| s18
    s13 -->|"出荷済"| s20
    s13 -->|"none"| oncEnd
    s18 --> oncEnd
    s18 -.->|"on failure"| oncEnd
    s13 -->|"配達済, 取消"| oncEnd
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s17,s20 bad
```

## 呼び出し

| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき | 呼び出しのあとの案件 |
|---:|---|---|---|---|---|---|
| 46 | `受注 ← 注文を見る(…)` | 自分で書くタスク, `observes`, `idempotent` | — | — | `timeout`, `failure` → `on failure` | `受注`: `受付`, `入金済`, `出荷済`, `配達済`, `取消` |
| 48 | `r = 引き当てる(…)` | 自分で書くタスク, `idempotent` | 1 秒おきに 1 回（failure, timeout） | — | `timeout`, `failure` → 49 行目 | — |
| 51 | `受注 ← 配達を待つ(…)` | 自分で書くタスク（応答はコールバック）, `observes` | — | 7 日 | `timeout` → 52 行目<br>`failure` → `on failure` | `受注`: `受付`, `入金済`, `出荷済`, `配達済`, `取消` |
| 58 | `知らせる(…)` | 自分で書くタスク, `key` | — | — | `timeout`, `failure` → 59 行目 | — |
| 67 | `受注 ← 取消を頼む(…)` | 自分で書くタスク, `sends 取消依頼`, `key` | — | — | `断られた` → 68 行目<br>`timeout`, `failure` → 69 行目 | `受注`: `取消` |
| 70 | `知らせる(…)` | 自分で書くタスク, `key` | — | — | `timeout`, `failure` → 71 行目 | — |

## 終わり方

ワークフローの終わり方のすべてと、そのとき各案件がとりうる状態です。外部のサービスで起きるイベントも含めています。

| 行 | 終わり方 | `受注` |
|---:|---|---|
| 49 | `fail NotReserved` "{一品} を引き当てられませんでした" `leaving 受注` | そのまま引き渡す: `受付`, `入金済`, `出荷済`, `配達済`, `取消` |
| 52 | `fail DeliveryLate` "七日たっても配達の知らせがありません" `leaving 受注` | そのまま引き渡す: `受付`, `入金済`, `出荷済`, `配達済`, `取消` |
| 55 | `fail NotDelivered` "配達されていません" `leaving 受注` | そのまま引き渡す: `受付`, `入金済`, `出荷済`, `配達済`, `取消` |
| 55 | flow が最後まで走り、ワークフローは成功する | `配達済`, `取消` |
| 60 | `fail Stopped` "途中で止まりました" `leaving 受注` | そのまま引き渡す: 始まっていないか、`受付`, `入金済`, `出荷済`, `配達済`, `取消` |
| 69 | `fail CancelFailed` "注文 {受注.id} を取り消せませんでした" `leaving 受注` | そのまま引き渡す: `受付`, `入金済`, `取消` |
| 72 | `fail ShippedAlready` "注文 {受注.id} はもう出荷されています" `leaving 受注` | そのまま引き渡す: `出荷済`, `配達済` |
| 73 | `on cancel` が最後まで走り、ワークフローはキャンセルで終わる | 始まっていないか、`配達済`, `取消` |

