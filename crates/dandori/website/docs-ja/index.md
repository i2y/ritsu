---
title: "ワークフローを書く。検査する。ビルドする。"
hide:
  - navigation
  - toc
---

<div class="dd-hero" markdown>
<img class="dd-hero__mark" src="images/mark.svg#only-dark" alt="">
<img class="dd-hero__mark" src="images/mark-light.svg#only-light" alt="">

# dandori

<p class="dd-hero__tag">ワークフローを書く。検査する。ビルドする。</p>

<p class="dd-hero__lede">
<strong>業務ルールを呼び出すワークフローを書くための、型付きの小さな言語です。</strong>ホテルの予約を受けてカードの与信を取る、注文の明細ごとに在庫を引き当てる、問い合わせに返事を書く、といった処理を書きます。ワークフローは API や規則を呼び出し、途中で待ち、失敗すればリトライします。Stripe の PaymentIntent のように、呼び出すたびに外部のサービスの中で状態が変わっていくものも扱えます。
</p>

<p class="dd-hero__lede">
<strong>動かす前に検査します。</strong>型や match の分岐の漏れだけでなく、ワークフローが終わったときに支払いや注文が中途半端な状態で残らないか、リトライで同じ変更を二度加えてしまわないか、実行履歴がプラットフォームの上限を超えないかまで調べます。
</p>

<p class="dd-hero__lede">
<strong>五つのプラットフォーム向けにビルドします。</strong>Temporal（TypeScript か Python）、AWS Step Functions、AWS Lambda durable functions、Argo Workflows、pydantic-graph です。どのプラットフォーム向けのコードも、テストで作ったすべてのシナリオで動かし、参照インタプリタと結果を突き合わせています。
</p>

<div class="dd-hero__cta" markdown>
[インストール](install.md){ .md-button .md-button--primary }
[ワークフローを書く](tour.md){ .md-button }
[例で見る](examples.md){ .md-button }
[GitHub](https://github.com/i2y/dandori){ .md-button }
</div>
</div>

<div class="dd-overview" markdown>
![.flow と、それが呼ぶ rulec の規則を検査に通す。検査は、型と match のすべての分岐、案件が最後に残りうるすべての状態、外部のデータを変える呼び出しのリトライ、プラットフォームごとの実行履歴の大きさ、範囲・子の .flow・API の記述、プラットフォームにできることとできないことを見る。参照インタプリタが意味を一つに決め、シナリオはどの分岐もどのエラーも通る。ビルドは Temporal（主なプラットフォーム）、AWS Step Functions、Lambda durable functions、Argo Workflows、pydantic-graph 向けにコードを生成し、どれもシナリオごとに走らせて参照と突き合わせる](images/overview-ja.svg#only-dark)
![.flow と、それが呼ぶ rulec の規則を検査に通す。検査は、型と match のすべての分岐、案件が最後に残りうるすべての状態、外部のデータを変える呼び出しのリトライ、プラットフォームごとの実行履歴の大きさ、範囲・子の .flow・API の記述、プラットフォームにできることとできないことを見る。参照インタプリタが意味を一つに決め、シナリオはどの分岐もどのエラーも通る。ビルドは Temporal（主なプラットフォーム）、AWS Step Functions、Lambda durable functions、Argo Workflows、pydantic-graph 向けにコードを生成し、どれもシナリオごとに走らせて参照と突き合わせる](images/overview-ja-light.svg#only-light)
</div>

---

## dandori がすること

<div class="dd-row" markdown>
<div markdown>

```flow
case pi : PaymentIntent follows payment_intent.payment
  held capture_method = manual
  held confirmation_method = automatic
  external authenticate, settle, expire
  refused when refused = true

flow
  let quote = hold(room: booking.room, nights: booking.nights)
  match quote.handling
    review => succeed outcome = awaiting_review
    auto => pi <- create_intent(amount: quote.amount, …)
  pi <- confirm_intent(intent: pi.id)
    on card_declined => pi <- get_intent(intent: pi.id)
```

</div>
<div markdown>

### 判断はワークフローの外に置く

`.flow` 自身は比較も計算もしません。分岐は、列挙・bool・オプショナルな値の `match` だけで、その値は規則の結果か、タスクの結果（API、エージェント、自分で書くコード、人の承認）です。抜けがあると困る判断は、[rulec](https://github.com/i2y/rulec) の表で書けます。表に抜けも重なりも無いことは rulec が証明し、表に書いたステートマシンは、ワークフローが状態を追いかける対象（ここでは Stripe の PaymentIntent）の型になります。

</div>
</div>

<div class="dd-row dd-row--flip" markdown>
<div markdown>

<div class="dd-term" markdown>

```text
エラー[E020]: tests/fixtures/hotel_naive.flow:95:1: 案件 `pi` が requires_payment_method, processing のまま、ここでワークフローが終わることがあります（終わりの状態は succeeded, canceled）
    95 |   succeed outcome = stayed
  そうなる例:
      81  quote = hold(…)
      84  match quote.handling: auto
      84  create_intent: pi が requires_confirmation で始まる
      85  confirm_intent: pi が requires_confirmation → requires_capture
      90  match pi.status: requires_capture
      90  booking.check_out まで待つ
      93  capture_intent: pi が requires_capture → processing
          外部のサービスで `settle` が起きる: pi processing → requires_payment_method
      95  succeed
```

</div>

</div>
<div markdown>

### ありうる流れを最後までたどる

ホテルの予約の最初の下書きは、チェックアウトの日まで待ってから支払いを確定します。検査は、ワークフローが何もしなくても Stripe の側で起きるものまで含めて、PaymentIntent の遷移をたどります。そして、支払いが確定も取り消しもされないまま終わる実行を見つけます。診断には、そこへ至る実行の例が付きます。[何を検査するか](checks.md)

</div>
</div>

<div class="dd-row" markdown>
<div markdown>

```text
dandori build hotel.flow --target temporal
dandori build hotel.flow --target temporal-python
dandori build hotel.flow --target asl
dandori build hotel.flow --target durable
dandori build hotel.flow --target argo
dandori build hotel.flow --target pydantic-graph
```

</div>
<div markdown>

### 一つの .flow から、使っているプラットフォーム向けに

主なプラットフォームは Temporal です。dandori は、ワークフローと、HTTP や AWS などの呼び出しを受け持つアクティビティ、ワーカー、クライアントを、TypeScript か Python で生成します。同じ `.flow` から AWS Step Functions、Lambda durable functions、Argo Workflows、pydantic-graph 向けにもビルドでき、生成したコードはどれも同じリクエストを送ります。プラットフォームにできないことは、ビルドが断ります。[プラットフォーム別のビルド](platforms.md)

</div>
</div>

<div class="dd-row dd-row--flip" markdown>
<div markdown>

```flow
task read_inquiry(text: string) -> Reading
  agent "Read the text of a customer's inquiry, choose its kind, …"
  model "gpt-oss:20b"
  effort low
  url "http://ollama.internal:11434/v1"
  timeout 60 seconds
  retry 2 times every 10 seconds
```

</div>
<div markdown>

### エージェントに読ませ、書かせ、選ばせる

タスクをエージェントにすることもできます。モデルはタスクの引数を受け取り、タスクの型の値を返します。その応答も、ほかのタスクの結果と同じく型で確かめます。応答はそのまま `match` で分岐に使うことも、規則に渡して判断させることもできます。OpenAI のモデル、Claude、Open Responses のエンドポイント（Ollama、vLLM、LM Studio、OpenRouter など）を呼べます。[エージェント](agents.md)

</div>
</div>

<div class="dd-row" markdown>
<div markdown>

```text
dandori doc hotel.flow > hotel.md
dandori doc hotel.flow --format html > hotel.html
```

</div>
<div markdown>

### レビューする人のために図にする

`dandori doc` はワークフローを図にします。呼び出し・`match`・待ち・ループのすべてと、各呼び出しのすること、エラーの行き先、呼び出しのあと案件がとりうる状態、ワークフローの終わり方のすべてが載ります。Markdown なら、GitHub がプルリクエストの中で描く Mermaid のフローチャートに、HTML なら、シナリオごとに実行の通るところが光るページ一枚になります。[ホテルの予約の図](doc/hotel.html) · [ワークフローを図にする](diagrams.md)

</div>
</div>

## いまの状況

まだ初期の段階です。次のものはまだありません。別々の処理を同時に走らせる Parallel、YAML で書いた OpenAPI の文書、protobuf のバイナリ形式と Connect のストリーム、ワークフロー自身が状態を持つ案件、規則の前提条件を値を作ったタスクの直後で確かめること。AWS の上や、本番構成の Temporal や Temporal Cloud の上ではまだ動かしておらず、Argo から caller のイメージで本物の Lambda・HTTP・AWS に送ることも、エージェントの呼び出しを本物の OpenAI や Anthropic に送ることもしていません。

設計の理由、決めたこと、残っていることは [DESIGN.md](https://github.com/i2y/dandori/blob/main/DESIGN.md) にあり、原則は [設計](design.md) にまとめています。ライセンスは Apache License 2.0 と MIT ライセンスのどちらかを選べます。
