# review v1

Jev scores an application and says how sure it is, and a rule decides whether the verdict is acted on at once or goes to a person for approval, due by 17:00 on the third business day after it comes in: approving at once asks more certainty than rejecting at once. Written for Temporal: the scoring is an activity dandori writes, which calls Jev with TypeSafe's key (TYPESAFE_API_KEY) on the workers of its own task queue, which may be written in another language, and so does the notice, an activity you write; the rule and the date of the approval run in the worker of the workflow; the approver's tool answers the callback by the update the generated client sends

`examples/review/temporal/review.flow` を `dandori doc` で描いたものです。入力は `application: Application`、出力は `verdict: policy.verdict` です。

## flow

```mermaid
flowchart TD
    start(["review v1"])
    s1["r = score(…)<br>jev · jev-1.13.0<br>retry 2 times every 10 seconds on busy, overloaded · timeout 10 seconds"]
    s2(["fail Unscorable<br>#quot;Could not score application {application.id}#quot;"])
    s3[["d = policy(…)<br>rule review_policy.rule"]]
    s4{{"match d.decision"}}
    s6[["due = approvals.due(…)<br>dates approval_terms.cal"]]
    s7[/"a = ask_for_approval(…)<br>自分で書くタスク（応答はコールバック）<br>timeout until by"/]
    s8(["fail NoAnswer<br>#quot;No approval by 17:00 on the third business day#quot;"])
    s9["notify(…)<br>自分で書くタスク"]
    s10(["succeed verdict = approve"])
    s11["notify(…)<br>自分で書くタスク"]
    s12(["succeed verdict = r.verdict"])
    start --> s1
    s1 -.->|"on failure"| s2
    s1 --> s3
    s3 --> s4
    s4 -->|"ask"| s6
    s6 --> s7
    s7 -.->|"on timeout"| s8
    s7 --> s9
    s9 --> s10
    s4 -->|"approve, reject"| s11
    s11 --> s12
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s10,s12 ok
    class s2,s8 bad
```

四角はタスク、両脇に線のある四角は規則か日付のファイルの日付、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。

## 呼び出し

| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき |
|---:|---|---|---|---|---|
| 51 | `r = score(…)` | `jev · jev-1.13.0` | 10 秒おきに 2 回（busy・overloaded） | 10 秒 | `busy`・`overloaded`・`timeout`・`failure` → 52 行目 |
| 53 | `d = policy(…)` | 規則 `review_policy.rule` | 1 秒後と 2 秒後の 2 回（failure・timeout） | 10 秒 | `timeout`・`failure` → ワークフローが失敗する |
| 57 | `due = approvals.due(…)` | `approval_terms.cal` の日付 `due` | 1 秒後と 2 秒後の 2 回（failure・timeout） | 10 秒 | `timeout`・`failure` → ワークフローが失敗する |
| 58 | `a = ask_for_approval(…)` | 自分で書くタスク（応答はコールバック） | — | ID を渡すのに 1 分、そのあと `due.at` まで | `timeout` → 59 行目<br>`failure` → ワークフローが失敗する |
| 60 | `notify(…)` | 自分で書くタスク・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |
| 62 | `notify(…)` | 自分で書くタスク・`idempotent` | — | — | `timeout`・`failure` → ワークフローが失敗する |

## 終わり方

ワークフローの終わり方のすべてです。

| 行 | 終わり方 |
|---:|---|
| 52 | `fail Unscorable` "Could not score application {application.id}" |
| 59 | `fail NoAnswer` "No approval by 17:00 on the third business day" |
| 61 | `succeed verdict = approve` |
| 63 | `succeed verdict = r.verdict` |

## 規則

このワークフローが呼ぶ規則です。`rulec doc` が描いた、人が読むページをそのまま載せています。

<details>
<summary><code>policy</code> · review_policy v1 · <code>../rules/review_policy.rule</code></summary>

<!-- rulec 0.36.0 が review_policy.rule (sha256:d71c8b54077c) から生成した資料です。読むためのもので、もとになるのは .rule のほうです。ここを編集しても .rule には戻せません。 -->
# 規則 review_policy v1

Whether the scoring's verdict on an application is acted on at once or goes to a person, by how sure the scoring is of it: approving at once asks more certainty than rejecting at once. Written for the example

## 入力

| 名前 | 型 | 範囲 | 注記 |
|---|---|---|---|
| verdict | verdict（3 値） |  |  |
| sure | rate | 0% 〜 100% |  |

## 出力

| 名前 | 型 | 丸め | 注記 |
|---|---|---|---|
| decision | decision（3 値） |  |  |

## 型

列挙は**閉じた**有限集合です。値を足すと、その値を扱っていない表は完全性の検査を通らなくなります。

- **verdict**（3 値）— reject、hold、approve
- **decision**（3 値）— approve、reject、ask

## 表 act（policy unique）

| 列 | 出どころ |
|---|---|
| verdict | 入力 |
| sure | 入力 |
| → decision | この規則の出力 |

| # | verdict | sure | → decision（decision） |
|---|---|---|---|
| 1 | approve | >=90% | approve |
| 2 | approve | <90% | ask |
| 3 | reject | >=80% | reject |
| 4 | reject | <80% | ask |
| 5 | hold | - | ask |

**`rulec check` が確かめたこと**

- どの入力の組合せも、いずれかの行に当てはまります（E101 完全性）
- どの入力にも当てはまらない行はありません（E102）
- 二つ以上の行に同時に当てはまる入力はありません（E105 重なり）。行の並べ替えは意味を変えません

## 例（検証済み）

| verdict | sure | → decision |
|---|---|---|
| approve | 95% | approve |
| approve | 89% | ask |
| reject | 80% | reject |
| hold | 99% | ask |

この 4 件は `rulec check` が参照評価器で実行し、すべて宣言どおりの値になりました（E107）。例は**実行される仕様**です。

</details>

