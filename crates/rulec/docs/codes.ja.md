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
| [E011](#e011) | error | 公開される名前に ASCII の別名がありません |
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
| [E024](#e024) | error | 行き先の無い判定があります |
| [E025](#e025) | error | 例に並びの列がありません |
| [E026](#e026) | error | `sequence` の書き方が正しくありません |
| [E027](#e027) | error | 例が指す並びがありません |
| [E028](#e028) | error | `count` の書き方が正しくありません |
| [E029](#e029) | error | この列は数えられません |
| [E030](#e030) | error | `count` に範囲が要ります |
| [E031](#e031) | error | `fold` と `count` は一緒に書けません |
| [E032](#e032) | error | 取り込んだ列挙と宣言がずれています |
| [E033](#e033) | error | 取り込んだ列挙の値に、行も `default` もありません |
| [E034](#e034) | error | 行ラベルが二度あります |
| [E035](#e035) | error | `overrides` の指す先がありません |
| [E036](#e036) | error | `overrides` の相手が同じ出力を定めていません |
| [E037](#e037) | error | 引用した箇所のハッシュが固定されていません |
| [E038](#e038) | error | 引用した箇所が変わっています |
| [E039](#e039) | error | 出典の写しがありません |
| [E040](#e040) | error | 準用する元の規則が、書いてあるハッシュと違います |
| [E041](#e041) | error | 準用の読み替えが元の規則と合いません |
| [E042](#e042) | error | 読み替えの型が合いません |
| [E043](#e043) | error | 渡す値が元の規則の範囲か制約に収まりません |
| [E044](#e044) | error | その規則は準用できません |
| [E045](#e045) | error | 出力を共有する表に、出力の列が二つ以上あります |
| [E046](#e046) | error | `clause` の形が読めません |
| [E047](#e047) | error | 宣言の後ろに余分な語があります |
| [E049](#e049) | error | 桁区切りのカンマは書けません |
| [E048](#e048) | error | この型には足し算も掛け算もありません |
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
| [E113](#e113) | error | 真偽定義の条件が、書ける二つの形のどちらでもありません |
| [E114](#e114) | error | 値が宣言した刻みに載っていません |
| [E115](#e115) | error | 変数では割れません |
| [E116](#e116) | error | 行の金額が、引いた写しに無いか、別の見出しの下にあります |
| [E117](#e117) | error | 配分の前提が揃っていません |
| [E118](#e118) | error | 呼び出しの形が違います |
| [E120](#e120) | error | `from` が入力の型と合いません |
| [E121](#e121) | error | `from` のパスが契約にありません |
| [E119](#e119) | error | 行の境界が、引いた写しと反対側です |
| [W122](#w122) | warning | その `shape` を使っている入力がありません |
| [E122](#e122) | error | 契約が通す値を、規則が断ります |
| [W123](#w123) | warning | 行が、契約の通さない値でしか当たりません |
| [E123](#e123) | error | 契約が、規則の `constraint` を破る組み合わせを通します |
| [W124](#w124) | warning | 行が、契約の通さない組み合わせでしか当たりません |
| [W105](#w105) | warning | 要確認の隠れ: 先の行が後の行の一部を隠しています |
| [W110](#w110) | warning | 重なりのない `first` です |
| [W111](#w111) | warning | 使われていない宣言があります |
| [W116](#w116) | warning | どの例も使っていない `sequence` です |
| [W119](#w119) | warning | ハッシュを書いた箇所が引用されていません |
| [W120](#w120) | warning | 写しの値を、どの行も使っていません |
| [W118](#w118) | warning | 準用した表の行が、この規則ではどれも当たりません |
| [W117](#w117) | warning | 効かない例外です |
| [W121](#w121) | warning | 別名が生成先の言葉とぶつかります |
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
  %x(x) : bool
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

**直し方。** 行頭に宣言の語を書いてください（`description / import / enum / group / inputs / elements / outputs / derive / define / constraint / table / fold / count / sum / sequence / result / examples / policy / overrides / clause / source / apply / shape`）。表の行なら `|` で始めます。

**最小の再現**:

```rule
rule t(t) v1

= 1
```

関係するコード: [E003](#e003), [E005](#e005)

## E005

`error` — **この位置に書けない語です**

**いつ出るか。** 行頭の語が語彙にないとき。語彙には同義の綴りがなく、英語の一種類だけです（§1.1）。

**直し方。** `description / import / enum / group / inputs / elements / outputs / derive / define / constraint / table / fold / count / sum / sequence / result / examples / policy / overrides / clause / source / apply / shape` のどれかに直してください。業務の語は名前とセルの中にだけ書きます。

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
| - | true           |
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
|   | true           |
```

関係するコード: [E101](#e101)

## E009

`error` — **宣言の名前が予約語と衝突しています**

**いつ出るか。** 宣言した名前（や別名）が語彙の語と同じとき。行指向のパーサがその行をセクションの始まりと読んで、宣言を黙って捨てる事故を防ぎます。

**直し方。** 名前を変えてください（`enum range(kind)` なら `enum 範囲区分(range_kind)`）。予約語は `src/kw.rs` の表ひとつで決まっています。

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
| w         | -> r(r) : bool |
| 0g..1000g | true           |
| >1000g    | false          |
```

関係するコード: [E105](#e105), [E101](#e101)

## E011

`error` — **公開される名前に ASCII の別名がありません**

**いつ出るか。** 規則名・入力・出力に、**ASCII でない名前**が付いていて括弧の中の別名も無いとき。別名は生成コードの公開名になります（漢字は大文字を持てず、Go の公開識別子になれません）。名前がもとから ASCII なら、それ自身が公開名になるので別名は要りません。

**直し方。** 括弧で別名を足してください（`重量 : bool` なら `重量(weight) : bool`）。導出・定義・グループ・表の別名は任意で、書けば生成コードがその名前を使い、書かなければ宣言した名前をそのまま使います。

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
| -    | true           |
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
| - | true           |
```

関係するコード: [E011](#e011), [E013](#e013)

## E013

`error` — **取込先がありません**

**いつ出るか。** `import` の先が無いとき。取込先は三つあります。組み込みの名前空間（いまは `std/都道府県`、47 値）、`.proto` の列挙（`import proto "<ファイル>" <列挙> -> <この規則の列挙>`）、JSON Schema の列挙（`import jsonschema "<ファイル>" "<ポインタ>" -> <この規則の列挙>`、OpenAPI も同じ）です。あとの二つは、ファイルを読めないとき、その列挙がファイルに無いとき、行の形が違うときに出ます。YAML は読まないので、JSON にしたものを指してください。

**直し方。** `import std/都道府県` に直すか、その列挙を `enum` でこのファイルに書いてください。ファイルから取り込むなら、パスは規則ファイルのある場所からたどるので、そこからの相対で書きます。JSON Schema のポインタは `#/components/schemas/<名前>` の形で、届かなかったときは、そこにある鍵が並びます。

**最小の再現**:

```rule
rule t(t) v1

import std/nope
```

関係するコード: [E012](#e012), [E032](#e032)

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
| r    | -> o(o) : money[円, incl_tax] |
| <=5% | 0円                           |
| -    | p × r                         |
```

関係するコード: [E008](#e008), [E012](#e012)

## E015

`error` — **`result` が書けるのは最初の出力だけです**

**いつ出るか。** `result` が二つ目以降の出力を名指ししたとき。`result` は最初の出力を組み立てるための書き方で、評価器も生成コードもそこにしか当てません（§1.2）。名指しが効かないまま通っていたので、`number` が `money` の枠に入っても E103 が出ませんでした。

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
| - | 100円                         |

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
| - | 100円                         |

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
| - | true           |
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
| - | true           |
```

関係するコード: [E017](#e017), [W111](#w111)

## E019

`error` — **例が制約を破っています**

**いつ出るか。** 例の入力が `constraint` を満たしていないとき。制約は「この組み合わせは起きない」という宣言で、完全性の検査はそれを信じて、その組み合わせには行を要求していません。生成コードもその入力を入口で断ります。答えを主張できない入力です。

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
| - | - | true           |

examples
| a | b | -> r |
| 5 | 1 | true |
```

関係するコード: [E017](#e017), [E018](#e018), [E101](#e101)

## E020

`error` — **`elements` の宣言が正しくありません**

**いつ出るか。** `elements` に名前が無いか、二本あるとき。規則がたどる並びは一つで、その一要素ぶんのフィールドをそこに書きます（§15.56）。

**直し方。** `elements 運賃行(fee_rows)` の形にして、続く行に一要素ぶんのフィールドを `inputs` と同じように書いてください。並びが二つ要るなら、それは別の規則です。

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
| - | true           |
```

関係するコード: [E021](#e021)

## E021

`error` — **`fold` の書き方が正しくありません**

**いつ出るか。** `fold <判定の列> over <並びの名前>` になっていないか、判定の行き先が `next` `stop` `stop with <値>` `take_unique <値>` `take_first <値>` `keep_max <値> by <鍵>` のどれでもないか、畳もうとしている列が列挙でないとき。

**直し方。** 見出しと行き先を上の形に直してください。`take` とだけ書くことはできません。**一件だけ採るのか、最初の一件を採るのか**は、書く人が選ぶことだからです（§15.56）。

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
| - | true           |

fold r
  empty -> false
```

関係するコード: [E020](#e020), [E022](#e022), [E023](#e023), [E024](#e024)

## E022

`error` — **要素がゼロ件のときの答えが宣言されていません**

**いつ出るか。** `fold` に `empty -> <値>` が無いとき。空の並びは必ず来ます。手で書いたループがいちばんよく落とすのがこの場合で、たいていは最初の要素をそのまま読んで落ちます。

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
| k     | -> d(d) : v |
| <=5円 | a           |
| >5円  | b           |

fold d over xs
  a -> next
  b -> take_first k
  exhausted -> held
```

関係するコード: [E023](#e023), [E024](#e024)

## E023

`error` — **最後まで見終えたときの答えが宣言されていません**

**いつ出るか。** `fold` に `exhausted -> <値>` が無いとき。どの要素も打ち切らずに並びが尽きた場合の答えです。保持していた暫定の値をそのまま返すつもりでも、それは書いて初めて決まります。

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
| k     | -> d(d) : v |
| <=5円 | a           |
| >5円  | b           |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0円
```

関係するコード: [E022](#e022), [E024](#e024)

## E024

`error` — **行き先の無い判定があります**

**いつ出るか。** 表が出しうる判定のどれかに、`fold` の行き先が無いとき。その判定の要素が来たら、次にどうするかが決まっていません。表の完全性と同じ検査を、畳み込みの側に当てたものです（§15.56）。

**直し方。** 行き先を足すか、表がその値を出さないようにしてください。逆に、どの要素も辿り着けない判定に行き先があるときは W115 が出ます。

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
| k     | -> d(d) : v |
| <=5円 | a           |
| >5円  | b           |

fold d over xs
  a -> next
  empty -> 0円
  exhausted -> held
```

関係するコード: [E022](#e022), [E023](#e023), [W115](#w115)

## E025

`error` — **例に並びの列がありません**

**いつ出るか。** 並びをたどる規則に `examples` があるのに、`elements` の名前の列が見出しに無いとき。どの並びをたどるかが決まっていない例は、答えの決まっていない例です（§15.56）。

**直し方。** `sequence <名前>` で並びを書き、例の見出しに並びの列を足して、その名前をセルに書いてください。行がゼロ本の `sequence` は、要素ゼロ件の例になります。

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
| k     | -> d(d) : v |
| <=5円 | a           |
| >5円  | b           |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0円
  exhausted -> held

examples
| -> r |
| 0円  |
```

関係するコード: [E026](#e026), [E027](#e027)

## E026

`error` — **`sequence` の書き方が正しくありません**

**いつ出るか。** `sequence` のフィールドが `elements` のフィールドとそろっていないとき——余分なフィールドがある、フィールドが足りない、`->` がある、たどる並びそのものが無い、同じ名前が二つある、セルが値でない（範囲や `-` が書いてある）。

**直し方。** `elements` のフィールドをそのまま見出しにして、一行に一件ぶんの値を書いてください。これは表ではなく、実際に渡す値の並びです。

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
| k     | -> d(d) : v |
| <=5円 | a           |
| >5円  | b           |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0円
  exhausted -> held

sequence s(s)
| m   |
| 3円 |
```

関係するコード: [E025](#e025), [E020](#e020)

## E027

`error` — **例が指す並びがありません**

**いつ出るか。** 例の並びの列に書かれた名前の `sequence` が無いとき、またはその列に名前でないもの（数や範囲）が書かれているとき。

**直し方。** その名前で `sequence` を書くか、セルを、書いてある `sequence` の名前に直してください。

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
| k     | -> d(d) : v |
| <=5円 | a           |
| >5円  | b           |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0円
  exhausted -> held

examples
| xs   | -> r |
| nope | 0円  |
```

関係するコード: [E025](#e025), [E026](#e026)

## E028

`error` — **`count` の書き方が正しくありません**

**いつ出るか。** `count <名前>(<別名>) over <並びの名前> where <列> = <値>` になっていないとき。`over` が無い、`where` が無い、指した並びが `elements` で宣言されていない、`=` の右に値が無い（§15.58）。

**直し方。** 上の形に直してください。`= <値>` は、真偽の列を数えるときだけ省けます。

**最小の再現**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=0 <=10

elements xs(xs)
  b(b) : bool

outputs
  r(r) : bool

count h(h) where b  range >=0 <=10

table j(j)
policy unique
| n | -> r(r) : bool |
| - | true           |
```

関係するコード: [E029](#e029), [E030](#e030), [E020](#e020)

## E029

`error` — **この列は数えられません**

**いつ出るか。** `where` が指す列が、要素ごとに決まる値でないとき（入力や導出は一件の呼び出しに一つしかないので、数えても 0 か 1 です）。値が有限の集合でないとき。書いた値がその列挙にないとき。列挙の列なのに `= <値>` が無いとき（§15.58）。

**直し方。** 要素のフィールドか、要素ごとの表が出した列を指してください。判定を表に書けば、その分類そのものも完全性の検査に掛かります。

**最小の再現**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=0 <=10
  ok(ok) : bool

elements xs(xs)
  b(b) : bool

outputs
  r(r) : bool

count h(h) over xs where ok  range >=0 <=10

table j(j)
policy unique
| h | -> r(r) : bool |
| - | true           |
```

関係するコード: [E028](#e028), [E012](#e012)

## E030

`error` — **`count` に範囲が要ります**

**いつ出るか。** `count` の行に `range >=0 <=<上限>` が無いか、上限が無いか、下限が負のとき。範囲は二つの意味を持ちます——数えた結果を列に使ったときに完全性の検査が見る全体集合と、**並びの長さの上限**です（§15.58）。

**直し方。** `range >=0 <=100` の形で書いてください。生成コードは、この上限より長い並びを入口で断ります。数値の入力と同じで、宣言の外は黙って通しません。

**最小の再現**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=0 <=10

elements xs(xs)
  b(b) : bool

outputs
  r(r) : bool

count h(h) over xs where b

table j(j)
policy unique
| h | -> r(r) : bool |
| - | true           |
```

関係するコード: [E028](#e028), [E112](#e112)

## E031

`error` — **`fold` と `count` は一緒に書けません**

**いつ出るか。** 一つの規則に `fold` と `count` の両方があるとき。どちらも同じ並びの終わり方で、`fold` は途中で打ち切れるので、止まった歩きの数え上げが何を意味するかが決まりません（§15.58）。

**直し方。** 数えるなら `fold` を消して、数えた結果を表で判定してください。畳むなら `count` を消してください。

**最小の再現**:

```rule
rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : number  range >=0 <=10

outputs
  r(r) : number  round down(1)

table j(j)
policy unique
| k   | -> d(d) : v |
| <=5 | a           |
| >5  | b           |

count h(h) over xs where d = a  range >=0 <=10

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0
  exhausted -> held
```

関係するコード: [E021](#e021), [E029](#e029)

## E032

`error` — **取り込んだ列挙と宣言がずれています**

**いつ出るか。** `import proto` や `import jsonschema` が名指しした列挙の値と、この規則の `enum` の別名がそろっていないとき。どちら側にしか無い値も、名前で出ます。増えるのはたいてい proto の側で、**互換な変更として通ってしまう、規則の外での変更です**（§15.59）。

**直し方。** 増えた値をこの規則の `enum` に足してください。日本語の名前は proto に入っていないので、そこは自分で決めます。消えた値なら、この規則からも消します。値を足すと、次はその値に行が要るかを表が問います（E033）。

**最小の再現**:

```rule
rule t(t) v1

import proto "tier.proto" Tier -> v
enum v(v) = one(one)

inputs
  x(x) : v

outputs
  r(r) : bool

table j(j)
policy unique
| x   | -> r(r) : bool |
| one | true           |
```

隣に置く `tier.proto`:

```proto
syntax = "proto3";

enum Tier {
  TIER_UNSPECIFIED = 0;
  TIER_ONE = 1;
  TIER_TWO = 2;
}
```

関係するコード: [E013](#e013), [E033](#e033), [E101](#e101)

## E033

`error` — **取り込んだ列挙の値に、行も `default` もありません**

**いつ出るか。** 取り込んだ列挙の値が、どの行にも現れず、`default` も付いていないとき。自分で書いた値なら書き忘れの警告（W111）ですが、契約から来た値は**外の変更がまだ誰にも読まれていない**という意味なので、止めます。既定の行がある表では完全性検査が通ってしまい、新しい値に既定の額が黙って当たります（§15.59）。

**直し方。** その値の行を表に足すか、値の宣言に `default` を付けてください。`default` は「既定の行に落ちるのが意図です」という宣言で、額を決めた人がいることの印になります。

**最小の再現**:

```rule
rule t(t) v1

import proto "tier.proto" Tier -> v
enum v(v) = one(one) | two(two)

inputs
  x(x) : v

outputs
  r(r) : bool

table j(j)
policy first
| x   | -> r(r) : bool |
| one | true           |
| -   | false          |
```

隣に置く `tier.proto`:

```proto
syntax = "proto3";

enum Tier {
  TIER_UNSPECIFIED = 0;
  TIER_ONE = 1;
  TIER_TWO = 2;
}
```

関係するコード: [E032](#e032), [W111](#w111), [E101](#e101)

## E034

`error` — **行ラベルが二度あります**

**いつ出るか。** 同じ表の二つの行が、最初の `|` の前に同じラベルを書いているとき。ラベルは `overrides` の行、記録の trace、後の版が行を指す名前なので、一つの表の中で一意でなければなりません。

**直し方。** どちらかのラベルを変えてください。

**最小の再現**:

```rule
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)
   | a     | -> x  |
r1 | true  | true  |
r1 | false | false |
```

関係するコード: [E009](#e009), [E035](#e035)

## E035

`error` — **`overrides` の指す先がありません**

**いつ出るか。** `overrides` が名指した表が無いか、この表より後ろで宣言されているか、`表:行ラベル` の行にそのラベルが無いとき。行の書き方が読めないときも同じです。例外は本文の後に書くので、指す先はいつも上にあります。

**直し方。** 上で宣言した表か、その行（行の先頭にラベルを書き、`表:ラベル` で指す）を名指してください。優先する側を後に書きます。

**最小の再現**:

```rule
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)
overrides 無い表
| a     | -> x  |
| true  | true  |
| false | false |
```

関係するコード: [E034](#e034), [E036](#e036)

## E036

`error` — **`overrides` の相手が同じ出力を定めていません**

**いつ出るか。** `overrides` の指す表が、この表とは別の出力を定めているとき。優先の順序は、同じ出力を定める定義のあいだにだけあります。

**直し方。** 同じ出力を定める表を指すか、この表の出力列をその相手と揃えてください。

**最小の再現**:

```rule
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool
  y(y) : bool

table 甲(ko)
| a | -> x |
| - | true |

table 乙(otsu)
overrides 甲
| a | -> y |
| - | true |
```

関係するコード: [E035](#e035), [E045](#e045)

## E037

`error` — **引用した箇所のハッシュが固定されていません**

**いつ出るか。** `@出典 第91条` のように引用した箇所に、`source` の行の下の `  第91条 sha256:…` というハッシュの行が無いとき。`file` の出典なら、その行に `sha256:…` が無いとき。法令を箇所無しで `@法` とだけ引用したとき、箇所の書き方や、引用・宣言の形が読めないときも同じです（隣に置いたファイルは `@郵便` と丸ごと引用できます）。文書から引けるのは表なので、引用箇所は `表3`（文書順に三つめの表）か `table3` と書きます。それ以外の書き方は読めません（§15.82）。語として読めない箇所は `"` で囲みます（`@osha "§1910.157"`）。ハッシュが無いと、写しが改訂されても check は何も言えません（§15.68）。

**直し方。** 原文を読んで写した行が正しいことを確かめたら、`fix.text` の行を貼るか `rulec source pin <file.rule>` を実行して、いまの写しのハッシュを書き込んでください。

**最小の再現**:

```rule
rule t(t) v1

source 法 = law "000AC0000000001" asof 2026-04-01

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)  @法 第1条
| a | -> x |
| - | true |
```

隣に置く `sources/law/000AC0000000001@2026-04-01/MainProvision-Article_1.xml`:

```proto
<Article Num="1"><ArticleTitle>第一条</ArticleTitle><Paragraph Num="1"><ParagraphNum/><ParagraphSentence><Sentence>甲は、乙とする。</Sentence></ParagraphSentence></Paragraph></Article>
```

関係するコード: [E038](#e038), [E039](#e039), [W119](#w119)

## E038

`error` — **引用した箇所が変わっています**

**いつ出るか。** 規則に書いてあるハッシュと、規則の隣にある写しのハッシュが違うとき。写しを取り直した PR で落ちます。その箇所を引用している表・節・行を名指しするので、読み直すのはそこだけで済みます。

**直し方。** 写しの差分を読み、写した行がまだ正しければ `fix.text` のとおりハッシュの行を書き換えてください（`rulec source pin` でも書けます）。行が変わるなら、先に行を直します。

**最小の再現**:

```rule
rule t(t) v1

source 法 = law "000AC0000000001" asof 2026-04-01
  第1条 sha256:0000000000000000

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)  @法 第1条
| a | -> x |
| - | true |
```

隣に置く `sources/law/000AC0000000001@2026-04-01/MainProvision-Article_1.xml`:

```proto
<Article Num="1"><ArticleTitle>第一条</ArticleTitle><Paragraph Num="1"><ParagraphNum/><ParagraphSentence><Sentence>甲は、乙とする。</Sentence></ParagraphSentence></Paragraph></Article>
```

関係するコード: [E037](#e037), [E039](#e039)

## E039

`error` — **出典の写しがありません**

**いつ出るか。** 引用した箇所の写しが規則の隣に無いとき（法令なら `sources/law/<法令ID>@<日付>/<要素>.xml`、文書の表なら文書の隣の `料金表.md.fragments/表3.tsv`）、または `file` の出典そのものが読めないとき。check は通信もしませんし、文書の中身も見ません。写しが無ければ照合できません。

**直し方。** `rulec source fetch <file.rule>` が、政府の法令データベース（e-Gov 法令検索）から引用箇所を取り、文書からは引いた表を取り出して、規則の隣に置きます。写しは git に入れてください。

**最小の再現**:

```rule
rule t(t) v1

source 法 = law "000AC0000000001" asof 2026-04-01

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)  @法 第2条
| a | -> x |
| - | true |
```

関係するコード: [E037](#e037), [E038](#e038)

## E040

`error` — **準用する元の規則が、書いてあるハッシュと違います**

**いつ出るか。** `apply` の見出しに `sha256:…` が無いとき、または書いてあるハッシュと、いま隣にある元の規則のファイルのハッシュが違うとき。元の規則が改正されれば、それを準用するこの規則の答えも変わっています。ハッシュを書いておかないと、その変化を誰も承認しないまま通ってしまいます（§15.69）。

**直し方。** `rulec diff <古い版> <新しい版>` でこの規則の答えが何件いくら動くかを見て、それでよければ、見出しを `fix.text` のとおり書き換えるか `rulec source pin <file.rule>` を実行してハッシュを書き直してください。

**最小の再現**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=1 <=10

outputs
  y(y) : number  round down(1)

apply 呼(c) = "呼び先.rule"
  a = n
  x -> y
```

隣に置く `呼び先.rule`:

```proto
rule 呼び先(callee) v1

inputs
  a(a) : number  range >=1 <=10

outputs
  x(x) : number  round down(1)

table 表(t)
policy unique
   | a   | -> x |
小 | <=5 | 1    |
大 | >5  | 2    |
```

関係するコード: [E037](#e037), [E038](#e038), [E044](#e044)

## E041

`error` — **準用の読み替えが元の規則と合いません**

**いつ出るか。** 元の規則の入力に読み替えの無いものがあるとき、元の規則に無い入力や出力を名指ししたとき、元の規則の出力に付けた名前がこの規則に既にあるとき、`apply` の下の行の形が読めないとき。読み替えは元の規則の入力を一つ残らず書くものなので、足りなければ「読み替えが書いてない」のと同じです（§15.69）。

**直し方。** 元の規則の入力を一つずつ `<元の規則の入力> = <この規則の値>` で読み替え、出力は `<元の規則の出力> -> <名前>` で名前を付けてください。元の規則の入力と出力の名前は文面に並びます。

**最小の再現**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=1 <=10

outputs
  y(y) : number  round down(1)

apply 呼(c) = "呼び先.rule" sha256:369102b8f803dc28
  b = n
  x -> y
```

隣に置く `呼び先.rule`:

```proto
rule 呼び先(callee) v1

inputs
  a(a) : number  range >=1 <=10

outputs
  x(x) : number  round down(1)

table 表(t)
policy unique
   | a   | -> x |
小 | <=5 | 1    |
大 | >5  | 2    |
```

関係するコード: [E040](#e040), [E042](#e042), [E043](#e043)

## E042

`error` — **読み替えの型が合いません**

**いつ出るか。** 読み替えた値の型が、元の規則の入力の型と違うとき（単位・税区分・列挙と数）。列挙どうしでは、この規則の列挙の値にあたる元の規則の値が無いとき（同じ綴りの値はそのまま対応します）、`with` にどちらにも無い値を書いたとき、リテラルが元の規則の型の値でないときも同じです。

**直し方。** 型を合わせてください。列挙は `<入力> = <値> with <この規則の値> -> <元の規則の値>, …` で、この規則の列挙の値を全部対応づけます。「『退職』とあるのは『任期の終了』と読み替える」を、そのまま書く形です。

**最小の再現**:

```rule
rule t(t) v1

enum 種別(kind) = 甲(a) | 乙(b) | 丙(c)

inputs
  k(k) : 種別

outputs
  y(y) : number  round down(1)

apply 呼(c) = "区分の呼び先.rule" sha256:2e6f2e04c21cdbb3
  a = k
  x -> y
```

隣に置く `区分の呼び先.rule`:

```proto
rule 区分の呼び先(enum_callee) v1

enum 区分(kind) = 甲(a) | 乙(b)

inputs
  a(a) : 区分

outputs
  x(x) : number  round down(1)

table 表(t)
policy unique
| a  | -> x |
| 甲 | 1    |
| 乙 | 2    |
```

関係するコード: [E041](#e041), [E043](#e043)

## E043

`error` — **渡す値が元の規則の範囲か制約に収まりません**

**いつ出るか。** 読み替えた値の取りうる範囲が、元の規則の入力の `range` からはみ出すとき（はみ出す値を一つ例に挙げます）、または元の規則の `constraint` がこの規則の宣言から導けないとき。元の規則の完全性はその範囲と制約の上でしか証明されていないので、外の値には定義がありません。`fix.text` は付けません。元の規則に行を足すのは、別の承認の話だからです（§15.69）。

**直し方。** この規則の入力の範囲を元の規則の範囲まで狭めるか、はみ出す部分をこの規則の節で定めてください。どちらにするかは業務の判断です。制約なら、同じ関係を `constraint` で宣言してください。

**最小の再現**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=0 <=10

outputs
  y(y) : number  round down(1)

apply 呼(c) = "呼び先.rule" sha256:369102b8f803dc28
  a = n
  x -> y
```

隣に置く `呼び先.rule`:

```proto
rule 呼び先(callee) v1

inputs
  a(a) : number  range >=1 <=10

outputs
  x(x) : number  round down(1)

table 表(t)
policy unique
   | a   | -> x |
小 | <=5 | 1    |
大 | >5  | 2    |
```

関係するコード: [E041](#e041), [E042](#e042), [E101](#e101)

## E044

`error` — **その規則は準用できません**

**いつ出るか。** 元の規則が読めないとき、元の規則が `check` を通らないとき（出たコードを添えます）、元の規則自身が `apply` を持つとき、元の規則が並びを順に見ていく規則（`elements`、`fold`、`count`）のとき、元の規則の表や節が元の規則自身の列挙を出力に持つとき。通らない規則を準用しても通らない規則になるだけで、準用の準用は一段に書き直してから書きます。

**直し方。** 先に元の規則を直してください。`apply` を持つ規則や、並びを順に見ていく規則を準用するなら、その中身をこの規則に書き写してください。

**最小の再現**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=1 <=10

outputs
  y(y) : number  round down(1)

apply 呼(c) = "壊れた呼び先.rule" sha256:c4f9eba5b2949205
  a = n
  x -> y
```

隣に置く `壊れた呼び先.rule`:

```proto
rule 壊れた呼び先(broken) v1

inputs
  a(a) : number  range >=1 <=10

outputs
  x(x) : number  round down(1)

table 表(t)
policy unique
| a   | -> x |
| <=5 | 1    |
```

関係するコード: [E040](#e040), [E041](#e041)

## E045

`error` — **出力を共有する表に、出力の列が二つ以上あります**

**いつ出るか。** ある出力を二つ以上の表が定めていて（または `overrides` で結ばれていて）、そのうちの表に出力の列が二つ以上あるとき。行が二つの出力の定義を束ねていると、一方だけが上書きされたときにもう一方の値の出どころが決まらず、生成コードでは同じ行の条件を二度書くことになります。

**直し方。** 二つ目の出力を、別の表に分けてください。

**最小の再現**:

```rule
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool
  y(y) : bool

table 甲(ko)
| a | -> x | y    |
| - | true | true |

table 乙(otsu)
overrides 甲
| a    | -> x  |
| true | false |
```

関係するコード: [E036](#e036), [E105](#e105)

## E046

`error` — **`clause` の形が読めません**

**いつ出るか。** `clause` の見出しに名前か `->` か出力が無いとき、`when` か `then` の行が無いか二度あるとき、`when` の条件が `<列> <セル> and …` の形でないとき（列が無い、同じ列が二度ある、条件が無い）。節は一行の表なので、条件と値が一つずつ要ります。

**直し方。** `clause <名前>(<別名>) -> <出力>` の下に `when <列> <セル> and …`（条件が無ければ `when always`）と `then <値>` を一行ずつ書いてください。優先する相手があれば `overrides <相手>` を足します。

**最小の再現**:

```rule
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool

clause 例外(exception) -> x
  then true
```

関係するコード: [E008](#e008), [E035](#e035), [E045](#e045)

## E047

`error` — **宣言の後ろに余分な語があります**

**いつ出るか。** 入力・出力・`derive`・`count` の宣言の行に、`range`・`round`・`contract_only` のどれにも属さない語が残っているとき。この道具は行から欲しい語を探して残りを踏み越える読み方をするので、こうした語はいままで黙って捨てられていました。範囲の後ろに書いた税区分（`range >=0円 <=10000円 incl_tax`）も、単位が単位として読めずに余った分も、どちらも通っていました。範囲は完全性検査が量化する全体集合で、生成コードの入口ガードでもあるので、境界が一つ落ちたまま「完全」と答えることになります。

**直し方。** その語を消すか、宣言の一部として正しい形に直してください。税区分や刻みは型の括弧の中（`money[円, incl_tax]`、`rate[step 0.1%]`）で、範囲は `range >=<値> <=<値>`、丸めは `round <向き>(<格子>)` です。

**最小の再現**:

```rule
rule t(t) v1

inputs
  a(a) : money[円, incl_tax]  range >=0円 <=10000円 incl_tax

outputs
  x(x) : bool

table 表(t1)
policy unique
| a | -> x |
| - | true |
```

関係するコード: [E011](#e011), [E103](#e103), [E104](#e104)

## E049

`error` — **桁区切りのカンマは書けません**

**いつ出るか。** 数を `1,000` や `1,949,000円` のように、1〜3 桁のあとに `,` と 3 桁の組を続けて書いたとき。`,` はセルの中で集合の要素を区切る記号なので、そのままでは二つ以上の値に読まれます。いままでは黙ってそう読んでいて、数の列の `<=1,000` は `<=1` として検査を通り、金額の列では `1` に単位が無いという的外れな E103 で止まっていました。文書の数字をそのまま写すと、ここに当たります。

**直し方。** カンマを外して `1000` と書いてください。桁を区切りたいときは `1_000` と書けます。`fix.text` が書き直したリテラルです。

**最小の再現**:

```rule
rule t(t) v1

inputs
  a(a) : money[円]  range >=0円 <=5000円

outputs
  x(x) : bool

table 表(t1)
policy unique
| a         | -> x  |
| <=1,000円 | true  |
| >1,000円  | false |
```

関係するコード: [E014](#e014), [E103](#e103)

## E048

`error` — **この型には足し算も掛け算もありません**

**いつ出るか。** 順序はあるが演算の無い型を、`+ - × ÷` のどれかに使ったとき。`date`、`temperature[℃]`・`temperature[℉]`、`sound[dB]` の三つがそれです。℃ は 0 が「無い」を意味しない目盛りなので `気温 × 2` に意味が無く、dB は対数なので二つ足しても音が二つ分にはならず、日付は暦日なので引き算の結果を入れる型がありません。どれも規則の中では閾値としてしか現れないので、比較と `range` だけを残して演算を落としています。

**直し方。** 閾値として比べるか、`range` に書いてください。差や倍率そのものが業務ルールなら、計算した結果を入力として受け取るか、表で引きます。二つの日付のあいだの日数は、呼び出し側で数えて `number` か `duration` として渡します。

**最小の再現**:

```rule
rule t(t) v1

inputs
  甲(a) : temperature[℃]  range >=0℃ <=40℃
  乙(b) : temperature[℃]  range >=0℃ <=40℃

outputs
  x(x) : bool

derive 差(gap) : temperature[℃] = 甲 - 乙  range >=-40℃ <=40℃

table 表(t1)
policy unique
| 差 | -> x |
| -  | true |
```

関係するコード: [E103](#e103), [E112](#e112), [E115](#e115)

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
| a | true           |
| b | false          |
```

関係するコード: [E102](#e102), [E105](#e105), [W111](#w111)

## E102

`error` — **どの入力にも当てはまらない行があります**

**いつ出るか。** 先行する行にすべて覆われているか、上流の表が決して出さない値を名指ししているとき。形は二つあり、文面が原因を書き分けます。

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
| w       | -> r(r) : bool |
| -       | true           |
| <=1000g | false          |
```

関係するコード: [E101](#e101), [W105](#w105), [W110](#w110)

## E103

`error` — **単位の混同: 型の違う値を混ぜています**

**いつ出るか。** 式やセルで、単位・通貨・税区分の違う値を足したり比べたりしているとき。`money[円, incl_tax]` と `money[円, excl_tax]` も別物です（§2.3）。型の刻みが読めないとき（`rate[step 1g]`）と、率の入力に刻みが無いときもこれです。どちらも、実行時に受け渡す整数が何を単位に数えているのかが決まりません。

**直し方。** 混ぜている片方を表に移してください。「重量に応じた加算料金」なら `table 重量加算 | 重量 | -> 加算額 : money[円, incl_tax] |` の形です。税の変換も、式ではなく表として書きます。刻みなら、型と同じ単位で書きます（率の入力は `rate[step 1%]` や `rate[step 0.1%]`）。

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
| - | 100円                         |

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
| x     | -> r(r) : money[円, incl_tax] |
| true  | 100円                         |
| false | 200円                         |
```

関係するコード: [E106](#e106), [E103](#e103)

## E105

`error` — **行の重なり: 同じ入力が二つ以上の行に当てはまります**

**いつ出るか。** `policy unique` の表で、両方に当てはまる入力を実際に構成できたとき。構成できなかった重なりは W114 に落ちます。上の表が同時には出さない値の組でしか重ならない対は、重なりとして報告しません（E102 がそういう行を死んだ行と呼ぶのと同じ読みです）。

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
| x | y     | -> r(r) : money[円, incl_tax] |
| a | -     | 100円                         |
| - | true  | 200円                         |
| b | false | 300円                         |
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
| x     | -> r(r) : money[円, incl_tax] |
| true  | 1451円                        |
| false | 1000円                        |
```

関係するコード: [E104](#e104)

## E107

`error` — **例の期待値と一致しません**

**いつ出るか。** `examples` の行を参照評価器で走らせた結果が、書かれた期待値と違うとき。**どの表のどの行に当てはまったか**が付きます。`examples` は実行される仕様です。

**直し方。** 表が正しいなら期待値を直してください。期待値が業務の真実なら、当てはまった行のほうを直します。どちらを直すかは、出典（規約、Excel、いま動いている実装）が決めます。

**最小の再現**:

```rule
rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| x     | -> r(r) : money[円, incl_tax] |
| true  | 100円                         |
| false | 200円                         |

examples
| x    | -> r  |
| true | 200円 |
```

関係するコード: [E111](#e111), [E105](#e105)

## E108

`error` — **中間値が int64 に収まることを証明できません**

**いつ出るか。** 宣言した範囲と刻みから計算した「実際に取りうる値」が、int64 を超えるとき。率の刻みが 1% なら格納される整数は 100 倍になります。

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
| -   | 0円                           |
```

関係するコード: [E112](#e112), [E103](#e103)

## E109

`error` — **検査の予算を超えたので、完全性を証明できませんでした**

**いつ出るか。** 領域検査が訪れたノード数が `--budget` を超えたとき。**証明できなかったことを緑にはしない**ので、警告ではなくエラーです。

**直し方。** 表を分けて列の数を減らすか、`--budget` を上げてください。列の積が効くので、一つの表に列を積むより、表を一列につないでいくほうが安く済みます（§5.1）。

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
| a | true           |
| b | false          |
| c | true           |
```

関係するコード: [E101](#e101), [W114](#w114)

## E110

`error` — **検査できない型の列があります**

**いつ出るか。** その列の型を、区画の計算ができる形に落とせないとき。**これが出たら rulec 自身のバグです。** 表の検査が黙って素通りするのを防ぐための内部の検査で、日付と optional で二度起きた同じ種類の事故を、まとめて塞いだものです（§6.3）。

**直し方。** その列を、いま検査できる型（真偽・列挙・数量・金額・率・日付・optional）に直してください。そのうえで報告してください — 素通りより止まるほうが正しいという判断でこの検査があります。

**最小の再現**:

```rule
rule t(t) v1

inputs
  s(s) : string?

outputs
  r(r) : bool

table j(j)
policy unique
| s               | -> r(r) : bool |
| starts_with "a" | true           |
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
| x     | -> ok(ok) : bool | fee(fee) : money[円, incl_tax] |
| true  | true             | 100円                          |
| false | false            | 0円                            |

examples
| x    | -> ok |
| true | true  |
```

関係するコード: [E107](#e107)

## E112

`error` — **導出の範囲が、実際に到達しうる値を含んでいません**

**いつ出るか。** 入力の範囲から計算した「実際に取りうる値」が、導出に宣言した `range` からはみ出すとき。範囲が狭いと、完全性検査が実際に起きる値を見ないまま「完全」と答えます。

**直し方。** 文面が示す範囲まで `range` を広げてください（`range >=-110万円 <=100万円` の形で書いてあります）。起こりえない分まで広げても、検査がそこを自分で外すので害はありません。

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
| gap   | -> r(r) : bool |
| <=0円 | false          |
| >0円  | true           |
```

関係するコード: [E108](#e108), [E101](#e101)

## E113

`error` — **真偽定義の条件が、書ける二つの形のどちらでもありません**

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
| true   | true           |
| false  | false          |
```

関係するコード: [E112](#e112), [E103](#e103)

## E114

`error` — **値が宣言した刻みに載っていません**

**いつ出るか。** `rate[step 1%]` の列に `0.5%` のように、宣言した刻みの整数倍でない値が書かれたとき。実行時の値はその刻みの整数一本なので（§2.1）、この値には表し方がありません。率の出力の丸めの刻みが、出力の宣言した刻みの整数倍でないとき（`rate[step 1%]` に `round down(0.5%)`）も同じです。答えは丸めの刻みに載り、呼び出し側へは宣言した刻みの整数で渡すので、表せない答えが出ます（§15.144）。

**直し方。** 刻みに載る値に直すか、型の刻みを細かくしてください（`rate[step 0.1%]`）。黙って近い刻みに寄せると、表で読める境界と生成コードの境界が食い違います。丸めの刻みなら、刻みを丸めに合わせるか、丸めの刻みを刻みの整数倍にしてください。

**最小の再現**:

```rule
rule t(t) v1

inputs
  r(r) : rate[step 1%]  range >=0% <=100%

outputs
  o(o) : bool

table j(j)
policy first
| r      | -> o(o) : bool |
| <=0.5% | true           |
| -      | false          |
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
| - | true           |
```

関係するコード: [E103](#e103), [E108](#e108)

## E116

`error` — **行の金額が、引いた写しに無いか、別の見出しの下にあります**

**いつ出るか。** 行の出力の値が、その行（またはその表）が `@出典 表1` で引いている写しのどこにも出てこないとき（§15.82）。行のセルの語（`関東`）と一字一句同じ見出しが写しにあれば、その見出しの行と列の中だけを探します。隣の行の金額は写しのどこかにはあるので、表全体を探したのでは取り違えを見逃すからです（§15.143）。比べるのは金額だけです。閾値は写すときに書き換わります（`1,949,000円まで` は `<=1949000円` になる）が、金額は書き換わらないからです。写しの `5/1,000` や `1,000分の5` は 0.5% と読みます。写しは `rulec source fetch` が文書から取り出したもので、check が見るのはその写しであって文書そのものではありません。

**直し方。** 写しを読み直して金額を直してください。別の見出しの下にあると言われたら、行を取り違えています。一桁の打ち間違いなら、たいてい同時に W120 が出て、どの値が使われずに残っているかを言います。値が別のところ（後の通知、正誤表、人の回答）から来たのなら、この行の引用を外し、どこから来たかを行末のコメントに書いてください。`rulec doc` がそのコメントを承認する人に見せます。

**最小の再現**:

```rule
rule t(t) v1

source 料金表 = file "料金表.md" sha256:75465b330d123ab8
  表1 sha256:0a95cedbd7311274

inputs
  a(a) : bool

outputs
  x(x) : money[円]  round down(1円)

table 表(t1)  @料金表 表1
policy unique
| a     | -> x  |
| true  | 990円 |
| false | 890円 |
```

隣に置く `料金表.md`:

```proto
# 料金表

| あて先 | 運賃 |
|---|---|
| 近畿 | 990円 |
| 関東 | 880円 |
```

隣に置く `料金表.md.fragments/表1.tsv`:

```proto
あて先	運賃
近畿	990円
関東	880円
```

関係するコード: [W120](#w120), [E038](#e038), [E107](#e107)

## E117

`error` — **配分の前提が揃っていません**

**いつ出るか。** `allocate(配る額, 累計, 全体)` の三つが配分の形になっていないとき。三つとも範囲を宣言した名前で、配る額と累計は負になれず、全体は正で、累計が全体を超えないと `constraint` が言っていなければなりません（§15.102）。

**直し方。** 足りないものを書いてください。`constraint 累計 <= 全体` がないと、配る分が配る額を超えることがあり、明細の合計が総額に一致しません。負が混じると、下に丸めるのか零へ丸めるのかで言語ごとに答えが割れます（§7.1）。

**最小の再現**:

```rule
rule t(t) v1

inputs
  値引き(off) : money[円]  range >=0円 <=1000円
  ここまで(upto) : money[円]  range >=0円 <=1000円
  合計(base) : money[円]  range >=1円 <=1000円

outputs
  o(o) : money[円]  round down(1円)

derive 配分(share) : money[円] = allocate(値引き, ここまで, 合計)  range >=0円 <=1000円

result o = 配分
```

関係するコード: [E115](#e115), [E108](#e108)

## E118

`error` — **呼び出しの形が違います**

**いつ出るか。** 無い関数を呼んでいるか、引数の数が合わないとき。書けるのは `min(a, b)` `max(a, b)` `allocate(配る額, 累計, 全体)` と丸めの五つ（`down(x, 1円)` など）だけです（§2.3）。

**直し方。** 綴りと引数の数を見てください。多い引数は黙って捨てられ、少なければ答えが決まりません——どちらも §15.102 までは素通りしていて、生成器のほうで初めて行き止まりになっていました。

**最小の再現**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=0 <=100
  d(d) : number  range >=1 <=100

outputs
  o(o) : number  round down(1)

define r(r) : number = min(n, d, n)

result o = r
```

関係するコード: [E103](#e103), [E115](#e115)

## E120

`error` — **`from` が入力の型と合いません**

**いつ出るか。** `from` の返すものが、それを受ける入力の型と合わないとき（§15.125）。`any` と `all` は `bool` を、`count` は `number` を返します。パスの先にあるものの型が入力と合わないとき（契約が文字列と言っているフィールドを `number` の入力で受けるなど）と、`any`・`all`・`count` が並びでないものを歩こうとしているとき、`where` の値がフィールドの型と合わないときも、これです。契約は値がどう運ばれるかを言うので、列挙も日付も文字列で、金額と数量は宣言した単位の整数で来ます。

**直し方。** 型のほうか `from` のほうを直してください。件数が欲しいなら `number` の入力に範囲を付けて受け、当てはまるかどうかが欲しいなら `bool` で受けます。値そのものが欲しいなら `from <shape の名前>.<フィールド>` です。`where` の値に単位は書けません——契約に単位は無く、目盛りの違う数どうしを黙って比べることになるからです。

**最小の再現**:

```rule
rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  a(a) : bool  from count order.lines

outputs
  x(x) : bool

table 表(t1)
policy unique
| a | -> x |
| - | true |
```

隣に置く `order.json`:

```proto
{"$defs":{"Order":{"type":"object","properties":{"lines":{"type":"array","items":{"type":"object","properties":{"category":{"type":"string"}}}}}}}}
```

関係するコード: [E121](#e121), [W122](#w122), [E103](#e103)

## E121

`error` — **`from` のパスが契約にありません**

**いつ出るか。** `from` のパスが、宣言した `shape` の契約の中に見つからないとき（§15.125）。パスの最初の語が `shape` の名前でないとき、途中のフィールドが無いとき、`where` の見るフィールドが要素に無いときの三つです。どこまで届いたかと、そこにあったフィールドの名前を出します。契約は `.proto` でも JSON Schema でもよく、`import proto` と同じく毎回の `check` で読まれ、固定は付きません。

**直し方。** 綴りを直すか、契約のほうが動いたのならパスを書き直してください。**これが出るのが目的です**——契約がフィールドの名前を変えたとき、手書きのつなぎのコードなら実行時まで気づかず、ここなら生成の前に止まります。

**最小の再現**:

```rule
rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  a(a) : bool  from order.nope

outputs
  x(x) : bool

table 表(t1)
policy unique
| a | -> x |
| - | true |
```

隣に置く `order.json`:

```proto
{"$defs":{"Order":{"type":"object","properties":{"lines":{"type":"array","items":{"type":"object","properties":{"category":{"type":"string"}}}}}}}}
```

関係するコード: [E120](#e120), [W122](#w122), [E032](#e032)

## E119

`error` — **行の境界が、引いた写しと反対側です**

**いつ出るか。** 引用のある行の閾値が、境界の値を写しと反対の側に入れているとき（§15.124）。閾値は写すときに書き換わる（`1,949,000円まで` は `<=1949000円` になる）ので文字としては比べられず、比べているのは**境界の値がどちらに入るか**だけです。写しの「60cm以下」と「60cmを超え」はどちらも 60cm を小さいほうに入れ、`<=60cm` と `>60cm` も同じことを言います。写しに境界の語が無いとき（`18 to 20`、`60〜80`）、語が数と別の列にあるとき（保険料額表の「円以上／円未満」）、同じ数を写しが両側に置いているときは、何も言いません。

**直し方。** 写しを読み直して直してください。`fix.text` はこのセルの境界の側だけを入れ替えた形です——向きは表の幾何であって写しが決めることではないので、`<` と `<=` の入れ替えしか書きません。一つの境界を写し間違えると、それを分け合う二つの行の両方が出ます。境界が別のところ（後の通知、本文の但し書き）から来たのなら、この行の引用を外し、どこから来たかを行末のコメントに書いてください。

**最小の再現**:

```rule
rule t(t) v1

source 寸法表 = file "寸法表.md" sha256:a19333e262c10371
  表1 sha256:a5c05813ad703b8e

inputs
  a(a) : length[cm]  range >=1cm <=80cm

outputs
  x(x) : money[円]  round up(10円)

table 表(t1)  @寸法表 表1
policy unique
| a             | -> x   |
| <60cm         | 1410円 |
| >=60cm <=80cm | 1710円 |
```

隣に置く `寸法表.md`:

```proto
# 寸法表

| サイズ | 運賃 |
|---|---|
| 60cmまで | 1410円 |
| 60cmを超え80cm以下 | 1710円 |
```

隣に置く `寸法表.md.fragments/表1.tsv`:

```proto
サイズ	運賃
60cmまで	1410円
60cmを超え80cm以下	1710円
```

関係するコード: [E116](#e116), [W120](#w120), [E105](#e105)

## W122

`warning` — **その `shape` を使っている入力がありません**

**いつ出るか。** `shape` を宣言しているのに、`from <その名前>.…` と書いた入力が一つも無いとき（§15.125）。契約は読まれますが、何も確かめていません。

**直し方。** 使うか、消してください。読まれているだけの契約は、次に読む人に「ここは契約に縛られている」と思わせます。縛られているのは `from` を書いた入力だけです。

**最小の再現**:

```rule
rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)
policy unique
| a | -> x |
| - | true |
```

隣に置く `order.json`:

```proto
{"$defs":{"Order":{"type":"object","properties":{"lines":{"type":"array","items":{"type":"object","properties":{"category":{"type":"string"}}}}}}}}
```

関係するコード: [E121](#e121), [W111](#w111)

## E122

`error` — **契約が通す値を、規則が断ります**

**いつ出るか。** `from` で読む値について、契約の検証は通すのに、入力の宣言が受け付けない値があるとき（§15.132）。比べるのは、数の範囲（Protovalidate の `gte`・`lte` など、JSON Schema の `minimum`・`maximum`）、並びの件数（`min_items`・`max_items`、`minItems`・`maxItems`）、列挙の値（`string.in`、`enum`）、JSON Schema の `required` です。proto3 で注釈の無い数のフィールドは、入れ忘れると 0 として届くので、0 を受け付けない入力はここで止まります。`required` の無いメッセージのフィールドと `optional` のフィールドは省略でき、そのとき中の値は規則を通らずに既定値（0、""、0 件）として届きます。`.proto` の文字列から読む日付は、"" を通すかどうかを見ます（§15.133）。フィールドをまたぐ条件（CEL の式、`oneof`、JSON Schema の `allOf`・`anyOf`・`oneOf`・`not`・`if`）が一つのフィールドの幅を狭めていれば、それも読みます（§15.140）。JSON Schema の型に null があるのに、入力が省略できないときも、これです。読めない規則（剰余や文字列の関数を使う CEL など）は、無いものとして扱います。契約を実際より広く読むので、要らないところで言うことはあっても、見逃すことはありません。

**直し方。** どちらを直すかは人が決めます。その値が来ないはずなら、契約を狭めてください。`fix.text` が、契約に書く注釈そのものです（`narrow_contract`）。来るのなら、規則の範囲を広げるか列挙に値を足して、その値の答えを決めてください。`where` で絞った件数の下限のように契約に書けない前提もあり、そのときは `fix.kind` が `none` です。読む値が無いことがあるなら、入力を `T?` にしてください。無いときは none として読みます。

**最小の再現**:

```rule
rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  a(a) : number  range >=0 <=5  from count order.lines

outputs
  x(x) : bool

table 表(t1)
policy unique
| a | -> x |
| - | true |
```

隣に置く `order.json`:

```proto
{"$defs":{"Order":{"type":"object","properties":{"lines":{"type":"array","maxItems":10,"items":{"type":"object"}}},"required":["lines"]}}}
```

関係するコード: [W123](#w123), [E123](#e123), [E121](#e121), [E032](#e032)

## W123

`warning` — **行が、契約の通さない値でしか当たりません**

**いつ出るか。** 行のセルが `from` で読む入力を試していて、そのセルが受け付ける値を、契約の検証が一つも通さないとき（§15.132）。契約を通ったものしか来ないので、その行に当たる要求やメッセージはありません。比べるのは入力そのものの列だけで、そこから導いた値の列は見ません。

**直し方。** 契約がこの先も広がらないなら、行を消して、入力の範囲を契約に合わせてください。広がる予定があって残しているのなら、そのままで構いません。CI の `check --diff-base` は、新しく生じたものだけを報告します。

**最小の再現**:

```rule
rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  a(a) : number  range >=0 <=20  from count order.lines

outputs
  x(x) : bool

table 表(t1)
policy unique
| a    | -> x  |
| <=10 | true  |
| >10  | false |
```

隣に置く `order.json`:

```proto
{"$defs":{"Order":{"type":"object","properties":{"lines":{"type":"array","maxItems":10,"items":{"type":"object"}}},"required":["lines"]}}}
```

関係するコード: [E122](#e122), [W124](#w124), [E102](#e102), [W111](#w111)

## E123

`error` — **契約が、規則の `constraint` を破る組み合わせを通します**

**いつ出るか。** `constraint` の両側が、同じ `shape` から `from` で読む入力で、契約の検証を通る要求のなかに、両方の値が入力の範囲に入っているのに `constraint` を満たさないものがあるとき（§15.140）。契約がフィールドのあいだに置く条件（`.proto` のメッセージの CEL、`oneof`、JSON Schema の組み合わせ）を読んだうえで、それでも破る組み合わせが残るかを確かめます。見つかれば、その値を例に出します。読めない規則は無いものとして扱うので、見逃すことはありません。

**直し方。** どちらを直すかは人が決めます。その組み合わせが来ないはずなら、契約で約束してください。`.proto` なら、`fix.text` がメッセージに書く `(buf.validate.message).cel` です（`narrow_contract`）。JSON Schema には二つのフィールドの値を比べる書き方がないので、`fix.kind` は `none` です。来るのなら、`constraint` を外して、その組み合わせのときの答えを表で決めてください。

**最小の再現**:

```rule
rule t(t) v1

shape 見積(q) = proto "quote.proto" shop.v1.Quote

inputs
  最小(min_g) : mass[g]  range >=1g <=30kg  from 見積.min_g
  最大(max_g) : mass[g]  range >=1g <=30kg  from 見積.max_g

constraint 最小 <= 最大

outputs
  x(x) : bool

table 表(t1)
policy unique
| 最小 | -> x |
| -    | true |
```

隣に置く `quote.proto`:

```proto
syntax = "proto3";
package shop.v1;

message Quote {
  option (buf.validate.message).cel = {id: "express_cap", expression: "!this.express || this.weight_g <= 5000"};
  int64 min_g = 1 [(buf.validate.field).int64 = {gte: 1, lte: 30000}];
  int64 max_g = 2 [(buf.validate.field).int64 = {gte: 1, lte: 30000}];
  int64 weight_g = 3 [(buf.validate.field).int64 = {gte: 1, lte: 30000}];
  bool express = 4;
}
```

関係するコード: [E122](#e122), [W124](#w124), [E018](#e018)

## W124

`warning` — **行が、契約の通さない組み合わせでしか当たりません**

**いつ出るか。** 行のセルが、同じ `shape` から `from` で読む二つ以上の入力を試していて、セルを一つずつ見れば契約の通す値なのに、契約がフィールドのあいだに置く条件のもとでは、その組み合わせが一つも通らないとき（§15.140）。たとえば CEL の `this.min <= this.max` のもとで「最小が 20kg を超え、最大が 10kg 以下」を求める行や、一つの `oneof` の二つのメンバーをどちらも 0 でないとする行です。ほかの列のセルは見ないので、当たると言いすぎることはあっても、当たらないと言いすぎることはありません。

**直し方。** 契約がこの先も変わらないなら、行を消してください。変わる予定があって残しているのなら、そのままで構いません。CI の `check --diff-base` は、新しく生じたものだけを報告します。

**最小の再現**:

```rule
rule t(t) v1

shape 見積(q) = proto "quote.proto" shop.v1.Quote

inputs
  重さ(weight) : mass[g]  range >=1g <=30kg  from 見積.weight_g
  急ぎ(express) : bool  from 見積.express

outputs
  料金(fee) : money[円]  round up(10円)

table 料金表(fees)
policy first
| 急ぎ  | 重さ | -> 料金 |
| true  | >5kg | 3000円  |
| true  | -    | 1500円  |
| false | -    | 800円   |
```

隣に置く `quote.proto`:

```proto
syntax = "proto3";
package shop.v1;

message Quote {
  option (buf.validate.message).cel = {id: "express_cap", expression: "!this.express || this.weight_g <= 5000"};
  int64 min_g = 1 [(buf.validate.field).int64 = {gte: 1, lte: 30000}];
  int64 max_g = 2 [(buf.validate.field).int64 = {gte: 1, lte: 30000}];
  int64 weight_g = 3 [(buf.validate.field).int64 = {gte: 1, lte: 30000}];
  bool express = 4;
}
```

関係するコード: [W123](#w123), [E123](#e123), [E102](#e102)

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
| x | y    | -> r(r) : money[円, incl_tax] |
| a | -    | 100円                         |
| - | true | 200円                         |
| - | -    | 300円                         |
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
| a | true           |
| b | false          |
```

関係するコード: [W105](#w105), [E105](#e105)

## W111

`warning` — **使われていない宣言があります**

**いつ出るか。** 入力・導出・グループ・列挙の値が、どの表のどのセルにも現れないとき。書き忘れのしるしであることも、意図した契約であることもあります。`import` で持ち込んだ型の値は対象外です。

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
| x     | -> r(r) : bool |
| true  | true           |
| false | false          |
```

関係するコード: [E101](#e101), [E012](#e012)

## W116

`warning` — **どの例も使っていない `sequence` です**

**いつ出るか。** `sequence` を書いたのに、どの例もその名前を書いていないとき。並びは例から名指しされて初めて走るので、走っていない並びです（§15.56）。

**直し方。** その並びをたどる例を足すか、並びのほうを消してください。書いたのに使っていないのは、たいてい例を書き忘れた跡です。

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
| k     | -> d(d) : v |
| <=5円 | a           |
| >5円  | b           |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0円
  exhausted -> held

sequence s(s)
| k   |
| 3円 |
```

関係するコード: [E027](#e027), [W111](#w111)

## W119

`warning` — **ハッシュを書いた箇所が引用されていません**

**いつ出るか。** `source` の下にハッシュの行があるのに、その箇所を引用する `@` が規則のどこにも無いとき。引用を消したあとの残りです。

**直し方。** その行を消してください。`rulec source pin` が消します。

**最小の再現**:

```rule
rule t(t) v1

source 法 = law "000AC0000000001" asof 2026-04-01
  第1条 sha256:ce31217424a10206
  第2条 sha256:0000000000000000

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)  @法 第1条
| a | -> x |
| - | true |
```

隣に置く `sources/law/000AC0000000001@2026-04-01/MainProvision-Article_1.xml`:

```proto
<Article Num="1"><ArticleTitle>第一条</ArticleTitle><Paragraph Num="1"><ParagraphNum/><ParagraphSentence><Sentence>甲は、乙とする。</Sentence></ParagraphSentence></Paragraph></Article>
```

関係するコード: [E037](#e037)

## W120

`warning` — **写しの値を、どの行も使っていません**

**いつ出るか。** 表が `@出典 表1` で丸ごと引いている写しに、数だけでできたセルがあって、その値をどの行も使っていないとき（§15.82）。行を一本落としても完全性検査には出ません——落ちた行の入力は、残った行のどれかに当てはまってしまうからです。数だけのセルしか見ないので、`2026年4月1日改定` のような文は金額として数えません。`<=3kg` の一行で写しの `1kg`・`2kg`・`3kg` をまとめて写した場合も出ません。

**直し方。** 写しと見比べて、落とした行がないか確かめてください。改定で行が増えたのなら、その行をここに写します。引用した表のうち一部だけを写したのなら（発地ごとの運賃表から、一つの発地だけを写したときなど）、引用を `table` の行から、写した行それぞれの末尾へ移してください。行の引用は「この行はここから来た」としか言わないので、残りは問われなくなり、金額の突き合わせ（E116）は残ります。

**最小の再現**:

```rule
rule t(t) v1

source 料金表 = file "料金表.md" sha256:75465b330d123ab8
  表1 sha256:0a95cedbd7311274

inputs
  a(a) : bool

outputs
  x(x) : money[円]  round down(1円)

table 表(t1)  @料金表 表1
policy unique
| a | -> x  |
| - | 990円 |
```

隣に置く `料金表.md`:

```proto
# 料金表

| あて先 | 運賃 |
|---|---|
| 近畿 | 990円 |
| 関東 | 880円 |
```

隣に置く `料金表.md.fragments/表1.tsv`:

```proto
あて先	運賃
近畿	990円
関東	880円
```

関係するコード: [E116](#e116), [W119](#w119)

## W118

`warning` — **準用した表の行が、この規則ではどれも当たりません**

**いつ出るか。** 準用した表か節の**全行**が、この規則では当たらないとき。読み替えた値がその表の条件に届かないか、この規則のほかの定義（`overrides 準用名:表` で優先する節など）が全部先に取っています。一部の行が当たらないだけなら何も言いません。元の規則の表はこの規則より広い範囲に書かれているのが普通で、そうした行は `doc` が「この準用では当たらない行」として挙げます（§15.69）。

**直し方。** その表がこの準用に要らないなら `except <表>` で外してください。要るはずなら、読み替えか `with` の値の対応を見直してください。

**最小の再現**:

```rule
rule t(t) v1

inputs
  n(n) : number  range >=1 <=10

outputs
  y(y) : number  round down(1)

apply 呼(c) = "呼び先.rule" sha256:369102b8f803dc28
  a = n
  x -> y

clause 特例(special) -> 呼:x
  when always
  then 3
  overrides 呼:表
```

隣に置く `呼び先.rule`:

```proto
rule 呼び先(callee) v1

inputs
  a(a) : number  range >=1 <=10

outputs
  x(x) : number  round down(1)

table 表(t)
policy unique
   | a   | -> x |
小 | <=5 | 1    |
大 | >5  | 2    |
```

関係するコード: [E102](#e102), [W117](#w117)

## W117

`warning` — **効かない例外です**

**いつ出るか。** `overrides` で優先すると書いた相手の行と、この表の行が一つも交わらないとき。優先は、両方に当てはまる入力があるときにどちらが勝つかを決めるものなので、交わらなければ何も決めていません。ただし書が本文の一部を切り出す形になっていない、という転記の誤りの徴候です。

**直し方。** 原文と見比べて、この表の行の条件か相手の行の条件を直してください。本当に交わらないなら、`overrides` の行を消します。

**最小の再現**:

```rule
rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool

table 甲(ko)
   | a     | -> x  |
r1 | true  | true  |
r2 | false | false |

table 乙(otsu)
overrides 甲:r1, 甲:r2
| a    | -> x  |
| true | false |
```

関係するコード: [E035](#e035), [E105](#e105)

## W121

`warning` — **別名が生成先の言葉とぶつかります**

**いつ出るか。** ASCII の別名が、生成先のどれかの予約語か、その言語がすでに使っている名前と同じとき（§15.103）。別名はそのまま関数・引数・型・メンバの名前になります。

**直し方。** 予約語なら、その言語の生成コードはコンパイルが通りません（`type` を入力の別名にすると Rust が落ちます）。すでにある名前なら、規則の関数や列挙の型がそれを隠します（`sum` を規則の別名にすると Python の組み込みが隠れます）。引数やローカル変数の名前は、その本体の外までは隠しません。だからそこで出るのは予約語のときだけです。使わない生成先なら、このままで構いません。

**最小の再現**:

```rule
rule t(t) v1

inputs
  種別(type) : number  range >=0 <=10

outputs
  o(o) : number  round down(1)

table j(j)
policy first
| 種別 | -> o(o) : number |
| >=5  | 1                |
| -    | 0                |
```

関係するコード: [E009](#e009), [E011](#e011)

## W115

`warning` — **どの要素もこの判定にはなりません**

**いつ出るか。** `fold` にその判定の行き先があるのに、表のどの行もその判定を出さないとき。E024 の裏返しで、こちらは穴ではなく届かない行き先です。書き忘れではなく、表のほうが変わった跡であることが多い（§15.56）。

**直し方。** 表の行を見直すか、その行き先を消してください。どちらが正しいかは表のほうを読まないと決まりません。

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
| - | a           |

fold d over xs
  a -> take_first k
  b -> next
  empty -> 0円
  exhausted -> held
```

関係するコード: [E024](#e024)

## W114

`warning` — **未確認の重なり: 両方に当てはまる入力が有り得ます**

**いつ出るか。** `policy unique` の表で二行が重なりうるが、それを実際に起こす入力を構成できず、実現不能の証明もできなかったとき。導出が入力を共有する形（§15.126）と、真偽の `define` の中の閾値（§15.127）は、消去が決めるようになったのでここには落ちてきません。残るのは、消去が有理数の上で解いているために決まらない形です——上の例の `倍` は必ず偶数なので `5円` ちょうどにはなりませんが、有理数には `2.5円` があります。予算（400 本）を超えた系と、単位をまたぐ系も同じで、どれも「証明できなかった」であって「起こりうる」ではありません。

**直し方。** その条件を同時に満たす注文が存在するなら、行を直してください（出力が違うので、当てはまれば矛盾です）。存在しないならこのままで構いません — 生成コードには、万一その条件に当てはまる入力が来たとき黙って先の行を選ばずエラーを返すガードが入ります。

**最小の再現**:

```rule
rule t(t) v1

inputs
  a(a) : money[円]  range >=0円 <=10万円

outputs
  r(r) : bool

derive 倍(d) : money[円] = a + a  range >=0円 <=20万円

define 上(up) : bool = 倍 >= 5円
define 下(dn) : bool = 倍 <= 5円

table j(j)
policy unique
| 上    | 下    | -> r(r) : bool |
| true  | -     | true           |
| -     | true  | false          |
| false | false | false          |
```

関係するコード: [E105](#e105), [W105](#w105), [E109](#e109)
