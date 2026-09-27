# 例で見る

例は五つあり、それぞれに Temporal 向け、AWS 向け、pydantic-graph 向けの三つの版があります。流れはどの版も同じで、タスクの呼び方と、外からの知らせの受け取り方だけを、プラットフォームのやり方に合わせています。dandori の主なプラットフォームは Temporal なので、最初に読むなら Temporal 向けの版がおすすめです。

| 例 | Temporal 向け | AWS 向け（Step Functions、Lambda durable functions） | pydantic-graph 向け |
|---|---|---|---|
| ホテルの予約。カードに与信を取り、チェックアウトの日に確定する。Stripe の OpenAPI の記述と照らし合わせる | [temporal](https://github.com/i2y/dandori/blob/main/examples/hotel/temporal/hotel.flow) | [aws](https://github.com/i2y/dandori/blob/main/examples/hotel/aws/hotel.flow) | [pydantic-graph](https://github.com/i2y/dandori/blob/main/examples/hotel/pydantic-graph/hotel.flow) |
| 倉庫のシステムの注文。催促し、出荷し、届ける | [temporal](https://github.com/i2y/dandori/blob/main/examples/order/temporal/order.flow) | [aws](https://github.com/i2y/dandori/blob/main/examples/order/aws/order.flow) | [pydantic-graph](https://github.com/i2y/dandori/blob/main/examples/order/pydantic-graph/order.flow) |
| 注文の明細を並べて引き当て、梱包し、配達する。倉庫は Connect で呼び、配達はすべてのプラットフォーム向けに一度だけ書いた子の `.flow`（[arrange_delivery](https://github.com/i2y/dandori/blob/main/examples/fulfillment/arrange_delivery.flow)） | [temporal](https://github.com/i2y/dandori/blob/main/examples/fulfillment/temporal/fulfillment.flow) | [aws](https://github.com/i2y/dandori/blob/main/examples/fulfillment/aws/fulfillment.flow) | [pydantic-graph](https://github.com/i2y/dandori/blob/main/examples/fulfillment/pydantic-graph/fulfillment.flow) |
| 問い合わせを読んで返事の下書きを書くエージェントと、振り分ける規則 | [temporal](https://github.com/i2y/dandori/blob/main/examples/inquiry/temporal/inquiry.flow) | [aws](https://github.com/i2y/dandori/blob/main/examples/inquiry/aws/inquiry.flow) | [pydantic-graph](https://github.com/i2y/dandori/blob/main/examples/inquiry/pydantic-graph/inquiry.flow) |
| 申し込みの審査。ほかのワーカーが点を付け、人が承認する。[Argo Workflows 向け](https://github.com/i2y/dandori/blob/main/examples/review/argo/review.flow)の版もある | [temporal](https://github.com/i2y/dandori/blob/main/examples/review/temporal/review.flow) | [aws](https://github.com/i2y/dandori/blob/main/examples/review/aws/review.flow)（Lambda durable functions） | [pydantic-graph](https://github.com/i2y/dandori/blob/main/examples/review/pydantic-graph/review.flow) |

## 版ごとの違い

**Temporal 向け**の版は、いちばん多くの機能を使っています。HTTP の API への呼び出しは生成したアクティビティが受け持ち（認証は `Transport` で付けるので、`connection` は要りません）、それ以外は自分で書くアクティビティです。ワークフローの外からの知らせは `event` で受け取ります。知らせは、ワークフローの ID を宛先にして送られてきます（ホテルでは Stripe の Webhook、注文では運送会社から）。答えが後から返ってくる呼び出しは `callback` のままで、答えは生成したクライアントから Update で送ります。ホテルと注文は、ワークフローがキャンセルされると、押さえていたものを手放します（`on cancel`）。引当と発送、審査は、ほかのタスクキューに仕事を送ります。引当と発送は、子の `.flow` が翌日便の車を取れなかったとき（子の `fail NoVan`。タスクはこのエラーを宣言しています）、通常便で頼み直します。問い合わせは規則をローカルアクティビティとして呼び、注文の催促のループは、履歴が伸びると新しい実行に引き継ぎます。

**AWS 向け**の版では、タスクが呼ぶのは Lambda 関数、EventBridge の接続を通した HTTP の API、SNS と SQS で、コールバックにはタスクトークンを渡します。Step Functions でも Lambda durable functions でも、同じ版が動きます。ほかのプラットフォーム向けに生成したコードも同じ呼び出しをするので、この版は五つのプラットフォームすべてに向けてビルドできます。例外は審査です。審査のタスクはどれも自分で書くコードなので、Step Functions では動かせず、AWS では Lambda durable functions だけで動きます。

**Argo Workflows 向け**の審査では、タスクはそれぞれ自分のイメージのコンテナで動きます（`image`）。ほかの例は、AWS 向けの版がそのまま Argo でも動きます。そこでは、生成した caller のイメージが呼び出しを受け持ちます。

**pydantic-graph 向け**の版では、グラフは入力を受け取った Python のプロセスの中で動きます。HTTP の API やエージェントへの呼び出しは生成した関数が受け持ち、規則も同じプロセスの中で動きます。それ以外は自分で書く関数です。コールバックにも同じプロセスの中で答えます（`Deps.callbacks`）。待っているあいだも、プロセスは生きていなければなりません。プロセスが落ちれば、その実行も失われます。

各例のディレクトリのすぐ下には、版のディレクトリと並べて、どのプラットフォームでもそのまま動くフローだけを置いています。引当と発送の子がそれで、呼び出しはどれも HTTP なので、dandori がどのプラットフォーム向けにも呼び出しのコードを生成できます。規則（`rules/`）と API の記述（`specs/`）は、版のあいだで共有しています。

## テスト用のフロー

[tests/flows](https://github.com/i2y/dandori/tree/main/tests/flows) のフローは、言語の細かいところまで試すためのものです。`json` に入れたリスト、並列の中の並列、あらゆる型の答えを返すエージェント、期限切れになる呼び出し、ローカルの規則、Connect のゼロ値、出来事などを扱います。名前はわざと日本語で書いてあり、ASCII でない名前が、五つのプラットフォームで識別子やキー、URL のパスとしてそのまま通ることを確かめています。
