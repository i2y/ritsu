# koyomi 実装計画

DESIGN.md を仕様として、koyomi を三つの段階（B・C・D）で作る。どの段階も、ここに書いた順に進め、各段階の最後にある完了の条件のテストが全部通ったら終わりにする。DESIGN.md と違うことをしたくなったら、先に DESIGN.md に決定・理由・捨てたものを書き、報告で言う。

B、C、D の三つの段階は終わった（どれも 2026-10-03）。各段階で決めたことと測ったことは、DESIGN.md の該当の節と、下の B、C、D の項に書き足してある。

この計画を書いた A の段階では、本体のコードは書いていない。意味を確かめるための Python の試作を作業場所で書いて走らせ、その数を DESIGN.md と下の完了の条件に転記した。試作はリポジトリに残していない（B の参照インタプリタが同じ数を出すことを、完了の条件にした）。リポジトリに置いたのは、DESIGN.md、この PLAN.md、試作が読んだ祝日の表のコピー二つ（`examples/calendars/data/`）、民法の例が引く 140〜143 条のコピー（`examples/sources/law/129AC0000000089@2026-10-01/`）だけである。

## 0. 全部の段階に共通の決まり

### 0.1 守ること

- 作者の決まり（段階ごとの指示書が挙げるメモ：`japanese-style`、`private-hobby`、`no-quoting-prompts`、`write-from-real-runs`、`features-are-first-class`、`do-the-whole-job`、`shell-gotchas`、`name-the-feature`）を先に読み、従う。日本語（DESIGN.md、`--lang ja` の診断、README.ja.md、報告）は、英語の概念語を漢字に直訳しない。関数や規則が返すものは「答え」ではなく「結果」と書く。
- git のコミットと push をしない。`~/koyomi` の外に書かない（`~/rulec` と `~/dandori` は読むだけ。そこでビルドも git もしない）。
- Rust は edition 2024 で、手元の stable 1.94.1 で通すこと。依存は `serde_json = "1"` だけ（clap、chrono、encoding_rs、sha2 などは使わない）。
- 診断は英語が既定で、`--lang ja` で日本語。golden は両方の言語で取る。
- テストは `cargo test`。外のツール（node、python3、go、rustc の別の版、PostgreSQL、curl、Chrome）が無いときは、`SKIP: <理由>` を一行出して通す。報告の前に `cargo test -- --nocapture 2>&1 | grep SKIP` で SKIP の行を読み、数を報告に書く。
- golden の取り直しは `KOYOMI_BLESS=1 cargo test`。取り直したら差分を読んでから報告する。
- サーバー（PostgreSQL、HTTP、Chrome）を立てたまま終わらない。テストは止める処理を Drop に置き、一時ディレクトリを消す。
- 成果物（文書、golden、生成物）に、手元の絶対パス、ユーザー名、マシン名を入れない。
- 文書に載せる出力と数は、実際に走らせたものを貼る。

### 0.2 作者の機械で気をつけること（macOS arm64）

ツールの場所は、テストが環境変数で受け取る。この機械での値は、段階ごとの指示書にある。

- macOS には `timeout` コマンドが無い。子プロセスの時間切れは、テストの Rust の側で `Child::try_wait` を回して決め、超えたら kill する。
- go のコマンドには `-trimpath` を付ける（付けないとビルドキャッシュが一回 1 GB 近く増える）。
- Python の venv は `uv venv --python 3.13 <場所>`。Homebrew の 3.14 の venv には pip が入らない。
- PostgreSQL は常駐のサーバーを使わず、`initdb` と `pg_ctl` で使い捨てのクラスタを立てる。二つの場所は `KOYOMI_PG_BIN`（無ければ PATH）で受け取る。ソケットのパスは 103 バイトまでなので、ソケットのディレクトリは `KOYOMI_PG_SOCKET_DIR`（無ければ OS の一時ディレクトリ）で受け取る。
- Chrome は `KOYOMI_CHROME` で受け取り、無ければ macOS が Google Chrome を入れる場所（`/Applications/Google Chrome.app`）、それも無ければ PATH の `google-chrome` か `chromium` を探す。dandori と chobo と同じ順にそろえた（D のあとの仕上げで。OS の既定の場所は手元の機械に固有のパスではないので、リポジトリに書いてよい）。`--headless --dump-dom` は、出力したあとも終わらないことがあった。Chrome を使うテストとスクリプトは、時間を区切って kill し、`--user-data-dir` に一時ディレクトリを渡して、終わったら消す。
- node v23.11.0 は `.ts` を型を剥がすだけで走らせる。
- ディスクの空きが少ない。ビルドの中間物や大きな一時ファイルを残さない。

### 0.3 手本にしてよいもの（読むだけ）

- dandori：`src/diag.rs`（診断の形と、二つの言語の文を持つ Diag）、`src/main.rs`（使い方の表と引数）、`skills/`（D のスキル）、`tests/docs.rs` と `tests/skill.rs`（D の文書のテスト）。
- rulec：`src/sha256.rs`（SHA-256）、`src/i18n.rs`（`tr!` の考え。ただし koyomi は言語をプロセス全体に持たない。DESIGN 4.1）、`src/codes.rs`（台帳と再現）、`src/sources.rs`（curl の呼び方と取り直し）、`src/codegen/sql.rs`（PostgreSQL の生成）、`src/main.rs`（CLI の表）、`src/kw.rs`（キーワードの表）。取り込むときは koyomi の DESIGN に合わせて書き直す。

## 1. ディレクトリ

```
Cargo.toml  .gitignore
src/
  main.rs  lib.rs
  cli.rs           コマンドとフラグの表、--help、引数の読み取り（B）
  i18n.rs          Lang と tr!（文の組を返す）（B）
  kw.rs            キーワードの表（DESIGN 1.2）（B）
  lex.rs  parse.rs  ast.rs                                       （B）
  resolve.rs       名前、別名、型、use calendar、日付の引数、無い日の扱いの要否（B）
  reserved.rs      出力先の言語の予約語（E009）（B）
  date.rs          通算日、年月日、月の足し算、無い日の扱い、締め、月初と月末（B）
  sha256.rs  sjis.rs  sjis_table.rs                              （B）
  holidays.rs      csv と govuk の読み取り、covers（B）
  sources.rs       コピーと固定（B）。fetch・pin・outdated（C）
  calendar.rs      営業日の判定、データの範囲、慣行、営業日の数え方、休みの理由（B）
  interp.rs        参照インタプリタ（計算の段、時刻、条件）（B）
  paraphrase.rs    操作の言い直し（DESIGN 7.1）。eval・診断・doc が使う（B）
  check.rs         検査の五つの段、総当たり、予算、報告（B）
  diag.rs  codes.rs                                              （B）
  eval.rs  api.rs  naming.rs（出力先ごとの名前と署名）           （B）
  codegen/  mod.rs typescript.rs python.rs go.rs rust.rs sql.rs   （C。PLAN は gen/ としていたが、gen は Rust 2024 の予約語）
  vectors.rs  fetch.rs                                           （C）
  doc/  mod.rs markdown.rs html.rs months.rs edges.rs            （D）
tests/
  date.rs syntax.rs sources.rs calendar.rs check.rs mutants.rs codes.rs cli.rs api.rs design.rs   （B）
  targets.rs vectors.rs fetch.rs  common/mod.rs（子プロセス、一時ディレクトリ、PostgreSQL）  （C）
  doc.rs docs.rs skill.rs                                                               （D）
  fixtures/  mutants/  golden/
tools/sjis/make_table.py  tools/sjis/README.md                   （B）
tools/package.json  tools/package-lock.json  tools/requirements.txt   （C。tsc と mypy の版）
examples/calendars/*.cal  examples/calendars/data/  examples/*.cal（B で書き、D で仕上げる）
docs/  skills/  README.md  README.ja.md  THIRD_PARTY_NOTICES.md、LICENSE-MIT、LICENSE-APACHE（D。ライセンスは D.6）
```

## 2. 段階 B：言語の芯

字句・構文・型、カレンダー、参照インタプリタ、条件の検査、診断（英語と日本語）、CLI の `check`・`eval`・`explain`・`api`。項の順に進め、各項のテストはその項のうちに書く。

### B.1 土台

- `Cargo.toml`（name `koyomi`、edition 2024、依存は serde_json だけ）と `.gitignore`（`/target`）。lib と bin の両方を持つ。
- `src/i18n.rs`：`Lang { En, Ja }`。`--lang` があればそれ、無ければ `KOYOMI_LANG`、無ければ英語。システムのロケールは見ない。`tr!("日本語", "English")` は `Text { ja, en }` を返し、描くときに `Lang` を渡す。プロセス全体の言語を持たない（テストが英語と日本語の golden を同じプロセスで並行して描くため）。
- `src/kw.rs`：DESIGN 1.2 の表を一枚で持つ。予約語の判定と `explain` の一覧はここから引く。

### B.2 日付（`src/date.rs`）

- 型は `Day(i32)`（1970-01-01 からの通算日）。扱える範囲は 0001-01-01（−719,162）〜9999-12-31（2,932,896）。
- 関数：`days_from_civil` と `civil_from_days`（Howard Hinnant の手順）、`is_leap`、`month_len`、`weekday`（月曜を 0）、`parse`（`YYYY-MM-DD`。表の読み取りには `YYYY/M/D` も）、`format`、`shift_month`、`place`、`add_days`、`add_months`、`day_of_month`、`start_of_month`、`end_of_month`、`close_day`、`close_end_of_month`。
- 無い日の扱いは `EndOfMonth`、`StartOfNextMonth`、`Reject`。無い日に当たりえない操作には内部の `Never` を使い、当たったら内部のエラー（koyomi のバグ）にする。
- 結果は `Result<Day, DateError>`（`Missing { 年, 月, 日 }`、`OutOfRange`）。
- 定義は DESIGN 2.2 のとおり。`close_day` は前の月・その月・次の月の三つの締め日の候補のうち、z 以後で最も早いもの。
- テスト（`tests/date.rs`）：
  - 0001-01-01〜9999-12-31 の 3,652,059 日のすべてで、通算日から年月日にして戻すと元に戻る。
  - 曜日：1970-01-01 は木、2000-02-29 は火、2026-10-02 は金、0001-01-01 は月、9999-12-31 は金。通算日：2026-01-01 は 20454、2027-11-20 は 21142、1955-01-01 は −5479、2027-12-31 は 21183。
  - 月の足し算を、年月日を組み立てて有効かを見るだけの素朴な定義と比べる。1900〜2100 年（73,414 日）のすべての日、k = −24〜24、三つの扱い。
  - Catala の Theorem 3・4 にあたる性質（DESIGN 1.7）を同じ範囲で総当たり：二つの扱いはどちらも単調、`EndOfMonth` の結果 ≤ `StartOfNextMonth` の結果、無い日に当たらなければ三つの扱いの結果は同じ、`± n years` は `± 12n months` と同じ。
  - DESIGN 1.7 の表：koyomi の `else end_of_month` は、dateutil・Java・PostgreSQL・Temporal・chrono・macOS の列と五つの組で同じ結果（2023-02-28、2024-02-29、2023-04-30、二回で 2023-05-30、一度に 2023-05-31）。`else start_of_next_month` は 2023-01-31 の `+ 1 month` が 2023-03-01。
  - DESIGN 2.3 の成り立たない性質の四つの例（2023-05-30 と 2023-05-31、2023-02-28 と 2023-03-01、2023-03-30、2023-03-01）。
  - `close_day`：N = 1〜31 と三つの扱いで、1900〜2100 年のすべての日について、結果が z 以上で、全部の月の締め日を素朴に並べたときの z 以上の最小と同じで、単調であること。

### B.3 字句と構文（`src/lex.rs`、`src/parse.rs`、`src/ast.rs`）

- 字句：名前（Unicode の文字・数字・`_`。数字で始まらない）、別名 `(ascii)`、文字列、日付 `YYYY-MM-DD`、月日 `MM-DD`、時刻 `HH:MM`、オフセット `±HH:MM`、整数、符号つきの数と名前（`+1`、`-1`、`+支払の月`）、`sha256:<16 進>`、`..`、`->`、`|`、`:`、`,`、`@`（法令の引用）、比較（`=`、`<`、`<=`、`>`、`>=`）、`#` から行末までのコメント。どの字句も行と列を持つ（列は文字で数える）。
- 構文：DESIGN 1.1 の二種類のファイルと節の順序、字下げ（スペース。タブは E005）、`date` の操作（DESIGN 1.6 の表と 1.8 の `if closed`）、`at`、`claims`（DESIGN 1.10）、`examples`（rulec と同じパイプの表）。
- E001〜E006 を出す。
- テスト（`tests/syntax.rs`）：DESIGN に出てくる `.cal` が全部構文を通る（1.1 の二つと 4.3 の二つはそのまま。1.7 の民法の抜き出しと 1.10 の claims は、見出しと入力を足したファイルにして）。E001〜E006 の変異ファイルが、そのコードを出す。

### B.4 名前と型（`src/resolve.rs`、`src/reserved.rs`）

- 名前の表と別名の決まり（DESIGN 1.2）、E007〜E015。
- `use calendar`：`.cal` からの相対パス、calendar のファイルであること、循環、オフセットの食い違い（E015）。
- 日付の引数：始まりをたどって使う入力（DESIGN 1.3）。
- 出力先の予約語（`src/reserved.rs`）：TypeScript（ECMAScript の予約語と strict mode の予約語）、Python（`keyword.kwlist` と `keyword.softkwlist`）、Go（25 のキーワードと事前宣言の識別子）、Rust（strict と reserved のキーワード）、PostgreSQL（文書の付録 C の reserved）。表の頭に、どの版のどの文書から転記したかを書く。生成物が使う名前（`is_open`、`Date`、`KoyomiError`、`<別名>_at`）とぶつかる別名も E009。
- 書いたものから決まること（DESIGN 3.1 の 4 段め）：E201、W201、W202。E201 の例の入力は、その操作の前までを範囲の入力で順に計算して探す（DESIGN 4.3）。
- テスト：E007〜E015、E201、W201、W202 の変異ファイル。E201 は DESIGN 4.3 のファイルで、例の入力が 2026-01-29 になること。

### B.5 出典と祝日の表（`src/sha256.rs`、`src/sjis.rs`、`src/sjis_table.rs`、`src/holidays.rs`、`src/sources.rs`）

- SHA-256 を自前で書く。FIPS 180-4 の既知の値でテストする（手元の `shasum -a 256` で確かめた値）：
  - 空 → `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`
  - `abc` → `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`
  - `abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq` → `248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1`
  - `a` を 1,000,000 個 → `cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0`
- Shift_JIS：WHATWG Encoding Standard の Shift_JIS decoder の手順どおりに読む。一バイトの 0x00〜0x80 はそのまま、0xA1〜0xDF は半角カナ（U+FF61 から）。二バイトは lead が 0x81〜0x9F と 0xE0〜0xFC、trail が 0x40〜0x7E と 0x80〜0xFC で、pointer = (lead − (lead < 0xA0 ? 0x81 : 0xC1)) × 188 + trail − (trail < 0x7F ? 0x40 : 0x41)。pointer が 8836〜10715 なら U+E000 + (pointer − 8836)、ほかは表を引く。読めなければ置き換え文字にせず、E104 でバイトの位置を言う。
  - `tools/sjis/make_table.py <index-jis0208.txt>`：索引の SHA-256 が `341dcde7e8b984e9c7bbf5ed75c8da7c6087d47083a1a2b3ed558bfd5bef9468`（Identifier `cbaa91f3deb7d0841faf5c33041fc15a285da0e87e64ab802c4bf04b7c4da861`、Date 2024-09-18）であることを確かめ、pointer 0〜11103 の文字を並べた一つの文字列（空きは U+FFFF。UTF-8 で 33,190 バイト）を `src/sjis_table.rs` に書く。表の頭に、索引の URL（`https://encoding.spec.whatwg.org/index-jis0208.txt`）、Identifier、日付、SHA-256、CC BY 4.0 を書く。索引そのものはリポジトリに入れず、`tools/sjis/README.md` に取り方を書く。索引が更新されていて SHA-256 が違えば、DESIGN 1.5 の Identifier と日付を書き換え、報告で言う。
- 形式（`src/holidays.rs`）：
  - `csv`：UTF-8 か Shift_JIS。一行目の最初の値が日付でなければ見出しとして飛ばす。`"…"` の引用、CR LF と LF、`YYYY-MM-DD` と `YYYY/M/D`、名前は無くてもよい。同じ日付が二度あれば E104。
  - `govuk`：serde_json で読み、地域を選ぶ。名前は `title` に、`notes` があれば括弧で添える（`Boxing Day (Substitute day)`）。
- `covers`（日付の区間と `listed years`）、E101〜E106（`src/sources.rs`）。E102 の直し方は、コピーから計算した `sha256:…` そのもの。
- 法令の出典（DESIGN 1.5 の「法令の出典」）：`source <名前> = law "<法令ID>" asof <日付>` と、その下の条の固定の行（`第143条 sha256:…`）。条の名前（条・項・号。算用数字と漢数字）から e-Gov の要素（`MainProvision-Article_143-Paragraph_2-Item_1`）とコピーのファイル名を作る（rulec の `src/sources.rs` の `egov_fragment` と同じ作り方。附則と別表は E111 でエラーにする）。コピーは `.cal` の隣の `sources/law/<法令ID>@<日付>/<要素>.xml`。引用 `@<出典> <条>[, <条>…]` は、`date` の宣言・操作・`at`・条件・`closed`・`open` の行の末尾。E111、W102、コピーの無い条は E101、固定と違えば E103。コピーの XML からタグを落として本文にする関数（rulec の `xml_text` と同じ考え。段落と文ごとに改行）も、ここで書く（D の doc が使う）。
- 祝日の表のコピー：`examples/calendars/data/syukujitsu.csv`（SHA-256 `cec37a743c96995cdb9cb52b685c9003634682a9b0e1a640a6b9b96881fe964a`）と `examples/calendars/data/bank-holidays.json`（SHA-256 `538b3482c28b85ecd2db606a0d5ae6ad17248900b6498700ce0a48d26a3ecde6`）は、A の段階で 2026-10-02 に取って置いた。DESIGN の数はこのバイト列から出したので、取り直さずに使う（内閣府は 2027 年 2 月に 2028 年の分を足し、GOV.UK は古い年を落とすので、取り直すと数が変わる）。
- テスト（`tests/sources.rs`）：
  - 内閣府の CSV：1,067 行。1955〜2027 年の 73 年で、どの年にも行がある。最初の行は 1955-01-01 元日、最後の行は 2027-11-23 勤労感謝の日。名前が「休日」の行は 116。
  - python3 があれば、CSV 全体を Python の `cp932` で読んだ文字列と koyomi が読んだ文字列が一致し、7,724 個の符号のどれもが `cp932` と同じ文字になる（無ければ SKIP）。
  - GOV.UK：`england-and-wales` は 83 件、2019〜2028 年、土日の日付は 0 件。
  - E101〜E106 の変異（コピーが無い、固定が無い、固定が違う、読めないバイト、`covers` の外の行、行の無い年）。
  - 法令：`examples/sources/law/129AC0000000089@2026-10-01/` の四つのコピーの固定が、140 条 `e880059021fbb67d`、141 条 `0575c131b9f08063`、142 条 `fc8c35a0769d3b35`、143 条 `6950bdfb988439b6` になる。143 条の本文が「週、月又は年によって期間を定めたときは、その期間は、暦に従って計算する。」で始まる。条の名前から e-Gov の要素の名前を作ること（`第143条`、`第百四十三条第二項`、`第20条の2第3項第4号`）。E111 と W102 の変異。

### B.6 カレンダー（`src/calendar.rs`）

- DESIGN 1.4：休みの曜日、毎年の休み（年をまたぐ区間）、特定の日、表、例外の営業日、`use calendar` で合わせる、オフセット。
- E107：`offset` の値が `±HH:MM` でなければエラーにする。`Asia/Tokyo` のように `/` を含むもの、`UTC` と `GMT` 以外の英字のものは「タイムゾーンの名前」として、DESIGN 1.9 の理由を言う。E108、W101。
- `is_open(day) -> Result<bool, Outside>` と、休みの理由（曜日、表の行の名前、毎年の休みの名前、特定の日の名前）。
- 四つの慣行と、営業日の数え方（DESIGN 1.8 と 2.2。0 のときは寄せる）。問い合わせるたびにデータの範囲を確かめる。
- いちばん長い連休と、年ごとの営業日の数（D の doc が使う。ここで書いてテストする）。
- テスト（`tests/calendar.rs`）：
  - 東京の営業日：2026 年の営業日は 240 日。2026〜2027 年のいちばん長い連休は 2026-12-29〜2027-01-03（6 日）。2026-05-04（みどりの日）と 2026-05-06（休日）は休み、2026-05-08 は営業日。2028-01-10 はデータの範囲の外。
  - 表の無い、土日休みのカレンダー：土曜 2026-10-03 の `+ 1 business day` は 2026-10-05、`+ 0 business days` は 2026-10-05、`- 0 business days` は 2026-10-02。金曜 2026-10-02 の `+ 1 business day` は 2026-10-05（DESIGN 1.8 の QuantLib 1.43 の結果と同じ）。
  - E107（`offset Asia/Tokyo`）、E108、W101 の変異。

### B.7 参照インタプリタ（`src/interp.rs`、`src/paraphrase.rs`）

- 入力の組を一つ受け取り、すべての日付を順に計算し、操作ごとに（行、操作、前の日付、後の日付、注）を残す。注は動いた理由を言う（「2026-05-10 は日曜、2026-05-09 は土曜で休み」）。
- エラー：無い日（`Reject`）、データの範囲の外（どの日を問うたか）、0001〜9999 の外。
- `at`：オフセットで、RFC 3339 の UTC（`Z`）の形とオフセット付きの形。`at end of day` は次の日の 00:00。
- 条件の値：入力の組ごとの真偽と、比較なら左右の差の日数（報告の「いちばん外れる」「いちばん余裕の無い」に使う）。単調性は隣り合う日の組。
- `src/paraphrase.rs`：DESIGN 7.1 の表を英語と日本語で。eval と診断の計算の段もここから引く。
- テスト：DESIGN 5 章の `eval` の例（受領日 2026-04-01）が同じ段と時刻を出す。DESIGN 1.1 の examples の二行。

### B.8 検査（`src/check.rs`）

- DESIGN 3.1 の五つの段。総当たりでは、整数の入力を外側（宣言の順に、小さい値から）、日付を内側（昇順）に回す。「最初の入力」はこの順で最初のもの。
- 予算（E305）：既定の値は測って決める。(a) `締め日と支払日を受け取る.cal`（871,596 通り）と、(b) 100 年 × 整数 31 × 12 の合成のファイル（約 1,360 万通り）の検査の時間を測り、既定の予算いっぱいの検査がこの機械で 10 秒ほどで終わる値にする。測った秒数と決めた値で、DESIGN 3.2 の「目安は 10⁷」を書き換える。
  - B で測った：(a) 0.07 秒、(b) 13,587,300 通りで 0.92 秒、(b) に 60 営業日の条件を足すと 3.5 秒。既定は 10⁸ にした（DESIGN 3.2）。成り立たない入力のまとまりは、一つの診断につき 100 万か所まで覚える（`check::MAX_RUNS`。DESIGN 3.3）。
- 報告（DESIGN 3.3、4.3）：続いた日をまとめ、文面には六か所まで。整数の入力の値の組ごとにまとめる。最初の入力の計算の段。比較の条件なら、いちばん外れる入力。E203 の直し方は、外に出る最初の入力の前日で終わる `range`。
- 通ったときの一行（DESIGN 3.1）。
- テスト（`tests/check.rs`）：B.12 の数を全部。

### B.9 診断と台帳（`src/diag.rs`、`src/codes.rs`）

- `Diag`：code、severity、line、col、message（`Text`）、notes（`Text` の並び）、inputs、steps、fails、fix。文面と JSON は DESIGN 4.1 の形（dandori の `src/diag.rs` の描き方を手本にする）。
- `src/codes.rs`：DESIGN 4.2 の全コード。各コードに、見出し、いつ出るか、どう直すか（英語と日本語）、走る最小の再現（`.cal` の本文。コピーが要るコードは、小さな CSV の本文も持つ）。
- `koyomi explain <CODE>`、`koyomi explain --all`、`--format markdown`。
- テスト：`tests/codes.rs`（全コードの再現が、そのコードを出す。台帳のどのコードにも変異ファイルが一つ以上ある）、`tests/mutants.rs`（`tests/mutants/<CODE>_<内容>.cal` ごとに、英語と日本語の golden `tests/golden/<同じ名前>.en.txt`・`.ja.txt` と一致する）。

### B.10 CLI と api（`src/cli.rs`、`src/main.rs`、`src/eval.rs`、`src/api.rs`、`src/naming.rs`）

- DESIGN 5 章：コマンドとフラグを一枚の表に置き、`koyomi --help`、`koyomi <cmd> --help`（`koyomi help <cmd>` も同じ）、`koyomi --version` を出す。exit code は 0・1・2。知らないフラグ、閉じた集合の外の値、値の無いフラグ、二度書いたフラグは exit 2。引数なしの `koyomi` は使い方を標準エラーに出して exit 2。
- B で実装するコマンドは `check`、`eval`、`explain`、`api`。まだ実装していないコマンドを表に載せない（C と D が足す）。
- `eval`：DESIGN 5 章の形と `--format json`。calendar のファイルは日付を一つ受け取り、営業日か休みか（休みの理由）を言う。
- `api`：DESIGN 8 章。出力先ごとの名前と署名は `src/naming.rs` で決め、C の生成器も同じ関数を使う（api と生成物が別々に名前を決めないように）。Go のパッケージ名は別名から `_` を除いたもの、関数は別名をパスカルケースにしたもの（`payment` → `Payment`、`payment_at` → `PaymentAt`、`is_open` → `IsOpen`）。
- テスト：`tests/cli.rs`（exit code、全コマンドの `--help`、知らないフラグ、`KOYOMI_LANG`）、`tests/api.rs`（例ごとの api の golden）。

### B.11 例の `.cal`

DESIGN 10 章の例を `examples/` に書く。D の段階で仕上げるが、B の完了の条件がこれを使う。

- `examples/calendars/東京の営業日.cal`（DESIGN 1.1 のまま）、`民法142条の休日.cal`（`closed weekly sun` と `closed 祝日`。description に「142 条が名指しする日だけを休みにした。『その他の休日』に何が入るかは、この例では決めない」）、`england_and_wales.cal`（`closed weekly sat, sun`、`source` は `format govuk "england-and-wales"` と `covers listed years`、オフセットは書かない）。
- `examples/支払_20日締め翌月10日払い.cal`（DESIGN 1.1 のまま）、`支払_月末締め翌々月末払い.cal`（DESIGN 4.3 のまま）。
- `examples/民法の期間.cal`：カレンダーは `民法142条の休日.cal`。`source 民法 = law "129AC0000000089" asof 2026-10-01` と、140・141・142・143 条の固定の行（値は B.5 のテストのとおり。コピーは A の段階で置いてある）。入力は `起点 : date range >=2026-01-01 <=2026-12-31` と `月数 : int range >=1 <=12`。日付は `起算日`（`@民法 第140条`）、`満了日`（DESIGN 1.7 の書き方。`@民法 第141条, 第143条`）、`満了日_142条`（`満了日` から `if closed + 1 day`。`@民法 第142条`）。`at end of day` は書かない（カレンダーにオフセットが無い）。条件は `満了日 is monotonic`、`満了日_142条 is monotonic`、`満了日 > 起点`、`満了日_142条 >= 満了日`。
- `examples/民法の期間_読み方の比較.cal`：同じカレンダー、同じ出典、同じ入力。`満了日`、`満了日_翌日`（`if closed + 1 day`）、`満了日_翌営業日`（`roll following`）、`月数を足して寄せる`（`起点` から `+ 月数 months else end_of_month`）。条件は `二つの読み方 : 満了日_翌日 = 満了日_翌営業日` と `月末に寄せる書き方 : 満了日 = 月数を足して寄せる`。
- `examples/締め日と支払日を受け取る.cal`：カレンダーは `東京の営業日.cal`。入力は `受領日 : date range >=2026-01-01 <=2027-10-01`、`締め日 : int range >=1 <=31`、`支払の月 : int range >=1 <=2`、`支払の日 : int range >=10 <=31`。`締め`（`close day 締め日 else end_of_month`）、`支払`（`締め` から `day 支払の日 of month +支払の月 else end_of_month`、`roll preceding`）。条件は `支払 is open`、`支払 > 締め`、`支払 is monotonic`。
- `examples/net30.cal`：英語の名前。`invoice_date : date range >=2026-01-01 <=2028-11-29`、`due`（`+ 30 days`、`roll following`）、条件は `due is open`、`due >= invoice_date + 30 days`、`due is monotonic`。カレンダーは `england_and_wales.cal`。
- どの description にも、DESIGN 10 章の注意書き（例の条件として文字どおりに書いた。法令の読み方を決めない）を入れる。

### B.12 B の完了の条件

`cargo build` が警告なしで通り、`cargo test -- --nocapture` が全部通ること。SKIP は python3 が無いときの Shift_JIS の比べ合わせだけで、この機械では 0。そのうえで、次の数が出ること。どれも A の段階の試作で出した数である。違えば、どちらが正しいかを DESIGN 2.2 の定義で決め、試作の側が誤っていたなら DESIGN の数を直して報告する。

| 対象 | 出ること |
|---|---|
| `東京の営業日.cal` | 表 1,067 行、データの範囲 1955-01-01〜2027-12-31。2026 年の営業日 240 日。2026〜2027 年のいちばん長い連休は 2026-12-29〜2027-01-03（6 日） |
| `支払_20日締め翌月10日払い.cal` | ok。689 日。受領から支払までがいちばん長いのは 51 日（受領日 2026-07-21、2026-12-21、2027-07-21）、いちばん短いのは 18 日。`roll preceding` が動かす支払日は六つ（2026-05-10→2026-05-08、2026-10-10→2026-10-09、2027-01-10→2027-01-08、2027-04-10→2027-04-09、2027-07-10→2027-07-09、2027-10-10→2027-10-08） |
| 同じファイルの範囲を `<=2027-12-31` にしたもの | E203。最初の入力 2027-11-21（日）、問う日 2028-01-10、同じことが起きる受領日は 41 日（2027-11-21〜2027-12-31）、直し方 `range >=2026-01-01 <=2027-11-20`（DESIGN 4.3） |
| `支払_月末締め翌々月末払い.cal` | E301。669 日のうち 648 日。続いた日は 10 か所：2026-01-01〜01-29（29）、2026-02-01〜03-29（57）、2026-04-01〜08-30（152）、2026-09-01〜10-28（58）、2026-11-01〜11-29（29）、2026-12-01〜12-27（27）、2027-01-01〜01-29（29）、2027-02-01〜05-30（119）、2027-06-01〜08-29（90）、2027-09-01〜10-28（58）。いちばん外れるのは受領日 2026-05-01、支払日 2026-07-31 の 91 日。最初の日は 2026-01-01 で 89 日。2026-12-31（木）の支払日は 2026-12-28（月）に動く。範囲を 2027-11-01 まで広げると E203（2028-01-31 を問う） |
| `民法の期間.cal` | ok。4,380 通り。結果は 2027-12-31 まで |
| `民法の期間_読み方の比較.cal` | E301 が二つ。`二つの読み方` は 4,380 通りのうち 121 通り（月数ごとに 1:11、2:10、3:11、4:11、5:11、6:11、7:11、8:11、9:9、10:9、11:8、12:8）。最初は月数 1、起点 2026-01-22 で、満了日 2026-02-22（日）、翌日 2026-02-23（天皇誕生日）、翌営業日 2026-02-24。月数 1 の 11 日は 2026-01-22、2026-04-03〜04-05、2026-06-19、2026-08-20〜08-22、2026-09-11、2026-10-22、2026-12-10。`月末に寄せる書き方` は 39 通り（1:5、2:3、3:3、4:5、5:1、6:5、7:2、8:4、9:4、10:2、11:5、12:0）。最初は月数 1、起点 2026-02-28 で、満了日 2026-03-31、寄せた結果 2026-03-28。月数 1 の 5 日は 2026-02-28、04-30、06-30、09-30、11-30 |
| `締め日と支払日を受け取る.cal` | ok。871,596 通り。受領から支払までがいちばん長いのは 121 日（受領日 2026-05-02、締め日 1、支払の月 2、支払の日 31 で支払 2026-08-31）。範囲を 2027-10-02 まで広げると E203（2028-01-10 を問う） |
| `net30.cal` | ok。1,064 日。いちばん長いのは 34 日で、請求日 2026-03-04、2026-11-25、2027-02-24、2027-11-25、2028-03-15、2028-11-23 の六日。範囲を 2028-11-30 まで広げると E203（2029-01-01 を問う）。`roll modified following` の日付を足すと following と 21 日で分かれ、最初は 2026-01-01（30 日後の 2026-01-31 は土曜。following は 2026-02-02、modified following は 2026-01-30） |
| `eval` | DESIGN 5 章の例と同じ段（受領日 2026-04-01 → 締め日 2026-04-20 → 2026-05-10 → 2026-05-08、時刻 2026-05-08T00:00:00Z） |
| E201 | DESIGN 4.3 のファイルで、例の入力が 2026-01-29、`2026-02-29`、直し方の結果が 2026-02-28 と 2026-03-01 |
| 診断 | DESIGN 4.2 の全コードに変異と再現があり、英語と日本語の golden と一致する |

報告には、測って決めた予算の既定値と、そのときの時間を書く。

B の結果：上の表の数は、どれも参照インタプリタが試作と同じに出した（`tests/check.rs`。一つも食い違わなかったので、DESIGN の数は直していない）。`cargo test -- --nocapture` は 60 のテスト（11 のテストのバイナリ）が通り、SKIP は 0（python3 があるので、Shift_JIS の比べ合わせも走った）。DESIGN に貼った出力は `tests/design.rs` が走らせて確かめる。

## 3. 段階 C：出力先と突き合わせ

五つの出力先の生成、`vectors`、突き合わせのテスト、`source fetch | pin | outdated`。

### C.1 生成の共通の部分（`src/gen/mod.rs`）

- DESIGN 6.1 のとおり。生成物の中に書く小さな関数と内部の名前は `_` で始める（B で決めた。別名は `[a-z]` で始まるのでぶつからず、E009 は公開する名前だけを予約している。`src/naming.rs` の `GENERATED`）。生成物の中に書く小さな関数は、B の `src/date.rs` と `src/calendar.rs` の手順を一つずつ移植する：`days_from_civil`、`civil_from_days`、`month_len`、`weekday`、`parse_date`、`format_date`、`shift_month`、`place`、`add_months`、`day_of_month`、`start_of_month`、`end_of_month`、`close_day`、`close_end_of_month`、`is_open`、`roll`、`add_business`、`at_utc`、入口のガード。言語ごとの綴りは `src/naming.rs`。
- エラーの種類を四つに分ける：`range`（入力が範囲の外）、`data`（データの範囲の外）、`reject`（無い日）、`date`（0001〜9999 の外）。どの言語でも、エラーから種類が取れるようにする。
- 頭（DESIGN 6.1）。生成物は決まった形にする（時刻、機械のパス、並びの揺れを含めない）。`--lang` でコメントの言語を変える。
- カレンダーの埋め込み：祝日は通算日の昇順の配列で、一行に一日、行末のコメントに `YYYY-MM-DD 名前`。
- 公開するもの：日付ごとの関数、`at` のある日付の `_at` の関数、`is_open`。
- ランナー：vectors を標準入力から読み、一行ずつ結果（またはエラーの種類）を書く。
- `gen` は検査を通らないファイルからは生成しない（診断を出して exit 1）。`gen --check` は、書く代わりにディスクの生成物と一字一句同じかを見て、違えば exit 1。
- CLI の表に `gen` を足す。

### C.2 TypeScript（`src/gen/typescript.rs`）

- `typescript/<別名>.ts` と `typescript/<別名>_runner.ts`。日付は `"YYYY-MM-DD"` の `string`。`KoyomiError extends Error` に `kind`。
- Node が型を剥がすだけで走る形（`enum`、実行時のコードを含む `namespace`、コンストラクタの引数でのプロパティ宣言を書かない）。
- 型の検査：`npm install --prefix tools typescript` で入れた `tsc --strict --noEmit`（`tools/package-lock.json` はコミットし、`tools/node_modules` は gitignore）。無ければ SKIP。

### C.3 Python（`src/gen/python.rs`）

- `python/<別名>.py` と `python/<別名>_runner.py`。日付は `datetime.date`。`KoyomiError(ValueError)` に `kind`。型注釈を付ける。
- 型の検査：`uv venv --python 3.13 tools/.venv` に mypy を入れて `--strict`（作り方は `tools/requirements.txt` の頭に書く。venv は gitignore）。無ければ SKIP。

### C.4 Go（`src/gen/go.rs`）

- `go/<パッケージ名>/<パッケージ名>.go`。`Date{Year, Month, Day int}`、`ParseDate`、`String()`。関数は `(Date, error)`、`_at` は `(string, error)`、エラーは `*KoyomiError`（`Kind` を持つ）。
- `gofmt -l` が何も出さないこと、`go vet` が通ること。
- ランナーは、テストの一時ディレクトリに go.mod を一つ作り、全部のパッケージを一度だけビルドする（`-trimpath`）。

### C.5 Rust（`src/gen/rust.rs`）

- `rust/<別名>.rs` と `rust/<別名>_runner.rs`（`#[path]` でモジュールを読む）。`Date`（`FromStr`、`Display`、`Copy`、`Ord`）。関数は `Result<_, KoyomiError>`（`kind` を持つ）。
- `rustc --edition 2021 -D warnings` と `--edition 2024 -D warnings` の両方で通す（生成物が利用者のクレートの版に依らないように）。

### C.6 SQL（PostgreSQL、`src/gen/sql.rs`）

- `sql/<別名>.sql`。`CREATE SCHEMA IF NOT EXISTS <別名>`、`<別名>.is_open(date)`、`<別名>.<日付の別名>(…) RETURNS date`、`<別名>.<日付の別名>_at(…) RETURNS timestamptz`。どれも `IMMUTABLE` の PL/pgSQL。エラーは `RAISE EXCEPTION … USING ERRCODE = '22023'` で、メッセージの頭に種類（`koyomi range:` など）。
- テストは使い捨てのクラスタ（0.2）で、vectors を `COPY … FROM STDIN` で入れ、結果を `COPY … TO STDOUT` で出す。`_at` は `to_char(… AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"')` で他と同じ形にして比べる。クラスタは Drop で止め、データのディレクトリを消す。

### C.7 vectors（`src/vectors.rs`）

- DESIGN 6.3 の形。検査を通らないファイルからは出さない（`gen` と同じ）。
- 両端の外の行：日付の入力の範囲の最初の日の前日と最後の日の翌日（整数の入力は最小の値）、整数の入力ごとに最小の値の一つ下と最大の値の一つ上（日付は範囲の最初の日）。期待は `"error":"range"`。
- calendar のファイル：データの範囲のすべての日と、両端の外の二日（`"error":"data"`）。表の無いカレンダーは 1900〜2100 年（73,414 日）で、両端の外の行は無い。
- CLI の表に `vectors` を足す。

### C.8 突き合わせのテスト（`tests/targets.rs`、`tests/vectors.rs`）

- 対象：`examples/` の検査を通るファイル全部（calendar の三つと、dates の四つ）と、`tests/fixtures/helpers_*.cal`。helpers は表の無いカレンダーで、すべての操作、三つの扱い、四つの慣行、`if closed`、`at`（`at end of day` も）、整数の入力を、1900〜2100 年の広い範囲で使う。Python のランナーで一つ 60 秒以内に収まる大きさに分ける。
- 五つの出力先のランナーに、vectors の全行を流して一行ずつ比べる。合わなければ、最初の 5 行の入力と、両方の結果を出して落ちる。
- 出力の印：出力先とファイルごとに `compared <出力先> <ファイル>: <行数> lines` を一行（`-- --nocapture` で数える）。
- ツールが無ければ SKIP。
- `tests/api.rs` に足す：api の署名が、生成物の中に一字一句ある。
- vectors の行数（両端の外の行を含む）：`支払_20日締め翌月10日払い` 691、`民法の期間` 4,384、`締め日と支払日を受け取る` 871,604、`net30` 1,066、`東京の営業日` と `民法142条の休日` 26,665、`england_and_wales` 3,655。
- 大きな vectors はディスクに書かず、パイプで流す。

### C.9 出典のコマンド（`src/fetch.rs`、`src/sources.rs` に足す）

- DESIGN 9 章。curl は `-fsSL` で、三度まで試す。`file://` も読める。
- `pin` は `sha256:` の 16 桁だけを書き換え、ほかは一字も変えない（テストでバイト列を比べる）。固定の無い `source` の行には ` sha256:…` を足す。
- `outdated` の文面（英語と日本語）：増えた日・消えた日・名前の変わった日（六件まで並べ、全部の数を言う）、`covers` の書き換え案、exit 1。変わっていなければ exit 0。
- テスト（`tests/fetch.rs`）：一時ディレクトリに、内閣府の CSV のコピーから 2027 年の行を除いた版（旧）と、元のコピー（新）を置く。`url` を新への `file://` にし、旧を固定した `.cal` で、`outdated` が 2027 年の 17 日が増えることと `covers` の案を言って exit 1、`fetch` が新のバイト列をそのまま書き、`pin` が固定を書き換え、`check` が通る。GOV.UK の JSON でも同じ形で（2028 年の 8 件を除いた旧を作る）。`KOYOMI_NET=1` のときだけ、本物の二つの url に `outdated` を走らせる（中身は問わず、走ること）。
- 法令（DESIGN 9 章）：`fetch` は引いている条ごとに `<KOYOMI_EGOV か https://laws.e-gov.go.jp/api/2>/law_data/<法令ID>?asof=<日付>&elm=<要素>&law_full_text_format=xml` を引き、JSON の `law_full_text`（base64）を戻した XML をコピーのファイルに、`revision_info.law_revision_id` を `revision.txt` に書く。本文（タグを落としたもの）が前のコピーと同じなら書き換えない。`outdated` は `law_revisions/<法令ID>` の `amendment_enforcement_date` が `asof` より後の版ごとに、その日を `asof` にして条を取り、本文を比べる。
- 法令のテスト（`tests/fetch.rs`）：テストの中に `std::net::TcpListener`（`127.0.0.1:0`）で小さな HTTP サーバーを立て、e-Gov の二つの API（`law_data` と `law_revisions`）に用意したレスポンスを返し、`KOYOMI_EGOV` にその場所を渡す。サーバーはテストの終わりに止める。用意するレスポンス：民法の 140〜143 条（`examples/sources/law/` のコピーを base64 にしたもの）と、後の版が五つある `law_revisions`。確かめること：`fetch` がコピーと同じバイト列を書く、`outdated` が五つの版で変わらないと言って exit 0、ある版の 143 条の本文を一文字変えたレスポンスにすると、その施行日と条を言って exit 1、属性だけを変えたレスポンスでは exit 0（本文で比べる）。`KOYOMI_NET=1` のときは、本物の e-Gov に `outdated` を走らせ、五つの版のどれでも変わらないと言うこと（2026-10-02 に確かめた結果。これが変われば、民法が改正されたということなので、DESIGN 1.5 の事実を書き直す）。
- `check` が通信しないこと：PATH から curl を外しても `check` が通る。
- CLI の表に `source` を足す。

### C.10 C の完了の条件

- `cargo test -- --nocapture` が全部通り、この機械で SKIP が 0（node、python3、go、rustc、PostgreSQL、curl がそろっている。tsc と mypy は tools に入れてから回す）。
- `compared` の行が、五つの出力先 × 対象のファイルの数だけあり、行数が C.8 の数と合う。
- `gen --check` が、例の生成物を書き換えたときに exit 1 になる。
- `koyomi api` の署名が生成物と一致する。
- 報告に、出力先ごとのツールの版（node、python3、mypy、tsc、go、rustc、PostgreSQL）と、テスト全体の時間を書く。

### C の結果

C.1〜C.10 を書いた順に作り、C.10 の条件を全部満たした（2026-10-03）。`cargo test -- --nocapture` は 82 のテスト（14 のテストのバイナリ）が通り、SKIP は 0、`compared` の行は 70（五つの出力先 × 14 のファイル）で、例の行数は C.8 の数と合った。五つの出力先とも vectors の全部の行（一つの出力先につき 5,340,627 行、合わせて 26,703,135 行）を突き合わせ、行を間引いていない。全体は 41 秒で、いちばん長いのは SQL の突き合わせ（24 秒）。ツールの版は、Node v23.11.0、tsc 7.0.2、Python 3.14.6、mypy 2.4.0（Python 3.13.11 の venv）、go 1.25.5、rustc 1.94.1、PostgreSQL 18.0。測った数と決めたことは DESIGN 6 章と 9 章にある。

計画と違えたこと（DESIGN の該当の節にも書いた）：

- C.1：生成器のモジュールは `src/codegen/`（`gen` は Rust 2024 の予約語）。小さな関数は、そのファイルの操作が使うものだけを書く。無い日の扱いと慣行は `.cal` の語を文字列で渡し、当たりえない操作には `"none"` を渡す。ランナーの出力は、日付の値と時刻を空白で区切った一行か `error <種類>`。
- C.1：小さな関数の言語ごとの綴り（`_closeDay` と `_close_day`）は、`src/naming.rs` ではなく出力先ごとの生成器に書いた。公開しない名前で、api と突き合わせる必要が無いからである。`src/naming.rs` は公開する名前と署名だけを持つ。
- C.1：生成物が使う名前を E009 でエラーにする範囲を広げた。Go の `err`、Python の生成物が呼ぶ組み込みの関数（`range`、`str` など）、PL/pgSQL の予約語（`begin`、`declare` など）、ファイルの別名にはモジュールとスキーマの名前（`json`、`std`、`pg_catalog` など）。B の `src/naming.rs` の `GENERATED` と `src/reserved.rs` に足し、`src/naming.rs` に `MODULES` を置いた。
- C.4：Go のランナーは別のディレクトリではなく、同じパッケージのテストのファイル `go/<パッケージ名>/<パッケージ名>_runner_test.go`（`TestMain` が `KOYOMI_RUNNER` を見る）。import のパスが要らない。テストは一時ディレクトリに go.mod を一つ書き、`go test -c -trimpath -o bin/ ./...` で全部のパッケージを一度にビルドする。
- C.6：式一つの小さな関数は SQL 標準の関数本体にし、`STRICT` を付けない（付けるとプランナーが展開しない）。`STRICT` を外して SQL の突き合わせは 33.7 秒から 22.5 秒になった。
- C.7：範囲の外の行は、どれかの日付の関数が受け取る入力にだけ出す（受け取らない入力にはガードが無い）。
- C.8：helpers は `tests/fixtures/` の七つ（表の無いカレンダー `calendars/休みの書き方を全部使う.cal`、`helpers_月を足す`、`helpers_月を引く`、`helpers_月の日と締め`、`helpers_営業日`、`helpers_エラーにする`、`helpers_日付が一つも無い`）。Python のランナーでいちばん長いものも 11 秒ほどで、60 秒の目安より十分短い。`else reject` は 2 月 29 日を含む範囲では検査が止めるので、`helpers_エラーにする` だけ範囲を 2001-03-01〜2003-12-31 にした。
- C.8：`--lang ja` の生成物も同じツールで確かめ、二つのファイル（`支払_20日締め翌月10日払い.cal` と `東京の営業日.cal`）はそのランナーでも全部の行を突き合わせる（計画には無かった。日本語のメッセージの文字列が構文を壊していないことを確かめるため）。この行は `compared` ではなく `<出力先> --lang ja: … agrees on its <n> lines` と出す。
- C.8：tsc は TypeScript 7.0.2 で、`--erasableSyntaxOnly` も付けて、型を剥がすだけで走る形を確かめる。mypy の venv は、作者の機械では作業場所に作り、`KOYOMI_MYPY` で渡した（`tools/.venv` に作れば何も渡さずに見つかる）。
- C.9：`fetch` は、引いている条に加えて固定している条も取る（固定だけが残った条（W102）のコピーも、check が読むため）。`pin` は、引いているのに固定の行が無い条（E111）に、固定の行を足す。`KOYOMI_NET=1` でないときの本物への問い合わせは、`SKIP:` ではなく `not asked:` の行を出す（ツールが無いのではなく、通信しないことを既定にしているから）。本物の内閣府、GOV.UK、e-Gov には C の段階で一度だけ問い合わせた（DESIGN 9 章）。

## 4. 段階 D：doc、例、README、スキル

### D.1 doc（`src/doc/`）

- DESIGN 7 章のとおり、Markdown と HTML（`--format html`）。HTML は一枚で外のファイルを読まず、明るい配色と暗い配色（`prefers-color-scheme`）を持つ。
- 言い直しは `src/paraphrase.rs` からだけ作る（B で書いたもの）。法令を引いている日付・操作・条件には、コピーの本文（B.5 のタグを落とす関数）と、時点と版（`revision.txt`）を添える。HTML では `<blockquote>`。
- エッジケースの選び方（`src/doc/edges.rs`）と月の表（`src/doc/months.rs`）は DESIGN 7 章のとおり。今日の日付を使わない。
- 検査を通らないファイルの扱い（E301〜E303 だけならページを作って光らせ、ほかのエラーなら作らない）。
- CLI の表に `doc` を足す（`--months`）。
- テスト（`tests/doc.rs`）：例ごとの Markdown を英語と日本語の golden で比べる。HTML は、外の URL を読まないこと、成り立たない日の印の数が検査の数と合うこと、どの祝日の名前も、コピーした表にあること、どの言い直しの文も `paraphrase.rs` の表にあること。Chrome があれば、`支払_月末締め翌々月末払い.cal` のページの明るい配色と暗い配色のスクリーンショットを撮り（時間を区切って kill する）、README に使う（`docs/images/`）。無ければ SKIP。

### D.2 例

- B.11 の例を仕上げる。description、行末のコメント（どの操作が条文や契約のどこにあたるか。法令の解釈を書かない）。
- 英語の版を二つ足す：`examples/payment_20th_close_next_10th.cal`（`支払_20日締め翌月10日払い.cal` と同じ決まりを英語の名前で）と `examples/eom_close_two_months_later.cal`（`支払_月末締め翌々月末払い.cal` の英語の版）。カレンダーは同じ `東京の営業日.cal`。テストで、英語の版と日本語の版の vectors が、名前を読み替えれば一行も違わないことと、わざと破る版の診断の日付が同じことを確かめる。
- 例の一覧と、どれがわざと破る例か（`koyomi check examples/*.cal` を落とすもの）を README に書く。

### D.3 docs/

- `docs/reference.md`（英語）：文法の全部、操作と無い日の扱い、営業日、時刻、条件、カレンダーと出典、コマンド、exit code、JSON の形。
- `docs/codes.md` と `docs/codes.ja.md`：`koyomi explain --all --format markdown`（`--lang ja`）の出力そのもの。手で直さない。
- `docs/generated-code.md`：出力先ごとの形、呼び方、エラー、ランナー。
- テスト（`tests/docs.rs`）：codes の二つがいまの出力と同じ。文書の中の ```` ```cal ```` の塊は、実在するファイルの抜き出しか、検査を通るもの。診断の塊は golden にある。コマンドの出力の塊は、走らせた出力と同じ。

### D.4 README.md と README.ja.md

- README.md（英語）：rulec の README の並びにならう。看板（DESIGN 0.1）、何か、`.cal` の例（英語の版）、わざと破る例の診断（本物の出力）、生成したコード（本物の抜き出し）、確かめることと確かめないこと（DESIGN 3.4）、入れ方（`cargo install --path .`）、コマンド、例の一覧、どう確かめているか（突き合わせと SKIP）、承認する人のページ（スクリーンショット）、DESIGN.md への案内、ライセンス。
- README.ja.md：英語の訳ではなく、普通の日本語で一から書く。例は日本語の版。
- どちらも、法令に触れる例について DESIGN 10 章の注意書きを書く。

### D.5 スキル（`skills/koyomi/`）

- dandori の `skills/dandori` を手本にする。`SKILL.md` は手で書く（frontmatter に name、description、compatibility、license。いつ使うか、ループ（書く → check → 直す → eval → doc で人に見せる → gen）、言語の一ページ、人に聞くこと（無い日の扱い、どの休みのカレンダーか、法令の読み方、範囲）、診断から直し方、出力先）。
- ほかのファイルは `skills/sync.sh` が `docs/` からコピーする（リンクはスキルのディレクトリの外へ出ないように書き換える）。`skills/README.md` に入れ方。
- テスト（`tests/skill.rs`）：sync.sh を一時ディレクトリに走らせた結果が、スキルに置いたコピーと同じか、スキルの中のリンクが外へ出ないか、frontmatter。

### D.6 ライセンスと出典

- ライセンスは作者が決めた（2026-10-03）。rulec と dandori と同じ MIT OR Apache-2.0 で、LICENSE-MIT と LICENSE-APACHE を置き、`Cargo.toml` の `license`、README の「ライセンス」の節、スキルの frontmatter にも書く。決まるまでは、どれも置かずにいた。
- `THIRD_PARTY_NOTICES.md`：内閣府の祝日の CSV（公共データ利用規約（第1.0版）、出典の書き方）、GOV.UK の bank holidays（Open Government Licence v3.0）、WHATWG の `index-jis0208.txt` から作った表（CC BY 4.0）、e-Gov から取った民法の条文のコピー（e-Gov の利用規約をこの段階で読み、書き方を決める。dandori の THIRD_PARTY_NOTICES.md の書き方にそろえる）。

### D.7 D の完了の条件

- `cargo test -- --nocapture` が全部通り、この機械で SKIP が 0（Chrome のスクリーンショットも撮れる）。
- 例ごとの doc の golden（英語と日本語）があり、HTML のテストが通る。
- README.md と README.ja.md の中のコード、診断、出力が、`tests/docs.rs` の確かめを通る。
- `docs/codes.md` と `docs/codes.ja.md` がいまの `explain` の出力と同じ。
- スキルのテストが通る。
- 報告に、README に貼った出力をどのコマンドで取ったかを書く。

### D の結果

D.1〜D.7 を書いた順に作り、D.7 の条件を全部満たした（2026-10-03）。`cargo test -- --nocapture` の件数と SKIP の数は、下の「D の終わりの実行」にある。測った数と決めたことは DESIGN 7 章、10 章、11 章、12 章にある。

計画と違えたこと（DESIGN の該当の節にも書いた）：

- D.1：ページは一度ブロックの並び（`Page`）に組み、Markdown と HTML はそれを書き出すだけにした（DESIGN 7 章）。月の表は月曜から始める。エッジケースの日数のいちばん多い入力と少ない入力は、ほかの日付の始まりにならない日付（結果）に限り、同じ入力で同じ日数の日付はまとめて言う（DESIGN 7.3）。24 か月を超える dates のファイルでは、エッジケースの入力の月に加えて、エッジケースの計算した日付の月も出す（DESIGN 7.2）。表を読まないカレンダーは `--months` を書いたときだけ月の表を出す。
- D.1：`koyomi doc` は、条件が成り立たなくてもページを出せば exit 0、ページを作れないときは診断を標準エラーに出して exit 1（DESIGN 7.4）。`--format` は `markdown|html`（既定は markdown）。
- D.1：法令の条文は、`xml_text`（文ごとに一行。`source outdated` が比べる）ではなく、項ごとに一行の `sources::article_lines` で引く（DESIGN 7.5）。
- D.1：スクリーンショットは、ヘッドレスの Chrome がフラグで暗い配色に替わらなかったので、ページをコピーし、その `<html>` に `data-theme` を書いて撮る。わざと破る二つの例（英語の版と日本語の版）の、ページの先頭（明るい配色）と月の表（暗い配色）の四枚を `docs/images/` に置いた。
- D.2：例の行末のコメントを、支払の例は支払条件の言葉、民法の例は条文の言葉か読み方の名前にした。英語の版の日付の名前は、日本語の版の別名と同じにした（生成した関数の名前が同じになる）。生成したコードの説明は、名前が別名と同じときは別名を繰り返さないようにした（`Gives payment.`。`codegen::named`）。
- D.3：README と `docs/` とスキルの `.cal` の塊は、`examples/` か `tests/` の `.cal` の行でなければならない（`…` で切ってよい）。コマンドの塊は、出力を載せたものだけを走らせて比べ、出力の無い一覧は、コマンドの名前があることだけを確かめる。比べた行の数は、テストが走らせる vectors の行の数を、行を作らずに数えて確かめる（`tests/docs.rs`）。DESIGN の ```` ```markdown ```` の塊も、`tests/design.rs` がページの golden の抜き出しかを確かめる。
- D.5：スキルの frontmatter の `license` は、作者がライセンスを決めるまで書かなかった。決まったあと（2026-10-03）に MIT OR Apache-2.0 と書き、`tests/skill.rs` は、それが `Cargo.toml` と同じであることを確かめる。
- D.6：WHATWG の索引は CC BY 4.0 だが、ソースコードに取り込んだ部分は BSD 3-Clause License になる、と Encoding Standard が書いているので、`src/sjis_table.rs` の頭（`tools/sjis/make_table.py` が書く）と THIRD_PARTY_NOTICES.md をそう書いた。頭の文を書き換えるため、索引を作業場所に一度取り、スクリプトで表を作り直して、表の中身が一字も変わらないことを確かめた（索引はリポジトリに置かない）。データのコピー（内閣府、GOV.UK、e-Gov）は取り直していない。

D の終わりの実行：`cargo test -- --nocapture` は 99 のテスト（`tests/` の 16 のファイルと、ライブラリの中のテスト）が通り、SKIP は 0（`not asked:` が一行。`KOYOMI_NET=1` でないので本物の内閣府、GOV.UK、e-Gov には問い合わせない）。`compared` の行は 75（五つの出力先 × 15 のファイル）で、出力先ごとに 5,341,318 行、合わせて 26,706,590 行を突き合わせた。全体は 40 秒。Chrome のスクリーンショットも撮れた（`KOYOMI_CHROME`）。

## 5. 次の段階への申し送り

### 5.0 D のあとへ（D の段階で書いた）

- コミットの前に、作業ツリーに残したものを読む。D の段階で足したのは、`src/doc/`、`docs/`（`images/` の四枚を含む）、`skills/`、`tests/doc.rs`・`tests/docs.rs`・`tests/skill.rs`、`tests/golden/doc/`、README.md、README.ja.md、THIRD_PARTY_NOTICES.md、例の英語の版二つ。
- ページの文面を変えたら `KOYOMI_BLESS=1 cargo test --test doc` で golden を取り直し、差分を読む。スクリーンショットも同じコマンドで撮り直す（Chrome は `KOYOMI_CHROME`）。
- `docs/` を変えたら `skills/sync.sh` を走らせる。`docs/codes.md` と `docs/codes.ja.md` は、診断の台帳を変えたら `koyomi explain --all --format markdown [--lang ja]` で書き直す。
- README の「どう確かめているか」の時間とテストの件数は、一度走らせたときのもので、テストは確かめない（比べた行の数は確かめる）。テストを足したら取り直す。
- ライセンスは MIT OR Apache-2.0（2026-10-03 に作者が決めた）。変えるときは、LICENSE のファイル、`Cargo.toml` の `license`、README の節、スキルの frontmatter（`tests/skill.rs` の確かめも）をそろえて直す。

### 5.1 C から D へ（C の段階で書いた）

- 生成したコードの本物の抜き出しは、`koyomi::codegen::unit_of` と `koyomi::codegen::files` で作れる。DESIGN の ```` ```ts ```` などの塊は、`tests/design.rs` が生成物の抜き出しかを確かめる（D の docs/ と README の塊にも、同じ確かめを広げられる）。```` ```jsonl ```` の行は vectors の行かを確かめる。
- `koyomi gen` の既定の書き出し先は `generated/`（`.gitignore` に入れた）。README の例を書くときに、リポジトリの中に生成物を残さない。
- D.3 の `docs/generated-code.md` には、DESIGN 6.1 と 6.2 の事実（ファイルの名前、関数の名前と署名、エラーの四つの種類、ランナーの形、出力先ごとのツール）を使う。言い回しは DESIGN から借りない。
- 突き合わせのテストが要るツールの場所は、環境変数で渡せる：`KOYOMI_TSC`（無ければ `tools/node_modules/.bin/tsc`）、`KOYOMI_MYPY`（無ければ `tools/.venv/bin/mypy`）、`KOYOMI_PG_BIN`、`KOYOMI_PG_SOCKET_DIR`、`KOYOMI_EGOV`（e-Gov の場所）、`KOYOMI_NET=1`（本物に問い合わせる）。README の「どう確かめているか」には、DESIGN 6.4 の表の数を `cargo test -- --nocapture` で取り直して貼る。
- `tests/cli.rs` の「まだ無いコマンド」は `doc` だけになった。D で `doc` を表に足したら消す。
- `koyomi source outdated` の本物への問い合わせの出力は DESIGN 9 章にある（2026-10-03）。README に載せるなら取り直さず、その日付のものとして引く。

### 5.2 B から C へ（B の段階で書いた）

- 意味の原本は `src/interp.rs`。`apply`（一つの操作）、`run`（一つの入力のすべての日付）、`trace`（計算の段つき）、`time`（`at` の時刻。ローカルの形と `Z` の形）。生成物の小さな関数は、`src/date.rs` と `src/calendar.rs`（`roll`、`add_business`、`is_open`）の手順を移植する。
- 入力を回す順は `check::Inputs`（整数の入力が外側で、最後に宣言したものがいちばん速く回り、日付が内側）。vectors もこの順に出すと、`check` の報告と行が対応する。
- エラーの種類（C.1）と参照インタプリタの対応：入力が範囲の外 → `range`、`interp::OpFail::Outside` → `data`、`OpFail::Missing` → `reject`、`OpFail::OutOfRange` → `date`。`api` の `wire.errors` がこの四つを言っている。
- 名前と署名は `src/naming.rs`（`Target::signature`、`Target::is_open`、`Target::file`、`Target::module`）。`api` はもうこれを使っているので、生成器が同じ関数を使えば C.8 の「api の署名が生成物の中に一字一句ある」が通る。`is_open` の引数は `day`。
- 生成物の中の小さな関数と内部の名前は `_` で始める（E009 は公開する名前だけを予約している）。
- CLI の表（`src/cli.rs`）に `gen`、`vectors`、`source` を足したら、`tests/cli.rs` の「まだ無いコマンド」の並びから消す。
- 例の別名：`起点(origin)`、`月数(month_count)`（`start` と `months` はキーワードなので使えない）。
- golden は `KOYOMI_BLESS=1 cargo test` で取り直す（`tests/golden/` の診断、`tests/golden/api/` の api）。
- 診断の JSON の `fails` は、一つの診断につき 100 万か所まで（`check::MAX_RUNS`）。

### 5.3 A から B へ（A の段階で書いた）

- DESIGN の数は、A の段階の試作で出した。B の参照インタプリタが違う数を出したら、まず DESIGN 2.2 の定義に照らす。試作は `close day`、`roll`、営業日の数え方、民法の書き方を DESIGN と同じに実装していたが、`+ 0 business days` は試作ではその日のままにしていた（例では使わないので数に影響しない）。
- 祝日の表のコピー（`examples/calendars/data/`）と民法のコピー（`examples/sources/law/`）は A の段階で取ったものを使い、取り直さない（B.5）。
- 作者が決めるべきだったかもしれないこと（A の報告で挙げたもの）：看板の言い方、ファイルの見出しの語（`calendar` と `dates`）、`+ 0 business days` を寄せること、日付の入力を一つに限ること、ライセンス（2026-10-03 に MIT OR Apache-2.0 に決まった）。作者の返事で変わったら、DESIGN と、この計画の該当の項を直してから進める。
