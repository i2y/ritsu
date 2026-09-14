# 生成して呼ぶ

```console
$ rulec gen rules/ --out generated/
```

出るのは普通の Python モジュール、普通の TypeScript モジュール、普通の Go パッケージです。ランタイムも設定も要らず、標準ライブラリの外に依存もありません — 最後の一つは主張ではなく**検査された性質**です。`rulec test` が Go 側を `GOPROXY=off` で走らせています。

検査を通らない規則からは、何も生成されません。

## 対応する出力言語

いま対応しているのは Python・TypeScript・Rust・Ruby・Go の五つで、**Java・Kotlin・Swift・SQL に対応予定**です。同じ表から、フロントエンドとバックエンドとモバイルと DB が同じ答えを返すことを、いまある一致検査の仕組みでそのまま証明できるようにするのが狙いです。

| | 状態 | 要るもの |
|---|---|---|
| Python | 対応済み | `python3` |
| TypeScript | 対応済み | `node` だけ（ビルド手順も tsconfig も要りません） |
| Rust | 対応済み | `rustc` だけ（cargo もクレートも要りません） |
| Ruby | 対応済み | `ruby` 3.x か 4.x。`json` が標準添付なので gem は要りません |
| Go | 対応済み | `go` |
| Java | 対応予定 | JDK。単一ファイル実行でビルドツール無しに走らせられます |
| Kotlin | 対応予定 | kotlinc |
| Swift | 対応予定 | swiftc |
| SQL | 対応予定（形を検討中） | 行が分岐ではなく `CASE` の枝になるので、方言と出す形を先に決めます |

一つだけ決めていることがあります。**一致検査に乗らない言語は入れません。** 参照評価器とバイト単位で突き合わせられない生成物は、「証明済み」という看板の外側にあることになるからです。三つめの TypeScript を足すのに掛かった実コストは生成器に約 700 行で、五つめの Ruby も同じくらいでした。

### 一覧に無い言語へ生成する

この一覧に縛られる必要はありませんし、バックエンドが足されるのを待つ必要もありません。規則は、コードを組み立てるのに要るものをすでに全部出しています — `rulec api` が名前・単位・範囲・丸めを、`rulec schema` がやりとりする JSON の形を出すので、**自分のジェネレータで好きな言語に生成できます**。

**そのとき突き合わせも一緒に付いてきます。** 出したものをアダプタの規約で包めば、`rulec verify` が規則の境界から組み立てた全件を当ててくれます。上の四つとまったく同じ扱いで、rulec 側は一行も変わりません。

[ほかの言語へ生成する](backends.md)では、その一周を SQL（四つのどれでもない相手）で実際に通しています。88 件全件一致、そのあと閾値を一つわざと壊して、当てはまった行とそれを起こした入力を報告が名指しするところまで。

ページ自体は英語です（読む相手がエージェントなので、この種の資料は英語で書いています）。**人の側が知っておけばいいのは「頼めばできる」ということだけ**で、手順はエージェントがそのページを読んで進めます。

## 出るものの形

表の一行が分岐の一本になり、もとの表のセルがそのままコメントで添えてあります。

```python
def fee_demo(dest: Prefecture, girth: Cm, weight: Gram) -> YenInclTax:
    """Rule 送料例 v1. Each branch corresponds 1:1 to a row of the rule source."""
    if not _isinstance(dest, Prefecture):
        raise RuleInputError(f"あて先 is not a value of enum Prefecture: {dest!r}")
    if not 1 <= girth <= 100:
        raise RuleInputError(f"三辺合計 is out of range: {girth}")
    # table サイズ判定 (policy first)
    if girth <= 60:  # row 1: <=60cm | S60
        size = SizeClass.S60
    elif girth <= 80:  # row 2: <=80cm | S80
        size = SizeClass.S80
    elif True:  # row 3: - | S100
        size = SizeClass.S100
    else:
        raise AssertionError("unreachable: completeness was statically checked by rulec")
    ...
    return _round_up(fee, 10)
```

```go
func FeeDemo(in Input) (YenInclTax, error) {
	if !in.Dest.Valid() {
		return 0, fmt.Errorf("あて先 is not a value of the enum: %d", in.Dest)
	}
	// table サイズ判定 (policy first)
	var size SizeClass
	if int64(in.Girth) <= 60 { // row 1: <=60cm | S60
		size = SizeClassS60
	} else if int64(in.Girth) <= 80 { // row 2: <=80cm | S80
		size = SizeClassS80
	} else if true { // row 3: - | S100
		size = SizeClassS100
	} else {
		panic("unreachable: completeness was statically checked by rulec")
	}
	...
	return YenInclTax(roundUp(int64(fee), 10)), nil
}
```

読める形であることを、生成器は四つで守っています。

- **セルを省略しません。** 手前の分岐で真とわかる条件も書きます（`elif True:` はそのため）。もとの表の行と目で突き合わせられることが、生成物の唯一の読み方です。
- **単位は型に載せます。** Python は `NewType`、TypeScript は branded bigint、Go は defined type。`YenInclTax` と `YenExclTax` を取り違えるとコンパイルで止まります。
- **丸めは自前のヘルパで行います。** Python の `//` は −∞ 方向、Go の整数除算は 0 方向で食い違うので、言語の素の除算には任せません。
- **言語の組み込み関数をそのまま呼びません。** 入力のエイリアスが `min` や `list` でも壊れないよう、`_min` `_max` `_isinstance` を生成側に持っています。

参照評価器（rulec の中にある「正解」の実装）と生成した各言語が同じ答えを返すことは、境界から自動で作ったテストケースを全部に流し、**決まった形の JSON にしてバイト単位で**突き合わせて確かめています。

## 呼び方は、読まなくても分かる

```console
$ rulec api rules/クーポン一枚.rule | jq -r .python.signature
def coupon_step(subtotal: YenInclTax, applied: YenInclTax, kind: CouponKind, rate: Rate, face: YenInclTax, dup: bool) -> Output:
```

一つの JSON に、module と関数名、引数（和名・エイリアス・型・単位・範囲）、出力（丸めつき）、列挙の値の**その言語での綴り**（Python は `CouponKind.PERCENT`、Go は `couponstep.CouponKindPercent`）、投げられる例外が入っています。

手で書いた呼び出し規約は、生成器が名前を変えた日から静かに嘘になります。だからこの目録は生成器の隣で組み立て、テストが生成物そのものに縛ります — 目録が言う名前が生成ファイルにそのまま書かれていること、Python は import して `inspect.signature` と比べること、Go は**目録だけから呼び出しプログラムを組み立てて** `go vet` に通すこと（名前が一つでも違えばコンパイルが通りません）。

[生成物の詳しい説明](generated-code.md){ .md-button }

## 二つのガード

**入口のガード**は、証明が前提にしたことを実行時に守らせます。数値の入力は宣言した範囲に、列挙の入力はその値の集合に照らされます。これが無いと、宣言の外から呼ばれた側が静かに間違った数字を受け取ります — 完全性の証明は、宣言されていない入力については何も言っていません。

**矛盾のガード**は W114 の片割れです。`unique` の表の二行が重なりうるかを検査器が決められなかったところでは、生成コードは黙って一方を選ばずに止まります。

```python
# guard: W114 (table 適用判定, row 1 × row 2): a pair of rows whose exclusivity could not be proven statically
if 残高A <= 1000 and 残高B >= 3980:
    raise RuleContradictionError("table 適用判定: row 1 and row 2 matched at the same time")
```

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

## テストケースの側は足りているか

```console
$ rulec coverage rules/送料.rule
68 vectors
  row coverage              7 / 7     satisfied
  boundary-pair coverage    4 / 4     satisfied
  shadow-pair coverage      3 / 3     satisfied
```

`coverage` は**テストの側に対する完全性検査**です。三つの条件は**規則のほうから先に**導きます。ベクタを見てから条件を決めるのでは、何も確かめたことになりません。欠けていればどの行・どの境界・どの隠れの対かを名指しして 1 で落ちます。

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
