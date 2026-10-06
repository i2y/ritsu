<!-- sekisho <version> が refunds.gate（sha256:678573b010ccf8c4）から生成したページです。読むためのもので、もとになるのは .gate のほうです。ここを編集しても .gate には戻せません。 -->
# refunds v1

Who may look at an order of the shop, refund it, and export the record of refunds. A sketch of a shop's own terms, written as sekisho's example

- ファイル：`refunds.gate`（gate refunds v1、sha256:678573b010ccf8c4）
- Cedar の名前空間：`Shop`
- sekisho：<version>

上のファイルと、それが読むファイルを sekisho <version> で検査して作ったページです。ファイルのハッシュが今のものと違えば、このページは古くなっています。

このファイルから `sekisho gen --target cedar --lang ja` が書く Cedar：

| ファイル | SHA-256（先頭 16 桁） |
|---|---|
| `cedar/refunds.cedar` | `3b5b9ae6484913ca` |
| `cedar/refunds.cedarschema` | `13b7d1e17e3a1af5` |
| `cedar/refunds.cedarschema.json` | `af775d867dd2b13b` |
| `cedar/refunds.policies.json` | `ee2b1291ba49c1a0` |

> [!NOTE]
> `sekisho check` は、3 つの action の組み合わせ 1,078 通りをすべて数え、どれも Cedar と同じ決まりで、許すか拒むかを決めました。3 つの期待は、どれも成り立ちます。職務の分離も成り立ちます。`can` を書いた 3 つの役割は、どれも、`can` に並べた action だけを許されます。

## だれが何をできるか

action ごとに、`sekisho check` が数えた組み合わせを、Cedar の答えと決めたポリシーが同じものどうしでまとめた表です。一つの列だけが違う行は一つにまとめ、その列にはまとめた値を書きます。その列がとるすべての値をまとめたときは「どれでも」、一つを除くすべてのときは「〜以外」と書きます。「-」は、その行には当てはまらない列です（ワークフローの役割など）。

### action `view_order`

Look at an order

- 守る操作：`openapi "api/orders.json" operation getOrder`
- principal の型：`User`、`Customer`。resource の型：`Order`（ID は操作の引数 `orderId` から読む）
- 組み合わせは 18 通りで、そのうち 8 通りを許します。

#### 許す組み合わせ

| principal | clerk | manager | auditor | suspended | resource.customer is principal | 決めたポリシー |
|---|---|---|---|---|---|---|
| User | はい | はい | はい | いいえ | - | staff_view_orders |
| User | はい | どれでも | いいえ | いいえ | - | staff_view_orders |
| User | どれでも | いいえ | はい | いいえ | - | staff_view_orders |
| Customer | - | - | - | - | はい | customers_view_their_orders |

#### 拒む組み合わせ

- `suspended_staff_do_nothing`（`principal is User`、`when principal.suspended`）は 8 通りに当てはまり、どれも拒みます。そのうち 7 通りには permit も当てはまりますが、forbid が勝ちます。
- 残る 2 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（4 行）</summary>

| principal | clerk | manager | auditor | suspended | resource.customer is principal | 決めたポリシー |
|---|---|---|---|---|---|---|
| User | はい | はい | どれでも | はい | - | suspended_staff_do_nothing |
| User | どれでも | いいえ | どれでも | はい | - | suspended_staff_do_nothing |
| User | いいえ | いいえ | いいえ | いいえ | - | 当てはまる permit なし |
| Customer | - | - | - | - | いいえ | 当てはまる permit なし |

</details>

### action `refund_order`

Refund an order, in part or in whole

- 守る操作：`openapi "api/orders.json" operation refundOrder`
- principal の型：`User`、`Workflow`。resource の型：`Order`（ID は操作の引数 `orderId` から読む）
- input `amount`：`money[GBP, incl_tax]`、1GBP〜10,000GBP
- 計算した値：`refund_band`、`in_period`、`business_day`（下の「計算した値」）
- 組み合わせは 1,056 通りで、そのうち 88 通りを許します。

#### 許す組み合わせ

| principal | clerk | manager | auditor | ワークフロー | suspended | status | amount | refund_band | in_period | business_day | 決めたポリシー |
|---|---|---|---|---|---|---|---|---|---|---|---|
| User | はい | いいえ | いいえ | - | いいえ | refunded 以外 | どれでも | within_limit | はい | どれでも | clerks_refund_within_their_limit |
| User | はい | はい | いいえ | - | いいえ | refunded 以外 | どれでも | within_limit | はい | どれでも | clerks_refund_within_their_limit、managers_refund_in_period |
| User | はい | はい | いいえ | - | いいえ | refunded 以外 | どれでも | over_limit | はい | どれでも | managers_refund_in_period |
| User | はい | はい | いいえ | - | いいえ | refunded 以外 | どれでも | どれでも | いいえ | はい | managers_refund_late_on_business_days |
| Workflow | - | - | - | returns | - | returned | <=50GBP | - | どれでも | どれでも | returns_refunds_returned_orders |

#### 拒む組み合わせ

- `no_second_refund`（`when resource.status is refunded`）は 264 通りに当てはまり、どれも拒みます。そのうち 112 通りには permit も当てはまりますが、forbid が勝ちます。
- `auditors_do_not_refund`（`principal in auditor`）は 512 通りに当てはまり、どれも拒みます。そのうち 224 通りには permit も当てはまりますが、forbid が勝ちます。
- `suspended_staff_do_nothing`（`principal is User`、`when principal.suspended`）は 512 通りに当てはまり、どれも拒みます。そのうち 224 通りには permit も当てはまりますが、forbid が勝ちます。
- 残る 128 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（21 行）</summary>

| principal | clerk | manager | auditor | ワークフロー | suspended | status | amount | refund_band | in_period | business_day | 決めたポリシー |
|---|---|---|---|---|---|---|---|---|---|---|---|
| User | はい | はい | いいえ | - | いいえ | refunded | どれでも | どれでも | どれでも | どれでも | no_second_refund |
| User | どれでも | いいえ | いいえ | - | いいえ | refunded | どれでも | どれでも | どれでも | どれでも | no_second_refund |
| Workflow | - | - | - | returns | - | refunded | どれでも | - | どれでも | どれでも | no_second_refund |
| User | はい | はい | はい | - | いいえ | refunded | どれでも | どれでも | どれでも | どれでも | no_second_refund、auditors_do_not_refund |
| User | どれでも | いいえ | はい | - | いいえ | refunded | どれでも | どれでも | どれでも | どれでも | no_second_refund、auditors_do_not_refund |
| User | はい | はい | はい | - | はい | refunded | どれでも | どれでも | どれでも | どれでも | no_second_refund、auditors_do_not_refund、suspended_staff_do_nothing |
| User | どれでも | いいえ | はい | - | はい | refunded | どれでも | どれでも | どれでも | どれでも | no_second_refund、auditors_do_not_refund、suspended_staff_do_nothing |
| User | はい | はい | いいえ | - | はい | refunded | どれでも | どれでも | どれでも | どれでも | no_second_refund、suspended_staff_do_nothing |
| User | どれでも | いいえ | いいえ | - | はい | refunded | どれでも | どれでも | どれでも | どれでも | no_second_refund、suspended_staff_do_nothing |
| User | はい | はい | はい | - | いいえ | refunded 以外 | どれでも | どれでも | どれでも | どれでも | auditors_do_not_refund |
| User | どれでも | いいえ | はい | - | いいえ | refunded 以外 | どれでも | どれでも | どれでも | どれでも | auditors_do_not_refund |
| User | はい | はい | はい | - | はい | refunded 以外 | どれでも | どれでも | どれでも | どれでも | auditors_do_not_refund、suspended_staff_do_nothing |
| User | どれでも | いいえ | はい | - | はい | refunded 以外 | どれでも | どれでも | どれでも | どれでも | auditors_do_not_refund、suspended_staff_do_nothing |
| User | はい | はい | いいえ | - | はい | refunded 以外 | どれでも | どれでも | どれでも | どれでも | suspended_staff_do_nothing |
| User | どれでも | いいえ | いいえ | - | はい | refunded 以外 | どれでも | どれでも | どれでも | どれでも | suspended_staff_do_nothing |
| User | はい | はい | いいえ | - | いいえ | refunded 以外 | どれでも | どれでも | いいえ | いいえ | 当てはまる permit なし |
| User | どれでも | いいえ | いいえ | - | いいえ | refunded 以外 | どれでも | どれでも | いいえ | どれでも | 当てはまる permit なし |
| User | どれでも | いいえ | いいえ | - | いいえ | refunded 以外 | どれでも | over_limit | はい | どれでも | 当てはまる permit なし |
| User | いいえ | いいえ | いいえ | - | いいえ | refunded 以外 | どれでも | within_limit | はい | どれでも | 当てはまる permit なし |
| Workflow | - | - | - | returns | - | paid、shipped | <=50GBP | - | どれでも | どれでも | 当てはまる permit なし |
| Workflow | - | - | - | returns | - | refunded 以外 | >50GBP | - | どれでも | どれでも | 当てはまる permit なし |

</details>

### action `export_refunds`

Export the record of refunds

- 守る操作：`openapi "api/orders.json" operation exportRefunds`
- principal の型：`User`。resource の型：`RefundRecord`
- 組み合わせは 4 通りで、そのうち 1 通りを許します。

#### 許す組み合わせ

| principal | auditor | suspended | 決めたポリシー |
|---|---|---|---|
| User | はい | いいえ | auditors_export_refunds |

#### 拒む組み合わせ

- `suspended_staff_do_nothing`（`principal is User`、`when principal.suspended`）は 2 通りに当てはまり、どれも拒みます。そのうち 1 通りには permit も当てはまりますが、forbid が勝ちます。
- 残る 1 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（2 行）</summary>

| principal | auditor | suspended | 決めたポリシー |
|---|---|---|---|
| User | どれでも | はい | suspended_staff_do_nothing |
| User | いいえ | いいえ | 当てはまる permit なし |

</details>

## ポリシー

ポリシーごとに、`.gate` に書いた行と、`sekisho gen --target cedar` が生成する Cedar を並べます。permit は、当てはまる組み合わせを許します。forbid は、当てはまる組み合わせを拒み、permit も当てはまるときは forbid が勝ちます。どのポリシーも当てはまらない組み合わせは、Cedar が拒みます。規則の答えを比べる条件は、`.gate` では `.rule` に書いた値の名前（`within_limit`）で、Cedar では Cedar に渡す文字列（`"within_limit"`）で書きます。

### permit `staff_view_orders`

The staff look at any order

- `view_order` では 7 通りを許します。どれも、ほかの permit は許しません。

`refunds.gate`

```gate
permit staff_view_orders
  description "The staff look at any order"
  principal in clerk, auditor
  action view_order
```

`cedar/refunds.cedar`

```cedar
@id("refunds/staff_view_orders")
@doc("The staff look at any order")
permit (
  principal,
  action == Shop::Action::"view_order",
  resource is Shop::Order
)
when { principal in Shop::Role::"clerk" || principal in Shop::Role::"auditor" };
```

### permit `customers_view_their_orders`

A customer looks at their own orders

- `view_order` では 1 通りを許します。ほかの permit は、それを許しません。

`refunds.gate`

```gate
permit customers_view_their_orders
  description "A customer looks at their own orders"
  principal is Customer
  action view_order
  when resource.customer is principal
```

`cedar/refunds.cedar`

```cedar
@id("refunds/customers_view_their_orders")
@doc("A customer looks at their own orders")
permit (
  principal is Shop::Customer,
  action == Shop::Action::"view_order",
  resource is Shop::Order
)
when { resource.customer == principal };
```

### permit `clerks_refund_within_their_limit`

A clerk refunds up to the clerk's own limit, while the refund period lasts

- `refund_order` では 36 通りを許します。そのうち 12 通りは、ほかの permit が許しません。

`refunds.gate`

```gate
permit clerks_refund_within_their_limit
  description "A clerk refunds up to the clerk's own limit, while the refund period lasts"
  principal in clerk
  action refund_order
  when refund_band is within_limit
  when in_period
```

`cedar/refunds.cedar`

```cedar
@id("refunds/clerks_refund_within_their_limit")
@doc("A clerk refunds up to the clerk's own limit, while the refund period lasts")
permit (
  principal in Shop::Role::"clerk",
  action == Shop::Action::"refund_order",
  resource is Shop::Order
)
when { context has refund_band && context.refund_band == "within_limit" }
when { context.in_period };
```

### permit `managers_refund_in_period`

A manager refunds any amount while the refund period lasts

- `refund_order` では 48 通りを許します。そのうち 24 通りは、ほかの permit が許しません。

`refunds.gate`

```gate
permit managers_refund_in_period
  description "A manager refunds any amount while the refund period lasts"
  principal in manager
  action refund_order
  when in_period
```

`cedar/refunds.cedar`

```cedar
@id("refunds/managers_refund_in_period")
@doc("A manager refunds any amount while the refund period lasts")
permit (
  principal in Shop::Role::"manager",
  action == Shop::Action::"refund_order",
  resource is Shop::Order
)
when { context.in_period };
```

### permit `managers_refund_late_on_business_days`

After the refund period, a manager may still refund, on a business day in England and Wales

- `refund_order` では 24 通りを許します。どれも、ほかの permit は許しません。

`refunds.gate`

```gate
permit managers_refund_late_on_business_days
  description "After the refund period, a manager may still refund, on a business day in England and Wales"
  principal in manager
  action refund_order
  unless in_period
  when business_day
```

`cedar/refunds.cedar`

```cedar
@id("refunds/managers_refund_late_on_business_days")
@doc("After the refund period, a manager may still refund, on a business day in England and Wales")
permit (
  principal in Shop::Role::"manager",
  action == Shop::Action::"refund_order",
  resource is Shop::Order
)
unless { context.in_period }
when { context.business_day };
```

### permit `returns_refunds_returned_orders`

The returns workflow refunds a returned order, up to 50 pounds

- `refund_order` では 4 通りを許します。どれも、ほかの permit は許しません。

`refunds.gate`

```gate
permit returns_refunds_returned_orders
  description "The returns workflow refunds a returned order, up to 50 pounds"
  principal is workflow returns
  action refund_order
  when resource.status is returned
  when amount <= 50GBP
```

`cedar/refunds.cedar`

```cedar
@id("refunds/returns_refunds_returned_orders")
@doc("The returns workflow refunds a returned order, up to 50 pounds")
permit (
  principal == Shop::Workflow::"returns",
  action == Shop::Action::"refund_order",
  resource is Shop::Order
)
when { resource.status == "returned" }
when { context.amount <= 50 };
```

### permit `auditors_export_refunds`

An auditor exports the record of refunds

- `export_refunds` では 1 通りを許します。ほかの permit は、それを許しません。

`refunds.gate`

```gate
permit auditors_export_refunds
  description "An auditor exports the record of refunds"
  principal in auditor
  action export_refunds
```

`cedar/refunds.cedar`

```cedar
@id("refunds/auditors_export_refunds")
@doc("An auditor exports the record of refunds")
permit (
  principal in Shop::Role::"auditor",
  action == Shop::Action::"export_refunds",
  resource is Shop::RefundRecord
);
```

### forbid `no_second_refund`

An order already refunded is not refunded again

- `refund_order` では 264 通りに当てはまり、どれも拒みます。そのうち 112 通りには permit も当てはまります。

`refunds.gate`

```gate
forbid no_second_refund
  description "An order already refunded is not refunded again"
  action refund_order
  when resource.status is refunded
```

`cedar/refunds.cedar`

```cedar
@id("refunds/no_second_refund")
@doc("An order already refunded is not refunded again")
forbid (
  principal,
  action == Shop::Action::"refund_order",
  resource is Shop::Order
)
when { resource.status == "refunded" };
```

### forbid `auditors_do_not_refund`

Whoever audits the refunds does not refund

- `refund_order` では 512 通りに当てはまり、どれも拒みます。そのうち 224 通りには permit も当てはまります。

`refunds.gate`

```gate
forbid auditors_do_not_refund
  description "Whoever audits the refunds does not refund"
  principal in auditor
  action refund_order
```

`cedar/refunds.cedar`

```cedar
@id("refunds/auditors_do_not_refund")
@doc("Whoever audits the refunds does not refund")
forbid (
  principal in Shop::Role::"auditor",
  action == Shop::Action::"refund_order",
  resource is Shop::Order
);
```

### forbid `suspended_staff_do_nothing`

A suspended member of the staff does nothing

- `view_order` では 8 通りに当てはまり、どれも拒みます。そのうち 7 通りには permit も当てはまります。
- `refund_order` では 512 通りに当てはまり、どれも拒みます。そのうち 224 通りには permit も当てはまります。
- `export_refunds` では 2 通りに当てはまり、どれも拒みます。そのうち 1 通りには permit も当てはまります。

`refunds.gate`

```gate
forbid suspended_staff_do_nothing
  description "A suspended member of the staff does nothing"
  principal is User
  action any
  when principal.suspended
```

`cedar/refunds.cedar`

```cedar
@id("refunds/suspended_staff_do_nothing")
@doc("A suspended member of the staff does nothing")
forbid (
  principal is Shop::User,
  action in
    [Shop::Action::"view_order",
     Shop::Action::"refund_order",
     Shop::Action::"export_refunds"],
  resource
)
when { principal.suspended };
```

## 計算した値

action が計算して、Cedar の context に入れる値です。生成したコードが、サービスのデータ、操作の引数、サーバーの時計から計算し、呼ぶ側からは受け取りません。規則と日付のページは、rulec と koyomi が描いた、人が読むページをそのまま載せています。

### `refund_order` の `refund_band`

- 書き方：`refund_band = refund_limit(amount: amount, limit: principal.refund_limit).band`
- `rulec "rules/refund_limit.rule" output band` が、`amount` と `principal.refund_limit` から計算します。
- 値：`within_limit`、`over_limit`。Cedar に渡す文字列は `"within_limit"`、`"over_limit"` です。
- principal の型が `User` のときだけ計算します。ほかの型のときは、context にこの値がありません。

<details>
<summary><code>rulec doc</code> が描いた <code>rules/refund_limit.rule</code> のページ</summary>

[the page rulec doc draws of rules/refund_limit.rule]

</details>

### `refund_order` の `in_period`

- 書き方：`in_period = today <= refund_terms.last_day(paid_on: resource.paid_on)`
- `today <= koyomi "dates/refund_terms.cal" date last_day` として計算します。koyomi が `resource.paid_on` から日付を求め、today がその日かそれより前なら「はい」です。
- today は、UTC からのオフセットが +00:00 の日付です（`today` の行）。
- 値：はい、いいえ（Cedar では true と false）。

<details>
<summary><code>koyomi doc</code> が描いた <code>dates/refund_terms.cal</code> のページ</summary>

[the page koyomi doc draws of dates/refund_terms.cal]

</details>

### `refund_order` の `business_day`

- 書き方：`business_day = today is open in uk`
- `today is open in koyomi "calendars/england_and_wales.cal"` として計算します。today がそのカレンダーの営業日なら「はい」です。
- today は、UTC からのオフセットが +00:00 の日付です（`today` の行）。
- 値：はい、いいえ（Cedar では true と false）。

`in_period` と `business_day` は、どちらも today を読みます。koyomi が宣言した範囲のすべての日で計算すると、値の組 4 通りのどれも起こります。

<details>
<summary><code>koyomi doc</code> が描いた <code>calendars/england_and_wales.cal</code> のページ</summary>

[the page koyomi doc draws of calendars/england_and_wales.cal]

</details>

## 期待、職務の分離、役割

### 期待

- `clerks_never_refund_over_their_limit`（`refund_order` で拒むことを期待）：A clerk who is not a manager never refunds more than the clerk's own limit. この期待が選ぶ 128 通りのすべてで成り立ちます。
- `nothing_refunded_twice`（`refund_order` で拒むことを期待）：No one refunds an order that is already refunded. この期待が選ぶ 264 通りのすべてで成り立ちます。
- `managers_refund_in_period`（`refund_order` で許すことを期待）：A manager who is not suspended and does not audit refunds an order not yet refunded, while the period lasts. この期待が選ぶ 48 通りのすべてで成り立ちます。

### 職務の分離

- `refunding_and_auditing`（`refund_order`、`export_refunds`）：The people who refund are not the people who audit the refunds. 成り立ちます。二つ以上を許される principal はいません。

### 役割

- `clerk`：Answers customers, and refunds small amounts. `can` に `view_order`、`refund_order` を並べています。
- `manager`：Refunds what a clerk cannot, and after the refund period on a business day. `clerk` を含みます。`can` に `view_order`、`refund_order` を並べています。
- `auditor`：Checks the refunds after the fact. `can` に `view_order`、`export_refunds` を並べています。

役割を一つだけ持つ principal（その役割が含む役割も持つ）が、action を許されるかです。「いつも許す」はほかの値のどの組み合わせでも許すこと、「組み合わせによる」は許す組み合わせと拒む組み合わせの両方があること、「許さない」はどの組み合わせでも拒むことです。「-」は、action がその型の principal を取らないことを表します。役割に `can` を書くと、いつも許すか組み合わせによる action が `can` と同じであることを `sekisho check` が確かめます。

| principal | `view_order` | `refund_order` | `export_refunds` |
|---|---|---|---|
| `clerk` を持つ `User` | 組み合わせによる | 組み合わせによる | 許さない |
| `manager` を持つ `User` | 組み合わせによる | 組み合わせによる | 許さない |
| `auditor` を持つ `User` | 組み合わせによる | 許さない | 組み合わせによる |
| `Customer` | 組み合わせによる | - | - |

## ワークフロー

ワークフローは、自分の資格で操作を呼びます。Cedar では principal `Workflow::"<名前>"` です。ワークフローごとに、どの action をどれだけ許されるかを並べます。

### ワークフロー `returns`

Refunds a returned order once the item is back at the warehouse

- フロー：`dandori "flows/returns.flow"`

| action | 許されるか |
|---|---|
| `refund_order` | 組み合わせによる（32 通りのうち 4 通りを許す） |

## 守る操作

action が守る契約の操作です。サービスは、操作を行う前に、その action について Cedar に尋ねます。参照のパスはルートからです。

| action | 操作 | 呼び出し方 |
|---|---|---|
| `view_order` | `openapi "api/orders.json" operation getOrder` | `GET /orders/{orderId}` |
| `refund_order` | `openapi "api/orders.json" operation refundOrder` | `POST /orders/{orderId}/refunds` |
| `export_refunds` | `openapi "api/orders.json" operation exportRefunds` | `POST /refunds/export` |

どの action も、操作を守っています。
