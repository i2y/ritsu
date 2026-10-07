---
hide:
  - toc
---

# sekisho

**だれが何をしてよいかを書く小さな言語です。業務の規則の答えを、認可の条件に使えます。sekisho はゲートを、書いた範囲のすべての組み合わせで確かめてから [Cedar](https://www.cedarpolicy.com/) のポリシーにします。リクエストごとの判断は Cedar がします。**

店の係は、自分の上限までなら注文を返金してよい、という決まりがあるとします。金額がその上限の内にあるかどうかは、業務の規則が決めることです。ゲート（`.gate` のファイル）は、その規則の答えを受け取り、permit は役割と同じようにそれを条件に使います。

```gate
use rule 返金の上限 from "rules/返金の上限.rule"
…
action 返金する(refund_order)
  …
  input
    金額(amount) : money[GBP, incl_tax]  range >=1GBP <=10_000GBP
  context
    返金の区分(refund_band)  = 返金の上限(金額: 金額, 上限: principal.返金できる額).区分
…
permit 係は上限まで返金できる(clerks_refund_within_their_limit)
  description "係は、返金の期間のあいだ、自分の上限まで返金できる"
  principal in 係
  action 返金する
  when 返金の区分 is 上限まで
  when 期間内
```

`返金の区分` は、リクエストのたびに計算する値です。ゲートは、返金しようとしている金額と、返金を頼んだ職員についてサービスが持っている上限を、規則 `返金の上限` に渡し、その出力 `区分` を受け取ります。規則は [rulec](../rulec/ja/) で書いてあります。

```rule
rule 返金の上限(refund_limit_ja) v1
…
enum 返金の区分(refund_band) = 上限まで(within_limit) | 上限超え(over_limit)

inputs
  金額(amount) : money[GBP, incl_tax]  range >=1GBP <=10_000GBP
  上限(limit)  : money[GBP, incl_tax]  range >=0GBP <=10_000GBP

outputs
  区分(band) : 返金の区分

derive 超過(excess) : money[GBP, incl_tax] = 金額 - 上限  range >=-9_999GBP <=10_000GBP

table 決める
policy unique
| 超過   | -> 区分 : 返金の区分 |
| <=0GBP | 上限まで             |
| >0GBP  | 上限超え             |
…
```

`sekisho check` は、この規則がどの答えを返しうるかを rulec に尋ね、そのすべての答えについてポリシーを確かめます。生成は、検査を通ったゲートからだけします。生成するのは、Cedar のポリシーと、rulec が生成したコードで規則の答えを計算してから Cedar に尋ねるコードです。

## なぜ Cedar を生成するのか

sekisho は、リクエストを自分では判断しません。サービスは、プロセスの中の Cedar（cedar-wasm、cedarpy、cedar-go、Rust のクレート）か、Amazon Verified Permissions に尋ね、Cedar がポリシーとスキーマとリクエストから答えを出します。sekisho が受け持つのは、そのポリシーとスキーマを書くことと、書く前に確かめることです。Cedar は標準の言語で、小さく、型があり、意味は Lean で形式化されています。sekisho が書くのは普通の Cedar で、strict の `cedar validate` を通ります。

それでも sekisho を使うのは、Cedar だけではできないことがあるからです。

- 規則と日付を条件にできます。Cedar のポリシーが読めるのは、リクエストとエンティティだけで、表を引くことも、営業日を数えることも、月を足すこともできません。そうした答えは rulec と koyomi が計算し、リクエストに入れて渡します。
- すべての組み合わせを表にできます。Cedar の解析は、二つのポリシーの集合が同じリクエストを許すか、どちらが多くを許すかを答えます。sekisho は、条件がとりうる組み合わせを全部たどり、だれが何を許されるかを action ごとの表にして、それを決める人に見せます。
- リクエストを、生成したコードが組み立てます。Cedar は、リクエストに書いてある値をそのまま信じます。sekisho が生成するコードはリクエストを自分で組み立て、ポリシーが読む値を、呼ぶ側からは受け取りません。
- プロジェクトのほかのものとつながります。action には守る契約の操作を書き、ワークフローも principal になれるので、`ritsu check` がゲートを契約、地図、ワークフローと突き合わせられます。

生成する Cedar は、言語の小さな部分だけを使います（拡張の型、算術、`like` は使いません）。どの実装でも同じ答えになるようにするためです。テストでは、例のすべての組み合わせを、公式の Cedar の CLI と、生成したコードが尋ねる三つのライブラリにかけています。

## 検査が規則に尋ねること

金額は 1〜10,000 ポンド、上限は 0〜10,000 ポンドで、検査はこれを一つずつ試すわけではありません。ポリシーが金額を区切るところで区間に分け、区間ごとに、規則がどの答えを返しうるかを rulec に尋ねます。この例では、返品のワークフローの permit が金額を 50 ポンドと比べるので、区間は二つです。職員の上限は 0〜10,000 ポンドのどれでもありうるので、どちらの区間でも「上限まで」と「上限超え」の両方が起こりえます。これに役割、属性、日付の答えを合わせると、三つの action の組み合わせは 1,078 通りになります。検査は、そのどれについても Cedar と同じ決まりで答えを決めます。どのポリシーも許さないリクエストは拒み、当てはまる forbid が一つでもあれば、permit があっても拒みます。

```console
$ cd crates/sekisho/examples/refunds
$ sekisho check refunds.ja.gate --lang ja
refunds.ja.gate: ok — action 3、ポリシー 10（permit 7、forbid 3）、期待 3、職務の分離 1
```

ゲートには、ポリシーの隣に、人が決めたことを期待として書いておけます。

```gate
expect deny 係は上限を超えて返金しない
  description "責任者でない係は、自分の上限を超えて返金することがない"
  principal in 係
  action 返金する
  unless principal in 責任者
  when 返金の区分 is 上限超え
```

たとえば、係の permit から、規則の答えを読む行を消したとします。

```diff
 permit 係は上限まで返金できる(clerks_refund_within_their_limit)
   description "係は、返金の期間のあいだ、自分の上限まで返金できる"
   principal in 係
   action 返金する
-  when 返金の区分 is 上限まで
   when 期間内
```

`sekisho check` は、上限を超えて返金できる係がいることを見つけます。

```text
警告[W301]: refunds.ja.gate:93:1: permit `責任者は期間内なら返金できる` が許す組み合わせは、どれも `係は上限まで返金できる` も許します
    93 | permit 責任者は期間内なら返金できる(managers_refund_in_period)
  = `責任者は期間内なら返金できる` は 48 通りを許し、そのどれも `係は上限まで返金できる` が許すので、`責任者は期間内なら返金できる` を消しても答えは変わりません。
  = `責任者は期間内なら返金できる` を消すか、`責任者は期間内なら返金できる` のほうが意図どおりなら `係は上限まで返金できる` を狭めてください。
エラー[E304]: refunds.ja.gate:134:1: 期待 `係は上限を超えて返金しない` が成り立ちません。選んだ 128 通りのうち 12 通りが許されます
   134 | expect deny 係は上限を超えて返金しない
  = 例：係を持つ職員（停止中：いいえ）、注文（状態：支払済）、金額：1GBP〜50GBP、返金の区分：上限超え、期間内：はい、営業日：いいえ。`係は上限まで返金できる` が許します。
  = ポリシーを直すか、期待が言いすぎているなら期待を直してください。
```

例に挙がったのは、50 ポンド以下の金額に、規則が「上限超え」と答える組み合わせです。係自身の上限がその金額より低ければ、そうなります。警告も同じ変更から出ています。責任者は係を含むので、係の permit が、責任者が期間内に返金できるという permit の許すものを、全部許すようになったからです。

どの検査も、成り立つ、成り立たない組み合わせがある、決められない（理由つき）の三つのどれかを答えます。ほかの検査は、次のものを見つけます。

- どの permit も許さない action（E301）
- 許すはずの組み合わせをどれも forbid が拒むので、何も許さない permit（E302）
- 条件が同時には成り立たないポリシー（E303）
- 一人の principal が両方を許されてしまう職務の分離（E305）
- `can` に無い action を許される役割（E306）と、`can` に書いた action を一度も許されない役割（W302）
- どの組み合わせも選ばない期待（W304）
- 規則の入力の範囲を外れる値や、規則の前提を破る値を規則に渡すこと（E206）
- rulec や koyomi の答えが起こりうるかを確かめられず、決められない問い（W303）

コードごとに、いつ出るか、どう直すか、それを出す最小のゲートを、`sekisho explain E304 --lang ja` のように引けます。

## 生成するもの

### Cedar

```console
$ cd crates/sekisho/examples/refunds
$ sekisho gen refunds.ja.gate --target cedar --out generated --lang ja
生成しました: generated/cedar/refunds_ja.cedar
生成しました: generated/cedar/refunds_ja.cedarschema
生成しました: generated/cedar/refunds_ja.cedarschema.json
生成しました: generated/cedar/refunds_ja.policies.json
```

係の permit は、Cedar ではこうなります。Cedar の中の名前は ASCII の別名で、日本語の名前は `@name` に残ります。

```cedar
@id("refunds_ja/clerks_refund_within_their_limit")
@name("係は上限まで返金できる")
@doc("係は、返金の期間のあいだ、自分の上限まで返金できる")
permit (
  principal in ShopJa::Role::"clerk",
  action == ShopJa::Action::"refund_order",
  resource is ShopJa::Order
)
when { context has refund_band && context.refund_band == "within_limit" }
when { context.in_period };
```

Cedar から見ると、規則の答えは、リクエストの context に入った文字列です。どこから来た値かは、スキーマの注釈に書いてあります。

```cedarschema
      @doc("rulec \"crates/sekisho/examples/refunds/rules/返金の上限.rule\" output 区分。… 生成したコードが計算し、呼ぶ側からは受け取らない")
      @name("返金の区分")
      refund_band?: String
```

`?` は、この値が無いこともあるという印です。この action のもう一つの principal であるワークフローには、規則に渡す上限がありません。

### Cedar に尋ねるコード

呼ぶ側が `refund_band` を送れるなら、自分の返金を自分で許せてしまいます。そこで、sekisho が TypeScript、Python、Go に生成するコードは、action ごとに、principal と注文をサービス自身のデータから読み、返金の区分を rulec が生成したコードで、日付を koyomi が生成したコードで計算し、リクエストを組み立てて Cedar に尋ねます。`ritsu gen` は、このコードを、読む規則と日付のコードと一緒に、一つのパッケージに書きます。

```console
$ cd crates/sekisho/examples/refunds
$ ritsu gen --target python --out generated --lang ja
生成しました: generated/python/generated/rules/refund_limit.py
生成しました: generated/python/generated/rules/refund_limit_ja.py
…
生成しました: generated/python/generated/dates/refund_terms_ja.py
…
生成しました: generated/python/generated/authz/refunds_ja.py
…
```

```python
from ..dates import england_and_wales as _dates_england_and_wales
from ..dates import refund_terms_ja as _dates_refund_terms_ja
from ..rules import refund_limit_ja as _rules_refund_limit_ja
…
def refund_order_request(store: Store, principal: Principal, input: RefundOrderInput, now: datetime | None = None) -> Request:
    …
    in_amount = _int('input', 'amount', input.get('amount'), 1, 10000)
    …
        p_user = _read_user(store, principal.id, ('refund_limit', 'suspended',))
    …
    # 返金の区分(refund_band)  = 返金の上限(金額: 金額, 上限: principal.返金できる額).区分
    if p_user is not None:
        try:
            v_refund_band = _rules_refund_limit_ja.refund_limit_ja(_rules_refund_limit_ja.GBPInclTax(in_amount), _rules_refund_limit_ja.GBPInclTax(p_user.refund_limit))
```

この関数は、計算した値を呼ぶ側から受け取りません。データがゲートの宣言と合わないときや、ゲートを確かめた範囲の外の日には、Cedar に尋ねる前に拒みます。Python のコードはプロセスの中の cedarpy に、TypeScript のコードは cedar-wasm に、Go のコードは cedar-go に尋ねます。`--authorizer avp` を付ければ、どれも Amazon Verified Permissions に尋ねます。

## 人が読むページ

`sekisho doc` は、だれが何をしてよいかを決める人のためのページを書きます。Markdown はプルリクエストに貼るためのもので、`--format html` を付ければ一つの HTML ファイルになります。action ごとに、許す組み合わせと拒む組み合わせの表と、それを決めたポリシーが並び、規則の答えも、役割と同じく一つの列になります。次の表は、例のページのうち、返金を許す組み合わせです（「どれでも」はその列のすべての値、「-」はその行に当てはまらない列です）。

| principal | 係 | 責任者 | 監査 | ワークフロー | 停止中 | 状態 | 金額 | 返金の区分 | 期間内 | 営業日 | 決めたポリシー |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 職員 | はい | いいえ | いいえ | - | いいえ | 返金済以外 | どれでも | 上限まで | はい | どれでも | 係は上限まで返金できる |
| 職員 | はい | はい | いいえ | - | いいえ | 返金済以外 | どれでも | 上限まで | はい | どれでも | 係は上限まで返金できる、責任者は期間内なら返金できる |
| 職員 | はい | はい | いいえ | - | いいえ | 返金済以外 | どれでも | 上限超え | はい | どれでも | 責任者は期間内なら返金できる |
| 職員 | はい | はい | いいえ | - | いいえ | 返金済以外 | どれでも | どれでも | いいえ | はい | 責任者は期間後も営業日なら返金できる |
| Workflow | - | - | - | 返品 | - | 返品済 | <=50GBP | - | どれでも | どれでも | 返品のワークフローは返品済の注文を返金できる |

ページには、ほかに、ポリシーごとの `.gate` の行と生成した Cedar、条件が読む規則と日付について rulec と koyomi が描いたページも入ります。規則と日付のページを除いた[例のページ](https://github.com/i2y/ritsu/blob/main/crates/sekisho/tests/golden/doc/refunds.ja.ja.md)を、リポジトリで読めます。

## ほかの言語と組む

同じ action は、koyomi の日付のファイルとカレンダー、それに契約も読みます。また、この action が守る操作を、dandori のワークフローが呼びます。

```gate
use dates 返金の期限 from "dates/返金の期限.cal"
use calendar 英国 from "calendars/england_and_wales.cal"
use openapi 注文 from "api/orders.json"
…
today range >=2026-10-01 <=2028-10-31 offset +00:00
…
action 返金する(refund_order)
  description "注文の全部か一部を返金する"
  guards 注文 refundOrder
  …
    期間内(in_period)        = today <= 返金の期限.最終日(支払日: resource.支払日)
    営業日(business_day)     = today is open in 英国
```

### 日付

`期間内` は、今日が返金の期間の最終日以前かどうかです。最終日は、koyomi の日付のファイルが、注文の支払日から計算します。支払日の 30 日後で、その日がイングランドとウェールズで休みなら、次の営業日です。`営業日` は、今日が営業日かどうかです。土日と、GOV.UK が公開しているイングランドとウェールズの祝日を休みとする、koyomi のカレンダーで決まります。検査は、`today` の範囲のすべての日について、どの答えが同じ日にそろいうるかを koyomi に尋ねます。ゲートに問い合わせうる日のデータを持たないカレンダーは、エラーになります（E207）。今日の日付はサーバーの時計から取り、生成したコードは、範囲の外の日を受け付けません。

### 契約

`guards 注文 refundOrder` は、この action が守る操作を、OpenAPI の文書 `api/orders.json` の操作で書いたものです。検査は、その操作が文書にあること（E202）、input が操作の受け取るものと合うこと（E203）、注文の ID を読む引数 `orderId` が操作にあること（E204）を確かめます。

### ワークフロー

ワークフロー `返品` は、返品された注文を、同じ操作で、自分の資格で返金します。

```flow
task refund_order(orderId: string, amount: money[GBP, incl_tax] range >=1 <=10000) -> Refund
  http POST orders "/orders/{orderId}/refunds"
  errors denied = 403
  key
```

```gate
permit 返品のワークフローは返品済の注文を返金できる(returns_refunds_returned_orders)
  description "返品のワークフローは、返品済の注文を 50 ポンドまで返金できる"
  principal is workflow 返品
  action 返金する
  when resource.状態 is 返品済
  when 金額 <= 50GBP
```

`ritsu check` は、フローが操作を呼ぶところごとに、ゲートがそれをワークフローに許しているかを確かめます。どの組み合わせでも許されない呼び出しはエラー（E908）です。拒まれることがある呼び出しで、タスクが拒まれたときのエラーを宣言していなければ警告（W909）、ワークフローに許しているのに呼ばない action も警告（W908）です。この例のタスクは、拒まれたときのエラー `denied` を宣言しています。呼び出しは一つですが、英語の版と日本語の版のゲートがそれぞれ確かめるので、言語の境目は 2 か所と数えます。

```console
$ cd crates/sekisho/examples/refunds
$ ritsu check . --lang ja
ok rules/refund_limit.rule
ok rules/返金の上限.rule
calendars/england_and_wales.cal: ok — カレンダー england_and_wales。表 bank_holidays 83 行、データの範囲 2019-01-01〜2028-12-31
dates/refund_terms.cal: ok — 3 つの条件が、paid_on 2026-01-01〜2028-11-25 の 1,060 日のすべてで成り立ちます。例 2 行も合っています
dates/返金の期限.cal: ok — 3 つの条件が、支払日 2026-01-01〜2028-11-25 の 1,060 日のすべてで成り立ちます。例 2 行も合っています
flows/returns.flow: 検査を通りました
refunds.gate: ok — action 3、ポリシー 10（permit 7、forbid 3）、期待 3、職務の分離 1
refunds.ja.gate: ok — action 3、ポリシー 10（permit 7、forbid 3）、期待 3、職務の分離 1
ritsu check: ファイル 8 個（rulec 2、koyomi 3、dandori 1、sekisho 2）。どれも検査を通りました。言語の境目: 確かめた 2 か所、決められない 0 か所
```

### 地図

sakai の地図で、あるコンテキストがほかのコンテキストに操作を公開している（`open host service`）とき、`ritsu check` は、その操作を守る action がどこかのゲートにあるかを確かめます。だれでも呼べると契約に書いた操作は除きます。コンテキストのほかの操作には守る action があるのに、守る action の無い操作があればエラー（E907）、コンテキストのどの操作にもまだ守る action が無ければ警告（W907）です。この例には地図がありませんが、ブラウザで試すページの通販には地図があります。

## 手で書いた Cedar

ritsu は、人が手で書いた Cedar も読みます。対象は、ポリシーのファイルとその横のスキーマ（`refunds.cedar` と `refunds.cedarschema`）の組で、地図の `owns` か yuen の要件が `cedar "…"` で指し、どの action がどの操作を守るかをスキーマの `@guards` に書いてあるものです。その action は、ゲートの action と同じく、地図が公開する操作と突き合わせます。sekisho は、自分の条件と同じ Cedar の部分（スコープ、役割、属性を定数と比べる条件、`has` など）でできたポリシーを数え、テストでは、数えたすべての組み合わせを公式の Cedar の CLI の答えと突き合わせています。その部分を超える式（`like`、算術、集合、拡張の関数）があれば、決められないと答え、どのポリシーのどの部分が理由かを言います。ワークフローの呼び出しを確かめる相手は、ゲートだけです。ワークフローがどのフローなのかを、Cedar では書けないからです。

## ブラウザで試す

[ブラウザで試すページ](playground/#project=sekisho/refunds.ja)で、この例を開けます。ゲートで「生成」を押すと Cedar が出て、「人が読むページ」も開けます。[小さな通販](playground/#project=shop.ja&file=gates/注文.gate)には、ゲートが二つあります。`gates/注文.gate` は、注文をそもそも取り消せるかを、ワークフローも従う規則に尋ねます。`gates/倉庫.gate` は、ワークフローが呼ぶ倉庫の操作を守ります。通販の地図で、受注と倉庫がほかのコンテキストに公開している操作は、どれも二つのゲートのどちらかの action が守っています。

## もっと読む

- [sekisho の README](https://github.com/i2y/ritsu/blob/main/crates/sekisho/README.ja.md)：言語のあらまし、コマンド、どう確かめているか
- [リファレンス](https://github.com/i2y/ritsu/blob/main/crates/sekisho/docs/reference.md)（英語）：言語の全部、`check` が言うこと、`gen` が書くもの
- [診断の一覧](https://github.com/i2y/ritsu/blob/main/crates/sekisho/docs/codes.ja.md)：コードごとに、いつ出るか、どう直すか、それを出す最小のゲート（`sekisho explain` でも引けます）
- [DESIGN.md](https://github.com/i2y/ritsu/blob/main/crates/sekisho/DESIGN.md)：なぜ Cedar を生成するのか、ほかの言語の答えをどう Cedar に届けるか、まだやっていないこと
- [スキル](https://github.com/i2y/ritsu/blob/main/skills/sekisho/SKILL.md)：ゲートを書いたり直したりするエージェントのためのもの
