# エージェント

`agent` のタスクは、モデルにタスクの引数を渡し、答えの型の値を受け取ります。答えは、ほかのタスクの答えと同じく `match` で分岐に使うことも、規則に渡すこともできます。問い合わせの例は、読むことと書くことをモデルに、振り分けを規則に任せています。
[examples/inquiry](https://github.com/i2y/dandori/blob/main/examples/inquiry/temporal/inquiry.flow)
は、お客さんからの問い合わせを、社内の Open Responses のエンドポイント（この例では Ollama）で動くモデルに読ませ、rulec の規則で振り分けて、返事の下書きを Claude に書かせます。

```flow
task read_inquiry(text: string) -> Reading
  agent "Read the text of a customer's inquiry, choose its kind, take out the order number if one is written, …"
  model "gpt-oss:20b"
  url "http://ollama.internal:11434/v1"
  timeout 60 seconds
  retry 2 times every 10 seconds

task draft_reply(kind: routing.kind, point: string, order_id: string?, within: duration[h]) -> string
  agent claude "Draft the first reply to the inquiry, politely, in three sentences at most. …"
  model "claude-sonnet-5"
  timeout 60 seconds

flow
  let reading = read_inquiry(text: inquiry.text)
    on failure => …
  let decision = routing(kind: reading.kind, member: inquiry.member)
```

## 答えには型がある

答えの型は JSON Schema になります。OpenAI の Structured Outputs の strict モードが受け付ける形で、レコードのフィールドはすべて必須、`T?` は `T` と null のどちらか、列挙は値の一覧です。一番外はオブジェクトでなければならないので、`{"answer": …}` で包みます。Claude の構造化出力にも同じ Schema を渡します。返ってきた答えは、ほかの答えと同じく型で確かめ、合わなければ呼び出しの失敗にします。

どのプラットフォームでも、モデルに送る中身は同じです。送るのは指示、JSON の文字列にした引数、Schema の三つだけで、温度などのモデルの設定は送りません。Agents SDK が既定の設定を足すことも止めています。

## どこへ送るか

**OpenAI** には、`url` を書かないときに送ります。Step Functions では、HTTP Task から Responses API に送ります。API キーは、タスクの `connection` に書いた EventBridge の接続に置きます。ほかのプラットフォーム向けに生成したコードは、`Transport` を通し、OpenAI の Agents SDK で呼びます。キーは `OPENAI_API_KEY` から読みます。自分で用意した実行の設定を渡すこともできます。

**Open Responses のエンドポイント**には、`url "<base>"` を書くと送ります。Open Responses は、OpenAI の Responses API をもとにしたオープンな仕様です。2026 年 1 月に、OpenAI、Hugging Face、OpenRouter、Ollama、vLLM、LM Studio、Vercel が採用しました。要求は、Step Functions が送るのと同じ形で、そのエンドポイントの `<base>/responses` へ送ります。生成したコードは、SDK を使わずに HTTP で直接送ります。SDK は仕様に無い項目まで送ることがあるからです。認証は `Transport` のヘッダで渡します。受け付ける Schema の制限はサーバーごとに違うので、答えの型を OpenAI の制限に照らして検査するのは、OpenAI に送るときだけです。

**Claude** には、`agent claude "…"` と書くと送ります。指示はシステムプロンプトに、引数の JSON の文字列はユーザーのメッセージに、Schema は `output_config.format` に入れ、`max_tokens` は 16000 にします。Step Functions では HTTP Task から送り、キーは接続に `x-api-key` として置きます。ほかのプラットフォームでは Anthropic の SDK を使い、キーは `ANTHROPIC_API_KEY` から読みます。Claude は、列挙の値の大文字と小文字を変えて返すことがあります。そこで、Claude のエージェントの答えにある列挙の値は、大文字と小文字を区別せずに受け取ります。そのため、大文字と小文字だけが違う値を持つ列挙は、この答えには使えません（E007）。

## 失敗とやり直し

エージェントは外部のデータを何も変えないので、`key` は要らず、やり直しはいつでも安全です。エラーも宣言しません。モデルが断ったときも、呼び出しが失敗したときも、`failure` になります。既定の `Transport` では、どちらの SDK のクライアントも自分ではやり直しません。やり直すのは、Step Functions と同じく、タスクの `retry` に従うワークフローのほうです。

検査が断る答えもあります。Schema で書けない答え（`json` や、ほかの型を経由して自分自身を含むレコード）と、提供元が受け付ける大きさを超える答え（Claude では、無いことがある値が 16 個を超えるもの）です（E007）。Step Functions 向けのビルドは、`connection` の無いエージェント、60 秒（HTTP Task が一回の要求を待てる長さ）を超える `timeout`、HTTPS でない送り先を断ります（E050）。HTTP Task が呼べるのは、公開のドメイン名と広く信頼された証明書を持つサーバーだけで、非公開の API でもこれは変わりません。EventBridge の接続には必ず認証の値が要るので、キーの要らないサーバーでも、何か入れておきます。
