# エージェント

`agent` のタスクは、モデルにタスクの引数を渡し、タスクに書いた型の値を応答として受け取ります。この応答は、ほかのタスクの結果と同じく `match` で分岐に使うことも、規則に渡すこともできます。問い合わせの例は、読むことと書くことをモデルに、振り分けを規則に任せています。
[examples/inquiry](https://github.com/i2y/dandori/blob/main/examples/inquiry/temporal/inquiry.flow)
は、お客さんからの問い合わせを、社内の Open Responses のエンドポイント（この例では Ollama）で動くモデルに読ませ、rulec の規則で振り分けて、返事の下書きを Claude に書かせます。振り分けに使う種類は [Jev](jev.md) が選んだもので、Jev が確信を持てないときだけモデルが読んだ種類を使います。

```flow
task read_inquiry(text: string) -> Reading
  agent "Read the text of a customer's inquiry, choose its kind, take out the order number if one is written, …"
  model "gpt-oss:20b"
  effort low
  url "http://ollama.internal:11434/v1"
  timeout 60 seconds
  retry 2 times every 10 seconds

task draft_reply(kind: routing.kind, point: string, order_id: string?, within: duration[h]) -> string
  agent claude "Draft the first reply to the inquiry, politely, in three sentences at most. …"
  model "claude-sonnet-5"
  effort medium
  timeout 60 seconds

flow
  let reading = read_inquiry(text: inquiry.text)
    on failure => …
  let kind = pick_kind(text: inquiry.text)
    on unsure, failure => let kind = reading.kind
  let decision = routing(kind: kind, member: inquiry.member)
```

## 応答には型がある

応答の型は JSON Schema になります。OpenAI の Structured Outputs の strict モードが受け付ける形で、レコードのフィールドはすべて必須、`T?` は `T` と null のどちらか、列挙は値の一覧です。一番外はオブジェクトでなければならないので、`{"answer": …}` で包みます。Claude の構造化出力にも同じ Schema を渡します。返ってきた応答は、ほかのタスクの結果と同じく型で確かめ、合わなければ呼び出しの失敗にします。

どのプラットフォームでも、モデルに送る中身は同じです。送るのは指示、JSON の文字列にした引数、Schema の三つと、タスクに書いたときだけのエフォート（次の節）です。温度などのほかのモデルの設定は送らず、Agents SDK が既定の設定を足すことも止めています。

## エフォート

`effort <レベル>` を書くと、モデルが応答する前にどれだけ推論するかを指定できます。書かなければ、モデルの既定のままです。レベルはそれぞれの API が受け付ける場所に入れ、どのプラットフォームでも同じように送ります。

| エージェント | レベルを入れる場所 | レベル |
|---|---|---|
| OpenAI と、Open Responses のエンドポイント（`url`） | Responses API の `reasoning.effort` | `none`・`minimal`・`low`・`medium`・`high`・`xhigh`・`max` |
| Claude | Messages API の `output_config.effort` | `low`・`medium`・`high`・`xhigh`・`max` |

プロバイダーが受け付けないレベルは、検査がエラーにします（E007）。そもそも推論するかどうかはモデル次第で、推論しないモデルにエフォートを送ると、その呼び出しは失敗します。たとえば Ollama は、そうしたモデルへのエフォートを「does not support thinking」として拒否します。その呼び出しは、ほかの失敗した呼び出しと同じく `failure` になります。

## どこへ送るか

**OpenAI** には、`url` を書かないときに送ります。Step Functions では、HTTP Task から Responses API に送ります。API キーは、タスクの `connection` に書いた EventBridge の接続に置きます。ほかのプラットフォーム向けに生成したコードは、`Transport` を通し、OpenAI の Agents SDK で呼びます。キーは `OPENAI_API_KEY` から読みます。自分で用意した実行の設定を渡すこともできます。OpenAI には Go の Agents SDK が無いので、Go 版は OpenAI の Go のクライアントで、Step Functions と同じリクエストを Responses API に送ります。

**Open Responses のエンドポイント**には、`url "<base>"` を書くと送ります。Open Responses は、OpenAI の Responses API をもとにしたオープンな仕様です。2026 年 1 月に、OpenAI、Hugging Face、OpenRouter、Ollama、vLLM、LM Studio、Vercel が採用しました。リクエストは、Step Functions が送るのと同じ形で、そのエンドポイントの `<base>/responses` へ送ります。生成したコードは、SDK を使わずに HTTP で直接送ります。SDK は仕様に無い項目まで送ることがあるからです。認証は `Transport` のヘッダで渡します。受け付ける Schema の制限はサーバーごとに違うので、応答の型を OpenAI の制限に照らして検査するのは、OpenAI に送るときだけです。

**Claude** には、`agent claude "…"` と書くと送ります。指示はシステムプロンプトに、引数の JSON の文字列はユーザーのメッセージに、Schema は `output_config.format` に入れ、`max_tokens` は 16000 にします。Step Functions では HTTP Task から送り、キーは接続に `x-api-key` として置きます。ほかのプラットフォームでは Anthropic の SDK を使い、キーは `ANTHROPIC_API_KEY` から読みます。Claude は、列挙の値の大文字と小文字を変えて返すことがあります。そこで、Claude のエージェントの応答にある列挙の値は、大文字と小文字を区別せずに受け取ります。そのため、大文字と小文字だけが違う値を持つ列挙は、この応答には使えません（E007）。

## 失敗とリトライ

エージェントは外部のデータを何も変えないので、`key` は要らず、リトライはいつでも安全です。エラーも宣言しません。モデルが応答を拒否したときも、呼び出しが失敗したときも、`failure` になります。既定の `Transport` では、どちらの SDK のクライアントも自分ではリトライしません。リトライするのは、Step Functions と同じく、タスクの `retry` に従うワークフローのほうです。

検査がエラーにする応答の型もあります。Schema で書けない型（`json` や、ほかの型を経由して自分自身を含むレコード）と、プロバイダーが受け付ける大きさを超える型（Claude では、オプショナルな値が 16 個を超えるもの）です（E007）。Step Functions 向けのビルドは、`connection` の無いエージェント、60 秒（HTTP Task が一回のリクエストを待てる長さ）を超える `timeout`、HTTPS でない送信先をエラーにします（E050）。HTTP Task が呼べるのは、公開のドメイン名と広く信頼された証明書を持つサーバーだけで、非公開の API でもこれは変わりません。EventBridge の接続には必ず認証の値が要るので、キーの要らないサーバーでも、何か入れておきます。
