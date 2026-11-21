<!-- sekisho <version> が tests/doc/services.ja.gate（sha256:2b251928a96a9ac1）から生成したページです。読むためのもので、もとになるのは .gate のほうです。ここを編集しても .gate には戻せません。 -->
# サービス v1

人が文書を読み、サービスが索引を作る。どの action にも書いたポリシーが、当てはまる action と当てはまらない action に分かれる

- ファイル：`tests/doc/services.ja.gate`（gate サービス v1、sha256:2b251928a96a9ac1）
- Cedar の名前空間：`LibraryJa`
- sekisho：<version>

上のファイルと、それが読むファイルを sekisho <version> で検査して作ったページです。ファイルのハッシュが今のものと違えば、このページは古くなっています。

このファイルから `sekisho gen --target cedar --lang ja` が書く Cedar：

| ファイル | SHA-256（先頭 16 桁） |
|---|---|
| `cedar/services_ja.cedar` | `<sha256>` |
| `cedar/services_ja.cedarschema` | `<sha256>` |
| `cedar/services_ja.cedarschema.json` | `<sha256>` |
| `cedar/services_ja.policies.json` | `<sha256>` |

> [!NOTE]
> `sekisho check` は、4 つの action の組み合わせ 22 通りをすべて数え、どれも Cedar と同じ決まりで、許すか拒むかを決めました。

## だれが何をできるか

action ごとに、`sekisho check` が数えた組み合わせを、Cedar の答えと決めたポリシーが同じものどうしでまとめた表です。一つの列だけが違う行は一つにまとめ、その列にはまとめた値を書きます。その列がとるすべての値をまとめたときは「どれでも」、一つを除くすべてのときは「〜以外」と書きます。「-」は、その行には当てはまらない列です（ワークフローの役割など）。

### action `文書を読む`（`read_doc`）

文書を読む

- どの操作も守りません。
- principal の型：`利用者`。resource の型：`文書`
- 組み合わせは 8 通りで、そのうち 2 通りを許します。

#### 許す組み合わせ

| principal | 職員 | 停止中 | 公開 | 決めたポリシー |
|---|---|---|---|---|
| 利用者 | はい | いいえ | どれでも | 職員は読めて消せる |

#### 拒む組み合わせ

- `停止中は何もできない`（`when principal.停止中`）は 4 通りに当てはまり、どれも拒みます。そのうち 2 通りには permit も当てはまりますが、forbid が勝ちます。
- `停止中の職員は公開の物を見られない`（`principal in 職員`、`when resource.公開`、`when principal.停止中`）は 1 通りに当てはまり、それを拒みます。permit も当てはまりますが、forbid が勝ちます。
- 残る 2 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（4 行）</summary>

| principal | 職員 | 停止中 | 公開 | 決めたポリシー |
|---|---|---|---|---|
| 利用者 | どれでも | はい | いいえ | 停止中は何もできない |
| 利用者 | いいえ | はい | はい | 停止中は何もできない |
| 利用者 | はい | はい | はい | 停止中は何もできない、停止中の職員は公開の物を見られない |
| 利用者 | いいえ | いいえ | どれでも | 当てはまる permit なし |

</details>

### action `索引を作る`（`index_doc`）

文書の索引を作る

- どの操作も守りません。
- principal の型：`サービス`。resource の型：`文書`
- 組み合わせは 2 通りで、そのうち 1 通りを許します。

#### 許す組み合わせ

| principal | 公開 | 決めたポリシー |
|---|---|---|
| サービス | はい | サービスは索引を作れる |

#### 拒む組み合わせ

- `サービスは非公開の文書に触れない`（`principal is サービス`、`unless resource.公開`）は 1 通りに当てはまり、それを拒みます。permit は当てはまりません。

<details>
<summary>拒む行の表（1 行）</summary>

| principal | 公開 | 決めたポリシー |
|---|---|---|
| サービス | いいえ | サービスは非公開の文書に触れない |

</details>

### action `文書を消す`（`purge_doc`）

文書を完全に消す

- どの操作も守りません。
- principal の型：`利用者`。resource の型：`文書`
- だれにも許さない：文書は残しておく
- 組み合わせは 8 通りで、どれも許しません。

#### 許す組み合わせ

許す組み合わせはありません。

#### 拒む組み合わせ

- `停止中は何もできない`（`when principal.停止中`）は 4 通りに当てはまり、どれも拒みます。そのうち 2 通りには permit も当てはまりますが、forbid が勝ちます。
- `だれも消せない`（この action のすべての組み合わせ）は 8 通りに当てはまり、どれも拒みます。そのうち 4 通りには permit も当てはまりますが、forbid が勝ちます。

<details>
<summary>拒む行の表（2 行）</summary>

| principal | 職員 | 停止中 | 公開 | 決めたポリシー |
|---|---|---|---|---|
| 利用者 | どれでも | はい | どれでも | 停止中は何もできない、だれも消せない |
| 利用者 | どれでも | いいえ | どれでも | だれも消せない |

</details>

### action `フォルダを見る`（`list_folder`）

フォルダの中身を並べる

- どの操作も守りません。
- principal の型：`利用者`。resource の型：`フォルダ`
- 組み合わせは 4 通りで、そのうち 1 通りを許します。

#### 許す組み合わせ

| principal | 職員 | 停止中 | 決めたポリシー |
|---|---|---|---|
| 利用者 | はい | いいえ | 職員はフォルダを見られる |

#### 拒む組み合わせ

- `停止中は何もできない`（`when principal.停止中`）は 2 通りに当てはまり、どれも拒みます。そのうち 1 通りには permit も当てはまりますが、forbid が勝ちます。
- 残る 1 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（2 行）</summary>

| principal | 職員 | 停止中 | 決めたポリシー |
|---|---|---|---|
| 利用者 | どれでも | はい | 停止中は何もできない |
| 利用者 | いいえ | いいえ | 当てはまる permit なし |

</details>

## ポリシー

ポリシーごとに、`.gate` に書いた行と、`sekisho gen --target cedar` が生成する Cedar を並べます。permit は、当てはまる組み合わせを許します。forbid は、当てはまる組み合わせを拒み、permit も当てはまるときは forbid が勝ちます。どのポリシーも当てはまらない組み合わせは、Cedar が拒みます。

### permit `職員は読めて消せる`（`staff_read_and_purge`）

職員は文書を読める。消すこともできることにしてある

- `文書を読む` では 2 通りを許します。どれも、ほかの permit は許しません。
- `文書を消す` では 4 通りに当てはまりますが、forbid がどれも拒むので、許す組み合わせはありません。

`tests/doc/services.ja.gate`

```gate
permit 職員は読めて消せる(staff_read_and_purge)
  description "職員は文書を読める。消すこともできることにしてある"
  principal in 職員
  action 文書を読む, 文書を消す
```

`cedar/services_ja.cedar`

```cedar
@id("services_ja/staff_read_and_purge")
@name("職員は読めて消せる")
@doc("職員は文書を読める。消すこともできることにしてある")
permit (
  principal in LibraryJa::Role::"staff",
  action in [LibraryJa::Action::"read_doc", LibraryJa::Action::"purge_doc"],
  resource is LibraryJa::Doc
);
```

### permit `サービスは索引を作れる`（`services_index`）

サービスは、公開の文書の索引を作れる

- `索引を作る` では 1 通りを許します。ほかの permit は、それを許しません。

`tests/doc/services.ja.gate`

```gate
permit サービスは索引を作れる(services_index)
  description "サービスは、公開の文書の索引を作れる"
  principal is サービス
  action 索引を作る
  when resource.公開
```

`cedar/services_ja.cedar`

```cedar
@id("services_ja/services_index")
@name("サービスは索引を作れる")
@doc("サービスは、公開の文書の索引を作れる")
permit (
  principal is LibraryJa::Service,
  action == LibraryJa::Action::"index_doc",
  resource is LibraryJa::Doc
)
when { resource.open_to_all };
```

### permit `職員はフォルダを見られる`（`staff_list`）

職員はフォルダを見られる

- `フォルダを見る` では 1 通りを許します。ほかの permit は、それを許しません。

`tests/doc/services.ja.gate`

```gate
permit 職員はフォルダを見られる(staff_list)
  description "職員はフォルダを見られる"
  principal in 職員
  action フォルダを見る
```

`cedar/services_ja.cedar`

```cedar
@id("services_ja/staff_list")
@name("職員はフォルダを見られる")
@doc("職員はフォルダを見られる")
permit (
  principal in LibraryJa::Role::"staff",
  action == LibraryJa::Action::"list_folder",
  resource is LibraryJa::Folder
);
```

### forbid `停止中は何もできない`（`suspended_do_nothing`）

停止中の人は何もできない

- `文書を読む` では 4 通りに当てはまり、どれも拒みます。そのうち 2 通りには permit も当てはまります。
- `索引を作る` では、どの組み合わせにも当てはまりません。`when principal.停止中` の行は、この action のどの組み合わせでも満たされません。
- `文書を消す` では 4 通りに当てはまり、どれも拒みます。そのうち 2 通りには permit も当てはまります。
- `フォルダを見る` では 2 通りに当てはまり、どれも拒みます。そのうち 1 通りには permit も当てはまります。

`tests/doc/services.ja.gate`

```gate
forbid 停止中は何もできない(suspended_do_nothing)
  description "停止中の人は何もできない"
  action any
  when principal.停止中
```

`cedar/services_ja.cedar`

```cedar
@id("services_ja/suspended_do_nothing")
@name("停止中は何もできない")
@doc("停止中の人は何もできない")
forbid (
  principal,
  action in
    [LibraryJa::Action::"read_doc",
     LibraryJa::Action::"index_doc",
     LibraryJa::Action::"purge_doc",
     LibraryJa::Action::"list_folder"],
  resource
)
when { principal has suspended && principal.suspended };
```

### forbid `サービスは非公開の文書に触れない`（`services_skip_closed_docs`）

サービスは、公開でない文書に触れない

- `文書を読む` では、どの組み合わせにも当てはまりません。`文書を読む` の principal（`利用者`）は、`principal is サービス` の行を満たしません。
- `索引を作る` では 1 通りに当てはまり、それを拒みます。permit は当てはまりません。
- `文書を消す` では、どの組み合わせにも当てはまりません。`文書を消す` の principal（`利用者`）は、`principal is サービス` の行を満たしません。
- `フォルダを見る` では、どの組み合わせにも当てはまりません。`フォルダを見る` の principal（`利用者`）は、`principal is サービス` の行を満たしません。

`tests/doc/services.ja.gate`

```gate
forbid サービスは非公開の文書に触れない(services_skip_closed_docs)
  description "サービスは、公開でない文書に触れない"
  principal is サービス
  action any
  unless resource.公開
```

`cedar/services_ja.cedar`

```cedar
@id("services_ja/services_skip_closed_docs")
@name("サービスは非公開の文書に触れない")
@doc("サービスは、公開でない文書に触れない")
forbid (
  principal is LibraryJa::Service,
  action in
    [LibraryJa::Action::"read_doc",
     LibraryJa::Action::"index_doc",
     LibraryJa::Action::"purge_doc",
     LibraryJa::Action::"list_folder"],
  resource
)
unless { resource has open_to_all && resource.open_to_all };
```

### forbid `だれも消せない`（`no_one_purges`）

文書は残しておく

- `文書を消す` では 8 通りに当てはまり、どれも拒みます。そのうち 4 通りには permit も当てはまります。

`tests/doc/services.ja.gate`

```gate
forbid だれも消せない(no_one_purges)
  description "文書は残しておく"
  action 文書を消す
```

`cedar/services_ja.cedar`

```cedar
@id("services_ja/no_one_purges")
@name("だれも消せない")
@doc("文書は残しておく")
forbid (
  principal,
  action == LibraryJa::Action::"purge_doc",
  resource is LibraryJa::Doc
);
```

### forbid `停止中の職員は公開の物を見られない`（`suspended_staff_see_nothing_open`）

停止中の職員は、公開の物を何も見られない

- `文書を読む` では 1 通りに当てはまり、それを拒みます。permit も当てはまります。
- `フォルダを見る` では、どの組み合わせにも当てはまりません。`principal` の行は 2 通りで満たされますが、`when` と `unless` の行が、そこで同時に満たされることがありません。

`tests/doc/services.ja.gate`

```gate
forbid 停止中の職員は公開の物を見られない(suspended_staff_see_nothing_open)
  description "停止中の職員は、公開の物を何も見られない"
  principal in 職員
  action 文書を読む, フォルダを見る
  when resource.公開
  when principal.停止中
```

`cedar/services_ja.cedar`

```cedar
@id("services_ja/suspended_staff_see_nothing_open")
@name("停止中の職員は公開の物を見られない")
@doc("停止中の職員は、公開の物を何も見られない")
forbid (
  principal in LibraryJa::Role::"staff",
  action in [LibraryJa::Action::"read_doc", LibraryJa::Action::"list_folder"],
  resource
)
when { resource has open_to_all && resource.open_to_all }
when { principal.suspended };
```

## 役割

- `職員`（`staff`）：職員のだれでも。

役割を一つだけ持つ principal（その役割が含む役割も持つ）が、action を許されるかです。「いつも許す」はほかの値のどの組み合わせでも許すこと、「組み合わせによる」は許す組み合わせと拒む組み合わせの両方があること、「許さない」はどの組み合わせでも拒むことです。「-」は、action がその型の principal を取らないことを表します。役割に `can` を書くと、いつも許すか組み合わせによる action が `can` と同じであることを `sekisho check` が確かめます。

| principal | `文書を読む` | `索引を作る` | `文書を消す` | `フォルダを見る` |
|---|---|---|---|---|
| `職員` を持つ `利用者` | 組み合わせによる | - | 許さない | 組み合わせによる |
| `サービス` | - | 組み合わせによる | - | - |

## 守る操作

どの action も、契約の操作を守りません。

## 検査の警告

`sekisho check` の警告です。ファイルは検査を通っています。

```text
警告[W306]: tests/doc/services.ja.gate:46:1: permit `職員は読めて消せる` は action `文書を消す` を挙げていますが、`文書を消す` では何も許しません。許すはずの組み合わせを、どれも forbid が拒みます
    46 | permit 職員は読めて消せる(staff_read_and_purge)
  = `文書を消す` で許すはずの組み合わせは 4 通りで、`だれも消せない` が 4 通り、`停止中は何もできない` が 2 通りを拒みます。
  = 例：職員を持つ利用者（停止中：いいえ）、文書（公開：いいえ）。`だれも消せない` が拒みます。
  = ほかに挙げた action では、`文書を読む` の 2 通りを許します。
  = `action` の行から `文書を消す` を外すか、`文書を消す` も許すつもりなら forbid を狭めてください。
警告[W307]: tests/doc/services.ja.gate:55:1: permit `サービスは索引を作れる` の行 `when resource.公開` は、どの決定も変えません。この行が外す組み合わせを、どれも forbid が拒みます
    55 |   when resource.公開
  = この行が無ければ、permit は、この行だけが満たされない 1 通りにも当てはまります。その組み合わせは、`サービスは非公開の文書に触れない` が拒みます。
  = 例：サービス、文書（公開：いいえ）。`サービスは非公開の文書に触れない` が拒みます。
  = 行を消しても、答えは変わりません。行を消すか、ほかの組み合わせを外すつもりだったなら、行を直してください。
警告[W305]: tests/doc/services.ja.gate:77:1: forbid `停止中の職員は公開の物を見られない` は action `フォルダを見る` を挙げていますが、`フォルダを見る` のどの組み合わせにも当てはまりません
    77 | forbid 停止中の職員は公開の物を見られない(suspended_staff_see_nothing_open)
  = `principal` の行が当てはまる 2 通りのうち、`when resource.公開` は 0 通り、`when principal.停止中` は 1 通りで満たされます。どの組み合わせでも満たされない行があります。
  = ほかに挙げた action では、`文書を読む` の 1 通りに当てはまります。
  = `action` の行から `フォルダを見る` を外すか、`フォルダを見る` にも当てはまるよう行を直してください。
```
