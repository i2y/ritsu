<!-- sekisho <version> が claims.ja.gate（sha256:f26fbb951b602169）から生成したページです。読むためのもので、もとになるのは .gate のほうです。ここを編集しても .gate には戻せません。 -->
# 請求 v1

請求をだれが支払ってよいか。査定の係はどの請求も支払い、窓口のワークフローは、決まりが窓口に任せる請求を支払う。claims.gate の日本語の版

- ファイル：`claims.ja.gate`（gate 請求 v1、sha256:f26fbb951b602169）
- Cedar の名前空間：`ClaimsJa`
- sekisho：<version>

上のファイルと、それが読むファイルを sekisho <version> で検査して作ったページです。ファイルのハッシュが今のものと違えば、このページは古くなっています。

このファイルから `sekisho gen --target cedar --lang ja` が書く Cedar：

| ファイル | SHA-256（先頭 16 桁） |
|---|---|
| `cedar/claims_ja.cedar` | `<sha256>` |
| `cedar/claims_ja.cedarschema` | `<sha256>` |
| `cedar/claims_ja.cedarschema.json` | `<sha256>` |
| `cedar/claims_ja.policies.json` | `<sha256>` |

> [!NOTE]
> `sekisho check` は、action の組み合わせ 6 通りをすべて数え、どれも Cedar と同じ決まりで、許すか拒むかを決めました。`can` を書いた役割は、`can` に並べた action だけを許されます。

## だれが何をできるか

action ごとに、`sekisho check` が数えた組み合わせを、Cedar の答えと決めたポリシーが同じものどうしでまとめた表です。一つの列だけが違う行は一つにまとめ、その列にはまとめた値を書きます。その列がとるすべての値をまとめたときは「どれでも」、一つを除くすべてのときは「〜以外」と書きます。「-」は、その行には当てはまらない列です（ワークフローの役割など）。

### action `請求を支払う`（`pay_claim`）

請求を支払う

- 守る操作：`openapi "api/desk.json" operation payClaim`
- principal の型：`職員`、`Workflow`。resource の型：`請求`（ID は操作の引数 `claimId` から読む）
- 計算した値：`支払う人`（下の「計算した値」）
- 組み合わせは 6 通りで、そのうち 3 通りを許します。

#### 許す組み合わせ

| principal | 査定 | ワークフロー | 支払う人 | 決めたポリシー |
|---|---|---|---|---|
| 職員 | はい | - | どれでも | 査定は請求を支払える |
| Workflow | - | 支払い | 窓口 | ワークフローは窓口の請求を支払える |

#### 拒む組み合わせ

- 3 通りは、どの permit も当てはまらないので拒みます。

<details>
<summary>拒む行の表（2 行）</summary>

| principal | 査定 | ワークフロー | 支払う人 | 決めたポリシー |
|---|---|---|---|---|
| 職員 | いいえ | - | どれでも | 当てはまる permit なし |
| Workflow | - | 支払い | 担当者 | 当てはまる permit なし |

</details>

## ポリシー

ポリシーごとに、`.gate` に書いた行と、`sekisho gen --target cedar` が生成する Cedar を並べます。permit は、当てはまる組み合わせを許します。forbid は、当てはまる組み合わせを拒み、permit も当てはまるときは forbid が勝ちます。どのポリシーも当てはまらない組み合わせは、Cedar が拒みます。規則の答えを比べる条件は、`.gate` では `.rule` に書いた値の名前（`窓口`）で、Cedar では Cedar に渡す文字列（`"desk"`）で書きます。

### permit `査定は請求を支払える`（`adjusters_pay_claims`）

査定の係は、どの請求も支払える

- `請求を支払う` では 2 通りを許します。どれも、ほかの permit は許しません。

`claims.ja.gate`

```gate
permit 査定は請求を支払える(adjusters_pay_claims)
  description "査定の係は、どの請求も支払える"
  principal in 査定
  action 請求を支払う
```

`cedar/claims_ja.cedar`

```cedar
@id("claims_ja/adjusters_pay_claims")
@name("査定は請求を支払える")
@doc("査定の係は、どの請求も支払える")
permit (
  principal in ClaimsJa::Role::"adjuster",
  action == ClaimsJa::Action::"pay_claim",
  resource is ClaimsJa::Claim
);
```

### permit `ワークフローは窓口の請求を支払える`（`the_workflow_pays_the_desks_claims`）

ワークフローは、決まりが窓口に任せる請求を支払える

- `請求を支払う` では 1 通りを許します。ほかの permit は、それを許しません。

`claims.ja.gate`

```gate
permit ワークフローは窓口の請求を支払える(the_workflow_pays_the_desks_claims)
  description "ワークフローは、決まりが窓口に任せる請求を支払える"
  principal is workflow 支払い
  action 請求を支払う
  when 支払う人 is 窓口
```

`cedar/claims_ja.cedar`

```cedar
@id("claims_ja/the_workflow_pays_the_desks_claims")
@name("ワークフローは窓口の請求を支払える")
@doc("ワークフローは、決まりが窓口に任せる請求を支払える")
permit (
  principal == ClaimsJa::Workflow::"pay",
  action == ClaimsJa::Action::"pay_claim",
  resource is ClaimsJa::Claim
)
when { context.payer == "desk" };
```

## 計算した値

action が計算して、Cedar の context に入れる値です。生成したコードが、サービスのデータ、操作の引数、サーバーの時計から計算し、呼ぶ側からは受け取りません。規則と日付のページは、rulec と koyomi が描いた、人が読むページをそのまま載せています。

### `請求を支払う` の `支払う人`（`payer`）

- 書き方：`支払う人 = 支払う人(金額: resource.金額).だれ`
- `rulec "rules/支払う人.rule" output だれ` が、`resource.金額` から計算します。
- 値：`窓口`、`担当者`。Cedar に渡す文字列は `"desk"`、`"person"` です。

<details>
<summary><code>rulec doc</code> が描いた <code>rules/支払う人.rule</code> のページ</summary>

[the page rulec doc draws of rules/支払う人.rule]

</details>

## 役割

- `査定`（`adjuster`）：窓口が支払わない請求を支払う。`can` に `請求を支払う` を並べています。

役割を一つだけ持つ principal（その役割が含む役割も持つ）が、action を許されるかです。「いつも許す」はほかの値のどの組み合わせでも許すこと、「組み合わせによる」は許す組み合わせと拒む組み合わせの両方があること、「許さない」はどの組み合わせでも拒むことです。「-」は、action がその型の principal を取らないことを表します。役割に `can` を書くと、いつも許すか組み合わせによる action が `can` と同じであることを `sekisho check` が確かめます。

| principal | `請求を支払う` |
|---|---|
| `査定` を持つ `職員` | いつも許す |

## ワークフロー

ワークフローは、自分の資格で操作を呼びます。Cedar では principal `Workflow::"<名前>"` です。ワークフローごとに、どの action をどれだけ許されるかを並べます。

### ワークフロー `支払い`（`pay`）

決まりが窓口に任せる請求を支払い、ほかの請求は担当者に回す

- フロー：`dandori "flows/pay.ja.flow"`

| action | 許されるか |
|---|---|
| `請求を支払う`（`pay_claim`） | 組み合わせによる（2 通りのうち 1 通りを許す） |

#### フローが resource について持つ値（ゲートが書いた前提）

ワークフローの下の `resource` の行です。フローがその値を resource の ID として渡す呼び出しでは、resource の属性は、横に書いたフローの値と同じだとゲートが言っています。`ritsu check` は、フローの呼び出しをゲートが許すかを、この行を前提にして確かめます。

| 行 | resource | ID になるフローの値 | 属性 | フローの値 |
|---|---|---|---|---|
| 22 | `請求`（`Claim`） | `請求番号` | `金額` | `金額` |

> [!NOTE]
> これはゲートを書いた人が置いた前提です。サービスが読む resource が、フローの持つ値と同じであることは、どの検査も確かめていません。フローの値がその属性のとりうる値であることは、`ritsu sekisho check` が確かめます（E212）。

#### フローが呼ぶ操作

フローのタスクが呼ぶ操作と、それを守る action、その呼び出しでワークフローがその action を許されるかです。答えは、その呼び出しまで進んだ実行で起こりうる組み合わせの上のものです（呼び出しが渡す値と、途中の `match` が残す値）。`ritsu check` は、これを言語をまたいで確かめます。

| 行 | タスク | 操作 | 守る action | 許されるか | 拒まれたときのエラー |
|---|---|---|---|---|---|
| 30 | `請求を支払う` | `openapi "api/desk.json" operation payClaim` | `請求を支払う`（`pay_claim`） | いつも許す（ゲートの `resource` の行を前提に） | `拒否` |

呼び出しまで進んだ実行で決まっていること：

- 30 行目：`支払う人(金額: resource.金額).だれ`（フローの `r.だれ`）は `窓口`。

> [!NOTE]
> 「ゲートの `resource` の行を前提に」と書いた答えは、ワークフローの下の `resource` の行のとおりに、サービスが読む resource がフローの値を持つときの答えです。そのとおりであることは、どの検査も確かめていません。

## 守る操作

action が守る契約の操作です。サービスは、操作を行う前に、その action について Cedar に尋ねます。参照のパスはルートからです。

| action | 操作 | 呼び出し方 |
|---|---|---|
| `請求を支払う`（`pay_claim`） | `openapi "api/desk.json" operation payClaim` | `POST /claims/{claimId}/payments` |

どの action も、操作を守っています。
