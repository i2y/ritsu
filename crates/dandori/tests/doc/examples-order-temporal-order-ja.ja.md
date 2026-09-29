# 出荷 v1

倉庫のシステムにある注文について、入金を催促し、出荷させ、配達の知らせを待つ。注文の状態は倉庫のシステムが持ち、その移り方は rulec の規則「注文の状態」に従う。Temporal 向けの版：倉庫の呼び出しは dandori が生成するアクティビティで、お知らせは自分で書くアクティビティ。配達のことは、運送会社のシステムが注文の ID を宛先にして知らせる（event）。お店が途中で注文を取り消すと、倉庫でも取り消す（on cancel）。催促のループは、履歴が長くなると新しい実行に引き継ぐ

`examples/order/temporal/order.ja.flow` を `dandori doc` で描いたものです。入力は `注文ID: string`、出力は `便: 急ぎ.便` です。

## flow

```mermaid
flowchart TD
    start(["出荷 v1"])
    s1["受注 ← 注文を見る(…)<br>GET https://warehouse.example.com/v1/orders/{id}<br>observes · retry 3 times every 1 second on 混雑"]
    subgraph L2 ["repeat at most 3 times"]
        s3{{"match 受注.状態"}}
        s4["催促の控え = 知らせる(…)<br>自分で書くタスク<br>retry 2 times every 5 seconds"]
        s5(["fail 連絡先なし<br>leaving 受注"])
        s6("wait 1 day")
        s7["受注 ← 注文を見る(…)<br>GET https://warehouse.example.com/v1/orders/{id}<br>observes · retry 3 times every 1 second on 混雑"]
        s8(["break"])
    end
    s9{{"match 受注.状態"}}
    s10["受注 ← 取消を頼む(…)<br>POST https://warehouse.example.com/v1/orders/{id}/cancellations<br>sends 取消依頼"]
    s12(["fail 未入金<br>#quot;三日たっても入金がありませんでした#quot;"])
    s13(["fail 取消済み<br>#quot;取り消された注文です#quot;"])
    s14(["fail 既に出荷<br>#quot;もう出荷されています#quot;<br>leaving 受注"])
    s16[["判定 = 急ぎ(…)<br>rule 出荷の急ぎ.rule"]]
    s17["受注 ← 出荷を頼む(…)<br>POST https://warehouse.example.com/v1/orders/{id}/shipments<br>sends 出荷"]
    s18(["fail 取消済み<br>#quot;出荷の前に取り消されました#quot;"])
    s19{{"match 判定.急ぎ"}}
    s20["出荷の控え = 知らせる(…)<br>自分で書くタスク<br>retry 2 times every 5 seconds"]
    s22[/"受注 ← 配達の知らせ()<br>event<br>observes · timeout 7 days"/]
    s23(["fail 配達の遅れ<br>#quot;七日たっても配達の知らせがありません#quot;<br>leaving 受注"])
    s24{{"match 受注.状態"}}
    s25(["succeed 便 = 判定.便"])
    s26(["fail 配達の遅れ<br>#quot;配達の知らせのあとも出荷済のままです#quot;<br>leaving 受注"])
    start --> s1
    s1 --> s3
    s3 -->|"受付"| s4
    s4 -.->|"on 宛先なし"| s5
    s4 --> s6
    s6 --> s7
    s3 -->|"入金済, 出荷済, 配達済, 取消"| s8
    s7 -->|"次の回"| s3
    L2 -->|"3 回終えたら"| s9
    s8 --> s9
    s9 -->|"受付"| s10
    s10 --> s12
    s10 -.->|"on 断られた"| s12
    s9 -->|"取消"| s13
    s9 -->|"出荷済, 配達済"| s14
    s9 -->|"入金済"| s16
    s16 --> s17
    s17 -.->|"on 断られた"| s18
    s17 --> s19
    s19 -->|"true"| s20
    s20 --> s22
    s19 -->|"false"| s22
    s22 -.->|"on timeout"| s23
    s22 --> s24
    s24 -->|"配達済"| s25
    s24 -->|"出荷済"| s26
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s25 ok
    class s5,s12,s13,s14,s18,s23,s26 bad
```

四角はタスク、両脇に線のある四角は規則、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。

## on failure

呼び出しが失敗し、そのエラーをその場で処理しないときに走ります（60・64・67・71・77・78・81・83 行目の呼び出しから）。最後まで走ると、ワークフローはそのエラーで失敗します。

```mermaid
flowchart TD
    onf(["on failure"])
    s27(["fail 中断<br>#quot;途中で止まりました。注文は倉庫のシステムにそのまま残ります#quot;<br>leaving 受注"])
    onf --> s27
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s27 bad
```

## on cancel

ワークフローがキャンセルされると、そのとき待っている呼び出しや wait から、ここに来ます。最後まで走ると、ワークフローはキャンセルで終わります。

```mermaid
flowchart TD
    onc(["on cancel"])
    s28{{"match 受注.状態"}}
    s30["受注 ← 取消を頼む(…)<br>POST https://warehouse.example.com/v1/orders/{id}/cancellations<br>sends 取消依頼"]
    s32(["fail 取消の失敗<br>#quot;倉庫で注文 {受注.id} を取り消せませんでした#quot;<br>leaving 受注"])
    s33(["fail 出荷後の取消<br>#quot;注文 {受注.id} はもう出荷されています#quot;<br>leaving 受注"])
    oncEnd(["キャンセルで終わる"])
    onc --> s28
    s28 -->|"受付, 入金済"| s30
    s30 -.->|"on failure"| s32
    s28 -->|"出荷済"| s33
    s28 -->|"取消, none"| oncEnd
    s30 --> oncEnd
    s30 -.->|"on 断られた"| oncEnd
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s32,s33 bad
```

## 呼び出し

| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき | 呼び出しのあとの案件 |
|---:|---|---|---|---|---|---|
| 60 | `受注 ← 注文を見る(…)` | `GET https://warehouse.example.com/v1/orders/{id}`, `observes`, `idempotent` | 1 秒おきに 3 回（混雑） | — | `混雑`, `timeout`, `failure` → `on failure` | `受注`: `受付`, `入金済`, `出荷済`, `配達済`, `取消` |
| 64 | `催促の控え = 知らせる(…)` | 自分で書くタスク, `key` | 5 秒おきに 2 回（failure, timeout） | — | `宛先なし` → 65 行目<br>`timeout`, `failure` → `on failure` | — |
| 67 | `受注 ← 注文を見る(…)` | `GET https://warehouse.example.com/v1/orders/{id}`, `observes`, `idempotent` | 1 秒おきに 3 回（混雑） | — | `混雑`, `timeout`, `failure` → `on failure` | `受注`: `受付`, `入金済`, `取消` |
| 71 | `受注 ← 取消を頼む(…)` | `POST https://warehouse.example.com/v1/orders/{id}/cancellations`, `sends 取消依頼`, `key` | — | — | `断られた` → 72 行目<br>`timeout`, `failure` → `on failure` | `受注`: `取消` |
| 77 | `判定 = 急ぎ(…)` | 規則 `出荷の急ぎ.rule` | 2 回（1 秒後と 2 秒後、failure） | — | `timeout`, `failure` → `on failure` | — |
| 78 | `受注 ← 出荷を頼む(…)` | `POST https://warehouse.example.com/v1/orders/{id}/shipments`, `sends 出荷`, `key` | — | — | `断られた` → 79 行目<br>`timeout`, `failure` → `on failure` | `受注`: `出荷済` |
| 81 | `出荷の控え = 知らせる(…)` | 自分で書くタスク, `key` | 5 秒おきに 2 回（failure, timeout） | — | `宛先なし`, `timeout`, `failure` → `on failure` | — |
| 83 | `受注 ← 配達の知らせ()` | `event`, `observes` | — | 7 日 | `timeout` → 84 行目<br>`failure` → `on failure` | `受注`: `出荷済`, `配達済` |
| 97 | `受注 ← 取消を頼む(…)` | `POST https://warehouse.example.com/v1/orders/{id}/cancellations`, `sends 取消依頼`, `key` | — | — | `断られた` → 98 行目<br>`timeout`, `failure` → 99 行目 | `受注`: `取消` |

## 終わり方

ワークフローの終わり方のすべてと、そのとき各案件がとりうる状態です。外部のサービスで起きるイベントも含めています。

| 行 | 終わり方 | `受注` |
|---:|---|---|
| 65 | `fail 連絡先なし` `leaving 受注` | そのまま引き渡す: `受付`, `入金済`, `取消` |
| 73 | `fail 未入金` "三日たっても入金がありませんでした" | `取消` |
| 74 | `fail 取消済み` "取り消された注文です" | `取消` |
| 75 | `fail 既に出荷` "もう出荷されています" `leaving 受注` | そのまま引き渡す: `出荷済`, `配達済` |
| 79 | `fail 取消済み` "出荷の前に取り消されました" | `取消` |
| 84 | `fail 配達の遅れ` "七日たっても配達の知らせがありません" `leaving 受注` | そのまま引き渡す: `出荷済`, `配達済` |
| 86 | `succeed 便 = 判定.便` | `配達済` |
| 87 | `fail 配達の遅れ` "配達の知らせのあとも出荷済のままです" `leaving 受注` | そのまま引き渡す: `出荷済`, `配達済` |
| 90 | `fail 中断` "途中で止まりました。注文は倉庫のシステムにそのまま残ります" `leaving 受注` | そのまま引き渡す: 始まっていないか、`受付`, `入金済`, `出荷済`, `配達済`, `取消` |
| 99 | `fail 取消の失敗` "倉庫で注文 {受注.id} を取り消せませんでした" `leaving 受注` | そのまま引き渡す: `受付`, `入金済`, `取消` |
| 100 | `fail 出荷後の取消` "注文 {受注.id} はもう出荷されています" `leaving 受注` | そのまま引き渡す: `出荷済`, `配達済` |
| 100 | `on cancel` が最後まで走り、ワークフローはキャンセルで終わる | 始まっていないか、`取消` |

