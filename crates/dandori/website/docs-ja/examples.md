# 例で見る

例は五つあり、それぞれに Temporal 版、AWS 版、pydantic-graph 版の三つがあります。流れはどれも同じで、タスクの呼び出し方と、外からの通知の受け取り方だけを、プラットフォームのやり方に合わせています。dandori の主なプラットフォームは Temporal なので、最初に読むなら Temporal 版がおすすめです。

| 例 | Temporal 版 | AWS 版（Step Functions、Lambda durable functions） | pydantic-graph 版 |
|---|---|---|---|
| ホテルの予約。カードに与信を取り、チェックアウトの日に確定する。Stripe の OpenAPI の記述と照らし合わせる | [temporal](https://github.com/i2y/dandori/blob/main/examples/hotel/temporal/hotel.flow) | [aws](https://github.com/i2y/dandori/blob/main/examples/hotel/aws/hotel.flow) | [pydantic-graph](https://github.com/i2y/dandori/blob/main/examples/hotel/pydantic-graph/hotel.flow) |
| 倉庫のシステムの注文。催促し、出荷し、届ける | [temporal](https://github.com/i2y/dandori/blob/main/examples/order/temporal/order.flow) | [aws](https://github.com/i2y/dandori/blob/main/examples/order/aws/order.flow) | [pydantic-graph](https://github.com/i2y/dandori/blob/main/examples/order/pydantic-graph/order.flow) |
| 注文の明細を並べて引き当て、梱包し、配達する。倉庫は Connect で呼び、配達はすべてのプラットフォーム向けに一度だけ書いた子の `.flow`（[arrange_delivery](https://github.com/i2y/dandori/blob/main/examples/fulfillment/arrange_delivery.flow)） | [temporal](https://github.com/i2y/dandori/blob/main/examples/fulfillment/temporal/fulfillment.flow) | [aws](https://github.com/i2y/dandori/blob/main/examples/fulfillment/aws/fulfillment.flow) | [pydantic-graph](https://github.com/i2y/dandori/blob/main/examples/fulfillment/pydantic-graph/fulfillment.flow) |
| 問い合わせを読んで返事の下書きを書くエージェントと、振り分ける規則 | [temporal](https://github.com/i2y/dandori/blob/main/examples/inquiry/temporal/inquiry.flow) | [aws](https://github.com/i2y/dandori/blob/main/examples/inquiry/aws/inquiry.flow) | [pydantic-graph](https://github.com/i2y/dandori/blob/main/examples/inquiry/pydantic-graph/inquiry.flow) |
| 申し込みの審査。ほかのワーカーが点を付け、人が承認する。[Argo Workflows 版](https://github.com/i2y/dandori/blob/main/examples/review/argo/review.flow)もある | [temporal](https://github.com/i2y/dandori/blob/main/examples/review/temporal/review.flow) | [aws](https://github.com/i2y/dandori/blob/main/examples/review/aws/review.flow)（Lambda durable functions） | [pydantic-graph](https://github.com/i2y/dandori/blob/main/examples/review/pydantic-graph/review.flow) |

## 版ごとの違い

**Temporal 版**は、いちばん多くの機能を使っています。HTTP の API への呼び出しは生成したアクティビティが受け持ち（認証は `Transport` で付けるので、`connection` は要りません）、それ以外は自分で書くアクティビティです。ワークフローの外からの通知は `event` で受け取ります。通知は、ワークフローの ID を宛先にして送られてきます（ホテルでは Stripe の Webhook、注文では運送会社から）。応答が後から返ってくる呼び出しは `callback` のままで、応答は生成したクライアントから Update で送ります。ホテルと注文は、ワークフローがキャンセルされると、押さえていたものを手放します（`on cancel`）。引当と発送、審査は、ほかのタスクキューに仕事を送ります。引当と発送は、子の `.flow` が翌日便の車を取れなかったとき（子の `fail NoVan`。タスクはこのエラーを宣言しています）、通常便で頼み直します。問い合わせは規則をローカルアクティビティとして呼び、注文の催促のループは、履歴が伸びると新しい実行に引き継ぎます。

**AWS 版**では、タスクが呼ぶのは Lambda 関数、EventBridge の接続を通した HTTP の API、SNS と SQS で、コールバックにはタスクトークンを渡します。Step Functions でも Lambda durable functions でも、同じものが動きます。ほかのプラットフォーム向けに生成したコードも同じ呼び出しをするので、AWS 版は五つのプラットフォームすべてに向けてビルドできます。例外は審査です。審査のタスクはどれも自分で書くコードなので、Step Functions では動かせず、AWS では Lambda durable functions だけで動きます。

**Argo Workflows 版**の審査では、タスクはそれぞれ自分のイメージのコンテナで動きます（`image`）。ほかの例は、AWS 版がそのまま Argo でも動きます。そこでは、生成した caller のイメージが呼び出しを受け持ちます。

**pydantic-graph 版**では、グラフは入力を受け取った Python のプロセスの中で動きます。HTTP の API やエージェントへの呼び出しは生成した関数が受け持ち、規則も同じプロセスの中で動きます。それ以外は自分で書く関数です。コールバックへの応答も、同じプロセスの中で返します（`Deps.callbacks`）。待っているあいだも、プロセスは生きていなければなりません。プロセスが落ちれば、その実行も失われます。

各例のディレクトリのすぐ下には、それぞれの版のディレクトリと並べて、どのプラットフォームでもそのまま動くフローだけを置いています。引当と発送の子がそれで、呼び出しはどれも HTTP なので、dandori がどのプラットフォーム向けにも呼び出しのコードを生成できます。規則（`rules/`）と API の記述（`specs/`）は、版のあいだで共有しています。

## テスト用のフロー

[tests/flows](https://github.com/i2y/dandori/tree/main/tests/flows) のフローは、言語の細かいところまで試すためのものです。`json` に入れたリスト、並列の中の並列、あらゆる型の応答を返すエージェント、タイムアウトする呼び出し、ローカルの規則、Connect のゼロ値、イベントなどを扱います。名前はわざと日本語で書いてあり、ASCII でない名前が、五つのプラットフォームで識別子やキー、URL のパスとしてそのまま通ることを確かめています。
