# 判断のモデル

判断のモデルは文章を書きません。渡されたものについての型の付いた質問に、どれだけ確かかを添えて答えます。`jev` のタスクは、タスクの結果の型が求める質問を一回のリクエストで判断のモデルに尋ね、その型の値を受け取ります。この結果は、ほかのタスクの結果と同じく `match` で分岐に使うことも、規則に渡すこともできます。確信度が足りない答えは、タスクが名前を付けたエラーで呼び出しを失敗させるので、フローはそのエラーを呼び出したところで処理します。

`jev` のタスクが尋ねる先は、次の三種類のサーバーです。

| タスクの書き方 | 尋ねる先 | 送り先 | キー |
|---|---|---|---|
| `jev "<質問>"` | TypeSafe AI の判断のモデル [Jev](https://docs.typesafe.ai/introduction)（System One の API） | `https://api.typesafe.ai/v1/systemone` | TypeSafe のキー（`TYPESAFE_API_KEY`） |
| `jev "<質問>"` と `url "<ベース URL>"` | System One の API を話すほかのサーバー。たとえば、自分の判断のモデルを動かす [Ollama](https://docs.ollama.com/api/systemone)（0.35 から） | `<ベース URL>/systemone` | なし。キーが要るサーバーには `Transport` のヘッダで渡す |
| `jev openai "<質問>"` | [OpenAI の Decisions API](https://developers.openai.com/api/docs/guides/decisions) | `https://api.openai.com/v1/decisions` | OpenAI のキー（`OPENAI_API_KEY`） |
| `jev openai "<質問>"` と `url "<ベース URL>"` | Decisions API を話すほかのサーバー（ゲートウェイなど） | `<ベース URL>/decisions` | なし（上と同じ） |

ベース URL には、エージェントの `url` と同じく、API のパスの手前までを書きます（Ollama なら `http://localhost:11434/v1`）。`jev typesafe "…"` は `jev "…"` と同じです。どのプラットフォームも、同じ送り先に同じリクエストを送り、答えを同じように読みます。

問い合わせの例の AWS 向けの版は、問い合わせの種類を Jev に選ばせ、Jev が確信を持てないときは、エージェントがほかのことと一緒に読んだ種類を使います
（[examples/inquiry](https://github.com/i2y/ritsu/blob/main/crates/dandori/examples/inquiry/aws/inquiry.ja.flow)）。

```flow
task 種類を選ぶ(本文: string) -> 振り分け.種類
  jev "この問い合わせの種類はどれか"
    返品 "商品を返したい、交換したい"
    配送 "荷物が遅れている、届かない、壊れていた、どこにあるか知りたい"
    請求 "請求、支払い、返金のこと"
    その他 "上のどれでもない"
  model "jev-1.13.0"
  confidence 0.8 else 迷い
  timeout 10 seconds
  retry 2 times every 1 second

flow
  …
  let 種類 = 種類を選ぶ(本文: 問い合わせ.本文)
    on 迷い, failure => let 種類 = 読み.種類
  let 判定 = 振り分け(種類: 種類, 会員: 問い合わせ.会員)
```

Temporal 向けの版は、同じ質問を、会社が自分で動かしている System One の API のサーバー（この例では Ollama）の判断のモデルに尋ねます。キーは要りません
（[examples/inquiry](https://github.com/i2y/ritsu/blob/main/crates/dandori/examples/inquiry/temporal/inquiry.ja.flow)）。

```flow
task 種類を選ぶ(本文: string) -> 振り分け.種類
  jev "この問い合わせの種類はどれか"
    …
  model "nimble:9b-q4_K_M"
  url "http://ollama.internal:11434/v1"
  plaintext "モデルのサーバーにはクラスターのネットワークの中だけで届き、そこはサービスメッシュが暗号化する"
  confidence 0.8 else 迷い
```

## 結果の型が質問になる

| タスクの結果 | 判断のモデルに尋ねること | 結果になる値 |
|---|---|---|
| 列挙で、`jev "<質問>"` | 列挙の値からの選択（choice）。下に、値ごとの意味を書く。書かなかった値は意味なしで送る | モデルが選んだ値 |
| 列挙で、`jev score "<質問>"` | 段階（score）。列挙の値を低い段階から順に並べ、すべてに意味を書く（TypeSafe の API では 2 から 10 段階） | モデルの答えにいちばん近い段階。ちょうど真ん中なら上の段階 |
| `bool` で、`jev "<質問>"` | はいかいいえ。下に `true` と `false` の意味を、両方書くか、どちらも書かない（Decisions API は意味を受け取らないので、質問の中に書く） | はいの確率が半分を超えれば true |
| レコードで、`jev` だけ | フィールドごとに上のどれかを尋ねる。すべて一回のリクエストで尋ねる | 答えを並べたレコード |

引数は、引数ごとに一つのフィールドにして、判断のモデルに読ませます。System One の API には入力（state）として、Decisions API には入力（input）の JSON の文字列として渡します。この文字列は、エージェントに渡すものと同じです。モデルは、選択の値を名前と意味で見分け、score の段階を書いた意味で見分けます。意味は「どれくらい」ではなく、モデルが当てはめられる具体的な場面として書きます。Decisions API では、score の段階のラベルに列挙の値を、説明に段階の意味を入れます。

レコードのフィールドには、ほかのフィールドの答えの確信度を率で受け取らせることもできます。審査の例は、申込がどの段階に当たるかを Jev に答えさせ、その確信度を規則に渡します
（[examples/review](https://github.com/i2y/ritsu/blob/main/crates/dandori/examples/review/temporal/review.ja.flow)）。

```flow
record 採点
  判断   : 方針.判断
  確信度 : rate[step 1%]  range >=0 <=100

task 採点する(用途: string) -> 採点
  jev
    判断 score "資金は、どれだけはっきり事業に使われるか"
      却下 "私用や投機、法に反することに使う"
      保留 "事業に使うが、何に払うのかがはっきりしない"
      承認 "仕入れ、設備、人件費、店舗など、事業のはっきりした費目に使う"
    確信度 confidence of 判断
  model "jev-1.13.0"
```

`rate[step 1%]` は 1% 刻みの個数で、端数は切り捨てます。確信度 0.87 なら 87 です。rulec の率も同じ形でやりとりするので、この値はそのまま規則に渡せます。

判断のモデルは文章を書かないので、それ以外の型の結果はエラーにします
（[tests/fixtures/jev.flow](https://github.com/i2y/ritsu/blob/main/crates/dandori/tests/fixtures/jev.flow)）。

<div class="dd-term" markdown>

```text
エラー[E007]: tests/fixtures/jev.flow:38:3: Jev が答えるのは、列挙の値のどれか、列挙の値を低いものから並べた段階のどこか（`score`）、はいかいいえ（`bool`）と、それらを並べたレコードです。文章は書きません。結果は `string` です
    38 |   jev "要点は何か"
```

</div>

## モデルが答えないとき

Decisions API は、質問に答えないことがあります。その質問の答えは `{"type": "refusal", "name": …}` になり、同じリクエストのほかの質問には答えが返ることもあります。`refusal else <エラー>` と書くと、答えない質問があった呼び出しは、そのエラーで失敗します。このエラーはこの行で宣言したことになり、フローは呼び出したところで処理します。書かなければ、呼び出しは `Dandori.Refused` で失敗し、`on failure` が受けます。答えない質問は、確信度より先に確かめます。同じ入力で尋ね直しても答えないことが多いので、リトライはしません
（[tests/flows/decisions.ja.flow](https://github.com/i2y/ritsu/blob/main/crates/dandori/tests/flows/decisions.ja.flow)）。

```flow
task 読む(本文: string) -> 読み
  jev openai
    種類 "この問い合わせの種類はどれか"
      返品 "商品を返したい、交換したい"
      請求 "請求や支払いのこと"
    急ぎ score "お客さまはどれほど急いでいるか"
      低 "急ぎを伝えていない"
      中 "期日を挙げているが、差し迫ってはいない"
      高 "今日中の対応を求めている"
    人 "お客さまは人の担当者と話したいと言っているか"
    確かさ confidence of 種類
  model "gpt-6-luna"
  confidence 0.6 else 自信なし
  refusal else 答えない
  connection "arn:aws:events:ap-northeast-1:123456789012:connection/openai/9c0d1e2f"

flow
  …
  let 読み = 読む(本文: 本文)
    on 答えない => fail 読み取れない "モデルが問い合わせを読みませんでした"
```

System One の API はどの質問にも答えるので、そこに `refusal` を書くとエラーにします。

<div class="dd-term" markdown>

```text
エラー[E007]: tests/fixtures/decisions.flow:18:16: System One の API（TypeSafe の Jev や、`url` のサーバー）は、どの質問にも答えを返し、答えないことはありません。`refusal` は `jev openai` のタスクに書くところです
    18 |   refusal else declined
```

</div>

## どれだけ確かなら使うか

選択と段階の答えには、モデルがどれだけ確かかを表す確信度（`confidence`、0 から 1）が付きます。はいかいいえの答えにはこの数が無いので、dandori は、選んだ答えの確率をそのまま確信度とみなします。はいの確率が 0.9 なら「はい」で確信度 0.9、0.3 なら「いいえ」で確信度 0.7 です。

`confidence 0.8 else 迷い` と書くと、確信度が 0.8 に届かない答えが一つでもある呼び出しは、エラー `迷い` で失敗します。このエラーはこの行で宣言したことになります。フローは、ほかの宣言したエラーと同じく、呼び出したところで処理し、検査もそれを確かめます。尋ね直してもモデルはほぼ同じように答えるので、`retry` の対象にはなりません。

確信度の意味は、サーバーごとに違います。TypeSafe は確率の散らばり方から計算します。Ollama の確信度は、一つの候補がほかよりどれだけ抜きん出ているかで、答えが正しい確率ではないと文書に書かれています。OpenAI は計算の仕方を書いていません。問い合わせの例の四つの種類を、同じ文について尋ねると（2026 年 10 月 9 日）、次のようになりました。

| 問い合わせ | Ollama の `tev1:0.8b` | OpenAI の `gpt-6-luna` |
|---|---|---|
| "My parcel arrived broken and I want my money back." | `returns` 0.59、`billing` 0.31。確信度 0.30 | `delivery` 0.51、`billing` 0.45。確信度 0.35 |
| 「靴のサイズが合わないので、代金を返してほしいです」 | `returns` 0.68。確信度 0.48 | `billing` 0.85。確信度 0.80 |

ですから、確信度の下限は、サーバーとモデルの組に結び付いた値です。答えを使ってよい確信度が、その答えで何をするかによって違うときは、確信度を規則に渡し、規則の表で決めます。尋ねるサーバーやモデルを替えたら、実際の答えで表を合わせ直します。審査の例の規則は、90% 以上ならそのまま承認し、80% 以上ならそのまま却下し、それ以外は人に回します。表に抜けがないことは、rulec が確かめます。

```flow
  let 結果 = 採点する(用途: 申込.用途)
    on failure => fail 採点不能 "申込 {申込.id} を採点できませんでした"
  let 判定 = 方針(判断: 結果.判断, 確信度: 結果.確信度)
  match 判定.決定
    承認, 却下 => pass
    人に回す =>
```

```text
table 決め方(act)
policy unique
| 判断 | 確信度 | -> 決定  |
| 承認 | >=90%  | 承認     |
| 承認 | <90%   | 人に回す |
| 却下 | >=80%  | 却下     |
| 却下 | <80%   | 人に回す |
| 保留 | -      | 人に回す |
```

同じ理由で、確信度の下限を書くタスクや確信度を受け取るタスクには、合わせたモデルを書きます。何もしなくても別のモデルに移る名前は、検査が警告します。Jev のエイリアス（`jev-latest` と `jev-preview`）、System One の API のサーバーのタグの無い名前と `:latest`（Ollama の名前の付け方）、そして、モデルのバージョンを固定する名前の無い Decisions API です。

<div class="dd-term" markdown>

```text
警告[W032]: tests/fixtures/jev.flow:192:3: `jev-latest` はエイリアスで、ここを変えなくても Jev の新しいバージョンに移ります。確信度の意味はバージョンごとに違うので、確信度を合わせたバージョンを `model "jev-1.13.0"` のように書いてください
   192 |   model "jev-latest"
```

```text
警告[W032]: tests/fixtures/decisions.flow:108:3: `nimble` にはタグが無いので、サーバーがモデルを取り直すと、ここを変えなくても新しいモデルに移ります（Ollama の名前の付け方では `:latest` と同じです）。確信度の意味はモデルごとに違うので、確信度を合わせたモデルを `model "tev1:0.8b"` のようにタグまで書いてください
   108 |   model "nimble"
```

```text
警告[W032]: tests/fixtures/decisions.flow:120:3: OpenAI の Decisions API には、モデルのバージョンを固定する名前がありません（`gpt-6-luna`）。モデルが替わると、ここを変えなくても確信度の意味が変わることがあるので、確信度の下限と確信度を受け取るフィールドを、ときどき実際の答えで確かめ直してください
   120 |   model "gpt-6-luna"
```

</div>

## どこへ送るか

どのプラットフォームも、タスクのサーバーに同じリクエストを送り、答えを同じように読みます。

- Step Functions では、HTTP Task から送ります。キーは、タスクの `connection` に書いた EventBridge の接続に、ヘッダ `Authorization: Bearer <キー>` として置きます。キーの要らないサーバーでも、接続には何かの値を入れます。HTTP Task は HTTPS の API しか呼べず、一回のリクエストを待てるのは 60 秒までなので、`http://` のサーバーと、それを超える `timeout` はビルドできません（E050）。
- ほかのプラットフォーム向けに生成したコードは、`Transport` を通して送ります。TypeSafe のキーは `TYPESAFE_API_KEY`（か `typesafe` オプション）から、OpenAI のキーは `OPENAI_API_KEY`（か `openai` オプション）から足します。`url` のサーバーには、どちらのキーも送りません。
- ほかのホストの `http://` のサーバーは、エージェントと同じく警告します（W902）。意図してそうしているなら、タスクの下に `plaintext "<理由>"` と書きます。秘密の値を TypeSafe、OpenAI、ほかのホストの `url` のサーバーに尋ねるタスクに渡すと、プロジェクトの外に送ることになります（E906）。

どの API も、エラーを HTTP ステータスで伝えます。レート制限に達すると 429、TypeSafe の API では混み合っていると 529 です。名前で処理したりリトライしたりしたいものは、`errors 混雑 = 429, 過負荷 = 529` のように宣言します。宣言していないステータスは `failure` になります。答えが無いときや、答えが質問に合わないとき（列挙に無い値や、確信度の無い選択）は、結果が型に合わないので、ほかのタスクの型に合わない結果と同じく実行を終えます。

## 使う前に

- Jev は 2026 年 9 月の時点で早期アクセス、OpenAI の Decisions API は 2026 年 10 月の時点で公開ベータで、どちらの API もまだ変わるかもしれません。OpenAI の API のモデルは `gpt-6-luna` だけで、上限とエラーは文書に書かれていません。
- TypeSafe によれば、Jev がいちばん正確なのは英語です。日本語などほかの言語も扱えますが、精度は落ちます。どの判断のモデルも、自分の文章で試してください。2026 年 9 月 29 日に jev-1.13.0 で試したときは、種類のはっきりした問い合わせは、英語でも日本語でも確信度 0.99 以上で正しい種類になりました。一方、「靴のサイズが合わないので、代金を返してほしいです」は `請求` で確信度 0.36（`返品` が 0.48）でした。同じ内容の英語は `returns` で 0.96 です。`confidence 0.8` なら、この問い合わせはエージェントが読んだ種類に回ります。
- 判断のモデルは、計算、数を数えること、日付の比較が苦手です。それはコードや規則に任せ、モデルには判断だけを尋ねます。
- 画像は送りません。どちらの API も画像を受け取ります（OpenAI はユーザーのメッセージの中に、Ollama は Clef のモデルで）が、タスクの引数は文字列と値だけです。
- テストでは、どの呼び出しにも、それぞれの API リファレンスにある形の答えをテスト用の `Transport` が返し、既定の `Transport` が送るリクエストはこのマシンの中で確かめます。タスクを一つずつ本物にも送ります。`TYPESAFE_API_KEY` があれば TypeSafe に、`OPENAI_API_KEY` があれば OpenAI に、このマシンの Ollama に判断のモデルがあればその Ollama に送ります（[どうやって確かめているか](assurance.md)）。
