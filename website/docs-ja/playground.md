# ブラウザで試す

このページで動いているのは ritsu そのものです。wasm32 に載せているだけで、プロジェクト全体にかける `ritsu check`（ファイルごとにその言語の検査をかけ、ある言語が確かめたことを隣の言語が読む）も、各言語の生成とページも、コマンドと同じものが走ります。**どこにも送っていません。** 編集したファイルは、ブラウザの中だけにあります。

<div class="pg" data-lang="ja">
  <div class="pg-bar">
    <select class="pg-project" aria-label="開くプロジェクト"></select>
    <button class="pg-add" type="button"></button>
    <button class="pg-remove" type="button"></button>
    <button class="pg-revert" type="button" hidden></button>
    <span class="pg-status"></span>
  </div>
  <div class="pg-files" role="tablist" aria-label="プロジェクトのファイル"></div>
  <textarea class="pg-src" spellcheck="false" autocapitalize="off" autocorrect="off" aria-label="ファイル"></textarea>
  <div class="pg-tabs">
    <button data-view="check" class="on" type="button">ritsu check</button>
    <button data-view="gen" type="button">生成</button>
    <button data-view="doc" type="button">人が読むページ</button>
    <select class="pg-target" aria-label="生成先" hidden></select>
    <select class="pg-picker" aria-label="書いたファイル" hidden></select>
  </div>
  <div class="pg-out"></div>
</div>

<script src="playground/playground.js" defer></script>

ここでのプロジェクトは数個のファイルで、一つのタブに一つのファイルが入っています。ファイルが変わるたびに、ページはファイルを全部モジュールに渡し、モジュールは、同じファイルを置いたディレクトリで走らせるのと同じようにコマンドを走らせます。プロジェクトには `ritsu check .` を走らせ、開いているファイルには、その言語の生成かページを出します。このページの答えが、同じファイルを置いたディレクトリで `ritsu` のバイナリが出す出力と書くファイルに一字も違わないことは、テストで確かめています（`crates/ritsu/tests/playground.rs`）。

## 試してみること

**最初に開くのは、受注が契約に値を一つ足したばかりの小さな通販**です。`proto/shop/v1/order.proto` に `ORDER_STATUS_RETURNED = 5;` が増えました。請求の規則はこの列挙を取り込み、`requirements/billing.req` の要件は規則の表を名指し、地図は規則を受注に対する請求の腐敗防止層として名指しています。一つのファイルの一行に、三つの言語がそれぞれ答えます。英語の名前のプロジェクトですが、出力は日本語です。

1. **指摘を読む。** 見出しには、それを言ったツールが入っています。rulec は「列挙 order_status が ../../proto/shop/v1/order.proto の OrderStatus と一致していません」と言い、規則に無い値を示します。yuen は「rulec が rulec "billing/rules/billing_need.rule" table decide について答えられません」と言います。要件の端のハッシュを取れなくなったからです。sakai は「billing/rules/billing_need.rule が rulec の検査を通らないか、読めません」と言います。`billing/rules/billing_need.rule:5` のような指摘の場所をクリックすると、その行に移ります。
2. **契約の側で直す。** `.proto` からその行を消すと、どのファイルも検査を通ります。
3. **規則の側で直す。** 「編集を取り消す」を押してから、`billing/rules/billing_need.rule` の列挙の最後に `| returned` を足します。rulec は「取り込んだ列挙 order_status の値に、行も `default` もありません」と言います。`cancelled` の行の下に `| returned  | skip        |` を足すと rulec は通りますが、今度は yuen が、表が「2026-10-04 に accounts がこのリンクを確かめたあとで変わりました」と言い、増えた行を見せます。検査を通った規則が、そのまま誰かの確かめた規則になるわけではありません。
4. **ワークフローと規則の境目を壊す。** `ordering/rules/urgency.rule` の出力 `carrier` を、`outputs` と、表と例の見出しで `courier` に変えます。rulec は通ります。dandori は規則の出力を同じプロセスの中で rulec から読むので、`ordering/ship_order.flow` でその出力を読む二つの行に「`urgency.outputs` にフィールド `carrier` はありません（urgent・courier）」と言います。
5. **生成する。** `ordering/ship_order.flow` を開いて「生成」を押すと、Temporal が動かすもの（TypeScript の八つのファイル）が出ます。横のリストで Step Functions を選ぶと、何も書きません。このフローはワークフローに送られるイベントを待ち、`on cancel` で後始末をしますが、Step Functions ではどちらもできません。倉庫を呼ぶための接続も、タスクに書いていません。生成する言語は、どれもここで生成できます。規則は 12 の言語のコード、カレンダーは 5 つの言語のコード、帳簿は SQL とクライアント、地図は Context Mapper の CML、要件は ReqIF か W3C PROV です。
6. **人が読むページを開く。** コードが実現すべきものを理解し、確かめる人のためのページです。規則、カレンダー、帳簿、フローのどれでも開けます。ウィンドウ全体を使うページなので、別のタブで開きます。それぞれがプルリクエスト向けに書く Markdown は、リンクの下にあります。

リストの「小さな通販（日本語）」は、同じプロジェクトの名前を日本語にしたもので、同じ手順をそのまま試せます。

## ここにないもの

- **geas。** geas は、主張が名指すコードを走らせて主張を確かめます。ページの中ではプロセスを起こせないので、`.geas` のファイルは、読んで構文を確かめたあと、プログラムを起こせないと言って止まります。
- **実行。** `dandori run`、`chobo run`、シナリオ、`rulec verify` と `replay` には、プロセスやサーバー、記録のファイルが要ります。それは[コマンド](https://github.com/i2y/ritsu)で使います。
- **ネットワーク。** `source fetch` と `outdated` は、e-Gov、eCFR、出典が名指す URL に問い合わせます。
- **地図の Rust のワークスペース。** sakai は、地図の `code rust "…"` のクレートを Cargo に尋ねます。ページの中では Cargo を走らせられません。
- **プロジェクトへの書き込み。** `rulec fmt` と `yuen review` は、渡したファイルを書き換えます。ここでは、ファイルは編集したときにだけ変わります。
