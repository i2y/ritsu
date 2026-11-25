# ringi 実装計画

DESIGN.md を仕様として、ringi を十の段階（B、C1、C2、C3、C4、C5、D、E、F、G）で作る。どの段階も、ここに書いた順に進め、各段階の最後にある完了の条件が全部成り立ったら終わりにする。一つの段階は、一人の担当が一続きにやり切れる大きさにした。DESIGN.md と違うことをしたくなったら、先に DESIGN.md に決めたことと理由と捨てたものを書き、報告で言う。

サーバー（言語ごとのフレームワーク）と web の画面は、ritsu のリポジトリの外の別のリポジトリで、ritsu の段階 D と E のあとに作る（11 章）。

段階 A（設計。DESIGN.md、この計画、例の下書き）、段階 B、段階 C1、段階 C2 は 2026-11-14 に、段階 C2b、C3、C4、C5、D は 2026-11-15 に、段階 D2（コメント）とその続き（代理の操作の履歴、申請者の側の人、E301 の足す行）と、段階 E とその続き（画面の検査を機械の断りにそろえる）は 2026-11-16 に終えた（13〜25 章）。

| 段階 | すること | 主に読む DESIGN の章 |
|---|---|---|
| B | クレートと登録、字句と構文、名前と `use` の読み込み、フォームの型、診断と台帳、`ritsu ringi check` と `ritsu check` への組み込み（ringi だけで決まる検査） | 3、4、9.1、9.5、16 |
| C1 | 下ろすことと行の対応、Overlay への置き方、口の `findings` と `Requests`、診断を ringi の言葉に戻すこと、経路の表と条件と期限の検査、主張と例 | 4、5.3、5.10、6、9.2、9.4 |
| C2 | 組織とマスタ：形の宣言、版と期間つきのデータ、代理、承認者の決め方の解き方、E5xx、`simulate` | 4.6、5.2、5.4、5.8、8、10.3 |
| C3 | チケットの機械の参照インタプリタ、シナリオと `run`、帳簿の仮押さえ、終わることといちばん長い日数、E4xx、ベクタ | 5.5〜5.10、7、10 |
| C4 | 言語をまたぐ検査（X18、X19、X20）、口の `Gates::allowed_with` と `Flows::declared`、ツール名 `ringi` と `Items`・`References`（yuen と sakai から指す） | 6.6、9.3、9.4、15 |
| C5 | 同じ承認者が離れたステップに当たるときに、最後のステップで一度だけ答えてもらう書き方（`same approver approves once, at the last stage`）。機械、検査の W506、`simulate`、いちばん長い日数、例（20 章） | 5.12、7.1、7.3、7.6、8.6、8.7、10.2〜10.4 |
| D | 生成：チケットの機械の kaname のモジュール、申請ごとのつなぎの `.kn`、`ritsu gen` の言語のパッケージと `--target requests`、OpenAPI、MCP の道具、組織のスキーマ、出力先ごとのベクタの突き合わせ | 12、15.4、18 |
| E | 画面：A2UI のテンプレート、ritsu のカタログ、`functions.js`、見せた値と送る値の突き合わせ、文言 | 13 |
| F | ページ（`doc`、`diff`、いちばん長い場合、組織の版の前後）、`explain` の台帳、flow がチケットを開く側の X20、例を `examples/` へ、README、スキル、十番目の言語にすること | 9.3、11、14、15.4、16、19 |
| G | Lean のモデルと証明、Rust との突き合わせ | 17 |

F は三つに分けて走らせる。F1（例を `examples/` へ、見本のシナリオ、入口のエラーの文、flow がチケットを開く側の X20）は E より先に、F2（ページ）と F3（文書と十番目の言語）は E のあとに走らせる。例の置き場とベクタの本数を先に決めておけば、E が作る golden と試験を書き直さずに済むからである。章の順（8 章が E、9 章が F）は変えない。

## 0. 全部の段階に共通の決まり

- **決めてあること**：中に書いた物は、今の言語のファイルへ下ろす（DESIGN 6 章。根の DESIGN の P1 に読み方の一文を足す）。チケットの機械は kaname のモジュール `ticket` で出す（DESIGN 12.1）。構文の語（DESIGN 3.6）、チケットの決まり四つ（取り下げたら申請者の手元に戻る、進行中のチケットは出したときの版で終わる、保留の上限はチケットごとの合計、エージェントは却下しない。DESIGN 5.7、7.7、15.1）、出力先の名前 `ritsu gen --target requests`（DESIGN 12.2）も決めてある。サーバーは言語ごとのフレームワークにし、一つ目の言語を Go にした（DESIGN 12.5、11 章）。二つ目からの言語と、ringi という名前は、まだ決めていない。
- git のコミットと push をしない。変更は作業ツリーに残し、段階の終わりにまとめて記録する。
- 依存の決まり（根の DESIGN 3.1）：ringi のクレートは ritsu-base、ritsu-units、ritsu-ports、ritsu-emit にだけ依存する。serde_json も使わない（JSON は ritsu-base のもの）。rulec、koyomi、chobo、kaname、dandori、sekisho は dev-dependency だけ。`cargo xtask deps` と `ritsu check ritsu.ctx` が通ること。
- テストは、書いているあいだは `cargo check -p ringi` だけにし、項目を書き終えたら、その項目のテストを一回回す。触ったクレートのテストと、変えた口を使う試験だけを回す。ワークスペース全体のテストは回さない。`--test-threads` は 6 まで、`CARGO_BUILD_JOBS=6`。ログを `reports/ringi-<段階>-logs/` に残し、`-- --nocapture` で SKIP の行を読んでから報告する。
- golden は `RINGI_BLESS=1`（ritsu の側は `RITSU_BLESS=1`）で取り直し、差分を読んでから報告する。
- 例、テストの材料、golden は英語が先で、日本語は `<名前>.ja.ringi` の版か、日本語の名前の材料として横に置く。日本語のものは消さない。
- 日本語（DESIGN、PLAN、`--lang ja` の診断、ページ、README.ja.md）は、日本語の例にかけた出力を読み直す。英語の概念語を漢字に直訳した造語をしない。読む人にしてほしいことは「〜してください」と書く。文書に、時刻の言葉や、だれが決めたか・頼んだかを書かない。
- 文書に載せる出力と数は、実際に走らせたものを貼る。案として書いたもの（DESIGN の 11 章の `doc` と `diff` の出力、9.3 の暦の日数）は、その出力を作った段階で実物に差し替える。
- 成果物に、手元の絶対パス、ユーザー名、マシン名を入れない。ほかの会社の製品の名前と評判を、リポジトリの中に書かない（段階の終わりに `git grep -i` で確かめる）。
- 公開リポジトリと npm のコード（A2UI のレンダラなど）を手元で走らせない。走らせるときはサンドボックスで。

## 1. ディレクトリ

```
crates/ringi/
  Cargo.toml  DESIGN.md  PLAN.md  README.md  README.ja.md                       (A、F)
  src/lib.rs  src/main.rs  src/run.rs  src/cli.rs                                (B。run.rs はコマンドを関数にしたもの)
  src/kw.rs  src/lex.rs  src/parse.rs  src/ast.rs  src/diag.rs  src/codes.rs     (B)
  src/names.rs  src/types.rs  src/form.rs  src/route.rs  src/security.rs         (B)
  src/check.rs  src/suite.rs  src/ports.rs                                       (B。ports.rs の口は C1、C4 で足す)
  src/lower/{mod,route,form,terms,words,lines}.rs  src/mapped.rs  src/claims.rs  (C1)
  src/lower/glue/{mod,form,on}.rs  src/lower/shape.rs                           (D。申請ごとのつなぎの .kn と、約束の JSON の形)
  src/lower/ticket.kn  src/lower/days.cal                                        (D。チケットの機械の本体と、それが読む日の番号)
  src/org/{mod,data,masters,resolve,listing}.rs  src/usable.rs  src/simulate.rs  (C2)
  src/engine/{mod,state,event,effects,part,eval,stages,timers,on,act,wire}.rs    (C3。チケットの機械)
  src/taken.rs  src/scenario.rs  src/ends.rs  src/vectors.rs                     (C3。フォームの値の受け取り、シナリオと run、終わることと仮押さえ、ベクタ)
  src/items.rs  src/api.rs  src/ticket.rs  src/apart.rs                          (C4。中の物、api、チケットの事実、別のカレンダーで数える催促。C1 の口 Requests と References は ports.rs)
  src/gen/{mod,types,openapi,mcp,orgschema}.rs                                   (D。--target requests)
  src/screens/{mod,a2ui,catalog,functions,bindings}.rs  tests/fixtures/a2ui-1.0/  (E。1.0 のスキーマは試験の材料として手元に置く)
  src/doc/{mod,route,diff,longest,versions}.rs                                   (F)
  examples/  examples/{shared,org,masters}/                                      (A。F1 で design/ から移した。五つの申請の種類の英日と、それが読む組織、マスタ、共有の物)
  docs/reference.md  docs/codes.md  docs/codes.ja.md                             (B の台帳、F)
  tests/  tests/golden/{codes,lowered,design,run,vectors,doc}/  tests/mutants/  tests/machine/  (各段階。tests/machine/ は機械のための申請。画面の golden は crates/ritsu/tests/golden/requests/a2ui/)
proofs/RingiCore/  proofs/RingiCore.lean                                         (G)
skills/ringi/{SKILL.md,reference.md,codes.md}                                    (F)
```

## 2. 段階 B：構文、名前、型、check

### B.1 クレートと登録

- `crates/ringi` を作り、ワークスペースに入れる。依存は 0 章のとおり。
- `ritsu-project` に `.ringi` のファイルの種類を足す。`ORDER` では sekisho のあと、yuen と sakai の前に置く（DESIGN 16.3）。`ritsu check` のまとめの行に `ringi N` を出す。
- `crates/ritsu` の `main.rs` の言語の振り分けと `languages.rs` に `ringi` を足す。`cli.rs` では、`ritsu ringi` を動かすが `--help` には出さない（kaname の段階 B と同じく、`NOT_LISTED_YET` のような一覧を置く）。`cli::LANGUAGES` に入れるのは F.3 で、`crates/ritsu/tests` の `readme.rs`、`release.rs`、`links.rs`、`npm.rs`、`skill.rs` が、その一覧と README、配布物、スキルの一致を求めるからである。クレートのバイナリ（`ringi`）も、ほかの言語と同じく作る。
- `xtask` の `LANGUAGES` に `ringi`（`WITH_SERDE` には入れない）。

### B.2 字句と構文

- 四つのファイルの種類（`request`、`route`、`organization`、`masters`）の字句と構文（DESIGN 3.3）。表の行は rulec と同じ綴りで読む。
- 3.6 のぶつかりの表のとおり、`step` を書いた行には E001 で `stage` を勧める。
- `labels` のブロック。

### B.3 名前と `use` の読み込み

- 名前の四つの置き場（フィールド、ステップ、承認者の決め方、表）と、`名前(ascii)` の書き方（kaname と同じ）。
- `use` の読み込みと種類の確かめ（E005）。ringi のファイル（組織、マスタ、共有の経路）は ringi が読み、ほかの言語のファイルは口で尋ねるので、この段階では、在ることと種類だけを確かめる。

### B.4 フォームの型

- 4.1 の型、範囲と長さ（E101、E102）、`optional`、`required when`、`shown when`、`approvers only`、明細。
- 式と条件の型（比べる値が型に合うか）。

### B.5 診断と台帳

- 9.5 の E0xx、E1xx（E103、E105、E107 を除く。C1 と C2 で）、E3xx の ringi だけで決まるもの（E304、E305、E307、E308）、W109、W901（ritsu-base の `secrets` で、ほかの言語と同じ形）の診断と、英語と日本語の文。
- `docs/codes.md`、`docs/codes.ja.md`、`ritsu ringi explain`。

### B.6 `check` と `ritsu check`

- `ritsu ringi check`、`--format json`、`--lang ja`、終了コード。
- クレートのバイナリは、ほかの言語が要る `.ringi`（`use` で今の言語のファイルを読むか、下ろす物がある）に、要る言語ごとに一つの E008 を出して 2 で終わる（DESIGN 16.3）。

### B.7 テスト

- 字句、構文、名前の単体の試験。
- B で足したどのコードにも、英語と日本語の golden と変異。
- `examples/` の 10 本の `.ringi` と共有の 3 本が、B の検査を通る（下ろす検査と組織の検査はまだ無い）。

### B.8 B の完了の条件

- `cargo test -p ringi` が通る。`ritsu check` が `examples/` の `.ringi` を読み、`ringi 13` と数える。
- `cargo xtask deps` が通る。
- 台帳の全部のコードに、golden と変異がある。

## 3. 段階 C1：下ろすことと、診断を ringi の言葉に戻すこと

### C1.1 下ろす

- 経路の表とステップの条件を `route.rule` に、フォームの自動計算と条件を `form.rule` に、名前の付いた表を `<名前>.rule` に、期限と催促を `terms.cal` に、選択の値を `choices.rule` に下ろす（DESIGN 6.4）。`tests/golden/lowered/expense/` の五つは `ringi extract` の出力で、どれも公開版で検査を通る。
- 名前の付け替え（キーワードとぶつかる名前）と、行の対応。
- 条件の偽の側の行、空のセルの `none` を作る。

### C1.2 Overlay に置く

- ringi の `check` が、`.ringi` ごとに下ろしたファイルを `<ファイル>.lowered/` のパスで Overlay（`ritsu_base::fs::current()` の上に重ねる）に置き、rulec と koyomi の口（`Rules::findings`、`Dates::findings`）で尋ねる。`ritsu ringi check` と `ritsu check` が同じ道を通る（DESIGN 6.5）。実のファイルがあれば E201。
- wasm（`crates/ritsu-wasm`）でも同じに動くこと。
- `ritsu ringi extract <file> --out <dir>`（DESIGN 16.1）：Overlay に置くのと同じ中身を、ディスクに書き出す。書いたファイルが、そのまま今の言語の検査を通ること。

### C1.3 口

- `Requests`（この段階では、下ろしたファイルと行の対応、フォームの事実）。
- `Rules::findings`、`Dates::findings`（診断ごとのコード、行、例の入力、行、直し方。`--format json` と同じ中身）。
- 口を変えたら `crates/ritsu-ports/tests/ports.rs` と、口を使う言語の試験を回す。

### C1.4 診断を戻す

- 9.2 の対応表のコード（E301、E302、E303、W303、E104、E306、E403、E404、E601）と、それ以外の E201 と W201。E103（どう入れても満たせない範囲）は、ringi が区間の計算で見つける（DESIGN 4.1）。
- 一つの原因から出た診断をまとめる（DESIGN 6.5。21 章の 7）。
- 足す行の形を ringi の表の形で作る（21 章の 9）。

### C1.5 主張と例

- `possible` と `every stage reminds before it is due` を下ろす。`only when` は `Rules::outputs_over` で尋ねる（E306）。
- `examples`（E601）。
- E105（必須なのに隠れる）。
- 21 章の 5（列挙が `use rule` の別名ごとに別の型になる件）を、下ろした `.kn` を作る前に、`.rule` の段で確かめておく。

### C1.6 C1 の完了の条件

- `examples/` の例の下ろしたファイルが、`tests/golden/lowered/expense/` の手本と同じ中身になり（名前と順のほかは）、rulec と koyomi の検査を通る。
- 下ろしたファイルに入れた変異（行を抜く、範囲を広げる、催促を期限のあとにする）の診断が、どれも ringi の行と ringi のコードで出る。
- `ritsu check examples` が、yuen の E011 のほかは通る。

## 4. 段階 C2：組織とマスタ

### C2.1 形の宣言とデータ

- 組織のファイル（役職、チームの種類、データの置き場）と、版のディレクトリ、三つの CSV と同じ形の JSON、代理の CSV（DESIGN 8.1〜8.4、5.8）。E501。
- マスタのファイルとデータ、有効期間（4.6）。E107、E505。
- `ritsu ringi check` に、ディレクトリと `--org`、`--at` を足す（DESIGN 16.1）。`extract` と `simulate` も同じ形で取る。
- 使う日の範囲（DESIGN 9.6）：始まりと終わりを一か所で求め、E107、E404、下ろした `terms.cal` と `form.cal` の入力の範囲、一覧に入れる組織の版が同じ範囲を使う。カレンダーの終わりは koyomi に尋ねて求める。終わりは `check` の出力（テキストと JSON の `usable`）と口の `Requests::usable` に事実として出し、`--through <日付>`（`ritsu check --through` も）を渡したときだけ E107 と E404 にする。

### C2.2 承認者の決め方を解く

- 8.5 の解き方（上長、チームの長、最も近い役職の人、チーム、指名、表引き、自己承認）。たどる深さの上限。
- 代理を足すこと（答えるときに有効な代理）。

### C2.3 版ごとの一覧

- 版ごと、申請の種類ごと、申請者ごとに、各ステップの承認者を決め、E502、W502、E503、E504、W506 を出す。データが無ければ W500（決められない）。
- `examples/org/2027-04-01/` で、DESIGN 8.6 の三件が実物として出ること。出たら DESIGN 8.6 の案を実物に差し替える。

### C2.4 `simulate`

- `ritsu ringi simulate`（DESIGN 10.3）と `--every-requester`。期限は下ろした `terms.cal` を `Dates::eval` で。代理（期間と申請の種類の絞り）を見せる。

### C2.5 C2 の完了の条件

- 組織の試験（版の選び方、予約、輪、兼務、代理の期間と申請の種類）が通る。
- `examples/` の例を二つの版で検査して、8.6 の三件と、ほかに出たものを報告に書く。

## 5. 段階 C3：チケットの機械

### C3.1 参照インタプリタ

- DESIGN 7 章の状態、イベント、遷移、することの一覧を、ringi のクレートの Rust で書く（`start` と `next`。`src/engine/`）。二つとも、時刻、組織の部分、フォームの値を引数で受け、ほかに何も読まない。表と条件と計算は `Rules::eval`、期限は `Dates::eval`、営業日は `Dates::calendar` の休みで数える。帳簿には触らず、仮押さえ、確定、取消を、することの一覧に入れて返す。
- 使う日の範囲（DESIGN 9.6）の外の日に始まる申請と再申請とステップを、どのデータが足りないか（どのマスタのどの行まで、どのカレンダーのどの日まで）を言って断る。範囲は `check` が求めた物（`Usable`）を使い、もう一度数えない。期限を過ぎて上へ回すことは、ステップを始めないので断らない。範囲の終わりは、保留を使い切ったステップの時計と、再申請の期限まで数えられる日にする。
- 議決と却下（届かなくなったら終わる）、差し戻しと取り下げを合わせた回数と `round`、再申請と破棄、承認の取り消し（期限を動かさない）、保留の合計、上へ回す（元の期限が切れた時刻から、期限と催促を数える）、時計の出口。催促は、期限より前に来るときだけ置く。鳴った時計のイベントが渡されるまで、ほかのイベントを断る（DESIGN 7.6）。
- 承認者のいないステップ（DESIGN 5.4、7.3）：`when empty` を構文木の `Deadline::Empty` から読む（下ろした物には出ない）。上へ回す、いちばん上で `then skip` なら飛ばし、`then <決め方>` ならその決め方を申請者について解いた人へ回し（そこにも人がいなければ止まる）、ほかは止まって `add approver` を待つ。どれも履歴に残す。期限を過ぎて上へ回す先（DESIGN 5.10）：表が無ければ期限を過ぎた人の上の長、表があれば表の決め方を申請者について解いた人、二度目は一度目に回した先から。回す先が無ければ `escalate` のあとに書いたことをその期限でし、履歴に「回す先が無い」と残す。どのステップにも答える人がいない申請者のチケット（W510）は、承認者のいないステップを一つずつ飛ばして履歴に残し、出したとたんに承認で終わる。検査の一覧（`org::listing`）と同じ解き方を使い、もう一度書かない。
- 申請者が組織からいなくなったチケット（DESIGN 7.7）：申請者から決める決め方がだれも見つけなければ、`when empty` に何と書いてあっても止まる。いなくなった人は、再申請も、取り下げも、破棄もできない。
- 同じ承認者が続くステップ（DESIGN 5.12、7.3）：`same approver approves once` を書いた申請の種類で、議決に届いた `approve` のあと、次のステップの承認者が 5.12 の決まりを満たせば、そのステップを済ませて次へ進む。することの一覧の `record` に `once_with` を書く。どのステップが済むかは、検査と `simulate` と同じ関数（`org::listing` の `before_on`、`origin_on`）で決める。
- だれが申請を出せるか（DESIGN 5.13、7.3）：`submit` のときに効いている版で、申請者（代理申請なら代わられた人）が当たるかを決め、当たらなければチケットを作らずに断る。`resubmit` は断らない。判定は検査と `simulate` と同じ物（`org::submit::Line`）を使う。
- フォーム（DESIGN 4.1、4.3、4.4）：人が入れるフィールドの範囲と字数、必須、マスタの行、明細の行の数を確かめ、計算するフィールドは下ろした規則に尋ねる。計算に書いた範囲は `define … range` と `sum … range` に下ろしてあり、入口の断りを、フィールドの名前と範囲の文に直す。値の受け取りは `src/taken.rs` の一か所で、`simulate --form`、シナリオ、機械が同じ物を呼ぶ。
- JSON の名前は、括弧の中の名前にする（DESIGN 3.4。`src/engine/wire.rs`）。

### C3.2 シナリオと `run`

- `scenario` の `given`、イベントの表（引数つきのイベント、人とチームの値）、期待のステップと状態。行ごとに機械へ渡し、帳簿の分を `Books::open` の帳簿に流す。E602、E603、E604。時計の行は、機械が置いた時刻ちょうどに書く。`check` もシナリオを流す。
- `ritsu ringi run <file> [--scenario <名前>] [--org <dir>] [--format json]`（DESIGN 10.2、16.2）。シナリオの無いファイルは、`vectors` と同じに 2 で終わる。

### C3.3 終わることと、いちばん長い日数

- 7.6 の数え方をコードにする（`src/ends.rs`）。E401、W402 と、`ok` の下の行。`examples/` の五つの例の主張（93、246、55、192、40 営業日）を数え直して確かめる。
- 仮押さえの開閉（E405、E406、E407）。仮押さえの条件か引数が、承認者の直せるフィールドを読む形も E405。

### C3.4 ベクタ

- `ritsu ringi vectors <file> [--list] [--org <dir>]`（DESIGN 10.4）。どのベクタも、参照インタプリタの遷移そのものにする。シナリオの行、議決の境、差し戻しの上限、休みをまたぐ期限、どの申請者も、代理、版をまたぐチケット、フォームの数と行の数の境、例の行から作る。

### C3.5 C3 の完了の条件

- 五つの例のシナリオが、書いたとおりに進む。
- 状態と遷移の表（7.3）の各行に、試験が一つ以上ある。
- 21 章の 1 と 2（シナリオが日付と帳簿の答えを持たないこと、上へ回したあとの期限）の試験がある。
- いちばん上で上へ回せないステップ（`then skip` で飛ぶ、`then <決め方>` で回る、その決め方にも人がいないか、書かないか `then stay` で止まる）と、期限を過ぎて回す先の無いステップ（`then expire`、`then stay`、二度目、上へ回った先の人と `then` の決め方の人が期限を過ぎる）と、どのステップにも答える人がいない申請者の試験があり、E502、W502、W507、W510 と、`ok` の下の数の JSON の `cases` が言う申請者と、同じ答えになる。
- `same approver approves once` の試験（済む、あいだのステップで済まない、並列のほかの部分、エージェント、上へ回したあと、代理、差し戻し、取り消し）があり、`simulate --form` が済むと言うステップと同じステップが済む。
- `submitted by` の試験（出せる人、出せない人、代わられた人が出せない代理申請、出したあとの版で外れた人の再申請）があり、`simulate --requester` が断る人と同じ人を断る。
- 実装を変えると試験が落ちることを、変異で確かめる（却下をすぐ終わりにする、議決の数を一つずらす、差し戻しの上限を一つ増やす、期限を走り始めの時刻で数える、上へ回したあとに数え直さない、上へ回したあとの催促を置かない、など）。

## 6. 段階 C4：言語をまたぐ検査

### C4.1 口

- `Gates::allowed_with`（属性の一部の値を決めて尋ねる）を sekisho に、`Flows::declared`（flow の入力と出力の型と範囲）を dandori に足す。`Requests` に、終わり方、帳簿と flow の呼び出し、`round` ごとのいちばん長い時間を足す。

### C4.2 ritsu-cross

- X18（E911、W911）、X19（E209、W209）、X20（E210、W210）を `crates/ritsu-cross` に足し、`codes.rs`、`docs/codes.md`、`docs/codes.ja.md`、`ritsu explain` に載せる。
- 根の DESIGN 7.2 の表に足す文は notes にある。根の文書は担当が直さない。

### C4.3 ツール名と、中の物

- `ritsu-base` の `naming.rs` の `Tool::ALL` に `Tool::Ringi` を入れ（16 個に）、`Tool::kinds` に ringi の中の物の種類を書き、`tests/fixtures/naming.tsv` に行を足す（`Tool::Ringi` そのものは、`ritsu-project` がファイルを見分けるために B で足した）。yuen と sakai のツール名の一覧の文と golden（E011 など）を直す。
- ringi が `Items` と `References` に答える（表、経路、承認者の決め方、期限、チケットの決まり、主張、例）。
- `ritsu ringi api`（DESIGN 16.1）：ファイルが宣言することを JSON で。`Items` と同じ物の種類と、`Requests` の事実（使う日の範囲と、それを決めているデータも）。
- `examples/shared/expense.req` が、E301（確かめ待ち）のほかは通ること。

### C4.4 C4 の完了の条件

- X18〜X20 の成り立つ例と成り立たない例の golden が `crates/ritsu/tests/cross.rs` にある。
- 下ろした `.kn` が無くても（D の前）、X2〜X6 の判定に影響しないことを確かめる。

## 7. 段階 D：生成

### D.1 チケットの機械の本体

- 参照インタプリタの型（`src/engine/state.rs`、`event.rs`、`effects.rs`、`part.rs`）を写して、kaname のモジュール `ticket`（`src/lower/ticket.kn`）を書く（DESIGN 7 章の全部の状態とイベント）。`ritsu kaname check` で total、警告なし。
- チケットの版（`revision`）を持ち、時計のほかのイベントの版が今のチケットと違えば `stale` で断る（DESIGN 7.1、7.2、12.4）。
- `ritsu-project` が、`.ringi` のあるプロジェクトでこのモジュールを根の下の `.lowered/ticket.kn` に Overlay で置く。

### D.2 申請ごとのつなぎ

- 下ろした規則と日付を読み、フォームの値を確かめ、申請の種類が言うことをデータにしてモジュール `ticket` に渡す `.kn` を下ろす（`<ファイル>.lowered/<名前>.kn`）。X2〜X6 がかかること。
- 計算するフィールド（DESIGN 4.4）：検査と参照インタプリタは範囲を書いた `form.rule` を読み、生成するコードは届く範囲だけを書いた写しの `values.rule` を読む。つなぎは写しが返した値を、書いた範囲と `if` で比べ（行ごとの計算は `max` と `min` で）、外なら参照インタプリタと同じ `form` の断りにする。比べる順は rulec の段の順で、断りの文は `out_of_range` と同じ関数から作る。写しが int64 に収まると示せない申請は、`ritsu ringi check` の E201 にする。明細の行の数の上限も、C3.1 と同じ形で守る。
- 出せる人の判定（DESIGN 5.13）を、グループを定数に書いて生成する。7.5 の組織の部分を受け取り、C3.1 と同じ形で断る。
- 下ろした `.kn` に kaname が何か言えば E201 に戻し、ringi の不具合として注を付ける（DESIGN 9.2）。
- 使う日の範囲の外で始まる申請とステップを、C3.1 と同じ形で断る（足りないデータを言う）。範囲は `Requests::usable` から受け取り、生成するコードに定数として書く。

### D.3 `ritsu gen`

- 言語のパッケージに、下ろした規則と日付、つなぎ、機械のコードと、申請の種類の一覧（ASCII の名前、版、`start` と `next` の JSON の入口、約束の版。DESIGN 12.2）を入れる（TypeScript、Python、Go、Rust）。`.ringi` の無いプロジェクトの生成物は 1 バイトも変えない。
- `--target requests`：`types/`、`openapi.json`、`mcp/tools.json`、`organization/`、`vectors/`（DESIGN 12.2）。A2UI は E で足す。`types/<名前>.json` に使う日の範囲と、それを決めているデータを書く（サーバーが、終わりの日の前に管理する人へ知らせるため）。出せる人（`submitted by` のグループ）も書く（サーバーが、申請の種類の一覧をその人が出せる物に絞るため）。`vectors/` は、生成したコードが受け渡す約束の形（DESIGN 12.4）で書く。
- `--check` と `--format json` を今と同じにする。ringi のファイルが検査を通らなければ、ringi の物を書かない（DESIGN 21 章の 10）。組織の版ごとの一覧の誤り（E502〜E504）では止めず、申請ごとに一行の警告で言う（DESIGN 12.3）。

### D.4 突き合わせ

- 生成したコードを四つの出力先で、ベクタの全部について、申請の種類の一覧の入口を通して流し、参照インタプリタと同じ答えになることを確かめる。断る申請（書いた範囲の外、上限より多い行）も、同じ種類の断りになること。Node、Python、Go が無ければ `SKIP:` の行を出す。

### D.5 D の完了の条件

- 五つの例のパッケージが四つの出力先で作れ、ベクタが全部通る。
- `openapi.json` が OpenAPI 3.1 として読め、dandori の `use openapi` でも読める（DESIGN 15.4 の flow の例で確かめる）。

## 8. 段階 E：画面

### E.1 テンプレート

- 申請書、承認カード、一覧、履歴の A2UI 1.0 のテンプレートを、英語と日本語で作る（DESIGN 13.1、13.4）。
- 承認カードと申請書にコメントの欄を置き、履歴にコメントを見せる（DESIGN 7.2、7.4）。欄は人の操作に添える文と、コメントだけを書く操作（`comment`）の文に使う。履歴は、人のコメントと、エージェントの理由（`agent answered` の行の `comment`）を同じ所に見せる。2,000 文字を越える文は、送る前に、機械の `form` の断りと同じ中身の文で止める。申請者に戻っているチケットでは、だれが差し戻したか（チケットの `sent_back_by`。DESIGN 7.1）を申請者に見せる。
- 出し直しの申請書と承認者の直しのカードでは、見えている任意のフィールドを空にでき、送るときは空にしたフィールドの名前を `clear` に挙げる（値を null にしても消えない。DESIGN 7.3、12.4）。表示の条件が変わって隠れるフィールドの値は、送る前に落として見せる（機械も落とす）。履歴には、行の `cleared` を「空になった」フィールドとして見せる（DESIGN 7.4）。
- `tests/fixtures/a2ui-1.0/` に写した 1.0 のスキーマ（Apache License 2.0 の文と NOTICE を添えた）で検証する。読むのは試験だけである。

### E.2 カタログと関数

- ritsu のカタログ（13.2）の定義と、`functions.js`（下ろした `values.rule` を rulec がページのために書く JavaScript から。DESIGN 13.7）。
- 関数がベクタで参照インタプリタと同じ答えになること（Node が無ければ SKIP）。画面の判定が、フィールドごと、検査の種類ごとに作ったフォームでも、参照インタプリタの判定と同じになること（分かれてよいのは文の両端の空白だけ。画面のほうが厳しい）。

### E.3 突き合わせと文言

- 見せた値と送る値の突き合わせ（E701、W702）、文言の欠け（E703）。
- 計算するフィールドの値が書いた範囲の外になったときの文（どのフィールドの、どの範囲か。英語と日本語）を、人が入れるフィールドの範囲の外と同じ形で、送る前に出す（値は `functions.js` で計算する）。明細の行は上限の数より多く足せないようにし、上限を文で見せる。断りの文は、D のつなぎのコードが返す断りと同じ中身にする。
- 出せない人への断り（だれが、どの版で、どの行で。DESIGN 5.13）を、D のつなぎのコードが返す断りと同じ中身で見せる。
- 確かめで NOT SHOWN になった形（文言を別のパスに置く）が出ないこと。

### E.4 E の完了の条件

- 五つの例の全部の画面が、スキーマと突き合わせを通る。
- レンダラで描いて見るのは、サンドボックスか、別のリポジトリの段階（S2）で行う。ritsu の試験では描かない。

## 9. 段階 F：ページ、例、文書、十番目の言語

### F.1 ページ

- `ritsu ringi doc`（経路を一枚で、`--longest`）、`ritsu ringi diff`（ファイルの前後と、組織の版の前後）。`Rules::diff` を口に足す。英語と日本語、Markdown と HTML。
- `ritsu impact` と `ritsu mod diff` が `.ringi` の変更を ringi の `diff` で言う。
- 使う日の範囲（DESIGN 9.6）を、終わりを決めているデータ（どのマスタのどの行まで、どのカレンダーのどの日まで）と一緒に、ページの頭に見せる。
- 計算するフィールドに書いた範囲を、受け付ける値の範囲としてページに見せる。計算の届く範囲が書いた範囲より広いときは、両方を見せる（`examples/expense.ringi` の `amount` なら、届くのは 1 円から 2,000 万円、受け付けるのは 1 円から 1,000 万円）。経路の表を読む人が、表がどの範囲を覆えば足りるかを読めるようにする。明細の行の数の上限も見せる。
- 出せる人（`submitted by` の文と版ごとの人数。DESIGN 5.13）を経路のページに、出せるようになる人と出せなくなる人を組織の版の前後のページに見せる。

### F.2 例と文書

- 例は `examples/` に置く（F1 で `design/` から移した）。五つの申請の種類の英日と、それが読む組織、マスタ、共有の物（`shared/`、`org/`、`masters/`）を、一つのプロジェクトとして置き、全部の検査とシナリオを通す。DESIGN の出力は走らせた実物にする（F1。README は F3 で書く）。
- 例のシナリオは、承認で終わる物のほかに、却下（購買の取締役会の議決）、差し戻しと出し直し（休暇）、取り下げ（経費。仮押さえの取消も見せる）、期限切れ（契約の再申請の期限）を、どれも一つは持つ（F1。DESIGN 10.4）。却下と差し戻しにはコメントを添え、日本語の名前の版にも同じシナリオを書く。
- `README.md`（英語）と `README.ja.md`（日本語。英語を訳さず、一から書く）。
- `skills/ringi/`（SKILL.md、reference.md、codes.md）と、`skills/README*.md`、`skills/ritsu/SKILL.md` の言語の一覧。
- 根の DESIGN、PLAN、README（英語、日本語、中国語）に書くことは、notes の文を、全体を見ている側が当てる。

### F.3 十番目の言語にすること

- `cli::LANGUAGES` に `ringi` を入れ、`ritsu --help` の言語の一覧と、`ritsu check --help` の拡張子の文に `.ringi` を足す。
- 根の README（英語、日本語、中国語）の言語の表、「Commands」の節のリンクの並び、ringi の節を、`LANGUAGES` と同じ変更で当てる（notes の文。節の `console` の塊は、この段階で走らせた出力を貼る）。`tests/common/mod.rs` の `CODE` に `.ringi` を足すなら、README の英語と日本語に、例のファイルの行の `.ringi` の塊が要る。
- 配布の形（`packaging/` のリンク、npm の bin、Homebrew、Linux のパッケージ）、`crates/ritsu/tests/links.rs`、playground の言語の一覧、サイトのページ。
- 看板の「nine」と「九つ」の書き換えは `git grep -i -E 'nine|九つ|九个|九种'` で探し、その日に何があったかの記録として書いた文は記録のまま残す（kaname の段階 E と同じ）。
- 12 章の名前の一覧の全部の場所を、ここで確かめる。

### F.4 flow がチケットを開く側の X20

- flow が `use openapi` で読んだ `openapi.json` の申請の操作の `x-ritsu-request`（ファイル、名前、版）から `.ringi` を引き、flow が渡すフォームの値の型と範囲を `.ringi` のフォームと比べる。`openapi.json` が `.ringi` の今の版から書いた物でないことも言う（DESIGN 9.3、15.4）。
- flow はチケットを開いたところで終わり、チケットが終わったあとの続きは申請の種類の `on` の行が始める（DESIGN 15.4）。チケットの終わりを開いた flow に返して待つ形は、後の版に回した（DESIGN 2.2、20.2）。

### F.5 F の完了の条件

- `crates/ritsu/tests` の `readme.rs`、`links.rs`、`release.rs`、`entry.rs`、`skill.rs`、`npm.rs` が、`LANGUAGES` に ringi を入れた状態で通る。`readme.rs` は、README に貼った出力と実物が同じことも確かめる。
- flow がチケットを開く側の X20 の、成り立つ例と成り立たない例（型が違う、範囲が広い、`openapi.json` が古い）の golden が `crates/ritsu/tests/cross.rs` にある。
- 日本語の出力を全部読み直し、`--lang ja` の golden を取り直す。

## 10. 段階 G：Lean

- `proofs/RingiCore/` に、DESIGN 17 章のモデルと六つの定理を書く。`sorry` と自前の公理を使わない。
- ベクタを Rust と Lean の両方に流して答えを突き合わせる（`crates/ritsu-model/tests/ringi.rs`）。CI の `proofs.yml` に足す。
- 完了の条件：定理が全部通り、例とテストの全部のベクタで Rust と Lean の答えが同じ。

## 11. 別のリポジトリの段階（フレームワークと web の画面）

別のリポジトリには、言語ごとのフレームワークを置く（DESIGN 12.5。2026-11-15）。生成したパッケージを渡せば、申請の種類の一覧（DESIGN 12.2）から全部の申請の種類が動き、組織ごとに違う所（認証、保管、通知、添付の置き場、組織のデータの取り込み元、flow の動かし方、エージェントのステップ、監査ログの出し先）だけを差し替える。一つ目の言語は Go で、`net/http` の `http.Handler` として出す。本体は PostgreSQL だけで動き、flow を始める申請の種類があるときだけ dandori の temporal-go の出力先を使う（flow のプラグインは別の Go のモジュール）。二つ目からの言語と形は、Go が済んでから決める。プロジェクトの最初の一式を書き出すコマンドは ritsu の側に置き、フレームワークの段階で作る。

ritsu の段階 D（言語のパッケージと申請の種類の一覧、約束の版、`--target requests`）が終わってから始める。S1〜S3 は、Go のフレームワークの段階である。

| 段階 | すること | 完了の条件 |
|---|---|---|
| S1 | フレームワークの本体：申請の種類の一覧を読む `http.Handler`、`openapi.json` の操作、一つのイベントの順（DESIGN 12.4。gate に尋ねてから `next` を呼び、`submit` は gate の `submit` と `submitted by` の両方を満たすときだけ通る）、チケットと履歴の保管（イベントを追記だけで持つ。PostgreSQL）、申請の種類の一覧をその人が出せる物に絞る、時計、することの一覧を冪等キーで行う（帳簿は chobo のクライアント、flow は dandori の temporal-go の出力先）、組織の版の読み込み、差し替える所 | ベクタの全部を、フレームワークの API を通して流して同じ答えになる。DESIGN 12.4 の適合の試験を通る |
| S2 | 画面の配信：A2UI のテンプレートに値を入れて送る、ボタンの操作をイベントにする、名前の手当て（DESIGN 13.5）、素の HTML の画面（13.6） | 五つの例の申請から決裁までを、画面で通せる。見せた値と送る値の突き合わせを、配信した物にもかける |
| S3 | 管理する人の作業：変更の反映と予約、組織の版の取り込み、yuen の確かめの記録を見せる、部門ごとの gate、監査ログ、MCP サーバー。申請の種類を使える最後の日（DESIGN 9.6）の前に、どのデータを足せばよいかを管理する人へ知らせる | 依頼から反映までを、例の一つで通せる |
| S4 | 後の版（DESIGN 2.2）：チャットの通知と承認、Webhook、SSO、繰り返しの催促 | 項目ごとに決める |

## 12. 名前（ringi）を書く場所の一覧

名前は 2026-11-18 に `ringi` と決めた（サーバーのフレームワークは `ringi-server`）。生成物とサーバーとの約束には名前を出さない（`--target requests`、モジュール `ticket`、`x-ritsu-request`、カタログの部品の `ritsu:`）。名前を書くのは次の場所だけにする。

| 場所 | 書くもの | 段階 |
|---|---|---|
| `crates/ringi/`（クレートとバイナリの名前 `ringi`、`--help` と `--version` の頭）、ワークスペースの `Cargo.lock` | クレートの名前 | B |
| `crates/ritsu-base/src/naming.rs`（`Tool::Ringi` の `"ringi"` と拡張子 `.ringi`。`Tool::ALL` には C4 で入れた） | 言語の名前と拡張子 | B |
| ファイルの拡張子 `.ringi`（`ritsu-project` の `ORDER` と `Joined::ringi`、`ritsu check` のまとめの `ringi N`） | 拡張子と言語の名前 | B |
| `crates/ritsu-project/Cargo.toml`、`crates/ritsu/Cargo.toml` の依存 | クレートの名前 | B |
| `crates/ritsu/src/cli.rs`（`LANGUAGES`）、`languages.rs`、`main.rs`、`crates/ritsu/tests/ringi.rs`、`crates/xtask/src/main.rs` の `LANGUAGES` | `ritsu ringi` | B（動かす）、F（`cli::LANGUAGES` と `--help`） |
| 診断の頭（`error[ringi E301]`）、`docs/codes.md`、`docs/codes.ja.md` | 言語の名前 | B |
| 環境変数 `RINGI_LANG`、`RINGI_BLESS` | 文面の言語、golden の取り直し | B |
| `Tool::Ringi` を挙げる一覧と振り分け：`crates/ritsu-base/src/fed.rs`、`crates/ritsu-wasm/src/playground.rs`、`crates/ritsu/src/check.rs`（`ONE_FILE`）、`crates/ritsu-project/src/check.rs`・`project.rs`・`lib.rs`（振り分けと説明） | 言語の名前 | B |
| `crates/ritsu-ports/src/requests.rs`（口 `Requests`）と、同じクレートの `rules.rs`・`dates.rs`・`lib.rs`・`tests/ports.rs` の説明、`crates/ritsu-base/src/fs.rs` と `tests/fs.rs` の Overlay の説明 | 口と Overlay の説明の中の言語の名前 | C1 |
| `crates/ritsu-project/src/joined.rs`（`Joined::requests()`、索引の `with_references(Tool::Ringi, …)`）、`crates/ritsu/src/check/waiting.rs`（`Tool::Ringi => code == "E202"`） | 言語の名前 | C1 |
| `crates/ritsu/tests/check.rs`（`TOOLS`、`a_request_waits_for_what_it_reads`）、`crates/ritsu/tests/playground.rs`（`a_request_is_lowered_in_memory_as_on_the_disk`）、材料の `crates/ritsu/tests/projects/requests/` | 試験の名前と材料の拡張子 | C1 |
| `Tool::ALL`、`tests/fixtures/naming.tsv`、yuen と sakai のツール名の一覧の文と golden | 参照の書き方のツール名（`satisfied by ringi "…"`） | C4 |
| `crates/ritsu-cross/docs/codes*.md` の X18〜X20 の文 | 言語の名前 | C4 |
| `crates/ringi/README.md`、`README.ja.md`、`DESIGN.md`、`PLAN.md`、`docs/reference.md`、`examples/` | 言語の名前。フレームワークの名前とリポジトリへのリンク | F |
| `crates/ringi/src/cli.rs` の `init`、`crates/ritsu/src/languages.rs`、`crates/ritsu/src/ringi_init.rs`（モジュールの名前と、フレームワークのモジュールのパス `FRAMEWORK`）、ひな形の `crates/ritsu/src/ringi_init/`（フレームワークのモジュールのパス、環境変数 `RINGI_OIDC_CLIENT_SECRET`・`RINGI_SESSION_KEYS`、スキーマ `ringi`）、`crates/ritsu/tests/ringi_init.rs` と `tests/golden/ringi-init/` | コマンド `ritsu ringi init` と、フレームワークの名前（DESIGN 16.4） | フレームワークの S6 |
| `skills/ringi/`、`skills/README.md`、`skills/README.ja.md`、`skills/ritsu/SKILL.md` | スキルの名前。`skills/ringi/` にはフレームワークの名前とリポジトリへのリンクも | F |
| 根の `README.md`、`README.ja.md`、`README.zh.md`、`DESIGN.md`、`PLAN.md`、`website/` のページ、playground の言語の一覧 | 言語の一覧と節。README とページの ringi の節には、フレームワークの名前とリポジトリへのリンクも（`crates/ritsu/tests/website.rs` が、ページが名前を出してよいリポジトリとして持つ） | F（notes の文を当てる） |
| `packaging/`（`archive.sh`、`homebrew.sh`、`linux.sh`、`nfpm.yaml`、`npm/bin/ringi.js`、`npm/package.json`、`smoke.sh`）、`crates/ritsu/tests/links.rs` | 言語の名前のリンク | F |
| `ritsu.ctx`、`contexts/requests.ctx`（コンテキスト `Requests(ringi)`）、`contexts/project.ctx`、`contexts/entry.ctx` | 処理系の地図のクレート | B |
| `proofs/RingiCore/`、`proofs/README.md`、`proofs/lakefile.toml`、`crates/ritsu-model/tests/ringi.rs`、`.github/workflows/` | Lean のモデルと CI | G |

## 13. 段階 A で作ったものと確かめたこと（2026-11-14）

- `DESIGN.md`、この計画、例の下書き（`design/` に置いた。F1 で `examples/` に移した）。申請の種類の五つ（英語と日本語の版で十本）、共有の `.ringi` 三本（組織、マスタ、共有の経路）、共有の今の言語のファイル（カレンダー、帳簿二つ、gate、決裁権限表、flow 六つ、要件、規程の条のコピー四つ）、組織のデータ二つの版と代理、マスタのデータ三つ、下ろしたファイルの手本四つ（F1 で経費の例の golden の `tests/golden/lowered/expense/` にした）、チケットの機械の kaname の下書き（F1 で消した。機械の本体は `src/lower/ticket.kn`）。
- 今の言語のファイルは、どれも公開版の 0.55.2 で検査を通る。`ritsu check shared lowered engine` は 17 ファイルを読み、落ちるのは `expense.req` の E011 の 9 件（`ringi` がまだツール名でない）だけ。
- 試し：経路の表から行を抜くと rulec の E101 が例の入力を出し、`--format json` は例の入力と直し方を持つ（口の `Said` は持たない）。`rulec diff` が経路の変更を入力の区間で言う。gate の `action any` の forbid が `resource.requester` を読むと、それを持たない resource で E101 になる。kaname の下書きは、最初の形で W501 を二つ、E401 と W301 を一つずつ出し、直して total になった。チケットの進み方を rulec の機械で書くと通り、終わった却下を開き直す変異は E124 と E126 になった。
- ログ：`reports/ringi-A-logs/design-files.log`、`experiments.log`。

## 14. 段階 B で作ったものと確かめたこと（2026-11-14）

- クレート `crates/ringi`：字句と構文（四つのファイルの種類、表、`labels`）、名前と `use`、フォームと経路の型、診断と台帳、`check` と `explain`。ringi の依存は ritsu-base、ritsu-units、ritsu-ports、ritsu-emit だけ。
- 組み込み：`ritsu ringi`（`ritsu --help` にはまだ出さない）、`ritsu check`（`ORDER` の sekisho のあと、まとめに `ringi N`）、`ringi` という名前のリンク、`Tool::Ringi`（`Tool::ALL` には C4 で入れる）、処理系の地図のコンテキスト `Requests(ringi)`。
- 台帳は 52 個で、いま出すのは 21 個。どれにも英語と日本語の名前の例と、変異がある（変異 110 本と、読み込み先 8 本。golden は変異の 218 本、台帳の例の 42 本）。型の食い違いを言う E009 を足した（DESIGN 9.5）。
- `examples/` の `.ringi` 13 本は、どれも検査を通る（`ritsu check examples/*.ringi examples/shared/*.ringi` は `13 files (ringi 13): all pass`）。`ritsu check examples` は 30 ファイルを読み、落ちるのは `expense.req` の E011 の 9 件だけ（C4.3 で消える）。クレートのバイナリは、組織とマスタを通し、申請の種類と共有の経路には要る言語ごとの E008 を出して 2 で終わる。
- 試験：`cargo xtask test --level fast -p ringi -p ritsu -p ritsu-project -p xtask` は 537 件が通る（ringi 30、ritsu 494、ritsu-project 7、xtask 6。SKIP は 81 行で、どれも tools と platforms の段に回したもの）。`cargo xtask deps`（22 クレート）と `ritsu check ritsu.ctx`（15 コンテキスト）も通る。
- ログ：`reports/ringi-B-logs/`。

## 15. 段階 C1 で作ったものと確かめたこと（2026-11-14）

- 下ろすこと（`src/lower/`）：経路の表とステップの条件を `route.rule` に、フォームの計算と条件を `form.rule` に（営業日の日付の計算は `form.cal` に）、名前の付いた表を `<名前>.rule` に、期限と催促を `terms.cal` に、選択の値を `choices.rule` に下ろす。共有の経路も下ろす。下ろしたファイルの行ごとに、元の申請の行と、それが何かを持つ（`lines.rs`）。`tests/golden/lowered/expense/` は `ringi extract` の出力に置き換え（五つ）、公開版 0.55.2 の `rulec check` と `koyomi check` を通る（`terms.cal` は 146,097 日を数える）。ほかの申請 9 本と共有の経路 1 本の下ろした物は `tests/golden/lowered/` の golden にし、読んで確かめた。
- 確かめ方：ringi の `check` が下ろしたファイルを Overlay に置き、口の `Rules::findings`、`Dates::findings`、`Rules::outputs_over` で尋ね、答えを ringi のコードと行と言葉に戻す（`src/mapped.rs`、`src/claims.rs`。DESIGN 6.5、9.2）。`ritsu ringi check` と `ritsu check` は同じ道を通り、wasm の Memory の上でも同じ答えになる（`crates/ritsu/tests/playground.rs`）。下ろしたファイルのパスは出力に出ない。
- 計算するフィールドに書いた範囲は、受け付ける値の上限と下限にした（DESIGN 4.1）。E103 は、どう入れても満たせない範囲だけを言う。合計は `sum … range` に、合計でない計算は範囲を付けない `define` に下ろす（比べた五つの形と理由は 4.4）。計算するフィールドを読む所は、届く範囲と書いた範囲の重なりで読む。
- 口：`Requests`（下ろしたファイルと行の対応、フォームの事実）、`Rules::findings`、`Dates::findings` を足し、rulec と koyomi が答える。ringi が `References` に答えるので、`use` で読むファイルが通らないときの E202 は、`ritsu check` で、読んだファイルを待つ行になる。
- `ritsu ringi extract <file> --out <dir>`（DESIGN 16.1）。
- 台帳は 54 個で、いま出すのは 34 個（C1 で、E103、E105、E201、W201、E202 のほかの言語のファイル、E203、E301、E302、E303、W303、E306、E403、E404、E601 を出すようにした）。変異は 147 本と読み込み先 24 本、golden は変異の 294 本、台帳の例の 68 本、下ろした物の 37 本。
- 試験：`cargo xtask test --level fast -p ringi -p ritsu-ports -p ritsu-project -p ritsu -p rulec -p koyomi -p xtask -p ritsu-base` は 2,507 件が通る（ringi 40、ritsu 505、rulec 1,519、ritsu-base 221、koyomi 198、ritsu-ports 11、ritsu-project 7、xtask 6。rulec の測る試験一つは `ignore`。SKIP は 468 行で、どれも tools と platforms の段に回したもの）。`cargo xtask deps`（22 クレート）と `ritsu check ritsu.ctx`（15 コンテキスト）も通る。`ritsu check examples` は 31 ファイルを読み、落ちるのは `expense.req` の E011 の 9 件だけ（C4.3 で消える）。
- 見つけて直した不具合（どれも試験を足した）：率（`3%`）を 300% として下ろし、届く範囲も 3 倍で数えていた。計算するフィールドを読む所が、計算の届く範囲を見ずに書いた範囲だけで読み、無い穴を E301 にしていた。E105 の問いが、計算した値とその計算が読む値を別々の値として尋ね、起こりえない組み合わせを例にしていた。E105 の問いを rulec が決められないとき、何も言っていなかった（W201 にした）。経路を選ぶ表（`-> route`）を下ろしておらず、穴を確かめていなかった。ステップの名前が rulec の予約語（`final`）のとき、下ろした規則が通らず E201 になっていた。koyomi の予約語のコピーに、大文字で書き込んだ語が二つあった。
- 残したこと：
  - 合計でない計算の書いた範囲は、下ろした規則の入口が守らない（rulec の `define` に範囲を書けない。DESIGN 4.4）。C3 の参照インタプリタと D のつなぎのコードが守る（C3.1、D.2）。
  - 明細の行の数の上限は、下ろした規則が確かめない（DESIGN 4.3）。C3.1、D.2、E.3 で守る。
  - 経路の表が、使う規則の出力と、その出力を決めるフィールドを一緒に読むと、規則が出さない組み合わせにも行を求められる（DESIGN 6.4）。rulec の網羅の検査のふるまいで、ringi の側では消さない。
  - 期限の `terms.cal` の `started` は、カレンダーのデータの終わりより手前で切るので、データの短いカレンダーでは、その先に始まるステップの期限を確かめない。どの日の申請を確かめるかは、E107 の「使った日の範囲」と一緒に C2 で決める。
  - playground の `website/docs/playground/ritsu.wasm` は作り直していない（`.ringi` を playground の言語の一覧に入れるのは F.3）。
- ログ：`reports/ringi-C1-logs/`。

## 16. 段階 C2 で作ったものと確かめたこと（2026-11-14）

- 組織のデータ（`src/org/mod.rs`、`data.rs`）：組織のファイルの `data` の行が指すディレクトリから、版のディレクトリ（効き始める日の名前）ごとに `teams.csv`、`people.csv`、`members.csv`（か `organization.json`）を、直下から `proxies.csv` を、`ritsu_base::fs` を通して読む（DESIGN 8.2〜8.4、5.8）。誤りは E501（代理の相手が版にいなければ E505）で、データのファイルの行で一度だけ言う。版の中は、列、行、主の所属と親と長、輪と深さの順に確かめ、前の段に誤りがあれば次へ進まない。組織が通らなければ、それを読む申請の種類は E202 で待つ（`ritsu check` では待っている行）。`--org` で置き場を替え、`--at` で一つの版だけを確かめる。
- マスタ（`src/org/masters.rs`、DESIGN 4.6）：行の有効期間（`valid_from`、`valid_to`。どちらもその日を含む）、キーの重なり、型に合わない値を E501 で言う。マスタのファイルは `use organization` で組織を読め、人の列の行が、行の期間と重なる版にいない人を指せば E505。データが無ければ W500。
- 承認者の決め方の解き方（`src/org/resolve.rs`）と版ごとの一覧（`src/org/listing.rs`）：版ごと、申請の種類ごと、申請者ごと（代理申請の形も）に各ステップの承認者を決め、E502、W502、E503、E504、W506 を言う。データが無ければ W500。同じ版、同じステップ、同じ理由の申請者をまとめて数と 3 人までの名前で言い、二つの版で同じことは一つにまとめる。JSON の `witness` に、版、ステップ、申請者の全部を ID で入れる（DESIGN 8.5、8.6、16.2）。エージェントのステップは `else` の決め方まで解く。
- 使う日の範囲（`src/usable.rs`、DESIGN 9.6）：始まりと終わりを一か所で求め、E107、E404、下ろした `terms.cal` の `started` と `form.cal` の入力の範囲、一覧に入れる版が同じ範囲を使う。カレンダーの終わりは、下ろした `terms.cal` を koyomi に検査させ、E203 の `fails` が並べる日の続きから求める（見当の余白で切らない）。上へ回したあとの期限（`due_escalated`、二度回すなら `due_escalated_twice`）も数える。終わりは `check` の `ok` の行の下と JSON の `usable`、口の `Requests::usable` に事実として出し、`--through <日付>`（`ritsu check --through` も）を渡したときだけ E107 と E404 にする。
- コマンド（DESIGN 16.1）：`check` がディレクトリと `--org`、`--at`、`--through` を取り、`extract` も同じ形で取る。`simulate`（`--requester` か `--every-requester`、`--form`、`--on`、`--org`、`--format json`）は、その日の版と代理で、承認者と期限を見せる（DESIGN 10.3）。`extract` と `simulate` は、版ごとの一覧の検査をしない。
- 台帳は 54 個で、いま出すのは 43 個（C2 で E107、W500、E501〜E505、W502、W506 を出すようにし、E404 を書き直した）。変異は 165 本と読み込み先 40 本、golden は変異の 330 本、台帳の例の 86 本、下ろした物の 37 本、`examples/` の 2 本。
- `examples/`：人に `name_ja` の列を足し、マスタのファイルに `use organization` を足した。`tests/golden/lowered/expense/` を取り直した（`terms.cal` の `started` の範囲と `due_escalated`）。二つの版で検査した実物と、手で数えた案との違いは DESIGN 8.6 に書いた。契約の二本は E502 と E504 で落ちる（書いたままにしてある）。`ritsu check examples` は 31 ファイルを読み、落ちるのは契約の二本と `expense.req` の E011 の 9 件。
- 大きさ：3,000 人、300 チーム、版が二つの架空の組織で、リリースビルドの `check` と `simulate --every-requester` は、期限を数えない申請の種類で 0.05〜0.15 秒。期限を数える申請の種類には、koyomi が `terms.cal` を数える 0.15〜0.2 秒が足される（組織の大きさには依らない）。
- 試験：`cargo xtask test --level fast -p ringi -p ritsu-ports -p ritsu-project -p ritsu` は 581 件のうち 577 件が通った。落ちた 4 件は段階 B の `crates/ritsu/tests/ringi.rs` で、`examples/` の例に何も言わないことを求めていた。組織の警告と事実の行を求めるように直し、回し直して通った。クレートごとに ringi 57、ritsu 506、ritsu-ports 11、ritsu-project 7。SKIP は 81 行で、どれも tools と platforms の段に回したもの。`cargo xtask deps`（22 クレート）と `ritsu check ritsu.ctx`（15 コンテキスト、731 個）も通る。
- 見つけて直した不具合：診断を並べて重なりを除く所（`diag::sort`）が、同じ場所で見出しの同じ診断を、注を比べずに一つにしていた（英語の購買の `division_head` の W506 と、日本語の 2027 年の版の `部長` の W506 が消えていた）。注まで比べるようにした。
- 残したこと：
  - W506 は、ステップの条件を読まずに比べる（DESIGN 20.2）。
  - `simulate` は、二つ目より後のステップの期限を、日付でなく「始まってから何営業日目」で言う。始まる日は、前のステップが終わるまで決まらない。日付で見せるのは C3 の `run`。
  - `simulate --format json` の `due` は文で、日付と時刻のキーに分けていない。
  - 期限を数える申請の種類の検査に、koyomi が下ろした `terms.cal` の 146,097 日を数える 0.15 秒がかかる。
  - 代理の行の `requests` の名前が、どこかの申請の種類の名前かは確かめない。組織のデータは、どんな申請の種類があるかを知らない。
  - playground の `website/docs/playground/ritsu.wasm` は作り直していない（F.3）。
- ログ：`reports/ringi-C2-logs/`。

## 17. 段階 C2b で作ったものと確かめたこと（2026-11-14〜15）

- 承認者のいないステップが、いちばん上で上へ回せないとき（DESIGN 5.4）：`when empty escalate` のあとに、`then skip`（飛ばす）、`then stay`（止まって、管理する人が承認者を足すのを待つ）、`then <決め方>`（そのファイルの `who` の決め方を申請者について解いた人へ回す。`board = team board` と宣言して `then board`）を書けるようにした（構文木は `WhenEmpty::Escalate(Option<AtTop>)`）。書かなければ `then stay` と同じで、上へ回せない申請者がいれば E502。`then` の決め方にも人がいなければ E502 で、さらに上へは回さない。`then escalate`、`escalate` でない物のあとの `, then …`、`then` のあとの語でない物は E001。決め方の名前の `skip` と `stay` は E002、`who` に無い決め方は E004。`when empty` は下ろした物に出ないので、C3 の機械は構文木の `Deadline::Empty` から読む。
- 期限を過ぎて上へ回す先（DESIGN 5.10）：表が無ければ、期限を過ぎた人の上の最初の長（チームのステップなら、チームの上）。表（`when late escalate by <表>`）があれば、表が決めた決め方を申請者について解いた人で、だれにも着かなければ、止まった所から上へ回す。二度目は一度目に回した先から。期限を過ぎる人には、`when empty escalate` で回った先の長と、`then` の決め方の人も入る。回す先が無ければ、`escalate` のあとに書いたことを、その期限でする（`then expire` か書かなければ期限切れ、`then stay` なら待つ）。
- 書いたとおりに動いている所と、手を打つ所の分け方（DESIGN 5.4、5.10、8.6）：ほかへ回るか飛ばされるステップと、期限を過ぎて回す先の無いステップを、データに穴があるかで分けた。穴は、長の空いたチーム、人のいないチーム、版にいない人やチーム、上へたどる途中の空いた長の席である。穴があれば警告（W502、新しいコード W507）で、データを直すように言う。穴が無ければ、原因は組織の形（承認者に当たるのが申請者本人だけ、申請者がいちばん上、その種類のチームが上に無い）で、書いたとおりに動いているので警告にしない。`ok` の行の下に、版ごとに何人の申請がそうなるかを書き、`simulate --every-requester` を指す。一件ずつは JSON の `facts` の `cases` に入る（版、ステップ、決め方、何が起きるか、理由、注、行、申請者の全部）。どのステップにも答える人がいない申請者は、新しいコード W510（警告）で言う。その人のチケットは、出したとたんに、だれも答えないまま承認で終わるからである。
- 同じ承認者が続くステップ（DESIGN 5.12）：`ticket` の `same approver approves once`。すぐ前のステップをただ一人で承認した人が、次のステップにもただ一人の承認者として来るとき、その承認で済ませる。あいだに許すのは、来ないステップ、回覧、飛ばしたステップだけで、エージェント、指名、二人以上のチームと議決は済ませない。W506 は済む組を言わず、済まない組には理由を、書いていない申請の種類で書けば済む組には書き方を言う。`simulate` は済むステップを「done with the approval of …」と言い、JSON に `once_with` を持つ。
- だれが申請を出せるか（DESIGN 5.13）：`ticket` の `submitted by <グループ>, … [except <グループ>, …]`（`everyone`、`everyone in <チーム>`、`team <チーム>`、`position <役職> [or higher] [in <チーム>]`、`person "<ID>"`）。代理申請は代わられる人を見る。版ごとの一覧は、出せる人だけを申請者にし、`ok` の下に出せる人の数を書く。新しいコード E508（版にいないチームか人を書いた）と E509（その版ではだれも出せない）。`simulate --requester` は出せない人を断って 2 で終わり、JSON に `may_not_submit` を持つ。判定は `org::submit::Line::may` の一か所にある。
- 経路とフォームが読めない条件（DESIGN 4.5、5.3、5.11、6.4）：新しいコード E309。経路の条件、フォームの条件（`shown when`、`required when`）、`only when` の主張の条件が、下ろせない物（申請者、チケット、文のフィールド、二つの値の比べ、条件の中の計算、計算する真偽・選択・文のフィールド、マスタから選ぶフィールド、申請が渡さない共有の経路の入力）を読むと、その場所で言い、種類ごとの書き換え方を注に書く。経路の表の列の見出しも同じに確かめる（規則の別名だけ、無い出力などは E004）。検査を通ったファイルには、いつも下ろした経路がある。
- 選択の値（DESIGN 4.5）：規則の列挙から選ぶ選択（`choice from authority.kind`）の値は、`use rule` で読んだ規則の値で、自分の値の選択と同じに確かめる。条件、経路の表のセル、例のセル、シナリオのフォームのどれでも、無い値は E004（注に値を並べる）、引用符で書いた値は E009。
- 例の値（DESIGN 5.3）：例の行は出せる申請として、セルの値をフォームの宣言で確かめる。範囲の外の数と日付（計算するフィールドは、計算がとりうる値の外）、単位の刻みの倍数でない数、長さが `count` の外の文、マスタのどの行でもないキーは、経路がその列を読むかどうかにかかわらず、そのセルで E601。経路が読まない列は、下ろした例から外す。
- 別の単位で書いた数（DESIGN 6.4）：フィールドと同じ次元の別の単位で書いた数（`mass[kg]` の `range <=500_000g`、条件の `weight > 100_000g`、`round down(100銭)`）は、フィールドの単位に直して下ろす。範囲の端、条件と表のセル、例のセル、丸めの刻みのどれも同じ。
- 注と台帳の直し方（DESIGN 9.5、18）：書き方を見せる注と台帳の直し方は、そのとおりに書いたファイルが検査を通ることを、書き方ごとに試験で確かめる（`tests/fixes.rs`）。`then` のあとには `who` に宣言した決め方の名前を書くので、E309 と W510 の注は、宣言の仕方（`board = team board`）と一緒に言う。
- `simulate` の今日（DESIGN 10.3、16.2）：`--on` が無ければ、今日を申請が使うカレンダーの UTC オフセットで読む（期限の時刻と同じ読み方）。オフセットの違うカレンダーを使う申請は、`--on` を求めて 2 で終わる。カレンダーを使わない申請は UTC で読む。どう読んだかを頭の行に書き、JSON には `on_given` と `today_in` を書く。試験は、今の時刻を引数で渡して確かめる。
- 台帳は 59 個で、いま出すのは 48 個（C2b で E309、W507、E508、E509、W510 を足した）。変異は 201 本と読み込み先のファイル 54 本、golden は変異の 402 本、台帳の例の 96 本、下ろした物の 37 本、`examples/` の 2 本。
- `examples/`：社長の申請をだれが見るかを例ごとに決め、例の注に書いた。経費は全部のステップ、購買は決裁を `then board`（取締役会のだれかが答える）にし、経費の表 `next_up` に `| board | board |` を足した。アカウントと契約の上長、購買の一次、休暇の共有の経路は `then skip`。契約の `counsel` は `when empty escalate` にし、今の 2026 年の版では警告だけで通り、予約した 2027 年の版だけが E503 と E504 で落ちる。経費、休暇、アカウント、購買に `same approver approves once` を、休暇に `submitted by everyone except team board`（取締役会の人は休暇の申請を出さない、という架空の決まり）を書いた。
- `ritsu ringi check examples`（英日の合計）：C2 の終わりの E502 6、E504 2、W502 68、W506 46 が、E503 2、E504 2、W502 8、W506 22 になった。残る W502 は、どれも 2027 年の版で法務部の長が空く物である。組織の形の行は `ok` の下に 16 行（`--at 2026-11-16` では 20 行で、0 で終わる）。`ritsu check examples` は 31 ファイルを読み、落ちるのは契約の二本と `expense.req` の E011 の 9 件で、通る物の警告は 112 から 28 になった。
- 大きさ：3,000 人、300 チーム、版が二つの架空の組織で、リリースビルドの `check` と `simulate --every-requester` は、期限を数えない申請の種類で 0.06〜0.16 秒。期限を数える申請の種類には、koyomi が `terms.cal` を数える 0.18〜0.21 秒が足される（組織の大きさには依らない）。
- 試験：変異を 38 本足し、組織の形になった 2 本を消した。`tests/org.rs` に、いちばん上の三つの書き方、期限を過ぎて回す先（人、チーム、表、二度目、上へ回った先の長、`then` の決め方の人）、組織の形と穴の分け方と JSON の `cases`、W510、`same approver approves once` の済む組と済まない組、`submitted by` のグループと代理申請の試験を、`tests/lowered.rs` に E309 と表の共有の経路と別の単位の数の試験を、`tests/parse.rs` に新しい書き方の試験を、`tests/fixes.rs`（注と台帳が見せる 24 通りの書き方と、データの直し方）と `tests/examples.rs`（例の値を、経路が読む列と読まない列を入れ替えて確かめる）を足し、`tests/cli.rs` に `simulate` の今日の読み方の試験を足した。`cargo xtask test --level fast -p ringi -p ritsu-ports -p ritsu-project -p ritsu` は 606 件のうち 605 件が通った。落ちた 1 件は `crates/ritsu/tests/check.rs` の `ritsu_check_holds_the_requests_to_a_day` で、組織の形で飛ぶステップが警告でなくなったので、最後の行の警告の数の期待を直し、回し直して通った。クレートごとに ringi 77、ritsu 511、ritsu-ports 11、ritsu-project 7。SKIP は 98 行（xtask の数え方で 81）で、どれも tools と platforms の段に回したもの。`cargo xtask deps`（22 クレート）と `ritsu check ritsu.ctx`（15 コンテキスト、732 個）も通る。そのあとの直し（注の書き方、`simulate` の今日、`ok` の下の英語、例の値と選択の値、別の単位の数）のあとは、`cargo test -p ringi`（85 件）と、ritsu の `tests/ringi.rs`（5 件）、`tests/check.rs` の ringi の 2 件を回して通った。fast は回していない。
- 見つけて直した不具合（どれも前からあった。試験を足した）：
  - `when late expire, then stay` のような、使わない `, then …` を黙って読み捨てていた（E001）。
  - 経路の条件が下ろせない物を読むと、経路が黙って下ろされずに検査が通るか、ステップが来ない物として下ろされていた（E309）。同じ所で、表で書いた共有の経路を使う申請の経路が下ろされず、申請が渡さない共有の経路の入力を申請の同じ名前のフィールドとして読み、マスタから選ぶフィールドの値を名前で書くと ringi が落ち、フォームの条件と `only when` の条件は黙って読み捨てられ、値を先に書いた比べ（`50_000JPY < amount`）は経路が下ろされず、例の列に経路が読まないフィールドがあると例が全部下ろされず、共有の経路から下ろした行は共有の経路のファイルの行番号に結ばれていた。
  - `simulate --form` は、マスタから引く決め方でマスタの全部の行の人を並べ、マスタから選ぶ値がその日に選べる行かを確かめていなかった。
  - `ticket` の一度だけ書く行を二度書いても通っていた（E006）。
  - W507 は、ステップの決め方が人を見つけたときしか見ておらず、上へ回った先の長と `then` の決め方の人が期限を過ぎる所を言っていなかった。
  - 例のセルの値は型しか確かめていなかった。経路が読む列の範囲の外の値は rulec の言い方で行の頭に E601、刻みの外の値は E201 になり、経路が読まない列は何も言わなかった。
  - 規則の列挙から選ぶ選択の値を確かめていなかった。条件の `kind is bogus` は W303 だけで通り、経路の表と例のセルは rulec の E012 が E201 で行の頭に出るか、何も言われなかった。
  - 別の単位で書いた数を、数だけ取ってフィールドの単位で書いていた（`mass[kg]` の `range <=500_000g` が `<=500_000kg` に、`round down(100銭)` が `100JPY` に下ろされた）。
  - E309 の注は、`then` のあとに決め方の宣言（`team board`）をじかに書く形を見せていた。そのまま書くと E001 になる。W510 の注は、`who` に宣言しなければ E004 になる `then board` を、宣言の仕方を言わずに見せていた。
  - `simulate` は `--on` が無いと今日を UTC で読み、日本時間の 0 時から 9 時のあいだは前の日で答えていた。
  - `ok` の下の行と W502 の英語は、飛ばされるステップも数えているのに「go elsewhere」とだけ言っていた。
- 残したこと：
  - あいだに別の人のステップがある二つのステップは、この段階では一度にしていない（`examples/` に残った W506 の 22 は、どれもこの形だった）。段階 C5 で、`same approver approves once, at the last stage` を作った（20 章）。
  - 書き換え方の無い読めない条件（日付どうし、選択どうし、真偽どうしの比べ、人やマスタから選ぶフィールド、規則の出力との比べ）には、E309 の注に書き換え方を書いていない。
  - 表が同じ人へ回す行（`next_up` の `division_head` から `division_head`）は言わない（DESIGN 5.10）。
  - E502 の文は、組織の形と穴を分けていない。空いた長の席のためにいちばん上まで行って止まるときも「nobody is above (the top of the organization)」と言う。
  - W506 は、ステップの条件を読まずに比べる（DESIGN 20.2）。
  - 期限を数える申請の種類の検査に、koyomi が `terms.cal` を数える 0.2 秒ほどがかかる（C4）。
  - playground の `website/docs/playground/ritsu.wasm` は作り直していない（F.3）。
  - 人とチームのフィールドの値を、シナリオのフォームに書く形が無い（`reviewer = "e7001"` は E009）。C3 で `person "e7001"`、`team legal` と書く形にした（18 章）。
  - 例の日付のセルが、`open in` のカレンダーの開いている日かは確かめていない（開いている日はカレンダーのデータを読んで決まる）。
- ログ：`reports/ringi-C2b-logs/`。

## 18. 段階 C3 で作ったものと確かめたこと（2026-11-15）

- チケットの機械（`src/engine/`、DESIGN 7 章、10.1）：`Machine::start(submit, organization, now)` と `Machine::next(ticket, event, organization, now)`。時刻、組織の部分、フォームの値を引数で受け、ほかに何も読まない。答えは、次のチケットとすることの一覧か、断り（十の種類と文）である。機械は検査の答えから一度作り、表と条件と計算は `Rules::eval`、期限は `Dates::eval`、営業日は `Dates::calendar` の休みで数える。承認者、承認者がいないときの回り方、期限を過ぎて回す先、`same approver approves once` で済むステップ、出せる人は、検査の一覧と `simulate` と同じ関数（`org::resolve`、`org::listing`、`org::submit`）で決める。フォームの値の受け取りは `src/taken.rs` の一か所にまとめ、`simulate --form`、シナリオ、機械が同じ物を呼ぶ。
- 機械の決まりのうち、書きながら決めたもの（どれも DESIGN に書いた）：
  - 却下は、議決に届かなくなったときにチケットを終える。届きうるあいだは、答えを足して待つ（7.3）。
  - 鳴った時計のイベントが渡されるまで、ほかのイベントを `clock` で断る（7.6）。これが無いと、期限を過ぎたステップへ承認の取り消しで戻して、同じ秒に承認し直せ、取り消しと承認のくり返しでチケットをいくらでも延ばせた。
  - 上へ回したあとの期限と催促は、元の期限が切れた時刻から、ステップの始まりから数えるのと同じ営業日数で数える（5.10、7.5）。サーバーが期限のイベントを遅れて渡しても延びない。二度目に回したあとも、回した先での保留と承認の取り消しのあとも同じである。
  - 催促は、期限より前に来るときだけ置く（5.10）。ステップが始まるときの催促も、上へ回したあとの催促も同じである。
  - 差し戻しと取り下げの回数を使い切ったチケットと、`send back` の行が無い申請の種類のチケットは、取り下げると `withdrawn` で終わる（7.3）。
  - 保留中のチケットも flow の答えを受け付け、その答えでステップが済めば、保留も終わる（7.2）。
  - 申請者が組織からいなくなったチケットは、申請者から決める決め方がだれも見つけなければ、`when empty` に何と書いてあっても止まる。いなくなった人は再申請も破棄もできない（7.7）。前は、そういうチケットが、どのステップも飛ばして、だれも答えないまま承認で終わった。
  - JSON の名前は括弧の中の名前にする（3.4）。英語の名前の申請と日本語の名前の申請が、同じベクタになる。
  - 仮押さえの引数を作れない申請（部の無い人の `dept: requester.department`）は、`submit` を `book` の種類で断る。
- フォーム：計算するフィールドに書いた範囲を、rulec の `define … range` に下ろすようにした（DESIGN 4.4。合計は前から `sum … range`）。入口の断りを、機械と `simulate --form` が、フィールドの名前と書いた範囲の文に直す。明細の行の数は機械が確かめる。人とチームのフィールドの値は `person "e7001"`、`team legal` と書く（文で書けば E009 で、書き方を言う）。添付の値は、ファイルの名前の文で書く。
- シナリオと `run`（`src/scenario.rs`、DESIGN 10.2）：`ritsu ringi run <file> [--scenario <名前>] [--org <dir>] [--format json]`。行ごとに、その時刻の組織の部分を切って機械へ渡し、帳簿の分を `Books::open` の帳簿に流す。日付と帳簿の答えは、シナリオが持たずに `run` が計算する。行と違うステップか状態は E602、機械か帳簿が断る行は E603、時計の行の時刻の違いと、時計が鳴ったあとの行は E604。`check` もシナリオを流す（`--at` のときを除く）。シナリオの無いファイルの `run` は、標準エラーに一行言って 2 で終わる（`ritsu ringi run` を通しても同じ）。必須のフィールドを機械が確かめるので、経費と購買のシナリオのフォームを直し、休暇に催促の行を、アカウントと契約にシナリオを足した（日本語の名前の版も）。
- 終わることと仮押さえ（`src/ends.rs`、DESIGN 7.6）：いちばん長い営業日を数え、`ok` の下に書く。五つの例は、経費 93、購買 246、アカウント 55、契約 192、休暇 40 で、手で数えて例の主張に書いた数と同じだった。数は上限で、主張が数以上なら必ず成り立つ。終わりの無い待ち（期限を過ぎても待つステップ、期限の無いステップ、`resubmit within` の無い申請者の番）は、主張があれば E401、無ければ `stay` だけ W402 で、どれも `ok` の下に書く。仮押さえは E405、E406、E407。仮押さえの条件（`when`）か引数が、承認者の直せるフィールド（`approvers edit`）を、そのまま、または計算するフィールドか規則の出力を通して読む形も E405 にした（DESIGN 7.6）。仮押さえは申請を出したときの値で一度だけ行うので、額を直せば確定される額が申請と食い違い、条件が成り立たなくなれば開いたまま終わる。五つの例と機械のための申請に、この形は無い。
- 使う日の範囲（`src/usable.rs`、DESIGN 9.6）：終わりの日を、保留を使い切ったステップの時計と、申請者へ戻った日からの再申請の期限まで数えられる日にした。機械は、その日を過ぎて始まる申請と再申請とステップを、足りないデータを言って断る。期限を過ぎて上へ回すことは断らない。
- 検査の直し（機械と突き合わせて見つけた）：W510 は、条件で来るステップにだけ人がいる申請者にも言う（DESIGN 5.4）。期限を過ぎて回す先の無いステップの二度目は、一度目に回った先の人から求める。
- 帳簿の確定と取消（`on approved post`、`on rejected void` など）は、そのチケットのその `round` の仮押さえが開いているときだけ、条件と引数を読む。仮押さえが無ければ、何も尋ねずに飛ばす（DESIGN 7.4）。
- ベクタ（`src/vectors.rs`、DESIGN 10.4）：`ritsu ringi vectors <file> [--list] [--org <dir>]`。一行が一つの遷移の JSON Lines で、どの行も参照インタプリタの遷移そのものである。五つの例で、経費 112 本、購買 126 本、アカウント 93 本、契約 163 本、休暇 88 本。経費の例の行のうち、出張の二つは書かれない（シナリオのフォームに出張先が無い）。
- 台帳は 59 個で、いま出すのは 56 個（C3 で E401、W402、E405〜E407、E602〜E604 を出すようにした）。変異は 245 本と読み込み先のファイル 57 本、golden は変異の 490 本、台帳の例の 112 本、下ろした物の 37 本、`run` の 28 本、ベクタの 19 本、`examples/` の 2 本。
- 試験：`tests/machine.rs`（73 件）と `tests/vectors.rs`（17 件）を足した。遷移の表（DESIGN 7.3）の 23 行は、どの行にも、その行の前の状態、次の状態、することの一覧を見る試験がある。機械のための申請は `tests/machine/` の 10 本（`desk` は遷移の表の行ごとにシナリオを持つ。`desk.ja` はその日本語の名前の版。`reminds` は、上へ回すたびの催促を、`every stage` の催促、自分の催促、自分の回数、自分の期限、期限より前に来ない催促のステップで通る）。機械と `simulate --every-requester` の突き合わせは、機械の申請 7 本と例 5 本の、二つの版のどの人についても、食い違いが無い。`simulate` が最初のステップに添える催促は、機械が置く催促と同じ日時である。机の申請の歩き方のうち、いちばん長い物は 73 営業日で、検査の数えた 74 営業日を越えない。
- 実装を変える 18 通りの変異は、どれも試験が落とす：却下をすぐ終わりにする、取り下げで終わらせる、議決の数を一つ減らす、一つ増やす、差し戻しの上限を一つ増やす、期限を走り始めの時刻で数える、上へ回したあとに期限を数え直さない、上へ回したあとの催促を置かない、その催促を数え直さない、期限より前に来ない催促を置く、自分の期限か催促を持つステップに `every stage` の催促を読ませる、シナリオの使わない行を通す、時計が鳴ったあとの行を通す、行と違うステップを通す、行と違う状態を通す、範囲の終わりを過ぎて上へ回すのを断る、保留を範囲の終わりに入れない、再申請の期限を一日多く数える。
- 残したこと：
  - いちばん長い日数は上限で、ちょうどその日数かかるチケットがあるとは限らない（DESIGN 7.6、20.2）。
  - 仮押さえの引数を作れない申請者（経費と購買の、部の無い 7 人）を、検査は言わない。機械が `submit` を断る（DESIGN 20.2。X19 で言う）。
  - 承認者の直しで仮押さえを押さえ直すことはしない。仮押さえが読むフィールドを承認者が直せる形は E405 にする（DESIGN 7.6、20.2）。
  - 催促と期限を別のカレンダーで数えるステップには、上へ回したあとの催促を置かない（DESIGN 5.10、20.2）。そういうステップでは、主張 `every stage reminds before it is due` も確かめられない（下ろした主張は、二つの日付が同じファイルにあるときだけ書く）。
  - `check` と `simulate` の JSON は、ステップと決め方を書いたとおりの名前で書く（DESIGN 3.4、20.2）。
  - 断りの文と履歴の `note` を、生成するコードが同じに作るかは、D で決める。
  - 経費の出張の例の行は、出張のシナリオが無いので、ベクタにならない。
  - `run` と `simulate` は `--root` を取らず、JSON の `file` は渡したとおりのパスになる。
  - 上へ回したあとの期限の間際に保留を始めると、保留の終わりを数えられない日がありうる（範囲の最後の日に始まり、一度上へ回ったステップ）。そのときは `pause` を `data` で断る。
  - 範囲の終わりを過ぎると、要約のステップの期限のイベントは、次のステップを始められないので `data` で断られ、データを延ばすまでチケットは進まない。
  - playground の `website/docs/playground/ritsu.wasm` は作り直していない（F.3）。
- 段階の終わりの試験（2026-11-15）：`cargo xtask test --level fast -p ringi -p ritsu-ports -p ritsu-project -p ritsu` は、`test result` の行が 61 本、695 件が通り、1 件が落ちた。落ちたのは `crates/ritsu/tests/ringi.rs` の `ritsu_ringi_checks_a_request_with_every_language_joined` で、`ok` の下に足した「長くても何営業日で終わるか」の行を、試験の期待が持っていなかった。期待に行を足し、その試験のバイナリ（5 件）と ringi のクレートの全部（169 件）を回し直して、どれも通った。SKIP の行は 98 で、どれも fast では tools と platforms の段を回さないためである（前の段階と同じ行）。`cargo xtask deps` は 22 クレートで ok、`ritsu check ritsu.ctx` は ok（747 の成果物、82 の越え）。
- 上へ回した先の催促、E405 の新しい形、`run` の終了コードを足したあとの試験（2026-11-15）：`cargo test -p ringi` は 175 件、`cargo test -p ritsu --test ringi` は 6 件で、どれも通った。例のシナリオとベクタの golden で変わったのは、下ろした `terms.cal` に足された `remind_escalated` と、上へ回したあとの `set timer remind`、だれも答えないときの時計の並びに入った `remind` の行だけである。いちばん長い日数と、使える最後の日は変わらない。
- ログ：`reports/ringi-C3-logs/`。

## 19. 段階 C4 で作ったものと確かめたこと（2026-11-15）

- 口（DESIGN 9.4）：`Gates::allowed_with`（属性の一部を値に決めて尋ねる。sekisho が `.gate` に答える）、`Flows::declared`（flow の入力と出力の型と範囲。dandori が `dandori api` と同じ一覧から答える）、`Requests::ticket`（チケットの事実。ringi の `src/ticket.rs` が、検査の答えと `src/ends.rs` の数から作る）。どれも既定の答えは「決められない」で、答えない言語のふるまいは変わらない。
- 言語をまたぐ検査（DESIGN 9.3）：X18（gate と経路。E911、W911）、X19（帳簿とチケット。E209、W209）、X20（flow とチケット。E210、W210）を `crates/ritsu-cross` の `ticket_gates.rs`、`ticket_holds.rs`、`ticket_flows.rs` に作った。JSON の記録の検査の名前は `ticket_gate`、`ticket_hold`、`ticket_flow`。六つのコードは、小さい架空の再現と一緒に ritsu の台帳に入り（43 個になった）、`ritsu explain` と `docs/codes.md`、`docs/codes.ja.md`、ページの語の一覧、ritsu のスキルの表に載る。エラーは `ritsu check` と `ritsu gen` を今の検査と同じに止める。文は、申請の渡し方（名前、絶対パス、ディレクトリ）で変わらず、相手が根の外にあっても同じに確かめる。
- X19 は、仮押さえの有効期限のほかに、仮押さえの引数を作れない申請者（`dept: requester.department` と、上に部の無い人）を W209 で言う。版ごとに、出せる何人のうち何人かと、3 人までの名前を言い、`simulate --every-requester` が、その人の行の頭に `submit` が断られることを書く（JSON は `holds_refused`）。行に `when requester.<種類> is not none` があれば、その申請者には仮押さえを作らないので、言わない。
- X20 は、渡す値が flow に渡る所で狭まる分を読む。エージェントは経路の表の行で、`on` の行とステップの `when` は、フォームのフィールドを定数と比べる条件を `and` でつないだ形で追う。追わない形（`or`、`not`）は、合わないように見えるときだけ W210 にする。
- ツール名と中の物（DESIGN 9.7）：`Tool::ALL` を 16 個にし、`Tool::kinds` に ringi の 13 種類を書いた。ringi が `Items` に答え、yuen は `ringi "<ファイル>" <種類> <名前>` で中の物を指せる（`verified by` は、主張、例、シナリオと、ファイルの全体）。`examples/shared/expense.req` は、名前の無い `who` の行を、決め方を一つずつ指す行に直し、残るのは E301（21 件）だけになった。
- `ritsu ringi api <file>`（DESIGN 16.1、16.2）：ファイルが宣言することを JSON で書く。`items` は口の `Items` と、`ticket` は口の `Requests::ticket` と、`usable` は `check` の JSON と同じ物である。flow に渡す値（`given`）は、運ぶ型と範囲のほかに、値が無いことがあるか（`none`）と、ringi が追わない条件でさらに狭いかもしれないこと（`may_be_less`）を言う。検査を通らないファイルには JSON を書かずに 1 で終わる。
- JSON の名前（DESIGN 3.4、16.2）：`check`、`simulate`、`api` の JSON も、名前を括弧の中の名前で書き、書いたとおりの名前を `written`（か `<キー>_written`）で添える。`simulate` の `computed` は、名前、書いた名前、ワイヤの値、書いたとおりの値の並びにした。
- 催促と期限を別のカレンダーで数えるステップ（DESIGN 5.10、`src/apart.rs`）：主張 `every stage reminds before it is due` を、ステップが始まりうる日の一日ずつについて ringi が確かめる（E403）。上へ回したあとの催促は、機械が催促のカレンダーの営業日で数えて置く。その催促を数えられる最後の日は、使える最後の日の一つになり、決めているのが催促であることを文と種類（`reminder`）で言う。
- 下ろした先の診断の直し（DESIGN 9.2）：計算するフィールドの範囲に、丸めや行の数まで数えると値が一つも入らないとき、rulec の E150 を ringi の E103 に直す。`use` に書いたディスクの根からのパスは、どの種類でも、`use` の行の E005 にする（前は、下ろした `terms.cal` が koyomi の E015 になり、E201 で落ちた）。
- `examples/`：経費と購買は、仮押さえの行に `when requester.department is not none` を書き、部の上の人（社長、本部長、取締役）の申請は部の予算を押さえずに進む形にした（負担する部をフォームで選ぶ形は、どの申請者にも部を選ばせることになる。組織のデータで部の上の人に部を持たせる形は、その人の申請の `dept_head` のステップの承認者まで変える。どちらも取らなかった）。予算の帳簿の仮押さえを 90 日から 365 日にした（購買は、前のステップへの差し戻しで、仮押さえが 216 営業日、312 日開いていることがある）。経費に、出張のシナリオを二つ足した（エージェントが決める日帰りの出張と、エージェントが人へ渡す出張）。経費の例の五つの行は、どれもベクタになる（経費 141 本）。
- `ritsu check examples`：31 ファイルで、境目の 76（X18 が 24、X19 が 6、X20 が 46）がどれも成り立つ。落ちるのは、契約の二本（2027 年の版の E503 と E504）と、`expense.req` の E301 の 21 件である。
- 台帳は 59 個のままで、いま出すのは 56 個。変異は 253 本と読み込み先のファイル 59 本、golden は変異の 506 本、台帳の例の 112 本、下ろした物の 37 本、`run` の 33 本、ベクタの 19 本、中の物の 13 本、`api` の 3 本、`examples/` の 2 本。機械のための申請は 13 本（`held` と、その日本語の名前の版、`apart` を足した）。言語をまたぐ検査の golden は、X18〜X20 の 40 の形の英語と日本語で 80 本。
- 期限を数える申請の種類の検査で、koyomi が下ろした日付を数える回数を測った。下ろした日付のファイル一つにつき一度で、英語と日本語の文と事実のために数え直すことは無い（`tests/lowered.rs` の `koyomi_checks_the_lowered_dates_once`）。残るのは、申請の種類ごとの 146,097 日の一度である（DESIGN 20.2）。
- 実装を変える 18 通りの変異（gate の答えを取り違える、E209 を W209 にする、開いている日数を一日減らす、値の無い申請者を言わない、範囲の端を逆に比べる、口の答えを変える、など）は、どれも試験が落とす。
- 見つけて直した不具合（どれも試験を足した）：
  - 根の外にある gate を、X18 が黙って飛ばしていた（申請をディレクトリで渡すと E911、名前で渡すと何も言わなかった）。
  - 予算の帳簿の仮押さえ（90 日）が、購買のいちばん長い場合より先に切れた（DESIGN の見込みは、前のステップへの差し戻しを数えていなかった）。
  - 経費と購買の例で、部の上の 7 人の `submit` が、仮押さえを作れずに断られていた。
  - 催促と期限を別のカレンダーで数えるステップで、主張 `every stage reminds before it is due` が、確かめられないまま通っていた。上へ回したあとの催促も置かれなかった。
  - カレンダーのデータの終わりが催促だけを止めるときも、使える最後の日の文が「期限を数えられない」と言っていた。
- 残したこと（DESIGN 20.2 にも書いた）：
  - flow がチケットを開く側の X20（DESIGN 15.4）は、段階 F で足す（F.4）。
  - X19 の日数は、検査が数えた営業日の上限をカレンダーに置いたもので、ちょうどその日数かかるチケットがあるとは限らない。
  - 仮押さえの行の `when` が `requester.<種類> is not none` のほかの条件のとき、X19 の人数は、その条件を読まない。
  - `or` と `not` のある `when` は、X20 が追わない。
  - `api` の単位の綴り（`money[円]`）は書いたとおりで、括弧の中の名前にあたる物が無い。
  - `simulate` の `today_in` のカレンダーは、共有の経路と申請が同じファイルを別の別名で読むと、別名ごとに並ぶ。
  - playground の `website/docs/playground/ritsu.wasm` は作り直していない（F.3）。
- 段階の終わりの試験（2026-11-15）：`cargo xtask test --level fast -p ringi -p ritsu-ports -p ritsu-project -p ritsu -p ritsu-cross -p sekisho -p dandori -p ritsu-base -p yuen -p sakai` は、`test result` の行が 229 本、1,840 件が通り、落ちた物は無い（dandori 179、ringi 184、ritsu 523、ritsu-base 221、ritsu-cross 42、ritsu-ports 11、ritsu-project 7、sakai 245、sekisho 168、yuen 260）。SKIP の行は 190 で（xtask の数えでは 173）、どれも fast では tools と platforms の段を回さないためである。そのあと `api` の `given` に `none` と `may_be_less` を足し、`cargo test -p ringi --test items --test cli`（6 件と 17 件）と `cargo test -p ritsu --test ringi`（8 件）を回し直して、どれも通った。`cargo xtask deps` は 22 クレートで ok、`ritsu check ritsu.ctx` は ok（768 の成果物、82 の越え）。

## 20. 段階 C5 で作ったものと確かめたこと（2026-11-15）

- 書き方（DESIGN 3.2、3.3、3.6、5.12）：`ticket` の `same approver approves once, at the last stage`。今の行に `, at the last stage` を続ける。同じ人が一人で答えるステップが二つ以上あるとき、その最後のステップで一度だけ答えてもらい、前のステップは、その承認を待つ。すぐ前後の二つのステップも同じに読む。今の行（すぐ前の承認で次を済ませる）のふるまいは変えていない。足した語は `last` で、kaname の組み込み（`xs.last()`）と同じ意味で使う。二つの書き方を両方書くと E006、コンマを忘れると E001。
- 機械（`src/engine/stages.rs`、`act.rs`、`state.rs`。DESIGN 7.1、7.3、7.4）：ステップが始まるときに、部分ごとに、一人に決まる承認者があとのステップにも一人で答えるかを、そのときのフォームと版で解いて決める。待つ部分とステップの段階は `deferred`。待つ先のステップの答えがそろったときに、待たせた人の席の承認（本人か代理）があれば、その承認で済む。無ければ（期限を過ぎて回した先の人、引き継いだ人、足した人が承認した）、待っていたステップをそのあとに始める。待つ先が、始まる前にその人に一人では当たらなくなっていれば（組織の版が替わった、承認者の直しで経路が変わった）、待っていたステップをそのときの版で決め直し、あとのステップに一人で答えるなら待たせ直し、そうでなければ先に始める。あとで始めるステップは、並びに同じ名前のステップをもう一度足す。履歴の行は `deferred`、`deferral ended`、`approved`（`once_with` に答えたステップ）。`Machine::start` と `Machine::next` の引数と答えの形は変えていない。
- 差し戻し、取り消し、取り下げ、保留、代理（DESIGN 5.12）：待っているステップへ差し戻せば、そこで答えてもらう。あいだのステップへの差し戻しと、先のステップの承認の取り消しでは、その承認で済んだステップが、また待つ。`withdraw until <ステップ>` は、そのステップが待っているあいだは閉じない。代理の承認は、代わられた人の席の承認として、その人の待っていたステップを済ませる。代わりに答えた人自身のステップは、その人が自分の席で答えたときに済む。
- 検査（`src/org/listing.rs`。DESIGN 8.6）：W506 は、新しい書き方の申請の種類で、前が後を待つ二つのステップを言わない（どちらかがエージェントの先に答えるステップのときだけ言う）。今の行の申請の種類で済まないものと、書いていない申請の種類には、書けば一度になる書き方を注に添える。新しいコードは無く、台帳は 59 個のまま。W506 の台帳の文を、二つの書き方で書き直した。
- `simulate`（`src/simulate.rs`。DESIGN 10.3、16.2）：待つステップに、どのステップで一度だけ答えるかを言う（必ず待つ、来れば待つ、待つ先になりうるステップが二つ以上、の三つの言い方）。JSON は、決め方（`ways`）の `once_with` に待つ先を、`once_if_it_comes` に来れば待つのかを、`once_among`（と `once_among_written`）に待つ先になりうるステップを書く。`ways` のキーは、`ticket` に何を書いた申請の種類でも同じで、あとのステップを待つことの無い申請の種類では、あとの三つは false と空のリストになる。
- いちばん長い日数（`src/ends.rs`。DESIGN 7.6）：新しい書き方の申請の種類で、決め方が二つ以上のステップを、決め方の数だけ数える（待たせた部分をあとで始めると、時計がもう一度始まる）。仮押さえが開いている日数（X19 が読む数）も同じ S から数える。決め方が一つのステップの数は変わらない。
- `examples/`：購買とアカウントを新しい書き方にし、経費と休暇は今の行のまま、契約は書かないままにした（理由は DESIGN 5.12 の表と、例の注）。購買にシナリオを三つ（待たせて一度で答える、決裁が期限を過ぎて回り待っていたステップがあとで始まる、待っているあいだに組織の版が替わる）、アカウントに一つ足した。`examples/` の W506 は、英語と日本語で 22 から 10 になった（経費の 8 と契約の 2）。主張 `ends within` の数は変わらない。ベクタは、購買 252 本、アカウント 146 本。
- `ritsu check examples`：31 ファイルで、境目の 76 がどれも成り立つ。落ちるのは、契約の二本（2027 年の版の E503 と E504）と、`expense.req` の E301 の 21 件である。
- 前と後：二つに分けて比べた。
  - 書き方、機械、検査の分：木の `.ringi` の 310 本について、`check`（英語、日本語、JSON）、`simulate`、`run`、`vectors`、`api` の出力 4,281 個を、この段の前のバイナリと比べた。違うのは、新しい書き方にした購買とアカウント、この段で作った機械のための申請、W506 の注の行（62 行。どれも、もう一つの書き方を添える注）だけである。
  - `simulate` の JSON のキーの分：五つの例（英語と日本語）と機械のための申請の 29 本について、同じコマンドの出力 681 個を、キーをそろえる前のバイナリと比べた。違うのは `simulate` の JSON の 121 個（19 本のファイル）だけで、消えた行は無く、足された 11,469 行は、どれも三つのキーの行である（3,823 個の決め方に、`"once_if_it_comes": false`、`"once_among": []`、`"once_among_written": []`）。新しい書き方の購買、アカウント、機械のための申請の三本は、どの出力も変わらない。
  - 合わせると、今の行だけを書いたファイルと、どちらも書いていないファイルの出力は、W506 の注の行と、この三つのキーのほかは一バイトも変わらない。
- 機械のための申請は 16 本（最後のステップで一度だけ答える `last` と、その日本語の名前の版、ステップの条件の形で待つ `waits` を足した）。golden は、`run` が 38 本、ベクタが 21 本。変異は 253 本のまま（W506 の二本の golden に、注の行が増えた）。
- 実装を変える 29 通りの変異（待たせない、待たせたまま答えを得ずに済ませる、相手が変わったのに始めない、期限を過ぎて回した先の答えで待っていたステップまで済ませる、今の行のふるまいを新しいほうに替える、取り消しても済んだままにする、最後でなく最初の先を待つ、日数を一度だけ数える、`simulate` の JSON の決め方からキーを一つ外す、など）は、どれも試験が落とす。
- 見つけて直したこと：
  - `crates/ritsu/tests/ringi.rs` の `ritsu ringi run` の試験が、アカウントの例の出力の終わりを一つ目のシナリオの行と決めていた。シナリオを足したので、二つの行を確かめる形にした。
  - DESIGN 8.6 の `expense.req` の数が、C4 の前のまま（E011 の 9 件）だった。E301 の 21 件に直した。
- 残したこと：
  - 期限を過ぎて回したステップを止めて、それを待っていたステップを先に始める形は作っていない。回したステップの答えがそろってから始める（DESIGN 20.1）。
  - 生成するコード（D）。チケットの `stages` に同じ名前のステップが二つ並ぶことがあるので、生成する機械は、今のステップを並びの位置で持つ。
- 段階の終わりの試験（2026-11-15）：`cargo xtask test --level fast -p ringi` は、`test result` の行が 15 本、200 件が通り、落ちた物も SKIP の行も無い。`cargo xtask test --level fast -p ritsu` は、`test result` の行が 41 本、523 件が通り、落ちた物は無い。SKIP の行は 98 で（xtask の数えでは 81）、どれも fast では tools と platforms の段を回さないためである。そのあと `simulate` の JSON の決め方のキーをそろえ、`tests/org.rs` に試験を一つ足して、`cargo test -p ringi --test org --test cli`（20 件と 17 件）と、`--test machine` の `the_machine_and_simulate_on_held` を回し、どれも通った。golden（732 個）は、`simulate` の JSON を持たないので、一つも変わらない。`cargo xtask deps` は 22 クレートで ok、`ritsu check ritsu.ctx` は ok（768 の成果物、82 の越え）。

## 21. 段階 D で作ったものと確かめたこと（2026-11-15）

- 先に直した例のデータ：`examples/org/proxies.csv` の代理の行の `requests` に、日本語の名前の版の申請の種類（`expense_ja`、`purchase_ja`、`leave_ja`）を足した。経費と休暇の日本語の版のベクタが、英語の版と同じ本数になった。変わったのは日本語の名前の版の出力だけで、英語の版は 1 バイトも変わらない。
- 機械の本体（D.1。DESIGN 7 章、12.1）：kaname のモジュール `ticket`（`src/lower/ticket.kn`）。228 関数で、total、警告は無い。型は参照インタプリタの型を写した。D.1 の「段階 A の機械の下書きを元に」とは違えた。下書きの型は、部分、席、`sole`、`deferred`、時計の時刻、仮押さえを持てないからである。DESIGN 7.3 の遷移の全部、5.4 の `when empty`、5.12 の二つの書き方、上へ回したあとの期限と催促、保留のあとの時計のずらし方を書いた。日の番号から日付を作る所は、機械と一緒に置く koyomi の日付（`src/lower/days.cal`。整数の入力を取る）が受け持つ。total に書くために置いた数の上限は、DESIGN 12.1 に書いた。
- 見た版（DESIGN 7.1、7.2、12.4）：チケットの版 `revision`（`round`、`stage`、`edits`）と断りの種類 `stale` を、参照インタプリタ、シナリオ、ベクタ、機械に足した。時計のほかのイベントは版を持ち、`next` は、鳴った時計、版、終わったチケットの順に確かめる。ベクタは、申請の種類ごとに、前の版を持たせた時計のほかのイベントを 15 本と、版を持たない承認を 1 本足した（`src/vectors.rs`）。承認者の直しのあとは、まだ答えていないエージェントに尋ね直す。直す前に頼んだ答えは `stale` で断られるからである。
- 約束の形（DESIGN 10.4、12.4）：ベクタ、`ritsu ringi run --format json`、生成したコードの入口は、`src/lower/shape.rs` が書く一つの JSON の形を使う。参照インタプリタの形は Rust の型の中だけにある。約束の版 `CONTRACT`（いまは 1）も `shape.rs` に置いた。
- つなぎ（D.2。DESIGN 4.4、6.4、12.1）：`src/lower/glue/{mod,form,on}.rs` が、検査を通った申請の機械から `<ファイル>.lowered/<名前>.kn` を書く。フォームの型と条件、明細の行、計算するフィールド、経路の表、決め方の全部、エージェントと要約のステップ、ステップ自身の期限、催促を別のカレンダーで数えるステップ、`submitted by`、使う日の範囲、`on` の行（条件、帳簿と flow の引数）を下ろす。営業日は、つなぎの横に置く koyomi の日付（`open.cal`、`open_<カレンダー>.cal`）が数える。例と機械の試験の申請の全部で、つなぎを書ける。`start` は、参照インタプリタと同じ `start(submit, organization, now)` にした（DESIGN 12.2）。
  - 計算するフィールド（DESIGN 4.4、13.7）：生成するコードは、範囲を届く範囲だけにした写し `values.rule` を読み、つなぎが書いた範囲と `if` で比べて、参照インタプリタと同じ `form` の断りにする。写しが int64 に収まると示せない申請は、`ritsu ringi check` の E201 にする（台帳の変異を 4 本足した）。
  - 置いたコード（つなぎと機械）に kaname が何か言えば、警告でも、total でない関数でも、`ritsu gen` が申請の最初の行の E201 で止まる（DESIGN 9.2）。注には kaname のコードと行だけを書く。
- `ritsu gen`（D.3。DESIGN 12.2、12.3）：
  - 言語のパッケージに、下ろした規則と日付、つなぎ、機械と、申請の種類の一覧（`ticket_kinds.ts`、`<パッケージ>/ticket_kinds.py`、`ticketkinds/ticketkinds.go`、`ticket_kinds.rs`）を入れる。一覧は、種類ごとの ASCII の名前、版、`start` と `next` の JSON の入口と、約束の版を持つ。TypeScript、Python、Go の入口は kaname の試験の道具と同じ読み替えで書き、Rust はパッケージの入口（kaname の `glue::door::answer`）を通す。
  - `--target requests`（`src/gen/`）：`types/<名前>.json`、`openapi.json`、`mcp/tools.json`、`organization/schema.json`、`organization/columns.md`、`vectors/<名前>.jsonl` を `<out>/requests/` に一度だけ書く。`a2ui/` は段階 E で足す。`ritsu.mod` の `target` の行にも書ける。
  - 組織の版ごとの一覧の誤り（E502〜E504 と、同じ一覧のほかのコード）では止まらず、申請ごとに一行の警告で言う。シナリオの誤り（E602〜E604）では今のまま止まる。
  - 申請がプロジェクトに無いファイルを読むときは、その申請とファイルを言って止まる（購買は `examples/shared/authority.rule` を一緒に渡す）。
  - `.ringi` の無いプロジェクトの生成物は変わらない（8 章の三つのプロジェクトの 448 ファイルと、kaname の試験のモジュールの 284 ファイルを、C5 の終わりのバイナリと比べた）。
- 突き合わせ（D.4）：`crates/ritsu/tests/gen_requests.rs`。五つの例の英日の 10 本と、機械の試験の申請の 15 本を、四つの出力先で、申請の種類の一覧の入口を通して流す。どれも、全部のベクタで参照インタプリタと同じ答えを返す（断りは種類まで、履歴の `note` は語まで）。例は 1,740 本、機械の試験の申請は 4,911 本で、ほかに、回覧だけのステップ 70 個を一度に過ぎる申請の 63 本も流す。生成物は `tsc`（strict）、`mypy --strict`、`go vet` と `gofmt`、clippy の `-D warnings` を通る。`--target requests` の `vectors/` は `ritsu ringi vectors` と同じバイトで、`openapi.json` は OpenAPI 3.1 として参照が解け、`operationId` が重ならず、dandori の `use openapi` で読める（休暇を出す flow が通り、範囲を広げた flow と無い種類を呼ぶ flow は E016 になる）。fast の段は、例の 2 本と機械の試験の申請の 2 本を作るだけで、走らせない。
- D.5 は成り立つ。
- 生成する側を変える 6 通りの変異（することの一覧の順、冪等キー、鳴った時計が先、断りの種類、上へ回したあとの催促、保留のあとの時計のずらし方）は、どれも突き合わせが落とす（機械の試験の申請の held、reminds、desk の 511 本を四つの出力先で流した。変えない回は全部が合う）。
- 見つけて直した不具合（どれも試験を足した）：
  - 機械：一つの動きで回る回数を 64 に決めていたので、64 より多くのステップを一度に過ぎるルートを `unread` で断っていた。ステップの数と部分の数から回数を決めるようにした（`crates/ritsu/tests/gen_requests.rs` の `a_route_of_many_stages_is_walked_in_one_move`）。
  - kaname：組み込みのレコード（`indexed` など）のフィールドが型の範囲を持たなかった（`src/ranges.rs`）。値の無いことがある直和型の `match` を、Go が読めない形に書いた（`src/gen/plan.rs`）。Rust で clippy の `-D warnings` が落とす二つの形（`src/gen/plan.rs`、`src/gen/ex.rs`）と、コンパイルできない三つの形（`result` の値を `match` で分ける、`result` を持つレコードの入れ子の深さを確かめる、出力が一つの規則の答えのフィールドを読む。`src/gen/plan.rs`）。
  - rulec：入力の無い規則の記録の関数の、先頭のコンマ（TypeScript、Python、Rust。`src/codegen.rs`）。
  - ringi の試験のデータ：`tests/machine/` の flows、last、once が読む共有の `precheck.flow` の入力がフォームに無く、ritsu の E210 で `ritsu gen` が止まっていた。機械の試験のための `tests/machine/precheck.flow` を足した。
- 残したこと：
  - 画面（A2UI。段階 E）。
  - flow がチケットを開く側の X20（DESIGN 15.4。F.4）。最初の版の flow はチケットを開いたところで終わり、続きは申請の種類の `on` の行が始める。チケットの終わりを開いた flow に返して待つ形は、後の版に回した（DESIGN 2.2、20.2）。
  - つなぎがまだ下ろさない形：値が無いことのある数や日付を、計算、条件、経路、規則が読むこと、規則が列挙として読むフィールド、数でない計算するフィールド、`on` の行のほかの形の条件、単位の刻みの整数にならないかもしれない帳簿と flow の引数など。そういう申請の種類は、コードを書かずに警告で言う。例と機械の試験の申請には無い。
  - kaname の生成器の、書き方で避けた形（Go の列挙の値と関数の名前のぶつかり、Go の `append` の型、Rust の一時の値、Rust の `let` に置いたラムダ、clippy の三つの警告）。直すのは今の言語の側である。
  - 置いたコードの検査の時間：kaname は、つなぎごとに機械のモジュールを検査し直す。例の 10 本の生成に 298 秒、機械の試験の 15 本に 522 秒かかる（試験のビルドで kaname を `opt-level = 2` にしたとき）。
  - playground の `website/docs/playground/ritsu.wasm` は作り直していない（F.3）。
- 段階の終わりの試験（2026-11-15〜16）：golden を書き直す回は一度だけで、`RINGI_BLESS=1 cargo test -p ringi --tests` は全部通り（206 秒）、`RITSU_BLESS=1 cargo test -p ritsu --test library` は 10 件が通り、`RITSU_BLESS=1 RITSU_TEST_LEVEL=tools cargo test -p ritsu --test gen_requests` は 5 件が通った（622 秒。SKIP の行は無い）。書き直した golden（ringi のベクタの一覧と休暇の約束の形の JSON、E201 と `values.rule` の新しい物、ritsu の出力先の一覧の断り、新しい `tests/golden/requests/` の 6 個）は、どれも読んだ。そのあとの `cargo xtask test --level fast -p ringi -p ritsu-ports -p ritsu-project -p ritsu -p kaname -p rulec -p ritsu-base` は、`test result` の行が 242 本、2,800 件が通り、落ちた物は無い（1,288 秒）。SKIP の行は 516 本で、xtask の表の 515 本は、fast では tools と platforms の段を回さないためで、ほかの 1 本は PostgreSQL のサーバーを立てていないためである（rulec の `tests/days.rs`）。`cargo xtask deps` は 22 クレートで ok、`ritsu check ritsu.ctx` は ok（783 の成果物、82 の越え）。

## 22. 段階 D2（人のコメント）で作ったものと確かめたこと（2026-11-16）

- 書き方（DESIGN 3.2、3.3、3.6、10.2）：シナリオの表に、入力の列 `comment` を `by` のあと、`->` の前に書ける。コメントだけのイベントは、イベントの欄に `comment`、文は同じ列に書く。時計と flow の行の `by` と `comment` のセルは空にする（書けば E603）。列の無い表は今のまま読む。`resubmit(…)` と `edit(…)` の引数はフォームのフィールドの名前なので、文は引数にせず列にした。
- 機械（DESIGN 7.2〜7.4）：人の操作（`submit`、`resubmit`、`approve`、`reject`、`send back`、`retract`、`pause`、`resume`、`withdraw`、`discard`、`edit`、`add approver`、`hand over`）は、その人が書いた文を一つ持て、その操作の履歴の行の `comment` に残る（無ければ null。空の文は無いのと同じ）。コメントだけのイベント `comment` は、履歴に `commented` の行を足し、知らせ `comment` を、申請者と代わりに出した人、申請者に戻っているあいだは差し戻した人、今のステップで待っている承認者とその日の承認の代理から、書いた本人を除いた人に出す（保留のあいだも出し、相手がいなければ出さない）。状態、時計、チケットの版を変えず、版を持たないので `stale` で断らない。チケットは、申請者へ差し戻した人（代理が差し戻したなら代理した人本人）を、申請者に戻っているあいだ `sent_back_by` に持つ。前のステップへの差し戻しと申請者の取り下げでは持たず、出し直し、破棄、再申請の期限切れで無くなる（DESIGN 7.1）。文は 2,000 字まで（字の数）で、越えれば `form`。確かめる順は `data`、`clock`、`stale`、`state`、`form`。だれが書けるかは gate が決め、機械は見ない。エージェントの理由は、履歴の `note` から `comment` に移した（`note` は flow の名前だけ）。
- 生成するコード（DESIGN 12.1、12.4）：機械のモジュール `ticket` は 235 関数で、total、警告は無い。人のイベントの型が文を持ち、つなぎの `start` が `submit` の文を確かめる。約束の JSON は、イベントの中の `comment`（`{"approve": {"by": …, "revision": …, "comment": …}}`、`{"comment": {"by": …, "comment": …}}`）、履歴の行の `comment`、チケットの `sent_back_by` を持つ。約束の版は 1 のまま（まだ出していない）。
- `--target requests`（DESIGN 12.2、12.4）：人の操作の本文と MCP の道具の入力に `comment`（文か null、2,000 字まで、無くてよい）。`POST /tickets/{id}/comment`（`operationId` は `comment`、本文は `comment` だけで版は無し）と MCP の道具 `comment`。`types/<名前>.json` の `events` と、履歴のスキーマに `comment`。チケットのスキーマに `sent_back_by`（文か null）。
- gate と X18（DESIGN 9.3、12.4）：例の gate に `action comment` と、申請者、今のステップの承認者、申請の種類を受け持つ部門の人に許す permit を足した。申請者に戻っているあいだは、サーバーが差し戻した人を承認者として渡すので、差し戻した人も申請者の問い返しに答えられる。X18 は、どの申請の種類も持つ操作として `comment` を見る（答えではないので、承認者に許すかは見ない）。X18 の再現の gate にも同じ action を足した。
- 例（DESIGN 10.2）：休暇の英日に、上長がコメントで尋ね、申請者が答え、コメントを添えて差し戻し、申請者がどこに書くかを尋ね返して上長が答え、出し直した申請をコメントを添えて承認するシナリオを一つずつ足した。今あるシナリオの行は変えていない。
- ベクタ（DESIGN 10.4）：人が書くことを、申請の種類ごとに一度ずつ足した（最初のシナリオの申請をコメントを添えて出す 2 本、最初のステップでのコメントとコメントを添えた承認と差し戻しの 8 本、時計が鳴る時刻ちょうどのコメント、差し戻したチケットへの差し戻した人と申請者のコメント、申請者が取り下げたチケットへの申請者のコメント、保留したチケット、代わりの人が出したチケット、終わったチケットへのコメント、代わりの人が出したチケットを差し戻したあとの申請者のコメント、版が進んだチケットへのコメントと、前の版の長すぎるコメントを添えた承認）。承認の代理が申請者へ差し戻すベクタも足した。長いコメントは「𠮷」（UTF-8 で 4 バイト、UTF-16 で 2 単位）を並べ、四つの出力先が字の数で数えることを確かめる。五つの例のベクタは、経費 175 本、購買 285 本、アカウント 179 本、契約 197 本、休暇 164 本になった。前からあるベクタは、キーが足されたほかは変わらない（約束の JSON の人のイベントと履歴の行の `comment`、チケットの `sent_back_by`。`sent_back_by` は、承認者が申請者へ差し戻したチケットでは差し戻した人で、ほかは null）。
- 帳簿が別のデータベースにあるとき（DESIGN 12.4、12.5）：チケットごとの鍵と、保存が失敗したときに取り消す前にやり直すことを、フレームワークが持つサーバーの約束として書いた。ritsu の生成物は変えていない。
- flow とチケット（DESIGN 15.4、20.2）：最初の版の flow はチケットを開いたところで終わり、続きは申請の種類の `on` の行が始める。開く側の X20 は F.4 で作る。
- 見つけて直したこと：DESIGN 16.2 の `run` の JSON の説明が、約束の形にする前の語（部分の `open` と `with`、することの `do`）のままで、チケットの `revision` と `edits` も抜けていた。約束の語に直した。承認の代理が申請者へ差し戻したときの履歴の行に、`proxy_of` が無かった（前のステップへの差し戻しにはあった）。両方の機械で入れた。
- 残したこと：
  - 却下と差し戻しでコメントを必須にする設定、コメントの添付、宛先、編集と削除（DESIGN 2.2）。
  - 承認カードと申請書のコメントの欄と、履歴にコメントを見せること（E.1）。申請者に戻っているチケットでは、だれが差し戻したか（`sent_back_by`）も見せる。
- 段階の終わりの試験（2026-11-16）：golden を書き直す回の `RITSU_BLESS=1 RINGI_BLESS=1 cargo xtask test --level fast -p ringi -p ritsu` は、`test result` の行が 57 本、748 件が通り、落ちた試験は無い（567 秒）。SKIP の行は 112 本で、どれも fast では tools と platforms の段を回さないためである。そのあと休暇のシナリオに上長の答えの行を足し、例の gate の説明の文を直して、`cargo test -p ringi`（218 件。golden を書き直して）と `cargo test -p ritsu --test ringi`（8 件）を回し、どれも通った。ritsu-cross（42 件）と ritsu-ports（11 件）は、差し戻した人を足す前の fast の回で通り、そのあとは触っていない（その回で落ちた 6 件は、どれも D2 に合わせて試験の期待を直す物で、直して回し直した）。`RITSU_TEST_LEVEL=tools cargo test -p ritsu --test gen_requests` は 5 件が通り（614 秒。SKIP の行は無い）、四つの出力先が、例の 2,000 本、機械の試験の申請の 5,147 本、長い経路の 77 本のベクタの、どれでも参照インタプリタと同じ答えを返した。書き直した golden（ベクタの一覧と休暇の約束の形の JSON、休暇の `run`、`items`、`api`、X18 の 18 個、`tests/golden/requests/` の 4 個）は、どれも読んだ。前からあるベクタは、キーが足されたほかは変わらない。`ritsu check src/lower/ticket.kn` は 235 関数で total、`ritsu check ritsu.ctx` は ok（783 の成果物、82 の越え）。

## 23. 段階 D2 の続き（代理の操作の履歴、申請者の側の人、E301 の足す行）で作ったものと確かめたこと（2026-11-16）

- 直しの木を合わせた木の上で作った。rulec の E101 のヒントが、表がすでに書いている区切りと範囲の端だけで切ったセルの行を、穴ごとに返すようになり（答えの列は `?`。根の PLAN 7.138）、ringi の E301 の「例の行のステップの列は、表の最初の行からコピーしたもの」という注が本当でなくなっていた。
- E301（DESIGN 6.5、16.2、21 の 9）：rulec が返す行を、ringi の表の列に並べた「足す行」として見せ、`?` に何を書くかを言う（承認者を決める表なら、そのステップに答える人を `who` の名前で。その申請がステップを飛ばすなら空のセル。経路を選ぶ表なら、その申請が使う共有の経路の別名）。行が幾つかあれば全部を見せ、`fix` の JSON にも全部を入れる。どの行にも当てはまらない申請の範囲を、フォームのフィールドの名前と単位で言う（穴が幾つかあれば、その数と一つずつの範囲。rulec が 8 か所で切ったときは、ほかにもあると言い、行を足せば次の検査が残りを言う。範囲が一点なら、例の申請と同じなので言わない）。コピーの注は消した。試験：E301 の変異の四本と、列の二つある表、穴の十ある表で、`?` を埋めて行を貼ると、次の検査に E301 が無いこと（穴の十ある表は二度の検査で閉じる。`tests/fixes.rs` の `the_rows_e301_shows_close_the_gap`）。
- E302（DESIGN 6.5）：rulec の E105 が言うようになった、二つの行がどちらも取るセルの値を、申請の言葉の範囲で注に足した（`The requests both rows take: amount from 100_000JPY to 200_000JPY`）。範囲で重なる二つの行の変異を、英日で一本ずつ足した。
- ほかの言語の診断を ringi の言葉に直す所を、同じ目で読んだ。E201 と W201 が rulec の注を三つまで見せるとき、注の続きの行（二つの空白で始まる、穴の一つずつや足す行）を別の注と数えて途中で切っていたので、上の注と一緒に数えるようにした。名前の付いた表の穴は、今までどおり E201 で rulec の文を見せる。X18 は sekisho の例をそのまま渡し、X19 と X20 は口の答え（仮押さえの事実と `Flows::declared`）を値で読むので、合わせで変わった文は届かない。置いたコードへの kaname の言葉は、コードと行だけを書く。
- 代理の操作の履歴（DESIGN 5.8、7.3、7.4）：代理がした操作の履歴の行のどれにも、だれの代わりか（`proxy_of`）を残す（両方の機械）。申請者の側の行（`submitted`、`resubmitted`、`withdrawn`、取り下げと破棄で終わる `ticket withdrawn`）は、する人が申請者でなければ申請者。答えの行と、却下で終わる `ticket rejected` は、代理が使った承認者の席。`paused`、`resumed`、`edited` は、する人が今のステップの承認者でなければ、その人がその日に代わる最初の承認者（あとのステップを待つ部分を除き、部分と承認者の並びの順）。`retracted` は、取り消した承認の席。管理する人の操作、`commented`、時計と flow の行には入れない。
- 見つけて直したこと：`same approver approves once`（5.12）で、元のステップの承認で済ませたステップの `approved` の行の `by` が、元のステップで代理が答えたときも代わられた人になっていた。`by` を答えた代理、`proxy_of` を代わられた人にした（最後のステップで一度の書き方は、もとからこの形）。両方の機械。
- ベクタ（DESIGN 10.4）：代わりに出した人がチケットを取り下げるもの、差し戻されたあとに破棄するもの。承認の代理が、承認者の席で却下するもの、保留して再開するもの、直すもの、承認して取り消すもの（保留、直し、取り消しは、申請の種類に行があるものだけ）。`same approver approves once` の申請の種類では、最初のステップを代わられた人に引き継がせて代理が承認し、すぐあとのステップがその承認で済むもの。承認の代理のベクタは、シナリオの申請を順に試し、代理が答えるステップに届く最初の申請で作る（前は最初の申請だけを見て、届かなければ作らなかったので、機械の試験の `last` と `once` には、代理の承認と差し戻しのベクタも加わった）。例のベクタは、経費 178 本、購買 285 本、アカウント 179 本、契約 197 本、休暇 166 本（日本語の名前の版も同じ本数で、合わせて 2,010 本）。機械の試験の申請の 15 本は 5,175 本になった（`last` と `last.ja` が 9 本ずつ、`once` が 9 本、`who` が 1 本増えた）。前からあるベクタは一覧から消えず、休暇の約束の形の JSON（golden）では、前からある行が 1 バイトも変わらない。試験は `tests/vectors.rs` の `a_proxy_says_whom_they_act_for`（代理のどの行も `proxy_of` を持つこと）と、`the_ones_who_submit` の期待の直し。
- 申請者の側の人（DESIGN 5.8、7.2、9.3、12.4）：例の gate に、申請者の側の人（申請者、代わりに出した人、その日に有効な申請の代理）をまとめる resource `Requesters` を足し、`Ticket` と `Stage` の属性 `requesters` にした。取り下げ（破棄を含む）とコメントの permit は、その人に許す。期待を一つ足した（代わりに出した人が取り下げられる）。自分の申請を決めない forbid は `requester` だけを読むまま（代わりに出した人は、自己承認を許さない経路では承認者から外れ、`next` が答えを断る。このチケットを出していない申請の代理は、承認者か承認の代理なら、機械が答えさせる）。DESIGN 12.4 に、サーバーが gate に渡す値を表にした。X18 は、足した属性もほかの属性と同じに全部の組み合わせで尋ねるので、検査は変えていない。`ritsu check examples` の答えは、gate を変える前と同じである。
- 段階の終わりの試験（2026-11-16）：golden を書き直す回の `RITSU_BLESS=1 RINGI_BLESS=1 cargo xtask test --level fast -p ringi -p ritsu` は、`test result` の行が 57 本、752 件が通り、落ちた試験は無い（ringi 220 件、ritsu 532 件。590 秒）。SKIP の行は 116 本で、どれも fast では tools の段を回さないためである。中身が変わった golden は、E301 の変異と `codes/E301`、新しい E302、ベクタの一覧と休暇の約束の形の JSON だけで、どれも読んだ。ritsu の golden（X18、`tests/golden/requests/`）は変わらない。`RITSU_TEST_LEVEL=tools cargo test -p ritsu --test gen_requests` は 5 件が通り（666 秒。SKIP の行は無い）、四つの出力先が、例の 2,010 本、機械の試験の申請の 5,175 本、長い経路の 77 本のベクタの、どれでも参照インタプリタと同じ答えを返した。`ritsu check src/lower/ticket.kn` は 237 関数で total、`ritsu check ritsu.ctx` は ok（784 の成果物、82 の越え）。ritsu-cross は触っていない。

## 24. 段階 E（画面）で作ったものと確かめたこと（2026-11-16）

- 画面（E.1。DESIGN 13.1、13.8）：`src/screens/a2ui.rs` が、申請の種類ごとに、申請書、承認カード、一覧、履歴の A2UI 1.0 のテンプレートを英語と日本語で作り、`ritsu gen --target requests` が `a2ui/<名前>/{form,card,list,history}.{en,ja}.json` に書く。例の 10 の申請の種類で、80 の画面と 240 のメッセージになる。テンプレートは値の無いデータの形を持ち、サーバーが値を入れる置き場は DESIGN 13.8 に表にした。コメントを書く所（申請書と承認カード。2,000 字を越える文は送る前に止める）、履歴のコメントとエージェントの理由（行の `comment`）とメモ、差し戻した人（`sent_back_by`）、空にしたフィールドの `clear`（出し直しは `<申請>.cleared`、承認者の直しは `<申請>.edits_cleared`）、履歴の「空になった項目」（行の `cleared`）を置いた。
- 代わりに申請する相手（DESIGN 5.8、13.8）：API の `submit_<名前>` は `requester` を取るが、申請書に相手を選ぶ所が無かった。`ritsu:PersonPicker` を足し、サーバーが入れる `/submit_for` の人だけから選ぶ（`among`）。`/submit_for` が空なら出ない。`submitted by` の検査は、自分で出せる（`/may_submit`）か、相手を選んだときに通す。
- ボタンのイベント（DESIGN 13.1）：名前を API の操作の `operationId` にそろえた。承認カードの直しのボタンは `edit`（API は `edit_<名前>`）を、一覧の開くボタンは `open`（API に無い）を送っていたので、`edit_<名前>` と `get_ticket` にした。
- カタログと関数（E.2。DESIGN 13.2、13.7）：`src/screens/catalog.rs` が `a2ui/catalog.json`（部品 7 個、どの種類も使う関数 7 個と、種類ごとの関数）を、`src/screens/functions.rs` が `a2ui/functions.js` を書く。`functions.js` は、ビルドの道具なしにブラウザが読む ES モジュールである。種類ごとの `values.rule` は rulec がページのために書く JavaScript をそのまま入れ、フィールドの検査は、宣言から作った説明を一つのコードが読む。DESIGN 13.2 の表に無かった物として、部品 `MasterPicker`（選ぶ値をファイルに書かないマスタの選択）、`PersonPicker` の `among`、`lookup` の `separator`、関数 `one`、`comment`、`commentFits`、`commentGiven`、`<申請>.given`、`.changes`、`.cleared`、`.edits`、`.edits_cleared` を足した。
- カタログの ID（DESIGN 13.2）：`ritsu gen` の `--catalog <url>` と、ritsu.mod の `catalog "<url>"` の行（`crates/ritsu-base/src/modules/manifest.rs`）。どちらも無いとき、http か https の URL でないとき、`requests` の無い `--catalog` は 2 で止まり、ritsu.mod の行の誤りは E302 である。
- 突き合わせと文言（E.3。DESIGN 4.8、13.3、13.4）：`src/screens/bindings.rs` が、どの画面も根からたどって、見せる値と送る値を比べる。E701 と W702 は、テンプレートを書く前に、申請の最初の行で言う。E703（宣言した言語の文言の欠け）は `src/screens/mod.rs` の `labels_said` が言い、例と英日の変異を足した（台帳の 59 個のうち、例と変異があるのは 57 個になった。E701 と W702 は ringi の不具合のときだけ出るので、例が無い）。計算するフィールドの範囲の外と明細の行の数は、送る前に、どのフィールドのどの範囲かを言う文で止める。出せない人には、送る前に「この申請を出せる人に入っていません」と言う。NOT SHOWN の形（文言を別のパスに置く）は作らない。
- 1.0 のスキーマ（E.1）：A2UI のリポジトリの 1.0 のスキーマを、`screens/a2ui-1.0/` でなく `tests/fixtures/a2ui-1.0/` に写した（Apache License 2.0 の文と NOTICE を添えた）。読むのは試験だけで、クレートのコードは読まないからである。検証は `tests/a2ui/schema.rs` に書いた JSON Schema 2020-12 の読み手で、写したスキーマが使うキーワードの外に会えば止まる。
- 試験：`tests/a2ui.rs` の 9 件（スキーマで読める、一つの木でデータにある置き場を読む、入力が範囲と検査を持つ、見せた値だけを送る、イベントが API の操作で本文の取るキーを送る、壊す変更を見つける、`functions.js` が Node でベクタに答える、選択の値を規則が別の名前で書く申請でも答える、検証の読み手そのもの）。`crates/ritsu/tests/gen_requests.rs` は、経費と休暇の画面とカタログと `functions.js` の golden と、カタログの ID の渡し方と断り（`the_screens_name_the_catalog_they_are_given`）。`crates/ritsu-base/tests/modules_manifest.rs` は `catalog` の行、`crates/ritsu/tests/once.rs` は `--catalog` を渡して `functions.js` まで書くこと、`requests` だけを書いても、どのファイルも一度だけ検査し、`functions.js` が同じになること。`gen_requests.rs` は、日本語の実行でも `requests` のどのファイルも同じになること。
- 見つけて直したこと：
  - W702 の突き合わせが、明細の列の数を読むとき、パスの最初の部分を飛ばしていた（`bindings.rs` の `field_at`）。
  - 一覧の画面が `/masters/<フィールド>` を読むのに、データに `masters` が無かった。
  - `functions.js` が、どの種類にも出し直しと直しの関数を定義していた。カタログと同じく、その操作のある種類だけにした。
  - 承認カードの直しと一覧の開くボタンのイベントの名前が、API の操作に無かった（上）。
  - 日本語の画面で、`withdraw` を「引き戻す」、`discard` を「申請を取り下げる」と言っていた。DESIGN の言葉（取り下げ、やめる）に直した。
  - 英語の名前の申請の日本語の画面で、英語の題に日本語の文がつながっていた（`Kind of leaveを選んでください。`）。空白を置いた（DESIGN 13.4）。
  - 小数のある数の範囲の端が -1 と 0 のあいだのとき、文が符号を落としていた。
  - 出せない人への文が、`submitted by` の行をそのまま見せていた。
  - 渡されなくなった申請の種類の画面のファイルを消したあと、そのディレクトリ（`a2ui/<名前>/`）が空のまま残った。空になれば消すようにした（ritsu の `package.rs`）。
  - `functions.js` が、下ろしたファイルが別の名前で書く選択の値（`after` は `after_value`、DESIGN 6.4）を、そのまま規則に渡していた。規則が入力を断るので、その値のときは何も計算できず、条件で隠すフィールドが見え、条件で要るフィールドを空のまま通した。フォームの説明に規則の側の名前を持たせ、関数がそれに直して渡す（DESIGN 13.7）。
  - `functions.js` に入れる規則の JavaScript を作るとき、規則が読む `choices.rule` をもう一度検査していた（ritsu の DESIGN 4.15 に反する）。rulec の `Engine::page_javascript` が、規則と `enums` のファイルを、検査で覚えたものから JavaScript にする（`Gen::page_javascript_with`）。ritsu は、下ろしたファイルの検査のあとにそれを呼び、実行の言語によらず英語で書く。
  - 経費の例の `labels ja` に `who board` の文言が無く、E703 になった。`取締役会` を足した（ritsu の `tests/golden/requests/types/expense.json` も変わる）。
- 残したこと：
  - レンダラで描いて見ること（サンドボックスか、別のリポジトリの S2。E.4）。
  - 承認者を加えるとき、並列のステップのどの部分に加えるか（API の `part`）を、承認カードでは選べない（null は、待っている最初の部分）。
  - 根の DESIGN の 9.6 と 17.3 に、`--catalog` と `catalog` の行を書くこと。
  - rulec の DESIGN の 15 章に、`Engine::page_javascript` と `Gen::page_javascript_with` の節を足すこと（節の番号は、ほかのレーンと合わせるときに決める）。
  - playground の `website/docs/playground/ritsu.wasm` は作り直していない（F.3）。
  - 日本語の実行では、下ろした規則が二度検査される（段階 E の前から。ritsu の DESIGN 4.15）。ringi の検査（`src/mapped.rs`）が下ろしたファイルを rulec に英語で尋ね（何か言うときだけ日本語でも）、実行の検査が日本語で尋ねるからである。出力は正しく、検査の回数だけが多い。直すなら、ringi の検査が実行の言語を受け取り、先にその言語で尋ねる。ritsu の `tests/once.rs` は英語の実行だけを見ている。
- E.4 は成り立つ。
- 段階の終わりの試験（2026-11-16）：ringi、ritsu、ritsu-base の fast を一度回し、1,021 件が通って 2 件が落ちた（上の `functions.js` の二つ）。直したあと、落ちた二つの試験のファイルと触った試験を回し直して通った（ringi の `tests/a2ui.rs` 9 件は Node で、ritsu の `gen_requests` 8 件、`once`、ringi と ritsu の lib）。直しで rulec を触ったので、rulec の fast も回し、1,585 件が通った。`cargo xtask deps` は 22 クレートが層の中、`ritsu check ritsu.ctx` は 15 のコンテキストと 784 の成果物で通る。段階 E の前のバイナリと比べ、例の申請、sekisho の返金の例、モジュール `japan-tax` の `ritsu gen` の出力は、`requests/a2ui/` のほかは一字も違わない。

## 25. 段階 E の続き（画面の検査を機械の断りにそろえる）で作ったものと確かめたこと（2026-11-16）

申請者が画面で通したフォームを、サーバーが断ることが無いようにする回である。ベクタの外のフォームには、画面と機械の判定が分かれる物があった。

- 見せているあいだ必須（DESIGN 4.2、13.2、13.7）：`shown when` があり、`optional` の無いフィールドは、画面でも見せているあいだ必須である。入力の部品と、申請するボタンと出し直すボタンに `<フィールド>.required` を置いた。`functions.js` の `isRequired` は、いつも必須、見せているあいだ必須、`required when` が成り立つあいだ必須の三つを読む。隠れているフィールドは検査せず、送らない。明細の行の中のフィールドと承認者が直すフィールドも同じに読む（`src/gen/mod.rs` の `Fld::asked`）。承認者だけのフィールドは、機械が必須にしないので必須にしない。例では経費の `trip_to` が変わり、出張で行き先が空の申請を画面が止める。
- 選択肢の一つか（DESIGN 13.2、13.7）：選択のフィールドに `<フィールド>.choice` を置き、選択肢に無い値を止める。部品は選択肢に無い値を作らないが、データは外から入ることがある。マスタから選ぶフィールドのキー、人とチームの ID が組織にいるか、`open in` の日がカレンダーの開いている日かは、画面で確かめられないので、サーバーが `next` で確かめる（DESIGN 13.7）。
- 承認者の直し（DESIGN 13.2、13.8）：直す部品の表示の条件と検査を、直したあとのフォーム（新しい関数 `<申請>.edited(edit, before)`）で読み、保存のボタンに、そのフォームの全部の検査を置いた（例の経費で 15 個）。前は保存のボタンにコメントの長さの検査しか無く、範囲の外の値も、直しで必須になったフィールドを空にしたままの直しも送った。`edits` は直したあとに見えるフィールドだけを送り、`edits_cleared` は直したあとに隠れたフィールドも挙げる。サーバーは `/edit` をチケットの値で埋めて送る（埋めなければ、値のあるフィールドを空にする直しになる）。出し直しの `cleared` は、任意のフィールドの印（`clearable`）を読まず、値を持っていたフィールドが隠れたか空になれば挙げる（必須のフィールドを空にすれば、検査がボタンを止める）。
- 突き合わせの試験（DESIGN 13.7、18）：`tests/a2ui/forms.rs` の `forms::the_screens_stop_what_the_reference_refuses`。`types/<名前>.json` から、フィールドごと、検査の種類ごとに、その検査だけを破るフォームと端でちょうど通るフォームを作り、申請、出し直し、承認者の直しの画面を、レンダラが読むとおりに読んで、参照インタプリタの判定と比べる。例の 10 の種類と、例に無い形のために書いた申請 `gear` で 842 のフォームを作り、両方が止めるのは 370 個、判定が分かれる物は 0 個、文の両端の空白で分かれうる 55 個は、書いたままの文も参照インタプリタに渡して数える。テンプレートから経費の `trip_to.required` を抜くと、この試験が落ちる。検査の種類と申請の種類ごとの表は notes に置いた。
- 2^53 を越える数（DESIGN 9.5、13.3、13.7）：範囲の端が、単位の刻みで数えて 2^53 − 1 を越える数のフィールドは、`ritsu gen --target requests` が画面を作る前に E704 で言い、何も書かない（`src/screens/mod.rs` の `numbers_said`）。台帳は 60 個で、例と変異のある 58 個に E704 が入った（英日の変異と golden）。画面の範囲の検査は `Number.isSafeInteger` で読み、丸まった数を止める。`gear` は、範囲が ±9_007_199_254_740_991 のフィールドで、端の値が書いたとおりに送られることを確かめる。
- ベクタ（DESIGN 10.4）：条件で要るフィールド（見せているあいだ必須と `required when`）を、値を持つ最初のシナリオの申請から空にして出すベクタを足した（`src/vectors.rs` の `left_empty`）。経費の `trip_to` と `receipt` は `form` で断り、休暇の `reason` は条件（`kind is special`）の成り立たない申請なので受け取る。例のベクタは、経費 235 本、購買 357 本、アカウント 188 本、契約 273 本、休暇 177 本（日本語の名前の版も同じで、合わせて 2,460 本）。機械の試験の申請では、`supplies` とその日本語の名前の版に 1 本ずつ足した。例のベクタに無い断りの種類は `data`、`book`、`unread` の三つで、どれも足すのは小さくないので、一覧と理由を notes に書いた。
- 書式の関数に null を渡さない（DESIGN 13.1）：承認カードと一覧の数、金額、日付、添付と、申請書の計算するフィールドの値は、値があるあいだだけ見せる（ラベルは残す）。
- 日本語の実行（DESIGN 6.5、ritsu の DESIGN 4.15）：ringi の検査は実行の言語を受け取り（`check::Run::set_lang`、`ports::Engine::set_lang`）、下ろしたファイルをその言語で先に尋ねる（`src/mapped.rs`）。ritsu の `check`、`gen`、`ritsu ringi` は、実行のあいだ rulec をその言語で読ませる。rulec は検査の報告を言語ごとに覚えるので、検査のあとに言語を言わずに尋ねる主張が、規則の読む `choices.rule` を英語で読み直し、二度目の検査になっていた。ritsu の `tests/once.rs` に日本語の実行（`ritsu check`、パッケージと一緒の `gen`、`requests` だけの `gen`、`ritsu ringi check --lang ja`）を戻した。
- 承認者を加える部分（DESIGN 13.1、13.8）：承認カードの承認者を加えるボタンを、今のステップの承認者を待っている部分ごとに置き（サーバーが入れる `/add_approver/parts`）、押した行の部分を API の `part` として送る。カタログに部品は足していない。
- カタログの ID（DESIGN 13.2）：ID は、そのプロジェクトのカタログを指す名前で、自分の持つ URL の形で書く。配る場所と同じにしておくと分かりやすいが、同じである要は無く、環境が変わっても同じ値を使う。`--catalog` が無いときの文、ヘルプ、ritsu.mod の `catalog` の行の注（`crates/ritsu-base/src/modules/manifest.rs` の `catalog_note`）を、この言い方にした。渡し方は変えていない。
- 文の両端の空白（DESIGN 13.7）：画面は、文のフィールド（短文、長文、明細の行の文）を、コメントと同じく両端の空白を除いて読み、何も残らなければ値が無いのと同じに読む。機械は変えていない。画面のほうが厳しい、ただ一つの分かれ方である。
- 古い行：この PLAN の E.1（スキーマの置き場）と E.2、DESIGN 13.7 の数と、DESIGN 6.5 の下ろしたファイルを尋ねる言語を、今の姿に書き直した。
- 見つけて直したこと：
  - 承認者の直しの保存のボタンが、直したあとのフォームを確かめていなかった（上）。
  - `/files` がフィールドの名前をキーにしていて、明細の行の添付を引けなかった。ファイルの ID をキーにした（DESIGN 13.8）。
  - 日本語の実行で、検査のあとに尋ねる主張が `choices.rule` をもう一度検査していた（上）。
- 残したこと：
  - 通貨でない単位の付いた数（率、`mass[kg]` など）を、承認カードと一覧は、送る整数のまま単位を付けずに見せ、申請書の `AmountField` は率に単位を渡さない（`rate[step 0.1%]` の 12.5% が「125.000」と見え、人は 0.125 と入れることになる）。例の申請には無い。直し方の案は notes にある。
  - 条件つきの表示と自動計算を、それが読むフィールドだけで決める形（次の段。今の `functions.js` で変える所は notes の申し送り）。
  - 例のベクタに無い断りの種類（`data`、`book`、`unread`）。
  - レンダラで描いて見ること（E.4、S2）、根の DESIGN の 4.15、9.6、17.3 と rulec の DESIGN の 15 章の節（notes に案）、playground の wasm。
- 回の終わりの試験（2026-11-16）：ringi、ritsu、ritsu-base、ritsu-project の fast を一度回し、1,030 件が通って 2 件が落ちた（ringi の `tests/cli.rs` が台帳の数を 59 と読んでいた。ritsu の `once` は、上の日本語の実行の二度目の検査）。直したあと、落ちた二つの試験と、直しが触る ritsu の試験（`check`、`codes`、`cross`、`enums_files`、`gen`、`playground`、`ringi`、`spellings`）を回し直して通った。tools の段では、ringi の `tests/a2ui.rs` の 10 件（Node v24.21.0 で、例のベクタの 388 のフォーム、選択の値を規則が別の名前で書く申請の 39 のフォーム、突き合わせの試験の 842 のフォーム）と、ritsu の `gen_requests` の 8 件と `once` が通った。rulec は触っていないので、rulec の試験は回していない。`cargo xtask deps` は 22 クレートが層の中、`ritsu check ritsu.ctx` は 15 のコンテキストと 784 の成果物で通る。段階 E の終わりのバイナリと比べ、例の申請の `ritsu gen` の出力は、`requests/a2ui/` と、経費と休暇のベクタのほかは一字も違わない。sekisho の返金の例とモジュール `japan-tax` の出力は一字も違わない。

## 26. フレームワークからの頼み（A〜Q）で直したものと確かめたこと（2026-11-17）

生成物を動かすサーバーのフレームワーク（別のリポジトリの S1〜S3）が、約束と生成物の足りない所を見つけた回である。どれも理想より劣るので、不具合として直した。決めたことの理由と、フレームワークの担当への知らせは notes にある。

- 帳簿の仮押さえのキー（A。DESIGN 7.1、12.4、20.1）：`submit` と `resubmit` が、サーバーの付けた試みの名前（`attempt`）を持ち、チケットは今の `round` を始めた試みを持つ（`ticket.attempt`）。例の予算と休暇の帳簿は `key request, attempt` にした。振替のキーが試みを受け取らない仮押さえは E408（台帳は 61 個、例と変異のある物は 59 個）。chobo は変えていない。`tests/books_apart.rs` は、chobo が例の予算から書く SQL を PostgreSQL で動かし、帳簿をチケットと同じトランザクションに入れる形と、帳簿の呼び出しだけを確定させる形の両方で、四つのこと（送り直しは同じ答えで一度だけ押さえる、断られたあとの新しい試みは新しく確かめられる、取り消した仮押さえがあとの試みを止めない、保存しなかった試みの仮押さえをその試みのキーで取り消せる）を確かめる。キーを申請と `round` に戻した帳簿では、別に置く形で二つ目と三つ目が落ちる。
- 本文の誤り（B。DESIGN 12.4）：形の誤りは 400、形の合う値でフォームが取らない値は `next` に渡して 409 の `form`。`openapi.json` の 400 と 409 の説明も同じにした。版を持たない承認のベクタ（申請の種類ごとに一本）は、機械が `stale` で断る行で、API では 400 になる。生成した入口を直に呼ぶサーバーのための行として残した。
- 管理する人の操作の `by`（C。DESIGN 5.9、12.4）：機械は前から受け、履歴の行に残していた。約束に書き、名指しの `add_approver` と `hand_over` のベクタを、申請の種類ごとに一度足した。
- 説明の `flows`（D。DESIGN 12.2、15）：申請の種類が使う flow ごとに、役目、別名、ファイル、使う所、ワークフローの名前と版、Temporal での型とタスクキュー、出力（宣言の順）。dandori の口 `FlowDeclared` に `version` と `workflow_type` を足した。キーを足しただけなので、これだけでは約束の版を上げない。
- dandori の結果の順（E。dandori の DESIGN 4.2）：Temporal の Go のワークフローは、出力と、出力が持つレコードのフィールドを宣言の順に書いた JSON を結果にする（`ddInOrder`）。TypeScript は `succeed` を出力の宣言の順に書く。Python は SDK の JSON の変換が名前の順に書くので、変えずに DESIGN に書いた。dandori の試験は、開発用サーバーで動かした実行の履歴から結果を読んで順を確かめる。サーバーは、要約の出力を説明の `flows` の順に並べて渡す。
- 要約の出力の型（F。DESIGN 15）：要約が持てない型（一つのレコード、レコードでない物の並び、`json`）の出力を、X20 の E210 で言う。ritsu の `tests/cross.rs` に四つの形（行の並びは成り立つ）。
- 画面の芯から（G。DESIGN 12.4、13.1、13.7、13.8）：承認カードに申請者と、代わりに出した申請なら出した人を見せる。サーバーが値を入れなければならない置き場を足すときは約束の版を上げる、と書き、この回で置き場を足したので約束の版を 2 にした。A2UI 1.0 の `renderer_to_agent.json` を試験の写しに足し、画面の突き合わせの試験が、ボタンの作るメッセージ（934 個）をそのスキーマで確かめる。部分が止まるシナリオを契約の例に足した（`a_lawyer_after_the_team_shrinks`。DESIGN 5.9）。
- 断りの置き場（H。DESIGN 12.4、13.1、13.8）：`/refused`（`text`、`field`）。申請書と承認カードは、フィールドを名指す断りをそのフィールドの横に、ほかを画面の頭に見せる。
- MCP から（I。DESIGN 12.2、12.4）：冪等キーの説明（新しい操作には新しいキー、送り直しには同じキー）と形（1〜255 バイトの表示できる ASCII）。道具の入力のスキーマに、フィールドの説明、ほかの言語の文言、表示と必須の条件、選択の値の文言を入れ、申請の道具の説明に申請の種類の説明を入れた。断りの文を英語と日本語の両方で書く（`text`、`text_ja`。どの申請の種類でも、`--lang` によらない）。申請者の操作の道具（`withdraw`、`discard`、`retract`）を、戻せない操作の印（`destructiveHint`）を付けて足し、読む道具に `readOnlyHint` を付けた。管理する人の操作を道具にしない理由を DESIGN 12.2 に書いた。
- 素の HTML の画面から（J。DESIGN 12.2、12.4、13.6）：断りの場所 `at`（フィールドの ASCII の名前か、明細の行のセルなら `<明細>[<行>].<セル>`）。説明のフィールドの `worked_on_screen`。契約の `risks` に `shown when kind is not nda` を付け、秘密保持契約のシナリオと、条件で見せる承認者のフィールドの直しのベクタ（`src/vectors.rs` の `edits_conditioned`）を足した。任意の明細は購買の `others`（L）。DESIGN 13.6 を、五つの画面のどれにも素の HTML の形がある今の姿に書き直した。
- 画面と HTTP から（K。DESIGN 12.2、12.4、13.1、13.2、13.8）：自分の申請の一覧（`GET /mine`、`list_mine`、画面 `mine`）。見られる人の決まり（組み込みの人と gate の `see`。見られない人の操作は 404）と、例の gate の `see`、`Ticket.owner`、期待の二つ。X18 は、gate に `see` の action が無ければ E911 で、見られる人がどうなるかを注で言う。添付の預け先 `/upload`（`POST`、`file` の一つの部分、`{"id": …}`）。マスタの名前の列 `name_field`（キーでない最初の文の列）。出せない人への画面の文は、自分の申請として出せないことを言う（「この申請は、自分の申請としては出せません。」）。代わりに出す申請は機械が断り、その文が出せない申請者を名指す。
- 例の率（L）：購買に、前払いの割合 `advance : rate[step 5%]`（0%〜100%）と、ほかの見積もり `others`（0〜3 行）を足し、財務の確認を `when authority.finance or advance > 30%` にした。シナリオ `half_paid_in_advance`。
- 組織に無い ID（M。DESIGN 8.6）：同じ行、同じ決め方、同じ ID の人かチームが二つ以上の版に無いとき、一つの E503 で、無い版を全部挙げる（`src/org/listing.rs` の `why_apart`、`versions_in`）。E502 と W502 の原因も同じ書き方なので、同じにまとまる。
- 初めの一回（N。DESIGN 9.3）：`ritsu gen --target requests` は、その回が書く `openapi.json` を検査の前に置いて検査する（ritsu の `package.rs` の `api_laid`）。文書は申請だけから作り、作るときは申請の読む flow と gate を読まずに通ったものとする（`FlowsUnread`、`GatesUnread`）。文書を作る ringi は自分の物で、ほかの言語（規則、カレンダー、帳簿、モジュール）は検査と同じ物を読むので、文書を置く回では、申請とその読むデータを二度検査し、ほかのファイルは一度である。置くかどうかは、プロジェクトが届く flow と gate（申請と共有の経路が `use flow` と `use gate` で読む物、gate が読む gate とワークフローとして名指す flow、flow の子）のどれかの `use openapi` が、その回の書く文書を指しているかで決める（`reads_the_api`。届く先は ringi と dandori の参照と sekisho の構文解析で、`use openapi` は dandori と sekisho の構文解析で、書いたとおりに読む。sekisho の参照は検査をするので使わない。パスは、書いたファイルのディレクトリから、システムが開くとおりに比べる）。ほかの API だけを読む回は置かず、どのファイルも一度である（`tests/once.rs` に両方の回）。依存するモジュールのファイルを集めるのも、文書を置いてからにした。初めの一回も通り、flow がその回の書く文書と合わなければ dandori の E016 で、何も書かない。
- 版の範囲（O。DESIGN 12.4）：`openapi.json` の `revision` に、生成した入口と同じ範囲を書いた（`round` 1〜1,000,000、`stage` 0〜1,000,000 か null、`edits` 0〜1,000,000）。生成した入口が止めた値は、サーバーが 400 で返す。
- 一覧のページと操作の数の制限（P。DESIGN 12.4、13.2）：`list_waiting` と `list_mine` の `limit`（1〜100、無ければ 50）と `before`。429 になってよい操作はコメント（`comment`）だけで、制限は、同じ人が同じチケットに続けて書く間隔と、チケットごとの数である。間隔の 429 には `Retry-After` を付け、待っても減らない数の上限には付けない。添付の預け先も、一人が預けてまだどのチケットにも付いていない数を制限してよく、`Retry-After` の無い 429 で答える。`tests/documents.rs` が、`comment` のほかに 429 を持つ操作が無いことと、`Retry-After` の形を確かめる。
- 入口の文の言語（Q2。DESIGN 12.2）：各言語の申請の種類の一覧に、入口のエラーの文の言語（`en` か `ja`）を書く。TypeScript は `lang`、Python と Rust は `LANG`、Go は `Lang` である。ritsu の `gen_requests` の入口の誤りの試験が、四つの出力先の英日で、一覧の言語が `--lang` と同じことを確かめる。
- `openapi.json` の `required`（Q3。DESIGN 12.4）：入口の読み方にそろえた。フォームと明細の行のフィールド、コメントの操作の文は `required` に書かず、null も取る。いつも必須のフィールドは、説明に `Required` と書く。版（`revision` と三つのキー）は要るままにし、入口より API が狭い一つの所として 12.4 に書いた。ritsu の `gen_requests` の `the_api_requires_what_the_entries_stop_without` が、経費と休暇の 13 の操作の本文を一か所ずつ変えた 168 の呼び出しで、スキーマと Python の入口を比べる（スキーマが取る 82 は入口も答え、取らない 56 は入口も止める。版の 30 は API だけが断る）。前の `openapi.json` を同じ比べ方で読むと、フォームとコメントの 20 か所が食い違っていた。
- 値を探すアドレス（Q4。DESIGN 13.2、13.8）：カタログの `PersonPicker` と `MasterPicker` に `find`（必須）を足し、テンプレートの部品は `/find` を読む。データの `/find` は、探す部品のある画面だけが持ち、サーバーが入れる。サーバーの答えの形（`<find>/people`、`<find>/teams`、`<find>/masters/<マスタ>` への `GET`。引数は `q`、`limit`、`lang` で、答えは `{"found": [{"value", "name", "label"}], "more"}`）を 13.2 に書いた。`tests/a2ui.rs` の `every_picker_finds_at_the_address_the_server_fills` と、壊す試験の二つ（`find` の無い部品はスキーマで、`/find` の無いデータは木の確かめで落ちる）。置き場を足したが、約束の版はこの回で 2 にしたので、2 に含めた。
- 入口の誤りの引数を構造で持つこと（Q1）：kaname の生成器を直すので、この回では直さず、直しのレーンの T15c に回した。案は notes にある。
- ベクタ（DESIGN 10.4）：例のベクタは、経費 237 本、購買 435 本、アカウント 190 本、契約 441 本、休暇 179 本（日本語の名前の版も同じで、合わせて 2,964 本）。前のベクタは全部残り、答えが変わったのは、例の帳簿のキーを試みにしたことによる、仮押さえと確定と取り消しの引数（`round` が `attempt` に）だけである（経費 90 本、購買 130 本、休暇 72 本。日本語の名前の版も同じ）。足したのは、経費 2 本、休暇 2 本、アカウント 2 本、購買 78 本、契約 168 本である（日本語の名前の版も同じ）。
- 見つけて直したこと：
  - E911 の台帳の文が、チケットの操作の一覧に `comment` を挙げていなかった。`see` と一緒に足した。
  - 置く文書を、はじめは検査を通る申請から作っていた。自分の申請の文書を読む flow は、文書が無いと通らないので、初めの一回で何も置けなかった（`gen_requests` の試験で見つけた）。文書を作るときは flow を読まないようにした。置くかどうかも、`ritsu gen` に渡した `.flow` だけを見ていて、申請が `use flow` で読む flow を見ていなかったので、参照をたどるようにした。
  - 申請の読む gate がその flow のワークフローを名指していると、文書を作るときの gate の検査（sekisho の E208）が文書の無い flow を読んで落ち、初めの一回で何も置けなかった。gate が API の操作を守る（`use openapi` と `guards`）形も、文書を作るときに gate を検査していたので、同じ作りだった。文書を作るときは gate も読まないようにし、`gen_requests` の試験の gate を、両方をする形にした。また、文書を作るときにほかの言語の検査を全部作り直していたので、規則、カレンダー、帳簿まで一回に二度検査していた。検査と同じ物を読むようにして、`tests/once.rs` に、flow が API を読み、gate がそのワークフローを名指す回を足した。置くかどうかを決めるときに sekisho の参照を読むと、sekisho の検査が文書の無い flow を読み、その答えが回の検査に残ることも、直す途中で分かったので、gate は構文解析だけで読む。
  - `untaken`（`src/lower/ticket.kn`）の断りを `refusal_at` で作ると、kaname の書く Rust が `Some((&_t1).clone())` になり、生成したクレートが clippy の `needless_borrow` で通らなかった。断りをレコードで書いた。kaname の側の直しは、直しのレーンへの知らせとして notes に書いた。

## 27. 値の無い所を読む条件と計算、画面の決め方、経路の `apply` で直したものと確かめたこと（2026-11-17）

条件と計算が、値の無いことがあるフィールドを読めるようにし（A）、画面が、条件と計算を読むフィールドだけで決めるようにし（B）、経路が使う規則を `apply` で呼ぶようにした（C.1）回である。合わせで落ちた 3 件（0）、断りの場所（D）、前の段からの申し送り（1〜10）も直した。勧めの形で決めた所（★）と、その理由は notes にある。

- 合わせで落ちた 3 件（0。DESIGN 4.4、6.4）：rulec が、計算するフィールドとその元を一緒に読む条件の表の、届かない行に E102 を言うようになり、ringi はそれを E201 と言っていた。ringi が条件を切った表（`form.rule`、`values.rule`、E105 の問い）の届かない行は、どの申請も来ない部分なので、その行を除いて尋ね直す。経路の表と名前の付いた表の届かない行は、申請の誤り（E303、E201）のままである。W201 の変異は、整数だけが外す組み合わせの形（`over = asked - left` と `asked >= 2 or left < 1`）に作り直した。
- 断りの場所（D。DESIGN 12.4、13.8）：明細の行のセルを `items[1].price`（行は 0 から）と書く。ベクタは、6 つのファイルの 17 本（例の 12 本：英語の版で経費 2、購買 4、日本語の名前の版も同じ。機械の試験の 5 本：`ranges` 1、`steps` 4）の `at` だけが変わった。
- 値の無い所（A。DESIGN 4.5、4.9、6.4）：値の無いことを `none` という値の一つとして読む。値の無いフィールドを読む計算は値を持たず、合計は、読むセルが空の行を足さない。条件だけが読むフィールドは値の無いことがある入力（`T?`）に、計算が読むフィールドは値と `<名前>_given` の二つの入力に下ろす。E309 の二つの理由（`none` になる規則の出力を経路が読む形と、共有の経路の入力を申請が渡さない形）は出なくなり、その変異（英語と日本語の名前の版）を除いた。機械の試験の申請 `tests/machine/absent.ringi`（三つのシナリオ、ベクタ 226 本）を足した。条件は `is`、`is not`、`is none`、`is not none`、大小の比べで値の無いフィールドと計算を読み、経路の表は `none` のセル、値を並べたセル（`small, large`）、値の無いことがある計算（`over`）を読む。読み方を一つずつ変えた七つの変異（`is` が空で成り立つ、比べが空で成り立つ、`is none` が成り立たない、合計が空の行の仮の値を足す、計算が空を読んでも仮の値から出した値を持つ、値を並べたセルが空で成り立つ、`is not none` が `is none` と同じ）は、どれも機械とベクタの試験が落とす。
- 画面の決め方（B。DESIGN 12.2、13.7）：画面は、出力ごとに、読むフィールドの値と、読まない引数の仮の値で `values.rule` を呼ぶ。申請の種類の説明は、条件と計算に `reads` を持つ。`functions.js`（例の 10 の種類）は 92,220 から 99,280 バイトになった。下ろすファイルの数は変わらない。
- 経路の `apply`（C.1。DESIGN 6.4）：経路は、使う規則の出力を全部読むなら、規則を `apply` で呼ぶ。出力の一部だけを読む規則、入力にフィールドを渡せない規則（単位を `円` で書いた日本語の版の金額など）、例と主張が出力を読む規則などは、今のまま `fed by` で読む。rulec の口に、規則の意味の固定を答える `Rules::meaning` を足した（ritsu-ports と rulec の `ports.rs`）。例と機械の試験と変異の 309 のファイルの検査の答え（W201 が 4、E301 が 4）と、通る 63 のファイルのベクタは、`apply` に下ろす前と 1 バイトも変わらない。購買の英語の版は、経路の規則のコードが権限表を中に持つので、権限表を渡さない `ritsu gen` でも組める。日本語の版は `fed by` のままで、権限表を渡さない `ritsu gen` は止まる。
- kaname の Rust（kaname の DESIGN 36.6）：値の無いことがある数を規則に渡すときと、規則の値の無いことがある数を読むとき、`Option` を同じ型に読み替える `x.map(|y| y)` を書いていて、clippy の `map_identity` で通らなかった。そのまま渡し、そのまま読むようにした。kaname の試験のモジュール `tests/modules/absent.kn`（と日本語の版）を足した。
- 申し送り：
  - 率 × 数（1。DESIGN 4.4）：`share * 2` を率と読む（rulec と同じ）。E009 にならない。
  - `℃` と `℉`（2）：字句が、単位を書く名前の頭として読む（`temperature[℃]`）。
  - `--target requests` だけの回（3）：組織を読まない申請を `requests` の説明に入れないことを、言語の出力先と同じ警告で言う（「`requests` does not describe `x` (…); the other parts are written」）。
  - オフセットの違うカレンダー（4）：今あるコードでは言えない。警告のコードを足すことになるので作らず、機械とつなぎが、時計の時刻を、その時計を数えるカレンダーのオフセットで決める形を勧めとして返した。
  - `ritsu impact --lang`（5）：パッケージを書いた言語（生成物の頭の行が `Source:` か `もと:` か）で生成し直して比べる。
  - 入口の端（6。DESIGN 12.2、kaname の DESIGN 16.4）：四つの出力先が、同じキーの二度目を読み、int64 の外の整数を桁の数によらず「int64 の外」と言い、対になっていないサロゲートを Unicode として正しくない文字列として断る。Go の入口はモジュールを `m0` などの名前で読み、申請の種類の名前（`order`、`pairs`、`strings`）と重ならない。突き合わせの試験は、約束の外の 35 の呼び出しと内の 5 つを、四つの出力先と英日で流す。
  - 断りの文（7）：生成したコードの突き合わせの試験は、断りの文も全部比べる。機械の文が参照の文とまだ違う 8 つの種類（人の名前、時計の時刻、イベントの名前、取り下げの段、戻せる回数の行、書いた段の名前など）は、理由を付けた名前つきの一覧にし、違いが全部どれかの種類に入ることと、どの種類にも違いが一つ以上あることを確かめる。機械の文を参照にそろえるのは次の段で、そのとき一覧を空にする。
  - 一回に一度の検査（8。DESIGN 9.3）：`ritsu gen --target requests` の、flow か gate がその回の書く API を読む回でも、申請とその読むデータを一度だけ検査する。`tests/once.rs` の許しを外した。
- 下ろした規則が準用する規則の検査（rulec の DESIGN §15.320 の決定 1、3、7）：C.1 のあと、日本語の `ringi check` が決裁権限表を二度検査していた。ringi は下ろした規則をディスクの上に重ねて検査し、準用する規則を、規則のディレクトリと書いたパスをつないだパスで、実行の言語で検査したからである（経路を下ろすとき、口は同じ規則を、走らせたディレクトリからのパスで、英語で検査している）。rulec は、覚えた検査を読むファイルの層によらず、読んだものが同じに読めるあいだ使い、準用する規則と `enums` のファイルを走らせたディレクトリからのパスで検査し、通るかと誤りのコードしか読まない所はどの言語の報告からでも答えるようになった。ritsu の `tests/once.rs` が通る。
  - 申請の種類の言語とラベル（10。DESIGN 4.8、12.2）：説明に、書いた言語 `language` を足した（約束の版は 2 のまま）。例のアカウント、休暇、購買、契約の英語の版に `labels ja` を、その日本語の名前の版と経費の日本語の名前の版に `labels en` を足し、申請の名前、フィールド、選択の値、ステップ、承認者の決め方に、もう一方の言語の文言を付けた（経費の英語の版は前から持っていた）。英語の名前の例の日本語の画面は、日本語の文言で出る。ベクタは 1 本も変わらない。英語の題を日本語の文に入れるときの空白（DESIGN 13.4）は、例の画面では確かめられなくなったので、機械の試験の申請の画面で確かめる（`tests/a2ui.rs` の `an_english_label_in_a_japanese_sentence_is_set_apart`）。
- ベクタ（DESIGN 10.4）：例のベクタの本数は変わらない。答えが変わったのは D の `at` の 17 本（例の 12 本と機械の試験の 5 本）だけである。C.1 と 10 で変わったベクタは無い。
- 見つけて、直さずに避けたこと（直しのレーンへの知らせとして notes に書いた）：
  - rulec が Rust に書く、規則を取り込む規則のコードは、取り込んだ規則の出力を全部変数に置くので、経路が出力の一部だけを読むと、rustc の `unused_variables` で通らない。経路はそういう規則を `fed by` で読む。
  - kaname が Rust に書く `match v` / `none => c` / `some y => y` は、clippy の `manual_unwrap_or`（`c` が 0 なら `manual_unwrap_or_default`）で通らない。つなぎは、単位の無い数の仮の値を 0 でない値にし、`if` で範囲に収める形で書く。
  - rulec が Python に書く表は、行ごとに同じ変数に注釈なしで値を入れる。一行目が金額の入力で、二行目が `0JPY`（Python では `0`）の表は、mypy の `--strict` が、一行目の型の変数に `int` を入れると言って通さない（`Incompatible types in assignment`）。ringi の下ろす合計の表は、`0JPY` の行を先に書く。
  - rulec が、準用する規則の Ruby と Swift で組めないコードを書く。Ruby は、値が一つでラベルを持つ列挙のラベルを返す関数を、`when` の無い `case` で書く。Swift は、準用される規則と、その出力の列挙を書き写した `choices.rule` が、同じ名前の列挙（`AuthorityDecider`）をそれぞれ書く。C.1 で準用する規則が木に増えたので、rulec の `tests/apply_names.rs` が tools の段で落ちる。直しのレーンの T15b で直す。

## 28. フレームワークの段階 S6b で直したものと確かめたこと（2026-11-17）

フレームワーク（別のリポジトリ）の段階 S6b で、ringi の側の検査と `ritsu ringi init` を直し、`init` にオプションを二つ足した。ritsu の側の二つ（書いて残す物の言語と、sekisho の action の名前で尋ねる入口）は、根の PLAN 7.153 にある。

- **自分の入力を受け取る action を、チケットの操作に使う形**（X18、E911。DESIGN 9.3）：サーバーは gate に、人、resource、時刻のほかを渡さない。チケットの操作が尋ねる action が自分の入力（`input` の行）を受け取ると、その操作はどのチケットでも種類 `input` で拒まれる（sekisho の DESIGN 30 章）。`ritsu check` はこれを止めていなかったので、X18 の操作の境目に E911 として足した。文は入力を受け取る action と入力の名前を挙げ、直し方は、その action から `input` の行を外すことである。ritsu の `tests/cross.rs` に二つの形（入力が一つの action、二つの action。英日の golden）を足した。今の木のプロジェクトには当たる物が無い。
- **`ritsu ringi init` が書く `gate.go`**：action ごとの 12 の場合分けをやめ、sekisho の入口 `company.Authorize` に、チケット、ステップ、申請の種類の三つの ID を渡す形にした（85 行から 62 行）。ゲートに action を足しても `gate.go` は変わらない。ゲートに無い action は誤り（種類 `action`）として返し、ほかの断りは「許さない」とする。フレームワークの例の `gate.go` も同じ形にした（83 行から 60 行）。例の場合分けに無かった `change_request_type` も、ゲートが答えるようになった。`init` の golden（英日）を取り直した。
- **`init` の `--mcp` と `--temporal`**（DESIGN 16.4）：`--mcp` は、サーバーの `/mcp` にフレームワークのモジュール `mcp` のエージェントのツールを置く `tools.go` と、その試験を書く。経路はフレームワークの `Stores.Handle` で足し、人はページと同じサインインで見つける。試験は、だれもサインインしていない要求が 401 になることと、エージェントがツールで購買を申請して読めることを確かめる。`--temporal` は、承認されると flow `pay` で払い戻す二つ目の申請の種類 `expense.ringi` と `shared/pay.flow` を書く。あわせて、flow を始める所とワーカーを作る `flows.go`、flow のタスク `payments.go`（ログに書くだけ）、ワーカーのコマンド `cmd/worker`、Temporal の開発用サーバーの上で承認から支払いまでを動かす試験を書く。この試験は、開発用サーバーの CLI が無ければ理由を書いて SKIP する。サーバーは、Temporal に届かなければ 1 で終わり、30 秒のうちにワーカーが見つからなければログに警告を書いて動き続ける。二つを一緒に渡してもよい。オプションの無いときに書く物は、README の一文のほかは前と同じである。その一文は、`gate.go` が action の名前で尋ねる形になって誤りになっていたので直した。ひな形はオプションの組ごとに分けず一組にして、オプションが足す行と差し替える行を、`{{if mcp}}`、`{{if temporal}}`、`{{else}}`、`{{end}}` だけの行で囲む。
- **ルートのパッケージとそのファイルの名前**：パッケージのファイル `<パッケージ>.go` が `init` の書くほかのファイルと同じ名前になるとき（`…/gate`、`--mcp` のときの `…/tools`、`--temporal` のときの `…/flows`）と、Go が一つのシステムでだけ組む名前になるとき（`…/approvals-windows` の `approvals_windows.go`）は、`project.go` にした。名前が `_test` で終わるパッケージは `project` にした（Go がその試験のファイルを別のパッケージの試験として読み、組めない）。どれも、前は組めないプロジェクトを書いていた。
- **試験**：ritsu の `tests/ringi_init.rs` は、言語とオプションの組の八つを golden で押さえ、書く物が環境変数の言語に従わないこと、断る場合（`--mcp=yes`、`--temporal` を二度）とその文、ファイルの名前の決め方を確かめる。フレームワークの `scaffold` の `TestInit` は、八つの組で書いて組み、それぞれの試験を走らせる。

残したもの（フレームワークを公開する前に要ること）：`mcp` のモジュールの試験が `examples` のモジュールを読み込むので、`--mcp` のプロジェクトは、公開した版に対して `go mod tidy` ができない（`go mod tidy` は依存先のパッケージの試験も読む）。`mcp` の試験から `examples` を外すか、`examples` も同じ版で公開する。`init` の書く `go.mod` が要るフレームワークの三つのモジュール（根、`mcp`、`temporal`）には、同じ版のタグを付ける。

## 29. 断りの文の要素の番号、探す操作、一覧の続き、機械の文、カレンダーごとの時計で直したものと確かめたこと（2026-11-17〜18）

フレームワークの S5 からの頼み（B）と、前の段から渡された物（A、C、C2）を作った回である。直しのレーンを合わせたあとの試験で落ちた物（0）と、作る途中で見つけた不具合（保留の前に置かなかった催促）も直した。約束の版は 3 になった（DESIGN 12.4）。勧めの形で決めた所（★）とその理由、フレームワークの担当への知らせは notes にある。

- 合わせのあとの落ち（0）：
  - 直しのレーンの T14 で、rulec の計算の範囲の断りが要素の番号を言う文（`lines[0].fee`）になり、文を比べて断りを見分けていた参照インタプリタが、明細の行で計算するセルの断りを見分けられなくなった。文を比べるのをやめ、rulec の評価が返す構造を読む形にした。ritsu-ports に `Outside`（`define` の名前と、要素ごとの `define` なら並びの名前と要素の番号、合計と数なら出力の名前）と、文を組む `Outside::text`、既定を持つ `Rules::eval_outside` を足し、rulec の口は `eval` と `eval_outside` を一つの `Engine::answer` から答える。ritsu-cross の `run_breaks` も同じ構造を読み、文の比べ（`define_refused`、`define_refused_at`、`is_define_refused`、`summary_refused`）は無くした（rulec の DESIGN §15.324 の 10）。
  - T10c の rulec は、W201 の変異の材料（整数の上でだけ届かない値）を決められるようになった。材料を二乗の和の形に作り直し、ファイルの頭に、rulec が決められない理由（整数の上の線形の形でないこと）を書いた。
  - ritsu の golden `requests/a2ui/functions.js` を、始めの木のバイナリで作り直した（T14 の rulec の JavaScript の要素の言い方で、3 行）。ritsu の `tests/cross.rs` の二つの `repro_ja` を一つにした。
  - ritsu の `codes` の `the_checks_say_a_number_as_the_files_write_it`（X20 の E210・W210 が数を `_` で区切らない）は、S6b を合わせる段で直すので、この回では直していない。
- 要素の番号（A。DESIGN 12.4）：フォームの断りの文は、明細の行のセルを、場所 `at` と同じ番号で言う（`` `0JPY` is outside the range of `items[0].price` ``）。明細の行で計算するセルの範囲の断りは、場所を明細のフィールドの名前（`lines`）から範囲の外になった最初の行のセル（`lines[0].subtotal`）にし、文は申請の名前（日本語の名前の版では `明細[0].小計`）と範囲（数は T16 の書き方、単位はファイルの綴り）を言う。参照、四つの出力先のつなぎ、機械が、断りの種類、場所、英日の文まで同じ答えを返す（ritsu の `gen_requests`）。
- コメントの断り（B.1。DESIGN 7.2、12.4、13.8）：断りの種類に `comment` を足した（12 種類）。イベント `comment` の文が無いか空のときと、人の操作に添えた文が 2,000 字を超えるときの断りで、場所は null である。フォームに `comment` という名前のフィールドがあっても読み違えない。画面は、`/refused/comment` が真なら、コメントを書く部品の下に文を見せ、画面の頭には、フィールドの断りでもコメントの断りでもないときだけ見せる。`tests/a2ui.rs` は、参照がフォームかコメントで断るベクタのフォーム（例の英日で 62 個。うちコメントの長さの断りが 10 個）が、画面のどれかの検査（コメントの長さは `commentFits`）で止まることを確かめる。
- 探す操作（B.2。DESIGN 12.2、12.4、13.2）：人、チーム、マスタの行を探す操作（`find_people`、`find_teams`、`find_master_<マスタのファイル>_<マスタ>`。`GET /find/people` など）を約束に足し、MCP の道具は `readOnlyHint` を持つ。探すのは組織とマスタの全部を持つサーバーの仕事なので、四つの出力先には置かない。参照インタプリタ（`src/find.rs`）が答え、そのベクタ `organization/find.jsonl`（46 行）でサーバーを縛る。`--target requests` は、申請の種類のベクタ `vectors/` と並べてこのファイルを書く。比べ方（NFKC、`White_Space` の字を除く、ひらがなをカタカナに、一字ずつの単純な小文字と語末のシグマ）と並べ方（五つの順位と、同じ順位の中の値のコードポイントの順）は 13.2 に一度だけ書き、四つの言語の標準の関数と、そのままでは決まりと違う所を表にした。ベクタは、直す段の一つずつ、標準の関数が違って読む字（語末のシグマ、`İ`、`ẞ`、全角の英数字、`①`、`㈱`、半角のカタカナと濁点、U+0085・U+FEFF・U+001C）、順位と同じ順位の並び、`limit` の端と `more`、空の `q`、200 字と 201 字の `q`、`lang` の場合を一行ずつ持つ。
- 一覧の並びと続き（B.3、B.4。DESIGN 12.2、12.4、13.1、13.8）：同じ時刻のチケットは ID の大きい順（後の物が先）である。`list_waiting` と `list_mine` の答えに `more`（`limit` より多くあったか）を足し、道具の説明は、`more` が真なら最後のチケットの ID を `before` にして次のページを読むと言う。一覧の画面（`list`、`mine`）はデータに `/more` を持ち、真のあいだだけ、行の下に次のページのボタンを置く。ボタンは、ページの最後のチケットの ID（関数 `lastOf`）を `before` にして同じ一覧を読む。
- 機械の文（C。DESIGN 7.2、12.4）：機械（`src/lower/ticket.kn` とつなぎ）の断りの文を、参照インタプリタと同じ言葉にした（英日）。前の回に残した違い（人の名前、時計の時刻とステップ、古くなった操作のイベントの名前、終わったチケットが受け付けないイベント、取り下げられる期限のステップ、ステップの書いた名前など）は無くなり、突き合わせの試験から残す物の一覧を外した。試験は、全部のベクタの文を、四つの出力先と英日で参照と比べる。機械が文に要る物のうち、人の名前は組織の部分が渡し（`people` を `{id, name, name_ja}` の並びにした）、ステップの書いた名前はつなぎが渡す。
- 約束の版 3（DESIGN 12.4）：組織の部分の人の形、断りの種類 `comment`、明細の行で計算するセルの断りの場所、探す操作とそのベクタ、一覧の `more`、画面の置き場 `/refused/comment` と `/more`、ステップの `reminded` を、版 3 として書いた。`CONTRACT`、`openapi.json` の `x-ritsu-contract`、`types/<名前>.json` の `contract` が 3 を持つ。
- 時計を、数えるカレンダーのオフセットで（C2.1。DESIGN 5.6、5.7、7.5、9.6）：一つのイベントの時刻は一つで、カレンダーごとにその地の日がある。営業日は、カレンダーごとに、イベントの時刻をそのカレンダーのオフセットで読んだ日から数え、時計が鳴る時刻もそのオフセットで読む。保留の終わりと再申請の期限も同じである。組織の版と使える日の範囲は、申請の種類の日で読む。保留のあとの時計は、ステップが始まった時刻を、止めていた営業日が保留のカレンダーで何日にわたるかだけ後ろへずらし、ずらした時刻をカレンダーごとに読んで数える。断りの文が言う時計の時刻、E403 の数え方、使える日の範囲の端も、カレンダーのオフセットで言う。機械の試験の申請 `behind`（申請の日を東京で読み、催促を UTC から 5 時間遅れの工場の営業日で数える）と `ahead`（逆）を英日で足した。参照、四つの出力先のつなぎ、機械が同じ時刻を出す。`tests/machine.rs` の `each_clock_rings_in_the_offset_of_its_calendar` は、保留のカレンダーにだけある休み（工場の棚卸し）をはさむ保留も確かめ、`a_clock_counted_after_business_days_rings_after_its_event` は、`after N business day`（N は 1 以上）の時計がイベントより後に鳴ることを、性質として確かめる。
- 行の数が下限に届かないあいだの合計（C2.2。DESIGN 4.4、13.7）：`values.rule` の合計の範囲を、行の数が 0 から上限までのどれでも届く範囲で下ろす（経費は `>=1JPY` が `>=0JPY` になった。購買も同じ）。画面は、行の数が下限に届かないあいだも、埋まった行の合計を見せ、合計を読む条件はその値で決める。行の数の下限の断りと、計算の値を範囲に照らす検査は、行の数が下限に届くまで言わない（`check` の `filling`）。申請するときの判定と送る値は変わらない。`tests/a2ui/steps.rs` は、行が下限に届かない時点の合計と、それを読む条件も比べる。下ろしたフォーム（`form.rule`）がその時点を入口で断るとき（合計がその範囲の下限より小さい）は、画面が読む写し（`values.rule`）を参照の評価器で求めた答えと比べる（`engine::eval::values_outs`。比べる答えは 2,191、ほかの理由で比べないのは 12）。
- 口が先に尋ねる規則の言語（C2.3。根の DESIGN 4.15、rulec の DESIGN §15.320）：rulec の口は、覚えた報告が無ければ、規則をスレッドの言語で検査する（`Engine::rule_here`、`Engine::errors_at`）。日本語の実行で、経路が準用する規則を口が先に尋ね、そのあと規則自身の診断を日本語で見せても、規則の検査は一回である（rulec の `tests/ports.rs` と、一つの実行の口の組に、ringi の検査、規則の検査の順で尋ねる ritsu の `tests/once.rs`）。覚えた検査は、それを作った口の組の日付の口を鍵に持つので、実行を二つ並べた形では、言語によらず二回になる。根の DESIGN 4.15 の、rulec が覚える鍵の文を今の形にした。
- 保留の前に置かなかった催促（DESIGN 7.2、7.3、7.5、12.4）：ステップが、催促が鳴ったか（`reminded`）を持つ。始まったときと上へ回したときは false で、催促が鳴ると true になる。再開したステップと、`retract` で戻ったステップは、鳴っていない催促を、前に置いたかどうかによらず、期限より前に来るときに置く。前は、前に置いていた時計だけを置き直したので、期限より後に来るので置かなかった催促は、保留で期限がずれて前に来ても置かれず、機械は E604 で止まった（`ahead` で、本社の年末年始の休みの前の日に出して保留し、年始に再開する形）。カレンダーのデータは、申請の種類の版を上げずに書き換えられる（7.7）ので、置かなかった理由を数え直さず、チケットに持たせた。四つの出力先は `record Stage` からキーを読み書きし、つなぎは時刻を数えるだけなので変わらない。`ahead` に四つのシナリオ（年始のあとに置く催促、二度目の再開で置く催促、二度の保留のあいだに鳴った催促、再開したときにもう過ぎていた期限。英日）を足し、`a_stage_says_whether_its_reminder_has_rung` が各行の催促、キー、期限、止めた日数を確かめる。カレンダーを書き換える前と後の申請（`tests/rewritten/`）で、鳴った催促は二度鳴らず、置かなかった催促は置くことを、`a_reminder_rings_once_when_the_calendars_are_written_again` が確かめる（英日）。
- ベクタ（DESIGN 10.4）：始めの木のバイナリが書いたベクタ（例と機械の試験の 63 のファイルの 7,999 本、契約の例の 882 本）は、組織の部分の人の形と `reminded` のほかは、134 本の答えだけが変わった。コメントの断りの種類（`form` が `comment` に）が 112 本（例 32 本、契約 8 本、機械の試験 72 本）、明細の行の断りの文が 17 本（例 12 本、機械の試験 5 本）、計算するセルの断りの場所と文が 5 本（購買の英日の 2 本ずつ、`ranges` の 1 本）である。足したのは `behind` と `ahead`（英日）の 894 本である。`reminded` を足す前と後のバイナリでは、どのベクタも、キーを除けば一字も同じで、キーが答えを変えるのは `ahead` の新しいシナリオの 10 本（英日 5 本ずつ）だけである。
- 変異（どれも、試験が落ちたのを見てから戻した）：口が構造を返さない（`machine` の `the_ends_of_a_range_are_read_alike`）、コメントの断りを画面の頭に出す（`a2ui` の `a_refusal_is_shown_beside_what_it_is_of`）、次のページのボタンが最初のチケットを読む（`a_full_page_reads_the_next`）、比べ方からひらがなの段を外す（`documents` の `the_vectors_of_finding_answer_as_the_comparison_says`）、長いコメントを `form` で断る（`vectors` の二つ）、参照が全部のカレンダーを申請の種類の日で数える（`machine` の三つ）、つなぎが同じことをする（`gen_requests` の機械の突き合わせ）、画面が行の数の下限まで合計を待つ（`a2ui` の `steps`）、機械の終わったチケットの文を前の形に戻す（`gen_requests` の機械の突き合わせ）、参照が `reminded` を読まず前に置いたかで決める（`machine` の四つ）、参照が鳴った催促も置き直す（`machine` の五つ）、機械が `reminded` を読まない（`gen_requests` の機械の突き合わせ）、口が覚えた報告の無い規則を英語で検査する（rulec の `ports` の二つ）。
- 見つけて、直さずに残したこと：ritsu の `package.rs` の `REQUESTS` の説明は、`--target requests` が書くファイルとして `vectors/` だけを挙げる。S6b を合わせる段が触るファイルなので、その段で `organization/find.jsonl` を並べて書く（文の案は notes）。


## 30. 段階 F2（ページ）で作ったものと確かめたこと（2026-11-18）

F.1 のページを作り、`ritsu mod diff` と `ritsu impact` に `.ringi` を入れた。あわせて、どの言語でも、インターフェースの差から、ものを書いた場所（行と、モジュールの根から書いたパス）を除いた（根の DESIGN 17.9.5）。

- **口の `Rules::diff`**（`ritsu-ports` の `rules.rs`、rulec の `ports.rs`）：規則の二つの版を、側ごとに自分のファイル（Overlay、git の版、置き場）で読み、`rulec diff` と同じ歩き方で比べた区間を返す。`rulec diff` の出力は変えていない。rulec の `tests/ports.rs` が、コーパスの規則ごとに `rulec diff` の JSON と突き合わせる。日付の側には同じ口を足さない。期限の変わりは `.ringi` に書いた日数で言え、カレンダーの変わりは koyomi の diff が比べるからである。
- **`ritsu ringi doc`**（DESIGN 11.1、11.3、11.6）：申請の種類のページ（Markdown と HTML、英日）、`--longest`、共有の経路と組織とマスタのページ。共有の経路のページは、プロジェクトの中でその経路を使う申請の種類を、題と名前とルートからのパスで並べる（リンクは無い）。何を読むかは、プロジェクトの索引と同じ `ports::uses_of` で読む。`--longest` は申請の種類だけが受け、ほかのファイルには 2 で断る。組織のページが書く人の情報は、名前、役職、チーム、代理だけにした。`tests/doc.rs`（12 件）と golden 52 枚。
- **`ritsu ringi diff`**（DESIGN 11.2）：経路は、下ろした `route.rule` を `Rules::diff` で比べ、区間をフィールドの題と値で、ステップを決め方の文で言う。承認者は、組織の版ごとに、全部の申請者の `simulate` で比べる。ほかは、ページの節ごとに比べる。足したステップは、歩き方が宣言の違いとしてしか言わないので、二つの経路を逆の向きにも比べて言う（この段階の中で見つけて直した）。進行中のチケットへの効き方は、ファイルの種類ごとに言う。チケットが持つのは申請の種類の版だけなので、共有の経路の変更には使う申請の種類の版を上げるように言い、組織とマスタの変更は次に始まるステップから効くと言う。JSON の `applies_to` も同じことを言う（この段階の中で、組織のファイルにも自分の版を上げるように言っていたのを見つけて直した）。`tests/diff.rs`（10 件。九つの変え方を、境目の両側と範囲の端で `simulate` と突き合わせる）と golden 26 枚。
- **`--versions`**（DESIGN 11.4）：一つの申請の種類を、二つの日に効く組織の版で比べる。
- **`ritsu mod diff` と `ritsu impact`**（DESIGN 16.3、根の DESIGN 17.9.5、17.10.1）：振る舞いの差は ringi の `diff` をこのプロセスで尋ねた答えで、どれかのファイルが変われば、両方の版の `.ringi` を全部比べる。インターフェースの差は `ringi api` の JSON で比べ、中の物は種類と名前で突き合わせ、例とシナリオと主張は比べない。`ritsu impact` に事実 `request_changes` を足した（27 個目。`.ringi` が無ければ 0）。
- **api の `where` と `none`**（DESIGN 16.2）：文に行を入れず、文が指す経路の行を `lines` に分けた。言語をまたぐ検査の文は、今までどおり文と行を合わせて書く（ritsu の `tests/cross.rs` の golden は変わらない）。口の `FlowGiven` の `none` と `narrowed` は、文と行と合わせた文を持つ `Lined` になった。
- **書いた場所を除く**（根の DESIGN 17.9.5）：sekisho、kaname、sakai、ringi の api から、行とモジュールの根から書いたパスを除いて比べる。sakai のコンテキストは、それを使う地図の api で比べ、注もそう言う。ほかの言語の例と試験の材料で、検査を通るのに api を持たないファイルが無いことを確かめた。

直したもの：組織のページが、代理のファイルを、走らせた場所からのパスで書いていた（`../org/proxies.csv` の形に直した。`tests/doc.rs` が、四つのファイルを三つの渡し方で確かめる）。`ritsu mod diff` が `diff` のページの Markdown を頭のコメントごと入れていたので、コメントを除いて見出しを二つ下げた形（`markdown_held`）にした。コメントはモジュールのファイルの名前の `@` をコードの外に置き、モジュールの一覧のサイトでは文字のまま出ていた。日本語のページで「チーム取締役会（`board`）の人」と、「チーム」のすぐあとに名前を続けていたのを、「取締役会（`board`）の人」にした。

試験：ritsu の `tests/modules_diff.rs` に 10 件（申請のモジュールの二つ、言語ごとに上に一行足した形と一つ深いディレクトリへ移した形の七つ、sakai のコンテキストの一つ）、`tests/impact.rs` に 1 件、`tests/modules_report.rs` に 1 件（`ritsu mod report` は同じ関数で振る舞いを比べるので、変わったモジュールの組織を読む手元の申請も比べる。依存の組織のデータだけが変わり、手元の申請の承認者が変わる形）。impact の golden は、事実の行が一つずつ増えただけである。

残したもの：例の経路の規則（`crates/ritsu/examples/impact/route.rule`）は `request_changes` を読まない。根の DESIGN 17.9.6 の「振る舞いを比べるファイル」と 17.10 の 3 の文は ringi を挙げていない。どれも根の文書と一緒に直す（notes に文の案）。
## 31. フレームワークの頼み（S6d、S6e）、数の書き方、理由を求める操作、人の言語、管理の機能のための頼みで作ったものと確かめたこと（2026-11-18）

フレームワークの S6d と S6e からの頼み（A〜E）、使う人に見える日本語の文の数の書き方（F）、持つと決まった二つ（H：却下と差し戻しに理由を求める行、I：人の言語）、管理の機能のための頼み（J）を作った回である。約束の版は 3 のままで、機械が受け取る申請と断る申請は、H を書いた申請の種類のほかは変わらない。勧めの形で決めた所（★）とその理由、フレームワークの担当への知らせは notes にある。

- 途中のフォームの答え（A。DESIGN 10.4、12.2、12.4、13.7、16.1、16.4）：申請の種類の一覧の各種類に、三つ目の JSON の入口 `worked`（Go は `Worked`）を足した。途中まで埋めたフォームと承認者の直し（`{"form": …, "edited": … | null}`）を受け、条件つきのフィールドを見せるか、必須か、計算するフィールドの値（明細の行の計算は行ごと）を `{"fields": …}` で答える。決まらない所は null である。決め方は画面と同じで、画面の求めない値（明細の行の中の計算、計算する日付）は機械と同じに求める。読む明細の行が上限より多いときだけ画面と分かれ（画面は全部の行を足し、`worked` は決まらないと答える。画面を直すのは F3）、行が下限より少ないときは画面と同じく、ある行から求める。参照は `engine::worked`、生成するコードはつなぎの `worked_out` である。何も計算せず、条件も無い種類は `worked_none` で、答えは `{"fields":{}}` である。入口のエラーの文は、関数の名前を `worked_out` で言う。スキーマは `openapi.json` の `<名前>.worked.in`・`<名前>.worked.out`（型だけで、どの値も null を許す）。ベクタは `vectors/worked/<名前>.jsonl` で、チケットの機械のベクタと同じディレクトリに置かない（サーバーの試験が `vectors/*.jsonl` を種類のベクタとして読むため）。種類を外すと、`ritsu gen` がそのベクタも消し、`--check` は `left` と言う。`worked` の七つの試験が参照の答えを場合ごとに確かめ、四つの出力先は全部のベクタで参照と同じ答えを返し（`gen_requests`。Rust の一覧は、大文字のある種類の名前の `worked` の関数にも `#[allow(non_snake_case)]` を付ける）、画面の関数も、行が上限より多い 22 のフォームを除く 593 のフォームの 1,750 の答えで同じである（`a2ui` の `worked`）。機械の申請で、kaname の Go と Rust の書き出しの形とつなぎの形の組み合わせで、生成したコードの検査が二つ落ちたので、つなぎの側で避けた（率の仮の値を `let` に置く、真偽の読み方を比べにする。kaname で直すのはあとの段）。約束の版は上げていない（足しただけで、前のサーバーが読み違える物は無い。Go の一覧の `Kind` は形が変わるので、形で受けるフレームワークはコンパイルで気づく）。
- 途中のフォームを尋ねる操作（A。DESIGN 12.2、12.4、13.6）：`openapi.json` と `mcp/tools.json` に、読むだけの操作と道具を二つ足した。`POST /requests/<名前>/worked`（`worked_<名前>`）は、新しい申請が取る今の版の決まりで、認証した人のだれにでも答える。答えはチケットも組織も読まず、同じ決まりは画面の `functions.js` が見る人のブラウザにもう渡しているからである。`POST /requests/<名前>/tickets/{id}/worked`（`ticket_worked_<名前>`）は、出し直しか承認者の直しのある種類だけにあり、そのチケットを出した版の決まりで、チケットを見られる人に答える。差し戻したチケットの出し直しと承認者の直しは、チケットを出した版の決まりで決まるためである。チケットからは版だけを読み、チケットの中身を答えに出さない。どちらも本文は入口 `worked` の入力、200 は答え、400 は新しい応答 `unworked` で、冪等キー、版、コメントを取らない。道具は `readOnlyHint` を持ち、版を選ぶ引数は無い（どの版で答えるかはサーバーが決める）。`documents` の `what_a_form_works_out_is_read_alone` と、`gen_requests` の `the_api_requires_what_the_entries_stop_without`（本文の一か所を変えたとき、スキーマが取る物は入口が答え、取らない物は入口も止める。17 の操作の本文の 234 の変え方）が確かめる。
- 約束のエラーの答え（A。DESIGN 12.2、12.4、14.2）：操作ごとに、フレームワークが返す答えを全部書いた。どの操作も 401（`unsigned`）と 500（`failed`）、本文を取る操作は 413（`oversized`）と 415（`not_json`）、一覧とマスタの行と探す操作は 400（`unasked`）、組織は 404（`no_organization`）、マスタの行を探す操作は、どの種類もそのマスタから選ばなければ 404（`unchosen`）である。種類ごとのチケットの操作（`resubmit_`、`edit_`、`ticket_worked_`）は、パスの種類と違う種類のチケットにも、チケットが無いときと同じ 404（`unseen`）で答える。外から見れば、どちらも見られるチケットが無いということなので、応答は一つにし、その文が両方を言う。`submit_` と `worked_` は、その日に効いている版が無ければ 404 で答える（`unoffered`。サーバーは先に gate に尋ね、出せる人にだけ、次の版が始まる日 `from` を言う）。断り（409）のほかのエラーの本文は、どれも `{"error": "…"}`（共有のスキーマ `error`。閉じた形）で、`unoffered` だけが `from` も持つ。操作ごとの答えは、コードの小さい順に並べた。`documents` の `an_error_answers_its_text_in_one_shape`、`each_404_says_what_is_not_there`、`the_lists_come_in_pages_and_the_answers_are_said`、`people_teams_and_rows_are_found_by_words` が確かめる。ritsu の X20 の E210 の注（今の API の、フォームを受け取る操作）には、`x-ritsu-request` を持つ worked の口も並ぶ（golden の英日の一行ずつ）。
- 種類の一覧の言語と題（B。DESIGN 12.4）：`openapi.json` の `request_kind`（`list_requests` の答えの一つずつ）に、種類の書いた言語（`language`。`en` か `ja`）と、ほかの言語の題（`labels`。`types/<名前>.json` と同じ形）を足し、どちらも `required` にした。サーバーは、見る人の言語の題を一覧から選べる。`documents` の `a_kind_in_the_list_says_its_language_and_its_titles` が、スキーマと、例の全部の種類の説明が持つ値を確かめる。
- 直すフィールド（C。DESIGN 4.4、12.4、13.7）：計算した値が範囲の外のときの断りと、画面の検査の文は、範囲を言ったあとに、申請者が直すフィールドを挙げる。挙げるのは計算が読むフィールド（種類の説明の `reads`）で、どれも同じ明細のセルなら明細の名前を一度だけ添える。機械の断り（参照と四つの出力先のつなぎ）は、ファイルが書いた名前で言う（「計算した `明細[0].小計` が、範囲（`>=1円 <=100_000_000円`）の外です。`明細` の `数量` か `単価` を直してください」）。画面は見る人の言語の題で言い（「明細の数量か単価を直してください。」）、サーバーのページが機械の断りを言い直す文と同じである。参照とつなぎは一つの関数（`lower::form::out_of_range_said`。名前の並べ方は `fix_said`）で文を作る。`machine` の `the_ends_of_a_range_are_read_alike` と、ベクタの文が機械の断りを、`a2ui` の `a_computed_value_outside_its_range_names_the_fields_to_change` が画面の文（購買の英日、経費の英日）を確かめる。
- ほかのファイルから来る文言の言語（D。DESIGN 4.8、5.11、12.2、codes の E703）：規則の列挙から選ぶフィールドは、値ごとの題を、`choice` の値と同じにフィールドの下に書き、ほかの言語の文言は `labels` の `<フィールド>.<値>` に書く。題を書かない行は名前をそのまま題にする。共有の経路のステップと決め方は、使う申請の種類の `labels` に無い言語の文言を、経路の `labels` から取る。ファイルの言語の題が無い項目（題の行の無い列挙の値と、申請の種類と違う言語で書いた共有の経路で、経路の `labels <種類の言語>` に題の無いステップと決め方）は E703 にした。項目ごとに言語の印を持たせる形は作らない。規則が持つ値の題を口で運ぶのは、rulec の生成器に触るので、あとの段に渡した。例の購買（英日）は `kind` の値の題を持つ。`titles`（新しい試験）と `items` が、題、`labels`、E703、E004、E001 を確かめる。
- 通知で種類を題で言う（E。DESIGN 16.4）：`ritsu ringi init` が書く `notify.go` は、フレームワークの `Server.Title` で、通知を書く言語の題と、その題の言語を書く。サーバーができたあとで通知の部品にサーバーを渡す `Started` を `<パッケージ>.go` に足し、`cmd/server` と試験が呼ぶ。`tests/golden/ringi-init/` を作り直した。
- 数と助数詞のあいだ（F。DESIGN 7.2、12.4）：機械の断りの日本語は、数と、そのあとの助数詞・単位のあいだを空けない（「コメントは2001文字で」「1回目、2番目のステップ、直し0回」「`明細` は21行で」「合わせて2回まで」「合わせて5営業日まで」「`note` は25文字で」）。英字の名前と仮名・漢字のあいだの空白は今のまま。画面は前から空けていない。開発者に向けた文（検査の診断、`simulate` と `run` の出力、文書）は今のまま。参照、四つの出力先のつなぎ、機械（`ticket.kn`）で同じ文である。
- 理由を求める操作（H。DESIGN 3.3、7.2、10.4、12.2、12.4、13.1、13.2、13.7、codes の E409）：`ticket` の行 `comment required  for reject, send back`（挙げるのは `reject` と `send back` の片方か両方）。書いた申請の種類では、理由の無い（コメントが無いか、空か、Unicode の White_Space だけの）却下と差し戻しを、機械が `comment` の種類で断る（場所は null）。確かめるのは、コメントの長さのあとで、だれがする操作かより先である。種類の説明は、ステップごとに `comment_required`（約束の操作の名前）を持ち、API の `comment` の説明と、MCP の `reject`・`send_back` の道具の説明もそれを言う。承認カードの却下と差し戻しのボタンは、送るコメントが無いあいだ、送らずに「理由をコメントに書いてください。」を見せる（`commentGiven`）。画面がコメントと文のフィールドの両端の空白を除くのは、JavaScript の `trim` をやめて、機械と同じ White_Space の 25 文字にした（`trim` は U+FEFF を除き、U+0085 を残すので、理由の判定が機械と分かれる）。同じ行を二度書けば E006、同じ操作を二度挙げれば E001、チケットの決まりで起きない操作を挙げれば E409（新しい。台帳は 62 個）である。例の休暇（英日）に行を書き、理由を添えて却下するシナリオを一つ足した。機械の試験の申請 `reasons`（英語。新しい）を足し、`machine` の `a_reject_and_a_send_back_need_a_reason`、`vectors` の `a_reason_asked_is_in_the_vectors`、`documents` の `a_kind_says_the_events_it_asks_a_reason_with` が確かめる。四つの出力先は、`gen_requests` が `reasons` と休暇のベクタで参照と比べる。ベクタは、理由を求める操作を、コメント無し、空、空白だけ、U+0085 だけのコメントで行って断られる物と、空白でない一文字（U+FEFF と U+001C。各言語の `trim` や `strip` が、どちらかを空白として除く）で行って受け付けられる物を持つ。`a2ui` の `a_reason_asked_is_held_by_the_buttons_of_the_card` がカードのボタンを確かめ、`the_functions_answer_the_vectors_as_the_reference_does` が、理由を求める操作のベクタのコメントで `commentGiven` が機械と同じに答えることを確かめる。
- 人の言語（I。DESIGN 8.2、9.6、12.2、12.4）：組織のデータの人に、知らせと画面を読む言語（`language`。`en` か `ja`。空なら申請の種類ごとにその種類の言語）を持たせた。`people.csv` の列と `organization.json` のキーで、ほかの値は E501 である。組織の説明（`organization/schema.json`、`columns.md`）と、`openapi.json` の `person`（`get_organization`）にある。機械に渡す組織の部分には入れない（機械は人の言語を読まない）。例の組織の 6 人に書いた。`org` の `a_persons_language_is_en_ja_or_none` が確かめる（英日）。
- 管理の操作を求める E911（J。DESIGN 9.3、14.3）：フレームワークの管理の機能が gate に尋ねる操作のうち、まだ求めていなかった三つ（`read_audit`、`import_organization`、`read_organization_audit`）を、X18 の操作の境目に足した（`ticket.rs` の `operations`、ritsu-cross の `ticket_gates.rs`）。管理の機能は、申請の種類の版の始まりを決めること、組織のデータの取り込み、二つの記録を読むことである。文はチケットの操作とサーバーの管理の操作を分けて挙げ、管理の操作の注は、四つの操作とその resource を言う。ritsu の `tests/cross.rs` に三つの形（英日の golden）を足した：管理の操作が三つ無い形、チケットの操作と管理の操作が一つずつ無い形、入力を受け取る管理の操作の形。今の X18 の golden 24 個は、gate の要約の行の action の数だけが変わった。例と `init`（英日）の gate には、役割 `system_admin` と `auditor`、resource `Organization`、三つの action、permit 三つ、期待四つ（例は五つ）を足した。部門の管理者がほかの部門の版を変えない期待には、全体の管理者を除く行を足した。例の検査の診断は、E911 のほかは古い gate と同じである。ritsu のスキル（`skills/ritsu/SKILL.md`）の E911 の行にも、管理の操作を足した。DESIGN の 14 章は、管理の画面、版の始まりの四つの決め方、組織の取り込みの二通り、四つの操作と例の gate の役割、部門の比べ方で書き直した。
- `init` の `gate.go` と gate の試験（J。DESIGN 12.4、14.3、16.4）：`gate.go` で直した不具合は三つである。組織に居ない人が、持ち主の無い種類のチケットを `see` で読めた。部門の種類が `"department"` の決め打ちで、課が受け持つ種類と、種類を書いた名前で書いた組織で一致しなかった。宣言していない役割を一つでも持つ人が、どの操作でも断られた。いまの `gate.go` は、組織の ID を渡す。その日の組織の版に主の所属の無い人は、gate が読まない。持ち主の無い申請の種類には、その版のどのチームとも違う ID（`(no owner)`）を渡す。人の部門は、持ち主のチームの種類で `TeamOf` から引く。gate が宣言した役割だけを残して尋ね、外した役割は名前ごとに一度（100 個まで）ログに書く。一つも残らなければ `staff` で尋ねる。宣言した役割の一覧は、sekisho の `Roles()` が入るまで `gate.go` の中に書いた。`TestGate` に、管理の四つの操作を役割と部門で、組織に居ない人と持ち主の無い種類（`(no owner)` という ID のチームがある組織も）、持ち主が課・部・本部のときの比べ（宣言した名前と書いた名前の組織）、宣言していない役割とログの上限の場合を足した。README（英日）に、動いているサーバーへ組織の新しい版を上げて取り込む手順を足した。`init` の golden 8 個を取り直した（E の差と一緒）。`FRAMEWORK_VERSION` の説明の古い一文を直した。`init` のプロジェクト全体（英日）で `go vet` と `go test` が通る（`TestVectors` 189/189、`TestPages`、`TestGate`）。ただし、フレームワークの今の木はまだ `Worked` を受けないので、生成した `Kind` から `Worked` を外して確かめた。
- ベクタ（DESIGN 10.4）：始めの木のバイナリが始めの木の申請から書いたベクタと、終わりの木のバイナリが書いたベクタを、行をそろえて比べた（例と機械の試験の 33 のファイルの 9,950 本）。受け付ける申請と断る申請は、H のほかは変わらない。答えの文が変わったのは F の 748 本（例 232 本、機械の試験 516 本）と C の 10 本（経費の英日の 1 本ずつ、購買の英日の 3 本ずつ、`ranges` の 2 本）である。休暇（英日）は 179 本が 192 本になった。足したのは、理由を求める操作の 12 本（却下と差し戻しを、コメント無し、空、空白だけ、U+0085 だけで行って断られる 8 本と、U+FEFF だけ、U+001C だけで行って受け付けられる 4 本）と、足したシナリオの 1 本である。ほかに、行を 2 行足したので `from` の行の番号が変わり、ベクタの却下と差し戻し 16 本が理由を持つ（答えの履歴の行の `comment` も）。購買（英日）は、値の題の 3 行で `from` の行の番号が変わった。機械の試験の申請 `reasons` のベクタ 124 本は新しい。
- 変異：変えた所ごとに、試験が落とすことを変異で確かめた（35 個。どれも落ちるのを見てから戻した）。`worked`（参照が隠れたフィールドを値のまま読む、申請者の入れるフィールドが空でも求める、つなぎが上限より多い行も求める、ほか）と二つの口と約束のエラーの答え（口を書かない、説明からだれに答えるかを外す、種類を外してもベクタを消さない、道具に読むだけの印を付けない、`edited` も求める、404・400・500 を書かない、`unoffered` の文から gate を外す、`unread` に本文を書かない）が 17 個、チケットの 404 の文から種類違いのチケットの場合を外す変異（`documents` の `each_404_says_what_is_not_there`）、計算した値の断りと画面の検査の文が直すフィールドを言わない変異（`machine` の `the_ends_of_a_range_are_read_alike`、`a2ui` の `a_computed_value_outside_its_range_names_the_fields_to_change`）、コメントの長さの断りで数の前後の空白を戻す変異（`machine` の `what_a_comment_is_refused_for`）、ほかのファイルから来る項目の題が無いことを言わない変異（`mutants` の二つ）、参照が理由を確かめない変異（`machine` の `a_reject_and_a_send_back_need_a_reason`）、カードのボタンが理由を確かめない変異（`a2ui` の `a_reason_asked_is_held_by_the_buttons_of_the_card`）、画面の空白の除き方を JavaScript の `trim` に戻す変異（`a2ui` の `the_functions_answer_the_vectors_as_the_reference_does` ほか）、人の言語を `en`・`ja` に限らない変異（`org` の `a_persons_language_is_en_ja_or_none`）と組織の説明から外す変異（`documents` の `a_person_says_the_language_they_read_in`）、J の 8 個（`ticket.rs` から `read_audit` を外すと `cross` の X18、`init` の英語の gate から `read_organization_audit` を外すと `ringi_init`、`gate.go` の六つは `TestGate` の小試験）である。
- 見つけて、直さずに残したこと：
  - 規則の列挙の値の題を口で運ぶこと（`ritsu_ports::EnumValue` が題を持たない）。rulec の生成器に触るので、あとの段に渡した（DESIGN 20.2）。
  - 読む明細の行が上限より多いとき、画面の関数は全部の行から求め、`worked` は決まらないと答える。画面を `worked` にそろえるのは F3 である。
  - 機械の申請で、kaname の Go と Rust の書き出しの形が、つなぎの形との組み合わせで生成したコードの検査に落ちる二つ。今はつなぎの側で避けている。kaname を直すのは別の段である。
  - `init` の `gate.go` は、宣言した役割の一覧を自分の中に書いている。sekisho の `Roles()` が入ったら、それを呼ぶ形に替え、`init` の golden を作り直す。
  - フレームワークは、まだ `Worked` の入口、二つの worked の口、種類違いのチケットの 404 と、`unoffered` の gate と `from` を持たない。フレームワークの段（S8）で足す。
  - 画面が数を書いた文字を数に直す所（`functions.js` の `wire` の数のフィールドと `numberFrom`）は、両端の空白を今も JavaScript の `trim` で除く。文とコメントは White_Space の 25 文字にしたので、画面の中で空白の読み方が二つある。機械は数を JSON の数で受けるので、機械と分かれる所は無い。違うのは、端の U+FEFF（`trim` は除き、White_Space は除かない）と U+0085（その逆）だけである。そろえるなら `trimmed` を使い、数の欄の二つの場合の試験を `a2ui` に足す（F3 で画面の関数を直すときに）。

## 32. フレームワークの段階 S8 からの頼み（頼み 4）で直したものと確かめたこと（2026-11-18）

フレームワークの S8 からの頼み（1〜13 のうち、2 と 12 を除く）を作った回である。約束の版は 3 のままで、新しく断る組織のデータ（次の段の一覧）と、探す操作の 404 と、申請を開く flow の `key` が変わった。新しい番号は無く、全部 E501 の中である。勧めの形で決めた所としない頼みの理由、フレームワークの決まりとの対応の表は notes（`reports/ringi-asks4-notes.md`）にある。

- 探す操作の 404（頼み 1。DESIGN 12.4）：`find_people`・`find_teams`（`POST /find/people`・`/find/teams`）の約束に 404 `no_organization` を足した。今日の版にいない人、効いている版が無いときである。`openapi.json` と `mcp/tools.json` の両方に入り、`documents` の試験と `requests` の golden が確かめる。
- 人の言語（頼み 3）：`language` は前の段で読まれていて（`en`・`ja`・空。大文字小文字を区別し、`EN`・`ja-JP` は断る）、この段は確かめただけである。
- 組織のデータの確かめ（頼み 4〜8。DESIGN 8.4）：フレームワークの `orgcheck.go`・`orgread.go` が断る物を全部断るようにした。足したのは、アドレスの形（254 バイトまで、空白を落としたあと `@` が一つで前後に文字があり、空白と `"(),:;<>[\]` を含まない）、同じ版の二人のアドレスのたたんだ同値（空白を落として `strings.EqualFold` 相当。U+212A＝`k`、U+017F＝`s`）、ID・名前・アドレス・マスタの値の Cc・Cf・Zl・Zp（ZWJ・ZWNJ・U+FEFF も）、名前 200 字・マスタの値 1,000 字（コードポイント）、ID とマスタのキー 255 バイト・マスタのキーの空、`organization.json` の最上位と配列の要素の中の知らないキー（大文字小文字の違いも）、列とは違う個数の値を持つ行（足りない行も）、行 1 MiB・値 1,000 個、引用符で囲んでいない値の中の引用符と閉じた引用符のあとの文字、JSON の先頭の BOM を除くことである。重複のキーは JSON のパーサーが先に断る（「JSON として読めない」）。診断に出す ID とアドレスと名前は、フレームワークの `escaped` と同じに、制御文字と書式の文字を `\uXXXX` にして出す。
- `init` の main（頼み 9。DESIGN 16.4）：`cmd/server/main.go` のひな形は、起動を止めたエラーを `server.LogStop(log, "the server stopped", err)` で一行ずつログに出す。`ringi-init` の golden 8 個を取り直した。
- 問題の数（頼み 10）：`ritsu ringi check` は問題を 1,000 件で止めていない。1,001 個の問題を持つ材料で全部出ることを `org` の `every_problem_of_the_data_is_said` が確かめる。
- `page.js` の写し（頼み 11）：ritsu の側に写しは無い。A・B の変更を写す対象が無いことを notes に書いた。
- 申請を開く flow の `key`（頼み 13）：`POST /requests/{kind}/tickets` を呼ぶ `http` のタスクに `key` を書いた（`gen_requests` の `TRIP_FLOW` とそれから作る wide の flow、ritsu-cross の X20 の英日の flow、codes.md・codes.ja.md の断片、DESIGN の X20 の節）。約束が必須のヘッダー `Idempotency-Key` を求めるので、dandori の `key` がそれに入る。渡さないタスクを E016 で止める形は直しのレーンである。
- 試験：`org` に、版の人に列を足す `people_with` の道具を足し（列とは違う個数の行を断るので、試験の材料は列をそろえる）、確かめごとの通る材料と断られる材料（英日）を足した。`mutants` に、JSON の知らないキーを持つ版の組織のデータを足した（英日の変異で落ちる）。
- しない頼み：2（約束を変える形。S8 の ★B7 で、約束を変えない形が勧めとして決まった）、12（お知らせの画面の雛形。最後の合わせの控え）。

## 33. 段階 G で作ったものと確かめたこと（2026-11-19）

チケットの機械を Lean 4 でモデルにして証明し、Rust の参照インタプリタと突き合わせる段（10 章）である。モデル、定理、突き合わせ、証明の組み込み、文書を揃えた。勧めの形で決めた所（★G-1、★G-2）とその理由は notes にある。

- **モデル**（`proofs/RingiCore/` の九つ。DESIGN 17 章）：`ticket.kn`（12.1）を関数ごとに写した。型（`Types`）、言語の組み込みの写し（`Prelude`）、レコード（`Records`）、日付と組織（`Org`）、ステップ（`Stages`）、一つのイベント（`Moves`）、つなぎ（`Glue`）、ベクタの読み書きと一行の答え方（`Json`、`Run`）。申請の種類のコードが答える物（時計、営業日、`on` の条件と引数、フォーム、flow へ渡す値）は関数として受け取る（`Oracle`）。フォームとイベントの値は JSON のまま持つ。`sorry`・自前の `axiom`・`native_decide`・`admit`・`implemented_by` は無い。
- **定理**（`Theorems/` の五つ。DESIGN 17 章）：`Next`（終わったチケットはどのイベントも断る、鳴った時計は時計でないイベントより先 `next_rung_first`、コメントは何も変えない、古い版は `stale`）、`Climb`（組織を 30 チームまでたどり、見つける人は外される人ではない）、`Approvers`（自己承認を許さない経路で申請者と代わりに出した人は承認も却下も保留もできない）、`Quorum`（議決は答えについて単調、`needed` は百分率が負でなければ承認者の数で単調、取り消しが消す答え、`at the last stage`、1,000,000 ステップ以下の前提）、`Holds`（仮押さえが行にそろっていれば、どの終わり方でも確定か取消で閉じる。断りが伝わるのは常に読む `hold` と `launch` の行に限る、終わり方の行に `hold` が無い前提）。始めの PLAN に挙げた六つのうち 2 は成り立たないことが分かり、成り立つ形（鳴った時計が先・コメント不変・古い版を断る、時計のイベントが進めることは未証明）に書き直した（★G-2、DESIGN 17 章の「終わること」）。
- **突き合わせ**（`crates/ritsu-model/tests/ringi.rs`、参照の読む口 `crates/ringi/src/model.rs`）：例五つ（英日）と機械のための申請の全部のベクタ（36 申請、10,428 行）を両方に流して答えを一行ずつ比べ、差 0。モデルは外に置いた物を `{"need": …}` で尋ね、試験が参照インタプリタの関数で答えて渡し直す。比べるのはチケット全部・することの一覧全部・断りの種類で、断りの文の中身は比べない。
- **変異**（13 種類。`mut/summary.txt`）：10 種類は突き合わせが落ちた（議決の数のずらし二つ、差し戻しの上限のずらし二つ、保留の合計を数えない、仮押さえを閉じない、版を見ない、代理の行を書かない、承認を取り消せる間、上へたどる深さを一つ浅くする）。残る三つは、どの入力でも見える答えが変わらない壊し方：鳴った時計の順と同時刻は、断りの文に出す時計を選ぶ `rung_first` だけを壊し、文は比べない（時計のイベントはその選びを見ない。届く状態で鳴って未処理の時計は多くて一つ）。深さを一つ深くする方は 31 番目のチームでしか効かず、正しい組織は 30 チームまでしか上を持てない（E501）。
- **組み込み**：`proofs/lakefile.toml` の `defaultTargets` に `RingiCore` があり、`proofs/RingiCore.lean` が五つの定理のモジュールを import する。`crates/ritsu-model/tests/proofs.rs` の `LIBRARIES` に `RingiCore` を足し、定理名の一覧（`THEOREMS`）にもモデルの性質を足した。CI の `.github/workflows/proofs.yml` の `paths` に ringi の `src/`・例・機械のための申請・`crates/ritsu-model/**` が入っている。
- **確かめたこと**：`proofs/` で `lake build` が全部通る（既定の標的に定理が入る）。`RITSU_TEST_LEVEL=tools cargo test -p ritsu-model --test proofs` が通り（22,615 宣言、10,727 定理）、全部の宣言が Lean の三つの公理のほかに立たない。`ritsu check ritsu.ctx` が通る（`.lean` は地図の成果物でないので、足す物は無い）。`RITSU_TEST_LEVEL=tools cargo test -p ritsu-model --test ringi` が 36 申請で差 0。`cargo test -p ringi --lib model::` が通る。`cargo test -p ringi --test vectors` が 26 件通る。`RITSU_TEST_LEVEL=tools cargo test -p ritsu --test gen_requests the_code_of_the_tests_of_the_machine_answers_as_the_reference_does` が四つの出力先で 24 申請・7,739 本差 0。
- **★G-1 の直し**（`origin_on`。DESIGN 17 章の「直した形」）：`same approver approves once` の元の承認を探す所を、すぐ前のステップがこの決まりで済んだものならさらに前へさかのぼり、その人が自分で答えたステップで止まる形に、参照インタプリタ（`src/org/listing.rs` の `origin_on` と、前のステップを読む `Seen` に `Answered` と `Once` を分ける機械側の `seen`）と `src/lower/ticket.kn` とモデル（`Records` の `Look`、`Stages` の `look_of`・`scan_step`・`origin_step`）を同じ読みに直した。`tests/machine/once.ringi` に、差し戻しで自分の答えたステップへ戻ってまた承認する筋のシナリオを二つ足した（自分で承認する版と、前の承認を代理がした版の `traced` の形）。履歴の `approved` の行の `once_with`・`by`・`proxy_of` と部分の `of` が正しい所を指す。
- **変異のためのベクタ**：落ちなかった筋を行く申請とシナリオを足した。`tests/machine/seat.ringi`（`request expense`、部分だけ答えのあるステージで承認を取り消すベクタが `state` で断られる形）、`tests/machine/deep.ringi` とその組織 `tests/machine/.deep_org/`（30 段目の長を見つける申請と、誰もいない形で限界を越える申請）。`tests/machine.rs` と `tests/vectors.rs` の `MACHINE` と、`crates/ritsu-model/tests/ringi.rs` の一覧に二本を足した（機械の golden `run/machine/{seat,deep}.*.txt` とベクタの `vectors/machine/{seat,deep}.txt` を新設）。
- **残したもの**：時計のイベントが進めることの証明。kaname の `--target lean` の書き出しが機械のファイルで組めない所（17 章の最後）。`crates/ringi/src/model.rs` は `engine/act.rs` の非公開の `ascii_of`・`laid`・`lost` を写しているので、それらを変える段では合わせで直す。組織を上へたどる深さを 31 にずらす変異は、どの正しい組織でも同じふるまいになる（組織の検査が 30 チームを超える上を誤りにするので、その先は読まれない。変異としては落とせない）。
