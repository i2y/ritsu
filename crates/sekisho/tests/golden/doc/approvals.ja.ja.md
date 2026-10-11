<!-- sekisho <version> が approvals.ja.gate（sha256:1bbfd58bd540c962）から生成したページです。読むためのもので、もとになるのは .gate のほうです。ここを編集しても .gate には戻せません。 -->
# 承認 v1

購入を、だれが承認し、だれが代わりに上の承認者に承認を求めるか。事務所の決まりの一例として書いた、別の action の答えを条件にする sekisho の例で、approvals.gate の日本語の版

- ファイル：`approvals.ja.gate`（gate 承認 v1、sha256:1bbfd58bd540c962）
- Cedar の名前空間：`OfficeJa`
- sekisho：<version>

上のファイルと、それが読むファイルを sekisho <version> で検査して作ったページです。ファイルのハッシュが今のものと違えば、このページは古くなっています。

このファイルから `sekisho gen --target cedar --lang ja` が書く Cedar：

| ファイル | SHA-256（先頭 16 桁） |
|---|---|
| `cedar/approvals_ja.cedar` | `<sha256>` |
| `cedar/approvals_ja.cedarschema` | `<sha256>` |
| `cedar/approvals_ja.cedarschema.json` | `<sha256>` |
| `cedar/approvals_ja.policies.json` | `<sha256>` |

> [!NOTE]
> `sekisho check` は、3 つの action の組み合わせ 132 通りをすべて数え、どれも Cedar と同じ決まりで、許すか拒むかを決めました。4 つの期待は、どれも成り立ちます。`can` を書いた 2 つの役割は、どれも、`can` に並べた action だけを許されます。

## だれが何をできるか

action ごとに、`sekisho check` が数えた組み合わせを、Cedar の答えと決めたポリシーが同じものどうしでまとめた表です。一つの列だけが違う行は一つにまとめ、その列にはまとめた値を書きます。その列がとるすべての値をまとめたときは「どれでも」、一つを除くすべてのときは「〜以外」と書きます。「-」は、その行には当てはまらない列です（ワークフローの役割など）。

### action `購入を見る`（`view_purchase`）

購入を見る

- どの操作も守りません。
- principal の型：`利用者`。resource の型：`購入`（ID は操作の引数 `purchaseId` から読む）
- 組み合わせは 4 通りで、そのうち 3 通りを許します。

#### 許す組み合わせ

| principal | 職員 | 承認者 | 決めたポリシー |
|---|---|---|---|
| 利用者 | はい | どれでも | 職員は購入を見られる |

#### 拒む組み合わせ

- 1 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（1 行）</summary>

| principal | 職員 | 承認者 | 決めたポリシー |
|---|---|---|---|
| 利用者 | いいえ | いいえ | 当てはまる permit なし |

</details>

### action `承認する`（`approve`）

購入を承認する

- どの操作も守りません。
- principal の型：`利用者`。resource の型：`購入`（ID は操作の引数 `purchaseId` から読む）
- input `金額`（`amount`）：`money[GBP, incl_tax]`、1GBP〜20,000GBP
- 計算した値：`区分`（下の「計算した値」）
- 組み合わせは 64 通りで、そのうち 2 通りを許します。

#### 許す組み合わせ

| principal | 職員 | 承認者 | 状態 | resource.求めた人 is principal | 区分 | 決めたポリシー |
|---|---|---|---|---|---|---|
| 利用者 | はい | はい | 提出済 | いいえ | 上限まで | 承認者は上限まで承認できる |

#### 拒む組み合わせ

- 62 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（4 行）</summary>

| principal | 職員 | 承認者 | 状態 | resource.求めた人 is principal | 区分 | 決めたポリシー |
|---|---|---|---|---|---|---|
| 利用者 | はい | はい | どれでも | はい | 上限まで | 当てはまる permit なし |
| 利用者 | はい | はい | どれでも | どれでも | 上限超え | 当てはまる permit なし |
| 利用者 | はい | はい | 提出済以外 | いいえ | 上限まで | 当てはまる permit なし |
| 利用者 | どれでも | いいえ | どれでも | どれでも | どれでも | 当てはまる permit なし |

</details>

### action `承認を求める`（`request_approval`）

上の承認者に、購入の承認を求める

- どの操作も守りません。
- principal の型：`利用者`。resource の型：`購入`（ID は操作の引数 `purchaseId` から読む）
- input `金額`（`amount`）：`money[GBP, incl_tax]`、1GBP〜20,000GBP
- 計算した値：`区分`（下の「計算した値」）
- `allowed 承認する(金額: 金額)` を読みます。同じ principal と resource で `承認する` を尋ねたら許されるかで、値はこのページの `承認する` の表のとおりです（下の「計算した値」）。
- 組み合わせは 64 通りで、そのうち 6 通りを許します。

#### 許す組み合わせ

| principal | 職員 | 承認者 | 状態 | resource.求めた人 is principal | 区分 | allowed 承認する | 決めたポリシー |
|---|---|---|---|---|---|---|---|
| 利用者 | はい | はい | 提出済 | はい | 上限まで | いいえ | 承認者は承認できないものの承認を求める |
| 利用者 | はい | はい | 提出済 | どれでも | 上限超え | いいえ | 承認者は承認できないものの承認を求める |

#### 拒む組み合わせ

- 58 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（3 行）</summary>

| principal | 職員 | 承認者 | 状態 | resource.求めた人 is principal | 区分 | allowed 承認する | 決めたポリシー |
|---|---|---|---|---|---|---|---|
| 利用者 | はい | はい | 提出済以外 | どれでも | どれでも | いいえ | 当てはまる permit なし |
| 利用者 | はい | はい | 提出済 | いいえ | 上限まで | はい | 当てはまる permit なし |
| 利用者 | どれでも | いいえ | どれでも | どれでも | どれでも | いいえ | 当てはまる permit なし |

</details>

## ポリシー

ポリシーごとに、`.gate` に書いた行と、`sekisho gen --target cedar` が生成する Cedar を並べます。permit は、当てはまる組み合わせを許します。forbid は、当てはまる組み合わせを拒み、permit も当てはまるときは forbid が勝ちます。どのポリシーも当てはまらない組み合わせは、Cedar が拒みます。規則の答えを比べる条件は、`.gate` では `.rule` に書いた値の名前（`上限まで`）で、Cedar では Cedar に渡す文字列（`"within_limit"`）で書きます。

### permit `職員は購入を見られる`（`staff_view_purchases`）

職員はどの購入も見られる

- `購入を見る` では 3 通りを許します。どれも、ほかの permit は許しません。

`approvals.ja.gate`

```gate
permit 職員は購入を見られる(staff_view_purchases)
  description "職員はどの購入も見られる"
  principal in 職員
  action 購入を見る
```

`cedar/approvals_ja.cedar`

```cedar
@id("approvals_ja/staff_view_purchases")
@name("職員は購入を見られる")
@doc("職員はどの購入も見られる")
permit (
  principal in OfficeJa::Role::"employee",
  action == OfficeJa::Action::"view_purchase",
  resource is OfficeJa::Purchase
);
```

### permit `承認者は上限まで承認できる`（`approvers_approve_within_their_limit`）

承認者は、ほかの人が求めた提出済の購入を、自分の上限まで承認できる

- `承認する` では 2 通りを許します。どれも、ほかの permit は許しません。

`approvals.ja.gate`

```gate
permit 承認者は上限まで承認できる(approvers_approve_within_their_limit)
  description "承認者は、ほかの人が求めた提出済の購入を、自分の上限まで承認できる"
  principal in 承認者
  action 承認する
  when resource.状態 is 提出済
  when 区分 is 上限まで
  unless resource.求めた人 is principal
```

`cedar/approvals_ja.cedar`

```cedar
@id("approvals_ja/approvers_approve_within_their_limit")
@name("承認者は上限まで承認できる")
@doc("承認者は、ほかの人が求めた提出済の購入を、自分の上限まで承認できる")
permit (
  principal in OfficeJa::Role::"approver",
  action == OfficeJa::Action::"approve",
  resource is OfficeJa::Purchase
)
when { resource.status == "submitted" }
when { context.band == "within_limit" }
unless { resource.requester == principal };
```

### permit `承認者は承認できないものの承認を求める`（`approvers_ask_for_what_they_cannot_approve`）

承認者は、自分で承認できないときだけ、提出済の購入の承認を上の承認者に求められる

- `承認を求める` では 6 通りを許します。どれも、ほかの permit は許しません。

`approvals.ja.gate`

```gate
permit 承認者は承認できないものの承認を求める(approvers_ask_for_what_they_cannot_approve)
  description "承認者は、自分で承認できないときだけ、提出済の購入の承認を上の承認者に求められる"
  principal in 承認者
  action 承認を求める
  when resource.状態 is 提出済
  unless allowed 承認する
```

`cedar/approvals_ja.cedar`

```cedar
@id("approvals_ja/approvers_ask_for_what_they_cannot_approve")
@name("承認者は承認できないものの承認を求める")
@doc("承認者は、自分で承認できないときだけ、提出済の購入の承認を上の承認者に求められる")
permit (
  principal in OfficeJa::Role::"approver",
  action == OfficeJa::Action::"request_approval",
  resource is OfficeJa::Purchase
)
when { resource.status == "submitted" }
unless { context.allowed_approve };
```

## 計算した値

action が計算して、Cedar の context に入れる値です。生成したコードが、サービスのデータ、操作の引数、サーバーの時計から計算し、呼ぶ側からは受け取りません。規則と日付のページは、rulec と koyomi が描いた、人が読むページをそのまま載せています。

### `承認する` の `区分`（`band`）

- 書き方：`区分 = 承認の上限(金額: 金額, 上限: principal.承認できる額).区分`
- `rulec "rules/承認の上限.rule" output 区分` が、`金額` と `principal.承認できる額` から計算します。
- 値：`上限まで`、`上限超え`。Cedar に渡す文字列は `"within_limit"`、`"over_limit"` です。

<details>
<summary><code>rulec doc</code> が描いた <code>rules/承認の上限.rule</code> のページ</summary>

[the page rulec doc draws of rules/承認の上限.rule]

</details>

### `承認を求める` の `区分`（`band`）

- 書き方：`区分 = 承認の上限(金額: 金額, 上限: principal.承認できる額).区分`
- `rulec "rules/承認の上限.rule" output 区分` が、`金額` と `principal.承認できる額` から計算します。
- 値：`上限まで`、`上限超え`。Cedar に渡す文字列は `"within_limit"`、`"over_limit"` です。

このファイルのページは、上の `区分` のところに載せています。

### `承認を求める` の `allowed 承認する`（`allowed_approve`）

- 書き方：`allowed 承認する(金額: 金額)`
- 同じ principal と resource、同じ日で `承認する` を尋ねたら許されるかです。生成したコードは、`承認する` の関数で先に尋ね、答えを context に入れてから、この action を尋ねます。`承認する` を尋ねて拒まれた理由は、そのままこのリクエストを拒む理由になります。
- `承認する` の入力に渡すもの：`金額` ← `金額`
- 値：はい、いいえ（Cedar では true と false）。
- 値は、このページの `承認する` の表のとおりです。

## 期待、職務の分離、役割

### 期待

- `承認できるものの承認はだれも求めない`（`承認を求める` で拒むことを期待）：自分で承認できる購入の承認は、だれも求めない。この期待が選ぶ 2 通りのすべてで成り立ちます。
- `承認者は上限を超えるものの承認を求める`（`承認を求める` で許すことを期待）：承認者は、自分の上限を超える提出済の購入の承認を求められる。この期待が選ぶ 4 通りのすべてで成り立ちます。
- `承認者は自分の購入の承認を求める`（`承認を求める` で許すことを期待）：承認者は、自分が求めた提出済の購入の承認を求められる。自分の購入はだれも自分で承認しない。この期待が選ぶ 4 通りのすべてで成り立ちます。
- `承認者でない職員は承認せず承認も求めない`（`承認する`、`承認を求める` で拒むことを期待）：承認者でない職員は、購入を承認せず、承認も求めない。この期待が選ぶ 32 通りのすべてで成り立ちます。

### 役割

- `職員`（`employee`）：仕事に要るものを求め、購入を見る。`can` に `購入を見る` を並べています。
- `承認者`（`approver`）：ほかの人が求めた購入を、自分の上限まで承認する。`職員` を含みます。`can` に `購入を見る`、`承認する`、`承認を求める` を並べています。

役割を一つだけ持つ principal（その役割が含む役割も持つ）が、action を許されるかです。「いつも許す」はほかの値のどの組み合わせでも許すこと、「組み合わせによる」は許す組み合わせと拒む組み合わせの両方があること、「許さない」はどの組み合わせでも拒むことです。「-」は、action がその型の principal を取らないことを表します。役割に `can` を書くと、いつも許すか組み合わせによる action が `can` と同じであることを `sekisho check` が確かめます。

| principal | `購入を見る` | `承認する` | `承認を求める` |
|---|---|---|---|
| `職員` を持つ `利用者` | いつも許す | 許さない | 許さない |
| `承認者` を持つ `利用者` | いつも許す | 組み合わせによる | 組み合わせによる |

## 守る操作

どの action も、契約の操作を守りません。
