# ほかの言語と組み合わせる

帳簿はそれだけで完結していて、chobo はほかの言語のファイルを読みません。ritsu のプロジェクトでは、ほかの言語のファイルのほうが帳簿を使います。dandori のワークフローは帳簿の操作をタスクとして走らせ、仮押さえを案件として追います。rulec の規則は、ワークフロー（や手で書いたコード）が帳簿に渡す額を計算します。sekisho のゲートは、操作ごとにだれが走らせてよいかを決めます。yuen の要件は、それを満たす操作を挙げます。sakai の地図は、どのコンテキストが帳簿を持つかを言います。`ritsu check` はそれらを一緒に読み、言語の境目で食い違いが無いかを確かめます。

## ワークフローが操作を走らせる

dandori の請求の例は、注文の品を支払期日まで押さえ、支払われたら出荷し、支払われなければ棚に戻します
（[crates/dandori/examples/invoice/invoice.ja.flow](https://github.com/i2y/ritsu/blob/main/crates/dandori/examples/invoice/invoice.ja.flow)）。在庫は在庫の引当の例と同じような帳簿で、仮押さえの有効期限は 60 日です。

```flow
use book 在庫 from "books/在庫.book"
…
task 引き当てる(注文: string, sku: string, 数: int) -> 在庫.引当
  book 在庫.引当.hold
  starts 在庫.引当
  errors 在庫切れ

task 出荷する(注文: string, sku: string) -> 在庫.引当
  book 在庫.引当.post
  sends post
  errors expired

task 戻す(注文: string, sku: string) -> 在庫.引当
  book 在庫.引当.void
  sends void
  errors expired, already_posted
…
case 押さえ : 在庫.引当 follows 在庫.引当

flow
  押さえ <- 引き当てる(注文: 受注.id, sku: 受注.sku, 数: 受注.数)
    on 在庫切れ => succeed 結果 = 在庫切れ, 期日 = none
```

- `book <帳簿>.<振替>.<操作>` と書くと、タスクがその帳簿の操作（`do`、または `hold`・`post`・`void`）になります。タスクの引数は、振替の引数を名前のとおりに書きます。額の型は、[お金と単位](units.md#ほかの言語から見た単位)の表のとおりの帳簿の単位です。
- 帳簿がその操作を拒否しうる理由が、そのままタスクのエラーになります（`errors 在庫切れ`）。帳簿がその操作で返しえない理由を書くと、ワークフローの検査でエラーになります。タスクが宣言していない理由で拒否されると、呼び出しは失敗します。
- 操作はキーごとに一度しか通らないので、タスクに冪等のキーは書きません。もう一度呼ぶと、前に済んでいるという結果（`done_before`）が返り、ワークフローはそれを通ったものとして読むので、リトライしても安全です。
- `case 押さえ : 在庫.引当 follows 在庫.引当` は、仮押さえを、そのライフサイクルのステートマシンに沿って追います。このステートマシンは `chobo api` も出します。仮押さえはひとりでに期限が切れうるので、dandori の検査は、どの確定と取消の前にも、期限が切れている場合を考えに入れます。品を押さえたまま終わりうるワークフローはエラーです。

dandori は、規則を読むのと同じく、帳簿を ritsu を通して読みます。`ritsu dandori` と `ritsu check` が、chobo が帳簿について知っていることを dandori に渡します。dandori が書くコードは、どのプラットフォーム向けでも、`chobo build` が書くクライアントを呼びます。くわしくは dandori の[日付と帳簿](../../dandori/ja/dates-and-books/)にあります。

## ワークフローを通さずに規則と帳簿を使う

帳簿を呼ぶのに、いつもワークフローが要るわけではありません。決済なら、受けた一つのリクエストの中で額を計算し、売上を記帳すれば済みます。`ritsu gen` は、規則のモジュールと帳簿のクライアントを一つのパッケージに書きます。その中では、お金と量の額は単位ごとに一つの型で、規則が返す額と帳簿の操作が受け取る額は同じ型です。ritsu のテストのマーケットプレイスでは、規則が売上をセントで分け、手で書いた決済のコードが、規則の返す額を帳簿に渡します
（[crates/ritsu/tests/projects/marketplace](https://github.com/i2y/ritsu/tree/main/crates/ritsu/tests/projects/marketplace)）。

```ts
import { books, rules, units } from "../generated/typescript/index";
…
export async function checkout(book: books.marketplace.Book, order: string, shop: string, plan: rules.sale_split.Plan, paid: units.USDc): Promise<books.marketplace.Result> {
  const split = rules.sale_split.sale_split(plan, paid);
  return book.sale.do({ order, shop, to_shop: split.to_shop, fee: split.fee });
}
```

規則の `fee` と `to_shop` は `units.USDc` で、帳簿（`unit USD scale 2`）の `sale` が受け取る額も同じ型です。二つのあいだにあるのは ritsu の言語ではないコードなので、`ritsu check` にはこの境目が見えません。代わりに、そのコードの言語の型の検査が確かめます。規則がドル（`money[USD]`）で数えていれば、tsc は、ドルが帳簿のセントに渡るところで決済のコードを止めます。

<div class="cb-term" markdown>

```text
service/checkout.ts(10,51): error TS2345: Argument of type 'USDc' is not assignable to parameter of type 'USD'.
  Type 'USDc' is not assignable to type '{ readonly __rulec: "USD"; }'.
    Types of property '__rulec' are incompatible.
      Type '"USDc"' is not assignable to type '"USD"'.
service/checkout.ts(11,38): error TS2322: Type 'USD' is not assignable to type 'USDc'.
  Type 'USD' is not assignable to type '{ readonly __rulec: "USDc"; }'.
    Types of property '__rulec' are incompatible.
      Type '"USD"' is not assignable to type '"USDc"'.
service/checkout.ts(11,62): error TS2322: Type 'USD' is not assignable to type 'USDc'.
  Type 'USD' is not assignable to type '{ readonly __rulec: "USDc"; }'.
    Types of property '__rulec' are incompatible.
      Type '"USD"' is not assignable to type '"USDc"'.
```

</div>

Python の決済のコードは mypy が同じように止め（`Argument "to_shop" to "do" of "_saleTransfer" has incompatible type "USD"; expected "USDc"`）、Go の決済のコードは go vet が止めます。数え方の単位（`個`、`席`）の額は、どこでもそうであるように素の整数で、クライアントが読む残高も素の整数です。ワークフローの中なら、同じ額を `ritsu check` が自分で確かめ、tsc には見えないことも確かめます。額の範囲と、その額で起きうる拒否です（次の節）。

## 言語の境目で確かめること

言語をまたぐ検査は、成り立つと言うか、成り立たない例を見つけるか、決められないと理由つきで言います。帳簿にかかわるものは三つあります。規則が振替に渡す額、その額で振替が拒否されうる理由、確定するまでの待ちと仮押さえの有効期限です。下の例のファイルは、`ritsu explain <コード>` が出すプロジェクトのものです。

### タスクが処理しない拒否

規則がイベントに要る席の数を計算し、コンサートなら 400 席までになります。ワークフローはその数を、ホールの帳簿に渡します。ホールの帳簿では、割り当てた席の勘定の上限が 300 です。額を規則が返しうる範囲に限った chobo の探索が、振替が拒否されうることを見つけます。

<div class="cb-term" markdown>

```text
エラー[ritsu E204]: booking.flow:18:1: 帳簿は `assign.do` を `over_capacity` で拒否することがありますが、タスク `give_seats` はそれを処理していません
    18 |   give_seats(event: event, count: need.needed)
  = 額を 30 から 400 まで（呼び出しが渡す額の範囲）に限った chobo の探索で、`assign.do` が `over_capacity` で拒否される例が見つかりました
  = タスクのエラーとして宣言し、処理してください（`errors over_capacity` と `on over_capacity =>`）。
```

</div>

### chobo が受け取らない額

規則の別のバージョンでは、返却で 20 席を戻すことを −20 で表しています。chobo が受け取る額は 0 から 2⁶³ − 1 までで、それ以外の額の呼び出しは、拒否されるのではなく失敗します。

<div class="cb-term" markdown>

```text
エラー[ritsu E203]: booking.flow:18:1: タスク `give_seats` が `count` に渡す額が -20 になることがあり、chobo はその額を受け取りません
    18 |   give_seats(event: event, count: need.needed)
  = `need.needed` は規則 "seats.rule" の出力 `needed` で、event_kind = handback のとき -20 になります
  = chobo が受け取る額は、単位のいちばん小さい刻みで 0 から 9223372036854775807 までです。それ以外の額の呼び出しは、拒否されるのではなく失敗します。
  = 規則の返す額を 0 以上にするか（負の額は、向きの違う振替に分けてください）、振替の前に分岐してください。
```

</div>

### いつも期限が切れている仮押さえ

ワークフローが品を 14 日押さえ、koyomi が数える支払日まで待ってから確定します。支払日は 18 日以上あとです。

<div class="cb-term" markdown>

```text
エラー[ritsu E206]: invoice.flow:31:1: `ship` が案件 `goods` の仮押さえを確定するときには、期限が必ず切れています
    31 |   goods <- ship(order: order, sku: sku)
  = `ship` は、仮押さえを作ってから（27 行目）早くても 17 日 9 時間あとに来ます。仮押さえは作ってから 14 日で期限が切れます（"stock.book" の `reserve`）
  = 30 行目の `wait until due.at` は、koyomi の日付 `payment` の時刻まで待ちます。その日は `received` から早くて 18 日後（2026-02-20 なら 2026-03-10）の 09:00 で、日付に渡した `now` は仮押さえのあと（29 行目）に読んでいます
  = 帳簿はどの実行でもこれを `expired` で拒否するので、`ship` が通ったあとの流れは動きません。
  = 仮押さえの期限を延ばすか（振替の `pending expires after`）、もっと早く呼んでください。
```

</div>

規則の額がどれもホールに収まるなら、どの実行も `over_capacity` には至りません。それでもタスクがこの理由を宣言していると、何も言わずに通すのではなく、警告を出します。chobo の探索がたどるのは chobo の検査と同じ深さまでなので、言えるのは、その深さまでは拒否が起きないということだけだからです。

## 帳簿の入った実行

`ritsu run` は、規則を rulec に、日付を koyomi に計算させ、帳簿の操作を chobo で動かしながら、ワークフローを走らせます。ほかのタスクの結果は、シナリオに書いたものを使います。終わりには、`chobo run` と同じく、帳簿の残高と仮押さえを出します（`crates/ritsu/tests/projects/invoice` の日本語の版）。

<div class="cb-term" markdown>

```text
invoice.ja.flow（シナリオ scenarios/invoice_paid.ja.json、2026-04-15T10:00:00Z から）
走らせる前の帳簿 在庫:
  入荷.do(納品書: "D-1", sku: "ペン", 数: 10)  通る
実行:
  引き当てる(注文: "A-1", sku: "ペン", 数: 3)      chobo     通る
  支払方法(会員: true, 金額: 120)                  rulec     {"方法":"請求書払い"}
  支払条件.支払日(受領日: "2026-04-15T10:00:00Z")  koyomi    {"day":"2026-05-08","at":"2026-05-08T09:00:00Z"}
  wait until 2026-05-08T09:00:00Z                            22 日と 23:00:00、2026-05-08T09:00:00Z まで
  入金を確かめる(注文: "A-1", 期日: "2026-05-08")  scenario  {"済み":true}
  出荷する(注文: "A-1", sku: "ペン")               chobo     通る
終わり: succeed {"結果":"出荷済","期日":"2026-05-08"}
終わりの帳簿 在庫（books/在庫.book）:
  勘定:
    棚(ペン)  確定 7、出ていく仮押さえ 0、入ってくる仮押さえ 0
    仕入先    確定 -10、出ていく仮押さえ 0、入ってくる仮押さえ 0
    客        確定 3、出ていく仮押さえ 0、入ってくる仮押さえ 0
  仮押さえ:
    引当(A-1, ペン)  確定
```

</div>

## ゲートが操作を守る

ritsu のブラウザで試すページにある小さな通販では、倉庫のゲートが、だれが在庫を動かしてよいかを決めます
（[website/playground/shop.ja](https://github.com/i2y/ritsu/tree/main/website/playground/shop.ja)）。ゲートの action は、帳簿の操作を `<振替>.<操作>` の形で指して守ります。

```gate
use book 在庫の引当 from "../warehouse/在庫.book"
…
action 在庫を押さえる(hold_stock)
  description "注文の分の在庫を、30 分まで押さえる"
  guards 在庫の引当 引当.hold
  principal 職員, 倉庫のシステム
  resource 在庫 from sku
…
permit 倉庫のシステムは仮押さえを動かせる(the_system_keeps_the_holds)
  description "倉庫のシステムは、注文の分の在庫を押さえ、注文が出荷されたら仮押さえを確定し、注文が取り消されたら仮押さえを取り消せる"
  principal is 倉庫のシステム
  action 在庫を押さえる, 仮押さえを確定する, 仮押さえを取り消す

expect deny 仮押さえは手で動かさない
  description "職員が手で注文の在庫を押さえたり、その仮押さえを確定したり取り消したりすることはない。仮押さえは注文に合わせて動く"
  principal is 職員
  action 在庫を押さえる, 仮押さえを確定する, 仮押さえを取り消す
```

`sekisho check` は、principal、action、resource の組み合わせを全部たどり、それぞれを Cedar と同じように判定します。帳簿に無い操作や、二つの action が守っている操作はエラーです。帳簿を持つサービスは、帳簿を動かす前に、`sekisho gen` が書く Cedar とコードを通して、ゲートに尋ねます。ワークフロー自身が帳簿を呼ぶときは、ゲートには尋ねません。chobo が返す拒否の理由はどれも帳簿のもので、ゲートの拒否にあたるものは無いからです。

## 操作で満たす要件

通販の要件は、倉庫での決め方がどこから来たかを書き、それを満たすものを、一つの振替の一つの操作まで挙げます。

```req
requirement 仮押さえは注文とともに動く(holds_go_with_the_orders)
  text "注文の分の在庫は倉庫のシステムが押さえ、注文が出荷されたら仮押さえを確定し、取り消されたら仮押さえを取り消す。だれも手で仮押さえを動かさない"
…
  satisfied by chobo "../warehouse/在庫.book" transfer 引当 operation hold
    reviewed 2026-10-07 by 倉庫の責任者 sha256:68556fb30e4f69df -> sha256:a740810644b16d29
```

矢印のあとのハッシュは、chobo がその操作について渡す定義のハッシュです。定義は、`chobo api` が出す振替を決まった形に整え、操作と、拒否されうる理由の名前を加えたものです。振替の移動、キー、仮押さえ、拒否されうる理由のどれかが変わると、yuen は、そのつながりがレビューしたときのものではなくなったと言います。

## コンテキストの地図

通販の地図は、倉庫のコンテキストに、帳簿のあるディレクトリを持たせています（`owns dir "../warehouse"`）。sakai は、あるコンテキストからほかのコンテキストにまたがる参照を、どの言語のファイルでも確かめます。ただし、`use book` と帳簿の操作のタスクは、その中に入りません。振替を帳簿に当てるのはホストの帳簿のクライアントで、ほかのコンテキストは帳簿の前に立つサービスを通して帳簿を使うからです。

通販は、帳簿を含むファイルと一緒に、ブラウザで開けます。[ブラウザで試すページで、通販の帳簿を開く](../../ja/playground/#project=shop.ja&file=warehouse/在庫.book)
