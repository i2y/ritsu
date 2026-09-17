<!-- `rulec explain --all --format markdown --lang ja` の出力です。手で編集しないでください。 -->

# rulec の診断

rulec が出しうるコードの全部と、いつ出るか、どう直すか。コードと JSON の形は安定 API で、文面だけが良くなります（DESIGN §11 原則 5）。一件だけ読むには `rulec explain E101`。

| コード | 種別 | 見出し |
|---|---|---|
| [E001](#e001) | error | 文字列が閉じていません |
| [E002](#e002) | error | 読めない文字があります |
| [E003](#e003) | error | ファイルが `rule` の行で始まっていません |
| [E004](#e004) | error | 行の先頭に語がありません |
| [E005](#e005) | error | この位置に書けない語です |
| [E006](#e006) | error | 宣言に `=` がありません |
| [E007](#e007) | error | そういう方式はありません |
| [E008](#e008) | error | 空のセルがあります |
| [E009](#e009) | error | 宣言の名前が予約語と衝突しています |
| [E010](#e010) | error | `..` を使った範囲記法は書けません |
| [E011](#e011) | error | 公開面の名前に ASCII 別名がありません |
| [E012](#e012) | error | 宣言されていない名前です |
| [E013](#e013) | error | 取込先がありません |
| [E014](#e014) | error | 出力のセルに式は書けません |
| [E015](#e015) | error | `result` が書けるのは最初の出力だけです |
| [E016](#e016) | error | `result` は一つしか書けません |
| [E017](#e017) | error | `constraint` の形が違います |
| [E018](#e018) | error | `constraint` は入力どうしの関係です |
| [E019](#e019) | error | 例が制約を破っています |
| [E020](#e020) | error | `elements` の宣言が正しくありません |
| [E021](#e021) | error | `fold` の書き方が正しくありません |
| [E022](#e022) | error | 要素がゼロ件のときの答えが宣言されていません |
| [E023](#e023) | error | 最後まで見終えたときの答えが宣言されていません |
| [E024](#e024) | error | 腕の無い判定があります |
| [E025](#e025) | error | 畳み込みのある規則には、まだ例を書けません |
| [E101](#e101) | error | 完全性の欠落: どの行にも当てはまらない入力があります |
| [E102](#e102) | error | どの入力にも当てはまらない行があります |
| [E103](#e103) | error | 単位の混同: 型の違う値を混ぜています |
| [E104](#e104) | error | 数値の出力に丸めの宣言がありません |
| [E105](#e105) | error | 行の重なり: 同じ入力が二つ以上の行に当てはまります |
| [E106](#e106) | error | 出力のリテラルが丸めの刻みに載っていません |
| [E107](#e107) | error | 例の期待値と一致しません |
| [E108](#e108) | error | 中間値が int64 に収まることを証明できません |
| [E109](#e109) | error | 検査の予算を超えたので、完全性を証明できませんでした |
| [E110](#e110) | error | 検査できない型の列があります |
| [E111](#e111) | error | 例に出力の列がありません |
| [E112](#e112) | error | 導出の範囲が、実際に到達しうる値を含んでいません |
| [E113](#e113) | error | 真偽定義の条件が、許された二形のどちらでもありません |
| [E114](#e114) | error | セルの値が列の刻みに載っていません |
| [E115](#e115) | error | 変数では割れません |
| [W105](#w105) | warning | 要確認の隠れ: 先の行が後の行の一部を隠しています |
| [W110](#w110) | warning | 重なりのない `first` です |
| [W111](#w111) | warning | 使われていない宣言があります |
| [W115](#w115) | warning | どの要素もこの判定にはなりません |
| [W114](#w114) | warning | 未確認の重なり: 両方に当てはまる入力が有り得ます |

## E001

`error` — **文字列が閉じていません**

**いつ出るか。** `"` で開いた文字列が、その行のうちに閉じていないとき。セルの中にも `description` にも改行は書けません。

**直し方。** その行のうちに `"` を閉じてください。長い説明は一行に収めるか、`#` のコメントに移します。

**最小の再現**:

```rule
rule t(t) v1
description "unterminated
```

関係するコード: [E002](#e002)

## E002

`error` — **読めない文字があります**

**いつ出るか。** 名前の位置に、文字でも `_` でもない文字（記号や制御文字）があるとき。打ち間違いが黙って名前になるのを防ぐための検査です。

**直し方。** その文字を消してください。名前は文字か `_` で始まります（`@x(x)` なら `x(x)`）。

**最小の再現**:

```rule
rule t(t) v1

inputs
  @x(x) : bool
```

関係するコード: [E001](#e001), [E009](#e009)

## E003

`error` — **ファイルが `rule` の行で始まっていません**

**いつ出るか。** 一つの `.rule` は一つの規則で、先頭行が規則名と版です。空行とコメントより前に他の宣言があるとき。

**直し方。** 先頭に `rule 規則名(alias) v1` の一行を足してください。

**最小の再現**:

```rule
inputs
  x(x) : bool
```

関係するコード: [E004](#e004), [E011](#e011)

## E004

`error` — **行の先頭に語がありません**

**いつ出るか。** 表でもコメントでも空行でもない行が、記号で始まっているとき。この構文は行指向なので、行の先頭の語が何の宣言かを決めます。

**直し方。** 行頭に宣言の語を書いてください（`description / import / enum / group / inputs / elements / outputs / derive / define / constraint / table / fold / result / examples / policy`）。表の行なら `|` で始めます。

**最小の再現**:

```rule
rule t(t) v1

= 1
```

関係するコード: [E003](#e003), [E005](#e005)

## E005

`error` — **この位置に書けない語です**

**いつ出るか。** 行頭の語が語彙にないとき。語彙には同義の綴りがなく、英語の一種類だけです（§1.1）。

**直し方。** `description / import / enum / group / inputs / elements / outputs / derive / define / constraint / table / fold / result / examples / policy` のどれかに直してください。業務の語は名前とセルの中にだけ書きます。

**最小の再現**:

```rule
rule t(t) v1

foo bar
```

関係するコード: [E004](#e004), [E009](#e009)

## E006

`error` — **宣言に `=` がありません**

**いつ出るか。** `enum` `group` `derive` `define` `result` は、名前と中身を `=` で分けます。その `=` が無いとき。

**直し方。** `=` を入れてください。例: `enum k(k) = a(a) | b(b)`。

**最小の再現**:

```rule
rule t(t) v1

enum k(k) a(a) | b(b)
```

関係するコード: [E005](#e005)

## E007

`error` — **そういう方式はありません**

**いつ出るか。** `policy` の後ろが unique と first 以外のとき。DMN の Any / Priority / Collect は採っていません（§4）。

**直し方。** `policy unique`（重なりはすべてエラー）か `policy first`（先に書いた行が勝つ）に直してください。

**最小の再現**:

```rule
rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : bool

table j(j)
policy any
| x | -> r(r) : bool |
| - | true |
```

関係するコード: [W110](#w110), [E105](#e105), [W105](#w105)

## E008

`error` — **空のセルがあります**

**いつ出るか。** 表のセルが空白だけのとき。**空欄は書き忘れと区別がつかない**ので構文エラーにしています（§3）。

**直し方。** 任意の値のつもりなら `-` と書いてください。値を書き忘れていたなら、その値を書きます。

**最小の再現**:

```rule
rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : bool

table j(j)
policy unique
| x | -> r(r) : bool |
|   | true |
```

関係するコード: [E101](#e101)

## E009

`error` — **宣言の名前が予約語と衝突しています**

**いつ出るか。** 宣言した名前（や別名）が語彙の語と同じとき。行指向のパーサがその行をセクションの始まりと読んで、宣言を黙って捨てる事故を防ぎます。

**直し方。** 名前を変えてください（`enum range(kind)` なら `enum 範囲区分(range_kind)`）。予約語は `src/kw.rs` の一枚の表で決まっています。

**最小の再現**:

```rule
rule t(t) v1

enum range(kind) = a(a) | b(b)
```

関係するコード: [E005](#e005), [E011](#e011)

## E010

`error` — **`..` を使った範囲記法は書けません**

**いつ出るか。** セルに `0g..1000g` のような `..` があるとき。「1000g まで」が両端を含むのか含まないのかが、書いてある字から読めないためです（§3.1）。

**直し方。** 比較演算子で書き直してください。`0g..1000g` は `<=1000g` か `<1000g` のどちらかです。両端を決めたいなら `>=0g <=1000g` と並記します。

**最小の再現**:

```rule
rule t(t) v1

inputs
  w(w) : mass[g]  range >=0g <=10kg

outputs
  r(r) : bool

table j(j)
policy unique
| w | -> r(r) : bool |
| 0g..1000g | true |
| >1000g | false |
```

関係するコード: [E105](#e105), [E101](#e101)

## E011

`error` — **公開面の名前に ASCII 別名がありません**

**いつ出るか。** 規則名・入力・出力に、**ASCII でない名前**が付いていて括弧の中の別名も無いとき。別名は生成コードの公開名になります（漢字は大文字を持てず、Go の公開識別子になれません）。名前がもとから ASCII なら、それ自身が公開名になるので別名は要りません。

**直し方。** 括弧で別名を足してください（`重量 : bool` なら `重量(weight) : bool`）。導出・定義・群・表の別名は任意で、書けば生成コードがその名前を使い、書かなければ宣言した名前をそのまま使います。

**最小の再現**:

```rule
rule t(t) v1

inputs
  重量 : bool

outputs
  r(r) : bool

table j(j)
policy unique
| 重量 | -> r(r) : bool |
| - | true |
```

関係するコード: [E009](#e009), [E012](#e012)

## E012

`error` — **宣言されていない名前です**

**いつ出るか。** 表の列や式が、どこにも宣言されていない名前を指しているとき。前方参照は書けないので、名前はそれを使う行より上で宣言します。

**直し方。** 綴りを宣言に合わせるか、その入力・導出・定義を上に宣言してください。

**最小の再現**:

```rule
rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : bool

table j(j)
policy unique
| y | -> r(r) : bool |
| - | true |
```

関係するコード: [E011](#e011), [E013](#e013)

## E013

`error` — **取込先がありません**

**いつ出るか。** `import` の先が組み込みの名前空間に無いとき。いまあるのは `std/都道府県`（47 値）だけです。

**直し方。** `import std/都道府県` に直すか、その列挙を `enum` でこのファイルに書いてください。

**最小の再現**:

```rule
rule t(t) v1

import std/nope
```

関係するコード: [E012](#e012)

## E014

`error` — **出力のセルに式は書けません**

**いつ出るか。** 表の `->` から右のセルに語が二つ以上あるとき。書けるのは値一つか名前一つだけです（§3.2）。読み飛ばして最初の語だけを採ると、かけ算が黙って消えた生成コードが出ます。

**直し方。** 計算に名前を付けて `define` の行へ出し、表にはその名前だけを書いてください（`| - | 率割引 |`）。表は分岐だけを持ちます。

**最小の再現**:

```rule
rule t(t) v1

inputs
  p(p) : money[円, incl_tax]  range >=0円 <=1万円
  r(r) : rate[step 1%]  range >=0% <=100%

outputs
  o(o) : money[円, incl_tax]  round down(1円)

table j(j)
policy first
| r | -> o(o) : money[円, incl_tax] |
| <=5% | 0円 |
| - | p × r |
```

関係するコード: [E008](#e008), [E012](#e012)

## E015

`error` — **`result` が書けるのは最初の出力だけです**

**いつ出るか。** `result` が二つ目以降の出力を名指ししたとき。`result` は最初の出力のための糖衣で、評価器も生成コードもそこにしか当てません（§1.2）。名指しが効かないまま通っていたので、`number` が `money` の枠に入っても E103 が出ませんでした。

**直し方。** その出力と同じ名前の `define` を書いてください（`define 付与点(pts) : number = 基本点 × 倍率`）。出力は宣言順に、同じ名前の束縛から取られます。`result` で組み立てたいなら、その出力を `outputs` の先頭へ移します。

**最小の再現**:

```rule
rule t(t) v1

inputs
  p(p) : money[円, incl_tax]  range >=0円 <=1万円

outputs
  a(a) : money[円, incl_tax]  round down(1円)
  b(b) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| p | -> a(a) : money[円, incl_tax] |
| - | 100円 |

result b = p
```

関係するコード: [E016](#e016), [E103](#e103)

## E016

`error` — **`result` は一つしか書けません**

**いつ出るか。** 一つのファイルに `result` の行が二本以上あるとき。組み立てられるのは最初の出力だけなので、二本目は一本目を置き換えるだけになります。以前はそれが黙って起きていました。

**直し方。** 一本だけ残してください。ほかの出力は、その出力と同じ名前の `define` から取ります。

**最小の再現**:

```rule
rule t(t) v1

inputs
  p(p) : money[円, incl_tax]  range >=0円 <=1万円

outputs
  a(a) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| p | -> a(a) : money[円, incl_tax] |
| - | 100円 |

result a = p
result a = p + 100円
```

関係するコード: [E015](#e015)

## E017

`error` — **`constraint` の形が違います**

**いつ出るか。** `constraint` の行が「入力 比較 入力」になっていないとき。比較が無い、片側が名前一つでない、のどちらかです。

**直し方。** `constraint <入力> <= <入力>` の形にしてください。比較は `<=` `<` `>=` `>` の四つです。`A = B` を言いたいなら、`A <= B` と `A >= B` の二行に分けます。

**最小の再現**:

```rule
rule t(t) v1

inputs
  a(a) : number  range >=0 <=10
  b(b) : number  range >=0 <=10

constraint a

outputs
  r(r) : bool

table j(j)
policy unique
| a | -> r(r) : bool |
| - | true |
```

関係するコード: [E018](#e018)

## E018

`error` — **`constraint` は入力どうしの関係です**

**いつ出るか。** `constraint` の片側が入力でないか、順序の無い型のとき。制約は「呼び出し側が渡す値の組み合わせのうち、どれが起きるか」を言うものなので、両側とも `inputs` の名前で、金額・数量・率・number・日付のいずれかです。

**直し方。** 両側を `inputs` の名前にしてください。導出や定義は入力から計算されるので、関係は元の入力どうしで書きます。列挙や真偽に大小はありません。

**最小の再現**:

```rule
rule t(t) v1

inputs
  a(a) : number  range >=0 <=10

constraint a <= r

outputs
  r(r) : bool

table j(j)
policy unique
| a | -> r(r) : bool |
| - | true |
```

関係するコード: [E017](#e017), [W111](#w111)

## E019

`error` — **例が制約を破っています**

**いつ出るか。** 例の入力が `constraint` を満たしていないとき。制約は「この組み合わせは起きない」という宣言で、完全性の検査はそれを信じてその升目に行を要求していません。生成コードもその入力を入口で断ります。答えを主張できない入力です。

**直し方。** 例の値を直してください。その組み合わせが本当に起きるなら、制約のほうが間違っているので消します。

**最小の再現**:

```rule
rule t(t) v1

inputs
  a(a) : number  range >=0 <=10
  b(b) : number  range >=0 <=10

constraint a <= b

outputs
  r(r) : bool

table j(j)
policy unique
| a | b | -> r(r) : bool |
| - | - | true |

examples
| a | b | -> r |
| 5 | 1 | true |
```

関係するコード: [E017](#e017), [E018](#e018), [E101](#e101)

## E020

`error` — **`elements` の宣言が正しくありません**

**いつ出るか。** `elements` に名前が無いか、二本あるとき。規則が歩く列は一つで、その一要素ぶんの欄をそこに書きます（§15.56）。

**直し方。** `elements 運賃行(fee_rows)` の形にして、続く行に一要素ぶんの欄を `inputs` と同じように書いてください。列が二つ要るなら、それは別の規則です。

**最小の再現**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=0 <=10

elements xs(xs)
  k(k) : number  range >=0 <=10

elements ys(ys)
  m(m) : number  range >=0 <=10

outputs
  r(r) : bool

table j(j)
policy unique
| n | -> r(r) : bool |
| - | true |
```

関係するコード: [E021](#e021)

## E021

`error` — **`fold` の書き方が正しくありません**

**いつ出るか。** `fold <判定の列> over <列の名前>` になっていないか、腕が `next` `stop` `stop with <値>` `take_unique <値>` `take_first <値>` `keep_max <値> by <鍵>` のどれでもないか、畳もうとしている列が列挙でないとき。

**直し方。** 見出しと腕を上の形に直してください。`take` とだけ書くことはできません。**一件だけ採るのか、最初の一件を採るのか**は、書く人が選ぶことだからです（§15.56）。

**最小の再現**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=0 <=10

elements xs(xs)
  k(k) : number  range >=0 <=10

outputs
  r(r) : bool

table j(j)
policy unique
| n | -> r(r) : bool |
| - | true |

fold r
  empty -> false
```

関係するコード: [E020](#e020), [E022](#e022), [E023](#e023), [E024](#e024)

## E022

`error` — **要素がゼロ件のときの答えが宣言されていません**

**いつ出るか。** `fold` に `empty -> <値>` が無いとき。空の列は必ず来ます。手で書いた走査がいちばんよく落とすのがこの場合で、たいていは最初の要素をそのまま読んで落ちます。

**直し方。** `empty -> <値>` を足してください。何を返すかは業務の判断で、道具が決められることではありません。

**最小の再現**:

```rule
rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[円, incl_tax]  range >=0円 <=10円

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| k | -> d(d) : v |
| <=5円 | a |
| >5円 | b |

fold d over xs
  a -> next
  b -> take_first k
  exhausted -> held
```

関係するコード: [E023](#e023), [E024](#e024)

## E023

`error` — **最後まで見終えたときの答えが宣言されていません**

**いつ出るか。** `fold` に `exhausted -> <値>` が無いとき。どの要素も打ち切らずに列が尽きた場合の答えです。保持していた暫定の値をそのまま返すつもりでも、それは書いて初めて決まります。

**直し方。** `exhausted -> <値>` を足してください。保持しているものを返すなら `exhausted -> held` です。

**最小の再現**:

```rule
rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[円, incl_tax]  range >=0円 <=10円

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| k | -> d(d) : v |
| <=5円 | a |
| >5円 | b |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0円
```

関係するコード: [E022](#e022), [E024](#e024)

## E024

`error` — **腕の無い判定があります**

**いつ出るか。** 表が出しうる判定のどれかに、`fold` の腕が無いとき。その判定の要素が来たら、歩き方が決まっていません。表の完全性と同じ検査を、畳み込みの側に当てたものです（§15.56）。

**直し方。** 腕を足すか、表がその値を出さないようにしてください。逆に、どの要素も辿り着けない判定に腕があるときは W115 が出ます。

**最小の再現**:

```rule
rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[円, incl_tax]  range >=0円 <=10円

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| k | -> d(d) : v |
| <=5円 | a |
| >5円 | b |

fold d over xs
  a -> next
  empty -> 0円
  exhausted -> held
```

関係するコード: [E022](#e022), [E023](#e023), [W115](#w115)

## E025

`error` — **畳み込みのある規則には、まだ例を書けません**

**いつ出るか。** `fold` のある規則に `examples` があるとき。例の一行はセルの並びで、要素の列を一つのセルに書く形がまだ決まっていません（§15.56）。

**直し方。** 例をいったん外してください。表そのものの検査（完全性・重なり・単位・オーバーフロー）と、畳み込みの四つの検査は、例が無くても効きます。生成も同じ段で、いまは `gen` が名前を挙げて断ります。

**最小の再現**:

```rule
rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[円, incl_tax]  range >=0円 <=10円

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| k | -> d(d) : v |
| <=5円 | a |
| >5円 | b |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0円
  exhausted -> held

examples
| k | -> r |
| 3円 | 3円 |
```

関係するコード: [E021](#e021)

## E101

`error` — **完全性の欠落: どの行にも当てはまらない入力があります**

**いつ出るか。** 行を全部合わせても、宣言した範囲の入力を覆いきれていないとき。完全性は宣言で外せず、常に必須です（§4）。当てはまらない入力の具体例が必ず付きます。

**直し方。** それを起こす入力に当てはまる行を足してください。列挙の値が増えたのが原因なら、その値の行か、全部を受ける `-` の行を足します。値に専用の行が要らないなら、列挙の宣言に `default` を付けます。

**最小の再現**:

```rule
rule t(t) v1

enum k(k) = a(a) | b(b) | c(c)

inputs
  x(x) : k

outputs
  r(r) : bool

table j(j)
policy unique
| x | -> r(r) : bool |
| a | true |
| b | false |
```

関係するコード: [E102](#e102), [E105](#e105), [W111](#w111)

## E102

`error` — **どの入力にも当てはまらない行があります**

**いつ出るか。** 先行する行にすべて覆われているか、上流の表が決して出さない値を名指ししているとき。二形あり、文面が原因を書き分けます。

**直し方。** その行が新しい仕様なら、覆っている行より上へ移してください。不要なら削除します。上流が出さない値を指しているなら、上流の表にその値を出す行を足すか、この行を消します。

**最小の再現**:

```rule
rule t(t) v1

inputs
  w(w) : mass[g]  range >=0g <=10kg

outputs
  r(r) : bool

table j(j)
policy first
| w | -> r(r) : bool |
| - | true |
| <=1000g | false |
```

関係するコード: [E101](#e101), [W105](#w105), [W110](#w110)

## E103

`error` — **単位の混同: 型の違う値を混ぜています**

**いつ出るか。** 式やセルで、単位・通貨・税区分の違う値を足したり比べたりしているとき。`money[円, incl_tax]` と `money[円, excl_tax]` も別物です（§2.3）。

**直し方。** 混ぜている片方を表に移してください。「重量に応じた加算料金」なら `table 重量加算 | 重量 | -> 加算額 : money[円, incl_tax] |` の形です。税の変換も、式ではなく表として書きます。

**最小の再現**:

```rule
rule t(t) v1

inputs
  w(w) : mass[g]  range >=0g <=10kg
  p(p) : money[円, incl_tax]  range >=0円 <=1万円

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| w | -> r(r) : money[円, incl_tax] |
| - | 100円 |

result r = p + w
```

関係するコード: [E108](#e108), [E112](#e112)

## E104

`error` — **数値の出力に丸めの宣言がありません**

**いつ出るか。** 数量・金額・率の出力に `round` が無いとき。端数がどう決まるかを宣言しないと、生成コードが黙って決めてしまいます。式が端数を生む場合は、丸め方で円がいくら動くかを数字で見せます。

**直し方。** 出力の宣言に丸めを書いてください。例: `round up(10円)`。向きは五種（`up` `down` `half_up` `half_down` `half_even`）で、負の側まで固定されています（§7.3）。

**最小の再現**:

```rule
rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : money[円, incl_tax]

table j(j)
policy unique
| x | -> r(r) : money[円, incl_tax] |
| true | 100円 |
| false | 200円 |
```

関係するコード: [E106](#e106), [E103](#e103)

## E105

`error` — **行の重なり: 同じ入力が二つ以上の行に当てはまります**

**いつ出るか。** `policy unique` の表で、両方に当てはまる入力を実際に構成できたとき。構成できなかった重なりは W114 に落ちます。

**直し方。** 出力が違うなら、どちらが正しいか決めて行を直してください。順序に意味を持たせたいなら `policy first` を宣言します。出力まで同じなら、片方を削ります。

**最小の再現**:

```rule
rule t(t) v1

enum k(k) = a(a) | b(b)

inputs
  x(x) : k
  y(y) : bool

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| x | y | -> r(r) : money[円, incl_tax] |
| a | - | 100円 |
| - | true | 200円 |
| b | false | 300円 |
```

関係するコード: [W105](#w105), [W114](#w114), [E102](#e102)

## E106

`error` — **出力のリテラルが丸めの刻みに載っていません**

**いつ出るか。** 出力セルに書かれたリテラルが、宣言した丸めの刻みの倍数でないとき。`round up(10円)` の表に `1451円` があるような、桁の打ち間違いをここで落とします（§7.2）。

**直し方。** リテラルを刻みに載せてください（`1451円` は `1450円` か `1460円`）。その額が本当に正しいなら、丸めの刻みのほうを直します。

**最小の再現**:

```rule
rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : money[円, incl_tax]  round up(10円)

table j(j)
policy unique
| x | -> r(r) : money[円, incl_tax] |
| true | 1451円 |
| false | 1000円 |
```

関係するコード: [E104](#e104)

## E107

`error` — **例の期待値と一致しません**

**いつ出るか。** `examples` の行を参照評価器で走らせた結果が、書かれた期待値と違うとき。**どの表のどの行が発火したか**が付きます。`examples` は実行される仕様です。

**直し方。** 表が正しいなら期待値を直してください。期待値が業務の真実なら、発火した行のほうを直します。どちらを直すかは、出典（規約・Excel・旧実装）が決めます。

**最小の再現**:

```rule
rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| x | -> r(r) : money[円, incl_tax] |
| true | 100円 |
| false | 200円 |

examples
| x | -> r |
| true | 200円 |
```

関係するコード: [E111](#e111), [E105](#e105)

## E108

`error` — **中間値が int64 に収まることを証明できません**

**いつ出るか。** 宣言した範囲と刻みから計算した到達区間が、int64 を超えるとき。率の刻みが 1% なら格納される整数は 100 倍になります。

**直し方。** 入力の範囲を狭めるか、途中に丸めを一つ入れてください。どこで丸めるかは円が動く業務の判断なので、道具は勝手に決めません（§7.1）。

**最小の再現**:

```rule
rule t(t) v1

inputs
  p(p) : money[円, incl_tax]  range >=0円 <=100000000000000000円
  q(q) : rate[step 1%]  range >=0% <=100%

outputs
  r(r) : money[円, incl_tax]  round down(1円)

define off(off) : money[円, incl_tax] = p × q

table j(j)
policy unique
| off | -> r(r) : money[円, incl_tax] |
| - | 0円 |
```

関係するコード: [E112](#e112), [E103](#e103)

## E109

`error` — **検査の予算を超えたので、完全性を証明できませんでした**

**いつ出るか。** 領域検査が訪れたノード数が `--budget` を超えたとき。**証明できなかったことを緑にはしない**ので、警告ではなくエラーです。

**直し方。** 表を分けて列の数を減らすか、`--budget` を上げてください。列の積が効くので、一つの表に列を積むより、線形パイプラインで表を連ねるほうが安く済みます（§5.1）。

**最小の再現**（`--budget 1` で）:

```rule
rule t(t) v1

enum k(k) = a(a) | b(b) | c(c)

inputs
  x(x) : k

outputs
  r(r) : bool

table j(j)
policy unique
| x | -> r(r) : bool |
| a | true |
| b | false |
| c | true |
```

関係するコード: [E101](#e101), [W114](#w114)

## E110

`error` — **検査できない型の列があります**

**いつ出るか。** その列の型を領域 IR に落とせないとき。**これが出たら rulec 自身のバグです。** 表の検査が黙って素通りするのを防ぐ内部の防波堤で、日付と optional で二度起きた事故を類として塞いだものです（§6.3）。

**直し方。** その列を、いま検査できる型（真偽・列挙・数量・金額・率・日付・optional）に直してください。そのうえで報告してください — 素通りより止まるほうが正しいという判断でこの検査があります。

**最小の再現**:

```rule
rule t(t) v1

inputs
  s(s) : string

outputs
  r(r) : bool

table j(j)
policy unique
| s | -> r(r) : bool |
| "a" | true |
```

関係するコード: [E101](#e101), [E105](#e105)

## E111

`error` — **例に出力の列がありません**

**いつ出るか。** `examples` が、宣言した出力の一部しか書いていないとき。実装どうしの照合は、生成物が揃って同じ誤りを持つと緑のままなので、**それを破れるのは人の書いた期待値だけ**です。実際に複数出力の丸めが、どの言語でも揃って抜けたことがあります。

**直し方。** 欠けている出力の列を `examples` に足してください。出力が二つ以上あるときは、二列目以降に `->` を書いても書かなくても構いません。

**最小の再現**:

```rule
rule t(t) v1

inputs
  x(x) : bool

outputs
  ok(ok) : bool
  fee(fee) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| x | -> ok(ok) : bool | fee(fee) : money[円, incl_tax] |
| true | true | 100円 |
| false | false | 0円 |

examples
| x | -> ok |
| true | true |
```

関係するコード: [E107](#e107)

## E112

`error` — **導出の範囲が、実際に到達しうる値を含んでいません**

**いつ出るか。** 入力の範囲から計算した到達区間が、導出に宣言した `range` からはみ出すとき。範囲が狭いと、完全性検査が実際に起きる値を見ないまま「完全」と答えます。

**直し方。** 文面が示す到達区間まで `range` を広げてください（`range >=-110万円 <=100万円` の形で書いてあります）。到達しない分まで広げても、実現不能な領域として検査が篩うので害はありません。

**最小の再現**:

```rule
rule t(t) v1

inputs
  a(a) : money[円, incl_tax]  range >=0円 <=100万円
  b(b) : money[円, incl_tax]  range >=0円 <=100万円

outputs
  r(r) : bool

derive gap(gap) : money[円, incl_tax] = a - b  range >=0円 <=100万円

table j(j)
policy unique
| gap | -> r(r) : bool |
| <=0円 | false |
| >0円 | true |
```

関係するコード: [E108](#e108), [E101](#e101)

## E113

`error` — **真偽定義の条件が、許された二形のどちらでもありません**

**いつ出るか。** `define … : bool` の条件が、「入力か導出の値ひとつを定数と比べる」形でも、「引き算で差を取れない型どうしの比較」（日付どうしなど）でもないとき。数値どうしを直接比べたときがこれに当たります（§5.3）。

**直し方。** 差を導出として宣言してから定数と比べてください。`define bigger : bool = a >= b` は `derive gap(gap) : money[円, incl_tax] = a - b  range …` を足して `| gap | >=0円 |` と書き換えます。そのほうが厳密に解析できます。

**最小の再現**:

```rule
rule t(t) v1

inputs
  a(a) : money[円, incl_tax]  range >=0円 <=100万円
  b(b) : money[円, incl_tax]  range >=0円 <=100万円

outputs
  r(r) : bool

define bigger(bigger) : bool = a >= b

table j(j)
policy unique
| bigger | -> r(r) : bool |
| true | true |
| false | false |
```

関係するコード: [E112](#e112), [E103](#e103)

## E114

`error` — **セルの値が列の刻みに載っていません**

**いつ出るか。** `rate[step 1%]` の列に `0.5%` のように、宣言した刻みの整数倍でない値が書かれたとき。実行時の値はその刻みの整数一本なので（§2.1）、この値には表し方がありません。

**直し方。** 刻みに載る値に直すか、型の刻みを細かくしてください（`rate[step 0.1%]`）。黙って近い刻みに寄せると、表で読める境界と生成コードの境界が食い違います。

**最小の再現**:

```rule
rule t(t) v1

inputs
  r(r) : rate[step 1%]  range >=0% <=100%

outputs
  o(o) : bool

table j(j)
policy first
| r | -> o(o) : bool |
| <=0.5% | true |
| - | false |
```

関係するコード: [E103](#e103), [E106](#e106)

## E115

`error` — **変数では割れません**

**いつ出るか。** `÷` の右が定数でないとき。割る数は正の整数の定数か、同じ単位の金額・数量の定数だけです（§2.3）。

**直し方。** 割る数が業務のデータなら、率として入力に取るか、定数を引く表として書いてください。刻みが静的に決まらないと生成コードは言語の除算に頼ることになり、Python は −∞ 方向、Go は 0 方向に丸めて答えが食い違います（§7.1）。

**最小の再現**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=0 <=100
  d(d) : number  range >=1 <=100

outputs
  o(o) : bool

define r(r) : number = n ÷ d

table j(j)
policy first
| r | -> o(o) : bool |
| - | true |
```

関係するコード: [E103](#e103), [E108](#e108)

## W105

`warning` — **要確認の隠れ: 先の行が後の行の一部を隠しています**

**いつ出るか。** `policy first` の表で、一部だけ重なっていて出力が違う行の対があるとき。階段状の隠れと、答えが同じ隠れは件数の注記に畳まれ、ここに一覧されるのは要確認の対だけです（§4）。

**直し方。** 意図どおりならこのままで構いません（CI の `check --diff-base` は新たに生じた対だけを報告します）。後の行を優先したいなら、その行を先の行より上へ移してください。全対を見るには `--show-shadow` を付けます。

**最小の再現**:

```rule
rule t(t) v1

enum k(k) = a(a) | b(b)

inputs
  x(x) : k
  y(y) : bool

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy first
| x | y | -> r(r) : money[円, incl_tax] |
| a | - | 100円 |
| - | true | 200円 |
| - | - | 300円 |
```

関係するコード: [E105](#e105), [W110](#w110), [E102](#e102)

## W110

`warning` — **重なりのない `first` です**

**いつ出るか。** `policy first` なのに、どの二行も重ならないとき。順序に意味が無いので、`unique` のほうが強い保証になります。

**直し方。** `policy unique` に変えてください。並べ替えても意味が変わらないことが、以後の検査で守られます。

**最小の再現**:

```rule
rule t(t) v1

enum k(k) = a(a) | b(b)

inputs
  x(x) : k

outputs
  r(r) : bool

table j(j)
policy first
| x | -> r(r) : bool |
| a | true |
| b | false |
```

関係するコード: [W105](#w105), [E105](#e105)

## W111

`warning` — **使われていない宣言があります**

**いつ出るか。** 入力・導出・グループ・列挙の値が、どの表のどのセルにも現れないとき。書き忘れのしるしであることも、意図した契約であることもあります。取込した型の値は対象外です。

**直し方。** 入力が範囲の入口検査としてだけ効いているなら、宣言に `contract_only` を付けて黙らせてください。列挙の値が既定行に吸われるのが正しいなら、値の宣言に `default` を付けます。どちらでもないなら、その列を表に足すか、宣言を消します。

**最小の再現**:

```rule
rule t(t) v1

inputs
  x(x) : bool
  w(w) : mass[g]  range >=0g <=10kg

outputs
  r(r) : bool

table j(j)
policy unique
| x | -> r(r) : bool |
| true | true |
| false | false |
```

関係するコード: [E101](#e101), [E012](#e012)

## W115

`warning` — **どの要素もこの判定にはなりません**

**いつ出るか。** `fold` に腕があるのに、その判定をどの行も出さないとき。E024 の裏返しで、こちらは穴ではなく届かない腕です。書き忘れではなく、表のほうが変わった跡であることが多い（§15.56）。

**直し方。** 表の行を見直すか、その腕を消してください。どちらが正しいかは表のほうを読まないと決まりません。

**最小の再現**:

```rule
rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[円, incl_tax]  range >=0円 <=10円

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| k | -> d(d) : v |
| - | a |

fold d over xs
  a -> take_first k
  b -> next
  empty -> 0円
  exhausted -> held
```

関係するコード: [E024](#e024)

## W114

`warning` — **未確認の重なり: 両方に当てはまる入力が有り得ます**

**いつ出るか。** `policy unique` の表で二行が重なりうるが、それを実際に起こす入力を構成できず、実現不能の証明もできなかったとき。導出どうしが入力を共有していると起こります（独立な区間の篩はその従属を見ません）。

**直し方。** その条件を同時に満たす注文が存在するなら、行を直してください（出力が違うので、当てはまれば矛盾です）。存在しないならこのままで構いません — 生成コードには、万一その条件に当てはまる入力が来たとき黙って先の行を選ばずエラーを返すガードが入ります。

**最小の再現**:

```rule
rule t(t) v1

inputs
  total(total) : money[円, incl_tax]  range >=0円 <=100万円
  d1(d1) : money[円, incl_tax]  range >=0円 <=100万円
  d2(d2) : money[円, incl_tax]  range >=0円 <=100万円

outputs
  r(r) : bool

derive restA(rest_a) : money[円, incl_tax] = total - d1  range >=-100万円 <=100万円
derive restB(rest_b) : money[円, incl_tax] = total - d1 - d2  range >=-200万円 <=100万円

table j(j)
policy unique
| restA | restB | -> r(r) : bool |
| <=1000円 | - | false |
| >1000円 | <3980円 | false |
| >1000円 | >=3980円 | true |
| <=1000円 | >=3980円 | true |
```

関係するコード: [E105](#e105), [W105](#w105), [E109](#e109)
