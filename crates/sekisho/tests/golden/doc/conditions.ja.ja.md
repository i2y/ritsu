<!-- sekisho <version> が tests/gen/conditions.ja.gate（sha256:9c1cc4811e4cc4f8）から生成したページです。読むためのもので、もとになるのは .gate のほうです。ここを編集しても .gate には戻せません。 -->
# 条件 v1

Cedar の生成器が書く条件の形を、全部並べたもの。無いことがある属性、一部の型だけが持つ属性、二つの属性の比べ、グループのメンバー、not、or、二つの resource の型、全部の action に効く forbid。conditions.gate の日本語の版

- ファイル：`tests/gen/conditions.ja.gate`（gate 条件 v1、sha256:9c1cc4811e4cc4f8）
- Cedar の名前空間：`LabJa`
- sekisho：<version>

上のファイルと、それが読むファイルを sekisho <version> で検査して作ったページです。ファイルのハッシュが今のものと違えば、このページは古くなっています。

このファイルから `sekisho gen --target cedar --lang ja` が書く Cedar：

| ファイル | SHA-256（先頭 16 桁） |
|---|---|
| `cedar/conditions_ja.cedar` | `<sha256>` |
| `cedar/conditions_ja.cedarschema` | `<sha256>` |
| `cedar/conditions_ja.cedarschema.json` | `<sha256>` |
| `cedar/conditions_ja.policies.json` | `<sha256>` |

> [!NOTE]
> `sekisho check` は、2 つの action の組み合わせ 1,444 通りをすべて数え、どれも Cedar と同じ決まりで、許すか拒むかを決めました。

## だれが何をできるか

action ごとに、`sekisho check` が数えた組み合わせを、Cedar の答えと決めたポリシーが同じものどうしでまとめた表です。一つの列だけが違う行は一つにまとめ、その列にはまとめた値を書きます。その列がとるすべての値をまとめたときは「どれでも」、一つを除くすべてのときは「〜以外」と書きます。「-」は、その行には当てはまらない列です（ワークフローの役割など）。

### action `読む`（`read`）

文書かフォルダーを読む

- どの操作も守りません。
- principal の型：`利用者`、`ボット`。resource の型：`文書`、`フォルダー`
- input `急ぎ`（`urgent`）：`bool`
- input `ページ数`（`pages`）：`number`、1〜500
- 組み合わせは 1,300 通りで、そのうち 234 通りを許します。

#### 許す組み合わせ

| principal | 編集者 | 閲覧者 | principal.部署 | 休職中 | resource | resource.部署 | 保管済 | resource.テナント is principal.テナント | 急ぎ | ページ数 | 決めたポリシー |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 利用者 | はい | いいえ | - | はい以外 | フォルダー | - | - | はい | どれでも | どれでも | 編集者と閲覧者は読める |
| 利用者 | はい | いいえ | どれでも | はい以外 | 文書 | どれでも | はい以外 | はい | どれでも | どれでも | 編集者と閲覧者は読める |
| 利用者 | どれでも | はい | - | はい以外 | フォルダー | - | - | はい | どれでも | どれでも | 編集者と閲覧者は読める |
| 利用者 | どれでも | はい | どれでも | はい以外 | 文書 | どれでも | はい以外 | はい | どれでも | どれでも | 編集者と閲覧者は読める |
| ボット | - | - | 営業 | - | 文書 | 営業 | どれでも | - | はい | >20 | ボットは自分の部署の文書を読める |
| ボット | - | - | 営業 | - | 文書 | 営業 | どれでも | - | どれでも | <=20 | ボットは自分の部署の文書を読める |
| ボット | - | - | サポート | - | 文書 | サポート | どれでも | - | はい | >20 | ボットは自分の部署の文書を読める |
| ボット | - | - | サポート | - | 文書 | サポート | どれでも | - | どれでも | <=20 | ボットは自分の部署の文書を読める |

#### 拒む組み合わせ

- `休職中の人は何もできない`（`when principal.休職中 is true`）は 416 通りに当てはまり、どれも拒みます。そのうち 108 通りには permit も当てはまりますが、forbid が勝ちます。
- 残る 650 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（13 行）</summary>

| principal | 編集者 | 閲覧者 | principal.部署 | 休職中 | resource | resource.部署 | 保管済 | resource.テナント is principal.テナント | 急ぎ | ページ数 | 決めたポリシー |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 利用者 | どれでも | どれでも | - | はい | フォルダー | - | - | どれでも | どれでも | どれでも | 休職中の人は何もできない |
| 利用者 | どれでも | どれでも | どれでも | はい | 文書 | どれでも | どれでも | どれでも | どれでも | どれでも | 休職中の人は何もできない |
| 利用者 | どれでも | どれでも | - | はい以外 | フォルダー | - | - | いいえ | どれでも | どれでも | 当てはまる permit なし |
| 利用者 | どれでも | どれでも | どれでも | はい以外 | 文書 | どれでも | はい | はい | どれでも | どれでも | 当てはまる permit なし |
| 利用者 | どれでも | どれでも | どれでも | はい以外 | 文書 | どれでも | どれでも | いいえ | どれでも | どれでも | 当てはまる permit なし |
| 利用者 | いいえ | いいえ | - | はい以外 | フォルダー | - | - | はい | どれでも | どれでも | 当てはまる permit なし |
| 利用者 | いいえ | いいえ | どれでも | はい以外 | 文書 | どれでも | はい以外 | はい | どれでも | どれでも | 当てはまる permit なし |
| ボット | - | - | - | - | フォルダー | - | - | - | どれでも | どれでも | 当てはまる permit なし |
| ボット | - | - | 営業 | - | 文書 | サポート | どれでも | - | はい | >20 | 当てはまる permit なし |
| ボット | - | - | 営業 | - | 文書 | サポート | どれでも | - | どれでも | <=20 | 当てはまる permit なし |
| ボット | - | - | どれでも | - | 文書 | どれでも | どれでも | - | いいえ | >20 | 当てはまる permit なし |
| ボット | - | - | サポート | - | 文書 | 営業 | どれでも | - | はい | >20 | 当てはまる permit なし |
| ボット | - | - | サポート | - | 文書 | 営業 | どれでも | - | どれでも | <=20 | 当てはまる permit なし |

</details>

### action `直す`（`edit`）

文書を直す

- どの操作も守りません。
- principal の型：`利用者`。resource の型：`文書`
- 組み合わせは 144 通りで、そのうち 24 通りを許します。

#### 許す組み合わせ

| principal | 編集者 | クリアランス | 休職中 | 機密度 | resource.確かめる人 is principal | principal in resource.チーム | 決めたポリシー |
|---|---|---|---|---|---|---|---|
| 利用者 | はい | どれでも | はい以外 | 低 | はい | いいえ | 編集者は直せる |
| 利用者 | はい | どれでも | はい以外 | 低 | どれでも | はい | 編集者は直せる |
| 利用者 | はい | >2 | はい以外 | 高 | はい | いいえ | 編集者は直せる |
| 利用者 | はい | >2 | はい以外 | 高 | どれでも | はい | 編集者は直せる |

#### 拒む組み合わせ

- `休職中の人は何もできない`（`when principal.休職中 is true`）は 48 通りに当てはまり、どれも拒みます。そのうち 12 通りには permit も当てはまりますが、forbid が勝ちます。
- 残る 72 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（8 行）</summary>

| principal | 編集者 | クリアランス | 休職中 | 機密度 | resource.確かめる人 is principal | principal in resource.チーム | 決めたポリシー |
|---|---|---|---|---|---|---|---|
| 利用者 | どれでも | どれでも | はい | どれでも | どれでも | どれでも | 休職中の人は何もできない |
| 利用者 | どれでも | <=2 | はい以外 | 高 | はい | いいえ | 当てはまる permit なし |
| 利用者 | どれでも | <=2 | はい以外 | 高 | どれでも | はい | 当てはまる permit なし |
| 利用者 | どれでも | どれでも | はい以外 | どれでも | いいえ | いいえ | 当てはまる permit なし |
| 利用者 | いいえ | どれでも | はい以外 | 低 | はい | いいえ | 当てはまる permit なし |
| 利用者 | いいえ | どれでも | はい以外 | 低 | どれでも | はい | 当てはまる permit なし |
| 利用者 | いいえ | >2 | はい以外 | 高 | はい | いいえ | 当てはまる permit なし |
| 利用者 | いいえ | >2 | はい以外 | 高 | どれでも | はい | 当てはまる permit なし |

</details>

## ポリシー

ポリシーごとに、`.gate` に書いた行と、`sekisho gen --target cedar` が生成する Cedar を並べます。permit は、当てはまる組み合わせを許します。forbid は、当てはまる組み合わせを拒み、permit も当てはまるときは forbid が勝ちます。どのポリシーも当てはまらない組み合わせは、Cedar が拒みます。

### permit `編集者と閲覧者は読める`（`readers_read`）

編集者と閲覧者は、同じテナントの、保管済でないものを読める

- `読む` では 216 通りを許します。どれも、ほかの permit は許しません。

`tests/gen/conditions.ja.gate`

```gate
permit 編集者と閲覧者は読める(readers_read)
  description "編集者と閲覧者は、同じテナントの、保管済でないものを読める"
  principal in 編集者, 閲覧者
  action 読む
  when resource.テナント is principal.テナント
  unless resource.保管済
```

`cedar/conditions_ja.cedar`

```cedar
@id("conditions_ja/readers_read")
@name("編集者と閲覧者は読める")
@doc("編集者と閲覧者は、同じテナントの、保管済でないものを読める")
permit (
  principal,
  action == LabJa::Action::"read",
  resource
)
when
{ principal in LabJa::Role::"editor" || principal in LabJa::Role::"viewer" }
when { resource.tenant == principal.tenant }
unless { resource has archived && resource.archived };
```

### permit `ボットは自分の部署の文書を読める`（`bots_read_their_department`）

ボットは、自分の部署の文書を、20 ページまでか急ぎなら読める

- `読む` では 18 通りを許します。どれも、ほかの permit は許しません。

`tests/gen/conditions.ja.gate`

```gate
permit ボットは自分の部署の文書を読める(bots_read_their_department)
  description "ボットは、自分の部署の文書を、20 ページまでか急ぎなら読める"
  principal is ボット
  action 読む
  when principal.部署 is resource.部署
  when ページ数 <= 20 or 急ぎ
```

`cedar/conditions_ja.cedar`

```cedar
@id("conditions_ja/bots_read_their_department")
@name("ボットは自分の部署の文書を読める")
@doc("ボットは、自分の部署の文書を、20 ページまでか急ぎなら読める")
permit (
  principal is LabJa::Bot,
  action == LabJa::Action::"read",
  resource
)
when { resource has dept && principal.dept == resource.dept }
when { context.pages <= 20 || context.urgent };
```

### permit `編集者は直せる`（`editors_edit`）

文書のチームにいるか、文書を確かめる編集者は、文書の機密度が高くないか、クリアランスが 3 以上なら直せる

- `直す` では 24 通りを許します。どれも、ほかの permit は許しません。

`tests/gen/conditions.ja.gate`

```gate
permit 編集者は直せる(editors_edit)
  description "文書のチームにいるか、文書を確かめる編集者は、文書の機密度が高くないか、クリアランスが 3 以上なら直せる"
  principal in 編集者
  action 直す
  when principal in resource.チーム or resource.確かめる人 is principal
  when resource.機密度 is not 高 or principal.クリアランス >= 3
```

`cedar/conditions_ja.cedar`

```cedar
@id("conditions_ja/editors_edit")
@name("編集者は直せる")
@doc("文書のチームにいるか、文書を確かめる編集者は、文書の機密度が高くないか、クリアランスが 3 以上なら直せる")
permit (
  principal in LabJa::Role::"editor",
  action == LabJa::Action::"edit",
  resource is LabJa::Doc
)
when
{
  principal in resource.team ||
  resource has reviewer &&
  resource.reviewer == principal
}
when { !(resource.level == "high") || principal.clearance >= 3 };
```

### forbid `休職中の人は何もできない`（`nobody_on_leave`）

休職中の人は何もできない

- `読む` では 416 通りに当てはまり、どれも拒みます。そのうち 108 通りには permit も当てはまります。
- `直す` では 48 通りに当てはまり、どれも拒みます。そのうち 12 通りには permit も当てはまります。

`tests/gen/conditions.ja.gate`

```gate
forbid 休職中の人は何もできない(nobody_on_leave)
  description "休職中の人は何もできない"
  action any
  when principal.休職中 is true
```

`cedar/conditions_ja.cedar`

```cedar
@id("conditions_ja/nobody_on_leave")
@name("休職中の人は何もできない")
@doc("休職中の人は何もできない")
forbid (
  principal,
  action in [LabJa::Action::"read", LabJa::Action::"edit"],
  resource
)
when { principal has on_leave && principal.on_leave == true };
```

## 役割

- `編集者`（`editor`）：文書を直す。
- `閲覧者`（`viewer`）：文書を読む。

役割を一つだけ持つ principal（その役割が含む役割も持つ）が、action を許されるかです。「いつも許す」はほかの値のどの組み合わせでも許すこと、「組み合わせによる」は許す組み合わせと拒む組み合わせの両方があること、「許さない」はどの組み合わせでも拒むことです。「-」は、action がその型の principal を取らないことを表します。役割に `can` を書くと、いつも許すか組み合わせによる action が `can` と同じであることを `sekisho check` が確かめます。

| principal | `読む` | `直す` |
|---|---|---|
| `編集者` を持つ `利用者` | 組み合わせによる | 組み合わせによる |
| `閲覧者` を持つ `利用者` | 組み合わせによる | 許さない |
| `ボット` | 組み合わせによる | - |

## 守る操作

どの action も、契約の操作を守りません。
