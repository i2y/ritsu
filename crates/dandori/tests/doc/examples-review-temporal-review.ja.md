# review v1

Jev scores an application and says how sure it is, and a rule decides whether the verdict is acted on at once or goes to a person for approval: approving at once asks more certainty than rejecting at once. Written for Temporal: the scoring is an activity dandori writes, which calls Jev with TypeSafe's key (TYPESAFE_API_KEY) on the workers of its own task queue, which may be written in another language, and so does the notice, an activity you write; the rule runs in the worker of the workflow; the approver's tool answers the callback by the update the generated client sends

`examples/review/temporal/review.flow` を `dandori doc` で描いたものです。入力は `application: Application`、出力は `verdict: policy.verdict` です。

## flow

```mermaid
flowchart TD
    start(["review v1"])
    s1["r = score(…)<br>jev · jev-1.13.0<br>retry 2 times every 10 seconds on busy, overloaded · timeout 10 seconds"]
    s2(["fail Unscorable<br>#quot;Could not score application {application.id}#quot;"])
    s3[["d = policy(…)<br>rule review_policy.rule"]]
    s4{{"match d.decision"}}
    s6[/"a = ask_for_approval(…)<br>自分で書くタスク（応答はコールバック）<br>timeout 3 days"/]
    s7(["fail NoAnswer<br>#quot;No approval in three days#quot;"])
    s8["notify(…)<br>自分で書くタスク"]
    s9(["succeed verdict = approve"])
    s10["notify(…)<br>自分で書くタスク"]
    s11(["succeed verdict = r.verdict"])
    start --> s1
    s1 -.->|"on failure"| s2
    s1 --> s3
    s3 --> s4
    s4 -->|"ask"| s6
    s6 -.->|"on timeout"| s7
    s6 --> s8
    s8 --> s9
    s4 -->|"approve, reject"| s10
    s10 --> s11
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s9,s11 ok
    class s2,s7 bad
```

四角はタスク、両脇に線のある四角は規則、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。

## 呼び出し

| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき |
|---:|---|---|---|---|---|
| 49 | `r = score(…)` | `jev · jev-1.13.0` | 10 秒おきに 2 回（busy, overloaded） | 10 秒 | `busy`, `overloaded`, `timeout`, `failure` → 50 行目 |
| 51 | `d = policy(…)` | 規則 `review_policy.rule` | 2 回（1 秒後と 2 秒後、failure） | — | `timeout`, `failure` → ワークフローが失敗する |
| 55 | `a = ask_for_approval(…)` | 自分で書くタスク（応答はコールバック） | — | 3 日 | `timeout` → 56 行目<br>`failure` → ワークフローが失敗する |
| 57 | `notify(…)` | 自分で書くタスク, `idempotent` | — | — | `timeout`, `failure` → ワークフローが失敗する |
| 59 | `notify(…)` | 自分で書くタスク, `idempotent` | — | — | `timeout`, `failure` → ワークフローが失敗する |

## 終わり方

ワークフローの終わり方のすべてです。

| 行 | 終わり方 |
|---:|---|
| 50 | `fail Unscorable` "Could not score application {application.id}" |
| 56 | `fail NoAnswer` "No approval in three days" |
| 58 | `succeed verdict = approve` |
| 60 | `succeed verdict = r.verdict` |

