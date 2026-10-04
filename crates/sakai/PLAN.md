# sakai 実装計画

DESIGN.md が仕様で、この計画はそれを作る順序と、段階ごとの完了の条件を決める。段階は三つある。

- **B**：言語の芯（字句・構文・名前）、proto の読み手、地図の検査（属し方、パターンどうしの整合、対応の網羅、同じ語）、診断（英語と日本語）、CLI の `check`・`api`・`explain`
- **C**：一式の読み込み（rulec・koyomi・dandori・chobo。ritsu の D.8 で、ritsu の口で作った。C.1〜C.5）、コードの import の検査の設定の出力と、本物のツールでの突き合わせ、CML の出力
- **D**：`doc`、例の仕上げ、README.md と README.ja.md、エージェント向けのスキル（`skills/sakai`）

どの段階も、ここに書いた順に進め、各段階の最後にある完了の条件のテストが全部通ったら終わりにする。実装して DESIGN.md の決定が成り立たないと分かったら、黙って変えずに、DESIGN.md を理由と捨てた形ごと直し、報告で言う。0 章の約束（名指しの形、api の形、診断のコード、コマンドの表、環境変数、外のツールの版）を変えるときも同じで、変えたら、それを使う段階のテストも直す。

proto の読み込みは、一式の読み込みと一緒に C に置く分け方もあるが、この計画では proto の読み手を B に置いた。B の検査のうち、公表された言語（package、サービス）と対応の網羅（上流の列挙の値）と同じ語（越えてくる要素）は、proto を読まないと決められないからである。C に残したのは、一式の言語の読み込み（ritsu の D.8 から、ritsu の口で読む）と、dandori の扱いである。

この計画を書いた A の段階では、本体のコードは書いていない。リポジトリにあるのは DESIGN.md とこの PLAN.md（と、前からある LICENSE-MIT と LICENSE-APACHE）だけである。例のために書く規則、カレンダー、proto、ワークフローの下書きは、A の段階に作業場所で作り、一式のツールに通した（DESIGN 4.7、11 章）。その本文と、写す元と直し方を C.0 に書いた。外のツール（import-linter、dependency-cruiser、ArchUnit、go-arch-lint、depguard、Spring Modulith、Context Mapper）も A の段階に小さな例で走らせ、その結果を DESIGN の 0.4、7 章、8 章に貼った。

## 0. 全部の段階に共通の決まり

### 0.1 守ること

- 作者の決まり（段階ごとの指示書が挙げるメモ：`japanese-style`、`private-hobby`、`no-quoting-prompts`、`write-from-real-runs`、`features-are-first-class`、`do-the-whole-job`、`shell-gotchas`、`name-the-feature`、`user-profile`）を先に読み、従う。日本語（DESIGN.md、`--lang ja` の診断、doc の日本語のページ、README.ja.md、報告）は、英語の概念語を漢字に直訳しない。物に「たち」を付けない。カタカナ英語が普通の語はカタカナで書く。DESIGN.md の語にそろえる：境界づけられたコンテキスト、コンテキストマップ、公表された言語、公開ホストサービス、上流、下流、順応者、腐敗防止層、顧客、供給者、共有カーネル、パートナーシップ、別々の道、用語集、語、成果物、属する、持ち主、範囲、参照、境界を越える、対応、断る、名指す、まとまり（import を決める単位）。
- git のコミットと push をしない。`~/sakai` の外に書かない。rulec、dandori、koyomi、chobo、geas の木は読むだけで、そこでビルドも git もしない。
- 一式のツールを入れるときは `cargo install --locked --path ~/<名前> --root <作業場所> --target-dir <作業場所>/target` とし、`--target-dir` を必ず付ける。cargo 1.94.1 の `cargo help install` のとおり、`--path` で入れるときは、`--target-dir` が無いとそのクレートの木の `target/` でビルドする（ほかの木に書くことになる）。
- Rust は edition 2024 で、手元の stable 1.94.1 で通すこと。依存は `serde_json = { version = "1", features = ["preserve_order"] }` だけ。
- 診断は英語が既定で、`--lang ja` か `SAKAI_LANG=ja` で日本語。golden は両方の言語で取る。
- テストは `cargo test`。外のツール（buf、python と import-linter、node と dependency-cruiser、Java と ArchUnit、go と go-arch-lint、Context Mapper、Chrome、Mermaid）が無いときは、`SKIP: <理由>` を一行出して通す。報告の前に `cargo test -- --nocapture 2>&1 | grep SKIP` で SKIP の行を読み、数を報告に書く。
- golden の取り直しは `SAKAI_BLESS=1 cargo test`。取り直したら差分を読んでから報告する。
- 一時ファイルは、テストの一時ディレクトリ（`std::env::temp_dir()` の下の `sakai-test-<pid>-<番号>`）に置き、`Drop` で消す。テストのプロセスは最初に、終わったプロセスの残した `sakai-test-*` を消す（dandori で、誰も消さずに 6.8 GB たまったことがある）。sakai のテストはサーバーを立てない。Chrome を使うテストは、時間を区切って止め、`--user-data-dir` の一時ディレクトリを消す。
- 成果物（文書、golden、生成物、例）に、手元の絶対パス、ユーザー名、マシン名を入れない。ツールの出力を golden にするときは、一時ディレクトリのパスを `<tmp>` に置き換える。
- 文書に載せる出力と数は、実際に走らせたものを貼る。DESIGN.md のスケッチ（3.1 の要約、5.3 の診断、8 章の CML、9 章の api）は、その段階で実物に差し替える。
- 作者の勤め先と結びつく書き方をしない。例には公開されているもの（一式の例、rulec の corpus、内閣府の祝日の表）か、例のために書いたものを使う。

### 0.2 作者の機械で気をつけること（macOS arm64）

- macOS には `timeout` コマンドが無い。子プロセスの時間切れは、Rust の側で `Child::try_wait` を回して決め、超えたら kill する（koyomi の `tests/common/mod.rs` と同じ）。
- go のコマンドには `-trimpath` を付け、`GOCACHE`、`GOMODCACHE`、`GOPATH` を `tools/go/` の下（git に入れない）か一時ディレクトリに置く。`GOTOOLCHAIN=local`。go は 1.25.5。
- Python の venv は `uv venv --python 3.13 <場所>`。Homebrew の 3.14 の venv には pip が入らない。
- node は v23.11.0。dependency-cruiser の 17 と 18 は node 23 を対象にしていない（DESIGN 7.3）ので 16.10.4 を使う。TypeScript は 6 未満でないと dependency-cruiser 16.10.4 が `.ts` を読まない。7.0.2 を入れると、黙って 0 モジュールで通る。
- Java は OpenJDK 27 が入っているが、PATH には無い（場所は段階ごとの指示書にある）。テストは `SAKAI_JAVA` と `SAKAI_JAVAC`（無ければ `JAVA_HOME/bin`、それも無ければ PATH）で受け取るので、この機械でテストを回すときはその二つを付ける。クラスは `--release 21` で組む（ArchUnit 1.5.1 で確かめた形）。
- Chrome は `SAKAI_CHROME`、無ければ macOS が Google Chrome を入れる場所（`/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`）、それも無ければ PATH の `google-chrome` か `chromium`（dandori、koyomi、chobo と同じ順。OS の既定の場所は手元の機械に固有のパスではないので、リポジトリに書いてよい）。
- buf 1.54.0 と protoc 35.1 は PATH にある。
- 一式の言語：ritsu の D.8 から、テストは rulec、koyomi、chobo、dandori を `[dev-dependencies]` に持ち、同じプロセスでつなぐ。入れるツールは無い（前は `SAKAI_RULEC`、`SAKAI_KOYOMI`、`SAKAI_CHOBO`、`SAKAI_DANDORI` でバイナリを渡していた）。
- ディスクの空きは 48 GB ほど。ビルドの中間物や大きな一時ファイルを残さない（`tools/` の下の venv、node_modules、jar、go のキャッシュは git に入れず、終わったら消してよい）。

### 0.3 手本にしてよいもの（読むだけ）

- koyomi：`src/i18n.rs`（`tr!` が文の組を返す形）、`src/diag.rs`、`src/codes.rs`（台帳と再現）、`src/cli.rs`（コマンドとフラグの一枚の表）、`src/kw.rs`、`tests/common/mod.rs`（一時ディレクトリ、ツールの探し方、時間の上限）、`tests/mutants.rs`、`tests/design.rs`、`tests/docs.rs`、`tests/skill.rs`、`skills/sync.sh`。
- chobo：`tests/doc.rs`（Mermaid と Chrome での確かめ方）、`src/draw.rs`（SVG の HTML のページ）、`tests/common/runners.rs`（外のツールを走らせて結果を比べる形）。
- dandori：`src/proto.rs`（proto の読み手。名前の解決、import の探し方、ファイルが無くても知っているファイル）、`src/rulec.rs`（`rulec api` の読み方）、`src/doc.rs` と `src/draw.rs`。
- rulec：`src/sha256.rs`。

写すときは sakai の DESIGN に合わせて書き直す。

### 0.4 段階をまたぐ約束

#### 名指しの形（DESIGN 2 章）

yuen との決着（DESIGN 2.8）のとおり。`src/naming.rs` の `Name { tool: Tool, path: String, items: Vec<(String, String)> }`。`Tool` は `Rulec`、`Dandori`、`Koyomi`、`Chobo`、`Geas`、`Proto`、`File`、`Yuen`、`Sakai` の九つで、`dir` はツールの語にしない（`.ctx` の構文の語）。種類の語は DESIGN 2.2 の表（二つの言語の和）で、子の種類（`method`、`field`、`value`）は proto と rulec の親のすぐあとに一つだけ書ける。文字にすること（`Name::text`）、文字から読むこと（`naming::read` と `naming::parse`。`.ctx` の `means` と対応の先も、行の文字をこの関数に渡して読む）、JSON にすること（`{"text", "tool", "path", "items"}`、`Name::to_json`）の三つを一か所に置き、api、診断、doc が同じ関数を使う。パスはルート（`.git` を持つ一番近いディレクトリ、`--root`）からの相対で、`.` と `..` と末尾の `/` は字の上で畳み、ルートの外、絶対パス、空のパスは E012。診断の文面では、ファイルの場所（位置、関わるものの行、文の中のファイルのパス）を走らせたディレクトリから書き、名指しはルートからの相対のまま書く（DESIGN 2.4。`paths::Shown` と `paths::shown`）。`tests/fixtures/naming.tsv` は yuen と同じ表（36 行）で、`tests/naming.rs` が全行と、誤りの行が表の理由のとおりに断られることを確かめる。表を直すときは yuen の表も同じに直す。

#### コマンドの表（DESIGN 6 章）

B で `check`、`api`、`explain` と `--help`、`--version`、`--lang`、`--format json`（check）を作る。C で `build` と `export cml` を、D で `doc` を表に足す。まだ作っていないコマンドを表に載せない。

#### 診断のコード（DESIGN 5.2）

B で作るもの：E001〜E012、E101〜E103、E106、W101〜W103、E201〜E206（proto の import から読む参照のぶん）、E301〜E313、W301、E401〜E404、E406〜E410、W401、W402（E403 は proto の列挙と名前だけの先のぶん）。

C で作るもの：E104、E105、N101、E405、E501、E502、および rulec と koyomi から読む参照のぶんの E201〜E204・E403・E406・E407。E501 と E502 は C の段階で作った。ほかは ritsu の D.8 で、一式の読み込み（C.1〜C.5）を口で作ったときに作った。そのとき、E104 と E105 の意味を替え、N101 を退かせ、dandori の参照のぶんの E207、E208、E209 を足した（DESIGN 5.2、12.2）。

台帳は B で全部のコードを `src/codes.rs` に書く（C のコードも、見出しと説明と直し方は B で書き、再現と変異は C で足す）。

#### 環境変数

| 変数 | 意味 | 既定 |
|---|---|---|
| `SAKAI_LANG` | 文面の言語 | 英語 |
| `SAKAI_BLESS` | golden を書き直す（テスト） | |
| `SAKAI_BUF` | proto の読み手の比べ合わせ（テスト） | PATH の `buf` |
| `SAKAI_LINT_IMPORTS` | import-linter（テスト） | `tools/.venv/bin/lint-imports` |
| `SAKAI_DEPCRUISE` | dependency-cruiser（テスト） | `tools/node_modules/.bin/depcruise` |
| `SAKAI_JAVA`、`SAKAI_JAVAC` | Java（テスト） | `JAVA_HOME/bin`、PATH |
| `SAKAI_ARCHUNIT_LIB` | ArchUnit と JUnit の jar のディレクトリ（テスト） | `tools/java/lib` |
| `SAKAI_GO`、`SAKAI_GO_ARCH_LINT` | go と go-arch-lint（テスト） | PATH の `go`、`tools/go/bin/go-arch-lint` |
| `SAKAI_CML_LIB` | Context Mapper CLI の `lib/`（テスト） | `tools/cml/context-mapper-cli-6.12.0/lib` |
| `SAKAI_CHROME` | Chrome（テスト） | 0.2 |
| `SAKAI_MERMAID` | Mermaid を描く node のスクリプトのディレクトリ（テスト） | `tools/mermaid` |

#### 外のツールの版

| ツール | 版 | 入れ方 |
|---|---|---|
| import-linter | 2.15（grimp 3.17） | `tools/requirements.txt`、`uv venv --python 3.13 tools/.venv` と `uv pip install -r` |
| dependency-cruiser | 16.10.4 | `tools/package.json` と `tools/package-lock.json`、`npm ci --prefix tools` |
| TypeScript | 5.9.3 | 同じ |
| ArchUnit | 1.5.1（`archunit`、`archunit-junit5-api`、`archunit-junit5-engine`、`archunit-junit5-engine-api`） | `tools/java/fetch.sh`（Maven Central から取り、SHA-256 を確かめる） |
| JUnit Platform Console Standalone | 6.1.3 | 同じ |
| slf4j-api | 2.0.17 | 同じ |
| go-arch-lint | v1.19.0 | `tools/go/install.sh`（`GOBIN=tools/go/bin go install -trimpath github.com/fe3dback/go-arch-lint@v1.19.0`） |
| Context Mapper CLI | 6.12.0 | `tools/cml/fetch.sh`（`context-mapper-cli-6.12.0.zip` を取り、SHA-256 を確かめて開く） |
| Mermaid | chobo の `tools/mermaid` と同じ版 | `tools/mermaid/`（D） |

A の段階で取った jar と zip の SHA-256（`shasum -a 256`）：

```
a4dbfc51c90005ad6ac9967672a7efb43aa6928287a57238b28e60e0a75c5c6f  archunit-1.5.1.jar
dc93df23e0113a82ab913a63135febe8b563a38d7e26aae12c3c7475893eeb26  archunit-junit5-api-1.5.1.jar
a3a9db142ea31a7bfd124580839dd7ffc83b292f6b09e32d2025ac24be7bc04d  archunit-junit5-engine-1.5.1.jar
d8a4c1e51fecf593abcf1a52925a48525c9fa0e0d93d9018e588b1211341fdf3  archunit-junit5-engine-api-1.5.1.jar
e62b96ac475dbcde8599ea905d088f65d90778f86e259b856a49fa5c4ea256ec  junit-platform-console-standalone-6.1.3.jar
7b751d952061954d5abfed7181c1f645d336091b679891591d63329c622eb832  slf4j-api-2.0.17.jar
96579d57a5afa110d7b1363463cc576a2494cbbf480c37620aa09960c7779ce0  context-mapper-cli-6.12.0.zip
```

#### api の形（DESIGN 9 章）

キーはこの順に出す：`sakai`、`map`、`covers`、`except`、`contexts`、`relationships`、`artifacts`、`crossings`（ritsu の D.8 から、`crossings[].via` は参照の種類ごとの語。いつも空だった `not_checked` は ritsu の段階 E で消した。DESIGN 9 章）。名前は名指しの形の JSON。`relationships[].kind` は `upstream_downstream`、`shared_kernel`、`partnership`、`separate_ways`。役割の語は `conformist`、`anticorruption_layer`、`customer`、`supplier`、`open_host_service`、`published_language`。B で形を決め、`tests/golden/api/` に固定する。C と D は、キーを足すことはあっても、名前を変えない。

## 1. ディレクトリ

```
Cargo.toml  .gitignore  LICENSE-MIT  LICENSE-APACHE
src/
  main.rs  lib.rs
  run.rs          コマンドを関数にしたもの：run(引数, 口, 標準出力, 標準エラー)（ritsu の D.8）
  ports.rs        ritsu の口に答える（Items、References。ritsu の D.2）
  cli.rs          コマンドとフラグの表、--help、引数の読み取り（B。C と D が足す）
  i18n.rs         Lang と tr!（文の組を返す）（B）
  kw.rs           キーワードの表（DESIGN 1.2）（B）
  lex.rs  parse.rs  ast.rs                               （B）
  naming.rs       名指しの形（0.4）（B）
  resolve.rs      名前の解決：コンテキスト、別名、語、package、パス（B）
  elements.rs     地図が名指す要素（`means`、対応の列挙と先）を、読んだ proto で引く。短い書き方も（B）
  paths.rs        パスの正規化、範囲を歩く、既定で外す名前、知っている proto（B）
  owners.rs       属し方（いちばん深い項）（B）
  proto.rs        proto の読み手（B）
  model.rs        解決した地図（B）
  patterns.rs     パターンどうしの整合（B）
  refs.rs         境界を越える参照（B は proto の import。C で一式のぶんを足す）（B、C）
  mapping.rs      対応の網羅（B、C）
  terms.rs        同じ語（B、C）
  check.rs        段の順と要約（B）
  diag.rs  codes.rs                                      （B）
  api.rs                                                 （B）
  sha256.rs                                              （B）
  suite.rs        ritsu の口のまとまり、ほかの言語に問うこと、E104 と E105（C.1〜C.5。ritsu の D.8）
  build/          mod.rs（--check と頭の書き方）、areas.rs（DESIGN 7.1 の表）、import_linter.rs、depcruise.rs、archunit.rs、go_arch_lint.rs（C）
  cml.rs                                                 （C）
  doc/            mod.rs（ページの中身）、markdown.rs、html.rs、draw.rs（SVG）（D）
tests/
  common/mod.rs   一時ディレクトリ、ツールの探し方、時間の上限、golden、例の一覧（B、C、D）
  naming.rs syntax.rs resolve.rs owners.rs proto.rs patterns.rs refs.rs mapping.rs terms.rs codes.rs mutants.rs cli.rs api.rs design.rs  （B）
  imports.rs build.rs cml.rs examples.rs（C）。C.1〜C.5 のテストは examples.rs、api.rs、mapping.rs、codes.rs、mutants.rs に置いた（ritsu の D.8）
  doc.rs docs.rs skill.rs                                                                                                 （D）
  maps/           検査を通る地図（B、C）
  mutants/        <コード>_<内容>/ ごとに一つの地図の組（B、C）
  golden/         mutants の golden（<コード>_<内容>.en.txt、.ja.txt）、api/、imports/、cml/、doc/（B、C、D）
  code/           import の検査の変異：<言語>/<番号>_<内容>/ と 入れ子/<言語>/<番号>_<内容>/ に足すファイル（C）
tools/
  requirements.in  requirements.txt  package.json  package-lock.json                   （C）
  java/fetch.sh  go/install.sh  cml/fetch.sh  cml/Validate.java  README.md            （C）
  mermaid/                                                                             （D）
examples/通販/    地図、コンテキスト、成果物、四つの言語のコード（C で作り、D で仕上げる）
docs/             reference.md、targets.md、codes.md、codes.ja.md（D）
skills/           sakai/（SKILL.md と docs/ のページの写し）、sync.sh、README.md（D）
README.md  README.ja.md（D）
```

`.gitignore`：`/target/`、`/tools/.venv/`、`/tools/node_modules/`、`/tools/java/lib/`、`/tools/go/bin/`、`/tools/go/cache/`、`/tools/cml/context-mapper-cli-6.12.0/`、`/tools/cml/*.zip`、`/tools/mermaid/node_modules/`、`__pycache__/`、`.import_linter_cache/`。

## 2. 段階 B：言語の芯

字句・構文・名前、proto の読み手、地図の検査、診断（英語と日本語）、CLI の `check`・`api`・`explain`。一式のツールは使わない（rulec の規則が先の対応、koyomi の参照、dandori の note は C）。項の順に進め、各項のテストはその項のうちに書く。

### B.1 土台

- `Cargo.toml`：name `sakai`、version `0.1.0`、edition 2024、license `MIT OR Apache-2.0`、description（英語の一文。看板の案に合わせる）、repository `https://github.com/i2y/sakai`、依存は serde_json だけ。`[profile.release] strip = true`。lib と bin の両方を持つ。
- `src/i18n.rs`：`Lang { En, Ja }`。`--lang` があればそれ、無ければ `SAKAI_LANG`、無ければ英語。システムのロケールは見ない。`tr!("日本語", "English")` は `Text { ja, en }` を返し、描くときに `Lang` を渡す（プロセス全体の言語を持たない）。日本語の文で ASCII の名前と日本語のあいだに空白を入れる処理も、描くときに一か所でする（koyomi の 4.1 と同じ）。
- `src/kw.rs`：DESIGN 1.2 のキーワードを一枚で持つ（複数の語のキーワード `published language`、`open host service`、`anticorruption layer`、`shared kernel`、`separate ways`、`proto root`、`generated dir`、`use context` も）。名前として書けない語の判定と、`explain` と docs の一覧はここから引く。
- `src/sha256.rs`：SHA-256。FIPS 180-4 の既知の値でテストする（空、`abc`、448 ビットの文、`a` を 1,000,000 個。値は koyomi の PLAN B.5 にある）。

### B.2 字句と構文（`src/lex.rs`、`src/parse.rs`、`src/ast.rs`）

- 字句：名前（Unicode の文字・数字・`_`。数字で始まらない。地図、コンテキスト、語、下流の値の名前）、成果物の名前の中の名前（種類の語のあとの名前。DESIGN 2 章の決まりで、空白・`"`・`#` を含まない一続きの文字か、文字列）、別名 `(ascii)`（名前の直後に空白なしで）、文字列 `"…"`（`\"` と `\\`。改行を含まない）、版 `v<数字>`、package（ASCII の識別子を `.` でつないだもの）、`,`、`->`、`.`、`#` から行末までのコメント。どの字句も行と列を持つ（列は文字で数える）。
- 字下げはスペースで、ブロックごとにそろえる（タブは E005）。
- 構文は次のとおり（`[…]` は省ける、`…*` は 0 回以上、`…+` は 1 回以上）。

```
map のファイル:
  map <名前>(<別名>) v<n>
  [description "<文>"]
  (use context "<パス>")+
  covers "<パス>" (, "<パス>")*
  [except "<パス>" (, "<パス>")*]
  [proto root "<パス>" (, "<パス>")*]
  (code python|typescript|java|go "<パス>"          言語ごとに一度まで
     [  test "<パス>"])*                          java だけ。build --target archunit に要る

context のファイル:
  context <名前>(<別名>) v<n>
  [description "<文>"]
  [owner "<文>"]
  [also "<文>" (, "<文>")*]
  owns
    <項> (, <項>)*                                 1 行以上
  (published language <package>
     (proto "<パス>" (, "<パス>")* | rulec "<パス>")   1 行以上。proto と rulec は混ぜない。rulec は一つだけ
     [open host service <名前> (, <名前>)*]
     (generated dir "<パス>" (, "<パス>")*)*)*
  [terms
     (<語> ("<定義>" | as <コンテキスト>.<語>)
        (means <要素>)*
        [also "<名前>" (, "<名前>")*])+]
  (<関係>)*

関係:
  upstream <コンテキスト> <役割> [, <役割>]
    through <package> (, <package>)*
    (layer <項> (, <項>)*)*
    (enum <列挙> -> <先>
       (<値> -> <下流の値> | <値> -> refuse ["<理由>"])*)*
    (term <語> -> <語>)*
  downstream <コンテキスト> supplier
  shared kernel with <コンテキスト>
    <項> (, <項>)*                                 1 行以上
  partnership with <コンテキスト>
  separate ways from <コンテキスト>

役割:   conformist | anticorruption layer | customer
項:     (dir | rulec | dandori | koyomi | chobo | geas | proto | file) "<パス>"
        項の並び（`<項> (, <項>)*`）では、ツールの語は次のツールの語までのパスに効く：`dir "a", "b", rulec "c.rule"`
要素:   <ツール> "<パス>" (<種類> <名前>)+        長い形（DESIGN 2 章）
        | (message|enum|service) <名前> [(field|value|method) <名前>]   短い形（proto だけ）
先:     <要素> | enum <名前> | <名前>
```

- 語は、定義の文か `as` のどちらか一つを、語の名前と同じ行に持つ（両方、どちらも無いは E004。DESIGN 1.6 の例の書き方）。
- 字下げした行の順序は、節の中では問わない（`upstream` の下の `through`、`layer`、`enum`、`term` も、語の下の `means` と `also` も）。一度だけの行（`through`、`open host service`、語の `also`）が二度あれば E004。
- 節の順序と数は DESIGN 1.1 のとおり（違えば E004）。
- キーワードを名前の位置に書けば E002（「予約語なので名前にできない」と注を付ける）。
- E001〜E005 を出す。
- テスト（`tests/syntax.rs`）：DESIGN と、この計画の C.0 に出てくる `.ctx` の塊が全部構文を通る。E001〜E005 の変異（B.12）が、そのコードを出す。字句の位置（行と列。日本語の名前のあとの列を文字で数える）。

### B.3 名前の解決（`src/resolve.rs`、`src/model.rs`）

- 地図が読むコンテキストのファイルを読み、名前と別名の表を作る（E006、E008、E010）。別名の形は `[A-Za-z_][A-Za-z0-9_]*`。
- 関係の相手、`as` の語、`term` の語、`through` の package、要素を解決する（E007）。要素の短い書き方は、`means` では自分の `published language` の proto の中から、`enum` の対応の左辺では、上流の `through` の package の proto の中から探す。名前が二つの package に当たれば E007 で、候補を並べる（解決には B.5 の proto が要るので、B.5 のあとでこの部分を足してよい）。
- `model.rs` は、検査が使う形にまとめた地図：コンテキストごとに、属し方の項、公表された言語（package、ファイル、サービス、生成したコードの置き場所）、語、関係（相手、役割、`through`、`layer`、対応）。関係は相手の側からも引ける（下流の一覧、共有カーネルとパートナーシップの組）。
- テスト（`tests/resolve.rs`）：E006〜E008、E010 の変異。短い書き方が長い形に解決されること（`means message ReserveResponse` が `proto "proto/warehouse/v1/stock.proto" message ReserveResponse` になる）。

### B.4 範囲と属し方（`src/paths.rs`、`src/owners.rs`）

- パスを地図のディレクトリから正規化する（0.4）。`covers` の下を、パスの順に歩く。`except` の下と、パスのどこかに `.` で始まる名前、`node_modules`、`site-packages`、`__pycache__`、`target` があるものは歩かない（DESIGN 1.3）。シンボリックリンクはたどらない。
- 成果物とみなすファイル（DESIGN 1.3）：一式の拡張子、`.proto`（ただし `google/protobuf/`、`buf/validate/` の下と、`dandori/v1/options.proto` は除く。proto のルートからのパスで見る）、`code` の言語の拡張子で、その言語の置き場所の下にあるもの。
- 属し方：各ファイルについて、全部のコンテキストの `owns` の項のうち、そのファイルを含むもの（同じファイルの項か、上のディレクトリの項）を集め、いちばん深いものを選ぶ。ファイルの項はどのディレクトリの項より深い。同じ深さで二つのコンテキストなら E102、どれにも含まれなければ E101。E101 は、属さないファイルしか含まないディレクトリをいちばん上でまとめる（DESIGN 3.2）。
- 項のパスが無ければ E009、範囲の外なら E103、一つも成果物を含まなければ W101。
- ディレクトリを渡された `check` で、どの地図にも読まれない context のファイルがあれば W103。
- テスト（`tests/owners.rs`）：いちばん深い項が勝つ（リポジトリの根を持つコンテキストと、その中のディレクトリを持つコンテキスト）。同じ深さの二つは E102。外すもの（`.venv`、`node_modules`、`target`、`except`）。知っている proto を成果物にしないこと。E101 のまとめ方（属さないファイルが十二あるディレクトリが一つの診断になる）。

### B.5 proto の読み手（`src/proto.rs`）

- DESIGN 4.2 のとおり。`syntax`（proto2、proto3、`edition` は proto3 と同じに読む）、`package`、`import`（`public`、`weak`）と行番号、メッセージ（入れ子）、フィールド（型の名前、番号、ラベル、`json_name`、`map`、`oneof` の中）、列挙と値（番号と行）、サービスとメソッド（入出力の型、ストリーム）、サービスとメソッドのオプションの名前と値の文字列。`extend`、`reserved`、`extensions`、ほかのオプションは読み飛ばす。`group`（proto2）は E106 で読まない、と言う。
- 型の名前の解決は protobuf の決まり（内側の入れ子から外へ、package の段を一つずつ外して探し、`.` で始まる名前は完全な名前）。import したファイルの型も見る（`public` の import は推移的に）。
- import の探し方（DESIGN 4.2）：地図の `proto root` から順に、無ければ dandori と同じ二つの場所。知っているファイル（well-known types、`buf/validate/validate.proto`、`dandori/v1/options.proto`）は読まずに知っているものとして扱う。見つからない import は W102。
- 列挙の 0 番の値が「値が無いこと」を表すかの判定（DESIGN 1.7）：列挙の名前から作る接頭辞（`PackingStatus` → `PACKING_STATUS_`。大文字の区切りは buf と同じく、前が小文字か後ろが小文字の大文字で切る。`HTTPMethod` は `HTTP_METHOD_`）を外すと `unspecified`（大文字と小文字を問わない）になる名前。接頭辞の無い名前（`unspecified`）もそうみなす。
- テスト（`tests/proto.rs`）：
  - buf があれば、C.0 の `order.proto`、`stock.proto`、`fulfillment.proto` と、dandori の例の `warehouse.proto` を写した fixture を `buf build <ルート> --path … -o -#format=json` の結果と比べる（package、import、メッセージとフィールドの名前と番号と型、列挙と値、サービスとメソッド）。`buf/validate` を import する proto は、buf が BSR の依存なしに組めないので比べない（DESIGN 12 章）。buf が無ければ SKIP。
  - `shipment.proto`（`buf/validate` を import する）が読め、W102 を出さないこと。
  - 0 番の値の判定：`ORDER_STATUS_UNSPECIFIED`、`STOCK_UNSPECIFIED`、dandori の例の `unspecified` はどれも「値が無い」。rulec の corpus の契約の `HANDLING_STANDARD = 0` は本当の値。
  - 名前の解決：入れ子のメッセージ、package の段を外して探す名前、`.` で始まる名前、import した package の型。

### B.6 公表された言語とパターン（`src/patterns.rs`）

- DESIGN 1.4 と 1.5 の検査：E301（公開ホストサービスが proto に無い）、E302（proto や生成したコードの置き場所がそのコンテキストのものでない、package が見出しと違う）、E303（顧客／供給者が片側だけ）、E304、E305、E306、E307（共有カーネルが片側だけか、並びが違う。並びは、正規化したパスの集まりで比べる。写しを両側に置くときは、パスでなく中身の SHA-256 の集まりで比べる）、E308（写しの中身が違う）、E309、E310、E311、E312、E313、W301（上流の向きの循環。循環を一つずつ、名前の順に言う）。
- rulec の塊（`published language rulec.<…>` の `rulec "…"`）の package とサービスの突き合わせ（E302、E301）は、`rulec api` が要るので C.2 で足す。B では、rulec の塊は構文と属し方だけを確かめる。
- テスト（`tests/patterns.rs`）：各コードの変異（B.12）。両側が書いた共有カーネルが同じなら通ること、片側だけなら E307、写しの中身が一字違えば E308。

### B.7 proto の import から読む参照（`src/refs.rs`）

- 参照：proto のファイルの `import` ごとに、もとのファイル、行、先のファイル、もとのファイルの中で先のファイルの型を使っているところ（フィールドとメソッドの型。どの要素か）。使っているメッセージからフィールドでたどれる型も、越えてくる要素に数える（DESIGN 3.3）。
- もとと先が別のコンテキストに属するとき、DESIGN 3.3 の決まりで確かめる：共有カーネルの中なら通す。先が相手の公表された言語のファイルで、関係（1.5 の表）が許せば通す。そうでなければ E201〜E206。E204 は、腐敗防止層に `layer` があり、もとのファイルが層の中に無いとき。E205 は、腐敗防止層の下流の公表された言語のファイルが、上流の型を使っているとき。
- 参照の連なり（診断の「その参照」）：もとの成果物（属するコンテキストと、それを決めた `owns` の行）、参照の行、先の成果物（同じく）、許すはずの関係（あれば）。
- テスト（`tests/refs.rs`）：B.12 の `基本` の参照の一覧（下に書く三つ）。各コードの変異。

### B.8 対応の網羅（`src/mapping.rs`）

- DESIGN 1.7 の検査のうち、proto の列挙が先のものと、名前だけの先のもの：E401（無い値を全部、proto の順に名指す）、E402、E403（下流の proto の列挙に無い値）、E404（腐敗防止層で、下流の成果物が参照している上流の列挙に対応が無い。参照は B.7 のもの）、W402。対応を書けるのは腐敗防止層だけ（E304、E305 は B.6）。
- rulec の規則が先の対応（値の行が無くてよい形と、E405）は C.2。
- テスト（`tests/mapping.rs`）：上流の proto に値を一つ足すと E401 がその値を名指す。0 番の「値が無い」を書くと W402。0 番が本当の値の列挙では、0 番も対応に要る。

### B.9 同じ語（`src/terms.rs`）

- DESIGN 1.6 と 3.6：越えてくる要素は、B.7 の参照のうち要素まで分かるもの（proto の型）。上流の語の `means` の要素が越えてくる要素の中にあり、下流に同じ名前（`also` を含む）の語があって `as` で取り入れていなければ、対応がその要素を読み替えているかを見る。無ければ E406（役割ごとに直し方の注を変える）、読み替えた先の名前が下流のその語と同じなら E407。
- E408（`means` が自分の公表された言語に無い）、E409（`term` の語が用語集に無い）、E410（`as` の語が無い、または関係が無い）、W401（境界を越えない語。境界を越えるの定義は DESIGN 1.6）。
- 共有カーネルの成果物を通る参照は対象にしない。
- テスト（`tests/terms.rs`）：B.12 の `基本` の「キャンセル」は、請求の対応で読み替えられているので通ること。対応を消すと E406、読み替えた先の値を `キャンセル` にすると E407。順応者の側の同じ語は E406 で、注が「腐敗防止層にする」を含むこと。

### B.10 診断と台帳（`src/diag.rs`、`src/codes.rs`）

- `Diag`：code、severity（error、warning、note）、file、line、col（どちらも無いことがある）、message（`Text`）、notes（`Text` の並び）、references（参照の連なり）、fix（`.ctx` に貼れる行）。文面と JSON は DESIGN 5.1 の形（koyomi の `src/diag.rs` を手本に）。位置の行と列が無い診断は、`<ファイル>: …` と書き、原文の抜粋を出さない。
- `src/codes.rs`：DESIGN 5.2 の全コード。各コードに、見出し、いつ出るか、どう直すか（英語と日本語）、最小の再現（ファイルの組：`[(パス, 本文)]`）。C のコードの再現は、C の段階で足す（B では再現を空にし、`tests/codes.rs` は C のコードを「C で足す」として飛ばす）。
- `sakai explain <コード>`（再現のファイルも出す）、`sakai explain --all`、`--format markdown`。
- テスト：`tests/codes.rs`（B のコードの再現を一時ディレクトリに書いて走らせ、そのコードが出る。台帳のどのコードにも変異が一つ以上ある。台帳の並びが DESIGN 5.2 の表と同じ）、`tests/mutants.rs`（`tests/mutants/<コード>_<内容>/` ごとに、`check` の英語と日本語の文面が `tests/golden/<コード>_<内容>.en.txt`・`.ja.txt` と一致する）。
- 台帳の再現は、甲と乙の二つのコンテキストの地図（`codes::BASE`）を土台にし、各コードは置き換えるファイルだけを持つ（`Entry::reproduction`）。
- 変異のディレクトリは、土台の地図の名前を書いたファイル `base`（`基本` か `パターン`）と、変えたファイル（ファイルの全体）を持ち、ファイルを取り除くときは `remove` に並べる。テストは土台を一時ディレクトリに写し、変異のファイルを重ねてから、そのディレクトリをルートにして `check` する（パスは一時ディレクトリからの相対になり、golden に手元のパスが入らない）。

### B.11 CLI、check、api、explain（`src/cli.rs`、`src/main.rs`、`src/check.rs`、`src/api.rs`、`src/naming.rs`）

- DESIGN 6 章：コマンドとフラグを一枚の表に置き、`sakai --help`、`sakai <cmd> --help`（`sakai help <cmd>` も同じ）、`sakai --version` を出す。exit code は 0・1・2。知らないフラグ、閉じた集合の外の値、値の無いフラグ、二度書いたフラグは exit 2。引数なしの `sakai` は使い方を標準エラーに出して exit 2。
- `check`：DESIGN 3.1 の段の順。通ったときの要約の一行（DESIGN 3.1。数は実物）。`--format json`（DESIGN 5.1）。ディレクトリを渡すと、その下の map のファイルを全部（パスの順に）。
- `api`：DESIGN 9 章と 0.4 の形。検査を通らない地図には出さない（exit 1）。
- テスト：`tests/cli.rs`（exit code、全コマンドの `--help`、知らないフラグ、`SAKAI_LANG`）、`tests/api.rs`（B.12 の地図ごとの api の golden、`tests/golden/api/<名前>.json`）、`tests/design.rs`（DESIGN に貼った sakai の出力が、実物と同じ。`$ sakai` の塊を走らせ、診断の塊を golden と、9 章の api の抜粋を api の golden と、2.6 の JSON を名指しの読み手と突き合わせる。`.ctx` の塊が構文を通ることは `tests/syntax.rs` が確かめる）。B で、3.1 の要約、5.3 の三つの診断、9 章の api は、B の地図（`tests/maps/基本/`）の実物に差し替えた。例の地図のものは、例を作る C で足す。

### B.12 B の地図（fixtures）

一式のツールを使わない地図を `tests/maps/` に置く。名前は日本語で書く。

**`tests/maps/基本/`**：三つのコンテキスト（受注、在庫、請求）、proto と Python のコードだけ。

- 地図 `基本.ctx`：`use context` で `ctx/受注.ctx`、`ctx/在庫.ctx`、`ctx/請求.ctx`。`covers "."`、`proto root "proto"`、`code python "py"`。
- 在庫：`proto/warehouse/v1/stock.proto`（C.0 の本文に、`import "shop/common/v1/money.proto";` と、`ReserveResponse` の `shop.common.v1.Money price = 4;` を足したもの）、`py/inventory/…`。公表された言語 `warehouse.v1`（公開ホストサービス `StockService`、`PackingService`）、語「引当」（`message ReserveResponse`）と「梱包の状態」（`enum PackingStatus`）。
- 受注：`proto/shop/ordering/v1/order.proto`（C.0 の本文）、`proto/shop/ordering/v1/fulfillment_lite.proto`（`import "warehouse/v1/stock.proto";` と `message FulfillResponse { repeated warehouse.v1.ReserveResponse reservations = 1; }`）、`py/ordering/…`。公表された言語 `shop.ordering.v1`、語「注文」（`message Order`）と「キャンセル」（`enum OrderStatus value ORDER_STATUS_CANCELLED`）。`upstream 在庫 conformist`、`through warehouse.v1`。`proto/shop/common/v1/money.proto`（`message Money { int64 amount_jpy = 1; }`）も受注のもので、受注と在庫の両方が `shared kernel with` にそれを並べる。
- 請求：`proto/shop/billing/v1/billing.proto`（`enum BillingStatus { BILLING_STATUS_UNSPECIFIED = 0; BILLING_STATUS_WAIT = 1; BILLING_STATUS_BILL = 2; BILLING_STATUS_SKIP = 3; }`。公表された言語 `shop.billing.v1`）、`proto/billing/acl/v1/order_view.proto`（package `billing.acl.v1`。`import "shop/ordering/v1/order.proto";` と `message OrderView { shop.ordering.v1.OrderStatus status = 1; }`。請求の内側で、腐敗防止層の中）、`py/billing/…`。語「キャンセル」（「請求を確定したあとに請求を取り消し、返金すること」）。`upstream 受注 anticorruption layer`、`through shop.ordering.v1`、`layer dir "../proto/billing/acl"`、`enum OrderStatus -> enum BillingStatus` の下に `ORDER_STATUS_RECEIVED -> BILLING_STATUS_WAIT`、`ORDER_STATUS_PAID -> BILLING_STATUS_BILL`、`ORDER_STATUS_SHIPPED -> BILLING_STATUS_BILL`、`ORDER_STATUS_CANCELLED -> BILLING_STATUS_SKIP`。
- 期待：`check` は exit 0 で、境界を越える参照は三つ：`stock.proto` → `money.proto`（共有カーネル）、`fulfillment_lite.proto` → `stock.proto`（順応者）、`order_view.proto` → `order.proto`（腐敗防止層、層の中）。受注の「キャンセル」は請求の「キャンセル」とぶつかるが、`BILLING_STATUS_SKIP` に読み替えられているので通る。

**`tests/maps/パターン/`**：顧客／供給者（両側）、顧客と腐敗防止層、パートナーシップ（両側）、別々の道（片側）、写しを両側に置く共有カーネル（中身が同じ二つのファイル）が、どれも通る地図。五つのコンテキスト：料金（`pricing.v1` を公表し、見積と契約の二つの顧客を `downstream … supplier` で引き受ける）、見積（`upstream 料金 customer`、通知とパートナーシップ、契約と写しの共有カーネル）、契約（`upstream 料金 customer, anticorruption layer`、層の中の `plan_view.proto` が料金のプランを受け取り、`enum Plan -> enum ContractClass` で `PLAN_FREE` を断る）、通知（見積の公表された言語を使う）、監査（`separate ways from 見積`）。境界を越える参照は三つ（見積と契約の層から料金へ、通知から見積へ）。

**`tests/mutants/`**：B のコードごとに一つ以上（B の段階で 51）。`基本` か `パターン` を写して一か所だけ変えたもの（B.10 の重ね方）。名前は `<コード>_<内容>`（`E401_注文の状態に値が増えた`、`E406_順応者の同じ語` など）。どの変異も、そのコードを一つだけ出す（ほかのコードが混ざると golden が読みにくい）。混ざるときは、その変異の `README.md` に理由を一行書き、テストは README に挙がったコードだけを許す（B では `E307_片側だけ` と `E309_片側だけのパートナーシップ` が E201 も出す。片側だけの合意は参照を許さないので）。

### B.13 B の完了の条件

`cargo build` が警告なしで通り、`cargo test -- --nocapture` が全部通ること。SKIP は buf が無いときの proto の比べ合わせだけで、この機械では 0。そのうえで、次が成り立つこと。

| 対象 | 成り立つこと |
|---|---|
| 構文 | DESIGN と C.0 の `.ctx` の塊が全部構文を通る（`tests/design.rs`、`tests/syntax.rs`） |
| `基本` | `check` が exit 0。境界を越える参照が B.12 の三つちょうど。api の golden と一致 |
| `パターン` | `check` が exit 0。api の golden と一致 |
| 台帳 | 0.4 の B のコードの全部に、変異と英語と日本語の golden があり、`explain` の再現がそのコードを出す |
| proto | buf と比べた五つの項（package、import、メッセージとフィールド、列挙と値、サービスとメソッド）が一致する |
| E401 | `基本` の `order.proto` に `ORDER_STATUS_RETURNED = 5;` を足した変異で、E401 が請求の対応の `ORDER_STATUS_RETURNED` だけを名指す |
| CLI | 知らないフラグ、二度書いたフラグ、値の無いフラグが exit 2。`--help` が全コマンドにある |

報告には、決めた要約の一行の形と、`基本` の変異の診断をいくつか（英語と日本語）書く。

## 3. 段階 C：一式の読み込み、コードの import、CML

### C.0 例の地図と成果物とコード

C のテストが使うので、例 `examples/通販/` をここで作る（D で英語の地図と説明を足して仕上げる）。置き場所は DESIGN 11 章のとおり。

**作った（C の段階）**：下の表と本文のとおりに写して書いた。写したファイルの頭のコメントに、写した元と直したところを書いた（祝日の表と `options.proto` は元のまま）。表の直し方のほかに、`受注.flow` のコメントと説明の中の古い名前（在庫の値、子の `.flow`）を直した。写したものは、どれもそれぞれのツールの検査を通った（`tests/examples.rs`）。TypeScript では、`ts/billing/invoice.ts` と `ts/delivery/ship.ts` が層の外で上流の型を import しないよう、Python と同じく数を受け取る形にした。Java は `java/src/test/java/` に sakai が書いた ArchUnit のテストを置き、Go は `go/go.mod`（`module example.com/shop`）で `go build -trimpath ./...` が通る。生成したコードが内側のディレクトリの中にある地図 `tests/maps/入れ子/`（四つの言語。Python は `warehouse/__init__.py` の下に protoc の `warehouse/v1/`）も作った。

**写すもの**（元の木は読むだけ。写したら、写した元をファイルの頭のコメントか、例の README に書く）：

| 例の中のパス | 元 | 直すところ |
|---|---|---|
| `inventory/在庫の引当.book` | chobo の `examples/inventory/inventory.ja.book` | なし |
| `calendars/東京の営業日.cal`、`calendars/data/syukujitsu.csv` | koyomi の `examples/calendars/` | なし（祝日の表は取り直さない） |
| `billing/支払条件.cal` | koyomi の `examples/支払_20日締め翌月10日払い.cal` | `use calendar "calendars/東京の営業日.cal"` を `use calendar "../calendars/東京の営業日.cal"` に |
| `billing/rules/決済手数料.rule` | rulec の `tests/corpus/決済手数料.rule` | なし |
| `billing/rules/出荷の送料.rule` | rulec の `tests/corpus/出荷の送料.rule` | `shape` の行の `"contracts/shipment.proto" shop.v1.CreateShipmentRequest` を `"../../proto/shop/delivery/v1/shipment.proto" shop.delivery.v1.CreateShipmentRequest` に |
| `delivery/rules/出荷の急ぎ.rule` | dandori の `examples/order/rules/出荷の急ぎ.rule` | なし |
| `proto/shop/delivery/v1/shipment.proto` | rulec の `tests/corpus/contracts/shipment.proto` | `package shop.v1;` を `package shop.delivery.v1;` に。`import` の行のあとに、`service DeliveryService { rpc CreateShipment(CreateShipmentRequest) returns (CreateShipmentResponse); }` を、末尾に `message CreateShipmentResponse { string tracking_number = 1; }` を足す |
| `proto/shop/ordering/v1/fulfillment.proto` | dandori の `examples/fulfillment/specs/fulfillment.ja.proto` | `package shop.ja.v1;` を `package shop.ordering.v1;` に、`import "warehouse.proto";` を `import "warehouse/v1/stock.proto";` に、`message Order {` を `message FulfillmentOrder {` に、`Order order = 1 [json_name = "注文"];` を `FulfillmentOrder order = 1 [json_name = "注文"];` に（`order.proto` の `Order` とぶつかるため） |
| `proto/dandori/v1/options.proto` | dandori の `proto/dandori/v1/options.proto` | なし（dandori の README のとおり、利用者が proto のルートに写すもの。sakai は成果物にしない） |
| `ordering/受注.flow` | dandori の `examples/fulfillment/temporal/fulfillment.ja.flow` | `use rule 急ぎ from "../../order/rules/出荷の急ぎ.rule"` を `use rule 急ぎ from "../delivery/rules/出荷の急ぎ.rule"` と、その下の行 `  connect "https://rules.example.com"` に。`use proto 店 from` の先を `"../proto/shop/ordering/v1/fulfillment.proto"` に、`use proto warehouse from` の先を `"../proto/warehouse/v1/stock.proto"` に、`flow "../arrange_delivery.ja.flow"` を `flow "../delivery/配送の手配.flow"` に、`short => let 足りない = true` と `secured => pass` を `STOCK_SHORT => let 足りない = true` と `STOCK_SECURED => pass` に |
| `delivery/配送の手配.flow` | dandori の `examples/fulfillment/arrange_delivery.ja.flow` | なし |

A の段階で、この直し方で作った下書きが、rulec 0.22.1（`出荷の送料.rule`、`決済手数料.rule` が ok）、koyomi 0.1.0（`支払条件.cal` が ok）、dandori 0.1.0（`受注.flow`、`配送の手配.flow` が ok）、chobo 0.1.0（`在庫の引当.book` が ok）を通った。写したら同じコマンドで確かめ直す（`tests/examples.rs`。ツールが無ければ SKIP）。

**例のために書くもの**（A の段階で一式のツールに通した本文）：

`proto/shop/ordering/v1/order.proto`（buf 1.54.0 の lint を通った）：

```proto
syntax = "proto3";

package shop.ordering.v1;

// 受注の公表された言語。注文の状態と、注文を引くサービス。
service OrderService {
  rpc GetOrder(GetOrderRequest) returns (GetOrderResponse);
}

enum OrderStatus {
  ORDER_STATUS_UNSPECIFIED = 0;
  ORDER_STATUS_RECEIVED = 1;
  ORDER_STATUS_PAID = 2;
  ORDER_STATUS_SHIPPED = 3;
  ORDER_STATUS_CANCELLED = 4;
}

message Order {
  string id = 1;
  OrderStatus status = 2;
  int64 amount_jpy = 3;
}

message GetOrderRequest {
  string id = 1;
}

message GetOrderResponse {
  Order order = 1;
}
```

`proto/warehouse/v1/stock.proto`（buf 1.54.0 の lint を通った。dandori の例の `warehouse.proto` の二つのメソッドに、値の名前の接頭辞と、梱包のサービスを足したもの）：

```proto
syntax = "proto3";

package warehouse.v1;

// 在庫の公表された言語。注文の一行ごとの引当と取消、梱包の進み。
service StockService {
  rpc Reserve(ReserveRequest) returns (ReserveResponse);
  rpc Release(ReleaseRequest) returns (ReleaseResponse);
}

service PackingService {
  rpc GetPacking(GetPackingRequest) returns (GetPackingResponse);
}

message ReserveRequest {
  string sku = 1;
  int32 quantity = 2;
}

enum Stock {
  STOCK_UNSPECIFIED = 0;
  STOCK_SECURED = 1;
  STOCK_SHORT = 2;
}

message ReserveResponse {
  string sku = 1;
  Stock stock = 2;
  optional string id = 3;
}

message ReleaseRequest {
  string id = 1;
}

message ReleaseResponse {}

enum PackingStatus {
  PACKING_STATUS_UNSPECIFIED = 0;
  PACKING_STATUS_WAITING = 1;
  PACKING_STATUS_PACKED = 2;
  PACKING_STATUS_SHORT = 3;
}

message GetPackingRequest {
  string order_id = 1;
}

message GetPackingResponse {
  PackingStatus status = 1;
}
```

`billing/rules/請求の要否.rule`（rulec 0.22.1 で ok。`order.proto` に値を足すと E032 で止まることも確かめた。DESIGN 11 章）：

```
rule 請求の要否(billing_need) v1
description "受注の注文の状態から、請求するか、待つか、請求しないかを決める"

import proto "../../proto/shop/ordering/v1/order.proto" OrderStatus -> 注文の状態
enum 注文の状態(order_status) = 受付(received) | 支払済(paid) | 出荷済(shipped) | 受注で取消(cancelled)
enum 請求の扱い(handling) = 待つ(wait) | 請求する(bill) | 請求しない(skip)

inputs
  状態(status) : 注文の状態

outputs
  扱い(handling) : 請求の扱い

table 扱い表(handling_table)
policy unique
| 状態       | -> 扱い(handling) : 請求の扱い |
| 受付       | 待つ                           |
| 支払済     | 請求する                       |
| 出荷済     | 請求する                       |
| 受注で取消 | 請求しない                     |
```

`delivery/出荷日.cal`（koyomi 0.1.0 で「2 claims hold on all 719 days of 受注日 (2026-01-01..2027-12-20)」）：

```
dates 出荷日(ship_date) v1
description "注文を受けた日の次の営業日に出荷する"
use calendar "../calendars/東京の営業日.cal"

inputs
  受注日(ordered) : date  range >=2026-01-01 <=2027-12-20

date 出荷日(ship) = 受注日
  + 1 business day

claims
  営業日に出す : 出荷日 is open
  遅い注文は遅い出荷 : 出荷日 is monotonic
```

**地図とコンテキスト**：`通販.ctx`、`contexts/在庫.ctx`、`contexts/配送.ctx` は DESIGN 1.1 の本文。残りの三つは次の本文（A の段階の案。C で sakai に通して直したら、DESIGN の該当の塊も直す）。

```ctx
context 受注(ordering) v1
description "注文を受け付け、在庫を押さえ、配送に渡し、届くまでを見届ける"
owner "受注チーム"

owns
  dir "../ordering", "../proto/shop/ordering"
  dir "../py/ordering", "../py/shop/ordering", "../ts/ordering", "../ts/shop/ordering"
  dir "../java/src/main/java/ordering", "../java/src/main/java/shop/ordering"
  dir "../go/ordering", "../go/shop/ordering"

published language shop.ordering.v1
  proto "../proto/shop/ordering/v1/order.proto", "../proto/shop/ordering/v1/fulfillment.proto"
  open host service OrderService, FulfillmentService
  generated dir "../py/shop/ordering/v1", "../ts/shop/ordering/v1"
  generated dir "../java/src/main/java/shop/ordering/v1", "../go/shop/ordering/v1"

terms
  注文 "客が確定させた購入の申し込み。取り消されても消えない"
    means message Order
  キャンセル "出荷の前に、客の申し出で注文を取り消すこと"
    means enum OrderStatus value ORDER_STATUS_CANCELLED

upstream 在庫 conformist
  through warehouse.v1

partnership with 配送
```

```ctx
context 請求(billing) v1
description "注文に請求するかを決め、手数料と送料を出し、支払の期日を決める"
owner "経理チーム"

owns
  dir "../billing", "../calendars"
  dir "../py/billing", "../py/calendars", "../ts/billing", "../ts/calendars"
  dir "../java/src/main/java/billing", "../java/src/main/java/calendars"
  dir "../go/billing", "../go/calendars"

published language rulec.payment_fee.v1
  rulec "../billing/rules/決済手数料.rule"
  open host service PaymentFeeService

terms
  手数料 "決済の一件ごとに、契約の率から決まる額"
    means rulec "../billing/rules/決済手数料.rule" output 手数料
  キャンセル "請求を確定したあとに請求を取り消し、返金すること"

upstream 受注 anticorruption layer
  through shop.ordering.v1
  layer rulec "../billing/rules/請求の要否.rule"
  layer dir "../py/billing/acl/ordering", "../ts/billing/acl/ordering"
  layer dir "../java/src/main/java/billing/acl/ordering", "../go/billing/acl/ordering"
  enum OrderStatus -> rulec "../billing/rules/請求の要否.rule" enum 注文の状態

upstream 配送 customer
  through shop.delivery.v1

shared kernel with 配送
  koyomi "../calendars/東京の営業日.cal"
  dir "../py/calendars", "../ts/calendars", "../java/src/main/java/calendars", "../go/calendars"
```

```ctx
context レビュー(reviews) v1
description "買った人の感想を集めて見せる。請求とは何もやりとりしない"
owner "受注チーム"

owns
  dir "../py/reviews", "../ts/reviews", "../java/src/main/java/reviews", "../go/reviews"

separate ways from 請求
```

**コード**：四つの言語に、同じ形の小さなコードを置く。公表された言語から生成したコードは、protoc が書く形（Python は `__init__.py` を置かない名前空間のパッケージで、ファイルは `<名前>_pb2.py`。TypeScript は `<名前>_pb.ts`。Java は package のディレクトリ。Go は `package <名前>v1`）の、数行の手書きにする。chobo と koyomi が書くコード（在庫の `ledger`、共有カーネルの `calendars` の `tokyo`）も同じく数行の手書きにする。どのファイルも頭のコメントに、どのコマンドが本物を書くかを書く。

Python（`py/`）の import は次のとおり。TypeScript（`ts/`。相対パスで、拡張子 `.ts` を付けて import する）、Java（`java/src/main/java/`。メソッドの中で実際に使い、クラスファイルに依存が残るようにする）、Go（`go/`。`go.mod` は `module example.com/shop` と `go 1.25`。`go build ./...` が通ること）も、同じ向きの import を持つ。

| ファイル | import するもの | 地図のどれが許すか |
|---|---|---|
| `ordering/fulfill.py` | `shop.ordering.v1.order_pb2`、`warehouse.v1.stock_pb2` | 自分の公表された言語、順応者 |
| `inventory/service.py` | `inventory.ledger`、`warehouse.v1.stock_pb2` | 自分の内側、自分の公表された言語 |
| `shop/ordering/v1/fulfillment_pb2.py` | `warehouse.v1.stock_pb2` | 生成したコードどうし（`fulfillment.proto` が `stock.proto` を import する） |
| `billing/invoice.py` | `billing.acl.ordering.status`、`calendars.tokyo`、`shop.delivery.v1.shipment_pb2` | 自分の層、共有カーネル、顧客 |
| `billing/acl/ordering/status.py` | `shop.ordering.v1.order_pb2` | 腐敗防止層の中 |
| `delivery/ship.py` | `delivery.acl.inventory.packing`、`calendars.tokyo`、`shop.ordering.v1.order_pb2`、`shop.delivery.v1.shipment_pb2` | 自分の層、共有カーネル、パートナー、自分の公表された言語 |
| `delivery/acl/inventory/packing.py` | `warehouse.v1.stock_pb2` | 腐敗防止層の中 |
| `reviews/stars.py` | なし | |

`tests/code/<言語>/` に、地図に無い import を一つずつ持つファイルを置く（DESIGN 7.6 の四つ）。

| 番号 | 置き場所（Python の例） | import | 捕まるはずのまとまり |
|---|---|---|---|
| 1 | `ordering/bad_internals.py` | `from inventory import ledger` | 在庫の内側 |
| 2 | `billing/bad_outside_layer.py` | `from shop.ordering.v1 import order_pb2` | 受注の公表された言語（請求は層の中からだけ） |
| 3 | `ordering/bad_kernel.py` | `from calendars import tokyo` | 請求と配送の共有カーネル |
| 4 | `reviews/bad_separate.py` | `from billing import invoice` | 請求の内側 |

### C.1 口のまとまり（`src/suite.rs`、`src/run.rs`）

**ritsu の D.8 で、口で作るように書き直してから作った**。C の段階では、一式のツールを子プロセスで呼び、`rulec api` などの JSON を読む計画で、一式の言語を一つの処理系にまとめるかが決まるまで止めてあった。まとめると決まったので、ritsu の口（ritsu の DESIGN 3.2）で読む（DESIGN 4.1）。子プロセス、ツールの探し方（環境変数と PATH）、版の確かめ、時間の上限は、どれも作らない。

- `Suite`：`rules`（rulec の `Rules`）、`references`（rulec、koyomi、dandori の `References`）、`books`（chobo の `Books`）。一つの実行で、同じファイルには一度だけ問う（`Suite::facts`、`Suite::references`）。
- `run(引数, 口, 標準出力, 標準エラー) -> 終了コード`（`src/run.rs`）。sakai のクレートのバイナリ（`src/main.rs`）は何もつながない `Suite::default()` を、ritsu の `ritsu sakai` はすべてをつないだ口を渡す。ライブラリの入口は `check::check_map_with`、`check::check_args_with`、`build::run_with` で、口を受け取る。
- 段 3（DESIGN 3.1）で `suite::read` を呼ぶ。地図が rulec・koyomi・dandori の成果物を含むのに、その言語の口が無ければ E104（言語ごとに一度、その言語の最初の成果物を持つ `owns` の行で。注に `ritsu sakai <同じ引数>`）。言語がファイルに答えなければ E105（その言語の `Said` を注に五つまで）。どちらのときも W401 は出さない（何が越えるかが分からない）。
- テスト：`tests/examples.rs` の `the_binary_of_this_crate_says_what_it_cannot_read`（クレートのバイナリが例で E104 を三つ出し、`ritsu sakai check examples/通販/通販.ctx` を言う）。`tests/codes.rs`（E104 の再現は何もつながずに、ほかの言語の成果物を含む再現は、すべてをつないで走らせる）。テストは rulec、koyomi、chobo、dandori を `[dev-dependencies]` に持ち、`tests/common/mod.rs` の `suite()` が `ritsu sakai` と同じにつなぐ。

### C.2 rulec（`Rules` と `References`）

- 事実（`Rules::facts`）を、地図の規則ごとに一度だけ問う。答えた規則には参照（`References`）も問う。答えなければ E105。
- 参照：`import proto`（先：proto の列挙。越える要素はその列挙）、`shape`（先：proto のメッセージ。越える要素は、そのメッセージとたどれる型の全部）、`apply`（先：規則。規則そのものを使う参照で、共有カーネルの中でなければ E202）。`import jsonschema`、JSON Schema の `shape`、`source` の写しは、規則の一部として数えない（DESIGN 3.3）。
- 公表された言語の rulec の塊：事実の Connect のパスの package と見出しの突き合わせ（E302）、パスのサービスと `open host service`（E301）。語の `means` の先の規則の要素（`input`、`output`、`enum`、`value`）が事実にあること。無ければ E007（★proto の要素と同じ。前のこの計画は E408 と書いていたが、E408 は公表された言語に無い要素のコードで、名指した先に無いことは proto でも E007 なので、そろえた）。
- 対応の先が rulec の列挙（DESIGN 1.7）：規則の Connect の列挙が、対応の左辺と同じ proto の同じ列挙を取り込んでいれば（取り込んだファイルが同じで、別名が列挙の完全な名前）、値の対応を事実から読む。値の行が書いてあれば突き合わせ、違えば E405。値の行が無ければ、規則の取り込みが対応になる。取り込んでいなければ値の行が要り、右辺が規則の列挙の値であること（E403）。
- 同じ語：rulec から読んだ越えてくる要素も、B.9 の検査に入れる。規則の取り込みの対応も、E407 の対応の先にする。
- テスト（`tests/examples.rs`、`tests/api.rs`、`tests/mapping.rs`、`tests/mutants/`）：
  - 例の `請求の要否.rule` から参照が一つ（`import proto`、先は `proto/shop/ordering/v1/order.proto` の `OrderStatus`）、`出荷の送料.rule` から一つ（`shape`、先は `CreateShipmentRequest` で、越える要素は `Destination`、`Parcel`、`Handling` と合わせて四つ）。`決済手数料.rule` の塊の package が `rulec.payment_fee.v1`、サービスが `PaymentFeeService`（見出しを替えると E302、サービスを替えると E301）。
  - 変異：`order.proto` に `ORDER_STATUS_RETURNED = 5;` を足すと、`請求の要否.rule` で E105（注に rulec の E032）。`請求の要否.rule` の値 `受注で取消(cancelled)` を `キャンセル(cancelled)` にすると E407。`.ctx` の値の行を一つ食い違わせると E405。語の `means` を規則に無い出力にすると E007。請求の規則が配送の規則を `apply` すると E202（`E202_境界の向こうの規則を展開`）。
  - 取り込んでいない規則の列挙を対応の先にし、右辺がその値でなければ E403（`tests/mapping.rs`）。

### C.3 koyomi（`References`）

- dates のファイルとカレンダーの `use calendar` を参照にする（先はカレンダー）。カレンダーは公表された言語にならないので、共有カーネルの中でなければ E202（関係が無ければ E201）。koyomi の `source`（祝日の表）は、カレンダーの一部として数えない。
- テスト：例の `出荷日.cal`（配送）から `calendars/東京の営業日.cal`（請求のもの）への参照が一つで、共有カーネルが許す。`支払条件.cal` から同じカレンダーへの参照は、同じコンテキストの中で、境界を越えない。変異：請求の `shared kernel with 配送` を消すと E307 と E201（`E307_片側だけの共有カーネルとカレンダー`）。前のこの計画は E202 と書いていたが、共有カーネルが片側だけになると、二つのあいだに参照を許す関係が無い（請求が配送の顧客で、配送から請求へは向きが逆）ので、DESIGN 3.3 の順で E201 になる。

### C.4 dandori（`References`）

作者の決定を待っていた dandori の api は、ritsu で dandori が口（`References`）に答えるようになって決着した（DESIGN 4.7 の「これまでの形」）。N101 は退かせた（台帳に残し、番号を使い回さない）。

- 参照：`use rule`（呼び方の語つき）、`use proto`、`connect`、`flow`、`implements`（DESIGN 4.7 の表）。`use openapi` と `use smithy` は数えない。
- 四つの検査：規則の同梱が境界を越える（E202。`connect` の無い `use rule`）、`connect` で呼ぶサービスが相手の公開ホストサービスでない（E207。`use rule … connect` の規則のサービスも）、`implements` するサービスが自分の公表された言語の公開ホストサービスでない（E208）、子の `.flow` が境界の向こうのもの（E209。パートナーシップ、共有カーネル、子が相手の公開ホストサービスを実装しているときは許す。DESIGN 4.7 の決定）。
- テスト：例の `受注.flow` から五つの参照が境界を越え、どれも関係が許す（`tests/examples.rs`、`tests/api.rs`）。例を土台にした変異：`use rule` の `connect` を消すと E202、在庫の `open host service` から `StockService` を外すと E207 が二つ、受注の `open host service` から `FulfillmentService` を外すと E208、受注と配送のパートナーシップを顧客と供給者に替えると E209。台帳の再現（甲と乙が土台）も四つにある。

### C.5 chobo（`Books`）

- `Books::facts` から勘定と振替の名前を読む（`suite::book_names`）。`check` は chobo に問わない（帳簿は参照のもとにならない。DESIGN 4.5）。D の doc がこれを使う。
- テスト（`tests/examples.rs`）：`在庫の引当.book` から、勘定 `在庫`、`仕入先`、`客`、振替 `入荷`、`引当`、`返品` を読む。

### C.6 import のまとまりと向き（`src/build/areas.rs`）

**作った（C の段階）**：計画のとおり。計画に無く決めたことが四つある（DESIGN 7.1 に書いた）。表の「X」は X の内側、生成したコード、層のまとまり。共有カーネルのまとまりは、両側のどのまとまりからも import されてよいが、自分からは自分の中しか import しない。同じ深さのディレクトリでは、生成したコード、層、共有カーネルが内側より先に来る。コードのファイルを一つだけ名指した `owns` や `layer` の項は E501。テストは例の四つの言語と `tests/maps/入れ子/` の表を確かめる。

- DESIGN 7.1 の四つの種類のまとまりを、言語ごとに作る。まとまりは、`code` の置き場所の下のディレクトリの集まりで、入れ子はいちばん深いまとまりに属する。コードのファイルを一つも含まないまとまりは作らない。
- 許す向きの表（DESIGN 7.1）を作る。生成したコードどうしの向きは、proto の import のうち、地図が許すもの（B.7）から作る。
- ツールの言葉への写し方の共通の部分：まとまりの名前（`sakai-<コンテキストの別名>`、`sakai-<別名>-pl-<package>`、`sakai-<別名>-kernel-<相手の別名>`、`sakai-<別名>-layer-<上流の別名>`）、地図の言葉の説明（英語と日本語。設定のファイルは `--lang` の言語で書く）。
- テスト（`tests/build.rs`）：例の Python のまとまりと、それぞれを import してよいまとまりの一覧（受注の公表された言語は、受注、請求の層、配送から。在庫の公表された言語は、在庫、受注、配送の層、受注の生成したコードから。共有カーネルは請求と配送から）。生成したコードが内側のディレクトリの中にある地図（`tests/maps/入れ子/`）のまとまり。

### C.7 import-linter（`src/build/import_linter.rs`）

**作った（C の段階）**：計画と違うところが三つある（DESIGN 7.2）。(1) grimp 3.17 は、パッケージの下の `__init__.py` の無いディレクトリに入らないので、そういうディレクトリのうちモジュールのあるものも `root_packages` に足す。(2) 入れ子を外すための `as_packages = False` は、別の契約にせず、その契約全体を一つずつのモジュールで書く形にした。import-linter は守るモジュールを import してよいのをそのモジュールとその下だけとみなすので、まとまり自身のモジュールも `allowed_importers` に並べる必要があり、別の契約に分けると、その中でパッケージとして並べたものが下のまとまりまで許してしまう。(3) 置き場所の直下のモジュールは、grimp が根に受け付けないので E501。頭のコメントに SHA-256 は書かない（C.11）。

- `.importlinter`（INI）。頭のコメントに、元の地図のファイルと SHA-256、`sakai build` で書いたこと。`[importlinter]` の `root_packages` は、Python の置き場所の直下のディレクトリのうち、`__init__.py` のあるものはその名前、無いもの（名前空間）はその下を `__init__.py` のあるディレクトリかモジュールのあるディレクトリまで降りた名前（`warehouse.v1`、`shop.ordering.v1`）。A の段階に、import-linter 2.15 が名前空間の一部をルートに受け付けることを確かめた。
- まとまりごとに `protected` の契約。`protected_modules` はまとまりのモジュール。入れ子のまとまりを外すため、まとまりのディレクトリのうち、ほかのまとまりを含まないいちばん大きいモジュールを並べる。ほかのまとまりを含み、`__init__.py` を持つディレクトリは、`as_packages = False` の別の契約でそのモジュールだけを守る。`allowed_importers` は DESIGN 7.1 の表で import してよいまとまりのモジュール。
- グラフに無いモジュールを書かない（DESIGN 7.2）。
- テスト：例の Python で、`lint-imports --no-logo --no-cache --config <書いた設定>` が全部 KEPT で exit 0、`Analyzed` の数が 0 でない。C.0 の四つの変異のそれぞれで exit 1、足したファイルのモジュールと行（`(l.1)` など）が出力にある。`tests/maps/入れ子/` で、入れ子の生成したコードを外の下流が import でき、内側は import できないこと。

### C.8 dependency-cruiser（`src/build/depcruise.rs`）

**作った（C の段階）**：入れ子は `to.pathNot` ではなく、否定先読み（`(?!…)`）で外す（`from` の側にも同じ外し方が要るため。DESIGN 7.3）。`--output-type json` は違反があっても exit 0 なので、テストは JSON の `summary.violations` で決める。TypeScript を読むことは、`depcruise --info` の `✔ typescript` で確かめる（読まなければ失敗）。

- `.dependency-cruiser.cjs`（先頭の行にコメント、続けて `module.exports = { … };`）。まとまりごとに `forbidden` の規則（`name` は C.6 の名前、`comment` は説明、`severity` は `error`、`to.path` はまとまりのパスの正規表現で入れ子のまとまりは `to.pathNot` で外す、`from.pathNot` は import してよいまとまりのパスとまとまり自身）。`options` は `tsPreCompilationDeps: true`、`doNotFollow: {path: "node_modules"}`、`except` のパスがあれば `exclude`。パスは TypeScript の置き場所から見たもので、正規表現の特別な文字は逃がす。先頭のコメントに元の地図とハッシュを書く（JSON にしないのは、dependency-cruiser 16.10.4 が知らないキーを一番上にも `options` の中にも置かせないから。DESIGN 7.3）。
- テスト：例の TypeScript で `depcruise --config <書いた設定> --output-type json .` が違反 0、`summary.totalCruised` が 0 でない。四つの変異のそれぞれで違反が一つ、`from` が足したファイル。TypeScript 6 以上が `tools/` に入っていたら、テストを失敗させて理由を言う（黙って通るのを防ぐ。DESIGN 7.3）。

### C.9 ArchUnit（`src/build/archunit.rs`）

**作った（C の段階）**：まとまりを `JavaClass.Predicates` の述語にし（入れ子は `resideOutsideOfPackages` で外す）、規則は `noClasses().that(DescribedPredicate.not(<import してよいまとまり>)).should().dependOnClassesThat(<まとまり>)`。どのクラスも使ってよいまとまりは、規則を書かずに理由をコメントに書く。テストは `--details=tree` で走らせ、見つかったテストの数が書いた `@ArchTest` の数と同じことを確かめる（例では 11、`tests/maps/入れ子/` では 2）。

- `SakaiContextsTest.java`（JUnit 5、package なし）。`@AnalyzeClasses(packages = {…})` に、Java の置き場所の直下のパッケージを全部。まとまりごとに `@ArchTest static final ArchRule <まとまりの名前を Java の識別子にしたもの>`。規則の形は DESIGN 7.4。守る相手のクラスが一つも無い規則は書かない（`failOnEmptyShould`。DESIGN 7.4）。
- テスト：例の Java を `javac --release 21` で組み、書いたテストを ArchUnit と JUnit の jar で組み、`java -jar junit-platform-console-standalone-6.1.3.jar execute --class-path … --select-class SakaiContextsTest --disable-banner --details=summary --disable-ansi-colors` が全部成功で exit 0、見つかったテストの数がまとまりの数と同じ。四つの変異のそれぞれで、失敗が一つで、その文面に足したクラスと行（`(BadInternals.java:5)` など）がある。

### C.10 go-arch-lint（`src/build/go_arch_lint.rs`）

**作った（C の段階）**：go-arch-lint v1.19.0 は、同じコンポーネントの中のパッケージどうしの import も、`mayDependOn` に自分を並べないと止め、中身の無い項を受け付けないので、どのコンポーネントも自分を並べる。go.mod の無い置き場所は E501。テストは go のキャッシュをテストの一時ディレクトリに置く。

- `.go-arch-lint.yml`（`version: 3`、`workdir: .`、`allow: {depOnAnyVendor: true}`、`excludeFiles` に `except` のパスの正規表現）。まとまりごとにコンポーネント（`in:` にディレクトリの `/**`）、`deps` の `mayDependOn` に import してよいコンポーネント。頭のコメントに元の地図とハッシュ。
- テスト：例の Go で `go-arch-lint check --json` が警告 0（`ArchWarningsDeps` と `ArchWarningsNotMatched` が空）で exit 0。四つの変異のそれぞれで exit 1、`ArchWarningsDeps` に足したファイルと行。`go` と `go-arch-lint` は `-trimpath` と、`tools/go/` の下のキャッシュで動かす。

### C.11 build と --check（`src/build/mod.rs`）

**作った（C の段階）**：計画と違うところが二つある。(1) 設定の頭に `.ctx` の SHA-256 を書かない。`.ctx` をどこか直すだけで変わり、`--check` が中身の変わらない設定を書き直させるからである（DESIGN 7.1）。頭には、設定のファイルのディレクトリから見た地図のパスと、書いたコマンド（`--lang ja` なら、それも）を書く。(2) golden は `tests/golden/build/` ではなく、例のコードの置き場所に置いた四つの設定そのものにした。利用者がそこで読み、CI の `sakai build --check` と同じ使い方になる（`tests/build.rs`。`SAKAI_BLESS=1` で書き直す）。違う `--lang` で `--check` すると E502 になり、注で書いたときの言語を言う。E501 は、計画の三つのほかに、ファイルの項、置き場所の直下の Python のモジュール、デフォルトパッケージの Java のクラス、`test` の無い ArchUnit、go.mod の無い Go。

- `sakai build <map.ctx> --target …`（DESIGN 6 章）。検査を通らない地図からは書かない（exit 1）。`code` の無い言語へのビルド、コードを持つコンテキストが無い言語、モジュールの名前にならないディレクトリ（Python で `-` を含む、Java で予約語）は E501。
- `--check`：書く代わりに、いまある設定の本文と、書くはずの本文を比べる。違えば E502 で、最初に違う行を注に出す。
- テスト：書いた設定が golden（作ったときに、例に置いた設定そのものにした。上の記録）と一致する。書いたあとの `--check` が exit 0。地図を一か所変えると `--check` が E502。

### C.12 tools/

**作った（C の段階）**：計画のとおり（`tools/requirements.in` も置いた）。この機械では、全部を `tools/` に入れ、go のビルドとモジュールのキャッシュは作業場所に置いた。

- `tools/requirements.txt`（`import-linter==2.15` と依存を、`uv pip compile --generate-hashes` でハッシュつきに）、`tools/package.json` と `tools/package-lock.json`（`dependency-cruiser` 16.10.4、`typescript` 5.9.3。`npm ci --prefix tools`）、`tools/java/fetch.sh`（0.4 の jar を `tools/java/lib/` に取り、SHA-256 を確かめる。違えば止まる）、`tools/go/install.sh`、`tools/cml/fetch.sh`（zip を取り、SHA-256 を確かめ、`tools/cml/` に開く）、`tools/cml/Validate.java`（DESIGN 0.4 の検査器。引数の CML のファイルを `IResourceValidator` の `CheckMode.ALL` で確かめ、`<severity> <ファイル>:<行>:<列>: <文>` を一行ずつ出し、エラーがあれば exit 1）、`tools/README.md`（入れ方）。
- テストは、ツールが無ければ SKIP の行に、入れ方（どのスクリプトを走らせるか）を書く。

`tools/cml/Validate.java` は、A の段階に書いて Context Mapper CLI 6.12.0 の `lib/` の jar と OpenJDK 27 で動かしたもの（DESIGN 0.4 の出力はこれで取った）を、そのまま使ってよい。組み方は `javac -cp "<lib>/*" -d <一時ディレクトリ> Validate.java`、走らせ方は `java -cp "<lib>/*:<一時ディレクトリ>" Validate <file.cml>`。標準エラーに出る `WARNING: A terminally deprecated method in sun.misc.Unsafe has been called` などの警告（Context Mapper が使う guava が出す）は読み飛ばす。

```java
import java.util.List;
import org.contextmapper.dsl.ContextMappingDSLStandaloneSetup;
import org.eclipse.emf.common.util.URI;
import org.eclipse.emf.ecore.resource.Resource;
import org.eclipse.xtext.resource.XtextResource;
import org.eclipse.xtext.resource.XtextResourceSet;
import org.eclipse.xtext.util.CancelIndicator;
import org.eclipse.xtext.validation.CheckMode;
import org.eclipse.xtext.validation.IResourceValidator;
import org.eclipse.xtext.validation.Issue;
import com.google.inject.Injector;

public class Validate {
  public static void main(String[] args) {
    Injector injector = new ContextMappingDSLStandaloneSetup().createInjectorAndDoEMFRegistration();
    XtextResourceSet rs = injector.getInstance(XtextResourceSet.class);
    int errors = 0;
    for (String path : args) {
      Resource r = rs.getResource(URI.createFileURI(new java.io.File(path).getAbsolutePath()), true);
      IResourceValidator v = ((XtextResource) r).getResourceServiceProvider().getResourceValidator();
      List<Issue> issues = v.validate(r, CheckMode.ALL, CancelIndicator.NullImpl);
      for (Issue i : issues) {
        System.out.println(i.getSeverity() + " " + path + ":" + i.getLineNumber() + ":" + i.getColumn() + ": " + i.getMessage());
        if (i.getSeverity() == org.eclipse.xtext.diagnostics.Severity.ERROR) errors++;
      }
    }
    System.exit(errors > 0 ? 1 : 0);
  }
}
```

### C.13 突き合わせのテスト（`tests/imports.rs`）

**作った（C の段階）**：例と `tests/maps/入れ子/` の二つの地図で、ツールごとに七つの写しを並べて走らせる（四つのツールで 6 秒ほど）。違反の行の golden は `tests/golden/imports/<ツール>.txt`。DESIGN 7 章の設定の抜粋は例の設定のファイルから、ツールの出力は golden から取り、`tests/design.rs` が確かめる。

- C.7〜C.10 のテストを一つのファイルに置き、同じ一時ディレクトリの作り方と、同じ四つの変異を使う。どのツールも「例のままで通る」「四つの変異を捕まえる」「黙って通っていない」の三つを確かめる（DESIGN 7.6）。
- ツールの出力の一部（違反の行）を golden にし、DESIGN 7 章の sakai の設定の抜粋と、ツールの出力を、sakai が書いた設定で取り直したものに差し替える（`tests/design.rs` が確かめる）。

### C.14 CML（`src/cml.rs`、`tests/cml.rs`）

**作った（C の段階）**：golden は英語と日本語の二つ（`tests/golden/cml/通販.cml`、`通販.ja.cml`）。頭のコメントに SHA-256 は書かない（C.11 と同じ理由）。`as` で取り入れた語は `responsibilities` に入れない。

- DESIGN 8 章の表のとおり。頭のコメントに元の地図とハッシュ。コンテキストと関係は、地図に書いた順に出す。
- `sakai export cml`（DESIGN 6 章）。
- テスト：例の CML が golden（`tests/golden/cml/通販.cml`）と一致する。Java と Context Mapper の jar があれば、`tools/cml/Validate.java` を一時ディレクトリで組み、書いた CML で走らせて、何も出ない（exit 0）こと。わざと誤った CML（顧客／供給者に CF）を同じ検査器に通すと、`The CONFORMIST pattern is not applicable for a Customer-Supplier relationship.` が出ること（検査器が働いていることの確かめ）。無ければ SKIP。

### C.15 C の完了の条件

`cargo build` が警告なしで通り、`cargo test -- --nocapture` が全部通ること。この機械では、0.2 のとおりにツールを入れて環境変数を渡せば、SKIP は 0。そのうえで、次が成り立つこと。

| 対象 | 成り立つこと |
|---|---|
| 例の `check` | すべての言語をつないで exit 0。要約が「5 contexts, 7 relationships」で、境界を越える参照は 9 件：proto 1（`fulfillment.proto` → `stock.proto`）、rulec 2（`請求の要否.rule`、`出荷の送料.rule`）、koyomi 1（`出荷日.cal`）、dandori 5（`受注.flow` の `use rule … connect`、`use proto`、`connect` 二つ、`flow`）。sakai のクレートのバイナリでは E104 が三つ（rulec、koyomi、dandori） |
| 一式の言語 | C.2〜C.5 のテストの値が出る。C.0 の写したものが、それぞれの言語の検査を通る（口で確かめる） |
| 変異 | `PACKING_STATUS_DAMAGED` で E401、`ORDER_STATUS_RETURNED` で E105、値の名前を `キャンセル` にして E407、共有カーネルを片側消して E307 と E201、受注の用語集に違う意味の「引当」を足して E406、C.4 の四つ（E202、E207、E208、E209） |
| 四つのツール | 例のままで通り、四つの変異をそれぞれ捕まえ、黙って通っていない（C.13） |
| build | 四つの設定の golden と一致し、`--check` が地図の変更を E502 で言う |
| CML | golden と一致し、Context Mapper 6.12.0 の検査器が何も言わない |
| 台帳 | C のコードにも、変異と英語と日本語の golden と、`explain` の再現がある |
| 文書 | DESIGN の 3.1 と 5.3 に、例の地図の要約と三つの診断（E401、E202、E406）を足し、7 章の設定の抜粋と 8 章の CML を実物に差し替え（9 章の api は B で差し替えた）、`tests/design.rs` が確かめる |

報告には、四つのツールの版と、変異を捕まえたときの出力の一行ずつを書く。

**C の段階（C.0、C.6〜C.14）で満たしたもの**：四つのツール、build、CML、文書（7 章と 8 章、3.1 の例の要約、5.3 の E401 と E406）、台帳（E501 と E502 に変異と golden と再現）、変異のうち `PACKING_STATUS_DAMAGED` の E401、違う意味の「引当」の E406、片側の共有カーネルの E307、「C.0 の写したものが、それぞれのツールの検査を通る」。この機械では、ツールを入れて環境変数（`SAKAI_JAVA`、`SAKAI_JAVAC`、当時は `SAKAI_RULEC`、`SAKAI_KOYOMI`、`SAKAI_CHOBO`、`SAKAI_DANDORI` も）を渡せば SKIP は 0。

**ritsu の D.8 で満たしたもの**（一式の読み込み。5.4）：例の `check` の要約（9 件）、C.2〜C.5 のテストの値、変異のうち `ORDER_STATUS_RETURNED` の E105、値の名前を `キャンセル` にした E407、片側の共有カーネルで出る E201（前は E202 と書いていた。C.3）、C.4 の四つ、台帳の E104、E105、E405、E207、E208、E209 の再現と、退いた N101 の項、DESIGN 3.1 と 5.3 の例。この機械では、外のツールを入れて `SAKAI_JAVA` と `SAKAI_JAVAC` を渡せば SKIP は 0（一式の言語はテストが同じプロセスでつなぐ）。

## 4. 段階 D：doc、例、README、スキル

### D.1 doc（`src/doc/`）

- DESIGN 10 章の中身と順序。`mod.rs` がページの中身（文、表、図の材料）を言語ごとに組み、`markdown.rs` と `html.rs` がそれぞれの形で出す。`draw.rs` が HTML の SVG の図を描く（コンテキストを四角、関係を線、パターンの名前を線のラベル。dandori と chobo の `draw.rs` を手本に）。
- Markdown の図は Mermaid の `flowchart LR`。コンテキストは `<別名>["<名前>"]`、上流から下流への線にパターンの名前のラベル、共有カーネルとパートナーシップは `<-->`、別々の道は点線。
- `sakai doc` をコマンドの表に足す（DESIGN 6 章）。`--out <dir>` があれば、地図のファイルの名前から `<名前>.md` か `<名前>.html` を書く。
- テスト（`tests/doc.rs`）：例の二つの地図の Markdown（英語と日本語）と HTML が golden（`tests/golden/doc/`）と一致する。Mermaid（`tools/mermaid`）が図を描けること（chobo の `tests/doc.rs` と同じ形。無ければ SKIP）。Chrome で HTML を開き、画面を撮って、図のコンテキストの四角を押すとそのページに移ること（無ければ SKIP）。

### D.2 例の仕上げ

- 英語の地図 `examples/通販/shop.ctx` と `contexts/ordering.ctx` ほか五つを書く。同じ成果物を、英語の名前（`Ordering(ordering)` など。別名は日本語の地図と同じ）と英語の語で書く。規則やカレンダーの名前は、成果物のもの（日本語）のまま名指す。
- 例の README（`examples/通販/README.md` と `README.ja.md`）：コンテキストと関係の表、写した元、コードが手書きの代わりであること。
- テスト（`tests/examples.rs`）：二つの地図がどちらも `check` を通り、境界を越える参照が、コンテキストの名前を読み替えれば同じ。CML は、コメントと文字列を除けば一字も違わない（別名が同じなので）。C.0 の写したものが、それぞれのツールの検査を通る（ツールが無ければ SKIP）。

### D.3 docs/

- `docs/reference.md`（言語の全部。英語）、`docs/targets.md`（四つのツールの設定と、それぞれが捕まえるものの違い、CML への写し方。英語）、`docs/codes.md` と `docs/codes.ja.md`（`sakai explain --all --format markdown` の出力そのもの）。
- 事実（数、名前、決めたこと）は DESIGN から引いてよいが、言い回しは DESIGN から写さない（作者の決まり）。

### D.4 README.md と README.ja.md

- README.md は英語、README.ja.md は日本語で、英語の写しではなく一から書き起こす。koyomi と chobo の README の組み立て（何をするか、検査が言うこと、例、入れ方、コマンド、どう確かめているか、状態、ライセンス）にそろえる。
- 載せる `.ctx` の行、コマンドの出力、診断、設定の抜粋は、どれも実物。`tests/docs.rs` が、README と `docs/` とスキルに載せた `.ctx` の行が例か fixture の行であること、`$ sakai …` の出力が実際の出力と同じこと、診断が `check` の出力にあることを確かめる（koyomi の `tests/docs.rs` と同じ）。
- 入れ方は `https://github.com/i2y/sakai` を取ってきて `cargo install --path .`。
- dependency-cruiser と TypeScript の版の注意（TypeScript 7 では黙って通る）と、四つのツールで捕まえるものの違い（ArchUnit はクラスファイルの依存、ほかは import の文）を書く。
- 「どう確かめているか」の時間とテストの件数は、一度走らせたときのもので、テストは確かめない。比べた数（四つのツールの変異の数など）は確かめる。

### D.5 スキル（`skills/sakai/`）

- `SKILL.md` は手で書く（地図を書く流れ、言語の一枚の要約、人に聞くこと、診断ごとの直し方への案内）。ほかは `skills/sync.sh` が `docs/` から写す（koyomi と同じ）。frontmatter の `license` は `MIT OR Apache-2.0`。
- テスト（`tests/skill.rs`）：写しが `docs/` と同じ、スキルの中のリンクが外に出ない、frontmatter の形。

### D.6 ライセンス

- MIT OR Apache-2.0（作者が一式に決めた）。`Cargo.toml` の `license`、README.md の「License」と README.ja.md の「ライセンス」の節、スキルの frontmatter の `license` に書く（koyomi と chobo と同じ形）。写した一式の例のファイルは、どれも作者の一式のもので、同じライセンスである。

### D.7 D の完了の条件

`cargo build` が警告なしで通り、`cargo test -- --nocapture` が全部通ること。この機械では SKIP は 0。そのうえで、次が成り立つこと。

| 対象 | 成り立つこと |
|---|---|
| doc | 例の二つの地図の Markdown（英語、日本語）と HTML が golden と一致。Mermaid が図を描き、Chrome で開いた HTML の図からコンテキストのページに移れる |
| 例 | 二つの地図が `check` を通り、参照が名前の読み替えで同じ、CML がコメントと文字列のほかは同じ |
| README と docs | `tests/docs.rs` が通る（`.ctx` の行、出力、診断、設定の抜粋が実物） |
| スキル | `tests/skill.rs` が通る |
| 文書 | DESIGN のスケッチが残っていない（`grep -n スケッチ DESIGN.md` が、スケッチを差し替えたと書いた段落のほかに当たらない） |

## 5. 次の段階への申し送り

### 5.1 A から B へ（A の段階で書いた）

- 外のツールの振る舞い（DESIGN 0.4、7 章、8 章）は、2026-10-03 にこの機械で確かめた。版を変えたら確かめ直す。とくに、dependency-cruiser と TypeScript の組み合わせ（黙って通る）、Context Mapper の CLI の `validate` が構文しか見ないこと、ArchUnit の `failOnEmptyShould`、import-linter が名前空間の一部をルートに受け付けること。
- C.0 の下書きは、A の段階で一式のツールに通した。直し方の表のとおりに写せば同じものになる。
- dandori の api は、作者が決めるまで無いものとして作る（C.4）。
- 作者が決めるべきだったかもしれないこと（A の報告で挙げたもの）：看板の言い方、二種類のファイルと下流が関係を書く形、`through` と役割を必ず書かせること、dandori に api を足すかどうか、名指し方のうち `dir` と `sakai` の二つのツールの語と、JSON のパスの基点。作者の返事で変わったら、DESIGN と、この計画の該当の項を直してから進める。

### 5.2 B から C へ（B の段階で書いた）

B で決めて、DESIGN と、この計画の B の項を直したこと：

- 名指しは yuen との決着どおり（DESIGN 2 章）。JSON と診断のパスはルートからの相対で、`check` と `api` は `--root` を取る。絶対パスとルートの外に出るパスは、新しいコード E012。（C の段階で、診断の文面のファイルの場所は走らせたディレクトリから書くことにした。5.3）
- 段の止め方（DESIGN 3.1）：段 1 と段 2 のエラーは後の段を止め、段 3 から後はどの段も走らせる。地図が名指す要素（`means`、対応の列挙と先）は、段 3 で proto を読んだあとに引く（E007、E011、範囲の外の proto は E103）。要素を一つでも引けなかった地図では、W401 を出さない（何が越えるかが分からないので）。
- proto の名前の表は、同じ完全な名前を二つのファイルが持てる（共有カーネルの写し）。型は、名指したファイルから見えるほうに解決する。
- `check` にディレクトリを渡すと、地図が読む `.ctx` は地図を通して言い、どの地図にも読まれない context のファイルに W103、map でも context でもない `.ctx` に読んだときの診断（E003 など）を出す。

C で作るときに気をつけること：

- 対応の先が rulec の列挙のとき、B の `mapping.rs` は、その対応の検査を全部飛ばしている（`Some(n) if n.tool == Tool::Rulec => continue`）。C.2 で、`rulec api` の `connect.enums` を読んでから値の行の有無と E405 を確かめる。同じく `means` の先の規則の要素（`input`、`output`、`enum`、`value`）は、B では書いたとおりに受け取っている（`elements.rs`）。rulec の公表された言語の塊の package とサービスの突き合わせ（E302、E301）も C.2 で足す。B が確かめているのは、規則のファイルがそのコンテキストのものであること（E302）だけである。
- `refs::Crossing` は proto の import のための形で、`tool()` は proto を返し、api の `via` は `proto import` と決め打ちしている。rulec と koyomi の参照を足すときは、もとのツールと、読んだ api のフィールド（`rulec api connect.enums[0].contract` など）を `Crossing` に持たせ、行の無い参照は `Diag::file` で位置をファイルまでにする（DESIGN 5.1）。
- `check` の要約（`check::summary`）は、括弧の中に proto の数だけを書いている。C で `rulec <n>`、`koyomi <n>` を足し、N101 を出す。api の `not_checked` は B では空である。
- 台帳の E104、E105、N101、E405（と E501、E502）は、見出しと説明と直し方だけで、再現が無い。作ったら `Entry::files` に再現を書き、`tests/codes.rs` の `the_codes_of_stage_b_are_printed` の「まだのコード」から外す。`check --help` の「出しうる診断」は、再現のあるコードから作るので、ひとりでに増える。
- DESIGN 3.1 の要約と 5.3 の診断は、B の地図（`tests/maps/基本/`）の実物を貼ってある。例（`examples/通販/`）を作ったら、例の地図の要約と診断を足す（`tests/design.rs` が、`$ sakai` の塊を走らせ、診断の塊を golden と突き合わせる）。`check --help` の例も、いまは `tests/maps/基本/基本.ctx` を指している。
- `tests/mapping.rs` の `a_rule_as_the_target_is_not_read_until_stage_c` は、中身が一行だけの規則（`rule 請求の要否(billing_need) v1`）を対応の先に置き、B が何も言わないことを確かめている。C で `rulec api` を呼ぶようになると、この規則は rulec の検査を通らず E105 になるので、このテストは C の振る舞い（rulec の対応を読む）を確かめるものに書き換える。
- 変異を足すときは、`tests/mutants/<コード>_<内容>/` に、土台の名前を書いた `base` と変えたファイルを置く（B.10）。golden は `SAKAI_BLESS=1 cargo test --test mutants` で書き、差分を読む。一つの変更で二つのコードが出るときは、その変異の `README.md` に理由を書く。

### 5.3 C から D へ（C の段階で書いた）

C の段階では、C.0 と C.6〜C.14 を作った。C.1〜C.5（一式のツールの読み込み）は、一式の言語を一つの処理系にまとめるかを作者が決めるまで止めてある（C.1 の頭）。

C で決めて、DESIGN と、この計画の該当の項を直したこと：

- 名指しの細かい形（naming.md の追記 8）：文字列の外の全角の空白、空のパス（E012）。`tests/fixtures/naming.tsv` は 36 行で、`tests/naming.rs` は誤りの行が表の理由のとおりに断られることも確かめる。
- 診断の文面のパス（naming.md の追記 9 と 10）：ファイルの場所は走らせたディレクトリから、渡したパスと同じ書き方で書き、名指しはルートからの相対のまま書く（DESIGN 2.4）。`main.rs` が `paths::show_from` で一度だけ決め、ライブラリとテストは決めないので、ルートで走らせたのと同じ形になる。変異の golden は変わらず（E009 の注の言い回しだけ直した）、`tests/cli.rs` がルートの上、下、絶対パスの三つを確かめる。
- W401 は、sakai がまだ参照を読まない成果物（規則、dandori のワークフロー）を地図が含むあいだ出さない（DESIGN 1.6。`terms::unread_references`）。例の請求の「キャンセル」が、規則を通って越えるのに W401 になっていたため。C.2 を作ったら、規則をこの条件から外す。
- import のまとまりの決まり（DESIGN 7.1）、import-linter の書き方（C.7）、dependency-cruiser の否定先読み（C.8）、go-arch-lint の自分への依存（C.10）、設定の頭にハッシュを書かないことと golden の置き場所（C.11）。
- 台帳の項目は、再現を走らせるコマンドを持てる（`Entry::command`。既定は `check .`）。変異も、ファイル `command` があればそのコマンドで走らせる（`tests/mutants.rs`）。

D で作るときに気をつけること：

- 例の英語の地図 `shop.ctx` を書いたら、同じコードの置き場所に設定を書くことになる。設定は `--lang` の言語で説明を書くので、二つの地図で同じ場所に書くと、どちらかの `--check` が E502 になる。どちらの地図の設定を例に置くかを決める（いまは日本語の地図の、`--lang ja` の設定）。
- doc の対応の表で、rulec の規則が先の対応（請求の腐敗防止層）は、いまは値を読めない（C.2 待ち）。「rulec の対応はまだ読んでいない」と出す。
- README に書くこと：dependency-cruiser と TypeScript の版の注意、四つのツールで捕まえるものの違い（ArchUnit はクラスファイルの依存、ほかは import の文）、`--output-type err` を CI で使うこと、import-linter が名前空間のディレクトリを読む決まり、`--check` は書いたときと同じ `--lang` で走らせること。
- `tests/maps/入れ子/` は、`check tests/maps` の結果にも並ぶ（`tests/cli.rs`）。

C.1〜C.5 を作るときに気をつけること（5.2 の申し送りに足す）：

- 例の要約に rulec 2 と koyomi 1 が足され、N101 が出ると、`tests/examples.rs` の要約の行と、DESIGN 3.1 と 11 章の数を直す。
- 例をもとにした変異（`E401_梱包の状態に値が増えた`、`E406_受注に違う意味の引当`）は、一式のツールを呼ぶようになると、ツールが無い機械で E104 を出すようになる。そのときの変異とテストの扱い（ツールが無ければ SKIP にするか、一式のツールを使わない地図に移すか）を決める。
- `tests/build.rs` と `tests/imports.rs` と `tests/cml.rs` も例の `check` を通すので、同じことが起きる。

### 5.4 ritsu の D.8 で（一式の読み込み）

sakai が ritsu に取り込まれ、一式の言語を一つの処理系にまとめると決まったので、止めていた C.1〜C.5 を口で作るように書き直し、そのとおりに作った（ritsu の PLAN の D.8）。DESIGN の 4.1、4.7、12.2 に、読むもの、決めたこと、変わった振る舞いを書いた。

決めたこと：

- E104 は「ほかの言語がつながっていない」、E105 は「成果物が、その言語の検査を通らないか、読めない」に意味を替えた。E104 は言語ごとに一度、その言語の最初の成果物を持つ `owns` の行で言う。N101 は退かせた。
- 子の `.flow` を境界の向こうから走らせてよいのは、パートナーシップ、共有カーネル、子が相手の公開ホストサービスを実装しているときだけ（E209。DESIGN 4.7）。
- 規則の同梱、Lambda、ローカル、`apply` は、規則そのものを使う参照で、共有カーネルの中でなければ E202。`use openapi` と `use smithy` の記述、JSON Schema、出典の写しは、それを読む成果物の一部で、参照に数えない（DESIGN 3.3）。
- koyomi と dandori は、構文を読めるファイルに参照を答える（rulec は検査を通る規則にだけ事実を答える）。
- sakai のクレートのバイナリは、地図が規則、カレンダー、ワークフローを含むと、`check`、`api`、`build`、`export` のどれでも E104 で止まる。
- api の `not_checked` は、いつも空のまま残した（ritsu の段階 E で消した。DESIGN 9 章）。

5.3 の申し送りの「C.1〜C.5 を作るときに気をつけること」は、次のとおりにした。

- 例の要約と、DESIGN 3.1 と 11 章の数を直した（9 件。`tests/examples.rs`、`tests/design.rs`）。
- 例を土台にした変異とテスト（`tests/build.rs`、`tests/imports.rs`、`tests/cml.rs`）は、ツールの有無で変わらない。一式の言語は `[dev-dependencies]` で、テストは `tests/common/mod.rs` の `suite()` で同じプロセスにつなぐからで、SKIP にすることも、地図を移すことも要らなかった。

変異は、例を土台に九つ足した（`E104_ほかの言語がつながっていない`、`E105_取り込みが合わない規則`、`E202_境界の向こうの規則を同梱`、`E202_境界の向こうの規則を展開`、`E207_公開していないサービスを呼ぶ`、`E208_公開していないサービスを実装`、`E209_境界の向こうの子のフロー`、`E307_片側だけの共有カーネルとカレンダー`、`E405_規則と違う対応`）。B と C の変異の golden は、一字も変わらない。
