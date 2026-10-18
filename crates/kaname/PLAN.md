# kaname 実装計画

DESIGN.md を仕様として、kaname を八つの段階（B、C1、C2、C3、D、E、F、G）で作る。どの段階も、ここに書いた順に進め、各段階の最後にある完了の条件が全部成り立ったら終わりにする。DESIGN.md と違うことをしたくなったら、先に DESIGN.md に決めたことと理由と捨てたものを書き、報告で言う。

段階 A（設計）、段階 B（構文、名前、型、`check`）、段階 C1（参照インタプリタ、`run`、`vectors`、`examples`）、段階 C2（範囲の解析、例を探すこと、エフェクト、逃げ道の警告）、段階 C3（口と ritsu-cross、中のものと参照、`api`）、段階 D（生成と出力先の突き合わせ、`ritsu gen`）、段階 E（`doc`、例、文書、九つ目の言語）、段階 F（Lean、範囲の解析の六か所、W901）、段階 G（main の 0.31.0 への乗せ直しと、規則の列挙の入力の範囲）を終えた（2026-10-19）。B、C1、C2、C3、D、E、F、G で作ったものと確かめたことは B.9、C1.7、C2.6、C3.8、D.8、E.8、F.10、G.1、残したことの申し送りは 10 章にある。C を三つに分けたのは、範囲の解析と言語をまたぐ判定と参照インタプリタを一人で一続きにやり切るには大きすぎるからである。

| 段階 | すること | 主に読む DESIGN の章 |
|---|---|---|
| B | クレートと登録、字句と構文、名前と `use` の読み込み、型、診断と台帳、`ritsu kaname check` と `ritsu check` への組み込み | 2、3、11、12 |
| C1 | 参照インタプリタ、`examples`、`run`、`vectors`、丸めと日付のテスト、rulec のスーパーセットのテスト | 3.3、3.4、8、9、16.2、16.3 |
| C2 | 範囲の解析、kaname 自身の確かめと例を探すこと、エフェクト、逃げ道の警告 | 4、5、7 |
| C3 | 口 `Programs` と `Verdicts`、ritsu-cross の判定、`Items` と `References`、`api` | 6、14 |
| D | 生成と出力先ごとの突き合わせ、`ritsu gen` への組み込み | 10、16.4 |
| E | `doc` のページ、例、README、スキル、台帳の文書、九つ目の言語にすること | 11.2、11.3、13、17 |
| F | Lean、範囲の解析の六か所をモデルと同じ形に直すこと、W901 | 5、12、15 |
| G | main の 0.31.0 への乗せ直し、規則が順序のある列挙の入力に書いた範囲を口から読んで確かめること | 5.3、6.1、10.4、15 |

## 0. 全部の段階に共通の決まり

- git のコミットと push をしない。変更は作業ツリーに残し、段階の終わりにまとめて記録する。
- 依存の決まり（ritsu の DESIGN 3.1）：kaname のクレートは ritsu-base、ritsu-units、ritsu-ports、ritsu-emit にだけ依存する。serde_json も使わない（JSON は ritsu-base のもの。rulec、geas、sekisho と同じ）。rulec・koyomi・chobo は dev-dependency だけ。`cargo xtask deps` と `ritsu check ritsu.ctx` が通ること。
- テストは、書いているあいだは `cargo check` だけにし、項目を書き終えたら、その項目のテストを一回回す。触ったクレートのテストだけを回し、ワークスペース全体のテストは段階の終わりに一度だけ回す。`--test-threads` は 6 まで、`CARGO_BUILD_JOBS=6`。ログを `reports/kaname-<段階>-logs/` に残す。`-- --nocapture` で SKIP の行を読んでから報告する。
- golden は `KANAME_BLESS=1`（ritsu の側は `RITSU_BLESS=1`）で取り直し、差分を読んでから報告する。
- 例、テストの材料、golden は英語が先で、日本語は `<名前>.ja.kn` の版か、日本語の名前の材料として横に置く。日本語のものは消さない。
- 日本語（DESIGN、PLAN、`--lang ja` の診断、ページ、README.ja.md）は、日本語の例にかけた出力を読み直す。英語の概念語を漢字に直訳した造語をしない。誰がするのか分からない言い方（「直します」）をせず、英語の語順を写さない。文書に、時刻の言葉（朝、夜）や、だれが決めたか・頼んだかを書かない。
- 文書に載せる出力と数は、実際に走らせたものを貼る。案として書いたもの（DESIGN の 12 章と 17 章の kaname の出力）は、その出力を作った段階で実物に差し替える。
- 成果物に、手元の絶対パス、ユーザー名、マシン名を入れない。

## 1. ディレクトリ

```
crates/kaname/
  Cargo.toml  DESIGN.md  PLAN.md  README.md  README.ja.md                       (A、E)
  src/lib.rs  src/main.rs  src/cli.rs  src/run.rs                                (B)
  src/kw.rs  src/lex.rs  src/parse.rs  src/ast.rs  src/diag.rs                   (B)
  src/names.rs  src/ty.rs  src/types.rs  src/codes.rs  src/check.rs  src/ports.rs  src/suite.rs   (B、C3)
  src/num.rs（有理数、丸め）  src/date.rs  src/value.rs  src/interp.rs  src/vectors.rs  src/examples.rs (C1)
  src/ranges.rs  src/effects.rs  src/hatch.rs  src/witness.rs                    (C2)
  src/api.rs                                                                     (C3)
  src/gen/{mod,ex,plan,module,books,harness}.rs  src/gen/runtime/kaname.{ts,py,go}   (D)
  tools/package.json  tools/package-lock.json  tools/requirements.{in,txt}  tools/go/{go.mod,go.sum}   (D)
  src/doc/{mod,markdown,html}.rs                                                 (E)
  design/                                                                        (A。E で examples/ に移す)
  examples/  docs/reference.md  docs/codes.md  docs/codes.ja.md  skills/         (B の台帳、E)
  tests/  tests/common/  tests/golden/{codes,run,vectors,api}/  tests/inputs/  tests/rules/  tests/modules/   (各段階。golden は B から、modules は C2 から)
  tests/typescript.rs  tests/python.rs  tests/go.rs  tests/common/targets.rs      (D)
proofs/KanameCore/  proofs/KanameCore.lean                                       (F)
```

## 2. 段階 B：構文、名前、型、check

### B.1 クレートと登録

- `crates/kaname/Cargo.toml`（edition 2024、`version.workspace`、ritsu-base・ritsu-units・ritsu-ports・ritsu-emit、dev に ritsu-testkit・rulec・koyomi・chobo）、`Cargo.lock`。段階 A は、クレートの無い `crates/kaname` をワークスペースと地図から外すために、根の `Cargo.toml` に `exclude = ["crates/kaname"]` を、`ritsu.ctx` の `except` に `crates/kaname` を足した。B で二つとも外す（ワークスペースの `members` は `crates/*` なので、それだけで入る）。
- `ritsu-base` の `Tool::Kaname`（語 `kaname`、拡張子 `kn`、種類 `function`・`record`（下に `field`）・`enum`（下に `value`））。`Tool::ALL` には入れない（C3）。
- `ritsu-project`：`ORDER` の `.proto` のあと、dandori の前に kaname（`kind_of` で `.kn` を読む）。`Joined` に `kaname: Rc<kaname::ports::Engine>`。`Project::check` に kaname の枝（B では `Verdicts` 無しで呼ぶ。C3 で `check_with`）。`Cargo.toml` に kaname。説明の文の言語の数。
- `ritsu`：`main.rs` の言語の振り分けに `kaname`、`languages.rs` に `kaname()`（B では rulec・koyomi・chobo の口だけをつなぐ）。`cli.rs` の `LANGUAGES` にはまだ入れない（`ritsu kaname` は動くが `--help` に出さない。E で入れる。sekisho の段階 A の形）。言語の名前のリンク（`kaname` という名前の `ritsu`）も E。
- `xtask` の `LANGUAGES` に `kaname`（`WITH_SERDE` には入れない）。
- 地図：`contexts/programs.ctx`（`context Programs(kaname)`、`owns dir "../crates/kaname"`、`published language kaname`、`upstream Base conformist`）、`contexts/project.ctx` と `contexts/entry.ctx` に `upstream Programs conformist through kaname`、`ritsu.ctx` に `use context` と、`except` に `crates/kaname/design`・`crates/kaname/tests`。sakai は `except` の無いパスを E009 にするので、`crates/kaname/examples` は、`design/` を移す段階 E（E.2）で `crates/kaname/design` と入れ替える。
- `Tool` の網羅する `match`：`ritsu-wasm` の `playground.rs` の生成の `match` に `Tool::Kaname` を足す（ブラウザで試すページは kaname を扱わないので、`None` を返す側）。`ritsu` の `check.rs` の `ONE_FILE`（一ファイルずつ確かめる言語）に `Tool::Kaname`、`cli.rs` に `NOT_LISTED_YET`（`ritsu kaname` は動くが `--help` に出さない）。

### B.2 字句と構文

- `kw.rs` に一枚の表（DESIGN 2.2）。名前にできない 13 語。
- 字句：rulec と同じ数と単位と日付のリテラル、`×`・`÷`・`−`・`≦`・`≧` と全角の数字、文字列、コメント、字下げ（タブは E001）、括弧の中の改行。
- 構文：DESIGN 2.3 の EBNF の全部。`examples` の続きの行。関数の名前の別名と引数の丸括弧。
- 診断：E001〜E004、E111。

### B.3 名前と `use` の読み込み

- `use rule|dates|book` を口（`Rules::facts`、`Dates::facts`、`Books::facts`）で読む。読めなければ E201（その言語の言うことを注に。dandori の `said_notes` の形）。口がつながっていなければ（`joined()` が false）E202 で exit 2。
- `use module`：パスを読み、モジュールのグラフを作る。輪は E203。
- 名前の解決：型と値の名前空間、前後どちらへの参照も（モジュールの中の `enum`・`record`・`fn`）、`let` は上だけ。規則の列挙と出力のレコードと並びの要素、日付の関数、振替の型。別名（E005）、同じ名前（E006）、生成するコードとぶつかる名前（E007。ritsu-emit の表）、知らない名前（E101）、二通りに読める語（E110）。

### B.4 型

- ritsu-units で単位を読み（`Unit::parse`、`same`、`convert`）、rulec の演算の決まり（rulec の DESIGN §2.3）を書く。丸めているかの推論。`in` の換算（E104）。
- レコード、リスト、オプショナルな値、列挙、規則・日付・振替の型、`running[T, W]`。組み込みの関数の型（E109）、ラムダの位置。
- 渡す先の型（関数の引数と結果、フィールド、規則と日付の入力、振替の引数）。丸めていない値を渡せば E103。
- 宣言の確かめ：数と日付の引数とフィールドの `range`、リストの `count`、率の刻み、自分を含むレコード（E108）。`match` の網羅（E106）、呼び出しの引数（E107）、その型にない演算（E105）、振替（E204）。

### B.5 診断と台帳

- `codes.rs`：B で出るコードの全部（E001〜E007、E101〜E111、E201〜E204、E602）を、英語と日本語の文と、英語と日本語の再現で書く（ritsu-base の `ledger`）。E602（例の書き方と答えの型）は型の確かめなので B で出す。C1〜C3 で出すコードも、番号と見出しだけは B で載せる（再現は出すようになった段階で足し、それまでは `Repro::Later`）。
- `ritsu kaname explain <code> | --all [--format markdown|json]`。`docs/codes.md` と `codes.ja.md` は `explain --all --format markdown` の出力そのもの（`tests/codes.rs` が確かめる）。

### B.6 `check` と `ritsu check`

- `ritsu kaname check <file.kn>... [--format json] [--root] [--lang]`。最後の行は「ok <ファイル>: <n> functions」（エフェクトの内訳は C2 で足す）。
- `kaname::ports::Engine::checked_with(root, files, &Ports, lang)`（dandori の形）で、`ritsu check` に入れる。`ritsu check` のテキストからツール名を除いたものが `ritsu kaname check` の出力を並べたものと同じこと（`crates/ritsu/tests/check.rs` の決まり）。
- kaname のクレートのバイナリ（`src/main.rs`）：口を持たずに `run::run` を呼ぶ。

### B.7 テスト

- `crates/kaname/tests/`：`parse.rs`（EBNF の形と位置、`design/` の八つの `.kn` が構文として通ること）、`names.rs`、`types.rs`、`codes.rs`（B のコードの再現が英語と日本語で出る）、`cli.rs`（`check` と `explain` の終了コード、`--format json`）。
- `crates/ritsu/tests/kaname.rs`（新しい）：`ritsu kaname check` で `design/` の例が通る（内側のファイルをつなぐ）。`crates/ritsu/tests/check.rs` に kaname のファイルを含む小さなプロジェクト。`crates/ritsu/tests/map.rs`（`ritsu check ritsu.ctx` が通る）、`cargo xtask deps`。

### B.8 B の完了の条件

1. `design/` の八つの `.kn` が `ritsu kaname check` を通る（B で確かめる範囲：構文、名前、型）。直すところがあれば、DESIGN と下書きを一緒に直し、報告に書く。
2. B のコードの全部に、英語と日本語の再現があり、`tests/codes.rs` が通る。
3. `ritsu check` に `.kn` が入り、`crates/ritsu` の `check.rs`・`kaname.rs`・`map.rs` と `cargo xtask deps` が通る。
4. rulec・koyomi・chobo・dandori・sekisho・yuen・sakai の出力が一字も変わらない（`crates/ritsu/tests` の golden が取り直し無しで通る）。

### B.9 B で作ったものと確かめたこと（2026-10-18）

- 八つの `.kn` は、下書きを直さずに `ritsu kaname check` を通った。
- 台帳の 32 のコードのうち、B が出す 23 に英語と日本語の再現があり、どの再現もそのコードだけを出す（`tests/codes.rs`）。再現が出す文は `tests/golden/codes/` の 46 のファイルにある。
- 型の決まりで設計から変えたもの、書き足したものは DESIGN 20 章の 22〜29 にまとめた（刻みで決める丸め、リテラルの読み方、範囲を求めるところ、`running` の型、名前空間）。

## 3. 段階 C1：参照インタプリタ、run、vectors、examples

### C1.1 値と有理数

- `num.rs`：i128 の既約分数、DESIGN 5.6 の int64 の決まり（`overflow`）、0 で割る（`zero`）。

### C1.2 丸めと日付

- 丸めの五つ（DESIGN 3.3 の整数の手順）。`tests/rounding.rs`：rulec の `RoundTo::round_to` と、DESIGN 16.3 の組の全部で比べる。
- 日付（Hinnant）と日付の差。`tests/dates.rs`：ritsu-ports の `day_text` と koyomi の日の番号と、0001-01-01〜9999-12-31 で比べる。

### C1.3 参照インタプリタ

- `interp.rs`（DESIGN 8 章）：入口の確かめ（`input`）、評価の順、`and`・`or` の短絡、内側の層（`Rules::eval`、`Dates::eval`）、振替の値、失敗（種類、行、文、呼び出しの並び）、呼び出しの深さの上限。
- `tests/interp.rs`：本物の口（rulec・koyomi・chobo の `ports::Engine`）で、`design/` の例とテストのモジュール。

### C1.4 `examples`、`run`、`vectors`

- `examples`：`check` で流す（E601。例の書き方と答えの型の E602 は B で出している）。`design/` の例の `examples` が全部成り立つ。
- `ritsu kaname run <file.kn> <function> --input <json|-> [--trace] [--format json]`（DESIGN 11.1）。
- `ritsu kaname vectors`（DESIGN 9 章）：候補、組み合わせ、2,000 件の上限、形、`--trace`。`tests/vectors.rs`（golden）。

### C1.5 rulec のスーパーセットのテスト

- `tests/superset.rs`（DESIGN 16.2）：コーパスの `derive` と `define` の全部を、rulec の `Checked` の型の環境で kaname の式として読み、型と、rulec のベクタの全部の入力での値を比べる。比べた件数と飛ばした件数（理由ごと）を出し、飛ばした件数の上限を書く。行の終わりの出典の引用（`@<出典> <条>`）は式ではないので、外して読む（段階 B で、コーパスの右辺 143 を kaname の構文で読み、読めなかったのは引用のある三つだけだった）。
- 広げたところ（値での割り算、比べの組み合わせ、日付の差、`in`）で、丸めていない値を渡す先に渡すと E103 になること。

### C1.6 C1 の完了の条件

1. 丸めと日付のテストが、DESIGN 16.3 の全部の組で rulec と koyomi に一致する。
2. スーパーセットのテストが、コーパスの全部の式について、型と値の一致を言う（飛ばしたものは理由つきで数え、報告に書く）。
3. `design/` の例の `examples` が全部成り立ち、`run` と `vectors` の golden が英語と日本語の版で取れている。英語と日本語の版のベクタが、名前を別名で読み替えれば同じ答えになる。

### C1.7 C1 で作ったものと確かめたこと（2026-10-18）

- `src/num.rs`（int64 の決まりつきの算術、丸めの五つ、`allocate`・`mod`・`isqrt`）、`src/date.rs`（Hinnant の手順。字句の日付もこれを使う）、`src/value.rs`（値、人が読む書き方、受け渡す形）、`src/interp.rs`（参照インタプリタ）、`src/examples.rs`（E601）、`src/vectors.rs`、`cli.rs` の `run` と `vectors`。`types.rs` は、`let` が名前に付ける型を `Typed::bound` に残す（参照インタプリタが読み替えに使う）。
- 丸めは rulec の `round_to` と 360,600 通りで、日付は ritsu-ports・koyomi・rulec と 3,652,059 日で一致する。日付の差は、koyomi の `days` と 730 日で一致する。
- rulec のコーパスの `derive` と `define` の 143 行の全部が、rulec と同じ型と、rulec のベクタの 20,033 の入力で同じ値になる。飛ばした式は無い。
- 四つの例の `examples` が成り立ち、`run`（`--trace` のテキストと JSON）と `vectors` の golden が英語と日本語の版で取れている。英語と日本語の版のベクタ 50 件が、名前を別名で読み替えれば同じ答えになる。
- 規則のオプショナルな出力（`tests/rules/coupon.rule`。オプショナルな入力をそのまま出力に渡す規則）を、kaname が `none` と値として読む。
- 設計から変えたものと足したもの（DESIGN 20 章の 30〜42）：率の刻みを関数の答えとフィールドにも求める（E108）、`let` の型の自由（rulec の宣言と同じ）、呼び出しの深さの上限を 200 に、`isqrt` の負の値を `range` に、入れ子のオプショナルな値の `none` を一つに。

## 4. 段階 C2：範囲の解析、エフェクト、逃げ道の警告

### C2.1 範囲の解析

- `ranges.rs`（DESIGN 5 章）：抽象の値（区間と分母の上限、集合、オプショナル、リスト、レコード、振替）、演算、分岐で狭めること、二つの値の関係（推移をたどって増やす）、関数の呼び出し（一つずつ）と再帰（型の全体）、組み込みの関数、`running_totals` の関係、`reduce` の広げ方。
- `tests/ranges.rs`：ベクタで見た値が、どの場所でも抽象の値に入る（解析が上からの見積もりであること）。狭めることと関係の材料。

### C2.2 kaname 自身の確かめと、例を探すこと

- 渡す先（規則と日付の整数の入力、関数の引数と結果、フィールド、`isqrt` の引数（0 以上））：E301・W301。あふれ：E302・W302。0 で割る：E303・W303。範囲を書かなかった刻みのある率の引数は、0% から 100% として読む（DESIGN 3.9。参照インタプリタも、入口と関数を呼ぶところでそう確かめている）。
- `witness.rs`（DESIGN 5.7）：示せない場所があるときだけ、ベクタ（C1 の `vectors::choose`）と、引数をそのまま運ぶ場所の作った入力を流し、例を探す。ベクタは、規則と日付の入力の範囲の一つ外の値をもう持つ（DESIGN 9）。

### C2.3 エフェクト

- `effects.rs`（DESIGN 4 章）：失敗しうる場所の種類、呼び出しのグラフの輪、伝わり方、書いたエフェクトの確かめ（E401）。C3 までは、言語をまたぐ判定（前提、koyomi の日、振替の額、koyomi の日付の入力）を、どれも決められないものとして数える（`rule`・`date`・`range`）。
- `check` の最後の行にエフェクトの内訳（「ok late_payment.kn: 1 function, total」）、`--format json` に関数ごとのエフェクト。

### C2.4 逃げ道の警告

- `hatch.rs`（DESIGN 7 章）：二つの形と、警告にしないもの。`tests/hatch.rs` に、7 章の例の全部と、`design/` の四つの例（どれも警告が出ない）。
- 警告が多すぎる・少なすぎる形を見つけたら、DESIGN 7 章を直し、報告に書く。

### C2.5 C2 の完了の条件

1. `design/` の四つの例の関数が、C3 の答えを待つもの（言語をまたぐ判定）を除いて、DESIGN 17 章の言うとおりに示せる。たとえば `media_mail.kn` の二つの関数は total。
2. DESIGN 12 章の E301・E401 の案の材料が、例を添えてそのコードを出す。
3. 範囲の解析のテストで、見た値が抽象の値の外に出る場所が一つも無い。

### C2.6 C2 で作ったものと確かめたこと（2026-10-18）

- `src/ranges.rs`（抽象の値、演算、分岐と答えた呼び出しで狭めること、関係、関数と輪、組み込みの関数、失敗しうる場所、引数の定数倍の和、計算した値の境目、場所の診断）、`src/witness.rs`（例を探すこと）、`src/effects.rs`（輪、エフェクトの推論、E401、最後の行、JSON）、`src/hatch.rs`（W501）。`check.rs` は型の確かめのあとにこの三つをかけ、`Checked::analysis` と `Outcome::analysed` に残す。参照インタプリタに見張り（`Interp::watch`）と止まった式（`Failure::expr`）、ベクタに境目の入力と、入力の一か所を読み書きする関数（`vectors::reach` ほか）を足した。型では、`first` の答えのオプショナルな値を一つにした（DESIGN 3.6）。
- 例の下書きの八つは検査を通り、警告も出ない。kaname 自身が決める場所は全部示せる。言語をまたぐ判定の場所は答えを待つ場所で、`media_mail.kn` の二つの関数が total、ほかの三つは `fails range`（X4）、`fails date`（X6）、`fails rule`（X2）になる。どれも E401 にはしない（DESIGN 4.3）。
- DESIGN 12 章の E301 の材料（17.2 の分岐を外したもの）が例を添えて E301 を、E401 の材料（17.3 の要素の数を 0 からにしたもの）が W301 と E401 を出す。17.1 の手数料を引いた形の E401 は、振替の額の答え（X4）を受け取る C3 で出る。
- 範囲の解析のテスト（`tests/ranges.rs`）：下書きの八つとテストのモジュール（`tests/modules/` の六つ）の全部の関数で、ベクタと、宣言から決まった順で選んだ 200 件の入力（合わせて 12,196 回の評価）を流し、355 の式に来た 401,670 の値のうち、抽象の値の外に出たものは無い。
- 台帳の 32 のコードの全部に、英語と日本語の再現がある。`docs/codes.md` と `codes.ja.md` を取り直した。
- 計算した値の境目で、`design/late_payment` の英語と日本語の版のベクタに三件ずつ足した（`late >= 1` の前、上、後）。ほかの下書きには、計算した値の比べが無い。
- 型と参照インタプリタが、温度の定数をほかの単位で読むときに零点のずれを落としていた（`30℃` を `54℉` と読み、`80℉` を `30℃` より高いと答えた）。型（`konst_in`）、参照インタプリタ（`interp::convert`）、範囲の解析（`factor_of`）で、零点も読み替えるように直した（DESIGN 3.2。`tests/interp.rs`）。
- 設計から変えたものと足したもの（DESIGN 20 章の 43〜53）。

## 5. 段階 C3：口と ritsu-cross、Items と References、api

### C3.1 ritsu-ports

- `programs.rs`（新しい）：`Programs`、`ProgramCrossings`、`ProgramRuleCall`、`ProgramDateCall`、`ProgramTransfer`、`Seen`、`Verdicts`、`CallVerdict`（DESIGN 14.2、14.5）。`lib.rs` のモジュールと再輸出と説明。
- `rules.rs`：`RuleEnum` に `origin` と `ordered`（DESIGN 14.1）。rulec の `ports.rs` が `import enum … from` と列挙の宣言の区切り（`<`）から埋める。ほかの口の実装は既定の値のまま。kaname は `ordered` を読んで、規則の列挙の大小の比べを許す（いまは E105。DESIGN 3.5）。
- 同じく `RuleEnum` に、規則が書いた列挙の名前と別名（値の `public` にあたるもの）を足し、kaname の `names::rule_enum_is` が別名をパスカルケースに直して `RuleEnum::alias`（生成するコードの名前）と比べているところを、それと比べるように置き換える。

### C3.2 kaname の `Programs`

- `ports.rs` の `Engine` が `crossings` に答える：呼び出しの場所ごとの区間、出どころ、関係（入力の名前に読み替えたもの）、並びの長さの上限、見た値（示せない場所があるときだけ）。

### C3.3 ritsu-cross

- `src/programs.rs`（新しい）：判定の関数は `borders` の公開の関数と、`preconditions::decide`（いまはモジュールの中だけの関数なので、`pub(crate)` にする。振る舞いは変えない）を使う。DESIGN 14.3 の X2（関係、見た値での確かめ）、X3 の (a)（日の集合と kaname の区間）、X4（どこから来た額も、拒否の理由を組み立てる場所ごとに）、X6。境目の数。`ProgramVerdicts`（`Verdicts` の実装）。`lib.rs` の `check` から呼ぶ。dandori の呼び出しの答えが一字も変わらないこと（`crates/ritsu/tests/cross.rs` の golden が取り直し無しで通る）。
- `codes.rs`：E201〜W205 の見出しと文を、ワークフローと kaname の関数の両方を言うように広げ、kaname の呼び出しの注の文を足す。`docs/codes.md` と `codes.ja.md` を取り直す。英語の文の言い直しは、替える前と後の突き合わせで、dandori の出力が変わらないことを確かめる。
- `tests/programs.rs`（新しい。DESIGN 16.5）。

### C3.4 つなぐ

- `ritsu-project`：`Project::check_with(joined, verdicts, lang)`（kaname に `Verdicts` を渡す）。`ritsu` の `check.rs` と `languages.rs` が `ProgramVerdicts` を作って渡す。
- kaname：`Verdicts` の答えでエフェクトを決め直し（C2 の「決められないものとして数える」を外す）、実行時の確かめの場所を決める（D が読む）。

### C3.5 `Items` と `References`

- kaname の `Engine` が `Items`（関数、レコードとフィールド、列挙と値。定義の文は `ritsu_base::definition`）と `References`（`use` の先）に答える。`Joined` の索引に足す。
- `Tool::ALL` に `kaname`、`naming.tsv` に英語の行と日本語の名前の行と誤りの行、yuen と sakai のツール名を並べる文と golden。

### C3.6 `api`

- `ritsu kaname api <file.kn>`（DESIGN 11.1）。`tests/api.rs`（golden）。

### C3.7 C3 の完了の条件

1. `design/` の四つの例が、DESIGN 17 章の言うとおりのエフェクトになる（四つとも total）。`payout.kn` を手数料を引く形に変えた材料が ritsu の E203 を、`discount_split.kn` を `reduce` で書いた材料が W201 を出す。
2. `ritsu check` の境目の数に kaname の呼び出しが入り、`crates/ritsu/tests/cross.rs` に kaname の小さなプロジェクトの golden がある。
3. ritsu-cross の dandori の呼び出しの答えと、yuen・sakai の出力（ツール名の文を除く）が変わらない。

### C3.8 C3 で作ったものと確かめたこと（2026-10-18）

- ritsu-ports：`programs.rs`（`At`、`Ends`、`Given`、`ProgramRuleCall`、`ProgramDateCall`、`ProgramTransfer`、`ProgramCrossings`、`Seen`、`Programs`、`Crossing`、`CallVerdict`、`Verdicts`。DESIGN 14.2、14.5）。`RuleEnum` に `public`・`ordered`・`origin`（`Default` も）、`dates.rs` に `day_of`（`tests/ports.rs` が、1900 年から 2100 年の全部の日の往復と、日付でない文字列を確かめる）。`RuleEnum` を作るのは rulec の `codegen::rule_enums`（口の `facts` が使う）だけなので、埋めたのは rulec で、ほかの口の実装は変えていない（C3.1 の「`ports.rs` が埋める」は、`codegen.rs` になった）。
- kaname：`ports.rs` が `Programs`（`crossings`、`seen`）、`Items`、`References` に答える。範囲の解析は呼び出しの場所（`ranges::CallSite`）と渡す値の出どころ（`Src`）と振替の拒否の理由（`Refusal`）を残し、答え（`Suite::with_verdicts`）があれば場所に当ててからエフェクトを推論する。参照インタプリタは、`seen` のために渡した値を残す（`Interp::given`）。規則の列挙を名前か公開名で指し、同じ元の列挙を同じ型と読み、順序のある列挙を比べる。`api.rs` と `ritsu kaname api`（`tests/api.rs`、`tests/golden/api/` の八つ）、`tests/ports.rs`。E401 の台帳の文と `docs/codes*.md`。
- ritsu-cross：`programs.rs`（X2・X3 の (a)・X4・X6、見た値での確かめ、診断、境目の数、`ProgramVerdicts`）と `check_with`。`preconditions::decide` と `said` は `pub(crate)` にしただけで、振る舞いは変えていない。台帳の E201〜W205 の文を広げ、見出しを四つ変えた（DESIGN 14.4）。`docs/codes*.md`、`tests/programs.rs`。
- つなぐ：`ritsu-project` の `Project::check_with` と、`Joined` の索引の kaname の `Items`・`References`。`ritsu` の `check.rs` と `languages.rs` が一つの `ProgramVerdicts` を作って渡す。
- 中のものと参照：`Tool::ALL` に kaname（`naming.tsv` に JSON の行を六つ、誤りの行を三つ。53 行と 31 行）。sakai の `Kind::ModuleUse` と `READ`、地図の最後の行の言語の並び。yuen の `READ` と、`verified by` のモジュール、E011 の文と golden、スキルの台帳のコピー（`skills/yuen/codes.md`。`crates/yuen/skills/sync.sh` で作り直した）。
- `design/` の四つの例：八つの `.kn` のどの関数も total（`ritsu kaname check`）。言語をまたぐ判定の場所は全部示せる（モジュールごとに、payout は X4 の額が二つと拒否の理由が二つ、late_payment は X6 が一つ、discount_split は X2 が二つ、media_mail は無し）。材料では、手数料を引いた `payout` が ritsu の E203（1 セントの支払いで −48）と kaname の E401、そのうえ支払いを 100 セント以上にしたものが W203 と E401、`upto` を `reduce` で組み立てた `discount_split` が W201 を二つと E401、`before` と `upto` を入れ替えたものが E201（例つき）と E401 を出す。
- `crates/ritsu/tests/cross.rs` の `kaname_calls_across_the_borders` に、七つのプロジェクトの golden（英語と日本語で 14 個）。`ritsu check` の境目の数に kaname の呼び出しが入る（payout の下書きで 8 か所、late_payment で 2 か所、discount_split で 4 か所）。
- 替える前（C2 の木を 0.30.0 に乗せ直したもの）と後の `ritsu` を、dandori・sekisho・yuen・sakai の例と ritsu のテストのプロジェクトの `ritsu check`（英語、日本語、JSON）、各言語の `check`、dandori の生成したコード（Temporal と Step Functions）、`ritsu check ritsu.ctx`、`ritsu explain --all`（英語と日本語）、kaname の下書きの `ritsu check` で突き合わせた。180 回のうち、違ったのは 8 回で、台帳の文を広げたことによる `explain --all` の二つと、エフェクトが total になり境目を数えた下書きの三つ（英語と日本語）だけである。`crates/ritsu/tests/cross.rs` の dandori の golden は、取り直さずに通る。yuen の E011 の golden（ツール名の一覧）は取り直した。
- 設計から変えたものと足したもの（DESIGN 20 章の 54〜68）。

## 6. 段階 D：生成と突き合わせ

### D.1 補助のモジュール

- TypeScript・Python・Go の `glue/kaname.*`（DESIGN 10.1、10.2、10.4）：有理数、確かめつきの算術、丸め、日付、`KanameError`、入口の確かめ、`text`。どれも標準ライブラリだけ。

### D.2 モジュールの生成

- 型、レコード、列挙、関数（入口、中の関数、エフェクトの形）、本体、実行時の確かめ、内側の層の生成物の呼び出し（DESIGN 10.3〜10.5）。生成物の頭（ritsu の DESIGN 9.2）、`--lang` のコメント。

### D.3 振替の型と、当てる関数

- 帳簿ごとの `glue/books/<帳簿>.*` と `<帳簿>_apply.*`（DESIGN 10.6）。拒否されうる理由の型は C3 の答えから。

### D.4 `ritsu gen`

- `crates/ritsu/src/package.rs`：sekisho のブロックのあとに kaname のブロック（種類 `glue`）、`checked` に kaname、拡張子を並べた文、パッケージの説明の文と `doc.go` の文、`generated_here`。kaname のモジュールは `ProgramVerdicts` を渡して一度だけ検査し（`kaname_modules`）、三つの言語のパッケージに使う。
- `crates/ritsu/tests/projects/` に kaname を含む英語の小さなプロジェクトを足し、`tests/gen.rs` と `golden/gen/` で確かめる。

### D.5 出力先ごとの突き合わせ

- `tests/typescript.rs`、`python.rs`、`go.rs`（DESIGN 16.4）：テストの中でパッケージを作り（rulec・koyomi・chobo の生成の関数を dev-dependency で）、ベクタを流して参照インタプリタと比べる。`tsc --strict --erasableSyntaxOnly`、`mypy --strict`、gofmt と `go vet`。道具は ritsu-testkit で探し、無ければ SKIP。型の検査の道具をランナーとして置くなら（`tools/runner-ts` の `typescript` など）、ロックファイルと、CI の `tools.yml` で入れる手順を足す。

### D.6 生成パッケージの `units` のモジュール

- D は初めから `units` のモジュールに合わせる。生成パッケージの中では、お金と量の額は、パッケージの単位のモジュール（TypeScript の `units.ts`、Python の `<name>/units.py`、Go の `<module>/units`）が宣言する単位ごとの型で受け渡す（ritsu の DESIGN 9.3、rulec の §15.248、chobo の DESIGN 4.3）。kaname の生成物は、その型で規則の生成物と帳簿のクライアントを呼び、受け取り、規則と帳簿を呼ぶところで型を直さない（DESIGN 10.2）。

### D.7 D の完了の条件

1. `design/` の例の全部とテストのモジュールの全部のベクタが、三つの出力先で参照インタプリタと一致する（件数を報告に書く）。
2. 生成物が、三つの型の検査を通る。
3. `ritsu gen --check` が、kaname を含むプロジェクトで何も言わず、`.kn` を変えたら言う。

### D.8 D で作ったものと確かめたこと（2026-10-18）

- 生成（`src/gen/`）：`mod.rs`（`Target`、`package` が補助のモジュール、モジュール、振替の型と当てる関数、`glue/` のインデックスを書き、名前を出す単位を返す）、`ex.rs`（式と文の木と、三つの言語の書き方。Go は gofmt の二項演算の空白の決め方を写した）、`plan.rs`（関数の本体を、参照インタプリタと同じ順と同じ読み替えで書く。名前、型の対応、確かめの場所）、`module.rs`（モジュールのファイル）、`books.rs`（振替の型と当てる関数）、`harness.rs`（突き合わせのハーネスと、ベクタと宣言の外の入力の答え）。補助のモジュールは `src/gen/runtime/kaname.{ts,py,go}` を `include_str!` で読む。
- 参照インタプリタ：規則と日付の呼び出しの失敗の文から理由を外した（DESIGN 20 章の 69。理由はトレースに残る）。`tests/interp.rs`、`tests/vectors.rs`、DESIGN 9 章の例の行を合わせた。
- `api` の `generated` を生成物の名前に合わせた（Go の関数のパスカルケースと `Fn`）。`tests/golden/api/` の八つを取り直し、変わったのは Go の関数の名前の二つ（`Claim` が `ClaimFn`、`Weight_of` が `WeightOf`）と、その日本語の版である。
- テストのモジュールを三つ足した：`tests/modules/walks.kn`（rulec のコーパスの三つの規則と ritsu のテストの帳簿 `stock.book` をたどる。`allocate`、`mod`、規則の列挙、帳簿の `hold`・`post`・`void`）、`lib.kn` と `calls.kn`（ほかのモジュールの列挙とレコードと関数を読む）。三つとも、どの関数も total。
- 出力先の突き合わせ（DESIGN 16.4）：`tests/typescript.rs`、`python.rs`、`go.rs` と `tests/common/targets.rs`。17 のモジュールで、ベクタ 559 件と宣言の外の入力 271 件が、三つの出力先とも、英語と日本語の文で参照インタプリタと一致した（止まらないベクタ 1 件は流さない）。型の検査：`tsc --strict --erasableSyntaxOnly`（TypeScript 7.0.2）、`mypy --strict`（2.4.0）、gofmt と `go vet`（Go 1.26.0）。道具は `tools/` に置き（npm のロック、ハッシュつきの `requirements.txt`、`go/go.mod` と `go.sum`）、`.gitignore` に `tools/node_modules` と `tools/.venv`、CI の `tools.yml` に「the others」の組の `-p kaname` と、道具を入れる手順（dandori の組にも。ritsu のテストが使う）を足した。
- 生成した Python の深さ：kaname の呼び出しの一段が Python の一段で、200 段の呼び出しは再帰の上限 203 で答える（100 段は 103）。CPython の既定の 1,000 の中に収まるので、DESIGN 8 章の上限 200 は変えない（8 章の理由の文は、測った値に書き直した）。
- `ritsu gen`（D.4）：`package.rs` の kaname のブロック、`checked` と `kaname_modules`、`generated_here`、インデックスと説明と `doc.go` の文、`cli.rs` の `gen` の文。`crates/ritsu/tests/projects/settlement`（17.1 と 17.2 の英語の版と、それが読む規則、日付のファイル、帳簿）と、`tests/gen.rs` で確かめる四つのこと（パッケージの形の golden `golden/gen/settlement.txt` と三つの型の検査、`--check` がモジュールを変えたあとにそのファイルだけを言うこと、モジュールだけを渡せば読むファイルが無いと言うこと、`ritsu gen` の生成物がベクタ 23 件と宣言の外の入力 10 件で三つの出力先とも参照インタプリタと一致すること）。確かめる場所は、言語をまたぐ判定の答えを渡すと 0 か所、渡さないと 3 か所（payout の X4 の額が二つ、late_payment の X6 が一つ）。`stockroom.txt` の golden は変わらない。
- `ranges.rs` と `effects.rs` の公開の型と関数は変えていない。

## 7. 段階 E：doc、例、文書、九つ目の言語にする

### E.1 `doc`

- `ritsu kaname doc`（DESIGN 13 章）：Markdown と HTML、英語と日本語。規則と日付のページの埋め込み。`tests/doc.rs`（golden）。日本語は、日本語の例にかけた出力を読み直す。

### E.2 例

- `design/` を `examples/` に移し（`git mv`）、規則と日付のファイルに出典の `source` の行を足して、コピーを保存して固定する（DESIGN 17.6）。`tests/examples.rs`。`design/` は消し、DESIGN 17 章のパスを直し、案の出力を実物に差し替える。`ritsu.ctx` の `except` の `crates/kaname/design` を `crates/kaname/examples` に替え、kaname のテスト（`tests/common/mod.rs` の `DRAFTS`、`tests/parse.rs`、`tests/cli.rs`、`tests/types.rs`）と `crates/ritsu/tests/kaname.rs`・`check.rs` の `design/` のパスも替える。

### E.3 文書

- `docs/reference.md`（言語の全部、`check` が言うこと、`gen` が書くもの、コマンド、終了コード、JSON）、`docs/codes.md`・`codes.ja.md`。
- `README.md`（英語）と `README.ja.md`（日本語。英語の訳ではなく、一から書き起こす）。`tests/docs.rs` が抜粋とコマンドの出力を確かめる。
- `tests/design.rs`：DESIGN の `kn` の塊が構文として通ること。いま、7 章の二つの例（`fn claim(…)`）は省略の `…` を、2.6 の例は関数の見出しだけを書いているので、全部を書くか、確かめから外す印を付ける。

### E.4 スキル

- `skills/kaname/`：`SKILL.md` は手で書き、`reference.md` と `codes.md` は `crates/kaname/skills/sync.sh` が `docs/` からコピーする。`tests/skill.rs`。ritsu のバイナリのスキルの一覧（`crates/ritsu/src/skills.rs`）、`skills/ritsu/SKILL.md` の行、`skills/README*.md`、`website/docs/marketplace.json`。

### E.5 九つ目の言語にする

- 9 章の一覧のうち、「九つ目の言語にする」に印を付けたもの全部。`cli::LANGUAGES` に `kaname` を入れ、`ritsu --help` の言語の一覧、言語の名前のリンク、README の表と「コマンド」の節、リリースとパッケージの全部。看板の「eight」と「八つ」の書き換えは、`git grep -i -E 'eight|八つ'` で探し、取り込んだときの記録として書いた文は記録のまま残す（sekisho の段階 D3 と同じ）。

### E.6 根の文書への notes

- 根の DESIGN.md と PLAN.md に書きたいこと（DESIGN の 0 章の言語の数と図、3.1 の表、3.2 の口の表と `Programs`・`Verdicts`、6.1 の `ORDER` と拡張子、6.2 のツール名、7.1 の台帳の文、7.2 の表の「確かめる場所」に kaname の呼び出し、8.1 のコマンド、9.3 のパッケージの `glue/`、11 章の `KanameCore`、14・15 章）を、`reports/kaname-E-notes.md` にそのまま貼れる文で書く。

### E.7 E の完了の条件

1. `crates/ritsu/tests` の `readme.rs`・`links.rs`・`release.rs`・`entry.rs`・`skill.rs`・`npm.rs` が、`LANGUAGES` に kaname を入れた状態で通る。
2. 例の四つの英語と日本語の版が `examples/` にあり、出典のコピーが固定されていて、`tests/examples.rs` が通る。
3. README と `docs/` と スキルのテストが通る。

### E.8 E で作ったものと確かめたこと（2026-10-18）

- **Go の名前**（E の最初）：引数とローカルの名前を小文字で始まるキャメルケースに、中の関数を `_` とキャメルケースにし、ぶつかったときの直し方を決めた（DESIGN 10.3、20 章の 79 と 80）。`src/gen/plan.rs`（`go_camel`、`param_names`、`Locals::fresh`、`inner_name`）、`src/gen/module.rs`（Python のホストが呼ぶ関数は別名のキーワードで受け取り、中の関数に分けた名前で渡す）、補助の Python に `itoa`。テストのモジュール `tests/modules/names.kn` を足した。三つの出力先とも、英語と日本語で 903 件（18 のモジュール、ベクタ 615 件と宣言の外の入力 288 件）が参照インタプリタと一致した。`ritsu gen` の生成物は、三つの出力先とも 33 件（ベクタ 23 件と宣言の外の入力 10 件）が一致した。
- **`doc`**（E.1）：`src/doc/`（`mod.rs` がページの塊を組み、`markdown.rs` と `html.rs` が書く）と `cli.rs` の `doc`。Markdown は規則と日付のページを畳んだ節に、HTML はシートに置く（DESIGN 13 章、20 章の 86）。`tests/doc.rs`（八つのページの golden、中のページが rulec と koyomi の描くものと同じこと、HTML が外から何も読まないこと、Chrome で中のページが開くこと）と材料 `tests/doc/decisions.kn`。ritsu の `tests/kaname.rs` に、四つの例の英語と日本語の Markdown と、支払いの HTML の golden（`tests/golden/kaname-doc/`）。
- **例**（E.2）：`design/` を `examples/` に移した（`design/` は一度もコミットしていないので `mv`）。一行目のコメントを外し、日本語の版の説明の終わりを「<名前>.kn の日本語の版」にした。出典のコピーを `rules/sources/` に置いて固定し、行と式から引用する（DESIGN 17.6）。遅延の請求の期日を 29 日後に直した（20 章の 81）。Media Mail の規則を一つのオプショナルな出力にした（20 章の 82）。`tests/examples.rs`（四つ）。`ritsu.ctx` の `except`、`tests/common/mod.rs` の `EXAMPLES`、kaname と ritsu と ritsu-cross のテストのパス、`crates/ritsu/tests/projects/settlement` を合わせた。
- **文書**（E.3）：`docs/reference.md`、`README.md`、`README.ja.md`、`tests/docs.rs`、`tests/design.rs`。README のコマンドの出力の材料に `tests/docs/`（`dollars.kn`、`unnarrowed.kn`、`net.kn`）。出力を添えたコマンドは ritsu の `tests/kaname.rs` が走らせる。
- **スキル**（E.4）：根の `skills/kaname/`、`crates/kaname/skills/sync.sh` と `README.md`、`tests/skill.rs`、ritsu の `src/skills.rs`、`skills/ritsu/SKILL.md`、`skills/README*.md`、`website/docs/marketplace.json`。
- **九つ目の言語にする**（E.5）：9 章の表の「E（九つ）」の行の全部。`LANGUAGES` に kaname、`ritsu --help`、`ritsu check --help` の拡張子、根の README とサイトの頭のページ（英日）、リリースとパッケージ、npm の bin。ほかの言語の文書：sakai と yuen の DESIGN（ツール名の数と kaname の扱い。sakai の DESIGN 18 章、yuen の DESIGN 3.8）、sakai と yuen の README とリファレンス、sakai のキーワードと二つの注（20 章の 87）。
- **台帳の再現**：ritsu-cross の `src/codes.rs` の E201〜W205 の八つに kaname のモジュールを足し、E204 の文に kaname の場合を足して、`docs/codes*.md` を取り直した（DESIGN 14.4、20 章の 85）。ritsu-cross の `src/programs.rs` の英文の誤り（「can be include」）を直した。
- **ブラウザで試すページの `ritsu.wasm`**：`website/tools/make_wasm.sh` で作り直し、ritsu の `tests/playground.rs` を回した。
- 根の DESIGN.md と PLAN.md に当てる文は `reports/kaname-E-notes.md` に書いた（E.6）。根の README.md と README.ja.md は、`readme.rs` が九つの言語の表と kaname の塊を求めるので、この段階で直した。

## 8. 段階 F：Lean、範囲の解析の六か所、W901

- F.1 `proofs/KanameCore`：値、式、評価、失敗の種類（DESIGN 15.1）。`lakefile.toml` の `lean_lib` と既定のターゲット。
- F.2 丸めの定理（RulecCert の `roundsTo` の形と、整数の手順）。
- F.3 範囲の解析の健全性（演算、狭めること、関係、`running_totals`）と、規則の呼び出しが残す関係の健全性。
- F.4 失敗しない定理と、total の定理（DESIGN 15.2 の 3）。言語をまたぐ判定の答えを、解析のあとで当てること。
- F.5 RitsuCross に、kaname の呼び出しの関係の段の定理と、kaname の呼び出しでの X2 の判定（DESIGN 15.2 の 4）。
- F.6 `ritsu-model` の `kaname` の問い（`Main.lean`）と、`crates/ritsu-model/tests/kaname.rs`（DESIGN 15.3。言語をまたぐ判定の答えと、規則の呼び出しの関係も比べる）、`cross.rs` の kaname の呼び出しでの X2。モデルを一か所ずつわざと壊して、突き合わせが落ちることを確かめる（ritsu の DESIGN 11.3 と同じ）。
- F.7 `crates/ritsu-model/tests/proofs.rs` の、どの宣言も三つの公理のほかに立たないことの確かめに `KanameCore` を入れる。`sorry`・`axiom`・`native_decide` が無いこと。
- F.8 範囲の解析が上からの見積もりになっていない六か所を、モデルと同じ形に直す（DESIGN 15.3 の表）。それぞれに届くテストのモジュールを足し、直す前の形で外れが出ることを確かめてから直す。参照インタプリタの `mod`・`isqrt`・`allocate`・丸めは、数でない値を 0 と読まずに止める。`corners.kn` を `tests/modules/` に移す。W901 を足す（DESIGN 12 章）。
- F.9 完了の条件：`lake build` が通り、F.6 の突き合わせが全部のモジュールで一致し、壊したどの場所でも落ち、F.7 が通る。範囲の解析のテスト（`tests/ranges.rs`）と出力先の突き合わせが、足したモジュールを入れて通る。

### F.10 F で作ったものと確かめたこと（2026-10-19）

F.1〜F.9 を終えた。

- `proofs/KanameCore`（根の `KanameCore.lean` と 25 のファイル、15,627 行）：値、式、評価（`Eval.lean`）、区間と抽象の値と範囲の解析（`Iv.lean`、`Value.lean`、`Analysis.lean`。規則の呼び出しが残す関係の `callRels` と、答えを当てる `analyzeWith` を含む）、`ritsu-model` が読み書きする形（`Json.lean`）、DESIGN 15.2 の定理（`Round.lean`、`IvSound.lean`、`AbsSound.lean` から `Total.lean` まで、`Verdicts.lean`、`Cross.lean`）。`lakefile.toml` の `lean_lib` と既定のターゲット、`Main.lean` の `kaname` の問い、`proofs/README.md`。
- `proofs/RitsuCross` に、`Program.lean`（kaname の呼び出しでの X2 の判定 `kept` と `x2k`）と `ProgramSound.lean`（`kept_sound`、`x2k_holds`、`x2k_fails`、`x2k_undecided`）、`Json.lean` に `x2k` の行を足した。
- 突き合わせ（`crates/ritsu-model/tests/kaname.rs`）：28 のモジュール（例の八つ、`tests/modules/` の十一、例を書き換えた九つ）、691 の評価、195 の場所（そのうち答えを受けた場所 27）、837 の式、見た値 12,571 で、違いは無い。規則の呼び出しの関係も一致する。`cross.rs` の kaname の呼び出しでの X2 は、96 行とも一致する。
- `crates/ritsu-model/tests/proofs.rs` に `KanameCore` と、KanameCore と RitsuCross の定理を足した。六つのライブラリの 14,375 の宣言（そのうち定理 6,818）が、`propext`・`Classical.choice`・`Quot.sound` のほかに立たない。
- モデルを一か所ずつ壊すと、どれも突き合わせが落ちる。壊すと証明が通らなくなるので、`Main.lean` が `KanameCore.Json` と `RitsuCross.Json` だけを読む形で `ritsu-model` を組んで試し、終えてから元に戻した。

| 壊したところ | 落ちたもの |
|---|---|
| 1. 丸めの `half_up` が半分ちょうどを切り捨てる（`Num.lean` の `past`） | `corners` の `half` の評価、結果、場所、式の値 |
| 2. 引き算が `b - a` を返す（`Eval.lean`） | `inner`・`rec`・`ranges`・`corners`・`payout`（手数料を引いた額の二つ）の評価 |
| 3. `<=` が `<` として成り立つ（`Eval.lean`） | `rec`・`lib`・`corners`・`bounds` の評価 |
| 4. 区間の和の上の端が 1 大きい（`Iv.plus`） | `late_payment` の結果、場所、式の値 |
| 5. `<` で狭めるときに刻みを引かない（`narrowIv`） | `ranges` の場所と式の値 |
| 6. int64 の場所をいつも示す（`Place.shown`） | `calc`・`rec`・`corners` の場所とエフェクト |
| 7. 呼んだ関数の止まらないことをエフェクトに入れない（`callEffect`） | `corners` の `outer` のエフェクト |
| 8. 要素の数が分からない `reduce` を二回で打ち切る（`reduceAbs`） | `corners` の `climb` の結果、エフェクト、場所、式の値 |
| 9. 等しいと分かった二つの名前の差を 0 にしない（`subRel`） | `corners` の `same` の結果、場所、式の値 |
| 10. レコードのフィールドの関係を持たない（`recordEdges`。`running_totals`） | `discount_split`（英日、書き換えた `reduce` と入れ替え）の規則の呼び出しの関係、`ranges` の `shares` の結果、場所、式の値 |
| 11. `some x` の値を何でもありうるとする（`armStep`） | `places` の `or_zero` の結果、エフェクト、場所、式の値 |
| 12. 答えをいつも成り立つとする（`decideAcross`） | `payout`（手数料を引いた額の二つ）と `discount_split`（書き換えた `reduce`）のエフェクトと場所 |
| 13. 規則の呼び出しの関係を、一つの向きでしか探さない（`relPairs`） | `discount_split`（英日と入れ替え）の規則の呼び出しの関係 |
| 14. kaname の呼び出しの X2 で、`<=` の関係が `<` も言うとする（RitsuCross の `kept`） | `cross.rs` の 96 行のうち 20 行 |
| 15. 和の分母の上限に、要素の上限をとる（`sums`） | `bounds` の `thirds` のエフェクトと場所 |
| 16. リストの中の数を読み替えるところで、int64 を見ない（`ivDeep`） | `bounds` の `many` のエフェクトと場所 |
| 17. 丸めた上の端を、下の整数から丸め直す（`roundEnd`） | `bounds` の `hundredths` の場所と式の値 |
| 18. 単位を読み替えた比べで関係を覚える（`addFact`） | `bounds` の `kept` の規則の呼び出しの関係 |

- 範囲の解析の六か所：DESIGN 15.3 の表のとおり。`tests/modules/bounds.kn`（四つの関数）を足し、`tests/ranges.rs` で、実行の失敗の種類が関数のエフェクトにあることも確かめるようにした。直す前は、`thirds` の値が 47 回、`hundredths` の値が 45 回、抽象の値の外に出て、`many` の実行の 201 回が、エフェクトの言わない `overflow` で失敗した。Lean との突き合わせも、直す前は `bounds` で 12 か所が違った。直したあとは、どちらも外れが無い。
- 参照インタプリタの `mod`・`isqrt`・`allocate`・丸めは、数でない値を 0 と読まずに止める（DESIGN 8 章）。
- `corners.kn` を `crates/ritsu-model/tests/kaname/` から `tests/modules/` に移し、範囲の解析のテストと出力先の突き合わせに入れた。範囲の解析のテストは、`tests/modules/` を全部読む（`walks`、`calls`、`names` も足した）。出力先の突き合わせは 20 のモジュールで、三つの出力先とも、英語と日本語で 964 件（ベクタ 654 件と宣言の外の入力 310 件）が一致する。
- W901：`src/security.rs`、`src/check.rs`（全文を、構文の誤りがあっても調べる）、台帳（`src/codes.rs`）の英語と日本語の再現、`tests/golden/codes/W901.*`、`docs/codes.md` と `codes.ja.md`、`docs/reference.md` の節、スキル（`skills/kaname/SKILL.md` の説明と表、`codes.md` と `reference.md` の写し）、`tests/secrets.rs`、`tests/codes.rs` と `tests/cli.rs`（`explain --all`）のコードの数。
- ブラウザで試すページの `ritsu.wasm` を `website/tools/make_wasm.sh` で作り直し、ritsu の `tests/playground.rs` を回した（ページが開くプロジェクトに `.kn` は無く、答えは変わらない）。
- 根の DESIGN.md の 11 章と 16 章、根の PLAN.md の 7.21 に当てる文は `reports/kaname-F2-notes.md` に書いた。
- `running_totals` の重み：丸めていない重みを E103 にした（`src/types.rs` の `weight_fits`。DESIGN 3.6、20 章の 100）。`tests/types.rs` の `the_weights_of_running_totals_are_rounded` が、`1 / x` の重みのモジュールを英語と日本語の名前で、ほかに金額と率の積、値で割った率、分岐の一つだけが丸めていない重み、丸めた重みと率の重みを確かめる。台帳の E103 の文、`docs/codes*.md`、`docs/reference.md` とスキルのコピーを合わせた。いつも `0%` の率の重みで、生成したコードが 0/0 で落ちていたのを直した（`src/gen/plan.rs` の `rate_step`。10 章）。

## 9. 言語を足すときに触る共通のファイル

sekisho を足したときのコミット（言語を足したもの、八つ目の言語にしたもの、生成したコードのもの、Cedar の CLI のテスト、ブラウザで試すページ、サイトのページ）と、言語の一覧を見るテストから集めた。段階の欄は、kaname でそれを直す段階である。「九つ」は E.5 の書き換えで、言語の数や一覧を言う文を直すもの。

| ファイル | 直すこと | 段階 |
|---|---|---|
| `Cargo.toml`（ワークスペース）、`Cargo.lock` | 段階 A で足した `exclude` を外す、`Cargo.lock` | B |
| `crates/ritsu-base/src/naming.rs` | `Tool::Kaname`、拡張子、種類。`Tool::ALL` は C3 | B、C3 |
| `crates/ritsu-base/tests/fixtures/naming.tsv`、`tests/naming.rs` | kaname の行、誤りの理由と行の数 | C3 |
| `crates/ritsu-base/src/lib.rs`、`Cargo.toml` | 説明の文の言語の数 | E（九つ） |
| `crates/ritsu-ports/src/lib.rs`、`src/programs.rs`（新）、`src/rules.rs`、`src/dates.rs` | 口 `Programs`・`Verdicts`、`RuleEnum` の `public`・`ordered`・`origin`、`day_of` | C3 |
| `crates/rulec/src/codegen.rs`（`rule_enums`） | `RuleEnum` の `public`・`ordered`・`origin` を埋める | C3 |
| `crates/ritsu-project/Cargo.toml`、`src/project.rs`、`src/joined.rs`、`src/check.rs`、`src/lib.rs`、`tests/project.rs` | 依存、`ORDER`、`Joined`、`Project::check` と `check_with`、索引、説明の文 | B、C3、E（九つ） |
| `crates/ritsu-cross/src/programs.rs`（新）、`src/lib.rs`、`src/codes.rs`、`docs/codes.md`、`docs/codes.ja.md`、`tests/programs.rs`（新）、`tests/codes.rs` | kaname の呼び出しの判定、`ProgramVerdicts`、台帳の文 | C3 |
| `crates/sakai/src/refs.rs`、`src/suite.rs`、`src/check.rs`、`tests/naming.rs` | kaname の `use` を境界を越える参照として確かめる（`Kind::ModuleUse`）、`READ`、地図の最後の行の言語の並び、ツール名を並べる文の行の数 | C3 |
| `crates/sakai/src/kw.rs`、`src/owners.rs`、`src/parse.rs`、`tests/golden/W101_*`、`DESIGN.md`、`docs/reference.md`、`README*.md`、`skills/sakai/reference.md` | キーワードの `kaname` と `function`、W101 と E002 の注の `.kn`、ツール名の数と kaname の扱い（18 章） | E |
| `crates/yuen/DESIGN.md`、`docs/reference.md`、`README*.md`、`skills/yuen/reference.md` | ツール名の数と kaname の扱い（3.8）、リファレンスの表 | E |
| `crates/yuen/src/suite.rs`、`src/project.rs`、`src/codes.rs`、`docs/codes.md`、`docs/codes.ja.md`、`tests/names.rs`、`tests/golden/E011_*`、`skills/yuen/codes.md`（`crates/yuen/skills/sync.sh` が作るコピー） | `READ`、`verified by` にモジュール、E011 の文と golden | C3 |
| `crates/ritsu/Cargo.toml` | kaname | B |
| `crates/ritsu/src/main.rs`、`src/languages.rs` | `ritsu kaname` の振り分け | B（C3 で `Verdicts`） |
| `crates/ritsu/src/cli.rs` | `NOT_LISTED_YET` と `is_language`（`ritsu kaname` を動かし、`--help` には出さない）。`gen` の説明と拡張子の文（`.kn`、つなぐコード）。`LANGUAGES`、言語の説明、`check` の拡張子の文 | B（`NOT_LISTED_YET`）、D（`gen`）、E（九つ） |
| `crates/ritsu/src/check.rs` | `ONE_FILE` に kaname。`Verdicts` を渡す | B（`ONE_FILE`）、C3 |
| `crates/ritsu/src/package.rs` | kaname のブロック（足すだけ） | D |
| `crates/ritsu/src/skills.rs`、`src/lib.rs` | スキルの一覧、説明の文 | E |
| `crates/ritsu/tests/kaname.rs`（新） | `ritsu kaname` | B〜E |
| `crates/ritsu/tests/check.rs`、`cross.rs`、`codes.rs`、`gen.rs`、`golden/` | `ritsu check`・境目・台帳・`ritsu gen` | B、C3、D |
| `crates/ritsu/tests/entry.rs`、`links.rs`、`readme.rs`、`release.rs`、`skill.rs`、`npm.rs`、`common/mod.rs`（`CODE` に `kn`） | 言語の一覧を見るテスト。`common/mod.rs` の `reproduction_above` は、ページが見せる再現のファイルだけを置く | E（九つ） |
| `crates/ritsu-base/src/ledger.rs` | 台帳の再現の `.kn` を `kn` の囲みで書く | E |
| `crates/ritsu/tests/map.rs` | 地図が通る | B |
| `crates/xtask/src/main.rs` | `LANGUAGES` | B |
| `ritsu.ctx`、`contexts/programs.ctx`（新）、`contexts/project.ctx`、`contexts/entry.ctx` | 地図。`except` に `crates/kaname/design` と `crates/kaname/tests`。`design/` を移したら `crates/kaname/design` を `crates/kaname/examples` に替える | B、E |
| `crates/ritsu-model/Cargo.toml`、`tests/kaname.rs`（新）、`tests/cross.rs`、`tests/proofs.rs`、`proofs/KanameCore*`（新）、`proofs/lakefile.toml`、`proofs/Main.lean`、`proofs/RitsuCross.lean`、`proofs/RitsuCross/`（`Program.lean` と `ProgramSound.lean` は新）、`proofs/README.md` | Lean のモデルと定理、突き合わせ、公理の確かめ | F |
| `packaging/archive.sh`、`homebrew.sh`、`linux.sh`、`nfpm.yaml`、`smoke.sh`、`skills.sh` | リンクとスキルと説明 | E（九つ） |
| `packaging/npm/lib/cli.js`、`lib/index.js`、`package.json`、`README.md`、`test/cli.test.js` | 言語の一覧と説明 | E（九つ） |
| `.github/workflows/release.yml`、`packages.yml`、`tools.yml` | リンク、スキル、ランナー | E（九つ）、D（ランナー） |
| `action.yml`、`SECURITY.md`、`deny.toml`、`THIRD_PARTY_NOTICES` | 言語の数（言うものだけ） | E（九つ） |
| `README.md`、`README.ja.md`（根） | 表、コマンドの節のリンクの並び、`.kn` の塊 | E（九つ） |
| `skills/README.md`、`skills/README.ja.md`、`skills/ritsu/SKILL.md`、`skills/kaname/`（新）、`website/docs/marketplace.json` | スキル（F で `skills/kaname/` の `SKILL.md`、`codes.md`、`reference.md` に W901） | E、F |
| `website/docs/index.md`、`website/docs-ja/index.md` | 言語の数と一覧（kaname のページは作らない） | E（九つ） |
| `website/docs/node.md`、`website/docs-ja/node.md`、`website/rulec/docs/install.md`、`website/rulec/docs-ja/install.md`、`crates/rulec/skills/sync.sh` | 例を流す言語の数、リリースのリンクの数、スキルの数を言う文 | E（九つ） |
| `crates/*/README*.md`、`crates/*/skills/README.md` | 入れ方の段落の言語の数 | E（九つ） |
| `crates/ritsu-wasm/src/playground.rs` | 生成の `match` の `None` を返す側に `Tool::Kaname`（`Tool` に足した値を網羅するため）。ほかは直さない（ブラウザで試すページの `.kn` は、まだやらない。DESIGN 19 章）。`ritsu-project` を通して kaname がモジュールに入るので、`ritsu.wasm` は大きくなる | B |
| `website/docs/playground/ritsu.wasm` | ritsu のライブラリが変わったので、`website/tools/make_wasm.sh` で作り直す | E、F |
| 根の `DESIGN.md`、`PLAN.md` | notes に書き、段階の記録と一緒に当てる | E.6 |

## 10. 申し送り（G のあと）

- 範囲の解析の答えは `Checked::analysis`（`ranges::Analysis`）にある。関数ごとの `FnFacts` は、答えの抽象の値（`result`）、失敗しうる場所（`places`）、呼んだ関数（`calls`）、計算した値の境目（`boundaries`）、規則の呼び出しの関係（`relations`）、ほかの言語の呼び出し（`sites`。渡す値、出どころ、関係）、振替の拒否の理由（`refusals`）、エフェクト（`effect`）を持つ。言語をまたぐ判定の場所の結果は、答えを受け取った解析では `Shown`・`Example`・`Undecided` で、示せない場所は `Place::across` に ritsu のコードと見出しを持つ。答えを渡さない実行（kaname のクレートのテスト）では `Pending` のままである。規則の呼び出しが渡す入力どうしの関係は `CallSite::relations` にあり、Lean のモデルの `callRels` と突き合わせている。
- 生成は `src/gen/` にあり、書くのは `ritsu gen`（`crates/ritsu/src/package.rs`）だけである。`kaname::r#gen::package` がパッケージの `glue/` の全部と、名前を出す単位を返す。突き合わせのハーネスと答えは `kaname::r#gen::harness`（`program`、`cases`、`outside_cases`、`compare`）で、kaname のテストと ritsu の `tests/gen.rs` が使う。生成物を手元に残して読むには、`KANAME_KEEP=<ディレクトリ>` を付けて `tests/typescript.rs`・`python.rs`・`go.rs` を走らせる（一つのテストが走るたびに、そのディレクトリを作り直す）。道具は `npm ci --prefix crates/kaname/tools`、`uv venv --python 3.13 crates/kaname/tools/.venv` と `uv pip install --python crates/kaname/tools/.venv/bin/python -r crates/kaname/tools/requirements.txt` で入る。
- Lean：`proofs/` で `lake build`（既定の全部）、`cargo test -p ritsu-model`（`tools` の段）。モデルの関数を変えたら突き合わせを流し、壊して試すときは、`Main.lean` が `KanameCore.Json` と `RitsuCross.Json` だけを読む形で `lake build ritsu-model` を組む（壊したモデルでは証明が通らない。F.10）。範囲の解析を変えるときは、Lean の `Analysis.lean` を同じ形に直し、両方で `tests/ranges.rs` と突き合わせを流す。
- ★ 宣言した `running[T, W]`（関数の答えやフィールドの型）に `running_totals` の答えを渡すと、重みの型の刻みが宣言の刻みと違えば、刻みの上にあっても E102 になる。答えを `list[running[Line, money[USDc]]]` と宣言した関数 `twice` の本体を `running_totals(lines, l => l.amount * 2)` にすると、``An element of the answer of `twice` is running[Line, money[USDc]]; this is running[Line, money[USDc]]`` と言う（重みが `5USDc` でも、率の `l.share * 2` を `running[Line, rate[step 0.01%]]` に渡しても同じ）。型の検査が、レコードの型を重みの刻みまで形で比べるからである。比べを `num_fits` にするだけでは直らない。生成したコードは率の重みを重みの型の刻みの数で持つ（DESIGN 10.2）ので、宣言の刻みと違えば、ホストが読む刻みの数がずれる。直すなら、`running_totals` が渡す先の型の刻みでレコードを作る（型の検査が渡す先の型を `running_totals` の式に覚えさせ、`src/gen/plan.rs` の `running` がそれを読む）。
- 率の重みの `running_totals`（境目の形が刻みの数になるもの）は、出力先の突き合わせの材料（`tests/modules/` と例）に無い。段階 F の終わりに、`bounds.kn` に二つの関数（`rate[step 0.01%]` の重みと、いつも `0%` の重み）を一時的に足して三つの出力先を流し、`0%` の重みで TypeScript が `0/0 is no whole number` で落ちること、`rate_step` を直したあとは英語と日本語で 982 件が一致することを確かめた。足した関数は残していない。残すと、突き合わせと Lean の数（kaname の README、DESIGN 15.3・16.1・16.4、PLAN F.10、根の DESIGN 11.3）が変わるからである。材料に足すなら、それらの数を取り直す。
- 規則の列挙の入力の範囲：口の `ColumnType::Enum` の `range`（範囲に入る値の並び）を `names::column_values` で読み、`ranges.rs` の規則の呼び出しで `What::RuleEnum` の場所にする（DESIGN 6.1）。Lean のモデルでは `RuleInput.values`、`What.oneOf`、`enumPlace` で、rulec が受け付けない値は `RuleInput.enumOut`。どちらかを変えるときは、もう一方も同じ形に直して、`tests/ranges.rs` と `crates/ritsu-model/tests/kaname.rs` を流す。答えた呼び出しのあとに、渡した名前を範囲に狭めることはしていない（DESIGN 5.3、19 章）。入れるなら、`ranges.rs` の `assume` にあたる狭め方を列挙の集合でも書き、モデルの `ruleStep` と `ruleFold_sat` の証明に、値の集合で狭める仮定の健全性を足す。
- テストの書き方：口の答えは `tests/ports.rs`、`api` は `tests/api.rs` と golden、言語をまたぐ判定の答えの当て方は `tests/effects.rs`（作り物の `Verdicts`）、本物の判定は `crates/ritsu-cross/tests/programs.rs` と `crates/ritsu/tests/cross.rs`（`kaname_example` で例を書き換えた材料を作る）と `crates/ritsu/tests/kaname.rs`。生成したコードは `tests/typescript.rs`・`python.rs`・`go.rs`（答えを渡さない生成）と `crates/ritsu/tests/gen.rs`（`ritsu gen` の生成）。ページは `tests/doc.rs` と ritsu の `tests/kaname.rs`。W901 は `tests/secrets.rs`。Lean との突き合わせは `crates/ritsu-model/tests/kaname.rs`（モジュールと、例を書き換えた材料）と `cross.rs`（kaname の呼び出しでの X2）。

## 11. 段階 G：main の 0.31.0 への乗せ直しと、規則の列挙の入力の範囲

段階 F までの変更を、main の 0.31.0 と、規則の列挙の入力の範囲を口に通す直し（根の PLAN 7.20）の上に乗せ直し、kaname を口の新しい形に合わせる。主に読む DESIGN の章は 5.3、6.1、10.4、15。

### G.1 G で作ったものと確かめたこと（2026-10-19）

- **乗せ直し**：ぶつかったのは、根の DESIGN.md（3.2 の口の表）・PLAN.md（列挙の直しの 7.20 と、kaname の節の 7.21）、`packaging/npm/package.json`、`website/docs/marketplace.json` の四つで、どれも両方の変更を残した。両方が触ったファイル（ritsu-ports の `rules.rs`、rulec の `codegen.rs`、ritsu の `tests/cross.rs`、sekisho の README の英日、版の行を持つ文書）は自動で合わさり、`RuleEnum` の `public`・`ordered`・`origin` と、`ColumnType::Enum { name, range }` の両方が残っている。`Cargo.lock` は cargo が作り直し、kaname の版は 0.31.0 になった。手で書いた版の文字列（`docs/reference.md` の JSON の見本とスキルのコピー、DESIGN の頭と 13 章のページの頭）を、0.31.0 で出した実物に直した。
- **口の型**：`ColumnType::Enum` の名前だけを使う 5 か所（`names.rs` の `column_ty`、`gen/harness.rs` の `encode_col` と `col_go`、`gen/plan.rs` の `rule_type` と `rule_out`）を、`ColumnType::Enum { name, .. }` の形にした。
- **範囲の解析**：`names::column_values` が列挙の入力の範囲（範囲に入る値の並び）を読み、`ranges.rs` の規則の呼び出しが `What::RuleEnum` の場所を作る（鍵は数の入力と同じ `input:<名前>`）。示したかは、渡す値がとりうる列挙の値がどれも範囲に入るかで決め、例は `witness.rs` が関数のベクタで探す。E301・W301 の文と注、生成したコードが規則の呼び出しを包むかの判定（`gen/plan.rs`）、エフェクトは、数の範囲と同じところを通る。台帳の E301・W301 の「いつ出るか」と直し方、`docs/codes.md`・`codes.ja.md`（`explain --all` で書き直した）、`docs/reference.md`、スキル（`SKILL.md` の表と、コピーの二つ）を合わせた。
- **参照インタプリタ**：コードは変えていない。口の `eval` が範囲に入らない列挙の値を `RuleError::Input` で受け付けないので、`rule` で失敗する。kaname は `outputs_over` を使わないので、その直しは響かない。`doc` のページの golden に、範囲を書いた列挙の入力を持つ規則を読むモジュールは無く、ページは変わらない。
- **Lean**：`RuleInput.values`（`Json.lean` が `values` を読む）、`RuleInput.enumOut`（`refuses` に入れた）、`What.oneOf`、`enumWithin`、`enumPlace` を足し、`shown_sound`、`What.eq_of_beq`、`ruleStep_ok`、`ruleStep_conv`、`ruleStep_wit`、`refuses_inRange`、`rule_case` を合わせた。足した補題は `enumWithin_sound`、`grows_enumPlace`、`RuleInput.enumOut_true`。`ritsu-model` の `kaname.rs` は、規則の入力の `values` を書き、`What::RuleEnum` の鍵を読む。
- **試験の材料**：`tests/modules/lift.kn` と `lift.ja.kn`。rulec のコーパスの `member_tier_shipping.rule` と `会員段階の送料.rule` を読み、そのままの値（E301。例は `platinum`）、`match` で狭めた値、`if` で `platinum` を除いた値（どちらも示す）、規則の答えの値（W301）を渡す四つの関数である。`tests/ranges.rs` の `an_enum_input_of_a_rule_is_held_to_its_range`（英日。診断、場所の答え、エフェクト、参照インタプリタの失敗）、範囲の解析のテスト、出力先の突き合わせ（`tests/common/targets.rs`）、Lean との突き合わせ（`crates/ritsu-model/tests/kaname.rs`）に入れた。
- **CI**：`.github/workflows/proofs.yml` の `paths`（push と pull request の二か所）に、kaname の `src`・`examples`・`tests/modules`・`tests/rules` を足し、見出しのコメントに `KanameCore` を書いた。根の DESIGN 10.5 も合わせた。
- **ブラウザで試すページ**：`ritsu.wasm` を作り直した（15,370,730 バイト。根の DESIGN 8.7）。
- **テスト**（ログは `reports/kaname-G-logs/`）：`proofs/` で `lake build`（155 のジョブ。組み直した KanameCore のファイルに警告は無い）。`cargo test -p ritsu-model`（`tools` の段）は 15 件が通り、SKIP は 0。kaname の突き合わせは 30 のモジュール、723 の評価、216 の場所（そのうち答えを受けた場所 27）、903 の式、見た値 12,799 で違いが無く（`lift` と `lift.ja` は 4 つの関数と 10 の場所ずつ）、六つのライブラリの 14,431 の宣言（そのうち定理 6,850）が三つの公理のほかに立たない。`cargo xtask test --level tools -p kaname` は 147 件が通り、SKIP は 0。範囲の解析のテストは、361 の式の 489,656 の値、19,054 回の評価（失敗 1,450）で外れが 0、三つの出力先の突き合わせは、英語と日本語で 996 件（22 のモジュールのベクタ 686 件と宣言の外の入力 310 件）のすべてが一致した。そのあと直した文書を読む `docs`・`skill`・`design`・`ranges`・`codes` を回し直し、19 件が通った。ritsu は npm のパッケージを組んでから `tools` の段で全部を回し、172 件が通った（npm の 7 件と playground を含む）。SKIP は機械に MySQL が無い `the_checkout_records_the_sale` の 1 行だけで、xtask はそのため 1 で終わった。ritsu-ports・ritsu-cross・ritsu-project・yuen・sakai は `tools` の段で 494 件、dandori と sekisho は速い段で 237 件（SKIP の 49 行は段が求めたもの。文書と台帳とスキルのコピーを読むテストは走った）、rulec の `tests/ports.rs` は `tools` の段で 18 件が通った。`cargo xtask deps`（21 のクレート）、`ritsu check ritsu.ctx`、`cargo check --locked -p ritsu-wasm --target wasm32-unknown-unknown` が通り、`cargo clippy -p kaname -p ritsu-model --all-targets` は kaname に警告が無い（ritsu-model の三つと、ほかのクレートの警告は、前からのもの）。kaname の例の 20 のファイルは、ritsu 0.31.0 の `ritsu check` を通る。
