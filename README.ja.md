# ritsu

**七つの小さな言語を、一つの処理系で。ある言語が確かめたことを、隣の言語が前提にできる。**

コーディングエージェントは、人が一行ずつ読めるより速く、多くのコードを書くようになりました。そこで問いがはっきりします。誰が、あるいは何がコードを書いても、コードが実現すべきものは何か。サービスのあいだと中の契約。業務の規則。暦と期日。帳簿と、その上限と下限。ワークフローの骨組み。要件がどこから来て、いま何がそれを満たしているか。

ritsu は、そのそれぞれに小さな言語を一つずつ与えます。どれも、人が読めるファイルです。各言語は、書いたことを確かめられるところまで確かめます。試しに選んだ例ではなく、すべての入力、すべての日、すべての道筋についてです。コードが要るところではコードを生成し、中身を理解したい人のためのページを作ります。七つが一つの処理系を共有しているので、証明が言語の境目で切れません。ワークフローは呼ぶ規則の前提を知り、規則は暦がとりうる日を知り、帳簿は規則が返しうる額を知っています。

つなぎのコードはエージェントが書き、コードが実現すべきものは ritsu が持ちます。

## 七つの言語

| | ファイル | 何のための言語か | 生成するもの |
|---|---|---|---|
| [rulec](#rulec業務の規則) | `.rule` | 業務の規則：運賃、手数料、資格、税 | 12 の言語の関数 |
| [dandori](#dandoriワークフローの骨組み) | `.flow` | ワークフローの骨組み | Temporal、Step Functions、Argo ほか |
| [koyomi](#koyomi期日) | `.cal` | 期日：締め日と支払日、営業日、法令の期間 | TypeScript、Python、Go、Rust、SQL |
| [chobo](#chobo帳簿) | `.book` | 帳簿：勘定、その上限と下限、振替、仮押さえ | PostgreSQL と TigerBeetle を使うクライアント |
| [geas](#geasエージェントが書いたコードについての主張) | `.geas` | エージェントが書いたコードについての主張 | — |
| [yuen](#yuen要件の出どころ) | `.req` | 要件の出どころと、それを満たすもの | ReqIF、W3C PROV |
| [sakai](#sakai境界づけられたコンテキストの地図) | `.ctx` | 境界づけられたコンテキストの地図 | import を確かめるツールの設定、Context Mapper |

どの言語も、それだけで使えます。`.flow` を書かずに rulec だけを使うこともできます。一つのファイルに二つの言語を混ぜることはしません。運賃、暦、帳簿、ワークフローを読む人は、それぞれ違うからです。

以下の例は英語で書いたものです。dandori、koyomi、chobo、geas などの例には、日本語で書いた版（`.ja.flow`、`.ja.cal` など）もあります。

### rulec：業務の規則

規則は、表と、そのまわりの計算、例外、ただし書きで書きます。`rulec check` は、宣言した範囲のすべての入力に答えがちょうど一つあることを証明し、単位とお金を確かめ、Lean の証明が検査し直す証明書を書きます。数字は、転記した元の出典に固定でき、出典が変われば検査が止まります。検査を通った規則だけが、Python、NumPy、TypeScript、JavaScript、Rust、Ruby、PHP、Go、Swift、Java、SQL、Wasm の普通の関数になります。

```rule
rule paypal_fee v1
description "The PayPal Checkout fee on one payment in the United States. Transcribed from PayPal's published merchant fees"

source paypal = file "sources/paypal-us-fees.md" sha256:0318950a982c3c7d  # PayPal's own published figures
  table1 sha256:5e481ef40e570eeb

inputs
  amount        : money[USDc]  range >=1USDc <=100000000USDc
  international : bool

# The fee has fractions of a cent in it, and the page does not say which way they settle, so
# the direction here is a placeholder — the thing a person has to decide before this ships.
outputs
  fee : money[USDc]  round half_up(1USDc)

# The page prints the domestic rate and, separately, what an international transaction adds.
# It does not print the sum, so neither does this: the row for a domestic payment adds
# nothing, and that row carries no citation because `0%` is not a figure the copy shows.
table surcharge
policy unique
| international | -> extra : rate[step 0.01%] |
| false         | 0%                          |
| true          | 1.5%                        |  @paypal table1

define fee : money[USDc] = amount × 3.49% + amount × extra + 49USDc  @paypal table1

examples
| amount    | international | -> fee  |
| 10000USDc | false         | 398USDc |
| 10000USDc | true          | 548USDc |
```

### dandori：ワークフローの骨組み

ワークフローには、何をどの順に呼ぶか、リトライとタイムアウト、そして呼び出しが外の案件（注文、支払い、予約）をどう動かすかを書きます。`dandori check` は、すべての道筋で、引き渡さない案件を、手を付けないか終えるかのどちらかで残し、途中で放り出さないことを示します。同じファイルが、Temporal（TypeScript、Python、Go）、Step Functions、Argo Workflows、Lambda の durable functions、pydantic-graph のコードになり、どれも dandori の参照インタプリタと突き合わせて確かめます。

```flow
workflow arrange_delivery v1
description "Book the delivery of an order with the carrier the urgency rule chose, and answer with its tracking number. Written once for every platform, since its calls are HTTP ones that dandori writes for each (`connection` is what Step Functions needs of them): each version of fulfillment runs it as its child, a child workflow on Temporal, an invoked durable function on Lambda durable functions, a workflow of this WorkflowTemplate on Argo, a nested execution on Step Functions"

enum carrier = standard | next_day

record Booking
  tracking_number : string

inputs
  order_id  : string
  carrier   : carrier
  recipient : string?
  extra     : json

outputs
  tracking_number : string

# The next-day carrier sends a van for the parcels; on a busy day it has no van left.
task book_next_day(order_id: string, recipient: string?) -> Booking
  http POST "https://next-day.example.com/v1/pickups"
  connection "arn:aws:events:ap-northeast-1:123456789012:connection/next-day/2a3b4c"
  errors no_van = 409
  key
  retry 2 times every 5 seconds

task book_standard(order_id: string, recipient: string?, extra: json) -> Booking
  http POST "https://post.example.com/v1/parcels"
  connection "arn:aws:events:ap-northeast-1:123456789012:connection/post/3b4c5d"
  key
  retry 2 times every 5 seconds

flow
  match carrier
    next_day =>
      let booked = book_next_day(order_id: order_id, recipient: recipient)
        on no_van => fail NoVan "No next-day van is left for order {order_id}"
      succeed tracking_number = booked.tracking_number
    standard =>
      let booked = book_standard(order_id: order_id, recipient: recipient, extra: extra)
      succeed tracking_number = booked.tracking_number
```

### koyomi：期日

期日は、契約や法令の言い方のまま書きます。請求日から 30 日後、休みなら次の営業日、というふうにです。`koyomi check` は、宣言した範囲のすべての日について日付を計算し、書いた条件をその一日ずつで確かめます。暦は固定した出典から読みます（ここでは GOV.UK が出しているイングランドとウェールズの祝日）。TypeScript、Python、Go、Rust、SQL のコードを生成します。

```cal
dates net30 v1
description "Net 30: due 30 days after the invoice date, moved to the next business day in England and Wales when that day is closed. The claims are this example's own, written as they read"
use calendar "calendars/england_and_wales.cal"

inputs
  invoice_date : date  range >=2026-01-01 <=2028-11-29

date due = invoice_date
  + 30 days             # "Net 30": due 30 days after the invoice date
  roll following        # on the next business day when that day is closed

claims
  due_on_a_business_day   : due is open
  at_least_30_days        : due >= invoice_date + 30 days
  later_invoice_later_due : due is monotonic
```

```console
$ cd crates/koyomi/examples
$ koyomi check net30.cal --lang ja
net30.cal: ok — 3 つの条件が、invoice_date 2026-01-01〜2028-11-29 の 1,064 日のすべてで成り立ちます
```

### chobo：帳簿

帳簿には、勘定と、それぞれが守る上限と下限、勘定のあいだの振替を書きます。chobo は、振替、仮押さえ、期限切れをどう重ねても、勘定が上限と下限の外に出ないことを確かめます。生成するクライアントでは、どの振替も、上限と下限を守る一度の書き込みになります。PostgreSQL と TigerBeetle の上で、TypeScript、Python、Go で使えます。

```book
# A refund never exceeds the sale: what is left to refund on each order is an account of its own.
book refunds v1
description "What is left to refund on each order is an account of its own: the sale adds to it, a refund takes from it, and it never goes below 0. A refund is held while it waits for approval"

unit USD scale 2

account refundable(order: string) : USD
  description "what is left to refund on the order"
  at least 0 refused as refund_exceeds_sale
account sales : USD outside
account refunded : USD outside

transfer sale(order: string, amount: USD)
  key order
  move amount from sales to refundable(order)

transfer refund(request: string, order: string, amount: USD)
  description "held while it waits for approval: posted when it is approved, voided when it is turned down"
  key request
  pending expires after 7 days
  move amount from refundable(order) to refunded
```

### geas：エージェントが書いたコードについての主張

コードはエージェントが書き、主張は人が読みます。geas は主張を一つずつ、動いているプログラム（コマンドライン、HTTP のサービス、ブラウザのページ）に当てて確かめ、差分を渡せば、それがどの主張にかかわるかを答えます。OpenSpec の仕様を渡せば、`geas scenarios` が、同じ名前の主張で確かめていないシナリオを挙げます。

```geas
# The human-auditable half. The implementation (calc.py) is agent-written;
# these claims are what a reviewer actually reads.

target calc {
  run "python3 calc.py"
}

claim "adds two integers" {
  when calc.run("2", "+", "3")
  then stdout is "5"
  and  exit is 0
}

claim "multiplies" {
  when calc.run("6", "*", "7")
  then stdout is "42"
}

claim "refuses division by zero" {
  when calc.run("1", "/", "0")
  then exit is 1
  and  stderr contains "division by zero"
}

claim "rejects an unknown operator" {
  when calc.run("1", "%", "2")
  then exit is 2
  and  stderr contains "unknown op"
}
```

### yuen：要件の出どころ

要件ごとに、どこから来たか（法令の条、文書、決定）、誰が受け持つか、何が満たし何が確かめるかを書きます。どのリンクも両端のハッシュで固定されていて、出典、要件、満たすもののどれかが変われば、その先のリンクは、人が見直すまで止まります。yuen は米国の eCFR と日本の e-Gov から法令を読み、ReqIF と W3C PROV に書き出せます。[OpenSpec](https://github.com/Fission-AI/OpenSpec) の仕様の要件も同じように固定し、変更が要件を書き換えると止まります。

```req
requirements osha v1
description "A requirement read from 29 CFR 1910.157, for the tests. Written as an example; it does not say how the regulation is to be read"

role safety "decides how the regulation reads"

source osha = law ecfr "29 CFR 1910" asof 2026-01-01
  "§1910.157" sha256:c2a9ce966c7e2269

requirement extinguisher_distance
  text "No employee travels more than 75 feet to a portable fire extinguisher for Class A fires"
  owner safety
  from @osha "§1910.157"
    reviewed 2026-10-03 by safety sha256:c2a9ce966c7e2269 -> sha256:08a4819829372b9b
  not satisfied "The test material names no artifact"
    approved 2026-10-03 by safety sha256:08a4819829372b9b
  not verified "The test material names no claim"
    approved 2026-10-03 by safety sha256:08a4819829372b9b
```

### sakai：境界づけられたコンテキストの地図

地図には、どのコンテキストがどのファイルを持ち、どの言語を公開し、誰が誰に何を通して依存してよいかを書きます。sakai は、コンテキストのあいだをまたぐすべての参照を、ここにあるすべての言語のファイルと、サービスが契約として持つ OpenAPI と AsyncAPI の文書と、コードについて確かめます。コードの import は import-linter、dependency-cruiser、ArchUnit、go-arch-lint に渡し、Rust のクレートは Cargo に尋ねます。ritsu は自分のクレートをこの形で地図にしています。次はそのコンテキストの一つです。

```ctx
context Workflows(dandori) v1
description "Workflows that call the rules, made into Temporal, Step Functions, Argo and more (dandori)"

owns
  dir "../crates/dandori"

published language dandori
  crate "../crates/dandori"

# A language stands on the base, and reads another language only through the ports.
upstream Base conformist
  through ritsu_base, ritsu_units, ritsu_proto, ritsu_emit, ritsu_ports
```

## 言語が出会うところ

七つが一つの処理系を共有しているのは、ある言語が確かめたことを、別の言語が前提にできるようにするためです。たとえば返金の規則は、支払った額より多くを求める人はいない、と決めてかかっています。

```rule
rule refund_check v1
description "Whether a refund is paid at once or reviewed. A refund never asks for more than was paid, which the rule takes for granted"

enum path = at_once | review

inputs
  paid  : number  range >=0 <=10000
  asked : number  range >=0 <=10000

constraint asked <= paid

outputs
  route : path

table pick
policy unique
| asked | -> route : path |
| <=100 | at_once         |
| >100  | review          |
```

ワークフローは、客が求めた額をそのまま渡して、この規則を呼びます。

```flow
workflow refund v1
description "Pays a refund back at once or sends it to review, as the rule decides"

use rule check from "refund_check.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:refund-check"

inputs
  order : string
  paid  : int  range >=0 <=10000

task ask_amount(order: string) -> int range >=0 <=10000
  lambda "arn:aws:lambda:us-east-1:123456789012:function:ask-amount"
  idempotent

task pay_back(order: string, amount: int range >=0 <=10000)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:pay-back"
  key

flow
  let asked = ask_amount(order: order)
  let decision = check(paid: paid, asked: asked)
  match decision.route
    at_once => pay_back(order: order, amount: asked)
    review => pass
```

どちらのファイルも、それぞれの言語の検査は通ります。合わせると通りません（この二つのファイルは、`ritsu explain E201` が出す再現です）。

```console
$ ritsu check . --lang ja
ok refund_check.rule
refund.flow: 検査を通りました
エラー[ritsu E201]: refund.flow:21:1: 規則 check を呼ぶところで、前提 `asked <= paid` を破る値を渡すことがあります
    21 |   let decision = check(paid: paid, asked: asked)
  = `asked` は `>=0 <=10000`、`paid` は `>=0 <=10000` で、asked = 10000, paid = 0 のとき `asked <= paid` が成り立ちません
  = 規則から生成したコードは、前提を破る呼び出しを入口で受け付けません。この呼び出しは、ワークフローを走らせたときに初めて落ちます。値の範囲は、dandori がその値を入れるすべての場所から集めたものです。
  = 値を渡す前に前提を保つよう分岐するか、範囲を狭めてください（入力やタスクの結果の `range`）。
ritsu check: ファイル 2 個（rulec 1、dandori 1）。検査を通らないもの 1 個（エラー 1 件）。言語の境目: 確かめた 1 か所、決められない 0 か所
```

言語をまたぐ検査の答えは、いつも三つのどれかです。成り立つ、成り立たない例がここにある、決められない（そのときは理由を言う）。黙って通すものはありません。たとえば次のものを確かめます。

- ワークフローが規則を呼ぶすべての場所で、規則の前提が成り立つか
- koyomi の日付がとりうる日を、規則の入力の範囲にすること（`range from koyomi`）。規則が宣言した範囲に収まるか
- 規則の出力を chobo の振替の額に渡すとき、その額で振替が拒否されうる理由
- chobo の仮押さえの有効期限と、ワークフローが営業日で数える待ちの長さ
- rulec、dandori、chobo で一つの型になったお金と単位
- yuen は表、日付、主張、タスクを一つずつハッシュで固定し、sakai はすべての言語の参照を行番号つきで確かめる

## 人のためのページ

コードが実現すべきものを理解し、確かめたい人（業務の担当者、経理や法務、運用する人、コードをレビューする開発者）のためのページも、同じファイルから英語か日本語で作れます。書かれたことを読み、自分の知っていることと照らし合わせられます。チームで規則を承認する決まりがあるなら、その承認にもこのページが使えます。

- `rulec doc`：規則の表、ただし書き、証明できたこと（Markdown か HTML）
- `dandori doc`：ワークフローの図と、走るすべてのシナリオ
- `koyomi doc`：暦を月ごとに
- `chobo doc`：帳簿、その上限と下限、振替
- `yuen trace`：要件を出典までさかのぼり、満たすものまでたどる
- `explain <コード>`：すべての診断の説明と、そのまま走らせられる再現

## 証明

rulec の証明書は、Lean 4 で検査し直します。chobo、koyomi、dandori の芯、ritsu の言語をまたぐ検査には、Rust の実装の横に Lean のモデルがあり、同じ入力（数百万行）を両方に流して、答えを一つずつ突き合わせます。`sorry` や自前の公理に頼る証明はありません。

## AI エージェント向け

ritsu とその言語は、AI エージェントに使ってもらうためのものです。[skills/](skills) に、八つの [Agent Skills](https://agentskills.io) を置いています。[skills/ritsu](skills/ritsu) は二つ以上の言語を使うプロジェクトのためのスキルで、`ritsu check` から、ワークフローの実行とパッケージの生成までの手順、言語をまたぐ診断の読み方と直し方、残りは言語ごとのどのスキルを読むか、が入っています。残りの七つは言語ごとのスキルで、`skills/<言語>/` にあります。一覧は [skills/README.ja.md](skills/README.ja.md) にあります。入れ方は四つあります。

- **Claude Code**：ritsu のサイトがプラグインのマーケットプレイスを公開していて、プラグイン `ritsu` に八つが入っています。`/plugin marketplace add https://i2y.github.io/ritsu/marketplace.json` を実行してから、`/plugin install ritsu@ritsu` を実行します。Claude Code が取ってくるのは `skills/` のフォルダーだけで、リポジトリ全体はダウンロードしません。
- **どのエージェントでも、バイナリから**：`ritsu skills install` が、プロジェクトの `.claude/skills/` に書きます。`--user` を付けると `~/.claude/skills/` に、`--dir <dir>` を付けると、ほかのエージェントがスキルを読む場所に書きます。名前を挙げると（`ritsu skills install rulec dandori`）そのスキルだけを書き、`ritsu skills list` で一覧を出します。
- **手で**：`skills/` から必要なフォルダーを、`~/.claude/skills/` か、プロジェクトの `.claude/skills/` にコピーします。
- **リリースから**：`ritsu-skills-v<版>.zip` に八つのフォルダーが入っています。エージェントがスキルを読む場所に展開します。

## コマンド

```
ritsu check <ディレクトリ>        各言語の検査と、言語をまたぐ検査
ritsu run <フロー> …              規則を評価し、日付を計算し、帳簿を動かしながらワークフローを走らせる
ritsu gen <ディレクトリ> --out …  プロジェクト全体を、TypeScript、Python、Go のどれか一つのパッケージに
ritsu explain <コード>            診断の意味と再現
ritsu skills install              Agent Skills を、エージェントが読む場所に書く
ritsu <言語> …                    各言語のコマンド（例：ritsu rulec doc fee.rule）
```

リリースのアーカイブには、`ritsu` と、それを指す七つのリンク（`rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`yuen`、`sakai`）が入っていて、どれもその言語として動きます。`cargo install` で入るのは `ritsu` のバイナリだけなので、言語は `ritsu <言語> …` で呼ぶか、言語の名前のリンクを自分で作ります（`ln -s "$(command -v ritsu)" ~/.cargo/bin/rulec` など）。日本語にするには `--lang ja` を付けます。

## 入れ方

ソースから入れます（新しい stable の Rust が要ります）。

```
cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

一つの言語だけを入れることもできます。たとえば、ほかの言語を必要としない rulec なら次のとおりです。

```
cargo install --git https://github.com/i2y/ritsu --locked rulec
```

リリースは 0.23.0 から（rulec の番号の続き）で、リリースごとに macOS と Linux のアーカイブと、`.deb` と `.rpm` を[リリースのページ](https://github.com/i2y/ritsu/releases)に置いています。Homebrew なら `brew install i2y/tap/ritsu`、GitHub Actions なら `uses: i2y/ritsu@v0.23.0` で入ります。

## リポジトリ

- `crates/<言語>`：各言語。それぞれに README と設計の文書がある：
  [rulec](crates/rulec/README.md)、[dandori](crates/dandori/README.md)、
  [koyomi](crates/koyomi/README.ja.md)、[chobo](crates/chobo/README.ja.md)、
  [geas](crates/geas/README.ja.md)、[yuen](crates/yuen/README.ja.md)、
  [sakai](crates/sakai/README.ja.md)（rulec と dandori は英語だけ）
- `crates/ritsu-*`：共通の土台、単位、口、プロジェクトの読み込み、言語をまたぐ検査、`.proto` の読み手、生成の共通部分、ブラウザ向けのビルド
- `proofs/`：Lean のモデル
- `website/`：サイト（ritsu のページ、ブラウザで試すページ、`website/rulec` に置いた rulec のサイト、`website/dandori` に置いた dandori のサイト）
- `DESIGN.md`、`PLAN.md`：全体の設計（日本語）
- `SECURITY.md`：脆弱性の知らせ方

## ライセンス

MIT OR Apache-2.0
