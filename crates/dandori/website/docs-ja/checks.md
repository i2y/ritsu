# 何を検査するか

`dandori check` は `.flow` と、それが使う規則を読みます。そして、すべての行き先、呼び出しが返しうるすべてのエラー、外部のサービスの側で起きうるすべての出来事を考えに入れて、処理の流れを追います。診断には、そこへ至る短い実行の例が付きます。

## 終わっていない案件を残す

カードに与信を取ってチェックアウトの日に確定するホテルの予約
（[tests/fixtures/hotel_naive.flow](https://github.com/i2y/dandori/blob/main/tests/fixtures/hotel_naive.flow)、例の最初の下書き）を `--lang ja` で検査すると、次の診断が出ます。

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
          相手の側で `settle` が起きる: pi processing → requires_payment_method
      95  succeed
```

</div>

案件 `pi` は、ワークフローが状態を追いかける Stripe の PaymentIntent です。その遷移は `payment_intent.rule` に書いてあります。Stripe の文書をもとに、PaymentIntent の状態の変わり方を rulec のステートマシンとして書き写したものです。ワークフローには、Stripe の側でひとりでに起きる出来事を `external authenticate, settle, expire` と宣言してあり、検査はそれが起きる場合もたどります。チェックアウトの日まで待つあいだに与信の期限が切れれば、確定の呼び出しは断られます。確定したあとでも、銀行の処理で支払いが通らないことがあります。

## 検査の項目

**型と名前。** 名前はどれも一度だけ宣言し、その型で使います。無いことがある値は `match` で確かめてから使い、引数と出力はそろっていなければなりません（E001〜E006）。

**行き先と文の置き場所。** `match` は、どの値にも行き先がなければならず（E010）、通ることのない行き先があってもいけません（E011）。変数を読めるのは、そこへ至るどの経路でも値が入っている場所だけです（E012）。`yield`、`break`、`succeed`、案件への呼び出しは、意味のある場所にしか書けません（E009）。

**案件。** 案件にタスクを呼べるのは案件を始めたあとだけで、始められるのは一度だけです（E013）。どの状態でも断られる出来事を送るのはエラーです（E021）。断られることがある出来事を送るなら、その断りを受けなければなりません（E022）。ワークフローが終わるとき、始めた案件はどれも終わりの状態になっていなければなりません。例外は、`fail … leaving` で引き渡すときだけです（E020）。どこでも受けないエラーで失敗して、案件が終わらないまま残ることもあります（W101）。これは `on failure` で片付けます。

**やり直し。** 外部のデータを変える呼び出しを冪等キー（`key`）なしでやり直すと、同じ変更を二度加えてしまうかもしれません（E030。変えるかもしれない、というだけなら W030）。案件を始めるタスクに `key` が無いのは警告です（W103）。

**範囲。** 規則、タスク、レコード、出力に渡す値は、そこの範囲に収まっていなければなりません（E014）。範囲の分からない値を渡すと警告になります（W104）。[範囲](tour.md#範囲)

**タスクが呼ぶもの。** ほかの `.flow` を走らせるタスクは子と（E015）、記述のある API を呼ぶタスクはその記述と（E016）照らし合わせます。[タスクが呼ぶもの](tasks.md)

**プラットフォーム。** プラットフォームごとに違うことは、`dandori build` が確かめます。一回の実行がプラットフォームの上限を超えうるか（E040）と、プラットフォームに要るものが欠けていないか、プラットフォームにできないことをしていないか（E050）です。

## 一回の実行の大きさ

一回の実行の大きさには、プラットフォームごとに上限があります。実行履歴は、Step Functions で 25,000 件、Temporal で 51,200 件、Lambda durable functions で 3,000 操作までです。Argo Workflows では Workflow のオブジェクトがすべてのノードを抱えるので、dandori は 10,000 ノードを目安にしています。ループには回数の上限を書き（`repeat at most 12 times`、`for x in xs at most 50`）、再帰も無いので、ビルドは一回の実行履歴が最も長くなる場合を数えられます。数え方は多めなので、上限に近いワークフローは、実際には収まる場合でも断ることがあります。

Temporal では、フローの一番外のループは、履歴が長くなったところで新しい実行に引き継ぎます（Continue-As-New）。そのため、そのループは引き継ぐまでの分だけを数えます。それでも上限を超えるのは、ループの外の部分か、ループの一回分が大きすぎるときです。診断は、そのどちらなのかを示します。

## 診断コードの一覧

[診断コード](reference/codes.md)に、28 種類すべてと、それぞれが何を見つけるかをまとめています。
