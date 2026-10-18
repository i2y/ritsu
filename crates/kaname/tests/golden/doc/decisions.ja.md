<!-- kaname <version> が tests/doc/decisions.kn（sha256:b5eecd8ea0b9c913）から生成したページです。読むためのもので、もとになるのは .kn のほうです。ここを編集しても .kn には戻せません。 -->
# decisions

What the page for people shows that the examples do not: a function checked when it runs, and decisions written in code. A material of kaname's tests, not an example to follow

- ファイル：`tests/doc/decisions.kn`（module decisions v1、sha256:b5eecd8ea0b9c913）
- 読むファイル：規則 `media_mail`（`examples/media_mail/rules/media_mail.rule`）
- kaname：<version>

上のファイルと、それが読むファイルを kaname <version> で検査して作ったページです。ファイルのハッシュが今のものと違えば、このページは古くなっています。

> [!NOTE]
> 関数 3 個: total 2 個、fails rule 1 個。呼ぶのは規則 1 個です。振替は組み立てません。コードに書いた判断が 1 個あります（下に挙げます）。

## 表にある判断

モジュールが使う判断は、読むファイルにあります。規則は rulec が、日付は koyomi が、帳簿は chobo が、それぞれ検査しています。モジュールは、その答えを運んでつなぎます。

### 規則 `media_mail`

`examples/media_mail/rules/media_mail.rule`。20 行目（`quote`）で呼んでいます。

<details>
<summary><code>rulec doc</code> が描いた <code>examples/media_mail/rules/media_mail.rule</code> のページ</summary>

[the page rulec doc draws of examples/media_mail/rules/media_mail.rule]

</details>

## 関数

関数は、エフェクトで三つに分かれます。total な関数は、宣言した範囲のどの入力にも答えます。実行時に確かめる関数には、検査が示せなかった場所があります。生成したコードはそこで値を確かめ、外れれば、その場所の失敗の種類（`rule` など）で失敗します。止まらないかもしれない関数は、再帰を持ちます。

### total な関数

#### `weight_of`

total な関数です。宣言した範囲のどの入力にも答えます。見出しにエフェクトは書いてありません。kaname が推論したものです。

```kn
fn weight_of(lines: list[Line] count >=0 <=30) -> mass[oz]
  sum(lines, l => l.weight × l.qty)
```

#### `shipping`

total な関数です。宣言した範囲のどの入力にも答えます。見出しにエフェクトは書いてありません。kaname が推論したものです。

```kn
fn shipping(total: money[USD] range >=0USD <=10_000USD) -> money[USD]
  if total >= 35USD then 0USD else 5USD
```

### 実行時に確かめる関数

#### `quote`

実行時に 1 か所を確かめ、`rule` で失敗することがあります。見出しにエフェクトは書いてありません。kaname が推論したものです。

The rule's price, where the weight is 15 pounds at most.

```kn
fn quote(lines: list[Line] count >=1 <=30) -> money[USDc]?
  description "The rule's price, where the weight is 15 pounds at most"
  let weight = weight_of(lines: lines)
  if weight <= 15lb
    media_mail(weight: weight, media_only: all(lines, l => l.media)).price
  else
    none
```

失敗しうる場所：

- 20 行目（`rule`）: `weight` が、規則 `media_mail` の入力 `weight` の範囲 >=1oz <=240oz に収まるかを決められません（W301）。範囲の解析では、値は >=0oz <=240oz です。関数のベクタにも、外れる入力はありませんでした。

### 止まらないかもしれない関数

ありません。

## コードにある判断

kaname は次を警告しています（W501）。どれも、比べた値によって業務の定数を選ぶ判断をコードに書いています。表に書けば、完全性と重なりを rulec が確かめ、承認する人がページで読めます。

- 25 行目: この `if` は、比べた値によって業務の定数（`0USD`、`5USD`）を選んでいます。表に書く判断です。

## 例

`kaname check` は、`examples` の行ごとに、ホストが呼ぶときと同じように関数を参照インタプリタで流し、書いてある答えと比べます。参照インタプリタの答えは次のとおりです。

```kn
shipping(total: 40USD)
  = 0USD
```
