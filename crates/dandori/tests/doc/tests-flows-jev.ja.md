# 仕分け v1

Jev のエッジケース：一部の値にだけ意味を書いた choice、score、はいといいえの意味を書いた noul、一回のリクエストで四つに答えるレコード（choice・score・noul と、確信度の割合）、確信度が下限を切ったときのエラーを処理する呼び出しと、処理せずに on failure へ行く呼び出し、下限を切った答えのあとも前の値が残る変数、並列のイテレーションの中の Jev、結果を読まない呼び出し、ステータスで宣言したエラーのリトライ

`tests/flows/jev.flow` を `dandori doc` で描いたものです。入力は `問い合わせ: list[string]`, `本文: string`, `会員: bool`、出力は `一通ごとの種類: list[種類]`, `種類: 種類`, `急ぎ: 急ぎ`, `人: bool`, `確かさ: rate[step 0.01%]` です。

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
    L3 -->|"すべてのイテレーションが終わったら"| s6
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

四角はタスク、両脇に線のある四角は規則、斜めの四角は外から値が届くタスク（イベントやコールバックの応答）、六角形は `match`、角の丸い四角は待ち、ステップを囲む枠はループです。破線の矢印は、呼び出しがその場で処理するエラーです。

## on failure

呼び出しが失敗し、そのエラーをその場で処理しないときに走ります（73・76・79・82・84・86 行目の呼び出しから）。最後まで走ると、ワークフローはそのエラーで失敗します。

```mermaid
flowchart TD
    onf(["on failure"])
    s14(["fail 読めない<br>#quot;問い合わせを読み取れませんでした#quot;"])
    onf --> s14
    classDef ok stroke:#2da44e,stroke-width:2px
    classDef bad stroke:#cf222e,stroke-width:2px
    class s14 bad
```

## 呼び出し

| 行 | 呼び出し | 呼ぶもの | リトライ | タイムアウト | 失敗したとき |
|---:|---|---|---|---|---|
| 73 | `人を求める(…)` | `jev · jev-1.13.0 · confidence 0.9 else 分からない` | — | — | `分からない` → 74 行目<br>`timeout`, `failure` → `on failure` |
| 76 | `k = 種類を選ぶ(…)` | `jev · jev-1.13.0 · confidence 0.8 else 迷い` | 1 秒おきに 2 回（混雑, 過負荷） | 10 秒 | `混雑`, `過負荷` → そのイテレーションが失敗し、`on failure` へ<br>`迷い` → 77 行目<br>`timeout`, `failure` → そのイテレーションが失敗し、`on failure` へ |
| 79 | `種類 = 種類を選ぶ(…)` | `jev · jev-1.13.0 · confidence 0.8 else 迷い` | 1 秒おきに 2 回（混雑, 過負荷） | 10 秒 | `混雑`, `過負荷` → `on failure`<br>`迷い` → 80 行目<br>`timeout`, `failure` → `on failure` |
| 82 | `種類 = 種類を選ぶ(…)` | `jev · jev-1.13.0 · confidence 0.8 else 迷い` | 1 秒おきに 2 回（混雑, 過負荷） | 10 秒 | `混雑`, `過負荷` → `on failure`<br>`迷い` → 83 行目<br>`timeout`, `failure` → `on failure` |
| 84 | `急 = 急ぎを測る(…)` | `jev · jev-1.13.0 · confidence 0.7 else 測れない` | — | — | `測れない` → 85 行目<br>`timeout`, `failure` → `on failure` |
| 86 | `読み = 読む(…)` | `jev · jev-1.13.0 · confidence 0.6 else 自信なし` | — | 10 秒 | `自信なし`, `timeout`, `failure` → `on failure` |

## 終わり方

ワークフローの終わり方のすべてです。

| 行 | 終わり方 |
|---:|---|
| 87 | `succeed 一通ごとの種類 = 一通ごとの種類, 種類 = 種類, 急ぎ = 急, 人 = 読み.人, 確かさ = 読み.確かさ` |
| 90 | `fail 読めない` "問い合わせを読み取れませんでした" |

