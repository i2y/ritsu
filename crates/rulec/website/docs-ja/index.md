---
title: "表を書く。証明つきで出る。"
hide:
  - navigation
  - toc
---

<div class="rc-hero" markdown>
<img class="rc-hero__mark" src="images/mark.svg#only-dark" alt="">
<img class="rc-hero__mark" src="images/mark-light.svg#only-light" alt="">

# rulec

<p class="rc-hero__tag">表を書く。証明つきで出る。</p>

<p class="rc-hero__lede">
送料、クーポン、返品可否 — 条件が絡み合った業務の判断を、業務担当者が読める<strong>一枚の表</strong>として書きます。rulec はその表に<strong>抜けも矛盾も、決して使われない行も無い</strong>ことを証明してから、依存ゼロの普通の Python と Go を生成します。
<strong>証明はコードが存在する前に済みます</strong> — 証明できない規則は、そもそも生成されません。
</p>

<div class="rc-hero__cta" markdown>
[インストール](install.md){ .md-button .md-button--primary }
[表を書く](tour.md){ .md-button }
[エージェント向け](agents.md){ .md-button }
[GitHub](https://github.com/i2y/rulec){ .md-button }
</div>
</div>

## 表が入って、関数が出る

<div class="rc-flow" markdown>
<div markdown>
<p class="rc-flow__label">書くもの</p>

```
table 運賃表(fee_table)
policy unique
| あて先      | サイズ | -> 運賃(fee) : money[円, incl_tax] |
| 近畿圏      | S60    | 990円                              |
| 近畿圏      | S80    | 1310円                             |
| not: 近畿圏 | S60    | 880円                              |
| not: 近畿圏 | S80    | 1200円                             |
```
</div>
<div class="rc-flow__arrow">→</div>
<div markdown>
<p class="rc-flow__label">出るもの</p>

```python
def fee_demo(dest: Prefecture, girth: Cm) -> YenInclTax:
    """Rule 送料例 v1. Each branch corresponds 1:1 to a row."""
    if not _isinstance(dest, Prefecture):
        raise RuleInputError(...)
    ...
    if dest in KINKI and サイズ == SizeClass.S60:  # row 1
        運賃 = 990
    ...
    return _round_up(運賃, 10)
```
</div>
</div>

同じ表から Go のパッケージも出ます。どちらも**普通の関数**です — エンジンも設定も、標準ライブラリ以外の依存もありません。

---

## 人と、エージェントと、この道具

図に出てくる形は二種類だけです。**角の折れた紙**が受け渡されるもの（表、診断、質問、答え、生成物）、**色の帯がついたカード**がそれを作る・受け取る人と道具です。矢印は必ず「カード → 紙」か「紙 → カード」なので、**誰が何を出して、誰がそれを受け取るのか**が線の引かれ方そのものになっています。

![エージェントが表を書き、rulec が証明して直し方を返し、決められないことだけを人が決める。出てくるのは証明済みの Python と Go と、入れる前に分かる影響](images/flow-ja.svg#only-dark)

![エージェントが表を書き、rulec が証明して直し方を返し、決められないことだけを人が決める。出てくるのは証明済みの Python と Go と、入れる前に分かる影響](images/flow-ja-light.svg#only-light)

線の色が三つの道を分けています。**グレー**が外から入って外へ出ていくもの、**青**がエージェントと rulec のあいだのループ、**山吹色**が人を通る回り道です。

青のループが中心です。表を渡して検査させ、診断（どこが・どう直すか・それを起こす入力）を受け取って直し、また渡す — **通るまでこれを繰り返します**。人はここには出てきません。

人が呼ばれるのは**山吹色の道だけ**です。ふつうの道具は「分からなかった」を黙って通すか、それらしい既定値で埋めます。rulec は**そこで止まり、具体的な入力を一つ添えた質問にして渡します** — 「山梨県あての S60 の運賃はいくらですか」。人が答えるのは金額と丸めの向きだけで、コードは一行も見ません。答えは表に書き込まれて、青のループへ戻ります。

---

## 書き写した瞬間に、抜けが出ます

これが最初の実用価値です。日本郵便のゆうパック運賃表を書き写して、47 都道府県を 6 つのグループにまとめたとき、県をひとつ書き落とすと:

```
error[E101]: Completeness gap: some input matches no row
  --> rules/ゆうパック運賃.rule:34 table 運賃表
   |
34 | table 運賃表(fee_table)
   |       ^^^^^^ the input space is not fully covered
   |
 An input that matches no row: あて先 = 山梨県, サイズ = S60
 hint: add a row that matches this input.
```

**旧実装も過去データも要りません。** 手元の Excel を書き写して `rulec check` に掛けるだけで、抜けと重なりが出はじめます。しかもそのすべてに、**それを起こす具体的な入力**（証人）が付きます。

---

## 何が証明されるか

| | |
|---|---|
| **完全性** | どの入力にも当たる行がある。無ければ、当たらない入力そのものが出る |
| **重なり** | `policy unique` では二行が同じ入力に当たらない。`policy first` では、階段としてふつうに起きるシャドーイングと、人の判断が要る対とを区別する |
| **決して当たらない行** | 前の行にすっかり覆われている場合と、前の表がその値を決して出さない場合を書き分ける |
| **単位** | 円と g は足せない。税込と税抜も別の型 |
| **丸め** | 数値出力は端数の決め方を宣言しなければならない。文面は**丸め方で円がいくら動くか**を数字で見せる |
| **オーバーフロー** | 途中の値が int64 に収まることを、宣言した範囲から証明する |
| **例** | 全部の例を実行する。外れたら**当たった行**つき。出力の列が欠けていたら止まる |

近似はしません。証明できないときは、通さずにそう言います。

[何を証明するか（詳しく）](checks.md){ .md-button }

---

## そして「いま動いているものと同じか」

二つのコマンドが、**入れてみて後から数字を見る**を**入れる前に見る**に変えます。

```console
$ rulec verify rules/送料.rule --adapter python3 adapter.py --lang ja
照合 207 件 / 一致 182 (87.923%)

影響 25 件 (12.077%)  金額 -250
  表 サイズ判定 行1 / 表 運賃表 行36                 7 件  差 -10 一様  合計 -70
    例: あて先=沖縄県, 三辺合計=1, 重量=1 → 規則 運賃=1450 / 旧 運賃=1460
```

`verify` は旧実装と、`diff` は規則の二つの版を過去の記録に当てて、**何件がいくら動くか**を出します。食い違いは「どの行に当たったか」でグループにまとめられ、件数・金額差・証人が付きます。ずれが全部その出力の丸めの刻みより小さいグループには「丸め方の違いでは？」という札が付きます。

[突き合わせと再生](compare.md){ .md-button }

---

## 誰のための道具か

**第一の利用者は AI エージェントです。** 規約の文書や Excel や旧実装から `.rule` を書き、検査の言うことを直し、生成物を組み込み、影響を見せる — その一連を、`--help` と診断とその JSON だけで、人に訊かずに回せるように作ってあります。

人は二つの役で残ります。**表を承認する人** — 金額も、丸めの向きも、食い違う二つの読みのどちらが正しいかも業務の判断で、検査はそこを決めません。決められないことを、**具体的な例の入った質問**に変えるのが検査の仕事です。そして**生成物を組み込むアプリの持ち主**。

[エージェントの手順](agents.md){ .md-button .md-button--primary }
