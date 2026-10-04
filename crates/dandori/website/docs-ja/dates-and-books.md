# 日付と帳簿

ワークフローは、日を数えることと、数を動かすことをよくします。注文の品を支払期日まで押さえ、支払われたら出荷し、支払われなければ棚に戻す、といった形です。`.flow` は算術を持たないので、どちらも自分ではしません。日は [koyomi](https://github.com/i2y/ritsu/tree/main/crates/koyomi) の日付のファイル（`.cal`）が数え、数は [chobo](https://github.com/i2y/ritsu/tree/main/crates/chobo) の帳簿（`.book`）が持ちます。`.flow` はそれを rulec の規則と同じように読み、日付は規則と同じように呼び、帳簿の操作はタスクとして呼びます。

請求の例が、これを全部使います。どのプラットフォームでもそのまま動きます（[examples/invoice](https://github.com/i2y/dandori/blob/main/examples/invoice/invoice.ja.flow)）。

```flow
use dates 支払条件 from "dates/支払条件.cal"
  lambda "arn:aws:lambda:ap-northeast-1:123456789012:function:payment-terms"
use book 在庫 from "books/在庫.book"
  lambda "arn:aws:lambda:ap-northeast-1:123456789012:function:stock"
…
task 引き当てる(注文: string, sku: string, 数: int) -> 在庫.引当
  book 在庫.引当.hold
  starts 在庫.引当
  errors 在庫切れ
…
case 押さえ : 在庫.引当 follows 在庫.引当

flow
  押さえ <- 引き当てる(注文: 受注.id, sku: 受注.sku, 数: 受注.数)
    on 在庫切れ => succeed 結果 = 在庫切れ, 期日 = none
  let 期限 = 支払条件.支払日(受領日: now)
  wait until 期限.at
  let 入金 = 支払を確かめる(注文: 受注.id, 期日: 期限.day)
  match 入金.済み
    true =>
      押さえ <- 出荷する(注文: 受注.id, sku: 受注.sku)
        on expired => succeed 結果 = 未払い, 期日 = 期限.day
      succeed 結果 = 出荷済, 期日 = 期限.day
  …
```

koyomi は日付のファイルを範囲のすべての日で確かめ、chobo は帳簿の境界と、操作ごとに断られうる理由を確かめます。dandori は、その結果を規則と同じく ritsu の口から読みます。`ritsu dandori` は日付のファイルと帳簿を同じプロセスの中で読み、`dandori` のコマンドだけでは、それらを使うワークフローを `ritsu dandori` で走らせるよう言います（E018）。

## 日付

例の日付のファイルは、20 日締め翌月 10 日の 9 時払いで、支払日が休みなら前の営業日です。

```text
dates 支払条件(payment_terms_ja) v1
description "20 日締め翌月 10 日の 9 時払い。支払日が休みなら前の営業日"
use calendar "../calendars/平日.cal"

inputs
  受領日(received) : date  range >=2026-01-01 <=2027-12-28

date 締め日(closing) = 受領日
  close day 20          # 「20 日締め」

date 支払日(payment) = 締め日
  day 10 of month +1    # 「翌月 10 日払い」
  roll preceding        # 「支払日が休みなら前の営業日」
  at 09:00
…
```

`use dates <名前> from "<file.cal>"` で読み、ファイルの日付を、規則と同じく名前で呼びます（`支払条件.支払日(受領日: …)`）。返すのは二つのフィールドのレコードで、`day` が日付、`at` が、日付が時刻を言うとき（`at 09:00`）のその時刻を UTC にしたものです。`at` はそのまま `wait until` に渡せます。時刻を言わない日付は `day` だけを返します。

- 日付の入力には `date` を渡します。ファイルのカレンダーが UTC オフセットを言うなら（`offset +09:00`）、`timestamp` も渡せます。時刻は、そのオフセットでその時刻にあたる日として読みます（`2026-03-31T15:30:00Z` は、`+09:00` では 4 月 1 日）。オフセットを言わないカレンダーの日付に時刻を渡すと E003 で、注が、カレンダーにオフセットを書くか、日付を渡すよう言います。
- 整数の入力には `int` を渡します。ファイルが宣言した範囲に収まるかは、規則の入力と同じく確かめます（E014、W104）。
- 一つのファイルの日付は、それぞれ別に、その日付が読む入力だけを渡して呼びます。
- `use dates` の下の `lambda "<関数>"` は、Step Functions と Lambda durable functions で日付を計算する Lambda 関数です。`local` を書くと、Temporal ではローカルアクティビティで呼びます。どちらも規則と同じです。

koyomi のコードは純関数ですが、日付は規則と同じくアクティビティ（各プラットフォームの呼び出し）で呼びます。カレンダーの祝日は毎年変わるので、ワークフローのコードの中で計算すると、表を入れ替えたワーカーで再生が食い違います。規則と同じく、入力と答えは実行の履歴に残ります。

`date` は型で、値は日付を `YYYY-MM-DD` と書いたものです。入力、出力、フィールド、タスクの引数と結果に使え、文字列に埋め込めます（`"{受注.id} は {出荷.day} に出荷"`）。ほかの型と同じく、入ってくるところで形を確かめます。比べることも足すこともしません。日を数えるのは日付のファイルの役目です。

`now` は、その文を走らせた時刻で、秒までの UTC の `timestamp` です。日付、タスク、`wait until`、文字列の中（`"受付 {now}"`）に書けます。どのプラットフォームも、再生や再試行で同じ時刻を読み直せるように読みます（下の表）。

## 帳簿

例の帳簿は、SKU ごとの在庫を持ちます。入荷で増え、注文は出荷まで（長くても 60 日）その数を押さえます。

```text
book 在庫 v1
…
unit 個

account 棚(sku: string) : 個
  …
  at least 0 refused as 在庫切れ
account 仕入先 : 個 outside
account 客 : 個 outside

transfer 入荷(納品書: string, sku: string, 数: 個)
  key 納品書, sku
  move 数 from 仕入先 to 棚(sku)

transfer 引当(注文: string, sku: string, 数: 個)
  …
  key 注文, sku
  pending expires after 60 days
  move 数 from 棚(sku) to 客
```

`use book <名前> from "<file.book>"` で読み、タスクに `book <帳簿>.<振替>.<操作>` と書くと、そのタスクが振替の操作になります。操作は chobo のものそのままで、すぐに確定する振替なら `do`、仮押さえにする振替なら `hold`・`post`・`void` です。

- タスクの引数は、振替の引数を名前のとおりに書きます。`do` と `hold` は全部、`post` と `void` はキーの引数です。`post` に数の引数も書くと、押さえた数のうちその数だけを確定し、残りを戻します。数は `int` で、帳簿の単位で数えた整数です。
- `hold`・`post`・`void` は仮押さえを返します。振替の名前のレコード（`在庫.引当`）で、キーの引数と、仮押さえの状態 `state` を持ちます。`do` は何も返しません。
- 帳簿が断る理由が、そのままタスクのエラーになります（`errors 在庫切れ`、`errors expired`）。帳簿がその操作を断りえない名前は E016 です。宣言していない理由で断られると、呼び出しは失敗します（`failure`）。
- 帳簿の操作は、キーごとに一度しか通りません。だから `key` は書きません。同じ操作をもう一度すると、前にしたと答え、それは通ったものとして読むので、リトライしても二度は動きません。`refused as` も書きません（下）。
- `use book` の下の `lambda "<関数>"` は、Step Functions で帳簿の操作をする Lambda 関数です。

[`tests/flows/dates_and_books.flow`](https://github.com/i2y/dandori/blob/main/tests/flows/dates_and_books.flow) は、押さえた分の一部を確定します。

```flow
task 一部を出す(注文: string, sku: string, 数: int) -> 倉庫.引当
  book 倉庫.引当.post
  sends post
  errors expired
```

### 仮押さえを案件にする

仮押さえは、押さえ中から、確定・取消・期限切れのどれかへ進みます。帳簿はこれをステートマシンとして言うので、`case <名前> : <帳簿>.<振替> follows <帳簿>.<振替>` で仮押さえを案件にでき、検査は規則のステートマシンと同じく追います。押さえるタスクは `starts <帳簿>.<振替>`、確定するタスクは `sends post`、取り消すタスクは `sends void` を書きます（違えば E008）。

振替の仮押さえに有効期限があれば、`expire` は外で勝手に起きます。`external` を書かなくても、検査はどの確定と取消の前にもそれを数えます。断る理由は状態ごとに帳簿が決めているので、案件には `refused when` を書きません。理由を処理しないまま返ってくることがあれば、検査がそう言います。

```text
エラー[E022]: tests/fixtures/book_refusals.flow:41:1: ここでは帳簿が `post` を `expired` で断ることがあります（`押さえ` が expired のとき）。`出荷する` の `errors` に `expired` を書き、`on expired =>` で処理してください
    41 |       押さえ <- 出荷する(注文: 受注.id, sku: 受注.sku)
  そうなる例:
      38  引き当てる: 押さえ が held で始まる
      40  match 受注.急ぎ: true
          外部のサービスで `expire` が起きる: 押さえ held → expired
```

押さえたままの品を残して終わりうるワークフローは、ほかの案件と同じく E020 です。請求の例は、`on failure` で棚に戻します。

## プラットフォームごと

| | Step Functions | Temporal | Lambda durable functions | Argo Workflows | pydantic-graph |
|---|---|---|---|---|---|
| 日付 | Lambda の Task。koyomi が書く Python を呼ぶ関数を dandori が書く | koyomi の TypeScript・Python・Go を呼ぶアクティビティ（`local` ならローカルアクティビティ） | 同じ Lambda 関数を invoke する | koyomi の TypeScript を呼ぶ caller のイメージ | koyomi の Python を呼ぶ関数 |
| 帳簿の操作 | 帳簿の Lambda の Task。chobo の Python のクライアントで操作する関数を dandori が書き、仮押さえは渡した引数から作る | dandori が書くアクティビティが、`Transport` の `book` で、渡された chobo のクライアントを呼ぶ | 同じものをステップの中で | 同じものを caller のイメージで | 同じものを `Deps.tasks` から |
| `now` | そのステートに入った時刻（`$states.context.State.EnteredTime` を秒まで） | ワークフローの時計（再生でも同じ時刻を読む） | 時計を読むステップ。答えはチェックポイントに残る（操作一回で、E040 が数える） | 値を計算するテンプレートの式の `now()`（一度だけ評価される） | `Deps.clock` |

生成したコードの横に置くもの：

- **日付**：`koyomi gen <file.cal> --out koyomi` が日付のファイルのコードを書き、生成したコードは、隣の `koyomi/` からそれを読み込みます。Temporal の Go では、日付のファイルごとに Go のモジュール（`koyomi/go/<パッケージ>`）になるので、`rules.go` の頭に書いたとおり、自分の `go.mod` で require して replace します。
- **帳簿**：chobo が帳簿のクライアントを書きます（`chobo build <file.book> --target postgres-typescript`、`tigerbeetle-python`、`postgres-go` など）。クライアントが作る帳簿の値を、帳簿の名前で `Transport` に渡します。TypeScript では `transport({ books: { 在庫: postgres(pool, { tenant }) } })`、Python では `transport(books={"在庫": postgres(connection)})`、Go では `TransportOptions{Books: map[string]any{"在庫": book.Postgres(pool, tenant)}}` です。Step Functions では、`lambda/book_在庫_handler.py` を chobo の Python のクライアントと一緒に置き、`handler = make_handler(lambda: tigerbeetle(ClientSync(…)))` で作ります。

## どう確かめているか

- どのプラットフォームも、ほかの呼び出しと同じく、すべてのシナリオを参照インタプリタと突き合わせます。シナリオは、日付には日付（と時刻）で、帳簿の操作には、通った・前に通った・断られた、のどれかで答えます。`now` がどこでも同じ時刻を読むよう、各プラットフォームの時計は一つの時刻に替えます。
- dandori が koyomi のコードのまわりに書くコード（Lambda 関数、`rules.ts`、`rules.py`、`rules.go`）は、koyomi が書くコードと一緒に、ファイルの範囲のすべての入力で動かし、koyomi が言う日付と同じ答えでなければなりません。七つに一つの入力は、日付の代わりにその日の時刻で渡し、カレンダーのオフセットで同じ日として読むことも確かめます。
- すべてのフローの実行が出す帳簿の操作を、dandori が TypeScript・Python・Go で書く `Transport` と、Step Functions 向けに書く Lambda 関数から、chobo が書くクライアントに通し、PostgreSQL と TigerBeetle の上で動かします。答えは、chobo の参照インタプリタと同じでなければなりません。空の帳簿、入荷で満たした帳簿、そして同じ帳簿でもう一度（どの操作も前にした）の三通りで確かめます。
