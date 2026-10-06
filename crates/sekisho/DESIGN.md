# sekisho 設計文書

認可を書く小さな言語。ファイルは `.gate`、コマンドは `sekisho`。だれ（`principal`）が、何（`resource`）に、何をして（`action`）よいかを、役割・属性・関係と、rulec の規則と koyomi の日付の答えを条件にして書く。宣言した範囲のすべての組み合わせで確かめ、Cedar のスキーマとポリシー、Cedar に渡すリクエストを組み立てるコードを生成する。評価するのは本物の Cedar である。

名前は関所（せきしょ）から取った。

この文書は設計の段階（2026-10-06）に書き、言語の芯を作った段階（A）で、作ったものに合わせて直した。設計の段階では、ritsu 0.23.0 を読み、2026-10 の時点の Cedar とまわりの道具を調べて決めた。決めたことのうち、Cedar で本当にそうなるかが分かれ目になるものは、手で書いた例を公式の Cedar の CLI 4.13.0、cedar-wasm 4.13.0、cedarpy 4.12.1、cedar-go v1.8.0 にかけて確かめた（6 章）。例の規則・日付・ワークフローは、ritsu 0.23.0 の `rulec check`・`koyomi check`・`dandori check` と `ritsu check` を通してある。例の Cedar とリクエストを組み立てるコードは、sekisho が生成する形を手で書いた見本である。`.gate` の検査の結果として載せた数は、設計の段階には、検査が数える形を Python で書いた使い捨ての試作の出力（1,090 通りなど）だった。A で全部の組み合わせの検査を作り、4 章、7 章、10 章の数と文を実物の出力に差し替えた（6.2 の表は、試作を四つの実装に流した記録なので残した）。A では、字句と構文、名前と型、診断の台帳、`check` と `explain` のコマンド、`ritsu sekisho` と `ritsu check` の入口、口のまとまり `GatePorts` と口 `Gates` の型、rulec の `outputs_over`、koyomi の `calendar` と `doc`、OpenAPI と AsyncAPI の操作の読み手、全部の組み合わせの検査と参照の評価、行を合わせた表を作った。2.8、2.10、3 章、4 章、7〜10 章をそれに合わせて直し、16 章を決めたことの形に書き直した。段階 B では、Cedar のスキーマとポリシーの生成（`gen --target cedar`）、全部の組み合わせを `cedar run-tests` のテストにする `vectors`、外のツールのための JSON の `api` を作り、5 章、6.1、11 章を作ったものに合わせて直し、16.1 に段階 B で決めたことを足した。

## 0. 全体像

sekisho（関所）は、認可を書く小さな言語である。だれ（`principal`）が、何（`resource`）に、何をして（`action`）よいかを `.gate` のファイルに書く。sekisho はそれを、宣言した範囲のすべての組み合わせで確かめ、Cedar のスキーマとポリシー、Cedar に渡すリクエストを組み立てるコードを生成する。評価するのは本物の Cedar である（Amazon Verified Permissions、Rust の `cedar-policy`、`@cedar-policy/cedar-wasm`、`cedar-go`、`cedarpy`）。dandori が `.flow` から Temporal のコードを生成し、走らせるのは Temporal であるのと同じ形である。

```gate
permit clerks_refund_within_their_limit
  description "A clerk refunds up to the clerk's own limit, while the refund period lasts"
  principal in clerk
  action refund_order
  when refund_band is within_limit
  when in_period
```

`refund_band` は rulec の規則が、`in_period` は koyomi の日付が決める値である。Cedar の条件は、リクエストとエンティティの属性しか見られない。「金額が上限を超えたら」「返金の期間のうち」「営業日だけ」を許可の条件にするには、その答えを計算してリクエストに入れる者が要る。sekisho は、それを ritsu の言語に計算させ、計算するコードを生成し、計算した答えがとりうる値の全部の上で、ポリシーを確かめる。

### 0.1 芯にする主張

sekisho は、生成する前に次を確かめる。数は、どれも宣言した範囲の上で、全部の組み合わせを数えて言う。

1. **action ごとの表が決まっている。** 役割、属性、操作の引数、規則と日付の答え、の組み合わせごとに、許すか拒むかと、決めたポリシー（Cedar の determining policies）が一つに決まる。これが人の読むページの表になる（7 章）。
2. **期待（`expect`）が成り立つ。** 「上限を超える返金を、責任者でない係はできない」と書けば、それが当てはまるすべての組み合わせで拒まれることを確かめる。成り立たなければ、その例を示す。
3. **効かない permit、当たらない条件、だれもできない action が無い。** forbid に全部覆われて効かない permit、どの組み合わせでも条件が成り立たないポリシー、どの permit も許さない action を、それぞれエラーにする（だれもできないことを意図するなら、理由を書かせる）。
4. **職務の分離と、役割の届く範囲。** 同じ人が二つの action の両方を許されることがないこと（`separate`）と、ある役割だけを持つ人が許される action が、その役割に書いた `can` と一致すること。
5. **生成した Cedar が、本物の Cedar で同じに評価される。** これは証明ではなくテストである（ritsu の DESIGN 0.4）。全部の組み合わせのリクエストを生成して公式の CLI にかけ、許すか拒むかと決めたポリシーが、sekisho の参照の評価と一致することを確かめる（6 章）。

ritsu の一式の中では、さらに言語をまたいで二つを確かめる（4.6）。sakai の地図で、コンテキストが公開する操作のどれにも、それを守る action があること（X15）。dandori のワークフローが呼ぶ操作を、そのワークフローが許されていて、許される操作をワークフローが全部使っていること（X16）。

### 0.2 なぜ別の言語にするか

Cedar は小さく、解析しやすく、意味は Lean で形式化されている（13 章）。Cedar をそのまま書けば足りる場合も多い。それでも sekisho を作るのは、次の四つが Cedar だけでは書けないか、確かめられないからである。

- **業務の規則と日付を条件にすること。** Cedar の式には、表を引くことも、営業日を数えることも、月を足すことも無い（`datetime` の拡張は時刻の比較と経過時間で、祝日を知らない）。規則と日付は、それを確かめる言語（rulec と koyomi）に置き、その答えを使う。
- **全部の組み合わせの表。** Cedar の解析（SymCC と Cedar Analysis）は、二つのポリシーの集合が同じか、どちらが広いかを SMT で答える（13.1）。人が読んで確かめたいのは、それとは別に、業務の語で書いた「この役割が、この条件で、何をできるか」の表である。sekisho の条件は有限の値だけを見るので（P3）、表を全部数えて出せる。
- **ほかの成果物とのつながり。** action を契約の操作（OpenAPI の操作、proto のメソッド）に結び付ければ、公開する操作に認可の決まりがあるか、ワークフローが呼ぶ操作を許されているかを、言語をまたいで確かめられる。要件（yuen）から、それを満たすポリシーと、確かめる期待を指せる。
- **リクエストの組み立て。** Cedar はリクエストに入ってきた値を信じる。規則の答えのような値を、利用者の送ってきたものから作れば、利用者が自分で許可を書けることになる。どこから値を取るかを言語の側で決め、そのとおりのコードを生成する（3.6）。

### 0.3 前提

- **P1：評価するのは Cedar。** sekisho は実行時の評価器を持たない。sekisho の参照の評価は、検査とテストのためのものである。生成するポリシーは Cedar の意味（既定で拒む、`forbid` が勝つ）をそのまま使い、sekisho が意味を足したり変えたりしない。
- **P2：生成する Cedar は小さな部分だけを使う。** スコープの `==`・`in`・`is`、`==`・`!=`、整数の比較、`&&`・`||`・`!`、`has`、エンティティの `in` だけを使う。拡張の型（`decimal`、`ip`、`datetime`、`duration`）、算術、`like`、タグ、テンプレートは使わない。四つの実装と Verified Permissions のどれでも同じに評価され、Cedar の検証（strict）を通ったポリシーが実行時にエラーを出さないためである（6.2 で四つの実装と突き合わせた）。
- **P3：条件は有限の値だけを見る。** 列挙、真偽、役割、関係（持ち主か、メンバーか）、定数で区間に分けた数、規則の有限の出力、日付の述語の真偽。だから全部の組み合わせを数えられる（4 章）。
- **P4：規則と日付は、その言語が計算する。** sekisho は規則を Cedar の式に訳さず（3.5）、rulec と koyomi が生成したコードを呼んで答えを context に入れる。答えがとりうる値は、それぞれの言語に尋ねる（ritsu の P4）。
- **P5：context に入れる値は、信頼できるデータから取る。** 計算に使う値は、サービスが自分のデータから読んだエンティティの属性と、操作がこれから行うことの引数と、サーバーの時計だけである。利用者の送ってきた context をそのまま Cedar に渡す口は、生成するコードに作らない（3.6）。
- **P6：Cedar の側だけを見ても読める。** 生成するスキーマとポリシーには、元の業務の言葉と、計算した値の出どころを注釈で書く（3.7）。
- **P7：決められないことを黙って通さない。** どの検査も、成り立つ、成り立たない例がある、決められない（理由つき）の三つのどれかを言う（ritsu の P5）。
- **P8：計算できなければ拒む。** 規則の入力が範囲の外、カレンダーが知らない日、エンティティが見つからないときは、生成したコードは Cedar に尋ねずに拒む（3.8）。

### 0.4 ほかの言語にそろえるもの

- キーワードは英語の一種類で、名前（役割、型、属性、action、ポリシー、期待）は日本語で書ける。Cedar に出る名前には、rulec と koyomi と同じく ASCII の別名を丸括弧で付ける（5.7）。
- 見出しは `gate <名前> v1`、次に `description "…"`。`v1` は人のための目印で、生成物の頭と `api` に書き出すだけである。
- ほかのファイルは `use rule … from "…"`、`use dates … from "…"`、`use calendar … from "…"`、`use openapi|proto … from "…"` で読む（dandori と同じ）。
- 数は rulec の単位の型で書く（`money[GBP, incl_tax]`、`range >=1GBP <=10_000GBP`）。日付は `date`。
- コマンドは `check`・`gen`・`doc`・`api`・`vectors`・`explain`。終了コードは 0、1、2（ritsu の DESIGN 4.4）。診断の形と台帳は ritsu の土台のもの。
- 例とテストは英語が先で、日本語の版を `<名前>.ja.gate` として横に置く（ritsu の DESIGN 10.10）。

### 0.5 語

- **許す・拒む**：Cedar の allow と deny。「拒否」は ritsu では chobo が操作を受け付けないことの語なので、認可の答えには使わない。
- **決めたポリシー**：Cedar の determining policies。許したときは、当てはまった permit の全部。拒んだときは、当てはまった forbid の全部（無ければ空で、どの permit も当てはまらなかったことになる）。
- **組み合わせ**：検査が数える、有限の値の一つの組。数えるときは「通り」と言う。
- **計算した値**：context のうち、ritsu の言語が計算して入れる値（規則の出力、日付の述語の真偽）。操作の引数（`input`）と区別する。

## 1. 読む人と、Cedar との関係

### 1.1 読む人と書く人

`.gate` を読むのは、次の三者である。

- **セキュリティを確かめる人**：だれが何をできるかの全体を見る。ページの表（7 章）と、期待と職務の分離の結果を読む。
- **業務の持ち主**：返金の上限、期限、承認の決まりを決めた人。ポリシーの `description` と、規則と日付のページ（rulec と koyomi の `doc`）を読む。
- **監査の人**：決まりがいつ、どの要件から来たか（yuen）と、生成した Cedar がそれと同じか（ハッシュ、テスト）を見る。

書くのは開発者かエージェントである。rulec と同じく、エージェントが書いて `sekisho check` で直し、人が読んで決める、という使い方を考える。ページの読み手の呼び方は、ritsu の DESIGN 4.8 のとおり「コードが実現すべきものを理解し、確かめる人」である。

### 1.2 Cedar が評価し、sekisho は書いて確かめて生成する

| 受け持つこと | Cedar | sekisho |
|---|---|---|
| リクエストごとの評価 | する（P1） | しない。参照の評価は検査とテストのため |
| ポリシーとスキーマの形式 | Cedar の形式そのもの | 生成する（5.1、5.2） |
| 型の検証 | `cedar validate`（strict） | 生成したものが通ることをテストで確かめる |
| 解析 | SymCC、Cedar Analysis（同値、より広い、効かないポリシー） | 全部の組み合わせを数える（4 章）。同じ問いのいくつかは重なる（4.7） |
| 規則と日付 | 見られない | rulec と koyomi に計算させ、context に入れる |
| リクエストの組み立て | 受け取るだけ | 生成する（5.3） |
| 契約、地図、ワークフロー、要件とのつながり | 無い | ritsu の口でつなぐ（8 章） |

### 1.3 Cedar をそのまま読む確かめとの分担

ritsu は、Cedar のポリシーとスキーマを、proto や OpenAPI と同じく標準の形式として読む（`ritsu_base::cedar`）。その上に、言語をまたぐ確かめ（公開する操作と action の突き合わせ、ワークフローの最小権限、要件とポリシーの結び付け）を載せる。sekisho の `.gate` から生成した Cedar も、人が手で書いた Cedar も、同じ確かめに届くようにする。

**決定**：言語をまたぐ確かめは、Cedar の形でなく、口 `Gates`（8.3）で読む。口に答えるのは sekisho のクレートで、`.gate` のファイルと、`.cedar` と `.cedarschema` の組の両方に答える。

- action と契約の操作の結び付けは、Cedar のスキーマの action に付けた注釈 `@guards("<参照の書き方>")` で表す。sekisho は生成するスキーマにこの注釈を書き（5.1）、Cedar を読むときもこの注釈を読む。手で書いた Cedar も、この注釈を書けば同じ確かめ（X15）に届く。
- 許すか拒むかを数える確かめ（X16 の「呼ぶ操作を許されているか」、ページの表）は、`.gate` なら全部の組み合わせで答える。手で書いた Cedar は、sekisho の条件と同じ有限の部分（スコープ、役割の `in`、文字列と真偽と整数を定数と比べる条件、`has`、principal との一致）だけでできたポリシーなら同じに数え、そうでない式（算術、`like`、拡張の関数、集合の演算）があれば、そのポリシーが関わる問いを「決められない」と答える（P7）。
- 要件（yuen）は、`.gate` の中のもの（`sekisho "refunds.gate" policy …`）を指す。手で書いた Cedar は、ツール名 `cedar` で `@id` のポリシーと action を指す（8.4）。生成した Cedar は指さない（生成物で、`.gate` の中のものと一対一だから）。

**理由**：Cedar の意味（既定で拒む、forbid が勝つ、`in` の推移、`has`）を知る場所を一つにするためである。ritsu-cross が Cedar を自分で評価すると、sekisho の参照の評価と二つの実装になる（ritsu の P4）。sekisho は認可の言語で、Cedar はその標準の交換の形式である。`.proto` が契約の標準の形式で、ritsu-proto が読み、意味は読む言語が決めるのと同じ形である。

**捨てた案**：

- Cedar をそのまま読む確かめを ritsu-cross に置き、sekisho は別に持つ。同じ問いに二つの答え方ができ、食い違ったときにどちらが正しいかを決める者がいない。
- 手で書いた Cedar を sekisho のモデルに読み直して、`.gate` に書き戻す（逆の生成）。Cedar のほうが表せるものが多く（算術、`like`、拡張の関数）、書き戻せないものが残る。読む確かめに要るのは操作との結び付けと有限の部分の評価で、`.gate` への変換ではない。

## 2. モデル

### 2.1 ファイルの形

`.gate` は一つの種類だけで、節の順は決まっている。見出し、`description`、`namespace`、`use`（いくつでも）、`today`、`enum`、`role`、`principal`、`workflow`、`resource`、`action`、`permit` と `forbid`（混ぜてよい）、`expect`、`separate` の順である（違えば E003）。`description`・`namespace`・`today` は一度まで。字下げは空白で、そろっていなければ E004（koyomi と同じ）。`#` から行末まではコメント。

一つのファイルが、一つの Cedar の名前空間の、一つのまとまり（action とそのポリシー）を書く。ほかの `.gate` が宣言した役割や型を使うときは `use gate "people.gate"` で読む（2.10）。

### 2.2 principal：利用者、サービス、ワークフロー

```gate
principal User
  description "A member of the shop's staff, signed in"
  roles clerk, manager, auditor
  attributes
    refund_limit : money[GBP, incl_tax]  range >=0GBP <=10_000GBP
    suspended    : bool

principal Customer
  description "A customer of the shop, signed in"

workflow returns from "flows/returns.flow"
  description "Refunds a returned order once the item is back at the warehouse"
```

- `principal <型>` は Cedar のエンティティの型になる。`roles` は、その型の principal が持てる役割（Cedar では役割のエンティティの子になる）。`attributes` は、ポリシーか計算した値が読む属性である。
- サービス（ほかのシステムが自分の資格で呼ぶもの）も `principal Service` のように型として書く。sekisho は人とサービスを分けない。型で分ける。
- **ワークフロー**は `workflow <名前> from "<.flow>"` で書く。Cedar では型 `Workflow` のエンティティ（ID は名前）になる。`Workflow` は sekisho が知っている型で、`workflow` の行が一つでもあれば生成する。ワークフローは、自分の資格（Temporal のワーカーの資格、mTLS の証明書など）で API を呼ぶ。資格から principal を作るのは API の側の認証で、sekisho はその後ろを受け持つ。`from` の `.flow` は dandori の検査を通ること（E208）。X16 が、その `.flow` が呼ぶ操作と、このワークフローが許されるものを突き合わせる（4.6）。

**捨てた案**：principal の種類（人、サービス、ワークフロー）をキーワードで分ける。Cedar では、どれもエンティティの型で、種類による意味の違いが無い。ワークフローだけを `workflow` にしたのは、dandori の `.flow` と一対一に結び付き、言語をまたぐ確かめの相手になるからである。

### 2.3 役割とグループ（RBAC）

```gate
role manager
  description "Refunds what a clerk cannot, and after the refund period on a business day"
  includes clerk
  can view_order, refund_order
```

- 役割は Cedar の型 `Role` のエンティティになる（ID は別名）。スキーマは `entity Role in [Role]` と書く。`includes clerk` は「manager を持つ人は clerk でもある」で、エンティティでは `Role::"manager"` の親を `Role::"clerk"` にする。Cedar の `in` は親を推移的にたどるので、`principal in Role::"clerk"` が manager にも当たる。役割の親子は `.gate` に書いたもので、生成したコードが定数としてエンティティに入れる。サービスのデータからは読まない（データで役割の親子を変えられないようにする）。
- 人が持つ役割（`User` の親）は、サービスのデータから読む。検査は、`roles` に書いた役割のどの組（持たない、一つ、二つ以上）も起こりうるものとして数える。職務の分離で確かめたいのは、まさに二つの役割を同時に持つ人だからである。
- グループ（チーム、部署）は、役割と同じ形で書ける（`role` で宣言し、`includes` で入れ子にする）。チームのように、エンティティごとに増えるものは関係（2.5）で書く。
- `can` は、その役割の届く範囲として、その役割（と `includes` した役割）だけを持つ人が許されうる action を並べる。書けば検査が両方向に確かめる（4.4 の E306、W302）。

**捨てた案**：役割を属性（`principal.role is clerk`）で書く。一人が複数の役割を持つ形と、役割の入れ子が書けず、Cedar の慣わし（役割をエンティティにして `in` で調べる）からも外れる。

### 2.4 属性（ABAC）と型

属性の型は、有限の値を持つものに限る（P3）。

| `.gate` の型 | Cedar の型 | 検査が数える値 |
|---|---|---|
| `bool` | `Bool` | 真と偽 |
| 列挙（`enum order_status = paid \| shipped \| …`） | `String`（値の別名） | 列挙の値。注釈 `@doc` に値の並びを書く |
| 数（rulec の単位の型と `range`） | `Long`（宣言した単位で数えた整数） | ポリシーが比べる定数で、範囲を区間に分けたもの（4.1） |
| `date`（`range` つき） | Cedar に出さない（計算した値の入力にだけ使う） | 日付の述語を通して数える |
| エンティティの型（`customer : Customer`） | その型 | 関係として数える（2.5） |
| `T?` | 省ける属性（`?`） | 値が無いことを一つの値として数える |

文字列（自由な文字列）は属性に持てない。比べる相手が無限にあるからである。

- **列挙を `String` にした理由**：Cedar 4 には列挙のエンティティ型（`entity Status enum ["paid", …]`）があるが、値をリクエストに入れるときにエンティティの参照（`{"__entity": …}`）にする要があり、Verified Permissions が受け付けるかを確かめていない。ポリシーの文字列は sekisho が生成するので、綴りの誤りは起きない。値の並びは注釈に書く。
- **数**：rulec と同じく、宣言した単位で数えた整数として運ぶ（`money[GBP, incl_tax]` なら 1 ポンドが 1）。定数との比較は ritsu-units で単位をそろえてから整数にする（dandori の範囲の端と同じ。ritsu の DESIGN 5.3）。
- **Cedar に出す属性は、生成したポリシーが読むものだけにする。** `refund_limit` や `paid_on` のように、計算した値の入力にだけ使う属性は、Cedar のスキーマにもエンティティにも出さない。Cedar に渡すデータを小さくし、sekisho が確かめていない生の値の上に、Cedar だけで新しいポリシーを書かせないためである（ページと注釈には、計算に使ったことを書く）。

### 2.5 関係（ReBAC）：どこまで入れるか

**決定**：v1 は、principal と resource の属性を一段だけたどる関係を入れる。

- `resource.customer is principal`：属性が指すエンティティが、principal そのもの（持ち主、担当）。
- `resource.tenant is principal.tenant`：二つの属性が、同じエンティティを指す（テナントや店舗が同じ。マルチテナントの分離に要る）。二つの属性は同じ型であること。
- `principal in resource.team`：principal が、属性が指すグループのメンバー（Cedar の `in`。メンバーであることは、サービスのデータが principal の親として渡す）。
- `principal.department is resource.department`：二つの属性がどちらも列挙のとき（これは列挙どうしの比較で、有限）。

**数え方**：エンティティを指す項（`principal` と、エンティティの型の属性）を、型ごとに、どれとどれが同じエンティティかで組に分ける。分け方の全部（項が四つなら 15 通り）を数える。メンバーであること（`principal in resource.team`）は、同じと決めた組ごとに一つの真偽として数える。こうすると、「A と B が同じで、B と C が同じなのに、A と C が違う」のような起こりえない組み合わせを数えず、起こりうるものは全部数える（どの分け方も、エンティティの ID を選べば作れる）。

**入れないもの**：二段以上の関係（`resource.project.owner`、「その注文の店の、その店の担当」）と、関係の書き換えのある Zanzibar の形（OpenFGA や SpiceDB の、関係から関係を作る定義）。二段以上の関係は、Cedar では属性をたどる形や `in` の親をたどる形で書けるが、たどった先のエンティティの属性もデータに要り、何を数えればよいかが関係のグラフ次第になる。Zanzibar の形は、関係のデータを持つストアと一緒に使うもので、sekisho の生成物（Cedar とリクエストを組み立てるコード）の外にある。要るなら、その答え（「この人はこの文書の編集者か」）を真偽の計算した値として渡す形で足せる（15 章）。

### 2.6 resource と action、操作との結び付け

```gate
resource Order
  description "An order of the shop"
  attributes
    status   : order_status
    paid_on  : date  range >=2026-01-01 <=2028-09-30
    customer : Customer

action refund_order
  description "Refund an order, in part or in whole"
  guards orders refundOrder
  principal User, Workflow
  resource Order from orderId
  input
    amount : money[GBP, incl_tax]  range >=1GBP <=10_000GBP
  context
    refund_band  = refund_limit(amount: amount, limit: principal.refund_limit).band
    in_period    = today <= refund_terms.last_day(paid_on: resource.paid_on)
    business_day = today is open in uk
```

- `guards <API> <操作>`：action が守る契約の操作。`<API>` は `use openapi|proto|asyncapi <名前> from "…"` の名前、`<操作>` は OpenAPI の `operationId`（無ければ `"POST /orders/{orderId}/refunds"`）、proto の `"Service/Method"`（dandori の `connect` と同じ書き方）、AsyncAPI の操作のキー。chobo の振替の操作は `guards <帳簿> <振替>.<操作>` で書ける（3.4）。一つの action が二つ以上の操作を守ってよい（同じ操作を REST と gRPC で出すとき）。一つの操作を二つの action が守ればエラーにする（E205。どちらの判断で守るかが決まらない）。文の中と JSON では、参照の書き方（`openapi "api/orders.json" operation refundOrder`。ritsu の DESIGN 6.2 に足すと 2026-10-06 に決まった `openapi` のツール名）で書く。
- `principal` と `resource` の行は、その action のリクエストに来うる型を並べる。Cedar のスキーマの `appliesTo` になる。
- `resource Order from orderId`：resource の ID を、操作のどの引数（パスの引数かクエリ）から取るか。生成するコードは、その引数で resource をサービスのデータから読む。書かなければ、生成するコードは resource の ID を引数に取る。
- `input`：操作の引数のうち、ポリシーか計算した値が読むもの。名前は操作の引数（OpenAPI ならパスとクエリの引数か本文のフィールド、proto ならリクエストのメッセージのフィールド）と同じで、型と範囲は操作が受け取るものと比べる（E203。dandori の E016 と同じ読み方。例のワークフローも同じ範囲を書いている）。
- `context`：計算した値（3 章）。名前は Cedar の context の属性の名前になる。

**操作と結び付けない action**も書ける（`guards` の無い action）。そのときは、ページと `api` に「どの操作も守っていない」と出る。バッチの処理のように、契約の無い入口を守るためである。

**捨てた案**：`guards` に参照の書き方をそのまま書く（`guards openapi "api/orders.json" operation refundOrder`）。action ごとにファイルのパスを繰り返すことになり、文書を動かしたときに全部の行を直すことになる。`use openapi orders from "…"` で一度だけパスを書き、`guards orders refundOrder` と書く形は、dandori の `use openapi` と `http POST stripe "/v1/…"` と同じである。参照の書き方は、yuen と sakai のように、ものを指すこと自体が仕事の言語の書き方として残し、sekisho では診断の文と JSON に使う。

### 2.7 permit と forbid

```gate
forbid suspended_staff_do_nothing
  description "A suspended member of the staff does nothing"
  principal is User
  action any
  when principal.suspended
```

- どのポリシーも当てはまらなければ拒み、`forbid` が一つでも当てはまれば、`permit` があっても拒む。Cedar の意味そのものである（P1）。
- `principal` の行は、`in <役割>, <役割>…`（どれかの役割）、`is <型>`、`is workflow <名前>`、のどれか一つ。無ければ、その action のどの principal にも当たる。
- `action` の行は、このファイルの action を一つ以上か、`any`（このファイルの action の全部）。ほかのファイルの action は書けない。
- `when` と `unless` の行は、いくつでも書ける。ポリシーは、書いた行が全部成り立つときに当てはまる（Cedar も同じ）。一つの行の中では `and`・`or`・`not` と括弧を使える。
- ポリシーには名前を付け、その別名が `@id`（`<ファイルの別名>/<ポリシーの別名>`）になる（5.2）。名前の無いポリシーは書けない。並びの順で ID を振ると、ポリシーを一つ足しただけで後ろの全部の ID が変わり、yuen の記録と Verified Permissions のポリシーが全部変わったように見えるからである。

**捨てた案**：表（rulec の決定表）で書き、行を Cedar のポリシーに訳す。表は「最初に当たった行」か「重ならない行」で読むが、Cedar は「どれかの forbid が当たれば拒む、どれかの permit が当たれば許す」で読む。書いた形と評価の形が違うと、Cedar の側だけを見た人が読めない（P6）。表は、出力として出す（7 章）。

### 2.8 期待、職務の分離、役割の届く範囲

```gate
expect deny clerks_never_refund_over_their_limit
  description "A clerk who is not a manager never refunds more than the clerk's own limit"
  principal in clerk
  action refund_order
  unless principal in manager
  when refund_band is over_limit

separate refunding_and_auditing
  description "The people who refund are not the people who audit the refunds"
  actions refund_order, export_refunds
```

- `expect allow|deny <名前>`：ポリシーと同じ形で選んだ組み合わせの全部で、答えがそうなること。成り立たなければ、成り立たない組み合わせの数と、一つの例（役割、属性、引数、計算した値）を示す（E304）。koyomi の `claims` と rulec の `examples` にあたる、人が書いた期待である。どの組み合わせも選ばない期待は、成り立つが何も確かめていないので、W304 で言う（たいていは行の書き違いである）。
- `separate <名前>` と `actions A, B…`：同じ principal（同じ型、同じ役割の組、同じ属性）が、並べた action の二つ以上を許されることが無い（E305）。resource と context は action ごとに自由に選んでよい（二つのリクエストは別のものだから）。
- `can`（2.3）：役割ごとの届く範囲。

### 2.9 計算した値と `today`

```gate
today range >=2026-10-01 <=2028-10-31 offset +00:00
```

`today` はリクエストの日で、生成したコードがサーバーの時計から取る。範囲（検査はこの範囲のすべての日を数え、生成したコードはほかの日を受け付けない）と、日を変えるオフセットを書く。koyomi と同じく、夏時間のあるタイムゾーンの名前は書けない（koyomi の DESIGN 1.9）。イングランドとウェールズのように夏時間のある地域では、日を UTC の 0 時で変えるなどと決めて `.gate` に書き、読む人に見せる（例では、夏のあいだ、現地の 0 時から 1 時までは前の日として扱われる）。`today` を使う計算した値があれば、`today` の行は要る（E107）。

### 2.10 ほかの `.gate` を読む

`use gate "people.gate"` は、そのファイルの `enum`・`role`・`principal`・`workflow`・`resource` を使えるようにする。読んだファイルの `forbid` のうち `action any` のもの（全部の action に効く forbid。停止中の人は何もできない、など）は、読んだ側の action にも効く。読んだファイルの `permit` は効かない（だれかに全部を許す permit がほかのファイルから来ると、そのファイルの検査が知らないところで許すことが増えるからである）。読んだファイルの forbid は、読んだ側の action のうち、その forbid の `principal` の行に当たる principal が来るものに効き、当たらない action には何も言わない（読んだ側の action が、その型を一つも取らないとき、E106 は出さない）。条件が読む属性と値は、効く action の上で確かめる。読んだファイルがさらに `use gate` で読むファイルの宣言も使える。読む向きが輪になれば E201 である。

`use gate` で読み合うファイルは、Cedar の名前空間をそろえる（違えば E210）。読んだ型と役割は、読む側の名前空間で使うからである。`namespace` を書かないファイルの名前空間はファイルの別名の Pascal case なので、読み合うファイルには同じ `namespace` の行を書く。また、読んだ二つのファイルが、同じ名前か別名の型・列挙・役割・ワークフローを宣言してはならない（E211）。一つの名前空間に、同じ名前のものが二つになるからである。読んだ二つのファイルが同じファイルを読むときは、そのファイルを一つと数える。

生成するとき、読んだファイルの `action any` の forbid は、読んだ側の action だけを並べたスコープで、読んだ側のポリシーとして書き出す（ID は `<読んだ側の別名>/<読んだファイルの別名>/<ポリシーの別名>`。Verified Permissions の名前に `.` は使えない）。こうすると、一つの `.gate` から生成した Cedar は、そのファイルの検査が見たポリシーとちょうど同じになる。同じ名前空間の複数のファイルを一つのポリシーストアに置いても、あるファイルの forbid がほかのファイルの action に、検査の知らないところで効くことが無い。

**捨てた案**：名前空間ごとに全部のファイルを一つにまとめて検査する。ファイルを足すたびに、ほかのファイルの表が変わり、ファイルごとの `sekisho check` が成り立たなくなる。

## 3. 条件と、Cedar に届く形

### 3.1 条件の形

ポリシーと期待の `when`・`unless` の行に書けるものは、次の形だけである。どれも有限の値を見る（P3）。

| `.gate` | 意味 | 生成する Cedar |
|---|---|---|
| `principal in clerk` | 役割を持つ（`includes` をたどる） | `principal in Shop::Role::"clerk"` |
| `principal is Customer` | principal の型 | `principal is Shop::Customer` |
| `principal is workflow returns` | そのワークフロー | `principal == Shop::Workflow::"returns"` |
| `resource.status is refunded` | 列挙の値 | `resource.status == "refunded"` |
| `principal.suspended` | 真偽の属性 | `principal.suspended` |
| `amount <= 50GBP` | 数と定数（`<`・`<=`・`>`・`>=`・`is`） | `context.amount <= 50` |
| `resource.customer is principal` | 関係（2.5） | `resource.customer == principal` |
| `resource.tenant is principal.tenant` | 関係（2.5） | `resource.tenant == principal.tenant` |
| `principal in resource.team` | 関係（2.5） | `principal in resource.team` |
| `refund_band is within_limit` | 計算した値（列挙） | `context has refund_band && context.refund_band == "within_limit"`（省ける値のとき） |
| `in_period` | 計算した値（真偽） | `context.in_period` |

`and`・`or`・`not` と括弧で組み合わせられる。`is not` も書ける。値が無いとき（省ける属性が無い、principal の型が計算した値の入力の属性を持たない）は、条件は当てはまらない。`x is not v` は `not (x is v)` と読むので、値が無いときは成り立つ。生成する Cedar は、これに合わせて `has` で守る（`!(context has x && context.x == "v")`）。計算した値は、action の `context` の節でだけ定義し、ポリシーは名前で使う。計算した値が Cedar の context の属性と一対一になり、Cedar を読む人が、何が計算されたものかを一か所で見られるようにするためである。

### 3.2 rulec の規則

```gate
use rule refund_limit from "rules/refund_limit.rule"
…
  context
    refund_band = refund_limit(amount: amount, limit: principal.refund_limit).band
```

- 呼び方は dandori と同じ（`<規則>(<入力>: <値>, …)` のあとに `.<出力>`）。入力に渡せるのは、principal と resource の属性、`input`、定数だけである（3.6）。ほかの計算した値は渡せない（v1。計算した値の順を考えずに済み、一つの計算した値が一つの規則の呼び出しになる）。
- 出力は列挙か真偽に限る（E105）。数の出力を定数と比べる形は、出力の区間を rulec に尋ねれば足せるが、v1 では入れない（15 章）。
- 型は単位の型で比べる（`Unit::same`。`JPY` と `円` は同じ、税込と税抜は違う）。渡す値の範囲は、規則の入力の範囲に収まっていなければならない。入力どうしの前提（rulec の `constraint`）は、渡す値の範囲の上で rulec に尋ねる（`Rules::preconditions_hold`。dandori の X2 と同じ問い）。破る例があれば E206、決められなければ W201（生成したコードは、走らせたときに前提を確かめ、破れば拒む。P8）。
- 規則に渡す値の範囲は、数も日付も rulec に渡す（`preconditions_hold` と `outputs_over`。日付は日の番号）。属性と `input` は宣言した範囲、定数はその一点、`today` は `today` の範囲である。渡さないと、rulec は規則が自分で宣言した範囲で答え、ゲートの範囲より広い範囲で前提を確かめることになる（起こらない値で E206 を言いうる）。段階 A では、範囲に端の無い値を規則に渡せない（E103）ので、W201 は出ない。安全網として残し、台帳の再現はまだ無い。
- **出力がとりうる値は rulec に尋ねる。** 規則の入力に渡す属性が、ほかの条件でも使われていなければ（`refund_limit` は規則にしか渡さない）、出力の値は、表の行が書く値の全部である。rulec はどの行にも当たる入力があること（E102 が無いこと）を確かめているので、行が書く値は全部起こりうる。入力に渡す値がほかの条件でも使われていれば（`amount` はポリシーが 50 ポンドと比べる）、その条件が切る区間ごとに、出力がとりうる値を rulec に尋ねる（口の問い `Rules::outputs_over`。8.3）。rulec は、入力を区間に限って規則を読み直し、出力を決める表の行のうち、ある入力が届きうる行を求める（導出と define の届く区間、`constraint`、上の表、一次のモデルのどれでも落ちない行）。その行が書く値の一つ一つに、区間の中の具体的な入力を規則のベクタから探して添え、二つがそろえば正確に答える。そろわないとき（同じ入力を読む二つの導出に、整数ではとれない組を求める行など）は、その値と行を言って決められないと答え、sekisho はその区間で出力の全部の値を数える（4.1）。rulec のコーパスとテストの規則では、数と日付の入力を小さな区間に限った 323 の問いのうち 322 が、区間の中の全部の入力を流した答えと同じに正確に答えられ、残る一つ（わざとそう作った規則）は決められないと答えた（rulec の DESIGN 15.188）。
- 例の `refund_limit` は、`derive excess = amount - limit` を表で 0 と比べる（`rules/refund_limit.rule`。ritsu 0.23.0 の `rulec check` を通る）。`amount` の区間 1〜50 と 51〜10000 のどちらでも、`within_limit` と `over_limit` の両方が起こりうる（rulec の `outputs_over` の答え。どちらも正確な答えで、検査は多めに数えない）。上限を 100 ポンド以上に限ると、1〜50 は `within_limit` だけになる。
- 規則の出力の値は、`.rule` に書いた名前（`上限まで`）、生成したコードの名前（`WithinLimit`）、`.rule` の別名（`within_limit`）のどれで書いてもよい。名前の検査は、三つのどれでも同じ値を引く。Cedar に渡すのは別名で（16.1 の 14）、ページの表と診断の例の文は、`.gate` と並べて読めるよう `.rule` に書いた名前で見せる（7 章）。

### 3.3 koyomi の日付

```gate
use dates refund_terms from "dates/refund_terms.cal"
use calendar uk from "calendars/england_and_wales.cal"
today range >=2026-10-01 <=2028-10-31 offset +00:00
…
  context
    in_period    = today <= refund_terms.last_day(paid_on: resource.paid_on)
    business_day = today is open in uk
```

- 書けるのは二つの形である。`today <比較> <日付>`（日付は koyomi の日付の関数の呼び出しか、日付の属性。比較は `<`・`<=`・`>`・`>=`・`is`）と、`today is open in <カレンダー>`（koyomi のカレンダーで営業日）。どちらも真偽の計算した値になる。
- 日付の関数の入力に渡す値は、`date` の属性か `input` で、範囲が関数の入力の範囲に収まっていること（E206）。
- カレンダーのデータの範囲が、`today` の範囲と、関数が返しうる日の全部を覆っていること（E207。koyomi の E203 と同じ考え）。
- **起こりうる真偽の組は koyomi に尋ねる。** `in_period` と `business_day` はどちらも `today` を読むので、別々に真偽を選ぶと起こりえない組を数えるかもしれない。koyomi は範囲のすべての日を計算できるので、`today` の日と `paid_on` の日のすべての組で、二つの値の組を数える。例では四つの組（期間内か、営業日か）がどれも起こる。たとえば (いいえ, いいえ) は、今日が 2026-10-03（土曜）で最終日が 2026-02-02 のとき（試作の答え）。koyomi の問いは、口 `Dates` の `eval` と `calendar`（8.3）で足りる。日の組の数も、組み合わせの予算（4.1）に入れて数え、超えれば E307 で止まる。koyomi の `calendar` は、カレンダーのデータの範囲と、その中で休みの日の全部を返す（例のイングランドとウェールズのカレンダーでは、2019-01-01〜2028-12-31 のうち 1,127 日）。日付の関数は、渡す値の組ごとに `eval` で一度ずつ尋ね（例では支払日の 1,004 日）、同じ組を二度は尋ねない。koyomi の口は検査したファイルを覚えているので、尋ねるたびに検査し直すことは無い（koyomi の DESIGN 11 章）。

### 3.4 chobo をどうするか

**決定**：帳簿の残高は条件にしない。帳簿の振替の操作は、action が守るものにできる（`use book stock from "books/stock.book"` と `guards stock receive.do`）。

- 「在庫が足りるときだけ出荷してよい」は認可ではなく、帳簿の境界である。chobo は、どの振替でも、一度の書き込みの中で境界を守り、守れなければ理由の名前つきで拒否する（ritsu の DESIGN 0.1）。認可の判断の時点で残高を読んで許しても、振替を書くまでに別の振替が残高を変えうる。判断と書き込みが別の時点になるので、残高は認可の条件として信用できない。
- 残高は状態で、有限の値にならない（P3）。
- 認可が受け持つのは、だれがどの振替を動かしてよいか（倉庫の人だけが入荷を記帳できる）である。これは、操作を守る action として書ける。dandori のワークフローが振替を呼ぶときは、ワークフローを principal にして X16 で確かめられる。

### 3.5 A・B・C の比較と決定

規則と日付の答えを Cedar に届ける形を、三つ比べた。

| | (A) context に答えを入れる | (B) 規則を Cedar の式に訳す | (C) 訳せるものは訳し、ほかは context |
|---|---|---|---|
| 意味 | rulec と koyomi が生成したコードがそのまま答える。規則を確かめたことが、そのまま効く | 訳した式が規則と同じ答えを返すことを、規則ごとに別に確かめる要がある | 二つの届け方があり、どちらになるかが規則の中身で変わる |
| 書ける規則 | 全部（表、`derive`、`define`、丸め、単位、並び、ステートマシン）と、koyomi の全部 | Cedar に無いもの（割り算と丸め、並びを歩く表、祝日の表、月の足し算）は書けない | (A) と同じ |
| Cedar の側の読みやすさ | `context.refund_band == "within_limit"`。中身は注釈とページで読む（3.7） | 式は読めるが、大きな表は長い式になる（Verified Permissions は一つのポリシーを 10,000 バイトまでとする） | 二つの書き方が混ざる |
| Cedar の解析（SymCC） | 計算した値は自由な値として扱われる | 全部を見られる | 半分を見られる |
| 信頼 | context を作る者を信じる要がある（3.6） | 要らない | (A) の分は要る |
| 生成するコード | context を作るコードが要る（rulec と koyomi の生成物を呼ぶ） | Cedar だけ | 両方 |
| 規則を直したとき | Cedar は変わらない（context の値の型が変わるときだけ変わる） | Cedar が変わる | 届け方が入れ替わると、Cedar も context の形も変わる |

**決定**：(A)。規則と日付の答えは、いつも context の計算した値として渡す。sekisho 自身の条件（役割、属性と定数の比較、関係）は、もともと Cedar の式で書けるので、そのまま Cedar の式にする。これは訳しているのではなく、sekisho の条件が Cedar の部分そのものだからである。

**理由**：

1. 規則の意味を書くのは rulec だけにする（ritsu の P4）。(B) は rulec の評価をもう一つ Cedar で書くことになり、二つが食い違わないことを、規則ごとに全部の入力で確かめ続ける要がある。
2. 書ける規則が減らない。(B) では、koyomi の日付は書けず（Cedar の `datetime` は祝日を知らず、月を足せない）、rulec の丸め、率、並び、ステートマシンも書けない。
3. Cedar が変わる理由が、ポリシーを直したときだけになる。表の境目を一つ変えるたびに Verified Permissions のポリシーが変わる (B) では、ポリシーの変更の記録（yuen の記録、Verified Permissions の変更）が規則の変更で埋まる。
4. (C) は、規則に計算の行を一つ足しただけで届け方が入れ替わり、Cedar と context の形がまとめて変わる。二つの届け方のテストも二倍になる。

(A) で弱くなる二つ、context を作る者を信じることと、Cedar の側だけでは条件の中身が見えないことには、3.6 と 3.7 で手当てをする。SymCC が計算した値を自由な値として扱うことは、sekisho が全部の組み合わせを数えることで補う（4.7）。

同じ形の先例がある。Dogwood（AI のエージェントとツールの呼び出しのガバナンスのための言語。Cedar から派生し、2026-07 に Apache-2.0 で公開された参照のインタプリタがある）は、計算する値（information providers）と時間の条件を Cedar のポリシーに変換するとき、その値を `context.providers.<id>` に置き、評価のときに Dogwood 自身が埋める（13.4）。sekisho が違うのは、計算するのが ritsu の言語で、その答えがとりうる値を確かめた上で数えることと、埋めるのが評価の仕組みではなく生成したコードであることである。

### 3.6 context を利用者の側で作らせない

計算した値は、利用者が書けてはならない。規則の入力も、利用者が書けてはならない。利用者の送ってきた context をそのまま Cedar に渡せば、利用者が `refund_band = "within_limit"` と書いて、自分の返金を許せる。次の形で守る。

1. **値の出どころを言語で限る。** 計算した値の入力に書けるのは、`principal.<属性>`、`resource.<属性>`、`input` の名前、定数、`today` だけである（E105）。リクエストの本文の任意の場所を指す書き方は無い。
2. **principal と resource の属性は、サービスのデータから読む。** 生成するコードは、サービスが実装する `Store`（型ごとに ID で属性を返す関数）を受け取り、principal の ID（認証が決めたもの）と resource の ID（`from orderId` の引数）で読む。属性をリクエストから受け取る引数は無い。
3. **`input` は、操作がこれから行うことの引数である。** 返金の額は、利用者が頼むものだが、操作はその額で返金する。認可がその額を見るのは正しい（Cedar の context は、もともとリクエストごとの値を入れる場所である。AgentCore Policy も、ツールの呼び出しの引数を context に入れる）。危ないのは、利用者が事実（自分の役割、注文の合計）を送ることで、それは `input` に書けない（`input` は操作の引数と同じ名前と型でなければならない。E203）。生成する関数は、ハンドラーが解析した引数のオブジェクトを受け取り、文書は「操作は、認可に渡したのと同じ引数で行う」と書く。
4. **生成するコードに、計算した値や context を外から受け取る口を作らない。** TypeScript なら、`RefundOrderInput` は宣言した `input` だけを持つ型で、Cedar のリクエストは関数の中で組み立てる。Python の `TypedDict`、Go の構造体も同じ。
5. **データの範囲を確かめる。** `Store` が返した属性が、`.gate` の宣言した範囲や列挙の外なら、エラーにして拒む（P8）。検査はその範囲の上で確かめたからである。
6. **テストで確かめる。** 生成したコードのテスト（6.3）は、生の値（`Store` の中身、`input`、時刻）から、生成したコードでリクエストを組み立てて Cedar に尋ねた答えと、同じ生の値から sekisho の参照の評価（rulec と koyomi の参照の評価器を口で呼ぶ）で求めた答えが一致することを確かめる。

Cedar の側では、計算した値と偽の値を見分けられない。守りは、計算した値を作るのが生成したコードだけであるという、コードの形による。Verified Permissions を使うときも同じで、`IsAuthorized` を呼べる資格を持つ者は、どんな context でも送れる。Verified Permissions は判断を返す場所で、守る場所（API のハンドラー）ではない。これはページと README に書く。

### 3.7 Cedar の側だけを見た人が読める形

- **スキーマの注釈。** 名前空間、エンティティの型、属性、action、context の属性に `@doc` を付ける。計算した値の `@doc` には、どの言語のどのファイルの何から、どの値で計算したかと、「生成したコードが計算し、呼ぶ側からは受け取らない」ことを書く。action には `@guards`（守る操作の参照の書き方）を付ける。Cedar 4.13.0 で、名前空間、エンティティの型、属性（context のレコードの属性も）、action の注釈が読め、JSON の形にも残ることを確かめた（`cedar translate-schema`）。
- **ポリシーの注釈。** `@id`（5.2）と `@doc`（`description`）。日本語の名前のポリシーは、ID が別名になり、`@name` に日本語の名前、`@doc` に日本語の説明が入る（5.7）。スキーマの型、属性、action も同じく `@name` を持つ。
- **生成物の頭。** `// Code generated by sekisho <版>. DO NOT EDIT.` と、元の `.gate` とそのハッシュ（ritsu の DESIGN 9.2）。Cedar のコメントは `//` で書ける。`cedar format` は、頭のコメントを最初のポリシーのすぐ上に付ける（空行を消す）ので、生成するテキストもその形にする。
- **ページ。** `sekisho doc` は、ポリシーごとに `.gate` の行と生成した Cedar を並べ、計算した値ごとに rulec と koyomi のページを開ける（7 章）。

例のスキーマの一部（設計の段階に手で書き、`cedar validate` を通した見本）：

```cedarschema
  @doc("Refund an order, in part or in whole")
  @guards("openapi \"api/orders.json\" operation refundOrder")
  action refund_order appliesTo {
    principal: [User, Workflow],
    resource: [Order],
    context: {
      @doc("What the operation is asked to refund: money[GBP, incl_tax], in whole pounds, 1 to 10000")
      amount: Long,
      @doc("rulec \"rules/refund_limit.rule\" output band, from amount and principal.refund_limit: within_limit or over_limit. For a User only. Computed by the code ritsu gen writes, never taken from the caller")
      refund_band?: String,
      @doc("today <= koyomi \"dates/refund_terms.cal\" date last_day, from resource.paid_on. Computed by the code ritsu gen writes, never taken from the caller")
      in_period: Bool,
      …
```

sekisho が生成する形（実物。並びと書き方は `cedar translate-schema` の形になる）は 5.1 にある。

`refund_band` が省ける属性（`?`）なのは、`principal.refund_limit` を持たない principal（ワークフロー）では計算しないからである。sekisho は、計算した値が読む属性を持たない principal の型があれば、その値を省けるものにし、ポリシーの条件に `context has refund_band &&` を付ける。付けなければ Cedar の検証が通らない（試した。`unable to guarantee safety of access to optional attribute`）。

### 3.8 計算できなければ拒む

生成したコードは、次のときに Cedar に尋ねずに拒み、理由を返す。

- `Store` が principal か resource を返さない。属性が宣言した範囲や列挙の外。
- `input` が宣言した範囲の外。
- `today` が宣言した範囲の外（例の試しでは 2029-01-02 で `today 2029-01-02 is outside 2026-10-01..2028-10-31` を返して拒んだ）。
- 規則や日付の生成物がエラーを返した（範囲を確かめていれば起きないはずのことへの備え）。

Cedar に尋ねる前に拒むのは、Cedar が、評価の途中でエラーになったポリシーを当てはまらなかったものとして扱うからである。forbid の条件がエラーになれば、その forbid は効かず、ほかの permit が許すことがある。生成するポリシーは Cedar の検証（strict）を通り、検証は、スキーマに合うリクエストではエラーが起きないことを保証する（13.1。整数のオーバーフローは別で、sekisho は算術を生成しない）。だから、エラーの元は、スキーマに合わない値を入れることだけで、それを Cedar の前で止める。突き合わせのテストは、どのリクエストでもエラーが 0 であることも確かめる（`num_errors: 0`）。

## 4. 確かめること

### 4.1 有限の組み合わせ

検査は action ごとに、その action に当たるポリシー（読んだファイルの `action any` の forbid を含む）と、その action についての期待が読むものだけを数える。

| 数えるもの | 値 |
|---|---|
| principal の型 | action の `principal` の行の型 |
| 役割 | その型の `roles` のうち、その action のポリシーと期待が読む役割と、それを `includes` する役割の、どの組も（持たない組を含む） |
| 真偽、列挙の属性 | 全部の値（省ける属性は、無いことも一つの値） |
| 数の属性と `input` | ポリシーが比べる定数で範囲を切った区間。テストには区間の両端を使う（6.1） |
| 関係 | エンティティを指す項の、同じかどうかの分け方の全部と、分けた組ごとのメンバーであるか（2.5） |
| 規則の出力 | rulec が言う、とりうる値（入力がほかの条件にも使われるなら、区間ごとに） |
| 日付の述語 | koyomi が言う、起こりうる真偽の組 |

同じデータを読む値（`amount` と `refund_band`、`today` を読む二つの述語）は、まとめて、起こりうる組だけを数える。rulec と koyomi が正確に答えるかぎり、数えた組み合わせはどれも本当に起こりうるもので、起こりうるものは全部入る。

rulec か koyomi が、ある区間で出力がとりうる値を決められないとき（同じ入力を読む二つの導出に、整数ではとれない組を求める行など。3.2）は、その区間では出力の全部の値を起こりうるものとして数える。多めに数えても、すべての組み合わせについて言う検査（期待、職務の分離）が成り立つという答えは、そのまま正しい。

多めに数えた値には印を付ける。ある組み合わせがあることで決まる答え（permit が何かを許す、action をだれかが許される、役割が `can` の action を許される）と、成り立たない例は、印の無い組み合わせから言う。印のある組み合わせからしか出ないときは、区間の両端と真ん中と、端の一つ内側の値（日付も同じ）、真偽と列挙の全部の値を組み合わせた具体的な入力を、規則の参照の評価器にかけて探す。見つかればその答えのまま、見つからなければ W303（決められない）にする。試す組み合わせは、最初の 8 つと、そのあとは 2 の累乗の番目に出たものを、64 まで取る（最初の 16 だけでは組み合わせの一つの隅に偏り、届く値を見逃すことがあった）。

規則に尋ねる回数にも上限を置く。比べられる数の区間の組が 4,096 を超える規則のまとまりは、区間ごとではなく宣言した範囲の全体で一度尋ね、その答えを多めに数える。区間の組が多くても、検査の時間が rulec の解析の回数で伸びないようにするためである。

例の数：`view_order` 18 通り、`refund_order` 1,056 通り、`export_refunds` 4 通り、合わせて 1,078 通り。`refund_order` は、User では役割の組 8 × 停止中かどうか 2 × 注文の状態 4 × 額の区間と規則の答えの組 4 × 日付の述語の組 4、ワークフローでは状態 4 × 区間 2 × 組 4 である。`export_refunds` のポリシーが読む役割は auditor だけなので、役割の組は持つか持たないかの 2 通りで、停止中かどうかと合わせて 4 通りになる（設計の段階の試作は、どの action でも役割を三つとも数えて 16 通りとし、合わせて 1,090 通りとしていた）。

数える数には上限（予算）を置く。既定は koyomi と同じ 10⁸ 通りで、`--budget` で変えられる。超える action があれば E307 で止まり、何も生成しない。どの値が数を増やしているかを注に書く（役割が多いときは、`roles` を分けるか、`can` で役割ごとに確かめることを勧める）。サンプリングして続けることはしない。確かめていないのに通ったと言わないためで、koyomi の E305 と rulec と同じ考えである。

### 4.2 三つの結果

どの検査も、成り立つ（何も言わない）、成り立たない例がある（エラー。その組み合わせを示す）、決められない（W303。理由を言う）の三つのどれかである。決められない理由は、多めに数えた組み合わせから出た例を具体的な入力で起こせなかった（4.1）、読んだ Cedar に有限でない式があった（1.3）、のどちらかである。予算を超えたときは、決められないではなく、E307 で止まる（何も確かめていないので）。

`use gate` で読んだファイルの forbid（`action any`）は、読んだ側の action の判断に効くが、E302・E303・W301 は、読んだ側のファイルに書いたポリシーにだけ言う。読んだファイルのポリシーの誤りは、そのファイル自身の検査が言う（読む側で言うと、ほかのファイルの行を、読む側のファイルの位置として出すことになる）。

### 4.3 検査の一覧

| 検査 | 入れるか | コード | 理由 |
|---|---|---|---|
| どの permit も許さない action | 入れる | E301。`nobody "<理由>"` を action に書けば通す。`nobody` を書いた action をだれかが許される組み合わせがあれば、E304（だれにも許さないという期待が成り立たない）を `nobody` の行で言う | だれもできない action は、たいてい書き忘れ。意図なら理由を残させる |
| forbid に全部覆われて効かない permit | 入れる | E302。覆う forbid と、permit が許そうとした組み合わせの例 | 書いた人の意図と、実際が食い違っている |
| どの組み合わせでも条件が成り立たない permit・forbid | 入れる | E303 | rulec の E102（どの入力も当たらない行）と同じ考え |
| ほかの一つの permit で足りている permit | 入れる（警告） | W301。足りている相手の permit を言う | 消しても答えが変わらない。最小権限の見直しの手がかり。相手を一つに限るのは、二つの permit が互いを覆い合うとき、両方に「要らない」と言うと、両方を消して答えが変わるからである（試した。4.3 の最後の段落） |
| どの判断も変えない forbid | 入れない（ページに出す） | — | 守りを重ねる forbid（停止中の人は何もできない）は、今の permit では何も変えなくても、置いておく意味がある |
| 期待 | 入れる | E304。選ぶ組み合わせが一つも無ければ W304（警告） | 人が書いた期待を、全部の組み合わせで。何も選ばない期待は、成り立つが何も確かめていない |
| 職務の分離 | 入れる | E305 | 二つの役割を持つ人が両方できることは、組み合わせを全部見ないと見落とす |
| 役割の届く範囲（`can`） | 入れる | E306（`can` に無い action を許される）、W302（`can` の action を一度も許されない） | 最小権限を、役割ごとに書いて確かめる |
| 公開する操作に action があるか（X15） | 入れる（ritsu） | 4.6 | 認証（sec-design の W903）の先の、認可 |
| ワークフローが呼ぶ操作（X16） | 入れる（ritsu） | 4.6 | ワークフローの最小権限 |

例の答え（`tests/walk/golden/refunds.txt`。テストが書く、検査が数えた形）：

```
combinations: view_order 18, refund_order 1056, export_refunds 4; 1078 in all
permit staff_view_orders: allows 7 combinations, 7 of them alone
permit customers_view_their_orders: allows 1 combinations, 1 of them alone
permit clerks_refund_within_their_limit: allows 36 combinations, 12 of them alone
permit managers_refund_in_period: allows 48 combinations, 24 of them alone
permit managers_refund_late_on_business_days: allows 24 combinations, 24 of them alone
permit returns_refunds_returned_orders: allows 4 combinations, 4 of them alone
permit auditors_export_refunds: allows 1 combinations, 1 of them alone
forbid no_second_refund: holds on 264 combinations, and turns 112 that a permit allows into a deny
forbid auditors_do_not_refund: holds on 512 combinations, and turns 224 that a permit allows into a deny
forbid suspended_staff_do_nothing: holds on 522 combinations, and turns 232 that a permit allows into a deny
expect deny clerks_never_refund_over_their_limit: holds on all 128
expect deny nothing_refunded_twice: holds on all 264
expect allow managers_refund_in_period: holds on all 48
separate refund_order, export_refunds: holds
role clerk: can ["refund_order", "view_order"]
role manager: can ["refund_order", "view_order"]
role auditor: can ["export_refunds", "view_order"]
```

どの permit にも、その permit だけが許す組み合わせがある（W301 は出ない）。英語の版と日本語の版は、action ごと、ポリシーごと、期待ごとの数が全部同じになる（`tests/walk.rs` が確かめる）。

例を一行ずつ変えた変異（`tests/mutants/`。英語と日本語の対で、出力は `tests/golden/`）：

- forbid `clerks_do_not_refund`（principal in clerk、refund_order）を足すと、clerk の permit だけでなく、manager の二つの permit も E302 になる。manager は clerk を `includes` するので、clerk に向けた forbid が manager にも当たる。期待 `managers_refund_in_period` も 48 通りのうち 48 通りで成り立たず（E304）、clerk と manager の `can` の `refund_order` が W302 になる。
- clerk の permit に `unless in_period` を足すと、その permit が E303 になる（`principal` の行が当てはまる 768 通りのうち、三つの行はそれぞれ 384 通りで満たされるが、同時には満たされない）。
- permit `auditors_export_refunds` を消すと、`export_refunds` が E301 になり、役割 auditor の `can` が W302 になる。
- 期待 `managers_refund_in_period` から `unless principal in auditor` と `unless principal.suspended` の二行を除くと、192 通りのうち 144 通りで成り立たず（E304）、最初の例は停止中の manager である。
- forbid `auditors_do_not_refund` を消すと、職務の分離が、clerk と auditor の両方を持つ停止中でない User で成り立たない（E305）。
- permit `managers_refund_any`（manager、期間内で、規則の答えが over_limit）を足すと、それが許す 24 通りを `managers_refund_in_period` が全部許すので、W301 になる。`managers_refund_in_period` は、`clerks_refund_within_their_limit` と `managers_refund_any` の二つで全部覆われるが、一つの permit では覆われないので、W301 にしない。
- clerk の `can` から `refund_order` を除くと E306、`export_refunds` を足すと W302 になる。
- 期待に、同時には満たせない二つの行（注文の状態が `returned` と、`refunded`）を書くと、どの組み合わせも選ばないので W304 になる（`refund_order` の 1,056 通りのうち、二つの行はそれぞれ 264 通りで満たされる）。
- 契約と境目の変異：操作の名前の書き違い（E202）、操作の受け取らない input（E203）、操作に無い引数からの `from`（E204）、二つの action が守る一つの操作（E205）、規則の受け取る範囲を超える属性（E206）、カレンダーの知らない日まで延びる `today`（E207）、dandori の検査を通らないフロー（E208）、rulec の検査を通らない規則（E201）。

rulec の検査を通らない規則を読む E201 の変異は、二組ある。読んだファイルの誤りを並べる注の形を確かめる小さなゲート（`E201_a_rule_that_does_not_pass.gate` と `E201_検査を通らない規則.gate`）と、例の規則を通らないものに差し替えた返金のゲート（`E201_rule_does_not_check.gate` と `E201_通らない規則を読む返金.gate`）である。日本語の版が同じ名前になったので、どちらも残し、後者の名前を替えた。

### 4.4 役割の届く範囲

役割 R の届く範囲は、R（と R が `includes` する役割）だけを持つ principal が、どれかの組み合わせで許される action の集合である。ほかの属性（停止中か）は自由に選ぶので、「停止中でなければできる」ことは届く範囲に入る。`can` と比べ、多ければ E306（例つき）、少なければ W302。二つ以上の役割を持つ人の範囲は、職務の分離（E305）が見る。

### 4.5 職務の分離

`separate` の action の二つずつについて、同じ principal（型、役割の組、属性の値）で、それぞれの action を許す resource と context の組み合わせがあるかを数える。あれば、その principal と、それぞれの action を許す組み合わせを示す（E305）。数える役割は action ごとに違う（4.1）ので、二つの action の principal は、両方が読む役割と属性がそろうかで同じものとみなす。読んでいない役割は、その action の答えを変えないからである。役割の届く範囲（`can`）の確かめも同じに読む。

### 4.6 言語をまたぐもの（X15、X16）

ritsu-cross に置き、口だけを通す（ritsu の DESIGN 7 章）。番号は、セキュリティの検査（ritsu の X14 と、コードの 9xx の帯）の続きにした。

**X15：公開する操作に、守る action があるか。**

- 読むもの：sakai の地図（sec-design が足す口 `Maps` に、問い `published_operations` を足す。8.3）の、コンテキストごとの公開ホストサービスの操作。OpenAPI は `open host service` に並べた `operationId`（と `"GET /x"` の形）、proto は並べたサービスのメソッドの全部。sekisho の口 `Gates` の、action が守る操作の全部（`.gate` と、`@guards` を書いた Cedar のスキーマ）。
- 言うこと：コンテキストの公開する操作のうち、どの action も守らないもの。そのコンテキストの操作を一つでも守る action があれば、守られない操作ごとに E907。一つも無ければ、コンテキストごとに一つの W907（そのコンテキストは、まだ sekisho で認可を書いていない）。
- 通すもの：OpenAPI の操作が `security: []`（だれでも呼べると書いたもの。sec-design の W903 が意図として読むもの）なら、守る action を求めない。
- AsyncAPI のチャネルは求めない。メッセージの送り受けの認可は、多くはブローカー（Kafka の ACL など）が受け持ち、リクエストごとに Cedar に尋ねる形が少ない。`guards` には書ける。
- sec-design の W903（認証の書いていない操作）との分担：W903 は「呼ぶ人を確かめるか」を契約の文書で見る。X15 は「確かめた人に何を許すかが決まっているか」を見る。

**X16：ワークフローが呼ぶ操作と、ワークフローに許すもの。**

- 読むもの：`.gate` の `workflow` の行（`Gates`）、dandori のフローが呼ぶ操作（口 `Flows` に問い `operation_calls` を足す。8.3。タスクの行、呼ぶ操作の参照、タスクが宣言したエラーとその HTTP の状態か Connect のコード）、sekisho の参照の評価（`Gates::allowed`）。
- 言うこと：
  - E908：ワークフローが呼ぶ操作を守る action が、そのワークフローをどの組み合わせでも許さない（その呼び出しに来る実行は、いつも拒まれる）。
  - W909：組み合わせによっては拒まれるのに、タスクが拒まれたとき（OpenAPI の操作の 403、Connect の `permission_denied`）のエラーを宣言していない。拒まれると、ワークフローは宣言していない失敗で止まる。
  - W908：ワークフローが許される action のうち、フローがどこでも呼ばない操作のもの（要るより多く許している）。
- 例：ワークフロー `returns` は `refund_order` を 32 通りのうち 4 通りで許され、28 通りで拒まれる。タスク `refund_order` は `errors denied = 403` を宣言しているので W909 は出ない。許されて呼ばないものは無い（試作の出力）。

### 4.7 Cedar の解析との関係

Cedar Analysis（2025-06 に公開）の四つの検出は、sekisho の検査と重なる。「Forbid Overrides」は E302、「Impossible Conditions」は E303、「Shadowed Permits」は W301、「Complete Denials」は E301 にあたる。違うのは、SymCC が context の値を自由な値として扱うのに対し、sekisho は計算した値がとりうる値（rulec と koyomi の答え）だけを数えることである。koyomi の日付で、ある述語の組が起こりえなければ、SymCC はそれを起こりうるものとして答え、sekisho は数えない。

sekisho は SMT のソルバーに頼らない。ritsu の検査は、ネットワークも外のソルバーも使わずに走る形をとってきたからである。生成した Cedar を SymCC で確かめることは、cvc5 があるときだけ走るテストとして足せる（15 章）。

## 5. 生成するもの

`sekisho gen <file.gate>... --target cedar|typescript|python|go [--authorizer cedar|avp] [--out <dir>] [--check]` が書く。`ritsu gen` は、プロジェクトの `.gate` から、同じものをパッケージの中に書く（5.6）。段階 B で `--target cedar` を作った。`typescript`・`python`・`go` と `--authorizer` は段階 C で足す。`--target` は省けない。三つの言語のコードは、サービスの言語を一つ選んで書くもので、全部をいつも書くものではないからである。

`--target cedar` は、`.gate` ごとに `<out>/cedar/` の下に四つのファイルを、ファイルの別名で書く（`--out` の既定は `generated`。`ritsu gen` の `generated/cedar/` と同じ形）。検査を通らないファイルからは何も書かない（その診断を出し、exit 1）。ほかの言語を読むファイルを sekisho のクレートのバイナリで走らせれば、`check` と同じく E209 で exit 2 になる。`--check` は何も書かず、ディスクのファイルが、いま書くものと一字も違わないかを見る（違うファイルと無いファイルを一行ずつ言い、exit 1。koyomi の `gen --check` と同じ）。

```
$ ritsu sekisho gen examples/refunds/refunds.gate --target cedar --out generated
generated: generated/cedar/refunds.cedar
generated: generated/cedar/refunds.cedarschema
generated: generated/cedar/refunds.cedarschema.json
generated: generated/cedar/refunds.policies.json
```

生成物の頭は、ほかの言語の生成器と同じ二行である（ritsu-emit の `header`。ritsu の DESIGN 9.2）。JSON の二つのファイルには頭を書かない（`cedar translate-*` の出力と一字も違わない形にするため）。

```cedar
// Code generated by sekisho 0.23.0. DO NOT EDIT.
// Source: refunds.gate (gate refunds v1, sha256:678573b010ccf8c4)
```

sekisho が自分で書く文（頭の二行目と、計算した値・`input`・列挙と数の属性・役割・ワークフローの `@doc`）は、`--lang` の言語で書く（koyomi の `gen` と同じ）。`.gate` に書いた `description` は、そのまま `@doc` になる。言語を変えると生成物も変わるので、`--check` は書いたときと同じ言語で走らせる。

### 5.1 Cedar のスキーマ

| `.gate` | Cedar のスキーマ |
|---|---|
| `namespace Shop`（無ければ、ファイルの別名をパスカルケースにしたもの） | `namespace Shop { … }` |
| 役割（一つでもあれば） | `entity Role in [Role];`（親子を書くため。`in [Role]` が無いと、manager の親を clerk にしたエンティティをスキーマが受け付けない。試した） |
| `principal User` と `roles` | `entity User in [Role] = { … };`。グループのメンバーであることをポリシーが聞く型（`principal in resource.team`）は、そのグループの型も並べる（`entity User in [Role, Team]`） |
| `workflow`（一つでもあれば） | `entity Workflow;` |
| `resource Order` | `entity Order = { … };`（属性が無ければ `entity Order;`） |
| 属性 | ポリシーが読むものだけ（2.4）。それも、そのポリシーが当てはまりうる型の属性だけを数える（`principal is Bot` のポリシーが読む `principal.dept` は、User も `dept` を持っていても、Bot の属性としてだけ出す）。`bool` は `Bool`、列挙は `String`、数は `Long`、エンティティの型はその型、省けるものは `?` |
| `action` | `action "refund_order" appliesTo { principal: […], resource: […], context: { … } };` |
| `input` と計算した値 | `context` のレコードの属性。ポリシーが読む `input` と、計算した値の全部。計算した値は、次のどれかのとき省ける（`?`）：action の principal か resource の型のどれかが、計算に使う属性を持たない（例の `refund_band` はワークフローでは計算しない）。計算に使う属性か `input` が省ける。検査が、値の無い組み合わせを数えた（koyomi の日付が止まるとき） |
| `description` | `@doc("…")` |
| 日本語の名前（名前が別名と違うもの） | `@name("…")`（型、属性、action、`input`、計算した値。5.7） |
| `guards` | `@guards("<参照の書き方>")`。二つ以上なら一行に一つ |

スキーマは人が読む形（`.cedarschema`）と JSON の形（`.cedarschema.json`。Verified Permissions の `PutSchema` が受け取る形）の両方を書く。人が読む形は、土台の `ritsu_base::cedar::write_schema` で書く。これは `cedar translate-schema --direction json-to-cedar` が書く形で、エンティティタイプと action と属性は名前の順に並び、action の名前は引用符で書き、文字列の中の `'` は `\'` になる（Cedar の CLI と同じ）。頭の二行のあとは、CLI のその出力から最後の改行を一つ除いたものと一字も同じである。JSON の形は、書いたテキストを `ritsu_base::cedar` で読み直して `schema_to_json` で書いたもので、`cedar translate-schema --direction cedar-to-json` の出力と一字も違わない（一行と改行）。

計算した値の `@doc` には、何から計算したかを ritsu の参照の書き方で書き、渡す値、Cedar に渡る値、計算する型（すべての型でないとき）、`today` を読むなら日を変えるオフセット、生成したコードが計算することを書く。例の `refund_order`（実物）：

```cedarschema
  @doc("Refund an order, in part or in whole")
  @guards("openapi \"api/orders.json\" operation refundOrder")
  action "refund_order" appliesTo {
    principal: [User, Workflow],
    resource: [Order],
    context: {
      @doc("An argument of the operation: money[GBP, incl_tax], 1 to 10000")
      amount: Long,
      @doc("today is open in koyomi \"calendars/england_and_wales.cal\", the day changing at +00:00. Computed by the generated code, never taken from the caller")
      business_day: Bool,
      @doc("today <= koyomi \"dates/refund_terms.cal\" date last_day, from resource.paid_on, the day changing at +00:00. Computed by the generated code, never taken from the caller")
      in_period: Bool,
      @doc("rulec \"rules/refund_limit.rule\" output band, from amount and principal.refund_limit: within_limit or over_limit. Only for a principal of type User. Computed by the generated code, never taken from the caller")
      refund_band?: String
    }
  };
```

日本語の版を `--lang ja` で書くと、`refund_band` の `@doc` は「rulec "rules/返金の上限.rule" output 区分。金額、principal.返金できる額 から計算する。値は within_limit か over_limit。principal の型が User のときだけ計算する。生成したコードが計算し、呼ぶ側からは受け取らない」になる。ほかの `@doc`：`input` は操作の引数であること（操作を守らない action では、リクエストの引数）と型と範囲、列挙の属性は値の並び（「One of paid, shipped, returned, refunded」。日本語の名前があれば `paid (支払済)`）、数の属性は型と範囲、役割の型は役割ごとの別名と日本語の名前と説明と `includes`、`Workflow` の型はワークフローごとの別名と説明と `.flow`（`dandori "flows/returns.flow"`）。

`@guards` は、守る操作を参照の書き方で書く。パスは `use` の行に書いたまま（`.gate` のディレクトリから）である。

- OpenAPI：`openapi "api/orders.json" operation refundOrder`（`operationId` の無い操作は `operation "POST /orders/{orderId}/refunds"`）
- AsyncAPI：`asyncapi "<パス>" operation <操作のキー>`
- proto：`proto "<パス>" service <サービス> method <メソッド>`（サービスは短い名前）
- 帳簿の振替の操作（`guards stock receive.do`）：`chobo "<パス>" transfer receive operation do`。参照の書き方の chobo の種類には、振替の操作がまだ無い（`transfer` の下に `operation` を足すかは決めていない）。

一つの action が二つ以上の操作を守るときは、一つの `@guards` に、参照を一行に一つ並べる（Cedar の注釈は、一つの宣言に同じキーを二つ持てない）。参照の文字列は `sekisho::cedar::guard_reference` が作り、口 `Gates`（8.3）が手で書いた Cedar の `@guards` を読むときも、同じ形で読む。

### 5.2 ポリシー

- ポリシーごとに `@id("<ファイルの別名>/<ポリシーの別名>")` を付け、名前が別名と違えば `@name`、`description` を書いたなら `@doc` を付ける。ID は `[a-z0-9_/]` だけでできる。Verified Permissions のポリシーの名前（`name`。`[a-zA-Z0-9-/_]*`、150 字まで）にそのまま使える形である（5.8）。`use gate` で読んだファイルの forbid は `<読んだ側の別名>/<読んだファイルの別名>/<ポリシーの別名>` である（2.10。`tests/gen/papers.gate` の `papers/people/suspended_do_nothing`）。
- スコープ：principal は `principal` の行から作る。役割が一つなら `principal in …::Role::"clerk"`、`principal in clerk, auditor` のように二つ以上なら、スコープに書けない（Cedar の principal のスコープは一つのエンティティしか取らない）ので、最初の条件を `when { principal in …::"clerk" || principal in …::"auditor" }` にする。`is <型>` は `principal is …`、`is workflow <名前>` は `principal == …::Workflow::"<名前>"`。action は、並べた action が一つなら `action == …`、二つ以上か `action any` なら `action in [ … ]`（`action any` は、このファイルの action の全部。2.10）。resource は、ポリシーの action がとる resource の型が一つなら `resource is …`、二つ以上なら何も書かない。
- 条件は 3.1 の表のとおり。値が無いことがある読みは、`has` で守る。属性は、ポリシーが当てはまりうる型（action の principal の型を `principal` の行で絞ったもの、resource の型）のどれかが、その属性を持たないか、省ける属性として持つとき。context の値は、ポリシーの action のどれかが、その値を持たないか、省ける値として持つとき。守った条件は `has … && …` の形で、`is not` と `not` は、その外側に `!` を付ける（値が無いとき、条件は当てはまらず、`x is not v` は成り立つ。3.1）。Cedar の strict な検証は、`has` の無い読みを、その属性を持たない型があればエラーにする。また、グループのメンバーを聞く条件（`principal in resource.team`）は、principal の型をそのグループに入れないスキーマでは「どのリクエストでも当てはまらない」という警告になる。どちらも CLI 4.13.0 で確かめた。
- action の無いポリシー（action の無いファイルの `action any`）は書かない。どの組み合わせにも当てはまらないからである（`tests/gen/people.gate` の `.cedar` は頭の二行だけになる）。
- テキストは、頭の二行のあとに `ritsu_base::cedar::write_policies` の出力を置き、`format_policies`（`cedar format` と同じ。幅 80、字下げ 2）で整えたもので、`cedar format` の出力と一字も違わない。テストが `cedar format --check` で確かめる。長い条件は `cedar format` が折る。日本語の版は名前空間が `ShopJa` と二文字長いので、`staff_view_orders` の条件が `when` のあとで改行される。英語と日本語の版の Cedar は、テキストではなく JSON の形で比べる（`tests/gen.rs`）。
- ポリシーは二つの形で書く。人が読み、Verified Permissions に置く `.cedar` と、`@id` をキーにした JSON の形（`.policies.json`。`cedar translate-policy --direction cedar-to-json` の出力と同じ）である。JSON の形が要るのは、四つの実装のうち三つ（cedar-wasm、cedarpy、cedar-go）が、一つのテキストのポリシーに `policy0`、`policy1`… と名前を付け、`@id` を読まないからである（6.2 で試した）。そのまま渡すと、決めたポリシーの名前が `.gate` の名前と結び付かない。公式の CLI だけは `@id` を名前にする。

例（`sekisho gen` の実物。設計の段階に手で書いた見本と、頭の二行のほかは同じになった）：

```cedar
@id("refunds/clerks_refund_within_their_limit")
@doc("A clerk refunds up to the clerk's own limit, while the refund period lasts")
permit (
  principal in Shop::Role::"clerk",
  action == Shop::Action::"refund_order",
  resource is Shop::Order
)
when { context has refund_band && context.refund_band == "within_limit" }
when { context.in_period };
```

条件の形を全部並べた試しのゲート（`tests/gen/conditions.gate`）からは、たとえば次が出る。`resource.reviewer` は省ける属性なので `has` で守り、`principal in resource.team` のために、スキーマは `entity User in [Role, Team]` になる。

```cedar
@id("conditions/editors_edit")
@doc("An editor in the document's team, or who reviews it, edits it when it is not high or the editor's clearance is 3 or more")
permit (
  principal in Lab::Role::"editor",
  action == Lab::Action::"edit",
  resource is Lab::Doc
)
when
{
  principal in resource.team ||
  resource has reviewer &&
  resource.reviewer == principal
}
when { !(resource.level == "high") || principal.clearance >= 3 };
```

### 5.3 リクエストを組み立てるコード

action ごとに、TypeScript・Python・Go で二つの関数を生成する。

- `<action>Request(store, principal, input, now)`：`Store` から principal と resource を読み、`input` と範囲を確かめ、rulec と koyomi の生成物を呼んで計算した値を求め、Cedar のリクエストとエンティティの JSON を返す。計算できなければ、種類の付いたエラー（`principal`・`resource`・`input`・`today`・`rule`・`date`）を投げる（3.8）。
- `authorize<Action>(store, principal, input, now)`：上で組み立てて、Cedar に尋ね、答え（許すか、決めたポリシーの `@id`、拒んだ理由）を返す。エラーは拒む答えにする。答えには、Cedar に渡した context も入れる。判断の記録（監査のログ）に、計算した値と決めたポリシーを一緒に残せるようにするためである。

```ts
// Code generated by sekisho 0.24.0. DO NOT EDIT.   （設計の段階に手で書いた見本から）
export interface RefundOrderInput {
  orderId: string;
  /** money[GBP, incl_tax], 1 to 10000 */
  amount: GBPInclTax;
}

export async function refundOrderRequest(store: Store, principal: Principal, input: RefundOrderInput, now: Date = new Date()): Promise<Request> {
  …
  if (user !== undefined) {
    // rulec "rules/refund_limit.rule" output band, from amount and principal.refund_limit
    const band: RefundBand = refund_limit(input.amount, user.refund_limit);
    context.refund_band = band;
  }
  // today <= koyomi "dates/refund_terms.cal" date last_day, from resource.paid_on
  context.in_period = day <= refund_terms_last_day(order.paid_on);
  // today is open in koyomi "calendars/england_and_wales.cal"
  context.business_day = uk_is_open(day);
  …
```

規則と日付の生成物の呼び方は、口から取る。rulec の関数の名前と型は `RuleFacts::typescript`・`python`・`go`（ritsu の DESIGN 3.2）、koyomi の関数の名前は dandori と同じく ritsu-emit の決め方で求める（ritsu の DESIGN 3.2 の E.5）。

例の見本は三つの言語とも、ritsu 0.23.0 の `rulec gen` と `koyomi gen` が書いた三つのモジュール（規則、日付、カレンダー）を呼び、生の値から組み立てた 11 のリクエストに、三つとも同じ、期待どおりの答えを返した（TypeScript は cedar-wasm 4.13.0、Python は cedarpy 4.12.1、Go は cedar-go v1.8.0。6.3）。Go では、koyomi の日付のパッケージがそれぞれ自分の `Date` の型を持つので、生成するコードは、日付のファイルとカレンダーのあいだで年月日を詰め替える。

### 5.4 リクエストとエンティティの JSON

Cedar のエンティティの JSON の形（`{"uid": {"type", "id"}, "attrs", "parents"}`）で書く。

- 役割のエンティティ：`.gate` の役割と `includes` から作った定数。
- principal：型ごとに、ポリシーが読む属性と、`Store` が返した役割を親にしたもの。ワークフローは属性を持たない。
- resource：ポリシーが読む属性。エンティティの型の属性は `{"__entity": {…}}` で書き、指す先のエンティティも入れる（例の `customer`）。
- context：ポリシーが読む `input`（宣言した単位で数えた整数）と、計算した値。

数は JSON の数として書く。TypeScript の `number` で正確に表せるのは 2⁵³ までなので、宣言する範囲は ±(2⁵³ − 1) に収める（E103）。

### 5.5 Cedar の実装の呼び方（2026-10 の時点）

| 実装 | 版（確かめた日） | 生成するコードからの呼び方 | 気をつけること |
|---|---|---|---|
| 公式の CLI `cedar-policy-cli` | 4.13.0（2026-09-15。Cedar の言語の版は 4.5） | テストだけで使う（`validate`・`format --check`・`run-tests`・`translate-*`） | `@id` を名前にする |
| Rust `cedar-policy` | 4.13.0（crates.io、2026-09-15） | 生成しない（Rust は生成先に無い。9.3 の三つの言語） | — |
| npm `@cedar-policy/cedar-wasm` | 4.13.0（2026-09-15） | `isAuthorized({principal, action, resource, context, entities, policies: {staticPolicies: {<@id>: <ポリシー>}}, schema, validateRequest: true})`。Node は `@cedar-policy/cedar-wasm/nodejs` | 一つのテキストを渡すと `policy0`… になる |
| Go `github.com/cedar-policy/cedar-go` | v1.8.0（2026-06-01） | `json.Unmarshal` で JSON の形のポリシーを `cedar.PolicySet` に読み、`cedar.Authorize(ps, entities, req)` | 検証器は実験（`x/exp/schema`）で、テンプレートと部分評価が無い（sekisho は使わない）。`NewPolicySetFromBytes` は `policy0`… |
| Python `cedarpy`（k9securityio。AWS の公式ではない） | 4.12.1（2026-09-24。Cedar 4.12.0） | `cedarpy.PolicySet.from_json_str(…)` を一度作り、`is_authorized(request, policies, entities, schema=…)` | 文字列で渡すと `policy0`… |
| Amazon Verified Permissions | Cedar 4 に上げた（`is`、タグ、`datetime` と `duration` の値が使える。Verified Permissions の Cedar 4 の FAQ） | `IsAuthorized`（`entities` と `context` は `cedarJson` で渡せる） | 決めたポリシーは生成された `policyId` で返る（5.8） |

`--authorizer cedar`（既定）は、その言語の Cedar の実装をプロセスの中で呼ぶ。`--authorizer avp` は、AWS の SDK（`@aws-sdk/client-verifiedpermissions`、boto3、`aws-sdk-go-v2/service/verifiedpermissions`）で `IsAuthorized` を呼ぶ。どちらも、組み立てる関数（`<action>Request`）は同じである。

### 5.6 `ritsu gen` のパッケージへの入り方

ritsu の DESIGN 9.3 のパッケージに、次を足す。

```
generated/cedar/<別名>.cedar, <別名>.cedarschema, <別名>.cedarschema.json, <別名>.policies.json   言語に依らない（一度だけ書く）
generated/typescript/authz/<別名>.ts
generated/python/<name>/authz/<別名>.py
generated/go/authz/<パッケージ>/<パッケージ>.go
```

- 認可のモジュールは、同じパッケージの `rules/` と `dates/` の生成物を読む（ワークフローと同じ）。インデックスのファイル（`index.ts`、`__init__.py`）に `authz` を足す。
- 依存は、`.gate` があるときだけ書く。TypeScript は `@cedar-policy/cedar-wasm` を `"4.13.0"`、Python は `cedarpy==4.12.1`、Go は `doc.go` に `github.com/cedar-policy/cedar-go v1.8.0`。`--authorizer avp` なら AWS の SDK。書くバージョンは、sekisho の `tools/` のランナーのロックファイルのバージョンと同じにし、ritsu の監査（ritsu の DESIGN 3.6）がそのロックファイルを調べる。`crates/ritsu/tests/audit.rs` の確かめに sekisho のランナーを足す。
- ポリシーとスキーマは、生成するモジュールの中に定数として書き込む（koyomi が祝日の表を書き込むのと同じ）。リクエストを組み立てるコードと、評価するポリシーの版が食い違わないようにするためである。Verified Permissions を使うときは、ストアに置いたポリシーと生成したコードの版を合わせるのは、置く側の仕事になる（5.8）。

### 5.7 日本語の名前と ASCII

Cedar の識別子は ASCII（`[_a-zA-Z][_a-zA-Z0-9]*`。Cedar 4.13.0 の文法 `grammar.lalrpop` の `IDENTIFIER`）で、名前空間、型、属性の名前（引用符なしで書くもの）になれるのはこれだけである。

**決定**：Cedar に出る名前（ファイル、役割、型、属性、列挙と値、action、`input`、計算した値、ポリシー）には、名前が ASCII の形でなければ、rulec と koyomi と同じく丸括弧で ASCII の別名を付ける（無ければ E007）。型の別名は `[A-Z][A-Za-z0-9]*`、ほかは `[a-z][a-z0-9_]*`。名前空間は ASCII で書く。名前が別名と違うもの（日本語の名前）は、注釈 `@name("係は上限まで返金できる")` に名前を書き、説明は `@doc` に書く。`expect` と `separate` は Cedar に出ないので、別名は要らない。

```gate
permit 係は上限まで返金できる(clerks_refund_within_their_limit)
  description "係は、返金の期間のあいだ、自分の上限まで返金できる"
  principal in 係
  action 返金する
  when 返金の区分 is 上限まで
  when 期間内
```

**捨てた案**：日本語を Cedar の文字列に入れる。エンティティの ID（`Shop::Role::"係"`）と、引用符で書く属性の名前（`context["返金の区分"]`）なら日本語も書ける。しかし、Verified Permissions のポリシーの名前は ASCII に限られ（5.8）、action の ID は契約の `operationId` と並べて読むことが多く、引用符の属性は生成するコードでも読みにくい。名前空間と型には、そもそも書けない。

英語の版と日本語の版（`refunds.gate` と `refunds.ja.gate`）は、別名をそろえておけば、名前空間、ID の頭、`@name` と `@doc` の文のほかは同じ Cedar になる。これを双子のテストにする（rulec のコーパスの双子と同じ考え。6.3）。

### 5.8 Verified Permissions に置くもの

sekisho は Verified Permissions に何も送らない（ritsu の検査と生成はネットワークを使わない）。置くためのファイルを書くだけである。

- スキーマ：`.cedarschema.json` を `PutSchema` に渡す。ポリシーより先に置く（Verified Permissions は `CreatePolicy` のときに、ストアのスキーマでポリシーを検証する）。context の値を足すときも、スキーマを先に変える。
- ポリシー：`.policies.json` の一つずつを `CreatePolicy` の `definition.static.statement` にし、`name` に `@id` を渡す。名前はポリシーの ID の代わりに使える（API に渡すときは頭に `name/` を付ける）。
- `IsAuthorized` の `determiningPolicies` は、ストアが付けた `policyId` を返す。生成する `--authorizer avp` のコードは、それをそのまま返し、ページと文書に、名前で引けることを書く。
- 上限（2026-10 の Verified Permissions のクォータ）：一つのポリシーは 10,000 バイトまで、スキーマは 100,000 バイトまで、一つのスキーマの名前空間は 100 まで、principal・action・resource のそれぞれの推移的な親は 100 まで、一つの認可のリクエストは 1 MB まで。`sekisho gen --authorizer avp` は、ポリシーとスキーマの大きさと、役割の親の深さがこれを超えれば W401 で言う。
- 置く手順（AWS の CLI、CloudFormation の `AWS::VerifiedPermissions::Policy` など）は生成しない。置くのは使う人で、sekisho が書くのは置くためのファイルだけである。

## 6. 本物の Cedar との突き合わせのテスト

### 6.1 決め：公式の CLI の `run-tests` で、全部の組み合わせ

- `sekisho vectors <file.gate> [--action <action>]` が、全部の組み合わせを、公式の CLI の `cedar run-tests` が読む形で書く。JSON の配列で、一行に一つのテスト（`name`、`request`、`entities`、`decision`、`reason`、`num_errors` の順）である。`decision` と `reason`（決めたポリシーの `@id`）は sekisho の参照の評価の答えで、`num_errors` はいつも 0 である。
  - 数の区間は、両端の値を一つずつ使う（境目の一つずれを見つけるため。rulec の vectors と同じ考え）。区間が二つ以上の値を持つ組み合わせは、リクエストに入る数を全部 low の端にしたものと、全部 high の端にしたものの二件にする（名前は `refund_order 1 (amount 1)` と `refund_order 1 (amount 50)`）。区間の中では、どの比べも一つの答えになるので、二件の答えは同じである。区間が一つの値か、数の無い組み合わせは一件（`view_order 1`）。
  - エンティティは、役割（`includes` した役割を親に）、principal（持つ役割と、メンバーであるグループを親に）、resource と、属性が指すエンティティのうち、スキーマで属性を持たない型のもの（例の `Customer`）。属性を持つ型のもの（`tests/walk/relations/tenants.gate` の、principal でない `owner`）は書かない。Cedar はそのエンティティの属性を読まず、リクエストは、エンティティを書かずに指してよい。
  - ID は、型の別名を小文字にして番号を付ける（`user1`、`order1`、`customer2`）。同じエンティティを指す項（2.5 の分け方の一つの組）は同じ ID で、principal を指すなら principal の ID である。ワークフローの principal は、ワークフローの別名を ID にする。
  - 組み合わせが数えない値（ほかの action のポリシーだけが読む属性など）は、その値がとりうる最初の値にする（真偽は false、列挙は最初の値、数は下限）。省けるものは書かない。
  - 例の英語の版では、1,078 通りから 2,134 件になる（`view_order` 18、`refund_order` 1,056 × 2、`export_refunds` 4）。日本語の版は、名前空間と ID の頭のほかは一字も同じである（`tests/cli.rs`）。
- テストは、生成した Cedar に、`cedar validate`（strict）、`cedar format --check`、`cedar run-tests` をかけ、JSON の形を `cedar translate-schema` と `translate-policy` の出力と比べる。
- CLI は、公式のリリースのバイナリ（`cedar-policy-cli` 4.13.0。macOS と Linux の四つ）を、チェックサムで確かめて置く。CI は ritsu の `tools.yml` が wasm-tools と protoc を入れるのと同じ形で入れる。テストは環境変数 `RITSU_CEDAR` で受け取り、無ければ `SKIP: sekisho: …` を言う（ritsu の DESIGN 10.3）。Cedar の読み手と書き手（`ritsu_base::cedar`）のテストの材料も同じバイナリで作るので、CI の入れ方は一つにする。
- ほかの言語のランナーと突き合わせのテストと同じ形にする：生成物（Cedar）を、本物の評価器（公式の CLI）に、参照の評価が作った全部の場合をかけ、一字も違わないこと。dandori が Temporal の dev server で、chobo が PostgreSQL と TigerBeetle で確かめるのと同じ位置にある。

### 6.2 試したこと（2026-10-06）

例 `refunds.gate` を、sekisho が生成する形で手で Cedar に書き、検査が数える形で書いた試作（Python）で 1,090 通りの組み合わせと答えを作って、四つの実装にかけた。段階 A の検査は 1,078 通りを数える（4.1。試作は `export_refunds` で、ポリシーが読まない役割も数えていた）。段階 B の `sekisho vectors` が書くのはこの 1,078 通りなので、突き合わせを流したら、この表をその数で取り直す。

| 実装 | 結果 | 時間 |
|---|---|---|
| `cedar validate`（4.13.0、strict） | `policy set validation passed`、エラーも警告も無し | — |
| `cedar format --check` | 書いたテキストが整形の結果と同じ（最初は二か所違った：頭のコメントのあとの空行と、一行に収まる `when`。整形の結果を生成の形にした） | — |
| `cedar run-tests` | `results: 1090 passed, 0 failed` | 0.17 秒 |
| cedar-wasm 4.13.0（Node 23） | 1,090 通りとも一致（ポリシーを `@id` ごとに渡したとき）。一つのテキストで渡すと 134 一致、956 不一致（決めたポリシーが `policy0`… になる） | 1.4 秒 |
| cedarpy 4.12.1（Python 3.13） | 1,090 通りとも一致（`PolicySet.from_json_str`）。文字列で渡すと同じく `policy0`… | 0.4 秒 |
| cedar-go v1.8.0（Go 1.25.5） | 1,090 通りとも一致（JSON の形を `json.Unmarshal`） | 2.6 秒（`go run` のビルドを含む） |

テストが誤りを見つけることも試した。ワークフローの permit の `context.amount <= 50` を `< 50` に変えると、`run-tests` は 4 通りで落ちた（区間の端の 50 を使っているため）。`context has refund_band &&` を消すと、`cedar validate` が `unable to guarantee safety of access to optional attribute refund_band` で落ちた。最初に書いたスキーマは `entity Role;` で、manager の親を clerk にしたエンティティを CLI が受け付けなかった（`Shop::Role::"manager"` is not allowed to have an ancestor of type `Shop::Role`）。5.1 の `entity Role in [Role]` はこれで決めた。

### 6.3 生成したコードのテスト（段階 C）

- 生の値の作り方：組み合わせごとに、区間の両端の値、規則の入力は rulec の vectors（境目から作った入力）、日付は koyomi が数えた日のうち述語の真偽が変わる日の前後を使い、`Store` の中身、`input`、時刻を作る。
- TypeScript（Node と cedar-wasm、`tsc --strict`）、Python（cedarpy、`mypy --strict`）、Go（cedar-go、`go vet` と gofmt）で、生成したコードに生の値を渡して答えを得て、同じ生の値から sekisho の参照の評価（rulec と koyomi の参照の評価器を口で呼ぶ）で求めた答えと比べる。決めたポリシーまで一致すること。
- 試し：例の三つの言語の見本（`generated/typescript`・`python`・`go`）に、11 の場合（上限の内と外、期間の最後の日と次の日、期間のあとの金曜と土曜、監査を兼ねる係、停止中の manager、ワークフローの 50 と 51 ポンド、返金済の注文、範囲の外の日）を生の値から渡し、三つとも同じ、期待どおりの答えを返した。規則と日付は、ritsu 0.23.0 の `rulec gen` と `koyomi gen` が書いた本物の生成物を呼んだ。TypeScript は `tsc --strict`（5.9.3）、Python は `mypy --strict`（2.4.0）、Go は `go vet` と gofmt を通した。
- `--authorizer avp` のコードは、型の検査だけをする（ネットワークを使わない。本物の Verified Permissions で走らせて確かめることは、まだしていない。15 章）。
- 英語の版と日本語の版の双子：`refunds.gate` と `refunds.ja.gate` から生成した Cedar が、名前空間、ID の頭、`@name` と `@doc` のほかは同じであること、`vectors` の答えが同じであること。

### 6.4 捨てた案

- **`tools/` の下の小さな Rust のプロジェクト（ワークスペースの外）で `cedar-policy` を呼ぶ**：CLI と同じ評価器を、テストのたびに数百のクレートごとビルドすることになる。CLI のバイナリは、Cedar の読み手と書き手（`ritsu_base::cedar`）のテストも同じものを使う。
- **cedar-wasm だけで確かめる**：段階 B の確かめ（生成した Cedar）に Node を要ることになる。cedar-wasm は段階 C の TypeScript の生成物のテストで使う。
- **SymCC で生成した Cedar と sekisho の表が同じかを確かめる**：cvc5 が要り、ネットワークも要る（取ってくるとき）。一つ一つのリクエストの答えを比べるには、`run-tests` で足りる。

## 7. 人が読むページ（`sekisho doc`）

`sekisho doc <file.gate> [--format html|markdown] [--lang en|ja]`。HTML は ritsu の土台の枠（`ritsu_base::docpage` の `html_head`、`Palette`、`HTML_TAIL`）で、外のファイルを読まない一枚のページ、Markdown は GitHub がそのまま見せるもの（ritsu の DESIGN 4.8）。英語と日本語。

ページの中身：

1. **頭**：`.gate` のパスとハッシュ、sekisho の版、名前空間、生成した Cedar のハッシュ。
2. **action ごとの表**：許す行を先に、合わせた形で出す。例の `refund_order` の 1,056 通りは、許す行が 5 行、拒む行が 21 行の 26 行になる（`tests/walk/golden/refund_order.table.md`）。列は principal の型、持つ役割、ワークフロー、停止中か、注文の状態、額、規則の答え、期間内か、営業日か。規則の答えは、`.gate` と並べて読めるよう、`.rule` に書いた名前で出す（日本語の版では `上限まで`・`上限超え`）。Cedar に渡す文字列（`.rule` に書いた別名、`within_limit`）は、3. のポリシーの生成した Cedar に出る。許す 5 行は次のとおり。

   | principal | clerk | manager | auditor | workflow | suspended | status | amount | refund_band | in_period | business_day | -> | policies |
   |---|---|---|---|---|---|---|---|---|---|---|---|---|
   | User | yes | no | no | - | no | not refunded | any | within_limit | yes | any | allow | clerks_refund_within_their_limit |
   | User | yes | yes | no | - | no | not refunded | any | within_limit | yes | any | allow | clerks_refund_within_their_limit, managers_refund_in_period |
   | User | yes | yes | no | - | no | not refunded | any | over_limit | yes | any | allow | managers_refund_in_period |
   | User | yes | yes | no | - | no | not refunded | any | any | no | yes | allow | managers_refund_late_on_business_days |
   | Workflow | - | - | - | returns | - | returned | <=50GBP | - | any | any | allow | returns_refunds_returned_orders |

   設計の段階の表に無かった列は「ワークフロー」で、`principal is workflow returns` を読むので、ワークフローの名前を列にする。

   行の合わせ方：役割は、持つ役割（`includes` をたどったもの）を列にする。答えと決めたポリシーが同じで、一つの列だけが違う行を、その列の値の集合にまとめ、変わらなくなるまで繰り返す。集合が全部の値なら any、一つを除く全部なら not … と書く。拒む行は、決めた forbid ごとにまとめ、残りを「どの permit も当てはまらない」として出す（例では 21 行。forbid が重なる組み合わせが多いので、ページでは forbid ごとの条件を先に見せ、表は畳んだ節に入れる）。
3. **ポリシー**：`.gate` の行と、生成した Cedar を並べる。それぞれのポリシーが許す（拒む）組み合わせの数と、そのポリシーだけが決める数（4.3 の数）。規則の答えを比べる条件は、`.gate` の行では `.rule` に書いた名前（`上限まで`）、生成した Cedar では別名の文字列（`"within_limit"`）になるので、二つを並べたここで対応が読める。
4. **計算した値**：値ごとに、どの言語のどのファイルの何から計算したか、とりうる値、規則と日付のページ。rulec の規則は、dandori と同じく `rulec doc` が描いたものを埋め込む（HTML は上に開くシート、Markdown は畳んだ節。口 `Rules::doc`）。koyomi の日付とカレンダーも同じ形で埋め込む（口に `Dates::doc` を足す。8.3）。
5. **期待、職務の分離、役割の届く範囲**：それぞれの結果。役割と action の表（いつも許す、組み合わせによる、許さない）。
6. **ワークフロー**：`workflow` ごとに、許される action と、`ritsu check` のときは、フローが呼ぶ操作（X16 の結果）。
7. **守る操作**：action ごとに、守る操作（OpenAPI の操作の方法とパス、proto のメソッド）と、どの操作も守らない action。

## 8. ほかの言語とのつながり（口）

### 8.1 読むもの

| 相手 | 口と問い | 使うところ |
|---|---|---|
| rulec | `Rules::facts`（入力と出力の型、列挙、生成物の呼び方）、`preconditions_hold`、`eval`、`doc`、`outputs_over`（段階 A で足した） | 計算した値の型と範囲（E105、E206）、とりうる値（4.1）、参照の評価、ページ |
| koyomi | `Dates::facts`、`eval`、`values`、`calendar` と `doc`（段階 A で足した） | 日付の述語の型と範囲（E206、E207）、起こりうる真偽の組（4.1）、ページ |
| 契約 | OpenAPI と AsyncAPI の文書（`ritsu_base::openapi`。8.6）、`.proto`（ritsu-proto） | `guards`、`input`、`from`（E202〜E204） |
| dandori | `Flows::crossings`（`.flow` が dandori の検査を通るか。通らなければ dandori が言うこと）、`Items`（フローが持つもの） | `workflow`（E208） |
| ritsu-cross に渡すもの | `Gates`（8.3） | X15、X16 |

sekisho が読む口は、口のまとまり `GatePorts`（`ritsu_ports`。rules・dates・books・flows・items）で受け取る。`ritsu sekisho` と `ritsu check` には `ritsu-project` の `Joined::sekisho()` が渡し（sekisho の `Suite` は `From<GatePorts>` で作る）、sekisho のクレートのバイナリは何もつながない口を持つ。`.flow` が検査を通るかを `Items` で尋ねないのは、dandori の `Items` が検査をせずに読む（規則が読めないフローでもタスクを返す）からである。`GatePorts::ports()` が、`crossings` に渡す規則と日付と帳簿の口の組を返す。

sekisho は受け取る側の言語である（dandori、yuen、sakai と同じ）。sekisho のクレートのバイナリはほかの言語を持たないので、`use` のあるファイルは、`ritsu sekisho` で走らせるよう言う診断（E209。dandori の E018 と同じ形で exit 2）を出す。`ritsu-project` の読む順（ritsu の DESIGN 6.1 の `ORDER`）では、rulec、koyomi、chobo、geas、`.proto`、dandori のあと、yuen と sakai の前に置く。

### 8.2 出すもの

- **`Gates`**（新しい口。8.3）：action と守る操作、ワークフロー、ポリシーと期待の一覧、ある principal がある action をどれだけ許されるか。答えるのは sekisho の `Engine` で、`.gate` と、`.cedar` と `.cedarschema` の組（1.3）の両方に答える。
- **`Items`**：種類 `principal`（下に `attribute`）、`resource`（下に `attribute`）、`role`、`workflow`、`enum`（下に `value`）、`action`（下に `input`、`context`）、`policy`、`expect`、`separate`。定義の文は、宣言の塊の行（コメントと前後の空白を除き、続いた空白を一つにし、字下げを深さごとに空白二つに直したもの。dandori と同じ。ritsu の DESIGN 3.2）。
- **`References`**：`use rule|dates|calendar|openapi|proto|asyncapi|book|gate` の先、`workflow … from` の先、`guards` の先（操作）。sakai が境界を越える参照として確かめ、yuen が `trace` でたどる。

### 8.3 ritsu-ports に足す型

```rust
// ritsu-ports/src/gates.rs（新しいファイル）。sekisho が答え、ritsu-cross が受け取る
/// What sekisho knows of one `.gate` that passes its check, or of a Cedar policy set with its schema.
#[derive(Clone, Debug, PartialEq)]
pub struct GateFacts {
    /// The file's name and alias (`gate 返金(refunds_ja)`), its version and SHA-256, and the Cedar namespace.
    pub name: String,
    pub alias: String,
    pub version: String,
    pub sha256: String,
    pub namespace: String,
    pub actions: Vec<GateAction>,
    pub workflows: Vec<GateWorkflow>,
    /// Each policy: its name, its `@id`, permit or forbid, and its line.
    pub policies: Vec<GatePolicy>,
    /// Each expectation and separation, by name, with its line.
    pub expects: Vec<(String, usize)>,
    pub separations: Vec<(String, usize)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GateAction {
    pub name: String,
    pub alias: String,
    pub line: usize,
    /// The operations the action guards, as references (`openapi "api/orders.json" operation refundOrder`,
    /// `proto "shop/v1/orders.proto" service Orders method Refund`), each with its line.
    pub guards: Vec<(ritsu_base::naming::Name, usize)>,
    /// The principal and resource types the action takes, by alias (`User`, `Workflow`).
    pub principals: Vec<String>,
    pub resources: Vec<String>,
    /// `nobody "<reason>"`: the action is meant to be allowed to no one.
    pub nobody: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GateWorkflow {
    /// The name the gate gives it (the Cedar id of `Workflow`), and the `.flow`, as the caller reaches it.
    pub name: String,
    pub flow: std::path::PathBuf,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GatePolicy {
    pub name: String,
    pub id: String,
    pub permit: bool,
    pub line: usize,
}

/// Who asks, for `Gates::allowed`.
#[derive(Clone, Debug, PartialEq)]
pub enum Asker {
    Workflow(String),
    /// A principal of this type holding exactly these roles (and the roles they include).
    Roles { ty: String, roles: Vec<String> },
}

/// How far an asker is allowed an action, over every combination of the rest.
#[derive(Clone, Debug, PartialEq)]
pub enum Allowance {
    Always,
    /// Allowed in some combinations and denied in others: one of each, in both languages.
    Sometimes { allowed: ritsu_base::text::Text, denied: ritsu_base::text::Text },
    Never,
}

pub trait Gates {
    fn facts(&self, file: &std::path::Path) -> Result<GateFacts, Vec<crate::Said>>;
    /// Whether `asker` is allowed `action`, over every combination the check walks; undecided with the
    /// reason when the walk is over budget or a language cannot say what a value can come to.
    fn allowed(&self, file: &std::path::Path, action: &str, asker: &Asker) -> Result<crate::Found<Allowance>, Vec<crate::Said>>;
    fn joined(&self) -> bool { true }
}
```

ほかの口に足す問い（どれも既定の実装を「決められない」か「無い」にし、いま口を実装しているもの（dandori の `NoRules` など）を直さずに済むようにする。ritsu の DESIGN 3.2 の E.4 と同じやり方）：

```rust
// Rules（rulec が答える）
/// Each value `output` can come to when the inputs are held to `ranges` (by name; both ends in; None
/// at an open end), with an input that reaches it, from rulec's own analysis; undecided with the
/// reason when a value the analysis cannot rule out has no input found to reach it.
fn outputs_over(&self, rule: &Path, output: &str, ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<Found<Vec<(Value, Values)>>, Vec<Said>> { … Undecided … }

// Dates（koyomi が答える）
/// What koyomi knows of a calendar file: its name and alias, the days its data covers, its offset,
/// and the days it closes within the data.
fn calendar(&self, file: &Path) -> Result<CalendarFacts, Vec<Said>>;
/// The page of `koyomi doc`, as `Rules::doc` gives rulec's.
fn doc(&self, file: &Path, shown: &str, html: bool, lang: Lang) -> Result<String, Vec<Said>>;

// Flows（dandori が答える）
/// The workflow's name, and every task call of the flow at `file` that calls an operation of a
/// contract (`http` on a `use openapi`, `connect` on a `use proto`): its line, the task, the operation as
/// a reference, and the error the task declares for a denial (403, `permission_denied`), if any.
fn operation_calls(&self, file: &Path) -> Result<(String, Vec<OperationCall>), Vec<Said>> { Ok((String::new(), Vec::new())) }

// Maps（sakai が答える。sec-design の口に足す）
/// Every operation a context of the map opens (`open host service`), with whether its contract says
/// anyone may call it (`security: []`).
fn published_operations(&self, root: &Path, map: &str) -> Result<Vec<PublishedOperation>, Vec<Said>> { Ok(Vec::new()) }
```

```rust
// 上の問いが使う型
pub struct CalendarFacts { pub name: String, pub alias: String, pub data: (Day, Day), pub offset: Option<i32>, pub closed: DaySet }
pub struct OperationCall { pub line: usize, pub task: String, pub operation: ritsu_base::naming::Name, pub denied: Option<String> }
pub struct PublishedOperation { pub context: String, pub operation: ritsu_base::naming::Name, pub open_to_anyone: bool, pub file: String, pub line: usize }
```

段階 A で作った形：

- `ritsu-ports/src/gates.rs` に、口のまとまりを足した：

```rust
pub struct GatePorts {
    pub rules: Rc<dyn Rules>,
    pub dates: Rc<dyn Dates>,
    pub books: Rc<dyn Books>,
    pub flows: Rc<dyn Flows>,
    pub items: Rc<dyn Items>,
}
impl GatePorts {
    pub fn ports(&self) -> Ports   // Flows::crossings に渡す、規則と日付と帳簿の口
}
```

- `Rules::outputs_over` の答えの値は、列挙なら `Value::Enum(公開名)`（`EnumValue::public`。`.rule` に書いた別名、無ければ名前。Cedar に入る文字列）、真偽なら `Value::Bool`、省ける出力の値が無いことは `Value::None`。添える入力は `eval` が受け取る形（列挙の値は名前）。値を宣言した順に並べ、添える入力は規則が宣言した順に並べる。区間の中に入力が無ければ空の並び。数の出力、並びを歩く規則、数でも日付でもない入力を区間に限る問いは、決められない。
- `ritsu_ports::EnumValue` に `public` を足した（`name` は規則の名前、`alias` は生成したコードの名前で、どちらも前のまま）。
- `Dates::calendar` と `Dates::doc` には既定の実装がある（答えない。`Err` で「この口はカレンダーを読みません」「この口はページを作りません」）。`Rules::outputs_over` の既定は決められない。いま口を実装しているもの（dandori の `NoRules`・`NoDates`）は直していない。
- `Flows::operation_calls` と `Maps::published_operations` は、まだ足していない（段階 D の X15 と X16 で足す）。

### 8.4 yuen：ツール名 `sekisho` と種類

参照の書き方（ritsu の DESIGN 6.2）に、ツール名 `sekisho` と、8.2 の種類を足す。例：

```req
requirement refunds_within_limit
  text "A clerk refunds no more than the clerk's own limit"
  satisfied by sekisho "refunds.gate" policy clerks_refund_within_their_limit
  verified by sekisho "refunds.gate" expect clerks_never_refund_over_their_limit
```

リンクの端は、そのポリシーと期待の定義の文で、ポリシーを書き換えれば、その端のリンクだけが止まる（yuen の DESIGN 3.2 と 4.1 の端と同じ）。手で書いた Cedar を指すツール名 `cedar`（種類 `policy`（`@id` で）、`action`、`entity`）も足す。`naming.tsv`（42 行）に、`sekisho` の行と入れ子の行、誤りの行を足す。

### 8.5 sakai

- `.gate` は、ほかの成果物と同じく、ちょうど一つのコンテキストに属する（`owns` に `sekisho "…"`）。
- `.gate` の参照（8.2）は、境界を越える参照として関係で確かめる。規則とカレンダーを読むのは、ほかの言語の成果物を読むのと同じ決まり（共有カーネル、公表された言語）。
- `guards` は、操作を守る参照である。操作の契約の文書と同じコンテキストの `.gate` だけが、その操作を守れる（段階 D で sakai と確かめる）。操作を持つサービスが、自分の認可を書くのが自然だからである。

### 8.6 `ritsu_base::cedar` との分担、OpenAPI の操作の読み手

- 土台の `ritsu_base::cedar`：ポリシーとスキーマを読む、書く（`cedar format` と同じ形）、JSON の形に変える。どの言語の意味も持たない。sekisho は、生成するときに `PolicySet` と `Schema` の値を組み立てて、`write_policies`、`policies_to_json`、`write_schema`、`schema_to_json` で書く。
- sekisho：Cedar の意味（有限の組み合わせでの評価、Cedar を読むときの有限の部分）。
- OpenAPI と AsyncAPI の操作（`operationId`、方法とパス、引数、本文のフィールドの型と範囲、`security`、レスポンスの状態）を読む部分は、いま dandori（`use openapi`、JSON だけ）と sakai（`src/contracts.rs`）が別々に持つ。sekisho が三つ目を書かないよう、土台に一つ置く（`ritsu_base::openapi`）。

`ritsu_base::openapi`（2026-10-06）：OpenAPI 3.0〜3.2 と AsyncAPI 3.0〜3.1 の文書を、土台の YAML の読み手で読み、操作の一覧にする。操作ごとに、`operationId`（AsyncAPI は `operations` のキー）、方法とパス（AsyncAPI は `send`・`receive` とチャネルのアドレス）、JSON Pointer と行と列、引数（パス・クエリ・ヘッダー・クッキー。パスの項目のものを操作のものが上書きする。3.2 の `querystring` はクエリ。AsyncAPI はアドレスの引数）、本文（JSON を先に、なければフォーム、なければ最初のメディアタイプ。AsyncAPI はメッセージの payload。`allOf` をたどったオブジェクトのプロパティ）、各値の型・`nullable`・`format`・`minimum` と `maximum`（3.0 の真偽の `exclusiveMinimum` と 3.1 の数の両方）・`enum` と `const`、`security`（操作のもの、なければ文書のもの）、レスポンスの状態のコードを読む。`$ref` は文書の中と、呼ぶ側が渡す読み手で外のファイルもたどる。たどれない `$ref` は `unresolved` に入れ、その先は読まない。`security` の無い操作は、だれでも呼べるとは読まない（`None`）。`Document::operation` が、`operationId` か `"POST /orders/{orderId}/refunds"` の形で操作を引く。dandori と sakai の読み手はまだ替えていない（出力が変わらないことを確かめる仕事が別に要る）。テストの材料の六つの文書（OpenAPI が四つで、そのうち一つは日本語の版。AsyncAPI が二つ）は、Redocly CLI 2.58.1（`redocly lint --extends minimal`）と AsyncAPI のパーサー 3.6.3 が誤りを言わず、操作の位置は Redocly が言う行と列と同じである。

## 9. 構文

### 9.1 キーワード

キーワードは英語の一種類だけで、`src/kw.rs` の一枚の表に置く（koyomi と同じ）。

| 位置 | 語 |
|---|---|
| 行頭 | `gate` `description` `namespace` `use` `today` `enum` `role` `principal` `workflow` `resource` `action` `permit` `forbid` `expect` `separate` |
| `use` | `rule` `dates` `calendar` `openapi` `proto` `asyncapi` `book` `gate` `from` |
| 塊の中 | `includes` `can` `roles` `attributes` `guards` `principal` `resource` `from` `nobody` `input` `context` `action` `actions` `when` `unless` |
| 条件 | `in` `is` `not` `and` `or` `any` `workflow` `open` `today` `allow` `deny` |
| 型と範囲 | `bool` `date` と rulec の単位の型（`money[…]`、`mass[…]` など）、`range`、`offset` |
| 記号 | `=` `:` `,` `.` `(` `)` `<` `<=` `>` `>=` `|`、`#`（行末までコメント） |

`principal` と `resource` は、行頭（宣言）と、ポリシーの塊の中（スコープ）と、式の中（`principal.suspended`）に出る。どれも同じ語で、位置で意味が決まる（Cedar と同じ）。

名前にも別名にもできない語は、キーワードのうち、条件と計算と型を書く語だけである（`kw::RESERVED`：`and`・`or`・`not`・`in`・`is`・`principal`・`resource`・`workflow`・`true`・`false`・`any`・`today`・`bool`・`date`・`number`・`rate`。E002）。名前が同じ綴りだと、条件が二通りに読めてしまう（`resource.status is not …` の `not` は、値か、`is not` か）。ほかのキーワードは、行の初めか、それだけが書ける位置にしか出ないので、名前と同じ綴りでも読み違えない。だから、列挙の値 `open`、属性 `description`、入力 `range` のように、業務の語をそのまま名前にできる。koyomi はキーワードを全部名前にできなくしたが、認可では列挙の値が業務の語（`open`・`allow`・`deny` など）であることが多いので、読み違えるものだけに絞った。

数は、単位をすぐあとに付けて書く（`50GBP`、`10_000GBP`、`100万円`、`5%`、`1.5kg`）。rulec と dandori と同じ書き方で、単位は ritsu-units の表にあるものだけである（無ければ E001）。数字のあとに日本語が続き、単位でないものは名前である（`3日以内`）。Cedar の演算子 `==`・`!=`・`&&`・`||`・`!` は sekisho の字句になく、E001 の文が sekisho の書き方（`is`、`is not`、`and`、`or`、`not`）を言う。

### 9.2 EBNF

```ebnf
file        = header, [description], [namespace], {use}, [today], {enum}, {role}, {principal}, {workflow},
              {resource}, {action}, {policy}, {expect}, {separate} ;
header      = "gate", name, version, NL ;
version     = "v", digit, {digit} ;
description = "description", string, NL ;
namespace   = "namespace", cedar_name, {"::", cedar_name}, NL ;  (* Foo か Foo::Bar。ASCII *)
use         = "use", ("rule" | "dates" | "calendar" | "openapi" | "proto" | "asyncapi" | "book"), word, "from", string, NL
            | "use", "gate", string, NL ;
today       = "today", range, "offset", utc_offset, NL ;          (* range >=2026-10-01 <=2028-10-31、offset +00:00 *)
enum        = "enum", name, "=", name, {"|", name}, NL ;

(* ブロックの中の行は、ここに書いた順に書く（違えば E003）。一度だけの行を二度書いても E003 *)
role        = "role", name, NL, [INDENT, [description], ["includes", refs, NL], ["can", refs, NL], DEDENT] ;
principal   = "principal", name, NL, [INDENT, [description], ["roles", refs, NL], [attributes], DEDENT] ;
workflow    = "workflow", name, "from", string, NL, [INDENT, description, DEDENT] ;
resource    = "resource", name, NL, [INDENT, [description], [attributes], DEDENT] ;
attributes  = "attributes", NL, INDENT, field, {field}, DEDENT ;
field       = name, ":", type, ["?"], [range], NL ;
type        = "bool" | "date" | "number" | "rate" | unit_word, "[", unit, "]" | ref ;   (* ref は列挙かエンティティの型 *)
range       = "range", bound, [bound] ;                            (* 両端が要るかは名前の検査が見る（E103） *)
bound       = (">=" | "<="), literal ;

action      = "action", name, NL, INDENT, [description], {guards}, "principal", refs, NL,
              "resource", refs, ["from", (word | string)], NL, ["nobody", string, NL], [input], [context], DEDENT ;
guards      = "guards", word, (word, [".", word] | string), NL ;  (* guards orders refundOrder、guards stock receive.do *)
input       = "input", NL, INDENT, field, {field}, DEDENT ;
context     = "context", NL, INDENT, computed, {computed}, DEDENT ;
computed    = name, "=", (rule_call | date_test), NL ;
rule_call   = word, "(", [arg, {",", arg}], ")", ".", ref ;
arg         = ref, ":", value ;
value       = ("principal" | "resource"), ".", ref | "today" | literal | ref ;
date_test   = "today", ("<" | "<=" | ">" | ">=" | "is"), date_value
            | "today", "is", "open", "in", word ;
date_value  = word, ".", ref, "(", [arg, {",", arg}], ")" | ("principal" | "resource"), ".", ref ;

policy      = ("permit" | "forbid"), name, NL, INDENT, [description], [who], what, {cond}, DEDENT ;
expect      = "expect", ("allow" | "deny"), name, NL, INDENT, [description], [who], what, {cond}, DEDENT ;
separate    = "separate", name, NL, INDENT, [description], "actions", ref, ",", ref, {",", ref}, NL, DEDENT ;
who         = "principal", ("in", refs | "is", ref | "is", "workflow", ref), NL ;
what        = "action", ("any" | refs), NL ;
cond        = ("when" | "unless"), expr, NL ;
expr        = term, {"or", term} ;
term        = factor, {"and", factor} ;
factor      = "not", factor | "(", expr, ")" | atom ;
atom        = path, "is", ["not"], (attr | "principal" | literal | ref)   (* 列挙、真偽、数、関係 *)
            | path, ("<" | "<=" | ">" | ">="), literal
            | path                                                 (* 真偽 *)
            | "principal", "in", (attr | ref)                      (* 関係、役割 *)
            | "principal", "is", ("workflow", ref | ref) ;
path        = attr | ref ;                                         (* ref だけなら input か計算した値 *)
attr        = ("principal" | "resource"), ".", ref ;               (* もう一段 `.` が続けば E104 *)

refs        = ref, {",", ref} ;
name        = word, ["(", alias, ")"] ;                             (* 日本語の名前と ASCII の別名 *)
ref         = word ;                                                (* 名前か別名 *)
literal     = ["-"], number_with_unit | date | "true" | "false" ;
```

### 9.3 例

英語の版（`examples/refunds/refunds.gate`）と日本語の版（`refunds.ja.gate`）を、例のディレクトリに並べる。日本語の版は、名前を日本語で書き、Cedar に出るものに英語の版と同じ別名を付けたもので、名前空間（`ShopJa`）とファイルの別名（`refunds_ja`）だけが違う。読む規則と日付も、日本語の版（`rules/返金の上限.rule`、`dates/返金の期限.cal`。ritsu 0.23.0 の検査を通る）を読む。カレンダーは英語の版と同じイングランドとウェールズのもの（koyomi の例の `england_and_wales.cal`）で、計算と結果は英語の版と同じになる。

例のディレクトリの形（英語が先。ritsu の DESIGN 10.10）：

```
examples/refunds/
  refunds.gate              refunds.ja.gate
  rules/refund_limit.rule   rules/返金の上限.rule
  dates/refund_terms.cal    dates/返金の期限.cal
  calendars/england_and_wales.cal, calendars/data/bank-holidays.json   （koyomi の例と同じもの）
  api/orders.json           （OpenAPI 3.1。getOrder、refundOrder、exportRefunds）
  flows/returns.flow        （dandori。ワークフロー returns が refundOrder を呼ぶ）
```

## 10. 診断

ritsu の土台の診断（`ritsu_base::diag`）と台帳（`ledger`）で書く。番号の帯は koyomi にそろえる：E0xx は字句と構文、E1xx は名前と型、E2xx はほかの言語と契約、E3xx は全部の組み合わせの検査、E4xx は生成。警告は同じ帯の W。

| コード | いつ出るか |
|---|---|
| E001 | 読めない字句（閉じていない文字列、使えない文字、文字列の外の全角の空白） |
| E002 | キーワードを名前に使った、名前の形が違う |
| E003 | 節やブロックの中の行の順が違う、一度しか書けない節や行を二度書いた、ブロックに要る行（action の `principal` と `resource`、ポリシーと期待の `action`、`separate` の `actions`）が無い、`attributes`・`input`・`context` の下に行が無い、ファイルが `gate` の行で始まらない |
| E004 | 字下げがそろっていない、タブ |
| E005 | その場所に書けない行、その語の取る形でない行 |
| E006 | 同じ名前を二度宣言した |
| E007 | Cedar に出る名前に ASCII の別名が無い、別名の形が違う |
| E008 | 別名が Cedar の予約語（`if`・`then`・`else`・`like`・`has`。`true`・`false`・`in`・`is` は E002）か、生成先の言語の予約語（TypeScript の予約語と strict モードの予約語、Python のキーワード、Go のキーワード）と同じ。型の名前が、Cedar の `Action`（スキーマに宣言できない）、組み込みの型（`Bool`・`Long`・`String`。`cedar validate` が警告する）、JSON のスキーマが型に使う語（`Boolean`・`Entity`・`Extension`・`Record`・`Set`）、sekisho が宣言する `Role`・`Workflow` と同じ（Cedar 4.13.0 で確かめた） |
| E101 | 知らない名前（役割、型、属性、列挙の値、action、`input`、計算した値、規則の出力） |
| E102 | 型が合わない（列挙を数と比べる、単位の違う数を比べる。単位は ritsu-units で比べる） |
| E103 | 範囲の誤り（`range` が無い、定数が範囲の外、整数にならない定数、±(2⁵³ − 1) を超える範囲） |
| E104 | v1 で書けない関係（二段以上たどる、型の違う二つの属性を比べる。2.5） |
| E105 | 計算した値の誤り（規則でも日付でもない、規則の出力が列挙か真偽でない、入力に書けないものを渡す） |
| E106 | `principal` のスコープが action の principal の型に無い（`principal is Customer` を Customer の来ない action に） |
| E107 | `today` を使うのに `today` の行が無い、オフセットにタイムゾーンの名前を書いた |
| E108 | 役割の `includes` が輪になる（`clerk → manager → clerk` のように、輪の役割を順に示す。輪一つにつき一度） |
| E201 | `use` のファイルが、その言語の検査を通らないか、読めない（その言語の言うことを注に）。`use rule` と `use dates` は名前の検査が口で事実を読むときに、`use calendar` と `use book` は検査がカレンダーと帳簿を読むときに、`use gate` は sekisho が読むときに出す。`use gate` が輪になっているときも |
| E202 | `guards` の操作が契約の文書に無い |
| E203 | `input` が操作の受け取るものに無いか、型か範囲が操作と合わない（dandori の E016 と同じ読み方） |
| E204 | `from` の引数が操作のパスかクエリの引数に無い |
| E205 | 二つの action が同じ操作を守る |
| E206 | 規則か日付の入力に渡す値の範囲が、その入力の範囲を外れうる、または規則の前提を破りうる（例つき） |
| W201 | 規則の前提を守るかを決められない（生成したコードが走らせたときに確かめる） |
| E207 | カレンダーのデータの範囲が、`today` の範囲か、日付の関数が返しうる日を覆わない |
| E208 | `workflow` の `.flow` が dandori の検査を通らないか、読めない |
| E209 | sekisho のクレートのバイナリはほかの言語を持たない（`ritsu sekisho …` で走らせるよう言い、exit 2。dandori の E018 と同じ形）。`use rule`・`use dates`・`use calendar`・`use book`・`workflow … from` の最初の一行で一度だけ出し、ほかの診断は出さない。契約（`use openapi`・`use proto`・`use asyncapi`）と `use gate` は、sekisho のクレートが土台の読み手で読むので出さない |
| E210 | `use gate` で読んだファイルの名前空間が、読む側と違う |
| E211 | `use gate` で読んだ二つのファイルが、同じ名前か別名の型・列挙・役割・ワークフローを宣言している（あとから読んだファイルの `use gate` の行に出る） |
| E301 | どの組み合わせでも、どの permit も action を許さない（`nobody "<理由>"` で通す） |
| E302 | permit が許そうとする組み合わせが、どれも forbid に拒まれる |
| E303 | permit か forbid の条件が、どの組み合わせでも成り立たない |
| E304 | 期待が成り立たない（成り立たない数と、一つの例） |
| E305 | 職務の分離が成り立たない（principal と、それぞれの action を許す例） |
| E306 | 役割が `can` に無い action を許される |
| E307 | 組み合わせの数が予算（既定 10⁸、`--budget`）を超えた。何も生成しない |
| W301 | permit が許す組み合わせを、ほかの一つの permit が全部許す |
| W302 | `can` に書いた action を、その役割だけでは一度も許されない |
| W303 | 決められない（多めに数えた組み合わせから出た例を、具体的な入力で起こせなかった。読んだ Cedar に有限でない式がある） |
| W304 | 期待が、どの組み合わせも選ばない（成り立つが、何も確かめていない。`principal` の行が当てはまる数と、それぞれの行が満たされる数を注に） |
| W401 | 生成したポリシーかスキーマが Verified Permissions の上限を超える（`--authorizer avp`） |

ritsu の台帳には、X15 と X16 の E907・W907・E908・W908・W909（4.6）を足す。

E006 で名前を比べるのは、同じ種類のもののあいだである。型（principal と resource）と列挙は、フィールドの型の位置でどちらも書けるので、一つの名前の集まりにする。ほかは、役割、一つの列挙の値、一つの型の属性、ワークフロー、action、一つの action の入力と計算した値（Cedar の `context` で並ぶので一つ）、ポリシー（permit と forbid。`@id` が重ならないように）、期待、職務の分離、`use` の名前、のそれぞれである。名前と別名のどちらが重なっても E006 で、`use gate` で読んだファイルの型・列挙・役割・ワークフローとも比べる。種類が違えば同じ名前でよい（例の日本語の版では、`use openapi 注文` と `resource 注文(Order)` が並ぶ。期待 `managers_refund_in_period` は、同じ名前の permit と並ぶ）。

次は、例に forbid `clerks_do_not_refund` を足した変異（4.3）の最初の E302 である。英語の版の英語の出力と、日本語の版の日本語の出力を並べる（`tests/golden/`）。

```text
error[E302]: tests/mutants/E302_forbid_covers_permit.gate:87:1: The permit `clerks_refund_within_their_limit` allows nothing: a forbid denies every combination it would allow
    87 | permit clerks_refund_within_their_limit
  = It would allow 192 combinations; `clerks_do_not_refund` denies 192 of them, `auditors_do_not_refund` 96, `suspended_staff_do_nothing` 96, `no_second_refund` 48.
  = For example: User holding clerk (suspended: no), Order (status: paid), amount: 1GBP to 50GBP, refund_band: within_limit, in_period: yes, business_day: no; denied by `clerks_do_not_refund`.
  = Narrow the forbid, or remove the permit if no one is meant to be allowed this.
```

```text
エラー[E302]: tests/mutants/E302_forbidに覆われるpermit.gate:87:1: permit `係は上限まで返金できる` は何も許しません。許すはずの組み合わせを、どれも forbid が拒みます
    87 | permit 係は上限まで返金できる(clerks_refund_within_their_limit)
  = 許すはずの組み合わせは 192 通りで、`係は返金しない` が 192 通り、`監査は返金しない` が 96 通り、`停止中の職員は何もできない` が 96 通り、`二度は返金しない` が 48 通りを拒みます。
  = 例：係を持つ職員（停止中：いいえ）、注文（状態：支払済）、金額：1GBP〜50GBP、返金の区分：上限まで、期間内：はい、営業日：いいえ。`係は返金しない` が拒みます。
  = forbid を狭めるか、だれにも許さないつもりなら permit を消してください。
```

W304（何も選ばない期待）の、英語の版の変異の英語と日本語の出力：

```text
warning[W304]: tests/mutants/W304_expect_picks_nothing.gate:156:1: The expectation `returned_and_refunded_orders_are_not_refunded` picks no combination: it holds, and checks nothing
   156 | expect deny returned_and_refunded_orders_are_not_refunded
  = Of the 1,056 combinations of `refund_order`, the lines are met: `when resource.status is returned` on 264, `when resource.status is refunded` on 264; but never all at once.
  = Correct the lines that cannot be met together, or remove the expectation.
```

```text
警告[W304]: tests/mutants/W304_expect_picks_nothing.gate:156:1: 期待 `returned_and_refunded_orders_are_not_refunded` は、どの組み合わせも選びません。成り立ちますが、何も確かめていません
   156 | expect deny returned_and_refunded_orders_are_not_refunded
  = `refund_order` の 1,056 通りのうち、`when resource.status is returned` は 264 通り、`when resource.status is refunded` は 264 通りで満たされますが、全部が同時に満たされることはありません。
  = 同時に満たせない行を直すか、期待を消してください。
```

W304 は警告なので、終了コードは変わらず、`ok —` の行も出る。多めに数えた組み合わせだけを選ぶ期待には言わない（印のある組み合わせも、選んだものとして数える。そうした組み合わせが具体的な入力で起こせなければ、成り立たないかの検査のほうが W303 を言う）。action の無い期待にも言わない。

## 11. コマンド

```
sekisho check <file.gate>... [--format json] [--budget <n>] [--lang en|ja]
sekisho gen <file.gate>... --target cedar|typescript|python|go [--authorizer cedar|avp] [--out <dir>] [--check]
sekisho doc <file.gate> [--format html|markdown] [--lang en|ja]
sekisho vectors <file.gate> [--action <action>]          全部の組み合わせを cedar run-tests の形で
sekisho api <file.gate>                                   外のツールのための JSON（action、守る操作、役割、ポリシーの @id、期待）
sekisho explain <code> | --all [--format markdown|json]
```

- 段階 B までにできたもの：`check`、`explain`、`gen --target cedar`（5 章）、`vectors`（6.1）、`api`。`gen` の `typescript`・`python`・`go` と `--authorizer` は段階 C、`doc` は段階 D で足す。コマンドの表（`src/cli.rs`）には、できたものだけを載せる（ritsu の DESIGN 4.4。載せたフラグは必ず効く）。
- `vectors` と `api` は、検査を通るファイルを一つ取る。通らなければ、その診断を出して exit 1。`vectors --action` は action の名前か別名で、無ければ exit 2。
- `api` の JSON は、koyomi と chobo の `api` と同じく、ツールと版とファイルのことから始まる：`sekisho`（版）、`name`、`alias`、`version`、`source_sha256`、`description`、`namespace`、`uses`（`use` の行）、`today`、`cedar`（`gen --target cedar` が書く四つのファイル）、`roles`（Cedar のエンティティ、`includes`、`can`）、`types`（エンティティタイプ、持てる役割、親になれる型、属性と Cedar に出るか）、`workflows`、`actions`（エンティティ、守る操作の参照、principal と resource の型、`from`、`nobody`、`input`、context の値と、計算の式、Cedar に渡る値、省けるか）、`policies`（`@id`、効く action、書いたファイル）、`expects`、`separations`。どれも行の番号を持つ。ファイルのパスは `.gate` のディレクトリから書く（手元のパスを書かない）。
- 終了コードは 0（問題なし、警告だけ）、1（エラー）、2（使い方の誤り、読めないファイル、ほかの言語を持たないバイナリ）。
- `ritsu sekisho <command>` は、rulec、koyomi、dandori、契約の読み手をつないで走らせる。`sekisho` という名前のリンクも同じ（ritsu の DESIGN 2.3）。
- `ritsu check` は `.gate` を読む順（8.1）で確かめ、X15 と X16 を言語をまたぐ検査に足す。要約の `borders` に、X15 は操作ごと、X16 はワークフローの呼び出しごとに一つと数える。
- `ritsu gen` は 5.6 のとおり。

## 12. 八つ目の言語として足すとき

根の README と DESIGN、CLI、リリース、スキル、サイト、テスト、地図、ブラウザで試すページの直すところの要点（`git grep` で探したもの）：

- 看板の「Seven small languages」「七つの小さな言語」を「Eight」「八つ」に（README、サイト、`ritsu --help`、Homebrew と `.deb`・`.rpm` の説明、`ritsu.ctx` の説明、スキル、`marketplace.json`）。
- 言語の名前を並べたコードとテスト（`crates/ritsu/src/cli.rs` の `LANGUAGES`、`crates/ritsu/tests/` の `LANGUAGES`・`TOOLS`、`crates/xtask/src/main.rs`、`packaging/archive.sh`・`homebrew.sh`・`linux.sh`、`.github/workflows/release.yml`）。
- 拡張子と読む順（`ritsu_base::naming::Tool`、`ritsu-project` の `ORDER`）、参照の書き方のツール名（`naming.tsv`、yuen と sakai の診断の文と golden）。
- 地図（`ritsu.ctx` と `contexts/` に `Gates` のコンテキスト）、ブラウザで試すページ（`ritsu-wasm` の `ritsu_gen` と `ritsu_doc`、プロジェクトの例）。

段階 A で先に入れたもの：

- `ritsu sekisho` の入口と、`ritsu check` が `.gate` を sekisho に渡すこと（`ritsu --help` の言語の一覧には、まだ出さない）。
- 読む順（`ritsu-project` の `ORDER`）の sekisho の位置（dandori のあと、yuen の前）。
- 地図の `Gates` のコンテキストと、`ritsu-project` と `ritsu` からの関係。
- `ritsu_base::naming::Tool::Sekisho`（拡張子と種類だけで、`Tool::ALL` には入れていない。参照の書き方のツール名には、まだならない）。

拡張子を並べる `ritsu-project` の二つの文には、まだ `.gate` が無い。いまの `ritsu.wasm` が同じ文を返すので、ブラウザで試すページを作り直すときに足す。

## 13. 先にあるもの（2026-10 の時点）

### 13.1 Cedar

- **言語と実装**：Cedar の言語の版は 4.5（<https://docs.cedarpolicy.com/>、`cedar language-version`）。Rust の `cedar-policy` と CLI の `cedar-policy-cli` は 4.13.0（2026-09-15。<https://github.com/cedar-policy/cedar/releases>）。4.12.0（2026-07-28）で protobuf の形のスキーマの対応を広げ（action の型を属性の型に使える）、4.13.0 で `has` の連鎖の JSON の形を変え、`InvalidActionApplication` をエラーから警告にした。CLI のコマンドは `authorize`・`evaluate`・`validate`・`check-parse`・`link`・`format`・`translate-policy`・`translate-schema`・`visualize`・`new`・`partially-authorize`・`tpe`・`run-tests`・`symcc`・`language-version`（4.13.0 の `--help`）。
- **型**：Bool、String、Long（64 ビット。オーバーフローはエラー）、Set、Record、エンティティ、拡張の `decimal`（小数 4 桁まで）、`ipaddr`、`datetime` と `duration`（ミリ秒。`offset`・`durationSince`・`toDate`・`toTime`・`toDays` など）（<https://docs.cedarpolicy.com/policies/syntax-datatypes.html>、<https://docs.cedarpolicy.com/policies/syntax-operators.html>）。浮動小数点は無い。識別子は `[_a-zA-Z][_a-zA-Z0-9]*`（4.13.0 の `grammar.lalrpop`）。
- **スキーマ**：名前空間、`entity … in […] { … } tags …`、列挙のエンティティ型（`entity Group enum ["G1", …]`）、`action … in […] appliesTo { principal, resource, context }`、共通の型、どの宣言にも注釈（<https://docs.cedarpolicy.com/schema/human-readable-schema.html>）。属性と context の属性の注釈も読めることを 4.13.0 で確かめた（3.7）。
- **検証**：既定は strict。検証を通ったポリシーは、スキーマに合うリクエストでは、ほとんどのエラーを起こさない（オーバーフロー、無いエンティティ、strict でない拡張の値を除く）。検証の健全性は Lean で証明されている（<https://docs.cedarpolicy.com/policies/validation.html>）。
- **Lean の形式化**：<https://github.com/cedar-policy/cedar-spec> の `cedar-lean`（Lean 4 の v4.34.1、2026-10-02 のコミット）。`Cedar/Thm/Authorization.lean`（forbid が当たれば拒む、明示の permit が無ければ許さない、ポリシーの順や重なりに依らない）、`Typechecking.lean`、`Validation/Levels.lean`、`Slicing.lean`、`SymbolicCompilation.lean`（記号的なコンパイルの健全性と完全性）、`Verification.lean`。Rust の実装とは、差分のランダムテスト（`cedar-drt`）で突き合わせている。
- **記号的な解析**：`cedar-policy-symcc` 0.7.0（2026-09-15）が Cedar を SMT-LIB に訳し、cvc5 で解く。Cedar Analysis（2025-06-16 に公開。<https://aws.amazon.com/blogs/opensource/introducing-cedar-analysis-open-source-tools-for-verifying-authorization-policies>）は、ポリシーの集合の比較（同じ、より広い、より狭い、比べられない）と、Shadowed Permits、Impossible Conditions、Forbid Overrides、Complete Denials を見つける。SymCert（Lean で SMT による解析を検証する枠組み。<https://www.amazon.science/publications/symcert-verifying-smt-based-policy-analyses>）。
- **Amazon Verified Permissions**：Cedar 4 に上げた（<https://docs.aws.amazon.com/verifiedpermissions/latest/userguide/cedar4-faq.html>）。`IsAuthorized` は `entities` と `context` を `cedarJson` で受け取り、`determiningPolicies` に `policyId` を返す（<https://docs.aws.amazon.com/verifiedpermissions/latest/apireference/API_IsAuthorized.html>）。`CreatePolicy` に名前 `name`（`[a-zA-Z0-9-/_]*`、150 字まで。<https://docs.aws.amazon.com/verifiedpermissions/latest/apireference/API_CreatePolicy.html>）。クォータは 5.8（<https://docs.aws.amazon.com/verifiedpermissions/latest/userguide/quotas.html>）。
- **実装**：cedar-wasm 4.13.0（npm、2026-09-15）、cedar-go v1.8.0（2026-06-01。検証器は実験、テンプレートと部分評価と CLI が無い。<https://github.com/cedar-policy/cedar-go>）、cedarpy 4.12.1（PyPI、2026-09-24。k9securityio、AWS の公式ではない。<https://github.com/k9securityio/cedar-py>）。

### 13.2 ALFA → XACML（いちばん近い先例）

ALFA（Abbreviated Language For Authorization）は、Axiomatics が作った、XACML 3.0 を生成する小さな言語で、認可を書く言語が別のエンジンの形式を生成する、いちばん近い先例である。ALFA 2.0 は IETF のインターネットドラフト（draft-brossard-alfa-authz-00、2024-07-22。いまは期限切れ。<https://datatracker.ietf.org/doc/draft-brossard-alfa-authz/>）になり、RBAC・ABAC・ReBAC を書けると言う。XACML は OASIS の標準で、4.0 にあたる ACAL（XML と JSON の二つの表し方）の Committee Specification Draft 01 が 2026-02-18 に出た（<https://docs.oasis-open.org/xacml/acal/xacml/core/v4.0/csd01/acal-core-xml-v4.0-csd01.html>）。

sekisho が ALFA から変えたのは、結合アルゴリズム（combining algorithm）を選ばせないことである。XACML は、deny-overrides、permit-overrides、first-applicable などを、ポリシーの集合ごとに選ぶ。sekisho は Cedar の一つ（forbid が勝つ、既定で拒む）に固定する。どのポリシーが決めたかを読む人が、結合アルゴリズムを気にせずに済み、全部の組み合わせの表も一通りに決まる。

### 13.3 OPA と Rego

OPA v1.21.1（2026-09-29）。v1.0.0（2024-12-20）で Rego の v1 の書き方が既定になった。Rego は Datalog に近い汎用の言語で、データの全体（JSON）を見られる。そのぶん、とりうる入力が有限でなく、確かめはテスト（`opa test`）による。sekisho は書ける形を有限の値に絞り、全部を数える側を選んだ。

### 13.4 Dogwood

AI のエージェントとツールの呼び出しのガバナンスのための言語（<https://github.com/dogwood-policy/dogwood>。Apache-2.0。2026-07-27 に作られ、本番向けでない参照のインタプリタ）。Cedar から派生した構文に、イベントの履歴を見る時間の条件（`since`、`formerly`、集計）と、評価のときに計算する値（information providers。Rhai のスクリプト）を足し、Cedar のポリシーに変換する。変換したポリシーでは、時間の条件と計算する値は `context.*` の値になり、評価のときに Dogwood 自身が埋める。sekisho の (A) と同じ形である（3.5）。Amazon Bedrock AgentCore の Policy は、エージェントのツールの呼び出しを Cedar（と Dogwood）のポリシーで守り、自然な言葉からポリシーを作って、広すぎる、狭すぎる、成り立たない条件を自動推論で確かめる（<https://docs.aws.amazon.com/bedrock-agentcore/latest/devguide/policy.html>。2025-12 にプレビュー）。

### 13.5 OpenFGA、SpiceDB、Permify、Topaz、Oso

- **OpenFGA** v1.21.0（2026-09-20）：Zanzibar の形の ReBAC。条件は CEL で、型の付いた引数を、関係（タプル）に書いた値とリクエストの値から取る（同じ名前ならタプルの値が勝つ。<https://openfga.dev/docs/modeling/conditions>）。
- **SpiceDB** v1.56.2（2026-09-11）：Zanzibar の形のスキーマと caveat（CEL）。値が足りなければ `CONDITIONAL_PERMISSION` を返す。`zed validate` で、`assertTrue`・`assertFalse`・`assertCaveated` と、期待する関係を確かめる（<https://authzed.com/docs/spicedb/concepts/caveats>）。
- **Permify** v1.7.4（2026-09-10）：Zanzibar の形に、属性と CEL に似た規則を足したもの。FusionAuth が買い、文書は <https://fusionauth.io/permify-docs/> に移った。
- **Topaz** v0.33.22（2026-09-28）：OPA を判断の芯にし、Zanzibar の形のディレクトリを同じコンテナに持つ（<https://www.topaz.sh/docs/intro>）。
- **Oso**：オープンソースのライブラリ（Polar）は v0.27.1（2023-12-18）で非推奨になり、Oso Cloud に移った。

Zanzibar の形（OpenFGA、SpiceDB、Permify、Topaz のディレクトリ）は、関係のデータを持つストアと一緒に動く。そのストアの答え（「この人はこの文書の編集者か」）は、sekisho では真偽の計算した値として受け取れる（15 章）。

### 13.6 sekisho にしかできないこと

- **確かめた規則と日付を、許可の条件にすること。** 条件の値は rulec と koyomi が全部の入力で確かめたもので、その答えがとりうる値の上で、ポリシーを数える。
- **全部の組み合わせの表と、人が書いた期待、職務の分離、役割の届く範囲。** SMT ではなく数えて出すので、例が業務の値で出る。
- **地図の全部の操作の網羅（X15）。** sakai の地図が言う、コンテキストが開いた操作のどれにも、認可の決まりがあるか。認証（sec-design の W903）と並ぶ。
- **ワークフローの最小権限（X16）。** dandori のワークフローが実際に呼ぶ操作と、許されるものを突き合わせる。
- **要件の来歴。** yuen から、ポリシーと期待をハッシュで指し、書き換えたら止める。
- **計算した値を、呼ぶ側に書かせないリクエストの組み立て。** 生成したコードの形で守る。

## 14. 捨てたもの

1.3、2.2、2.3、2.7、2.10、3.4、3.5、5.7、6.4 にそれぞれ書いた。ほかに：

- **Cedar をそのまま書いて、sekisho は確かめるだけにする（`.gate` を作らない）**：規則と日付を条件にする書き方と、`input` と計算した値を区別する書き方が Cedar に無い。注釈で書くことはできるが、注釈は Cedar が評価しない文字列で、確かめる側が読み解く別の言語になる。
- **sekisho が自分で評価する（Dogwood の形）**：評価を Cedar に任せれば、Verified Permissions とどの言語の実装でも同じに評価され、Cedar の Lean の証明がそのまま効く（P1）。
- **時間の条件（履歴を見る `formerly` など）を入れる**：履歴は状態で、有限の値にならない。ワークフローの順序は dandori が受け持つ。
- **自由な文字列の属性**：比べる相手が無限にある。

## 15. まだやらないこと

- **本物の Verified Permissions での確かめ**：スキーマの注釈、ポリシーの `name`、`--authorizer avp` のコードを、本物のアカウントで一度確かめること（6.3）。するかは決めていない。

- **数の出力の規則**：出力の区間を rulec に尋ねれば足せる（`outputs_over` が区間を返す形）。
- **二段以上の関係、Zanzibar の形**：答えを真偽の計算した値で受け取る形（`context` に `editor = <ストアに尋ねる関数>`）が候補。そのとき値の出どころの信頼は、規則と同じく生成したコードの形で守る。
- **`sekisho diff`**：二つの版の `.gate` で、表のどの行が変わるか（rulec の `diff` と同じ考え）。Cedar Analysis の比較（より広い、より狭い）に近い。
- **Cedar のテンプレート**（`?principal`、`?resource`）：一つ一つの共有（「この文書をこの人に」）はデータで、`.gate` に書くものではない。要るなら、テンプレートを生成し、つないだポリシーはデータとして扱う。
- **エンティティのタグ、部分評価（一覧の絞り込み）**：Cedar の部分評価（CLI の `partially-authorize` と、まだ実験の `tpe`）を使えば、「見られる注文だけを返す」問いに近づける。生成するコードに足せる。
- **SymCC を使うテスト**：cvc5 があるときだけ走らせ、E302 と E303 を SymCC の答えと比べる。
- **Lean のモデル**：区間に分けて数えることが、全部のリクエストを覆うことの証明（ritsu の P7）。Cedar の意味は Cedar の Lean の形式化に任せ、sekisho の数え方だけをモデルにする。
- **LSP**：ritsu の LSP と同じく作らない（ritsu の DESIGN 15 章）。

## 16. 決めたことと、危ないところ

### 16.1 決めたこと

設計の段階で比べて選び、2026-10-06 に決めたもの：

1. **計算した値を context に入れる形（(A)）。** 3.5 の比較のとおり。
2. **生成する Cedar の部分を絞り（P2）、列挙を `String` にする（2.4）。** 列挙のエンティティ型は使わない。Verified Permissions がスキーマの注釈と列挙のエンティティ型を受け付けるかは、本物のアカウントでしか確かめられないからである。
3. **chobo の残高は条件にしない（3.4）。**
4. **関係は一段だけ（2.5）。** テナントの分離（`resource.tenant is principal.tenant`）とメンバーであることは入れ、二段以上と Zanzibar の形は入れない。
5. **言語をまたぐ検査は X15 と X16、コードは E907〜W909（4.6）。** セキュリティの検査（X14、9xx の帯）の続き。
6. **X15 の範囲（4.6）。** 公開ホストサービスの操作だけを求め、AsyncAPI のチャネルは求めない。`security: []` の操作は求めない。
7. **`guards` は、操作と同じコンテキストの `.gate` に限る（8.5）。** 段階 D で sakai と確かめる。
8. **手で書いた Cedar を指すツール名 `cedar` を足す（8.4）。**
9. **Verified Permissions に置く手順は生成しない（5.8）。** 置くためのファイルだけを書く。
10. **既定の評価の場所は `--authorizer cedar`（5.5）。** その言語の Cedar の実装を同じプロセスの中で呼び、Verified Permissions は `--authorizer avp` で選ぶ。
11. **ワークフローの principal の作り方は、v1 では書かない（2.2）。** ワークフローの資格から `Workflow::"<名前>"` を作るのは API の側の認証で、sekisho は書かない。dandori の生成するアクティビティが、どの資格で呼ぶかを決める項目も、v1 では足さない。
12. **契約が秘密と印を付けたフィールドを `input` に書けば、警告にする。** 段階 D で、セキュリティの検査の印の読み手を使って足す。
13. **名前は sekisho のまま。** npm には同じ名前のパッケージ `sekisho`（React のアプリの認証とアクセス制御。0.7.0、2026-07-01）があり、crates.io と PyPI では空いていた（2026-10-06）。看板は「Eight small languages」（「八つの小さな言語」）にする（12 章。段階 D3 で直す）。
14. **Cedar に渡す規則の列挙の値は、`.rule` に書いた別名にする（3.1）。** 例の `within_limit` で、英語と日本語の版で同じになる。生成するコードのメンバーの名前（`WithinLimit`）は使わない。rulec の口が、列挙の値ごとにこの別名も返す。

段階 A で決めたもの：

15. **名前にも別名にもできない語は、条件と計算と型を書く 16 語だけ（9.1）。**
16. **E008 の生成先の予約語は、TypeScript の予約語と strict モードの予約語、Python のキーワード、Go のキーワードにする（10 章）。** Go の predeclared、Python のソフトキーワード、TypeScript の `eval`・`arguments` は入れない。生成する側が `_` を付けてよける。
17. **`use gate` で読み合うファイルは名前空間をそろえ（E210）、読んだ二つのファイルは同じ型・列挙・役割・ワークフローを宣言しない（E211。2.10）。**
18. **役割の `includes` の輪は E108 にし、輪の役割を順に示す（10 章）。**
19. **役割の組は action ごとに、その action のポリシーと期待が読む役割と、それを `includes` する役割だけを数える（4.1）。** 例は 1,078 通りになる。職務の分離と `can` の確かめは、両方が読む役割と属性で principal を突き合わせる（4.5）。
20. **`nobody` を書いた action をだれかが許されるときは、E304 を `nobody` の行で言う（4.3）。** 新しいコードは足さず、「だれにも許さない」という期待が破れたと読む。
21. **E203 の範囲の向きは dandori の E016 と同じにする。** `input` の範囲が、操作の受け取る範囲（`minimum`・`maximum`、Protovalidate の整数の決まり）に収まること。操作が受け取る数が小数（`number`）なら E203。列挙は、操作が受け取る値が全部ゲートの列挙にあること。操作が求めない値（`required` でない本文のフィールド、クエリの引数）に `?` が無ければ E203。
22. **値が無いとき、条件は当てはまらない。`x is not v` は `not (x is v)` で、値が無いときは成り立つ（3.1）。** 生成する Cedar は `has` で守る。
23. **何も選ばない期待は W304（警告）にする（2.8、4.3、10 章）。**
24. **`.flow` が dandori の検査を通るか（E208）は、`Flows::crossings` で尋ねる（8.1）。** 口のまとまり `GatePorts` は ritsu-ports に置き、sekisho の `Suite` は `From<GatePorts>` で作る。
25. **規則の出力の値は、表と診断の例の文では `.rule` に書いた名前で見せ（7 章）、Cedar には `.rule` に書いた別名で渡す（14）。** 表は `.gate` と並べて読むもので、`.gate` には `is 上限まで` と書いてあるからである。Cedar の文字列は、ページのポリシーのところで、生成した Cedar の行として読める。
26. **名前の検査は、規則の値を、`.rule` に書いた名前、生成したコードの名前、`.rule` の別名のどれでも引く（3.2）。**

段階 B で決めたもの：

27. **スキーマのテキストは `cedar translate-schema --direction json-to-cedar` の形で書く（5.1）。** 土台の書き手（`write_schema`）がその形で書き、CLI と一字も違わないことを土台のテストが確かめているからである。名前の順に並び、`'` が `\'` になるのも CLI と同じにした。手で書いた見本（3.7）の並びとは違う。
28. **Cedar に出す属性は、ポリシーが読むもののうち、そのポリシーが当てはまりうる型のものだけにする（5.1）。** `principal is Bot` のポリシーが読む属性を、ほかの型の属性としては出さない。検証が通る最小のスキーマにし、確かめていない値をデータに増やさないためである（2.4 と同じ考え）。
29. **値が無いことがある読みは `has` で守る。守るのは、無いことがありうるときだけ（5.2）。** いつもある値を `has` で守っても検証は通るが、Cedar の側だけを読む人には、無いことがあるように見える（P6）。設計の担当の見本と、例の生成物は、頭の二行のほかは同じになった。
30. **グループのメンバーを聞くポリシーがあれば、principal の型をそのグループの型に入れる（`entity User in [Role, Team]`。5.1）。** 入れないと、Cedar の検証が「どのリクエストでも当てはまらない」と警告し、メンバーであることをエンティティの親として渡せない。
31. **計算した値は、ポリシーが読まないものも全部 context に入れ、省けるかは型と、計算に使う値と、検査の数えたものから決める（5.1）。** 生成したコードが計算した値の全部を判断の記録に残せるようにするためである（5.3）。
32. **`@guards` は、一つの注釈に参照を一行に一つ並べる（5.1）。** 帳簿の振替の操作は `chobo "<パス>" transfer <振替> operation <操作>` と書く。参照の書き方の chobo の種類に操作がまだ無いので、足すかは決めていない。
33. **`vectors` は、区間に二つ以上の値がある組み合わせを、数を全部 low の端にしたものと high の端にしたものの二件にする（6.1）。** 数ごとに端を掛け合わせると、数が増えるたびに倍になる。二件でも、どの区間のどちらの端も、どれかのテストで踏む。
34. **`vectors` のエンティティは、属性が指すもののうち、スキーマで属性を持たない型のものだけを書く（6.1）。** 属性を持つ型のものを書くには、その属性の値も決める要があり、Cedar はそれを読まない。
35. **生成物の、sekisho が書く文は `--lang` で書く（5 章）。** koyomi の `gen` と同じにした。`--check` は書いたときと同じ言語で走らせる。
36. **`--target` は省けない（5 章）。** koyomi の `gen` は省くと全部の出力先を書くが、sekisho のリクエストを組み立てるコードは、サービスの言語を一つ選んで書くものだからである。
37. **関係の項は、条件が書いた綴りではなく、属性の宣言した名前でそろえる。** 段階 A の数え方は、同じ属性を一つの条件で名前、別の条件で別名と書くと、二つの項として数え、起こりえない組み合わせ（同じ属性が principal を指し、かつ指さない）を数えていた。Cedar では一つの属性なので、生成した Cedar と食い違う（`tests/gen/two_spellings.gate` で 10 通りを数え、`run-tests` が 2 件落ちた）。そろえたあとは 4 通りで、例と変異の数は変わらない。
38. **規則の列挙の値を生成したコードの名前（`WithinLimit`）で書いた条件も、数え方と生成で同じ値を引く（3.2）。** 段階 A の数え方は、名前の検査が受け付けるこの綴りを、規則の名前と公開名の中に探して見つけられず、その条件をどの組み合わせでも成り立たないものとして数えていた（例を `WithinLimit` と書くと E303 になった）。口が返す値ごとの生成したコードの名前も持ち、それでも引く。例をそう書き換えたものは、例と同じ数になり、同じポリシーを生成する（`tests/gen.rs`）。

### 16.2 危ないところ

- **計算した値を作る者を信じること（3.6）。** 守りはコードの形による。生成したコードを使わずに、自分で Cedar のリクエストを組み立てる人には効かない。README とページで、生成したコードを通すことを強く書く。
- **Verified Permissions の版のずれ。** ストアに置いたポリシーと、生成したコードの版が合わないと、計算した値の名前が食い違う。置く手順を生成しない（5.8）ので、合わせるのは使う人である。
- **数え方の予算。** 役割が多い（`roles` が 15 を超える）と、役割の組だけで 3 万を超え、ほかの値と掛け合わせると予算（10⁸）に届きうる。超えれば E307 で止まる。`roles` を型ごとに分けることと、役割ごとの確かめ（`can`）が手当てになる。
- **rulec と koyomi に新しい問いを足すこと（8.3）。** `outputs_over` は rulec の解析を区間に限って走らせるもので、rulec の中に手を入れる仕事になる。正確でない答えを「決められない」と言えるかが、sekisho の検査の正しさを決める。段階 A で作り、rulec のコーパスとテストの規則の 323 の問いを、区間の中の全部の入力を流した答えと突き合わせた（3.2）。rulec の E102 は変えていないので、E102 を通る行が、区間に限った問いでは落ちることがある（rulec の DESIGN 15.188）。
- **Cedar の実装の差。** 四つの実装で、決めたポリシーの名前の付け方が違った（6.2）。版が上がるたびに、生成したコードのテストで突き合わせる。cedarpy は AWS の公式ではない。
- **日本語の名前と Cedar。** Cedar に出るものは全部 ASCII の別名で、Cedar の側では、日本語は注釈（`@name` と `@doc`）にしか残らない。
- **判断と操作のあいだにデータが変わること。** `Store` から読んだ属性で許したあと、操作を行うまでに属性が変わりうる（注文の状態が返金済になる）。どの認可の仕組みにもあることで、sekisho が閉じられるものではない。生成するコードの文書に、操作と同じトランザクションの中で読むか、操作の側でもう一度確かめる（返金済の注文には返金しない、など）ことを書く。
- **秘密の値を context に入れること。** `input` が、契約で秘密と印を付けたフィールド（proto の `debug_redact`、OpenAPI の `x-data-classification` など）なら、その値は Cedar のリクエストと、Verified Permissions の判断の記録に残る。そういう `input` は、段階 D で、セキュリティの検査の印の読み手を使って警告にする（16.1 の 12）。
