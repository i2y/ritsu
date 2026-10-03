# ritsu 設計文書

七つの小さな言語（rulec、dandori、koyomi、chobo、geas、yurai、sakai）を、一つのリポジトリの一つの処理系にまとめる。言語は七つのまま残し、土台（診断、二つの言語の文、出典、名指し、doc のページの枠、テストの共通部分）、単位の型、プロジェクトの読み込み、CLI、バージョン、Lean の層を一つにする。目的は、ある言語が確かめたことを、隣の言語が型の付いたまま受け取れるようにすることである。

名前は律（りつ）から取った。

この文書は段階 A（設計）で書いた。コードはまだ無い。1 章の行数と数、1.4 と 12.5 の出力は、2026-10-03 にこの機械（macOS arm64、rustc 1.94.1）で、各リポジトリを読み、作業場所に写したものを走らせて取った。元のリポジトリでは何もビルドしていない。各リポジトリのテストの件数と時間のうち、ここで走らせていないものは、それぞれの最後の記録から引き、そう書いた。段階ごとの作業と完了の条件は PLAN.md にある。

## 0. 全体像

```
ritsu（バイナリ。CLI と LSP。ritsu-wasm はブラウザで動かすもの）
 ├── ritsu-cross    言語をまたぐ検査（7 章）
 └── ritsu-project  プロジェクトの読み込み、名前の解決、口のつなぎ（6 章）
      ├── rulec  dandori  koyomi  chobo  geas  yurai  sakai     言語のクレート（互いに依存しない）
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

看板の言い方は、次を候補にする（★作者が決める）。

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
- **P9**：土台の層は std だけで書き、外のクレートに依存しない。いま serde_json を使っている五つ（dandori、koyomi、chobo、yurai、sakai）はそのまま使ってよい。rulec と geas は依存が無いまま保つ（4.9）。

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
- **境目**：ある言語の値や事実が、別の言語に渡るところ。dandori のフローが規則を呼ぶところ、yurai の要件が規則の表を名指すところなど。
- **出す側、受け取る側**：口を通して事実を出す言語（rulec、koyomi、chobo、geas）と、それを受け取る言語（dandori、yurai、sakai）。dandori は出す側でもあり（タスクと案件を yurai と sakai に出す）、rulec は入力の範囲を koyomi から受け取るときに受け取る側になる（7.5）。
- **成果物**、**名指し**：yurai と sakai の DESIGN.md の 2 章と同じ意味（6.2）。

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
| yurai | 9,878 | 30 | 94 | 2,393 | serde_json | 2024 | 0.1.0 | 1 | なし |
| sakai | 9,190 | 31 | 97 | 2,443 | serde_json | 2024 | 0.1.0 | 1 | なし |
| 合計 | 163,913 | 244 | 1,305 | 42,272 | | | | | |

serde_json を使う五つの `Cargo.lock` は、どれも serde_json 1.0.151 と同じ 15 のパッケージを固定している。rulec と geas の `Cargo.lock` には自分しか無い。rulec には Lean の証明（`proofs/`、5,476 行、Lean v4.34.0、mathlib なし）がある。dandori だけが edition 2021 で、`--version` と `explain` を持たない。

### 1.2 重なっているコード

同じ役目のコードを、言語ごとに書いている。行数は 2026-10-03 のもの。

| 重なり | ファイルと行数 | 合計 | 土台に移したあと、言語に残るもの |
|---|---|---|---|
| SHA-256 | rulec `src/sha256.rs` 87、koyomi `src/sha256.rs` 75、yurai `src/sha256.rs` 75、sakai `src/sha256.rs` 84、chobo `src/ids.rs` の 12〜79 行（68） | 389 | なし。chobo の ID の決め方は chobo に残る |
| 診断 | `src/diag.rs`：rulec 549、dandori 159、koyomi 335、chobo 184、geas 260、yurai 223、sakai 214 | 1,924 | そこに至る例の部分（rulec の枠と入力の例、dandori の実行、koyomi の計算の段、chobo の操作、yurai の差分とつながり、sakai の関わるもの） |
| 二つの言語の文 | `src/i18n.rs`：rulec 76、koyomi 155、yurai 167、sakai 183。ほかに chobo の `src/lib.rs` の `tr!` と `src/diag.rs` の `Text`、geas の `src/diag.rs` の `Lang` と `Text`、dandori の `src/diag.rs` の `Lang` | 581（`i18n.rs` だけ） | なし |
| 診断の台帳の枠 | `src/codes.rs` の `find` と書き出し：rulec 2064〜2198 行（135）、chobo 767〜854（88）、geas 1075〜1182（108）、koyomi 467〜539（73）、yurai 413〜500（88）、sakai 703〜776（74） | 566 | 台帳の中身（コードごとの文と再現。`codes.rs` の全体は rulec 2,198、koyomi 539、chobo 854、geas 1,182、yurai 500、sakai 776） |
| CLI の表 | koyomi `src/cli.rs` 412、yurai `src/cli.rs` 412、sakai `src/cli.rs` 330、chobo `src/main.rs` の 20〜346 行。表を読む仕組みの部分は、それぞれ 190〜240 行ほど | 約 875（仕組みの部分） | コマンドとフラグの一覧。rulec の `src/main.rs`（2,214）と geas の `src/main.rs`（782）は別の形の表を持ち、dandori の `src/main.rs`（305）は表を持たない |
| 出典の写しと固定、改正の検知 | rulec `src/sources.rs` の 20〜313 行と 1047〜2142 行（1,390）、koyomi `src/fetch.rs` 581 と `src/sources.rs` の 199〜524 行（326）、yurai `src/fetch.rs` 762・`src/copies.rs` 323・`src/base64.rs` 74・`src/sources.rs` 305 | 約 3,760 | rulec の写しと表の突き合わせ（`src/sources.rs` の 534〜1009 行）、koyomi の祝日の表、yurai の借りた出典 |
| 名指し | yurai `src/names.rs` 422、sakai `src/naming.rs` 367。ほかにルートとパスの扱いが sakai `src/paths.rs` 268 と yurai `src/project.rs` の一部、geas `src/tree.rs` の一部 | 789（+268） | なし |
| `.proto` の読み手 | rulec `src/proto.rs` 1,358、dandori `src/proto.rs` 1,153、sakai `src/proto.rs` 1,080。yurai は段階 C で四つめを書く予定だった | 3,591 | 読んだものの使い方（rulec の契約の突き合わせ、dandori の `connect` と `implements`、sakai の境界を越える参照） |
| JSON（依存の無い二つ） | rulec `src/json.rs` 456、geas `src/json.rs` 513。geas は `tests/common/mod.rs` にも読み手（336〜497 行） | 969 | なし |
| doc のページの CSS | rulec `src/doc.rs` 2369〜2402 行、dandori `src/doc.rs` 2039〜2166、koyomi `src/doc/html.rs` 298〜404、chobo `src/draw.rs` 528〜600。どれも明るい配色と暗い配色を持ち、色の変数の名前と値がそれぞれ違う | 約 340 | ページの中身 |
| 生成物の予約語 | rulec `src/backend.rs` の `words`（18〜206 行、12 言語）、koyomi `src/reserved.rs` 76、dandori `src/temporal_py.rs` と `src/temporal_go.rs`、chobo `src/client/python.rs` と `src/client/go.rs` | 約 300 | なし |
| wasm の境目 | rulec `src/wasm.rs` 194、dandori `src/wasm.rs` 142。どちらも「バッファの頭に長さを書く」同じ決まり | — | 呼び出しの中身 |
| テストの共通部分 | `tests/common`：koyomi 247、chobo 1,040（`mod.rs` 116、`runners.rs` 730、`servers.rs` 194）、geas 546、yurai 185、sakai 337 | 2,355 | 言語ごとのランナー（chobo の `runners.rs` など） |

テストの共通部分の中で重なっているのは次のもの。

- 自分を消す一時ディレクトリ（終わったテストのプロセスの分も消す）：koyomi 23 行、chobo 51、geas 49、yurai 36、sakai 51。dandori は `tests/examples.rs` の中に持つ。
- 時間を区切って子プロセスを走らせる（macOS に `timeout` が無いため）：koyomi 87 行、geas 58、sakai 39。
- golden と取り直し（`<名前>_BLESS=1`）：chobo 26 行、geas 57、yurai 14、sakai 11。koyomi、dandori、rulec はテストの中に書いている。
- 使い捨ての PostgreSQL のクラスタ：koyomi 66 行、chobo 107。rulec は `PG*` の環境変数で受け取る（CI はサービスで立てる）。
- Chrome を探す：dandori の `tests/doc.rs` と `tests/playground.rs`、koyomi と chobo の `tests/doc.rs`、geas の `tests/common/mod.rs`。順は五つとも同じ（環境変数、macOS の既定の場所、PATH）。
- Mermaid で図を描けるかを確かめる：dandori と chobo の `tests/doc.rs`。`tools/mermaid` も二つある。

表の合計を足すと、テストの共通部分を除いて約 1 万 4 千行になる（診断のうち言語に残る部分も含む）。土台に移せば、これが 6 千行ほど（`.proto` の読み手 1,600 行と出典 1,500 行ほどを含む）になる見込みである。この 6 千行は見込みで、測ったものではない。

キーワードの表（各言語の `kw.rs` や `syntax.rs`）、字句と構文、検査、参照インタプリタは重なりに数えない。言語ごとの語彙と意味そのものだからである。

### 1.3 同じ考えで、形が違うもの

- **診断の JSON のキー**：dandori は `code`・`severity`・`line`・`col`・`message`・`notes`・`path`（ファイルは外側に）、chobo は `v`・`column`・`title`・`excerpt`・`operations`・`hint`、geas は dandori の形に `file`、koyomi は `inputs`・`steps`・`fails`・`fix`、yurai は `diff`・`chain`・`candidates`、sakai は `references`。ファイルのパスは、yurai がルートからの相対、sakai が走らせたディレクトリからの相対で食い違っている（6.2 の 9）。rulec は別の形（`v` が 2 の形。`where`・`witness`・`rows`・`fix`）。
- **二つの言語の文の書き方**：rulec、koyomi、chobo、yurai、sakai は `tr!("日本語", "English")`（日本語が先）、geas は `t(en, ja)`、dandori は `Diag::error(code, line, col, en, ja)`（英語が先）。rulec の `tr!` は、プロセスで一つの言語（`src/i18n.rs` の `AtomicU8`）を読んで `String` を返す。ほかの四つの `tr!` は、二つの文を持つ `Text` を返し、どちらを出すかは出すところが決める。
- **SKIP の書き方**：`SKIP:` の行（dandori 55 か所、chobo 27、koyomi 11、yurai 6、sakai 5、geas 1）、rulec のテストの `注意:`（53 か所。rulec のテストは日本語で回すため）、`rulec test` の `skipped`。
- **ルートの決め方**：geas（`src/tree.rs`）、yurai（`src/project.rs`）、sakai（`src/paths.rs`）が、それぞれ「いちばん近い `.git` のあるディレクトリ」を探す。
- **言語と取り直しの環境変数**：`RULEC_LANG`、`DANDORI_LANG`、`KOYOMI_LANG`、`CHOBO_LANG`、`GEAS_LANG`、`YURAI_LANG`、`SAKAI_LANG`。取り直しは `<名前>_BLESS`。
- **名前の正規化**：rulec の DESIGN §1.1 は「識別子は NFC に正規化する」と書くが、src には無い。chobo は結合文字を含む名前を E001 で断る。

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

rulec の中では `JPY` は `円` の別の綴りで（rulec の DESIGN §15.18、`src/types.rs` の `money_unit`）、dandori は単位を文字列のまま比べる（dandori の `src/model.rs` の `Ty::Num(String)` と `src/rulec.rs` の `normalize_unit`）。

**率の刻みが、説明の文にしか無い。** `off : rate[step 0.1%]` を入力に持つ規則で：

```
$ rulec schema rate.rule --lang en
… "off": {"type": "integer", "description": "an integer: the rate as a count of 0.1% steps (100% is 1000)", "minimum": 0, "maximum": 500}
$ rulec schema rate.rule --lang ja
… "off": {"type": "integer", "description": "整数。率を 0.1% 刻みの個数で書く（100% なら 1000）", "minimum": 0, "maximum": 500}
$ rulec certificate rate.rule
… "types": {"off": "rate", "pay": "money[円, incl_tax]", "price": "money[円, incl_tax]"}
```

証明書の型は `rate` だけで、刻みは説明の文の中にしか無い。dandori は `src/rulec.rs` の `rate_scale` で、その文から `100% is ` か `100% なら ` のあとの数を読んでいる（dandori の DESIGN 7 章にも残してある）。

**バージョンの文字列で、二つのツールが食い違う。** dandori は、`rulec doc` が描いたものを読み解かずに埋め込む。そのため dandori の `tests/` と `website/` の 57 のファイルに `rulec 0.22.0` という文字列があり、rulec を 0.22.1 にすると、テストは中身が同じでもバージョンの文字列で落ちる（dandori の README は「tested with rulec 0.22.0」と書く）。一つの処理系なら、二つは同じバージョンでしかありえない。

**境目のためのコードと、止めている作業。**

- dandori の `src/rulec.rs`（678 行）は、規則ごとに rulec を三回（`schema`、`certificate`、`api`）子プロセスで走らせ、JSON を読む。ブラウザで試すページでは rulec を走らせられないので、`src/sources.rs`（292 行）が、記録しておいた rulec の出力を返す。
- yurai は、rulec、koyomi の日付、geas、dandori の成果物を、ファイルの単位でしか名指せない（yurai の DESIGN 3.2）。JSON が、表や日付の関数の一つ一つの定義を出さないからである。
- sakai は、dandori の参照を確かめられず、N101 で「確かめていない」と言う（sakai の DESIGN 4.7）。
- yurai の段階 C の「一式の読み込み」（yurai の PLAN の C.1〜C.9）と、sakai の段階 C の C.1〜C.5 は、この処理系の形が決まるまで止めてある。
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
    rulec/  dandori/  koyomi/  chobo/  geas/  yurai/  sakai/                            元のリポジトリを履歴ごと（B）
    ritsu-project/  ritsu-cross/                                                        つなぎの層（E）
    ritsu/  ritsu-wasm/                                                                 入口（E、F）
    xtask/                 テストの段、SKIP の集計、変えたクレートの選び出し、依存の決まりの確かめ（C）
  proofs/                Lean の層（F で rulec の proofs/ をここへ移し、モデルを足す。11 章）
  skills/ritsu/          エージェント向けのスキル（F）
  .github/workflows/     CI（10.5）
```

言語のクレートの中は、元のリポジトリの木をそのまま残す（README、DESIGN.md、PLAN.md、`tests/`、`examples/`、`tools/`、`docs/`、`skills/`、rulec と dandori の `website/`、rulec の `proofs/` と `experiments/`）。元の `.github/workflows/` も `crates/rulec/.github/` と `crates/dandori/.github/` に来るが、GitHub はそこにあるワークフローを走らせない。ritsu の CI は根の `.github/workflows/` に新しく書く（C）。

### 2.2 クレートと名前

| クレート | 役目 | 元にするもの |
|---|---|---|
| `ritsu-base` | 土台（4 章） | 1.2 の重なり |
| `ritsu-units` | 単位の型（5 章） | rulec の `src/types.rs` の単位の表と `src/num.rs` の有理数 |
| `ritsu-ports` | 口（3.2） | dandori の `src/rulec.rs` が JSON から組み立てている型、yurai と sakai が読む予定だったもの |
| `ritsu-proto` | `.proto` の読み手（4.10） | sakai の `src/proto.rs`（型の名前の解決）に、rulec の Protovalidate と buf の依存の読み方、dandori の `json_name` とオプションの読み方を足す |
| `ritsu-emit` | 生成先の言語ごとの予約語、識別子、文字列のリテラル、生成物の頭（9.2） | rulec の `src/backend.rs`、koyomi の `src/reserved.rs` と `src/naming.rs`、dandori と chobo の予約語の表 |
| `ritsu-testkit` | テストの共通部分（10 章） | 五つの `tests/common` と、dandori の `tests/` の中の同じ役目のコード |
| `rulec` `dandori` `koyomi` `chobo` `geas` `yurai` `sakai` | 言語 | 元のリポジトリ。パッケージの名前もバイナリの名前も変えない |
| `ritsu-project` | プロジェクトの読み込み、索引、名前の解決、口の実装をつなぐ（6 章） | 新しく書く |
| `ritsu-cross` | 言語をまたぐ検査と、その診断の台帳（7 章） | 新しく書く |
| `ritsu` | バイナリ `ritsu`（CLI と `ritsu lsp`） | 新しく書く |
| `ritsu-wasm` | ブラウザで動かす wasm32 のモジュール | rulec と dandori の `src/wasm.rs` |
| `xtask` | 開発のための作業。公開しない | 新しく書く |

言語のクレートのパッケージの名前を変えないのは、コード（`use rulec::…`）、テスト（`env!("CARGO_BIN_EXE_rulec")`）、環境変数の名前を、B の段階で一つも直さずに済ませるためである。crates.io では `koyomi`（「Japanese calendar written in Rust」、0.4.0）と `yurai`（「Forensics-grade provenance explorer for AI models」、0.4.0）が別のクレートに使われている（2026-10-03 に crates.io の API で確かめた。`ritsu`、`rulec`、`dandori`、`chobo`、`geas`、`sakai`、`ritsu-base`、`ritsu-units` は空いていた）。中のクレートは公開しない（13.2）ので、いまは困らない。

### 2.3 バイナリ

- 言語のクレートは、いまの `[[bin]]`（`rulec` など）を残す。開発と、そのクレートのテストが使う。
- すべての言語をつなぐバイナリは `crates/ritsu` の `ritsu` だけである。
- リリースで配るのは `ritsu` 一つで、`rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`yurai`、`sakai` はそれを指すリンクにする。リンクの名前で呼ばれたら、その言語のコマンドとして、すべての口をつないで動く（8.2）。Cargo のバイナリの名前は、ワークスペースの中で重ならない（言語のクレートの `rulec` と、リリースのリンクの `rulec` は、作られる場所が違う）。
- 受け取る側（dandori、yurai、sakai）のクレートのバイナリは、D の段階からほかの言語を読めない（ほかの言語のクレートに依存しないため）。ほかの言語を読むところに来たら、`ritsu <言語>` で走らせるよう言う診断を出す。dandori のクレートのバイナリは、規則を使わないフローならいまと同じに動く。

## 3. 依存の決まり

### 3.1 層

| 層 | クレート | 依存してよいもの |
|---|---|---|
| 土台 | `ritsu-base` | std だけ |
| | `ritsu-units` | `ritsu-base` |
| | `ritsu-proto`、`ritsu-emit` | `ritsu-base` |
| | `ritsu-ports` | `ritsu-base`、`ritsu-units` |
| 言語 | `rulec` `dandori` `koyomi` `chobo` `geas` `yurai` `sakai` | 土台の層。serde_json（いま使っている五つだけ） |
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
| `Rules` | rulec | dandori、yurai、sakai、ritsu-cross | 入力と出力（名前、別名、単位の付いた型、範囲、率の刻み）、列挙（値の名前と、Connect のワイヤでの名前と番号）、ステートマシン（軸、行が受け付ける座標、遷移、書く値、held）、前提（`constraint`・`sum`・`length`）、Connect のサービスの形、生成したコードの呼び方、`rulec doc` が描いたもの。問いは「この範囲の値で、前提は必ず成り立つか」と「この入力の値をこの集合に限ったとき、表は完全で、重なりが無く、当てはまらない行が無いか」。評価は入力から出力 |
| `Dates` | koyomi | dandori、rulec、ritsu-cross | 関数、引数の型と範囲、カレンダーとデータの範囲、`at` の時刻と UTC オフセット、条件の名前と文。問いは「入力の範囲で、関数がとりうる値の集合」と「入力から値までの日数の最小と最大」。評価 |
| `Books` | chobo | dandori、ritsu-cross | 単位、勘定と境界と断る理由、振替の種類（引数と単位、キー、仮押さえと有効期限、移動）、仮押さえのステートマシン。問いは「額がこの範囲のとき、どの操作が、どの理由で断られうるか」。評価（帳簿の状態を持つ） |
| `Claims` | geas | yurai、ritsu-cross | 主張の一覧（名前、行、書いたとおりの手順）。map の記録の読み方 |
| `Items` | 七つ全部 | yurai、sakai、LSP | 中のもの（種類、名前、行の範囲、定義の文）。6.4 |
| `References` | 七つ全部 | sakai、yurai、LSP | 参照（行、先の名指し、参照の仕方）。6.4 |

どの問いの答えも、P5 の三つのどれかになる。

```rust
// ritsu-ports のスケッチ。名前と細部は C と D で決める
pub enum Answer<E> {
    Holds,                 // 成り立つことを示した
    Fails(E),              // 成り立たない例（入力の値と、そこに至るもの）
    Undecided(Text),       // 決められない。理由を言う
}

pub trait Rules {
    fn facts(&self, rule: &Name) -> Result<RuleFacts, Vec<Diag>>;
    fn preconditions_hold(&self, rule: &Name, ranges: &[(String, Range)]) -> Vec<(Precondition, Answer<Values>)>;
    fn checked_over(&self, rule: &Name, input: &str, days: &DaySet) -> Answer<Diag>;
    fn eval(&self, rule: &Name, inputs: &Values) -> Result<Values, RuleError>;
}
```

`RuleFacts` は、dandori の `src/rulec.rs` がいま三つの JSON から組み立てている `RuleInfo`（入力と出力の `Column`、列挙、`Machine`、前提、`walks`）を、単位の型（5 章）の付いた形にしたものである。D の段階では、例のすべての規則について、型の付いた呼び出しで得た事実と、いまの JSON から読んだ事実が同じになることを一度確かめてから、JSON の読み手を消す。

### 3.3 テストと dev-dependency

受け取る側のクレートのテストは、出す側のクレートを `[dev-dependencies]` に持ち、本物の実装をつないで走らせてよい。口のトレイトは出す側が実装するので（3.2）、つなぎ方は一か所にしか無い。出す側は受け取る側に依存しないので、依存は輪にならない。

受け取る側のクレートのテストのうち、いまバイナリを走らせて規則などを読むもの（dandori の `tests/examples.rs` など）は、D の段階で、CLI を関数として呼ぶ形（`dandori::cli::run(引数, 口, 標準出力, 標準エラー)`）に替える。すべてをつないだバイナリを走らせるテストは、`crates/ritsu/tests/` に置く。

### 3.4 決まりの確かめ方

- C の段階：`xtask` に、`cargo metadata` を読んで 3.1 の表と突き合わせる確かめを置き、CI の `fast` のジョブで走らせる。破れば落ちる。
- E の段階：この処理系そのものの地図 `ritsu.ctx` を sakai で書く。クレートをコンテキストに、`ritsu-ports` と `ritsu-units` と `ritsu-base` を言語のクレートの上流の公表された言語にし、言語のクレートどうしの参照を許さない。Rust では、`use` できるクレートは `Cargo.toml` の依存に限られ、コンパイラがそれ以外を断るので、境界を越える参照は `Cargo.toml` の依存として読める。sakai の決まり（sakai の P6「コードの import は、各言語の既存のツールの設定にして、そのツールで確かめる」）に合わせて、cargo-deny の `[bans]` の `wrappers`（そのクレートに依存してよいクレートを並べる）の設定を `sakai build --target cargo-deny` で書く形を第一の案にする。cargo-deny がワークスペースの中のクレートどうしの依存にも効くかは、E の段階で確かめる。効かなければ、sakai が `cargo metadata` の JSON を読む形にする。どちらでも `ritsu check .` が ritsu のリポジトリに地図を当て、CI で走らせる（7.13）。

### 3.5 捨てたもの

- **言語のクレートどうしが直接依存する形**（dandori が rulec に依存する）：rulec の中の型（AST、型付けの結果）が dandori に漏れ、rulec の中を直すたびに dandori が壊れる。口を挟めば、渡すものは決めた型に絞られる。
- **一つのクレートにまとめる形**：依存の決まりがモジュールの規約になり、コンパイラが守らない。
- **プロセスの境目を残し、JSON を型の付いた形式に直す形**：一つの処理系にする理由（型の付いたまま渡す）が無くなる。7 章の問い（「この集合で完全か」）は、渡すたびに子プロセスと JSON の往復になり、二つのツールのバージョンが違えば答えも食い違う。

## 4. 土台

`ritsu-base` に置くものと、その元。どれも言語の意味を持たない。

### 4.1 二つの言語の文

`Text`（日本語と英語の文の組）と `tr!("日本語", "English")` を一つにする。koyomi、chobo、yurai、sakai の形で、どちらの文を出すかは出すところが決める。同じプロセスの中で英語と日本語の golden を取れ、wasm では呼ぶたびに言語を変えられ、ほかの言語のクレートからも、欲しい言語で呼べる。

- geas の `t(en, ja)` と dandori の `(en, ja)` の組は、C の段階で `tr!("日本語", "English")` の順に直す。機械的な直しで、文は一字も変えない。
- rulec の `tr!` は、プロセスで一つの言語を読む（呼び出しは 2,800 か所）。C の段階ではそのままにする。D の段階で、言語をスレッドごとに持てるようにする（`i18n::with(lang, || …)`）。dandori や yurai が rulec を同じプロセスの中で、ほかのテストと並んで、違う言語で呼ぶからである。CLI の振る舞いは変わらない。rulec のすべての文を `Text` に移すことは、要るとわかるまでしない（15 章）。
- 言語の選び方は、`--lang`、`<名前>_LANG`、`RITSU_LANG`、英語の順。システムのロケールは見ない（rulec の §11 の原則 7。生成物と CI のログが機械で変わらないため）。
- 文の幅（East Asian Width で W と F を 2 と数える）、件数、日本語の空白の詰め方の小さな関数も置く。

### 4.2 診断

一つの `Diag` に、どの言語にも共通の部分を置く。コード、重さ（エラー、警告、備考）、ファイル、行、列、文、注（`= ` で始まる行）、直し方。テキストの形は、dandori、koyomi、chobo、geas、yurai、sakai がもう使っている形（`error[E301]: <ファイル>:<行>:<列>: <文>`、原文の行、注、そこに至る例）にする。

そこに至る例の部分は、言語ごとに中身が違う（1.2 の表）。そこは言語ごとの型にし、テキストと JSON への書き方だけを `Diag` が呼ぶ小さなトレイトで決める。

JSON は、キーを英語で固定し、`code`、`severity`、`file`、`line`、`col`、`message`、`notes`、`fix` と、言語ごとの部分のキーにする。`file` はルートからの相対で、JSON の外側に `root`（走らせたディレクトリから見たルート）を添える（6.2 の 9）。いまの形から変わるのは、chobo の `column`・`title`（→ `col`・`message`）、sakai の `file`（走らせたディレクトリから → ルートから）などで、変える言語の DESIGN.md に理由を書く（P6）。rulec の `--format json`（`v` が 2 の形）は変えない。rulec の診断は rulec の `src/diag.rs` に残し、`ritsu check` の JSON に入れるときだけ、共通のキーを外側に足す（8.3）。

テキストの中のパスの書き方は、6.2 の 8 のとおり。ファイルの場所は走らせたディレクトリから（渡されたとおりに）、文の中の名指しはルートからの相対で書く。

### 4.3 診断の台帳と explain

`Entry`（コード、重さ、一行の題、いつ出るか、直し方、最小の再現、隣に置くファイル、関連するコード）と、`find`、テキストと Markdown（コードごとのアンカーつき）と JSON の書き出し、そしてどの再現も自分のコードを出すことを確かめるテストの共通部分を置く。台帳の中身は言語ごとに残す。`docs/codes.md` と `docs/codes.ja.md` は、どの言語も `explain --all --format markdown` の出力そのものにする（いまもそうしている五つの形）。

### 4.4 CLI の表

koyomi、yurai、sakai、chobo の形を一つにする。コマンドとフラグを一枚の表に置き、`--help` の表示と引数の読み取りが同じ表を引く。知らないフラグ、閉じた集合の外の値、値の無いフラグ、繰り返せないフラグの二度目は exit 2（rulec の §12.1）。繰り返せるフラグは表に書く（yurai の `--map`）。終了コードは 0（問題なし、警告と備考だけ）、1（エラー）、2（使い方の誤り、読めないファイル、中の異常）。

rulec の `src/main.rs` の表は同じ決まりで作られているので、C の段階では動かさない（合うところだけ移す）。dandori は C の段階でこの表に移し、`--version` とコマンドごとの `--help` を足す。足すだけで、いまのコマンドとフラグは変えない。

### 4.5 ハッシュ

SHA-256（FIPS 180-4）を一つにし、`hex` と、先頭 16 桁の `short` を置く。FIPS の既知の値でテストする。chobo の ID の決め方（長さを頭に付けた部分を SHA-256 に通す）は chobo に残し、SHA-256 だけを土台から使う。geas の SHA-1（git の blob の ID）は geas にしか要らないので geas に残す。

### 4.6 出典の固定と改正の検知

rulec、koyomi、yurai の三つは、法令の写しを同じ場所と同じ名前で持つ（`sources/law/<法令の ID>@<時点>/<要素>.xml`。yurai の `src/copies.rs` の頭に、そうそろえたと書いてある）。そこで、次を一つにする。

- 引用の書き方から要素の名前を作る（e-Gov の `第143条第2項` → `MainProvision-Article_143-Paragraph_2`、`別表第一` → `AppdxTable_1`、附則。eCFR の節）。
- 写しの場所、写しの本文（タグを落とした文、項ごとの行）、固定（先頭 16 桁）、`.rule`・`.cal`・`.req` の固定の行の書き換え。
- 通信：`curl` を子プロセスで呼び、三度まで試す（二度めの前に 2 秒、三度めの前に 4 秒）。e-Gov の法令 API v2（`law_data` と `law_revisions`）、eCFR（全文と版の一覧）、GitHub の raw のファイル（rulec）、e-Gov の応答の base64。

`source fetch | pin | outdated` のコマンドは、それぞれの言語に残す（固定の行の書き方は言語の構文だからである）。`check` は通信しない。

一つになれば、同じプロジェクトの規則とカレンダーと要件が同じ条を引くとき、写しも一つで済む。yurai の E107（要件の写しと規則の写しの本文の食い違い）は、写しが一つなら起きにくくなり、二つあるときも同じ手続きで比べられる（7.11）。

### 4.7 名指しとパス

名指しの決まり（6.2）を一つの実装にする。いまは yurai の `src/names.rs` と sakai の `src/naming.rs` が別々に書き、同じ 36 行の `naming.tsv` を通している。その表は土台のテストに移す。

パスは、ルートの決め方（`--root`、無ければ最初に渡したパスの上でいちばん近い `.git` のあるディレクトリ、それも無ければ渡したディレクトリ）、`.` と `..` を字の上で畳むこと、表示のパス（走らせたディレクトリから）、ディレクトリを歩くときに飛ばす名前（`.git`、`target`、`node_modules`、`.venv`、`.geas`、`__pycache__`）を一つにする。geas、yurai、sakai が別々に持っているものである。

12.5 で、取り込んだ形（ルートが上にある形）で yurai を走らせたら、写しのパスを `../../crates/yurai/tests/…` と回り道で書いた。土台の表示のパスは、走らせたディレクトリからのいちばん短い相対にする。

### 4.8 doc のページの枠

承認する人のページ（rulec、dandori、koyomi、chobo の `doc`。yurai と sakai は F で作る）の枠を一つにする。

- HTML：`<!doctype html>`、`lang`、viewport、ツールとバージョンを書く `generator`、題、一つの CSS。CSS は色の変数を明るい配色と暗い配色で持ち、`prefers-color-scheme` と `data-theme` で切り替える。ページの頭に、元のファイルのパスと SHA-256 の先頭 16 桁とツールのバージョンを書く。外のファイルを読まない（スクリプトも CSS も中に書く）。狭い画面で横にはみ出さない。
- Markdown：頭のコメント（`<!-- Generated by <ツール> <バージョン> from <パス> (sha256:…) -->`）。

ページの中身（rulec の表とカード、dandori の図とシナリオ、koyomi の月の表、chobo の残高）は言語に残す。色の値をそろえると見た目が少し変わるので、C の段階で golden とスクリーンショットを取り直し、変わったページを報告に並べる。

### 4.9 JSON

土台は外のクレートに依存しない（P9）ので、診断や名指しの JSON は、std だけで書いた小さな JSON の値の型で書く。キーの順を保ち、整数を正確に持つ。元は rulec の `src/json.rs`（456 行。geas の `src/json.rs` も同じ役目）。serde_json を使う五つは、土台の JSON を文字列にして読み直すか、そのまま埋め込む。

serde_json を土台に入れない理由は、rulec と geas が依存の無いことを保っているからである。rulec の README は、依存が無いので `cargo install --path .` が何も取ってこないと書き、rulec の DESIGN §12.1 は引数のパーサを入れない理由に、依存を足さない方針を挙げている。ritsu 全体のバイナリには、いまと同じく serde_json が入る。

### 4.10 `.proto` の読み手

`ritsu-proto` は proto3 を読む。`syntax`、`package`、`import`（`public` と `weak` も）、入れ子のメッセージ、フィールド（`json_name`、`optional`、`repeated`、`map`、`oneof`）、列挙と値と番号、サービスとメソッド（ストリームかどうか）、サービスとメソッドのオプション（dandori の `(dandori.v1.workflow)` など）、Protovalidate の規則（`buf.validate.field` の整数の範囲と `required`。CEL の式は文字列のまま渡し、rulec の `src/cel.rs` が読む）、`buf.yaml` の依存と `buf.lock` の固定（rulec）。Google の well-known types、`buf/validate/validate.proto`、`dandori/v1/options.proto` はファイルが無くても知っているものとして扱う。型の名前は protobuf の決まり（内側から外へ、package を一段ずつ）で解決する（sakai の形）。

読み手が一つになれば、同じ `.proto` を、rulec の契約の突き合わせ、dandori の `connect` と `implements`、sakai の境界を越える参照、yurai の端が、同じに読む（7.12）。

### 4.11 土台に置かないもの

キーワードの表、字句と構文、名前の解決、検査、参照インタプリタ、生成器の芯は、言語に残す。どれも言語の語彙と意味そのものである。

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

### 5.2 rulec

rulec の `Ty::Money`・`Ty::Qty`・`Ty::Rate`・`Ty::Number` は、中に `ritsu_units::Unit` を持つ形にする。振る舞いは変えない（D の段階で、コーパスの golden と証明書が一字も変わらないことを確かめる）。率の刻みは、入力と出力の型として `Rules` の口に出す。dandori は説明の文から刻みを読まなくなる。

### 5.3 dandori

- `Ty::Num(String)` を `Ty::Num(Unit)` にする。`src/syntax.rs` の `UNIT_KINDS`（rulec の九つの次元を写した表）は `ritsu-units` から引く。
- 二つの単位が同じかは、単位の型で決める。1.4 の例（`money[JPY, incl_tax]` を `money[円, incl_tax]` に渡す）は通る。
- 同じ次元で違う単位（`mass[kg]` を `mass[g]` に渡す）は、いまと同じく E003 で断る。dandori の式は計算しない（dandori の P1）ので、境目で黙って換算すれば、`.flow` が計算をすることになる。換算は規則に書く。
- `src/model.rs` の `rate_unit` と `rate_per`（刻みを文字列で作って読む）は、単位の型の `step` に替える。
- 範囲の端に単位を付けて書けるようにする（dandori の DESIGN 7 章に残っていたもの）。`range >=1kg` は `mass[g]` の場所では 1000 で、整数にならない換算は rulec と同じく断る。
- 値はいまと同じく、宣言した単位で数えた JSON の整数として運ぶ。どのプラットフォームの生成物も変わらない。`int` は単位の無い数（rulec の `number`）のまま。

### 5.4 chobo

chobo の単位（`unit 個`、`unit 円`、`unit USD scale 2`）は、名前だけを比べる単位で、量はいちばん小さい単位で数えた整数である。これを次のように単位の型に載せる。

- 名前が通貨の表にあれば、`scale 0` なら `money[<通貨>]`、`scale 2` なら `money[<通貨>c]`（百分の一で数える）。`円` と `JPY` は `scale 0` だけ（百分の一は `銭`）。ほかの scale は chobo だけの単位にする。
- 名前が質量や長さなどの単位の綴り（`g`、`kg`、`L` など）で `scale 0` なら、その単位。
- それ以外は、名前だけの数の単位 `Count(<名前>)` にし、自分とだけ同じにする。rulec は規則のためにラベルの付いた数を捨てた（rulec の §15.11）が、chobo では、同じ数え方のもの（商品の `個` と予約の枠の `席`）を分けるのに要る。

chobo のお金の単位には、税込か税抜かの区別が無い。rulec の `money[円, incl_tax]` を chobo の `unit 円` の額に渡せるようにすると、一つの勘定に税込の額と税抜の額が入りうる。rulec が型で止めていること（E103）が、chobo の勘定で破れる。そこで、chobo のお金の単位に区別を書けるようにする（`unit 円 incl_tax`）。境目では区別が同じでなければ渡せない。区別の無い chobo の単位は、区別の無い額（rulec の `money[円]`）だけを受け取る。chobo の中の意味は変わらない（★作者が決めること。chobo の構文を一つ足す）。

chobo の額は 0 から 2⁶³ − 1 までで、rulec の値は負にもなりうる（返金など）。これは 7.6 の検査が見る。

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

ファイルの種類は拡張子で決める：`.rule`（rulec）、`.flow`（dandori。`.ja.flow` も）、`.cal`（koyomi）、`.book`（chobo）、`.geas`（geas）、`.req`（yurai）、`.ctx`（sakai）、`.proto`。ほかのファイルは、誰かが名指したとき（yurai と sakai の `file "…"`、dandori の `use openapi|smithy` のパス）にだけ読む。

読み方の順：

1. 全部のファイルを一度ずつ読み、言語ごとに字句、構文、ファイルの中の名前の解決まで進める。
2. ファイルをまたぐ参照（dandori の `use rule … from "…"` や `flow "…"`、rulec の `import proto`、koyomi の `use calendar`、yurai と sakai の名指し）を、索引（6.4）で解決する。
3. 各言語の検査を、出す側から順に走らせる（rulec、koyomi、chobo、geas と `.proto` のあとに dandori、そのあとに yurai と sakai）。受け取る側は、出す側の結果を口から受け取る。
4. 言語をまたぐ検査（7 章）。

一つのファイルのエラーは、ほかのファイルの検査を止めない。受け取る側が、エラーのある出す側を参照していれば、その参照のところで、出す側の診断を名指す診断を出す（yurai の E203 にあたるものを、子プロセスの終了コードではなく型で）。

一回の実行の中では、同じファイルを二度読まない。実行をまたぐキャッシュは作らない（測ってから考える。15 章）。

### 6.2 名指しを処理系全体のものにする

`ritsu-base` の名指しは、yurai と sakai の DESIGN.md の 2 章で決め、二つのリポジトリの `tests/fixtures/naming.tsv`（36 行の試しの表）で確かめているものを、そのまま処理系全体の決まりにする。決まりは次のとおり。

1. **形**：`<ツール> "<パス>" [<種類> <名前>]...`。組はツールの構造どおりに入れ子にできる。入れ子にできるのは、proto の `service S [method M]`、`message M [field f]`、`enum E [value V]`（入れ子のメッセージは名前を `.` でつなぐ：`message Order.Line`）と、rulec の `enum E [value V]` だけで、ほかのツールの組は一つまで。子の種類は、親の種類のすぐあとにしか書けない。
2. **ツールの語**：`rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`proto`、`file`、`yurai`、`sakai`。`dir` はツールの語にしない（sakai の `.ctx` の構文の語にとどめる）。
3. **パス**：`.req` や `.ctx` の中では、書いたファイルのディレクトリからの相対。区切りは `/` で、`.` と `..` は字の上で畳む。絶対パスと空のパスはエラー。`"."` はルートを指す。末尾の `/` は取り除く。JSON では、ルート（`--root`、無ければ最初に渡したパスの上でいちばん近い `.git` のあるディレクトリ、それも無ければ渡したディレクトリ）からの相対で、ルートの外に出るパスはエラー。
4. **種類の語**：rulec は `input`・`output`・`enum`（下に `value`）・`table`・`clause`・`define`・`derive`・`machine`・`source`、koyomi は `input`・`date`・`claim`・`source`、chobo は `unit`・`account`・`transfer`、geas は `claim`、proto は `service`（下に `method`）・`message`（下に `field`）・`enum`（下に `value`）、yurai は `requirement`・`source`、sakai は `context`・`term`。dandori と `file` には無い（dandori には 6.3 で足す）。どの言語も、自分が使わない種類も名指しとして受け付ける。
5. **名前**：ツールの名前（JSON の `name`）。別名は使わない。語（空白、`"`、`#` を含まない一続きの文字。頭が数字でもよい）か `"…"` で書く。`"…"` の中のエスケープは `\"` と `\\` だけで、ほかはエラー。正規化せず、大文字と小文字を区別する。proto の名前は、そのファイルの package から見た名前。文字列の外の全角の空白、`"…"` で書いた種類やツールの語、名前の無い種類はエラー。
6. **同じ・含む**：同じは、ツールの語と、ルートからのパスと、組の並びが同じとき。ファイルは中のものを全部含み、親の組（proto の `service`・`message`・`enum`、rulec の `enum`）は子を全部含む。
7. **JSON の形**：`{"text": …, "tool": …, "path": …, "items": [[種類, 名前], …]}`（キーはこの順）。`text` は、パスをルートからの相対に直し、名前を語で書けるなら引用符なしで書いた形。空白を入れない詰めた書き方で、ASCII でない文字はそのまま出す。
8. **文の中の書き方**：診断などの文に書くファイルの場所（`<パス>:<行>:<列>`、写しのパス）は、走らせたディレクトリから、渡されたとおりに書く。文の中の名指しは、JSON と同じくルートからの相対で書く（読み直すと同じ名指しになり、`.req` や `.ctx` にそのまま貼れる）。
9. **JSON の中のファイルの場所**：診断の `file` なども、名指しと同じくルートからの相対にし、JSON の外側に `root`（走らせたディレクトリから見たルート）を添える。いまは yurai がルートからの相対、sakai が走らせたディレクトリからの相対で食い違っていて、土台で一つにするときにそろえる（4.2）。

この決まりを、処理系のどこでも使う一つの書き方にする。yurai と sakai の `.req` と `.ctx` の中、診断の文と JSON、LSP の「定義へ移る」、`ritsu check` の JSON の中のもの、のどれも同じ形で書き、読み直すと同じものを指す。ツールの語は九つのまま。`ritsu` はツールの語にしない（ritsu は言語ではない）。

### 6.3 種類の語を足す

名指しの決まり（6.2）では、dandori に種類の語が無かった（dandori が中のものを JSON で出していなかったため）。D の段階で、次を足す。

- dandori：`task`、`case`、`record`（下に `field`）、`enum`（下に `value`）、`input`、`output`。

`naming.tsv` の `dandori "order.flow" task reserve` の行（いまは `ERROR: dandori has no kinds yet`）は、JSON の行に変わる。入れ子の決まり（子の種類は親のすぐあと）は proto と rulec と同じにする。geas の `target` など、ほかの言語の種類を足すのは、使う側が要るとわかってからにする。

### 6.4 索引：中のものと参照

各言語は、`Items` と `References` の口（3.2）で、自分の中のものと参照を出す。`ritsu-project` は、それをプロジェクト全体の索引にする。

**中のもの**（`Items`）は、種類、名前、行の範囲、定義の文を持つ。定義の文は、yurai が端のハッシュを取る元で、何を定義の文にするかは各言語が自分の DESIGN.md に書く。案は次のとおり。

| 言語 | 種類 | 定義の文 |
|---|---|---|
| rulec | `table`、`clause`、`define`、`derive`、`input`、`output`、`enum`、`machine`、`source` | そのものの行を `rulec fmt` が書く形にしたもの（表なら見出しから最後の行まで） |
| koyomi | `date`、`claim`、`input`、`source` | `date … =` の塊の行（操作の行を含む）、条件の行 |
| chobo | `unit`、`account`、`transfer` | yurai の DESIGN 3.2 の形（`chobo api` の一つから名前とコードを除いたもの）を土台の JSON で |
| dandori | 6.3 の種類 | タスクや案件の宣言の塊の行 |
| geas | `claim` | 主張の塊の行 |
| proto | `service`、`method`、`message`、`field`、`enum`、`value` | yurai の DESIGN 3.4 の決まった形の文 |

yurai の端は、いまはファイル全体のもの（rulec、koyomi の日付、geas、dandori）がある（yurai の DESIGN 3.2）。定義の文が出れば、表や日付の関数やタスクの一つ一つが端になる（7.10）。端の中身が変わるので、yurai のテストと例の確かめた記録（`.req` のハッシュ）は D の段階で取り直す。

**参照**（`References`）は、参照のある行、先の名指し、参照の仕方を持つ。dandori の `use rule … from`（同梱、Lambda、Connect の URL、`local`）、`use proto|openapi|smithy`、`connect`、`implements`、子の `flow "…"`、rulec の `import proto`、`shape`、`source … file`、koyomi の `use calendar` と `source`、yurai と sakai の名指しを出す。sakai はこれで全部の言語の参照を行番号つきで確かめ（7.10）、yurai は `trace` と `affected` でたどる。

## 7. 言語をまたぐ検査

### 7.1 結果の三つと、検査の診断

言語をまたぐ検査は `ritsu-cross` に置き、口（3.2）だけを通す。どの検査も、結果は三つのどれかである（P5）。

- **成り立つことを示した**：何も出さない（`ritsu check` の要約の数に入る）。
- **成り立たない例がある**：エラー。その値と、そこに至るもの（フローの実行、規則の入力、koyomi の入力の日付）を添える。
- **決められない**：警告。理由を言い、実行時に確かめる手当てがあればそれを言う。

検査の診断は、ritsu の台帳（`crates/ritsu-cross/src/codes.rs`）のコードで出す。言語ごとの台帳とは番号を分け、`ritsu explain <コード>` で引く。どの言語の診断かは、テキストでは見出しの括弧に（`error[ritsu E201]`）、JSON では `tool` に書く（8.3）。

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
| X8 | yurai の端を一つずつ | リンクの端が、表、日付の関数、タスクの一つ一つになる | `Items`、`Claims` | yurai の `.req` | D |
| X9 | sakai の参照を全部の言語で | dandori を含む全部の言語の参照が、宣言した関係と公表された言語を通る | `References`、`Rules` の列挙 | sakai の `.ctx` | D |
| X10 | 名指しの解決 | どこに書いた名指しも、プロジェクトの中のものを指す | `Items` | 全部 | D |
| X11 | 同じ条の写し | 同じ法令の同じ条を、規則、カレンダー、要件が同じ本文で写している | 土台の出典 | rulec、koyomi、yurai | D |
| X12 | 一つの `.proto` の読み方 | 同じ `.proto` を、どの言語も同じに読む | `ritsu-proto` | rulec、dandori、sakai、yurai | C〜D |
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
- **(b) 規則の宣言で**：規則が入力の範囲を koyomi の関数の値の集合として宣言する（たとえば `支払日 : date  range from koyomi "支払条件.cal" date 支払日`。書き方は E で決める。★作者が決めること。rulec の構文を一つ足す）。rulec は、その入力の値をその集合に限って、完全性、重なり、当てはまらない行を確かめる。rulec がもう持っている、宣言した `constraint` で尋ねる組み合わせを絞る仕組み（rulec の §15.55、`RulecCert/Sieve.lean`）と同じ形で、日付の軸に集合を置く。証明書には、集合の出どころ（koyomi のファイルと関数と SHA-256）と集合を書き、Lean の再検査も集合の上で通す（11 章）。

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

「渡す日付が仮押さえを作った日である」ことを、いまの dandori は言えない。dandori には時刻を読む式が無く、日付はタスクの結果か入力としてしか入ってこないからである。そこで、その文を走らせた時刻を読む式（仮に `now`）を dandori に足すかを、E の段階で dandori の DESIGN に決める（★作者が決めること。dandori の構文を足す）。Temporal には、再生しても同じ時刻を返す `workflow.now()` がある。ほかのプラットフォームでの読み方は、そのとき調べて、作れないプラットフォームは dandori の P6 のとおり E050 にする。足さないあいだは、決まった長さの待ちだけで下限と上限を求め、ほかは「決められない」として、いまと同じく期限切れが起きうるものとして数える。

### 7.8 koyomi の関数と chobo の振替を、dandori から呼ぶ（X6）

dandori に、期日と帳簿を読む宣言を足す（E の段階。★作者が決めること。dandori の構文を足す）。書き方の案は、chobo の DESIGN 5 章のもの（`use book 在庫 from "在庫.book"`、タスクの呼び方 `book 在庫.引当.hold`、`case 押さえ : 引当 follows 在庫.引当`）と、それに合わせた `use dates 支払条件 from "支払条件.cal"`（呼び出しは規則と同じく `let d = 支払条件.支払日(受領日: …)`）である。

- koyomi の関数の呼び出しは、規則と同じくアクティビティ（各プラットフォームのタスク）にする。koyomi の関数は純関数だが、祝日の表が毎年変わるので、ワークフローのコードの中で計算すると、表を入れ替えたワーカーで再生が食い違う。規則を普通のアクティビティにした理由（dandori の DESIGN 4.2。判定の記録が履歴に残る、直しても再生が食い違わない）と同じである。
- 渡す日付が、関数の入力の範囲とカレンダーのデータの範囲に収まるかを確かめる（X6）。収まらない日付があればエラーで、その日付とそこに至る実行を添える。dandori の範囲は、いまは数にだけ書けるので、日付の範囲を求めて運ぶ仕組みを dandori に足す。
- chobo の振替の呼び出しは、タスクの呼び方の一つにする。断られた理由は、タスクの宣言したエラーとしてそのまま使える（chobo の DESIGN 5 章）。chobo の操作はキーで冪等なので、dandori の E030（キーの無いリトライ）にあたらない。
- どちらも、dandori のすべてのプラットフォーム（Temporal の TypeScript・Python・Go、Step Functions、Lambda durable functions、Argo、pydantic-graph）で作る。koyomi は TypeScript・Python・Go のコードを、chobo はその三つの言語のクライアントを生成するので、作れないプラットフォームは無い見込みである。作れないものが見つかれば、dandori の P6 のとおり E050 にする。生成、参照インタプリタの見え方、E040 と E050、ランナーと突き合わせのテスト、README、DESIGN の全部に載せる（作者の決まり）。

### 7.9 一つの参照インタプリタ（X7）

`ritsu run <file.flow> --scenario <file.json>` は、dandori の参照インタプリタで、規則の呼び出しを rulec の評価器で、期日の呼び出しを koyomi のインタプリタで、振替を chobo のインタプリタ（帳簿の状態を持つ）で計算しながら流す。ほかのタスクの結果は、いまと同じくシナリオに書いたものを使う。

いまの `dandori scenarios` は、規則の結果も選ぶ（どの分岐も通るように）。それはそのまま残す（プラットフォームとの突き合わせは、規則の結果を選べる方が分岐を全部通せる）。`ritsu run` は、入力から規則の結果を計算する、もう一つの流し方である。規則の入力の選び方には、rulec の `vectors`（境界から作った入力）を使える。

これは証明ではない。三つの言語の意味を一度に流せる参照で、一つの生成パッケージ（9.3）との突き合わせの基準にする。

### 7.10 yurai、sakai、名指し（X8〜X10）

- **yurai**：段階 C で止めていた「一式の読み込み」（yurai の PLAN の C.1〜C.9）を、子プロセスと JSON ではなく、`Rules`・`Dates`・`Books`・`Claims`・`Items` の口で作る。端は 6.4 の定義の文になり、表や日付の関数や主張の一つ一つを追える。借りた出典は土台の出典（4.6）から読み、E107 も同じ手続きで比べる。`affected` は geas の記録を geas の口で読む。
- **sakai**：止めていた C.1〜C.5 を、`References` と `Rules` の列挙（`connect.enums` にあたるもの）で作る。dandori の参照も読めるので、N101 は要らなくなる。sakai の DESIGN 4.7 が挙げていた四つの検査（規則の同梱が境界を越える、`connect` で呼ぶサービスが上流の公開ホストサービスでない、`implements` するサービスが自分の公表された言語に無い、子の `.flow` が境界の向こうのもの）を足す。子の `.flow` の扱い（sakai の DESIGN 4.7 の最後の段落）は、そのとき sakai の DESIGN に決める。
- **名指し**：プロジェクトのどこに書いた名指しも、索引のものを指すかを確かめる。いまは yurai と sakai がそれぞれ確かめている（yurai の E202 など）。言語ごとのコードと文はそのまま残し、引き方だけを索引に替える。子プロセスと JSON のためのコード（yurai の E203「ツールがファイルを読めない」と E204「ツールの JSON が知らない形」、sakai の E104「ツールが無い」と E105「ツールの api が失敗した」）は、出す側の検査のエラーを名指すもの（6.1）に意味を替えるか、退かせる。退かせるコードは台帳に退いたと書いて残し、番号を使い回さない（rulec の docs/compatibility.md と同じ決まり）。

### 7.11 同じ条の写し（X11）

規則（rulec）、カレンダー（koyomi）、要件（yurai）が同じ法令の同じ条を引くとき、写しの本文が同じかを確かめる。写しの場所の決まりは三つとも同じ（4.6）なので、同じプロジェクトの中では写しを一つにでき、別々に写したときは本文を比べる（e-Gov は改正の無い条でも XML の属性を書き換えることがあるので、バイト列ではなく本文で比べる。rulec の §15.71、yurai の DESIGN 3.3）。

### 7.12 一つの `.proto` の読み方（X12）

検査を足すのではなく、`ritsu-proto` に読み手を一つにすること（4.10）で、同じ `.proto` を rulec、dandori、sakai、yurai が違って読む余地を無くす。C の段階で sakai を、D の段階で rulec と dandori を移し、移す前と後で、それぞれのテストの結果が同じことを確かめる。

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
ritsu <言語> <引数>...        rulec・dandori・koyomi・chobo・geas・yurai・sakai のコマンドそのもの
ritsu --help | --version
```

- `ritsu check` は、パスを渡さなければ今いるディレクトリを読む。各言語の `check` を全部のファイルに走らせ、言語をまたぐ検査をし、最後に一行の要約（言語ごとのファイルの数、確かめた境目の数、決められなかった数）を出す。
- `ritsu explain` は、ritsu の台帳（言語をまたぐ検査のコード）を引く。各言語のコードは `ritsu <言語> explain <コード>` で引く。言語ごとにコードの番号が重なる（rulec の E101 と koyomi の E101 は別のもの）からである。
- `ritsu <言語> …` は、その言語のコマンドと同じものを、すべての口をつないで走らせる。

### 8.2 各言語のコマンドの残し方

- 名前は残す。`rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`yurai`、`sakai` は、リリースでは `ritsu` を指すリンクで、呼ばれた名前の言語として動く（2.3）。`ritsu rulec check …` と `rulec check …` は同じである。
- コマンド、フラグ、終了コード、診断、`--format json` と `api` の形は変えない（P6）。変わるのは、ほかの言語を同じプロセスの中で読むようになることだけである。dandori は rulec を子プロセスで走らせなくなり、`DANDORI_RULEC` は要らなくなる（D の段階で消し、README と dandori の DESIGN を直す）。yurai と sakai が子プロセスで呼ぶために予定していた `YURAI_RULEC`・`SAKAI_RULEC` などの環境変数は作らない。
- `--version` は `<名前> <ritsu のバージョン>` を一行で出す（`rulec 0.23.0` など。13.1）。`rulec --version` を読むスクリプトは、そのまま動く。
- 言語の環境変数（`RULEC_LANG` など）は残し、全部に効く `RITSU_LANG` を足す。

### 8.3 出力

`ritsu check` のテキストは、ファイルごとに、その言語の `check` が出すとおりの診断を出し、そのあとに言語をまたぐ検査の診断を出す。言語ごとに番号が重なるので、`ritsu check` の中でだけ、見出しの括弧にツールの語を足す（`error[rulec E101]`、`error[ritsu E201]`）。rulec の診断の枠（`-->` で場所を示す形）は、そのまま使う。

JSON は一つのオブジェクトにする。

```json
{"ritsu": "0.23.0", "root": ".", "files": [{"tool": "rulec", "file": "rules/送料.rule", "ok": true}], "diagnostics": [{"tool": "ritsu", "code": "E201", "severity": "error", "file": "flows/order.flow", "line": 12, "col": 3, "message": "…", "notes": [], "fix": null}], "crossings": {"proved": 14, "failed": 1, "undecided": 2}}
```

（形の案。キーは E の段階で決め、テストで固定する。）各言語の診断は、その言語の JSON の形のまま入れ、外側に `tool` と、ルートからの `file` を足す。

### 8.4 終了コード

0（問題なし、または警告と備考だけ）、1（どれかの言語か、言語をまたぐ検査にエラーがある）、2（使い方の誤り、読めないファイル、中の異常）。どの言語のコマンドとも同じ。

### 8.5 外のツールのための JSON

`rulec schema|certificate|api|graph|vectors`、`koyomi api|vectors`、`chobo api`、geas の `--json` と `map` の記録、`yurai api`、`sakai api` は、これまでどおり出す（P6）。ritsu の中ではこれらを読まない（口を通す）が、外のツールとエージェントが読む。プロジェクト全体の中のものと参照を一つの JSON で出すコマンド（`ritsu index`）は、外のツールが要るとわかってから足す（15 章）。

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

## 10. テストの組み立て

### 10.1 いまのテストの重さ

| | 件数と時間 | 外のもの |
|---|---|---|
| rulec | 727 件が飛ばし 0 で通った（0.22.1 のリリースのとき、2026-10-01 の記録）。GitHub の CI の test のジョブは 28 分（2026-09-25 の記録）。Kani は 118 のハーネスで 192 秒 | python3（NumPy、mypy、ruff、Connect の venv）、node、rustc、ruby（rbs と steep）、php、go、swiftc、JDK、protoc と buf、PostgreSQL、Lean。CI では Kani と wasmtime も |
| dandori | 全体で 6 分 20 秒、SKIP 0（2026-10-02 の記録）。大半は examples の 24 件で、同じ日の別の実行では 365 秒 | rulec 0.22.0、Node のツール（npm の六か所）、Python の venv、Go、Temporal の dev server、kind の上の Argo と argo CLI、LocalStack 4.14.0 の Docker イメージ、Chrome、Mermaid。任意で Ollama と TypeSafe の Jev（通信する） |
| koyomi | 99 件、SKIP 0、39〜52 秒（2026-10-03 の記録） | tsc、mypy、go、rustc、PostgreSQL、Chrome |
| chobo | 63 件、SKIP 0、約 1 分（2026-10-03 の記録） | PostgreSQL、TigerBeetle、Node と Python のランナー、Go、Chrome、Mermaid |
| geas | 236 件、SKIP 0、77 秒（2026-10-03 の記録） | pixie で作った greeter、Chrome、LLVM のツール、Go、Node、Python |
| yurai | 94 件、SKIP 0（2026-10-03 の記録） | Python の venv（prov と reqif）、ReqIF のスキーマ、xmllint |
| sakai | 97 件、SKIP 0、約 20 秒（2026-10-03 の記録） | import-linter、dependency-cruiser、Java と ArchUnit、Context Mapper、go-arch-lint、buf、rulec・koyomi・chobo・dandori のバイナリ |

全部を一度に回すと、dandori だけで負荷の平均が 60〜110 になり（2026-10-01 に dandori の全体を回したときの記録）、負荷の中でしか出ない揺れ（Argo のコントローラーが立ち上がり直す、Temporal の dev server が間に合わない）もある。毎回全部を回す形にはしない。

### 10.2 段

テストを三つの段に分ける。

| 段 | 走らせるもの | 目安 |
|---|---|---|
| `fast` | cargo のほかに何も要らないテスト。字句、構文、検査、診断の golden、変異、`explain` の再現、`naming.tsv`、api の JSON、外のツールを走らせない文書のテスト。git は使ってよい | ワークスペース全体で数分 |
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
| `tools` | main への push、コードを変えた pull request、毎晩 | ツールを入れ（rulec の `ci.yml` の一覧に、koyomi、chobo、geas、yurai、sakai のものを足す。PostgreSQL はサービスで）、クレートの組ごとに並べて `RITSU_TEST_LEVEL=tools` で回す。許す SKIP は無し |
| `proofs` | `proofs/` か、証明書とモデルにかかわるコードを変えたとき | `lake build`、コーパスの証明書の再検査、Lean のモデルとの突き合わせ（11 章） |
| `kani` | 毎晩と、rulec の生成器を変えたとき | rulec の CI の Kani の段（生成した Rust のハーネス） |
| `platforms` | 毎晩、手で始めたとき、`crates/dandori/` を変えた pull request | kind の上の Argo、LocalStack、Temporal の dev server を立てて、dandori の `platforms` の段を回す。ほかのジョブと並べない |
| `release` | タグ（F） | 13.2 |

### 10.6 揺れるテスト

dandori の重いテストには、原因を突き止めていない揺れがある（dandori の DESIGN 7 章の終わり）。`platforms` の段は、ほかの段と同時に走らせない。落ちたテストは一度だけ回し直してよいが、回し直したことを出力と報告に書く。二度続けて落ちたら、揺れではなく失敗として読む。

### 10.7 golden と取り直し

`RITSU_BLESS=1` で全部の golden を、`<名前>_BLESS=1` でそのクレートの golden を取り直す（いまの名前は残す）。取り直したら差分を読む。golden に入っているツールのバージョン（dandori の 57 のファイルの `rulec 0.22.0` など）は、バージョンが一つになれば、リリースのたびに一度取り直すだけになる。

### 10.8 テストの共通部分（`ritsu-testkit`）

1.2 の重なりを一つにする：自分を消す一時ディレクトリ（終わったプロセスの分も消す）、ツールを環境変数と既定の場所と PATH で探す（`RITSU_<ツール>` と、いまの `<クレート>_<ツール>` の名前の両方）、時間を区切って子プロセスを走らせる、golden と取り直し、SKIP、使い捨ての PostgreSQL のクラスタ（ソケットのパスは 103 バイトまで）、TigerBeetle のレプリカ、Chrome を探して時間を区切って走らせる（`--user-data-dir` は一時ディレクトリ）、Mermaid。std だけで書く。dandori の Argo、LocalStack、Temporal のランナーは dandori の `tools/` に残す。

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

`~/rulec`、`~/dandori`、`~/koyomi`、`~/chobo`、`~/geas`、`~/yurai`、`~/sakai` には、書かない、そこでビルドしない、git の状態を変えない。取り込みは、それぞれを作業場所に `git clone --no-local` で写してから、その写しの上で行う。元のリポジトリは、作者が公開の扱いを決めるまで、そのまま残す。

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

取り込んだあとの開発は ritsu で行う。元のリポジトリの扱い（GitHub で公開している rulec と dandori を、アーカイブにするか、ritsu を指す一文を足すか、リリースとサイトをどこから出すか）は作者が決める。それまで、rulec と dandori のサイト（GitHub Pages）とリリースは、元のリポジトリのものが残る。

### 12.4 B で気をつけること

取り込んだだけで、いまのテストの振る舞いが変わるところがある。どれも、中身を直さずに済ませる方法か、直し方を PLAN B に書いた。

- **git のルートが上に移る**：geas、yurai、sakai は、いちばん近い `.git` をルートにする。取り込むと、`crates/<名前>/` には `.git` が無く、ルートは `~/ritsu` になる。12.5 で試した結果、yurai のテストが二つ落ちる。
- **rulec の `.cargo/config.toml`**：`[env]` で `RULEC_LANG=ja` を強いている（rulec のテストは日本語の文を確かめる）。cargo は、走らせたディレクトリとその上の `.cargo/config.toml` だけを読むので、`~/ritsu` の根から `cargo test` すると読まれない。B では、各クレートのテストを `crates/<名前>/` で走らせる。
- **dandori が使う rulec のバージョン**：dandori の golden は rulec 0.22.0 で取ってある（57 のファイル）。ワークスペースで作る rulec は 0.22.1 なので、B では 0.22.0 の rulec（rulec のタグ `rulec/v0.22.0` から作業場所で作るか、リリースのバイナリ）を `DANDORI_RULEC` で渡す。
- **gitignore したものは来ない**：各クレートの `tools/` の `node_modules` と venv、Java の jar、TigerBeetle、`go-arch-lint`、ReqIF のスキーマ、rulec の `website/` が `sync.sh` で写すページ、dandori の `website/docs-ja/` の写し、rulec の `proofs/.lake` は、取り込んでも来ない。それぞれのクレートの手順で入れ直す（PLAN B.5）。
- **プロファイル**：ワークスペースの中のクレートの `[profile.*]` は読まれない。koyomi の `[profile.test] opt-level = 2`（1900〜2100 年のすべての日を何度も回すテストのため）は、根の `[profile.test.package.koyomi]` に移す。五つの `[profile.release] strip = true` は根の `[profile.release]` に一つ置く。
- **`Cargo.lock`**：根に一つ作る。五つの `Cargo.lock` の依存は同じバージョン（serde_json 1.0.151 ほか 15 のパッケージ）なので、通信せずに作れる。クレートの中の `Cargo.lock` は使われなくなる（C で消す）。

### 12.5 取り込んだ形で走らせてみたこと

2026-10-03 に、作業場所に `sim/.git`（空のディレクトリ）と `sim/crates/<名前>/`（元のリポジトリを写して `.git` を消したもの）を作り、B の形（ルートがクレートの二つ上にある形）で yurai、sakai、geas のテストを走らせた。外のツールは渡していない。

- **yurai**：92 件が通り、2 件が落ちた。SKIP の行は 3（prov と reqif の venv、ReqIF のスキーマが無いため）。コンパイルを含めて 7 秒ほど。
  - `tests/cli.rs` の `the_json_of_check`：`yurai check tests/fixtures/period --format json` の `root` が `"."` でなく `"../.."` になった。
  - `tests/design.rs` の `every_command_in_design_prints_what_design_shows`：DESIGN.md の `$ yurai check tests/mutants/E302_条が変わった`（`--root` なし）の出力の、写しのパスが変わった。

    ```
    --- DESIGN.md shows
      what changed in the text (the copy tests/mutants/E302_条が変わった/sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml):
    --- it prints
      what changed in the text (the copy ../../crates/yurai/tests/mutants/E302_条が変わった/sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml):
    ```

    パスはルートからの相対を走らせたディレクトリからの相対に直したもので、間違いではないが、回り道をしている（`../../crates/yurai/` は要らない）。git のリポジトリの下のディレクトリで yurai を走らせれば、取り込まなくても起きる。4.7 で直す。
  - 二つとも、`--root .` を渡せばルートはクレートのディレクトリになり、出力は元のものと同じになった。ただし E302 の直し方の行は、渡したとおり `--root .` を含めて書く（`yurai review tests/mutants/E302_条が変わった --root . --at …`）ので、DESIGN.md のその三行も合わせて直すことになる。
- **sakai**：97 件が全部通った。SKIP の行は 9（ツールが無いため）。コンパイルを含めて 12 秒ほど。ただし `tests/cli.rs` の `check_exit_codes_and_formats` は、テストのディレクトリに `.git` があるときだけ `--root` なしの形を確かめる作りで、B の形ではその部分を黙って飛ばす。
- **geas**：236 件が全部通った。SKIP の行は pixie の二種類が二回ずつ（`GEAS_PIXIE_GREETER` を渡していないため）。geas のテストは、例を一時ディレクトリに写して走らせるので、ルートが移っても変わらなかった。コンパイルを含めて 1 分 32 秒。
- rulec、dandori、koyomi、chobo は、`.git` からルートを決めない（`src/` を探して確かめた）。rulec の `@リビジョン` の読み方は、走らせたディレクトリからの相対（`git show <rev>:./<パス>`）なので、`crates/rulec/` の下でもそのまま読める。

## 13. バージョンとリリース

### 13.1 一つのバージョン

ワークスペースのバージョンを一つにし（`[workspace.package] version`）、どのクレートも `version.workspace = true` にする。どのコマンドの `--version` も、生成物の頭も、doc のページも、このバージョンを書く。バージョンは生成物の頭や golden に入っているので、変えるのは作者が番号を決めたあと、F の段階で一度にする（それまでは、各クレートのいまのバージョンのまま）。

バージョンの番号は rulec の続きにし、ritsu の最初のリリースを 0.23.0 にする案を勧める（★作者が決めること）。rulec だけがリリースを重ね（タグ 28）、Homebrew の formula を持ち、生成物の頭と、README が案内する CI の書き方（`uses: i2y/rulec@v0.22.1`）にバージョンが入っているからである。ritsu を 0.1.0 から始めると、`rulec --version` が 0.22.1 から 0.1.0 に戻る。言語ごとにバージョンを持ち続ける形は、バージョンを一つにすること（このまとめの目的の一つ）に反し、1.4 のバージョンの食い違いが残る。

### 13.2 配り方

- GitHub のリリース：rulec の `release.yml` を広げ、macOS（arm64、x64）と Linux（x64、arm64。musl で静的にリンク）の四つを作る。アーカイブには `ritsu` と、言語の名前のリンク七つを入れ、`SHA256SUMS` を添える。
- Homebrew：`i2y/tap/ritsu` を足す。いまの `i2y/tap/rulec` を、ritsu のアーカイブを入れて `rulec` だけを見せる formula にするか、そのまま残すかは作者が決める（★）。
- `.deb` と `.rpm`：rulec の `packaging/` を広げる。
- GitHub Actions：rulec の `action.yml` を広げ、`uses: i2y/ritsu@v0.23.0` で入れる。
- crates.io：いまは出さない。`koyomi` と `yurai` の名前は別のクレートが使っていて（2.2）、`ritsu` を crates.io に出すには、それが依存する中のクレートを全部出すことになる。出すなら、中のクレートを `ritsu-` で始まる名前にする。

リリースと push は、作者の指示があるときだけ行う。

## 14. 捨てた形

- **一つの言語にまとめる**：検査は、それぞれの狭さの上に立っている（rulec の自分の列への単項テスト、koyomi の一つの日付と有限の範囲、chobo の勘定の上限と下限、dandori の比較も計算もしない式、yurai のつながりとハッシュと期間、sakai の確かめられる部分）。混ぜれば崩れ、節で分ければファイルが一つになるだけである。
- **一つのファイルに言語ごとの節を並べる**：読む人と承認する人が言語ごとに違う。承認のページも、差分の読み方も、ファイルの単位で分かれている。
- **リポジトリを分けたまま、土台だけを公開するクレートにする**：診断や出典の重なりは消えるが、境目は JSON のまま残り、バージョンの食い違い（1.4）も残る。このまとめの目的（境目で証明を切らない）に届かない。
- **一つのリポジトリに入れ、プロセスの境目は残す**：同じ理由。境目の問い（7 章）が子プロセスと JSON の往復になる。
- **生成器の共通の中間表現**：一般のプログラムの表現になり、rulec の表の一行が一つの分岐になる読みやすさや、koyomi の操作ごとの関数が消える。共通にするのは表面にかかわる部分（9.2）まで。
- **`git subtree` で取り込む**：12.2。
- **Lean 4 で処理系を書く**：生成器、診断、CLI、テストの共通部分まで Lean で書く利点が無く、ビルドとツールの手間が増える。意味の中心部分だけを Lean のモデルにし、Rust と突き合わせる（11 章）。
- **言語のコマンドを `ritsu <言語>` だけにして、`rulec` などの名前を捨てる**：rulec は Homebrew と GitHub Actions で配られ、README とサイトがその名前で入れ方と CI の書き方を案内している。名前を残すのはリンク一つで済む。
- **土台を serde_json に依存させる**：4.9。
- **yurai と sakai が、ほかの言語のファイルを自分で読み解く**：二つの読み手が同じ言語の構文を持つことになる（sakai の DESIGN 4.7 の C と同じ理由）。口を通して読む。
- **言語ごとの診断のコードを一つの番号に振り直す**：rulec の診断のコードは、rulec の docs/compatibility.md が 1.0 から保つと書いているものである。番号は言語ごとのまま残し、`ritsu check` の中でだけツールの語を添える（8.3）。

## 15. まだやらないこと

どれも、作る理由が見えたら作る。ここに書くのは、黙って消えたように見せないためである。

- **サイト**：rulec と dandori のドキュメントのサイトとブラウザで試すページ（GitHub Pages）を ritsu から出すこと。出し方とアドレスは作者が決める。ritsu のブラウザで試すページ（F）は、まず ritsu の中に作る。
- **`ritsu index`**：プロジェクトの中のものと参照を一つの JSON で出すコマンド（8.5）。
- **`ritsu doc`**：プロジェクトのページ（各言語のページへのリンクと、言語をまたぐ検査の結果）。
- **rulec のすべての文を `Text` に移すこと**（4.1）。
- **実行をまたぐキャッシュ**（6.1）。
- **crates.io**（13.2）。
- **Windows**：geas の DESIGN 15 章と同じ理由（プロセスとサービスの止め方が Unix の振る舞いに立っている）。
- **スキルを一つにまとめること**：各言語のスキルは残し、ritsu のスキル（F）は、プロジェクトを `ritsu check` で回す流れと、どの言語のスキルを読むかを書く。
- **geas の DESIGN.md を日本語にすること**：geas の DESIGN.md は英語のままにする（geas を作ったときの決め）。
- **LSP の、診断、定義へ移る、型と範囲を見せる、中のものの一覧、rulec の整形、のほかの機能**。
