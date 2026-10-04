# 診断のコード

`sakai explain --all --format markdown --lang ja` の出力です。手で直しません。

<a id="e001"></a>

## E001 — 読めない字句があります

**いつ出るか**: 閉じていない文字列、文字列の中の知らないエスケープ、全角の空白、名前に使えない文字があるとき。名前は文字、数字、`_` で書き、数字では始められません。

**直し方**: 示された位置を直してください。文字列は同じ行の `"` で閉じ、エスケープは `\"` と `\\` だけを使ってください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
description "閉じていない
owns
  dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E002](#e002)

<a id="e002"></a>

## E002 — この位置に書けない語があります

**いつ出るか**: その位置に、構文が受け付けない語があるとき。役割の無い `upstream`、名前にした予約語、見出しに無いバージョン、行の終わりの余分な語などです。

**直し方**: 注に挙がる書き方のどれかにしてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙
  through b.v1
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E004](#e004)

<a id="e003"></a>

## E003 — ファイルが `map` か `context` の行で始まっていません

**いつ出るか**: コメントと空行を除いた最初の行が、`map …` でも `context …` でもないとき。

**直し方**: コンテキストマップなら `map 通販(shop) v1`、一つのコンテキストなら `context 在庫(inventory) v1` のように書き始めてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
owns
  dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E004](#e004)

<a id="e004"></a>

## E004 — 節の順序か数が違います

**いつ出るか**: 節が決まった順序にないとき、一度だけの節が二度あるとき、省けない節（地図の `use context` と `covers`、コンテキストの `owns`、`upstream` の下の `through`）が無いとき、map のファイルに `owns` があるなど種類の違うファイルの節があるとき。語が定義の文と `as` のどちらか一つを持たないときにも出ます。

**直し方**: map のファイルは見出し、`description`、`use context`、`covers`、`except`、`proto root`、`code` の順に、context のファイルは見出し、`description`、`owner`、`also`、`owns`、`published language`、`terms`、関係の順に書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"
owner "甲のチーム"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E002](#e002), [E003](#e003)

<a id="e005"></a>

## E005 — 字下げが合いません

**いつ出るか**: 字下げにタブがあるとき、同じ節の行の字下げがそろっていないとき、受ける行の無いところに字下げした行があるとき。

**直し方**: 字下げにはスペースを使い、同じ節の行は同じ幅だけ字下げしてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
	dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

<a id="e006"></a>

## E006 — 同じ名前を二度宣言しています

**いつ出るか**: コンテキストの名前や別名、一つのコンテキストの語の名前と `also`、公表された言語の package、同じ相手への同じ関係、`owns` の項、対応の左辺のどれかが二度出てくるとき。

**直し方**: どちらかの名前を変えるか、二つめを消してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"
```

`乙.ctx`:

```ctx
context 甲(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E007](#e007)

<a id="e007"></a>

## E007 — 宣言されていない名前です

**いつ出るか**: 関係の相手や `as` のコンテキストが地図に無いとき、`means` や対応の要素が proto に無いとき。短い書き方の名前が二つの package に当たるときにも出ます。

**直し方**: 名前の書き違いを直すか、`use context` を足してください。二つの package に当たる名前は package から書いてください（`message warehouse.v1.ReserveResponse`）。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 丙 conformist
  through b.v1
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E006](#e006), [E410](#e410)

<a id="e008"></a>

## E008 — 地図かコンテキストに ASCII の別名がありません

**いつ出るか**: 見出しの名前のすぐあとに `(alias)` が無いとき、または別名が `[A-Za-z_][A-Za-z0-9_]*` の形でないとき。別名は CML の名前と、import の検査の設定の名前になります。

**直し方**: `受注(ordering)` のように、名前のすぐあとに丸括弧で書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲 v1
owns
  dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

<a id="e009"></a>

## E009 — 書いたパスがありません

**いつ出るか**: `use context`、`covers`、`except`、`proto root`、`code`、`owns`、公表された言語の `proto` と `crate` と `generated dir`、`layer`、共有カーネル、`means` のパスが、ディスクに無いとき。パスは、書いたファイルのディレクトリからの相対パスです。

**直し方**: パスを直してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a", "nowhere"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E012](#e012)

<a id="e010"></a>

## E010 — `use context` の先が使えません

**いつ出るか**: `use context` の先が map のファイルのとき、同じファイルを二度読むとき。

**直し方**: `use context` の先には context のファイルを書き、同じファイルは一度だけ読んでください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
use context "甲.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

<a id="e011"></a>

## E011 — 成果物の名指しの形が違います

**いつ出るか**: ツールの語と拡張子が合わないとき、`dir` や公表された言語の `crate` の先がファイルのとき、ツールの語の先がディレクトリのとき、知らないツールの語や、そのツールに無い種類の語を書いたとき、子の種類（`value`、`field`、`method`）が親のすぐあとにないとき、`file` に種類を書いたとき。対応の先が列挙でないときにも出ます。

**直し方**: `<ツール> "<パス>" [<種類> <名前>]…` の形で書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  rulec "a/a.proto"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E012](#e012)

<a id="e012"></a>

## E012 — 絶対パスか、ルートの外に出るパスか、空のパスです

**いつ出るか**: パスを `/` から書いたとき、`..` でルートの外に出るとき、`""` と空で書いたとき。ルートは、sakai に渡したパスの上で .git を持つ一番近いディレクトリです（無ければ渡したディレクトリ。`--root` で替えられます）。

**直し方**: ルートの中のパスを、書いたファイルのディレクトリからの相対パスで書いてください。そのディレクトリ自身は `"."` と書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a", "../outside"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E009](#e009)

<a id="e101"></a>

## E101 — どのコンテキストにも属さない成果物があります

**いつ出るか**: 範囲の成果物が、どのコンテキストの `owns` の項にも含まれないとき。属さないファイルしか含まないディレクトリは、いちばん上のディレクトリにまとめて一つの診断になります。

**直し方**: どれかのコンテキストの `owns` に、そのディレクトリかファイルを書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`c/c.proto`:

```proto
syntax = "proto3";
package c;
message C {}
```

関連: [E102](#e102)

<a id="e102"></a>

## E102 — 二つのコンテキストが同じ深さで持つ成果物があります

**いつ出るか**: 二つのコンテキストの `owns` が、同じディレクトリかファイルを書いているとき。成果物はそれを含むいちばん深い項のコンテキストに属するので、同じ深さの項が二つあると、どちらのものか決められません。

**直し方**: 一方を消すか、どちらかにもっと深いディレクトリを書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b", "a"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E101](#e101)

<a id="e103"></a>

## E103 — 範囲の外のものを、持っているか参照しています

**いつ出るか**: `owns` の項や、proto の import の先、`means` の proto が、地図の範囲の外にあるとき。範囲は `covers` で決まり、`except` と、パスに . で始まる名前、node_modules、site-packages、__pycache__、target を含むものは外れます。

**直し方**: 範囲を広げるか、項や参照を消してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
except "c"
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a", "c"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`c/c.proto`:

```proto
syntax = "proto3";
package c;
message C {}
```

<a id="e104"></a>

## E104 — ほかの言語の成果物を読めません（言語がつながっていません）

**いつ出るか**: 地図が rulec、koyomi、dandori の成果物を含むのに、その言語を読む部分がつながっていないとき（sakai 単独のバイナリで走らせたとき）。sakai は、言語ごとに一度、その最初の成果物を持つ `owns` の行でこのエラーを出します。確かめていないことを、黙って通すことはしません。exit code は 2 です（地図の誤りではなく、走らせ方の問題なので）。

**直し方**: すべての言語をつないだ `ritsu sakai` で走らせてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/x.rule`:

```rule
rule x(x) v1

import proto "../b/v1/b.proto" Kind -> 種類
enum 種類(kind) = 一(one) | 二(two)

inputs
  k : 種類

outputs
  n : 種類

table t
policy unique
| k  | -> n |
| 一 | 一   |
| 二 | 二   |
```

関連: [E105](#e105)

<a id="e105"></a>

## E105 — 成果物が、その言語の検査を通らないか、読めません

**いつ出るか**: rulec が規則の情報を返さないとき（規則が rulec の検査を通らないとき）や、koyomi と dandori がファイルを読めないとき。注には、その言語の診断が並びます。sakai は、その成果物の参照を確かめられません。

**直し方**: その成果物を、その言語の検査が通るように直してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/fee.rule`:

```rule
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

関連: [E104](#e104)

<a id="e106"></a>

## E106 — proto が読めません

**いつ出るか**: 範囲の `.proto` を、sakai が読めないとき（構文の誤り、proto2 の `group`）。

**直し方**: 示された位置を直してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = ; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

<a id="e107"></a>

## E107 — Cargo から Rust のクレートを読めません

**いつ出るか**: 地図が `code rust` を書いていて、その場所に `Cargo.toml` が無いか、`cargo metadata` が失敗したとき（cargo が無い、マニフェストが読めない、など）。sakai は、Rust のクレートとその依存を Cargo から読みます（DESIGN 7.7）。読めなければ、Rust のクレートの依存は確かめられません。

**直し方**: `code rust` には、ワークスペースの（クレートが一つなら、そのクレートの）`Cargo.toml` のあるディレクトリを書いてください。そのディレクトリで `cargo metadata --no-deps --offline` が通ることも確かめてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`shop.ctx`:

```ctx
map Shop(shop) v1
use context "orders.ctx"
covers "."
code rust "."
```

`orders.ctx`:

```ctx
context Orders(orders) v1
owns
  dir "orders"
```

`orders/Cargo.toml`:

```
[package]
name = "orders"
version = "0.1.0"
edition = "2021"
```

`orders/src/lib.rs`:

```
pub fn reserve() {}
```

<a id="w101"></a>

## W101 — `owns` の項が成果物を一つも含みません

**いつ出るか**: `owns` のディレクトリかファイルに、成果物が一つも無いとき。たいていはパスの書き誤りです。

**直し方**: パスを直すか、項を消してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a", "empty"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`empty/README.txt`:

```
nothing here is an artifact
```

<a id="w102"></a>

## W102 — proto の import が見つかりません

**いつ出るか**: import の先が、地図の `proto root`、その proto の package の形から決まるディレクトリ、その proto のディレクトリのどこにも無いとき。Google の well-known types、`buf/validate`、dandori の `options.proto` は、ファイルが無くても sakai が中身を知っています。

**直し方**: 地図に `proto root` を書くか、import を直してください。範囲の外の契約なら、そのままでかまいません（その型は参照の検査から外れます）。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "elsewhere/v1/x.proto";
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

<a id="w103"></a>

## W103 — どの地図にも読まれない context のファイルがあります

**いつ出るか**: `check` にディレクトリを渡したとき、その下の context のファイルを、どの地図も `use context` で読んでいないとき。

**直し方**: 地図の `use context` に足すか、ファイルを消してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`丙.ctx`:

```ctx
context 丙(c) v1
owns
  dir "a"
```

<a id="n101"></a>

## N101 — dandori の参照を確かめていません

**いつ出るか**: いまは出ません。dandori が参照を JSON で出さなかったころに、地図が `.flow` を含むとき、属し方だけを確かめたことを伝えるためのコードでした。

**直し方**: 直すものはありません。

**再現**: ritsu 0.23.0 で使われなくなりました。いまの sakai は、dandori の参照を ritsu の中で（`References` から）読み、境界を越えるものを確かめます（E202、E207〜E209）。

関連: [E202](#e202), [E207](#e207), [E208](#e208), [E209](#e209)

<a id="e201"></a>

## E201 — 関係の無いコンテキストへの参照です

**いつ出るか**: 境界を越える参照の相手と、上流と下流（下流が書く）、パートナーシップ、共有カーネルのどの関係も無いとき。関係が逆向き（参照の先が下流）のときにも出ます。

**直し方**: 参照する側に `upstream <相手> <役割>` と `through <package>` を書くか、参照を消してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "b/v1/b.proto";
message A { b.v1.B b = 1; }
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E202](#e202), [E206](#e206)

<a id="e202"></a>

## E202 — 相手の内側への参照です

**いつ出るか**: 参照の先が、相手の公表された言語にも、二つの共有カーネルにも入っていないとき（公表された言語でない proto、規則やカレンダーそのもの）。

**直し方**: 相手の公表された言語を通して参照するか、二つの共有カーネルに並べてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 conformist
  through b.v1
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "b/internal/v1/x.proto";
message A { b.internal.v1.X x = 1; }
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`b/internal/v1/x.proto`:

```proto
syntax = "proto3";
package b.internal.v1;
message X {}
```

関連: [E201](#e201)

<a id="e203"></a>

## E203 — `through` に無い package を通る参照です

**いつ出るか**: 上流の公表された言語のうち、関係の `through` に並べていない package を参照しているとき。

**直し方**: `through` に package を足すか、参照を直してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 conformist
  through b.v1
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

published language b.v2
  proto "b/v2/b2.proto"

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "b/v2/b2.proto";
message A { b.v2.B2 b = 1; }
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`b/v2/b2.proto`:

```proto
syntax = "proto3";
package b.v2;
message B2 {}
```

関連: [E312](#e312)

<a id="e204"></a>

## E204 — 腐敗防止層の外からの、上流の公表された言語への参照です

**いつ出るか**: 腐敗防止層の関係に `layer` があるのに、層の外の成果物が上流の公表された言語を参照しているとき。

**直し方**: 参照を層の中に移すか、`layer` に足してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 anticorruption layer
  through b.v1
  layer dir "a/acl"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "b/v1/b.proto";
message A { b.v1.Plain p = 1; }
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/acl/v1/acl.proto`:

```proto
syntax = "proto3";
package a.acl.v1;
message Seen {}
```

関連: [E205](#e205)

<a id="e205"></a>

## E205 — 腐敗防止層の下流の公表された言語に、上流の型が出ています

**いつ出るか**: 腐敗防止層の下流が、自分の公表された言語の proto で、上流の公表された言語を import しているとき。上流のモデルが、層を通らずに下流の外へ出ていきます。

**直し方**: 層の中で自分の型に読み替え、公表された言語には自分の型だけを出してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

published language a.v1
  proto "a/v1/a1.proto"

upstream 乙 anticorruption layer
  through b.v1
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/v1/a1.proto`:

```proto
syntax = "proto3";
package a.v1;
import "b/v1/b.proto";
message A1 { b.v1.Plain p = 1; }
```

関連: [E204](#e204)

<a id="e206"></a>

## E206 — 別々の道の相手への参照です

**いつ出るか**: `separate ways from` を書いた二つのあいだに、境界を越える参照があるとき。

**直し方**: 参照を消すか、別々の道をやめて関係を書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

separate ways from 乙
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "b/v1/b.proto";
message A { b.v1.Plain p = 1; }
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E310](#e310)

<a id="e207"></a>

## E207 — ワークフローが、相手の公開ホストサービスでないサービスを呼んでいます

**いつ出るか**: ワークフローが境界の向こうのサービスを `connect` で呼ぶか、規則を `use rule … connect` で呼ぶのに、そのサービスが、相手の公表された言語の `open host service` に無いとき。

**直し方**: 相手にそのサービスを `open host service` に並べてもらうか、相手が並べたサービスを呼んでください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 conformist
  through b.v1
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/w.flow`:

```flow
workflow w v1

use proto b from "../b/v1/b.proto"
  url "https://b.example.com"

inputs
  kind : b.Kind

task get(kind: b.Kind) -> b.B
  connect b "BService/Get"

flow
  let r = get(kind: kind)
  succeed
```

関連: [E202](#e202), [E301](#e301)

<a id="e208"></a>

## E208 — ワークフローが実装するサービスが、自分の公表された言語にありません

**いつ出るか**: ワークフローが `implements` で実装するサービスが、そのワークフローを持つコンテキストの公表された言語の `open host service` に無いとき（proto が公表された言語に無いときも）。

**直し方**: 自分の公表された言語に proto を並べ、`open host service` にサービスを書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

published language a.v1
  proto "a/v1/a.proto"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/v1/a.proto`:

```proto
syntax = "proto3";
package a.v1;
import "dandori/v1/options.proto";
service AService {
  option (dandori.v1.workflow) = {name: "w", version: 1};
  rpc Start(StartRequest) returns (StartResponse) {
    option (dandori.v1.start) = {};
  }
}
message StartRequest { string id = 1; }
message StartResponse { string id = 1; }
```

`a/w.flow`:

```flow
workflow w v1 implements a1.AService

use proto a1 from "v1/a.proto"

inputs
  id : string

outputs
  id : string

flow
  succeed id = id
```

関連: [E301](#e301)

<a id="e209"></a>

## E209 — ワークフローが、境界の向こうのワークフローを子として走らせています

**いつ出るか**: ワークフローが `flow` で走らせる子のフローが別のコンテキストのもので、二つがパートナーシップでなく、子のフローが共有カーネルになく、子のフローが相手の公開ホストサービスを実装していないとき。

**直し方**: 相手が公表したサービスを `connect` で呼ぶか、二つをパートナーシップにするか、子のフローを共有カーネルに並べるか、子のフローに相手の公開ホストサービスを実装させてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 conformist
  through b.v1
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`b/c.flow`:

```flow
workflow c v1

inputs
  id : string

outputs
  id : string

flow
  succeed id = id
```

`a/p.flow`:

```flow
workflow p v1

record 答え
  id : string

inputs
  id : string

task run_child(id: string) -> 答え
  flow "../b/c.flow"

flow
  let r = run_child(id: id)
  succeed
```

関連: [E202](#e202), [E207](#e207)

<a id="e301"></a>

## E301 — 公開ホストサービスが、公表された言語に無いサービスです

**いつ出るか**: `open host service` に並べたサービスが、その公表された言語の proto に無いとき。Rust のクレートの公表された言語に `open host service` を書いたときにも出ます（クレートはサービスを持ちません）。

**直し方**: サービスの名前を直すか、proto にサービスを足してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService, NoService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E302](#e302)

<a id="e302"></a>

## E302 — 公表された言語のファイルが、そのコンテキストのものでないか、package が違います

**いつ出るか**: 公表された言語の proto、規則、Rust のクレート、生成したコードの置き場所が、そのコンテキストに属さないとき。proto の package やクレートの名前（`-` を `_` にしたもの）が見出しと違うとき、`crate` の先が地図の `code rust` のワークスペースのクレートでないとき、地図に `code rust` が無いのにクレートを公表したときにも出ます。

**直し方**: 見出しの package を直すか、そのコンテキストのファイルを並べてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v2
  proto "b/v1/b.proto"
  open host service BService
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E301](#e301)

<a id="e303"></a>

## E303 — 顧客／供給者が片側だけです

**いつ出るか**: 顧客の側の `upstream <供給者> customer` と、供給者の側の `downstream <顧客> supplier` の、どちらかが無いとき。

**直し方**: もう片方のファイルにも書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 customer
  through b.v1
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E306](#e306)

<a id="e304"></a>

## E304 — 順応者に対応か `layer` があります

**いつ出るか**: `conformist` の関係に、`layer`、`enum` の対応、`term` の対応のどれかがあるとき。順応者は上流のモデルをそのまま使います。

**直し方**: 読み替えるなら役割を anticorruption layer にし、そうでなければ対応と層を消してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 conformist
  through b.v1
  layer dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E305](#e305)

<a id="e305"></a>

## E305 — 腐敗防止層でないのに、対応か `layer` があります

**いつ出るか**: 役割に anticorruption layer の無い関係（customer だけなど）に、対応か `layer` があるとき。

**直し方**: 役割に `, anticorruption layer` を足すか、対応と層を消してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 customer
  through b.v1
  layer dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind

downstream 甲 supplier
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E304](#e304)

<a id="e306"></a>

## E306 — 一緒に書けない役割です

**いつ出るか**: conformist と customer、conformist と anticorruption layer を一緒に書いたとき。

**直し方**: どちらか一つにしてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 conformist, customer
  through b.v1
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind

downstream 甲 supplier
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E303](#e303)

<a id="e307"></a>

## E307 — 共有カーネルが片側だけか、両側の並びが違います

**いつ出るか**: 共有カーネルを片側にしか書いていないとき、両側の並びが違うとき（写しを両側に置くときを除く）、並べたものが二つのどちらのものでもないとき。

**直し方**: 両方のファイルに `shared kernel with <相手>` を書き、同じものを並べてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

shared kernel with 乙
  proto "a/a.proto"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E308](#e308), [E202](#e202)

<a id="e308"></a>

## E308 — 共有カーネルの写しの中身が違います

**いつ出るか**: 両側がそれぞれの写しを共有カーネルに並べているのに、写しのバイト列が違うとき。

**直し方**: 両側の写しの中身をそろえてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

shared kernel with 乙
  proto "a/k/units.proto"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind

shared kernel with 甲
  proto "b/k/units.proto"
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/k/units.proto`:

```proto
syntax = "proto3";
package k;
message Yen { int64 amount = 1; }
```

`b/k/units.proto`:

```proto
syntax = "proto3";
package k;
message Yen { int32 amount = 1; }
```

関連: [E307](#e307)

<a id="e309"></a>

## E309 — パートナーシップが片側だけです

**いつ出るか**: `partnership with` を片側にしか書いていないとき。

**直し方**: もう片方のファイルにも書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

partnership with 乙
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E201](#e201)

<a id="e310"></a>

## E310 — 別々の道とほかの関係が両立しません

**いつ出るか**: `separate ways from` を書いた二つのあいだに、ほかの関係もあるとき。

**直し方**: どちらかを消してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 conformist
  through b.v1

separate ways from 乙
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E206](#e206)

<a id="e311"></a>

## E311 — 自分自身との関係です

**いつ出るか**: 関係の相手が、そのコンテキスト自身のとき。

**直し方**: 相手の名前を直してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

partnership with 甲
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

<a id="e312"></a>

## E312 — 上流が `through` の package を公表していません

**いつ出るか**: `through` に並べた package が、上流の `published language` に無いとき。

**直し方**: 上流が公表している package を書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 conformist
  through b.v9
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E203](#e203)

<a id="e313"></a>

## E313 — 腐敗防止層の `layer` が下流のものではありません

**いつ出るか**: `layer` に並べたディレクトリかファイルが、その関係を書いた下流のコンテキストに属さないとき。腐敗防止層は、下流の側に置くものです。

**直し方**: 下流に属するディレクトリを書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 anticorruption layer
  through b.v1
  layer dir "b"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

<a id="w301"></a>

## W301 — 上流をたどると、元のコンテキストに戻ります

**いつ出るか**: 二つ以上のコンテキストが、たどると互いに上流になっているとき。

**直し方**: 向きを一つにそろえるか、パートナーシップにすることを考えてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

published language a.v1
  proto "a/v1/a1.proto"

upstream 乙 conformist
  through b.v1
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind

upstream 甲 conformist
  through a.v1
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/v1/a1.proto`:

```proto
syntax = "proto3";
package a.v1;
enum AKind {
  A_KIND_UNSPECIFIED = 0;
  A_KIND_X = 1;
}
message A1 {}
```

<a id="e401"></a>

## E401 — 対応に、上流の列挙の値が抜けています

**いつ出るか**: 腐敗防止層の `enum` の対応に、上流の列挙の値で下流の値も refuse も書いていないものがあるとき。上流が値を足すと、これが出ます。値が無いことを表す 0 番の値には、対応は要りません。

**直し方**: 値ごとに `<上流の値> -> <下流の値>` か `<上流の値> -> refuse "<理由>"` を書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 anticorruption layer
  through b.v1
  enum Kind -> 甲の種類
    KIND_ONE -> 一つめ
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E402](#e402), [W402](#w402)

<a id="e402"></a>

## E402 — 対応に、上流の列挙に無い値があります

**いつ出るか**: `enum` の対応の左辺に、上流の列挙の値でない名前を書いたとき。

**直し方**: 値の名前を直すか、その行を消してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 anticorruption layer
  through b.v1
  enum Kind -> 甲の種類
    KIND_ONE -> 一つめ
    KIND_TWO -> 二つめ
    KIND_THREE -> 三つめ
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E401](#e401)

<a id="e403"></a>

## E403 — 対応の先の値が、下流の列挙に無い値です

**いつ出るか**: 対応の先が proto の列挙なのに、右辺がその列挙の値でないとき。

**直し方**: 右辺を、先の列挙の値にしてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

published language a.v1
  proto "a/v1/a1.proto"

upstream 乙 anticorruption layer
  through b.v1
  enum Kind -> enum AKind
    KIND_ONE -> A_KIND_X
    KIND_TWO -> A_KIND_Y
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/v1/a1.proto`:

```proto
syntax = "proto3";
package a.v1;
enum AKind {
  A_KIND_UNSPECIFIED = 0;
  A_KIND_X = 1;
}
message A1 {}
```

関連: [E401](#e401)

<a id="e404"></a>

## E404 — 参照している上流の列挙に、腐敗防止層の対応がありません

**いつ出るか**: 腐敗防止層の下流の成果物が上流の列挙を参照している（使うメッセージからたどれるものも含む）のに、その列挙の `enum` の対応が無いとき。

**直し方**: `enum <上流の列挙> -> <先>` と値の行を書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 anticorruption layer
  through b.v1
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "b/v1/b.proto";
message A { b.v1.B b = 1; }
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E401](#e401)

<a id="e405"></a>

## E405 — rulec の規則の取り込みと、`.ctx` に書いた対応が食い違います

**いつ出るか**: 対応の先が、上流の列挙を `import proto` で取り込む規則の列挙なのに、`.ctx` の値の行が、規則の取り込み（rulec が渡す、値ごとの proto での名前）と違うとき。

**直し方**: 値の行を消して規則の取り込みに任せるか、規則と同じにしてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 anticorruption layer
  through b.v1
  layer rulec "a/x.rule"
  enum Kind -> rulec "a/x.rule" enum 種類
    KIND_ONE -> 二
    KIND_TWO -> 二
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/x.rule`:

```rule
rule x(x) v1

import proto "../b/v1/b.proto" Kind -> 種類
enum 種類(kind) = 一(one) | 二(two)

inputs
  k : 種類

outputs
  n : 種類

table t
policy unique
| k  | -> n |
| 一 | 一   |
| 二 | 二   |
```

関連: [E401](#e401)

<a id="e406"></a>

## E406 — 同じ名前で違う意味の語が、対応の無いまま境界を越えます

**いつ出るか**: 上流の語が、その `means` の要素とともに下流に越えてくるのに、下流に同じ名前（`also` を含む）で `as` で取り入れていない語があり、腐敗防止層の対応がそれを読み替えていないとき。

**直し方**: 下流の語の名前を変えるか、同じ意味なら `as` で取り入れるか、腐敗防止層にして読み替えてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

terms
  種類 "甲が客に出す区分"

upstream 乙 conformist
  through b.v1
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "b/v1/b.proto";
message A { b.v1.B b = 1; }
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E407](#e407)

<a id="e407"></a>

## E407 — 違う意味の語を、同じ名前に読み替えています

**いつ出るか**: 腐敗防止層で上流の語を読み替えた先の値や語の名前が、下流にある、意味の違う語の名前と同じとき。

**直し方**: 読み替えた先に、違う名前を付けてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

terms
  種類 "甲が客に出す区分"

upstream 乙 anticorruption layer
  through b.v1
  enum Kind -> 種類
    KIND_ONE -> 一つめ
    KIND_TWO -> 二つめ
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "b/v1/b.proto";
message A { b.v1.B b = 1; }
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E406](#e406)

<a id="e408"></a>

## E408 — `means` の要素が、そのコンテキストの公表された言語にありません

**いつ出るか**: 語の `means` が、自分の公表された言語のファイル以外を指すとき。

**直し方**: 自分の公表された言語の要素を指すか、`means` を消してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

terms
  品 "甲が売るもの"
    means proto "b/v1/b.proto" message Plain
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E007](#e007)

<a id="e409"></a>

## E409 — `term` の対応の語が、用語集にありません

**いつ出るか**: `term <上流の語> -> <下流の語>` の左が上流の用語集に、右が下流の用語集に無いとき。

**直し方**: 語の名前を直すか、用語集に語を足してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 anticorruption layer
  through b.v1
  term 無い語 -> 何か
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E410](#e410)

<a id="e410"></a>

## E410 — `as` で取り入れる語が使えません

**いつ出るか**: `as` の語が相手の用語集に無いとき、相手と関係が無いとき。

**直し方**: 語の名前を直すか、相手との関係を書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

terms
  種類 as 乙.種類
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E007](#e007)

<a id="w401"></a>

## W401 — 境界を越えない語です

**いつ出るか**: 語が、`means` で自分の公表された言語の要素を指さず、`as` で取り入れたものでもなく、腐敗防止層の対応の先でもなく、上流から越えてくる同じ名前の語ともぶつからないとき。用語集に載せるのは、境界を越える語だけです。

**直し方**: 境界を越える語にするか、用語集から消してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

terms
  孤立 "どこにも出ていかない語"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

<a id="w402"></a>

## W402 — 値が無いことを表す 0 番の値に対応を書いています

**いつ出るか**: 0 番の値で、名前から列挙の接頭辞を外すと unspecified になるものに、対応を書いたとき。その値は、値が設定されていないことを表す値なので、対応は要りません。

**直し方**: その行を消してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai check .` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"

upstream 乙 anticorruption layer
  through b.v1
  enum Kind -> 甲の種類
    KIND_UNSPECIFIED -> 無し
    KIND_ONE -> 一つめ
    KIND_TWO -> 二つめ
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E401](#e401)

<a id="e501"></a>

## E501 — ツールの設定に書けません

**いつ出るか**: `sakai build` で、地図にその言語の `code` の行が無いとき、その言語のコードを持つコンテキストが無いとき、ディレクトリの名前がその言語のモジュールの名前にならないとき（Python と Java）、コードのファイルを一つだけ名指した項があるとき、Python の置き場所の直下にモジュールがあるとき（import-linter は読めない）、Java にデフォルトパッケージのクラスがあるとき、ArchUnit に `test` の置き場所が無いとき、Go の置き場所に go.mod が無いとき。

**直し方**: 注のとおりに、地図の `code` と `owns` か、コードの置き場所を直してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai build 地図.ctx --target import-linter` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

関連: [E502](#e502)

<a id="e502"></a>

## E502 — 書いてある設定が、いまの地図から書くものと違います

**いつ出るか**: `build --check` で、ディスクの設定のファイルが、いまの地図から書くものと一字でも違うとき（無いときも）。注には、最初に違う行と、地図から書くとその行がどうなるかが出ます。

**直し方**: `sakai build` で書き直してください。書いたときと違う `--lang` で確かめると、説明の文が違うので、同じ `--lang` を渡してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `sakai build 地図.ctx --target import-linter --check` を走らせます。

`地図.ctx`:

```ctx
map 地図(m) v1
use context "甲.ctx"
use context "乙.ctx"
covers "."
code python "py"
```

`甲.ctx`:

```ctx
context 甲(a) v1
owns
  dir "a", "py"
```

`乙.ctx`:

```ctx
context 乙(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  種類 "乙が扱うものの種類"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`py/k/x.py`:

```
X = 1
```

`py/.importlinter`:

```
# written by hand
```

関連: [E501](#e501)
