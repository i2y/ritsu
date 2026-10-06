# yuen 実装計画

DESIGN.md を仕様として、yuen を三つの段階（B・C・D）で作る。どの段階も、ここに書いた順に進め、各段階の最後にある完了の条件のテストが全部通ったら終わりにする。DESIGN.md と違うことをしたくなったら、先に DESIGN.md に決定・理由・捨てたものを書き、報告で言う。

この計画を書いた A の段階では、本体のコードは書いていない。リポジトリに置いたのは DESIGN.md とこの PLAN.md だけである（LICENSE-MIT と LICENSE-APACHE は前からある）。端のハッシュの値は、DESIGN の定義どおりに組んだ使い捨ての Python の試作で計算し、DESIGN 1.1・3.2・4.1・19 章と、下の完了の条件に転記した。試作はリポジトリに残していない。B と C の yuen が同じ値を出すことを、完了の条件にした。

この計画は、yuen が yurai という名前で自分のリポジトリにあったときに書いた。いまは ritsu の `crates/yuen` にあり、ritsu の段階 C の最初に名前を yuen に改めた（ritsu の DESIGN.md 2.2）。0 章の決まりと機械の扱いのうち、ritsu の PLAN.md と食い違うところは ritsu の PLAN.md に従う。C の残り（一式の読み込み）は、ritsu の口で作るように書き直し（下の C.1〜C.9）、ritsu の PLAN の D.7 で作った。D は ritsu の PLAN の F.1 で書き直して作った（4 章）。

## 0. 全部の段階に共通の決まり

### 0.1 守ること

- 書き方の決まり（`japanese-style`、`private-hobby`、`no-quoting-prompts`、`write-from-real-runs`、`features-are-first-class`、`do-the-whole-job`、`shell-gotchas`、`name-the-feature`、`user-profile`）を先に読み、従う。日本語（DESIGN.md、`--lang ja` の診断、README.ja.md、報告）は、英語の概念語を漢字に直訳しない。「道具」ではなく「ツール」、「原本」ではなく「出典」か「元」と書き、条文から引いた部分は「条」か「引用箇所」と書く。物に「たち」を付けない。
- git のコミットと push をしない。書くのは ritsu の木の中だけにする。元のリポジトリ（前の名前の `~/yurai` と、`~/rulec`・`~/dandori`・`~/koyomi`・`~/chobo`・`~/geas`・`~/sakai`）は読むだけで、そこでビルドも git もしない。
- Rust は edition 2024 で、手元の stable 1.94.1 で通すこと。依存は `serde_json = { version = "1", features = ["preserve_order"] }` だけ。
- 診断は英語が既定で、`--lang ja` で日本語。golden は両方の言語で取る。
- テストは `cargo test`。外のツール（xmllint とスキーマ、Python の venv、curl、Chrome）が無いときは、`SKIP: <理由>` を一行出して通す。ほかの言語（rulec、koyomi、chobo、geas、dandori、sakai）は、ritsu のワークスペースのクレートを dev-dependency に持ち、同じプロセスの中でつなぐので SKIP しない（ritsu の DESIGN 3.3）。報告の前に `cargo test -- --nocapture 2>&1 | grep SKIP` で SKIP の行を読み、数を報告に書く。
- golden の取り直しは `YUEN_BLESS=1 cargo test`。取り直したら差分を読んでから報告する。
- サーバー（HTTP、Chrome）を立てたまま終わらない。テストは止める処理を Drop に置き、一時ディレクトリを消す。
- 成果物（文書、golden、生成物、例の `.req` と `reviewed/`）に、手元の絶対パス、ユーザー名、マシン名を入れない。
- 文書に載せる出力と数は、実際に走らせたものを貼る。DESIGN の「形の案」は、その段階で実物に差し替える。

### 0.2 作者の機械で気をつけること（macOS arm64）

- （取り込む前の手順。ritsu の中では、ほかの言語はワークスペースのクレートで、テストがつなぐ）一式のツールのバイナリ：rulec 0.22.1 は作業場所の `bin/rulec` にコピーしてある。koyomi・chobo・geas は `cargo install --locked --path ~/<名前> --root <作業場所>/yuen/tools --target-dir <作業場所>/yuen/target-tools` で入れる。**`--target-dir` を必ず付ける**。`cargo install --path` は、`--target-dir` が無ければ、そのクレートの木の `target/` でビルドするので、読むだけのはずの木に書くことになる。テストには `YUEN_RULEC`・`YUEN_KOYOMI`・`YUEN_CHOBO`・`YUEN_GEAS` で場所を渡す。
- macOS には `timeout` コマンドが無い。子プロセスの時間切れは、テストの Rust の側で `Child::try_wait` を回して決め、超えたら kill する。
- Python の venv は `uv venv --python 3.13 <場所>`。Homebrew の 3.14 の venv には pip が入らない。`prov==3.2.2` と `reqif==0.1.0` を入れる（`tools/requirements.txt`。A の段階で、この組み合わせが Python 3.13.11 で動くことを確かめた）。venv の `python` はシンボリックリンクなので、テストはリンクのまま渡す（たどると venv の外の Python になり、`prov` が見えない）。
- ReqIF のスキーマは `tools/reqif/fetch.sh [<dir>]` で取る。既定の置き場所は `tools/reqif/xsd`（git に入れない）で、ほかの場所に置いたらテストに `YUEN_REQIF_XSD=<dir>` で渡す。C の段階では、venv とスキーマを作業場所に置いて `YUEN_PYTHON` と `YUEN_REQIF_XSD` で渡し、終わったら消した。
- xmllint は `/usr/bin/xmllint`（libxml 2.9.13）。スキーマを読み込むたびに、同じ名前空間を二度読み込もうとした警告（`Skipping import of schema …`）を多く出すが、検証の結果には関わらない。テストは結果の行（`validates` か `fails to validate`）と exit code を見る。
- Chrome は `YUEN_CHROME`、無ければ `/Applications/Google Chrome.app`、それも無ければ PATH の `google-chrome` か `chromium`（koyomi と chobo と同じ順）。`--headless` は書き出したあとも終わらないことがあるので、時間を区切って kill し、`--user-data-dir` に一時ディレクトリを渡して、終わったら消す。
- zsh で変数名に `path` を使わない。コマンドや Write で、バックスラッシュと `u` に 16 進 4 桁を続けた形を書くと文字に変わるので、エスケープを文字どおりに書くときは Python で `chr(92)` から組み立てる。
- ディスクの空きは 48 GB ほど。ビルドの中間物や大きな一時ファイルを残さない。

### 0.3 手本にしてよいもの（読むだけ）

- koyomi：`src/i18n.rs`（`tr!` が文の組を返す）、`src/diag.rs`（診断の形と JSON）、`src/codes.rs`（台帳と再現）、`src/cli.rs`（コマンドとフラグの表）、`src/sha256.rs`、`src/sources.rs`（コピーと固定、`fetch`・`pin`・`outdated`、`curl` の呼び方、e-Gov の要素の名前、`xml_text` と `article_lines`）、`src/doc/`（Markdown と HTML のページ）、`tests/docs.rs` と `tests/skill.rs`。
- rulec：`src/sources.rs`（e-Gov と eCFR から条を取ってきて保存するやり方。`egov_fragment`、eCFR の `title`・`part`・`section`、附則）。
- dandori：`src/proto.rs`（`.proto` の読み手と `import` の探し方）。
- geas：DESIGN の 7 章と PLAN の付録 A（記録と `affected --json` の形）。
- chobo：`src/syntax.rs`（キーワードの表の置き方）。

コピーするときは yuen の DESIGN に合わせて書き直す。

### 0.4 段階をまたぐ約束

次のものは、どの段階でも DESIGN のとおりにし、変えるときは DESIGN を先に直す。

- **成果物の名前**（DESIGN 2 章）：形、パス（書いたファイルからの相対、JSON ではルートからの相対）、ツールと種類の語、名前の書き方、同じかどうか・含むかどうか、JSON の形。2026-10-03 に sakai と突き合わせて決着した（DESIGN 2.8）。試しの表 `tests/fixtures/naming.tsv` は sakai のリポジトリと同じもので、直すときは二つで同じに直す。変えるなら報告で言う。
- **端の中身**（DESIGN 3.2、4.1）：バイト列まで決めてある。要件の端の中身、決まった形の JSON、proto の決まった形の文。B の段階の値と C の段階の値が、同じ要件について同じでなければならない（借りた出典でも、自分で保存した出典でも）。
- **確かめた記録の行**（DESIGN 4.2）：`reviewed <日付> by <役割> sha256:<16>[, sha256:<16>…] -> sha256:<16>` と `approved <日付> by <役割> sha256:<16>`。
- **確かめたときの中身**（DESIGN 4.4）：`.req` の隣の `reviewed/<16 桁>`。中身はその端の中身のバイト列そのもの。
- **診断のコード**（DESIGN 6.2）：番号と意味。増やすときは番台の末尾に。
- **コマンドの表**（DESIGN 7 章）：まだ作っていないコマンドは、表に載せない（その段階で足す）。
- **api の JSON**（DESIGN 11 章）：キーの名前と順。
- **環境変数**：`YUEN_LANG`、`YUEN_BLESS`、`YUEN_PYTHON`（`prov` と `reqif` の入った Python）、`YUEN_XMLLINT`（無ければ PATH の `xmllint`）、`YUEN_REQIF_XSD`（`tools/reqif/fetch.sh` が取ったスキーマとカタログの置き場所。無ければ `tools/reqif/xsd`）、`YUEN_EGOV`、`YUEN_ECFR`、`YUEN_NET`、`YUEN_CHROME`。ほかの言語を子プロセスで呼ぶために予定していた `YUEN_RULEC`・`YUEN_KOYOMI`・`YUEN_CHOBO`・`YUEN_GEAS`・`YUEN_SAKAI` は、口で読むようになったので作らない（ritsu の DESIGN 8.2）。

## 1. ディレクトリ

```
Cargo.toml  .gitignore（/target、/tools/.venv、/tools/reqif/xsd）
src/
  main.rs  lib.rs
  cli.rs           コマンドとフラグの表、--help、引数の読み取り（B）
  i18n.rs          Lang と tr!（文の組を返す）（B）
  kw.rs            キーワードの表（DESIGN 1.2）（B）
  lex.rs  parse.rs  ast.rs                                        （B）
  names.rs         成果物の名前の形、パスの畳み方、ツールと種類の表（DESIGN 2 章）（B）
  project.rs       .req の集まり、ルート、要件と版と別名、役割、出典、範囲の名前の表（B）
  date.rs          日付と期間、review の「その日」（B）
  sha256.rs  base64.rs                                            （B）
  copies.rs        法令のコピーの置き場所、xml_text と article_lines、e-Gov と eCFR の要素の名前（B）
  sources.rs       出典のコピーと固定と引用の検査（E101〜E105、W101）（B）
  ends.rs          端の中身とハッシュ：要件、出典の条、file の出典、file の成果物（B）。ツールの成果物（C）
  graph.rs         リンク、循環、期間、置き換え（B）
  marks.rs         確かめた記録と今のハッシュ、印の理由、並べ方（B）
  diff.rs          行の LCS と統一形式（B）
  coverage.rs      カバレッジ、見送り、範囲（B。geas の記録を通る道は C）
  check.rs         検査の七つの段と一行の結果（B）
  review.rs        確かめた記録を書く、reviewed/ を書いて片づける（B）
  trace.rs         なぜこうなっているかをたどる（B。成果物が固定している条は C）
  diag.rs  codes.rs                                               （B）
  api.rs                                                          （B）
  suite.rs         ほかの言語の口（Items、Sources、Rules、Dates、Claims）と、一回の実行で覚えた答え（C。ritsu の D.7）
  run.rs           コマンドを関数にしたもの（main.rs と ritsu yuen が呼ぶ）（C）
  ports.rs         yuen が ritsu の口に答える（ritsu の D.2。D.7 で、つないだ言語を持つ）
  proto.rs         .proto の端の文（ritsu-proto で読む）（C）
  sources.rs       借りた出典、成果物が固定している条、E106 と E107（C。B の出典の検査と同じファイル）
  affected.rs      差分 → 主張と要件（差分は ritsu-base の udiff で読む）（C）
  export/  mod.rs（二つの書き出しが読むグラフ、識別子のハッシュ） reqif.rs prov.rs   （C）
  fetch.rs         source fetch・pin・outdated、curl の呼び方（C）
  doc/  mod.rs markdown.rs html.rs                                 （D）
tests/
  common/mod.rs（一時ディレクトリ、yuen を走らせる、golden、B.7 の変更）  （B。ツールの場所と SKIP は C）
  syntax.rs project.rs names.rs copies.rs ends.rs marks.rs periods.rs coverage.rs review.rs trace.rs mutants.rs codes.rs cli.rs api.rs design.rs   （B）
  suite.rs（C.1〜C.9 と dandori、sakai） ports.rs export.rs fetch.rs   （C）
  doc.rs docs.rs skill.rs examples.rs                            （D）
  fixtures/（period/、ecfr/、payment/。C で rulec/、koyomi/、chobo/、geas/、proto/、dandori/、sakai/）  mutants/  golden/（marks/、trace/、api/、export/、ends/、affected/ を含む）
tools/requirements.txt（prov==3.2.2、reqif==0.1.0）  tools/reqif/fetch.sh  tools/reqif/README.md   （C）
examples/<例>/                                                    （D）
docs/  skills/  README.md  README.ja.md  THIRD_PARTY_NOTICES.md   （D）
```

## 2. 段階 B：言語の芯

字句・構文・型、プロジェクトと名前、出典のコピーと固定、端の中身とハッシュ、グラフ（循環、期間）、確かめた記録と印、カバレッジと範囲、`review`、`trace`、診断（英語と日本語）、CLI の `check`・`review`・`trace`・`api`・`explain`。成果物は `file` だけを読む（一式のツールを読むのは C）。項の順に進め、各項のテストはその項のうちに書く。

### B.1 土台

- `Cargo.toml`（name `yuen`、edition 2024、`license = "MIT OR Apache-2.0"`、`repository = "https://github.com/i2y/yuen"`、依存は serde_json だけ）と `.gitignore`。lib と bin の両方を持つ。
- `src/i18n.rs`：`Lang { En, Ja }`。`--lang` があればそれ、無ければ `YUEN_LANG`、無ければ英語。システムのロケールは見ない。`tr!("日本語", "English")` は `Text { ja, en }` を返し、描くときに `Lang` を渡す（koyomi と同じ）。
- `src/kw.rs`：DESIGN 1.2 の表を一枚で持つ。予約語の判定と `explain` と、D の `docs/reference.md` のキーワードの表は、ここから引く。

### B.2 字句と構文（`src/lex.rs`、`src/parse.rs`、`src/ast.rs`）

- 字句：名前（Unicode の文字・数字・`_`）、別名 `(ascii)`、版 `v<n>`、文字列（`\"` と `\\`）、日付 `YYYY-MM-DD`、期間（`<日付>..<日付>`、`<日付>..`、`..<日付>`）、`sha256:<16 桁>`、`@`、`,`、`->`、`=`、`#` から行末までのコメント。どの字句も行と列を持つ（列は文字で数える）。
- 構文：DESIGN 1.1 の節の順（見出し、`description`、`role`、`source`、`scope`、`requirement`）、`source` の三つの形と固定の行、`scope`、`requirement` のブロックの行の順（DESIGN 1.5 の表）、リンクと見送りと、その下の記録の行。成果物の名前（DESIGN 2.1、2.4）は、ツール名、パスの文字列、種類と名前の組（語か文字列）で読む。字下げはスペース。
- E001〜E006 を出す。
- テスト（`tests/syntax.rs`）：DESIGN に出てくる `.req` の塊が全部構文を通る（1.1 はそのまま。1.4〜1.8 の抜き出しは、見出しを足したファイルにして）。E001〜E006 の変異ファイルが、そのコードを出す。

### B.3 プロジェクトと名前（`src/project.rs`、`src/names.rs`）

- プロジェクト：`yuen <コマンド> <path>...` に渡したファイルとディレクトリ（下の `.req` を全部、パスの順に）を一つのプロジェクトとして読む。
- ルート（DESIGN 2.2）：最初に渡したパスの上で `.git` を持つ一番近いディレクトリ、無ければ渡したディレクトリ（ファイルならそのディレクトリ）。`--root` で替える。git は走らせない。
- 名前の表：要件と版と別名、役割、出典（ファイルごと）、ファイルの見出し。E007〜E010。別名の決まり（DESIGN 1.2）。版の決まり（DESIGN 1.5）。
- 成果物の名前（DESIGN 2 章）：パスを `.req` のディレクトリから読み、`.` と `..` を字の上で畳み、ルートからの相対にする。絶対パスとルートの外は E013。ツール名（E011）と、ツールごとの種類の語と組の並び（E012。2.3 の表を `src/names.rs` に一枚で持つ。dandori の種類、`source` を借りた出典の外に書いたもの、`method` を `service` の外に書いたものも E012）。名前の文字の形（DESIGN 2.4）と JSON の形（2.6）。同じかどうか、含むかどうか（2.5）。
- `verified by` に書ける種類（DESIGN 1.6 の表）。違えば E403。
- テスト（`tests/project.rs`、`tests/names.rs`）：E007〜E013 と E403 の変異。名前の文字の形が、2.4 の決まりで往復する（語と文字列、`\"` を含む名前）。パスの畳み方（`../x/./y.rule`）。ルートの探し方（一時ディレクトリに `.git` のディレクトリを作って）。sakai と共有する `tests/fixtures/naming.tsv` の全行が、表のとおりの JSON かエラー（理由に当たるコード）になる（DESIGN 2.6）。

### B.4 出典のコピーと固定（`src/sha256.rs`、`src/base64.rs`、`src/copies.rs`）

- SHA-256 を自前で書く。FIPS 180-4 の既知の値でテストする（空 → `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`、`abc` → `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`、`abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq` → `248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1`、`a` を 1,000,000 個 → `cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0`。koyomi の PLAN B.5 と同じ値）。base64 は C の `source fetch` が使うが、ここで書いてテストする。
- `law` の出典（e-Gov と eCFR）：条の名前から要素の名前とコピーのファイル名を作る（rulec の `egov_fragment` と同じ作り方。`第143条第2項` → `MainProvision-Article_143-Paragraph_2`、`別表第一` → `AppdxTable_1`、附則。eCFR は `"§1910.157"` → `1910.157.xml`、ディレクトリは `29-CFR-1910@<日付>`）。コピーは `.req` の隣の `sources/law/…`。
- `file` の出典：パス、`url`、固定。
- 引用 `@<出典> <条>[, <条>…]`：E105（書き方、宣言の無い出典、`file` の出典に条を書いた）。
- コピーと固定：E101〜E104、W101（検査は `src/sources.rs`、置き場所と本文は `src/copies.rs`）。コピーの XML からタグを落として本文にする関数（`xml_text`。文ごとに一行。rulec の `xml_text` と同じ）と、項ごとに一行の `article_lines`（koyomi と同じ。D の doc と `trace` が使う）。E104 は、UTF-8 の XML で、最初の要素が条の要素（`Article`、`Paragraph`、`Item`、`AppdxTable`、`SupplProvision`、eCFR は `DIV8`）のときに通す。
- 借りた出典（`<ツール> "<パス>" source <名前>`）は、ここでは構文と名前だけを読む。解くのは C（C.7）。
- テストの材料：`tests/fixtures/period/`（下の B.14）に、koyomi の `examples/sources/law/129AC0000000089@2026-10-01/` の四つの XML と `revision.txt` をコピーする（取り直さない）。
- テスト（`tests/copies.rs`）：四つのコピーの固定が、140 条 `e880059021fbb67d`、141 条 `0575c131b9f08063`、142 条 `fc8c35a0769d3b35`、143 条 `6950bdfb988439b6` になる。142 条の `xml_text` が「期間の末日が日曜日、国民の祝日に関する法律（昭和二十三年法律第百七十八号）に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する。」を含む。条の名前からファイル名を作る決まり。E101〜E105 と W101 の変異。

### B.5 成果物（`file` だけ）と端の中身（`src/ends.rs`）

- `file "<path>"` の成果物：ファイルがあるか（E201）、端の中身はバイト列、ハッシュは SHA-256 の先頭 16 桁。
- 出典の条の端：コピーのバイト列（ハッシュは固定と同じ値）。`file` の出典の端：ファイルのバイト列。
- 要件の端：DESIGN 4.1 の形を一字一句。`text` の行、`from` の行（自分の `law` の出典は `from law <db> <ID> <条> sha256:<固定>`、`file` の出典は `from file <ルートからのパス> sha256:<固定>`、元になった要件は `from requirement <名前> v<n> sha256:<端>`）、`in force` の行。二行めから下を UTF-8 のバイト列の順に並べ、どの行も LF で終える。元になった要件の端を先に計算する（循環は B.6 で止める）。
- ほかのツールの成果物は、名前の検査（B.3）までで止め、端を作らない（C で作る）。B の段階のテストは、ほかのツールの成果物を使わない。B の yuen は、名前の検査を通ったプロジェクトがほかのツールの成果物、範囲、借りた出典を指していれば、「yuen はまだ koyomi の成果物を読めません」と言って exit 2 にする（`src/check.rs` の `not_yet`）。確かめないまま通すことはしない（DESIGN 2.3）。C はここをツールの読み方に差し替える。
- テスト（`tests/ends.rs`）：B.14 の `period` の要件の端が、A の段階の試作と同じ値になる。`起算日` `a9ebc73907faddc8`、`満了日` `465b83ed8c251406`、`満了日_142条` `d4f2d2a67322df17`。`file "民法の期間.cal"` の端が `c9b94eecde23e6b5`。`起算日` の端の中身が DESIGN 4.1 の 3 行と一字一句同じ。

### B.6 グラフ、循環、期間（`src/graph.rs`）

- リンク（DESIGN 4.1 の表）：`from`（出典の条、元になった要件）、`satisfied by`、`verified by`、見送り。
- 循環（E405）：`from <要件>` と `replaces <要件>` の辺。循環の要件を、ファイルの位置と一緒に順に並べる。
- 期間（DESIGN 5.5）：E406（隙間の日を並べる）、E407（重なる日と二つの版）、E408、E409。日付は 0001-01-01〜9999-12-31 の暦日（koyomi の `days_from_civil` と `civil_from_days` の手順で通算日にする）。
- テスト（`tests/periods.rs`）：隙間が一日（2027-04-01 だけ）、重なりが三日、終わりを開けた版が最後でない、版の番号が期間の順でない、置き換えの始まりが一日遅い、分ける置き換えとまとめる置き換えが通る、の変異。循環が二つの要件と三つの要件のとき。

### B.7 確かめた記録と印（`src/marks.rs`、`src/diff.rs`）

- 記録の行（DESIGN 4.2）を読む。ハッシュの数がリンク元の数と合わない、形が崩れている、は E305。
- 比べ方（DESIGN 4.3）：E301〜E304。理由（出典の条が変わった、要件の文か期間が変わった、要件のリンク元だけが変わった、成果物が変わった）と、それぞれの差分。差分は `reviewed/` の中身と今の端の中身から作る。法令のコピーは `xml_text` の行で、ほかはバイト列を UTF-8 の行で比べる（LCS、統一形式、前後二行、40 行まで）。中身が無ければ W301。UTF-8 でない中身と 1 MiB を超える中身は、ハッシュと大きさだけを言う。
- 並べ方（DESIGN 4.3）：変わったものごとにまとめ、出典、要件、成果物の順。まとまりの中は `from` のつながりの順、同じならファイルの行の順。一本のリンクは、最初に当たったまとまりで一度だけ言う。同じ成果物の変更が何本ものリンクに出るとき、差分は最初の一本だけに載せ、ほかは「同じ変更」と言う。
- テスト（`tests/marks.rs`）：B.14 の `period` のコピーで、次の変異を golden にする（英語と日本語）。
  1. 142 条のコピーの本文を一文字変え、固定の行をそのコピーのハッシュに書き換える：E302 が二つ（`from @民法 第142条` と、`満了日_142条` の `satisfied by`）と E304 が一つ（`満了日_142条` の見送り）。最初の E302 に本文の差分。
  2. `満了日` の `text` を一文字変える：その `from` が E303（要件の端の中身の差分）、`satisfied by` が E302、見送りが E304。
  3. `民法の期間.cal` の一行を変える：三本の `satisfied by` が E303。差分は一本めだけ。
  4. 記録の行を一つ消す：E301。承認の行を一つ消す：E304。
  5. `reviewed/` を消してから 1 を当てる：同じ印に W301 が加わり、差分は出ない。

### B.8 カバレッジと範囲（`src/coverage.rs`）

- DESIGN 5.2：E401、E402、W401。版ごとに数える。
- DESIGN 5.3：範囲の成果物を集める（`scope file "<path>"` のファイルとディレクトリ。除くディレクトリは DESIGN 1.8）。辿れるかの 1〜3（指されている、含むものが指されている、含まれるものが指されている）。4（geas の記録を通る）は C。E404。
- テスト（`tests/coverage.rs`）：満たすもの・確かめるものが無い、見送りとリンクの両方がある、範囲のファイルを指すリンクが無い、ディレクトリの範囲で一つだけ辿れない、の変異。

### B.9 検査（`src/check.rs`）

- DESIGN 5.1 の七つの段と、止まり方（1 と 2 で止まる、3 と 4 は読めない端を持つリンクを飛ばす、5 の循環は飛ばす）。
- 通ったときの一行（英語と日本語。DESIGN 5.1 の形の案を、実物に差し替える）。
- `--format json`（DESIGN 6.1）。

### B.10 review（`src/review.rs`）

- 選び方（DESIGN 7 章）：`--at <file.req>:<行>`、`--requirement '<名前か別名>[ v<n>]'`、`--all`。選んだものに印が無ければ書かない。端を作れないリンクには書かずに exit 1。1 と 2 の段にエラーがあれば何も書かない。
- 書き方（DESIGN 4.2）：記録の行を足すか書き換える。ほかのバイトは CR LF も含めて一字も変えない。足す行の字下げはリンクの行より空白二つ深く、行の終わりはそのファイルの改行に合わせる。`--by` は宣言した役割（E008）。`--date` を省けばその日。
- 確かめたときの中身（DESIGN 4.4）：両端の端の中身を `reviewed/<16 桁>` に書き、どの記録からも指されなくなったファイルを消す（名前が 16 桁の 16 進数のものだけ）。UTF-8 でない中身と 1 MiB を超える中身は書かない。
- 書いたリンクごとに、何が変わっていたかと、書いた記録の行を一行ずつ出す。
- テスト（`tests/review.rs`）：B.7 の 1〜3 の変異に `yuen review --all --by 法務 --date 2026-10-04` を当てると、`check` が通る。`.req` のバイト列が、記録の行のほかは元と同じ（CR LF のファイルでも）。`reviewed/` に新しい中身が書かれ、古い中身が消える。`shasum` で、どの中身もファイルの名前のハッシュになっている。印の無いリンクを `--at` で選んでも書かない。

### B.11 trace（`src/trace.rs`）

- DESIGN 9 章：要件から（出どころ、コピーの本文と時点と版、決めたこと、リンクと記録と印）、`file` の成果物から、出典の条から。`--format json`。成果物のファイルが固定している条は C で足す。
- テスト（`tests/trace.rs`）：`period` の三つの要件、`file "民法の期間.cal"`、`@民法 第142条` の trace を、英語と日本語と JSON の golden にする。

### B.12 診断と台帳（`src/diag.rs`、`src/codes.rs`）

- `Diag`：code、severity、file、line、col、message（`Text`）、notes（`Text` の並び）、diff、chain、candidates、fix。文面と JSON は DESIGN 6.1 の形。
- `src/codes.rs`：DESIGN 6.2 の全コード。各コードに、見出し、いつ出るか、どう直すか（英語と日本語）、走る最小の再現（`.req` の本文と、要るならコピーやファイルの中身）。一式のツールか proto を読まないと出せないコード（E106、E107、E202、E203、E204、E205、W201）は、C の段階で再現を足す。それまでは、`explain` がそのコードを引けて、再現が「C の段階で足す」と分かるようにし、`tests/codes.rs` が B の段階で確かめるコードの集まりから外す。
- `yuen explain <コード>`、`yuen explain --all`、`--format markdown`。
- テスト：`tests/codes.rs`（B のコードの再現が、そのコードを出す）、`tests/mutants.rs`（`tests/mutants/<コード>_<内容>/` ごとに、英語と日本語の golden `tests/golden/<同じ名前>.en.txt`・`.ja.txt` と一致する）。

### B.13 CLI と api（`src/cli.rs`、`src/main.rs`、`src/api.rs`）

- DESIGN 7 章：コマンドとフラグを一枚の表に置き、`yuen --help`、`yuen <cmd> --help`（`yuen help <cmd>` も同じ）、`yuen --version`。知らないフラグ、閉じた集合の外の値、値の無いフラグ、二度書いたフラグ（何度でも書けるものを除く）は exit 2。引数なしの `yuen` は使い方を標準エラーに出して exit 2。
- B で実装するコマンドは `check`、`review`、`trace`、`api`、`explain`。まだ実装していないコマンドを表に載せない。
- `api`（DESIGN 11 章）：B では `file` の成果物だけ。キーの順は DESIGN のとおり。
- テスト（`tests/cli.rs`、`tests/api.rs`）：exit code、全コマンドの `--help`、知らないフラグ、`YUEN_LANG`。`period` の api の golden。

### B.14 テストの材料

- `tests/fixtures/period/`：koyomi の `examples/民法の期間.cal`（B では `file` の成果物として読む）と、`sources/law/129AC0000000089@2026-10-01/`（四つの XML と `revision.txt`）をコピーしたもの。`民法の期間.req` は、DESIGN 1.1 の要件三つを、次のように B の段階で読める形にしたもの：出典は自分で保存した `source 民法 = law "129AC0000000089" asof 2026-10-01`（四つの固定）、成果物は `satisfied by file "民法の期間.cal"`、確かめる側は三つとも `not verified` の見送り（理由と承認）、範囲は `scope file "民法の期間.cal"`。確かめた記録と承認は、B.10 の `review` で書いたもの（日付は 2026-10-03）と、その `reviewed/`。
- `tests/fixtures/ecfr/`：rulec の `tests/corpus/sources/law/29-CFR-1910@2026-01-01/1910.157.xml` をコピーしたもの（固定 `c2a9ce966c7e2269`）と、それを `@osha "§1910.157"` で引く英語の要件一つ。eCFR の置き場所と E104 の要素の名前を、B の段階で確かめる。
- `tests/mutants/`：台帳のどのコードにも、変異が一つ以上ある（`<コード>_<内容>/` のディレクトリに、`.req` と要るファイル）。どの変異も自分をルートにして検査するので（`--root`）、golden はリポジトリの置き場所に依らない。
- 要件の端のハッシュは、出典を自分で保存しても借りても同じになる（DESIGN 4.1）。C.7 で、同じ要件を koyomi から借りた出典で書き、同じ値になることを確かめる。

### B.15 B の完了の条件

`cargo build` が警告なしで通り、`cargo test -- --nocapture` が全部通ること。B のテストは外のツールを使わないので、SKIP は 0。そのうえで、次が出ること。

| 対象 | 出ること |
|---|---|
| SHA-256 | B.4 の四つの既知の値 |
| `period` のコピー | 固定が 140 条 `e880059021fbb67d`、141 条 `0575c131b9f08063`、142 条 `fc8c35a0769d3b35`、143 条 `6950bdfb988439b6` |
| `period` の端 | `起算日` `a9ebc73907faddc8`、`満了日` `465b83ed8c251406`、`満了日_142条` `d4f2d2a67322df17`、`file "民法の期間.cal"` `c9b94eecde23e6b5`（A の段階の試作と同じ値。違えば、どちらが DESIGN 4.1 の定義どおりかを決め、試作の側が誤っていたなら DESIGN の値を直して報告する） |
| `period` の check | exit 0。一行の結果が、要件 3 件、リンク 6 本、見送り 3 件を言う |
| B.7 の変異 | 1 は E302 が二つと E304 が一つ、2 は E303・E302・E304 が一つずつ、3 は E303 が三つ（差分は一本め）、4 は E301 と E304、5 は 1 の印に W301 |
| B.10 の review | 1〜3 の変異が `review --all` のあと通る。`.req` は記録の行のほかは一字も変わらない |
| 診断 | DESIGN 6.2 のうち B のコードに変異と再現があり、英語と日本語の golden と一致する |
| 参照 | `tests/fixtures/naming.tsv` の全行が表のとおり（B の段階では JSON 18 行、エラー 9 行。2026-10-03 に sakai との突き合わせで 36 行（JSON 21 行、エラー 15 行）になり、C の段階でコピーし直した） |

報告には、`cargo test` のテストの数と時間を書く。DESIGN の形の案のうち、B で実物に差し替えたもの（5.1 の一行、4.3 の例の診断、6.3 の例、9 章の trace）を挙げる。

## 3. 段階 C：一式の読み込み、affected、書き出し、出典のコマンド

一式の言語が持つものと proto を読み、借りた出典と食い違いの検査、名前の変わった成果物、`affected`、ReqIF と PROV の書き出し、`source fetch | pin | outdated` を作る。

**C は二つに分けて進めた。** 一式の言語を一つの処理系（ritsu）にまとめるかを決めていなかったので、一式の読み込み（C.1〜C.9）はそれが決まるまで止め、yuen の要件だけで作れる C.10〜C.12（ReqIF、PROV、出典のコマンド）を 2026-10-03 に先に作った。まとめると決まり、yuen は ritsu の `crates/yuen` になった。C.1〜C.9 は、子プロセスでツールの CLI を呼んで JSON を読む計画だったものを、ritsu の口（ritsu の DESIGN 3.2）で読むように書き直し（下の C.1〜C.9 がその計画）、ritsu の PLAN の D.7 で作った（2026-10-04）。書き直す前の計画の要点は、それぞれの項の最後に「前の計画」として残した。

### C.1 口をつなぐ（`src/suite.rs`、`src/run.rs`、`src/ends.rs`）

- yuen は、読む言語を `Suite`（言語ごとの `Items` と `Sources`、それに `Rules`、`Dates`、`Claims`）として渡される。yuen はほかの言語のクレートを知らない。`ritsu yuen`（ritsu の `crates/ritsu`）はすべての言語をつないで渡し、yuen のクレートのバイナリは何も渡さない（ritsu の DESIGN 2.3）。
- コマンドを関数にする（`yuen::run::run(引数, Suite, 標準出力, 標準エラー)`）。クレートのバイナリはそれを呼ぶだけにする。
- 一回の実行の中で、同じファイルを同じ口に二度尋ねない（`Suite` が答えを覚える）。
- 言語がつながっていなければ、何を読めないかと、`ritsu yuen` に同じコマンドを続けた形を言って exit 2。言語が答えられなければ E203（その言語の診断を注に）。中のものの定義の文が空なら、空の文のハッシュを端にせず E203（ritsu の PLAN 7.6）。E204 は退かせる（JSON を読まない）。
- 端（DESIGN 3.2）：ファイルを指したらバイト列、中のものを指したら、その言語が渡す定義の文。rulec と koyomi のファイルは、`Sources` がそのファイルの検査を通してから答えるので、検査を通らないものからは端を作らない。
- テスト（`tests/suite.rs`、`tests/cli.rs`）：クレートのバイナリが、六つの言語のどれを指すプロジェクトでも exit 2 で止まり、`ritsu yuen check …` を言い、同じプロジェクトをすべての言語をつないで走らせれば通ること。一つの規則のファイルに六本のリンクがあっても、口に一度だけ尋ねること（数える口を挟む）。定義の文の空のものが E203 になること（空の文を渡す口を挟む）。検査を通らないカレンダー（koyomi の例 `民法の期間_読み方の比較.cal`）を指せば E203 で、koyomi の診断が注に出ること。
- 前の計画：ツールを `YUEN_RULEC` などか PATH から探し、そのファイルのディレクトリで呼び、exit 0 でなければ E203、JSON にキーが無ければ E204、偽のツールのシェルスクリプトで E204 を試す、だった。

### C.2 rulec

- 中のもの（`input`、`output`、`enum` と `value`、`table`、`clause`、`define`、`derive`、`machine`、`source`）は rulec の `Items` で、別名は `Rules` の事実（入力、出力、列挙、値の別名）で、規則が固定している出典は `Sources` で読む。
- テストの材料：`tests/fixtures/rulec/`。rulec のコーパスの `印紙税の本則と軽減.rule` と `osha_extinguisher.rule` を `rules/` に、その出典のコピーを `rules/sources/law/` にコピーする。要件の側には、同じ条を自分で保存して固定した `印紙税.req`（コピーは規則のコピーと同じバイト列）と、規則の出典を借りる `fire_extinguishers.req` を置く。
- テスト：`output 印紙税額`、`input 契約金額`、`table 本則`、`table 軽減`、`clause 非課税`、`define 軽減期間` があると分かり、`output tax`（別名）は E202 で名前を候補に示し、`table 無い表` は E202。ファイルを指したときの端が `dc176eebd83f26e3` と `a52e955b88af88d6`。表と節と `define` の端（取り直した値は C.13）。`yuen api` の成果物の `pins` が、`法 別表第一 0ba69792e960021e`、`措置法 第91条 85faf53f6f6e8196`、`osha "§1910.157" c2a9ce966c7e2269`。
- 前の計画：`rulec api` と `rulec graph` の JSON から読み、端はファイル全体（`source_sha256`）にする、だった。

### C.3 koyomi

- 中のもの（`input`、`date`、`claim`、`source`）は koyomi の `Items` で、別名は `Dates` の事実で、日付のファイルとカレンダーのファイルが固定している出典は `Sources` で読む。
- テストの材料：`tests/fixtures/koyomi/`。koyomi の `examples/` から `民法の期間.cal`、`支払_20日締め翌月10日払い.cal`、`calendars/民法142条の休日.cal`、`calendars/東京の営業日.cal`、`calendars/data/syukujitsu.csv`、`sources/law/129AC0000000089@2026-10-01/` をコピーする。要件は、民法を借りる `民法の期間.req`（日付と条件を一つずつ指し、範囲は `scope koyomi "民法の期間.cal" date`）と、カレンダーの祝日の表（`file` の出典）を借りる `支払条件.req`。
- テスト：日付 `起算日`・`満了日`・`満了日_142条` と条件の端（取り直した値は C.13）。条件 `142条の満了日は満了日以後` の端の中身が、その条件の行であること。`calendars/東京の営業日.cal` の出典 `祝日` が、固定 `cec37a743c96995c` の `file` の出典として読めること。カレンダーの 142 条のコピーを一文字変えて固定し直すと、印が付くのは `満了日_142条` の三本だけであること（DESIGN 4.3）。
- 前の計画：`koyomi api` の JSON から読み、日付の端はファイル全体、条件の端は `claims[]` の一つから `name` を除いた JSON にする、だった。

### C.4 chobo

- 中のもの（`unit`、`account`、`transfer`）は chobo の `Items` で読む。定義の文は、A の段階に決めた形の JSON（DESIGN 3.2）を chobo が作る。
- テストの材料：`tests/fixtures/chobo/` に、chobo の `examples/refunds/refunds.ja.book` をコピーする。出典は、テストのために書いた返金の方針の文書（`file` の出典）。
- テスト：`transfer 返金` の端が `84e9ce254075c697`（1,206 バイト）、`transfer 売上計上` が `851ab806078168fe`、`account 返金できる残り` が `9f9b0d74872f62a4`、`account 売上` と `account 返金済み` がどちらも `35a4ec5a2ee5eb06`。`返金` の端の中身が golden（`tests/golden/ends/chobo-返金.json`）と一字一句同じ。
- 前の計画：`chobo api` の JSON から、名前とコードを除いて yuen が組み立てる、だった。

### C.5 geas

- 主張は geas の `Items`（spec そのものから。記録は要らない）で、記録は `Claims` の `map_record` で読む。W201 は退かせる。
- 範囲の 4（DESIGN 5.3）：記録の、主張ごとに走らせたファイルを集める。パスは記録の一行めの `root`（spec のディレクトリからの相対）から読み、yuen のルートからの相対に直す。記録の無い spec があれば、範囲の `file` の成果物の E404 に注で添える。
- テストの材料：`tests/fixtures/geas/`。geas の `examples/greeter/` の `greeter.geas`、`greeter.ja.geas`、`server.py`、`server_refactored.py` を `greeter/` にコピーし、`geas map greeter/greeter.geas --root .` で一度だけ取った記録を `greeter/.geas/greeter.map.jsonl` に、`server.py` の、空の名前を受け付けないときの文言を一行変えた差分と、その変更のあとに取った記録を `changes/` に置く。要件は、テストのために書いた契約の文書を出典にした `greeter.req`。
- テスト：四つの主張が名前で分かり、`claim "no such claim"` は E202。`scope file "greeter/server.py"` が、`verified by geas "greeter/greeter.geas" claim "rejects an empty name"` だけを書いた要件から、記録を通って辿れる。記録を消すと E404 で、記録が無いことを注で言う。`server_refactored.py` は記録で `code` が null（どのランタイムも報告しない）なので、範囲に入れると E404。
- 前の計画：記録の一行めで主張の名前を確かめ、記録が無ければ W201、記録はテストの中で `geas map` を走らせて作る（python3 が無ければ SKIP）、だった。

### C.6 proto（`src/proto.rs`）

- ritsu の `.proto` の読み手（ritsu-proto）で、ファイルと、それが読み込むファイルを読む（DESIGN 3.4）。読めなければ E205。
- 種類（`service`、`method`、`message`、`field`、`enum`、`value`）と、端の中身（コメントと空白を落とした決まった形の文。`method` は入力と出力からたどれる `message` と `enum` の全部を、完全な名前の順に足す）。見つからない `import` の型は、書いたとおりの名前で入る。
- テストの材料：dandori の `examples/fulfillment/specs/warehouse.proto` と `fulfillment.proto` を `tests/fixtures/proto/` にコピーする。
- テスト：`warehouse.proto` の `service StockService method Reserve`、`method Release`、`enum Stock` が分かる。`method Release` を `service` の外に書けば E012。`ReserveRequest` にフィールドを一つ足すと、`method Reserve` と `service StockService` の端は変わり、`method Release` の端は変わらない。コメントだけを書き換えても、どの端も変わらない。端の中身の golden（`tests/golden/ends/proto-*.txt`。`fulfillment.proto` の `Fulfill` は、読めない `import "dandori/v1/options.proto"` のオプションを書いたとおりに持つ）。
- 前の計画：dandori の `src/proto.rs` と同じ範囲を yuen が自分で読む、だった。

### C.7 借りた出典と食い違い（`src/sources.rs`）

- 借りた出典（DESIGN 1.4、3.3）：借りた先の言語の `Sources` から名前を引き、固定を読み、コピーはそのファイルの隣の `sources/law/…` から読む。ファイルが無い、出典を宣言していない、条を固定していない、コピーが読めないか固定と違う、のどれも E106。言語が答えられなければ E203。
- 成果物のファイルが固定している条を、グラフに足す（`trace`、`affected`、`source outdated`、PROV、`api` の `pins` が使う）。
- 食い違い（E107）：要件の `from @… 第N条` と、その要件を満たす規則やカレンダーのファイルの固定が同じ条（データベース、ID、条）を指すとき、コピーの本文を比べる。成果物がその条を固定しているのに、どのコピーも本文が違えば E107（本文の差分つき）。
- テスト（`tests/suite.rs`）：借りた出典で書いた `tests/fixtures/koyomi` の `民法の期間.req` が、出典を自分で保存した `period` と同じ要件の端（`a9ebc73907faddc8`、`465b83ed8c251406`、`d4f2d2a67322df17`）を出し、`check` が通る。koyomi が固定していない条（`第144条`）を引けば E106 で、固定している条を注に並べる。`tests/fixtures/rulec/` の、自分で保存した `法` と `措置法` を引く要件では E107 は出ず、yuen の側のコピーの本文を一文字変えて固定し直すと E107 が出る。
- 前の計画：ツールの api の `sources` から読む、だった。

### C.8 名前の変わった成果物（DESIGN 4.5）

- E202 のとき、同じファイルの同じ種類のもので、今の端が記録のリンク先のハッシュと同じものを、候補として添える。端に名前が入る種類では、どのリンクも指していない同じ種類のものを並べ、候補が一つで、確かめたときの中身が `reviewed/` にあれば、その差分を見せる。
- テスト（`tests/suite.rs` と変異 `E202_名前が変わった`）：chobo の帳簿で `transfer 返金` の名前を `返金の申請` に変えると、候補に `返金の申請` が一つ出る。`account 返金済み` を `返した額` に変えると、候補に `売上` と `返した額` の二つが出る（DESIGN 3.2）。koyomi の日付 `起算日` の名前を変えると、どのリンクも指していない日付が候補に出て、名前の行だけが変わった差分が出る。

### C.9 affected（`src/affected.rs`）

- DESIGN 8 章。統一形式の差分（`git diff` と `diff -u`。`diff --git`、`---`/`+++`、`a/`・`b/`、引用符で書いたパス、改名、ファイルの追加と削除）を読み、ファイルごとに、`.req`（差分の行が入る要件のブロック）、出典のコピー、成果物のファイル、コード（geas の spec ごとに geas の `Claims` の `affected` に尋ねる。`--map <spec>=<記録>` はその spec の記録）、範囲のファイル、そのほか、に分ける。差分の読み手は、geas の `src/diff.rs` の読む部分を ritsu-base（`udiff`）に移して、geas と yuen で一つにする（geas の振る舞いは変えない）。
- 答えの文面（英語と日本語）と `--format json`。exit code は 0・1・2（DESIGN 8 章）。コマンドの表に `affected` を足す（`--diff`、何度でも書ける `--map`、`--format json`）。
- テスト（`tests/suite.rs`）：`tests/fixtures/geas` に A の段階と同じ差分を当て、変更の前と後の記録を渡すと、主張 `rejects an empty name` と、それを確かめる要件と持ち主が出る（exit 0。英語、日本語、JSON の golden）。前の記録だけを渡すと、geas の E063 を主張の節に言って exit 2。`.req` の要件の文を変える差分、借りた出典のコピーを変える差分（その条を引く要件と、固定している成果物が出る）、規則を変える差分（その規則を指す要件が出る）、自分で保存した条のコピーを変える差分（同じ条を固定している規則も出る）の golden（`tests/golden/affected/`）。範囲の中でどの要件にも辿れないファイルを変える差分は exit 1。読めない差分と、どのリンクも指していない spec の `--map` は exit 2。
- 前の計画：geas の `affected` を子プロセスで呼び（`geas affected <spec> <差分> --json`）、差分は yuen が自分で読む、だった。

### dandori と sakai

C.1〜C.9 の計画には無かったが、ritsu の D.6 で dandori が中のものを口で渡すようになったので、同じ形で読む。

- テストの材料：`tests/fixtures/dandori/`（dandori の例 `fulfillment/arrange_delivery.ja.flow` と、テストのために書いた配送の手順の文書）、`tests/fixtures/sakai/`（sakai の例の `contexts/受注.ctx` と、テストのために書いた用語集）。
- テスト：dandori のタスク `翌日便を頼む` の端が、そのタスクの宣言の塊の行で `3c19313a3f633021`。`record 集荷 field 追跡番号` と `output 追跡番号` の端が、どちらも `追跡番号 : string`。sakai の語 `キャンセル` の端が、その語の塊の行で `dbfd211b7e4cef4b`。

### C.10 ReqIF（`src/export/reqif.rs`）

- DESIGN 12 章の書き出し方。識別子（`_` と SHA-256 の先頭 32 桁）、`LAST-CHANGE` と `CREATION-TIME`（`--time`、無ければプロジェクトのいちばん新しい日付、それも無ければ exit 2）、`RELATION-GROUP`、文字列の `MAX-LENGTH` 100,000（超えれば exit 2）。
- `tools/reqif/fetch.sh`：次の 24 個のファイルを取り（`http://www.w3.org/…` と `http://www.omg.org/…` は https で取る）、SHA-256 を確かめ、`tools/reqif/xsd/<ホスト>/<パス>` に置いて、元の URL を手元のファイルに向ける XML カタログ `tools/reqif/xsd/catalog.xml` を書く。`tools/reqif/xsd/` は git に入れない。`tools/reqif/README.md` に、取り方と、ファイルが OMG と W3C のものであることを書く。

  | SHA-256（A の段階、2026-10-03 に取ったもの） | ファイル |
  |---|---|
  | `9243f345540f25db3b53403da9ad9cd4744277ef01492ac3589937f533ba94c0` | `www.omg.org/spec/ReqIF/20110401/reqif.xsd` |
  | `4995bc97cf0a9b8462ca295006dd54d9a85fb820cf9fd6e134a51743fc44effd` | `www.omg.org/spec/ReqIF/20110402/driver.xsd` |
  | `61960fb3131e38022caad5360e2f33a3382578ab3c80cd58bd74320ede61b20c` | `www.w3.org/2001/xml.xsd` |
  | `cc701736c42cc64126fad063bb95f94484b5de3b5f808a86ea098b0957aff829` | `www.w3.org/2009/01/xml.xsd` |
  | `ab0c593a06a60a5fee2b77ec9283394a3974079b75589ced177944e627f4083e` | `www.w3.org/TR/xhtml-modularization/SCHEMA/xhtml-attribs-1.xsd` |
  | `34479ecd862fea5ed1d8eb2c561bc5fcb75ee0443046aead242a123b5cff49e3` | `…/xhtml-blkphras-1.xsd` |
  | `4f40e2d55ea57a7356d638d6b562f828e33fc7804002cf26823259f3f5e95e67` | `…/xhtml-blkpres-1.xsd` |
  | `080afe0ce1da906020e53d9385151790416a8255f5a1f6b1757c02ecb8edecfa` | `…/xhtml-blkstruct-1.xsd` |
  | `0559368d5dba054a941537296a77ae327209c172dd1facad3ae05bab09a0a9ae` | `…/xhtml-charent-1.xsd` |
  | `cb5a32da43a65d91cf7ee6a42cc47a8ff70796256587884b7bac9d1444938fd7` | `…/xhtml-datatypes-1.xsd` |
  | `d7b2af85393f31c8a0325ffbfea351e80929af0824e3f2eaf26e2b34d1c0196d` | `…/xhtml-edit-1.xsd` |
  | `425afbe531545f084783a077b2dcb44b09f23ba36536c3547a70ef100a6dd02d` | `…/xhtml-framework-1.xsd` |
  | `88344fdf2a127ae4449d7b6c813f8b755ca6eb71c8583a50f6012e2459cf7e30` | `…/xhtml-hypertext-1.xsd` |
  | `3e5234930694552e6f69f4c133ba57db24944e4bb346169ad5718182e8aec07c` | `…/xhtml-inlphras-1.xsd` |
  | `0ee7157cf3f99900d15a3a1a4b8daf50876623631e3cab145710b0eda6423541` | `…/xhtml-inlpres-1.xsd` |
  | `ec861146ae81ad25331130865513951aef1852f2952f7b4604ca3456f9da97cc` | `…/xhtml-inlstruct-1.xsd` |
  | `764b9e6edf496f348449727610d7d19ec24b1027bacc352636200db7976a06c5` | `…/xhtml-inlstyle-1.xsd` |
  | `74bd94dcdbc889e2006fa354c27b85b3ee0e8e06069827c4db796c060b96b4a1` | `…/xhtml-list-1.xsd` |
  | `c623c60fb9cd2c26cb8aaddc8df5d7dabe99e573e354437ac44e12d9c7e6cdef` | `…/xhtml-notations-1.xsd` |
  | `4862dd0a3ab00eb5cd5c29a46736f89461b822f074d743d3faa3d6c8a200cbdb` | `…/xhtml-object-1.xsd` |
  | `99d1df8882159a1ec923b064450e207ce03443b621045d438faa654e402820e3` | `…/xhtml-param-1.xsd` |
  | `1bcde28585c9372b8cdbb2dc8c44b4f04e798446df415d6f6e494ce1e531791e` | `…/xhtml-pres-1.xsd` |
  | `11b510f8a116b937b20b13339fbf33bbc9bc133e917ee3df4f3b5abba3f63fc2` | `…/xhtml-table-1.xsd` |
  | `527da2d8384ea77159648dc85e81703d402b9dbe5ae099816c61ff9994a3ca5d` | `…/xhtml-text-1.xsd` |

- テスト（`tests/export.rs`）：テストの材料のプロジェクト（`period`、`ecfr`、`payment` と、一式の読み込みで足した `rulec`、`koyomi`、`chobo`、`geas`、`proto`、`dandori`、`sakai`）ごとに ReqIF を書き出し、次を確かめる。`XML_CATALOG_FILES=<スキーマ>/catalog.xml xmllint --nonet --noout --schema …/reqif.xsd` が `validates` を出して exit 0（スキーマが無ければ SKIP）。`reqif validate` と `reqif validate --use-reqif-schema` が exit 0（`YUEN_PYTHON` の venv が無ければ SKIP）。yuen のテストが自分で、すべての `-REF` が、その名前の要素（`<X-REF>` なら `X`）の `IDENTIFIER` を指していることと、`IDENTIFIER` が `_` と 32 桁の 16 進数で重ならないことを確かめる（これは SKIP しない。壊した参照を見つけることも確かめる）。同じプロジェクトを二度書き出して、バイト列が同じ。要件の数、つながりの数、`SPEC-HIERARCHY` の数が `yuen api` の数と合う。golden（`tests/golden/export/<名前>.reqif`）。止まるとき：1〜4 の段のエラーで exit 1（何も書かない）、印は状態として書く、日付が一つも無ければ `--time` を求めて exit 2、`--time` を渡せば `CREATION-TIME` になる、長すぎる文字列と XML に書けない文字で exit 2、`--format` と `--time` を逆の形に渡せば exit 2、`--out`。

### C.11 PROV（`src/export/prov.rs`）

- DESIGN 13 章の書き出し方。PROV-N（既定）と PROV-JSON（`--format json`）。成果物のファイルが固定している条（`yuen:pins`）は、一式の読み込み（ritsu の D.7）で足した。
- テスト（`tests/export.rs`）：同じテストの材料のプロジェクトごとに、`prov` 3.2.2 で PROV-N（`profile="strict"`）と PROV-JSON の両方を読み（`ProvDocument.deserialize`）、二つが等しいこと（`==`）、種類ごとの記録の数が `yuen api` から数えた数と合うことを確かめる（`YUEN_PYTHON` が無ければ SKIP）。`hadPrimarySource` の項を書かないこと。二度書き出して同じバイト列になること。golden（`tests/golden/export/<名前>.provn` と `.prov.json`）。

### C.12 出典のコマンド（`src/fetch.rs`）

- DESIGN 14 章。`curl -fsSL`、三度まで試す。`file://` も読む。e-Gov は `YUEN_EGOV`（無ければ `https://laws.e-gov.go.jp/api/2`）、eCFR は `YUEN_ECFR`（無ければ `https://www.ecfr.gov/api/versioner/v1`）。
- `fetch`：本文が前のコピーと同じなら書き換えない。`revision.txt`。借りた出典は取らずに、ツールのコマンドを言う。
- `pin`：16 桁だけを書き換え、ほかは一字も変えない。足りない固定の行を足す。
- `outdated`：後の版の本文を一つ前の本文と比べ、変わる条ごとに、施行日、版、本文の差分、引く要件と持ち主、確かめ直しになるリンクの数、その条を固定している成果物を言う。`file` の出典は `url` のもののハッシュを固定と比べる。
- テスト（`tests/fetch.rs`）：テストの中に `std::net::TcpListener`（`127.0.0.1:0`）で小さな HTTP サーバーを立て、e-Gov の `law_data` と `law_revisions`、eCFR の `full` と `versions` に用意したレスポンスを返す。用意するのは、民法 140〜143 条（`period` のコピーを base64 にしたもの）と後の版が五つある `law_revisions`、29 CFR 1910.157（`tests/fixtures/ecfr/` のコピー。一式の読み込みを作れば `tests/fixtures/rulec/` にも同じコピーが入る）と、後の版を一つ持つ `versions`（中身の変わらない版と、`substantive` が偽の版も混ぜる）。確かめること：`fetch` がコピーと同じバイト列を書く。`pin` のあとの `.req` が、16 桁のほかは一字も変わらない（CR LF のファイルでも）。`outdated` が、変わらない版では exit 0、ある版の 142 条の本文を一文字変えたレスポンスでは、その施行日と、`満了日_142条` と持ち主 `法務` と確かめ直しになるリンクの数を言って exit 1、属性だけを変えたレスポンスでは exit 0。`file` の出典は `file://` の URL で（`payment` の `約款`）、変わらなければ exit 0、変われば exit 1 で、コピーとの差分、それを引く要件とそれを元にした要件、確かめ直すリンクの数を言い、`fetch` と `pin` のあとの `check` が、その数だけ印を付ける。借りた出典は、`fetch` と `pin` が借りた先の言語のコマンドを言い、`outdated` は、yuen のクレートのバイナリなら `ritsu yuen source outdated …` を言って exit 2 で止まり、`ritsu yuen` なら借りた先の固定とコピーで問う（ritsu の D.7。`crates/ritsu/tests/yuen.rs` が、テストの中の e-Gov で確かめる）。curl が無ければ試し直さずに exit 2。`YUEN_NET=1` のときだけ、本物の e-Gov と eCFR に `outdated` を走らせ、走らせないときは `not asked:` の行を出す。
- `check` が通信しないこと：PATH から `curl` を外しても `check`・`trace`・`api`・`export` が通る（`tests/cli.rs` に足す）。

### C.13 C の完了の条件

- `cargo test -- --nocapture` が全部通り、この機械で SKIP が 0（`tools/reqif/xsd` と `YUEN_PYTHON` の venv を用意して回す。ほかの言語は同じプロセスの中でつなぐので、SKIP にならない）。
- C.2〜C.4 の端のハッシュが出る。ファイルを指したときの端は、A の段階の値のまま（`印紙税の本則と軽減.rule` `dc176eebd83f26e3`、`osha_extinguisher.rule` `a52e955b88af88d6`）。chobo の端も A の段階の値のまま（`84e9ce254075c697`、`851ab806078168fe`、`9f9b0d74872f62a4`、`35a4ec5a2ee5eb06`）。koyomi の日付と条件、rulec の表と節は、言語が渡す定義の文が端になったので取り直した（下の表。A の段階の試作の値 `c9b94eecde23e6b5`、`825aa6c6f7ccf314`・`447d80ca751bd681`・`2a8e130e4c527692`・`0b951fef68a36592` は、ファイル全体と `koyomi api` の JSON から作る端の値で、もう出ない）。
- C.7 の、借りた出典で書いた `民法の期間.req` が、B と同じ要件の端を出す。
- C.9 の `affected` が、greeter の差分で主張から要件と持ち主まで答える。
- C.10 と C.11 の書き出しが、xmllint、`reqif validate`、`prov` のどれでも通る（十のプロジェクト）。
- DESIGN 6.2 のうち C で再現を足したコード（E106、E107、E202、E203、E205）に変異と再現があり、英語と日本語の golden と一致する。E204 と W201 は退いたコードとして台帳に残る。
- 報告に、ツールのバージョンと、テスト全体の時間を書く。DESIGN の形の案のうち、C で実物に差し替えたもの（8 章の affected、11 章の api）を挙げる。

**2026-10-03 に満たしたもの**（書き出しと出典のコマンドの部分）：`cargo test -- --nocapture` が全部通り、この機械で SKIP が 0（スキーマと venv を `YUEN_REQIF_XSD` と `YUEN_PYTHON` で渡して）。C.10 と C.11 の書き出しが、xmllint、`reqif validate`（`--use-reqif-schema` も）、`prov` のどれでも通る（`period`、`ecfr`、`payment`）。DESIGN 14 章の outdated を、本物の e-Gov に一度問い合わせた出力に差し替えた（12 章と 13 章も実物にした）。

**2026-10-04 に満たしたもの**（一式の読み込み。ritsu の D.7）：上の条件の全部。yuen のテスト（19 のファイルと単体テスト）が全部通り、SKIP は 0。DESIGN 3 章を口で読む形に書き直し、8 章と 11 章を実物にし、14 章に借りた出典の `outdated` の出力を足し、19 章に取り直したハッシュを書いた。取り直したハッシュ（`tests/suite.rs` が確かめる）：

| 成果物 | 端 |
|---|---|
| `rulec "rules/印紙税の本則と軽減.rule" table 本則` | `21432c19a67fe72e` |
| `… clause 非課税` | `5bc61581e0c31022` |
| `… table 軽減` | `4ba4c2dec2c8262b` |
| `… define 軽減期間` | `b5b30cefd66e58d6` |
| `rulec "rules/osha_extinguisher.rule" table distance` | `5681500476af8a56` |
| `koyomi "民法の期間.cal" date 起算日` | `15aa6c91d6aaae80` |
| `… date 満了日` | `23441fd408f07548` |
| `… date 満了日_142条` | `bba4761410179e6e` |
| `… claim 142条の満了日は満了日以後` | `7c616f5dcc9503a8` |
| `… claim 満了日は単調` | `53b5eb9afcef2219` |
| `… claim 142条の満了日も単調` | `44569913a3f84e29` |
| `… claim 満了日は起点より後` | `5e7bb0ed51835dc7` |
| `koyomi "支払_20日締め翌月10日払い.cal" date 支払日` | `feb9535fb9350ecc` |
| `… claim 営業日に払う` | `3cf8fc3fb2dda9d9` |
| `geas "greeter/greeter.geas" claim "rejects an empty name"` | `3f7c8d1b2bbcc309` |
| `proto "warehouse.proto" service StockService method Reserve` | `cadb2fc727e9af81` |
| `… service StockService method Release` | `2a0912a66889e515` |
| `dandori "arrange_delivery.ja.flow" task 翌日便を頼む` | `3c19313a3f633021` |
| `sakai "contexts/受注.ctx" term キャンセル` | `dbfd211b7e4cef4b` |

ほかの言語のものの端の全部は DESIGN 19 章にある。

## 4. 段階 D：doc、例、README、スキル

ritsu の PLAN の F.1 で、ritsu の中で作るように書き直して作った（2026-10-04）。書き直したのは次のところである。doc の HTML の枠は ritsu-base の `docpage`、テストの道具は ritsu-testkit（`TempDir`、`need` と SKIP、golden の BLESS）。ほかの言語のもの（rulec の表、koyomi の日付、geas の主張、chobo の勘定と振替）は、C の後半と同じく口から読む。例、テストの材料、文書は英語を先にする（ritsu の決まり）。yuen は ritsu の一部として配り、入れ方は `cargo install --git https://github.com/i2y/ritsu --locked ritsu`、コマンドは `ritsu yuen …` である。README とスキルが言うページの読み手は「コードが実現すべきものを理解し、確かめる人」で、「承認する人」とは書かない。

### D.1 doc（`src/doc/`）

- DESIGN 10 章のとおり、Markdown（既定）と HTML（`--format html`）。ページは一度ブロックの並び（見出し、段落、表、条文の引用、診断）として組み、Markdown と HTML はそれを書き出すだけにする。グラフは `api` の JSON から読み、条文は ritsu-base の `quote_lines`（`trace` と同じ）でコピーから引く。
- 節の順（見出し、トレーサビリティの表、出典、要件ごとの「なぜ」、範囲、確かめた記録の一覧）。
- 1〜4 の段のエラーがあればページを作らず、診断を標準エラーに出して exit 1（E206 なら 2）。5〜7 の段のエラーは、ページに載せて exit 0。
- HTML は一枚で、script も外のファイルも読まない。枠と配色は ritsu-base の `docpage`。幅の狭い画面では表が枠の中で横に動く。
- `--out <dir>` があれば、最初の `.req` のファイル名から `<名前>.md` か `<名前>.html` を書く。CLI の表に `doc` を足した。
- テスト（`tests/doc.rs`）：例の `.req` ごと（十）に、Markdown と HTML、英語と日本語の golden（`tests/golden/doc/<名前>/<言語>.md|html`、四十）。HTML が外を読まないこと（`docpage::outside_urls`）。ページに引いた条文の各行が、コピーの `quote_lines` にあること。印がページに載ること、コピーが固定と違えばページを作らないこと、`--out`。Chrome があれば、`civil_code_periods_reread` のページの先頭（明るい配色）と、印の付いた要件の節（暗い配色）を英語と日本語で撮り（時間を区切る）、`YUEN_BLESS=1` のときに `docs/images/` に置く。

### D.2 例（`examples/`）

DESIGN 15 章の七つ。英語の例を先に、日本語の版を `<名前>.ja.req` として横に置く。コピーしたファイルの元（ritsu の中のパス）を、例ごとの `README.md` に書く。確かめた記録は `yuen review --date 2026-10-04` で書いた。

- `osha/`（英語）、`greeter/`（`greeter.req` と `greeter.ja.req`）、`payment_terms/`（`payment_terms.req` と `.ja.req`）、`refunds/`（`refunds.req` と `.ja.req`）、`civil_code_periods/`、`civil_code_periods_reread/`、`stamp_tax/`（この三つは e-Gov の法令にしか無いので日本語だけ。ファイルは `<英語の名前>.ja.req`）。
- テスト（`tests/examples.rs`）：`civil_code_periods_reread/` のほかの `.req` は、どれも `check` が exit 0。`civil_code_periods_reread/` は exit 1 で、E303 が一つ（差分は `if closed + 1 day` → `roll following`）、ほかのコードは出ない。A の段階では E303 が三つと書いていたが、ritsu の D.7 で koyomi の日付の端がその日付の定義の文になったので、一つになった（DESIGN 15 章）。二つの civil code の例の `.req` と `reviewed/` が同じバイト列で、`.cal` が一行だけ違うこと。止まる例のほかは、`review --all` を当てても `.req` が変わらないこと。greeter の記録（前と後）が `geas map --root` で作り直したものと同じこと。`affected` が変更から `rejects_an_empty_name` に届くこと。

### D.3 docs/

- `docs/reference.md`（英語）：言語の全部、参照の書き方、端とハッシュと確かめた記録、検査、コマンド、exit code、JSON の形、環境変数。キーワードの表は `src/kw.rs` の `TABLE` と同じ並び。
- `docs/codes.md` と `docs/codes.ja.md`：`yuen explain --all --format markdown`（`--lang ja`）の出力そのもの。
- テスト（`tests/docs.rs`）：codes の二つがいまの出力と同じ。キーワードの表が `kw.rs` と同じ。

### D.4 README.md と README.ja.md

- README.md（英語）：看板、何か、`.req` の例（`osha`）、`check` の結果、わざと止まる例の診断（日本語の名前の例であることを書き添える）、`affected`、確かめることと確かめないこと、`doc` のページ（スクリーンショット）、エージェント向け、入れ方（ritsu）、コマンド、例の一覧、どう確かめているか、次に読むもの、License。
- README.ja.md：英語の版をそのまま訳したものではなく、日本語で一から書いた。例は `civil_code_periods`。「ライセンス」の節。
- テスト（`tests/docs.rs`）：README、`docs/`、例の README、スキルの ` ```req ` の塊のどの行も、`examples/` か `tests/` の `.req` の行であること（`…` で切ってよい）。` ```console ` の塊に出力つきで書いた `ritsu yuen`（か `yuen`）のコマンドを、クレートのディレクトリで、すべての言語をつないで走らせ、書いた行がそのまま出ること。出力の無いコマンドは、yuen のコマンドであること。単独で載せた診断が、そのファイルの `check` の出力にあること。相対リンクの先があること。README に書いたコードの数が台帳の数と同じこと。

### D.5 スキル（`skills/yuen/`）

- `SKILL.md` は手で書いた（frontmatter に `name`、`description`、`compatibility`、`license: MIT OR Apache-2.0`。いつ使うか、ループ、言語の一ページ、人に聞くこと、診断から直し方、エージェントが自分の判断で `review` を走らせないこと）。
- `reference.md` と `codes.md` は `skills/sync.sh` が `docs/` からコピーする（リンクはスキルのディレクトリの外へ出ないように書き換える）。`skills/README.md` に入れ方。
- テスト（`tests/skill.rs`）：sync.sh を一時ディレクトリに走らせてコピーと同じか、スキルの中のリンクが外へ出ないか、frontmatter の形と `license` が `Cargo.toml` と同じか。

### D.6 ライセンスと出典

- MIT OR Apache-2.0。`Cargo.toml` の `license`、README.md の「License」、README.ja.md の「ライセンス」、スキルの frontmatter に書いた。
- `THIRD_PARTY_NOTICES.md`：内閣府の祝日の CSV（PDL1.0）、GOV.UK の bank holidays の JSON（OGL v3.0）、e-Gov の民法・印紙税法・租税特別措置法の条の XML（PDL1.0）、eCFR の 29 CFR 1910.157 と 37 CFR 1 の節。一式からコピーした `.rule`・`.cal`・`.book`・`.geas`・`server.py` などは同じ作者の同じライセンスのものであることと、元の場所。

### D.7 D の完了の条件

- `cargo test -p yuen -- --nocapture` が全部通り、この機械で SKIP が 0（Chrome のスクリーンショットも撮れる）。
- 例ごとの doc の golden（英語と日本語、Markdown と HTML）があり、HTML のテストが通る。
- 例の検査の結果が D.2 のとおり（`civil_code_periods_reread/` だけが E303 一つで止まる）。
- README.md と README.ja.md の中の `.req`、コマンドの出力、診断が、`tests/docs.rs` の確かめを通る。
- `docs/codes.md` と `docs/codes.ja.md` がいまの `explain` の出力と同じ。
- スキルのテストが通る。
- DESIGN の形の案が、どれも実物に差し替わっている（`grep -n '形の案' DESIGN.md` が何も出さない）。
- 報告に、README に貼った出力をどのコマンドで取ったかを書く（どれも README の ` ```console ` の塊に書いたコマンドそのもので、`tests/docs.rs` が走らせ直す）。

## 5. 次の段階への申し送り

### 5.1 A から B へ（A の段階で書いた）

- DESIGN の端のハッシュの値は、A の段階の試作で出した。B の yuen が違う値を出したら、まず DESIGN 4.1 と 3.2 の定義に照らす（とくに、要件の端の中身の行の並べ方と最後の改行、決まった形の JSON のキーの順と字下げ）。
- 法令のコピーは、koyomi と rulec の例から持ってくる（B.4、C.2、C.3）。取り直さない。e-Gov と eCFR に取りに行くのは、C.12 の `YUEN_NET=1` のテストだけである。
- A の段階で確かめた一式の振る舞い（DESIGN 19 章）：三つのツールの `source_sha256` はファイルのバイト列のハッシュ、行ごとの引用はどの JSON にも無い、検査を通らないファイルには api が出ない、geas の記録と `affected --json` の形。どれかが変わっていたら（ツールのバージョンが上がって）、DESIGN 3 章を直してから進む。
- 段階 A の時点で、まだ変わりうるとしたもの：看板の言い方、要件の端にリンク元のハッシュを入れて先のリンクを一本ずつ確かめさせること（DESIGN 4.1）、確かめた記録に人と日付を書くこと（4.2）、`reviewed/` を git に入れること（4.4）、rulec・koyomi の日付・geas の端をファイル全体にしたこと（3.2）、dandori をファイルの単位でだけ指すこと（2.7、3.5）、一式に出してほしいもの（3.5）、PROV-N と PROV-JSON の両方を出すこと（13 章）。変わったら、DESIGN と、この計画の該当の項を直してから進める。
- sakai と突き合わせる参照の書き方（DESIGN 2.8）が、突き合わせで変わったら、2 章と、B.3 と C.2〜C.6 の名前の読み方を直す。（B の段階で決着した。下の 5.2。）

### 5.2 B から C へ（B の段階で書いた）

- 参照の書き方は `tests/fixtures/naming.tsv`（sakai と同じ表）で決着した。C で種類を読むとき、DESIGN 2.3 の表の「名前を読むところ」に従う。rulec の `enum`・`value` は `rulec api` の `python.enums[]`、`machine` は `rulec api` の `machine.name` である（B の段階で `rulec api` を走らせて確かめた）。`input` と `output` を `graph` と `api` の両方から読み、一致することを確かめる。
- ツールを読むところは `src/check.rs` の `not_yet` と `src/ends.rs` の `artifact_end`（`Unread::NotYet`）と `src/coverage.rs` の `gather` に集めてある。C はそこを `src/tools/` の読み方に差し替える。借りた出典は `src/sources.rs` の `Resolved::Borrowed` で止めてある。
- 印の並べ方と説明は `src/marks.rs`。まとまりの元は `cause_of`（要件の端が変わったのは、どの上の端のせいか）で決める。端がファイル全体の種類（rulec、koyomi の日付、geas、dandori、sakai）では、`Thing::Artifact` を、参照ではなく、そのファイルの参照（`Name::file()`）にすると、同じ `.cal` を指す三本が一つのまとまりになり、差分も一度だけになる（DESIGN 4.3 の例）。B の `file` は参照がファイルそのものなので、いまは区別が要らない。
- 端の値：B の yuen は、要件の端（`a9ebc73907faddc8`、`465b83ed8c251406`、`d4f2d2a67322df17`）と `file "民法の期間.cal"`（`c9b94eecde23e6b5`）で試作と同じ値を出した。C.7 の借りた出典で同じ値が出ればよい。
- テストの一時ディレクトリは `std::env::temp_dir()` の下に作り、終われば消す。この機械では `TMPDIR` を作業場所に向けて回した。

### 5.3 C の前半から次へ（C の前半で書いた）

- 書き出しは `src/export/mod.rs` の `Graph` を読む。一式のツールの成果物を読むようになったら、`graph` の中で、借りた出典の条を `SourceNode` に（`SourceKey::Law` はデータベース、ID、時点、条なので、借りた条も自分で保存した条と同じ識別子になる）、成果物が固定している条を新しい関係（ReqIF なら型 `yuen pins` の `SPEC-RELATION`、PROV なら `wasInfluencedBy(成果物, 出典の条, [prov:type='yuen:pins'])`）に足す。テストの `prov_counts` と `references` は、そのまま新しいテストの材料にも使える。
- `source outdated` が借りた出典で exit 2 にするところは `src/main.rs` の `source_cmd`（`yuen::fetch::borrowed`）。借りた出典の固定はツールの api の `sources[].pins` から、コピーはツールのファイルの隣から読む（C.7）。条を固定している成果物は、`src/fetch.rs` の `reach_lines` に一行足せば言える。
- 診断と `trace` と `source` のコマンドが言うファイルのパスは、走らせたディレクトリからの相対にした（DESIGN 2.2。`Project::shown`）。ツールのファイルや proto のパスを文面に出すときも、`shown` を通す。JSON はルートからの相対のまま。
- 本物の e-Gov には、2026-10-03 10:38 に一度だけ問い合わせた（DESIGN 19 章）。eCFR には問い合わせていない。`YUEN_NET=1 cargo test --test fetch -- --nocapture` で両方に問い合わせられる。

### 5.4 C の後半（ritsu の D.7）から次へ

- C は済んだ。残りは D（doc、例、README、スキル）で、ritsu の PLAN の F.1 で書き直してから作る。doc（DESIGN 10 章）は、ほかの言語のものの名前と端を `Suite` から読み、規則やカレンダーのページを埋め込まずに名前とファイルを書く（10 章の「捨てたもの」のとおり）。
- ほかの言語を読むところは `src/suite.rs`（口と、一回の実行の中で覚えた答え）、`src/ends.rs` の `artifact_end`、`src/coverage.rs` の `gather` と `ran_by_claims`、`src/sources.rs` の `borrow` と `pinned_by` に集めてある。言語が新しい種類を口で渡すようになっても、yuen の側は参照の決まり（ritsu-base）に種類が入れば読める。
- テストは、ほかの言語のクレートを dev-dependency に持ち、`tests/common/mod.rs` の `suite()` と `run()` で、`ritsu yuen` と同じにつないで同じプロセスの中で走らせる。環境変数を変えて走らせるテスト（テストの中の e-Gov など）で、ほかの言語も要るものは、`crates/ritsu/tests/yuen.rs` に置く（ritsu のバイナリを走らせる）。
- `ritsu yuen` に同じコマンドを続けて言うために、`run` が引数を `check::COMMAND`（スレッドに一つ）に置く。ライブラリの関数を直接呼んだときは、`check` と渡したパスから組み立てる。

### 5.5 D（ritsu の F.1）から次へ

- D は済んだ（2026-10-04）。例は英語を先にし、e-Gov の法令にしか無い三つだけを日本語にした。例の `.req` は一つずつがプロジェクトで、英語の版と日本語の版は同じディレクトリの別のプロジェクトである。
- 例の記録は `review --date 2026-10-04` で書いた。例のファイル（コピーした `.rule`・`.cal`・`.book`・`.geas`・`server.py`）を元の言語で直したら、コピーし直して、人が確かめてから `review` を走らせ直す（記録を書くのは人が確かめたときだけ。例でも同じ）。
- `doc` のページの画像（`docs/images/`）は `YUEN_BLESS=1 cargo test -p yuen --test doc` で撮り直す。golden も同じ。
- 文書のテスト（`tests/docs.rs`）は、`$ ritsu yuen …` を、クレートのディレクトリで、すべての言語をつないで走らせる。README に出力を足すときは、走らせた出力をそのまま貼る。

### 5.6 OpenSpec の仕様を出典にする（2026-10-05 に作った）

DESIGN 20 章。ritsu の v0.23.0 のあとに作った。

- 読み手は ritsu-base の `openspec`（geas と分け合う）。yuen の側は、`src/ast.rs` の `SourceKind::OpenSpec`、`src/kw.rs` の `openspec`、`src/lex.rs`（`openspec "…"` を参照として切り出さない）、`src/parse.rs`、`src/project.rs`（パスの解決と E013）、`src/sources.rs`（`spec_source` の E101・E102・E103・E104・E108、引用の E102・E105・E108、プロジェクト全体で一度の W102、`Cited` の `from openspec …` の行）、`src/marks.rs`（差分の見出しと、名前の引用符）、`src/fetch.rs`（`fetch` の一行、`pin`、`spec_outdated`）、`src/openspec.rs`（変更の提案が固定した要件に何をするか）、`src/affected.rs`（仕様の両側を比べる `spec_touched`、提案の差分の `Proposed` と JSON の `openspec_changes`）、`src/trace.rs`、`src/doc/mod.rs`、`src/api.rs`、`src/export/`（`SourceKey::OpenSpec`。属性は `yuen.file` を使い回す）、`src/ports.rs`、`src/codes.rs`（E108、W102、W402 と、E101〜E105 の文）、`src/coverage.rs`（W402。シナリオと、要件を確かめる geas の主張の名前を突き合わせる）。
- 例は `examples/openspec_greeter` と `examples/openspec_greeter_archived`（英語と、`ja/` の下の日本語）。仕様と提案は OpenSpec 1.14.0 の `openspec validate` を通し、archive したあとの木は OpenSpec が書いたものである。固定は `yuen source pin`、記録は `yuen review --date 2026-10-05` が書いた。geas の記録（`.geas/`）は、同じ主張と同じサーバーの `examples/greeter/.geas/` のコピー。
- テストは `tests/openspec.rs`（英語と日本語、golden は `tests/golden/openspec/`）、変異は E104・E108・W102・W402 の英語と日本語の対（`tests/english_mutants.rs` の `PAIRS` は 70 組）、`tests/examples.rs` は二つめのわざと止まる例を知っている（`tests/doc.rs` は、それがページを作らないことを確かめる）。台帳のテストは 89 回の再現と 47 個のコード。
- 文書は README.md と README.ja.md の節、例ごとの README、`docs/reference.md`、`docs/codes.md` と `docs/codes.ja.md`（`explain --all --format markdown` の出力）、スキルの SKILL.md（`skills/sync.sh` が reference と codes をコピーする）。
- 残したこと（DESIGN 20.7）：OpenSpec のストアとほかのリポジトリの仕様、シナリオの単位の固定、要件の文を仕様から取ること、Spec Kit と Kiro。
