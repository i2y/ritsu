# ritsu

**システムが守るべき決まりを小さな言語で書き、どんな入力にも答えが一つに決まるか、決まりどうしが食い違っていないかを確かめる。確かめた決まりから、コード、人が読むページ、テストに使う入力と答えの組やシナリオを生成できる。**

コーディングエージェントは、人が一行ずつ読めるより速く、多くのコードを書くようになりました。そこで問いがはっきりします。誰が、あるいは何がコードを書いても、コードが実現すべきものは何か。サービスのあいだと中の契約。業務の規則。暦と期日。帳簿と、その上限と下限。ワークフローの骨組み。だれが何をしてよいか。要件がどこから来て、いま何がそれを満たしているか。

ritsu は、そのそれぞれに小さな言語を一つずつ与えます。どれも、人が読めるファイルです。各言語は、書いたことを確かめられるところまで確かめます。試しに選んだ例ではなく、すべての入力、すべての日、すべての道筋についてです。コードが要るところではコードを生成し、中身を理解したい人のためのページを作ります。九つが一つの処理系を共有しているので、証明が言語の境目で切れません。ワークフローは呼ぶ規則の前提を知り、規則は暦がとりうる日を知り、帳簿は規則が返しうる額を知っています。

つなぎのコードはエージェントが書き、コードが実現すべきものは ritsu が持ちます。

## 九つの言語

| | ファイル | 何のための言語か | 生成するもの |
|---|---|---|---|
| [rulec](#rulec業務の規則) | `.rule` | 業務の規則：運賃、手数料、資格、税 | 12 の言語の関数 |
| [dandori](#dandoriワークフローの骨組み) | `.flow` | ワークフローの骨組み | Temporal、Step Functions、Argo ほか |
| [koyomi](#koyomi期日) | `.cal` | 期日：締め日と支払日、営業日、法令の期間 | TypeScript、Python、Go、Rust、SQL、Wasm |
| [chobo](#chobo帳簿) | `.book` | 帳簿：勘定、その上限と下限、振替、仮押さえ | PostgreSQL、TigerBeetle、MySQL、TiDB、Vitess を使うクライアント |
| [geas](#geasエージェントが書いたコードについての主張) | `.geas` | エージェントが書いたコードについての主張 | — |
| [yuen](#yuen要件の出どころ) | `.req` | 要件の出どころと、それを満たすもの | ReqIF、W3C PROV |
| [sakai](#sakai境界づけられたコンテキストの地図) | `.ctx` | 境界づけられたコンテキストの地図 | import を確かめるツールの設定、Context Mapper |
| [sekisho](#sekishoだれが何をしてよいか) | `.gate` | だれが何をしてよいか：役割、属性、関係。規則と日付を条件に使える | Cedar と、Cedar に尋ねる TypeScript、Python、Go |
| [kaname](#kanamefunctional-core) | `.kn` | Functional Core。入出力を持たない汎用のコードを ritsu の型で書き、規則と日付と帳簿も呼べる | `ritsu gen` が書くパッケージの中の TypeScript、Python、Go |

どの言語も、それだけで使えます。`.flow` を書かずに rulec だけを使うこともできます。一つのファイルに二つの言語を混ぜることはしません。運賃、暦、帳簿、ワークフローを読む人は、それぞれ違うからです。

以下の例は英語で書いたものです。dandori、koyomi、chobo、geas などの例には、日本語で書いた版（`.ja.flow`、`.ja.cal` など）もあります。

### rulec：業務の規則

規則は、表と、そのまわりの計算、例外、ただし書きで書きます。`rulec check` は、宣言した範囲のすべての入力に答えがちょうど一つあることを証明し、単位とお金を確かめ、Lean の証明が検査し直す証明書を書きます。数字は、転記した元の出典に固定でき、出典が変われば検査が止まります。検査を通った規則だけが、Python、NumPy、TypeScript、JavaScript、Rust、Ruby、PHP、Go、Swift、Java、SQL、Wasm の普通の関数になります。

```rule
rule paypal_fee v1
description "The PayPal Checkout fee on one payment in the United States. Transcribed from PayPal's published merchant fees"

source paypal = file "sources/paypal-us-fees.md" sha256:0318950a982c3c7d  # PayPal's own published figures
  table1 sha256:5e481ef40e570eeb

inputs
  amount        : money[USDc]  range >=1USDc <=100000000USDc
  international : bool

# The fee has fractions of a cent in it, and the page does not say which way they settle, so
# the direction here is a placeholder — the thing a person has to decide before this ships.
outputs
  fee : money[USDc]  round half_up(1USDc)

# The page prints the domestic rate and, separately, what an international transaction adds.
# It does not print the sum, so neither does this: the row for a domestic payment adds
# nothing, and that row carries no citation because `0%` is not a figure the copy shows.
table surcharge
policy unique
| international | -> extra : rate[step 0.01%] |
| false         | 0%                          |
| true          | 1.5%                        |  @paypal table1

define fee : money[USDc] = amount × 3.49% + amount × extra + 49USDc  @paypal table1

examples
| amount    | international | -> fee  |
| 10000USDc | false         | 398USDc |
| 10000USDc | true          | 548USDc |
```

規則は、ほかのファイルが足せる場所を残しておけます。項を合わせて決まる値、列挙、表です。`contribution` のファイルがそこに足します。キャンペーンなら値引きを、商品ラインなら商品の種類とその上乗せを足し、頼る形を固定するので、その形が変われば止まります。読み込むものの一覧はありません。`ritsu check` と `ritsu gen` は、プロジェクトの寄与を、それが届く規則に合わせ、規則をそれだけでも合わせたものでも検査し、寄与が列挙に足した値の行を、その列挙を読むどの表にも求めます。

```rule
contribution garden_price(garden_price) v1
description "The markup of garden supplies"
to "../product_price.rule" signature:d5379dca48aa13d2
after "garden_kind.rule" signature:650e995b6929bca3

clause garden_markup -> markup
  when item_kind garden
  then 15%
```

```console
$ cd crates/ritsu/tests/projects/product_line
$ ritsu check --lang ja
ok rules/contrib/garden_kind.rule（rules/shared/product_kinds.rule への寄与）
ok rules/contrib/garden_price.rule（rules/product_price.rule への寄与）
ok rules/product_price.rule（寄与 2 件: rules/contrib/garden_price.rule, rules/contrib/garden_kind.rule）
ok rules/shared/product_kinds.rule（寄与 1 件: rules/contrib/garden_kind.rule）
pricing.flow: 検査を通りました
ritsu check: ファイル 5 個（rulec 4、dandori 1）。どれも検査を通りました。言語の境目: 確かめた 0 か所、決められない 0 か所
```

### dandori：ワークフローの骨組み

ワークフローには、何をどの順に呼ぶか、リトライとタイムアウト、そして呼び出しが外の案件（注文、支払い、予約）をどう動かすかを書きます。`dandori check` は、すべての道筋で、引き渡さない案件を、手を付けないか終えるかのどちらかで残し、途中で放り出さないことを示します。同じファイルが、Temporal（TypeScript、Python、Go）、Step Functions、Argo Workflows、Lambda の durable functions、pydantic-graph のコードになり、どれも dandori の参照インタプリタと突き合わせて確かめます。

```flow
workflow arrange_delivery v1
description "Book the delivery of an order with the carrier the urgency rule chose, and answer with its tracking number. Written once for every platform, since its calls are HTTP ones that dandori writes for each (`connection` is what Step Functions needs of them): each version of fulfillment runs it as its child, a child workflow on Temporal, an invoked durable function on Lambda durable functions, a workflow of this WorkflowTemplate on Argo, a nested execution on Step Functions"

enum carrier = standard | next_day

record Booking
  tracking_number : string

inputs
  order_id  : string
  carrier   : carrier
  recipient : string?
  extra     : json

outputs
  tracking_number : string

# The next-day carrier sends a van for the parcels; on a busy day it has no van left.
task book_next_day(order_id: string, recipient: string?) -> Booking
  http POST "https://next-day.example.com/v1/pickups"
  connection "arn:aws:events:ap-northeast-1:123456789012:connection/next-day/2a3b4c"
  errors no_van = 409
  key
  retry 2 times every 5 seconds

task book_standard(order_id: string, recipient: string?, extra: json) -> Booking
  http POST "https://post.example.com/v1/parcels"
  connection "arn:aws:events:ap-northeast-1:123456789012:connection/post/3b4c5d"
  key
  retry 2 times every 5 seconds

flow
  match carrier
    next_day =>
      let booked = book_next_day(order_id: order_id, recipient: recipient)
        on no_van => fail NoVan "No next-day van is left for order {order_id}"
      succeed tracking_number = booked.tracking_number
    standard =>
      let booked = book_standard(order_id: order_id, recipient: recipient, extra: extra)
      succeed tracking_number = booked.tracking_number
```

### koyomi：期日

期日は、契約や法令の言い方のまま書きます。請求日から 30 日後、休みなら次の営業日、というふうにです。`koyomi check` は、宣言した範囲のすべての日について日付を計算し、書いた条件をその一日ずつで確かめます。暦は固定した出典から読みます（ここでは GOV.UK が出しているイングランドとウェールズの祝日）。TypeScript、Python、Go、Rust、SQL のコードと、どのホストからも JSON のオブジェクト一つで呼べる Wasm のモジュールを生成します。

```cal
dates net30 v1
description "Net 30: due 30 days after the invoice date, moved to the next business day in England and Wales when that day is closed. The claims are this example's own, written as they read"
use calendar "calendars/england_and_wales.cal"

inputs
  invoice_date : date  range >=2026-01-01 <=2028-11-29

date due = invoice_date
  + 30 days             # "Net 30": due 30 days after the invoice date
  roll following        # on the next business day when that day is closed

claims
  due_on_a_business_day   : due is open
  at_least_30_days        : due >= invoice_date + 30 days
  later_invoice_later_due : due is monotonic
```

```console
$ cd crates/koyomi/examples
$ koyomi check net30.cal --lang ja
net30.cal: ok — 3 つの条件が、invoice_date 2026-01-01〜2028-11-29 の 1,064 日のすべてで成り立ちます
```

### chobo：帳簿

帳簿には、勘定と、それぞれが守る上限と下限、勘定のあいだの振替を書きます。chobo は、振替、仮押さえ、期限切れをどう重ねても、勘定が上限と下限の外に出ないことを確かめます。生成するクライアントでは、どの振替も、上限と下限を守る一度の書き込みになります。PostgreSQL、TigerBeetle、MySQL の系統（MySQL、TiDB、Vitess）の上で、TypeScript、Python、Go、Rust で使えます。

```book
# A refund never exceeds the sale: what is left to refund on each order is an account of its own.
book refunds v1
description "What is left to refund on each order is an account of its own: the sale adds to it, a refund takes from it, and it never goes below 0. A refund is held while it waits for approval"

unit USD scale 2

account refundable(order: string) : USD
  description "what is left to refund on the order"
  at least 0 refused as refund_exceeds_sale
account sales : USD outside
account refunded : USD outside

transfer sale(order: string, amount: USD)
  key order
  move amount from sales to refundable(order)

transfer refund(request: string, order: string, amount: USD)
  description "held while it waits for approval: posted when it is approved, voided when it is turned down"
  key request
  pending expires after 7 days
  move amount from refundable(order) to refunded
```

### geas：エージェントが書いたコードについての主張

コードはエージェントが書き、主張は人が読みます。geas は主張を一つずつ、動いているプログラム（コマンドライン、HTTP のサービス、ブラウザのページ）に当てて確かめ、差分を渡せば、それがどの主張にかかわるかを答えます。OpenSpec の仕様を渡せば、`geas scenarios` が、同じ名前の主張で確かめていないシナリオを挙げます。

```geas
# The human-auditable half. The implementation (calc.py) is agent-written;
# these claims are what a reviewer actually reads.

target calc {
  run "python3 calc.py"
}

claim "adds two integers" {
  when calc.run("2", "+", "3")
  then stdout is "5"
  and  exit is 0
}

claim "multiplies" {
  when calc.run("6", "*", "7")
  then stdout is "42"
}

claim "refuses division by zero" {
  when calc.run("1", "/", "0")
  then exit is 1
  and  stderr contains "division by zero"
}

claim "rejects an unknown operator" {
  when calc.run("1", "%", "2")
  then exit is 2
  and  stderr contains "unknown op"
}
```

### yuen：要件の出どころ

要件ごとに、どこから来たか（法令の条、文書、決定）、誰が受け持つか、何が満たし何が確かめるかを書きます。どのリンクも両端のハッシュで固定されていて、出典、要件、満たすもののどれかが変われば、その先のリンクは、人が見直すまで止まります。yuen は米国の eCFR と日本の e-Gov から法令を読み、ReqIF と W3C PROV に書き出せます。[OpenSpec](https://github.com/Fission-AI/OpenSpec) の仕様の要件も同じように固定し、変更が要件を書き換えると止まります。

```req
requirements osha v1
description "A requirement read from 29 CFR 1910.157, for the tests. Written as an example; it does not say how the regulation is to be read"

role safety "decides how the regulation reads"

source osha = law ecfr "29 CFR 1910" asof 2026-01-01
  "§1910.157" sha256:c2a9ce966c7e2269

requirement extinguisher_distance
  text "No employee travels more than 75 feet to a portable fire extinguisher for Class A fires"
  owner safety
  from @osha "§1910.157"
    reviewed 2026-10-03 by safety sha256:c2a9ce966c7e2269 -> sha256:08a4819829372b9b
  not satisfied "The test material names no artifact"
    approved 2026-10-03 by safety sha256:08a4819829372b9b
  not verified "The test material names no claim"
    approved 2026-10-03 by safety sha256:08a4819829372b9b
```

### sakai：境界づけられたコンテキストの地図

地図には、どのコンテキストがどのファイルを持ち、どの言語を公開し、誰が誰に何を通して依存してよいかを書きます。sakai は、コンテキストのあいだをまたぐ参照を、ここにあるすべての言語のファイルと、サービスが契約として持つ OpenAPI と AsyncAPI の文書と、コードについて確かめます。コードの import は import-linter、dependency-cruiser、ArchUnit、go-arch-lint に渡し、Rust のクレートは Cargo に尋ねます。帳簿の `use book` は数えません。ほかのコンテキストからは、帳簿の前に立つサービスを通して使うからです。ritsu は自分のクレートをこの形で地図にしています。次はそのコンテキストの一つです。

```ctx
context Workflows(dandori) v1
description "Workflows that call the rules, made into Temporal, Step Functions, Argo and more (dandori)"

owns
  dir "../crates/dandori"

published language dandori
  crate "../crates/dandori"

# A language stands on the base, and reads another language only through the ports.
upstream Base conformist
  through ritsu_base, ritsu_units, ritsu_proto, ritsu_emit, ritsu_ports
```

### sekisho：だれが何をしてよいか

ゲート（`.gate` のファイル）には、どの principal が、どの resource に、どの action をしてよいかを書きます。条件には、役割、属性、関係のほかに、規則の答えと日付の答えを使えます。`sekisho check` は、規則がどの答えを返しうるかを rulec に、どの日付が同じ日にそろいうるかを koyomi に尋ね、起こりうる組み合わせを全部たどって、どれも Cedar と同じ決まりで答えを決めます。だれもできない action、forbid に覆われた permit、成り立たない期待、一人が両方を許される二つの職務があれば、検査は通りません。検査を通ったゲートだけを、Cedar のスキーマとポリシーと、TypeScript、Python、Go のコードにします。このコードが、条件の値をサービス自身のデータから計算して Cedar に尋ねるので、呼ぶ側が条件の値を渡すことはできません。

```gate
action refund_order
  description "Refund an order, in part or in whole"
  guards orders refundOrder
  principal User, Workflow
  resource Order from orderId
  input
    amount : money[GBP, incl_tax]  range >=1GBP <=10_000GBP
  context
    refund_band  = refund_limit(amount: amount, limit: principal.refund_limit).band
    in_period    = today <= refund_terms.last_day(paid_on: resource.paid_on)
    business_day = today is open in uk
…
permit clerks_refund_within_their_limit
  description "A clerk refunds up to the clerk's own limit, while the refund period lasts"
  principal in clerk
  action refund_order
  when refund_band is within_limit
  when in_period
…
expect deny clerks_never_refund_over_their_limit
  description "A clerk who is not a manager never refunds more than the clerk's own limit"
  principal in clerk
  action refund_order
  unless principal in manager
  when refund_band is over_limit
```

```console
$ cd crates/sekisho/examples/refunds
$ sekisho check refunds.gate --lang ja
refunds.gate: ok — action 3、ポリシー 10（permit 7、forbid 3）、期待 3、職務の分離 1
```

この例は、[サイトのページ](https://i2y.github.io/ritsu/ja/sekisho/)で、検査が規則に何を尋ねるか、生成する Cedar とコード、人が読むページまで、順に見られます。

### kaname：Functional Core

kaname は、プログラムのうち、入出力を持たずに値を受け取って値を返す部分（Functional Core）を書く、小さな汎用の言語です。モジュール（`.kn` のファイル）には、イベントで注文の状態を進める、人が入力した文字列を確かめて形を整える、倉庫の二回の棚卸しを辞書と集合で比べる、木を作り替える、といった計算を、ritsu の型（レコード、直和型、リスト、辞書、集合、文字列、ジェネリクス、`result`、関数の値）で書きます。規則、日付、帳簿もそれぞれの型のまま呼べ、規則の入力を組み立て、一つの答えを次の呼び出しに渡し、額を帳簿に渡すコードも書けます。`kaname check` は、モジュールをほかの言語と同じ決まり（単位とお金、丸め、int64）で確かめ、渡す値がどれも相手の範囲に入るかも確かめます。`total` と書いた関数が、宣言した範囲のどの入力にも答えることも示します。表ではなくコードに書いた判断は、警告になります。`ritsu gen` は、ホストが呼ぶ TypeScript、Python、Go のコードを、呼ぶ規則と日付と帳簿のコードの横に書きます。

イベントで進む注文は、それだけで閉じた Functional Core です。状態を、値の下にフィールドを書いた列挙の値にし、次の状態か、イベントを拒む理由を、状態とイベントの `match` で決めます。

```kn
enum Order
  cart
    lines : list[Line]  count <=50
  placed
    lines : list[Line]  count >=1 <=50
    total : money[USDc]
    on    : date
…
fn next(order: Order, event: Event) -> result[Order, string]
  match order
    cart c =>
      match event
        add a =>
          if c.lines.count() < 50
            ok(Order.cart { lines: c.lines.append(a.line) })
          else
            err("a cart holds 50 lines")
…
    shipped, cancelled => err("the order is closed")
```

検査は、どの `match` も状態とイベントのどの値にも答えることを確かめ、カートの明細が 50 件に収まること、その合計が int64 に収まることを示します。そのため、どの関数も total です。

```console
$ cd crates/kaname/examples/order_flow
$ ritsu check --lang ja
ok order_flow.ja.kn: 関数 5 個、total
ok order_flow.kn: 関数 5 個、total
ritsu check: ファイル 2 個（kaname 2）。どれも検査を通りました。言語の境目: 確かめた 0 か所、決められない 0 か所
```

注文を持ち、イベントが来るたびに `ritsu gen` がモジュールから書いたコードを呼ぶのは、ホストです。TypeScript のホスト（`crates/kaname/examples/order_flow/host/main.ts`。横にある Python と Go のホストも同じものを出力します）は、合計 700 セントの注文に 650 セントの支払いを含む七つのイベントを、生成したコードに順に渡します。

```ts
for (const event of events) {
  const r = next(order, event);
  if (r.kind === "err") {
    console.log(`${event.kind}: refused: ${r.error}`);
    continue;
  }
  order = r.value;
  console.log(`${event.kind}: ${order.kind}`);
}
```

```text
add: cart
add: cart
place: placed
pay: refused: the amount is not the total
pay: paid
ship: shipped
cancel: refused: the order is closed
```

次は、モジュールがほかの言語をつなぐ例です。

```kn
module late_payment v1
description "What a supplier claims for an invoice paid late: the day it fell due, how many days late it was paid, and the fixed sum and the interest the rule gives for them"

use dates terms from "dates/payment_terms.cal"
use rule charges from "rules/late_payment_charges.rule"

record Invoice
  id       : string
  invoiced : date  range >=2026-01-01 <=2027-12-31
  amount   : money[GBPc]  range >=1GBPc <=100_000_000GBPc

record Claim
  due          : date
  days_late    : number  range >=1 <=1500
  compensation : money[GBP]
  interest     : money[GBPc]

fn claim(invoice: Invoice, paid_on: date range >=2026-01-01 <=2029-12-31, reference_rate: rate[step 0.01%] range >=0% <=15%) -> Claim? total
  description "None when the invoice was paid by the day it fell due"
  let due = terms.due(invoiced: invoice.invoiced)
  let late = paid_on - due
  if late >= 1
    let c = charges(debt: invoice.amount, days_late: late, reference_rate: reference_rate)
    Claim { due: due, days_late: late, compensation: c.compensation, interest: c.interest }
  else
    none
```

請求書の期日は koyomi が、遅れた日数に対して請求できる額は規則が言い、そのあいだの日数は kaname が数えます。`paid_on - due` は −758 日から 1,431 日で、`late >= 1` の分岐の中では 1 から 1,431 になり、規則の範囲（1 から 1,500）に入ります。`ritsu check` は、三つの言語を一度に読みます。

```console
$ cd crates/kaname/examples/late_payment
$ ritsu check --lang ja
…
ok late_payment.kn: 関数 1 個、total
ritsu check: ファイル 6 個（rulec 2、koyomi 2、kaname 2）。どれも検査を通りました。言語の境目: 確かめた 2 か所、決められない 0 か所
```

モジュールの書き方、ホストと一緒の Functional Core の四つの例、検査が保証することは、[サイトのページ](https://i2y.github.io/ritsu/ja/kaname/)で順に見られます。そのほかは [kaname の README](crates/kaname/README.ja.md) にあります。

## 言語が出会うところ

九つが一つの処理系を共有しているのは、ある言語が確かめたことを、別の言語が前提にできるようにするためです。たとえば返金の規則は、支払った額より多くを求める人はいない、と決めてかかっています。

```rule
rule refund_check v1
description "Whether a refund is paid at once or reviewed. A refund never asks for more than was paid, which the rule takes for granted"

enum path = at_once | review

inputs
  paid  : number  range >=0 <=10000
  asked : number  range >=0 <=10000

constraint asked <= paid

outputs
  route : path

table pick
policy unique
| asked | -> route : path |
| <=100 | at_once         |
| >100  | review          |
```

ワークフローは、客が求めた額をそのまま渡して、この規則を呼びます。

```flow
workflow refund v1
description "Pays a refund back at once or sends it to review, as the rule decides"

use rule check from "refund_check.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:refund-check"

inputs
  order : string
  paid  : int  range >=0 <=10000

task ask_amount(order: string) -> int range >=0 <=10000
  lambda "arn:aws:lambda:us-east-1:123456789012:function:ask-amount"
  idempotent

task pay_back(order: string, amount: int range >=0 <=10000)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:pay-back"
  key

flow
  let asked = ask_amount(order: order)
  let decision = check(paid: paid, asked: asked)
  match decision.route
    at_once => pay_back(order: order, amount: asked)
    review => pass
```

どちらのファイルも、それぞれの言語の検査は通ります。合わせると通りません（この二つのファイルは、`ritsu explain E201` が出す再現のものです。再現には、同じ規則を kaname のモジュールから呼ぶファイルもあります）。

```console
$ ritsu check . --lang ja
ok refund_check.rule
refund.flow: 検査を通りました
エラー[ritsu E201]: refund.flow:21:1: 規則 check を呼ぶところで、前提 `asked <= paid` を破る値を渡すことがあります
    21 |   let decision = check(paid: paid, asked: asked)
  = `asked` は `>=0 <=10000`、`paid` は `>=0 <=10000` で、asked = 10000, paid = 0 のとき `asked <= paid` が成り立ちません
  = 規則から生成したコードは、前提を破る呼び出しを入口で受け付けません。この呼び出しは、ワークフローを走らせたときに初めて落ちます。値の範囲は、dandori がその値を入れるすべての場所から集めたものです。
  = 値を渡す前に前提を保つよう分岐するか、範囲を狭めてください（入力やタスクの結果の `range`）。
ritsu check: ファイル 2 個（rulec 1、dandori 1）。検査を通らないもの 1 個（エラー 1 件）。言語の境目: 確かめた 1 か所、決められない 0 か所
```

言語をまたぐ検査の答えは、いつも三つのどれかです。成り立つ、成り立たない例がここにある、決められない（そのときは理由を言う）。黙って通すものはありません。たとえば次のものを確かめます。

- ワークフローが規則を呼ぶすべての場所で、規則の前提が成り立つか
- koyomi の日付がとりうる日を、規則の入力の範囲にすること（`range from koyomi`）。規則が宣言した範囲に収まるか
- 規則の出力を chobo の振替の額に渡すとき、その額で振替が拒否されうる理由
- chobo の仮押さえの有効期限と、ワークフローが営業日で数える待ちの長さ
- kaname のモジュールが規則、日付、振替に渡すどの値も、相手が宣言した範囲に入るか
- rulec、dandori、chobo、kaname で一つの型になったお金と単位
- sakai の地図のコンテキストが公開する操作に、それを守るゲートの action があるか。ワークフローが呼ぶ操作を、ゲートがそのワークフローに許しているか
- yuen は表、日付、主張、タスク、関数を一つずつハッシュで固定し、sakai はコンテキストをまたぐ参照を行番号つきで確かめる

## セキュリティの検査

各言語の検査は、セキュリティの誤りも探します。ファイルに直に書いた鍵、暗号化しない通信、認証の指定が無い操作、契約が秘密と印を付けた値をワークフローが履歴に残したり外へ送ったりすることです。どれもネットワークを使わず、何も実行しません。依存の脆弱性をデータベースで調べるものではなく、プロジェクトのファイルに書いてあることを読みます。

| コード | 見つけるもの | 出るところ |
|---|---|---|
| W901 | プロバイダーが形を決めている鍵（AWS、GitHub、Slack、Stripe、OpenAI、Anthropic、Google、PEM の秘密鍵）を、ファイルに書いていること。診断は種類と接頭辞と長さだけを出し、鍵そのものは出しません | 各言語の `check`（自分のファイル）。プロジェクトの `.proto` と、`.flow`・`.rule`・地図の公表された言語が参照する文書は `ritsu check` |
| W902 | このマシンの外への、暗号化しない通信 | `dandori check`（タスクが呼ぶ URL）、`sakai check`（地図の OpenAPI と AsyncAPI の文書のサーバー） |
| W903 | 公表された言語の操作やチャネルの文書に、`security` が無いこと | `sakai check` |
| W904 | プラットフォームが実行の履歴に残す秘密の値（入力、出力、引数、結果） | `dandori check` |
| E906 | プロジェクトの外（モデルのプロバイダー、判断のモデルの API（Jev、OpenAI の Decisions API、このマシンでないサーバー）、URL だけで書いた相手、AWS のサービス）へ送る秘密の値 | `dandori check` |
| E905、W905 | 地図の外のファイルや、印を付けたコンテキストと地図の上で関係の無いコンテキストへ送る秘密の値。地図が sakai の検査を通らず、送り先が地図のどこかを決められないこと | `ritsu check` |
| W910 | ゲートの action の入力のうち、ポリシーが読み、守る操作の契約が秘密と印を付けたもの。その値は Cedar に尋ねるリクエストに入り、判断の記録に残ります | `sekisho check` |
| E907、W907 | コンテキストが公開する操作（`open host service`）を、どのゲートの action も守っていないこと。公開する操作を一つも守っていないコンテキスト | `ritsu check`（ゲートのあるプロジェクト） |
| E908、W908、W909 | ゲートがどの組み合わせでも許さない、ワークフローの呼び出し。ワークフローが許されているのに呼ばない action。拒まれることがあるか、許されるかを決められない呼び出しで、拒まれたときのエラーを宣言していないもの | `ritsu check` |

値が秘密になるのは、契約が印を付けたとき（`.proto` の `debug_redact`、OpenAPI のスキーマの `x-data-classification`・`x-sensitive-data`・`format: password`）と、`.flow` が型のあとに `secret` と書いたときです。dandori は、その値が入るすべての変数を追います。次の例は、dandori のテストにある、例 `crates/dandori/examples/payout` を変えたものです。売り手への知らせを下書きするモデルに、口座の名義を読ませています。

```console
$ cd crates/dandori/tests/fixtures/security
$ dandori check E906_payout_holder.flow --lang ja
…
エラー[E906]: E906_payout_holder.flow:41:1: タスク `draft_notice` が、秘密の値 `account.holder` をプロジェクトの外の OpenAI に送ります
    41 |   let notice = draft_notice(amount: amount, payout_id: paid.payoutId, holder: account.holder)
  = 印は ../../../examples/payout/specs/payout.proto:35 の `debug_redact = true` です。
  = 参照か、相手に要るものだけを送ってください。そこへ送ることを意図しているなら、タスクの下に `discloses holder "<理由>"` と書いてください。
…
```

意図は、それが関わるもののすぐそばに書きます。書いてあれば、検査はそれに従います。テスト用の鍵には同じ行のコメントに `ritsu: test secret`、平文の通信には `.flow` の URL を書いたところに `plaintext "<理由>"`（文書ならサーバーに `x-ritsu-plaintext`）、だれでも呼べる操作には `security: []`、自分の持つ鍵で履歴を暗号化しているワークフローには `history encrypted`（dandori が生成する Temporal のコードは、ペイロードのコーデックを要るようになります）、送ると決めたタスクには `discloses <引数> "<理由>"` と書きます。

## 人のためのページ

コードが実現すべきものを理解し、確かめたい人（業務の担当者、経理や法務、運用する人、コードをレビューする開発者）のためのページも、同じファイルから英語か日本語で作れます。書かれたことを読み、自分の知っていることと照らし合わせられます。チームで規則を承認する決まりがあるなら、その承認にもこのページが使えます。

- `rulec doc`：規則の表、ただし書き、証明できたこと（Markdown か HTML）
- `dandori doc`：ワークフローの図と、走るすべてのシナリオ
- `koyomi doc`：暦を月ごとに
- `chobo doc`：帳簿、その上限と下限、振替
- `sekisho doc`：だれが何をしてよいかを action ごとの表にし、生成する Cedar と並べる
- `kaname doc`：モジュールの、表にある判断、エフェクトごとの関数とそれぞれが失敗しうる場所、コードにある判断
- `yuen trace`：要件を出典までさかのぼり、満たすものまでたどる
- `explain <コード>`：すべての診断の説明と、そのまま走らせられる再現

## 証明

rulec の証明書は、Lean 4 で検査し直します。chobo、koyomi、dandori の芯、kaname の評価と範囲の解析、ritsu の言語をまたぐ検査には、Rust の実装の横に Lean のモデルがあり、同じ入力（数百万行）を両方に流して、答えを一つずつ突き合わせます。kaname では、範囲の解析が健全であることと、`total` と推論した関数が、宣言した範囲のどの入力でも失敗しないことも Lean で証明しています。`sorry` や自前の公理に頼る証明はありません。

## AI エージェント向け

ritsu とその言語は、AI エージェントに使ってもらうためのものです。[skills/](skills) に、十の [Agent Skills](https://agentskills.io) を置いています。[skills/ritsu](skills/ritsu) は二つ以上の言語を使うプロジェクトのためのスキルで、`ritsu check` から、ワークフローの実行とパッケージの生成までの手順、言語をまたぐ診断の読み方と直し方、残りは言語ごとのどのスキルを読むか、が入っています。残りの九つは言語ごとのスキルで、`skills/<言語>/` にあります。一覧は [skills/README.ja.md](skills/README.ja.md) にあります。入れ方は四つあります。

- **Claude Code**：ritsu のサイトがプラグインのマーケットプレイスを公開していて、プラグイン `ritsu` に十のスキルが入っています。`/plugin marketplace add https://i2y.github.io/ritsu/marketplace.json` を実行してから、`/plugin install ritsu@ritsu` を実行します。Claude Code が取ってくるのは `skills/` のフォルダーだけで、リポジトリ全体はダウンロードしません。
- **どのエージェントでも、バイナリから**：`ritsu skills install` が、プロジェクトの `.claude/skills/` に書きます。`--user` を付けると `~/.claude/skills/` に、`--dir <dir>` を付けると、ほかのエージェントがスキルを読む場所に書きます。名前を挙げると（`ritsu skills install rulec dandori`）そのスキルだけを書き、`ritsu skills list` で一覧を出します。
- **手で**：`skills/` から必要なフォルダーを、`~/.claude/skills/` か、プロジェクトの `.claude/skills/` にコピーします。
- **リリースから**：`ritsu-skills-v<版>.zip` に十のフォルダーが入っています。エージェントがスキルを読む場所に展開します。

## コマンド

```
ritsu check <ディレクトリ>        各言語の検査と、言語をまたぐ検査
ritsu run <フロー> …              規則を評価し、日付を計算し、帳簿を動かしながらワークフローを走らせる
ritsu gen <ディレクトリ> --out …  プロジェクト全体を、TypeScript、Python、Go のどれか一つのパッケージに
ritsu gen --check …               パッケージが今の状態に合っているか。--format json ならファイルごとに
ritsu explain <コード>            診断の意味と再現
ritsu skills install              Agent Skills を、エージェントが読む場所に書く
ritsu <言語> …                    各言語のコマンド（例：ritsu rulec doc fee.rule）
```

リリースのアーカイブには、`ritsu` と、それを指す九つのリンク（`rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`yuen`、`sakai`、`sekisho`、`kaname`）が入っていて、どれもその言語として動きます。`cargo install` で入るのは `ritsu` のバイナリだけなので、言語は `ritsu <言語> …` で呼ぶか、言語の名前のリンクを自分で作ります（`ln -s "$(command -v ritsu)" ~/.cargo/bin/rulec` など）。日本語にするには `--lang ja` を付けます。

## 入れ方

ソースから入れます（新しい stable の Rust が要ります）。

```
cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

一つの言語だけを入れることもできます。たとえば、ほかの言語を必要としない rulec なら次のとおりです。

```
cargo install --git https://github.com/i2y/ritsu --locked rulec
```

リリースは 0.23.0 から（rulec の番号の続き）で、リリースごとに macOS と Linux のアーカイブと、`.deb` と `.rpm` を[リリースのページ](https://github.com/i2y/ritsu/releases)に置いています。Homebrew なら `brew install i2y/tap/ritsu`、GitHub Actions なら `uses: i2y/ritsu@v0.36.0` で入ります。

0.26.0 からは、リリースに `i2y-ritsu-<バージョン>.tgz` も付きます。npm のパッケージ `@i2y/ritsu` で、中身は WebAssembly 向けに組んだ ritsu です。ネイティブのバイナリなしに Node 22 以降で動き、ネイティブのバイナリと同じコマンドと、JavaScript から呼ぶための API があります。npm のレジストリには置いていないので、リリースから入れます。

```
npm install --save-dev https://github.com/i2y/ritsu/releases/download/v0.36.0/i2y-ritsu-0.36.0.tgz
```

使い方と、ほかのプログラムを起動したりネットワークに接続したりするためにネイティブのバイナリが要るコマンドは、[Node で使う](https://i2y.github.io/ritsu/ja/node/)にあります。

## リポジトリ

- `crates/<言語>`：各言語。それぞれに README と設計の文書がある：
  [rulec](crates/rulec/README.md)、[dandori](crates/dandori/README.md)、
  [koyomi](crates/koyomi/README.ja.md)、[chobo](crates/chobo/README.ja.md)、
  [geas](crates/geas/README.ja.md)、[yuen](crates/yuen/README.ja.md)、
  [sakai](crates/sakai/README.ja.md)、[sekisho](crates/sekisho/README.ja.md)、
  [kaname](crates/kaname/README.ja.md)（rulec と dandori は英語だけ）
- `crates/ritsu-*`：共通の土台、単位、口、プロジェクトの読み込み、言語をまたぐ検査、`.proto` の読み手、生成の共通部分、ブラウザ向けのビルド
- `proofs/`：Lean のモデル
- `website/`：サイト（ritsu のページ、ブラウザで試すページ、`website/rulec` に置いた rulec のサイト、`website/dandori` に置いた dandori のサイト、`website/chobo` に置いた chobo のサイト）
- `DESIGN.md`、`PLAN.md`：全体の設計（日本語）
- `SECURITY.md`：脆弱性の知らせ方

## ライセンス

MIT OR Apache-2.0 です（[LICENSE-MIT](LICENSE-MIT)、[LICENSE-APACHE](LICENSE-APACHE)）。三つの言語は、他者のデータを含みます。rulec の組み込みの区分の名前は Unicode CLDR から取ったもの（Unicode-3.0）、chobo の、Rust の名前に使えない字の表は Unicode Character Database から作ったもの（Unicode-3.0）、koyomi の Shift_JIS の変換表は WHATWG の Encoding Standard から作ったもの（BSD-3-Clause）です。そのため `ritsu` のバイナリのライセンスは `(MIT OR Apache-2.0) AND Unicode-3.0 AND BSD-3-Clause` です。バイナリには crates.io のクレートも 8 個入っていて、ライセンスはそれぞれのクレートのものです。`explain` の例のために、e-Gov から取った民法の条文（PDL1.0）と、eCFR から取った米国の連邦規則集（CFR）の二つの節のコピーも入っています。それぞれの出どころと、ライセンスの文または利用の条件は [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES) にあり、このファイルはリリースのアーカイブとパッケージのどれにも入っています。
