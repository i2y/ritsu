<!-- sekisho <version> が tests/walk/together/together.ja.gate（sha256:db7a9d54d9e57f6c）から生成したページです。読むためのもので、もとになるのは .gate のほうです。ここを編集しても .gate には戻せません。 -->
# 一緒に数える v1

同じデータを読む値。一つの呼び出しの二つの出力、二つの表の出力、同じ属性を渡す二つの規則、二つの入力に渡す一つの属性（属性で答えが変わる規則と、変わらない規則）、無いことのある属性。起こりうる組だけを数える

- ファイル：`tests/walk/together/together.ja.gate`（gate 一緒に数える v1、sha256:db7a9d54d9e57f6c）
- Cedar の名前空間：`DocsJa`
- sekisho：<version>

上のファイルと、それが読むファイルを sekisho <version> で検査して作ったページです。ファイルのハッシュが今のものと違えば、このページは古くなっています。

このファイルから `sekisho gen --target cedar --lang ja` が書く Cedar：

| ファイル | SHA-256（先頭 16 桁） |
|---|---|
| `cedar/together_ja.cedar` | `<sha256>` |
| `cedar/together_ja.cedarschema` | `<sha256>` |
| `cedar/together_ja.cedarschema.json` | `<sha256>` |
| `cedar/together_ja.policies.json` | `<sha256>` |

> [!NOTE]
> `sekisho check` は、6 つの action の組み合わせ 30 通りをすべて数え、どれも Cedar と同じ決まりで、許すか拒むかを決めました。9 つの期待は、どれも成り立ちます。そのうち 2 つは、どの組み合わせも選ばないので、何も確かめていません。

## だれが何をできるか

action ごとに、`sekisho check` が数えた組み合わせを、Cedar の答えと決めたポリシーが同じものどうしでまとめた表です。一つの列だけが違う行は一つにまとめ、その列にはまとめた値を書きます。その列がとるすべての値をまとめたときは「どれでも」、一つを除くすべてのときは「〜以外」と書きます。「-」は、その行には当てはまらない列です（ワークフローの役割など）。

### action `読む`（`read_doc`）

文書を読む。一つの呼び出しの二つの出力で、同じ表が一緒に書く

- どの操作も守りません。
- principal の型：`利用者`。resource の型：`文書`
- 計算した値：`甲`、`乙`（下の「計算した値」）
- 組み合わせは 6 通りで、そのうち 3 通りを許します。

#### 許す組み合わせ

| principal | 職員 | 金額 | 甲 | 乙 | 決めたポリシー |
|---|---|---|---|---|---|
| 利用者 | はい | どれでも | 低 | 低 | 甲が低なら読める |
| 利用者 | はい | >100JPY | 高 | 高 | 乙が高なら読める |

#### 拒む組み合わせ

- 3 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（2 行）</summary>

| principal | 職員 | 金額 | 甲 | 乙 | 決めたポリシー |
|---|---|---|---|---|---|
| 利用者 | いいえ | どれでも | 低 | 低 | 当てはまる permit なし |
| 利用者 | いいえ | >100JPY | 高 | 高 | 当てはまる permit なし |

</details>

### action `綴じる`（`file_doc`）

文書を綴じる。一つの呼び出しの二つの出力で、それぞれ別の表が書く

- どの操作も守りません。
- principal の型：`利用者`。resource の型：`文書`
- 計算した値：`甲`、`丙`（下の「計算した値」）
- 組み合わせは 6 通りで、そのうち 3 通りを許します。

#### 許す組み合わせ

| principal | 職員 | 甲 | 丙 | 決めたポリシー |
|---|---|---|---|---|
| 利用者 | はい | 低 | 低 | 甲が低なら綴じられる |
| 利用者 | はい | 低 | 高 | 甲が低なら綴じられる、丙が高なら綴じられる |
| 利用者 | はい | 高 | 高 | 丙が高なら綴じられる |

#### 拒む組み合わせ

- 3 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（2 行）</summary>

| principal | 職員 | 甲 | 丙 | 決めたポリシー |
|---|---|---|---|---|
| 利用者 | いいえ | 低 | 低 | 当てはまる permit なし |
| 利用者 | いいえ | どれでも | 高 | 当てはまる permit なし |

</details>

### action `送る`（`send_doc`）

文書を送る。二つの規則に同じ金額を渡す

- どの操作も守りません。
- principal の型：`利用者`。resource の型：`文書`
- 計算した値：`甲`、`段階`（下の「計算した値」）
- 組み合わせは 6 通りで、そのうち 3 通りを許します。

#### 許す組み合わせ

| principal | 職員 | 甲 | 段階 | 決めたポリシー |
|---|---|---|---|---|
| 利用者 | はい | 低 | 小口 | 甲が低なら送れる |
| 利用者 | はい | 低 | 大口 | 甲が低なら送れる、大口なら送れる |
| 利用者 | はい | 高 | 大口 | 大口なら送れる |

#### 拒む組み合わせ

- 3 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（2 行）</summary>

| principal | 職員 | 甲 | 段階 | 決めたポリシー |
|---|---|---|---|---|
| 利用者 | いいえ | 低 | 小口 | 当てはまる permit なし |
| 利用者 | いいえ | どれでも | 大口 | 当てはまる permit なし |

</details>

### action `量る`（`weigh_doc`）

文書を量る。一つの金額を、規則の二つの入力の両方に渡す

- どの操作も守りません。
- principal の型：`利用者`。resource の型：`文書`
- 計算した値：`どちら側`（下の「計算した値」）
- 組み合わせは 4 通りで、そのうち 2 通りを許します。

#### 許す組み合わせ

| principal | 職員 | どちら側 | 決めたポリシー |
|---|---|---|---|
| 利用者 | はい | どれでも | 片方ずつでなければ量れる |

#### 拒む組み合わせ

- 2 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（1 行）</summary>

| principal | 職員 | どちら側 | 決めたポリシー |
|---|---|---|---|
| 利用者 | いいえ | どれでも | 当てはまる permit なし |

</details>

### action `見比べる`（`compare_doc`）

文書をそれ自身と見比べる。一つの金額を、規則の二つの入力の両方に渡す。二つが同じ金額なら、規則の答えはどこでも同じになる

- どの操作も守りません。
- principal の型：`利用者`。resource の型：`文書`
- 計算した値：`側`（下の「計算した値」）
- 組み合わせは 2 通りで、そのうち 1 通りを許します。

#### 許す組み合わせ

| principal | 職員 | 側 | 決めたポリシー |
|---|---|---|---|
| 利用者 | はい | 同じ | 同じ側なら見比べられる |

#### 拒む組み合わせ

- 1 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（1 行）</summary>

| principal | 職員 | 側 | 決めたポリシー |
|---|---|---|---|
| 利用者 | いいえ | 同じ | 当てはまる permit なし |

</details>

### action `値を付ける`（`price_doc`）

文書に値を付ける。無いことのある金額から出す二つの出力

- どの操作も守りません。
- principal の型：`利用者`。resource の型：`文書`
- 計算した値：`甲`、`乙`（下の「計算した値」）
- 組み合わせは 6 通りで、そのうち 2 通りを許します。

#### 許す組み合わせ

| principal | 職員 | 甲 | 乙 | 決めたポリシー |
|---|---|---|---|---|
| 利用者 | はい | 低 | 低 | 甲が低なら値を付けられる |
| 利用者 | はい | 高 | 高 | 乙が高なら値を付けられる |

#### 拒む組み合わせ

- 4 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（3 行）</summary>

| principal | 職員 | 甲 | 乙 | 決めたポリシー |
|---|---|---|---|---|
| 利用者 | どれでも | - | - | 当てはまる permit なし |
| 利用者 | いいえ | 低 | 低 | 当てはまる permit なし |
| 利用者 | いいえ | 高 | 高 | 当てはまる permit なし |

</details>

## ポリシー

ポリシーごとに、`.gate` に書いた行と、`sekisho gen --target cedar` が生成する Cedar を並べます。permit は、当てはまる組み合わせを許します。forbid は、当てはまる組み合わせを拒み、permit も当てはまるときは forbid が勝ちます。どのポリシーも当てはまらない組み合わせは、Cedar が拒みます。規則の答えを比べる条件は、`.gate` では `.rule` に書いた値の名前（`低`）で、Cedar では Cedar に渡す文字列（`"low"`）で書きます。

### permit `甲が低なら読める`（`staff_read_low_a`）

職員は、甲が低の文書を読める

- `読む` では 2 通りを許します。どれも、ほかの permit は許しません。

`tests/walk/together/together.ja.gate`

```gate
permit 甲が低なら読める(staff_read_low_a)
  description "職員は、甲が低の文書を読める"
  principal in 職員
  action 読む
  when 甲 is 低
```

`cedar/together_ja.cedar`

```cedar
@id("together_ja/staff_read_low_a")
@name("甲が低なら読める")
@doc("職員は、甲が低の文書を読める")
permit (
  principal in DocsJa::Role::"staff",
  action == DocsJa::Action::"read_doc",
  resource is DocsJa::Doc
)
when { context.a == "low" };
```

### permit `乙が高なら読める`（`staff_read_high_b`）

職員は、乙が高の文書を読める

- `読む` では 1 通りを許します。ほかの permit は、それを許しません。

`tests/walk/together/together.ja.gate`

```gate
permit 乙が高なら読める(staff_read_high_b)
  description "職員は、乙が高の文書を読める"
  principal in 職員
  action 読む
  when 乙 is 高
```

`cedar/together_ja.cedar`

```cedar
@id("together_ja/staff_read_high_b")
@name("乙が高なら読める")
@doc("職員は、乙が高の文書を読める")
permit (
  principal in DocsJa::Role::"staff",
  action == DocsJa::Action::"read_doc",
  resource is DocsJa::Doc
)
when { context.b == "high" };
```

### permit `甲が低なら綴じられる`（`staff_file_low_a`）

職員は、甲が低の文書を綴じられる

- `綴じる` では 2 通りを許します。そのうち 1 通りは、ほかの permit が許しません。

`tests/walk/together/together.ja.gate`

```gate
permit 甲が低なら綴じられる(staff_file_low_a)
  description "職員は、甲が低の文書を綴じられる"
  principal in 職員
  action 綴じる
  when 甲 is 低
```

`cedar/together_ja.cedar`

```cedar
@id("together_ja/staff_file_low_a")
@name("甲が低なら綴じられる")
@doc("職員は、甲が低の文書を綴じられる")
permit (
  principal in DocsJa::Role::"staff",
  action == DocsJa::Action::"file_doc",
  resource is DocsJa::Doc
)
when { context.a == "low" };
```

### permit `丙が高なら綴じられる`（`staff_file_high_c`）

職員は、丙が高の文書を綴じられる

- `綴じる` では 2 通りを許します。そのうち 1 通りは、ほかの permit が許しません。

`tests/walk/together/together.ja.gate`

```gate
permit 丙が高なら綴じられる(staff_file_high_c)
  description "職員は、丙が高の文書を綴じられる"
  principal in 職員
  action 綴じる
  when 丙 is 高
```

`cedar/together_ja.cedar`

```cedar
@id("together_ja/staff_file_high_c")
@name("丙が高なら綴じられる")
@doc("職員は、丙が高の文書を綴じられる")
permit (
  principal in DocsJa::Role::"staff",
  action == DocsJa::Action::"file_doc",
  resource is DocsJa::Doc
)
when { context.c == "high" };
```

### permit `甲が低なら送れる`（`staff_send_low_a`）

職員は、甲が低の文書を送れる

- `送る` では 2 通りを許します。そのうち 1 通りは、ほかの permit が許しません。

`tests/walk/together/together.ja.gate`

```gate
permit 甲が低なら送れる(staff_send_low_a)
  description "職員は、甲が低の文書を送れる"
  principal in 職員
  action 送る
  when 甲 is 低
```

`cedar/together_ja.cedar`

```cedar
@id("together_ja/staff_send_low_a")
@name("甲が低なら送れる")
@doc("職員は、甲が低の文書を送れる")
permit (
  principal in DocsJa::Role::"staff",
  action == DocsJa::Action::"send_doc",
  resource is DocsJa::Doc
)
when { context.a == "low" };
```

### permit `大口なら送れる`（`staff_send_large`）

職員は、大口の文書を送れる

- `送る` では 2 通りを許します。そのうち 1 通りは、ほかの permit が許しません。

`tests/walk/together/together.ja.gate`

```gate
permit 大口なら送れる(staff_send_large)
  description "職員は、大口の文書を送れる"
  principal in 職員
  action 送る
  when 段階 is 大口
```

`cedar/together_ja.cedar`

```cedar
@id("together_ja/staff_send_large")
@name("大口なら送れる")
@doc("職員は、大口の文書を送れる")
permit (
  principal in DocsJa::Role::"staff",
  action == DocsJa::Action::"send_doc",
  resource is DocsJa::Doc
)
when { context.step == "large" };
```

### permit `片方ずつでなければ量れる`（`staff_weigh_what_is_not_mixed`）

職員は、二つの金額が同じ側にある文書を量れる

- `量る` では 2 通りを許します。どれも、ほかの permit は許しません。

`tests/walk/together/together.ja.gate`

```gate
permit 片方ずつでなければ量れる(staff_weigh_what_is_not_mixed)
  description "職員は、二つの金額が同じ側にある文書を量れる"
  principal in 職員
  action 量る
  unless どちら側 is 片方ずつ
```

`cedar/together_ja.cedar`

```cedar
@id("together_ja/staff_weigh_what_is_not_mixed")
@name("片方ずつでなければ量れる")
@doc("職員は、二つの金額が同じ側にある文書を量れる")
permit (
  principal in DocsJa::Role::"staff",
  action == DocsJa::Action::"weigh_doc",
  resource is DocsJa::Doc
)
unless { context.kind == "mixed" };
```

### permit `同じ側なら見比べられる`（`staff_compare_what_is_on_one_side`）

職員は、二つの金額が 500 円の同じ側にある文書を見比べられる

- `見比べる` では 1 通りを許します。ほかの permit は、それを許しません。

`tests/walk/together/together.ja.gate`

```gate
permit 同じ側なら見比べられる(staff_compare_what_is_on_one_side)
  description "職員は、二つの金額が 500 円の同じ側にある文書を見比べられる"
  principal in 職員
  action 見比べる
  when 側 is 同じ
```

`cedar/together_ja.cedar`

```cedar
@id("together_ja/staff_compare_what_is_on_one_side")
@name("同じ側なら見比べられる")
@doc("職員は、二つの金額が 500 円の同じ側にある文書を見比べられる")
permit (
  principal in DocsJa::Role::"staff",
  action == DocsJa::Action::"compare_doc",
  resource is DocsJa::Doc
)
when { context.side == "same" };
```

### permit `甲が低なら値を付けられる`（`staff_price_low_a`）

職員は、甲が低の文書に値を付けられる

- `値を付ける` では 1 通りを許します。ほかの permit は、それを許しません。

`tests/walk/together/together.ja.gate`

```gate
permit 甲が低なら値を付けられる(staff_price_low_a)
  description "職員は、甲が低の文書に値を付けられる"
  principal in 職員
  action 値を付ける
  when 甲 is 低
```

`cedar/together_ja.cedar`

```cedar
@id("together_ja/staff_price_low_a")
@name("甲が低なら値を付けられる")
@doc("職員は、甲が低の文書に値を付けられる")
permit (
  principal in DocsJa::Role::"staff",
  action == DocsJa::Action::"price_doc",
  resource is DocsJa::Doc
)
when { context has a && context.a == "low" };
```

### permit `乙が高なら値を付けられる`（`staff_price_high_b`）

職員は、乙が高の文書に値を付けられる

- `値を付ける` では 1 通りを許します。ほかの permit は、それを許しません。

`tests/walk/together/together.ja.gate`

```gate
permit 乙が高なら値を付けられる(staff_price_high_b)
  description "職員は、乙が高の文書に値を付けられる"
  principal in 職員
  action 値を付ける
  when 乙 is 高
```

`cedar/together_ja.cedar`

```cedar
@id("together_ja/staff_price_high_b")
@name("乙が高なら値を付けられる")
@doc("職員は、乙が高の文書に値を付けられる")
permit (
  principal in DocsJa::Role::"staff",
  action == DocsJa::Action::"price_doc",
  resource is DocsJa::Doc
)
when { context has b && context.b == "high" };
```

## 計算した値

action が計算して、Cedar の context に入れる値です。生成したコードが、サービスのデータ、操作の引数、サーバーの時計から計算し、呼ぶ側からは受け取りません。規則と日付のページは、rulec と koyomi が描いた、人が読むページをそのまま載せています。

### `読む` の `甲`（`a`）

- 書き方：`甲 = 金額の区分(金額: resource.金額).甲`
- `rulec "tests/walk/together/金額の区分.rule" output 甲` が、`resource.金額` から計算します。
- 値：`低`、`高`。Cedar に渡す文字列は `"low"`、`"high"` です。

<details>
<summary><code>rulec doc</code> が描いた <code>tests/walk/together/金額の区分.rule</code> のページ</summary>

[the page rulec doc draws of tests/walk/together/金額の区分.rule]

</details>

### `読む` の `乙`（`b`）

- 書き方：`乙 = 金額の区分(金額: resource.金額).乙`
- `rulec "tests/walk/together/金額の区分.rule" output 乙` が、`resource.金額` から計算します。
- 値：`低`、`高`。Cedar に渡す文字列は `"low"`、`"high"` です。

このファイルのページは、上の `甲` のところに載せています。

### `綴じる` の `甲`（`a`）

- 書き方：`甲 = 金額の区分(金額: resource.金額).甲`
- `rulec "tests/walk/together/金額の区分.rule" output 甲` が、`resource.金額` から計算します。
- 値：`低`、`高`。Cedar に渡す文字列は `"low"`、`"high"` です。

このファイルのページは、上の `甲` のところに載せています。

### `綴じる` の `丙`（`c`）

- 書き方：`丙 = 金額の区分(金額: resource.金額).丙`
- `rulec "tests/walk/together/金額の区分.rule" output 丙` が、`resource.金額` から計算します。
- 値：`低`、`高`。Cedar に渡す文字列は `"low"`、`"high"` です。

このファイルのページは、上の `甲` のところに載せています。

### `送る` の `甲`（`a`）

- 書き方：`甲 = 金額の区分(金額: resource.金額).甲`
- `rulec "tests/walk/together/金額の区分.rule" output 甲` が、`resource.金額` から計算します。
- 値：`低`、`高`。Cedar に渡す文字列は `"low"`、`"high"` です。

このファイルのページは、上の `甲` のところに載せています。

### `送る` の `段階`（`step`）

- 書き方：`段階 = 金額の段階(金額: resource.金額).段階`
- `rulec "tests/walk/together/金額の段階.rule" output 段階` が、`resource.金額` から計算します。
- 値：`小口`、`大口`。Cedar に渡す文字列は `"small"`、`"large"` です。

<details>
<summary><code>rulec doc</code> が描いた <code>tests/walk/together/金額の段階.rule</code> のページ</summary>

[the page rulec doc draws of tests/walk/together/金額の段階.rule]

</details>

### `量る` の `どちら側`（`kind`）

- 書き方：`どちら側 = 二つの金額(一つ目: resource.金額, 二つ目: resource.金額).どちら側`
- `rulec "tests/walk/together/二つの金額.rule" output どちら側` が、`resource.金額` から計算します。
- 値：`両方以下`、`両方超え`、`片方ずつ`。Cedar に渡す文字列は `"both_low"`、`"both_high"`、`"mixed"` です。

<details>
<summary><code>rulec doc</code> が描いた <code>tests/walk/together/二つの金額.rule</code> のページ</summary>

[the page rulec doc draws of tests/walk/together/二つの金額.rule]

</details>

### `見比べる` の `側`（`side`）

- 書き方：`側 = 同じ側か(一つ目: resource.金額, 二つ目: resource.金額).側`
- `rulec "tests/walk/together/同じ側か.rule" output 側` が、`resource.金額` から計算します。
- 値：`同じ`、`別`。Cedar に渡す文字列は `"same"`、`"apart"` です。

<details>
<summary><code>rulec doc</code> が描いた <code>tests/walk/together/同じ側か.rule</code> のページ</summary>

[the page rulec doc draws of tests/walk/together/同じ側か.rule]

</details>

### `値を付ける` の `甲`（`a`）

- 書き方：`甲 = 金額の区分(金額: resource.価格).甲`
- `rulec "tests/walk/together/金額の区分.rule" output 甲` が、`resource.価格` から計算します。
- 値：`低`、`高`。Cedar に渡す文字列は `"low"`、`"high"` です。

このファイルのページは、上の `甲` のところに載せています。

### `値を付ける` の `乙`（`b`）

- 書き方：`乙 = 金額の区分(金額: resource.価格).乙`
- `rulec "tests/walk/together/金額の区分.rule" output 乙` が、`resource.価格` から計算します。
- 値：`低`、`高`。Cedar に渡す文字列は `"low"`、`"high"` です。

このファイルのページは、上の `甲` のところに載せています。

## 期待、職務の分離、役割

### 期待

- `職員はどの文書も読める`（`読む` で許すことを期待）：甲と乙は同じ値なので、二つの permit のどちらかが必ず当てはまる。この期待が選ぶ 3 通りのすべてで成り立ちます。
- `職員は少額の文書を読める`（`読む` で許すことを期待）：職員は、100 円以下の文書を読める。この期待が選ぶ 1 通りのすべてで成り立ちます。
- `職員はどの文書も綴じられる`（`綴じる` で許すことを期待）：甲が高の所では丙も高なので、二つの permit のどちらかが必ず当てはまる。この期待が選ぶ 3 通りのすべてで成り立ちます。
- `職員はどの文書も送れる`（`送る` で許すことを期待）：500 円を超える文書は大口なので、二つの permit のどちらかが必ず当てはまる。この期待が選ぶ 3 通りのすべてで成り立ちます。
- `職員はどの文書も量れる`（`量る` で許すことを期待）：一つの金額が 500 円の両側にあることはない。この期待が選ぶ 2 通りのすべてで成り立ちます。
- `職員はどの文書も見比べられる`（`見比べる` で許すことを期待）：一つの金額は、どちらの側であれ、500 円の片側にある。この期待が選ぶ 1 通りのすべてで成り立ちます。
- `価格の無い文書には値を付けない`（`値を付ける` で拒むことを期待）：甲が低でなく乙が高でもないのは、どちらも無いときだけ。価格の無い文書である。この期待が選ぶ 2 通りのすべてで成り立ちます。
- `甲と乙は食い違わない`（`読む` で拒むことを期待）：規則は甲と乙に同じ値を出すので、これが選ぶ組み合わせは無い。どの組み合わせも選ばないので、成り立ちますが、何も確かめていません。
- `金額はそれ自身と別の側にはない`（`見比べる` で拒むことを期待）：一つの金額が 500 円の両側にあることはないので、これが選ぶ組み合わせは無い。どの組み合わせも選ばないので、成り立ちますが、何も確かめていません。

### 役割

- `職員`（`staff`）：職員のだれでも。

役割を一つだけ持つ principal（その役割が含む役割も持つ）が、action を許されるかです。「いつも許す」はほかの値のどの組み合わせでも許すこと、「組み合わせによる」は許す組み合わせと拒む組み合わせの両方があること、「許さない」はどの組み合わせでも拒むことです。「-」は、action がその型の principal を取らないことを表します。役割に `can` を書くと、いつも許すか組み合わせによる action が `can` と同じであることを `sekisho check` が確かめます。

| principal | `読む` | `綴じる` | `送る` | `量る` | `見比べる` | `値を付ける` |
|---|---|---|---|---|---|---|
| `職員` を持つ `利用者` | いつも許す | いつも許す | いつも許す | いつも許す | いつも許す | 組み合わせによる |

## 守る操作

どの action も、契約の操作を守りません。

## 検査の警告

`sekisho check` の警告です。ファイルは検査を通っています。

```text
警告[W308]: tests/walk/together/together.ja.gate:109:1: permit `片方ずつでなければ量れる` の行 `unless どちら側 is 片方ずつ` は、起こりうるどの組み合わせも外しません
   109 |   unless どちら側 is 片方ずつ
  = `principal` の行が当てはまる 2 通りの、どれでも満たされます。
  = `どちら側` が `片方ずつ` になることはありません。`どちら側` は `一つ目` と `二つ目` の両方に渡す `resource.金額` から計算する値で、ゲートが渡す値のどれにも、規則は `片方ずつ` を答えません。
  = これは規則が決めていることで、この行は、permit が当てはまる組み合わせを何も変えません。規則の答えを確かめたいなら規則の側に書き（`examples`。rulec が検査します）、この行は消すか、直してください。
警告[W308]: tests/walk/together/together.ja.gate:115:1: permit `同じ側なら見比べられる` の行 `when 側 is 同じ` は、起こりうるどの組み合わせも外しません
   115 |   when 側 is 同じ
  = `principal` の行が当てはまる 1 通りで満たされます。
  = `側` が `別` になることはありません。`側` は `一つ目` と `二つ目` の両方に渡す `resource.金額` から計算する値で、ゲートが渡す値のどれにも、規則は `別` を答えません。
  = これは規則が決めていることで、この行は、permit が当てはまる組み合わせを何も変えません。規則の答えを確かめたいなら規則の側に書き（`examples`。rulec が検査します）、この行は消すか、直してください。
警告[W304]: tests/walk/together/together.ja.gate:166:1: 期待 `甲と乙は食い違わない` は、どの組み合わせも選びません。成り立ちますが、何も確かめていません
   166 | expect deny 甲と乙は食い違わない
  = `principal` の行が当てはまる 3 通りのうち、`when 甲 is 低 and 乙 is 高` は 0 通りで満たされます。どの組み合わせでも満たされない行があります。
  = `甲` が `低` で、`乙` が `高` になることはありません。どちらも `resource.金額` から計算する値です。
  = これは規則が決めていることで、ポリシーをどう書いても成り立ちます。この期待は、ポリシーについて何も確かめていません。規則の答えを確かめたいなら規則の側に書き（`examples`。rulec が検査します）、この期待は消すか、行を直してください。
警告[W304]: tests/walk/together/together.ja.gate:172:1: 期待 `金額はそれ自身と別の側にはない` は、どの組み合わせも選びません。成り立ちますが、何も確かめていません
   172 | expect deny 金額はそれ自身と別の側にはない
  = `principal` の行が当てはまる 1 通りのうち、`when 側 is 別` は 0 通りで満たされます。どの組み合わせでも満たされない行があります。
  = `側` が `別` になることはありません。`側` は `一つ目` と `二つ目` の両方に渡す `resource.金額` から計算する値で、ゲートが渡す値のどれにも、規則は `別` を答えません。
  = これは規則が決めていることで、ポリシーをどう書いても成り立ちます。この期待は、ポリシーについて何も確かめていません。規則の答えを確かめたいなら規則の側に書き（`examples`。rulec が検査します）、この期待は消すか、行を直してください。
```
