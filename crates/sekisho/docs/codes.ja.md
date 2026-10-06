# 診断のコード

`sekisho explain --all --format markdown --lang ja` の出力です。手で直しません。

<a id="e001"></a>

## E001 — 読めない字句があります

**いつ出るか**: 知らない文字、閉じていない文字列、文字列の外の全角の空白、形の崩れた日付、単位の表に無い単位の付いた数があるとき。Cedar の `==`、`!=`、`&&`、`||`、`!` も sekisho の字句にはありません。

**直し方**: 示された位置を直してください。等しいことは `is`、等しくないことは `is not` で書き、「かつ」「または」「でない」は `and`、`or`、`not` で書きます。

**再現**:

```gate
gate t v1
description "閉じていない
```

関連: [E005](#e005)

<a id="e002"></a>

## E002 — キーワードを名前にしています

**いつ出るか**: 宣言した名前か別名が、条件と計算と型を書く語（`and`、`or`、`not`、`in`、`is`、`principal`、`resource`、`workflow`、`true`、`false`、`any`、`today`、`bool`、`date`、`number`、`rate`）のどれかのとき。名前を書く位置に、文字列や数など名前でないものを書いたときも出ます。

**直し方**: 別の名前にしてください。

**再現**:

```gate
gate t v1

role 係(is)
```

関連: [E007](#e007)

<a id="e003"></a>

## E003 — 節や行の順か数が違います

**いつ出るか**: ファイルが `gate` の行で始まらないとき、節が決まった順にないとき、一度だけの節（`description`、`namespace`、`today`）やブロックの中の一度だけの行を二度書いたとき、ブロックに要る行（action の `principal` と `resource`、ポリシーと期待の `action`、`separate` の `actions`）が無いとき、`attributes`、`input`、`context` の下に行が無いとき。

**直し方**: ファイルは、見出し（`gate`）、`description`、`namespace`、`use`、`today`、`enum`、`role`、`principal`、`workflow`、`resource`、`action`、`permit` と `forbid`、`expect`、`separate` の順に書いてください。ブロックの中の順は、診断の注に出ます。

**再現**:

```gate
gate t v1

resource 注文(Order)

role 係(clerk)
```

関連: [E005](#e005)

<a id="e004"></a>

## E004 — 字下げがそろっていません

**いつ出るか**: 字下げにタブがあるとき、同じブロックの行の字下げがそろっていないとき、字下げした行を取らない行の下に字下げした行があるとき。

**直し方**: 字下げはスペースで書き、同じブロックの行は同じ幅にしてください。`attributes`、`input`、`context` の下の行は、もう一段深く字下げします。

**再現**:

```gate
gate t v1

role 係(clerk)
	description "お客さまに応対する"
```

関連: [E003](#e003)

<a id="e005"></a>

## E005 — その場所に書けない行です

**いつ出るか**: 行の初めの語を、そのブロックが取らないとき（`role` の中の `roles`、行の初めの `when` など）、行がその語の取る形になっていないとき（`use` のあとの知らない語、`offset` の無い `today`、ほかの action と並べた `action any`、一つだけの `actions` など）。

**直し方**: 注に挙がる形に直してください。

**再現**:

```gate
gate t v1

role 係(clerk)
  roles 責任者
```

関連: [E003](#e003), [E004](#e004)

<a id="e006"></a>

## E006 — 同じ名前を二度宣言しています

**いつ出るか**: 同じ種類のもの（型と列挙、役割、列挙の値、型の属性、ワークフロー、action、action の入力と計算した値、ポリシー、期待、職務の分離、`use` の名前）のあいだで、名前か別名が重なるとき。`use gate` で読んだファイルの宣言とも比べます。規則の入力に二度値を渡したとき、`actions` に同じ action を二度並べたときも出ます。

**直し方**: どちらかの名前か別名を変えてください。

**再現**:

```gate
gate t v1

role 係(clerk)
role 窓口(clerk)
```

関連: [E007](#e007)

<a id="e007"></a>

## E007 — Cedar に出る名前に ASCII の別名がありません

**いつ出るか**: Cedar に出る名前（ファイル、列挙と値、役割、principal と resource の型、属性、ワークフロー、action、入力、計算した値、ポリシー）が別名の形の ASCII でなく、丸括弧の別名も無いとき、別名の形が違うとき、名前空間が ASCII の識別子でないとき。型の別名は `[A-Z][A-Za-z0-9]*`、ほかは `[a-z][a-z0-9_]*` です。期待と職務の分離は Cedar に出ないので、別名は要りません。

**直し方**: `返金する(refund_order)` のように、丸括弧で別名を付けてください。

**再現**:

```gate
gate t v1

role 係
```

関連: [E002](#e002), [E008](#e008)

<a id="e008"></a>

## E008 — 別名が Cedar か生成するコードの予約語です

**いつ出るか**: 別名（別名の無い ASCII の名前も）が、Cedar の予約語（`if`、`then`、`else`、`has`、`like`）か、TypeScript・Python・Go の予約語のとき。型の名前が、Cedar の `Action` と組み込みの型、sekisho が宣言する `Role` と `Workflow` のときも出ます。

**直し方**: 別の別名にしてください。

**再現**:

```gate
gate t v1

role 持つ(has)
```

関連: [E007](#e007)

<a id="e101"></a>

## E101 — 知らない名前です

**いつ出るか**: 参照した役割、型、属性、列挙の値、action、入力、計算した値、`use` の名前、規則の出力と入力、日付の関数、単位が無いとき。ポリシーと期待が、ほかのファイルの action を書いたときも出ます。

**直し方**: 書き違いを直すか、宣言を足してください。注に、書ける名前が並びます。

**再現**:

```gate
gate t v1

role 係(clerk)

principal 職員(User)
  roles 係

resource 注文(Order)

action 見る(view)
  principal 職員
  resource 注文

permit 係は見られる(clerks_view)
  principal in 系
  action 見る
```

関連: [E006](#e006)

<a id="e102"></a>

## E102 — 型が合いません

**いつ出るか**: 真偽を列挙の値と比べる、列挙を数と比べる、単位の違う数を比べる、真偽でない値を条件にそのまま書く、範囲を書けない型に範囲を書く、規則の入力に型の違う値を渡すとき。

**直し方**: 比べる値を、左の値の型に合わせてください。数は単位まで同じにします（`JPY` と `円` は同じ、税込と税抜は別）。

**再現**:

```gate
gate t v1

principal 職員(User)
  attributes
    停止中(suspended) : bool

resource 注文(Order)

action 見る(view)
  principal 職員
  resource 注文

forbid 停止中の職員(suspended_users)
  principal is 職員
  action 見る
  when principal.停止中 is はい
```

関連: [E103](#e103)

<a id="e103"></a>

## E103 — 範囲の誤りです

**いつ出るか**: 数か日付の属性と入力に `range` が無いとき、端が片方しかないとき、範囲に入る値が無いとき、定数がその単位で整数にならないとき、±(2⁵³ − 1) を超えるとき、条件で比べる定数が範囲の外のとき。

**直し方**: `range >=1GBP <=10_000GBP` のように両端を書き、定数をその単位で整数になるように直してください。

**再現**:

```gate
gate t v1

principal 職員(User)
  attributes
    返金できる額(refund_limit) : money[GBP, incl_tax]
```

関連: [E102](#e102)

<a id="e104"></a>

## E104 — v1 で書けない関係です

**いつ出るか**: 属性を二段以上たどるとき（`resource.order.customer`）、型の違う二つの属性を同じかで比べるとき、principal の来ない型の属性を principal と比べるとき、数をほかの値と比べるとき。

**直し方**: 関係は一段だけにしてください（`resource.customer is principal`、`resource.tenant is principal.tenant`、`principal in resource.team`）。二段たどる答えは、それを計算する規則の真偽の出力にできます。

**再現**:

```gate
gate t v1

principal 職員(User)

resource 注文(Order)

action 見る(view)
  principal 職員
  resource 注文

permit 持ち主は見られる(owners_view)
  principal is 職員
  action 見る
  when resource.注文.持ち主 is principal
```

関連: [E102](#e102)

<a id="e105"></a>

## E105 — 計算した値の誤りです

**いつ出るか**: 計算した値が規則でも日付でもないもの（`use openapi` の名前など）を呼ぶとき、規則の出力が列挙か真偽でないとき、要素の並びをたどる規則を呼ぶとき、規則や日付の入力に計算した値かエンティティを渡すとき、入力に値を渡していないとき。

**直し方**: 計算した値は、規則の列挙か真偽の出力、日付の関数との比べ方（`today <= …`）、営業日（`today is open in …`）にしてください。入力に渡せるのは、principal と resource の属性、action の入力、定数、`today` です。

**再現**:

```gate
gate t v1

use openapi 店 from "shop.json"

principal 職員(User)

resource 注文(Order)

action 見る(view)
  principal 職員
  resource 注文
  context
    許す(allowed) = 店(id: 1).ok
```

`shop.json`:

```
{"openapi": "3.1.0", "info": {"title": "shop", "version": "1"}, "paths": {}}
```

関連: [E101](#e101), [E102](#e102)

<a id="e106"></a>

## E106 — principal の型が action に来ません

**いつ出るか**: ポリシーか期待の `principal` の行（`is <型>`、`is workflow <名前>`、`in <役割>`）に当たる principal が、並べた action に来ないとき。`action any` では、どの action にも来ないとき。

**直し方**: `principal` の行を直すか、action の `principal` の行に型を足してください。役割で選ぶなら、その役割を持てる型の `roles` に役割を書きます。

**再現**:

```gate
gate t v1

principal 職員(User)

principal 顧客(Customer)

resource 注文(Order)

action 返金する(refund)
  principal 職員
  resource 注文

permit 顧客は返金できる(customers_refund)
  principal is 顧客
  action 返金する
```

関連: [E101](#e101)

<a id="e107"></a>

## E107 — `today` の誤りです

**いつ出るか**: 計算した値か規則の入力が `today` を使うのに `today` の行が無いとき、`offset` がタイムゾーンの名前か、`±HH:MM` の形でないとき。

**直し方**: `today range >=2026-10-01 <=2028-10-31 offset +00:00` のように、範囲と、日を変えるオフセットを数で書いてください。

**再現**:

```gate
gate t v1

# イングランドとウェールズの日で
today range >=2026-10-01 <=2026-12-31 offset Europe/London
```

関連: [E103](#e103)

<a id="e108"></a>

## E108 — 役割の `includes` が輪になっています

**いつ出るか**: 役割の `includes` をたどると、もとの役割に戻るとき（`clerk → manager → clerk`）。自分を `includes` する役割も輪です。輪一つにつき一度、輪の最初の役割の `includes` の行に、輪の役割を順に並べて出ます。

**直し方**: 輪のどこかの `includes` を消してください。`includes` は「この役割を持つ人は、その役割も持つ」なので、輪になると、どの役割がどれを含むかが決まらず、Cedar の役割の親子も輪になります。

**再現**:

```gate
gate t v1

role 係(clerk)
  includes 責任者

role 責任者(manager)
  includes 係
```

関連: [E101](#e101)

<a id="e201"></a>

## E201 — `use` のファイルが検査を通らないか、読めません

**いつ出るか**: `use rule` の規則が rulec の、`use dates` の日付のファイルが koyomi の検査を通らないか、読めないとき（その言語の言うことが注に出ます）。`use gate` のファイルが sekisho の検査を通らないとき、読めないとき、`use gate` が輪になっているときも出ます。

**直し方**: そのファイルを、その言語の検査が通るように直してください。

**再現**:

```gate
gate t v1

use gate "職員.gate"
```

関連: [E209](#e209)

<a id="e202"></a>

## E202 — `guards` が書く操作が、契約にありません

**いつ出るか**: `guards` の操作が、`use openapi`・`use asyncapi` の文書の `operationId`（無ければ方法とパス）や操作のキー、`use proto` のサービスとメソッド、`use book` の振替の操作のどれでもないとき。

**直し方**: 注にある操作の名前で書いてください。

**再現**:

```gate
gate shop v1
description "A shop's orders"

use openapi orders from "orders.json"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  guards orders refundOrders
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order
```

関連: [E203](#e203), [E205](#e205)

<a id="e203"></a>

## E203 — input が、操作の受け取るものと合いません

**いつ出るか**: `input` の名前が、守る操作の引数にも本文のフィールドにも無いとき。型が違うとき（数を小数や文字列として受け取る、列挙の値が違う）。範囲が操作の受け取る範囲を超えるとき。操作が求めない値に `?` が無いとき。

**直し方**: 操作が受け取るものを、同じ名前、同じ型、その中の範囲で書いてください。

**再現**:

```gate
gate shop v1
description "A shop's orders"

use openapi orders from "orders.json"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  guards orders refundOrder
  principal User
  resource Order from orderId
  input
    amount : money[GBP]  range >=1GBP <=20_000GBP

permit clerks_refund
  description "A clerk refunds up to 50 pounds"
  principal in clerk
  action refund_order
  when amount <= 50GBP
```

関連: [E202](#e202), [E204](#e204)

<a id="e204"></a>

## E204 — `from` の引数が、操作のパスかクエリにありません

**いつ出るか**: `resource … from <引数>` の引数が、守る操作のパスかクエリの引数（proto ならリクエストのフィールド）に無いとき。生成するコードは、その引数で resource を読みます。

**直し方**: 操作のパスかクエリの引数の名前で書いてください。

**再現**:

```gate
gate shop v1
description "A shop's orders"

use openapi orders from "orders.json"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  guards orders refundOrder
  principal User
  resource Order from order_id

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order
```

関連: [E203](#e203)

<a id="e205"></a>

## E205 — 一つの操作を、二つの action が守ります

**いつ出るか**: 二つの action が、同じ契約の同じ操作を `guards` に書いたとき。どちらの判断で守るかが決まりません。

**直し方**: 一つの action にまとめてください。

**再現**:

```gate
gate shop v1
description "A shop's orders"

use openapi orders from "orders.json"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  guards orders refundOrder
  principal User
  resource Order

action refund_again
  description "Refund an order once more"
  guards orders refundOrder
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order, refund_again
```

関連: [E202](#e202)

<a id="e206"></a>

## E206 — 規則か日付に渡す値が、入力の範囲を外れるか、規則の前提を破りえます

**いつ出るか**: 計算した値で規則や日付の関数に渡す属性・input・定数の範囲が、その入力の範囲に収まらないとき。渡す値の範囲の中に、規則の前提（`constraint`）を破る例があるとき。

**直し方**: 渡す値の範囲を、入力の範囲（前提を守る範囲）に狭めてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu sekisho check example.gate` を走らせます。

`example.gate`:

```
gate shop v1
description "A shop's orders"

use rule limit from "limit.rule"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk
  attributes
    refund_limit : money[GBP]  range >=0GBP <=200GBP

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal User
  resource Order
  input
    amount : money[GBP]  range >=1GBP <=100GBP
  context
    band = limit(amount: amount, limit: principal.refund_limit).band

permit clerks_refund_within_their_limit
  description "A clerk refunds up to the clerk's limit"
  principal in clerk
  action refund_order
  when band is within_limit
```

`limit.rule`:

```rule
rule limit v1
description "Whether a refund is within a limit of at most 100 pounds"

enum band = within_limit | over_limit

inputs
  amount : money[GBP]  range >=1GBP <=100GBP
  limit  : money[GBP]  range >=0GBP <=100GBP

outputs
  band : band

derive excess : money[GBP] = amount - limit  range >=-99GBP <=100GBP

table decide
policy unique
| excess | -> band : band |
| <=0GBP | within_limit   |
| >0GBP  | over_limit     |
```

関連: [W201](#w201), [E201](#e201)

<a id="w201"></a>

## W201 — 規則に渡す値が規則の前提を守るかを、決められません

**いつ出るか**: 渡す値の範囲で、規則の前提（`constraint`）を守るかを rulec が決められないとき。生成するコードは、走らせたときに前提を確かめ、破ればリクエストを拒みます。

**直し方**: 渡す値の範囲を狭めると、決められることがあります。

**再現**: まだ再現がありません。これを出す検査は、この先の段階で入ります。

関連: [E206](#e206)

<a id="e207"></a>

## E207 — カレンダーが、today の日をすべては覆いません

**いつ出るか**: `today is open in <カレンダー>` のカレンダーのデータの範囲（表が知る日）が、`today` の範囲を覆わないとき。

**直し方**: today の範囲をデータの範囲の中に狭めるか、新しい表が出てからカレンダーのコピーを取り直してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu sekisho check example.gate` を走らせます。

`example.gate`:

```
gate shop v1
description "A shop's orders"

use calendar days from "closed_days.cal"

today range >=2026-10-01 <=2027-03-31 offset +00:00

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal User
  resource Order
  context
    business_day = today is open in days

permit clerks_refund_on_business_days
  description "A clerk refunds on a business day"
  principal in clerk
  action refund_order
  when business_day
```

`closed_days.cal`:

```cal
calendar closed_days v1

source holidays = file "holidays.csv" sha256:899aee90fcd554a9
  format csv
  covers 2026-01-01..2026-12-31

closed weekly sat, sun
closed holidays
```

`holidays.csv`:

```
2026-01-01,New Year's Day
2026-05-04,Greenery Day
```

関連: [E107](#e107)

<a id="e208"></a>

## E208 — ワークフローの `.flow` が dandori の検査を通りません

**いつ出るか**: `workflow <名前> from "<.flow>"` のフローが、dandori の検査を通らないか、読めないとき。

**直し方**: フローを dandori の検査が通るように直してください。注に、dandori の言うことがあります。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu sekisho check example.gate` を走らせます。

`example.gate`:

```
gate shop v1
description "A shop's orders"

workflow returns from "returns.flow"
  description "Refunds a returned order"

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal Workflow
  resource Order

permit returns_refunds
  description "The returns workflow refunds"
  principal is workflow returns
  action refund_order
```

`returns.flow`:

```flow
workflow returns v1
description "Calls a task it does not declare"

inputs
  order_id : string

outputs
  refund : string

flow
  let r = refund_order(orderId: order_id)
  succeed refund = r.id
```

関連: [E201](#e201)

<a id="e209"></a>

## E209 — ほかの言語を読めない sekisho で、それを読むファイルを確かめています

**いつ出るか**: sekisho 単独のバイナリ（`sekisho`）で、`use rule`、`use dates`、`use calendar`、`use book`、`workflow … from` のあるファイルを確かめるとき。このバイナリには rulec、koyomi、chobo、dandori が入っていません。最初の一行で一度だけ出し、ほかの診断は出しません。exit code は 2 です（ファイルの誤りではなく、走らせ方の問題なので）。

**直し方**: 同じコマンドを `ritsu sekisho` で走らせてください（`ritsu sekisho check refunds.gate`）。`ritsu sekisho` は、規則、日付のファイル、カレンダー、帳簿、フローを同じプロセスの中で読みます。

**再現**:

```gate
gate t v1

use rule 上限 from "上限.rule"
```

関連: [E201](#e201)

<a id="e210"></a>

## E210 — `use gate` で読むファイルの名前空間が違います

**いつ出るか**: `use gate` で読んだファイルの Cedar の名前空間が、読む側のファイルの名前空間と違うとき。`namespace` を書かないファイルの名前空間は、ファイルの別名を Pascal case にしたもの（`refunds` なら `Refunds`）なので、`use gate` で読み合うファイルには、同じ `namespace` の行が要ります。

**直し方**: 二つのファイルに、同じ `namespace` の行を書いてください（`namespace Shop`）。

**再現**:

```gate
gate 返金(refunds) v1
namespace Shop

use gate "職員.gate"
```

`職員.gate`:

```
gate 職員(staff) v1
namespace Staff

role 係(clerk)
```

関連: [E211](#e211), [E201](#e201)

<a id="e211"></a>

## E211 — `use gate` で読んだ二つのファイルが、同じものを宣言しています

**いつ出るか**: `use gate` で読んだ二つのファイル（読んだファイルがさらに読むものも含む）が、同じ名前か別名の型・列挙・役割・ワークフローを宣言しているとき。読んだものは一つの Cedar の名前空間に並ぶので、同じ名前のものが二つになります。読んだ二つのファイルが同じファイルを読むときは、そのファイルを一つと数えます。あとから読んだファイルの `use gate` の行に出ます。

**直し方**: 同じ名前のものは一つのファイルにだけ宣言し、ほかのファイルはそのファイルを `use gate` で読んでください。

**再現**:

```gate
gate 返金(refunds) v1
namespace Shop

use gate "職員.gate"
use gate "店員.gate"
```

`職員.gate`:

```
gate 職員(staff) v1
namespace Shop

role 係(clerk)
```

`店員.gate`:

```
gate 店員(clerks) v1
namespace Shop

role 係(clerk)
```

関連: [E210](#e210), [E006](#e006)

<a id="e301"></a>

## E301 — どの組み合わせでも、どの permit も action を許しません

**いつ出るか**: action のどの組み合わせ（principal の型と役割、属性、input、計算した値）でも、当てはまる permit が無いか、forbid に拒まれるとき。だれもその action をできません。

**直し方**: permit を書いてください。だれにもさせないつもりなら、action の下に `nobody "<理由>"` を書いてください。

**再現**:

```gate
gate shop v1
description "A shop's orders"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal User
  resource Order
```

関連: [E302](#e302), [E303](#e303)

<a id="e302"></a>

## E302 — permit が許すはずの組み合わせを、どれも forbid が拒みます

**いつ出るか**: permit が当てはまる組み合わせが、どれも forbid にも当てはまるとき。その permit は何も許しません。

**直し方**: forbid を狭めるか、だれにも許さないつもりなら permit を消してください。

**再現**:

```gate
gate shop v1
description "A shop's orders"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order

forbid nobody_refunds
  description "No one refunds"
  action refund_order
```

関連: [E301](#e301), [E303](#e303)

<a id="e303"></a>

## E303 — permit か forbid が、どの組み合わせにも当てはまりません

**いつ出るか**: ポリシーの `principal` の行と `when`・`unless` の行が、どの組み合わせでもそろって成り立たないとき。

**直し方**: 満たせない行か、ポリシーそのものを消してください。

**再現**:

```gate
gate shop v1
description "A shop's orders"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk
  attributes
    suspended : bool

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order

forbid suspended_and_not
  description "Asks for a suspended user who is not suspended"
  action refund_order
  when principal.suspended
  unless principal.suspended
```

関連: [E302](#e302)

<a id="e304"></a>

## E304 — 期待が成り立ちません

**いつ出るか**: `expect allow` か `expect deny` が選ぶ組み合わせのうち、期待と違う答えになるものがあるとき。成り立たない数と、一つの例を示します。

**直し方**: ポリシーを直すか、期待が言いすぎているなら期待を直してください。

**再現**:

```gate
gate shop v1
description "A shop's orders"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order

expect deny clerks_never_refund
  description "No clerk refunds"
  principal in clerk
  action refund_order
```

関連: [E305](#e305)

<a id="e305"></a>

## E305 — 職務の分離が成り立ちません

**いつ出るか**: `separate` に並べた action のうち二つを、同じ principal（同じ型、同じ役割の組、同じ属性）が許されるとき。resource と context は action ごとに選べます。

**直し方**: 片方を許される principal がもう片方をできないよう forbid を足すか、permit を狭めてください。

**再現**:

```gate
gate shop v1
description "A shop's orders"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal User
  resource Order

action export_refunds
  description "Export the refunds"
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order

permit clerks_export
  description "A clerk exports the refunds"
  principal in clerk
  action export_refunds

separate refunding_and_exporting
  description "Whoever refunds does not export the refunds"
  actions refund_order, export_refunds
```

関連: [E304](#e304)

<a id="e306"></a>

## E306 — 役割が、`can` に無い action を許されます

**いつ出るか**: その役割（と、それが includes する役割）だけを持つ principal が、どれかの組み合わせで、`can` に無い action を許されるとき。

**直し方**: `can` に action を足すか、許す permit を狭めてください。

**再現**:

```gate
gate shop v1
description "A shop's orders"

role clerk
  description "Answers customers"
  can view_order

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"

action view_order
  description "Look at an order"
  principal User
  resource Order

action refund_order
  description "Refund an order"
  principal User
  resource Order

permit clerks_view_and_refund
  description "A clerk looks at an order and refunds it"
  principal in clerk
  action view_order, refund_order
```

関連: [W302](#w302)

<a id="e307"></a>

## E307 — action の組み合わせが、予算を超えます

**いつ出るか**: action の組み合わせの数（と、日付を確かめるために数える today と値の組）が、予算（既定は 10⁸ 通り、`--budget` で変えられる）を超えるとき。サンプリングはせず、その action は確かめず、何も生成しません。

**直し方**: 注に、数を増やしているものが出ます。役割なら、型の役割を分けるか役割ごとに `can` で確かめ、属性や定数なら、条件が読む値を減らしてください。`--budget` で予算を上げることもできます。

**再現**:

```gate
gate shop v1
description "Fourteen attributes of four values each: more than 10^8 combinations"

enum level = a | b | c | d

principal User
  description "A member of the staff"
  attributes
    x1 : level
    x2 : level
    x3 : level
    x4 : level
    x5 : level
    x6 : level
    x7 : level
    x8 : level
    x9 : level
    x10 : level
    x11 : level
    x12 : level
    x13 : level
    x14 : level

resource Order
  description "An order"

action read_order
  description "Look at an order"
  principal User
  resource Order

permit level_a_reads
  description "Whoever is at level a everywhere reads"
  action read_order
  when principal.x1 is a and principal.x2 is a and principal.x3 is a and principal.x4 is a and principal.x5 is a and principal.x6 is a and principal.x7 is a
  when principal.x8 is a and principal.x9 is a and principal.x10 is a and principal.x11 is a and principal.x12 is a and principal.x13 is a and principal.x14 is a
```

<a id="w301"></a>

## W301 — permit が許すものを、ほかの一つの permit が全部許します

**いつ出るか**: permit が許す組み合わせを、どれもほかの一つの permit も許すとき。消しても答えは変わりません。二つが互いを覆うときは、後に書いたほうにだけ言います。

**直し方**: その permit を消すか、そちらが意図どおりなら、覆うほうの permit を狭めてください。

**再現**:

```gate
gate shop v1
description "A shop's orders"

enum status = paid | refunded

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"
  attributes
    status : status

action refund_order
  description "Refund an order"
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order

permit clerks_refund_paid_orders
  description "A clerk refunds a paid order"
  principal in clerk
  action refund_order
  when resource.status is paid
```

関連: [E302](#e302)

<a id="w302"></a>

## W302 — 役割だけでは、`can` に書いた action を一度も許されません

**いつ出るか**: その役割（と、それが includes する役割）だけを持つ principal が、`can` に書いた action を、どの組み合わせでも許されないとき。

**直し方**: `can` から action を消すか、許す permit を書いてください。

**再現**:

```gate
gate shop v1
description "A shop's orders"

role clerk
  description "Answers customers"

role auditor
  description "Checks the refunds"
  can refund_order

principal User
  description "A member of the staff"
  roles clerk, auditor

resource Order
  description "An order"

action refund_order
  description "Refund an order"
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds"
  principal in clerk
  action refund_order
```

関連: [E306](#e306)

<a id="w303"></a>

## W303 — 起こるかどうかを確かめられない値に答えがかかっていて、決められません

**いつ出るか**: 規則が、問われた範囲で出力がとりうる値を正確に言えず（数を計算する derive の上の表など）、多めに数えた値から出た答えを、具体的な入力で起こせないとき。読んだ Cedar に有限でない式があるときも出ます。

**直し方**: 規則に渡す値の範囲を狭めるか、規則の表を、整数で届く行だけにしてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu sekisho check example.gate` を走らせます。

`example.gate`:

```
gate shop v1
description "A shop's orders"

use rule halves from "halves.rule"

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk
  attributes
    first  : number  range >=0 <=3
    second : number  range >=0 <=3

resource Order
  description "An order"

action read_order
  description "Look at an order"
  principal User
  resource Order
  context
    kind = halves(first: principal.first, second: principal.second).answer

permit clerks_read
  description "A clerk reads"
  principal in clerk
  action read_order

permit halves_read
  description "Whoever is at one and a half twice reads"
  principal in clerk
  action read_order
  when kind is odd_half
```

`halves.rule`:

```rule
rule halves v1
description "A row that asks for a sum of 3 and a difference of 0: real numbers reach it, whole numbers do not"

enum kind = odd_half | other

inputs
  first  : number  range >=0 <=3
  second : number  range >=0 <=3

outputs
  answer : kind

derive total : number = first + second  range >=0 <=6
derive gap   : number = first - second  range >=-3 <=3

table t
policy first
| total | gap | -> answer : kind |
| 3     | 0   | odd_half         |
| -     | -   | other            |
```

関連: [W201](#w201)

<a id="w304"></a>

## W304 — 期待が、どの組み合わせも選びません

**いつ出るか**: `expect` の `principal`・`when`・`unless` の行が、その action のどの組み合わせでも、そろって成り立たないとき。期待は成り立ちますが、何も確かめていません（たいていは行の書き違いです）。

**直し方**: 同時に満たせない行を直すか、期待を消してください。

**再現**:

```gate
gate 店(shop) v1
description "店の注文"

enum 注文の状態(order_status) = 支払済(paid) | 返金済(refunded)

role 係(clerk)
  description "お客さまに応対する"

principal 職員(User)
  description "店の職員"
  roles 係

resource 注文(Order)
  description "店の注文"
  attributes
    状態(status) : 注文の状態

action 返金する(refund_order)
  description "注文を返金する"
  principal 職員
  resource 注文

permit 係は返金できる(clerks_refund)
  description "係は、まだ返金していない注文を返金できる"
  principal in 係
  action 返金する
  unless resource.状態 is 返金済

expect deny 支払済で返金済の注文は返金しない
  description "支払済でもあり返金済でもある注文を、返金する人はいない"
  action 返金する
  when resource.状態 is 支払済
  when resource.状態 is 返金済
```

関連: [E304](#e304), [E303](#e303)

<a id="w401"></a>

## W401 — Verified Permissions の上限を超えます

**いつ出るか**: 生成したポリシーかスキーマが、Verified Permissions の上限（ポリシー 10,000 バイト、スキーマ 100,000 バイト、親の深さ 100 など）を超えるとき（`--authorizer avp`）。

**直し方**: ポリシーを分けるか、役割の入れ子を浅くしてください。

**再現**: まだ再現がありません。これを出す検査は、この先の段階で入ります。
