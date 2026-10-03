<!-- `chobo explain --all --format markdown --lang ja` の出力です。手で編集しないでください。 -->

# chobo の診断

chobo が出すコードの全部と、いつ出るか、どう直すか。一件だけ読むには `chobo explain E020`。

| コード | 種別 | 見出し |
|---|---|---|
| [E001](#e001) | エラー | 読めない行があります |
| [E002](#e002) | エラー | 名前が見つかりません |
| [E003](#e003) | エラー | 同じものが二度書かれています |
| [E004](#e004) | エラー | 勘定に渡す引数の数が合いません |
| [E005](#e005) | エラー | 型が合いません |
| [E010](#e010) | エラー | 移動の単位が合いません |
| [E011](#e011) | エラー | 数が単位に収まりません |
| [E012](#e012) | エラー | 移動の元と先がいつも同じ勘定です |
| [E013](#e013) | エラー | 振替に移動がありません |
| [E020](#e020) | エラー | 中の勘定に境界がありません |
| [E021](#e021) | エラー | 外の勘定に境界があります |
| [E022](#e022) | エラー | 境界に収まる残高がありません |
| [E023](#e023) | エラー | 境界に断る理由の名前がありません |
| [E030](#e030) | エラー | 振替にキーがありません |
| [E031](#e031) | エラー | キーに額の引数があります |
| [E040](#e040) | エラー | 仮押さえの終わり方がありません |
| [E041](#e041) | エラー | 有効期限が範囲の外です |
| [E050](#e050) | エラー | 前のリビジョンからある勘定の定義が変わりました |
| [E051](#e051) | エラー | 前のリビジョンからある振替の定義が変わりました |
| [E060](#e060) | エラー | 一つの操作が TigerBeetle の一つのリクエストに入りません |
| [E061](#e061) | エラー | 名前が PostgreSQL の識別子に収まりません |
| [W101](#w101) | 警告 | 残高がたまる一方の勘定 |
| [W102](#w102) | 警告 | いつも断られる振替 |
| [W103](#w103) | 警告 | 移動の順序のせいで断られる振替 |
| [W104](#w104) | 警告 | 同じ仮押さえのほかの移動に頼る移動 |
| [W105](#w105) | 警告 | 使われない宣言 |
| [W106](#w106) | 警告 | 効かない境界 |
| [W107](#w107) | 警告 | 前のリビジョンにあった勘定や振替がありません |

## E001

`エラー` — **読めない行があります**

**いつ出るか。** 知らない語がある、字下げがそろっていない、文字列が閉じていない、行が決まった形になっていないときに出ます。ファイルの先頭の BOM、タブ、結合文字（濁点やアクセントを別の文字として付けたもの）もここで止めます。読めない行があると、そこから先は推測になるので、名前の解決やほかの検査はしません。

**直し方。** メッセージが示す形に書き直します。勘定の下に書けるのは `description`、`at least`、`at most`、振替の下に書けるのは `description`、`key`、`pending`、`move` です。結合文字は、合成済みの一文字（「か」と濁点ではなく「が」）で書き直します。

**最小の再現**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
  keep 3
```

関係するコード: [E002](#e002)

## E002

`エラー` — **名前が見つかりません**

**いつ出るか。** 単位、勘定、型、振替の引数の名前が、宣言されたどれとも合わないときに出ます。名前は書いたとおりに比べます。

**直し方。** 宣言を足すか、宣言にある名前に直します。外の世界を表す勘定（仕入先、客）も `account 仕入先 : 個 outside` のように宣言します。

**最小の再現**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
```

関係するコード: [E001](#e001), [E004](#e004)

## E003

`エラー` — **同じものが二度書かれています**

**いつ出るか。** 同じ名前の単位、勘定、振替、引数を二度宣言したときに出ます。一つの勘定や振替の下に `description`、`at least`、`at most`、`key`、`pending` を二度書いたとき、キーに同じ引数を二度並べたときも出ます。

**直し方。** どちらかを消すか、別の名前にします。キーは一行にまとめて書きます。

**最小の再現**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
```

## E004

`エラー` — **勘定に渡す引数の数が合いません**

**いつ出るか。** 移動に書いた勘定の引数の数が、勘定の宣言と違うときに出ます。引数は宣言の順に、位置で渡します。

**直し方。** 宣言の数だけ渡します。`account 在庫(sku: string)` なら `在庫(sku)` です。引数の無い勘定は、括弧を書かずに `客` と書きます。

**最小の再現**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock
```

関係するコード: [E002](#e002), [E005](#e005)

## E005

`エラー` — **型が合いません**

**いつ出るか。** string の引数を移動の額に使ったとき、額の引数を勘定の引数に使ったとき、勘定の引数を string 以外の型で宣言したときに出ます。

**直し方。** 額には、単位を型にした引数（`数: 個`）か数を書きます。勘定の引数には、string の引数か文字列を書きます。

**最小の再現**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move sku from supplier to stock(sku)
```

関係するコード: [E010](#e010)

## E010

`エラー` — **移動の単位が合いません**

**いつ出るか。** 一つの移動の、元の勘定、先の勘定、額の単位がそろっていないときに出ます。chobo は換算しません。

**直し方。** 単位をそろえます。両替や換算は、単位ごとに外の勘定を置いて、単位の違う二つの移動で書きます。換算した額は呼ぶ側が計算して渡します。

**最小の再現**:

```book
book shop v1
unit pcs
unit yen
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account customers : yen outside
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

関係するコード: [E005](#e005)

## E011

`エラー` — **数が単位に収まりません**

**いつ出るか。** 帳簿に書いた数（境界や移動の額）の小数が単位の `scale` より多いとき、移動の額が負のとき、いちばん小さい単位で数えて −(2⁶³ − 1) から 2⁶³ − 1 までの外にあるときに出ます。

**直し方。** 単位の `scale` に合う桁数で書きます。小数の要る単位は `unit USD scale 2` のように宣言します。向きを変えたい移動は、`from` と `to` を入れ替えます。

**最小の再現**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0.5 refused as out_of_stock
```

関係するコード: [E022](#e022)

## E012

`エラー` — **移動の元と先がいつも同じ勘定です**

**いつ出るか。** 一つの移動の `from` と `to` に、同じ勘定と同じ引数を書いたときに出ます。どう呼んでも same_account で断られます。

**直し方。** 動かす先を別の勘定にします。同じ種類の勘定のあいだで動かすなら、別の引数にします（`在庫(元) to 在庫(先)`）。

**最小の再現**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
transfer shuffle(note: string, sku: string, qty: pcs)
  key note
  move qty from stock(sku) to stock(sku)
```

## E013

`エラー` — **振替に移動がありません**

**いつ出るか。** 振替の下に `move` の行が一つも無いときに出ます。

**直し方。** `move <額> from <勘定> to <勘定>` で、どこからどこへ動かすかを書きます。

**最小の再現**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
transfer count(note: string)
  key note
```

## E020

`エラー` — **中の勘定に境界がありません**

**いつ出るか。** `outside` を付けていない勘定に、`at least` も `at most` も無いときに出ます。chobo が守るのは勘定の境界だけなので、境界の無い中の勘定では何も守られません。

**直し方。** `at least 0 refused as 在庫切れ` のように境界を書きます。外の世界を表す勘定（仕入先、客、銀行）なら `outside` を付けます。

**最小の再現**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
```

関係するコード: [E021](#e021)

## E021

`エラー` — **外の勘定に境界があります**

**いつ出るか。** `outside` を付けた勘定に `at least` か `at most` を書いたときに出ます。外の勘定は外の世界を表し、マイナスにもなります。

**直し方。** 境界を消します。その勘定の残高を守りたいなら、`outside` を外して中の勘定にします。

**最小の再現**:

```book
book shop v1
unit pcs
account supplier : pcs outside
  at least 0 refused as supplier_short
```

関係するコード: [E020](#e020)

## E022

`エラー` — **境界に収まる残高がありません**

**いつ出るか。** 下限が上限より大きいとき、上限が 0 より小さいときに出ます。勘定は残高 0 から始まるので、上限が 0 より小さいと何も入れられません。

**直し方。** 下限を上限以下にし、上限を 0 以上にします。

**最小の再現**:

```book
book shop v1
unit pcs
account shelf(sku: string) : pcs
  at least 10 refused as below_safety_stock
  at most 5 refused as shelf_full
```

関係するコード: [E011](#e011)

## E023

`エラー` — **境界に断る理由の名前がありません**

**いつ出るか。** 境界に `refused as <理由>` が無いとき、または理由の名前が chobo の決めている名前（key_conflict、already_refused、same_account、no_such_hold、already_posted、already_voided、expired、over_hold）と同じときに出ます。理由の名前は、断ったときに呼ぶ側が受け取るもので、dandori のタスクのエラーの名前にもなります。

**直し方。** 業務の言葉で理由を付けます（`at least 0 refused as 在庫切れ`）。

**最小の再現**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0
```

## E030

`エラー` — **振替にキーがありません**

**いつ出るか。** 振替に `key` の行が無いとき、またはキーに書いた名前が振替の引数に無いときに出ます。どの振替にも冪等のキーが要ります。同じキーで同じ中身の二度目は何もせず、中身が違えば断ります。

**直し方。** 一度だけにしたい単位を、引数で並べます。注文と SKU ごとに一度なら `key 注文, sku` です。

**最小の再現**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  move qty from supplier to stock(sku)
```

関係するコード: [E031](#e031)

## E031

`エラー` — **キーに額の引数があります**

**いつ出るか。** キーに、単位を型にした引数（額）を入れたときに出ます。入れると、額だけが違うリトライが別の振替として両方通り、二重に動きます。

**直し方。** キーから額を外します。額だけが違う二度目は、key_conflict で断られるようになります。

**最小の再現**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, qty
  move qty from supplier to stock(sku)
```

関係するコード: [E030](#e030)

## E040

`エラー` — **仮押さえの終わり方がありません**

**いつ出るか。** `pending` だけを書いて、有効期限も `never expires` も書いていないときに出ます。仮押さえは、確定、取消、有効期限切れのどれかで終わります。

**直し方。** `pending expires after 30 minutes` か `pending never expires` と書きます。`never expires` の仮押さえを終わらせるのは、呼ぶ側だけです。

**最小の再現**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account customers : pcs outside
transfer reserve(order: string, sku: string, qty: pcs)
  key order, sku
  pending
  move qty from stock(sku) to customers
```

関係するコード: [E041](#e041)

## E041

`エラー` — **有効期限が範囲の外です**

**いつ出るか。** 有効期限が 1 秒より短いとき、4294967295 秒（2³² − 1 秒、約 136 年）より長いとき、整数でないときに出ます。TigerBeetle の timeout は 32 ビットの秒です。

**直し方。** 1 秒から 2³² − 1 秒までの整数で書きます。それより長く押さえるなら `never expires` にします。

**最小の再現**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account customers : pcs outside
transfer reserve(order: string, sku: string, qty: pcs)
  key order, sku
  pending expires after 0 minutes
  move qty from stock(sku) to customers
```

関係するコード: [E040](#e040)

## E050

`エラー` — **前のリビジョンからある勘定の定義が変わりました**

**いつ出るか。** `--diff-base` で比べたリビジョンにある勘定の、単位、`scale`、外の勘定かどうか、引数、境界が変わったときに出ます。勘定の同一性に帳簿のバージョンは入らないので、すでにある勘定は前の定義のまま残ります。

**直し方。** 新しい名前の勘定を宣言し、残高を移す振替を書きます。変わりうる限度は、はじめから勘定にしておきます（会員ごとの与信枠なら「使える枠」の勘定）。

**比べるリビジョンの帳簿**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

**いまの帳簿（最小の再現）**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 3 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

関係するコード: [E051](#e051), [W107](#w107)

## E051

`エラー` — **前のリビジョンからある振替の定義が変わりました**

**いつ出るか。** `--diff-base` で比べたリビジョンにある振替の、引数、キー、仮押さえの終わり方、移動が変わったときに出ます。呼んでいる途中の操作をリトライすると key_conflict で断られ、押さえ中の仮押さえを確定できなくなります。

**直し方。** 新しい名前の振替を宣言し、前の振替は残しておきます。前の振替の仮押さえがどれも終わってから、前の振替を消します。

**比べるリビジョンの帳簿**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

**いまの帳簿（最小の再現）**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship(order: string, sku: string, qty: pcs)
  key order
  move qty from stock(sku) to customers
```

関係するコード: [E050](#e050), [W107](#w107)

## E060

`エラー` — **一つの操作が TigerBeetle の一つのリクエストに入りません**

**いつ出るか。** `chobo build` の TigerBeetle のターゲットで、ある振替の一つの操作が送るチェーンが 253 件の振替を超えるときに出ます。チェーンは一つのリクエストで送らないと全部か無しになりませんが、`--development` で立てたレプリカは一つのリクエストに 253 件までしか受け取りません（そうでないレプリカは 8189 件まで）。境界のある勘定のあいだの移動は、一つで最大 6 件の振替になります。

**直し方。** 振替の種類を、移動の少ないいくつかの種類に分けます。分けた振替はそれぞれが一度の書き込みになるので、全部か無しにしたい移動は同じ種類に残します。

**どのコマンドで出るか**: `chobo build <file.book> --target tigerbeetle-typescript`

**最小の再現**:

```book
book split v1
unit yen
account pool(id: string) : yen
  at least 1 refused as pool_short
  at most 1000000 refused as pool_full
account credit(id: string) : yen
  at least -1000 refused as over_limit
  at most 1000 refused as overpaid
account bank : yen outside
transfer fund(note: string, id: string, amount: yen)
  key note
  move amount from bank to pool(id)
transfer repay(note: string, id: string, amount: yen)
  key note
  move amount from credit(id) to bank
transfer spread(note: string, a: string, b: string, amount: yen)
  key note
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
```

関係するコード: [E061](#e061)

## E061

`エラー` — **名前が PostgreSQL の識別子に収まりません**

**いつ出るか。** `chobo build` の PostgreSQL のターゲットで、スキーマ（帳簿の名前）、関数（`<振替>_<操作>`、`balance_<勘定>`）、関数の引数（`p_<引数>`）の名前が 63 バイトを超えるときに出ます。PostgreSQL はそれより長い名前を黙って切り詰めるので、二つの関数が同じ名前になりえます。日本語は一字 3 バイトです。

**直し方。** 名前を短くします。ASCII なら 63 字、日本語なら 21 字までです（関数の名前は、後ろに付く `_hold` などの分を引きます）。

**どのコマンドで出るか**: `chobo build <file.book> --target postgres`

**最小の再現**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship_the_goods_to_the_customers_who_have_ordered_and_paid_for_them(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

関係するコード: [E060](#e060)

## W101

`警告` — **残高がたまる一方の勘定**

**いつ出るか。** 中の勘定へ入れる移動はあるのに、そこから取る移動がどこにも無いときに出ます。入ったものは出ていけません。

**直し方。** 取り出す振替を書きます。外の世界を表す勘定なら `outside` にします。

**最小の再現**:

```book
book shop v1
unit yen
account wallet(member: string) : yen
  at least 0 refused as short
account bank : yen outside
transfer top_up(slip: string, member: string, amount: yen)
  key slip
  move amount from bank to wallet(member)
```

関係するコード: [W102](#w102), [W106](#w106)

## W102

`警告` — **いつも断られる振替**

**いつ出るか。** 振替が取る元の勘定に、入れる移動がどこにも無く、その勘定の下限が 0 以上のときに出ます。0 より多く動かせば、いつも断られます。

**直し方。** その勘定へ入れる振替（入荷、入金）を書きます。

**最小の再現**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account customers : pcs outside
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

関係するコード: [W101](#w101)

## W103

`警告` — **移動の順序のせいで断られる振替**

**いつ出るか。** すぐに確定する振替で、前の移動が勘定から取り、後の移動が同じ勘定へ入れるとき（下限）、または前の移動が入れ、後の移動が取るとき（上限）に出ます。移動は書いた順に一つずつ確かめるので、合わせれば収まる場合でも、前の移動で断られます。

**直し方。** 頼られる側の移動を先に書きます。下限なら入れる移動を、上限なら取る移動を先にします。

**最小の再現**:

```book
book market v1
unit yen
account buyers : yen outside
account fees : yen outside
account sales(shop: string) : yen
  at least 0 refused as sales_short
transfer sell(order: string, shop: string, price: yen, fee: yen)
  key order
  move fee from sales(shop) to fees
  move price from buyers to sales(shop)
```

関係するコード: [W104](#w104)

## W104

`警告` — **同じ仮押さえのほかの移動に頼る移動**

**いつ出るか。** 仮押さえの振替で、ある移動が、同じ仮押さえのほかの移動が入れる勘定から取るとき（または、ほかの移動が取る勘定へ入れるとき）に出ます。押さえた額は、入ってくる側では使えず、出ていく側では空きになりません。順序を変えても通りません。

**直し方。** 取る分をはじめからその勘定に入れておくか、確定のあとで動かす別の振替に分けます。

**最小の再現**:

```book
book market v1
unit yen
account buyers : yen outside
account payouts : yen outside
account escrow(order: string) : yen
  at least 0 refused as escrow_short
transfer pay(order: string, price: yen)
  key order
  pending expires after 1 hour
  move price from buyers to escrow(order)
  move price from escrow(order) to payouts
```

関係するコード: [W103](#w103)

## W105

`警告` — **使われない宣言**

**いつ出るか。** どの勘定にも引数にも使われない単位、どの移動にも出てこない勘定、キーにも移動にも使われない引数があるときに出ます。

**直し方。** 消すか、使うところを書きます。

**最小の再現**:

```book
book shop v1
unit pcs
unit boxes
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

関係するコード: [W106](#w106)

## W106

`警告` — **効かない境界**

**いつ出るか。** 上限のある勘定へ入れる移動がどこにも無いとき（下限のある勘定から取る移動が無いときも）に出ます。その境界で断ることがありません。

**直し方。** 境界を消すか、その境界で確かめるはずだった振替を書きます。

**最小の再現**:

```book
book shop v1
unit yen
account credit(member: string) : yen
  at least -1000 refused as over_limit
  at most 0 refused as overpaid
account shops : yen outside
transfer spend(slip: string, member: string, amount: yen)
  key slip
  move amount from credit(member) to shops
```

関係するコード: [W101](#w101), [W105](#w105)

## W107

`警告` — **前のリビジョンにあった勘定や振替がありません**

**いつ出るか。** `--diff-base` で比べたリビジョンにあった勘定や振替が、いまの帳簿に無いとき、または帳簿の名前が変わったときに出ます。データベースには、その残高と仮押さえが残ります。

**直し方。** 残高を移す振替を書いてから消すか、残しておきます。

**比べるリビジョンの帳簿**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

**いまの帳簿（最小の再現）**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer send(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

関係するコード: [E050](#e050), [E051](#e051)
