# 仕分け v1

Jev のエッジケース：一部の値にだけ意味を書いた choice、score、はいといいえの意味を書いた noul、一回のリクエストで四つに答えるレコード（choice・score・noul と、確信度の割合）、確信度が下限を切ったときのエラーを処理する呼び出しと、処理せずに on failure へ行く呼び出し、下限を切った答えのあとも前の値が残る変数、並列のイテレーションの中の Jev、結果を読まない呼び出し、ステータスで宣言したエラーのリトライ

`tests/flows/jev.flow`, drawn by `dandori doc`. Inputs: `問い合わせ: list[string]`, `本文: string`, `会員: bool`. Outputs: `一通ごとの種類: list[種類]`, `種類: 種類`, `急ぎ: 急ぎ`, `人: bool`, `確かさ: rate[step 0.01%]`.

## flow

```mermaid
flowchart TD
    start(["仕分け v1"])
    s1["人を求める(…)<br>jev · jev-1.13.0 · confidence 0.9 else 分からない"]
    subgraph L3 ["let 一通ごとの種類 = for 一通 in 問い合わせ at most 3 in parallel · yield k"]
        s4["k = 種類を選ぶ(…)<br>jev · jev-1.13.0 · confidence 0.8 else 迷い<br>retry 2 times every 1 second on 混雑, 過負荷 · timeout 10 seconds"]
        s5["let k = その他"]
    end
    s6["種類 = 種類を選ぶ(…)<br>jev · jev-1.13.0 · confidence 0.8 else 迷い<br>retry 2 times every 1 second on 混雑, 過負荷 · timeout 10 seconds"]
    s7["let 種類 = その他"]
    s8["種類 = 種類を選ぶ(…)<br>jev · jev-1.13.0 · confidence 0.8 else 迷い<br>retry 2 times every 1 second on 混雑, 過負荷 · timeout 10 seconds"]
    s10["急 = 急ぎを測る(…)<br>jev · jev-1.13.0 · confidence 0.7 else 測れない"]
    s11["let 急 = 高"]
    s12["読み = 読む(…)<br>jev · jev-1.13.0 · confidence 0.6 else 自信なし<br>timeout 10 seconds"]
    s13(["succeed 一通ごとの種類 = 一通ごとの種類, 種類 = 種類, 急ぎ = 急, 人 = 読み.人, 確…"])
    start --> s1
    s1 --> s4
    s1 -.->|"on 分からない"| s4
    s4 -.->|"on 迷い"| s5
    L3 -->|"every round done"| s6
    s6 -.->|"on 迷い"| s7
    s6 --> s8
    s7 --> s8
    s8 --> s10
    s8 -.->|"on 迷い"| s10
    s10 -.->|"on 測れない"| s11
    s10 --> s12
    s11 --> s12
    s12 --> s13
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s13 ok
```

A rectangle is a task, one with a line down each side a rule, a slanted one a task whose value comes from outside (an event, or the answer to a callback), a hexagon a `match`, a rounded box a wait, and a box around steps a loop. A dashed arrow is an error the call handles.

## on failure

Runs when a call fails and nothing at the call handles the error: from the calls on lines 73, 76, 79, 82, 84 and 86. When it runs to its end, the workflow fails with that error.

```mermaid
flowchart TD
    onf(["on failure"])
    s14(["fail 読めない<br>#quot;問い合わせを読み取れませんでした#quot;"])
    onf --> s14
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s14 bad
```

## Calls

| Line | Call | Calls | Retries | Timeout | When it fails |
|---:|---|---|---|---|---|
| 73 | `人を求める(…)` | `jev · jev-1.13.0 · confidence 0.9 else 分からない` | — | — | `分からない` → line 74<br>`timeout`, `failure` → `on failure` |
| 76 | `k = 種類を選ぶ(…)` | `jev · jev-1.13.0 · confidence 0.8 else 迷い` | 2 times every 1 second (混雑, 過負荷) | 10 seconds | `混雑`, `過負荷` → the round fails, then `on failure`<br>`迷い` → line 77<br>`timeout`, `failure` → the round fails, then `on failure` |
| 79 | `種類 = 種類を選ぶ(…)` | `jev · jev-1.13.0 · confidence 0.8 else 迷い` | 2 times every 1 second (混雑, 過負荷) | 10 seconds | `混雑`, `過負荷` → `on failure`<br>`迷い` → line 80<br>`timeout`, `failure` → `on failure` |
| 82 | `種類 = 種類を選ぶ(…)` | `jev · jev-1.13.0 · confidence 0.8 else 迷い` | 2 times every 1 second (混雑, 過負荷) | 10 seconds | `混雑`, `過負荷` → `on failure`<br>`迷い` → line 83<br>`timeout`, `failure` → `on failure` |
| 84 | `急 = 急ぎを測る(…)` | `jev · jev-1.13.0 · confidence 0.7 else 測れない` | — | — | `測れない` → line 85<br>`timeout`, `failure` → `on failure` |
| 86 | `読み = 読む(…)` | `jev · jev-1.13.0 · confidence 0.6 else 自信なし` | — | 10 seconds | `自信なし`, `timeout`, `failure` → `on failure` |

## Ends

Every way the workflow can end.

| Line | End |
|---:|---|
| 87 | `succeed 一通ごとの種類 = 一通ごとの種類, 種類 = 種類, 急ぎ = 急, 人 = 読み.人, 確かさ = 読み.確かさ` |
| 90 | `fail 読めない` "問い合わせを読み取れませんでした" |

