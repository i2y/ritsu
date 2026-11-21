<!-- sekisho <version> が tests/walk/together/sums.ja.gate（sha256:f6e3181687f2abe1）から生成したページです。読むためのもので、もとになるのは .gate のほうです。ここを編集しても .gate には戻せません。 -->
# 合計 v1

二つの数の合計から出す二つの値。どの組が起こるかは、片方の数だけを動かしても分からない。入力の見つからなかった組は、起こるかもしれないものとして数える

- ファイル：`tests/walk/together/sums.ja.gate`（gate 合計 v1、sha256:f6e3181687f2abe1）
- Cedar の名前空間：`DocsJa`
- sekisho：<version>

上のファイルと、それが読むファイルを sekisho <version> で検査して作ったページです。ファイルのハッシュが今のものと違えば、このページは古くなっています。

このファイルから `sekisho gen --target cedar --lang ja` が書く Cedar：

| ファイル | SHA-256（先頭 16 桁） |
|---|---|
| `cedar/sums_ja.cedar` | `<sha256>` |
| `cedar/sums_ja.cedarschema` | `<sha256>` |
| `cedar/sums_ja.cedarschema.json` | `<sha256>` |
| `cedar/sums_ja.policies.json` | `<sha256>` |

> [!NOTE]
> `sekisho check` は、action の組み合わせ 4 通りをすべて数え、どれも Cedar と同じ決まりで、許すか拒むかを決めました。1 つの期待のうち 0 つが成り立ち、ほかは成り立つかを決められません。

## だれが何をできるか

action ごとに、`sekisho check` が数えた組み合わせを、Cedar の答えと決めたポリシーが同じものどうしでまとめた表です。一つの列だけが違う行は一つにまとめ、その列にはまとめた値を書きます。その列がとるすべての値をまとめたときは「どれでも」、一つを除くすべてのときは「〜以外」と書きます。「-」は、その行には当てはまらない列です（ワークフローの役割など）。

### action `読む`（`read_doc`）

文書を読む

- どの操作も守りません。
- principal の型：`利用者`。resource の型：`文書`
- 計算した値：`前`、`後`（下の「計算した値」）
- 組み合わせは 4 通りで、そのうち 3 通りを許します。

> [!NOTE]
> この action の組み合わせには、ほかの言語が起こるかどうかを言えなかった値を、起こるかもしれないものとして数えたものがあります。下の表にもその行が入っています。そのために決められなかった答えは、下の「検査の警告」にあります。

#### 許す組み合わせ

| principal | 前 | 後 | 決めたポリシー |
|---|---|---|---|
| 利用者 | 低 | 低 | 前が低なら読める |
| 利用者 | 低 | 高 | 前が低なら読める、後が高なら読める |
| 利用者 | 高 | 高 | 後が高なら読める |

#### 拒む組み合わせ

- 1 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（1 行）</summary>

| principal | 前 | 後 | 決めたポリシー |
|---|---|---|---|
| 利用者 | 高 | 低 | 当てはまる permit なし |

</details>

## ポリシー

ポリシーごとに、`.gate` に書いた行と、`sekisho gen --target cedar` が生成する Cedar を並べます。permit は、当てはまる組み合わせを許します。forbid は、当てはまる組み合わせを拒み、permit も当てはまるときは forbid が勝ちます。どのポリシーも当てはまらない組み合わせは、Cedar が拒みます。規則の答えを比べる条件は、`.gate` では `.rule` に書いた値の名前（`低`）で、Cedar では Cedar に渡す文字列（`"low"`）で書きます。

### permit `前が低なら読める`（`low_x_reads`）

前が低の人は読める

- `読む` では 2 通りを許します。そのうち 1 通りは、ほかの permit が許しません。

`tests/walk/together/sums.ja.gate`

```gate
permit 前が低なら読める(low_x_reads)
  description "前が低の人は読める"
  principal is 利用者
  action 読む
  when 前 is 低
```

`cedar/sums_ja.cedar`

```cedar
@id("sums_ja/low_x_reads")
@name("前が低なら読める")
@doc("前が低の人は読める")
permit (
  principal is DocsJa::User,
  action == DocsJa::Action::"read_doc",
  resource is DocsJa::Doc
)
when { context.x == "low" };
```

### permit `後が高なら読める`（`high_y_reads`）

後が高の人は読める

- `読む` では 2 通りを許します。そのうち 1 通りは、ほかの permit が許しません。

`tests/walk/together/sums.ja.gate`

```gate
permit 後が高なら読める(high_y_reads)
  description "後が高の人は読める"
  principal is 利用者
  action 読む
  when 後 is 高
```

`cedar/sums_ja.cedar`

```cedar
@id("sums_ja/high_y_reads")
@name("後が高なら読める")
@doc("後が高の人は読める")
permit (
  principal is DocsJa::User,
  action == DocsJa::Action::"read_doc",
  resource is DocsJa::Doc
)
when { context.y == "high" };
```

## 計算した値

action が計算して、Cedar の context に入れる値です。生成したコードが、サービスのデータ、操作の引数、サーバーの時計から計算し、呼ぶ側からは受け取りません。規則と日付のページは、rulec と koyomi が描いた、人が読むページをそのまま載せています。

### `読む` の `前`（`x`）

- 書き方：`前 = 合計の区分(甲: principal.一つ目, 乙: principal.二つ目).前`
- `rulec "tests/walk/together/合計の区分.rule" output 前` が、`principal.一つ目` と `principal.二つ目` から計算します。
- 値：`低`、`高`。Cedar に渡す文字列は `"low"`、`"high"` です。

<details>
<summary><code>rulec doc</code> が描いた <code>tests/walk/together/合計の区分.rule</code> のページ</summary>

[the page rulec doc draws of tests/walk/together/合計の区分.rule]

</details>

### `読む` の `後`（`y`）

- 書き方：`後 = 合計の区分(甲: principal.一つ目, 乙: principal.二つ目).後`
- `rulec "tests/walk/together/合計の区分.rule" output 後` が、`principal.一つ目` と `principal.二つ目` から計算します。
- 値：`低`、`高`。Cedar に渡す文字列は `"low"`、`"high"` です。

このファイルのページは、上の `前` のところに載せています。

## 期待、職務の分離、役割

### 期待

- `前が低なら後は高にならない`（`読む` で拒むことを期待）：合計が 100 以下なら 150 を超えないので、前が低で後が高の人はいない。成り立つかを決められません。

### 役割

役割を一つだけ持つ principal（その役割が含む役割も持つ）が、action を許されるかです。「いつも許す」はほかの値のどの組み合わせでも許すこと、「組み合わせによる」は許す組み合わせと拒む組み合わせの両方があること、「許さない」はどの組み合わせでも拒むことです。「-」は、action がその型の principal を取らないことを表します。役割に `can` を書くと、いつも許すか組み合わせによる action が `can` と同じであることを `sekisho check` が確かめます。

| principal | `読む` |
|---|---|
| `利用者` | 組み合わせによる |

## 守る操作

どの action も、契約の操作を守りません。

## 検査の警告

`sekisho check` の警告です。ファイルは検査を通っています。

```text
警告[W303]: tests/walk/together/sums.ja.gate:36:1: 期待 `前が低なら後は高にならない` が成り立つかどうかを決められません
    36 | expect deny 前が低なら後は高にならない
  = 規則が、問われた範囲で出力がとりうる値を正確に言えず、その値になる具体的な入力も見つかりませんでした。
```
