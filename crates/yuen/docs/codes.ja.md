# 診断のコード

`yuen explain --all --format markdown --lang ja` の出力です。手で直しません。

<a id="e001"></a>

## E001 — 読めない字句があります

**いつ出るか**: yuen で使えない文字、閉じていない文字列、`\"` と `\\` のほかのエスケープ、形の崩れた日付・ハッシュ・版・別名、ASCII の数字で始まる ASCII の名前、全角の空白があるとき。

**直し方**: 示された位置を直してください。文字列は `"…"` で閉じ、日付は `2026-10-03`、ハッシュは `sha256:` と 16 桁の小文字の 16 進数、別名は `(payment_day)` の形で書いてください。

**再現**:

```req
requirements 例 v1
description "閉じていない
```

関連: [E002](#e002)

<a id="e002"></a>

## E002 — この位置に書けない語があります

**いつ出るか**: yuen に無い行、行の中で構文が受け付けない語、足りない語、名前に使った yuen の語（`text` など）があるとき。

**直し方**: 注に挙がる書き方のどれかにしてください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  because "y"
```

関連: [E004](#e004)

<a id="e003"></a>

## E003 — ファイルが `requirements` の行で始まっていません

**いつ出るか**: コメントと空行を除いた最初の行が `requirements …` でないとき、ファイルが空のとき。

**直し方**: `requirements 民法の期間 v1` のように、要件の集まりの名前と版で書き始めてください。

**再現**:

```req
role 法務
```

関連: [E004](#e004)

<a id="e004"></a>

## E004 — 節や行の順序か数が違います

**いつ出るか**: ファイルの節が `requirements`、`description`、`role`、`source`、`scope`、`requirement` の順にないとき、要件の中の行が `text`、`in force`、`owner`、`replaces`、`from`、`decided`、`satisfied by`・`not satisfied`、`verified by`・`not verified` の順にないとき、一つだけの行（`description`、`text`、`in force`、`owner`、確かめた記録）が二つあるとき。

**直し方**: 決まった順序に並べ替え、二つめを消してください。

**再現**:

```req
requirements 例 v1
role 法務
description "役割より後ろ"
```

関連: [E002](#e002)

<a id="e005"></a>

## E005 — 字下げが合いません

**いつ出るか**: 字下げにタブがあるとき、同じブロックの行の字下げがそろっていないとき、字下げした行を続けられないところに字下げした行があるとき、確かめた記録の行がリンクか見送りの行のすぐ下に一段深く書かれていないとき。

**直し方**: 字下げにはスペースを使い、同じブロックの行はそろえ、記録はリンクの行より深く字下げしてください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
	text "x"
```

<a id="e006"></a>

## E006 — 無い日付か、終わりが始まりより前の期間が書かれています

**いつ出るか**: `2026-02-30` のように暦に無い日付を書いたとき、期間の終わりが始まりより前のとき。書ける日付は 0001-01-01〜9999-12-31 です。

**直し方**: 暦にある日付に直し、期間は始まりを先に書いてください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-02-30 by 法務 "例"
```

関連: [E001](#e001)

<a id="e007"></a>

## E007 — 同じ名前を二度宣言しています

**いつ出るか**: 要件（版を書かずに二つ）、要件の別名、役割、同じファイルの出典、ファイルの見出しの名前のどれかが二度出てくるとき、一つの要件に別名が二つあるとき。

**直し方**: どちらかの名前を変えてください。同じ要件の版なら `v1`、`v2` と書き分けてください。

**再現**:

```req
requirements 例 v1
role 法務
role 法務
```

関連: [E009](#e009)

<a id="e008"></a>

## E008 — 宣言されていない名前です

**いつ出るか**: `owner`、`decided … by`、確かめた記録の `by`、`review --by` の役割が宣言されていないとき、`from` と `replaces` の要件がプロジェクトに無いとき。

**直し方**: `role 法務 "…"` で役割を宣言するか、名前の書き間違いを直してください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務部
  decided 2026-10-03 by 法務 "例"
```

関連: [E007](#e007)

<a id="e009"></a>

## E009 — 版の書き方が違います

**いつ出るか**: `v0` と書いたとき、同じ版が二つあるとき、版が二つ以上ある要件の版で版を書かなかったとき、版が二つ以上ある要件を版を書かずに指したとき、無い版を指したとき。

**直し方**: 版の番号は `v1` から始まります。版が二つ以上あるなら、どの版にも版を書き、その要件を指すときにも版を書いてください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1 v0
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
```

関連: [E007](#e007), [E408](#e408)

<a id="e010"></a>

## E010 — 要件に要るものがありません

**いつ出るか**: 要件に `text` か `owner` が無いとき、出どころ（`from` も `decided` も）が無いとき、名前が ASCII の小文字・数字・`_` でない要件に別名が無いとき。

**直し方**: 足りない行を書いてください。別名は `支払日(payment_day)` のように、名前のすぐあとに付けてください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  owner 法務
  decided 2026-10-03 by 法務 "例"
```

<a id="e011"></a>

## E011 — 名指しの最初の語が、ツールの語ではありません

**いつ出るか**: 名指しの最初の語が、rulec、dandori、koyomi、chobo、geas、proto、file、yuen、sakai のどれでもないとき（`dir` も、名指しの語ではありません）。

**直し方**: 九つのどれかを書いてください。ほかのファイルは `file "…"` で名指してください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  satisfied by excel "a.xlsx"
```

関連: [E012](#e012), [E013](#e013)

<a id="e012"></a>

## E012 — そこに書けない種類か組があります

**いつ出るか**: ツールに無い種類を書いたとき、子の種類（`method`、`field`、`value`）が親のすぐあとにないとき、子の組や、入れ子の無いツールの組が二つあるとき、file に種類を書いたとき、種類のあとに名前が無いとき。`source` や `yuen` の名指しをリンクや範囲に書いたとき、借りた出典が rulec か koyomi の `source` でないときにも出ます。

**直し方**: 診断の文に挙がる種類のどれかを書いてください。file はファイルを丸ごと名指します（`file "src/app.py"`）。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  satisfied by dandori "order.flow" table reserve
```

関連: [E011](#e011), [E013](#e013)

<a id="e013"></a>

## E013 — パスの書き方が違います

**いつ出るか**: パスに引用符が無いとき、空のとき、絶対パスのとき、`..` を解いたあとでルートの外に出るとき。

**直し方**: 名指しを書いたファイルのディレクトリからの相対パスを、`"…"` で囲んで書いてください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  satisfied by file "/etc/hosts"
```

関連: [E011](#e011), [E012](#e012)

<a id="e101"></a>

## E101 — 出典のコピーがありません

**いつ出るか**: 固定した条のコピー（`sources/law/<ID>@<日付>/<要素>.xml`）か、`file` の出典のファイルか、`openspec` の出典の仕様が無いとき。check は通信しません。

**直し方**: `yuen source fetch` でコピーを取ってくるか、パスを直してください。OpenSpec の仕様はプロジェクトのファイルなので、パスを直します。

**再現**:

```req
requirements 例 v1
role 法務

source 民法 = law "129AC0000000089" asof 2026-10-01
  第142条 sha256:fc8c35a0769d3b35

requirement r1
  text "x"
  owner 法務
  from @民法 第142条
  not satisfied "例なので置かない"
  not verified "例なので置かない"
```

関連: [E102](#e102), [E103](#e103)

<a id="e102"></a>

## E102 — 出典が固定されていません

**いつ出るか**: 引いている条か OpenSpec の要件に固定の行が無いとき、固定の行や `file` の出典に `sha256:` が無いとき。

**直し方**: コピー（OpenSpec の要件なら、そのブロック）の SHA-256 の先頭 16 桁を書いてください（直した行が注に出ます。`yuen source pin` でも書けます）。

**再現**:

```req
requirements 例 v1
role 法務

source 民法 = law "129AC0000000089" asof 2026-10-01
  第142条

requirement r1
  text "x"
  owner 法務
  from @民法 第142条
  not satisfied "例なので置かない"
  not verified "例なので置かない"
```

関連: [E101](#e101), [E103](#e103)

<a id="e103"></a>

## E103 — コピーが固定と違います

**いつ出るか**: コピーのハッシュが、固定の行の `sha256:` と違うとき。固定したあとでコピーが変わっています。OpenSpec の要件なら、変更の archive か書き直しで、要件のブロックが変わっています（確かめたときのブロックが reviewed/ にあれば、差分を見せます）。

**直し方**: 何が変わったかを読んでから（`yuen source outdated`）、固定を書き換えてください。

**再現**:

```req
requirements 例 v1
role 法務

source 民法 = law "129AC0000000089" asof 2026-10-01
  第142条 sha256:0000000000000000

requirement r1
  text "x"
  owner 法務
  from @民法 第142条
  not satisfied "例なので置かない"
  not verified "例なので置かない"
```

関連: [E102](#e102), [E302](#e302)

<a id="e104"></a>

## E104 — コピーが読めません

**いつ出るか**: 法令のコピーが UTF-8 の XML でないか、e-Gov や eCFR が配る形（条なら `<Article>`、eCFR の section なら `<DIV8>` で始まる）でないとき。OpenSpec の仕様が、UTF-8 でないか、`## Requirements` の節を持たないか（変更の提案の差分を名指したときを含む）、同じ名前の要件を二つ持つときにも出ます。

**直し方**: コピーは手で直さず、`yuen source fetch` で取り直してください。OpenSpec の仕様は `openspec/specs/<capability>/spec.md` を名指し、形の誤りを `openspec validate --specs` で直してください。

**再現**:

```req
requirements 例 v1
role 法務

source 民法 = law "129AC0000000089" asof 2026-10-01
  第142条 sha256:6210aedce8fd1601

requirement r1
  text "x"
  owner 法務
  from @民法 第142条
  not satisfied "例なので置かない"
  not verified "例なので置かない"
```

`sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml`:

```
not xml
```

関連: [E101](#e101)

<a id="e105"></a>

## E105 — 引用が使えません

**いつ出るか**: 引用の条が読めない形のとき、同じファイルで宣言されていない出典を引いたとき、法令を条なしで、OpenSpec の仕様を要件なしで引いたとき、`file` の出典に条を書いたとき。

**直し方**: 出典を同じファイルで宣言し、法令は `@民法 第142条` のように条で、`file` の出典は `@約款` と丸ごと引いてください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  from @商法 第1条
  not satisfied "例なので置かない"
  not verified "例なので置かない"
```

関連: [E102](#e102)

<a id="e106"></a>

## E106 — 借りた出典が使えません

**いつ出るか**: 借りた出典を、名指したファイルが宣言していないか、引いた条を固定していないとき。ファイルが無いとき、コピーが読めないか固定と違うときにも出ます。

**直し方**: そのファイルが宣言して固定している出典と条を書いてください。ほかの条を引くなら、そのファイルに固定の行を足してください。

**再現**:

```req
requirements 例 v1
role 法務

source 民法 = koyomi "a.cal" source 民法

requirement r1
  text "x"
  owner 法務
  from @民法 第142条
  not satisfied "例なので置かない"
  not verified "例なので置かない"
```

`a.cal`:

```
dates 例(example) v1

inputs
  起点(origin) : date  range >=2026-01-01 <=2026-12-31

date 翌日(next_day) = 起点
  + 1 day
```

関連: [E105](#e105), [E203](#e203)

<a id="e107"></a>

## E107 — 要件と成果物が、同じ条の違う本文を読んでいます

**いつ出るか**: 要件が引く条を、それを満たす規則かカレンダーのファイルも固定していて、どのコピーも要件のコピーと本文が違うとき。どちらかが古いコピーです。

**直し方**: 本文の差分を読み、古いほうのコピーを取り直して固定し直してください。

**再現**:

```req
requirements 例 v1
role 法務

source 民法 = law "129AC0000000089" asof 2026-10-01
  第142条 sha256:54a319e4148c24c7

requirement r1
  text "x"
  owner 法務
  from @民法 第142条
    reviewed 2026-10-04 by 法務 sha256:54a319e4148c24c7 -> sha256:f0db3717ffd12ba0
  satisfied by koyomi "cal/a.cal" date 満了日
    reviewed 2026-10-04 by 法務 sha256:f0db3717ffd12ba0 -> sha256:3c4a7518c3f1dead
  not verified "例なので置かない"
    approved 2026-10-04 by 法務 sha256:f0db3717ffd12ba0
```

`cal/a.cal`:

```
dates 例(example) v1

source 民法 = law "129AC0000000089" asof 2026-10-01
  第142条 sha256:fc8c35a0769d3b35

inputs
  起点(origin) : date  range >=2026-01-01 <=2026-12-31

date 満了日(last_day) = 起点  @民法 第142条
  + 1 day
```

関連: [E103](#e103)

<a id="e108"></a>

## E108 — OpenSpec の仕様に、その名前の要件がありません

**いつ出るか**: `openspec` の出典の固定の行か引用が名指す要件が、仕様に無いとき。OpenSpec は名前を書いたとおりに比べるので、大文字と小文字や空白だけが違うときにも出ます（そのときは近い名前を注に挙げます）。変更を archive して名前が変わった（RENAMED）か、無くなった（REMOVED）ときにも出ます。

**直し方**: 仕様の `### Requirement:` のあとの名前を、そのまま書いてください。archive で変わったのなら、その要件を読む要件を見直してから、固定と引用を直してください。

**再現**:

```req
requirements 例 v1
role 開発

source あいさつ = openspec "openspec/specs/greeting/spec.md"
  "名前で あいさつする" sha256:0000000000000000

requirement r1
  text "x"
  owner 開発
  from @あいさつ "名前で あいさつする"
  not satisfied "例なので置かない"
  not verified "例なので置かない"
```

関連: [E101](#e101), [E104](#e104), [W102](#w102)

<a id="w101"></a>

## W101 — 固定した条が、どの要件からも引かれていません

**いつ出るか**: 出典の下に固定の行があるのに、同じファイルのどの要件の `from` もその条を引いていないとき。

**直し方**: 引用を消したあとの残りなら、固定の行を消してください。

**再現**:

```req
requirements 例 v1
role 法務

source 民法 = law "129AC0000000089" asof 2026-10-01
  第142条 sha256:fc8c35a0769d3b35

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  not satisfied "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
```

関連: [E102](#e102)

<a id="w102"></a>

## W102 — OpenSpec の仕様の要件を、どの出典も固定していません

**いつ出るか**: プロジェクトが `openspec` の出典として読む仕様に、プロジェクトのどの出典も固定していない要件があるとき。仕様に書いた要件を、プロジェクトのどの要件も読んでいません。一つの仕様について、それを名指す最初の出典に一度だけ出します。

**直し方**: 固定の行を足し、それを引く要件を書いてください。読まないと決めた要件も、引く要件を書いて `not satisfied` と `not verified` に理由を書けば、外したことが承認とともに残ります。

**再現**:

```req
requirements 例 v1
role 開発

source あいさつ = openspec "openspec/specs/greeting/spec.md"

requirement r1
  text "x"
  owner 開発
  decided 2026-10-05 by 開発 "例"
  not satisfied "例なので置かない"
    approved 2026-10-05 by 開発 sha256:fbdfb71af500ce5f
  not verified "例なので置かない"
    approved 2026-10-05 by 開発 sha256:fbdfb71af500ce5f
```

関連: [W101](#w101), [E108](#e108)

<a id="e201"></a>

## E201 — 成果物のファイルがありません

**いつ出るか**: リンクが名指すファイルか、範囲のパスが無いとき。リンクにディレクトリを書いたときにも出ます。

**直し方**: パスを直してください。ファイルの名前を変えたのなら、リンクも直してください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  satisfied by file "missing.txt"
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
```

関連: [E013](#e013)

<a id="e202"></a>

## E202 — 成果物の名前が、そのファイルにありません

**いつ出るか**: 名指した名前が、そのファイルに無いとき（その言語が渡す名前にも、`.proto` の中にも無いとき）。別名で書いたときや、名前が変わったときにも出ます（注に候補が出ます）。

**直し方**: 別名ではなく、その言語の名前で書いてください。名前が変わったのなら、リンクも直してください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  satisfied by proto "a.proto" message Orders
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
```

`a.proto`:

```
syntax = "proto3";

package shop.v1;

message Order {
  string id = 1;
}
```

関連: [E201](#e201)

<a id="e203"></a>

## E203 — 名指したものの言語から、そのファイルの情報を得られません

**いつ出るか**: 名指したファイルが、その言語の検査を通らないか、読めないとき。そのようなファイルからは成果物の定義を読み取れないので、yuen はハッシュを取れません。注には、その言語の診断が並びます。

**直し方**: そのファイルを、その言語の検査を通るように直してください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  satisfied by rulec "a.rule" table fees
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
```

`a.rule`:

```
rule fee v1

inputs
  amount : money[JPY]  range >=0JPY <=10000JPY

outputs
  fee : money[JPY]  round down(1JPY)

table fees
| amount       | -> fee  |
| <5000JPY     | 500JPY  |
| >5000JPY     | 0JPY    |
```

関連: [E106](#e106), [E202](#e202)

<a id="e204"></a>

## E204 — ツールの JSON が、yuen の知らない形です

**いつ出るか**: いまは出ません。ツールの JSON に、yuen が読むキーが無いときのためのコードでした。

**直し方**: 直すものはありません。

**再現**: ritsu 0.23.0 で使われなくなりました。ほかの言語を子プロセスの JSON で読む予定だったころに決めたコードです。いまの yuen は、ほかの言語を ritsu の中で型のまま読み、JSON を読みません。

関連: [E203](#e203)

<a id="e205"></a>

## E205 — proto が読めません

**いつ出るか**: `.proto` を、ritsu の `.proto` のパーサーが読めないとき（proto3 として読めないとき、import の先が読めないとき）。

**直し方**: `.proto` を直してください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  satisfied by proto "a.proto" message Order
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
```

`a.proto`:

```
syntax = "proto3";

package shop.v1;

message Order {
  string id = 1;
```

関連: [E201](#e201)

<a id="e206"></a>

## E206 — 名指したものの言語がつながっていません

**いつ出るか**: yuen 単独のバイナリ（`yuen`）に、ほかの言語のもの（規則、カレンダー、帳簿、主張、ワークフロー、コンテキスト）を名指すプロジェクトを渡したとき。このバイナリにはほかの言語が入っておらず、それを読めません。yuen は、言語ごとに一度、最初に名指したところでこのエラーを出し、そこで止まります。exit code は 2 です（プロジェクトの誤りではなく、走らせ方の問題なので）。

**直し方**: 同じコマンドを `ritsu yuen` で走らせてください（`ritsu yuen check .`）。ritsu はすべての言語をつなぎ、同じプロセスの中で読みます。

**再現**:

```req
requirements shipping v1
role legal

requirement r1
  text "a fee for every amount"
  owner legal
  decided 2026-10-04 by legal "an example"
  satisfied by rulec "fee.rule" table fees
```

`fee.rule`:

```
rule fee v1

inputs
  amount : money[USD]  range >=0USD <=10000USD

outputs
  fee : money[USD]  round down(1USD)

table fees
| amount    | -> fee |
| <5000USD  | 5USD   |
| >=5000USD | 0USD   |
```

関連: [E203](#e203)

<a id="w201"></a>

## W201 — geas の記録が無いので、主張があるかを確かめていません

**いつ出るか**: いまは出ません。`geas map` の記録が無く、spec のファイルがあることしか確かめられないときのためのコードでした。

**直し方**: 直すものはありません。

**再現**: ritsu 0.23.0 で使われなくなりました。いまは geas が spec から主張を読むので、yuen は記録が無くても主張があるかを確かめます。記録が無いために辿れない範囲のファイルは、E404 の注に出ます。

関連: [E202](#e202), [E404](#e404)

<a id="e301"></a>

## E301 — まだ確かめていないリンクです

**いつ出るか**: リンクの下に、確かめた記録（`reviewed …`）が無いとき。

**直し方**: 両端を読んで確かめたら、`yuen review … --by <役割>` で記録を書いてください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  satisfied by file "a.txt"
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
```

`a.txt`:

```
a
```

関連: [E302](#e302), [E303](#e303)

<a id="e302"></a>

## E302 — 確かめたあとで、リンク元が変わりました

**いつ出るか**: 記録のリンク元のハッシュが、いまのハッシュと違うとき。リンク元は、`from` なら出典の条か元の要件、`satisfied by` と `verified by` なら要件です。要件のハッシュは出典のハッシュも含めて取るので、条が変われば、その先のリンクにも印が付きます。

**直し方**: 差分を読み、要件がまだ正しく読めているか、成果物がまだ満たしているかを確かめてから、`yuen review` で記録を書き直してください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  satisfied by file "a.txt"
    reviewed 2026-10-03 by 法務 sha256:f1e653e8ce72c16f -> sha256:87428fc522803d31
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
```

`a.txt`:

```
a
```

`reviewed/f1e653e8ce72c16f`:

```
text the old x
```

関連: [E303](#e303), [E304](#e304), [W301](#w301)

<a id="e303"></a>

## E303 — 確かめたあとで、リンク先が変わりました

**いつ出るか**: 記録のリンク先のハッシュが、いまのハッシュと違うとき。リンク先は、`from` なら要件、`satisfied by` と `verified by` なら成果物です。

**直し方**: 差分を読み、成果物がまだ要件を満たしているかを確かめてから、`yuen review` で記録を書き直してください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  satisfied by file "a.txt"
    reviewed 2026-10-03 by 法務 sha256:fbdfb71af500ce5f -> sha256:0263829989b6fd95
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
```

`a.txt`:

```
a
```

`reviewed/0263829989b6fd95`:

```
b
```

関連: [E302](#e302), [W301](#w301)

<a id="e304"></a>

## E304 — 見送りが承認されていないか、承認のあとで要件が変わりました

**いつ出るか**: `not satisfied` か `not verified` の下に承認の記録（`approved …`）が無いとき、記録の要件のハッシュがいまと違うとき。

**直し方**: 持ち主が理由を読んで承認したら、`yuen review … --by <役割>` で承認を書いてください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  not satisfied "例なので置かない"
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
```

関連: [E301](#e301)

<a id="e305"></a>

## E305 — 確かめた記録の形が崩れています

**いつ出るか**: 記録の行の形が崩れているとき（`->` や `by` が無いとき、見送りの下に `reviewed` があるとき）、ハッシュの数がリンク元の数と合わないとき。

**直し方**: 記録は `yuen review` が書くものです。確かめ直してから、`yuen review` で書き直してください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  satisfied by file "a.txt"
    reviewed 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
```

`a.txt`:

```
a
```

関連: [E301](#e301)

<a id="w301"></a>

## W301 — 確かめたときの中身が reviewed/ に無いので、差分を見せられません

**いつ出るか**: 印の付いたリンクの、確かめたときの中身（`reviewed/<ハッシュ>`）が無いとき。印は、中身が無くてもハッシュで付きます。

**直し方**: `reviewed/` を git に入れておいてください。無くした中身は、git の履歴から戻せることがあります。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  satisfied by file "a.txt"
    reviewed 2026-10-03 by 法務 sha256:fbdfb71af500ce5f -> sha256:0263829989b6fd95
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
```

`a.txt`:

```
a
```

関連: [E302](#e302), [E303](#e303)

<a id="e401"></a>

## E401 — 満たす成果物も、その見送りもありません

**いつ出るか**: 要件の版に、`satisfied by` も `not satisfied` も無いとき。

**直し方**: `satisfied by <成果物>` を書くか、`not satisfied "<理由>"` を書いて承認してもらってください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
```

関連: [E402](#e402)

<a id="e402"></a>

## E402 — 確かめる主張も、その見送りもありません

**いつ出るか**: 要件の版に、`verified by` も `not verified` も無いとき。

**直し方**: `verified by <主張>` を書くか、`not verified "<理由>"` を書いて承認してもらってください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  not satisfied "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
```

関連: [E401](#e401)

<a id="e403"></a>

## E403 — 確かめる側に、何も確かめないものが書かれています

**いつ出るか**: `verified by` に、geas と koyomi の主張、検査するツールのファイル全体、テストのファイルのほかを書いたとき（rulec の出力、chobo の振替、proto など）。

**直し方**: 満たすものなら `satisfied by` に書いてください。確かめる側には、落ちることのあるものを書いてください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  verified by koyomi "支払条件.cal" date 支払日
```

関連: [E012](#e012)

<a id="e404"></a>

## E404 — 範囲の成果物を、どの要件からも辿れません

**いつ出るか**: `scope` が集めた成果物を、どのリンクも名指していないとき（それを含むものも、それに含まれるものも名指していないとき）。

**直し方**: `satisfied by` か `verified by` でそれを名指す要件を足すか、範囲を狭めてください。

**再現**:

```req
requirements 例 v1
role 法務

scope file "b.txt"

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  not satisfied "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
```

`b.txt`:

```
b
```

関連: [E401](#e401)

<a id="e405"></a>

## E405 — 要件のあいだに循環があります

**いつ出るか**: `from <要件>` と `replaces <要件>` をたどると、元の要件に戻ってくるとき。

**直し方**: どちらが元かを決め、もう一方の `from` か `replaces` を消してください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  from r2
  not satisfied "例なので置かない"
  not verified "例なので置かない"

requirement r2
  text "y"
  owner 法務
  from r1
  not satisfied "例なので置かない"
  not verified "例なので置かない"
```

関連: [E409](#e409)

<a id="e406"></a>

## E406 — 版の期間に隙間があります

**いつ出るか**: 前の版の終わりの翌日に、次の版が始まらないとき。注に、どの版にも入らない日が出ます。

**直し方**: 次の版の始まりを、前の版の終わりの翌日にしてください（直した行が注に出ます）。

**再現**:

```req
requirements 例 v1
role 法務

requirement x v1
  text "x"
  in force 2026-01-01..2026-12-31
  owner 法務
  decided 2026-10-03 by 法務 "例"
  not satisfied "例なので置かない"
    approved 2026-10-03 by 法務 sha256:5ca1701c0312a54b
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:5ca1701c0312a54b

requirement x v2
  text "x"
  in force 2027-01-02..
  owner 法務
  decided 2026-10-03 by 法務 "例"
  not satisfied "例なので置かない"
    approved 2026-10-03 by 法務 sha256:4e7392102a031a5b
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:4e7392102a031a5b
```

関連: [E407](#e407), [E408](#e408)

<a id="e407"></a>

## E407 — 版の期間が重なります

**いつ出るか**: 次の版が、前の版の終わり以前に始まるとき。注に、重なる日と二つの版が出ます。

**直し方**: 一つの日に効く版は一つです。前の版の終わりか、次の版の始まりを直してください。

**再現**:

```req
requirements 例 v1
role 法務

requirement x v1
  text "x"
  in force 2026-01-01..2026-12-31
  owner 法務
  decided 2026-10-03 by 法務 "例"
  not satisfied "例なので置かない"
    approved 2026-10-03 by 法務 sha256:5ca1701c0312a54b
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:5ca1701c0312a54b

requirement x v2
  text "x"
  in force 2026-12-30..
  owner 法務
  decided 2026-10-03 by 法務 "例"
  not satisfied "例なので置かない"
    approved 2026-10-03 by 法務 sha256:7523d312c91d81c0
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:7523d312c91d81c0
```

関連: [E406](#e406), [E408](#e408)

<a id="e408"></a>

## E408 — 版の期間が、そろって書かれていません

**いつ出るか**: 版が二つ以上あるのに期間の無い版があるとき、終わりを開けた版が最後でないとき、始まりを開けた版が最初でないとき、版の番号が期間の順に増えていないとき。

**直し方**: どの版にも `in force` を書き、版の番号を期間の順にそろえてください。

**再現**:

```req
requirements 例 v1
role 法務

requirement x v1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  not satisfied "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f

requirement x v2
  text "x"
  in force 2027-01-01..
  owner 法務
  decided 2026-10-03 by 法務 "例"
  not satisfied "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fa2942b05a851b79
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fa2942b05a851b79
```

関連: [E406](#e406), [E407](#e407)

<a id="e409"></a>

## E409 — 置き換える要件の期間が、置き換えられる要件の終わりの翌日から始まりません

**いつ出るか**: `replaces` で置き換えられる要件に終わりの日が無いとき、`replaces` を書いた版の始まりが、その翌日でないとき。

**直し方**: 置き換えられる要件に終わりの日を書き、置き換える版をその翌日から始めてください（直した行が注に出ます）。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  in force 2026-01-01..2026-12-31
  owner 法務
  decided 2026-10-03 by 法務 "例"
  not satisfied "例なので置かない"
    approved 2026-10-03 by 法務 sha256:5ca1701c0312a54b
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:5ca1701c0312a54b

requirement r2
  text "y"
  in force 2027-01-02..
  owner 法務
  replaces r1
  decided 2026-10-03 by 法務 "例"
  not satisfied "例なので置かない"
    approved 2026-10-03 by 法務 sha256:cb98f1b81b3a40fd
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:cb98f1b81b3a40fd
```

関連: [E406](#e406), [E405](#e405)

<a id="w401"></a>

## W401 — 見送りと、同じ側のリンクの両方があります

**いつ出るか**: `satisfied by` と `not satisfied`（か `verified by` と `not verified`）が同じ要件の版にあるとき。見送りは要りません。

**直し方**: リンクを置いたのなら、見送りを消してください。

**再現**:

```req
requirements 例 v1
role 法務

requirement r1
  text "x"
  owner 法務
  decided 2026-10-03 by 法務 "例"
  satisfied by file "a.txt"
    reviewed 2026-10-03 by 法務 sha256:fbdfb71af500ce5f -> sha256:87428fc522803d31
  not satisfied "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
  not verified "例なので置かない"
    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f
```

`a.txt`:

```
a
```

関連: [E401](#e401)

<a id="w402"></a>

## W402 — OpenSpec のシナリオに、同じ名前の主張がありません

**いつ出るか**: OpenSpec の仕様の要件を引き、geas の主張で確かめている要件で、その仕様の要件のシナリオのうち、同じ名前の主張が、要件を確かめる主張（`verified by geas …`。spec を丸ごと名指したなら、その spec のすべての主張）の中に無いとき。変更の archive でシナリオが足されたあとによく出ます。

**直し方**: その名前の主張を、主張を読む人と一緒に書き、`verified by` でつないでください（`geas scenarios --draft` が下書きを出します）。別の名前の主張がそのシナリオを確かめているなら、どちらの名前を直すかを聞いてください。

**再現**:

```req
requirements 例 v1
role 開発

source あいさつ = openspec "openspec/specs/greeting/spec.md"
  知らないパス sha256:0f97c4b193dbba2b

requirement r1
  text "x"
  owner 開発
  from @あいさつ 知らないパス
  not satisfied "例なので置かない"
  verified by geas "greeter.geas" claim 名前であいさつする
```

`greeter.geas`:

```
target api {
  serve "python3 server.py {port}"
  port auto
}

claim "名前であいさつする" {
  when api.get("/greet?name=Alice")
  then status is 200
}
```

関連: [W102](#w102), [E402](#e402)

<a id="w901"></a>

## W901 — 鍵がファイルに書かれています

**いつ出るか**: `.req` のどこか（文字列でもコメントでも）に、鍵の形の値があるとき。調べる鍵は、AWS のアクセスキー ID、GitHub・Slack・Stripe・OpenAI・Anthropic・Google の鍵やトークン、Slack の Incoming Webhook の URL、PEM の秘密鍵で、どれもプロバイダーが接頭辞や形を決めているものです。ritsu のどの言語も同じ決まりで調べます。診断には鍵の種類と、接頭辞と、長さだけを出し、鍵そのものも、その行も出しません。

**直し方**: 鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。本物の鍵なら、まずプロバイダーで無効にしてください。ファイルから消しても、リポジトリの履歴には残ります。テスト用の値なら、同じ行のコメントに `ritsu: test secret` と書いてください。

**再現**:

```req
requirements 支払 v1
# 支払のサービスが祝日を読む Google カレンダーの API キー: AIzaSyD-ritsu-fake-key-for-tests-000000

role 経理

requirement 支払日(payment_day)
  text "20 日締め翌月 10 日払い"
  owner 経理
  decided 2026-10-03 by 経理 "例として決めた"
  not satisfied "この例では置かない"
    approved 2026-10-03 by 経理 sha256:cdd8998a17be5a4b
  not verified "この例では置かない"
    approved 2026-10-03 by 経理 sha256:cdd8998a17be5a4b
```
