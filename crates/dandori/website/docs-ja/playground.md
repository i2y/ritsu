# ブラウザで試す

このページで動いているのは dandori そのものです。wasm32 に載せているだけで、検査も、各プラットフォーム向けのビルドも、`dandori doc` が描くページも、コマンドと同じものが走ります。**どこにも送っていません。** 打ち込んだフローは、ブラウザの中だけにあります。

<div class="pg" data-lang="ja">
  <div class="pg-bar">
    <select class="pg-preset" aria-label="開くフロー"></select>
    <span class="pg-path"></span>
    <button class="pg-revert" type="button" hidden></button>
    <span class="pg-status"></span>
  </div>
  <textarea class="pg-src" spellcheck="false" autocapitalize="off" autocorrect="off" aria-label="フロー"></textarea>
  <div class="pg-tabs">
    <button data-view="check" class="on" type="button">検査</button>
    <button data-view="build" type="button">ビルド</button>
    <button data-view="doc" type="button">図にする</button>
    <button data-view="rules" type="button">規則</button>
    <select class="pg-target" aria-label="プラットフォーム" hidden></select>
    <select class="pg-picker" aria-label="ファイル" hidden></select>
  </div>
  <div class="pg-out"></div>
</div>

<script src="playground/playground.js" defer></script>

ここでのフローは、例と同じパスから、例が読むものを読みます。規則と、呼び出す API の記述と、引当と発送の子のフローです。ページの中では rulec を動かせないので、例の規則について rulec が出力したものと、`rulec doc` が描いたものを、リポジトリから記録して持っています。その記録がいまの rulec の出力と同じであること、このページの答えがコマンドの答えと同じであることは、テストで確かめています（[どうやって確かめているか](assurance.md)）。

## 試してみること

**最初に開くのは、ホテルの予約の最初の下書き**です。[何を検査するか](checks.md)の冒頭と同じもので、例と同じ規則と Stripe の API を呼びます。検査すると、エラーが四つ見つかります。

1. **指摘を読む。** 最後の指摘は、PaymentIntent が `processing` のまま、または `requires_payment_method` に戻ったまま、ワークフローが終わることがあると言い、そうなる例を添えています。売上を確定したあとでも、銀行が支払いを断ることがあるからです（Stripe の側で `settle` が起きる）。`tests/fixtures/hotel_naive.flow:95:1` のような指摘の場所をクリックすると、その行に移ります。
2. **一つ直す。** 91 行目の `fail CardDeclined "The card was declined"` の後ろに `leaving pi` を足すと、ワークフローは PaymentIntent をそのまま引き渡すことになり、91 行目の指摘が消えます。ほかの三つは残ります。
3. **図にする。** 「図にする」では、`dandori doc` がワークフローをレビューする人向けに書くページを開けます。エラーがあっても描き、エラーごとに、そうなる例が図の上で光ります。
4. **規則を読む。** 「規則」には、フローが呼ぶ規則が、リポジトリにあるままに出ます。与信の額とフロントで確認するかを決める `hold` と、PaymentIntent のステートマシンの `payment_intent` です。「規則のページを開く」では、`rulec doc` が承認する人向けに描いたページが開き、rulec のプレイグラウンドと同じように、ケースを打って試せます。`dandori doc` が書くページからも、左の規則の一覧から開けます。
5. **例の版を開く。** [「ホテルの予約・Temporal 版」](#flow=examples/hotel/temporal/hotel.ja.flow&view=build)は検査を通り、「ビルド」で Temporal が動かすもの（ワークフロー、アクティビティ、クライアント、ワーカー。TypeScript）が見られます。横のリストでプラットフォームを選び直せます。Step Functions では何もビルドせず、その理由を示します。この版は Stripe の Webhook をワークフローに送られるイベントとして待ち、`on cancel` で与信を取り消しますが、Step Functions ではどちらもできません。Stripe を呼ぶための EventBridge の接続も、規則を呼ぶための Lambda 関数も書いていません。Step Functions 向けに書いたのは[「ホテルの予約・AWS 版」](#flow=examples/hotel/aws/hotel.ja.flow&view=build)です。
6. **わざと壊す。** 「ホテルの予約・Temporal 版」で分岐 `canceled => fail 決済の取消 …` を消すと、`canceled` の分岐がない `match` が見つかり、`与信を取る` で決済が canceled になる例が添えられます。リトライする `売上を確定する` から `key` を消したり、`fail 確定の結果不明` から `leaving 決済` を消したりして、返ってくるものも読んでみてください。

## ここにないもの

- **実行。** `dandori run`、シナリオ、プラットフォームそのもの。ビルドしたものがすべてのシナリオでどう動くかは、テストで走らせています。
- **規則を直すこと。** 規則は例のものだけで、ここでのフローが使えるのもそれだけです。「規則」で読めますが、ここでは書き換えられません。規則を書くなら、[rulec のプレイグラウンド](https://i2y.github.io/rulec/ja/playground/)を使ってください。
- **呼び出し。** API も、エージェントも、Jev も呼びません。呼び出すコードは「ビルド」で見られます。
- **ほかのファイル。** 編集できるのは枠の中のフローだけです。子のフローと API の記述は、リポジトリにあるとおりに読みます。
