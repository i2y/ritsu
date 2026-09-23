# 生成して呼ぶ

```console
$ rulec gen rules/ --out generated/
```

出るのは普通の Python・TypeScript・JavaScript・Rust・Ruby・PHP・Swift のモジュールと、Java のクラスと、普通の Go パッケージと、SQL の一つの問い合わせと、Wasm の一つのモジュールです。ランタイムも設定も要らず、標準ライブラリの外に依存もありません — 最後の一つは主張ではなく**検査された性質**です。`rulec test` が Go 側を `GOPROXY=off` で走らせています。NumPy だけは別で、出るのはコードではなく、規則そのものと、それを読む固定の評価器です。こちらは numpy に依存します。列ごとまとめて判定したいホストが、もう持っているはずのものです。

検査を通らない規則からは、何も生成されません。

## 対応する出力言語

いま対応しているのは Python・TypeScript・JavaScript・Rust・Ruby・PHP・Go・Swift・Java・SQL・Wasm・NumPy の12 言語です。同じ表から、フロントエンドとバックエンドとモバイルと DB が同じ答えを返すことを、いまある一致検査の仕組みでそのまま証明できるようにするのが狙いです。

| | 状態 | 要るもの |
|---|---|---|
| Python | 対応済み | `python3` |
| TypeScript | 対応済み | `node` だけ（ビルド手順も tsconfig も要りません） |
| JavaScript | 対応済み | `node` だけ、あるいはブラウザ。TypeScript から型を取り除いたもので、ES モジュール（`.mjs`）として出ます |
| Rust | 対応済み | `rustc` だけ（cargo もクレートも要りません） |
| Ruby | 対応済み | `ruby` 3.x か 4.x。`json` が標準添付なので gem は要りません。モジュールの隣に `.rbs` も出ます |
| PHP | 対応済み | `php` 8.2 以降。`ext/json` は本体に入っているので composer は要りません。列挙と型付きの引数を使い、割り算はすべて `intdiv` です |
| Go | 対応済み | `go` |
| Swift | 対応済み | `swiftc` だけ（SwiftPM も `Package.swift` も要りません）。Rust と同じく単位が型に載ります |
| Java | 対応済み | JDK。`javac` と `java` だけで、Maven も Gradle も要りません。`--release 17` で組むので 17・21・25 のどれでも動きます。Kotlin や Scala からはそのまま呼べます |
| SQL | 対応済み | `python3`。その標準添付の `sqlite3` で問い合わせの一致検査を回します。一緒に出る関数のほうは、`psql` で本物の PostgreSQL に流して確かめます。どちらも PostgreSQL 向けに書いてあります。**畳み込み（`fold`）のある規則にだけは生成しません** |
| NumPy | 対応済み | `python3` と `numpy`。コードではなく規則そのものを data として書き出し、隣に置いた固定の評価器が読み込み時に列ごとの閉包を組みます。**並びをたどる規則だけは生成しません** |
| Wasm | 対応済み | `wasm32-unknown-unknown` を入れた `rustc` と、一致検査に使う `node`。モジュール自体は何も import しません |

一つだけ決めていることがあります。**一致検査に乗らない言語は入れません。** 参照評価器とバイト単位で突き合わせられない生成物は、「証明済み」という看板の外側にあることになるからです。三つめの TypeScript を足すのに掛かった実コストは生成器に約 700 行で、五つめの Ruby も六つめの Swift も同じくらいでした。ただし Ruby のときは手で直したファイルが二十数個あったのに対して、Swift で足したのは対象言語をまとめた一覧の一行だけです。

### 一覧に無い言語へ生成する

この一覧に縛られる必要はありませんし、バックエンドが足されるのを待つ必要もありません。規則は、コードを組み立てるのに要るものをすでに全部出しています — `rulec api` が名前・単位・範囲・丸めを、`rulec schema` がやりとりする JSON の形を出すので、**自分のジェネレータで好きな言語に生成できます**。

**そのとき突き合わせも一緒に付いてきます。** 出したものをアダプタの規約で包めば、`rulec verify` が規則の境界から組み立てた全件を当ててくれます。対応済みの言語とまったく同じ扱いで、rulec 側は一行も変わりません。

[ほかの言語へ生成する](backends.md)では、その一周を SQL で手で通しています（SQL に生成対象が付く前に書いたもので、手順は相手が何でも同じです）。88 件全件一致、そのあと閾値を一つわざと壊して、当てはまった行とそれを起こした入力を報告が名指しするところまで。

### SQL には問い合わせと関数の両方が出ます

出るものが二つあります。一つめは**入力の関係に対する一つの問い合わせ**です。`shipping_fee_input` に `_id` の列と入力ごとの列（別名）を用意すると、`_id`、入力、出力、そして表ごとに当てはまった行の番号の列が返ります。表の一行が `WHEN` 一つ、丸めは最後の `SELECT` の中の算術で、百万行でも一文で通ります。締めのバッチや再計算や分析の集計に要るのはこれで、一件ずつの関数にはできないことです。ビューにすれば、規則がデータベースの中の一つの表になります。

問い合わせは PostgreSQL 向けに書き、SQLite でも同じに動く範囲に留めてあります。`rulec test` はその SQLite で参照評価器と突き合わせるので、要るのは `python3` だけです。値はやりとりする JSON と同じで、宣言した単位の整数、列挙はその名前、日付は 1970-01-01 からの日数。どの列が何かはファイルの先頭に書いてあります。問い合わせは止まれないので、入口のガードも列です。`_input_error` は宣言の中の行なら NULL、外の行ならその文を持ちます。

二つめが `shipping_fee_function.sql` で、同じ問い合わせを関数にしたものです。こちらは一件ずつ呼びます。中身は上の問い合わせそのままで、引数を一行だけの入力の関係にする CTE を頭に足しただけです。二つが別々のコードになっていないので、片方だけ古くなることがありません。

違うのは、宣言の外の入力に**例外を投げる**ところです。問い合わせは途中で止まれないので `_input_error` の列で返すしかありませんが、HTTP 越しに呼ばれたとき、その列を見ない相手には間違った金額が 200 で返ってしまいます。

PostgREST や Supabase が公開しているスキーマに置けば、それだけで規則を HTTP から呼べます。`POST /rpc/shipping_fee` に `{"dest": "北海道", …}` を投げると `[{"fee":1200,…}]` が返り、宣言の外の入力なら **400** と規則の文が返ります。このファイルは PostgreSQL 専用です。SQLite には `CREATE FUNCTION` が無いので、`rulec test` は `psql` で本物の PostgreSQL に流して確かめます。繋ぎ先が無ければ、そう言って飛ばします。

**並びをたどる規則だけは、SQL に生成しません。** 一つの問い合わせには、行から行へ値を持ち越して途中で打ち切る場所がないからです。入れる予定もありません。`gen` はその規則についてだけ、そう言って飛ばします。

ページ自体は英語です（読む相手がエージェントなので、この種の資料は英語で書いています）。**人の側が知っておけばいいのは「頼めばできる」ということだけ**で、手順はエージェントがそのページを読んで進めます。

## 出るものの形

表の一行が分岐の一本になり、もとの表のセルがそのままコメントで添えてあります。

```python
def fee_demo(dest: Prefecture, girth: Cm, weight: Gram) -> YenInclTax:
    """Rule 送料例 v1: the same decision as fee_demo_traced, without the rows that matched."""
    out, _ = fee_demo_traced(dest, girth, weight)
    return out


def fee_demo_traced(dest: Prefecture, girth: Cm, weight: Gram) -> tuple[YenInclTax, list[Fired]]:
    if not _isinstance(dest, Prefecture):
        raise RuleInputError("あて先 is not a value of enum Prefecture", dest)
    if not 1 <= girth <= 100:
        raise RuleInputError("三辺合計 is out of range", girth)
    trace: _Trace = []
    # table サイズ判定 (policy first)
    if girth <= 60:  # row 1: <=60cm | S60
        size = SizeClass.S60
        trace.append(Fired("サイズ判定", 1))
    elif girth <= 80:  # row 2: <=80cm | S80
        size = SizeClass.S80
        trace.append(Fired("サイズ判定", 2))
    elif True:  # row 3: - | S100
        size = SizeClass.S100
        trace.append(Fired("サイズ判定", 3))
    else:
        raise AssertionError("unreachable: completeness was statically checked by rulec")
    ...
    return YenInclTax(_round_up(fee, 10)), trace
```

```go
func FeeDemo(in Input) (YenInclTax, error) {
	out, _, err := FeeDemoTraced(in)
	return out, err
}

func FeeDemoTraced(in Input) (YenInclTax, []Fired, error) {
	if !in.Dest.Valid() {
		return 0, nil, &RuleInputError{What: "あて先 is not a value of the enum", Value: int64(in.Dest), HasValue: true}
	}
	var trace []Fired
	// table サイズ判定 (policy first)
	var size SizeClass
	if int64(in.Girth) <= 60 { // row 1: <=60cm | S60
		size = SizeClassS60
		trace = append(trace, Fired{"サイズ判定", 1})
	} else if int64(in.Girth) <= 80 { // row 2: <=80cm | S80
		size = SizeClassS80
		trace = append(trace, Fired{"サイズ判定", 2})
	} else if true { // row 3: - | S100
		size = SizeClassS100
		trace = append(trace, Fired{"サイズ判定", 3})
	} else {
		panic("unreachable: completeness was statically checked by rulec")
	}
	...
	return YenInclTax(roundUp(int64(fee), 10)), trace, nil
}
```

呼ぶのは `fee_demo` で、その形は変わりません。分岐があるのは `fee_demo_traced` のほうで、こちらは値と一緒に、当てはまった行を表ごとに一つ、順に返します（表の名前と行番号）。ログの一行や「なぜこの送料か」への答えに要るのはこれです。下の一致検査は、値だけでなくこの行も参照評価器と突き合わせます。三つめの関数 `fee_demo_record` は、一回の呼び出しを記録の形（`replay` と `diff` が読む fixtures の一行）にします。生成コードがログを書けば、改定のたびに実データで diff が回ります。

読める形であることを、生成器は四つで守っています。

- **NumPy だけは別です。** 生成したコードではなく、規則そのものを data として配り、固定の評価器がそれを読みます。
- **セルを省略しません。** 手前の分岐で真とわかる条件も書きます（`elif True:` はそのため）。もとの表の行と目で突き合わせられることが、生成物の唯一の読み方です。
- **単位は型に載せます。** Rust は newtype、Swift は値が一つだけの struct、Go は defined type、TypeScript は branded bigint、Python は `NewType`、Wasm のモジュールは Rust のものなので同じ newtype。`YenInclTax` と `YenExclTax` を取り違えるとコンパイルで止まります。Ruby・PHP・JavaScript・Java・SQL は単位を置ける型が無いので、そこは注記で伝えます。ただし PHP と Java は引数の種類そのものは宣言するので、入口の検査は範囲だけで済みます。
- **丸めは自前のヘルパで行います。** Python と Ruby の整数除算は −∞ 方向、Rust・Swift・Go・Java・TypeScript・JavaScript・PHP の `intdiv`・SQL は 0 方向で食い違うので、言語の素の除算には任せません。
- **言語の組み込み関数をそのまま呼びません。** 入力のエイリアスが `min` や `list` でも壊れないよう、`_min` `_max` `_isinstance` を生成側に持っています。

参照評価器（rulec の中にある「正解」の実装）と生成した各言語が同じ答えを返すことは、境界から自動で作ったテストケースを全部に流し、**決まった形の JSON にしてバイト単位で**突き合わせて確かめています。

## 呼び方は、読まなくても分かる

```console
$ rulec api rules/クーポン一枚.rule | jq -r .python.signature
def coupon_step(subtotal: YenInclTax, applied: YenInclTax, kind: CouponKind, rate: Rate, face: YenInclTax, dup: bool) -> Output:
```

一つの JSON に、module と関数名、引数（和名・エイリアス・型・単位・範囲）、出力（丸めつき）、列挙の値の**その言語での綴り**（Python は `CouponKind.PERCENT`、Go は `couponstep.CouponKindPercent`）、投げられる例外が入っています。

手で書いた呼び出し規約は、生成器が名前を変えた日から静かに嘘になります。だからこの一覧は生成器の隣で組み立て、テストが生成物そのものに縛ります — 一覧が言う名前が生成ファイルにそのまま書かれていること、Python は import して `inspect.signature` と比べること、Go は**一覧だけから呼び出しプログラムを組み立てて** `go vet` に通すこと（名前が一つでも違えばコンパイルが通りません）。

[生成物の詳しい説明](generated-code.md){ .md-button }

## 呼び出し側のオブジェクトのまま呼ぶ

入力に `from` を書いた規則（[表(.rule)を書く](tour.md)の「入力を、呼び出し側のオブジェクトから取る」）には、規則の関数のほかに、呼び出し側のオブジェクトを受け取る関数が一つ出ます。名前は、規則の関数の名前に `_from` を付けたものです。

```python
from order_shipping import order_shipping_from

order = {
    "shipping": {"zone": "okinawa"},
    "lines": [{"sku": f"s{i}", "chilled": i == 3, "amount_jpy": 100} for i in range(11)],
}
print(order_shipping_from(order))  # 2200
```

`.proto` の形なら、protojson が書いた JSON をそのまま渡します。名前は lowerCamelCase のままでよく、既定値のフィールドが省かれていても、int64 が文字列で来ても読めます。

```python
import json
from shipment_fee import shipment_fee_from

body = """{"destination": {"region": "okinawa"},
 "parcels": [{"weightG": "1200"}, {"weightG": "800"}, {"weightG": "500"}],
 "declaredValueJpy": "200000", "deliveryWindow": "evening"}"""
print(shipment_fee_from(json.loads(body)))  # 5100
```

この関数を書くのは、12 の対象のうち五つです。Python・TypeScript・JavaScript・Ruby・PHP は、呼び出し側がオブジェクトをただの連想配列として持っているので、その型に名前を付けずに受け取れます。Go・Swift・Java・Rust はオブジェクトを型で持っていて、その型を名指すには、型を生成するか、呼び出し側の型を追いかけるかしかありません。この道具はどちらもしません。SQL と NumPy は列を、Wasm のモジュールは規則自身の入力を並べた JSON を受け取るので、射影する相手のオブジェクトがありません。パスを契約に照らす検査は、`rulec check` の中で起きるので、12 の対象のどれにも効きます。

`rulec api` は `projection` の下に、借りた契約、入力ごとのパス、五つの言語での関数名と署名を並べます。

## 二つのガード

**入口のガード**は、証明が前提にしたことを実行時に守らせます。数値の入力は宣言した範囲に、列挙の入力はその値の集合に照らされます。これが無いと、宣言の外から呼ばれた側が静かに間違った数字を受け取ります — 完全性の証明は、宣言されていない入力については何も言っていません。整数でない数は、範囲を見る前に断ります（呼び出し側がそういう値を渡せる言語で）。小数はどんな範囲にも収まるので、0.1% 刻みの率に 18.3 を渡すと、そのままでは 1.83% として計算されてしまうからです。

**矛盾のガード**は W114 の片割れです。`unique` の表の二行が重なりうるかを検査器が決められなかったところでは、生成コードは黙って一方を選ばずに止まります。

```python
# guard: W114 (table 適用判定, row 1 × row 2): a pair of rows whose exclusivity could not be proven statically
if 残高A <= 1000 and 残高B >= 3980:
    raise RuleContradictionError("table 適用判定: row 1 and row 2 matched at the same time")
```


### 生成した Rust を、モデル検査器にも読ませる

Rust のモジュールの隣に、`gen` は `<別名>_proof.rs` を書きます。[Kani](https://model-checking.github.io/kani/)（モデル検査器）の証明ハーネスで、`#[cfg(kani)]` の下にあるので `rustc` は読みません。`kani <別名>_proof.rs`、または `rulec test --proofs` が、ベクタではなく**宣言した範囲のすべての入力**について生成コードを読みます。確かめるのは、どの表も素通りしないこと、矛盾のガードが当たらないこと、`i64` があふれないこと、そして `unique` の表の行が範囲をちょうど一度ずつ覆うことです。

モデル検査器と、表を証明した検査器は、コードを一行も共有していません。走らせる値打ちはそこにあります。一致すれば、無関係な二つの道具が同じことを言ったことになります。食い違えば、どちらかが誤っていて、それを示す入力がそのまま出てきます。コーパス 47 本では、ハーネス 111 本が 175 秒で通ります。

言っていないこと：表そのものについては何も。Rust 以外の生成物についても何も。それと、ハーネスがそもそも出ない規則が二種類あり、どちらもファイルに理由が書いてあります。`string` の入力があるもの（記号として置けません）と、`allocate` で配分を出しているもの（変数で割る式が二つ入るとモデル検査器が返ってきません）です。

## 規則をエージェントのツールにする

`rulec mcp` は、規則を**書く**エージェントのためのものです。規則を**呼ぶ**エージェント、つまり何かの途中で「この注文は返品できるか」「この会員の手数料率は」と決めなければならない側には、表を渡します。

これは「モデルに判断させるな」という話ではありません。**もう決まっている判断**（規約に書いてある、料金表に載っている、承認を経ている）を毎回考え直させるな、という話です。毎回考え直した答えは境界のあたりで揺れ、あとから監査もできません。まだ決まっていない判断 — どの区分か、どのくらい深刻か — は人かモデルの仕事で、その答えを**値として渡せば**、そこから先は表が決めます。

そのために `gen` は、規則そのものを MCP サーバとしてモジュールの隣に書きます。Python のモジュールの隣に `shipping_fee_mcp.py`、JavaScript の隣に `shipping_fee_mcp.mjs`。登録すれば、規則がその別名のツール一つになります。

```console
$ claude mcp add shipping_fee -- python3 generated/python/shipping_fee_mcp.py
```

これは手元のエージェントの話です。URL しか受け付けない相手 — チャットのカスタムコネクタ、エージェントの組み立て画面、ワークフロー製品の MCP ノード — が欲しがるのは MCP の Streamable HTTP で、それも**同じサーバ**です。

```console
$ python3 generated/python/shipping_fee_mcp.py --http 8000
http://127.0.0.1:8000/mcp
```

待ち受けるのは既定で `127.0.0.1` だけです。`Origin` が手元でない要求は、`--origin` で名指ししない限り断ります。**TLS と認証は入っていません**。前に置いてください。`rulec test` は stdio と HTTP のどちらでもベクタを全件流すので、つなぎ方で答えが変われば食い違いとして出ます。

**MCP Apps** を表示できるホストには、同じサーバがもう一つ渡します。承認者向けのページを、そのツールの view として渡すのです。`gen` がサーバの隣に書く `shipping_fee_page.html`（`rulec doc --format html` が出すページそのもの）で、ホストはそれを、**いま訊かれた件が入った状態で**開きます。フィールドは埋まっていて、答えが出ていて、決めた行に色が付いています。チャットを読む人に見えるのが、数字だけでなく「どの表のどの行がそう言ったか」になります。

ツールも同じ JSON の形でやりとりします。`inputSchema` は `rulec schema` の `in` そのもので、整数の単位と、率なら刻みが説明に書いてあります。答えはモジュールが書く記録の一行で、入力と、出力と、**決めた表の行**が入っています。

```json
{"in":{"届け先":"鹿児島県","重量":800,"注文金額":4200,"会員":"一般"},"observed":{"送料":800},"trace":[{"table":"基本送料","row":3},{"table":"負担判定","row":3}]}
```

ここから二つのことが出ます。答えが行を名指しするので、監査できること。呼び出し一件が fixtures の記録一件になること。`--record calls.jsonl` を付けて起動すると、答えた呼び出しを全部そのファイルに残し、`replay` と `diff` がそのまま読みます。エージェントが尋ねたことが、次の改定を測る記録になります。

規則が受けられない呼び出しは、引数の名前を挙げて断ります。範囲の外の値、列挙に無い名前、整数でない数。0.1% 刻みの率に 18.3 を渡すと、1.83% として読まれるのではなく断られます。

サーバもほかと同じ生成物です。`python3` か `node` のほかに入れるものは無く、`rulec test` が runner と同じベクタでサーバを叩き、答えを参照評価器と突き合わせます。

[生成物の詳しい説明](generated-code.md#the-rule-as-an-mcp-tool){ .md-button }

## ほかのチームが呼ぶサービスにする

エージェントは呼び手の一つです。もう一つは、別のリポジトリにある、たいていは別の言語で書かれた、すでに何かのやりとりの作法を持っているコードです。そちらのために `gen` は、規則を [Connect](https://connectrpc.com/) のサービスとして書きます。契約は `.proto` 一枚、その後ろに立つのは二十五行ほどの変換です。

```proto
service ShippingFeeService {
  // 規則は純関数なので、この手続きには副作用が無く、GET でも呼べる。
  rpc Decide(DecideRequest) returns (DecideResponse) {
    option idempotency_level = NO_SIDE_EFFECTS;
  }
}
```

この中の三つは、書き写しではなく決めたことです。

**置いた場所がそのまま package になっています。** buf のモジュールが求める形なので、この `.proto` はそのまま置けます。`buf lint` は何も言いません。

**この手続きには副作用が無い、と宣言してあります。** ここではそれは願いではなく、すでに証明されていることです。同じ入力なら同じ答えが返る、表の一つの版についてはいつまでも。Connect はそういう手続きを GET でも呼ばせます。答えをキャッシュに載せられるのはそのためです。

**答えには、決めた行が付いてきます。** だから呼び出し一件が fixtures の記録一件になります。返す見出しには `rulec-source-sha256` が入っていて、どの版の表が答えたのかが分かります。

stub の作り方は [connect-py](https://github.com/connectrpc/connect-py) の文書がすすめるとおり、buf です。`buf.yaml` と `buf.gen.yaml` も `gen` が書くので、出てきたディレクトリはそのまま buf のモジュールになっています。

```console
$ uv add connectrpc
$ cd generated/proto && buf generate
```

サービスは **ASGI と WSGI の両方**として書き出します。規則は純関数で待つものが無いので、二つは同じ本体に付いた二つの扉です。

```console
$ uvicorn shipping_fee_service:app --port 8080      # ASGI
$ gunicorn 'shipping_fee_service:wsgi_app'          # WSGI
```

`connectrpc` は `gen` の出すものの中で唯一よその依存で、それもサービスの一枚に閉じています。呼ばれるモジュールのほうは相変わらず何も import しません。`rulec test` は全ベクタを、二つのアプリそれぞれに POST と GET で、合わせて四回通して参照評価器と突き合わせます。

そして、**いま動いている**ほうが Connect のサービスなら、矢印は逆を向きます。`rulec adapter --template connect-python` が二十行を出すので、その答えを `rulec verify` の前に置けば、一致率と、食い違っている行が出ます。

[生成物の詳しい説明](generated-code.md#the-rule-as-a-connect-service){ .md-button }

## 入口の検査も同じ表から

生成した関数は自分の入口を守りますが、値はたいていもっと手前で入ってきます。フォーム、HTTP の受け口、キュー。`rulec schema` は、そのやりとりの JSON Schema を出します。入力ごとに型・単位・範囲・列挙の値が入っているので、手前の検査も同じ表から組めて、ガードとずれません。

```console
$ rulec schema rules/送料.rule                # キーは規則の名前（届け先）
$ rulec schema rules/送料.rule --keys alias   # キーは ASCII の別名（dest）。元の名前は title に入る
```

JSON Schema は、OpenAPI のパラメータやリクエストボディにそのまま置けます。型付きのモデルが欲しければ、いつもの変換器が読みます。pydantic なら `datamodel-codegen`、zod なら `json-schema-to-zod`。どれも rulec のバックエンドではありません。スキーマが元で、JSON Schema を読む生成器はもうあるので、この道具が抱える必要がありません。

入口で気をつけることは二つです。数は全部、宣言した単位の整数で、率は刻みの個数。どの単位かはプロパティの説明に書いてあるので、18.3% と表示するフォームは 183 を送ります。列挙は規則に書いた名前のまま渡します。

## 生成物を走らせる

```console
$ rulec test generated/ --lang ja
ok    shipping_fee (Python) ベクタ 68 件
ok    shipping_fee (Go) ベクタ 68 件
ok    丸めヘルパ (Python) 単体ベクタ
ok    丸めヘルパ (Go) 単体ベクタ

4 件すべて一致しました。
```

出力の「ベクタ」は、**規則の境界から自動で作ったテストケース**のことです。出どころが規則の境界であって、生成コードではないところが要点です。丸めヘルパにも専用のテストが付きます — 表ごとの一致だけを見ていると、端数の出ない表ではヘルパの誤りが隠れてしまうからです。

## Wasm: どの実行環境にも入る一つのモジュール

九つ目の生成対象は、言語の関数ではなくモジュールです。`wasm/` には、`rust/` と同じ Rust のモジュール、それを canonical ABI の `call: func(input: string) -> string` で包む crate root、その関数を world の export として名指しする `.wit`、`rulec test` が回す Node の runner が入ります。cargo もクレートも要らず、`rustc` だけで組めます。送料の規則で 40 KB、import は一つもありません。

```console
$ rustc --edition 2021 -C opt-level=s -C lto -C panic=abort -C strip=symbols \
    --target wasm32-unknown-unknown --crate-type cdylib shipping_fee_wasm.rs -o shipping_fee.wasm
```

ホストは、入力を JSON のオブジェクトにしてモジュールのメモリに書き、`call` を呼んで、記録の行を読み取ります。行の形はほかの言語の `_record` が書くものと同じで、契約の外の入力には `{"error":"…"}` が返ります。`.wit` があるので、`wasm-tools component new` でモジュールを変えずに component にでき、wasmtime のような component の実行環境からは `call("{…}")` の形で呼べます。ホストの書き方、component にする手順、`rulec api` の `wasm` の項は[生成物](generated-code.md#wasm)にあります。

`rulec test` はモジュールを組み、ほかの言語と同じようにベクタと突き合わせます。

```console
$ rulec test generated/ --lang ja
ok    shipping_fee (Rust) ベクタ 68 件
ok    shipping_fee (Rust, WASI) ベクタ 68 件
ok    shipping_fee (Wasm) ベクタ 68 件
…
```

二行目と三行目は別のものです。二行目は Rust の runner そのものを `wasm32-wasip1` 向けにコンパイルして wasmtime で走らせたもので、Fastly Compute や Spin のように標準入出力でやり取りする実行環境の形です。Shopify Functions が取るのは三行目のほう、名前の付いた関数を export する形です。どの実行環境がどちらの形を取り、その境界をどこで切るかは、[一覧に無い言語へ生成する](backends.md#a-wasm-host-shopify-functions)にまとめてあります。

## テストケースの側は足りているか

```console
$ rulec coverage rules/送料.rule
ベクタ 70 件
  行カバー                  7 / 7     満たす
  境界の両側カバー          4 / 4     満たす
  隠れ対カバー              3 / 3     満たす
  丸めの同着カバー          0 / 0     満たす
  畳み込みの遷移カバー      0 / 0     満たす
```

`coverage` は**テストの側に対する完全性検査**です。五つの条件は**規則のほうから先に**導きます。ベクタを見てから条件を決めるのでは、何も確かめたことになりません。欠けていればどの行・どの境界・どの隠れの対・どの丸めの同着・どの畳み込みの遷移かを名指しして 1 で落ちます。

丸めの同着は、刻みのちょうど半分ずれた値のことです。`half_up` と `half_down` の答えが分かれるのはこの一点だけなので、ここに載る入力が無いと、二つを入れ替えても期待値が変わりません。規則が実際に届く出力にだけ義務が立ちます。18.3% × 標準報酬月額は必ず偶数の円なので、折半額に端数が出ない厚生年金保険料は 0 / 0 です。

## もとの表とずれないように

生成物はコミットし、CI が作り直して突き合わせます。

```console
$ rulec gen rules/ --out generated/ --check
```

`--check` は何も書かず、生成物が新しい生成と違うか欠けていれば 1 で落ちます。**生成コードを手で直さないでください** — ヘッダに `DO NOT EDIT` と書いてあり、次の `gen` が上書きします。

整形は生成器が内蔵しています。後段で `gofmt` や `black` を走らせません — 環境に入っている道具の版に出力が依存した瞬間、生成が決定的でなくなるからです。代わりに `gofmt -l` が空であることと、`ruff check --select E,W` が行長を除いて無指摘であることをテストが確かめています。

---

[突き合わせと再生](compare.md){ .md-button .md-button--primary }
[形式](formats.md){ .md-button }
