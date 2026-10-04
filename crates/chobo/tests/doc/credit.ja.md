<!-- `chobo doc tests/books/credit.book --lang ja` の出力です。手で編集しないでください。 -->

# credit v1

A pay-later line for each member. It can be used down to minus 50000 yen, and prepaid up to 10000 yen

`chobo doc` が `tests/books/credit.book` から作ったページ。勘定ごとに残高を持ち、残高は入った量から出た量を引いたもの。どの振替も勘定の下限と上限を守り、守れない振替は、その境界に付けた名前で断られて、どの移動も行われない。振替には、すぐに動かすもの（`do`）と、動かす量をまず押さえるもの（`hold`）がある。押さえた分は、あとで確定される（`post`。全部か一部）か、取り消される（`void`）か、有効期限で切れる。

## 勘定

| 勘定 | 分け方 | 単位 | 境界 | 説明 |
|---|---|---|---|---|
| `credit` | `member` ごと | JPY | -50000 以上。下回る振替は `over_the_credit_line` で断る<br>10000 以下。超える振替は `over_prepayment` で断る |  |
| `shop` | 一つだけ | JPY | 外の勘定。境界は無く、マイナスにもなる |  |
| `bank` | 一つだけ | JPY | 外の勘定。境界は無く、マイナスにもなる |  |

## 勘定のあいだの流れ

```mermaid
flowchart LR
    a0["credit(member)<br>JPY<br>-50000 以上（over_the_credit_line）<br>10000 以下（over_prepayment）"]
    a1(["shop<br>JPY・外の勘定"])
    a2(["bank<br>JPY・外の勘定"])
    a0 -.->|"pay_later"| a1
    a2 -->|"repayment"| a0
```

四角は勘定で、矢印は振替の移動。角の丸い四角は外の勘定で、境界を持たない。破線の矢印は仮押さえの振替で、まず押さえ、確定したときに動く。

## 振替

### pay_later

- 移動: `credit(member)` から `shop` へ `amount`。
- キー: `order` ごとに一回。同じ呼び出しの二度目は何もせず、`done_before` を返す。`member` か `amount` だけが違う二度目は `key_conflict` で断られる。押さえが終わったあとに同じキーで押さえ直すと `done_before` になり、何も押さえない。
- 仮押さえ: まず押さえる。確定と取消は呼ぶ側がする。押さえてから 1 日で期限が切れ、押さえた量は元に戻る。

```mermaid
stateDiagram-v2
    direction LR
    state "押さえ中" as held
    state "確定" as posted
    state "取消" as voided
    state "期限切れ" as expired
    [*] --> held : pay_later.hold
    held --> posted : 確定（post）
    held --> voided : 取消（void）
    held --> expired : 1 日たつ
    posted --> [*]
    voided --> [*]
    expired --> [*]
```

| 仮押さえの状態 | post（確定） | void（取消） |
|---|---|---|
| 押さえ中 | 確定する。額を渡せばその額、渡さなければ全額で、残りは元に戻る。押さえた額を超えれば `over_hold` で断る。押さえてから 1 日たっていれば `expired` で断る | 取り消す。押さえた量は元に戻る。押さえてから 1 日たっていれば `expired` で断る |
| 確定 | 同じ額なら `done_before`、違う額なら `key_conflict` で断る | `already_posted` で断る |
| 取消 | `already_voided` で断る | `done_before` |
| 期限切れ | `expired` で断る | `expired` で断る |
| 仮押さえが無い | `no_such_hold` で断る | `no_such_hold` で断る |

| 操作 | 断られうる理由 | いつ |
|---|---|---|
| `pay_later.hold` | `over_the_credit_line` | `credit(member)` が -50000 を下回る |
| `pay_later.hold` | `key_conflict` | `order` が同じで、ほかの引数が違う呼び出しが、前に済んでいる |
| `pay_later.hold` | `already_refused` | `order` が同じ呼び出しが、前に境界で断られている。境界で断られたキーは、あとで足りるようになっても通らない |
| `pay_later.post` | `key_conflict` | 仮押さえは、違う額で確定済み |
| `pay_later.post` | `already_voided` | 仮押さえはもう取り消されている |
| `pay_later.post` | `expired` | 仮押さえの期限が切れている |
| `pay_later.post` | `over_hold` | 押さえた額より多く確定しようとした |
| `pay_later.post` | `no_such_hold` | その `order` の仮押さえが無い |
| `pay_later.void` | `already_posted` | 仮押さえはもう確定している |
| `pay_later.void` | `expired` | 仮押さえの期限が切れている |
| `pay_later.void` | `no_such_hold` | その `order` の仮押さえが無い |

<details><summary>断られる例</summary>

#### pay_later.hold: over_the_credit_line

```text
 1  pay_later.hold(order: order-1, member: member-2, amount: 50001)  over_the_credit_line で断られる（1 つ目の移動が credit(member-2) から 50001 を取る。確定 0、出ていく仮押さえ 0）
```

#### pay_later.hold: key_conflict

```text
 1  pay_later.hold(order: order-1, member: member-2, amount: 1)  通る
 2  pay_later.hold(order: order-1, member: member-2, amount: 2)  key_conflict で断られる
```

#### pay_later.hold: already_refused

```text
 1  pay_later.hold(order: order-1, member: member-2, amount: 50001)  over_the_credit_line で断られる（1 つ目の移動が credit(member-2) から 50001 を取る。確定 0、出ていく仮押さえ 0）
 2  pay_later.hold(order: order-1, member: member-2, amount: 50001)  already_refused で断られる
```

#### pay_later.post: key_conflict

```text
 1  pay_later.hold(order: order-1, member: member-2, amount: 2)  通る
 2  pay_later.post(order: order-1)                               通る
 3  pay_later.post(order: order-1, amount: 1)                    key_conflict で断られる
```

#### pay_later.post: already_voided

```text
 1  pay_later.hold(order: order-1, member: member-2, amount: 2)  通る
 2  pay_later.void(order: order-1)                               通る
 3  pay_later.post(order: order-1)                               already_voided で断られる
```

#### pay_later.post: expired

```text
 1  pay_later.hold(order: order-1, member: member-2, amount: 2)  通る
 2  pass 1 日                                                    pay_later(order-1) が期限切れ
 3  pay_later.post(order: order-1)                               expired で断られる
```

#### pay_later.post: over_hold

```text
 1  pay_later.hold(order: order-1, member: member-2, amount: 2)  通る
 2  pay_later.post(order: order-1, amount: 3)                    over_hold で断られる
```

#### pay_later.post: no_such_hold

```text
 1  pay_later.post(order: order-1)  no_such_hold で断られる
```

#### pay_later.void: already_posted

```text
 1  pay_later.hold(order: order-1, member: member-2, amount: 2)  通る
 2  pay_later.post(order: order-1)                               通る
 3  pay_later.void(order: order-1)                               already_posted で断られる
```

#### pay_later.void: expired

```text
 1  pay_later.hold(order: order-1, member: member-2, amount: 2)  通る
 2  pass 1 日                                                    pay_later(order-1) が期限切れ
 3  pay_later.void(order: order-1)                               expired で断られる
```

#### pay_later.void: no_such_hold

```text
 1  pay_later.void(order: order-1)  no_such_hold で断られる
```

</details>

### repayment

- 移動: `bank` から `credit(member)` へ `amount`。
- キー: `repayment_id` ごとに一回。同じ呼び出しの二度目は何もせず、`done_before` を返す。`member` か `amount` だけが違う二度目は `key_conflict` で断られる。
- すぐに動かす（`do`）。

| 操作 | 断られうる理由 | いつ |
|---|---|---|
| `repayment.do` | `over_prepayment` | `credit(member)` が 10000 を超える |
| `repayment.do` | `key_conflict` | `repayment_id` が同じで、ほかの引数が違う呼び出しが、前に済んでいる |
| `repayment.do` | `already_refused` | `repayment_id` が同じ呼び出しが、前に境界で断られている。境界で断られたキーは、あとで足りるようになっても通らない |

<details><summary>断られる例</summary>

#### repayment.do: over_prepayment

```text
 1  repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 10000)  通る
 2  repayment.do(repayment_id: repayment_id-3, member: member-2, amount: 1)      over_prepayment で断られる（1 つ目の移動が credit(member-2) へ 1 を入れる。確定 10000、入ってくる仮押さえ 0）
```

#### repayment.do: key_conflict

```text
 1  repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 1)  通る
 2  repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 2)  key_conflict で断られる
```

#### repayment.do: already_refused

```text
 1  repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 10000)  通る
 2  repayment.do(repayment_id: repayment_id-3, member: member-2, amount: 1)      over_prepayment で断られる（1 つ目の移動が credit(member-2) へ 1 を入れる。確定 10000、入ってくる仮押さえ 0）
 3  repayment.do(repayment_id: repayment_id-3, member: member-2, amount: 1)      already_refused で断られる
```

</details>

## シナリオ

`chobo scenarios` が帳簿から作ったシナリオ 24 本。境界の手前・ちょうど・超える、同じキーの二度目、仮押さえの終わり方、二つの呼び出し元が同時に最後の一つを取りに来るもの、などがある。どれも参照インタプリタで流し、ステップごとに、そのあとの残高を載せる。残高は確定した量で、仮押さえがあれば括弧の中に書く。

<details><summary>1. 境界: pay_later.hold が credit(member) を -49999 まで減らす。<code>at least -50000</code> の 1 つ手前</summary>

| # | 操作 | 結果 | credit(member-2) | shop | bank |
|---|---|---|---|---|---|
| 1 | repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 2) | 通る | 2 | 0 | -2 |
| 2 | pay_later.hold(order: order-3, member: member-2, amount: 50001) | 通る | 2（出ていく仮押さえ 50001） | 0（入ってくる仮押さえ 50001） | -2 |

</details>

<details><summary>2. 境界: pay_later.hold が credit(member) をちょうど -50000 まで減らす。<code>at least -50000</code> ちょうど</summary>

| # | 操作 | 結果 | credit(member-2) | shop | bank |
|---|---|---|---|---|---|
| 1 | repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 2) | 通る | 2 | 0 | -2 |
| 2 | pay_later.hold(order: order-3, member: member-2, amount: 50002) | 通る | 2（出ていく仮押さえ 50002） | 0（入ってくる仮押さえ 50002） | -2 |

</details>

<details><summary>3. 境界: pay_later.hold は credit(member) を -50001 まで減らすので断られる。<code>at least -50000</code> を割る</summary>

| # | 操作 | 結果 | credit(member-2) | shop | bank |
|---|---|---|---|---|---|
| 1 | repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 2) | 通る | 2 | 0 | -2 |
| 2 | pay_later.hold(order: order-3, member: member-2, amount: 50003) | over_the_credit_line で断られる | 2 | 0 | -2 |

</details>

<details><summary>4. 境界: repayment.do が credit(member) を 9999 まで増やす。<code>at most 10000</code> の 1 つ手前</summary>

| # | 操作 | 結果 | credit(member-2) | bank |
|---|---|---|---|---|
| 1 | repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 9998) | 通る | 9998 | -9998 |
| 2 | repayment.do(repayment_id: repayment_id-3, member: member-2, amount: 1) | 通る | 9999 | -9999 |

</details>

<details><summary>5. 境界: repayment.do が credit(member) をちょうど 10000 まで増やす。<code>at most 10000</code> ちょうど</summary>

| # | 操作 | 結果 | credit(member-2) | bank |
|---|---|---|---|---|
| 1 | repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 9998) | 通る | 9998 | -9998 |
| 2 | repayment.do(repayment_id: repayment_id-3, member: member-2, amount: 2) | 通る | 10000 | -10000 |

</details>

<details><summary>6. 境界: repayment.do は credit(member) を 10001 まで増やすので断られる。<code>at most 10000</code> を超える</summary>

| # | 操作 | 結果 | credit(member-2) | bank |
|---|---|---|---|---|
| 1 | repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 9998) | 通る | 9998 | -9998 |
| 2 | repayment.do(repayment_id: repayment_id-3, member: member-2, amount: 3) | over_prepayment で断られる | 9998 | -9998 |

</details>

<details><summary>7. キー: pay_later.hold を同じ引数で二度呼ぶ</summary>

| # | 操作 | 結果 | credit(member-2) | shop |
|---|---|---|---|---|
| 1 | pay_later.hold(order: order-1, member: member-2, amount: 1) | 通る | 0（出ていく仮押さえ 1） | 0（入ってくる仮押さえ 1） |
| 2 | pay_later.hold(order: order-1, member: member-2, amount: 1) | done_before（前に済んでいる） | 0（出ていく仮押さえ 1） | 0（入ってくる仮押さえ 1） |

</details>

<details><summary>8. キー: pay_later.hold を amount だけ変えてもう一度呼ぶ</summary>

| # | 操作 | 結果 | credit(member-2) | shop |
|---|---|---|---|---|
| 1 | pay_later.hold(order: order-1, member: member-2, amount: 1) | 通る | 0（出ていく仮押さえ 1） | 0（入ってくる仮押さえ 1） |
| 2 | pay_later.hold(order: order-1, member: member-2, amount: 2) | key_conflict で断られる | 0（出ていく仮押さえ 1） | 0（入ってくる仮押さえ 1） |

</details>

<details><summary>9. キー: pay_later.hold が over_the_credit_line で断られ、同じ引数でもう一度、credit(member) が足りるようになってからもう一度呼ぶ</summary>

| # | 操作 | 結果 | credit(member-2) | shop | bank |
|---|---|---|---|---|---|
| 1 | pay_later.hold(order: order-1, member: member-2, amount: 50001) | over_the_credit_line で断られる | 0 | 0 | 0 |
| 2 | pay_later.hold(order: order-1, member: member-2, amount: 50001) | already_refused で断られる | 0 | 0 | 0 |
| 3 | repayment.do(repayment_id: repayment_id-3, member: member-2, amount: 1) | 通る | 1 | 0 | -1 |
| 4 | pay_later.hold(order: order-1, member: member-2, amount: 50001) | already_refused で断られる | 1 | 0 | -1 |

</details>

<details><summary>10. キー: repayment.do を同じ引数で二度呼ぶ</summary>

| # | 操作 | 結果 | credit(member-2) | bank |
|---|---|---|---|---|
| 1 | repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 1) | 通る | 1 | -1 |
| 2 | repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 1) | done_before（前に済んでいる） | 1 | -1 |

</details>

<details><summary>11. キー: repayment.do を amount だけ変えてもう一度呼ぶ</summary>

| # | 操作 | 結果 | credit(member-2) | bank |
|---|---|---|---|---|
| 1 | repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 1) | 通る | 1 | -1 |
| 2 | repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 2) | key_conflict で断られる | 1 | -1 |

</details>

<details><summary>12. キー: repayment.do が over_prepayment で断られ、同じ引数でもう一度呼ぶ</summary>

| # | 操作 | 結果 | credit(member-2) | bank |
|---|---|---|---|---|
| 1 | repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 10000) | 通る | 10000 | -10000 |
| 2 | repayment.do(repayment_id: repayment_id-3, member: member-2, amount: 1) | over_prepayment で断られる | 10000 | -10000 |
| 3 | repayment.do(repayment_id: repayment_id-3, member: member-2, amount: 1) | already_refused で断られる | 10000 | -10000 |

</details>

<details><summary>13. 仮押さえ: pay_later を全額で確定する</summary>

| # | 操作 | 結果 | credit(member-2) | shop |
|---|---|---|---|---|
| 1 | pay_later.hold(order: order-1, member: member-2, amount: 2) | 通る | 0（出ていく仮押さえ 2） | 0（入ってくる仮押さえ 2） |
| 2 | pay_later.post(order: order-1) | 通る | -2 | 2 |

</details>

<details><summary>14. 仮押さえ: pay_later を一部だけ確定する</summary>

| # | 操作 | 結果 | credit(member-2) | shop |
|---|---|---|---|---|
| 1 | pay_later.hold(order: order-1, member: member-2, amount: 2) | 通る | 0（出ていく仮押さえ 2） | 0（入ってくる仮押さえ 2） |
| 2 | pay_later.post(order: order-1, amount: 1) | 通る | -1 | 1 |

</details>

<details><summary>15. 仮押さえ: pay_later を取り消す</summary>

| # | 操作 | 結果 | credit(member-2) | shop |
|---|---|---|---|---|
| 1 | pay_later.hold(order: order-1, member: member-2, amount: 2) | 通る | 0（出ていく仮押さえ 2） | 0（入ってくる仮押さえ 2） |
| 2 | pay_later.void(order: order-1) | 通る | 0 | 0 |

</details>

<details><summary>16. 仮押さえ: pay_later を確定してから取り消そうとする</summary>

| # | 操作 | 結果 | credit(member-2) | shop |
|---|---|---|---|---|
| 1 | pay_later.hold(order: order-1, member: member-2, amount: 2) | 通る | 0（出ていく仮押さえ 2） | 0（入ってくる仮押さえ 2） |
| 2 | pay_later.post(order: order-1) | 通る | -2 | 2 |
| 3 | pay_later.void(order: order-1) | already_posted で断られる | -2 | 2 |

</details>

<details><summary>17. 仮押さえ: pay_later を取り消してから確定しようとする</summary>

| # | 操作 | 結果 | credit(member-2) | shop |
|---|---|---|---|---|
| 1 | pay_later.hold(order: order-1, member: member-2, amount: 2) | 通る | 0（出ていく仮押さえ 2） | 0（入ってくる仮押さえ 2） |
| 2 | pay_later.void(order: order-1) | 通る | 0 | 0 |
| 3 | pay_later.post(order: order-1) | already_voided で断られる | 0 | 0 |

</details>

<details><summary>18. 仮押さえ: pay_later を押さえた額より多く確定しようとし、押さえた額で確定する</summary>

| # | 操作 | 結果 | credit(member-2) | shop |
|---|---|---|---|---|
| 1 | pay_later.hold(order: order-1, member: member-2, amount: 2) | 通る | 0（出ていく仮押さえ 2） | 0（入ってくる仮押さえ 2） |
| 2 | pay_later.post(order: order-1, amount: 3) | over_hold で断られる | 0（出ていく仮押さえ 2） | 0（入ってくる仮押さえ 2） |
| 3 | pay_later.post(order: order-1, amount: 2) | 通る | -2 | 2 |

</details>

<details><summary>19. 仮押さえ: pay_later を押さえる前に確定しようとし、押さえてから確定する</summary>

| # | 操作 | 結果 | credit(member-2) | shop |
|---|---|---|---|---|
| 1 | pay_later.post(order: order-1) | no_such_hold で断られる | 0 | 0 |
| 2 | pay_later.hold(order: order-1, member: member-2, amount: 1) | 通る | 0（出ていく仮押さえ 1） | 0（入ってくる仮押さえ 1） |
| 3 | pay_later.post(order: order-1) | 通る | -1 | 1 |

</details>

<details><summary>20. 仮押さえ: pay_later を二度確定する。同じ額で、そして違う額で</summary>

| # | 操作 | 結果 | credit(member-2) | shop |
|---|---|---|---|---|
| 1 | pay_later.hold(order: order-1, member: member-2, amount: 2) | 通る | 0（出ていく仮押さえ 2） | 0（入ってくる仮押さえ 2） |
| 2 | pay_later.post(order: order-1) | 通る | -2 | 2 |
| 3 | pay_later.post(order: order-1) | done_before（前に済んでいる） | -2 | 2 |
| 4 | pay_later.post(order: order-1, amount: 2) | done_before（前に済んでいる） | -2 | 2 |
| 5 | pay_later.post(order: order-1, amount: 1) | key_conflict で断られる | -2 | 2 |

</details>

<details><summary>21. 期限切れ: pay_later が期限切れになってから、確定と取消を試す</summary>

| # | 操作 | 結果 | credit(member-2) | shop |
|---|---|---|---|---|
| 1 | pay_later.hold(order: order-1, member: member-2, amount: 2) | 通る | 0（出ていく仮押さえ 2） | 0（入ってくる仮押さえ 2） |
| 2 | pass 1441 分 | pay_later(order-1) が期限切れ | 0 | 0 |
| 3 | pay_later.post(order: order-1) | expired で断られる | 0 | 0 |
| 4 | pay_later.void(order: order-1) | expired で断られる | 0 | 0 |

</details>

<details><summary>22. 期限切れ: pay_later が期限切れになってから、同じキーで押さえ直す</summary>

| # | 操作 | 結果 | credit(member-2) | shop |
|---|---|---|---|---|
| 1 | pay_later.hold(order: order-1, member: member-2, amount: 2) | 通る | 0（出ていく仮押さえ 2） | 0（入ってくる仮押さえ 2） |
| 2 | pass 1441 分 | pay_later(order-1) が期限切れ | 0 | 0 |
| 3 | pay_later.hold(order: order-1, member: member-2, amount: 2) | done_before（前に済んでいる） | 0 | 0 |

</details>

<details><summary>23. 同時: 二つの呼び出し元が pay_later.hold で credit(member) の最後の 50000 を取り合う</summary>

同時の操作の順序によって、結果は 2 通りある。

結果 1:

| # | 操作 | 結果 | credit(member-2) | shop |
|---|---|---|---|---|
| 1 | together<br>呼び出し元 1: pay_later.hold(order: order-1, member: member-2, amount: 50000)<br>呼び出し元 2: pay_later.hold(order: order-3, member: member-2, amount: 50000) | <br>通る<br>over_the_credit_line で断られる | 0（出ていく仮押さえ 50000） | 0（入ってくる仮押さえ 50000） |

結果 2:

| # | 操作 | 結果 | credit(member-2) | shop |
|---|---|---|---|---|
| 1 | together<br>呼び出し元 1: pay_later.hold(order: order-1, member: member-2, amount: 50000)<br>呼び出し元 2: pay_later.hold(order: order-3, member: member-2, amount: 50000) | <br>over_the_credit_line で断られる<br>通る | 0（出ていく仮押さえ 50000） | 0（入ってくる仮押さえ 50000） |

</details>

<details><summary>24. 同時: 二つの呼び出し元が repayment.do で credit(member) の最後の空き 1 を取り合う</summary>

同時の操作の順序によって、結果は 2 通りある。

結果 1:

| # | 操作 | 結果 | credit(member-2) | bank |
|---|---|---|---|---|
| 1 | repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 9999) | 通る | 9999 | -9999 |
| 2 | together<br>呼び出し元 1: repayment.do(repayment_id: repayment_id-3, member: member-2, amount: 1)<br>呼び出し元 2: repayment.do(repayment_id: repayment_id-4, member: member-2, amount: 1) | <br>通る<br>over_prepayment で断られる | 10000 | -10000 |

結果 2:

| # | 操作 | 結果 | credit(member-2) | bank |
|---|---|---|---|---|
| 1 | repayment.do(repayment_id: repayment_id-1, member: member-2, amount: 9999) | 通る | 9999 | -9999 |
| 2 | together<br>呼び出し元 1: repayment.do(repayment_id: repayment_id-3, member: member-2, amount: 1)<br>呼び出し元 2: repayment.do(repayment_id: repayment_id-4, member: member-2, amount: 1) | <br>over_prepayment で断られる<br>通る | 10000 | -10000 |

</details>

