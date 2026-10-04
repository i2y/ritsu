# sakai 設計文書

境界づけられたコンテキストと、そのあいだの関係を書く小さな言語。ファイルは `.ctx`、コマンドは `sakai`。どの成果物がどのコンテキストに属するか、境界を越える参照が宣言した関係と公表された言語を通っているか、腐敗防止層が上流の列挙のどの値も読み替えているか、同じ語が違う意味のまま境界を越えていないかを、一式の成果物（rulec・dandori・koyomi・chobo・geas）、proto、コードの import に照らして確かめる。コードの import は、各言語の既存のツール（Python は import-linter、JavaScript と TypeScript は dependency-cruiser、Java は ArchUnit、Go は go-arch-lint）の設定にして、そのツールで確かめる。Rust のクレートの依存は、Cargo が言うものを sakai が確かめる（7.7）。Context Mapper の CML にも書き出す。

名前は境（さかい）から取った。

この文書は設計の段階（A）で書き、言語の芯と地図の検査を作った段階（B）と、例とコードの import の検査の設定と CML を作った段階（C の一部）で、作ったものに合わせて直した。C の段階のうち、一式の成果物を読むところ（PLAN の C.1〜C.5）は、一式の言語を一つの処理系（ritsu）にまとめると決まってから、子プロセスと JSON ではなく ritsu の口で作った（ritsu の D.8。4.1、4.7、12.2）。作ったもの（`.ctx` の構文、`check`、`api`、`explain`、`build`、`export cml`、診断）について貼った sakai の出力と、7 章のツールの出力は、どれも実際に出したもので、`tests/design.rs` が実物と同じかを確かめる。D の段階（ritsu の F.2）で、doc、例の README、`docs/`、README、スキルを作った（10 章、11 章、12.5）。doc のページと README に貼った出力も実際に出したもので、`tests/doc.rs` と `tests/docs.rs` が確かめる。一方、一式のツール（rulec 0.22.1、koyomi 0.1.0、chobo 0.1.0、geas 0.0.1、dandori 0.1.0）と外のツール（import-linter 2.15、dependency-cruiser 16.10.4、ArchUnit 1.5.1、go-arch-lint v1.19.0、depguard v2.2.1、Spring Modulith 2.1.1、Context Mapper CLI 6.12.0、buf 1.54.0）の振る舞いとして書いたことは、2026-10-03 にこの機械（macOS arm64）で実際に走らせて確かめたもので、出力を貼るときは版を添える。外のツールの出力からは、この機械の場所を示すパスと、端末の色の制御文字と、行頭の字下げを省いた。Rust のクレートの依存（7.7）のために試した cargo 1.94.1 と cargo-deny 0.20.2 は、2026-10-04 に同じ機械で走らせた。

## 0. 全体像

```
.ctx（map と context）── 字句・構文 ── 名前の解決 ── 地図（コンテキスト、属し方、公表された言語、関係、用語集、対応）
   │
   ├── 範囲のファイルを歩く ──────── 属し方：どの成果物も、ちょうど一つのコンテキストに属する
   ├── proto を直接読む ─────────── 公表された言語の中身、列挙の値、proto どうしの import
   ├── 一式の言語に口で問う ──────── rulec・koyomi・dandori の参照、rulec の列挙と Connect のサービス、chobo の勘定と振替（4.1）
   │
   ├── check：属し方、境界を越える参照、パターンどうしの整合、対応の網羅、同じ語
   ├── build --target import-linter | dependency-cruiser | archunit | go-arch-lint：コードの import を確かめる設定
   ├── export cml：Context Mapper の CML
   ├── api：地図、属し方、境界を越える参照（JSON）
   └── doc：コンテキストマップの図、コンテキストごとの用語集、関係と対応の表（Markdown と HTML）
```

### 0.1 芯：コンテキストマップのうち、確かめられる部分だけを書く

コンテキストマップは、たいてい図か文章で書かれ、書いた日から実物とずれていく。図の矢印は、どのファイルのどの import のことなのかを言わない。だから、地図に無い参照が増えても誰も気づかないし、図を直しても実物は変わらない。

sakai は、コンテキストマップのうち、実物と突き合わせられる部分だけを書く言語にする。確かめるのは次の三つで、ほかのことは確かめない。

1. **依存**：どの成果物がどのコンテキストに属するか。境界を越える参照が、宣言した関係に沿い、公表された言語を通っているか。
2. **境界を越える語**：二つのコンテキストで同じ名前の語が違う意味を持つとき、その語が対応を書かないまま境界を越えていないか。
3. **対応の網羅**：腐敗防止層が、上流の列挙のどの値にも、下流の値か「断る」を書いているか。上流が値を足すと、検査がその値を名指しする。

**決定**：`.ctx` に書けるのは、この三つのどれかの検査に使うものだけにする。例外は、コンテキストの説明と持ち主のチームで、これは doc と api のために持つ（持ち主を CODEOWNERS と突き合わせることは 14 章に残した）。

理由は、書いても確かめられないものを書かせると、それがまた実物からずれるからである。Bounded Context Canvas が並べる戦略上の分類、ドメインでの役割、ビジネス上の決定、仮定、指標は、どれもコンテキストを設計するうえで大事だが、`.ctx` には入れない（13 章）。用語集も、境界を越える語に絞る。コンテキストの中だけで使う語まで載せさせると、用語集が名前の一覧になり、境界の語が埋もれる。公表された言語のすべての名前に語を付けることも求めない。

看板の言い方は、次を候補にする（★作者が決めること）。rulec と koyomi の看板と同じ三つの句にした。

- **Write the map. Check every crossing. Hand the imports to your linters.**
- **地図を書く。境界を越えるところを全部確かめる。import は各言語のリンターに渡す。**

### 0.2 前提

- **P1**：成果物は、ファイルの単位でコンテキストに属する。属し方はディレクトリとファイルで書き、いちばん深く書いたコンテキストに属する（1.3）。
- **P2**：一式の成果物の中身は、その言語が ritsu の口（ritsu の DESIGN 3.2）で答えるものだけから読む。sakai は一式のどの言語の構文も持たず、どの言語のクレートにも依存しない。口を実装するのは出す側の言語で、それをつないで sakai に渡すのは ritsu である（4.1）。proto は標準の形式なので、ritsu の一つの読み手（ritsu-proto）でファイルを直接読む。どの言語も sakai を知らない。一式を ritsu にまとめる前は、それぞれの CLI の出力（JSON）だけから読む決まりだった。ほかの言語の構文を二か所で持たない、という考えは同じで、渡し方が口になった。
- **P3**：境界を越えて参照してよいのは、公表された言語（proto の package と、rulec が規則ごとに書く Connect のサービス）の要素と、共有カーネルに並べた成果物だけである。
- **P4**：上流と下流の関係は、下流のファイルに書く。下流が、上流のモデルをどう扱うか（そのまま使う順応者か、読み替える腐敗防止層か、要望を出せる顧客か）を決めるのは下流だからである。二つのコンテキストが合意して成り立つパターン（顧客／供給者、共有カーネル、パートナーシップ）は、両方のファイルに書き、検査が食い違いを言う。
- **P5**：語の意味は確かめない。二つの語が同じ意味かは、書いた人の宣言（`as` で上流の語を取り入れる）だけで決め、定義の文を比べない。
- **P6**：コードの import は sakai が読まない。各言語の既存のツールの設定を書き、そのツールで確かめる。書いた設定が地図に合っているかは、`sakai build --check` が言う。
- **P7**：確かめていないことは、確かめていないと言う。ほかの言語が渡されていないとき（sakai のクレートのバイナリ）は、その言語の成果物を読めないと言って止める（E104）。言語がファイルに答えないとき（その言語の検査を通らない、読めない）も、そう言う（E105）。実行時にしか見えない呼び出し（URL を文字列で持つ HTTP、キュー、データベースの共有）は、doc に「確かめていない」と出す（3.7）。黙って通さない。

### 0.3 なぜ別の言語にするか

コンテキストの境界は、一つの成果物の性質ではなく、成果物のあいだの関係である。規則（rulec）、ワークフロー（dandori）、帳簿（chobo）、カレンダー（koyomi）、proto、コードが、それぞれ別のツールで書かれていて、境界はそれらをまたぐ。各ツールに「このファイルはどのコンテキストか」を書く場所を足せば、地図が何十のファイルに散らばり、関係（どこからどこへ、どの言語を通って）を書く場所がどこにもない。各ツールを変えることにもなる。

**決定**：一式のどの言語にも書く場所を足さずに、それらが答えるものを読む別の言語にする。はじめは一式のツールの出力（JSON）を読むつもりだった。一式を ritsu にまとめてからは、ritsu の口で読む（4.1）。

**費用**：構文、検査、診断、文書を、もう一揃い作ることになる。書き方は一式にそろえる（キーワードは英語、名前は日本語で書ける、`(別名)`、字下げのブロック、`v1`、診断の形、CLI の表）。

### 0.4 似たもの

**Context Mapper**（CML。Context Mapper CLI 6.12.0、2024-08-30）。境界づけられたコンテキストと関係のパターンを書く Xtext の DSL で、図、PlantUML、MDSL を出す。関係の書き方は `Ordering [D,ACL]<-[U,OHS,PL] Inventory` で、上流の役割（OHS、PL）と下流の役割（ACL、CF）、顧客／供給者（`[C]<-[S]`）、対称な共有カーネル（`[SK]<->[SK]`）とパートナーシップ（`[P]<->[P]`）がある。別々の道を書く構文は無い。文書は意味の規則を十挙げている（OHS と PL は上流だけ、ACL と CF は下流だけ、ACL と CF は一緒に使わない、対称な関係にはどの役割も付けない、顧客／供給者に CF と OHS は付けない、など）。

手元で確かめたことが三つある。一つ、CLI の `cm validate` は構文の誤りしか言わない。中身を逆アセンブルすると、`CMLResource.getErrors()`（構文解析の誤り）を出すだけで、意味の規則を走らせていない。宣言していないコンテキストへの関係、顧客／供給者に付けた CF、自分自身との関係を書いても「validated without errors」と言い、exit code はいつも 0 だった。二つ、同じ jar の Xtext の検査器（`IResourceValidator` を `CheckMode.ALL` で）を呼ぶ小さな Java のプログラムを書くと、意味の規則が走った。

```
ERROR t.cml:4:10: The CONFORMIST pattern is not applicable for a Customer-Supplier relationship.
ERROR t.cml:4:17: The OPEN-HOST SERVICE pattern is not applicable for a Customer-Supplier relationship.
ERROR t.cml:4:3: Bounded context relationships must be declared between two different bounded contexts.
ERROR t2.cml:3:14: The Bounded Context 'B' is not part of the Context Map.
ERROR t5.cml:4:25: The aggregate 'Orders' is not part of the upstream context 'B'.
ERROR t6.cml:3:14: Couldn't resolve reference to BoundedContext 'Zed'.
```

ただし、文書が挙げる規則のうち「ACL と CF を一緒に使わない」（`[D,ACL,CF]`）と「ORGANIZATIONAL の地図には TEAM だけ」は、6.12.0 の検査器では何も言われなかった。三つ、名前（コンテキスト、地図）は ASCII の識別子に限られる。`BoundedContext 受注` は `mismatched input '受' expecting RULE_ID` になる。文字列（`domainVisionStatement` など）とコメントには日本語を書ける。

Context Mapper には、Spring Boot のコードと Docker Compose から CML を起こす discovery と、CML に照らして Java のコードを確かめる ArchUnit の拡張（context-mapper-archunit-extension 1.2.0、2023-04）がある。後者が確かめるのは、一つのコンテキストの中の集約、エンティティ、値オブジェクトが CML の定義と合っているかで、コンテキストのあいだの依存を関係と照らすことはしない。書いた地図に照らして、境界を越える依存を実物で確かめるツールは見当たらなかった。sakai はそこを受け持ち、CML へは書き出す（8 章）。

**Contextive**。用語集をエディタで見せる（ホバーと補完）。`*.glossary.yml` を置いたディレクトリの下で、その用語集が効く。語には定義、例、別名を書ける。何も確かめない。sakai の用語集は、境界を越える語だけで、検査に使う（1.6）。sakai のコンテキストのファイルをそのコンテキストのディレクトリに置く形は、Contextive が勧める置き方と同じになる。用語集を Contextive の形で書き出すことは 14 章に残した。

**Bounded Context Canvas**（DDD-Crew）。一つのコンテキストについて、名前、目的、戦略上の分類、ドメインでの役割、入ってくるやりとりと出ていくやりとり、ユビキタス言語、ビジネス上の決定、仮定、指標、未解決の問いを一枚に書く。sakai のコンテキストのファイルは、そのうち確かめられる部分（目的、入ってくるやりとり＝公表された言語と公開ホストサービス、出ていくやりとり＝上流との関係、境界を越えるユビキタス言語）だけを持つ。

**Java のツール**。ArchUnit（1.5.1）は、クラスファイルを読み、パッケージやクラスのあいだの依存の規則を JUnit のテストとして書く。jMolecules（jmolecules-ddd 2.0.1）は `@BoundedContext` や `@Module` の注釈をコードに書けるようにし、jmolecules-archunit（0.33.0）は集約の参照、値オブジェクト、レイヤー、オニオン、ヘキサゴナルの規則を持つ。コンテキストのあいだの関係の規則は無い。Spring Modulith（2.1.1）は、アプリケーションのパッケージの直下をモジュールとし、`@NamedInterface` で外に見せるパッケージを、`@ApplicationModule(allowedDependencies = "inventory::api")` で使ってよい相手を、コードの注釈に書く。手元で走らせると、見せていないパッケージへの依存を次のように言った（spring-boot の jar が無いと `ApplicationModules.of` が動かなかった）。

```
- Module 'ordering' depends on non-exposed type shop.inventory.internal.Stock within module 'inventory'!
Method <shop.ordering.domain.Order.place(shop.inventory.api.Reservations)> calls method <shop.inventory.internal.Stock.count(java.lang.String)> in (Order.java:5)
```

モジュールが境界づけられたコンテキストに、見せるパッケージが公表された言語に、使ってよい相手が関係にあたる。sakai に一番近い形だが、Java と Spring に限られ、宣言はコードの注釈に散らばり、順応者と腐敗防止層の区別も、語と値の対応も、proto やほかの成果物も持たない。sakai の Java の出力は ArchUnit にした（7.4）。Spring Modulith も内側で ArchUnit を使っている。

**言語ごとの import の規則**。import-linter（Python）、dependency-cruiser（JavaScript と TypeScript）、go-arch-lint と depguard（Go）。どれも、どのモジュールがどのモジュールを import してよいかを設定に書き、違反を行番号つきで言う。sakai は、地図からこれらの設定を書き、コードの import の検査をそのまま任せる（7 章）。

## 1. 言語

### 1.1 二種類のファイル

**決定**：`.ctx` のファイルは二種類ある。一行目の語で分かれる。

- `map`：コンテキストマップ。どのコンテキストのファイルを集めるか、地図がどの範囲のファイルを覆うか、コードの言語ごとの置き場所。
- `context`：一つの境界づけられたコンテキスト。説明、持ち主、属する成果物とディレクトリ、公表された言語、用語集、ほかのコンテキストとの関係、腐敗防止層での対応。

11 章の例から、地図と在庫と配送のファイルを抜き出す。英語の名前で書いた例（`examples/shop/`）を先に、同じ例を日本語の名前で書いた例（`examples/shop.ja/`）をあとに並べる（`tests/design.rs` が、この文書の `.ctx` の塊を構文に通す）。

```ctx
map Shop(shop) v1
description "A small online shop that takes orders, holds stock, delivers and bills"

use context "contexts/ordering.ctx"
use context "contexts/inventory.ctx"
use context "contexts/delivery.ctx"
use context "contexts/billing.ctx"
use context "contexts/reviews.ctx"

covers "."
proto root "proto"

code python "py"
code typescript "ts"
code java "java/src/main/java"
  test "java/src/test/java"
code go "go"
```

```ctx
context Inventory(inventory) v1
description "Keeps the counts on the warehouse shelves, holds stock for each line of an order, and answers how packing is going"
owner "Warehouse team"
also "stock control"

owns
  dir "../inventory", "../proto/warehouse"
  dir "../py/inventory", "../py/warehouse", "../ts/inventory", "../ts/warehouse"
  dir "../java/src/main/java/inventory", "../java/src/main/java/warehouse"
  dir "../go/inventory", "../go/warehouse"

published language warehouse.v1
  proto "../proto/warehouse/v1/stock.proto"
  open host service StockService, PackingService
  generated dir "../py/warehouse/v1", "../ts/warehouse/v1"
  generated dir "../java/src/main/java/warehouse/v1", "../go/warehouse/v1"

terms
  reservation "Holding shelf stock for one line of an order until it ships or is cancelled"
    means message ReserveResponse
  out_of_stock "The count asked for is not on the shelf"
    means enum Stock value STOCK_SHORT
  packing_status "Whether the goods of an order are in the box. A shortage can turn up part-way through packing"
    means enum PackingStatus
```

```ctx
context Delivery(delivery) v1
description "Decides the day to ship, chooses how to carry, and arranges delivery of the parcel"
owner "Delivery team"

owns
  dir "../delivery", "../proto/shop/delivery"
  dir "../py/delivery", "../py/shop/delivery", "../ts/delivery", "../ts/shop/delivery"
  dir "../java/src/main/java/delivery", "../java/src/main/java/shop/delivery"
  dir "../go/delivery", "../go/shop/delivery"

published language shop.delivery.v1
  proto "../proto/shop/delivery/v1/shipment.proto"
  open host service DeliveryService
  generated dir "../py/shop/delivery/v1", "../ts/shop/delivery/v1"
  generated dir "../java/src/main/java/shop/delivery/v1", "../go/shop/delivery/v1"

published language rulec.urgency.v1
  rulec "../delivery/rules/urgency.rule"
  open host service UrgencyService

terms
  shipment "Handing a parcel over from the warehouse to a carrier"
    means message CreateShipmentRequest
  fragile "A parcel that nothing is put on top of while it travels"
    means enum Handling value HANDLING_FRAGILE
  urgent "Whether to carry by the next-day service"
    means rulec "../delivery/rules/urgency.rule" output urgent

upstream Inventory anticorruption layer
  through warehouse.v1
  layer dir "../py/delivery/acl/inventory", "../ts/delivery/acl/inventory"
  layer dir "../java/src/main/java/delivery/acl/inventory", "../go/delivery/acl/inventory"
  enum PackingStatus -> shipping_decision
    PACKING_STATUS_WAITING -> wait
    PACKING_STATUS_PACKED  -> ship
    PACKING_STATUS_SHORT   -> refuse "A box with an item missing is not shipped. It goes back to ordering"

downstream Billing supplier

shared kernel with Billing
  koyomi "../calendars/tokyo_business_days.cal"
  dir "../py/calendars", "../ts/calendars", "../java/src/main/java/calendars", "../go/calendars"

partnership with Ordering
```

日本語の名前で書いた例（`examples/shop.ja/`）では、次のとおりである。

```ctx
map 通販(shop) v1
description "注文を受け、在庫を押さえ、届け、請求する小さな通販"

use context "contexts/受注.ctx"
use context "contexts/在庫.ctx"
use context "contexts/配送.ctx"
use context "contexts/請求.ctx"
use context "contexts/レビュー.ctx"

covers "."
proto root "proto"

code python "py"
code typescript "ts"
code java "java/src/main/java"
  test "java/src/test/java"
code go "go"
```

```ctx
context 在庫(inventory) v1
description "倉庫の棚にある数を持ち、注文の一行ごとに押さえ、梱包の進みを答える"
owner "倉庫チーム"
also "在庫管理"

owns
  dir "../inventory", "../proto/warehouse"
  dir "../py/inventory", "../py/warehouse", "../ts/inventory", "../ts/warehouse"
  dir "../java/src/main/java/inventory", "../java/src/main/java/warehouse"
  dir "../go/inventory", "../go/warehouse"

published language warehouse.v1
  proto "../proto/warehouse/v1/stock.proto"
  open host service StockService, PackingService
  generated dir "../py/warehouse/v1", "../ts/warehouse/v1"
  generated dir "../java/src/main/java/warehouse/v1", "../go/warehouse/v1"

terms
  引当 "注文の一行のために、棚の在庫を出荷か取消まで押さえておくこと"
    means message ReserveResponse
  在庫切れ "押さえようとした数が棚に無いこと"
    means enum Stock value STOCK_SHORT
  梱包の状態 "注文の品を箱に詰め終えたか。詰める途中で欠品が分かることがある"
    means enum PackingStatus
```

```ctx
context 配送(delivery) v1
description "出荷の日を決め、運び方を選び、荷物を届ける手配をする"
owner "配送チーム"

owns
  dir "../delivery", "../proto/shop/delivery"
  dir "../py/delivery", "../py/shop/delivery", "../ts/delivery", "../ts/shop/delivery"
  dir "../java/src/main/java/delivery", "../java/src/main/java/shop/delivery"
  dir "../go/delivery", "../go/shop/delivery"

published language shop.delivery.v1
  proto "../proto/shop/delivery/v1/shipment.proto"
  open host service DeliveryService
  generated dir "../py/shop/delivery/v1", "../ts/shop/delivery/v1"
  generated dir "../java/src/main/java/shop/delivery/v1", "../go/shop/delivery/v1"

published language rulec.urgency.v1
  rulec "../delivery/rules/出荷の急ぎ.rule"
  open host service UrgencyService

terms
  出荷 "荷物を倉庫から運送会社に渡すこと"
    means message CreateShipmentRequest
  割れ物 "運ぶときに上に物を載せない荷物"
    means enum Handling value HANDLING_FRAGILE
  急ぎ "翌日便で運ぶかどうか"
    means rulec "../delivery/rules/出荷の急ぎ.rule" output 急ぎ

upstream 在庫 anticorruption layer
  through warehouse.v1
  layer dir "../py/delivery/acl/inventory", "../ts/delivery/acl/inventory"
  layer dir "../java/src/main/java/delivery/acl/inventory", "../go/delivery/acl/inventory"
  enum PackingStatus -> 出荷の可否
    PACKING_STATUS_WAITING -> 待つ
    PACKING_STATUS_PACKED  -> 出荷する
    PACKING_STATUS_SHORT   -> refuse "欠品のある箱は出荷しない。受注に戻す"

downstream 請求 supplier

shared kernel with 請求
  koyomi "../calendars/東京の営業日.cal"
  dir "../py/calendars", "../ts/calendars", "../java/src/main/java/calendars", "../go/calendars"

partnership with 受注
```

節の順序は決まっている。map のファイルは、見出し、`description`、`use context`、`covers`、`except`、`proto root`、`code` の順。context のファイルは、見出し、`description`、`owner`、`also`、`owns`、`published language`（いくつでも）、`terms`、関係（`upstream`、`downstream`、`shared kernel with`、`partnership with`、`separate ways from`。いくつでも、どの順でも）の順（違えば E004）。`use context`、`owns`、`covers` は省けない。パスは、そのパスを書いたファイルのディレクトリから読む（koyomi の `use calendar`、rulec の `import proto`、dandori の `use` と同じ）。字下げはスペースで書き、タブは使えない（E005）。`#` から行末まではコメント。見出しの `v1` は人のための目印で、api と生成物の頭に写すだけに使う（一式と同じ）。

**理由**：コンテキストのファイルは、そのコンテキストのチームが持つ。用語集と腐敗防止層の対応は、そのチームが書き、そのチームがレビューする。地図のファイルは、範囲とコードの置き場所という、どのコンテキストのものでもないことだけを持つ。コンテキストのファイルは `contexts/` に集めても、それぞれのコンテキストのディレクトリに置いてもよい（後者は Contextive が勧める置き方と同じ）。

**捨てたもの**：

- 地図を一つのファイルに全部書くこと（CML の形）。小さな地図では読みやすいが、用語集と対応がコンテキストの数だけ一つのファイルに並び、どのチームのものかがファイルで分からない。両側が書くことで合意を表す P4 の形も書けない。
- 関係を地図のファイルにまとめて書くこと。腐敗防止層の対応は下流のチームのもので、関係と対応が別のファイルに分かれる。
- 一つのファイルに複数のコンテキストを書けること。書ける場所が二つになり、一式の「一つのファイルに一つ」（規則、ワークフロー、帳簿、カレンダー）ともずれる。

### 1.2 キーワードと名前

キーワードは英語の一種類だけで、名前（地図、コンテキスト、語、下流の値）は日本語で書ける。rulec、dandori、koyomi、chobo と同じ線引きで、ツールの語（英語）と業務の語（日本語）が見ただけで分かれる。パターンの語は、DDD の英語の名前をそのまま使い、doc と `--lang ja` の診断では、エヴァンスの本の日本語訳で定着した語で書く。

| キーワード | 日本語で書くとき |
|---|---|
| `map` | コンテキストマップ |
| `context` | 境界づけられたコンテキスト |
| `published language` | 公表された言語 |
| `open host service` | 公開ホストサービス |
| `upstream`、`downstream` | 上流、下流 |
| `conformist` | 順応者 |
| `anticorruption layer` | 腐敗防止層 |
| `customer`、`supplier` | 顧客、供給者 |
| `shared kernel` | 共有カーネル |
| `partnership` | パートナーシップ |
| `separate ways` | 別々の道 |
| `terms` | 用語集 |
| `refuse` | 断る |

ほかのキーワードは、`description`、`owner`、`also`、`use`、`covers`、`except`、`proto root`、`code`、`python`、`typescript`、`java`、`go`、`rust`、`test`、`owns`、`dir`、`crate`、`generated`、`through`、`layer`、`means`、`as`、`enum`、`term`、`with`、`from`、成果物のツールの語（`rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`proto`、`file`）と要素の種類の語（2 章）。表は B の段階の `src/kw.rs` に一枚で置く。

**決定**：地図とコンテキストには ASCII の別名を必ず書く（`受注(ordering)`。無ければ E008）。形は `[A-Za-z_][A-Za-z0-9_]*`。

理由は二つ。一つ、別名は CML の名前（ASCII の識別子に限られる。0.4）と、各ツールの設定の名前（import-linter の契約の ID、go-arch-lint のコンポーネントの名前）になる。二つ、別名があれば、日本語の名前を読めない人も、CML と設定から地図を読める。語には別名を要らない。語の名前はどの設定にも出ないからである。

名前の重なり：一つの地図の中で、コンテキストの名前と別名はどれも違うこと。一つのコンテキストの中で、語の名前と `also` の名前はどれも違うこと（E006）。

### 1.3 成果物とディレクトリの属し方

```ctx
owns
  dir "../ordering", "../proto/shop/ordering"
  dandori "../flows/受注.flow"
```

**決定**：成果物はファイルの単位でコンテキストに属する（P1）。`owns` には、ディレクトリ（`dir`）とファイル（2 章の名指し方。`rulec "…"` など）を並べる。成果物のファイルは、それを含むいちばん深い項を書いたコンテキストに属する。ファイルを名指した項は、どのディレクトリの項よりも深いものとして扱う。同じ深さの項を二つのコンテキストが書いていれば E102、どのコンテキストの項にも含まれなければ E101。

決まりは「いちばん深い項が勝つ」で、CODEOWNERS の「後の行が勝つ」と似た、切り出しを書ける形にした。リポジトリの大部分を一つのコンテキストに、その中の一つのディレクトリを別のコンテキストに、と書ける。

成果物とみなすファイルは、次のものだけである。

- 一式の成果物：`.rule`、`.flow`、`.cal`、`.book`、`.geas`
- `.proto`
- 地図の `code` で宣言した言語のコードで、その言語の置き場所の下にあるもの：Python は `.py`、TypeScript は `.ts`・`.tsx`・`.mts`・`.cts`・`.js`・`.jsx`・`.mjs`・`.cjs`、Java は `.java`、Go は `.go`、Rust は `.rs` と、クレートのマニフェスト（`[package]` のある `Cargo.toml`。ワークスペースのためだけのマニフェストは、どのクレートのものでもないので成果物にしない。7.7）

`.ctx` は成果物にしない。祝日の表（koyomi の `data/`）、法令の写し（rulec と koyomi の `sources/`）、OpenAPI や Smithy の記述（dandori の `specs/`）も、それを読む成果物の一部として扱い、属し方を問わない。

範囲は地図の `covers` で決め、`except` で除く。どちらも地図のファイルのディレクトリから読む。さらに、パスのどこかに `.` で始まる名前（`.git`、`.venv`、`.geas`）、`node_modules`、`site-packages`、`__pycache__`、`target` があるファイルは、いつも範囲の外にする。ツールの状態、入れた依存、生成物の置き場所で、geas の `map` が外すものとそろえた（`.` で始まる名前を全部外すのは、`.venv` のような仮想環境を拾わないため）。

proto の import が指す、ほかで配られているファイル（`google/protobuf/…`、`buf/validate/…`、dandori の `dandori/v1/options.proto`）は、リポジトリに写してあっても成果物にしない。どのコンテキストのものでもなく、dandori もファイルが無くても知っているものとして扱っている。

`owns` の項が成果物を一つも含まなければ W101（パスの書き誤りのことが多い）。項のパスが無ければ E009。

**理由**：属し方をファイルとディレクトリで決めるのは、それが CODEOWNERS とも各言語のツールとも同じ単位だからである。要素（規則の出力、proto のメッセージ）の単位で属し方を決めると、一つのファイルが二つのコンテキストにまたがり、コードの import の検査に写せない。

**捨てたもの**：

- 重なりを全部エラーにすること。切り出しが書けず、リポジトリの根を持つコンテキストが書けない。
- `owns` に glob（`../*/inventory`）を書けること。どのファイルがどの項に当たったかの説明が難しくなり、いちばん深い項という決まりと組み合わせにくい。言語ごとにディレクトリを並べれば足りる。
- 属さないファイルをエラーにせず、警告にすること。地図が覆うと言った範囲に持ち主の無い成果物があれば、境界の検査がその成果物を素通りする。

### 1.4 公表された言語と公開ホストサービス

```ctx
published language warehouse.v1
  proto "../proto/warehouse/v1/stock.proto"
  open host service StockService, PackingService
  generated dir "../py/warehouse/v1", "../ts/warehouse/v1"

published language rulec.urgency.v1
  rulec "../delivery/rules/出荷の急ぎ.rule"
  open host service UrgencyService
```

**決定**：公表された言語は proto の package を単位にし、`published language <package>` の塊で書く。塊には、その package を宣言する proto のファイルか、rulec の規則を並べる。

- `proto "…"`：そのファイルの `package` が見出しの package と同じであること（E302）。ファイルはそのコンテキストに属すること（E302）。
- `rulec "…"`：`rulec gen` がその規則のために書く Connect のサービス（rulec の DESIGN 15.112）を、公表された言語にする。package は、rulec が口（`Rules`）で言う Connect のパス（`/rulec.urgency.v1.UrgencyService/Decide`）の package（`rulec.urgency.v1`）で、見出しと同じであること（E302）。規則はそのコンテキストに属すること。
- `open host service <サービス>`：公開ホストサービスにするサービスを、package の中の名前で並べる。そのサービスが塊の proto にあること（rulec なら、Connect のパスのサービスと同じこと）。無ければ E301。
- `generated dir "…"`：その package の proto から生成したコードの置き場所。コードの import の検査で、ほかのコンテキストが import してよいところになる（7 章）。そのコンテキストに属するディレクトリであること（E302）。
- `crate "…"`：Rust のクレート（その `Cargo.toml` のあるディレクトリ）を、公表された言語にする（7.7）。見出しは、コードが書くクレートの名前（クレートの名前の `-` を `_` にしたもの。`ritsu-ports` なら `ritsu_ports`）で、違えば E302。クレートはそのコンテキストに属し、地図の `code rust` のワークスペースのメンバーであること（E302）。クレートは境界の向こうから呼ぶサービスを持たないので、`open host service` は書けない（E301）。一つの塊に一つのクレートで、proto や規則とは混ぜない（E004）。

公表された言語の要素は、塊の proto で定義されたもの全部（メッセージ、そのフィールド、列挙、その値、サービス、そのメソッド）である。rulec の塊なら、rulec が口で名前を言うもの（規則の入力、出力、列挙と値）である。Rust のクレートの塊は要素を持たない（sakai はクレートの中の型を読まず、依存をクレートの単位で確かめる）。

**理由**：package を単位にしたのは、proto の世界で、package が言語の名前とバージョンを兼ねているからである（`warehouse.v1`、`rulec.urgency.v1`）。互換が崩れたときは package のバージョンが上がり（buf の `breaking` がそれを止める）、地図では、下流がどのバージョンを通るかが `through` に書いてある。rulec の Connect のサービスを公表された言語に入れたのは、規則を一か所のサービスに置いて、ほかのコンテキストから Connect で呼ぶ形が、一式の中にもうあるからである（dandori の DESIGN 1.13）。Rust のクレートを入れたのは、Rust では、クレートがほかのクレートに見せる型とトレイトの集まりで、ほかのクレートは、それを依存に書いたときだけ使えるからである。ritsu では、言語のクレートが共通に使う `ritsu-ports` などが、言語のクレートの上流の公表された言語にあたる（ritsu の DESIGN 3.4）。

公開ホストサービスと公表された言語を分けたのは、DDD での意味が違うからである。公表された言語は、やりとりする型の言語で、イベントやメッセージとして型だけを公表することもある。公開ホストサービスは、だれでも呼べるように開いたサービスで、公表された言語を使う。sakai では、サービスを呼ぶ参照には公開ホストサービスが要り、型を使うだけの参照には公表された言語が要る（3.3）。公開ホストサービスを `published language` の塊の中に書くので、公表された言語の無い公開ホストサービスは書けない。塊の proto に無いサービスを並べたときに E301 になる。

**捨てたもの**：

- 公表された言語を、名前を付けた成果物の集まりにすること（`published language 在庫API`）。proto がすでに package という名前を持っていて、名前が二つになる。
- OpenAPI の記述や JSON Schema を公表された言語に入れること。対応の網羅に使う列挙を proto から読むと決めてある。rulec は JSON Schema の列挙も取り込める（rulec の DESIGN 15.60）ので、要れば同じ形で足せる（14 章）。
- chobo の帳簿や koyomi のカレンダーを、そのまま公表された言語にすること。どちらも生成したコードを呼ぶ側に同梱する形で、境界の向こうから呼ぶ口（サービスと型の契約）を持たない。ほかのコンテキストに見せたければ、proto のサービスの後ろに置く（在庫の例の `StockService` は、chobo の帳簿の後ろに立つ）。二つのコンテキストで同じカレンダーを使うなら、共有カーネルに並べる（1.5）。

### 1.5 関係とパターン

```ctx
upstream 在庫 anticorruption layer        # 配送は在庫の下流で、在庫のモデルを読み替える
  through warehouse.v1
  layer dir "../py/delivery/acl/inventory"

downstream 請求 supplier                  # 請求は配送の顧客で、配送はそれを引き受ける

shared kernel with 請求
  koyomi "../calendars/東京の営業日.cal"

partnership with 受注

separate ways from 請求                   # レビューのファイルに書く
```

**決定**：

- **上流と下流**は、下流のファイルに `upstream <上流> <役割>` と書く。役割は、`conformist`（順応者）、`anticorruption layer`（腐敗防止層）、`customer`（顧客）のどれかで、`customer` には `anticorruption layer` を添えてよい（`customer, anticorruption layer`）。役割の無い `upstream` は書けない（E002）。塊の中に、通る公表された言語の package を `through` で並べる（省けない）。腐敗防止層なら、層の置き場所（`layer`）と対応（1.7）を書ける。
- **顧客／供給者**は、顧客の側の `upstream <供給者> customer` と、供給者の側の `downstream <顧客> supplier` の両方で成り立つ。片側だけなら E303（顧客の側だけでも、下流が書いた `upstream` として参照は通す。3.3）。
- **共有カーネル**は、両方のファイルに `shared kernel with <相手>` を書き、共有する成果物とディレクトリを並べる。両側の並びが違えば E307。
- **パートナーシップ**は、両方のファイルに `partnership with <相手>` を書く。片側だけなら E309。
- **別々の道**は、どちらかのファイルに `separate ways from <相手>` を書けば成り立つ。二つのあいだに参照があれば E206、ほかの関係があれば E310。

関係ごとに、境界を越える参照で許すものは次のとおり（3.3）。

| 関係 | 許す参照 |
|---|---|
| 順応者 | 下流から、上流が公表した `through` の package の要素へ。下流の公表された言語が、上流の型をそのまま使ってもよい |
| 腐敗防止層 | 下流から、上流が公表した `through` の package の要素へ。`layer` を書いたなら、参照する成果物が層の中にあるときだけ。下流の公表された言語に、上流の型を出してはいけない（E205） |
| 顧客 | 順応者と同じ。供給者がそれを引き受けていること（`downstream … supplier`） |
| 共有カーネル | 両側から、並べた成果物とディレクトリへ（公表された言語でなくてよい） |
| パートナーシップ | 両側から、相手が公表した package の要素へ（`through` は要らない） |
| 別々の道 | 何も許さない |

サービスを呼ぶ参照（dandori の `connect` のタスク、規則を Connect で呼ぶ `use rule … connect`）には、そのサービスが相手の `open host service` に並んでいることも求める（E207。パートナーシップでも同じ）。サービスを呼ぶ参照は、いまは dandori のワークフローにしか無い（4.7）。

パターンどうしの整合として、次を確かめる（3.4）。公開ホストサービスのサービスが公表された言語の proto にあること（E301）。順応者は対応も層も持たない（E304）。対応や層を書くなら腐敗防止層であること（E305）。順応者と顧客、順応者と腐敗防止層は一緒に書けない（E306）。腐敗防止層の `layer` は下流のものであること（E313）。上流が `through` の package を公表していること（E312）。自分自身との関係は書けない（E311）。二つのコンテキストが互いに上流なら W301。

**理由**：関係を下流が書くのは、DDD で、順応するか読み替えるかを決めるのが下流だからである（P4）。上流が自分の下流を全部並べる形だと、公開ホストサービスを開いた上流は、知らないうちに増える下流を書き続けることになる。顧客／供給者、共有カーネル、パートナーシップは、二つのチームの合意そのものなので、片側の宣言だけでは成り立たせない。

役割を必ず書かせるのは、役割で検査が変わるからである（対応が要るか、上流の型を自分の公表された言語に出してよいか）。CML の `[D]<-[U]` のような、役割の無い上流と下流は書けない。

顧客と順応者を一緒に書けないのは、Context Mapper の規則（顧客／供給者に CF は付けない）と同じ考えで、要望を出して上流を動かせる顧客は、上流にただ合わせる順応者ではないからである。顧客に腐敗防止層を添えることは、Context Mapper が警告にとどめているのと同じく許す。

別々の道を書けるようにしたのは、関係が無いことと、関係を持たないと決めたことを分けるためである。関係が無いだけなら、だれかが `upstream` を足せば参照が通る。別々の道は、その足し方自体を E310 で止める。

**捨てたもの**：

- 上流が下流を並べる形（上流のファイルに `downstream 受注 conformist`）。上に書いた理由。
- 上流と下流の関係に `through` を省けること。どの公表された言語を通ってよいかが、関係の中身である。
- 互いに上流であることをエラーにすること。二つのコンテキストが、別々の公表された言語で互いに呼ぶことは実際にある。DDD では自律を損なう形なので、警告にとどめる。

### 1.6 用語集

```ctx
terms
  注文 "客が確定させた購入の申し込み。取り消されても消えない"
    means message Order
  キャンセル "出荷の前に、客の申し出で注文を取り消すこと"
    means enum OrderStatus value ORDER_STATUS_CANCELLED
```

```ctx
terms
  キャンセル "請求を確定したあとに請求を取り消し、返金すること"
  注文 as 受注.注文
```

**決定**：

- 語は、名前と定義の文を持つ。`also` で同じものの別の呼び方を並べられる。
- `means` は、その語が指す、自分の公表された言語の要素を名指す（2 章。proto の要素は、自分の公表された言語の package の中の名前で短く書ける）。自分の公表された言語に無い要素なら E408。`means` を持つ語は、その要素とともに境界を越えていく。
- `as <上流>.<語>` は、関係のある相手の語を、同じ意味のまま取り入れる（定義の文は書かない）。相手の用語集にその語が無いか、相手との関係が無ければ E410。
- 境界を越えない語は W401。地図が名指す要素を一つでも引けなかった地図では、何が越えるかが分からないので出さない。ほかの言語の成果物を一つでも読めなかった地図（E104、E105）でも、同じ理由で出さない（P7）。11 章の例の請求の「キャンセル」は、規則の `import proto` を通って受注の「キャンセル」とぶつかる語で、規則を読まずに W401 を出すと、越える語を越えないと言うことになる。語が境界を越えるのは、次のどれかに当てはまるときである。
  - `means` を持つ（自分の公表された言語の要素とともに外へ出ていく）。
  - 腐敗防止層の対応の先の語である（1.7）。
  - `as` で上流の語を取り入れている。
  - 上流から越えてくる同じ名前の語とぶつかっている（下の「同じ語の検査」）。

**同じ語の検査**（3.6）：上流の語 t が `means` の要素とともに下流 D に越えてくる（D の成果物がその要素を参照している）とき、D の用語集に t と同じ名前（`also` を含む）の語があり、それが `as` で t を取り入れたものでなければ、二つは違う意味の同じ語である。このとき、D の腐敗防止層の対応が、越えてくる要素を読み替えていなければ E406 になる。読み替えた先の値や語が、D の同じ名前の語と同じ名前なら E407（違う意味の語を、同じ名前で受けていることになる）。D が腐敗防止層でなければ（順応者、顧客、パートナー）、対応は書けないので E406 で、直し方は「名前を変える」「`as` で取り入れて同じ意味にする」「腐敗防止層にして読み替える」のどれかである。

**理由**：二つの語が同じ意味かは、ツールには分からない（P5）。定義の文が一字違えば違う意味、と決めることもできるが、言い回しの違いで対応を書かされ、同じ文をたまたま書いた二つの語が同じ意味になる。だから、同じ意味だと言うのは書いた人の `as` だけにした。

検査の対象を「越えてくる要素にぶつかる語」に絞ったのは、境界づけられたコンテキストの考えそのものによる。二つのコンテキストが同じ語を違う意味で使うのは正しいことで、それが問題になるのは、境界を越えたときだけである。

**捨てたもの**：

- 公表された言語のすべての要素に語を付けさせること。用語集が名前の一覧になる（0.1）。
- 語の名前の英字の別名。どの設定にも出ないので、検査に使わない。
- 共有カーネルの語を検査すること。共有カーネルは二つのチームが一緒に持つモデルで、語の食い違いは合意の問題である。共有カーネルの成果物を通る参照は、同じ語の検査の対象にしない。

### 1.7 腐敗防止層での対応

```ctx
upstream 在庫 anticorruption layer
  through warehouse.v1
  enum PackingStatus -> 出荷の可否
    PACKING_STATUS_WAITING -> 待つ
    PACKING_STATUS_PACKED  -> 出荷する
    PACKING_STATUS_SHORT   -> refuse "欠品のある箱は出荷しない。受注に戻す"
  term 引当 -> 押さえ
```

（`term` の行は、配送の用語集に「押さえ」という語があるときの書き方の例で、11 章の例には無い）

```ctx
upstream 受注 anticorruption layer
  through shop.ordering.v1
  layer rulec "../billing/rules/請求の要否.rule"
  enum OrderStatus -> rulec "../billing/rules/請求の要否.rule" enum 注文の状態
```

**決定**：

- `enum <上流の列挙> -> <下流の先>` の下に、上流の列挙の値ごとに、下流の値か `refuse`（断る。理由の文を添えられる）を書く。上流の列挙は `through` の package の中の名前で短く書ける（2 章）。
- 上流の列挙の値は proto から読む。対応に無い値があれば E401 で、無い値を全部名指す。上流が値を足したときに出るのがこれである。上流の列挙に無い値を書けば E402。
- 0 番の値で、名前から列挙の名前の接頭辞（`PackingStatus` なら `PACKING_STATUS_`）を外すと `unspecified`（大文字と小文字は問わない）になるものは、値が無いことを表す印なので、対応に要らない（書けば W402）。ほかの名前の 0 番の値（rulec の corpus の契約にある `HANDLING_STANDARD = 0`）は、本当の値として対応を求める。rulec が `import proto` で列挙を突き合わせるとき（rulec の DESIGN 15.59）と、dandori が proto から型を作るとき（dandori の DESIGN 1.12）の決まりと同じである。
- 下流の先は三つの形のどれかである。
  - rulec の列挙（`rulec "…" enum 注文の状態`）：その規則が `import proto` で上流の列挙を取り込んでいるなら、値の対応は rulec が決めている（rulec が口（`Rules`）で渡す事実の Connect の列挙が、規則の値の名前と proto の値の名前と番号を並べる）。sakai はそれを対応として読み、値の行を書かなくてよい。書いたなら、rulec の対応と同じであること（E405）。網羅は rulec が E032 と E033 で守る。規則が取り込んでいない列挙なら、値の行が要り、右辺は規則の列挙の値であること（E403）。
  - 下流の proto の列挙（`proto "…" enum ShippingDecision`、または自分の公表された言語の中の短い名前）：右辺はその列挙の値であること（E403）。
  - 名前だけ（`出荷の可否`）：下流の値は、書いたとおりに受け取り、確かめない（doc にそう出す）。下流の値がコードの中の列挙にしか無いとき、この形になる。
- `term <上流の語> -> <下流の語>` は、上流の語を下流のどの語として受けるかを書く。どちらの用語集にもその語があること（E409）。
- 腐敗防止層で、下流の成果物が参照している上流の列挙に対応が無ければ E404。参照は 3.3 で読めるもの（proto の import、rulec の `import proto` と `shape`）に限る。コードの import はモジュールの単位でしか分からないので、どの列挙を使っているかは分からず、E404 の対象にならない。
- 対応を書けるのは腐敗防止層だけである（E304、E305）。

**理由**：網羅を「上流の値ごとに、下流の値か断る」と決めたのは、dandori の `match` の網羅と同じで、上流が値を足したときに、その値をどう扱うかをだれかが決めるまで止めるためである。上流が proto に値を足すのはワイヤの上では互換な変更で、buf の `breaking` は止めない。止めなければ、下流のコードの `default` の分岐が、だれも決めていない扱いを新しい値に与える（rulec の DESIGN 15.59 が規則について書いたのと同じ事故）。

rulec の規則を対応の先にできるようにしたのは、規則の `import proto` が、すでに対応そのもので、網羅も rulec が確かめているからである。同じ対応を `.ctx` に二度書かせると、二つが食い違う。sakai は rulec の対応を読み、doc の対応の表に出す。

「断る」を書けるようにしたのは、下流がその値を受け取らないと決めることも、扱いの一つだからである。腐敗防止層は、その値が来たら呼び出しを失敗させる。それがコードで実際にそうなっているかは、sakai には分からない（3.7）。

**捨てたもの**：

- 腐敗防止層の対応から、翻訳のコードを生成すること。対応の網羅を確かめるのと、コードを生成するのは別の決定で、生成すれば各言語の出力と突き合わせのテストが要る。14 章に残した。
- 対応を書かなかった値を「そのまま」とみなすこと。新しい値がだれも決めないまま通る。

## 2. 成果物の名指し方

yuen（要件の来歴の言語）と sakai は、成果物を同じ形で名指す。この章の決まりは、A の段階の終わりに二つの設計を突き合わせて決めたもので、二つの言語は同じ形を一字も違えずに使う。決まりの試しの表 `tests/fixtures/naming.tsv` を二つのリポジトリが同じものとして持ち、`tests/naming.rs` が全行を確かめる。

### 2.1 形

`<ツール> "<パス>" [<種類> <名前>]…`

```
rulec "rules/送料.rule"
rulec "rules/送料.rule" output 送料
rulec "billing/rules/請求の要否.rule" enum 注文の状態 value 受注で取消
dandori "order.flow"
koyomi "支払条件.cal" date 支払日
koyomi "支払条件.cal" claim 受領から60日以内
chobo "在庫.book" transfer 引当
geas "greeter.geas" claim "rejects an empty name"
proto "shop/v1/order.proto" service OrderService method Create
proto "warehouse/v1/stock.proto" enum PackingStatus value PACKING_STATUS_SHORT
proto "shop/v1/order.proto" message Order.Line field quantity
file "src/app.py"
yuen "民法の期間.req" requirement 満了日_142条
sakai "contexts/受注.ctx" term キャンセル
```

ツールの語のあとに、パスを `"…"` で書き、そのあとに種類と名前の組をゼロ個以上続ける。組がゼロ個ならファイルそのものを指す。

組は、ツールの中の構造どおりに入れ子にできる。入れ子にできるのは次だけで、ほかのツールの組は一つまでである。

- proto：`service S [method M]`、`message M [field f]`、`enum E [value V]`。入れ子のメッセージは名前を `.` でつなぐ（`message Order.Line`）。
- rulec：`enum E [value V]`。

子の種類（`method`、`field`、`value`）は、親の種類のすぐあとにしか書けない。子の組は一つまでである。

sakai が列挙の値（対応の網羅）とフィールド（語の `means`）を名指すので、入れ子が要る。yuen もこの形をそのまま受け付ける。

### 2.2 ツールの語と種類の語

ツールの語は `rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`proto`、`file`、`yuen`、`sakai` の九つである。

種類の語は、それぞれのツールが JSON で出す名前の種類から取り、二つの言語で使う種類を合わせた。表の右の列は、種類の語を取った元である。ritsu に取り込んでからは、どの言語の名前も、その言語の口（`Items`）が渡す（ritsu の DESIGN 6.4）。

| ツール | ファイル | 種類の語 | 名前を読むところ |
|---|---|---|---|
| `rulec` | `.rule` | `input`、`output`、`enum`（下に `value`）、`table`、`clause`、`define`、`derive`、`machine`、`source` | `rulec api` の `python.params`、`python.outputs`、`python.enums` と `values`、`machine.name`、`sources`。`rulec graph` の `nodes`（`input`、`output`、`table`、`clause`、`define`、`derive`）。`rulec certificate` の `tables` |
| `koyomi` | `.cal` | `input`、`date`、`claim`、`source` | `koyomi api` の `inputs`、`dates`、`claims`、`sources`（カレンダーのファイルでは `calendar.sources`） |
| `chobo` | `.book` | `unit`、`account`、`transfer` | `chobo api` の `units`、`accounts`、`transfers` |
| `geas` | `.geas` | `claim` | `geas map` の記録の一行め（`.geas/<stem>.map.jsonl` の主張の並び）。geas は主張を、走らせずに並べるコマンドを持たない（4.6） |
| `dandori` | `.flow` | `task`、`case`、`record`（下に `field`）、`enum`（下に `value`）、`input`、`output` | ritsu の D.6 で足した（ritsu の DESIGN 6.3）。名前は dandori の口（`Items`）が渡す。sakai が dandori から読むのは、ワークフローが名指すもの（`References`。4.7）で、`.ctx` に書いた dandori の種類の語は形だけを確かめる |
| `proto` | `.proto` | `service`（下に `method`）、`message`（下に `field`）、`enum`（下に `value`） | proto のファイルの中の名前 |
| `file` | 何でも | なし | |
| `yuen` | `.req` | `requirement`、`source` | `yuen api` |
| `sakai` | `.ctx` | `context`、`term` | sakai の地図とコンテキストのファイル |

どちらの言語も、自分が使わない種類も名指しとして受け付け、JSON に出せる。sakai が名前まで確かめるのは、自分が読むもの（proto の要素と、rulec の `input`、`output`、`enum`、`value`）だけで、ほかの種類（koyomi の `date`、yuen の `requirement` など）は形だけを確かめる。rulec の名前は、rulec が口（`Rules`）で渡す事実から引く。名指した規則にその名前が無ければ、proto の要素と同じく E007 である。

ディレクトリは成果物ではないので、ツールの語にしない。sakai の `.ctx` は、属し方、腐敗防止層の置き場所、生成したコードの置き場所、共有カーネルにディレクトリを書くので、`dir "<パス>"` を `.ctx` の構文の語として持つ（名指しの形ではない）。JSON では、ディレクトリはパスの文字列で出す。

### 2.3 名前

名前は、ツールの JSON の `name` で書き、別名（`alias`）では書かない。語か `"…"` で書く。語は、空白、`"`、`#` を含まない一続きの文字で、頭が数字でもよい（`40営業日以内`）。そのほか（空白を含む主張の名前など）は `"…"` で書き、その中のエスケープは `\"` と `\\` だけである（ほかのエスケープは誤り）。名前は書いたとおりの文字列で比べ、Unicode の正規化はせず、大文字と小文字も区別する。proto の名前は、そのファイルの package から見た名前で書く（`OrderService`、入れ子は `Order.Line`）。

文字列の外に全角の空白があれば誤りである。全角の空白は半角の空白と見分けにくく、名前の区切りなのか名前の一部なのかが、読む人に分からないからで、`.ctx` の字句も全角の空白を E001 で断っている。文字列の中の全角の空白は名前の一部で、`text` ではその名前を引用符で囲む（`claim "受領から　60日"`）。ツールの語と種類の語は `"…"` で書けず、種類のあとに名前が無いのも誤りである。

### 2.4 パス

`.ctx` の中では、パスを書いたファイルのディレクトリからの相対で書く（koyomi の `use calendar`、rulec の `import proto`、dandori の `use` と同じ）。区切りは `/` で、`.` と `..` は字の上で畳む（シンボリックリンクはたどらない）。末尾の `/` も畳んで取り除く（`"src/"` は `"src"`。ディレクトリかどうかはファイルを見て決める）。書いたファイルのディレクトリそのものは `"."` と書く。絶対パスと空のパス（`""`）は書けない（E012）。

JSON（`api` と、名指しの JSON の形）では、パスをルートからの相対にする。ルートは、`sakai` に最初に渡したパスの上で `.git` を持つ一番近いディレクトリで、git は走らせずにディレクトリを見て探す。無ければ、渡したディレクトリ（ファイルなら、それがあるディレクトリ）がルートになる。`--root` で替えられる。ルートそのものは `"."` と書く。ルートの外に出るパスは E012 である。geas の `map` の記録も git の差分も、このルートからの相対でパスを書くので、そのまま突き合わせられる。

診断の文面では、ファイルの場所を、一式のツール（koyomi、chobo、dandori、geas）と同じく、sakai を走らせたディレクトリから、渡したパスと同じ書き方で書く。相対パスを渡したなら走らせたディレクトリからの相対、絶対パスを渡したなら絶対パスになる。ファイルの場所とは、位置の `<パス>:<行>:<列>`、関わるものの `<パス>:<行>`、文と注の中のファイルのパス（「「在庫」の proto/warehouse/v1/stock.proto が…」）、通ったときの要約の行の地図である。一方、名指しを文字にして出すときは（関わるものの `proto "…" enum OrderStatus`、文と注の中の名指し、`.ctx` の項を写した `dir "…"`）、JSON と同じくルートからの相対の形のままにする。`--format json` では、ファイルの場所である `file` と `references[].file` も、名指しと同じくルートからの相対で書き、地図ごとの JSON の外側に `root`（走らせたディレクトリから見たルート。絶対パスを渡したなら絶対パス）を添える。JSON は人が開くためでなく、ほかのツールと突き合わせるためのもので、ritsu のどの言語も同じ形で書く（ritsu の C.8 で替えた。12.1）。開くときは `root` と `file` をつなぐ。文と注の中のファイルのパスは、JSON でも文面と同じに書く。ルートで走らせれば、文面と JSON の書き方は同じになる（`tests/golden/` の変異の診断は、変異のディレクトリをルートにして、そこで `sakai check .` を走らせた形である）。`tests/cli.rs` が、ルートの上のディレクトリ、ルートの下のディレクトリ、絶対パスの三つで走らせて、この書き方を確かめる。

**理由**：文面のファイルの場所は、読む人がそのまま開くためのものなので、走らせた場所から書く。名指しと JSON は、読み直すと同じものを指し、ほかのツール（yuen、git の差分、geas の記録）や api の JSON と突き合わせられることが大事なので、どこで走らせても同じ形にする。

**捨てたもの**：

- 文面の診断のパスを全部ルートからの相対にすること（B の段階の形）。リポジトリの下のディレクトリで走らせると、診断に出るファイルの場所がそこからは開けないパスになり、一式のツールの診断と並べたときに書き方がそろわない。
- JSON のファイルの場所を文面と同じく走らせたディレクトリから書くこと（ritsu の C.8 の前の形）。同じ診断が、走らせた場所によって違う JSON になり、yuen（ルートからの相対）と食い違っていた。
- 名指しの中のパスも走らせたディレクトリから書くこと。同じ名指しが、走らせた場所によって違う文字になり、読み直しても api の名指しと一致しない。

### 2.5 同じ・含む

二つの名指しは、ツールの語と、ルートからのパスと、組の並びが同じときに同じものを指す。ファイルは、中のものを全部含む。親の組（proto の `service`・`message`・`enum`、rulec の `enum`）は、その子を全部含む。sakai は「含む」を、境界を越えてくる要素を数えるとき（列挙が越えてくれば、その値も越えてくる）と、語の `means` の要素が越えてきたかを決めるときに使う。

### 2.6 JSON の形

```json
{"text":"proto \"warehouse/v1/stock.proto\" enum PackingStatus value PACKING_STATUS_SHORT","tool":"proto","path":"warehouse/v1/stock.proto","items":[["enum","PackingStatus"],["value","PACKING_STATUS_SHORT"]]}
```

キーはこの順に出す。`items` は種類と名前の組の並び、`text` は、パスをルートからの相対に直し、名前を語で書けるなら引用符なしで、書けなければ `"…"` で書いた形である。serde_json の詰めた書き方（空白なし、ASCII でない文字はそのまま）で出す。

### 2.7 `.ctx` の中だけの短い書き方

`means` の右辺では、自分の公表された言語の proto の要素を、パスを省いて書ける（`message Order`、`enum Stock value STOCK_SHORT`）。`enum` の対応の左辺では、`through` の package の列挙を名前だけで書ける（`enum PackingStatus`）。対応の先でも、自分の公表された言語の列挙を `enum <名前>` と書ける。sakai はどのファイルの要素かを決め、api と診断には長い形で書く。名前が二つの package に当たるときは E007 で、package から書く（`message warehouse.v1.ReserveResponse`）。

### 2.8 試しの表と、決着した食い違い

`tests/fixtures/naming.tsv` は、この章の決まりの試しの表である。一行が一つの名指しで、タブの左が名指し（書いたファイルがルートにあるとする）、右が JSON か `ERROR: <理由>` である。`tests/naming.rs` は、全行で右と同じ JSON を出すことと、`ERROR` の行をどれも誤りにすることを確かめる。表を直すときは、yuen の同じ表も同じに直す。

`tests/naming.rs` は、誤りの行が、表の理由のとおりの理由で断られることも確かめる（理由ごとに、sakai の英語の文面に出る語句を表にしてある）。別の理由でたまたま断られた行を、通ったことにしないためである。

`.ctx` に書いた名指しの誤りは、形の誤り（知らないツールの語、そのツールに無い種類の語、親のすぐあとでない子、二つめの子、`file` の種類、文字列の外の全角の空白、使えないエスケープ）が E011、絶対パス、ルートの外に出るパス、空のパスが E012 である。

A の段階の終わりに、yuen の DESIGN 2 章（2.8 の七項目）と並べると、五つが食い違っていた。それぞれ次のように決めた。

| 項目 | 決めたこと |
|---|---|
| 組の数 | ツールの構造どおりに入れ子にする（proto の `service … method …`、`message … field …`、`enum … value …` と、rulec の `enum … value …`）。ほかのツールは一つまで |
| ツールの語 | `dir` はツールの語にしない。sakai の `.ctx` の構文の語にとどめる（yuen の `scope` の書き方は yuen の構文の中で決める） |
| パスの基点 | JSON はルート（`.git` を持つ一番近いディレクトリ。`--root` で替える）からの相対。ルートの外はエラー。診断の文面のファイルの場所は、走らせたディレクトリから書く（名指しはルートからの相対のまま。B の段階のあとで決めた。2.4） |
| 種類の語 | 二つの言語の和（2.2 の表） |
| 同じ・含む | yuen の「含む」に、proto の `message`・`enum` と rulec の `enum` が子を含むことを足した |
| JSON の形 | `{"text", "tool", "path", "items"}` |

名前の決まり（2.3）は、A の段階のうちに yuen の 2.4 に合わせてあった。

B の段階のあとで、二つの言語の細かい形をもう一度そろえた。試しの表に 9 行を足して 36 行にし（全角の空白、エスケープ、`"…"` で書いた種類とツールの語、名前の無い種類、空のパス、`"."`、末尾の `/`。2.3、2.4）、診断の文面のファイルの場所を走らせたディレクトリから書くことにした（名指しはルートからの相対のまま。2.4）。

## 3. 検査

### 3.1 何を確かめるか

`sakai check` は、次の順に確かめる。段 1 と段 2 にエラーがあれば、後の段は走らせない（構文や名前が読めなければ属し方は決められず、属し方が決まらなければ参照を確かめる相手が無い）。

| 段 | 確かめること | コード |
|---|---|---|
| 1 | 字句、構文、節の順序、名前、別名、パスがあるか | E001〜E012 |
| 2 | 属し方：範囲の成果物がどれもちょうど一つのコンテキストに属するか | E101、E102、E103、W101、W103 |
| 3 | 成果物を読む：proto と、rulec・koyomi・dandori が口で答えるもの（4.1）と、Rust のクレートとその依存（Cargo が言うもの。7.7）。そのあとで、地図が名指す要素（`means`、対応の列挙）を、読んだ proto と rulec の事実で引く | E104、E105、E106、E107、W102、E103（範囲の外の import や参照）、E007・E011（要素） |
| 4 | パターンどうしの整合 | E301〜E313、W301 |
| 5 | 境界を越える参照 | E201〜E209 |
| 6 | 対応の網羅と同じ語 | E401〜E410、W401、W402 |

段 3 から後は、前の段にエラーがあっても、どの段も走らせる。読めない成果物があっても、読めた成果物の参照だけを確かめ、読めなかったものは E105（proto なら E106）として言う（読めない規則が一つあるだけで、地図の検査が全部止まると、直す順序が分からない）。ほかの言語が渡されていなければ（sakai のクレートのバイナリ）、その言語の成果物は読まずに、言語ごとに一つの E104 を言う（4.1）。パターンの誤りも後の段を止めない。5.3 の二つめの例のように、片側だけの共有カーネル（E307）と、それで許されなくなった参照（E201）は、一緒に言う。

通ったときは、一行で要約を言う。コンテキストと関係の数、成果物の数（どれもちょうど一つのコンテキストに属すること）、確かめた境界を越える参照の数と、それを何から読んだかを並べる。B の段階の地図には、英語の名前のもの（`tests/maps/basic/`）と日本語の名前のもの（`tests/maps/基本/`）があり、中身は同じである。英語の名前のものでは次のとおり。

```
$ sakai check tests/maps/basic/basic.ctx --root tests/maps/basic
tests/maps/basic/basic.ctx: ok — 3 contexts, 3 relationships; 9 artifacts, each in one context; 3 crossings checked (proto 3)
$ sakai check tests/maps/basic/basic.ctx --root tests/maps/basic --lang ja
tests/maps/basic/basic.ctx: ok — コンテキスト 3、関係 3。成果物 9 件は、どれも一つのコンテキストに属する。境界を越える参照 3 件を確かめた（proto 3）
```

日本語の名前のものでは次のとおり。

```
$ sakai check tests/maps/基本/基本.ctx --root tests/maps/基本
tests/maps/基本/基本.ctx: ok — 3 contexts, 3 relationships; 9 artifacts, each in one context; 3 crossings checked (proto 3)
$ sakai check tests/maps/基本/基本.ctx --root tests/maps/基本 --lang ja
tests/maps/基本/基本.ctx: ok — コンテキスト 3、関係 3。成果物 9 件は、どれも一つのコンテキストに属する。境界を越える参照 3 件を確かめた（proto 3）
```

関係は、上流と下流の関係を一つ（顧客／供給者は、両側の宣言で一つ）、共有カーネル、パートナーシップ、別々の道を、二つのコンテキストの組ごとに一つと数える。警告があっても、エラーが無ければこの行を出す。

11 章の例の地図では次のとおり。成果物の 79 件は、proto 4、規則 4、カレンダー 3、帳簿 1、ワークフロー 2 と、四つの言語のコード 65 である。例は規則とカレンダーとワークフローを含むので、すべての言語をつないだ `ritsu sakai` で走らせる。

```
$ ritsu sakai check examples/shop/shop.ctx
examples/shop/shop.ctx: ok — 5 contexts, 7 relationships; 79 artifacts, each in one context; 9 crossings checked (proto 1, rulec 2, koyomi 1, dandori 5)
$ ritsu sakai check examples/shop/shop.ctx --lang ja
examples/shop/shop.ctx: ok — コンテキスト 5、関係 7。成果物 79 件は、どれも一つのコンテキストに属する。境界を越える参照 9 件を確かめた（proto 1、rulec 2、koyomi 1、dandori 5）
$ ritsu sakai check examples/shop.ja/通販.ctx
examples/shop.ja/通販.ctx: ok — 5 contexts, 7 relationships; 79 artifacts, each in one context; 9 crossings checked (proto 1, rulec 2, koyomi 1, dandori 5)
```

境界を越える参照の 9 件は、proto の import が一つ（Ordering の `fulfillment.proto` から Inventory の `stock.proto` へ）、rulec が二つ（Billing の `billing_need.rule` の `import proto` が Ordering の `OrderStatus` へ、`shipment_fee.rule` の `shape` が Delivery の `CreateShipmentRequest` へ）、koyomi が一つ（Delivery の `ship_date.cal` が、Billing と Delivery の共有カーネルの `tokyo_business_days.cal` を `use calendar` で読む）、dandori が五つ（Ordering の `fulfillment.flow` が、Delivery の規則 `urgency` を `use rule … connect` で、Inventory の `stock.proto` を `use proto` で読み、その `StockService` の `Reserve` と `Release` を `connect` で呼び、Delivery の `arrange_delivery.flow` を子として走らせる）である。日本語の名前で書いた例では、同じ九つが、受注の `受注.flow`、請求の `請求の要否.rule` と `出荷の送料.rule`、配送の `出荷日.cal`、`配送の手配.flow` などの上にある。括弧の中は、参照のもとの言語ごとの数で、proto、rulec、koyomi、dandori、rust（Rust のクレートのマニフェスト。7.7）の順に並べ、一つも無い言語は書かない。

### 3.2 属し方

1.3 の決まりで、範囲の成果物ごとに属するコンテキストを決める。E101 は、属さないファイルを一つずつ言うと、初めて地図を書いたときに何百にもなる。そこで、属さないファイルしか含まないディレクトリは、いちばん上のディレクトリにまとめて一つの診断にする（`py/scripts/: この下のファイル 12 件は、どのコンテキストにも属しません`）。

公表された言語の proto、`generated dir`、`layer`、共有カーネルに並べたものも、属し方と食い違ってはいけない（E302、E313、E307）。

### 3.3 境界を越える参照

参照は、成果物から別の成果物（かその要素）への依存で、sakai が読めるのは次のものである。

| 参照のもと | 何を読むか | 参照の先 |
|---|---|---|
| proto | `import` の行（行番号つき）、フィールドとメソッドの型 | 別の proto のファイルと、使っている要素 |
| rulec の規則 | rulec の `References` の `import proto`（行と、proto の列挙）、`shape`（行と、proto のメッセージ）、`apply`（行と、規則）。列挙の値は rulec の `Rules` の事実（値ごとのワイヤでの名前）から | proto のファイルと、列挙と値、メッセージ、規則 |
| koyomi の dates とカレンダー | koyomi の `References` の `use calendar`（行と、カレンダー） | koyomi のカレンダー |
| dandori のワークフロー | dandori の `References` の `use rule`（呼び方の語を添えて）、`use proto`、`connect`（proto のサービスとメソッド）、`flow`（子の `.flow`）、`implements`（proto のサービス）。4.7 | 規則、proto のファイルとサービスとメソッド、ワークフロー |
| chobo の帳簿、geas の主張 | 参照を持たない（chobo の帳簿はほかの成果物を読まない。geas の主張はプログラムを外から叩く） | |
| Rust のクレート（`[package]` のある `Cargo.toml`） | Cargo が言う、パスで書いたほかのクレートへの依存（`[dependencies]` と `[build-dependencies]`。行は依存を書いたマニフェストの行）。7.7 | クレート（その `Cargo.toml`） |
| ほかの言語のコード | sakai は読まない。7 章の設定で各ツールが確かめる | |

ほかの言語が口で言う参照のうち、ここに無いもの（rulec の `import jsonschema` と JSON Schema の `shape`、rulec と koyomi の `source` の写し、dandori の `use openapi` と `use smithy`）は、境界を越える参照に数えない。JSON Schema、出典の写し、祝日の表、OpenAPI と Smithy の記述は、それを読む成果物の一部として扱い、属し方を問わない（1.3）からである。範囲の外にあっても何も言わない。

参照のもとと先が別のコンテキストに属するとき、境界を越える参照になる。そのとき、次のどれかでなければ診断を出す。

1. 先が、二つのあいだの共有カーネルに並べた成果物（の中）にある。
2. 先が、相手の公表された言語の要素で、関係がそれを許す（1.5 の表）。

先が公表された言語の要素かは、参照の種類で決まる。proto のファイルを指す参照（`import`、規則の `import proto` と `shape`、ワークフローの `use proto` と `connect`）は、そのファイルを並べた `published language` の塊の package のものである。規則を Connect のサービスとして呼ぶ参照（`use rule … connect`）は、その規則を並べた `published language rulec.…` の塊のものである。子の `.flow` は、それが相手の公開ホストサービスを `implements` で実装しているときに、そのサービスの proto のものになる（4.7）。Rust のクレートの依存は、その先のクレートを並べた `published language` の塊（`crate "…"`）のものである。規則そのものを使う参照（`use rule` の同梱、Lambda、ローカルと、規則の `apply`）とカレンダー（`use calendar`）は、どの公表された言語のものでもない。

診断は、関係が無ければ E201、相手の内側（公表された言語でない proto、規則そのもの、カレンダー）を参照していれば E202、関係はあるが `through` に無い package なら E203、腐敗防止層の `layer` の外からの参照なら E204、腐敗防止層の公表された言語に上流の型が出ていれば E205、別々の道の相手なら E206、相手の公開ホストサービスでないサービスを呼んでいれば E207、境界の向こうのワークフローを子として走らせていれば E209（4.7）。ワークフローの `implements` は境界を越える参照ではなく、実装するサービスが自分の公表された言語の公開ホストサービスであることを確かめる（E208。4.7）。

一つの参照に出す診断は一つで、次の順に決める。

1. 二つが別々の道なら E206。
2. 先が、両側が書いた共有カーネルの中にあれば通す。
3. 参照する側から相手への `upstream` も、両側が書いたパートナーシップも無ければ E201。相手の側に参照する側への `upstream` がある（向きが逆）ときと、片側だけのパートナーシップや共有カーネルがあるときは、そう注に書く。
4. 子の `.flow` が相手の公開ホストサービスを実装していなければ、パートナーシップなら通し、そうでなければ E209。
5. 先が相手の公表された言語に無ければ E202。
6. サービスを呼ぶ参照（`connect`、`use rule … connect`）で、そのサービスが先の package の `open host service` に無ければ E207。
7. パートナーシップなら通す（相手のどの公表された言語でもよい）。
8. 先の package が `through` に無ければ E203。
9. 腐敗防止層で、参照のもとが下流の公表された言語のファイル（proto か、公表された言語に並べた規則）なら E205（上流の公表された言語を参照すること自体を、上流の型を出すこととみなす）。`layer` があり、もとが層の外なら E204。
10. ほかは通す。

関係が成り立つのは、下流が書いた `upstream` は、それだけで成り立ち（P4。顧客が片側だけでも、E303 を出したうえで参照は通す）、パートナーシップと共有カーネルは、両側が書いたときだけである。片側だけのパートナーシップや共有カーネルは参照を許さず、E309 や E307 と一緒に、許されなくなった参照を言う。

proto のフィールドやメソッドが使うメッセージと、rulec の `shape` が読むメッセージは、そこからフィールドでたどれる型も全部参照したものとして数える（下流は、メッセージの中の列挙の値も受け取るからである）。rulec の口は `shape` のメッセージを名指すだけで、規則がそのどのフィールドを読むかは言わない（規則の入力がどこから読むかは rulec の構文の式で、それを読み解くと rulec の構文に依ることになる）。多めに数えるので、同じ語の検査（3.6）で、規則が実際には読まないフィールドの語までぶつかることがある。読むフィールドだけを数えることは 14 章に残した。

### 3.4 パターンどうしの整合

1.4〜1.7 に書いたもの（E301〜E313、W301）。多くは `.ctx` だけから決まる。E301 と E302 は proto と rulec の事実（`Rules`）を、E308 は共有カーネルの写しの中身を読む。

### 3.5 対応の網羅

1.7 のとおり。上流の列挙の値は proto から読み、rulec の規則が先なら、rulec の事実（`Rules`）の列挙（値ごとのワイヤでの名前）を読む。

### 3.6 同じ語

1.6 のとおり。越えてくる要素は、3.3 で読んだ参照のうち、要素まで分かるもの（proto が使う型とそこからたどれる型、rulec の `import proto` の列挙と値、`shape` のメッセージとたどれる型）から決める。コードの import と、dandori の参照（呼ぶメソッド、使う規則、子のワークフロー）は、越えていく型を言わないので対象にならない（14 章）。

### 3.7 確かめないこと

- 腐敗防止層のコードが、書いた対応どおりに読み替えているか。sakai が確かめるのは、対応が上流の列挙を覆っているかと、対応の先が下流の列挙にあるかまでである（rulec の規則が先のときは、rulec が規則の表を確かめる）。
- 実行時にしか見えない呼び出し：URL を文字列で持つ HTTP、メッセージのキュー、データベースの共有、リフレクションと動的な import。どれも成果物にも import にも現れない。
- 公表された言語から生成したコード（`generated dir`）が、本当にその proto から生成したものか。
- 語の定義の文の中身（P5）。
- Rust の `[dev-dependencies]`（テスト、例、ベンチマークだけが使い、クレートの中には入らない。ritsu では、受け取る側のクレートのテストが、出す側のクレートを本物のままつなぐために dev-dependency に持つ。ritsu の DESIGN 3.3）と、レジストリや git のクレートへの依存（どのコンテキストのものでもない）。7.7。
- Rust のクレートの中のファイルどうしの参照。クレートの依存はクレートの単位なので、クレートの `.rs` のファイルをマニフェストと別のコンテキストに属させても、その境界は確かめない（Rust では、どのモジュールも、クレートの依存の全部を使える）。

## 4. 一式とのつなぎ方

### 4.1 読むもの

sakai が一式の成果物から読むものは、どれも ritsu の口（ritsu の DESIGN 3.2）を通す。口を実装するのは出す側の言語のクレート（`rulec::ports::Engine` など）で、sakai はそれらのクレートを知らない。sakai が受け取るのは口のまとまり（`suite::Suite`）で、コマンドは `sakai::run::run(引数, 口, 標準出力, 標準エラー)` である。すべての口をつないで渡すのは ritsu の入口の `ritsu sakai` で（ritsu の DESIGN 8.6）、sakai のクレートのバイナリは何もつながない。

| 言語 | 口 | 読むもの | 使うところ |
|---|---|---|---|
| rulec | `Rules`（`facts`） | Connect のパス（package とサービス）、Connect の列挙（値の名前と、ワイヤでの名前と番号、取り込んだ proto のファイルと列挙の完全な名前）、入力と出力の名前 | 公表された言語の rulec の塊（E302、E301）、対応の先が規則の列挙のとき（E403、E405、E407）、`use rule … connect` が呼ぶサービス（E207）、doc の成果物の表と対応の表 |
| rulec | `Items`（索引で） | 入力、出力、列挙と値の名前 | 語の `means` の先（E007） |
| rulec | `References`（索引で） | `import proto`、`shape`、`apply`（3.3） | 境界を越える参照 |
| koyomi | `References`（索引で） | `use calendar`（3.3） | 境界を越える参照 |
| dandori | `References`（索引で） | `use rule`、`use proto`、`connect`、`flow`、`implements`（4.7） | 境界を越える参照、E207〜E209 |
| koyomi | `Dates`（`facts`） | 日付の名前と、使うカレンダーの名前とデータの範囲 | doc のコンテキストのページ（4.4） |
| chobo | `Books`（`facts`） | 勘定と振替の名前 | doc のコンテキストのページ（4.5） |
| geas | 読まない | | 属し方だけ（4.6） |
| proto | ritsu-proto でファイルを直接読む | `package`、`import`、メッセージ、フィールド、列挙と値、サービスとメソッド、サービスのオプション | 4.2 |

一つの実行で、同じファイルには一度だけ問う（規則を参照のもとと対応の先で二度問わない）。問う相手のファイルは、`References` にはルートとルートからのパスで、`Rules`、`Dates`、`Books` にはファイルのパスで渡す。

参照（`References`）と、規則が持つもの（rulec の `Items`）は、sakai が言語ごとに尋ねるのではなく、プロジェクトの索引（ritsu の DESIGN 6.4 の `Index`）で引く。索引は、各言語の答えをファイルごとに一度だけ尋ねて持つ。`ritsu sakai` と `ritsu check` では、索引は ritsu-project が一つ作り、yuen や言語をまたぐ検査と分け合う。語の `means` が規則の入力、出力、列挙、列挙の値を名指すときは、rulec がその規則に答えた（`Rules` の事実がある）うえで、索引で引く（規則が自分のファイルに書いたものを引く。`apply` で展開したものは、その規則のファイルで名指す）。無ければ、これまでどおり sakai の E007 と文で言う。引き方を索引に替えただけで、sakai の出力は変わらない（ritsu の PLAN の E.1）。

rulec には、検査を通る規則にだけ答える事実（`Rules`）を先に問い、答えた規則にだけ参照を問う。koyomi と dandori は、構文を読めるファイルに参照を答える（カレンダーとワークフローがそれぞれの検査を通るかは、それぞれの `check` が言う。参照を読むのに要るのは構文だけである）。答えないとき（規則が rulec の検査を通らない、構文を読めない、ファイルが無い）は E105 で、その言語が言うこと（`Said`。コードと行と文）を注に五つまで並べる。その成果物の参照は確かめない。11 章の例で、受注が注文の状態に値を足したとき（`order.proto` に `ORDER_STATUS_RETURNED = 5;`）は、次のようになる。

```
エラー[E105]: billing/rules/請求の要否.rule:5:1: billing/rules/請求の要否.rule が rulec の検査を通らないか、読めません
     5 | enum 注文の状態(order_status) = 受付(received) | 支払済(paid) | 出荷済(shipped) | 受注で取消(cancelled)
  = rulec の診断: [E032] billing/rules/請求の要否.rule:5: 列挙 注文の状態 が ../../proto/shop/ordering/v1/order.proto の OrderStatus と一致していません
  = そのファイルを、rulec の検査を通るように直してください。検査を通らないファイルや読めないファイルからは参照を読み取れないので、sakai はその参照を確かめられません。
```

ほかの言語が渡されていないとき（sakai のクレートのバイナリ）は、地図が rulec、koyomi、dandori の成果物を含めば、言語ごとに一つの E104 を、その言語の最初の成果物を持つ `owns` の行に出す。注に、同じコマンドを `ritsu sakai` で走らせる形を書き、exit 2 で終わる（走らせ方の問題で、地図の誤りではないため。ritsu の段階 E の前は、ほかの誤りと同じく exit 1 だった。12.3）。

```
$ sakai check examples/shop/shop.ctx
error[E104]: examples/shop/contexts/billing.ctx:6:7: This sakai cannot read rulec artifacts (4 of them, the first examples/shop/billing/rules/billing_need.rule)
     6 |   dir "../billing", "../calendars"
  = The binary of sakai's own crate holds no other language; run it with every language joined, through ritsu: `ritsu sakai check examples/shop/shop.ctx`.
error[E104]: examples/shop/contexts/billing.ctx:6:7: This sakai cannot read koyomi artifacts (3 of them, the first examples/shop/billing/payment_terms.cal)
     6 |   dir "../billing", "../calendars"
  = The binary of sakai's own crate holds no other language; run it with every language joined, through ritsu: `ritsu sakai check examples/shop/shop.ctx`.
error[E104]: examples/shop/contexts/delivery.ctx:6:7: This sakai cannot read dandori artifacts (2 of them, the first examples/shop/delivery/arrange_delivery.flow)
     6 |   dir "../delivery", "../proto/shop/delivery"
  = The binary of sakai's own crate holds no other language; run it with every language joined, through ritsu: `ritsu sakai check examples/shop/shop.ctx`.
```

**決定**：E104 は、ほかの言語がつながっていないことを言うコードにする（前は「ツールが無い」）。ritsu の決まりで、言語のクレートはほかの言語のクレートに依存しない（ritsu の DESIGN 3.1）ので、sakai のクレートのバイナリは一式の言語を持てない。そのバイナリで例を確かめるとき、規則やワークフローを属し方でだけ扱って通すこともできるが、それは確かめていない参照を黙って通すことになる（P7）。言語ごとに一つにしたのは、成果物ごとに言うと例でも九つになり、言うことは同じだからである。`check` のほかに `api`、`build`、`export` も、地図の検査を通らなければ出さないので、同じ E104 で止まる。

**捨てたもの**：

- 一式のツールを子プロセスで呼び、`rulec api` と `koyomi api` と `chobo api` の JSON を読む形（C の段階の計画）。一式を ritsu にまとめたので、口で読む。ツールの探し方（`SAKAI_RULEC` などの環境変数と PATH）、版の確かめ（rulec 0.22.0 より古ければ E104）、時間の上限、標準エラーの最初の行を E105 の注に添えることは、どれも要らなくなった。
- sakai のクレートのバイナリで、ほかの言語の成果物を属し方でだけ扱って通すこと。上の理由。

### 4.2 proto

proto は sakai が直接読む（P2）。読み手は自分で書いた（依存を足さない。rulec と dandori もそうしていた）。ritsu の C.9 で、この読み手を元に ritsu の一つの読み手（ritsu-proto）を作り、sakai はそれで読むようになった（12.1）。読むのは `syntax`、`package`、`import`（`public` と `weak` も）、メッセージ（入れ子）、フィールド（型、番号、`json_name`、`optional`、`repeated`、`map`、`oneof`）、列挙と値、サービスとメソッド（ストリームかどうか）、サービスとメソッドのオプションの名前と値（dandori の `(dandori.v1.workflow)` を doc に出すため）である。拡張（`extend`）と、ほかのオプションは読み飛ばす。

`import` は、地図の `proto root` のディレクトリから順に探し、無ければ、dandori と同じ決まり（ファイルのディレクトリが自分の package の形で終わっていればその上から、次にファイルのディレクトリから）で探す。Google の well-known types、`buf/validate/validate.proto`、`dandori/v1/options.proto` は、ファイルが無くても知っているものとして扱う（中は読まない）。それ以外で見つからない import は W102 で、そのファイルの型は分からないものとして、参照の検査からも外す（buf の依存のように、リポジトリの外にある契約は地図の範囲の外である）。

11 章の例の `shipment.proto` は、rulec の corpus の契約を写したもので、`buf/validate/validate.proto` を import する。buf 1.54.0 は、BSR の依存を書かないとこのファイルを組めない（`import "buf/validate/validate.proto": file does not exist`）。rulec 0.22.1 はこのファイルを読み、規則の範囲を Protovalidate の注釈と突き合わせる（注釈を外すと E122 になった）。sakai も、ファイルが無くても読める。

### 4.3 rulec

規則は、属し方（ファイル）、公表された言語（`published language rulec.<別名>.v<版>` の塊）、参照のもと（`import proto`、`shape`、`apply`）、対応の先（1.7）、語の `means` の先（規則の入力、出力、列挙）として現れる。rulec が規則について答えるのは、規則が rulec の検査を通るときだけである。通らない規則は E105 で、その規則の参照も対応も確かめない（4.1）。

### 4.4 koyomi

dates のファイルとカレンダーは、属し方と、参照のもと（`use calendar`）として現れる。カレンダーは公表された言語にできない（1.4）ので、カレンダーを境界の向こうから読めるのは、共有カーネルに並べたときだけである。11 章の例では、請求の支払条件と配送の出荷日が同じ `東京の営業日.cal` を読み、二つのコンテキストがそれを共有カーネルに並べる。

doc のコンテキストのページには、koyomi の口（`Dates`）の事実から、dates のファイルの日付の名前と、使うカレンダーの名前とデータの範囲（例では `東京の営業日`、1955-01-01..2027-12-31）を並べる。カレンダーのファイルそのものには `Dates` は答えない（日付のファイルではないので）。そこで、カレンダーのファイルには、地図の中でそれを `use calendar` で読む dates のファイルが聞いた、カレンダーの名前とデータの範囲を並べる。口は D の段階（ritsu の F.2）で `Suite` に足し、`ritsu sakai` は ritsu-project が koyomi のものを渡す。`check` は `Dates` に問わない。

### 4.5 chobo

帳簿はほかの成果物を読まないので、参照のもとにならない。帳簿をほかのコンテキストから使うのは、帳簿の後ろに立つサービス（proto）を通すときで、そのときの参照はサービスへの参照になる。chobo の口（`Books`）の事実から、勘定と振替の名前を doc のコンテキストのページに並べる（10 章）。読むところ（`suite::book_names`）は ritsu の D.8 で作り、例の `在庫の引当.book` から勘定 `在庫`、`仕入先`、`客` と振替 `入荷`、`引当`、`返品` を読むことを `tests/examples.rs` が確かめる。`check` は chobo に問わない。

### 4.6 geas

geas の主張は、プログラムを外から叩き、成果物の名前を持たない（geas の DESIGN の 0 章）。属し方だけを決め、中は読まない。主張の一覧は geas の口（`Claims`）が渡すが、sakai は使わない。主張を名指すこと（2 章の `geas "…" claim "…"`）が要るのは yuen の側である。

### 4.7 dandori

dandori のワークフローは、境界を越える参照をいちばん多く持つ成果物である。sakai は、dandori が口（`References`）で言うものを読む。

| dandori が言う参照 | 先 | sakai の扱い |
|---|---|---|
| `use rule`（呼び方の語を添えて：`use rule`、`use rule … connect`、`use rule … lambda`、`use rule … local`） | 規則 | `connect` があれば、規則の Connect のサービスを呼ぶ参照で、先の規則が相手の公表された言語（`published language rulec.…`）に入っていること（E202）と、そのサービスが `open host service` に並ぶこと（E207）。`connect` が無ければ（同梱、Lambda、ローカル）、規則そのものを使う参照で、共有カーネルの中でなければ E202 |
| `use proto` | proto のファイル | proto の import と同じく、相手の公表された言語の proto であること |
| `connect` | proto のサービスとメソッド | サービスを呼ぶ参照。サービスが相手の `open host service` に並ぶこと（E207） |
| `flow` | 子の `.flow` | 下の決定（E209） |
| `implements` | proto のサービス | 境界を越える参照ではない。そのワークフローを持つコンテキストの公表された言語の `open host service` に並ぶこと（E208） |
| `use openapi`、`use smithy` | 記述のファイル | 記述は、それを読むワークフローの一部（1.3）なので、参照として数えない |

11 章の例では、受注の `受注.flow` から五つの参照が境界を越え（3.1）、`implements 店.FulfillmentService` は受注の公表された言語 `shop.ordering.v1` の公開ホストサービスなので通る。

**決定**：子の `.flow` を境界の向こうから走らせてよいのは、次のどれかのときだけにする。ほかは E209 で、相手が公表したサービスを `connect` で呼ぶように言う。

1. 二つがパートナーシップである。
2. 子の `.flow` が、二つの共有カーネルに並んでいる。
3. 子の `.flow` が、相手の公表された言語の `open host service` に並ぶサービスを `implements` で実装している。このとき参照は、そのサービスの proto への参照として、ほかの参照と同じ決まり（`through`、腐敗防止層）で確かめる。

**理由**：子のワークフローを走らせると、親は子の入力と出力の形と、失敗の仕方に依る。それは相手の内側のモデルで、公表された言語ではない。子が公開ホストサービスを実装していれば、その形は相手が公表した proto のサービスで決まっているので、サービスを呼ぶのと同じに扱える。パートナーシップは、二つのチームが互いのものを合わせて変えると決めた関係なので、内側のワークフローを走らせてもよい。共有カーネルは、二つで一緒に持つものなので、どちらからでも使える。

日本語の値の列挙を受け取るワークフローは proto のサービスを実装できない（proto の値は ASCII の識別子に限られる。dandori の DESIGN 1.14）ので、そういうワークフローを境界の向こうから子として走らせるには、パートナーシップか共有カーネルが要る。11 章の例の `配送の手配.flow` がそうで、便（`通常便 | 翌日便`）を受け取る。例の受注と配送はパートナーシップなので通り、関係を顧客と供給者に替えると E209 になる（5.3）。

**捨てたもの**：

- 子の `.flow` の入力と出力を、公表された言語に入れる書き方（`published language` の塊に `dandori "…"` を並べる）。A の段階で挙げたもう一つの形である。公表された言語を proto の package と規則の Connect のサービスに限る決まり（1.4）を崩し、`.flow` の入力と出力の互換を確かめる仕組み（proto なら buf の `breaking`）も無い。
- 子の `.flow` を、関係があればいつも通すこと。順応者や腐敗防止層の下流が、上流の内側のワークフローに依ることになる。
- 子の `.flow` を、いつも E202（相手の内側）にすること。パートナーシップの二つのチームが互いのワークフローを組み合わせる形を書けない。

**これまでの形**：一式を ritsu にまとめる前、dandori は参照を JSON で出さず（`dandori check --format json` は診断だけを出し、`api` が無かった）、sakai は `.flow` を属し方でだけ扱い、確かめていないことを N101 で言うつもりだった。A の段階では、dandori に `dandori api` を足す形（選択肢 A）を勧め、sakai が `.flow` を読む形（C）と、dandori の doc や生成したコードを読み解く形（D）を、P2 に反するので取らなかった。ritsu で dandori が口（`References`）に答えるようになり（ritsu の D.6）、選択肢 A が JSON ではなく口の形で入った。N101 は退かせた（5.2）。A の段階で挙げた四つの検査は、E202（規則の同梱）、E207、E208、E209 として入れた。

## 5. 診断

### 5.1 形

koyomi と chobo にそろえる。

```
error[E201]: <ファイル>:<行>:<列>: <一行の見出し>
  <行> | <原文の行>
  = <注>
  = The line, fixed: <.ctx にそのまま貼れる行>
  involved:
      <コンテキスト>  <ファイル:行、または名指し>  <それが何か>
```

日本語では、重さの語が `エラー`、`警告`、`備考`（GCC の日本語の訳語と同じ）、直した行が `= 直した行:`、最後の見出しが `関わるもの:` になる。

一行目は地図の言葉で完結させる（どのコンテキストの、どの成果物が、どのコンテキストの何を）。「関わるもの」には、診断の根拠を一行ずつ並べる。参照のもとの成果物とその行、参照の先の成果物、許すはずだった関係の `.ctx` の行、語とそれが指す要素である。ファイルの行が分かるものは `<パス>:<行>`、要素は 2 章の名指しで書き、コンテキストに属するものは、そのコンテキストの名前を頭に置く。ファイルの場所は走らせたディレクトリから、名指しはルートからの相対で書く（2.4）。直せる行が一つに決まるときは、書き換え後の `.ctx` の行を「直した行」に書く。

位置は、参照のもとの成果物の行にする（proto の import の行と列。ほかの言語の参照は、その言語が口で言う行で、列は 1）。パターンと対応と語の診断は、`.ctx` の行を位置にする。ほかの言語が渡されていないこと（E104）は、その言語の最初の成果物を持つ `owns` の行を位置にする。

文面は英語が既定で、`--lang ja`、`SAKAI_LANG=ja`、ritsu のどの言語も読む `RITSU_LANG=ja` のどれかで日本語にする（この順に読む）。システムのロケールは見ない。日本語と英語は `tr!` で隣に書き、文は描くときに言語を渡す（koyomi と同じ。テストが英語と日本語の golden を同じプロセスで並行して描けるように）。日本語の文で、ASCII の名前と日本語のあいだには空白を入れる（koyomi と同じ）。コンテキストと語の名前は「」で囲む。

`--format json` は、地図ごとに一行で `{"root", "file", "ok", "summary", "diagnostics": [{"code", "severity", "file", "line", "col", "message", "notes", "references", "fix"}]}` を出す。`root` は走らせたディレクトリから見たルート、`file` はルートからの相対である（2.4）。`references` は「関わるもの」の並びで、要素ごとに `{"context", "name", "file", "line", "what", "via"}` を持つ。`name` は 2 章の JSON の形の名指し（ファイルの行なら null。パスはルートからの相対）、`file` と `line` はファイルの行（名指しなら null。パスはルートからの相対。2.4）、`via` は参照の仕方（proto の import なら `proto import`、ほかの言語の参照なら、その言語が口で言う語。`import proto`、`use calendar`、`use rule … connect` など）である。`fix` は `.ctx` にそのまま貼れる書き換え後の行（無ければ null）。`line` と `col` は分からなければ null。キーは `--lang` に依らず英語。

`severity` は `error`、`warning`、`note` の三つ。exit code は 0（エラーなし。警告と note はあってよい）、1（エラーあり）、2（引数の誤り、読めないファイル、sakai 自身の不具合）。

### 5.2 台帳

番号と意味はこの表で決め、`src/codes.rs` の台帳と一致させる（`tests/codes.rs` が表と台帳を突き合わせる）。各コードは、いつ出るか、どう直すか、最小の再現を持ち、`sakai explain <コード>` が引く。再現は複数のファイルになるので、台帳は、二つのコンテキスト（甲と乙）の検査を通る地図を土台に持ち、各コードはその上に置き換えるファイルを持つ（rulec の DESIGN 15.59 が、隣に置くファイルを台帳に持たせたのと同じ考え）。テストは全コードの再現を一時ディレクトリに書き、そこで走らせて、そのコードが出ることを確かめる。走らせるのはたいてい `check .` で、`build` が出す E501 と E502 は、再現に `sakai build …` のコマンドを持つ（`sakai explain` もそのコマンドを言う）。E107 の再現は、土台が Rust のコードを持たないので、Rust のコードを持つ一つのコンテキストの地図を自分で持つ。ほかの言語の成果物を含む再現（E105、E207、E208、E209、E405）は `ritsu sakai check .` で走らせ（`sakai explain` もそう言う）、E104 の再現は、何もつながない sakai のクレートのバイナリの `sakai check .` で走らせる。退いたコード（N101）は、台帳に残して `sakai explain` で引けるようにし、退いた理由と版を書く。番号はほかのものに使い回さない（ritsu の DESIGN 7.10）。

| コード | いつ出るか |
|---|---|
| E001 | 読めない字句（知らない文字、閉じていない文字列） |
| E002 | この位置に書けない語（役割の無い `upstream` など） |
| E003 | ファイルが `map` か `context` の行で始まらない |
| E004 | 節の順序や数が違う（同じ節が二度、map に `owns`、context に `covers`、省けない節が無い） |
| E005 | 字下げが合わない（タブ、揃っていない） |
| E006 | 同じ名前を二度宣言している（コンテキスト、別名、語、`published language` の package、同じ相手への同じ関係） |
| E007 | 宣言されていない名前（コンテキスト、語、package、要素。名指した proto にその要素が無いときも）。短い書き方の名前が二つの package に当たるときも |
| E008 | 地図かコンテキストに ASCII の別名が無い、または形が違う |
| E009 | 書いたパスが無い（`use context`、`owns`、`proto`、`crate`、`generated dir`、`layer`、`covers`、`code`） |
| E010 | `use context` の先が context のファイルでない、同じファイルを二度読む |
| E011 | 成果物の名指しの形が違う（ツールの語と拡張子が合わない、`dir` や `crate` の先がファイル、知らないツールや種類の語、親のすぐあとでない子、`file` の種類、列挙でない対応の先） |
| E012 | 絶対パス、ルートの外に出るパス、空のパス（2.4） |
| E101 | どのコンテキストにも属さない成果物 |
| E102 | 二つのコンテキストが同じ深さで持つ成果物 |
| E103 | 範囲の外のものを、持っている、または参照している |
| E104 | 地図が含む成果物の言語がつながっていない（sakai のクレートのバイナリ。`ritsu sakai` で走らせる。exit 2） |
| E105 | 成果物が、その言語の検査を通らないか、読めない（その言語の診断を注に添える） |
| E106 | proto が読めない |
| E107 | 地図が `code rust` を書いていて、その場所のクレートを Cargo から読めない（`Cargo.toml` が無い、`cargo metadata` が失敗する。7.7） |
| W101 | `owns` の項が成果物を一つも含まない |
| W102 | 見つからない proto の import（範囲の外のものとして扱う） |
| W103 | どの地図にも読まれない context のファイル（`check` にディレクトリを渡したときだけ） |
| N101 | 退いたコード（ritsu 0.23.0）。dandori の参照を確かめていない、と言うためのものだった |
| E201 | 関係の無いコンテキストへの参照（関係が逆向き、つまり参照の先が下流のときも） |
| E202 | 相手の内側への参照（公表された言語でない proto、公表された言語に無い規則、規則そのもの（同梱、Lambda、ローカル、`apply`）、カレンダー） |
| E203 | `through` に無い package を通る参照 |
| E204 | 腐敗防止層の `layer` の外からの、上流の公表された言語への参照 |
| E205 | 腐敗防止層の下流の公表された言語に、上流の型が出ている |
| E206 | 別々の道の相手への参照 |
| E207 | ワークフローが、境界の向こうの、相手の公開ホストサービスでないサービスを呼んでいる（`connect`、`use rule … connect`） |
| E208 | ワークフローが `implements` で実装するサービスが、自分の公表された言語の公開ホストサービスでない |
| E209 | ワークフローが、境界の向こうのワークフローを子として走らせている（パートナーでなく、共有カーネルになく、子が相手の公開ホストサービスを実装していない） |
| E301 | 公開ホストサービスのサービスが、公表された言語に無い（Rust のクレートの公表された言語に `open host service` を書いたときも） |
| E302 | 公表された言語の proto や規則やクレートや生成したコードの置き場所が、そのコンテキストのものでない、package やクレートの名前が見出しと違う、`crate` の先がワークスペースのクレートでない、地図に `code rust` が無いのにクレートを公表している |
| E303 | 顧客／供給者が片側だけ |
| E304 | 順応者に対応か `layer` がある |
| E305 | 腐敗防止層でないのに、対応か `layer` がある |
| E306 | 一緒に書けない役割（順応者と顧客、順応者と腐敗防止層） |
| E307 | 共有カーネルが片側だけ、または両側の並びが違う |
| E308 | 共有カーネルに並べた二つの写しの中身が違う |
| E309 | パートナーシップが片側だけ |
| E310 | 同じ二つのあいだの関係が両立しない（別々の道とほかの関係） |
| E311 | 自分自身との関係 |
| E312 | 上流が `through` の package を公表していない |
| E313 | 腐敗防止層の `layer` が、下流のものでない |
| W301 | 二つのコンテキストが互いに上流（循環） |
| E401 | 対応に、上流の列挙の値が抜けている（抜けた値を全部名指す） |
| E402 | 対応に、上流の列挙に無い値がある |
| E403 | 対応の先の値が、下流の列挙に無い |
| E404 | 腐敗防止層なのに、下流の成果物が参照している上流の列挙に対応が無い |
| E405 | rulec の規則の取り込みと、`.ctx` に書いた対応が食い違う |
| E406 | 同じ名前で違う意味の語が、対応の無いまま境界を越える |
| E407 | 違う意味の語を、同じ名前の値や語に読み替えている |
| E408 | `means` の要素が、そのコンテキストの公表された言語に無い |
| E409 | `term` の対応の語が、どちらかの用語集に無い |
| E410 | `as` で取り入れる語が相手の用語集に無い、または相手との関係が無い |
| W401 | 境界を越えない語 |
| W402 | 値が無いことを表す 0 番の値に対応を書いている |
| E501 | ツールの設定に書けない（言語のモジュールにならないディレクトリの名前、`code` の無い言語へのビルド、コードを持つコンテキストが無い） |
| E502 | `build --check`：書いてある設定が、いまの地図から書くものと違う |

番台は、構文と名前（E0xx）、属し方と成果物を読むこと（E1xx）、境界を越える参照（E2xx）、パターン（E3xx）、語と対応（E4xx）、出力（E5xx）で分けた。増やすときは番台の末尾に足し、番台をまたがない。

### 5.3 診断の例

B の段階の地図（`tests/maps/基本/`）と 11 章の例を一か所だけ変えた変異（`tests/mutants/`）に、sakai が出したもの。英語と日本語の全文は `tests/golden/` にある。ほかの言語の参照を読む例（E105 から後の五つ）は、すべての言語をつないだ `ritsu sakai check` で出したものである。

受注が注文の状態に値を足したとき（`order.proto` に `ORDER_STATUS_RETURNED = 5;`）：

```
エラー[E401]: ctx/請求.ctx:17:3: 「請求」の腐敗防止層の対応に、「受注」の列挙 shop.ordering.v1.OrderStatus の値 ORDER_STATUS_RETURNED がありません
    17 |   enum OrderStatus -> enum BillingStatus
  = ORDER_STATUS_RETURNED は proto/shop/ordering/v1/order.proto:16 の値です。
  = 上流の列挙の値ごとに、下流の値か refuse（断る）を書いてください。上流が値を足すと、その値をどう扱うかを決めるまで、検査は通りません。
  = 直した行: ORDER_STATUS_RETURNED -> refuse "…"
  関わるもの:
      請求  ctx/請求.ctx:14                                              upstream 受注 anticorruption layer
      受注  proto "proto/shop/ordering/v1/order.proto" enum OrderStatus  値は 5 個あり、そのうち 1 個に対応がありません
```

在庫の側の `shared kernel with 受注` を消したとき（在庫の `stock.proto` が、共有カーネルの `money.proto` を import している）：

```
エラー[E307]: ctx/受注.ctx:21:1: 共有カーネルが「受注」の側にしか書かれていません
    21 | shared kernel with 在庫
  = 共有カーネルは二つのチームが一緒に持つものなので、「在庫」のファイルにも `shared kernel with 受注` を書き、同じものを並べてください。
  関わるもの:
      受注  ctx/受注.ctx:21  shared kernel with 在庫
エラー[E201]: proto/warehouse/v1/stock.proto:5:1: 「在庫」の proto/warehouse/v1/stock.proto が、関係の無い「受注」の proto/shop/common/v1/money.proto を import しています
     5 | import "shop/common/v1/money.proto";
  = 「受注」が「在庫」の下流で、参照の向きが逆です。上流と下流の関係が許すのは、下流から上流への参照です。
  = 二つのあいだの共有カーネルは、「受注」の側にしか書かれていません（E307 も出ています）。
  = 境界を越えて参照するには、上流と下流（下流が `upstream` を書く）、パートナーシップ、共有カーネルのどれかの関係が要ります。
  関わるもの:
      在庫  proto/warehouse/v1/stock.proto:5          import "shop/common/v1/money.proto"
      受注  proto "proto/shop/common/v1/money.proto"  「受注」の内側のファイル
```

受注が在庫に順応しているのに、在庫と違う意味の「引当」を持つとき：

```
エラー[E406]: ctx/受注.ctx:17:3: 「在庫」の「引当」が、「受注」の違う意味の「引当」とぶつかったまま、境界を越えています
    17 |   引当 "客の注文の一行に、届ける日を割り当てること"
  = 「在庫」の「引当」は「注文の一行のために、棚の在庫を出荷か取消まで押さえておくこと」で、proto "proto/warehouse/v1/stock.proto" message ReserveResponse を指します。
  = 「受注」の「引当」は「客の注文の一行に、届ける日を割り当てること」です。
  = 「受注」は「在庫」の順応者なので、対応を書いて読み替えることはできません。
  = 「受注」の語の名前を変えてください。同じ意味なら `引当 as 在庫.引当` にし、読み替えるなら `upstream 在庫` を anticorruption layer にして `term 引当 -> <「受注」の語>` を書いてください。
  関わるもの:
      受注  proto/shop/ordering/v1/fulfillment_lite.proto:5                 import "warehouse/v1/stock.proto"
      在庫  proto "proto/warehouse/v1/stock.proto" message ReserveResponse  「在庫」の語「引当」が指すもの
      在庫  ctx/在庫.ctx:13                                                 引当 "注文の一行のために、棚の在庫を出荷か取消まで押さえておくこと"
      受注  ctx/受注.ctx:17                                                 引当 "客の注文の一行に、届ける日を割り当てること"
```

例で、在庫が梱包の状態に値を足したとき（`stock.proto` に `PACKING_STATUS_DAMAGED = 4;`）。配送の腐敗防止層は、新しい値をどう扱うかを決めるまで通らない：

```
エラー[E401]: contexts/配送.ctx:33:3: 「配送」の腐敗防止層の対応に、「在庫」の列挙 warehouse.v1.PackingStatus の値 PACKING_STATUS_DAMAGED がありません
    33 |   enum PackingStatus -> 出荷の可否
  = PACKING_STATUS_DAMAGED は proto/warehouse/v1/stock.proto:43 の値です。
  = 上流の列挙の値ごとに、下流の値か refuse（断る）を書いてください。上流が値を足すと、その値をどう扱うかを決めるまで、検査は通りません。
  = 直した行: PACKING_STATUS_DAMAGED -> refuse "…"
  関わるもの:
      配送  contexts/配送.ctx:29                                       upstream 在庫 anticorruption layer
      在庫  proto "proto/warehouse/v1/stock.proto" enum PackingStatus  値は 4 個あり、そのうち 1 個に対応がありません
```

例で、受注の用語集に、在庫と違う意味の「引当」を足したとき（受注は在庫の順応者で、受注の生成したコードが在庫の `ReserveResponse` をそのまま使う）：

```
エラー[E406]: contexts/受注.ctx:22:3: 「在庫」の「引当」が、「受注」の違う意味の「引当」とぶつかったまま、境界を越えています
    22 |   引当 "客の注文の一行に、届ける日を割り当てること"
  = 「在庫」の「引当」は「注文の一行のために、棚の在庫を出荷か取消まで押さえておくこと」で、proto "proto/warehouse/v1/stock.proto" message ReserveResponse を指します。
  = 「受注」の「引当」は「客の注文の一行に、届ける日を割り当てること」です。
  = 「受注」は「在庫」の順応者なので、対応を書いて読み替えることはできません。
  = 「受注」の語の名前を変えてください。同じ意味なら `引当 as 在庫.引当` にし、読み替えるなら `upstream 在庫` を anticorruption layer にして `term 引当 -> <「受注」の語>` を書いてください。
  関わるもの:
      受注  proto/shop/ordering/v1/fulfillment.proto:15                     import "warehouse/v1/stock.proto"
      在庫  proto "proto/warehouse/v1/stock.proto" message ReserveResponse  「在庫」の語「引当」が指すもの
      在庫  contexts/在庫.ctx:19                                            引当 "注文の一行のために、棚の在庫を出荷か取消まで押さえておくこと"
      受注  contexts/受注.ctx:22                                            引当 "客の注文の一行に、届ける日を割り当てること"
```

例で、受注が注文の状態に値を足したとき（`order.proto` に `ORDER_STATUS_RETURNED = 5;`）。請求の規則が取り込む列挙が proto と合わなくなり、rulec がその規則について答えない（4.1）：

```
エラー[E105]: billing/rules/請求の要否.rule:5:1: billing/rules/請求の要否.rule が rulec の検査を通らないか、読めません
     5 | enum 注文の状態(order_status) = 受付(received) | 支払済(paid) | 出荷済(shipped) | 受注で取消(cancelled)
  = rulec の診断: [E032] billing/rules/請求の要否.rule:5: 列挙 注文の状態 が ../../proto/shop/ordering/v1/order.proto の OrderStatus と一致していません
  = そのファイルを、rulec の検査を通るように直してください。検査を通らないファイルや読めないファイルからは参照を読み取れないので、sakai はその参照を確かめられません。
```

例で、請求の側の `shared kernel with 配送` を消したとき（配送の `出荷日.cal` が、共有カーネルに並べた請求のカレンダー `東京の営業日.cal` を読む）。カレンダーは公表された言語にできないので、共有カーネルが崩れると、その参照を許すものが無くなる：

```
エラー[E307]: contexts/配送.ctx:40:1: 共有カーネルが「配送」の側にしか書かれていません
    40 | shared kernel with 請求
  = 共有カーネルは二つのチームが一緒に持つものなので、「請求」のファイルにも `shared kernel with 配送` を書き、同じものを並べてください。
  関わるもの:
      配送  contexts/配送.ctx:40  shared kernel with 請求
エラー[E201]: delivery/出荷日.cal:3:1: 「配送」の delivery/出荷日.cal が、関係の無い「請求」の calendars/東京の営業日.cal を参照しています（use calendar）
     3 | use calendar "../calendars/東京の営業日.cal"
  = 「請求」が「配送」の下流で、参照の向きが逆です。上流と下流の関係が許すのは、下流から上流への参照です。
  = 二つのあいだの共有カーネルは、「配送」の側にしか書かれていません（E307 も出ています）。
  = 境界を越えて参照するには、上流と下流（下流が `upstream` を書く）、パートナーシップ、共有カーネルのどれかの関係が要ります。
  関わるもの:
      配送  delivery/出荷日.cal:3                use calendar "../calendars/東京の営業日.cal"
      請求  koyomi "calendars/東京の営業日.cal"  「請求」の内側のもの
```

例で、受注のワークフローが配送の規則を、Connect で呼ばずに同梱して使うとき（`受注.flow` の `use rule` の下の `connect` を消す）：

```
エラー[E202]: ordering/受注.flow:6:1: 「受注」の ordering/受注.flow が、「配送」の内側の delivery/rules/出荷の急ぎ.rule を参照しています（use rule）
     6 | use rule 急ぎ from "../delivery/rules/出荷の急ぎ.rule"
  = 規則を同梱するか Lambda で呼ぶと、規則そのもの（「配送」の内側）を使います。境界の向こうの規則は、「配送」にその規則を公表された言語（`published language rulec.…`）に入れてもらい、`use rule … connect` で、その Connect のサービスとして呼んでください。
  関わるもの:
      受注  ordering/受注.flow:6                    use rule "../delivery/rules/出荷の急ぎ.rule"
      配送  rulec "delivery/rules/出荷の急ぎ.rule"  「配送」の内側のもの
```

例で、在庫が `StockService` を公開ホストサービスから外したとき（受注のワークフローは、それを `connect` で呼ぶ）。二つ出るうちの一つめ：

```
エラー[E207]: ordering/受注.flow:47:1: 「受注」の ordering/受注.flow が、「在庫」の公開ホストサービスでない StockService を呼んでいます
    47 |   connect warehouse "StockService/Reserve"
  = 公表された言語 warehouse.v1 の公開ホストサービスは PackingService です。
  = 境界の向こうで呼べるサービスは、相手が `open host service` に並べたものだけです。
  関わるもの:
      受注  ordering/受注.flow:47                                                       connect StockService/Reserve
      在庫  proto "proto/warehouse/v1/stock.proto" service StockService method Reserve  公表された言語 warehouse.v1 のもの
```

例で、受注と配送のパートナーシップを、受注が配送の顧客になる関係に替えたとき（受注のワークフローは、配送のワークフローを子として走らせる。4.7）：

```
エラー[E209]: ordering/受注.flow:59:1: 「受注」の ordering/受注.flow が、「配送」のワークフロー delivery/配送の手配.flow を子として走らせています
    59 |   flow "../delivery/配送の手配.flow"
  = 境界の向こうのワークフローを子として走らせてよいのは、二つがパートナーシップのとき、子のフローが二つの共有カーネルにあるとき、子のフローが「配送」の公開ホストサービスを `implements` で実装しているときです。そうでなければ、「配送」が公表したサービスを `connect` で呼んでください。
  関わるもの:
      受注  ordering/受注.flow:59               flow "../delivery/配送の手配.flow"
      配送  dandori "delivery/配送の手配.flow"  「配送」の内側のもの
```

## 6. コマンド

| コマンド | すること |
|---|---|
| `sakai check <map.ctx \| dir>... [--format json] [--root <dir>]` | 検査（3 章）。ディレクトリを渡すと、その下の map のファイルを全部（パスの順に）。どの地図にも読まれない context のファイルがあれば、ディレクトリを渡したときだけ W103 を出す |
| `sakai build <map.ctx> --target import-linter\|dependency-cruiser\|archunit\|go-arch-lint [--out <dir>] [--check] [--root <dir>]` | コードの import の検査の設定を書く（7 章）。検査を通らない地図からは書かない（診断を出して exit 1）。`--out` の既定は、その言語の `code` の置き場所（ArchUnit は `test` の置き場所）。書けば `examples/shop/py/.importlinter: Written (11 contracts)` のように一行で言う。`--check` は、書く代わりに、いまの設定が地図から書くものと同じかを確かめる（同じなら `Up to date (11 contracts)`、違えば E502）。書けない地図は E501 |
| `sakai export cml <map.ctx> [--out <file>] [--root <dir>]` | Context Mapper の CML を書く（8 章）。既定は標準出力。検査を通らない地図からは書かない（診断は標準エラーに出して exit 1） |
| `sakai doc <map.ctx> [--format markdown\|html] [--out <dir>] [--root <dir>]` | コンテキストマップの図、コンテキストごとの成果物と用語集、関係と越える参照、対応の表（10 章）。既定は Markdown を標準出力に。`--out` があれば、地図のファイルの名前から `<名前>.md` か `<名前>.html` を書き、`site/shop.html: Written` のように一行で言う。検査を通らない地図からは書かない（診断は標準エラーに出して exit 1） |
| `sakai api <map.ctx> [--root <dir>]` | 地図、属し方、境界を越える参照を JSON で（9 章） |
| `sakai explain <コード>`、`sakai explain --all [--format markdown]` | 診断のコードを引く |

どのコマンドも、ritsu の入口から `ritsu sakai <コマンド> …` で走らせられる（ritsu の DESIGN 8.6）。そのときは、sakai が読むすべての言語がつながる（4.1）。sakai のクレートのバイナリの `sakai` は、ほかの言語を持たないので、地図が規則、カレンダー、ワークフローを含めば E104 を言う（帳簿は `check` が問わないので含んでもよい。4.5）。規則、カレンダー、ワークフローを含まない地図（B の段階の地図など）では、二つは同じに動く。

`--lang ja|en` はどのコマンドにも付けられる（無ければ `SAKAI_LANG`、次に ritsu のどの言語も読む `RITSU_LANG`、どちらも無ければ英語）。`--root` は、パスを数えるルートを替える（2.4。無ければ、最初に渡したパスの上で `.git` を持つ一番近いディレクトリ）。`sakai --help`、`sakai <コマンド> --help`、`sakai --version`。

**決定**：コマンドとフラグの定義を `src/cli.rs` の一枚の表に置き、`--help` の表示と引数の読み取りが同じ表を引く（rulec の 12.1、koyomi、chobo と同じ）。知らないフラグ、閉じた集合の外の値（`--target depguard`）、値の無いフラグ、二度書いたフラグは exit 2 で止める。黙って無視すると、エージェントはフラグが効いたと信じて次に進むからである。

`check`、`api`、`doc`、`export` は、通信も子プロセスもしない（ほかの言語には、同じプロセスの中で口で問う。4.1）。例外は、地図が `code rust` を書くときに、Rust のクレートを尋ねる `cargo metadata` の一回である（7.7。マニフェストを読むだけで、ビルドもネットワークへの接続もしない）。`build` も、設定を書くだけで、import の検査のツールは走らせない。ツールを走らせるのは利用者の CI で、sakai のテストはツールを本当に走らせて設定を確かめる（7.6）。

## 7. コードの import の検査（build）

### 7.1 import を許す単位と向き

**決定**：コードの import は、四つの種類のディレクトリのまとまりで決める。

- **内側**：コンテキストに属するディレクトリのうち、下の三つでないもの。
- **公表された言語から生成したコード**：`published language` の塊の `generated dir`。
- **共有カーネル**：`shared kernel with` に並べたディレクトリ。
- **腐敗防止層**：`upstream … anticorruption layer` の `layer` に並べたディレクトリ（下流の内側の中にある）。

許す import は次のとおりで、ほかは全部許さない。表の「X」は、X の内側、X の生成したコード、X の層のまとまりのことである。

| import のもと | 先 |
|---|---|
| コンテキスト X のどこでも | X のどこでも（内側、X の生成したコード、X の層） |
| X | X が持つ共有カーネル（両側から） |
| X（腐敗防止層に `layer` を書いたなら、その上流に向けた層だけ） | 上流 Y の、`through` に並べた package の生成したコード |
| X | パートナー Y の生成したコード |
| X の生成したコード | Y の生成したコード（X の proto が Y の proto を import していて、それを地図が許すとき） |

C の段階で作ったときに、表だけでは決まらないことが三つあった。

- 共有カーネルのまとまりは、両側のどのまとまりからも import してよいが、共有カーネルの中から import してよいのは共有カーネルの中だけにした。共有カーネルが片方の内側に依れば、もう片方のチームがそれに引きずられるからである。
- 腐敗防止層の `layer` に並べたのが規則のファイルだけで、その言語の層のディレクトリが無ければ、その言語のどのコードも上流の生成したコードを import できない。`layer` を書いたなら層の中だけ、という決まりを、そのまま当てはめた。
- 生成したコードどうしの行は、proto の import のうち、3.3 の検査が許したものから作る。例では、受注の `fulfillment.proto` が在庫の `stock.proto` を import しているので、受注の生成したコードは在庫の生成したコードを import してよい。

7.2〜7.5 の四つの設定は、どれもこの表を、それぞれのツールの言葉に写したものである。ディレクトリは、地図の `code` の置き場所から見たパスで、Python と Java ではモジュール（パッケージ）の名前に、TypeScript と Go ではパスのまま使う。コードのファイルは、それを含むいちばん深いディレクトリのまとまりに属する（1.3 と同じ決まり）。同じ深さなら、生成したコード、層、共有カーネルのほうが内側より先に来る。例では `../py/calendars` が請求の `owns` と共有カーネルの両方にあり、共有カーネルのまとまりになる。コードのファイルを一つも含まないまとまりは作らない。設定に写せないもの（コードのファイルを一つだけ名指した `owns` の項、モジュールの名前にならないディレクトリ。Go と Java ではパッケージがディレクトリなので、ファイルの単位では書けない）は E501 で、書かずに止める。

例の地図の Python のまとまりは次の 11 個で、TypeScript、Java、Go も同じ形になる（`tests/build.rs` が確かめる）。

| まとまり | ディレクトリ | import してよいもの（自分のほか） |
|---|---|---|
| `sakai-ordering` | `ordering` | 受注の生成したコード |
| `sakai-ordering-pl-shop.ordering.v1` | `shop/ordering/v1` | 受注、配送の内側と層（パートナー）、請求の層 |
| `sakai-inventory` | `inventory` | 在庫の生成したコード |
| `sakai-inventory-pl-warehouse.v1` | `warehouse/v1` | 在庫、受注（順応者）、受注の生成したコード、配送の層 |
| `sakai-delivery` | `delivery` | 配送の生成したコードと層 |
| `sakai-delivery-pl-shop.delivery.v1` | `shop/delivery/v1` | 配送、受注の内側（パートナー）、請求の内側と層（顧客） |
| `sakai-delivery-layer-inventory` | `delivery/acl/inventory` | 配送 |
| `sakai-billing` | `billing` | 請求の層 |
| `sakai-billing-layer-ordering` | `billing/acl/ordering` | 請求 |
| `sakai-billing-kernel-delivery` | `calendars` | 請求、配送 |
| `sakai-reviews` | `reviews` | 無し |

設定は四つとも、どのツールでも同じ扱いにする。地図から書く、頭に元の地図と書いたコマンドを書く、`--check` で古くなったかを言う、テストで本物のツールを走らせ、地図に無い import を捕まえ、地図どおりの import を通すことを確かめる（7.6）。頭の地図は、設定のファイルのディレクトリから見たパスで書く（英語の例では `../shop.ctx`、日本語の例では `../通販.ctx`）。ルートに依らないので、どこで書いても同じ文字になる。設定の説明の文（契約の名前、規則の説明）は `--lang` の言語で書き、頭のコマンドにも `--lang ja` を書き残す。違う `--lang` で `--check` すると説明の文が違って E502 になるので、そのときは注で、ファイルがどの言語で書かれているかを言う。

**捨てたもの**：設定の頭に `.ctx` のファイルの SHA-256 を書くこと（A の段階の案）。`.ctx` をどこか一字でも直すと（コメントでも）ハッシュが変わり、`--check` が、中身の変わらない設定を全部書き直させることになる。古くなったかは、ツールが読む中身を一行ずつ比べて言う。

### 7.2 Python：import-linter

**決定**：まとまりごとに、import-linter の `protected` の契約を一つ書く。`protected_modules` がそのまとまりのモジュールで、`allowed_importers` が 7.1 の表でそこを import してよいモジュールである。設定は `.importlinter`（INI）で、`code python` の置き場所に書く。

`root_packages` は、置き場所の直下のディレクトリのうち `__init__.py` のあるものの名前と、`__init__.py` の無いディレクトリ（protoc が書く名前空間のパッケージ）を、`__init__.py` かモジュールのあるディレクトリまで降りた名前（`warehouse.v1`、`shop.ordering.v1`）である。grimp 3.17 は、パッケージの下にある `__init__.py` の無いディレクトリに入らない。`warehouse/__init__.py` があり、その下に protoc の `warehouse/v1/` があると、grimp は `warehouse.v1` のモジュールも、それへの import も読まなかった（`tests/maps/入れ子` で確かめた）。そこで、そういうディレクトリも `root_packages` に足す。置き場所の直下にあるモジュール（`py/top.py`）は、grimp が根に受け付けない（`NotATopLevelModule`）ので E501 にする。

import-linter は、守るモジュールをパッケージとして扱い、その下を全部含める。まとまりのディレクトリが別のまとまりを含むとき（配送の内側 `delivery` の中の、在庫に向けた層 `delivery/acl/inventory`）は、別のまとまりを含まないいちばん大きいモジュールを並べる。ただ、別のまとまりを含むパッケージが自分の `__init__.py` を持つと、それをパッケージとして並べれば下のまとまりまで含めてしまう。そのときは、その契約だけ `as_packages = False` にして、モジュールを一つずつ並べる。また import-linter は、守るモジュールを import してよいのは、そのモジュール自身とその下だけとみなすので、まとまりの中のモジュールどうしの import を止めないよう、まとまり自身のモジュールも `allowed_importers` に並べる。例の `.importlinter` から、在庫の公表された言語の契約と、配送の内側の契約を抜き出す（`…` は省いたところ）。

```ini
[importlinter:contract:sakai-inventory-pl-warehouse.v1]
name = 「在庫」の公表された言語 warehouse.v1 から生成したコードは、「受注」、「在庫」、「配送」の「在庫」に向けた腐敗防止層だけが import する
type = protected
protected_modules =
    warehouse.v1
allowed_importers =
    delivery.acl.inventory
    inventory
    ordering
    shop.ordering.v1
    warehouse.v1
…
[importlinter:contract:sakai-delivery]
name = 「配送」(delivery) の内側は、「配送」だけが import する
type = protected
as_packages = False
protected_modules =
    delivery
    delivery.acl
    delivery.ship
allowed_importers =
    delivery
    delivery.acl
    delivery.acl.inventory
    delivery.acl.inventory.packing
    delivery.ship
    shop.delivery.v1.shipment_pb2
```

例のままでは、11 の契約がどれも KEPT で exit 0 になる。受注の内側に在庫の内側を import するファイルを足すと、import-linter 2.15 は次を出して exit 1 になった。

```text
Illegal imports of protected package inventory:
- ordering.bad_internals -> inventory.ledger (l.2)
```

確かめたことがほかに三つある。契約の名前には日本語を書ける。`protected_modules` に、グラフに無いモジュールを書くと `"shop.nothing.here" not present in the graph.` で exit 1 になる（sakai は、`.py` のあるまとまりだけを書く）。何も付けずに走らせると、作業ディレクトリに `.import_linter_cache` を書く（テストは `--no-cache` を付ける）。

**捨てたもの**：`forbidden` の契約（もとと先の組ごとに書くので、コンテキストの数の二乗になり、新しいコンテキストを足したときに書き漏らすと素通りする）。`independence` の契約（共有カーネルや公表された言語のような、許す import を書けない）。

### 7.3 JavaScript と TypeScript：dependency-cruiser

**決定**：まとまりごとに、`forbidden` の規則を一つ書く。`to.path` がそのまとまりのファイルに当たる正規表現、`from.pathNot` が 7.1 の表でそこを import してよいまとまりのファイルに当たる正規表現である。設定は `.dependency-cruiser.cjs`（`module.exports = { … };` の CommonJS）で、`code typescript` の置き場所に書き、パスもそこから見たものにする。`options.tsPreCompilationDeps` を true にし、型だけの import も数える。

まとまりのディレクトリが別のまとまりを含むときは、否定先読み（`(?!…)`）で外す（`^(?:delivery/(?!(?:acl/inventory)/))`）。A の段階の案は `to.pathNot` で外す形だったが、外したいのは `to` の側だけではない。import してよい側のディレクトリが、import してはいけないまとまりを含むこともある（`from.pathNot` には例外を書けない）。否定先読みなら、どちらの側でも、まとまりのファイルにちょうど当たる式を書ける。

JSON の設定にしないのは、頭に元の地図を書けないからである。dependency-cruiser 16.10.4 は、知らないキー（`"$comment"`）を一番上にも `options` の中にも置かせず、`The supplied configuration is not valid: data must NOT have additional properties.` で止まった。`.cjs` なら先頭の行にコメントを書ける。例の `.dependency-cruiser.cjs` から、配送の内側の規則を抜き出す。

```js
    {
      name: "sakai-delivery",
      comment: "「配送」(delivery) の内側は、「配送」だけが import する",
      severity: "error",
      from: { pathNot: "^(?:delivery/|shop/delivery/v1/|delivery/acl/inventory/)" },
      to: { path: "^(?:delivery/(?!(?:acl/inventory)/))" },
    },
```

dependency-cruiser 16.10.4 と TypeScript 5.9.3 で、例のままでは違反が無く、受注の内側に在庫の内側を import するファイルを足すと、次を出した。

```text
error sakai-inventory: ordering/bad_internals.ts → inventory/ledger.ts
```

`--output-type err` は、この行を出して、エラーの数を exit code にする（一つなら 1）。`--output-type json` は、違反があっても exit 0 で、違反は JSON の `summary.violations` に入る。CI では `err` で走らせる。sakai のテストは JSON を読み、違反の有無を JSON から決める。

**気をつけること**：dependency-cruiser 16.10.4 が読める TypeScript は 2 以上 6 未満で、TypeScript 7.0.2 を入れたまま走らせると、`.ts` のファイルを黙って読み飛ばし、`✔ no dependency violations found (0 modules, 0 dependencies cruised)` で exit 0 になった（`depcruise --info` で `x typescript >=2.0.0 <6.0.0` と分かる）。地図に無い import があっても通ってしまう。sakai のテストは、`depcruise --info` で TypeScript が読まれる（`✔ typescript`）ことと、`summary.totalCruised` が `.ts` のファイルの数より少なくないことを確かめ、どちらかが崩れたら SKIP ではなく失敗にする。README にもこれを書く。

版は、dependency-cruiser 16.10.4 と TypeScript 5.9.3 に固定する（`tools/package.json` と `tools/package-lock.json`）。dependency-cruiser の 17 と 18 は node 23 を対象にしていない（17 は `^20.12||^22||>=24`、18 は `^22||^24||>=26`）ので、この機械の node v23.11.0 で `npm install` すると 16.10.4 が入る。

**捨てたもの**：`allowed` の規則（許すものだけを並べる形）。地図の外のモジュール（node の組み込みや npm のパッケージ）も並べることになる。

### 7.4 Java：ArchUnit

**決定**：まとまりごとに、ArchUnit の規則を一つ持つ JUnit 5 のテストのクラス `SakaiContextsTest.java` を書く（`code java` の `test` の置き場所に。無ければ E501）。デフォルトパッケージのクラスは、規則がパッケージで名指せないので E501 にする。まとまりはクラスの述語にし（そのパッケージで、中の別のまとまりのパッケージでないもの）、規則は「import してよいまとまりの外のクラスは、まとまりのクラスに依らない」と書く。`as(…)` に地図の言葉の説明を付ける。例の `SakaiContextsTest.java` から、配送の内側の述語と規則を抜き出す。

```java
  // 「配送」(delivery) の内側
  private static final DescribedPredicate<JavaClass> SAKAI_DELIVERY = resideInAnyPackage("delivery..").and(resideOutsideOfPackages("delivery.acl.inventory.."));
…
  @ArchTest
  static final ArchRule sakai_delivery =
      noClasses().that(DescribedPredicate.not(SAKAI_DELIVERY.or(SAKAI_DELIVERY_PL_SHOP_DELIVERY_V1).or(SAKAI_DELIVERY_LAYER_INVENTORY)))
          .should().dependOnClassesThat(SAKAI_DELIVERY)
          .as("「配送」(delivery) の内側は、「配送」だけが使う");
```

ArchUnit 1.5.1（archunit-junit5 1.5.1、JUnit Platform Console 6.1.3、OpenJDK 27 で `--release 21` に組んだ）で、例のままでは 11 の規則がどれも通る。レビューのクラスに請求のクラスを使わせると、次を言った。

```text
Rule '「請求」(billing) の内側は、「請求」だけが使う' was violated (2 times):
Method <reviews.BadSeparate.billed(java.lang.String)> calls constructor <billing.Invoice.<init>()> in (BadSeparate.java:6)
```

テストの Java の変異は、import の文を書かず、完全な名前でクラスを使っている。ArchUnit はクラスファイルの依存を読むので、import の文が無くても見つける。import-linter、dependency-cruiser、go-arch-lint は import の文を読むので、四つのツールで捕まえるものが少し違う（import の文だけがあって使っていない依存は、ArchUnit には見えない）。README と docs に書く。

ArchUnit は、`that()` に当たるクラスが一つも無い規則を、既定で失敗させる（`failed to check any classes`。`archRule.failOnEmptyShould`）。パッケージの名前を書き誤った設定が黙って通らないのはよいことだが、どのクラスも使ってよいまとまり（`tests/maps/入れ子` の在庫の公表された言語）では、`that()` に当たるクラスが無い。sakai はその規則を書かず、代わりに理由をコメントに書く。

**捨てたもの**：Spring Modulith の `package-info.java` の注釈を書くこと（0.4。Spring の jar が要り、宣言がコードに散らばる）。ArchUnit の PlantUML の図に照らす規則（`adhereToPlantUmlDiagram`）。図の構文で書けるのはコンポーネントのあいだの矢印で、腐敗防止層だけが import してよい、のような向きを書きにくい。

### 7.5 Go：go-arch-lint（depguard と比べて）

**決定**：go-arch-lint にする。まとまりごとにコンポーネント（`in:` にディレクトリの `**`）を書き、`deps` の `mayDependOn` に 7.1 の表で import してよいコンポーネントを並べる。設定は `.go-arch-lint.yml`（version 3）で、`code go` の置き場所（`go.mod` のあるディレクトリ。無ければ E501）に書く。`except` で外したファイルは `excludeFiles` に入れる。

go-arch-lint v1.19.0（2026-09-07）は、同じコンポーネントの中のパッケージどうしの import（在庫の `inventory` から `inventory/ledger` へ）も、`mayDependOn` に自分を並べないと止めた。中身の無い `mayDependOn` の項も受け付けない。そこで、どのコンポーネントも自分を並べる。例の `.go-arch-lint.yml` から、配送の層のコンポーネントを抜き出す。

```yaml
  sakai-delivery-layer-inventory:
    mayDependOn:
      - sakai-ordering-pl-shop.ordering.v1
      - sakai-inventory-pl-warehouse.v1
      - sakai-delivery
      - sakai-delivery-pl-shop.delivery.v1
      - sakai-delivery-layer-inventory
      - sakai-billing-kernel-delivery
```

例のままでは `OK - No warnings found` で exit 0 になり、受注の内側に在庫の内側を import するファイルを足すと exit 1 になった。`--json` の結果を、文字の出力と同じ形に並べると次のとおり（文字の出力は、ファイルを絶対パスで書く）。

```text
Component sakai-ordering shouldn't depend on example.com/shop/inventory/ledger in ordering/bad_internals.go:4
```

入れ子のコンポーネント（`delivery/**` の中の `delivery/acl/inventory/**`）は、深いほうに属するものとして読まれた。どのコンポーネントにも当たらないファイルがあると `File /cmd/shop/main.go not attached to any component in archfile` で失敗した（A の段階）。コンポーネントの名前には `.` と `-` を使える。

depguard（v2.2.1 の単独の CLI。`.depguard.yaml` の一番上に、名前ごとの `files` の glob と `deny` の package を書く）も、A の段階に手で書いた設定で走らせた。

```
ordering/domain/bad.go:4:2: import 'example.com/shop/inventory' is not allowed from list 'ordering': 在庫(inventory) の中は、在庫 だけが使う
ordering/domain/bad.go:5:2: import 'example.com/shop/gen/shop/inventory/v1' is not allowed from list 'ordering-outside-acl': 在庫 の公表された言語は、受注 の腐敗防止層だけが使う
```

go-arch-lint にした理由は、許すものを並べる形だからである。go-arch-lint は、コンポーネントが import してよいものを並べ、並べていない import と、どのコンポーネントにも属さないファイルを全部止める。地図に新しいコンテキストを足して設定を書き直し忘れても、新しいディレクトリが「どのコンポーネントにも属さない」で止まる。depguard は、ファイルの glob ごとに許さない package を並べる形で、sakai はコンテキストの組ごとに許さないものを書き出すことになり、書き漏らした import は通る。go-arch-lint の形は、地図の形（まとまりと、許す向き）とそのまま重なる。

depguard の利点は、golangci-lint に入っていて、多くのチームがもう走らせていることと、許さない理由の文を出力に出せることである。14 章に、depguard への出力を足すことを残した。

### 7.6 突き合わせ

`tests/imports.rs` は、ツールごとに、地図を一時ディレクトリに写し、`sakai build --target <ツール> --lang ja` で設定を書き、本物のツールを走らせる。地図は 11 章の例と、生成したコードが内側のディレクトリの中にある `tests/maps/入れ子` の二つである。

- 例のままでは、どのツールも違反を言わない。
- 次の四つの import を一つずつ足した写しでは、どのツールもその import を、足したファイルで言う（`tests/code/<言語>/`）。(1) 受注の内側が在庫の内側を import する、(2) 請求の腐敗防止層の外が、受注の公表された言語から生成したコードを import する、(3) 受注が、請求と配送の共有カーネルのコードを import する、(4) レビューが請求を import する。`tests/maps/入れ子` では、受注が、在庫の生成したコードを囲む内側を import する一つ。
- 黙って通っていないことを、ツールごとに確かめる。import-linter は `Analyzed N files` の N が `.py` の数より少なくないこと、dependency-cruiser は TypeScript を読むことと `summary.totalCruised`、ArchUnit は見つかったテストの数が書いた規則の数と同じこと、go-arch-lint は「どのコンポーネントにも属さない」ファイルが無いこと。
- ツールが捕まえた import について言うことを、ツールごとの golden（`tests/golden/imports/<ツール>.txt`）に固定する。この章に貼ったツールの出力は、そこから抜き出したものである（`tests/design.rs` が確かめる）。

ツールごとに、例の五つと入れ子の地図の二つの写しを並べて走らせる。四つのツールで合わせて 28 の写しになり、この機械で 6 秒ほどかかる。

`tests/build.rs` は、例の地図のまとまりと、それぞれを import してよいまとまり（7.1 の表）、例に置いた四つの設定がいまの地図から書くものと一字も違わないこと（`SAKAI_BLESS=1` で書き直す。この章に貼った設定は、そのファイルから抜き出した）、地図を変えると `--check` が E502 を言うこと、E501 の場合を確かめる。

ツールの置き場所は sakai の `tools/` の下で、git に入れない（版を書いたファイルと取ってくるスクリプトだけを入れる。入れ方は `tools/README.md`）。環境変数でほかの場所のものも使える。ツールが無ければ、そのテストは `SKIP: sakai: <理由>` の一行を出して通す（PLAN の 0 章）。

### 7.7 Rust：Cargo に尋ねて、sakai が確かめる（cargo-deny と比べて）

**決定**：Rust のコードは、地図の `code rust "<パス>"` に、ワークスペースの（クレートが一つなら、そのクレートの）`Cargo.toml` のあるディレクトリを書く。境界を越える参照は、クレートの依存として読む。Rust では、コードが `use` できるほかのクレートは `Cargo.toml` の依存に書いたものだけで、それ以外はコンパイラが断るので、クレートの依存を確かめれば、コードがほかのクレートを参照するところを全部確かめたことになる。依存は、`check` のたびに sakai が Cargo に尋ねる（`cargo metadata --format-version 1 --no-deps --offline`。マニフェストを読むだけで、ビルドもネットワークへの接続もしない）。Cargo が言うワークスペースのクレートの依存のうち、パスで書いたもの（`[dependencies]` と `[build-dependencies]`）を、依存を書いたマニフェストの行から、依存先のクレートのマニフェストへの参照として、3.3 の検査にかける。設定を書いてほかのツールに任せる `build` の形（7.1〜7.5）にはしない。

`tests/maps/rust` の Billing のクレートに、関係の無い Orders のクレートを `[dependencies]` で足すと（変異の `E201_a_crate_with_no_relationship`）、次のように言う。

```
error[E201]: billing/Cargo.toml:7:1: The file billing/Cargo.toml of Billing depends on orders of Orders (dependencies), which Billing has no relationship with
     7 | orders = { path = "../orders" }
  = A reference across the boundary needs a relationship: upstream and downstream (the downstream writes `upstream`), a partnership, or a shared kernel.
  involved:
      Billing  billing/Cargo.toml:7      orders = { path = "../orders" }
      Orders   file "orders/Cargo.toml"  a crate inside Orders
```

- クレートは、そのマニフェスト（`[package]` のある `Cargo.toml`）が属するコンテキストのものとする。クレートの `.rs` のファイルも成果物で、どれかのコンテキストに属する（E101）。ワークスペースのためだけのマニフェスト（`[package]` の無い `Cargo.toml`）は、どのクレートのものでもないので成果物にしない。
- 境界の向こうのクレートに依存してよいのは、そのクレートが相手の公表された言語で（`published language` の下の `crate "…"`。1.4）関係がそれを許すとき（3.3）と、二つの共有カーネルに並べたクレートのときだけである。公表された言語に無いクレートなら E202、関係が無ければ E201 になる。
- 数えないもの：`[dev-dependencies]`（テスト、例、ベンチマークだけが使い、クレートの中に入らない）、レジストリや git のクレートへの依存（どのコンテキストのものでもない）、ルートの外のクレート。ワークスペースのメンバーでないクレートの依存は、Cargo が言わないので分からない。範囲の外のクレートに依存していれば E103 で、Cargo に尋ねられなければ（`Cargo.toml` が無い、cargo が無い、マニフェストが読めない）E107 である。E107 のときは、クレートの依存も、クレートの公表された言語の名前も確かめない。
- Cargo は依存を書いた行を言わないので、行は sakai がマニフェストから探す。`[dependencies]`、`[build-dependencies]`、`[target.<…>.dependencies]` の下で、依存のキー（別名を付けたものは別名）で始まる行か、`[dependencies.<キー>]` の表の見出しの行である。
- 要約の括弧の中では、`rust` が、クレートのマニフェストからの参照の数である（3.1）。api の `crossings[].via` は、`dependencies` か `build-dependencies` になる（9 章）。

**理由**：sakai の決まり（P6）では、コードの import は各言語の既存のツールの設定にして、そのツールで確かめる。Rust でも、cargo-deny 0.20.2 の `[bans]` の `wrappers`（そのクレートに依存してよいクレートを並べる）の設定を `sakai build --target cargo-deny` で書く形を、まず試した（ritsu の DESIGN 3.4。ritsu のワークスペースのマニフェストを写したもので、2026-10-04）。分かったことは次の四つである。

- ワークスペースの中の、パスで書いた依存にも効く。ただし `[graph]` に `exclude-dev = true` が要る。無いと dev-dependency も数え、yuen、sakai、dandori のテストが rulec を dev-dependency に持つことを、`unmatched-wrapper` の警告と `banned` のエラーで言う。
- 言語のクレートに別の言語のクレートを足した変異（koyomi が rulec に依存する）は、`error[banned]: crate 'rulec = 0.22.1' is explicitly banned` と、`direct parent 'koyomi = 0.1.0' of banned crate 'rulec = 0.22.1' was not marked as a wrapper` を出して止める。ただし、指す行は `deny.toml` の `{ crate = "rulec", wrappers = [...] }` の行で、依存を足した `Cargo.toml` の行ではない。
- 「どのクレートも依存してはいけない」クレート（ritsu の入口の `ritsu` や `xtask`）を書けない。`wrappers = []` にすると、依存するものが無くても、クレートそのものが `banned` になる。自分を `wrappers` に並べると通るが、`unused-wrapper` の警告が残る。
- CI では、cargo-deny を取ってくる必要がある（Rust のツールチェーンには入っていない）。

Cargo に尋ねる形なら、依存を足したマニフェストの行を名指せ、ほかの言語の境界と同じ診断（E201〜E206）とコードと文になり、Rust のツールチェーンのほかに何も要らない。sakai がコードの import の読み手を書くことにもならない。依存を言うのは Cargo で、sakai は `use` の文を読まない（13 章の「sakai がコードの import を読むこと」には当たらない）。

**捨てたもの**：

- cargo-deny の設定を `sakai build` で書く形（上の四つ）。
- sakai が `Cargo.toml` を自分で読み解く形。TOML の読み手を持つことになり、ワークスペースから継いだ依存（`workspace = true`）、別名（`package = "…"`）、プラットフォームごとの依存を、Cargo と同じに解かなければならない。sakai が自分で読むのは、診断が指す行を探すところだけにした。
- `.rs` の `use` を読む形。クレートを越える参照は依存に書いたものに限られ、コンパイラが確かめている。

## 8. CML への出力

**決定**：`sakai export cml` は、地図を Context Mapper の CML にする。

| sakai | CML |
|---|---|
| 地図 | `ContextMap <別名>`、`type = SYSTEM_LANDSCAPE`、`state = AS_IS`、`contains` に全部のコンテキスト |
| コンテキスト | `BoundedContext <別名>`。`domainVisionStatement` に説明、`responsibilities` に用語集の語の名前（`as` で取り入れた語は除く）。日本語の名前と持ち主はコメント |
| 順応者 | `<下流> [D,CF]<-[U,<上流の役割>] <上流>` |
| 腐敗防止層 | `<下流> [D,ACL]<-[U,<上流の役割>] <上流>` |
| 顧客／供給者 | `<下流> [D,C]<-[U,S,PL] <上流>`（腐敗防止層を添えたら `[D,C,ACL]`） |
| 共有カーネル | `<一方> [SK]<->[SK] <他方>` |
| パートナーシップ | `<一方> [P]<->[P] <他方>` |
| 別々の道 | CML に書く形が無いので、コメント |
| 上流の役割 | `through` の package に公開ホストサービスがあれば `OHS,PL`、無ければ `PL` |
| `through` の package と公開ホストサービス | 関係の `implementationTechnology`（`"Connect: warehouse.v1.StockService, warehouse.v1.PackingService"`。サービスの無い package は名前だけ） |

名前は別名で書く（CML の名前は ASCII の識別子に限られる。0.4）。顧客／供給者には OHS を付けない。Context Mapper の規則（顧客／供給者に OHS は付けない）に合わせたもので、sakai の公開ホストサービスは「境界の向こうから呼べるサービス」の意味なので、顧客も同じサービスを呼ぶ。そのことは `implementationTechnology` に残る。持ち主を CML のチーム（`type = TEAM` のコンテキスト）にしないのは、`SYSTEM_LANDSCAPE` の地図にチームを入れられないからである。`state` を `AS_IS` にしたのは、sakai の地図が実物と突き合わせたものだからである。関係は地図に書いた順（`use context` の順のコンテキストの、ファイルの中の順）に出し、両側に書く関係は先に出てきたほうで一度だけ出す。コメントは `--lang` の言語で書く。頭のコメントに元の地図の名前と書いたコマンドを書き、`.ctx` のハッシュは書かない（7.1 と同じ理由）。

日本語の名前の例の地図から sakai が書いた CML（`--lang ja`。全文は `tests/golden/cml/通販.ja.cml`）の頭の部分を抜き出す。英語の名前の例の CML は `tests/golden/cml/shop.cml`（英語）と `shop.ja.cml`（日本語）で、コンテキストの名前の違いのほかは同じである。

```cml
/* `sakai export cml --lang ja` が 通販.ctx から書いた。直すときは .ctx を直して書き直す。 */
ContextMap shop {
  type = SYSTEM_LANDSCAPE
  state = AS_IS
  contains ordering, inventory, delivery, billing, reviews

  // 「受注」は「在庫」に順応する
  ordering [D,CF]<-[U,OHS,PL] inventory {
    implementationTechnology = "Connect: warehouse.v1.StockService, warehouse.v1.PackingService"
  }
…
  // 「請求」は「配送」の顧客で、「配送」はそれを供給者として引き受ける
  billing [D,C]<-[U,S,PL] delivery {
    implementationTechnology = "Connect: shop.delivery.v1.DeliveryService"
  }

  // 「レビュー」(reviews) と「請求」(billing) は別々の道（CML には書く形が無い）
}

// 「受注」(ordering)。持ち主: 受注チーム
BoundedContext ordering {
  domainVisionStatement = "注文を受け付け、在庫を押さえ、配送に渡し、届くまでを見届ける"
  responsibilities = "注文", "キャンセル"
}
```

この CML と英語の CML を、Context Mapper 6.12.0 の Xtext の検査器（0.4 の小さなプログラム、`tools/cml/Validate.java`）に通すと、どちらも何も言われず exit 0 だった。テストは毎回これを確かめ、顧客／供給者に CF を付けた CML を同じ検査器に通して、`The CONFORMIST pattern is not applicable for a Customer-Supplier relationship.` が出ることも確かめる（検査器が働いていることの確かめ）。CLI の `cm validate` は構文しか見ず、誤りがあっても exit 0 なので使わない（0.4）。検査器は、Context Mapper CLI の配布物（Maven Central の `context-mapper-cli-6.12.0.zip`、SHA-256 `96579d57a5afa110…`）の `lib/` の jar で動かす。Java か jar が無ければ SKIP。

**捨てたもの**：CML から `.ctx` を起こすこと（読み込み）。CML は境界を越える参照の根拠（どの成果物か）を持たないので、起こしても属し方と `through` を人が書くことになる。

## 9. api

**決定**：`sakai api <map.ctx>` は、地図と属し方と境界を越える参照を JSON で出す。読むのは yuen（成果物がどのコンテキストに属し、持ち主はだれか）と、将来の dandori（自分の参照が地図に沿うか）で、検査を通らない地図には出さない（exit 1。診断は標準エラーに出す）。形は一式の api にそろえ、キーを決まった順に出す（serde_json の `preserve_order`）。名前は 2 章の JSON の形、パスはルートからの相対で書く。

キーは `sakai`、`map`、`covers`、`except`、`contexts`、`relationships`、`artifacts`、`crossings` の順。`tests/maps/基本/` の api（全文は `tests/golden/api/基本.json`）から、頭と、関係と成果物と境界を越える参照の一つめずつを抜き出す（`…` は省いたところ）。

```json
{
  "sakai": "0.23.0",
  "map": {
    "name": "基本",
    "alias": "basic",
    "version": "1",
    "file": "基本.ctx",
    "source_sha256": "0a5af23692d634805c9b0291804b16fe1969a029f72d98374aa31f82baece8c9",
    "description": "注文を受け、在庫を押さえ、請求するかを決める"
  },
  "covers": [
    "."
  ],
  "except": [],
…
  "relationships": [
    {
      "kind": "upstream_downstream",
      "upstream": "在庫",
      "downstream": "受注",
      "roles": {
        "upstream": [
          "open_host_service",
          "published_language"
        ],
        "downstream": [
          "conformist"
        ]
      },
      "through": [
        "warehouse.v1"
      ],
      "layer": [],
      "enums": [],
      "terms": [],
      "declared": [
        "ctx/受注.ctx:18"
      ]
    },
…
  "artifacts": [
    {
      "name": {
        "text": "proto \"proto/billing/acl/v1/order_view.proto\"",
        "tool": "proto",
        "path": "proto/billing/acl/v1/order_view.proto",
        "items": []
      },
      "context": "請求",
      "by": "ctx/請求.ctx:6",
      "sha256": "39e90d262543c1ec"
…
  "crossings": [
    {
      "from": {
        "text": "proto \"proto/billing/acl/v1/order_view.proto\"",
        "tool": "proto",
        "path": "proto/billing/acl/v1/order_view.proto",
        "items": []
      },
      "line": 5,
      "to": {
        "text": "proto \"proto/shop/ordering/v1/order.proto\"",
        "tool": "proto",
        "path": "proto/shop/ordering/v1/order.proto",
        "items": []
      },
      "from_context": "請求",
      "to_context": "受注",
      "via": "proto import",
      "elements": [
        {
          "text": "proto \"proto/shop/ordering/v1/order.proto\" enum OrderStatus",
          "tool": "proto",
          "path": "proto/shop/ordering/v1/order.proto",
          "items": [
            [
              "enum",
              "OrderStatus"
            ]
          ]
        }
      ],
      "allowed_by": {
        "relationship": "upstream_downstream",
        "roles": [
          "anticorruption_layer"
        ],
        "declared": "ctx/請求.ctx:14"
      }
    },
```

- `contexts[]`：名前、別名、版、ファイル、ファイルの SHA-256、説明、持ち主、`also`、`owns`（ディレクトリは `{"dir": "<パス>"}`、ファイルは `{"name": <名指し>}`）、`published`（package、`from` に proto か規則の名指し（Rust のクレートなら、その `Cargo.toml` の `file` の名指し）、`services`、`generated`）、`terms`（名前、定義、`also`、`means` の名指し、`as`）。
- `relationships[]`：`kind` は `upstream_downstream`、`shared_kernel`、`partnership`、`separate_ways`。上流と下流の関係は、`roles` の `upstream`（`supplier`、`open_host_service`、`published_language`）と `downstream`（`conformist`、`anticorruption_layer`、`customer`）、`through`、`layer`、`enums`（上流の列挙の名指し、先、`checked`、値の対応）、`terms`、`declared`（宣言した `.ctx` の行）を持つ。対応の先が名前だけなら `"to": {"name": "出荷の可否"}` で、`checked` は false になる。共有カーネルは `sides` に両側の並びを持つ。
- `artifacts[]`：範囲の成果物の全部。名指しと、属するコンテキストと、それを決めた `owns` の行と、ファイルの SHA-256（先頭 16 桁）。yuen が、成果物の定義が変わったかを知るのに使える。
- `crossings[]`：境界を越える参照の全部。もとと先の名指し、行、二つのコンテキスト、参照の仕方（`via`。proto の import は `proto import`、Rust のクレートの依存は、依存を書いた表の名前の `dependencies` か `build-dependencies`、ほかは、もとの言語が口で言う語：`import proto`、`shape`、`apply`、`use calendar`、`use rule`、`use rule … connect` など、`use proto`、`connect`、`flow`）、越えていく要素（使う型と、そこからフィールドでたどれる型。3.3。proto の import と、規則の `import proto` と `shape` のほかは空）、許した関係（`allowed_by`）。
- 確かめていない成果物を並べる `not_checked[]` は、ritsu の段階 E で消した。ritsu の口で一式を読むようになってから、いつも空の並びだったからである。ほかの言語の成果物を読めなければ、検査が E104 か E105 を出し、api は検査を通らない地図には出さないので、api を出すときには、確かめていない成果物は無い。言語をまたいで決められなかったこと（ritsu の口の答えの `Undecided`）は、sakai が尋ねる口（`Rules` の事実、索引の `References` と `Items`、`Books` の事実）には無く、言語の境目の検査（ritsu の DESIGN 7 章）の結果として `ritsu check` が言う（要約の境目の数と JSON の `borders`、`tool` が `ritsu` の診断）。実行のときにしか見えない呼び出し（3.7）は成果物ごとのものではなく、doc の「確かめていないこと」に書く（10 章）。いつも空のキーを残すと、読む人がそこに何かが入ると思って待つことになる。

## 10. doc

**決定**：`sakai doc` は、コードが実現すべきものを理解し、確かめる人が読むページを出す。読み手は、事業を回す人、システムを運用する人、コードを読む開発者である。Markdown（GitHub がそのまま描く Mermaid の図を入れる）と、一枚の HTML（sakai が描く SVG の図）の二つで、中身は同じ。検査を通った地図にだけ書く（`api` と `export cml` と同じ。検査を通らなければ診断を標準エラーに出して exit 1、ほかの言語がつながっていなければ E104 で exit 2）。次の順に並べる。

1. 見出し（`Context map: Shop (shop) v1`、`コンテキストマップ：通販（shop）v1`）、地図の説明、`check` の要約の一行、コンテキストマップの図。コンテキストを四角に、関係を線にし、線に上流の役割（一つに一行）、通る package、`→ 下流の役割` を書く（`open host service` / `published language warehouse.v1` / `→ conformist`）。共有カーネルとパートナーシップは両向きの線、別々の道は点線。図の読み方を一段落で添える。
2. コンテキストの一覧（名前、別名、持ち主、成果物の数、説明）。名前は、そのコンテキストの節へのリンクにする（Markdown では見出しへ、HTML では節の `id` へ）。
3. コンテキストごとの節：説明、持ち主、ほかの呼び名、書いたファイル。成果物の表（ツール、ファイル、口から読んだ中身。規則は入力と出力、帳簿は勘定と振替、dates のファイルは日付と、使うカレンダーの名前とデータの範囲、カレンダーはそのデータの範囲、`.proto` はメッセージと列挙とサービス、ワークフローは実装するサービスと子のフロー。コードは数だけを一行で）。公表された言語（書いたもの、公開ホストサービスとメソッド、それを実装するワークフロー、生成したコードの置き場所）。用語集（語、定義、指すもの、越えていく先。越えた先で値か語に読み替えられるなら「`cancelled_in_ordering` として」と添える）。関係（相手、役割、通る package、その関係を通る参照を `ファイル:行 (読み方) → 名指し` で）。
4. 用語集の索引：全部のコンテキストの語を名前の順に並べる。同じ名前の語がほかのコンテキストにもあれば、そう書く。
5. 対応の表：腐敗防止層の列挙の対応ごとに、上流の値と下流の値（断るなら理由）。対応がどこで決まっているかを一文で書く。規則の `import proto` が対応なら「規則から rulec が読んだ」、対応の先が下流の列挙なら「下流の値は対応の先の列挙にあることを確かめた」、名前だけなら「下流の値は確かめていない」。
6. 確かめていないこと（3.7 のうち、読み手に関わるもの）。最後に、書いた sakai の版と地図のファイル。

パスは、地図のファイルのディレクトリからの相対で書く（`contexts/ordering.ctx`、`billing/rules/billing_need.rule`）。ページはどこで `doc` を走らせても同じになり、例のディレクトリを開いた人がそのままたどれる。名指しも同じ形にする（`proto "proto/warehouse/v1/stock.proto" enum PackingStatus`）。

図の置き方（HTML）：上流を下流の左に置く。列は、上流から下流への矢印をたどった一番長い鎖で決める。上流も下流も持たないコンテキスト（例のレビュー）は、関係のある相手の列に置く。同じ二つのコンテキストのあいだの二つの関係（例の配送と請求の、顧客／供給者と共有カーネル）は、線を並べ、ラベルを線の上でずらす。四角は、そのコンテキストの節へのリンクである。

HTML の枠は ritsu-base の `docpage`（頭、配色の変数、明るい配色と暗い配色）を使い、外のものを何も読まない（スクリプトも無い）。`--lang ja` で日本語のページになる。`--out <dir>` で、地図のファイルの名前から `<名前>.md` か `<名前>.html` を書く。

テスト（`tests/doc.rs`）：例の二つの地図（英語の名前の `shop.ctx` と日本語の名前の `通販.ctx`）について、Markdown を英語と日本語で、HTML を地図の言語で書き、golden（`tests/golden/doc/`）と一字も違わないこと。HTML が外のものを読まず、図の五つの四角がどれも節に行くこと。Markdown のリンクがどれも見出しに行くこと。sakai のクレートのバイナリが例では E104 で止まり、ほかの言語を含まない地図では `ritsu sakai` と同じページを書くこと。Mermaid 11 と 12 が四つの図をどれも描けること（`tools/mermaid`。無ければ SKIP）。Chrome で二つの HTML を開いて画面を撮り、図の四角（請求と在庫）を押すと、その節に移ること（無ければ SKIP）。

**捨てたもの**：

- ページを `api` の JSON から別のツールで組むこと。sakai が `check` のあとに持っているもの（成果物、参照、口の答え）をそのまま使えるのに、JSON を読み直す手間が増える。
- 図の配置をグラフの配置の一般のアルゴリズム（交差を減らす並べ替え）で決めること。コンテキストマップは数個から十数個の四角で、上流を左に置く決まりだけで読める図になり、golden が配置の細かい揺れで変わらない。
- 対応の表に、対応の無い上流の値（0 番の「設定されていない」）を並べること。対応に要らない値で、表が読みにくくなる。

## 11. 例

**決定**：一式の例を写して、五つのコンテキストからなる小さな通販を作る。同じ例を二つ置く。英語の名前で書いた `examples/shop/`（地図は `shop.ctx`）を先に、日本語の名前で書いた `examples/shop.ja/`（地図は `通販.ctx`）を並べる。二つは、コンテキスト、語、規則、カレンダー、フロー、帳簿、コードの識別子の名前が違うだけで、成果物の中身と関係は同じである。どちらも同じ数の境界を越える参照（9 件）を出し、同じ診断が出る（`tests/examples.rs` が両方を確かめる）。日本語の例の名前は、英語の例の名前の順に次の表の括弧に書く。

| コンテキスト | 成果物（写した元） |
|---|---|
| Ordering（受注） | dandori の `fulfillment.flow`（日本語の例では `受注.flow`。dandori の例の引当と発送を写して直す）、`proto/shop/ordering/v1/`（`order.proto` は例のために書く。`fulfillment.proto` は dandori の例を写して直す） |
| Inventory（在庫） | chobo の `inventory.book`（日本語の例では `在庫の引当.book`。chobo の例の `inventory.ja.book`）、`proto/warehouse/v1/stock.proto`（dandori の例の `warehouse.proto` に梱包のサービスを足す） |
| Delivery（配送） | dandori の `arrange_delivery.flow`（`配送の手配.flow`。dandori の例）、rulec の `urgency.rule`（`出荷の急ぎ.rule`。dandori の例）、koyomi の `ship_date.cal`（`出荷日.cal`。例のために書く）、`proto/shop/delivery/v1/shipment.proto`（rulec の corpus の契約を写し、package とサービスを足す） |
| Billing（請求） | rulec の `payment_fee.rule`（`決済手数料.rule`。rulec の corpus）、`shipment_fee.rule`（`出荷の送料.rule`。rulec の corpus を写して `shape` の先を直す）、`billing_need.rule`（`請求の要否.rule`。例のために書く）、koyomi の `payment_terms.cal`（`支払条件.cal`。koyomi の例）と `calendars/tokyo_business_days.cal`（`東京の営業日.cal`。koyomi の例。祝日の表の写しも） |
| Reviews（レビュー） | コードだけ |

関係は七つで、六つのパターンがどれも現れる（腐敗防止層は、rulec の規則が先の対応と、値ごとに書く対応の二つ）。

| 関係 | パターン | 境界を越える参照（sakai が読めるもの） |
|---|---|---|
| Ordering → Inventory | 順応者、`through warehouse.v1` | `fulfillment.proto` が `stock.proto` を import する（Ordering の公表された言語が Inventory の型をそのまま使う。順応者なので許す）。`fulfillment.flow` が `stock.proto` を `use proto` で読み、`StockService` の `Reserve` と `Release` を `connect` で呼ぶ（Inventory の公開ホストサービス） |
| Billing → Ordering | 腐敗防止層、`through shop.ordering.v1`、層は `billing_need.rule` と各言語の `billing/acl/ordering` | `billing_need.rule` が `import proto` で `OrderStatus` を取り込む。対応の先はその規則の列挙で、値の対応は rulec が決める。Ordering の `cancel`（`ORDER_STATUS_CANCELLED`）は、Billing の違う意味の `cancel` とぶつかるが、規則の値 `cancelled_in_ordering` に読み替えられている（日本語の例では、受注の「キャンセル」が、規則の値「受注で取消」に） |
| Delivery → Inventory | 腐敗防止層、`through warehouse.v1`、層は各言語の `delivery/acl/inventory` | 対応は `.ctx` に値ごとに書く（`PACKING_STATUS_SHORT` は断る） |
| Billing → Delivery | 顧客／供給者、`through shop.delivery.v1` | `shipment_fee.rule` の `shape` が `CreateShipmentRequest` を読む |
| Billing ＝ Delivery | 共有カーネル（`tokyo_business_days.cal` と、koyomi がそれから書くコードの置き場所） | `payment_terms.cal` と `ship_date.cal` が同じカレンダーを読む |
| Ordering ＝ Delivery | パートナーシップ | `fulfillment.flow` が、規則 `urgency` を Connect で呼び（`use rule … connect`。規則は Delivery の公表された言語 `rulec.urgency.v1`）、子の `arrange_delivery.flow` を走らせる（パートナーシップなので許す。4.7） |
| Reviews ／ Billing | 別々の道 | 何も無いことを確かめる |

A の段階で、例のために書く規則とカレンダーと proto の下書きを、一式のツールに通した（2026-10-03）。`billing_need.rule` は rulec で `ok`。`order.proto` に `ORDER_STATUS_RETURNED = 5;` を足すと、rulec が次を出して止める（上流が値を足すと、規則の側で止まる。sakai の E105 は、rulec が言うこの診断を注に添える。4.1。2026-10-04 に、英語の例で走らせた出力）。

```
error[E032]: Enum order_status does not agree with OrderStatus in ../../proto/shop/ordering/v1/order.proto
  --> billing_need.rule:5
  |
5 | enum order_status = received | paid | shipped | cancelled_in_ordering(cancelled)
  |      ^^^^^^^^^^^^
  |
 In ../../proto/shop/ordering/v1/order.proto but not in this enum: returned
 The form to add: `<name>(returned)`. The name is yours to decide — the file carries no Japanese.
 A value appeared through the contract, not through this rule. What the new value costs is a decision nobody has made yet (§15.59).
```

日本語の例の `請求の要否.rule` では、A の段階で次だった。

```
error[E032]: Enum 注文の状態 does not agree with OrderStatus in ../../proto/shop/ordering/v1/order.proto
  --> 請求の要否.rule:5
  |
5 | enum 注文の状態(order_status) = 受付(received) | 支払済(paid) | 出荷済(shipped) | 受注で取消(cancelled)
  |      ^^^^^^^^^^
  |
 In ../../proto/shop/ordering/v1/order.proto but not in this enum: returned
```

`shipment_fee.rule`（`shape` の先を `shop.delivery.v1.CreateShipmentRequest` にしたもの）は `ok`。`ship_date.cal` は koyomi で「2 claims hold on all 719 days of ordered (2026-01-01..2027-12-20)」、`payment_terms.cal`（`use calendar` を直したもの）は「3 claims hold on all 689 days of received」（日本語の例の `出荷日.cal` と `支払条件.cal` は、受注日と受領日の同じ日数で、同じ数を言う）。`order.proto` と `stock.proto` は buf 1.54.0 の lint を通った。

コードは、Python（`py/`）、TypeScript（`ts/`）、Java（`java/`）、Go（`go/`）の四つに、同じ形の小さなものを置く。公表された言語から生成したコードと、chobo と koyomi と rulec が書くコードは、本物の代わりに数行の手書きのものにする（頭のコメントに、どのコマンドが本物を書くかを書く）。import の検査で見るのは境界で、中身ではないからである。本物の protobuf のコードは、各言語の protobuf のライブラリが無いと組めず、テストに外の依存が増える。

C の段階で、この例を作った（日本語の名前の `通販.ctx` と五つのコンテキストのファイル）。英語の名前の例は、英語を先にする段階（12.4）で、日本語の例の名前を英語に直した版として足し、日本語の例は `通販` から `shop.ja` に名前を替えた（中身は、`支払条件.cal` の頭のコメントが指す koyomi の例の名前を、koyomi が例の名前を替えたのに合わせて直したほかは、変えていない）。例の README は、D の段階で書いた（英語の例の `examples/shop/README.md` と、日本語の例の `examples/shop.ja/README.ja.md`。コンテキストと関係の表、写した元、コードが本物の代わりであること）。写したファイルには、頭のコメントに写した元と直したところを書いた（祝日の表と dandori の `options.proto` は、写した元のまま）。PLAN の C.0 の直し方のほかに、`受注.flow` のコメントと説明の中の、在庫の値の名前と子の `.flow` のファイルの名前を、直したあとのものに合わせた。写したものと例のために書いたものは、どれもそれぞれのツールの検査を通る（rulec 0.22.1 で規則 4 本、koyomi 0.1.0 でカレンダー 3 本、chobo 0.1.0 で帳簿 1 本、dandori 0.1.0 でワークフロー 2 本。`tests/examples.rs` が確かめる。ritsu の D.8 から、それぞれの言語の口で確かめる）。

四つの言語のコードの置き場所には、sakai が書いた設定（`py/.importlinter`、`ts/.dependency-cruiser.cjs`、`java/src/test/java/SakaiContextsTest.java`、`go/.go-arch-lint.yml`。英語の例には `--lang en`、日本語の例には `--lang ja`）も置く。CI で `sakai build --check` を走らせる使い方そのままの形で、`tests/build.rs` が、いまの地図から書くものと一字も違わないことを確かめる。四つのツールは、例のままでは何も言わず、7.6 の四つの import のどれも捕まえる。

ritsu の D.8 から、`ritsu sakai check` は上の表の参照を全部読む。境界を越える参照は 9 件で（3.1）、どれも関係が許す。英語の例では、境界を越える参照を、参照元のファイルの名前の順に並べるので、規則の二つ（`billing_need.rule` と `shipment_fee.rule`）の順が、日本語の例（`出荷の送料.rule` と `請求の要否.rule`）と逆になる。言語ごとの数と、関係と、診断は同じである。

## 12. 実装

- Rust（edition 2024、手元の stable 1.94.1 で通ること）。依存は serde_json だけ（`preserve_order` の機能を使う）。
- SHA-256 は ritsu-base のもの（FIPS 180-4 の既知の値でテストしてある）。地図とコンテキストのファイルのハッシュ（api）と、共有カーネルの写しの比べ合わせ（E308）に使う。
- proto の読み手は ritsu-proto（4.2。ritsu の C.9 で、sakai の読み手を元に作った）。sakai に残したのは、要素の名指し方と、何も設定していないことを言う列挙の値の決め方（1.7）である。テストは、buf があれば、例と fixture の proto を `buf build -o -#format=json` の結果と比べる（package、import、メッセージ、列挙と値、サービスとメソッド）。`buf/validate` を import する proto は、buf が BSR の依存なしに組めないので比べない。
- 一式の言語は、ritsu の口で読む（`src/suite.rs`。4.1）。口は `Suite`（`Rules`、プロジェクトの索引 `Index`、`Books`）にまとめて渡され、同じファイルには一度の実行で一度だけ問う。コマンドは `src/run.rs` の `run(引数, 口, 標準出力, 標準エラー)` で、sakai のクレートのバイナリ（`src/main.rs`）は何もつながない口を、`ritsu sakai` はすべてをつないだ口を渡す。E104 の注に書く「同じコマンドを `ritsu sakai` で」は、`run` が受け取った引数から作る（スレッドに置く。`suite::COMMAND`）。診断の文面のパスの基点（2.4）も、`run` がスレッドに置いて決め、終わったら戻す（`paths::show_from`。前は `main.rs` が一度だけ決めていた。同じプロセスで何度もコマンドを走らせるテストと ritsu のため）。
- 診断の文面は `tr!` で英語と日本語を隣に書く。台帳は `src/codes.rs`。
- コードの import の検査の設定は `src/build/`（`areas.rs` が 7.1 の表を作り、`import_linter.rs`、`depcruise.rs`、`archunit.rs`、`go_arch_lint.rs` がツールごとの言葉に写し、`mod.rs` が頭と書き出しと `--check` を受け持つ）。CML は `src/cml.rs`。doc は `src/doc/`（`mod.rs` がページの中身を言語ごとに塊の並びに組み、`markdown.rs` と `html.rs` が書き、`draw.rs` が HTML の図を描く。10 章）。
- 外のツールは、版を固定して `tools/` に置く。import-linter は `tools/requirements.txt`（2.15。`uv venv --python 3.13 tools/.venv`）、dependency-cruiser と TypeScript は `tools/package.json` と `tools/package-lock.json`（16.10.4 と 5.9.3）、ArchUnit と JUnit は `tools/java/fetch.sh`（Maven Central から取って SHA-256 を確かめる）、go-arch-lint は `tools/go/install.sh`（`go install …@v1.19.0`、`-trimpath`）、Context Mapper は `tools/cml/fetch.sh` と `tools/cml/Validate.java`、Mermaid は `tools/mermaid/`。どれも、取ってきたものは git に入れない。

モジュールの分け方と、各段階の作業は PLAN.md にある。

### 12.1 ritsu の土台へ移したもの

sakai は ritsu（七つの言語を一つにまとめる処理系）に取り込まれ、ほかの言語と重なっていたコードを、ritsu の土台のクレート（ritsu-base と ritsu-testkit）のものに替えた（ritsu の PLAN の C.8）。替えたのは、SHA-256、二つの言語の文（`tr!`、`Text`、`Lang`）、診断の共通の部分、台帳の書き出しと `explain`、コマンドの表の読み方と `--help` の組み立て、名指しを読む仕組み（2 章の決まりのうち、ツールの語と種類の語と組の読み方）、ルートの探し方とパスの畳み方と表示のパス、テストの共通の部分（一時ディレクトリ、時間の上限つきの実行、golden、ツールの探し方、SKIP の行）である。sakai に残したのは、`.ctx` の字句と構文、診断の sakai の部分（関わるもの。`Refs`）、名指しの診断の文、台帳とコマンドの表の中身、地図の検査、`build`、`export cml`、`api` である。名指しを試す表は ritsu-base の `tests/fixtures/naming.tsv` 一つになり、sakai の写しは消した。

出力は、次のものを除いて一字も変えていない。

- **`--format json` のファイルの場所**：`file` と `references[].file` を、走らせたディレクトリからでなくルートからの相対で書き、地図ごとの JSON の先頭に `root`（走らせたディレクトリから見たルート）を足した（2.4、5.1）。ritsu のどの言語も JSON のパスを同じ形で書くためで（ritsu の DESIGN 6.2 の 9）、yuen とも食い違わなくなる。文面と `api` は変わらない。
- **言語の選び方に `RITSU_LANG` が入った**：`--lang`、`SAKAI_LANG`、`RITSU_LANG`、英語の順に読む。`--lang` の説明と `sakai --help` の最後の行が、この順を書くようになった。
- テストの SKIP の行は、ritsu のどのクレートとも同じ `SKIP: sakai: <理由>` の形になった。`RITSU_TEST_LEVEL` が `fast` なら、外の linter と buf を使うテストは走らせずに SKIP を言う。

続く C.9 で、proto の読み手を ritsu-proto に移した。ritsu-proto は sakai の読み手を元にし、rulec と dandori の読み手が読むもの（Protovalidate の規則、`buf.yaml` と `buf.lock`、オプションの木、import の先を何段でも読むこと）を足したものである。移す前と後で、三つのリポジトリの `.proto` の全部について sakai の読み手が出すもの（要素と行、読めないときの位置と文、import の行き先、型の名前の解決）が一字も違わないことを確かめ、それを ritsu-proto の `tests/golden/sakai.txt` に残した。読み手の単体のテスト（入れ子、`group`、名前の解決、import を探す場所）も ritsu-proto に移した。sakai の出力は変わらない。

段階 D の最初の部分で、ritsu の口（ritsu の DESIGN 3.2）に答える `src/ports.rs` を足した。コンテキストのファイルが持つもの（コンテキストと語）を渡す。定義の文は、コンテキストならファイルの行、語ならその塊の行（語、定義、`means`）で、どの行もコメントと前後の空白を除く。外を名指すものは、`owns`、公表された言語（proto、規則、公開ホストサービス、生成したコード）、関係の共有カーネルとレイヤー、レイヤーが写す列挙、語の `means`、地図の `use context`・`covers`・`except`・`proto root`・`code` である。公表された言語の短い書き方（`means message Order`）は、`sakai api` と同じく、それを宣言した `.proto` から完全に名指す（`tests/ports.rs`）。

同じとき（ritsu の PLAN の D.10）、ritsu-proto が読めない `.proto` を言う文の日本語を直した。期待したものが語のとき、英語のまま日本語に混ぜていた（「a name が要るところに `{` があります」）のを、日本語の語にした（「名前が要るところに `{` があります」）。E106 の文に出る。英語の文と、記号を期待するときの文（「`;` が要るところに `}` があります」）は変わらない。sakai の golden は変わらず、ritsu-proto の `tests/golden/sakai.txt` の一行が変わった。コマンドの振る舞いは、ほかに変えていない（205 回の出力が一字も違わない）。

段階 D の二つ目の部分で、例が一式から写したものをそれぞれのツールの `check` で確かめるテスト（`what_was_copied_passes_the_suite`）は、dandori のワークフローを `ritsu dandori check` で確かめるようにした。dandori のクレートのバイナリは、ritsu の D.3 から規則を読まない（ritsu の DESIGN 2.3。規則を使うワークフローは `ritsu dandori` で走らせる）からである。ツールの場所は `SAKAI_DANDORI` に代えて `SAKAI_RITSU`（無ければワークスペースの `target/debug/ritsu`、次に PATH）から読む。sakai のコマンドの振る舞いは変わらない。

段階 D の二つ目の部分で、ritsu の名指しの決まりに dandori の種類の語（`task`、`case`、`record` と下の `field`、`enum` と下の `value`、`input`、`output`）が入った（ritsu の DESIGN 6.3、PLAN の D.6）。sakai の振る舞いは二つ変わった。一つ、`dandori "…" task reserve` のような名指しを、これまでの E011（dandori にはまだ種類が無い）ではなく、ほかのツールの種類と同じに読む。dandori に無い種類（`table` など）と組の並びの誤りは、これまでどおり E011 である。二つ、種類の語は `.ctx` の予約語でもあるので（1.2）、`task`、`case`、`record` を地図、コンテキスト、語、下流の値の名前にできなくなった（E002）。例とテストに、この三つを名前にしたものは無かった。

### 12.2 一式の読み込み（ritsu の D.8）

段階 D の最後の部分で、止めていた一式の読み込み（PLAN の C.1〜C.5）を ritsu の口で作った（4.1、4.7）。コマンドの振る舞いは、次のところが変わった。どれも決めて変えたもので、ほかの出力は変えていない。例と fixture の `.ctx` に対する 238 回の出力（`check`、`api`、`export cml`、`build`、`explain`、`--help`）を、替える前の sakai と `ritsu sakai` で突き合わせ、違ったのは 13 回で、どれも下のどれかだった。sakai のクレートのバイナリでは、さらに例の `build` の四つと `export cml` が E104 で止まる（18 回）。B の段階の地図と変異の診断は、一字も変わらない。

- 境界を越える参照に、rulec（`import proto`、`shape`、`apply`）、koyomi（`use calendar`）、dandori（`use rule`、`use proto`、`connect`、`flow`）の参照が加わった。例の `check` の要約は「1 crossing checked (proto 1)」から「9 crossings checked (proto 1, rulec 2, koyomi 1, dandori 5)」になり（3.1）、api の `crossings` に 8 件が加わった。api の `via` は、参照の種類ごとの語になった（前は `proto import` と決め打ちしていた。9 章）。
- E104 と E105 の意味を替えた。E104 は「地図が含む成果物の言語がつながっていない」（前は「ツールが無い」）、E105 は「成果物が、その言語の検査を通らないか、読めない」（前は「ツールの api が失敗した」）。どちらも、前は出さないコードだった。
- N101 を退かせた。dandori の参照を読めるようになり、要らなくなった。台帳に残し、`sakai explain N101` は退いた理由と版を言う。番号はほかのものに使い回さない。
- E207、E208、E209 を足した（4.7）。E405 と、規則が先の E403・E407 を出すようになった。
- 規則の `means` の先の要素（`input`、`output`、`enum`、`value`）を、rulec の事実で引くようになった。無ければ E007（前は書いたとおりに受け取っていた）。
- sakai のクレートのバイナリは、ほかの言語を持たない。地図が規則、カレンダー、ワークフローを含むと、`check`、`api`、`build`、`export` は、言語ごとに一つの E104 を出して止まる（exit 1）。前は、それらを属し方でだけ扱って通していた。11 章の例がそうで、例は `ritsu sakai` で走らせる。
- `sakai explain` の E104・E105・N101 の文と再現、E207〜E209 の項、`check --help` の「出しうる診断」（E104、E105、E207、E208、E209、E405 が加わった）。ほかの言語の成果物を含む再現は `ritsu sakai check .` で走らせると書く（5.2）。

テストは、rulec、koyomi、chobo、dandori を `[dev-dependencies]` に持ち、`ritsu sakai` と同じにつないで、コマンドを関数（`sakai::run::run`）として呼ぶ（ritsu の DESIGN 3.3）。例が一式から写したものも、それぞれの言語の口で確かめる。これで、テストが一式のツールのバイナリを走らせるところ（`SAKAI_RULEC`・`SAKAI_KOYOMI`・`SAKAI_CHOBO`・`SAKAI_RITSU`）は無くなり、ritsu-testkit の `Need::Suite` と CI の `SAKAI_*` の変数も消した。

### 12.3 ritsu の段階 E で変えたこと

段階 E の最初の部分（ritsu の PLAN の E.1〜E.3 と、受け取る側の断り方）で、次のことを変えた。

- 参照と規則が持つものを、言語ごとにではなく、プロジェクトの索引で引く（4.1）。コマンドの振る舞いは変えていない（例と fixture の全部の 238 回の出力が、クレートのバイナリでも `ritsu sakai` でも一字も違わない）。
- `ritsu check` のために、`sakai::run::checked` が、`sakai check` が地図ごとに印字するもの（診断一つずつのテキストと `--format json` のオブジェクト、通った地図の `ok —` の行）を、コマンドと同じ関数で作って渡す（ritsu の DESIGN 8.3）。ルートの決め方と、パスをルートからの相対にする部分は、コマンドと同じ関数を使う（`run::root_from`、`run::from_root`）。コマンドの振る舞いは変えていない。
- E104 で止まったときの exit code を 1 から 2 にした（`check`、`api`、`build`、`export`。4.1）。ritsu の受け取る側の三つの言語（dandori の E018、yuen の E206）と同じ断り方にそろえた（ritsu の DESIGN 2.3）。sakai のクレートのバイナリで、規則、カレンダー、ワークフローを含む地図を渡したときの終了コードだけが変わる。台帳の E104 の説明と、`check --help` と `sakai --help` の終了コードの行にも書いた。
- api から `not_checked` を消した（9 章に理由）。api の JSON のキーが一つ減る。`tests/golden/api/` の二つを取り直した。
- Rust のクレートの依存を確かめるようにした（ritsu の PLAN の E.8。ritsu の地図 `ritsu.ctx` のため）。地図の `code rust`、公表された言語の `crate "…"`、成果物の `.rs` とクレートの `Cargo.toml`、Cargo に尋ねる `cargo metadata` の一回（6 章の例外）、E107（7.7）。キーワードに `rust` と `crate` が増え、名前に使えなくなった（`code` の後の言語を誤ったときの注と、公表された言語の下に書けるものの注に、Rust を足した）。Rust のコードを書かない地図の振る舞いは変えていない。テストの土台の地図に `tests/maps/rust`（英語の名前で書いた、四つのクレートのワークスペース）を足し、変異を四つ（E107、E201、E202、E302）足した。

### 12.4 英語の版と日本語の版（例、テストの材料、explain の再現）

例、テストの材料、golden、文書の例は、英語を先にする。日本語のものは消さず、中身も変えず、日本語の版として残して、英語の版を足した。名前の付け方は、次の一つの決まりにそろえた。

- **例**（`examples/`）：英語の名前の版が `examples/shop/`、日本語の名前の版が `examples/shop.ja/` である。もとの `通販` は、`git mv` で名前だけを替えた（中身は、`billing/支払条件.cal` の頭のコメントが指す koyomi の例の名前を、koyomi が例の名前を替えたのに合わせて直したほかは、一字も変えていない）。英語の版は、dandori、koyomi、chobo、rulec の英語の例を写して、コンテキスト、語、フロー、規則、カレンダー、帳簿、コードの識別子の名前を英語にしたもの（92 ファイル。祝日の表は写しのまま）で、日本語の版と同じ数の境界を越える参照（9 件）と、同じ診断を出す（`tests/examples.rs`）。コンテキストの別名は日本語の版と同じなので、設定の中のグループの名前（`sakai-ordering` など）も同じになり、設定の説明の文だけが英語になる。
- **テストの地図**（`tests/maps/`）：日本語の名前の `基本`、`入れ子`、`パターン` はそのまま残し、英語の名前の `basic`、`nested`、`patterns`（コンテキストのファイル 13 本、proto、コード）を足した。診断の同じコードを出す。
- **変異**（`tests/mutants/`）：日本語の名前の 64 本はそのまま残し、同じコードを出す英語の名前のものを 64 本足した。名前は診断のコードで始め（`E001_閉じていない文字列` と `E001_unclosed_string`）、`base` は英語の地図か英語の例を指す。日本語の変異の `base` のうち、例を指す 11 本は、`examples/通販` と書いたまま残した（日本語の名前のものの中身は変えない）。テストの道具（`tests/common/mod.rs` の `base_dir`）が、それを `examples/shop.ja` と読み替える。一つのコードに、英語の名前の変異が日本語の名前のものと同じ数かそれ以上あることは、`tests/codes.rs` の `every_japanese_mutant_has_an_english_one` が確かめる。もとから英語の名前だった 4 本（Rust の地図のもの）は、そのままである。
- **import の検査のコード**（`tests/code/`）：日本語の名前のディレクトリ（`1_在庫の内側` など）はそのまま残し、同じ import を英語の名前のディレクトリ（`1_inside_of_inventory` など、入れ子の地図のものは `nested/`）に足した。`tests/imports.rs` は、英語の例と英語の入れ子の地図でも、四つのツールを走らせる。
- **golden**：日本語の名前のものは、名前も中身もそのまま残した。足したもの：英語の変異ごとの `.en.txt` と `.ja.txt`、`api/basic.json` と `api/patterns.json`、`cml/shop.cml` と `cml/shop.ja.cml`。`imports/*.txt` は、日本語の例のラベルが例の名前の変更に合わせて `examples/shop.ja` になり、英語の例の分が足された。
- **テスト**：日本語の地図、変異、例を読むテストは、名前もコードも終了コードも変えず、英語の地図、変異、例を読む同じ振る舞いのテストを `…_in_english` として足した。日本語でしか確かめられないもの（`columns_after_japanese_count_characters`、全角の空白の E001 など）は、日本語のまま持ち、英語のテストは同じ振る舞い（桁が文字で数えられること）を英語の材料で確かめる。

**`explain` の再現**：台帳の項（`ritsu_base::ledger::Entry`）が、再現を二つ持てるようにした。英語の出力は英語の名前の再現（`map.ctx`、`alpha.ctx`、`beta.ctx`。コンテキストは `Alpha(a)` と `Beta(b)`）を、日本語の出力は日本語の名前の再現（`地図.ctx`、`甲.ctx`、`乙.ctx`）を見せる。決めて変えたことで、sakai の `explain` の英語の出力の再現と、英語の説明文の中の日本語の名前の例（E003 の `map 通販(shop) v1`、E008 の `受注(ordering)`）が英語の名前になる。ほかの言語の出力は変わらない。再現は `tests/codes.rs` が両方とも走らせて、そのコードが出ることを確かめる。診断そのものの文（`check` の出力）は変えていない。

### 12.5 段階 D（ritsu の F.2）で作ったもの

sakai の段階 D を、ritsu の中で作った（ritsu の PLAN の F.2）。作ったものは、doc（10 章）、例の README（11 章）、`docs/`（`reference.md` と `targets.md` は英語、`codes.md` と `codes.ja.md` は `sakai explain --all --format markdown` の出力そのもの）、`README.md`（英語）と `README.ja.md`（日本語で一から書いたもの）、エージェントのスキル（`skills/sakai/`。`SKILL.md` は手で書き、ほかは `skills/sync.sh` が `docs/` から写す）、`THIRD_PARTY_NOTICES.md`（例の中の内閣府の祝日の表の写し）である。

コマンドの振る舞いは、次のところが変わった。どれも決めて変えたもので、ほかのコマンドの出力は変えていない。

- `doc` のコマンドが加わった（6 章）。`sakai --help` のコマンドの一覧に一行増える。
- 口のまとまり（`Suite`）に、koyomi の `Dates` が加わった（4.1、4.4）。doc の成果物の表のためで、`check`、`api`、`build`、`export` は問わない。`ritsu sakai` には ritsu-project が koyomi のものを渡す（ritsu-project の `Joined::sakai`）。
- `Cargo.toml` の `repository` を ritsu のリポジトリにした。sakai は ritsu の一部として配る（入れ方は `cargo install --git https://github.com/i2y/ritsu --locked ritsu`）。
- Mermaid の版を固定した `tools/mermaid/package.json` と `package-lock.json` を足した（chobo の `tools/mermaid` と同じ版。`npm ci --prefix tools/mermaid` で入れ、`node_modules` は git に入れない）。`.gitignore` の行は、`node_modules` がディレクトリでもリンクでも外すよう、末尾の `/` を取った。

テストは四つのファイルに増えた。`tests/doc.rs`（10 章）、`tests/docs.rs`（README、例の README、`docs/`、スキルに載せた `.ctx` の行、`$ sakai …` の出力、診断、設定の抜粋、ツールが言うこと、比べた数、リンク、キーワードの一覧が本物であること。koyomi と chobo の `tests/docs.rs` の形で、コマンドは `ritsu sakai` と同じにすべての言語をつないで走らせ、診断は変異の golden にあることを確かめる）、`tests/skill.rs`（写しが `docs/` と同じ、スキルのリンクが外に出ない、frontmatter の形）、`tests/examples.rs` の `the_two_maps_say_the_same_but_for_the_names`（二つの地図の境界を越える参照が、コンテキストを別名で読めば同じで、CML がコメントと文字列のほかは同じ）。

## 13. 捨てたもの

ここまでの節に書いたもののほかに、次を捨てた。

- **書いても確かめられない項目**（Bounded Context Canvas の戦略上の分類、ドメインでの役割、ビジネス上の決定、仮定、指標）：0.1。
- **sakai がコードの import を読むこと**：四つの言語の import の読み手を書くことになる。どの言語にも成熟したツールがあり、チームはそれをもう CI で走らせている。sakai は地図からその設定を書くほうに回る（P6）。Rust のクレートの依存を sakai が確かめるのは、これに当たらない。依存を言うのは Cargo で、sakai は `use` の文を読まない（7.7）。
- **`.flow` を sakai が読むこと**：dandori の構文を二か所で持つことになる。dandori が口（`References`）で言うものを読む（4.7）。
- **一式のツールを子プロセスで呼び、JSON を読むこと**（C の段階の計画）：一式を ritsu にまとめたので、口で読む（4.1）。
- **定義の文が同じなら同じ意味とすること**：1.6。
- **共有カーネルを、どのコンテキストにも属さない第三のものにすること**：「どの成果物も、ちょうど一つのコンテキストに属する」が崩れる。共有カーネルの成果物は、どちらかのコンテキストに属し、両側が並べる。写しを両側に置くなら、写しはそれぞれの側に属し、中身が同じであること（E308）を確かめる。
- **関係が無いことを、別々の道とみなすこと**：1.5。
- **地図の状態（AS_IS と TO_BE）を書けること**：書いても確かめるのは今の実物で、TO_BE の地図は実物と突き合わせられない。

## 14. まだやらないこと

どれも、作る理由が見えたら作る。ここに書くのは、黙って消えたように見せないためである。

一式とのつなぎ：

- 規則が実際に読むフィールドだけを、`shape` の越える要素に数えること（3.3）。rulec の口が、規則の入力ごとに読むフィールドを構造で言うようになれば、そちらを読む。
- dandori の `connect` のメソッドの入力と出力のメッセージを、越えていく要素に数えること（3.6）。
- geas の `map` の記録から、主張ごとにどのコンテキストのコードを走らせたかを doc に出すこと。

言語：

- 持ち主を CODEOWNERS と突き合わせること。CODEOWNERS の照合の決まりを正しく写す必要がある。
- OpenAPI と JSON Schema の列挙を、対応の網羅に使うこと（rulec の `import jsonschema` と同じ形で）。外のシステム（Stripe、AWS）を、OpenAPI や Smithy の記述を公表された言語に持つコンテキストとして書くこと。
- 腐敗防止層の対応から、翻訳のコードを生成すること（1.7）。
- 実行時の呼び出し（HTTP の URL、キュー、データベース）を成果物として書くこと。
- Rust のクレートの中の、モジュールの単位の境界（7.7。いまはクレートの単位で確かめる）。

出力：

- depguard（golangci-lint の設定として）と Spring Modulith の注釈への出力（7.4、7.5）。
- 用語集を Contextive の `*.glossary.yml` として、コンテキストのディレクトリに書くこと。
- CML の読み込み（8 章）。Context Mapper のほかの生成器（MDSL など）につなぐこと。
- `sakai diff`（二つの版の地図で、何が変わったか）、MCP のサーバー、ブラウザで試すページ、ドキュメントのサイト。

公開：

- リリースとバイナリ、crates.io。今の入れ方は、リポジトリを取ってきて `cargo install --path .` である。
