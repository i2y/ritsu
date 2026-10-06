# ritsu 設計文書

七つの小さな言語（rulec、dandori、koyomi、chobo、geas、yuen、sakai）を、一つのリポジトリの一つの処理系にまとめる。言語は七つのまま残し、土台（診断、二つの言語の文、出典、名指し、doc のページの枠、テストの共通部分）、単位の型、プロジェクトの読み込み、CLI、バージョン、Lean の層を一つにする。目的は、ある言語が確かめたことを、隣の言語が型の付いたまま受け取れるようにすることである。

名前は律（りつ）から取った。

この文書は段階 A（設計）で書き、段階 C の最初の部分で、作者が決めたこと（★だった項目）、yuen への改名（2.2）、作った土台の形（4.12）とテストの道具の形（10.9）を書き足した。段階 D の最初の部分で、口の実物（3.2）、rulec の言語をスレッドごとに持つこと（4.1）、rulec と dandori の `.proto` の読み手（4.13）、単位の型の実物（5.1、5.2）を書き足した。1 章の行数と数、1.4 と 12.5 の出力は、2026-10-03 にこの機械（macOS arm64、rustc 1.94.1）で、各リポジトリを読み、作業場所にコピーしたものを走らせて取った。元のリポジトリでは何もビルドしていない。各リポジトリのテストの件数と時間のうち、ここで走らせていないものは、それぞれの最後の記録から引き、そう書いた。段階 E と F の担当の記録（2026-10-04）から、口の問い（3.2）、ファイルの読み書きの入口（4.15）、言語をまたぐ検査の作った形と台帳（7 章）、`ritsu run`（7.9）、ブラウザで試すページ（8.7）、`ritsu gen` と生成物の頭（9 章）、英語の版と日本語の版（10.10、10.11）、Lean の層（11 章）、リリースの準備（13 章）、F.1〜F.3 の yuen と sakai の doc（4.8）、入れ方（8.2、13.2）、根の README とスキルの確かめ（10.12）、作者の 19:30 の指示で入れたこと（X4 の額 0 は 7.6 と 11.2、サイトの移動と `repository` は 13.2、rulec の英語のページと台帳と出力は 10.10）、作者の 10/4 の夜の決めと日本語の出力の読み直し（4.1、4.3、4.8、5.6、8.7、13.2）を書き足し、A の段階のスケッチと見込み（3.2 と 5.1 のコード、6.4 の表、7.2 の表、7.5 と 7.8 の案、8.1 のコマンド、8.3 の例、9.3 の形）を実物に差し替えた。段階ごとの作業と完了の条件は PLAN.md にある。

## 0. 全体像

```
ritsu（バイナリ。CLI。ritsu-wasm はブラウザで動かすもの。LSP は今回は作らない。15 章）
 ├── ritsu-cross    言語をまたぐ検査（7 章）
 └── ritsu-project  プロジェクトの読み込み、名前の解決、口のつなぎ（6 章）
      ├── rulec  dandori  koyomi  chobo  geas  yuen  sakai     言語のクレート（互いに依存しない）
      └── 土台の層
           ├── ritsu-base    診断、二つの言語の文、診断の台帳、CLI の表、ハッシュ、名指しとパス、出典、doc の枠、JSON、YAML、Cedar、鍵の形、URL、秘密の印
           ├── ritsu-units   単位の型（5 章）
           ├── ritsu-ports   口：言語のあいだで渡すものの型と、問いの形（3.2）
           ├── ritsu-proto   .proto の読み手
           └── ritsu-emit    生成先の言語ごとの書き出しの共通部分（9.2）
 ritsu-testkit：テストの共通部分。どのクレートも dev-dependency としてだけ使う（10 章）
```

### 0.1 芯：言語の境目で、証明を切らない

七つの言語は、それぞれ小さいから確かめられる。rulec は表の完全性と重なりを、koyomi は範囲のすべての日の計算を、chobo は勘定の境界がどの振替でも一度の書き込みの中で守られることを、dandori はワークフローが終わるときの案件の状態と、外部のデータを二度変えうるリトライを確かめる。

ところが、いまは言語の境目で、確かめたことが途切れる。境目は各ツールの CLI が出す JSON で、受け取る側は JSON の中の文字列と数しか見ない。rulec は `JPY` と `円` が同じ単位だと知っているが、dandori は二つを別の文字列として比べてエラーにする。rulec は率の刻みを知っているが、dandori はそれを JSON Schema の説明の文から読み取っている（どちらも 1.4 に実際の出力を載せた）。koyomi は関数がとりうる値を全部数えられるのに、その集合を rulec の表の検査に渡す口が無い。

ritsu は、境目を JSON から型の付いた呼び出しに替える。ある言語が確かめたことを、隣の言語が前提として受け取り、その先を確かめられるようにする（7 章の検査）。そのために七つを一つの処理系にまとめる。入口が一つになることや、インストールが一回で済むことは、その結果としてついてくる。

看板の言い方は、次のとおりにする。README（F）の頭と、リリースの説明に使う。

- **Seven small languages, one toolchain. What one checks, the next can build on.**
- **七つの小さな言語を、一つの処理系で。ある言語が確かめたことを、隣の言語が前提にできる。**

### 0.2 前提

- **P1**：言語は七つのまま。構文、検査、ページを読んで理解し確かめる人に見せるページ（`doc`）、名前、拡張子は言語ごとに別々に保ち、一つのファイルに二つの言語を混ぜない。規則、期日、帳簿、ワークフロー、主張、要件、地図は、書く人も、ページを読んで理解し確かめる人も違うからである。どれか一つの言語だけでも成り立つ（`.flow` を書かずに rulec だけを使える）。
- **P2**：一つにする理由は、言語の境目で証明を切らないこと。便利さは設計の判断の基準にしない。
- **P3**：小ささは、プロセスの境目ではなく、クレートの依存の決まりで守る（3 章）。言語のクレートは、土台の層にだけ依存する。言語をまたぐ検査は別の層に置き、口だけを通す。この決まりは、sakai でこの処理系そのものの地図を書いて確かめる（7.13）。
- **P4**：言語の意味は、それぞれの言語の参照インタプリタと検査が決める。言語をまたぐ層は、各言語に問いを渡して答えを受け取るだけで、ある言語の意味をもう一度書かない。たとえば ritsu-cross は rulec の表の完全性を自分で解き直さず、rulec に「入力をこの集合に限ったとき、表は完全か」を尋ねる。
- **P5**：言語をまたぐ検査の結果は、成り立つことを示した、成り立たない例がある、決められない（理由つき）の三つのどれかにする。決められないことを黙って通さない（sakai の P7 を処理系全体に広げたもの）。
- **P6**：各言語のコマンドの振る舞い（コマンド、フラグ、終了コード、診断のコード、`--format json` と `api` の形）は、取り込んだあとも保つ。変えるときは、その言語の DESIGN.md に理由を書く。外のツールのための JSON（各ツールの `api` など）は、これまでどおり出す。
- **P7**：実装は Rust。意味の中心部分（参照インタプリタの評価と、検査の判定）は Lean 4 でモデルにして証明し、Rust の実装と Lean のモデルを、テストが作る入力で突き合わせる（11 章）。
- **P8**：元のリポジトリには書かない。git の履歴を残して取り込み、取り込んだあとの開発は ritsu で行う（12 章）。
- **P9**：土台の層は std だけで書き、外のクレートに依存しない。いま serde_json を使っている五つ（dandori、koyomi、chobo、yuen、sakai）はそのまま使ってよい。rulec と geas は依存が無いまま保つ（4.9）。

dandori を作ったときの前提の一つ、「rulec の CLI の出力だけを読む」（dandori の DESIGN 0.1 の P2）は外した。あの前提は、rulec の小ささと、rulec の木で別に進んでいる作業の邪魔をしないことを、プロセスの境目で守っていた。ritsu では同じことを P3 で守る。

### 0.3 一つにするもの、別々に保つもの

| 一つにする | 別々に保つ |
|---|---|
| リポジトリ（Cargo のワークスペース）と、バージョンとリリース | 言語の名前、字句、構文、キーワード、拡張子 |
| 土台：診断と `--lang ja`、二つの言語の文、診断の台帳と `explain`、CLI の表、ハッシュ、出典の固定と改正の検知、名指しとパス、doc のページの枠、JSON、YAML（JSON と行き来できる YAML 1.2）、Cedar のポリシーとスキーマ、セキュリティの検査が使う決まり（鍵の形、URL、秘密の印。4.19） | 各言語の検査と参照インタプリタ |
| 単位の型 | ページを読んで理解し確かめる人に見せるページの中身 |
| `.proto` の読み手、生成先の言語ごとの書き出しの共通部分 | 生成するコードの形（rulec の表の一行が一つの分岐になる形など） |
| プロジェクトの読み込みと名前の解決 | 各言語の README、DESIGN.md、PLAN.md、スキル、例 |
| CLI（`ritsu`）と wasm | 各言語のコマンド（`rulec` など。8.2） |
| テストの共通部分と CI | 診断のコードの番号（言語ごと） |
| Lean の層 | |

### 0.4 語

- **証明**：rulec の §15.47 と同じく、宣言した範囲のどの入力でも成り立つことを、しらみつぶしに調べて示したものにだけ使う。Lean の定理もそう呼ぶ。生成したコードと参照インタプリタの突き合わせはテストで、証明とは呼ばない。ツールの出力は、肯定形では「証明」を使わず、通ったことを「確かめた」と書く。
- **土台**：言語のクレートが共通に使う部分。どの言語の意味も持たない。
- **口**：言語のあいだで渡すものの型と、ある言語に尋ねる問いの形。`ritsu-ports` に置く（3.2）。
- **境目**：ある言語の値や事実が、別の言語に渡るところ。dandori のフローが規則を呼ぶところ、yuen の要件が規則の表を名指すところなど。
- **出す側、受け取る側**：口を通して事実を出す言語（rulec、koyomi、chobo、geas）と、それを受け取る言語（dandori、yuen、sakai）。dandori は出す側でもあり（タスクと案件を yuen と sakai に出す）、rulec は入力の範囲を koyomi から受け取るときに受け取る側になる（7.5）。
- **成果物**、**名指し**：yuen と sakai の DESIGN.md の 2 章と同じ意味（6.2）。

## 1. いまの七つと、重なっているコード

### 1.1 大きさ

src の行数は、git が追っている `.rs` のファイルだけを数えた。括弧の中は、生成物が使う実行時のコードなど `.rs` でないもの。`#[test]` は数えたもので、走らせた数ではない。

| | src の `.rs` の行 | ファイル | `#[test]` | `tests/*.rs` の行 | 依存 | edition | バージョン | コミット | リモート |
|---|---|---|---|---|---|---|---|---|---|
| rulec | 69,622（Python 295） | 56 | 635 | 21,264 | なし | 2024 | 0.22.1 | 355（タグ 28） | 公開 |
| dandori | 30,802 | 30 | 81 | 4,897 | serde_json | 2021 | 0.1.0 | 52（タグ 1） | 公開 |
| koyomi | 16,750 | 38 | 99 | 3,644 | serde_json | 2024 | 0.1.0 | 2 | なし |
| chobo | 11,269（TS・Python・Go 1,893） | 24 | 63 | 3,759 | serde_json | 2024 | 0.1.0 | 1 | なし |
| geas | 16,402（136） | 35 | 236 | 3,872 | なし | 2024 | 0.0.1 | 6 | なし |
| yuen | 9,878 | 30 | 94 | 2,393 | serde_json | 2024 | 0.1.0 | 1 | なし |
| sakai | 9,190 | 31 | 97 | 2,443 | serde_json | 2024 | 0.1.0 | 1 | なし |
| 合計 | 163,913 | 244 | 1,305 | 42,272 | | | | | |

serde_json を使う五つの `Cargo.lock` は、どれも serde_json 1.0.151 と同じ 15 のパッケージを固定している。rulec と geas の `Cargo.lock` には自分しか無い。rulec には Lean の証明（`proofs/`、5,476 行、Lean v4.34.0、mathlib なし）がある。dandori だけが edition 2021 で、`--version` と `explain` を持たない（`--version` とコマンドごとの `--help` は C.11 で、`explain` と診断の台帳は D の二つ目の部分で足した）。

### 1.2 重なっているコード

同じ役目のコードを、言語ごとに書いていた。段階 C でそれを土台の層へ移した。「移す前」は取り込んだとき（2026-10-03）の行数、「土台」と「言語に残るもの」は段階 C の終わり（同じ日）の行数である。土台の行数は、そのモジュールのファイル全体（単体テストを含む）を数えた。

| 重なり | 移す前 | 土台 | 言語に残るもの |
|---|---|---|---|
| SHA-256 | rulec `src/sha256.rs` 87、koyomi `src/sha256.rs` 75、yuen `src/sha256.rs` 75、sakai `src/sha256.rs` 84、chobo `src/ids.rs` の 12〜79 行（68）。計 389 | `ritsu-base` の `sha256` 82 | なし（`grep -rn 0x428a2f98 crates/*/src` は土台にしか当たらない）。chobo の ID の決め方は chobo に残る |
| 診断 | `src/diag.rs`：rulec 549、dandori 159、koyomi 335、chobo 184、geas 260、yuen 223、sakai 214。計 1,924 | `diag` 296 | そこに至る例の部分（`src/diag.rs`：koyomi 207、chobo 144、geas 210、yuen 138、sakai 138）。rulec（549）と dandori（147）は自分の診断の型を残した（4.2。dandori の文は土台の `Text`） |
| 二つの言語の文 | `src/i18n.rs`：rulec 76、koyomi 155、yuen 167、sakai 183。計 581。ほかに chobo、geas、dandori の `Text` や `Lang` | `text` 248 | rulec の `src/i18n.rs` 117（プロセスの言語を持つ `tr!` に、D.5 でスレッドごとの言語 `with` を足した） |
| 診断の台帳の枠 | `src/codes.rs` の `find` と書き出し：rulec 135、chobo 88、geas 108、koyomi 73、yuen 88、sakai 74。計 566 | `ledger` 284 | rulec の 135（rulec の台帳は自分の形のまま）。台帳の中身（`codes.rs`：rulec 2,198、koyomi 458、chobo 850、geas 1,175、yuen 398、sakai 711） |
| CLI の表 | 表を読む仕組み：koyomi、yuen、sakai、chobo にそれぞれ 190〜240 行ほど。計約 875 | `cli` 316 | コマンドとフラグの表（koyomi `src/cli.rs` 242、yuen `src/cli.rs` 232、sakai `src/cli.rs` 161、chobo `src/main.rs` 637 の一部、dandori `src/cli.rs` 145）。rulec の `src/main.rs`（2,214）と geas の `src/main.rs`（785）は自分の形の表を残した |
| 出典のコピーと固定、改正の検知 | rulec `src/sources.rs` の 1,390 行、koyomi `src/fetch.rs` 581 と `src/sources.rs` の 326 行、yuen `src/fetch.rs` 762・`src/copies.rs` 323・`src/base64.rs` 74・`src/sources.rs` 305。計約 3,760 | `sources` 938 | rulec `src/sources.rs` 1,640（コピーと表の突き合わせ、`source fetch`・`pin`・`outdated` を rulec の文で言う部分、単体テスト。移す前は全体で 2,142）、koyomi `src/fetch.rs` 455 と `src/sources.rs` 370（祝日の表を含む全体）、yuen `src/fetch.rs` 567・`src/copies.rs` 54・`src/sources.rs` 296（借りた出典を含む） |
| 名指し | yuen `src/names.rs` 422、sakai `src/naming.rs` 367。計 789。ほかにルートとパスの扱い（sakai `src/paths.rs` 268 など） | `naming` 496、`paths` 247 | 診断のコードと文（yuen `src/names.rs` 146、sakai `src/naming.rs` 142、sakai `src/paths.rs` 75） |
| `.proto` の読み手 | rulec `src/proto.rs` 1,358、dandori `src/proto.rs` 1,153、sakai `src/proto.rs` 1,080。計 3,591 | `ritsu-proto` 1,850 | sakai `src/proto.rs` 76。rulec `src/proto.rs` 569（列挙の別名、`shape` のパスがたどるフィールドの取り方、単体テスト）と dandori `src/proto.rs` 551（import の探し方、proto3 だけを読むこと、型の名前の解き方、単体テスト）。どちらも D.10 で ritsu-proto で読むようにした |
| JSON（依存の無い二つ） | rulec `src/json.rs` 456、geas `src/json.rs` 513。計 969 | `json` 577 | rulec `src/json.rs` 204（誤りの文、値の種類の名前、キーを並べて書き戻すこと、`--format json` の書き手）。geas `src/json.rs` 513 は残した（4.9） |
| doc のページの CSS | rulec、dandori、koyomi、chobo の四つ。計約 340 | `docpage` 139（ページの頭、配色、外の URL の確かめ） | koyomi と chobo は土台の配色を使う。rulec と dandori は、doc の出力を変えないために自分の CSS を残した（4.8） |
| 生成物の予約語 | rulec `src/backend.rs` の `words`（189 行）、koyomi `src/reserved.rs` 76、dandori と chobo の表。計約 300 | `ritsu-emit` の `words` 139、`copies` 276（ほかに `ident`・`lit`・`header` 155） | koyomi `src/reserved.rs` 23（出力先と表の組）。rulec と dandori は、`copies` にコピーした自分の表を読む（9.5） |
| wasm の境目 | rulec `src/wasm.rs` 194、dandori `src/wasm.rs` 142。どちらも「バッファの頭に長さを書く」同じ決まり | — | F.5 で、同じ決まりの `ritsu-wasm` を作った（8.7）。二つの `src/wasm.rs` は、前のページを ritsu のページへの転送に替えたときに消した（8.7、2026-10-05） |
| テストの共通部分 | `tests/common`：koyomi 247、chobo 1,040（`mod.rs` 116、`runners.rs` 730、`servers.rs` 194）、geas 546、yuen 185、sakai 337。計 2,355。ほかに dandori と rulec のテストの中 | `ritsu-testkit` 1,169 | 言語ごとのもの（`tests/common`：koyomi 25、chobo 823、geas 299、yuen 142、sakai 209） |

テストの共通部分の中で重なっていたのは、自分を消す一時ディレクトリ（koyomi、chobo、geas、yuen、sakai と、dandori の `tests/examples.rs`）、時間を区切って子プロセスを走らせること（macOS に `timeout` が無いため。koyomi、geas、sakai）、golden と取り直し（chobo、geas、yuen、sakai と、koyomi、dandori、rulec のテストの中）、使い捨ての PostgreSQL のクラスタ（koyomi、chobo）、Chrome を探すこと（dandori、koyomi、chobo、geas。順は五つとも同じ）、Mermaid で図を描けるかを確かめること（dandori と chobo）である。どれも `ritsu-testkit` の一つになった（10.8、10.9）。rulec のテストは PostgreSQL を `PG*` の環境変数で受け取る形のまま、SKIP と段を `ritsu-testkit` にした。一時ディレクトリも、C のあとに `TempDir` にした（PLAN の 7.5）。

単位の表と有理数（rulec の `src/types.rs` の `CURRENCIES`、`money_unit`、`unit_info`、`unit_offset` と、`src/num.rs` の有理数）は、D.1 で `ritsu-units`（628 行。表と有理数と単位の型。テストは別のファイル）に移した。rulec の `src/num.rs` は 282 行から 165 行になり、丸めの五つの仕方だけが残った。口の型とトレイトは D.2 で `ritsu-ports`（706 行）に置き、各言語の `src/ports.rs`（rulec 493、koyomi 267、chobo 266、sakai 211、yuen 128、geas 93）が答える。

移す前の表の合計は、テストの共通部分を除いて約 1 万 4 千行だった（診断のうち言語に残る部分も含む）。土台に移せば 6 千行ほどになると見込んでいた。段階 C の終わりの土台の三つは 6,087 行（`ritsu-base` 3,654、`ritsu-proto` 1,846、`ritsu-emit` 587）で、見込みに近い。七つの言語の src は 163,913 行から 158,157 行になった（rulec 68,571、dandori 30,763、koyomi 15,764、chobo 11,070、geas 16,380、yuen 8,414、sakai 7,195）。土台が言語ごとの形のいちばん広いものを取り、単体テストも持つので、全体の行数はほとんど減っていない。減ったのは、同じ役目の実装の数である。段階 D の最初の部分のあとは、七つの言語の src が 158,439 行になった（D の前のコミットでは 158,226 行。rulec 68,648 から 68,447、dandori 30,755 から 30,161、koyomi 15,764 から 16,033、chobo 11,070 から 11,364、geas 16,380 から 16,482、yuen 8,414 から 8,544、sakai 7,195 から 7,408）。口に答える `src/ports.rs` の 1,458 行が増え、rulec と dandori の `.proto` の読み手が 1,383 行減った。rulec の診断と台帳と `tr!`（E）、二つの doc の CSS が、まだ言語の側に残っている。段階 D の二つ目の部分のあとは、七つの言語の src が 159,716 行になった（rulec 68,447 から 68,493、dandori 30,161 から 31,400、yuen 8,544 から 8,540、sakai 7,408 から 7,404。koyomi、chobo、geas は変わらない）。dandori では、rulec の JSON の読み手と子プロセスが消え（`src/rulec.rs` 678 から 542）、記録から答える口（`src/record.rs` 334）、`Items` と `References`（`src/ports.rs` 190）、診断の台帳（`src/codes.rs` 315）が増えた。コマンドの本体は `src/main.rs`（340 から 12）から `src/cli.rs`（145 から 559。`explain` を含む）に移った。入口の最小の形の `crates/ritsu` は 90 行である。段階 D の最後の部分のあとは、七つの言語の src が 162,148 行になった（rulec 68,493 から 68,529、koyomi 16,033 から 16,065、chobo 11,364 から 11,464、geas 16,482 から 15,934、yuen 8,540 から 10,605、sakai 7,404 から 8,151。dandori は変わらない）。geas の統一形式の差分の読み手は ritsu-base の `udiff`（642 行）に移り、geas の `src/diff.rs` は 734 行から 121 行になった（ritsu-base は 3,654 から 4,323。台帳の `Repro::Retired` を含む）。yuen では `affected`（`src/affected.rs` 854）、口のまとまり（`src/suite.rs` 75）、コマンドの関数（`src/run.rs` 351）が、sakai では口のまとまり（`src/suite.rs` 206）とコマンドの関数（`src/run.rs` 305）が増えた。口の `ritsu-ports` は 706 から 818 行（`Sources` と `Claims::affected`）、入口の `crates/ritsu` は 141 行（`ritsu yuen` と `ritsu sakai`）である。段階 E の最初の部分のあとは、七つの言語の src が 163,088 行になった（rulec 68,529 から 68,631、dandori 31,400 から 31,524、koyomi 16,065 から 16,102、chobo 11,464 から 11,511、geas 15,934 から 16,020、yuen 10,605 から 10,687、sakai 8,151 から 8,613）。増えたのは、`ritsu check` のために各言語が検査の結果を型で渡す関数（`checked`。8.3）、受け取る側がほかの言語を読めないときに出す診断（dandori の E018、yuen の E206。2.3）、sakai が Rust のクレートの依存を読むところ（`src/cargo.rs` 198 行。3.4）である。rulec、koyomi、chobo、geas は、コマンドの本体を `src/main.rs` からライブラリ（rulec と geas は `src/cli.rs`、koyomi と chobo は `src/run.rs`）に移し、`src/main.rs` はそれを呼ぶだけになった。口の `ritsu-ports` は 818 から 1,042 行（索引と、検査の結果の型）、新しい `ritsu-project` は 409 行、`ritsu-cross` は 139 行、入口の `crates/ritsu` は 141 から 580 行（`ritsu check`、`ritsu explain`、七つの言語の入口）である。段階 E と F のあと（2026-10-04、全部を取り込んで 0.23.0 にそろえた main）は、七つの言語の src が 170,844 行になった（rulec 68,631 から 69,507、dandori 31,524 から 35,937、koyomi 16,102 から 16,194、chobo 11,511 から 11,640、geas 16,020 から 16,052、yuen 10,687 から 11,528、sakai 8,613 から 9,986）。dandori では、日付と帳簿、言語をまたぐ検査に渡すもの（`src/crossings.rs`）、決められない前提を確かめる文（`src/prechecks.rs`）、`ritsu run` のための計算（`src/computed.rs`）、生成パッケージの中のフローが、yuen と sakai では `doc` が増えた。土台と入口は、`ritsu-base` 4,788、`ritsu-units` 628、`ritsu-proto` 1,850、`ritsu-emit` 672、口の `ritsu-ports` 1,383、`ritsu-project` 425、`ritsu-cross` 1,507、入口の `crates/ritsu` 1,740（`ritsu run` と `ritsu gen` を含む）、`ritsu-wasm` 565、`ritsu-testkit` 1,183、`ritsu-model` 121、`xtask` 606 行である。Lean の `proofs/` は 12,490 行（rulec の証明書の検査を移したものを含む）。

キーワードの表（各言語の `kw.rs` や `syntax.rs`）、字句と構文、検査、参照インタプリタは重なりに数えない。言語ごとの語彙と意味そのものだからである。

### 1.3 同じ考えで、形が違うもの

- **診断の JSON のキー**：dandori は `code`・`severity`・`line`・`col`・`message`・`notes`・`path`（ファイルは外側に）、chobo は `v`・`column`・`title`・`excerpt`・`operations`・`hint`、geas は dandori の形に `file`、koyomi は `inputs`・`steps`・`fails`・`fix`、yuen は `diff`・`chain`・`candidates`、sakai は `references`。ファイルのパスは、yuen がルートからの相対、sakai が走らせたディレクトリからの相対で食い違っている（6.2 の 9）。rulec は別の形（`v` が 2 の形。`where`・`witness`・`rows`・`fix`）。
- **二つの言語の文の書き方**：rulec、koyomi、chobo、yuen、sakai は `tr!("日本語", "English")`（日本語が先）、geas は `t(en, ja)`、dandori は `Diag::error(code, line, col, en, ja)`（英語が先）。rulec の `tr!` は、プロセスで一つの言語（`src/i18n.rs` の `AtomicU8`）を読んで `String` を返す。ほかの四つの `tr!` は、二つの文を持つ `Text` を返し、どちらを出すかは出すところが決める。
- **SKIP の書き方**：`SKIP:` の行（dandori 55 か所、chobo 27、koyomi 11、yuen 6、sakai 5、geas 1）、rulec のテストの `注意:`（53 か所。rulec のテストは日本語で回すため）、`rulec test` の `skipped`。
- **ルートの決め方**：geas（`src/tree.rs`）、yuen（`src/project.rs`）、sakai（`src/paths.rs`）が、それぞれ「いちばん近い `.git` のあるディレクトリ」を探す。
- **言語と取り直しの環境変数**：`RULEC_LANG`、`DANDORI_LANG`、`KOYOMI_LANG`、`CHOBO_LANG`、`GEAS_LANG`、`YUEN_LANG`、`SAKAI_LANG`。取り直しは `<名前>_BLESS`。
- **名前の正規化**：rulec の DESIGN §1.1 は「識別子は NFC に正規化する」と書くが、src には無い。chobo は結合文字を含む名前を E001 でエラーにする。

段階 C で、このうち二つの言語の文の書き方（七つとも `tr!("日本語", "English")`）、SKIP の書き方（七つとも `SKIP: <クレート>: <理由>`）、ルートの決め方（土台の `paths`）、`RITSU_LANG`（rulec のほかの六つ。rulec は D の二つ目の部分で読むようにした）、`RITSU_BLESS`（`ritsu-testkit` の golden を使う六つ。rulec のテストは自分の取り直しのまま）、chobo と sakai の診断の JSON のキーを、土台の形にそろえた（4.12、10.9）。rulec の `tr!` がプロセスの言語で `String` を返すこと、rulec と dandori の診断の JSON の形、名前の正規化は、まだそれぞれのままである。

### 1.4 境目で切れているもの

次の出力は、2026-10-03 に作業場所で取った。rulec は 0.22.1 のリリースのバイナリ、dandori は main をそのまま作業場所にコピーして作ったもの。

**同じ単位が、境目で違う単位になる。** rulec の規則で、列を `money[円, incl_tax]` にし、セルを `JPY` で書く。

```
| amount     | -> fee : money[円, incl_tax] |
| <30000JPY  | 500JPY                       |
| >=30000JPY | 0円                          |
```

```
$ rulec check same_unit.rule --lang en
ok same_unit.rule
```

dandori の例の規則 `hold_amount.rule` は、出力 `amount` を `money[JPY, incl_tax]` と書いている。それを `money[円, incl_tax]` を取るタスクに渡す。

```
use rule hold from "rules/hold_amount.rule"
task authorize(amount: money[円, incl_tax])
…
  authorize(amount: quote.amount)
```

```
$ dandori check hold.flow
error[E003]: hold.flow:17:21: expected `money[円, incl_tax]` here, but this is `money[JPY, incl_tax]`
    17 |   authorize(amount: quote.amount)
```

rulec の中では `JPY` は `円` の別の綴りで（rulec の DESIGN §15.18、`src/types.rs` の `money_unit`）、dandori は単位を文字列のまま比べる（dandori の `src/model.rs` の `Ty::Num(String)` と `src/rulec.rs` の `normalize_unit`）。D の二つ目の部分で、dandori も単位の型で比べるようにし、この例は通る（5.3）。

**率の刻みが、説明の文にしか無い。** `off : rate[step 0.1%]` を入力に持つ規則で：

```
$ rulec schema rate.rule --lang en
… "off": {"type": "integer", "description": "an integer: the rate as a count of 0.1% steps (100% is 1000)", "minimum": 0, "maximum": 500}
$ rulec schema rate.rule --lang ja
… "off": {"type": "integer", "description": "整数。率を 0.1% 刻みの個数で書く（100% なら 1000）", "minimum": 0, "maximum": 500}
$ rulec certificate rate.rule
… "types": {"off": "rate", "pay": "money[円, incl_tax]", "price": "money[円, incl_tax]"}
```

証明書の型は `rate` だけで、刻みは説明の文の中にしか無い。dandori は `src/rulec.rs` の `rate_scale` で、その文から `100% is ` か `100% なら ` のあとの数を読んでいる（dandori の DESIGN 7 章にも残してある）。D の二つ目の部分で、dandori は刻みを口の単位の型から読むようにした（3.2）。

**バージョンの文字列で、二つのツールが食い違う。** dandori は、`rulec doc` が描いたものを読み解かずに埋め込む。そのため dandori の `tests/` と `website/` の 57 のファイルに `rulec 0.22.0` という文字列があり、rulec を 0.22.1 にすると、テストは中身が同じでもバージョンの文字列で落ちる（dandori の README は「tested with rulec 0.22.0」と書く）。一つの処理系なら、二つは同じバージョンでしかありえない。

**境目のためのコードと、止めている作業。**

- dandori の `src/rulec.rs`（678 行）は、規則ごとに rulec を三回（`schema`、`certificate`、`api`）子プロセスで走らせ、JSON を読む。ブラウザで試すページでは rulec を走らせられないので、`src/sources.rs`（292 行）が、記録しておいた rulec の出力を返す。（D の二つ目の部分で、どちらも規則の口に替えた。3.2、PLAN の D.3）
- yuen は、rulec、koyomi の日付、geas、dandori の成果物を、ファイルの単位でしか名指せない（yuen の DESIGN 3.2）。JSON が、表や日付の関数の一つ一つの定義を出さないからである。
- sakai は、dandori の参照を確かめられず、N101 で「確かめていない」と言う（sakai の DESIGN 4.7）。
- yuen の段階 C の「一式の読み込み」（yuen の PLAN の C.1〜C.9）と、sakai の段階 C の C.1〜C.5 は、この処理系の形が決まるまで止めてある。
- koyomi の `api`（`wire` の `at` は「dandori の timestamp と同じ形」と書く）と chobo の `api`（仮押さえのステートマシンを、dandori が rulec のステートマシンを読むのと同じフィールドで出す）は、いずれ dandori が読むために作ったが、dandori はまだ読まない。

## 2. ワークスペースの構成

### 2.1 ディレクトリ

```
ritsu/
  Cargo.toml             ワークスペース。members は crates/*。version、license、repository、edition は [workspace.package] に
  Cargo.lock
  DESIGN.md  PLAN.md  LICENSE-MIT  LICENSE-APACHE
  README.md  README.ja.md（F）
  crates/
    ritsu-base/  ritsu-units/  ritsu-ports/  ritsu-proto/  ritsu-emit/  ritsu-testkit/    土台の層とテストの共通部分（C〜D）
    ritsu-model/           Rust と Lean のモデルの突き合わせ（F。11.3）
    rulec/  dandori/  koyomi/  chobo/  geas/  yuen/  sakai/                            元のリポジトリを履歴ごと（B）
    ritsu-project/  ritsu-cross/                                                        つなぎの層（E）
    ritsu/  ritsu-wasm/                                                                 入口（E、F）
    xtask/                 テストの段、SKIP の集計、変えたクレートの選び出し、依存の決まりの確かめ（C）
  proofs/                Lean の層（一つの Lake のパッケージ。ライブラリは五つ、プログラムは rulec-recheck と ritsu-model。F で作り、rulec の proofs/ もここへ移した。11 章）
  skills/ritsu/          エージェント向けのスキル（F）
  website/               ritsu のサイト（F）。根の設定（zensical.toml、zensical.ja.toml）と docs/・docs-ja/ に ritsu のページ（index とブラウザで試すページ）、docs/playground/ にそのモジュールと JS、playground/ にページが開くプロジェクト、tools/make_wasm.sh、build.sh・sync.sh・serve.sh。言語のサイトは下のディレクトリに置く（rulec/ と dandori/。F.7 で移した。13.2）
  packaging/  action.yml リリースで配るものを作るスクリプトと GitHub Action（F.7。13.2）
  ritsu.ctx  contexts/   処理系自身の地図（E.8。3.4）
  .github/workflows/     CI（10.5）
  ci/skips/              CI のジョブが許す SKIP の一覧（10.3、10.5）
```

言語のクレートの中は、元のリポジトリの木をそのまま残す（README、DESIGN.md、PLAN.md、`tests/`、`examples/`、`tools/`、`docs/`、`skills/`、rulec の `experiments/`。rulec と dandori の `website/` は F.7 で根の `website/rulec/` と `website/dandori/` に、rulec の `proofs/` は F で根の `proofs/` に移した。11.4、13.2）。元の `.github/workflows/` も `crates/rulec/.github/` と `crates/dandori/.github/` に来たが、GitHub はそこにあるワークフローを走らせない。ritsu の CI は根の `.github/workflows/` に新しく書き（C.12）、クレートの中のものは消した（10.5）。

### 2.2 クレートと名前

| クレート | 役目 | 元にするもの |
|---|---|---|
| `ritsu-base` | 土台（4 章） | 1.2 の重なり |
| `ritsu-units` | 単位の型（5 章） | rulec の `src/types.rs` の単位の表と `src/num.rs` の有理数 |
| `ritsu-ports` | 口（3.2） | dandori の `src/rulec.rs` が JSON から組み立てている型、yuen と sakai が読む予定だったもの |
| `ritsu-proto` | `.proto` の読み手（4.10） | sakai の `src/proto.rs`（型の名前の解決）に、rulec の Protovalidate と buf の依存の読み方、dandori の `json_name` とオプションの読み方を足す |
| `ritsu-emit` | 生成先の言語ごとの予約語、識別子、文字列のリテラル、生成物の頭（9.2） | rulec の `src/backend.rs`、koyomi の `src/reserved.rs` と `src/naming.rs`、dandori と chobo の予約語の表 |
| `ritsu-testkit` | テストの共通部分（10 章） | 五つの `tests/common` と、dandori の `tests/` の中の同じ役目のコード |
| `rulec` `dandori` `koyomi` `chobo` `geas` `yuen` `sakai` | 言語 | 元のリポジトリ。パッケージの名前もバイナリの名前も変えない（yuen だけは、取り込んだときの yurai から C の最初に改めた。下の段落） |
| `ritsu-project` | プロジェクトの読み込み、名前の解決、口の実装をつなぐ（6 章）。索引の型は `ritsu-ports` に置き、ここで言語の答えをつなぐ | 新しく書いた（E.1） |
| `ritsu-cross` | 言語をまたぐ検査と、その診断の台帳（7 章） | 新しく書いた（E.3〜E.5） |
| `ritsu` | バイナリ `ritsu`（CLI）と、`ritsu check` の本体を持つライブラリ（8.7） | 新しく書いた（D、E） |
| `ritsu-wasm` | ブラウザで動かす wasm32-unknown-unknown のモジュール（8.7） | 新しく書いた（F.5）。境目の決まり（バッファの頭に長さを書く）は rulec と dandori の `src/wasm.rs` から（二つとも、前のページを転送に替えたときに消した。8.7） |
| `ritsu-model` | Lean のモデルの実行ファイルに入力を流し、Rust の参照インタプリタと一行ずつ比べる（11.3） | 新しく書いた（F.6） |
| `xtask` | 開発のための作業。公開しない | 新しく書いた（C.3） |

言語のクレートのパッケージの名前を変えないのは、コード（`use rulec::…`）、テスト（`env!("CARGO_BIN_EXE_rulec")`）、環境変数の名前を、B の段階で一つも直さずに済ませるためである。crates.io では `koyomi`（「Japanese calendar written in Rust」、0.4.0）と `yurai`（「Forensics-grade provenance explorer for AI models」、0.4.0）が別のクレートに使われている（2026-10-03 に crates.io の API で確かめた。`ritsu`、`rulec`、`dandori`、`chobo`、`geas`、`sakai`、`ritsu-base`、`ritsu-units` は空いていた）。中のクレートは公開しない（13.2）ので、名前がぶつかること自体は困らない。

ただし要件の来歴の言語は、取り込んだときの名前 yurai を、段階 C の最初に yuen に改めた。crates.io の `yurai` は AI のモデルの来歴を調べるツールで、扱うもの（来歴）が近い。中のクレートを公開しなくても、近い分野に同じ名前のツールが二つあれば、読む人が取り違える。`yuen` は crates.io で空いていた（2026-10-03 に確かめた）。改めたのは、ディレクトリ（`crates/yuen`。`git mv` で履歴を保った）、パッケージとバイナリの名前、環境変数（`YUEN_LANG`、`YUEN_BLESS` など）、文書と診断と golden、名指しのツールの語（6.2 の 2。`yuen "民法の期間.req" requirement 満了日_142条`）、書き出しの名前（ReqIF の `yuen.` の属性、PROV の名前空間、識別子のハッシュの頭の `yuen/1`）である。ファイルの拡張子 `.req` と、元のリポジトリ（`~/yurai`）の名前は変えていない。名前を含む例のファイル（yuen のテストの `payment` のコードと約款）はハッシュで固定してあるので、改めたあとのハッシュで固定と確かめた記録を書き直した。

### 2.3 バイナリ

- 言語のクレートは、いまの `[[bin]]`（`rulec` など）を残す。開発と、そのクレートのテストが使う。
- すべての言語をつなぐバイナリは `crates/ritsu` の `ritsu` だけである。
- リリースで配るのは `ritsu` 一つで、`rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`yuen`、`sakai` はそれを指すリンクにする。リンクの名前で呼ばれたら、その言語のコマンドとして、すべての口をつないで動く（8.2。E.2 で作った）。Cargo のバイナリの名前は、ワークスペースの中で重ならない（言語のクレートの `rulec` と、リリースのリンクの `rulec` は、作られる場所が違う）。
- `rulec mcp` は、ツールの呼び出しのたびに自分自身（`current_exe()`）を走らせるので、そのとき `arg0` を `rulec` にする。リンクを通って起動されても `current_exe()` は `ritsu` の実体を返すので、そうしないと `ritsu` の名前で走り、`rulec check` の呼び出しが `ritsu check` になる（`crates/ritsu/tests/links.rs`）。
- 受け取る側（dandori、yuen、sakai）のクレートのバイナリは、D の段階からほかの言語を読めない（ほかの言語のクレートに依存しないため）。ほかの言語を読むところに来たら、`ritsu <言語>` で走らせるよう言う診断を出す。dandori のクレートのバイナリは、規則を使わないフローならいまと同じに動く。D の二つ目の部分で dandori をそうした。規則を読む口に、何も読まない口（`dandori::sources::NoRules`）を渡し、`use rule` のところで E005 が `ritsu dandori …` で走らせるよう言う。`ritsu dandori` は、D で先に作った入口の最小の形にある（8.6）。D の最後の部分で yuen と sakai もそうした。yuen のクレートのバイナリは何もつながない口の束（`yuen::suite::Suite` の空のもの）を渡され、ほかの言語のものを名指すプロジェクトには、`ritsu yuen` に同じコマンドを続けた形を言って exit 2 で終わる。sakai のクレートのバイナリも何もつながない口の束（`sakai::suite::Suite` の空のもの）を渡され、地図が規則、カレンダー、ワークフローを含めば、言語ごとに一つの E104 で `ritsu sakai` に同じコマンドを続けた形を言う（診断なので exit 1。sakai の DESIGN 4.1）。
- ライセンス：リリースの `ritsu` と、ブラウザで試すページの `ritsu.wasm` は、`(MIT OR Apache-2.0) AND Unicode-3.0 AND BSD-3-Clause` である。ワークスペースのコードは MIT OR Apache-2.0 で、rulec は Unicode CLDR の区分の名前（Unicode-3.0）を、koyomi は WHATWG の Encoding Standard の索引から作った Shift_JIS の変換表（BSD-3-Clause）を持つ。そこで rulec の `license` は `(MIT OR Apache-2.0) AND Unicode-3.0`、koyomi は `(MIT OR Apache-2.0) AND BSD-3-Clause` にし、二つを含む `ritsu` と `ritsu-wasm` は、ワークスペースの `license` を使わずに、三つを合わせた式を書く。ほかのクレートは、他者のものを持たないので、ワークスペースの `MIT OR Apache-2.0` のままである。外のクレート 8 個はそれぞれのライセンスのままバイナリに入り、式には入れない（Cargo と Homebrew の慣例どおり、`license` はそのプロジェクトのソースのものを書く。外のクレートは通知に並べる）。`explain` の例のための法令のコピーも入るが、式には入れない。e-Gov の利用規約（PDL1.0）には SPDX の識別子が無く、求めるのは出典の記載で、それは通知に書いた。CFR は米国では著作権の対象にならない。一覧、出どころ、ライセンスの文は根の `THIRD_PARTY_NOTICES` にある（13.2）。テスト（`crates/ritsu/tests/release.rs`）が、式が `ritsu`・`ritsu-wasm`・`packaging/nfpm.yaml`・formula・通知でそろい、中の言語のクレートが持つものを合わせたものであることを確かめる。

E の最初の部分で、三つの言語がほかの言語を読めないときの扱いを、一つの形にそろえた。どの言語も、ほかの言語を読むところに来たら、コードのある診断を、要る言語ごとに一つ、その言語を最初に要するところに出し、注に同じコマンドを `ritsu <言語>` で走らせる形を書いて（「<言語> のクレートのバイナリは、ほかの言語を持ちません。…」）、使い方の誤りとして exit 2 で終わる。コードは言語ごとのもので、dandori は新しい E018（最初の `use rule` で言い、規則を読めないことから起きるほかの診断は出さない。前は規則ごとの E005 と、それに続く E002 を出して exit 1 だった）、yuen は新しい E206（前はコードの無い文を標準エラーに出していた）、sakai はこれまでの E104（exit を 1 から 2 にした）である。exit 2 にしたのは、走らせ方の問題で、ファイルの誤りではないからである（読めないファイルや知らないフラグと同じ）。exit 1 のままにすると、`dandori check` を CI で走らせる人は、フローの誤りと、ritsu で走らせていないことを、終了コードで見分けられない。コードを付けたのは、`--format json` を読むエージェントが、文を読み解かずに、`ritsu <言語>` で走らせ直せばよいと分かるようにするためである。それぞれの DESIGN.md（dandori の 0.3、yuen の 3.1 と 6.2、sakai の 4.1 と 12.3）と台帳に書いた。

## 3. 依存の決まり

### 3.1 層

| 層 | クレート | 依存してよいもの |
|---|---|---|
| 土台 | `ritsu-base` | std だけ |
| | `ritsu-units` | `ritsu-base` |
| | `ritsu-proto`、`ritsu-emit` | `ritsu-base` |
| | `ritsu-ports` | `ritsu-base`、`ritsu-units` |
| 言語 | `rulec` `dandori` `koyomi` `chobo` `geas` `yuen` `sakai` | 土台の層。serde_json（いま使っている五つだけ） |
| つなぎ | `ritsu-project` | 土台の層、七つの言語 |
| | `ritsu-cross` | 土台の層、`ritsu-project`。七つの言語は口の実装を通してだけ使う |
| 入口 | `ritsu`、`ritsu-wasm` | すべて |
| テスト | `ritsu-testkit` | std だけ。どのクレートも `[dev-dependencies]` としてだけ使う |
| | `xtask` | std と serde_json |
| | `ritsu-model` | std だけ。言語と serde_json は `[dev-dependencies]` としてだけ使う。どのクレートもこれに依存しない（11.3） |

決まりは四つある。

1. 言語のクレートは、ほかの言語のクレートに `[dependencies]` で依存しない。テストのための `[dev-dependencies]` は許す（3.3）。
2. 土台の層は、言語にも、つなぎの層にも、入口にも依存しない。
3. 言語のクレートは、つなぎの層と入口に依存しない。
4. 外のクレートは serde_json だけ（`preserve_order` と `float_roundtrip`）。足すときは、この文書に理由を書く。

`ritsu-wasm` は `ritsu` のライブラリ（`ritsu check` の本体）、`ritsu-project`、言語のクレート、`ritsu-base`、serde_json に依存する。同じ入口の層どうしの依存で、地図では同じコンテキスト（`Entry`）の中にある。入口の `ritsu` も、`ritsu run` の JSON のために serde_json に依存する（E.6）。

### 3.2 口

境目でやりとりするものは、全部 `ritsu-ports` の型にする。出す側の言語は、自分が確かめたことをこの型で出し、受け取る側はこの型で受け取る。どちらも `ritsu-ports` のトレイトを通すので、受け取る側は出す側のクレートを知らない。口のトレイトは出す側が自分で実装し（`rulec::Engine` が `Rules` を実装する、など）、`ritsu-project` がそれを作って受け取る側に渡す。

| 口 | 出す側 | 受け取る側 | 中身 |
|---|---|---|---|
| `Rules` | rulec | dandori、yuen、sakai、ritsu-cross | 入力と出力（名前、別名、単位の付いた型、範囲、率の刻み）、列挙（値の名前と、Connect のワイヤでの名前と番号）、ステートマシン（軸、行が受け付ける座標、遷移、書く値、held）、前提（`constraint`・`sum`・`length`）、Connect のサービスの形、生成したコードの呼び方、`rulec doc` が描いたもの。問いは「この範囲の値で、前提は必ず成り立つか」と「この入力の値をこの集合に限ったとき、表は完全で、重なりが無く、当てはまらない行が無いか」。評価は入力から出力 |
| `Dates` | koyomi | dandori、rulec、sakai（doc のため。F.2）、ritsu-cross | 関数、引数の型と範囲、カレンダーとデータの範囲、`at` の時刻と UTC オフセット、条件の名前と文。問いは「入力の範囲で、関数がとりうる値の集合」と「入力から値までの日数の最小と最大」。評価 |
| `Books` | chobo | dandori、sakai（doc のため。D.8）、ritsu-cross | 単位（ritsu の単位の型。D.9）、勘定と境界と拒否の理由、振替の種類（引数と単位、キー、仮押さえと有効期限、移動）、仮押さえのステートマシン。問いは「額がこの範囲のとき、どの操作が、どの理由で拒否されうるか」。評価（帳簿の状態を持つ） |
| `Claims` | geas | yuen、ritsu-cross | 主張の一覧（名前、行、書いたとおりの手順）。map の記録の読み方。差分が主張に何をもたらすか（geas の `affected`。D.7 で足した） |
| `Items` | 七つ全部 | yuen、sakai | 中のもの（種類、名前、行の範囲、定義の文）。6.4 |
| `References` | 七つ全部 | sakai、yuen | 参照（行、先の名指し、参照の仕方）。6.4 |
| `Sources` | rulec、koyomi | yuen | ファイルが宣言して保存している出典（法令の ID と時点と条ごとの固定、文書のパスと url と固定）。検査を通るファイルにだけ答える。D.7 で足した（下の段落） |
| 索引（`Index`） | （口ではなく、`Items` と `References` の答えを持つもの） | yuen、sakai、ritsu-cross | 各言語の `Items` と `References` の答えをファイルごとに一度だけ尋ねて持ち、名指しで引く。E.1 で足した（6.4） |
| `Flows` | dandori | ritsu-cross | 検査を通るフローが、規則・koyomi の日付・chobo の振替を呼ぶところと、渡す値がどこから来うるか、期限のある仮押さえを確定か取消をするまでの長さ。E.4 と E.5 で足した（下の段落）。秘密の値をプロジェクトの中の成果物へ送る呼び出し（`sends`。2026-10-06 に足した。16.8）：呼び出しの行、タスクか規則、送る先のファイル（OpenAPI の文書、`.proto`、Connect の規則、子の `.flow`、帳簿、日付のファイル。dandori が届くパス）、引数ごとの秘密の値（フローの書き方、印のファイルと行と印）、`discloses` の引数と理由 |
| `Maps` | sakai | ritsu-cross | 地図のコンテキストと関係、ファイルがどのコンテキストに属するか。sakai の検査の段 1 と段 2（構文、名前、パスと、属し方）だけで答える。2026-10-06 に足した（16.8、16.9） |
| `Undecided` | ritsu-cross（`ritsu_cross::UndecidedCalls`） | dandori | フローの規則の呼び出しごとに、X2 が決められなかった前提（呼び出しの行、規則のファイル、前提）。dandori の生成したコードが、ワークフローを走らせたときに確かめる（7.4 の 3）。E.5 で足した |

どの問いの答えも、P5 の三つのどれかになる。値を尋ねる問い（koyomi の日付がとりうる値の集合など）は、その値か、決められない理由かの二つになる。

```rust
// ritsu-ports（段階 D の最初の部分で作り、E で問いと口を足した形。crates/ritsu-ports/src）
pub enum Answer<E> { Holds, Fails(E), Undecided(Text) }   // 成り立つ、成り立たない例、決められない理由
pub enum Found<T> { Value(T), Undecided(Text) }            // 値を尋ねる問いの答え
pub struct Said { code, file, line, message: Text }        // 答えられないときに、その言語が言うこと

pub trait Rules {
    fn facts(&self, rule: &Path) -> Result<RuleFacts, Vec<Said>>;
    fn preconditions_hold(&self, rule: &Path, ranges: &[(String, Option<i128>, Option<i128>)], max_len: Option<i128>) -> Result<Vec<(Precondition, Answer<Values>)>, Vec<Said>>;
    fn output_values(&self, rule: &Path, output: &str) -> Result<Found<OutputValues>, Vec<Said>>;
    fn checked_over(&self, rule: &Path, input: &str, days: &DaySet) -> Result<Answer<Text>, Vec<Said>>;
    fn date_range(&self, rule: &Path, input: &str) -> Result<(Option<i64>, Option<i64>), Vec<Said>>;
    fn eval(&self, rule: &Path, inputs: &Values) -> Result<Values, RuleError>;
    fn doc(&self, rule: &Path, shown: &str, html: bool, lang: Lang) -> Result<String, Vec<Said>>;
    fn joined(&self) -> bool;   // 何も読まない口（dandori の NoRules など）は false
}
// `..` は `&self, file: &Path`。`Dates`・`Books`・`Claims` の答えは `Result<_, Vec<Said>>` に包む（`Ledger` の `apply` と `balance` は `Result<_, Text>`）
pub trait Dates { fn facts(..) -> DateFacts; fn values(.., date) -> Found<DaySet>; fn days(.., date) -> Found<(i64, i64)>; fn span(.., date) -> Found<DaySpan>; fn input_for(.., date, day) -> Option<inputs>; fn eval(.., inputs) -> Vec<(String, DateValue)>; fn joined(&self) -> bool; }
pub trait Books { fn facts(..) -> BookFacts; fn refusals(.., transfer, amounts) -> Found<Vec<(op, reasons)>>; fn open(..) -> Box<dyn Ledger>; fn joined(&self) -> bool; }
pub trait Ledger { fn apply(&mut self, &BookCall) -> Result<BookOutcome, Text>; fn pass(&mut self, seconds) -> expired; fn balance(&self, account, args) -> Balance; fn accounts(&self) -> named; fn holds(&self) -> holds; }
pub trait Claims { fn claims(..) -> Vec<Claim>; fn map_record(..) -> Option<MapRecord>; fn affected(.., root, diff, diff_shown, records) -> Affected; }
pub trait Sources { fn sources(..) -> Vec<Source>; }   // Source { name, line, kind: Law { db, id, asof, pins } | File { path, url, pin } }
pub trait Items { fn items(&self, root: &Path, file: &str) -> Result<Vec<Item>, Vec<Said>>; }       // Item { naming, lines, text }
pub trait References { fn references(&self, root: &Path, file: &str) -> Result<Vec<Reference>, Vec<Said>>; } // Reference { line, target, how }
pub struct Ports { rules: Rc<dyn Rules>, dates: Rc<dyn Dates>, books: Rc<dyn Books> }   // フローを読む三つの口。ritsu-project の Joined::ports が作る
pub trait Flows { fn rule_calls(.., rules) -> Vec<RuleCall>; fn crossings(.., ports: &Ports) -> Crossings; fn sends(.., ports: &Ports) -> Vec<Send>; }   // dandori が答える。E.4 と E.5、sends は 16.8
pub trait Undecided { fn preconditions(&self, file: &Path) -> Vec<UndecidedPrecondition>; }   // ritsu-cross が答え、dandori が受け取る。E.5
pub trait Maps { fn map(&self, root: &Path, map: &str) -> Result<Option<MapFacts>, Vec<Said>>; fn context_of(&self, root: &Path, map: &str, file: &str) -> Result<Option<String>, Vec<Said>>; }   // sakai が答え、ritsu-cross が受け取る。16.8
```

`RuleFacts` は、dandori の `src/rulec.rs` がいま三つの JSON から組み立てている `RuleInfo`（入力と出力の `Column`、列挙、`Machine`、前提、`walks`）を、単位の型（5 章）の付いた形にしたものである。D の段階では、例のすべての規則について、型の付いた呼び出しで得た事実と、いまの JSON から読んだ事実が同じになることを一度確かめてから、JSON の読み手を消す。D の二つ目の部分でそうした。rulec のコーパスの 50 本と dandori の 14 本の規則で、dandori が読む 1,939 の項目を突き合わせ、違った 7 か所は、どれも JSON の側の読み違えだった（規則がたどる並びを入力に数えて文字列と読んだこと 5 本、無いことがある入力を文字列と読んだこと 2 本）。ステートマシンの状態の軸（口では `Option`）は、dandori も `Option` で持ち、None は「表が状態を読まないので、どの状態からも同じ行が当てはまる」と読む（前の dandori は null を最初の軸と読み違えていた）。くわしくは PLAN の D.3。

**段階 D の最初の部分で作った形**（PLAN の D.2）。スケッチから変えたのは次のことである。

- 問いの相手は、名指し（`Name`）ではなく、呼ぶ側が届くファイルのパス（`&Path`）にした。dandori の `use rule` のパスも、yuen と sakai の名指しも、呼ぶ側がファイルのパスに直してから尋ねる。中のものと参照（`Items`、`References`）だけは、名指しを作るためにルート（6.2 の 3）とルートからのパスを受け取る。
- 答えられないときに返すのは、言語ごとの診断の型ではなく `Said`（その言語のコード、ファイル、行、二つの言語の文）にした。言語の診断の型を口に出すと、受け取る側が出す側のクレートを知ることになる。
- `Rules` に `doc` を足した（表の中身にあった「`rulec doc` が描いたもの」）。言語は呼ぶ側が渡し、rulec はそのスレッドの言語で描く（4.1）。ファイルの名前（ページの頭に書くもの）も呼ぶ側が渡す。dandori は、ファイルの名前だけを書いたページを埋め込むからである。
- `RuleFacts` は `RuleInfo` の項目を全部作れる形にした。規則の名前と別名と版と SHA-256、rulec の版、入力と出力（名前、別名、型。数は rulec の綴りのままの型と、単位の型（率は刻みつき）と、受け渡す整数での範囲）、並び、列挙（別名と値の別名つき）、ステートマシン（軸、行が受け付ける座標、行き先、書く値、held）、前提、Connect のサービスの形（パス、フィールド、ワイヤの列挙の名前と番号）、生成したコードの TypeScript・Python・Go での呼び方（モジュール、関数、引数と出力の名前と型、列挙のメンバー）である。`RuleInfo` の `api`（`rulec api` の JSON そのもの）にあたるものは、dandori が読んでいるところ（TypeScript・Python・Go の呼び方と Connect の形）を型にした。率の刻みは、JSON の側が JSON Schema の説明の文から読んでいるが、型の側は rulec の型の刻みを持つ。rulec の `tests/ports.rs` が、コーパスの 50 本の全部で、口の事実が `rulec api`・`certificate`・`schema` の JSON を読んだものと同じことを確かめる。
- 出す側の実装は、どれも各言語の `src/ports.rs` の `Engine` である。geas は、口に答えるためにライブラリとコマンドに分けた（`src/lib.rs` と `src/main.rs`）。

この段階で答えを作った問いと、まだ答えない問い（決められない、と理由を言う）は次のとおり。

| 口 | 答える | まだ答えない |
|---|---|---|
| `Rules`（rulec） | `facts`、`doc`、`eval`（参照評価器。生成したコードが入口で受け付けない入力、つまり型と列挙と範囲と入力どうしの関係を破るものは受け付けない）、`preconditions_hold` のうち入力どうしの関係（範囲の箱のいちばん厳しい角で決まる。成り立たなければその角が例） | `preconditions_hold` のうち並びの合計と長さの上限（問いが並びの長さの範囲を持たない）、`checked_over`（rulec の検査が日付の集合を軸に置く形を持たない。E の X3 (b)） |
| `Dates`（koyomi） | `facts`、`values`、`days`、`eval`（範囲のすべての入力で計算する） | 入力の組み合わせが koyomi の確かめる数を超えるとき、途中で計算が止まる入力があるときは、決められないと言う |
| `Books`（chobo） | `facts`、`open`（参照インタプリタの帳簿。操作、時間を進める、残高） | `refusals`（chobo の検査は額を決まった値でしか試さない。E の X4） |
| `Claims`（geas） | `claims`、`map_record`、`affected`（D.7） | — |
| `Sources`（rulec、koyomi） | `sources`（D.7。検査を通らないファイルには、その検査の診断を返す） | — |
| `Items`、`References` | rulec、koyomi、chobo（中のものだけ）、geas（中のものだけ）、yuen、sakai、dandori（D の二つ目の部分。D.6） | — |

**D の最後の部分で足したもの**（PLAN の D.7）。yuen が一式を口で読むために、口を二つ足した。

- `Sources`：yuen が借りる出典（規則とカレンダーが保存して固定しているもの）と、E107 で比べるコピーの固定を渡す。取り込む前の yuen は `rulec api` と `koyomi api` の `sources` を読むつもりだった。`Rules` と `Dates` の事実に入れずに別の口にしたのは、受け取るのが yuen だけで、`RuleFacts` と `DateFacts` を作るより軽く答えられ（rulec は規則を読み直さずに覚えたものから、koyomi は検査を通したあとで出典の行だけを読み直す）、検査を通らないファイルに答えないこと（その検査の診断を返すこと）が、yuen の E203 にそのままつながるからである。
- `Claims::affected`：差分（バイト列と、見せるときの名前）と記録から、geas の `affected` の答え（記録がどちら側のものか、触る主張と行、どの主張も走らせない行、消えるファイル、ソースでないファイル、spec と基準の変わり）を渡す。geas の `affected` のコマンドと同じ関数（`answer_for`）が答えるので、二つが食い違わない。

**E.5 の前半で足したもの**（PLAN の E.5。dandori が koyomi と chobo を呼ぶため）。

- `BookFacts` に `typescript`・`python`・`go`（型は `BookClient { module, transfers: Vec<ClientTransfer> }`、`ClientTransfer { member, params }`）。chobo のクライアントが振替と引数をどう名付けるかで、chobo の口は、chobo のクライアントの名前の決め方（`client::{typescript,python,go}`）から埋める。koyomi の生成したコードの名前は口に足さず、dandori が koyomi と同じ ritsu-emit の決め方で求める。
- `Dates::joined` と `Books::joined`（既定は true）。規則の口の `joined` と同じで、dandori の、口を持たない走らせ方の `NoDates`・`NoBooks` は false を返し、dandori は E018 を出す。
- あわせて、chobo の口の `checked`（`facts` が使う読み込み）が、ファイルを自分で読むようにした。前は `check_file` の誤りの文（`cannot read <パス>: …`）を `Said::unreadable` でもう一度包み、「cannot read」が二度出ていた。

**E.4 で答えるようにした問いと、足した口**（PLAN の E.4）。

| 口 | E.4 で変えたこと |
|---|---|
| `Rules`（rulec） | `preconditions_hold` に並びの長さの上限 `max_len` を足した。並びの合計の上限は、並びがいちばん長く、要素のフィールドがどれも範囲の上の端のときがいちばん大きい（上の端が 0 未満なら空の並びの 0）ので、それが上限を超えなければ成り立ち、超えれば、その並びが例になる。長さの上限は `max_len` と比べ、超えれば上限より一つ長い並びが例になる。`max_len` が None なら、どちらも決められない。`checked_over`（日付の入力を日の集合に限ったとき、表が完全で、重ならず、どの行も当たるか）に答えるようにした（rulec の §15.174 と同じ仕組み。自分の検査を通る規則が対象なので、集合に限って新しく出るのは、どの日も当たらない行の E102）。`output_values`（数の出力の最小と最大、行がどれも数を書くならその数の集合、それぞれになる入力の例を規則のベクタから）と `date_range`（日付の入力の範囲。既定の実装は「分からない」を返すので、dandori の口の三つの実装（`NoRules`、`Recorded`、`Recording`）は直していない）を足した。前提に `Days`（`range from koyomi` の入力。rulec の §15.174）を足した |
| `Books`（chobo） | `refusals` に答えるようにした。chobo の検査と同じ探索を、額を範囲に限って走らせる（範囲の両端、そのすぐ内側、まん中、範囲の中に入る決まった候補を試し、新しく作る額も範囲の中に置き、境界から作る額が範囲の外なら試さない）。見つけた理由には、そこへ至る操作の例がある。見つけない理由は、その深さまでは起きないということで、起きないことの証明ではない。負の額だけの範囲は決められない（chobo の受け取る額は 0 以上。0 だけの範囲は 0 で探す。X4 を額 0 から数えるようにしたときに直した。7.6） |
| `Flows`（dandori。新しい口） | `rule_calls(ファイル, 規則の口)`：検査を通るフローが規則を呼ぶところごとに、行、規則のファイル、`use rule` の名前、渡す値ごとに入力の名前・書いたとおりの値・範囲（値を入れるすべての場所に範囲があるときだけ。dandori の E014 と同じ読み方）・範囲の無い出どころ |

まだ答えない問いは、koyomi の確かめる数を超える入力と、途中で計算が止まる入力（どちらも決められない理由を返す。D のまま）だけになった。

**E.5 で足した口と問い**（PLAN の E.5）。

| 口 | E.5 で変えたこと |
|---|---|
| `Flows`（dandori） | `crossings(ファイル, 口)` を足した。口は `Ports`（規則・日付・帳簿の三つの口の組）で、`ritsu-project` の `Joined::ports` が作る。dandori の検査を通るフローについて、規則を呼ぶところ（`RuleCall`）、koyomi の日付を呼ぶところ（`DateCall`：日付の入力に渡す値）、chobo の振替の操作を呼ぶところ（`TransferCall`：振替、操作、タスクが宣言したエラー、渡す額）、期限のある仮押さえを確定か取消をするところまでの長さ（`HoldSpan`）を返す。渡す値ごとに、どこから来うるか（`Origin`）を添える。koyomi の日付の日（`Day`：ファイルと日付）、規則の数の出力（`Output`：規則と出力）、`now`、dandori が範囲を知っている数（`Range`）、何も言わないところ（`Unknown`：その場所の名前）の五つで、変数に値を入れるすべての場所を合わせる。`RuleCall` の渡す値（`CallArg`）にも `from` を足した。E.4 の `rule_calls` は、規則の口だけで読む問いとして残した |
| `Dates`（koyomi） | `span`（日付の入力から日付までの日数の最小と最大を、それぞれに初めてなる入力の日とともに）と `input_for`（日付がある日になる最初の入力）を足した。どちらも既定の実装がある（`span` は `days` から例の日なしで、`input_for` は「無い」）ので、dandori の `NoDates`・`RecordedDates` は直していない。koyomi が範囲のすべての入力を計算して答える |
| `Undecided`（ritsu-cross。新しい口） | 型は `ritsu-ports` の `flows.rs` に置いた（`UndecidedPrecondition { line, rule, precondition }` と、トレイト `Undecided { fn preconditions(&self, file) -> Vec<UndecidedPrecondition> }`）。出す側が言語のクレートでなく ritsu-cross なのは、決められるかどうかを決めるのが言語をまたぐ検査だからである。dandori は ritsu-cross に依存せず、口だけを受け取る（3.1 の決まりのまま。`cargo xtask deps` も通る） |

ritsu-ports には、秒を最も大きな単位で書く `seconds_text`（`17 days 9 hours`、`17 日 9 時間`）と、日の番号を `YYYY-MM-DD` で書く `day_text` も置いた（dandori と ritsu-cross の文が使う。`ritsu_cross::borders::day_text` はこれを指す）。

**E の二つ目の部分で足したもの**（PLAN の E.6）。`ritsu run` が帳簿の終わりを見せるために、`Ledger` に問いを二つ足した。`accounts` は `do` と `hold` が名指した勘定の全部（拒否された操作が名指した勘定も入る）とその残高、`holds` は仮押さえの全部といまの状態で、どちらも `chobo run` がシナリオの終わりに出すものと同じものを同じ順に返す（chobo の DESIGN 8 章）。

**2026-10-06 に足したもの**（16 章のセキュリティの検査）。`Flows::sends` は、秘密の値をプロジェクトの中のファイルへ渡す呼び出しを返し、新しい口 `Maps` は、sakai の地図のコンテキストと関係と、ファイルがどのコンテキストに属するかを返す。ritsu-cross の X14 が二つを合わせ、秘密の値の送り先が地図の上で送ってよいところかを確かめる。どちらも、検査を通らないもの（フローか地図）には、その検査の診断を `Said` で返す。型と決まりは 16.9 に書いた。

中のものの定義の文は、6.4 の表のとおりにした。rulec は `rulec fmt` が書く形の行、koyomi は `date … =` の塊の行と条件の行（コメントと前後の空白を除く）、chobo は yuen の DESIGN 3.2 の形の JSON（yuen の試作が計算したハッシュと同じになる）、geas は主張の塊の行、dandori はタスク・案件・レコードの宣言の塊の行（コメントと前後の空白を除き、文字列の外の続いた空白を一つにし、字下げは深さごとに空白二つに直す。dandori の DESIGN 0.3）である。表に無かった yuen は要件の端の中身（yuen の DESIGN 4.1）と出典の固定の行、sakai はコンテキストのファイルの行と語の塊の行にした。

### 3.3 テストと dev-dependency

受け取る側のクレートのテストは、出す側のクレートを `[dev-dependencies]` に持ち、本物の実装をつないで走らせてよい。口のトレイトは出す側が実装するので（3.2）、つなぎ方は一か所にしか無い。出す側は受け取る側に依存しないので、依存は輪にならない。

受け取る側のクレートのテストのうち、いまバイナリを走らせて規則などを読むもの（dandori の `tests/examples.rs` など）は、D の段階で、CLI を関数として呼ぶ形（`dandori::cli::run(引数, 口, 標準出力, 標準エラー)`）に替える。すべてをつないだバイナリを走らせるテストは、`crates/ritsu/tests/` に置く。

D の二つ目の部分で、dandori をそうした。D の最後の部分で、yuen も rulec、koyomi、chobo、geas、dandori、sakai を `[dev-dependencies]` に持ち、`ritsu yuen` と同じにつないで、コマンドを関数（`yuen::run::run`）として呼ぶようになった。sakai も同じで、rulec、koyomi、chobo、dandori を `[dev-dependencies]` に持ち、`ritsu sakai` と同じにつないで `sakai::run::run` を呼ぶ。例が一式からコピーしたものを、前は一式のバイナリ（`SAKAI_RULEC` など）で確かめていたのも、口で確かめるようになり、ritsu-testkit の `Need::Suite` と CI の `SAKAI_*` の変数を消した。環境変数を変えて走らせるもの（テストの中の e-Gov に問う `source outdated`）は、ritsu のバイナリを走らせる `crates/ritsu/tests/yuen.rs` に置いた。dandori のテストは rulec を `[dev-dependencies]` に持ち、規則を `rulec::ports::Engine` から同じプロセスの中で読む。ライブラリを呼ぶテストは、読む口をスレッドに置いて（`dandori::sources::with_rules`）呼び、バイナリを走らせていたテストは `dandori::cli::run` を呼ぶ。テストが要る `rulec gen` の出力は、rulec の `gen` の本体をライブラリに移した `rulec::codegen::generate`（コマンドと同じ関数。出力は変わらない）で作り、`rulec vectors` の出力は `rulec::vectors` で作る。規則を読むだけのテストは、rulec のバイナリが要らなくなったので `fast` の段でも走る。

土台の層の `ritsu-proto` と `ritsu-emit` のテストは、言語の側が移るまでのあいだだけ、rulec（`ritsu-proto` は dandori も）を `[dev-dependencies]` に持ち、言語のいまの読み手と表を、土台のものと生のまま比べる（C.9、C.10）。決まり 2 の例外で、移したあとに残しておく理由は無い。言語の側が土台のものを使うようになるとき（表は C.11、読み手は D.10）に、比べる部分とその dev-dependency を消し、golden と比べるテストだけを残す。そのままにすると依存が輪になり、比べる相手も土台のものになって、比べる意味が無くなる。`cargo xtask deps` は dev-dependency を決まり 1〜3 の外に置くので、この例外はこの節で守る。

C.11 で rulec と dandori が `copies` の表を読むようになったので、`ritsu-emit` の比べる部分と rulec への dev-dependency を消した。`crates/ritsu-emit/tests/copies.rs` は、表を語の並びにしたものを `tests/golden/copies.txt` と比べるだけになった。D.10 で rulec と dandori が `ritsu-proto` で読むようになったので、`ritsu-proto` の比べる部分と、rulec と dandori への dev-dependency も消した。`crates/ritsu-proto/tests/readers.rs` は、三つの言語の形にしたものを `tests/golden/` の `sakai.txt`、`rulec.txt`、`dandori.txt` と比べるだけになった。これで、土台の層のクレートの dev-dependency に言語のクレートは無くなった。

E と F で足した dev-dependency は三つある。ritsu-cross は、判定を本物の答えで確かめるために rulec、koyomi、chobo を持つ（E.4）。`ritsu-model` は、言語と serde_json を `[dev-dependencies]` としてだけ持つ（11.3）。`ritsu` は `ritsu-wasm` を持つ（ページのテストがバイナリと突き合わせるため。ritsu-wasm は `ritsu` のライブラリに依存するので輪になるが、dev-dependency の輪は Cargo が許し、`cargo xtask deps` と地図も dev-dependency を数えない。8.7）。

### 3.4 決まりの確かめ方

- C の段階：`xtask` に、`cargo metadata` を読んで 3.1 の表と突き合わせる確かめを置き、CI の `fast` のジョブで走らせる。破れば落ちる。C.3 で `cargo xtask deps` として作った（10.9）。
- E の段階（E.8 で作った）：この処理系そのものの地図 `ritsu.ctx` を根に置き、コンテキストのファイルを `contexts/` に置いた。Rust では、`use` できるクレートは `Cargo.toml` の依存に限られ、コンパイラがそれ以外をエラーにするので、境界を越える参照は `Cargo.toml` の依存として読める。コンテキストは 12 個で、土台の五つのクレート（`Base`）、七つの言語（`Rules`、`Workflows`、`Calendars`、`Books`、`Claims`、`Requirements`、`ContextMaps`）、`ritsu-project`（`Project`）、`ritsu-cross`（`Borders`）、入口の `ritsu`（`Entry`）、`ritsu-testkit` と `xtask`（`Testing`）である。土台の五つのクレートを `Base` の公表された言語にし（sakai の `crate "…"`）、七つの言語と `Project`、`Borders`、`Entry` は `Base` に順応する（`upstream Base conformist`）。言語のクレートも、それぞれ自分のクレートを公表された言語にするが、それに関係を書くのは `Project` と `Entry` だけで、言語どうしには関係を書かない。だから、言語のクレートが別の言語のクレートに依存すれば、sakai が E201 で、依存を書いた `Cargo.toml` の行を名指す。土台が言語に依存するとき、`ritsu-cross` が言語のクレートに依存するとき（言語は口の実装を通してだけ使う）、どれかのクレートが `ritsu-testkit` や `ritsu` を `[dependencies]` に書くときも同じである（`Testing` と `Entry` は何も公表しない。dev-dependency は sakai が数えない）。
  - 依存の読み方は、sakai の決まり（sakai の P6「コードの import は、各言語の既存のツールの設定にして、そのツールで確かめる」）に合わせて、cargo-deny 0.20.2 の `[bans]` の `wrappers`（そのクレートに依存してよいクレートを並べる）の設定を書く形をまず試し、sakai が Cargo に尋ねる形（`cargo metadata --format-version 1 --no-deps --offline`）にした（sakai の DESIGN 7.7）。cargo-deny は、`[graph]` に `exclude-dev = true` を書けばワークスペースの中の依存にも効き、言語のクレートに別の言語のクレートを足した変異も止めたが、指す行は `deny.toml` の行で、依存を足した `Cargo.toml` の行ではなかった。「どのクレートも依存してはいけない」クレート（`ritsu`、`xtask`）は書けず（`wrappers = []` ではクレートそのものが禁止になり、自分を並べると `unused-wrapper` の警告が残る）、CI で取ってくる必要もある。
  - 地図は 3.1 の表より粗い。土台の中の向き（`ritsu-base` は std だけ、など）と、外のクレートが serde_json だけであることは、`cargo xtask deps` だけが確かめる。二つとも CI の `fast` のジョブで走らせる（7.13）。

### 3.5 捨てたもの

- **言語のクレートどうしが直接依存する形**（dandori が rulec に依存する）：rulec の中の型（AST、型付けの結果）が dandori に漏れ、rulec の中を直すたびに dandori が壊れる。口を挟めば、渡すものは決めた型に絞られる。
- **一つのクレートにまとめる形**：依存の決まりがモジュールの規約になり、コンパイラが守らない。
- **プロセスの境目を残し、JSON を型の付いた形式に直す形**：一つの処理系にする理由（型の付いたまま渡す）が無くなる。7 章の問い（「この集合で完全か」）は、渡すたびに子プロセスと JSON の往復になり、二つのツールのバージョンが違えば答えも食い違う。

### 3.6 依存の脆弱性、ライセンス、出どころ（2026-10-06）

ritsu が依存するものの既知の脆弱性、ライセンス、出どころを、CI の `audit` のジョブ（`.github/workflows/audit.yml`）で確かめる。確かめるのは次の二つで、どちらもネットワークが要るので、テストではなく CI で走らせる。

- **Rust のクレート**：cargo-deny 0.20.2 が、`deny.toml` のとおりに `Cargo.lock` を確かめる。アドバイザリ（RustSec のデータベース。脆弱性、保守されなくなったクレート、健全でない（unsound）クレート、crates.io から取り下げられたバージョン）、ライセンス（MIT、Apache-2.0、Unicode-3.0、BSD-3-Clause。memchr は「Unlicense OR MIT」で、MIT で足りる。Unicode-3.0 と BSD-3-Clause は 2026-10-06 に足した。rulec と koyomi が持つデータのためで、二つのクレートと `ritsu`・`ritsu-wasm` の `license` がそう言う。`unused-allowed-license = "deny"` なので、使わなくなれば検査が落ちる）、同じクレートの二つ目のバージョン、出どころ（crates.io だけ。ほかのレジストリと git は使わない）の四つである。`Cargo.lock` に載る外のクレート 14 個のうち、ビルドされるのは 8 個（serde_json、serde_core、indexmap、hashbrown、equivalent、itoa、memchr、zmij）で、残りの 6 個（serde、serde_derive、syn、quote、proc-macro2、unicode-ident）は、serde_json と serde_core が `cfg(any())`（どの対象でも成り立たない条件）の依存としてバージョンをそろえるために載るだけである。cargo-deny は cargo と同じくこの 6 個を外し、osv-scanner は `Cargo.lock` のとおりに 14 個とも調べる。
- **リポジトリのすべてのロックファイル**：osv-scanner 2.6.0（osv-scalibr 0.5.2）が、根から下のロックファイルを OSV のデータベースで調べる。2026-10-06 の時点で 32 のファイルで、`Cargo.lock`、`crates/*/tools` の `package-lock.json` が 12（778 パッケージ）、`requirements.txt` が 9（142 パッケージ）、`go.mod` が 10（ツールの二つと、テストの材料と例の八つ）である。この中に、`ritsu gen` が書くバージョンを固定しているもの（dandori の Temporal のランナーと chobo のランナー）がある（9.3）。

ジョブは、push（ブランチ）と pull request のたび、毎日（UTC 16:00）、手で始めたとき、それと `release.yml` がビルドの前に呼ぶ（13.2）。アドバイザリは、リポジトリが変わらなくても出るからである。二つのツールは、リリースのバイナリを決めたバージョンで取り、チェックサムで確かめて使う（`tools.yml` が wasm-tools と protoc を入れる形と同じ）。GitHub のもの（`actions/checkout`）のほかに action は使わない。

**配るバイナリが、含むクレートを言う（2026-10-06）。** リリースの `ritsu` は、cargo-auditable 0.7.7 でビルドする（`cargo auditable build`）。含むクレートの一覧（名前、バージョン、出どころ）が、圧縮した JSON としてバイナリの中のセクション（`.dep-v0`）に入る。使う人が、入れた ritsu を自分のスキャナーで調べられるようにするためである（コンテナのイメージに `.deb` で入れた場合など）。確かめたこと：serde_json だけに依存する小さなクレートを `strip = true` のリリースでビルドすると、cargo-auditable を使ったものは、`cargo audit bin` が「Found 'cargo auditable' data（9 dependencies）」と読み、osv-scanner 2.6.0 は `--experimental-plugins rust/cargoauditable` で 9 パッケージを読んで調べた（既定のプラグインでは読まない）。使わないものは、`cargo audit bin` がパニックの文から 3 個を拾うだけで（「not built with 'cargo auditable', the report will be incomplete」）、osv-scanner は 0 個だった。増えたのは 32 バイトだった。macOS では `otool -l` に、Linux では `readelf -S` に `.dep-v0` が出るので、`release.yml` はアーカイブを作る前にそれを確かめる。Trivy、Grype、Syft も読むと、それぞれの文書が言う（この機械では試していない）。

**脆弱性の知らせ方（2026-10-06）。** 根の `SECURITY.md` に、脆弱性は GitHub の非公開の報告（private vulnerability reporting）で知らせること、直すのは最新のリリースだけであること、リポジトリが自分の依存をどう確かめているかを、英語と日本語で書いた。報告のボタンは、リポジトリの設定で private vulnerability reporting を有効にすると出る。

通すもの：通すアドバイザリは、`deny.toml` の `ignore`（理由を書いた表の形）と `osv-scanner.toml` の `IgnoredVulns`（理由と期日 `ignoreUntil`）に書く。osv-scanner の期日を過ぎると、ジョブはまた落ち、誰かが理由を読み直す。

osv-scanner は `--no-resolve` で走らせる。`requirements.txt` が入れたい名前だけを書いていると、osv-scanner は deps.dev で依存を解決するが、その結果は誰も入れないバージョンになった（`tools/wire` では、idna 3.9.0 と setuptools 9.1.0 について、八つのアドバイザリを挙げた）。そこで、テストが入れる `requirements.txt` は、どれも依存の依存までバージョンを書いたものにして、osv-scanner は書いてあるとおりに読む。

テストでの確かめ（`crates/ritsu/tests/audit.rs`、fast の段）：ネットワークを使わずに、監査の答えが正しくなるための前提を確かめる。`crates/*/tools` の `package.json` の横に `package-lock.json` があり、`go.mod` の横に `go.sum` があり、`requirements.txt` のどの行も一つのバージョン（`name==version`）であること。`ritsu gen` がパッケージに書くバージョンと、生成したコードのコメントが名指すバージョンが、ツールのロックファイルのバージョンと同じであること（9.3）。通すアドバイザリのどれにも理由があり、osv-scanner のものは期日が一年より先でないこと。`audit.yml` が全体を毎日調べ、`release.yml` がビルドの前にそれを呼ぶこと。`release.yml` が四つのプラットフォームとも cargo-auditable でビルドし、それを動かす機械の cargo-auditable をチェックサムつきで取り、セクションを確かめること。

#### 2026-10-06 に見つかったものと、したこと

| どこ | 何 | したこと |
|---|---|---|
| `crates/chobo/tools/runner/go`（pgx v5.11.0 が求める） | golang.org/x/text v0.29.0、GO-2026-5970（正規化が不正な UTF-8 で止まらなくなる。v0.39.0 で直った） | v0.41.0 に上げた（それが求める golang.org/x/sync も v0.22.0 に）。dandori の `tools/temporal-go` と同じバージョン。govulncheck v1.7.0 で、pgx の SCRAM の認証（`pgconn.scramAuth` → `precis` → `norm`）から関数の単位で届くことを確かめ、上げたあとは消えた |
| `crates/dandori/tools/mermaid`（mermaid 12.0.0 の chevrotain が指す） | lodash-es 4.17.23、GHSA-f23m-r3pf-42rh（中）と GHSA-r5fr-rjxr-66jc（高） | mermaid の 12 系を 12.1.0 にした（chobo と sakai の `tools/mermaid` と同じ。ロックファイルの中身も同じになった）。lodash-es は 4.18.1 |
| `crates/dandori/tools`（`tools/asl-run.mjs`） | jsonata 2.0.6 の五つ（細工した式によるコードの実行が三つ、プロトタイプ汚染、`$toMillis` の入力によるリソースの枯渇） | 上げない。AWS の Step Functions は JSONata を 2.0.6 の仕様で評価し（AWS の開発者ガイド「Transforming data with JSONata in Step Functions」）、ランナーはそれをまねる。2.0.x の続きは出ていない。ランナーが評価するのは、dandori が書いた式と、テストのシナリオの入力だけである。`osv-scanner.toml` に理由を書き、期日は 2027-04-05 |
| `crates/dandori/tools/wire`、`crates/koyomi/tools` の `requirements.txt` | 入れたい名前だけ（boto3 と moto、mypy）で、依存はバージョンを書いていない。上の deps.dev の解決で、入れていないバージョンのアドバイザリが出た | `requirements.in` を置き、`uv pip compile --generate-hashes` で依存まで固定した（61 と 6 のパッケージ）。既知の脆弱性は無い |
| `Cargo.lock` | なし（cargo-deny、cargo-audit 0.22.2 の 1,290 件、OSV のどれでも） | なし |
| `crates/dandori/tools/temporal`（2026-10-06 の昼、GitHub で `audit` を初めて走らせたとき） | source-map-js 1.2.1、GHSA-68fv-2mgg-jv7q（高。索引つきのソースマップの行の数を確かめず、イベントループを止められる。2026-09-18 に出た） | 依存の範囲（`^1.0.2`）の中で 1.2.2 に上げた（`npm update --package-lock-only`） |
| `crates/*/tools/mermaid`（同じ回） | KaTeX 0.16.47、GHSA-238p-pmpm-9mq7（低。ほかのパッケージがすでに汚した `Object.prototype` があり、攻撃者が数式を書けるとき。2026-10-05 に出た） | 上げない。Mermaid 11.17.2 と 12.1.0 が `^0.16.47` を求め、直った 0.18.2 はその外にある。テストが描くのは言語が書いた図だけで、数式を含まない。`osv-scanner.toml` に理由を書き、期日は 2027-01-06 |

ほかの四つの dandori の `requirements.txt`（agents、connect、pydantic-graph、temporal-python）は、`uv pip compile` にかけて、もう依存まで書いてあることを確かめた（足りないものは無かった）。

#### 調べていないもの

- `fetch.sh` と `install.sh` で取ってくるバイナリ（TigerBeetle、sakai の Java のツールと Context Mapper、wasm-tools、protoc）。チェックサムで確かめているが、OSV では調べていない。
- Lean の依存（`proofs/lake-manifest.json`）。OSV に Lean のエコシステムが無い。
- ワークフローが使う action（タグで指している）。`release.yml` は GitHub のもの（checkout、upload-artifact、download-artifact）しか使わず、ほかの action を使うのは、読むだけの権限で、secret を持たないジョブだけである。
- 手元と CI のツールチェーンそのもの。この機械の Go 1.25.5 の標準ライブラリには、1.25.6〜1.25.10 で直ったアドバイザリが 30 ほどある（govulncheck の結果）。CI は `setup-go` の `1.25` で最新のパッチを入れる。
- yuen の `tools/requirements.txt`（prov と reqif の二つの名前だけ）。依存まではまだ固定していないので、osv-scanner は二つだけを調べる。

#### 捨てたもの

- **CI で cargo-audit も走らせる**：cargo-deny がアドバイザリに加えてライセンスと出どころも見る。osv-scanner も OSV（RustSec を取り込んでいる）で `Cargo.lock` をもう一度調べる。cargo-audit は、バイナリを調べる `cargo audit bin` のほか、足すものが無い。
- **osv-scanner の依存の解決（deps.dev）に任せる**：ネットワークの向こうの答えでバージョンが決まり、誰も入れないバージョンになった（上）。ロックファイルを置くほうが、CI が入れるものと同じになる。
- **cargo-deny の `wildcards = "deny"`**：七つの言語のクレートは、Cargo から見れば公開できるクレートで、土台のクレートをパスで指す。cargo-deny はそれをワイルドカードと数える。外のクレートの指定は serde_json の `"1"` だけで、`cargo xtask deps` がそれ一つに保つ。
- **jsonata を 2.2.x に上げる**：上の表。
- **google の `osv-scanner-action` の再利用できるワークフロー（SARIF を code scanning に上げる）**：入れない（2026-10-06 に決めた）。`security-events: write` の権限が要り、結果は GitHub の Security のタブに出る。読む人は一人なので、`audit` のジョブが落ちることで知らせは足りる。
- **Dependabot と Renovate**：入れない（2026-10-06 に決めた）。`audit` のジョブが毎日すべてのロックファイルを調べ、新しいアドバイザリが出れば落ちるので、知らせはそれで足りる。テストのツールには、わざと固定しているバージョン（Step Functions に合わせた jsonata 2.0.6 など）があり、二十を超えるロックファイルに更新の pull request が来ても、多くは閉じることになる。

参照した文書とバージョン（2026-10-06）：cargo-deny 0.20.2（2026-07-09、<https://embarkstudios.github.io/cargo-deny/checks/cfg.html> と各検査の `cfg.html`）、cargo-audit 0.22.2（2026-06-05）、osv-scanner 2.6.0（2026-09-14、<https://google.github.io/osv-scanner/supported-languages-and-lockfiles/>、<https://google.github.io/osv-scanner/configuration/>）、govulncheck v1.7.0（v1.8.0 は Go 1.26 が要る）、CycloneDX 1.7.2（2026-09-17）、cargo-cyclonedx 0.5.9、cargo-auditable 0.7.7（2026-10-02）、Trivy 0.75.0、Syft 1.54.0、Grype 0.120.0、<https://docs.aws.amazon.com/step-functions/latest/dg/data-transform.html>（JSONata 2.0.6）、<https://pkg.go.dev/vuln/GO-2026-5970>。


## 4. 土台

`ritsu-base` に置くものと、その元。どれも言語の意味を持たない。

### 4.1 二つの言語の文

`Text`（日本語と英語の文の組）と `tr!("日本語", "English")` を一つにする。koyomi、chobo、yuen、sakai の形で、どちらの文を出すかは出すところが決める。同じプロセスの中で英語と日本語の golden を取れ、wasm では呼ぶたびに言語を変えられ、ほかの言語のクレートからも、欲しい言語で呼べる。

- geas の `t(en, ja)` と dandori の `(en, ja)` の組は、C の段階で `tr!("日本語", "English")` の順に直す。機械的な直しで、文は一字も変えない。
- rulec の `tr!` は、プロセスで一つの言語を読む（呼び出しは 2,800 か所）。C の段階ではそのままにする。D の段階で、言語をスレッドごとに持てるようにする（`i18n::with(lang, || …)`）。dandori や yuen が rulec を同じプロセスの中で、ほかのテストと並んで、違う言語で呼ぶからである。CLI の振る舞いは変わらない。D.5 で作った（rulec の §15.165）。`with` の中で作る文はそのスレッドではその言語になり、抜ければ（パニックで抜けても）前の言語に戻る。スレッドの言語はプロセスの言語に勝つ。rulec の `tests/lang.rs` が、英語と日本語の `rulec doc` を四つのスレッドで同時に描いて確かめる。D の二つ目の部分で、rulec も `RITSU_LANG` を読むようにした（下の順。rulec の §15.168）。rulec のすべての文を `Text` に移すことは、要るとわかるまでしない（15 章）。
- 言語の選び方は、`--lang`、`<名前>_LANG`、`RITSU_LANG`、英語の順。システムのロケールは見ない（rulec の §11 の原則 7。生成物と CI のログが機械で変わらないため）。
- 文の幅（East Asian Width で W と F を 2 と数える）、件数、日本語の空白の詰め方の小さな関数も置く。

日本語の文の書き方は、2026-10-04 から 10-05 にかけて、全部の言語の利用者に見える日本語の出力を実物から読み直して、次のようにそろえた（作者がブラウザで試すページで日本語の出力の不自然さを指摘したため。rulec の分は下の段落）。読む人にしてほしいことは「〜してください」と書き、「〜を直します」「〜を書きます」のように、読む人がするのか道具がするのか分からない言い方をしない。道具がすることを言うときは、`sakai は`、`geas は` のように、する側を書く。`explain` の直し方の文も同じである。「〜ときも。」で切れた文は「〜ときにも出ます。」と閉じる。言語が事実を返せないことは「rulec から〜の情報を得られません」と言い（「rulec が〜に答えません」とは言わない）、規則から生成したコードは「規則から生成したコード」と言う（「規則の生成したコード」は、規則がコードを書くように読める）。範囲に上限や下限が無いことは「上限か下限が無い」と言い、「端が無い」とは言わない。言語の内側の部品の名前（口、読み手）は利用者に見える文に出さず、「ritsu を通して」「ritsu の共通のパーサー」と言う。sakai の腐敗防止層の対応は、ページでも診断でも「読み替える」と言う（「写す」と言っていたところをそろえた）。`ritsu check` の要約の結果は「どれも検査を通りました」と、文を閉じる（★）。英語の文は一字も変えていない（geas、sakai、ritsu-cross、ritsu、ritsu-base は日本語の読み直しの担当 ja-b）。

chobo と koyomi も同じ決まりでそろえた（ja-a）。chobo の `check` と `doc` のキーの行は「<振替> は、<引数> と <引数> の組ごとに一度だけ動きます」の形にした（★「一度だけ呼べます」にしないのは、同じ呼び出しの二度目は呼べて `done_before` が返るため）。chobo の `doc` のページは常体のまま、koyomi のページは敬体のままで、二つの文体はそろっていない（★）。yuen も同じ書き方にそろえた（ja-c）。yuen の設計の語「端」（リンクの両端と、そのハッシュを取る中身）は利用者に見える文に出さず、「リンク元」「リンク先」「両端」「要件のハッシュ」と言う（yuen の DESIGN 6.1。DESIGN の地の文の「端」は設計の語として残した）。dandori の日本語も同じ決まりでそろえた（ja-d）。dandori がすることを言うときは、「dandori は、値の範囲を…から求めます」「dandori は、Argo Workflows 向けの `on cancel` をまだ生成しません」のように、する側を書く。dandori が規則のまわりに出力するコードは「dandori が生成するコード」と言う（「書くコード」は作者が書くように読める）。名前の並びは、日本語の文の中では「・」でつなぎ（★doc のページの表のセルも。英語は `, ` のまま）、値が一つでも読めるように「ここで `押さえ` がとりうる状態（posted）では」の形にする。`--help` の各コマンドの短い説明（「〜を検査する」）は、CLI の書き方として言い切りのまま残した（★）。どの言語でも、英語の出力は替える前と後で突き合わせて一字も違わない。

rulec の診断と `--help` の日本語も、2026-10-05 に同じ決まりでそろえた（ja-e）。読む人にしてほしいことは「〜してください」と書き（「行を足します」「`2000g` と書きます」のような、誰がするのか分からない言い方をやめた）、道具がすることは「rulec は〜」と、する側を書く。英語の構文をそのまま持ち込んだ言い方（「契約は値がどう運ばれるかを言います」「どちらも同じ並びの終わり方です」「緑にはしません」）は、日本語として読める順に組み直した。規則の中身を決めてかかる言い方もやめた。E101 の「正しい額ではありません」は「正しい値とは限りません」に、E116 の「行の金額が」は「行の値が」に、`diff` の「何件がいくら動くか」は「何件の答えがどれだけ動くか」に、★`replay` と `diff` の見出しの「金額」（付与点のような金額でない出力にも出ていた）は「差の合計」にした。E032 の注は、言い方の担当が書き直した文を土台にして、「足します」「決めます」を「足してください」「自分で決めてください」にした。英語を漢字に直訳した語は、カタカナか普通の言い方にした（★群 → グループ、溢れ・あふれ → オーバーフロー、握手 → ハンドシェイク、代理（サロゲート）→ サロゲート、符号位置 → コードポイント、鍵（JSON）→ キー、ワイヤ形式 → 生成コードが受け取る入力と同じ形、読み手（JSON）→ パーサー、koyomi の口 → ritsu を通して、端が無い → 上限か下限が無い、覆いきっていない → 網羅していない）。★MCP のツールの説明は、`--lang ja` のときに英語のつなぎ（`Exit codes:`、`For example:`）が混ざっていたので、日本語のつなぎ（`exit code:`、`例:`）にした。`--terse` の最後の一行 `details: rulec explain <code>` は、rulec の決め（両方の言語で同じ一行）のとおり英語のまま残した。

rulec の `explain` の台帳と人が読むページの日本語も、同じ決まりで読み直した（ja-f）。rulec がすることは「rulec は」「`rulec check` が」と、する側を書き、診断の注に出るものは「注に〜が出ます」と言う。「——」でつないだ文は二つの文に分ける。英語を漢字に置いた語は、台帳のほかの項目とページがすでに使っている語にそろえた（束縛 → 同じ名前の `define`、群 → グループ、格子 → 刻み、反例 → 主張が破れる例、転記の誤り → 写し間違い、コンマ → カンマ、パーサ → パーサー）。規則の中身を決めてかからない（丸めで動くものを「円」と言わず、重なりを起こすものを「注文」と言わず、刻みに載らない値を「額」と言わない）。★ページの頭の刻印は「…から生成した資料です。読むためのもので、もとになるのは .rule のほうです。ここを編集しても .rule には戻せません」とし、「本物」とは言わない。★ページの中で常体だった文（優先の節、準用、節の行き先）は、ほかの文に合わせて敬体にした。台帳の見出しは診断の一行目と同じ文にする決まりなので、変えていない。英語の文は一字も変えていない。

rulec の日本語の出力の診断の見出しは、ほかの言語と同じく「エラー」「警告」で始める（`エラー[E101]`）。前は rulec だけが日本語の出力でも `error[E101]` と英語で書き、`ritsu check` の一つの出力の中で、rulec の `error[rulec E032]` と yuen や sakai の `エラー[yuen E203]` が並んでいた。英語の出力と `--format json` の `severity` は変わらない（`error`・`warning`）。yuen は、`.` を渡したときのまとめの行の名前を、渡した `.` ではなく見つけた `.req` のファイルにし（`requirements/billing.req: エラー 1 件`）、`.` の下で見つけたファイルの道の頭に `./` を付けない。

2026-10-05 に、日本語の文書と出力から「断る」「写す」を除き、文脈ごとに普通の言い方に替えた。言語ごとに担当を分け、置き換えの語は全部の言語で同じにした。帳簿（chobo）が操作を受け付けないこと（refuse）は「拒否する」「拒否される」、名詞は「拒否」と言う（「`在庫切れ` で拒否されます」「拒否されうる理由」。chobo の DESIGN 2.7 の見出しは「拒否の理由」）。言語をまたぐ検査、`ritsu run`、dandori の診断、README とサイトが chobo の拒否を言うときも同じ語を使い、sakai の `refuse`（腐敗防止層が値を受け取らないと決めること）も「拒否」と言う。生成したコードや道具が入力や呼び出しを受け付けないことは、文脈で「受け付けない」か「エラーにする」と言い、返すものが決まっているときはそれを書く（「規則から生成したコードは、前提を破る呼び出しを入口で受け付けません」「koyomi はどの名前も受け付けません」「protoc がエラーにします」「`invalid_argument` を返す」）。rulec の生成コードの入口には「拒否」を使わない。拒否は、一式の中では理由の名前が付く chobo の語だからである。コマンドが何も書かずに終わることは「エラーで止まる」「上書きせずに止まる」と言う。出典（法令の条、CSV などのファイル）を手元に保存したもの（copy）は「コピー」と言い、文から出典だと分からないときは「出典のコピー」と言う。それを取ってきて置くことは「保存する」、文書にある数字や文を規則に書き入れること（transcribe）は「転記する」、ファイルやフォルダーを複製することは「コピーする」と言う。上の ja-f の段落の「転記の誤り → 写し間違い」は、これで「転記の誤り」に戻った。名前に二つの語を持っていた日本語のテストの材料と変異（chobo の `拒否される.book`、koyomi の `helpers_エラーにする.cal` と `E101_コピーが無い.cal`、yuen の `E101_コピーが無い`、sakai の `E308_コピーの中身が違う`）、日本語の例のエラーの名前（dandori の例とブラウザで試すページの `拒否された`）、geas の例 greeter の主張の名前（「空の名前は受け付けない」。yuen の例が持つコピーと、その記録も取り直した）も替えた。残したのは記録だけである。dandori のテストのフロー `tests/flows/cancel.flow` のエラーの名前は、記録した Temporal の履歴がアクティビティの失敗の型として持っていて、古い履歴を新しいコードで再生できるかを確かめるテストがそれに頼る。ほかに、rulec の実験の実行の記録と、前の語を「」で引いている記録（この節、PLAN 7.10、rulec の §15.184）がある。英語の文は一字も変えていない。言語ごとに、例とテストの材料の全部にコマンドを英語と日本語でかけ、替える前と後で突き合わせた。英語の出力で変わったのは、コメントや名前を直した日本語の材料に由来するハッシュや識別子だけである。
### 4.2 診断

一つの `Diag` に、どの言語にも共通の部分を置く。コード、重さ（エラー、警告、備考）、ファイル、行、列、文、注（`= ` で始まる行）、直し方。テキストの形は、dandori、koyomi、chobo、geas、yuen、sakai がもう使っている形（`error[E301]: <ファイル>:<行>:<列>: <文>`、原文の行、注、そこに至る例）にする。

そこに至る例の部分は、言語ごとに中身が違う（1.2 の表）。そこは言語ごとの型にし、テキストと JSON への書き方だけを `Diag` が呼ぶ小さなトレイトで決める。

JSON は、キーを英語で固定し、`code`、`severity`、`file`、`line`、`col`、`message`、`notes`、`fix` と、言語ごとの部分のキーにする。`file` はルートからの相対で、JSON の外側に `root`（走らせたディレクトリから見たルート）を添える（6.2 の 9）。いまの形から変わるのは、chobo の `column`・`title`（→ `col`・`message`）、sakai の `file`（走らせたディレクトリから → ルートから）などで、変える言語の DESIGN.md に理由を書く（P6）。rulec の `--format json`（`v` が 2 の形）は変えない。rulec の診断は rulec の `src/diag.rs` に残し、`ritsu check` の JSON に入れるときだけ、共通のキーを外側に足す（8.3）。

テキストの中のパスの書き方は、6.2 の 8 のとおり。ファイルの場所は走らせたディレクトリから（渡されたとおりに）、文の中の名指しはルートからの相対で書く。

### 4.3 診断の台帳と explain

`Entry`（コード、重さ、一行の題、いつ出るか、直し方、最小の再現、隣に置くファイル、関連するコード）と、`find`、テキストと Markdown（コードごとのアンカーつき）と JSON の書き出し、そしてどの再現も自分のコードを出すことを確かめるテストの共通部分を置く。台帳の中身は言語ごとに残す。`docs/codes.md` と `docs/codes.ja.md` は、どの言語も `explain --all --format markdown` の出力そのものにする（いまもそうしている五つの形）。dandori の台帳と `explain` は D の二つ目の部分で足した（dandori の `src/codes.rs`）。dandori の診断コードの一覧は、サイトの一行ずつの表（`website/docs/reference/codes.md`）のままで、`explain` の Markdown にはまだしていない（サイトを ritsu に移す F で決める）。

ritsu 自身の台帳（言語をまたぐ検査のコード）は `crates/ritsu-cross/src/codes.rs` で、`ritsu explain` が引き、`crates/ritsu-cross/docs/codes.md` と `codes.ja.md` がその Markdown である（E.3。7.1）。

台帳の項は、再現を二つまで持てる（`Entry::english(Repro)`。英語を先にする担当が足した）。持たせると、`Entry::repro` が英語の再現になり、日本語の再現は `repro_ja` に残る。`explain` は `--lang en` で英語の再現を、`--lang ja` で日本語の再現を見せる（`Entry::repro_in(lang)`、`Entry::shown_in(lang)`）。一つしか持たない項は、これまでどおり、どちらの言語でも同じ再現を見せる。再現を走らせるテストの共通部分 `check_every` は、二つ持つ項では両方を一時ディレクトリ（`<コード>` と `<コード>-ja`）に置いて走らせ、どちらも自分のコードを出すことを確かめる。日本語の出力は変えていない。英語の出力に出る再現が、日本語の名前の例から英語の名前の例になったのは、決めて変えたことである（英語を先にする。10.10）。koyomi（14 項）、sakai（58 項）、yuen（41 項）が使った。ほかの言語の出力と、診断そのものの文（`check` の出力）は変わらない。`Entry::beside` は、いま見せている側の再現（`english()` のあとは英語の再現）に隣のファイルを足すので、日本語の再現に隣のファイルを足すときは `english()` の前に呼ぶ。rulec は自分の形の台帳（`crates/rulec/src/codes.rs`）を持つので、同じ考えを `Entry::english(example)`（隣のファイルが違うときは `english_with_files`）で持つ。`Entry::repro(lang)` が、英語の出力では英語の再現を、日本語の出力では元の再現を返す（55 項。rulec の DESIGN §15.179）。

利用者に見える出力には、設計の文書の節の番号を書かない（2026-10-04、作者の指示）。診断の文と注、`explain` の台帳（`docs/codes.md` と `docs/codes.ja.md`）、`--help`、`doc` のページ、生成したコードのコメントと文字列のどれも、`§15.59`、`DESIGN 1.17`、`dandori の DESIGN 1.17`、`ritsu の DESIGN 2.3` のような番号を持たない。言語をまたぐ検査の印（X2〜X6、「X3 の (a)」）も、台帳の文から外した（★どれも DESIGN の中の呼び名である）。番号は、読む人が持っていない文書を指すだけだからである。番号を外すと意味が欠ける文は、番号の代わりに中身を一言で言う（rulec の丸めの五つのモード、E118 の直し方の「以前の版では」、dandori の E006 の、使えない名前の一覧の「など」）。README、docs、DESIGN の中で設計の文書を指すこと、コードとテストのコメントに番号を書くことは、これまでどおりでよい。法令の節（`§1910.157`、`§1.7`）と MCP の `§SEP-1865` は番号ではなく引用なので、そのまま。★rulec のテストのコーパスの規則の `description` にある §（「(§15.132, §15.133)」など）は、規則そのものの文で、替えるとコーパスのハッシュが動くので残した。外したのは 391 か所である（rulec 286、ritsu-cross 40、chobo 33、koyomi 18、dandori 4、sakai 4、geas 2、yuen 2、ritsu-wasm 2）。

rulec の E032（取り込んだ列挙と宣言がずれている）の注は、列挙の書き方に合わせる。値を ASCII の名前で書く列挙（別名の付いた値が混じっていてもよい）には「この列挙に `returned` を足します」、値の名前を日本語などで書いて別名を付ける列挙には「`<名前>(returned)` の形で足します。名前は自分で決めます」と言う。前は、どちらにも `<名前>(returned)` と「日本語はそのファイルに入っていません」を出していた。最後の注は「契約（order.proto）に値が増えました。この規則がその値をどう扱うかは、まだ決まっていません」で、増えた値に金額を付けると決めてかからない。この注は、契約にだけある値があるときだけ出す（前は、契約から値が消えただけのときにも出ていた）。E033 の台帳の文も同じ考えで、「新しい値に既定の額が黙って当たる」を「既定の行の答えが黙って当たる」、「額を決めた人がいることの印」を「扱いを決めた人がいることの印」にした（rulec の DESIGN §15.183）。

退いたコードは、台帳に残して引けるようにし、番号を使い回さない（7.10）。D の最後の部分で、再現の代わりに退いた理由と版を持つ形（`Repro::Retired`、`Entry::retired`）を足した。`explain` は再現の見出しの下に理由を書き、再現を走らせるテスト（`check_every`）は退いたコードを飛ばす。yuen の E204 と W201 が最初に使い、sakai の N101 が続いた。

再現をディレクトリで持つ台帳（sakai）では、再現を走らせるコマンドを `explain` が書く（`sakai check .` を走らせます）。D.8 で、`ritsu` で始まるコマンドは、言語のコマンドの名前を前に付けずに、そのまま書くようにした。ほかの言語の成果物を含む再現は、すべての言語をつないだ `ritsu sakai check .` で走らせるからである（sakai の DESIGN 5.2）。

### 4.4 CLI の表

koyomi、yuen、sakai、chobo の形を一つにする。コマンドとフラグを一枚の表に置き、`--help` の表示と引数の読み取りが同じ表を引く。知らないフラグ、閉じた集合の外の値、値の無いフラグ、繰り返せないフラグの二度目は exit 2（rulec の §12.1）。繰り返せるフラグは表に書く（yuen の `--map`）。終了コードは 0（問題なし、警告と備考だけ）、1（エラー）、2（使い方の誤り、読めないファイル、中の異常）。

rulec の `src/main.rs` の表は同じ決まりで作られているので、C の段階では動かさない（合うところだけ移す）。dandori は C の段階でこの表に移し、`--version` とコマンドごとの `--help` を足す。足すだけで、いまのコマンドとフラグは変えない。

### 4.5 ハッシュ

SHA-256（FIPS 180-4）を一つにし、`hex` と、先頭 16 桁の `short` を置く。FIPS の既知の値でテストする。chobo の ID の決め方（長さを頭に付けた部分を SHA-256 に通す）は chobo に残し、SHA-256 だけを土台から使う。geas の SHA-1（git の blob の ID）は geas にしか要らないので geas に残す。

### 4.6 出典の固定と改正の検知

rulec、koyomi、yuen の三つは、法令のコピーを同じ場所と同じ名前で持つ（`sources/law/<法令の ID>@<時点>/<要素>.xml`。yuen の `src/copies.rs` の頭に、そうそろえたと書いてある）。そこで、次を一つにする。

- 引用の書き方から要素の名前を作る（e-Gov の `第143条第2項` → `MainProvision-Article_143-Paragraph_2`、`別表第一` → `AppdxTable_1`、附則。eCFR の節）。
- コピーの場所、コピーの本文（タグを落とした文、項ごとの行）、固定（先頭 16 桁）、`.rule`・`.cal`・`.req` の固定の行の書き換え。
- 通信：`curl` を子プロセスで呼び、三度まで試す（二度めの前に 2 秒、三度めの前に 4 秒）。e-Gov の法令 API v2（`law_data` と `law_revisions`）、eCFR（全文と版の一覧）、GitHub の raw のファイル（rulec）、e-Gov の応答の base64。

`source fetch | pin | outdated` のコマンドは、それぞれの言語に残す（固定の行の書き方は言語の構文だからである）。`check` は通信しない。

一つになれば、同じプロジェクトの規則とカレンダーと要件が同じ条を引くとき、コピーも一つで済む。yuen の E107（要件のコピーと規則のコピーの本文の食い違い）は、コピーが一つなら起きにくくなり、二つあるときも同じ手続きで比べられる（7.11）。

### 4.7 名指しとパス

名指しの決まり（6.2）を一つの実装にする。いまは yuen の `src/names.rs` と sakai の `src/naming.rs` が別々に書き、同じ 36 行の `naming.tsv` を通している。その表は土台のテストに移す。

パスは、ルートの決め方（`--root`、無ければ最初に渡したパスの上でいちばん近い `.git` のあるディレクトリ、それも無ければ渡したディレクトリ）、`.` と `..` を字の上で畳むこと、表示のパス（走らせたディレクトリから）、ディレクトリを歩くときに飛ばす名前（`.git`、`target`、`node_modules`、`.venv`、`.geas`、`__pycache__`）を一つにする。geas、yuen、sakai が別々に持っているものである。

12.5 で、取り込んだ形（ルートが上にある形）で yuen を走らせたら、コピーのパスを `../../crates/yurai/tests/…` と回り道で書いた（そのときの名前は yurai）。土台の表示のパスは、走らせたディレクトリからのいちばん短い相対にする（`paths::Shown`）。yuen の表示は、土台へ移す前に C.0 で直した（yuen の `Project::shown`）。

### 4.8 doc のページの枠

ページを読んで理解し確かめる人のためのページ（rulec、dandori、koyomi、chobo、yuen、sakai の `doc`。yuen と sakai の `doc` は F で作った）の枠を一つにする。

- HTML：`<!doctype html>`、`lang`、viewport、ツールとバージョンを書く `generator`、題、一つの CSS。CSS は色の変数を明るい配色と暗い配色で持ち、`prefers-color-scheme` と `data-theme` で切り替える。ページの頭に、元のファイルのパスと SHA-256 の先頭 16 桁とツールのバージョンを書く。外のファイルを読まない（スクリプトも CSS も中に書く）。狭い画面で横にはみ出さない。
- Markdown：頭のコメント（`<!-- Generated by <ツール> <バージョン> from <パス> (sha256:…) -->`）。

ページの中身（rulec の表とカード、dandori の図とシナリオ、koyomi の月の表、chobo の残高）は言語に残す。色の値をそろえると見た目が少し変わるので、C の段階で golden とスクリーンショットを取り直し、変わったページを報告に並べる。

段階 C で土台の枠に移したのは、koyomi と chobo のページである（C.4、C.5）。rulec と dandori の `doc` は、C.11 で出力を一字も変えない決まりなので、自分の CSS と頭のまま残した。元のファイルとハッシュとツールを頭に書くこと（HTML の頭と Markdown のコメント）は、四つとも言語ごとのいまの書き方のままである。一つの書き方にすると全部のページが変わるので、生成物の頭（9.5）と同じく E でそろえる。土台に置いていたその部品（`stamp` と `markdown_head`）は、使う言語が無いので C.11 で消した。E.7 でそろえたのは生成物の頭（9.2）だけで、doc のページの頭は、まだ言語ごとの書き方のままである。

F.1 で作った yuen の `doc` は、最初から土台の枠（`docpage::html_head`、`Palette`、`HTML_TAIL`）を使う。中身（トレーサビリティの表、出典と条文の引用、要件ごとの「なぜ」、範囲、確かめた記録）と、表と引用と診断の見た目の CSS は yuen のものである。頭に書くのは、元の `.req` のパスとハッシュと yuen のバージョンで、ほかの言語と同じく、いまは yuen の書き方のままである。ページの読み手は、コードが実現すべきものを理解し、確かめる人（事業を回す人、経理や法務、運用する人、コードを見る開発者）と、来歴を監査する人である。

sakai の `doc`（F.2 で作った）も、初めから土台の枠（`html_head`、`Palette`、`HTML_TAIL`）で書いた。ページの中身（コンテキストマップの図、コンテキストごとの成果物と用語集、関係と越える参照、対応の表）は sakai に残す。元のファイルとハッシュを頭に書く行は、ほかの言語がまだ自分の書き方のままなので、足していない（ページの最後に、書いた sakai の版と地図のファイルの名前を書く）。そろえるときに、一緒に足す。カレンダーのデータの範囲を出すため、sakai の口のまとまりに koyomi の `Dates` を足し、ritsu-project の `Joined::sakai` がそれを渡す（★。カレンダーのファイルそのものには `Dates` が答えないので、それを `use calendar` で読む日付のファイルが聞いた範囲を出す）。

ページとその読み手の言い方は、七つの言語でそろえた（2026-10-04、作者の決め。言い方の担当）。ページは、英語では "the page for people"（短く言うなら "the doc page"）、日本語では「人が読むページ」と言う。読む人は、コードが実現すべきものを理解し、確かめるために読む人（事業を回す人、経理や法務、運用する人、コードを見る開発者）で、短く言うなら "readers"、「読む人」「確かめる人」と言う。前は「承認する人」「approver」「for whoever approves」と言っていたが、ページは承認する人のためだけのものではない。チームで規則を承認する決まりがあるなら、その承認にもページが使える、という順で書く（根の README の「Pages for people」「人のためのページ」）。直したのは、言語の README、DESIGN、docs、スキル、rulec と dandori のサイトのページと図、ritsu のブラウザで試すページの、規則のページを言う文、`--help`（`rulec doc` と `koyomi doc`）、診断の文（rulec の W1xx の直し方）、`doc` のページの文（dandori の規則の節）、生成したコードの文（rulec の MCP のツールの、隣のページの説明）、コードのコメントである。替えなかったのは次のものである。業務の中の承認（dandori の審査の例の承認、`approve`・`approved`・`approver` のフィールド、chobo の返金の承認待ち、proto の `ApproveRequest` など）。CLI の値 `rulec doc --audience approver`（利用者のスクリプトが使う名前。`--help` は「既定は approver で、人が読む資料になる」と説明する）。yuen の見送りの承認（`approved … by`、E304）と持ち主の役割。rulec の `source pin` でコピーの変更を受け入れること（「この内容で承認するなら」）と、E037・E043 の承認。★規則を決める人の役割としての承認（rulec の AGENTS.md と README の「someone approves the table」、rulec の DESIGN 0.1 の「承認する人がいて」）。日付の入った決定の記録（rulec の DESIGN 15 章など。§15.183 に、この変更の記録を足した）と、テストの関数の名前。

### 4.9 JSON

土台は外のクレートに依存しない（P9）ので、診断や名指しの JSON は、std だけで書いた小さな JSON の値の型で書く。キーの順を保ち、整数を正確に持つ。元は rulec の `src/json.rs`（456 行）で、C.11 で rulec もこれに替えた。geas も依存の無い JSON の読み手（`src/json.rs`、513 行）を持つが、それは残した。geas が読むのは試す相手のプログラムの出力で、数を浮動小数点として比べ、指数（`2e3`）も読み、読んだ値を自分の形で書き戻す。土台の読み手は ritsu のツールが出す JSON のためのもので、指数と二度出るキーを受け付けず、整数を正確に持つ（geas の DESIGN 13.1）。serde_json を使う五つは、土台の JSON を文字列にして読み直すか、そのまま埋め込む。

serde_json を土台に入れない理由は、rulec と geas が依存の無いことを保っているからである。rulec の README は、依存が無いので `cargo install --path .` が何も取ってこないと書き、rulec の DESIGN §12.1 は引数のパーサを入れない理由に、依存を足さない方針を挙げている。ritsu 全体のバイナリには、いまと同じく serde_json が入る。

### 4.10 `.proto` の読み手

`ritsu-proto` は proto3 を読む。`syntax`、`package`、`import`（`public` と `weak` も）、入れ子のメッセージ、フィールド（`json_name`、`optional`、`repeated`、`map`、`oneof`）、列挙と値と番号、サービスとメソッド（ストリームかどうか）、サービスとメソッドのオプション（dandori の `(dandori.v1.workflow)` など）、Protovalidate の規則（`buf.validate.field` の整数の範囲と `required`。CEL の式は文字列のまま渡し、rulec の `src/cel.rs` が読む）、`buf.yaml` の依存と `buf.lock` の固定（rulec）。Google の well-known types、`buf/validate/validate.proto`、`dandori/v1/options.proto` はファイルが無くても知っているものとして扱う。型の名前は protobuf の決まり（内側から外へ、package を一段ずつ）で解決する（sakai の形）。

読み手が一つになれば、同じ `.proto` を、rulec の契約の突き合わせ、dandori の `connect` と `implements`、sakai の境界を越える参照、yuen の端が、同じに読む（7.12）。

### 4.11 土台に置かないもの

キーワードの表、字句と構文、名前の解決、検査、参照インタプリタ、生成器の芯は、言語に残す。どれも言語の語彙と意味そのものである。

### 4.12 段階 C で作った形（`ritsu-base`）

段階 C の最初の部分で `ritsu-base` を作り、二つ目の部分（C.4〜C.8）で koyomi・chobo・geas・yuen・sakai を、最後の部分（C.11）で rulec と dandori の合うところをこれに移した。移したときに出力が変わらないよう、重なっていたコードのうちいちばん広い形を取り、言語によって形が違っていたところは、言語が選べるようにした。koyomi・yuen・sakai の `explain`（テキストと Markdown。再現の三つの形を一つずつ）と、koyomi の `--help`、yuen の `review --help` は、土台で組み直したものが一字も違わないことをテストで確かめている（`crates/ritsu-base/tests/ledger.rs` と `cli.rs`。比べる相手は、移す前のそれぞれの出力を `tests/golden/compat/` に保存したもの）。

- `text`：`Text` と `tr!`（日本語が先）。`Lang::pick` は `--lang`、`<名前>_LANG`、`RITSU_LANG`、英語の順に読む（4.1）。文の出し方は二つある。yuen は書いたとおりに出し（`as_written`）、koyomi と sakai は英語の頭を大文字にし、日本語の中の英字のまわりに空白を入れる（`spaced`）。どちらを使うかは言語が決める。chobo の `{key}` の差し込み（`Text::sub`）、件数（`count`、`plural`）、幅（W と F を 2 と数える）も置いた。
- `diag`：`Diag<X: Extra>`。共通の部分（コード、重さ、場所、文、注、直し方）は土台が書き、言語ごとの部分（`Extra`）は、直し方の前に出す行（yuen のつながりと差分）、直し方の後に出す行（koyomi の計算の段、sakai の関わるもの、chobo の操作）、JSON のキー、文の出し方を自分で決める。テキストのファイルの場所（`file`）と、JSON に書くルートからのパス（`rel`）を別々に持つ（6.2 の 8 と 9）。行の無い診断は、JSON の `line` と `col` を `null` にする（移す前は、geas のほかに koyomi の E001 と yuen も `0` と書いていた。三つとも替え、geas の golden を一つ取り直した）。言語の部分は、JSON に `fix` のキーを持たないこともできる（`Extra::fix_key`。geas）。ファイルの無い診断（geas のコマンドラインの誤り）は、文に場所を書かず、JSON の `file` を `null` にする。直した行は、テキストでは前後の空白を落とし、JSON では渡されたまま書く（koyomi は字下げを含めて JSON に出している）。
- `ledger`：再現は三つの形のどれかにした（隣に置くファイルとまだ無い再現は、`Entry::beside` と `Entry::later` で台帳の項に足す）。ファイル一つと隣に置くもの（koyomi と yuen）、一つのディレクトリに置くファイルとそこで走らせるコマンド（sakai）、まだ無いもの。`check_every` は、全部の再現を一時ディレクトリに置いて走らせ、自分のコードが出たかを確かめる。
- `cli`：表の読み方の違いは二つあり、`Reading` で選ぶ。koyomi は `-5` を引数として読み、sakai と chobo は `--` で始まる値を受け付けない。chobo のために二つ足した。`--format=json` を一語として読むこと（`no_inline_values`）と、`--` で始まるものだけをフラグにすること（`single_dash_args`）である。使い方の行は、引数とフラグから組み立てるか、言語が手で書く（`Cmd.usage`）。何が誤りかは種類（`Misuse`）でも返し、言語が自分の文で言える（chobo）。`--lang` の説明（`lang_flag`）は、C.4 から `RITSU_LANG` を書く（`<名前>_LANG`、`RITSU_LANG`、英語の順）。
- `sha256`：`digest`、`hex`、`short`（先頭 16 桁）、`to_hex`。
- `naming` と `paths`：6.2 の決まりを一つの実装にした。何が悪いかは `ErrorKind` で返し（どの文字で起きたかも）、文は土台のものを一つ持つ。yuen と sakai はコードと文が違うので、移すときは種類から自分のコードと文を選べる。「含む」は二つ置いた。yuen の、自分自身を含まない `contains` と、sakai の、自分自身も含む `is_or_contains` である。ルートは、渡したパスを字の上で絶対パスにしてから `.git` を探す（sakai の形。yuen はシンボリックリンクをたどってから探していた）。歩くときに飛ばす名前は sakai の組（`.` で始まる名前、`node_modules`、`site-packages`、`__pycache__`、`target`）にした。表示のパスは、走らせたディレクトリからいちばん短い相対で書く（`Shown`）。
- `sources`：三つの中でいちばん広い形を取った。漢数字は百と千まで読み（koyomi と yuen の形。rulec は九十九までで、`第0条` も通していた）、本文は表の行と列も読む rulec と yuen の `xml_text`、条の読み下しは yuen の `article_lines` にした。固定の行の書き換えは三つ置いた。数字だけを替える `pinned`（koyomi と yuen の `source pin`）、診断が出す直した行の `fixed_pin_line`、コメントの前を空白二つにそろえる `pinned_spaced`（rulec の `source pin`）である。base64 は、標準の形だけを読む `base64_decode`（yuen）と、URL 用の文字やパディングの無い形も読む `base64_decode_lenient`（rulec と koyomi）。curl には `--compressed` を付けた（eCFR は付けないと 406 を返す。rulec が見つけたこと）。e-Gov と eCFR はベースの URL を持つ値（`Egov`、`Ecfr`）にし、ベースの URL をどの環境変数から読むかは言語が決める（`KOYOMI_EGOV` など）。本物の e-Gov と eCFR に問い合わせるテストは `RITSU_TEST_LEVEL=platforms` のときだけ走らせ、2026-10-03 に一度走らせて、民法 142 条と 29 CFR 1910.157 の本文がテストのコピーと同じことを確かめた。
- `docpage`：色の役割（`bg`、`fg`、`dim`、`line`、`soft`、`panel`、`code`、`accent`、`ok`、`warn`、`bad`）と、その明るい配色と暗い配色の値（chobo と dandori の値）を置き、ページは自分の色を足せる。ページの頭（`html_head`）と、外の URL を読んでいないかの確かめも置いた。元のファイルとハッシュとツールを書く部分（`stamp`）と Markdown の頭のコメント（`markdown_head`）も置いていたが、rulec と dandori の `doc` は出力を変えずにはこれを使えず（どちらも頭をそれぞれの形で書いていて、golden が変わる）、ほかに使う言語も無いので、C.11 で消した（4.8）。
- `json`：キーの順を保つオブジェクト、正確な整数（`i128`）、書いた桁のままの小数。書き出しは serde_json の `to_string` と `to_string_pretty` と同じバイト列になる（エスケープも同じ）。読み手は rulec のもの（指数は読まない、同じキーが二度あれば止める、入れ子は 256 段まで）。止まったときは、位置と何が悪いか（`Problem`）と土台の文を返す。rulec は C.11 で、種類から自分の文を選ぶ形にして、誤りの文を前のまま保った。

### 4.13 段階 C で作った形（`ritsu-proto`）

C.9 で `ritsu-proto` を作り、sakai をこれに替えた（rulec と dandori は D.10）。sakai の読み手を元にし、4.10 の全部を読む。

- 一つのファイル（`read`）：要素ごとに行を持ち、どの要素のオプションも書いたまま持つ（名前、値の文、protobuf のテキスト形式として読んだ値）。宣言した `oneof`、`syntax` を書いた行も持つ。読めないときは、何が悪いか（`Problem`）と位置を返し、文は読んだプログラムの名前を渡して作る（知らない `syntax` を言う文だけが、プログラムの名前を言う）。
- オプションの木（`value::tree`）：dandori の読み方で、拡張の名前で引く。同じフィールドを二度書けば並びになり、整数は整数、`true` と `false` は真偽になる。小数は書いた桁のまま持つ（dandori は f64 に直す。いまのどの `.proto` にも小数のオプションは無い）。
- Protovalidate（`validate`）：rulec の読み方のまま。比べる値に効かない規則は名前だけを `unread` に残す。
- `buf.yaml` と `buf.lock`（`buf`）：rulec の読み方のまま。
- 多くのファイル：sakai の形（渡したファイルだけを読み、import の行き先と型の解決を持つ `load`）と、dandori の形（入口のファイルから import の先を何段でも読む `load_from`。中身を渡されたファイルは、その文を読む）の二つ。型の名前は、どちらも見えるファイル（自分、import した先、`import public` の先）だけから引く。
- 読んだものから言語が作るもの（rulec の列挙の値の別名、sakai の何も設定していない値、dandori が proto2 を受け付けないこと）は、言語に残した。
- 三つの読み手と同じものを読むことは、三つのリポジトリの全部の `.proto` と、三つの読み手が自分のテストに使っていた例で確かめる（`crates/ritsu-proto/tests/readers.rs`）。違うのは、rulec の読み手が行の頭にしか `package` を見つけないことと、壊れたファイルを途中まで読むことだけで、どちらも新しい読み手の方が多く読む（PLAN の C.9）。

**D.10 で rulec と dandori を替えた形**。二つの言語は、自分の `src/proto.rs` の読み手を消して `ritsu-proto` で読む。言語に残したのは、読んだものから自分が作るものだけである。

- rulec：`proto::read(パス, 中身)` が、`ritsu-proto` の読んだものから、規則が契約に尋ねるもの（package、import、列挙と値、メッセージと`shape` のパスがたどるフィールド、Protovalidate の規則）を取る。列挙の値の別名、`upper_snake`、整数の範囲（`int_bounds`）は rulec に残した。`src/proto.rs` は 1,358 行から 569 行になった。
- **rulec は、読めない `.proto` を途中まで読まない**（★、rulec の §15.166）。前の読み手は、読めない文を飛ばして読めたところまでを返していた。最後の `}` が欠けた契約は、そこまでの値で突き合わせて通り、値の行の `=` が抜けた契約は、その値が消えたと E032 で言っていた。いまは E013（「`order.proto` を読めません」）で止め、どこで何が読めないかを注に書く。コーパスと変異にもテストにも壊れた契約は無く、出力は 1,782 回とも変わらない。
- dandori：import をたどる部分は dandori に残した。ファイルを、ディスクからも、ブラウザで試すページが持つファイル（`Sources`）からも探すからである。たどった一つずつのファイルを `ritsu_proto::read` で読み、型の名前は `Protos` で、見えるファイル（自分、import した先、`import public` の先）だけから引く。proto2 と editions は受け付けない。読めなかった import があるときは、引けない名前を書いたまま持つ（その import の先の型を、無い型としてエラーにしない）。`src/proto.rs` は 1,145 行から 551 行になった（1.2 の表の 1,153 行は元のリポジトリのもので、C で単体テストの一時ディレクトリを替えて 1,145 行になっていた）。読めないファイルの文の形が変わった（dandori の DESIGN 0.3）が、例と `tests/flows` の出力は 701 回とも変わらない。
- `ritsu-proto` の文：何が要るか（`Problem::Expected` の `what`）を、英語の語から二つの言語の文にした。日本語の文に英語の語が混ざっていた（「a name が要るところに」）のが、「名前が要るところに」になる。sakai の E106 の日本語の文はこれで変わるが、sakai の golden には当たるものが無かった。`tests/golden/sakai.txt` の一行を取り直した。
- 三つの読み手と生のまま比べるのはやめ、`tests/readers.rs` は三つの golden と比べる形だけを残した（3.3）。比べるのをやめる前に、替えたあとの rulec と dandori の読み方と生のまま比べて、三つの golden の全部と同じことを確かめた。

### 4.14 統一形式の差分（`ritsu_base::udiff`、D の最後の部分で足した）

`git diff` と `diff -u` が書く差分の読み手を、土台に一つ置く。geas の `affected`（差分がどの主張に触るか）と yuen の `affected`（差分がどの要件に触るか）が、同じ差分を読むからである。geas の `src/diff.rs` の読む部分（ファイルの前と後のパス、引用符で書いたパス、改名と追加と削除、`index` の行の blob、二進のファイル、ハンクとその行、「末尾に改行が無い」の行）と、ディスクのファイルが差分の前と後のどちらに合うかを決める部分（`Content`、`fits`、`other_side`）を、中身を変えずに移した。blob のハッシュ（git の blob の形）は geas に残した。geas はそれを `pub use` で使い、振る舞いは変わらない（geas の `affected` のテストの golden がそのまま通る）。yuen は、`.req` のどの行が変わったかを、ディスクのファイルが差分のどちら側かを決めてから読む（yuen の DESIGN 8 章）。

### 4.15 ファイルの読み書きの入口（`ritsu_base::fs`、F.5 で足した）

ブラウザで試すページ（8.7）にはディスクが無い。言語がプロジェクトのファイルを読むところを、`std::fs` から `ritsu_base::fs` に替えた。関数の名前と答えは `std::fs`（と `Path::exists`・`is_dir`・`is_file`、`std::env::current_dir`）と同じで、`fs::with` の外では `std::fs` そのものである。`fs::with(ファイル, …)` の中では、そのスレッドで、渡されたファイルを読み書きする。ritsu-wasm が渡すのは `Memory`（メモリに持つファイルと、それが入るディレクトリ。書いたものも覚える）で、無いファイルやディレクトリには、ディスクと同じ文（`No such file or directory (os error 2)` など）で答える。診断がその文を引くとき、ページとコマンドで同じに読めるようにするためである。替えたのは rulec 27 か所、yuen 28、sakai 31、koyomi 12、dandori 7、geas 6、chobo 1、ritsu-base 8、ritsu-project 6、ritsu-proto 4、ritsu-cross 1 である。

決まり：check、gen、doc の道筋で、プロジェクトのファイルを読み、gen が書くコードは、`ritsu_base::fs` を通す。`std::fs` を直接使うと、ページの中ではそのファイルが「この対象ではできない」で読めず、ページの答えがコマンドとずれる（`crates/ritsu/tests/playground.rs` が、ページのプロジェクトで通る道筋なら、そのずれを見つける）。各言語のコマンドだけが使う読み書き（`source fetch`、`yuen review`、`rulec replay`、テストのランナー）は `std::fs` のままでよい。

同じ理由で、パスが絶対かどうかは `ritsu_base::paths::rooted` で見る。wasm32-unknown-unknown の std の `Path::is_absolute` は、ドライブの無いどのパスにも false を返すからである。geas は、Unix にしかないもの（プロセスグループ、シグナル、実行の許可のビット）を `#[cfg(unix)]` の中に入れ、ほかの対象では何もしない形にした。Unix での振る舞いは変えていない。

捨てたもの：wasm32-wasip1 で、ページが WASI のファイルの呼び出しを JavaScript で肩代わりする形。言語には一行も触らずに `std::fs` が動くが、PLAN の F.5 が wasm32-unknown-unknown と「バッファの頭に長さを書く」決まりを決めていて、肩代わりする JavaScript が、std がどの WASI の呼び出しをするかに追いつき続けるもう一つの実装になる。

### 4.16 OpenSpec の読み手（`ritsu_base::openspec`、2026-10-05 の夜に足した）

OpenSpec（Fission-AI の `@fission-ai/openspec`。2026-09-30 の 1.14.0 で確かめた）の仕様と変更の提案を読む読み手を、土台に一つ置く。yuen が仕様の要件を出典として要件ごとに固定し（yuen の DESIGN 20 章）、geas が仕様のシナリオを同じ名前の主張と突き合わせる（geas の DESIGN §17）ので、二つの言語が同じ読み方で読む必要がある。

読むのは、仕様（`openspec/specs/<capability>/spec.md`）の `## Requirements` の下の要件（`### Requirement: <名前>`）のブロックとシナリオ（`#### ` の見出しで本文のあるもの）、変更の提案の差分（`openspec/changes/<id>/specs/<capability>/spec.md`）の ADDED・MODIFIED・REMOVED・RENAMED の四つの節、`openspec/` の置き方（仕様のパスから capability と変更の提案の場所を引き、`changes/archive/` を除いた提案を並べる）である。決まりは OpenSpec 自身の読み手（ソースの `src/core/parsers/`）と同じにした：コードブロックの中の行は見出しにしない、CR LF を LF として読み BOM を落とす、要件の名前は `Requirement:` のあとの文字列から見出しの末尾の `#` の並びを除いて前後の空白を落とし、書いたとおりに比べる、要件のブロックは見出しから次の要件か `## ` の行の手前までで末尾の空白を落とす（archive が MODIFIED で置き換える範囲）、archive は RENAMED・REMOVED・MODIFIED・ADDED の順に当てる。

確かめ方：OpenSpec 1.14.0 の読み手（`extractRequirementsSection`、`MarkdownParser`、`parseDeltaSpec`）を npm で入れて node で呼び、同じファイルから作ったブロックとシナリオの名前と差分を `crates/ritsu-base/tests/fixtures/openspec/expected.json` に置いた（作り方は隣の `expected.mjs`）。`tests/openspec.rs` は、土台の読み手がそれと一字も違わないことを確かめる。テストは Node も OpenSpec も要らない。材料には、コードブロックの中の見出し、見出しの末尾の `#`、CR LF と BOM、`Scenario:` の無い四段の見出し、本文の無いシナリオ、要件でない三段の見出し、`*` と `+` の箇条書きの RENAMED と REMOVED、対の無い `FROM:` を入れた。

捨てたもの：`openspec` の CLI（`openspec show --json`）を子プロセスで呼ぶこと。`yuen check` と `geas scenarios` が Node と OpenSpec を要るようになり、yuen の `check` は何も走らせないという前提（yuen の P4）から外れる。`show --json` の要件の `text` はシナリオを含まないので、要件の端にもそのままは使えない。

### 4.17 YAML の読み手（`ritsu_base::yaml`、sakai が OpenAPI と AsyncAPI の文書を読むときに足した）

YAML と JSON を、値ごとに行と列を持つ値（`yaml::Node`）に読む。読むのは、RFC 9512（YAML のメディアタイプ）の 3.4 節の言う、JSON と行き来できる YAML 1.2 で、OpenAPI 3.2 と AsyncAPI 3.1 が文書に勧めるものである。ブロックとフローのマップとシーケンス、四つの書き方のスカラー（プレーン、一重引用符、二重引用符、`|` と `>`）、コメント、`---` と `...`、`%YAML 1.2`、アンカーとエイリアス（エイリアスはコピーとして読む）、JSON の型に当たるタグ（`!!str` など）と `!` を読む。値の型は YAML 1.2 のコアスキーマで、マップのキーは書いたままの文字列（フェイルセーフのスキーマ）にする。二つ目の文書、`%YAML 1.1` と `%TAG`、ほかのタグ、`?` のキー、スカラーでないキー、同じキーの二度書き、自分の中を指すエイリアス、`.inf` と `.nan`、字下げのタブは、読まずに、どの行の何が読めないかを二つの言語で返す。`.json` は JSON の決まりで読む（同じキーの二度書きは止める）。JSON Pointer（RFC 6901）で値を引ける。

確かめ方は、YAML の公式のテストスイート（yaml-test-suite の data-2022-01-17、MIT。`crates/ritsu-base/tests/fixtures/yaml-test-suite.json` に一つのファイルにして持ち、ライセンスの文は隣の `yaml-test-suite.LICENSE`）の全部のケースにかけることである。2026-10-06 の回で、402 ケースのうち 204 をスイートの JSON と同じ値に読み、104 を読まずに止め、YAML でない 94 をどれも止めた。違う値を返したケースは無い。手では、Stripe と GitHub の OpenAPI の YAML（6.6 MB と 9.9 MB）を、js-yaml 4.3.2 のコアスキーマと同じ値に読むことも確かめた（リリースのビルドで 59 ms と 93 ms）。

土台に置いたのは、どの言語の意味も持たず（4.11）、rulec の `import jsonschema` と dandori の `use openapi` も同じ文書を読むからである（二つとも、いまは JSON だけを読む）。土台は std だけで書く決まり（P9）なので、YAML のクレート（yaml-rust2 0.13.0、saphyr 0.1.0、serde-saphyr 1.3.0。serde_yaml は 2024 年に保守を終え、serde_yml は非推奨）は使わなかった。くわしくは sakai の DESIGN 15.3。


### 4.18 Cedar の読み手と書き手（`ritsu_base::cedar`、2026-10-06 に足した）

認可の標準の言語 Cedar（<https://github.com/cedar-policy/cedar>）のポリシーとスキーマを読み書きする部品を、土台に一つ置く。ritsu は Cedar を、proto や OpenAPI と同じく標準の形式として読み、その上に言語をまたぐ確かめ（公開する操作と action の突き合わせ、ワークフローの最小権限、yuen の要件とポリシーの結び付け）を載せる。八つ目の言語 sekisho（`.gate`）は、Cedar のポリシーとスキーマを書き出す。どちらも同じ読み方と書き方を使う必要があるので、土台に置いた。どの言語の意味も持たない（4.11）。

読む版は Cedar 4.13.0（2026-09-15。`cedar-policy-cli` 4.13.0 で、`cedar language-version` が言う言語の版は 4.5）である。2026-10-06 に、crates.io と GitHub のリリース（<https://github.com/cedar-policy/cedar/releases>）で、これが最新であることを確かめた。

読むもの：

- ポリシー（`.cedar`）：`permit` と `forbid`、スコープ（`principal`・`resource` の `==`・`in`・`is`・`is … in`、`action` の `==`・`in`・`in [ … ]`）、`when` と `unless`、式の全部（演算子の優先順位、`has`（`a.b.c` の形も）、`like`、`is … in`、`in`、`if … then … else`、集合とレコード、メソッド（`contains` など六つと、拡張の 18）、拡張の関数（`decimal`、`ip`、`datetime`、`duration`、`unknown`））、注釈、コメント、テンプレート（`?principal`、`?resource`）。式ごとに行と列を持つ。ポリシーの id は CLI と同じく、`@id("…")` の値か、ファイルの中の順の `policy<n>` にする。
- スキーマ：人が読む形（`.cedarschema`）と JSON の形の両方。名前空間、`entity`（`in`、属性、`tags`、`enum`）、`action`（`in`、`appliesTo` の `principal`・`resource`・`context`）、`type`（共通の型）、注釈。モデルは JSON の形のもの（Cedar の `json_schema::Fragment`）にした。`cedar translate-schema` は、二つの向きとも、これを通るからである。名前と属性は書いた位置を持つ。

書くもの：

- ポリシーの JSON の形（Cedar の EST）。`cedar translate-policy --direction cedar-to-json` と一字も違わない。
- `cedar format`。Cedar のフォーマッター（`cedar-policy-formatter`）が具象構文木から文書を組む手順と、コメントをトークンから取る順番、文書を行に割り付けて出す pretty 0.12.5 の手順（グループを一行に収めるかは、その後ろの、次に改行できる位置までの長さで決まる。改行の後の字下げは、次の文書のものになる）を、そのまま書いた。幅と字下げの幅は引数で渡す。
- 書き手（`write_policies`）：構文木を、文法が要る括弧だけを付けた Cedar のテキストにし、上の整形にかけて出す。だから出力は `cedar format` が直さない形で、読み直すと同じポリシー（同じ JSON）になる。sekisho はこれでポリシーを書き出す。
- スキーマの JSON の形と人が読む形。`translate-schema` の二つの向きと一字も違わない。人が読む形で書けないもの（名前のある名前空間で、エンティティタイプと共通の型が同じ名前のもの、shape がレコードでないもの）は、CLI と同じく書かずにエラーにする。

読めないときは、何行目の何列目の何が読めないかを、二つの言語で返す（`ritsu_base::yaml` と同じ形の `Error`）。公式の CLI が読めないものは読まない。Cedar が構文木を作るときにする確かめ（予約語、`__cedar`、スコープに書けるもの、action の型が `Action` であること、メソッドと関数の名前と引数の数、`i64` に収まる整数、エスケープ、同じキーと同じ注釈と同じ id、スロットはスコープにだけ、スキーマの同じ名前の宣言と名前空間、共通の型に使えない名前、`appliesTo` の `principal` と `resource`）は、全部した。

確かめ方は、OpenSpec の読み手（4.16）と同じ形にした。公式の CLI 4.13.0 に材料を通した答え（JSON の形、`cedar format` の出力（幅 80・字下げ 2 と、幅 40・字下げ 4）、スキーマの二つの向きの変換）を `crates/ritsu-base/tests/fixtures/cedar/` に置き、作り方を隣の `expected.sh` に書いた。`tests/cedar.rs` は、土台の読み手と書き手が、それと一字も違わないことを確かめる。テストは Cedar もネットワークも要らない。材料は英語が先で、日本語の版（`ja.cedar`、`ja.cedarschema`）を横に置いた。

- 自分で書いた材料：ポリシーが 16（演算子の優先順位、文字列のエスケープと Unicode、コメントのあらゆる位置、CR LF、空白とタブ、Unicode の空白とひとつだけの CR、空のファイル、コメントだけのファイル、注釈、テンプレート、拡張の関数とメソッド、入れ子のレコード、名前空間つきの名前、予約語に見えるが読める名前、80 列を超える行と全角の文字）、スキーマが 6（人が読む形 4、JSON の形 2）。
- Cedar のリポジトリの材料（`upstream/`。Apache-2.0 で、ライセンスと NOTICE を隣に置いた）：フォーマッターのテストの入力 21 と、CLI の sample-data の sandbox_a〜c のポリシー 10 とスキーマ 3 組。
- 書き手：各ポリシーのファイルから書いたもの（`written/`）を、`expected.sh` が CLI で確かめる（`cedar format --check` が通り、JSON が元と同じ）。コードで組んだ木（四つ続く `-` の後の負の数、五つ続く `!`、予約語の属性、空白を含む `has` の名前、`\*` を含むパターンなど）も、読み直すと同じ JSON になることを確かめる。
- 読めない材料：ポリシーが 86、スキーマが 70（人が読む形 40、JSON の形 30）。`expected.sh` が、CLI がどれも読まないことを確かめ、CLI の文を `.err` に残す。土台も全部を読まない（文までは合わせていない）。

2026-10-06 には、テストに入れていない突き合わせもした。Cedar のリポジトリの v4.13.0 にある `.cedar` の全部（88。CLI が読めない 1 を含む）、`.cedarschema` の全部（66）、パスに `schema` を含む JSON（15。CLI が読めない 1 を含む）で、JSON の形、整形、書き手、スキーマの二つの向きが CLI と一字も違わず、CLI が読めないものは土台も読まなかった。乱数で組んだポリシー 1,800（コメントをトークンの間に入れ、幅を 20〜129、字下げを 1〜5 に変えた。どれも CLI が読める）、それを一か所ずつ変えた変異 1,600（トークンを消す、二つにする、別のものに替える、足す。CLI が読めたのは 135 で、読めない 1,465 は土台も読まない）、乱数で組んだスキーマ 600（CLI が読めたのは 274）でも、違うものは無かった。

速さ：約 50 万バイトのファイル（1,600 のポリシー）を、リリースのビルドで 57〜74 ms で読んで JSON の形にし、67〜75 ms で整形する（二回測った。同じファイルの `cedar format` は 6.1 秒と 13.8 秒。ほかの担当が同時にビルドしているので、時間は揺れる）。Cedar のフォーマッターは、コメントを探すたびにトークンの並びを頭からたどるが、ここでは並びが位置の順であることを使って二分探索にした（答えは同じ）。

捨てたもの：

- `cedar-policy` のクレート（4.13.0）に依存すること。土台が std だけで書く決まり（P9）から外れ、lalrpop-util、serde、miette などを引き込む。rulec と geas は依存の無いことを保っている（4.9）。
- 公式の CLI を子プロセスで呼ぶこと。`ritsu check` と sekisho の生成が Cedar の CLI を要るようになる（OpenSpec で捨てたのと同じ理由。4.16）。
- 書き手を、構文木をそのまま文字列にする形にすること（`translate-policy --direction json-to-cedar` が使う形）。Cedar のその書き方は、`>` を `!(… <= …)` に、`unless` を `when` の否定に、`is … in` を二つの式に替え、括弧を全部に付ける。人が書く形から離れるので、フォーマッターを通す形にした。

まだやっていないこと：

- JSON の形のポリシーを読むこと（書くだけ）と、テンプレートのリンク（`templateLinks`）。テキストの形には書けないので、読むのは JSON の形のときに足す。
- エンティティとリクエストの JSON、スキーマの名前の解決（人が読む形の名前が、エンティティタイプか、共通の型か、`Long` などの組み込みの型かを決めること）。Cedar の `translate-schema` も、名前は書いたまま出す。
- ASCII でない文字の幅は、東アジアの全角、ハングル、絵文字を 2、結合文字などを 0 とする表で数える（`cedar format` が使う unicode-width の表の全部ではない）。珍しい文字を含む文字列が行の幅の境にあると、改行の位置が `cedar format` と違うことがある。スキーマの人が読む形のエスケープ（Rust の `escape_debug`）も、表示しない文字の表は主なものだけである。

参照したものとバージョン（2026-10-06）：Cedar 4.13.0 のソース（文法の `cedar-policy-core/src/parser/grammar.lalrpop` と `cst_to_ast.rs`、JSON の形の `est/`、スキーマの `validator/cedar_schema/` と `validator/json_schema.rs`、フォーマッターの `cedar-policy-formatter/src/pprint/`、CLI の `cedar-policy-cli/src/command/`）、pretty 0.12.5（crates.io）、cedar-policy-cli 4.13.0 のリリースのバイナリ（<https://github.com/cedar-policy/cedar/releases/tag/cedar-policy-cli-v4.13.0>）、<https://docs.cedarpolicy.com/policies/syntax-grammar.html>、<https://docs.cedarpolicy.com/policies/json-format.html>、<https://docs.cedarpolicy.com/schema/json-schema.html>。

### 4.19 鍵の形、URL、秘密の印（`ritsu_base::secrets`・`urls`・`marks`、2026-10-06）

セキュリティの検査（16 章）が言語をまたいで同じ決まりを使うために、三つを土台に置いた。どれも、どの言語の意味も持たない（4.11）。

- `secrets`：16.3 の表の九つの種類を、ファイルの全文（文字列もコメントも）から探し、行と列と、伏せた形（接頭辞と `…`、秘密鍵はその頭の行）と長さと、行に `ritsu: test secret` があるかを返す（`scan`）。正規表現のクレートは使わず、gitleaks の既定の規則の正規表現の意味を、接頭辞、文字の種類、長さ、前後の区切りで書いた（P9）。gitleaks の規則が前後を決めていない種類（GitHub のトークン、Slack のトークンと Incoming Webhook の URL）も、前後が英数字に続くものは鍵としない。`mask` は、テキストの中の鍵（テスト用の値も）を伏せた形に替える。診断が見せる行は、どの言語でもこれを通る。土台の `Diag::source` を使う koyomi、yuen、sakai、ritsu は土台の中で、自分の診断の型で行を見せる rulec、dandori、chobo、geas はそれぞれの中で通す。だから、どの診断も鍵を行ごと出さない（16.3）。
- `urls`：URL のスキームとホスト（`scheme_and_host`）、ループバックか（`is_loopback`：`localhost`、`.localhost` で終わる名前、127.0.0.0/8、`::1`、`::ffff:127.0.0.1` のような IPv4 のループバックを表す IPv6 のアドレス）、暗号化しない通信か（`plaintext`：`http://` と `ws://` で、ループバックの外）、AsyncAPI の `protocol` の、暗号化して通信するプロトコルの名前（`encrypted_form`）。
- `marks`：OpenAPI・AsyncAPI・JSON Schema のスキーマのプロパティが秘密の印を持つかを決める（`schema_mark`）。`x-data-classification` は `sensitivity` が `confidential` か `restricted` のとき（書かなければ `confidential`）、`x-sensitive-data` はあれば、`format: password` もあれば印にする。dandori（serde_json）と sakai（土台の YAML）が、自分の JSON から三つのキーワードを読んで渡す。

`ritsu-proto` は `extend` を読むようにした（前は読み飛ばしていた）。トップレベルとメッセージの中の `extend` のフィールドを `ProtoFile::extensions` に入れ、メッセージの中で宣言したものはメッセージからの名前（`Holder.tag`）にする。フィールドとして読めない文は、前と同じく読み飛ばす。`Protos::redaction` は、フィールドが `debug_redact` の印を持つかを、直に書いたもの（`[debug_redact = true]`）と、`debug_redact` を付けた列挙の値をカスタムのオプションで付けたもの（`[(acme.v1.sensitivity) = PERSONAL]`）の両方で答える。オプションは、フィールドのファイルが見るファイル（自分、import するもの、それが `import public` するもの）の `extend google.protobuf.FieldOptions` から、protobuf の名前の決まり（フィールドのメッセージから外へ）で探し、列挙の型は `extend` を書いたところから探す。三つの読み手の golden（`tests/golden/` の三つ）は変わらない。


## 5. 単位の型

### 5.1 rulec の書き方を土台にする

`ritsu-units` は、rulec の単位の書き方と表をそのまま土台にする。

- 書き方：`<次元>[<単位>]`（`mass[kg]`、`length[cm]`、`duration[h]`）、お金は `money[<通貨>, incl_tax|excl_tax]`、率は `rate` と入力の `rate[step <刻み>]`、単位の無い数は `number`。
- 閉じた表：通貨（ISO 4217 の 31 のコードと `円`。`JPY` は `円` の別の綴り、`銭` は円の百分の一、`<コード>c` はその通貨の百分の一）、質量、長さ、面積、体積、時間（基準の単位への係数は正確な有理数）、温度と音量（順序だけの次元。℉ は一次式）。rulec の `src/types.rs` の `money_unit`・`unit_info`・`unit_offset`・`CURRENCIES` と、`src/num.rs` の有理数を移す。
- 決まり：通貨ごとに別の次元（為替の率はこの処理系の中に無い。rulec の §15.18）。換算は正確な有理数でだけ行い、整数にならない換算はエラーにする（`1lb` は `mass[g]` に書けない）。

`ritsu-units` が答えるのは、二つの単位が同じか、正確に換算できるか、順序だけの次元か、決まった綴り（`money[円, incl_tax]`）は何か、である。演算の型の決まり（rulec の E103 の単位の混同、E048 の日付の足し算、E104 の丸め）は rulec の検査のもので、rulec に残す。

単位の型は、rulec の型が持たないものを二つ持つ。率の刻み（rulec ではいま入力の宣言にあり、型には無い。1.4）と、chobo の名前だけの単位（5.4）である。

```rust
// ritsu-units（D.1 で作った形。crates/ritsu-units/src/unit.rs）
pub enum Dim { Mass, Length, Area, Volume, Duration, Temperature, Sound, Money(String), Rate, Number, Count(String) }
pub struct Unit { pub dim: Dim, pub unit: String, pub tax: Option<Tax>, pub step: Option<Rat> }   // unit は書いたとおりの綴り、step は整数一つが数える単位の割合
pub enum Tax { Incl, Excl }
```

**D.1 で作った形**。`ritsu-units` は、有理数 `Rat`（rulec の `src/num.rs` から移した。`i128` の分子と分母）、表 `table`（`CURRENCIES`、お金の単位を引く `money`、量の単位を次元と係数で引く `unit`、℉ のずれの `offset`、すべての綴りの `spellings`）、単位の型 `Unit` と `Dim`、`Tax`、綴りが単位でない理由の `Problem` を持つ。表の中身は rulec のものを一つも変えずに移した。A の段階のスケッチ（通貨を別の型 `Currency` にしていた）から変えたのは次のことである。

- 通貨は、別の型にせず、表の綴りの文字列で持つ（`Dim::Money("円")`）。通貨の表は閉じているので、表に無い綴りは `Unit::money` が受け付けない。
- `Unit` の `unit` は、書いたとおりの綴りを持つ（`money[JPY, incl_tax]` の `JPY` は `JPY` のまま）。どの言語も、出力は書いたとおりに出すからである。同じ単位かは `Unit::same` が表を引いて決める（`JPY` と `円` は同じで、`kg` と `g` は違う）。表の綴りにそろえた形は `Unit::canonical` で得る。
- rulec の綴りを読む `Unit::parse` と、書く `Display` を置いた。正確な換算は `Unit::convert`（有理数のまま）と `Unit::whole`（換算した値が整数のときだけ）、順序だけの次元かは `compares_only` である。
- 丸めの五つの仕方（rulec の `RoundMode` と `round_to`）は、rulec の意味（rulec の §7.3）なので、rulec に残した。

### 5.2 rulec

rulec の `Ty::Money`・`Ty::Qty`・`Ty::Rate`・`Ty::Number` は、中に `ritsu_units::Unit` を持つ形にする。振る舞いは変えない（D の段階で、コーパスの golden と証明書が一字も変わらないことを確かめる）。率の刻みは、入力と出力の型として `Rules` の口に出す。dandori は説明の文から刻みを読まなくなる。

**D.1 で変えたこと（★）**：`Ty` の中に `Unit` を持たせるのはやめ、`Ty` は書いたとおりの綴りを持ち続ける（`Ty::Money { cur, tax }`、`Ty::Qty { dim, unit }`）。単位の意味（次元、係数、ずれ）は、どれも `ritsu-units` の表から引く。`Ty` を `Unit` にするのは `Ty::unit(刻み)` で、`Rules` の口はこれで入力と出力の単位を渡す（率には、入力の宣言にある刻みを添える）。理由は二つある。率の刻みは rulec の型ではなく入力の宣言にあり（1.4）、`Ty::Rate` に刻みを入れると、刻みの違う二つの率が違う型になって、rulec の型の決まり（E103 など）が変わる。また、税の区別の無いお金の値（`500円` という書き方）はどちらの区別のお金とも合う（rulec の `unifies`）が、`Unit::same` は区別まで同じものだけを同じとする。`Ty` を `Unit` にすると、rulec の 19 のファイルの 129 か所の `Ty::Money` と `Ty::Qty` を、この違いを保ったまま書き直すことになり、得るものが無い。税の語を `incl_tax` と `excl_tax` のほかに書いた型（`money[円, foo]`）は、rulec が黙って通していた。その型には単位が無い（`Ty::unit` が None を返す）。D の二つ目の部分で、rulec がその型を E103 でエラーにするようにした（rulec の §15.169）。

### 5.3 dandori

- `Ty::Num(String)` を `Ty::Num(Unit)` にする。`src/syntax.rs` の `UNIT_KINDS`（rulec の九つの次元をコピーした表）は `ritsu-units` から引く。
- 二つの単位が同じかは、単位の型で決める。1.4 の例（`money[JPY, incl_tax]` を `money[円, incl_tax]` に渡す）は通る。
- 同じ次元で違う単位（`mass[kg]` を `mass[g]` に渡す）は、いまと同じく E003 でエラーにする。dandori の式は計算しない（dandori の P1）ので、境目で黙って換算すれば、`.flow` が計算をすることになる。換算は規則に書く。
- `src/model.rs` の `rate_unit` と `rate_per`（刻みを文字列で作って読む）は、単位の型の `step` に替える。
- 範囲の端に単位を付けて書けるようにする（dandori の DESIGN 7 章に残っていたもの）。`range >=1kg` は `mass[g]` の場所では 1000 で、整数にならない換算は rulec と同じくエラーにする。
- 値はいまと同じく、宣言した単位で数えた JSON の整数として運ぶ。どのプラットフォームの生成物も変わらない。`int` は単位の無い数（rulec の `number`）のまま。

**D.4 で作った形**（D の二つ目の部分）。`Ty::Num` は `ritsu_units::Unit` を持ち、二つの数の型が同じかは `Unit::same` で決める（`Ty` の等しさを手で書いた）。型の綴りは `Unit::parse` で読み、`Display` で書いたとおりに出す。`UNIT_KINDS` は、ritsu-units が次元の語を知っているので、表ごと消した（次元でない語は前と同じ E002 と注、表に無い単位と税区分の誤りは、ritsu-units の言う理由を注にした E002）。率の `rate_unit` と `rate_per` は、単位の型の刻みを読む `rate_per(&Unit)` 一つにした。規則の型も、口が渡す単位の型のまま持つ。範囲の端は、単位を付けて書けば ritsu-units の換算（`Unit::convert`）で型の単位に数え、率は百分率を刻みで割る。整数にならない端と、型の次元に無い単位は E003 である。温度の単位（`℃`、`℉`）を字句で読めるようにした（型の `[` のすぐあとと範囲の端の単位としてだけ。名前の一部にはならない）。生成物は、例とテストのフローのどのプラットフォームでも一字も変わらない（PLAN の D.4）。

### 5.4 chobo

chobo の単位（`unit 個`、`unit 円`、`unit USD scale 2`）は、名前だけを比べる単位で、量はいちばん小さい単位で数えた整数である。これを次のように単位の型に載せる。

- 名前が通貨の表にあれば、`scale 0` なら `money[<通貨>]`、`scale 2` なら `money[<通貨>c]`（百分の一で数える）。`円` と `JPY` は `scale 0` だけ（百分の一は `銭`）。ほかの scale は chobo だけの単位にする。
- 名前が質量や長さなどの単位の綴り（`g`、`kg`、`L` など）で `scale 0` なら、その単位。
- それ以外は、名前だけの数の単位 `Count(<名前>)` にし、自分とだけ同じにする。rulec は規則のためにラベルの付いた数を捨てた（rulec の §15.11）が、chobo では、同じ数え方のもの（商品の `個` と予約の枠の `席`）を分けるのに要る。

chobo のお金の単位には、税込か税抜かの区別が無い。rulec の `money[円, incl_tax]` を chobo の `unit 円` の額に渡せるようにすると、一つの勘定に税込の額と税抜の額が入りうる。rulec が型で止めていること（E103）が、chobo の勘定で破れる。そこで、chobo のお金の単位に区別を書けるようにする（`unit 円 incl_tax`）。境目では区別が同じでなければ渡せない。区別の無い chobo の単位は、区別の無い額（rulec の `money[円]`）だけを受け取る。chobo の中の意味は変わらない。chobo の構文を一つ足すことになる（D.9 で作る）。

chobo の額は 0 から 2⁶³ − 1 までで、rulec の値は負にもなりうる（返金など）。これは 7.6 の検査が見る。

**D.9 で作った形**（D の最後の部分）。chobo の単位の行の最後に `incl_tax` か `excl_tax` を書ける（`unit 円 incl_tax`、`unit USD scale 2 excl_tax`）。chobo の `model::unit_type` が、単位を上の三つの決まりで ritsu の単位の型にする。上の決まりから一つ細かくしたのは、量の単位として読むのを、足し引きのできる次元（質量、長さ、面積、体積、時間）に限ったことである。温度と音の大きさは順序だけで足し引きをしない（5.1）ので、勘定のあいだを動く額にならず、名前だけの数え方にする。お金でない単位に税込か税抜を書けば、chobo の新しいコード E014 でエラーにする（chobo の台帳に足し、chobo の DESIGN 1.2、3.1 に書いた）。`incl_tax` と `excl_tax` は chobo の修飾のキーワードになった。口の `BookUnit` は単位ごとに ritsu の単位の型（`unit`）を持ち、`BookFacts::unit(名前)` で勘定や引数の単位から引ける。`chobo api` は税込か税抜を書いた単位にだけ `tax` を出し、`chobo doc` は勘定の単位を `円 incl_tax` のように書く。帳簿の中の意味と生成するコード（TigerBeetle の ledger の番号も）は、区別を書いても変わらない。区別を書かない帳簿では、chobo のどの出力も変わらない（PLAN の D.9）。

### 5.5 日付、時刻、時間

- `date`：タイムゾーンを持たない暦の一日（rulec の §2.1、koyomi）。koyomi は日付を計算し、rulec と dandori は比べるだけ（rulec の E048）。
- `timestamp`：UTC の RFC 3339（dandori）。koyomi の `at` は、もうこの形で時刻を出している（koyomi の `api` の `wire`）。
- `duration`：秒で数える正確な長さ。rulec の時間の次元（`ms`、`s`、`min`、`h`、`d`、`w`）、dandori の `wait` と `timeout`、chobo の `expires after`（1 秒から 2³² − 1 秒）、koyomi の条件の `+ 60 days` を一つの型で比べる。koyomi は夏時間のあるタイムゾーンを受け付けない（koyomi の P6）ので、koyomi の一日はいつも 86,400 秒である。7.7 の比べは、これを前提にする。
- koyomi の `int` の入力は `number`。

### 5.6 列挙

列挙は、どこで宣言したかで決まる：rulec の規則の列挙（規則と名前）、`.proto` の列挙（完全な名前）、dandori のフローの列挙、chobo の仮押さえの状態。rulec の列挙が `.proto` の列挙を取り込んでいれば（rulec 0.22.0 からの `connect.enums` の `alias` と番号）、二つは同じ列挙として口を渡る。

rulec の組み込みの名前空間は、十三か国の一段目の区分である（`std/us/states`、`std/gb/nations`、`std/cn/provinces`、`std/tw/divisions`、`std/kr/provinces`、`std/in/states`、`std/fr/regions`、`std/es/communities`、`std/it/regions`、`std/de/states`、`std/au/states`、`std/br/states`、`std/jp/prefectures`。日本は `std/都道府県` でも取り込め、そのときは値を日本語で持つ。作者の決め、2026-10-04。rulec の DESIGN §15.182）。値は英語の名前の ASCII（`New_York`）で、口を渡るのもこの綴りである。規則の中で書ける現地の綴りと ISO 3166-2 の符号は、パーサのあとに値へ書き換わるので、口にも生成コードにも出ない。生成の型の名前（`UsState` など）と値の名前は、rulec の `src/prelude.rs` に凍結して持つ。★列挙の名前は `<国>_<種類の単数>`（`us_state`、`jp_prefecture`。パスの最後の語では `states` が五か国でぶつかる）、★値は CLDR の英語の名前からダイアクリティカルマークと略語の点を落とし、空白・ハイフン・アポストロフィを `_` にしたもの、★`std/jp/prefectures` の値は英語、`std/都道府県` は日本語にした（区分も生成コードも同じで、どちらの取り込みでも両方の綴りを書ける）。名前のもとは Unicode CLDR 48.2（Unicode License V3。`crates/rulec/THIRD_PARTY_NOTICES`）。ほかの六つの言語は、いまはこの列挙を名指さない。dandori が規則の列挙を口から受け取るときは、ほかの列挙と同じく値の綴りで受け取る。

### 5.7 捨てたもの

- **次元の掛け算と割り算**（長さ × 長さ = 面積）：rulec の §2.1 と §15.83 が捨てたもので、ここでも足さない。
- **dandori の境目での暗黙の換算**：上の 5.3。
- **chobo の名前だけの単位をやめ、rulec の単位だけにする**：chobo の勘定は、同じ数え方の違うものを分けるのに名前の単位を使っている。
- **単位を自由な名前にする**：rulec の §15.18 と同じく、閉じた表にする（`100lbs` が `lbs` という通貨として通らないように）。

## 6. プロジェクトの読み込みと名前の解決

### 6.1 プロジェクト

`ritsu check <パス>...` は、渡したファイルとディレクトリを一つのプロジェクトとして読む。ルートは 6.2 の 3 のとおり（4.7）。ディレクトリは、4.7 の飛ばす名前を除いて下まで歩く。

ファイルの種類は拡張子で決める：`.rule`（rulec）、`.flow`（dandori。`.ja.flow` も）、`.cal`（koyomi）、`.book`（chobo）、`.geas`（geas）、`.req`（yuen）、`.ctx`（sakai）、`.proto`。ほかのファイルは、誰かが名指したとき（yuen と sakai の `file "…"`、dandori の `use openapi|smithy` のパス）にだけ読む。

読み方の順：

1. 全部のファイルを一度ずつ読み、言語ごとに字句、構文、ファイルの中の名前の解決まで進める。
2. ファイルをまたぐ参照（dandori の `use rule … from "…"` や `flow "…"`、rulec の `import proto`、koyomi の `use calendar`、yuen と sakai の名指し）を、索引（6.4）で解決する。
3. 各言語の検査を、出す側から順に走らせる（rulec、koyomi、chobo、geas と `.proto` のあとに dandori、そのあとに yuen と sakai）。受け取る側は、出す側の結果を口から受け取る。
4. 言語をまたぐ検査（7 章）。

一つのファイルのエラーは、ほかのファイルの検査を止めない。受け取る側が、エラーのある出す側を参照していれば、その参照のところで、出す側の診断を名指す診断を出す（yuen の E203 にあたるものを、子プロセスの終了コードではなく型で）。

一回の実行の中では、同じファイルを二度読まない。実行をまたぐキャッシュは作らない（測ってから考える。15 章）。

**E.1 で作った形**（`crates/ritsu-project`。PLAN の E.1）。

- `Project::load(パス, --root)` が、渡したパス（無ければ `.`）の下のファイルを、土台の `paths::walk`（4.7 の名前を飛ばす）で歩き、拡張子で言語を決める。ルートは `--root`、無ければ最初のパスの上でいちばん近い `.git` のあるディレクトリ、それも無ければ最初のパスである（6.2 の 3）。並びは、上の 3 の順の言語（rulec、koyomi、chobo、geas、`.proto`、dandori、yuen、sakai。`ORDER`）、言語の中ではパスの順である。止めるのは四つで、どれも使う人が直すもの（使い方の誤り。8.4 の exit 2）である。無いパス、ルートの外のパス、名前で渡した言語の無いファイル（`.py` など）、言語のファイルが一つも無いプロジェクト。最後のものを止めるのは、何も確かめずに通ったように見せないためである（rulec の `check` が、無いディレクトリを空のまま通さないのと同じ考え）。
- 言語は `Joined` が一度だけ作る。rulec の `Engine`（確かめた規則を覚える）、koyomi・chobo・geas・dandori・sakai の `Engine`、その上の索引（6.4）である。`ritsu dandori` と `ritsu check` の dandori には規則と日付と帳簿の口を、`ritsu yuen` と `ritsu sakai` には索引を含む口のまとまりを、ここから渡す。dandori の口は、E の二つ目の部分から三つになった（前は規則の口だけで、日付か帳簿を使うフローは E018 で止まっていた）。`Joined::ports` が三つの口の組 `ritsu_ports::Ports` を作り、`ritsu check` は `dandori::ports::Engine::checked_with(root, files, &Ports, lang)` を呼ぶ。`ritsu run` の担当と言語をまたぐ検査の担当がそれぞれ足した入口（`checked_with_ports` と `checked_with`）は、取り込むときに `checked_with` 一つにした。`ritsu dandori` には、ritsu-cross の `Undecided` も渡す（7.4）。D の入口では三つがそれぞれ rulec の `Engine` を作り、同じ規則を別々に読んでいた（PLAN の 7.8）。
- 一度だけ読むことの中身は、口の問いを同じファイルに一度しか問わないことである（索引、rulec の `Engine` の覚え書き、yuen と sakai の口のまとまりの覚え書き）。`ritsu check`（E.2）は、rulec の `check` の報告も rulec の `Engine` から作る。`Engine` は報告をファイルと中身で覚え、dandori や sakai がその規則の事実を尋ねたら、報告から検査を通ったかを読んで、規則を検査し直さない（パスの書き方が違っても同じファイルとみなす。`rulec/tests/ports.rs` が、コーパスの 50 本で、`check` と事実の二つを尋ねても検査は一回ずつであることを確かめる）。各言語の `check` の文は、頼まれた言語で一度作る。
- ファイルをまたぐ参照（上の 2）は `Project::references` が解く。各ファイルの言語が `References` で言う名指しごとに、行き着くファイル、そのファイルがプロジェクトのものか、行き着き方（`Landing`：ファイルが無い、索引が読むファイルならその言語の答え、`.proto` なら ritsu-proto で読んでその要素があるか、索引が読まないファイル）を返す。参照の誤りは、これまでどおり各言語が自分の `check` と自分のコードで言う（7.10）ので、これは言語をまたぐ検査（E.4）が読み、LSP を作るならそれも読む、プロジェクトの一枚の見取り図である。テストは、sakai の例をコピーした `crates/ritsu/tests/projects/通販` の参照の全部（`crates/ritsu-project/tests/golden/shop.references.txt`）を golden にする。

### 6.2 名指しを処理系全体のものにする

`ritsu-base` の名指しは、yuen と sakai の DESIGN.md の 2 章で決め、二つのリポジトリの `tests/fixtures/naming.tsv`（36 行の試しの表）で確かめているものを、そのまま処理系全体の決まりにする。決まりは次のとおり。

1. **形**：`<ツール> "<パス>" [<種類> <名前>]...`。組はツールの構造どおりに入れ子にできる。入れ子にできるのは、proto の `service S [method M]`、`message M [field f]`、`enum E [value V]`（入れ子のメッセージは名前を `.` でつなぐ：`message Order.Line`）と、rulec の `enum E [value V]` だけで、ほかのツールの組は一つまで。子の種類は、親の種類のすぐあとにしか書けない。
2. **ツールの語**：`rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`proto`、`file`、`yuen`、`sakai`。`dir` はツールの語にしない（sakai の `.ctx` の構文の語にとどめる）。
3. **パス**：`.req` や `.ctx` の中では、書いたファイルのディレクトリからの相対。区切りは `/` で、`.` と `..` は字の上で畳む。絶対パスと空のパスはエラー。`"."` はルートを指す。末尾の `/` は取り除く。JSON では、ルート（`--root`、無ければ最初に渡したパスの上でいちばん近い `.git` のあるディレクトリ、それも無ければ渡したディレクトリ）からの相対で、ルートの外に出るパスはエラー。
4. **種類の語**：rulec は `input`・`output`・`enum`（下に `value`）・`table`・`clause`・`define`・`derive`・`machine`・`source`、koyomi は `input`・`date`・`claim`・`source`、chobo は `unit`・`account`・`transfer`、geas は `claim`、proto は `service`（下に `method`）・`message`（下に `field`）・`enum`（下に `value`）、yuen は `requirement`・`source`、sakai は `context`・`term`、dandori は `task`・`case`・`record`（下に `field`）・`enum`（下に `value`）・`input`・`output`（6.3。D の二つ目の部分で足した）。`file` には無い。どの言語も、自分が使わない種類も名指しとして受け付ける。
5. **名前**：ツールの名前（JSON の `name`）。別名は使わない。語（空白、`"`、`#` を含まない一続きの文字。頭が数字でもよい）か `"…"` で書く。`"…"` の中のエスケープは `\"` と `\\` だけで、ほかはエラー。正規化せず、大文字と小文字を区別する。proto の名前は、そのファイルの package から見た名前。文字列の外の全角の空白、`"…"` で書いた種類やツールの語、名前の無い種類はエラー。
6. **同じ・含む**：同じは、ツールの語と、ルートからのパスと、組の並びが同じとき。ファイルは中のものを全部含み、親の組（proto の `service`・`message`・`enum`、rulec の `enum`）は子を全部含む。
7. **JSON の形**：`{"text": …, "tool": …, "path": …, "items": [[種類, 名前], …]}`（キーはこの順）。`text` は、パスをルートからの相対に直し、名前を語で書けるなら引用符なしで書いた形。空白を入れない詰めた書き方で、ASCII でない文字はそのまま出す。
8. **文の中の書き方**：診断などの文に書くファイルの場所（`<パス>:<行>:<列>`、コピーのパス）は、走らせたディレクトリから、渡されたとおりに書く。文の中の名指しは、JSON と同じくルートからの相対で書く（読み直すと同じ名指しになり、`.req` や `.ctx` にそのまま貼れる）。
9. **JSON の中のファイルの場所**：診断の `file` なども、名指しと同じくルートからの相対にし、JSON の外側に `root`（走らせたディレクトリから見たルート）を添える。取り込んだときは、yuen がルートからの相対、sakai が走らせたディレクトリからの相対で食い違っていた。土台で一つにするときにそろえる（4.2）とし、C.8 で sakai をルートからの相対にした。

この決まりを、処理系のどこでも使う一つの書き方にする。yuen と sakai の `.req` と `.ctx` の中、診断の文と JSON、LSP の「定義へ移る」、`ritsu check` の JSON の中のもの、のどれも同じ形で書き、読み直すと同じものを指す。ツールの語は九つのまま。`ritsu` はツールの語にしない（ritsu は言語ではない）。

OpenAPI と AsyncAPI の文書の要素（スキーマ、チャネル、メッセージ、操作）には、まだ名指しの形が無い。sakai は `.ctx` の中では短い書き方で、api と診断ではファイルと JSON Pointer（`payments/api.yaml#/components/schemas/Charge`）で書く（sakai の DESIGN 15.10）。ツールの語 `openapi` と `asyncapi` を足すと、yuen の診断の文（書けるツールの語の並び）と `naming.tsv` が変わるので、yuen と一緒に決める。


### 6.3 種類の語を足す

名指しの決まり（6.2）では、dandori に種類の語が無かった（dandori が中のものを JSON で出していなかったため）。D の段階で、次を足す。

- dandori：`task`、`case`、`record`（下に `field`）、`enum`（下に `value`）、`input`、`output`。

`naming.tsv` の `dandori "order.flow" task reserve` の行（`ERROR: dandori has no kinds yet` だった）は、JSON の行に変わる。入れ子の決まり（子の種類は親のすぐあと）は proto と rulec と同じにする。geas の `target` など、ほかの言語の種類を足すのは、使う側が要るとわかってからにする。

D の二つ目の部分で、これを足した（PLAN の D.6）。表は、dandori の行を JSON にし、入れ子の行（`record 予約 field 泊数`、`enum Outcome value awaiting_review`）と、入れ子の誤りの行（親のすぐあとでない `value`、`task` の下の組、`record` の下の `field` でない組）と、種類の無い `file` に種類を書いた行（`dandori has no kinds yet` の行が受け持っていた誤りの種類）を足して、42 行（名指し 24、誤り 18）になった。

### 6.4 索引：中のものと参照

各言語は、`Items` と `References` の口（3.2）で、自分の中のものと参照を出す。`ritsu-project` は、それをプロジェクト全体の索引にする。

**中のもの**（`Items`）は、種類、名前、行の範囲、定義の文を持つ。定義の文は、yuen が端のハッシュを取る元で、何を定義の文にするかは各言語が自分の DESIGN.md に書く。D の段階で次のとおりにした（細かいところと、表に無い yuen と sakai は 3.2 の終わりの段落）。

| 言語 | 種類 | 定義の文 |
|---|---|---|
| rulec | `table`、`clause`、`define`、`derive`、`input`、`output`、`enum`、`machine`、`source` | そのものの行を `rulec fmt` が書く形にしたもの（表なら見出しから最後の行まで） |
| koyomi | `date`、`claim`、`input`、`source` | `date … =` の塊の行（操作の行を含む）、条件の行 |
| chobo | `unit`、`account`、`transfer` | yuen の DESIGN 3.2 の形（`chobo api` の一つから名前とコードを除いたもの）を土台の JSON で |
| dandori | 6.3 の種類 | タスクや案件やレコードの宣言の塊の行（列挙、フィールド、入力、出力はその行、列挙の値はその名前） |
| geas | `claim` | 主張の塊の行 |
| proto | `service`、`method`、`message`、`field`、`enum`、`value` | yuen の DESIGN 3.4 の決まった形の文 |

yuen の端は、いまはファイル全体のもの（rulec、koyomi の日付、geas、dandori）がある（yuen の DESIGN 3.2）。定義の文が出れば、表や日付の関数やタスクの一つ一つが端になる（7.10）。端の中身が変わるので、yuen のテストと例の確かめた記録（`.req` のハッシュ）は D の段階で取り直す。D.7 で取り直し、F.1 の例もこの端で記録を書いた。koyomi の日付の端がその日付の定義の文になったので、カレンダーの一行を書き換えた例（yuen の `civil_code_periods_reread`）は、書き換えた日付へのリンク一本だけが E303 で止まる（ファイル全体を端にしていた A の段階の見込みでは、同じ `.cal` を指す三本が止まるはずだった）。

**参照**（`References`）は、参照のある行、先の名指し、参照の仕方を持つ。dandori の `use rule … from`（同梱、Lambda、Connect の URL、`local`）、`use proto|openapi|smithy`、`connect`、`implements`、子の `flow "…"`、rulec の `import proto`、`shape`、`source … file`、`range from koyomi`（先は `koyomi "<ファイル>" date <名前>`。E.4 で足した）、koyomi の `use calendar` と `source`、yuen と sakai の名指しを出す。sakai はこれで全部の言語の参照を行番号つきで確かめ（7.10）、yuen は `trace` と `affected` でたどる。

**E.1 で作った形**（PLAN の E.1）。索引は `ritsu-ports` の `Index` である。言語の口（`Items`、`References`）をツールの語ごとに持ち、ファイルごとの答えを、ツールとルートとルートからのパスで一度だけ尋ねて覚える（名指しのパスはルートからなので、ルートが違えば別の答えになる）。名指しを渡せば `find` が引く。答えは四つのどれかで（`Lookup`）、その言語がつながっていない、その言語がファイルに答えない（言うこと `Said` を添える）、ある（ファイルそのものの名指しなら、言語がファイルを読めたこと）、無い（同じ親の下の同じ種類のものを添える）である。何が誤りかは言わない。言うのは、名指しを書いた言語である（yuen の E202、sakai の E007）。

索引を言語の層ではなく口の層に置いたのは、受け取る側（yuen、sakai）が型として持つものだからである。ritsu-project に置けば、言語のクレートがつなぎの層に依存することになる（3.1 の決まり 3）。中身は、どの言語の意味も持たない、答えを覚えて名指しで引くだけのものである。

| | `Items` | `References` |
|---|---|---|
| rulec | 索引に入れる | 入れる |
| koyomi | 入れる | 入れる |
| chobo | 入れる | （答えない） |
| geas | 入れる | （答えない） |
| dandori | 入れる | 入れる |
| sakai | 入れる | 入れる |
| yuen | 入れない | 入れない |

yuen を入れていないのは、yuen の `Engine` が、借りた出典の端を作るためにほかの言語の口（この索引を含む）を持つので、索引が yuen を持つと輪になるからである。yuen の要件を名指す言語は、いまは無い（LSP が要るようになったら、ritsu-project で輪にならない持ち方を決める）。

yuen と sakai は、名指しを自分で言語ごとに引くのをやめ、この索引で引く（7.10）。yuen は `ends.rs`（リンクの端）と `coverage.rs`（範囲が集めるもの）、sakai は `suite.rs`（参照を読む）と `elements.rs`（語の `means` が名指す規則の入力、出力、列挙、値）である。どちらも出力は変わらない（PLAN の E.1 の突き合わせ）。

## 7. 言語をまたぐ検査

### 7.1 結果の三つと、検査の診断

言語をまたぐ検査は `ritsu-cross` に置き、口（3.2）だけを通す。どの検査も、結果は三つのどれかである（P5）。

- **成り立つことを示した**：何も出さない（`ritsu check` の要約の数に入る）。
- **成り立たない例がある**：エラー。その値と、そこに至るもの（フローの実行、規則の入力、koyomi の入力の日付）を添える。
- **決められない**：警告。理由を言い、実行時に確かめる手当てがあればそれを言う。

検査の診断は、ritsu の台帳（`crates/ritsu-cross/src/codes.rs`）のコードで出す。言語ごとの台帳とは番号を分け、`ritsu explain <コード>` で引く。どの言語の診断かは、テキストでは見出しの括弧に（`error[ritsu E201]`）、JSON では `tool` に書く（8.3）。

**E.3 で作った形**（PLAN の E.3）。台帳は ritsu-base の `ledger` で書き、番号を帯で分ける。E1xx はプロジェクトのファイルを ritsu が読むところ、E2xx は言語の境目の検査（7.2 の X1〜X7。E の二つ目の部分から）、9xx はセキュリティの検査（16 章。2026-10-06 から）である。E の最初の部分で載せたのは、次の一つである。

| コード | いつ出るか |
|---|---|
| E101 | プロジェクトの `.proto` を、ritsu の一つの読み手（ritsu-proto）が読めない。どの言語もこの読み手で読むので、どの言語からも読めない。読む言語は、読むところで自分のコードでも言う（rulec の E013、dandori の E016、sakai の E106、yuen の E205）。どの言語も読まない `.proto` は、これが無ければ誰も言わない |

E.4 と E.5 で、言語の境目の検査のコードを載せた（E2xx がエラー、W2xx が決められないときの警告）。

| コード | いつ出るか |
|---|---|
| E201 | ワークフローが規則を呼ぶところで、dandori が知っている値の範囲と出どころの中に、規則の前提（入力どうしの関係か、`range from koyomi` の日の集合）を破る値がある（X2） |
| W201 | 規則を呼ぶところで、前提が保たれるかを決められない（範囲の無い出どころ、並びの合計や長さ、`range from koyomi` の日の前提に koyomi の日付の日のほかからも来る値。X2）。決められない前提は、`ritsu dandori build` が書くワークフローのコードが、走らせたときに、値ができたところですぐに確かめ、破る実行を `Dandori.BrokenPrecondition` で失敗させる（dandori の DESIGN 1.17） |
| E202 | ワークフローが koyomi の日付の日を規則の日付の入力に渡すとき、koyomi が数えたその日付の日のどれかが、規則の宣言した入力の範囲の外にある（X3 の (a)）。注に、外れる日と、koyomi がその日を返す入力を書く |
| W202 | 規則の日付の入力に渡す値が、koyomi の日付の日のほかに、何日かを言わないところ（ワークフローの入力、タスクの結果、`now`）からも来うる。または koyomi がその日付の日を数えない（X3 の (a)） |
| E203 | 規則の数の出力を振替の額に渡すとき、出力が負か 2⁶³ − 1 を超えうる（X4。chobo は 0 から 2⁶³ − 1 までを受け取る）。注に、その額になる規則の入力（規則のベクタから）を書く |
| W203 | 規則の数の出力を振替の額に渡すとき、出力の範囲に端が無いか、値が範囲の分からないところからも来うる（X4） |
| E204 | 規則の出力を額に渡す `do` か `hold` で、額を呼び出しの渡す範囲に限った chobo の探索が見つける、帳簿の境界の理由を、タスクが処理していない（X4） |
| W204 | タスクが処理する帳簿の境界の理由を、chobo の探索が見つけない（探索の深さまでしか言えない）。額の範囲が分からない、帳簿が答えないときも（X4） |
| E205 | koyomi の日付の入力に渡す日が、入力の範囲を外れうる（X6）。注に、外れる日と、それを返す koyomi の入力を書く |
| W205 | koyomi の日付の入力に渡す日を、dandori が知らない（ワークフローの入力、タスクの結果、`now` から来る。X6） |
| E206 | 仮押さえを作ってから確定か取消をするまでの長さの下限が、有効期限以上である。帳簿はどの実行でもその呼び出しを `expired` で拒否する（X5）。注に、下限と、それを作る文を書く |
| W206 | 仮押さえを作ってから確定か取消をするまでの長さが、有効期限の前にも後にもなりうる、または長さの上限が分からない（X5） |

2026-10-06 に、セキュリティの検査のコードを載せた（16 章）。番号は、どの言語の台帳でも同じ検査を指す 9xx の帯に置いた（16.2）。

| コード | いつ出るか |
|---|---|
| W901 | 鍵の形の値が、プロジェクトの `.proto` か、言語が参照する契約の文書（`.json`・`.yaml`・`.yml`）に書いてある（16.3）。言語のファイルの鍵は、その言語の W901 |
| E905 | フローが秘密の値を、地図の外か、印を付けたコンテキストと関係の無いコンテキストへ送る（X14、16.8） |
| W905 | フローが秘密の値を送る先が地図のどこかを、決められない（X14、16.8） |

どのコードにも、英語の小さなプロジェクトの再現がある（E.4 で判定より先に載せた `Repro::Later` は、E.5 で無くなった。X14 の二つも、判定より先に `Repro::Later` で載せ、`Flows::sends` と `Maps` が入ってから再現に替えた）。X2 は返金の確認（`refund_check.rule`・`refund.flow`）、X3 の (a) は支払日を規則に渡す請求（`payment_terms.cal`・`batch.rule`・`billing.flow`）、X4 は催しに座席を割り当てるホール（`seats.rule`・`hall.book`・`booking.flow`）、X6 は支払日の一週間前の催促（`payment_terms.cal`・`reminders.cal`・`reminding.flow`）、X5 は支払日まで商品を押さえる請求（`weekdays.cal`・`payment_terms.cal`・`stock.book`・`invoice.flow`）。W901 はコメントに偽の鍵を書いた `maps.proto`、E905 は Payments・Ordering・Notices の地図と、`debug_redact` の付いたカードの番号を Notices の API へ渡すフロー（`shop.ctx`・`contexts/*.ctx`・`card.proto`・`notices.json`・`checkout.flow`）、W905 は同じプロジェクトの地図が無いファイルを `use context` する形。`crates/ritsu/tests/codes.rs` が全部を英語と日本語で走らせ、自分のコードが出ることを確かめる。

ritsu の台帳に X10（名指しの解決）のコードは無い。名指しを書いた言語が、自分のコードで言うからである（7.10）。どのコードも、再現（小さなプロジェクトのファイル）を持ち、`crates/ritsu/tests/codes.rs` がそれを一時ディレクトリに置いて `ritsu check .` を英語と日本語で走らせ、見出しが `[ritsu <コード>]` の診断が出ることを確かめる。`ritsu explain` は、ritsu-base の台帳の書き方で、テキスト、Markdown、JSON を出す。言語のコードを渡されたら、`ritsu <言語> explain` で引くように言って 2 で終わる。`crates/ritsu-cross/docs/codes.md` と `codes.ja.md` は `ritsu explain --all --format markdown` の出力そのもので、`crates/ritsu-cross/tests/codes.rs` がそれを確かめる。

### 7.2 一覧

| | 検査 | 示すこと | 読むもの（口） | 確かめる場所 | 診断 | 段階 |
|---|---|---|---|---|---|---|
| X1 | 境目の単位 | 境目を越える値の単位が、単位の型の上で同じ | `Rules` の型、chobo の単位（`BookFacts::unit`）、koyomi の型、dandori の型 | dandori の呼び出しと受け取り、rulec の `range from koyomi` | 言語の検査のまま（dandori の E003、rulec の E065。7.3） | D（E で、言語の検査のまま残すと決めた） |
| X2 | 規則の前提を、呼び出しの場所で | フローが渡しうるどの値でも、規則の入力の範囲と前提が成り立つ | dandori の値の範囲と出どころ（`Flows::crossings`）、`Rules` の前提と問い、`Dates` の値の集合 | dandori の規則の呼び出し | ritsu の E201、W201。決められない前提は、生成したコードが走らせたときに確かめる | E.4 で作り、E.5 で koyomi の日の前提を決められるようにし、決められないものを生成コードで確かめるようにした |
| X3 | 期日の値の集合を、規則の入力の範囲に | koyomi の関数がとりうる値が、規則の入力の範囲に収まる。規則がその集合を範囲として宣言したら、その集合の上で表を確かめる | `Dates` の値の集合、`Rules` の問い（`checked_over`、`date_range`） | (a) dandori の呼び出し、(b) rulec の入力の宣言 | (a) ritsu の E202、W202。(b) rulec の検査（集合の上の E101、E102 など）と E129、E130 | (b) は E.4（rulec の §15.174）、(a) は E.5 |
| X4 | 規則の出力から、振替の額へ | 振替に渡る額が chobo の受け取れる範囲に収まり、その額で拒否されうる理由がこれだけだと言える | `Rules` の出力の範囲（`output_values`）、`Books` の問い（`refusals`）、dandori のタスクのエラー | dandori の振替の呼び出し（`do` と `hold`） | ritsu の E203、W203、E204、W204 | E.5（rulec と chobo の問いと判定は E.4） |
| X5 | 仮押さえの有効期限と、待ちの長さ | 「必ず期限が切れる」か「期限は切れない」 | `Books` の有効期限、`Dates` の日数（`span`）、dandori の待ちとタイムアウト（`HoldSpan`） | dandori の、chobo の仮押さえに従う案件の確定と取消 | ritsu の E206、W206（期限が切れないと示したときは何も言わない） | E.5 |
| X6 | koyomi の関数を呼ぶ | 渡す日付が、関数の入力の範囲に収まる | `Dates`、dandori の値の出どころ | dandori の期日の呼び出し | ritsu の E205、W205 | E.5 |
| X7 | 一つの参照インタプリタ | （証明ではない）規則の評価、期日の計算、帳簿の動きを含めて、フローを一度に流す | `Rules`・`Dates`・`Books` の評価 | `ritsu run` | （検査ではない。実行のトレースと終わりの帳簿を出す） | E.6 |
| X8 | yuen の端を一つずつ | リンクの端が、表、日付の関数、タスクの一つ一つになる | `Items`、`Claims`、`Sources` | yuen の `.req` | yuen の検査 | D.7 |
| X9 | sakai の参照を全部の言語で | dandori を含む全部の言語の参照が、宣言した関係と公表された言語を通る | `References`、`Rules` の列挙、`Books` | sakai の `.ctx` | sakai の E201〜E209 | D.8 |
| X10 | 名指しの解決 | どこに書いた名指しも、プロジェクトの中のものを指す | `Items`、`References`（索引） | 全部 | 名指しを書いた言語のコード（yuen の E202、sakai の E007） | D（E.1 で索引で引くようにした） |
| X11 | 同じ条のコピー | 同じ法令の同じ条を、規則、カレンダー、要件が同じ本文で保存している | 土台の出典 | rulec、koyomi、yuen | yuen の E107 | C〜D（土台の出典は C、yuen が借りた出典とコピーを比べるのは D.7） |
| X12 | 一つの `.proto` の読み方 | 同じ `.proto` を、どの言語も同じに読む | `ritsu-proto` | rulec、dandori、sakai、yuen | 読めないファイルは ritsu の E101 と、読む言語のコード | C〜D（C.9、D.10） |
| X13 | 処理系自身の依存 | ritsu のクレートの依存が 3.1 のとおり | sakai の地図、Cargo の依存 | ritsu のリポジトリ | sakai の E201 など（`ritsu check ritsu.ctx`）と `cargo xtask deps` | E.8 |
| X14 | 秘密の値の行き先 | 契約が秘密と印を付けた値を、フローが、印のコンテキストと関係の無いコンテキストや地図の外へ送らない | dandori の送る値と印（`Flows::sends`）、sakai の地図（`Maps`） | dandori の、プロジェクトの中の成果物を呼ぶタスク | ritsu の E905、W905 | 2026-10-06（16.8） |

### 7.3 境目の単位（X1）

dandori が規則に渡す値、規則から受け取る値、chobo の振替に渡す額、koyomi の関数に渡す日付と受け取る日付や時刻の単位を、単位の型（5 章）で比べる。同じ単位の別の綴り（`JPY` と `円`）は同じ、同じ次元の違う単位（`kg` と `g`）と、税込と税抜は違う。いまは dandori が自分の文字列の比べ（E003）でしていることを、単位の型の上でする。D の段階では dandori の E003 のまま出し、E の段階で、chobo と koyomi の境目も加えて ritsu の台帳のコードに移すかを決める。

**E で決めたこと**：X1 は言語の検査のまま残し、ritsu の台帳には移さない（★作者が決めるべきだったかもしれない）。境目の単位が合わなければ、生成するコードの型が決まらないので、生成する言語が自分の検査で止めなければならない（`dandori build` は `ritsu check` を通らない）。言語の外にもう一度置けば、同じことを二度言うか、何も言わないかのどちらかになる。境目ごとの持ち主は次のとおり。規則の呼び出しと受け取りは dandori の E003（D で単位の型にした）。koyomi の日付と規則の入力のあいだ（rulec の `range from koyomi`）は rulec の E065（日付でない入力には書けない）。koyomi の日付と chobo の振替を dandori から呼ぶところは、E.5 で dandori が自分の検査（E003 と同じ形）で確かめる（口の `BookFacts::unit` が、帳簿の単位を ritsu の単位の型で渡す）。

### 7.4 規則の前提を、呼び出しの場所で（X2）

rulec は、形だけでは表せない入力の前提を `api` の `preconditions` に出している（rulec の §15.116）。種類は三つ：入力どうしの関係（`constraint`）、並びの合計の上限（`sum`）、並びの長さの上限（`length`）。いまの dandori は、これを読むだけで確かめない（dandori の DESIGN 7 章）。規則の生成したコードは入口でエラーにするので、前提を破る値は、それを作ったタスクの結果が実行の履歴に残ったあと、規則を呼ぶところで初めて落ちる。

ritsu では、呼び出しの場所で次のように確かめる。

1. dandori は、規則の入力に渡す値の範囲を求めている（dandori の DESIGN 1.3。変数の範囲は、入れるすべての値の範囲を合わせたもの）。
2. ritsu-cross は、入力ごとのその範囲と前提を `Rules` に渡して尋ねる。rulec は、範囲のどの組み合わせでも一次の不等式が成り立つかを、もう持っている消去の手続きで決める（`src/fourier.rs`。rulec の §15.139 と §15.141）。
3. 成り立てば何も出さない。成り立たない値があればエラーで、その値を添える。決められなければ警告（W201）で、`ritsu dandori` が、決められなかった前提を口 `Undecided` で dandori に渡し、dandori の生成したコードが、ワークフローを走らせたときに、値ができたところですぐに確かめる（どのプラットフォームでも。dandori の DESIGN 1.17）。

並びの合計と長さは、dandori がリストの長さの上限を知っているときだけ決められる。いま知っているのは、`for … at most n` で集めたリスト（長さは n まで）と、リストのリテラルである。タスクの結果のリストには長さの範囲が無いので、決められない（警告と、実行時の確かめ）。リストに長さの範囲を書けるようにするかは、E の段階で決めることにしていた。E では書き方を足していない。dandori は並びをたどる規則を呼ばない（E005）ので、並びの合計と長さの前提は、どれも決められない（下の判定の 2）。

**E.4 で作った形**（PLAN の E.4。`crates/ritsu-cross/src/preconditions.rs`）。`ritsu check` は、プロジェクトの `.flow` のうち dandori の検査を通るものごとに、口の `Flows::crossings` で規則の呼び出しを受け取り（日付と帳簿を使うフローも読めるよう、規則・日付・帳簿の三つの口で読む。E.4 では規則の口だけの `rule_calls` で、E.5 で替えた）、呼ぶ規則の前提ごとに、次のように決める。前提一つと呼び出し一つの組が、境目一つである（`borders` に数える）。dandori の検査を通らないフローと、rulec の検査を通らない規則は、それぞれの言語が言うので、ここでは飛ばす。

判定（Lean の `RitsuCross` は、この規則をそのままモデルにする）：

1. 入力どうしの関係 `left op right`（op は `<=`、`<`、`>=`、`>`）。
   - 呼び出しが `left` か `right` を渡していなければ、決められない。
   - 同じ値（書いたとおりの値が一字も違わない）を両方に渡していれば、`<=` と `>=` は成り立ち、`<` と `>` はどの値でも破る（値の範囲の下の端、無ければ上の端を例にする）。
   - どちらかの値に範囲が無ければ（範囲の無い出どころがある）、決められない。理由に出どころを書く。
   - 両方に範囲があれば、`Rules::preconditions_hold` に二つの範囲を渡す。rulec は、範囲の箱のいちばん厳しい角（`<` と `<=` なら左の上の端と右の下の端、`>` と `>=` ならその逆）で比べる。角で成り立てば箱のどの組でも成り立つ（左は上の端以下、右は下の端以上だから）。角で破れば、その角が例である。端が開いていれば決められない。
2. 並びの合計と長さの上限は、決められない（dandori は並びを歩く規則を呼ばず（E005）、並びの長さを知らない）。
3. 日付の入力の範囲が koyomi の日付の日の集合（`Days`。rulec の `range from koyomi`）のとき（E.5 で決められるようにした。E.4 ではいつも決められないとしていた）、渡す値がどこから来うるか（`CallArg` の `from`）で決める。来うるところがどれも koyomi の日付の日なら、それぞれの日付について koyomi が数えた日（`Dates::values`）のどれもが、規則の集合の日でなければならない。そうでない日があれば、最初の日付の最初のその日を例にして E201。koyomi の日付の日のほかに、何日かを言わないところ（ワークフローの入力、タスクの結果、`now`）からも来うるとき、koyomi が数えないとき、値の来るところが無いときは決められない（W201）。規則と同じ日付の日を渡すなら、集合が同じなので、必ず成り立つ（7.5 の (b) の終わりの段落）。E201 の注の二つ目と三つ目は、日の前提では「koyomi は、入力の範囲のすべてで日付の日を数えています」と「koyomi "<ファイル>" date <日付> の日を渡すか、規則の範囲を直してください」にした。

例は、dandori の範囲が値ごとに独立に取れるとしたときの角である。dandori の E014（「範囲を外れることがある」）と同じ読み方で、二つの値が同じ出どころから来て連動している場合（同じ値そのものは上の 1 で扱う）には、実際には起きない角を例にすることがある。決められないときの警告（W201）は、`ritsu dandori build` が書くワークフローのコードが、走らせたときに、値ができたところですぐに前提を確かめることを言う（下の「E.5 で作った形」）。

再現は英語の小さなプロジェクト（`refund_check.rule` と `refund.flow`。返金の額は払った額を超えない、という前提）で、`crates/ritsu/tests/cross.rs` が、範囲で前提を保つもの（成り立つ）、超えうるもの（E201、例は asked = 10000, paid = 0）、範囲の無いタスクの結果を渡すもの（W201）、同じ値を両方に渡すもの（成り立つ）を、英語と日本語のテキストと JSON の `borders` で確かめる。

```
$ ritsu check .
ok refund_check.rule
refund.flow: ok
error[ritsu E201]: refund.flow:21:1: The call of the rule check can give it values that break its precondition `asked <= paid`
    21 |   let decision = check(paid: paid, asked: asked)
  = `asked` is `>=0 <=10000` and `paid` is `>=0 <=10000`, and at asked = 10000, paid = 0 `asked <= paid` does not hold
  = The rule's generated code refuses a call that breaks a precondition at its door, so this call fails only when the workflow runs. The ranges are dandori's, gathered from every place the values come from.
  = Branch so that the precondition holds before the call, or narrow the ranges (the `range` of an input or a task's result).
ritsu check: 2 files (rulec 1, dandori 1): 1 fail (1 error); borders between the languages: 1 checked, 0 undecided
```

**E.5 で作った形**（PLAN の E.5。決められない前提を、生成したコードが確かめる）。

- `ritsu dandori` は、ritsu-project の `Joined` から `ritsu_cross::UndecidedCalls`（ritsu の口 `Undecided` の実装）を作り、dandori に渡す（`dandori::cli::run_with_undecided`。koyomi と chobo の口も一緒に渡す）。`UndecidedCalls` は、`ritsu check` と同じく `Flows::crossings`（日付のファイルと帳簿も読む）でフローを読み、同じ判定（`preconditions::decide`。日付の口も渡す）で、フローの呼び出しごとに決められない前提を返す。そのため、koyomi の日の前提で ritsu-cross が決められるもの（koyomi の日付の日だけが来る値）は、確かめる文にならない（担当の版は規則の口だけの `rule_calls` で読んでいたのを、取り込むときにこの形にした）。dandori の `build`・`run`・`scenarios`・`doc` は、それを確かめる文としてフローに入れ（dandori の `src/prechecks.rs`）、生成し、走らせ、描く。`check` は入れない。決められた前提（成り立つと示したもの、E201 の例があるもの）しか持たない呼び出しの生成物は、これまでと一字も変わらない（`crates/ritsu/tests/dandori.rs` が、七つのプラットフォームで、ritsu-cross を通さない build と比べる）。
- どこで確かめるか：値ができたところで、すぐに。ただし、その呼び出しに必ず届く場所に限る。規則を呼ぶところから同じブロックを前へたどり、前提が読む値を変えず、次の文へ進むか実行を失敗させるかしかない文（ほかの変数への `let`、待ち、`pass`、ハンドラーがどれも `succeed` も `break` もしない呼び出し）を越え、値を作る文（たいていは、その値を返したタスク）か、ほかへ進むことがある文（`match`、ループ、実行を成功で終えるかループを抜けるハンドラーを持つ呼び出し）の直後で止まる。ブロックの頭まで来れば、ブロックの頭に置く。A の段階の「その値を作ったタスクの直後」をいつもそうすると、`match` の一方の分岐だけが規則を呼ぶフローで、規則に何も渡さない実行まで落とすので、この形にした（★）。
- 破ったとき：実行は `Dandori.BrokenPrecondition` で失敗する（★。理由は `line <呼び出しの行>: the values given to the rule <規則> break its precondition <前提>`。どのプラットフォームでも同じ文）。`Dandori.BadResponse` と同じく、リトライせず、`on` でも `on failure` でも処理しない。規則の生成したコードの入口の確かめは、そのまま残る。
- 七つのプラットフォーム：Step Functions は Choice と Fail、Temporal の三つと durable functions はワークフローのコードの `if`、Argo は値を計算するテンプレート一つ、pydantic-graph は確かめる節点一つ。E040 の見積もりに、Step Functions で 4 件、Argo で 4 ノードを足す。E050 にしたものは無い（整数の比べと日の並びは、どのプラットフォームでも書ける）。
- 前提の種類：入力どうしの関係（`<=`、`<`、`>=`、`>`）と、koyomi の日（`Days`。日の並びをコードに書き込む）。並びの合計と長さは、dandori が並びをたどる規則を呼ばない（E005）ので、口から来ても確かめる文にしない。
- 同じ担当が、規則の日付の入力と出力を、つなぐコード（`rules.ts`、`rules.py`、Lambda の Python、`rules.go`）で日の番号に直すようにした。rulec の生成したコードは日付を 1970-01-01 からの日数で受け取り、返すが、つなぐコードは数として読んでいた（日付の入力を持つ規則が例とテストに無く、表に出ていなかった）。
- 例は英語の小さなフロー（dandori の `tests/flows/preconditions.flow` と、日本語の `preconditions.ja.flow`）。範囲の無いタスクの結果から来る額の関係と、`range from koyomi` の日を、タスクの直後、分岐の頭、順に回すループと並列のループのイテレーションの頭で確かめる。

```
$ ritsu check preconditions.flow rules/refund_check.rule rules/settlement.rule dates/payment_terms.cal
…
warning[ritsu W201]: preconditions.flow:41:1: Whether the call of the rule check keeps its precondition `asked <= paid` cannot be decided
    41 |   let decision = check(paid: paid, asked: asked)
  = nothing says what range `asked` is in (the answer of `ask_amount` has no range)
  = The workflow's code that `ritsu dandori build` writes checks it when the workflow runs, as soon as the values are made; a run that breaks it fails there with `Dandori.BrokenPrecondition`.
…
ritsu check: 4 files (rulec 2, koyomi 1, dandori 1): all pass (7 warnings); borders between the languages: 5 checked, 4 undecided
```

（dandori のディレクトリの `tests/flows` で走らせたもの。取り込んだあとの木で走らせても、同じ出力だった。46 行目の `range from koyomi` の前提は、渡す値がタスクの結果（`pick_day`。何日かを言わない）から来るので、決められない。）`ritsu dandori build preconditions.flow --target temporal` が書く `workflow.ts` の一部：

```ts
    // line 41: the precondition asked <= paid of the rule check, which ritsu could not decide
    if (!(asked! <= paid)) throw dd.fail("Dandori.BrokenPrecondition", "line 41: the values given to the rule check break its precondition asked <= paid");
```

Lean の `RitsuCross` は、この判定と rulec の角の答えをモデルにし、成り立つと言えば呼び出しが渡しうるどの組でも前提が成り立ち、例を言えばその組で破れることを証明している（11.2 の 3）。同じ値を両方に渡す場合の近道は、二つの入力が同じ刻みで運ばれるときに正しく、それは dandori の E003 が保証する。

### 7.5 期日の値の集合を、規則の入力の範囲に（X3）

koyomi は、関数を範囲のすべての入力で計算するので、とりうる値の集合が正確にわかる。rulec は日付の足し算を持たない（rulec の E048）が、日付を入力に取る表の完全性と重なりを、範囲の上で確かめられる。二つをつなぐ。

koyomi の例 `payment_20th_close_next_10th.cal`（日本語の版は `payment_20th_close_next_10th.ja.cal`。受領日の範囲は 2026-01-01〜2027-11-20 の 689 日）で、`koyomi vectors` の全行から数えると（2026-10-03。英語の版と日本語の版で同じ）：

- `payment`（日本語の版の `支払日`）がとる値は 23 通り、2026-02-10 から 2027-12-10 までで、どれも月の 8 日、9 日、10 日のどれか。
- 受領日から支払日までは、最短 18 日、最長 51 日。

（England and Wales の例 `close_20th_pay_10th.cal` では、受領日 1,055 日に対して 35 通り、2026-02-10 から 2028-12-08 まで、最短 18 日、最長 51 日。koyomi の `tests/ports.rs` の `a_dates_values_and_its_days_are_counted_exactly_in_english` が確かめている。）

この支払日を入力に取る規則を書くと、いまの rulec では入力に `range >=2026-02-10 <=2027-12-10` を書き、その 669 日のすべてで表が完全でなければならない。koyomi の集合を渡せば、確かめるのは 23 日だけで済む。

二つの形にする。

- **(a) 呼び出しの場所で**：dandori が koyomi の関数の結果を規則の入力に渡すとき、値の集合が規則の入力の範囲に収まるかを確かめる。書き足すものは無い。
- **(b) 規則の宣言で**：規則が入力の範囲を koyomi の関数の値の集合として宣言する（`payment : date  range from koyomi "payment_terms.cal" date payment`。E.4 で rulec の構文に足した。rulec の §15.174）。rulec は、その入力の値をその集合に限って、完全性、重なり、当てはまらない行を確かめる。rulec がもう持っている、宣言した `constraint` で尋ねる組み合わせを絞る仕組み（rulec の §15.55、`RulecCert/Sieve.lean`）と同じ形で、日付の軸に集合を置く。証明書には、集合の出どころ（koyomi のファイルと関数と SHA-256）と集合を書き、Lean の再検査も集合の上で通す（11 章）。

(b) は、rulec が `.proto` の列挙の値を契約から読むのと同じ考えである（rulec の §15.59。値の集合を決めるのは別の成果物で、rulec はそれを読んで表と突き合わせる）。rulec のセルの中では日付を計算しない。koyomi の集合を読めないとき（ファイルが無い、koyomi の検査を通らない、`rulec` を単独のクレートのバイナリで走らせた）は、範囲全体で確かめ直すのではなく、エラーで止める。

**E.4 と E.5 で作った形**。

- **(b)**（E.4）は rulec の §15.174 に書いた。書き方は `<入力> : date  range from koyomi "<ファイル>" date <日付の名前>`（`from` の後ろは 6.2 の名指しと同じ形。★`range` のすぐあとの `from` は射影の始まりとして読まない）。rulec は koyomi の集合を口 `Dates` から読み（`rulec::days::with`、`ports::Engine::with_dates`。`ritsu-project` の `Joined` と `ritsu rulec` が koyomi をつなぐ）、日付の軸の座標のうち集合の日を一つも含まないものを「起きない」と読む。完全性の穴、重なり、当てはまらない行、例、ベクタ、証明書の点は、どれも集合の日だけを見る。生成コードは集合の外の日を入口で受け付けない（範囲の両端だけでは、表が確かめていない日が入口を通る）。証明書は上位の `days` に出どころ（koyomi のファイルと日付の名前、ファイルの SHA-256）と日の並びを書き、覆いの葉 `{"days_axis": 軸}` を `tools/recheck.py` と Lean の再検査（`RulecCert/Sieve.lean` の `daysRulesOut`、`not_asked_of_days`）が確かめる。koyomi がつながっていなければ E129（★終了コード 2。dandori の E018、yuen の E206 と同じ扱い）、集合を読めなければ E130 で、どちらも範囲全体で確かめ直さずに止める。`ritsu check` は、検査を通る規則の `range from koyomi` の入力ごとに、境目を一つ「確かめた」と数える（rulec が自分の検査で確かめたもの。通らない規則は rulec が言うので数えない）。
- 7.5 の例の数は、新しい書き方で走らせても同じだった。koyomi の例の日本語の版（当時の名前は `支払_20日締め翌月10日払い.cal`）の `支払日` を範囲にした規則を `ritsu rulec check` と `certificate` にかけると、集合は 23 日（2026-02-10〜2027-12-10、月の 8 日か 9 日か 10 日）で、rulec の `doc` のページの注記に 23 日が並ぶ。rulec のテストの材料は英語の小さなもの（`crates/rulec/tests/days/payment_terms.cal`。20 日締め翌月 10 日払い、カレンダーなし、受領日 2026-01-01〜2026-12-20 の 354 日に対して支払日は 12 日）にした。表 `settlement.rule` は、すべての日の上では 2026-07-01〜07-09 と 2026-12-11〜12-31 が穴になり（E101、例は 2026-07-01）、支払日の上では完全である。
- **(a)**（E.5）：規則の日付の入力に渡す値で、koyomi の日付の日から来うるものごとに、境目を一つ数える（`ritsu_cross::dates`）。範囲を `range from koyomi` にした入力は、日が規則の前提なので、ここでは数えず X2 が日ごとに確かめる（7.4）。判定（`borders::days_given`）は、値が来うる koyomi の日付ごとに、koyomi が数えた日（`Dates::values`）が、規則の宣言した範囲（`Rules::date_range`。両端を含む。開いた端は制限しない）に収まるかを見る。収まらない日があれば、最初の日付の最初のその日を例にして E202（注に、koyomi がその日を返す最初の入力を `Dates::input_for` で書く）。例が無く、値が何日かを言わないところからも来うるか、koyomi が数えない日付があれば W202。どれでもなければ成り立つ。
- 判定は、koyomi の日付の入力の範囲のすべてで数えた日の上でする。フローが日付に渡す日が範囲の一部だけなら、例の日にはならないことがある。フローが日付に何を渡すかは X6 が見る。
- 例：支払日（20 日締め翌月 10 日、受領日は 2026-01-01〜2026-12-20）を、範囲が `>=2026-03-01 <=2027-01-31` の規則に渡すと、`error[ritsu E202]` で、`due.day` は 2026-02-10 になりうる（koyomi "payment_terms.cal" date payment が received = 2026-01-01 のときに返す日）と言う（`crates/ritsu/tests/golden/cross/x3a-E202.en.txt`）。

Lean の `RitsuCross` は、koyomi の集合を `KoyomiModel` で数え直し、(a) が成り立てば koyomi の範囲のどの入力の日も規則の範囲にあること、(b) で規則の証明書が通れば koyomi の範囲のどの入力の日についても規則がちょうど一つの行で答えることを証明している（11.2 の 3）。

### 7.6 規則の出力から、振替の額へ（X4）

dandori が規則の出力を chobo の振替の額に渡すとき、次を確かめる。

- 額が chobo の受け取れる範囲（0〜2⁶³ − 1）に収まる。規則の出力が負になりうるなら（返金など）エラーで、その出力を返す規則の入力の例を添える。chobo は範囲の外の額を、拒否（業務の結果）ではなく失敗にする（chobo の DESIGN 1.5）からである。
- その額の範囲で、どの操作がどの理由で拒否されうるか。いまの chobo の検査は、額に 1、2、3、5、10、100、1000 を試し、入れる振替を三つ先までたどって、例が見つかった理由だけを並べる（chobo の DESIGN 3.1 と 11.1。手数料が 0 のときだけ起きる理由が漏れる例がある）。ritsu では、試す額を規則の出力の範囲（表の出力なら値の集合）から選ぶ。そうして見つかった理由と、dandori のタスクが宣言したエラーを突き合わせる。起きうる理由を処理していなければエラー、起きない理由を宣言していれば警告。たどる深さは chobo の探し方のままなので、「起きない」と言えるのはその深さまでで、診断にもそう書く。

**E.5 で作った形**（`ritsu_cross::transfers`。rulec と chobo の問いと判定は E.4 で先に作り、呼び出しの場所には E.5 でつないだ）。振替の操作を呼ぶところで、渡す額のどれかが規則の数の出力から来うるものだけを見る。規則から来ない額は、dandori がタスクの範囲で確かめ（dandori の E014）、拒否されうる理由は chobo の検査が言うものである。

- 額（`borders::amounts_given`）：規則の出力から来うる額ごとに、境目を一つ数える。額が来うるところごとに見る。規則の出力は rulec が数えた最小と最大（`Rules::output_values`。行がどれも数を書くならその数の集合）が 0 以上 2⁶³ − 1 以下か、dandori が範囲を知っている数はその範囲がそうか。外れる額があれば E203 で、例は最初の外れる額（行がどれも数を書く出力なら、最初の負の数）と、その額になる規則のベクタの入力。例が無く、範囲の端が無いもの、範囲の分からないところがあれば W203。
- 拒否の理由（`borders::refusals_met`）：額が規則の出力から来うる `do` と `hold` ごとに、境目を一つ数える。額を、渡す額の全部の最小と最大を chobo の受け取る額（0〜2⁶³ − 1）に切り詰めた範囲（`borders::amounts_hull`）に限って、chobo の探索（`Books::refusals`）を走らせ、その操作が拒否されうる理由と、タスクが宣言したエラーを比べる。比べるのは帳簿の境界の理由（勘定の `refused as`）だけである（★）。境界の理由は額で決まるが、ほかの理由（`key_conflict`、`already_refused`）は前の呼び出しで決まり、どの額でも起きうるからで、比べれば、その処理をどの `do` と `hold` にも求めることになる。探索が見つけた境界の理由をタスクが宣言していなければ E204（例が見つかった）。タスクが宣言した境界の理由を探索が見つけなければ W204（探索は chobo の検査の深さまでなので、起きないと言えるのはその深さまで。決められないとして数える）。額の範囲が決まらない（端が無い、範囲の分からないところがある、受け取る額が残らない）、帳簿が答えないときも W204。`post` と `void` は数えない。仮押さえの状態で拒否され、dandori が案件の状態ごとに確かめるからである（dandori の E022）。
- 境界の理由でも、勘定を前もって境界の近くまで満たせば、どの額でも起きうる。chobo の探索はそれも試す（ほかの呼び出しが同じ勘定を満たす場合）。額の範囲で起きないと言えるのは、一度しか動かない勘定（キーがその勘定の引数を全部含み、ほかの振替が入れない）の境界である。再現のホール（`hall.book`）がその形で、催しごとの座席の勘定 `given(event)` は、催しをキーにした `assign` でしか増えない。
- 例：座席数を返す規則が 30 と 80 なら、額も理由も成り立つ。400（ホールの 300 を超える）を返すなら、`assign.do` は `over_capacity` で拒否されうるので、それを処理しないタスクは E204。返却に −20（20 席を返す）を返すなら E203（負の額は向きの違う振替に分ける）（`crates/ritsu/tests/golden/cross/x4-*.txt`）。判定そのものは、`crates/ritsu-cross/tests/borders.rs` が、本物の rulec と chobo の答え（chobo の例 `inventory.book` の `reserve` を額 300〜800 に限ると `hold` が `out_of_stock` で拒否されうること、など）で確かめる。

chobo の探索（`crates/chobo/src/witness.rs` の `passing_call`）は、額を範囲に押さえたとき、範囲の上の端がその操作に要る最小の額に届かなければ、すぐに「そういう呼び出しは無い」と答える。前は範囲の中で額を作り直し続け、在庫の例の `reserve` を額 1 に限ると終わらず、それを呼ぶ `ritsu check` も終わらなかった（Lean の担当が見つけ、取り込むときに直した。chobo の `tests/ports.rs` に、額を 1 に押さえた `reserve` の拒否がすぐ返るテストがある）。X4 を額 0 から数えるようにした担当が見つけた、額の引数が五つ以上ある振替の五つ目からの額（前は、探索を限った範囲を見ずに 1 を入れうる）も、取り込むときに、探索を限った範囲の下の端を使うように直した（`held_to().map_or(min.max(1), |(lo, _)| min.max(lo))`。リポジトリの帳簿には額の引数が三つ以上の振替が無いので、どのテストも届かない）。

Lean の `RitsuCross` は、額の判定が chobo のインタプリタ（`ChoboModel.fits`）と過不足なく同じことを証明している。判定が通す額は `fits` が受け取り（`amountFits_chobo_takes`）、判定が例に挙げる額は受け取らない（`amountFits_fails_chobo`）。F の終わりまでは、X4 が額 0 を E203 でエラーにし、chobo（DESIGN 1.5、`interp.rs`、`ChoboModel.fits`）は 0 を受け取っていた。作者が chobo に合わせて 0 を通すと決め（PLAN の 7.10）、判定（`amount_fits`、`amounts_given`、`amounts_hull`）、額を範囲に限った chobo の探索（`witness::within`、`Books::refusals`）、台帳の E203（文と再現）、Lean のモデルを、0 からにそろえた。額を範囲に限った探索は、範囲が 0 だけなら 0 で探す（下の端を 1 のままにすると `(1, 0)` の空の範囲になり、範囲の外の額を作ってしまう）。探索が見つける理由にはそこへ至る操作の例があるので、額 0 で見つけた理由は、0 を渡すフローで実際に起きる。範囲が 0 を含むのは規則の出力が 0 になりうるときだけで、そのとき額 0 で拒否される理由は、タスクが処理すべき理由である。探索が範囲に限られていないとき（chobo 自身の検査）の額は、これまでどおり 1 から試す。★額 0 は、ritsu が何も言わずに通す。0 の振替を業務として避けたいかは帳簿の書き手が決めることで、X4 は chobo が受け取るかだけを確かめる。E203 の再現は、ウェビナーが席を 0 席使う形から、返却が 20 席を返す形（−20）に替えた。

### 7.7 仮押さえの有効期限と、待ちの長さ（X5）

dandori が chobo の仮押さえを案件として追うと（7.8、`case … follows <帳簿>.<振替>`）、いまの dandori の検査（E020 など）が、chobo のステートマシンでそのまま効く。有効期限切れは外で起きるイベントで、確定の前に期限が切れうることも数える（dandori の DESIGN 2.2）。

ritsu では、さらに長さを比べる。仮押さえを作ってから確定するまでの長さに下限があり、それが有効期限を超えるなら、確定は必ず期限切れで拒否される（エラー。確定が通る分岐は通らない）。上限があり、それが有効期限より短いなら、期限は切れない（期限切れを処理する分岐は通らない。警告）。

長さの下限と上限は、仮押さえと確定のあいだの文から求める。

- `wait <n> days` などの決まった長さの待ちは、そのまま下限にも上限にも入る。
- `wait until <時刻>` の時刻が koyomi の関数の `at` で、その関数に渡す日付が仮押さえを作った日なら、koyomi が入力の範囲のすべてで数えた日数の最小と最大から、下限と上限が出る。7.5 の例の支払日なら、受領日の終わりに押さえて 18 日後の 9 時に確定する場合が最短で、17 日と 9 時間である。有効期限が 14 日なら、必ず切れる。
- 待ち以外の文（タスク）は、タイムアウトとリトライの回数がわかれば上限に入り、下限には 0 として入る。タイムアウトの無いタスクがあれば、上限は無い。

「渡す日付が仮押さえを作った日である」ことを、いまの dandori は言えない。dandori には時刻を読む式が無く、日付はタスクの結果か入力としてしか入ってこないからである。そこで、その文を走らせた時刻を読む式 `now` を dandori に足す（E の段階。dandori の構文を足す。書き方の細部は dandori の DESIGN に書く）。Temporal には、再生しても同じ時刻を返す `workflow.now()` がある。ほかのプラットフォームでの読み方は、そのとき調べて、作れないプラットフォームは dandori の P6 のとおり E050 にする。作るまでのあいだは、決まった長さの待ちだけで下限と上限を求め、ほかは「決められない」として、いまと同じく期限切れが起きうるものとして数える。

`now` は E の段階で dandori に足した（dandori の DESIGN 1.16、4.6）。どのプラットフォームでも作れたので、E050 にしたものは無い。Temporal ではワークフローの時計（TypeScript の `Date.now()`、Python の `workflow.now()`、Go の `workflow.Now`。再生でも同じ時刻）、Step Functions ではそのステートに入った時刻（`$states.context.State.EnteredTime` を秒まで。Retry のあいだも変わらない）、Lambda durable functions では文の前に時計を読む step（答えはチェックポイントに残る。操作を一回使うので E040 が数える）、Argo では値を計算するテンプレートの出力の式の `now()`（そのテンプレートに来たときに一度だけ評価される）、pydantic-graph では `Deps.clock` から読む。値は秒までの UTC の `timestamp`。参照インタプリタはシナリオの `now`（無ければ `2026-03-31T15:30:00Z`）を読み、ランナーはどのプラットフォームの時計もその時刻に替えて突き合わせる。

**E.5 で作った形**。長さは dandori が数え（流れの口の `HoldSpan`。dandori の DESIGN 0.3）、ritsu-cross が仮押さえの有効期限と比べる（`ritsu_cross::holds`、`borders::held_until`）。

期限のある仮押さえを追う案件（`case … follows <帳簿>.<振替>`）の、確定（`sends post`）と取消（`sends void`）の呼び出しごとに、境目を一つ数える。取消も数えるのは、期限の切れた仮押さえは取消も `expired` で拒否されるからである。長さの数え方（dandori の `src/crossings.rs`）は次のとおりで、フローの分岐をたどり、分かれた先ごとに数える。

1. 仮押さえは、それを作る呼び出しの中でできる。だから、その呼び出しのあとで、仮押さえを作ってからの時間は 0 秒以上で、その呼び出しの最大の長さ以下である。
2. 呼び出しは最小 0 秒、最大は `timeout` に、`retry` の回数だけの試しと、あいだの待ち（間隔に倍率を掛けていったもの）を足したもの。`timeout` の無いタスクと、規則と日付の呼び出し（`.flow` に時間の上限が無い）は、上限が無い。
3. `wait <n>` は、ちょうどその長さ。決められない前提を確かめる文（`TK::Check`。7.4）は、時間を取らない文として `pass` と同じに扱う（取り込むときに足した）。
4. `wait until x.at` は、`x` が koyomi の日付の答えで、日付の入力に `now`（または `now` を入れた変数）を渡し、その `now` を仮押さえのあとで読んだときだけ数えられる。koyomi が入力の範囲のすべてで数えた、入力から日付までの日数を d_min〜d_max、日付の時刻（`at`）を a とする。`now` はその日のどの時刻でもありうるので、`now` から日付の時刻までは、(d_min − 1) 日 + a より長く、d_max 日 + a 以下である。待ちは始まるより早くは終わらないので、待ちが終わるとき、仮押さえを作ってからの時間は、最小が「`now` を読んだときの最小 + (d_min − 1) 日 + a」と「待ちを始めるときの最小」の大きいほう、最大が「`now` を読んだときの最大 + d_max 日 + a」と「待ちを始めるときの最大」の大きいほうである。ほかの `wait until` は最小 0 秒で、上限が無い。
5. `match` の分岐と、呼び出しのハンドラーは、分かれた先を合わせる（最小の最小、最大の最大）。ループ（`repeat`、`for`）は最小 0 秒、最大は一回でいちばん長くかかる場合の長さを、回せる回数だけ。ループの中で作った仮押さえは、ループのあと、残りの回数ぶん古くなりうる。
6. `on failure` と `on cancel` は、仮押さえを作ってからの時間が 0 秒から始まり、最大は、失敗やキャンセルが届きうるどの点（呼び出しと待ち）での最大よりも大きくない。
7. 確定や取消は、その呼び出しの中で効く。だから、境目の最小は呼び出しの始まりでの最小、最大は始まりでの最大に呼び出しの最大の長さを足したもの。

判定（`borders::held_until`）：有効期限を E 秒とする。chobo では、期限に時計が届いた仮押さえは期限切れなので（chobo の DESIGN 2.5、参照インタプリタの `now >= deadline`）、最小が E 以上なら、呼び出しはどの実行でも期限切れのあとに来る。例がある（E206。注に最小と、それを作る文）。最大が E より小さければ、どの実行でも期限の前に来るので成り立つ。何も言わない。どちらでもなければ決められない（W206。注に最小と最大か、上限を作れない最初のもの）。

例：7.5 の形の支払日（`at 09:00`、休みなら前の営業日）までの `wait until` のあとで確定する請求は、最小が 17 日 9 時間で（2026-02-20 に受けると支払日は 2026-03-10、18 日後）、期限が 14 日なら E206、60 日なら W206（仮押さえを作るタスクに `timeout` が無い）。三日待ってから確定し、どの呼び出しにも `timeout` がある形は成り立つ（`crates/ritsu/tests/golden/cross/x5-*.txt`）。

★ 成り立つとき（期限は切れない）は何も言わない。この節のはじめの案は、そのとき `expired` を処理する分岐が通らないことを警告にしていた。それをやめたのは、dandori の検査が長さを比べず、期限が切れうるものとして `expired` の処理を求める（dandori の E022）ので、警告を消す手段がフローに無いからである。また、最大はタスクの `timeout` から出すが、プラットフォームは `timeout` の前の待ち（Temporal でタスクキューからワーカーに取られるまで、Argo で Pod がノードに置かれるまで）を限らない。成り立つという答えは、その待ちが短いときのものである。それでも、分岐を残しておけば実行は安全なので、何も言わない形にした。

作るまで「決められない」としていた二つ（`now` と日付の時刻）は、どちらも作った。`now` を仮押さえより前に読んだとき、日付の入力に日付で入ってきた値を渡したときは、待ちの長さは分からない（最小 0 秒、上限なし）。

Lean の `RitsuCross` は、`held_until` の判定をモデルにし、例を言えばどの呼び出しも有効期限かそれより後に来て、そのとき chobo のインタプリタ（`ChoboModel.expiredAt`）が仮押さえを期限切れと言うこと、成り立てばどの呼び出しのときも期限が切れていないことを証明している（11.2 の 3）。秒数の最小と最大は dandori が数えたものを受け取る。

### 7.8 koyomi の関数と chobo の振替を、dandori から呼ぶ（X6）

dandori に、期日と帳簿を読む宣言を足す（E の段階。dandori の構文を足す）。書き方の案は、chobo の DESIGN 5 章のもの（`use book 在庫 from "在庫.book"`、タスクの呼び方 `book 在庫.引当.hold`、`case 押さえ : 引当 follows 在庫.引当`）と、それに合わせた `use dates 支払条件 from "支払条件.cal"`（呼び出しは規則と同じく `let d = 支払条件.支払日(受領日: …)`）である。

- koyomi の関数の呼び出しは、規則と同じくアクティビティ（各プラットフォームのタスク）にする。koyomi の関数は純関数だが、祝日の表が毎年変わるので、ワークフローのコードの中で計算すると、表を入れ替えたワーカーで再生が食い違う。規則を普通のアクティビティにした理由（dandori の DESIGN 4.2。判定の記録が履歴に残る、直しても再生が食い違わない）と同じである。
- 渡す日付が、関数の入力の範囲とカレンダーのデータの範囲に収まるかを確かめる（X6）。収まらない日付があればエラーで、その日付とそこに至る実行を添える。dandori の範囲は数にだけ書けるので、日付の範囲を求めて運ぶ仕組みを dandori に足す案だった（E では足していない。カレンダーのデータの範囲とも比べない。下の「E.5 で作った形」）。
- chobo の振替の呼び出しは、タスクの呼び方の一つにする。拒否された理由は、タスクの宣言したエラーとしてそのまま使える（chobo の DESIGN 5 章）。chobo の操作はキーで冪等なので、dandori の E030（キーの無いリトライ）にあたらない。
- どちらも、dandori のすべてのプラットフォーム（Temporal の TypeScript・Python・Go、Step Functions、Lambda durable functions、Argo、pydantic-graph）で作る。koyomi は TypeScript・Python・Go のコードを、chobo はその三つの言語のクライアントを生成するので、作れないプラットフォームは無かった（E.5 で七つの全部に作った。dandori の DESIGN 1.16）。作れないものが見つかれば、dandori の P6 のとおり E050 にする。生成、参照インタプリタの見え方、E040 と E050、ランナーと突き合わせのテスト、README、DESIGN の全部に載せる（作者の決まり）。

dandori の側の書き方は、E の段階でこう決めた（dandori の DESIGN 1.16、2.7）。

- `use dates <名前> from "<file.cal>"` の下に `lambda "<関数>"`（Step Functions と Lambda durable functions で日付を計算する関数）と `local`（Temporal のローカルアクティビティ）を書ける。規則のものと同じ意味である。日付の呼び出しは `<名前>.<日付>(<入力>: …)` で、`{ day: date, at: timestamp }`（時刻を言わない日付は `day` だけ）を返す。日付の入力に時刻を渡せるのは、カレンダーが UTC オフセットを言うときだけで（そのオフセットでその時刻にあたる日として読む）、言わなければ E003。
- `use book <名前> from "<file.book>"` の下に `lambda "<関数>"`（Step Functions で操作する関数）を書ける。タスクの呼び方は `book <帳簿>.<振替>.<操作>`（`do`・`hold`・`post`・`void`）。引数は振替の引数を名前のとおりに書き（`post` に数も書けば一部の確定）、拒否された理由が宣言したエラーの名前になる。`key` と `refused as` は書かない（E007）。結果の仮押さえのレコードは、クライアントが仮押さえを返さないので、渡した引数と操作から作る。
- 仮押さえの案件は `case <名前> : <帳簿>.<振替> follows <帳簿>.<振替>` で、レコードは仮押さえのレコードそのものである。chobo の DESIGN 5 章の案と違い、`external expire` は書かなくても帳簿から足し（期限のある振替だけ）、`refused when` を書くとエラーにする（E008）。拒否の理由は状態ごとに帳簿が決めていて、理由ごとに受けているかを E022 で見る。
- 帳簿の口に、クライアントが振替と引数をどう名付けるか（`BookFacts` の `typescript`・`python`・`go`）を足した。koyomi の生成したコードの名前は、口に足さず、koyomi と同じ ritsu-emit の決め方で求める。日付の口と帳簿の口に `joined` を足した（3.2）。
- rulec の `date` の列も、dandori では文字列ではなく `date` として読む。日付のファイルの日付を規則の入力に渡す形（7.5 の X3 の (a)）の前提である。
- 七つの出力先のすべてに作った。日付は規則と同じくアクティビティ（Step Functions と durable functions は Lambda、Temporal は `dates_<ファイル>_<日付>` のアクティビティ、`local` ならローカルアクティビティ、Argo は caller のコンテナ、pydantic-graph は関数）で、koyomi が生成する TypeScript・Python・Go を呼ぶ。帳簿の操作は、Step Functions では帳簿の Lambda 関数（chobo の Python のクライアントで操作するコードを dandori が書く）、ほかでは `Transport` の `book`（`io.ts`・`io.py`・`io_books.go`）が利用者の渡す chobo のクライアントを呼ぶ。

**E.5 で作った形**（`ritsu_cross::dates`）。koyomi の日付を呼ぶところごとに、日付の入力（`date` の入力は一つだけ。koyomi の DESIGN 1.3）に渡す値で、境目を一つ数える。整数の入力は、規則の入力と同じく dandori が自分の範囲の検査で確かめる（dandori の E014、W104）。判定は X3 の (a) と同じ `borders::days_given` で、範囲は日付の入力の範囲（`borders::input_range`）である。値がほかの koyomi の日付の日から来うるなら、koyomi が数えたその日の全部で確かめ、外れる日があれば E205（注に、その日を返す koyomi の入力）。ワークフローの入力、タスクの結果、`now` から来うるなら W205。

- ★ カレンダーのデータの範囲とは比べない（E.4 の判定は比べていた）。koyomi の検査は、入力の範囲のすべての入力について、カレンダーに問い合わせる日がデータの範囲に収まることを確かめている（koyomi の E203）。だから範囲の中の日は koyomi が計算できる日で、範囲の外の日は koyomi の生成したコードが受け付けない。データの範囲と比べると、問い合わせない日（暦日を足すだけの日付）を、誤って外れると言うことがある。
- ★ dandori に日付の範囲を書く書き方は、足していない。dandori では、入ってくる値の範囲は、走らせたときに生成したコードが確かめる（dandori の DESIGN 1.3）。書き方を足すなら、七つのプラットフォームの生成コードに日付の範囲の確かめを足し、ランナーで突き合わせることになる。そのため、ワークフローの入力やタスクの結果や `now` から来る日は、どれも W205 になる（数の W104 にあたる）。koyomi の日付をほかの日付に渡すつなぎだけが、決められる。
- 例：支払日を、範囲が 2026-02-01〜2026-12-31 の催促の日付に渡すと、`error[ritsu E205]` で、`due.day` は 2027-01-10 になりうる（received = 2026-11-21 のとき）と言う（`x6-E205.en.txt`）。

Lean の `RitsuCross` は、成り立てば渡す値は koyomi の日付の日にしかならず、そのどの日も日付の入力の範囲にあること、例がその日になりうる koyomi の日付の日で範囲の外にあることを証明している（`daysGiven`、`inputRange`）。

### 7.9 一つの参照インタプリタ（X7）

`ritsu run <file.flow> --scenario <file.json>` は、dandori の参照インタプリタで、規則の呼び出しを rulec の評価器で、期日の呼び出しを koyomi のインタプリタで、振替を chobo のインタプリタ（帳簿の状態を持つ）で計算しながら流す。ほかのタスクの結果は、いまと同じくシナリオに書いたものを使う。

いまの `dandori scenarios` は、規則の結果も選ぶ（どの分岐も通るように）。それはそのまま残す（プラットフォームとの突き合わせは、規則の結果を選べる方が分岐を全部通せる）。`ritsu run` は、入力から規則の結果を計算する、もう一つの流し方である。規則の入力の選び方には、rulec の `vectors`（境界から作った入力）を使える。

これは証明ではない。三つの言語の意味を一度に流せる参照で、一つの生成パッケージ（9.3）との突き合わせの基準にする。

**E の二つ目の部分で作った形**（PLAN の E.6）。

- 流すのは dandori の参照インタプリタそのものである。インタプリタに、呼び出しの結果をシナリオの並びのほかから受け取る口を一つ足した（`dandori::interp::Answers`）。`ritsu run` が渡す `dandori::computed::Computed` は、規則の呼び出しには rulec の参照評価器（`Rules::eval`）の出力を、日付の呼び出しには koyomi のインタプリタ（`Dates::eval`）の日と時刻を、帳簿のタスクには chobo の参照インタプリタ（`Books::open` で開いた `Ledger`）が操作した結果を返す。ほかのタスク（HTTP、Lambda、AWS の API、エージェント、Jev、子のフロー、イベント、コールバック）の結果は、`dandori run` と同じくシナリオの並びから取る。シナリオの並びには、計算しない呼び出しの結果だけを書く。
- 結果はどれも、シナリオに書く形（`{"ok": …}`、`{"error": …, "cause": …}`）で返す。参照インタプリタはそれをシナリオの結果と同じに扱う（リトライ、エラーの名前、結果の検査）。だから `ritsu run` の実行は、計算した結果を並びに入れたシナリオを `dandori run` で流したものと、どのターゲットの見え方でも一字も違わない。`--format json` の `replay` がそのシナリオである。フローの意味は dandori の参照インタプリタが決め、rulec・koyomi・chobo は結果だけを決める（P4）。
- 値の渡し方と結果の読み方は、dandori が生成するコードと同じにした（dandori の DESIGN 5 章）。日付の入力に渡す時刻はカレンダーのオフセットで日に読み、`at` は koyomi の時刻の決め方で書く。Connect で呼ぶ規則の結果は、サービスが書く形にしてから読み直す。規則が入力を受け付けなければ失敗になり、規則のほかの呼び出しと同じくリトライする。宣言していない理由で拒否された帳簿の操作は `Dandori.Failure.<理由>` の失敗になる（Step Functions の見え方では理由そのもの）。
- 帳簿は、シナリオの `books`（`use book` の名前ごとに、`chobo run` のシナリオと同じ形の操作の並び）の操作を済ませたところから始まる。走らせる前の操作が拒否されたら、シナリオの誤りとして止める。
- 時間：実行はシナリオの `now` に始まり、`wait`（`wait until` はその時刻まで）、リトライの前の待ち、タイムアウトした呼び出しのタイムアウトの長さだけ進む。結果が返った呼び出しには時間を数えない（シナリオはかかった長さを言わず、0 がありうるうちでいちばん短い）。帳簿にも同じ時間がたち、有効期限の来た仮押さえは期限切れになる。`now` の値は、参照インタプリタのとおり、シナリオの時刻のままにした（dandori の DESIGN 1.16）。
- テキストは、走らせる前の帳簿の操作、呼び出しごとに誰が結果を返したか（rulec、koyomi、chobo、scenario）とその結果、待ちとその長さ、期限が切れた仮押さえ、終わり方、終わりの帳簿（`chobo run` と同じ勘定と仮押さえ）を出す。JSON は一つのオブジェクトで、キーは `ritsu`、`flow`、`scenario`、`target`、`diagnostics`、`start`、`end`、`elapsed`、`events`、`trace`（`dandori run` が出すもの）、`books`、`replay` の順である。
- 終了コードは、最後まで流れたら 0（成功、失敗、キャンセルのどれで終わっても）、フローにエラーがあるか流れが途中で止まったら 1（シナリオの結果が足りないなど）、引数の誤り、読めないファイル、流せないシナリオなら 2。

テストのプロジェクト `crates/ritsu/tests/projects/invoice/` の請求書払いのフローは、受領が 4 月 21 日なら、支払日が 6 月 10 日の 9 時になり、待ちは 49 日と 23 時間になる。30 日で切れる仮押さえは待ちのあいだに期限切れになり、出荷は chobo が `expired` で拒否する（`crates/ritsu/tests/golden/run/invoice_expired.en.txt`）。X5 の検査（7.7）が「必ず期限が切れる」と言う場合を、一回の実行で見せたものである。

```
$ ritsu run invoice.flow --scenario scenarios/invoice_expired.json
invoice.flow (scenario scenarios/invoice_expired.json, from 2026-04-21T10:00:00Z)
before the run, book stock:
  receive.do(delivery: "D-1", sku: "pen", qty: 10)  done
the run:
  reserve(order: "A-1", sku: "pen", qty: 3)        chobo     done
  method(member: true, amount: 120)                rulec     {"billing":"invoice"}
  terms.payment(received: "2026-04-21T10:00:00Z")  koyomi    {"day":"2026-06-10","at":"2026-06-10T09:00:00Z"}
  wait until 2026-06-10T09:00:00Z                            49 days 23:00:00, to 2026-06-10T09:00:00Z
  the hold reserve(A-1, pen) of the book stock               expired
  check_payment(order: "A-1", due: "2026-06-10")   scenario  {"paid":true}
  ship(order: "A-1", sku: "pen")                   chobo     refused: expired
end: succeed {"outcome":"hold_expired","due":"2026-06-10"}
book stock (books/stock.book) at the end:
  accounts:
    shelf(pen)  posted 10, held out 0, held in 0
    suppliers   posted -10, held out 0, held in 0
    customers   posted 0, held out 0, held in 0
  holds:
    reserve(A-1, pen)  expired
```

テスト（`crates/ritsu/tests/run.rs`）は、プロジェクトの二つの版（英語の `invoice.flow` と日本語の `invoice.ja.flow`）の五つのシナリオ（請求書払いで出荷、期日の前に仮押さえが切れる、リトライのあと未払いで戻す、前払い、在庫切れ）を、それぞれの言語のテキストと JSON の golden と比べる。ほかに、`replay` を `dandori run` で流すと同じトレースになること（reference と Temporal の見え方）、各言語が返した結果がその言語のコマンドの出すものと同じであること（規則は `rulec replay`、日付は `koyomi eval`、帳簿は、走らせる前の操作と実行の中の操作をそのあいだの時間と一緒に `chobo run` で流し、操作ごとの結果と終わりの勘定と仮押さえ）、二つの版が名前のほかは同じに流れること、Connect で呼ぶ規則の結果がサービスの書く形で返り、同じに終わること、止まる場合（終了コードと文）を確かめる。

### 7.10 yuen、sakai、名指し（X8〜X10）

- **yuen**：段階 C で止めていた「一式の読み込み」（yuen の PLAN の C.1〜C.9）を、子プロセスと JSON ではなく、`Rules`・`Dates`・`Books`・`Claims`・`Items` の口で作る。端は 6.4 の定義の文になり、表や日付の関数や主張の一つ一つを追える。借りた出典は土台の出典（4.6）から読み、E107 も同じ手続きで比べる。`affected` は geas の記録を geas の口で読む。D の最後の部分で、そう作った（PLAN の D.7）。読むのは `Items`（中のものと定義の文）、`Sources`（規則とカレンダーの出典。D.7 で足した口）、`Rules` と `Dates`（別名だけ）、`Claims`（記録と、D.7 で足した `affected`）で、`Books` は読まない（chobo の中のものと定義の文は `Items` が渡す）。E203 は「名指したものの言語が、そのファイルについて答えられない」に意味を替え、E204 と、記録で主張の名前を確かめていた W201 を退かせた（yuen の DESIGN 6.2）。
- **sakai**：止めていた C.1〜C.5 を、`References` と `Rules` の列挙（`connect.enums` にあたるもの）で作る。dandori の参照も読めるので、N101 は要らなくなる。sakai の DESIGN 4.7 が挙げていた四つの検査（規則の同梱が境界を越える、`connect` で呼ぶサービスが上流の公開ホストサービスでない、`implements` するサービスが自分の公表された言語に無い、子の `.flow` が境界の向こうのもの）を足す。子の `.flow` の扱い（sakai の DESIGN 4.7 の最後の段落）は、そのとき sakai の DESIGN に決める。D の最後の部分で、そう作った（PLAN の D.8）。読むのは rulec の `Rules`（Connect のパスと列挙、入力と出力の名前）と `References`（`import proto`、`shape`、`apply`）、koyomi の `References`（`use calendar`）、dandori の `References`（`use rule`、`use proto`、`connect`、`flow`、`implements`）、chobo の `Books`（doc のための勘定と振替の名前）である。四つの検査は E202（規則の同梱と `apply`）、E207、E208、E209 になった。子の `.flow` は、パートナーシップ、共有カーネル、子が相手の公開ホストサービスを実装しているときだけ許す（sakai の DESIGN 4.7）。E104 は「地図が含む成果物の言語がつながっていない」、E105 は「成果物が、その言語の検査を通らないか、読めない」に意味を替え、N101 を退かせた（sakai の DESIGN 5.2）。
- **名指し**：プロジェクトのどこに書いた名指しも、索引のものを指すかを確かめる。いまは yuen と sakai がそれぞれ確かめている（yuen の E202 など）。言語ごとのコードと文はそのまま残し、引き方だけを索引に替える。E.1 でそうした（6.4）。ritsu の台帳には、この検査のコードを足していない。名指しを書いた言語が、自分のコードで言うからである。子プロセスと JSON のためのコード（yuen の E203「ツールがファイルを読めない」と E204「ツールの JSON が知らない形」、sakai の E104「ツールが無い」と E105「ツールの api が失敗した」）は、出す側の検査のエラーを名指すもの（6.1）に意味を替えるか、退かせる。退かせるコードは台帳に退いたと書いて残し、番号を使い回さない（rulec の docs/compatibility.md と同じ決まり）。

### 7.11 同じ条のコピー（X11）

規則（rulec）、カレンダー（koyomi）、要件（yuen）が同じ法令の同じ条を引くとき、コピーの本文が同じかを確かめる。コピーの場所の決まりは三つとも同じ（4.6）なので、同じプロジェクトの中ではコピーを一つにでき、別々に保存したときは本文を比べる（e-Gov は改正の無い条でも XML の属性を書き換えることがあるので、バイト列ではなく本文で比べる。rulec の §15.71、yuen の DESIGN 3.3）。

### 7.12 一つの `.proto` の読み方（X12）

検査を足すのではなく、`ritsu-proto` に読み手を一つにすること（4.10）で、同じ `.proto` を rulec、dandori、sakai、yuen が違って読む余地を無くす。C の段階で sakai を、D の段階で rulec と dandori を移し、移す前と後で、それぞれのテストの結果が同じことを確かめる。D.10 で rulec と dandori を移した（4.13）。rulec は 1,782 回、dandori は 701 回の出力が移す前と一字も違わず、rulec の契約の突き合わせ（コーパスと変異）と、dandori の `connect`・`implements`・`.proto` の型（`tests/protos.rs` と例）のテストも同じに通った。出力が変わったのは、`.proto` として読めないファイルのときだけである。rulec は途中まで読まずに E013 で止め（★）、dandori の E016 の注と sakai の E106 の日本語の文は言い方が変わった（4.13）。

### 7.13 処理系自身の依存（X13）

3.4 の地図 `ritsu.ctx` を ritsu のリポジトリの根に置き、`ritsu check ritsu.ctx` で確かめる（E.8 で作った）。`ritsu check .` にしないのは、リポジトリには、言語が自分を試すためにわざと通らないファイル（変異、エラーの例）があり、地図の `except` もそれを範囲から外しているからである。言語のクレートが別の言語のクレートを `[dependencies]` に足せば、sakai がその行を名指して止める（`crates/ritsu/tests/map.rs` の変異。koyomi のクレートに rulec を足すと、`error[sakai E201]: crates/koyomi/Cargo.toml:10:1: The file crates/koyomi/Cargo.toml of Calendars depends on crates/rulec of Rules (dependencies), which Calendars has no relationship with`）。CI の `fast` のジョブで走らせる。E の最初の部分（2026-10-04）の地図は `356 artifacts`、`50 crossings` で通った。E の終わり（F の `ritsu-wasm` と `ritsu-model` が入ったあと）には、`ritsu.ctx: ok — 12 contexts, 27 relationships; 381 artifacts, each in one context; 58 crossings checked (rust 58)` で通る（増えた依存の八つは ritsu-wasm のもの。`ritsu-model` は `Testing` の `owns` に足した）。

## 8. 一つの CLI

### 8.1 ritsu のコマンド

```
ritsu check [<パス>...] [--root <dir>] [--format json] [--lang ja|en]
ritsu run <file.flow> --scenario <file.json> [--target <target>] [--format json]
ritsu gen [<パス>...] [--target typescript|python|go] [--out <dir>] [--check] [--books postgres|tigerbeetle] [--name <name>] [--module <path>] [--root <dir>]
ritsu explain <コード> | --all [--format markdown|json]
ritsu <言語> <引数>...        rulec・dandori・koyomi・chobo・geas・yuen・sakai のコマンドそのもの
ritsu --help | --version
```

- `ritsu check` は、パスを渡さなければ今いるディレクトリを読む。各言語の `check` を全部のファイルに走らせ、言語をまたぐ検査をし、最後に一行の要約（言語ごとのファイルの数、確かめた境目の数、決められなかった数）を出す。
- `ritsu explain` は、ritsu の台帳（言語をまたぐ検査のコード）を引く。各言語のコードは `ritsu <言語> explain <コード>` で引く。言語ごとにコードの番号が重なる（rulec の E101 と koyomi の E101 は別のもの）からである。
- `ritsu <言語> …` は、その言語のコマンドと同じものを、すべての口をつないで走らせる。

**E.2 で作った形**（PLAN の E.2）。`ritsu check` は、プロジェクトを読み（6.1 の `Project::load`）、言語ごとの `check` を 6.1 の順に走らせる。rulec、koyomi、chobo、geas、dandori には、プロジェクトのファイルを一つずつ、使う人が書くとおりのパス（走らせたディレクトリから）で渡す。dandori には rulec・koyomi・chobo の口をつなぐ（E の二つ目の部分から。前は rulec の規則の口だけだった。6.1）。yuen と sakai は自分でファイルを探して一つのプロジェクトや地図として確かめる言語なので、渡されたパスのうち自分のファイルを含むものと、プロジェクトのルートを `--root` で渡す。`.proto` には自分の言語の `check` が無い（言語をまたぐ検査が読む。7 章）。

- 各言語は、自分のコマンドが印字に使う関数で、単位（ファイル、yuen のプロジェクト、sakai の地図）ごとに、何をどう印字するかを返す（`ritsu_ports::Checked`。診断一つずつのテキストと JSON、そのほかの行、単位の結果）。`ritsu check` は言語の出したテキストを読み直さない。言語ごとの関数は `rulec::ports::Engine::checked`、`koyomi::ports::Engine::checked`、`chobo::ports::Engine::checked`、`geas::cli::checked`、`dandori::ports::Engine::checked`、`yuen::ports::Engine::checked`、`sakai::run::checked` である。
- geas の `check` は主張を走らせる（プログラムを動かし、ジャーナルを書く）。`ritsu check` でも同じで、`geas check` が走らせるものを走らせる。
- 言語は `--lang` で選び、無ければ `RITSU_LANG`、どちらも無ければ英語である。各言語の `<名前>_LANG` は読まない（一つのコマンドの文面を一つの言語にする）。
- `ritsu explain <コード> | --all [--format markdown|json]` は E.3 で作った（7.1）。
- 言語のコマンドは、どれもライブラリの関数になった。rulec、koyomi、chobo、geas は、E.2 で `src/main.rs` の中身を `rulec::cli::run`、`koyomi::run::run`、`chobo::run::run`、`geas::cli::run` に移した（振る舞いは変えていない。各言語の DESIGN.md）。これで `ritsu <言語>` は七つの全部にある（8.2）。

**E の二つ目の部分で作った形**。`ritsu run` を作った（7.9）。★`--target` は、8.1 の A の段階の形には無かったもので、`dandori run` と同じ八つ（`reference`、`asl`、`temporal`、`temporal-python`、`temporal-go`、`durable`、`argo`、`pydantic-graph`。既定は `reference`）から、トレースの呼び出しをどのプラットフォームの形で出すかを選ぶ。一つの生成パッケージ（9.3）と突き合わせるときは Temporal の見え方を使う。フローの診断は、`dandori run` と同じくテキストなら標準エラーに出し、`--format json` なら JSON の `diagnostics` に入れる。`ritsu gen` は E.7 で作った（9.3）。

`ritsu dandori` は、rulec の規則の口に加えて、koyomi の日付の口、chobo の帳簿の口、ritsu-cross の `Undecided`（7.4）を渡す（`dandori::cli::run_with_undecided`）。これで、日付と帳簿を使うフローも `ritsu dandori` で走る（前は E018 で、`ritsu dandori` で走らせるよう言っていた）。`ritsu run` の担当が同じ行に置いた `run_with_ports` は、取り込むときにこの形にまとめた。リリースのリンクの名前（`dandori` という名前の `ritsu`）も同じ関数を通るので、同じくつながる。

### 8.2 各言語のコマンドの残し方

- 名前は残す。`rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`yuen`、`sakai` は、リリースでは `ritsu` を指すリンクで、呼ばれた名前の言語として動く（2.3）。`ritsu rulec check …` と `rulec check …` は同じである。
- コマンド、フラグ、終了コード、診断、`--format json` と `api` の形は変えない（P6）。変わるのは、ほかの言語を同じプロセスの中で読むようになることだけである。dandori は rulec を子プロセスで走らせなくなり、`DANDORI_RULEC` は要らなくなる（D の段階で消し、README と dandori の DESIGN を直す）。yuen と sakai が子プロセスで呼ぶために予定していた `YUEN_RULEC`・`SAKAI_RULEC` などの環境変数は作らない。
- `--version` は `<名前> <ritsu のバージョン>` を一行で出す（`rulec 0.23.0` など。13.1）。`rulec --version` を読むスクリプトは、そのまま動く。
- 言語の環境変数（`RULEC_LANG` など）は残し、全部に効く `RITSU_LANG` を足す。

E.2 で、`ritsu <言語>` を七つの全部に作った。rulec、koyomi、chobo、geas のコマンドはそれぞれのクレートのバイナリと同じ関数を、dandori、yuen、sakai のコマンドは ritsu-project が一度つないだ口（6.1）を渡して呼ぶ。`ritsu` を言語の名前で呼ぶと（`rulec` という名前のリンク）、その言語のコマンドとして動く（2.3）。`crates/ritsu/tests/entry.rs` が、七つの `--version`、いくつかのコマンド、`koyomi` と `rulec` という名前のリンクで確かめる。

`cargo install --git <ritsu のリポジトリ> --locked ritsu` が入れるのは `ritsu` のバイナリ一つで、言語の名前のリンクは作らない（`crates/ritsu` のバイナリは `src/main.rs` の一つだけで、`[[bin]]` も `src/bin` も無い）。リンクを作るのはリリースのアーカイブ（`packaging/archive.sh`）か、使う人自身である（`ln -s "$(command -v ritsu)" ~/.cargo/bin/rulec`）。リンクの名前から言語として動くことは `crates/ritsu/tests/links.rs` が七つの全部で確かめ、根の README の「コマンド」の節は、アーカイブにはリンクが入ること、`cargo install` では `ritsu <言語> …` で呼ぶか自分でリンクを作ることを書く（`crates/ritsu/tests/readme.rs` が、`ritsu` のバイナリが一つであることと、手で作ったリンクが言語として動くことを確かめる。F.3）。

### 8.3 出力

`ritsu check` のテキストは、ファイルごとに、その言語の `check` が出すとおりの診断を出し、そのあとに言語をまたぐ検査の診断を出す。言語ごとに番号が重なるので、`ritsu check` の中でだけ、見出しの括弧にツールの語を足す（`error[rulec E101]`、`error[ritsu E201]`）。rulec の診断の枠（`-->` で場所を示す形）は、そのまま使う。

**E.2 で決めた形**。テキストは、言語の `check` が単位ごとに出すもの（診断、`ok …` の行、要約、chobo の報告など）を、そのまま 6.1 の順に並べる。足すのは、診断の見出しのツールの語（見出しの最初の `[<コード>]` を `[<ツール> <コード>]` にする）と、最後の一行の要約だけである。sakai の英語の例をコピーしたプロジェクト（`crates/ritsu/tests/projects/shop`）で、受注が注文の状態に値を足したとき（`order.proto` に `ORDER_STATUS_RETURNED = 5;`）は次のようになる（`crates/ritsu/tests/golden/check/shop-returned.en.txt`。途中の、通るファイルの行は省いた。日本語の版のプロジェクト `通販` の同じ場合は `通販-returned.en.txt` で、境目の数と診断の数は同じ。E032 の注の二行は、言い方の担当が直したあとの形である。この例の列挙は値の名前が ASCII なので `returned` をそのまま足す形を言い、値の名前を日本語で書いて別名を付ける `通販` の版は `<名前>(returned)` の形を言う。4.3）。

```
$ ritsu check .
error[rulec E032]: Enum order_status does not agree with OrderStatus in ../../proto/shop/ordering/v1/order.proto
  --> billing/rules/billing_need.rule:5
  |
5 | enum order_status = received | paid | shipped | cancelled_in_ordering(cancelled)
  |      ^^^^^^^^^^^^
  |
 In ../../proto/shop/ordering/v1/order.proto but not in this enum: returned
 Add `returned` to this enum.
 The contract (../../proto/shop/ordering/v1/order.proto) has gained a value. How this rule treats it has not been decided yet.

ok billing/rules/payment_fee.rule
ok billing/rules/shipment_fee.rule
ok delivery/rules/urgency.rule
…
delivery/arrange_delivery.flow: ok
ordering/fulfillment.flow: ok
error[sakai E105]: billing/rules/billing_need.rule:5:1: The file billing/rules/billing_need.rule does not pass rulec's check, or cannot be read
     5 | enum order_status = received | paid | shipped | cancelled_in_ordering(cancelled)
  = What rulec says: [E032] billing/rules/billing_need.rule:5: Enum order_status does not agree with OrderStatus in ../../proto/shop/ordering/v1/order.proto
  = Make the file pass rulec's check; the references of a file that cannot be read cannot be checked.
ritsu check: 21 files (rulec 4, koyomi 3, chobo 1, proto 5, dandori 2, sakai 6): 1 fail (2 errors); borders between the languages: 0 checked, 0 undecided
```

要約は、言語ごとのファイルの数、結果、言語の境目の検査の数を言う。結果は、どれも通れば `all pass`（日本語は「どれも検査を通りました」）、通らないファイルがあればその数、確かめられなかったファイルがあればその数で、エラーと警告の数を括弧に添える。ファイルが通らないとは、そのファイルを場所とするエラーがあるか、ファイルを一つずつ確かめる言語（rulec、koyomi、chobo、geas、dandori）がそのファイルを通さなかった（geas の成り立たない主張など、コードの無いものも含む）ことである。境目の数は、言語をまたぐ検査（7 章）が確かめた境目と、そのうち決められなかったものである。E.4 で、X2（前提を呼び出しの場所で）と X3 の (b)（rulec の `range from koyomi`）が数に入り、E.5 で、`ritsu check` が dandori に日付の口と帳簿の口も渡すようになって（6.1。前は規則の口だけで、`use dates` と `use book` のあるフローは `ritsu check` でも E018 になっていた）、X3 の (a)、X4、X5、X6 が入った。テストのプロジェクト（`shop` と `通販`）は、フローが呼ぶ規則に前提も koyomi の範囲も無く、日付も帳簿も使わないので、どちらも 0 のままである（golden は変わらない）。2026-10-06 に、X14（16.8）も数に入った。フローが地図のコンテキストに属し、秘密の値をプロジェクトの中のファイルへ送る呼び出しごとに、境目を一つ数える（地図の無いプロジェクトでは数えない）。テストのプロジェクト四つ（`shop`、`通販`、`invoice`、`stockroom`）には秘密の値を送るフローが無いので、数は変わらない（替える前と後で確かめた）。dandori の例の `invoice` を `ritsu check` にかけると、W205 が二つ（`now` から来る日）と W206 が六つ（仮押さえを作るタスクに `timeout` が無い）出て、要約は `ritsu check: 8 files (koyomi 4, chobo 2, dandori 2): all pass (8 warnings); borders between the languages: 8 checked, 8 undecided` になる。これが正しい振る舞いで、`ritsu run` の担当のテスト（`crates/ritsu/tests/dandori.rs`）の期待は、取り込むときにこの 8 件に合わせた。日本語の要約は「ritsu check: ファイル 21 個（rulec 4、…）。検査を通らないもの 1 個（エラー 2 件）。言語の境目: 確かめた 0 か所、決められない 0 か所」の形になる。

JSON は一つのオブジェクトで、キーは `ritsu`（バージョン）、`root`（走らせたディレクトリから見たルート）、`ok`（exit 0 になるか）、`files`（ファイルごとの `tool`、ルートからの `file`、`ok`）、`diagnostics`、`borders`（境目の検査の `held`・`failed`・`undecided`）の順である。各言語の診断は、その言語の `check --format json` が書く形のまま入れ、その先頭に `tool` を置き、`file` をルートからの相対にする（言語の形に `file` が無ければ `tool` のすぐあとに足す）。rulec の診断は rulec の形（`v` が 2 の形）のままで、`title` や `column` は rulec の名前である。上の例では次のようになる（`crates/ritsu/tests/golden/check/shop-returned.json`。`…` は省いたところ）。境目の検査が数えたときの `borders` の三つの数は、`crates/ritsu/tests/cross.rs` がプロジェクトごとに確かめる（テキストは `tests/golden/cross/` の golden）。

```json
{
  "ritsu": "0.1.0",
  "root": ".",
  "ok": false,
  "files": [
    {
      "tool": "rulec",
      "file": "billing/rules/billing_need.rule",
      "ok": false
    },
    …
  ],
  "diagnostics": [
    {
      "tool": "rulec",
      "v": 2,
      "severity": "error",
      "code": "E032",
      "file": "billing/rules/billing_need.rule",
      "line": 5,
      "column": 6,
      "title": "Enum order_status does not agree with OrderStatus in ../../proto/shop/ordering/v1/order.proto",
      …
    },
    {
      "tool": "sakai",
      "code": "E105",
      "severity": "error",
      "file": "billing/rules/billing_need.rule",
      "line": 5,
      "col": 1,
      "message": "The file billing/rules/billing_need.rule does not pass rulec's check, or cannot be read",
      …
    }
  ],
  "borders": {
    "held": 0,
    "failed": 0,
    "undecided": 0
  }
}
```

A の段階の案で `crossings` と呼んでいたものは `borders` にした。sakai の要約と api の `crossings` は、境界づけられたコンテキストの境界を越える参照のことで、別のものだからである。案の `proved` も、0.4 の決まり（肯定形で「証明」と言わない）に合わせて `held` にした。

テストは `crates/ritsu/tests/check.rs` で、テストのプロジェクトとそれを変えたコピーのテキスト（英語と日本語）と JSON を golden にし、テキストからツールの語を除いたものが、同じファイルに各言語のコマンド（`ritsu <言語> check`）が出すものを順に並べたものと一字も違わないことを、テストのプロジェクト、変えたコピー、yuen のテストの材料（規則を名指す要件）、geas の例（主張が Python のプログラムを走らせる）で確かめる。

`crates/ritsu/tests/projects/` は、日本語のプロジェクト（`通販/`）と英語のプロジェクト（`shop/`）を持ち、golden（`tests/golden/check/`）は `通販.*` と `shop.*` を持つ（取り込んだときの `shop.*` は `通販` の golden だったので、中身を変えずに `通販.*` に名前を替え、`shop.*` を英語のプロジェクトのものにした）。どちらも `ritsu check` を英語と日本語と JSON で走らせたもので、境界越えと診断の数は同じ。ほかに、`ritsu run` の `invoice/`（7.9）と、`ritsu gen` の `stockroom/`（9.3）がある。

### 8.4 終了コード

0（問題なし、または警告と備考だけ）、1（どれかの言語か、言語をまたぐ検査にエラーがある）、2（使い方の誤り、読めないファイル、中の異常）。どの言語のコマンドとも同じ。

E.2 で、言語の結果をこう読むことにした。言語が単位を確かめられなかったとき（読めないファイル、geas がジャーナルを書けないとき）は 2 で、ほかの単位の検査は続ける（6.1）。言語がエラーを見つけたとき、または単位を通さなかったときは 1 である。geas は、読めない主張のファイル（構文の誤り）にも 2 で終わるが、`ritsu check` では言語がファイルの誤りを見つけたこととして 1 にする（ritsu の 2 は、走らせ方とファイルを読めることの問題だけに使う）。`ritsu <言語>` の終了コードは、その言語のコマンドのものである。

### 8.5 外のツールのための JSON

`rulec schema|certificate|api|graph|vectors`、`koyomi api|vectors`、`chobo api`、geas の `--json` と `map` の記録、`yuen api`、`sakai api` は、これまでどおり出す（P6）。ritsu の中ではこれらを読まない（口を通す）が、外のツールとエージェントが読む。プロジェクト全体の中のものと参照を一つの JSON で出すコマンド（`ritsu index`）は、外のツールが要るとわかってから足す（15 章）。

### 8.6 段階 D の最小の入口（`ritsu dandori`、`ritsu yuen`、`ritsu sakai`）

D.3 で、dandori のクレートは rulec を読まなくなり、そのバイナリは規則を読めなくなった（2.3）。`ritsu` の入口は E で作る予定だったので、そのままでは、D の残りと E のあいだ、規則を使うワークフローを手で走らせる手段が無い。困るのは次のところである。

- 例を走らせる手順。dandori の README とサイトの入れ方のページは、`dandori check examples/hotel/temporal/hotel.flow` を走らせ、例の多くは規則を使う。
- `dandori doc` で、規則のページを埋め込んだ図を描くこと。
- コマンドの出力を、替える前と後のバイナリで突き合わせること（D の各部分の確かめ）。

ブラウザで試すページは困らない。wasm のモジュールは初めから rulec を持たず、規則の答えを記録して持つからである（D.3 で、記録を口の答えにした）。

**決定（★）**：入口の最小の形を D で先に作る。`crates/ritsu` のバイナリ `ritsu` が持つのは、`ritsu dandori <引数>…`（dandori のコマンドを、rulec の規則の口をつないで走らせる）と、`--help`、`--version` だけである。ほかの言語の名前（`ritsu rulec` など）には、まだ無いと言って 2 で終わる。それらの言語は、これまでどおり自分のクレートのバイナリで動く（rulec・koyomi・chobo・geas はほかの言語を読まない。yuen と sakai がほかの言語を子プロセスで呼ぶところは、D.7 と D.8 で口に替える）。`ritsu check`、`ritsu run`、`ritsu gen`、`ritsu explain`、すべての言語の `ritsu <言語>`、リンクの名前で呼ばれたときの振る舞い（2.3）は、E で作る。

D の最後の部分で、`ritsu yuen <引数>…` を足した（PLAN の D.7）。yuen のコマンドを、yuen が読むすべての言語の口（rulec、koyomi、chobo、geas、dandori、sakai の `Items`、rulec と koyomi の `Sources`、rulec の `Rules`、koyomi の `Dates`、geas の `Claims`）をつないで走らせる。テストは `crates/ritsu/tests/yuen.rs`（ほかの言語のものを名指す yuen のテストの材料の全部が通ること、`affected`、借りた出典の `source outdated`）。

同じ部分で、`ritsu sakai <引数>…` も足した（PLAN の D.8）。sakai のコマンドを、sakai が読むすべての言語の口（rulec の `Rules` と `References`、koyomi と dandori の `References`、chobo の `Books`、doc のための koyomi の `Dates`（F.2 で足した））をつないで走らせる。テストは `crates/ritsu/tests/sakai.rs`（例の `check` と `api` と四つの `build --check`、ほかの言語を通してしか見えない変更の E209 と E105）。これで入口が持つ言語は、ほかの言語を読む dandori、yuen、sakai の三つになった。残りの四つ（rulec、koyomi、chobo、geas）はほかの言語を読まないので、自分のクレートのバイナリで同じに動く。

E.2 で、入口を 8.1 の形にした（`ritsu check`、七つの全部の `ritsu <言語>`、リンクの名前）。下の段落と捨てたものは、D でこの最小の形を先に作った理由として残す。

待つ費用が大きく、作る費用が小さいからである。待てば、規則を使う例をコマンドで走らせる手段が E まで無く、README に書ける手順も無い。作るのは、dandori のコマンドを関数（`dandori::cli::run`）にしたので、引数を渡すだけで済む（`src/main.rs` は 90 行）。入口は何に依存してもよく（3.1）、`cargo xtask deps` も通る。テストは `crates/ritsu/tests/dandori.rs` に置いた（すべてをつないだバイナリを走らせるテストの置き場所。3.3）。規則を使うフローを `ritsu dandori check` と `doc` が読むこと（英語と日本語）と、`ritsu` が持たないコマンドに 2 で終わることを見る。

捨てたもの：

- **E まで待つこと**：上の理由。
- **dandori のクレートのバイナリに rulec を入れること**：依存の決まりの 1（言語のクレートはほかの言語のクレートに依存しない）を破る。
- **テストのための小さなバイナリを別に作ること**：README の手順は、使う人が走らせられるものでなければならない。

### 8.7 ブラウザで試すページ（F.5 で作った）

`crates/ritsu-wasm` を wasm32-unknown-unknown にビルドしたモジュールが、ページの中で ritsu を動かす。ページは小さなプロジェクト（数個のファイル）をタブで持ち、ファイルが変わるたびに、全部のファイルと言語と開いているファイルを JSON でモジュールに渡す。モジュールはファイルを `ritsu_base::fs::Memory` の `/playground` に置き、そこでコマンドを走らせる（4.15）。境目は rulec と dandori と同じ決まり（バッファの頭に長さを書く）の六つの関数（`ritsu_alloc`、`ritsu_free`、`ritsu_version`、`ritsu_check`、`ritsu_gen`、`ritsu_doc`）で、モジュールはインポートを持たない。

- `ritsu_check`：`ritsu check .`。`ritsu` のクレートのライブラリの `ritsu::check::run`（バイナリと同じ関数）を呼ぶので、言語をまたぐ検査が増えても、ページは何も直さずに出す。答えは標準出力・標準エラー・終了コードと、`--format json` の JSON（ページはこれでファイルのタブに印を付ける）。
- `ritsu_gen`：開いているファイルの言語の生成。rulec は `gen`、koyomi は `gen`、chobo は `build --target`、dandori は `build --target`、sakai は `export cml`、yuen は `export reqif|prov`（yuen と sakai には `--root .`）。生成したファイルは `generated/` に書いた順に返す（`Memory::written`）。geas と `.proto` は生成しない。
- `ritsu_doc`：rulec・koyomi・chobo・dandori の `doc` の HTML と Markdown。
- 書き出し先を引数に取るコマンド（ritsu の check、rulec の gen、dandori、yuen、sakai）はそのまま呼ぶ。標準出力に直接書くコマンド（rulec の doc、koyomi の gen と doc、chobo の build と doc）は、コマンドが呼ぶ関数を同じ順に呼び、同じ文を返す。四つの言語のコマンドを書き出し先を引数に取る形に直すことは捨て、テストがページの答えを全部 `ritsu` のバイナリの出力と突き合わせる。

ページは根の `website/` にある（英語の `website/docs/playground.md` と日本語の `website/docs-ja/playground.md`、`website/docs/playground/` の `playground.js`・`playground.css`・`projects.json`・`ritsu.wasm`、wasm を作る `website/tools/make_wasm.sh`）。ページが開くプロジェクトは、ここで書き下ろした小さな通販の英語版（`website/playground/shop/`）と、二つ目に置く日本語版（`shop.ja/`）で、rulec の規則三つ、koyomi の二つ、chobo の帳簿、`.proto` 一つ、dandori のフロー一つ、yuen の要件一つと見た記録のコピー、sakai の地図を持つ。開いたときは、受注が `.proto` に `ORDER_STATUS_RETURNED = 5;` を足したところで、rulec（E032）、yuen（E203）、sakai（E105）の三つが答える状態にした。`make_wasm.sh` はビルドの置き場所を `cargo metadata` に尋ねるので、ワークスペースの `target/` でも `CARGO_TARGET_DIR` でも動く。ritsu.wasm は 9.1 MB（gzip で 2.7 MB）で、七つの言語の検査と生成器が全部入る。

ページの終わりに、`ritsu.wasm` が含む他者のもの（`ritsu` のバイナリと同じ）と、根の `THIRD_PARTY_NOTICES` へのリンクを書いた。ページが読み込む `projects.json` の dandori の例が含む API の記述のコピーについても、dandori の通知にリンクした（13.2。2026-10-06）。

ページにないもの：geas（主張はコードを走らせて確かめ、ページはプロセスを起こせない。`.geas` は、プログラムを起こすところで E030 になる）、実行（`dandori run`、`chobo run`、シナリオ、`rulec verify`・`replay`）、ネットワーク（`source fetch`・`outdated`）、地図が名指すコードの読み込みのうち Cargo に尋ねるもの（`code rust`）、プロジェクトへの書き込み（`fmt`、`yuen review`）。

テスト（`crates/ritsu/tests/playground.rs`）：ページの全部の答えを、同じファイルを置いたディレクトリで走らせた `ritsu` のバイナリの出力と書いたファイルに突き合わせ（1,744 の答え。前の二つのページの例と、それが確かめられていたことを足した。下の段落）、コミットした ritsu.wasm を node で呼んでライブラリと突き合わせ（410 の問い）、Chrome でページを開く。ritsu.wasm と projects.json はコミットする作られたもので、古くなればこのテストが落ちる（`website/tools/make_wasm.sh`、`RITSU_BLESS=1`）。`ritsu check` の出力を変えたら、ritsu.wasm を作り直す。

**前の二つのページの例を全部並べた（作者の指示、2026-10-04）。** このページは、前の rulec と dandori のブラウザで試すページを置き換えるもの（置き換えは、ritsu を public にして切り替えるとき。13.2）なので、二つのページが開いた例と、二つのページが読んだリンクの形を全部持つ。

- プロジェクトの選択は `<optgroup>` で四つに分ける。ritsu（通販の英語版と日本語版。いままでどおり先頭）、ファイル一つから始める（空のプロジェクト）、rulec（規則一つ）、dandori（フローと、フローが読むファイル）。英語のページは英語の版を、日本語のページは日本語の版を並べる（前のページと同じ）。dandori の最初の下書きには日本語の版が無いので、両方のページに出る。選択の名前は、前のページの言い方を使う（rulec はボタンの五つ、dandori は「例・版」）。
- rulec の例：前のページのボタンの五つ（一行足りない表、そろった表、表をつなぐ、大きい規則、並びを歩く）の英日で十のプロジェクト。どれもファイル一つで、ファイルの名前はコーパスの名付け方（`rule` の行の名前に `.rule`。`Fee.rule`、`運賃.rule`、`parcel_rate.rule` など）。中身は、rulec の `tests/website.rs` が前のページのコピーを突き合わせているのと同じもとから作る。トップページの表（`website/rulec/tools/overview.rule` と `overview-ja.rule`）から絵についての注を除いたものと、その最後の行を抜いたもの、コーパスの三つ。
- dandori の例：前のページが並べたフロー（最初の下書きと、六つの例の版と、版の横のフロー）の英日で 38 本、プロジェクトは 37（下書きは一つ）。プロジェクトは、フローと、フローが読むファイル（規則、API の記述、子のフロー、日付のファイルとそれが使うカレンダー、帳簿）からなる。★パスは dandori のクレートの中のままにしたので、指摘の場所は前のページと同じ `tests/fixtures/hotel_naive.flow:95:1` になる。読むファイルは手で挙げない。テストが dandori の例を全部メモリに置き、`ritsu check` と `dandori doc` がフローを読むときに読んだファイルを記録し、読んだファイルにも `ritsu check` をかけて、新しく読むものが無くなるまで繰り返す。生成の対象は、前のページと同じく版のディレクトリで決める（AWS 版は Step Functions。Step Functions が受け付けない版は Lambda durable functions）。
- projects.json の形を `{"projects": [{"name", "group", "lang"?, "open", "target"?, "files": [[パス, もと]]}], "texts": {もと: 中身}}` にした。もとはリポジトリの中のパスで、中身はもとごとに一度だけ書く（Stripe の API の記述は七つのプロジェクトが持つ）。大きさは 30 KB から 491 KB になった（前の dandori のページの presets.json は 1.1 MB）。
- 空のプロジェクト：ファイルが一つもないあいだは、「ファイルを足す」のボタンに色を付け、拡張子で言語が決まることと、読むファイルはもう一つのファイルとして足すことを出力のところに書く（その下に、`ritsu check .` が空のディレクトリに言うことも出す）。
- 共有のリンク：「リンクをコピー」を押すと、いまのプロジェクトのリンクをアドレスバーに入れる（ブラウザが許せばクリップボードにも）。形は `#project=<名前>&file=<パス>&view=<check|gen|doc>&target=<対象>&edits=<変えたもの>` で、`file`・`view`・`target` は開いたときと違うときだけ、`edits` は変えたものがあるときだけ書く。★`edits` は、開いたときのプロジェクトから変えたファイルと足したファイル（中身）と消したファイル（null）の組の JSON を、ブラウザの CompressionStream で raw DEFLATE に縮め、base64url（埋めの `=` なし）にしたもの（リンクを短くするため、開いたときとの差分にした。例のもとが後で変わると、古いリンクは新しい例に差分を当てる）。ページはどこにも保存しない。★名前は、通販が `shop` と `shop.ja`、空のプロジェクトが `empty`、rulec の例が `rulec/<ボタンの名前>`（日本語は後ろに `.ja`）、dandori の例が `dandori/<フローのパス>`。
- 前のページのリンク：dandori のページの `#flow=<パス>&view=<check|build|doc|rules>&target=<対象>` を読む。`build` は生成、`rules` はプロジェクトの最初の規則のファイルの人が読むページ（前のページの「規則」のタブは、規則の中身と `rulec doc` のページを見せていた。ここでは規則がプロジェクトのファイルで、書き換えもできる）。リンクが別の言語のページのプロジェクトを名指しても開く（選択にそのプロジェクトを足す）。rulec の前のページは URL を読んでいなかった（開くといつも一行足りない表で、共有の形は無かった）。 前の二つのページは、いまはここへ送るだけのページで、来たリンクを付けて送る（この節の最後の段落）。
- 例を足したことで、ページの dandori の生成と人が読むページが、日付のファイルと帳簿を使うフロー（請求の例）で E018 になっていたのが見つかった。ページは `dandori::cli::run` に規則の口だけを渡していて、`ritsu dandori` が渡す日付と帳簿の口と、X2 で決められない前提（7.4）を渡していなかった（通販のフローは日付も帳簿も使わないので、テストに出なかった）。`ritsu` のライブラリに `languages::dandori`（`ritsu dandori` の本体）を置き、バイナリ（`src/main.rs`）とページが同じ関数を呼ぶようにして、ritsu.wasm を作り直した。

テスト（`crates/ritsu/tests/playground.rs`、六つ）は次を確かめる。projects.json が、もとのファイル（`website/playground/`、rulec のコーパスとトップページの表、dandori の例）の今の中身であること。前の二つのページが開いた例を、このページが全部持つこと（48 の例）。rulec の例は、ボタンの五つ（テストの表 `RULEC`）が英日の順に並び、どれもファイル一つで、トップページの表（`website/rulec/tools/overview.rule` と `overview-ja.rule`）の絵についての注の下と、その最後の行を抜いたもの、コーパスの三つの規則と一字も違わないこと。dandori の例は、前のページが並べたフロー 19 本（テストの表 `DANDORI_PAGE`。日本語のページは例を `<名前>.ja.flow` にしたもの）が、同じ順に、前のページと同じ生成の対象で並ぶこと（前のページが出したリンクはこのパスを名指すので、どれも残す。例が増えれば、その間に並んでよい）と、dandori の例のフローがどれもプロジェクトであること。ページの 1,744 の答えが、同じファイルを置いたディレクトリで走らせた `ritsu` のバイナリと一字も違わないこと（92 のプロジェクトと編集。編集は英日あわせて 38 で、通販の 14、前のページの手順の 8、空のプロジェクトに足したファイル一つの 2 に、前の二つのページが確かめられていたことの 14 を足した。rulec の `tests/wasm.rs` がわざと選んでいた規則の三つ（消去で答えが決まる `coupon_stacking`・`クーポン併用`、決まらない `m_w114`、℃ より後ろに出る指摘）を英日で 6、dandori の `tests/playground.rs` の、プロジェクトに無い規則と子のフロー、自分を子として走らせるフロー、下書きの横に足した自分のフローを英日で 8）。コミットした ritsu.wasm が 410 の問いにライブラリと同じ答えを返すこと（ファイル一つの編集は、check に加えて生成と人が読むページも問う）。Chrome で英語と日本語のページを開くこと（22 の画面）。前の二つのページが、来たリンクをこのページへ送ること（Chrome で 7 本。下の段落）。

**前の二つのページを、ここへの転送に替えた（作者の決め、2026-10-05）。** ritsu を public にしてサイトを切り替えたので、前の rulec と dandori のブラウザで試すページ（`website/rulec/docs/playground.md`・`website/dandori/docs/playground.md` と日本語のページ。公開先は `/ritsu/rulec/playground/` など）を、このページへ送るだけのページにした。

- 英語のページは `/ritsu/playground/` へ、日本語のページは `/ritsu/ja/playground/` へ送る。来たリンクのハッシュが `=` を含む（このページが読む形。dandori のページが出していた `#flow=…&view=…&target=…`）なら、そのまま付けて送る。そうでなければ（ハッシュが無いか、前のページの見出しのアンカー）、前のページが開いていたものを開く。dandori のページは下書き（`#flow=tests/fixtures/hotel_naive.flow`）、★rulec のページは一行足りない表（`#project=rulec/gap`、日本語は `#project=rulec/gap.ja`。前の rulec のページは URL を読まず、共有の形を持たなかった。履歴のどの版にも無い）。
- ページは、移ったことを言う一文と、このページへのリンク（`id="moved"`）と、リンクの先へ `location.replace` で移るスクリプトだけを持つ。スクリプトはリンクの `href` を読むので、行き先を書くのはリンクの一か所である。スクリプトが動かなくても、リンクから行ける。リンクは、Markdown のリンクと同じくページのファイルから読む相対のパスで書く（`../playground/#…`、日本語は `../../ja/playground/#…`）。Zensical は、Markdown の中の HTML の `href` も同じように読み、ページを一つ深いディレクトリ（`playground/`）に出すので、出したページでは一つ上がる形（`../../playground/#…`）に書き直す。はじめは出したページから読む形で書いていて、Zensical がもう一つ `../` を足し、`/ritsu/` の上の `/playground/` を指していた。テストは `/ritsu/` を付けずにリンクをたどっていたので（ブラウザも、根より上へは上がらない）、それを見逃した。いまは、どのテストも `/ritsu/` の下でたどる（下の段落）。
- ★言語のサイトのナビの「ブラウザで試す」とトップページのボタン（dandori は入れ方と確かめ方のページのリンクも）は、転送のページを通さず、このページを直接指す（同じ `#project=rulec/gap`・`#flow=…` 付き）。ナビのリンクは Zensical がサイトのルートからたどってページごとに書き直し（`../playground/#…`、日本語は `../../ja/playground/#…`）、ページの中の Markdown のリンクはページのファイルからたどる。転送のページはナビから外した。
- 前のページだけが使っていたものを消した。rulec の `src/wasm.rs`、dandori の `src/wasm.rs`・`src/playground.rs`・`src/record.rs` と `sources` の `Playground`・`Recorder`・`Bundle` と記録から答える三つの口、`website/rulec/docs/playground/`（`playground.js`・`playground.css`・`rulec.wasm`）、`website/dandori/docs/playground/`（`dandori.wasm`・`presets.json`・`playground.js`・`playground.css`）、二つの `tools/make_wasm.sh`、二つの `sync.sh` が日本語の木へコピーしていたところと、その gitignore の行、テスト（rulec の `tests/wasm.rs` と `tests/website.rs` の二つ、dandori の `tests/playground.rs`）。ritsu.wasm は、使わない `rulec_*` と `dandori_*` の関数を出さなくなり（出す関数は 23 から 9）、作り直して 9,929,706 バイトから 9,813,955 バイト（gzip で 2.9 MB）になった。
- rulec の E129（koyomi をつないでいない rulec で `range from koyomi` を確かめた）の注と台帳は、koyomi が無いところとして「ブラウザで試すページ」も挙げていた。このページは koyomi をつなぐので、rulec のクレートのバイナリだけにした（rulec の §15.185）。
- 確かめていたことのうち、いまも意味のあるものは、前のページのファイルではなく、もとのファイルとこのページに対して確かめる形で、上のテストに移した。`tests/website.rs` は、転送のページのリンクが、サイトのルート（`/ritsu/<言語>/`。ページのファイルはそこにある）から読んで、このページ（言語も同じ）に着き、その `#…` がこのページの並べるプロジェクトを開くこと、スクリプトがリンクをたどり、`=` を含むハッシュを持っていくこと、サイトのナビとトップページが同じところを指し、ナビに転送のページが無いことを確かめる。Zensical があれば、組んだ木の転送のページには `index.html` のほかに何も無く、そのリンクとサイトのトップのナビが木のこのページに着くことも確かめる。`tests/playground.rs` の `the_pages_before_send_their_links_on` は、転送のページ（Markdown の HTML に、Zensical と同じくリンクに `../` を一つ足したもの）を、サイトと同じく `/ritsu/` の下の公開先のパスに置き、Chrome で開いて、送られた先と、そこで開くプロジェクト・ファイル・出力がライブラリの答えと同じことを確かめる（rulec の英語のページにハッシュ無しと見出しのアンカー、日本語のページにハッシュ無し、dandori の英語のページにハッシュ無しと `#flow=…&view=build&target=asl`、日本語のページにハッシュ無しと `#flow=…hotel.ja.flow&view=build` の 7 本）。headless Chrome の `--dump-dom` は、ページがほかへ移るとページを出さないまま止まるので、転送のページは枠（iframe）の中で開き、枠が着いた先と、そこに出たものを、枠の外のページに書き込む。dandori のページのスクリプトから `=` を持っていく行をわざと消すと、このテストが落ちることを見た。rulec のページのリンクを、`/ritsu/` の上へ出る形にわざと戻すと、この Chrome のテストと、`tests/website.rs` の二つ（ソースと組んだ木）が落ちることも見た。

## 9. 生成

### 9.1 生成器は言語ごとのまま

生成するコードの形は、言語ごとに決まっている。rulec は表の一行を一つの分岐にし、koyomi は操作ごとの小さな関数を生成物の中に置き、chobo は振替の種類ごとの関数とチェーンを書き、dandori はプラットフォームごとにワークフローを書く。共通の中間表現は作らない（14 章）。

### 9.2 書き出しの共通部分（`ritsu-emit`）

生成器が共通に使う、生成先の言語の表面にかかわる部分を一つにする。

- 生成先の言語ごとの予約語（いまは rulec の `src/backend.rs` の 12 言語、koyomi の `src/reserved.rs`、dandori と chobo の表）と、名前がぶつかったときの書き方。
- 公開する名前に ASCII の別名を使う決まり（rulec の §1.3）と、識別子の作り方。
- 文字列と数のリテラルの書き方、コメントの書き方。
- 生成物の頭：`Code generated by <名前> <ritsu のバージョン>. DO NOT EDIT.` と、元のファイルのパスと SHA-256。

対象は、TypeScript、JavaScript、Python、Go、Rust、Ruby、PHP、Swift、Java、SQL（PostgreSQL）、それと dandori の書く YAML と JSON（Argo と Step Functions）。

頭は、ritsu-emit の `header` の `generated(<言語>)` と `Source` で書く（E.7 で、rulec・koyomi・chobo・dandori の全部をこれにそろえた）。`ritsu gen` が `crates/ritsu/tests/projects/stockroom` から書いた TypeScript の頭：

```
// Code generated by rulec 0.1.0. DO NOT EDIT.
// Source: rules/delivery.rule (rule delivery v1, sha256:0b2eab7841247ba7)

// Code generated by koyomi 0.1.0. DO NOT EDIT.
// Source: dates/payment_terms.cal (dates payment_terms v1, sha256:20eacc1719046107)
// Calendar: calendars/weekdays.cal (calendar weekdays v1, sha256:dbf95c1f04262520)

// Code generated by chobo 0.1.0. DO NOT EDIT.
// Source: books/stock.book (book stock v1, sha256:1b4a24898f8c7357)

// Code generated by dandori 0.1.0. DO NOT EDIT.
// Source: orders/order.flow (workflow order v1, sha256:e96e824074cf7459)
```

- ファイルは、言語のコマンド（`rulec gen` など）ではファイルの名前で、`ritsu gen` ではプロジェクトのルートからのパスで書く（6.2 の名指しのパスと同じ）。どちらも、生成した機械のパスは書かない。
- ★ハッシュは、ritsu の固定と同じ 16 桁（`ritsu_base::sha256::short`）。rulec は 12 桁から延び、koyomi は前のまま。
- 二行目は `--lang ja` で `もと: <ファイル>（<種類の語> <名前> v<版>、sha256:<16 桁>）` になる（rulec と koyomi。chobo と dandori の生成物は英語だけ）。種類の語は、言語のファイルに書く語（`rule`・`dates`・`calendar`・`book`・`workflow`）である（rulec の日本語は前は `規則`）。三行目から下（rulec の `Applies:`・`Cites:`、koyomi の `Calendar:`・`Cites:`）は言語ごとのまま。
- 元のファイルを持たないもの（chobo の Go の `runtime.go`、rulec の丸めのテスト、パッケージのインデックスのファイルと依存を書くファイル）は一行目だけ。パッケージそのものの部分の一行目の言語は `ritsu`。

- **頭とコメントに書く文字列は、その中に収める（2026-10-06）。** 頭の `Source:` の行は、ファイルのパスをそのまま書いていた。Unix ではファイルの名前に改行を入れられ、名前の続きが生成したコードの行になった（`ritsu gen` で、`rules/` に `pickup` と改行と `print('ran') #.rule` の名前の規則を置くと、Python のモジュールの頭の次の行がその文になった）。dandori のワークフローの説明（`\n` を書ける）も、rulec の出典の URL とパス（U+2028 や `\r` を書ける）も、同じことが起きた。いまは、ritsu-emit の `header::one_line` が、行を終える五つの文字（`\n`、`\r`、U+0085、U+2028、U+2029）を `U+XXXX`（`U+000A`、`U+2028` など）にして一行に収め、`Source::line` と `Comment::line` がそれを通る。`U+XXXX` にしたのは、Java と Scala 2 がコメントの中でも `\uXXXX` を、ほかのどの処理より先に読むからである（JLS 3.3）。前の `\u{2028}` は javac を「不正な Unicode エスケープ」で止め、文字列がもともと持つ六文字の `\u000a` はコメントの中で改行になって外へ出た。`U+XXXX` は `\` も `\u` も含まないのでどの言語でも安全で、文字列がもともと持つ `\` は、Java を書く生成器（いまは rulec）が、頭を `header::for_unicode_comment` に通して二つにし、無害にする（`u` の前の `\` が偶数個のときだけエスケープが始まる決まりによる）。rulec の Java は、頭だけでなく、本文のコメント（表の名前、行のラベル、列挙の値、`starts_with` の文字列が入る、分岐の上と行の `//` と `/** … */`）も同じ関数に通し、記録の JSON のキーは、名前を JSON の文字列にしてから Java のリテラルにする。文字列リテラルは、前から `\` を `\\` にしていたので、`\u0022` でリテラルが閉じることは無かった。rulec の Java 以外の出力先と、koyomi・chobo・dandori の出力先は、コメントの中で `\uXXXX` を読まないので、二つにしない（Python、Ruby、PHP、JavaScript と TypeScript、Go、Swift で、コメントに六文字の `\u000a` と `\u{2028}` を入れて確かめた。Rust と SQL も、仕様上コメントで `\u` を読まない）。どこかの出力先がこの五つで行を終える（Python は `\r`、TypeScript と JavaScript は U+2028 と U+2029、YAML 1.1 は三つとも）。説明のように何行にもなってよい文は、`Comment::lines` が一行ずつコメントにする（dandori の DESIGN 4.7）。ページに埋め込むスクリプトは、ritsu-base の `docpage::script_text` が `</script` を `<\/script` にする（rulec の DESIGN §15.187。chobo と dandori は前から JSON の `</` を `<\/` にしていた）。確かめ方は、ritsu-emit の `tests/emit.rs`、dandori の `tests/comments.rs`、rulec の `tests/heads.rs`（`a_cited_address_does_not_break_the_java_head` と `rule_strings_stay_what_they_are_in_the_java` は、生成した Java を javac にかけ、後者は走らせて値を比べる）、ritsu の `tests/gen.rs` の `a_files_name_stays_in_the_head`。ふつうの入力の出力は変わらない。0.23.0 のリリースと、例とテストの材料を全部の生成器にかけた出力を比べ（dandori は 52 のフローを七つのプラットフォームで 2,149 ファイル、rulec はコーパスの 87 の規則を英語と日本語で 10,140 ファイル、koyomi は 21 の日付のファイルで 150 ファイル、chobo は 8 つの帳簿を七つの出力先で 72 ファイル、`ritsu gen` は三つのプロジェクトを帳簿の二つの置き場所で 410 ファイル）、違ったのは、9.3 の doc.go の二つだけだった。
- **sakai の書き出し（2026-10-06）。** 同じ形で確かめた。CML の持ち主のコメントと、`build` の設定の頭の地図のパスを一行に収め（ArchUnit の Java では、`\u000a` を javac が改行として読むので、バックスラッシュも二つにする）、`doc` の Markdown の `\r` を空白にした。HTML と `api` は、もとからエスケープしていた（sakai の DESIGN 16.7）。yuen と geas の書き出し、`dandori doc` と `rulec doc` の Markdown は、16 章の頭に書いた。


### 9.3 一つの生成パッケージ（E）

`ritsu gen` は、プロジェクトの規則、期日、帳簿のクライアント、ワークフローを、言語ごとに一つのパッケージにする。対象は、四つの言語が共に生成している TypeScript、Python、Go。E.7 で作った形は次のとおりである（A の段階の形は、Go と Python のディレクトリと、依存を書くファイルを持たなかった）。

```
generated/typescript/          generated/python/              generated/go/
  package.json                   pyproject.toml                 doc.go
  index.ts                       <name>/__init__.py, py.typed
  rules/<別名>.ts                <name>/rules/<別名>.py         rules/<パッケージ>/<別名>.go
  dates/<別名>.ts                <name>/dates/<別名>.py         dates/<パッケージ>/<パッケージ>.go
  books/<帳簿>.ts (.sql)         <name>/books/<帳簿>.py (.sql)  books/<パッケージ>/book.go, runtime.go
  flows/<名前>/…                 <name>/flows/<名前>/…          flows/<パッケージ>/…
```

`ritsu gen [<path>...] [--target typescript|python|go] [--out <dir>] [--check] [--books postgres|tigerbeetle] [--name <name>] [--module <path>]` は、プロジェクト（`ritsu check` と同じ `ritsu_project::Project::load` で読む）の規則、日付のファイルとカレンダー、帳簿のクライアント、ワークフローを、言語ごとに一つのパッケージにして `<out>/<言語>/`（既定は `generated/<言語>/`）に書く。`--target` が無ければ三つとも。言語ごとの生成器が自分の部分を書き（rulec の `codegen::package_module`、koyomi の `codegen::unit_shown` と各出力先の `module`、chobo の `target::build`、dandori のビルドと `InPackage`）、ritsu はインデックスのファイルと依存を書くファイルだけを書く（`crates/ritsu/src/package.rs`。`gen` は Rust 2024 の予約語なので、モジュールの名前は `package`）。

- ワークフローは、規則と期日と帳簿を、同じパッケージの `rules/`・`dates/`・`books/` から読む。import はパッケージのモジュールを名指し、帳簿のトランスポートが受け取るクライアントは、パッケージの `books/` のクライアントの型である（TypeScript の `Books`、Python の `TypedDict` の `Books`、Go の `Books` と `Map()`）。渡すクライアントが帳簿と違えば、その言語の型の検査が言う。dandori のモデルに `package`（`InPackage`）があるときだけ、Temporal の三つの SDK のビルドが読み込む先を替える。`package` が無い `dandori build` の生成物は前と同じである（dandori の DESIGN 4.2）。
- 依存は、入れたものが要るものだけを書く。TypeScript の `package.json` は、生成物が読み込むパッケージ（フローがあれば `@temporalio/*` 1.24.0、TigerBeetle の帳簿なら `tigerbeetle-node` 0.17.9）。Python の `pyproject.toml` は `temporalio==1.33.0` と `tigerbeetle==0.17.9`。PostgreSQL の帳簿のクライアントは、呼ぶ側が渡す接続を使うので依存を持たない（Go の pgx だけは import する）。バージョンは、ここで生成物を確かめているもの（dandori と chobo のランナー）。★Go のパッケージは `go.mod` を書かず、利用者のモジュールのディレクトリとして置く（`--module` が import のパス）。`go mod tidy` が書き換える `go.mod` を生成すると、`--check` が古いと言うからである。生成物が import するモジュールとバージョンは `doc.go` に書く。
- **書くバージョンは、ツールのロックファイルのバージョンと同じにし、その既知の脆弱性を監査で見る（2026-10-06）。** パッケージが書くバージョン（`package.json` の `@temporalio/*` 1.24.0 と `tigerbeetle-node` 0.17.9、`pyproject.toml` の `temporalio==1.33.0` と `tigerbeetle==0.17.9`、`doc.go` の Go のモジュール）と、生成したコードがコメントで名指すバージョン（chobo のクライアントの「through tigerbeetle-node 0.17.9」、dandori の Go の「written against go.temporal.io/sdk v1.49.0」など）は、dandori の `tools/temporal`・`tools/temporal-python`・`tools/temporal-go` と、chobo の `tools/runner` と `tools/runner/go` のロックファイルのバージョンと同じである。`crates/ritsu/tests/audit.rs` が、stockroom のパッケージを帳簿の二つの置き場所で生成して、それを確かめる。だから、監査（3.6）がツールのロックファイルを調べることは、パッケージが求めるバージョンと、ここでそれを確かめたときの依存の依存を調べることになる。リリースの前にも調べる（13.2）。
- **Go の依存の依存は、`doc.go` に上げるよう書く（2026-10-06）。** Go のモジュールは、求められたうちで最小のバージョンを選ぶ（minimal version selection）。npm や PyPI と違い、利用者が `go mod tidy` をしても、依存の依存は新しくならない。pgx v5.11.0（いま一番新しい）は golang.org/x/text v0.29.0 を求め、そのバージョンには GO-2026-5970 があり、pgx の SCRAM の認証から届く。PostgreSQL の帳簿の Go のクライアントは pgx を import するので、そのままだと利用者のモジュールは v0.29.0 でビルドされる（ほかの依存が上げなければ）。`ritsu gen` は、pgx を import するパッケージの `doc.go` に、次の行を足す（表は `src/package.rs` の `GO_RAISED`。pgx が直ったバージョンを求めるようになったら外す）。

  ```go
  //
  // Of the modules those require, these are tested at a later version than the one asked for, which
  // has a known vulnerability; raise them in the go.mod (`go get <module>@<version>`):
  //
  //	golang.org/x/text v0.41.0 (GO-2026-5970, through github.com/jackc/pgx/v5)
  ```

- **パッケージに SBOM は書かない（2026-10-06）。** 確かめたこと：osv-scanner 2.6.0 は、生成したパッケージの `package.json`（ロックファイルが無い）、`pyproject.toml`、`doc.go` のどれも読まない（三つとも「No package sources found」）。同じ依存を並べた CycloneDX 1.6 の `bom.cdx.json` を手で書いて置くと読み、x/text v0.29.0 の GO-2026-5970 などを見つけた。それでも書かない理由は三つある。一つ目に、ritsu が書ける SBOM は、パッケージが直接求めるものだけか、ここで確かめたときの依存の依存で、利用者の npm や uv が解決するものではない。利用者のロックファイル（`npm install` の `package-lock.json`、`uv lock`、`go mod tidy` の `go.mod` と `go.sum`）が、スキャナーが読むべきものである。二つ目に、直接の依存は、GitHub の依存関係グラフ（Dependabot）が `package.json` と `pyproject.toml` からそのまま読む。三つ目に、ritsu の書くバージョンが利用者の依存の依存を決めるのは Go だけで、それは上の `doc.go` の行が言う。作るなら、`ritsu gen --sbom` が、直接の依存と、生成したファイル（頭のハッシュ）を部品にした CycloneDX 1.7 を書く形がよい（15 章）。
- **パッケージは、確かめたバージョンを「ちょうど」で書く**（`"1.24.0"`、`temporalio==1.33.0`。2026-10-06 に決めた）。確かめていないバージョンを入れないためである。`@temporalio/*` そのものに脆弱性が出たときは、生成した `package.json` か `pyproject.toml` を書き換える（`ritsu gen --check` は手で直したと言う）か、ritsu の次のリリースを待つ。監査（3.6）はツールのロックファイルでそのバージョンを毎日調べるので、アドバイザリが出れば ritsu の側で気づき、直ったバージョンを確かめてからリリースする。下限つきの範囲（`^1.24.0`、`>=1.33.0,<2`）は捨てた。利用者のロックファイルだけで上げられる代わりに、確かめていないバージョンが入るからである。
- ★帳簿は `--books` で PostgreSQL（既定）か TigerBeetle を選ぶ。PostgreSQL なら、クライアントが呼ぶ SQL（スキーマと関数）も `books/<帳簿>.sql` に書く。クライアントだけでは動かないからである。
- ★パッケージの名前（`--name`、既定は `generated`）は、npm のパッケージの名前、Python のパッケージのディレクトリ、Go の import のパスの既定になる。ディレクトリの名前から決めると、CI で別の名前のディレクトリに取り出したときに `--check` が古いと言うからである。
- 何も書かずにエラーで止まるもの：検査を通らないファイル（その言語の診断を `ritsu check` と同じ形で出し、exit 1）、パッケージの同じファイルを書く二つのファイル（同じ別名の二つの規則、日本語の名前の二つのフローの Go の `workflow`。どちらの名前を替えるかは書いた人が決めることなので、exit 2）、インデックスのファイルや Go のディレクトリが import できない名前（Python と Go が予約している語。exit 2）、プロジェクトに無いファイルを読むワークフロー（exit 1）。
- ★生成物のコメントは `--lang` の言語で書く（`rulec gen` と同じ）。`--check` は同じ言語で走らせる（違えば古いと言う）。
- `--check` は書かずに、無いファイル、古いか手で直したファイル、前の `gen` が書いて今は書かないファイル（頭が ritsu の生成器のもの）を挙げて 1 で終わる。書くときは、前に書いて今は書かないファイルを消す（rulec の `gen --check` と同じ考え）。
- パッケージの Python が `mypy --strict` を通るように、dandori の Temporal の Python（と同じ部品を使う pydantic-graph と Step Functions の Lambda の Python）と、chobo の Python のクライアントの型の書き方を直した（★単独の出力も変わった。振る舞いは変えていない。dandori の DESIGN 0.3、chobo の DESIGN 8.1）。

確かめ方：`crates/ritsu/tests/gen.rs`。テストのプロジェクト `crates/ritsu/tests/projects/stockroom`（英語。rulec の規則 `delivery`、koyomi の日付のファイルとカレンダー、chobo の帳簿、三つを使う dandori のフロー）のパッケージが、帳簿の二つの出力先で、`tsc --strict`、`mypy --strict`、`go vet` と gofmt を通ること（通販は TypeScript だけ。dandori の Python の残りは dandori の DESIGN 7 章）。フローの import がパッケージのモジュールを名指し、規則と日付のアクティビティを走らせるとパッケージのモジュールを通って答えること（Python と Go）。`--check` が、無いもの・古いもの・残ったものを言うこと。どのファイルの頭も 9.2 の形で、元のファイルとそのハッシュを名指すこと（パッケージの形の golden `crates/ritsu/tests/golden/gen/stockroom.txt`。バージョンは `<version>` に置き換えてある）。中身ごとの突き合わせは、いままでどおり各言語のテスト（rulec の 12 言語のベクタ、koyomi の五つの出力先、chobo の七つの組み合わせ、dandori のプラットフォーム）がする。

### 9.4 生成物のバージョン

生成物の頭のバージョンは ritsu のバージョンになる。バージョンが上がれば生成し直してコミットすることになる（rulec の docs/compatibility.md が言うとおり）。

★ワークスペースのバージョンをそろえる（13.1）までは、頭のバージョンは ritsu-emit のクレートの 0.1.0（`CARGO_PKG_VERSION`）で、rulec の生成物の頭も rulec のクレートの 0.22.1 ではなく 0.1.0 を言う。F.7 で 0.23.0 にそろえたので、いまの頭はどの言語も 0.23.0 を言う（9.2 の例と 8.3 の JSON の例は、そろえる前に取ったものである）。

### 9.5 段階 C で作った形（`ritsu-emit`）

C.10 で `ritsu-emit` を作り、koyomi と chobo をこれに替えた。生成物は一バイトも変わっていない（PLAN の C.10）。

- 予約語（`words`）は、標準が並べるものを標準ごとに一つの表にした（ECMAScript 2025 の予約語と strict mode の予約語、Python 3.14.6 の `keyword.kwlist` と `softkwlist`、Go 1.25 のキーワードと事前宣言の識別子、Rust 1.94 のキーワード、PostgreSQL 18.0 の `kwlist.h` と PL/pgSQL の予約語）。生成器が照らし合わせるのは、いくつかの表をまとめた `Words` である。名前をエラーにする（koyomi の E009）か、`_` を後ろに付けて避ける（chobo、dandori）かは、生成器が決める。
- rulec と dandori の表は `copies` にコピーした。標準の表と同じところはそれを指し、違うところ（rulec は `Self` を持たず、Go の `complex64` と `complex128` を持たない。dandori は生成物が使う名前を足す）は、それぞれの表に持つ。C.11 で、二つは自分の表をやめて `copies` を読むようにした。rulec は出力先ごとの定数（`copies::rulec::PYTHON` など）と、`backend.rs` の並びの `BACKENDS` を、dandori は Python・Go・TypeScript の表を読む。中身はコピーしたときのままで、生成物も診断も変わらない。標準の表にそろえるかは、一つの生成パッケージ（9.3）を作る E で決める。表の違いが、dandori の生成物が rulec の生成物の名前を参照するところで食い違いを起こすかを C.11 で調べた結果は、PLAN の 7.5 にある。
  E の段階（`ritsu gen`）で、表は一つにしないと決めた。一つのパッケージの中で、ある言語の名前をほかの言語が参照するところは三つあり、どれも表の違いで食い違わない。dandori は rulec の名前（関数、列挙のクラス、単位の型、Go のパッケージ）と chobo の名前（振替のメンバーと引数）を、それぞれの口から、その言語が自分の表で決めたとおりに受け取って書く。koyomi の名前は dandori が ritsu-emit の同じ決め方（`go_package`、`pascal`、`<別名>_at`）で求めるが、koyomi は表に当たる別名を、名前を替えて避けずに E009 でエラーにするので、決め方だけで同じ名前になる。表が違っても、名前を作り直すところが無いので、食い違いは起きない。新しく名前を使うのは、パッケージのインデックスのファイル（`index.ts`、`__init__.py`）と Go のディレクトリ（パッケージ）で、それは言語が作ったモジュールの名前をそのまま使う。rulec は、出力先が予約している語の別名を警告（W121）で通すので、`ritsu gen` は、インデックスのファイルと Go のディレクトリが import するモジュールの名前を標準の表（Python 3.14.6 の `keyword.kwlist`、Go 1.25 のキーワード）で確かめ、当たれば何も書かずにエラーで止まる（`crates/ritsu/tests/gen.rs`）。TypeScript の `export * as <名前>` は予約語も名前に取れるので確かめない。
- 名前（`ident`）、リテラル（`lit`）、生成物の頭の一行とコメント（`header`）は、koyomi と chobo の形である。9.2 の頭（`Code generated by <名前> <ritsu のバージョン>.` と元のファイルとハッシュ）にそろえるのは、生成物が変わるので E の段階（9.3、9.4）にする。E.7 でそろえた（9.2。`header` に `VERSION`、`generated(tool)`、`Source`、`Origin`、`file_name` を置き、rulec・koyomi・chobo・dandori がそれで書く）。

## 10. テストの組み立て

### 10.1 いまのテストの重さ

| | 件数と時間 | 外のもの |
|---|---|---|
| rulec | 727 件が飛ばし 0 で通った（0.22.1 のリリースのとき、2026-10-01 の記録）。GitHub の CI の test のジョブは 28 分（2026-09-25 の記録）。Kani は 118 のハーネスで 192 秒 | python3（NumPy、mypy、ruff、Connect の venv）、node、rustc、ruby（rbs と steep）、php、go、swiftc、JDK、protoc と buf、PostgreSQL、Lean。CI では Kani と wasmtime も |
| dandori | 全体で 6 分 20 秒、SKIP 0（2026-10-02 の記録）。大半は examples の 24 件で、同じ日の別の実行では 365 秒 | rulec 0.22.0、Node のツール（npm の六か所）、Python の venv、Go、Temporal の dev server、kind の上の Argo と argo CLI、LocalStack 4.14.0 の Docker イメージ、Chrome、Mermaid。任意で Ollama と TypeSafe の Jev（通信する） |
| koyomi | 99 件、SKIP 0、39〜52 秒（2026-10-03 の記録） | tsc、mypy、go、rustc、PostgreSQL、Chrome |
| chobo | 63 件、SKIP 0、約 1 分（2026-10-03 の記録） | PostgreSQL、TigerBeetle、Node と Python のランナー、Go、Chrome、Mermaid |
| geas | 236 件、SKIP 0、77 秒（2026-10-03 の記録） | pixie で作った greeter、Chrome、LLVM のツール、Go、Node、Python |
| yuen | 94 件、SKIP 0（2026-10-03 の記録） | Python の venv（prov と reqif）、ReqIF のスキーマ、xmllint |
| sakai | 97 件、SKIP 0、約 20 秒（2026-10-03 の記録） | import-linter、dependency-cruiser、Java と ArchUnit、Context Mapper、go-arch-lint、buf、rulec・koyomi・chobo・dandori のバイナリ |

rulec の Kani の記録（`crates/rulec/experiments/kani/report.txt`）は、コーパスが英語の双子で 87 本になったあと（10.10）、207 本のハーネスが通り、手元で 766 秒かかる（`.github/workflows/kani.yml` の 60 分の上限の内）。

段階 F の終わり（2026-10-04）に、全部を取り込んで 0.23.0 にそろえた main で、根から `cargo test --workspace --no-fail-fast -- --nocapture`（dandori の重い 12 件を `--skip` で除く）を一度回した。1,955 件が通り、落ちたものは 0、ignored は 2（rulec の `予算の分布を測る` と、ritsu の `one_version_for_the_workspace_and_every_crate`。後者はそろえたあとに `--ignored` で回して通った）、1,057 秒だった。ritsu-model の 13 件は、ほかの作業場所と共有したビルドの置き場所に、消した作業場所のパス（`CARGO_MANIFEST_DIR`）が焼き込まれたまま SKIP したので、作り直して回し直し、13 件が SKIP 0 で通った（77 秒）。dandori の重い 12 件は一件ずつ回し、どれも通って SKIP は 0、合わせて 635 秒（Temporal の TypeScript 74、Python 63、Go 72、言語をまたぐもの 108、Worker Versioning 19、子 13、記録した履歴の再生 15、Argo 151、LocalStack 71、durable 17、pydantic-graph 5、Ollama 27 秒）だった。

同じ日の夜、X4 を 0 から通す直し、rulec の英語の台帳と英語の出力と英語のページ、rulec と dandori のサイトの `website/` への移動を取り込んだ main で、同じ全体をもう一度回した。1,972 件が通り、落ちたものは 0、ignored は 2、SKIP は 0（ritsu-model の突き合わせと、Zensical でサイトを組む確かめを含む）、1,136 秒だった。dandori の重い 12 件は、その前の回のあとで dandori の生成器、runner、規則の読み方を変えていないので、回し直していない。

全部を一度に回すと、dandori だけで負荷の平均が 60〜110 になり（2026-10-01 に dandori の全体を回したときの記録）、負荷の中でしか出ない揺れ（Argo のコントローラーが立ち上がり直す、Temporal の dev server が間に合わない）もある。毎回全部を回す形にはしない。

### 10.2 段

テストを三つの段に分ける。

| 段 | 走らせるもの | 目安 |
|---|---|---|
| `fast` | cargo のほかに何も要らないテスト。字句、構文、検査、診断の golden、変異、`explain` の再現、`naming.tsv`、api の JSON、外のツールを走らせない文書のテスト。git は使ってよい。curl も、テストが自分の中に立てたサーバー（e-Gov と eCFR の代わり）に問い合わせるためなら使ってよい（土台の `sources` が curl で問い合わせるため） | ワークスペース全体で数分（2026-10-03、この機械で 2 分 15 秒） |
| `tools` | 手元に入れるツールが要るテスト。生成したコードの型の検査と突き合わせ（Node、Python、Go、rustc、Ruby、PHP、Swift、Java、protoc と buf）、PostgreSQL、TigerBeetle、Chrome、Mermaid、xmllint、Lean、sakai の四つのリンター | 数十分 |
| `platforms` | サービスやクラスタを立てるか、外と通信するテスト。dandori の Temporal の dev server、kind の上の Argo、LocalStack、Ollama、TypeSafe、e-Gov と eCFR に本当に問い合わせるもの、Kani | 長い。揺れがある |

テストは、要るものを `ritsu-testkit` で言う（`need(Tool::Postgres)` など）。`RITSU_TEST_LEVEL` が `fast`、`tools`、`platforms` のどれかなら、その段までのテストだけを走らせ、それより上の段のテストは SKIP の行を出して通す。`RITSU_TEST_LEVEL` が無ければ、いまと同じく、見つかったツールで走れるものを全部走らせる（B と C の振る舞いを変えないため）。

### 10.3 SKIP の扱い

- 書き方を一つにする：`SKIP: <クレート>: <理由>`。rulec のテストの `注意:` と dandori などの `SKIP:` を、C の段階でこの形にそろえる。`rulec test` が出す `skipped` の行は、テストの文ではなく rulec のコマンドの出力なので変えない。
- `RITSU_SKIP_LOG=<ファイル>` があれば、`ritsu-testkit` は SKIP をそのファイルにも一行ずつ書く（クレート、テストの名前、理由）。`cargo xtask test` はそれを集め、最後に表にして出す。
- CI のジョブには、許す SKIP の一覧（`ci/skips/<段>.txt`）を置く。一覧に無い SKIP が出たら、ジョブは落ちる。rulec の CI が「skipped の行があれば落とす」としているのと同じ考えで、走っていないものを走ったように見せない。
- 「全部通った」と言う前に、SKIP の行を数える（作者の決まり）。
- SKIP の理由は英語で書く。ritsu-testkit の skip が拾って `cargo xtask test` の表に出し、CI の記録で作者でない人も読むからである。rulec のテストには、日本語の理由がまだ 109（26 のファイル）ある。`tests/wasm_target.rs` の四つは英語にし、wasmtime が無いときに SKIP の行を出さずに返していたところも、SKIP の行を出すようにした。残りをそろえるかは作者が決める。
- dandori の `books_run_on_postgres_and_tigerbeetle` は、PostgreSQL と TigerBeetle がどちらも無いとき、TigerBeetle の側の SKIP にも PostgreSQL の理由を書いていた（CI の記録の「PostgreSQL is not here …; the books are not run on tigerbeetle」）。出力先ごとに自分の理由を書くようにした。

### 10.4 変えたところだけを回す

`cargo xtask test --changed <基準のリビジョン> [--level <段>]` は、変わったファイルからクレートを選び、そのクレートに依存するクレートも足して（`cargo metadata` の依存から）、そのテストだけを回す。土台の層が変われば全部になる。文書だけが変わったら、そのクレートの文書のテストを回す。作業の途中は変えたところだけを回し、全体は区切りで一度回す（dandori でも、作業中は変えたフローだけを回し、全体はコミットの前に一度回している）。

### 10.5 CI

ritsu のリモートを作るまで（作者が決める）、CI は走らない。ワークフローは C の段階で書いておく。

| ジョブ | いつ | すること |
|---|---|---|
| `fast` | push と pull request のたび | `cargo build --workspace --locked`、`RITSU_TEST_LEVEL=fast cargo test --workspace --locked`、依存の決まりの確かめ（3.4。`cargo xtask deps` と、ritsu 自身の地図の `ritsu check ritsu.ctx`） |
| `tools` | main への push、コードを変えた pull request、毎晩 | ツールを入れ（rulec の `ci.yml` の一覧に、koyomi、chobo、geas、yuen、sakai、dandori のものを足す。PostgreSQL は、rulec にはサービスで、ほかには使い捨てのクラスタで）、クレートの組ごとに並べて `RITSU_TEST_LEVEL=tools` で回す。許す SKIP は、CI で用意できない pixie の greeter の四つだけ（下） |
| `proofs` | `proofs/` か、証明書とモデルにかかわるコードを変えたとき | `lake build`、コーパスの証明書の再検査、Lean のモデルとの突き合わせ（11 章） |
| `kani` | 毎晩と、rulec の生成器を変えたとき | rulec の CI の Kani の段（生成した Rust のハーネス） |
| `platforms` | 毎晩、手で始めたとき、`crates/dandori/` を変えた pull request | kind の上の Argo、LocalStack、Temporal の dev server を立てて、dandori の `platforms` の段を回す。外のサーバー（e-Gov、eCFR、Buf Schema Registry）に問い合わせるテストもここで回す（下）。ほかのジョブと並べない |
| `release` | タグ（F） | 13.2 |
| `docs` | 手で始めたとき（サイトを切り替えるまで） | ritsu のサイト（根のページと rulec と dandori のサイト）を `website/build.sh` で組み、出力の木に要るファイルがあることを確かめてから GitHub Pages に出す（13.2。F.7 で足した） |
| `audit` | push（ブランチ）と pull request のたび、毎日、手で始めたとき、`release` から | cargo-deny で `Cargo.lock` のアドバイザリ、ライセンス、二つ目のバージョン、出どころを、osv-scanner でリポジトリのすべてのロックファイルの既知の脆弱性を確かめる（3.6） |

C.12 で、`release` のほかの五つを根の `.github/workflows/` に書いた（ジョブ一つにファイル一つ。`fast.yml`、`tools.yml`、`proofs.yml`、`kani.yml`、`platforms.yml`）。F.7 で `release.yml`（タグで走る。13.2）と `packages.yml`（main への push と pull request で、文書だけの変更を除く）を足した。`packages.yml` は、rulec の `ci.yml` の `packages` のジョブのうち、静的な `ritsu` の musl のビルド、アーカイブ、`.deb` と `.rpm` を Debian と Fedora に入れて消すこと、を引き継ぐ。`cargo package` の半分は、crates.io に出さないので引き継がない。許す SKIP の一覧は `ci/skips/fast.txt`、`tools.txt`、`platforms.txt` にある。リモートが無いので、どれもまだ走らせていない。手元で確かめたのは、YAML として読めること、actionlint（v1.7.12）が何も言わないこと、`run` の中身が `bash -n` を通ること、ジョブが呼ぶコマンドがこの機械で通ることである（PLAN の C.12）。書いたときに決めたことは次のとおり。

- `fast`：新しく取り出した木で走らせることを考え、`website/rulec/sync.sh` と `website/dandori/sync.sh` で（F.7 で rulec と dandori のサイトを根の `website/` の下に移した。`tools.yml` と `platforms.yml` の同じ段も直した）、サイトが共有するページのコピー（gitignore してある）を先に作る。テストは `cargo xtask test --level fast` で回し、`ci/skips/fast.txt` は空である。 F.5 で、ritsu-wasm を wasm32-unknown-unknown でコンパイルする一段を足した（`cargo check --locked -p ritsu-wasm --target wasm32-unknown-unknown`）。言語のクレートが Unix にしかないもの（プロセスグループ、シグナル）を、ほかの対象でどうするかを書かずに使うと、ここで落ちる。ページのテストのうち、ライブラリとバイナリを突き合わせる二つは fast の段で、node と Chrome を使う二つは tools の段（dandori の組。`-p ritsu` を回している）で走る。
- `tools`：クレートを三つの組（rulec、dandori、それ以外の五つの言語と `ritsu-base`・`ritsu-testkit`・`ritsu-proto`・`ritsu-emit`・xtask）に分け、matrix で並べて走らせる。組ごとに要るものだけを入れる。PostgreSQL は、rulec の組がサービスのサーバーを libpq の環境変数で使い、ほかの組は PGDG の PostgreSQL 18 のプログラムで使い捨てのクラスタを立てる（`RITSU_PG_BIN`）。dandori の組は、rulec 0.22.0 のリリースのバイナリをチェックサムで確かめて `DANDORI_RULEC` に渡し（D.3 まで）、protoc 35.1 のリリースの zip を、書いたときに取ったチェックサムで確かめて入れる。rulec の `ci.yml` が `cargo test` のあとに走らせていたもの（`rulec test --require-all` で飛ばした側が無いこと、証明書の再検査、`fmt --check` と `check`）は、rulec の組の最後に残した。`--proofs` の付いた回は `kani` に移した。`ci/skips/tools.txt` は、PLAN の C.12 が空としていたのと違い、geas の pixie の四つを許す。pixie は ritsu の外でビルドするもので、pixie のテストは CI では回さず、greeter のある手元の機械で回すと決めた。 dandori の組は、ritsu の `tests/website.rs` がサイトを組むために、Zensical を `website/.venv` に入れる（F.7）。
- `proofs`：根の `proofs/` で `lake build` を一度だけ走らせ（五つのライブラリと、`rulec-recheck` と `ritsu-model`）、コーパスの全部の証明書を `rulec-recheck` にかけ、rulec の `tests/lean.rs` と `cargo test --release -p ritsu-model` を `tools` の段で回す。どちらも SKIP の行が一つでもあれば落ちる（突き合わせが走らなかったことになる）。走るのは、`proofs/`、`crates/ritsu-model/`、rulec の src・コーパス・`tests/days/`・`tests/lean.rs`・`tools/recheck.py`、土台の src、口（`ritsu-ports`）と `ritsu-cross` の src、chobo・koyomi・dandori の src と突き合わせが読む例とテストの材料、`Cargo.lock` のどれかが変わったときである。`tools` の rulec の組は、rulec のテストが使う `RulecCert` と `rulec-recheck` だけを作る。`fast` のジョブでは、`ritsu-model` の文字で穴を探すテストだけが走り、ほかは段の SKIP になる。C.12 では rulec の `ci.yml` の `proofs` のジョブをコピーしていたが、F.6 で rulec の証明を根に移したので、この形にした。
- `kani`：rulec の `ci.yml` の Kani の段（コーパスの全部の規則を Rust にして Kani で証明する）と、`rulec test --proofs` の回（`フラグを付ければ証明が走る` を platforms の段で）。毎晩と、rulec の生成器、`ritsu-emit` の src、コーパスが変わったとき。
- `platforms`：kind の上の Argo（kind 0.33 は Go の `go install` で、argo CLI v4.1.4 はチェックサムで確かめて入れ、`crates/dandori/tools/argo/setup.sh` でクラスタを作る）、LocalStack 4.14.0 のイメージ、Temporal の dev server（TypeScript の SDK の `@temporalio/testing` が取ってくる）を用意し、dandori の platforms の段のテストを一つずつ回す（10.6 のとおり、落ちたら一度だけ回し直し、そのことを出力に書く）。最後に kind の上にワークフローが残っていないことを確かめる。そのあと、外のサーバーに問い合わせるテスト（土台、koyomi、yuen の本物の e-Gov と eCFR、rulec の Buf Schema Registry）を platforms の段で回す。10.5 の表に無かったこの四つは、ほかにどのジョブも回さないので、ここに置いた。TypeSafe には CI から送らない。呼ぶたびにお金がかかり、CI では呼ぶ回数を見込めないので、鍵をリポジトリの secret にも置かない。ワークフローは `TYPESAFE_API_KEY` を空にして走らせるので、secret があっても読まず、Jev のテストは SKIP になる。それと Ollama の無い runner での SKIP を、`ci/skips/platforms.txt` で許す。

クレートの中に残っていた `.github/workflows/` は、GitHub が走らせない（2.1）。どれも消した。

- rulec の `ci.yml`：中身は根の `tools`・`proofs`・`kani`・`packages` に移した。クレートの中のものは、ritsu の最初のリリースのあと（2026-10-05）に、`release.yml`、`packaging/`、`action.yml` と一緒に消した（`packages` のジョブだけを消して残りを残す形は取らなかった。残るジョブも走らず、同じことを根がしている）。
- rulec の `docs.yml` と dandori の `docs.yml` はサイトを移したとき（F.7）に、rulec の `release.yml` は上と同じときに消した。
- rulec の `experiments/library/.github/workflows/` の三つは、規則のライブラリを外のリポジトリに出したときに走らせる見本で、ritsu の CI ではない（ritsu の中では走らない）。

2026-10-05 の昼に、リポジトリを public にして初めて GitHub で回した。`fast`、`proofs`、`packages`、`docs` は通り、`tools` は三つの組がどれも落ちた。rulec の組では、rulec の `witとモジュールはcomponentになりwasmtimeが呼べる` が wasm-tools を見つけられず、その SKIP が一覧に無かった。rulec の `ci.yml` も wasm-tools を入れていなかったので、このテストは CI で一度も走っていなかった。rulec の組には、wasm-tools 1.245.1（このテストを書いた機械の版）のリリースのアーカイブを、書いたときに取ったチェックサムで確かめて入れる。数 MB なのでキャッシュはしない（`cargo install` は数分かかり、版ごとのキャッシュも要る）。dandori の組は、dandori の `tests/examples.rs` の `books_run_on_postgres_and_tigerbeetle`（例が呼ぶ帳簿を chobo のクライアントで PostgreSQL と TigerBeetle に流す）と、ritsu の `tests/gen.rs` の `the_packages_pass_the_type_checkers`（パッケージを tsc と mypy で確かめる）の、六つの SKIP で落ちた。dandori の組には、chobo の `tools/runner`（npm と `.venv`）、TigerBeetle（`fetch.sh` と `RITSU_TIGERBEETLE`）、koyomi の `.venv`（mypy）を入れる段を足し、PostgreSQL 18 のプログラムを入れる段を dandori の組でも走らせる（`if: matrix.group != 'rulec'`）。入れ方は、ほかの五つの言語の組と同じにした。ほかの五つの言語の組では、sakai、geas、chobo の四つのテストが落ちた（10.6）。 直したあとは、同じ日の夜に、六つのワークフローが全部通った。

2026-10-06 に `audit.yml` を足し、`release.yml` に `audit` のジョブと cargo-auditable の段を足した。手元で確かめたのは、ジョブが走らせる二つのコマンド（cargo-deny 0.20.2 と osv-scanner 2.6.0。macOS の arm64 のリリースのバイナリ）がこの機械で通ること、`crates/ritsu/tests/audit.rs` が通ること、actionlint 1.7.12 が全部のワークフローに何も言わないことである。GitHub ではまだ走らせていない（Linux のバイナリのチェックサムは、リリースの `SHA256SUMS` と `.sha256` から取った）。

`crates/ritsu-base/tests/yaml.rs` は、リポジトリの中のケースだけを読むので fast の段で走る。sakai の webshop の例の CML を Context Mapper の検査器に通すテスト（`crates/sakai/tests/contracts.rs`）は、ほかの CML のテストと同じく tools の段である。


### 10.6 揺れるテスト

dandori の重いテストには、原因を突き止めていない揺れがある（dandori の DESIGN 7 章の終わり）。`platforms` の段は、ほかの段と同時に走らせない。落ちたテストは一度だけ回し直してよいが、回し直したことを出力と報告に書く。二度続けて落ちたら、揺れではなく失敗として読む。

CI を初めて回したときに落ちた四つは、どれも GitHub の環境か負荷の中でしか出ないもので、原因を突き止めて直した。

- sakai の `dependency_cruiser_keeps_the_map`：GitHub Actions は `CI=true` を置く。dependency-cruiser が色に使う picocolors は、`CI` があればパイプに出すときも色を付けるので、`depcruise --info` の行が `\x1b[32m✔\x1b[39m` で始まり、`✔` で始まるかの確かめが外れた。import-linter が使う rich は `FORCE_COLOR` で同じことになる。テストは、出力を読むツールに `NO_COLOR=1` を渡し、`FORCE_COLOR` を消して走らせる（sakai の DESIGN 7.6）。
- geas の `go_service`：`snap -j4` で、ある worker が `port auto` のポートを読むソケットを開いているあいだに、別の worker がサービスを起動すると、起動中のプログラムがそのソケットのコピーを exec まで持つ。geas が閉じたあとも、ポートは誰も受け付けないまま開いていて、そのポートを渡された主張は、コピーへの接続でサービスが立ったと読み、リクエストはコピーが消えるときに切られた（E033）。Linux のコンテナで tally-node に同じことが起き、そのとき接続した先の LISTEN のソケットは、どのプロセスのものでもなかった。ポートを読むことと、プログラムを起動することを一つのロックで分けた（geas の DESIGN §10）。
- chobo の `every_scenario_matches_on_every_target`：テストのコピーの仮押さえは本当の時間の 3 秒で切れるが、参照インタプリタは `pass` でしか時間を進めない。CI の遅い機械では、与信の `together` の終わりの読み取りが仮押さえから 3 秒より後になり、TigerBeetle が仮押さえを `expired` にした。ランナーは、そういうシナリオに `late` を付け、テストは、遅れて食い違ったシナリオだけを別のテナントでもう一度流す（chobo の DESIGN 6 章の突き合わせ）。
- chobo の `what_only_postgres_has`：REPEATABLE READ では、続けざまに呼ぶ呼び出し元がいると、負けた呼び出し元は相手が呼び終えるまで失敗し続ける。生成するクライアントのやり直しを 10 回から 30 回にした（chobo の DESIGN 4.1）。PLAN 7.9 に「ほかの作業で機械が混んでいて使い切ったと見られる」と書いた落ち方も、これである。
- dandori の `temporal_activities_run_in_the_other_language`（2026-10-06、毎日の `platforms` で）：ワークフローとアクティビティを別の言語で動かすこのテストは、全部のフロー（およそ 25 本）を二つの組み合わせで同時に走らせ、フローごとに二つの言語のワーカーを立てていた。GitHub のランナーの 4 コアでは、いくつかの実行で、最初のアクティビティが StartToClose の時間切れ（規則・日付・帳簿の呼び出しは 10 秒）になった。落ちるフローは回ごとに違い、答えの食い違いではなかった。生成するコードの時間切れは変えず、テストが同時に走らせるフローの数を、機械のコアの数（`DANDORI_AT_ONCE` で変えられる）までにした。

### 10.7 golden と取り直し

`RITSU_BLESS=1` で全部の golden を、`<名前>_BLESS=1` でそのクレートの golden を取り直す（いまの名前は残す）。取り直したら差分を読む。golden に入っているツールのバージョン（dandori の 57 のファイルの `rulec 0.22.0` など）は、バージョンが一つになれば、リリースのたびに一度取り直すだけになる。

### 10.8 テストの共通部分（`ritsu-testkit`）

1.2 の重なりを一つにする：自分を消す一時ディレクトリ（終わったプロセスの分も消す）、ツールを環境変数と既定の場所と PATH で探す（`RITSU_<ツール>` と、いまの `<クレート>_<ツール>` の名前の両方）、時間を区切って子プロセスを走らせる、golden と取り直し、SKIP、使い捨ての PostgreSQL のクラスタ（ソケットのパスは 103 バイトまで）、TigerBeetle のレプリカ、Chrome を探して時間を区切って走らせる（`--user-data-dir` は一時ディレクトリ）、Mermaid。std だけで書く。dandori の Argo、LocalStack、Temporal のランナーは dandori の `tools/` に残す。

### 10.9 段階 C で作った形（`ritsu-testkit` と `xtask`）

- `ritsu-testkit` には、10.8 のものに、テストの中に立てる小さな HTTP サーバー（yuen の形。e-Gov と eCFR の代わり）を足した。一時ディレクトリの名前は `ritsu-test-<クレート>-<pid>-<n>-<用途>` で、最初の一つを作るときに、終わったテストのプロセスの分を消す。そのプロセスが書き残したサーバー（PostgreSQL、TigerBeetle）は、まだ同じプログラムであるときだけ止める（プロセスの番号は、そのあいだに別のプログラムのものになっていることがある）。golden の取り直しは `RITSU_BLESS` と `<クレート>_BLESS` のどちらでもよく、空と `0` は取り直さない。
- SKIP の記録は、一行に四つの値をタブで区切って書く。クレート、テスト、理由の種類、理由である。理由の種類は `level`（`RITSU_TEST_LEVEL` が上の段のテストを外した）と `missing`（要るものがこの機械に無い）の二つで、`cargo xtask test` が `ci/skips/<段>.txt` と突き合わせるのは `missing` だけにした。段で外したテストは、その段の意味どおりに外れたもので、一覧に書き並べても何も確かめないからである。
- 本物の外部のサービスに問い合わせるテストが段で外れたときは、SKIP ではなく `not asked:` の行を出す（yuen の形）。SKIP の数は、この機械に無いものの数として読めるように保つ。
- `cargo xtask test --changed <リビジョン>` は、変わったファイルのクレートと、それに依存する（どの種類の依存でも）クレートを回す。クレートの外のファイル（根の `Cargo.toml` など）が変われば全部を回す。文書だけの変更を見分けて文書のテストだけを回すことは、まだしない（そのクレートのテストを全部回す）。
- `cargo xtask deps` は、`cargo metadata` の宣言した依存を 3.1 の表と突き合わせ、破った依存を名指して exit 1 で終わる。表に無いクレートと、`ritsu-testkit` を `[dependencies]` に置いたクレートも名指す。
- C の二つ目の部分で、koyomi・chobo・geas・yuen・sakai のテストの共通部分をこれに替えた。足したのは `TempDir::exists`（chobo）だけである。各クレートの `tests/common` には、そのクレートだけのもの（例や変異の並べ方、外のツールの既定の場所）が残る。ツールの変数は、`RITSU_<ツール>` のあとに、いままでの `<クレート>_<ツール>` も読む。
- C.11 で rulec と dandori のテストもこれに替えた。足したのは、段を聞いてから機械を聞く `ready(要るもの, 見つかるか, 理由)`（rulec と dandori のテストは、ツールが無いかだけを聞いていた）と、`Need::Rulec`（dandori のテストが規則を読む rulec 0.22.0。D.3 で消した）である。C.12 で、`Need::Suite`（sakai のテストが例のコピーを確かめる rulec・koyomi・chobo・dandori のバイナリ。D.8 まで）と `Need::Pixie`（geas のテストが動かす pixie の greeter）を足した（下）。三つとも tools の段である。`Need::Suite` は D.8 で、sakai のテストが一式の言語を同じプロセスでつなぐようになって消した。rulec のテストは、PostgreSQL を `PG*` の環境変数で受け取る形のまま、SKIP と段を替えた（PLAN の C.11）。一時ディレクトリは、C のあとの片づけで `TempDir` に替えた（PLAN の 7.5）。
- C.12 で、まっさらに取り出した木を、外のツールを呼ぶと記録を残して失敗するコマンドを PATH の先頭に置いて `cargo xtask test --level fast` で回し、段を聞かずにツールを呼ぶテストを探した。geas の五か所（例を Python・Node・Go・rustc で走らせるもの、Go のサービスを SIGTERM で止めるもの、`explain` の再現、W061、pixie の greeter）と sakai の一か所（`what_was_copied_passes_the_suite`）が見つかり、段を聞くようにした。いまは、fast の段で呼ぶのは cargo と git と、テストが立てたサーバーへの curl だけである。
- C のあとの片づけで、テストが子に渡す TMPDIR を一時ディレクトリの下に作る `tmp::tmpdir_in` を足した。rulec のテストは、`rulec test` を走らせるときと swiftc を呼ぶときにこれを渡す（swiftc は `--version` に答えると、ほとんど毎回、空の `TemporaryDirectory.*` を TMPDIR に残す）。Chrome には、一時ディレクトリをプロファイルの下に向けて渡す。止められた Chrome がシングルトンのソケットのディレクトリ（`com.google.Chrome.*`）を OS の一時ディレクトリに残さないためで、macOS の Chrome はその場所を `MAC_CHROMIUM_TMPDIR` から、ほかの Chrome は `TMPDIR` から読む。
- 根から `cargo test --workspace` を回すときの `--skip` は、ワークスペースのすべてのテストの名前に効く。dandori の重い段を外す `--skip argo` は `cargo` を含む名前にも当たるので、そういう名前のテストを作らない（xtask のテストの名前を一度直した）。

### 10.10 英語の版と日本語の版（例とテストの材料）

例、テストの材料、サンプル、golden、文書の例は英語を先にする。日本語のものは消さず、中身も変えず、日本語の版として残す（対等）。英語のものは足して先に見せる。対の名前の付け方は、言語ごとに次の一つの決まりにそろえ、各言語の DESIGN.md に書く（koyomi と chobo は 10.1）。

- 例（`examples/`）：dandori と同じ形。英語の版は `<英語の名前>.<拡張子>`、日本語の版は `<英語の名前>.ja.<拡張子>`。もとからある日本語の例は、中身を一字も変えずに名前だけを替える（`git mv`）。英語の名前は日本語の版の別名と違うものにする（テストが全部の例を一つのディレクトリに生成するので、別名が同じだと生成物がぶつかる）。ほかのファイルに名指されるもの（koyomi のカレンダー、dandori の規則）は、日本語の版の名前を替えない。替えると、それを名指す日本語の版の中身を書き換えることになるからで、英語の版は英語の名前で足し、説明に対の名前を書く。koyomi は England and Wales の例を 5 本と、日本の暦の例ごとに英語に訳した版を足した。
- テストの材料、変異、golden（`tests/`）：日本語の名前のものは、名前も中身もそのまま残す（日本語の名前で日本語の版だと分かる）。同じ振る舞い（同じ診断のコードと終了コード）を確かめる英語の名前のものを足す。変異の対は、診断のコードを頭に付けた名前でそろえる（`E001_閉じていない文字列.cal` と `E001_unclosed_string.cal`）。対が同じことを言うかは、テストが確かめる（koyomi の `every_japanese_mutant_has_an_english_one`、chobo の `every_japanese_fixture_has_an_english_one`）。
- rulec のコーパス（`crates/rulec/tests/corpus/`）：日本語の名前の規則 37 本は、名前も中身もそのまま残し、同じ規則を英語の名前で書いた双子を同じディレクトリに足した（87 本になり、74 本が 37 組の対である）。対は `tests/corpus/twins.tsv` に書く。双子の名前は、元の規則の別名と重ならないようにする（重なると `gen` は何も言わずに後のファイルで上書きし、コーパスを一つのディレクトリへ生成するテストが片方を二度走らせる）。双子は元の規則の名前の置き換えで作る（`円` は `JPY`、`万` と `億` は桁）。表・セル・例はコピーしたもので、打ち直していない。`tests/twins.rs` が、対を同じ検査の結果・同じ網羅・同じ答え（ベクタ）・同じ証明書の主張・同じ `api` の約束で結ぶ。日本語でしか書けないもの（`std/都道府県` の値、法令や表計算の引用箇所の名前（`別表第一`、`第91条`、`表1`））は、双子でも日本語のまま残る（rulec の DESIGN の §15.177）。
- rulec の `explain` の台帳（`crates/rulec/src/codes.rs`）：日本語の名前・文字列・単位を使っていた再現 61 項のうち 55 項に、同じ規則を英語の名前で書いた版を持たせた。英語の出力（`rulec explain <コード>`、`--all --format markdown`、`docs/codes.md`、サイトの英語の台帳のページ）は英語の版を、日本語の出力（`--lang ja`、`docs/codes.ja.md`）は元の再現を見せる（`docs/codes.ja.md` は一字も変わらない）。英語の版は元の再現の名前の置き換えで作り（別名は残す。`円` は `JPY`、`万円` は桁、`×` は `*`）、`tests/codes.rs` が、英語の版も日本語の版も自分のコードを出すこと、英語の版が日本語の版と同じ診断を同じ行に出すことを確かめる。日本語が残る 5 項（E011：ASCII でない名前の別名が要点。E037・E038・E039・W119：e-Gov の条の名前とコピー）は、テストの一覧 `KEPT` に理由を書いて決めてあり、ほかの項の英語の再現に日本語が入るとテストが落ちる。英語の説明（`when` と `fix`）も 28 項で英語にし、E043 と E103 の英語の文を直した（★E103 の例は、`1JPY` の固定ではなく列の単位から作る。rulec の DESIGN §15.179）。
- rulec の英語の出力：診断の文の日本語の例（`src/*.rs` の `tr!` の英語の側 30 か所のうち 16 か所。残りは e-Gov の条の名前、組み込みの `std/都道府県`、ASCII でない名前を言う E011、言語が日本語の書き方も読むことを言う文で、理由は rulec の DESIGN §15.180）、大きい金額の書き戻し（★英語は `1_000_000JPY` のように 5 桁以上を `_` で区切る。規則が読む書き方で、`,` は規則の中で E049 になる。日本語は `100万円` のまま）、`doc` の範囲（英語は `0JPY to 10_000_000JPY`。日本語は `〜`）、`--help` の例と引数の名前（英語は `rules/member_shipping_fee.rule`、`<field=value>`。日本語は替えていない）を英語にした。★JSON の `fix.text` は `witness` と同じデータで、言語で変わらない（`tests/json_v2.rs`）ので、金額を英語の出力でも `万` で書く。英語の `doc` の全角のかっこ（列挙の値の `（default）`、状態の遷移の `（row 1）`）と、優先する表を囲む `「」` は、取り込むときに `src/doc.rs` の三か所を `tr!` の対にして、英語では ASCII にした（dandori の `doc` は rulec の `doc` を埋め込むので、dandori の図の英語の golden、サイトの図のページ、ブラウザで試すページの presets も取り直した）。`tests/golden_en.rs` が、英語の出力に日本語が出てよい語を `KEPT`（いまは `届け先` だけ）に絞り、全部の英語の `--help` のページを調べる。日本語の出力は、規則とコーパスと変異に走らせた全部（1,981 回）で一字も変わらない。
- rulec の英語の文書（`README.md`、`AGENTS.md`、`docs/`、サイトの英語のページ、スキル）に残っていた日本語の例は、英語の双子（`member_shipping_fee`、`single_coupon`、`cart_shipping_fee`、`coupon_stacking`、`order_lifecycle`、`nationwide_freight` など）と、英語で書いた規則（`parcel_rate`、`uk_minimum_wage`）に走らせた実物に替えた（英語の 18 ページの日本語の連なりは 729 から 300 になった。rulec の DESIGN §15.178）。出力は走らせて貼る。日本語のまま残るのは、日本語であること自体が要点の所だけである（のちに各国の一段目の区分を足し、三本の双子は `std/jp/prefectures` と英語の綴りにしたので、英語のページの都道府県の日本語の連なりは 109 から 6 になった。5.6。残るのは、`std/都道府県` の値、e-Gov の引用箇所の名前、コピーの日本語の閾値の言い方、名前と別名の例、単位と桁の言葉、全角の記号）。サイトの英語の画面写真（`try-*-en*.png`）は、はじめ `member_shipping_fee` の画面にし、rulec のサイトを移した担当が `parcel_rate` の画面に替えた（下の項）。日本語のページの画像（`try-*-ja*.png`）は元のまま。`tests/formats.rs` は `docs/formats.md` の JSON の例を実物の出力に照らし、`tests/docs.rs` は W114 の防壁の抜粋を、テストの材料の英語の規則（`crates/rulec/tests/pages/half_bound.rule`）の生成物に照らす。
- rulec の英語のページの画面写真（`try-*-en*.png` の六枚）と、`compare.md` と `scenarios.md` の突き合わせの実演（`verify`、`fixtures lint`、`replay`、`diff`）は、都道府県の無い英語の規則 `parcel_rate` の実物にした（F.7。rulec の DESIGN §15.181）。双子の `member_shipping_fee` と `yupack_base_fee` は、入力 `dest` が `std/都道府県` なので、画面と出力に都道府県の値が出ていた。英語の `flow.svg` の質問も英語にした。日本語のページと日本語の画像は変えていない（図の URL の刻印だけが変わった）。英語の `compare.md` の日本語の連なりは 7 から 0 に、`scenarios.md` は 51 から 47 になった（残りは e-Gov の条文）。テストは `tests/verify.rs` と `tests/vdiff.rs` の英語の材料を `parcel_rate` にした（日本語の材料はそのまま）。
- 日本語でしか確かめられない振る舞い（内閣府の Shift_JIS の CSV、e-Gov の条文、漢数字の条、全角の幅、日本語の名前のバイトの長さ）のテストは日本語の材料のまま持ち、同じ振る舞いを英語の材料でも確かめるテストを足す。テストは両方を回す。

### 10.11 英語の版と日本語の版（sakai、yuen、chobo、ritsu）

対の名前の付け方は、10.10 の決まりに従い、言語ごとの DESIGN.md に書く（sakai は 12.4、yuen は 16.2、chobo は 10.1）。

- 例（sakai の `examples/`）：英語の版は `examples/shop/`、もとの日本語の版は中身を変えずに `examples/shop.ja/` に名前を替えた。英語の版の中の名前（コンテキスト、語、フロー、規則、カレンダー、コードの識別子、ファイルの名前）は全部英語である。英語の版も日本語の版も、同じ境界越えと同じ診断を出す（テストが確かめる）。ritsu のテストのプロジェクトも同じで、`crates/ritsu/tests/projects/` に英語の `shop/` と日本語の `通販/` がある（8.3）。
- テストの材料と変異と golden：日本語の名前のものは、名前も中身もそのまま残す。同じ振る舞いを確かめる英語のものを、英語の名前で足す。変異の対は診断のコードの頭でそろえ、対のテストが確かめる（sakai と yuen の `every_japanese_mutant_has_an_english_one`。yuen のは、対を一覧で持ち、英語の側に日本語の文字が無いことと、同じコードが同じ順で出ることまで確かめる）。名前が ASCII でも中身が日本語のものは、日本語の側に数える（yuen の `E011_dir`）。英語の golden の名前は、日本語の golden と重ならないようにする（重なると黙って上書きする）。
- 法令の材料：英語の側は米国の eCFR（yuen の `tests/fixtures/period_of_months` は 37 CFR 1 の §1.6、1.7、1.8、1.10、`fee_rules` は §1.17 と 1.27。どれも `yuen source fetch` で取った本物のコピー）。e-Gov の条の漢数字、項・号・別表・附則のファイルの名前、版の ID と施行日を使う問い合わせは、日本語の材料のまま持つ（eCFR にそれが無いから）。koyomi が固定できる法令は e-Gov のものだけなので、「koyomi のカレンダーが固定した条が改正されたとき」を見るテスト（yuen の `an_article_taken_into_a_calendar_marks_only_what_reads_it`）には英語の対が無い。同じ振る舞いの核（借りた出典の端、印が付く範囲）は、カレンダーが固定する英国の祝日の JSON（`calendar_sources`）で確かめる。
- 順に歩くテスト（yuen の `mutants.rs`、`export.rs`、`codes.rs`、sakai の `mutants.rs`）は、本体を変えずに英語の材料も歩く。golden は材料の名前ごとなので、英語のものは別の名前で足す（日本語の golden を上書きしない）。
- chobo のテストの帳簿の名前は、PostgreSQL のスキーマの名前になるので、`examples/` の帳簿の名前と重ならないものにする（`stock_reservation`）。日本語と英語の対のテストが同じ PostgreSQL に同時に負荷をかけないよう、`tests/postgres.rs` は `ONE_AT_A_TIME` の mutex で一つずつ走らせる。

### 10.12 根の README とスキルの確かめ（F.3 で作った）

根の README.md と README.ja.md は `crates/ritsu/tests/readme.rs`（7 件）、`skills/ritsu` は `crates/ritsu/tests/skill.rs`（6 件）が確かめる。二つに共通の部分（Markdown の塊の読み取り、`console` の塊のコマンドを走らせること、コードの塊をファイルと突き合わせること、リンク、本文が名指すコマンド）は `crates/ritsu/tests/common/mod.rs` にある。

- コマンドは、リリースを PATH に展開した人が見つけるとおりに、`ritsu` と、それを指す言語の名前のリンクを置いたディレクトリを PATH の頭にして、`sh -c` で走らせる（標準エラーは標準出力に流す。端末が見せるとおり）。塊の行は、`…` だけの行を「任意の行数」として、出力と行ごとに一致しなければならない。
- ★走らせる場所は、リポジトリの根、塊の中の `$ cd` の先、または、同じ節で塊の上にあるコードの塊が、ritsu の台帳のある再現のファイルそのものであるときの、そのファイルを置いたディレクトリ（根の README の `ritsu check .` は E201 の再現）。`ritsu check` を根で走らせることはしない（リポジトリの全部の `.geas` の主張を走らせてしまう）。
- ★コードの塊（`rule`、`flow`、`cal`、`book`、`geas`、`req`、`ctx`）は、一つの塊の行がリポジトリの中の一つのファイルの行で、同じ順に並んでいなければならない。`…` は行の途中を切る。手本の koyomi は行ごとの突き合わせだったが、別のファイルの行を混ぜた塊を通さないために、ファイルを一つにした。
- スキルは、リンクがスキルのディレクトリの外へ出ないこと（言語のスキルと同じ。別のプロジェクトにコピーして使う）、言語のスキルの置き場所を `crates/<言語>/skills/<言語>/` の形で案内すること（★Markdown のリンクにしない）、表が ritsu の台帳のコードをすべて、それだけを挙げること、見せる出力が ritsu のテストの golden の行であることを確かめる。ページを読む人は「コードが実現すべきものを理解し、確かめる人」と書く。

八つのスキルの入れ方と呼び方は ritsu にそろえた（2026-10-05）。`compatibility` は、八つとも「`ritsu` を PATH に（`cargo install --git https://github.com/i2y/ritsu --locked ritsu`）。言語は `ritsu <言語> <command>`、ritsu への言語の名前のリンクでも同じ」と書き、スキルは rulec の Homebrew、言語ごとの `cargo install … --locked <言語>`、言語ごとの旧リポジトリやサイトの入れ方のページを案内しない（rulec の README と入れ方のページは、ritsu の最初のリリースが引き継ぐまで rulec のリリースを残す。13.2）。リンクで呼んだ dandori が、`ritsu dandori` と同じく規則を同じプロセスで読むことは、`dandori` という名前のリンクで `order.flow` を検査して確かめた。本文で走らせるコマンドも `ritsu <言語> <コマンド>` の形にした（出力の塊は替えない）。rulec のスキルは本文が AGENTS.md のコピーなので、頭で一度だけ呼び方を言い、AGENTS.md の本文のコマンドは `rulec …` のままにした（AGENTS.md は `rulec mcp` のリソースでもあり、rulec の `tests/docs.rs` が CI の行を README と突き合わせる）。AGENTS.md の CI の例は `uses: i2y/ritsu@v0.23.0`（リリースは各言語の名前のリンクを持つので、`run: rulec …` の行はそのまま動く）。geas の `tests/docs.rs` は、ページのインラインのコードのうち `geas ` で始まるものの二語目をコマンドとして確かめるので、`compatibility` の呼び方は `geas …` ではなく `geas <command>` と書く（八つともそろえた）。

## 11. Lean の層

### 11.1 いまあるもの

Lean の層は、根の `proofs/` の一つの Lake のパッケージ（`ritsu_proofs`。Lean v4.34.0、mathlib なし、依存なし）で、ライブラリは五つ、プログラムは二つである（12,490 行。まっさらから `lake build` で 20 秒ほど。2026-10-04、ほかの担当のビルドと同時に走らせて 20〜32 秒）。

- `RulecCert`（4,539 行）：rulec の証明書が通らなければならない検査を関数として書き、検査が通れば主張（完全性、重なり、当てはまらない行、単位、int64）が成り立つことを証明している。ステートマシン（`Machine.lean`）、一次の不等式を打ち消す乗数（`Linear.lean`）、契約と入口（`Contract.lean`）、koyomi の日の集合（`Sieve.lean` の `days`）も含む。プログラム `rulec-recheck`（`RulecMain.lean`）は、証明書を読んで、その関数そのものを走らせる。F の段階で `crates/rulec/proofs/` から移した。
- `ChoboModel`（1,318 行）、`KoyomiModel`（830 行）、`DandoriCore`（2,755 行）：chobo・koyomi・dandori の意味の中心部分のモデルと、その性質の証明（11.2）。
- `RitsuCross`（1,875 行）：言語をまたぐ検査の判定のモデルと、それぞれの答えが言っていることの証明（11.2 の 3）。
- プログラム `ritsu-model`（`Main.lean`）は、`ChoboModel`・`KoyomiModel`・`DandoriCore`・`RitsuCross` の関数を走らせる（11.3）。

rulec の `tests/lean.rs` は、コーパスの証明書が `rulec-recheck` を通ること、偽った証明書が落ちること、rulec の主張が立つ定理の `#print axioms`、パッケージのどの `.lean` にも `sorry`・`axiom`・`native_decide` が無いことを確かめる。`crates/ritsu-model/tests/proofs.rs` は、五つのライブラリのどの宣言も三つの公理のほかに立たないことを確かめる（11.2 の最後）。

### 11.2 作ったもの

AWS の Cedar と同じ形にした。意味の中心部分を Lean でモデルにし、性質を証明し、Rust の実装とモデルを、テストが作る入力で突き合わせる。

1. **chobo の振替**（`ChoboModel`）：状態（勘定ごとの確定した残高、入ってくる仮押さえ、出ていく仮押さえ。仮押さえとキー）、移動を書いた順に行い一つごとに境界を確かめること、全部か無し、キーの冪等、仮押さえの終わり方、時間が進むこと、`together`。chobo の `interp.rs` を関数で書き直したもので、定理もこの関数について言う。定理は次のとおり。
   - `apply_kept`・`pass_kept`：どの呼び出しも、時間が進むことも、勘定を境界の禁じた向きに越えさせない。出ていく仮押さえを引いた残り（下限で確かめる量）は、下限以上に保つか、減らない。入ってくる仮押さえを足した量（上限で確かめる量）は、上限以下に保つか、増えない。下限が 0 より大きい勘定は下限を下回って始まるので、言えるのは「上がるだけ」である。
   - `within_stays`：だから、境界の内にある勘定は、何を呼んでも内にとどまる。
   - `refused_keeps_balances`・`doneBefore_keeps_state`：拒否された呼び出しは残高も仮押さえも変えない（全部か無し）。前に済んでいる呼び出しは何も変えない。
   - `again_doneBefore`：通った `do` と `hold` をもう一度呼ぶと `done_before` になり、何も変えない（リトライが二重に動かないこと）。
   どれも、仮押さえの額が 0 以上であること（`State.Good`。`fits` が通すもので、どの呼び出しも保つ）だけを前提にする。
2. **koyomi の日付の計算**（`KoyomiModel`）：通算日（Hinnant の手順を Rust と同じ 0 へ切り捨てる割り算で）、月の足し算と無い日の三つの扱い、締め、月初と月末、カレンダーの決まり（休みの曜日、表の休日、毎年の休み、特定の日、例外の営業日、データの範囲）、四つの慣行、営業日の数え方、`if closed`、`at` の時刻。定理は次のとおり。
   - `monotone_of_adjacentOk`・`monotone_on_range`：`is monotonic` の検査（範囲のどの日も翌日と比べる）が通れば、範囲のどの二日についても、遅い日の値は早い日の値より前にならない。
   - `seek_following`・`seek_preceding`：`roll following` はその日以後で最初の営業日、`roll preceding` はその日以前で最後の営業日を返す。
   - `roll_open`・`addBusiness_open`・`ifClosed_open`：四つの慣行のどれも、営業日の数え方（`± 0` を含む）も、営業日を返す。`if closed` は営業日をそのままにする。
   カレンダーはどんな答え方のものでもよい形で言うので、どの祝日の表についても成り立つ。
3. **言語をまたぐ検査**（`RitsuCross`）：`ritsu-cross` の判定を、Rust と同じ形の関数として書き直した（`crates/ritsu-cross/src/borders.rs` と `preconditions.rs`、rulec の口の答え `ports.rs` の `preconditions_hold`）。定理は、どれも「答えが言っていることが成り立つ」の形である。
   - **X3 の (b) の証明書の層**（E.4。rulec の §15.174）：入力を集合に限った完全性は、rulec の証明書の層で作った。`RulecCert/Sieve.lean` の `Sieve` に `days`（軸ごとの日の集合）を足し、`Sieve.asked`（入力が尋ねる点）に「日付の軸の値は集合の日」を加え、`daysRulesOut`（座標が集合の日を一つも含まない）が点を「起きない」と言えることを `not_asked_of_days` で、パスの延長で保たれることを `daysRulesOut_mono` で証明し、`pointRuledOut` に加えた。到達の点の値の確かめ（`witnessOk`）にも集合を加えた。軸が分けず、集合の日が分ける行の対は、証明書の `days_apart` に書き、`daysPart` と `not_asked_of_daysPart` で、両方の行に入る点はどれも尋ねられないことを証明し、`Certified.pairsApart` の分け方に加えた。
   - **X2**（`Relation.lean`）：rulec の答え（`corner`）は、二つの範囲の箱のうち関係をいちばん厳しく試す角（`<` と `<=` なら左の上の端と右の下の端、`>` と `>=` ならその逆）で決める。値は wire の整数で、刻み（`wire_scale`）の違う二つの入力は、刻みをそろえて比べる（左の値 × 右の刻み と、右の値 × 左の刻み）。`corner_holds`：成り立つと言えば、範囲のどの組でも成り立つ。`corner_fails`：例を言えば、二つの値はそれぞれの範囲にあり（範囲が空でなければ）、関係は成り立たない。`corner_undecided`：決められないのは、角の端が開いているときだけ。呼び出しの場所の判定（`x2`）は、同じ値を両方に渡せば `<=` と `>=` は成り立ち `<` と `>` は破れ、範囲の無い値があれば決められず、ほかは `corner` に任せる。`x2_holds`・`x2_fails` は、呼び出しが渡しうる組（dandori の E014 と同じ読み方：値ごとに、その値を入れるすべての場所の範囲から。同じ値なら同じ値）について、同じことを言う。同じ値を両方に渡す場合は、二つの入力が同じ刻みで運ばれることを前提にする。これは dandori の E003 が保証する（刻みの違う率に同じ値を渡すと、dandori は E003 で止める）。日付の入力が koyomi の日付の日だけをとる前提（`range from koyomi`）は、渡す値がなりうる日で決める（`daysKept`）。`x2_days_holds`：成り立てば、渡す値は koyomi の日付の日にしかならず、その日付がとるどの日も規則の日である。`x2_days_fails`：例は、渡す値がその日になりうる koyomi の日付の日で、規則の日ではない。並びの合計と長さは、決めない（dandori は並びの長さを知らない）。
   - **X3 の (a) と X6**（`Borders.lean` の `daysFit`、`daysGiven`、`inputRange`）：渡す値がなりうる日が、範囲（X3 の (a) は規則の日付の入力の範囲、X6 は koyomi の日付の日付の入力の範囲 `inputRange`）に収まるか。`daysFit_holds`・`daysFit_fails`：一つの日付の日の集合について、成り立てばどの日も範囲にあり、例は集合の日で範囲の外にある（集合でそれより前の日は範囲の中）。`daysGiven_holds`：成り立てば、値は koyomi の日付の日にしかならず（何日か分からないところから来ない）、その日付がとるどの日も範囲にある。`daysGiven_fails`：例は、値がその日になりうる koyomi の日付の日で、範囲の外にある。`inputRange_some`：X6 が日を比べる範囲は、その名前の日付の入力の範囲である。
   - **X3 を、境目をまたいで**（`Days.lean`）：`daysOf` は、koyomi の口の `Dates::values` と同じく、入力の範囲のすべての組み合わせで日付のファイルの全部の日付を計算し（`KoyomiModel.DatesFile.run`）、尋ねた日付の日を集める。組み合わせが予算を超えるか、どこかで計算が止まれば決められない。`daysOf_mem`・`mem_daysOf`：集合の日は、ちょうど、koyomi の範囲のどれかの入力が計算する日である。`x3a_holds`：その集合の上で X3 の (a) が成り立てば、koyomi の範囲のどの入力でも日付は計算でき、その日は規則の範囲にある。`x3a_fails`：例は、koyomi の範囲のある入力が実際に計算する日で、規則の範囲の外にある。`x3b_complete`・`x3b_unique`：証明書が日付の軸の日の集合（`Sieve.days`）として koyomi の集合を持ち、覆いの検査（と対の検査）を通る表は、koyomi の範囲のどの入力についても、その日を日付の軸に持つ、規則が尋ねられるどの点にも、行で答える（ちょうど一つの行で答える）。X3 の (b) を `ritsu check` が「確かめた」と数えるのは、規則が自分の検査を通るときで、そのとき言えることがこれである（rulec の検査と、それを証明する `RulecCert` の定理から、koyomi の入力まで）。証明書の日の並びが koyomi の集合であることは、突き合わせで確かめ（11.3）、証明はしていない。
   - **X4**：`amountFits_holds_iff`：一つの出力の額の判定が成り立つのは、出力の両端が分かり、下の端が 0 以上、上の端が 2⁶³ − 1 以下のときだけ。`amountFits_holds`・`amountFits_fails`：そのとき出力の範囲のどの値も 0〜2⁶³ − 1 にあり、例は負か 2⁶³ − 1 超で、行が書く数か範囲の端である。`amountsGiven_holds`：値がなりうる額（規則の出力と、dandori が範囲を知る数）の判定が成り立てば、値は何も言わないところから来ず、どの出力の範囲のどの値も、どの範囲のどの数も 0〜2⁶³ − 1 にある。`amountsHull_covers`：chobo の探索に渡す範囲（`amountsHull`）は 0〜2⁶³ − 1 の中にあり、値がなりうる額のうち chobo が受け取るものを全部含む。`amountFits_chobo_takes`・`amountFits_fails_chobo`：判定が通す額は、chobo のインタプリタ（`ChoboModel.fits`）が額の引数として受け取り、判定が例に挙げる額は受け取らない（7.6。額 0 から数えるようにしたときに、後者を足した）。`refusalsMet_holds`：成り立てば、額で決まる理由（帳簿の境界の理由）のうち、探索が見つけたものはどれもタスクが処理し、タスクが処理するものはどれも探索が見つけている。`refusalsMet_fails`：例は、探索が見つけたのにタスクが処理していない理由（一つはある）と、タスクが処理するのに探索が見つけていない理由を、過不足なく挙げる。`refusalsMet_undecided`：決められないのは、探索が見つけた理由はどれも処理していて、探索が見つけない理由をタスクが処理しているときである。
   - **X5**（`heldUntil`）：仮押さえを作ってから呼び出しまでの秒数の最小と最大を、有効期限の秒数と比べる。`heldUntil_holds`：成り立てば、フローがしうるどの呼び出しも有効期限より前に来る。`heldUntil_fails`：例（最小の秒数）を言えば、どの呼び出しも有効期限かそれより後に来る。`heldUntil_fails_expired`：そのとき、`t0` に作り、有効期限が来る仮押さえ（`deadline = t0 + 期限`）は、呼び出しが来たとき `ChoboModel.expiredAt` が期限切れと言う（chobo は確定も取消も `expired` で拒否する）。`heldUntil_holds_held`：成り立てば、どの呼び出しのときも期限は切れていない。秒数の最小と最大は、dandori がフローに沿って数えたものをそのまま受け取る（`Flows::crossings` の `holds`）。
4. **dandori の芯**（`DandoriCore`）：文（`match`、上限のある `for` と `repeat`、`break`、`succeed`、`fail … leaving`）、タスクと規則の呼び出し、その引数と結果（リトライ、ハンドラ、`on failure`）、koyomi の日付のファイルの日付（`use dates`）と chobo の帳簿の操作（`use book`）、式 `now`、ステートマシンか帳簿の振替の仮押さえに従う案件（`case … follows`）。二つの部分からなる。
   - シナリオを流す部分（`Replay.lean`）：dandori の参照インタプリタが Temporal の見え方（エラーを種類で呼ぶ）で流すのと同じに流し、通った文・分岐・応答・ハンドラ・イテレーション、呼び出しごとの引数、終わり方、案件のレコードの状態を出す。`now` はシナリオの時刻を読み、帳簿の操作は、渡した引数から作る仮押さえ（キーの引数と、操作が残す状態）を返す。`for … in parallel` と `on cancel` も入る。突き合わせのためのもので、`partial` の関数で書いてあり、これについては何も証明しない（構造的な再帰で書くと、カーネルが深い再帰で止まった。定理はこの部分に依存しない）。
   - 案件の状態の検査（`Check.lean`）と、外部のサービスの本当の状態まで入れた実行の意味（`World.lean`）と、証明（`Sound.lean`）。定理 `chkFlow_sound` は、検査が通る flow なら、検査が見る終わり方（flow の終わり、`succeed`、`fail`、`on failure` の終わり）で終わるどの実行でも、`leaving` で引き渡す案件のほかは、始まっていないか終わりの状態にいて、外部のサービスのイベントでもそこから出ないことを言う。実行は一つのシナリオではなく、flow と外部のサービスが一緒に作れるどの実行でもよい（呼び出しはどのエラーでも返りうる、案件の状態以外の `match` はどの分岐にも進みうる、ループは何回でも回りうる、外部のイベントはどの呼び出しの前にも起きうる）。帳簿の操作は、仮押さえに従う案件にとっては `starts`（hold）か `sends`（post と void）で、拒否されたときは、その状態での帳簿の理由がエラーとして返る。日付の呼び出しは規則の呼び出しと同じく案件を動かさず、`now` は案件に関わらない。キャンセル（Temporal の `on cancel`）は、この定理の実行に入れていない。Lean の検査は `for … in parallel` を受け付けない（★並列のラウンドの失敗は全部のラウンドが終わってから上がるので、その実行の意味を書くのは次にした）。
   実行が前提にすることは四つで、どれも dandori の検査の読み方と同じである（`World.lean` の冒頭）。
   1. 一つの呼び出しは、何度リトライしても外部では一つのイベントである。タスクの `key`（帳簿の操作では帳簿のキー）が与えるもの。
   2. 案件は一度だけ始まる。
   3. **案件を始める呼び出しが失敗したら、案件は始まっていない**（`World.lean` の `CallStep` に、始める呼び出しが失敗したあとで外部に案件ができている場合を入れない）。現実の仮定としては、外部のサービスが案件を作ったなら呼び出しは成功を返す、つまり作ったのに失敗として返ることは無い、ということである。実際には、作ったあとで応答がタイムアウトすることがある。dandori はその案件をレコードの無いものとして数えず、そういう案件を見失わないよう、始めるタスクに `key` を付けることを W103 で勧める（`key` があればリトライで同じ案件が返る）。この前提を外すと、始める呼び出しが失敗したあと `on failure` で「レコードが無いなら何もしない」（`none => pass`）とする flow がどれも E020 にあたり、例のホテルの予約などを `none => fail … leaving` に直すことになる。そのとき検査は、前提を置かない分だけ多くをエラーにする。
   4. **拒否のエラーは、拒否のときにしか返らない**（`CallStep` の `sendFailed` と `sendFailedAfter` が返すエラーは、拒否のエラーではない）。拒否のエラーとは、タスクの `refused as` か、帳簿が拒否するときの理由（chobo の `reason`）である。現実の仮定としては、外部のサービスがそのエラー（たとえば `unexpected_state`）を返すのはステートマシンがイベントを拒否したときだけで、プラットフォームもタイムアウトや接続の失敗をそのエラーとして返さない、ということである。検査は、拒否のエラーだけを受ける `on` には拒否する状態の組だけを渡し、拒否する状態が無ければその `on` は動かないものとする（dandori の W102「ここでは動きません」と同じ読み方）。この前提を外すと、拒否のエラーだけを受ける `on` にも、失敗した呼び出しが残しうる組が全部渡る。拒否する状態が終わりの状態で、拒否されたら `pass` して終える flow（有効期限の切れた仮押さえの取消など）が、E020 にあたるようになる。
   検査は、案件ごとに「流れが最後に聞いた状態」と「そのときかそれ以後に案件がいた状態」の組を持つ。dandori の検査（`flow.rs`）も同じ組を持つ（dandori の DESIGN 2.1）。前は dandori の検査が一つの集合しか持たず、失敗した呼び出しのあとの `match` が、レコードで選んだ分岐の中で案件がいる状態まで絞っていた。そのため E020 を見落とす flow があり、Lean の担当が見つけ、dproof の担当が直した（PLAN の F.6。例のうちホテルの予約、請求、注文の Temporal 版も同じ見落としを持っていて、直した）。`CallStep` の前提 3 と 4 の読み方に合わせて、検査（`Check.lean`）は、案件を始める呼び出しの失敗では案件を前のまま（始まっていない）とし、`sends` の呼び出しでは、拒否のエラーごとに、その拒否が残す組（拒否する状態にいる組）を分けて持ち（`CallRes.refused`）、`on` ごとに受け取る組を決める（`handlerIn`。拒否のエラーだけを受ける `on` は拒否の組だけ、ほかは失敗の組と、受ける拒否の組）。どの `on` も受けないエラーは `on failure` へ渡る（`raisedIn`）。決められないところ（決めた回数のうちに落ち着かないループ、外部のイベントで移れる先を 64 回たどっても出そろわない machine、始まったと言い切れない案件へのイベント、二度目の `starts`、`for … in parallel`）では検査が通らないので、通れば必ず定理の言うとおりになる。`Examples.lean` は四つの flow でこの検査をビルドのたびに Lean に計算させる（`decide`。小さな flow で 0.3 秒ほど）。売上の確定がタイムアウトしたあとに、レコードの `pending` を見て取消を送る flow と、同じことを `on failure` でする flow（どちらも、拒否を受けて終えるとエラーにし、引き渡すと通す。dandori の `tests/fixtures/capture_timeout.flow` と `capture_timeout_handed_over.flow` と同じ判定）、前提 3 と 4 で通るようになった、支払いを開く呼び出しの失敗を `on failure` の `none => pass` で受ける flow と、有効期限が切れると拒否する取消を `on unexpected_state => pass` で受ける flow である。dandori の検査も、同じ形の `.flow` を通す。

五つのライブラリのどの宣言（定理と、定理と `rulec-recheck` と `ritsu-model` が使う定義の全部。5,333 個、うち定理 2,066 個。X4 を額 0 から数えるようにしたときに二つ増えた）も、Lean そのものが立つ三つの公理（`propext`、`Classical.choice`、`Quot.sound`）のほかに立っていないことを、`crates/ritsu-model/tests/proofs.rs` が `collectAxioms`（`#print axioms` が使うもの）で確かめる。

### 11.3 Rust と Lean の突き合わせ

Lean のモデルを実行できるプログラム `ritsu-model`（`proofs/` の `lake build` が作る）にした。`ritsu-model <言語> <ファイル>` は、テストが書いたファイル（帳簿、日付やカレンダーのファイル、flow を、モデルが読む形に解決したもの）を読み、標準入力の JSON の行の一つ一つに一行で答える。Rust のテストは `crates/ritsu-model` にある（std だけのライブラリと、言語のクレートを dev-dependency として読むテスト）。入力を作り、言語の参照インタプリタとモデルの両方に流し、作りながら一行ずつ比べる。合わなければ、最初の五行の入力と二つの結果を出して落ちる。`tools` の段で走らせる（`lake build` が要る）。

| 言語 | 入力 | 比べるもの | 数（2026-10-04） |
|---|---|---|---|
| chobo | 例とテストのすべての帳簿について、chobo が書くシナリオ（`scenarios::generate`）と、横に手で書いたシナリオ（`<帳簿>.more.json`） | ステップごとの結果と拒否の理由、名指した勘定の残高、仮押さえの状態。`together` は結果の集まりとして | 20 冊、468 本 |
| koyomi | 例とテストの、解決できるすべての `.cal`（主張をわざと破る二つの例も）について、`koyomi vectors` の全部の行（範囲のすべての入力と、そのすぐ外） | 日付と時刻、エラーの種類、カレンダーなら営業日か | 39 ファイル、11,864,129 行 |
| dandori | 例とテストのすべての flow（日付と帳簿を使うものも）について、`dandori scenarios` が書くシナリオ | 通った文・分岐・応答・ハンドラ・イテレーション・ループの終わり・`on failure`・`on cancel`、呼び出しの引数（Temporal から見た呼び出しが引数をそのまま運ぶもの：規則と日付のアクティビティ、タスク自身のアクティビティ、Lambda 関数）、終わり方（出力、またはエラーと理由）、案件のレコードの状態 | 52 本、1,195 本（引数は 137 か所） |
| 言語をまたぐ検査（`cross.rs`） | rulec の答え：前提に `constraint` を持つ規則（四つの比べ方、同じ刻みと違う刻みの組を三つ）に、端の格子が作る範囲のすべての組を尋ねる。呼び出しの場所：規則を呼ぶ小さなプロジェクト（関係：範囲の組 25、同じ値、範囲の無い値。koyomi の日：同じ日付の日、別の日付の日、受領日でもありうる値）を `ritsu check` と同じに確かめ、呼び出しが渡す値は dandori の口（`Flows::crossings`）が言うものを使う。X3 の (a)：koyomi の例とテストの、検査を通るすべての日付のファイルのすべての日付について、koyomi の口が渡す日の集合と、その集合のまわりの範囲（`days_fit`、`days_given`）。X3 の (b)：koyomi の日付を範囲にする規則の証明書の日の並び（rulec のテストの材料、koyomi の例の英語版と日本語版の三つ）。X4：rulec のコーパスのすべての数の出力の `output_values` と、分岐をすべて通るように書いた出力、そのいくつかと範囲を組にした `amounts_given` と `amounts_hull`。chobo の例とテストのすべての帳簿のすべての振替について、検査が見つけた理由と、額を範囲に限った探索の答えを、帳簿の境界の理由と組にした `refusals_met`。X5：秒数の格子（最小、最大、有効期限のまわり）。X6：すべての日付のファイルのすべての入力の `input_range` と、その範囲に koyomi の日付の日を渡す `days_given` | 判定の答え（成り立つ、例とその値、決められない）。日の集合 | 42 回、55,660 行（rulec の答え 12 の規則・49,152 行、呼び出しの場所 119、X3 の (a) 22 ファイル・76 の日付・1,424 行、X3 の (b) 3、X4 の額 2,345、振替の理由 2,061、X5 360、X6 196） |

X4 を額 0 から数えるようにしたあとの回では、X4 の額が 2,058 行、拒否の理由が 2,115 行（額を限った探索の範囲は (300, 800)、(0, 0)、(−5, −1) の三つ）で、どれも Lean のモデルと同じだった。

koyomi の口が答えない日付のファイル、つまり主張をわざと破る例は、X3 と X6 の突き合わせに入らない。

dandori のうち、答えを flow が受け取る前に読み直すところ（Jev の答え、Claude のエージェントの列挙の値、Connect のサービスと flow が実装するサービスの protobuf のゼロ値、Connect のサービスとして呼ぶ規則の答え）は芯の外とし、モデルには、dandori 自身の関数がそれぞれの答えを何として読むか（`render::jev_read`、`apis::fill`、`render::fold_enums`、`render::rule_read`）を、答えの順番と呼ぶ相手ごとに渡す。モデルが確かめるのは、その値で flow が何をするかである。呼び出しの引数は、そのまま運ぶもの（`carrying`）だけを比べ、プラットフォームの形に作り直す呼び出し（HTTP、エージェント、帳簿、Jev など）は比べない。

テストが違いを見つけることは、モデルを一度ずつわざと壊して確かめた。koyomi の 11 月を 31 日にすると最初のファイルで 671 行中 90 行、chobo の下限の比べ方を `<` から `≤` にすると在庫の帳簿で 21 行中 14 行、dandori の `repeat` の終わりの記録を落とすと hotel で 50 行中 2 行、dandori の `now` を決まった時刻にすると請求の 19 行中 17 行、帳簿の `hold` が返す状態を `posted` にすると同じく 17 行が違うと言って落ちた（引数を比べる前は、`now` を壊しても落ちなかった）。日付の形の確かめ（`isDate`）を、どんな文字列も通すように壊しても落ちなかった。形の崩れた日付の文字列を返すシナリオが無いためで、Rust の `render::is_date` も、同じく試されていない。

どれも、テストが作る入力で一致することを確かめるテストであって、Rust の実装がモデルと同じ関数であることの証明ではない。

### 11.4 置き場所

根の `proofs/` は一つの Lake のパッケージで、ライブラリは `RulecCert`、`ChoboModel`、`KoyomiModel`、`DandoriCore`、`RitsuCross`、プログラムは `rulec-recheck`（`RulecMain.lean`）と `ritsu-model`（`Main.lean`）。ツールチェーンは一つ（`lean-toolchain`）で、mathlib は入れない（ビルドを速く保つ）。`RitsuCross` は `RulecCert`・`KoyomiModel`・`ChoboModel` を読む（言語をまたぐ定理は、言語のモデルの上に立つ）。F の段階で、`crates/rulec/proofs/` をここへ移した（`git mv`。rulec の `Main.lean` は、`ritsu-model` の `Main.lean` と名前がぶつかるので `RulecMain.lean` にした）。rulec の `proofs/README.md` は `proofs/RulecCert/README.md` にある。rulec の `tests/lean.rs`・`tests/days.rs`・`tests/machine.rs` は、根の `proofs/` の `rulec-recheck` を使う。rulec の文書（サイト、`docs/`、スキル）が `proofs/` と書くところは、リポジトリの根の `proofs/` として読めるので、直していない（rulec の README の木の一行だけ直した）。`ritsu-model` は二つあり、Lean の実行ファイルと、それを走らせる Rust のクレートで、どちらも `ritsu-model` とした（Cargo のパッケージと Lake の実行ファイルは名前の置き場所が別で、ぶつからない）。

## 12. 元のリポジトリと、履歴の取り込み

### 12.1 元のリポジトリには書かない

`~/rulec`、`~/dandori`、`~/koyomi`、`~/chobo`、`~/geas`、`~/yurai`（yuen の前の名前のまま）、`~/sakai` には、書かない、そこでビルドしない、git の状態を変えない。取り込みは、それぞれを作業場所に `git clone --no-local` でコピーしてから、そのコピーの上で行う。元のリポジトリは、作者が公開の扱いを決めるまで、そのまま残す。

### 12.2 `git filter-repo` で取り込む

```sh
git clone --no-local ~/rulec <作業場所>/import/rulec
git -C <作業場所>/import/rulec filter-repo --to-subdirectory-filter crates/rulec --tag-rename '':'rulec/'
git -C ~/ritsu remote add rulec <作業場所>/import/rulec
git -C ~/ritsu fetch rulec --tags
git -C ~/ritsu merge --allow-unrelated-histories -m "chore: bring rulec into the workspace with its history" rulec/main
git -C ~/ritsu remote remove rulec
```

七つを、この形で一つずつ取り込む。`git filter-repo` を選んだ理由は二つある。

- 過去のどのコミットでも、ファイルが `crates/<名前>/` の下にある形に書き直すので、`git log crates/rulec/src/main.rs` と `git blame` が、取り込む前の履歴までそのままたどれる。`git subtree add` では、取り込む前のコミットのファイルは根に置かれたままで、たどるには `--follow` が要り、それでもたどれないことがある。
- タグの名前を書き換えられる。rulec の `v0.1.0`〜`v0.22.1`（28）と dandori の `v0.1.0` は、そのままでは `v0.1.0` がぶつかる。`rulec/v0.22.1`、`dandori/v0.1.0` の形にする。

コミットの作者と日時は書き直さない（`filter-repo` は元のまま残す）。コミットのハッシュは変わるが、元のリポジトリはそのまま残るので、文書に書かれた古いハッシュ（rulec の DESIGN.md にある）は元のリポジトリで引ける。

取り込みのマージは、新しいコミットを作る。作者の決まり（勤務時間の外にコミットする、日時を偽らない）があるので、`~/ritsu` に対するマージは、指示する側が許した時刻にだけ行う。許しが出るまでは、作業場所のコピーの上で同じ手順を走らせ、ビルドとテストまで済ませておく（PLAN B.2）。

### 12.3 取り込んだあと

取り込んだあとの開発は ritsu で行う。リリースとサイトは ritsu から出す（13.2）。rulec と dandori のサイト（GitHub Pages）、Homebrew の formula、リリースは、F の最後に ritsu へ移すまで、元のリポジトリのものが残る。元のリポジトリそのものの扱い（GitHub で公開している rulec と dandori を、アーカイブにするか、ritsu を指す一文を足すか）は作者が決める。

### 12.4 B で気をつけること

取り込んだだけで、いまのテストの振る舞いが変わるところがある。どれも、中身を直さずに済ませる方法か、直し方を PLAN B に書いた。

- **git のルートが上に移る**：geas、yuen、sakai は、いちばん近い `.git` をルートにする。取り込むと、`crates/<名前>/` には `.git` が無く、ルートは `~/ritsu` になる。12.5 で試した結果、yuen のテストが二つ落ちる。
- **rulec の `.cargo/config.toml`**：`[env]` で `RULEC_LANG=ja` を強いている（rulec のテストは日本語の文を確かめる）。cargo は、走らせたディレクトリとその上の `.cargo/config.toml` だけを読むので、`~/ritsu` の根から `cargo test` すると読まれない。B では、各クレートのテストを `crates/<名前>/` で走らせた。C.0 で、rulec のテストが自分で子プロセスに `RULEC_LANG=ja` を渡し、プロセスの中では日本語を選ぶ形にして、この設定を消した。いまは根からも、どのディレクトリからも同じに回る。
- **dandori が使う rulec のバージョン**：dandori の golden は rulec 0.22.0 で取ってある（57 のファイル）。ワークスペースで作る rulec は 0.22.1 なので、B では 0.22.0 の rulec（rulec のタグ `rulec/v0.22.0` から作業場所で作るか、リリースのバイナリ）を `DANDORI_RULEC` で渡す。
- **gitignore したものは来ない**：各クレートの `tools/` の `node_modules` と venv、Java の jar、TigerBeetle、`go-arch-lint`、ReqIF のスキーマ、rulec の `website/` が `sync.sh` でコピーするページ、dandori の `website/docs-ja/` のコピー、rulec の `proofs/.lake` は、取り込んでも来ない。それぞれのクレートの手順で入れ直す（PLAN B.5）。
- **プロファイル**：ワークスペースの中のクレートの `[profile.*]` は読まれない。koyomi の `[profile.test] opt-level = 2`（1900〜2100 年のすべての日を何度も回すテストのため）は、根の `[profile.test.package.koyomi]` に移す。五つの `[profile.release] strip = true` は根の `[profile.release]` に一つ置く。クレートの中の `[profile.*]` は C.0 で消した。
- **`Cargo.lock`**：根に一つ作る。五つの `Cargo.lock` の依存は同じバージョン（serde_json 1.0.151 ほか 15 のパッケージ）なので、通信せずに作れる。クレートの中の `Cargo.lock` は使われなくなる（C.0 で消した）。

### 12.5 取り込んだ形で走らせてみたこと

2026-10-03 に、作業場所に `sim/.git`（空のディレクトリ）と `sim/crates/<名前>/`（元のリポジトリをコピーして `.git` を消したもの）を作り、B の形（ルートがクレートの二つ上にある形）で yuen、sakai、geas のテストを走らせた。外のツールは渡していない。yuen はこのとき yurai という名前で、下に貼った出力のパスは、そのときのまま（`crates/yurai/`）にしてある。

- **yuen**：92 件が通り、2 件が落ちた。SKIP の行は 3（prov と reqif の venv、ReqIF のスキーマが無いため）。コンパイルを含めて 7 秒ほど。
  - `tests/cli.rs` の `the_json_of_check`：`yuen check tests/fixtures/period --format json` の `root` が `"."` でなく `"../.."` になった。
  - `tests/design.rs` の `every_command_in_design_prints_what_design_shows`：DESIGN.md の `$ yuen check tests/mutants/E302_条が変わった`（`--root` なし）の出力の、コピーのパスが変わった。

    ```
    --- DESIGN.md shows
      what changed in the text (the copy tests/mutants/E302_条が変わった/sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml):
    --- it prints
      what changed in the text (the copy ../../crates/yurai/tests/mutants/E302_条が変わった/sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml):
    ```

    パスはルートからの相対を走らせたディレクトリからの相対に直したもので、間違いではないが、回り道をしている（`../../crates/yurai/` は要らない）。git のリポジトリの下のディレクトリで yuen を走らせれば、取り込まなくても起きる。C.0 で yuen の表示を直した（4.7）。
  - 二つとも、`--root .` を渡せばルートはクレートのディレクトリになり、出力は元のものと同じになった。ただし E302 の直し方の行は、渡したとおり `--root .` を含めて書く（`yuen review tests/mutants/E302_条が変わった --root . --at …`）ので、DESIGN.md のその三行も合わせて直すことになる。
- **sakai**：97 件が全部通った。SKIP の行は 9（ツールが無いため）。コンパイルを含めて 12 秒ほど。ただし `tests/cli.rs` の `check_exit_codes_and_formats` は、テストのディレクトリに `.git` があるときだけ `--root` なしの形を確かめる作りで、B の形ではその部分を黙って飛ばす。
- **geas**：236 件が全部通った。SKIP の行は pixie の二種類が二回ずつ（`GEAS_PIXIE_GREETER` を渡していないため）。geas のテストは、例を一時ディレクトリにコピーして走らせるので、ルートが移っても変わらなかった。コンパイルを含めて 1 分 32 秒。
- rulec、dandori、koyomi、chobo は、`.git` からルートを決めない（`src/` を探して確かめた）。rulec の `@リビジョン` の読み方は、走らせたディレクトリからの相対（`git show <rev>:./<パス>`）なので、`crates/rulec/` の下でもそのまま読める。

## 13. バージョンとリリース

### 13.1 一つのバージョン

ワークスペースのバージョンを一つにし（`[workspace.package] version`）、どのクレートも `version.workspace = true` にする。どのコマンドの `--version` も、生成物の頭も、doc のページも、このバージョンを書く。バージョンは生成物の頭や golden に入っているので、変えるのは F の段階で一度にする（それまでは、各クレートのいまのバージョンのまま）。F.7 で 0.23.0 にそろえた（PLAN の F.7）。

バージョンの番号は rulec の続きにし、ritsu の最初のリリースを 0.23.0 にする。rulec だけがリリースを重ね（タグ 28）、Homebrew の formula を持ち、生成物の頭と、README が案内する CI の書き方（`uses: i2y/rulec@v0.22.1`）にバージョンが入っているからである。ritsu を 0.1.0 から始めると、`rulec --version` が 0.22.1 から 0.1.0 に戻る。言語ごとにバージョンを持ち続ける形は、バージョンを一つにすること（このまとめの目的の一つ）に反し、1.4 のバージョンの食い違いが残る。

そろえ方は、ルートの `Cargo.toml` の `[workspace.package]` に `version = "0.23.0"` を書き、全部の `crates/*/Cargo.toml` の `version = "…"` を `version.workspace = true` にし、`Cargo.lock` を作り直す。そのあと、バージョンを書いている golden（`<名前>_BLESS=1` か `RITSU_BLESS=1` で取り直す。fast の段だけで 111 のファイルが変わることを、作業ツリーのコピーで試した）、出力を貼った文書（DESIGN.md の api の抜粋、koyomi の生成物の頭の抜粋、chobo の `tests/doc/*.html` など。テストが実物と突き合わせて教える）、ブラウザで試すページの `rulec.wasm`・`dandori.wasm`・`ritsu.wasm`（版の文字列を持つ）、dandori のサイトの例のページと `presets.json` を直す。`release.yml` は、タグとワークスペースの版、そして `ritsu` とリンク七つが `--version` で言う版の八つが、全部そろっていなければ止まる。`crates/ritsu/tests/release.rs` の `one_version_for_the_workspace_and_every_crate`（`#[ignore]`）は、そろえたあとに `--ignored` で走らせて確かめる。

### 13.2 配り方

- GitHub のリリース：`release.yml` が、macOS（arm64、x64）と Linux（x64、arm64。musl で静的にリンク）の四つを作る。アーカイブ `ritsu-v<版>-<target>.tar.gz` には、`ritsu` と、それを指す相対のリンク七つ（`rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`yuen`、`sakai`）と、`LICENSE-MIT`、`LICENSE-APACHE`、`THIRD_PARTY_NOTICES` が平らに入る（`packaging/archive.sh`）。入れ方は、`tar -xzf <アーカイブ> -C <PATH にあるディレクトリ>` である（ライセンスの二つと通知も展開される。要らなければ `--exclude 'LICENSE-*' --exclude THIRD_PARTY_NOTICES`）。`SHA256SUMS` に、アーカイブと `.deb` と `.rpm` の SHA-256 を書く。ビルドのあと、`ritsu` とリンクの八つの名前が、タグの版を `--version` で言うことと、七つの言語がそれぞれ例を読み、`ritsu check` がプロジェクトを読むこと（`packaging/smoke.sh`）を確かめてから、アーカイブを残す。
- 配るのは ritsu だけにする。言語ごとのリリース、formula、サイトは作らない。いまの rulec と dandori のサイト（GitHub Pages）とブラウザで試すページ、rulec の Homebrew の formula も、F の最後に ritsu へ移す。
- Homebrew：`i2y/tap/ritsu` を足す。formula は `packaging/homebrew.sh` が、公開した `SHA256SUMS` から書く。`ritsu` を入れて、リンク七つを `bin.install_symlink` で作る。keg には、brew が名前で入れる `LICENSE-MIT` と `LICENSE-APACHE` の横に、formula が `prefix.install` で `THIRD_PARTY_NOTICES` を入れる（brew の `Metafiles.copy?` は、LICENSE や COPYING などの名前（`LICENSE-MIT` のように、`-` や `.` の前がそれのもの）と、NOTICE や README などを keg に入れるが、`THIRD_PARTY_NOTICES` は入れない）。formula の test が三つのファイルを確かめる。`license` は `all_of: [{ any_of: ["MIT", "Apache-2.0"] }, "Unicode-3.0", "BSD-3-Clause"]` を一行に一つずつ書く。brew の style（`brew audit --strict` が走らせる）の FormulaAudit/Licenses が、入れ子の `license` を複数行に分けるよう求めるためである。リリースのあと、macOS と Linux で `brew audit --strict --online`、`brew install`、`brew test` を通してから、tap（`i2y/homebrew-tap`）に push する。push には tap だけに書ける deploy key を使う（secret は `HOMEBREW_TAP_KEY`）。
- サイト：rulec と dandori のサイトの中身（英語と日本語のページ、ブラウザで試すページ）は、ritsu のリポジトリから出すサイトに移した。ritsu のサイトは根の `website/` で、`https://i2y.github.io/ritsu/`（英語）と `/ritsu/ja/`（日本語）に出す。根の設定（`website/zensical.toml` と `zensical.ja.toml`）は ritsu のページ（`index.md` と、ブラウザで試すページの `playground.md`）だけを組む。言語のサイトは、言語のリポジトリにあったときの設定のまま、下のディレクトリに置いた（`website/rulec/` と `website/dandori/`。どちらも `git mv` で移した）。`website/build.sh` が、根のページを英日で組み（英語が `build/` を消してから書くので、いつも最初）、そのあと `sites=(rulec dandori)` に挙げた言語のサイトを、それぞれの `build.sh` で `website/<言語>/build` に組んで、`website/build/<言語>` にコピーする。言語のサイトは `/ritsu/<言語>/` と `/ritsu/<言語>/ja/` に出る（`site_url` と言語の切り替えのリンクをそう直した）。Zensical の `site_dir` は設定のディレクトリの外を指せない（`site_dir must be within project root`）ので、言語のサイトを直接 `website/build/<言語>` に書かせる形は取れず、コピーする形にした。Zensical（0.0.67）は `website/.venv` に一つだけ入れ、言語のサイトの `build.sh` もそれを使う。プレビューは `website/serve.sh`（`/ritsu/` の下で配る。ポートは 8003）で、言語のサイトの `serve.sh` は消した。rulec のサイトは、ページの一部（`AGENTS.md` と `docs/` の五つ）を rulec のクレートからコピーする（`website/rulec/sync.sh`。コピー元は `crates/rulec` のまま）。サイトの図と例のページを作る道具（`website/rulec/tools/`、`website/dandori/tools/`）は、コーパスと例を言語のクレートから読む。移す前の `crates/rulec/website` と `crates/dandori/website` の gitignore のコピーは消した。公開は `.github/workflows/docs.yml` で、`website/build.sh` で組んだ `website/build/` を GitHub Pages に出す。いまは `workflow_dispatch` だけで走る（push では走らない。切り替えると決めたときに、`push: branches: [main]` と `website/**` のパスを足す。rulec のサイトは `crates/rulec/AGENTS.md` と `crates/rulec/docs/**` もコピーするので、そのパスも足す）。ritsu.wasm はリリースのたびに作り直す（バージョンの番号をそろえるときも。13.1）。作者は、ritsu を public にしたらすぐにサイトを切り替えると決め、2026-10-05 に切り替えた。前の二つのブラウザで試すページ（rulec と dandori の、英語と日本語の四つ）は、ritsu のページへの転送に替えた（作者の決め。8.7）。英語のページは `/ritsu/playground/` へ、日本語のページは `/ritsu/ja/playground/` へ送り、来たリンクのハッシュが `=` を含めばそのまま付け、そうでなければ前のページが開いていたもの（dandori は `#flow=tests/fixtures/hotel_naive.flow`、★rulec は `#project=rulec/gap`、日本語は `#project=rulec/gap.ja`）を開く。言語のサイトのナビの「ブラウザで試す」とトップページのボタンは、ritsu のページを直接指す。元のリポジトリのサイトは、パスをそのまま ritsu のサイトの下へ送るページに替える（前のブラウザで試すページへのリンクは `#…` ごと送るので、転送のページを通っても、ritsu のページに直接送っても、同じものが開く）。
- 言語の名前空間のページ：言語が書き出すものの語の IRI を開くと、ritsu のサイトのページが語の説明を見せる。いまは yuen の PROV だけで、名前空間は `https://i2y.github.io/ritsu/ns/yuen#`（作者の決め、2026-10-04。リポジトリの URL `https://github.com/i2y/yuen/ns#` から替えた。yuen を配るのは ritsu だけで、yuen のリポジトリは作らないため）、ページは `website/docs/ns/yuen.md`（日本語は `website/docs-ja/ns/yuen.md`。公開先は `/ritsu/ns/yuen/` と `/ritsu/ja/ns/yuen/`）である。語の IRI は名前空間に語を続けたもの（`…/ns/yuen#Requirement`）で、`#` の前に `/` が無いので、サーバーは `…/ns/yuen` を `…/ns/yuen/` へ 301 で送り、ブラウザは `#Requirement` を持ち越す。GitHub Pages がディレクトリの URL を 301 で `/` 付きへ送ることは、rulec と dandori のサイトの URL で確かめた（`curl` が `Location` に `/` 付きの URL を返す）。ローカルに組んだサイトを Chrome で開き、`#Requirement`・`#inForce`・`#sha256` がその語の見出しに着き、`#inforce` は着かないことも確かめた。id の照合は大文字小文字を区別し、Zensical の既定の id は小文字なので、見出しは `### inForce { #inForce }` と書き（`attr_list`。設定にある）、id を語の綴りそのままにする。★ページは nav に出さない（IRI か、yuen の `docs/reference.md` から行く）。語は 28 で、yuen の `src/export/prov.rs` の `TERMS`（★yuen の公開 API に `Kind`・`Term`・`TERMS` が増えた）と同じでなければならない（テストが確かめる）。★ページに「語の意味は変えない」といった互換の約束は書いていない。IRI は、ritsu のサイトを公開したあとにしか開けない（2026-10-04 に `curl -I https://i2y.github.io/ritsu/ns/yuen` は 404 を返した）。
- サイトのテスト：根のページは `crates/ritsu/tests/website.rs`（10 件）が守る。根のページの相対リンクとアンカーの先があること（ページか、`build.sh` が組む言語のサイト）、ritsu のページと言語のサイトのページの、ritsu のリポジトリへのリンクの先のファイルがあることと、一つの言語のリポジトリを名指さないこと（一つの言語のリポジトリを名指さないこと（そのリポジトリのリリースも含む。言語は ritsu のリリースで配る））、index のコード（返金の規則とフロー）がリポジトリのファイルの行で、その下の `ritsu check .` が見せたとおりに出すこと（`tests/readme.rs` と同じ `tests/common` の関数）、`build.sh` が rulec と dandori のサイトを組むこと、両方の index が `build.sh` の組む言語のサイトにリンクし、そのサイトに `build.sh` と二つの設定があること、設定の `site_url`・`repo_url`・`edit_uri`・`docs_dir`・`site_dir`・言語の切り替え、`docs.yml` が main への push（サイトのもとが変わったとき）と手で走ること。言語のサイトの前のブラウザで試すページ（`playground.md`）が ritsu のページへ送ること（リンクの先と `#…`、スクリプト、ナビとトップページのリンク。8.7）。Zensical が `website/.venv` にあれば、`website/` のコピーで `build.sh` を走らせ（コピーの横に `crates/` へのリンクを置く。rulec の `sync.sh` がそこから読む）、出力の木（根と言語のサイトの英日の index、ritsu のブラウザで試すページ、言語のサイトの転送のページ。転送のページの横には何も無く、そのリンクとサイトのトップのナビが木の ritsu のページに着くこと）と、根の組んだページのリンクの先が木にあることを確かめる（無ければ SKIP。CI の tools の段の dandori の組で Zensical を入れる）。yuen の名前空間のページ（英語と日本語）が、`TERMS` の語を一つずつ、型・関係の種類・属性の節（`## … { #types }` など）の下に、書かれる先（型なら PROV の記録、属性なら型）の書き込みとともに持ち、ほかの語を持たないこと、型の項が持つ属性へのリンクが `TERMS` の属性と同じこと、ページの例（`ritsu yuen export prov` の出力）がコマンドの出力であること。Zensical があれば、組んだ木に `ns/yuen/index.html` と `ja/ns/yuen/index.html` があり、語ごとに id がその語の綴りの要素があること。`pages()` は `docs/` と `docs-ja/` の下のディレクトリも歩くので、ページの相対リンクとアンカー、組んだ HTML のリンクも確かめられる。`tests/common` の `page()` は、`{ #id }` を持つ見出しの anchor を、その id にする。言語のサイトのページは、言語のクレートのテストが守る。テストはクレートに置いたまま、ワークスペースの根の `website/<言語>/` を読む（rulec の `tests/website.rs`・`docs.rs`・`skill.rs`・`m0.rs`・`verify.rs`・`vdiff.rs`・`doc.rs`、dandori の `tests/docs.rs`・`doc.rs`・`skill.rs`）。
- 移したページの中の元のリポジトリへのリンク（`github.com/i2y/rulec/…`、`github.com/i2y/dandori/…`）は、ritsu のリポジトリの `crates/<言語>/…` を指すようにした。入れ方のページは、言語の README と同じく、ritsu のリポジトリから `cargo install --git https://github.com/i2y/ritsu --locked <パッケージ>` で入れる形にした。rulec の入れ方のページと README は、ritsu の最初のリリース（0.23.0）が出たあと（2026-10-05）、Homebrew、`.deb` と `.rpm`、リリースのアーカイブ、CI の action を、ritsu のもの（`i2y/tap/ritsu`、`ritsu_<版>-1_<arch>.deb`・`ritsu-<版>-1.<arch>.rpm`、`ritsu-v<版>-<target>.tar.gz`、`uses: i2y/ritsu@v0.23.0`）に替え、rulec 自身のリリース（0.22.1 まで）から移る人のための節を足した（rulec の DESIGN §15.186）。ページの出力は、0.23.0 のリリースの実物で取り直した。2026-10-05 にサイトを切り替えたとき、言語の README とスキルからサイトへのリンク、rulec の例のページ（とスキルのコピー）から日本語の例のページへのリンク、rulec の `Cargo.toml` の `homepage` を、ritsu のサイト（`https://i2y.github.io/ritsu/rulec/…`、`…/ritsu/dandori/…`）に直した。dandori のブラウザで試すページから rulec のブラウザで試すページへのリンクは、前のページごと無くなった。元のリポジトリのサイト（`https://i2y.github.io/rulec/` と `https://i2y.github.io/dandori/`）は、来たパスをそのまま ritsu のサイトの下へ、クエリと `#…` ごと送るページだけを出す（各リポジトリの `moved/` と `docs.yml`）。ritsu のブラウザで試すページが言語の `doc` のページを呼ぶ言い方は、根の index にそろえて「the page for people」「人が読むページ」にした。
- `.deb` と `.rpm`：`packaging/nfpm.yaml` と `packaging/linux.sh`。`/usr/bin/ritsu` と、リンク七つが入り、`/usr/share/doc/ritsu/` にライセンスの二つと `THIRD_PARTY_NOTICES` が入る。`packaging/linux.sh` は、入れる前に、パッケージのファイルの一覧（`dpkg-deb -c`、`rpm -qlp`）に三つがあることを確かめる。Debian の slim のイメージは `/usr/share/doc` を入れないので、入れたあとのファイルでは確かめられない。rpm の License のタグは `(MIT OR Apache-2.0) AND Unicode-3.0 AND BSD-3-Clause` になる（deb の control には nfpm がライセンスを書かない）。依存は無い。`rulec` のパッケージを置き換える（deb は Replaces・Conflicts・Provides、rpm は Obsoletes・Conflicts・Provides）。Debian と Fedora のコンテナに、ネットワークなしで入れ、七つの名前と `ritsu check` が動くことを確かめ、消してから残す。
- GitHub Actions：根の `action.yml`。`uses: i2y/ritsu@v0.23.0` で、そのタグのアーカイブを `SHA256SUMS` と突き合わせて入れ、`ritsu` とリンクのあるディレクトリを PATH に足す。入力 `version` と `sha256` は、rulec の 0.22.1 までの `action.yml` と同じである（クレートの中の `action.yml` は、0.23.0 のリリースのあとに消した）。
- crates.io：いまは出さない。`koyomi` の名前は別のクレートが使っていて（2.2）、`ritsu` を crates.io に出すには、それが依存する中のクレートを全部出すことになる。出すなら、中のクレートを `ritsu-` で始まる名前にする。
- `Cargo.toml` の `repository`：言語の七つのクレートを `repository.workspace = true`（ワークスペースの `https://github.com/i2y/ritsu`）にした（作者の指示。F.7）。rulec の `homepage` は、公開している `https://i2y.github.io/rulec/` を指すので、サイトを切り替えるまで残した。
- rulec の古いリリースのファイル：rulec だけのリリースを作っていた `crates/rulec/.github/workflows/release.yml`、`ci.yml`、`crates/rulec/packaging/`、`crates/rulec/action.yml` は、ritsu の最初のリリースのあと（2026-10-05）に消した（10.5）。rulec の `Cargo.toml` の `[package.metadata.binstall]` も外した。`pkg-url` は ritsu のリポジトリの `rulec-v<版>-<target>.tar.gz` を指していてその名前のアーカイブは無く、ritsu のアーカイブの `rulec` は `ritsu` へのリンクなので、それだけを取り出しても動かないからである（crates.io にも出さない）。rulec の `experiments/library/` の CI の見本（規則のライブラリを外のリポジトリに出したときに走らせるもの）は、`uses: i2y/ritsu@v0.23.0` にした（rulec の DESIGN §15.186）。
- 他者のものの通知（2026-10-06）：`ritsu` のバイナリと `ritsu.wasm` には、ritsu のコードのほかに他者のものが入る。rulec の Unicode CLDR 48.2 の区分の名前（Unicode-3.0。5.6）、koyomi の WHATWG の索引から作った Shift_JIS の変換表（BSD-3-Clause）、crates.io のクレート 8 個（equivalent 1.0.2、hashbrown 0.17.1、indexmap 2.14.2、itoa 1.0.18、memchr 2.8.3、serde_core 1.0.229、serde_json 1.0.151、zmij 1.0.23。六つが MIT OR Apache-2.0、memchr は Unlicense OR MIT、zmij は MIT）、zmij が一行ずつ移植した Victor Zverovich の C++ の実装（MIT）、それに `koyomi explain` と `yuen explain` の例のための法令のコピー（e-Gov の民法 142 条は PDL1.0、eCFR の 37 CFR 1.7 と 29 CFR 1910.157 は米国政府の著作物）である。根の `THIRD_PARTY_NOTICES`（英語）が、それぞれの出どころ、バージョン、ライセンスの文を並べる。CLDR の節は `crates/rulec/THIRD_PARTY_NOTICES` の文、WHATWG の節は `crates/koyomi/THIRD_PARTY_NOTICES.md` の最後の節の文そのものである。クレートの節は、各クレートの `LICENSE-MIT` そのものである。8 個とも MIT を選べるので、選べるものは MIT で使う。itoa、serde_core、serde_json、zmij の `LICENSE-MIT` には著作権の行が無いので、クレートの `Cargo.toml` の `authors` を節の頭に書いた。MIT の文は、バイナリで配るときにも著作権の表示と許諾の文を付けることを求めるので、配るもののどれにもこのファイルを入れる（上の三つの箇条）。スキルの zip は他者のものを含まないので、ライセンスの二つのままにした。ブラウザで試すページ（英日）は、終わりの一段落で、`ritsu.wasm` が含むものと、この通知へのリンクを書く。ページが読み込む `projects.json` には dandori の例の API の記述（Stripe の OpenAPI の文書と、Amazon SNS・SQS の Smithy のモデルから、例が呼ぶところだけを残したコピー）が入るので、その段落で dandori の `THIRD_PARTY_NOTICES.md` にもリンクした。テスト（`crates/ritsu/tests/release.rs`）が確かめるのは次のことである。通知の名前・バージョン・ライセンスが `cargo tree -p ritsu -e normal`（手元と、リリースの四つのプラットフォーム、`ritsu-wasm` の手元と wasm32）と同じこと。クレートの文が、`cargo metadata` の指す各クレートの `LICENSE-MIT` と一字も違わないこと（出どころ、リポジトリ、作者の行も各クレートの `Cargo.toml` と同じこと）。CLDR と WHATWG の節が二つのクレートの通知と同じこと。zmij の README が同じ元の実装（同じコミット）を指していること。頭の一覧が節と同じ順に並ぶこと。アーカイブ、`.deb`・`.rpm`、formula が三つのファイルを持つこと。`license` の式がそろうこと（2.3）。`include_bytes!` で入る法令のコピーが通知の一覧と同じこと。

**`i2y/tap/rulec` を入れている人の移り方。** tap の `Formula/rulec.rb` を消し、tap の根の `formula_renames.json` に `"rulec": "ritsu"` を足す。同じコミットで、`Formula/ritsu.rb` もそこにある状態にする。Homebrew は formula の名前の変更として扱い、`rulec` の名前で入っているものを `ritsu` に移し、新しい版に上げる。ritsu のアーカイブの `rulec` はリンクなので、移ったあとも `rulec` のコマンドは同じに動く（`rulec --version` は `rulec 0.23.0`）。Homebrew 7 は、第三者の tap の formula を読むのに信頼（`brew trust`）を求めるので、移る人は `brew install i2y/tap/ritsu`（名前を指せば自動で信頼される）のあと `brew migrate ritsu` と `brew upgrade ritsu` を走らせるか、`brew trust --formula i2y/tap/ritsu` のあと `brew upgrade` を走らせる。名前の変更を置く前に `ritsu` の formula だけを足すと、`rulec` が入っているところでは `bin/rulec` のリンクが重なって、ritsu はリンクされないまま入るので、二つは同じコミットでする。formula を ritsu のアーカイブを入れるものに替える案は、`ritsu` の formula と `rulec` の formula が同じ `bin/ritsu` を持ち合うので捨てた。formula を消して README で案内する案は、すでに入れている人が `brew upgrade` で更新を受けられなくなるので捨てた。切り替えは、最初のリリース（0.23.0、2026-10-05）と同じときにした（作者の決め）。tap には、`release.yml` が `Formula/ritsu.rb` を足したコミットと、`Formula/rulec.rb` を消して `formula_renames.json` を足したコミットが、40 秒の間をおいて二つ入った（同じコミットではない。その 40 秒のあいだに `brew update` した人は、上の重なりに当たりうる）。rulec の入れ方のページの「From rulec's own releases」「rulec 自身のリリースから移る」の節が、移り方を案内する。二つの道（`brew install i2y/tap/ritsu`・`brew migrate ritsu`・`brew upgrade ritsu` と、`brew trust --formula i2y/tap/ritsu`・`brew upgrade`）は、作者の Homebrew とは別の使い捨ての Homebrew 7.0.6 で、rulec 0.22.1 を入れた状態から走らせて確かめた（どちらも `rulec --version` が `rulec 0.23.0` になる。rulec の DESIGN §15.186）。古い名前の `brew install i2y/tap/rulec` は、formula を信頼する前は `Refusing to load formula i2y/tap/ritsu from untrusted tap i2y/tap` と言って止まる（名前を変えた先の formula は、古い名前で指しても自動では信頼されない）。`brew trust --formula i2y/tap/ritsu` のあとなら、古い名前でも ritsu が入る。

**deb と rpm を入れている人**は、`apt install ./ritsu_<版>-1_<arch>.deb` と `dnf install ./ritsu-<版>-1.<arch>.rpm` で、`rulec` のパッケージが取り除かれ、`/usr/bin/rulec` が `ritsu` を指すリンクになる（rulec 0.22.1 のパッケージで確かめた）。**GitHub Action を使っている人**は、`uses: i2y/rulec@v0.22.1` が、元の rulec のリポジトリが残るあいだはそのまま動く。新しい書き方は `uses: i2y/ritsu@v0.23.0` で、`rulec` のコマンドは PATH に入る。

言語ごとの README の入れ方は、ritsu から入れる形にそろえた（F.3。yuen と sakai は F.1 と F.2）。`cargo install --git https://github.com/i2y/ritsu --locked ritsu`（全部）と、`--locked <言語>`（その言語だけ。rulec、koyomi、chobo、geas は、ほかの言語を読まない）。dandori は、`--locked dandori` で入る `dandori` だけでは、規則、日付のファイル、帳簿を使うワークフローを走らせられず、`ritsu dandori` を使うよう言う。言語ごとの旧リポジトリの URL は書かない。rulec の README の入れ方は、ritsu の最初のリリースのあと（2026-10-05）、Homebrew（`brew install i2y/tap/ritsu`）、`.deb` と `.rpm`、リリースのアーカイブ、GitHub Actions（`uses: i2y/ritsu@v0.23.0`）を ritsu のものにし、rulec 自身のリリースからの移り方を一文で書いて、入れ方のページの節にリンクした。根の README とサイトのトップページの「リリースを出す予定」も、いまの事実に直した。

**Agent Skills の配り方（PLAN F.3、2026-10-05）。** ritsu と七つの言語は AI エージェントに使ってもらうためのものなので、根の `skills/` の八つの Agent Skills（`ritsu`、`rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`yuen`、`sakai`）を、四つの入れ方で配る。どの入れ方でも、入るファイルは同じである。案内は根の `skills/README.md` と、日本語の版の `skills/README.ja.md`（八つの一覧、四つの入れ方、スキルにコマンドを毎回確かめずに走らせてもらう権限の書き方）と、根の README の「For AI agents」「AI エージェント向け」にある。

- **Claude Code のプラグイン。** マーケットプレイスは、ritsu のサイトが公開する一つのファイル `marketplace.json`（`website/docs/marketplace.json`。公開先は `https://i2y.github.io/ritsu/marketplace.json`）で、プラグイン `ritsu` を一つだけ持つ。プラグインの `source` は `git-subdir` で、ritsu のリポジトリ（`https://github.com/i2y/ritsu.git`）の `skills/` だけを指す。Claude Code は、このフォルダーだけを sparse checkout で取ってくる。`ref` は付けないので既定のブランチを追い、新しいコピーを取るかどうかは項目の `version` が決める。`skills/` には plugin.json を置かず、マーケットプレイスの項目がそのまま manifest になる。項目に書くのは、名前、説明、バージョン（ワークスペースと同じ 0.23.0）、作者（`i2y`。メールは書かない）、ホームページ（ritsu のサイト）、リポジトリ、ライセンス、キーワードと、`skills: ["./"]`（`skills/` のすぐ下の八つのフォルダーがスキル）である。八つは `ritsu:ritsu`、`ritsu:rulec` … として読まれる。入れ方は `/plugin marketplace add https://i2y.github.io/ritsu/marketplace.json` と `/plugin install ritsu@ritsu` で、リポジトリを public にしてサイトを切り替えてから働く。リポジトリの URL は `owner/repo` ではなく https で書く。`owner/repo` だと Claude Code は先に SSH を試し、GitHub の鍵の無い人では失敗するからである。

  はじめは、リポジトリのルートをマーケットプレイスにしていた（`.claude-plugin/marketplace.json` で、プラグインの `source` は `"./"`）。この形では、マーケットプレイスを足すときに Claude Code がリポジトリ全体を clone する（6,473 ファイル、ディスク上で 120〜160 MB）。プラグインを入れるときには、プラグインのフォルダー、つまりリポジトリ全体を、もう一度キャッシュへコピーする（約 100 MB）。要るのは `skills/` の 47 ファイル（1.1 MB）だけである。さらに、新しいコミットがあれば更新のたびに clone し直し、Claude Code の文書によれば、大きなリポジトリの clone は 120 秒で時間切れになることがある。マーケットプレイスを別の小さなリポジトリに置けば、入れ方を `owner/repo` の短い形にできる。それでもサイトに置いたのは、リポジトリを一つにまとめた方針に合わせるためである。

  確かめたこと：`claude plugin validate --strict`（Claude Code 2.1.289）が `website/docs/marketplace.json` を通す。作者の `~/.claude` に触れないよう、`HOME` と `CLAUDE_CONFIG_DIR` を使い捨てのディレクトリにして、同じ `marketplace.json`（プラグインの取り先だけを手元のリポジトリの `file://` の URL に替えたもの）を入れた。ファイルとして足す形と、手元の HTTP サーバーの URL から足す形の両方で入れ、どちらも `claude plugin details ritsu` が八つのスキルを並べた。手元に残ったのは、`marketplace.json`（4 KB）とスキルの 47 ファイル（1.1 MB）だけである。`tests/skill.rs` が、`marketplace.json` の中身と、四つの README の入れ方の文を確かめる。`tests/website.rs` は、サイトの組み立てで `marketplace.json` がそのまま公開されることを確かめる。
- **`ritsu skills install`。** 八つのスキルを ritsu のバイナリに埋め込み（`crates/ritsu/src/skills.rs`。`include_str!` で 45 ファイル、1.1 MB）、`<dir>/<name>/` に書く。既定は今いるディレクトリの `.claude/skills/`、`--user` で `~/.claude/skills/`、`--dir <dir>` でほかのエージェントがスキルを読む場所に書く。名前を挙げると（`ritsu skills install rulec dandori`）、そのスキルだけを書く。すでにあって中身が同じファイルはそのままにする。ritsu の持つものと中身が違うファイル（手で変えたもの、または別の版の ritsu が書いたもの）が一つでもあれば、何も書かずに exit 1 で止まり、違うファイルを並べる。`--force` を付けると上書きする。ritsu が書かないファイルには触らない。`ritsu skills list` は八つの名前と一行の説明を出す（英語と日本語）。geas の `geas skill --install` と同じ考えだが、geas がフォルダーがあるだけで上書きせずに止まるのに対し、ritsu はファイルごとに中身を比べる。同じ版を入れ直しても止まらず、手で変えたファイルだけが止める。ブラウザで試すページ（ritsu-wasm）には入れていない。
- **フォルダーをコピーする。** リポジトリのクローンから `skills/<name>` を `~/.claude/skills/` かプロジェクトの `.claude/skills/` にコピーする。
- **リリースの zip。** `packaging/skills.sh` が `ritsu-skills-v<版>.zip` を書く。八つのフォルダーを zip の一番上に置き、横に LICENSE-MIT と LICENSE-APACHE を置く（アーカイブの tar.gz と同じ）。スキルはプラットフォームによらないので、`release.yml` の `publish` のジョブがタグから一度だけ作り、八つの `SKILL.md` があることを確かめてから `SHA256SUMS` に載せ、リリースに付ける。入れ方は `unzip ritsu-skills-v0.23.0.zip -d ~/.claude/skills -x 'LICENSE-*'` である。

テストは `crates/ritsu/tests/skill.rs` に 9 件を足して 15 件にし、`tests/release.rs` の一件に zip の行を足した。確かめることは次のとおり。`skills/` の下が八つのフォルダーだけで、それぞれの `SKILL.md` の frontmatter の `name` がフォルダーの名前、`description` が 1,024 字以内、`license` がワークスペースと同じであること。サイトの `marketplace.json` が決まったことを書き、バージョンがワークスペースと同じで、プラグインが `skills/` だけであること。四つの README の入れ方の文が、その `marketplace.json` を足すこと。埋め込む一覧が `skills/` の全ファイルと一致すること（足し忘れを落とす）。`ritsu skills install` を一時ディレクトリで走らせて、八つが一字も違わず書かれ、二度目は何も書かないこと。手で変えたファイルがあれば止まって何も書かず、`--force` で上書きし、プロジェクトのファイルは残すこと。名前を挙げたものだけが、`--dir` と `--user` の先に入ること。`ritsu skills list` の名前と順。zip の中身が `skills/` と二つのライセンスに一字も違わないこと。`skills/README.md` と `skills/README.ja.md` に載せた `ritsu skills list` の出力（英語と日本語）と相対リンク。

スキルの中の、言語のサイトへのリンク（rulec のスキルの最後の表と AGENTS.md の 7 章、dandori のスキルの最後の行）も、切り替えまでは公開しているほうを指したままにした。入れ方の URL（rulec の `compatibility` にあった `https://i2y.github.io/rulec/install/`）は、スキルの入れ方として案内しないので外した。ritsu のスキルのブラウザで試すページへのリンク（`https://i2y.github.io/ritsu/playground/`）は、ほかに公開しているものが無いので、切り替えのあとの URL で書いた（切り替えまでは 404）。

リリースと push は、作者の指示があるときだけ行う。

- リリースは、ビルドの前に `audit.yml` を呼び（`release.yml` の `audit` のジョブ。`build` は `needs: audit`）、通すと書いていない既知の脆弱性があれば何も作らない（2026-10-06）。ritsu のバイナリが含むクレートと、`ritsu gen` がパッケージに書くバージョン（9.3）の両方を、その日のアドバイザリで確かめるためである。
- バイナリは cargo-auditable 0.7.7 でビルドし、含むクレートの一覧をバイナリに入れる（3.6）。cargo-auditable は、ビルドする機械（ubuntu の x64 と arm64、macOS の x64 と arm64）のリリースのアーカイブを、決めた SHA-256 で確かめて使う。ビルドのあと、`readelf -S`（Linux）か `otool -l`（macOS）で `.dep-v0` のセクションがあることを確かめてから、アーカイブを作る。リリースに SBOM のファイル（CycloneDX）を別に付けることはしない。スキャナーはバイナリから読み、要る人は cargo-auditable の `auditable2cdx` でバイナリから作れるからである。


## 14. 捨てた形

- **一つの言語にまとめる**：検査は、それぞれの狭さの上に立っている（rulec の自分の列への単項テスト、koyomi の一つの日付と有限の範囲、chobo の勘定の上限と下限、dandori の比較も計算もしない式、yuen のつながりとハッシュと期間、sakai の確かめられる部分）。混ぜれば崩れ、節で分ければファイルが一つになるだけである。
- **一つのファイルに言語ごとの節を並べる**：書く人も、ページを読んで理解し確かめる人も、言語ごとに違う。そのページも、差分の読み方も、ファイルの単位で分かれている。
- **リポジトリを分けたまま、土台だけを公開するクレートにする**：診断や出典の重なりは消えるが、境目は JSON のまま残り、バージョンの食い違い（1.4）も残る。このまとめの目的（境目で証明を切らない）に届かない。
- **一つのリポジトリに入れ、プロセスの境目は残す**：同じ理由。境目の問い（7 章）が子プロセスと JSON の往復になる。
- **生成器の共通の中間表現**：一般のプログラムの表現になり、rulec の表の一行が一つの分岐になる読みやすさや、koyomi の操作ごとの関数が消える。共通にするのは表面にかかわる部分（9.2）まで。
- **`git subtree` で取り込む**：12.2。
- **Lean 4 で処理系を書く**：生成器、診断、CLI、テストの共通部分まで Lean で書く利点が無く、ビルドとツールの手間が増える。意味の中心部分だけを Lean のモデルにし、Rust と突き合わせる（11 章）。
- **言語のコマンドを `ritsu <言語>` だけにして、`rulec` などの名前を捨てる**：rulec は Homebrew と GitHub Actions で配られ、README とサイトがその名前で入れ方と CI の書き方を案内している。名前を残すのはリンク一つで済む。
- **土台を serde_json に依存させる**：4.9。
- **yuen と sakai が、ほかの言語のファイルを自分で読み解く**：二つの読み手が同じ言語の構文を持つことになる（sakai の DESIGN 4.7 の C と同じ理由）。口を通して読む。
- **言語ごとの診断のコードを一つの番号に振り直す**：rulec の診断のコードは、rulec の docs/compatibility.md が 1.0 から保つと書いているものである。番号は言語ごとのまま残し、`ritsu check` の中でだけツールの語を添える（8.3）。

- **生成したパッケージに SBOM を書く**：9.3。
- **CI で cargo-audit も走らせる、osv-scanner の依存の解決に任せる**：3.6。
- **エントロピー（文字のばらつき）で、決まった形の無い鍵を探す**：ハッシュや ID や出典の固定に当たる。W901 は、プロバイダーが形を決めている鍵だけを探す（16.3）。
- **地図の外の相手（モデルのプロバイダー、Jev）へ秘密の値を送ることも、言語をまたぐ検査で言う**：地図が要らない判定まで `ritsu check` だけのものになる。dandori の E906 にした（16.8）。


## 15. まだやらないこと

どれも、作る理由が見えたら作る。ここに書くのは、黙って消えたように見せないためである。

- **`ritsu index`**：プロジェクトの中のものと参照を一つの JSON で出すコマンド（8.5）。
- **`ritsu doc`**：プロジェクトのページ（各言語のページへのリンクと、言語をまたぐ検査の結果）。
- **rulec のすべての文を `Text` に移すこと**（4.1）。
- **実行をまたぐキャッシュ**（6.1）。
- **crates.io**（13.2）。
- **Windows**：geas の DESIGN 15 章と同じ理由（プロセスとサービスの止め方が Unix の振る舞いに立っている）。
- **スキルを一つにまとめること**：各言語のスキルは残し、ritsu のスキル（F）は、プロジェクトを `ritsu check` で回す流れと、どの言語のスキルを読むかを書く。
- **geas の DESIGN.md を日本語にすること**：geas の DESIGN.md は英語のままにする（geas を作ったときの決め）。
- **LSP（`ritsu lsp`）**：今回は作らない（作者の決め、2026-10-04。PLAN の F.4）。3.2 と 6.4 が LSP に触れるところは、作るときの手がかりとして残す。
- **`ritsu run` で、子のフロー（`flow "…"` のタスク）を、子の `.flow` を同じように流して結果を出すこと**：いまは、ほかのタスクと同じくシナリオの結果を使う。
- **`ritsu run` の時間で、並列のイテレーションの待ちを重ねて数えること**：参照インタプリタはイテレーションを一つずつ回すので、いまはイテレーションの中の待ちが足し合わさる。
- **`ritsu run` の入力を、rulec の `vectors` から選ぶこと**（7.9 が書いた使い方）：いまはシナリオを人が書く。
- **`ritsu gen --sbom`**：直接の依存と、生成したファイル（頭のハッシュ）を部品にした CycloneDX 1.7（9.3）。
- **言語としての、セキュリティの検査**は 16 章（2026-10-06 に作った）。その中でまだやらないことは 16.12 にある。


## 16. 言語としての、セキュリティの検査（2026-10-06）

ritsu を使う人のプロジェクトの成果物から見える、セキュリティの誤りを言う。成果物とは、七つの言語のファイルと、プロジェクトが読む契約の文書（`.proto`、OpenAPI、AsyncAPI、JSON Schema、Smithy）である。ritsu 自身の依存の監査（3.6）とは別のもので、使う人のプロジェクトの依存の脆弱性（OSV などのデータベース）は調べない。どの検査もネットワークを使わず、実行時のことは見ない。

確かめるのは次の五つである。

1. 鍵の形の値を、ファイルに直に書いていること（W901）。
2. このマシンの外（ループバックでない相手）へ、暗号化しない通信をすること（W902）。
3. コンテキストが公開する OpenAPI の操作と AsyncAPI のチャネルに、認証の指定が無いこと（W903）。
4. 契約が秘密と印を付けた値が、ワークフローの入力・出力・タスクの引数や結果として、プラットフォームの履歴に残ること（W904）。
5. 秘密の値を、外のサービス（モデルのプロバイダー、Jev、URL だけで書いた相手、AWS のサービス）へ送ること（dandori の E906）、地図の外や、地図の上で印を付けたコンテキストと関係の無いコンテキストへ送ること（ritsu の E905・W905。言語をまたぐ検査の X14）。

**前からあるもの（2026-10-06 の朝に作った）。** 検査を通ったファイルの文字列が、生成したコードのコメントや文字列の外に出ないこと（9.2）。ritsu の言語は、人が読んで確かめるのはソースのファイル（`.rule`、`.flow` など）で、生成したコードは「DO NOT EDIT」として読まない、という前提に立つ。その前提のもとでは、ソースの文字列が生成したコードの中でコードになることは、検査の抜け穴になる。次の四つが見つかり、直した（9.2、dandori の DESIGN 4.7、rulec の §15.187）。

1. dandori のワークフローの `description` に `\n` を書くと、生成した TypeScript、Python、Go のコメントの外に、続きがコードの行として出た。TypeScript では、モジュールを読み込んだときに動く文になる。Argo の YAML では、`---` と別の文書（Pod）を書けて、`kubectl apply -f` がそれも作る。`dandori check` は通していた。
2. ファイルの名前に改行があると、すべての生成器の頭の `Source:` の行の続きが、コードの行になった。
3. rulec の出典の URL とパス（`source … = file "…" url "…"`、`applies`）に U+2028 や `\r` を書くと、TypeScript や Python の頭のコメントの外に出た。
4. rulec が `rulec gen` で書くページ（`<別名>_page.html`）は、生成した JavaScript を頭ごと `<script>` に埋め込む。出典の URL の `</script>` で要素が閉じ、続きが HTML として読まれて、ページを開いた人のブラウザでスクリプトが動いた。

Python と Go は、ファイルの先頭の `from __future__` や `package` より前に文が来るので、多くは構文の誤りで止まる。TypeScript と YAML と HTML では、そのまま動いた。

同じ日の昼に、残りの書き出しも同じ形のテスト（行を終える五つの文字と、`</script>`、`---`、`]]>` を入れた材料）で確かめた。

- sakai：CML の持ち主のコメントと、`build` の設定の頭の地図のパスを一行に収め、`doc` の Markdown の `\r` を空白にした（9.2、sakai の DESIGN 16.7）。HTML のページと `api` は、もとからエスケープしていた。
- yuen の書き出し（ReqIF、PROV-N、PROV-JSON）と geas の下書き（`geas scenarios --draft`）：直すところは無く、確かめるテストだけを足した（`crates/yuen/tests/export_text.rs`、`crates/geas/tests/draft_text.rs`）。ReqIF は `&`・`<`・`>`・`\r` を文字参照にし、PROV-N は文字列の `"`・`\`・`\n`・`\r`・`\t` をエスケープしている。geas の下書きは、要件とシナリオの文を `#` のコメントに入れる。geas の字句は `\n` でしかコメントを終えないので、ほかの四つの文字はコメントの中にとどまる。
- rulec の Java の生成物：Java はコメントの中でも Unicode のエスケープを読むので、頭の行を終える文字の書き方を替え、頭と本文のコメントの `\` を二つにし、記録の JSON のキーを JSON の文字列から作るようにした（9.2）。
- `dandori doc`・`rulec doc`・`sakai doc` の Markdown：HTML として読まれうる `<`、つまり `<` のあとが英字・`/`・`!`・`?` のもの（タグ、閉じタグ、コメント、宣言、処理命令、自動リンクの始まり）だけを、コードスパンの外で `&lt;` にする。CommonMark と Python-Markdown（Zensical のもの）が生の HTML として読むのは、この形で始まるものだけである。`>` と `&` はそのまま書くので、`<=60cm`、`a > b`、`R&D` のような普通の文は、Markdown の元の文のまま読める（`&amp;` や `&gt;` が混ざらない）。コードスパンの中は、Markdown が文字参照を解かないので、手を付けない。dandori はワークフローの説明と `fail` の理由（終わり方の表のセル）を、rulec は規則の説明（人が読むページと、顧客向けのページ）と、準用する規則の説明を、これに通す。HTML の版は、どちらも前から全部をエスケープしている。いまの例とテストの材料には、説明に `<` を書いたものが無いので、生成物は一バイトも変わらない（`crates/dandori/tests/doc_text.rs`）。

```
description "Tells the customer, a > b and R&D. <img src=x onerror=alert(1)> <!-- c --> and `<b>` stay as they read"
```

は、`dandori doc` の Markdown の説明の段落で次のようになる。

```
Tells the customer, a > b and R&D. &lt;img src=x onerror=alert(1)> &lt;!-- c --> and `<b>` stay as they read
```

### 16.1 置き場所と、単体で使う人に届くか

| 検査 | 置き場所 | `dandori check`・`sakai check` などを単体で使う人に |
|---|---|---|
| W901（鍵） | 検出は ritsu-base の `secrets`。言語のファイルは各言語の `check` が自分のファイルを調べる。契約の文書は ritsu-cross が調べる | 言語のファイルは届く。契約の文書は `ritsu check` だけ |
| W902（平文） | dandori の `check`（タスクが実際に呼ぶ URL）と sakai の `check`（地図の文書のサーバー）。判定は ritsu-base の `urls` | 届く |
| W903（認証） | sakai の `check` | 届く |
| W904（履歴） | dandori の `check` | 届く |
| E906（外のサービスへ） | dandori の `check` | 届く |
| E905・W905（地図） | ritsu-cross（X14） | `ritsu check` だけ |

**理由**：ファイル一つ、言語一つで決まることは、その言語の検査に置く。単体で使う人にも届き、`ritsu check` では言語の出力をそのまま並べる（8.3）ので、同じことが二度出ない。言語をまたいで初めて決まるのは、送る値の印が付いたコンテキストと、宛先のコンテキストの関係だけである。地図は sakai が、送ることは dandori が知っているので、ここだけを ritsu-cross に置く。

契約の文書の鍵を ritsu-cross に置いたのは、文書がどの言語のソースでもないからである。同じ `.proto` を rulec と dandori と sakai が読むので、読む言語ごとに調べると、`ritsu check` で同じ鍵が三度出る。ritsu-cross は、プロジェクトの `.proto` を E101 で読むのと同じく、ファイルごとに一度だけ調べる。だから、単体の `dandori check` は `use proto` や `use openapi` の文書の鍵を言わない。単体で使う人にも届けるには、文書を読む言語が出し、`ritsu check` で重なるものを一つにする仕組みが要るが、`ritsu check` は言語の出力を読み直さない（8.3）。契約の文書の W901 は、`ritsu check` だけに出すと決めた。

### 16.2 コード

| コード | 重さ | 台帳 | いつ出るか |
|---|---|---|---|
| W901 | 警告 | 七つの言語、ritsu | 鍵の形の値が、ファイル（契約の文書を含む）に書いてある |
| W902 | 警告 | dandori、sakai | このマシンの外へ、暗号化しない通信をする（dandori はタスクが呼ぶ URL、sakai は文書のサーバー） |
| W903 | 警告 | sakai | 公表された言語の OpenAPI の操作、または AsyncAPI のチャネルが使うサーバーに、認証の指定が無い |
| W904 | 警告 | dandori | 秘密の値が、ワークフローの入力・出力、呼び出しの引数や結果、`fail` の理由として、プラットフォームの履歴に残る |
| E905 | エラー | ritsu | フローが、秘密の値を、地図の外のファイルか、印を付けたコンテキストと関係の無いコンテキストの成果物へ送る（X14） |
| W905 | 警告 | ritsu | フローが秘密の値を送る先が地図のどこかを、決められない（X14） |
| E906 | エラー | dandori | 秘密の値を、プロジェクトの外の相手（モデルのプロバイダー、Jev、URL だけで書いた相手、AWS のサービス）へ送る |

**9xx の帯にしたこと。** どの台帳でも空いている帯で、セキュリティの検査をまとめて置く。一つの番号は一つの検査を指し、言語が違っても同じことを言う（`warning[rulec W901]` も `warning[dandori W901]` も鍵のこと）。使う人は、言語ごとの台帳を引かずに、番号で何の検査かが分かる。ritsu の X14 を、ほかの X と同じ E2xx にしなかったのも同じ理由である（E905 は、dandori の E906 と組で読む）。そのため、ritsu の台帳の帯の決まり（E1xx はファイル、E2xx は境目。7.1）に、9xx はセキュリティの検査、を足した。

**警告とエラー。** W901〜W904 は、形から推すものである。鍵の形をしていてもテスト用の値のことがあり、平文でもサービスメッシュが守っていることがあり、認証を文書に書かずにゲートウェイで課すことがあり、履歴をカスタマー管理の鍵で暗号化していることがある。だから警告にして、意図を書けば消えるようにする（16.3〜16.7）。E905 と E906 は、書いたもの同士の食い違いである。契約を書いた人が「この値は秘密」と書き、フローを書いた人が意図を書かずにそれを外へ送っている。dandori の E014 が、範囲を書いた場所に範囲を外れうる値を渡すとエラーにするのと同じ考えで、エラーにする。

エラーにすると、使う人が契約に印を足した日から、その値を送るフローは `dandori check` か `ritsu check` を通らなくなる。警告にすれば印を足すことは気軽になるが、送っていることに気づかないままになりうる。印は「この値を外へ出さない」という宣言なので、宣言を足したら、それを破る呼び出しを止めるほうを選んだ。取り込んだときに印を持つのは、新しい例 `payout`（どの検査も何も言わない形。16.11）と検査の材料だけで、前からある例とテストのプロジェクトは止まらない。

### 16.3 鍵の形の値（W901）

**検出の決まり**（ritsu-base の `secrets`。4.19）。gitleaks の既定の規則と、GitHub の secret scanning が検出する種類から、プロバイダーが接頭辞や形を決めていて、誤って当たることの少ないものを選んだ。正規表現のクレートは使わない（土台は std だけ。P9）ので、接頭辞、文字の種類、長さ、前後の区切りを手で書く。

| 種類（英 / 日） | 形 | 元にした規則 |
|---|---|---|
| an AWS access key ID / AWS のアクセスキー ID | `AKIA`・`ASIA`・`ABIA`・`ACCA`（または `A3T` と英大文字か数字の 1 字）のあとに、`A`〜`Z` と `2`〜`7` が 16 字。前後は英数字でない | gitleaks `aws-access-token` |
| a GitHub token / GitHub のトークン | `ghp_`・`gho_`・`ghu_`・`ghs_`・`ghr_` のあとに英数字 36 字。`github_pat_` のあとに英数字と `_` が 82 字 | gitleaks `github-pat` ほか |
| a Slack token / Slack のトークン | `xoxb-`・`xoxp-`・`xoxe-`・`xapp-` で始まり、gitleaks の各規則の数字と英数字の並び | gitleaks `slack-bot-token` ほか |
| a Slack incoming webhook URL / Slack の Incoming Webhook の URL | `hooks.slack.com/services/`（`workflows`・`triggers`）のあとに英数字と `+`・`/` が 43〜56 字 | gitleaks `slack-webhook-url` |
| a Stripe secret key / Stripe のシークレットキー | `sk_` か `rk_` のあとに `test_`・`live_`・`prod_`、英数字 10〜99 字。後ろは引用符、空白、`;`、行の終わり | gitleaks `stripe-access-token` |
| an OpenAI API key / OpenAI の API キー | `sk-proj-`・`sk-svcacct-`・`sk-admin-` か `sk-` で始まり、途中に `T3BlbkFJ` を持つ形 | gitleaks `openai-api-key` |
| an Anthropic API key / Anthropic の API キー | `sk-ant-api03-` か `sk-ant-admin01-` のあとに英数字と `_`・`-` が 93 字、最後が `AA` | gitleaks `anthropic-api-key`、`anthropic-admin-api-key` |
| a Google API key / Google の API キー | `AIza` のあとに英数字と `_`・`-` が 35 字 | gitleaks `gcp-api-key` |
| a private key / 秘密鍵 | `-----BEGIN` と、`PRIVATE KEY-----`（`PRIVATE KEY BLOCK-----`）までの行のあとに、base64 の文字が 64 字以上（行をまたいでよい。文字列の中の `\n` は飛ばす） | gitleaks `private-key`、GitHub の非プロバイダーのパターン |

- **調べるのはファイルの全文である。** 文字列の中だけでなく、コメントも調べる。コメントに残した鍵も、リポジトリを読める人に渡るからである。構文の誤りがあっても調べる（鍵は、ファイルが読めるかどうかに関係なくリポジトリにある）。
- **前後の区切り。** gitleaks の規則にある区切り（AWS の `\b`、Stripe・OpenAI・Anthropic・Google の「後ろは引用符、空白、`;`、`\n`」）はそのまま書いた。規則に区切りが無い種類（GitHub、Slack、Webhook の URL）も、前後が英数字に続くものは鍵としない。そのため、Google の API キーが URL の `&` の前にあるとき（`?key=AIza…&q=…`）は、gitleaks と同じく見つけない。秘密鍵は、頭の行のあとの base64 を、行の終わり（文字列の中の `\n` も）と次の行の字下げと行の頭の `#`・`//` を飛ばして数え、行の中の空白で切る。頭の行のあとに文章が続いても秘密鍵とはしない。
- **例の値は言わない。** AWS のアクセスキー ID が `EXAMPLE` で終わるもの（AWS の文書の例 `AKIAIOSFODNN7EXAMPLE`。gitleaks の規則の allowlist と同じ）と、接頭辞のあとが一つの文字の繰り返しのもの（`ghp_` と `x` が 36 字。`-` と `_` は数えない）は、例の値として言わない。gitleaks の各規則にあるエントロピーの下限は入れなかった（下の「捨てたもの」）。
- **値は伏せて出す。** 診断には、種類と、接頭辞と `…` と、長さだけを出す（`AIza…`、39 文字。秘密鍵は `-----BEGIN RSA PRIVATE KEY-----` の行だけ）。鍵そのものを出すと、CI のログに鍵が残る。
- **鍵のある行は引用しない。** 診断は、ふつうは原文の行を添える（rulec は行の枠、ほかは `<行> | <原文>`）。W901 はその行に鍵があるので、行を添えない。場所は見出しの `<ファイル>:<行>:<列>` と JSON の `line`・`col` が言う（rulec は `-->` の行と、JSON の `line`・`column`・`where`。`spans` は空）。chobo の JSON の `excerpt` も空にした。dandori だけは、どの診断でも原文の行を引いて見せる形なので、W901 でも行を引き、行の鍵を伏せた形に置き換えて見せる。
- **ほかの診断の行も伏せる。** ほかの診断が鍵のある行を見せるときも、ritsu-base の `secrets::mask` が、行の鍵を（テスト用の値も）接頭辞と `…` に替える。土台の診断（`Diag::source`）を使う koyomi、yuen、sakai、ritsu はこれで伏せ、自分の診断の型で行を見せる rulec、dandori、chobo、geas も、行を見せるところで同じ関数を通す。rulec は `^` も伏せたあとの行で数え、chobo は JSON の `excerpt` も伏せる。geas は、診断の文と注、ここまでの実行の行、成り立たなかったチェックの文とプログラムの出力、`--json` の主張の `error` も伏せる。geas の `.geas/` の journal とベースラインは、実行したことの記録なので、伏せない。呼び出しの引数に鍵を書いた仕様や、鍵を出力するプログラムでは、そこに鍵が残る（geas の DESIGN の W901 の段落）。
- **出すのは見つけた場所ごとに一つ**（行と列）。同じ値が二か所にあれば二つ出す。
- **契約の文書**（ritsu-cross。`ritsu check` だけ。16.1）。プロジェクトの `.proto` と、言語の参照の口（`References`）が指す文書のうち拡張子が `.json`・`.yaml`・`.yml` のもの（dandori の `use openapi`・`use smithy`、rulec の `import jsonschema` と `shape … jsonschema`、sakai の地図の公表された言語の `openapi "…"`・`asyncapi "…"`）を、ファイルごとに一度だけ調べる。

**意図の書き方。** テスト用の値は、同じ行のコメントに `ritsu: test secret` と書く（`.flow`・`.rule` などは `# ritsu: test secret`、`.proto` は `// ritsu: test secret`、YAML は `# ritsu: test secret`）。秘密鍵は `-----BEGIN` の行に書く。ritsu-base は、行の中にこの言葉があるかだけを見て、コメントの書き方は見ない。JSON の文書にはコメントが無いので、この書き方は使えない。JSON の文書の値は、例の値（`EXAMPLE` で終わる、一つの文字の繰り返し）に替える。

```
task find(q: string) -> Place
  http GET "https://maps.example.com/v1/find?key=AIzaSy…"   # ritsu: test secret
```

**診断の文。** どの言語も同じ三つの注を持つ。dandori の材料 `tests/fixtures/security/W901_key_in_a_string.flow`（`http` の URL に偽の鍵）で、dandori は次のとおり言う（dandori の文は小文字で始まる。ほかの言語は `A Google API key is written here` と大文字で始め、行を引かない）。

```
warning[W901]: tests/fixtures/security/W901_key_in_a_string.flow:5:50: a Google API key is written here (AIza…, 39 characters)
     5 |   http GET "https://maps.example.com/v1/find?key=AIza…"
  = A key in a file reaches everyone who can read the repository, its history and its builds. Keep it where the code runs (an environment variable, the platform's connection or secret store) and read it from there.
  = If this key is real, revoke it with Google first: taking it out of the file leaves it in the history of the repository.
  = If it is a value for tests, write `ritsu: test secret` in a comment on the same line.
```

```
警告[W901]: tests/fixtures/security/W901_key_in_a_string.flow:5:50: Google の API キーがここに書かれています（AIza…、39 文字）
     5 |   http GET "https://maps.example.com/v1/find?key=AIza…"
  = ファイルに書いた鍵は、リポジトリとその履歴とビルドを読めるすべての人に渡ります。鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。
  = 本物の鍵なら、まず Google で無効にしてください。ファイルから消しても、リポジトリの履歴には残ります。
  = テスト用の値なら、同じ行のコメントに `ritsu: test secret` と書いてください。
```

秘密鍵（プロバイダーの無い種類）の二つ目の注は「本物の鍵なら、まず新しい鍵に替えて、古い鍵を使えないようにしてください。ファイルから消しても、リポジトリの履歴には残ります。」（"If this key is real, replace it with a new one first, and see that the old one is no longer accepted: taking it out of the file leaves it in the history of the repository."）である。

契約の文書の W901 は、ritsu の台帳の再現（コメントに偽の鍵を書いた `maps.proto`）で、`ritsu check .` が次のとおり言う。三つ目の注が、文書のコメントの書き方（`.proto` は `//`、YAML は `#`）を言い、JSON の文書なら「JSON にはコメントが書けないので、例の値に替えてください」になる。

```
warning[ritsu W901]: maps.proto:5:56: A Google API key is written here (AIza…, 39 characters)
  = A key in a file reaches everyone who can read the repository, its history and its builds. Keep it where the code runs (an environment variable, the platform's connection or secret store) and read it from there.
  = If this key is real, revoke it with Google first: taking it out of the file leaves it in the history of the repository.
  = If it is a value for tests, write `// ritsu: test secret` on the same line.
ritsu check: 1 file (proto 1): all pass (1 warning); borders between the languages: 0 checked, 0 undecided
```

```
警告[ritsu W901]: maps.proto:5:37: Google の API キーがここに書かれています（AIza…、39 文字）
  = ファイルに書いた鍵は、リポジトリとその履歴とビルドを読めるすべての人に渡ります。鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。
  = 本物の鍵なら、まず Google で無効にしてください。ファイルから消しても、リポジトリの履歴には残ります。
  = テスト用の値なら、同じ行に `// ritsu: test secret` と書いてください。
ritsu check: ファイル 1 個（proto 1）。どれも検査を通りました（警告 1 件）。言語の境目: 確かめた 0 か所、決められない 0 か所
```

`explain` の再現は、どの言語でも、その言語のいちばん小さいファイルに、偽の Google の API キー（16.10）を書いたものである。

**捨てたもの**：

- **エントロピー（文字のばらつき）で、決まった形の無い鍵を探すこと。** gitleaks の generic な規則はこれをするが、ハッシュ、ID、base64 の値に当たる。ritsu の言語のファイルには、ハッシュの固定（`sha256:…`）や出典の固定が多い。決まった形のものに絞る。
- **gitleaks の設定（`.gitleaks.toml`、`.gitleaksignore`）や `gitleaks:allow` を読むこと。** ほかのツールの設定の形に合わせ続けることになる。ritsu の書き方は一つにする。
- **鍵が本物かをプロバイダーに問い合わせること。** ネットワークを使わない決まり（この章の頭）。
- **警告ではなくエラーにすること。** テスト用の値と本物を形では分けられない。

### 16.4 暗号化しない通信（W902）

**dandori が見る URL**：タスクとその呼び出し方が実際に送る先の URL である。

- `http` のタスクの URL。`use openapi` の操作を呼ぶなら、`use` の下の `url`、無ければ文書の最初のサーバー（dandori が実際に呼ぶもの。dandori の DESIGN 1.10）。
- エージェントの `url`（Open Responses のエンドポイント）。
- `use rule … connect "<URL>"`（規則の Connect のサービス）。
- `connect` のタスクが呼ぶ、`use proto` の下の `url`。

`lambda`、`aws`、Jev、`url` の無いエージェントは、プラットフォームや SDK が HTTPS で送るので見ない。`use` の URL の W902 は、それを呼ぶタスクがあるときだけ、`url` の行（無ければ `use` の行）に、`use` ごとに一つ出す。タスクごとに出すと、一つの直し方に、同じ警告がタスクの数だけ並ぶからである。

**sakai が見るサーバー**：地図の成果物の OpenAPI と AsyncAPI の文書の、ルートの `servers`（OpenAPI はパスの項と操作の `servers`、OpenAPI 3.2 の `additionalOperations` も）である。公表された言語に入れたかは問わない。

- OpenAPI：`url` が `http://` か `ws://` で始まり、ホストがループバックでないもの。`url` が相対（OpenAPI 3.2 の Server Object は「相対でもよく、そのときは文書を置いた場所からの相対」と書く）なら、スキームが分からないので見ない。サーバー変数（`{scheme}://…`）は、まず全部を `default` の値にし、次に変数ごとに `enum` の値を一つずつ入れて（ほかは `default` のまま）見る。全部の組み合わせは見ない。注は、どの変数のどの値でそうなったかを言う。ほかのファイルへの `$ref` で書いたパスの項のサーバーは、まだ見ない（16.12）。
- AsyncAPI：`protocol` が次の表の左の列のもので、`host` がループバックでないもの。AsyncAPI 3.0 と 3.1 の Server Object は `protocol` の値を並べていない。2.6.0 は「Supported protocol include, but are not limited to: `amqp`, `amqps`, `http`, `https`, `ibmmq`, `jms`, `kafka`, `kafka-secure`, `anypointmq`, `mqtt`, `secure-mqtt`, `solace`, `stomp`, `stomps`, `ws`, `wss`, `mercure`, `googlepubsub`, `pulsar`」と書き、暗号化して通信する名前を持つのは、次の表の六つである。

| 暗号化しない | 暗号化して通信するプロトコル（注に出す） |
|---|---|
| `http` | `https` |
| `ws` | `wss` |
| `amqp` | `amqps` |
| `mqtt`（`mqtt5` も） | `secure-mqtt` |
| `stomp` | `stomps` |
| `kafka` | `kafka-secure` |

表に無い `protocol`（`nats`、`jms`、`ibmmq`、`solace`、`pulsar`、`googlepubsub`、`sns`、`sqs`、`redis` など）は見ない。暗号化するかが `protocol` の名前に出ないからである。

**ループバック**（ritsu-base の `urls::is_loopback`）：`localhost`、`.localhost` で終わる名前（RFC 6761 がループバックに決めている）、`127.0.0.0/8` の IPv4、`::1`（`[::1]`）、IPv4 のループバックを表す IPv6 のアドレス（`::ffff:127.0.0.1`）。プライベートなネットワーク（`10.0.0.0/8`、`.internal` など）は入れない。このマシンの外へ出る通信だからである。

**意図の書き方**：

- dandori：URL を書いたところに `plaintext "<理由>"` と書く。`http` のタスクとエージェントの `url` はタスクの下、規則のサービスは `use rule … connect` の下、`use openapi`・`use proto` から呼ぶ操作は `use` の下である。理由は要る。`use` から呼ぶタスクの下の `plaintext` は E007 で、その `use` の下に書くよう注で言う。URL を持たないところ（`connect` の無い `use rule`、`use smithy`、`url` の無い `use proto`、サーバーの無い OpenAPI の文書）の `plaintext` と、空の理由も E007 である。

  ```
  task read_inquiry(text: string) -> Reading
    agent "Read the text of a customer's inquiry, …"
    model "gpt-oss:20b"
    url "http://ollama.internal:11434/v1"
    plaintext "The model server is reached only inside the cluster network, which the service mesh encrypts"
  ```

- OpenAPI と AsyncAPI の文書：Server Object に `x-ritsu-plaintext: "<理由>"` と書く。どちらの仕様も Server Object に `x-` で始まる拡張を書けるので、ほかのツールはこれを読み飛ばす。意図が、文書と一緒に動く。空の文字列や文字列でない値は意図として読まず、W902 の注で理由を書くよう求める。

**Step Functions の E050 との関係。** Step Functions の HTTP Task は HTTPS の送り先しか呼べないので、dandori は `http://` の送り先を、`dandori build --target asl` で E050 にしている（dandori の DESIGN 1.7）。W902 はどのプラットフォームにも出す `check` の警告で、E050 はそのまま残る。`plaintext` を書いても、Step Functions には出せない。

**診断の文。** dandori（材料 `tests/fixtures/security/W902_agent_url.flow`）：

```
warning[W902]: tests/fixtures/security/W902_agent_url.flow:7:3: the agent `read` sends its requests to ollama.internal over plain HTTP
     7 |   url "http://ollama.internal:11434/v1"
  = Whoever is on the network between can read and change the requests, the answers, and any key in the headers.
  = Use https://. If the connection is protected another way (a service mesh, a private link), say so under the task with `plaintext "<why>"`.
```

```
警告[W902]: tests/fixtures/security/W902_agent_url.flow:7:3: エージェント `read` は、ollama.internal に暗号化しない HTTP でリクエストを送ります
     7 |   url "http://ollama.internal:11434/v1"
  = 途中のネットワークにいる人は、リクエストとレスポンスと、ヘッダーの鍵を読んだり書き換えたりできます。
  = https:// にしてください。ほかの仕組み（サービスメッシュ、プライベートな接続など）で守っているなら、タスクの下に `plaintext "<理由>"` と書いてください。
```

sakai（webshop の決済の AsyncAPI の文書に、TLS の無い Kafka のブローカーを足した変異 `W902_kafka_broker` と、日本語の版 `W902_Kafka_のブローカー`）：

```
warning[W902]: payments/events/payments.yaml:43:15: The server production of payments/events/payments.yaml does not encrypt the connection (kafka)
    43 |     protocol: kafka
  = Whoever is on the network between can read and change the messages, and any key sent with them.
  = The encrypted form of the protocol is kafka-secure.
  = If the connection is protected another way (a service mesh, a private link), write `x-ritsu-plaintext: "<why>"` in the server.
  involved:
      Payments  payments/events/payments.yaml:41  #/servers/production
```

```
警告[W902]: payments/events/payments.yaml:43:15: payments/events/payments.yaml のサーバー production は、通信を暗号化しません（kafka）
    43 |     protocol: kafka
  = 途中のネットワークにいる人は、メッセージと、一緒に送る鍵を読んだり書き換えたりできます。
  = 暗号化して通信するプロトコルは kafka-secure です。
  = ほかの仕組み（サービスメッシュ、プライベートな接続など）で守っているなら、サーバーに `x-ritsu-plaintext: "<理由>"` と書いてください。
  関わるもの:
      決済  payments/events/payments.yaml:41  #/servers/production
```

**捨てたもの**：

- **プライベートなネットワークのアドレスも通すこと。** 「社内だから平文でよい」をツールが決めることになる。通すなら、書いた人が理由を書く。
- **意図を `.ctx` に書くこと。** URL を書いたファイル（`.flow` と文書）に意図を置くほうが、差分を読む人が一緒に読める。
- **dandori が `use openapi` の文書のサーバーを見ないこと（文書は sakai に任せる）。** 地図を書かずに dandori だけを使う人には、呼ぶ URL が平文でも何も出なくなる。dandori は呼ぶ URL を、sakai は文書を見る。`ritsu check` で一つの文書のサーバーについて二つ出ることがあるが、指す場所が違い（`.flow` の `use` の行と、文書のサーバーの行）、直す場所もそれぞれにある。

### 16.5 認証を書いていない公開の操作（W903）

**OpenAPI**（sakai。公表された言語の OpenAPI の文書の、`paths` の操作）。操作の実際の `security` は、操作に `security` があればそれ、無ければ文書のルートの `security` である（OpenAPI 3.2 の Operation Object：「This definition overrides any declared top-level `security`. To remove a top-level security declaration, an empty array can be used.」）。

| 実際の `security` | 判定 |
|---|---|
| どこにも書いていない | W903 |
| 一つ以上の要件があり、空の要件 `{}` を含まない | 通る |
| 空の要件 `{}` を含む（OpenAPI 3.2：「To make security optional, an empty security requirement (`{}`) can be included in the array.」） | 通る（認証を任意にすると書いた） |
| 操作に書いた空の配列 `security: []` | 通る（だれでも呼べると書いた） |
| ルートに書いた空の配列 `security: []` で、操作に何も無い | 通る（文書全体を、だれでも呼べると書いた） |

`webhooks` の操作は見ない。webhook の `security` は、API がクライアントの側を呼ぶときのことで、コンテキストが公開する操作ではないからである。公表された言語に入れていない文書（コンテキストの中だけで使う API）も見ない。

**AsyncAPI**（sakai。公表された言語の AsyncAPI の文書の、チャネル）。AsyncAPI 3.1 の `security` は、サーバー（接続）と操作（Operation Object：「In cases where Server Security also applies, it MUST also be satisfied.」）に書く。操作の `security` はサーバーのものに足すもので、置き換えない。空の配列の意味は仕様に書いていない。

- チャネルが使うサーバー（Channel Object の `servers`。無ければ文書のすべてのサーバー）のうち、`security` を書いていないサーバーがあり、そのチャネルを使う操作のどれも `security` を書いていなければ、W903。
- サーバーか操作に、空の配列 `security: []` を書いたものは、意図として通す。仕様は空の配列を禁じておらず、「認証の方法を一つも並べない」と書いたことになるので、OpenAPI と同じ読み方にそろえた。
- 文書に `servers` が一つも無ければ見ない（接続のことを何も言っていない）。
- ほかの文書のチャネルへの `$ref`（受け取る側の文書が書くもの）は、そのチャネルを持つ文書の側で見る。両方で見ると、同じチャネルについて二度出る。

診断の見出しは、操作を `operationId` で、チャネルを名前で言い、関わるもの（`involved`）の行に、場所と `#<JSON Pointer>`（と、操作ならメソッドとパス）を書く。参照の書き方に `openapi`・`asyncapi` のツール名が入れば、その形にも書ける。検査そのものは、どちらにも頼らない。

**診断の文。** webshop の決済の OpenAPI の文書から、ルートの `security` を消した変異 `W903_operation_without_security`（日本語の版 `W903_認証の無い操作`）の最初の出力：

```
warning[W903]: payments/api/payments.yaml:9:7: The operation createCharge of the published language payments.v1 says no authentication
     9 |       operationId: createCharge
  = Neither the operation nor the document has `security`, so a reader of the contract cannot tell how a client proves who it is.
  = Add `security` to the operation, or to the whole document. If the operation is open to anyone on purpose, write `security: []` on it.
  involved:
      Payments  payments/api/payments.yaml:9  #/paths/~1charges/post (POST /charges)
```

```
警告[W903]: payments/api/payments.yaml:9:7: 公表された言語 payments.v1 の操作 createCharge に、認証の指定がありません
     9 |       operationId: createCharge
  = 操作にも文書にも `security` が無いので、契約を読む人には、クライアントがどう認証すればよいかが分かりません。
  = 操作か文書全体に `security` を書いてください。だれでも呼べるようにわざとしている操作なら、その操作に `security: []` と書いてください。
  関わるもの:
      決済  payments/api/payments.yaml:9  #/paths/~1charges/post（POST /charges）
```

AsyncAPI のチャネル（変異 `W903_channel_on_a_server_without_security`）：

```
warning[W903]: payments/events/payments.yaml:9:3: The channel paymentSucceeded of the published language payments.v1 says no authentication on the server production
     9 |   paymentSucceeded:
  = Neither the server nor an operation on the channel has `security`, so a reader of the contract cannot tell how a client proves who it is.
  = Add `security` to the server, or to the operations on the channel. If it is open to anyone on purpose, write `security: []` on the server or on an operation.
  involved:
      Payments  payments/events/payments.yaml:9   #/channels/paymentSucceeded (a channel of payments/events/payments.yaml)
      Payments  payments/events/payments.yaml:38  #/servers/production (a server without `security`)
```

**捨てたもの**：

- **`open host service` に並べた操作だけを見ること。** 公開ホストサービスは「ほかのコンテキストのだれでも呼べる」という地図の上の約束で、インターネットに公開するかとは別である。公表された言語の文書は、どれも境界の外から読む契約なので、全部の操作を見る。
- **認証の方式の強さ（`apiKey` を query に置く、`http` の `basic`）を言うこと。** 書いた方式が弱いかは、運用の決めで、ここでは書いたかどうかだけを見る。

**認可とのつながり。** W903 は、OpenAPI と AsyncAPI の文書の `security`（だれが呼んでいるかを確かめること）を見る検査のままにする。その先の、どの操作にも許可の決まり（ritsu が読む Cedar のポリシーとスキーマ（4.18）、または八つ目の言語 sekisho の `.gate` が生成する Cedar）があるかは、sekisho の側の言語をまたぐ検査が言う。

### 16.6 秘密の印

**proto**：`google.protobuf.FieldOptions` の `debug_redact`（フィールド番号 16）。印の付け方は二つで、protobuf の 2024-12-04 の告知が書くとおりである（「Mark a field with the field option `debug_redact = true`, directly」と、カスタムのオプションの列挙の値に `debug_redact = true` を付け、その値でフィールドに印を付ける形）。どちらも読む（ritsu-proto の `Protos::redaction`。4.19）。

```proto
message BankAccount {
  string id = 1;
  string number = 2 [debug_redact = true];
  string holder = 3 [(acme.v1.sensitivity) = PERSONAL];   // PERSONAL = 1 [debug_redact = true];
}
```

C++ の protobuf は v30 から、この印のフィールドをデバッグの出力（`DebugString` など）で伏せる。JSON やバイナリにしたときの中身は変わらない。ritsu は、この印を「この値を履歴やほかのサービスへ平文で出さない」という契約の宣言として読む。dandori では、`.proto` の印は、メッセージから作ったレコードのフィールドのほか、`connect` のタスクの引数と結果（リクエストとレスポンスのフィールドと突き合わせるところ）にも効く。手で書いた引数（`account_number: string`）が、印の付いたフィールドに送られる形を取りこぼさないためである。

**OpenAPI・AsyncAPI・JSON Schema**：スキーマのプロパティが、次のどれかを持てば印とする（ritsu-base の `marks`。4.19）。

| 書き方 | 出典 | 印にするもの |
|---|---|---|
| `x-data-classification` | OpenAPI の拡張の登録簿（Schema Object に書く。`category` と `masking` が要り、`sensitivity` は `public`・`internal`・`confidential`・`restricted`、書かなければ `confidential`） | `sensitivity` が `confidential` か `restricted`（書かないときを含む） |
| `x-sensitive-data` | OpenAPI の拡張の登録簿（Schema Object に書く。表示で値を伏せる） | 書いてあれば |
| `format: password` | OpenAPI のフォーマットの登録簿（「a string that hints to obscure the value」） | 書いてあれば |

`writeOnly: true` だけでは印にしない。JSON Schema 2020-12 の 9.4 は「'writeOnly' would be used to mark a password input field」と例に挙げるが、意味は「取り出すときには無い」で、作るときにだけ送るフィールド（初めの残高など）にも使う。パスワードのフィールドは、たいてい `format: password` も持つ。`x-data-classification` の `internal` は、組織の中で扱ってよい値なので、印にしない（履歴に残ってよい）。外へ送ることだけを止めたい値として区別することは、16.12 に残した。dandori は、OpenAPI の操作と突き合わせるタスクの引数と結果で、これを読む。

**dandori の `secret`**：dandori の値を置くところ（レコードのフィールド、入力、出力、タスクの引数と結果）の型のあとに、`range` と同じ位置で `secret` と書ける（`range` と両方なら `range` のあと）。契約の無い値（ワークフローに渡すトークン、手で書いたレコード）に印を付けるためである。

```
inputs
  seller_id  : string
  api_token  : string secret

task score(text: string secret) -> Score
```

`secret`・`plaintext`・`discloses`・`history encrypted` は、dandori の予約語の表（`src/syntax.rs` の `KEYWORDS`）に足さず、`agent`・`model`・`jev` と同じく、書ける位置でだけ読む。いまの `.flow` で名前に使える語は減らない。

**dandori が秘密の値をたどる決まり**：値の範囲を求めるときと同じく、流れに沿わず、変数に値を入れるすべての場所を合わせる（dandori の DESIGN 1.3）。

- 印の付いたところ（`secret` を書いた入力・フィールド・引数・結果、印の付いたフィールドを持つ `.proto` のメッセージから作ったレコード、`connect` のタスクと OpenAPI の操作と突き合わせたタスクの引数と結果のうち、フィールドやプロパティが印を持つもの）から読んだ値は秘密である。印の付いたフィールドを中に持つレコードは、そのフィールドのパスごとに秘密を持つ（`account.number`）。
- `{…}` のレコード、`[…]` のリスト、`for` の変数、`some x` の `x` は、中に入れた値の秘密をそのまま持つ。値を埋め込んだ文字列は、埋め込んだ値のどれかが秘密なら、全体が秘密になる。`json` の値は、秘密の値から作ったなら、全体が秘密になる。
- 規則の出力は秘密にしない。規則の出力は、入力から決めた判断（列挙、bool、額）であり、秘密を運ぶものではないからである。印を付けていないタスクの結果も、秘密にしない。
- 変数は、値を入れるどこかで秘密が入れば秘密である（範囲と同じく、実際には通らないパスのせいで言うことがある。そのときは変数を分ける）。
- 診断は、秘密の値を、フローがそれを最初に読んだ書き方で言い（`account.number`。変数を通しても、`let note = "… {account.number}"` を渡しても同じ）、印がどこに書いてあるかを注に出す。一つの値に印が二つあれば（入力の `secret` と、引数の `format: password`）、診断は一つにして、注に印を二つ並べる。同じ場所で同じ値を何度も言わない。
- 印のファイルは、`.flow` のある場所から見たパスで言う（`../../../examples/payout/specs/payout.proto:34`）。`.flow` に書いた `secret` はファイル名と行で、OpenAPI の文書は JSON Pointer で言う（`secrets/crm.json#/components/schemas/Customer/properties/email`）。

### 16.7 履歴に残る秘密の値（W904）

プラットフォームは、ワークフローの入力と出力と、呼び出しの引数と結果を、実行の履歴に残す。

| プラットフォーム | 残るもの | 鍵を持つ人だけが読めるようにする手段 |
|---|---|---|
| Temporal | ワークフローの入力と結果、アクティビティ（タスク、規則、日付、帳簿）の入力と結果、子ワークフロー、Update の値。失敗の文とスタックトレースは、既定ではペイロードの外に平文で残る | ペイロードのコーデックで暗号化し、失敗のコンバーターの `encodeCommonAttributes` で失敗の文もそれに通す |
| Step Functions | 実行の入力と出力、各ステートの入力と出力 | ステートマシンにカスタマー管理の KMS キーを設定する（実行の履歴を暗号化し、`GetExecutionHistory` と `DescribeExecution` には `kms:Decrypt` が要る） |
| Lambda durable functions | ステップの結果（実行の履歴として読める） | durable の実行のデータを、カスタマー管理の KMS キーで暗号化する（2026-07-22 の告知から。関数の `DurableConfig.KMSKeyArn`） |
| Argo Workflows | テンプレートの入力と出力のパラメーター（Workflow のオブジェクトの中。UI と `argo get` で読める） | 無い（Argo の文書は、秘密を Kubernetes の Secret から環境変数かボリュームで渡すよう勧める） |
| pydantic-graph | 残さない（2.x はどこにも状態を残さない。dandori の DESIGN 4.5） | 要らない |

**W904 を出すところ**（dandori の `check`。どのプラットフォームにも出すものなので `build` ではなく `check` に置く）。列は 1（E014 と同じ）。

- 秘密の値を持つワークフローの入力と出力（宣言の行）。`succeed` が出力に書いた秘密の値は、その行。
- 秘密の値を持つタスクの結果（タスクの宣言の行。フローが呼ぶタスクだけ。結果は、ワークフローがそれを読まなくても履歴に残る）。
- 秘密の値を渡すタスク・規則・日付・帳簿・子のフローの呼び出し（呼び出しの行）。
- 秘密の値を埋め込む `fail` の理由（その行）。

**意図の書き方**：`workflow` の行の下に `history encrypted` と書く。「この実行の履歴は、秘密を読んでよい人だけが持つ鍵で暗号化してある」という宣言である。プラットフォームごとに次のようになる。

| プラットフォーム | `history encrypted` で dandori がすること |
|---|---|
| Temporal（TypeScript） | `worker.ts` の `WorkerConfig.codec` を要るフィールドにし、ワーカーの `dataConverter` に、そのコーデックと `failure.ts` の `DefaultFailureConverter({ encodeCommonAttributes: true })` を入れる。`client.ts` の関数は、`encryptedClient(connection, codec)` だけが返す `EncryptedClient`（`Client` に、モジュールの外から作れない `unique symbol` の印を足した型）を受け取る。`replay` もコーデックを受け取る |
| Temporal（Python） | `client.py` の `connect(target, codec)` だけが返す `EncryptedClient`（`NewType`）を、関数と `worker.py` の `make_worker` が受け取る。データコンバーターの失敗のコンバーターは `DefaultFailureConverterWithEncodedAttributes`。`replay` もコーデックを受け取る |
| Temporal（Go） | `client.go` の `Dial(options, codec)` だけが、パッケージの外では作れない `EncryptedClient` を返し、関数と `NewWorker` が受け取る。失敗のコンバーターは `temporal.NewDefaultFailureConverter(… EncodeCommonAttributes: true)`。`Replay` もコーデックを受け取る |
| Step Functions | 何も変えない。ステートマシンの `EncryptionConfiguration`（`Type: CUSTOMER_MANAGED_KMS_KEY` と `KmsKeyId`）に設定する鍵で、`GetExecutionHistory` と `DescribeExecution` には `kms:Decrypt` が要る |
| Lambda durable functions | 何も変えない。関数の `DurableConfig.KMSKeyArn` に設定する鍵で、`GetDurableExecution` と `GetDurableExecutionHistory`（`IncludeExecutionData=true`）には `kms:Decrypt` が要る。実行は始まったときの鍵を最後まで使う |
| Argo Workflows | E050（`history encrypted` の行）。パラメーターを鍵で暗号化する手段が無いので、宣言を守れない（P6） |
| pydantic-graph | 何もしない。履歴を残さない |

`history encrypted` で生成するクライアントが増やす名前（Go の `EncryptedClient`・`Dial`・`CodecDataConverter`・`CodecFailureConverter` など）は、クライアントがもとから持つ名前と同じに扱う。サービスのメソッドとメッセージの名前は `Rpc`（Python は `_rpc`、Go は `RPC`）を付け、Go のレコードと列挙の型は `_` を付けて、生成する側が避ける（dandori の DESIGN 1.14、1.18）。E006 では止めない。`.proto` の名前は利用者が変えられないことが多く、止めると、そのサービスでは暗号化を宣言できなくなるからである。

コーデックを型で要るものにしたので、渡し忘れは、利用者のコードの型の検査（`tsc --strict`、`mypy --strict`、`go vet`）が言う。Step Functions と durable functions の鍵は、dandori が書く ASL と関数のコードの外にあり、dandori はそれを確かめられない。宣言の無いフローの生成物は一バイトも変えない。Go の `NewWorker` は、包む前の SDK のクライアントを `worker.New` に渡す（SDK 1.49.0 の `worker.New` は、`client.Dial` が作ったクライアントそのものを求め、包んだ値では「Client must be created with client.Dial() or client.NewLazyClient()」で止まった。実際に走らせて見つけた）。

**診断の文**（材料 `tests/fixtures/security/W904_payout_number.flow`。例の `payout` を、口座の番号を運ぶ形に一か所変えたもの）：

```
warning[W904]: tests/fixtures/security/W904_payout_number.flow:39:1: the secret `account.number` is kept in the history of the workflow, as the argument `reference` of `pay`
    39 |   let paid = pay(account_id: account_id, amount: amount, reference: "Sales payout to {account.number}")
  = The mark is `debug_redact = true` at ../../../examples/payout/specs/payout.proto:34.
  = Temporal, Step Functions, Lambda durable functions and Argo Workflows keep the inputs and outputs of the workflow and of every call in the history, which whoever may read the executions can read.
  = Pass a reference instead (an ID, the name of a secret) and fetch the value inside the task. If the history is encrypted with a key you hold (Temporal: a payload codec; Step Functions and Lambda durable functions: a customer managed KMS key), write `history encrypted` under `workflow`.
```

```
警告[W904]: tests/fixtures/security/W904_payout_number.flow:39:1: 秘密の値 `account.number` が、`pay` の引数 `reference` として、ワークフローの履歴に残ります
    39 |   let paid = pay(account_id: account_id, amount: amount, reference: "Sales payout to {account.number}")
  = 印は ../../../examples/payout/specs/payout.proto:34 の `debug_redact = true` です。
  = Temporal、Step Functions、Lambda durable functions、Argo Workflows は、ワークフローとすべての呼び出しの入力と出力を履歴に残し、実行を読める人はだれでもそれを読めます。
  = 値の代わりに参照（ID やシークレットの名前）を渡し、タスクの中で値を取ってきてください。履歴を自分の持つ鍵で暗号化しているなら（Temporal はペイロードのコーデック、Step Functions と Lambda durable functions はカスタマー管理の KMS キー）、`workflow` の下に `history encrypted` と書いてください。
```

同じ材料は、口座を返すタスクの宣言の行（15 行）にも、結果の形の W904 を出す（「`get_account` の結果が持つ秘密の値 `number` が、ワークフローの履歴に残ります」。注の三つ目は、タスクには値の代わりに参照を返させるよう言う）。

**捨てたもの**：

- **`dandori build` でプラットフォームごとに出すこと（E040 と同じ置き場所）。** `ritsu check` に出なくなる。履歴に残ることは五つのうち四つのプラットフォームで同じなので、`check` に置いた。pydantic-graph だけに出すフローでも W904 は出る（文は、四つのプラットフォームが残すと言う）。
- **Temporal だけの宣言にすること（`payload codec`）。** Step Functions と Lambda durable functions も、カスタマー管理の鍵で同じことができる（読むのに `kms:Decrypt` が要る）。宣言は「鍵を持つ人だけが読める」という事実にし、手段はプラットフォームごとにした。
- **`history encrypted` のとき、Step Functions と durable functions でも E050 にすること。** できるプラットフォームで締め出すことになる（P6 は、できないプラットフォームでだけ E050 にする）。

### 16.8 秘密の値を外へ送ること（dandori の E906、ritsu の E905・W905）

**宛先の決め方**（dandori が、タスクと呼び出し方から決める）：

| 呼び出し | 宛先 | 判定する検査 |
|---|---|---|
| `agent`（`url` が無い、またはループバックでない `url`） | モデルのプロバイダー（OpenAI、Anthropic）か、`url` のサーバー | dandori の E906 |
| `jev` | TypeSafe | dandori の E906 |
| `http`（`use openapi` の文書を使わず、URL だけで書いたもの。ホストがループバックのものは除く） | URL のホスト | dandori の E906 |
| `aws` | その AWS のサービス | dandori の E906 |
| `http` で `use openapi` の操作を呼ぶもの | その文書のファイル | ritsu の E905 |
| `connect`（`use proto`） | その `.proto` のファイル | ritsu の E905 |
| `use rule … connect` の規則の呼び出し | その規則のファイル | ritsu の E905 |
| `flow "<パス>"` | 子の `.flow` | ritsu の E905 |
| 帳簿の操作、日付の呼び出し | その `.book`、`.cal` | ritsu の E905 |
| `lambda`、`image`、利用者が書く実装、dandori で書いていない子ワークフロー、同梱の規則、ループバックの `url` のエージェントと `http` | フローと同じところ（利用者のコードと、このマシン） | 見ない |

**dandori の E906**：秘密の値を、プロジェクトの外の相手へ送る。地図が要らないので dandori の検査に置き、`dandori check` を単体で使う人にも届く。モデルのプロバイダーに個人の情報を読ませる、というよくある漏れ方がここに入る。診断は宛先を、OpenAI、Anthropic、`url` のホスト、`TypeSafe (Jev)`、`AWS (<サービス>)`、`http` の URL のホストと言う。

**ritsu の E905**（X14）：秘密の値を、プロジェクトの中の成果物へ送るとき、地図の上で送ってよいかを確かめる。

- フローのファイルがどの地図のコンテキストにも属さないときは、見ない（地図が何も言っていない）。属するときは、その地図で次のように決める。
- 地図が二つ以上あるときは、sakai が地図と答える `.ctx` をパスの順に見て、フローを持つ（`context_of` が `Some` を返す）最初の地図で決める。
- 印のコンテキスト：印を書いたファイル（`.proto`、文書）が属するコンテキスト。印を `.flow` の `secret` で書いたとき、印を書いたファイルがどのコンテキストにも属さないときは、フローのコンテキスト。
- 宛先のコンテキスト：宛先のファイルが属するコンテキスト。
- 宛先がどのコンテキストにも属さないとき（地図の外）：E905。
- 宛先のコンテキストが印のコンテキストと同じか、地図にその二つの関係（`separate ways` のほかのどれでも。上流と下流、共有カーネル、パートナーシップ）があるとき：通る。
- 関係が無いとき：E905。宛先のコンテキストが印のコンテキストと `separate ways` を書いているときは、注にそう添える。
- 地図が、sakai の検査のうちコンテキストと関係と属し方を決める段（構文、名前、パスと、属し方。sakai の DESIGN 16.4）を通らないとき：W905（決められない理由を言う。P5）。どの地図もフローを持たず、通らない地図があるときは、そのフローの、秘密を送る呼び出しの全部が W905 になる。

**意図の書き方**：タスクの下に `discloses <引数>, … "<理由>"` と書く。そのタスクがその引数の秘密を宛先へ送ることを、書いた人が決めたという宣言である。E906 と E905 の両方を通す。履歴（W904）は通さない（送ることと、履歴に残ることは別のことである）。`discloses` の引数がタスクに無いとき、理由が空のときは E007。

```
task read_inquiry(text: string) -> Reading
  agent "Read the text of a customer's inquiry, …"
  model "gpt-5.4-mini"
  discloses text "The model reads the inquiry to sort it; the provider keeps no data under our agreement"
```

**数え方**：X14 は、宛先がプロジェクトの中で、秘密の値を渡す呼び出しごとに、境目一つと数える（`ritsu check` の要約の `borders`。8.3）。秘密の値が二つあれば、どれか一つが E905 なら failed、そうでなく W905 があれば undecided、どれも通れば held。`discloses` で通したものは held に数える。地図の無いプロジェクトでは数えない。

**診断の文。** dandori の E906（材料 `tests/fixtures/security/E906_payout_holder.flow`。例の `payout` の、エージェントが下書きする知らせに口座の名義を渡す形）：

```
error[E906]: tests/fixtures/security/E906_payout_holder.flow:41:1: the task `draft_notice` sends the secret `account.holder` to OpenAI, outside the project
    41 |   let notice = draft_notice(amount: amount, payout_id: paid.payoutId, holder: account.holder)
  = The mark is `debug_redact = true` at ../../../examples/payout/specs/payout.proto:35.
  = Send a reference or only what the other side needs. If sending it there is intended, write `discloses holder "<why>"` under the task.
```

```
エラー[E906]: tests/fixtures/security/E906_payout_holder.flow:41:1: タスク `draft_notice` が、秘密の値 `account.holder` をプロジェクトの外の OpenAI に送ります
    41 |   let notice = draft_notice(amount: amount, payout_id: paid.payoutId, holder: account.holder)
  = 印は ../../../examples/payout/specs/payout.proto:35 の `debug_redact = true` です。
  = 参照か、相手に要るものだけを送ってください。そこへ送ることを意図しているなら、タスクの下に `discloses holder "<理由>"` と書いてください。
```

ritsu の E905 は、台帳の再現で `ritsu check .` が次のとおり言う。地図は Payments・Ordering・Notices の三つのコンテキストで、Ordering は二つと `partnership` を結び、Payments は `card.proto` を、Notices は `notices.json` を公表した言語に置く（文書には `security` を書いたので、sakai の W903 は出ない）。`card.proto` は `card.number` に `debug_redact` を付け、Ordering のフローが `card` を Notices の API へ渡す。フローは `history encrypted` を書いたので、dandori の W904 も出ない。英語の版を英語で：

```
ordering/checkout.flow: ok
shop.ctx: ok — 3 contexts, 2 relationships; 3 artifacts, each in one context; 2 crossings checked (dandori 2)
error[ritsu E905]: ordering/checkout.flow:17: The task `tell` sends the secret `card.number`, marked by Payments, to Notices, which has no relationship with Payments
    17 |   tell(order: order, card: card)
  = The mark is `debug_redact = true` at payments/v1/card.proto:7.
  = The map shop.ctx relates Notices with Ordering only.
  = Send a reference instead, add the relationship to the map, or, if sending it there is intended, write `discloses card "<why>"` under the task.
ritsu check: 6 files (proto 1, dandori 1, sakai 4): 1 fail (1 error); borders between the languages: 1 checked, 0 undecided
```

日本語の版を日本語で：

```
受注/注文.flow: 検査を通りました
店.ctx: ok — コンテキスト 3、関係 2。成果物 3 件は、どれも一つのコンテキストに属する。境界を越える参照 2 件を確かめた（dandori 2）
エラー[ritsu E905]: 受注/注文.flow:17: タスク `知らせる` が、「決済」が印を付けた秘密の値 `カード.number` を、「決済」と関係の無い「通知」に送ります
    17 |   知らせる(order: 注文, card: カード)
  = 印は 決済/v1/card.proto:7 の `debug_redact = true` です。
  = 地図 店.ctx は、「通知」を「受注」とだけ関係づけています。
  = 値の代わりに参照を送るか、地図に関係を足すか、そこへ送ることを意図しているなら、タスクの下に `discloses card "<理由>"` と書いてください。
ritsu check: ファイル 6 個（proto 1、dandori 1、sakai 4）。検査を通らないもの 1 個（エラー 1 件）。言語の境目: 確かめた 1 か所、決められない 0 か所
```

地図の外へ送るとき（`crates/ritsu-cross/tests/egress_map.rs`。地図とコンテキストは sakai の口が答え、フローが何を送るかだけをテストが与える）：

```
error[E905]: ordering/checkout.flow:32: The task `report` sends the secret `card.number`, marked by Payments, to "tools/report.yaml", which no context of the map shop.ctx holds
    32 |   report(card: card)
  = The mark is `debug_redact = true` at payments/v1/card.proto:7.
  = Send a reference instead, give the file to a context of the map (`owns`), or, if sending it there is intended, write `discloses card "<why>"` under the task.
```

```
エラー[E905]: ordering/checkout.flow:32: タスク `report` が、「決済」が印を付けた秘密の値 `card.number` を、地図 shop.ctx のどのコンテキストにも属さない "tools/report.yaml" に送ります
    32 |   report(card: card)
  = 印は payments/v1/card.proto:7 の `debug_redact = true` です。
  = 値の代わりに参照を送るか、そのファイルを地図のコンテキストに入れるか（`owns`）、そこへ送ることを意図しているなら、タスクの下に `discloses card "<理由>"` と書いてください。
```

`separate ways` のときは、二つ目の注が「The map shop.ctx relates Notices with no other context (with Payments, it writes `separate ways`: the two have nothing to do with each other).」になる。呼び出しは、送り先のファイルが `.rule` なら「規則 `x` の呼び出し」、`.cal` なら「`x` の呼び出し」、ほかは「タスク `x`」と言う（`Send::task` は、規則の呼び出しでは規則の名前だからである）。

W905 は、同じ再現の地図が、無いファイルを `use context` する形で、`ritsu check .` が次のとおり言う。

```
ordering/checkout.flow: ok
error[sakai E009]: shop.ctx:7:13: The path "contexts/billing.ctx" is not there
     7 | use context "contexts/billing.ctx"
  = A path counts from the directory of this .ctx; as written, it points at contexts/billing.ctx.
warning[ritsu W905]: ordering/checkout.flow:17: Where in the map shop.ctx the task `tell` sends the secret `card.number` cannot be decided
    17 |   tell(order: order, card: card)
  = The map does not pass sakai's check, so which context a file belongs to is not known (sakai's E009: The path "contexts/billing.ctx" is not there).
  = Correct the map so that `sakai check` passes.
ritsu check: 6 files (proto 1, dandori 1, sakai 4): 1 fail (1 error, 1 warning); borders between the languages: 1 checked, 1 undecided
```

```
受注/注文.flow: 検査を通りました
エラー[sakai E009]: 店.ctx:7:13: パス "contexts/請求.ctx" がありません
     7 | use context "contexts/請求.ctx"
  = パスは、この .ctx のあるディレクトリからの相対パスです。書いてあるパスは contexts/請求.ctx を指します。
警告[ritsu W905]: 受注/注文.flow:17: タスク `知らせる` が秘密の値 `カード.number` を送る先が、地図 店.ctx のどこかを決められません
    17 |   知らせる(order: 注文, card: カード)
  = 地図が sakai の検査を通らないので、ファイルがどのコンテキストに属するかが分かりません（sakai の E009: パス "contexts/請求.ctx" がありません）。
  = `sakai check` が通るよう地図を直してください。
ritsu check: ファイル 6 個（proto 1、dandori 1、sakai 4）。検査を通らないもの 1 個（エラー 1 件、警告 1 件）。言語の境目: 確かめた 1 か所、決められない 1 か所
```

**捨てたもの**：

- **地図の外の相手（プロバイダー、Jev）も ritsu-cross で言うこと。** 地図が要らない判定まで `ritsu check` だけのものになり、`dandori check` を単体で使う人に届かない。
- **送ってよい関係を、印の付いた要素を公表された言語に持つ関係（`through`）に限ること。** より細かく言えるが、印はたいてい公表された言語の文書に付くので、ほとんど同じ答えになり、判定が sakai の公表された言語の決まりをもう一度書くことになる（ritsu の P4）。
- **`discloses` を `.ctx` に書くこと。** 送ると決めるのは呼び出しを書く人で、その差分と一緒に読めるところに置く。
- **利用者が書く実装（`lambda` など）を地図の外と数えること。** 値はフローの持ち主のコードにとどまる。数えれば、ほとんどのタスクが E905 になる。

### 16.9 口と型

口（3.2）に、次を足した。`Flows::sends` は dandori が、`Maps` は sakai が答える。

```rust
// ritsu-ports/src/flows.rs
/// Where a call of a flow sends what it gives, inside the project (X14, DESIGN 16.8). dandori
/// says the parties outside the project itself (its E906), so they are not here.
pub enum Destination {
    /// A file of the project that holds the other side: an OpenAPI document a task calls an
    /// operation of, a `.proto` a `connect` task calls, a rule called at its Connect service, a
    /// child `.flow`, a book, a dates file. As dandori reaches it.
    File(PathBuf),
}

/// A value marked secret, as the flow gives it, and where the mark is written.
pub struct Secret {
    /// The value as the flow writes it, down to the field that is secret: `account.number`.
    pub shown: String,
    /// The file that marks it (a `.proto`, an OpenAPI or AsyncAPI document, or the `.flow`
    /// itself, for `secret`), as dandori reaches it; the line of the mark; and the mark as the
    /// file writes it (`debug_redact = true`, `x-data-classification`, `secret`).
    pub marked_in: PathBuf,
    pub line: usize,
    pub mark: String,
}

/// One call of a flow that gives a secret value to something in the project (X14).
pub struct Send {
    /// The line of the call, from 1, and the task (or the rule) called.
    pub line: usize,
    pub task: String,
    pub to: Destination,
    /// Each secret the call gives, with the parameter that carries it.
    pub secrets: Vec<(String, Secret)>,
    /// The parameters the task says it discloses (`discloses`), with the reason written.
    pub disclosed: Vec<(String, String)>,
}

pub trait Flows {
    // …（rule_calls と crossings は前のまま）
    /// Every call of the flow at `file` that gives a secret value to a file of the project, read
    /// with the other languages through `ports`, when the flow passes dandori's check; else what
    /// the check says. None by default, until dandori answers it.
    fn sends(&self, file: &Path, ports: &Ports) -> Result<Vec<Send>, Vec<Said>> { … }
}
```

```rust
// ritsu-ports/src/maps.rs。sakai が答える（sakai の src/ports.rs）
pub struct MapFacts { pub file: String, pub contexts: Vec<String>, pub relationships: Vec<MapRelationship> }
/// One relationship between two contexts, as the `.ctx` of `from` writes it. `words`: `upstream`,
/// `downstream`, `shared kernel with`, `partnership with`, `separate ways from`.
pub struct MapRelationship { pub from: String, pub to: String, pub words: String, pub separate: bool, pub file: String, pub line: usize }

pub trait Maps {
    /// The map at `map` (a `.ctx` from the root), when it is a map and passes the stages of sakai's
    /// check that decide its contexts, their relationships and who owns what (its words, names and
    /// paths, and its owners); None when the file is a context file, not a map; else what those
    /// stages say. The stages after them (the references, the patterns, the mappings) do not change
    /// which context a file is in, and are the map's own check's to say.
    fn map(&self, root: &Path, map: &str) -> Result<Option<MapFacts>, Vec<Said>>;
    /// The context of the map at `map` that the file at `file` (from the root) belongs to, as
    /// sakai's check decides it (the context of the deepest entry of `owns` that holds it; a
    /// `layer`, a shared kernel and a published language must agree with it); None when the map
    /// does not cover the file, or covers it and gives it to no context.
    fn context_of(&self, root: &Path, map: &str, file: &str) -> Result<Option<String>, Vec<Said>>;
}
```

- `Flows::sends` は、dandori の検査を通るフローにだけ答え、通らないフローには検査のエラーを `Said` で返す。E906 はエラーなので、プロジェクトの外へ秘密を送るフローは、`sends` も通らない（`Err` の中に E906 がある）。`Destination::File` と `Secret::marked_in` のパスは、`RuleCall::rule` と同じく dandori が届くパス（フローのファイルのディレクトリに、`.flow` が書いたパスをつないだもの）である。`disclosed` は、そのタスクに書いた `discloses` の全部（秘密の値を渡していない引数のものも入る）。
- `Maps` は、sakai の検査の段 1 と段 2（構文、名前、パスと、属し方）だけで答える（sakai の DESIGN 16.4）。段 3 から後は、どのファイルがどのコンテキストに属するかを変えない。段 3 はほかの言語の口を要るので、全部を走らせると、`Joined::maps` が sakai の検査の全部を持つことになり、X14 の問い一つごとに地図の検査を全部走らせることにもなる。
- `ritsu-project` の `Joined` に `maps()`（sakai の `Engine`）を足した。ritsu-cross の X14（`src/egress.rs`）は、プロジェクトの `.ctx` のうち `map` が `Some` を返すものを地図とし、フローのファイルを `context_of` で地図に当て、`Flows::sends` の宛先と印のファイルも同じく当てる。関係があるかは `MapFacts::relationships` に、`separate` でない二つの向きのどちらかがあるかで決める（関係を言うのは sakai の決まりのままで、ritsu-cross はそれを並べ直さない）。判定は、`Send` の並びと `MapFacts` と `context_of` の答えを受け取る純粋な関数にした。
- 型の名前 `Send` は、`use ritsu_ports::Send` をしたモジュールでは std の `Send` を隠す。いまワークスペースに `ritsu_ports::*` の glob の use は無い。使うところは `ritsu_ports::Send` と書くか、読み替える（`use ritsu_ports::Send as SecretSend`）。

土台に足したもの（std だけ。4.19）：

```rust
// ritsu-base/src/secrets.rs
pub struct Kind { pub id: &'static str, pub name: Text, pub provider: &'static str }  // "aws-access-key-id", tr!("AWS のアクセスキー ID", "an AWS access key ID"), "AWS"
pub struct Found { pub line: usize, pub col: usize, pub kind: &'static Kind, pub shown: String, pub len: usize, pub test: bool }  // shown: "AKIA…"
pub const TEST_MARK: &str = "ritsu: test secret";
pub fn kinds() -> &'static [Kind];
pub fn scan(text: &str) -> Vec<Found>;  // in the order found; a language reports the ones whose `test` is false
pub fn mask(text: &str) -> String;      // every key in the text put as `shown` gives it: what a diagnostic shows of a line goes through this

// ritsu-base/src/urls.rs
pub fn is_loopback(host: &str) -> bool;
pub fn scheme_and_host(url: &str) -> Option<(String, String)>;   // None for a relative URL
pub fn plaintext(url: &str) -> Option<(String, String)>;         // `http`, `ws` to a host that is not the loopback
pub fn encrypted_form(protocol: &str) -> Option<&'static str>;   // 16.4's table

// ritsu-base/src/marks.rs
pub struct SchemaMark { pub keyword: &'static str, pub detail: String }  // ("x-data-classification", "PII, confidential")
pub fn schema_mark(format: Option<&str>, sensitive_data: bool, classification: Option<(&str, Option<&str>)>) -> Option<SchemaMark>;

// ritsu-proto：model.rs と load.rs
pub struct Extension { pub extendee: String, pub name: String, pub ty: Type, pub number: i64, pub line: usize }  // ProtoFile::extensions
pub enum Redaction { Direct { line: usize }, ByOption { option: String, value: String, file: String, line: usize } }
impl Protos { pub fn redaction(&self, file: &str, field: &Field) -> Option<Redaction>; }
```

### 16.10 テスト

- **ritsu-base**：`tests/secrets.rs` が、表の種類ごとに当たる値と当たらない値（短い、文字の種類が違う、前後が英数字に続く、`EXAMPLE` で終わる、一つの文字の繰り返し、`ritsu: test secret` の行）と `mask` を確かめる。`tests/urls.rs` はループバックと平文の判定、`tests/marks.rs` は印の三つの書き方と `sensitivity` の読み方、`tests/diag.rs` は見せる行の鍵を伏せることを確かめる。
- **ritsu-proto**：`tests/redaction.rs` と `tests/redaction/` の五つの `.proto`（読み手の golden の外に置いた）。`debug_redact` を直に書いたフィールド、カスタムのオプションの値で付けたフィールド（オプションと列挙を別のファイルに置き、import をたどる）、`debug_redact = false`、`extend` をメッセージの中に書いたもの。`extend` を読んでも、三つの読み手の golden（`tests/golden/` の三つ）は変わらない。
- **各言語の W901**：英語の材料（`W901_key_in_a_string`、`W901_key_in_a_comment`、`W901_test_secret`）と日本語の版（`W901_文字列の鍵`、`W901_コメントの鍵`、`W901_テスト用の鍵`。同じ鍵の値）を置き、英語と日本語の出力を golden にする。台帳の再現（英語と日本語）を `check_every` などが走らせる。rulec、chobo、geas は、鍵のある行をほかの診断に引用させて、出力に鍵が無いことも確かめる（`tests/secrets.rs`）。
- **dandori**：`tests/fixtures/secrets.flow`（英語）と `secrets.ja.flow`（日本語）が、印の三つの出どころ（`.proto` の直とカスタムのオプション、OpenAPI の三つ、`.flow` の `secret`）、レコード、リスト、文字列への埋め込み、`for`、`some`、`json`、規則の結果（秘密にならない）、`writeOnly` だけのプロパティと `internal`（印にならない）、`discloses` を一つのフローで見せ、`check` の英語と日本語のテキストと JSON、`Flows::sends` の答え（`secrets.sends.json`、`secrets.ja.sends.json`）を golden にする。`tests/fixtures/security` に、W901・W902・W904・E906・E007・E001 の変異と Argo の E050 を、英語と日本語の対で置いた（例の `payout` を一か所ずつ変えた変異も）。例と `tests/flows` の全部のフローは `sends` に空を返す。`tests/secrets.rs` が、`history encrypted` の生成物が、コーデックを渡すプログラムでは `tsc --strict`・`mypy --strict`・`go vet`（と `gofmt -l`）を通り、渡さないプログラムでは型の誤りになることと、`tests/encrypted/pay.flow` を Temporal の dev server で TypeScript・Python・Go から、ペイロードを base64 にするコーデックで走らせ、コーデックの無いクライアントで読んだ履歴に、入力の値も失敗の文も平文で残らないことを確かめる。
- **sakai**：台帳の W901・W902・W903 の英語と日本語の再現。webshop を一か所ずつ変えた変異 8 組（W901 のコメントの鍵と文字列の鍵、W902 の平文のサーバー、サーバー変数、Kafka と MQTT のブローカー、W903 の認証の無い操作と、認証の無いサーバーのチャネル）。`tests/security.rs` は、何も出ないこと（相対の `url`、`https`、ループバック、`x-ritsu-plaintext`、名前で暗号化が分からないプロトコル、文書と操作とサーバーの `security: []`、空の要件 `{}`、`webhooks`、公表された言語でない文書、`security` を持つ AsyncAPI の操作、`ritsu: test secret`、AWS の文書の例の鍵）を、英語と日本語の例の両方で確かめる。`tests/maps.rs` は、shop と webshop（と日本語の版）の地図の答えと、例の全部のファイルの `context_of` を golden にする。
- **ritsu-cross**：契約の文書の W901（`tests/secrets.rs`。`.proto` のコメント、YAML の文書の値、JSON の文書の値、YAML の `# ritsu: test secret`、`EXAMPLE` で終わる AWS の ID（何も出ない））。X14 の判定の単体テスト（`tests/egress.rs`。地図の外、関係の無いコンテキスト、同じコンテキスト、関係がある、`separate ways` だけがある、`discloses`、印が `.flow` の `secret`、印のファイルがどのコンテキストにも属さない、地図が検査を通らない）と、sakai の本物の口で答えさせる `tests/egress_map.rs`（英語と日本語の名前のプロジェクトを、英語と日本語で。golden は `tests/golden/egress/`）。`crates/ritsu/tests/codes.rs` が、ritsu の新しい三つのコードの再現を英語と日本語で走らせる。
- **突き合わせ**：替える前と後のバイナリを、同じ入力のコピーにかけた。
  - rulec・koyomi・chobo・yuen・geas の例とテストの材料の全部、rulec のコーパス、ritsu のテストのプロジェクト（`shop`、`通販`、`invoice`、`stockroom`）を、`night/2026-10-06` の先のバイナリと比べた。英語と日本語と JSON で 2,139 回走らせ、違ったのは新しい W901 の材料の 90 回だけだった。
  - dandori は、例と `tests/flows` と `tests/fixtures` の 102 本の `.flow` に、`check`（英語、日本語、JSON）、七つのプラットフォームの `build`、`scenarios`、`doc` をかけた。変わったのは、`plaintext` を書き足した問い合わせの例の二つの版（`.flow` が変わったので、生成物の頭のハッシュと `doc` の抜粋も）と、W902 が出る三つの材料（どのコマンドも、先に W902 を出す）の五本だけで、ほかの 97 本は一字も変わらなかった。
  - sakai の例と `tests/maps` の地図と変異の全部、`ritsu check ritsu.ctx` は、共通の型を足したあとと最後とで比べた。違ったのは、新しい変異の出力と、webshop（直す前に出た W903 の五つが、直したあとに消えた）と、`ritsu.ctx` の成果物の数（sakai の `src/security.rs` の分の 421 → 422）だけだった。

**偽の鍵。** 材料の鍵は、どの言語でも一つの偽の値にした。`AIzaSyD-ritsu-fake-key-for-tests-` のあとに `0` を 6 字並べた 39 字で、読めば偽と分かる（この文書には、鍵の形の値を一続きでは書かない）。Google の API キーは、GitHub の push protection の既定の対象でない（GitHub の文書の表）。ソースの `.rs` には一続きで書かず、台帳の再現は `concat!` で、テストは部品をつないで作る（`["AKIA", "Q7TF", …].concat()`）。一続きで持つのは、W901 の材料（七つの言語）と、生成する診断の一覧（`crates/*/docs/codes.md`・`codes.ja.md`、スキルの `codes.md`、geas の `explain --all` の golden）の 59 本だけである。リポジトリの中の鍵の形の値をこの一つに限ると、GitHub の secret scanning の知らせが来ても、どれが何かがすぐ分かる。知らせが出ないよう、`.github/secret_scanning.yml` を置き、`paths-ignore` で、偽の鍵を書いた材料と、生成する codes のページを外す。

### 16.11 例

- **dandori の `examples/payout`**（英語の版 `payout.flow` と、日本語の版 `payout.ja.flow`、契約 `specs/payout.proto`。日本語の版は、JSON の名前を日本語にした `payout.ja.proto` を呼ぶ）。売り手への支払いを、銀行の API（`connect`）で送り、エージェントが下書きした知らせを送る。契約は口座の番号と名義に `debug_redact` を付け、フローは口座の ID だけを運ぶ。どの検査も何も言わない形で、参照を渡して値を渡さないことを見せる。テストの変異が、番号を運ぶ形（W904）、エージェントに名義を読ませる形（E906）、`discloses` で通す形を作る。どのプラットフォームでも走る例として、ほかの例と同じくシナリオを全部のプラットフォームで突き合わせる。
- **dandori の `examples/inquiry`**：Temporal 版の Open Responses のエンドポイント（この例では Ollama）が `http://ollama.internal:11434/v1` で、W902 が英語と日本語の版で一つずつ出た。`plaintext "<理由>"` を書き足して、意図の書き方の例にした（README とサイトのこの例のページの抜粋も直した）。
- **sakai の `examples/webshop`**：三つの OpenAPI の文書（`payments/api/payments.yaml`、`shipping/api/shipping.yaml`、`ordering/api/ordering.json`）は `security` を持たず、公表された言語の操作の五つに W903 が出た。文書の終わりに、ベアラートークンの方式（`components.securitySchemes`）とルートの `security` を足した。受注の `createOrder` は、アカウントの無い客も注文できるように、わざとだれでも呼べるようにして、操作に `security: []` を書いた（JSON の `operationId` と同じ行に書き、前からある行の番号を変えない）。注文の状態を ID だけでだれでも読めるようにする形は、例として勧めにくいので選ばなかった。日本語の版（`webshop.ja`）も同じに直した。足したあとの三つの文書は、Redocly CLI 2.58.1（`redocly lint --extends minimal`）が正しいと言う（警告は servers と summary が無いことだけ）。
- **dandori のテストの材料** `agent_targets.flow`、`agents.flow`、`rule_connect_targets.flow`：E050 を確かめる材料で、`http://` の送り先に W902 が出る。golden を取り直した（材料は直していない。E050 を確かめる形を変えないため）。

### 16.12 まだやらないこと

- `x-data-classification` の `internal` を、外へ送る検査（E905、E906）だけの印にすること。
- AsyncAPI の文書の印を使うこと（いまは dandori が AsyncAPI を読まないので、印を読むのは OpenAPI だけになる）。
- rulec の `shape … proto|jsonschema` の入力の印を、規則の口で渡すこと。
- 入力をそのまま出力に運ぶ規則（`carry` や、入力を書く行）。規則の出力を秘密にしないので、そうした規則に秘密を渡すと、出力は秘密のまま、印が消える。規則の口に「この出力は、この入力をそのまま出しうる」を足せば言える。
- 子の `.flow` の出力の印（子の `outputs` の `secret`）を、親のタスクの結果に引き継ぐこと。いまは、親のタスクの結果に `secret` を書けば秘密になる。
- `dandori doc` の表と図に、`secret`・`plaintext`・`discloses`・`history encrypted` を見せること。
- 出典の URL（rulec、koyomi、yuen の `source … url`）が平文であること。`source fetch` が取ってきた本文を、そのまま固定することになる。
- 契約の文書の W901 を、単体の `sakai check` と `dandori check` にも出すこと。いまは `ritsu check` だけにした（16.1）。
- 契約の文書が `$ref` で読む、ほかの文書の鍵。ritsu-cross が調べるのは、言語が直に参照する文書だけである。
- OpenAPI の、ほかのファイルへの `$ref` で書いたパスの項のサーバー（W902）。
- `ritsu check` の警告を CI で失敗にするフラグ。
- 公開する操作のどれにも、許可の決まり（Cedar のポリシー、sekisho の `.gate`）があるかを確かめること。W903 は `security` を見るところまでで、その先は sekisho の側の言語をまたぐ検査になる。
- 生成器のほかの出力のうち、dandori のコメントに入るほかの文（`for … in …` の式の表示、規則の前提の文、`.proto` のファイルの名前）と、Argo の注釈の U+0085・U+2028・U+2029（dandori の DESIGN 7 章）。同じ形のテスト（行を終える五つの文字と、`</script>`、`---`、`]]>` を入れた材料）で確かめる。

### 16.13 調べたもの（2026-10-06）

| 何 | URL | 版・日付 | 読んだこと |
|---|---|---|---|
| protobuf の `debug_redact` | <https://protobuf.dev/news/2024-12-04/> | 2024-12-04 の告知 | 二つの印の付け方。C++ のデバッグの出力が v30 から伏せる |
| 同（descriptor.proto） | <https://raw.githubusercontent.com/protocolbuffers/protobuf/main/src/google/protobuf/descriptor.proto> | 2026-10-06 の main | `FieldOptions` の `debug_redact = 16`（「Indicate that the field value should not be printed out when using debug formats, e.g. when the field contains sensitive credentials.」）と、`EnumValueOptions` の `debug_redact = 3`（「fields annotated with this enum value should not be printed out」） |
| OpenAPI 3.2.0 | <https://spec.openapis.org/oas/v3.2.0.html> | 3.2.0（2025-09-19） | ルートと操作の `security`、空の配列、空の要件 `{}`、Server Object の `url` は相対でもよい |
| OpenAPI のフォーマットの登録簿 | <https://spec.openapis.org/registry/format/password.html> | 2026-10-06 に読んだ | `password`：「a string that hints to obscure the value」 |
| OpenAPI の拡張の登録簿 | <https://spec.openapis.org/registry/extension/x-data-classification>、<https://spec.openapis.org/registry/extension/x-sensitive-data> | 2026-10-06 に読んだ（提案は OAI/OpenAPI-Specification の discussion #4330） | 書く場所（Schema Object）、値の形、`category` と `sensitivity` の値 |
| AsyncAPI 3.1.0 | <https://www.asyncapi.com/docs/reference/specification/v3.1.0> | 3.1.0（2026-01-31） | Server Object の `host`・`protocol`・`security`、Operation Object の `security`、Security Scheme Object の `type`、サーバーのバインディングの名前の一覧 |
| AsyncAPI 3.0.0 と 2.6.0 | <https://raw.githubusercontent.com/asyncapi/spec/v3.0.0/spec/asyncapi.md>、<https://raw.githubusercontent.com/asyncapi/spec/v2.6.0/spec/asyncapi.md> | 3.0.0、2.6.0 | 3.0.0 は `protocol` の値を並べない。2.6.0 は暗号化する名前を含む一覧を持つ |
| JSON Schema 2020-12 | <https://json-schema.org/draft/2020-12/json-schema-validation> | 2020-12、9.4 | `writeOnly` の意味と、パスワードの例 |
| gitleaks の既定の規則 | <https://raw.githubusercontent.com/gitleaks/gitleaks/master/config/gitleaks.toml> | 2026-10-06 の master（3,209 行）。最新のリリースは v8.30.1（2026-03-21） | 16.3 の表の各規則、AWS の `.+EXAMPLE$` の allowlist |
| GitHub の secret scanning | <https://docs.github.com/en/code-security/secret-scanning/introduction/supported-secret-scanning-patterns> | 2026-10-06 に読んだ | プロバイダーのパターンと push protection の既定（Google の API キーは既定でない）、秘密鍵の非プロバイダーのパターン |
| Temporal のペイロードのコーデック | <https://docs.temporal.io/payload-codec>、<https://docs.temporal.io/failure-converter> | 2026-10-06 に読んだ。生成物が使う SDK は TypeScript 1.24.0・Python 1.33.0・Go v1.49.0 | コーデックで暗号化する。失敗の文とスタックトレースは既定では平文で、`encodeCommonAttributes` で通す |
| Step Functions の保存時の暗号化 | <https://docs.aws.amazon.com/step-functions/latest/dg/encryption-at-rest.html> | 2026-10-06 に読んだ | カスタマー管理の鍵は実行の履歴を暗号化し、`GetExecutionHistory` と `DescribeExecution` に `kms:Decrypt` が要る |
| Lambda durable functions のカスタマー管理の鍵 | <https://aws.amazon.com/about-aws/whats-new/2026/07/durablefunctions-cmk/>、<https://docs.aws.amazon.com/lambda/latest/dg/durable-encryption.html> | 2026-07-22 の告知。設定のページは 2026-10-06 に本文で読んだ | 関数の `DurableConfig.KMSKeyArn`、`GetDurableExecution` と `GetDurableExecutionHistory`（`IncludeExecutionData=true`）に `kms:Decrypt` が要る、実行は始まったときの鍵を使う |
| Argo Workflows の秘密 | <https://argo-workflows.readthedocs.io/en/latest/walk-through/secrets/> | latest（2026-10-06 に読んだ） | 秘密は Kubernetes の Secret を環境変数かボリュームで渡す |
| RFC 6761 | <https://www.rfc-editor.org/rfc/rfc6761.txt> | 2013 | 6.3 節：「The domain "localhost." and any names falling within ".localhost." are special」、名前の問い合わせはループバックのアドレスになると考えてよい |
