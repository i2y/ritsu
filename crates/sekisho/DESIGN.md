# sekisho 設計文書

認可を書く小さな言語。ファイルは `.gate`、コマンドは `sekisho`。だれ（`principal`）が、何（`resource`）に、何をして（`action`）よいかを、役割・属性・関係と、rulec の規則と koyomi の日付の答えを条件にして書く。宣言した範囲のすべての組み合わせで確かめ、Cedar のスキーマとポリシー、Cedar に渡すリクエストを組み立てるコードを生成する。評価するのは本物の Cedar である。

名前は関所（せきしょ）から取った。

この文書は設計の段階（2026-10-06）に書き、言語の芯を作った段階（A）で、作ったものに合わせて直した。設計の段階では、ritsu 0.23.0 を読み、2026-10 の時点の Cedar とまわりの道具を調べて決めた。決めたことのうち、Cedar で本当にそうなるかが分かれ目になるものは、手で書いた例を公式の Cedar の CLI 4.13.0、cedar-wasm 4.13.0、cedarpy 4.12.1、cedar-go v1.8.0 にかけて確かめた（6 章）。例の規則・日付・ワークフローは、ritsu 0.23.0 の `rulec check`・`koyomi check`・`dandori check` と `ritsu check` を通してある。例の Cedar とリクエストを組み立てるコードは、sekisho が生成する形を手で書いた見本である。`.gate` の検査の結果として載せた数は、設計の段階には、検査が数える形を Python で書いた使い捨ての試作の出力（1,090 通りなど）だった。A で全部の組み合わせの検査を作り、4 章、7 章、10 章の数と文を実物の出力に差し替えた（6.2 の表は、試作を四つの実装に流した記録なので残した）。A では、字句と構文、名前と型、診断の台帳、`check` と `explain` のコマンド、`ritsu sekisho` と `ritsu check` の入口、口のまとまり `GatePorts` と口 `Gates` の型、rulec の `outputs_over`、koyomi の `calendar` と `doc`、OpenAPI と AsyncAPI の操作の読み手、全部の組み合わせの検査と参照の評価、行を合わせた表を作った。2.8、2.10、3 章、4 章、7〜10 章をそれに合わせて直し、16 章を決めたことの形に書き直した。段階 B では、Cedar のスキーマとポリシーの生成（`gen --target cedar`）、全部の組み合わせを `cedar run-tests` のテストにする `vectors`、外のツールのための JSON の `api`、生成した Cedar を公式の CLI にかけるテスト（`tests/cedar.rs`）を作り、5 章、6.1、6.2、11 章を作ったものに合わせて直し、16.1 に段階 B で決めたことを足した。取り込みのときに、守る操作を、参照の書き方とルートからのパスで言うようにした（2.6）。段階 C では、action ごとにリクエストを組み立てて尋ねるコードの生成（`gen --target typescript|python|go` と `--authorizer`、`--module`）、組み合わせごとの生の値と、それを参照の評価にかけた答え（`src/raw.rs`）、生成したコードを本物の Cedar の実装（cedar-wasm、cedarpy、cedar-go）にかけてその答えと突き合わせるテスト、Verified Permissions の上限の警告（W401）、`ritsu gen` のパッケージへの取り込みを作り、2.9、3.8、5 章、6.3、11 章を直し、16.1 に段階 C で決めたことを足した。段階 D では、人が読むページ（`doc`）を作り、7 章と 11 章を作ったものに合わせて直し、16.1 に決めたことを足した。段階 D の最後（D3）で、八つ目の言語として ritsu の看板、README、CLI、リリース、スキル、サイト、地図、ブラウザで試すページに入れ、言語の文書（`docs/reference.md`）、README、スキル（`skills/sekisho`）を書き、12 章を直し、16.1 に決めたことを足した。2026-10-07 に、規則に渡す列挙と真偽を、組み合わせの値と定数に限って rulec に尋ねるようにし、3.2、4.1、16 章を直した。同じ日に、ワークフローが帳簿の振替を直接呼ぶ形を X16 が見ないことと、その理由を 3.4 と 15 章に書き、ブラウザで試すページの通販に倉庫の契約とゲートを置いた形を 3.4 と 4.6 に足した。2026-10-11 に、ほかの action の答えを条件にする `allowed` を足して 2.11 を書き、3.1、3.6、3.8、4.1、5.1、5.3、6.3、7 章、9〜11 章、15 章、16 章を直した。

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
- コマンドは `check`・`gen`・`doc`・`api`・`vectors`・`diff`・`explain`。終了コードは 0、1、2（ritsu の DESIGN 4.4）。診断の形と台帳は ritsu の土台のもの。
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

**段階 D で作った形（2026-10-06）**：`src/cedar_in.rs`。

- 組は、ポリシーの `<名前>.cedar` と、その横のスキーマ `<名前>.cedarschema`（無ければ `<名前>.cedarschema.json`）である。口は、組のどちらのファイルを渡されても、同じ組に答える。スキーマが無ければ答えない（「There is no schema (refunds.cedarschema or refunds.cedarschema.json) beside refunds.cedar」）。ポリシーの無いスキーマは、ポリシーが一つも無い組として答える（どの action も、だれにも許さない）。
- 読むのは、スキーマの action と、その `@guards`（一行に一つの参照で、ルートからのパス。参照として読めない行があれば、その行を言って答えない）、`appliesTo` の型、ポリシーの `@id`（無ければ ritsu-base の読み手が付ける `policy0`、`policy1`）と permit か forbid かである。名前空間は、action を宣言した最初のもの。組の SHA-256 は、ポリシーとスキーマのテキストを続けたものから取る。
- 許すかを数えるのは、`.gate` と同じく、action のポリシーが読む値の全部の組み合わせである。有限の部分は、スコープ（`==`、`in`、`is`）、principal が入る役割とグループ（`principal in Role::"clerk"`）、principal・resource・context の属性（スキーマで宣言した一段だけ）を文字列・真偽・整数の定数と比べる条件、文字列どうしと真偽どうしの二つの属性の比べ（`principal.department == resource.department`。`.gate` で二つの列挙の属性を比べる式は、生成した Cedar でこの形になる）、`has`、principal であるエンティティの属性（`resource.owner == principal`）とそうした属性どうしの一致、エンティティの属性への `principal in`、resource がエンティティに入るか（`resource in Folder::"archive"`）、`&&`・`||`・`!`・`if … then … else` である。どの型も宣言しない属性を読む式は、どの組み合わせでもエラーになり、ポリシーが当てはまらないものとして数える。それより外のもの（算術、`like`、集合とそのメソッド、拡張の関数、属性の属性、二つの整数の属性の比べ、テンプレートの穴、ID を並べたエンティティ型）に届く問いは、決められないと答え、どのポリシーの何かを言う（「the like at line 19, column 8 is outside the finite part sekisho counts, in the policy drafts_are_not_printed」「ポリシー drafts_are_not_printed の、19 行 8 列の like は、sekisho が数える有限の部分の外です」）。比べる式の中に算術などがあれば、比べる式ではなく、中のその部分を挙げる（`resource.pages * context.copies <= principal.limit` なら `*`）。同じ組のほかの action の問いには答える。組み合わせが 10⁶ を超えても、決められないと答える。
- 数えるのは、リクエストとエンティティで作れる組み合わせだけである。どの型のエンティティがどの型に入れるか（`in [...]` を、親の親までたどる）、型ごとにどの属性をどの型で持ち、どれを省けるかは、スキーマが決める。だから、principal はその型が入れない役割やグループには入っておらず、resource もその型が入れないエンティティには入っておらず（テンプレートはフォルダに入らない）、型の違う二つの項は同じエンティティにならない。resource はそれ自身に入っている（resource がアーカイブのフォルダなら、`resource in Folder::"archive"` が成り立つ）。エンティティのリテラルは、型と ID で比べる（書いた位置では比べない）。型の名前は、名前空間まで含めて比べる。属性と型の違う定数は、その属性と等しくならない。省ける整数の属性は、どのポリシーもそれを定数と比べなくても、無いときと有るときの両方を数える。ある resource の型では要り、ほかの型では省ける属性は、省ける型では無いときも数える。
- 役割は、principal が入るエンティティである。だれがどこに入るかは、Cedar ではエンティティのデータが持ち、ポリシーとスキーマには無い。そこで、尋ねる側が並べた役割を、スキーマがその型を入れられるものに限って数え、役割の中の役割はたどらない（`.gate` の `includes` は `.gate` に書いてあるが、手で書いた Cedar では、尋ねる側が principal の入る役割を全部並べる）。
- 2026-10-07 に、この数え方を、組み合わせごとに公式の CLI にかけて確かめた（6.5）。そのとき、前の数え方の誤りを直した。前は、スキーマが許さない組み合わせを数えていた（その型が入れない役割を持つ Customer やワークフロー、文書の持ち主やチームのメンバーであるボット、フォルダに入ったテンプレート）。同じエンティティを、書いた位置の違いで二つと数えていた。省ける整数を、定数と比べないときは無いときだけ数え、ある型では要りほかの型では省ける属性を、省けないと数えていた。型の違う定数と等しくなりうると数え、型の名前を名前空間を見ずに比べていた。どれも、口の答え（いつも・ときどき・だれも）を誤ることがあった。二つの属性の比べは、前は決められないと答えていた。
- `ritsu check` が読む組は、地図の `owns` に `cedar "…"` と書いたものと、要件が `cedar "…"` で指すものだけである。ルートの下の `.cedar` を探しはしない。地図にも要件にも無い Cedar は、どのコンテキストのものでも、どの要件のものでもなく、確かめる相手が無いからである（sakai の DESIGN 17 章）。
- ワークフローは読まない。`Workflow` のエンティティと `.flow` の結び付きは Cedar には書けないので、X16 は `.gate` だけを見る（15 章）。
- 材料は `tests/cedar_in/` で、三組ある。一つは、例の `refunds.gate` から生成した Cedar を、手で保つ人が置くように置いたもの（頭の二行を除き、コメントを足した）。二つ目は、有限の部分の外の式（`like` と算術）を持つ図書館のもの（`outside.*`）。三つ目は、会社の文書とフォルダとテンプレート、職員と、インデックスを作るボットのもの（`documents.*`。2026-10-07）で、sekisho が数える条件の形を全部使い、生成する Cedar には出ない形（二つの文字列の比べ、二つの真偽の比べ、`if`、入れ子のフォルダに入った resource、二つのポリシーに書いた同じエンティティ）と、スキーマがグループやチームに入れない型の principal を持つ。どれも英語と日本語の版があり（`outside` と `documents` の日本語の版は、名前空間と `@id` が英語の版と同じで、コメントと `@doc` と `@name` が日本語）、公式の CLI 4.13.0 の `cedar validate`（strict、警告も誤り）が何も言わない。`tests/cedar_in.rs` は、生成した Cedar のコピーが `.gate` と同じ action・守る操作・型・ポリシーを持つこと、七通りの尋ねる側と三つの action のどれでも `.gate` と同じ答えを返すこと、外の式を読む action の問いだけが決められず、理由が英語と日本語で言えること、読めない組が理由を言うことを確かめる。スキーマが許さない組み合わせを数えないことは、小さな十二の組（ボットは文書の持ち主にならない、Customer は役割に入らない、テンプレートはフォルダに入らない、アーカイブはそれ自身に入る、二つのポリシーに書いた indexer は一つ、action のグループへのポリシーはグループの action に当たる、など）の答えで確かめる。組み合わせの一つ一つは、公式の CLI で確かめる（6.5）。

## 2. モデル

### 2.1 ファイルの形

`.gate` は一つの種類だけで、節の順は決まっている。見出し、`description`、`namespace`、`use`（いくつでも）、`today`、`import enum` と `enum`（混ぜてよい。25 章）、`role`、`principal`、`workflow`、`resource`、`action`、`permit` と `forbid`（混ぜてよい）、`expect`、`separate` の順である（違えば E003）。`description`・`namespace`・`today` は一度まで。字下げは空白で、そろっていなければ E004（koyomi と同じ）。`#` から行末まではコメント。

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
- **ワークフロー**は `workflow <名前> from "<.flow>"` で書く。Cedar では型 `Workflow` のエンティティ（ID は名前）になる。`Workflow` は sekisho が知っている型で、`workflow` の行が一つでもあれば生成する。ワークフローは、自分の資格（Temporal のワーカーの資格、mTLS の証明書など）で API を呼ぶ。資格から principal を作るのは API の側の認証で、sekisho はその後ろを受け持つ。`from` の `.flow` は dandori の検査を通ること（E208）。X16 が、その `.flow` が呼ぶ操作と、このワークフローが許されるものを突き合わせる（4.6）。`description` の下には、フローが resource について持つ値を書ける（`resource <型> from <フローの値>` とその下の行。2.12）。

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

- `guards <API> <操作>`：action が守る契約の操作。`<API>` は `use openapi|proto|asyncapi <名前> from "…"` の名前、`<操作>` は OpenAPI の `operationId`（無ければ `"POST /orders/{orderId}/refunds"`）、proto の `"Service/Method"`（dandori の `connect` と同じ書き方）、AsyncAPI の操作のキー。chobo の振替の操作は `guards <帳簿> <振替>.<操作>` で書ける（3.4）。一つの action が二つ以上の操作を守ってよい（同じ操作を REST と gRPC で出すとき）。一つの操作を二つの action が守ればエラーにする（E205。どちらの判断で守るかが決まらない）。
- `principal` と `resource` の行は、その action のリクエストに来うる型を並べる。Cedar のスキーマの `appliesTo` になる。
- `resource Order from orderId`：resource の ID を、操作のどの引数（パスの引数かクエリ）から取るか。生成するコードは、その引数で resource をサービスのデータから読む。書かなければ、生成するコードは resource の ID を引数に取る。
- `input`：操作の引数のうち、ポリシーか計算した値が読むもの。名前は操作の引数（OpenAPI ならパスとクエリの引数か本文のフィールド、proto ならリクエストのメッセージのフィールド）と同じで、型と範囲は操作が受け取るものと比べる（E203。dandori の E016 と同じ読み方。例のワークフローも同じ範囲を書いている）。
- `context`：計算した値（3 章）。名前は Cedar の context の属性の名前になる。

**操作と結び付けない action**も書ける（`guards` の無い action）。そのときは、ページと `api` に「どの操作も守っていない」と出る。バッチの処理のように、契約の無い入口を守るためである。

**守る操作の参照。** 診断の文（E202〜E205）、`api` の JSON、生成するスキーマの `@guards`、リクエストを組み立てるコードのコメント（5.3）、口 `Gates` の `GateAction::guards` では、守る操作を参照の書き方（ritsu の DESIGN 6.2）で書く。

- OpenAPI：`openapi "api/orders.json" operation refundOrder`。名前は `operationId` で、無い操作だけ方法とパス（`operation "POST /orders/{orderId}/refunds"`）。`operationId` のある操作は、`guards` の行が方法とパスで書いていても `operationId` で書く（参照の名前の書き方を一つにする。ritsu の DESIGN 6.5）。
- AsyncAPI：`asyncapi "<パス>" operation <操作のキー>`。
- proto：`proto "<パス>" service <サービス> method <メソッド>`。サービスは、そのファイルの package から見た名前（`guards` の行が `shop.v1.Orders/Refund` と書いても `service Orders`）。
- 帳簿：`chobo "<パス>" transfer <振替> operation <操作>`（`guards stock receive.do` なら `transfer receive operation do`）。

参照は、契約の検査（`src/contracts.rs`）が操作を見つけたときに `ritsu_base::naming::Name` で組み、モデル（`Guard::reference`）に置く。`@guards`、`api`、生成するコード、口は、どれも `Action::references` の同じ値を書き、文字列を組まない。

**パスとルート。** 参照のパスは、`use` の行に書いたパスではなく、ルートからのパスである（ritsu の DESIGN 6.2 の 3）。ルートは yuen と sakai と同じく、`--root` があればそれ、無ければ最初に渡したパスの上で `.git` か `ritsu.mod` を持つ一番近いディレクトリ（ritsu の DESIGN 17.6）、どちらも無ければ、渡したファイルのあるディレクトリである。sekisho のクレートのバイナリと `ritsu sekisho` では、`check`・`gen`・`vectors`・`api` が `--root <dir>` を取り（ディレクトリでなければ exit 2）、無ければ最初に渡したファイルから探す。`ritsu check` は、プロジェクトのルート（ritsu の DESIGN 6.1）を渡す（`check::checked`）。ライブラリから呼ぶときは `check::Options::root` に渡し、`None` なら、確かめるファイルから同じ決まりで探す。`use` のパスを書いたまま参照にすると、`.gate` の置き場所によって、同じ文書の同じ操作が違う参照になり、yuen と sakai が書く参照と突き合わせられない。

`use` のファイルがルートの外にあれば `use` の行で E201、ワークフローの `.flow` がルートの外にあれば `workflow` の行で E208 にする（`--root` で、そのファイルを含むディレクトリをルートにすれば通る）。守る操作だけでなく、生成する Cedar の `@doc` に書く規則・日付・カレンダー・フロー（5.1）と、口 `References` が出す参照（8.2）も、ルートからのパスで書くからである。参照にならないパスを、黙って別の形で書かないためである。ルートの外のファイルは読まない（その言語にも尋ねない）。契約と帳簿の E201 は契約の検査（`src/contracts.rs`）が、規則・日付・カレンダー・`use gate` の E201 と `.flow` の E208 は、ほかの言語との境の検査（`src/borders.rs`）が言う。規則・日付・カレンダー・`use gate`・`.flow` をルートの外で E にしたのは段階 D（2026-10-06）で、それまでは契約と帳簿だけだった。例は、例のディレクトリをルートにして（`--root examples/refunds`）走らせる（yuen と sakai の例と同じ）。`tests/mutants/` の変異と `tests/gen/` の材料は例の文書を読むので、テストはクレートのディレクトリをルートにする（`tests/common/mod.rs` の `root_of`）。

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

`today` はリクエストの日で、生成したコードがサーバーの時計から取る。範囲（検査はこの範囲のすべての日を数える）と、日を変えるオフセットを書く。生成したコードは、`today` を読む計算した値を持つ action では、範囲の外の日を受け付けない（3.8）。`today` を読まない action は、どの日でも、ほかの決まりのとおりに答える。Cedar に渡すものが日に依らず、検査が数えた答えがそのまま当てはまるからである。koyomi と同じく、夏時間のあるタイムゾーンの名前は書けない（koyomi の DESIGN 1.9）。イングランドとウェールズのように夏時間のある地域では、日を UTC の 0 時で変えるなどと決めて `.gate` に書き、読む人に見せる（例では、夏のあいだ、現地の 0 時から 1 時までは前の日として扱われる）。`today` を使う計算した値があれば、`today` の行は要る（E107）。

### 2.10 ほかの `.gate` を読む

`use gate "people.gate"` は、そのファイルの `enum`・`role`・`principal`・`workflow`・`resource` を使えるようにする。読んだファイルの `forbid` のうち `action any` のもの（全部の action に効く forbid。停止中の人は何もできない、など）は、読んだ側の action にも効く。読んだファイルの `permit` は効かない（だれかに全部を許す permit がほかのファイルから来ると、そのファイルの検査が知らないところで許すことが増えるからである）。読んだファイルの forbid は、読んだ側の action のうち、その forbid の `principal` の行に当たる principal が来るものに効き、当たらない action には何も言わない（読んだ側の action が、その型を一つも取らないとき、E106 は出さない）。条件が読む属性と値は、効く action の上で確かめる。読んだファイルがさらに `use gate` で読むファイルの宣言も使える。読む向きが輪になれば E201 である。

`use gate` で読み合うファイルは、Cedar の名前空間をそろえる（違えば E210）。読んだ型と役割は、読む側の名前空間で使うからである。`namespace` を書かないファイルの名前空間はファイルの別名の Pascal case なので、読み合うファイルには同じ `namespace` の行を書く。また、読んだ二つのファイルが、同じ名前か別名の型・列挙・役割・ワークフローを宣言してはならない（E211）。一つの名前空間に、同じ名前のものが二つになるからである。読んだ二つのファイルが同じファイルを読むときは、そのファイルを一つと数える。

生成するとき、読んだファイルの `action any` の forbid は、読んだ側の action だけを並べたスコープで、読んだ側のポリシーとして書き出す（ID は `<読んだ側の別名>/<読んだファイルの別名>/<ポリシーの別名>`。Verified Permissions の名前に `.` は使えない）。こうすると、一つの `.gate` から生成した Cedar は、そのファイルの検査が見たポリシーとちょうど同じになる。同じ名前空間の複数のファイルを一つのポリシーストアに置いても、あるファイルの forbid がほかのファイルの action に、検査の知らないところで効くことが無い。

**捨てた案**：名前空間ごとに全部のファイルを一つにまとめて検査する。ファイルを足すたびに、ほかのファイルの表が変わり、ファイルごとの `sekisho check` が成り立たなくなる。

### 2.11 ほかの action の答え（`allowed`）

```gate
permit approvers_ask_for_what_they_cannot_approve
  description "An approver asks a higher approver to approve a submitted purchase, only when the approver could not approve it"
  principal in approver
  action request_approval
  when resource.status is submitted
  unless allowed approve
```

条件 `allowed <action>` は、同じ principal と同じ resource でその action を尋ねたら許されるか、を言う真偽である（2026-10-11）。業務では、自分で承認できないときだけ上の人に承認を求めるとき（上の例。`examples/approvals`）と、弱いほうの操作を先に使わせるとき（下の例。`examples/documents` は、アーカイブできる文書を消させない）に使う。

```gate
forbid what_can_be_archived_is_not_deleted
  description "A document the person could archive is archived, not deleted"
  action delete_document
  when allowed archive_document
```

Cedar のポリシーは、ほかの action の判断を条件に書けない。sekisho は、この答えを計算した値の一つとして扱う。生成したコードが先にその action を尋ね、答えを context に入れてから、この action を尋ねる（P5）。

- **入力**：尋ねる action の `input` には、この action の、同じ名前か同じ別名の入力を渡す。名前が違うか、この action に無いときは、`allowed approve(amount: total)` のように書く。渡せるものは、計算した値の入力と同じく、`input` の名前、`principal.<属性>`、`resource.<属性>`、定数、`today` である（3.6。計算した値は渡せない）。省ける入力（`?`）に何も渡らなければ、その入力は無いものとして尋ねる。尋ねる action の計算した値は、その action に書いたとおりに、渡した入力と、同じ principal と resource から計算する。
- **E110**：次のときに出す。省けない入力に何も渡らない。型か単位か範囲が入力に合わない値を渡す。無いことがある値を省けない入力に渡す。この action のとる型のどれかが持たない属性を渡す。この action のとる principal か resource の型を、尋ねる action がとらない（生成したコードは、この action のリクエストの principal と resource を、そのまま尋ねる action のリクエストにする）。
- **E109**：ポリシーの `allowed` が action のあいだで輪になる（自分を尋ねるのも輪）。尋ねる action の答えは、尋ねる側より先に決まっていなければならない。期待だけが読む `allowed` は輪に数えない。期待は答えを読むだけで、どの答えも変えないからである。
- **一つの action で一通り**：一つの action が同じ action を違う入力で尋ねれば、E110 である（`unless allowed approve` と `when allowed approve(amount: 100GBP)`）。Cedar の context では、`allowed_approve` は一つの値だからである。違う入力で尋ねたいときは、action を分ける。
- **Cedar に届く形**：ポリシーが読む `allowed approve` は、context の `allowed_approve`（真偽。省けない）になる（5.1）。生成したコードは、`approve` を同じ principal、resource、時刻で `approve` の関数に尋ね、答えを context に入れる（5.3）。`approve` を尋ねて拒まれたとき（リクエストを組み立てられなかったとき）は、同じ種類で、この action も拒む（3.8、P8）。期待だけが読む `allowed` は、Cedar に渡さず、生成したコードも尋ねない。
- **`today`**：尋ねる action が `today` を読めば、尋ねる側も `today` を範囲で確かめる（5.3 の 2）。範囲の外の日には、どちらも同じ `today` で拒む。
- **ワークフロー（X16）**：口 `Gates::allowed` は検査と同じ歩きで答えるので、ワークフローの permit が `allowed` を読んでも、X16 は同じ数え方で、呼び出しが許されるかを言う（4.6）。

**数え方：二つの action をいっしょに歩く。** この action の組み合わせを数えるとき、尋ねる action のポリシーが読むものも、同じ組み合わせの値として数える。principal と resource は同じなので、属性、役割、関係は、一つの値を両方の action が読む。尋ねる action の入力は、渡したもの（この action の入力、属性、定数、`today`）に置き換え、尋ねる action の計算した値は、その入力から計算した値として数える。同じ規則を同じ値で計算する二つの値は、一つの値と数える（例の `approve` の `band` と `request_approval` の `band` は、同じ金額と同じ上限で同じ規則を計算する）。`allowed approve` は、組み合わせごとに、`approve` のポリシーから Cedar と同じ読み方で決まる（forbid が一つでも当てはまれば拒み、そうでなく permit が一つでも当てはまれば許す）。だから答えは組み合わせごとに一つに決まり、組み合わせの数は増えず、多めに数えることもない。尋ねる action がさらに `allowed` を読めば、その先も同じに歩く（輪は E109 で止めてある）。

例の `request_approval` の 64 通りは、employee と approver を持つかの組 4 × 購入の状態 4 × 求めた人が principal か 2 × 金額と上限の区間と規則の答えの組 2 で、`approve` と同じ数になる。求めた人が principal かは `approve` の permit（`unless resource.requester is principal`）が、employee は期待 `only_approvers_approve_or_ask_for_approvals` が読むので数える。許すのは 6 通りで、表は次のとおりである（`tests/walk/golden/request_approval.table.md`。検査の表の形）。

| principal | employee | approver | status | resource.requester is principal | band | allowed approve | -> | policies |
|---|---|---|---|---|---|---|---|---|
| User | yes | yes | submitted | yes | within_limit | no | allow | approvers_ask_for_what_they_cannot_approve |
| User | yes | yes | submitted | any | over_limit | no | allow | approvers_ask_for_what_they_cannot_approve |
| User | yes | yes | not submitted | any | any | no | deny | (no permit) |
| User | yes | yes | submitted | no | within_limit | yes | deny | (no permit) |
| User | any | no | any | any | any | no | deny | (no permit) |

1 行目は、上限までの額でも、自分が求めた購入は自分で承認できないので、承認を求められることを言う。4 行目は、期待 `no_one_asks_for_what_they_could_approve` が選ぶ 2 通りで、拒まれる。文書の例では、`delete_document` の 16 通りのうち、`archive_document` が許す 1 通り（持ち主の、保全中でない確定した文書）を forbid が拒み、消せるのは下書きの 1 通りだけになる。

**捨てた案**：

- **この action の組み合わせごとに、尋ねる action の答えを参照の評価で決めて数える。** この action の組み合わせは、尋ねる action のポリシーが読む値（`approve` の permit が読む、求めた人が principal か）を持たないことがあり、そのとき答えは一つに決まらない。決まらない組み合わせを両方の答えで数えれば、多めに数えることになり（4.1 の印）、上の期待のように答えに頼る確かめを決められなくなる。
- **尋ねる action のポリシーを、この action のポリシーの中に式として展開する。** 尋ねる action の計算した値（`approve` の `band`）は、この action の context に無いことがあり、Cedar の側だけを見た人が、展開された式がどこから来たかを読めない（P6）。
- **尋ねる action のとらない型の principal や resource では「許されない」と読む。** その答えは Cedar に尋ねずに生成したコードが作ることになり、sekisho が Cedar の意味に足すことになる（P1）。型を合わせることを E110 にした。

### 2.12 ワークフローの下の `resource … from`：フローが持つ値と resource の属性

```gate
workflow pay from "flows/pay.flow"
  description "Pays a claim the rule leaves to the desk, and hands the others to a person"
  resource Claim from claim_id
    amount = amount
```

- **読み方**：「このフローが、`Claim` を resource に取る action の操作を、resource の ID に `claim_id` を渡して呼ぶとき、サービスが読むその `Claim` の `amount` は、そのときのフローの `amount` である」。`=` の左が resource の属性、右がフローの値で、`context` の行（左がゲートの名前、右がその出どころ）と同じ向きである。左と右は別の名前の場所なので、同じ綴りでもぶつからない。
- **`resource <型> from <フローの値>`** は、action の `resource Claim from claimId`（型と、ID を渡す引数）と同じ形である。`from` は省けない（E005）。フローが同じ型の resource を二つ扱うとき（リストを回す、別の ID を持つ）に、どちらの話かが決まらなくなるからである。この行が呼び出しに当たるのは、action が ID を引数から取り、フローがその引数に `from` と同じ値を渡しているときだけである。
- **右と `from` に書ける値**は、フローの入力、変数、そのフィールド（`order.total`）である。属性は、真偽、列挙、数、日付と、その `?` で、エンティティを指す属性は左に書けない（E102）。属性は全部書かなくてよい。書かなかった属性は、フローが値を持たないものとして、全部の値を数える。下に行の無い `resource Claim from claim_id` も書ける。
- **誤り**：その型に無い属性（E101）、同じ属性の二度書きと、同じ型と `from` の二度書き（E006）、`from` の無い行（E005）、resource でない型（E102）。フローに無い値、型か単位の食い違う値は、フローを dandori の口（`Flows::value`）で読んで E212 にする。属性の範囲の外になりうる値は、行が当たる呼び出し（`from` の値を resource の id に渡す呼び出し）ごとに、その呼び出しに届く実行が持つ値で読んで E212 にする（27 章）。注は、フローに無い値には書ける名前を、範囲の外になりうる値にはその値と、それが届く呼び出しを言う。
- **だれも確かめない。** この行は、ゲートを書く人の主張である。サービスが読む `Claim` の `amount` がフローの `amount` と同じかは、sekisho も dandori も確かめない。確かめられる形が無いからである（27.4）。だから、この行に頼った答えは、どこに出すときも、頼った行と、だれも確かめていないことを言う（X16 の E908 と W909 の注、`ritsu check --format json` の境界の `assumes`、`sekisho doc` のワークフローの節、`sekisho api` の `workflows` の `assumes`）。この行があっても、拒まれたときの手当て（タスクの `errors denied = 403` と `on denied`）は要る（4.6）。
- **Cedar は変わらない。** 生成するスキーマ、ポリシー、リクエストを組み立てるコードは、この行があっても無くても一字も変わらない（`tests/gen.rs`）。Cedar が評価するものではないからである。手で書いた Cedar の組（1.3）には、書く場所が無い。

**捨てた案**：(1) 属性の行に書く（`amount : … = pay.amount`）。ワークフローごとの話が resource の宣言に混ざり、どの `Claim` かを言えない。(2) `fed by`（26 章）を使う。`fed by` は、手書きのコードがここへ値を運ぶとき、受け取る側が全部の値を受け取れるかを比べる。呼び出しのときに二つが同じ値だという主張ではない。(3) フローの側に書く。フローに認可の語を持ち込むことになる。(4) `from` を書かず、action の `from` の引数に渡した値で結ぶ。どの resource かが決まらない。

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
| `allowed approve` | ほかの action の答え（2.11）：同じ principal と resource で `approve` を尋ねたら許されるか | `context.allowed_approve` |

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
- 型は単位の型で比べる（`Unit::same`。`JPY` と `円` は同じ、税込と税抜は違う）。渡す値の範囲は、規則の入力の範囲に収まっていなければならない。入力どうしの前提（rulec の `constraint` と `never`）は、渡す値の範囲の上で rulec に尋ねる（`Rules::preconditions_hold`。dandori の X2 と同じ問い）。破る例があれば E206、決められなければ W201（生成したコードは、走らせたときに前提を確かめ、破れば拒む。P8）。
- 規則に渡す値の範囲は、数も日付も rulec に渡す（`preconditions_hold` と `outputs_over`。日付は日の番号）。属性と `input` は宣言した範囲、定数はその一点、`today` は `today` の範囲である。渡さないと、rulec は規則が自分で宣言した範囲で答え、ゲートの範囲より広い範囲で前提を確かめることになる（起こらない値で E206 を言いうる）。段階 A では、範囲に端の無い値を規則に渡せない（E103）ので、W201 は出ない。安全網として残し、台帳の再現はまだ無い。
- **出力がとりうる値は rulec に尋ねる。** 規則の入力に渡す属性が、ほかの条件でも使われていなければ（`refund_limit` は規則にしか渡さない）、出力の値は、表の行が書く値の全部である。rulec はどの行にも当たる入力があること（E102 が無いこと）を確かめているので、行が書く値は全部起こりうる。入力に渡す値がほかの条件でも使われていれば（`amount` はポリシーが 50 ポンドと比べる）、その条件が切る区間ごとに、出力がとりうる値を rulec に尋ねる（口の問い `Rules::outputs_over`。8.3）。rulec は、入力を区間に限って規則を読み直し、出力を決める表の行のうち、ある入力が届きうる行を求める（導出と define の届く区間、`constraint`、上の表、一次のモデルのどれでも落ちない行）。その行が書く値の一つ一つに、区間の中の具体的な入力を規則のベクタから探して添え、二つがそろえば正確に答える。そろわないとき（同じ入力を読む二つの導出に、整数ではとれない組を求める行など）は、その値と行を言って決められないと答え、sekisho はその区間で出力の全部の値を数える（4.1）。rulec のコーパスとテストの規則では、数と日付の入力を小さな区間に限った 323 の問いのうち 322 が、区間の中の全部の入力を流した答えと同じに正確に答えられ、残る一つ（わざとそう作った規則）は決められないと答えた（rulec の DESIGN 15.188）。
- **一つの action が、同じデータを読む規則の出力を二つ以上持つときは、一緒にとる組を数える（4.1）。** 同じ呼び出しの二つの出力は、rulec に一緒に尋ねる（口の問い `Rules::outputs_together`。8.3）。別の規則に同じ属性を渡すとき、一つの属性を二つの入力に渡すときは、具体的な入力を規則の参照の評価器にかけて、答えの変わり目で区間を割る。
- 例の `refund_limit` は、`derive excess = amount - limit` を表で 0 と比べる（`rules/refund_limit.rule`。ritsu 0.23.0 の `rulec check` を通る）。`amount` の区間 1〜50 と 51〜10000 のどちらでも、`within_limit` と `over_limit` の両方が起こりうる（rulec の `outputs_over` の答え。どちらも正確な答えで、検査は多めに数えない）。上限を 100 ポンド以上に限ると、1〜50 は `within_limit` だけになる。
- **列挙と真偽の入力も、組み合わせの値に限って尋ねる（2026-10-07）。** 規則に渡す列挙か真偽の属性を、ポリシーの条件も読むとき（注文の状態を規則に渡し、permit も `resource.state is paid` と読む）は、組み合わせが決めたその値に、規則の入力を限って尋ねる。列挙か真偽を定数で渡すとき（`event: cancel`）も、その定数に限る。ゲートの列挙の値が規則の列挙より少ないとき（ゲートが規則の状態の一部しか持たない）は、ポリシーが読まない属性でも、ゲートの列挙の値に限る。列挙は規則が宣言した順の値の位置で、真偽は 0 と 1 で渡す（口の `outputs_over` の決まり。rulec の DESIGN 15.188）。
  - 前は、列挙と真偽を限る問いが rulec に無く、規則の入力の全部の値で尋ねて多めに数え（4.1）、規則の答えに頼る期待を W303（決められない）にしていた。ritsu のブラウザで試すページの通販のゲート（`website/playground/shop/gates/orders.gate`）は、注文の状態とイベント `cancel` を規則 `order_state` に渡す。前は取消の action を 80 通り（そのうち許すのは 17 通り）と数え、出荷した注文はだれも取り消さないという期待が W303 になった。いまは 40 通り（許すのは 5 通り）で、期待は成り立つ。
  - ゲートの列挙の値が少ないときは、前は規則の全部の値で尋ねた答えを、正確なものとして数えていた。ゲートの属性がとらない値から出る答えまで、起こりうる値として数えることになる（`tests/walk/held/held.gate` の支払いの action は、限らなければ、ゲートの持たない状態 `cancelled` から出る答えまで数えて 3 通り、限れば 2 通り）。
  - 木にある `.gate` の全部（143）と rulec のコーパスの規則の全部（87）で、直す前と後の `check`・`doc`・`vectors`・`api`・生成した Cedar（rulec は `check` と `vectors`）を比べ、どれも変わらなかった。それまで木にあったゲートには、規則に渡す列挙か真偽を、ポリシーも読むものや定数で渡すものが無かったからである。
- **規則の、順序のある列挙の入力の範囲（2026-10-18）。** rulec の規則は、順序のある列挙の入力に、値で範囲を書ける（`campaign : tier  range >=basic <=gold`、rulec の DESIGN 15.210）。範囲に入らない値は、規則の生成コードの入口が受け付けない。ritsu の規則の口は、その入力の型に、範囲に入る値の並びを付けて渡す（ritsu の DESIGN 3.2、rulec の DESIGN 15.251）ので、数の範囲と同じく E206 にする。
  - 列挙の属性か `input` は、ゲートの列挙の値のどれかが範囲に入らなければ「外になりえます」と言い、例は、ゲートの列挙の順で最初の、範囲に入らない値にする。定数は、範囲に入らなければ「外です」と言う。範囲は、範囲に入る値の最初と最後で書く（`basic..gold`）。直し方は、値がどれも範囲の中にある列挙の属性か `input` を渡すか、範囲の中の値を定数で書くことである。
  - 前は、口が列挙の名前しか渡さなかったので、範囲の外になりうる列挙の属性を渡すゲートが検査を通った。生成したコードは、その値のリクエストで、規則の生成コードの `RuleInputError` に当たる。定数の `platinum` を渡すゲートは、rulec が範囲の中に入力が無いと答えたので、どの組み合わせも許さないという E301 と E303 になり、値が規則の範囲の外だとは言わなかった。
  - `tests/walk/ranges/tiers.gate`（日本語の版は `tiers.ja.gate`）は、ゴールドで止まる列挙の属性と、定数のゴールドを渡す。検査を通り、多めに数えるものが無い。変異 `tests/mutants/E206_an_enum_outside_the_rule.gate`（日本語の版は `E206_規則の列挙の範囲の外.gate`）は、プラチナまである属性とプラチナの定数で、E206 を二つ言う。規則は、同じ範囲を持ち、真偽を表で決める `campaign_lift.rule` にした。二つの列挙の入力を比べる define（`define lifted : bool = campaign > member`）は、rulec が値の組ごとの答えを決められず（範囲が無くても同じ）、期待が W303 になるからである。
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
- 認可が受け持つのは、だれがどの振替を動かしてよいか（倉庫の人だけが入荷を記帳できる）である。これは、操作を守る action として書ける。ゲートが効くのは、帳簿を持つサービスが、振替を動かす前にゲートに尋ねるときである（生成したコードの `authorize…` で許されてから、chobo のクライアントを呼ぶ）。
- dandori のワークフローが振替を直接呼ぶ形（`book <帳簿>.<振替>.<操作>` のタスク）は、X16 が見ない（2026-10-07）。そのタスクで dandori が書くアクティビティ（Step Functions では帳簿の Lambda 関数）は、chobo のクライアントを呼ぶだけで、ゲートに尋ねない。拒まれたときにワークフローに何が返るかも決まっていない。chobo が振替を拒否する理由は、勘定の境界、仮押さえの状態、キーのぶつかり（`out_of_stock`、`expired`、`key_conflict` など）のどれかで、OpenAPI の操作の 403 や Connect の `permission_denied` に当たるものが無い。尋ねる者も、拒まれたときの答えも無いので、E908（いつも拒まれる）と W909（拒まれたときのエラーを宣言していない）は、帳簿の呼び出しには意味を持たない。そこで、dandori の口 `Flows::operation_calls` は帳簿の呼び出しを返さない（8.3）。ワークフローが帳簿を直接呼ぶ形を X16 で確かめるのは、だれがゲートに尋ね、拒まれたときにタスクに何が返るかを決めてからにする（15 章）。
- ritsu のブラウザで試すページの通販は、ワークフローが帳簿を直接呼ばない形にした（2026-10-07）。受注のワークフロー `ship_order` は、倉庫の契約（OpenAPI の文書 `warehouse/api/warehouse.json`）の三つの操作を呼ぶ。倉庫のゲート `gates/warehouse.gate` は、その三つの操作と、帳簿 `warehouse/inventory.book` の振替の五つの操作を守り、ワークフローを principal に書く。帳簿は倉庫のシステムが動かす。ゲートは、入荷と返品の記帳を入荷係に許し、注文の分の在庫を押さえることと、仮押さえの確定と取消を倉庫のシステムに許す。受注のフローが倉庫の帳簿を直接動かすより、倉庫が公開する操作を呼び、帳簿は倉庫の中で動かすほうが、コンテキストの境界として自然だからである。フローの呼び出しは X16 が確かめ（4.6）、帳簿の振替の操作を守る action は、帳簿を持つ倉庫のシステムが尋ねる。

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

1. **値の出どころを言語で限る。** 計算した値の入力に書けるのは、`principal.<属性>`、`resource.<属性>`、`input` の名前、定数、`today` だけである（E105）。`allowed` で尋ねる action の入力に渡せるものも同じである（E110。2.11）。リクエストの本文の任意の場所を指す書き方は無い。
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

生成したコードは、次のときに Cedar に尋ねずに拒み、理由を種類とともに返す（種類は括弧の中の六つ。確かめる順は 5.3）。

- principal の型がその action のとるものでない。宣言していないワークフロー。`Store` が principal を返さない。属性が宣言した範囲や列挙の外。型の持たない役割（`principal`）。
- resource の ID が無い（TypeScript と Python。Go の入力の ID は文字列の型で、いつもある）。resource の型がその action のとるものでない。`Store` が resource を返さない。属性が宣言した範囲や列挙の外（`resource`）。
- `input` が無いか、宣言した範囲や列挙の外（`input`）。
- `today` を読む計算した値を持つ action で、`today` が宣言した範囲の外（`today`）。例の `refund_order` に 2029-01-02 を渡すと、生成したコードは `today 2029-01-02 is outside 2026-10-01..2028-10-31, the range "refunds.gate" was checked over` を返して拒む。`today` を読まない action は、範囲の外の日でも、ほかの決まりのとおりに答える（2.9）。
- 規則や日付の生成物がエラーを返した（`rule`・`date`。範囲を確かめていれば起きないはずのことへの備え）。
- `allowed` で先に尋ねた action が、リクエストを組み立てられずに拒んだ（その種類のまま。2.11）。

尋ねたあとも、Cedar がエラーを言ったとき、尋ねる先に届かないとき（`--authorizer avp`）は拒み、七つ目の種類 `cedar` を返す（5.3）。

action の名前で尋ねる入口（30 章）は、action の関数を呼ぶ前に、三つのときに拒む。ゲートが宣言していない action は八つ目の種類 `action`、自分の入力（`input`）を受け取る action は `input`、action が尋ねる resource の型の ID が `resources` に無いか、二つ以上の型の ID があるときは `resource` である。

Cedar に尋ねる前に拒むのは、Cedar が、評価の途中でエラーになったポリシーを当てはまらなかったものとして扱うからである。forbid の条件がエラーになれば、その forbid は効かず、ほかの permit が許すことがある。生成するポリシーは Cedar の検証（strict）を通り、検証は、スキーマに合うリクエストではエラーが起きないことを保証する（13.1。整数のオーバーフローは別で、sekisho は算術を生成しない）。だから、エラーの元は、スキーマに合わない値を入れることだけで、それを Cedar の前で止める。突き合わせのテストは、どのリクエストでもエラーが 0 であることも確かめる（`num_errors: 0`）。

## 4. 確かめること

### 4.1 有限の組み合わせ

検査は action ごとに、その action に当たるポリシー（読んだファイルの `action any` の forbid を含む）と、その action についての期待が読むものだけを数える。ポリシーか期待が `allowed` でほかの action の答えを読むときは、その action のポリシーが読むものも、いっしょに数える（2.11）。

| 数えるもの | 値 |
|---|---|
| principal の型 | action の `principal` の行の型 |
| 役割 | その型の `roles` のうち、その action のポリシーと期待が読む役割、それを `includes` する役割、それが `includes` する役割の、どの組も（持たない組を含む） |
| 真偽、列挙の属性 | 全部の値（省ける属性は、無いことも一つの値） |
| 数の属性と `input` | ポリシーが比べる定数で範囲を切った区間。テストには区間の両端を使う（6.1） |
| 関係 | エンティティを指す項の、同じかどうかの分け方の全部と、分けた組ごとのメンバーであるか（2.5） |
| 規則の出力 | rulec が言う、とりうる値（入力がほかの条件にも使われるなら、数は区間ごと、列挙と真偽は値ごとに。3.2） |
| 日付の述語 | koyomi が言う、起こりうる真偽の組 |

読む役割が `includes` する役割も組に入れるのは、その役割だけを持つ人では `in` の答えが違うからである。`manager` が `clerk` を `includes` するとき、`principal in manager` の action は、clerk だけを持つ人を拒む——その答えは、clerk だけを持つ組を数えて初めて表れ、ベクタがその拒みを持つ。

同じデータを読む値（`amount` と `refund_band`、`today` を読む二つの述語、同じ金額から出る二つの規則の出力）は、まとめて、起こりうる組だけを数える。rulec と koyomi が正確に答えるかぎり、数えた組み合わせはどれも本当に起こりうるもので、起こりうるものは全部入る。

**規則の出力のまとまり。** 規則の出力は、次のどれかでつながるものを一つのまとまりにして数える。同じ属性か `input` を渡される。同じ呼び出しである（同じ規則に同じ値を渡し、読む出力だけが違う）。どちらも `today` を渡される。まとまりの出力が一つなら、rulec がその出力について言う値（3.2）が、そのまま数える値になる。

**二つ以上の出力は、一緒にとる組を数える。** 出力ごとの値を掛け合わせると、起こりえない組が入る。金額が 500 円以下なら `a` も `b` も low、超えれば両方 high と決める規則では、`a` が low で `b` が high の組は起こらない。500 円で切る規則と 300 円で切る規則に同じ金額を渡せば、起こる組は三つで、四つではない。数え方は次のとおり。

1. まとまりの出力を、呼び出し（規則のファイルと、渡す値）ごとに分ける。一つの呼び出しは、その出力の全部を一度に決める。
2. ポリシーが切った値の組（数は区間、列挙と真偽は値）ごとに、まとまりが読む物の**領域**を持つ。切られていない属性と `input` は宣言した範囲の全体、無いことのある属性は、有るときと無いときに分ける（無いときは、それを渡される呼び出しの出力が全部いっしょに無い）。規則の側の入力が書かなくてもよいもの（`age : number?`）でも、gate が属性を渡した入力は、値のある入力として尋ねる。gate は、その入力を書かずに規則を呼ぶことがないからである。規則が、入力を書かないときにだけ出す値（`none` の行の `unknown`）は、数えない（rulec の DESIGN 15.316）。
3. 領域で、呼び出しごとに rulec に尋ねる。出力が二つ以上の呼び出しは、一緒にとる組を尋ねる（口の問い `Rules::outputs_together`。8.3）。同じ表が書く二つの出力なら、届きうる行が書く組がそのまま答えになる。rulec が決められないとき（別々の表が決める二つの出力は、片方の表の行からもう片方の行が分からない）は、出力ごとの値（`outputs_over`）を尋ねる。
4. どの呼び出しの答えも「出力ごとの値のすべての組」のままでなく、組が二つ以上ある二つの呼び出しが、領域で幅のある同じ物を渡されていなければ、呼び出しごとの組を掛け合わせたものが、その領域で起こる組である（ほかの呼び出しの答えは、領域のどこでも同じなので）。
5. そうでなければ、領域を割って尋ね直す。割る場所は rulec に聞かず、具体的な入力で探す。領域の両端と、rulec が答えに添えた入力を、規則の参照の評価器（口の `eval`）にかけ、答えの違う二つを、渡す物を一つずつ替えてつなぎ、答えが変わった物を割る。数と日付は、二分して、答えが変わる隣り合う二つの値のあいだで割る。列挙と真偽は値ごとに割る。割った部分の組を合わせたものが、領域の組になる。
6. 一つの属性を、一つの呼び出しの二つの入力に渡すとき（`two_amounts(first: resource.amount, second: resource.amount)`）も、同じように割る。rulec には入力ごとの範囲しか渡せず、二つが同じ値だとは伝えられないからである。
7. 試した入力の答えがどれも同じで、5 の割る場所が見つからないときは、rulec が答えに添えた入力で割る。二つの金額が 500 円の同じ側にあるかを答える規則（`same_side`）に一つの金額を二度渡すと、gate が送る入力（二つが同じ値）の答えはどこでも `same` で、変わり目が無い。rulec は二つの入力を別々のものとして `apart` も答え、そうなる入力（二つが 500 円の両側にある）を添える。添えた入力が金額に与える値のうち、いちばん小さいもの以外（ここでは 501 円）から部分を始めると、1〜500 円と 501〜1,000 円に割れる。どちらの部分にもその入力は入らず、尋ね直した rulec は `same` だけを答える。組が二つ以上ある二つの呼び出しが同じ物を渡されるときも、それぞれに添えた入力がその物に与える値で同じように割る。列挙と真偽は値ごとに割る。部分の数は 5 と合わせて 64 までである。

**どこから先は決められないと言うか。** 割っても決まらない領域は、すべての組を、起こるかもしれないものとして数える。そうなるのは、答えが一つの入力だけを動かした所では変わらない規則（二つの数の合計を表が読む、など。割る場所が見つからないか、割り続けても決まらない）、一つの組の領域を 64 より多くの部分に割ることになるとき、まとまりで尋ねた回数が 4,096 を超えたとき、切られていない無いことのある属性が 6 個を超えるときである。

**多めに数えた組には印を付ける。** 印が付かないのは、rulec の答えだけで起こると言える組（どの答えも正確で、二つの呼び出しが、領域で幅のある同じ物を渡されていない）と、具体的な入力を評価器にかけてその組になった組だけである。日付と同じ物を読むまとまりと、`today` を渡される呼び出しの組には、いつも印を付ける（日付の述語との組を、いっしょには数えていないので）。rulec か koyomi が、ある区間で出力がとりうる値を決められないとき（同じ入力を読む二つの導出に、整数ではとれない組を求める行など。3.2）も、その区間では出力の全部の値を数えて印を付ける。多めに数えても、すべての組み合わせについて言う検査（期待、職務の分離）が成り立つという答えは、そのまま正しい。

ある組み合わせがあることで決まる答え（permit が何かを許す、action をだれかが許される、役割が `can` の action を許される）と、成り立たない例は、印の無い組み合わせから言う。印のある組み合わせからしか出ないときは、区間の両端と真ん中と、端の一つ内側の値（日付も同じ）、真偽と列挙の全部の値を組み合わせた具体的な入力を、規則の参照の評価器にかけて探す。見つかればその答えのまま、見つからなければ W303（決められない）にする。試す組み合わせは、最初の 8 つと、そのあとは 2 の累乗の番目に出たものを、64 まで取る（最初の 16 だけでは組み合わせの一つの隅に偏り、届く値を見逃すことがあった）。起こらない組を許すと言うこと、成り立つ期待を落とすことは、どの形でもしない。

`tests/walk/together/together.gate`（日本語の版は `together.ja.gate`）が、上の形を action ごとに持つ：同じ表が書く二つの出力（6 通り）、別々の表の二つの出力（6 通り。組は三つ）、同じ金額を渡す二つの規則（6 通り。組は三つ）、一つの金額を二つの入力に渡す規則（4 通り）、一つの金額を二つの入力に渡し、二つが同じ値なら答えの変わらない規則（2 通り。7 の形）、無いことのある金額の二つの出力（6 通り。両方有るか、両方無い）。どれも印が付かない。`sums.gate` は、二つの数の合計を二つの表が読む規則で、起きた三つの組に印が付かず、入力の見つからない一つ（合計が 100 以下で 150 を超える組）にだけ印が付く。その組を選ぶ期待は W303 になる。`tests/walk/given/given.gate`（日本語の版は `given.ja.gate`）は、書かなくてもよい入力に属性を渡す形で、いつも有る属性を渡す action は 4 通り、無いことのある属性を渡す action は 3 通り（二つの区分と、区分が無いとき）。入力を書かないときだけの区分は、どちらでも数えない。

`tests/walk.rs` は、ほかに次のことを確かめる。金額・列挙・真偽の属性を一つの呼び出しに二度渡す三つの形で、起こらない答え（`apart`）を数えず、それを求める permit が E303、選ぶ期待が W304 になること。rulec が出力の値を決められないと答える代役（評価器のあるものと無いもの）では、`apart` を多めに数え、同じ permit と期待が W303 になって、誤った断定が出ないこと。rulec の答えはそのままで評価器だけが答えない代役では、rulec が添えた入力だけで割れて同じ E303 と W304 になり、具体的な入力に頼る答えだけが W303 になること。例が言う入力（4.2。`tests/walk/golden/E304_inputs.txt`）と、規則が理由の E303 と W304 の注（一緒にならない値は `tests/walk/golden/E303_W304_apart.txt`、規則が答えない値は `E303_W304_never.txt`）。

規則に尋ねる回数にも上限を置く。比べられる数の区間の組が 4,096 を超える規則のまとまりは、区間ごとではなく宣言した範囲の全体で一度尋ね、その答えを多めに数える（出力が二つ以上でも、出力ごとの値を掛け合わせて印を付ける）。区間の組が多くても、検査の時間が rulec の解析の回数で伸びないようにするためである。

例の数：`view_order` 18 通り、`refund_order` 1,056 通り、`export_refunds` 4 通り、合わせて 1,078 通り。`refund_order` は、User では役割の組 8 × 停止中かどうか 2 × 注文の状態 4 × 額の区間と規則の答えの組 4 × 日付の述語の組 4、ワークフローでは状態 4 × 区間 2 × 組 4 である。`export_refunds` のポリシーが読む役割は auditor だけなので、役割の組は持つか持たないかの 2 通りで、停止中かどうかと合わせて 4 通りになる（設計の段階の試作は、どの action でも役割を三つとも数えて 16 通りとし、合わせて 1,090 通りとしていた）。

数える数には上限（予算）を置く。既定は koyomi と同じ 10⁸ 通りで、`--budget` で変えられる。超える action があれば E307 で止まり、何も生成しない。どの値が数を増やしているかを注に書く（役割が多いときは、`roles` を分けるか、`can` で役割ごとに確かめることを勧める）。サンプリングして続けることはしない。確かめていないのに通ったと言わないためで、koyomi の E305 と rulec と同じ考えである。

### 4.2 三つの結果

どの検査も、成り立つ（何も言わない）、成り立たない例がある（エラー。その組み合わせを示す）、決められない（W303。理由を言う）の三つのどれかである。決められない理由は、多めに数えた組み合わせから出た例を具体的な入力で起こせなかった（4.1）、読んだ Cedar に有限でない式があった（1.3）、のどちらかである。予算を超えたときは、決められないではなく、E307 で止まる（何も確かめていないので）。

**例は、数えた組み合わせをそのまま言い、確かめた入力を計算した値の横に添える。** 例の組み合わせは、ポリシーが読むもの（役割、属性、`input`、計算した値）で書く。数は、ポリシーが区別しない区間のまま言う（`amount: 1GBP to 50GBP`）。診断が数えているのは組み合わせで、例はその一つだからである。ただ、計算した値は規則が属性と `input` から出すので、値だけを言われても、読む人はその値になるリクエストを作れない。数えるとき規則の参照の評価器にかけた入力（4.1 の 5）がその組になっていれば、例は、その入力を、計算した値のあとのかっこに添える。

- 一つの入力（`refund_band: within_limit (as when refund_limit is 1GBP and amount is 1GBP)`）：規則に渡す属性と `input` の全部を、試した値で言う。ポリシーも読む属性も、かっこの中ではその一点を言う。同じ組になった入力のうち、いちばん小さいものである。
- 領域（`a: low, step: small (whenever amount is 101JPY to 300JPY)`）：rulec がその領域で値が一つに決まると答え、試した入力がその値になったとき。領域のどの入力もその値になる。言うのは、領域が組み合わせより狭めている物だけである（ポリシーが切った区間の一部、ポリシーが読まない属性の区間、無いことのある属性が無いこと）。領域は数え方が最後に持っていたもので、その値になる入力の全部とは限らない。
- かっこは、同じデータを読む規則の出力のまとまりごとに一つで、例が言うそのまとまりの最後の値のあとに置く（`a: low, step: large (as when amount is 301JPY)`）。
- 属性は名前だけで言う。action が読むほかの物（もう一方の型の属性、`input`、計算した値）と同じ名前のときは、ゲートの書き方（`principal.refund_limit`）で言う。
- かっこが無いのは、入力を評価器にかけていない組（評価器の無い規則、多めに数えた組、日付と同じ物を読むまとまり）と、組み合わせのどの入力もその値になる組（領域が組み合わせと同じ広さ）である。そのときの例は、組み合わせだけである。

日本語の出力は `返金の区分：上限まで（たとえば返金できる額が 1GBP、金額が 1GBP のとき）` と `step：small（amount が 101JPY〜300JPY のときはいつも）` である。

JSON は二つの鍵に分けて持つ。`example` は数えた組み合わせで、値の名前から値へのオブジェクトである。確かめた入力は、そのあとの `example_inputs` に、まとまりごとのオブジェクトの配列で持つ：`computed`（説明する計算した値の名前）、`kind`（一つの入力は `point`、領域は `region`）、`given`（属性と `input` ごとに、ゲートの書き方の `name`、その属性を持つ型の `type`（`input` は null）、`value`）。確かめた入力が無い診断には、`example_inputs` の鍵そのものを出さない。

ページの表と `sekisho vectors` は、数えた形だけを使う（表は組み合わせを載せ、ベクタは区間の両端を使う）。`sekisho diff` の例にも、入力は添えない。二つの版を比べる例に片方の版で試した入力を混ぜると、もう片方の版ではその入力が別の答えになりうるのに、同じ入力で比べたように読めるからである。

**捨てた案**：ポリシーも読む属性の区間を、試した一点に替えて言う（`amount: 1GBP`）。そのまま送れるリクエストにはなるが、「50 ポンドまでのどの金額でも」という広さが読めなくなる。いちばん小さい入力は端の特別な場合に見え、数えた組の値と一例の値が同じ並びに混ざって、どれが組でどれが一例かも読み分けられない。

**規則が決めているために満たせない行は、そう言う。** E303 と W304 は、行ごとの満たされる数を注に書く。行はそれぞれ満たされるのに全部が同時には満たされないとき、または一つの行の中の `and` が満たされないとき、理由が規則にあるかを確かめる。規則の出力のまとまり一つについて、読むものをそれぞれ別々に動かせるとして数え直し、それなら行が満たされるのであれば、満たされないのは規則がその組を出さないからである。そのときは、一緒にならない値と、それが何から計算されるかを注に足す（`a` is never `high` while `step` is `small`: both are computed from `resource.amount`.）。計算した値と、それが読む属性の区間が一緒にならないときも同じ形で言う（`a` is never `low` while `resource.amount` is 601JPY to 1000JPY）。直し方の行も替える。E303 は、規則が一緒に出す値に行を直すか、ポリシーを消すこと。W304 は、ポリシーをどう書いても成り立つので、規則の答えを確かめたいなら規則の側（`examples`）に書くこと。rulec の `never` の行は入力どうしの組について言うもので、二つの出力の組は書けないので、行き先は `examples` にする。

一緒にならない値が見つからないときは、計算した値を、規則の出力が宣言する値の全部で数え直す。それで行が満たされ、満たした値を持つ組み合わせが一つも無ければ、ゲートが渡す値に規則がその値を答えないことが理由である。その値と、何から計算するかを言う（`side` is never `apart`: it is computed from `resource.amount` (given as both `first` and `second`), and the rule answers `apart` for none of the values the gate gives it.）。一つの物を二つの入力に渡しているときは、その入力の名前も言う。ゲートの属性の範囲が規則の入力より狭くて出ない値（300 円までの金額に、500 円を超えたときの `high`）も、定数だけを渡す呼び出しが出さない値も、同じ文になる。直し方は、E303 は規則が出す値に行を直すかポリシーを消すこと、W304 は上と同じである。

どちらの数え直しも 10 万通りまでで、超えたら理由を足さない。行が規則と関わりなく満たせないとき（ある行と、その逆の行）は、行ごとの数の注と、前からの直し方のままである。

`use gate` で読んだファイルの forbid（`action any`）は、読んだ側の action の判断に効くが、E302・E303・W301 は、読んだ側のファイルに書いたポリシーにだけ言う。読んだファイルのポリシーの誤りは、そのファイル自身の検査が言う（読む側で言うと、ほかのファイルの行を、読む側のファイルの位置として出すことになる）。

### 4.3 検査の一覧

| 検査 | 入れるか | コード | 理由 |
|---|---|---|---|
| どの permit も許さない action | 入れる | E301。`nobody "<理由>"` を action に書けば通す。`nobody` を書いた action をだれかが許される組み合わせがあれば、E304（だれにも許さないという期待が成り立たない）を `nobody` の行で言う | だれもできない action は、たいてい書き忘れ。意図なら理由を残させる |
| forbid に全部覆われて効かない permit | 入れる | E302。覆う forbid と、permit が許そうとした組み合わせの例 | 書いた人の意図と、実際が食い違っている |
| どの組み合わせでも条件が成り立たない permit・forbid | 入れる | E303 | rulec の E102（どの入力も当たらない行）と同じ考え |
| 名前で挙げた action の一つに、どの組み合わせでも当てはまらないポリシー（ほかに挙げた action には当てはまる） | 入れる（警告） | W305。当てはまらない action、そこで `principal` の行と条件の行が満たされる数、規則が理由ならその値、ほかに挙げた action で当てはまる数 | 挙げた action に効いていないのに、行は効くように読める。`action any` は action を挙げていないので言わない。数えるのは起こりうる組み合わせ（4.2）で、起こるかを決められない組み合わせ（多めに数えたもの）も当てはまる側に数え、それでも一つも無いときだけ言う（偽の警告を出さない。2026-11-17） |
| 名前で挙げた action の一つで、許すはずの組み合わせをどれも forbid が拒む permit（ほかに挙げた action では何かを許す） | 入れる（警告） | W306。その action、許すはずの数、拒む forbid とその数、例、ほかに挙げた action で許す数 | E302 を action ごとにした形。その action では permit が無くても答えが変わらないのに、行は許すように読める。多めに数えた組み合わせも許す側に数え、それでも何も許さず、permit が当てはまる組み合わせのうち一つが起こると確かめられたときだけ言う（どれも確かめられないなら、当てはまるものが無い W305 の形かもしれず、どちらとも決められない）。`action any` は action を挙げていないので言わない（2026-11-17） |
| ほかの一つの permit で足りている permit | 入れる（警告） | W301。足りている相手の permit を言う | 消しても答えが変わらない。最小権限の見直しの手がかり。相手を一つに限るのは、二つの permit が互いを覆い合うとき、両方に「要らない」と言うと、両方を消して答えが変わるからである（試した。4.3 の最後の段落） |
| どの決定も変えない permit の行（行が外す組み合わせを、どれも forbid が拒む） | 入れる（警告） | W307。行が外す数、拒む forbid とその数、例 | W301 を行ごとにした形。行を消しても答えが変わらない。多めに数えた組み合わせも行が外す側に数え、そのどれも forbid が拒むときだけ言う。外す組み合わせをほかの permit が許す行、forbid の行、W301 が言う permit の行は言わない（下の段落）（2026-11-17） |
| 起こりうるどの組み合わせも外さない行（ほかの行が満たされるところで、いつも満たされる） | 入れる（警告） | W308。満たされる数。規則が決めていれば、どの値がなぜ来ないかと、規則の答えは規則の側（`examples`）で確かめること | 起こらない組み合わせにだけ効く行で、消しても何も変わらない。permit と forbid の両方に言う。多めに数えた組み合わせも行が外す側に数え、そう数えても外す組み合わせが無いときだけ言う。範囲の外の定数と比べる行は、E103 が先に言う（2026-11-17） |
| どの判断も変えない forbid と、その行 | 入れない（ページに出す） | — | 守りを重ねる forbid（停止中の人は何もできない）は、今の permit では何も変えなくても、置いておく意味がある。forbid の行のうち、外す組み合わせをどの permit も許さない行も、同じ理由で言わない |
| 期待 | 入れる | E304。選ぶ組み合わせが一つも無ければ W304（警告） | 人が書いた期待を、全部の組み合わせで。何も選ばない期待は、成り立つが何も確かめていない |
| 職務の分離 | 入れる | E305 | 二つの役割を持つ人が両方できることは、組み合わせを全部見ないと見落とす |
| 役割の届く範囲（`can`） | 入れる | E306（`can` に無い action を許される）、W302（`can` の action を一度も許されない） | 最小権限を、役割ごとに書いて確かめる |
| 公開する操作に action があるか（X15） | 入れる（ritsu） | 4.6 | 認証（sec-design の W903）の先の、認可 |
| ワークフローが呼ぶ操作（X16） | 入れる（ritsu） | 4.6 | ワークフローの最小権限 |

**何も変えない行と action を言う（W306〜W308、2026-11-17）。** 生成した Cedar を一か所ずつ変えた変異のうち、どの確かめにも落ちず、検査が数える組み合わせの全部で答えの変わらないもの（6.2）は、ゲートの側で見れば、消しても何も変わらない行か action である。そのうち、書いた人の思い違いでありうる三つの形を警告にした。どれも起こりうる組み合わせの数え方（4.2）で決め、起こるかを決められない組み合わせ（多めに数えたもの）は、警告を言わない側に数える（W306 では permit が許す側、W307・W308 では行が外す側）。そう数えても成り立つときだけ言うので、偽の警告は出ない。

- W306 は E302 を action ごとにした形である。permit が挙げた action の一つで、permit が当てはまる組み合わせを、どれも forbid が拒む。
- W307 は W301 を行ごとにした形である。行が外す組み合わせ（ほかの行は満たされ、その行だけが満たされないもの）を、どれも forbid が拒む。外す組み合わせをほかの permit が許す行は言わない。場合ごとに書いた permit（例の、期間内の返金と、期間のあとの営業日の返金）では、行がどの場合の permit かを言っていて、消すと読む人に場合が分からなくなる。この形まで言うと、木の `.gate` とフローのあるディレクトリの検査で、出力の変わる所が 12 から 43 に増えた（W306〜W308 の変異を足す前の木で比べた数）。増えたのは、例の `managers_refund_late_on_business_days` の `unless in_period`（例の英日と、例から作った変異）、`tests/walk/together/together.gate` の場合ごとの permit の 6 行、遊び場の店の `when resource.state is paid` である。permit ごと要らないなら W301 が言うので、W301 が言う permit の行は言わない。
- W308 は、ほかの行が満たされるところでいつも満たされる行である。起こらない組み合わせにだけ効く（ほかの行が先に外したもの、規則が出さない値）。規則が決めているときは、E303・W304 と同じく、どの値がなぜ来ないかを注に言い（`kind` is never `mixed`: it is computed from `resource.amount` (given as both `first` and `second`), …）、直し方は W304 と同じく、規則の答えは規則の側（`examples`）で確かめることにする。規則がいつか変わったときのために行を残すより、規則の答えを規則の `examples` に書くほうが、変わったことを rulec が言う。
- 同じポリシーの二つ以上の行が言われ、それを一緒に消すと答えが変わりうるとき（二つの forbid が、片方の行だけが満たされない組み合わせをそれぞれ拒み、両方が満たされない組み合わせはどちらも拒まない）は、それぞれの注でそう言い、一つずつ消して検査し直すよう言う。歩くときに、満たされない行が二つ以上あって、それを消せば何かが変わる組み合わせを、満たされない行の組として覚えておく（permit なら、forbid もほかの permit も当てはまらない組み合わせ。forbid ならどこでも）。

言わないものは四つある。forbid の行のうち、外す組み合わせをどの permit も許さない行（どの判断も変えない forbid を言わないのと同じ理由）。W301 が言う permit の行。`action any` のポリシーが当てはまらない action：`action any` は、今ある action と、あとで足す action の全部に効くよう書くもので、一部の action に当てはまらないのは書いたとおりである。生成した Cedar は `action any` を action の並びとして書くので、そこから当てはまらない action を外す変異は答えを変えないが、ゲートには外す行が無い（6.2 の等しい変異の 4）。生成した `has`：属性を省ける型や、属性を持たない型をとるポリシーで、gen が読む前に書く守りであり、ゲートの行ではない。読むどの型も属性を持つ組み合わせでは答えを変えないが、直す行が無い（等しい変異の 1）。

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
- 契約と境目の変異：操作の名前の書き違い（E202）、操作の受け取らない input（E203）、操作に無い引数からの `from`（E204）、二つの action が守る一つの操作（E205）、規則の受け取る範囲を超える属性と、規則の列挙の範囲の外になりうる属性と定数（E206）、カレンダーの知らない日まで延びる `today`（E207）、dandori の検査を通らないフロー（E208）、rulec の検査を通らない規則（E201）。

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

- 読むもの：`.gate` の `workflow` の行とその下の行（`Gates`）、dandori のフローが呼ぶ操作（口 `Flows::operation_calls`。8.3。タスクの行、呼ぶ操作の参照、タスクが宣言したエラー、渡す引数、その呼び出しに届く実行が持つ値）、sekisho の答え（`Gates::allowed_at`。呼び出しごとに尋ねる）。
- 尋ね方：sekisho は、その呼び出しに届く実行がなりうる組み合わせだけで、action を数え直す（`src/at.rs`）。フローの値は、三通りにゲートの語に読み替える。action の `input` は、呼び出しが同じ名前の引数に渡す値である（操作の引数そのものなので、何も書かなくても決まる）。resource の属性は、ワークフローの下の行（2.12）が言うフローの値である。規則の出力は、フローが同じ規則を呼んだ答えを `match` で分けていれば、その分岐が残した値である（規則に渡した値が、どれも `input` か属性か定数に読み替えられるとき。ゲートの計算した値と同じ値を渡した呼び出しは、同じ計算として数える）。読み替えられないものは絞らない。絞らなければ組み合わせが増えるだけなので、「いつも」と「決して」は正しいままである。
- 言うこと：
  - E908：届く実行がなりうる組み合わせのどれでも、action がそのワークフローを許さない（その呼び出しに来る実行は、いつも拒まれる）。答えがワークフローの下の行に頼るなら、頼った行を注に言う。
  - W909：拒まれることがあるのに、タスクが拒まれたとき（OpenAPI の操作の 403、Connect の `permission_denied`）のエラーを宣言していない。拒まれると、ワークフローは宣言していない失敗で止まる。形は四つある。組み合わせによって拒まれる（注に、届く組み合わせのうち許される例と拒まれる例）。ゲートは組み合わせによって拒むが、届く実行がどちらになるかがどこにも書いていない（答えが読む属性について、ワークフローの下の行が何も言っていない。注は、ゲートが許す例と拒む例と、行を書けば確かめられることを言う）。届く組み合わせのどれでも許されるが、それがワークフローの下の行に頼っている（行はだれも確かめていないので、サービスが読む値が違えば拒まれる）。許すかを決められない（注に理由）。
  - W908：ワークフローが許される action のうち、フローがどこでも呼ばない操作のもの（要るより多く許している）。
  - E909：呼び出しが引数に渡す数の型と、action の同じ名前の `input` の型が、違う単位か刻みか税で数える（2026-11-17。27.3）。フローは数を自分の型で送り、ゲートは同じ数を自分の型で読むので、どちらかの書き違いである。タスクが拒まれたときのエラーを宣言していても言う。その呼び出しについては、ゲートの答えのうち E908 だけを言う。
- タスクが拒まれたときのエラーを宣言していれば、「決して」のほかは通る。ワークフローの下の行は、診断を消さない。行に頼って「いつも」になっても、エラーの宣言が無ければ W909 のままである。
- 例：ワークフロー `returns` は `refund_order` を 32 通りのうち 4 通りで許され、28 通りで拒まれる。タスク `refund_order` は `errors denied = 403` を宣言しているので W909 は出ない。許されて呼ばないものは無い（試作の出力）。

**段階 D で作った形（2026-10-06）**：ritsu-cross の `src/gates.rs`。コードは ritsu の台帳（ritsu-cross の `src/codes.rs`）にあり、再現は英語と日本語の版のプロジェクトである。`crates/ritsu/tests/cross.rs` が、通るもの（手で書いた Cedar の組が操作を守るもので、組を地図が指すものと要件が指すものを含む）と五つのコードのそれぞれを、出力の golden と境界の数で確かめる。2026-10-11 に、ワークフローの permit がほかの action の答えを読む形（`x16-allowed`。返品のワークフローは、戻った注文を返金するが、返金の見直しを許される注文（異議のある注文）は返金しない）を足した。呼び出しは組み合わせによって許され、タスクが拒まれたときのエラーを宣言しているので、境界が一つ通る。

- 読むゲートは、プロジェクトの `.gate` の全部と、地図の `owns` と要件が指す Cedar の組（1.3）である。口 `Gates::facts` が答えないゲート（検査を通らない `.gate`、読めない組）は、その言語の検査が理由を言うので、X15 と X16 は黙って外す。
- X15 は、プロジェクトに `.gate` か Cedar の組が一つでもあるときだけ見る。sekisho で認可を書いていないプロジェクトでは、地図の全部のコンテキストが W907 になり、何も教えない警告で埋まるからである。見るのは、sakai の段 1 と段 2（構文と属し方）を通る地図の `published_operations`（8.3）である。守る action は、どのゲートのものも数える（ほかのコンテキストの操作を守るゲートは、sakai が E211 で言う）。答えないゲートが属するコンテキストは見ない。
- X16 が見るのは、ワークフローを `workflow … from` で書いたゲートの action が守る操作の呼び出しだけである。ほかのゲートは、そのワークフローを principal として知らないので、そのゲートが守る操作の呼び出しは言わない。
- W909 は、タスクが拒まれたときのエラー（状態 403。Connect の `permission_denied` は dandori が 403 に読む）を宣言していないときだけ出す。決められないことと、だれも確かめていない行に頼ることを黙って通さず、ワークフローが拒まれたときの手当てを求める。
- 呼ぶ操作は、dandori の `Flows::operation_calls`（8.3）が、守る操作と同じ参照で言う（`use openapi` の `http` は `operationId` で、無ければ方法とパス。`use proto` の `connect` はサービスとメソッド）。だから、`guards` の参照と一字ずつ突き合わせられる。
- 境界の数え方（ritsu の DESIGN 7 章の `borders`）：X15 は公開する操作ごとに一つで、守られている操作とだれでも呼べる操作が通り、E907 が落ち、W907 の操作は決められない。X16 は呼び出しごとに一つで、E908 と E909 が落ち、W909 は決められず、ほかは通る。W908 は呼び出しではないので数えない。
- 例（`examples/claims`）は、規則の答えで分けた分岐の中で操作を呼ぶ。フローは規則 `who_pays` の答えが `desk` の分岐で支払いを呼び、ゲートは、同じ規則を `Claim` の `amount` で呼んだ答えが `desk` のときだけワークフローに許す。ワークフローの下の行（`amount = amount`）があるので、その呼び出しに届く組み合わせはどれも許され、タスクが `errors denied = 403` を宣言しているので通る。分岐を入れ替えて `person` の分岐で呼ぶと、届く組み合わせはどれも拒まれ、E908 になる。
- 例（`examples/refunds`）は、二つの版のゲートがそれぞれワークフロー `returns` を書き、フローの返金の呼び出しは、どちらのゲートでも組み合わせによって許され、タスクが `errors denied = 403` を宣言しているので通る。`ritsu check examples/refunds` の最後の行は「borders between the languages: 2 checked, 0 undecided」になる。
- ritsu のブラウザで試すページの通販（`website/playground/shop/`。日本語の版は `shop.ja/`）は、2026-10-07 から X15 と X16 の両方を持つ。地図は、受注の公表された言語に `.proto` の `OrderService` の二つのメソッドを、倉庫の公表された言語に OpenAPI の文書 `warehouse/api/warehouse.json` の三つの操作を、公開ホストサービスとして並べ、受注のゲート `gates/orders.gate` と倉庫のゲート `gates/warehouse.gate` が五つとも守る（X15 で五つ）。倉庫のゲートはワークフロー `ship_order` を書き、フロー `ordering/ship_order.flow` は倉庫の操作を五か所で呼ぶ（`get_order` と `request_cancel` が二か所ずつ、`request_shipment` が一か所）。ゲートはワークフローに三つの action をどの組み合わせでも許すので、五か所とも通る（X16 で五つ）。最後の行は、ページが開いたとき（受注が `.proto` に値を足し、請求の規則が rulec の検査を通らない）も、契約を直したあとも「borders between the languages: 10 checked, 0 undecided」である。注文の状態でワークフローを拒まないのは、出荷と取消を受け付けるかを倉庫のシステムが規則 `order_state` で決め、受け付けないときは 409 で返すからである。ゲートが状態で拒めば、同じ呼び出しが 403 で返ることになり、フローは一つのことを二つのエラーで扱うことになる。ページの手順の一つは、ワークフローの permit から `view_order` を外す。ピッキング係には注文を見ることを許したままなのでゲートは検査を通り、X16 が `get_order` の二か所に E908 を言い、permit を指す要件のリンクに yuen が E303 を言う。

```
error[ritsu E908]: ordering/ship_order.flow:62: The task `get_order` calls openapi "warehouse/api/warehouse.json" operation getOrder, and the action `view_order` of gates/warehouse.gate that guards it allows the workflow `ship_order` in no combination
    62 |   order <- get_order(id: order_id, expand: "items")
  = Every run that comes to the call is denied there.
  = Write a permit in gates/warehouse.gate that allows the workflow (`principal is workflow ship_order`), or take the call out.
```

### 4.7 Cedar の解析との関係

Cedar Analysis（2025-06 に公開）の四つの検出は、sekisho の検査と重なる。「Forbid Overrides」は E302、「Impossible Conditions」は E303、「Shadowed Permits」は W301、「Complete Denials」は E301 にあたる。違うのは、SymCC が context の値を自由な値として扱うのに対し、sekisho は計算した値がとりうる値（rulec と koyomi の答え）だけを数えることである。koyomi の日付で、ある述語の組が起こりえなければ、SymCC はそれを起こりうるものとして答え、sekisho は数えない。

sekisho は SMT のソルバーに頼らない。ritsu の検査は、ネットワークも外のソルバーも使わずに走る形をとってきたからである。生成した Cedar を SymCC で確かめることは、cvc5 があるときだけ走るテストとして足せる（15 章）。

## 5. 生成するもの

`sekisho gen <file.gate>... --target cedar|typescript|python|go... [--authorizer cedar|avp] [--module <path>] [--out <dir>] [--check] [--root <dir>]` が書く。`ritsu gen` は、プロジェクトの `.gate` から、同じものをパッケージの中に書く（5.6）。段階 B で `--target cedar` を、段階 C で `typescript`・`python`・`go` と `--authorizer`・`--module` を作った。`--target` は省けず、何度でも書ける（23 章）。三つの言語のコードは、サービスの言語を選んで書くもので、全部をいつも書くものではないからである。

`.gate` ごとに、`--target typescript` は `<out>/typescript/authz/<別名>.ts` を、`python` は `<out>/python/authz/<別名>.py` を、`go` は `<out>/go/authz/<パッケージ>/<パッケージ>.go` を書く。中身は `ritsu gen` がパッケージに入れるもの（5.6）と同じで、隣の `rules/` と `dates/` にある規則と日付の生成物を読む。rulec と koyomi の生成物の場所をコマンドごとに変えず、どちらから書いても同じコードにするためである。Go の import のパスは `--module` で決める（既定は `generated`。`ritsu gen` の既定と同じ）。

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
| ポリシーが読む `allowed`（2.11） | `context` の `allowed_<尋ねる action の別名>: Bool`（省けない）。`@doc` に、同じ principal と resource で尋ねること、入力に渡すもの、生成したコードが尋ねて計算し、呼ぶ側からは受け取らないことを書く。尋ねる action が日本語の名前を持てば `@name("allowed 承認する")`。期待だけが読む `allowed` は書かない |
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

ポリシーが `allowed` で読む答えも、context の真偽になる。例の `request_approval`（実物。`--root examples/approvals`）：

```cedarschema
  @doc("Ask a higher approver to approve a purchase")
  action "request_approval" appliesTo {
    principal: [User],
    resource: [Purchase],
    context: {
      @doc("Whether approve is allowed for the same principal and resource, its inputs given amount: amount. Asked and computed by the generated code, never taken from the caller")
      allowed_approve: Bool,
      @doc("rulec \"rules/approval_limit.rule\" output band, from amount and principal.approval_limit: within_limit or over_limit. Computed by the generated code, never taken from the caller")
      band: String
    }
  };
```

ポリシーは `unless { context.allowed_approve };` と読む。日本語の版を `--lang ja` で書くと、`@doc` は「同じ principal と resource で approve が許されるか。approve の入力には amount: amount を渡す。生成したコードが尋ねて計算し、呼ぶ側からは受け取らない」になる。

`@guards` は、守る操作を参照の書き方で書く（2.6 の四つの形）。パスはルートからで、上の例は、例のディレクトリをルートにして（`--root examples/refunds`）生成したものである。`tests/gen/guards.gate` は、二つの操作の一つを方法とパスで書いていて、クレートのディレクトリをルートにすると `@guards("openapi \"examples/refunds/api/orders.json\" operation getOrder\nopenapi \"examples/refunds/api/orders.json\" operation refundOrder")` になる（`operationId` で書く）。

一つの action が二つ以上の操作を守るときは、一つの `@guards` に、参照を一行に一つ並べる（Cedar の注釈は、一つの宣言に同じキーを二つ持てない）。一行一行は `Name::text` が書いたもので、`api` の `guards` と口 `Gates`（8.3）の `GateAction::guards` と同じ値である。口が手で書いた Cedar の `@guards` を読むときも、同じ参照の書き方で読む。

計算した値とワークフローの `@doc` の参照（`rulec "rules/refund_limit.rule" output band`、`koyomi "dates/refund_terms.cal" date last_day`、`koyomi "calendars/england_and_wales.cal"`、`dandori "flows/returns.flow"`）も、`@guards` と同じくルートからのパスで書き、`Name::text` が書く（段階 D、2026-10-06）。口 `References` が出す参照（8.2）と同じ値である。読むファイルがルートの外なら、E201 か E208 で生成しない（2.6）。例は `.gate` がルートにあるので、`use` の行に書いたパスと同じになる。`tests/gen.rs` の `what_a_gate_reads_is_named_from_the_root` は、例の二つの版を一つ下のディレクトリ（`gates/`）に置いてパスを `../rules/…` と書き直し、上のディレクトリをルートにして生成したスキーマが、頭の二行のほかは例のものと一字も違わない（`@doc` に `../` が出ない）ことと、`gates/` をルートにすると四つの `use` の行が E201、`workflow` の行が E208 になることを確かめる。

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

action ごとに、TypeScript・Python・Go で二つの関数を生成する。TypeScript なら `refundOrderRequest` と `authorizeRefundOrder`（Python は `refund_order_request` と `authorize_refund_order`、Go は `RefundOrderRequest` と `AuthorizeRefundOrder`）である。ゲートごとに一つ、action の名前で尋ねる入口（TypeScript と Python は `authorize`、Go は `Authorize`）も生成する。入口は、名前から action の関数を選んで呼ぶだけである（30 章）。役割の一覧も二つ生成する。ゲートが宣言した役割（TypeScript と Python は `roles()`、Go は `Roles()`）と、Cedar での名前で引く principal の型が持てる役割（TypeScript は `rolesOf(t)`、Python は `roles_of(t)`、Go は `RolesOf(t)`）で、サインインが運ぶ役割を、サービスがレコードに入れる前に絞るためのものである（33 章）。

- `<action>Request(store, principal, input, now)`：`Store` から resource と principal を読み、`input` と範囲を確かめ、rulec と koyomi の生成物を呼んで計算した値を求め、Cedar のリクエストとエンティティを返す。計算できなければ、種類の付いたエラー（`principal`・`resource`・`input`・`today`・`rule`・`date`）を投げる（3.8）。
- `authorize<Action>(store, principal, input, now)`：上で組み立てて Cedar に尋ね、答え（許すか、決めたポリシーの `@id` を並べ替えたもの、Cedar に渡した context、拒んだ理由）を返す。上のエラーは拒む答えにする。Cedar がエラーを言ったとき（`--authorizer avp` では尋ねられなかったときも）も拒む答えにし、種類は七つ目の `cedar` である。評価の途中でエラーになったポリシーを Cedar は当てはまらなかったものとして扱い、forbid が効かずに許すことがあるからである。`Store` の失敗（データベースに届かない、など）は、そのまま投げる。判断ではなく、呼ぶ側が扱う失敗だからである。`--authorizer avp` では、尋ねる先 `avp` を最初の引数で受け取る（Go は `ctx` のあと。5.5）。

`principal` は型と ID（ワークフローは名前）、`input` は宣言した `input` と、resource の ID を持つ。resource の ID のフィールドは、`from` を書けばその引数（`orderId`）、書かなければ `resource` である。resource の型を二つ以上とる action では、`resource_type` が型を言う。`now` は時刻で、日は `today` のオフセットで決める。

確かめる順は決めてある。最初に外れたものの種類で拒み、Cedar には尋ねない。

1. `principal`：principal の型が、その action のとるものでない。宣言していないワークフロー。
2. `today`：その action の計算した値が `today` を読むとき、`now` の日が `today` の範囲の外。`today` を読まない action では確かめない（2.9）。
3. `input`：宣言した入力が無い（`?` のものを除く）、型か範囲か列挙の外。
4. `resource`：resource の ID が無い（TypeScript と Python）。resource の型が、その action のとるものでない。読むときは、`Store` が返さない。属性が無い（`?` のものを除く）か、範囲か列挙の外。
5. `principal`：4 と同じく principal。役割がその型の `roles` に無い、メンバーであるグループの型が違う。
6. `rule`・`date`：規則の生成物が入力を受け付けない、日付が止まる、カレンダーの知らない日。
7. `allowed`（2.11）：ポリシーが読む `allowed` の action を、同じ principal、resource、`now` で、その action の `authorize<Action>` に尋ね、答えの真偽を context に入れる。その action が拒めば（リクエストを組み立てられないか、Cedar がエラーを言えば）、同じ種類で拒み、文の前にその action の別名を付ける（`approve: …`。三つの言語で同じ）。尋ねる action が `today` を読めば、2 の `today` も確かめる。

`Store` がエンティティについて返すものは、型ごとに一つに決まり、どの action でも同じである。役割（型が役割を持つとき）、メンバーであるグループ（ポリシーがその型の principal に聞くとき）、属性（スキーマがその型に与えるものと、その型をとるどれかの action の計算した値が読むもの）である。このどれかがある型は、その型をとるどの action でも読み、全部を確かめる。どれも無い型（例の `Customer`、`RefundRecord`）は ID だけでエンティティを作り、ワークフローは名前で作る。action ごとに読むものを変える形も試したが、同じデータが action によって拒まれたり通ったりし、型ごとの `Store` の形とも合わないので捨てた。

例の `refund_order` の TypeScript（`ritsu sekisho gen examples/refunds/refunds.gate --target typescript` の実物から）：

```ts
export interface User {
  /** The roles it holds directly (clerk, manager, auditor) */
  roles: Role[];
  /** money[GBP, incl_tax], 0 to 10000 */
  refund_limit: bigint;
  suspended: boolean;
}
…
export async function refundOrderRequest(store: Store, principal: Principal, input: RefundOrderInput, now: Date = new Date()): Promise<Request> {
  if (principal.type !== "User" && principal.type !== "Workflow") _fail("principal", `refund_order is asked by User, Workflow, not by a ${(principal as Uid).type}`);
  if (principal.type === "Workflow" && !_WORKFLOWS.includes(principal.id)) _fail("principal", `the gate declares no workflow ${principal.id}`);
  const day = _today(now);
  const in0 = _num(input.amount, 1n, 10000n, "input", "amount");
  const resourceId = _id(input.orderId, "resource", "orderId");
  …
  const r_Order: Order | undefined = _read_Order(await store.order(resourceId), "resource", resourceId);
  const p_User: User | undefined = principal.type === "User" ? _read_User(await store.user(principal.id), "principal", principal.id) : undefined;
  const context: Record<string, Value> = {};
  context["amount"] = Number(in0);
  // refund_band = rulec "rules/refund_limit.rule" output band, from amount: amount, limit:
  // principal.refund_limit
  if (p_User !== undefined) {
    context["refund_band"] = _public({ [rule_refund_limit.RefundBand.WITHINLIMIT]: "within_limit", [rule_refund_limit.RefundBand.OVERLIMIT]: "over_limit" }, _rule(() => rule_refund_limit.refund_limit(in0 as rule_refund_limit.GBPInclTax, p_User.refund_limit as rule_refund_limit.GBPInclTax)));
  }
  // in_period = today <= koyomi "dates/refund_terms.cal" date last_day, from paid_on:
  // resource.paid_on
  if (r_Order !== undefined) {
    context["in_period"] = day <= _dated(() => dates_refund_terms.last_day(r_Order.paid_on));
  }
  // business_day = today is open in koyomi "calendars/england_and_wales.cal"
  context["business_day"] = _dated(() => dates_england_and_wales.is_open(day));
  …
```

規則と日付の生成物の呼び方は、口から取る。rulec の関数の名前と引数の型は `RuleFacts::typescript`・`python`・`go`（ritsu の DESIGN 3.2）、koyomi の関数の名前と引数の順は `DateFacts` の日付の別名と入力、カレンダーの関数は `is_open` で、モジュールはカレンダーの別名（`Dates::calendar`）である。どちらのモジュールも名前空間ごと別名で読み（TypeScript は `import * as rule_refund_limit`、`dates_refund_terms`、Python は `from ..rules import refund_limit as _rules_refund_limit`、Go は `rulesrefundlimit "<module>/rules/refundlimit"`）、ゲートの型や列挙の名前とぶつからないようにする。ゲートの型や列挙の名前が、生成するコードが自分で使う名前（`Principal`、`Store`、`Request`、`Answer` など）や、呼ぶ組み込みの名前、読み込む名前（TypeScript の `Date`、Python の `Protocol` など）とぶつかるときは、名前を付け替えず、`check` が W402 で言い、その出力先の `gen` を E401 で止める（24 章）。名前が、コードがそのまま書く所でその言語の予約語になるとき（Python のレコードのフィールドと `Store` のメソッド、ファイルの別名がなる Python のモジュールと Go のパッケージの名前、`ritsu gen` の TypeScript の索引が書き出す名前）も同じである（32 章）。

- 規則の列挙の出力は、生成物の値をそのまま Cedar に渡さない。rulec の生成物の値は `.rule` に書いた名前で、日本語の版では `上限まで` になるからである。生成物のメンバーから公開名（`EnumValue::public`。`within_limit`）を引く表を書き、それで引いて渡す（16.1 の 14）。設計の段階の見本は値をそのまま渡していて、英語の版でしか合わなかった。ゲートの列挙の値を規則の列挙の入力に渡すときも、値ごとの表で規則のメンバーを引く。
- 数は、宣言した単位で数えた整数として受け取る（TypeScript は `bigint`、Python は `int`、Go は `int64`）。規則に渡すときに、規則の単位の型（`GBPInclTax`）にする。二つの規則が同じ単位の型をそれぞれ持てば、Python の `NewType` も Go の名前付きの型も別の型になり、どちらかの型で受け取ると、もう一方に渡せないからである。
- 日付は、Python は `datetime.date`、Go はパッケージの `Date{Year, Month, Day}` で受け取る。TypeScript では `YYYY-MM-DD` の文字列で受け取り、koyomi の生成物にはそのまま渡し、rulec の生成物の日付の入力には 1970-01-01 からの日数を渡す（rulec の生成物がそう受け取る）。`today` の比べ方（`<=` など）は文字列の比べ方で、同じ長さの `YYYY-MM-DD` なので日の順と同じになる。
- Go では、koyomi の日付のパッケージがそれぞれ自分の `Date` の型を持つので、生成するコードは、日付のファイルとカレンダーのあいだで年月日を詰め替える。

`allowed` は、尋ねる action の関数を呼ぶ一行になる。例の `request_approval` の TypeScript（`ritsu sekisho gen examples/approvals/approvals.gate --target typescript --root examples/approvals` の実物から）：

```ts
  // allowed_approve = allowed approve(amount: amount): asked with the same principal and resource
  context["allowed_approve"] = _allowed("approve", await authorizeApprove(store, principal, { purchaseId: resourceId, amount: in0 }, now));
```

Python は `context['allowed_approve'] = _allowed('approve', authorize_approve(store, principal, q_approve, now))`、Go は `AuthorizeApprove(store, p, ApproveInput{PurchaseId: rID, Amount: in.Amount}, now)` の答えを `ctx["allowed_approve"]` に入れる。入力に渡すのは、名前で渡す入力なら呼ぶ側が渡した値、属性なら `Store` から読んだ値、定数ならその値である。`--authorizer avp` では、`allowed` を読む action の `<action>Request` も、尋ねる先 `avp` を最初の引数で受け取る（Go は `cx context.Context` と `avp`。関数の中の `ctx` は context の値の名前である）。尋ねる action を、同じポリシーストアに尋ねるためである。生成した Python は、あとで読まない確かめ（`today` を読む action を尋ねるだけの action の `today` の範囲、尋ねる action にだけ名前で渡す入力）を、変数に入れずに呼ぶ（pyright の strict が、読まない変数を誤りにするため）。

三つの言語の形（例の `refund_order`）：

| | TypeScript | Python | Go |
|---|---|---|---|
| 組み立てる | `refundOrderRequest(store, principal, input, now = new Date()): Promise<Request>`。拒むときは `SekishoError` を投げる | `refund_order_request(store, principal, input, now=None) -> Request`。拒むときは `SekishoError` を投げる | `RefundOrderRequest(store Store, p Principal, in RefundOrderInput, now time.Time) (Request, error)`。拒むときは `*Error` を返す |
| 尋ねる | `authorizeRefundOrder(store, principal, input, now = new Date()): Promise<Answer>` | `authorize_refund_order(store, principal, input, now=None) -> Answer` | `AuthorizeRefundOrder(store, p, in, now) Answer` |
| 名前で尋ねる（30 章） | `authorize(store, principal, action, resources, now = new Date()): Promise<Answer>`。`resources` は `{ readonly [type: string]: string \| undefined }` | `authorize(store, principal, action, resources, now=None) -> Answer`。`resources` は `dict[str, str]` | `Authorize(store, p, action, resources, now) Answer`。`resources` は `map[string]string` |
| 役割の一覧（33 章） | `roles(): string[]`、`rolesOf(t: string): string[]` | `roles() -> list[str]`、`roles_of(t: str) -> list[str]` | `Roles() []string`、`RolesOf(t string) []string` |
| `--authorizer avp` | `authorizeRefundOrder(avp, store, principal, input, now)`。`avp` は `VerifiedPermissions { client, policyStoreId }` | `authorize_refund_order(avp, store, principal, input, now=None)`。`avp` は `VerifiedPermissions(client, policy_store_id)` | `AuthorizeRefundOrder(ctx, avp, store, p, in, now)`。`avp` は `VerifiedPermissions{Client, PolicyStoreID}` |
| `Store` | `interface`。型ごとに `user(id): Promise<User \| undefined \| null>` | `Protocol`。型ごとに `user(id) -> User \| None` | `interface`。型ごとに `User(id string) (*User, error)` |
| レコード | `interface`。役割は `roles`、グループは `member_of`（型と ID） | `@dataclass(frozen=True)`。役割は `roles`、グループは `member_of_<型>`（ID） | 構造体。`Roles`、`MemberOf<型>` |
| 無いことがある値 | `?` のプロパティ（`null` も受け付ける） | `X \| None` | ポインタ |
| `now` | `Date`（省けばいま） | タイムゾーンのある `datetime`（省けばいまの UTC。`today` を読む action では、タイムゾーンの無いものは日を決められないので、`today` で拒む） | `time.Time` |

レコードは ID を持たない。ID は尋ねたもので、`Store` が別の ID のものを返す食い違いを作らないためである。Python の規則の呼び方（`ritsu sekisho gen examples/refunds/refunds.gate --target python` の実物から）：

```python
    # refund_band  = refund_limit(amount: amount, limit: principal.refund_limit).band
    if p_user is not None:
        try:
            v_refund_band = _rules_refund_limit.refund_limit(_rules_refund_limit.GBPInclTax(in_amount), _rules_refund_limit.GBPInclTax(p_user.refund_limit))
            context['refund_band'] = {_rules_refund_limit.RefundBand.WITHINLIMIT: 'within_limit', _rules_refund_limit.RefundBand.OVERLIMIT: 'over_limit'}[v_refund_band]
        except Exception as x:
            raise SekishoError("rule", f'rulec "rules/refund_limit.rule": {x}') from x
```

### 5.4 リクエストとエンティティの JSON

Cedar のエンティティの JSON の形（`{"uid": {"type", "id"}, "attrs", "parents"}`）で書く。

- 役割のエンティティ：`.gate` の役割と `includes` から作った定数。
- principal：スキーマがその型に与える属性と、`Store` が返した役割とグループを親にしたもの。ワークフローは属性を持たない。
- resource：スキーマがその型に与える属性。エンティティの型の属性は `{"__entity": {…}}` で書き、指す先の型がスキーマで属性を持たないときは、指す先のエンティティも入れる（例の `customer`）。属性を持つ型のものは書かない。Cedar はそのエンティティの属性を読まないからである（`vectors` と同じ。6.1）。同じ ID のエンティティは一つだけ入れる（顧客が注文の持ち主で、principal でもあるとき）。
- context：ポリシーが読む `input`（宣言した単位で数えた整数）と、計算した値の全部（計算しない型のときと、値の無いときは入れない）。

数は JSON の数として書く。TypeScript の `number` で正確に表せるのは 2⁵³ までなので、宣言する範囲は ±(2⁵³ − 1) に収める（E103）。

### 5.5 Cedar の実装の呼び方（2026-10 の時点）

| 実装 | 版（確かめた日） | 生成するコードからの呼び方 | 気をつけること |
|---|---|---|---|
| 公式の CLI `cedar-policy-cli` | 4.13.0（2026-09-15。Cedar の言語の版は 4.5） | テストだけで使う（`validate`・`format --check`・`run-tests`・`translate-*`） | `@id` を名前にする |
| Rust `cedar-policy` | 4.13.0（crates.io、2026-09-15） | 生成しない（Rust は生成先に無い。9.3 の三つの言語） | — |
| npm `@cedar-policy/cedar-wasm` | 4.13.0（2026-09-15） | `isAuthorized({principal, action, resource, context, entities, policies: POLICIES, schema: SCHEMA, validateRequest: true})`。Node は `@cedar-policy/cedar-wasm/nodejs`。`POLICIES` は `.policies.json`（`@id` ごと）、`SCHEMA` は `.cedarschema.json` を、モジュールの中の JSON の文字列から一度だけ読んだもの | 一つのテキストを渡すと `policy0`… になる |
| Go `github.com/cedar-policy/cedar-go` | v1.8.0（2026-06-01） | `json.Unmarshal` で JSON の形のポリシーを `cedar.PolicySet` に読み、`cedar.Authorize(ps, entities, req)`。スキーマは渡さない | 検証器は実験（`x/exp/schema`）で、テンプレートと部分評価が無い（sekisho は使わない）。だから生成するコードはスキーマを持たず、範囲と列挙は尋ねる前に自分で確かめる。`NewPolicySetFromBytes` は `policy0`… |
| Python `cedarpy`（k9securityio。AWS の公式ではない） | 4.12.1（2026-09-24。Cedar 4.12.0） | `cedarpy.PolicySet.from_json_str(…)` と `cedarpy.Schema.from_json_str(…)` をモジュールを読み込むときに一度作り、`is_authorized(request, policies, entities, schema=…)`。スキーマを渡すので、cedarpy がリクエストとエンティティをスキーマで確かめる | 文字列で渡すと `policy0`… |
| Amazon Verified Permissions | Cedar 4 に上げた（`is`、タグ、`datetime` と `duration` の値が使える。Verified Permissions の Cedar 4 の FAQ） | `IsAuthorized`（`entities` と `context` は `cedarJson` で渡す） | 決めたポリシーは生成された `policyId` で返る（5.8） |

`--authorizer cedar`（既定）は、その言語の Cedar の実装をプロセスの中で呼ぶ。`--authorizer avp` は、AWS の SDK（`@aws-sdk/client-verifiedpermissions` 3.1146.0、boto3 1.43.103、`github.com/aws/aws-sdk-go-v2` v1.47.1 と `service/verifiedpermissions` v1.41.1）で `IsAuthorized` を呼び、context とエンティティを `cedarJson` で渡す。boto3 は dandori の `tools/wire` と同じ版で、Python の型の検査には `boto3-stubs[verifiedpermissions]` 1.43.103 を使う。どちらも、組み立てる関数（`<action>Request`）は同じである。`avp` の `authorize<Action>` は、尋ねる先 `avp` を最初の引数で受け取る（Go は `ctx` のあと）。型はどの言語も `VerifiedPermissions` で、Verified Permissions のクライアントとポリシーストアの ID を持つ（TypeScript は `{ client, policyStoreId }`、Python は `(client, policy_store_id)`、Go は `{Client, PolicyStoreID}`。Go の `Client` は `IsAuthorized` を持つもので、AWS の SDK の `*verifiedpermissions.Client` がそうである）。モジュールに AWS の資格や地域を書き込まないためである。`avp` のモジュールはポリシーとスキーマを持たない。持つのはポリシーストアである（5.8）。

### 5.6 `ritsu gen` のパッケージへの入り方

ritsu の DESIGN 9.3 のパッケージに、次を足した。

```
generated/cedar/<別名>.cedar, <別名>.cedarschema, <別名>.cedarschema.json, <別名>.policies.json   言語に依らない（一度だけ書く）
generated/typescript/authz/<別名>.ts, authz/index.ts
generated/python/<name>/authz/<別名>.py
generated/go/authz/<パッケージ>/<パッケージ>.go
```

- `ritsu gen` は、プロジェクトの `.gate` を `ritsu check` と同じく全部の言語をつないで確かめ、通らなければ何も書かない（ほかの言語のファイルと同じ）。`.gate` が読む規則と日付とカレンダーは、プロジェクトのファイルでなければならない（ワークフローと同じ。パッケージの `rules/` と `dates/` から読むため）。
- Cedar の四つのファイルは、`--target` に依らず `<out>/cedar/` に一度だけ書く。頭は、プロジェクトの根からのパスで元の `.gate` を言う（ritsu の DESIGN 9.2）。`--check` も同じく見る。
- 認可のモジュールは、同じパッケージの `rules/` と `dates/` の生成物を読む（ワークフローと同じ）。インデックスのファイル（`index.ts` と `authz/index.ts`、`__init__.py`）に `authz` を足す。パッケージの説明の文は、`.gate` があるときだけ「ゲートに尋ねるコード」を言う。
- 依存は、`.gate` があるときだけ書く。TypeScript は `@cedar-policy/cedar-wasm` を `"4.13.0"`（`--authorizer avp` なら `@aws-sdk/client-verifiedpermissions` を `"3.1146.0"`）、Python は `cedarpy==4.12.1`（avp なら `boto3==1.43.103`）、Go は `doc.go` に `github.com/cedar-policy/cedar-go v1.8.0`（avp なら `github.com/aws/aws-sdk-go-v2 v1.47.1` と `github.com/aws/aws-sdk-go-v2/service/verifiedpermissions v1.41.1`）。`ritsu gen` も `--authorizer cedar|avp` を取る。
- 書くバージョンは、sekisho の `tools/` のランナーのロックファイルのバージョンと同じにし、ritsu の監査（ritsu の DESIGN 3.6）がそのロックファイルを調べる。`crates/ritsu/tests/audit.rs` は、sekisho の例をプロジェクトにして二つの authorizer でパッケージを生成し、書いたバージョンがランナーのロックファイル（`tools/runner-ts/package-lock.json`、`tools/runner-py/requirements.txt`、`tools/runner-go/go.mod`）と同じことを確かめる。TypeScript の版は、生成器の定数（`sekisho::r#gen::typescript::CEDAR_WASM`、`AWS_SDK`）を `ritsu gen` も読む。
- ポリシーとスキーマは、生成するモジュールの中に定数として書き込む（koyomi が祝日の表を書き込むのと同じ）。リクエストを組み立てるコードと、評価するポリシーの版が食い違わないようにするためである。Verified Permissions を使うときは、ストアに置いたポリシーと生成したコードの版を合わせるのは、置く側の仕事になる（5.8）。
- `ritsu gen --authorizer avp` も、`sekisho gen` と同じく W401 を言う（5.8）。
- 確かめ方：`crates/ritsu/tests/gen.rs` が、sekisho の例をプロジェクトにして三つの言語のパッケージを書き、ファイルの形、インデックス、依存、型の検査（TypeScript は dandori のランナーの `tsc --strict` と sekisho のランナーの cedar-wasm の型、Python は `mypy --strict` と cedarpy、Go は sekisho の Go のランナーの `go.mod` で作ったモジュールの `go vet` と gofmt）、`--check` が何も言わないことと `.gate` を変えたときに言うこと、`--authorizer avp` の依存を確かめる。中身の突き合わせは sekisho のテスト（6.3）がする。

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
- ポリシー：`.cedar` のポリシーを一つずつ Cedar のテキストに書いたもの（`ritsu_base::cedar::write_policy`）を、`CreatePolicy` の `definition.static.statement` にし、`name` に `@id` を渡す。`statement` は Cedar のポリシーの言語で書いた中身で（API の文書の `StaticPolicyDefinition`）、JSON の形ではない。`.policies.json` は、三つの実装（cedar-wasm、cedarpy、cedar-go）に `@id` ごとに渡すためのものである（5.2）。`name` はポリシーストアの中で一意な名前で、ポリシーを指す API では ID の代わりに使え、そのときは頭に `name/` を付ける（`name/refunds/clerks_refund_within_their_limit`）。`@id` は `name` の形（`[a-zA-Z0-9-/_]*`、150 字まで）に収まる（5.2）。どれも 2026-10-06 に、API の文書の CreatePolicy（Request Syntax と `definition`・`name` の説明。<https://docs.aws.amazon.com/verifiedpermissions/latest/apireference/API_CreatePolicy.html>）、StaticPolicyDefinition（<https://docs.aws.amazon.com/verifiedpermissions/latest/apireference/API_StaticPolicyDefinition.html>）、GetPolicy（`policyId` に名前を渡すときの `name/`。<https://docs.aws.amazon.com/verifiedpermissions/latest/apireference/API_GetPolicy.html>）で確かめた。本物のアカウントでは確かめていない（15 章）。
- `IsAuthorized` の `determiningPolicies` は、ストアが付けた `policyId` を返す。生成する `--authorizer avp` のコードは、それをそのまま返し、ページと文書に、名前で引けることを書く。
- 上限（2026-10 の Verified Permissions のクォータ）：一つのポリシーは 10,000 バイトまで、スキーマは 100,000 バイトまで、一つのスキーマの名前空間は 100 まで、principal・action・resource のそれぞれの推移的な親は 100 まで、一つの認可のリクエストは 1 MB まで。`sekisho gen --authorizer avp`（`--target` に依らない）と `ritsu gen --authorizer avp` は、次のどれかが超えれば W401 で言い、生成は続ける。ポリシーは、一つずつを `cedar format` の形で書いたテキストの大きさ（ストアに置くもの）。スキーマは JSON の形（`PutSchema` が受け取るもの）の大きさ。親は、型が持てる役割の全部と、それらが含む役割の数（役割を全部持つ principal の推移的な親）。グループのメンバーであることはデータで決まり、数えられないので数えない。名前空間は一つのゲートで一つなので、リクエストの大きさはデータで決まるので、見ない。例（`sekisho explain W401` の再現。役割が一つずつ前の役割を含み、101 個並ぶ）：

  ```text
  warning[W401]: example.gate:206:1: A User can hold 101 roles with the roles they include, past the 100 transitive parents Verified Permissions lets an entity have
     206 | principal User
    = Make the roles nest less deep, or spread the type's roles over types of their own.
  ```
- 置く手順（AWS の CLI、CloudFormation の `AWS::VerifiedPermissions::Policy` など）は生成しない。置くのは使う人で、sekisho が書くのは置くためのファイルだけである。

## 6. 本物の Cedar との突き合わせのテスト

### 6.1 決め：公式の CLI の `run-tests` で、全部の組み合わせ

- `sekisho vectors <file.gate> [--action <action>]` が、全部の組み合わせを、公式の CLI の `cedar run-tests` が読む形で書く。JSON の配列で、一行に一つのテスト（`name`、`request`、`entities`、`decision`、`reason`、`num_errors` の順）である。`decision` と `reason`（決めたポリシーの `@id`）は sekisho の参照の評価の答えで、`num_errors` はいつも 0 である。
  - 数の区間は、両端の値を一つずつ使う（境目の一つずれを見つけるため。rulec の vectors と同じ考え）。区間が二つ以上の値を持つ組み合わせは、リクエストに入る数を全部 low の端にしたものと、全部 high の端にしたものの二件にする（名前は `refund_order 1 (amount 1)` と `refund_order 1 (amount 50)`）。区間の中では、どの比べも一つの答えになるので、二件の答えは同じである。区間が一つの値か、数の無い組み合わせは一件（`view_order 1`）。
  - エンティティは、役割（`includes` した役割を親に）、principal（持つ役割と、メンバーであるグループを親に）、resource と、属性が指すエンティティのうち、スキーマで属性を持たない型のもの（例の `Customer`）。属性を持つ型のもの（`tests/walk/relations/tenants.gate` の、principal でない `owner`）は書かない。Cedar はそのエンティティの属性を読まず、リクエストは、エンティティを書かずに指してよい。
  - ID は、型の別名を小文字にして番号を付ける（`user1`、`order1`、`customer2`）。同じエンティティを指す項（2.5 の分け方の一つの組）は同じ ID で、principal を指すなら principal の ID である。ワークフローの principal は、ワークフローの別名を ID にする。
  - 組み合わせが数えない値（ほかの action のポリシーだけが読む属性など）は、その値がとりうる最初の値にする（真偽は false、列挙は最初の値、数は下限）。省けるものは書かない。
  - 例の英語の版では、1,078 通りから 2,134 件になる（`view_order` 18、`refund_order` 1,056 × 2、`export_refunds` 4）。日本語の版は、名前空間と ID の頭のほかは一字も同じである（`tests/cli.rs`）。
- テスト（`tests/cedar.rs`）は、例とテストの材料の `.gate` のうち、検査を通るものの全部を、言語を全部つないで生成し（`gen --target cedar` を `--lang en` と `--lang ja` で）、公式の CLI に次をかける。`cedar validate`（strict。警告も誤りとする）を、Cedar の形と JSON の形の両方に。`cedar format --check`。JSON の形が `cedar translate-policy` と `translate-schema` の出力と一字も違わないこと。`cedar run-tests` を、`sekisho vectors` が書く全部のテストに、Cedar の形と JSON の形の両方で。あわせて、検査を通るファイルにだけ `gen` が書くこと、vectors の組み合わせの数と許す数が検査の数えたものと同じであること、一つの組み合わせの二件の答えが同じであることも確かめる。
- `cedar run-tests` は、テストに書いた決めたポリシーが、実際に決めたポリシーの中にあるかだけを見る（4.13.0 の `cedar-policy-cli/src/command/run_test.rs`）。多いものは見ない。そこでテストは、ポリシーを一つずつ permit にして単独で流し、そのポリシーが当てはまるかを答えが言うテストで、当てはまるところが答えと同じであることを確かめる。forbid は、拒んだ答えの決めたポリシーにあるところでだけ当てはまる。permit は、許した答えの決めたポリシーにあるところで当てはまり、それ以外の許した答えと、forbid の無い拒んだ答えでは当てはまらない。forbid が拒んだテストでは、permit が当てはまるかを答えは言わないので、流さない。これと `run-tests` で、決めたポリシーが一致する。
- スキーマが要るとする属性（エンティティの型と action の context）ごとに、その属性を持つ最初のテストから属性を一つ抜いたテストを作り、`cedar run-tests` がどれもスキーマに合わないと言うことを確かめる（Cedar の形と JSON の形の両方で）。Verified Permissions は、渡されたスキーマでリクエストを確かめる。要る属性を省けるものとして書いたスキーマは、値の無いリクエストを受け付け、値が無いものとして答える。この誤りは、読むところが全部 `has` で守られていると、`validate` にも答えにも出ない。
- 生成の誤りを見つけられることも、同じファイルで確かめる。生成したポリシーとスキーマを ritsu-base の Cedar で読み、一か所ずつ変えて書き直した変異のうち、検査が数える組み合わせのどれかの答えを変えるものの全部が、上の確かめ（`validate`、`run-tests`、ポリシーを一つずつ流すこと、要る属性を抜いたテスト）のどれかで落ちること（6.2）。構文木を変えて書き直すので、どの変異も CLI が読め、`cedar format` の形である。答えに出ない変異（役割が一つで `includes` が無いときの、役割の型の親）は作らない。
- どの確かめにも落ちない変異は、検査が数える組み合わせの全部で元と同じ答えを出すと、テストが自分で示せたものだけを、数から分けて名前で言う（2026-11-17）。示せるのは、三つが揃うときである。vectors が、検査が数える組み合わせの全部を持つこと（組み合わせの数と許す数が、検査の数えたものと同じ）。その全部で、変異が元と同じ決定と、同じ決めたポリシーを出すこと（どの確かめにも落ちないことが、それを言う）。区間の中でも、変異が元と同じ答えを出すと言えること。検査は数を、条件が比べる定数で区間に切り、vectors は区間の両端を試す。変異が、元のポリシーが数や時刻と比べていない定数と、数や時刻を比べなければ、区間の中で答えが変わる所は無い。比べるとき（`resource.amount > 1000` の 1000 を 1001 にする、`>` を `>=` にする）は、新しい切れ目が区間を分ける。比べる数が principal か resource か context の整数の属性の一つなら、テストは、vectors の各件のその数を、新しい定数とその一つ下と一つ上（vectors がその属性にとる値の範囲の中）に置いた要求を作り、元の Cedar の答え（決定と決めたポリシー）を CLI に求め（`run-tests` と、ポリシーを一つずつ permit にして流したもの）、変異が同じ答えを出すことを確かめる（`run-tests` と、ポリシーを一つずつ流す確かめ）。分けた区間のどの部分にも試した数があるので、区間の全部で同じと言える。二つの属性に切れ目を足す変異と、時刻や小数との比べに切れ目を足す変異は、組を全部試す要求が要るので、示せないものとする。スキーマを変えた変異は分けない。示せない変異は、前と同じくテストを落とす。起こりうる組み合わせだけを数える材料（4.2）では、起こらない組み合わせにだけ効く条件や、当てはまらない action を外す変異が、この形になる。
- CLI は、公式のリリースのバイナリ（`cedar-policy-cli` 4.13.0）を、チェックサムで確かめて置く。テストは ritsu-testkit の `cedar::cli()`（`Need::Cedar`、tools の段）で、環境変数 `RITSU_CEDAR`（か `SEKISHO_CEDAR`）、無ければ PATH の `cedar` を探し、`cedar --version` が `cedar-policy-cli 4.13.0` のときだけ使う。出力を一字ずつ比べるので、ほかの版では違う形で出すことがあるからである。無いか版が違えば `SKIP: sekisho: …` を言う（ritsu の DESIGN 10.3）。CLI を使うテストは三つ（全部のファイルを CLI にかけるもの、変異のもの、区間の中でだけ答えの変わる変異を等しいと言わないことのもの）で、それぞれが SKIP の行を一つ出す。CI は ritsu の `tools.yml` の「それ以外」の組で、Linux の x86-64 のアーカイブ（`cedar-policy-cli-x86_64-unknown-linux-gnu.tar.xz`、SHA-256 `23d358ef38edad8ade629a09f6a7359ecbd1761c9ff82e9fd0a5c5bce36b8c79`。リリースの `.sha256` と `sha256.sum` と同じ）を、wasm-tools と protoc と同じ形で入れ、`RITSU_CEDAR` に渡す。Cedar の読み手と書き手（`ritsu_base::cedar`）の材料も、同じ組が同じバイナリの `expected.sh` で作り直し、一字も変わらないことを確かめる（ritsu の DESIGN 4.18）。CI の入れ方は一つである。
- ほかの言語のランナーと突き合わせのテストと同じ形にする：生成物（Cedar）を、本物の評価器（公式の CLI）に、参照の評価が作った全部の場合をかけ、一字も違わないこと。dandori が Temporal の dev server で、chobo が PostgreSQL、TigerBeetle、MySQL、TiDB、Vitess で確かめるのと同じ位置にある。

### 6.2 試したこと（2026-10-06）

例 `refunds.gate` を、sekisho が生成する形で手で Cedar に書き、検査が数える形で書いた試作（Python）で 1,090 通りの組み合わせと答えを作って、四つの実装にかけた。段階 A の検査は 1,078 通りを数える（4.1。試作は `export_refunds` で、ポリシーが読まない役割も数えていた）。下の表は、そのときの記録である。段階 B で、生成した Cedar と `sekisho vectors` で取り直したものは、表のあとに書く。

| 実装 | 結果 | 時間 |
|---|---|---|
| `cedar validate`（4.13.0、strict） | `policy set validation passed`、エラーも警告も無し | — |
| `cedar format --check` | 書いたテキストが整形の結果と同じ（最初は二か所違った：頭のコメントのあとの空行と、一行に収まる `when`。整形の結果を生成の形にした） | — |
| `cedar run-tests` | `results: 1090 passed, 0 failed` | 0.17 秒 |
| cedar-wasm 4.13.0（Node 23） | 1,090 通りとも一致（ポリシーを `@id` ごとに渡したとき）。一つのテキストで渡すと 134 一致、956 不一致（決めたポリシーが `policy0`… になる） | 1.4 秒 |
| cedarpy 4.12.1（Python 3.13） | 1,090 通りとも一致（`PolicySet.from_json_str`）。文字列で渡すと同じく `policy0`… | 0.4 秒 |
| cedar-go v1.8.0（Go 1.25.5） | 1,090 通りとも一致（JSON の形を `json.Unmarshal`） | 2.6 秒（`go run` のビルドを含む） |

テストが誤りを見つけることも試した。ワークフローの permit の `context.amount <= 50` を `< 50` に変えると、`run-tests` は 4 通りで落ちた（区間の端の 50 を使っているため）。`context has refund_band &&` を消すと、`cedar validate` が `unable to guarantee safety of access to optional attribute refund_band` で落ちた。最初に書いたスキーマは `entity Role;` で、manager の親を clerk にしたエンティティを CLI が受け付けなかった（`Shop::Role::"manager"` is not allowed to have an ancestor of type `Shop::Role`）。5.1 の `entity Role in [Role]` はこれで決めた。

段階 B で、生成した Cedar と `sekisho vectors` で取り直した（例の英語の版。CLI 4.13.0、2026-10-06）：

| 確かめ | 結果 | 時間 |
|---|---|---|
| `cedar validate`（strict、`--deny-warnings`）、Cedar の形と JSON の形 | `policy set validation passed`、`no errors or warnings` | 0.02 秒、0.01 秒 |
| `cedar format --check` | 生成したテキストが整形の結果と同じ | 0.01 秒 |
| `cedar translate-policy`、`translate-schema` | `.policies.json`、`.cedarschema.json` と一字も違わない | — |
| `cedar run-tests`、Cedar の形と JSON の形 | `results: 2134 passed, 0 failed`（1,078 通りの 2,134 件。許すのは 97 通り） | 0.34〜0.35 秒 |
| ポリシーを一つずつ permit にして流す | 10 のポリシーとも、答えと同じところで当てはまる | — |
| 要る属性を一つ抜いたテスト | 6 件とも、スキーマに合わないと言う | — |

検査を通る材料の全部（131 のうち 27）で、同じ確かめが通る。`tests/gen/conditions.gate` は 1,444 通りの 2,888 件である。

生成したテキストを一か所ずつ変えた変異は、名前のほかが同じ Cedar と vectors になるファイルをまとめて、9 のファイルで 465 あり、どれもどれかの確かめで落ちた。例の 124 は、`validate` で 47、`run-tests` で 76 が落ち、残る 1 は `run-tests` を通って、ポリシーを一つずつ流す確かめだけで落ちた。`managers_refund_late_on_business_days` から `unless { context.in_period }` を除いたもので、期間内の営業日には `managers_refund_in_period` も許すので、答えは変わらず、決めたポリシーが一つ増えるだけである。同じ形のものが、ほかの材料に三つあった。`tests/gen/conditions.gate` の 106 のうち一つ（`Doc` の `dept` を省けるものにした変異）は、`validate` も `run-tests` も通り、要る属性を抜いたテストだけで落ちた。`dept` を読む permit は、resource が `Doc` と `Folder` の二つの型をとるので `resource has dept &&` で守っていて、vectors の `Doc` はいつも `dept` を持つからである。2026-10-11 に `allowed` の例と材料（2.11）を足したあとは、15 のファイルで 706 あり、どれもどれかの確かめで落ちる（足す前は 12 のファイルで 499）。承認の例で、役割 approver を employee に替えた変異を見つけるには、employee だけを持つ人を数える期待が要った（16.1 の 104）。2026-11-17 には、49 のファイルの 1,417 の変異がどれかの確かめで落ち、ほかに 8 のファイルの 22 の変異が、どの確かめにも落ちず、検査が数える組み合わせの全部で元と同じ答えを出すと示せた。テストは、その一つずつを、`.gate` の検査がその場所で言う警告と突き合わせる（action を外す変異はポリシーの行の W305 か W306 で、その action を名前で言うもの。条件の変異は、その条件を書いた行の W307 か W308）。警告が無ければ、言わないと決めた二つの形（`action any` のポリシーから action を外す、生成した `has` を外す。4.3）のどちらかであることを確かめ、どちらでもない等しい変異はテストを落とす。材料の写しで確かめたこと：`services.gate` のフォルダーに `open_to_all` を足すと、等しいと数えていた `suspended_staff_see_nothing_open` から `list_folder` を外す変異は、起こりうる組み合わせの答えを変え、確かめで落ちるものに移る。

数の行に W308 が付く材料を置けるのは、区間の中を切る変異を、新しい切れ目の近くの要求で比べるようにしてからである（2026-11-17。6.1）。数の行の W308 の変異（`tests/mutants/W308_a_numeric_line_that_leaves_out_no_combination.gate` と日本語の版。`when resource.amount >= 10_000JPY` のある permit に `when resource.amount > 1_000JPY` を書いた）では、17 の変異のうち 13 がどれかの確かめで落ち、4 つ（行を外す、`>` を `>=` に、1000 を 999 に、1001 に）が等しいと示せて、下の表に載る。前の決め方の試験では、行を外すもののほかの 3 つが、等しいとも示せない変異として試験を落とした。区間の中でだけ答えの変わる変異を等しいと言わないことは、`a_change_that_answers_otherwise_inside_a_cell_is_not_shown_to_answer_alike` が確かめる。`resource.amount > 1000` を `resource.amount != 50000` にしたものは、どの確かめにも落ちないが（50,000 は最後の区間の中にある）、近くの要求で元と違う答えを出す。`resource.amount != 5000` にしたもの（もう一つの行が、いつも外す額）は、等しいと示せる。近くの要求を作らずに比べると、この試験が落ちる。

| 材料 | 等しい変異 | 検査が言うこと |
|---|---|---|
| `tests/doc/services.gate` | `staff_read_and_purge` から `purge_doc` を外す | W306（`no_one_purges` が、`purge_doc` で許すはずの組み合わせを全部拒む） |
| 同 | `services_index` の `when { resource.open_to_all }` を外す | W307（外す組み合わせを `services_skip_closed_docs` が拒む） |
| 同 | `suspended_staff_see_nothing_open` から `list_folder` を外す | W305（フォルダーは `open_to_all` を持たない） |
| W305 の変異 | 同じ | W305 |
| `tests/doc/services.gate` | `suspended_do_nothing` から `index_doc` を外す | 言わない（`action any`） |
| 同 | `services_skip_closed_docs` から `read_doc`・`purge_doc`・`list_folder` を外す（3） | 言わない（`action any`） |
| 同 | `services_skip_closed_docs` の `unless` から `resource has open_to_all` を外す | 言わない（生成した `has`） |
| `tests/walk/given/given.gate` | `readers_of_a_band_read` の `unless { context.band == "unknown" }` を外すか、`"unknown"` を替える（2） | W308（`band` は `unknown` にならない） |
| `tests/walk/together/together.gate` | `staff_weigh_what_is_not_mixed` の `unless { context.kind == "mixed" }` を外すか、`"mixed"` を替える（2） | W308（`kind` は `mixed` にならない） |
| 同 | `staff_compare_what_is_on_one_side` の `when { context.side == "same" }` を外す | W308（`side` は `apart` にならない） |
| W306 の変異 | `staff_read_and_purge` から `purge_doc` を外す | W306 |
| W307 の変異 | `services_index` の `when { resource.open_to_all }` を外す | W307 |
| W308 の変異 | `clerks_refund_paid_orders` の `unless { resource.status == "refunded" }` を外すか、`"refunded"` を替える（2） | W308（`when resource.status is paid` が満たされるところでは、いつも満たされる） |
| W308 の数の変異 | `clerks_refund_large_orders` の `when { resource.amount > 1000 }` を外すか、`>` を `>=` にするか、1000 を 999 か 1001 にする（4） | W308（`when resource.amount >= 10_000JPY` が満たされるところでは、いつも満たされる） |

### 6.3 生成したコードのテスト（段階 C）

組み合わせごとに、生成したコードに渡す生の値（`Store` の中身、`input`、時刻）を作り、同じ生の値から sekisho の参照の評価で求めた答えと、生成したコードの答えを比べる（`src/raw.rs`、`tests/raw.rs`、`tests/typescript.rs`、`tests/python.rs`、`tests/go.rs`）。

- 生の値の作り方：検査が歩いた組み合わせの全部から作る。ポリシーが比べる数は、区間の両端で一つずつ（`vectors` と同じく、区間に二つ以上の値がある組み合わせは二件。名前も `vectors` と同じ `refund_order 3 (amount 1)`）。規則だけが読む値（係の上限）は、rulec がその区間で出力の値ごとに言う入力（`Rules::outputs_over`。rulec の vectors の、境目から作った入力）を先に、範囲の両端と真ん中と端の一つ内側をそのあとに試し、出力が組み合わせの値になる最初のものを使う。日付だけが読む値（注文の支払日）は範囲の両端と真ん中と端の一つ内側、`today` は範囲のすべての日から選び、日付の述語の真偽が組み合わせのとおりになる組のうち、`today` が述語の変わる日（期間の最後の日とその次の日）にいちばん近いものを使う。時刻は、その日の `today` のオフセットでの最初の秒か最後の秒を交互に使う。エンティティの ID は `vectors` と同じ形（`user1`、`order1`、`customer2`）で、同じエンティティを指す項は同じ ID である。
- 拒むはずの生の値も足す。action ごとに一つずつ壊したもの：`today` の範囲の前の日と次の日（`today` を読む action）、入力の範囲の下と上と列挙に無い値、`Store` に無い resource と principal、属性の範囲の外と列挙に無い値、型の持たない役割、action のとらない型の principal、宣言していないワークフロー（二つ以上の型をとる action では、とらない型の resource）。`today` を読まない action には、範囲の次の日を渡し、拒まずにほかの決まりのとおりに答えることを確かめる（2.9）。
- 参照の答えは、組み合わせの答えをそのまま使わず、生の値から求め直す（`raw::Model::evaluate`）。rulec と koyomi の参照の評価器を口で呼んで計算した値を求め、組み合わせの場所に当てはめて、検査と同じ形にしたポリシーで決める（`eval`）。拒む順と種類も 5.3 のとおりに言う。生成するコードが従う決まりは、この関数が一か所に書く。テストは、組み合わせから作った生の値の答えが、その組み合わせの答えとどれも同じであることも確かめる。
- 生成したコードは、言語ごとのハーネスが生の値（JSON）を読み、ケースごとに `Store` を作って `authorize<Action>` を呼び、答えを一件一行の JSON で出す。比べるのは、許すか、決めたポリシーの集合（並べ替えて比べる）、拒んだ種類、Cedar に渡した context（尋ねたとき）である。
- 例の英語の版は、1,078 通りから区間の両端で 2,134 件、拒むはずの 28 件、範囲の外の日に答える 2 件（`view_order` と `export_refunds`）の 2,164 件になる。検査を通る 35 のファイル（action の無い 7 つには件が無い）では 27,579 件である（段階 C を取り込んだときは、27 のファイルで 27,531 件だった）。生の値が見つからない組み合わせは、W303 の材料の 2 通り（整数では届かない値）だけである。2026-10-11 に `allowed` の例と材料（2.11）を足し、cedar-wasm、cedarpy、cedar-go のどれでも、30,204 件が参照の評価と一致した（足す前は 27,611 件）。足した分は、承認の例の二つの版が 132 通りから 154 件ずつ、文書の例の二つの版が 34 通りから 46 件ずつ、`tests/gen/asked.gate` が 1,376 通りから 2,193 件である。mypy の strict と go vet は 126 のパッケージに通り、pyright 1.1.414 の strict は、`allowed` のために書く行に誤りを言わない（6 のパッケージで 55 の誤りは、どれも前からある形で、使わない関数、要らない比べ、cedarpy の型）。
- TypeScript（`tests/typescript.rs`）：どのケースも二度渡す。値の無いことがある属性と action の入力を、データのとおり書かずに渡す回と、`null` にして渡す回である（31 章）。action のある 28 の `.gate` で、英語と日本語（`--lang ja`）と `--authorizer avp` のモジュールの全部が `tsc --strict`（5.9.3）を通り、Node 23.11.0 と cedar-wasm 4.13.0 で、27,579 件の全部が参照の評価と一致した（テストは 55 秒ほど）。名前がぶつかるゲートは `gen` が止める（24 章）。テストが誤りを見つけることも試した。生成するコードの日付の比べ方 `<=` を `<` にすると、例の英語の版の 2,164 件のうち 528 件が食い違った（期間の最後の日を使っているため）。
- Python と Go（`tests/python.rs`、`tests/go.rs`）：同じ 27,579 件で、cedarpy 4.12.1 と cedar-go v1.8.0 の答えが参照の評価と一致した（どちらも 50 秒ほど）。型の検査は、35 の `.gate` を `--authorizer cedar`、`--authorizer avp`、`--lang ja` の三通りで生成した 105 のパッケージで、Python は `mypy --strict`（2.4.0）、Go は gofmt と `go vet`（Go 1.25.5）を通した。Go のハーネスは、反射で生の値をレコードと入力の構造体に入れ、ゲートごとに `Store` のメソッドを書く。
- 規則と日付の生成物は、テストの中で rulec と koyomi の生成の関数（`rulec::codegen::package_module`、koyomi の言語ごとの `module`）で作り、`ritsu gen` のパッケージと同じ形に並べる。ランナーは `tools/runner-ts/`（`package-lock.json`）、`tools/runner-py/`（依存までハッシュつきで固定した `requirements.txt`。`install.sh` が uv で `.venv` を作る）、`tools/runner-go/`（`go.mod` と `go.sum`。`install.sh` が `go mod download`）で、Go のテストはモジュールのキャッシュだけでビルドする（`GOPROXY=off`）。CI は `tools.yml` の「the others」の組で三つを入れる。
- `--authorizer avp` のコードは、型の検査だけをする（ネットワークを使わない。本物の Verified Permissions で走らせて確かめることは、まだしていない。15 章）。
- 英語の版と日本語の版の双子：`refunds.gate` と `refunds.ja.gate` から生成した Cedar が、名前空間、ID の頭、`@name` と `@doc` のほかは同じであること、`vectors` の答えが同じであること。生の値も、ゲートの名前とポリシーの ID の頭のほかは同じである（`tests/raw.rs`）。

### 6.4 捨てた案

- **`tools/` の下の小さな Rust のプロジェクト（ワークスペースの外）で `cedar-policy` を呼ぶ**：CLI と同じ評価器を、テストのたびに数百のクレートごとビルドすることになる。CLI のバイナリは、Cedar の読み手と書き手（`ritsu_base::cedar`）のテストも同じものを使う。
- **cedar-wasm だけで確かめる**：段階 B の確かめ（生成した Cedar）に Node を要ることになる。cedar-wasm は段階 C の TypeScript の生成物のテストで使う。
- **SymCC で生成した Cedar と sekisho の表が同じかを確かめる**：cvc5 が要り、ネットワークも要る（取ってくるとき）。一つ一つのリクエストの答えを比べるには、`run-tests` で足りる。

### 6.5 手で書いた Cedar の答えを、公式の CLI で確かめる（2026-10-07）

**決め**：口が手で書いた Cedar に答えるとき（1.3）に sekisho が数える組み合わせの全部を、`cedar run-tests` のテストにして公式の CLI 4.13.0 にかけ、許すか拒むかと決めたポリシーの集合が、組み合わせごとに一致することを確かめる。決めたポリシーは、6.1 と同じく、`run-tests` に加えて、ポリシーを一つずつ permit にして流して確かめる。

- 組み合わせごとの答え：`cedar_in::cases` が、問い（action と尋ねる側）の組み合わせのそれぞれについて、許すか、決めたポリシー、action に当たるポリシーのそれぞれが成り立つか、成り立たないか、エラーかを返す。決め方は Cedar の authorizer と同じで、成り立つ forbid があれば拒み（決めたポリシーは、成り立つ forbid の全部）、なければ、成り立つ permit があれば許し（成り立つ permit の全部）、どちらも無ければ拒む（決めたポリシーは無い）。エラーになるポリシーは成り立たず、`num_errors` に数える。口の `allowed` は、同じ数え方の上で、許す組み合わせと拒む組み合わせを一つずつ探す。
- リクエストとエンティティの組み立て：同じ関数が、組み合わせを `run-tests` のテストにする。
  - ID は、型の名前を小文字にして番号を付ける（`user1`、`order1`）。ポリシーが書く ID とはぶつけない。同じエンティティとした項は同じ ID で、principal と同じなら principal の ID である。
  - ポリシーが比べるどの文字列でもない値は、`other1`、`other2` のように作る。同じ文字列とした属性どうしは、同じ値にする。
  - 整数は、定数で切った区間の両端を一つずつ使う（6.1 と同じ考え）。端の開いた区間は、i64 の最小と最大を使う。例の `context.amount` は 50 と比べるので、区間は 49 以下、50、51 以上の三つで、使う値は -9223372036854775808 と 49、50、51 と 9223372036854775807 である。区間に二つ以上の値がある組み合わせは、低い端と高い端の二件にする。
  - エンティティは、principal、resource、役割とグループ、属性が指すエンティティのうち属性を持たない型のものである。principal の親は、尋ねる側の役割のうちスキーマがその型を入れられるものと、組み合わせがメンバーとする属性のエンティティで、スキーマが親の親を通してしか入れないときは、間のエンティティを作る。resource の親は、組み合わせが入るとするエンティティである。組み合わせが値を決めない属性は、スキーマが要るものだけを、最初の値（偽、0、空の文字列、作ったエンティティ）で書く。
  - 尋ねる側は、action がとる principal の型のそれぞれに、その action のポリシーが principal を入れる役割のどの組も持たせる。型 `Workflow` には、ポリシーが書くワークフローの名前と、書かない名前を一つ置く。
- CLI のかけ方：まず、ポリシーの集合の全部で `run-tests` を流す。ポリシーが strict の検証を通るならスキーマつき、通らないならスキーマなしで流す（`run-tests` はスキーマを渡すとテストのたびにポリシーを検証し、通らなければどのテストもエラーにする）。次に、action に当たるポリシーを一つずつ permit にして流し、sekisho がそのポリシーを成り立つとした組み合わせだけで許し、エラーとした組み合わせでは `num_errors` を 1 にする。一つずつ流すほうは、スキーマなしで流す。リクエストとエンティティがスキーマに合うことは、全部で流す回が確かめている。action のグループの無いスキーマでは、スキーマなしでも答えは変わらず、速い（例のゲートの 2,134 件で、スキーマつきが 0.34 秒、なしが 0.14 秒）。action のグループがあるスキーマでは、スキーマつきで流す（グループはスキーマから来るため）。`@id` の無いポリシーは、一つだけで流すと CLI が `policy0` と呼ぶので、集合での ID を `@id` として付けて流す。
- 決められない問い：action に当たるポリシーのうち、数える条件にならない部分（`like`、算術、メソッド、関数、集合、レコード、テンプレートの穴）を持つ最初のポリシーの、Cedar が読む順で最初のその部分を、理由が挙げることを確かめる。そういう部分を持つポリシーが無い action の問いは、どれも決まることも確かめる。
- 確かめる対象（`tests/cedar_in.rs`。CLI を使う三つのテストは、CLI が無ければそれぞれ SKIP の行を一つ出す。CLI の呼び方と、Cedar を一か所ずつ変える関数は、`tests/cedar.rs` と共に `tests/common/cedar_cli.rs` に置いた）：
  1. 材料の三組と、その日本語の版、小さな十二の組（1.3）。
  2. 例とテストの材料の `.gate` のうち、検査を通る 35 のファイルから生成した Cedar を、手で書いたものとして読ませたもの。生成する Cedar は数える部分に入るので、どの問いも決まることも確かめる。
  3. 材料の英語の三組を、一か所ずつ変えたもの（6.1 の変え方：数を一つずらす、文字列や役割を替える、演算子を替える、`has` や `!` を外す、`when` と `unless` を替えるか条件を外す、forbid と permit を替える、スコープを替える、ポリシーを外す）。変えたポリシーが当たる action（変える前と後の両方）の問いを流す。ほかの action の問いは、材料のものと同じだからである。日本語の版は、名前のほかは英語の版と同じ Cedar なので、まとめる。変えたポリシーで、材料のテストを CLI がどう答えるかも数える（答えの変わるもの、決められない問いを決めるもの、決められないままのもの、材料のテストに同じ答えのもの）。

走らせた結果（CLI 4.13.0、2026-10-07）：

| 対象 | 問い | 組み合わせ（許すもの） | テスト | 結果 |
|---|---|---|---|---|
| `refunds`（日本語の版も同じ数） | 28 | 3,692（344） | 6,140 | 全部一致。10 のポリシーとも一つずつ流して一致 |
| `outside`（同） | 5 | 6（2） | 6 | 全部一致。`print` の 2 問は、`drafts_are_not_printed` の `like`（英語の版で 19 行 8 列、日本語の版で 21 行 8 列）で決められない |
| `documents`（同） | 32 | 1,982（284） | 2,894 | 全部一致。13 のポリシーとも一致 |
| 小さな十二の組 | 22 | 30（10） | 31 | 全部一致 |
| 生成した Cedar（action のある 28 のファイル） | 328 | 41,353（4,102） | 68,753 | どの問いも決まり、全部一致 |
| 変異 223（`refunds` 97、`outside` 28、`documents` 98） | — | — | — | 176 は検証を通ってスキーマつきで、47 はスキーマなしで、全部一致 |

2026-10-11 に `allowed` の例と材料（2.11）を足したあとも、生成した Cedar を手で書いたものとして読ませる確かめは、どの問いも決まり、全部一致する（承認の例は 6 問で 26 通り、文書の例は 6 問で 34 通り、`tests/gen/asked.gate` は 10 問で 664 通り）。手で書いた Cedar を読む口には、`context.allowed_approve` は context の真偽の一つに見える。

変異のうち、材料のテストに同じ答えのものは四つあった。`managers_refund_late_on_business_days` から `unless { context.in_period }` を外したもの（期間内の営業日に決めたポリシーが一つ増えるだけで、`run-tests` は多いものを見ない。6.2 と同じ）と、`outside` の `print` のポリシーを `read` か `archive` に付け替えたもの三つ（材料の `read` と `archive` のテストでは、付け替えたポリシーが当てはまらない。作った文書の題に draft が無く、`archive` を尋ねる側は reader を持たない）である。`outside` の 10 の変異は、`print` の問いが決められないままである。テストの三つは、合わせて 90 秒ほどかかる（スレッドで並べて）。

この確かめを作るときに、替える前の数え方と比べた（材料、生成した Cedar、英語の材料の変異の 6,225 問）。答えが違った 3,291 問は、次のとおりである。

- 968 問は、前の答えが誤りだった。原因は四つで、その型が入れない役割、文書の持ち主やチームのメンバーとしたボット、二つのポリシーに書いた同じエンティティ、フォルダに入ったテンプレートである（どれも 1.3 に書いた誤り）。前の答えが「いつも」か「だれも」だった 840 問は、いまの答えのうち反対側の例を CLI にかけ、CLI がいまの答えのとおりに決めた。前の答えが「ときどき」だった 128 問は、前が挙げた例のうち、いまは無いと言う側の例が、CLI に受け付けられない（エンティティがスキーマに合わない）か、CLI が逆に決めることを確かめた。
- 2,311 問は、前は決められず、いまは決まる（二つの属性の比べと、どの型も宣言しない属性を読む式）。いまの答えの例 4,142 件を CLI にかけ、全部がいまの答えのとおりだった。
- 12 問は、どちらも決められず、理由に挙げる部分が、比べる式から中の算術に変わった。

**捨てた案**：

- **範囲を小さく決めた具体的なリクエストを全部作って CLI に流し、数えなかった組み合わせが無いことを確かめる**：数える関数とは別に、もう一つの組み立てを書くことになる。CLI の確かめは、数えた組み合わせの一つ一つが正しいことを言い、数えなかったリクエストが無いことは、`.gate` と同じく数え方の議論が受け持つ（4.1）。整数の区間の両端と、小さな組の答えで、その一部を確かめる（15 章）。
- **一つずつ流す回もスキーマつきにする**：答えは変わらず、2 倍あまり遅い。

## 7. 人が読むページ（`sekisho doc`）

`sekisho doc <file.gate> [--format markdown|html] [--root <dir>] [--lang en|ja]`。段階 D で作った（`src/doc/`。ページの中身を組み立てる `mod.rs`、書き出す `markdown.rs` と `html.rs`）。`api` と `vectors` と同じく、検査を通るファイルにだけページを出す（通らなければ、`check` と同じ診断を出して exit 1）。表は全部の組み合わせを歩いた結果から作るので、歩けないファイルには表が無いからである。HTML は ritsu の土台の枠（`ritsu_base::docpage` の `html_head`、`Palette`、`HTML_TAIL`）で、外のファイルを読まない一枚のページ、Markdown は GitHub がそのまま表示するもの（ritsu の DESIGN 4.8）。どちらも英語と日本語で書ける。

ページの中身は、次の順に並べる。

1. **頭**：`.gate` のパスとハッシュ（SHA-256 の先頭 16 桁）、名前空間、sekisho の版。パスは、参照と同じくルートから書く（2.6。`--root` は `check` と同じに取る）。Markdown は、ほかの言語と同じく頭のコメント（`<!-- Generated by sekisho 0.23.0 from refunds.gate (sha256:678573b010ccf8c4). … -->`）から始める。そのあとに、このファイルから `sekisho gen --target cedar --lang <ページの言語>` が書く四つのファイルと、それぞれのハッシュの表を置く。言語を書くのは、sekisho が書く文（頭の二行目と、計算した値などの `@doc`）が `--lang` で変わるからである（5 章）。最後に、検査が数えた組み合わせの数と、期待、職務の分離、`can` の結果を短くまとめる。
2. **action ごとの表**：action ごとに、守る操作、principal と resource の型、`input`、計算した値、組み合わせの数と許す数を並べ、表を「許す組み合わせ」と「拒む組み合わせ」に分ける。行の合わせ方は `src/table.rs`（段階 A）のとおりで、答えと決めたポリシーが同じで、一つの列だけが違う行を、その列の値の集合にまとめ、変わらなくなるまで繰り返す。集合が全部の値なら any（「どれでも」）、一つを除く全部なら not …（「〜以外」）と書く。その列が当てはまらない行（「-」。無い属性から計算する値など）は、その列に値のある行とは合わせない。合わせると、値の集合の中で「-」が見えなくなり、その組み合わせが表から読めなくなるからである。表を二つに分けたので、答えの列は持たない。例の `refund_order` の 1,056 通りは、許す行が 5 行、拒む行が 21 行になる。許す 5 行は次のとおり（英語の版の英語のページ。`sekisho doc` の実物）。

   | principal | clerk | manager | auditor | workflow | suspended | status | amount | refund_band | in_period | business_day | Deciding policies |
   |---|---|---|---|---|---|---|---|---|---|---|---|
   | User | yes | no | no | - | no | not refunded | any | within_limit | yes | any | clerks_refund_within_their_limit |
   | User | yes | yes | no | - | no | not refunded | any | within_limit | yes | any | clerks_refund_within_their_limit, managers_refund_in_period |
   | User | yes | yes | no | - | no | not refunded | any | over_limit | yes | any | managers_refund_in_period |
   | User | yes | yes | no | - | no | not refunded | any | any | no | yes | managers_refund_late_on_business_days |
   | Workflow | - | - | - | returns | - | returned | <=50GBP | - | any | any | returns_refunds_returned_orders |

   列は principal の型、持つ役割（`includes` をたどったもの）、ワークフロー、属性、`input`、計算した値である。設計の段階の表に無かった列は「ワークフロー」で、`principal is workflow returns` を読むので、ワークフローの名前を列にする。規則の答えは、`.gate` と並べて読めるよう、`.rule` に書いた名前で出す（日本語の版では `上限まで`・`上限超え`）。Cedar に渡す文字列（`.rule` に書いた別名、`within_limit`）は、3. のポリシーの生成した Cedar に出る。

   拒む組み合わせは、forbid ごとの条件と数を先に見せ、表は畳んだ節（`<details>`）に入れる。forbid が重なる組み合わせが多いからである。節の見出しは、行の数で「The 21 rows that deny」「拒む行の表（21 行）」と書く（一行なら「The 1 row that denies」。2026-10-07 に、一行のときの英語の動詞の誤りを直した）。例の `refund_order` では次のようになる（日本語の版の日本語のページ）。

   - `二度は返金しない`（`when resource.状態 is 返金済`）は 264 通りに当てはまり、どれも拒みます。そのうち 112 通りには permit も当てはまりますが、forbid が勝ちます。
   - `監査は返金しない`（`principal in 監査`）は 512 通りに当てはまり、どれも拒みます。そのうち 224 通りには permit も当てはまりますが、forbid が勝ちます。
   - `停止中の職員は何もできない`（`principal is 職員`、`when principal.停止中`）は 512 通りに当てはまり、どれも拒みます。そのうち 224 通りには permit も当てはまりますが、forbid が勝ちます。
   - 残る 128 通りは、どの permit も当てはまらないので拒みます。

   並べるのは、その action の組み合わせに一つでも当てはまる forbid だけである。一つも当てはまらない forbid（`principal is サービス` と書いた forbid は、利用者だけが使う action には当てはまらない）は、ここには出さず、3. のポリシーの節で、なぜ当てはまらないかを言う。拒まない forbid を「0 通りに当てはまり、どれも拒みます」と並べると、読み手はその action にも効いていると読むからである。文は数で言い分ける。当てはまるのが 1 通りなら「1 通りに当てはまり、それを拒みます」、permit がどれにも当てはまらなければ「permit は、どれにも当てはまりません」、どれにも当てはまれば「どれにも permit も当てはまりますが、forbid が勝ちます」、一部なら上の例の形である。forbid を一つも並べない action では、最後の行は「残る」を付けずに「1 通りは、どの permit も当てはまらないので拒みます」と書く。組み合わせを一つも許さない action は、数の行を「組み合わせは 8 通りで、どれも許しません」と書く。

   ほかの言語が起こるかどうかを言えなかった値を数えた action（4.1 の印のある組み合わせ）には、表の前に、その組み合わせが表に入っていることを注で書く（W303 の変異のページ）。そのために決められなかった答えがあるとき（W303 があるとき）だけ、注は「検査の警告」の節を指す。警告が無ければその節はページに無いので、指さない。ページの注がページの中の節を指すのは、その節があるときだけである（`tests/doc.rs` が、どのページでも、指した節の見出しがあることを確かめる）。

   ポリシーが `allowed` で読む答えは列になる（`allowed approve`。2.11）。action の箇条には、その値が、同じ principal と resource でその action を尋ねたらどうなるかで、このページのその action の表のとおりであることを書く（「Reads `allowed approve(amount: amount)`: whether `approve` is allowed for the same principal and resource, as the table of `approve` on this page says」）。尋ねる action のポリシーが読む属性と関係も列になる（`request_approval` の表の `resource.requester is principal`）。尋ねる action の計算した値は列にしない。その action の表にあり、答えの列があれば行が読めるからである。期待だけが読む `allowed` も、検査が数えるので列になる。箇条には「An expectation reads …」と書き、Cedar には渡さないことを言う（`tests/golden/doc/asked.en.md` の `escalate`）。
3. **ポリシー**：ポリシーごとに、`.gate` に書いた行と、生成した Cedar を並べる（HTML では横に並べ、狭い画面では縦に積む）。生成した Cedar は、`cedar::policies` の一つずつを `ritsu_base::cedar::write_policy` で書き、`format_policies`（幅 80、字下げ 2）で整えたもので、`.cedar` のファイルの中のそのポリシーと一字も違わない（テストが確かめる）。ポリシーが action ごとに許す（拒む）組み合わせの数と、そのポリシーだけが決める数（4.3 の数）を書く。その action のどの組み合わせにも当てはまらないポリシーは、数の代わりに理由を書く。`principal` の行をその action の principal の型が満たさないとき（「`文書を読む` の principal（`利用者`）は、`principal is サービス` の行を満たしません」）、条件の行が一つで、それがどの組み合わせでも満たされないとき、`principal` の行は満たされるが `when` と `unless` の行がそこで同時に満たされないとき、の三つを言い分ける（`principal` の行が当たる組み合わせの数は、歩くときにポリシーごとに数えておく）。当てはまるのに、forbid がどれも拒むので何も許さない permit は、「4 通りに当てはまりますが、forbid がどれも拒むので、許す組み合わせはありません」と書く。forbid の文は、2. と同じ数の言い分けをする。`use gate` で読んだ forbid は、それを書いたファイルの行を見せ、読んだファイルを書く。規則の答えを比べる条件があれば、`.gate` の行では `.rule` に書いた名前（`上限まで`）、生成した Cedar では別名の文字列（`"within_limit"`）になることを、節の初めに一文で書く。二つを並べたここで対応が読める。
4. **計算した値**：値ごとに、書いた式、何から計算するか（参照の書き方で `rulec "rules/refund_limit.rule" output band`、`koyomi "dates/refund_terms.cal" date last_day`、`koyomi "calendars/england_and_wales.cal"`）、とりうる値（規則の列挙なら、`.rule` に書いた名前と、Cedar に渡す文字列）、計算する型（action のすべての型でないとき）、`today` を読むなら UTC からのオフセットを書く。同じデータを読む日付の値（`in_period` と `business_day`）は、koyomi が数えた、起こりうる値の組を一文で書く（3.3）。rulec の規則のページは、dandori と同じく `rulec doc` が描いたものを埋め込む（口 `Rules::doc`）。koyomi の日付とカレンダーも同じ形で埋め込む（口 `Dates::doc`）。Markdown では畳んだ節に入れ、HTML ではボタンを押すとページの上に開く。開く枠は `<iframe sandbox="allow-scripts">` の `srcdoc` で、埋め込んだページのスクリプト（rulec のページでケースを試すもの）は動くが、sekisho のページには届かない。ページは `<script type="application/json">` に JSON で持ち、JSON の文字列の中の `<` は `\u003c` と書く（埋め込んだページの文が要素を閉じないように）。同じファイルを読む値が二つあれば、ページは最初の値のところに一度だけ載せる。`#page=<n>`（1 から）を付けて開くと、そのページを開いた状態で読み込む。ポリシーが読む `allowed` の値も、ここに節を持つ。書き方、同じ principal と resource と日で尋ねること、拒まれたときはこの action も拒むこと、尋ねる action の入力に渡すもの（`amount` ← `amount`）、値（はいといいえ）と、値がこのページのその action の表のとおりであることを書く。
5. **期待、職務の分離、役割**：期待ごとに、期待する答え、action、`description`、選ぶ組み合わせの数と結果。職務の分離ごとに、結果。役割ごとに、`description`、`includes`、`can`。それから、役割を一つだけ持つ principal（役割を持たない型は、その型の principal）と action の表（いつも許す、組み合わせによる、許さない。action が取らない型は「-」）。表の答えは `checks::allowance`（口 `Gates::allowed` と同じ数え方）で求める。例では次のようになる（英語の版の英語のページ）。

   | principal | `view_order` | `refund_order` | `export_refunds` |
   |---|---|---|---|
   | `User` holding `clerk` | sometimes | sometimes | never |
   | `User` holding `manager` | sometimes | sometimes | never |
   | `User` holding `auditor` | sometimes | never | sometimes |
   | `Customer` | sometimes | - | - |

   停止中の User はどの action も拒まれるので、役割を持つ User は、許される action でも「組み合わせによる」になる。期待も職務の分離も無いファイルでは、この節は「役割」だけになる。
6. **ワークフロー**：`workflow` ごとに、`description`、`.flow` の参照、`Workflow` を取る action ごとの答えと数（例では `sometimes (4 of 32 combinations allowed)`）。フローが呼ぶ操作（dandori の口 `Flows::operation_calls` の答え）を受け取ったときは、呼び出しごとに、行、タスク、操作の参照、それを守る action、そのワークフローがその action を許されるか、タスクが宣言した、拒まれたときのエラーを表にする。`ritsu check` が X16 で確かめることを、ページの上で読めるようにするためである。答えは呼び出しごとのもの（`Gates::allowed_at` と同じ数え方）で、表の下に、届く実行がそこで持つ値を並べる。ワークフローの下の行（2.12）があれば、節の中に、ゲートがフローについて言っていることの表と、だれも確かめていない前提であることの注を出し、その行に頼った答えには印を付ける。ページの関数は、この答えを引数（`doc::FlowCalls`）で受け取り、`ritsu sekisho doc` はフローごとに dandori の口に尋ねる（`doc::calls_of`。ルートと、ルートからのフローのパスと、規則と日付と帳簿の口を渡す）。守る action は、呼ぶ操作の参照と同じ参照を守る action である。dandori を持たない sekisho のクレートのバイナリでは、この表を出さない。例のワークフロー `returns` では次のようになる（英語の版の英語のページ）。

   | Line | Task | Operation | Guarded by | Allowed | Error on a denial |
   |---|---|---|---|---|---|
   | 22 | `refund_order` | `openapi "api/orders.json" operation refundOrder` | `refund_order` | sometimes | `denied` |
7. **守る操作**：action ごとに、守る操作の参照と呼び出し方（OpenAPI の操作の方法とパス、AsyncAPI の `send`・`receive` とチャネルのアドレス、proto の `/<package>.<Service>/<Method>`、帳簿の振替と操作）。契約の操作を一つも守らない action は、最後に並べる。
8. **検査の警告**：ファイルが検査を通っていても、`sekisho check` が警告を言うなら（W301〜W308）、その文をそのまま載せる。

**テスト**（`tests/doc.rs`）：例の二つの版を、それぞれ英語と日本語の Markdown と自分の言語の HTML で、テストの材料の代表（条件の形を並べた `tests/gen/conditions.gate` とその日本語の版、`use gate` の forbid を読む `papers.gate`、二つの操作を守る `guards.gate`、W303 と W304 の変異の英語と日本語の版、action によって当てはまったり当てはまらなかったりするポリシーを並べた `tests/doc/services.gate` とその日本語の版、同じデータを読む値を並べた `tests/walk/together/` の二つとその日本語の版）を Markdown で、golden（`tests/golden/doc/`）と突き合わせる。2026-10-11 に、`allowed` の例（承認の英語の版と日本語の版、文書の英語の版）と、尋ね方を並べた `tests/gen/asked.gate` を足した。golden では、埋め込んだページを、それを指す一行に置き換え、埋め込んだページそのものは、rulec と koyomi の口が今描くものと一字も違わないことを別のテストで確かめる。頭の、生成する Cedar のハッシュも同じく、golden では `<sha256>` と書き、`cedar::files` が今書くファイルのハッシュと同じであることを別のテストで確かめる。rulec と koyomi のページの文や、Cedar の生成器が書く `@doc` が変わっても、sekisho のページの golden を取り直さずに済むようにするためである。ほかに、HTML とそれが埋め込むページが外のファイルを読まないこと、`.gate` の説明に書いた `</script>` などがページの中で文字のままであること、ポリシーごとの Cedar が `.cedar` のファイルの中のものと同じであること、フローが呼ぶ操作の表、検査を通らないファイル（exit 1）と E209（exit 2）、Chrome で描けて、埋め込んだページがボタンと `#page=<n>` の両方で開くこと（ritsu-testkit の Chrome を探す関数。見つからなければ SKIP）を確かめる。`ritsu sekisho doc` は、ritsu のクレートの `tests/sekisho.rs` が確かめる。

## 8. ほかの言語とのつながり（口）

### 8.1 読むもの

| 相手 | 口と問い | 使うところ |
|---|---|---|
| rulec | `Rules::facts`（入力と出力の型と範囲、列挙、生成物の呼び方。`import enum` が読む `enums` ファイルの列挙もここから）、`preconditions_hold`、`eval`、`doc`、`outputs_over`、`outputs_together` | 計算した値の型と範囲（E105、E206）、とりうる値と、一緒にとる組（4.1）、参照の評価、ページ |
| koyomi | `Dates::facts`、`eval`、`values`、`calendar` と `doc`（段階 A で足した） | 日付の述語の型と範囲（E206、E207）、起こりうる真偽の組（4.1）、ページ |
| 契約 | OpenAPI と AsyncAPI の文書（`ritsu_base::openapi`。8.6）、`.proto`（ritsu-proto） | `guards`、`input`、`from`（E202〜E204） |
| dandori | `Flows::crossings`（`.flow` が dandori の検査を通るか。通らなければ dandori が言うこと）、`Items`（フローが持つもの） | `workflow`（E208） |
| ritsu-cross に渡すもの | `Gates`（8.3） | X15、X16 |

sekisho が読む口は、口のまとまり `GatePorts`（`ritsu_ports`。rules・dates・books・flows・items）で受け取る。`ritsu sekisho` と `ritsu check` には `ritsu-project` の `Joined::sekisho()` が渡し（sekisho の `Suite` は `From<GatePorts>` で作る）、sekisho のクレートのバイナリは何もつながない口を持つ。`.flow` が検査を通るかを `Items` で尋ねないのは、dandori の `Items` が検査をせずに読む（規則が読めないフローでもタスクを返す）からである。`GatePorts::ports()` が、`crossings` に渡す規則と日付と帳簿の口の組を返す。

sekisho は受け取る側の言語である（dandori、yuen、sakai と同じ）。sekisho のクレートのバイナリはほかの言語を持たないので、`use` のあるファイルは、`ritsu sekisho` で走らせるよう言う診断（E209。dandori の E018 と同じ形で exit 2）を出す。`ritsu-project` の読む順（ritsu の DESIGN 6.1 の `ORDER`）では、rulec、koyomi、chobo、geas、`.proto`、dandori のあと、yuen と sakai の前に置く。

### 8.2 出すもの

- **`Gates`**（新しい口。8.3）：action と守る操作、ワークフロー、ポリシーと期待の一覧、ある principal がある action をどれだけ許されるか。答えるのは sekisho の `Engine`（`src/ports.rs`）で、`.gate` と、`.cedar` と `.cedarschema` の組（1.3）の両方に答える。
- **`Items`**：種類 `principal`（下に `attribute`）、`resource`（下に `attribute`）、`role`、`workflow`、`enum`（下に `value`）、`action`（下に `input`、`context`）、`policy`、`expect`、`separate`。定義の文は、宣言の塊の行（コメントと前後の空白を除き、続いた空白を一つにし、字下げを深さごとに空白二つに直したもの。dandori と同じ。ritsu の DESIGN 3.2）。
- **`References`**：`use rule|dates|calendar|openapi|proto|asyncapi|book|gate` の先、`import enum` の先（`rulec "<ファイル>" enum <列挙>`）、`workflow … from` の先、`guards` の先（操作）。sakai が境界を越える参照として確かめ、yuen が `trace` でたどる。どれもルートからのパスの参照で、ルートの外のファイルは参照にしない（その `.gate` は E201 か E208 になる。2.6）。`guards` の先は検査が見つけた操作（`Action::references`）なので、名前の検査を通らないファイルと、見つからない操作（E202）は出さない。行は、`use` と `workflow` と `guards` の行。手で書いた Cedar の組には、スキーマの `@guards` の参照を、action の行で出す（ポリシーのファイルは何も参照しない）。

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
    /// The action's `input`s, in the order written (none for a Cedar pair, whose schema writes
    /// neither a unit nor a range of its context). X16 compares them with what a call gives; X18
    /// stops an operation of a ticket that asks an action taking any (E911).
    pub inputs: Vec<GateInput>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GateInput {
    /// Its name and alias: a call gives it the argument named either way.
    pub name: String,
    pub alias: String,
    /// What it holds, as the port of `fed by` says it (26 章): a number with the type as written,
    /// its unit and its range.
    pub holds: ritsu_ports::Carried,
    pub line: usize,
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
/// Each combination of values the outputs `outputs` can come to in one call, the inputs held as
/// `outputs_over` takes them, each with an input that reaches it; undecided with the reason when a
/// combination the analysis cannot rule out has no input found to reach it.
fn outputs_together(&self, rule: &Path, outputs: &[String], ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<Found<Vec<(Vec<Value>, Values)>>, Vec<Said>> { … Undecided … }

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
- `Rules::outputs_together` は、同じ決まりで、出力の組を返す。組は `outputs` の順の値の並びで、添える入力は一つ。出力を決める表ごとに、届きうる行が書く組を取り、二つの表のあいだはすべての組を候補にして、範囲の中のベクタを評価器にかけた組と突き合わせる。合えば正確で、入力の見つからない候補があれば、その組を言って決められないと答える（rulec の DESIGN 15.315）。
- `ritsu_ports::EnumValue` に `public` を足した（`name` は規則の名前、`alias` は生成したコードの名前で、どちらも前のまま）。
- `Dates::calendar` と `Dates::doc` には既定の実装がある（答えない。`Err` で「この口はカレンダーを読みません」「この口はページを作りません」）。`Rules::outputs_over` と `Rules::outputs_together` の既定は決められない。いま口を実装しているもの（dandori の `NoRules`・`NoDates`）は直していない。
- `Flows::operation_calls` と `Maps::published_operations` は、まだ足していない（段階 D の X15 と X16 で足す）。

段階 D で作った形（2026-10-06）：

```rust
pub trait Gates {
    fn facts(&self, root: &Path, file: &str) -> Result<GateFacts, Vec<Said>>;
    fn allowed(&self, root: &Path, file: &str, action: &str, asker: &Asker) -> Result<Found<Allowance>, Vec<Said>>;
    fn joined(&self) -> bool { true }
}
pub struct GateWorkflow { pub name: String, pub alias: String, pub flow: ritsu_base::naming::Name, pub line: usize }

// Flows（dandori が答える）
fn operation_calls(&self, root: &Path, file: &str, ports: &Ports) -> Result<(String, Vec<OperationCall>), Vec<Said>> { Ok((String::new(), Vec::new())) }
// Maps（sakai が答える）
fn published_operations(&self, root: &Path, map: &str) -> Result<Vec<PublishedOperation>, Vec<Said>> { Ok(Vec::new()) }
```

- 問いは、ファイルをルートとルートからのパスで受け取る（`Items` と `References` と同じ）。答えの参照がルートからのパスなので、問う側も同じ形で渡す。上の計画の `&Path` の一つの引数からは、そこを変えた。
- `GateWorkflow` には、別名（Cedar の `Workflow::"…"` の ID。`Gates::allowed` の `Asker::Workflow` はこれで尋ねる）を足し、`.flow` はルートからの参照（`dandori "flows/returns.flow"`）にした。
- `Flows::operation_calls`（dandori の `src/ports.rs`）：dandori の検査を通るフローだけに答える（規則と日付と帳簿は `ports` で読む）。タスクの呼び出しのうち、`use openapi` の `http` と `use proto` の `connect` のタスクのものを、行の順に返す。操作は、`ritsu_base::openapi` で文書を読んで `operationId` を引き（無ければ方法とパス）、proto はサービスの最後の名前とメソッドで書く。拒まれたときのエラーは、タスクが宣言したエラーのうち状態が 403 のもの（Connect の `permission_denied` も dandori は 403 に読む）。帳簿の振替のタスク（`book`）は返さない（3.4）。テストは dandori の `tests/ports.rs` の `a_flow_says_which_operations_it_calls`（例の `hotel` と `fulfillment`、一時ディレクトリのフロー、検査を通らないフロー）。
- `Maps::published_operations`（sakai の `src/ports.rs`）：sakai の段 1 と段 2 を通る地図に答える。コンテキストの `open host service` に並べた名前ごとに、proto のサービスならメソッドの全部、OpenAPI の文書の操作（webhook でないもの）ならその操作を返す。`open_to_anyone` は、操作の `security: []`。AsyncAPI のチャネルと、規則の Connect のサービスは返さない（X15 が求めないもので、`guards` で守るものでもない。4.6）。`file` と `line` は、その名前を並べた context のファイルと行。テストは sakai の `tests/maps.rs` で、四つの例の golden の「published operations」の節。
- sekisho の `Engine` が `Gates`、`Items`、`References` に答え、同じファイルの検査の結果を、一つの実行の中で（ルートとファイルごとに）一度だけ持つ。`ritsu check` も `.gate` の検査をこの `Engine` で走らせるので、口に答えるときに検査をやり直さない。`Items` と `References` の `use` と `workflow` の行は構文だけから読み、`guards` の先だけが検査の結果を使う。

**2026-11-15 に足したもの**（27 章）。X16 が、呼び出しに届く実行が持つ値で尋ねるためのものである。

```rust
// flows.rs：OperationCall に足したフィールド
pub args: Vec<(String, FlowValue)>,   // 渡す引数（タスクの引数の名前と値）
pub reached: Option<Reached>,         // 届く実行が持つもの。dandori がたどらない呼び出しは None

pub struct FlowValue { pub shown: String, pub path: Option<String>, pub holds: Carried, pub constant: Option<bool> }
pub struct Reached { pub held: Vec<(String, Vec<String>)>, pub answers: Vec<(String, Vec<AnswerCall>)>, pub set: Vec<String> }
pub struct AnswerCall { pub line: usize, pub rule: PathBuf, pub inputs: Vec<(String, FlowValue)> }

// trait Flows（どちらも既定は「答えない」）
fn value(&self, root: &Path, file: &str, ports: &Ports, path: &str) -> Result<Option<FlowValue>, Vec<Said>>;
fn value_names(&self, root: &Path, file: &str, ports: &Ports, under: &str) -> Result<Vec<String>, Vec<Said>>;

// gates.rs
pub struct GateHolds { pub resource: String, pub id: String, pub line: usize, pub attributes: Vec<(String, String, usize)> }
// GateWorkflow に足したフィールド
pub holds: Vec<GateHolds>,

pub struct AllowedAt {
    pub allowance: Found<Allowance>,
    pub held: Vec<Text>,              // 答えを絞ったもの（ゲートの語と、フローの語）
    pub rests_on: Vec<(Text, usize)>, // 答えが頼ったワークフローの下の行と、その行番号
    pub unheld: Vec<Text>,            // 届く実行が持つのに、数えられなかったものと理由
    pub unrelated: Vec<String>,       // 答えが読む属性のうち、どの行も何も言っていないもの
}

// trait Gates（既定は Gates::allowed の答えで、フローの値には頼らない）
fn allowed_at(&self, root: &Path, file: &str, action: &str, workflow: &str, call: &OperationCall) -> Result<AllowedAt, Vec<Said>>;

// 属性の値を決めて尋ねる問い（既定は「決められない」）
pub struct GateGiven { pub of_resource: bool, pub attribute: String, pub value: GateGivenValue }
pub enum GateGivenValue { Bool(bool), Enum(String), IsPrincipal(bool), HoldsPrincipal(bool) }
fn allowed_with(&self, root: &Path, file: &str, action: &str, principal: Option<&str>, given: &[GateGiven]) -> Result<Found<Allowance>, Vec<Said>>;
```

- `FlowValue` の `holds` は、`fed by` の口が場所の持つものを言う型（`Carried`。26 章）である。数は単位と範囲、列挙は値、日付は日で、ゲートの属性と同じ比べ方ができる。操作の呼び出しに書いた数は、引数の型と単位の数である（整数が刻みを数える型の引数では刻みの数。27 章）。`path` は、値を読んでから呼び出しまでに別の値が入っていなければ、読んでいる入力・変数・フィールドである。
- `Reached` の `values` は、呼び出しに届く実行が、`Flows::value` より狭い値しか持たない入力と変数の、その呼び出しでの値である。ワークフローの下の行の右の値と、規則に渡した値のうちゲートに言葉の無いものを、これで読む（27 章）。
- `Reached` は dandori の語のままで、ゲートの語への読み替えは sekisho がする。ritsu-cross は、dandori の答えをそのまま sekisho に渡す。口に「ゲートの語で書いた条件」の型を作らずに済む。
- 「いつも」と「決して」の答えは、`unheld` と `unrelated` を空にする。数えなかったものを数えても、答えが変わらないからである。
- `allowed_at` は `.gate` にだけ答える。手で書いた Cedar の組は、既定の実装（`Gates::allowed`）で答える。
- `allowed_with` は、属性のいくつかの値を知っている読み手のための問いである。principal かリソースの属性を値に決め（真偽、列挙の値、エンティティを指す属性が principal そのものか、principal を含むか）、その値をとる組み合わせだけで、action がいつも・組み合わせによって・決して許されるかを答える。principal は型で言い（None は action がとる型のすべて）、役割はどれも数える。action のどのポリシーも読まない属性は、何も絞らない。どの組み合わせもとらない値（principal と別の型の属性を「principal そのもの」と決める）は、組み合わせを残さないので「決して」である。属性の型がとれない値は、理由を言って「決められない」と答える。`allowed`・`allowed_at` と同じ数え上げ（`checks::allowance_over`）を、選ぶ組み合わせだけ変えて読むので、三つの問いはゲートを同じに読む。`.gate` にだけ答え、手で書いた Cedar の組と既定の実装は「決められない」と答える。

### 8.4 yuen：ツール名 `sekisho` と種類

参照の書き方（ritsu の DESIGN 6.2）に、ツール名 `sekisho` と、8.2 の種類を足す。例：

```req
requirement refunds_within_limit
  text "A clerk refunds no more than the clerk's own limit"
  satisfied by sekisho "refunds.gate" policy clerks_refund_within_their_limit
  verified by sekisho "refunds.gate" expect clerks_never_refund_over_their_limit
```

リンクの端は、そのポリシーと期待の定義の文で、ポリシーを書き換えれば、その端のリンクだけが止まる（yuen の DESIGN 3.2 と 4.1 の端と同じ）。手で書いた Cedar を指すツール名 `cedar`（種類 `policy`（`@id` で）、`action`、`entity`）も足す。`naming.tsv`（42 行）に、`sekisho` の行と入れ子の行、誤りの行を足す。

2026-10-06 に、ツール名 `cedar` と `openapi`・`asyncapi` を参照の書き方に足した（yuen の DESIGN 3.6）。`cedar` の `policy` の名前は `@id`（無ければ Cedar の CLI と同じく、ファイルの中の順の `policy0`、`policy1`）、`action` と `entity` は宣言した名前で、名前空間は付けない。yuen の端は、ポリシーを `cedar format` の形で書いたものと、宣言をスキーマの人が読む形で書いたもの（action は `context` の共通の型も含む）である。`naming.tsv` は 63 行になった。ツール名 `sekisho` は、まだ `Tool::ALL` に入れていない。段階 D で、口 `Items` と一緒に足す。

段階 D（2026-10-06）で、`sekisho` を `Tool::ALL` の十三番目に入れた。種類は 8.2 の表のとおりで、子の種類 `attribute` は `principal` と `resource` のどちらの下にも書ける（ritsu-base の `ErrorKind::ChildFirst` が親の全部を持ち、誤りの文は「`attribute` は `principal` か `resource` のすぐあとにしか書けません」と言う）。`naming.tsv` に sekisho の JSON の行を六つ、誤りの行を三つ足して、75 行（JSON 47 行、誤り 28 行）になった。`Items` の定義の文は、宣言の塊の行で、ritsu-base の `definition::block`（dandori から土台に移した）が作る。yuen の `verified by` には、sekisho の `expect` と `separate`（落ちることのあるもの）と、`.gate` のファイル全体（sekisho の検査）を書ける。ポリシーなどほかの種類は、`satisfied by` に書く（`verified by` なら E403）。端は yuen の `tests/gates.rs` が、英語の材料 `tests/fixtures/refund_gates/` と日本語の版 `返金のゲート/` で確かめる（ポリシーと期待の端、同じ `.gate` のほかのポリシーを直しても止まらないこと）。

`guards` の操作は、`openapi "api/orders.json" operation refundOrder`、`asyncapi "…" operation <キー>`、`proto "…" service S method M`、帳簿の振替の操作なら `chobo "books/stock.book" transfer receive operation do` の参照で書く（2.6、3.4。`ritsu_base::naming::Tool::Openapi` と `Tool::Asyncapi`、chobo の `transfer` の下の `operation`）。パスはルートからである（2.6）。E202 は、yuen の E202 と同じく、文書のパス（走らせたディレクトリから）と、書いた組で、無いものを言う（`There is no operation refundOrders in examples/refunds/api/orders.json`、`examples/refunds/api/orders.json に operation refundOrders はありません`）。E203〜E205 は、見つけた操作の参照で言う（10 章）。`api` の `guards` は sakai の `api` と同じ参照の JSON（`{"text", "tool", "path", "items"}`）と行で、口 `Gates` の `GateAction::guards` は同じ `Name` と行である（段階 D で口に答えるときに、`Action::references` を入れる）。

### 8.5 sakai

- `.gate` は、ほかの成果物と同じく、ちょうど一つのコンテキストに属する（`owns` に `sekisho "…"`）。
- `.gate` の参照（8.2）は、境界を越える参照として関係で確かめる。規則とカレンダーを読むのは、ほかの言語の成果物を読むのと同じ決まり（共有カーネル、公表された言語）。
- `guards` は、操作を守る参照である。操作の契約の文書と同じコンテキストの `.gate` だけが、その操作を守れる（段階 D で sakai と確かめる）。操作を持つサービスが、自分の認可を書くのが自然だからである。

段階 D で作った形（2026-10-06。sakai の DESIGN 17 章）：

- `.gate` は一式の成果物で、`dir` か `sekisho "…"` でコンテキストに属する。手で書いた Cedar のファイル（`.cedar`、`.cedarschema`、`.cedarschema.json`）は、`owns` の `cedar "…"` の項で書いたものだけが成果物になる（`dir` の下にあるだけのものは、どのコンテキストのものでもない）。
- 操作を守る参照（`.gate` の `guards` と、Cedar のスキーマの `@guards`）が、ほかのコンテキストの契約の操作を指せば、関係によらず sakai の E211 である。
- `use rule`・`use dates`・`use calendar`・`use gate`・`import enum` は、境界を越えれば、二つの共有カーネルに並べたものだけを読める（関係が無ければ E201、あれば E202）。ゲートから生成したコードが規則と日付から生成したコードを呼び、`use gate` はほかのゲートの宣言を読み、`import enum` は `enums` ファイルの列挙を丸ごと自分の列挙にするので、使うのは相手の内側だからである。
- `use openapi`・`use proto`・`use asyncapi`・`use book` と `workflow … from` は、境界を越える参照に数えない。契約は `guards` が守る操作のために読み（その操作は E211 で確かめる）、ワークフローは呼ぶ側の名前で、ワークフロー自身の呼び出しはそのフローの参照として確かめるからである。

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
| 行頭 | `gate` `description` `namespace` `use` `today` `import` `enum` `role` `principal` `workflow` `resource` `action` `permit` `forbid` `expect` `separate` |
| `use` | `rule` `dates` `calendar` `openapi` `proto` `asyncapi` `book` `gate` `from` |
| `import` | `enum` `from` |
| 塊の中 | `includes` `can` `roles` `attributes` `guards` `principal` `resource` `from` `nobody` `input` `context` `action` `actions` `when` `unless` |
| 条件 | `in` `is` `not` `and` `or` `any` `workflow` `open` `today` `allow` `deny` `allowed` |
| 型と範囲 | `bool` `date` と rulec の単位の型（`money[…]`、`mass[…]` など）、`range`、`offset` |
| 出どころ | `fed` `by`（行の最後に二語が揃って初めて形になる。参照の中は読まない。26 章） |
| 記号 | `=` `:` `,` `.` `(` `)` `<` `<=` `>` `>=` `|` `->`、`#`（行末までコメント） |

`principal` と `resource` は、行頭（宣言）と、ポリシーの塊の中（スコープ）と、式の中（`principal.suspended`）に出る。どれも同じ語で、位置で意味が決まる（Cedar と同じ）。

名前にも別名にもできない語は、キーワードのうち、条件と計算と型を書く語だけである（`kw::RESERVED`：`and`・`or`・`not`・`in`・`is`・`principal`・`resource`・`workflow`・`true`・`false`・`allowed`・`any`・`today`・`bool`・`date`・`number`・`rate`。E002）。名前が同じ綴りだと、条件が二通りに読めてしまう（`resource.status is not …` の `not` は、値か、`is not` か）。ほかのキーワードは、行の初めか、それだけが書ける位置にしか出ないので、名前と同じ綴りでも読み違えない。だから、列挙の値 `open`、属性 `description`、入力 `range` のように、業務の語をそのまま名前にできる。koyomi はキーワードを全部名前にできなくしたが、認可では列挙の値が業務の語（`open`・`allow`・`deny` など）であることが多いので、読み違えるものだけに絞った。`allowed` は 2026-10-11 に足した。真偽の値の名前が `allowed` だと、`when allowed` が、その値か、ほかの action の答え（2.11）かで二通りに読めるからである。

数は、単位をすぐあとに付けて書く（`50GBP`、`10_000GBP`、`100万円`、`5%`、`1.5kg`、`-40℃`）。rulec と dandori と同じ書き方で、単位は ritsu-units の表にあるものだけである（無ければ E001）。単位の字は ritsu-units の `chars` が決め、全角の `％` は `%` として読む。`℃` と `℉` は文字ではなく記号なので、数のすぐあとのほかには、型の `[` のすぐあと（`temperature[℃]`）でだけ一つの語になり、名前の一部にはならない（17 章）。数字のあとに日本語が続き、単位でないものは名前である（`3日以内`）。単位を持つ数と比べる定数に単位が無ければ E102 で、文は単位を付けた形を見せる。率の単位は百分率で、刻みが 1% でない率では、同じ数を刻みの数として読んだときの百分率も見せる（`rate[step 0.1%]` の `fee` と比べた `10` には、`10%` と、0.1% の刻みを 10 数えた `1%`）。定数を刻みの数として読むことはしない。書いた形から、どちらの意味かが読む人に分からないからである。Cedar の演算子 `==`・`!=`・`&&`・`||`・`!` は sekisho の字句になく、E001 の文が sekisho の書き方（`is`、`is not`、`and`、`or`、`not`）を言う。

### 9.2 EBNF

```ebnf
file        = header, [description], [namespace], {use}, [today], {enum | import_enum}, {role}, {principal}, {workflow},
              {resource}, {action}, {policy}, {expect}, {separate} ;
header      = "gate", name, version, NL ;
version     = "v", digit, {digit} ;
description = "description", string, NL ;
namespace   = "namespace", cedar_name, {"::", cedar_name}, NL ;  (* Foo か Foo::Bar。ASCII *)
use         = "use", ("rule" | "dates" | "calendar" | "openapi" | "proto" | "asyncapi" | "book"), word, "from", string, NL
            | "use", "gate", string, NL ;
today       = "today", range, "offset", utc_offset, NL ;          (* range >=2026-10-01 <=2028-10-31、offset +00:00 *)
enum        = "enum", name, "=", name, {"|", name}, NL ;
import_enum = "import", "enum", word, "from", string, ["->", name], NL ;   (* 25 章 *)

(* ブロックの中の行は、ここに書いた順に書く（違えば E003）。一度だけの行を二度書いても E003 *)
role        = "role", name, NL, [INDENT, [description], ["includes", refs, NL], ["can", refs, NL], DEDENT] ;
principal   = "principal", name, NL, [INDENT, [description], ["roles", refs, NL], [attributes], DEDENT] ;
workflow    = "workflow", name, "from", string, NL, [INDENT, description, {holds}, DEDENT] ;
holds       = "resource", name, "from", flow_value, NL, [INDENT, {name, "=", flow_value, NL}, DEDENT] ;   (* 2.12 *)
flow_value  = name, {".", name} ;                                  (* フローの入力か変数と、そのフィールド *)
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
            | "principal", "is", ("workflow", ref | ref)
            | "allowed", ref, ["(", arg, {",", arg}, ")"] ;             (* ほかの action の答え。2.11 *)
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
| E006 | 同じ名前を二度宣言した（`import enum` が持ってきた列挙と、その `->` の名前も） |
| E007 | Cedar に出る名前に ASCII の別名が無い、別名の形が違う |
| E008 | 別名が Cedar の予約語（`if`・`then`・`else`・`like`・`has`。`true`・`false`・`in`・`is` は E002）と同じ。型の名前が、Cedar の `Action`（スキーマに宣言できない）、組み込みの型（`Bool`・`Long`・`String`。`cedar validate` が警告する）、JSON のスキーマが型に使う語（`Boolean`・`Entity`・`Extension`・`Record`・`Set`）、sekisho が宣言する `Role`・`Workflow` と同じ（Cedar 4.13.0 で確かめた）。生成先の言語の予約語は E008 にしない（W402。32 章） |
| E101 | 知らない名前（役割、型、属性、列挙の値、action、`input`、計算した値、規則の出力、`import enum` が読むファイルに無い列挙） |
| E102 | 型が合わない（列挙を数と比べる、単位の違う数を比べる。単位は ritsu-units で比べる） |
| E103 | 範囲の誤り（`range` が無い、定数が範囲の外、整数にならない定数、±(2⁵³ − 1) を超える範囲） |
| E104 | v1 で書けない関係（二段以上たどる、型の違う二つの属性を比べる。2.5） |
| E105 | 計算した値の誤り（規則でも日付でもない、規則の出力が列挙か真偽でない、入力に書けないものを渡す） |
| E106 | `principal` のスコープが action の principal の型に無い（`principal is Customer` を Customer の来ない action に） |
| E107 | `today` を使うのに `today` の行が無い、オフセットにタイムゾーンの名前を書いた |
| E108 | 役割の `includes` が輪になる（`clerk → manager → clerk` のように、輪の役割を順に示す。輪一つにつき一度） |
| E109 | ポリシーの `allowed` が action のあいだで輪になる（自分を尋ねるものを含む。輪の action を順に示す。輪一つにつき一度。期待の `allowed` は数えない。2.11） |
| E110 | `allowed` の入力と型：省けない入力に何も渡らない、型か単位か範囲が入力に合わない値や、無いことがある値を省けない入力に渡す、計算した値を渡す、この action のとる型のどれかが持たない属性を渡す、この action のとる principal か resource の型を尋ねる action がとらない、一つの action で同じ action を二通りの入力で尋ねる、`allowed_<別名>` がこの action の入力か計算した値と同じ名前（2.11） |
| E201 | `use` のファイルが、その言語の検査を通らないか、読めない（その言語の言うことを注に）。`use rule` と `use dates` は名前の検査が口で事実を読むときに、`use calendar` と `use book` は検査がカレンダーと帳簿を読むときに、`use gate` は sekisho が読むときに出す。`use gate` が輪になっているときも。`use` のファイルがルートの外にあるときも（gate が読むファイルは、守る操作も `@doc` も口も、ルートからのパスで参照する。2.6）。`import enum` のファイルが `enums` ファイルでなく規則であるとき、rulec の検査を通らないときも、この番号（25 章）。 |
| E202 | `guards` の操作が契約の文書に無い（文書のパスと、書いた組で言う。2.6） |
| E203 | `input` が操作の受け取るものに無いか、型か範囲が操作と合わない（dandori の E016 と同じ読み方） |
| E204 | `from` の引数が操作のパスかクエリの引数に無い |
| E205 | 二つの action が同じ操作を守る |
| E206 | 規則か日付の入力に渡す値の範囲が、その入力の範囲を外れうる（規則の、順序のある列挙の入力では、範囲に入らない値になりうる）、または規則の前提を破りうる（例つき） |
| W201 | 規則の前提を守るかを決められない（生成したコードが走らせたときに確かめる） |
| E207 | カレンダーのデータの範囲が、`today` の範囲か、日付の関数が返しうる日を覆わない |
| E208 | `workflow` の `.flow` が dandori の検査を通らないか、読めないか、ルートの外にある（2.6） |
| E209 | sekisho のクレートのバイナリはほかの言語を持たない（`ritsu sekisho …` で走らせるよう言い、exit 2。dandori の E018 と同じ形）。`use rule`・`import enum`・`use dates`・`use calendar`・`use book`・`workflow … from` の最初の一行で一度だけ出し、ほかの診断は出さない。契約（`use openapi`・`use proto`・`use asyncapi`）と `use gate` は、sekisho のクレートが土台の読み手で読むので出さない |
| E210 | `use gate` で読んだファイルの名前空間が、読む側と違う |
| E211 | `use gate` で読んだ二つのファイルが、同じ名前か別名の型・列挙・役割・ワークフローを宣言している（あとから読んだファイルの `use gate` の行に出る） |
| E212 | ワークフローの下の行（2.12）が書くフローの値が、フローに無いか、属性と型か単位が違うか、行が当たる呼び出しで属性の範囲の外になりうる（注に、書ける名前か、外になる値とそれが届く呼び出し。27 章） |
| W202 | ワークフローの下の `resource … from …` の行が当たる呼び出しが、フローに無い（注に、なぜか：その型の resource を引数から受け取る action が無い、受け取る action が ID を渡す引数を書いていない、フローがその action の守る操作を呼ばない、呼び出しが ID の引数に別の値を渡す。渡す値も。27 章） |
| E301 | どの組み合わせでも、どの permit も action を許さない（`nobody "<理由>"` で通す） |
| E302 | permit が許そうとする組み合わせが、どれも forbid に拒まれる |
| E303 | permit か forbid の条件が、どの組み合わせでも成り立たない（行ごとの満たされる数を注に。規則が一緒に出さない値か、ゲートが渡す値に答えない値を求めているなら、その値と、何から計算するかも。4.2） |
| E304 | 期待が成り立たない（成り立たない数と、一つの例。例は、計算した値の横に、確かめた入力を添える。4.2） |
| E305 | 職務の分離が成り立たない（principal と、それぞれの action を許す例） |
| E306 | 役割が `can` に無い action を許される |
| E307 | 組み合わせの数が予算（既定 10⁸、`--budget`）を超えた。何も生成しない |
| W301 | permit が許す組み合わせを、ほかの一つの permit が全部許す |
| W302 | `can` に書いた action を、その役割だけでは一度も許されない |
| W303 | 決められない（多めに数えた組み合わせから出た例を、具体的な入力で起こせなかった。読んだ Cedar に有限でない式がある） |
| W304 | 期待が、どの組み合わせも選ばない（成り立つが、何も確かめていない。`principal` の行が当てはまる数と、それぞれの行が満たされる数を注に。規則が一緒に出さない値か、ゲートが渡す値に答えない値を選んでいるなら、その値と、何から計算するかも。4.2） |
| W305 | 名前で挙げた action の一つに、ポリシーがどの組み合わせでも当てはまらない（ほかに挙げた action には当てはまる。その action で `principal` の行と条件の行が満たされる数、規則が理由ならその値、当てはまる action とその数を注に。4.3） |
| W306 | permit が、名前で挙げた action の一つでは何も許さない。許すはずの組み合わせを、どれも forbid が拒む（ほかに挙げた action では許す。許すはずの数、拒む forbid とその数、例を注に。4.3） |
| W307 | permit の行が、どの決定も変えない。行が外す組み合わせを、どれも forbid が拒む（外す数、拒む forbid とその数、例を注に。一緒に消すと答えが変わりうるほかの行も。4.3） |
| W308 | ポリシーの行が、起こりうるどの組み合わせも外さない（満たされる数を注に。規則が決めていれば、どの値がなぜ来ないかも。一緒に消すと答えが変わりうるほかの行も。4.3） |
| E401 | `gen` の `--target typescript`・`python`・`go` で、W402 がその出力先について言うものがある。何も書かず、1 で終わる（`ritsu gen` も同じ） |
| W401 | 生成したポリシーかスキーマが Verified Permissions の上限を超える（`--authorizer avp`） |
| W402 | ゲートの二つの名前が生成するコードで一つの識別子になるか、名前がコードの自分で使う名前と同じか、名前がコードのそのまま書く所でその言語の予約語になる（`gen` はその出力先で E401） |
| W901 | 鍵の形の値（ritsu の DESIGN 16.3 の九つの種類）を、文字列かコメントに書いた。同じ行に `ritsu: test secret` と書いた値は言わない |
| W910 | ポリシーが読む `input` が、守る操作の契約が秘密と印を付けた引数かフィールドである（その値は、Cedar に尋ねるリクエストと判断の記録に残る） |

ritsu の台帳には、X15 と X16 の E907・W907・E908・W908・W909 を足した（4.6 の段階 D で作った形）。2026-11-17 に、X16 の E909（呼び出しが渡す数を、`input` が違う単位か刻みで読む。27.3）を足した。

**W901 と W910（段階 D、2026-10-06）**：セキュリティの検査の帯（ritsu の DESIGN 16 章）のうち、sekisho のファイルの中だけで決まる二つを、sekisho の台帳に置く（`src/security.rs` と `src/contracts.rs`）。

- W901 は、ほかの言語と同じく `ritsu_base::secrets` でファイルのテキストを調べる。鍵の行は出さない（出すと、走らせるたびのログに鍵が残る）。名前の検査より先に調べるので、読めないファイルでも言う。変異は英語と日本語の三つずつ（文字列の鍵、コメントの鍵、テストの値の印）で、どれも ritsu の DESIGN 16.10 の偽の鍵を使う。
- W910 は、ポリシーが条件で読む `input` だけに言う。ポリシーが読まない `input` は、生成した Cedar の `context` に入らず（5.1）、Cedar に届かないからである。印の読み手は、セキュリティの検査と同じもの（OpenAPI と AsyncAPI は ritsu-base の `marks::schema_mark` で、`x-data-classification`（`sensitivity` が `confidential` か `restricted`）、`x-sensitive-data`、`format: password`。ritsu-base の `openapi` の `Schema::mark` に持ち、`allOf` の中の印も読む。proto は ritsu-proto の `Protos::redaction` で、`debug_redact` と、それを付けるカスタムオプション）である。一つの `input` に一つだけ、最初に印を見つけた操作で言う。変異は `W910_secret_input.gate` と日本語の版で、契約は `tests/mutants/loans.json`。英語の版の英語の出力（`tests/golden/`）：

```text
warning[W910]: tests/mutants/W910_secret_input.gate:22:1: The input `credit_score` is a field of the body of openapi "tests/mutants/loans.json" operation openLoan, which its contract marks secret (`x-data-classification`: PII, restricted)
    22 |     credit_score : number  range >=0 <=999
  = An input a policy reads goes into the request Cedar is asked, and stays in the record of the decision (the answer the generated code gives, Verified Permissions' logs): a secret there reaches everyone who can read them.
  = Give the policies what they need of it as a value a rule or a date computes (`context`), or take it out of the policies' conditions.
```

日本語の版の日本語の出力：

```text
警告[W910]: tests/mutants/W910_秘密のinput.gate:22:1: input `信用スコア` は openapi "tests/mutants/loans.json" operation openLoan の本文のフィールドで、契約が秘密と印を付けています（`x-data-classification`: PII, restricted）
    22 |     信用スコア(credit_score) : number  range >=0 <=999
  = ポリシーが読む input の値は、Cedar に尋ねるリクエストに入り、判断の記録（生成したコードが返す答え、Verified Permissions のログ）に残ります。秘密の値は、その記録を読めるすべての人に渡ります。
  = ポリシーが値そのものを読まずに済むよう、規則か日付で計算した値（`context`）にして渡すか、ポリシーの条件から外してください。
```

E006 で名前を比べるのは、同じ種類のもののあいだである。型（principal と resource）と列挙は、フィールドの型の位置でどちらも書けるので、一つの名前の集まりにする。ほかは、役割、一つの列挙の値、一つの型の属性、ワークフロー、action、一つの action の入力と計算した値（Cedar の `context` で並ぶので一つ）、ポリシー（permit と forbid。`@id` が重ならないように）、期待、職務の分離、`use` の名前、のそれぞれである。名前と別名のどちらが重なっても E006 で、`use gate` で読んだファイルの型・列挙・役割・ワークフローとも比べる。種類が違えば同じ名前でよい（例の日本語の版では、`use openapi 注文` と `resource 注文(Order)` が並ぶ。期待 `managers_refund_in_period` は、同じ名前の permit と並ぶ）。

E203〜E205 は、操作を参照の書き方で言う（2.6）。E205 の変異の、英語の版の英語の出力と、日本語の版の日本語の出力（テストと同じく、クレートのディレクトリをルートにしたもの。`tests/golden/`）：

```text
error[E205]: tests/mutants/E205_one_operation_two_actions.gate:60:1: Two actions, `view_order` and `refund_order`, guard openapi "examples/refunds/api/orders.json" operation refundOrder
    60 |   guards orders refundOrder
  = Which of them decides is not settled: guard it with one action.
```

```text
エラー[E205]: tests/mutants/E205_二つのactionが守る操作.gate:60:1: openapi "examples/refunds/api/orders.json" operation refundOrder を、`注文を見る` と `返金する` の二つの action が守ります
    60 |   guards 注文 refundOrder
  = どちらの判断で守るかが決まりません。一つの action にまとめてください。
```

次は、例に forbid `clerks_do_not_refund` を足した変異（4.3）の最初の E302 である。英語の版の英語の出力と、日本語の版の日本語の出力を並べる（`tests/golden/`）。

```text
error[E302]: tests/mutants/E302_forbid_covers_permit.gate:87:1: The permit `clerks_refund_within_their_limit` allows nothing: a forbid denies every combination it would allow
    87 | permit clerks_refund_within_their_limit
  = It would allow 192 combinations; `clerks_do_not_refund` denies 192 of them, `auditors_do_not_refund` 96, `suspended_staff_do_nothing` 96, `no_second_refund` 48.
  = For example: User holding clerk (suspended: no), Order (status: paid), amount: 1GBP to 50GBP, refund_band: within_limit (as when refund_limit is 1GBP and amount is 1GBP), in_period: yes, business_day: no; denied by `clerks_do_not_refund`.
  = Narrow the forbid, or remove the permit if no one is meant to be allowed this.
```

```text
エラー[E302]: tests/mutants/E302_forbidに覆われるpermit.gate:87:1: permit `係は上限まで返金できる` は何も許しません。許すはずの組み合わせを、どれも forbid が拒みます
    87 | permit 係は上限まで返金できる(clerks_refund_within_their_limit)
  = 許すはずの組み合わせは 192 通りで、`係は返金しない` が 192 通り、`監査は返金しない` が 96 通り、`停止中の職員は何もできない` が 96 通り、`二度は返金しない` が 48 通りを拒みます。
  = 例：係を持つ職員（停止中：いいえ）、注文（状態：支払済）、金額：1GBP〜50GBP、返金の区分：上限まで（たとえば返金できる額が 1GBP、金額が 1GBP のとき）、期間内：はい、営業日：いいえ。`係は返金しない` が拒みます。
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

W305（名前で挙げた action に当てはまらないポリシー）の、英語の版の変異の英語と日本語の出力。`list_folder` の resource（フォルダー）は `open_to_all` を持たないので、`when resource.open_to_all` はそこで満たされない：

```text
warning[W305]: tests/mutants/W305_a_policy_that_never_applies_to_an_action_it_names.gate:37:1: The forbid `suspended_staff_see_nothing_open` names the action `list_folder`, and applies to no combination of it
    37 | forbid suspended_staff_see_nothing_open
  = Of the 2 combinations its `principal` line takes, the lines are met: `when resource.open_to_all` on 0, `when principal.suspended` on 1. A line is met on none.
  = On the other actions it names, it applies to 1 combination of `read_doc`.
  = Remove `list_folder` from its `action` line, or correct its lines so that it applies to `list_folder` too.
```

```text
警告[W305]: tests/mutants/W305_a_policy_that_never_applies_to_an_action_it_names.gate:37:1: forbid `suspended_staff_see_nothing_open` は action `list_folder` を挙げていますが、`list_folder` のどの組み合わせにも当てはまりません
    37 | forbid suspended_staff_see_nothing_open
  = `principal` の行が当てはまる 2 通りのうち、`when resource.open_to_all` は 0 通り、`when principal.suspended` は 1 通りで満たされます。どの組み合わせでも満たされない行があります。
  = ほかに挙げた action では、`read_doc` の 1 通りに当てはまります。
  = `action` の行から `list_folder` を外すか、`list_folder` にも当てはまるよう行を直してください。
```

W306（挙げた action で何も許さない permit）、W307（どの決定も変えない permit の行）、W308（どの組み合わせも外さない行）の、英語の版の変異の英語と日本語の出力：

```text
warning[W306]: tests/mutants/W306_a_permit_that_allows_nothing_of_an_action_it_names.gate:26:1: The permit `staff_read_and_purge` names the action `purge_doc`, and allows nothing of it: a forbid denies every combination it would allow there
    26 | permit staff_read_and_purge
  = On `purge_doc` it would allow 1 combination; `no_one_purges` denies it.
  = For example: User holding staff, Doc; denied by `no_one_purges`.
  = On the other actions it names, it allows 1 combination of `read_doc`.
  = Remove `purge_doc` from its `action` line, or narrow the forbid if the permit is meant to allow `purge_doc`.
```

```text
警告[W306]: tests/mutants/W306_a_permit_that_allows_nothing_of_an_action_it_names.gate:26:1: permit `staff_read_and_purge` は action `purge_doc` を挙げていますが、`purge_doc` では何も許しません。許すはずの組み合わせを、どれも forbid が拒みます
    26 | permit staff_read_and_purge
  = `purge_doc` で許すはずの組み合わせは 1 通りで、`no_one_purges` が 1 通りを拒みます。
  = 例：staff を持つ User、Doc。`no_one_purges` が拒みます。
  = ほかに挙げた action では、`read_doc` の 1 通りを許します。
  = `action` の行から `purge_doc` を外すか、`purge_doc` も許すつもりなら forbid を狭めてください。
```

```text
warning[W307]: tests/mutants/W307_a_line_of_a_permit_that_changes_no_decision.gate:22:1: The line `when resource.open_to_all` of the permit `services_index` changes no decision: a forbid denies every combination it leaves out
    22 |   when resource.open_to_all
  = Without it, the permit would also apply to the 1 combination only this line is not met on, which `services_skip_closed_docs` denies.
  = For example: Service, Doc (open_to_all: no); denied by `services_skip_closed_docs`.
  = Taking it out changes no answer: remove it, or correct it if it is meant to leave out something else.
```

```text
警告[W307]: tests/mutants/W307_a_line_of_a_permit_that_changes_no_decision.gate:22:1: permit `services_index` の行 `when resource.open_to_all` は、どの決定も変えません。この行が外す組み合わせを、どれも forbid が拒みます
    22 |   when resource.open_to_all
  = この行が無ければ、permit は、この行だけが満たされない 1 通りにも当てはまります。その組み合わせは、`services_skip_closed_docs` が拒みます。
  = 例：Service、Doc（open_to_all：いいえ）。`services_skip_closed_docs` が拒みます。
  = 行を消しても、答えは変わりません。行を消すか、ほかの組み合わせを外すつもりだったなら、行を直してください。
```

```text
warning[W308]: tests/mutants/W308_a_line_that_leaves_out_no_combination.gate:28:1: The line `unless resource.status is refunded` of the permit `clerks_refund_paid_orders` leaves out no combination that can happen
    28 |   unless resource.status is refunded
  = It is met on the 1 combination its `principal` line and its other lines are met on.
  = Remove the line, or correct it so that it leaves out what it is meant to.
```

```text
警告[W308]: tests/mutants/W308_a_line_that_leaves_out_no_combination.gate:28:1: permit `clerks_refund_paid_orders` の行 `unless resource.status is refunded` は、起こりうるどの組み合わせも外しません
    28 |   unless resource.status is refunded
  = `principal` の行とほかの行が満たされる 1 通りで満たされます。
  = 行を消すか、外すつもりの組み合わせを外すよう行を直してください。
```

規則が決めている行の W308（`tests/walk/together/together.gate`。一つの金額を規則の二つの入力に渡すので、`kind` は `mixed` にならない）：

```text
warning[W308]: tests/walk/together/together.gate:109:1: The line `unless kind is mixed` of the permit `staff_weigh_what_is_not_mixed` leaves out no combination that can happen
   109 |   unless kind is mixed
  = It is met on each of the 2 combinations its `principal` line takes.
  = `kind` is never `mixed`: it is computed from `resource.amount` (given as both `first` and `second`), and the rule answers `mixed` for none of the values the gate gives it.
  = The rules decide this, so the line changes nothing of what the permit applies to. To hold what a rule answers, write it in the rule (its `examples`, which rulec checks); here, remove the line or correct it.
```

規則が一緒に出さない値を求める行（4.2）は、理由と、それに合う直し方を言う。500 円で切る規則の `a` と 300 円で切る規則の `step` に同じ金額を渡すゲートの、permit の E303（英語）と期待の W304（日本語）（`tests/walk/golden/E303_W304_apart.txt`）：

```text
error[E303]: tests/walk/inline.gate:27:1: The permit `high_and_small` applies to no combination
    27 | permit high_and_small
  = Of the 4 combinations its `principal` line takes, the lines are met: `when a is high and step is small` on 0. A line is met on none.
  = `a` is never `high` while `step` is `small`: both are computed from `resource.amount`.
  = Correct the lines to values the rules give together, or remove the permit itself.
```

```text
警告[W304]: tests/walk/inline.gate:44:1: 期待 `high_small_is_not_sent` は、どの組み合わせも選びません。成り立ちますが、何も確かめていません
    44 | expect deny high_small_is_not_sent
  = `send_doc` の 8 通りのうち、`when a is high` は 4 通り、`when step is small` は 2 通りで満たされますが、全部が同時に満たされることはありません。
  = `a` が `high` で、`step` が `small` になることはありません。どちらも `resource.amount` から計算する値です。
  = これは規則が決めていることで、ポリシーをどう書いても成り立ちます。この期待は、ポリシーについて何も確かめていません。規則の答えを確かめたいなら規則の側に書き（`examples`。rulec が検査します）、この期待は消すか、行を直してください。
```

ゲートが渡す値に規則が答えない値を求める permit の E303（`tests/walk/golden/E303_W304_never.txt`。一つの金額を、二つの金額が 500 円の同じ側にあるかを答える規則の両方の入力に渡す）：

```text
error[E303]: tests/walk/inline.gate:42:1: The permit `apart_side` applies to no combination
    42 | permit apart_side
  = Of the 1 combination its `principal` line takes, the lines are met: `when side is apart` on 0. A line is met on none.
  = `side` is never `apart`: it is computed from `resource.amount` (given as both `first` and `second`), and the rule answers `apart` for none of the values the gate gives it.
  = Correct the lines to a value the rule gives, or remove the permit itself.
```

同じ二つの規則を読むゲートの E304 の例は、計算した値の横に、確かめた入力を添える（`tests/walk/golden/E304_inputs.txt`。期待が 100 円より上を選ぶので、組み合わせの金額は 101 円から 1,000 円。`a` が low で `step` が small になるのは 300 円までなので、領域は 101 円から 300 円）：

```text
error[E304]: tests/walk/inline.gate:54:1: The expectation `small_ones_over_100_are_not_sent` does not hold: the 1 combination it picks is allowed
    54 | expect deny small_ones_over_100_are_not_sent
  = For example: User holding staff, Doc (amount: 101JPY to 1000JPY), a: low, step: small (whenever amount is 101JPY to 300JPY); allowed by `staff_send`.
  = Fix the policies, or the expectation if it says more than is meant.
```

W304 は警告なので、終了コードは変わらず、`ok —` の行も出る。多めに数えた組み合わせだけを選ぶ期待には言わない（印のある組み合わせも、選んだものとして数える。そうした組み合わせが具体的な入力で起こせなければ、成り立たないかの検査のほうが W303 を言う）。action の無い期待にも言わない。

E110 の、承認の例の変異（`request_approval` の入力 `amount` を外したもの）の、英語の版の英語の出力と、日本語の版の日本語の出力（`tests/golden/`）：

```text
error[E110]: tests/mutants/E110_input_given_nothing.gate:73:18: The input `amount` of `approve` is given nothing: `request_approval` has no input of that name
    73 |   unless allowed approve
  = Write what it is given, like `allowed approve(amount: <value>)`: an input of `request_approval`, an attribute of the principal or the resource, a constant, or `today`.
```

```text
エラー[E110]: tests/mutants/E110_値の無い入力.gate:73:18: `承認する` の入力 `金額` に値がありません。`承認を求める` に同じ名前の入力がありません
    73 |   unless allowed 承認する
  = `allowed 承認する(金額: <値>)` のように、渡す値を書いてください。渡せるのは、`承認を求める` の入力、principal と resource の属性、定数、`today` です。
```

**一つの誤りは一度だけ言う（2026-11-16）。** ritsu の DESIGN 8.3 の決まりに従う。黙るのは、ほかの行がエラーを言った誤りから決まる行だけなので、通るか通らないかと終了コードは変わらない。

- 無い名前（E101）は、種類（役割、action、型）ごとに、ファイルの中で最初に書いた所で一度だけ言い、同じ名前を書いたほかの行の番号を注に挙げる。宣言の側で名前を誤ったのなら、注の一覧（宣言してある名前）が直す所を指す。
- action の `principal` か `resource` の行が無い型を書くと（E101）、その action を読む条件は、ほかの型が持たない属性について何も言わない。その属性は、無い型のものかもしれないからである。principal の型が無い action に、ポリシーの principal が来ないこと（E106）も言わない。

役割と action の名前を宣言の側で書き誤った `tests/chains/chain_names.gate` は、4 件が 2 件になった（英日の版とも）：

```text
error[E101]: chain_names.gate:10:7: There is no action `refund_order`
    10 |   can refund_order
  = The actions there are `refund_ordr`.
  = `refund_order` is written on line 32 too.
error[E101]: chain_names.gate:14:9: There is no role `clerk`
    14 |   roles clerk
  = The roles there are `clrk`.
  = `clerk` is written on line 31 too.
```

principal と resource の型を宣言の側で書き誤った `chain_types.gate` は、3 件が 2 件になった。試験は `tests/chains.rs`。

## 11. コマンド

```
sekisho check <file.gate>... [--format json] [--budget <n>] [--root <dir>] [--lang en|ja]
sekisho gen <file.gate>... --target cedar|typescript|python|go... [--authorizer cedar|avp] [--module <path>] [--out <dir>] [--check] [--root <dir>] [--lang en|ja]
sekisho doc <file.gate> [--format markdown|html] [--root <dir>] [--lang en|ja]
sekisho vectors <file.gate> [--action <action>] [--root <dir>]          全部の組み合わせを cedar run-tests の形で
sekisho api <file.gate> [--root <dir>]                                   外のツールのための JSON（action、守る操作、役割、ポリシーの @id、期待）
sekisho diff <old> <new> [--format text|markdown|json] [--budget <n>] [--root <dir>] [--lang en|ja]   二つの版の決定の差
sekisho explain <code>... | --all [--format markdown|json]
```

- 段階 D までにできたもの：`check`、`explain`、`gen --target cedar`（段階 B。5 章）と `typescript`・`python`・`go`・`--authorizer`・`--module`（段階 C。5.3〜5.6）、`vectors`（6.1）、`api`、`doc`（段階 D。7 章）。`diff` はモジュールの仕組みと一緒に足した（ritsu の DESIGN 17.9.4）。両方にある action ごとに、二つの版の切り方を合わせた組み合わせの全部で決定を比べ、許可が増えたか減ったか、片方にしかないものがあるか、ワークフローの下の `resource … from …` の行が違えば exit 1、比べられなければ exit 2。行の差は、答えの差とは分けて並べる（2026-11-17。27 章）。コマンドの表（`src/cli.rs`）には、できたものだけを載せる（ritsu の DESIGN 4.4。載せたフラグは必ず効く）。
- `gen --authorizer avp` は、Verified Permissions の上限を超えるものがあれば W401 を言い（5.8）、生成は続ける。警告なので終了コードは変わらない。
- `vectors`、`api`、`doc` は、検査を通るファイルを一つ取る。通らなければ、その診断を出して exit 1。`vectors --action` は action の名前か別名で、無ければ exit 2。
- `api` の JSON は、koyomi と chobo の `api` と同じく、ツールと版とファイルのことから始まる：`sekisho`（版）、`name`、`alias`、`version`、`source_sha256`、`description`、`namespace`、`uses`（`use` の行）、`today`、`cedar`（`gen --target cedar` が書く四つのファイル）、`roles`（Cedar のエンティティ、`includes`、`can`）、`types`（エンティティタイプ、持てる役割、親になれる型、属性と Cedar に出るか）、`workflows`、`actions`（エンティティ、守る操作、principal と resource の型、`from`、`nobody`、`input`、context の値と、計算の式、Cedar に渡る値、省けるか。`allowed` の値は `asks` に、尋ねる action と、その入力に渡すもの）、`policies`（`@id`、効く action、書いたファイル）、`expects`、`separations`。どれも行の番号を持つ。守る操作（`actions` の `guards`）は、一つずつ `{"reference": …, "line": …}` で、`reference` は sakai の `api` と同じ参照の JSON（`{"text", "tool", "path", "items"}`。ritsu の DESIGN 6.2 の 7）、パスはルートから（2.6）である。`uses` の `path` と `policies` の `file` は、`.gate` のディレクトリから書く（手元のパスを書かない）。
- 終了コードは 0（問題なし、警告だけ）、1（エラー）、2（使い方の誤り、読めないファイル、ほかの言語を持たないバイナリ）。
- `ritsu sekisho <command>` は、rulec、koyomi、dandori、契約の読み手をつないで走らせる。`sekisho` という名前のリンクも同じ（ritsu の DESIGN 2.3）。
- `ritsu check` は `.gate` を読む順（8.1）で確かめ、X15 と X16 を言語をまたぐ検査に足す。要約の `borders` に、X15 は操作ごと、X16 はワークフローの呼び出しごとに一つと数える。
- `ritsu gen` は 5.6 のとおり。`--authorizer cedar|avp` を取る。

## 12. 八つ目の言語として足すとき

根の README と DESIGN、CLI、リリース、スキル、サイト、テスト、地図、ブラウザで試すページの直すところの要点（`git grep` で探したもの）：

- 看板の「Seven small languages」「七つの小さな言語」を「Eight」「八つ」に（README、サイト、`ritsu --help`、Homebrew と `.deb`・`.rpm` の説明、`ritsu.ctx` の説明、スキル、`marketplace.json`）。
- 言語の名前を並べたコードとテスト（`crates/ritsu/src/cli.rs` の `LANGUAGES`、`crates/ritsu/tests/` の `LANGUAGES`・`TOOLS`、`crates/xtask/src/main.rs`、`packaging/archive.sh`・`homebrew.sh`・`linux.sh`、`.github/workflows/release.yml`）。
- 拡張子と読む順（`ritsu_base::naming::Tool`、`ritsu-project` の `ORDER`）、参照の書き方のツール名（`naming.tsv`、yuen と sakai の診断の文と golden）。
- 地図（`ritsu.ctx` と `contexts/` に `Gates` のコンテキスト）、ブラウザで試すページ（`ritsu-wasm` の `ritsu_gen` と `ritsu_doc`、プロジェクトの例）。

段階 A で先に入れたもの：

- `ritsu sekisho` の入口と、`ritsu check` が `.gate` を sekisho に渡すこと（`ritsu --help` の言語の一覧には、段階 D3 で出した）。
- 読む順（`ritsu-project` の `ORDER`）の sekisho の位置（dandori のあと、yuen の前）。
- 地図の `Gates` のコンテキストと、`ritsu-project` と `ritsu` からの関係。
- `ritsu_base::naming::Tool::Sekisho`（拡張子と種類だけで、`Tool::ALL` には入れていない。参照の書き方のツール名には、まだならない）。段階 D で `Tool::ALL` に入れ、yuen と sakai の診断の文と golden を直した（8.4）。

拡張子を並べる `ritsu-project` の二つの文と `ritsu check --help` の文には、段階 D3 で `.gate` を足し、`ritsu.wasm` を作り直した。

段階 D3（2026-10-06）で入れたもの：

- 看板を「Eight small languages」「八つの小さな言語」にし、言語の数を言う文を八つに、スキルの数を九つにした（根の README とサイトの頭のページ、`ritsu --help`、Homebrew の formula と `.deb`・`.rpm` の説明、`action.yml`、リリースのワークフロー、`ritsu.ctx` の説明、`marketplace.json`、言語の README の入れ方の段落）。取り込んだときの記録として書いた文（言語の DESIGN の「ritsu（七つの言語を一つにまとめる処理系）に取り込まれ」など）は、記録のまま残した。
- `ritsu --help` の言語の一覧と、`sekisho` という名前のリンク（`crates/ritsu/src/cli.rs` の `LANGUAGES`。段階 A の `NOT_LISTED_YET` は除いた）。アーカイブ、Homebrew の formula、`.deb`・`.rpm`、`packaging/smoke.sh`（例の `refunds.gate` を `--root` つきで確かめる）、リリースのワークフローと action のリンクとスキルの並び。
- 言語の文書 `docs/reference.md`（言語の全部、`check` が言うこと、`gen` が書くもの、コマンド、終了コード、JSON）と README（英語と日本語）。`tests/docs.rs` が、載せた `.gate` の行、Cedar の行、`$ sekisho …` の出力、診断、変異の数、キーワードの一覧、リンクを確かめ、ritsu の `tests/sekisho.rs` が README の `ritsu check` を確かめる。
- スキル `skills/sekisho`：`SKILL.md` は手で書き、`reference.md` と `codes.md` は `skills/sync.sh` が `docs/` からコピーする（`tests/skill.rs`）。ritsu のバイナリが九つのスキルを持ち、`skills/ritsu/SKILL.md` に sekisho の行を足した。
- ブラウザで試すページ：生成は `gen --target cedar`、ページは `doc`。例の二つの版を、ゲートと、ゲートが読むファイル（規則、日付のファイル、カレンダーとそのデータ、契約、フロー）のプロジェクトにした（グループ `sekisho`）。ページの中では、sekisho がファイルを ritsu-base の `fs` で読み書きする（`check_file`、`gen` の書き出し、`use gate` の読み込み）。

2026-10-08 に、ritsu のサイトに、規則の答えを条件に使う例を芯にした sekisho のページを英語と日本語で一枚ずつ足し（`website/docs/sekisho.md`、`website/docs-ja/sekisho.md`）、載せた出力を ritsu の `tests/website.rs` が実物と突き合わせるようにした。

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

- **ワークフローの下の行（2.12）を保証する形**：サービスが読む resource の属性が、フローの値と同じであることを確かめる形は無い（27.4）。フローの値とゲートの属性に、同じ出どころを書いて突き合わせる形（`fed by` の続き）が候補である。

- **本物の Verified Permissions での確かめ**：スキーマの注釈、ポリシーの `name`、`--authorizer avp` のコードを、本物のアカウントで一度確かめること（6.3）。するかは決めていない。

- **数の出力の規則**：出力の区間を rulec に尋ねれば足せる（`outputs_over` が区間を返す形）。
- **二段以上の関係、Zanzibar の形**：答えを真偽の計算した値で受け取る形（`context` に `editor = <ストアに尋ねる関数>`）が候補。そのとき値の出どころの信頼は、規則と同じく生成したコードの形で守る。
- **Cedar のテンプレート**（`?principal`、`?resource`）：一つ一つの共有（「この文書をこの人に」）はデータで、`.gate` に書くものではない。要るなら、テンプレートを生成し、つないだポリシーはデータとして扱う。
- **エンティティのタグ、部分評価（一覧の絞り込み）**：Cedar の部分評価（CLI の `partially-authorize` と、まだ実験の `tpe`）を使えば、「見られる注文だけを返す」問いに近づける。生成するコードに足せる。
- **SymCC を使うテスト**：cvc5 があるときだけ走らせ、E302 と E303 を SymCC の答えと比べる。
- **Lean のモデル**：区間に分けて数えることが、全部のリクエストを覆うことの証明（ritsu の P7）。Cedar の意味は Cedar の Lean の形式化に任せ、sekisho の数え方だけをモデルにする。
- **LSP**：ritsu の LSP と同じく作らない（ritsu の DESIGN 15 章）。
- **手で書いた Cedar のワークフロー（X16）**：Cedar には `Workflow` のエンティティと `.flow` を結ぶ書き方が無いので、X16 は `.gate` だけを見る（1.3）。`@guards` と同じく、スキーマの注釈で `.flow` を書く形が候補。
- **ワークフローが帳簿の振替を直接呼ぶ形（X16）**：dandori の `book <帳簿>.<振替>.<操作>` のタスクは、X16 が見ない（3.4）。足すには二つを決める要がある。一つは、だれがゲートに尋ねるかである。dandori が書くアクティビティ（Step Functions では帳簿の Lambda 関数）に、chobo のクライアントを呼ぶ前にゲートの生成したコードを呼ばせるか、chobo のクライアントに尋ねる口を持たせるかの、どちらかになる。もう一つは、拒まれたときにタスクに何が返るかである。chobo の拒否の理由に、認可で拒まれたことを表す名前（たとえば `denied`）を足し、タスクがそれを `errors` に宣言できるようにする形が候補で、それがあれば `operation_calls` が帳簿の呼び出しも返し、W909 がその宣言を求められる。
- **手で書いた Cedar の役割の中の役割**：だれがどこに入るかはエンティティのデータが持つので、口は尋ねる側が並べた役割だけを数える（1.3）。役割の親子を書いたエンティティの JSON を読む形が候補。
- **Cedar の JSON の形のポリシー**：ritsu-base が読まないので、口も読まない（ritsu の DESIGN 4.18）。
- **手で書いた Cedar の `vectors`**：`sekisho vectors` は `.gate` だけを読む。手で書いた Cedar の組み合わせを `run-tests` のテストにするのは、テストが使う `cedar_in::cases` だけで、コマンドにはしていない（6.5）。コマンドにするには、尋ねる側（principal の型と、持つ役割の組）を指定する書き方が要る。テストは、型のそれぞれに、ポリシーが書く役割のどの組も持たせている。
- **数えなかった組み合わせが無いことの確かめ**：6.5 の CLI の確かめは、数えた組み合わせの一つ一つが正しいことを言う。数えなかったリクエストが無いことは、`.gate` と同じく数え方の議論が受け持ち（4.1）、テストは整数の区間の両端と、小さな組の答えで、その一部を確かめるだけである。範囲を小さく決めた具体的なリクエストを全部作って CLI に流すことと、数え方を Lean のモデルにすること（上の「Lean のモデル」）が候補。
- **手で書いた Cedar の、数える部分の外の式**：二つの整数の属性の比べ（`context.amount <= principal.limit`）、属性の属性、集合とそのメソッド、ID を並べたエンティティ型（`entity Color enum [...]`）は、決められないと答える（1.3）。整数の比べは、二つの属性の値を一緒に区間に切れば数えられる。
- **`allowed` で、ほかの principal や resource を尋ねる形**：「この人の上長がこれを承認できるか」は principal が違う。だれを尋ねるかをデータから選ぶことになり、数える相手も増える。いまは同じ principal と resource だけにした（2.11）。
- **一つの action で、同じ action を二通りに尋ねる形**：`allowed approve(amount: …)` に別の名前を付けて二つの値にすれば書ける。いまは E110 にして、action を分けてもらう（2.11）。

## 16. 決めたことと、危ないところ

### 16.1 決めたこと

設計の段階で比べて選び、2026-10-06 に決めたもの：

1. **計算した値を context に入れる形（(A)）。** 3.5 の比較のとおり。
2. **生成する Cedar の部分を絞り（P2）、列挙を `String` にする（2.4）。** 列挙のエンティティ型は使わない。Verified Permissions がスキーマの注釈と列挙のエンティティ型を受け付けるかは、本物のアカウントでしか確かめられないからである。
3. **chobo の残高は条件にしない（3.4）。**
4. **関係は一段だけ（2.5）。** テナントの分離（`resource.tenant is principal.tenant`）とメンバーであることは入れ、二段以上と Zanzibar の形は入れない。
5. **言語をまたぐ検査は X15 と X16、コードは E907〜W909（4.6）。** セキュリティの検査（X14、9xx の帯）の続き。
6. **X15 の範囲（4.6）。** 公開ホストサービスの操作だけを求め、AsyncAPI のチャネルは求めない。`security: []` の操作は求めない。
7. **`guards` は、操作と同じコンテキストの `.gate` に限る（8.5）。** 段階 D で sakai の E211 にした。
8. **手で書いた Cedar を指すツール名 `cedar` を足す（8.4）。**
9. **Verified Permissions に置く手順は生成しない（5.8）。** 置くためのファイルだけを書く。
10. **既定の評価の場所は `--authorizer cedar`（5.5）。** その言語の Cedar の実装を同じプロセスの中で呼び、Verified Permissions は `--authorizer avp` で選ぶ。
11. **ワークフローの principal の作り方は、v1 では書かない（2.2）。** ワークフローの資格から `Workflow::"<名前>"` を作るのは API の側の認証で、sekisho は書かない。dandori の生成するアクティビティが、どの資格で呼ぶかを決める項目も、v1 では足さない。
12. **契約が秘密と印を付けたフィールドを `input` に書けば、警告にする。** 段階 D で、セキュリティの検査の印の読み手を使って W910 にした（10 章）。
13. **名前は sekisho のまま。** npm には同じ名前のパッケージ `sekisho`（React のアプリの認証とアクセス制御。0.7.0、2026-07-01）があり、crates.io と PyPI では空いていた（2026-10-06）。看板は「Eight small languages」（「八つの小さな言語」）にした（12 章。段階 D3）。2026-10-07 に、ritsu の看板から言語の数を外した（ritsu の DESIGN 0.1）。
14. **Cedar に渡す規則の列挙の値は、`.rule` に書いた別名にする（3.1）。** 例の `within_limit` で、英語と日本語の版で同じになる。生成するコードのメンバーの名前（`WithinLimit`）は使わない。rulec の口が、列挙の値ごとにこの別名も返す。

段階 A で決めたもの：

15. **名前にも別名にもできない語は、条件と計算と型を書く 16 語だけ（9.1）。**
16. **生成先の言語の予約語は、その出力先のコードが名前をそのまま書く所でだけ、その出力先の W402 にする（32 章）。** 当たるのは、Python のキーワードでは、属性がなるレコードのフィールド、型を小文字にした `Store` のメソッド、一番上の階層の名前（型か列挙の `None` など）、ファイルの別名がなるモジュールの名前、Go のキーワードでは、ファイルの別名から `_` を除いたパッケージの名前、TypeScript では、`ritsu gen` の索引がファイルの別名で書き出す名前（strict モードの予約語と `yield`・`await`）である。ほかの名前は、どの言語の予約語でも当たらない。Cedar の予約語は、どの出力先にも書くので E008 のままである。
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
32. **`@guards` は、一つの注釈に参照を一行に一つ並べる（5.1）。** 帳簿の振替の操作は `chobo "<パス>" transfer <振替> operation <操作>` と書く（参照の書き方の chobo の `transfer` の下に `operation` を足した。ritsu の DESIGN 6.2）。
33. **`vectors` は、区間に二つ以上の値がある組み合わせを、数を全部 low の端にしたものと high の端にしたものの二件にする（6.1）。** 数ごとに端を掛け合わせると、数が増えるたびに倍になる。二件でも、どの区間のどちらの端も、どれかのテストで踏む。
34. **`vectors` のエンティティは、属性が指すもののうち、スキーマで属性を持たない型のものだけを書く（6.1）。** 属性を持つ型のものを書くには、その属性の値も決める要があり、Cedar はそれを読まない。
35. **生成物の、sekisho が書く文は `--lang` で書く（5 章）。** koyomi の `gen` と同じにした。`--check` は書いたときと同じ言語で走らせる。
36. **`--target` は省けない（5 章）。** koyomi の `gen` は省くと全部の出力先を書くが、sekisho のリクエストを組み立てるコードは、サービスの言語を一つ選んで書くものだからである。
37. **関係の項は、条件が書いた綴りではなく、属性の宣言した名前でそろえる。** 段階 A の数え方は、同じ属性を一つの条件で名前、別の条件で別名と書くと、二つの項として数え、起こりえない組み合わせ（同じ属性が principal を指し、かつ指さない）を数えていた。Cedar では一つの属性なので、生成した Cedar と食い違う（`tests/gen/two_spellings.gate` で 10 通りを数え、`run-tests` が 2 件落ちた）。そろえたあとは 4 通りで、例と変異の数は変わらない。
38. **規則の列挙の値を生成したコードの名前（`WithinLimit`）で書いた条件も、数え方と生成で同じ値を引く（3.2）。** 段階 A の数え方は、名前の検査が受け付けるこの綴りを、規則の名前と公開名の中に探して見つけられず、その条件をどの組み合わせでも成り立たないものとして数えていた（例を `WithinLimit` と書くと E303 になった）。口が返す値ごとの生成したコードの名前も持ち、それでも引く。例をそう書き換えたものは、例と同じ数になり、同じポリシーを生成する（`tests/gen.rs`）。
39. **守る操作は、診断の文、`api`、`@guards`、口のどれでも、同じ参照で書く（2.6）。** 参照は `ritsu_base::naming::Name` で組み、文字列を組まない。`operationId` のある操作は、`guards` の行の書き方によらず `operationId` で書き、proto のサービスはファイルの package から見た名前で書く。同じ操作が、書き方によって二つの参照にならないためである。E202 は、yuen の E202 と同じく、文書のパスと書いた組で無いものを言う。
40. **参照のパスはルートからにし、ルートは yuen と sakai と同じに決める（2.6）。** `check`・`gen`・`vectors`・`api` が `--root` を取る。守る契約がルートの外にあれば、E201 にする。参照に書けないパスを、別の形で書いて通さないためである。
41. **決めたポリシーの一致は、`cedar run-tests` に加えて、ポリシーを一つずつ permit にして流して確かめる（6.1）。** `run-tests` は決めたポリシーが含まれるかだけを見るので、それだけでは、ほかの permit と重なる permit の条件を一つ落とした生成の誤りを見逃す（材料の変異で四つあった。6.2）。
42. **スキーマが要る属性を一つ抜いたテストを、CLI がスキーマに合わないと言うことを確かめる（6.1）。** 要る属性を省けるものとして書くスキーマの誤りは、読むところが `has` で守られていると、検証にも答えにも出ない。Verified Permissions がスキーマでリクエストを確かめるときに効く。
43. **生成した Cedar の変異は、ritsu-base の Cedar で読んだ構文木を一か所ずつ変えて作り、検査が数える組み合わせの答えを変えるものの全部が、どれかの確かめで落ちることを求める（6.1、6.2）。** 答えを変えないとテストが示せた変異だけを、分けて言う（2026-11-17）。 文字を変えると、CLI が読めない変異が多くなり、読めないことで落ちても何も確かめていない。答えに出ない変異は作らず、名前のほかが同じファイルは一度だけ変える。変異の材料は絞らない。手元の 14 コアで約 30 秒（CPU の時間は 2 分ほど）、CI の 4 コアの runner では 1〜2 分の見込みで、tools の段で走る。
44. **テストに使う CLI は 4.13.0 だけで、ほかの版は SKIP にする（6.1）。** 出力を一字ずつ比べるからである。CI では、版を上げてテストを直していなければ、その SKIP が許す一覧に無いので落ちる。生成した Cedar を CLI にかけるテストは三つで、CLI が無いときは、それぞれが SKIP の行を一つ出す（合わせて三つ）。SKIP の記録はテストごとなので、三つのテストがそれぞれ自分の理由を言う。英語と日本語の版の双子は `tests/gen.rs` が確かめるので、CLI のテストには重ねて持たない。
45. **CI では、tools の「それ以外」の組で CLI を入れる（6.1）。** ritsu-base の Cedar の材料と同じ組で、入れ方を一つにする。同じ組が、その材料を `expected.sh` で作り直して確かめる（ritsu の DESIGN 4.18）。2026-10-07 から、ritsu のテストが走る dandori の組でも入れる。ブラウザで試すページの通販のゲートの Cedar を、ritsu の `tests/playground.rs` が CLI にかけるからである。
46. **計算した値の `@doc` の最後の文は、どのコマンドが書くかを言わない（5.1）。** 「Computed by the generated code, never taken from the caller」（「生成したコードが計算し、呼ぶ側からは受け取らない」）。同じ Cedar を `sekisho gen` も `ritsu gen` も書くからである。

段階 C で決めたもの：

47. **生成したコードの答えは、組み合わせの答えをそのまま使わず、生の値から参照の評価で求め直した答えと比べる（6.3）。** 生成したコードが従う決まり（読むもの、確かめる順、拒む種類、context）は `raw::Model::evaluate` が一か所に書き、三つの言語の生成器はそれに合わせる。組み合わせの答えとも同じであることを確かめるので、生の値の作り方の誤りも見つかる。
48. **`Store` が返すものは型ごとに一つで、どの action でも同じにする（5.3）。** その型のエンティティを読む action では、全部を確かめる。action ごとに読むものを変えると、同じデータが action によって拒まれたり通ったりする。
49. **確かめる順を決める（5.3）。** principal の型、`today`（`today` を読む action だけ。57）、入力、resource、principal、計算した値の順に確かめ、最初に外れたものの種類で拒む。どの言語でも同じ答えになるようにするためである。resource の ID が無いときは、resource の型が違うときと同じく `resource` で拒む。
50. **resource の ID は `input` の中に置く（5.3）。** `from` を書けばその引数、書かなければ `resource`。型が二つ以上なら `resource_type`。関数の形は、どの action も `(store, principal, input, now)` のままで、生の値の形とも同じになる。
51. **規則の列挙の出力は、生成物のメンバーから公開名を引く表で Cedar に渡す（5.3）。** rulec の生成物の値（TypeScript の列挙の値、Python の `.value`、Go の `String()`）は `.rule` に書いた名前で、日本語の版では Cedar の文字列と違う。
52. **拒む種類は、3.8 の六つに `cedar` と `action` を足した八つにする（3.8、5.3、30 章）。** `cedar` は、Cedar がエラーを言ったときと、尋ねる先に届かないとき（Verified Permissions）で、どれも拒む答えになる。リクエストを組み立てられないことと、尋ねて失敗したことは、直す場所が違うからである。`action` は、名前で尋ねる入口に、ゲートが宣言していない action の名前を渡したときで、尋ね方の誤りを、答えとしての断りと分けて扱えるようにする。`Store` の失敗は、そのまま投げる。
53. **`sekisho gen --target typescript|python|go` は、`ritsu gen` のパッケージと同じ形（`authz/` の隣に `rules/` と `dates/`）で書く（5 章）。** コマンドによってコードを変えないためである。Go の import のパスは `--module` で決め、既定は `ritsu gen` と同じ `generated` にする。
54. **`--authorizer avp` のコードは、尋ねる先 `avp` を最初の引数で受け取り（Go は `ctx` のあと）、ポリシーとスキーマを持たない（5.5）。** 尋ねる先の型は、どの言語も `VerifiedPermissions` という名前にする。
55. **W401 は、`gen --authorizer avp` が `--target` に依らず言い、`ritsu gen --authorizer avp` も言う（5.8）。** ポリシーは一つずつ `cedar format` の形で、スキーマは JSON の形で量り、親は型が持てる役割とそれらが含む役割を数える。グループはデータで決まるので数えない。再現は `check` ではなく `gen` のコマンドなので、台帳の例はコマンドの形にし、変異は置かない。
56. **`ritsu gen` は、ゲートの Cedar を `<out>/cedar/` に一度だけ書き、`--authorizer` を取る（5.6）。** 書くバージョンは sekisho の三つのランナーのロックファイルと同じで、監査のテストが確かめる。
57. **範囲の外の日に拒むのは、`today` を読む計算した値を持つ action だけにする（2.9、3.8）。** `today` を読まない action は、Cedar に渡すものが日に依らず、検査が数えた答えがそのまま当てはまる。ゲートの `today` の範囲が尽きても、日付を読まない action まで止めないためである。生の値のテストは、そうした action に範囲の外の日を渡し、ほかの決まりのとおりに答えることを確かめる（6.3）。
58. **数は宣言した単位の整数（`bigint`、`int`、`int64`）で受け取り、規則を呼ぶときに規則の単位の型に包む（5.3）。** 二つの規則が同じ単位の型をそれぞれ持つと、Python の `NewType` も Go の名前付きの型も別の型になるからである。
59. **Python の生成物はスキーマを持ち、Go の生成物は持たない（5.5）。** cedarpy はスキーマでリクエストとエンティティを確かめ、cedar-go には安定した検証器が無い。範囲と列挙は、どの言語でも尋ねる前に生成したコードが確かめる。
60. **boto3 は dandori の `tools/wire` と同じ 1.43.103 にする（5.5）。** リポジトリのランナーが入れる boto3 を一つの版にする。
61. **生成するコードのコメントに書く守る操作は、`@guards` と同じ参照の書き方にする（2.6、5.3）。** パスはルートからで、`ritsu gen` もプロジェクトのルートを `sekisho` に渡す。同じ操作を、ファイルによって別の書き方で言わないためである。

段階 D の人が読むページ（D1）で決めたもの：

62. **ページは検査を通るファイルにだけ出す（7 章）。** `api` と `vectors` と同じにした。表は全部の組み合わせを歩いた結果から作るので、歩けないファイルには表が無い。
63. **ページのパスは、参照と同じくルートから書く。** `doc` も `--root` を取る。守る操作と計算した値の参照（2.6）と、頭のパスが、同じルートから読めるようにするためである。
64. **頭のハッシュは、ページの言語で `gen --target cedar` が書く四つのファイルのものにする。** sekisho が書く `@doc` と生成物の頭は `--lang` で変わる（16.1 の 35）ので、どの言語で生成したファイルのハッシュかを、ページに書く。
65. **埋め込んだ rulec と koyomi のページと、頭の Cedar のハッシュは、golden では置き換え、今の出力と別に突き合わせる。** rulec と koyomi のページの文や、生成する Cedar の `@doc` が変わるたびに、sekisho のページの golden を取り直さずに済むようにするためである。HTML では、ページを JSON で持ち、`<` を `\u003c` と書く。
66. **表の「〜以外」は、英字の値のあとだけ空白を入れる（`paid 以外`、`返金済以外`）。** 土台の `ja_spacing` で書く。段階 A の表の golden（`tests/walk/golden/refund_order.table.ja.md`）を取り直した。英語は変わらない。
67. **ワークフローがフローで呼ぶ操作の表（X16 の中身）は、dandori の答え（`Flows::operation_calls`）を受け取ったときだけ出す。** ページの関数は答えを `doc::FlowCalls` で受け取り、`ritsu sekisho doc` は dandori の口に尋ねる。ritsu-cross の X16 と同じ口の答えを読み、守る action は参照が同じものを探す（文字列を組まない）。
68. **役割と action の表の行は、役割を一つだけ持つ principal と、役割を持たない型の principal にする。** 役割の届く範囲（4.4）と同じ問いを、いつも許すか、組み合わせによるか、許さないかの三つで見せる。二つ以上の役割を持つ人の組み合わせは、職務の分離（E305）と action ごとの表が見せる。

段階 D の、言語をまたぐ検査とほかの言語のつなぎで決めたもの（2026-10-06）：

69. **口の問いは、ファイルをルートとルートからのパスで受け取る（8.3）。** `Gates::facts`・`Gates::allowed`・`Flows::operation_calls`・`Maps::published_operations` のどれも。答えの参照がルートからのパスだからである。`GateWorkflow` には別名を足し、`.flow` は参照にした。
70. **計算した値とワークフローの `@doc` の参照も、ルートからのパスで書く（5.1）。** `@guards` と口 `References` と同じ値にするためである。
71. **ルートの外の規則・日付・カレンダー・`use gate` のファイルは E201、`.flow` は E208 にする（2.6）。** 契約と同じく、参照にならないパスを、黙って別の形で書かないためである。ルートの外のファイルは読まない。
72. **X15 は、プロジェクトに `.gate` か Cedar の組が一つでもあるときだけ見る（4.6）。** sekisho を使わないプロジェクトの地図の全部のコンテキストに W907 を出さないためである。答えないゲートが属するコンテキストも見ない。
73. **X15 は、どのゲートの `guards` も数える（4.6）。** ほかのコンテキストの操作を守ることは、sakai の E211 が言う。二つで同じことを言わない。
74. **X16 は、ワークフローを書いたゲートの action が守る操作の呼び出しだけを見る（4.6）。** ほかのゲートは、そのワークフローを principal として知らない。
75. **W909 は、拒まれうるときと、許すかを決められないときの両方に出す（4.6）。** どちらもタスクが拒まれたときのエラーを宣言していないときだけで、決められないことを黙って通さない。
76. **W908 は境界に数えない（4.6）。** 呼び出しではなく、許しすぎを言う警告だからである。
77. **W910 は、ポリシーが読む `input` にだけ言う（10 章）。** ポリシーが読まない `input` は、生成した Cedar の `context` に入らない。
78. **手で書いた Cedar の役割は、尋ねる側が並べたものをそのまま数え、役割の中の役割はたどらない（1.3）。** だれがどこに入るかは、Cedar ではエンティティのデータが持つからである。
79. **`ritsu check` が読む Cedar は、地図の `owns` と要件が指す組だけにする（1.3）。** ルートの下の `.cedar` を探さない。sakai も、`cedar "…"` の項で書いたファイルだけを成果物にする（sakai の DESIGN 17 章）。
80. **`published_operations` は、AsyncAPI のチャネルと規則の Connect のサービスを返さない（8.3）。** X15 が求めず、`guards` で守るものでもないからである。
81. **定義の文の作り方（宣言の塊の行）は、dandori から ritsu-base の `definition` に移し、sekisho と dandori で一つにする（8.2）。**

段階 D3（八つ目の言語としての取り込み）で決めたもの：

82. **スキルのページは、言語の文書のコピーにする（12 章）。** `docs/reference.md` と `docs/codes.md` を `skills/sync.sh` がそのままコピーし、`SKILL.md` だけを手で書く。yuen と sakai と同じ形で、文書を直せばスキルも同じになり、`tests/skill.rs` が食い違いを見つける。生成したコードの呼び方は、koyomi の `generated-code.md` のような別のページにせず、`reference.md` の「What `gen` writes」の節に置いた。
83. **ブラウザで試すページで生成するのは Cedar だけにする（12 章）。** TypeScript・Python・Go のコードは、パッケージの `rules/` と `dates/` にある規則と日付の生成物を読むので、ゲート一つから生成すると、読む先のコードが無いまま出ることになる。Cedar の四つのファイルは、それだけで読め、`cedar` の CLI にそのまま渡せる。
84. **ファイルは ritsu-base の `fs` で読み書きする。** ブラウザで試すページは、ファイルをメモリに持って同じコマンドを走らせる（ritsu の DESIGN 4.15）。`check_file` と `gen` の書き出し、`use gate` の読み込みと同じファイルの見分けを `std::fs` から替えた。ディスクの上では答えは変わらない。
85. **README の数は、テストが数え直すものだけを書く。** 変異の数と日本語の名前のものの数は `tests/docs.rs` が数え直す。テストの件数と時間は書かない（時間は同じ機械でほかのビルドやテストが走っているかで変わり、件数はテストを足すたびに古くなる）。

2026-10-07 に決めたもの：

86. **規則に渡す列挙と真偽は、組み合わせの値と定数に限って rulec に尋ねる（3.2）。** ポリシーも読む列挙と真偽の属性は組み合わせの値に、定数はその値に、規則の列挙より値の少ないゲートの列挙はその値に限る。口の `outputs_over` の問いは数と日付の区間のまま、列挙は値の位置、真偽は 0 と 1 で渡す（口の型を変えず、rulec と sekisho の決まりにした）。前は、列挙と真偽を限れずに多めに数え、規則の答えに頼る期待を決められないとしていた。
87. **ワークフローが帳簿の振替を直接呼ぶ形は、X16 で確かめない（3.4、15 章）。** dandori が書くアクティビティはゲートに尋ねず、chobo の拒否の理由には、認可で拒まれたことに当たるものが無い。尋ねる者と、拒まれたときの答えが決まらないうちは、E908 と W909 が意味を持たない。帳簿の振替の操作を守る action は、帳簿を持つサービスが振替を動かす前に尋ねるときに効く。ブラウザで試すページの通販は、受注のワークフローが倉庫の契約の操作を呼び、帳簿は倉庫のシステムが動かす形にした。

2026-10-08 に決めたもの：

88. **率の値の文字は、`ritsu_units::Rat` の表示で書く（10 章）。** 診断が区間の端に書く率（`cells::show`）は、自前の書き方で小数の整数の部分から符号を取っていたので、−1% と 0% のあいだの率の符号が落ちていた（−0.5% が `0.5%`）。`Rat` の表示を直した（符号を先に書く）ので、それを使い、自前の書き方はやめた。刻みは小数なので、ほかの値の書き方は変わらない。

2026-10-11 に決めたもの（`allowed`。2.11）：

89. **`allowed` の答えは、二つの action をいっしょに歩いて数える。** 尋ねる action のポリシーが読むものを、尋ねる側の組み合わせの値に足し、答えを組み合わせごとに一つに決める。組み合わせごとに答えを参照の評価で決める形は、尋ねる側の組み合わせが尋ねる action の読む値を持たないとき、答えが決まらず、多めに数えることになる。
90. **同じ規則を同じ値で計算する値は、一つの値と数える。** 尋ねる側と尋ねる action が同じ規則を同じ入力で計算するとき（例の二つの `band`）、別々に数えると、起こりえない組（一方が上限まで、もう一方が上限超え）まで数える。
91. **この action のとる principal と resource の型は、どれも尋ねる action がとるものに限る（E110）。** 尋ねる action のとらない型を「許されない」と読む形は、Cedar に尋ねずに生成したコードが答えを作ることになり、sekisho が Cedar の意味に足すことになる（P1）。
92. **尋ねた action がリクエストを組み立てられずに拒めば、尋ねる側も同じ種類で拒む（3.8、5.3）。** 拒まれたことを「許されない」と読むと、`unless allowed approve` のポリシーが、データの誤りで許すことになる（P8）。生成したコードは、拒んだ理由の文の前に尋ねた action の別名を付ける。三つの言語で同じにした。
93. **期待だけが読む `allowed` は、Cedar に渡さず、生成したコードも尋ねず、E109 の輪にも数えない。** 期待は答えを読むだけで、どの判断も変えない。検査は、ポリシーが読むものと同じに数える。
94. **一つの action が同じ action を尋ねるのは、一通りの入力だけにする（E110）。** Cedar の context では `allowed_<別名>` が一つの値だからである。
95. **省ける入力（`?`）には、何も渡さなくてよい。** 何も渡らなければ、無いものとして尋ねる。省けない入力に何も渡らなければ E110 である。
96. **渡す属性は、この action のとる型のどれもが持つものに限る（E110）。** 型によって値が無くなる形は、省けない入力に渡せない型を作るからである。
97. **`today` を読む action を尋ねる action も、`today` を範囲で確かめる（5.3 の 2）。** 拒む順を、`today` を自分で読む action と同じにするためである。尋ねる action の関数も範囲の外の日を `today` で拒むが、それは、この action の入力とデータを確かめたあとになる。参照の評価も同じ順で拒む。
98. **`allowed` を条件の語として予約する（9.1。E002）。** E105 の再現（`docs/codes.md`）は、計算した値の名前を `allowed` から `known` に替えた。
99. **action の表に、尋ねる action の計算した値の列は出さない（7 章）。** 答えの列と、尋ねる action のポリシーが読む属性と関係の列で、行が読める。
100. **`allowed` の `@doc` の文は、ほかの計算した値の `@doc` の形にそろえた（5.1）。** 何をどう尋ねるかと、生成したコードが計算し、呼ぶ側からは受け取らないことを、同じ言い方で書く。
101. **`api` の context の項に `asks`（尋ねる action と、入力に渡すもの）を足した（11 章）。** 外のツールが、どの値がほかの action の答えかを、式の文字を読まずに分かるようにする。`computed` には `allowed approve(amount: amount)` を書き、`type` は `bool` である。
102. **`--authorizer avp` では、`allowed` を読む action の `<action>Request` も `avp` を受け取る（5.3）。** 尋ねる action を、同じ Verified Permissions のポリシーストアに尋ねるためである。
103. **生成した Python は、あとで読まない確かめを、変数に入れずに呼ぶ（5.3）。** 名前で渡す入力は、呼ぶ側が渡した値（`input['amount']`）を渡す。確かめたあとの列挙の値は `str` で、尋ねる action の入力の型（`Literal`）に合わないからである。
104. **例は、承認（`examples/approvals`）と文書（`examples/documents`）の二つを、英語の版と日本語の版で足した。** 承認の例の期待 `only_approvers_approve_or_ask_for_approvals` は、職員だけを持つ人を数えさせる。これが無いと、`approve` と `request_approval` のポリシーは approver しか読まず、職員だけを持つ人は役割を持たない人と同じ組み合わせになり、生成した Cedar の `principal in Office::Role::"approver"` を `"employee"` に替えた変異を、どの確かめも見つけられなかった（6.1）。

2026-11-15 に決めたもの（同じデータを読む規則の出力。4.1）：

105. **同じデータを読む二つ以上の規則の出力は、一緒にとる組を数える。** 出力ごとの値を掛け合わせた組を、正確なものとして数えない。掛け合わせは、同じ表が同じ行で書く二つの出力にも、起こりえない組（一方が low、もう一方が high）を作り、ページに載せ、期待を E304 で落としていた。
106. **同じ呼び出しの出力は、rulec に一緒に尋ねる（口の `Rules::outputs_together`）。** 表の行を見れば起こる組がそのまま分かり、具体的な入力で探すより確かで、問いの数も少ない。別の規則に同じ属性を渡す形は、一つの規則への問いでは答えられないので、sekisho の側で数える。
107. **区間を割る場所は、具体的な入力で探す。** rulec に区画を聞く問いを足さない。参照の評価器にかけた入力は、そのまま「その組が起こる」ことの証拠になり、規則の書き方（表、導出、`define`）によらず同じに働く。変わり目が一つの入力に沿わない規則では割れないが、そのときは決められないと言えばよい（108）。
108. **印を付けないのは、rulec の答えだけで言える組と、具体的な入力で起きた組だけにする。** 割り切れなかった領域の組は、起こるかもしれないものとして数え、印を付ける。それに頼る答えは W303 になり、E304 のような断定にはならない。
109. **規則が出さない組だけを選ぶ期待は W304 で言い、理由を足す。** `expect deny … when a is low and b is high` は、規則が `a` と `b` に同じ値を出すなら、選ぶ組が無い。成り立つが、ポリシーについては何も確かめていないので、エラーにはしない。注は、一緒にならない値と、それが何から計算されるかを言い、直し方は、規則の答えを確かめたいなら規則の `examples` に書くことを勧める（4.2）。同じ理由で当てはまらない permit と forbid の E303 にも、同じ注を足す。ゲートが渡す値に規則が答えない値を一つ求める行（`when side is apart`）も、その値と、何から計算するかを言う。
110. **試した入力の答えが変わらない領域は、rulec が添えた入力で割る（4.1 の 7）。** 一つの属性を二度渡す呼び出しで、二つが同じ値なら答えの変わらない規則は、具体的な入力では割る場所が見つからず、rulec が別々の入力として答えた値（起こらない）を多めに数えていた。その値に頼る期待は W303 になった。rulec が添えた入力は、その値がどこで出るかを言っているので、割る場所に使う。割ったあとの答えは rulec の解析のもので、印の付け方（108）は変わらない。rulec が何も添えなければ、前のとおり多めに数える。
111. **例は、数えた組み合わせをそのまま言い、確かめた入力を計算した値の横のかっこに添える（4.2）。** 計算した値だけの例は、その値になるリクエストを読む人に探させる。数えるときに評価器にかけた入力をそのまま使うので、rulec に尋ねる回数は増えない（規則の出力が一つのまとまりでも、入力を一つずつ評価器にかける）。組み合わせの区間を一点に替えることはしない。診断が数えているのは組み合わせで、替えると広さが読めなくなるからである。領域として言えるのは、数え方が持つ領域の全体が一つの組になるときだけで、そうでなければ一つの入力を言う。組ごとの領域をいつも言うには、組ごとに領域を割り直す問いが要るので、していない。JSON は、組み合わせを `example` に、確かめた入力を `example_inputs` に分けて持つ。
112. **`sekisho diff` の例は、数えた形のままにする（4.2）。** 試した入力は版ごとのもので、二つの版の例に混ぜない。

2026-11-17 に決めたもの（文の中の数。ritsu の DESIGN 6.2 の 8）：

113. **診断とページが言う数は、ゲートに書く形で書く（`cells::show`）。** 区切りは 5 桁からの `_` で、単位は書いたとおり（`10_000GBP`、`5000JPY`、`1_500_000円`）。書くのは ritsu の共通の関数（`ritsu_units::written::value`）で、言語をまたぐ検査の文も同じ関数で書く。前は `,` で三桁ごとに区切っていた（`10,000GBP`、`1,000JPY`）。`,` はゲートの字句が数の中に読まないので、文の数を行に写すと誤りになり、ほかの言語の文とも形が違った。率（刻みの個数を百分率で）と日付の書き方は変わらない。

### 16.2 危ないところ

- **計算した値を作る者を信じること（3.6）。** 守りはコードの形による。生成したコードを使わずに、自分で Cedar のリクエストを組み立てる人には効かない。README とページで、生成したコードを通すことを強く書く。
- **Verified Permissions の版のずれ。** ストアに置いたポリシーと、生成したコードの版が合わないと、計算した値の名前が食い違う。置く手順を生成しない（5.8）ので、合わせるのは使う人である。
- **数え方の予算。** 役割が多い（`roles` が 15 を超える）と、役割の組だけで 3 万を超え、ほかの値と掛け合わせると予算（10⁸）に届きうる。超えれば E307 で止まる。`roles` を型ごとに分けることと、役割ごとの確かめ（`can`）が手当てになる。
- **rulec と koyomi に新しい問いを足すこと（8.3）。** `outputs_over` は rulec の解析を区間に限って走らせるもので、rulec の中に手を入れる仕事になる。正確でない答えを「決められない」と言えるかが、sekisho の検査の正しさを決める。段階 A で作り、rulec のコーパスとテストの規則の 323 の問いを、区間の中の全部の入力を流した答えと突き合わせた（3.2）。2026-10-07 に、列挙と真偽の入力も値に限れるようにし、列挙か真偽の入力を持つ 15 の規則で、ベクタの値に入力を限った 358 の問いを、同じように突き合わせた（346 が正確で、12 は決められないと答えた。rulec の DESIGN 15.188）。rulec の E102 は、導出の取りうる値の外だけを求める行も止めるようになった（rulec の DESIGN 15.189）が、`define` の取りうる値と、`constraint` だけで死ぬ行は読まないので、E102 を通る行が、区間に限った問いでは落ちることがある（rulec の DESIGN 15.188）。
- **Cedar の実装の差。** 四つの実装で、決めたポリシーの名前の付け方が違った（6.2）。版が上がるたびに、生成したコードのテストで突き合わせる。cedarpy は AWS の公式ではない。
- **日本語の名前と Cedar。** Cedar に出るものは全部 ASCII の別名で、Cedar の側では、日本語は注釈（`@name` と `@doc`）にしか残らない。
- **判断と操作のあいだにデータが変わること。** `Store` から読んだ属性で許したあと、操作を行うまでに属性が変わりうる（注文の状態が返金済になる）。どの認可の仕組みにもあることで、sekisho が閉じられるものではない。生成するコードの文書に、操作と同じトランザクションの中で読むか、操作の側でもう一度確かめる（返金済の注文には返金しない、など）ことを書く。
- **秘密の値を context に入れること。** `input` が、契約で秘密と印を付けたフィールド（proto の `debug_redact`、OpenAPI の `x-data-classification` など）なら、その値は Cedar のリクエストと、Verified Permissions の判断の記録に残る。そういう `input` は、ポリシーが読むなら W910 になる（16.1 の 12、10 章）。ポリシーが読まない `input` は Cedar に届かないので言わない。
- **`allowed` で組み合わせが掛け算で増えること。** 尋ねる action のポリシーが読むものを、尋ねる側の組み合わせに足して数えるので、尋ねる action が多くのものを読めば、尋ねる側の組み合わせも増える。予算を超えれば E307 で止まる。
- **`allowed` で二度読むこと。** 生成したコードは、尋ねる action の関数で `Store` をもう一度読む。二つの読みのあいだにデータが変われば、尋ねる側が読んだ値と、尋ねた答えのもとの値が食い違うことがある（上の、判断と操作のあいだにデータが変わることと同じ種類のこと）。Verified Permissions では、`allowed` の数だけ `IsAuthorized` を多く呼ぶ。

## 17. 記号で書く単位（2026-10-21）

### 17.1 きっかけ

属性の型に `temperature[℃]` か `temperature[℉]` を書くと、E001（「`℃` is not part of the language」）で止まっていた。`mass[kg]` は通り、範囲の `>=-40℃` も、数のすぐあとなので読めた。温度の属性を書けないので、rulec の規則が `temperature[℃]` で受け取る値を属性から渡せず、冷蔵の荷の温度で出発を許すゲートが書けなかった。

### 17.2 原因

字句の読み手（`src/lex.rs`）は、`%`・`℃`・`℉` を、数のすぐあとの単位としてだけ読んでいた。語は文字か `_` で始まるものとして切り出すが、`℃` と `℉` は Unicode では文字ではなく記号なので、型の `[` のあとでは語にならず、知らない文字として E001 になった。単位の字の読み方は言語ごとに別々に書いてあり、そろっていなかった。rulec と kaname は `℃` と `℉` を語の始まりとして読み、dandori は型の `[` のすぐあとでだけ語にしていた。全角の `％` は、rulec と kaname が `%` として読み、sekisho と dandori は読まなかった。

### 17.3 決定

- 単位を書ける言語は、どれも rulec と同じ単位の字を読む。単位の字は ritsu-units の `chars` に一つだけ置き（ritsu の DESIGN 5.1）、sekisho の字句はそれを使う。数のすぐあとの単位は、記号一つ（`%`、`％`、`℃`、`℉`）か、名前に使える字の続き（`kg`、`USDc`、`万円`）で、全角の `％` は `%` として読む。
- `℃` と `℉` は、型の `[` のすぐあとでも一つの語にする（`temperature[℃]`、`temperature[ ℉ ]`）。名前の一部にはしない。名前は Cedar の名前と生成するコードの名前になり、記号はそこに使えないからである。数のすぐあとと型の `[` のすぐあとのほかに書いた `℃` と `℉` は E001 で、文はどちらかに書くよう言う。
- 数の前に無い `%` も E001 で、文は、率の型には `rate[step 1%]` のように刻みを書くよう言う。`%` は語にしない。rulec も `rate[%]` を読まず（rulec の E002）、率の型は刻みを書くからである（★）。
- 規則に渡すところでは、記号で書く単位も、文字で書く単位と同じく `Unit::same` で突き合わせる。前から単位の型で比べていたので、読めるようになれば、同じ決まりで比べる。`temperature[℃]` の入力に `temperature[℉]` の属性を渡せば E102 で、`rate[step 1%]` の入力に `rate[step 0.1%]` の属性を渡しても E102 になる。
- E102 などの文で規則の入力の型を書くときは、ritsu-units の単位の綴りで書く。率なら刻みまで書く（`rate[step 1%]`。前は rulec の型の綴りの `rate` で、刻みの違いが文から読めなかった）。
- 台帳の E001 の「いつ出るか」に、単位の記号を書ける所のほかに書いたときを足した。

### 17.4 捨てたもの

- `℃` と `℉` を、kaname のように、どこでも一字の語にすること。名前を書く位置の `℃` は構文の誤り（E002 など）になり、それが単位の記号だとは言えない。型の `[` のすぐあとにだけ許せば、ほかの所の `℃` は字句の段で E001 になり、どこに書けばよいかを言える。
- `rate[%]` を `rate[step 1%]` の別の綴りとして読むこと。rulec にも無い書き方で、rulec の読み方を基準にするかぎり、ほかの言語だけが読むわけにいかない。足すなら、rulec と ritsu-units の `Unit::parse` から足すことになる。

### 17.5 変わること

- `temperature[℃]`・`temperature[℉]` の属性と入力を書ける。Cedar に渡すのは、ほかの数と同じく型の単位で数えた `Long` で、スキーマの `@doc` は `temperature[℃], -40 to 40` のように書く。℉ の定数は、℃ の属性の単位で数える（`86℉` は 30）。
- 数のすぐあとの全角の `％` を `%` として読む。
- 前は E001 で止まったファイルのうち、`℃`・`℉`・`％` を型の `[` のすぐあとか数のすぐあとに書いたものは、検査を通る。名前の中の `℃` と、数の前に無い `%` は、前と同じく E001 で、文だけが変わる。前に検査を通ったファイルは、どれも同じに通る。
- 規則の率の入力に刻みの違う値を渡したときの E102 の文に、入力の刻みが出る。

### 17.6 確かめたこと

- 字句の単体テスト（`src/lex.rs`）。
- `tests/gen/cold_chain.gate` と日本語の版 `cold_chain.ja.gate`。規則（`tests/gen/rules/cold_chain_departure.rule`、`出発の可否.rule`）は、荷の温度帯と、-40℃ からの荷室の温度と、1% 刻みの湿度から出発してよいかを決める。ゲートは、温度と湿度の属性をその規則に渡し、温度を ℉ の定数（`86℉`、`5℉`）とも比べる。二つの版は、名前のほかは同じ Cedar と同じベクタになる（`tests/gen.rs`）。このゲートは、生成した Cedar を公式の CLI にかけるテスト（`tests/cedar.rs`）と、生成した TypeScript・Python・Go を三つの Cedar の実装にかけて参照の評価と突き合わせるテスト（`tests/typescript.rs`・`python.rs`・`go.rs`）、組み合わせの生の値のテスト（`tests/raw.rs`）にも入る。action `depart` の組み合わせは 136 通り（許すのは 38 通り）で、公式の CLI の `run-tests` はその 272 の試験を通す。生成した三つの言語のコードは、組み合わせから作った 280 の場合（許すもの 76、データの誤りで拒むもの 8）に、参照の評価と同じ答えを返す。
- 変異は英語と日本語の 4 組。名前の中の `℃`（`E001_a_sign_of_a_unit_in_a_name.gate`、`E001_名前の中の単位の記号.gate`）、`rate[%]`（`E001_a_rate_type_written_with_percent.gate`、`E001_百分率で書いた率の型.gate`）、℉ の温度の属性を ℃ の入力に渡すもの（`E102_a_temperature_in_another_unit.gate`、`E102_ほかの単位の温度.gate`）、0.1% 刻みの湿度を 1% 刻みの入力に渡すもの（`E102_a_rate_in_another_step.gate`、`E102_ほかの刻みの率.gate`）である。
- ritsu の `tests/cross.rs` は、同じゲートを規則とフローと kaname のモジュールと一緒に一つのプロジェクトにして `ritsu check` にかけ、℉ の属性と 0.1% 刻みの属性を E102 と言うことを確かめる。ritsu の `tests/gen.rs` は、そのプロジェクトのパッケージ（`Celsius` の型を持つ）を tsc・mypy・go vet にかける。ritsu の `tests/units.rs` は、単位の表の全部の綴り（107）と `％` を、sekisho を含む五つの言語が、型と範囲に書いて読むことを確かめる。
- 直す前と後の突き合わせ。替える前のバイナリ（0.33.0 のまま組んだもの）と後のバイナリで、木の `.gate` の全部（172。足した 10 を含む）の `check`（英語と日本語と JSON）と、検査を通るものの `gen` の四つの生成先（書いたファイルの中身まで）、`doc` の英日、`vectors`、`api` を比べた。前からある 162 の出力 1,146 は、一字も変わらない。変わったのは足した 10 だけで、前のバイナリでは、どれも `℃` か `%` の E001 で止まる。

### 17.7 見つけて直していないもの

- 刻みの無い `rate` の属性は、整数一つが 100% を数える（`Unit::rate(None)`）。範囲の `>=0% <=100%` は通るが、比べる定数は 0% と 100% のほかは整数にならず E103 になる（`when resource.x <= 50%`）。rulec の率の入力、dandori、kaname の引数は刻みを求める。木のゲートに率の属性は、この節の材料のほかに無い（19 章で、E103 の注が型に刻みを書くよう言うようにした）。

## 18. 注が並べる操作と引数（2026-10-21）

### 18.1 きっかけ

`guards` の行に契約に無い操作を書くと、E202 の注は、契約にある操作を並べる。操作は OpenAPI の `operationId`（無ければメソッドとパス）、AsyncAPI の操作のキー、proto の `Service/Method`、帳簿の `<振替>.<操作>` で、注はどれも引用符なしで並べていた。`operationId` には空白や `-` を含む名前も書け、`operationId` の無い操作はメソッドとパスで呼ぶ。そういう操作は `guards` の行に `"…"` で囲んで書くのに、注は「Its operations are refundOrder, GET /orders/{id}, add note and orders.hold.」のように並べ、どこで操作が切れるかも、どれを引用符で囲むかも、文から読めなかった。注のとおりに `guards orders add note` と書くと、検査は E005（`note` はここに書けない）で止まった。E204 の注も、操作の引数（`order-id`、`page size`）を引用符なしで並べていた。

### 18.2 原因

注は、操作と引数の名前をそのまま `Text::list` でつないでいた。sekisho には、名前を `.gate` の行にどう書くかを決める関数が無かった。見出しの `operation refundOrders` は ritsu-base の `naming::word_or_quote` で書いているが、これは参照の中の名前の決まりで、`.gate` の字句の決まりとは違う。参照の決まりでは `refund-order` も `orders.hold` も引用符なしの語だが、`.gate` の字句は `-` と `.` で語を切る。

### 18.3 決定

- 注が `.gate` にそのまま書ける操作や引数の名前を並べるときは、行が書く形で並べる。字句がその名前を一つの語として読み直せるときだけ引用符なしにし、ほかは `"…"` で囲む（中の `"` と `\` は `\"` と `\\`）。読み直せるかは、字句（`lex::lex`）そのものに、その名前だけを書いた字下げした行を読ませて決める（`lex::is_bare_word`、書く形は `lex::spelled_word`）。sakai の 21 章と yuen の 21 章と同じ考えで、字句の決まりを字句の外にもう一度書かない。
- `guards` の行は、`.` でつないだ二つの語（帳簿の `receive.do`）も一つの操作として読む。E202 の注は、操作が一つの語か、`.` でつないだ二つの語として読めるときは引用符なしで並べる（`contracts.rs` の `operation_word`）。`.` を一つ含む `operationId`（`orders.hold`）も、同じ形で書ける。
- 使うのは、E202 の三つの注（文書の操作、proto のメソッド、帳簿の振替の操作）と、E204 の注（パスとクエリの引数、proto ならリクエストのフィールド）である。proto のメソッドは `/` を含むので、いつも `"Orders/Refund"` の形になる。`guards` の行も、proto のメソッドは文字列でしか書けない。
- 並びの区切りは `Text::list` のまま（日本語は `、`、英語は `, ` と最後の ` and `）。引用符なしの語は文字・数字・`_` だけからなり、どちらの区切りも含まない。
- 見出しの `operation refundOrders` は、これまでどおり参照の決まりで書く（2.6。yuen の E202 と同じ書き方）。

### 18.4 捨てたもの

- 字句の決まりを、文字の種類で書いた関数として字句の外に置くこと。字句は数（`5kg`）と日付を名前より先に読み、ASCII の数字で始まる語（`2fa`）を E001 にするので、二つ目の決まりは字句と食い違いうる。
- 参照の決まり（`naming::word_or_quote`）で並べること。`refund-order` を引用符なしで並べることになり、そのまま書くと読めない。
- 名前を並べるほかの注（E101 の、書ける役割・型・規則の出力と入力と列挙の値など）も同じ形にすること。その位置は語しか受け付けず、引用符で囲む形が無い（18.7）。

### 18.5 変わること

- E202 と E204 の注は、一つの語として読めない操作と引数を `"…"` で囲んで並べる。proto のメソッドは、いつも `"Orders/Refund"` の形になる。前は引用符なしで、注のとおりに書くと読めなかった（`add note` と `order-id` は E005、`2fa` は E001）。
- 例とテストの材料には、一つの語として読めない操作も引数も無いので、`check` の出力は変わらない（18.6 の突き合わせ）。文が変わるのは、proto のメソッドを並べる E202 の注で、`tests/contracts.rs` の期待を直した。
- 台帳の E202 と E204 の直し方に、注が行にそのまま書ける形で並べることを足した（`sekisho explain`、`docs/codes.md`、`docs/codes.ja.md`、スキルのコピー）。
- 前に検査を通ったファイルは、どれも同じに通る。

### 18.6 確かめたこと

- `src/lex.rs` の単体のテストは、引用符なしで書ける名前と書けない名前を並べ、書けない名前を引用符つきで書いた `guards` の行が、同じ名前の文字列として読み直せることを確かめる。
- `tests/contracts.rs` の `the_operations_and_parameters_a_note_lists_are_written_as_the_line_takes_them` と日本語の版：OpenAPI の文書に、引用符なしで書ける操作（`refundOrder`、`.` でつないだ `orders.hold`、日本語の版は `返金する`、`注文.保留`、`3日以内`）と書けない操作（`operationId` の無い `GET /orders/{id}`、`add note`、`release,partly`、`refund-order`、`2fa`、`say "hi"`、日本語の版は `返金 一部`、`保留、手動`、`出荷-一部`）を置く。無い操作を守ると、E202 の注がそれぞれを行が書く形で並べる。並びを引用符の外の区切りで分け、操作ごとに一つの action の `guards` の行にそのまま書くと、検査が通る。引数（`order-id` と `page size`、日本語の版は `注文-番号` と `件数 上限`）も、E204 の注が引用符つきで並べ、`from` のあとにそのまま書くと通る。
- 替える前と後の突き合わせ：替える前のバイナリ（この変更の前の木のまま組んだもの）と後のバイナリで、木の `.gate` の全部（182）に、`ritsu sekisho check`（英語と日本語、それぞれ JSON も）と sekisho のクレートのバイナリの `check`（英語と日本語）をかけた 1,092 の出力は、一字も変わらなかった。木の契約には、一つの語として読めない操作も引数も無く、proto のメソッドを並べる E202 も無い。

### 18.7 見つけて直していないもの

- 規則の出力・入力・列挙の値を並べる E101 の注は、この変更では引用符の形にしていない。その位置（計算した値の入力と、渡す値と、`is` の右辺）は語しか受け付けず、引用符で囲む形が無いからである。sekisho の字句が名前として読まない規則の名前（列挙の値 `O'Brien(obrien)` の `O'Brien`）は、20 章で、`.gate` にそのまま書ける別名（`obrien`）で並べるようにした。
- E203 の注は、操作が受け取る列挙の値のうち gate の列挙に無いものを並べる。gate の列挙の値と別名は語だけなので、`in transit` のような値は gate に足せない。並べているのは操作が受け取るもので、`.gate` に書く値ではないので、この変更では変えていない。

## 19. 刻みの無い率の E103（2026-10-21）

### 19.1 きっかけ

刻みの無い `rate` の属性は、整数一つが 100% を数える（17.7）。範囲の `>=0% <=100%` は通るが、`when share <= 30%` のように比べる定数は、0% と 100% のほかは整数にならず E103 になる。E103 の文は「定数 `30%` を rate で数えると、整数になりません」と言い、注は「値は型の単位で数えた整数として Cedar に渡る」と言うだけだった。何を直せばよいか、つまり率の型に刻みを書けばよいことが、文から分からなかった。

### 19.2 原因

E103 の文と注は、どの単位にも同じものだった。刻みの無い率では、整数一つが 100% を数えることも、型に刻みを書けば定数が整数になることも、言っていなかった。

### 19.3 決定

- 定数が刻みの無い率の値として整数にならないときは、E103 に注を一つ足す。注は、刻みの無い `rate` は 100% を 1 と数えるので、書ける定数は 0% や 100% のような 100% の整数倍だけであることと、その属性か入力の型に刻みを書くことを言う。示す刻みは、`1%`、`0.1%`、`0.01%` … のうち、その定数が整数になる一番粗いものにする（`30%` なら `rate[step 1%]`、`12.5%` なら `rate[step 0.1%]`。`types::step_for`）。
- 注を足すのは、定数が整数にならないと言うところの全部である（`names.rs` の `miss`）。属性と入力の範囲の端、条件で比べる定数、`allowed` でほかの action の入力に渡す定数がそれに当たる。規則の率の入力は、rulec が刻みを求める（rulec の §15.139）ので、刻みの無いものは来ない。注が挙げる名前は、範囲の端なら属性か入力の名前、条件なら左辺（`resource.discount`）である。
- 率の型の決まりは変えない。刻みは rulec と同じく `rate[step 1%]` のように書き、刻みの無い `rate` は、これまでどおり整数一つが 100% を数える。`rate[%]` は型にしない（17.4）。
- 台帳の E103 の「いつ出るか」に刻みの無い率の場合を、「直し方」に率の属性と入力には刻みを書くことを足した。

### 19.4 捨てたもの

- 刻みの無い `rate` の属性と入力を、rulec の率の入力（rulec の §15.139）のように、それだけで E103 にすること。0% と 100% とだけ比べるゲートは、いま通っていて意味もあり、止めると、前に通っていたファイルが落ちる。
- 刻みの無い `rate` を 1% 刻みとして数えること。Cedar に渡す整数と、生成するコードが入口で確かめる範囲が変わる。rulec と ritsu-units の率の決まり（`Unit::rate(None)`）から変えることになる。
- 注で、いつも `rate[step 1%]` を示すこと。`12.5%` のような定数は 1% 刻みでも整数にならず、注のとおりに直すと、刻みの付いた率で同じ E103 になる。

### 19.5 変わること

- 刻みの無い率の値として整数にならない定数の E103 に、注が一つ増える。同じファイルが、前と同じコードで止まる。
- 例とテストの材料には、刻みの無い率の属性も入力も無いので、`check` の出力は変わらない（19.6 の突き合わせ）。前に検査を通ったファイルは、どれも同じに通る。

### 19.6 確かめたこと

- `src/types.rs` の単体のテスト：刻みの無い `rate` で `100%` は 1、`0%` は 0 で、`30%` は整数にならない。`step_for` は `30%` と `150%` と `-5%` に `1%`、`12.5%` に `0.1%`、`0.25%` に `0.01%` を示し、どの定数も、示した刻みの率では整数になる。
- `tests/names.rs` の `a_rate_with_no_step_is_told_to_write_one`：刻みの無い率の属性で、範囲の `>=0% <=100%` と `100%` との比べ方は通る。`30%` と `12.5%` との比べ方と、範囲の端の `30%` は E103 で、注は英日とも、示す刻み（`rate[step 1%]`、`rate[step 0.1%]`）と名前（`resource.discount`、`discount`）を言う。型を注のとおりにすると、どれも通る。刻みのある率の E103 には、この注は付かない。
- 変異は英語と日本語の 1 組。返金する割合を刻みの無い率の入力にして、係の permit を `30%` と比べる（`E103_a_rate_with_no_step.gate`、`E103_刻みの無い率.gate`）。
- 替える前と後の突き合わせ：替える前のバイナリと後のバイナリで、木の `.gate` の全部（184。足した 2 を含む）に、`ritsu sekisho check`（英語と日本語、それぞれ JSON も）と sekisho のクレートのバイナリの `check`（英語と日本語）をかけた 1,104 の出力のうち、変わったのは足した二つの変異の 12 だけで、どれも E103 に注が一つ増えたことである。替える前のバイナリでも、二つの変異は同じ E103 で止まる。

### 19.7 見つけて直していないもの

- 刻みのある率に、その刻みの整数倍でない定数を書いたとき（`rate[step 1%]` に `12.5%`）の E103 は、これまでどおりの注だけで、細かい刻みを示さない。文は型を刻みまで書く（`rate[step 1%]` で数えると整数にならない）ので、何が合わないかは文から読める。

## 20. 注が並べる規則の名前（2026-10-24）

### 20.1 きっかけ

18.7 で見つけて残したもの。E101 の注は、規則の出力・入力・列挙の値を、規則での名前で並べていた。規則の名前には、sekisho の字句が名前として読まない字（`'`、`＆`、`＋`、全角の丸括弧）が入りうる。売り場の列挙が `Home | Men's(mens) | Women's(womens) | Children's(childrens)` の規則は rulec の検査を通る。計算した値でその入力にどの値でもない語（`men`）を渡すと、E101 の注は「The values there are `Home`, `Men's`, `Women's` and `Children's`.」と並べた。注のとおりに `Men's` と書くと E001（「`'` is not part of the language」）になる。その位置（計算した値の入力に渡す値、`is` の右辺）は語しか受け付けないので、引用符で囲んでも E005 になり、書けるのは別名の `mens` だけだった。

### 20.2 原因

注は、規則での名前（ポートの `name`）をそのまま並べていた。gate は、規則の列挙の値を三つの名前で書ける。規則での名前、`.rule` が丸括弧に書く別名、生成するコードのメンバーである（`is_rule_value`）。規則の入力と出力は、名前と別名の二つで書ける。注はそのうちの一つしか並べず、それを `.gate` に書けるかも確かめていなかった。

値の位置には、もう一つの決まりがある。条件と計算した値に使う語（`today`、`in`、`any` など、`kw::RESERVED`）は、構文解析がそこでほかのものとして読む。注が並べた値の `today` を書くと `today` の行を求められ（E107）、`in` と `any` は E005 になった。rulec は、こういう語を列挙の値として通す（rulec が値にさせない語は `not`、`none`、`true` などの 11 で、sekisho の語とは違う）。

### 20.3 決定

- E101 の注が規則と日付のファイルの名前を並べるときは、`.gate` のその位置にそのまま書ける形で並べる。そのファイルでの名前、そのファイルが丸括弧に書く別名、列挙の値なら生成するコードのメンバー、の順に見て、最初に書けるものを並べる（`names::writable`、`foreign_notes`）。書けるかどうかは、字句（`lex::is_bare_word`）がそれを一つの語として読み直せるかで決め、値の位置ではさらに `kw::RESERVED` の語でないことを求める（★）。18 章の E202・E204 と同じく、字句の決まりを字句の外にもう一度書かない。違うのは、E202・E204 の位置は引用符で囲んだ文字列も受け付け、E101 の位置は語しか受け付けないことで、書けない名前は引用符で囲まず、ほかの名前で並べる。
- 名前の位置と値の位置を分ける。入力（`<入力>: …` の左）、出力（`.<出力>`）、日付（`<日付のファイル>.<日付>`）では、構文解析がどの語も名前として読むので、`in` のような語もそのまま並べる。値の位置（入力に渡す値、`is` の右辺）では、値の `today` はメンバーの `Today` で、別名 `in_store` を持つ値の `in` はその別名で並べる。
- どの名前でも書けないもの（別名の無い `Children's`、別名にも `'` のある `Ma'am(ma'am)`）は、書ける名前の並びから外す。そして別の注で、ここには書けないことと、そのファイルの側で英字・数字・`_` だけの別名を付けることを言う（★）。並びに書けない名前を混ぜると、注のとおりに書いたファイルが読めない。rulec は、こういう名前も検査で通す。rulec の E011 は列挙の値に別名を求めず、別名の字も問わない（`o'clock(o'clk)` の入力も通る）。
- 使うのは、計算した値の E101 の注の全部である。規則の出力、規則と日付の入力、規則の列挙の入力に渡す値、規則の列挙の出力と比べる `is` の右辺、日付のファイルの日付である。境の検査（`borders.rs`）が同じ E101 を言うところも、同じ関数で並べる。そこは名前の検査が先に言うので、ふだんは出ない。
- 台帳の E101 の直し方に、規則と日付のファイルの名前は書ける形で並ぶことと、書けないものにはそのファイルの側で別名を付けることを足した（`sekisho explain`、`docs/codes.md`、`docs/codes.ja.md`、スキルのコピー）。

### 20.4 捨てたもの

- いつも別名で並べること。日本語の規則では、注が `上限まで` ではなく `within_limit` を並べることになり、規則を読む人には分かりにくい。gate は規則での名前でも書け、木の材料もそう書いている。書ける名前なら、注は前と同じに並べる。
- 書けない名前も並びに残し、印を付けること（`Children's`（書けません））。注の文は「書ける値は」と言っているので、並びには書けるものだけを入れる。
- 別名で並べる名前に、規則での名前を添えること（`mens`（`Men's`））。`.rule` は名前と別名を並べて書く（`Men's(mens)`）ので、別名だけでも規則を読む人には対応が分かる。添えると、書く形だけを並べる 18 章の E202 とそろわない。
- どの字がなぜ書けないかを注の文で言うこと。値の位置の `today` のように、字ではなく構文が読めない名前もあり、直し方（別名を付ける）はどれも同じである。

### 20.5 変わること

- E101 の注は、規則と日付のファイルの名前のうち `.gate` に書けないものを、別名かメンバーで並べる。どの名前でも書けないものは並べず、別の注で別名を付けるよう言う。前は規則での名前をそのまま並べ、注のとおりに書くと E001 か E005 か E107 になった。
- 例とテストの材料の規則の名前は、どれも `.gate` に書けるので、`check` の出力は変わらない（20.6 の突き合わせ）。前に検査を通ったファイルは、どれも同じに通る。

### 20.6 確かめたこと

- 変異は英語と日本語の 1 組（`E101_values_a_gate_writes_by_an_alias.gate`、`E101_別名で書く値.gate`）。読む規則は `tests/mutants/read/refund_desk.rule` と `返金の窓口.rule` で、どの窓口が返金するかを、売った売り場と開封したかで決める。英語の版は売り場の値の三つを `'` で、日本語の版は二つを `＆` で名付け、どれにも別名がある。gate は、売った売り場に `men`（日本語の版は `紳士`）を渡す。
  - 替える前のバイナリでは、注が並べる四つの値を一つずつ書くと、書けない字のある値（`Men's`・`Women's`・`Children's`、`ホーム＆ガーデン`・`ベビー＆キッズ`）が E001 になった。
  - 替えた後は、注は `Home`・`mens`・`womens`・`childrens`（日本語の版は `紳士服`・`婦人服`・`home`・`baby_and_kids`）を並べ、どれを書いても、ファイルは検査を通る（英語と日本語の出力の両方で確かめた）。
  - どの名前でも書けない値（別名の無い `Children's`）は、木に置く材料にしていない。そういう名前は、生成するコードがそのままでは使えない名前でもあり、rulec が W121 で言う（日本語の名前には、いまでも Connect の W121 が出る）か、言うべきもの（20.7）なので、木の規則を見回す rulec の試験（`tests/own_names.rs`、`tests/spellings.rs`）に当たる。その形は、次の試験が一時ディレクトリに書く規則で確かめる。
- `tests/names.rs` の `the_names_of_a_rule_a_note_lists_are_written_as_the_gate_takes_them` と日本語の版：規則の名前を書く四つの位置（列挙の入力に渡す値、列挙の出力と比べる `is` の右辺、入力、出力）ごとに、書けない名前を混ぜた規則を読み、注が並べる名前と、書けないと言う名前を、英日の注で確かめる。英語の版の規則は、`'` のある名前と別名、別名の無い値、値の `today`・`in`・`any`（`Today`・`in_store`・`Any` で並ぶ）、入力の `in`（名前の位置なので `in` で並ぶ）を持ち、日本語の版の規則は、`＆`・`＋`・全角の丸括弧のある名前を持つ。注が並べる名前をそれぞれの位置に書くと、名前の検査が通る。`an_input_the_gate_cannot_write_is_said_to_be_one` は、名前にも別名にも書けない字のある入力（`o'clock(o'clk)`、`受付時刻（任意）(受付時刻（任意）)`）を、注が書けないと言うことを確かめる。
- `src/names.rs` の単体のテスト：書く形の選び方（名前の位置と値の位置、日付の名前の `5坪`。koyomi は名前として読み、`.gate` は単位 坪 の数として読むので、別名で並ぶ）と、注の文。
- 替える前と後の突き合わせ：替える前のバイナリ（この変更の前の木のまま組んだもの）と後のバイナリで、木の `.gate` の全部（188。足した 2 を含む）に、`ritsu sekisho check`（英語と日本語、それぞれ JSON も）と sekisho のクレートのバイナリの `check`（英語と日本語）をかけた 1,128 の出力のうち、変わったのは足した二つの変異の `ritsu sekisho check` の 8 だけである。クレートのバイナリは、二つの変異に、前も後も同じ E209 を言う。

### 20.7 見つけて直していないもの

- 計算した値の入力に渡す語は、まず action の入力か計算した値の名前として読まれ、そうでないときに規則の列挙の値として読まれる。注が並べる値が action の入力と同じ名前なら、そう書くとその入力を渡すことになる。木の材料にそういう名前は無く、この変更では注を変えていない。
- 値の無いことがある規則の入力（`bool?`）は、名前の検査では渡さなくてよい（E105 にならない）が、境の検査は、渡していないことを E105 と言う（「The rule `t` is not given its input opened」）。替える前から同じである。どちらに合わせるかは、規則の参照の評価が渡されない入力をどう扱うかを決めることになるので、この変更には入れていない。
- 規則の入力に値を渡していないときの E105 の文と、順序のある列挙の範囲の外になりうる値の E206 の文は、規則での名前をそのまま書く（「The input `o'clock` of `t` is given nothing」、「… of the rule `t` (Sir..Lord's)」）。どちらも、並びではなく、どの入力かと、規則が書く範囲を指す名前なので、この変更では変えていない。E206 の注は範囲の中の値を定数で書くよう言うが、範囲の端の `Lord's` は、そのままでは書けない。
- rulec は、`'` を含み別名の無い列挙の値（`Children's`）にも、`'` を含む別名（`o'clock(o'clk)`）にも、何も言わない。生成したコードは、値のメンバーを `SectionChildren's`（Go）、`CHILDREN'S`（Python と TypeScript）と、入力を `O'clk`（Go）、`o'clk`（Python）と書き、組めない。ASCII でない名前なら、Connect の W121 が言う形である。rulec の検査の穴で、sekisho の変更ではないので、ここでは直していない。

## 21. サイトのコードの色（2026-10-24）

### 21.1 きっかけ

ritsu のサイトの sekisho のページ（`website/docs/sekisho.md` と `docs-ja/sekisho.md`）は、`gate` の塊を五つずつと、sekisho が書き出す Cedar のポリシーとスキーマの塊を一つずつ見せるが、どれにも色が付いていなかった。chobo のサイトで帳簿をほかの言語と使うページ（`together`、英日）の `gate` の塊も同じだった。ほかの言語の塊（`kn`、`rule`、`flow`、`book`）には、組むときに Pygments が色を付ける（ritsu の DESIGN 13.2）。

### 21.2 原因

sekisho の字句の定義が無かった。Pygments（2.21.0）にも Cedar の字句の定義は無い。Pygments は字句の定義を知らない塊を `text` として組むので、二つのページの 14 の塊と、chobo の二つのページの二つの塊は、全体が一色だった。

### 21.3 決定

- kaname の `knlexer.py` と同じ書き方で、`website/tools/gatelexer.py`（`gate`）と `website/tools/cedarlexer.py`（`cedar` と `cedarschema`）を足す。モジュールを Markdown の拡張として設定に挙げると読み込まれ、読み込まれると Pygments の表に自分を載せる。ritsu のサイトの本体の二つの設定は二つを挙げる。chobo のサイトの二つの設定は `gatelexer` を挙げ、chobo の `build.sh` は `website/tools` を `PYTHONPATH` に置く。rulec と dandori のサイトはゲートも Cedar も見せないので、つながない。
- ゲートの語には、構文解析がその位置で読むとおりの色を付ける（9 章）。
  - 行の頭の語と、ブロックの行の頭の語は、キーワードにする。行の頭でだけキーワードなので、同じつづりの名前（列挙の値 `open`、属性 `description`、入力 `range`）は、ほかの位置では名前のままである。`use` のあとの種類と `from`、ワークフローの行と action の `resource` の行の `from`、`range` と `offset`、`expect` のあとの `allow` と `deny`、`today is open in` の `open`、率の型の `step` も、その位置でだけキーワードにする。
  - 予約語（`kw::RESERVED`）は名前にならない（E002）ので、どこでも同じ色にする。`bool` `date` `number` `rate` は型、`true` `false` は定数、`and` `or` `not` `in` `is` は演算子と同じ種類にし、残りはキーワードにする。`[` が続く `money` と量の語、型の角括弧の中の `incl_tax` と `excl_tax` も型にする。
  - 宣言が付ける名前（列挙、役割、principal、ワークフロー、resource、action、ポリシー、期待、職務の分離）、ファイルの名前と名前空間と `use` が付ける名前、名前の ASCII の別名、単位の付いた数と日付と `offset` のあとのオフセット、文字列、コメント、演算子、区切りも、それぞれ種類を分ける。色そのものは、テーマの明るい配色と暗い配色が種類ごとに決める。
  - `:` の前の語は、フィールド、属性、入力、引数の名前である。字下げした行の頭で `=` の前にある語は、計算した値の名前である。`.` のあとの語は属性である。`guards` の行の語、列挙の値、action の `resource` の行が並べる型と `from` のあとの引数も、名前である。どれも、つづりがキーワードと同じでも名前にする。構文解析は、`attributes`・`input`・`context` の下の行を字下げの深さで見分けるが、字句の定義は行の形で見分ける（頭の語のあとが `:` ならフィールド、`=` なら計算した値）。ブロックの行は頭の語のあとに `:` も `=` も書かないので、検査を通るファイルでは同じになる。
  - 数は `lex.rs` のとおりに読む。数字（あいだを `_` で区切ってよく、`.` のあとに小数を書ける）のすぐあとに、単位の記号を一つか、単位を書く。単位は、桁の語（`万` `億`）のあとの、ASCII の英字と数字の並びか、ほかの文字の単位（`円` `銭` `坪`）一字で、そのあとに名前の字が続かないものである。続けば、数字で始まる名前（`3日以内`、`100万`）として読む。`lex.rs` が読まない Cedar の `==` と `||` は、読めない字にする（E001）。
- Cedar の語は、ritsu が読む Cedar 4.13.0 のものにする（ritsu の DESIGN 4.18）。`v4.13.0` のタグの二つの文法（`cedar-policy-core/src/parser/grammar.lalrpop` と `cedar-policy-core/src/validator/cedar_schema/grammar.lalrpop`）の `match` が挙げる語と区切りで、Cedar の文法の文書（Policy grammar と Cedar schema grammar）が終端記号として書くものでもある。
  - ポリシーでは、`permit` と `forbid`、`when` と `unless`、`if` `then` `else`、四つの変数（`principal` `action` `resource` `context`）をキーワードにし、`true` `false` は定数、`in` `has` `like` `is` は演算子と同じ種類にする。エンティティ型（`Shop::Role::"clerk"` の `Role`、`is` のあとの型）、名前空間、拡張の関数、注釈（`@id`）、テンプレートのスロット（`?principal`）も、それぞれ種類を分ける。
  - スキーマでは、宣言の語（`namespace` `entity` `action` `type` `in` `appliesTo` `tags` `enum` `attributes` と、`appliesTo` の中の `principal` `resource` `context`）をキーワードにし、組み込みの型（`Long` `String` `Bool`）と `Set` は型、宣言する名前と、宣言や属性の型の名前はエンティティ型と同じ種類にする。
  - `.` と `has` のあとの語と、`:` の前の語（レコードのキー、スキーマの属性）は、つづりによらず名前にする。
- 字句の定義が持つ語の一覧は、元の一覧のコピーにし、元を持つクレートの試験で、一字も違わないことを確かめる。
  - `gatelexer.py` は、`kw.rs` の `TABLE`（位置ごとの語）と `RESERVED` を、同じ名前で、語も順も同じに持つ。ほかに、ritsu-units の語（`Tax` の二つと、率の刻みの `step`）、単位の字（`ritsu_units::chars` の `is_unit_char` が ASCII の英字と記号のほかに取る字、`is_sign`、`is_word_sign`）、`lex.rs` の `unit_after` の桁の語を持つ。sekisho の `tests/docs.rs` の `the_site_colours_the_words_of_the_language` が、どれも元と一字も違わないことと、単位の表のどのつづりも、字句の定義が単位として読む形であることを確かめる。
  - `cedarlexer.py` は、二つの文法の語、予約語、区切りと、Cedar のバージョンを持つ。ritsu-base の `tests/cedar.rs` の `the_site_colours_the_words_these_readers_read` が、どれも `ritsu_base::cedar` に合わせる。ポリシーの語は `Ident::from_word` がキーワードとして読む語、スキーマの語は `is_word` で読む語と、スキーマを Cedar の形で書くときの組み込みの型、予約語は `RESERVED`、区切りは字句が文法ごとに読むもの（`POLICY_PUNCT`、`SCHEMA_PUNCT`）、バージョンは `ritsu_base::cedar` が自分の説明に書く版である。
  - 言語に語を足して字句の定義に足し忘れれば、そこで落ちる。ritsu が新しい版の Cedar を読むようにして、字句の定義を直し忘れたときも落ちる。

### 21.4 捨てたもの

- 字句の定義を `kw.rs` から生成すること。サイトは Zensical だけで組める形にしてあり、ほかの四つの字句の定義も、手で書いて試験で元に合わせている（kaname の DESIGN 31 章と同じ）。
- 単位の表を字句の定義に持つこと。数のあとの並びが単位かどうかは表で決まるが、表のコピーは百を超えるつづりになる。ASCII の並びは、表に無ければ E001 なので、検査を通るファイルでは単位である。ほかの文字の単位は一字ずつなので、そのあとに名前の字が続かないことで、`5円` と名前の `5円引き` を分けられる。
- Cedar の語を、いま公開されている文法の文書から取ること。スキーマの文法の文書には、4.13.0 の文法に無い `external` がある。ritsu が読まない語に色を付けると、ritsu が読めない Cedar が、読めるように見える。
- Cedar の拡張の関数（`ip`、`decimal` など）と拡張の型の名前を、一覧に持つこと。文法ではなく拡張が決めるもので、関数は `(` の前にあることで分かる。sekisho が書く Cedar は拡張を使わない（3.7）。
- `cedar` と `cedarschema` を一つの字句の定義にすること。二つの文法は、語も区切りも違う（スキーマには `?` があり、`==` などの演算子が無い）。

### 21.5 変わること

ritsu のサイトの sekisho のページ（英日）の `gate` の塊五つずつと Cedar の塊二つずつ、chobo のサイトの `together` のページ（英日）の `gate` の塊一つずつに色が付く。この変更の前と後に `website/build.sh` で組んだ木を突き合わせると、405 のファイルのうち変わったのはこの四つのページだけで、変わったところは 16 の塊の中だけ、塊の文字は一字も変わらない。

### 21.6 確かめたこと

- sekisho の `tests/docs.rs` の `the_site_colours_the_words_of_the_language`。字句の定義の `TABLE` から語を一つ消す、位置の名前を変える、`RESERVED` から語を一つ消す、`TAX` から語を一つ消す、`STEP` の語を変える、`UNIT_LETTERS`・`SIGNS`・`WORD_SIGNS`・`MULTIPLIERS` から字を一つ消す、の九つで、それぞれ落ちることを確かめた（字句の定義は元に戻した）。
- ritsu-base の `tests/cedar.rs` の `the_site_colours_the_words_these_readers_read`。ポリシーの語を一つ変える、スキーマのキーワードと組み込みの型を一つずつ消す、予約語の順を変える、ポリシーの区切りの順を変える、スキーマの区切りを一つ消す、Cedar のバージョンを変える、の七つで、それぞれ落ちることを確かめた。
- ritsu の `tests/website.rs` の `build_sh_builds_the_tree_that_is_published`（Zensical が `website/.venv` にあるとき）。組んだ木で、ritsu のページの `gate`・`cedar`・`cedarschema` の塊がフェンスと同じ数だけその言語のブロックとして組まれ、空白・区切り・ただの名前のほかの種類の語を持ち、読めなかった文字（`err`）を持たないこと。言語のサイトのページのゲートと Cedar の塊も同じであること。sekisho のページが英日それぞれで、ゲートのキーワード（`permit`）、型（`money`）、税（`incl_tax`）、単位の付いた数（`10_000GBP`）、日付、文字列、宣言の名前、`use` の名前、演算子と同じ種類の語（`is`）、比較（`<=`）を、Cedar のキーワード（`permit`）、演算子と同じ種類の語（`has`）、演算子（`&&`）、エンティティ型（`Role`）、名前空間、エンティティの ID（`"clerk"`）、注釈（`@id`）を、スキーマの型（`String`）と注釈（`@doc`）を、それぞれの種類で見せること。日本語のページは別名（`refund_order`）も見せる。日本語の設定から `gatelexer` を外す、英語の設定から `cedarlexer` を外す、chobo の日本語の設定から `gatelexer` を外す、字句の定義の別名から `gate` を外す、`cedarschema` を外す、の五つで、それぞれ落ちることを確かめた。
- ritsu の `tests/website.rs` の `the_lexers_of_gate_and_cedar_read_what_the_tools_read`（Pygments が `website/.venv` にあるとき）。木の `.gate`（E001 の変異 8 を除く 180）、ritsu-base の試験の材料のうち公式の CLI が読むポリシーとスキーマ、CLI がポリシーを整形したもの（`cedar format` の 80 桁と 40 桁）、sekisho の golden と yuen の材料の Cedar、`sekisho gen` が例の英日の六つに書き出す Cedar（ポリシーは整形したものを含めて 218、スキーマは 49）を、字句の定義が読めない字なしに読むこと。CLI が読まない材料（`invalid/` と `invalid-schemas/`）のうち、CLI が invalid token と言う 6 には読めない字があり、ほかの 118（CLI は字句を読み、構文で止まる）には無いこと。バイトオーダーマークの二つは、Pygments が字句の定義に渡す前に取るので除いた。E001 の変異 8 には、どれも字句の定義でも読めない字がある。
- 木の文書（README、リファレンス、診断の一覧、DESIGN、スキル。golden は除く）の `gate` の囲み 169 と、`cedar` と `cedarschema` の囲み 18 を字句の定義に通した。読めない字があるのは、閉じていない文字列を見せる E001 の再現の三つ（英日の診断の一覧と、スキルのコピー）だけである。
- 組んだ塊を、サイトのスタイルシートの明るい配色と暗い配色で Chrome（headless）に開き、色が付くことを目で確かめた。

### 21.7 見つけて直していないもの

- 字句の定義は、検査を通るファイルを正しく読む。E001 のうち、単位の表に無い単位（`50XYZ`）、無い日付（`2026-02-30`）、大きすぎる数は、字句の定義には数に見える。単位の表と暦を持たないためである。サイトに載るゲートは検査を通るものなので、ページの色は変わらない。

## 22. `ritsu check` の JSON に渡す、ゲートのオブジェクト（2026-10-31）

`ritsu check --format json` の `files` の各項目は、`check` に、その言語の `check --format json` がそのファイルについて書くオブジェクトを持つ（ritsu の DESIGN 8.3）。`check::unit` は、診断と最後の行に加えて、`sekisho check --format json` がファイルごとに書くオブジェクト（`{file, ok, summary, diagnostics}`）を、コマンドと同じ `to_json` で作り、`ritsu_ports::Checked::json` で渡す。ritsu は `file` と `diagnostics` を除いた `{ok, summary}` を入れる。読めないファイルは `null` になる。sekisho のコマンドの出力は変えていない。

確かめたこと：ritsu の `tests/check.rs` の `each_files_check_in_the_json_is_what_its_language_writes` で、ritsu のテストのプロジェクト `cold_chain` と `regions`（検査を通らないゲートを含む）、例の `refunds` のゲートの `check` が、`ritsu sekisho check --format json --root <プロジェクト>` のオブジェクトから二つのキーを除いたものと同じであること。

## 23. `gen` の `--target` を何度でも書ける（2026-11-01）

ritsu の生成のコマンドは、どれも `--target` を何度でも書き、指定した出力先だけを書く形にそろえた（ritsu の DESIGN 9.6）。`sekisho gen` は前から `--target` が要り、指定した出力先だけを書いていたので、変えたのは一度に書ける数だけである。0.40.0 で通っていた実行は変わらない。

- `--target cedar|typescript|python|go` を何度でも書ける。書く順はフラグの順ではなく、`TARGETS` の順（cedar、typescript、python、go）である。`--authorizer` は三つの言語のコードに、`--module` は Go のコードに効く。
- 置き方（`<out>/<出力先>/`）、`--out` の既定（`generated`）、`--check` は変えない。
- `--target` が無いときは、どの言語の要るフラグとも同じ文（「`sekisho gen` には `--target cedar|typescript|python|go` が要ります」）と使い方の行で言う（ritsu の DESIGN 6.2 の 10）。同じ名前を二度書けば「二度書かれています」で、知らない名前は前と同じくフラグの選択肢の文で、どちらも何も書かずに exit 2。
- 付属物は無い。三つの言語のコードはポリシーとスキーマを中に持ち（`POLICIES` と `SCHEMA`）、`cedar/` のファイルを読まない。

確かめたこと：`tests/cli.rs` の `gen_writes_every_target_named_in_one_run` で、例の `refunds.gate` について、`--target go --target python --target typescript --target cedar` の一度の実行が書くファイルの全部と出力の行が、四つを一つずつ書いたものを cedar、typescript、python、go の順につないだものと同じであること（`--target go --target cedar` も同じ）。同じ名前を二度書いたとき（英日）、`--target` が無いとき（英日）、知らない名前のときに、標準エラーに一行を出し、何も書かずに exit 2 で終わること。

## 24. 生成するコードの名前を付け替えない（2026-11-04）

### 24.1 きっかけ

rulec、koyomi、chobo は、書いた名前が生成先で一つの識別子になるときと、生成先がその名前を持てないときに `check` で言い、`gen` がその出力先で止まる（rulec の DESIGN 15.263、15.265）。sekisho の三つの言語の生成器だけが、二つ目の名前に `_2` を足して、黙って別の名前を書いていた。

### 24.2 原因

生成器は `ritsu_emit::ident::unique` で、宣言する名前を順に受け付け、すでにある名前には `_2` を足していた（5 章）。ゲートの型 `Store` は、生成するコードの `Store` が `Store_2` になり、型 `Date` は、ゲートの名前のほうが `Date_2` になる。呼ぶ側が実装し読む型の名前が、ゲートのほかの名前の有無と言語で変わるのに、`check` は何も言わなかった。

この章の前のバイナリで次の九つの形を書くと、どれも `gen` が通り、書いていない名前が生成物に現れた（`Store_2`、`Date_2`、`Value_2`、`Literal_2`、`OrderState_2`、`ReplyInput_2`、`RefundOrderInput_2`、`OrderId_2`、`Roles_2`）。

### 24.3 決定

1. **`typescript.rs`、`go.rs`（二か所）、`python.rs` の `ident::unique` を外す。** 生成器は名前を書き換えない。`names()` と `record()` は、どの名前も書いたとおりの綴りのまま読む。
2. **`check` は W402 で言い、`gen` は E401 で止まる。** 番号は台帳の決まりのとおり、生成の帯（W401 と同じ 4xx）に置く。W402 はゲートの検査が通ったときに言い、出力先を挙げ、ゲートは通る。`sekisho gen` は、書く出力先のどれかを W402 が挙げていれば E401 を言い、その実行では何も書かずに 1 で終わる。W402 の挙げない出力先だけの実行は書く。
3. **何を宣言するかは生成器が持つ。** `typescript::declared`・`global`、`python::declared`・`global`、`go::declared`・`declared_fields` が、書く名前と出どころ（`spellings::From`）を返し、`spellings::find` がそれを比べる。W402 と生成するコードが食い違わないように、表を一つにした。

   | 形 | 例 | 挙げる出力先 |
   |---|---|---|
   | 読む型が、コードが自分で宣言する名前 | 型 `Store` | TypeScript、Python、Go |
   | 型が、グローバルかパッケージの型の名前 | 型 `Date` | TypeScript、Go |
   | 列挙が、モジュールが宣言する名前 | 列挙 `value` | TypeScript |
   | 列挙が、モジュールが読み込む名前 | 列挙 `literal` | Python |
   | 列挙と型が一つの名前 | 列挙 `order_state` と型 `OrderState` | TypeScript、Python |
   | 型が、action の入力の型の名前 | 型 `ReplyInput` | TypeScript、Python、Go |
   | 二つの action の入力の型が一つの名前 | `refund_order` と `refund__order` | TypeScript、Python、Go |
   | 二つの属性が Go の一つのフィールド | `order_id` と `order__id` | Go |
   | 属性が Go の役割のフィールド | 属性 `roles` | Go |
   | 型が、Go の名前で尋ねる入口の名前（30 章） | 型 `Authorize` | Go |
   | 型が、Go の役割の一覧の関数の名前（33 章） | 型 `Roles`、`RolesOf` | Go |
   | 名前が、コードがそのまま書く所でその言語の予約語（32 章） | 属性 `class`、型 `Return`、ファイル `select` | その言語の出力先 |

   TypeScript は読む型だけを宣言するので（`raw::reads`）、属性を読まない型は当たらない。Cedar の出力先はどの形でも止まらない。

### 24.4 捨てたもの

- 警告だけを足して、付け替えを残すこと。呼ぶ側が読む名前が言語で違うままで、警告を読まない人は `Store_2` を実装することになる。
- 生成するコードの自分の名前を変えて、ぶつかりにくくすること（`Store` を別の名前に）。ぶつからないゲートの生成物のバイト列まで変わる。
- `ident::unique` を ritsu-emit から消すこと。生成器は読まなくなったが、ritsu-emit の試験が読んでいるので、この章では残した。

### 24.5 変わること

- 上の形のゲートは、挙げる出力先の `gen` が止まる。前は `_2` の名前で書いて通っていた。その名前を読むコードがあるなら、別名を変えて読み替える。別名は Cedar の名前でもあるので、使っているゲートなら `sekisho diff` で比べる。
- 形に当たらないゲートの生成物は一字も変わらない。

### 24.6 確かめたこと

- `tests/spellings.rs`（6）と、材料の `tests/spellings/`（九つの形。英語を先に、日本語の版を横に置いて 18 本、どれも英日の golden を持つ）。どのゲートも W402 を一つだけ言い、挙げる出力先は形ごとに決めたとおりである。挙げる出力先の `sekisho gen` は E401 で何も書かず、挙げない出力先は書く。注のとおりに別名を変えた 18 本（`tests/spellings/renamed/`）は何も言われず、四つの出力先のどれでも書く。
- ツールの段で、W402 の挙げる出力先も含めて、36 本を言語の道具に通した。`go vet` は 12 本、`tsc --strict` は 12 本、`mypy --strict` は 10 本が止まり、止まったのは W402 の挙げるものだけで、止まり方は注のとおりである。TypeScript の二つの `interface` は黙って一つになるので、同じ名前の宣言があることも壊れたと数える。
- 名前を変えた版は、`tests/typescript.rs`、`tests/python.rs`、`tests/go.rs` のほかのゲートと同じく、生成したコードの答えを参照の評価と比べる。W402 の挙げる出力先は、そこから外す。
- W402 の変異（`tests/mutants/W402_*.gate`）と、E401 の台帳の例（`sekisho gen` のコマンドの形）。
- この章の前のバイナリといまのバイナリで、木の `.gate`（252 本）の `check`（英語、日本語、JSON）と四つの出力先の生成物を比べた。違うのは 20 本で、材料の 18 本と変異の 2 本である。ほかは一字も変わらない。

### 24.7 見つけて直していないもの

- `ritsu gen` は止まるが、文が違う。`crates/ritsu/src/package.rs` は `generate` のエラーを sekisho の不具合として包んで exit 2 にするので、E401 の文がそのまま届かない。koyomi のように、先に `sekisho::spellings::stops` を読む形にするのは ritsu の側の直しである。
- `ident::unique` のほかにも、名前をよける所が二つ残っている。TypeScript の `entity_field` は属性を `roles` と `member_of` からよけ（`ident::aside`）、Python の `record` は自分のフィールドをよける（属性 `roles` は `roles_`）。どちらもエンティティのデータの中のキーか、クラスの中の名前で、組めて答えも同じなので、今回は外していない。
- `GLOBALS` は表ごと見るので、モジュールがそのグローバルを読まないゲートでも、その名前の型は W402 になる。読むかどうかはゲートで変わり、多めに止める側に倒した。

## 25. `enums` ファイルの列挙を読む（`import enum`。2026-11-07）

### 25.1 きっかけ

注文の状態のような列挙は、ワークフローも、ゲートも、規則も使う。規則は `enums` ファイルに列挙を一度だけ書き、`import enum` で読めるが、`.gate` と `.flow` は自分で `enum` を書くしかなかった。同じ状態を二か所に書くと、片方の値の名前だけを変えても、どちらの検査も黙る。ワークフローの `不可` を `拒否` に変えた試しで、そのとおりになった。

### 25.2 決め

```gate
import enum order_state from "rules/order_states.rule"
```

名前を付け替えるときは、行の終わりに `-> status` を付ける。

- 形は rulec と同じ。`import enum <列挙> from "<ファイル>" [-> <名前>]` で、パスは `.gate` のあるディレクトリから書く（`use` と同じ）。置く場所は `enum` と同じで、二つは混ぜてよい。違う場所は E003。
- **読むのは口だけ。** `Rules::facts` に尋ねる。`enums` ファイルにも rulec の口は規則と同じに答え、その列挙の値、順、表示名、別名を言う。`enums` ファイルが `import proto` や `import jsonschema` で値を確かめているなら、その検査も rulec のものとしてそのまま効く。sekisho は `enums` ファイルの読み方を持たない（P4）。ブラウザの試すページも、ほかの `use` と同じ仮想ファイルから読む。
- **読んだあとは、その場で書いた `enum` と同じに使う。** 読んだ列挙は、`.gate` の列挙の並びに、`import enum` を書いた行の位置で入る。Cedar、生成する三つのコード、ベクタは、どこから来た列挙かで形を変えない（2 章の `enum` と同じ。`doc` と `api` は、下の「見せ方」のとおり読んだファイルを言う）。`use gate` で読まれたときも同じに見える。型を比べるところ（名前の検査と、組み合わせを数える歩き）は、宣言した場所を見る。一つの `enums` ファイルの列挙は、どの名前で読んでも一つの型である（28 章）。
- **値の名前。** 値は、`enums` ファイルの名前と、その別名（丸括弧の中）で書ける。Cedar に出るのは別名で、別名が無ければ名前である。生成するコードの `WithinLimit` のような綴りでは書けない（`use rule` の出力を読むときは書けるが、`.gate` の列挙の値は、`enum` に書いたものも名前か別名だけである）。
- **順。** rulec の列挙には `<` で順を持つものがある。`.gate` の列挙は値の順を使わない（比べる式が無い）ので、順は持ってこない。ただし `doc` と `api` は、順があるものだと言う。
- **持ち物にしない。** 読んだ列挙は `Items` に出さない。その列挙を持つのは `enums` ファイルである。代わりに `References` が `import enum` として、`rulec "<ファイル>" enum <列挙>` を返す。sakai はこれを境界を越える参照として確かめ、yuen は影響をたどる。
- **間違い。** ファイルが無い、`enums` ファイルでなく規則である、rulec の検査を通らない（E201）。その列挙が無い（E101。ファイルにある列挙を注に並べる）。`-> <名前>` か名前が、同じファイルの `enum` や別の `import enum` と重なる（E006）。名前か値が Cedar の名前の形でない（E007、E008。注が、直すのは `enums` ファイルの側だと言う。`->` で付けた名前は、そのままこの行で直す）。rulec をつながない sekisho では E209。診断はどれも `import enum` の行（ファイルの間違いは文字列、列挙の間違いは列挙の名前）に付く。列挙を持ってこなかった行の名前（`->` の名前と別名も）は、ファイルの中にあるものとして扱い、それを型に書く属性や入力は何も言わない。間違いは `import enum` の行で一度だけ言い、型の行に「型 `…` はありません」（E101）を重ねない（ritsu の DESIGN 8.3：読めないファイルは、それを読む行だけが言う）。
- **見せ方。** `doc` は「ほかのファイルから読んだ列挙」の節に、値と読んだファイルと順の有無を出す（読んだ列挙があるときだけ）。`api` は新しい `enums` に、全部の列挙の値と、読んだものには `from`（ファイルと、そこでの名前）と `ordered` を出す。

### 25.3 捨てたもの

(1) `use rule` の列挙を `use rule … enum` のように読ませる：`enums` ファイルは呼ぶ規則ではない（`enums` ファイルの出力を読むのは E101）。(2) 値を一つずつ `.gate` に書き写させて、突き合わせる検査を足す：同じ一覧を二か所に持つ形が残る。(3) 順を持ち込み、`<` で比べられるようにする：`.gate` には列挙を比べる式が無く、足すのは別の話である。

### 25.4 変わること

節の順に `import enum` が加わる（E003 の文）。キーワードに `import` が加わる（行頭に一語、`import` の次に `enum` と `from`）。`Gates` は変わらない。

### 25.5 確かめたこと

`tests/imports.rs`：読める、別名と順、`->`、五つの間違い、`Items` と `References`、`doc` と `api`、書いた `enum` と読んだ `enum` で、Cedar・TypeScript・Python・Go の生成物が同じ（先頭のハッシュの行を除く）。`tests/mutants` に四つ（E201、E101、E006、E209。英日）。例 `examples/order_states`（ワークフローとゲートが同じ `enums` ファイルから状態を読む）は、`ritsu check` で通り、`enums` ファイルの値の名前を変えると、ワークフローとゲートの、その値を書いた所で止まる。ritsu の `tests/sakai.rs`：コンテキストをまたいで読むと、sakai が関係を見る（関係が無ければ E201、パートナーの内側なら E202、共有カーネルに並べれば通る）。

### 25.6 残したこと

`.gate` の列挙の順を使う式。`enums` ファイルの列挙の値の表示名（`"Same city"`）を `doc` に出すこと。

## 26. 属性と入力の値の出どころを書く `fed by`（2026-11-07）

### 26.1 きっかけ

ゲートに尋ねるコードは、属性と入力の値をほかの場所から取ってくる。JSON Schema の `Order.amount` の名前や範囲を変えても、ゲートの属性と合わなくなったことはどの検査にも見えなかった。

### 26.2 決定

`principal`・`resource` の属性の行と、`action` の `input` の行の最後に `fed by <参照>` を書く。

```gate
    amount : money[USDc]  range >=1USDc <=1000000USDc  fed by jsonschema "schemas/order.schema.json" def Order property amount
```

- **答えるのは `ritsu check`。** 値の比べ方と三つの答えは根の DESIGN 7.16 にあり、X17（`ritsu-cross`）が口だけを通して行う。単独の `sekisho check` は形と場所の誤りだけを言う（E009）。sekisho は口 `Feeds` で受け取る側の型と範囲を渡す。
- **`context` の行には書けない（E009）。** `context` の値は `limits(amount: 1).small` か `today` の形で、計算の式がすでに出どころを言っている。宣言された型も無く、比べるものが無い。
- **読み方。** 行を字句に分ける前に `ritsu_base::fed::cut` が切り取り、`ritsu_base::naming` が読む。`fed` と `by` は字句の表に載せるが（色付けと文書のため）、二語が揃って初めて形になるので、名前を奪わない。
- **嘘を隠さない。** `doc` のページに「属性の出どころ」の節と入力の注を出す。文は「出どころ: `…`（値の範囲は `ritsu check` が確かめます。値を運ぶ手書きのコードは確かめません）」。
- **ハッシュ。** `source_sha256`（`api`、Cedar、生成コードの頭）はこの形を除いたものから取る。Cedar と三つの生成コードは一字も変わらない。
- `api` の属性と入力に `fed_by`。`Items` は本文からこの形を除き、`References` は `how: "fed by"` で返す。

属性と `action` の入力は、ほかの場所の `fed by` の出どころにもなる（`sekisho "<ファイル>" resource <名前> attribute <名前>`、`principal` も、`sekisho "<ファイル>" action <名前> input <名前>`。口 `Feeds::holds`。手で書いた Cedar の組は答えない）。ゲートが先に受け取った値をコードがワークフローに渡すとき、ワークフローの入力がこの属性を指せる（根の DESIGN 7.16 の 2）。列挙の値は、`.gate` の名前か別名のどちらかが同じなら同じ値である（7.16 の 5）。

### 26.3 捨てたもの

(1) `context` の行にも書く：上のとおり。(2) 属性の値を Cedar の `@doc` に載せる：Cedar を変えることになり、Cedar を読む人に確かめていないことを確かめたように見せる。

### 26.4 変わること

属性と入力の行の最後の形が一つ増える。E009 の場合が一つ増える。例 `examples/order_desk`。

### 26.5 確かめたこと

ritsu の `tests/fed_by.rs`：JSON Schema の名前、型、範囲を変えた場合、書けない場所、全部の出力先で生成物が同じ、`doc` と `api`。`tests/mutants/E009_fed_by_on_a_computed_value.gate`、`E009_計算する値の出どころ.gate`。例の JSON Schema の上限を 2,000,000 に広げると：

```console
$ ritsu check order_desk.gate
order_desk.gate: ok — 1 action, 1 policy (1 permit, 0 forbids), 0 expectations, 0 separations
error[ritsu E207]: order_desk.gate:18:1: `resource Order attribute amount` takes money[USDc] >=1USDc <=1_000_000USDc, and its source jsonschema "schemas/order.schema.json" def Order property amount can be 1_000_001USDc
    18 |     amount : money[USDc]  range >=1USDc <=1000000USDc  fed by jsonschema "schemas/order.schema.json" def Order property amount
  = The contract's validation lets it through.
  = Widen what this place takes, or narrow what the source gives. Which of the two is a person's decision.
ritsu check: 1 file (sekisho 1): 1 fail (1 error); borders between the languages: 2 checked, 0 undecided
```

### 26.6 残したこと

`use rule` の答えを読む `context` の値は、規則の言語が出力の範囲を言うので、この形を足す理由が無い。JSON Schema は sakai の成果物でないので、コンテキストをまたいで指しても境目の決まりに当たらない（sakai の DESIGN 32 章）。

## 27. ワークフローの呼び出しを、届く実行が持つ値で確かめる（2026-11-15、2026-11-17）

### 27.1 きっかけ

X16（4.6）は、ゲートがワークフローに action を許すかを、フローのどこで呼んでも同じ答えで確かめていた。規則 `k` の答えが `agent` のときだけワークフローに許すゲートに対して、フローが `match r.who` の `agent` の分岐で呼んでも、`person` の分岐で呼んでも、「組み合わせによって許される」になり、タスクが拒まれたときのエラーを宣言していれば、どちらも通った。`person` の分岐の呼び出しは、届く実行が全部拒まれる。エラーを宣言していないときの W909 の注も、その呼び出しに起こらない組み合わせ（`agent` になる金額）を、許される例として言った。

呼び出しが渡す引数も読んでいなかった。フローが 1 ポンドから 50 ポンドの額しか渡さない呼び出しに、W909 が「51GBP to 10,000GBP で拒まれる」と言い、ゲートが 100 ポンドを超える額だけを許すとき（届く実行は全部拒まれる）も通った。

2026-11-15 に呼び出しごとに尋ねるようにしたあとも、同じ読み方で直せる所が残っていた。

- E212 は、行の右のフローの値を、フローの全体で合わせた範囲で読んだ（`Flows::value`）。次のフローは、`amount` に 5,000 ポンドまでの額を入れ、返金の操作を呼ばない分岐でだけ 10,000 ポンドまでの額に入れ直す。行が当たる呼び出し（`refund`）に届くのは 5,000 ポンドまでの額だけである。

  ```text
  flow
    let amount = small
    match by_hand
      true =>
        let amount = large
        hand_over(orderId: order_id)
      false => refund(orderId: order_id)
    succeed refunded = amount
  ```

  `total : money[GBP]  range >=0GBP <=5_000GBP` の属性に `total = amount` と書いたゲートに、2026-11-17 より前の `sekisho check` は次のように言った。

  ```text
  error[E212]: example.gate:9:1: The flow's `amount` can be outside the range of the attribute `total` (0GBP..5,000GBP)
       9 |     total = amount
    = For example: `amount` at 10,000GBP.
    = Narrow what the flow's value can be to what the attribute takes (the `range` of the flow's input or of the record's field), or widen the attribute's range.
  ```

- X16 が規則の答えを読み替えられるのは、フローが規則に渡した値が、どれも action の `input` か属性か定数に当たるときだけだった。途中で入れた変数やタスクの結果を渡した呼び出しは、範囲が分かっていても、数えなかったものとして注に出た。
- フローの呼び出しに書いた数（`refund(amount: 40)`）は、単位を持たない数として口を通り、金額の `input`（`money[GBP]`）として読めなかった。全部の実行が許される（か、全部拒まれる）と分かる呼び出しが、W909 のままだった。
- ループの中の呼び出しとループのあとの呼び出しでは、ループの中で値を入れる変数を、フローの全体で合わせた値で読んだ（dandori の DESIGN 33 章）。
- `sekisho diff` は、行の差を言わなかった。行だけが違う二つの版は、ゲートの答えが同じなので「どの action も同じ」と言って 0 で終わった。行の差は、X16 の答えを変えうる。
- 名前の検査の E105 は、エンティティの型を、Cedar の名前（ASCII の別名）で言った。`顧客(Customer)` と書いたゲートで「型 `Customer` のエンティティ」と言った。

### 27.2 原因

- 口が運ぶものが足りなかった。dandori の `Flows::operation_calls` は、呼び出しの行、タスク、操作、拒まれたときのエラーだけを答え、渡す引数も、どの分岐の中かも言わなかった。届く実行がフローの全体より狭い値しか持たない変数があることも、言わなかった。書いた数には、それを受け取る引数の型を付けなかった。
- sekisho の `Gates::allowed` は、検査のときに数えた action の全部の組み合わせから、そのワークフローの組み合わせを選ぶだけで、フローのことを何も知らなかった。
- フローの値と resource の属性を結ぶものが、どこにも無かった。action の `input` は操作の引数なので、呼び出しが渡す値と同じものだと分かる。resource の属性は、サービスが自分のデータから読むので、フローのどの値に当たるかは、書かなければ決まらない。
- E212 は、行を一度だけ、ゲートの検査の前に確かめていた。行がどの呼び出しについての言い分かは、action が何を守るかで決まり、それは契約を読んでから分かる。
- 規則の呼び出しを読み替える所（`at.rs`）は、ゲートに言葉のある値（`input`、属性、定数）しか置き場を持たなかった。
- `sekisho diff` は、二つの版の答えとインターフェースを比べるもので、ワークフローの下の行は、そのどちらでもなかった。
- E105 の文は、型を、Cedar の名前のまま入れていた。

### 27.3 決定

- **呼び出しごとに尋ねる**（`Gates::allowed_at`、`src/at.rs`。8.3）。dandori が、呼び出しが渡す引数と、届く実行が持つ値（`match` が読んだ値、規則の答えを持つ変数とその呼び出し、フローの全体より狭い値を持つ入力と変数）を答え（dandori の DESIGN 33 章）、sekisho が、その値でなりうる組み合わせだけで action を数え直す。数え方は検査と同じ歩き（4.1）で、範囲を絞り、フローの規則の呼び出しを、action の計算した値と並べて数える。ゲートの計算した値と同じ規則に同じ値を渡した呼び出しは、同じ計算として数えるので、規則がどれほど込み入っていても、逆に解かずに決まる。違う規則が同じ属性を読む形も、同じ数え方で決まる。ループの中の呼び出しも、届く回が最後に入れた値で尋ねる。
- **引数は、何も書かなくても読む。** action の `input` と呼び出しの引数は、どちらも操作の引数の名前で書いてあり、同じ値である。書いていないゲートの結果も、これで変わる（27.5）。書いた数は、引数の型と単位の数として読む。整数が刻みを数える型（`rate[step 0.1%]`）の引数に書いた数は、刻みの数である（`fee: 5` は 0.5%）。dandori が生成するコードが、書いた整数をそのまま渡すからである。単位を付けて書いた数（`fee: 0.5%`。dandori の DESIGN 40 章）は、dandori が引数の型の単位で数えた整数（5）にして渡すので、整数で書いたときと同じに読み、文は書いたとおりの `0.5%` を引く。規則の呼び出しに書いた数も、規則の入力の型で同じに読む。単位の合わない数（GBP の引数に書いた数を、JPY の `input` として）と、刻みの違う率（0.1% の刻みの引数を、1% の刻みの `input` として）は読まない（`hold` が数えなかったものにする）。その組は、ritsu の X16 が、引数ごとに E909 として呼び出しの位置で言う（2026-11-17。ritsu の DESIGN 7.14）。比べるのは、口 `Gates::facts` の action の `inputs`（`GateInput`）の型と、引数の `FlowValue` の型で、比べ方は `fed by` と同じである。その呼び出しでは、ゲートの答えのうち、引数がどうであれ成り立つ E908 だけを言い、W909 は言わない。
- **resource の属性は、ワークフローの下の行で結ぶ**（2.12）。綴りは `resource <型> from <フローの値>` と、その下の `<属性> = <フローの値>` である。属性の値は、呼び出しに届く実行が、行の右の値に最後に入れた値で読む。
- **ゲートに言葉の無い値は、dandori がそこで言う値で読む**（`at.rs` の `free_input`）。フローが規則に渡した値が、`input` にも属性にも定数にも当たらないとき、dandori がその呼び出しで両端を言う数と日、列挙の値、真偽なら、それを action に無い `input` として足し、その全部の上で規則の答えを数える。範囲の端だけで決めず、組み合わせを数え直すので、偽の断定はしない。読める形と読まない形は次のとおりである。

  | 規則に渡した値 | 読み方 |
  |---|---|
  | 呼び出しが、action の `input` と同じ名前の引数に渡す値 | その `input` |
  | ワークフローの下の行が、属性の値と書いた値 | その属性 |
  | 書いた値（真偽、列挙の値、`input` の単位の整数） | 定数 |
  | ほかの値のうち、dandori がその呼び出しで両端を言う数と日、列挙の値、真偽（途中で入れた変数、範囲を書いたタスクの結果、ゲートが読まない入力）（2026-11-17） | action に無い `input` として、言う値の全部の上で数える |
  | 端の分からない数と日（範囲の無いタスクの結果、期間の無い `now`）、ほかの型（文字列、レコード、リスト） | 読まない。数えなかったものとして注に言う |
  | どの値も、答えが読む `input` か属性に当たらない呼び出し | 答えはゲートの組み合わせについて何も言わないので、読まない。言葉の無い値を渡していれば、数えなかったものとして注に言う |

- **行は前提として見せ、何も消さない。** 行に頼った答えは、頼った行と、だれも確かめていないことを言う。行に頼って「いつも」になっても、タスクが拒まれたときのエラーを宣言していなければ W909 である。行に頼って「決して」になれば E908 で、注が頼った行を言う（行のほうが違っているなら、直すのは行である）。
- **行が無いときの「組み合わせによって」は、決められないことを言う。** ゲートが組み合わせによって拒み、答えが読む属性について行が何も言っていないなら、届く実行がどちらになるかは書いていない。W909 は、ゲートが許す例と拒む例を、ゲートについての事実として言い、行を書けば確かめられることを案内する。
- **数えられなかったものは言う**（`AllowedAt::unheld`）。フローが持つ値のうち、ゲートの語に読み替えられなかったもの（上の表の読まない形、別の値を渡した `resource` の行）は絞りに使わず、何をなぜ数えなかったかを注に出す。絞らなければ組み合わせが増えるだけなので、「いつも」と「決して」は正しいままである。
- **行の誤りは、sekisho が言う**（E212。フローは dandori の口で読む）。フローに無い値には、書ける名前（入力と変数、レコードのフィールド。`Flows::value_names`）を注に並べる。型と単位の食い違いは、値にかかわらず行の誤りなので、行ごとに一度言う。
- **値が属性の範囲の外になりうるかは、行が当たる呼び出しごとに確かめる**（`borders::values_at_calls`）。行は、`from` の値を resource の id に渡す呼び出しの全部についての言い分である。当たる呼び出しは、ゲートの action が守る操作の呼び出しのうち、action が `resource <型> from <引数>` でその型の resource を引数から取り、呼び出しがその引数に行の `from` の値を渡すものである。それぞれの呼び出しで、届く実行が行の右の値に最後に入れた値から、途中の `match` が外した値を除いて確かめ、外れる値が届く呼び出しを注に並べる。dandori が呼び出しを言えないときは、前と同じくフローの全体の値で確かめる。action が何を守るかは契約を読んでから分かるので、契約を読んだあとに確かめる（`checks.rs`）。上のフローには、何も言わない。
- **当たる呼び出しの無い行は、W202 で言う**（`borders::spoken_of_by_no_call`、2026-11-17）。範囲について確かめることは無いが、`ritsu check` がどの呼び出しでもその行を前提にしないので、行は何も言っていない。書いた人は効くつもりで書いているので、書き違いのことが多い。注で、なぜ当たらないかを、四つのどれかで言う。その型の resource を引数から受け取る action が無い（型の書き違い。引数から受け取る action と、その型を並べる）、受け取る action が、どの引数が ID かを書いていない（`resource <型> from <引数>` を勧める）、フローがその action の守る操作を一つも呼ばない（別のワークフローの行か、要らない行）、その操作の呼び出しが ID の引数に別の値を渡す（値の書き違い。呼び出しごとに渡す値を並べ、一つなら `resource <型> from <その値>` を勧める）。言うのは、フローが dandori の検査を通り（通らなければ E208）、dandori が呼び出しを言え、行の `from` の値がフローにあり（無ければ E212）、その型を受け取る action の `guards` が契約で見つかったとき（見つからなければ E202 が言い、呼び出しが分からない）である。
  - コード。sekisho のコードは、E0xx が字句と構文、E1xx がファイルの中の名前と型、E2xx と W201 がほかのファイルと言語との境目（`use`、契約、規則、日付、ワークフロー。E212 もここ）、E3xx と W3xx が組み合わせの全部の検査、E4xx と W4xx が生成、W9xx が秘密である。この警告は、ゲートの行とフローの呼び出しの境目のことで、E212 と同じ行について言うので、境目の警告の次の空きの W202 にした。
  - 言う所。行はゲートにあり、指す呼び出しがフローに無いので、ゲートの行で言う。X16（ritsu の言語をまたぐ検査）の W909 と E908 は、フローの呼び出しの位置で、その呼び出しとそれを守る action について言うもので、この警告には指す呼び出しが無い。行が当たる呼び出しを集めるのも、E212 を確かめる sekisho である。
- **`sekisho diff` は、行の差を言う**（`diff::lines`）。ワークフロー、`resource` の行、属性ごとに、二つの版で違う行（変わった行、片方の版にしかない行）を、答えの差とは分けて並べる（JSON は `workflow_lines`）。行はゲートの答えを変えないので、決定が同じことを言う結びの一文はそのまま出す。終了コードは 1 である。行は、`ritsu check` がフローの呼び出しを確かめるときの前提で、二つの版で前提が違うことは、使う側が知るべき違いだからである（ritsu の DESIGN 17.9.4）。`ritsu mod diff` の振る舞いの差は答えの差だけを数え、行の差は、ゲートの api の JSON の差としてインターフェースの差に出る。
- **名前は書いたとおりに言う。** E105 の型は、ファイルが書いた名前で言う（`names.rs`）。文を作る所の全部を探し、型や属性を Cedar の名前のまま文に入れる所は、ほかに無かった。
- **行は、言語の全部の場所に載せる。** 構文と EBNF（9 章）、名前の検査、台帳と再現（E212）、変異、`sekisho doc`、`sekisho api`、`sekisho diff`、口（`GateWorkflow::holds`）、`ritsu check` の JSON（境界の `assumes`）、例（`examples/claims` の英日）、reference、README、サイト、スキルである。

### 27.4 捨てたもの

- **行を保証として扱うこと。** ゲートのコードはサービスの中で動き、読めるのは resource の属性と操作の引数だけである。フローの変数は届かない。条件は数を定数としか比べられない（3.1）ので、属性と引数を比べて「同じ値のときだけ許す」とも書けない。フローの値を操作の引数に足せば、それは `input` で、行が無くても読める。フローが先にサービスから読んだ値（タスクの結果）を右に書いても、読んでから呼ぶまでに変わりうる（残高を読まない理由と同じである。3.4）。だから、行があっても、拒まれたときの手当てを求め続ける。
- **引数は、行を書いたときだけ読むこと。** 引数が `input` と同じ値であることは、だれかの主張ではない。読まなければ、届く実行が全部拒まれる呼び出しが通り続ける。
- **`Gates::allowed` に、値を決めた属性を渡す形で尋ねること。** 尋ねる側（ritsu-cross）が、フローの値をゲートの語に読み替えることになる。ワークフローを型でなく名前で尋ねること、action の `input`、規則の答えを運べず、規則の答えを属性の範囲に直すには、規則を逆に解く問いが要る。
- **フローの規則の答えを、属性の範囲に直してから数えること。** 入力が二つ以上の規則や、導出を通る規則では、答えが区間の組の集まりにならない。
- **行があれば、拒まれたときのエラーの宣言を要らないとすること。** 上のとおり、行を確かめるものが無い。
- **E212 を、フローの全体の値のまま確かめること。** 行が当たる呼び出しに届かない値のせいで止め、値を入れ直す変数の名前を分けさせることになる。
- **当たる呼び出しの無い行も、フローの全体の値で確かめること。** 行は呼び出しについての言い分なので、当たる呼び出しが無ければ、範囲について言うことは何も無い。
- **当たる呼び出しの無い行を、エラーにすること。** 行は何も言っていないだけで、どの答えも誤らせない。0.55.2 で通るゲートを止めることになる。
- **当たる呼び出しの無い行を、X16 の側（ritsu-cross）で言うこと。** X16 の診断は、フローの呼び出しの位置で、その呼び出しを守る action について言う。この行には指す呼び出しが無く、行を確かめているのは sekisho である。
- **端の分からない値を、型の全部の値として数えること。** どの値が来るかをどこにも書いていない値で答えを決めると、書いていないことに頼った答えになる。前と同じく、数えなかったものとして言い、範囲を書けば読めることを示す。
- **行の差を、`ritsu mod diff` の答えの差に数えること。** 行はゲートの答えを変えない。答えの変わらない差を、答えが違うと言わない（ritsu の DESIGN 17.9.4 の決め）。

### 27.5 変わること

- **通っていたものが通らなくなる。** 呼び出しが渡す引数か、分岐が残した値だけで、届く実行が全部拒まれると分かる呼び出しは、E908 になる（前は、タスクがエラーを宣言していれば通り、していなければ W909 だった）。行を書いていないゲートでも変わる。2026-11-17 からは、ループの中の呼び出し、書いた数（刻みの率に書いた数も、刻みの数として）、ゲートに言葉の無い値を渡した規則の答えでも同じに決まる。木の例とテストのプロジェクトのゲートでは、結果の変わるものは無い。
- **`sekisho diff` の終了コードが変わる形がある。** ワークフローの下の行だけが違う二つの版は、0 で終わっていたのが 1 で終わる。木の中で、`sekisho diff` の終了コードを読むのはテストだけである。
- W909 の注が、届く実行がなりうる組み合わせから例を出す。起こらない値を例に言わなくなる。行が無く、答えが resource の属性を読むときは、見出しの文が「決められない」に変わる。
- E212 は、行が当たる呼び出しに届かない値では出なくなる。出るときは、そういう値が届く呼び出しを注で言う。当たる呼び出しの無い行には、範囲の E212 の代わりに W202 が出る。止まっていたゲートが通るようになることがある。通っていたゲートは通るままである。台帳の E212 の再現には、行が当たる呼び出しを足した。
- 当たる呼び出しの無い行に W202 が出る（終了コードは変わらない）。木の例とテストのプロジェクトのゲートには出ない（PLAN 7.151）。
- E105 の文が、型を書いた名前で言う。
- `ritsu check --format json` の境界（`workflow_permission`）に `assumes`（頼った行の並び。頼っていなければ空）が増える。`sekisho api` の `workflows` の要素に `assumes` が増える。`sekisho doc` のワークフローの節は、呼び出しごとの答えと、届く実行が持つ値と、行の表を出す。`sekisho diff --format json` に `workflow_lines` が増える。
- 口の型が増える（8.3）。`OperationCall` を組み立てる所は、`OperationCall::new` で済む。2026-11-17 に、`GateAction` に `inputs`（`GateInput`）が増えた。
- **通っていたものが通らなくなる（2026-11-17）。** 呼び出しが渡す数を、action の `input` が違う単位か刻みか税で読む組は、ritsu の E909 になる。0.55.2 は、タスクが拒まれたときのエラーを宣言していれば何も言わず、していなければ W909 の注で型が違うと言うだけだった。木の例とテストのプロジェクトのゲートには、当たる組が無い（PLAN 7.152）。
- 生成する Cedar とコードは変わらない。

### 27.6 確かめたこと

- `tests/at.rs`（十五）：規則の答えで分けた分岐の中の呼び出し、ゲートが使わない規則をフローが呼ぶ形、呼び出しが渡す引数、resource の列挙・真偽・無いことのある属性、数えなかったものの注、ループの中の呼び出し、どの行にも頼らない答え、分岐の中の分岐と抜ける分岐、エンティティを指す属性、行が当たる呼び出しごとの E212（入れ直す値が届かない呼び出しと届く呼び出し）、金額の `input` に書いた数（単位の合わない数は読まず、数えなかったものとして注に言うことも）、刻みの率の引数に書いた数（`fee: 5` は 0.5% で、1% までを許すゲートで「いつも」、`fee: 50` は「決して」。1% の刻みの `input` には読まず、注に言う）、規則の刻みの率の入力に書いた数（規則の答えを 0.5% で数える）、ゲートに言葉の無い値を dandori が言う値で読む形（端の分からない値は読まない）、当たる呼び出しの無い行の W202（呼び出しが ID に別の値を渡す形、フローが操作を呼ばない形、その型を受け取る action が無い形、受け取る action が ID の引数を書いていない形と、それぞれの注。当たる呼び出しのある行と、フローに無い値の E212 には出ないこと）。
- dandori の `tests/ports.rs` の `a_number_written_out_is_of_the_parameters_unit`：呼び出しに書いた数が引数の単位の数として口を通り、整数が刻みを数える型の引数に書いた数は刻みの数として通ること（英日の名前のフローで）。
- `tests/diff.rs` の `the_lines_under_a_workflow`：変わった行、新しい版だけの行、古い版だけの行を、テキストと Markdown と JSON の英日で言い、同じ版どうしは差が無いこと。
- `tests/parse.rs`、変異（E212 の三つ、E101・E006・E102・E005 の、ワークフローの下の形、E105 の規則に渡すエンティティ、W202 のどの action も受け取らない型の行。英日）、台帳（W202 の再現は、交換の元の注文を返金するフローと、交換の注文番号を書いた行）、`tests/gen.rs`（行を足しても、生成するものがハッシュのほか一字も変わらない）、`tests/doc.rs` と `tests/cli.rs`（`examples/claims` のページと `api`）。
- ritsu の `tests/cross.rs` の X16：引数だけで決まる形（通る、E908、W909）、規則の答えの分岐で呼ぶ形（許される分岐、エラーの宣言の無い形、拒まれる分岐の E908、行の無い形の W909）、違う規則が同じ属性を読む形、境界の JSON の `assumes`。
- ritsu の `tests/website.rs`：サイトのページ（英日）が見せる、`examples/claims` のフローの二つの分岐を入れ替える編集と、そうした例に `ritsu check` が言うこと（E908）。ページは、編集を diff のブロックで、言うことをそのあとの text のブロックで見せ、試験が、例の写しに同じ編集をして比べる。
- 変えて確かめたこと。E212 をフローの全体の値で読む、書いた数に引数の型を付けない、刻みを数える型の引数に書いた数を読まない、言葉の無い値を読まない、行の差を集めない、E105 の型を Cedar の名前に戻す、のどれにしても、上のテストのどれかが落ちる。
- 木の、ワークフローを書いたゲートの全部で、0.55.2、2026-11-17 より前、今の `ritsu check` を比べた（PLAN 7.151）。

### 27.7 残したこと

- フローのタスクの引数と、ゲートの `input` が、同じ操作の引数の刻みを違えて書いた形（`fee: rate[step 0.1%]` と `fee : rate[step 1%]`）は、数えなかったものとして注に言うだけである（27.3 の引数の項）。同じ数を、二つの言語が別の率として読むので、どちらかの書き違いである。

## 28. 一つの `enums` ファイルの列挙は、どの名前で読んでも一つの型（2026-11-15）

### 28.1 きっかけ

列挙を `enums` ファイルに一度だけ書き、ゲートと規則がそれぞれ読む形（25 章）で、0.55.2 は、同じ列挙を別の名前で読んだところを型の違いとして止めた。`states.rule` の `order_status` について、次の四つである。

- 同じ列挙を二度読み、一方に `-> asked_status` で名前を付けて、`order_status` の属性と `asked_status` の属性を比べる。E104 になった（「`resource.status` (the enum `order_status`) and `resource.asked` (the enum `asked_status`) are of different types, and do not compare」）。
- `use gate` で読んだファイルが `order_status` を読み、読む側が同じ列挙を `-> asked_status` で読む。同じく E104 だった。
- `allowed` で、`order_status` の入力を持つ action を、`asked_status` の入力を持つ action から尋ねる。E110 になった（「The input `status` of `refund` is the enum `order_status`, and `status` it is given is the enum `asked_status`」）。
- 二つの action の計算した値 `tier` に、同じ列挙を `import enum` で読む二つの規則の答えを入れ、一つのポリシーが二つの action を名指す。E102 になった（「`tier` is of different types in different actions (the enum `tier` of the rule `first`, and the enum `tier` of the rule `second` in `hold`)」）。

dandori と kaname は、同じ形を一つの型として通す（dandori の DESIGN 28 章、kaname の DESIGN 40 章）。E102 と E104 は、二つの型がどこで宣言した列挙かも言わなかった。

### 28.2 原因

- 名前の検査は、列挙の型を `.gate` での名前で比べていた（`Ty::Enum` の名前、規則の列挙は規則の名前と列挙の名前の組）。読んだ列挙がどの `enums` ファイルのものかは、25 章の `from` と、rulec の口の `RuleEnum::origin`（ritsu の DESIGN 3.2）が持っていたが、比べには使っていなかった。
- 組み合わせを数える歩き（4.1）は、二つの属性の列挙の比べ（`CAtom::EnumEq`）を、列挙の番号が同じときだけ作っていた。

### 28.3 決定

- **一つの型になる条件は、dandori・kaname と同じ。** 宣言した場所（ファイルを正規のパスにしたものと、そのファイルでの列挙の名前）が同じで、値が同じ順に同じこと（`Ck::declared`、`Ck::same_ty`）。
  - `import enum` で読んだ列挙は、その `enums` ファイルの列挙。`use gate` で読んだファイルの `import enum` も、そのファイルのディレクトリからたどる。
  - ゲートが自分で書いた `enum` は、そのゲートのファイルの列挙。
  - 規則の列挙（計算した値の型）は、規則が `enums` ファイルから丸ごと読んだもの（口の `origin`）なら、その `enums` ファイルの列挙。規則が自分で宣言したものなら、その規則のファイルの列挙。
- **比べる所。** 属性の型が action によって違わないか（E102）、計算した値と入力の型が action によって違わないか（E102）、二つの属性の比べ（E104）、`allowed` で渡す入力（E110）。歩きでは、列挙ごとに、同じ型の最初の列挙の番号（`Enum::one`。モデルを組むときに決める）を持ち、二つの属性の比べは `one` が同じときに値どうしで数える。
- **診断の名前は書いたとおり。** 型の名前は、どこでも `.gate` に書いた名前で言う。別の型の列挙どうしの E102・E104・E110 には、それぞれをどこで宣言したかを言う注を付け、値が（順を除いて）同じなら、一つの型にする直し方も言う（`Ck::enums_apart`）。両方がゲート自身の `enum` なら、注を付けない（どちらも見えている）。

  ```text
  error[E104]: tests/mutants/E104_enums_declared_at_two_places.gate:31:36: `resource.status` (the enum `order_status`) and `resource.asked` (the enum `shared_status`) are of different types, and do not compare
      31 |   when resource.status is resource.asked
    = The enum `order_status` is an enum this gate declares; the enum `shared_status` is the enum `order_status` of the `enums` file `read/order_states.rule`. Enums declared at two places are two types, even with the same values; to make them one, put the enum in an `enums` file, and read it with `import enum` in each file that uses it.
  ```

- **Cedar、生成する三つのコード、ベクタ、`doc`、`api` は変えない。** 列挙は `.gate` の名前ごとに一つのまま、Cedar では文字列である（2.4）。同じ型の名前を並べる節やキーは足さない。読んだ列挙は、`doc` の「ほかのファイルから読んだ列挙」の節と `api` の `enums` の `from` が、どのファイルのどの列挙かをすでに言う（25.2）ので、同じ `from` を持つ名前が一つの型だと読める。
- **言語の境目は、値で比べたまま。** ゲートの列挙からフローの値や規則の入力に渡る所（名前の検査の `fits_column`、X16 の `at.rs`、境目の判定の `borders.rs`、ritsu-cross の `fed.rs` と `transfers.rs`）は、受け取る側の列挙が、渡りうる値を全部持つかで確かめる。サービスの間を渡るのは値の綴りで、型の同一性ではないからである（根の DESIGN 7.16 の 5）。同じ `enums` ファイルを読んでいれば値は同じなので、この比べは必ず通る。
- **`.proto` の操作の入力の列挙は、値で比べる**（E203。この章の調べで見つけた）。action の `input` の列挙の型に対して、前はフィールドの型の名前が列挙の名前で終わるかだけを見ていた（`OddCategory` というメッセージの型も通り、値は比べなかった）。今は、protobuf の名前の決まりでフィールドの型を解き（`Protos::resolve`）、列挙なら、Protovalidate の `const`・`in`・`not_in` を通る値（`ritsu_proto::validate::enum_names`。0 の `…_UNSPECIFIED` を除き、前置きを外して小文字にした名前で、rulec の `import proto` と同じ読み方）が、どれもゲートの列挙にあるかを見る。列挙でない型、読んだどのファイルにも無い型も E203 である。読んでいない import の先にある型は、決められないので何も言わない。OpenAPI の `enum` を値で比べるのと同じ形になる。

### 28.4 捨てたもの

- **同じ列挙を二つの名前で読むことを E006 にすること。** `use gate` で読んだファイルが読む列挙を、読む側が別の名前で読みたいことがあり、読む側の人は相手のファイルを直せないことがある。dandori・kaname と同じく、名前ごとに型を持ち、一つの宣言を共有したものを一つの型にする。
- **値が同じなら一つの型にすること。** 別の場所で宣言した列挙が、意味の違うまま比べられてしまう（dandori の DESIGN 28.4 と同じ）。
- **言語の境目も、宣言した場所で比べること。** 境目の向こうのコードは値を綴りで受け取り、型の同一性を持ち込まない。宣言した場所で比べると、同じ値を受け付ける別の宣言（相手の言語が自分で書いた列挙）を、通る値まで止めることになる。

### 28.5 変わること

- 28.1 の四つの形が通る。歩きは、二つの名前の属性が同じ値のときに条件が成り立つと数える（`tests/imports.rs` のベクタで、許される組み合わせは二つの属性が同じ値のものだけ）。
- 別の場所で宣言した列挙どうしの E102・E104・E110 に、宣言の場所を言う注が付く。
- `.proto` の操作を守る action で、操作が受け取る値を持たない入力の列挙、列挙でない型のフィールドに当てた入力の列挙が、E203 になる。
- `explain E102`・`E104`・`E203`、reference（`import enum` の節と `input` の項）が、宣言した場所で比べることと、`.proto` の列挙の値の比べを言う。
- Cedar、生成する三つのコード、ベクタ、`doc`、`api` は変わらない。

### 28.6 確かめたこと

- `tests/imports.rs` の `one_enum_of_an_enums_file_is_one_type_by_every_name`：28.1 の四つの形が通ることと、二つの名前の属性を比べるポリシーのベクタで、許されるのが二つの属性が同じ値の組み合わせだけであること。
- 変異（英日）：二つの規則の計算した値で、一方の規則が自分で同じ値の列挙を宣言するもの（`E102_a_computed_value_of_the_enums_of_two_rules.gate`、`E102_二つの規則の列挙の計算した値.gate`）と、ゲートの `enum` と同じ値の `import enum` の属性を比べるもの（`E104_enums_declared_at_two_places.gate`、`E104_二か所で宣言した列挙.gate`）。注は 28.3 の文になる。
- `tests/contracts.rs` の `a_proto_enum_field_and_the_values_it_lets_through`：値を全部持つ列挙、値の足りない列挙、Protovalidate の `in` で狭めたフィールド、列挙でないメッセージの型。
- 木の全部の `.gate`（297）を、木の外の写しで、直す前と後のビルドにかけた（`check`、`gen` の四つの出力先、`api`、`doc`、`vectors`）。出力の変わるものは無い（`gen` が出力のディレクトリを言う行のほか）。
- X16 の値の比べを手で確かめた。同じ `enums` ファイルを読むワークフローとゲートで、許される呼び出しは前と後で同じに通り、拒まれる呼び出しは前と後で同じ E908 になる。フローがゲートの列挙に無い値を渡す形は、前と後で同じく dandori の E016 が止める。

### 28.7 残したこと

- 属性と計算した値を比べる条件（`when resource.kind is cat`）は、sekisho に無い形である。計算した値は列挙の値とだけ比べる（3.1）。書くと、`cat` を列挙の値として読み、E101（「The enum `category` has no value `cat`」）になる。
- 属性の型に `<規則>.<列挙>` は書けない。規則の列挙を属性に持つには、その列挙を `enums` ファイルに置いて `import enum` で読む。

## 29. パスをどう書いて尋ねても、ゲートは一つのファイル（2026-11-15）

### 29.1 きっかけ

ritsu の DESIGN の 4.15 と 6.2 の 8 で、ファイルの見分け方と、文に書くパスの書き方を、どの言語にも一つの決まりにした。sekisho は二つの点で外れていた。

- `Engine` は検査を、ルートとファイルを、たどり着いたとおりのパスの組で覚えていた。診断がファイルを何と呼ぶかは、最初に尋ねた側の書き方で決まった。`company.gate` と `./company.gate` で尋ねると、検査は二度になり、後から尋ねた側も、最初の書き方で呼ぶ答えを受け取った。
- E201 と E101 は、ゲートが書いたパスを、走らせたディレクトリからつないだパスで言っていた。木の `.gate` の全部（301 本）を、名前、絶対パス、木の根からのパス、根の隣のディレクトリから `..` で上るパスの四つで渡すと、22 本で文が違った。

  ```text
  error[E201]: crates/sekisho/tests/mutants/E201_a_gate_read_that_does_not_pass.gate:5:1: `crates/sekisho/tests/mutants/read/staff_through.gate` does not pass sekisho's check
       5 | use gate "read/staff_through.gate"
    = E210: The namespace of `crates/sekisho/tests/mutants/read/staff_elsewhere.gate`, `Staff`, is not this file's, `Shop` (crates/sekisho/tests/mutants/read/staff_through.gate:5:1)
  ```

  ゲートのディレクトリで名前を渡すと、同じ文が `` `read/staff_through.gate` does not pass `` になった。

### 29.2 決定

- **ファイルで覚える。** 鍵は、ルートとファイル（どちらも絶対にして、`.` と `..` をシステムが開くとおりに畳んだもの。`paths::opened`、ritsu の DESIGN 4.15）である。ルートからの名前もシステムが開くファイルで付けるので、`..` でシンボリックリンクを越えて書いたルートは、リンクの先から書いたルートと同じ一つの鍵になる。`--root` も、システムが開くディレクトリとして読む。検査はファイルを、走らせたディレクトリからのパス（`paths::show`。絶対パスを渡した実行では絶対パス）で呼ぶので、どの書き方で尋ねても同じ答えになる。`ritsu gen` は、ゲートのコードを、同じ覚えの検査から作る（`Engine::outcome_of`）。
- **覚えるのは、口を作った実行のあいだだけ。** 前と同じに、覚えた検査を読み直して確かめることはしない。口はコマンドごとと、ブラウザのページの問いごとに作られ（`ritsu_project::Joined`）、実行の中でゲートを書き換えるものも、一つの口に二組のファイルで同じゲートを尋ねるものも無い。rulec などと同じに読んだものを問い直す形（`ritsu_base::fs::Seen`）も試したが、デバッグビルドで、176 本のゲートがある `tests/mutants` の `ritsu check .` が 520 ms から 589 ms に、試しのプロジェクトの approvals が 16.4 ms から 17.1 ms になったので、採らなかった。
- **ページの一問も一度。** ブラウザで試すページは、ゲートのページを HTML と Markdown の二つで見せる。二つを一度の検査から描く `run::doc_pages`（`cli::doc_pages`）を足し、ページはそれを呼ぶ（前は `sekisho doc` を二度走らせ、ゲートを二度検査した）。
- **数える。** `Engine::checks` が、ファイルを丸ごと検査した回数を言う。`check::check_text` は、検査したゲートを `ritsu_base::tally` に `sekisho` として数える。
- **ゲートが書いたものを引く文は、書いたとおり。** E201（検査を通らない、`enums` ファイルではなく規則である、読めない、`use gate` が輪になる）と E101（`enums` ファイルに無い列挙）は、`use` と `import enum` に書いたパスをそのまま言う。読んだゲートの誤りを注に挙げるときは、そのゲートを `sekisho check` したときと同じ文である（そのゲートが書いたとおり）。
- **たどった先のファイルは、このゲートのディレクトリから。** `use gate` で読んだゲートがさらに読むファイルで、同じ名前を二か所が宣言する E211 は、二つのファイルを、このゲートの `use gate` の行が書くとおりのパス（`paths::seen_from`）で言う。
- **開く所は、走らせたディレクトリから。** 注に挙げるほかの言語の診断の場所、読んだゲートの誤りの場所、文の中の `sekisho check <パス>` は、走らせたディレクトリから、`.` と `..` を畳んだいちばん短い相対で言う。

  ```text
  error[E201]: crates/sekisho/tests/mutants/E201_a_gate_read_that_does_not_pass.gate:5:1: `read/staff_through.gate` does not pass sekisho's check
       5 | use gate "read/staff_through.gate"
    = E210: The namespace of `staff_elsewhere.gate`, `Staff`, is not this file's, `Shop` (crates/sekisho/tests/mutants/read/staff_through.gate:5:1)
    = `sekisho check crates/sekisho/tests/mutants/read/staff_through.gate` says everything that is wrong with it.
  ```

### 29.3 捨てたもの

- **鍵を書き方のままにし、最初に尋ねた側の呼び方を使うこと。** 答えが、誰が先に尋ねたかで変わる。

### 29.4 変わること

- 一回の実行の中で、一つのゲートの検査は、パスの書き方によらず一度になる。
- E201・E101・E211 の文のパスが、書き方によらなくなる。注の場所と文の中のコマンドは、走らせたディレクトリからのパスで、絶対パスを渡さない限り、この機械のパスを言わない。
- Cedar、生成する三つのコード、ベクタ、`doc`、`api` は変わらない。

### 29.5 確かめたこと

- `tests/ports.rs` の `a_gate_is_one_file_however_its_path_is_written`：`ritsu check` に四つの書き方で渡し、口と `ritsu gen` が三つの書き方で尋ねても、検査は 1 回である。検査を通らないゲートについて口が言うことは、どの書き方でも同じである。
- 同じく `a_gate_is_kept_for_the_run_and_read_again_by_the_next`：一つの口に二度尋ねても検査は 1 回で、ゲートを書き換えたあとの次の実行の口は、いまのゲートを検査する。
- 木の `.gate` の全部を、上の四つの書き方で、直す前と後のビルドにかけた。違う本は 22 から 10 になり、残った違いは、どれも注の場所と文の中のコマンドの、走らせたディレクトリからのパスの違いである。
- ritsu の `tests/spellings.rs`：ゲートの 4 本（検査を通らないゲートを読むもの、検査を通らない規則を読むもの、無いゲートを読むもの、規則に無い列挙を読むもの）で、上の決まりのとおりに言う。

## 30. action の名前で尋ねる入口（2026-11-17）

### 30.1 きっかけ

生成したコードは、action ごとの型つきの関数（`AuthorizeRefundOrder` など）だけを持っていた。尋ねる action を名前（文字列）で持つ呼び手は、名前から関数を選ぶ場合分けを自分で書くことになる。申請と承認のサーバーのフレームワークでは、例の `gate.go` が 11、`ritsu ringi init` が書くプロジェクトの `gate.go` が 12 の場合分けを持っていた。ゲートに action を足しても、場合分けを足すまでは「ゲートにその action が無い」という誤りになる（例のゲートにある `change_request_type` は、例の場合分けに無かった）。

### 30.2 決め

1. **ゲートごとに一つ、action の名前で尋ねる入口を書く。** TypeScript と Python は `authorize`、Go は `Authorize`。引数は、`Store`、principal、action の別名（Cedar の名前）、`resources`（resource の型の別名から ID へ）、時刻で、ほかの関数と同じ並びである。`--authorizer avp` では、各 action の関数と同じに、尋ねる先 `avp` を前に取る（Go は `ctx` と `avp`）。例の Go（`ritsu sekisho gen examples/refunds/refunds.gate --target go` の実物から）：

   ```go
   func Authorize(store Store, p Principal, action string, resources map[string]string, now time.Time) Answer {
   	switch action {
   	case "view_order":
   		id, ok := resources["Order"]
   		if !ok {
   			return Answer{Policies: []string{}, Err: &Error{"resource", "resources holds no id of the resource of view_order (Order)"}}
   		}
   		return AuthorizeViewOrder(store, p, ViewOrderInput{OrderId: id}, now)
   	case "refund_order":
   		return Answer{Policies: []string{}, Err: &Error{"input", "refund_order takes inputs of its own (amount): ask it through AuthorizeRefundOrder"}}
   	case "export_refunds":
   		…
   	}
   	return Answer{Policies: []string{}, Err: &Error{"action", fmt.Sprintf("the gate has no action %q", action)}}
   }
   ```

2. **`resources` からは、その action がとる型の ID だけを読む。** ほかの型の ID は読まないので、呼び手は知っている ID を全部渡せばよい（フレームワークの例と、`ritsu ringi init` が書く `gate.go` は、チケット、ステップ、申請の種類の三つを渡す）。型を二つ以上とる action では、渡した中にそのどれかの型の ID がちょうど一つあれば、その型で尋ねる（`resource_type`）。一つも無いか、二つ以上あれば、どれを尋ねるか決められないので `resource` で拒む。この選び方は補助の関数（Go の `given`、TypeScript と Python の `_given`）で、そうした action があるゲートにだけ書く。
3. **自分の入力を受け取る action は、名前では尋ねない。** `input` の行を持つ action（`?` の入力だけのものも）は、入口が値を持たないので、関数を呼ばずに `input` で拒み、文で型つきの関数を名指す。入力は型つきの関数で渡す。ringi の操作がそうした action を尋ねる形は、`ritsu check` が X18 の E911 で止める（ringi の DESIGN）。
4. **ゲートに無い名前は、八つ目の種類 `action` で拒む（3.8、決定の 52）。** 文は `the gate has no action "x"`（日本語は `ゲートに action "x" はありません`）。名前の引用は言語ごとの書き方（Go の `%q`、TypeScript の `JSON.stringify`、Python の `json.dumps`）による。
5. **入口は、選んだ関数の答えをそのまま返す。** 確かめる順（5.3）も、拒む種類も、Cedar に尋ねるかも、その関数と同じである。
6. **Go の `Authorize` は、コードが自分で使う名前に入る（24 章）。** 型の名前が `Authorize` になるゲートは、W402 が Go を挙げ、`gen --target go` は E401 で止まる。TypeScript と Python の `authorize` は小文字で始まるので、型（大文字で始まる）とも、action の関数（`authorize` のあとに別名が続く）ともぶつからない。

### 30.3 捨てたもの

- **名前と型の無い入力を受け取り、入力を持つ action も尋ねられる入口。** 入力の型と範囲の確かめを型の無い値に移すことになり、型つきの関数の意味が薄れる。入力を持つ action は、型つきの関数で尋ねる。
- **action の名前を列挙の型で受け取ること。** 呼び手は名前を文字列で持っていて、型に直す場合分けが呼び手に戻る。
- **ゲートに無い名前を、ほかの断りと同じ種類（`resource` など）で返すこと。** 尋ね方の誤りと、答えとしての断りを、呼び手が分けられない。フレームワークは前者をサーバーの誤り（500）、後者を「許さない」として扱う。

### 30.4 変わること

- TypeScript・Python・Go のコードに、入口と、要るときは補助の関数が足される。前からある action の関数は一字も変わらない。
- 拒む種類に `action` が足される。TypeScript の型 `SekishoErrorKind` と Python の種類の `Literal` に値が一つ増えるので、種類を全部挙げる `switch` には場合が要る。
- Go で、型の名前が `Authorize` になるゲートは、W402 と E401 になる（前は通っていた。木のゲートには無い）。
- Python のコードは、Cedar に尋ねるときも `json` を読み込む（ゲートに無い名前を文に書くのに使う）。Go のコードは、action の無いゲートでも `time` を読み込む（入口が時刻を受け取る）。
- ritsu の X18 は、口の `GateAction` の `inputs`（`GateInput`。8.3、27.5）から、action が自分の入力を受け取るかと、その名前を読む。Cedar を手で書いたゲートの action は、いつも空である。

### 30.5 確かめたこと

- `tests/gen.rs` の `the_entry_by_name_of_each_language`：例のゲート（自分の入力を受け取る action を含む）、その `--authorizer avp`、日本語の版の `--lang ja`、型を二つとる action を持つ `tests/gen/asked.gate` の入口と補助を、三つの言語の golden（`tests/golden/gen/by-name/`）と比べる。
- ツールの段の三つのランナー（`tests/typescript.rs`、`tests/python.rs`、`tests/go.rs`）：どのケースでも、名前で尋ねた答えを、型つきの関数の答えと比べる（許すか、決めたポリシー、拒む種類、Cedar に渡した context）。ケースの action が自分の入力を受け取るときは、関数を呼ばずに `input` で拒むこと。ゲートのほかの型の ID を `resources` に足しても、答えが変わらないこと。加えて、ゲートに無い名前（`action`）、ID の無い `resources`（`resource`）、型を二つとる action に二つの型の ID（`resource`）を尋ねる。Go は 91 本のゲートで、名前で尋ねた 2,385 件が型つきの関数と同じ答えになり、29,365 件が `input` で拒まれ、関数を呼ぶ前の断りが 206 件。TypeScript は 93 本で 2,395 件・29,365 件・210 件、Python は 104 本で 2,439 件・29,365 件・227 件。違いはどれも 0 だった。Go と TypeScript のランナーは、この段の前から落ちる `tests/walk/given/` の二本のゲート（英語と日本語の版。Go は、rulec の Go で規則と列挙の名前が重なる。TypeScript は、絞り込みが閉包の中に届かない。どちらも直しの段で直す）を一時的に外して回した。Python のランナーは外さずに通る。
- `tests/spellings.rs`：十番目の形として、型 `Authorize` のゲート（英語と日本語の版）が W402 で Go だけを挙げ、`gen --target go` が E401 で止まり、別名を `Slip` にした版は何も言われない。
- `tests/ports.rs` の golden：口が action の入力を言う（`input` の行。名前、別名、行、何を持つか）。

## 31. 値の無いことがある属性を、規則と日付の関数に渡すコード（2026-11-17）

### 31.1 きっかけ

値の無いことがある属性（`age : number?`）を規則の入力に渡す値（`band = band(age: principal.age).band`）は、検査も `gen` も通るのに、生成した TypeScript を `tsc --strict` が断った。

```text
authz/given.ts: error TS2345: Argument of type 'bigint | null | undefined' is not assignable to parameter of type 'bigint | null'.
```

TypeScript は `if (p_Guest.age != null) { … _rule(() => rule_band.band(p_Guest.age)) }` と書いていた。tsc は、プロパティの読み出しに付けた絞り込みを、矢印関数の中へ持ち込まない。規則の書かなければならない入力、列挙の入力、日付の入力、koyomi の関数の日付の引数でも同じである。

同じ形を Python で読むと、別の穴があった。属性を置く局所の変数（`a0`）を、値ごとに 0 から数えていた。Python の変数は関数で一つなので、一つの action に型の違う二つの値があると、`mypy --strict` が一つの名前の二つの型を断る（`Incompatible types in assignment (expression has type "Literal['low', 'high'] | None", variable has type "int | None")`）。木の材料には、どちらの形も無かった。

### 31.2 決定

- **TypeScript は、渡す属性を `const` に置いてから確かめる。** 値の無いことがある属性を規則か koyomi の関数に渡すとき、ガードの前に `const a<k> = <属性>;` と置き、ガードと呼び出しはその名前を読む。`const` の局所の変数の絞り込みは、矢印関数の中へも持ち込まれる。置く行は、値ごとの `if (p_… !== undefined) {` の中にあるので、番号はそのブロックごとに 0 から数える。
- **Python は、局所の変数の番号を action の関数の全体で数える。**
- **Go は変えない。** 属性をポインタで持ち、ガードの中で `(*pGuest.Age)` と読む。書かなくてもよい入力には `a<k> := …` と置いて `&a<k>` を渡す。
- **無いことと `null` は、三つの出力先で同じ「値が無い」。** Python と Go は、記録と入力の型が二つを一つの値（`None`、`nil`）にする。TypeScript は、置き場の記録の属性と action の入力を `x == null ? undefined : …` と読む。値の無い属性を読む値は計算せず、Cedar の実体にも入れない。参照の評価と同じである。

```text
// 前
    if (p_Guest.age != null) {
      context["grown"] = _rule(() => rule_grown.grown(p_Guest.age));
// 後
    const a0 = p_Guest.age;
    if (a0 != null) {
      context["grown"] = _rule(() => rule_grown.grown(a0));
```

### 31.3 捨てたもの

- **呼び出しに `!` や `as` を書くこと。** 型の検査器を黙らせるだけで、ガードと呼び出しが同じ値を読んでいることを型で言わなくなる。
- **TypeScript の番号も関数の全体で数えること。** `const` はブロックの中だけの名前なので、要らない。値ごとのコードが、ほかの値のコードの番号によらずに読める。

### 31.4 変わること

- 値の無いことがある属性を規則か koyomi の関数に渡す gate の TypeScript は、`const` の行が増え、呼び出しがその名前を読む。木の 306 の gate を前と後のビルドで生成して比べると、変わるのは材料の `tests/walk/given/`（新しい `absent` と、前は組めなかった `given`）と、`tests/walk/together/together.gate` の二つの版である。`together` は前から組めていて、`r_Doc.price as rule_sizes.JPY` が `a0 as rule_sizes.JPY` になる。
- Python は、一つの action の二つ目からの局所の変数の名前が変わる（`together` で `a0` が `a1`）。
- Go と、例（`examples/`）の生成物は変わらない。Cedar、ベクタ、`doc` も変わらない。

### 31.5 確かめたこと

- 材料 `tests/walk/given/absent.gate` と日本語の版 `absent.ja.gate`：値の無いことがある属性三つ（数、列挙、日付）と、値の無いことがある action の入力を、規則の書かなければならない入力と書かなくてもよい入力、koyomi の関数、日付の比べ、`allowed` で尋ねる action の入力に渡す。三つの出力先のどれでも型の検査を通り、1,441 件が参照の評価と同じに答える。
- `tests/typescript.rs` は、どのケースも、値の無いことがある物を書かずに渡す回と、`null` にして渡す回の二度走らせ、どちらも参照と比べる。名前で尋ねる入口（30 章）の答えも、二つの回の両方で、型つきの関数の答えと比べる。TypeScript が記録を読むとき `undefined` だけを無いと読むように変えると、`null` の回の `tests/gen/asked.gate` で 2,193 件のうち 704 件が食い違う。
- `const` に置くのをやめると、`tsc --strict` が材料で落ちる。Python の番号を値ごとに戻すと、`mypy --strict` が材料で落ちる。

## 32. 生成先の言語の予約語は、その出力先の W402 にする（2026-11-18）

### 32.1 きっかけ

E008 は、別名（別名の無い ASCII の名前も）が TypeScript の予約語か strict モードの予約語、Python のキーワード、Go のキーワードと同じなら、名前の種類（ファイル、列挙、列挙の値、役割、型、属性、ワークフロー、action、入力、計算する値、ポリシー）を問わず、検査の誤りにしていた（決定の 16 の前の形）。生成するコードがその名前をそのまま書くかも、その出力先を使うかも見ていなかった。属性 `type`（Go のキーワード）、action `delete`（TypeScript の予約語）、役割 `public`（strict モードの予約語）、列挙の値 `return` は、どの出力先のコードも壊さないのに、Cedar も含めて何も書けなかった。一方、型 `Return` は、Python が `Store` のメソッドを `return` と書いて壊れるのに、別名が予約語と一字ずつ同じではないので、検査を通り、`gen` が壊れたコードを書いていた。ファイルの別名 `se_lect` も、Go がパッケージの名前を `select` と書いて壊れるのに、通っていた。

使わない出力先のせいで検査を止めず、組めないコードは `gen` で頼まれた出力先でだけ止める、という決まりに、予約語をそろえる。二つの名前が一つの識別子になる形は、すでにこの形である（24 章）。

### 32.2 決定

1. **生成先の言語の予約語を、E008 から外す。** E008 に残すのは、どの出力先でも取れない名前、つまり Cedar の予約語（`if`・`then`・`else`・`has`・`like`）と、Cedar と sekisho が使う型の名前である。Cedar はどのゲートでも書く。
2. **予約語は、その出力先のコードが名前をそのまま書く所でだけ、その出力先の W402 にする。** `sekisho gen` のその出力先は E401 で何も書かずに 1 で終わり、`ritsu gen` も同じく 1 で終わる。当たる所は次のとおりで、どれも生成器が名前と出どころを返し（`python::declared_fields`・`store_methods`、`typescript::index_word`、`go::keyword`）、`spellings::find` が読む。

   | 所 | 書く形 | 例 | 挙げる出力先 |
   |---|---|---|---|
   | モジュールの一番上の宣言 | 型と列挙（24 章から） | 型 `None` | Python |
   | レコードのフィールド | 属性の別名そのまま | 属性 `class` | Python |
   | `Store` のメソッド | 型の別名を小文字に | 型 `Return`（`return`） | Python |
   | モジュールの名前 | ファイルの別名（`authz/<別名>.py`。`ritsu gen` の `__init__.py` が `from . import <別名> as <別名>` で読む） | ファイル `pass` | Python |
   | 索引が書き出す名前 | ファイルの別名（`ritsu gen` の索引の `export * as <別名> from …`） | ファイル `interface` | TypeScript |
   | パッケージの名前 | ファイルの別名から `_` を除いたもの | ファイル `select` | Go |

   TypeScript の索引は、`delete` や `class` のような予約語は書き出す名前に取れるが、strict モードの予約語（`interface`・`package`・`private`・`protected`・`public`・`static`・`implements`・`let`）と `yield` を TS1214、`await` を TS1262 のエラーにする（TypeScript 5.9.3 で、予約語と strict モードの予約語の全部を一つずつ確かめた）。モジュールそのものは型の検査を通るので、`sekisho gen` だけで書いたコードは使えるが、W402 と E401 は出力先ごとに一つの答えにする。
3. **ほかの所では、予約語は当たらない。** 名前は、プロパティかメソッドか文字列として書くか、前後に何かを付けて書き、Go は大文字で始めて書く。TypeScript は、属性と入力をプロパティに、型を `Store` のメソッド（小文字で始める）に、action を `authorize<Action>`・`<action>Request` に、計算する値を context のキーに書く。Python は、入力のフィールドに予約語があれば `TypedDict` の関数の形で宣言し、入力と計算する値の変数に `in_`・`v_`、principal と resource の変数に `p_`・`r_` を付ける。役割、列挙の値、ポリシー、ワークフローは、どの言語でも文字列である。
4. **ファイルの別名の W402 は、`gate` の行で言う。** 注は、別名が生成するファイルとモジュールとパッケージの名前でもあるので、読むコードも合わせて直すよう言う（ほかの名前の注は、別名が Cedar の名前でもあるので `sekisho diff` で比べるよう言う）。

### 32.3 捨てたもの

- **生成器が予約語をよけること（`class_`、`return_`）。** 名前を付け替えないと 24 章で決めた。呼ぶ側が読む名前が、言語とほかの名前の有無で変わる。
- **どれかの言語の予約語を、全部の出力先の W402 にすること。** 壊れない出力先の `gen` まで止まる。
- **TypeScript の索引の名前を、`ritsu gen` が付け替えること。** 上と同じで、パッケージを読む側の名前が、ファイルの別名から決まらなくなる。

### 32.4 変わること

- 前は E008 で止まったゲートのうち、予約語がどの出力先でもそのまま書く所に無いものは、検査を通り、どの出力先も書く（検査の終了コードが 1 から 0 になる）。そのまま書く所にあるものは、検査を通り（W402）、その出力先の `gen` だけが止まる（E401）。
- 前は検査を通り、`gen` が壊れたコードを書いていた形（Python の `Store` のメソッドが予約語になる型、`_` を除くと Go の予約語になるファイルの別名）は、W402 になり、その出力先の `gen` が止まる。木のゲートには、この章の材料のほかに無い。
- E008 と W402 と E401 の文。E008 の関係するコードに W402 が入る。

### 32.5 確かめたこと

- `tests/spellings.rs`：この章の五つの形（属性 `class`、型 `Return`、ファイル `public`・`pass`・`select`。英語と日本語の版で 10 本、どれも英日の golden を持つ）が、W402 を一つだけ言い、表の出力先だけを挙げる。挙げる出力先の `sekisho gen` は E401 で何も書かず、挙げない出力先は書く。注のとおりに別名を変えた版（`tests/spellings/renamed/`）は何も言われず、四つの出力先のどれでも書く。
- 予約語を、コードがそのまま書かない所の全部に置いた材料（`tests/spellings/words/`。英語と日本語の版）：列挙 `switch`、列挙の値 `new`・`return`・`import`・`export`・`default`・`lambda`・`func`、役割 `public`・`global`、型 `Package`、属性 `default`・`type`・`var`、action `delete`・`return`、入力 `class`、計算する値 `finally`、ポリシー `continue`・`default`・`try`・`with`。何も言われず、四つの出力先のどれでも書く。ツールの段の三つのランナーが、ほかのゲートと同じく、答えを参照の評価と比べる。
- ツールの段で、材料のコードを `ritsu gen` の置き方（索引の `export * as <別名>`、`__init__.py` の `from . import <別名> as <別名>`、Go のパッケージの名前）で、tsc、`mypy --strict`、`go vet` にかけた。66 本のうち、tsc は 14 本、`mypy --strict` は 16 本、`go vet` は 18 本が止まり、止まったのは W402 の挙げるものだけである。名前を変えた版と、予約語の材料は、どれも通る。Go は、構文の誤りのパッケージが `go vet ./...` をほかのパッケージの前に止めるので、ゲートごとに `go vet` を回す。
- 変異：E008 に生成先の予約語を戻すと `tests/names.rs` が落ち、W402 が予約語を全部の言語について言うようにすると `tests/spellings.rs` の二つが落ちる。
- この章の前のバイナリといまのバイナリで、木の `.gate`（358 本）の `check`（英語と日本語）と、四つの出力先を一度に頼んだ `gen` を比べた。`check` が変わったのは `tests/spellings/` の材料の 16 本（英日で 32）だけである。`gen` の終了コードが変わったのは 6 本で、予約語の材料の 2 本が書くようになり、型 `Return` と型 `Roles`（33 章）の材料の 4 本が E401 で止まるようになった。前と後のどちらでも書いた 148 本では、Cedar の 592 のファイルは一字も変わらず、コードの 444 のファイルは 33 章の二つの関数のほかは一字も変わらない。

### 32.6 残したこと

- Cedar の予約語の E008 は、Cedar が文字列として書く名前（役割、action、列挙の値、ポリシー、ワークフローの ID）にも出る。Cedar がそこで予約語を取れるなら、検査の誤りにする理由は無い。この章では Cedar で確かめていない。
- ファイルの別名から `_` を除くと `main` になるゲートは、Go のパッケージの名前が `main` になる。`go vet` は通るが、組めず（`function main is undeclared in the main package`）、ほかのパッケージから読めない（`is a program, not an importable package`）。予約語ではないので、この章の形に入れていない（Go 1.25.5 で確かめた）。

## 33. 役割の一覧（`Roles` と `RolesOf`。2026-11-18）

### 33.1 きっかけ

生成したコードは、レコードの役割に、その型が持てない役割（ゲートが宣言していない役割も）が一つでもあると、どの action も `principal` で拒む（3.8）。サインインがゲートの知らない役割も運ぶ配備（役割をほかの所の名簿から持ってくるサービス）では、その人は何もできない。つなぎのコードは、レコードに入れる前に役割を絞る必要があるが、生成したコードは、ゲートが宣言した役割も、型ごとに持てる役割も出していなかった。

### 33.2 決め

1. **ゲートが宣言した役割の一覧を出す。** TypeScript と Python は `roles()`、Go は `Roles()`。役割の別名を宣言の順に並べ、呼ぶたびに新しいリストを返す。役割の無いゲートでは空のリストである。
2. **principal の型ごとに、持てる役割の一覧を出す。** TypeScript は `rolesOf(t)`、Python は `roles_of(t)`、Go は `RolesOf(t)`。引数は Cedar での型の名前（名前で尋ねる入口の `resources` と同じ書き方）で、その型の `roles` の行に書いた役割を、ゲートが宣言した順に並べ、呼ぶたびに新しいリストを返す。principal でない型と、ゲートに無い名前には、空のリストを返す。`tests/gen/roles_of.gate`（二つの principal の型が、それぞれの役割を持つ）の Go（`ritsu sekisho gen tests/gen/roles_of.gate --target go` の実物から）：

   ```go
   // RolesOf returns the aliases of the roles a principal of the type named t in Cedar can hold
   // (those its roles line names), in the order the gate declares them, in a new slice each call; an
   // empty one for a name that is not a principal's type, or that the gate does not declare. A
   // record's Roles holds only these: one who holds any other is refused every action, so pass the
   // roles a sign-in carries through it first.
   func RolesOf(t string) []string {
   	switch t {
   	case "Employee":
   		return []string{"clerk", "manager"}
   	case "Inspector":
   		return []string{"auditor"}
   	}
   	return []string{}
   }
   ```

3. **つなぎは `RolesOf` で絞る。** 二つの関数の注がそう言い、reference とサイトのページは、サインインが運ぶ役割を principal の型の一覧で絞ってからレコードに入れる例を見せる。ゲートの一覧（`Roles`）は、型ごとに持てる役割が違うゲートでは、ほかの型の役割が残るので、絞るのには使わない。
4. **名前は、コードが自分で使う名前に入る（24 章）。** Go の `Roles` と `RolesOf` は、型の名前とぶつかれば W402 で Go を挙げ、`gen --target go` は E401 で止まる。TypeScript と Python の名前は小文字で始まるので、型（大文字で始まる）とも、action の関数とも、ぶつからない。
5. **sekisho の意味は変えない。** 型が持てない役割を持つ人は、これまでどおり拒む。

### 33.3 捨てたもの

- **ゲートの一覧だけを出すこと。** 型ごとに持てる役割が違うゲートでは、その一覧で絞っても、その型が持てない役割が残り、拒まれる。
- **一覧を公開の変数にすること（Go の `var Roles = []string{…}`）。** 呼ぶ側が中身を書き換えられ、ほかの呼び出しに届く。
- **生成したコードが、知らない役割を黙って捨てること。** 検査が数えた組み合わせ（その型が持てる役割の組だけ）と、コードが答える組み合わせがずれる。

### 33.4 変わること

- 三つの言語のコードに、二つの関数が足される。ほかは一字も変わらない（32.5 の比べ）。
- Go で、型の名前が `Roles` か `RolesOf` になるゲートは、W402 と E401 になる（前は通っていた。木のゲートには、材料のほかに無い）。

### 33.5 確かめたこと

- `tests/gen.rs` の `the_lists_of_the_roles_of_each_language`：例のゲートと日本語の版、`tests/gen/roles_of.gate` とその日本語の版、役割の無い `tests/gen/guards.gate` で、三つの言語の二つの関数を、二つの尋ね先（`cedar` と `avp`）のどちらでも、宣言から作った行と比べる。
- ツールの段の三つのランナー：どのゲートでも、型ごとの一覧とゲートに無い名前の一覧を出させ、ゲートの宣言と比べる。答えが誤りでなく、principal のレコードが役割を持つケースでは、ゲートの全部の役割とゲートに無い役割を一つ、principal の型の一覧で絞ってレコードに入れ、もう一度名前で尋ね、`principal` で拒まれないことを確かめる。`tests/gen/roles_of.gate` では、そうしたケースが一つ以上あることも確かめる。
- 変異：ランナーの絞る所を外すと（全部の役割をそのまま入れると）、三つのランナーのどれもが、`principal` で拒まれるケースを挙げて落ちる。
- `tests/spellings.rs`：型 `Roles`（英語と日本語の版）が W402 で Go だけを挙げ、型 `RolesOf` も Go だけを挙げる。

## 34. `.proto` の列挙の値の名前は、契約を読むほかの所と同じ一つの読み方で読む（2026-11-17）

### 34.1 きっかけ

28 章で、E203 は `.proto` の列挙を値で比べるようになった。値の名前の読み方は、rulec の `import proto` と違っていた。

- 入れ子の列挙（`message PayRequest { enum Category {…} }`）では、前置きを `PayRequest.Category` から作ったので、前置きが一つも落ちなかった。gate の列挙に `category_unspecified` と `category_travel` を求めた。
- 前置きの大文字小文字や `_` の位置が buf の形と違う値（`Category_Food`）は、前置きの付いたまま読み、0 番の `Category_Unspecified` も値として求めた。
- `allow_alias` で一つの番号に付けた二つ目の名前（`CATEGORY_TRIP = 1`）を、別の値として求めた。
- `repeated` のフィールドを一つの値の input に当てても通した。
- `guards proto "…" "Service/Method"` のサービスを、名前の最後の段で比べた。別の package の同じ名前のサービス（`"other.v1.Orders/Refund"`）を書いても、このファイルのサービスに当たった。

### 34.2 決め

- **値の名前は、rulec の `import proto` と言語をまたぐ検査と同じ一つの読み方で読む**（`ritsu_proto::validate::names`）。頭にある列挙自身の名前を、大文字小文字と `_` の位置を問わずに、語の切れ目で終わるときだけ落とし、残りを小文字にする。入れ子の列挙でも、落とすのは列挙自身の名前である。読み方の表は rulec の DESIGN §15.59、決めは §15.346。
- **一つの番号の名前は一つの値である。** gate の列挙がそのどれか一つを持てば通す。足りない値は、その番号の名前を ` / ` で並べて言う（`travel / trip`）。
- **`repeated` のフィールドを一つの値の input に当てると E203 にする**（「the field is `repeated`, a list of values, and the input takes one value」「フィールドは `repeated` で値の並びですが、input は一つの値を受け取ります」）。新しいコードは足さない。
- **サービスは、書いた名前か、ファイルの package を付けた名前で当てる。** 当たらないときの E202 は、書いたサービスの名前で言う。リクエストのメッセージは、型の名前の引き方（`ritsu_proto::resolve_name`）で package から引く。

### 34.3 捨てたもの

- **一つの番号の名前を二つの値として求めること**（前のふるまい）。線の上では一つの値で、protojson はいつも先に書いた名前で書く。gate に二つの値を書かせると、二つ目の値は来ないのに、ポリシーがそれを読めてしまう。
- **`repeated` のフィールドを要素の型で比べて通すこと。** input は Cedar の context の一つの値である。並びを一つの値として渡す形は、sekisho に無い。

### 34.4 変わること

- 前は E203 で止まっていた、入れ子の列挙、buf の形でない前置きの列挙、`allow_alias` の列挙が通る。
- `repeated` のフィールドを一つの値の input に当てていた gate は E203 になる。別の package の同じ名前のサービスを書いた `guards` は E202 になる。どちらも木には無い。
- `explain E203` と reference（`input` の項）が、値の名前の読み方と `repeated` を言う。
- Cedar、生成する三つのコード、ベクタ、`doc`、`api` は変わらない。

### 34.5 確かめたこと

- `tests/contracts.rs` の `every_form_of_an_enum_field_of_a_proto`：列挙のフィールドの 25 通り（buf の形、大文字小文字の違う前置き、語の途中まで続く名前、前置きの無い名前、`allow_alias`、入れ子、package を付けた型の名前、`repeated`、Protovalidate の `in`・`const`・`not_in` など）を、通る形と止まる形に分け、止まる形は E203 の英語と日本語の文で確かめる。前置きのあとが数字の名前とぶつかる名前は、ritsu-proto の `tests/enum_names.rs` が読み方の表の行ごとに確かめる。`a_proto_method_its_fields_and_their_ranges` に、別の package の同じ名前のサービスの E202 を足した。
- 変異：語の切れ目を見ない、同じ番号の名前を別の値に数える、前置きを大文字のときだけ読む、入れ子の列挙の前置きをメッセージの名前から作る、サービスを名前の最後の段で比べる。どれも上の試験が落とす。
- `use proto` を読む木の gate 2 本の `check` は、前のビルドと後のビルドで 1 バイトも違わない。

### 34.6 残したこと

- 日付の input は、フィールドの型を見ずに `.proto` のどのフィールドにも当てる（`repeated` のほか）。数や列挙のフィールドでも通る。前からのふるまいで、どう比べるかは決めていない。

## 35. 生成するコードの型の注は、型の名前の形に依らない文で言う（2026-11-17）

### 35.1 きっかけ

生成した Go と Python の記録の型の注が、名前が s で終わる型のグループを `the ids of the Approverss it is a member of` と書き（`s` を重ねる）、エンティティを指す値を `the id of a Editor`、`the id of a Approvers` と書いていた。TypeScript の注も `The id of a Editor` だった（0.55.2 から）。

### 35.2 決め

- **型は gate が付けた名前のまま書き、複数形にも冠詞の選び分けにもしない。**
  - グループ（Go と Python の、型ごとのフィールド）：`the ids of the groups of type Approvers it is a member of`。TypeScript は一つのフィールドにまとめていて、`The groups it is a member of` のまま。
  - エンティティを指す値（属性。Go、Python、TypeScript）：`the id of an entity of type Editor`。Cedar のスキーマが型を `entity Editor` と書くのに合わせた。
  - action の resource のフィールド（`the id of the Claim`）は、どの名前でも読めるので変えない。
- 日本語の注（`メンバーである Approvers の ID`、`Editor の ID`）は、名前の形で崩れないので変えない。
- Cedar のスキーマと `doc` のページは、型を名前だけで言う（`entity User in [Role, Approvers, Owners]`）ので変えない。

### 35.3 捨てたもの

- **名前から複数形と冠詞を作ること。** `Owners` が一つのグループの名前か、何かの複数形かは、名前からは分からない。母音字で始まる名前に `an` を付けると、`User` は `an User` になる。
- **注の中だけ名前を言い換えること**（`Approver`）。gate の名前と生成コードの注が食い違う。

### 35.4 変わること

- エンティティを指す属性かグループを持つ gate で、生成する Go・Python・TypeScript の英語の注が変わる。木の gate のうちコードを書く 130 本では 24 本で、出力先ごとに 1 ファイル、計 72 ファイルである。注の三つの文のほかは 1 バイトも変わらない。日本語の注は変わらない。
- Cedar、ベクタ、`doc`、`api` は変わらない。

### 35.5 確かめたこと

- `tests/gen.rs` の `the_comments_of_the_records_say_a_type_by_its_name_as_it_is`：`tests/gen/approvals.gate`（型 `Approvers`・`Owners`・`Editor`）と日本語の版を、英語と日本語の注で。変異：四つの文（Go と Python のグループ、Go と Python の値、TypeScript の値）のどれを前に戻しても落ちる。
- 木の gate の全部と足した 2 本を、直す前と後のビルドで `sekisho gen --target go --target python --target typescript` にかけ、英語と日本語の注で比べた（変わったファイルを、三つの文を前に戻すと前と同じになることで確かめた）。
- 足した 2 本は、ツールの段の三つのランナー（`tests/typescript.rs`、`tests/python.rs`、`tests/go.rs`）にも入る。
