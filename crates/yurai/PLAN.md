# yurai 実装計画

DESIGN.md を仕様として、yurai を三つの段階（B・C・D）で作る。どの段階も、ここに書いた順に進め、各段階の最後にある完了の条件のテストが全部通ったら終わりにする。DESIGN.md と違うことをしたくなったら、先に DESIGN.md に決定・理由・捨てたものを書き、報告で言う。

この計画を書いた A の段階では、本体のコードは書いていない。リポジトリに置いたのは DESIGN.md とこの PLAN.md だけである（LICENSE-MIT と LICENSE-APACHE は前からある）。端のハッシュの値は、DESIGN の定義どおりに組んだ使い捨ての Python の試作で計算し、DESIGN 1.1・3.2・4.1・19 章と、下の完了の条件に写した。試作はリポジトリに残していない。B と C の yurai が同じ値を出すことを、完了の条件にした。

## 0. 全部の段階に共通の決まり

### 0.1 守ること

- 作者の決まり（段階ごとの指示書が挙げるメモ：`japanese-style`、`private-hobby`、`no-quoting-prompts`、`write-from-real-runs`、`features-are-first-class`、`do-the-whole-job`、`shell-gotchas`、`name-the-feature`、`user-profile`）を先に読み、従う。日本語（DESIGN.md、`--lang ja` の診断、README.ja.md、報告）は、英語の概念語を漢字に直訳しない。「道具」ではなく「ツール」、「原本」ではなく「出典」か「元」、「断片」ではなく「条」か「引用箇所」と書く。物に「たち」を付けない。
- git のコミットと push をしない。`~/yurai` の外に書かない。rulec・dandori・koyomi・chobo・geas の木は読むだけで、そこでビルドも git もしない。`~/sakai` には触れない。
- Rust は edition 2024 で、手元の stable 1.94.1 で通すこと。依存は `serde_json = { version = "1", features = ["preserve_order"] }` だけ。
- 診断は英語が既定で、`--lang ja` で日本語。golden は両方の言語で取る。
- テストは `cargo test`。外のツール（rulec、koyomi、chobo、geas、xmllint とスキーマ、Python の venv、python3、curl、Chrome）が無いときは、`SKIP: <理由>` を一行出して通す。報告の前に `cargo test -- --nocapture 2>&1 | grep SKIP` で SKIP の行を読み、数を報告に書く。
- golden の取り直しは `YURAI_BLESS=1 cargo test`。取り直したら差分を読んでから報告する。
- サーバー（HTTP、Chrome）を立てたまま終わらない。テストは止める処理を Drop に置き、一時ディレクトリを消す。
- 成果物（文書、golden、生成物、例の `.req` と `reviewed/`）に、手元の絶対パス、ユーザー名、マシン名を入れない。
- 文書に載せる出力と数は、実際に走らせたものを貼る。DESIGN の「形の案」は、その段階で実物に差し替える。

### 0.2 作者の機械で気をつけること（macOS arm64）

- 一式のツールのバイナリ：rulec 0.22.1 は作業場所の `bin/rulec` に写してある。koyomi・chobo・geas は `cargo install --locked --path ~/<名前> --root <作業場所>/yurai/tools --target-dir <作業場所>/yurai/target-tools` で入れる。**`--target-dir` を必ず付ける**。`cargo install --path` は、`--target-dir` が無ければ、そのクレートの木の `target/` でビルドするので、読むだけのはずの木に書くことになる。テストには `YURAI_RULEC`・`YURAI_KOYOMI`・`YURAI_CHOBO`・`YURAI_GEAS` で場所を渡す。
- macOS には `timeout` コマンドが無い。子プロセスの時間切れは、テストの Rust の側で `Child::try_wait` を回して決め、超えたら kill する。
- Python の venv は `uv venv --python 3.13 <場所>`。Homebrew の 3.14 の venv には pip が入らない。`prov==3.2.2` と `reqif==0.1.0` を入れる（`tools/requirements.txt`。A の段階で、この組み合わせが Python 3.13.11 で動くことを確かめた）。venv の `python` はシンボリックリンクなので、テストはリンクのまま渡す（たどると venv の外の Python になり、`prov` が見えない）。
- ReqIF のスキーマは `tools/reqif/fetch.sh [<dir>]` で取る。既定の置き場所は `tools/reqif/xsd`（git に入れない）で、ほかの場所に置いたらテストに `YURAI_REQIF_XSD=<dir>` で渡す。C の段階では、venv とスキーマを作業場所に置いて `YURAI_PYTHON` と `YURAI_REQIF_XSD` で渡し、終わったら消した。
- xmllint は `/usr/bin/xmllint`（libxml 2.9.13）。スキーマを読み込むたびに、同じ名前空間を二度読み込もうとした警告（`Skipping import of schema …`）を多く出すが、検証の結果には関わらない。テストは結果の行（`validates` か `fails to validate`）と exit code を見る。
- Chrome は `YURAI_CHROME`、無ければ `/Applications/Google Chrome.app`、それも無ければ PATH の `google-chrome` か `chromium`（koyomi と chobo と同じ順）。`--headless` は書き出したあとも終わらないことがあるので、時間を区切って kill し、`--user-data-dir` に一時ディレクトリを渡して、終わったら消す。
- zsh で変数名に `path` を使わない。コマンドや Write で、バックスラッシュと `u` に 16 進 4 桁を続けた形を書くと文字に変わるので、エスケープを文字どおりに書くときは Python で `chr(92)` から組み立てる。
- ディスクの空きは 48 GB ほど。ビルドの中間物や大きな一時ファイルを残さない。

### 0.3 手本にしてよいもの（読むだけ）

- koyomi：`src/i18n.rs`（`tr!` が文の組を返す）、`src/diag.rs`（診断の形と JSON）、`src/codes.rs`（台帳と再現）、`src/cli.rs`（コマンドとフラグの表）、`src/sha256.rs`、`src/sources.rs`（写しと固定、`fetch`・`pin`・`outdated`、`curl` の呼び方、e-Gov の要素の名前、`xml_text` と `article_lines`）、`src/doc/`（Markdown と HTML のページ）、`tests/docs.rs` と `tests/skill.rs`。
- rulec：`src/sources.rs`（e-Gov と eCFR の写し方。`egov_fragment`、eCFR の `title`・`part`・`section`、附則）。
- dandori：`src/proto.rs`（`.proto` の読み手と `import` の探し方）。
- geas：DESIGN の 7 章と PLAN の付録 A（記録と `affected --json` の形）。
- chobo：`src/syntax.rs`（キーワードの表の置き方）。

写すときは yurai の DESIGN に合わせて書き直す。

### 0.4 段階をまたぐ約束

次のものは、どの段階でも DESIGN のとおりにし、変えるときは DESIGN を先に直す。

- **成果物の名前**（DESIGN 2 章）：形、パス（書いたファイルからの相対、JSON ではルートからの相対）、ツールと種類の語、名前の書き方、同じかどうか・含むかどうか、JSON の形。2026-10-03 に sakai と突き合わせて決着した（DESIGN 2.8）。試しの表 `tests/fixtures/naming.tsv` は sakai のリポジトリと同じもので、直すときは二つで同じに直す。変えるなら報告で言う。
- **端の中身**（DESIGN 3.2、4.1）：バイト列まで決めてある。要件の端の中身、決まった形の JSON、proto の決まった形の文。B の段階の値と C の段階の値が、同じ要件について同じでなければならない（借りた出典でも、自分で写した出典でも）。
- **確かめた記録の行**（DESIGN 4.2）：`reviewed <日付> by <役割> sha256:<16>[, sha256:<16>…] -> sha256:<16>` と `approved <日付> by <役割> sha256:<16>`。
- **確かめたときの中身**（DESIGN 4.4）：`.req` の隣の `reviewed/<16 桁>`。中身はその端の中身のバイト列そのもの。
- **診断のコード**（DESIGN 6.2）：番号と意味。増やすときは番台の末尾に。
- **コマンドの表**（DESIGN 7 章）：まだ作っていないコマンドは、表に載せない（その段階で足す）。
- **api の JSON**（DESIGN 11 章）：キーの名前と順。
- **環境変数**：`YURAI_LANG`、`YURAI_BLESS`、`YURAI_RULEC`、`YURAI_KOYOMI`、`YURAI_CHOBO`、`YURAI_GEAS`、`YURAI_SAKAI`、`YURAI_PYTHON`（`prov` と `reqif` の入った Python）、`YURAI_XMLLINT`（無ければ PATH の `xmllint`）、`YURAI_REQIF_XSD`（`tools/reqif/fetch.sh` が取ったスキーマとカタログの置き場所。無ければ `tools/reqif/xsd`）、`YURAI_EGOV`、`YURAI_ECFR`、`YURAI_NET`、`YURAI_CHROME`。

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
  copies.rs        法令の写しの置き場所、xml_text と article_lines、e-Gov と eCFR の要素の名前（B）
  sources.rs       出典の写しと固定と引用の検査（E101〜E105、W101）（B）
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
  tools/  mod.rs（子プロセス、場所、まとめて一度）rulec.rs koyomi.rs chobo.rs geas.rs   （C）
  proto.rs         .proto の読み手と決まった形の文（C）
  borrowed.rs      借りた出典、成果物が固定している条、E106 と E107（C）
  affected.rs      差分の読み手と、差分 → 要件（C）
  export/  mod.rs（二つの書き出しが読むグラフ、識別子のハッシュ） reqif.rs prov.rs   （C）
  fetch.rs         source fetch・pin・outdated、curl の呼び方（C）
  doc/  mod.rs markdown.rs html.rs                                 （D）
tests/
  common/mod.rs（一時ディレクトリ、yurai を走らせる、golden、B.7 の変更）  （B。ツールの場所と SKIP は C）
  syntax.rs project.rs names.rs copies.rs ends.rs marks.rs periods.rs coverage.rs review.rs trace.rs mutants.rs codes.rs cli.rs api.rs design.rs   （B）
  tools.rs proto.rs borrowed.rs affected.rs export.rs fetch.rs   （C）
  doc.rs docs.rs skill.rs examples.rs                            （D）
  fixtures/（naming.tsv、period/、ecfr/、payment/）  mutants/  golden/（marks/、trace/、api/、export/ を含む）
tools/requirements.txt（prov==3.2.2、reqif==0.1.0）  tools/reqif/fetch.sh  tools/reqif/README.md   （C）
examples/<例>/                                                    （D）
docs/  skills/  README.md  README.ja.md  THIRD_PARTY_NOTICES.md   （D）
```

## 2. 段階 B：言語の芯

字句・構文・型、プロジェクトと名前、出典の写しと固定、端の中身とハッシュ、グラフ（循環、期間）、確かめた記録と印、カバレッジと範囲、`review`、`trace`、診断（英語と日本語）、CLI の `check`・`review`・`trace`・`api`・`explain`。成果物は `file` だけを読む（一式のツールを読むのは C）。項の順に進め、各項のテストはその項のうちに書く。

### B.1 土台

- `Cargo.toml`（name `yurai`、edition 2024、`license = "MIT OR Apache-2.0"`、`repository = "https://github.com/i2y/yurai"`、依存は serde_json だけ）と `.gitignore`。lib と bin の両方を持つ。
- `src/i18n.rs`：`Lang { En, Ja }`。`--lang` があればそれ、無ければ `YURAI_LANG`、無ければ英語。システムのロケールは見ない。`tr!("日本語", "English")` は `Text { ja, en }` を返し、描くときに `Lang` を渡す（koyomi と同じ）。
- `src/kw.rs`：DESIGN 1.2 の表を一枚で持つ。予約語の判定と `explain` と、D の `docs/reference.md` のキーワードの表は、ここから引く。

### B.2 字句と構文（`src/lex.rs`、`src/parse.rs`、`src/ast.rs`）

- 字句：名前（Unicode の文字・数字・`_`）、別名 `(ascii)`、版 `v<n>`、文字列（`\"` と `\\`）、日付 `YYYY-MM-DD`、期間（`<日付>..<日付>`、`<日付>..`、`..<日付>`）、`sha256:<16 桁>`、`@`、`,`、`->`、`=`、`#` から行末までのコメント。どの字句も行と列を持つ（列は文字で数える）。
- 構文：DESIGN 1.1 の節の順（見出し、`description`、`role`、`source`、`scope`、`requirement`）、`source` の三つの形と固定の行、`scope`、`requirement` のブロックの行の順（DESIGN 1.5 の表）、リンクと見送りと、その下の記録の行。成果物の名前（DESIGN 2.1、2.4）は、ツールの語、パスの文字列、種類と名前の組（語か文字列）で読む。字下げはスペース。
- E001〜E006 を出す。
- テスト（`tests/syntax.rs`）：DESIGN に出てくる `.req` の塊が全部構文を通る（1.1 はそのまま。1.4〜1.8 の抜き出しは、見出しを足したファイルにして）。E001〜E006 の変異ファイルが、そのコードを出す。

### B.3 プロジェクトと名前（`src/project.rs`、`src/names.rs`）

- プロジェクト：`yurai <コマンド> <path>...` に渡したファイルとディレクトリ（下の `.req` を全部、パスの順に）を一つのプロジェクトとして読む。
- ルート（DESIGN 2.2）：最初に渡したパスの上で `.git` を持つ一番近いディレクトリ、無ければ渡したディレクトリ（ファイルならそのディレクトリ）。`--root` で替える。git は走らせない。
- 名前の表：要件と版と別名、役割、出典（ファイルごと）、ファイルの見出し。E007〜E010。別名の決まり（DESIGN 1.2）。版の決まり（DESIGN 1.5）。
- 成果物の名前（DESIGN 2 章）：パスを `.req` のディレクトリから読み、`.` と `..` を字の上で畳み、ルートからの相対にする。絶対パスとルートの外は E013。ツールの語（E011）と、ツールごとの種類の語と組の並び（E012。2.3 の表を `src/names.rs` に一枚で持つ。dandori の種類、`source` を借りた出典の外に書いたもの、`method` を `service` の外に書いたものも E012）。名前の文字の形（DESIGN 2.4）と JSON の形（2.6）。同じかどうか、含むかどうか（2.5）。
- `verified by` に書ける種類（DESIGN 1.6 の表）。違えば E403。
- テスト（`tests/project.rs`、`tests/names.rs`）：E007〜E013 と E403 の変異。名前の文字の形が、2.4 の決まりで往復する（語と文字列、`\"` を含む名前）。パスの畳み方（`../x/./y.rule`）。ルートの探し方（一時ディレクトリに `.git` のディレクトリを作って）。sakai と共有する `tests/fixtures/naming.tsv` の全行が、表のとおりの JSON かエラー（理由に当たるコード）になる（DESIGN 2.6）。

### B.4 出典の写しと固定（`src/sha256.rs`、`src/base64.rs`、`src/copies.rs`）

- SHA-256 を自前で書く。FIPS 180-4 の既知の値でテストする（空 → `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`、`abc` → `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`、`abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq` → `248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1`、`a` を 1,000,000 個 → `cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0`。koyomi の PLAN B.5 と同じ値）。base64 は C の `source fetch` が使うが、ここで書いてテストする。
- `law` の出典（e-Gov と eCFR）：条の名前から要素の名前と写しのファイル名を作る（rulec の `egov_fragment` と同じ写し方。`第143条第2項` → `MainProvision-Article_143-Paragraph_2`、`別表第一` → `AppdxTable_1`、附則。eCFR は `"§1910.157"` → `1910.157.xml`、ディレクトリは `29-CFR-1910@<日付>`）。写しは `.req` の隣の `sources/law/…`。
- `file` の出典：パス、`url`、固定。
- 引用 `@<出典> <条>[, <条>…]`：E105（書き方、宣言の無い出典、`file` の出典に条を書いた）。
- 写しと固定：E101〜E104、W101（検査は `src/sources.rs`、置き場所と本文は `src/copies.rs`）。写しの XML からタグを落として本文にする関数（`xml_text`。文ごとに一行。rulec の `xml_text` と同じ）と、項ごとに一行の `article_lines`（koyomi と同じ。D の doc と `trace` が使う）。E104 は、UTF-8 の XML で、最初の要素が条の要素（`Article`、`Paragraph`、`Item`、`AppdxTable`、`SupplProvision`、eCFR は `DIV8`）のときに通す。
- 借りた出典（`<ツール> "<パス>" source <名前>`）は、ここでは構文と名前だけを読む。解くのは C（C.7）。
- テストの材料：`tests/fixtures/period/`（下の B.14）に、koyomi の `examples/sources/law/129AC0000000089@2026-10-01/` の四つの XML と `revision.txt` を写す（取り直さない）。
- テスト（`tests/copies.rs`）：四つの写しの固定が、140 条 `e880059021fbb67d`、141 条 `0575c131b9f08063`、142 条 `fc8c35a0769d3b35`、143 条 `6950bdfb988439b6` になる。142 条の `xml_text` が「期間の末日が日曜日、国民の祝日に関する法律（昭和二十三年法律第百七十八号）に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する。」を含む。条の名前の写し方。E101〜E105 と W101 の変異。

### B.5 成果物（`file` だけ）と端の中身（`src/ends.rs`）

- `file "<path>"` の成果物：ファイルがあるか（E201）、端の中身はバイト列、ハッシュは SHA-256 の先頭 16 桁。
- 出典の条の端：写しのバイト列（ハッシュは固定と同じ値）。`file` の出典の端：ファイルのバイト列。
- 要件の端：DESIGN 4.1 の形を一字一句。`text` の行、`from` の行（自分の `law` の出典は `from law <db> <ID> <条> sha256:<固定>`、`file` の出典は `from file <ルートからのパス> sha256:<固定>`、元になった要件は `from requirement <名前> v<n> sha256:<端>`）、`in force` の行。二行めから下を UTF-8 のバイト列の順に並べ、どの行も LF で終える。元になった要件の端を先に計算する（循環は B.6 で止める）。
- ほかのツールの成果物は、名前の検査（B.3）までで止め、端を作らない（C で作る）。B の段階のテストは、ほかのツールの成果物を使わない。B の yurai は、名前の検査を通ったプロジェクトがほかのツールの成果物、範囲、借りた出典を名指していれば、「yurai はまだ koyomi の成果物を読めません」と言って exit 2 にする（`src/check.rs` の `not_yet`）。確かめないまま通すことはしない（DESIGN 2.3）。C はここをツールの読み方に差し替える。
- テスト（`tests/ends.rs`）：B.14 の `period` の要件の端が、A の段階の試作と同じ値になる。`起算日` `a9ebc73907faddc8`、`満了日` `465b83ed8c251406`、`満了日_142条` `d4f2d2a67322df17`。`file "民法の期間.cal"` の端が `c9b94eecde23e6b5`。`起算日` の端の中身が DESIGN 4.1 の 3 行と一字一句同じ。

### B.6 グラフ、循環、期間（`src/graph.rs`）

- リンク（DESIGN 4.1 の表）：`from`（出典の条、元になった要件）、`satisfied by`、`verified by`、見送り。
- 循環（E405）：`from <要件>` と `replaces <要件>` の辺。循環の要件を、ファイルの位置と一緒に順に並べる。
- 期間（DESIGN 5.5）：E406（隙間の日を並べる）、E407（重なる日と二つの版）、E408、E409。日付は 0001-01-01〜9999-12-31 の暦日（koyomi の `days_from_civil` と `civil_from_days` の手順で通算日にする）。
- テスト（`tests/periods.rs`）：隙間が一日（2027-04-01 だけ）、重なりが三日、終わりを開けた版が最後でない、版の番号が期間の順でない、置き換えの始まりが一日遅い、分ける置き換えとまとめる置き換えが通る、の変異。循環が二つの要件と三つの要件のとき。

### B.7 確かめた記録と印（`src/marks.rs`、`src/diff.rs`）

- 記録の行（DESIGN 4.2）を読む。ハッシュの数がリンク元の数と合わない、形が崩れている、は E305。
- 比べ方（DESIGN 4.3）：E301〜E304。理由（出典の条が変わった、要件の文か期間が変わった、要件のリンク元だけが変わった、成果物が変わった）と、それぞれの差分。差分は `reviewed/` の中身と今の端の中身から作る。法令の写しは `xml_text` の行で、ほかはバイト列を UTF-8 の行で比べる（LCS、統一形式、前後二行、40 行まで）。中身が無ければ W301。UTF-8 でない中身と 1 MiB を超える中身は、ハッシュと大きさだけを言う。
- 並べ方（DESIGN 4.3）：変わったものごとにまとめ、出典、要件、成果物の順。まとまりの中は `from` のつながりの順、同じならファイルの行の順。一本のリンクは、最初に当たったまとまりで一度だけ言う。同じ成果物の変更が何本ものリンクに出るとき、差分は最初の一本だけに載せ、ほかは「同じ変更」と言う。
- テスト（`tests/marks.rs`）：B.14 の `period` の写しで、次の変異を golden にする（英語と日本語）。
  1. 142 条の写しの本文を一文字変え、固定の行をその写しのハッシュに書き換える：E302 が二つ（`from @民法 第142条` と、`満了日_142条` の `satisfied by`）と E304 が一つ（`満了日_142条` の見送り）。最初の E302 に本文の差分。
  2. `満了日` の `text` を一文字変える：その `from` が E303（要件の端の中身の差分）、`satisfied by` が E302、見送りが E304。
  3. `民法の期間.cal` の一行を変える：三本の `satisfied by` が E303。差分は一本めだけ。
  4. 記録の行を一つ消す：E301。承認の行を一つ消す：E304。
  5. `reviewed/` を消してから 1 を当てる：同じ印に W301 が加わり、差分は出ない。

### B.8 カバレッジと範囲（`src/coverage.rs`）

- DESIGN 5.2：E401、E402、W401。版ごとに数える。
- DESIGN 5.3：範囲の成果物を集める（`scope file "<path>"` のファイルとディレクトリ。除くディレクトリは DESIGN 1.8）。辿れるかの 1〜3（名指されている、含むものが名指されている、含まれるものが名指されている）。4（geas の記録を通る）は C。E404。
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
- テスト（`tests/review.rs`）：B.7 の 1〜3 の変異に `yurai review --all --by 法務 --date 2026-10-04` を当てると、`check` が通る。`.req` のバイト列が、記録の行のほかは元と同じ（CR LF のファイルでも）。`reviewed/` に新しい中身が書かれ、古い中身が消える。`shasum` で、どの中身もファイルの名前のハッシュになっている。印の無いリンクを `--at` で選んでも書かない。

### B.11 trace（`src/trace.rs`）

- DESIGN 9 章：要件から（出どころ、写しの本文と時点と版、決めたこと、リンクと記録と印）、`file` の成果物から、出典の条から。`--format json`。成果物のファイルが固定している条は C で足す。
- テスト（`tests/trace.rs`）：`period` の三つの要件、`file "民法の期間.cal"`、`@民法 第142条` の trace を、英語と日本語と JSON の golden にする。

### B.12 診断と台帳（`src/diag.rs`、`src/codes.rs`）

- `Diag`：code、severity、file、line、col、message（`Text`）、notes（`Text` の並び）、diff、chain、candidates、fix。文面と JSON は DESIGN 6.1 の形。
- `src/codes.rs`：DESIGN 6.2 の全コード。各コードに、見出し、いつ出るか、どう直すか（英語と日本語）、走る最小の再現（`.req` の本文と、要るなら写しやファイルの中身）。一式のツールか proto を読まないと出せないコード（E106、E107、E202、E203、E204、E205、W201）は、C の段階で再現を足す。それまでは、`explain` がそのコードを引けて、再現が「C の段階で足す」と分かるようにし、`tests/codes.rs` が B の段階で確かめるコードの集まりから外す。
- `yurai explain <コード>`、`yurai explain --all`、`--format markdown`。
- テスト：`tests/codes.rs`（B のコードの再現が、そのコードを出す）、`tests/mutants.rs`（`tests/mutants/<コード>_<内容>/` ごとに、英語と日本語の golden `tests/golden/<同じ名前>.en.txt`・`.ja.txt` と一致する）。

### B.13 CLI と api（`src/cli.rs`、`src/main.rs`、`src/api.rs`）

- DESIGN 7 章：コマンドとフラグを一枚の表に置き、`yurai --help`、`yurai <cmd> --help`（`yurai help <cmd>` も同じ）、`yurai --version`。知らないフラグ、閉じた集合の外の値、値の無いフラグ、二度書いたフラグ（何度でも書けるものを除く）は exit 2。引数なしの `yurai` は使い方を標準エラーに出して exit 2。
- B で実装するコマンドは `check`、`review`、`trace`、`api`、`explain`。まだ実装していないコマンドを表に載せない。
- `api`（DESIGN 11 章）：B では `file` の成果物だけ。キーの順は DESIGN のとおり。
- テスト（`tests/cli.rs`、`tests/api.rs`）：exit code、全コマンドの `--help`、知らないフラグ、`YURAI_LANG`。`period` の api の golden。

### B.14 テストの材料

- `tests/fixtures/period/`：koyomi の `examples/民法の期間.cal`（B では `file` の成果物として読む）と、`sources/law/129AC0000000089@2026-10-01/`（四つの XML と `revision.txt`）を写したもの。`民法の期間.req` は、DESIGN 1.1 の要件三つを、次のように B の段階で読める形にしたもの：出典は自分で写した `source 民法 = law "129AC0000000089" asof 2026-10-01`（四つの固定）、成果物は `satisfied by file "民法の期間.cal"`、確かめる側は三つとも `not verified` の見送り（理由と承認）、範囲は `scope file "民法の期間.cal"`。確かめた記録と承認は、B.10 の `review` で書いたもの（日付は 2026-10-03）と、その `reviewed/`。
- `tests/fixtures/ecfr/`：rulec の `tests/corpus/sources/law/29-CFR-1910@2026-01-01/1910.157.xml` を写したもの（固定 `c2a9ce966c7e2269`）と、それを `@osha "§1910.157"` で引く英語の要件一つ。eCFR の置き場所と E104 の要素の名前を、B の段階で確かめる。
- `tests/mutants/`：台帳のどのコードにも、変異が一つ以上ある（`<コード>_<内容>/` のディレクトリに、`.req` と要るファイル）。どの変異も自分をルートにして検査するので（`--root`）、golden はリポジトリの置き場所に依らない。
- 要件の端のハッシュは、出典を自分で写しても借りても同じになる（DESIGN 4.1）。C.7 で、同じ要件を koyomi から借りた出典で書き、同じ値になることを確かめる。

### B.15 B の完了の条件

`cargo build` が警告なしで通り、`cargo test -- --nocapture` が全部通ること。B のテストは外のツールを使わないので、SKIP は 0。そのうえで、次が出ること。

| 対象 | 出ること |
|---|---|
| SHA-256 | B.4 の四つの既知の値 |
| `period` の写し | 固定が 140 条 `e880059021fbb67d`、141 条 `0575c131b9f08063`、142 条 `fc8c35a0769d3b35`、143 条 `6950bdfb988439b6` |
| `period` の端 | `起算日` `a9ebc73907faddc8`、`満了日` `465b83ed8c251406`、`満了日_142条` `d4f2d2a67322df17`、`file "民法の期間.cal"` `c9b94eecde23e6b5`（A の段階の試作と同じ値。違えば、どちらが DESIGN 4.1 の定義どおりかを決め、試作の側が誤っていたなら DESIGN の値を直して報告する） |
| `period` の check | exit 0。一行の結果が、要件 3 件、リンク 6 本、見送り 3 件を言う |
| B.7 の変異 | 1 は E302 が二つと E304 が一つ、2 は E303・E302・E304 が一つずつ、3 は E303 が三つ（差分は一本め）、4 は E301 と E304、5 は 1 の印に W301 |
| B.10 の review | 1〜3 の変異が `review --all` のあと通る。`.req` は記録の行のほかは一字も変わらない |
| 診断 | DESIGN 6.2 のうち B のコードに変異と再現があり、英語と日本語の golden と一致する |
| 名指し | `tests/fixtures/naming.tsv` の全行が表のとおり（B の段階では JSON 18 行、エラー 9 行。2026-10-03 に sakai との突き合わせで 36 行（JSON 21 行、エラー 15 行）になり、C の段階で写し直した） |

報告には、`cargo test` のテストの数と時間を書く。DESIGN の形の案のうち、B で実物に差し替えたもの（5.1 の一行、4.3 の例の診断、6.3 の例、9 章の trace）を挙げる。

## 3. 段階 C：一式の読み込み、affected、書き出し、出典のコマンド

一式のツールの JSON（rulec、koyomi、chobo、geas）と proto を読み、借りた出典と食い違いの検査、名前の変わった成果物、`affected`、ReqIF と PROV の書き出し、`source fetch | pin | outdated` を作る。

**C は二つに分けて進めている（2026-10-03）。** 作者が一式の言語を一つの処理系にまとめるかを考えていて、まとめるなら一式の読み込みは JSON ではなく型の付いた呼び出しで作ることになる。そこで、一式の読み込み（C.1〜C.9：ツールの JSON、proto、借りた出典と E107、名前の変わった成果物、`affected`）は決まるまで止め、yurai の要件だけで作れる C.10〜C.12（ReqIF、PROV、出典のコマンド）を先に作った。止めているあいだ、B の「まだ読めない」と exit 2 で断る振る舞い（`src/check.rs` の `not_yet`）はそのまま残し、借りた出典の `source outdated` も同じく exit 2 で断る。C.10〜C.12 のテストは、出典が yurai の写しで、成果物を名指さないか `file` だけを名指すテストの材料（`period`、`ecfr`、`payment`）で行う。どこまで済んだかは C.13 にある。

### C.1 ツールを呼ぶ（`src/tools/mod.rs`）

- DESIGN 3.1：場所は `YURAI_RULEC`・`YURAI_KOYOMI`・`YURAI_CHOBO`・`YURAI_GEAS`、無ければ PATH。要るのに無ければ、何が要るかを言って exit 2。
- そのファイルのディレクトリで、ファイルの名前を渡して呼ぶ。一回の実行の中で、同じファイルの同じコマンドは一度だけ（結果を覚えておく）。
- exit 0 でなければ E203（標準エラーの初めの数行を注に）。JSON に要るキーが無ければ E204（ツールの名前と、出していればバージョン）。
- テスト（`tests/tools.rs`）：ツールが無いときの exit 2、E203（rulec の変異 `m_e038.rule` を写したもの、koyomi の `民法の期間_読み方の比較.cal` を写したもの）、E204（JSON を差し替えた偽のツール。小さなシェルスクリプトを一時ディレクトリに書いて `YURAI_RULEC` に渡す）。

### C.2 rulec（`src/tools/rulec.rs`）

- `rulec api` から `source_sha256`、`sources`（`law` の `db`・`id`・`asof`・`pins`、`file` の `path`・`url`・`sha256`・`pins`）。`rulec graph` から種類（DESIGN 2.3 の表：`input`、`output`、`table`、`clause`、`define`、`derive`）。名前が別名に当たれば E202 で名前を示す。
- 端の中身はファイル全体（DESIGN 3.2）。ハッシュは `source_sha256` の先頭 16 桁。
- 成果物のファイルが固定している条（DESIGN 3.3）を、グラフに足す（C.7）。
- テストの材料：`tests/fixtures/rulec/` に、rulec の `tests/corpus/印紙税の本則と軽減.rule` と、その写し（`sources/law/342AC0000000023@2026-04-01/`、`sources/law/332AC0000000026@2026-04-01/`）、`osha_extinguisher.rule` と `sources/law/29-CFR-1910@2026-01-01/1910.157.xml` を写す。
- テスト：`印紙税の本則と軽減.rule` の `output 印紙税額`、`table 本則`、`table 軽減`、`clause 非課税`、`define 軽減期間`、`input 契約金額` があると分かり、`output tax`（別名）は E202、`table 無い表` は E202。端が `dc176eebd83f26e3`、`osha_extinguisher.rule` の端が `a52e955b88af88d6`。固定している条が、`法 別表第一 0ba69792e960021e`、`措置法 第91条 85faf53f6f6e8196`、`osha "§1910.157" c2a9ce966c7e2269`。

### C.3 koyomi（`src/tools/koyomi.rs`）

- `koyomi api` から `kind`、`source_sha256`、`inputs`・`dates`・`claims`・`sources`（dates のファイル）、`calendar`（`calendar.sources`）。
- 端の中身：ファイル、`input`、`date` はファイル全体。`claim` は `claims[]` の一つから `name` を除いた決まった形の JSON（DESIGN 3.2）。
- テストの材料：`tests/fixtures/koyomi/` に、koyomi の `examples/` から `民法の期間.cal`、`支払_20日締め翌月10日払い.cal`、`calendars/民法142条の休日.cal`、`calendars/東京の営業日.cal`、`calendars/data/syukujitsu.csv`、`sources/law/129AC0000000089@2026-10-01/` を写す。
- テスト：`民法の期間.cal` の `date 満了日_142条` の端が `c9b94eecde23e6b5`。条件の端が、`142条の満了日は満了日以後` `825aa6c6f7ccf314`、`満了日は単調` `447d80ca751bd681`、`142条の満了日も単調` `2a8e130e4c527692`、`満了日は起点より後` `0b951fef68a36592`。`calendars/東京の営業日.cal` の出典 `祝日` が、固定 `cec37a743c96995c` の `file` の出典として読める。

### C.4 chobo（`src/tools/chobo.rs`）

- `chobo api` から `source_sha256`、`units`、`accounts`、`transfers`。
- 端の中身（DESIGN 3.2）：`unit` は `name` と `ledger` を除いたもの、`account` は `name` と `code` を除いたもの、`transfer` は `name`・`code`・`definition`・`operations` を除き、移動が触る勘定を名前をキーにした `accounts` として足したもの。どれも決まった形の JSON。
- テストの材料：`tests/fixtures/chobo/` に、chobo の `examples/refunds/refunds.ja.book` を写す。
- テスト：`transfer 返金` の端が `84e9ce254075c697`（1,206 バイト）、`transfer 売上計上` が `851ab806078168fe`、`account 返金できる残り` が `9f9b0d74872f62a4`、`account 売上` と `account 返金済み` がどちらも `35a4ec5a2ee5eb06`。`返金` の端の中身が、A の段階の試作が出した中身と一字一句同じ（golden）。

### C.5 geas（`src/tools/geas.rs`）

- 記録は `<spec のディレクトリ>/.geas/<stem>.map.jsonl`。一行め（`geas_map`、`spec`、`root`、`claims`）で主張の名前を確かめる（無い名前は E202）。記録が無ければ W201 で、spec のファイルがあることだけを確かめる。
- 範囲の 4（DESIGN 5.3）：`{"claim", "target", "file", "ran"}` の行から、主張が走らせたファイルを集める。パスは一行めの `root`（spec のディレクトリからの相対）から読み、yurai のルートからの相対に直す。
- 端の中身は spec 全体。
- テストの材料：`tests/fixtures/geas/greeter/` に、geas の `examples/greeter/` の `greeter.geas`、`greeter.ja.geas`、`server.py` を写す。記録はテストの中で `geas map greeter.geas --root <一時ディレクトリ>` で作る（python3 が要る。無ければ SKIP）。
- テスト：記録があるとき、四つの主張（`greets by name`、`rejects an empty name`、`totals accumulate across requests`、`unknown paths are 404`）が名前で分かり、`claim "no such claim"` は E202。記録が無いとき W201。`scope file "server.py"` が、`verified by geas "greeter.geas" claim "rejects an empty name"` だけを書いた要件から、記録を通って辿れる。`server_refactored.py` は記録で `code` が null（どのランタイムも報告しない）なので、範囲に入れると E404 になる。

### C.6 proto（`src/proto.rs`）

- DESIGN 3.4：dandori の `src/proto.rs` と同じ範囲を読む（`package`、`import`、入れ子の `message`、`enum`、`service` と `rpc`、オプション）。読めなければ E205。
- 種類（`service`、`method`、`message`、`enum`）と、端の中身（コメントと空白を落とした決まった形の文。`method` は入力と出力からたどれる `message` と `enum` の全部を、完全な名前の順に足す）。見つからない `import` は、読めなかったものとして名前だけを書く。
- テストの材料：dandori の `examples/fulfillment/specs/warehouse.proto` と `fulfillment.proto` を `tests/fixtures/proto/` に写す。
- テスト：`warehouse.proto` の `service StockService method Reserve`、`method Release`、`message ReserveRequest`、`enum Stock` が分かる。`method Release` を `service` の外に書けば E012。`ReserveRequest` にフィールドを一つ足すと、`method Reserve` と `service StockService` の端は変わり、`method Release` の端は変わらない。コメントだけを書き換えても、どの端も変わらない。`fulfillment.proto` の `import "dandori/v1/options.proto"` は、ファイルが無いので読めなかったものとして端の中身に名前が入る。端の中身の golden。

### C.7 借りた出典と食い違い（`src/borrowed.rs`）

- 借りた出典（DESIGN 1.4、3.3）：ツールの api の `sources` から名前を引き、固定を読み、写しはツールのファイルの隣の `sources/law/…` から読む。無ければ E106。
- 成果物のファイルが固定している条を、グラフに足す（`trace`、`affected`、`source outdated`、`doc` が使う）。
- 食い違い（E107）：要件の `from @… 第N条` と、その要件を満たす成果物のファイルの固定が同じ条（データベース、ID、条）を指すとき、写しの `xml_text` を比べる。成果物がその条を固定しているのに、どの写しも本文が違えば E107（本文の差分つき）。
- `trace` に、成果物のファイルが固定している条を足す（DESIGN 9 章）。
- テスト（`tests/borrowed.rs`）：B.14 の `period` の `.req` を、出典を `source 民法 = koyomi "民法の期間.cal" source 民法` に替え、成果物を `koyomi "民法の期間.cal" date …` に替えたものが、B と同じ要件の端（`a9ebc73907faddc8`、`465b83ed8c251406`、`d4f2d2a67322df17`）を出し、`check` が通る。koyomi が固定していない条を引けば E106。`tests/fixtures/rulec/` に、自分で写した `source 法 = law "342AC0000000023" asof 2026-04-01` と `source 措置法 = law "332AC0000000026" asof 2026-04-01`（写しは rulec の写しと同じバイト列）を引く要件を置くと、E107 は出ない。yurai の側の写しの本文を一文字変えて固定し直すと、E107 が出る。

### C.8 名前の変わった成果物（DESIGN 4.5）

- E202 のとき、同じファイルの同じ種類のもので、今の端が記録のリンク先のハッシュと同じものを、候補として添える。端がファイル全体の種類では、どのリンクも指していない同じ種類のものを並べ、確かめたときのファイルとの差分を見せる。
- テスト：chobo の帳簿で `transfer 返金` の名前を `返金の申請` に変えた写しを作ると、候補に `返金の申請` が一つ出る。`account 返金済み` を指していて、その名前を変えたときは、候補に新しい名前と `売上` の二つが出る（DESIGN 3.2）。koyomi の日付の名前を変えたときは、どのリンクも指していない日付が候補に出て、差分が出る。

### C.9 affected（`src/affected.rs`）

- DESIGN 8 章。統一形式の差分（`git diff` と `diff -u`。`diff --git`、`---`/`+++`、`a/`・`b/`、改名、ファイルの追加と削除）を読み、ファイルごとに、`.req`（差分の行が入る要件のブロック）、出典の写し、成果物のファイル、コード（geas の spec ごとに `geas affected <spec> <差分> --json`。`--map <spec>=<記録>` を `--map <記録>` で渡す）、そのほか、に分ける。
- 答えの文面（英語と日本語）と `--format json`。exit code は 0・1・2（DESIGN 8 章）。
- テスト（`tests/affected.rs`。geas と python3 が無ければ SKIP）：`tests/fixtures/geas/greeter/` の写しに、A の段階と同じ差分（`server.py` の `"name required"` を `"a name is required"` に）を当て、変更の前と後の記録を渡すと、主張 `rejects an empty name` と、それを確かめる要件と持ち主が出る（exit 0）。前の記録だけを渡すと、geas の E063 を注に言って exit 2。`.req` の要件の文を変える差分、法令の写しを変える差分（その条を引く要件と、固定している成果物が出る）、`.rule` を変える差分（その規則を名指す要件が出る）の golden。

### C.10 ReqIF（`src/export/reqif.rs`）

- DESIGN 12 章の写し方。識別子（`_` と SHA-256 の先頭 32 桁）、`LAST-CHANGE` と `CREATION-TIME`（`--time`、無ければプロジェクトのいちばん新しい日付、それも無ければ exit 2）、`RELATION-GROUP`、文字列の `MAX-LENGTH` 100,000（超えれば exit 2）。
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

- テスト（`tests/export.rs`）：テストの材料のプロジェクト（いまは `period`、`ecfr`、`payment`。一式の読み込みを作ったら `rulec`、`koyomi`、`chobo`、`geas` を足す）ごとに ReqIF を書き出し、次を確かめる。`XML_CATALOG_FILES=<スキーマ>/catalog.xml xmllint --nonet --noout --schema …/reqif.xsd` が `validates` を出して exit 0（スキーマが無ければ SKIP）。`reqif validate` と `reqif validate --use-reqif-schema` が exit 0（`YURAI_PYTHON` の venv が無ければ SKIP）。yurai のテストが自分で、すべての `-REF` が、その名前の要素（`<X-REF>` なら `X`）の `IDENTIFIER` を指していることと、`IDENTIFIER` が `_` と 32 桁の 16 進数で重ならないことを確かめる（これは SKIP しない。壊した参照を見つけることも確かめる）。同じプロジェクトを二度書き出して、バイト列が同じ。要件の数、つながりの数、`SPEC-HIERARCHY` の数が `yurai api` の数と合う。golden（`tests/golden/export/<名前>.reqif`）。止まるとき：1〜4 の段のエラーで exit 1（何も書かない）、印は状態として書く、日付が一つも無ければ `--time` を求めて exit 2、`--time` を渡せば `CREATION-TIME` になる、長すぎる文字列と XML に書けない文字で exit 2、`--format` と `--time` を逆の形に渡せば exit 2、`--out`。

### C.11 PROV（`src/export/prov.rs`）

- DESIGN 13 章の写し方。PROV-N（既定）と PROV-JSON（`--format json`）。
- テスト（`tests/export.rs`）：同じテストの材料のプロジェクトごとに、`prov` 3.2.2 で PROV-N（`profile="strict"`）と PROV-JSON の両方を読み（`ProvDocument.deserialize`）、二つが等しいこと（`==`）、種類ごとの記録の数が `yurai api` から数えた数と合うことを確かめる（`YURAI_PYTHON` が無ければ SKIP）。`hadPrimarySource` の項を書かないこと。二度書き出して同じバイト列になること。golden（`tests/golden/export/<名前>.provn` と `.prov.json`）。

### C.12 出典のコマンド（`src/fetch.rs`）

- DESIGN 14 章。`curl -fsSL`、三度まで試す。`file://` も読む。e-Gov は `YURAI_EGOV`（無ければ `https://laws.e-gov.go.jp/api/2`）、eCFR は `YURAI_ECFR`（無ければ `https://www.ecfr.gov/api/versioner/v1`）。
- `fetch`：本文が前の写しと同じなら書き換えない。`revision.txt`。借りた出典は取らずに、ツールのコマンドを言う。
- `pin`：16 桁だけを書き換え、ほかは一字も変えない。足りない固定の行を足す。
- `outdated`：後の版の本文を一つ前の本文と比べ、変わる条ごとに、施行日、版、本文の差分、引く要件と持ち主、確かめ直しになるリンクの数、その条を固定している成果物を言う。`file` の出典は `url` のもののハッシュを固定と比べる。
- テスト（`tests/fetch.rs`）：テストの中に `std::net::TcpListener`（`127.0.0.1:0`）で小さな HTTP サーバーを立て、e-Gov の `law_data` と `law_revisions`、eCFR の `full` と `versions` に用意したレスポンスを返す。用意するのは、民法 140〜143 条（`period` の写しを base64 にしたもの）と後の版が五つある `law_revisions`、29 CFR 1910.157（`tests/fixtures/ecfr/` の写し。一式の読み込みを作れば `tests/fixtures/rulec/` にも同じ写しが入る）と、後の版を一つ持つ `versions`（中身の変わらない版と、`substantive` が偽の版も混ぜる）。確かめること：`fetch` が写しと同じバイト列を書く。`pin` のあとの `.req` が、16 桁のほかは一字も変わらない（CR LF のファイルでも）。`outdated` が、変わらない版では exit 0、ある版の 142 条の本文を一文字変えたレスポンスでは、その施行日と、`満了日_142条` と持ち主 `法務` と確かめ直しになるリンクの数を言って exit 1、属性だけを変えたレスポンスでは exit 0。`file` の出典は `file://` の URL で（`payment` の `約款`）、変わらなければ exit 0、変われば exit 1 で、写しとの差分、それを引く要件とそれを元にした要件、確かめ直すリンクの数を言い、`fetch` と `pin` のあとの `check` が、その数だけ印を付ける。借りた出典は、`fetch` と `pin` がツールのコマンドを言い、`outdated` が exit 2 で断る。curl が無ければ試し直さずに exit 2。`YURAI_NET=1` のときだけ、本物の e-Gov と eCFR に `outdated` を走らせ、走らせないときは `not asked:` の行を出す。
- `check` が通信しないこと：PATH から `curl` を外しても `check`・`trace`・`api`・`export` が通る（`tests/cli.rs` に足す）。

### C.13 C の完了の条件

- `cargo test -- --nocapture` が全部通り、この機械で SKIP が 0（rulec・koyomi・chobo・geas の場所、`tools/reqif/xsd`、`YURAI_PYTHON` の venv を用意して回す）。
- C.2〜C.4 の端のハッシュ（`dc176eebd83f26e3`、`a52e955b88af88d6`、`c9b94eecde23e6b5`、`825aa6c6f7ccf314`・`447d80ca751bd681`・`2a8e130e4c527692`・`0b951fef68a36592`、`84e9ce254075c697`・`851ab806078168fe`・`9f9b0d74872f62a4`・`35a4ec5a2ee5eb06`）が出る。違えば、DESIGN 3.2 の定義と照らし、A の段階の試作の側が誤っていたなら DESIGN の値を直して報告する。
- C.7 の、借りた出典で書いた `period` が、B と同じ要件の端を出す。
- C.9 の `affected` が、greeter の差分で主張から要件と持ち主まで答える。
- C.10 と C.11 の書き出しが、xmllint、`reqif validate`、`prov` のどれでも通る。
- DESIGN 6.2 のうち C で再現を足したコード（E106、E107、E202、E203、E204、E205、W201）に変異と再現があり、英語と日本語の golden と一致する。
- 報告に、ツールのバージョン（rulec、koyomi、chobo、geas、xmllint、Python と `prov` と `reqif`）と、テスト全体の時間を書く。DESIGN の形の案のうち、C で実物に差し替えたもの（8 章の affected、14 章の outdated、11 章の api の rulec・koyomi・chobo の部分）を挙げる。

**2026-10-03 に満たしたもの**（書き出しと出典のコマンドの部分）：`cargo test -- --nocapture` が全部通り、この機械で SKIP が 0（スキーマと venv を `YURAI_REQIF_XSD` と `YURAI_PYTHON` で渡して。一式のツールを使うテストは、まだ無い）。C.10 と C.11 の書き出しが、xmllint、`reqif validate`（`--use-reqif-schema` も）、`prov` のどれでも通る（`period`、`ecfr`、`payment`）。DESIGN 14 章の outdated を、本物の e-Gov に一度問い合わせた出力に差し替えた（12 章と 13 章も実物にした）。

**残したもの**（一式の読み込みを止めているため）：C.2〜C.4 の端のハッシュ、C.7 の借りた出典で書いた `period`、C.9 の `affected`、C で再現を足すコード（E106、E107、E202〜E205、W201）、テストの材料 `rulec`・`koyomi`・`chobo`・`geas` の書き出しと、その ReqIF と PROV の確かめ、借りた出典の `source outdated`、条を固定している成果物を `outdated` と PROV（`yurai:pins`）で挙げること、DESIGN 8 章と 11 章の形の案、報告に一式のツールのバージョンを書くこと。

## 4. 段階 D：doc、例、README、スキル

### D.1 doc（`src/doc/`）

- DESIGN 10 章のとおり、Markdown（既定）と HTML（`--format html`）。ページは一度ブロックの並び（見出し、段落、表、引用、差分）として組み、Markdown と HTML はそれを書き出すだけにする（koyomi の `src/doc/` と同じ考え。二つの形の中身が食い違わないように）。
- 節の順（見出し、トレーサビリティの表、出典、要件ごとの「なぜ」、範囲、確かめた記録の一覧）。出典の条文は `article_lines` で項ごとに引用し、時点と版（`revision.txt`）を添える。HTML では `<blockquote>`。
- 1〜4 の段のエラーがあればページを作らず、診断を標準エラーに出して exit 1。5〜7 の段のエラーは、ページに載せて exit 0。
- HTML は一枚で、script も外のファイルも読まない。明るい配色と暗い配色（`prefers-color-scheme` と `data-theme`）。幅の狭い画面で横にはみ出さない。
- `--out <dir>` があれば、最初の `.req` のファイル名から `<名前>.md` か `<名前>.html` を書く。
- CLI の表に `doc` を足す。
- テスト（`tests/doc.rs`）：例ごとの Markdown と HTML を、英語と日本語の golden で比べる。HTML が外の URL を読まないこと（`src=`、`href=` に `http` が無い。出典の URL を文字として載せるのはよい）。ページに載せた条文の各行が、写しの `article_lines` にあること。Chrome があれば、`民法の期間_読み方を変えた` のページの先頭（明るい配色）と、要件ごとの節（暗い配色）のスクリーンショットを撮り（時間を区切って kill する）、`YURAI_BLESS=1` のときに `docs/images/` に置く。無ければ SKIP。

### D.2 例（`examples/`）

DESIGN 15 章の七つを作る。写したファイルの元（リポジトリと、そのパス）を、例ごとの `README.md` に書く。

- `民法の期間/`：DESIGN 1.1 の `.req`（出典は koyomi から借りる）。確かめた記録は `yurai review` で書く（`--date` は、その例を作った日）。
- `民法の期間_読み方を変えた/`：`民法の期間/` を写し、`.cal` の `満了日_142条` の `if closed + 1 day` を `roll following` に替えたもの（A の段階で、koyomi の検査を通ることを確かめた。DESIGN 19 章）。`.req` と `reviewed/` は `民法の期間/` と同じバイト列にする。
- `支払条件/`：要件は、支払日（`decided` で、例として決めた決まりと書く）と、営業日（`from @祝日`。`source 祝日 = koyomi "calendars/東京の営業日.cal" source 祝日`）。確かめるのは koyomi の条件（`営業日に払う`、`受領から60日以内`）。範囲は `koyomi "支払_20日締め翌月10日払い.cal" date`。
- `印紙税/`：契約書の印紙税額の要件を、v1（`in force 2014-04-01..2027-03-31`、`from @法 別表第一` と `from @措置法 第91条`）と v2（`in force 2027-04-01..`、`from @法 別表第一`）の二つの版で書く。期間の始まりは規則の入力 `作成日` の範囲の始まり（2014-04-01）、軽減の終わりは規則の `define 軽減期間`（`作成日 <= 2027-03-31`）にそろえる。満たすのは `rulec "印紙税の本則と軽減.rule" output 印紙税額`、確かめるのは `rulec "印紙税の本則と軽減.rule"`（例と完全性）。出典は yurai が自分で写して固定する（写しは rulec の写しと同じバイト列。E107 が出ないことを確かめる）。
- `返金/`：要件「返金は、その注文の売上を超えない」（`decided`）。満たすのは `chobo "refunds.ja.book" account 返金できる残り`、`transfer 売上計上`、`transfer 返金`。確かめるのは `chobo "refunds.ja.book"`。範囲は `chobo "refunds.ja.book" transfer`。
- `greeter/`：`greeter.req`（英語）と `greeter.ja.req`（日本語）。要件は主張ごとに一つ（名前で挨拶する、空の名前は断る、足した数が積み上がる、知らないパスには 404 を返す）で、出どころは `decided`。満たすのは `file "server.py"`、確かめるのは geas の主張。範囲は `file "server.py"`。geas の記録（`.geas/greeter.map.jsonl`）を置き、テストが `geas map --root` で作り直したものと同じかを確かめる。`affected` の例の差分（`change.diff`）と、変更のあとの記録も置く。
- `osha/`：英語の、法令を引く例。`source osha = law ecfr "29 CFR 1910" asof 2026-01-01`（自分で写す。写しは rulec の写しと同じバイト列）、要件は消火器までの距離、満たすのは `rulec "osha_extinguisher.rule" table distance`、確かめるのは `rulec "osha_extinguisher.rule"`。
- どの例の `description` にも、法令を引くものには「例として書いたもので、法令の読み方を示すものではない」、例として決めた決まりには「この例のために決めたもの」と書く。
- テスト（`tests/examples.rs`）：`民法の期間_読み方を変えた/` のほかの例は、どれも `yurai check` が exit 0。`民法の期間_読み方を変えた/` は exit 1 で、E303 が三つ（差分は一本め）、ほかのコードは出ない。どの例も、`review --all` を当てても `.req` が変わらない（印が無い）。

### D.3 docs/

- `docs/reference.md`（英語）：言語の全部（DESIGN 1 章）、成果物の名指し方（2 章）、端とハッシュと確かめた記録（3、4 章）、検査（5 章）、コマンド、exit code、JSON の形、書き出し、出典のコマンド。キーワードの表は `src/kw.rs` と同じ並びにする。
- `docs/codes.md` と `docs/codes.ja.md`：`yurai explain --all --format markdown`（`--lang ja`）の出力そのもの。手で直さない。
- テスト（`tests/docs.rs`）：codes の二つがいまの出力と同じ。キーワードの表が `src/kw.rs` と同じ。

### D.4 README.md と README.ja.md

- README.md（英語）：koyomi と chobo の README の並びにならう。看板（DESIGN 0.1）、何か、`.req` の例（英語の例 `greeter` か `osha`）、`check` の結果、わざと止まる例の診断（本物の出力。英語の README なので、日本語の名前の例の診断を載せるなら、そう断る）、確かめることと確かめないこと（DESIGN 0.1 と P1）、入れ方（`cargo install --path .`）、コマンド、例の一覧、どう確かめているか（テストの数、SKIP、外のツール）、`doc` のページ（スクリーンショット）、DESIGN.md への案内、ライセンス（「License」の節。MIT OR Apache-2.0）。
- README.ja.md：英語の写しではなく、普通の日本語で一から書く。例は日本語の例（`民法の期間`）。「ライセンス」の節。
- どちらも、法令に触れる例について DESIGN 15 章の断りを書く。名前の分かる勤め先や人に結びつく書き方をしない。
- テスト（`tests/docs.rs`）：README と `docs/` とスキルの ` ```req ` の塊のどの行も、`examples/` か `tests/` の `.req` の行であること（`…` で切ってよい）。` ```console ` の塊に出力つきで書いた `yurai` のコマンドを、リポジトリの根で実際に走らせ、書いた行が書いた順に出ること。文書の中の相対リンクの先があること。README に書いたコードの数が台帳の数と同じこと。

### D.5 スキル（`skills/yurai/`）

- dandori と koyomi の `skills/` を手本にする。`SKILL.md` は手で書く（frontmatter に `name`、`description`、`compatibility`、`license: MIT OR Apache-2.0`。いつ使うか、ループ（要件を書く → `check` → 差分を読む → 人に確かめてもらう → `review` → `trace` と `doc` で人に見せる）、言語の一ページ、人に聞くこと（要件の文、出典のどの条か、持ち主、見送りの理由、誰が確かめたか）、診断から直し方、`review` をエージェントが自分の判断で走らせないこと（人が確かめた記録なので、確かめた人の `--by` で、その人が頼んだときだけ走らせる））。
- ほかのファイルは `skills/sync.sh` が `docs/` から写す（リンクはスキルのディレクトリの外へ出ないように書き換える）。`skills/README.md` に入れ方。
- テスト（`tests/skill.rs`）：sync.sh を一時ディレクトリに走らせて写しと同じか、スキルの中のリンクが外へ出ないか、frontmatter の形と `license` が `Cargo.toml` と同じか。

### D.6 ライセンスと出典

- ライセンスは MIT OR Apache-2.0（作者が一式に決めた）。LICENSE-MIT と LICENSE-APACHE はもう置いてある。`Cargo.toml` の `license`、README.md の「License」、README.ja.md の「ライセンス」、スキルの frontmatter の `license` に書く。
- `THIRD_PARTY_NOTICES.md`：例とテストに写した出典の写し（e-Gov の民法・印紙税法・租税特別措置法の条の XML、内閣府の祝日の CSV（公共データ利用規約（第1.0版）、出典の書き方）、eCFR の 29 CFR 1910.157）の扱いを、koyomi の THIRD_PARTY_NOTICES.md の書き方にそろえて書く。一式の例から写した `.rule`・`.cal`・`.book`・`.geas`・`server.py`・`.proto` は同じ作者の同じライセンスのものであることと、元のリポジトリを書く。ReqIF のスキーマはリポジトリに入れない（C.10）ので載せない。

### D.7 D の完了の条件

- `cargo test -- --nocapture` が全部通り、この機械で SKIP が 0（Chrome のスクリーンショットも撮れる）。
- 例ごとの doc の golden（英語と日本語、Markdown と HTML）があり、HTML のテストが通る。
- 例の検査の結果が D.2 のとおり（`民法の期間_読み方を変えた/` だけが E303 三つで止まる）。
- README.md と README.ja.md の中の `.req`、コマンドの出力、診断が、`tests/docs.rs` の確かめを通る。
- `docs/codes.md` と `docs/codes.ja.md` がいまの `explain` の出力と同じ。
- スキルのテストが通る。
- DESIGN の形の案が、どれも実物に差し替わっている（`grep -n '形の案' DESIGN.md` が何も出さない）。
- 報告に、README に貼った出力をどのコマンドで取ったかを書く。

## 5. 次の段階への申し送り

### 5.1 A から B へ（A の段階で書いた）

- DESIGN の端のハッシュの値は、A の段階の試作で出した。B の yurai が違う値を出したら、まず DESIGN 4.1 と 3.2 の定義に照らす（とくに、要件の端の中身の行の並べ方と最後の改行、決まった形の JSON のキーの順と字下げ）。
- 法令の写しは、koyomi と rulec の例から写す（B.4、C.2、C.3）。取り直さない。e-Gov と eCFR に取りに行くのは、C.12 の `YURAI_NET=1` のテストだけである。
- A の段階で確かめた一式の振る舞い（DESIGN 19 章）：三つのツールの `source_sha256` はファイルのバイト列のハッシュ、行ごとの引用はどの JSON にも無い、検査を通らないファイルには api が出ない、geas の記録と `affected --json` の形。どれかが変わっていたら（ツールのバージョンが上がって）、DESIGN 3 章を直してから進む。
- 作者が決めるべきだったかもしれないこと（A の報告で挙げたもの）：看板の言い方、要件の端にリンク元のハッシュを入れて先のリンクを一本ずつ確かめさせること（DESIGN 4.1）、確かめた記録に人と日付を書くこと（4.2）、`reviewed/` を git に入れること（4.4）、rulec・koyomi の日付・geas の端をファイル全体にしたこと（3.2）、dandori をファイルでだけ名指すこと（2.7、3.5）、一式に出してほしいもの（3.5）、PROV-N と PROV-JSON の両方を出すこと（13 章）。作者の返事で変わったら、DESIGN と、この計画の該当の項を直してから進める。
- sakai と突き合わせる名指し方（DESIGN 2.8）が、作者か突き合わせた人の判断で変わったら、2 章と、B.3 と C.2〜C.6 の名前の読み方を直す。（B の段階で決着した。下の 5.2。）

### 5.2 B から C へ（B の段階で書いた）

- 名指し方は `tests/fixtures/naming.tsv`（sakai と同じ表）で決着した。C で種類を読むとき、DESIGN 2.3 の表の「名前を読むところ」に従う。rulec の `enum`・`value` は `rulec api` の `python.enums[]`、`machine` は `rulec api` の `machine.name` である（B の段階で `rulec api` を走らせて確かめた）。`input` と `output` を `graph` と `api` の両方から読み、一致することを確かめる。
- ツールを読むところは `src/check.rs` の `not_yet` と `src/ends.rs` の `artifact_end`（`Unread::NotYet`）と `src/coverage.rs` の `gather` に集めてある。C はそこを `src/tools/` の読み方に差し替える。借りた出典は `src/sources.rs` の `Resolved::Borrowed` で止めてある。
- 印の並べ方と説明は `src/marks.rs`。まとまりの元は `cause_of`（要件の端が変わったのは、どの上の端のせいか）で決める。端がファイル全体の種類（rulec、koyomi の日付、geas、dandori、sakai）では、`Thing::Artifact` を、名指しではなくファイルの名指し（`Name::file()`）にすると、同じ `.cal` を指す三本が一つのまとまりになり、差分も一度だけになる（DESIGN 4.3 の例）。B の `file` は名指しがファイルそのものなので、いまは区別が要らない。
- 端の値：B の yurai は、要件の端（`a9ebc73907faddc8`、`465b83ed8c251406`、`d4f2d2a67322df17`）と `file "民法の期間.cal"`（`c9b94eecde23e6b5`）で試作と同じ値を出した。C.7 の借りた出典で同じ値が出ればよい。
- テストの一時ディレクトリは `std::env::temp_dir()` の下に作り、終われば消す。この機械では `TMPDIR` を作業場所に向けて回した。

### 5.3 C の前半から次へ（C の前半で書いた）

- 書き出しは `src/export/mod.rs` の `Graph` を読む。一式のツールの成果物を読むようになったら、`graph` の中で、借りた出典の条を `SourceNode` に（`SourceKey::Law` はデータベース、ID、時点、条なので、借りた条も自分で写した条と同じ識別子になる）、成果物が固定している条を新しい関係（ReqIF なら型 `yurai pins` の `SPEC-RELATION`、PROV なら `wasInfluencedBy(成果物, 出典の条, [prov:type='yurai:pins'])`）に足す。テストの `prov_counts` と `references` は、そのまま新しいテストの材料にも使える。
- `source outdated` が借りた出典で exit 2 にするところは `src/main.rs` の `source_cmd`（`yurai::fetch::borrowed`）。借りた出典の固定はツールの api の `sources[].pins` から、写しはツールのファイルの隣から読む（C.7）。条を固定している成果物は、`src/fetch.rs` の `reach_lines` に一行足せば言える。
- 診断と `trace` と `source` のコマンドが言うファイルのパスは、走らせたディレクトリからの相対にした（DESIGN 2.2。`Project::shown`）。ツールのファイルや proto のパスを文面に出すときも、`shown` を通す。JSON はルートからの相対のまま。
- 本物の e-Gov には、2026-10-03 10:38 に一度だけ問い合わせた（DESIGN 19 章）。eCFR には問い合わせていない。`YURAI_NET=1 cargo test --test fetch -- --nocapture` で両方に問い合わせられる。
