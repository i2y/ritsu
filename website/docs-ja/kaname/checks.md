# 検査が保証すること

`kaname check` は、モジュールを ritsu のほかの言語と同じ決まり（単位、通貨と税区分、率の刻み、範囲、丸め、int64）で確かめ、宣言した範囲から、値が渡る場所ごとに、その値がとりうる範囲を求めます。`ritsu check` は、プロジェクトのすべての `.kn` に、モジュールが呼ぶ規則、日付のファイル、帳簿と一緒にこの検査をかけます。例のモジュールは、どれも警告も無く通ります。このページでは、検査が何を言うかと、それを実際に動くコードとどう突き合わせて確かめているかを見ます。

## エフェクト

関数の型には、どこまで確かめられたかが載ります。

| エフェクト | 意味 |
|---|---|
| total | 宣言した範囲のどの入力にも答える |
| fails | 検査が示せなかった場所があり、そこはコードが実行時に確かめる。失敗の種類（`rule`、`date`、`range`、`overflow`、`zero`）も載る |
| diverges | 直接にか、ほかの関数を通して再帰する。止まることは確かめない |

エフェクトは kaname が推論し、関数の見出しに書いてあれば、それに収まるかを確かめます（E401）。呼んだ関数のエフェクトは、呼んだ側に伝わります。ほかの関数に渡した関数のエフェクトは、渡した側に伝わります。モジュールが通ったことを言う行に、エフェクトがまとめて出ます。

```console
$ cd crates/kaname
$ ritsu kaname check examples/order_flow/order_flow.kn examples/catalog/catalog.kn tests/doc/decisions.kn --lang ja
ok examples/order_flow/order_flow.kn: 関数 5 個、total
ok examples/catalog/catalog.kn: 関数 9 個、diverges
警告[W301]: tests/doc/decisions.kn:20:24: `weight` が、規則 `media_mail` の入力 `weight` の範囲 >=1oz <=240oz に収まるかを決められません
    20 |     media_mail(weight: weight, media_only: all(lines, l => l.media)).price
  = 範囲の解析では、値は >=0oz <=240oz です。関数のベクタにも、外れる入力はありませんでした。
  = 生成したコードが、走らせたときにこの場所で確かめます。
  = `if` で範囲に狭めると、範囲の解析がそれを読んで示せるようになります。
警告[W501]: tests/doc/decisions.kn:25:3: この `if` は、比べた値によって業務の定数（`0USD`、`5USD`）を選んでいます。表に書く判断です
    25 |   if total >= 35USD then 0USD else 5USD
  = 判断を rulec の表に書いて、規則を呼んでください。表に書けば、完全性と重なりを rulec が確かめ、承認する人がページで読めます。
ok tests/doc/decisions.kn: 関数 3 個: total 2 個、fails rule 1 個
```

三つ目のモジュールは kaname のテストの材料で、手本にする例ではありません。その `quote` は、Media Mail の料金の規則に重さを渡します。その重さが規則の範囲に収まると示すことも、外れる入力を見つけることも、検査にはできません。そこで検査は警告を出し（W301）、生成したコードが実行時にそこで重さを確かめ、関数のエフェクトは `fails rule` になります。`shipping` は、合計を比べる `if` で送料を選んでいます。これは業務の判断で、rulec の表に書くべきものです（W501）。表に書けば、完全性と重なりを rulec が確かめ、承認する人がページで読めます。規則を呼ぶ前に値を規則の範囲に狭めるだけの分岐には、警告は出ません。

生成するコードの形もエフェクトで決まります。Go では、失敗しうる関数だけが `(T, error)` を返します。TypeScript と Python では、関数ごとに、失敗しうる種類をまとめた型が付きます。

## 範囲を書かない値

範囲を書かない値は型の全体をとり、そこから計算した値も、ほかの値と同じく int64 に収まるかを確かめます。

```kn
fn next_id(last: number) -> number total
  last + 1
```

```console
$ cd crates/kaname
$ ritsu kaname check tests/docs/ids.kn --lang ja
エラー[E401]: tests/docs/ids.kn:4:1: 関数 `next_id` には `total` と書いてありますが、`overflow` で失敗することがあります
     4 | fn next_id(last: number) -> number total
  = 5 行目: `last + 1` の値は、範囲を書かない値から来るので、int64 を超えることがあります（`overflow`）
  = 書いたエフェクトに収まるよう、失敗しうる場所を示せるようにする（渡す値を `if` で範囲に狭める）か、エフェクトを広げて書いてください。
```

`last` は int64 のいちばん大きい数でもよく、それに 1 を足すと int64 を超えます。型の端から来るあふれは、それだけではエラーになりません。見出しに `total` が無ければ、`next_id` は `fails overflow` として通り、コードが `last + 1` を計算するところで確かめます。`last: number range >=0 <=1_000_000` のように範囲を書けば、その場所は示せて、関数は total になります。

## 渡す値を、渡す先の範囲で確かめる

関数の引数と答え、レコードのフィールド、規則と日付の入力、振替の額に渡す値は、どれも渡す先の範囲で確かめます。次のモジュールは、請求書が遅れて払われた日数を、規則の範囲に狭める分岐なしに規則に渡しています（分岐のある例 `late_payment` は[規則・日付・帳簿を呼ぶ](together.md)にあります）。

```kn
fn interest(invoice: Invoice, paid_on: date range >=2026-01-01 <=2029-12-31, reference_rate: rate[step 0.01%] range >=0% <=15%) -> money[GBPc]
  let due = terms.due(invoiced: invoice.invoiced)
  let late = paid_on - due
  charges(debt: invoice.amount, days_late: late, reference_rate: reference_rate).interest
```

```console
$ cd crates/kaname
$ ritsu kaname check tests/docs/unnarrowed.kn --lang ja
エラー[E301]: tests/docs/unnarrowed.kn:15:44: `late` が、規則 `charges` の入力 `days_late` の範囲 >=1 <=1500 の外になることがあります
    15 |   charges(debt: invoice.amount, days_late: late, reference_rate: reference_rate).interest
  = たとえば `interest(invoice: Invoice { id: "a", invoiced: 2026-01-01, amount: 50000000GBPc }, paid_on: 2026-01-01, reference_rate: 0%)` のとき、-29 になります。
  = 渡す前に `if` でその範囲に狭めるか、渡す先の範囲を広げてください。
```

エラーには、値を範囲の外に出す関数の入力が必ず付きます。関数のベクタの中から見つけたものか、式を解いて作ったものです。この例では、請求書を出したその日に払った入力で、期日の 29 日前に払ったことになります。範囲の解析が見積もった範囲の端は、実際には起きないことがあるので、例にはしません。示すことも、外れる入力を見つけることもできない場所は、上の W301 のように警告になり、コードが実行時にそこで確かめます。

## 生成したコードと参照インタプリタの答え

モジュールの意味を決めるのは参照インタプリタです。`kaname run` はそれで関数を走らせ、`vectors` はそれに流す入力を選びます。`ritsu gen` が書く TypeScript、Python、Go のコードは、これと突き合わせています。例とテストの 52 のモジュールのすべてのベクタ（13,982 件）、引数の宣言のすぐ外の入力、自分を含む型の値を、ホストが渡せるいちばん深い 200 段と、それより一段深い 201 段まで入れ子にした値です。英語の版と日本語の版の両方で、TypeScript の 16,717 件、Python の 16,633 件、Go の 16,613 件がすべて一致します（型の全体の外の入力は、その値を持てる言語で流します）。生成したコードは、`tsc --strict --erasableSyntaxOnly`、`mypy --strict`、gofmt と `go vet` を通ります。[Functional Core の四つの例](examples.md)のホストも、`output.txt` のとおりに出力します。

## 範囲の解析と、実際の値

範囲の解析は、式ごとに、その値がとりうる範囲を言います。例とテストのモジュールのすべての関数を、ベクタと、引数の宣言から選んだ 200 件の入力で走らせると（139,754 回）、5,009 の式に来た 3,452,025 の値は、どれも解析の言う範囲に入ります。失敗した 1,617 回も、どれも関数のエフェクトが言う種類で失敗します。

## Lean

kaname の評価と範囲の解析は、Lean でも書いてあります（ritsu の一つの Lean のパッケージの中の `proofs/KanameCore`）。範囲の解析が健全であることと、total と推論した関数が、宣言した範囲のどの入力でも失敗しないことを、関数を引数に取らない関数について証明しています。関数を引数に取る関数は、下の突き合わせでモデルと比べていますが、証明はまだありません。`sorry` や自前の公理に頼る証明はありません。`cargo test -p ritsu-model` は、59 のモジュールの 14,092 の評価で、モデルを参照インタプリタと範囲の解析に、場所ごと、式ごとに突き合わせ、違いはありません。モデルを 42 か所のどこか一か所だけ壊すと、そのたびに突き合わせが落ちることも確かめています。
