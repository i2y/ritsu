# 秘密の値

ワークフローは、呼び出しから呼び出しへ値を運びます。pydantic-graph のほかのプラットフォームは、その値を実行の履歴に残し、実行を読める人はだれでもそれを読めます。`dandori check` は、契約が秘密と言う値が履歴に残るとき、タスクがそれをプロジェクトの外の相手に送るとき、鍵が `.flow` に書いてあるとき、呼び出しが暗号化しない HTTP で送るときに、そう言います。どれも `.flow` だけで決まるので、`dandori check` を単体で使っても出ます。`ritsu check` は dandori の出したとおりに並べます。

| コード | 言うこと | 意図を書く書き方 |
|---|---|---|
| W901 | プロバイダーが決めた形の鍵が、`.flow` に書いてある | 同じ行のコメントに `# ritsu: test secret` |
| W902 | タスクが、ほかのマシンの URL を暗号化しない HTTP で呼ぶ | URL を書いたところに `plaintext "<理由>"` |
| W904 | 秘密の値が、ワークフローの履歴に残る | `workflow` の下に `history encrypted` |
| E906 | タスクが、秘密の値をプロジェクトの外に送る | タスクの下に `discloses <引数> "<理由>"` |

## 何が秘密か

秘密の印が付いたところから読んだ値が、秘密の値です。印の付け方は三つあります。

- **`.flow` の `secret`**：入力、出力、フィールド、タスクの引数と結果の型のあとに書きます（`range` があれば、そのあと）。

  ```flow
  record 連絡先
    名前 : string
    電話 : string  secret

  inputs
    顧客番号       : string
    APIトークン    : string  secret

  task トークンを出す(利用者: string) -> string  secret
  ```

- **`.proto` の `debug_redact`**：フィールドに直に書くか、`debug_redact` を付けた列挙の値を、フィールドのカスタムのオプションで付けます。protobuf のデバッグの出力が伏せるのと同じ印です。メッセージから作ったレコード（`銀行.Account`）のフィールドと、`connect` のタスクの引数と結果に効きます。

  ```proto
  message Account {
    string id = 1;
    string number = 2 [debug_redact = true];
    string holder = 3 [(acme.v1.sensitivity) = PERSONAL];
  }

  enum Sensitivity {
    SENSITIVITY_UNSPECIFIED = 0;
    PUBLIC = 1;
    PERSONAL = 2 [debug_redact = true];
  }
  ```

- **OpenAPI のスキーマ**：プロパティに、`confidential` か `restricted` の `x-data-classification`（`sensitivity` を書かなければ `confidential`）、`x-sensitive-data`、`format: password` のどれかがあるもの。操作と突き合わせるタスクの引数と結果に効きます。`writeOnly` だけのプロパティと、`internal` は印にしません。

レコードはフィールドの秘密を、リストは項目の秘密を持ちます。値を埋め込んだ文字列は、埋め込んだ値のどれかが秘密なら秘密です。秘密の値から作った `json` の値は、全体が秘密になります。規則の結果は判断なので、秘密を持ちません。印の無いタスクの結果も同じです。変数が何を持つかは、[範囲](tour.md#範囲)と同じく、変数に値を入れるすべての場所を合わせて決めます。

## 履歴に残る（W904）

Temporal、Step Functions、Lambda durable functions、Argo Workflows は、ワークフローとすべての呼び出しの入力と出力を残します。そのため W904 は、秘密の値が入力か出力、フローが呼ぶタスクの結果、呼び出しの引数、`fail` の理由になるところに出ます。次の例では、銀行の契約が口座の番号に印を付けていて、フローがそれを送金の摘要に書いています（[tests/fixtures/security/W904_送金の口座番号.flow](https://github.com/i2y/ritsu/blob/main/crates/dandori/tests/fixtures/security/W904_送金の口座番号.flow)）。

<div class="dd-term" markdown>

```text
警告[W904]: tests/fixtures/security/W904_送金の口座番号.flow:39:1: 秘密の値 `口座.番号` が、`送金する` の引数 `摘要` として、ワークフローの履歴に残ります
    39 |   let 送金 = 送金する(口座ID: 口座ID, 金額: 金額, 摘要: "{口座.番号} への売上の支払い")
  = 印は ../../../examples/payout/specs/payout.ja.proto:36 の `debug_redact = true` です。
  = Temporal、Step Functions、Lambda durable functions、Argo Workflows は、ワークフローとすべての呼び出しの入力と出力を履歴に残し、実行を読める人はだれでもそれを読めます。
  = 値の代わりに参照（ID やシークレットの名前）を渡し、タスクの中で値を取ってきてください。履歴を自分の持つ鍵で暗号化しているなら（Temporal はペイロードのコーデック、Step Functions と Lambda durable functions はカスタマー管理の KMS キー）、`workflow` の下に `history encrypted` と書いてください。
```

</div>

直し方は、値の代わりに参照を運ぶことです。支払いの例（[examples/payout](https://github.com/i2y/ritsu/blob/main/crates/dandori/examples/payout/payout.ja.flow)）は、銀行に口座の ID を渡すので、何も出ません。履歴を、秘密を読んでよい人だけが持つ鍵で暗号化しているなら、`workflow` の下にそう書きます。

```flow
workflow 支払い v1
  history encrypted
```

| プラットフォーム | `history encrypted` で変わること |
|---|---|
| Temporal（TypeScript・Python・Go） | ワーカーとクライアントが、ペイロードのコーデックを型で受け取ります。TypeScript は `makeWorker(own, { codec })` と `encryptedClient(connection, codec)`、Python は `connect(target, codec)`、Go は `Dial(options, codec)` です。コーデックを渡さずに作ったクライアントを、生成したクライアントの関数やワーカーに渡すと、型の検査が通りません。失敗の文とスタックトレースも、コーデックを通します |
| Step Functions、Lambda durable functions | dandori が書くものは変わりません。鍵は、ステートマシン（`EncryptionConfiguration`）や関数（`DurableConfig.KMSKeyArn`）に設定するカスタマー管理の KMS キーで、dandori からは見えません。鍵を設定すると、履歴を読むのに `kms:Decrypt` が要ります |
| Argo Workflows | E050 です。Workflow はパラメーターをそのまま持ち、それを暗号化する手段がありません |
| pydantic-graph | 何も変わりません。履歴を残さないからです |

## プロジェクトの外へ送る（E906）

モデルのプロバイダー（OpenAI、Anthropic、このマシンでない Open Responses のサーバー）、Jev、URL だけで書いた `http` の送り先、AWS のサービスは、プロジェクトの外です。タスクがそこへ秘密の値を送るとエラーになります。意図しているなら、タスクにそう書きます。履歴には残るので、W904 は出たままです。

```flow
task 知らせを書く(金額: money[円, incl_tax], 送金ID: string, 名義: string) -> string
  agent "売り手に、売上の送金を済ませたことを知らせる短い文面を下書きしてください。…"
  model "gpt-5.4-mini"
  discloses 名義 "知らせは口座の名義で売り手に呼びかける。プロバイダーとの取り決めで、データは残らない"
```

プロジェクトの中のほかのファイル（OpenAPI の文書、`.proto`、Connect の規則、子の `.flow`、帳簿、日付のファイル）へ送る秘密の値は、`ritsu check` がコンテキストの地図と照らし合わせます。dandori は、どの呼び出しがそうしているかを伝えます。

## 暗号化しない HTTP（W902）

タスクが呼ぶ URL は、`http` のタスクの URL、エージェントの `url`、`use rule … connect`、`use openapi` と `use proto` の下の URL（無ければ OpenAPI の文書の最初のサーバー）です。それが `http://` で、送り先がこのマシン（`localhost`、`127.0.0.1`、`::1`）でなければ、警告になります。

<div class="dd-term" markdown>

```text
警告[W902]: tests/fixtures/security/W902_エージェントの_URL.flow:7:3: エージェント `読む` は、ollama.internal に暗号化しない HTTP でリクエストを送ります
     7 |   url "http://ollama.internal:11434/v1"
  = 途中のネットワークにいる人は、リクエストとレスポンスと、ヘッダーの鍵を読んだり書き換えたりできます。
  = https:// にしてください。ほかの仕組み（サービスメッシュ、プライベートな接続など）で守っているなら、タスクの下に `plaintext "<理由>"` と書いてください。
```

</div>

問い合わせの例の Temporal 版は、Open Responses のエンドポイント（この例では Ollama）にクラスターの中で届くので、そう書いてあります。

```flow
task 読み取る(本文: string) -> 読み取り
  agent "お客さまからの問い合わせの本文を読み、種類を一つ選び、…"
  model "gpt-oss:20b"
  effort low
  url "http://ollama.internal:11434/v1"
  plaintext "モデルのサーバーにはクラスターのネットワークの中だけで届き、そこはサービスメッシュが暗号化する"
```

`plaintext` は URL を書いたところに書きます。タスクの下、`use rule … connect` の下、API の `use` の下のどれかです。Step Functions は HTTPS しか呼べないので、Step Functions 向けのビルドは、`plaintext` があっても `http://` を E050 にします。

## ファイルの鍵（W901）

AWS のアクセスキー ID、GitHub・Slack・Stripe・OpenAI・Anthropic・Google の鍵やトークン、Slack の Incoming Webhook の URL、PEM の秘密鍵の形をした値が、`.flow` のどこか（文字列でもコメントでも）に書いてあると、警告になります。メッセージと、引いて見せる行には、鍵の接頭辞と長さだけを出し、鍵そのものは出しません。ビルドのログにも残りません。

<div class="dd-term" markdown>

```text
警告[W901]: tests/fixtures/security/W901_文字列の鍵.flow:5:50: Google の API キーがここに書かれています（AIza…、39 文字）
     5 |   http GET "https://maps.example.com/v1/find?key=AIza…"
  = ファイルに書いた鍵は、リポジトリとその履歴とビルドを読めるすべての人に渡ります。鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。
  = 本物の鍵なら、まず Google で無効にしてください。ファイルから消しても、リポジトリの履歴には残ります。
  = テスト用の値なら、同じ行のコメントに `ritsu: test secret` と書いてください。
```

</div>

ritsu のどの言語も、同じ形を同じやり方で探します。
