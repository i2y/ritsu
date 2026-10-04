# ritsu 設計文書

七つの小さな言語（rulec、dandori、koyomi、chobo、geas、yuen、sakai）を、一つのリポジトリの一つの処理系にまとめる。言語は七つのまま残し、土台（診断、二つの言語の文、出典、名指し、doc のページの枠、テストの共通部分）、単位の型、プロジェクトの読み込み、CLI、バージョン、Lean の層を一つにする。目的は、ある言語が確かめたことを、隣の言語が型の付いたまま受け取れるようにすることである。

名前は律（りつ）から取った。

この文書は段階 A（設計）で書き、段階 C の最初の部分で、作者が決めたこと（★だった項目）、yuen への改名（2.2）、作った土台の形（4.12）とテストの道具の形（10.9）を書き足した。段階 D の最初の部分で、口の実物（3.2）、rulec の言語をスレッドごとに持つこと（4.1）、rulec と dandori の `.proto` の読み手（4.13）、単位の型の実物（5.1、5.2）を書き足した。1 章の行数と数、1.4 と 12.5 の出力は、2026-10-03 にこの機械（macOS arm64、rustc 1.94.1）で、各リポジトリを読み、作業場所に写したものを走らせて取った。元のリポジトリでは何もビルドしていない。各リポジトリのテストの件数と時間のうち、ここで走らせていないものは、それぞれの最後の記録から引き、そう書いた。段階ごとの作業と完了の条件は PLAN.md にある。

## 0. 全体像

```
ritsu（バイナリ。CLI と LSP。ritsu-wasm はブラウザで動かすもの）
 ├── ritsu-cross    言語をまたぐ検査（7 章）
 └── ritsu-project  プロジェクトの読み込み、名前の解決、口のつなぎ（6 章）
      ├── rulec  dandori  koyomi  chobo  geas  yuen  sakai     言語のクレート（互いに依存しない）
      └── 土台の層
           ├── ritsu-base    診断、二つの言語の文、診断の台帳、CLI の表、ハッシュ、名指しとパス、出典、doc の枠、JSON
           ├── ritsu-units   単位の型（5 章）
           ├── ritsu-ports   口：言語のあいだで渡すものの型と、問いの形（3.2）
           ├── ritsu-proto   .proto の読み手
           └── ritsu-emit    生成先の言語ごとの書き出しの共通部分（9.2）
 ritsu-testkit：テストの共通部分。どのクレートも dev-dependency としてだけ使う（10 章）
```

### 0.1 芯：言語の境目で、証明を切らない

七つの言語は、それぞれ小さいから確かめられる。rulec は表の完全性と重なりを、koyomi は範囲のすべての日の計算を、chobo は勘定の境界がどの振替でも一度の書き込みの中で守られることを、dandori はワークフローが終わるときの案件の状態と、外部のデータを二度変えうるリトライを確かめる。

ところが、いまは言語の境目で、確かめたことが途切れる。境目は各ツールの CLI が出す JSON で、受け取る側は JSON の中の文字列と数しか見ない。rulec は `JPY` と `円` が同じ単位だと知っているが、dandori は二つを別の文字列として比べて断る。rulec は率の刻みを知っているが、dandori はそれを JSON Schema の説明の文から読み取っている（どちらも 1.4 に実際の出力を載せた）。koyomi は関数がとりうる値を全部数えられるのに、その集合を rulec の表の検査に渡す口が無い。

ritsu は、境目を JSON から型の付いた呼び出しに替える。ある言語が確かめたことを、隣の言語が前提として受け取り、その先を確かめられるようにする（7 章の検査）。そのために七つを一つの処理系にまとめる。入口が一つになることや、インストールが一回で済むことは、その結果としてついてくる。

看板の言い方は、次のとおりにする。README（F）の頭と、リリースの説明に使う。

- **Seven small languages, one toolchain. What one checks, the next can build on.**
- **七つの小さな言語を、一つの処理系で。ある言語が確かめたことを、隣の言語が前提にできる。**

### 0.2 前提

- **P1**：言語は七つのまま。構文、検査、承認する人に見せるページ、名前、拡張子は言語ごとに別々に保ち、一つのファイルに二つの言語を混ぜない。規則、期日、帳簿、ワークフロー、主張、要件、地図は、読む人と承認する人が違うからである。どれか一つの言語だけでも成り立つ（`.flow` を書かずに rulec だけを使える）。
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
| 土台：診断と `--lang ja`、二つの言語の文、診断の台帳と `explain`、CLI の表、ハッシュ、出典の固定と改正の検知、名指しとパス、doc のページの枠、JSON | 各言語の検査と参照インタプリタ |
| 単位の型 | 承認する人に見せるページの中身 |
| `.proto` の読み手、生成先の言語ごとの書き出しの共通部分 | 生成するコードの形（rulec の表の一行が一つの分岐になる形など） |
| プロジェクトの読み込みと名前の解決 | 各言語の README、DESIGN.md、PLAN.md、スキル、例 |
| CLI（`ritsu`）、LSP、wasm | 各言語のコマンド（`rulec` など。8.2） |
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
| 出典の写しと固定、改正の検知 | rulec `src/sources.rs` の 1,390 行、koyomi `src/fetch.rs` 581 と `src/sources.rs` の 326 行、yuen `src/fetch.rs` 762・`src/copies.rs` 323・`src/base64.rs` 74・`src/sources.rs` 305。計約 3,760 | `sources` 938 | rulec `src/sources.rs` 1,640（写しと表の突き合わせ、`source fetch`・`pin`・`outdated` を rulec の文で言う部分、単体テスト。移す前は全体で 2,142）、koyomi `src/fetch.rs` 455 と `src/sources.rs` 370（祝日の表を含む全体）、yuen `src/fetch.rs` 567・`src/copies.rs` 54・`src/sources.rs` 296（借りた出典を含む） |
| 名指し | yuen `src/names.rs` 422、sakai `src/naming.rs` 367。計 789。ほかにルートとパスの扱い（sakai `src/paths.rs` 268 など） | `naming` 496、`paths` 247 | 診断のコードと文（yuen `src/names.rs` 146、sakai `src/naming.rs` 142、sakai `src/paths.rs` 75） |
| `.proto` の読み手 | rulec `src/proto.rs` 1,358、dandori `src/proto.rs` 1,153、sakai `src/proto.rs` 1,080。計 3,591 | `ritsu-proto` 1,850 | sakai `src/proto.rs` 76。rulec `src/proto.rs` 569（列挙の別名、`shape` のパスがたどるフィールドの取り方、単体テスト）と dandori `src/proto.rs` 551（import の探し方、proto3 だけを読むこと、型の名前の解き方、単体テスト）。どちらも D.10 で ritsu-proto で読むようにした |
| JSON（依存の無い二つ） | rulec `src/json.rs` 456、geas `src/json.rs` 513。計 969 | `json` 577 | rulec `src/json.rs` 204（誤りの文、値の種類の名前、キーを並べて書き戻すこと、`--format json` の書き手）。geas `src/json.rs` 513 は残した（4.9） |
| doc のページの CSS | rulec、dandori、koyomi、chobo の四つ。計約 340 | `docpage` 139（ページの頭、配色、外の URL の確かめ） | koyomi と chobo は土台の配色を使う。rulec と dandori は、doc の出力を変えないために自分の CSS を残した（4.8） |
| 生成物の予約語 | rulec `src/backend.rs` の `words`（189 行）、koyomi `src/reserved.rs` 76、dandori と chobo の表。計約 300 | `ritsu-emit` の `words` 139、`copies` 276（ほかに `ident`・`lit`・`header` 155） | koyomi `src/reserved.rs` 23（出力先と表の組）。rulec と dandori は、`copies` に写した自分の表を読む（9.5） |
| wasm の境目 | rulec `src/wasm.rs` 194、dandori `src/wasm.rs` 142。どちらも「バッファの頭に長さを書く」同じ決まり | — | そのまま（F.5 で `ritsu-wasm` に一つにする。2.2） |
| テストの共通部分 | `tests/common`：koyomi 247、chobo 1,040（`mod.rs` 116、`runners.rs` 730、`servers.rs` 194）、geas 546、yuen 185、sakai 337。計 2,355。ほかに dandori と rulec のテストの中 | `ritsu-testkit` 1,169 | 言語ごとのもの（`tests/common`：koyomi 25、chobo 823、geas 299、yuen 142、sakai 209） |

テストの共通部分の中で重なっていたのは、自分を消す一時ディレクトリ（koyomi、chobo、geas、yuen、sakai と、dandori の `tests/examples.rs`）、時間を区切って子プロセスを走らせること（macOS に `timeout` が無いため。koyomi、geas、sakai）、golden と取り直し（chobo、geas、yuen、sakai と、koyomi、dandori、rulec のテストの中）、使い捨ての PostgreSQL のクラスタ（koyomi、chobo）、Chrome を探すこと（dandori、koyomi、chobo、geas。順は五つとも同じ）、Mermaid で図を描けるかを確かめること（dandori と chobo）である。どれも `ritsu-testkit` の一つになった（10.8、10.9）。rulec のテストは PostgreSQL を `PG*` の環境変数で受け取る形のまま、SKIP と段を `ritsu-testkit` にした。一時ディレクトリも、C のあとに `TempDir` にした（PLAN の 7.5）。

単位の表と有理数（rulec の `src/types.rs` の `CURRENCIES`、`money_unit`、`unit_info`、`unit_offset` と、`src/num.rs` の有理数）は、D.1 で `ritsu-units`（628 行。表と有理数と単位の型。テストは別のファイル）に移した。rulec の `src/num.rs` は 282 行から 165 行になり、丸めの五つの仕方だけが残った。口の型とトレイトは D.2 で `ritsu-ports`（706 行）に置き、各言語の `src/ports.rs`（rulec 493、koyomi 267、chobo 266、sakai 211、yuen 128、geas 93）が答える。

移す前の表の合計は、テストの共通部分を除いて約 1 万 4 千行だった（診断のうち言語に残る部分も含む）。土台に移せば 6 千行ほどになると見込んでいた。段階 C の終わりの土台の三つは 6,087 行（`ritsu-base` 3,654、`ritsu-proto` 1,846、`ritsu-emit` 587）で、見込みに近い。七つの言語の src は 163,913 行から 158,157 行になった（rulec 68,571、dandori 30,763、koyomi 15,764、chobo 11,070、geas 16,380、yuen 8,414、sakai 7,195）。土台が言語ごとの形のいちばん広いものを取り、単体テストも持つので、全体の行数はほとんど減っていない。減ったのは、同じ役目の実装の数である。段階 D の最初の部分のあとは、七つの言語の src が 158,439 行になった（D の前のコミットでは 158,226 行。rulec 68,648 から 68,447、dandori 30,755 から 30,161、koyomi 15,764 から 16,033、chobo 11,070 から 11,364、geas 16,380 から 16,482、yuen 8,414 から 8,544、sakai 7,195 から 7,408）。口に答える `src/ports.rs` の 1,458 行が増え、rulec と dandori の `.proto` の読み手が 1,383 行減った。rulec の診断と台帳と `tr!`（E）、二つの doc の CSS が、まだ言語の側に残っている。段階 D の二つ目の部分のあとは、七つの言語の src が 159,716 行になった（rulec 68,447 から 68,493、dandori 30,161 から 31,400、yuen 8,544 から 8,540、sakai 7,408 から 7,404。koyomi、chobo、geas は変わらない）。dandori では、rulec の JSON の読み手と子プロセスが消え（`src/rulec.rs` 678 から 542）、記録から答える口（`src/record.rs` 334）、`Items` と `References`（`src/ports.rs` 190）、診断の台帳（`src/codes.rs` 315）が増えた。コマンドの本体は `src/main.rs`（340 から 12）から `src/cli.rs`（145 から 559。`explain` を含む）に移った。入口の最小の形の `crates/ritsu` は 90 行である。段階 D の最後の部分のあとは、七つの言語の src が 162,148 行になった（rulec 68,493 から 68,529、koyomi 16,033 から 16,065、chobo 11,364 から 11,464、geas 16,482 から 15,934、yuen 8,540 から 10,605、sakai 7,404 から 8,151。dandori は変わらない）。geas の統一形式の差分の読み手は ritsu-base の `udiff`（642 行）に移り、geas の `src/diff.rs` は 734 行から 121 行になった（ritsu-base は 3,654 から 4,323。台帳の `Repro::Retired` を含む）。yuen では `affected`（`src/affected.rs` 854）、口のまとまり（`src/suite.rs` 75）、コマンドの関数（`src/run.rs` 351）が、sakai では口のまとまり（`src/suite.rs` 206）とコマンドの関数（`src/run.rs` 305）が増えた。口の `ritsu-ports` は 706 から 818 行（`Sources` と `Claims::affected`）、入口の `crates/ritsu` は 141 行（`ritsu yuen` と `ritsu sakai`）である。

キーワードの表（各言語の `kw.rs` や `syntax.rs`）、字句と構文、検査、参照インタプリタは重なりに数えない。言語ごとの語彙と意味そのものだからである。

### 1.3 同じ考えで、形が違うもの

- **診断の JSON のキー**：dandori は `code`・`severity`・`line`・`col`・`message`・`notes`・`path`（ファイルは外側に）、chobo は `v`・`column`・`title`・`excerpt`・`operations`・`hint`、geas は dandori の形に `file`、koyomi は `inputs`・`steps`・`fails`・`fix`、yuen は `diff`・`chain`・`candidates`、sakai は `references`。ファイルのパスは、yuen がルートからの相対、sakai が走らせたディレクトリからの相対で食い違っている（6.2 の 9）。rulec は別の形（`v` が 2 の形。`where`・`witness`・`rows`・`fix`）。
- **二つの言語の文の書き方**：rulec、koyomi、chobo、yuen、sakai は `tr!("日本語", "English")`（日本語が先）、geas は `t(en, ja)`、dandori は `Diag::error(code, line, col, en, ja)`（英語が先）。rulec の `tr!` は、プロセスで一つの言語（`src/i18n.rs` の `AtomicU8`）を読んで `String` を返す。ほかの四つの `tr!` は、二つの文を持つ `Text` を返し、どちらを出すかは出すところが決める。
- **SKIP の書き方**：`SKIP:` の行（dandori 55 か所、chobo 27、koyomi 11、yuen 6、sakai 5、geas 1）、rulec のテストの `注意:`（53 か所。rulec のテストは日本語で回すため）、`rulec test` の `skipped`。
- **ルートの決め方**：geas（`src/tree.rs`）、yuen（`src/project.rs`）、sakai（`src/paths.rs`）が、それぞれ「いちばん近い `.git` のあるディレクトリ」を探す。
- **言語と取り直しの環境変数**：`RULEC_LANG`、`DANDORI_LANG`、`KOYOMI_LANG`、`CHOBO_LANG`、`GEAS_LANG`、`YUEN_LANG`、`SAKAI_LANG`。取り直しは `<名前>_BLESS`。
- **名前の正規化**：rulec の DESIGN §1.1 は「識別子は NFC に正規化する」と書くが、src には無い。chobo は結合文字を含む名前を E001 で断る。

段階 C で、このうち二つの言語の文の書き方（七つとも `tr!("日本語", "English")`）、SKIP の書き方（七つとも `SKIP: <クレート>: <理由>`）、ルートの決め方（土台の `paths`）、`RITSU_LANG`（rulec のほかの六つ。rulec は D の二つ目の部分で読むようにした）、`RITSU_BLESS`（`ritsu-testkit` の golden を使う六つ。rulec のテストは自分の取り直しのまま）、chobo と sakai の診断の JSON のキーを、土台の形にそろえた（4.12、10.9）。rulec の `tr!` がプロセスの言語で `String` を返すこと、rulec と dandori の診断の JSON の形、名前の正規化は、まだそれぞれのままである。

### 1.4 境目で切れているもの

次の出力は、2026-10-03 に作業場所で取った。rulec は 0.22.1 のリリースのバイナリ、dandori は main をそのまま作業場所に写して作ったもの。

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
    rulec/  dandori/  koyomi/  chobo/  geas/  yuen/  sakai/                            元のリポジトリを履歴ごと（B）
    ritsu-project/  ritsu-cross/                                                        つなぎの層（E）
    ritsu/  ritsu-wasm/                                                                 入口（E、F）
    xtask/                 テストの段、SKIP の集計、変えたクレートの選び出し、依存の決まりの確かめ（C）
  proofs/                Lean の層（F で rulec の proofs/ をここへ移し、モデルを足す。11 章）
  skills/ritsu/          エージェント向けのスキル（F）
  .github/workflows/     CI（10.5）
  ci/skips/              CI のジョブが許す SKIP の一覧（10.3、10.5）
```

言語のクレートの中は、元のリポジトリの木をそのまま残す（README、DESIGN.md、PLAN.md、`tests/`、`examples/`、`tools/`、`docs/`、`skills/`、rulec と dandori の `website/`、rulec の `proofs/` と `experiments/`）。元の `.github/workflows/` も `crates/rulec/.github/` と `crates/dandori/.github/` に来るが、GitHub はそこにあるワークフローを走らせない。ritsu の CI は根の `.github/workflows/` に新しく書く（C.12 で書いた。クレートの中のものの扱いは 10.5）。

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
| `ritsu-cross` | 言語をまたぐ検査と、その診断の台帳（7 章） | 新しく書く |
| `ritsu` | バイナリ `ritsu`（CLI と `ritsu lsp`） | 新しく書く |
| `ritsu-wasm` | ブラウザで動かす wasm32 のモジュール | rulec と dandori の `src/wasm.rs` |
| `xtask` | 開発のための作業。公開しない | 新しく書く |

言語のクレートのパッケージの名前を変えないのは、コード（`use rulec::…`）、テスト（`env!("CARGO_BIN_EXE_rulec")`）、環境変数の名前を、B の段階で一つも直さずに済ませるためである。crates.io では `koyomi`（「Japanese calendar written in Rust」、0.4.0）と `yurai`（「Forensics-grade provenance explorer for AI models」、0.4.0）が別のクレートに使われている（2026-10-03 に crates.io の API で確かめた。`ritsu`、`rulec`、`dandori`、`chobo`、`geas`、`sakai`、`ritsu-base`、`ritsu-units` は空いていた）。中のクレートは公開しない（13.2）ので、名前がぶつかること自体は困らない。

ただし要件の来歴の言語は、取り込んだときの名前 yurai を、段階 C の最初に yuen に改めた。crates.io の `yurai` は AI のモデルの来歴を調べるツールで、扱うもの（来歴）が近い。中のクレートを公開しなくても、近い分野に同じ名前のツールが二つあれば、読む人が取り違える。`yuen` は crates.io で空いていた（2026-10-03 に確かめた）。改めたのは、ディレクトリ（`crates/yuen`。`git mv` で履歴を保った）、パッケージとバイナリの名前、環境変数（`YUEN_LANG`、`YUEN_BLESS` など）、文書と診断と golden、名指しのツールの語（6.2 の 2。`yuen "民法の期間.req" requirement 満了日_142条`）、書き出しの名前（ReqIF の `yuen.` の属性、PROV の名前空間、識別子のハッシュの頭の `yuen/1`）である。ファイルの拡張子 `.req` と、元のリポジトリ（`~/yurai`）の名前は変えていない。名前を含む例のファイル（yuen のテストの `payment` のコードと約款）はハッシュで固定してあるので、改めたあとのハッシュで固定と確かめた記録を書き直した。

### 2.3 バイナリ

- 言語のクレートは、いまの `[[bin]]`（`rulec` など）を残す。開発と、そのクレートのテストが使う。
- すべての言語をつなぐバイナリは `crates/ritsu` の `ritsu` だけである。
- リリースで配るのは `ritsu` 一つで、`rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`yuen`、`sakai` はそれを指すリンクにする。リンクの名前で呼ばれたら、その言語のコマンドとして、すべての口をつないで動く（8.2。E.2 で作った）。Cargo のバイナリの名前は、ワークスペースの中で重ならない（言語のクレートの `rulec` と、リリースのリンクの `rulec` は、作られる場所が違う）。
- 受け取る側（dandori、yuen、sakai）のクレートのバイナリは、D の段階からほかの言語を読めない（ほかの言語のクレートに依存しないため）。ほかの言語を読むところに来たら、`ritsu <言語>` で走らせるよう言う診断を出す。dandori のクレートのバイナリは、規則を使わないフローならいまと同じに動く。D の二つ目の部分で dandori をそうした。規則を読む口に、何も読まない口（`dandori::sources::NoRules`）を渡し、`use rule` のところで E005 が `ritsu dandori …` で走らせるよう言う。`ritsu dandori` は、D で先に作った入口の最小の形にある（8.6）。D の最後の部分で yuen と sakai もそうした。yuen のクレートのバイナリは何もつながない口の束（`yuen::suite::Suite` の空のもの）を渡され、ほかの言語のものを名指すプロジェクトには、`ritsu yuen` に同じコマンドを続けた形を言って exit 2 で終わる。sakai のクレートのバイナリも何もつながない口の束（`sakai::suite::Suite` の空のもの）を渡され、地図が規則、カレンダー、ワークフローを含めば、言語ごとに一つの E104 で `ritsu sakai` に同じコマンドを続けた形を言う（診断なので exit 1。sakai の DESIGN 4.1）。

E の最初の部分で、三つの断り方を一つの形にそろえた。どの言語も、ほかの言語を読むところに来たら、コードのある診断を、要る言語ごとに一つ、その言語を最初に要するところに出し、注に同じコマンドを `ritsu <言語>` で走らせる形を書いて（「<言語> のクレートのバイナリは、ほかの言語を持ちません。…」）、使い方の誤りとして exit 2 で終わる。コードは言語ごとのもので、dandori は新しい E018（最初の `use rule` で言い、規則を読めないことから起きるほかの診断は出さない。前は規則ごとの E005 と、それに続く E002 を出して exit 1 だった）、yuen は新しい E206（前はコードの無い文を標準エラーに出していた）、sakai はこれまでの E104（exit を 1 から 2 にした）である。exit 2 にしたのは、走らせ方の問題で、ファイルの誤りではないからである（読めないファイルや知らないフラグと同じ）。exit 1 のままにすると、`dandori check` を CI で走らせる人は、フローの誤りと、ritsu で走らせていないことを、終了コードで見分けられない。コードを付けたのは、`--format json` を読むエージェントが、文を読み解かずに、`ritsu <言語>` で走らせ直せばよいと分かるようにするためである。それぞれの DESIGN.md（dandori の 0.3、yuen の 3.1 と 6.2、sakai の 4.1 と 12.3）と台帳に書いた。

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

決まりは四つある。

1. 言語のクレートは、ほかの言語のクレートに `[dependencies]` で依存しない。テストのための `[dev-dependencies]` は許す（3.3）。
2. 土台の層は、言語にも、つなぎの層にも、入口にも依存しない。
3. 言語のクレートは、つなぎの層と入口に依存しない。
4. 外のクレートは serde_json だけ（`preserve_order` と `float_roundtrip`）。足すときは、この文書に理由を書く。

### 3.2 口

境目でやりとりするものは、全部 `ritsu-ports` の型にする。出す側の言語は、自分が確かめたことをこの型で出し、受け取る側はこの型で受け取る。どちらも `ritsu-ports` のトレイトを通すので、受け取る側は出す側のクレートを知らない。口のトレイトは出す側が自分で実装し（`rulec::Engine` が `Rules` を実装する、など）、`ritsu-project` がそれを作って受け取る側に渡す。

| 口 | 出す側 | 受け取る側 | 中身 |
|---|---|---|---|
| `Rules` | rulec | dandori、yuen、sakai、ritsu-cross | 入力と出力（名前、別名、単位の付いた型、範囲、率の刻み）、列挙（値の名前と、Connect のワイヤでの名前と番号）、ステートマシン（軸、行が受け付ける座標、遷移、書く値、held）、前提（`constraint`・`sum`・`length`）、Connect のサービスの形、生成したコードの呼び方、`rulec doc` が描いたもの。問いは「この範囲の値で、前提は必ず成り立つか」と「この入力の値をこの集合に限ったとき、表は完全で、重なりが無く、当てはまらない行が無いか」。評価は入力から出力 |
| `Dates` | koyomi | dandori、rulec、ritsu-cross | 関数、引数の型と範囲、カレンダーとデータの範囲、`at` の時刻と UTC オフセット、条件の名前と文。問いは「入力の範囲で、関数がとりうる値の集合」と「入力から値までの日数の最小と最大」。評価 |
| `Books` | chobo | dandori、sakai（doc のため。D.8）、ritsu-cross | 単位（ritsu の単位の型。D.9）、勘定と境界と断る理由、振替の種類（引数と単位、キー、仮押さえと有効期限、移動）、仮押さえのステートマシン。問いは「額がこの範囲のとき、どの操作が、どの理由で断られうるか」。評価（帳簿の状態を持つ） |
| `Claims` | geas | yuen、ritsu-cross | 主張の一覧（名前、行、書いたとおりの手順）。map の記録の読み方。差分が主張に何をもたらすか（geas の `affected`。D.7 で足した） |
| `Items` | 七つ全部 | yuen、sakai、LSP | 中のもの（種類、名前、行の範囲、定義の文）。6.4 |
| `References` | 七つ全部 | sakai、yuen、LSP | 参照（行、先の名指し、参照の仕方）。6.4 |
| `Sources` | rulec、koyomi | yuen | ファイルが宣言して写している出典（法令の ID と時点と条ごとの固定、文書のパスと url と固定）。検査を通るファイルにだけ答える。D.7 で足した（下の段落） |
| 索引（`Index`） | （口ではなく、`Items` と `References` の答えを持つもの） | yuen、sakai、ritsu-cross、LSP | 各言語の `Items` と `References` の答えをファイルごとに一度だけ尋ねて持ち、名指しで引く。E.1 で足した（6.4） |

どの問いの答えも、P5 の三つのどれかになる。値を尋ねる問い（koyomi の日付がとりうる値の集合など）は、その値か、決められない理由かの二つになる。

```rust
// ritsu-ports（段階 D の最初の部分で作った形）
pub enum Answer<E> { Holds, Fails(E), Undecided(Text) }   // 成り立つ、成り立たない例、決められない理由
pub enum Found<T> { Value(T), Undecided(Text) }            // 値を尋ねる問いの答え
pub struct Said { code, file, line, message: Text }        // 答えられないときに、その言語が言うこと

pub trait Rules {
    fn facts(&self, rule: &Path) -> Result<RuleFacts, Vec<Said>>;
    fn preconditions_hold(&self, rule: &Path, ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<Vec<(Precondition, Answer<Values>)>, Vec<Said>>;
    fn checked_over(&self, rule: &Path, input: &str, days: &DaySet) -> Result<Answer<Text>, Vec<Said>>;
    fn eval(&self, rule: &Path, inputs: &Values) -> Result<Values, RuleError>;
    fn doc(&self, rule: &Path, shown: &str, html: bool, lang: Lang) -> Result<String, Vec<Said>>;
}
// `..` は `&self, file: &Path`。`Dates`・`Books`・`Claims` の答えは `Result<_, Vec<Said>>` に包む（`Ledger` の `apply` と `balance` は `Result<_, Text>`）
pub trait Dates { fn facts(..) -> DateFacts; fn values(.., date) -> Found<DaySet>; fn days(.., date) -> Found<(i64, i64)>; fn eval(.., inputs) -> Vec<(String, DateValue)>; }
pub trait Books { fn facts(..) -> BookFacts; fn refusals(.., transfer, amounts) -> Found<Vec<(op, reasons)>>; fn open(..) -> Box<dyn Ledger>; }
pub trait Ledger { fn apply(&mut self, &BookCall) -> Result<BookOutcome, Text>; fn pass(&mut self, seconds) -> expired; fn balance(&self, account, args) -> Balance; }
pub trait Claims { fn claims(..) -> Vec<Claim>; fn map_record(..) -> Option<MapRecord>; fn affected(.., root, diff, diff_shown, records) -> Affected; }
pub trait Sources { fn sources(..) -> Vec<Source>; }   // Source { name, line, kind: Law { db, id, asof, pins } | File { path, url, pin } }
pub trait Items { fn items(&self, root: &Path, file: &str) -> Result<Vec<Item>, Vec<Said>>; }       // Item { naming, lines, text }
pub trait References { fn references(&self, root: &Path, file: &str) -> Result<Vec<Reference>, Vec<Said>>; } // Reference { line, target, how }
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
| `Rules`（rulec） | `facts`、`doc`、`eval`（参照評価器。生成したコードが入口で断る入力、つまり型と列挙と範囲と入力どうしの関係を破るものは断る）、`preconditions_hold` のうち入力どうしの関係（範囲の箱のいちばん厳しい角で決まる。成り立たなければその角が例） | `preconditions_hold` のうち並びの合計と長さの上限（問いが並びの長さの範囲を持たない）、`checked_over`（rulec の検査が日付の集合を軸に置く形を持たない。E の X3 (b)） |
| `Dates`（koyomi） | `facts`、`values`、`days`、`eval`（範囲のすべての入力で計算する） | 入力の組み合わせが koyomi の確かめる数を超えるとき、途中で計算が止まる入力があるときは、決められないと言う |
| `Books`（chobo） | `facts`、`open`（参照インタプリタの帳簿。操作、時間を進める、残高） | `refusals`（chobo の検査は額を決まった値でしか試さない。E の X4） |
| `Claims`（geas） | `claims`、`map_record`、`affected`（D.7） | — |
| `Sources`（rulec、koyomi） | `sources`（D.7。検査を通らないファイルには、その検査の診断を返す） | — |
| `Items`、`References` | rulec、koyomi、chobo（中のものだけ）、geas（中のものだけ）、yuen、sakai、dandori（D の二つ目の部分。D.6） | — |

**D の最後の部分で足したもの**（PLAN の D.7）。yuen が一式を口で読むために、口を二つ足した。

- `Sources`：yuen が借りる出典（規則とカレンダーが写して固定しているもの）と、E107 で比べる写しの固定を渡す。取り込む前の yuen は `rulec api` と `koyomi api` の `sources` を読むつもりだった。`Rules` と `Dates` の事実に入れずに別の口にしたのは、受け取るのが yuen だけで、`RuleFacts` と `DateFacts` を作るより軽く答えられ（rulec は規則を読み直さずに覚えたものから、koyomi は検査を通したあとで出典の行だけを読み直す）、検査を通らないファイルに答えないこと（その検査の診断を返すこと）が、yuen の E203 にそのままつながるからである。
- `Claims::affected`：差分（バイト列と、見せるときの名前）と記録から、geas の `affected` の答え（記録がどちら側のものか、触る主張と行、どの主張も走らせない行、消えるファイル、ソースでないファイル、spec と基準の変わり）を渡す。geas の `affected` のコマンドと同じ関数（`answer_for`）が答えるので、二つが食い違わない。

中のものの定義の文は、6.4 の表のとおりにした。rulec は `rulec fmt` が書く形の行、koyomi は `date … =` の塊の行と条件の行（コメントと前後の空白を除く）、chobo は yuen の DESIGN 3.2 の形の JSON（yuen の試作が計算したハッシュと同じになる）、geas は主張の塊の行、dandori はタスク・案件・レコードの宣言の塊の行（コメントと前後の空白を除き、文字列の外の続いた空白を一つにし、字下げは深さごとに空白二つに直す。dandori の DESIGN 0.3）である。表に無かった yuen は要件の端の中身（yuen の DESIGN 4.1）と出典の固定の行、sakai はコンテキストのファイルの行と語の塊の行にした。

### 3.3 テストと dev-dependency

受け取る側のクレートのテストは、出す側のクレートを `[dev-dependencies]` に持ち、本物の実装をつないで走らせてよい。口のトレイトは出す側が実装するので（3.2）、つなぎ方は一か所にしか無い。出す側は受け取る側に依存しないので、依存は輪にならない。

受け取る側のクレートのテストのうち、いまバイナリを走らせて規則などを読むもの（dandori の `tests/examples.rs` など）は、D の段階で、CLI を関数として呼ぶ形（`dandori::cli::run(引数, 口, 標準出力, 標準エラー)`）に替える。すべてをつないだバイナリを走らせるテストは、`crates/ritsu/tests/` に置く。

D の二つ目の部分で、dandori をそうした。D の最後の部分で、yuen も rulec、koyomi、chobo、geas、dandori、sakai を `[dev-dependencies]` に持ち、`ritsu yuen` と同じにつないで、コマンドを関数（`yuen::run::run`）として呼ぶようになった。sakai も同じで、rulec、koyomi、chobo、dandori を `[dev-dependencies]` に持ち、`ritsu sakai` と同じにつないで `sakai::run::run` を呼ぶ。例が一式から写したものを、前は一式のバイナリ（`SAKAI_RULEC` など）で確かめていたのも、口で確かめるようになり、ritsu-testkit の `Need::Suite` と CI の `SAKAI_*` の変数を消した。環境変数を変えて走らせるもの（テストの中の e-Gov に問う `source outdated`）は、ritsu のバイナリを走らせる `crates/ritsu/tests/yuen.rs` に置いた。dandori のテストは rulec を `[dev-dependencies]` に持ち、規則を `rulec::ports::Engine` から同じプロセスの中で読む。ライブラリを呼ぶテストは、読む口をスレッドに置いて（`dandori::sources::with_rules`）呼び、バイナリを走らせていたテストは `dandori::cli::run` を呼ぶ。テストが要る `rulec gen` の出力は、rulec の `gen` の本体をライブラリに移した `rulec::codegen::generate`（コマンドと同じ関数。出力は変わらない）で作り、`rulec vectors` の出力は `rulec::vectors` で作る。規則を読むだけのテストは、rulec のバイナリが要らなくなったので `fast` の段でも走る。

土台の層の `ritsu-proto` と `ritsu-emit` のテストは、言語の側が移るまでのあいだだけ、rulec（`ritsu-proto` は dandori も）を `[dev-dependencies]` に持ち、言語のいまの読み手と表を、土台のものと生のまま比べる（C.9、C.10）。決まり 2 の例外で、移したあとに残しておく理由は無い。言語の側が土台のものを使うようになるとき（表は C.11、読み手は D.10）に、比べる部分とその dev-dependency を消し、golden と比べるテストだけを残す。そのままにすると依存が輪になり、比べる相手も土台のものになって、比べる意味が無くなる。`cargo xtask deps` は dev-dependency を決まり 1〜3 の外に置くので、この例外はこの節で守る。

C.11 で rulec と dandori が `copies` の表を読むようになったので、`ritsu-emit` の比べる部分と rulec への dev-dependency を消した。`crates/ritsu-emit/tests/copies.rs` は、表を語の並びにしたものを `tests/golden/copies.txt` と比べるだけになった。D.10 で rulec と dandori が `ritsu-proto` で読むようになったので、`ritsu-proto` の比べる部分と、rulec と dandori への dev-dependency も消した。`crates/ritsu-proto/tests/readers.rs` は、三つの言語の形にしたものを `tests/golden/` の `sakai.txt`、`rulec.txt`、`dandori.txt` と比べるだけになった。これで、土台の層のクレートの dev-dependency に言語のクレートは無くなった。

### 3.4 決まりの確かめ方

- C の段階：`xtask` に、`cargo metadata` を読んで 3.1 の表と突き合わせる確かめを置き、CI の `fast` のジョブで走らせる。破れば落ちる。C.3 で `cargo xtask deps` として作った（10.9）。
- E の段階：この処理系そのものの地図 `ritsu.ctx` を sakai で書く。クレートをコンテキストに、`ritsu-ports` と `ritsu-units` と `ritsu-base` を言語のクレートの上流の公表された言語にし、言語のクレートどうしの参照を許さない。Rust では、`use` できるクレートは `Cargo.toml` の依存に限られ、コンパイラがそれ以外を断るので、境界を越える参照は `Cargo.toml` の依存として読める。sakai の決まり（sakai の P6「コードの import は、各言語の既存のツールの設定にして、そのツールで確かめる」）に合わせて、cargo-deny の `[bans]` の `wrappers`（そのクレートに依存してよいクレートを並べる）の設定を `sakai build --target cargo-deny` で書く形を第一の案にする。cargo-deny がワークスペースの中のクレートどうしの依存にも効くかは、E の段階で確かめる。効かなければ、sakai が `cargo metadata` の JSON を読む形にする。どちらでも `ritsu check .` が ritsu のリポジトリに地図を当て、CI で走らせる（7.13）。

### 3.5 捨てたもの

- **言語のクレートどうしが直接依存する形**（dandori が rulec に依存する）：rulec の中の型（AST、型付けの結果）が dandori に漏れ、rulec の中を直すたびに dandori が壊れる。口を挟めば、渡すものは決めた型に絞られる。
- **一つのクレートにまとめる形**：依存の決まりがモジュールの規約になり、コンパイラが守らない。
- **プロセスの境目を残し、JSON を型の付いた形式に直す形**：一つの処理系にする理由（型の付いたまま渡す）が無くなる。7 章の問い（「この集合で完全か」）は、渡すたびに子プロセスと JSON の往復になり、二つのツールのバージョンが違えば答えも食い違う。

## 4. 土台

`ritsu-base` に置くものと、その元。どれも言語の意味を持たない。

### 4.1 二つの言語の文

`Text`（日本語と英語の文の組）と `tr!("日本語", "English")` を一つにする。koyomi、chobo、yuen、sakai の形で、どちらの文を出すかは出すところが決める。同じプロセスの中で英語と日本語の golden を取れ、wasm では呼ぶたびに言語を変えられ、ほかの言語のクレートからも、欲しい言語で呼べる。

- geas の `t(en, ja)` と dandori の `(en, ja)` の組は、C の段階で `tr!("日本語", "English")` の順に直す。機械的な直しで、文は一字も変えない。
- rulec の `tr!` は、プロセスで一つの言語を読む（呼び出しは 2,800 か所）。C の段階ではそのままにする。D の段階で、言語をスレッドごとに持てるようにする（`i18n::with(lang, || …)`）。dandori や yuen が rulec を同じプロセスの中で、ほかのテストと並んで、違う言語で呼ぶからである。CLI の振る舞いは変わらない。D.5 で作った（rulec の §15.165）。`with` の中で作る文はそのスレッドではその言語になり、抜ければ（パニックで抜けても）前の言語に戻る。スレッドの言語はプロセスの言語に勝つ。rulec の `tests/lang.rs` が、英語と日本語の `rulec doc` を四つのスレッドで同時に描いて確かめる。D の二つ目の部分で、rulec も `RITSU_LANG` を読むようにした（下の順。rulec の §15.168）。rulec のすべての文を `Text` に移すことは、要るとわかるまでしない（15 章）。
- 言語の選び方は、`--lang`、`<名前>_LANG`、`RITSU_LANG`、英語の順。システムのロケールは見ない（rulec の §11 の原則 7。生成物と CI のログが機械で変わらないため）。
- 文の幅（East Asian Width で W と F を 2 と数える）、件数、日本語の空白の詰め方の小さな関数も置く。

### 4.2 診断

一つの `Diag` に、どの言語にも共通の部分を置く。コード、重さ（エラー、警告、備考）、ファイル、行、列、文、注（`= ` で始まる行）、直し方。テキストの形は、dandori、koyomi、chobo、geas、yuen、sakai がもう使っている形（`error[E301]: <ファイル>:<行>:<列>: <文>`、原文の行、注、そこに至る例）にする。

そこに至る例の部分は、言語ごとに中身が違う（1.2 の表）。そこは言語ごとの型にし、テキストと JSON への書き方だけを `Diag` が呼ぶ小さなトレイトで決める。

JSON は、キーを英語で固定し、`code`、`severity`、`file`、`line`、`col`、`message`、`notes`、`fix` と、言語ごとの部分のキーにする。`file` はルートからの相対で、JSON の外側に `root`（走らせたディレクトリから見たルート）を添える（6.2 の 9）。いまの形から変わるのは、chobo の `column`・`title`（→ `col`・`message`）、sakai の `file`（走らせたディレクトリから → ルートから）などで、変える言語の DESIGN.md に理由を書く（P6）。rulec の `--format json`（`v` が 2 の形）は変えない。rulec の診断は rulec の `src/diag.rs` に残し、`ritsu check` の JSON に入れるときだけ、共通のキーを外側に足す（8.3）。

テキストの中のパスの書き方は、6.2 の 8 のとおり。ファイルの場所は走らせたディレクトリから（渡されたとおりに）、文の中の名指しはルートからの相対で書く。

### 4.3 診断の台帳と explain

`Entry`（コード、重さ、一行の題、いつ出るか、直し方、最小の再現、隣に置くファイル、関連するコード）と、`find`、テキストと Markdown（コードごとのアンカーつき）と JSON の書き出し、そしてどの再現も自分のコードを出すことを確かめるテストの共通部分を置く。台帳の中身は言語ごとに残す。`docs/codes.md` と `docs/codes.ja.md` は、どの言語も `explain --all --format markdown` の出力そのものにする（いまもそうしている五つの形）。dandori の台帳と `explain` は D の二つ目の部分で足した（dandori の `src/codes.rs`）。dandori の診断コードの一覧は、サイトの一行ずつの表（`website/docs/reference/codes.md`）のままで、`explain` の Markdown にはまだしていない（サイトを ritsu に移す F で決める）。

ritsu 自身の台帳（言語をまたぐ検査のコード）は `crates/ritsu-cross/src/codes.rs` で、`ritsu explain` が引き、`crates/ritsu-cross/docs/codes.md` と `codes.ja.md` がその Markdown である（E.3。7.1）。

退いたコードは、台帳に残して引けるようにし、番号を使い回さない（7.10）。D の最後の部分で、再現の代わりに退いた理由と版を持つ形（`Repro::Retired`、`Entry::retired`）を足した。`explain` は再現の見出しの下に理由を書き、再現を走らせるテスト（`check_every`）は退いたコードを飛ばす。yuen の E204 と W201 が最初に使い、sakai の N101 が続いた。

再現をディレクトリで持つ台帳（sakai）では、再現を走らせるコマンドを `explain` が書く（`sakai check .` を走らせます）。D.8 で、`ritsu` で始まるコマンドは、言語のコマンドの名前を前に付けずに、そのまま書くようにした。ほかの言語の成果物を含む再現は、すべての言語をつないだ `ritsu sakai check .` で走らせるからである（sakai の DESIGN 5.2）。

### 4.4 CLI の表

koyomi、yuen、sakai、chobo の形を一つにする。コマンドとフラグを一枚の表に置き、`--help` の表示と引数の読み取りが同じ表を引く。知らないフラグ、閉じた集合の外の値、値の無いフラグ、繰り返せないフラグの二度目は exit 2（rulec の §12.1）。繰り返せるフラグは表に書く（yuen の `--map`）。終了コードは 0（問題なし、警告と備考だけ）、1（エラー）、2（使い方の誤り、読めないファイル、中の異常）。

rulec の `src/main.rs` の表は同じ決まりで作られているので、C の段階では動かさない（合うところだけ移す）。dandori は C の段階でこの表に移し、`--version` とコマンドごとの `--help` を足す。足すだけで、いまのコマンドとフラグは変えない。

### 4.5 ハッシュ

SHA-256（FIPS 180-4）を一つにし、`hex` と、先頭 16 桁の `short` を置く。FIPS の既知の値でテストする。chobo の ID の決め方（長さを頭に付けた部分を SHA-256 に通す）は chobo に残し、SHA-256 だけを土台から使う。geas の SHA-1（git の blob の ID）は geas にしか要らないので geas に残す。

### 4.6 出典の固定と改正の検知

rulec、koyomi、yuen の三つは、法令の写しを同じ場所と同じ名前で持つ（`sources/law/<法令の ID>@<時点>/<要素>.xml`。yuen の `src/copies.rs` の頭に、そうそろえたと書いてある）。そこで、次を一つにする。

- 引用の書き方から要素の名前を作る（e-Gov の `第143条第2項` → `MainProvision-Article_143-Paragraph_2`、`別表第一` → `AppdxTable_1`、附則。eCFR の節）。
- 写しの場所、写しの本文（タグを落とした文、項ごとの行）、固定（先頭 16 桁）、`.rule`・`.cal`・`.req` の固定の行の書き換え。
- 通信：`curl` を子プロセスで呼び、三度まで試す（二度めの前に 2 秒、三度めの前に 4 秒）。e-Gov の法令 API v2（`law_data` と `law_revisions`）、eCFR（全文と版の一覧）、GitHub の raw のファイル（rulec）、e-Gov の応答の base64。

`source fetch | pin | outdated` のコマンドは、それぞれの言語に残す（固定の行の書き方は言語の構文だからである）。`check` は通信しない。

一つになれば、同じプロジェクトの規則とカレンダーと要件が同じ条を引くとき、写しも一つで済む。yuen の E107（要件の写しと規則の写しの本文の食い違い）は、写しが一つなら起きにくくなり、二つあるときも同じ手続きで比べられる（7.11）。

### 4.7 名指しとパス

名指しの決まり（6.2）を一つの実装にする。いまは yuen の `src/names.rs` と sakai の `src/naming.rs` が別々に書き、同じ 36 行の `naming.tsv` を通している。その表は土台のテストに移す。

パスは、ルートの決め方（`--root`、無ければ最初に渡したパスの上でいちばん近い `.git` のあるディレクトリ、それも無ければ渡したディレクトリ）、`.` と `..` を字の上で畳むこと、表示のパス（走らせたディレクトリから）、ディレクトリを歩くときに飛ばす名前（`.git`、`target`、`node_modules`、`.venv`、`.geas`、`__pycache__`）を一つにする。geas、yuen、sakai が別々に持っているものである。

12.5 で、取り込んだ形（ルートが上にある形）で yuen を走らせたら、写しのパスを `../../crates/yurai/tests/…` と回り道で書いた（そのときの名前は yurai）。土台の表示のパスは、走らせたディレクトリからのいちばん短い相対にする（`paths::Shown`）。yuen の表示は、土台へ移す前に C.0 で直した（yuen の `Project::shown`）。

### 4.8 doc のページの枠

承認する人のページ（rulec、dandori、koyomi、chobo の `doc`。yuen と sakai は F で作る）の枠を一つにする。

- HTML：`<!doctype html>`、`lang`、viewport、ツールとバージョンを書く `generator`、題、一つの CSS。CSS は色の変数を明るい配色と暗い配色で持ち、`prefers-color-scheme` と `data-theme` で切り替える。ページの頭に、元のファイルのパスと SHA-256 の先頭 16 桁とツールのバージョンを書く。外のファイルを読まない（スクリプトも CSS も中に書く）。狭い画面で横にはみ出さない。
- Markdown：頭のコメント（`<!-- Generated by <ツール> <バージョン> from <パス> (sha256:…) -->`）。

ページの中身（rulec の表とカード、dandori の図とシナリオ、koyomi の月の表、chobo の残高）は言語に残す。色の値をそろえると見た目が少し変わるので、C の段階で golden とスクリーンショットを取り直し、変わったページを報告に並べる。

段階 C で土台の枠に移したのは、koyomi と chobo のページである（C.4、C.5）。rulec と dandori の `doc` は、C.11 で出力を一字も変えない決まりなので、自分の CSS と頭のまま残した。元のファイルとハッシュとツールを頭に書くこと（HTML の頭と Markdown のコメント）は、四つとも言語ごとのいまの書き方のままである。一つの書き方にすると全部のページが変わるので、生成物の頭（9.5）と同じく E でそろえる。土台に置いていたその部品（`stamp` と `markdown_head`）は、使う言語が無いので C.11 で消した。

### 4.9 JSON

土台は外のクレートに依存しない（P9）ので、診断や名指しの JSON は、std だけで書いた小さな JSON の値の型で書く。キーの順を保ち、整数を正確に持つ。元は rulec の `src/json.rs`（456 行）で、C.11 で rulec もこれに替えた。geas も依存の無い JSON の読み手（`src/json.rs`、513 行）を持つが、それは残した。geas が読むのは試す相手のプログラムの出力で、数を浮動小数点として比べ、指数（`2e3`）も読み、読んだ値を自分の形で書き戻す。土台の読み手は ritsu のツールが出す JSON のためのもので、指数と二度出るキーを断り、整数を正確に持つ（geas の DESIGN 13.1）。serde_json を使う五つは、土台の JSON を文字列にして読み直すか、そのまま埋め込む。

serde_json を土台に入れない理由は、rulec と geas が依存の無いことを保っているからである。rulec の README は、依存が無いので `cargo install --path .` が何も取ってこないと書き、rulec の DESIGN §12.1 は引数のパーサを入れない理由に、依存を足さない方針を挙げている。ritsu 全体のバイナリには、いまと同じく serde_json が入る。

### 4.10 `.proto` の読み手

`ritsu-proto` は proto3 を読む。`syntax`、`package`、`import`（`public` と `weak` も）、入れ子のメッセージ、フィールド（`json_name`、`optional`、`repeated`、`map`、`oneof`）、列挙と値と番号、サービスとメソッド（ストリームかどうか）、サービスとメソッドのオプション（dandori の `(dandori.v1.workflow)` など）、Protovalidate の規則（`buf.validate.field` の整数の範囲と `required`。CEL の式は文字列のまま渡し、rulec の `src/cel.rs` が読む）、`buf.yaml` の依存と `buf.lock` の固定（rulec）。Google の well-known types、`buf/validate/validate.proto`、`dandori/v1/options.proto` はファイルが無くても知っているものとして扱う。型の名前は protobuf の決まり（内側から外へ、package を一段ずつ）で解決する（sakai の形）。

読み手が一つになれば、同じ `.proto` を、rulec の契約の突き合わせ、dandori の `connect` と `implements`、sakai の境界を越える参照、yuen の端が、同じに読む（7.12）。

### 4.11 土台に置かないもの

キーワードの表、字句と構文、名前の解決、検査、参照インタプリタ、生成器の芯は、言語に残す。どれも言語の語彙と意味そのものである。

### 4.12 段階 C で作った形（`ritsu-base`）

段階 C の最初の部分で `ritsu-base` を作り、二つ目の部分（C.4〜C.8）で koyomi・chobo・geas・yuen・sakai を、最後の部分（C.11）で rulec と dandori の合うところをこれに移した。移したときに出力が変わらないよう、重なっていたコードのうちいちばん広い形を取り、言語によって形が違っていたところは、言語が選べるようにした。koyomi・yuen・sakai の `explain`（テキストと Markdown。再現の三つの形を一つずつ）と、koyomi の `--help`、yuen の `review --help` は、土台で組み直したものが一字も違わないことをテストで確かめている（`crates/ritsu-base/tests/ledger.rs` と `cli.rs`。比べる相手は、移す前のそれぞれの出力を `tests/golden/compat/` に写したもの）。

- `text`：`Text` と `tr!`（日本語が先）。`Lang::pick` は `--lang`、`<名前>_LANG`、`RITSU_LANG`、英語の順に読む（4.1）。文の出し方は二つある。yuen は書いたとおりに出し（`as_written`）、koyomi と sakai は英語の頭を大文字にし、日本語の中の英字のまわりに空白を入れる（`spaced`）。どちらを使うかは言語が決める。chobo の `{key}` の差し込み（`Text::sub`）、件数（`count`、`plural`）、幅（W と F を 2 と数える）も置いた。
- `diag`：`Diag<X: Extra>`。共通の部分（コード、重さ、場所、文、注、直し方）は土台が書き、言語ごとの部分（`Extra`）は、直し方の前に出す行（yuen のつながりと差分）、直し方の後に出す行（koyomi の計算の段、sakai の関わるもの、chobo の操作）、JSON のキー、文の出し方を自分で決める。テキストのファイルの場所（`file`）と、JSON に書くルートからのパス（`rel`）を別々に持つ（6.2 の 8 と 9）。行の無い診断は、JSON の `line` と `col` を `null` にする（移す前は、geas のほかに koyomi の E001 と yuen も `0` と書いていた。三つとも替え、geas の golden を一つ取り直した）。言語の部分は、JSON に `fix` のキーを持たないこともできる（`Extra::fix_key`。geas）。ファイルの無い診断（geas のコマンドラインの誤り）は、文に場所を書かず、JSON の `file` を `null` にする。直した行は、テキストでは前後の空白を落とし、JSON では渡されたまま書く（koyomi は字下げを含めて JSON に出している）。
- `ledger`：再現は三つの形のどれかにした（隣に置くファイルとまだ無い再現は、`Entry::beside` と `Entry::later` で台帳の項に足す）。ファイル一つと隣に置くもの（koyomi と yuen）、一つのディレクトリに置くファイルとそこで走らせるコマンド（sakai）、まだ無いもの。`check_every` は、全部の再現を一時ディレクトリに置いて走らせ、自分のコードが出たかを確かめる。
- `cli`：表の読み方の違いは二つあり、`Reading` で選ぶ。koyomi は `-5` を引数として読み、sakai と chobo は `--` で始まる値を受け付けない。chobo のために二つ足した。`--format=json` を一語として読むこと（`no_inline_values`）と、`--` で始まるものだけをフラグにすること（`single_dash_args`）である。使い方の行は、引数とフラグから組み立てるか、言語が手で書く（`Cmd.usage`）。何が誤りかは種類（`Misuse`）でも返し、言語が自分の文で言える（chobo）。`--lang` の説明（`lang_flag`）は、C.4 から `RITSU_LANG` を書く（`<名前>_LANG`、`RITSU_LANG`、英語の順）。
- `sha256`：`digest`、`hex`、`short`（先頭 16 桁）、`to_hex`。
- `naming` と `paths`：6.2 の決まりを一つの実装にした。何が悪いかは `ErrorKind` で返し（どの文字で起きたかも）、文は土台のものを一つ持つ。yuen と sakai はコードと文が違うので、移すときは種類から自分のコードと文を選べる。「含む」は二つ置いた。yuen の、自分自身を含まない `contains` と、sakai の、自分自身も含む `is_or_contains` である。ルートは、渡したパスを字の上で絶対パスにしてから `.git` を探す（sakai の形。yuen はシンボリックリンクをたどってから探していた）。歩くときに飛ばす名前は sakai の組（`.` で始まる名前、`node_modules`、`site-packages`、`__pycache__`、`target`）にした。表示のパスは、走らせたディレクトリからいちばん短い相対で書く（`Shown`）。
- `sources`：三つの中でいちばん広い形を取った。漢数字は百と千まで読み（koyomi と yuen の形。rulec は九十九までで、`第0条` も通していた）、本文は表の行と列も読む rulec と yuen の `xml_text`、条の読み下しは yuen の `article_lines` にした。固定の行の書き換えは三つ置いた。数字だけを替える `pinned`（koyomi と yuen の `source pin`）、診断が出す直した行の `fixed_pin_line`、コメントの前を空白二つにそろえる `pinned_spaced`（rulec の `source pin`）である。base64 は、標準の形だけを読む `base64_decode`（yuen）と、URL 用の文字やパディングの無い形も読む `base64_decode_lenient`（rulec と koyomi）。curl には `--compressed` を付けた（eCFR は付けないと 406 を返す。rulec が見つけたこと）。e-Gov と eCFR はベースの URL を持つ値（`Egov`、`Ecfr`）にし、ベースの URL をどの環境変数から読むかは言語が決める（`KOYOMI_EGOV` など）。本物の e-Gov と eCFR に問い合わせるテストは `RITSU_TEST_LEVEL=platforms` のときだけ走らせ、2026-10-03 に一度走らせて、民法 142 条と 29 CFR 1910.157 の本文がテストの写しと同じことを確かめた。
- `docpage`：色の役割（`bg`、`fg`、`dim`、`line`、`soft`、`panel`、`code`、`accent`、`ok`、`warn`、`bad`）と、その明るい配色と暗い配色の値（chobo と dandori の値）を置き、ページは自分の色を足せる。ページの頭（`html_head`）と、外の URL を読んでいないかの確かめも置いた。元のファイルとハッシュとツールを書く部分（`stamp`）と Markdown の頭のコメント（`markdown_head`）も置いていたが、rulec と dandori の `doc` は出力を変えずにはこれを使えず（どちらも頭をそれぞれの形で書いていて、golden が変わる）、ほかに使う言語も無いので、C.11 で消した（4.8）。
- `json`：キーの順を保つオブジェクト、正確な整数（`i128`）、書いた桁のままの小数。書き出しは serde_json の `to_string` と `to_string_pretty` と同じバイト列になる（エスケープも同じ）。読み手は rulec のもの（指数は読まない、同じキーが二度あれば止める、入れ子は 256 段まで）。止まったときは、位置と何が悪いか（`Problem`）と土台の文を返す。rulec は C.11 で、種類から自分の文を選ぶ形にして、誤りの文を前のまま保った。

### 4.13 段階 C で作った形（`ritsu-proto`）

C.9 で `ritsu-proto` を作り、sakai をこれに替えた（rulec と dandori は D.10）。sakai の読み手を元にし、4.10 の全部を読む。

- 一つのファイル（`read`）：要素ごとに行を持ち、どの要素のオプションも書いたまま持つ（名前、値の文、protobuf のテキスト形式として読んだ値）。宣言した `oneof`、`syntax` を書いた行も持つ。読めないときは、何が悪いか（`Problem`）と位置を返し、文は読んだプログラムの名前を渡して作る（知らない `syntax` を言う文だけが、プログラムの名前を言う）。
- オプションの木（`value::tree`）：dandori の読み方で、拡張の名前で引く。同じフィールドを二度書けば並びになり、整数は整数、`true` と `false` は真偽になる。小数は書いた桁のまま持つ（dandori は f64 に直す。いまのどの `.proto` にも小数のオプションは無い）。
- Protovalidate（`validate`）：rulec の読み方のまま。比べる値に効かない規則は名前だけを `unread` に残す。
- `buf.yaml` と `buf.lock`（`buf`）：rulec の読み方のまま。
- 多くのファイル：sakai の形（渡したファイルだけを読み、import の行き先と型の解決を持つ `load`）と、dandori の形（入口のファイルから import の先を何段でも読む `load_from`。中身を渡されたファイルは、その文を読む）の二つ。型の名前は、どちらも見えるファイル（自分、import した先、`import public` の先）だけから引く。
- 読んだものから言語が作るもの（rulec の列挙の値の別名、sakai の何も設定していない値、dandori の proto2 を断ること）は、言語に残した。
- 三つの読み手と同じものを読むことは、三つのリポジトリの全部の `.proto` と、三つの読み手が自分のテストに使っていた例で確かめる（`crates/ritsu-proto/tests/readers.rs`）。違うのは、rulec の読み手が行の頭にしか `package` を見つけないことと、壊れたファイルを途中まで読むことだけで、どちらも新しい読み手の方が多く読む（PLAN の C.9）。

**D.10 で rulec と dandori を替えた形**。二つの言語は、自分の `src/proto.rs` の読み手を消して `ritsu-proto` で読む。言語に残したのは、読んだものから自分が作るものだけである。

- rulec：`proto::read(パス, 中身)` が、`ritsu-proto` の読んだものから、規則が契約に尋ねるもの（package、import、列挙と値、メッセージと`shape` のパスがたどるフィールド、Protovalidate の規則）を取る。列挙の値の別名、`upper_snake`、整数の範囲（`int_bounds`）は rulec に残した。`src/proto.rs` は 1,358 行から 569 行になった。
- **rulec は、読めない `.proto` を途中まで読まない**（★、rulec の §15.166）。前の読み手は、読めない文を飛ばして読めたところまでを返していた。最後の `}` が欠けた契約は、そこまでの値で突き合わせて通り、値の行の `=` が抜けた契約は、その値が消えたと E032 で言っていた。いまは E013（「`order.proto` を読めません」）で止め、どこで何が読めないかを注に書く。コーパスと変異にもテストにも壊れた契約は無く、出力は 1,782 回とも変わらない。
- dandori：import をたどる部分は dandori に残した。ファイルを、ディスクからも、ブラウザで試すページが持つファイル（`Sources`）からも探すからである。たどった一つずつのファイルを `ritsu_proto::read` で読み、型の名前は `Protos` で、見えるファイル（自分、import した先、`import public` の先）だけから引く。proto2 と editions は断る。読めなかった import があるときは、引けない名前を書いたまま持つ（その import の先の型を、無い型として断らない）。`src/proto.rs` は 1,145 行から 551 行になった（1.2 の表の 1,153 行は元のリポジトリのもので、C で単体テストの一時ディレクトリを替えて 1,145 行になっていた）。読めないファイルの文の形が変わった（dandori の DESIGN 0.3）が、例と `tests/flows` の出力は 701 回とも変わらない。
- `ritsu-proto` の文：何が要るか（`Problem::Expected` の `what`）を、英語の語から二つの言語の文にした。日本語の文に英語の語が混ざっていた（「a name が要るところに」）のが、「名前が要るところに」になる。sakai の E106 の日本語の文はこれで変わるが、sakai の golden には当たるものが無かった。`tests/golden/sakai.txt` の一行を取り直した。
- 三つの読み手と生のまま比べるのはやめ、`tests/readers.rs` は三つの golden と比べる形だけを残した（3.3）。比べるのをやめる前に、替えたあとの rulec と dandori の読み方と生のまま比べて、三つの golden の全部と同じことを確かめた。

### 4.14 統一形式の差分（`ritsu_base::udiff`、D の最後の部分で足した）

`git diff` と `diff -u` が書く差分の読み手を、土台に一つ置く。geas の `affected`（差分がどの主張に触るか）と yuen の `affected`（差分がどの要件に触るか）が、同じ差分を読むからである。geas の `src/diff.rs` の読む部分（ファイルの前と後のパス、引用符で書いたパス、改名と追加と削除、`index` の行の blob、二進のファイル、ハンクとその行、「末尾に改行が無い」の行）と、ディスクのファイルが差分の前と後のどちらに合うかを決める部分（`Content`、`fits`、`other_side`）を、中身を変えずに移した。blob のハッシュ（git の blob の形）は geas に残した。geas はそれを `pub use` で使い、振る舞いは変わらない（geas の `affected` のテストの golden がそのまま通る）。yuen は、`.req` のどの行が変わったかを、ディスクのファイルが差分のどちら側かを決めてから読む（yuen の DESIGN 8 章）。

## 5. 単位の型

### 5.1 rulec の書き方を土台にする

`ritsu-units` は、rulec の単位の書き方と表をそのまま土台にする。

- 書き方：`<次元>[<単位>]`（`mass[kg]`、`length[cm]`、`duration[h]`）、お金は `money[<通貨>, incl_tax|excl_tax]`、率は `rate` と入力の `rate[step <刻み>]`、単位の無い数は `number`。
- 閉じた表：通貨（ISO 4217 の 31 のコードと `円`。`JPY` は `円` の別の綴り、`銭` は円の百分の一、`<コード>c` はその通貨の百分の一）、質量、長さ、面積、体積、時間（基準の単位への係数は正確な有理数）、温度と音量（順序だけの次元。℉ は一次式）。rulec の `src/types.rs` の `money_unit`・`unit_info`・`unit_offset`・`CURRENCIES` と、`src/num.rs` の有理数を移す。
- 決まり：通貨ごとに別の次元（為替の率はこの処理系の中に無い。rulec の §15.18）。換算は正確な有理数でだけ行い、整数にならない換算は断る（`1lb` は `mass[g]` に書けない）。

`ritsu-units` が答えるのは、二つの単位が同じか、正確に換算できるか、順序だけの次元か、決まった綴り（`money[円, incl_tax]`）は何か、である。演算の型の決まり（rulec の E103 の単位の混同、E048 の日付の足し算、E104 の丸め）は rulec の検査のもので、rulec に残す。

単位の型は、rulec の型が持たないものを二つ持つ。率の刻み（rulec ではいま入力の宣言にあり、型には無い。1.4）と、chobo の名前だけの単位（5.4）である。

```rust
// ritsu-units のスケッチ
pub enum Dim { Mass, Length, Area, Volume, Duration, Temperature, Sound, Money(Currency), Rate, Number, Count(String) }
pub struct Unit { pub dim: Dim, pub unit: String, pub tax: Option<Tax>, pub step: Option<Rat> }
```

**D.1 で作った形**。`ritsu-units` は、有理数 `Rat`（rulec の `src/num.rs` から移した。`i128` の分子と分母）、表 `table`（`CURRENCIES`、お金の単位を引く `money`、量の単位を次元と係数で引く `unit`、℉ のずれの `offset`、すべての綴りの `spellings`）、単位の型 `Unit` と `Dim`、`Tax`、綴りが単位でない理由の `Problem` を持つ。表の中身は rulec のものを一つも変えずに移した。スケッチから変えたのは次のことである。

- 通貨は、別の型にせず、表の綴りの文字列で持つ（`Dim::Money("円")`）。通貨の表は閉じているので、表に無い綴りは `Unit::money` が断る。
- `Unit` の `unit` は、書いたとおりの綴りを持つ（`money[JPY, incl_tax]` の `JPY` は `JPY` のまま）。どの言語も、出力は書いたとおりに出すからである。同じ単位かは `Unit::same` が表を引いて決める（`JPY` と `円` は同じで、`kg` と `g` は違う）。表の綴りにそろえた形は `Unit::canonical` で得る。
- rulec の綴りを読む `Unit::parse` と、書く `Display` を置いた。正確な換算は `Unit::convert`（有理数のまま）と `Unit::whole`（換算した値が整数のときだけ）、順序だけの次元かは `compares_only` である。
- 丸めの五つの仕方（rulec の `RoundMode` と `round_to`）は、rulec の意味（rulec の §7.3）なので、rulec に残した。

### 5.2 rulec

rulec の `Ty::Money`・`Ty::Qty`・`Ty::Rate`・`Ty::Number` は、中に `ritsu_units::Unit` を持つ形にする。振る舞いは変えない（D の段階で、コーパスの golden と証明書が一字も変わらないことを確かめる）。率の刻みは、入力と出力の型として `Rules` の口に出す。dandori は説明の文から刻みを読まなくなる。

**D.1 で変えたこと（★）**：`Ty` の中に `Unit` を持たせるのはやめ、`Ty` は書いたとおりの綴りを持ち続ける（`Ty::Money { cur, tax }`、`Ty::Qty { dim, unit }`）。単位の意味（次元、係数、ずれ）は、どれも `ritsu-units` の表から引く。`Ty` を `Unit` にするのは `Ty::unit(刻み)` で、`Rules` の口はこれで入力と出力の単位を渡す（率には、入力の宣言にある刻みを添える）。理由は二つある。率の刻みは rulec の型ではなく入力の宣言にあり（1.4）、`Ty::Rate` に刻みを入れると、刻みの違う二つの率が違う型になって、rulec の型の決まり（E103 など）が変わる。また、税の区別の無いお金の値（`500円` という書き方）はどちらの区別のお金とも合う（rulec の `unifies`）が、`Unit::same` は区別まで同じものだけを同じとする。`Ty` を `Unit` にすると、rulec の 19 のファイルの 129 か所の `Ty::Money` と `Ty::Qty` を、この違いを保ったまま書き直すことになり、得るものが無い。税の語を `incl_tax` と `excl_tax` のほかに書いた型（`money[円, foo]`）は、rulec が黙って通していた。その型には単位が無い（`Ty::unit` が None を返す）。D の二つ目の部分で、rulec がその型を E103 で断るようにした（rulec の §15.169）。

### 5.3 dandori

- `Ty::Num(String)` を `Ty::Num(Unit)` にする。`src/syntax.rs` の `UNIT_KINDS`（rulec の九つの次元を写した表）は `ritsu-units` から引く。
- 二つの単位が同じかは、単位の型で決める。1.4 の例（`money[JPY, incl_tax]` を `money[円, incl_tax]` に渡す）は通る。
- 同じ次元で違う単位（`mass[kg]` を `mass[g]` に渡す）は、いまと同じく E003 で断る。dandori の式は計算しない（dandori の P1）ので、境目で黙って換算すれば、`.flow` が計算をすることになる。換算は規則に書く。
- `src/model.rs` の `rate_unit` と `rate_per`（刻みを文字列で作って読む）は、単位の型の `step` に替える。
- 範囲の端に単位を付けて書けるようにする（dandori の DESIGN 7 章に残っていたもの）。`range >=1kg` は `mass[g]` の場所では 1000 で、整数にならない換算は rulec と同じく断る。
- 値はいまと同じく、宣言した単位で数えた JSON の整数として運ぶ。どのプラットフォームの生成物も変わらない。`int` は単位の無い数（rulec の `number`）のまま。

**D.4 で作った形**（D の二つ目の部分）。`Ty::Num` は `ritsu_units::Unit` を持ち、二つの数の型が同じかは `Unit::same` で決める（`Ty` の等しさを手で書いた）。型の綴りは `Unit::parse` で読み、`Display` で書いたとおりに出す。`UNIT_KINDS` は、ritsu-units が次元の語を知っているので、表ごと消した（次元でない語は前と同じ E002 と注、表に無い単位と税区分の誤りは、ritsu-units の言う理由を注にした E002）。率の `rate_unit` と `rate_per` は、単位の型の刻みを読む `rate_per(&Unit)` 一つにした。規則の型も、口が渡す単位の型のまま持つ。範囲の端は、単位を付けて書けば ritsu-units の換算（`Unit::convert`）で型の単位に数え、率は百分率を刻みで割る。整数にならない端と、型の次元に無い単位は E003 である。温度の単位（`℃`、`℉`）を字句で読めるようにした（型の `[` のすぐあとと範囲の端の単位としてだけ。名前の一部にはならない）。生成物は、例とテストのフローのどのプラットフォームでも一字も変わらない（PLAN の D.4）。

### 5.4 chobo

chobo の単位（`unit 個`、`unit 円`、`unit USD scale 2`）は、名前だけを比べる単位で、量はいちばん小さい単位で数えた整数である。これを次のように単位の型に載せる。

- 名前が通貨の表にあれば、`scale 0` なら `money[<通貨>]`、`scale 2` なら `money[<通貨>c]`（百分の一で数える）。`円` と `JPY` は `scale 0` だけ（百分の一は `銭`）。ほかの scale は chobo だけの単位にする。
- 名前が質量や長さなどの単位の綴り（`g`、`kg`、`L` など）で `scale 0` なら、その単位。
- それ以外は、名前だけの数の単位 `Count(<名前>)` にし、自分とだけ同じにする。rulec は規則のためにラベルの付いた数を捨てた（rulec の §15.11）が、chobo では、同じ数え方のもの（商品の `個` と予約の枠の `席`）を分けるのに要る。

chobo のお金の単位には、税込か税抜かの区別が無い。rulec の `money[円, incl_tax]` を chobo の `unit 円` の額に渡せるようにすると、一つの勘定に税込の額と税抜の額が入りうる。rulec が型で止めていること（E103）が、chobo の勘定で破れる。そこで、chobo のお金の単位に区別を書けるようにする（`unit 円 incl_tax`）。境目では区別が同じでなければ渡せない。区別の無い chobo の単位は、区別の無い額（rulec の `money[円]`）だけを受け取る。chobo の中の意味は変わらない。chobo の構文を一つ足すことになる（D.9 で作る）。

chobo の額は 0 から 2⁶³ − 1 までで、rulec の値は負にもなりうる（返金など）。これは 7.6 の検査が見る。

**D.9 で作った形**（D の最後の部分）。chobo の単位の行の最後に `incl_tax` か `excl_tax` を書ける（`unit 円 incl_tax`、`unit USD scale 2 excl_tax`）。chobo の `model::unit_type` が、単位を上の三つの決まりで ritsu の単位の型にする。上の決まりから一つ細かくしたのは、量の単位として読むのを、足し引きのできる次元（質量、長さ、面積、体積、時間）に限ったことである。温度と音の大きさは順序だけで足し引きをしない（5.1）ので、勘定のあいだを動く額にならず、名前だけの数え方にする。お金でない単位に税込か税抜を書けば、chobo の新しいコード E014 で断る（chobo の台帳に足し、chobo の DESIGN 1.2、3.1 に書いた）。`incl_tax` と `excl_tax` は chobo の修飾のキーワードになった。口の `BookUnit` は単位ごとに ritsu の単位の型（`unit`）を持ち、`BookFacts::unit(名前)` で勘定や引数の単位から引ける。`chobo api` は税込か税抜を書いた単位にだけ `tax` を出し、`chobo doc` は勘定の単位を `円 incl_tax` のように書く。帳簿の中の意味と生成するコード（TigerBeetle の ledger の番号も）は、区別を書いても変わらない。区別を書かない帳簿では、chobo のどの出力も変わらない（PLAN の D.9）。

### 5.5 日付、時刻、時間

- `date`：タイムゾーンを持たない暦の一日（rulec の §2.1、koyomi）。koyomi は日付を計算し、rulec と dandori は比べるだけ（rulec の E048）。
- `timestamp`：UTC の RFC 3339（dandori）。koyomi の `at` は、もうこの形で時刻を出している（koyomi の `api` の `wire`）。
- `duration`：秒で数える正確な長さ。rulec の時間の次元（`ms`、`s`、`min`、`h`、`d`、`w`）、dandori の `wait` と `timeout`、chobo の `expires after`（1 秒から 2³² − 1 秒）、koyomi の条件の `+ 60 days` を一つの型で比べる。koyomi は夏時間のあるタイムゾーンを断る（koyomi の P6）ので、koyomi の一日はいつも 86,400 秒である。7.7 の比べは、これを前提にする。
- koyomi の `int` の入力は `number`。

### 5.6 列挙

列挙は、どこで宣言したかで決まる：rulec の規則の列挙（規則と名前）、`.proto` の列挙（完全な名前）、dandori のフローの列挙、chobo の仮押さえの状態。rulec の列挙が `.proto` の列挙を取り込んでいれば（rulec 0.22.0 からの `connect.enums` の `alias` と番号）、二つは同じ列挙として口を渡る。

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
- 言語は `Joined` が一度だけ作る。rulec の `Engine`（確かめた規則を覚える）、koyomi・chobo・geas・dandori・sakai の `Engine`、その上の索引（6.4）である。`ritsu dandori` には規則の口を、`ritsu yuen` と `ritsu sakai` には索引を含む口のまとまりを、ここから渡す。D の入口では三つがそれぞれ rulec の `Engine` を作り、同じ規則を別々に読んでいた（PLAN の 7.8）。
- 一度だけ読むことの中身は、口の問いを同じファイルに一度しか問わないことである（索引、rulec の `Engine` の覚え書き、yuen と sakai の口のまとまりの覚え書き）。`ritsu check`（E.2）は、rulec の `check` の報告も rulec の `Engine` から作る。`Engine` は報告をファイルと中身で覚え、dandori や sakai がその規則の事実を尋ねたら、報告から検査を通ったかを読んで、規則を検査し直さない（パスの書き方が違っても同じファイルとみなす。`rulec/tests/ports.rs` が、コーパスの 50 本で、`check` と事実の二つを尋ねても検査は一回ずつであることを確かめる）。各言語の `check` の文は、頼まれた言語で一度作る。
- ファイルをまたぐ参照（上の 2）は `Project::references` が解く。各ファイルの言語が `References` で言う名指しごとに、行き着くファイル、そのファイルがプロジェクトのものか、行き着き方（`Landing`：ファイルが無い、索引が読むファイルならその言語の答え、`.proto` なら ritsu-proto で読んでその要素があるか、索引が読まないファイル）を返す。参照の誤りは、これまでどおり各言語が自分の `check` と自分のコードで言う（7.10）ので、これは言語をまたぐ検査（E.4）と LSP（F）が読む、プロジェクトの一枚の見取り図である。テストは、sakai の例を写した `crates/ritsu/tests/projects/通販` の参照の全部（`crates/ritsu-project/tests/golden/shop.references.txt`）を golden にする。

### 6.2 名指しを処理系全体のものにする

`ritsu-base` の名指しは、yuen と sakai の DESIGN.md の 2 章で決め、二つのリポジトリの `tests/fixtures/naming.tsv`（36 行の試しの表）で確かめているものを、そのまま処理系全体の決まりにする。決まりは次のとおり。

1. **形**：`<ツール> "<パス>" [<種類> <名前>]...`。組はツールの構造どおりに入れ子にできる。入れ子にできるのは、proto の `service S [method M]`、`message M [field f]`、`enum E [value V]`（入れ子のメッセージは名前を `.` でつなぐ：`message Order.Line`）と、rulec の `enum E [value V]` だけで、ほかのツールの組は一つまで。子の種類は、親の種類のすぐあとにしか書けない。
2. **ツールの語**：`rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`proto`、`file`、`yuen`、`sakai`。`dir` はツールの語にしない（sakai の `.ctx` の構文の語にとどめる）。
3. **パス**：`.req` や `.ctx` の中では、書いたファイルのディレクトリからの相対。区切りは `/` で、`.` と `..` は字の上で畳む。絶対パスと空のパスはエラー。`"."` はルートを指す。末尾の `/` は取り除く。JSON では、ルート（`--root`、無ければ最初に渡したパスの上でいちばん近い `.git` のあるディレクトリ、それも無ければ渡したディレクトリ）からの相対で、ルートの外に出るパスはエラー。
4. **種類の語**：rulec は `input`・`output`・`enum`（下に `value`）・`table`・`clause`・`define`・`derive`・`machine`・`source`、koyomi は `input`・`date`・`claim`・`source`、chobo は `unit`・`account`・`transfer`、geas は `claim`、proto は `service`（下に `method`）・`message`（下に `field`）・`enum`（下に `value`）、yuen は `requirement`・`source`、sakai は `context`・`term`、dandori は `task`・`case`・`record`（下に `field`）・`enum`（下に `value`）・`input`・`output`（6.3。D の二つ目の部分で足した）。`file` には無い。どの言語も、自分が使わない種類も名指しとして受け付ける。
5. **名前**：ツールの名前（JSON の `name`）。別名は使わない。語（空白、`"`、`#` を含まない一続きの文字。頭が数字でもよい）か `"…"` で書く。`"…"` の中のエスケープは `\"` と `\\` だけで、ほかはエラー。正規化せず、大文字と小文字を区別する。proto の名前は、そのファイルの package から見た名前。文字列の外の全角の空白、`"…"` で書いた種類やツールの語、名前の無い種類はエラー。
6. **同じ・含む**：同じは、ツールの語と、ルートからのパスと、組の並びが同じとき。ファイルは中のものを全部含み、親の組（proto の `service`・`message`・`enum`、rulec の `enum`）は子を全部含む。
7. **JSON の形**：`{"text": …, "tool": …, "path": …, "items": [[種類, 名前], …]}`（キーはこの順）。`text` は、パスをルートからの相対に直し、名前を語で書けるなら引用符なしで書いた形。空白を入れない詰めた書き方で、ASCII でない文字はそのまま出す。
8. **文の中の書き方**：診断などの文に書くファイルの場所（`<パス>:<行>:<列>`、写しのパス）は、走らせたディレクトリから、渡されたとおりに書く。文の中の名指しは、JSON と同じくルートからの相対で書く（読み直すと同じ名指しになり、`.req` や `.ctx` にそのまま貼れる）。
9. **JSON の中のファイルの場所**：診断の `file` なども、名指しと同じくルートからの相対にし、JSON の外側に `root`（走らせたディレクトリから見たルート）を添える。取り込んだときは、yuen がルートからの相対、sakai が走らせたディレクトリからの相対で食い違っていた。土台で一つにするときにそろえる（4.2）とし、C.8 で sakai をルートからの相対にした。

この決まりを、処理系のどこでも使う一つの書き方にする。yuen と sakai の `.req` と `.ctx` の中、診断の文と JSON、LSP の「定義へ移る」、`ritsu check` の JSON の中のもの、のどれも同じ形で書き、読み直すと同じものを指す。ツールの語は九つのまま。`ritsu` はツールの語にしない（ritsu は言語ではない）。

### 6.3 種類の語を足す

名指しの決まり（6.2）では、dandori に種類の語が無かった（dandori が中のものを JSON で出していなかったため）。D の段階で、次を足す。

- dandori：`task`、`case`、`record`（下に `field`）、`enum`（下に `value`）、`input`、`output`。

`naming.tsv` の `dandori "order.flow" task reserve` の行（`ERROR: dandori has no kinds yet` だった）は、JSON の行に変わる。入れ子の決まり（子の種類は親のすぐあと）は proto と rulec と同じにする。geas の `target` など、ほかの言語の種類を足すのは、使う側が要るとわかってからにする。

D の二つ目の部分で、これを足した（PLAN の D.6）。表は、dandori の行を JSON にし、入れ子の行（`record 予約 field 泊数`、`enum Outcome value awaiting_review`）と、入れ子の誤りの行（親のすぐあとでない `value`、`task` の下の組、`record` の下の `field` でない組）と、種類の無い `file` に種類を書いた行（`dandori has no kinds yet` の行が受け持っていた誤りの種類）を足して、42 行（名指し 24、誤り 18）になった。

### 6.4 索引：中のものと参照

各言語は、`Items` と `References` の口（3.2）で、自分の中のものと参照を出す。`ritsu-project` は、それをプロジェクト全体の索引にする。

**中のもの**（`Items`）は、種類、名前、行の範囲、定義の文を持つ。定義の文は、yuen が端のハッシュを取る元で、何を定義の文にするかは各言語が自分の DESIGN.md に書く。案は次のとおり。

| 言語 | 種類 | 定義の文 |
|---|---|---|
| rulec | `table`、`clause`、`define`、`derive`、`input`、`output`、`enum`、`machine`、`source` | そのものの行を `rulec fmt` が書く形にしたもの（表なら見出しから最後の行まで） |
| koyomi | `date`、`claim`、`input`、`source` | `date … =` の塊の行（操作の行を含む）、条件の行 |
| chobo | `unit`、`account`、`transfer` | yuen の DESIGN 3.2 の形（`chobo api` の一つから名前とコードを除いたもの）を土台の JSON で |
| dandori | 6.3 の種類 | タスクや案件やレコードの宣言の塊の行（列挙、フィールド、入力、出力はその行、列挙の値はその名前） |
| geas | `claim` | 主張の塊の行 |
| proto | `service`、`method`、`message`、`field`、`enum`、`value` | yuen の DESIGN 3.4 の決まった形の文 |

yuen の端は、いまはファイル全体のもの（rulec、koyomi の日付、geas、dandori）がある（yuen の DESIGN 3.2）。定義の文が出れば、表や日付の関数やタスクの一つ一つが端になる（7.10）。端の中身が変わるので、yuen のテストと例の確かめた記録（`.req` のハッシュ）は D の段階で取り直す。

**参照**（`References`）は、参照のある行、先の名指し、参照の仕方を持つ。dandori の `use rule … from`（同梱、Lambda、Connect の URL、`local`）、`use proto|openapi|smithy`、`connect`、`implements`、子の `flow "…"`、rulec の `import proto`、`shape`、`source … file`、koyomi の `use calendar` と `source`、yuen と sakai の名指しを出す。sakai はこれで全部の言語の参照を行番号つきで確かめ（7.10）、yuen は `trace` と `affected` でたどる。

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

**E.3 で作った形**（PLAN の E.3）。台帳は ritsu-base の `ledger` で書き、番号を帯で分ける。E1xx はプロジェクトのファイルを ritsu が読むところ、E2xx は言語の境目の検査（7.2 の X1〜X7。E の二つ目の部分から）である。E の最初の部分で載せたのは、次の一つである。

| コード | いつ出るか |
|---|---|
| E101 | プロジェクトの `.proto` を、ritsu の一つの読み手（ritsu-proto）が読めない。どの言語もこの読み手で読むので、どの言語からも読めない。読む言語は、読むところで自分のコードでも言う（rulec の E013、dandori の E016、sakai の E106、yuen の E205）。どの言語も読まない `.proto` は、これが無ければ誰も言わない |

ritsu の台帳に X10（名指しの解決）のコードは無い。名指しを書いた言語が、自分のコードで言うからである（7.10）。どのコードも、再現（小さなプロジェクトのファイル）を持ち、`crates/ritsu/tests/codes.rs` がそれを一時ディレクトリに置いて `ritsu check .` を英語と日本語で走らせ、見出しが `[ritsu <コード>]` の診断が出ることを確かめる。`ritsu explain` は、ritsu-base の台帳の書き方で、テキスト、Markdown、JSON を出す。言語のコードを渡されたら、`ritsu <言語> explain` で引くように言って 2 で終わる。`crates/ritsu-cross/docs/codes.md` と `codes.ja.md` は `ritsu explain --all --format markdown` の出力そのもので、`crates/ritsu-cross/tests/codes.rs` がそれを確かめる。

### 7.2 一覧

| | 検査 | 示すこと | 読むもの（口） | 確かめる場所 | 段階 |
|---|---|---|---|---|---|
| X1 | 境目の単位 | 境目を越える値の単位が、単位の型の上で同じ | `Rules` の型、chobo の単位、koyomi の型、dandori の型 | dandori の呼び出しと受け取り | D |
| X2 | 規則の前提を、呼び出しの場所で | フローが渡しうるどの値でも、規則の入力の範囲と前提が成り立つ | dandori の値の範囲、`Rules` の前提と問い | dandori の規則の呼び出し | E |
| X3 | 期日の値の集合を、規則の入力の範囲に | koyomi の関数がとりうる値が、規則の入力の範囲に収まる。規則がその集合を範囲として宣言したら、その集合の上で表を確かめる | `Dates` の値の集合、`Rules` の問い | dandori の呼び出し、rulec の入力の宣言 | E |
| X4 | 規則の出力から、振替の額へ | 振替に渡る額が chobo の受け取れる範囲に収まり、その額で断られうる理由がこれだけだと言える | `Rules` の出力の範囲、`Books` の問い、dandori のタスクのエラー | dandori の振替の呼び出し | E |
| X5 | 仮押さえの有効期限と、待ちの長さ | 「必ず期限が切れる」か「期限は切れない」 | `Books` の有効期限、`Dates` の日数、dandori の待ちとタイムアウト | dandori の、chobo の仮押さえに従う案件 | E |
| X6 | koyomi の関数を呼ぶ | 渡す日付が、関数の入力の範囲とデータの範囲に収まる | `Dates`、dandori の値の範囲 | dandori の期日の呼び出し | E |
| X7 | 一つの参照インタプリタ | （証明ではない）規則の評価、期日の計算、帳簿の動きを含めて、フローを一度に流す | `Rules`・`Dates`・`Books` の評価 | `ritsu run` | E |
| X8 | yuen の端を一つずつ | リンクの端が、表、日付の関数、タスクの一つ一つになる | `Items`、`Claims` | yuen の `.req` | D |
| X9 | sakai の参照を全部の言語で | dandori を含む全部の言語の参照が、宣言した関係と公表された言語を通る | `References`、`Rules` の列挙 | sakai の `.ctx` | D |
| X10 | 名指しの解決 | どこに書いた名指しも、プロジェクトの中のものを指す | `Items` | 全部 | D |
| X11 | 同じ条の写し | 同じ法令の同じ条を、規則、カレンダー、要件が同じ本文で写している | 土台の出典 | rulec、koyomi、yuen | D |
| X12 | 一つの `.proto` の読み方 | 同じ `.proto` を、どの言語も同じに読む | `ritsu-proto` | rulec、dandori、sakai、yuen | C〜D |
| X13 | 処理系自身の依存 | ritsu のクレートの依存が 3.1 のとおり | sakai の地図、Cargo の依存 | ritsu のリポジトリ | E |

### 7.3 境目の単位（X1）

dandori が規則に渡す値、規則から受け取る値、chobo の振替に渡す額、koyomi の関数に渡す日付と受け取る日付や時刻の単位を、単位の型（5 章）で比べる。同じ単位の別の綴り（`JPY` と `円`）は同じ、同じ次元の違う単位（`kg` と `g`）と、税込と税抜は違う。いまは dandori が自分の文字列の比べ（E003）でしていることを、単位の型の上でする。D の段階では dandori の E003 のまま出し、E の段階で、chobo と koyomi の境目も加えて ritsu の台帳のコードに移すかを決める。

### 7.4 規則の前提を、呼び出しの場所で（X2）

rulec は、形だけでは表せない入力の前提を `api` の `preconditions` に出している（rulec の §15.116）。種類は三つ：入力どうしの関係（`constraint`）、並びの合計の上限（`sum`）、並びの長さの上限（`length`）。いまの dandori は、これを読むだけで確かめない（dandori の DESIGN 7 章）。規則の生成したコードは入口で断るので、前提を破る値は、それを作ったタスクの結果が実行の履歴に残ったあと、規則を呼ぶところで初めて落ちる。

ritsu では、呼び出しの場所で次のように確かめる。

1. dandori は、規則の入力に渡す値の範囲を求めている（dandori の DESIGN 1.3。変数の範囲は、入れるすべての値の範囲を合わせたもの）。
2. ritsu-cross は、入力ごとのその範囲と前提を `Rules` に渡して尋ねる。rulec は、範囲のどの組み合わせでも一次の不等式が成り立つかを、もう持っている消去の手続きで決める（`src/fourier.rs`。rulec の §15.139 と §15.141）。
3. 成り立てば何も出さない。成り立たない値があればエラーで、その値と、そこに至るフローの実行を添える。決められなければ警告で、生成したコードが、その値を作ったタスクの直後で前提を確かめる（どのプラットフォームでも。dandori の DESIGN 7 章の残り）。

並びの合計と長さは、dandori がリストの長さの上限を知っているときだけ決められる。いま知っているのは、`for … at most n` で集めたリスト（長さは n まで）と、リストのリテラルである。タスクの結果のリストには長さの範囲が無いので、決められない（警告と、実行時の確かめ）。リストに長さの範囲を書けるようにするかは、E の段階で dandori の DESIGN に決める。

### 7.5 期日の値の集合を、規則の入力の範囲に（X3）

koyomi は、関数を範囲のすべての入力で計算するので、とりうる値の集合が正確にわかる。rulec は日付の足し算を持たない（rulec の E048）が、日付を入力に取る表の完全性と重なりを、範囲の上で確かめられる。二つをつなぐ。

koyomi の例 `支払_20日締め翌月10日払い.cal`（受領日の範囲は 2026-01-01〜2027-11-20 の 689 日）で、`koyomi vectors` の全行から数えると（2026-10-03）：

- `支払日` がとる値は 23 通り、2026-02-10 から 2027-12-10 までで、どれも月の 8 日、9 日、10 日のどれか。
- 受領日から支払日までは、最短 18 日、最長 51 日。

この支払日を入力に取る規則を書くと、いまの rulec では入力に `range >=2026-02-10 <=2027-12-10` を書き、その 669 日のすべてで表が完全でなければならない。koyomi の集合を渡せば、確かめるのは 23 日だけで済む。

二つの形にする。

- **(a) 呼び出しの場所で**：dandori が koyomi の関数の結果を規則の入力に渡すとき、値の集合が規則の入力の範囲に収まるかを確かめる。書き足すものは無い。
- **(b) 規則の宣言で**：規則が入力の範囲を koyomi の関数の値の集合として宣言する（たとえば `支払日 : date  range from koyomi "支払条件.cal" date 支払日`。rulec の構文を一つ足す。書き方は E で決める）。rulec は、その入力の値をその集合に限って、完全性、重なり、当てはまらない行を確かめる。rulec がもう持っている、宣言した `constraint` で尋ねる組み合わせを絞る仕組み（rulec の §15.55、`RulecCert/Sieve.lean`）と同じ形で、日付の軸に集合を置く。証明書には、集合の出どころ（koyomi のファイルと関数と SHA-256）と集合を書き、Lean の再検査も集合の上で通す（11 章）。

(b) は、rulec が `.proto` の列挙の値を契約から読むのと同じ考えである（rulec の §15.59。値の集合を決めるのは別の成果物で、rulec はそれを読んで表と突き合わせる）。rulec のセルの中では日付を計算しない。koyomi の集合を読めないとき（ファイルが無い、koyomi の検査を通らない、`rulec` を単独のクレートのバイナリで走らせた）は、範囲全体で確かめ直すのではなく、エラーで止める。

### 7.6 規則の出力から、振替の額へ（X4）

dandori が規則の出力を chobo の振替の額に渡すとき、次を確かめる。

- 額が chobo の受け取れる範囲（0〜2⁶³ − 1）に収まる。規則の出力が負になりうるなら（返金など）エラーで、その出力を返す規則の入力の例を添える。chobo は範囲の外の額を、断る（業務の結果）ではなく失敗にする（chobo の DESIGN 1.5）からである。
- その額の範囲で、どの操作がどの理由で断られうるか。いまの chobo の検査は、額に 1、2、3、5、10、100、1000 を試し、入れる振替を三つ先までたどって、例が見つかった理由だけを並べる（chobo の DESIGN 3.1 と 11.1。手数料が 0 のときだけ起きる理由が漏れる例がある）。ritsu では、試す額を規則の出力の範囲（表の出力なら値の集合）から選ぶ。そうして見つかった理由と、dandori のタスクが宣言したエラーを突き合わせる。起きうる理由を処理していなければエラー、起きない理由を宣言していれば警告。たどる深さは chobo の探し方のままなので、「起きない」と言えるのはその深さまでで、診断にもそう書く。

### 7.7 仮押さえの有効期限と、待ちの長さ（X5）

dandori が chobo の仮押さえを案件として追うと（7.8、`case … follows <帳簿>.<振替>`）、いまの dandori の検査（E020 など）が、chobo のステートマシンでそのまま効く。有効期限切れは外で起きるイベントで、確定の前に期限が切れうることも数える（dandori の DESIGN 2.2）。

ritsu では、さらに長さを比べる。仮押さえを作ってから確定するまでの長さに下限があり、それが有効期限を超えるなら、確定は必ず期限切れで断られる（エラー。確定が通る分岐は通らない）。上限があり、それが有効期限より短いなら、期限は切れない（期限切れを処理する分岐は通らない。警告）。

長さの下限と上限は、仮押さえと確定のあいだの文から求める。

- `wait <n> days` などの決まった長さの待ちは、そのまま下限にも上限にも入る。
- `wait until <時刻>` の時刻が koyomi の関数の `at` で、その関数に渡す日付が仮押さえを作った日なら、koyomi が入力の範囲のすべてで数えた日数の最小と最大から、下限と上限が出る。7.5 の例の支払日なら、受領日の終わりに押さえて 18 日後の 9 時に確定する場合が最短で、17 日と 9 時間である。有効期限が 14 日なら、必ず切れる。
- 待ち以外の文（タスク）は、タイムアウトとリトライの回数がわかれば上限に入り、下限には 0 として入る。タイムアウトの無いタスクがあれば、上限は無い。

「渡す日付が仮押さえを作った日である」ことを、いまの dandori は言えない。dandori には時刻を読む式が無く、日付はタスクの結果か入力としてしか入ってこないからである。そこで、その文を走らせた時刻を読む式 `now` を dandori に足す（E の段階。dandori の構文を足す。書き方の細部は dandori の DESIGN に書く）。Temporal には、再生しても同じ時刻を返す `workflow.now()` がある。ほかのプラットフォームでの読み方は、そのとき調べて、作れないプラットフォームは dandori の P6 のとおり E050 にする。作るまでのあいだは、決まった長さの待ちだけで下限と上限を求め、ほかは「決められない」として、いまと同じく期限切れが起きうるものとして数える。

### 7.8 koyomi の関数と chobo の振替を、dandori から呼ぶ（X6）

dandori に、期日と帳簿を読む宣言を足す（E の段階。dandori の構文を足す）。書き方の案は、chobo の DESIGN 5 章のもの（`use book 在庫 from "在庫.book"`、タスクの呼び方 `book 在庫.引当.hold`、`case 押さえ : 引当 follows 在庫.引当`）と、それに合わせた `use dates 支払条件 from "支払条件.cal"`（呼び出しは規則と同じく `let d = 支払条件.支払日(受領日: …)`）である。

- koyomi の関数の呼び出しは、規則と同じくアクティビティ（各プラットフォームのタスク）にする。koyomi の関数は純関数だが、祝日の表が毎年変わるので、ワークフローのコードの中で計算すると、表を入れ替えたワーカーで再生が食い違う。規則を普通のアクティビティにした理由（dandori の DESIGN 4.2。判定の記録が履歴に残る、直しても再生が食い違わない）と同じである。
- 渡す日付が、関数の入力の範囲とカレンダーのデータの範囲に収まるかを確かめる（X6）。収まらない日付があればエラーで、その日付とそこに至る実行を添える。dandori の範囲は、いまは数にだけ書けるので、日付の範囲を求めて運ぶ仕組みを dandori に足す。
- chobo の振替の呼び出しは、タスクの呼び方の一つにする。断られた理由は、タスクの宣言したエラーとしてそのまま使える（chobo の DESIGN 5 章）。chobo の操作はキーで冪等なので、dandori の E030（キーの無いリトライ）にあたらない。
- どちらも、dandori のすべてのプラットフォーム（Temporal の TypeScript・Python・Go、Step Functions、Lambda durable functions、Argo、pydantic-graph）で作る。koyomi は TypeScript・Python・Go のコードを、chobo はその三つの言語のクライアントを生成するので、作れないプラットフォームは無い見込みである。作れないものが見つかれば、dandori の P6 のとおり E050 にする。生成、参照インタプリタの見え方、E040 と E050、ランナーと突き合わせのテスト、README、DESIGN の全部に載せる（作者の決まり）。

### 7.9 一つの参照インタプリタ（X7）

`ritsu run <file.flow> --scenario <file.json>` は、dandori の参照インタプリタで、規則の呼び出しを rulec の評価器で、期日の呼び出しを koyomi のインタプリタで、振替を chobo のインタプリタ（帳簿の状態を持つ）で計算しながら流す。ほかのタスクの結果は、いまと同じくシナリオに書いたものを使う。

いまの `dandori scenarios` は、規則の結果も選ぶ（どの分岐も通るように）。それはそのまま残す（プラットフォームとの突き合わせは、規則の結果を選べる方が分岐を全部通せる）。`ritsu run` は、入力から規則の結果を計算する、もう一つの流し方である。規則の入力の選び方には、rulec の `vectors`（境界から作った入力）を使える。

これは証明ではない。三つの言語の意味を一度に流せる参照で、一つの生成パッケージ（9.3）との突き合わせの基準にする。

### 7.10 yuen、sakai、名指し（X8〜X10）

- **yuen**：段階 C で止めていた「一式の読み込み」（yuen の PLAN の C.1〜C.9）を、子プロセスと JSON ではなく、`Rules`・`Dates`・`Books`・`Claims`・`Items` の口で作る。端は 6.4 の定義の文になり、表や日付の関数や主張の一つ一つを追える。借りた出典は土台の出典（4.6）から読み、E107 も同じ手続きで比べる。`affected` は geas の記録を geas の口で読む。D の最後の部分で、そう作った（PLAN の D.7）。読むのは `Items`（中のものと定義の文）、`Sources`（規則とカレンダーの出典。D.7 で足した口）、`Rules` と `Dates`（別名だけ）、`Claims`（記録と、D.7 で足した `affected`）で、`Books` は読まない（chobo の中のものと定義の文は `Items` が渡す）。E203 は「名指したものの言語が、そのファイルについて答えられない」に意味を替え、E204 と、記録で主張の名前を確かめていた W201 を退かせた（yuen の DESIGN 6.2）。
- **sakai**：止めていた C.1〜C.5 を、`References` と `Rules` の列挙（`connect.enums` にあたるもの）で作る。dandori の参照も読めるので、N101 は要らなくなる。sakai の DESIGN 4.7 が挙げていた四つの検査（規則の同梱が境界を越える、`connect` で呼ぶサービスが上流の公開ホストサービスでない、`implements` するサービスが自分の公表された言語に無い、子の `.flow` が境界の向こうのもの）を足す。子の `.flow` の扱い（sakai の DESIGN 4.7 の最後の段落）は、そのとき sakai の DESIGN に決める。D の最後の部分で、そう作った（PLAN の D.8）。読むのは rulec の `Rules`（Connect のパスと列挙、入力と出力の名前）と `References`（`import proto`、`shape`、`apply`）、koyomi の `References`（`use calendar`）、dandori の `References`（`use rule`、`use proto`、`connect`、`flow`、`implements`）、chobo の `Books`（doc のための勘定と振替の名前）である。四つの検査は E202（規則の同梱と `apply`）、E207、E208、E209 になった。子の `.flow` は、パートナーシップ、共有カーネル、子が相手の公開ホストサービスを実装しているときだけ許す（sakai の DESIGN 4.7）。E104 は「地図が含む成果物の言語がつながっていない」、E105 は「成果物が、その言語の検査を通らないか、読めない」に意味を替え、N101 を退かせた（sakai の DESIGN 5.2）。
- **名指し**：プロジェクトのどこに書いた名指しも、索引のものを指すかを確かめる。いまは yuen と sakai がそれぞれ確かめている（yuen の E202 など）。言語ごとのコードと文はそのまま残し、引き方だけを索引に替える。E.1 でそうした（6.4）。ritsu の台帳には、この検査のコードを足していない。名指しを書いた言語が、自分のコードで言うからである。子プロセスと JSON のためのコード（yuen の E203「ツールがファイルを読めない」と E204「ツールの JSON が知らない形」、sakai の E104「ツールが無い」と E105「ツールの api が失敗した」）は、出す側の検査のエラーを名指すもの（6.1）に意味を替えるか、退かせる。退かせるコードは台帳に退いたと書いて残し、番号を使い回さない（rulec の docs/compatibility.md と同じ決まり）。

### 7.11 同じ条の写し（X11）

規則（rulec）、カレンダー（koyomi）、要件（yuen）が同じ法令の同じ条を引くとき、写しの本文が同じかを確かめる。写しの場所の決まりは三つとも同じ（4.6）なので、同じプロジェクトの中では写しを一つにでき、別々に写したときは本文を比べる（e-Gov は改正の無い条でも XML の属性を書き換えることがあるので、バイト列ではなく本文で比べる。rulec の §15.71、yuen の DESIGN 3.3）。

### 7.12 一つの `.proto` の読み方（X12）

検査を足すのではなく、`ritsu-proto` に読み手を一つにすること（4.10）で、同じ `.proto` を rulec、dandori、sakai、yuen が違って読む余地を無くす。C の段階で sakai を、D の段階で rulec と dandori を移し、移す前と後で、それぞれのテストの結果が同じことを確かめる。D.10 で rulec と dandori を移した（4.13）。rulec は 1,782 回、dandori は 701 回の出力が移す前と一字も違わず、rulec の契約の突き合わせ（コーパスと変異）と、dandori の `connect`・`implements`・`.proto` の型（`tests/protos.rs` と例）のテストも同じに通った。出力が変わったのは、`.proto` として読めないファイルのときだけである。rulec は途中まで読まずに E013 で止め（★）、dandori の E016 の注と sakai の E106 の日本語の文は言い方が変わった（4.13）。

### 7.13 処理系自身の依存（X13）

3.4 の地図 `ritsu.ctx` を ritsu のリポジトリに置き、`ritsu check .` で確かめる。言語のクレートが別の言語のクレートを `[dependencies]` に足せば、sakai がその行を名指して止める。CI の `fast` のジョブで走らせる。

## 8. 一つの CLI

### 8.1 ritsu のコマンド

```
ritsu check [<パス>...] [--root <dir>] [--format json] [--lang ja|en]
ritsu run <file.flow> --scenario <file.json> [--format json]                     （E）
ritsu gen [<パス>...] --target typescript|python|go [--out <dir>] [--check]        （E）
ritsu explain <コード> | --all [--format markdown|json]
ritsu lsp                                                                        （F）
ritsu <言語> <引数>...        rulec・dandori・koyomi・chobo・geas・yuen・sakai のコマンドそのもの
ritsu --help | --version
```

- `ritsu check` は、パスを渡さなければ今いるディレクトリを読む。各言語の `check` を全部のファイルに走らせ、言語をまたぐ検査をし、最後に一行の要約（言語ごとのファイルの数、確かめた境目の数、決められなかった数）を出す。
- `ritsu explain` は、ritsu の台帳（言語をまたぐ検査のコード）を引く。各言語のコードは `ritsu <言語> explain <コード>` で引く。言語ごとにコードの番号が重なる（rulec の E101 と koyomi の E101 は別のもの）からである。
- `ritsu <言語> …` は、その言語のコマンドと同じものを、すべての口をつないで走らせる。

**E.2 で作った形**（PLAN の E.2）。`ritsu check` は、プロジェクトを読み（6.1 の `Project::load`）、言語ごとの `check` を 6.1 の順に走らせる。rulec、koyomi、chobo、geas、dandori には、プロジェクトのファイルを一つずつ、使う人が書くとおりのパス（走らせたディレクトリから）で渡す。dandori には rulec の規則の口をつなぐ。yuen と sakai は自分でファイルを探して一つのプロジェクトや地図として確かめる言語なので、渡されたパスのうち自分のファイルを含むものと、プロジェクトのルートを `--root` で渡す。`.proto` には自分の言語の `check` が無い（言語をまたぐ検査が読む。7 章）。

- 各言語は、自分のコマンドが印字に使う関数で、単位（ファイル、yuen のプロジェクト、sakai の地図）ごとに、何をどう印字するかを返す（`ritsu_ports::Checked`。診断一つずつのテキストと JSON、そのほかの行、単位の結果）。`ritsu check` は言語の出したテキストを読み直さない。言語ごとの関数は `rulec::ports::Engine::checked`、`koyomi::ports::Engine::checked`、`chobo::ports::Engine::checked`、`geas::cli::checked`、`dandori::ports::Engine::checked`、`yuen::ports::Engine::checked`、`sakai::run::checked` である。
- geas の `check` は主張を走らせる（プログラムを動かし、ジャーナルを書く）。`ritsu check` でも同じで、`geas check` が走らせるものを走らせる。
- 言語は `--lang` で選び、無ければ `RITSU_LANG`、どちらも無ければ英語である。各言語の `<名前>_LANG` は読まない（一つのコマンドの文面を一つの言語にする）。
- `ritsu explain <コード> | --all [--format markdown|json]` は E.3 で作った（7.1）。
- 言語のコマンドは、どれもライブラリの関数になった。rulec、koyomi、chobo、geas は、E.2 で `src/main.rs` の中身を `rulec::cli::run`、`koyomi::run::run`、`chobo::run::run`、`geas::cli::run` に移した（振る舞いは変えていない。各言語の DESIGN.md）。これで `ritsu <言語>` は七つの全部にある（8.2）。

### 8.2 各言語のコマンドの残し方

- 名前は残す。`rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`yuen`、`sakai` は、リリースでは `ritsu` を指すリンクで、呼ばれた名前の言語として動く（2.3）。`ritsu rulec check …` と `rulec check …` は同じである。
- コマンド、フラグ、終了コード、診断、`--format json` と `api` の形は変えない（P6）。変わるのは、ほかの言語を同じプロセスの中で読むようになることだけである。dandori は rulec を子プロセスで走らせなくなり、`DANDORI_RULEC` は要らなくなる（D の段階で消し、README と dandori の DESIGN を直す）。yuen と sakai が子プロセスで呼ぶために予定していた `YUEN_RULEC`・`SAKAI_RULEC` などの環境変数は作らない。
- `--version` は `<名前> <ritsu のバージョン>` を一行で出す（`rulec 0.23.0` など。13.1）。`rulec --version` を読むスクリプトは、そのまま動く。
- 言語の環境変数（`RULEC_LANG` など）は残し、全部に効く `RITSU_LANG` を足す。

E.2 で、`ritsu <言語>` を七つの全部に作った。rulec、koyomi、chobo、geas のコマンドはそれぞれのクレートのバイナリと同じ関数を、dandori、yuen、sakai のコマンドは ritsu-project が一度つないだ口（6.1）を渡して呼ぶ。`ritsu` を言語の名前で呼ぶと（`rulec` という名前のリンク）、その言語のコマンドとして動く（2.3）。`crates/ritsu/tests/entry.rs` が、七つの `--version`、いくつかのコマンド、`koyomi` と `rulec` という名前のリンクで確かめる。

### 8.3 出力

`ritsu check` のテキストは、ファイルごとに、その言語の `check` が出すとおりの診断を出し、そのあとに言語をまたぐ検査の診断を出す。言語ごとに番号が重なるので、`ritsu check` の中でだけ、見出しの括弧にツールの語を足す（`error[rulec E101]`、`error[ritsu E201]`）。rulec の診断の枠（`-->` で場所を示す形）は、そのまま使う。

**E.2 で決めた形**。テキストは、言語の `check` が単位ごとに印字するもの（診断、`ok rules/送料.rule` の行、要約、chobo の報告など）を、そのまま 6.1 の順に並べる。足すのは、診断の見出しのツールの語（見出しの最初の `[<コード>]` を `[<ツール> <コード>]` にする）と、最後の一行の要約だけである。sakai の例を写したプロジェクトで、受注が注文の状態に値を足したとき（`order.proto` に `ORDER_STATUS_RETURNED = 5;`）は次のようになる（`crates/ritsu/tests/golden/check/shop-returned.en.txt`。途中の、通るファイルの行は省いた）。

```
$ ritsu check .
ok billing/rules/出荷の送料.rule
ok billing/rules/決済手数料.rule
error[rulec E032]: Enum 注文の状態 does not agree with OrderStatus in ../../proto/shop/ordering/v1/order.proto
  --> billing/rules/請求の要否.rule:5
  |
5 | enum 注文の状態(order_status) = 受付(received) | 支払済(paid) | 出荷済(shipped) | 受注で取消(cancelled)
  |      ^^^^^^^^^^
  |
 In ../../proto/shop/ordering/v1/order.proto but not in this enum: returned
 The form to add: `<name>(returned)`. The name is yours to decide — the file carries no Japanese.
 A value appeared through the contract, not through this rule. What the new value costs is a decision nobody has made yet (§15.59).

ok delivery/rules/出荷の急ぎ.rule
…
ordering/受注.flow: ok
error[sakai E105]: billing/rules/請求の要否.rule:5:1: The file billing/rules/請求の要否.rule does not pass rulec's check, or cannot be read
     5 | enum 注文の状態(order_status) = 受付(received) | 支払済(paid) | 出荷済(shipped) | 受注で取消(cancelled)
  = What rulec says: [E032] billing/rules/請求の要否.rule:5: Enum 注文の状態 does not agree with OrderStatus in ../../proto/shop/ordering/v1/order.proto
  = Make the file pass rulec's check; the references of a file that cannot be read cannot be checked.
ritsu check: 21 files (rulec 4, koyomi 3, chobo 1, proto 5, dandori 2, sakai 6): 1 fail (2 errors); borders between the languages: 0 checked, 0 undecided
```

要約は、言語ごとのファイルの数、結果、言語の境目の検査の数を言う。結果は、どれも通れば `all pass`（日本語は「どれも検査を通った」）、通らないファイルがあればその数、確かめられなかったファイルがあればその数で、エラーと警告の数を括弧に添える。ファイルが通らないとは、そのファイルを場所とするエラーがあるか、ファイルを一つずつ確かめる言語（rulec、koyomi、chobo、geas、dandori）がそのファイルを通さなかった（geas の成り立たない主張など、コードの無いものも含む）ことである。境目の数は、言語をまたぐ検査（7 章）が確かめた境目と、そのうち決められなかったものである。検査は E の二つ目の部分（PLAN の E.4）で入るので、いまはどちらも 0 である。日本語の要約は「ritsu check: ファイル 21 個（rulec 4、…）。検査を通らないもの 1 個（エラー 2 件）。言語の境目: 確かめた 0 か所、決められない 0 か所」の形になる。

JSON は一つのオブジェクトで、キーは `ritsu`（バージョン）、`root`（走らせたディレクトリから見たルート）、`ok`（exit 0 になるか）、`files`（ファイルごとの `tool`、ルートからの `file`、`ok`）、`diagnostics`、`borders`（境目の検査の `held`・`failed`・`undecided`）の順である。各言語の診断は、その言語の `check --format json` が書く形のまま入れ、その先頭に `tool` を置き、`file` をルートからの相対にする（言語の形に `file` が無ければ `tool` のすぐあとに足す）。rulec の診断は rulec の形（`v` が 2 の形）のままで、`title` や `column` は rulec の名前である。上の例では次のようになる（`crates/ritsu/tests/golden/check/shop-returned.json`。`…` は省いたところ）。

```json
{
  "ritsu": "0.1.0",
  "root": ".",
  "ok": false,
  "files": [
    {
      "tool": "rulec",
      "file": "billing/rules/出荷の送料.rule",
      "ok": true
    },
    …
  ],
  "diagnostics": [
    {
      "tool": "rulec",
      "v": 2,
      "severity": "error",
      "code": "E032",
      "file": "billing/rules/請求の要否.rule",
      "line": 5,
      "column": 6,
      "title": "Enum 注文の状態 does not agree with OrderStatus in ../../proto/shop/ordering/v1/order.proto",
      …
    },
    {
      "tool": "sakai",
      "code": "E105",
      "severity": "error",
      "file": "billing/rules/請求の要否.rule",
      "line": 5,
      "col": 1,
      "message": "The file billing/rules/請求の要否.rule does not pass rulec's check, or cannot be read",
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

形の案で `crossings` と呼んでいたものは `borders` にした。sakai の要約と api の `crossings` は、境界づけられたコンテキストの境界を越える参照のことで、別のものだからである。案の `proved` も、0.4 の決まり（肯定形で「証明」と言わない）に合わせて `held` にした。

テストは `crates/ritsu/tests/check.rs` で、テストのプロジェクトとそれを変えた写しのテキスト（英語と日本語）と JSON を golden にし、テキストからツールの語を除いたものが、同じファイルに各言語のコマンド（`ritsu <言語> check`）が出すものを順に並べたものと一字も違わないことを、テストのプロジェクト、変えた写し、yuen のテストの材料（規則を名指す要件）、geas の例（主張が Python のプログラムを走らせる）で確かめる。

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

同じ部分で、`ritsu sakai <引数>…` も足した（PLAN の D.8）。sakai のコマンドを、sakai が読むすべての言語の口（rulec の `Rules` と `References`、koyomi と dandori の `References`、chobo の `Books`）をつないで走らせる。テストは `crates/ritsu/tests/sakai.rs`（例の `check` と `api` と四つの `build --check`、ほかの言語を通してしか見えない変更の E209 と E105）。これで入口が持つ言語は、ほかの言語を読む dandori、yuen、sakai の三つになった。残りの四つ（rulec、koyomi、chobo、geas）はほかの言語を読まないので、自分のクレートのバイナリで同じに動く。

E.2 で、入口を 8.1 の形にした（`ritsu check`、七つの全部の `ritsu <言語>`、リンクの名前）。下の段落と捨てたものは、D でこの最小の形を先に作った理由として残す。

待つ費用が大きく、作る費用が小さいからである。待てば、規則を使う例をコマンドで走らせる手段が E まで無く、README に書ける手順も無い。作るのは、dandori のコマンドを関数（`dandori::cli::run`）にしたので、引数を渡すだけで済む（`src/main.rs` は 90 行）。入口は何に依存してもよく（3.1）、`cargo xtask deps` も通る。テストは `crates/ritsu/tests/dandori.rs` に置いた（すべてをつないだバイナリを走らせるテストの置き場所。3.3）。規則を使うフローを `ritsu dandori check` と `doc` が読むこと（英語と日本語）と、`ritsu` が持たないコマンドに 2 で終わることを見る。

捨てたもの：

- **E まで待つこと**：上の理由。
- **dandori のクレートのバイナリに rulec を入れること**：依存の決まりの 1（言語のクレートはほかの言語のクレートに依存しない）を破る。
- **テストのための小さなバイナリを別に作ること**：README の手順は、使う人が走らせられるものでなければならない。

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

### 9.3 一つの生成パッケージ（E）

`ritsu gen --target <言語>` は、プロジェクトの規則、期日、帳簿のクライアント、ワークフローを、その言語の一つのパッケージにする。対象は、四つの言語が共に生成している TypeScript、Python、Go。

```
generated/typescript/
  rules/<別名>.ts      rulec gen が書くもの
  dates/<別名>.ts      koyomi gen が書くもの
  books/<別名>.ts      chobo build のクライアント（PostgreSQL か TigerBeetle）
  flows/<名前>/        dandori build（Temporal）が書くもの
  index.ts             全部を一つの名前で出す
```

ワークフローは、規則と期日と帳簿を、フローごとに写したコードではなく、同じパッケージの `rules/`・`dates/`・`books/` から読む。どのファイルの頭にも同じバージョンと元のファイルを書く。パッケージの依存は、入れたものが要るものだけ（rulec と koyomi の生成物は依存を持たない。chobo のクライアントはデータベースのクライアントを、dandori の生成物は Temporal の SDK を要る）。

確かめ方：パッケージがその言語の型の検査（`tsc --strict`、`mypy --strict`、`go vet`）を通ること。中身ごとの突き合わせは、いままでどおり各言語のテスト（rulec の 12 言語のベクタ、koyomi の五つの出力先、chobo の七つの組み合わせ、dandori のプラットフォーム）がする。`ritsu gen --check` は、いまの生成物が古いかを言う（rulec の `gen --check` と同じ）。

### 9.4 生成物のバージョン

生成物の頭のバージョンは ritsu のバージョンになる。バージョンが上がれば生成し直してコミットすることになる（rulec の docs/compatibility.md が言うとおり）。

### 9.5 段階 C で作った形（`ritsu-emit`）

C.10 で `ritsu-emit` を作り、koyomi と chobo をこれに替えた。生成物は一バイトも変わっていない（PLAN の C.10）。

- 予約語（`words`）は、標準が並べるものを標準ごとに一つの表にした（ECMAScript 2025 の予約語と strict mode の予約語、Python 3.14.6 の `keyword.kwlist` と `softkwlist`、Go 1.25 のキーワードと事前宣言の識別子、Rust 1.94 のキーワード、PostgreSQL 18.0 の `kwlist.h` と PL/pgSQL の予約語）。生成器が照らし合わせるのは、いくつかの表をまとめた `Words` である。名前を断る（koyomi の E009）か、`_` を後ろに付けて避ける（chobo、dandori）かは、生成器が決める。
- rulec と dandori の表は `copies` に写した。標準の表と同じところはそれを指し、違うところ（rulec は `Self` を持たず、Go の `complex64` と `complex128` を持たない。dandori は生成物が使う名前を足す）は、それぞれの表に持つ。C.11 で、二つは自分の表をやめて `copies` を読むようにした。rulec は出力先ごとの定数（`copies::rulec::PYTHON` など）と、`backend.rs` の並びの `BACKENDS` を、dandori は Python・Go・TypeScript の表を読む。中身は写したときのままで、生成物も診断も変わらない。標準の表にそろえるかは、一つの生成パッケージ（9.3）を作る E で決める。表の違いが、dandori の生成物が rulec の生成物の名前を参照するところで食い違いを起こすかを C.11 で調べた結果は、PLAN の 7.5 にある。
- 名前（`ident`）、リテラル（`lit`）、生成物の頭の一行とコメント（`header`）は、koyomi と chobo の形である。9.2 の頭（`Code generated by <名前> <ritsu のバージョン>.` と元のファイルとハッシュ）にそろえるのは、生成物が変わるので E の段階（9.3、9.4）にする。

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

### 10.4 変えたところだけを回す

`cargo xtask test --changed <基準のリビジョン> [--level <段>]` は、変わったファイルからクレートを選び、そのクレートに依存するクレートも足して（`cargo metadata` の依存から）、そのテストだけを回す。土台の層が変われば全部になる。文書だけが変わったら、そのクレートの文書のテストを回す。作業の途中は変えたところだけを回し、全体は区切りで一度回す（dandori でも、作業中は変えたフローだけを回し、全体はコミットの前に一度回している）。

### 10.5 CI

ritsu のリモートを作るまで（作者が決める）、CI は走らない。ワークフローは C の段階で書いておく。

| ジョブ | いつ | すること |
|---|---|---|
| `fast` | push と pull request のたび | `cargo build --workspace --locked`、`RITSU_TEST_LEVEL=fast cargo test --workspace --locked`、依存の決まりの確かめ（3.4） |
| `tools` | main への push、コードを変えた pull request、毎晩 | ツールを入れ（rulec の `ci.yml` の一覧に、koyomi、chobo、geas、yuen、sakai、dandori のものを足す。PostgreSQL は、rulec にはサービスで、ほかには使い捨てのクラスタで）、クレートの組ごとに並べて `RITSU_TEST_LEVEL=tools` で回す。許す SKIP は、CI で用意できない pixie の greeter の四つだけ（下） |
| `proofs` | `proofs/` か、証明書とモデルにかかわるコードを変えたとき | `lake build`、コーパスの証明書の再検査、Lean のモデルとの突き合わせ（11 章） |
| `kani` | 毎晩と、rulec の生成器を変えたとき | rulec の CI の Kani の段（生成した Rust のハーネス） |
| `platforms` | 毎晩、手で始めたとき、`crates/dandori/` を変えた pull request | kind の上の Argo、LocalStack、Temporal の dev server を立てて、dandori の `platforms` の段を回す。外のサーバー（e-Gov、eCFR、Buf Schema Registry）に問い合わせるテストもここで回す（下）。ほかのジョブと並べない |
| `release` | タグ（F） | 13.2 |

C.12 で、`release` のほかの五つを根の `.github/workflows/` に書いた（ジョブ一つにファイル一つ。`fast.yml`、`tools.yml`、`proofs.yml`、`kani.yml`、`platforms.yml`）。許す SKIP の一覧は `ci/skips/fast.txt`、`tools.txt`、`platforms.txt` にある。リモートが無いので、どれもまだ走らせていない。手元で確かめたのは、YAML として読めること、actionlint（v1.7.12）が何も言わないこと、`run` の中身が `bash -n` を通ること、ジョブが呼ぶコマンドがこの機械で通ることである（PLAN の C.12）。書いたときに決めたことは次のとおり。

- `fast`：新しく取り出した木で走らせることを考え、`crates/rulec/website/sync.sh` と `crates/dandori/website/sync.sh` で、サイトが共有するページの写し（gitignore してある）を先に作る。テストは `cargo xtask test --level fast` で回し、`ci/skips/fast.txt` は空である。
- `tools`：クレートを三つの組（rulec、dandori、それ以外の五つの言語と `ritsu-base`・`ritsu-testkit`・`ritsu-proto`・`ritsu-emit`・xtask）に分け、matrix で並べて走らせる。組ごとに要るものだけを入れる。PostgreSQL は、rulec の組がサービスのサーバーを libpq の環境変数で使い、ほかの組は PGDG の PostgreSQL 18 のプログラムで使い捨てのクラスタを立てる（`RITSU_PG_BIN`）。dandori の組は、rulec 0.22.0 のリリースのバイナリをチェックサムで確かめて `DANDORI_RULEC` に渡し（D.3 まで）、protoc 35.1 のリリースの zip を、書いたときに取ったチェックサムで確かめて入れる。rulec の `ci.yml` が `cargo test` のあとに走らせていたもの（`rulec test --require-all` で飛ばした側が無いこと、証明書の再検査、`fmt --check` と `check`）は、rulec の組の最後に残した。`--proofs` の付いた回は `kani` に移した。`ci/skips/tools.txt` は、PLAN の C.12 が空としていたのと違い、geas の pixie の四つを許す。pixie は ritsu の外でビルドするもので、pixie のテストは CI では回さず、greeter のある手元の機械で回すと決めた。
- `proofs`：rulec の `ci.yml` の `proofs` のジョブを、パスを `crates/rulec/` の下に直して写した。走るのは、`proofs/`、rulec の src、コーパス、`tests/lean.rs`、`tools/recheck.py`、土台の src（証明書のダイジェストと JSON）、`Cargo.lock` のどれかが変わったときである。
- `kani`：rulec の `ci.yml` の Kani の段（コーパスの全部の規則を Rust にして Kani で証明する）と、`rulec test --proofs` の回（`フラグを付ければ証明が走る` を platforms の段で）。毎晩と、rulec の生成器、`ritsu-emit` の src、コーパスが変わったとき。
- `platforms`：kind の上の Argo（kind 0.33 は Go の `go install` で、argo CLI v4.1.4 はチェックサムで確かめて入れ、`crates/dandori/tools/argo/setup.sh` でクラスタを作る）、LocalStack 4.14.0 のイメージ、Temporal の dev server（TypeScript の SDK の `@temporalio/testing` が取ってくる）を用意し、dandori の platforms の段のテストを一つずつ回す（10.6 のとおり、落ちたら一度だけ回し直し、そのことを出力に書く）。最後に kind の上にワークフローが残っていないことを確かめる。そのあと、外のサーバーに問い合わせるテスト（土台、koyomi、yuen の本物の e-Gov と eCFR、rulec の Buf Schema Registry）を platforms の段で回す。10.5 の表に無かったこの四つは、ほかにどのジョブも回さないので、ここに置いた。TypeSafe には CI から送らない。呼ぶたびにお金がかかり、CI では呼ぶ回数を見込めないので、鍵をリポジトリの secret にも置かない。ワークフローは `TYPESAFE_API_KEY` を空にして走らせるので、secret があっても読まず、Jev のテストは SKIP になる。それと Ollama の無い runner での SKIP を、`ci/skips/platforms.txt` で許す。

クレートの中に残っている `.github/workflows/` は、GitHub が走らせない（2.1）。それぞれ次のように扱う。

- rulec の `ci.yml`：中身は根の `tools`・`proofs`・`kani` に移した。`packages` のジョブ（`cargo package` と、`.deb` と `.rpm` を入れて消すこと）だけは、配るものの確かめなので、F.7 の `release` と一緒に根へ移す。それまでは、元のリポジトリの CI の記録として消さずに残す。
- rulec の `docs.yml` と `release.yml`、dandori の `docs.yml`：元のリポジトリから出しているサイトとリリースのワークフローである。サイトとリリースは F の最後に ritsu へ移す（12.3、13.2）ので、そのときに根のワークフローに書き直し、クレートの中のものを消す。
- rulec の `experiments/library/.github/workflows/`：規則のライブラリのリポジトリが使う CI の見本で、実験の中身である。ritsu の CI ではないので、そのまま残す。

### 10.6 揺れるテスト

dandori の重いテストには、原因を突き止めていない揺れがある（dandori の DESIGN 7 章の終わり）。`platforms` の段は、ほかの段と同時に走らせない。落ちたテストは一度だけ回し直してよいが、回し直したことを出力と報告に書く。二度続けて落ちたら、揺れではなく失敗として読む。

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
- C.11 で rulec と dandori のテストもこれに替えた。足したのは、段を聞いてから機械を聞く `ready(要るもの, 見つかるか, 理由)`（rulec と dandori のテストは、ツールが無いかだけを聞いていた）と、`Need::Rulec`（dandori のテストが規則を読む rulec 0.22.0。D.3 で消した）である。C.12 で、`Need::Suite`（sakai のテストが例の写しを確かめる rulec・koyomi・chobo・dandori のバイナリ。D.8 まで）と `Need::Pixie`（geas のテストが動かす pixie の greeter）を足した（下）。三つとも tools の段である。`Need::Suite` は D.8 で、sakai のテストが一式の言語を同じプロセスでつなぐようになって消した。rulec のテストは、PostgreSQL を `PG*` の環境変数で受け取る形のまま、SKIP と段を替えた（PLAN の C.11）。一時ディレクトリは、C のあとの片づけで `TempDir` に替えた（PLAN の 7.5）。
- C.12 で、まっさらに取り出した木を、外のツールを呼ぶと記録を残して失敗するコマンドを PATH の先頭に置いて `cargo xtask test --level fast` で回し、段を聞かずにツールを呼ぶテストを探した。geas の五か所（例を Python・Node・Go・rustc で走らせるもの、Go のサービスを SIGTERM で止めるもの、`explain` の再現、W061、pixie の greeter）と sakai の一か所（`what_was_copied_passes_the_suite`）が見つかり、段を聞くようにした。いまは、fast の段で呼ぶのは cargo と git と、テストが立てたサーバーへの curl だけである。
- C のあとの片づけで、テストが子に渡す TMPDIR を一時ディレクトリの下に作る `tmp::tmpdir_in` を足した。rulec のテストは、`rulec test` を走らせるときと swiftc を呼ぶときにこれを渡す（swiftc は `--version` に答えると、ほとんど毎回、空の `TemporaryDirectory.*` を TMPDIR に残す）。Chrome には、一時ディレクトリをプロファイルの下に向けて渡す。止められた Chrome がシングルトンのソケットのディレクトリ（`com.google.Chrome.*`）を OS の一時ディレクトリに残さないためで、macOS の Chrome はその場所を `MAC_CHROMIUM_TMPDIR` から、ほかの Chrome は `TMPDIR` から読む。
- 根から `cargo test --workspace` を回すときの `--skip` は、ワークスペースのすべてのテストの名前に効く。dandori の重い段を外す `--skip argo` は `cargo` を含む名前にも当たるので、そういう名前のテストを作らない（xtask のテストの名前を一度直した）。

## 11. Lean の層

### 11.1 いまあるもの

rulec の `proofs/`（5,476 行。Lean v4.34.0、mathlib なし、まっさらから `lake build` で 5 秒ほど）は、rulec の証明書が通らなければならない検査を関数として書き、検査が通れば主張（完全性、重なり、当てはまらない行、単位、int64）が成り立つことを証明している。ステートマシン（`Machine.lean`）、一次の不等式を打ち消す乗数（`Linear.lean`）、契約と入口（`Contract.lean`）も含む。`rulec-recheck` は証明書を読んで、その関数そのものを走らせる。`tests/lean.rs` は、`sorry`・`axiom`・`native_decide` がどこにも無いことと、コーパスの証明書が通ることを確かめる。

### 11.2 足すもの

AWS の Cedar と同じ形にする。意味の中心部分を Lean でモデルにし、性質を証明し、Rust の実装とモデルを、テストが作る入力で突き合わせる。足す順は、小さくて効くものからにする。

1. **chobo の振替**：状態（勘定ごとの確定した残高と仮押さえ）、移動を書いた順に行い一つごとに境界を確かめること、全部か無しか、キーの冪等、仮押さえの終わり方（chobo の DESIGN 2 章）。定理は、done と done_before の操作が、どの勘定の境界も禁じた向きに越えさせないこと。
2. **koyomi の日付の計算**：通算日、月の足し算と無い日の扱い、締め、休みの寄せ方、営業日の数え方。koyomi の検査は総当たりなので、検査の健全さは数え上げで明らかだが、「単調性は隣り合う二日を比べれば足りる」（koyomi の `is monotonic`）は定理にできる。
3. **言語をまたぐ検査の足場**：入力を集合に限った完全性（RulecCert の `completeHolds` に、入力の集合を引数として足す。7.5 の (b)）、入力ごとの範囲の上で示した前提が、範囲のどの値でも成り立つこと（7.4）、待ちの下限が有効期限を超えれば確定は期限切れで断られること（7.7）。
4. **dandori の芯**：列挙の `match`、回数に上限のある `for` と `repeat`、タスクの呼び出しとその結果、ステートマシンに従う案件。定理は、案件の状態の検査（E020 など）が通れば、どの実行も、終わりの状態でない案件を残して終わらないこと。

### 11.3 Rust と Lean の突き合わせ

Lean のモデルを実行できるプログラム（`ritsu-model`）にし、JSON の行で入力を読み、結果を JSON の行で出す。Rust のテストが入力を作り（koyomi の `vectors` の入力、chobo の `scenarios`、dandori の `scenarios` のうち芯に入るもの）、Rust の参照インタプリタとモデルの両方に流して、一行ずつ比べる。合わなければ、最初の五行の入力と二つの結果を出して落ちる。`tools` の段で走らせる（`lake` が要る）。

### 11.4 置き場所

F の段階で、`crates/rulec/proofs/` を根の `proofs/` に移し、一つの Lake のパッケージにする。ライブラリは `RulecCert`（いまのまま）、`ChoboModel`、`KoyomiModel`、`RitsuCross`、`DandoriCore`。ツールチェーンは一つ（`lean-toolchain`）で、mathlib は入れない（ビルドを速く保つ）。rulec の `tests/lean.rs` は新しい場所を読むように直し、`sorry`・`axiom`・`native_decide` の確かめを全部のライブラリに広げる。

## 12. 元のリポジトリと、履歴の取り込み

### 12.1 元のリポジトリには書かない

`~/rulec`、`~/dandori`、`~/koyomi`、`~/chobo`、`~/geas`、`~/yurai`（yuen の前の名前のまま）、`~/sakai` には、書かない、そこでビルドしない、git の状態を変えない。取り込みは、それぞれを作業場所に `git clone --no-local` で写してから、その写しの上で行う。元のリポジトリは、作者が公開の扱いを決めるまで、そのまま残す。

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

取り込みのマージは、新しいコミットを作る。作者の決まり（勤務時間の外にコミットする、日時を偽らない）があるので、`~/ritsu` に対するマージは、指示する側が許した時刻にだけ行う。許しが出るまでは、作業場所の写しの上で同じ手順を走らせ、ビルドとテストまで済ませておく（PLAN B.2）。

### 12.3 取り込んだあと

取り込んだあとの開発は ritsu で行う。リリースとサイトは ritsu から出す（13.2）。rulec と dandori のサイト（GitHub Pages）、Homebrew の formula、リリースは、F の最後に ritsu へ移すまで、元のリポジトリのものが残る。元のリポジトリそのものの扱い（GitHub で公開している rulec と dandori を、アーカイブにするか、ritsu を指す一文を足すか）は作者が決める。

### 12.4 B で気をつけること

取り込んだだけで、いまのテストの振る舞いが変わるところがある。どれも、中身を直さずに済ませる方法か、直し方を PLAN B に書いた。

- **git のルートが上に移る**：geas、yuen、sakai は、いちばん近い `.git` をルートにする。取り込むと、`crates/<名前>/` には `.git` が無く、ルートは `~/ritsu` になる。12.5 で試した結果、yuen のテストが二つ落ちる。
- **rulec の `.cargo/config.toml`**：`[env]` で `RULEC_LANG=ja` を強いている（rulec のテストは日本語の文を確かめる）。cargo は、走らせたディレクトリとその上の `.cargo/config.toml` だけを読むので、`~/ritsu` の根から `cargo test` すると読まれない。B では、各クレートのテストを `crates/<名前>/` で走らせた。C.0 で、rulec のテストが自分で子プロセスに `RULEC_LANG=ja` を渡し、プロセスの中では日本語を選ぶ形にして、この設定を消した。いまは根からも、どのディレクトリからも同じに回る。
- **dandori が使う rulec のバージョン**：dandori の golden は rulec 0.22.0 で取ってある（57 のファイル）。ワークスペースで作る rulec は 0.22.1 なので、B では 0.22.0 の rulec（rulec のタグ `rulec/v0.22.0` から作業場所で作るか、リリースのバイナリ）を `DANDORI_RULEC` で渡す。
- **gitignore したものは来ない**：各クレートの `tools/` の `node_modules` と venv、Java の jar、TigerBeetle、`go-arch-lint`、ReqIF のスキーマ、rulec の `website/` が `sync.sh` で写すページ、dandori の `website/docs-ja/` の写し、rulec の `proofs/.lake` は、取り込んでも来ない。それぞれのクレートの手順で入れ直す（PLAN B.5）。
- **プロファイル**：ワークスペースの中のクレートの `[profile.*]` は読まれない。koyomi の `[profile.test] opt-level = 2`（1900〜2100 年のすべての日を何度も回すテストのため）は、根の `[profile.test.package.koyomi]` に移す。五つの `[profile.release] strip = true` は根の `[profile.release]` に一つ置く。クレートの中の `[profile.*]` は C.0 で消した。
- **`Cargo.lock`**：根に一つ作る。五つの `Cargo.lock` の依存は同じバージョン（serde_json 1.0.151 ほか 15 のパッケージ）なので、通信せずに作れる。クレートの中の `Cargo.lock` は使われなくなる（C.0 で消した）。

### 12.5 取り込んだ形で走らせてみたこと

2026-10-03 に、作業場所に `sim/.git`（空のディレクトリ）と `sim/crates/<名前>/`（元のリポジトリを写して `.git` を消したもの）を作り、B の形（ルートがクレートの二つ上にある形）で yuen、sakai、geas のテストを走らせた。外のツールは渡していない。yuen はこのとき yurai という名前で、下に貼った出力のパスは、そのときのまま（`crates/yurai/`）にしてある。

- **yuen**：92 件が通り、2 件が落ちた。SKIP の行は 3（prov と reqif の venv、ReqIF のスキーマが無いため）。コンパイルを含めて 7 秒ほど。
  - `tests/cli.rs` の `the_json_of_check`：`yuen check tests/fixtures/period --format json` の `root` が `"."` でなく `"../.."` になった。
  - `tests/design.rs` の `every_command_in_design_prints_what_design_shows`：DESIGN.md の `$ yuen check tests/mutants/E302_条が変わった`（`--root` なし）の出力の、写しのパスが変わった。

    ```
    --- DESIGN.md shows
      what changed in the text (the copy tests/mutants/E302_条が変わった/sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml):
    --- it prints
      what changed in the text (the copy ../../crates/yurai/tests/mutants/E302_条が変わった/sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml):
    ```

    パスはルートからの相対を走らせたディレクトリからの相対に直したもので、間違いではないが、回り道をしている（`../../crates/yurai/` は要らない）。git のリポジトリの下のディレクトリで yuen を走らせれば、取り込まなくても起きる。C.0 で yuen の表示を直した（4.7）。
  - 二つとも、`--root .` を渡せばルートはクレートのディレクトリになり、出力は元のものと同じになった。ただし E302 の直し方の行は、渡したとおり `--root .` を含めて書く（`yuen review tests/mutants/E302_条が変わった --root . --at …`）ので、DESIGN.md のその三行も合わせて直すことになる。
- **sakai**：97 件が全部通った。SKIP の行は 9（ツールが無いため）。コンパイルを含めて 12 秒ほど。ただし `tests/cli.rs` の `check_exit_codes_and_formats` は、テストのディレクトリに `.git` があるときだけ `--root` なしの形を確かめる作りで、B の形ではその部分を黙って飛ばす。
- **geas**：236 件が全部通った。SKIP の行は pixie の二種類が二回ずつ（`GEAS_PIXIE_GREETER` を渡していないため）。geas のテストは、例を一時ディレクトリに写して走らせるので、ルートが移っても変わらなかった。コンパイルを含めて 1 分 32 秒。
- rulec、dandori、koyomi、chobo は、`.git` からルートを決めない（`src/` を探して確かめた）。rulec の `@リビジョン` の読み方は、走らせたディレクトリからの相対（`git show <rev>:./<パス>`）なので、`crates/rulec/` の下でもそのまま読める。

## 13. バージョンとリリース

### 13.1 一つのバージョン

ワークスペースのバージョンを一つにし（`[workspace.package] version`）、どのクレートも `version.workspace = true` にする。どのコマンドの `--version` も、生成物の頭も、doc のページも、このバージョンを書く。バージョンは生成物の頭や golden に入っているので、変えるのは F の段階で一度にする（それまでは、各クレートのいまのバージョンのまま）。

バージョンの番号は rulec の続きにし、ritsu の最初のリリースを 0.23.0 にする。rulec だけがリリースを重ね（タグ 28）、Homebrew の formula を持ち、生成物の頭と、README が案内する CI の書き方（`uses: i2y/rulec@v0.22.1`）にバージョンが入っているからである。ritsu を 0.1.0 から始めると、`rulec --version` が 0.22.1 から 0.1.0 に戻る。言語ごとにバージョンを持ち続ける形は、バージョンを一つにすること（このまとめの目的の一つ）に反し、1.4 のバージョンの食い違いが残る。

### 13.2 配り方

- GitHub のリリース：rulec の `release.yml` を広げ、macOS（arm64、x64）と Linux（x64、arm64。musl で静的にリンク）の四つを作る。アーカイブには `ritsu` と、言語の名前のリンク七つを入れ、`SHA256SUMS` を添える。
- 配るのは ritsu だけにする。言語ごとのリリース、formula、サイトは作らない。いまの rulec と dandori のサイト（GitHub Pages）とブラウザで試すページ、rulec の Homebrew の formula も、F の最後に ritsu へ移す。
- Homebrew：`i2y/tap/ritsu` を足す。いまの `i2y/tap/rulec` を入れている人の移り方（formula を ritsu のアーカイブを入れるものに替えるか、消して README で案内するか）は、F の最後に移すときに決めて、この節に書く。
- サイト：rulec と dandori のサイトの中身（英語と日本語のページ、ブラウザで試すページ）は、ritsu のリポジトリから出すサイトに移す。
- `.deb` と `.rpm`：rulec の `packaging/` を広げる。
- GitHub Actions：rulec の `action.yml` を広げ、`uses: i2y/ritsu@v0.23.0` で入れる。
- crates.io：いまは出さない。`koyomi` の名前は別のクレートが使っていて（2.2）、`ritsu` を crates.io に出すには、それが依存する中のクレートを全部出すことになる。出すなら、中のクレートを `ritsu-` で始まる名前にする。

リリースと push は、作者の指示があるときだけ行う。

## 14. 捨てた形

- **一つの言語にまとめる**：検査は、それぞれの狭さの上に立っている（rulec の自分の列への単項テスト、koyomi の一つの日付と有限の範囲、chobo の勘定の上限と下限、dandori の比較も計算もしない式、yuen のつながりとハッシュと期間、sakai の確かめられる部分）。混ぜれば崩れ、節で分ければファイルが一つになるだけである。
- **一つのファイルに言語ごとの節を並べる**：読む人と承認する人が言語ごとに違う。承認のページも、差分の読み方も、ファイルの単位で分かれている。
- **リポジトリを分けたまま、土台だけを公開するクレートにする**：診断や出典の重なりは消えるが、境目は JSON のまま残り、バージョンの食い違い（1.4）も残る。このまとめの目的（境目で証明を切らない）に届かない。
- **一つのリポジトリに入れ、プロセスの境目は残す**：同じ理由。境目の問い（7 章）が子プロセスと JSON の往復になる。
- **生成器の共通の中間表現**：一般のプログラムの表現になり、rulec の表の一行が一つの分岐になる読みやすさや、koyomi の操作ごとの関数が消える。共通にするのは表面にかかわる部分（9.2）まで。
- **`git subtree` で取り込む**：12.2。
- **Lean 4 で処理系を書く**：生成器、診断、CLI、テストの共通部分まで Lean で書く利点が無く、ビルドとツールの手間が増える。意味の中心部分だけを Lean のモデルにし、Rust と突き合わせる（11 章）。
- **言語のコマンドを `ritsu <言語>` だけにして、`rulec` などの名前を捨てる**：rulec は Homebrew と GitHub Actions で配られ、README とサイトがその名前で入れ方と CI の書き方を案内している。名前を残すのはリンク一つで済む。
- **土台を serde_json に依存させる**：4.9。
- **yuen と sakai が、ほかの言語のファイルを自分で読み解く**：二つの読み手が同じ言語の構文を持つことになる（sakai の DESIGN 4.7 の C と同じ理由）。口を通して読む。
- **言語ごとの診断のコードを一つの番号に振り直す**：rulec の診断のコードは、rulec の docs/compatibility.md が 1.0 から保つと書いているものである。番号は言語ごとのまま残し、`ritsu check` の中でだけツールの語を添える（8.3）。

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
- **LSP の、診断、定義へ移る、型と範囲を見せる、中のものの一覧、rulec の整形、のほかの機能**。
