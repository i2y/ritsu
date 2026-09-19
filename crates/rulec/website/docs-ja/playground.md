# ブラウザで試す

このページで動いているのは検査器そのものです。wasm32 に載せているだけで、`report` も診断も生成器も、コマンドと同じものが走ります。**どこにも送っていません。** 打ち込んだ表はブラウザの中だけにあるので、web のフォームに貼ってはいけない料金表でも、ここには貼れます。

<div class="pg" data-lang="ja">
  <div class="pg-bar">
    <button data-preset="gap" type="button">一行足りない表</button>
    <button data-preset="full" type="button">そろった表</button>
    <select class="pg-picker" hidden></select>
    <span class="pg-status"></span>
  </div>
  <textarea class="pg-src" spellcheck="false" autocapitalize="off" autocorrect="off"></textarea>
  <div class="pg-tabs">
    <button data-view="check" class="on" type="button">検査</button>
    <button data-view="gen" type="button">生成コード</button>
    <button data-view="doc" type="button">承認者向けの資料</button>
  </div>
  <div class="pg-out"></div>
</div>

<script src="playground/playground.js" defer></script>

## 試してみること

**最初に出ているのは、最後の行が欠けた表**です。[ホーム](index.md)の一枚目の絵に描いてある、あの表です。`check` は「不完全です」とは言いません。**どの入力がすり抜けるか**を名指しして、その穴を塞ぐ行の形まで出します。

1. **指摘を読む。** `当てはまらない例: あて先 = 遠隔地, 重量 = 2001g`。これを見つけるのに、データも旧実装も要りません。
2. **穴を塞ぐ。** 「そろった表」を押すと、欠けていた行 `| 遠隔地 | >2kg <=5kg | 1500円 |` が戻って、指摘が消えます。ヒントが出す行は**形**で、額は一行目から写したもの、重量はちょうど 2001g です。検査が名指しした一点だけを塞ぎます。額を勝手に決めないし、帯がどこまで続くかも推測しないからです。
3. **「生成コード」を開く。** `rulec gen` がこの表に対して書くもの全部です。Python・TypeScript・JavaScript・Rust・Ruby・PHP・Go・Swift・Java・SQL・Wasm と、それぞれのランナー、規則を MCP ツールにするサーバ、そして表の境界から作ったテストベクタ。
4. **「承認者向けの資料」を開く。** `rulec doc --format html` が、表に判子を押す人のために用意するページです。答えの写真ではありません。**生成した JavaScript がその場で動いている**ので、打ち込んだケースは同じコードが決めています。
5. **わざと壊す。** `<=2kg` を `<=6kg` に変えると、両方の行に当てはまる入力つきで重なりが返ります。出力から `round up(10円)` を消すと、丸めの診断が何を訊いてくるか読めます。

## ここに無いもの

`verify`（いま動いている実装との突き合わせ）、`replay` と `diff`（過去の記録との突き合わせ）は、プロセスとファイルが要るのでこのページには置いていません。そちらは[コマンド](install.md)の仕事です。文法は[表(.rule)を書く](tour.md)に、指摘の一つ一つの意味は[何を証明するか](checks.md)にあります。
