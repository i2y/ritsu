# ringi 設計文書

会社の申請と承認（経費、購買の稟議、休暇、アカウントの申請、契約の確認など）を書く言語。ファイルは `.ringi`、コマンドは `ritsu ringi`、クレートは `crates/ringi`。ritsu の十番目の言語である。名前は 2026-11-18 に `ringi` と決めた。サーバーのフレームワークは `ringi-server` である。生成物とサーバーとの約束（`--target requests` の文書、チケットの機械のモジュール `ticket`、`x-ritsu-request`、カタログの `ritsu:`）には、言語の名前を出さない（12.4）。

申請の種類一つを、一つのファイルに書く。フォーム、経路と承認者の決め方、チケットのステートマシン、組織の四つを、ringi が自分の意味として持つ。中に書いた表、条件、計算、期限は、今の言語のファイル（`.rule`、`.cal`、`.kn`）に下ろし、rulec、koyomi、kaname に確かめさせる。会社のカレンダー、予算や休暇の残り、会社全体の決まり、いくつもの申請が使う表、承認のあとの処理、規程の条との対応は、koyomi、chobo、sekisho、rulec、dandori、yuen のファイルに置いて `use` で読む。

この文書は ritsu 0.55.2 の上で書いた（2026-11-14〜18）。ringi は、構文、名前、型の検査、中に書いた物を今の言語のファイルに下ろして確かめること、組織とマスタのデータを読んで版ごとに承認者を決めること、申請の種類を使える日の範囲、チケットの機械（参照インタプリタ）とシナリオ、人がチケットに書くコメント、チケットが終わることと仮押さえの検査、ベクタ、言語をまたぐ検査、`check`、`simulate`、`run`、`vectors`、`api`、`doc`、`diff`、`extract`、`init`、`explain` のコマンド、`ritsu gen` の言語のパッケージ（チケットの機械、申請ごとのつなぎ、申請の種類の一覧）と `--target requests`（サーバーの API、エージェントの道具、画面）を持つ。コマンドと `ritsu gen` の出力、診断の文、下ろしたファイル（`tests/golden/lowered/expense/` は `extract` の出力）は、走らせた実物である。今の言語のファイルだけで試した物（共有のファイル、kaname と rulec での試し）の出力は、公開版の 0.55.2 で走らせた実物である。例は `examples/` にある（19 章）。

この文書の並び：

- 1 章：全体像、芯、前提、ほかの言語との分担、語。
- 2〜3 章：対象の範囲と、文法。
- 4〜5 章：フォームと経路。
- 6 章：中に書いた物の意味と、`use` で読む物のつなぎ方。
- 7〜8 章：チケットのステートマシンと、組織。
- 9 章：検査と診断のコード。
- 10〜13 章：参照インタプリタ、ページ、生成、画面。
- 14〜16 章：管理する人の作業、エージェントと dandori、入口。
- 17〜19 章：Lean、試験、例。
- 20〜21 章：捨てた形とまだやらないこと、確かめで見つかった気になる点との関係。

## 1. 全体像

### 1.1 図

```
 申請の種類 expense.ringi ─ use ─▶ 共有の経路 .ringi、組織 .ringi、マスタ .ringi
   │                       └ use ─▶ .cal（会社のカレンダー）、.book（予算）、.gate（会社全体の決まり）、
   │                                .rule（決裁権限表）、.flow（エージェント、承認のあとの処理）
   │                                ▲ .req（規程の条との対応）が ringi の表、経路、期限を指す
   │ ritsu ringi check / ritsu check
   ▼
 ringi の検査（フォーム、経路、チケット、組織）
   │ 下ろす（ディスクに書かない。ritsu_base::fs::Overlay の上に置く）
   ▼
 expense.lowered/route.rule  form.rule  values.rule  choices.rule  next_up.rule  terms.cal
   │ rulec・koyomi が普通のファイルとして確かめる。ritsu-cross が境目を確かめる
   │ ritsu gen は、申請ごとのつなぎ（expense.lowered/expense.kn）とチケットの機械
   │ （根の下の .lowered/ticket.kn）も置き、kaname が確かめる
   ▼
 各言語の診断 ──行の対応──▶ ringi の行と ringi の言葉の診断
   │
   ├─ ritsu ringi doc / diff ：経路を一枚で見せるページ、変更の前後、期限がいちばん長くなる場合
   ├─ ritsu ringi run / simulate / vectors ：チケットのステートマシンの参照インタプリタ
   └─ ritsu gen ：言語ごとのパッケージ（決まりとチケットのコード、申請の種類の一覧）と、
                  <out>/requests/（A2UI の画面、カタログ、OpenAPI、MCP の道具、組織のスキーマ、ベクタ）
                     │
                     ▼
        言語ごとのフレームワーク（別のリポジトリ。12.5）：チケットと履歴の保管、イベントの受け口、
        期限と催促の時計、することの実行、組織の版、画面の配信、MCP サーバー。
        認証、保管、通知、添付の置き場、組織の取り込み元、flow の動かし方、
        エージェントのステップ、監査ログの出し先は、組織ごとに差し替える
```

### 1.2 芯：申請一件をチケットとして持ち、イベントで進める

ringi のほかの九つの言語で経費申請を一つ書くと、一通り書けて走るが、申請の種類一つに 944 行かかり、次の所で止まる。並列のステップで各人の答えが見えず「N 人以上」の議決が書けない。組織図を上にたどる指定が書けない。承認と取り下げと期限を同時に待てない。差し戻しの回数を数えられない。組織のデータの置き場が無い。申請を受ける口と画面が手書きになる。どれも、申請を「まっすぐ待つワークフロー」に押し込んだために出ている。

ringi は、申請一件をチケットとして持つ。意味は、チケットの状態、来たイベント、そのときの組織、時刻から、次の状態と、することの一覧（通知、時計、帳簿の仮押さえと確定と取消、flow の開始）を決める一つの純粋な関数である。

```
next(ticket, event, organization, now) -> (ticket', effects)
```

時刻も組織も入力として渡す。検査、参照インタプリタ、シミュレーション（人を選んで経路を試す）、ページ、サーバーと画面のコードは、どれもこの関数の上に立つ。

### 1.3 前提

- **R1：申請の種類一つは一つのファイル。** フォーム、経路、分岐の表や条件、期限と催促を、その中に書く。別のファイルに置いて `use` で読むのは、いくつもの申請で共有する物だけにする。
- **R2：ringi が自分で持つ意味は四つ。** フォーム、経路と承認者の決め方、チケットのステートマシン、組織。表、条件、計算、期限の意味は持たない。中に書いたそれらは今の言語のファイルに下ろし、今の言語が確かめてコードにする（6 章）。根の DESIGN の P4（ある言語の意味をもう一度書かない）を、言語の中でも守る。
- **R3：意味は純粋な関数。** 1.2 の `next`。時刻と組織は入力で、関数は時計も組織も読みに行かない。
- **R4：決められないことを通さない。** 根の DESIGN の P5 のとおり。組織のデータが無いときの「承認者が空くか」は決められないと言い、通ったことにしない。
- **R5：ritsu は生成までを受け持つ。** 生成するコードは、ランタイムを持たない読めるライブラリにする（rulec の P5 にそろえる）。動かすのは別のリポジトリの、言語ごとのフレームワーク（12.5）で、ritsu はそれとの約束（12.4）を書き出す。
- **R6：診断は ringi の行と ringi の言葉で出す。** 下ろした先の言語の診断も、ringi のステップ、承認者、申請の言葉に直して出す（9.2）。
- **R7：ファイルを書くのはエージェント、確かめるのは人。** 構文はエージェントが書き違えにくい形にし、人はページ（11 章）で確かめる。エディタの拡張と LSP は作らない。
- **R8：同じ決まりを二か所で動かすときは、同じ元から出す。** フォームの自動計算と入力の検査は、下ろした規則から、サーバーの側と画面の側の両方のコードにする（13.7）。

### 1.4 ほかの言語との分担

| 書くもの | 言語 | ringi からのつなぎ |
|---|---|---|
| フォーム、経路と承認者の決め方、チケット、組織、マスタ | ringi | 自分の意味 |
| 中に書いた表、条件、計算 | rulec | 下ろす（`.rule`） |
| 中に書いた期限と催促、日付の計算 | koyomi | 下ろす（`.cal`） |
| 申請ごとのチケットのつなぎ | kaname | 下ろす（`.kn`）。チケットの機械の本体は一度だけ書いた kaname のモジュール（12.1） |
| 会社のカレンダー（休みの日） | koyomi | `use calendar` |
| 予算、休暇の残り | chobo | `use book`。チケットが仮押さえ、確定、取消をする |
| 会社全体の決まり（自分の申請は承認しない、部門の管理者が変えられるのは自分の部門の申請の種類だけ） | sekisho | `use gate` |
| いくつもの申請が使う表（決裁権限表、日当、税率） | rulec | `use rule` |
| 承認のあとの処理、エージェントの仕事 | dandori | `use flow` |
| 規程の条との対応と、人が確かめた記録 | yuen | `.req` が ringi のファイルの中の物を指す |

### 1.5 語

| 日本語 | 英語（構文とページ） | 意味 |
|---|---|---|
| 申請の種類 | request type（`request`） | `.ringi` の一つ。フォーム、経路、チケットの決まり |
| 申請 | request | 申請者が出す一件。出したあとはチケットとして持つ |
| チケット | ticket | 申請一件の、今の状態と履歴 |
| フォーム | form | 申請者と承認者が入れる値の宣言 |
| フィールド | field | フォームの一つの値 |
| 明細 | rows | 行を繰り返すフィールド |
| 自動計算 | computed field | `=` で書くフィールド |
| マスタ | master | 選ぶ元の一覧。行ごとに有効期間を持つ |
| 経路 | route | ステップの並びと、それぞれの承認者 |
| ステップ | stage | 経路の一段。承認、並列、回覧、エージェントの答え、要約の五つ。構文の語は `stage` にした（3.6） |
| 承認者 | approver | ステップに答える人 |
| 承認者の決め方 | who | 承認者を、人、相対、チーム、指名、マスタ、エージェントで決める宣言 |
| 議決 | quorum | 並列のステップが通るのに要る承認の数（「N 人以上」「N% 以上」「全員」） |
| 回覧 | circulate | 知らせて読めるようにするだけで、答えを待たないステップ |
| 申請者 | requester | 申請を出した人。代理申請では、代わりに出された人 |
| 出せる人 | submitted by | 申請の種類を出せる人。`ticket` の `submitted by` で、チーム、役職、人で絞る。書かなければ版のどの人も |
| 差し戻し | send back | 承認者が申請者か前のステップへ戻すこと |
| 再申請 | resubmit | 戻ってきた申請を、申請者が直して出し直すこと |
| 取り下げ | withdraw | 申請者が進行中の申請を自分の手元へ戻すこと |
| 却下 | reject | 承認者が申請を終わらせること。直しても出し直せない |
| 承認の取り消し | retract approval | 承認した人が、次のステップが答える前に承認を取り消すこと |
| 保留 | pause | 承認者が期限と催促を止めること |
| 代理 | proxy | 期間と申請の種類を絞って、ほかの人の代わりに承認か申請をすること |
| 上へ回す | escalate | 期限が過ぎたか承認者がいないとき、組織の上の人へステップを回すこと |
| 期限、催促 | due、reminder（`remind`） | ステップごとの営業日の期限と、その前の知らせ |
| イベント | event | チケットに来るもの。人の操作、時計、flow の答え |
| コメント | comment | 人が操作に添えるか、それだけで書く文。チケットの履歴に残し、状態は変えない（7.2） |
| することの一覧 | effects | `next` が返す、サーバーがすること |
| 終わり方 | ending | 承認、却下、取り下げ、期限切れの四つ |
| 組織 | organization | チーム、役職、所属、チームの長、代理。データは版ごと |
| チーム | team | 会社、本部、部、課、委員会など。種類は組織のファイルが宣言する |
| 役職 | position | 低い順に並べた列挙 |
| 所属 | membership | 人とチームの組。一つが主の所属で、ほかは兼務 |
| 上長 | manager | 申請者の主の所属のチームの長 |
| 版 | version | 組織のデータの版（効き始める日で呼ぶ）と、申請の種類の版（`v1`、`v2`） |
| 下ろす | lower | 中に書いた表、条件、計算、期限を `.rule`、`.cal`、`.kn` にすること |
| 行の対応 | line map | 下ろしたファイルの行から ringi のファイルの行への対応 |
| 管理する人 | administrator | 申請の種類を受け持つ部門で、変更を頼み、確かめて反映する人 |

## 2. 対象の範囲と、対象の外

範囲は「申請と承認の製品が使えるために要る物」から決めた。ほかの製品との違いを作るために選ばない。ritsu の検査や証明は、作りの結果として付く物で、範囲を選ぶ基準にしない。機能ごとの仕分け（最初の版、後の版、作らない）は、仕組みの言葉で下に書く。

### 2.1 最初の版に入れる物

- **フォーム**：短文、長文、整数、単位の付いた数（金額、率など）、日付、選択、添付、人、チーム、真偽。必須と任意、条件つきの表示と条件つきの必須、明細（行の繰り返し）と合計、自動計算、マスタからの選択と表引き（有効期間つき）、承認者だけが入れるフィールド、承認者が申請を直す設定、英語と日本語の文言（4 章）。
- **経路**：承認、並列（「N 人以上」「N% 以上」「全員」）、回覧、エージェントが答えるステップ、エージェントの要約。承認者の決め方は、人を指定、申請者の上長、申請者のチームの長、申請者に最も近い役職の人、チーム、申請者か前の承認者の指名、マスタからの表引き、使う規則の出力。分岐は表の形とステップの条件の形の両方と、経路そのものを切り替える表。承認者がいないときは留まる、上へ回す、飛ばす。自己承認を許すか、同じ承認者が二つ以上のステップに当たるときに一度の承認で済ませるか（すぐ前の承認で済ませる形と、最後のステップで一度だけ答える形）、だれが申請を出せるか（チーム、役職、人で絞る。どれも申請の種類ごと）。差し戻し（申請者へ、前の任意のステップへ、回数の上限つき）、再申請の期限、取り下げ、却下、承認の取り消し、保留。代理承認と代理申請（期間と申請の種類を絞れる）。管理する人が今のステップに承認者を足すことと、承認者の受け持ちをほかの人に引き継ぐこと。営業日の期限と催促、期限が過ぎたときの扱い。いくつもの申請で使う共有の経路（5 章）。
- **チケット**：状態、イベント、遷移、することの一覧。人の操作に添えるコメントと、コメントだけを書くこと。帳簿の仮押さえと確定と取消、承認のあとに flow を始めること（7 章）。
- **組織**：役職とチームの種類の宣言、版と期間つきのデータ（CSV か JSON）、日付で切り替える予約、兼務、代理。どの版で、どの申請者の、どのステップの承認者が空くかの一覧（8 章）。
- **管理する人の作業**：普通の言葉の依頼をエージェントが直し、検査が確かめ、変わる所をページで見せ、頼んだ人が確かめて記録し、反映する（申請の種類の版は、今すぐ、日付、説明に戻す、止めるのどれか。組織は差を見てから取り込む）。反映と取り込みは記録に残り、チケットの動きと並べて読める。部門ごとに、だれがどの申請の種類を変え、どの記録を読めるかを分ける（14 章）。
- **ページ**：経路を一枚で見せるページ、変更の前後を見せるページ、期限がいちばん長くなる場合、組織の版の前後で変わる承認者（11 章）。
- **生成**：決まりとチケットのコード（TypeScript、Python、Go、Rust）、申請書と承認カードと一覧と履歴の A2UI 1.0 のメッセージ、ritsu のカタログ、サーバーの API の OpenAPI、エージェントが使う道具の定義（MCP）、組織のデータのスキーマ、ベクタ（12、13 章）。
- **エージェント**：出た申請を人のステップの前に確かめるステップ、要約、表がエージェントに渡した範囲の決裁。dandori の flow の途中で人の承認が要るとき、flow がチケットを開き、続きを申請の種類の `on` の行が始めること（15 章）。

### 2.2 後の版に回す物

- 確認ありの回覧（全員が読んだと答えるまで待つ）。最初の版の回覧は待たない。
- 曜日や時間帯を選ぶ催促、繰り返しの催促。最初の版は、期限ごとに、その前に一度だけ知らせる（ステップの期限と、上へ回したあとの期限。5.10）。
- 申請者に見せずに値だけを持つフィールド（最初の版は、承認者だけのフィールドと自動計算で足りる）。いくつかのフィールドをまとめて表示の条件を付ける区切り（最初の版は、フィールドごとに条件を書く）。
- 決まった五つの種類に入らないステップ（最初の版は、エージェントのステップと要約で代える）。
- チームをマスタから引く指定（最初の版は、マスタから人を引く）。申請者のどの所属のチームの長も承認者にする指定（最初の版は主の所属）。いくつかのチームのどれか一人でよい指定（最初の版は、チームごとに一人ずつ）。
- 承認者が回覧する人を足すこと（最初の版は、経路に書いた回覧だけ）。条件に合う人に申請を見せる権限（サーバーと gate の側で作る）。
- 人事の仕組みと API でつないで組織を取り込むこと。最初の版は、CSV か JSON の版を取り込む（8.2）。
- 申請者向けのエージェントの機能（資料からフォームを埋める、出す前に食い違いを指摘する、申請の種類を探す）。サーバーと MCP の側で作る。
- flow の中で、開いたチケットの終わりを待つ形（終わり方とフォームの値を、チケットを開いた flow に返し、同じ flow が続きを走らせる）。最初の版の flow は、チケットを開いたところで終わり、続きは申請の種類の `on` の行が始める（15.4）。dandori の `callback` のタスクはトークンを HTTP の操作に渡せないので、待つ形には、dandori とサーバーのどちらに何を足すかを先に決める。
- コメントの添付と宛先、書いたコメントの直しと消し。最初の版のコメントは、理由を求める却下と差し戻し（`comment required for`）のほかは無くてよく、書いたあとは変えない（7.2）。
- チャットの道具での通知と承認、Webhook、SSO、IP アドレスの制限、監査ログの保存期間の設定。どれもサーバーの側の物で、ritsu が書き出す物は変わらない。
- 試しの環境と本番のあいだで申請の種類とマスタをそろえること。ファイルは git にあるので、サーバーへの反映の手順として後で作る。

### 2.3 作らない物

- フォームと経路を画面で組み立てるエディタ。ファイルを書いて直すのはエージェントで、人はページを読んで確かめる（R7）。
- 経路を CSV から取り込むこと。CSV で経路を持っている会社には、エージェントが `.ringi` に書き直し、検査とページで確かめる。
- 外のワークフローエンジンの上で申請を動かすこと。エンジニアでない人が使う製品なので、外のエンジンの運用を前提にしない。

### 2.4 対象の外

- サーバーの実装（チケットの保管、イベントの受け口、時計、通知、監査ログ、画面の配信、認証、MCP サーバー）。別のリポジトリの、言語ごとのフレームワーク（`ringi-server`。一つ目は Go）が持つ（12.5）。ritsu はサーバーとの約束を書き出し（12.4）、`ritsu ringi init` が、そのフレームワークの上のプロジェクトの最初のファイルを書く（16.4）。
- 添付のファイルの中身の保管とメールの配信は、サーバーが外の保管と配信の仕組みを使う。ritsu が書き出すのは、どのフィールドが添付か（`types/<名前>.json`）と、知らせの宛先と種類（`notify`。7.4）である。
- 電子署名。

## 3. 文法

### 3.1 ファイルの種類

`.ringi` のファイルは、最初の行の語で種類が決まる。

| 最初の行 | 書くもの | 例 |
|---|---|---|
| `request <名前> v<版> "<題>"` | 申請の種類一つ | `expense.ringi` |
| `route <名前> v<版> "<題>"` | いくつもの申請で使う経路 | `shared/manager_approval.ringi` |
| `organization <名前> v<版> "<題>"` | 組織の形（役職、チームの種類、データの置き場） | `shared/company.ringi` |
| `masters <名前> v<版>` | マスタの形とデータの置き場 | `shared/masters.ringi` |

申請の種類のファイルは、次の順でブロックを並べる。

```
request expense v1 "Expense request"
description "…"
owner accounting
use …
form …
route …
who …
every stage …
table <名前> …        （0 個以上。経路と期限から使う表）
ticket …
claims …
examples …
scenario <名前> …     （0 個以上）
labels ja …           （0 個以上。ほかの言語の文言）
```

### 3.2 キーワード

| 置き場所 | 語 |
|---|---|
| 行の頭 | `request` `route` `organization` `masters` `description` `owner` `use` `form` `who` `every stage` `table` `ticket` `claims` `examples` `scenario` `labels` `inputs` `positions` `team kinds` `data` `master` `key` `fields` `effective from` |
| `use` の種類 | `organization` `masters` `calendar` `book` `flow` `rule` `dates` `route` `gate` `from` |
| フィールドの型 | `text` `long text` `number` `money[…]` `rate[…]` そのほかの単位の型、`date` `choice` `choice from` `attachment` `person` `team` `bool` `rows` |
| フィールドの印 | `range` `count` `optional` `required when` `shown when` `approvers only` `open in` `round` |
| 経路とステップ | `at least` `of` `%` `all of` `circulate to` `note from` `when` `and` |
| 承認者の決め方 | `person` `team` `manager of` `head of` `nearest … above` `requester` `chosen by` `previous approver` `in` `lookup … by` `agent` `else` |
| 期限 | `due in` `remind after` `business day(s)` `at end of day` `at` `when late` `when empty` `escalate` `by` `then` `expire` `stay` `skip` |
| チケット | `self approval` `allowed` `not allowed` `same approver approves once` `at the last stage` `submitted by` `everyone` `everyone in` `team` `position` `or higher` `in` `person` `except` `send back` `to the requester` `to any earlier stage` `at most` `times` `resubmit within` `withdraw until` `it is decided` `retract approval until the next stage answers` `pause at most` `approvers edit` `comment required for` `reject` `on` `hold` `post` `void` `launch` |
| 主張 | `only when` `possible` `is` `every stage reminds before it is due` `every ticket ends within` |
| シナリオ | `requester` `given` `form`、表の列 `at` `event` `by` `comment` `-> stage` `status`（`comment` の列は無くてよい） |
| 式 | `is` `is not` `none` `and` `or` `not` `sum(…)` `+` `-` `*` `/` `business days in` |

### 3.3 EBNF に近い形

```
request      = "request" name version string NL header* use* form route who every_stage? table*
               ticket claims? examples? scenario* labels*
header       = "description" string NL | "owner" team_id NL | "effective from" date NL
use          = "use" use_kind alias "from" string NL | "use" "gate" string NL
use_kind     = "organization" | "masters" | "calendar" | "book" | "flow" | "rule" | "dates" | "route"
form         = "form" NL ( field | rows_field )+
field        = name ":" type string? mark* ( "=" expr mark* )? NL option*
option       = INDENT name string? NL                         (* choice の値と文言、choice from の規則の列挙の値の題。文言の無い行は名前を題にする *)
rows_field   = name ":" "rows" string "count" bounds NL ( INDENT field )+
type         = "text" | "long" "text" | "number" | unit_type | "date" | "choice" | "choice" "from" ref
             | "attachment" | "person" | "team" | "bool"
mark         = "range" bounds | "count" bounds | "optional" | "required" "when" cond | "shown" "when" cond
             | "approvers" "only" | "open" "in" alias | "round" rounding
route        = "route" NL ( route_table | stage_line+ ) | "route" alias "(" args? ")" NL
route_table  = header_row row+                                (* 入力の列、"->"、ステップの列 *)
stage_line   = name stage_spec ( "when" cond )? NL ( INDENT deadline_line )*
stage_spec   = who_name | quorum "of" who_name | "circulate" "to" who_name | "note" "from" alias | ref
quorum       = "all" | "at" "least" INT | "at" "least" INT "%"
who          = "who" NL ( name "=" who_expr NL )+
who_expr     = "person" string | "team" team_id | "manager" "of" "requester"
             | "head" "of" "requester" "." team_kind | "nearest" position "above" "requester"
             | "chosen" "by" ( "requester" | "previous" "approver" ) "in" name
             | "lookup" ref "by" name | "agent" alias "," "else" who_name
every_stage  = "every" "stage" NL ( INDENT deadline_line )+
deadline_line = "due" "in" INT business "at" ( "end" "of" "day" | TIME )
             | "remind" "after" INT business "at" TIME
             | "when" "late" ( "escalate" ( "by" name )? ( "," "then" late )? | "expire" | "stay" )
             | "when" "empty" ( "stay" | "escalate" ( "," "then" ( "skip" | "stay" | who_name ) )? | "skip" )
late         = "escalate" ( "by" name )? | "expire" | "stay"
ticket       = "ticket" NL ( INDENT ticket_line )+
ticket_line  = "self" "approval" ( "allowed" | "not" "allowed" )
             | "same" "approver" "approves" "once" ( "," "at" "the" "last" "stage" )?
             | "submitted" "by" groups ( "except" groups )?
             | "send" "back" ( "to" "the" "requester" | "to" "any" "earlier" "stage" ) "," "at" "most" INT "times"
             | "resubmit" "within" INT business
             | "withdraw" "until" ( name | "it" "is" "decided" )
             | "retract" "approval" "until" "the" "next" "stage" "answers"
             | "pause" "at" "most" INT business
             | "approvers" "edit" name ( "," name )*
             | "comment" "required" "for" reasoned ( "," reasoned )*
             | "on" ending ( "," ending )* action ( "when" cond )?
ending       = "submitted" | "approved" | "sent" "back" | "rejected" | "withdrawn" | "expired"
reasoned     = "reject" | "send" "back"
action       = ( "hold" | "post" | "void" ) alias "." name "(" args ")" | "launch" alias "(" args ")"
groups       = group ( "," group )*
group        = "everyone" ( "in" team_id )? | "team" team_id | "person" string
             | "position" position ( "or" "higher" )? ( "in" team_id )?
claims       = "claims" NL ( INDENT name ":" claim NL )+
claim        = name "is" spec "only" "when" cond | "possible" name "is" spec
             | "every" "stage" "reminds" "before" "it" "is" "due" | "every" "ticket" "ends" "within" INT business
examples     = "examples" NL route_table
scenario     = "scenario" name NL INDENT "requester" ( person_id | string ) NL ( INDENT "given" alias "." name "(" args ")" NL )*
               INDENT "form" assignments NL event_table          (* given は帳簿の始めの振替 *)
event_table  = "|" "at" "|" "event" "|" "by" "|" ( "comment" "|" )? "->" "stage" "|" "status" "|" NL event_row+
event_row    = "|" DATE TIME "|" event ( "(" args ")" )? "|" person_id? "|" ( string? "|" )? name? "|" status "|" NL
                                                              (* 文のセルは、見出しに comment を書いた表だけ *)
event        = "submit" | "resubmit" | "approve" | "reject" | "send" "back" | "retract" | "pause" | "resume"
             | "withdraw" | "discard" | "edit" | "add" "approver" | "hand" "over" | "comment"
             | "remind" | "late" | "resubmit" "late" | "pause" "ends"
             | "agent" "answered" | "note" "ready" | "flow" "failed"
status       = "in" "progress" | "paused" | "with" "requester" | "approved" | "rejected" | "withdrawn" | "expired"
labels       = "labels" lang NL ( INDENT label_key string NL )+
label_key    = "request" | name ( "." name )? | "stage" name | "who" name
business     = "business" ( "day" | "days" )
```

式（`expr`、`cond`）とセルの綴りは rulec と同じにする（`>30_000JPY <=100_000JPY`、`none`、単位、列挙の値、`-` はどの値でもよいセル）。人とチームのフィールドの値は、`person "e7001"`、`team legal` と書く（シナリオのフォームと、イベントの引数、`simulate --form`）。等しいかを比べる語は `is`（sekisho と同じ）で、下ろすときに rulec のセル（`= paid`）にする。`=` は、自動計算と承認者の決め方の宣言だけに使う。

`ticket` の行は、`on` の行のほかは、一つの `ticket` に一度だけ書ける。二度書けば、どちらが効くかを読む人が迷うので、あとの行で E006 にする（`submitted by` を二度書いた、など）。

組織とマスタのファイルの形は 8.1 と 4.6、共有の経路のファイルの形は 5.11 にある。

### 3.4 名前と日本語

名前は kaname と同じ書き方をする。英語の名前はそのまま書き、日本語の名前は `名前(ascii)` と書いて、括弧の中の名前を生成するコードと JSON の名前にする（`種別(category)`、`上長(manager)`）。名前の使える字と、生成先の言語の予約語を避ける決まりは kaname と同じにする。

チケットの機械は、名前を書いたとおりに持つ。`run` の文と診断は、書いたとおりの名前を見せる（シナリオの `-> stage` に書く名前と同じ）。JSON に書く所（チケット、イベント、することの一覧、組織の部分。`run --format json` とベクタ）では、ステップ、承認者の決め方、フィールド、`use` の別名、マスタとその列を、括弧の中の名前で書く。だから、英語の名前で書いた申請と、同じ中身を日本語の名前で書いた申請は、申請の種類の名前と、人が読む文と、フォームに入れた文を除いて、同じベクタになる（`tests/machine/desk.ringi` と `desk.ja.ringi`）。`check` と `simulate` の `--format json` と、`api` の JSON も、同じ括弧の中の名前で書き、書いたとおりの名前を横に添える（キーが `name` なら `written`、ほかのキーなら `<キー>_written`。16.2）。サーバーと画面は括弧の中の名前で引き、人に見せる所でだけ、書いたとおりの名前を使う。

名前の置き場は四つに分ける。フォームのフィールド、ステップ、承認者の決め方、表。置き場が違えば同じ名前を使える（`account.ringi` のステップ `manager` と承認者の決め方 `manager`）。式の中の名前はフィールド、`use` の別名、`requester`、`ticket` を指し、ステップのセルと `stage_spec` の名前は承認者の決め方を指す。文言の `labels` は、置き場を `stage` と `who` で言う。

`requester` と `ticket` は、フィールドの名前に使えない（E002）。式の中で `requester.id`、`requester.department`、`ticket.id`、`ticket.round`、`ticket.attempt` を読むためである。`skip` と `stay` は、承認者の決め方の名前に使えない（E002）。`when empty escalate, then …` のあとでは、この二つを語として読み、ほかの名前を承認者の決め方として読むためである（5.4）。kaname の予約語（`then`、`match`、`some` など）は、フォームと明細の行のフィールドの ASCII の名前に使えない（E002）。フィールドはチケットの JSON のキーで、それを読み書きする kaname のコードが、予約語を名前にできないためである（6.4）。下ろしたファイルの言語（rulec と koyomi）の予約語は、どの名前にも使える。下ろすときに別の名前で書く（6.4）。

### 3.5 `use`

dandori と sekisho にそろえる。種類、別名、`from`、パスの順で書き、gate だけは別名を持たない（sekisho の `use gate` と同じ）。パスはファイルからの相対で、`@<モジュール>/…` の形（根の DESIGN 17 章）も読める。ディスクの根から書いたパスは、どの種類の `use` でも読まずに E005 にする。ツリーを別の所へ写すと、別のファイルを指すか、どのファイルも指さなくなるからである。

```text
$ ritsu ringi check tests/mutants/E005_a_calendar_from_the_root_of_the_disk.ringi
error[E005]: tests/mutants/E005_a_calendar_from_the_root_of_the_disk.ringi:3:26: `/srv/shared/office.cal` is an absolute path; write it from the directory of the file it is in
     3 | use calendar office from "/srv/shared/office.cal"
  = Written from the directory this file is in, it names the same file wherever the tree is copied (`../` is the directory above).
```

```
use organization org from "shared/company.ringi"
use calendar office from "shared/office.cal"
use rule authority from "shared/authority.rule"
use book budget from "shared/budget.book"
use flow pay from "shared/pay.flow"
use route standard from "shared/manager_approval.ringi"
use gate "shared/company.gate"
```

`use dates` は koyomi のカレンダーでない日付の関数を読む（フォームに支払日を見せる、など）。`use calendar` は営業日を数えるカレンダーで、期限と `open in`、`business days in` が使う。申請の種類一つが `use calendar` を二つ持つとき、期限がどちらで数えるかを `every stage` に書く（`due in 3 business days in office`）。一つなら書かなくてよい。

### 3.6 ほかの言語のキーワードとのぶつかり

ほかの言語のキーワードと同じ語は、同じ意味のときだけ使う。各言語の `kw.rs`（dandori と chobo は `syntax.rs`）の語と突き合わせて、次のように決めた。

| 語 | ほかの言語での意味 | ringi での扱い |
|---|---|---|
| `step` | rulec と kaname の、率の型の刻み（`rate[step 1%]`） | 使わない。ステップは `stage` と書く。フォームに率のフィールドを書くと、一つのファイルに二つの意味が並ぶため |
| `up` | rulec と kaname の、丸めの向き（`round up(1JPY)`） | 使わない。上へ回すのは `escalate` |
| `start` | rulec の機械の始まりの状態、koyomi の月の初め | 使わない。flow を始めるのは `launch` |
| `cancel` | dandori の、ワークフローのキャンセル | 使わない。承認の取り消しは `retract approval` |
| `file` | yuen、sakai、rulec、koyomi の、文書を指す参照の書き方 | 使わない。添付の型は `attachment` |
| `words` | kaname の、文字列を語に分ける関数 | 使わない。文言のブロックは `labels` |
| `never`、`always` | rulec の前提と機械の主張 | 使わない。主張は `only when` と `possible` で書く |
| `hold` | chobo と dandori の仮押さえ | 同じ意味でだけ使う（`hold budget.reserve(…)`）。保留は `pause` |
| `once` | rulec の、一つの場合に一度だけ答える呼び出し（`once 返金額 >0円`） | 一度だけという同じ意味で使う（`same approver approves once`。同じ承認者は一度だけ承認する） |
| `last` | kaname の、リストの最後の要素を取る組み込み（`xs.last()`） | 並びのいちばん後ろという同じ意味で使う（`same approver approves once, at the last stage`。その人が答える最後のステップ） |
| `except` | rulec の `apply` の、取り込まない定義（`except 減額`）。sakai の地図の、範囲から除くパス | 集まりから除くという同じ意味で使う（`submitted by everyone except team board`。出せる人から取締役会の人を除く） |
| `unit` | chobo の単位、sakai の単位 | 使わない。部署や課は `team` |
| `examples`、`scenario`、`possible`、`table` | rulec、koyomi、kaname の例、機械のシナリオ、届く値、表 | 同じ意味で使う |
| `open` | koyomi の開いている日 | 同じ意味で使う（`open in office`） |
| `when`、`every`、`any`、`times`、`else`、`of`、`by`、`in` | 条件、繰り返し、回数、そのほか | 同じ意味で使う |
| `agent` | dandori のエージェントのタスク | 同じ意味で使う（dandori の flow に答えさせる） |
| `key` | chobo と dandori の冪等キー | マスタの、行を見分ける列に使う。一つを見分ける値という同じ意味 |
| `owner` | yuen と sakai の、受け持つ人 | 申請の種類を受け持つチームに使う。同じ意味 |
| `empty` | rulec の、要素の無いときの値 | 承認者が一人もいないときに使う。要素が無いという同じ意味 |
| `reject` | koyomi の、入力を受け付けない | 却下に使う。受け付けないという同じ意味 |
| `form` | dandori の、HTTP の本文の形式 | ringi の芯の語なので使う。どちらも入力の形という意味で、一つのファイルに並ばない |
| `comment` | どの言語も語にしない | 人が操作に添える文に使う（シナリオの表の列と、イベント `comment`）。ファイルの中で読む人に向けて書く `#` の注とは別の物で、表の見出しとイベントのセルにだけ出る |

`step` を書いたファイルには、E001 が次のように言う（`tests/mutants/` の変異）。

```text
$ ritsu ringi check E001_every_step.ringi
error[E001]: E001_every_step.ringi:12:7: Write `stage`, not `step`
    12 | every step
  = In rulec and kaname `step` is the step of a rate's type (`rate[step 1%]`); so that one file does not give the word two meanings, ringi calls a stage `stage`.
$ ritsu ringi check E001_ステップをstepと書いた.ringi --lang ja
エラー[E001]: E001_ステップをstepと書いた.ringi:12:7: `step` ではなく `stage` と書いてください
    12 | every step
  = rulec と kaname では `step` は率の型の刻み（`rate[step 1%]`）です。一つのファイルで二つの意味にならないよう、ringi はステップを `stage` と書きます。
```

## 4. フォーム

### 4.1 フィールドの型

型は ritsu の型にする。人が入れる数と日付には範囲を、文には長さの上限を、必ず書く（E101、E102）。範囲は、下ろした規則の入力の範囲と、画面の入力の検査と、生成するコードの確かめの全部に使う。

計算するフィールド（4.3、4.4）は、人が入れないので範囲を書かなくてもよい。書かなければ、範囲は読む値の範囲から計算で決まる。書けば、その範囲が受け付ける値の上限と下限になる。計算した値が範囲の外になる申請は出せず、画面と生成するコードは、人が入れるフィールドの範囲と同じように、どのフィールドのどの範囲かを言って断る。そのフィールドを読む所（経路の表、条件、ほかの計算、主張）は、書いた範囲の中だけを見ればよい。経路の表は、書いた範囲を覆えば足りる。

範囲が計算の届く範囲より広くても、一部だけ重なっても、誤りではない（外の分は断る）。誤りにするのは、どう入れても満たせない範囲、つまり計算の届く範囲と一点も重ならない範囲だけで、E103 にする。届く範囲は、読む値の範囲から区間で計算する。読む値が計算するフィールドなら、その計算の届く範囲（範囲を書いていれば、それとの重なり）で読む。丸めは、書いた向きのとおりに数える。金額が 1 円から 33 円までの `fee = amount * 3%  round down(1JPY)` は、切り捨てて 0 円にしかならないので、`range >0JPY` は満たせない。丸めと明細の行の数を、生成するコードと同じに数えるのは、下ろした先の rulec である。下ろした `form.rule` の範囲に値が一つも入らないと rulec が言えば（rulec の E150）、ringi は同じ E103 にして、rulec が数えた届く範囲を ringi の書き方で添える（9.2）。

```text
$ ritsu ringi check tests/mutants/E103_a_fee_rounded_down_to_nothing.ringi
error[E103]: tests/mutants/E103_a_fee_rounded_down_to_nothing.ringi:6:62: The range `>0JPY` of the field fee holds no value its computation comes to
     6 |   fee    : money[JPY] "Fee" = amount * 3%  round down(1JPY)  range >0JPY
  = The computation comes to `>=0JPY <=0JPY` (from the ranges of what it reads). With this range no request can be made.
  = Correct the range so that it meets what the computation comes to. A request whose value falls outside the range is refused; a range wider or narrower than that is no mistake.
```

範囲を出うることを誤りにしないのは、上限を書く理由が、多くは届く範囲を狭めることそのものだからである。経費の `amount` は、20 行 × 1 行 100 万円で 2,000 万円まで届くが、一回の申請は 1,000 万円までと書いてある。届く範囲まで広げなければ通らないなら、この決まりは書けない。rulec の `sum … range` も同じ意味を持つ（4.3）。

| 型 | 書き方 | 下ろした先の型 | 画面（申請書） |
|---|---|---|---|
| 短文 | `text "題"  count <=100` | rulec の `string` | 一行の入力 |
| 長文 | `long text "題"  count >=1 <=200` | rulec の `string` | 複数行の入力 |
| 整数 | `number "題"  range >=1 <=20` | `number` | 整数の入力 |
| 単位の付いた数 | `money[JPY]`、`rate[step 0.1%]`、`mass[kg]` など | 同じ型 | 整数を書き込み、単位を付けて見せる部品 |
| 日付 | `date "題"  range >=2026-01-01 <=2027-12-31` | `date` | 日付の入力 |
| 選択 | `choice "題"` と、下に値と文言 | 列挙 | 一つを選ぶ部品 |
| マスタから選ぶ | `choice from m.vendors "題"` | `string`（キー） | 有効な行だけを並べた選択 |
| 規則の列挙から選ぶ | `choice from authority.kind "題"` | その列挙 | 一つを選ぶ部品 |
| 添付 | `attachment "題"` | （下ろさない） | ファイルを選ぶ部品 |
| 人 | `person "題"` | `string`（人の ID） | 組織の人を探して選ぶ部品 |
| チーム | `team "題"` | `string`（チームの ID） | チームを選ぶ部品 |
| 真偽 | `bool "題"` | `bool` | チェックボックス |
| 明細 | `rows "題"  count >=1 <=20` と、下にフィールド | `elements` | 行を足し、消せる表 |

単位の無い小数は持たない。rulec が持たないからである。数値は単位の型か率の型で書く。

### 4.2 必須、条件つきの表示と必須

フィールドは、書かなければ必須である。任意なら `optional` を書く。`required when <条件>` は条件が成り立つときだけ必須にし、`shown when <条件>` は条件が成り立つときだけ見せる。見せていないフィールドは必須にならず、送られもしない。

```
  trip_to : text "Destination"  count <=100  shown when category is travel
  receipt : attachment "Receipt"  required when amount > 30_000JPY
```

必須なのに隠れることがある組み合わせ（`required when` が成り立ち、`shown when` が成り立たない値があるもの）は E105 にする。ringi は二つの条件から真偽の出力を一つ持つ規則（問いの規則。下ろしたファイルの横に `<フィールド>.hidden.rule` として置き、`extract` は書かない）を作り、rulec に「真になる入力があるか」を尋ねる（`Rules::outputs_over`）。あれば、その入力を例にして言う。計算するフィールドを読む条件の尋ね方は 4.4 に書いた。

出し直し（`resubmit`）と承認者の直し（`edit`）で、渡した値や空にした値によって `shown when` が成り立たなくなったフィールドは、チケットの値を落とす（2026-11-16。7.3）。条件が、落とした値か、それを読む計算を読むなら、もう一度見て、落ちる物が無くなるまで繰り返す。そのイベントが値を渡したフィールドは落とさず、隠れるなら申請と同じに断る。種別を出張から経費に変えた出し直しは、出張のときだけ見せる行き先の値を落として通る。

### 4.3 明細と合計

明細は行の繰り返しで、行の中にフィールドを持つ。行の数の上限は必ず書く（`count >=1 <=20`。無ければ E101）。行の数は、チケットの機械が確かめ、書いた数の外を `form` の種類で断る（7.3）。生成するコードと画面も、同じに受け付けない（12 章、13 章）。下ろした規則は行の数の上限を持たない。rulec の `elements` には行の数を書く所が無く、`count` の集計は `where` を要する（rulec の E028）からである。

合計は `sum(items.price)` と書き、rulec の `elements` と `sum … over … of … range` に下ろす。範囲を書いた合計は、書いた範囲をそのまま `sum` の範囲にする。範囲を書かない合計は、行の数の上限と行の範囲から ringi が計算した範囲を書く。行の中の自動計算（`subtotal = quantity * price`）は要素ごとの `define` に下ろし、合計はそれを足す（`examples/purchase.ringi`）。

rulec は、`sum` の範囲が届く範囲より狭くても誤りにせず、範囲の外になる入力を入口で断る。だから、書いた範囲を `sum` に写せば、4.1 の意味がそのまま成り立つ。経費の `amount`（行は 20 行まで、1 行は 100 万円まで、合計は 1,000 万円まで）は、次のように下ろす（`tests/golden/lowered/expense/form.rule`）。

```
elements items
  price : money[JPY]  range >=1JPY <=1_000_000JPY

sum amount over items of price  range >=1JPY <=10_000_000JPY
```

このファイル（と、それが読む `choices.rule`）に、100 万円の行を 10 行と 11 行渡すと、次のようになる。

```text
$ ritsu rulec check form.rule
ok form.rule
$ ritsu rulec run form.rule --input ten.json
amount = 10_000_000JPY
trip_to_shown = true
receipt_required = true
$ ritsu rulec run form.rule --input eleven.json
refused (input): amount is out of range
```

rulec は行の数の上限を知らないので、合計の範囲を行の数の上限から確かめることはしない。

### 4.4 自動計算

`= <式>` で書いたフィールドは、申請者が入れず、式の値になる。式は rulec の式と同じ綴りで、`form.rule` に下ろす。お金と単位の付いた数の割り算や率の掛け算には丸めを書く（`round down(1JPY)`）。書いていなければ E104 で、ringi が式から決める（フィールドの刻みより細かくなりうる式に `round` が無いとき）。下ろすのは ringi の検査が通ってからなので、下ろした規則に rulec の E104 が出るのは ringi が言い漏らしたときだけで、それも E104 として ringi の行に戻す。日付の計算（`back_on = first_day + days business days in office`）は koyomi の日付の式に下ろす（`form.cal`。6.4）。

合計でない計算（`fee = amount * 3%`、行の中の `subtotal = quantity * price`）は `define` に下ろす。範囲を書いた計算は、その範囲を `define` の行の終わりに写す（rulec の `define … range`。rulec の DESIGN §15.313）。rulec はこの範囲を、規則が受け付ける範囲として読み、計算した値が外になる呼び出しを入口で断る。`sum … range`（4.3）と同じ意味なので、書いた範囲は、どの計算でも 4.1 のとおりに守られる。範囲を書かない計算は、範囲の無い `define` のままである。

`tests/machine/ranges.ringi` は、行の中の計算（`fee = price * 2  range <1_500JPY`）、合計（`total = sum(lines.price)  range >=10JPY <2_000JPY`）、合計からの計算（`double = total * 2  range >20JPY <=3_000JPY`）を持つ。次のように下りる（`ritsu ringi extract` が書く `form.rule` から）。

```
elements lines
  price : money[JPY]  range >=1JPY <=1000JPY

define fee : money[JPY] = price * 2  range <1500JPY

sum total over lines of price  range >=10JPY <2000JPY

define double : money[JPY] = total * 2  range >20JPY <=3000JPY
```

入口の断りは、下ろした規則の名前で言う（`fee is out of range`）。チケットの機械と `simulate --form` は、これをフィールドの名前と書いた範囲の文に直し、`form` の種類で断る（7.3）。文の終わりに、申請者が直すフィールドを挙げる。挙げるのは、計算が読むフィールド（種類の説明の `reads`。12.2）で、ファイルが書いた名前で言う。どれも同じ明細のセルなら、明細の名前を一度だけ添える。画面の検査も、同じフィールドを見る人の言語の題で挙げる（13.7）。

```text
$ ritsu ringi simulate tests/machine/ranges.ringi --requester e1201 --on 2026-11-16 --form 'rate = 5, lines = [{price: 800JPY}, {price: 500JPY}]'
error: the `lines[0].fee` the form works out is outside its range (`<1500JPY`); change `price` (`lines`)
$ ritsu ringi simulate tests/machine/ranges.ringi --requester e1201 --on 2026-11-16 --form 'rate = 5, lines = [{price: 800JPY}, {price: 500JPY}]' --lang ja
エラー: 計算した `lines[0].fee` が、範囲（`<1500JPY`）の外です。`lines` の `price` を直してください
```

**生成するコードが読む写し**（2026-11-15）。生成するコード（12.1）は `form.rule` でなく、その写しの `values.rule` を読む。写しは、計算と合計に書いた範囲を `define` と `sum` の行に写さず、届く範囲で読んだ値の最大と最小の行も持たない。合計の範囲は、行の数が 0 から上限までのどれでも届く範囲にする（経費なら `sum amount over items of price  range >=0JPY <=20_000_000JPY`。行の数の下限から数えれば `>=1JPY` になる）。画面が、行の数が下限に届かないあいだも、そのときある行の合計を求めるからである（13.7）。計算した値を書いた範囲と比べるのは、申請ごとのつなぎで、外なら上と同じ文の `form` の種類で断る。検査と参照インタプリタは、範囲を写した `form.rule` を読む。二つが同じ答えを返すことは、ベクタの全部で確かめる（10.4、12.1）。

写しを分けたのは、生成するコードが、計算した値を範囲と比べる前に求めるからである。範囲を書いた計算でも、読むフィールドの範囲から求めた値が int64 を越えうるなら、生成するコードは計算の途中で溢れる。`form.rule` の検査は各値を書いた範囲の中で読むので、これを言わない。写しを rulec にかけると、溢れうる計算を E108 で言い、ringi はそれを E201 にして、計算するフィールドの行に、届く値と、狭める読むフィールドを添えて出す（9.2）。検査が通らないので、`ritsu gen` も止まる。写しの警告と、`form.rule` が通らないときの写しの診断は出さない。`form.rule` が同じことを言うからである。

行は 30 行まで、1 行は数量 100 万まで、単価 1 兆円までで、行の小計と合計に `range >=1JPY <=100_000_000JPY` を書いた申請（`crates/ritsu/tests/gen_requests.rs` の `big.ringi`）：

```text
$ ritsu ringi check big.ringi
error[E201]: big.ringi:8:1: What the computation of the field `amount` is lowered to for the generated code does not pass rulec's check: from the ranges of the fields it reads, `amount` can come to 30_000_000_000_000_000_000JPY, past int64
     8 |   amount : money[JPY] "Amount" = sum(lines.subtotal)  range >=1JPY <=100_000_000JPY
  = The generated code works out what the form computes before it holds each value to the range its field writes; the check and `ritsu ringi run` read each value within that range.
  = Narrow the ranges of the fields `amount` reads (`lines.quantity`, `lines.price`) so that what it can come to fits in int64.
  = From rulec: E108 at line 15 of the lowered `values.rule`: Cannot prove that amount fits in int64
```

計算するフィールドを読む所は、計算の届く範囲と書いた範囲の重なりで読む（4.1）。`route.rule` のように、その値を入力として受ける規則では、それが入力の範囲になり、入口が外を断る。ringi の主張と E103 も同じ範囲で読む。書いた範囲が届く範囲より広い所は、どの申請も取らないので、経路の表に行は要らない。

計算した値に範囲を守らせる書き方は、ほかにも比べた。行は 20 行まで、1 行は 100 万円まで、書いた範囲は 1,000 万円までとして、どれも意味が違った。

| 形 | rulec の答え | 使わない理由 |
|---|---|---|
| `derive subtotal : money[JPY] = quantity * price  range >=1JPY <=10_000_000JPY` | E112（届く範囲を含まない） | 範囲を出うることを誤りにする。4.1 の意味と逆になる |
| 出力の行の `range >=1JPY <=10_000_000JPY` | E140（同じ理由） | 同じ |
| `constraint quantity * price <= 10_000_000JPY` | E017（両側に書けるのは入力の名前と、`+` でつないだ入力の和だけ） | 掛け算も定数も書けない |
| 計算した値を、範囲を付けた入力にする | 通る。入口は範囲の外を断る | 式が規則から消え、表の検査も生成するコードも、値が式から決まることを知らない |
| 範囲を付けない `define` | 通る。`quantity=20 price=1000000` に `subtotal = 20000000JPY` を返す | 入口が範囲の外を断らない |
| `define … range`（選んだ形） | 通る。同じ入力を `refused (input): subtotal is out of range` で断る | |

E105 の問い（4.2）は、計算するフィールドを、届く範囲と書いた範囲の重なりの中の値として尋ねる。ただし、条件が、計算の読む値も読むとき（`required when fee > 900JPY` と `shown when amount > 30_000JPY`）は、二つを別々の値として尋ねると、起こりえない組み合わせ（`fee = 901JPY`、`amount = 1JPY`）を例にしてしまう。そのときは、合計でない計算を `define` のまま問いの規則に入れ、書いた範囲は問いの条件に足す（例の申請には、計算した値も添える）。rulec は、表が `define` とその読む値を一緒に読む行を、セルの値の上で計算し直して読む。この例では、`fee > 900JPY` と `amount <= 30_000JPY` の行（必須なのに見せていない行）に届く入力が無いと示す（rulec の E102。`fee > 900JPY` は `amount > 30_033JPY` と同じである）。ringi はその行を問いから除いて尋ね直し（6.4）、問いの答えは「必須なのに見せていない申請は無い」になって、何も出ない。条件を `required when amount > 30_033JPY` と書いても、同じく何も出ない。

rulec が、届く入力を見つけられず、無いとも示せない行が残ると、答えは決まらない。そのときは W201 で、決められないと言う（主張を決められないときと同じ。9.5）。rulec は、整数の上の一次の形でない計算を、届く範囲の幅で読むので、整数だけが外す値は決まらない。`spread = asked * asked + left * left` を必須の条件（`spread > 15 and spread < 17`）が読み、`asked` と `left` を表示の条件（`asked >= 4 or left >= 4`）が読む申請（`tests/mutants/W201_a_question_left_open.ringi`）では、見せていない申請（`asked` も `left` も 3 以下）の `spread` は 0、1、2、4、5、8、9、10、13、18 のどれかで、16 にはならないので、必須にならない。けれども 16 は、そのときの `spread` の届く範囲（0 から 18）の中にある。

```text
$ ritsu ringi check tests/mutants/W201_a_question_left_open.ringi
warning[W201]: tests/mutants/W201_a_question_left_open.ringi:8:45: Whether the field note may be asked for while it is hidden cannot be settled
     8 |   note   : attachment "Note"  required when spread > 15 and spread < 17  shown when asked >= 4 or left >= 4
  = Whether there is a request where `required when` holds and `shown when` does not, rulec could not settle. This can happen when the two conditions read a computed value and a value the computation reads, one each.
  = If both conditions can be written over the same value (the value the computation reads, instead of the computed value), rewrite them so.
ok tests/mutants/W201_a_question_left_open.ringi: a request, 4 fields, 1 stage
  may never end: the stage `check` has no deadline (`due in …`), and waits until it is answered
```

必須の条件を `spread > 15 and asked >= 4` と書くと、必須の行は表示の条件の偽の側（`asked` が 3 以下）と重ならないので、rulec は届かないと示し、何も出ない。

### 4.5 条件

`shown when`、`required when`、ステップの `when` は、比べる `is`、`is not`、`>`、`<=` などと `and`、`or`、`not` で書く。使う規則の真偽の出力（`when authority.board`）も条件にできる。条件は、下ろした規則の真偽の出力になる。条件の片側（真になる場合）だけを書けば、ringi が残り（偽になる場合）の行も作るので、rulec の完全性の検査が、穴の無い条件であることを確かめる。

条件が読めるのは、表の列にできる物である。フォームのフィールドのうち、自分の値か規則の列挙から選ぶ選択、数と金額、日付、真偽（計算するフィールドは、フォームが計算する数、金額、日付）と、使う規則の出力、共有の経路の入力を、値（`travel`、`30_000JPY`、`2026-04-01`、`true`）と比べる。値を先に書いた比べ（`50_000JPY < amount`）は、向きを変えて読む。どのフィールドでも、入れたか（`reviewer is not none`）は読める。値の無いときにどの比べが成り立つかは 4.9 に書いた。選択と比べる値は、その選択の値のどれかでなければ E004 にし、注に値を並べる。規則の列挙から選ぶ選択（`choice from authority.kind`）の値は、`use rule` で読んだ規則から ringi が読み、自分の値の選択と同じに確かめる。経路の表のセル、例のセル、シナリオのフォームの値も同じである。

ほかの物を読む条件は、その条件の場所で E309 にする。申請者（`requester.department`、`requester.id`）、チケット（`ticket.round`）、文や人やマスタから選ぶフィールド、数でも日付でもない計算するフィールド、二つの値どうしの比べ（`amount > limit`）、条件の中の計算（`sum(items.price) > 100_000JPY`）である。表のセルは一つの列を値と比べる物なので、これらは列にならない。値の無いことがある物（空のことがあるフィールド、`none` になることのある規則の出力 `decider : decider?`、共有の経路の入力のうち申請が渡さない物）は、値の無いことを `none` という値の一つとして読むので、どれも読める（4.9）。経路の条件なら、黙って経路を下ろさずにおくと、検査を通ったのに経路の無い申請の種類ができ、`simulate` は「ステップが無い」と言い、チケットの機械は経路を持たない。比べる値を書き換えて下ろす（`amount is limit` をどの申請も来ないステップにする）と、別の意味になる。どちらの道も残さない。E309 の注は、書き換え方を添える。数どうしの比べと条件の中の計算は、計算するフィールドに置いて値と比べる（`difference : money[JPY] = amount - limit` として `when difference > 0JPY`。このフィールドは、フォームが計算して経路に渡す）。文のフィールドで分けるなら `choice` にする。申請者で分けたい場合のうち、出せる人を絞るのは `ticket` の `submitted by`（5.13）、いちばん上の人の申請の承認者を決めるのは `when empty escalate, then <決め方>`（5.4）である。日付の差、二つの選択どうし、人やマスタから選ぶフィールドには、今は書き換え方が無い（PLAN §17）。

`only when` の主張の条件も、経路の条件と同じに読む（主張は経路の入力の上で確かめるため）。

### 4.6 マスタ

マスタは `masters` のファイルに、キーの列、フィールド、データの置き場を書く。データの CSV は、フィールドごとの列と、`valid_from`、`valid_to` の列を持つ。列の名前は、フィールドの書いたままの名前か、その ASCII の名前である。一つのフィールドの列を両方の名前で書けば、どちらの値を読むかが決まらないので、二つの列の名前を挙げて E501 にする。`optional` のフィールドは列を省ける。省けば、どの行の値も空（null）である（`organization/columns.md` の「Needed: no」）。`valid_from` と `valid_to` は、その行を選べる最初の日と最後の日で、どちらもその日を含む（空の `valid_from` はどの日からも、空の `valid_to` は終わり無し）。申請を出す日に有効な行だけを選べる。チケットは選んだ行のキーを持ち、ページと画面は出した日の行の文言を見せる。

```
masters company_masters v1
use organization org from "company.ringi"

master systems "Systems"
  key id
  fields
    id    : text "System"  count <=30
    name  : text "Name"  count <=100
    owner : person "Owner"
  data "../masters/systems.csv"
```

表引きは承認者の決め方（`system_owner = lookup m.systems.owner by system`）とフォームの自動計算で使う。マスタのデータは、マスタのファイルを確かめるときに一度だけ読み、それを読む申請の種類はそこから行を受け取る。検査は次のとおり。

- データが読めない（列が足りない、一つのキーに同じ日の行が二つ、フィールドが取らない値）ときは、データのファイルの行で E501。マスタを読む申請の種類は、マスタのファイルが通るのを待つ（E202）。
- 人のフィールド（`owner : person`）の値が、行の有効な日と重なる組織の版にいなければ、行の場所で E505。マスタのファイルが `use organization` で組織を読むときに確かめ、読まないとき（読んだ組織にデータが無いときも）は、確かめていないことを W500 で言う。
- 申請の種類を使う日の範囲（9.6）のどの日にも、選べる行がなければならない。行の無い日があれば E107 で、その日々と、前後の行の有効期間をどう延ばせば埋まるかを言う。

```text
error[E107]: example.ringi:6:3: The master `m.accounts` has no row to choose on 2026-07-01 to 2026-07-31
     6 |   account : choice from m.accounts "Account code"
  = The request is used from 2026-01-01 on, and the field `account` reads the master on each of those days.
  = The row before: `accounts.csv`, line 2 (`5103`, from 2026-01-01 to 2026-06-30); making its `valid_to` 2026-07-31 fills it.
  = The row after: `accounts.csv`, line 3 (`5110`, from 2026-08-01 with no end); making its `valid_from` 2026-07-01 fills it.
```

行の終わりは誤りにしない。どの行も終わるマスタでは、いちばん遅い `valid_to` が、申請の種類を使える最後の日の一つになり、`check` がそれを事実として言う（9.6）。`check --through <日付>` を渡したときだけ、その日より前に終わるマスタが E107 になる。

### 4.7 承認者だけのフィールドと、承認者が申請を直すこと

`approvers only` のフィールドは申請書に出ず、承認カードで承認者が入れる（勘定科目、契約の担当、法務が書くリスク）。`ticket` の `approvers edit <フィールド>` は、承認者が申請者のフィールドを直せるようにする。経路が読むフィールドを承認者が直せるなら W108 で知らせ、直したときは、今のステップより後の経路をその値で決め直す（7.3）。前のステップの答えは変えない。直しで値を空にできるのは、直せるフィールドのうち、どの申請でも必須ではない物である（7.3）。シナリオの行では値に `none` を書き（`edit(note: none)`）、約束の JSON では空にするフィールドの名前を `clear` に挙げる。JSON の値の null は直さず、チケットの値のままにする（12.4）。仮押さえの条件か引数が読むフィールドを直せる形は E405 である（7.6）。

### 4.8 文言

フィールドの題と選択の文言は、ファイルの言語で書く。ほかの言語の文言は `labels <言語>` に書く。英語のファイルに `labels ja` を書けば、日本語の画面とページはそれを使う。日本語の名前で書いたファイル（`expense.ja.ringi`）は日本語の文言を持ち、`labels en` で英語の文言を足せる。キーは `request`（申請の種類の題）、フィールドの名前（明細の列は `<明細>.<列>`）、`<フィールド>.<値>`、`stage <名前>`、`who <名前>`。宣言した言語で、画面に出る物のキーが欠けていれば E703 にし、`ritsu gen --target requests` は画面を作る前に言って、何も書かない。宣言していない言語の画面は、ファイルに書いた題を出す（13.4）。申請の種類の説明（`types/<名前>.json`）は、ファイルの言語を `language` に書く（12.2）。

ほかのファイルから来る項目も、ファイルの言語の題で見せる。規則の列挙から選ぶフィールド（`kind : choice from authority.kind "Kind"`）は、値ごとの題を、`choice` の値と同じにフィールドの下に一行ずつ書く（`goods "Goods and services"`）。値は、規則が書いた名前か、括弧の中の名前で書く。ほかの言語の文言は、選ぶ値と同じに `labels <言語>` の `<フィールド>.<値>` に書く（`kind.goods "物品・サービス"`）。題を書かずに値だけを書いた行は、`choice` の値と同じく、名前をそのまま題にする（通貨のコードのように、名前がそのまま見せる形である値のため）。フィールドの下に行の無い値は、規則が値の後ろに書いた表示名（rulec の `goods "Goods and services"`）を、ritsu の口から受け取って題にする。ただし、規則がファイルの言語で書いてあるときだけである。規則の表示名は、規則を書いた言葉で一つだけ書くからである。規則の言語は、規則の名前が ASCII でなければ `ja`、ほかは `en` と読む（申請の種類が言語を宣言しないときの読み方と同じ。12.2）。例の英語の規則 `authority` の表示名は、英語の `purchase.ringi` なら行を書かなくても題になり、日本語の `purchase.ja.ringi` では題にならないので、日本語の題をフィールドの下に書く。列挙に無い値の題は E004、同じ値の題を二度書けば E003 である。マスタから選ぶフィールドの値はマスタの行で、画面は行を名前の列で見せるので、下に行を書かない（E001）。共有の経路のステップと決め方は、経路のファイルの言語の名前を持ち、ほかの言語の題を経路の `labels <言語>` に書く（5.11）。使う申請の種類は、自分の `labels` に無い言語の文言を、経路の `labels` から取る。

ファイルの言語の題が要るのに無い項目も、E703 にする。規則の列挙の値で、フィールドの下に題の行が無く、規則がファイルの言語で書いた表示名も無い物（フィールドの行で言う）と、申請の種類と違う言語で書いた共有の経路のステップと決め方で、経路の `labels <申請の種類の言語>` に題が無い物（`use route` の行で言う）である。題が無いまま画面を作れば、規則や経路が別の言語で書いた名前が、申請の種類の言語の画面に出てしまう。

### 4.9 値の無い所を読む

値の無いことがあるフィールドは、`optional` と書いたフィールド、`shown when` か `required when` を持つフィールド、承認者だけのフィールド（承認者が入れるまで）である。隠れているフィールドも値を持たない（4.2）。いつも必須のフィールドは、空なら申請を先に断る（7.3）ので、条件と計算が読むときには値がある。計算するフィールドは、値の無いことがあるフィールドを読むなら（ほかの計算を通して読む物も数える）、そのどれかが空のとき値を持たない。

条件は、値の無いことを `none` という値の一つとして読む。

- `is none` は値が無いときに、`is not none` は値があるときに成り立つ。
- `is not <値>` は、値が無いときも成り立つ。`is <値>` と大きさの比べ（`>`、`<=` など）は、値が無いときは成り立たない。
- 表で値の無いことを選ぶのは、`none` と書いたセルである。値を並べたセル（`small, large`）は、選択の値を全部並べても、値の無いことを含まない。`-` のセルと `not:` のセル（`not: large`）は、値の無いことも含む。何も書かないセルは誤りで、注が `-` と `none` を案内する。どれも rulec の表と同じ読み方である。

この読み方は、経路の条件と経路の表、表示と必須の条件、`only when` の主張、E105 の問いのどれでも同じである。使う規則の出力のうち `none` になることのある物（`decider : decider?`）と、共有の経路の入力のうち申請が渡さない物も、値の無いことがある値として同じに読む（4.5）。

計算は次のように読む。

- 合計でない計算（`over = asked - left`）は、読むフィールドのどれかが空なら値を持たない。値の無い計算は、書いた範囲に照らさない。その計算を読む条件は、値の無いフィールドと同じに読むので、`required when over > 0` は成り立たず、`shown when over is none` は成り立つ。
- 合計（`sum(lines.price)`）は、いつも値を持つ。読むセルが空の行は足さない。行の中の計算が空のセルを読むなら、その行の計算は値を持たず、合計はその行を足さない。
- 日付の計算（`back_on = first_day + days business days in office`）は、読むフィールドが空なら値を持たない。

約束の JSON は、値の無いことを、null か、キーを書かないことで言う（12.4）。

**下ろし方**。rulec は、値の無いことがある入力（`size : size?`）を表のセルでは読めるが、計算には使えない（rulec の E103）。ringi は二つの形を使い分ける。

- 条件だけが読むフィールドは、値の無いことがある入力にして、`none`、`-`、`not:` のセルで読む。経路の規則（`route.rule`）と名前の付いた表は計算をしないので、計算するフィールドもこの形で受ける。
- 計算が読むフィールドは、計算をする規則（`form.rule`、`values.rule`、E105 の問いの規則）では、値と、入れたか（`<名前>_given`）の二つの入力にする。空のとき、参照インタプリタとつなぎは、値の所に範囲の中の一つの値（仮の値）を、`<名前>_given` に false を渡す。仮の値は、下限、無ければ上限、無ければ 0 である。単位の無い数は、範囲が許せば 0 でない値にする。つなぎの Rust では、空のとき 0 を答える `match` を、clippy が `unwrap_or_default` に書き換えさせるからである。仮の値を読む答えは無い。その計算を読む条件は、読むフィールドが全部入っているときだけ比べが成り立つ行に下ろし、計算の値は、読むフィールドの一つでも空なら、参照インタプリタとつなぎが値なしにする。
- 値の無いことがある計算は、`define` に範囲を写さない。rulec は入口で範囲に照らすので、仮の値から出た値を断ってしまうからである。範囲には、値のあるときに、参照インタプリタとつなぎが同じ順で照らす（4.4）。
- 明細の行は、空のことがあるセルごとに `<セル>_given` を要素に持つ。合計は、入っていないセルを 0 にする表を通して足す。

`tests/machine/absent.ringi` は、空のことがあるフィールドを条件、計算、経路が読む申請である。

```
form
  kind   : choice "Kind"
    goods   "Goods"
    service "Service"
  size   : choice "Size"  optional
    small "Small"
    large "Large"
  lines  : rows "Lines"  count >=1 <=5
    what  : text "What"  count <=100
    price : money[JPY] "Price"  range >=1JPY <=1_000JPY  optional
  total  : money[JPY] "Total" = sum(lines.price)  range >=0JPY <=5_000JPY
  left   : number "Days left"  range >=0 <=20  optional
  asked  : number "Days asked"  range >=0 <=20
  over   : number "Days over" = asked - left  range >=-20 <=20
  urgent : bool "Urgent"  optional
  stock  : number "Units in stock"  range >=0 <=100  optional
  note   : text "Why it runs over"  count <=100  required when over > 0
  detail : text "Detail of a large one"  count <=100  shown when size is large
  plain  : text "What it is"  count <=100  shown when size is not large
  why    : text "Why no size"  count <=100  required when size is none
  hurry  : text "Why it is urgent"  count <=100  shown when urgent is true
  many   : text "Days to spare"  count <=100  shown when left >= 5 or over is none
  soon   : text "Why so few days are left"  count <=100  required when left < 3
  costly : text "Why it costs over 300 yen"  count <=100  required when total > 300JPY
  refill : text "Why stock up"  count <=100  required when stock < 10
  where  : text "Where the stock is kept"  count <=100  shown when stock is not none

route
| size         | left | over | -> first | check      |
| small, large | >=3  | -    | manager  |            |
| small, large | <3   | -    | manager  | accounting |
| small, large | none | -    | manager  | accounting |
| none         | -    | >0   | manager  | accounting |
| none         | -    | <=0  | manager  |            |
| none         | -    | none | manager  | accounting |
```

`ritsu ringi extract` が書く `form.rule` の一部である。`left` は計算が読むので値と `left_given` の二つになり、条件だけが読む `size`、`urgent`、`stock` は値の無いことがある入力になる。

```
inputs
  asked      : number  range >=0 <=20
  left       : number  range >=0 <=20
  left_given : bool
  size       : size?
  urgent     : bool?
  stock      : number?  range >=0 <=100

elements lines
  price       : money[JPY]  range >=1JPY <=1000JPY
  price_given : bool

table price_counted_by
| price_given | -> price_counted : money[JPY] |
| false       | 0JPY                          |
| true        | price                         |

sum total over lines of price_counted  range >=0JPY <=5000JPY

define over_value : number = asked - left

table note_required
| over_value | left_given | -> note_required |
| <=0        | true       | false            |
| -          | false      | false            |
| >0         | true       | true             |

table why_required
| size         | -> why_required |
| small, large | false           |
| none         | true            |
```

`route.rule` は計算をしないので、同じ `left` と計算の `over` を、値の無いことがある入力で受け、`none` のセルで読む。`over` の値は、参照インタプリタとつなぎが計算して渡し、`left` が空なら値なしにする。

```
inputs
  size       : size?
  left       : number?  range >=0 <=20
  over_value : number?  range >=-20 <=20
```

大きさを選んだ申請は、残りの日数が 3 日より少ないか、入れなければ、経理のステップが来る。`small, large` のセルは、大きさを選ばない申請では成り立たない。大きさを選ばない申請は、日数が超えているときと、残りの日数を入れないとき（`over` が値を持たない。`<=0` のセルは値の無いことを含まない）に、経理のステップが来る。大きいもので、残りの日数を入れない申請は、次のとおり経理のステップが来る。

```text
$ ritsu ringi simulate tests/machine/absent.ringi --requester e1201 --on 2026-11-16 --form 'kind = goods, size = large, lines = [{what: "Shelf", price: 700JPY}, {what: "Screws"}], asked = 1, urgent = true, detail = "A shelf", hurry = "Monday", many = "No end set"'
tests/machine/absent.ringi on 2026-11-16, in the organization from 2026-04-01
  computed: total = 700JPY, over = none
requester e1201 (Sales staff 1)
  first  manager: e1101 (Manager of Section 1A)
         due 2026-11-19 at the end of the day
  check  accounting: one of e1001 (Head of Accounting), e1002 (Accountant)
         due 3 business days after it starts, at the end of the day
```

チケットの機械の試験（`tests/machine.rs`）は、この申請の三つのシナリオ（何も入れない、全部入れる、残りの日数を出し直しで足す）を流し、生成するコードの四つの出力先も、そのベクタに同じ答えを返す（ritsu の `tests/gen_requests.rs`）。画面は、同じ読み方でフォームを決める（13.7）。

## 5. 経路

### 5.1 ステップの種類

| ステップ | 書き方 | 通る条件 |
|---|---|---|
| 承認 | `first  manager` | 承認者のうち一人が承認する。承認者の決め方がチームなら、チームのだれか一人 |
| 並列 | `second  dept_head and accounting`（表のセル）、`board  at least 60% of board`、`at least 2 of legal`、`all of legal` | 議決に届くまで待つ。`a and b` は、a と b のそれぞれから一人ずつの承認 |
| 回覧 | `inform  circulate to general_affairs` | 知らせて読めるようにし、待たずに次へ進む |
| エージェントの答え | 承認者の決め方を `agent <flow>, else <承認者>` にしたステップ | flow の答えが `ok` なら通る。`doubt` か失敗なら `else` の人のステップになる（15.1） |
| 要約 | `summary  note from summarize` | flow を始め、その出力をあとの承認カードに付けて進む。決めない（15.2） |

議決の数は、承認者の数に対して決める。`at least 60% of board` は、委員会の 5 人なら 3 人（切り上げ）。並列のステップで却下が出て、残りの人が全員承認しても議決に届かなくなったら、ステップは通らず、チケットは却下で終わる。届く間は待つ。チケットの機械（`src/lower/ticket.kn`）では、`needed` が要る承認の数を決め、`answer` が、却下のあとで残りの人が全員承認しても届かない部分を見つけて、チケットを却下で終える。

### 5.2 承認者の決め方

`who` に、名前と決め方を並べる。

| 決め方 | 書き方 | 決まる人 |
|---|---|---|
| 人を指定 | `counsel = person "e9001"` | その人 |
| 申請者の上長 | `manager = manager of requester` | 申請者の主の所属のチームの長。長が申請者自身か空なら、上のチームの長へたどる |
| 申請者のチームの長 | `dept_head = head of requester.department` | 主の所属のチームから上へたどって最初の、その種類のチームの長 |
| 申請者に最も近い役職の人 | `division_head = nearest division_head above requester` | 主の所属のチームから上へ、チームの長をたどり、役職がその役職以上の最初の人 |
| チーム | `accounting = team accounting` | そのチームの人（一人の承認で通る。並列の議決ならその数） |
| 申請者が指名 | `reviewer = chosen by requester in reviewer` | 申請者が `person` のフィールドに入れた人 |
| 前の承認者が指名 | `reviewer = chosen by previous approver in reviewer` | 前のステップの承認者が、承認者だけのフィールドに入れた人 |
| マスタから引く | `system_owner = lookup m.systems.owner by system` | フォームの値で引いたマスタの行の人 |
| エージェント | `check = agent precheck_agent, else manager` | dandori の flow が答え、疑いがあれば `else` の人 |
| 規則の出力 | ステップに `authority.decider` と書く | 使う規則の列挙の出力。値の名前が、どれも `who` の名前でなければならない（E203） |

決め方は、ステップが始まるときに、そのときに効いている組織の版で決める（8.6）。決まった人には、そのときに有効な代理（5.8）も加わる。

### 5.3 分岐

分岐は二つの形で書ける。

表の形：列は入力（フォームのフィールドか、使う規則の出力）、`->` のあとがステップ。セルは承認者の決め方の名前で、`a and b` は並列、空のセルはそのステップが無いことを言う。表は rulec の表と同じ綴りで、入力のセルも rulec と同じ（`-` はどの値でもよい）。

```
route
| category | amount                     | -> precheck | first   | second                   | decide        |
| travel   | <=30_000JPY                |             |         |                          | travel_agent  |
| travel   | >30_000JPY <=100_000JPY    |             |         |                          | manager       |
| expense  | <=100_000JPY               | check       |         |                          | manager       |
| purchase | >1_000_000JPY              |             | manager | dept_head and accounting | division_head |
```

ステップの条件の形：ステップを並べ、要るものに `when` を付ける。

```
route
  manager     manager
  owner       system_owner
  security    security  when level is admin
  operations  circulate to operations
```

経路そのものを切り替える分岐は、表の形で、出力の列を `-> route` にし、セルに `use route` の別名を書く（下ろした規則では、値が経路の名前の出力 `route` になる）。どの形も、下ろした規則で rulec が、どの申請にも当てはまる行が一つだけあることを確かめる（E301、E302）。どの申請も当てはまらない行（フォームの範囲の外の行や、ほかの行に覆われた行）は E303、どの申請も届かないステップの承認者は W303 になる。

表の列とステップの条件が読めるのは、4.5 の物である。表の列に申請者、チケット、文や人やマスタから選ぶフィールドを書くと、列の見出しの場所で E309 にする。列の見出しに規則の別名だけを書いたもの（`authority`）、無い出力（`authority.nothing`）、フィールドの中の名前（`amount.x`）は E004 にする。検査を通った申請の種類と共有の経路には、いつも下ろした経路がある（`tests/lowered.rs` が、木の中の検査を通るどのファイルについても確かめる）。

経路の例（`examples`）も表の形で書く。列は経路の入力、`->` のあとがステップで、行ごとに、その入力の申請がどのステップにだれを置くかを書く。経路が読む列の行は、下ろした規則で rulec が表と突き合わせ、答えが違えば E601 にする（9.2）。経路が読まない列（文やマスタから選ぶフィールドなど、経路が読めない列も）は、下ろした例から外す。

例の行は出せる申請なので、セルにはフォームが取る値を書く。範囲の外の数と日付（計算するフィールドは、計算がとりうる値の外）、単位の刻みの倍数でない数（円の金額の `60_000.5JPY`）、長さが `count` の外の文、マスタのどの行でもないキーは、そのセルで E601 にし、選べない値は E004、型の違う値は E009 にする。経路がその列を読むかどうかにかかわらず同じことを同じ場所で言い、一つでもあれば経路を下ろさない（`tests/example_rows.rs` が、同じ宣言の二つの列の、読む側を入れ替えて確かめる）。例の行は日を持たないので、マスタのキーは、どれかの日に選べる行なら通す。

```text
error[E601]: example.ringi:21:15: `99` is outside the range of `count` (`>=1 <=10`)
    21 | | 50_000JPY | 99    | manager   |
  = A row of the examples is a request: write values the form takes, whether the route reads the column or not.
```

### 5.4 承認者がいないとき

ステップの承認者が、決めたあとで一人もいないことがある（チームの長が空いている、チームに人がいない、指定した人が組織にいない、自己承認を許さない経路で申請者しかいない）。`when empty` で扱いを選ぶ。

- `stay`（書かなければこれ）：チケットはそのステップで待つ。管理する人が承認者を足せる（5.9）。
- `escalate`：上へ回す。決め方が止まったチームの上へ、チームの長をたどり、外す人（申請者と、代わりに出した人）でない最初の長に回す。チームなら、そのチームの上のチームから。決め方がチームに着かなかったとき（上に長がいない、その種類のチームが上に無い）は、申請者の主の所属のチームの上から。組織にいない人とチームを指定したステップは上へ回せないので、検査がそれを言う（E503）。
- `skip`：飛ばして次へ進む。履歴に「承認者がいないので飛ばした」と残す。

組織のいちばん上より上には回す先が無い（社長の申請の、上長のステップなど）。そこでどうするかを、`escalate` のあとに `then` で書く。

| 書き方 | いちばん上でのふるまい | 検査 |
|---|---|---|
| `when empty escalate, then skip` | ステップを飛ばす。履歴に「上に回せる人がいないので飛ばした」と残す | 組織の形なら `ok` の下で数える。上の長の席が空いているなら W502。どのステップにも答える人がいなければ W510 |
| `when empty escalate, then board` | `who` に宣言した承認者の決め方 `board` を申請者について解き、その人へ回す。履歴に「上に回せる人がいないので `board` へ回した」と残す | 組織の形なら `ok` の下で数える。上の長の席が空いているなら W502。`board` にも人がいなければ E502 |
| `when empty escalate, then stay` | そのステップで、管理する人が承認者を足すまで待つ | E502 |
| `when empty escalate` | `then stay` と同じ。黙って飛ばさない | E502 |

`then` のあとに書けるのは、`skip`、`stay`、承認者の決め方の名前である。決め方は、`when empty` の行を書いたファイルの `who` に宣言したもので（`every stage` の行なら、その申請の種類か共有の経路の `who`）、無ければ E004 になる。`skip` と `stay` は決め方の名前にできない（E002。3.4）。`then` の決め方は一度だけ解く。その決め方にも人がいなければ、さらに上へは回さず、チケットはそのステップで管理する人を待つ（E502）。上へ回すのは上に人がいる間ずっとなので、`when late` の `escalate, then escalate` と違って二度目は無く、`then escalate` と書けばその場で E001 になる。`, then …` は `escalate` のあとにだけ書ける（`when empty stay, then skip` も E001）。

いちばん上の人の申請を飛ばしてよいかは、申請の種類で決まる。だれかが見るべきなら、`then` のあとに、その人の申請に答える決め方を書く（`who` に `board = team board` と宣言して `when empty escalate, then board`）。経路の条件は申請者を読めず、表も申請者を読まないので、いちばん上の人の申請にだけ別の承認者を決めるのは、この書き方である。`examples/` の例は、どれを選んだかと、社長の申請をだれが見るかを、例の中の注に書いた。経費は、上の人に聞くステップを取締役会へ回し、取締役のだれかが答える（`every stage` の `when empty escalate, then board`）。購買は、決裁のステップを取締役会へ回し（決裁の行の `then board`）、上長のステップは飛ばす。アカウントと契約は、上長のステップを飛ばしても、ほかのステップ（システムの持ち主、情報セキュリティ、法務）が見る。休暇の共有の経路は、いちばん上の人の申請では、どちらのステップも飛ばす（休暇そのものは、取締役会の人が出さない。5.13）。

検査は、組織のデータがあれば、版ごとに、どの申請者の、どのステップが空くかを一覧にする（8.6）。チケットがそのステップで止まるもの（`stay`、いちばん上で `then skip` も `then <決め方>` も無い `escalate`、いちばん上で `then` の決め方にも人がいない `escalate`）は E502 である。E502 の直し方は、`then skip` と書くことと、`then` のあとに決め方を書くことの両方を言う。

```text
$ ritsu ringi check tests/mutants/E502_escalate_with_nobody_above.ringi
error[E502]: tests/mutants/E502_escalate_with_nobody_above.ringi:9:10: In the organization from 2026-04-01, the stage `check` has no approver
     9 |   check  manager
  = `manager` finds no head above the requester.
  = For 1 requester: e01 (President (fictional)).
  = The stage is `when empty escalate`, and nobody is above (the top of the organization): a ticket that comes to it waits there until an administrator adds an approver.
  = If the stage may be skipped for them, write `when empty escalate, then skip`; if someone must see their requests, write after `then` the way to choose who answers them (`when empty escalate, then board`, with `board = team board` in `who`).
$ ritsu ringi check tests/mutants/E502_the_way_after_then_finds_nobody.ringi
error[E502]: tests/mutants/E502_the_way_after_then_finds_nobody.ringi:9:10: In the organization from 2026-04-01, the stage `check` has no approver
     9 |   check  manager
  = `manager` finds no head above the requester.
  = For 1 requester: e01 (President (fictional)).
  = The stage is `when empty escalate, then auditor`, and nobody is above (the top of the organization): it goes to `auditor`, but `auditor` names e09, who is not in the organization from 2026-04-01. A ticket that comes to it waits there until an administrator adds an approver.
  = Correct the data of the version, or write after `then` a way that has someone in every version.
```

ほかへ回るものと飛ぶもの（上へ回せた `escalate`、いちばん上で `then skip` か `then <決め方>` の `escalate`、`skip`）は、承認者がいない理由で分ける。

- 組織の形によるもの：承認者が申請者だけで、自己承認を許さない（部長自身の申請の、部長のステップ）。申請者が組織のいちばん上にいる（社長の申請の、上長のステップ）。決め方の種類のチームが上に無い（社長の申請の `head of requester.department`）。どれも、ステップは書いたとおりに回るので、警告にしない。`ok` の行の下に、版ごとに、ほかへ回るか飛ばされる申請者の数を書き、だれの、どのステップがどこへ回るかは `ringi simulate --every-requester` で見せる（10.3）。その行は、コマンドを、検査を走らせたとおりの名前で書く（`ritsu ringi check` と `ritsu check` では `ritsu ringi simulate …`）。数えるのは、申請を出せる人（5.13）の人数である。JSON の `facts` には、一件ずつ、版、ステップ、何が起きるか、理由、申請者の全部を入れ、ページが一覧にできるようにする（16.2）。
- データの欠けによるもの：長の空いたチーム、人のいないチーム、版にいない人かチーム、上へたどる途中の空いた長の席。これは W502 で、注に欠けを言う。空いた席を書いたとおりに埋めてよいのか、データの誤りなのかは、人が決めることだからである。上の長の席が空いているので、いちばん上と同じく `then` の書き方が効くときは、「組織のいちばん上」とは言わず、空いた席を言う。

W502 は、上へ回すステップなら回す先の人ごとに申請者をまとめて言い、`then` の決め方へ回るなら、その決め方の人を注に並べる。2027 年の版では法務部の長が空き、部長のステップが本部長へ回る（`ritsu ringi check examples` の出力から）。

```text
warning[W502]: examples/expense.ringi:33:93: In the organization from 2027-04-01, the stage `decide` goes elsewhere or is skipped for lack of an approver
    33 | | travel   | >100_000JPY                |             | manager |                          | dept_head     |
  = `dept_head` comes to the team legal (Legal), whose head is empty.
  = The stage is `when empty escalate, then board`, and it goes up.
  = It goes up to e0102 (Head of Corporate Division) for 2 requesters: e9002 (Lawyer 2) and e9003 (Lawyer 3).
  = Correct the data of the version, or make sure this is meant.
  = The same on the rows of lines 33, 35, 37.
```

組織の形でほかへ回るステップは、同じ経費の申請の `ok` の下で数える。社長の申請の上長のステップが取締役会へ回るのも、部長自身の申請の部長のステップが上へ回るのも、ここに入る。

```text
ok examples/expense.ringi: a request, 8 fields, 4 stages
  used from 2026-04-01: the first version of the organization `shared/company.ringi` (data `../org`)
  used with no end: no data it reads ends
  ends within 93 business days at the most (4 × 21 at its stages, 3 × 3 with the requester)
  requesters whose stages (`precheck`, `first`, `second`, `decide`) go elsewhere or are skipped by the shape of the organization: 14 of 24 in the organization from 2026-04-01, and 13 of 23 from 2027-04-01
  requesters whose late stages (`second`) have nowhere to go up to by the shape of the organization: 1 of 24 in the organization from 2026-04-01, and 1 of 23 from 2027-04-01
  who they are, and where their stages go: `ritsu ringi simulate examples/expense.ringi --every-requester`
```

一つ一つのステップが書いたとおりに飛ばされても、ある申請者では、どのステップにも答える人がいないことがある。社長の申請で、ただ一つの上長のステップを `then skip` で飛ばすときである。そのチケットは、出したとたんに、だれも答えないまま承認で終わる。これは黙って通さず、W510 で言う。言う場所は最初の承認のステップの行で、飛ばされるステップの組ごとに申請者をまとめる。エラーにしないのは、社長の申請はだれも見ずに通す、と決めている会社もあるからである。直し方として、`then` のあとに答える人の決め方を書くこと（`who` に宣言した名前を書く）と、その人が出さない申請なら `submitted by … except …` で外すことを言う。

```text
$ ritsu ringi check tests/mutants/W510_nobody_answers_the_president.ringi
warning[W510]: tests/mutants/W510_nobody_answers_the_president.ringi:9:10: In the organization from 2026-04-01, no stage of the request has anyone to answer it for some requesters
     9 |   check  manager
  = `check` is skipped for lack of an approver, and no other stage has anyone to answer it.
  = A ticket of theirs is approved as soon as it is submitted, with nobody answering it.
  = For 1 requester: e01 (President (fictional)).
  = If someone must see their requests, write after `then` the way to choose who answers them (`when empty escalate, then board`, with `board = team board` in `who`), or correct the data of the version; if they do not make the request, leave them out with `submitted by … except …`.
ok tests/mutants/W510_nobody_answers_the_president.ringi: a request, 1 field, 1 stage
  used from 2026-04-01: the first version of the organization `shared/small.ringi` (data `org-small`)
  used with no end: no data it reads ends
  may never end: the stage `check` has no deadline (`due in …`), and waits until it is answered
  requesters whose stages (`check`) go elsewhere or are skipped by the shape of the organization: 1 of 4 in the organization from 2026-04-01
  who they are, and where their stages go: `ritsu ringi simulate tests/mutants/W510_nobody_answers_the_president.ringi --every-requester`
```

条件で来るステップ（`when`）は、来るかもしれないし、来ないかもしれないものとして読む。条件の無いステップがどれも飛ばされ、答える人がいるのは条件で来るステップだけ、という申請者にも W510 を言う。そのステップが来ないチケットは、だれも答えないまま承認で終わるからである。`tests/machine/desk.ringi` は、上長、取締役会（10 万円を超えるとき）、部長の三つのステップを持ち、社長の 9 万円の申請は、出したとたんに承認で終わる。

```text
$ ritsu ringi check tests/machine/desk.ringi
warning[W510]: tests/machine/desk.ringi:13:11: In the versions of the organization from 2026-04-01 and 2027-04-01, no stage of the request has anyone to answer it for some requesters
    13 |   first   manager
  = `first` and `decide` are skipped for lack of an approver, and a ticket that `board` does not come to (a condition decides it) has no stage with anyone to answer it.
  = Such a ticket of theirs is approved as soon as it is submitted, with nobody answering it.
  = For 1 requester: e0001 (President (fictional)).
  = If someone must see their requests, write after `then` the way to choose who answers them (`when empty escalate, then board`, with `board = team board` in `who`), or correct the data of the version; if they do not make the request, leave them out with `submitted by … except …`.
…
```

飛ばされるステップが一つも無い経路（条件だけで、どのステップも来ない）は、申請者によらないので、この診断にしない。

`when empty` は、下ろした規則と日付のどれにも出ない（`route.rule` が決めるのは、ステップに答える承認者の決め方の名前までである）。チケットの機械は、構文木の期限の行 `Deadline::Empty`（`WhenEmpty::Stay`、`WhenEmpty::Skip`、`WhenEmpty::Escalate(None | Some(AtTop::Stay) | Some(AtTop::Skip) | Some(AtTop::To(<決め方の名前>)))`）から読む。ステップの行の下に書いた物があればそれを、無ければ `every stage` の物を使う。共有の経路を使う申請では、申請の `every stage` があればそれが、無ければ経路の物が効く（5.11）。`then` の決め方は、効いている行を書いたファイルの `who` から引く。

### 5.5 自己承認

`self approval not allowed`（書かなければこれ）では、申請者（代理申請なら、代わりに出した人も）は承認者から外れる。承認者の代理（5.8）としても答えないので、`simulate` も、その人を代理として並べない。外したあとにだれもいなければ、5.4 のとおり。`allowed` なら外さない。会社全体の決まりは sekisho の gate に書き、ringi はステップごとに `self_approval` の値を gate に渡す。経路の設定と gate が食い違えば ritsu の E911（X18、9.3）。

### 5.6 差し戻しと再申請

`send back to the requester, at most N times` は申請者へだけ、`send back to any earlier stage, at most N times` は申請者か前の任意のステップへ戻せる。申請者へ戻すと `round` が一つ増え、再申請で最初のステップから進め直す。前のステップへ戻すと、そのステップから答えを空にして進め直す（`round` はそのまま）。戻せるのは、そのチケットでだれかが答えたステップである。回数は、承認者の差し戻し（申請者へも、前のステップへも）と申請者の取り下げ（5.7）を合わせて数え、上限に達したら、承認者は差し戻せない（承認か却下を選ぶ）。

`resubmit within N business days` は、申請者へ戻ってから再申請するまでの期限で、戻った日（戻った時刻を、数えるカレンダーのオフセットで読んだ日。7.5）から N 営業日の日の終わりである。過ぎればチケットは期限切れで終わる。再申請では、変えるフィールドだけを渡し、ほかは前の値のままになる。値を空にするフィールドは、シナリオの行では値に `none` を書き（`resubmit(note: none)`）、約束の JSON では名前を `clear` に挙げる（7.3）。JSON の値の null は、前の値のままにする（12.4）。渡した値で見せなくなったフィールドの値は落とす（4.2）。申請者（代理申請なら、代わりに出した人も）が、そのときに効いている組織の版にいなければ、再申請も破棄もできず、チケットは再申請の期限で切れる（7.7）。

### 5.7 取り下げ、却下、承認の取り消し、保留

- **取り下げ**：`withdraw until <ステップ>` は、そのステップが答えるまで、申請者が取り下げられる。`until it is decided` は、終わるまで。取り下げたチケットは申請者の手元へ戻り（差し戻しと同じ状態）、直して再申請するか、やめる（`discard`。取り下げで終わる）かを選ぶ。保留中のチケットも取り下げられる。差し戻しと取り下げの回数を使い切ったチケットと、`send back` の行が無い申請の種類のチケットは、取り下げると手元へ戻らずに、取り下げで終わる（7.3）。
- **却下**：承認者が却下すると、チケットは却下で終わる。直しても出し直せない。議決のあるステップでは、却下が重なって、もう議決に届かなくなったときに終わる（7.3）。
- **承認の取り消し**：`retract approval until the next stage answers` は、承認した人が、次のステップがまだ答えていない間だけ、承認を取り消せる。チケットはそのステップへ戻る。ステップの期限は始まったときのまま動かさない。期限をもう過ぎていれば、戻ったとたんに期限の時計が鳴る（7.6）。
- **保留**：`pause at most N business days` は、承認者が今のステップを保留にして、催促と期限の時計を止められる。保留は一つのチケットで合わせて N 営業日までで、保留した日（保留した時刻を、数えるカレンダーのオフセットで読んだ日）から数え、尽きれば自動で戻る（`pause ends`）。戻ったステップの催促と期限は、止めていた営業日だけ後ろへずれる（オフセットの違うカレンダーでのずらし方は 7.5）。保留のあいだも、申請者は取り下げられ、管理する人は承認者を足せ、flow は答えを返せる（7.2）。

期限を始まったときのまま動かさないことと、保留の合計に上限を置くことで、どのチケットも終わる（7.6）。

### 5.8 代理

代理は、組織のデータの置き場の直下の `proxies.csv` に書く。列は、代わられる人（`person`）、代わる人（`proxy`）、種類（`kind`。`approve` か `submit`）、期間（`from` と `to`。どちらもその日を含み、空の `to` は終わり無し）、申請の種類（`requests`。申請の種類の ASCII の名前を空白で区切って並べ、空ならどれも。日本語の名前で書いた `経費(expense_ja)` は `expense_ja` と書く）。代理は版をまたいで効く。例は、英語の名前の版と日本語の名前の版の両方を並べる。

```
person,proxy,kind,from,to,requests
e3101,e1101,approve,2026-12-28,2027-01-08,expense expense_ja purchase purchase_ja
e1203,e1201,submit,2026-12-01,2026-12-31,leave leave_ja
```

代理承認は、答えるときに有効な代理で決める。代理は、決め方が選んだ人の横に加わる（`simulate` は `p03 (Manager of Sales 1) [p02 (Head of Sales) too, as the proxy of p03]` と見せる）。承認者のいないステップを埋めはしない（5.4 のとおりに扱う）。

答えは、承認者の席に数える。答えた人自身が承認者なら、その人が承認者である部分のどれにも、自分の席で数える。承認者でなければ、有効な代理として、まだ答えていない最初の承認者の席に数える。一度の答えで使う代理の席は一つで、一つの席は一度しか答えない（代わられた人と代わる人の両方が答えても、一票である）。代理は、保留と再開、承認者が直すフィールドの `edit`、自分が代わりにした承認の取り消し（`retract`）もできる。申請者の側の操作（取り下げの `withdraw` と、差し戻しか取り下げのあとの `resubmit` と `discard`）は、申請者の側の人（申請者、代わりに出した人、その日に有効な `submit` の代理）ができる。例の gate も、取り下げとコメントを、申請者の側の人に許す（12.4）。

代理がしたことは、履歴のどの行にも、だれの代わりにしたか（`proxy_of`）を残す（「e1101 が e3101 の代わりに承認」。7.4）。承認者の側では、代理が使った承認者の席で、申請者の側では申請者である。

代理申請では、申請者は代わられた人で、代わりに出した人も履歴に残し、自己承認の判定ではどちらも申請者として扱う。検査の一覧（8.6）と `simulate --every-requester` は、代理申請で出せる申請の形（代わられた人と、代わりに出した人の組）も申請者として数え、`p04 (Sales staff) [made by p06 (Accountant)]` と見せる。

代理の行は、行の期間（`from` から `to` まで）と効く日が重なる組織の版の一つずつで確かめる。代わられる人か代わる人が、その版にいなければ、行の場所で E505 にする（辞めた人の代理）。ほかの版にいても、いない版があれば E505 である。いない人と版の組ごとに一つ言い、切り替えの前の日々を残す直し方を添える。行の期間と重ならない版は見ない。

```text
error[E505]: org/proxies.csv:2:1: `p03`, the proxy of this row, is not in the organization from 2027-04-01
     2 | p02,p03,approve,2027-03-20,2027-04-10,
  = The proxy is in effect 2027-03-20 to 2027-04-10, which overlaps the days of the organization from 2027-04-01.
  = If the person has left, make this row's `to` 2027-03-31, the day before the organization from 2027-04-01; if the ID is wrong, correct it.
```

### 5.9 管理する人の操作

申請の種類を受け持つ部門の管理する人は、今のステップに承認者を足せる（`add approver`）。承認者の一人の受け持ちを、ほかの人に引き継げる（`hand over`）。どちらもイベントで、gate の `add_approver` と `hand_over` が、だれができるかを決める。期限は動かさない。どちらのイベントも、操作した人（`by`）を持ち、履歴の行に残す（12.4）。

足すのは、承認者の足りない部分を動かすためでもある。契約の `a_lawyer_after_the_team_shrinks` では、法務のチームが小さくなった版で、法務の人が自分の契約を出す。二人の承認が要る `second` のステップには、申請者を外すと一人しか残らず、部分は止まる（`stopped`）。管理する人が承認者を足すと、部分はまた待ち、二人の承認で進む。

### 5.10 期限と催促

`every stage` は、どのステップにも効く期限と催促を書く。ステップの行の下に字下げして書けば、そのステップだけ変えられる（`purchase.ringi` の `board`）。

```
every stage
  due in 3 business days at end of day
  remind after 2 business days at 09:00
  when late escalate by next_up, then expire
  when empty escalate
```

- `due in N business days at end of day` は、ステップが始まった日から N 営業日の日の終わり。`at 17:00` のように時刻も書ける。営業日は `use calendar` のカレンダーで数え、始まった日も時刻も、そのカレンダーの UTC オフセットで読む（7.5）。
- `remind after N business days at 09:00` は、その日時に承認者へ一度知らせる。催促は期限の前の知らせで、期限より前に来ない催促は置かない（`due in 1 business day at 09:00` のステップの `remind after 1 business day at 10:00`）。主張 `every stage reminds before it is due` を書けば、そういう催促を検査が E403 で言う。催促は、期限と別のカレンダーで数えてもよい（`remind after 1 business day in plant at 09:00`。下の「催促と期限を別のカレンダーで数えるステップ」）。
- `when late` は、期限が過ぎたときの扱い。`escalate` は上へ回し、回した先に新しい期限と催促を付ける。どちらも、元の期限が切れた時刻から、ステップの始まりから数えるのと同じ営業日数で数える（サーバーが期限のイベントを遅れて渡しても延びない。7.5）。回した先の人には、上へ回った知らせのあとに、催促が一度届く。二度目に回したあとも同じである。まだ答えていない部分だけが上へ回り、済んだ部分の答えは残る。`by <表>` は、表で回す先の承認者を決める（`table next_up`）。`then expire` は、回した先でも過ぎたら期限切れで終わる。`expire` はすぐ期限切れ、`stay` は待ち続ける（W402）。

名前の付いた表（`table next_up`）の入力の列は、名前がフォームのフィールドなら、そのフィールドの値を書く列で、そうでなければ承認者の決め方の名前を書く列である（`next_up` の `approver` は、期限が過ぎたステップの承認者の決め方）。`->` のあとの列には、承認者の決め方の名前を書く。承認者の決め方の名前が `who` に無ければ E004 になる。フィールドと同じ名前の列の行に承認者の決め方を書いたとき（フィールド `approver` がある申請の `| approver | -> goes_to |`）は、E004 にせず、E002 で、列とフィールドが同じ名前だと言う。フィールドの名前はチケットの JSON のキーなので、ほかの名前に置き換えられず、承認者の決め方を書く列のほうに、どのフィールドにも無い名前を付ける（`| current | -> goes_to |`）。

期限を過ぎて上へ回す先は、次のとおりに決める。

- 表が無ければ、期限を過ぎた承認者の主の所属のチームから上へたどり、その人でも外す人でもない最初の長。チームのステップなら、そのチームの上のチームの長。
- `by <表>` なら、表がそのステップの決め方から回す先に決めた決め方を、申請者について解いた人。その決め方がだれにも着かなければ、`when empty escalate` と同じに、止まった所から上へ回す（5.4）。規則の出力の値で決まるステップ（`authority.decider`）は、その値の決め方を表の入力にする。
- `then escalate` の二度目は、一度目に回した先の人から、同じように決める。表があれば、一度目に表が回した先の決め方を、表の入力にする。
- 回した先が期限を過ぎた人と同じ人になる行（`next_up` の `division_head` から `division_head`）は、その人に新しい期限を付けて待つ。検査は、これを言わない。

どこにも回せないとき（期限を過ぎた人が組織のいちばん上にいる、表の決め方にも、その上にも人がいない）は、黙って飛ばしも止めもせず、`escalate` のあとに書いたことを、その期限でする。`then expire` か何も書かなければ期限切れで終わり、`then stay` なら同じ承認者を待ち続け（W402）、`then escalate` で一度目に回せなければ、二度とも回せないので期限切れで終わる。二度目に回せなければ、二度目の期限で期限切れになる。チケットの機械は、履歴に「回す先が無いので期限切れにした」と残す（7.3）。

検査は、組織のデータがあれば、回す先の無いステップを、版ごとに、期限を過ぎる人ごとに申請者をまとめて一覧にする（8.6）。期限を過ぎる人には、承認者がいないので `when empty escalate` で回った先の人（上の長と、いちばん上で `then` の決め方の人）も入り、注に、どう回ったかを添える。表は、上の長にはステップの決め方を、`then` の決め方の人にはその決め方を入力にする（表の入力の列には `who` のどの決め方の行も要るので、`then` の決め方の行がそのまま効く。経費の `next_up` の `board` から `board` は、取締役会が期限を過ぎても、取締役会に新しい期限を付けて待つ）。JSON の `to` は、期限を過ぎる人である（16.2）。回す先の無い理由で、言い方を 5.4 と同じに分ける。

- 組織の形によるもの：期限を過ぎる人が組織のいちばん上にいる（社長が決裁するステップ）。チームの上にだれもいない。表の決め方が、組織の形で人を見つけない（社長の申請で、表が「申請者の上の本部長」へ回す）。どれも書いたとおりに期限で終わるので、警告にしない。`ok` の行の下に、版ごとに申請者の数を書き、JSON の `facts` に一件ずつの中身を入れる。そのステップが期限を過ぎたらどうなるかは、`ringi simulate` が承認者の行に添える（10.3）。
- データの欠けによるもの：期限を過ぎる人の上の長の席が空いている。表の決め方が、データの欠けで人を見つけない。データの欠けで回ってきたステップを、回った先の人が期限を過ぎ、その人の上にだれもいない。これは W507 で、注に欠けを言う。

社長が出した経費の申請（種別が購入）で経理が期限を過ぎると、表は本部長へ回すが、社長の上に本部長はいない。そのときは期限切れで終わる。これは組織の形なので、経費の `ok` の下の「requesters whose late stages (`second`) have nowhere to go up to …」の行に入り、`simulate` は経理の行に添える（`ritsu ringi simulate examples/expense.ringi --requester e0001 --form 'category = purchase, items = [{what: "Server", price: 800_000JPY}]' --on 2026-11-16` の出力から）。

```text
  second  accounting: one of e1001 (Head of Accounting), e1002 (Accountant); when it is late, it cannot go up, and expires at the deadline
```

社長の席が空いた版で、営業部長が決裁するステップが期限を過ぎると、上に回す先が無い。これはデータの欠けなので W507 である。人が答えるステップは、期限を過ぎる人ごとに申請者をまとめる。チームのステップは一人に決まらないので、申請者だけを言う。

```text
$ ritsu ringi check tests/mutants/W507_nobody_above_the_head_of_sales.ringi
warning[W507]: tests/mutants/W507_nobody_above_the_head_of_sales.ringi:10:11: In the organization from 2026-04-01, the stage `decide` has nowhere to go up to when it is late
    10 |   decide  sales_head
  = Nobody is above the one who is late to go up to.
  = The head of the team company (Example Co. (fictional)) is empty.
  = The stage is `when late escalate, then expire`: it cannot go up, and expires at the deadline.
  = Late with e02 (Head of Sales) for 4 requesters: e01 (President (fictional)), e02 (Head of Sales), e03 (Sales staff) and 1 more.
  = Correct the data of the version; for it to go to someone else, say where it goes in the table of `when late escalate by <table>`.
ok tests/mutants/W507_nobody_above_the_head_of_sales.ringi: a request, 1 field, 1 stage
  used from 2026-04-01: the first version of the organization `shared/no_president.ringi` (data `org-no-president`)
  used with no end: no data it reads ends
  ends within 7 business days at the most
```

期限と催促は koyomi の `.cal` に下ろし、「催促は期限より前」は koyomi の主張として確かめる（6.4。催促と期限を別のカレンダーで数えるステップだけは、下のとおり ringi が確かめる）。`when late escalate` があれば、上へ回したあとの期限と催促も、`due_escalated` と `remind_escalated` として下ろす（二度回すなら `due_escalated_twice` と `remind_escalated_twice` も）。どちらも、前の期限から数える。期限が日の終わりなら、上へ回るのは次の日の 00:00 なので、次の日から同じ営業日数を数える。koyomi が、これも数えられることを確かめる。経費の `terms.cal`（`tests/golden/lowered/expense/terms.cal`）の日付は、次の四つである。

```
date remind = started
  + 2 business days
  at 09:00

date due = started
  + 3 business days
  at end of day

date due_escalated = due
  + 1 day
  + 3 business days
  at end of day

date remind_escalated = due
  + 1 day
  + 2 business days
  at 09:00
```

期限か催促を自分で書いたステップと、上へ回す回数が `every stage` と違うステップは、自分の名前の日付を持つ（`board_due`、`own_remind_escalated`）。期限を自分で書いたステップの上へ回したあとの催促は、そのステップの期限から数える。催促が期限より前に来ないステップには、上へ回したあとの催促を下ろさない。回したあとも期限より前に来ないからである。

**催促と期限を別のカレンダーで数えるステップ**（`due in 2 business days in office at end of day` と `remind after 1 business day in plant at 09:00`。事務所の営業日で決裁し、工場の営業日に知らせる、など）では、二つの日付が別の下ろしたファイル（`terms.cal` と `terms_plant.cal`）に入る。下ろしたファイル一つはカレンダー一つで数えるので、koyomi は二つの日付を比べられず、上へ回したあとの催促を期限から数えることもできない。そこで、次の三つは ringi が、カレンダーの休みの日から自分で数える（`src/apart.rs`。保留の営業日を数えるのと同じ数え方である）。

- 主張 `every stage reminds before it is due` は、ステップが始まりうる日の一日ずつについて、催促と期限を数えて比べる。数える日は、二つのカレンダーのデータが重なる日で、休みの表が無ければ、グレゴリオ暦の一巡りの 146,097 日である。二つのカレンダーのオフセットが違えば、日は催促のカレンダーの日で、その一日のうちに始まるステップは、時刻によって、期限のカレンダーでは前の日か次の日に始まる。そのどちらも数え、催促と期限を時刻で比べる（7.5）。催促が期限と同じか後になる日があれば E403 にして、数え方、最初にそうなる日、そうなる日の数を言う。
- 上へ回したあとの催促は、機械が、期限の時計が鳴った時刻を催促のカレンダーのオフセットで読んだ日から、催促のカレンダーで同じ営業日数を数えて置く。回したあとの期限は、期限のカレンダーで、ほかのステップと同じに下ろした `due_escalated` から読む。催促が期限より前に来なければ置かないのも、ほかのステップと同じである。
- 上へ回したあとの催促を、催促のカレンダーで数えられる最後の日は、申請の種類を使える最後の日の一つになる（9.6）。

```text
$ ritsu ringi check tests/mutants/E403_a_reminder_by_another_calendar.ringi
error[E403]: tests/mutants/E403_a_reminder_by_another_calendar.ringi:23:1: A reminder does not come before its deadline
    23 |   reminded_in_time : every stage reminds before it is due
  = Every stage is due after 2 business days in `office` at the end of the day, and reminded after 2 business days in `plant` at 09:00.
  = A stage started on 2000-01-01 is due at the end of 2000-01-04, and its reminder would go on 2000-01-05 at 09:00.
  = 41,742 of the 146,097 days a stage can start on are so (counted from 2000-01-01 through 2399-12-31), and a stage started on them is set no reminder.
  = Make the reminder come after fewer business days, or count the reminder and the deadline in one calendar.
```

機械のための申請 `tests/machine/apart.ringi` は、期限を事務所（土日が休み）の 2 営業日、催促を工場（日月が休み）の 1 営業日で数え、二度上へ回す。金曜に出した申請の催促は、工場が開く土曜の朝に届く。火曜の終わりに期限が過ぎて水曜に上へ回ると、回った先の人への催促は工場の次の営業日の木曜、二度目に回った土曜のあとの催促は、日月の休みをはさんだ火曜になる。

```text
$ ritsu ringi run tests/machine/apart.ringi
2026-11-13 10:00  submit  by e1201  -> decide (e1101), due 2026-11-17 end of day
2026-11-14 09:00  remind            -> decide (e1101), due 2026-11-17 end of day; reminded e1101
2026-11-18 00:00  late              -> decide (e3101), due 2026-11-20 end of day; decide went up to e3101
2026-11-19 09:00  remind            -> decide (e3101), due 2026-11-20 end of day; reminded e3101
2026-11-21 00:00  late              -> decide (e0101), due 2026-11-24 end of day; decide went up to e0101
2026-11-24 09:00  remind            -> decide (e0101), due 2026-11-24 end of day; reminded e0101
2026-11-25 00:00  late              -> expired; expired (late)
scenario `late_and_up_twice`: as written (7 rows)
```

カレンダーの休みの表には終わりがある。ステップが始まった日から期限と催促と上へ回したあとの期限までを数えられる最後の日が、申請の種類を使える最後の日の一つになる（9.6）。上へ回したあとの催促は、期限と同じカレンダーで数えるなら、その期限より前の日なので、この日を動かさない（別のカレンダーで数える催促は、上のとおり、そのカレンダーの側の最後の日を決める）。その日は、下ろした `terms.cal` の `started` をカレンダーのデータの全部の日に広げて koyomi に尋ね、koyomi が数えられないと言う日（E203）の手前の日として求める。見当の余白は置かない。

### 5.11 共有する経路

いくつもの申請で使う経路は、`route` のファイルに書く。受け取る入力、経路、承認者の決め方、期限を持つ。

```
route manager_approval v1 "Approval by the manager"
inputs
  long : bool
route
  manager  manager
  head     dept_head  when long
who
  manager   = manager of requester
  dept_head = head of requester.department
every stage
  due in 2 business days at end of day
  …
```

経路のファイルは `labels <言語>` を持てる。キーは `stage <名前>` と `who <名前>` で、ほかの言語のステップと決め方の題を書く（例の `manager_approval.ringi` の `labels ja`）。使う申請の種類が経路と違う言語で書かれていれば、その言語の題は経路の `labels` に要る（4.8、E703）。

申請の種類は `use route standard from "shared/manager_approval.ringi"` で読み、`route standard(long: days > 5)` で使う。引数の型と数は ringi が確かめる（E307）。承認者の決め方の `requester` は、使う申請の申請者を指す。期限は経路のものを使い、申請の側で `every stage` を書けばそれが勝つ。

共有の経路は、申請が渡す物で下ろす。経路の条件が入力を読む所に、申請が渡した式を置く（`when long` は `when days > 5` になる）。渡さない入力は、計算する入力ならその計算、`bool` なら偽、ほかは「無い」として読む。無い入力は `cap is none` だけが成り立ち、値と比べる条件（`cap > 50_000JPY`）は読めないので、申請の `route` の行で E309 にする。表で書いた共有の経路は、行ごとに、セルの試しを入力の比べとして読み、そこへ申請が渡した式を置いて、その行が成り立つ所を申請の表の行にする（`long` の列が `true` の行は `days > 5` の行に、`false` の行は `days <= 5` の行になる）。共有の経路の行から下ろした行と出力は、申請の `route` の行に結ぶ。rulec が何か言えば、申請の `route` の行で言う。

### 5.12 同じ承認者が二つ以上のステップに当たるとき

上長と部長を兼ねる人、課長と決裁者を兼ねる人のように、同じ一人が、一件の申請の二つ以上のステップの承認者になることがある。その人に何度答えてもらうかは、`ticket` の `same approver` の行で決める。書き方は二つあり、書かなければ、どのステップにも答えてもらう。

| 書き方 | すること |
|---|---|
| `same approver approves once` | すぐ前のステップを承認した人のステップを、その承認で済ませる。あいだにほかの人のステップがある二つのステップには、どちらにも答えてもらう |
| `same approver approves once, at the last stage` | その人が一人で答えるステップのうち、最後のステップで一度だけ答えてもらう。前のステップは、その承認を待つ |

経費の例（`examples/expense.ringi`）は前の書き方、購買の例（`examples/purchase.ringi`）は後の書き方である。

```
ticket
  self approval  not allowed
  # A stage right after one the same person approved is done with that approval. No stage waits: the manager answers before accounting.
  same approver  approves once
```

```
ticket
  self approval     not allowed
  # One who answers two or more stages (a manager who also decides) answers once, at the last of
  # them: the decision is made after finance and the board have answered.
  same approver     approves once, at the last stage
```

二つは、どのステップで答えてもらうかが違う。前の書き方は、先に得た承認を次のステップに使うので、確認の順は変わらない。あいだにほかの人のステップがあれば済ませない。済ませると、決裁する人が、あいだの確かめの結果を見ないまま決裁したことになるからである。後の書き方は、答えるのを後ろへ送る。決裁する人は、あいだの確かめの結果を見てから一度だけ答える。代わりに、前のステップの確認（上長の目）が、あいだの人より後になる。それでよいかは申請の種類によるので、どちらを書くかは申請の種類ごとに決める。一つの `ticket` に書けるのは、どちらか一つである（二つ書けば E006）。自己承認（5.5）と同じくチケットの進み方の決まりなので、申請の種類の `ticket` に書く。共有の経路を使う申請の種類（休暇）も、自分の `ticket` に書けば、経路のステップに効く。

どちらの書き方も、同じ人と数えるのは、**ステップの承認者が一人に決まる**ときだけである。一人に決まるのは、上長、チームの長、最も近い役職の人、人の指定、マスタの表引き、規則の出力の値の決め方と、外す人（申請者と、代わりに出した人）を除くと一人だけのチームである。エージェントのステップ（`agent …, else …`）と、指名のステップ（`chosen by …`）は数えない。エージェントはいつも答え、指名された人は、そのステップの仕事をする人として選ばれたからである。二人以上が答えられるチームと、二票以上の議決も、そのステップで答えてもらう（ほかの人の目を減らさない）。並列のステップ（`a and b`）は、決め方ごとの部分を一つずつ見る。

代理の承認は、代わられた人の承認として数える（5.8）。代わりに答えた人自身が承認者のステップには数えない。一度の操作で、二人の権限を使わないためである。

**前の承認で済ませる（`same approver approves once`）。** ステップが始まるときに、次の三つがそろえば、そのステップをその承認で済ませる。

1. ステップの承認者が、上のとおり一人に決まる。
2. その人が、すぐ前のステップを承認した。すぐ前のステップがこの決まりで済んだものなら、元の承認をその人の承認に数える。申請を出したことは承認に数えない（自己承認を許す経路でも、最初のステップは答えてもらう）。
3. あいだにあるのは、来ないステップ（条件が成り立たない、表のセルが空）、回覧、承認者がいないので飛ばしたステップだけである。ほかの人かエージェントが答えたステップや、要約（`note from`）のステップがあれば済ませない。その人が前に承認したときに無かったもの（ほかの人の答え、エージェントの疑い、要約）が、カードに載るからである。

並列のステップは、一人に決まる部分だけを済ませ、ほかの部分は待つ。すぐ前のステップが並列なら、どの部分もその人が承認したときだけ、次を済ませる。端の場合は次のとおりにする。

- **上へ回したあと**：ステップが始まるときに `when empty escalate` で回した先も、承認者として見る。部長が自分の休暇を出すと、部長のステップは本部長へ回り、上長のステップで承認した本部長の承認で済む。期限を過ぎて回した先（`when late escalate`）では済ませない。ステップはもう始まっていて、決めるのは始まるときだけだからである。すぐ前のステップを、期限を過ぎて回した先の人が承認したときは、その人の承認として次を済ませる。
- **差し戻し**：申請者へ戻して出し直した `round` では、前の `round` の承認を使わない。前のステップ k へ戻したときは、k は済ませずに答えてもらう（戻した人が k の答えを求めている）。k のあとは、また済みうる。
- **取り下げ**：済んだステップは、答えたステップとして扱う。`withdraw until <ステップ>` は、そのステップが済んだときに閉じる。
- **承認の取り消し**：済んだステップは、元の承認と一つである。元の承認を取り消すと、済んだステップも一緒に戻る。取り消せるのは、済んだステップの次の、ほかの人が答えるステップが答えるまでである。
- **保留、期限、催促**：済んだステップには時計を置かず、知らせも出さない。次のステップの時計は、済ませた時刻（元の承認の時刻）から始まる。いちばん長くかかる日数（7.6）は短くなるだけなので、数え方は変わらない。

**最後のステップで一度だけ答える（`same approver approves once, at the last stage`）。** ステップが始まるときに、部分ごとに次のように決める。

1. その部分の承認者が、上のとおり一人に決まる。
2. 機械は、このチケットの経路の、あとのステップを、そのときのフォームの値（表の行と条件）と、そのときに効いている組織の版で解く。同じ人が一人で答える部分を持つステップがあれば、その最後のステップを選ぶ。
3. あれば、その部分は、選んだステップの承認を待つ。無ければ、そこで答えてもらう。

すぐ前後の二つのステップも、同じに読む。この書き方の申請の種類に、前の承認で後を済ませる決まりは無い。決まりは「その人が一人で答える最後のステップで、一度だけ答える」の一つである。三つ以上のステップに同じ人が当たれば、前のステップはどれも最後のステップを待つ。

待っている部分には時計を置かず、知らせも出さない。どの部分も待つステップは、始まらずに次へ進む。ほかの部分のあるステップ（`a and b`）は、ほかの部分に答えてもらうために始まり、その答えがそろえば次へ進む。待たせた人は、そのステップでは答えず、保留もしない（`person` で断る）。あいだに、その人がほかの人と一緒に答えるステップ（二人以上のチーム、議決）があれば、そこではそのステップの一人として答える。

選んだステップの答えがそろったとき、待っていた部分は、**待たせた人の席の承認**がそこにあれば、その承認で済む。席の承認は、本人の承認か、その人の代理の承認である。代わりに答えた人自身のステップが待っていても、その答えでは済まない。その人は、自分の席でも答える。待たせたステップが、だれの答えも無いまま済むことは無い。待たせた人の席の承認が得られなければ、次のとおり、待たせたステップを始めて答えてもらう。

相手が変わる場合は、二つの時に確かめる。

- **選んだステップへチケットが来たとき**（始まる前）。そのステップが、待たせた人に一人では当たらなくなっていれば、待っていた部分を、そのときの版で決め直す。当たらなくなる元は二つある。一つは、あいだに組織の版が替わったことで、その人がいなくなる、長が替わる、席が空いてステップが `when empty` のとおりに上へ回るか飛ぶ、のどれかになる。もう一つは、承認者の直し（`approvers edit`）で経路の行が変わったことで、そのステップが来なくなるか、別の人に当たる。どちらでも、決め直した承認者が、さらにあとのステップに一人で答えるなら、その最後のステップを待ち続ける。そうでなければ、待たせたステップを、選んだステップより前に始める。来なくなったステップを待っていた部分は、次に来るステップの前で、同じに決め直す。
- **選んだステップの答えがそろったとき**。待たせた人の席の承認がそこに無ければ、待たせたステップを、次へ進む前に始める。無いのは、期限を過ぎて回した先の人が承認した（`when late escalate`）、管理する人が引き継がせた人が承認した（`hand over`）、管理する人が足した承認者が承認した（`add approver`）、の三つである。回した先の人の答えで、待たせたステップまで済ませない。ステップが期限を過ぎて回るのは、そのステップだけのことだからである。

始めるときは、チケットのステップの並びに、同じ名前のステップをもう一度足す（7.1）。承認者はそのときの版で決め直し、期限と催促を、そのときから数えて置く。だれもいなければ、`when empty` のとおりにする（5.4）。

先のステップでの答えごとのふるまいは、次のとおりである。

- **却下**：チケットは `rejected` で終わる。待っていたステップは、答えないまま終わる。
- **申請者への差し戻し**：出し直した `round` で、最初から決め直す。
- **前のステップへの差し戻し**：待っているステップにも、その承認で済んだステップにも戻せる。戻されたステップは待たずに、そこで答えてもらう（戻した人が、そのステップの答えを求めている）。あいだのステップへ戻したときは、先の承認が消えるので、その承認で済んだステップは、また待つ。
- **保留**：保留できるのは、今のステップの承認者である。待っているステップは始まっていないので、保留するものが無い。
- **承認の取り消し**：先のステップの承認を取り消すと、その承認で済んだステップは、また待つ。答えは一つなので、取り消しも一度で両方に効く。先のステップが並列で、ほかの部分の承認が取り消されたときも、ステップの答えがそろわなくなるので、また待つ。そろい直したときに、もう一度済む。
- **取り下げ**：待っているステップは、まだ答えていない。`withdraw until <ステップ>` がそのステップを指すなら、待っていた承認が得られるまで取り下げられる。あとで始めたステップ（二度目）がチケットの今のステップなら、そのステップが答えるまでである。
- **期限**：先のステップが期限を過ぎて上へ回っても、待っているステップは待ち続ける。回した先の人が承認すれば、上の「答えがそろったとき」のとおり、待たせたステップをそのあとに始める。
- **履歴**：待たせたとき（`deferred`）、待たせるのをやめて始めたとき（`deferral ended`）、先のステップの承認で済んだとき（`approved`）に、待たせたステップの行を一つずつ残し、どのステップを待つか、どのステップの承認で済んだかを書く（7.4）。`when empty skip` で飛ばした行とは別の言い方にするので、あとから履歴を読んでも、だれも見ていないステップには見えない。

いちばん長くかかる日数（7.6）は、決め方が一つのステップでは変わらない。待つあいだは時計が無く、始まるのは一度だけだからである。決め方が二つ以上のステップは、一つの部分にそこで答えてもらい、待たせた部分をあとで始めることがあるので、この書き方の申請の種類では、決め方の数だけ数える。

**検査。** 組織のデータがあれば、版ごと、申請者ごとに、どの二つのステップが一度で済むかを決め、W506 の言うことを変える（8.6）。新しいコードは無い。

- `same approver approves once` を書いた申請の種類では、あとのステップが前の承認で済む二つのステップを言わない。済まないものは、なぜ済まないかを注に添える（あいだにほかの人が答えるステップがある、前のステップがほかの部分の承認も待つ、あとのステップはエージェントが答える）。`, at the last stage` を足せば一度になるものには、その書き方も添える。
- `same approver approves once, at the last stage` を書いた申請の種類では、前のステップが後のステップを待つ二つのステップを言わない。言うのは、どちらかが、エージェントが先に答えるステップ（`agent …, else …`）のときだけで、待たない理由を注に添える。
- 書いていない申請の種類では、書けば一度になる書き方を、注に添える。すぐ前後なら `same approver approves once`、あいだにステップがあれば `same approver approves once, at the last stage` である。
- 条件は読まないので、ステップの条件の形で、来ないこともあるステップがあいだにある二つのステップは、来るものとして読む。
- 上へ回るステップの言い方（`ok` の下の数と W502。5.4）は変えない。上へ回る先がほかのステップの承認者と同じでも、そのステップに承認者がいないことは変わらない。W502 は、2027 年の版で長の空く部のような、データの欠けを知らせるものである。
- 待たせる二つのステップが一つも無い申請の種類に `, at the last stage` を書いても、警告にしない。組織が変われば効く行であり、書いた人が手を打つことは無いからである。

経費の例（`same approver approves once`）は、あいだに経理のステップがある購入の行で、次のように言う。

```text
warning[W506]: examples/expense.ringi:37:93: In the organization from 2027-04-01, one person answers two stages of a ticket
    37 | | purchase | <=1_000_000JPY             |             | manager | accounting               | dept_head     |
  = The stage `first` is `manager` and the stage `decide` is `dept_head`, and both come to one person.
  = For 3 requesters, e3101 (Head of Sales Department 1) answers both: e1101 (Manager of Section 1A), e1201 (Sales staff 1) and e1203 (Sales staff 3).
  = For 2 requesters, e7001 (Head of Information Systems) answers both: e7101 (Manager of IT Security) and e7201 (Manager of IT Operations).
  = `same approver approves once` does not do `decide` with the approval of `first`: `second` comes between them.
  = With `same approver approves once, at the last stage`, they answer once: `first` waits for the approval of `decide`, after the stages between them.
  = One person who checks twice is not two pairs of eyes: change the way to choose one of the stages, or make sure it is meant.
```

書いていない申請の種類では、直し方の前に一行を足す（`tests/mutants/W506_one_person_checks_twice.ringi` から）。

```text
  = If one approval is enough, write `same approver approves once` under `ticket`: `decide` is then done with the approval of `check`.
```

日本語では「`same approver approves once` でも、`decide` は `check` の承認では済みません。あいだに `sales` があります。」「`same approver approves once, at the last stage` と書けば、一度の承認で済みます。`check` は、あいだのステップのあとの `decide` の承認を待ちます。」と言う（`tests/mutants/W506_the_same_approver_with_a_stage_between.ringi` から）。

**`simulate`。** `simulate`（10.3）は、済むステップと待つステップに、どのステップの承認で答えが一度になるかを言い、期限を書かない。`--form` を渡すと経路が一つに決まるので、前の書き方では ``done with the approval of `first` (`same approver approves once`)``、後の書き方では ``answers once, at `decide` (`same approver approves once, at the last stage`)`` と言う。`--form` が無いときは、表の行ごと、ステップの並びごとに決める。前の書き方で、条件のあるステップがあいだにあって、それが来るかどうかで変わるときは ``done with the approval of `first` where no other stage comes between them`` と言う。後の書き方で、待つ先が条件のあるステップか、決め方が値で変わるステップのときは ``answers once, at `decide`, where it comes, and to them`` と言う（購買の `first`）。JSON の名前は 16.2 に書く。

**例。** `examples/` の五つの例は、次のように書き分ける。

| 例 | 書き方 | 理由 |
|---|---|---|
| 経費 | `same approver approves once` | 上長の確認は、経理の確認より前に要る。すぐ次のステップが同じ人のときだけ、一度で済ませる |
| 休暇 | `same approver approves once` | 上長と部長のステップが続いていて、あいだにステップが無い |
| 購買 | `same approver approves once, at the last stage` | 決裁は、財務と取締役会が答えたあとにするものである。上長と決裁者を兼ねる人は、決裁で一度だけ答える |
| アカウント | `same approver approves once, at the last stage` | 上長がシステムの持ち主や情報セキュリティの担当でもあるとき、その人の最後のステップで一度だけ答える |
| 契約 | 書かない | 法務の受付から審査まで、どのステップもそのステップの仕事として答えてもらう |

`examples/` の英語の例の W506 は、どの申請の種類にも書かなければ 23、経費・休暇・アカウント・購買に `same approver approves once` を書くと 11、購買とアカウントを `, at the last stage` にすると 5 になる（8.6）。残る 5 は、経費の 4（あいだに経理のステップがある購入の行）と、契約の 1 である。

### 5.13 だれが申請を出せるか

`ticket` に `submitted by` と書くと、その申請の種類を出せる人を絞る。除く人は `except` のあとに書く。書かなければ、版のどの人も出せる。自己承認（5.5）と同じくチケットの決まりなので、申請の種類の `ticket` に書く。共有の経路のファイルには `ticket` が無いので書けず、経路を使う申請の種類が、自分の `ticket` で決める。

```
ticket
  # The leave rules are for the staff (fictional): the members of the board do not submit them.
  submitted by   everyone except team board
  self approval  not allowed
```

| 書き方 | 当たる人 |
|---|---|
| `everyone` | 版のどの人も |
| `everyone in sales_div` | 主の所属が、そのチームか、その下のチームの人（本部なら、その下の部と課の人も） |
| `team board` | そのチームの人（主の所属でも兼務でも。`who` の `team` と同じ） |
| `position manager`、`position manager or higher` | 主の所属での役職がその役職の人、その役職以上の人。役職は組織のファイルの `positions` の名前で、`nearest … above requester` と同じ並びで比べる。`in <チーム>` を足すと、主の所属がそのチームか、その下のチームの人に絞る（`position manager or higher in sales_div`） |
| `person "e9001"` | その人 |

いくつかは `,` で並べ、どれかに当たれば出せる。`except` のあとに並べた物のどれかに当たれば、出せない。`team` と `person` は承認者の決め方と同じ語で、同じ人を指す。部で絞るときは下の課の人も入れたいので、`team`（そのチームの人。`who` とそろえる）とは別に、木をたどる `everyone in` を置いた。たどるのは主の所属で、`head of requester.department` と同じである。

- **だれを見るか**：申請者である。代理申請（5.8）では、代わられた人を見る。代わりに出す人は見ない（代わりに出せるかは、`proxies.csv` の `submit` の行が決める）。出せない人の代わりに出すこともできない。
- **いつの組織で見るか**：`submit` のときに効いている版で見る。断るのは新しく出すことだけである。出したあとに組織が変わって外れた人のチケットは、そのまま進む。差し戻しのあとの再申請（`resubmit`）も断らない。同じチケットの続きで、新しい申請ではないからである（7.3）。
- **自己承認**：変わらない。出せる人だけが申請者になり、その人を自己承認の判定で外す。

検査は次のとおりにする。

- 役職の名前が組織のファイルの `positions` に無ければ E004。組織のデータが無くても決まる。
- 書いたチームか人が版にいなければ E508。ほかのグループに人がいても、名前を直すかデータを足すまで誤りにする。課がなくなって、その課の人が出せなくなるのを、切り替えの前に見つけるためである。書いた場所で、版をまとめて言う。
- 版のだれも当たらなければ E509（その版では、だれもこの申請を出せない）。E508 の出る版では、同じ原因を二度言わないよう、E509 は言わない。
- 版ごとの一覧（8.6）の申請者を、出せる人だけにする。代理申請の形も、代わられた人が出せるときだけ数える。E502、W502、E503、E504、W506、W507、W510 と、`ok` の下の組織の形の数（5.4）は、出せる人についてだけ言う。
- `ok` の行の下に、版ごとの出せる人の数を書く。一覧に出る申請者が減ることを、黙って済ませないためである。JSON の `facts` にも同じ文が入る（16.2）。
- 組織のデータが無ければ、ほかの一覧と同じに、決められないと W500 で言う。

```text
$ ritsu ringi check examples/leave.ringi      （警告を省いた）
ok examples/leave.ringi: a request, 6 fields, 2 stages
  used from 2026-04-01: the first version of the organization `shared/company.ringi` (data `../org`)
  used with no end: no data it reads ends
  ends within 40 business days at the most (3 × 10 at its stages, 2 × 5 with the requester)
  submitted by 19 of 24 people in the organization from 2026-04-01, and 18 of 23 from 2027-04-01 (`everyone except team board`)
  requesters whose stages (`head`) go elsewhere or are skipped by the shape of the organization: 9 of 19 in the organization from 2026-04-01, and 8 of 18 from 2027-04-01
  requesters whose late stages (`manager`, `head`) have nowhere to go up to by the shape of the organization: 2 of 19 in the organization from 2026-04-01, and 2 of 18 from 2027-04-01
  who they are, and where their stages go: `ritsu ringi simulate examples/leave.ringi --every-requester`
```

日本語では「出せる人: 2026-04-01 からの組織の 24 人のうち 19 人、2027-04-01 からの 23 人のうち 18 人（`everyone except team board`）」と書く。

```text
$ ritsu ringi check tests/mutants/E509_everyone_left_out.ringi
error[E509]: tests/mutants/E509_everyone_left_out.ringi:15:3: In the organization from 2026-04-01, no one may submit the request
    15 |   submitted by   position dept_head or higher in sales except person "e02"
  = `position dept_head or higher in sales except person "e02"` comes to nobody in the organization from 2026-04-01.
  = Widen `submitted by`, or correct the data of the version.
```

`simulate`（10.3）は、`--requester` に出せない人を渡すと、だれが、どの版で、どの行で出せないかを言い、2 で終わる。申請はファイルの名前で言い、パスの書き方によらない。チケットの `submit` の断りも、生成したコードの断りも同じ文なので、ベクタはどのディレクトリから作っても同じで、作った機械のパスを持たない。`--every-requester` は出せる人だけを並べ、出せない人を頭の行に書く。`--form` に、表示の条件（`shown when`）が成り立たずに隠れるフィールドの値を渡したときも、機械の `submit` と同じ文で断り、2 で終わる（`--format json` も同じ）。機械が受け取らない申請の行き先を、`simulate` だけが答えることはない。

```text
$ ritsu ringi simulate examples/expense.ringi --requester e1201 --on 2026-11-16 --form 'category = purchase, trip_to = "Osaka", items = [{what: "Server", price: 1_000JPY}]'
error: `trip_to` is not shown for this request (its `shown when` does not hold), and takes no value
```

```text
$ ritsu ringi simulate examples/leave.ringi --requester e0201 --on 2026-11-16
error: e0201 (Director 1) may not submit leave.ringi in the organization from 2026-04-01 (`submitted by everyone except team board`)
```

`examples/` の例では、休暇にだけ書いた。取締役会の人（社長と取締役。架空の決まりでは役員）は、社員の休暇の申請を出さないものとする。ほかの四つ（経費、購買、アカウント、契約）は、取締役会の人も出す申請なので書かない。書かなければ、社長の休暇は上長と部長のステップをどちらも飛ばし、だれも答えないので W510 になる。取締役 4 人も、部長のステップが社長へ回る申請者と、社長が期限を過ぎる申請者に入る。書いたことで W510 は消え、`ok` の下の数は、出せる人のうちの数になった（2026 年の版で、組織の形でほかへ回る申請者は 24 人のうち 14 人から 19 人のうち 9 人へ、期限を過ぎても上へ回せない申請者は 6 人から 2 人へ）。

チケットの機械は、`submit` のときに、検査と同じ判定（`org::submit`）で決める（7.3、7.5）。

## 6. 中に書いた物の意味と、`use` で読む物のつなぎ方

### 6.1 決めたこと：今の言語のファイルへ下ろす

申請の種類一つを一つのファイルに書き、分岐の表、条件、計算、期限もその中に書く（R1）。中に書いた物は、同じ物を `.rule` や `.cal` に書いたときと同じ意味でなければならず、食い違いが黙って入り込んではならず、あとで別のファイルへ移せなければならない。一方、根の DESIGN の P1 は一つのファイルに二つの言語を混ぜないと決め、P3 は言語のクレートが互いに依存せず口だけを通すと決め、P4 はある言語の意味をもう一度書かないと決めている。

この両方を、次の形で満たす。

- `.ringi` は ringi の構文だけで書く。中に書いた表、条件、計算、期限は、ringi が `.rule` と `.cal`（申請ごとのつなぎは `.kn`）に下ろす（6.4）。
- 下ろしたファイルはディスクに書かず、`ritsu_base::fs::Overlay` に置く。rulec、koyomi、kaname は、それを普通のファイルとして確かめ、コードにする。
- 下ろした先の診断は、行の対応で ringi の行へ戻し、コードごとに ringi の言葉に直す（6.5）。
- 別のファイルへ移すのは、下ろした物をファイルとして置くことになる（`ritsu ringi extract`）。

根の DESIGN の P1 には、この読み方を一文で足す：「ringi の `.ringi` の中に書いた表、条件、計算、期限は、ringi がそれぞれの言語のファイルに下ろし、そのファイルをその言語が読む。一つのファイルに二つの言語の構文は書かない。」

### 6.2 理由

理由は三つある。

1. 意味が一つしかない。表の完全性と重なり、範囲、丸め、日付の計算は rulec と koyomi が決め、ringi は同じ物をもう一度書かない。だから「中に書いた物の意味を今の言語と食い違わせない」が、作りの上で成り立つ。
2. 今のコードの上に作れる。`ritsu_base::fs::Overlay`（`crates/ritsu-base/src/fs.rs`）は、渡したファイルを、ディスクのファイルの代わりにそのパスで読ませる `Files` の実装で、モジュールの版を読むとき（根の DESIGN 17.9.1）にすでに使っている。言語の検査はファイルを `Files` を通して読むので、下ろしたファイルを Overlay に置けば、各言語は普通のファイルとして読む。ディスクに書かないので、wasm（ブラウザの playground）でも同じに動く。`ritsu-project` の言語のつなぎと ritsu-cross の判定も、そのまま使える。足すのは、口の三つ（`findings`、`diff`、`Requests`）と、下ろす所と、診断を直す所である。`findings` が要るのは、rulec の `--format json` が診断ごとに例の入力（`witness`）、行（`rows`）、直し方（`fix`）を持つのに、口の `Said`（コード、ファイル、行、文）はそれらを持たず、そのままでは診断を ringi の言葉に直せないからである。
3. 移すのが、下ろした物を置くことと同じになる。`ritsu ringi extract` が書く `.rule` は、検査を通したのと同じファイルなので、あとで共有に移しても意味が変わらない。

### 6.3 捨てた形

ほかに三つの形を考えて、捨てた。

- **形 1：ringi の構文として持つ。** 表、条件、期限を ringi の構文で書き、検査も ringi が持つ。式とセルの綴りと意味は rulec と同じにし、同じ答えになることを、rulec と koyomi を dev-dependency にした試験で確かめる。rulec の表の検査（範囲の区分け、型、丸め。rulec のおよそ 1 万 6 千行）をもう一度書くことになり、P4 に反する。実装が二つになるので、食い違いが入り込む。
- **形 2：今の言語の領域として埋め込む。** `.ringi` の中の表は rulec の表、期限は koyomi の式として、口を通して rulec と koyomi に確かめさせ、コードも作らせる。一つのファイルに二つの言語を書くので、P1 を変えることになる。rulec と koyomi にはファイルの一部だけを読む形が無く、口と `ritsu-project` の読み方を作り替えることになる。診断の行はずらせば戻るが、言葉は rulec のままになる。
- **下ろしたファイルをディスクに書き、利用者のリポジトリに置く形**（`ritsu gen` の生成物と同じ扱い）。申請の種類一つが二つ以上のファイルになり、古くなったファイルが残りうる。

下ろす形（形 3）を、形 1、形 2 と次の観点で比べた。

| 観点 | 形 1（ringi の構文） | 形 2（埋め込む） | 形 3（下ろす） |
|---|---|---|---|
| P1 | 字面は守る | 変える（一つのファイルに二つの言語） | 字面は守る。下ろした物は別のファイルとして今の言語が読む、という読み方を一文足す |
| P3、P4 | rulec の表の検査（範囲の区分け、型、丸め。rulec のおよそ 1 万 6 千行）をもう一度書く。P4 に反する | 守る。ただし口にファイルの中の領域を尋ねる仕組みが要る | 守る。ringi は下ろすだけで、意味は今の言語が決める |
| 今のコードで成り立つか | 成り立つが、書く量が大きい | rulec と koyomi に、ファイルの一部だけを読む形が無い。口と `ritsu-project` の読み方を作り替える | 成り立つ。Overlay が既にある |
| 診断を ringi の行と言葉で | そのまま出せる | 行はずらせば戻るが、言葉は rulec のもの | 行の対応で戻し、コードごとに ringi の言葉に直す。直すには例の入力と行が要るので、口に `findings` を足す |
| ページと diff | ringi が作り直す | rulec の `doc` と `diff` を領域に使えるようにする | 下ろした規則に rulec の `doc` と `diff` をかけ、ringi が自分の言葉のページに組む |
| 参照インタプリタ | ringi が評価する | 口の `eval` | 口の `eval`（下ろした規則と日付） |
| 生成するコード | 出力先ごとに ringi が書く | rulec と koyomi が書く | rulec、koyomi、kaname が書く（TypeScript、Python、Go、Rust） |
| 別のファイルへ移す | rulec に書き直す。食い違いうる | 領域を切り出す | 下ろしたファイルを置く（`ritsu ringi extract`）。同じ物なので食い違わない |
| 作る量 | 大きい | 中くらい。ほかの言語にも手が入る | 中くらい。下ろす所と、診断を直す所 |
| 食い違いが入り込む危うさ | 大きい（二つの実装） | 小さい | 小さい。危ういのは下ろし方と言葉の直しで、試験（18 章）で押さえる |

### 6.4 下ろし方

ringi は、申請の種類一つと共有の経路一つを、次のファイルに下ろす。置き場は、`.ringi` と同じディレクトリの `<ファイルの名前>.lowered/` で、ディスクには書かない。ringi の検査が、ringi だけで決まる検査（9.1）と E103 が通ったあとに下ろし、`ritsu_base::fs::Overlay` に置いて、口の `Rules::findings` と `Dates::findings` で rulec と koyomi に尋ねる（6.5、9.4）。`ritsu ringi check` と `ritsu check` は同じ道を通る。Overlay は尋ねるあいだだけあるので、下ろしたファイルは、ほかの言語の検査にも、プロジェクトのファイルの一覧にも入らない。同じパスに実のファイルがあれば、下ろさずに E201 にする。

| 中に書いた物 | 下ろした先 | 経費の例を下ろした物 |
|---|---|---|
| 選択の値（フォームの `choice`、`choice from` で読む規則の列挙、経路と条件が読む規則の出力の列挙。`none` になる出力 `decider : decider?` の列挙も） | `choices.rule`（`enums` のファイル） | `tests/golden/lowered/expense/choices.rule` |
| 経路の表（ステップごとに一つの任意の出力。値は承認者の決め方の名前。空のセルは `none`）、ステップの条件、`possible` の主張、`examples` | `route.rule` | `tests/golden/lowered/expense/route.rule` |
| 自動計算、合計、条件つきの表示と必須 | `form.rule` | `tests/golden/lowered/expense/form.rule` |
| 同じ物の、生成するコードが読む写し（計算に書いた範囲を写さない。4.4） | `values.rule` | |
| フォームの、営業日で数える日付の計算 | `form.cal` | `tests/golden/lowered/leave/form.cal` |
| 名前の付いた表 | `<表の名前>.rule` | `tests/golden/lowered/expense/next_up.rule` |
| 期限、催促、「催促は期限より前」 | `terms.cal`（ほかのカレンダーで数えるステップは `terms_<カレンダーの語>.cal`） | `tests/golden/lowered/expense/terms.cal` |
| 必須なのに隠れるかの問い（4.2） | `<フィールド>.hidden.rule`（`extract` は書かない） | |
| 申請ごとのつなぎ（フォームの値を規則と日付に渡し、答えをチケットの機械に渡す）と、それが数える営業日の日付のファイル（保留、申請者の番、期限と別に数える催促の `open.cal`、`open in` の日付を開いている日に寄せる `opened_<カレンダーの語>.cal`） | `<申請の ASCII の名前>.kn` と、その日付のファイル（`ritsu gen` だけが置く。12.1、12.3） | |

例の申請の下ろした物は `tests/golden/lowered/<申請の名前>/` にある（日本語の名前の版は `<申請の名前>.ja/`）。試験（`tests/lowered.rs`）が、下ろすたびにそれと同じになることと、経費の各ファイルが今の rulec と koyomi の検査を通ることを確かめる。経費の物は、公開版の 0.55.2 でも通る。購買の `form.rule` は、計算に範囲を書く `define … range`（4.4）を使うので、0.55.2 の rulec は読めない（E059）。

```text
$ ritsu rulec check tests/golden/lowered/expense/*.rule
ok tests/golden/lowered/expense/choices.rule
ok tests/golden/lowered/expense/form.rule
ok tests/golden/lowered/expense/next_up.rule
ok tests/golden/lowered/expense/route.rule
ok tests/golden/lowered/expense/values.rule
$ ritsu koyomi check tests/golden/lowered/expense/terms.cal
tests/golden/lowered/expense/terms.cal: ok — 1 claim holds on all 146,097 days of started (2026-04-01..2426-03-31)
```

下ろすときの決まり：

- 名前は、生成するコードの名前（括弧の中の ASCII の名前）を使う。下ろした先の言語が別の物として読む名前と、同じファイルの中でぶつかる名前は、別の名前で書く。PascalCase にすると同じになる二つの名前も、ぶつかる名前に数える（`a_b` と `aB` は、Go に生成するコードでどちらも `AB`）。付けた名前は、申請が宣言するどの ASCII の名前（フィールドと明細の欄、選択の値、ステップ、承認者の決め方、名前の付いた表とその列、`use` の別名）とも重ならない。
  - rulec と koyomi の予約語には `_value` を付ける。ステップ `final` の出力は `final_value`、表 `up` は `up_value.rule` の `table up_value`、明細 `count` とその欄 `max` は `elements count_value` と `max_value` になる。名前の付いた表が下ろしたほかのファイル（`route.rule`、`form.rule`、`values.rule`、`choices.rule`）と同じ名前なら、それも付け替える。
  - 選択の列挙は、rulec が型として読む語（`date`、`string`、`money` など）なら `_value` を付ける（`date_value`）。rulec が規則に生成するコードが、自分の型に使う名前と、名前だけで読む言語の標準の型の名前（PascalCase にして `Input`、`Output`、`Fired`、`Element` と `…Element`、`RuleError`、Rust の `Option` と `Result`、TypeScript の `Object`、Python の `NamedTuple` など）も、列挙の名前にしない（選択 `option` の列挙は `option_value`）。同じ名前の列挙は、そのコードで別の型を指し、コードが組めなくなるからである。ステップの出力の列挙（ステップ `first` の `first_by`）は、選択の列挙と、経路が読む規則の出力の列挙を避けて名付ける。選択の値は、rulec がセルの値の位置で別の物として読む語（`after`、`by`、`empty`、`exhausted`、`starts_with` など）なら `_value` を付ける（`after_value`）。列挙の値の文言は、書いたとおりの名前のままにする。
  - 承認者の決め方の名前を値として書くとき、その語がセルの値の位置で別の物に読めるか、同じファイルの宣言と同じ名前か、PascalCase にしてほかの値と同じなら、`_approver` を付ける（`after_approver`。ステップと同じ名前の決め方 `manager` は `manager_approver`）。それも申請の宣言する名前（`after_approver` という決め方など）かほかの値と同じなら、番号を続ける（`after_approver2`）。チケットのステップの値（`stages` の `value`）は、付け替える前の名前で持つ。いくつかの決め方やその割合を書いたステップの値（`a and b` の `a_and_b`、`all of legal` の `all_of_legal`）と同じ名前の決め方は、チケットの JSON の値で二つを分けられないので、E002 にする。
  - つなぎ（`<申請の ASCII の名前>.kn`）では、関数の引数と `let` の名前が、つなぎの関数がほかの物に使う名前（`g`、`org`、`form`、`approver`、`y`）か `use` の行の名前と同じなら、`_value` を付ける（フィールド `form` の値は、引数 `form_value` で受け取る）。フィールドごとに宣言する関数とレコード（`given_<フィールド>`、`val_<フィールド>`、`cell_<明細>_<欄>`、`row_vals_<明細>`、`row_<明細>`、`<明細>Row` など）は、ほかに宣言する名前と、そのままか PascalCase で同じなら `_value` を付ける（明細 `x` の `row_vals_x` と、明細 `vals_x` の `row_vals_x`。kaname は、関数の名前を PascalCase にして生成するコードの型を名付けるので、`given_a_b` と `given_aB` もぶつかる）。申請の種類の名前が kaname の予約語なら、モジュールを別の名前で宣言して括弧に ASCII の名前を書く（`module match_value(match)`）。生成するコードはファイルと型に括弧の中の名前を使い、Rust のクレートの入口は、モジュールを宣言した名前で呼ぶ。
  - rulec が Python に生成するコードは、規則の出力と明細の要素を `NamedTuple` に、列挙を `Enum` に持つ。`NamedTuple` は `_` で始まるフィールドを受け取らず、`Enum` は `_x_` や `__x` の形の名前を値にしない。そこで、rulec に下ろすファイルでは、`_` で始まる名前（フィールド、明細とその欄、選択の値、ステップ、承認者の決め方の値、`use` の別名から作る入力）の前に `n` を付ける（計算するフィールド `_t` は `n_t`、選択の値 `_a` は `n_a`、決め方 `_mgr` の値は `n_mgr`）。ほかとぶつかれば、ほかの名前と同じく `_value` か番号を付ける。
  - カレンダーごとに下ろす日付のファイル（期限の `terms_<語>.cal`、営業日の `open_<語>.cal`、開いている日の `opened_<語>.cal`）と、つなぎがそれを読む名前は、カレンダーの語で名付ける。語は、ふつうはカレンダーの別名のままである。`_` で始まる別名は前に `n` を付け、続く `_` は一つにする（`_office` は `n_office`、`a__b` は `a_b`）。ファイルの名前は `<申請>_opened_<語>` のように `_` でつなぐので、そのままでは、`_` の続くモジュールの名前を rustc が警告する（koyomi の E016）。生成するコードは、日付のファイルを、TypeScript、Python、Rust では小文字にした名前のモジュールに、Go では、さらに `_` を除いた名前のパッケージに置く。そこで、ある語のファイルの名前のどれかが、小文字にして `_` を除くと、先に名付けたカレンダーのファイルの名前か、カレンダーの無いファイルの名前（`form`、`terms`、`open`）と同じなら、その語に `_2`、`_3` と番号を付ける。`a_b` のあとの `aB` と、`ab` のあとの `aB` は、どちらも `aB_2` になる。`x` のあとの `ed_x` は、`opened_x` と `open_ed_x` が Go で同じ `openedx` になるので、`ed_x_2` になる。ぶつからない申請は、前と同じ名前のままである（`tests/lowered.rs` の `a_calendars_files_are_named_apart_in_every_language`）。
  - つなぎの `use` の行の名前と、カレンダーごとに宣言する関数は、一つの組で、この順に名付ける。経路が出力を読む規則（申請が読む別名のまま）、`open in` のカレンダーの開いている日のファイル（`opened_<語>`）、二つ目からのカレンダーの期限のファイル（`terms_1`）と営業日のファイル（`open_1`）、カレンダーの営業日を数える関数（`open_<語>`）である。つなぎ自身が宣言する名前（`open_after`、`values` など）か、先に名付けたものと、そのままか PascalCase で同じなら `_value` を付ける。カレンダー `after` の営業日の関数は `open_after_value`、別名 `open_office` で読む規則の横のカレンダー `office` の関数は `open_office_value`、別名 `terms_1` で読む規則の横の、二つ目のカレンダーの期限のファイルは `terms_1_value` で読む。
  - koyomi は、日付のファイル、入力、日付の名前を、小文字の形（`[a-z][a-z0-9_]*`）でしか受け取らない（koyomi の E010）。`_` が二つ続く名前は受け取るが、koyomi が Rust に生成するコードを rustc が警告するので、`ritsu gen` が断る（koyomi の E016）。大文字を含む名前、`_` で始まる名前、`_` が二つ続く名前は、日付のファイルの入力と日付では小文字にし、`_` で始まるなら前に `n` を付け、続く `_` を一つにする（フィールド `firstDay` は `firstday`、`_x` は `n_x`、`x__y` は `x_y`、ステップ `firstB` の期限は `firstb_due`）。ほかとぶつかれば `_value` を付ける。日付のファイルの名前（`<申請>_form`、`<申請>_terms`、`<申請>_open`、`<申請>_opened_<語>`）は、そのまま書いたあとの括弧に小文字の名前を書き（`capsDays_form(capsdays_form)`）、生成するコードは括弧の中の名前を使う。申請の種類、カレンダー、フィールド、ステップを大文字を含む名前にしても、検査と生成を通る（`tests/lowered.rs` の `names_in_capitals_are_lowered_as_koyomi_takes_them`）。`_` で始まる名前と `_` が二つ続く名前も同じである（`names_with_underscores_are_lowered_as_each_language_takes_them`）。
  - フォームと明細の行のフィールドは、つなぎのレコードのフィールドになる。kaname が生成するコードはレコードの JSON を宣言した名前で読み書きするので、ほかの名前に置き換えられない。そこで、kaname の予約語（`then`、`match`、`some` など。ほかの予約語はコードを生成する言語の予約語でもある）は、フィールドの ASCII の名前にできない（E002）。kaname が生成するコードの補助のモジュールとその型の名前（`kaname`、`KanameError`、`Q`、`Date`、`Rat`、`glue`）は、括弧の中に書いても生成するコードが受け取れないので、どの名前にもできない（E002）。フォームの二つのフィールド（明細の一つの行の二つの欄も）の ASCII の名前が PascalCase にして同じ（`cost_code` と `costCode`）なら、Go に生成するコードのレコードで一つのフィールドになるので、E002 にする。
  - `_` だけの名前は、Go、Rust、Python の `match` が値を持たない名前として読むので、E002 にする。申請の種類の ASCII の名前は、下ろしたファイルと、生成するコードのモジュールとパッケージの名前の頭になる（`<申請>_route`、`<申請>_terms`）。Go は `_` で始まる名前のファイルを読まず、`_` で終わる名前は `x__terms` のように `_` が続き、rustc がそれを警告する。そこで、`_` で始まる名前、`_` で終わる名前、`_` が二つ続く名前は、申請の種類の名前にできない（E002）。ほかの名前は、上の決まりで付け替えるので使える。
  - 予約語の一覧は rulec、koyomi、kaname の一覧のコピーで、試験が本物と突き合わせる。付け替えた名前は、ほかの言語の文の中でも申請の名前に戻して言い（6.5）、`check`、`simulate`、`api` の出力とチケットの JSON には出さない。
- 選択の値は `choices.rule` に一度だけ書き、ほかの下ろしたファイルは `import enum` で読む。フォームの選択、経路の表、名前の付いた表が別々の列挙にならない。`enums` のファイルにするのは、`rule` のファイルの列挙は `import enum` で読めないからである（rulec の E066）。`choice from authority.kind` の値は、下ろすたびに今の規則から写す。
- 数の範囲と日付の範囲は、フォームの宣言のまま規則と日付の入力の範囲になる。計算するフィールドは、届く範囲と書いた範囲の重なりになる（4.1、4.4）。フィールドと同じ次元の別の単位で書いた数は、フィールドの単位に直して書く（`mass[kg]` の `range <=500_000g` は `<=500kg`、条件の `weight > 100_000g` は `>100kg`）。範囲の端、条件と表のセルの値、例のセル、丸めの刻み（`round down(100銭)` は `1JPY`）のどれも同じである。表の範囲がそれを覆わなければ、rulec の E101 が、当てはまらない申請を例にして言う（E301）。
- 条件の偽の側の行（`not:`、`<=` と `>`）と、表の空のセルの `none` は ringi が作る。条件は原子ごとに真と偽の行に分ける。
- 条件を分けた行のうち、どの申請も当てはまらないと rulec が示した行（rulec の E102。計算するフィールドと、その計算が読む値を一緒に読む行に出る）は、条件の、どの申請も来ない部分なので、申請の誤りにしない。ringi は、その行を除いたファイルを置き直して、もう一度尋ねる（`form.rule`、その写しの `values.rule`、E105 の問い）。除いても、ほかの行の答えは変わらず、rulec の網羅の検査は、届かない組み合わせに行を求めない。除いたファイルは中身が変わるので、もとのファイルと別に、一度ずつ検査する。範囲の外の値を求める部分は、もとから行にしない（区切りを列の範囲で切る）ので、どちらの形も、どの申請も来ない部分を黙って落とす。`shown when category is travel or (fee > 900JPY and amount <= 30_000JPY)` は、旅費なら見せ、旅費でなければ見せない条件になる（`tests/lowered.rs` の `a_row_no_request_reaches_is_left_out_of_a_condition`）。経路の表と名前の付いた表は申請が書いた表なので、届かない行は申請の誤り（E303、E201）のままである。
- 表に書けない物を読む条件（4.5）は、経路、フォーム、`only when` のどれでも E309 にして、その申請の種類は何も下ろさない。読めない所はまとめて全部言う。
- 人の有無（`reviewer is not none`）は、真偽の入力 `reviewer_given` にする。文の任意の列は表に書けないからである（rulec の E110）。
- 経路が使う規則の出力（`authority.board`）は、`route.rule` が規則を `apply` で呼んで読む。規則の入力には、同じ名前のフォームのフィールドの列を渡し、出力にはファイルの中の名前を付ける。`meaning:` には、下ろすたびに今の規則の意味の固定を書く（rulec の E040）。下ろしたファイルはディスクに無いので、規則が変われば、次に下ろすときに書き直される。購買の `route.rule`（`tests/golden/lowered/purchase/route.rule`）：

  ```
  apply authority = "../../../../examples/shared/authority.rule" meaning:19261fb92723d78d
    kind = kind
    amount = amount
    finance -> authority_finance
    board -> authority_board
    decider -> authority_decider
  ```

  rulec は、取り込んだ規則の出力を、それを決める入力から計算した値として数える（rulec の DESIGN §15.322）。経路の表が、出力の列と、それを決めるフィールドの列を一緒に読んでも、規則が出さない組み合わせに行を求めない。生成するコードでは、経路の規則のコードが、取り込んだ規則を中に持つ（rulec がそう書く）。つなぎは経路の規則だけを呼ぶ。
- `apply` で呼ぶ規則の出力の列挙は `choices.rule` に書かない。取り込んだ出力は経路の規則の列で、`fed by` の入力ではないので、どの下ろしたファイルも `import enum` で読まない。書くと、生成するコードがその型を二つ宣言し（Swift では `AuthorityDecider` が二つ）、rulec の W121（ringi の W201）になる。フォームの条件（`shown when`、`required when`）が読む出力は `fed by` なので、その列挙は今までどおり書く。経路がその規則を準用しつつフォームの条件がその出力を読む申請では、`route.rule` が `import enum` で読む列挙と `apply` が持ち込む列挙が一つの型名になるが、二つは同じ列挙（値の並びが全部同じ）なので、rulec は `enums` ファイルのモジュールに一度だけ書き、W121（ringi の W201）にはならない（rulec の DESIGN 15.355。根の PLAN 7.161）。
- 次の規則は `apply` で呼ばない。出力を `route.rule` の入力として受け、`fed by` で出どころを書く（`authority_board : bool  fed by rulec "../shared/authority.rule" output board`）。つなぎが規則を呼び、出力を経路に渡す。
  - 経路が出力の一部だけを読む規則。rulec が Rust に書く、規則を取り込む規則のコードは、取り込んだ規則の出力を全部変数に置くので、読まない出力の変数を rustc が警告し（`unused_variables`）、`-D warnings` で通らない。
  - 入力に渡せるフィールドの無い規則。同じ名前のフィールドが無いか、型が違うか、値の無いことがあるフィールドを値の無いことを受けない入力に渡すか、フィールドの範囲が入力の範囲に入らないか、選択の値が規則の列挙に無いときである。rulec は単位を書いた綴りで比べるので（rulec の E042）、`円` で書いた金額は `JPY` の入力に渡せない。購買の日本語の版（`purchase.ja.ringi`）はこれで `fed by` になる。
  - 意味の固定の分からない規則、モジュールの規則（`@…`）、明細を歩く規則（`elements`）、値の無いことがある数を出力に持つ規則（rulec の E044。下ろしたファイルがその値を持てない）。
  - 経路の例（`examples`）か `only when` の主張が出力を読む規則。例の行と主張の問いは、出力を入力の列として読むからである。
- `fed by` で受けた出力は、決めるフィールドとは別の列として数えられる。経路の表が、出力の列と、その出力を決めるフィールドの列を一緒に読むと、規則が出さない組み合わせにも行を求められる。購買の決裁権限表の三つの出力のうち `finance` だけを `kind` と `amount` の横に置いた表は、`kind = goods, amount = 1JPY, authority.finance = true` の行が無いと E301 になる。権限表は、この申請に `finance = false` を返す。ringi は、この穴を消す回り道を作らない。使う規則の出力でステップを分けるときは、ステップの行（`finance finance when authority.finance`）に書けば、偽の側の行を ringi が作るので、この形にならない。同じ表を、出力が `finance` 一つの規則で書けば、`apply` で呼ぶので E301 にならない。
- ステップの出力の列挙の値は、表のセルの値と、主張と例が指す値。`a and b` の値は `who` に宣言した順で名付け、`accounting and dept_head` と `dept_head and accounting` を同じ値にする。
- 期限の `terms.cal` の入力 `started`（ステップが始まる日）の範囲は、2000-01-01 から 2399-12-31（グレゴリオ暦の一巡りの 146,097 日）を、カレンダーのデータの範囲で切ったもの。データの終わりの側は、いちばん長い営業日数の 2 倍と 61 日を残して切る。二年の窓では、その外の年にだけ成り立たない主張が見えない。
- 主張は二つに分ける。`possible` は rulec の `possible` に、`every stage reminds before it is due` は koyomi の主張 `remind < due` に下ろす。`X is Y only when C` は下ろさず、ringi が、C の成り立たない入力の区間で X がとる値を rulec に尋ねる（`Rules::outputs_over`）。Y があれば、それに届く入力を例にして E306。`every ticket ends within` はチケットの機械の主張なので、ringi が数えて確かめる（7.6、E401）。下ろした `terms.cal` が持つ主張と例は、`.ringi` に書いたものだけである。

### 6.5 診断を ringi の行と言葉に戻す

下ろすときに、下ろしたファイルの行ごとに、元の ringi の行と、その行が何か（経路の表の 3 行目、フィールド `amount` の計算、主張 `reminder_before_due`）を持つ（行の対応）。下ろした先の言語の診断は、口の `findings`（9.4）で、コード、行、例の入力、行、直し方を受け取り、9.2 の表のとおり ringi のコードと言葉に直して、ringi の行に出す。表に無いコードは、エラーなら E201、警告なら W201 にして、元の言語とコード、下ろしたファイルの名前と行、元の文を注に付ける（`From rulec: E108 at line 15 of the lowered `values.rule`: …`、`rulec の E108（下ろした `values.rule` の 15 行目）: …`）。下ろしたファイルはディスクに無く、だれも開けないので、パス（`expense.lowered/values.rule`）は出力に出さず、下ろした先の名前で言う。下ろしたファイルは、一回の実行で一度だけ検査する（ritsu の DESIGN 4.15）。申請の種類ごとに一つの Overlay に置き、検査も、そのあとの主張も、`run` と `simulate` の機械も、同じ Overlay を通して尋ねる。口には実行の言語（`--lang`）で尋ね、何か言われたときだけもう一方の言語でも尋ねる。rulec と koyomi は見つけたときの言語で文を書くので、言語ごとに一度の検査になるからである。何も言われないファイルは一度で済み、そのあとの主張と機械が同じファイルを尋ねても、検査し直さない（ritsu は、実行のあいだ rulec を実行の言語で読ませる。ritsu の DESIGN 4.15）。

- 例の入力は、rulec の答えの値を、申請のフィールドの名前と値の書き方で言う（選択は `.ringi` に書いた名前）。付け替えた名前（`final_value`、koyomi の `days_value`）は、ほかの言語の文の中でも申請の名前に戻す。
- E301 は、rulec の E101 の `fix` の行から三つを言う。どの行にも当てはまらない申請の範囲（`fix` の行の入力のセルを、申請のフィールドの名前、フィールドの単位、選択の書いた名前で言い直す。`>100_000JPY` は、範囲の端まで `amount from 100_001JPY to 1_000_000JPY`。一点なら例と同じなので言わない）、その一つの例（`witness`）、足す行である。足す行は、rulec の行を ringi の表の列に並べ直すだけで、入力のセルは rulec のまま使う（表がすでに書いている区切りと範囲の端だけで切ってあり、新しい区切りを決めない）。ステップのセル（経路を選ぶ表なら経路のセル）は `?` のままで、注が、そこに `who` の名前（経路なら `use route` の別名）を書くように言う。`?` は ringi の検査を通らない（E001）ので、貼っただけの行が、だれも決めていない承認者で動くことは無い。穴が二か所以上なら、範囲と行を一か所ずつ並べ、`fix` は行を改行でつなぐ。rulec は 8 か所までを挙げるので、それを超えるときは、挙げた所だけが埋まり、残りは次の検査で出ると言う。足した行で穴が埋まることは、`tests/fixes.rs` が、E301 の変異と、二つの列の表、10 か所の表で確かめる（`?` を表の最初の行の答えで埋めて貼り、10 か所の表は二回の検査で通る）。
- E302 は、rulec の E105 の二つの行に当てはまる申請が一つでなければ、その範囲を、E301 と同じ言い方で、例の前に言う（`The requests both rows take: amount from 100_000JPY to 200_000JPY`）。二つの行が同じ申請を取るときと、片方の行の申請をもう片方がどれも取るときは、行の番号でそう言う。範囲は、申請の表の二つの行のセルから ringi が求める（rulec の文は、下ろした規則の名前と書き方なので使わない）。
- ほかのコードを E201 と W201 にするときの注は、三つまでである。rulec の注の下の一覧（穴の範囲、足す行。二つの空白で始まる行）は、上の注と合わせて一つに数え、途中で切らない。
- 経路の表の誤り（rulec の E101、E105、E102）があるときは、その表から出たほかの診断（成り立たなくなった例と主張など）を出さず、最初の診断に「この誤りから出た診断が、ほかに N 件」と注を付ける（21 章の 7）。`choices.rule` が通らないときの、ほかのファイルの `import enum` の失敗も同じ。
- 例と `possible` の主張だけが指す値に、rulec が「どの行も与えない」（W111）と言うときは、W303 を出さない。その主張の E306 か、例の E601 が言うからである。`only when` だけが指す値は W303 を残す（`only when` の主張は、その値に届かなくても成り立つので、W303 だけが知らせになる）。
- 問いの規則（4.2）への rulec の警告は出さない。申請についての言葉ではないからである。
- 条件を分けた行に rulec が言う E102（どの入力も当てはまらない）は出さず、その行を除いて尋ね直す（6.4）。

例：経路の表に 10 万円を超える行が無い申請（`tests/mutants/E301_no_row_over_100000_yen.ringi`）。

```
form
  amount : money[JPY] "Amount"  range >=1JPY <=1_000_000JPY

route
| amount       | -> check | decide  |
| <=100_000JPY | finance  | manager |
```

```text
$ ritsu ringi check E301_no_row_over_100000_yen.ringi
error[E301]: E301_no_row_over_100000_yen.ringi:6:1: No row of the route takes some requests
     6 | route
  = The requests no row takes: amount from 100_001JPY to 1_000_000JPY
  = A request no row takes: amount = 100_001JPY
  = Add this row to the table, writing in place of each `?` who answers that stage (a name in `who`; a blank cell where those requests skip it): | >100_000JPY | ? | ? |
  = The cells are cut only at the bounds the table already writes and at the ends of the ranges, so the rows close the gap as they are; where the approvers change inside a row, split it there.
```

穴が二か所以上なら、範囲と行を一つずつ並べる（二つの列の表）。

```text
error[E301]: fix.ringi:10:1: No row of the route takes some requests
    10 | route
  = The requests no row takes, in 4 parts:
  =   kind = travel, amount from 200_001JPY to 300_000JPY
  =   kind = travel or goods, amount from 100_001JPY to 200_000JPY
  =   kind = travel or goods, amount from 300_001JPY to 1_000_000JPY
  =   kind = goods, amount from 1JPY to 100_000JPY
  = A request no row takes: kind = travel, amount = 100_001JPY
  = Add these rows to the table, writing in place of each `?` who answers that stage (a name in `who`; a blank cell where those requests skip it):
  =   | travel | >200_000JPY <=300_000JPY | ? | ? |
  =   | travel, goods | >100_000JPY <=200_000JPY | ? | ? |
  =   | travel, goods | >300_000JPY | ? | ? |
  =   | goods | <=100_000JPY | ? | ? |
  = The cells are cut only at the bounds the table already writes and at the ends of the ranges, so the rows close the gap as they are; where the approvers change inside a row, split it there.
```

`use` で読むファイル（共有の経路、規則、カレンダー、帳簿、flow、gate）が、その言語の検査を通らないときは、`use` の行に E202 を出す（そのファイルのエラーを三つまで注に）。`ritsu check` では、この E202 は、読むファイルの根のエラーを待っている行として、根の塊の下に並ぶ（根の DESIGN 8.3）。下ろしたファイルどうしの連鎖は、利用者には見えないファイルなので、上のとおり ringi の診断一つにまとめる。

### 6.6 `use` で読む物のつなぎ方

どの `use` も、読むファイルがその言語の検査を通らなければ、`use` の行に E202 を出す（6.5）。ringi は `References`（`use` の行と読むファイル）に答え、`ritsu check` の索引に入るので、`ritsu check` では、この E202 は読むファイルを待っている行になる。

| `use` | 読む口 | ringi がすること | 確かめる所 |
|---|---|---|---|
| `organization` | （ringi のファイル） | 役職、チームの種類、データの置き場を読む | 8 章 |
| `masters` | （ringi のファイル） | マスタの形とデータ | 4.6 |
| `route` | （ringi のファイル） | 共有の経路を使う。共有の経路も、それだけで下ろして確かめる | 5.11、ringi の E202 |
| `calendar` | `Dates::calendar`、`Dates::facts` | 下ろした `.cal` が `use calendar` で読む。`open in`、営業日の期限、フォームの営業日の日付 | koyomi の検査、ringi の E404 |
| `dates` | `Dates` | 下ろした `.kn` が日付の関数を呼ぶ | ritsu の X6 |
| `rule` | `Rules::facts` | 選択の値（`choice from authority.kind`）、承認者を決める出力、ステップの条件の、名前と型（無い出力と列挙は E004、型の違いは E009、承認者の決め方の無い値は E203）。下ろした `route.rule` が出力を `fed by` で受ける。下ろした `.kn` が規則を呼ぶ | rulec の検査、ritsu の X2 |
| `book` | `Books` | `hold`、`post`、`void` の引数と単位、拒否の理由、仮押さえの有効期限 | ritsu の X19 と X4、ringi の E405、E406 |
| `flow` | `Flows` | `launch`、エージェントのステップ、要約の、入力と出力の型 | ritsu の X20 |
| `gate` | `Gates` | 会社全体の決まりが、チケットの操作とサーバーの管理の操作を宣言し、経路と食い違わないか | ritsu の X18 |

yuen は、`.req` の `satisfied by ringi "../expense.ringi" table route` のように、ringi のファイルの中の物を指す。参照の書き方のツール名に `ringi` があり（`ritsu-base` の `naming.rs` の `Tool`、`tests/fixtures/naming.tsv`）、ringi が `Items` と `References` に答える。中の物の種類と名前は 9.7 にある。`examples/shared/expense.req` は、ringi の物を指す 13 行がどれも読め、残るのは、人がまだ確かめていないと言う yuen の E301（21 件）だけである。

## 7. チケットのステートマシン

### 7.1 状態

| 状態 | 持つもの | 終わりか |
|---|---|---|
| `in progress` | 今のステップ、ステップごとの答え、承認者、期限と催促の時刻、上へ回した回数 | いいえ |
| `paused` | `in progress` の全部と、保留を始めた時刻、保留が尽きる時刻 | いいえ |
| `with requester` | 再申請の期限と、差し戻した人（申請者が自分で取り下げたなら無し） | いいえ |
| `approved` | 終わった時刻 | はい |
| `rejected` | 終わった時刻と、却下した人、そのステップ | はい |
| `withdrawn` | 終わった時刻と、やめた人（進行中に取り下げて終わったなら、そのステップも） | はい |
| `expired` | 終わった時刻と、期限切れになったステップ（再申請の期限なら、ステップは無い） | はい |

どの状態も、フォームの値、申請の種類の版、`round`、その `round` を始めた試み（`attempt`）、差し戻しと取り下げを合わせた回数、承認者の直し（`edit`）が通った回数、残りの保留の日数、開いている仮押さえ（帳簿、振替、`round`）を持つ。下書き（出す前）はサーバーの物で、チケットの状態ではない。

**試み（`attempt`）。** `submit` と `resubmit` は、サーバーが付けた試みの名前を持つ。同じ操作を送り直したとき（ネットワークのやり直し、答えが届かなかった呼び出し）は同じ名前、新しい操作（値を変えた出し直し、断られたあとの出し直し）には新しい名前である。チケットは、今の `round` を始めた試みの名前を持ち（`ticket.attempt`）、帳簿の仮押さえは、申請とこの名前をキーにする（12.4 の「帳簿の仮押さえのキー」）。機械は名前を比べず、持って渡すだけである。シナリオとベクタは、`round` ごとに一つの名前を付ける（`attempt-1`、`attempt-2`）。

**差し戻した人（`sent_back_by`）。** 承認者か、その代理が申請者へ差し戻すと、チケットは、申請者に戻っているあいだ、差し戻した人を持つ。代理が差し戻したなら代理した人本人で、だれの席で答えたかは、履歴の `sent back` の行の `proxy_of` にある。前のステップへの差し戻しでは、チケットは進行中のままなので持たない。申請者が自分で取り下げて手元に戻したときも、だれも差し戻していないので持たない。申請者の手元を出るとき（出し直し、破棄、再申請の期限切れ）に無くなる。使うのは、申請者に戻っているあいだのコメントの知らせ（7.3）、サーバーが gate に渡す承認者（12.4）、だれが差し戻したかを申請者に見せる画面である。

**チケットの版（`revision`）。** `round`、今のステップの並びの位置（申請者に戻っているあいだと終わったあとは無し）、承認者の直しが通った回数の三つを一つにした値である。チケットから導き、遷移ごとに手で増やさない（参照インタプリタは `Ticket::revision`、機械のコードは `revision_of` の一か所で作る）。`start` と `next` が返すチケットの JSON は、これを `revision` に持つ（`{"round": 1, "stage": 0, "edits": 0}`）。版が変わるのは、ステップが進むか戻るとき、申請者に戻るか出し直したとき、終わったとき、承認者の直しが通ったときである。同じステップでほかの人が答えただけでは変わらない（議決を待つステップで、ほかの承認者に余計な断りを出さない）。上へ回す、承認者を足す、席を渡すのも、版を変えない（承認者でなくなった人は、`person` で断る）。

ステップは、一つずつ段階を持つ。まだ来ていない（`ahead`）、答えを待っている（`open`）、答えがそろった（`answered`）、承認者がいなくて飛ばした（`skipped`）、ほかのステップの承認で済んだ（`once`。5.12）、あとのステップの承認を待っている（`deferred`。5.12）、回覧した（`circulated`）、要約が届いたか、届かずに進んだ（`noted`）。ステップは、決め方ごとの部分（`and` でつないだ一つずつ）に、承認者、要る数、答えを持つ。答えは、答えた人と、その人が使った承認者の席を持つ（代理の答えなら、席は代わられた人。5.8）。部分も状態を持ち、ほかのステップの承認で済んだ部分（`once`）と、あとのステップの承認を待っている部分（`deferred`）は、そのステップの名前を持つ。

ステップの並びは、いつも時間の順である。`same approver approves once, at the last stage` で待たせたステップをあとで始めるときは、始める所に、同じ名前のステップをもう一度足す（`again`）。足したステップは、待っていた部分だけを持ち、承認者をそのときの版で決め直す。元のステップは `deferred` のまま残り、そこで答えた人の答えを持つ。まだ待っている部分は、足したほうのステップの物になる。こうすると、承認の取り消し（すぐ前の承認へ戻る）、前のステップへの差し戻し（そこから先をやり直す）、承認者の直し（今より後の経路を決め直す）が、並びの前と後を読むだけで決まる。取り消しか差し戻しで、足したステップより前へ戻るときは、足したステップを並びから外す。元のステップの待っていた部分は、また待つ。

### 7.2 イベント

| イベント | だれから | 受け付ける状態 |
|---|---|---|
| `submit` | 申請者か代理 | （新しいチケット） |
| `resubmit(form, clear)` | 申請者か代理 | `with requester` |
| `approve`、`reject`、`send back(to)` | 今のステップの承認者か代理 | `in progress` |
| `retract` | 前のステップで承認した人 | `in progress`（次のステップが答える前） |
| `pause` | 今のステップの承認者か代理 | `in progress` |
| `resume` | 今のステップの承認者か代理 | `paused` |
| `withdraw` | 申請者（代わりに出した人と、その日に有効な申請の代理も） | `in progress`、`paused`（`withdraw until` の間） |
| `discard` | 申請者（同じ） | `with requester` |
| `edit(fields, clear)` | 今のステップの承認者か代理（`approvers edit` のフィールド） | `in progress` |
| `add approver`、`hand over` | 管理する人（イベントの `by` はその人。人の無い操作なら無し。12.4） | `in progress`、`paused` |
| `comment` | gate が許す人（例の gate では、申請者の側の人（申請者、代わりに出した人、その日に有効な申請の代理）、今のステップの承認者（申請者に戻っているあいだは差し戻した人。12.4）、受け持つ部門の人） | `in progress`、`paused`、`with requester` |
| `remind`、`late` | 時計 | `in progress`（その時計を置いたステップ） |
| `pause ends` | 時計 | `paused` |
| `resubmit late` | 時計 | `with requester` |
| `agent answered(verdict, reason)`、`note ready(outputs)`、`flow failed` | flow | `in progress`、`paused`（エージェントか要約のステップ） |

時計のほかのイベントは、それを作ったときのチケットの版（`revision`）を持つ。人の操作は、画面や道具がその人に見せたチケットの版を、flow の答えは、その flow を頼んだ遷移が返したチケットの版を持つ。`submit`、`comment`、時計のイベントは版を持たない（時計は、チケットが持つ時刻で確かめる。`comment` は何も動かさないので、前に見せた画面から書いても、今のチケットに当たってよい）。シナリオは版を書かない。`run` とベクタは、そのときのチケットの版を付けて渡す。

**コメント。** 人の操作（`submit`、`resubmit`、`approve`、`reject`、`send back`、`retract`、`pause`、`resume`、`withdraw`、`discard`、`edit`、`add approver`、`hand over`）は、その人が書いた文（`comment`）を一つ持てる。無くてよく、空の文は無いのと同じに読む。機械は、その操作の履歴の行（その人がしたことの行）の `comment` に残す（7.4）。却下と差し戻しには、申請の種類が `ticket` に `comment required  for reject, send back` と書けば、理由が要る（書いた操作だけで、片方でもよい）。文が無いか、空白（Unicode の White_Space の 25 文字。全角の空白も U+0085 も入り、U+FEFF は入らない。kaname の `trim` が除く空白と同じ）だけの文なら `comment` で断る。確かめるのは文の長さのあとで、だれがする操作かより先である。理由を求めるのに、その操作がチケットに無ければ E409 にする（`send back` の行が無い申請の種類の `send back`、人の答えるステップが無い申請の種類の `reject`）。説明はステップごとに理由の要る操作を `comment_required` に挙げ（12.2）、承認カードは、文が空白だけのあいだ、その操作のボタンから送らない（13.1）。書かない申請の種類では、理由を書かずに却下も差し戻しもできる。どちらでも、書けば、申請者は何を直せばよいかを履歴で読める。イベント `comment` は、文だけを書く。履歴に一行（`commented`）を足して知らせを出すだけで、状態、時計、チケットの版を変えない。だれが書けるかは、管理する人の操作と同じに gate が決め、機械は見ない（例の gate の `comment`。9.3 の X18 は、ほかの操作と同じに、gate がこの action を宣言しているかを見る）。文は 2,000 字まで（数え方はフォームの `count` と同じに、文字で数える）で、越えれば `comment` で断る。`comment` の空の文も `comment` で断る。コメントの断りを `form` と別の種類にするのは、フォームに `comment` という名前のフィールドがあってもよく、場所（`at`）にフィールドの名前と同じ語を入れれば、どちらの断りかを読み違えるからである。エージェントの答えの理由（`agent answered` の `reason`）も、その行の `comment` に入れる（15.1）。

flow の答えは、保留のあいだも受け付ける。flow は済んだときに答えるので、断ると、サーバーが答えを持ち続けることになるためである。その答えでステップが済むなら、保留もそのステップと一緒に終わる（使った営業日を数え、`pause ends` の時計を取り消し、履歴に `resumed` と残す）。エージェントの疑いと flow の失敗は、`else` の決め方の人を承認者に置いて、保留のまま待つ。

受け付けないイベントは、チケットを変えずに、種類と文を付けて断る。種類は次の十二で、生成するコードとサーバーも同じ種類で断る（12.4）。文は人が読むもので、英語と日本語の二つを持つ（約束の JSON の `text` と `text_ja`。`--lang` によらず両方を書き、`run` の表のように人に見せる所では `--lang` の言語の文を見せる）。生成するコードも、参照インタプリタと同じ文を返す。フォームの断り（`form`）は、どのフィールドの断りかを場所（`at`）に持ち、コメントの断り（`comment`）は場所を持たない（12.4）。

| 種類 | 断るとき |
|---|---|
| `state` | その状態では受け付けないイベント。終わったチケットへのイベント、`with requester` での `approve`、`withdraw until` のステップを過ぎた取り下げ、次のステップが答えたあとの `retract` |
| `clock` | 時計のイベントで、その時計が置かれていないか、鳴る時刻より前。時計でないイベントで、鳴った時計のイベントがまだ渡されていない（7.6） |
| `stale` | 時計と `comment` のほかのイベントで、持つ版が今のチケットの版と違う（見せたあとで、チケットが進んだか、戻ったか、直されたか、出し直された）か、版を持たない。する人は、チケットを見せ直してもらう |
| `person` | する人が違う。承認者でも代理でもない、もう答えた、申請者自身（5.5）、その版の組織にいない |
| `event` | 申請の種類に無い操作（`pause` の行が無い申請の種類の `pause`）、戻せないステップへの差し戻し、もう承認者である人の `add approver` |
| `limit` | 差し戻しと取り下げの回数、保留の日数を使い切った |
| `form` | フォームが取らない値（範囲の外、行の数、必須、その日に選べないマスタの行）と、空にできないフィールドを空にすること（7.3）。次のステップの人を選ぶフィールドが空のままの承認 |
| `comment` | 人の操作に添えた文（`comment`）が 2,000 字を超える。イベント `comment` の文が無いか空。理由を求める却下か差し戻し（`comment required for`）の文が無いか空白だけ |
| `submit` | `submitted by` に当たらない申請者（5.13） |
| `data` | 使う日の範囲の外（9.6）、カレンダーのデータで数えられない期限、イベントの日と違う日の組織の部分（7.5） |
| `book` | 仮押さえの引数を作れない（`dept: requester.department` で、申請者の上に部が無い） |
| `unread` | 下ろしたファイルを読めない、規則の答えが一つに決まらない |

### 7.3 遷移

| 今 | イベント | 次 | することの一覧 |
|---|---|---|---|
| （無し） | `submit` | `in progress`（最初のステップ） | `on submitted` の処理、承認者への知らせ、期限と催促の時計 |
| `in progress` | `approve` で議決に届く | 次のステップ、無ければ `approved` | 今のステップの時計の取り消し、次のステップの知らせと時計。`approved` なら `on approved` の処理と、申請者への知らせ |
| `in progress` | `approve` で議決に届き、このステップを待っていたステップがある（5.12） | 待たせた人の席の承認があれば、待っていたステップが済んで次のステップ。無ければ、待っていたステップ（二度目として始める） | 済んだなら、待っていたステップの履歴の行（`approved`）。始めるなら、履歴の行（`deferral ended`）と、そのステップの知らせと時計 |
| `in progress` | `approve` で議決に届かない | 同じステップ（答えを足す） | 履歴の行だけ |
| `in progress` | `reject` で、議決にまだ届きうる | 同じステップ（答えを足す） | 履歴の行だけ |
| `in progress` | `reject` で、議決に届かなくなった | `rejected` | 時計の取り消し、`on rejected` の処理、申請者への知らせ |
| `in progress` | `send back`（申請者へ） | `with requester`（差し戻した人を持つ。7.1） | 時計の取り消し、`on sent back` の処理、申請者への知らせ、再申請の時計 |
| `in progress` | `send back(to: k)` | `in progress`（ステップ k、答えを空に） | 時計の取り消し、ステップ k の知らせと時計 |
| `in progress`、`paused` | `withdraw` | `with requester`。戻せる回数を使い切っているか、`send back` の行が無い申請の種類では `withdrawn` | 時計の取り消し、`on sent back` の処理と再申請の時計。`withdrawn` なら `on withdrawn` の処理 |
| `in progress` | `retract` | `in progress`（前のステップ。期限はそのまま）。その承認で済んでいたステップは、また待つ（5.12） | 今のステップの時計の取り消し、前のステップの知らせと、まだ鳴っていない時計 |
| `in progress` | `remind` | 同じステップ（催促が鳴ったことを持つ） | 承認者への催促の知らせ |
| `in progress` | `late`（`escalate`） | `in progress`（同じステップ、回した先の承認者、新しい期限と催促） | 回した先への知らせと、催促と期限の時計（催促は、期限より前に来るときだけ） |
| `in progress` | `late`（`escalate` で、回す先が無い。5.10） | `then stay` なら同じステップで待つ。ほかは `expired` | 履歴に「回す先が無い」と残す。`expired` なら `on expired` の処理 |
| `in progress` | `late`（`stay`） | 同じステップで待つ。時計はもう置かない | 履歴に `late` と残す |
| `in progress` | `late`（`expire`、または回したあと） | `expired` | `on expired` の処理、申請者への知らせ |
| `in progress` | `late`（要約のステップ） | 次のステップ（要約なしで進む） | 履歴に `note missing` と残す。次のステップの知らせと時計 |
| `in progress` | `pause` | `paused` | ステップの時計の取り消し、保留が尽きる時計、申請者への知らせ |
| `paused` | `resume`、`pause ends` | `in progress`（止めていた営業日だけ後ろへずらした期限で） | 保留の時計の取り消し、まだ鳴っていないステップの時計（催促は、ずらした期限より前に来るときだけ）、申請者への知らせ |
| `with requester` | `resubmit` | `in progress`（最初のステップ、`round` + 1）。値を当て、見せなくなったフィールドの値を落とす（下に書く） | 再申請の時計の取り消し、`on submitted` の処理、知らせと時計 |
| `with requester` | `discard` | `withdrawn` | 再申請の時計の取り消し、`on withdrawn` の処理 |
| `with requester` | `resubmit late` | `expired` | `on expired` の処理、申請者への知らせ |
| `in progress` | `edit` | 同じステップ。値を当て、見せなくなったフィールドの値を落とす（下に書く）。経路が読む値が変われば（空にしたか、落としたときも）、今より後の経路を決め直す。直しの回数が増え、版が変わる | 履歴の行。まだ答えていないエージェントには、直したフォームで尋ね直す（`ask agent`） |
| `in progress`、`paused` | `add approver`、`hand over` | 同じステップ（承認者が変わる。期限はそのまま） | 足した人、受けた人への知らせ（保留のあいだは出さない） |
| `in progress`、`paused` | `agent answered`、`note ready`、`flow failed` | ステップが済めば次のステップ。エージェントの疑いと失敗は、同じステップで `else` の人を待つ | 済めば、次のステップの知らせと時計 |
| `in progress`、`paused`、`with requester` | `comment` | 同じ（状態、時計、版は変えない） | 履歴の行（`commented`。今のステップ、書いた人、文）と、知らせ（`comment`）。知らせの相手は、申請者と代わりに出した人、申請者に戻っているあいだは差し戻した人（7.1）、今のステップで答えを待っている承認者（その日に有効な代理も）から、書いた人を除いた人。保留のあいだも出し、相手がいなければ出さない（申請者が自分で取り下げたチケットに申請者が書いても、代わりに出した人がいなければ出ない） |

承認者の答えは、今のステップにだけ数える。別のステップの承認者の答えは `person` で断る。

**確かめる順。** `next` は、組織の部分がイベントの日の物か（`data`）、鳴った時計のイベントがまだ渡されていないか（`clock`）、時計と `comment` のほかのイベントなら、持つ版が今のチケットの版か（`stale`）、チケットが終わっていないか（`state`）、人のイベントなら、添えた文が 2,000 字までか（`comment`）を、この順に確かめ、そのあとでほかの状態、する人、イベントの中身を確かめる（`comment` の文が空なら、ここで `comment`）。`submit` は、`submitted by`（`submit`）のあと、フォームの値より先に文の長さを確かめる。版を、する人と状態より先に確かめるのは、古い画面から操作した人に、「承認者ではない」でなく「見せ直す」と言うためである。同じ人が続く二つのステップの承認者でも、別のタブから送った二度目の `approve` は、前のステップの版を持つので、次のステップの承認にならない。出し直す前に頼んだエージェントの答えは、出し直したあとのチケットに当たらない。

**承認者の直し。** 承認者の直しより前に済んでいた答えは、直しで消さない（前のステップの答えも、同じステップの中で先に済んだ答えも）。直しより前の版で作った操作は `stale` で断るので、直したあとの値を見ずにした承認は数えない。まだ答えていないエージェントは直す前のフォームを確かめているので、直したフォームで尋ね直す。その答えは直したあとの版を持ち、直す前に頼んだ答えは `stale` で断る。答え済みのエージェントには尋ね直さない。

**議決と却下。** 部分は、承認が要る数に届けば済み、どの部分も済めばステップが済む。却下は、どれかの部分で、承認者の数から却下の数を引いた残りが要る数を下回ったとき、つまり、もう議決に届かなくなったときに、チケットを終える。届きうるあいだは、答えを足して待つ。承認者が一人のステップでは、一つの却下で終わる。五人のうち二人の承認が要るステップでは、四人目の却下で終わる。

**出せる人。** `submit` は、そのときに効いている版で、申請者（代理申請なら代わられた人）が `submitted by` に当たるかを見て、当たらなければチケットを作らずに断る。だれが、どの版で、どの行で出せないかを返す（5.13）。`resubmit` は `submitted by` で断らない。

**フォーム。** `submit`、`resubmit`、`edit` は、渡された値をフォームに照らす。人が入れるフィールドの範囲と字数、選択の値、その日に選べるマスタの行、必須、明細の行の数（4.3）を確かめ、計算するフィールドの値を下ろした規則に尋ねる。計算した値が書いた範囲の外なら、規則の入口が断り、機械はその断りを、フィールドの名前と範囲の文に直す（4.4）。どれも `form` で断る。

**値を空にすることと、見せなくなったフィールドの値を落とすこと（2026-11-16）。** `resubmit` と `edit` は、チケットが持つ値に、次の順で当てる。

1. 渡した値（`resubmit` の `form`、`edit` の `fields`）で、そのフィールドの値を置き換える。値が null のフィールドは渡していないのと同じで、チケットの値のままにする。
2. `clear` に挙げたフィールドの値を消す。値を持っていないフィールドを挙げても、何も変わらない。
3. 当てたあとの値で、`shown when` が成り立たなくなったフィールドの値を落とす。一度に落とすのは、その時点で隠れている物の全部である（宣言の順に一つずつではない）。条件は、落ちたフィールドや、それを読む計算を読むことがあるので、落としたあとの値でもう一度見て、落ちる物が無くなるまで繰り返す（値は減るだけなので、フィールドの数までで止まる）。1 で値を渡したフィールドは落とさない。計算が値を作れない（空のフィールドを読む、範囲の外）ときは、そこで落とすのをやめ、4 の検査が断る。
4. できた値をフォームに照らす（上の **フォーム。**）。見せていないフィールドに 1 で値を渡したときと、空にしたせいで必須が欠けたときは、ここで `form` で断る。

`clear` は、次の形を `form` で断る。値を渡したときと同じ文で断り、断りの種類は足さない。名前は `clear` の並びの順に見て、最初に当たった物を言う。

| 形 | 英語の文 |
|---|---|
| フォームに無い名前 | `the form has no field …` |
| 計算するフィールド | `… is computed, and is not given` |
| そのイベントで値を渡せないフィールド（出し直しでの承認者だけのフィールド、直しで `approvers edit` に無い申請者のフィールド） | 値を渡したときと同じ（`… is a field of the approvers only, which the requester does not fill in`、``… is not a field approvers may edit (it is not in `approvers edit`)``） |
| 同じイベントが値も渡しているフィールド | ``… is given a value, and is in `clear` too`` |
| どの申請でも必須のフィールド（`optional`、`required when`、`shown when` のどれも無い、申請者のフィールド） | `… is not filled in, and the form asks for it` |

見せているあいだ必須のフィールド（`shown when` があり、`optional` の無い物）と、`required when` が成り立つフィールドを空にすると、4 の検査が、必須が欠けたときの文で断る。通すのは、値を持っていないフィールドを空にすること（何も変わらない）と、当てたあとに隠れるフィールドを空にすること（3 で落ちるのと同じ値になる）である。

経路が読むフィールドを直しで空にしたか、直しで落としたときは、値を直したときと同じに、今のステップより後の経路を決め直す。前のステップの答えは変えない。直しで空にすることは値を直すことと同じに扱うので、仮押さえの条件か引数が読むフィールドを直せる形は、今のとおり E405 である（7.6）。直しの回数（`edits`）と版の数え方も、値を直したときと同じである（何も変わらなかった直しも一回と数える）。

その行で値が無くなったフィールド（空にした物と、落とした物）は、履歴の行（`resubmitted`、`edited`）の `cleared` に、宣言の順で残す（7.4）。直しの `to` は値を渡したフィールドで、`cleared` とは重ならない。

約束の JSON で、空にするフィールドを名前の一覧で渡し、値の null を「空にする」と読まないのは、null と無いキーを分ける読み方が、型のある言語のクライアントで取り違えやすいからである。値の無いフィールドを null で書き出す直列化は多く、null を空にすると読めば、変えないつもりのフィールドまで消える。null を無いキーと同じに読むので、エージェントが変えないつもりで null を書いても、値は消えない。名前の一覧は、あとで別の約束の形（型のあるメッセージ）に写すときも、そのまま写せる。シナリオの行の `none` は、申請の始めのフォームでも出し直しと直しの引数でも「そのイベントのあと、そのフィールドに値が無い」と読めるので、出し直しと直しでは空にすると読む（10.2）。

**承認者のいないステップ。** 次のステップへ進むとき（`submit`、議決に届いた `approve`、`resubmit`）に、そのステップの承認者がいなければ、`when empty` のとおりにする（5.4）。上へ回せたら回した先を承認者にし、いちばん上で `then <決め方>` なら、その決め方を申請者について解いた人を承認者にし、飛ばすなら次のステップへ進み、どれも履歴に残す（`went up`、`went to`、`skipped`。なぜ人がいないかを `note` に書く）。止まるなら（`stay`、いちばん上の `escalate` で `then skip` も `then <決め方>` も無いもの、`then` の決め方にも人がいないもの）、履歴に `stopped` と残し、管理する人に知らせて、同じステップで `add approver` を待つ。期限の時計は、ほかのステップと同じに、ステップが始まったときに置く（5.9）。どのステップにも答える人がいない申請者のチケット（W510。5.4）は、ステップを一つずつ飛ばして履歴に残し、出したとたんに `approved` になる。

**同じ承認者が二つ以上のステップに当たるとき。** `same approver approves once`（5.12）を書いた申請の種類では、議決に届いた `approve` で次のステップへ進むときに、そのステップの承認者が 5.12 の三つを満たせば、そのステップを済ませて、さらに次へ進む（済んだステップのあとの、承認者のいないステップも、同じ一度の遷移で扱う）。`submit` と `resubmit` で始めた最初のステップは済ませない。済んだステップには時計を置かず、知らせも出さない。次のステップの時計は、元の承認の時刻から始まる。することの一覧には、済んだステップごとに `record` を一つ入れ、`once_with` に元のステップを、`by` に元のステップでその人の席に答えた人を書く（代理が答えたなら、`by` は代理、`proxy_of` は代わられた人）。`send back(to: k)` で戻したステップ k と、期限を過ぎて回した先は済ませない。`retract` は、元の承認を取り消し、そのあとの済んだステップも一緒に戻して、元の承認のステップへ戻す。取り消せるのは、済んだステップの次の、ほかの人が答えるステップが答えるまでである。

`same approver approves once, at the last stage`（5.12）を書いた申請の種類では、ステップへ進むたびに（`submit`、`resubmit`、議決に届いた `approve`、`send back(to: k)`、flow の答え）、次の順に確かめる。どれも一度の遷移の中で行い、同じ入力には同じ答えを返す。

1. 前のステップの待っている部分のうち、待つ先を失ったものを、イベントと一緒に渡された組織の部分で決め直す。待つ先を失うのは、待つ先がこのステップで、待たせた人に一人では当たらなくなっているとき、待つ先のステップが、待たせた人の席の承認なしにもう済んでいるとき、待つ先が経路に無くなったとき（承認者の直しで行が変わった）である。決め直した承認者が、あとのステップに一人で答えるなら、そのステップを待たせ直して、履歴に `deferred` を残す（`note` に、なぜ待つ先を失ったかを書く）。そうでなければ、待たせたステップを二度目としてここで始め、履歴に `deferral ended` を残して、そのステップで待つ。経路の終わりまで来たときも、同じに確かめる。
2. そのステップを始める。部分ごとに、承認者が一人に決まり、その人があとのステップにも一人で答えるなら、その最後のステップを待たせて、履歴に `deferred` を残す（`once_with` に待つ先のステップ、`to` に待たせた人）。`send back(to: k)` で戻したステップ k は待たせない。どの部分も待つか済んでいれば、ステップは始まらずに `deferred` になり、次のステップへ進む。
3. そのステップも待つことになったとき、そのステップを待っていた部分は、同じ先を待ち続ける（履歴に `deferred`）。

議決に届いた `approve` でステップの答えがそろったときは、そのステップを待っていた部分ごとに、待たせた人の席の承認（本人か代理の承認）がそこにあれば、その部分を済ませ、待っていたステップの `approved` の行を履歴に残す（`by` に答えた人、代理なら `proxy_of` に代わられた人、`once_with` に答えたステップ）。席の承認が無い部分は待ったままにし、次へ進むときの 1 で始める。待っていた部分のあるステップは、ほかの部分の答えがそろうと `deferred` になり、待っていた部分が済むと `answered` になる（始まらなかったステップは `once`）。

待っているステップには時計を置かず、知らせも出さない。待たせた人は、ほかの部分に答えてもらっているそのステップで `approve` も `pause` もできない（`person`）。`retract` で先のステップの承認を取り消すと、その承認で済んだ部分は、また待つ。二度目として始めたステップの前の承認を取り消すと、二度目のステップを並びから外して、前のステップへ戻る。`send back(to: k)` は、待っているステップへも戻せる。戻したステップより後ろは、まだ来ていない並びに戻り、二度目のステップは外す。`withdraw until <ステップ>` は、そのステップが待っているあいだは閉じない。

**差し戻しと取り下げの回数。** 承認者の差し戻し（申請者へも、前のステップへも）と、申請者の取り下げを、合わせて数える。`round` が増えるのは再申請だけで、前のステップへの差し戻しでは増えない。回数を使い切ると、承認者の差し戻しは、どちらも `limit` で断る（承認か却下を選ぶ）。申請者の取り下げは、戻さずに `withdrawn` で終える。戻して出し直せば、回数の上限から数えたいちばん長い日数（7.6）を越え、断れば、申請者はチケットをやめられないからである。`send back` の行が無い申請の種類の取り下げも、回数の上限が無いので、`withdrawn` で終える。前のステップへ戻せるのは、このチケットでだれかが答えたステップ（ほかのステップの承認で済んだステップと、あとのステップの承認を待っているステップも）だけである。

**時計。** 時計のイベントは、機械が置いた時刻かそのあとに渡す。早ければ `clock` で断る。催促の時計は、期限より前に来るときだけ置く。催促が鳴ったかは、ステップが持つ（`reminded`）。始まったステップと、上へ回したステップの催促は、まだ鳴っていない。`late` で上へ回したステップには、回した時刻から数えた催促と期限を置く。保留から再開したステップと、`retract` で戻ったステップは、まだ鳴っていない時計を数え直して置く。期限は鳴っていなければ置き直す。催促は、鳴っていなければ、前に置いたかどうかによらず、期限より前に来るときに置く。前に期限より後に来たので置かなかった催促も、保留で期限がずれて前に来るようになれば置く。鳴った催促は、二度は置かない。時刻は、`retract` で戻ったステップでは元のまま（上へ回したあとのステップなら、回したあとの催促と期限）で、再開したステップでは止めていた営業日だけ後ろへずれる（7.5）。`add approver` と `hand over` は時計を動かさない。

### 7.4 することの一覧

`next` が返すのはデータで、サーバーが行う。どれも冪等キー（チケット、`round`、通し番号）を持ち、サーバーが二度行っても一度と同じになる。履歴の行と時計は、チケットと一緒に保存する物で、帳簿の操作も、帳簿が同じデータベースにあれば同じトランザクションに入れる。知らせと処理の開始（`launch`、`ask agent`、`ask note`）は、保存のあとに、保存した一覧から行う（12.4）。一度の遷移の中では、次の順に並べる。履歴の行、時計の取り消し、`post`、`void`、`hold`、`launch`、`ask agent` と `ask note`、知らせ、時計。

| すること | 中身 |
|---|---|
| `record` | 履歴に残す行。何をしたか（`submitted`、`approved`、`rejected`、`sent back`、`withdrawn`、`went up`、`skipped`、`stopped`、`paused`、`commented`、`ticket approved` など）、ステップ、だれが、代理ならだれの代わりか（`proxy_of`。下に書く）、どの版の組織で決めたか、ほかのステップの承認で済んだステップなら、その承認のステップ（`once_with`）、相手（回した先、足した人）、短い `note`（`late`、`nowhere to go up`、保留した営業日、エージェントの行なら flow の名前）、人が書いた文（`comment`。7.2。人の操作の行と `commented` の行、エージェントの答えの理由。ほかの行と、文の無い行は null）。一つの遷移が二つ以上の行を残すときも、操作に添えた文は、その人がしたことの行（最初の行）にだけ入れる。`same approver approves once, at the last stage` の行は三つある。待たせた（`deferred`。`once_with` に待つ先のステップ、`to` に待たせた人）、待たせるのをやめて始めた（`deferral ended`。`once_with` に待っていた先、`to` に決め直した承認者）、先のステップの承認で済んだ（`approved`。`once_with` に答えたステップ）。相手が変わって待たせ直したか始めたときは、`note` に、`the stage it waited for does not come to <人> alone`、`the stage it waited for was answered without <人>`、`the stage it waited for is not on the route`、`the stage it waited for waits too` のどれかを書く。`resubmitted` と `edited` の行は、その行で値が無くなったフィールド（空にした物と、見せなくなって落とした物。7.3）を、宣言の順に `cleared` に持つ。ほかの行の `cleared` は空 |
| `cancel timer` | 時計の種類（`remind`、`late`、`resubmit late`、`pause ends`）とステップ。置いてある時計だけを取り消す |
| `post`、`void` | 帳簿、振替、引数。そのチケットのその `round` の仮押さえが開いているときだけ出す。開いていなければ、行の条件と引数を読まずに飛ばす |
| `hold` | 帳簿、振替、引数。額は帳簿の刻みの整数、ほかは文字列にして、帳簿の振替の引数の形で渡す |
| `launch` | flow と引数（承認のあとの処理） |
| `ask agent`、`ask note` | エージェントか要約の flow と引数 |
| `notify` | だれに、何の知らせか（`waiting`、`reminder`、`went up`、`returned`、`decided`、`circulation`、`paused`、`resumed`、`stopped`、`comment`）。`stopped` は相手が空で、管理する人への知らせである（だれが管理する人かは gate が決める）。`comment` は、コメントが書かれたことの知らせで、文は履歴の行にある |
| `set timer` | 時計の種類、ステップ、鳴る時刻 |

**だれの代わりか（`proxy_of`）。** 人がした操作の行のうち、代理がした物のどれにも入る。履歴は監査の材料で、操作によって残ったり残らなかったりすると、読む人が履歴だけでは判断できないからである。

| 行 | `proxy_of` |
|---|---|
| 申請者の側（`submitted`、`resubmitted`、`withdrawn`、取り下げと破棄で終わった `ticket withdrawn`） | する人が申請者でなければ、申請者 |
| 答え（`approved`、`rejected`、`sent back`）と、却下で終わった `ticket rejected` | 代理が使った承認者の席（答えと同じに決める。5.8） |
| `paused`、`resumed`、`edited` | する人が今のステップの承認者でなければ、その人がその日に代わる最初の承認者（あとのステップを待っている部分を除き、部分と承認者の並びの順で） |
| `retracted` | 取り消した承認の席が、する人でなければ、その席 |
| ほかのステップの承認で済んだステップの `approved`（5.12） | 元のステップで席に答えた人が代理なら、代わられた人 |

管理する人の操作（`approver added`、`handed over`）、`commented`（コメントは書いた人の名で書く）、時計と flow の行には入らない。

履歴の `note` は、決まった英語の語と ID だけで書く（人とチームの名前は入れない。組織の部分は名前を持たないので、生成するコードが同じ行を作れるようにするため）。人とエージェントが書いた文は `note` に入れず、`comment` に、渡されたとおりに入れる。画面は、`did` と `note` を見せる言葉に直し、`comment` はそのまま見せる（13.4）。

### 7.5 時刻と組織の渡し方

時刻は、イベントが起きた瞬間（UTC の秒）で渡す。イベントの日は、申請の種類が使うカレンダー（共有の経路が使うものも）のうち、オフセットを書いた最初の物の UTC オフセットで読む（書いたカレンダーが無ければ UTC）。koyomi のカレンダーのオフセットは一つの決まった値（`offset +09:00`）で、日によって変わらないので、申請の種類ごとに一つの整数で言える。サーバーは、それを `types/<名前>.json` の `utc_offset_minutes`（UTC から東へ何分か。12.2）で読み、イベントの日と、渡す組織の版を決める。ステップの期限と催促は、下ろした `terms.cal` の関数（カレンダーごとに一つ）が、始まりの日を一つ受け取って計算する。日の終わりは、次の日の 00:00 である。

一つのイベントの時刻は一つで、カレンダーごとにその地の日がある。申請の種類の日（上のオフセットで読んだ日）は、渡す組織の版と、申請の種類を使える日（9.6）を決める。営業日は、カレンダーごとに、イベントの時刻をそのカレンダーのオフセットで読んだ日から、そのカレンダーの開いている日で数え、時計が鳴る時刻（`at 09:00`、日の終わり）も、そのカレンダーのオフセットで読む。つなぎは、カレンダーごとの下ろした関数に、そのカレンダーのオフセットで読んだ始まりの日を渡す。

期限を東京の時刻（`+09:00`）の本社のカレンダーで、催促を `offset -05:00` の工場のカレンダーで数える申請の種類（`remind after 1 business day in west at 09:00`）では、東京の月曜 10:00 に出したステップは、工場ではまだ日曜の 20:00 である。催促は、工場の日曜から数えて 1 営業日の月曜の、工場の 09:00 に鳴る。東京では月曜の 23:00 である。逆に、日を工場のオフセットで読み、催促を本社で数える申請の種類（`remind after 1 business day in office at 09:00`）では、工場の月曜 20:00 に出したステップは、東京ではもう火曜の 10:00 である。催促は、本社の火曜から数えて水曜の 09:00、工場の火曜 19:00 に鳴る。申請の種類の日（月曜）から数えれば東京の火曜 09:00 になり、出す 1 時間前に鳴ってしまう。カレンダーごとにその地の日から数えるので、`after N business days`（N は 1 以上）の時計は、どのオフセットでも、イベントの時刻より後に鳴る（機械の試験の `tests/machine/behind.ringi` と `ahead.ringi`、その日本語の名前の版）。

保留が尽きる時計（`pause at most … in <カレンダー>`）と再申請の期限（`resubmit within … in <カレンダー>`）も同じで、保留した時刻や申請者へ戻った時刻を、そのカレンダーのオフセットで読んだ日から数える。カレンダーを書かなければ、申請の種類の最初のカレンダーで数える。断りの文が時計の時刻を言うとき（`clock`。7.2）も、その時計のオフセットで書く（「the clock of `remind` is set for 2026-11-16 09:00, and it is 2026-11-15 22:00」）。チケットの JSON の時刻は、どれも UTC の秒で、オフセットを持たない。

時刻は、イベントごとに渡された物を読む。走り始めの時刻を持ち続けない（21 章の 2）。

- ステップの時計は、そのステップが始まった時刻から数える。前のステップの承認が遅ければ、その分だけ後になる。
- 上へ回したあとの期限と催促は、期限の時計が鳴った時刻から数える。下ろした `due_escalated` と `remind_escalated`（二度目は `due_escalated_twice` と `remind_escalated_twice`）を、同じ始まりの日から読む（5.10）。催促と期限を別のカレンダーで数えるステップの催促だけは、下ろした日付からでなく、期限の時計が鳴った時刻を催促のカレンダーのオフセットで読んだ日から、そのカレンダーの営業日で数える（5.10）。サーバーが `late` を遅れて渡しても、どちらも延びない。渡したときに、催促の時刻がもう過ぎていれば、渡した時刻に催促を置く。期限ももう過ぎていれば、渡した時刻を期限にして、催促は置かない。
- 保留のあとの時計は、ステップが始まった時刻を、止めていた営業日が保留のカレンダーで何日にわたるかだけ後ろへずらし、その時刻から数える。日数は、始まった時刻を保留のカレンダーのオフセットで読んだ日から、止めていた営業日を数えて決める。ずらした時刻を、カレンダーごとにそのオフセットで読んだ日から、期限と催促を数える。どのカレンダーの日も同じ日数だけずれるので、オフセットが一つの申請の種類では、始まった日を止めていた営業日だけずらすのと同じである。保留のカレンダーだけが休む日も、その日数に入る。工場の営業日で保留を数え、期限を本社の営業日で数える申請の種類（`pause at most 3 business days in west`）で、工場の日曜（東京の 11 月 30 日の月曜）に保留し、3 営業日を使って再開すると、工場が棚卸しで休む 12 月 2 日をはさむので、3 営業日（月曜、火曜、木曜）は工場の暦で 4 日にわたる。その日も開いている本社のカレンダーで数える期限も、ほかの時計と同じく 4 日あとの時刻から数え、保留をせずに 4 日あとに出した申請のステップと同じ時刻に鳴る。3 日だけずらすより 1 日遅い（機械の試験の `tests/machine/behind.ringi` の `paused_over_the_plants_inventory` と、その日本語の名前の版）。上へ回した先の人が保留したときも同じで、回したあとの催促と期限が後ろへずれる。
- 再開したステップは、まだ鳴っていない時計を、ずらした時刻から数え直して置く（7.2）。期限は、鳴っていなければ置き直す。催促は、鳴っていなければ（ステップの `reminded` が false）、ずらした期限より前に来るときに置く。前に期限より後に来たので置かなかった催促も、ここで前に来れば置き、鳴った催促は、前に来ても置かない。本社の営業日で催促と保留を数え、工場の営業日で期限を数える申請の種類（`ahead`）では、本社の年末年始の休みの前の日に出したステップの催促は、休みのあとに来て、工場の期限より後なので置かない。保留して年始に再開し、時計が休みのあとへずれると、催促は期限より前に来るので置く。再開したときに期限がもう過ぎていれば（保留のカレンダーが休んでいるあいだに、期限のカレンダーの営業日が過ぎた）、再開した時刻に期限を置いてすぐ鳴らし、催促は置かない。鳴ったかを時計の数え直しで決めないのは、カレンダーの休みの表が、進行中のチケットの下で差し替わりうるからである（7.7）。臨時の休みを足した表で数え直すと、前に置いたときの催促と期限の前後が入れ替わり、鳴った催促をもう一度鳴らすか、置くはずの催促を置かないことになる（機械の試験の `tests/machine/ahead.ringi` の `set_after_the_new_year`、`set_at_the_second_resume`、`rung_between_two_pauses`、`late_when_resumed` と、その日本語の名前の版。`tests/rewritten/` の、カレンダーを書き換える前と後の同じ申請の種類）。
- 再申請の期限は、申請者へ戻った時刻を再申請のカレンダーのオフセットで読んだ日から数える。
- `retract` で戻ったステップの期限と催促は、始まったときのままである（上へ回したあとなら、回したときのまま）。

組織は、`now` の日に効いている版の中の、そのチケットとイベントに要る部分だけを渡す。関数は組織を読みに行かない。渡された部分の日がイベントの日と違えば、`data` で断る。部分の切り方は次のとおり一つに決まっていて、参照インタプリタもサーバーも同じに切る。ベクタの `organization` は、こう切った部分である。

部分は、次の人、チーム、マスタの行から始める。

- **申請の種類がいつも要る物。** `types/<名前>.json` の `organization.needs` に書く（12.2）。`people` は、経路（使う共有の経路も）の決め方が ID で指す人と、`submitted by` のグループと除くグループに書いた人である。`teams` は、`submitted by` のグループと除くグループに書いたチームである。`members_of` は、経路の決め方が、所属する人の全員を承認者にするチームで、そのチームと、所属する人の全員を入れる。
- **チケットが持つ人。** 申請者（`requester`）、代わりに作った人（`made_by`）、どのステップのどの部分（`parts`）についても、承認者（`approvers`）と、答えた人とその席（`answers` の `by` と `seat`）である。ステップの部分がチーム（`team`）を持てば、そのチームと、所属する人の全員も入れる。差し戻した人（`sent_back_by`）は入れない。
- **イベントが持つ人。** `requester`、`by`、`approver`、`of`、`to` である。`send_back` の `to` はステップの名前なので、人と読まない。
- **フォームの値。** チケットの `form`、`submit` と `resubmit` の `form`、`edit` の `fields` の順に、それぞれフィールドの宣言の順に読む。人のフィールドはその人を、チームのフィールドはそのチームを入れる。マスタから選ぶフィールドは、その日に選べる行（`valid_from` と `valid_to`）を `rows` に入れ、行の人の列が指す人も入れる。同じマスタの同じキーの行は一度だけ書く。明細の行の中の値は読まない。

そこへ、次の物を、増えなくなるまで足す。

- 部分の人が所属するチーム
- 部分のチームの、上のチームの並び（`parent`）と、それぞれの長（`head`）
- その日に効いている、その種類の代理（種類を書かない代理か、その種類を書いた代理）のうち、本人が部分にいる物の、代理をする人

`submitted by` を書いた申請の種類の `submit` が要る、申請者の主の所属から上のチームの並びと、主の所属での役職も、これで部分に入る。

部分に書くのは、その日（`day`）、版の日（`version`）、役職の並びの全部（`positions`。低い順）、部分のチームと人、その人の所属、本人が部分にいる代理、マスタの行である。チーム、人、所属、代理は、データの順に書く。版のデータに無いチームと人は書かない。人は、ID と、データが書く名前（`name` と `name_ja`。無ければ空の文字列）で渡す。断りの文が、人を ID と名前で言うからである（「e1201 (Sales staff 1) made the request, and does not answer it (`self approval not allowed`)」）。

これより多く渡しても、答えは変わらない。版の全部を渡しても同じである（`tests/vectors.rs` の `more_of_the_organization_is_answered_the_same` が、どのベクタでも確かめる）。部分を切るのは、一つのイベントの費用が、組織の大きさでなく、チケットの大きさで決まるようにするためである。

### 7.6 どのチケットも終わること

終わりでない状態には、どれも時計の出口がある。`in progress` は期限（`when late`）、`paused` は保留の残り、`with requester` は再申請の期限である。差し戻しは回数に上限があり、承認の取り消しは期限を動かさず、保留は合計に上限がある。だから、終わりの無い待ち（下の三つ）を書かない限り、どのチケットも終わる。ringi はこれを確かめ、いちばん長くかかる場合の営業日を数える。

**鳴った時計が先。** `next` は、鳴った時計（時刻が `now` 以下の時計）があるあいだ、時計でないイベントを `clock` で断る。サーバーは、先にその時計のイベントを渡す。これが無いと、期限を過ぎたステップへ `retract` で戻して同じ秒に承認し直せ、次のステップの時計が新しく始まるので、取り消しと承認をくり返して、いくらでも延ばせる。時計のイベントをサーバーがどれだけ遅れて渡しても、チケットは同じ道を通る。

**数え方。** ステップは、長くて次の営業日だけかかる。期限で切れるか、要約のステップなら、期限の日数 N。期限を過ぎて一度上へ回すなら 2N + e、二度回すなら 3N + 2e。e は、期限が日の終わりなら 1（上へ回るのは次の日の 00:00 なので、回す日の一日が入る）、時刻なら 0 である。一つの `round` は、経路のいちばん長い並びのステップを足した S（条件で来るステップは、来るものとして数える）。申請者へ戻せる回数を K、再申請の期限を R、保留の合計を P とすると、合計は (K + 1) × S + K × R + P になる。

経費の例では、期限は 3 営業日の日の終わりで、一度上へ回すので、一つのステップは 7 営業日。人が答えるステップは一つの `round` で最大三つなので S = 21。差し戻しは 3 回まで、再申請は 3 営業日までなので、4 × 21 + 3 × 3 = 93 営業日になる。`check` は、数えた結果を `ok` の行の下に書く。

```text
$ ritsu ringi check examples/expense.ringi
…
  ends within 93 business days at the most (4 × 21 at its stages, 3 × 3 with the requester)
…
```

ほかの例は、購買 246（4 × 54 + 3 × 10）、アカウント 55（3 × 15 + 2 × 5）、契約 192（4 × 38 + 3 × 10 + 10）、休暇 40（3 × 10 + 2 × 5）で、どれも例の主張 `every ticket ends within … business days` に書いた数と同じである。購買の取締役会のステップは上へ回さずに期限で切れるので 10 営業日、契約の要約のステップは期限で要約なしに進むので 3 営業日、契約の保留は合わせて 10 営業日である。

`same approver approves once, at the last stage`（5.12）を書いた申請の種類では、決め方が二つ以上のステップ（`a and b`）を、決め方の数だけ数える。一つの部分にそこで答えてもらい、待たせた部分をあとで始めると、そのステップの期限と催促が、始めた時刻からもう一度数えられるからである。決め方が一つのステップは、待つあいだ時計が無く、始まるのは一度なので、数は変わらない（購買とアカウントの数は、書き方を替える前と同じである）。機械のための申請 `tests/machine/waits.ringi` は、最初のステップが `manager and accounting` で、上長の部分があとのステップを待つ。一つのステップは 7 営業日で、一つの `round` は 2 × 7 + 7 + 7 = 28、数は 3 × 28 + 2 × 3 + 5 = 95 営業日になる。いちばん長くかかるチケットは 94 営業日で、最初のステップを一度だけ数えた 74 を越える。仮押さえが開いているいちばん長い営業日（9.3 の X19 が読む数）も、同じ S から数える。

数は上限である。主張の日数が数以上なら、主張は必ず成り立つ。数のとおりにかかるチケットが、あるとは限らない。条件どうしが両立するかは読まず、上へ回す日が休みに当たる並びでは、一つのステップが 2N + e より短くなる（机の申請 `tests/machine/desk.ringi` は、数が 74 営業日で、いちばん長くかかるチケットは 73 営業日）。E401 は「数が主張を越えた」ことを言い、数え方を添える。

**終わりの無い待ち。** 次の三つは、時計の出口が無い。

- 期限を過ぎても待つステップ（`when late stay`、`when late escalate, then stay`）
- 期限を書かないステップ
- `send back` があるのに `resubmit within` が無い申請の種類の、申請者の番

`every ticket ends within` の主張があれば、三つとも E401 で、その待ちを言う。主張が無ければ、`stay` だけを W402 にする（期限を書かないことは誤りではないので、警告にしない）。三つとも、`ok` の行の下に `may never end: …` と書く。

**仮押さえ。** `on submitted hold` で押さえた仮押さえが、開いたまま迎えうる終わり方（承認、人が答えるステップがあれば却下、`send back` があれば差し戻し、`withdraw` か `send back` があれば取り下げ、期限で切れるステップか `resubmit within` があれば期限切れ）のどれかで、確定も取消もされなければ E405。`when` の付いた `post` と `void` は、仮押さえに同じ `when` が付いているときだけ数える。`hold` の無い振替への `post` と `void` は E406、起きない終わり方への `on` の行は E407、試みを見分けないキーの仮押さえ（振替のキーが `ticket.attempt` を受け取らない）は E408 である（12.4 の「帳簿の仮押さえのキー」）。

仮押さえの条件（`when`）か引数が、承認者の直せるフィールド（`approvers edit`）を読むときも E405 である。そのフィールドから計算するフィールドを通して読むときも、そのフィールドを入力にする規則の出力を通して読むときも同じである。仮押さえは、申請を出したときの値で一度だけ行い、承認者が直しても押さえ直さない。`post` と `void` が帳簿に渡すのは、チケットと試み（`attempt`）のキーだけなので、額を直したあとに確定されるのは、押さえたときの額である。申請の額とは食い違う。条件のフィールドを直して条件が成り立たなくなれば、同じ条件の `post` と `void` は行われず、仮押さえが開いたまま終わる。診断は、読んでいる所を指し、どのフィールドを、何行目の `approvers edit` が直させるかを言う。

```text
$ ritsu ringi check tests/mutants/E405_a_hold_of_what_approvers_edit.ringi
error[E405]: tests/mutants/E405_a_hold_of_what_approvers_edit.ringi:23:104: The hold `budget.reserve` reads `amount`, a field approvers may edit
    23 |   on submitted    hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: dept, amount: amount)
  = The argument `amount` of the hold reads `amount`, and `approvers edit` on line 22 lets an approver edit `amount`.
  = A hold is made once, with the values the request is submitted with, and is not made again when an approver edits them.
  = What is posted is what was held, which is no longer what the request says.
  = Take `amount` out of `approvers edit` (the line itself, where it lists no other field).
```

直し方は、そのフィールドを `approvers edit` から外すことである。機械が `when` を読まずに確定と取消をする形にはしない。条件が成り立たなくなった仮押さえを確定してしまい、額の食い違いも残るからである。承認者の直しで押さえ直すことは、後の版に回す（20.2）。五つの例に、この形は無い。

### 7.7 進行中のチケットと、版

- 組織の版が切り替わっても、承認者が決まったステップはそのまま進む。次のステップは、始まるときに効いている版で決める（8.7）。答える人、差し戻す人、申請者の側の操作をする人は、そのイベントの日に効いている版にいなければならない（`person` で断る）。
- 申請者が、ステップが始まるときの版にいないことがある（出したあとに辞めた）。そのとき、申請者から決める決め方（上長、部の長）は、だれも見つけない。これを `when empty` のとおりに飛ばすと、だれも答えないまま承認で終わるので、`when empty` に何と書いてあっても、止まって管理する人の `add approver` を待つ（履歴の `note` に `the requester <id> is not in the organization from <日>` と残す）。`then skip` は、組織のいちばん上の人のために書く物で、いなくなった人のための物ではない。チームや指名のように、申請者を読まない決め方は、今までどおりに人が決まる。申請者へ戻ったチケットは、申請者が版にいなければ再申請も破棄もできず、再申請の期限で切れる。
- 申請の種類の版を上げても（`v1` から `v2`）、進行中のチケットは出したときの版で終わる。新しい申請は、`v2` が効き始めてから `v2` で出す（14.2）。
- 使う日の範囲（9.6）の終わりを過ぎると、新しい申請と再申請と、ステップを始めるイベントを `data` で断る。データを延ばせば、止まっていたチケットは、同じイベントで進む。

## 8. 組織

### 8.1 形の宣言

組織のファイルは、役職を低い順に、チームの種類を広い順に並べ、データの置き場を書く。

```
organization company v1 "Company"
positions
  staff         "Staff"
  manager       "Manager"
  dept_head     "Head of department"
  division_head "Head of division"
  president     "President"
team kinds
  company    "Company"
  division   "Division"
  department "Department"
  section    "Section"
  committee  "Committee"
data "../org"
```

役職は順序のある列挙で、`nearest dept_head above requester` は「その役職以上」で比べる。

### 8.2 データ

組織のファイルの `data` の行が、データの置き場を、そのファイルからの相対のパスで言う。置き場の下に、効き始める日の名前のディレクトリを一つずつ置く。各版は三つの CSV を持つ（同じ形の JSON の `organization.json` 一つでもよい。`teams`、`people`、`members` の配列に、CSV の列をキーにしたオブジェクトを並べる。値は文字列か null で、null と、書いていないキーは空のセルと同じに読む。`members` の `primary` だけは `true` と `false` も書ける。数、配列、オブジェクト、`primary` のほかの真偽値は、`organization/schema.json`（12.2）が断る値なので、E501 にする）。

| ファイル | 要る列 | 無くてもよい列 |
|---|---|---|
| `teams.csv` | `id`、`name`、`kind`、`parent`、`head` | `name_ja` |
| `people.csv` | `id`、`name` | `name_ja`、`email`、`language` |
| `members.csv` | `person`、`team`、`position`、`primary`（`yes` が主の所属。`no` か空は兼務） | |

`kind` と `position` は、組織のファイルが宣言した名前で書く（日本語の名前 `部(department)` なら、`部` でも `department` でもよい）。`parent` と `head` は空でもよい（いちばん上のチーム、長の空いたチーム）。診断と `simulate` は、人とチームを ID と名前の両方で言い、日本語の出力では `name_ja` があればそれを使う。

`language` は、その人が知らせと画面を読む言語で、`en` か `ja` で書く。空なら、申請の種類ごとに、その種類の言語で読む。ほかの値は E501 にする。サーバーは、知らせの題と文をこの言語で出し、ページの既定の言語にも使う（その人がページで選んだ言語、ブラウザが送る言語の順のあとで、種類の言語より先に）。言語は人に一つで、版ごとに持つので、変えるのは名前と同じく新しい版である。版の `organization.json` の JSON Schema と列の説明（12.2）、`get_organization` の答えの人（12.4）も、`language` を持つ。

代理は版をまたぐので、置き場の直下の `proxies.csv` に置く（5.8）。`--org <ディレクトリ>` を渡すと、どの組織のファイルの `data` の行の代わりにも、そのディレクトリを読む（別の場所に用意した次の版を、置き換える前に確かめるとき）。

データは、組織のファイルを確かめるときに一度だけ読み、それを読む申請の種類は、そこから版を受け取る。読むのは `ritsu_base::fs` を通すので、ブラウザで試すページのメモリの上のファイルも、ディスクのファイルと同じに読める。診断は、データのファイルを、組織のファイルを渡したときの書き方から `data` の行のとおりにたどったパスで言う（`examples/shared/company.ringi` を渡せば `examples/org/2027-04-01/teams.csv`）。渡していない絶対パスは出さない。`ritsu gen --target requests` は、申請の種類が読む組織のファイルから、版の `organization.json` の JSON Schema と、CSV の列の説明を書き出す（`organization/schema.json`、`organization/columns.md`。12.2）。`kind` と `position` は宣言した名前（書いたままと ASCII の両方）の列挙で、`primary` は JSON では真偽も `yes`、`no`、空も受ける。

### 8.3 版と期間と予約

版のディレクトリの名前の日が、その版が効き始める日である。ある日に効いている版は、その日以前でいちばん新しい版で、次の版の前の日まで効く。先の日付の版を置けば、切り替えの予約になる。

検査は、置いてあるどの版にもかける（予約した版も）。申請の種類の一覧（8.6）には、その申請の種類を使う日の範囲（9.6。`effective from` があればその日から）と重なる版だけを入れる。`effective from` が最初の版より前なら、その間はどのステップにも承認者がいないので E502 にする。

`--at <日付>` を渡すと、その日に効いている版だけを確かめる。その日に効いている版が無いとき（最初の版より前の日）は、承認者を決められないことを W500 で言い、組織のファイルの行もそう言う。

```text
$ ritsu ringi check examples/leave.ringi examples/shared/company.ringi --at 2025-01-01
warning[W500]: examples/leave.ringi:5:1: No version of the organization is in effect on 2025-01-01, the day `--at` gives, so whether each stage has an approver is not decided
     5 | use organization org from "shared/company.ringi"
  = The first version takes effect on 2026-04-01.
  = Give `--at` a day on or after 2026-04-01, or add a version's directory that takes effect by 2025-01-01.
ok examples/leave.ringi: a request, 6 fields, 2 stages
  ends within 40 business days at the most (3 × 10 at its stages, 2 × 5 with the requester)
ok examples/shared/company.ringi: an organization, 5 positions, 5 kinds of team
  no data: no version is in effect on 2025-01-01, the day `--at` gives; the first takes effect on 2026-04-01
```

組織のファイルの `ok` の行の下には、読んだデータを書く。`--at` を渡せば、確かめた一つの版を書く。

```text
$ ritsu ringi check examples/shared/company.ringi
ok examples/shared/company.ringi: an organization, 5 positions, 5 kinds of team
  data `../org`: 2 versions (2026-04-01, 2027-04-01), 13 teams and 23 people in the last, 2 proxies
$ ritsu ringi check examples/shared/company.ringi --at 2026-11-16
ok examples/shared/company.ringi: an organization, 5 positions, 5 kinds of team
  data `../org`: the version from 2026-04-01, the one in effect on 2026-11-16 (`--at`), 14 teams and 24 people, 2 proxies
```

### 8.4 データを読むときの検査

次は E501 にする。列が足りない、知らない列、列とは違う個数の値を持つ行、1 MiB を越える行や 1,000 個を越える値を持つ行、引用符で囲んでいない値の中の引用符や閉じた引用符のあとに続く文字（どれもサーバーの CSV リーダーが断る形）、空の ID、255 バイトを越える ID、同じ ID が二度、宣言していないチームの種類や役職、版にいない人やチームを指す所属の行、`primary` が `yes` でも `no` でもない、同じ人とチームの行が二つ、主の所属が一人に一つでない、`parent` の指すチームが無い、長がその版の人でないかそのチームの人でない、`parent` の輪、30 段より深いチーム、日付の名前でない版のディレクトリ、CSV と `organization.json` の両方がある版、`organization.json` の文字列でも null でもない値（`primary` の真偽値のほか）。値の中の、サーバーが断る文字も E501 にする：ID、名前、アドレス、マスタの値のどれにも、制御文字（Cc）、書式の文字（Cf。ゼロ幅の結合子と結合しない子と、バイトオーダーマーク U+FEFF を含む）、行や段落を区切る文字（Zl、Zp）は置けない。名前は 200 字（Unicode のコードポイント）まで、アドレスは 254 バイトまでで、アドレスの形は、空白を落としたあと `@` がちょうど一つで前と後に文字があり、空白と `"(),:;<>[\]` を含まない。同じ版の二人のアドレスが、空白を落として大文字小文字をたたむと（Go の `strings.EqualFold`。U+212A と `k`、U+017F と `s` も同じ）同じなら、片方の行で言う。人の `language` は `en`、`ja` か空である。マスタの値は 1,000 字までで、キーは空でなく 255 バイトまでである。`organization.json` の、最上位でも配列の要素の中でも、知らないキー（大文字と小文字の違うキーも）は E501 である。代理の行の誤り（`kind` が `approve` でも `submit` でもない、日付でない `from`、`from` より前の `to`、自分が自分の代理）も E501 にする。問題は 1,000 件で止めず、全部を言う。

```text
error[E501]: org/2026-04-01/teams.csv:3:1: The kind `section` of the team `sales` is not a kind of team the organization's file declares
     3 | sales,Sales,営業部,section,company,e02
  = The kinds of team declared: company, department
```

一つの誤りは、その場所で一度だけ言う。どの誤りもデータのファイルの行で言い（ファイル全体の誤りは 1 行目）、その誤りから決まる検査は、決められないので言わない。版の中では、列、行（人、チーム、所属）、主の所属と親と長、輪と深さの順に確かめ、前の段に誤りがあれば次の段に進まない。所属の読めない版で輪を探せば、同じ誤りを別の言い方で重ねることになるからである。データが通らない組織のファイルは通らず、それを読む申請の種類は `use` の行の E202 で待ち（`ritsu check` では「待っている」行になる）、承認者の一覧を出さない。言わないことで通ることは無い（組織のファイルが通らないので、終了コードは変わらない）。

### 8.5 承認者の決め方の解き方

決め方ごとに、版のチーム、人、所属から、ステップに答えられる人を決める。

- **上長**（`manager of requester`）：申請者の主の所属のチームの長。長が空か申請者自身なら、上のチームの長へたどる。
- **チームの長**（`head of requester.department`）：主の所属のチームから上へたどり、その種類のチームに着いたら、その長。長が空なら空のまま（5.4 の `when empty` が決める）。長が申請者自身で自己承認を許さないとき、上にその種類のチームが無いときも空。
- **最も近い役職の人**（`nearest division_head above requester`）：主の所属のチームから上へ、チームの長をたどり、申請者でない長のうち、そのチームでの役職がその役職以上の最初の人。いなければ空。
- **チーム**（`team accounting`）：そのチームの人の全部（主の所属でも兼務でも）。
- **人の指定**（`person "e9001"`）：その人。その版にいなければ空。
- **マスタから引く**（`lookup m.systems.owner by system`）：その版の日々に有効なマスタの行ごとの人。行ごとに別の場合として数え、診断は行のキーを添える（``(the row `erp` of the master `systems`)``）。
- **エージェント**（`agent precheck_agent, else manager`）：組織からは `else` の決め方を解く。flow に尋ねるのは、チケットを動かすときである（7.3）。
- **指名**（`chosen by …`）：チケットが動くときに人が入れるので、組織からは決めない。
- **自己承認**：`not allowed` なら、決まった人から、申請者と、代わりに出した人（代理申請のとき）を外す。外したあとにだれもいなければ、5.4 のとおり。
- **上へ回す**（`when empty escalate`）：決め方が止まったチームの上へ、チームの長をたどり、外す人でない最初の長（5.4）。決め方が人なら、その人の主の所属のチームから。チームに着かなかった決め方は、申請者の主の所属のチームから。いちばん上より上にはだれもいないので、`then skip` なら飛ばし、`then <決め方>` ならその決め方を申請者について解いた人へ回し（そこにも人がいなければ止まる。E502）、どちらも無ければ止まる（E502）。版にいない人とチームからは回せない（E503）。期限を過ぎて上へ回す先（`when late escalate`）も、同じたどり方で決める（5.10）。

たどる回数には上限（チームの深さ 30）を置き、データに輪があれば E501 で止める。チケットの機械（`src/lower/ticket.kn`）の `chain` は、たどることを再帰でなく `reduce(numbers(1, 30), …)` で書く。再帰は kaname が `diverges` と数え、total にならないからである。

### 8.6 組織のデータがあるときと、無いとき

データがあると、版ごと、申請の種類ごと、申請者ごとに、各ステップの承認者を 8.5 のとおりに決めて、次を言う。

| 起きること | コード |
|---|---|
| 承認者のいないステップで、チケットがそこで止まる。`when empty stay`（書かなければこれ）か、いちばん上で上へ回せない `when empty escalate`（`then skip` も `then <決め方>` も無いもの、`then` の決め方にも人がいないもの） | E502 |
| 承認者のいないステップが、組織の形で、`escalate` で上へ回るか、`skip` で飛ぶか、いちばん上で `escalate, then skip` で飛ぶか、`then <決め方>` の決め方へ回る。組織の形とは、承認者が申請者だけで自己承認を許さない、申請者がいちばん上にいる、決め方の種類のチームが上に無いことである（5.4） | `ok` の下で数える |
| 同じことが、データの欠けで起きる。データの欠けとは、長の空いたチーム、人のいないチーム、版にいない人かチーム、上へたどる途中の空いた長の席である（5.4） | W502 |
| `escalate` のステップの決め方が、版にいない人かチームを指していて、上へ回す元が無い | E503 |
| 並列のステップの議決に、申請者を除いたチームの人数が届かない | E504 |
| 決め方の違う二つのステップに、一人だけが答える（`ticket` の `same approver` の行で、答えが一度になるものは除く。5.12） | W506 |
| 期限を過ぎて上へ回すステップ（`when late escalate`）に、組織の形で回す先がだれもいない。期限を過ぎる人（`when empty escalate` で回った先の人も）がいちばん上にいる、チームの上にだれもいない、表の決め方が組織の形で人を見つけない（5.10） | `ok` の下で数える |
| 同じことが、データの欠けで起きる。期限を過ぎる人の上の長の席が空いている、表の決め方がデータの欠けで人を見つけない、データの欠けで回ってきたステップの人の上にだれもいない（5.10） | W507 |
| `submitted by` が、版にいないチームか人を指す（5.13） | E508 |
| 版のだれも `submitted by` に当たらず、この申請をだれも出せない（5.13） | E509 |
| 承認者のいないステップがどれも飛ばされ、ほかに答える人のいるステップが無い申請者がいる。チケットは、出したとたんに、だれも答えないまま承認で終わる（5.4） | W510 |

申請者は、版のどの人も（`submitted by` を書いた申請の種類では、出せる人だけ。5.13）と、代理申請で出せる申請の形（5.8。代わられた人が出せるときだけ）である。出せる人で絞った申請の種類は、`ok` の行の下に、版ごとの出せる人の数を書く。組織の形でほかへ回るか飛ぶステップのある申請者と、期限を過ぎても組織の形で上へ回せないステップのある申請者も、`ok` の行の下に、版ごとに、出せる人のうちの人数で書き、だれの申請かを見るコマンド（`ringi simulate <file> --every-requester`）を添える。警告は、人が直すか確かめることのあるものだけにし、書いたとおりに回るものは数で知らせる。経路は、表なら行ごとに、ステップの条件の形なら全部のステップを一つの並びとして読む。条件は読まないので、両方は来ないことのある二つのステップも、両方来るものとして比べる。W506 は、一人しか答えられないステップだけを数える。だれか一人が答えればよいステップ（二人以上のチーム）と、上へ回したあとの人は数えない。`same approver approves once` を書いた申請の種類では、あとのステップが前の承認で済む二つのステップを言わず、済まないものには、なぜ済まないかと、`, at the last stage` を足せば一度になることを注に添える。`same approver approves once, at the last stage` を書いた申請の種類では、前のステップが後のステップの承認を待つ二つのステップを言わない。書いていない申請の種類で、書けば一度になるものには、その書き方を注に添える（5.12）。

数千人の組織でも読めるように、まとめて言う。

- 同じ版、同じステップ、同じ理由の申請者をまとめ、数と 3 人までの名前を出す。上へ回すステップは回す先の人ごとに、W506 は二つのステップに答える人ごとに、W507 は期限を過ぎる人ごとにまとめ（チームのステップは一人に決まらないので、申請者だけを言う）、まとまりは三つまで見せて、残りは数で言う。
- 二つの版で同じことを言うなら、一つにまとめて両方の版を言う（`In the versions of the organization from 2026-04-01 and 2027-04-01`）。表の行が違うだけなら、最初の行で言い、ほかの行を添える（`The same on the rows of lines 33, 35, 37.`）。
- 一つの場所で言うことは、経路のステップの順に、同じステップなら見つけた順に並べる。英語と日本語で同じ順になる。
- JSON の `witness` には、版、ステップ、申請者の全部を ID で入れる（16.2）。テキストで名前を省いた申請者も、ここで全部読める。`ok` の下の数のもとになった一件ずつは、JSON の `facts` の `cases` に入れる（版、ステップ、承認者の決め方、何が起きるか、理由、注、行、申請者の全部）。

3,000 人、300 チーム、版が二つの架空の組織で測ると、リリースビルドで、申請の種類一つの `check` は 0.06〜0.08 秒、`simulate --every-requester` は 0.09〜0.16 秒だった（期限をカレンダーで数える申請の種類では、どちらにも 0.18〜0.21 秒が足される。下ろした `terms.cal` の 146,097 日を koyomi が数える時間で、組織の大きさには依らない）。

`examples/` の例を二つの版で検査すると、案に書いた三件は次のとおりに出る（`ritsu ringi check examples` の出力から）。

```text
error[E504]: examples/contract.ringi:31:12: In the organization from 2027-04-01, the stage `second` cannot get 2 approvals for some requesters
    31 |   second   at least 2 of legal  when value > 5_000_000JPY
  = `legal` has only 1 member besides the requester.
  = For 2 requesters: e9002 (Lawyer 2) and e9003 (Lawyer 3).
  = Add people to the team, or change the quorum.
error[E503]: examples/contract.ringi:41:3: In the organization from 2027-04-01, `counsel` names someone who is not there
    41 |   counsel  = person "e9001"
  = `counsel` names e9001, who is not in the organization from 2027-04-01.
  = The stage `counsel` is `when empty escalate`, and it cannot go up from someone the organization does not have (what `then` says holds only where nobody is above).
  = Correct the ID, or add the person (the team) to the data of the version.
warning[W506]: examples/expense.ringi:37:93: In the organization from 2027-04-01, one person answers two stages of a ticket
    37 | | purchase | <=1_000_000JPY             |             | manager | accounting               | dept_head     |
  = The stage `first` is `manager` and the stage `decide` is `dept_head`, and both come to one person.
  = For 3 requesters, e3101 (Head of Sales Department 1) answers both: e1101 (Manager of Section 1A), e1201 (Sales staff 1) and e1203 (Sales staff 3).
  = For 2 requesters, e7001 (Head of Information Systems) answers both: e7101 (Manager of IT Security) and e7201 (Manager of IT Operations).
  = `same approver approves once` does not do `decide` with the approval of `first`: `second` comes between them.
  = With `same approver approves once, at the last stage`, they answer once: `first` waits for the approval of `decide`, after the stages between them.
  = One person who checks twice is not two pairs of eyes: change the way to choose one of the stages, or make sure it is meant.
```

2027-04-01 の版（`examples/org/2027-04-01/`）では、営業一課がなくなって三人が営業一部に移り、法務部長の e9001 が辞めて長が空く。手で数えた三件（二人に減った法務の議決、辞めた法務部長を指す `counsel`、e3101 が二度当たること）は、どれも数えた場所に出た。違ったのは次の点である。

- 言い方：申請者を、数と、ID と名前で言う。W506 は二つのステップに答える人ごとにまとめるので、案の「e1101、e1201、e1203 について、first も decide も e3101」は「e3101 が、3 人の申請者について両方に答える」になった。
- e3101 の W506 の行：経費には `same approver approves once` を書いたので（5.12）、あいだにステップの無い出張と経費の行（33 行と 35 行）では、部長のステップが上長の承認で済み、W506 にならない。あいだに経理のステップがある購入の行（37 行）では済まないので、そこで言い、なぜ済まないかと、`, at the last stage` を足せば一度になることを注に添える。経費は、上長の確認が経理より前に要るので、足さない（5.12）。
- `counsel` のコード：契約の `counsel` のステップは `when empty escalate` なので、辞めた人からは上へ回せないことを、決め方の行（`counsel = person "e9001"`）で E503 として言う。止まるステップ（`when empty stay`）なら、ステップの行の E502 になる。
- 案が数えていなかったもの：2027 年の版の経費の W506 には、e3101 のほかに、e7001（情報システム部長が、課長二人の上長と部長を兼ねる）のまとまりもある。経理部長の e1001 が経理担当の上長と部長を兼ねる組は、経理のステップも e1001 一人になる（申請者がチームから外れる）ので、三つのステップが一度の承認で済み、言わない。2027 年の版では法務部の長も空くので、法務の人の申請の部長のステップは本部長の e0102 へ回る（W502。経費、購買、休暇）。
- 組織の形でほかへ回るステップ：`ok` の下で数える。2026 年の版の契約で、法務部長の e9001 自身の申請の `counsel` が申請者だけになって（自己承認を許さない）上の e0102 へ回ること。社長の e0001 の上長のステップが、経費では取締役会へ回り（`then board`）、アカウント、契約、購買では `when empty escalate, then skip` で飛ぶこと。購買の決裁のステップが、社長の申請では取締役会へ回ること。本部長と部長の申請で、部長のステップが上へ回ること。期限を過ぎて上へ回せないのも、組織の形である。社長が期限を過ぎると上にだれもおらず、購買では、決裁のステップを取締役会が期限を過ぎたときも、取締役会の上にだれもいない。社長の申請にも、どの例でも答える人のいるステップが残るので、W510 は出ない（休暇は、取締役会の人が出さない。5.13）。
- 数：英語の例で、`account.ringi` は警告が無く、`contract.ringi` は E503 が 1・E504 が 1・W506 が 1、`expense.ringi` は W502 が 2・W506 が 4、`leave.ringi` は W502 が 1、`purchase.ringi` は W502 が 1。アカウントと購買は `same approver approves once, at the last stage` を書いたので、上長とあとのステップを兼ねる人の W506 が出ない（5.12）。W502 はどれも、2027 年の版で長の空く法務部のものである。日本語の名前の版も同じ数である。`ok` の下の数は、2026 年の版で、ほかへ回るか飛ぶ申請者がアカウント 24 人のうち 4 人、契約 2 人、経費 14 人、休暇 19 人のうち 9 人、購買 24 人のうち 14 人、期限を過ぎて上へ回せない申請者がアカウント 6 人、契約 6 人、経費 1 人、休暇 2 人、購買 24 人（金額によっては社長が決裁するので全員）である。契約のエラーで、`ritsu ringi check examples` は 1 で終わる（`ritsu check examples` は 31 ファイルを読み、落ちるのは契約の二本と、`expense.req` の E301 の 21 件）。

契約の例は、今の 2026 年の版では警告だけで通り、予約した 2027 年の版だけで落ちる。切り替えの前に、予約した版で確かめることを見せるためである。`ritsu ringi check examples --at 2026-11-16` は 0 で終わり（W506 が 6。`ok` の下の数の行が 20）、`--at 2027-04-01` は 1 で終わる（E503 が 2、E504 が 2、W502 が 8、W506 が 4。数の行は、落ちる契約の二本を除いて 16。どれも日本語の名前の版を含む数）。

データが無いときは、組織に頼る判定（空くか、議決に届くか、同じ人が当たるか）を決められないと W500 で言い、通ったことにしない（R4）。組織に頼らない判定（表の穴、主張、期限）は、データが無くても決まる。

```text
warning[W500]: example.ringi:3:1: The organization has no data, so whether each stage has an approver is not decided
     3 | use organization org from "company.ringi"
  = The organization's file has no `data` line, and no `--org` is given.
  = Write a `data` line in the organization's file, or give the data's directory with `--org`: with the data, the check says which stages have nobody, which quorums cannot be met and who answers two stages.
ok example.ringi: a request, 1 field, 1 stage
  may never end: the stage `check` has no deadline (`due in …`), and waits until it is answered
```

### 8.7 切り替えの前後の進行中のチケット

承認者が一度決まったステップは、切り替えのあとも同じ承認者で待つ。次のステップへ進むときに、そのとき効いている版で承認者を決める。履歴には、ステップごとに、どの版で決めたかを残す。切り替えで今のステップの承認者が組織からいなくなったときは、`when empty` でなく、管理する人の引き継ぎ（`hand over`）で扱う。いなくなった人の答えは、`person` の種類で断る。期限を過ぎて上へ回すときは、回すときに効いている版で、回す先を決める。申請者がいなくなったチケットは、7.7 のとおりに扱う。ページ（11.4）は、切り替えで承認者が変わる申請者を、予約した版について先に見せる。

あとのステップの承認を待っているステップ（`same approver approves once, at the last stage`。5.12）は、承認者がまだ答えていないので、切り替えのあとの版でも確かめる。待つ先のステップが始まるときに、そのステップが待たせた人に一人では当たらなくなっていれば、待っていたステップを、そのときの版で決め直す。購買の例のシナリオ `the_organization_changes_while_the_manager_waits` では、法務部の e9002 の申請で、上長のステップ `first` が、部長の決裁 `decide` を待つ（どちらも法務部長の e9001）。技術の確認のあいだに 2027-04-01 の版に替わり、e9001 がいなくなる。`decide` が始まるときに `first` を決め直すと、どちらも長の空いた法務部から管理本部長の e0102 へ回るので、`first` は e0102 の `decide` の承認を待ち続け、e0102 が一度だけ答える。

```text
$ ritsu ringi run examples/purchase.ringi
…
2027-03-30 10:00  submit   by e9002  -> technical (e7201), due 2027-04-06 end of day; first waits for the approval of decide (e9001); hold budget.reserve 300_000 JPY
2027-04-01 11:00  approve  by e7201  -> decide (e0102), due 2027-04-08 end of day; first waits for the approval of decide (e0102; the stage it waited for does not come to e9001 alone); decide went up to e0102 (the head of legal is empty)
2027-04-01 15:00  approve  by e0102  -> approved; first done with the approval of decide (e0102); inform circulated to e5001; launch order; post budget.reserve
the book `budget` at the end: dept_budget(legal) 2_700_000 JPY, allocations -3_000_000 JPY, payees 300_000 JPY
  the hold `reserve` (T-1, attempt-1): posted
scenario `the_organization_changes_while_the_manager_waits`: as written (3 rows)
…
```

## 9. 検査

### 9.1 ringi だけで決まる検査

- 構文、名前、型、`use` の種類（E0xx）。
- フォーム：範囲と長さ、丸め、必須と表示、承認者だけのフィールドを読む順、マスタ（E1xx）。
- 経路：表の穴と重なり（下ろした規則で rulec が）、届かないステップ、議決の数、指名の順、主張、共有の経路の引数（E3xx）。
- チケットと期限：終わるか、いちばん長い日数、催促と期限の順（下ろした日付で koyomi が）、カレンダーのデータの範囲、仮押さえの確定と取消（E4xx）。
- 組織：データの形、版ごとの空くステップ、議決に届くか、同じ人、代理（E5xx。データが無ければ決められない）。
- 例とシナリオ（E6xx）、画面（E7xx。`ritsu gen` のとき）。秘密の値を入れるフィールド（W109）と、鍵の形の値（W901。ほかの言語と同じ検査）。

**決定（宣言の無い名前は一度）**：宣言の無い名前（フィールド、承認者の決め方、規則の出力、明細の列など、E004 のどれも。チケット、例、シナリオが書く、経路に無いステップの E308 も）は、ほかの言語と同じく、ファイルで最初に書いた所で一度だけ言い、同じ名前を書いたほかの行の番号を注に挙げる（根の DESIGN 8.3。`` `reviewer` is written on lines 30 and 43 too. ``）。名前の種類と宣言する所ごとに数えるので、同じ綴りでも、フィールドと承認者の決め方、二つの明細の列、マスタのフィールドとフォームのフィールドは別の名前で、それぞれに言う。購買の例の写しに誤りを一つずつ入れて測ると、宣言の綴りを誤ったフィールドを三か所が読むときに 3 件、宣言の綴りを誤ったステップをチケットとシナリオの 7 か所が書くときに 7 件言っていた（試験の材料の、宣言の無い承認者の決め方を二つのステップが書く形も 2 件）。ほかの 17 の誤り（無い型、無い単位、無い承認者の決め方、無い役職とチームの種類、読めない規則・組織・マスタ・カレンダーのファイル、明細の無い列、規則の無い出力、字句と構文）は、もとから 1 件だった。材料は `tests/chains/`、試験は `tests/chains.rs` である。組織のデータに無いチームの ID は、組織の版ごとに E503 を言う（版が二つなら 2 件）。

### 9.2 下ろした先の言語の検査

下ろしたファイルには、rulec、koyomi、kaname の検査が全部かかる。ringi はその診断を 6.5 のとおり自分のコードと言葉に直す。対応：

| 下ろした先の診断 | ringi のコード |
|---|---|
| rulec E101（`route.rule` の表に当てはまる行が無い） | E301 |
| rulec E105（`route.rule` の表の二つの行が当てはまる） | E302 |
| rulec E102（`route.rule` の表の、どの入力も当てはまらない行。範囲の外の行、ほかの行に覆われた行） | E303 |
| rulec W111（`route.rule` のステップの列挙に、どの行も与えない値） | W303 |
| rulec E104（丸めが無い） | E104 |
| rulec E150（`form.rule` の計算に書いた範囲に、計算した値が一つも入らない） | E103 |
| rulec E146（`possible` の主張が成り立たない） | E306 |
| rulec E107、E019（`examples` の行が成り立たない） | E601 |
| koyomi E301（主張 `remind < due` が成り立たない） | E403 |
| koyomi E203（営業日で数えた日がカレンダーのデータの外に出る。`terms.cal` と `form.cal`） | E404 |
| rulec E108（生成するコードが読む写し `values.rule` の計算が、int64 を越えうる。4.4） | E201（計算するフィールドの行に、届く値と狭める読むフィールドを添えて） |
| kaname が `ritsu gen` の置くコード（申請ごとのつなぎと、チケットの機械）について言うこと。診断は警告も、total でない関数も、そこでの言語をまたぐ呼び出しの検査の診断も | E201（申請の最初の行。機械のものは最初の申請に。ringi の不具合とし、kaname のコードと行だけを注に書く） |
| そのほかのエラー | E201（元の言語とコードと文を注に） |
| そのほかの警告 | W201（同じ） |

**決定（率のつなぎ方）**：申請ごとのつなぎは、率を kaname の決まりに合わせて書く。

- 刻みを書かない `rate` は、JSON が数える刻みで `rate[step 1%]` と書く。kaname は、刻みの無い率を引数とフィールドに取らない（kaname の E108）。
- 範囲を書かない所の率には、JSON が持てる値の全体、int64 の刻みを範囲に書く（`rate[step 0.25%]  range >=-2_305_843_009_213_693_952% <=2305843009213693951.75%`）。kaname は、範囲の無い率を 0% から 100% として読み、ほかの値を入口で断る。参照は、率もほかの数と同じく、フィールドの範囲だけで確かめる。書かなければ、範囲が 0% から 100% を越えるフィールドの値を入口で断り、範囲の外の値を参照と違う種類で断る。チケットの機械の総称の関数（`present` と `kept`）の引数も、率なら 0% から 100% と読まれるので、率には刻みごとの関数を使う。
- 刻みごとに四つの関数を書く。`rate_steps_<刻み>` は JSON が持つ刻みの整数で、フローの引数になる（率を率で割った値は kaname では率なので、型を書いた `let` で数にする）。`rate_text_<刻み>` は断りの文に書く値で、kaname の `text` は率を書かないので、参照と同じ書き方で書く（`31.25%`、`-0.5%`、`100%`）。int64 を小数の最後の桁で越える値は、どのフォームも取らない値で、刻みの数で書く（`922337203685477581 × 0.25%`）。参照は 5 桁以上の整数の率を `_` で区切るが（`12_345%`）、つなぎは区切らない。突き合わせは断りの文を比べない（12.4）。`rate_present_<刻み>` と `rate_kept_<刻み>` は、チケットの機械の `present` と `kept` の率の版である。
- 計算する率は、規則が刻みに丸めた値を、つなぎがもう一度同じ刻みに丸めて受ける。同じ値になる。kaname は、規則の答えの率を、丸めていない値として読むからである。

理由：これまで、率のフィールドを持つ申請は、`ritsu gen --target requests` が必ず E201 で止まった（kaname の E108、E109、E102。どれも ringi の不具合）。例にも機械の試験にも率が無く、試験に出ていなかった。`tests/machine/steps.ringi` は、0.25%、0.1%、2.5%、0.5% と、刻みを書かない率、0% を下回り 100% を越える率、空にできる率、条件で見せる率、計算する率、明細の中の率と、セントと銭の金額を持つ。機械の試験と、生成したコードの突き合わせ（`crates/ritsu/tests/gen_requests.rs`。fast の段でも、つなぎを kaname の検査にかける）に入る。

計算するフィールドに書いた範囲に値が一つも入らないこと（E103。4.1）は、二か所で見つかる。読む値の範囲から区間で分かるものは、下ろす前に ringi が言う。丸めと明細の行の数まで数えて分かるものは、下ろした `form.rule` で rulec が言い（E150）、ringi が同じ見出しと注の E103 に直して、rulec の数えた届く範囲を添える。例のセルがフォームの取らない値であることも、下ろす前に ringi がそのセルで言う（E601。5.3）。rulec の E102 は警告の W303 とは混ぜず、エラーの E303 にした。届かない行は表の誤りで、直さなければならないからである。

### 9.3 言語をまたぐ検査

確かめで見込んだ七つの境目は、次の所で確かめる。

| 境目 | 確かめる所 |
|---|---|
| フォームと表（選択の値と表の列、金額の範囲） | 下ろした規則で rulec。選択の値と列は一つの宣言から作るので食い違わない |
| 表と承認者（表の役のどれにも決め方がある。エージェントが決めるのは表が渡した行だけ） | ringi の E004、E203。エージェントの範囲は主張（`only when`）を ringi が rulec に尋ねる（E306） |
| 経路と組織（どの版、どの申請者でも承認者がいる） | ringi の E5xx（組織は ringi の意味） |
| 期限とカレンダー（催促が期限より前、いちばん長い日数） | 下ろした日付で koyomi（E403）、ringi の E401 |
| 帳簿とチケット（どう終わっても、押さえた予算は確定か取消になる） | ringi の E405、E406。期限と額は X19 と X4 |
| gate と経路（経路が、会社全体の決まりの禁じる人を承認者にしない） | X18 |
| 要件（規程の条と、表、経路、期限の対応） | yuen（X8）。ringi が `Items` と `References` に答える |

ritsu の検査の一覧（根の DESIGN 7.2）に三つ足した（`crates/ritsu-cross/src/ticket_gates.rs`、`ticket_holds.rs`、`ticket_flows.rs` と、X20 の反対の向きの `ticket_opened.rs`）。どれも、申請の種類と相手のファイルが別々の人に書かれ、それぞれの検査は一つだけで通る所を見る。

| | 検査 | 示すこと | 読むもの（口） | 診断 |
|---|---|---|---|---|
| X18 | gate と経路 | 申請の種類と、それが `use gate` で読む gate の組ごとに、境目が三つ。**操作**：サーバーがその申請の種類について gate に尋ねる操作のどれにも、gate が action を宣言していて、その action が自分の入力（`input`）を受け取らない。尋ねる操作は、チケットが持つ物（`submit`、`approve`、`reject`、`add_approver`、`hand_over`、`comment`、チケットに載っていない人に見せるかを尋ねる `see`（12.4 の「見られる人」）と、書いていれば `withdraw`、`send_back`、`pause`、`retract`）と、サーバーの管理の操作の四つ（申請の種類の版の始まりを決める `change_request_type` と、その記録を読む `read_audit`、組織のデータを取り込む `import_organization` と、その記録を読む `read_organization_audit`。14.3）である。サーバーが gate に渡すのは人と resource と時刻だけなので、入力を受け取る action は、どの問いでも拒まれる（種類 `input` の断り）。宣言の無い管理の操作は、だれもできない。`comment` は答えではないので、承認者に許すかは見ない。`see` が無ければ、注で、見られるのが申請者の側と承認者の側の人だけになり、チケットに載っていない管理する人が操作できないことを言う。**承認者**：経路が決めた承認者に、承認者のすること（`approve`、`reject` と、あれば `send_back`、`pause`、`retract`）を gate が許す。**自己承認**：許さない経路では、申請者が自分の申請を決める操作（`approve`、`reject`、`send_back`）を gate がどの組み合わせでも許さず、許す経路では gate も許す | `Requests::ticket`、`Gates::facts`、`Gates::allowed_with`（属性の値を決めて尋ねる） | ritsu の E911、W911 |
| X19 | 帳簿とチケット | 仮押さえ（`on submitted hold`）ごとに境目が一つ：仮押さえが開いているいちばん長い時間が、帳簿の有効期限に届かない。申請者の組織の中の場所を読む引数（`dept: requester.department`）ごとにも一つ：出せる人のだれにも、その値がある。額が帳簿の受け取れる範囲に入るかは、下ろした `.kn` が組み立てる振替に X4 がかかる（12.2） | `Requests::ticket`、`Requests::usable`、`Books::facts`、`Dates::calendar` | ritsu の E209、W209 |
| X20 | flow とチケット | flow に渡す値ごとに境目が一つ：`launch` が書いた値と、エージェントと要約の flow が受け取るチケットの ID（`request_id`）とフォームのフィールドが、flow の入力の型と範囲に入る。エージェントの flow の答えに一つ：出力が `verdict`（`ok` と `doubt` を持つ列挙）と `reason`（文）である。要約の flow の答えに一つ：出力が一つ以上ある。反対の向きに、flow がサーバーの API で申請の操作を呼ぶ所ごとに一つ：呼ぶ操作が、今の申請から ringi が書く操作である（下の「flow がチケットを開く側」） | `Requests::ticket`、`Flows::declared`。反対の向きは `Flows::operation_calls`、`Requests::openapi` | ritsu の E210、W210 |

エラー（E911、E209、E210）は、今の言語をまたぐ検査と同じに、`ritsu check` を 1 で終わらせ、`ritsu gen` を何も書かせずに止める。警告（W911、W209、W210）は止めない。診断の文は、申請をファイルの名前、絶対パス、ディレクトリのどれで渡しても同じで、相手のファイルがプロジェクトの根の外にあっても同じに確かめる（gate は、根と gate の両方を含むいちばん近いディレクトリから sekisho に尋ねる）。flow がチケットを開く側（15.4）は、X20 の反対の向きとして下に書く。

**X18 の尋ね方**。ringi は、チケットの操作のたびに、ステップの属性を gate に渡す（`resource.requester`、`resource.approvers`、`resource.self_approval` と、例の gate が読む申請者の側の人 `resource.requesters` など。12.4）。検査は、経路が決める三つ（`requester`、`approvers`、`self_approval`）を経路が与える値に決め、ほかの属性と役割は全部の組み合わせを歩かせて尋ねる（`Gates::allowed_with`）。尋ね方は、組織のデータがあっても無くても同じである。gate は会社全体の決まりで、組織の版より長く使うからである。

| 境目 | 決める値 | 成り立つ | E911 |
|---|---|---|---|
| 操作 | （尋ねない。`Gates::facts` の action の一覧と、それぞれの入力） | どの操作にも action があり、自分の入力を受け取らない | action の無い操作がある（だれにも許されない）。自分の入力を受け取る action がある（サーバーには渡す値が無く、どのチケットでも拒まれる） |
| 承認者 | principal が `resource.approvers` に入っていて、`resource.requester` でない | 許す組み合わせがある（停止中の人を拒むのはよい） | どの組み合わせでも許さない |
| 自己承認（許さない経路） | `resource.requester` が principal、`resource.self_approval` が false | どの組み合わせでも許さない | 許す組み合わせがある（その一つを添える） |
| 自己承認（許す経路） | principal が `resource.requester` で `resource.approvers` にも入り、`resource.self_approval` が true | 許す組み合わせがある | どの組み合わせでも許さない |

sekisho が決められないとき（gate が三つの属性を、ringi の渡す型と違う型で宣言している。答えが、ほかの言語の決めない条件に依る）は W911 で、ringi が渡す三つの属性と型を注に書く。gate が自分の検査を通らなければ、言うのは sekisho である。

```text
$ ritsu check .      （台帳の E911 の再現の `purchase.ringi` と `company.gate`。`ritsu explain E911` が見せる。診断だけを抜いた）
error[ritsu E911]: purchase.ringi:15: `purchase` does not let a requester decide their own request, and the gate company.gate allows a requester `approve` and `reject` on it
    15 |   self approval  not allowed
  = Line 15 of the request says `self approval not allowed`.
  = Asked of the gate: `resource.requester` is the principal and `resource.self_approval` is false.
  = `approve` is allowed, for example: Staff, Stage, principal is in resource.approvers.
  = Write a forbid in company.gate (`action approve`, `when resource.requester is principal`, `unless resource.self_approval`), or allow it in the request (`self approval allowed`).
```

**X19 の数え方**。仮押さえは、申請を出したときに作り、その `round` が終わるときに確定か取消になる。開いているいちばん長い営業日は、ringi の検査が数えた一つの `round`（7.6）と保留の合計である。前のステップへ差し戻す申請の種類（`send back to any earlier stage`）は、差し戻しても `round` が変わらず、仮押さえが開いたままなので、`round` を差し戻しの上限の回数だけ歩き直す分を足す。この営業日を、申請を出せるどの日（9.6）からもカレンダーに置き、出した日の 00:00 から最後の日の終わりまでの、いちばん長い暦の日数を取る。休みの表の無いカレンダーは、グレゴリオ暦の一巡り（146,097 日）で同じ並びをくり返すので、そこまでを見る。

- その日数が有効期限より短ければ成り立つ。有効期限の無い仮押さえも成り立つ。
- 届けば E209 で、いちばん長くなる出した日と、その日数を言う。ちょうど届く日数も、届いたものとする（帳簿は、時計が期限に届いた仮押さえを切れたものとする）。営業日は検査が数えた上限で、ringi が自分の主張（`every ticket ends within`）を確かめるのと同じ数である（7.6、20.2）。
- 決められなければ W209：`round` の中に終わりの無い待ち（期限の無いステップ、期限を過ぎても待つステップ）がある。営業日を二つ以上のカレンダーで数えていて、どれかが休む日を休みとして数えた並び（どのチケットも越えない上界）が有効期限に届く。カレンダーのデータが `round` を覆わない。
- 引数の境目：仮押さえの行に `when requester.<種類> is not none` があれば、その種類のチームが上にある申請者にだけ仮押さえを作るので、成り立つ。無ければ、出せる人のうち値の無い人の `submit` を機械が断るので（7.2 の `book`）、W209 で、版ごとに、出せる何人のうち何人かと、3 人までの名前を言う（4 人以上なら、3 人のあとに数）。組織のデータが無ければ、決められないので W209。

```text
$ ritsu check .      （台帳の E209 の再現の `purchase.ringi`、`budget.book`、`office.cal`。診断だけを抜いた）
error[ritsu E209]: purchase.ringi:22:22: The hold `budget.reserve` can expire before a ticket of `purchase` posts or voids it
    22 |   on submitted  hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: dept, amount: amount)
  = The hold is made when a request is submitted and stays open until its round ends: 10 business days at the most, as ringi's check counts them (a round of 10).
  = A request submitted on 2026-04-01 can stay open through 2026-04-15, 10 business days later in the calendar `office`: 15 days.
  = The hold expires 7 days after it is made (`reserve` of "budget.book").
  = The book refuses a post or a void of an expired hold with `expired`: the ticket would be approved with nothing held for it.
  = Make the hold last longer (the transfer's `pending expires after`), or give the stages shorter deadlines.
```

部の上に人のいる小さい組織（9 人と 7 人の二つの版）で、行に `when` を書かないと、次のようになる（`crates/ritsu/tests/cross.rs` の `x19-many-requesters-W209` の golden から）。

```text
warning[ritsu W209]: purchase.ringi:22:88: Some requesters of `purchase` have no value for `dept`, an argument of the hold `budget.reserve`
    22 |   on submitted  hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: requester.department, amount: amount)
  = In the organization from 2026-04-01, 5 of the 9 people who may submit the request have no `requester.department`: e0001 (President (fictional)), e0002 (Director 1), e0003 (Director 2) and 2 more.
  = In the organization from 2027-04-01, 3 of the 7 people who may submit the request have no `requester.department`: e0001 (President (fictional)), e0002 (Director 1) and e0003 (Director 2).
  = The hold cannot be made without it: their `submit` is refused.
  = Who they are: `ritsu ringi simulate purchase.ringi --every-requester`.
  = If they do not submit the request, leave them out (`submitted by … except …`). If they do, give the hold a value every requester has, or write `when requester.department is not none` on the line, so that their requests hold nothing.
```

例の数（検査が数えた営業日を、`examples/shared/office.cal` に置いた）：購買は `send back to any earlier stage, at most 3 times` なので、一つの `round` の 54 営業日を四度歩きうる。仮押さえは 216 営業日開いていることがあり、2030-03-01 に出した申請では 2031-01-06 まで、312 日になる。経費は 21 営業日で、2030-11-29 に出した申請の 39 日がいちばん長い。休暇は 10 営業日で 22 日。予算の帳簿の有効期限は 365 日、休暇の帳簿は 120 日なので、三つとも成り立つ（予算の帳簿は、はじめ 90 日と書いていて、購買で E209 になった。前のステップへの差し戻しを数えていなかったためで、365 日に直した）。経費と購買の仮押さえは `dept: requester.department` を読むが、行に `when requester.department is not none` を書いたので、部の上にいる人（社長、本部長、取締役）の申請は部の予算を押さえずに進む。

**X20 の読み方**。`launch` は、行に書いた値を渡す。エージェントと要約の flow には、チケットの ID（`request_id`）とフォームのフィールドの全部を渡し、flow は、そのうち自分が宣言した入力を受け取る。渡す値の範囲は、flow に渡る所で狭まる分を読む。エージェントは、経路の表がそのステップに渡す行の範囲で読む。`on` の行とステップの `when` は、フォームのフィールドを定数と比べる条件を `and` でつないだ形（`<フィールド> is not none` も）を追う。`requester.<種類>` を渡す行の `when requester.<種類> is not none` は、値があるものとして読む。

- 渡しうる値が、どれも入力の受け取る値なら成り立つ。
- 受け取らない値があれば E210 で、その値を言う：入力の範囲の外の数か日付、入力の列挙に無い選択の値、別の型、`T?` でない入力に渡す値が無いこと。`launch` が渡さない入力、flow に入力の無い値、エージェントか要約の flow の入力で `request_id` でもフォームのフィールドでもないものも E210。型や単位が違うときは、条件に関係なく E210。
- 決められなければ W210：ringi が追わない値、値ごとに比べない型（明細とリスト）、組織のデータが無いときの `requester.<種類>`、ringi が追わない条件（`or`、`not`、渡す値と一緒に計算するフィールドを読む条件）の所で、合わないように見える値。注に、ringi が追う条件の形を書く。

```text
$ ritsu check .      （台帳の E210 の再現の `purchase.ringi` と `order.flow`。診断だけを抜いた）
error[ritsu E210]: purchase.ringi:15: `purchase` can give the flow `order` 1_000_000JPY as its input `amount`, which takes money[JPY] >=1JPY <=500_000JPY
    15 |   on approved  launch order(request_id: ticket.id, amount: amount)
  = `amount` is money[JPY] >=1JPY <=1_000_000JPY.
  = The flow takes 500_000JPY or less.
  = Hold the value to what the flow takes, or widen the input `amount` of "order.flow" (line 6).
```

行に `when amount <= 500_000JPY` を書けば、渡す値がその範囲に絞られて成り立つ。flow の入力を広げても成り立つ。どちらにするかは、人が決める。

**X20 の、flow がチケットを開く側**。flow は、`ritsu gen --target requests` が書いた `openapi.json` を `use openapi` で読み、申請を出す操作（`submit_<名前>`、`resubmit_<名前>`、`edit_<名前>`）を `http` のタスクで呼ぶ（15.4）。このタスクには `key` を書く。どの POST も冪等キーをヘッダ `Idempotency-Key` で求め、dandori は、`key` のあるタスクにそのキーを渡す（`key` の無いタスクを dandori の E016 で止める形は、別の直しで入る）。フォームを受け取るこの三つの操作には、`x-ritsu-request`（申請のファイル、名前、版）がある（12.4）。dandori は、タスクの引数を操作の本文のスキーマと比べる（dandori の E016：型、範囲、列挙の値、要るフィールド）。この比べは文書が相手なので、文書が今の申請から書いたものなら、フォームと比べたのと同じになる。そこで X20 は、flow が呼ぶ操作が今の申請のものかを確かめる。flow が `x-ritsu-request` のある操作を呼ぶ所ごとに、境目が一つある。先に決めた四つは次のとおり。

- (a) 申請のファイルを引く根。`x-ritsu-request` の `file` は、文書を書いた `ritsu gen` の、プロジェクトの根からのパスである。`ritsu check` は、自分のプロジェクトの根から引く（根は、どちらも `.git` か `ritsu.mod` のあるいちばん近いディレクトリなので、同じプロジェクトなら同じ）。根の外を指すパス（`..` を含む、`/` で始まる）、無いファイル、`.ringi` でないファイルなら、比べる相手が無いので W210 で、文書の名指す申請を言う。flow は文書とだけ比べる。申請のファイルが自分の検査を通らなければ、言うのは ringi で、X20 は黙る。ringi がその申請の API を書けない（組織を読まない申請。`ritsu gen` もその API を書かない）ときも W210。
- (b)「古い」の決め方。呼ぶ操作を、今の申請から ringi が書く操作（`Requests::openapi`。`--target requests` と同じ書き方で、ベクタは作らない）と比べる。比べるのは、操作の場所（メソッドとパス）、本文（`requestBody`。`$ref` をたどって、指す先の中身で比べる）、`x-ritsu-request` で、どれかが違えば E210 である。フォームを変えて `gen` を忘れた形は本文が違い、版だけを上げた形は `x-ritsu-request` の版が違うので、どちらも捕まる。申請の名前を変えた（操作の名前が変わった）ときも、今の申請に無い操作を呼ぶので E210 で、今の申請の操作を挙げる。違う所は三つまで、場所（本文のスキーマは `body.form.amount.maximum` のように本文から、`properties` を除いて）と、文書の値と今の値で言い、四つ目からは数だけを言う。ほかの申請の操作や、操作の引数（`Idempotency-Key`）と答えはどの申請の種類にも同じなので、比べない（ほかの申請と一緒に書いた文書と、一つだけで書いた文書の操作は、`$ref` をたどると同じ。例の休暇で確かめた）。
- (c) 一つの誤りは一度。文書が今の申請のものなら、flow が送る値の型と範囲は dandori の E016 が言い、flow は自分の検査を通らないので、X20 は何も言わない（境目にも数えない）。文書が古ければ、dandori は古い文書と比べて通し、X20 が E210 を呼ぶ所に一度だけ言う（フィールドごとには言わない）。古い文書とも合わなければ dandori の E016 だけが出て、それを直したあとに X20 が古いことを言う。比べないものもある。dandori の `string` には長さが無く、OpenAPI は日付の範囲を `description` にしか書かず、表示の条件で隠れるフィールドはスキーマに書けない。この三つは、サーバーが申請を断り（409）、flow のタスクのエラーになる。
- (d) 口。dandori の `Flows::operation_calls`（sekisho の X16 が使う、flow が呼ぶ操作と渡す値）をそのまま使い、ringi の `Requests` に `openapi` を足した（9.4）。検査は `crates/ritsu-cross/src/ticket_opened.rs` で、X20 の境目（`ticket_flow`）として数える。境目の場所は flow の呼ぶ行、相手は申請のファイルである。

`ritsu gen` は、ほかの E210 と同じに、古い文書で止まる。ただし `--target requests` を書く回は、その回が書く `<out>/requests/openapi.json` を、検査の前に今の申請から作り、ディスクの上の文書の代わりにその場所に置いて（ファイルを重ねて読ませる。ディスクに書くのは、検査が通ったあと）、flow と gate をその文書と比べる。文書がまだ無い初めの一回も通り、ディスクの文書が古くても、古いことを直す回が古いことで止まらない。flow がその回の書く文書と合わなければ dandori の E016 が、gate の守る操作がその文書に無ければ sekisho の E202 が言い、何も書かない。検査が読む文書と、その回が書く文書は同じ物である（`crates/ritsu/tests/gen_requests.rs` の `a_flow_that_reads_the_api_is_held_to_the_one_the_run_writes`）。文書は申請だけから作り（フォームとチケットの操作で、flow と gate のことは持たない）、作るときは申請の読む flow と gate を読まずに通ったものとする。そのため、flow が自分の申請の API を読んでも、ほかの申請の API を読んでも、申請の読む gate が API の操作を守っていても（`use openapi` と `guards`）、その gate が API を読む flow をワークフローとして名指していても、一度で作れる（同じ試験）。文書を作るのは flow と gate を読まない ringi で、ほかの言語（規則、カレンダー、帳簿、kaname のモジュール）は検査と同じ物を読む。文書を置くかどうかは、その回の届く flow と gate を書いたとおりに読んで決め（検査はしない。検査すれば、文書を置く前の答えが残る）、どれかの `use openapi` がその回の書く文書を指していれば置く（パスは、書いたファイルのディレクトリから、システムが開くとおりに比べる）。置く回でも、どのファイルも一度だけ検査する。文書を作るのはその回の検査をする ringi で、flow と gate を通ったものとして検査した申請とその読むデータの答えを覚えておく。flow と gate を読んで検査するとき、申請の読む flow と gate がどれも通れば、覚えた答えを使う（同じ検査になるからである）。どれかが通らなければ、その申請を flow と gate を読んでもう一度検査する。その回は検査が通らないので、何も書かない。ほかの API だけを読む回は文書を置かず、どのファイルも一度である（`crates/ritsu/tests/once.rs`）。

```text
$ ritsu check .      （crates/ritsu/tests/cross.rs の x20-api-E210 の golden から。診断だけを抜いた）
error[ritsu E210]: restock.flow:27: The flow calls `submit_purchase` of "out/requests/openapi.json", which is not written from `purchase.ringi` as it is now
    27 |   let moved = submit_purchase(form: purchase)
  = The document says the operation is written from the request `purchase` (version 1).
  = `body.form.amount.maximum` is 1_000_000 in the document, and 300_000 now.
  = dandori holds what the flow sends to this document, not to what the request takes now.
  = Write "out/requests/openapi.json" again (`ritsu gen --target requests`), then check the flow again.
```

文書を書いたあとに、フォームの金額を 1,000,000 円までから 300,000 円までに狭めた形である。flow は 500,000 円まで送るので、古い文書とは合い、dandori は通す。`ritsu gen --target requests` で書き直そうとすると、その回が作る文書と dandori が比べて E016 を言い（「in `amount`, it is `>=1 <=500000`, and `>=1 <=300000` is taken」）、何も書かない。flow の送る範囲を狭めるか、フォームの範囲を戻すまで、文書は書けない。

`ritsu check crates/ringi/examples` は、25 ファイルで 76 の境目を確かめ、どれも成り立つ（X18 が 24、X19 が 6、X20 が 46。経費 18、購買 8、アカウント 8、休暇 4 が、英語と日本語の版のそれぞれにある）。

下ろした `.kn` が規則、日付、振替を呼ぶ所には、今の X2、X3、X4、X6 がそのままかかる（kaname の `Programs` を通る）。フォームの範囲から規則の入力の範囲への食い違いは、ここで出る。

### 9.4 口に足すもの

足す口は、どれも既定の答え（決められない）を持ち、答えない言語のふるまいを変えない。

| 口 | 足すもの | 使う所 | 段階 |
|---|---|---|---|
| `Requests`（新しい口。答えるのは ringi） | 下ろしたファイル（`lowered`：パス、言語、中身、行ごとの元の行と何か）、フォームの事実（`form`：フィールドと型と範囲）、使う日の範囲（`usable`：始まりと終わりの日と、それを決めているデータ。9.6）、チケットの事実（`ticket`：自己承認の設定と行、読む gate、サーバーが gate に尋ねる操作（チケットの操作と管理の操作）、終わり方、いちばん長い営業日（`round`、差し戻しの回数、再申請、保留、合計、仮押さえが開いている営業日、終わりの無い待ち、数えるカレンダー）、仮押さえと申請者を読む引数、flow ごとの渡す値。`ritsu ringi api` の `ticket` と同じ中身。16.2） | `ritsu-project` の `Joined::requests()`（生成）、ritsu-cross（X18〜X20）、ページ | C1（下ろしたファイルとフォーム）、C2（使う日の範囲）、C4（チケットの事実） |
| `References`（ringi が答える） | `use` の行と読むファイル | `ritsu check` の索引（待っている行） | C1 |
| `Items`（ringi が答える） | 中の物の種類と名前、行、定義（9.7） | yuen と sakai が ringi の物を指す（`ringi "<ファイル>" <種類> <名前>`）、`ritsu ringi api` | C4 |
| `Rules::findings`、`Dates::findings` | 診断ごとのコード、行、例の入力、行、直し方（`--format json` と同じ中身）。形は `Result<Found<Vec<Finding>>, Vec<Said>>` | ringi が診断を直す（6.5） | C1 |
| `Programs::findings` | 同じ（kaname） | 下ろした `.kn` の診断 | D |
| `Rules::diff` | 二つの規則の、答えの違う入力の区間と行（`rulec diff` と同じ中身） | 変更の前後のページ（11.2） | F |
| `Flows::declared`（dandori が答える） | flow の名前と、入力と出力の名前、型、範囲（`dandori api` と同じ一覧から作る） | X20 | C4 |
| `Requests::openapi`（ringi が答える） | 申請のファイルから、`--target requests` が今書く `openapi.json`（ベクタは作らない）。申請の種類でないファイルと、API を書けない申請（組織を読まない）には「決められない」 | X20 の、flow がチケットを開く側 | F1 |
| `Flows::operation_calls`（dandori が答える。sekisho の X16 が足した口） | flow が契約の操作を呼ぶ所ごとの、操作（`openapi "<文書>" operation <operationId>`）、行、渡す値 | X20 の、flow がチケットを開く側 | F1（使う所を足しただけ） |
| `Gates::allowed_with`（sekisho が答える） | 属性の一部を値に決めたときの「いつも・組み合わせによって・決して許す」。決める値は、真偽、列挙の値、エンティティを指す属性が principal 自身かどうか、principal がその中に入っているかどうかの四つ。答えるのは `.gate` だけで、手で書いた Cedar には「決められない」 | X18 | C4 |

### 9.5 診断のコードの一覧

コードは ringi のもので、ほかの言語と同じ番号でもよい（根の DESIGN 0.3）。台帳は `src/codes.rs` の 62 個で、`docs/codes.md` と `docs/codes.ja.md` は、そこから `ringi explain --all --format markdown` が書く。62 個の全部を出す。E001〜E009、E101〜E107、W108、W109、E201、W201、E202、E203、E301、E302、E303、W303、E304〜E309、E401、W402、E403〜E409、W500、E501〜E505、W502、W506、W507、E508、E509、W510、E601〜E604、E703、E704、W901 の 60 個には、英語の名前と日本語の名前の例と、試験の変異（`tests/mutants/`）がある。E703 と E704 の例と変異は、`ritsu gen --target requests` が画面を作る前にかけるのと同じ検査で回す。E701 と W702 は、ringi が書くテンプレートからは出ず、ringi の不具合のときだけ出るので、例が無い。`ringi explain` は、例が無いことと、そのわけを言う。

E009 は、型の食い違いを言うコードとして足した。E001（読めない行）や E004（宣言の無い名前）に混ぜると、直し方が違うのに同じコードになるからである。

| コード | 見出し（英語） | 見出し（日本語） | 出る例 |
|---|---|---|---|
| E001 | A line that does not read | 読めない行です | `every stage` の下の `due in three days`。`step` と書いた行。`when empty escalate, then escalate`。`comment required  for reject, send back, reject`。マスタから選ぶフィールドの下の行 |
| E002 | A name that cannot be used | 使えない名前です | フィールドの名前 `ticket`、`match`、`_`。申請の種類の名前 `_expense`、`expense_`、`an__expense`。承認者の決め方の名前 `stay`。Go で同じ名前になる二つのフィールド `cost_code` と `costCode`。フィールド `approver` のある申請の、決め方を書く表の列 `approver`。`a and b` を書いた経路の、決め方 `a_and_b` |
| E003 | A name declared twice | 同じ名前が二度宣言されています | `who` に `manager` が二つ。規則の列挙から選ぶフィールドの下に、同じ値の題が二つ |
| E004 | A name that is not declared | 宣言の無い名前です | 経路の表のセル `finance` が `who` に無い。`when empty escalate, then board` の `board` が `who` に無い。`choice from authority.kind` の列の `bogus` が規則の列挙に無い。その下の題の行の `ceo` が列挙に無い |
| E005 | A `use` that cannot be read | 読めない `use` です | 無いファイル。`use calendar` に `.book`。ディスクの根から書いたパス（`"/srv/shared/office.cal"`） |
| E006 | A block this kind of file does not take | この種類のファイルには書けないブロックです | 組織のファイルに `route`。`ticket` に `submitted by` を二度 |
| E007 | A label for nothing | 何も指していない文言です | `labels ja` の `stage approve` |
| E008 | A file that needs another language, given to ringi's own binary | ほかの言語が要るファイルです。`ritsu ringi` で確かめてください | クレートのバイナリに、経路の表か `use calendar` のある申請のファイル |
| E009 | A value that does not fit where it is written | 型の合わない値です | `required when amount > 30_000`（円の金額と単位の無い数）。`count` を数に、`open in` を日付でないフィールドに書く |
| E101 | A number, an amount or a date with no range | 範囲の書かれていない数、金額、日付です | `days : number "Days"` |
| E102 | A text with no limit on its length | 長さの上限の書かれていない文です | `purpose : long text "Purpose"` |
| E103 | A range a computed field never comes to | 計算した値が一度も入らない範囲です | 単価 1,000 円まで、数量 10 までの `total = price * quantity` に `range >=20_000JPY`。33 円までの金額の `fee = amount * 3%  round down(1JPY)` に `range >0JPY`（切り捨てて 0 円にしかならない） |
| E104 | A computed amount that does not say how it rounds | 丸め方の書かれていない計算です | `fee = amount * 3%` |
| E105 | A required field that may be hidden | 隠れていることのある必須のフィールドです | `required when amount > 30_000JPY` と `shown when category is travel` |
| E106 | A condition that reads a field before it is filled | 入る前のフィールドを読む条件です | 申請のときの条件が、承認者だけのフィールドを読む |
| E107 | A choice from a master with no row on some day | 選べる行の無い日があるマスタです | 勘定科目の行が 2026-06-30 で終わり、次の行が 2026-08-01 から。`--through` の日より前に、どの行も終わる |
| W108 | A field approvers may edit that the route reads | 承認者が直すと経路が変わるフィールドです | `approvers edit amount` |
| W109 | A field that asks for a secret | 秘密の値を入れるフィールドです | `password : text`。入れた値はチケットの履歴に残り、承認者に見える |
| E201 | Something written inside does not pass the language it is lowered to | 中に書いた物が、下ろした先の言語の検査を通りません | 9.2 の表に無いエラー（合計が int64 を越える rulec の E108 など。元のコードと文を注に）。下ろす所にあるファイル |
| W201 | The language something written inside is lowered to warns of it | 中に書いた物に、下ろした先の言語が警告を出します | 9.2 の表に無い警告（選択の値 `impl` が Rust の予約語、rulec の W121）。rulec が決められない主張と E105 の問い（4.4） |
| E202 | A file read with `use` does not pass its check | `use` で読んだファイルが検査を通りません | 共有の経路、組織、規則、カレンダーのファイルの誤り |
| E203 | A value of a rule's output that names no approver | 規則の出力の値に、承認者の決め方がありません | `authority.decider` の値 `cfo` が `who` に無い |
| E301 | No row of the route takes some requests | 経路の表に当てはまらない申請があります | 10 万円を超える行が無い |
| E302 | Two rows of the route take one request | 経路の表の二つの行に当てはまる申請があります | `<=100_000JPY` と `>=100_000JPY` |
| E303 | A row of the route no request reaches | どの申請も当てはまらない経路の表の行です | 金額の範囲が 100 万円までなのに `>1_000_000JPY` の行 |
| W303 | An approver of a stage no request reaches | どの申請も届かないステップの承認者です | 金額の範囲が 10 万円までなのに `when amount > 200_000JPY` のステップ |
| E304 | A quorum larger than the approvers it counts | 議決に要る数が、承認者の数を超えます | `at least 2 of manager` |
| E305 | A choice the stage before cannot make | 前のステップが指名できない指名です | 最初のステップの `chosen by previous approver` |
| E306 | A claim about the route that does not hold | 経路の主張が成り立ちません | `decide is travel_agent only when amount <= 30_000JPY` に、3 万 1 円の行 |
| E307 | A shared route given what it does not take | 共有の経路に、受け取らない引数を渡しています | `route standard(days: 5)` |
| E308 | A stage the route does not have | 経路に無いステップです | `withdraw until counsel` で、経路に `counsel` が無い |
| E309 | A condition the route or the form cannot read | 経路かフォームが読めない条件です | `when requester.department is none`、文のフィールドの `when place is "Osaka"`、`when amount > limit`（4.5） |
| E401 | A ticket that may not end within what the claim says | 主張の日数のうちに終わらないことがあるチケットです | 経費で `every ticket ends within 40 business days`（数えた 93 営業日と、その数え方を添える）。主張があるのに、`when late stay` のステップ、期限の無いステップ、`resubmit within` の無い申請者の番がある |
| W402 | A stage that may wait for ever | いつまでも待つことのあるステップです | `when late stay`、`when late escalate, then stay`（主張が無いとき） |
| E403 | A reminder that does not come before its deadline | 期限より前に来ない催促があります | `remind after 3 business days` と `due in 3 business days at 09:00`。事務所の 2 営業日の期限と、工場の 2 営業日の催促（別のカレンダー。5.10） |
| E404 | Days of use the calendar's data does not cover | 使う日の中に、カレンダーのデータが無い日があります | 祝日の表が 2026 年からで、`back_on = first_day + days business days in office` の `first_day` が 2025 年 10 月から。`--through` の日より前に、表が終わる |
| E405 | A hold an ending may leave open, or an edit may leave unlike the request | 確定も取消もされずに残るか、申請の値と食い違うことのある仮押さえがあります | `on expired` に `void` が無い。`when` の付いた `void` だけで、仮押さえには `when` が無い。`on approved hold`。`approvers edit amount` と、`amount` を引数か `when` で読む `on submitted hold` |
| E406 | A post or a void with no hold before it | 先に仮押さえの無い確定か取消です | `on approved post` だけで `on submitted hold` が無い |
| E407 | An action on an ending this ticket never has | このチケットには起きない終わり方の処理です | `withdraw` も `send back` も無いのに `on withdrawn`。期限で切れるステップも `resubmit within` も無いのに `on expired` |
| E408 | A hold keyed by nothing that tells one try from another | 試みを見分けないキーの仮押さえです | 振替のキーが申請と `round` だけで、`ticket.attempt` を受け取らない（12.4 の「帳簿の仮押さえのキー」）。chobo が帳簿を読めるときに確かめる |
| E409 | A reason asked with an operation this ticket never has | このチケットには無い操作に理由を求めています | `send back` の行が無いのに `comment required  for reject, send back`。人の答えるステップが無いのに `reject` |
| W500 | No organization data, so missing approvers are not decided | 組織のデータが無いので、承認者が空くかを決められません | 組織のファイルに `data` の行が無い。`--at` の日に効いている版が無い |
| E501 | Organization data that does not read | 読めない組織のデータです | `parent` の輪、宣言していないチームの種類、主の所属の無い人 |
| E502 | A stage with no approver in a version of the organization | 組織のある版で、承認者のいないステップがあります | 社長の `manager of requester`（`when empty stay`、または `then skip` も `then <決め方>` も無い `when empty escalate`）。`then auditor` の `auditor` が辞めた人 |
| W502 | A stage that goes elsewhere or is skipped for lack of an approver | 承認者がいないので、ほかへ回るか飛ばすステップがあります | 財務部の長が空いていて、`head of requester.department` のステップが `when empty escalate` で社長へ回る。2027 年の版で長の空く法務部（8.6）。組織の形で回るもの（社長の上長のステップの `then skip`、部長自身の申請の部長のステップ）は、`ok` の下で数える |
| E503 | A stage that goes up names someone a version does not have | 組織のある版にいない人かチームを、上へ回すステップが指しています | `when empty escalate` のステップの `person "e09"` で、e09 が辞めた。8.6 の counsel |
| E504 | A quorum some requesters cannot reach | 議決に届かない申請者がいます | 二人のチームに `at least 2 of sales`。8.6 の second |
| E505 | A proxy or a master row for someone not in the organization | 組織にいない人の代理か、マスタの行です | `proxies.csv` の辞めた人。マスタの `owner` の辞めた人 |
| W506 | One person at two stages of one ticket | 一つのチケットの二つのステップに同じ人が当たります | 一人だけの財務のチームと、別の決め方のステップに、同じ財務部長が当たる。8.6 の e3101 |
| W507 | A late stage with nowhere to go up to | 期限を過ぎても、上へ回す先のないステップがあります | 社長の席が空いた版で、営業部長が決裁するステップの `when late escalate, then expire`。社長が決裁するステップは組織の形なので、`ok` の下で数える |
| E508 | `submitted by` names a team or a person a version does not have | 組織のある版にいないチームか人を、`submitted by` が指しています | 2027 年の版でなくなった課の `everyone in sales1_a`。辞めた人の `person "e09"` |
| E509 | No one may submit the request in a version of the organization | 組織のある版で、この申請をだれも出せません | 部長しかいない財務部の `team finance except position dept_head` |
| W510 | Requesters for whom no stage has anyone to answer it | どのステップにも答える人がいない申請者がいます | 上長のステップ一つだけの申請に `when empty escalate, then skip` と書き、社長の申請がだれにも見られない |
| E601 | An example of the route that does not hold | 経路の例が成り立ちません | `examples` の行の答えが表と違う。例のセルが、フォームの取らない値（`range >=1 <=10` の列の `99`、マスタに無いキー） |
| E602 | A scenario that goes another way | シナリオが書いたとおりに進みません | 承認のあとのステップか状態が違う（実際のステップと状態、置いてある時計を添える） |
| E603 | An event of a scenario the ticket does not take | チケットが受け付けないイベントがシナリオにあります | 承認者でない人の `approve`。終わったチケットのあとの行。帳簿が断る仮押さえ |
| E604 | A timer at another time | 時計のイベントの時刻が違います | 期限が日の終わりなのに `late` を 23:59 に書いた。催促の時計が鳴ったあとの行を、`remind` の行なしに書いた |
| E701 | A button sends a value the screen does not show | 画面に見せていない値をボタンが送ります | 承認カードが種別の文言を見せ、コードを送る |
| W702 | A number shown or read in other steps than it is sent in | 送る値と違う刻みで見せるか読む数です | 円の小数を整数に丸めて見せる、0.25% 刻みの率を 0.01% 刻みで読む |
| E703 | An item a screen shows has no words in a language | 画面に出る項目に、その言語の文言がありません | `labels ja` に `items.price` が無い。規則の列挙から選ぶ `level` の値 `cfo` に、フィールドの下の題が無い。日本語で書いた共有の経路の `labels en` に、英語の申請の種類が見せるステップ `確認` の題が無い |
| E704 | A number's range a screen cannot hold exactly | 画面が正確に持てない数の範囲です | 明細の列 `serial : number "Serial number"  range >=1 <=99_999_999_999_999_999`。端が、単位の刻みで数えて 2^53 − 1 を越える |
| W901 | A value shaped like a key is written in the file | 鍵の形の値がファイルに書いてあります | マスタの行や説明の文に書いた API の鍵（ほかの言語の W901 と同じ検査。根の DESIGN 16.3） |

ritsu の新しいコード。X18 は認可の検査なので、X15 と X16 と同じくセキュリティの検査の帯（9xx）の続きに、X19 と X20 は言語の境目の検査なので E2xx の続きに置いた。9xx の番号はどの台帳でも一つの検査を指すので（根の DESIGN 16.2）、使われている W910（sekisho）を避けた。



| コード | 見出し（英語） | 見出し（日本語） |
|---|---|---|
| E911、W911 | The gate and the route do not agree on who answers a stage | ステップに答えられる人について、gate と経路が食い違います |
| E209、W209 | A hold may expire before the ticket posts it | チケットが確定する前に、仮押さえの期限が切れることがあります |
| E210、W210 | A flow and a ticket do not agree on what passes between them | flow とチケットのあいだで渡す値が合いません |

### 9.6 使う日の範囲

祝日の表もマスタの行も、いつか終わる。申請の種類は、終わりを書かずに使い続ける。そこで、申請の種類ごとに「使う日の範囲」を一か所（`src/usable.rs`）で求め、マスタの穴（E107）、カレンダーのデータ（E404）、下ろした `terms.cal` と `form.cal` の入力の範囲、一覧に入れる組織の版（8.3）が、同じ範囲を使う。

- **始まり**：`effective from` の日。書いていなければ、読むデータの全部がそろう最初の日（組織の最初の版、選ぶマスタの行の最初の日、カレンダーのデータの最初の日のうち、いちばん遅い日）。
- **終わり**（いつまで使えるかの日）：読むデータの全部がそろう最後の日。選ぶマスタの行を選べる最後の日（終わりの無い行があれば、終わり無し）、ステップが始まった日から期限と催促と上へ回したあとの期限までをカレンダーで数えられる最後の日（5.10。催促を期限と別のカレンダーで数えるステップでは、上へ回したあとの催促を、催促のカレンダーで数えられる最後の日も）、申請者へ戻った日から再申請の期限を数えられる最後の日（5.6）、フォームの営業日の日付を数えられる最後の日のうち、いちばん早い日。保留を書いた申請の種類（`pause at most N business days`）では、ステップの時計の始まりが、止めていた営業日だけ後ろへずれる（5.7、7.5）。だから、ステップを始められる最後の日は、時計を数えられる最後の日より、保留のカレンダーの N 営業日だけ前になる。休みの表を持たないカレンダー（曜日と毎年の休みだけ）は終わらない。カレンダーの終わりは koyomi に尋ねて求め、見当の余白で切らない。
- **日の読み方**：範囲の日は申請の種類の日（7.5）である。オフセットの違うカレンダーは、申請の種類の一日の時刻を、時刻によって前の日か次の日として読む。そこで、そのカレンダーのデータの最初の日と最後の日を、その分だけ内へ詰めて比べ、申請の種類の一日のどの時刻も、そのカレンダーの日でデータの中にあるようにする。下ろした `terms.cal` の入力の範囲は、カレンダーごとに、そのカレンダーの日で書く。

範囲の中の穴は誤りにする。マスタに選べる行の無い日がある（E107）。`effective from` の日に、組織の版がまだ無い（E502）。`effective from` の日か、フォームの営業日の日付を数え始めうる日に、カレンダーのデータがまだ無い（E404）。

終わりは誤りにしない。事実として、`check` の `ok` の行の下に、それを決めているデータと一緒に書く（JSON では `usable`。16.2）。`Requests` の口も同じ事実を返す（9.4）。たとえば、カレンダー `finite.cal` の休みの表が 2026-01-01 から 2027-12-31 までで、勘定科目の行がどれも終わる申請の種類では、次のようになる。

```
request expense v1 "Expense"

use masters m from "masters.ringi"
use calendar office from "finite.cal"

form
  account : choice from m.accounts "Account code"
  amount  : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance
    due in 3 business days at end of day
…
```

```
code,label,valid_from,valid_to
5101,Travel,2026-01-01,2027-09-30
5102,Meetings,2026-04-01,2027-03-31
```

```text
$ ritsu ringi check expense.ringi
ok expense.ringi: a request, 2 fields, 1 stage
  used from 2026-01-01: the first row of the master `m.accounts` (`accounts.csv`, line 2); the first day of the data of the calendar `office` (`finite.cal`)
  used through 2027-09-30: the rows of the master `m.accounts` end on 2027-09-30 (`accounts.csv`, line 2)
  ends within 3 business days at the most
$ ritsu ringi check expense.ringi --lang ja
ok expense.ringi: 申請の種類、フィールド 2 個、ステップ 1 個
  2026-01-01 から使えます: マスタ `m.accounts` の最初の行（`accounts.csv` の 2 行目）。カレンダー `office`（`finite.cal`）のデータの最初の日
  2027-09-30 まで使えます: マスタ `m.accounts` の行は 2027-09-30 まで（`accounts.csv` の 2 行目）
  長くても 3 営業日で終わります
```

終わりを決めるデータが無いときは `used with no end: no data it reads ends`、始まりを決めるデータが無いときは `used from any day: no data it reads has a first day` と書く。どちらも無いとき（組織もマスタも、休みの表を持つカレンダーも読まない申請の種類）は、この二行を書かない。

`check --through <日付>` を渡したときだけ、その日まで使えなければ誤りにする。マスタは E107、カレンダーは E404 で、使える最後の日と、それを決めているデータと、どこまで延ばせばよいかを言う。来年度の末まで使えることを CI で確かめる使い方のためで、`ritsu check --through <日付>` も、同じ日をどの申請の種類にも渡す。

```text
$ ritsu ringi check expense.ringi --through 2028-03-31
error[E107]: expense.ringi:7:3: The request cannot be used through 2028-03-31 (`--through`): it can be used through 2027-09-30
     7 |   account : choice from m.accounts "Account code"
  = The rows of the master `m.accounts` end on 2027-09-30 (`accounts.csv`, line 2).
  = Add rows to the master, or extend them, so that a row may be chosen through 2028-03-31.
error[E404]: expense.ringi:12:1: The request cannot be used through 2028-03-31 (`--through`): it can be used through 2027-12-28
    12 |     due in 3 business days at end of day
  = The data of the calendar `office` (`finite.cal`) ends on 2027-12-31, and the deadlines of a stage started later cannot be counted in it.
  = Extend the calendar's table of closed days, so that the deadlines of a stage started on 2028-03-31 can be counted in it.
```

保留と再申請の期限は、それを決めている行と一緒に言う。上の申請の種類に `pause at most 2 business days` を足すと、ステップを始められる最後の日は、2 営業日だけ前の 2027-12-26 になる。

```text
$ ritsu ringi check paused.ringi --through 2028-03-31      （E404 だけを抜いた）
error[E404]: paused.ringi:12:1: The request cannot be used through 2028-03-31 (`--through`): it can be used through 2027-12-26
    12 |     due in 3 business days at end of day
  = The data of the calendar `office` (`finite.cal`) ends on 2027-12-31, and the deadlines of a stage started later and paused for 2 business days (`pause at most 2 business days`) cannot be counted in it.
  = Extend the calendar's table of closed days, so that the deadlines of a stage started on 2028-03-31 can be counted in it.
```

再申請の期限が決めるときは、`resubmit within` の行で、`the days to resubmit a ticket that goes back to its requester later cannot be counted in it (`resubmit within 13 business days`)` と言う（JSON の `kind` は `resubmit`）。

カレンダーが催促だけを数えるとき（期限は別のカレンダーで数える。5.10）は、数えられないのが催促であることを言う（JSON の `kind` は `reminder`）。上へ回したあとの催促が決めるときは、そう言う。`tests/machine/apart.ringi` の工場のカレンダーを、休みの表が 2027-12-31 で終わるものに替えると、次のようになる（2027-12-23 に始まったステップは、27 日の終わりに期限が過ぎて 28 日に上へ回り、事務所の年末年始の休みのあとの 2028-01-05 に二度目の期限が過ぎる。そのあとの催促は、工場のデータの外になる）。

```text
  used through 2027-12-22: the data of the calendar `plant` (`tests/machine/plant_finite.cal`) ends on 2027-12-31, and the reminders of a stage started later cannot be counted in it after the stage goes up
```

チケットの機械は、この範囲を引数で受け取り、範囲の外の日に始まる物を、どのデータが足りないかを言って `data` の種類で断る（7.2）。断るのは、新しい申請（`submit`）と再申請と、ステップを始めるイベント（議決に届く承認、前のステップへの差し戻し、ステップを済ませる flow の答え、要約のステップの期限）である。文は、`ok` の行の下の文と同じである。

```text
the request can be used through 2027-12-15, not on 2027-12-16 (the data of the calendar `office` (`tests/mutants/shared/finite.cal`) ends on 2027-12-31, and the deadlines of a stage started later and paused for 5 business days (`pause at most 5 business days`) cannot be counted in it)
```

期限を過ぎて上へ回すことは、ステップを始めないので、範囲の終わりを過ぎていても断らない。回したあとの期限は、ステップが始まった日から数える期限で（7.5）、範囲の終わりは、その期限まで数えられる日として求めてある。範囲の最後の日に始まったステップは、保留を使い切っても、催促、期限、上へ回したあとの催促と期限のどれも、データの中で鳴る（`tests/machine.rs` の `a_day_the_data_does_not_cover_is_refused`）。

範囲の終わりを過ぎたあとも、進行中のチケットは、ステップを始めないイベント（議決に届かない答え、最後のステップの承認、却下、取り下げ、催促と期限の時計）を受け付ける。ステップは始まらず、数えられない時計を置く操作（再申請の期限を数えられない日に申請者へ戻すこと、保留の終わりを数えられない日の保留）は、カレンダーのデータを言って `data` で断る。データを延ばせば、同じイベントで進む。管理する人は、`--through` で、その日が来る前に確かめる。

生成するコード（12 章）とサーバーも同じに断る。`api` は、`check` の JSON と同じ `usable` を書き（16.2）、ページ（11 章）は、終わりの日と、それを決めているデータを見せる。

### 9.7 中の物

ringi のファイルが持つ物を、種類と名前で指せる（`src/items.rs`）。口の `Items` が yuen と索引に渡す物と、`ritsu ringi api` の `items` は、同じ関数から作るので、同じ物になる。物は、書いてある行（最初と最後）と、定義を持つ。定義は、意味が変われば変わる文で、行からコメントを除き、文字列の外の続いた空白を一つにした物である。表の桁をそろえ直しても変わらない。

| 種類 | 名前 | 何か |
|---|---|---|
| `field` | フィールド（`amount`。明細のフィールドは、`labels` と同じに `items.price`） | その行と、選択の値か、明細のフィールド |
| `input` | 共有の経路が受け取る入力 | その行 |
| `table` | 経路の表は `route`。名前の付いた表は、その名前 | 見出しと行 |
| `stage` | ステップ | 表の中のステップは、その列（行ごとの入力のセルと、その列のセル）。行で書いたステップは、その行と、下に書いた期限 |
| `who` | 承認者の決め方 | その行 |
| `deadline` | `every_stage`。自分の期限を書いたステップは、その名前 | 期限の行 |
| `ticket` | 決まりの名前（`self_approval`、`same_approver`、`submitted_by`、`send_back`、`resubmit`、`withdraw`、`retract`、`pause`、`approvers_edit`）。`on` の行は、することと相手（`hold_budget.reserve`、`launch_pay`） | その行。同じことを同じ相手にする `on` の行は、合わせて一つ |
| `claim` | 主張 | その行 |
| `example` | `route` | 例の表 |
| `scenario` | シナリオ | その行と、イベントの表 |
| `position`、`team_kind` | 組織のファイルの役職と、チームの種類 | その行 |
| `master` | マスタのファイルのマスタ | その塊 |

名前は、書いたとおりの名前で指す（`金額(amount)` なら `金額`）。`api` は、括弧の中の名前を `name` に、書いたとおりの名前を `written` に書く。

yuen は、`satisfied by ringi "<ファイル>" <種類> <名前>` で、どの種類の物も指せる。`verified by` に書けるのは、ringi の検査が確かめて落としうる物、つまり主張（`claim`）、例（`example`）、シナリオ（`scenario`）と、ファイルの全体（`verified by ringi "<ファイル>"`）である。ほかの種類を `verified by` に書けば、yuen の E403 になる。例の要件（`examples/shared/expense.req`）は、経路の表、承認者の決め方、自己承認の決まり、期限、上へ回す表を `satisfied by` で、例と主張を `verified by` で指している。

```
  satisfied by ringi "../expense.ringi" table route
  satisfied by ringi "../expense.ringi" who manager
  verified by ringi "../expense.ringi" example route
  satisfied by ringi "../expense.ringi" ticket self_approval
  satisfied by ringi "../expense.ringi" deadline every_stage
  verified by ringi "../expense.ringi" claim reminder_before_due
```

定義は、yuen が、人が確かめた記録に持つハッシュの元になる。定義が変われば、その物を指す確かめ済みのつながりは、人がもう一度確かめるまで yuen の E303 になる（yuen の決まりで、どの言語の物でも同じ）。だから、一つの直しで変わる定義は、その直しが意味を変える物だけにしてある。経費の `due in 3 business days` を 5 営業日に変えれば、変わるのは `deadline every_stage` だけで、経路の表のセルを一つ変えれば、表と、その列のステップだけが変わる（`tests/items.rs` の `a_change_of_meaning_changes_the_things_it_is_written_in`）。

## 10. 参照インタプリタ、`run`、シミュレーション、ベクタ

### 10.1 参照インタプリタ

参照インタプリタは、ringi のクレートの Rust で書いたチケットの機械で、7 章の意味を決める（`src/engine/`）。関数は二つである。

- `start(submit, organization, now)` は、申請からチケットを作る。
- `next(ticket, event, organization, now)` は、チケットをイベント一つで進める。

どちらも、時刻、組織の部分（7.5）、フォームの値を引数で受け、ほかに何も読まない。答えは、次のチケットとすることの一覧か、断り（種類と文。7.2）である。同じ引数には、いつも同じ答えを返す。

機械は、申請の種類の検査の答えから一度作る。表と条件と計算は、下ろした規則を口の `Rules::eval` で、期限と催促は、下ろした日付を `Dates::eval` で尋ねる。営業日で数える再申請の期限と保留は、`Dates::calendar` が返すカレンダーの休みで数える。承認者は、検査の一覧と `simulate` と同じ関数（`org::resolve`、`org::listing`）で、渡された組織の部分の上で決める。経路の歩き方、承認者の決め方、フォームの値の受け取りのどれも、機械だけの写しを持たない（`simulate --form` も同じ関数を呼ぶ）。ringi のクレートは rulec、koyomi、chobo に依存しない。

機械は帳簿に触らない。仮押さえ、確定、取消は、することの一覧に入れて返し、`run` が `Books::open` の帳簿に流す（サーバーは chobo のクライアントで行う）。

組織の部分は、`Machine::part` が、版のデータから 7.5 のとおりに切る（`run` と `vectors` が呼ぶ）。サーバーは、自分の保管から同じに切って渡す。サーバーが持つ物（申請の種類の説明、版のデータ、チケットとイベントの JSON）だけから切った部分が、どのベクタの部分とも同じことは、`tests/vectors.rs` の `a_server_cuts_the_part_of_each_vector` が確かめる。機械と `simulate` と検査の一覧は、どの申請者についても同じ承認者、同じ回り方を答える。試験は、例と機械の申請の全部で、組織のどの人も申請者にして、これを突き合わせる（18 章）。

### 10.2 `run` とシナリオ

`scenario` は、申請者、帳簿の始めの振替（`given leave.grant(person: "e1203", year: "2026", days: 20)`。空の帳簿では仮押さえが拒否されるため）、フォームの値、イベントの表を持つ。表の行は、時刻（`at`）、イベント、する人（`by`。時計と flow のイベントでは空）、人が書いた文（`comment`。7.2）、そのイベントのあとに期待するステップ（`-> stage`。終わったチケットと、申請者の手元のチケットでは空）、期待する状態（`status`）である。`comment` の列は、文を書く表だけが見出しに書き、`by` のあとに置く。セルは `"` で囲んだ文一つか空で、空ならコメント無しである（時計と flow の行では空）。文を `resubmit(…)` や `edit(…)` の引数にしないのは、引数の名前がフォームのフィールドの名前で、`comment` という名前のフィールドとぶつかるからである。日付と帳簿の答えはシナリオに書かず、`run` が計算する（21 章の 1）。

イベントの引数は、括弧に書く。`send back(to: first)`（書かなければ申請者へ）、`edit(price: 90_000JPY)`、`resubmit(price: 90_000JPY)`、`add approver(approver: person "e0102")`、`hand over(from: person "e1101", to: person "e3101")`、`agent answered(verdict: doubt, reason: "No budget line fits")`、`note ready(summary: "…")`。`submit` の `by` が申請者でなければ、代理申請である。人とチームのフィールドの値は `person "e9003"`、`team legal` と書き、添付のフィールドの値は、ファイルの名前の文で書く（`receipt = "receipt.pdf"`）。

`resubmit` と `edit` の引数に書いた `none` は、そのフィールドを空にする（`resubmit(note: none)`。7.3）。申請の始めのフォーム（`form` の行と `simulate --form`）の `none` は、値を入れないことである。どちらも「そのイベントのあと、そのフィールドに値が無い」と読む。`run` は、その行で値が無くなったフィールド（空にした物と、見せなくなって落とした物）を、行の終わりに宣言の順で言う。機械の試験の `supplies` で、物品を役務に変えた出し直しは、物品のときだけ見せる仕入先と、仕入先を書いたときに見せる連絡先を落とす。

```text
$ ritsu ringi run tests/machine/supplies.ringi --scenario a_kind_changed_on_resubmit
2026-11-16 10:00  submit                     by e1201  -> first (e1101), due 2026-11-19 end of day
2026-11-16 11:00  send back                  by e1101  -> with requester, resubmit by 2026-11-19 end of day
2026-11-16 13:00  resubmit (kind = service)  by e1201  -> first (e1101), due 2026-11-19 end of day; now empty: vendor, contact
2026-11-16 14:00  approve                    by e1101  -> approved
scenario `a_kind_changed_on_resubmit`: as written (4 rows)
```

日本語の名前の版を `--lang ja` で流すと、同じ行は `resubmit (種類 = 役務)` で始まり、`空になった: 仕入先、連絡先` で終わる。引数の値は、選択の値も、申請に書いた名前で見せる（JSON の名前の `service` ではない）。

`ritsu ringi run` は、行を一つずつ、書いた時刻に機械へ渡す。行ごとに、その時刻の組織の部分を切り、帳簿の時間を進め、機械が返したすることの一覧のうち帳簿の分を帳簿に流す。見せるのは、行ごとに、チケットがどこにいて、だれを待ち、いつが期限で、帳簿と flow に何をさせたかである。終わりに、帳簿の残高と仮押さえを見せる。

```text
$ ritsu ringi run examples/leave.ringi --scenario a_long_paid_leave
2026-11-16 09:00  submit   by e1203  -> manager (e1101), due 2026-11-18 end of day; hold leave.take 8 day
2026-11-16 17:00  approve  by e1101  -> head (e3101), due 2026-11-18 end of day
2026-11-17 09:00  remind             -> head (e3101), due 2026-11-18 end of day; reminded e3101
2026-11-19 00:00  late               -> head (e0101), due 2026-11-23 end of day; head went up to e0101
2026-11-19 10:00  approve  by e0101  -> approved; post leave.take
the book `leave` at the end: leave_left(e1203) 12 day, granted -20 day, taken 8 day
  the hold `take` (T-1, attempt-1): posted
scenario `a_long_paid_leave`: as written (5 rows)
```

`head` の期限は、上長が承認した 11 月 16 日から 2 営業日なので、上長のステップと同じ日になる。期限が切れた 11 月 19 日 00:00 に本部長へ回り、新しい期限は、下ろした `due_escalated` のとおり、その日から 2 営業日の 11 月 23 日の終わりである（7.5）。

文を書いた行は、行の終わりに文を見せる。休暇の例の次のシナリオでは、上長が代わりの人を尋ね、申請者が答え、上長が理由を書いて差し戻し、申請者がどこに書くかを尋ね返して上長が答え、申請者が代わりの人を書いて出し直す。申請者に戻っているあいだの申請者のコメントは、差し戻した上長に知らせる（7.3）。

```text
$ ritsu ringi run examples/leave.ringi --scenario a_leave_sent_back_with_a_comment
2026-11-16 09:00  submit                             by e1203  -> manager (e1101), due 2026-11-18 end of day; hold leave.take 3 day
2026-11-16 11:00  comment                            by e1101  -> manager (e1101), due 2026-11-18 end of day; comment "Who covers for you while you are away?"
2026-11-16 13:00  comment                            by e1203  -> manager (e1101), due 2026-11-18 end of day; comment "e1201 does"
2026-11-16 17:00  send back                          by e1101  -> with requester, resubmit by 2026-11-23 end of day; comment "Write e1201 as the cover, please"; void leave.take
2026-11-16 17:30  comment                            by e1203  -> with requester, resubmit by 2026-11-23 end of day; comment "Where do I write the cover?"
2026-11-16 17:45  comment                            by e1101  -> with requester, resubmit by 2026-11-23 end of day; comment "In the cover field of the form"
2026-11-17 09:00  resubmit (cover = person "e1201")  by e1203  -> manager (e1101), due 2026-11-19 end of day; comment "e1201 is written as the cover"; hold leave.take 3 day
2026-11-17 10:00  approve                            by e1101  -> approved; comment "Have a good trip"; post leave.take
the book `leave` at the end: leave_left(e1203) 17 day, granted -20 day, taken 3 day
  the hold `take` (T-1, attempt-1): voided
  the hold `take` (T-1, attempt-2): posted
scenario `a_leave_sent_back_with_a_comment`: as written (8 rows)
```

`same approver approves once, at the last stage`（5.12）のステップは、行の終わりに、待たせたこと（`first waits for the approval of decide (e0101)`）、先のステップの承認で済んだこと（`first done with the approval of decide (e0101)`）、待たせるのをやめて始めたことを言う。購買の例の二つのシナリオでは、営業一部長の e3101 の申請で、上長と決裁者がどちらも本部長の e0101 になる。一つ目では、財務のあとの決裁で e0101 が一度だけ答える。二つ目では、決裁が期限を過ぎて社長へ回り、社長が承認するので、待っていた `first` がそのあとに始まり、e0101 はそこで答える。

```text
$ ritsu ringi run examples/purchase.ringi --scenario the_manager_decides_after_finance
2026-11-16 10:00  submit   by e3101  -> finance (e4001), due 2026-11-23 end of day; first waits for the approval of decide (e0101); hold budget.reserve 2_000_000 JPY
2026-11-16 14:00  approve  by e4001  -> decide (e0101), due 2026-11-23 end of day
2026-11-17 10:00  approve  by e0101  -> approved; first done with the approval of decide (e0101); inform circulated to e5001; launch order; post budget.reserve
the book `budget` at the end: dept_budget(sales1) 8_000_000 JPY, allocations -10_000_000 JPY, payees 2_000_000 JPY
  the hold `reserve` (T-1, attempt-1): posted
scenario `the_manager_decides_after_finance`: as written (3 rows)
$ ritsu ringi run examples/purchase.ringi --scenario the_decision_goes_up_and_the_manager_answers_after
2026-11-16 10:00  submit   by e3101  -> finance (e4001), due 2026-11-23 end of day; first waits for the approval of decide (e0101); hold budget.reserve 2_000_000 JPY
2026-11-16 14:00  approve  by e4001  -> decide (e0101), due 2026-11-23 end of day
2026-11-19 09:00  remind             -> decide (e0101), due 2026-11-23 end of day; reminded e0101
2026-11-24 00:00  late               -> decide (e0001), due 2026-12-01 end of day; decide went up to e0001
2026-11-24 10:00  approve  by e0001  -> first (e0101), due 2026-12-01 end of day; first no longer waits for decide, and starts (e0101; the stage it waited for was answered without e0101)
2026-11-24 15:00  approve  by e0101  -> approved; inform circulated to e5001; launch order; post budget.reserve
the book `budget` at the end: dept_budget(sales1) 8_000_000 JPY, allocations -10_000_000 JPY, payees 2_000_000 JPY
  the hold `reserve` (T-1, attempt-1): posted
scenario `the_decision_goes_up_and_the_manager_answers_after`: as written (6 rows)
```

シナリオの表の `-> stage` には、チケットの今のステップを書く。待っているステップは今のステップにならないので、一つ目のシナリオの `submit` の行は `finance` である。

行は、黙って通さない。

- 機械が断るイベント（2,000 字を超えるコメントも）、帳簿が断る仮押さえ、終わったチケットのあとの行、時計と flow の行の `by` か `comment` のセルに書いた物は E603。
- 行に書いたステップか状態が、チケットのステップか状態と違えば E602。
- 時計の行（`remind`、`late`、`resubmit late`、`pause ends`）は、機械が置いた時刻ちょうどに書く。違う時刻の時計の行と、置いていない時計の行は E604。時計が鳴る時刻より後の行を、時計の行なしに書いても E604 で、書けばよい行を注に出す（鳴った時計が先。7.6）。催促も時計なので、行に書く。上へ回したあとの催促も同じである。
- 使わない行は無い。シナリオは最初の一つで止まり、そこまでの行と、チケットがどこにいるかを見せる。

```text
$ ritsu ringi run tests/mutants/E604_a_row_after_a_clock_rings.ringi
2026-06-01 09:00  submit  by e03  -> check (e04), due 2026-06-04 end of day
scenario `a_late_approval`: not as written
error[E604]: tests/mutants/E604_a_row_after_a_clock_rings.ringi:28:22: The clock of `remind` rings at 2026-06-03 09:00, before this row (2026-06-03 15:00)
    28 | | 2026-06-03 15:00 | approve | e04 |          | approved    |
  = Write the row of a clock that rings: `| 2026-06-03 09:00 | remind | | check | … |`.
  = The ticket is at `check`, and is `in progress`. The clocks set: remind 2026-06-03 09:00 (check), late 2026-06-05 00:00 (check).
```

`check` も、申請の種類のシナリオを同じに流し、E602〜E604 を診断にする。`--at` を渡したときは流さない。シナリオは行の日付で書いてあり、`--at` は組織の版を一つしか読まないからである。

`run` の終了コードは、どのシナリオも書いたとおりなら 0、そうでないか、ファイルが検査を通らなければ 1、シナリオが無いか、その名前のシナリオが無いか、組織のデータが無ければ 2 である。シナリオの無い申請の種類には、流す物が無いことを標準エラーに一行で言う（`vectors` と同じ終了コード）。

```text
$ ritsu ringi run tests/machine/bare.ringi
error: `tests/machine/bare.ringi` has no scenario
```

`--lang ja` は、つなぎの言葉を日本語にする（イベント、状態、履歴の語は、書くときと同じ英語のままである）。`--format json` は 16.2。

### 10.3 シミュレーション

`ritsu ringi simulate` は、申請者とフォームの値と日付を選んで、経路の各ステップの承認者と期限を見せる。承認者は、`--on` の日（無ければ今日）に効いている組織の版で 8.5 のとおりに決め、その日に有効な代理（5.8）も添える。フォームの値は、シナリオの `form` の行と同じ書き方で `--form` に渡す。値は下ろした規則に渡して計算し、経路の表の行を選ぶ。フォームが取らない値（範囲の外、無いフィールド、その日に選べないマスタの行）は、どのフィールドかを言って断る。マスタから引く決め方は、フォームで選んだ行の人だけを見せる（`--form` が無ければ、行ごとに見せる）。期限は、最初のステップは下ろした `terms.cal` で日付と時刻まで数え、二つ目からは始まった日から何営業日かを言う（始まる日は、前のステップがいつ答えるかで決まる）。最初のステップの催促は、機械が置くとき（期限より前に来るとき。5.10）だけ、日付と時刻を添える。`same approver approves once`（5.12）で済むステップには、どのステップの承認で済むかを添え、期限を書かない。`same approver approves once, at the last stage` で待つステップには、どのステップで一度だけ答えるかを添え、同じく期限を書かない。

```text
$ ritsu ringi simulate examples/expense.ringi --requester e1201 --form 'category = purchase, items = [{what: "Server", price: 800_000JPY}, {what: "Storage", price: 400_000JPY}]' --on 2026-11-16
examples/expense.ringi on 2026-11-16, in the organization from 2026-04-01
  computed: amount = 1_200_000JPY
requester e1201 (Sales staff 1)
  first   manager: e1101 (Manager of Section 1A)
          due 2026-11-19 at the end of the day, reminder on 2026-11-18 09:00
  second  dept_head and accounting
            dept_head: e3101 (Head of Sales Department 1)
            accounting: one of e1001 (Head of Accounting), e1002 (Accountant)
          due 3 business days after it starts, at the end of the day
  decide  division_head: e0101 (Head of Sales Division)
          due 3 business days after it starts, at the end of the day
```

購買の例は `same approver approves once, at the last stage` を書いている。営業一部長の e3101 の申請では、上長のステップ `first` と決裁 `decide` がどちらも本部長の e0101 に当たるので、`first` は `decide` で一度だけ答えると言う。

```text
$ ritsu ringi simulate examples/purchase.ringi --requester e3101 --on 2026-11-16 --form 'kind = goods, vendor = "V-0012", lines = [{item: "Demo units", quantity: 4, price: 500_000JPY}], summary = "Demo units", quote = "quote.pdf", advance = 0%'
examples/purchase.ringi on 2026-11-16, in the organization from 2026-04-01
  computed: amount = 2_000_000JPY
requester e3101 (Head of Sales Department 1)
  first    manager: e0101 (Head of Sales Division); answers once, at `decide` (`same approver approves once, at the last stage`)
  finance  finance: e4001 (Head of Finance)
           due 2026-11-23 at the end of the day, reminder on 2026-11-19 09:00
  decide   division_head (`authority.decider` is `division_head`): e0101 (Head of Sales Division)
           due 5 business days after it starts, at the end of the day
  inform   general_affairs: e5001 (Head of General Affairs)
```

`--form` が無いときは、待つ先が、フォームの値で来るかどうかの変わるステップか、値で決め方の変わるステップなら、`where it comes, and to them` を付けて言う。待つ先になりうるステップが二つ以上あれば、その名前を並べて、来て同じ人に当たる最後のステップで答えると言う（試験の申請では ``answers once, at the last of `second` and `third` that comes, and to them``）。

```text
$ ritsu ringi simulate examples/purchase.ringi --requester e3101 --on 2026-11-16
examples/purchase.ringi on 2026-11-16, in the organization from 2026-04-01
  without `--form`: every stage the route can come to, by each way to choose its approver; which come depends on the form
requester e3101 (Head of Sales Department 1)
  first      manager: e0101 (Head of Sales Division); answers once, at `decide`, where it comes, and to them
  technical  reviewer: a person chooses when the ticket runs
  finance    finance: e4001 (Head of Finance)
  board      board: 3 of e0001 (President (fictional)), e0201 (Director 1), e0202 (Director 2), e0203 (Director 3), e0204 (Director 4)
  decide     dept_head (`authority.decider` is `dept_head`): it comes to the requester alone; it goes up to e0101 (Head of Sales Division)
             division_head (`authority.decider` is `division_head`): e0101 (Head of Sales Division)
             president (`authority.decider` is `president`): e0001 (President (fictional)); when it is late, it cannot go up, and expires at the deadline
  inform     general_affairs: e5001 (Head of General Affairs)
```

`--on` が無ければ、今日を、申請が使うカレンダー（共有の経路が使うものも）の UTC オフセットで読む。期限の時刻と同じ読み方で、日本時間（+09:00）のカレンダーを使う申請なら、0 時から 9 時のあいだも日本の日付が今日になる。オフセットの違うカレンダーを二つ以上使う申請では、今日がどの日かが決まらないので、`--on` で日を選ぶように言って 2 で終わる（どちらか一つを選ぶ決まりを置くと、期限を数えるカレンダーと今日の読み方が食い違う申請者が出るため）。オフセットを書いたカレンダーを使わない申請は、今日を UTC で読む。どう読んだかは、頭の行の下に書く。

```text
$ ritsu ringi simulate examples/leave.ringi --requester e1201
…
  today, read in the UTC offset of the calendar `office` (+09:00), as the deadlines are; `--on <date>` gives another day
  without `--form`: every stage the route can come to, by each way to choose its approver; which come depends on the form
…
$ ritsu ringi simulate tests/mutants/W510_nobody_answers_the_president.ringi --requester e01
…
  today, read in UTC: the request uses no calendar that says its UTC offset; `--on <date>` gives another day
  without `--form`: every stage the route can come to, by each way to choose its approver; which come depends on the form
…
```

頭の行の日は、走らせた日による。2026-11-15 の 0 時台（日本時間）に走らせれば、カレンダーを使う休暇は 11 月 15 日、使わない申請は UTC の 11 月 14 日になる。

```
examples/leave.ringi on 2026-11-15, in the organization from 2026-04-01
tests/mutants/W510_nobody_answers_the_president.ringi on 2026-11-14, in the organization from 2026-04-01
```

`--form` を渡さなければ、経路が来うるステップの全部を、承認者の決め方ごとに見せる（どれが来るかはフォームで決まる）。承認者のいないステップは、`when empty` のとおりに、その人のその版で上へ回るか、飛ぶか、止まるかを言う（`it goes up to e0102 (Head of Corporate Division)`、`no head is above the requester; the stage is skipped`、`no head is above the requester; the ticket waits until an administrator adds an approver`）。版にいない人を指す上へ回すステップは、回せないと言う（2027 年の版の契約の `counsel: e9001 is not in the organization; it cannot go up`）。

期限を過ぎて上へ回すステップ（`when late escalate`）で、回す先の無い人には、承認者の行に、期限を過ぎたらどうなるかを添える（`when it is late, it cannot go up, and expires at the deadline`。二度目に回せないなら `when it is late, the second time, it cannot go up, and expires at the second deadline`。5.10）。`check` が `ok` の下で数え、W507 で言う申請者を、ここで一人ずつ確かめられる。回覧のステップには添えない。前の承認で済むステップと、あとのステップの承認を待つステップ（5.12）にも、フォームの値によらずそうなるときは添えない（時計を置かないステップは、期限を過ぎない）。

`--every-requester` は、組織のどの人についても見せ、同じ承認者になる申請者を一つにまとめる。`submitted by` を書いた申請の種類では、出せる人だけを並べ、出せない人の数と 3 人までの名前を頭の行に書く（5.13）。管理する人が、組織の切り替えの前に、予約した版の日を `--on` に渡して確かめるのに使う。

```text
$ ritsu ringi simulate examples/leave.ringi --every-requester --on 2027-04-01
examples/leave.ringi on 2027-04-01, in the organization from 2027-04-01
  without `--form`: every stage the route can come to, by each way to choose its approver; which come depends on the form
  5 people may not submit it (`submitted by everyone except team board`): e0001 (President (fictional)), e0201 (Director 1), e0202 (Director 2) and 2 more
requesters e0101 (Head of Sales Division), e0102 (Head of Corporate Division) (2 requesters)
  manager  manager: e0001 (President (fictional)); when it is late, it cannot go up, and expires at the deadline
  head     dept_head: no team of the kind `department` is above the requester; it goes up to e0001 (President (fictional)); done with the approval of `manager` (`same approver approves once`)
…
requesters e9002 (Lawyer 2), e9003 (Lawyer 3) (2 requesters)
  manager  manager: e0102 (Head of Corporate Division)
  head     dept_head: the head of legal (Legal) is empty; it goes up to e0102 (Head of Corporate Division); done with the approval of `manager` (`same approver approves once`)
```

仮押さえの引数を作れない申請者（`dept: requester.department` を読む仮押さえの行に `when` が無く、上に部の無い人）には、その人の行の頭に、`submit` が断られることを書く（機械が `book` の種類で断るのと同じ人。7.2。検査は X19 の W209 で人数を言う。9.3）。下は、そういう仮押さえを持つ機械のための申請 `tests/machine/held.ringi` である。

```text
$ ritsu ringi simulate tests/machine/held.ringi --every-requester --on 2026-11-16
tests/machine/held.ringi on 2026-11-16, in the organization from 2026-04-01
  without `--form`: every stage the route can come to, by each way to choose its approver; which come depends on the form
requester e0001 (President (fictional))
  `submit` is refused: the hold `budget.reserve` takes `dept` from `requester.department`, and no team of the kind `department` is above the requester
  decide  manager: no head is above the requester; it goes to `board`: one of e0201 (Director 1), e0202 (Director 2), e0203 (Director 3), e0204 (Director 4); when it is late, it cannot go up, and expires at the deadline
requesters e0101 (Head of Sales Division), e0102 (Head of Corporate Division), e0201 (Director 1) and 3 more (6 requesters)
  `submit` is refused: the hold `budget.reserve` takes `dept` from `requester.department`, and no team of the kind `department` is above the requester
  decide  manager: e0001 (President (fictional)); when it is late, it cannot go up, and expires at the deadline
…
```

`--format json` は、申請者ごとに、ステップ、承認者の決め方、人の ID、代理、期限、断られる仮押さえ（`holds_refused`）を一つのオブジェクトに書く。出せない人の ID は `may_not_submit` に並べ、日を `--on` で選んだかを `on_given` に、今日をどのオフセットで読んだかを `today_in` に書く（16.2）。組織のデータが無いとき、`--on` の日に効いている版が無いとき、組織にいない申請者、`submitted by` に当たらない申請者（代理申請なら、代わられた人）、フォームが取らない値は、2 で終わる。ファイルが検査を通らなければ、`check` と同じ診断を出して 1 で終わる。

### 10.4 ベクタ

`ritsu ringi vectors <file>` は、チケットの機械の遷移を JSON Lines で書く。一行が一つの遷移で、機械に渡した物（前のチケット、イベント、組織の部分、時刻）と、機械の答えを持つ。生成したコード（TypeScript、Python、Go、Rust）とサーバーは、同じ物を渡されて、同じ答えを返さなければならない（18 章、12.4）。JSON の形は、生成したコードの `start` と `next` が受け渡す約束の形（12.4）で、`ritsu ringi run --format json` も同じ形で書く。

| キー | 中身 |
|---|---|
| `request` | 申請の種類の名前（3.4 の括弧の中の名前） |
| `from` | どこから作ったベクタか（人が読む文） |
| `now` | イベントの時刻（UTC の秒） |
| `ticket` | 前のチケット。`submit` では null |
| `event` | イベントの名前の一つのキーに、引数と、時計、`submit`、`comment` のほかのイベントならチケットの版（`revision`）、人のイベントならその人が書いた文（`comment`。無ければ null）。`submit` と `resubmit` は試みの名前（`attempt`。7.1）も持つ。管理する人のイベント（`add_approver`、`hand_over`）の `by` は、操作した人か null（12.4）。時計のイベントは名前だけ（`"late"`）。`submit` は `{"submit": …}` |
| `organization` | 組織の部分（その日、版、役職の並び、チーム、人、所属、代理、マスタの行） |
| `answer` | `ok` に、次のチケット（`ticket`）とすることの一覧（`effects`）。断りなら `err` に、種類（`refused`）、英語と日本語の文（`text`、`text_ja`）、場所（`at`。12.4 の「断り」） |

申請者が自分の申請を承認しようとするベクタは、次の一行である（`ticket` と `organization` の中身を省いた）。

```text
{"request":"leave","from":"scenario a_long_paid_leave, after line 47: the requester","now":1794787200,"ticket":{…},"event":{"approve":{"by":"e1203","revision":{"round":1,"stage":0,"edits":0},"comment":null}},"organization":{…},"answer":{"err":{"refused":"person","text":"e1203 (Sales staff 3) made the request, and does not answer it (`self approval not allowed`)","text_ja":"e1203（営業担当 3）は、この申請を出した人なので、答えられません（`self approval not allowed`）","at":null}}}
```

承認する人が、空の文だけのコメントを書くベクタは、次の一行である。

```text
{"request":"leave","from":"scenario a_long_paid_leave, after line 47: comments","now":1794787200,"ticket":{…},"event":{"comment":{"by":"e1101","comment":""}},"organization":{…},"answer":{"err":{"refused":"comment","text":"the text of `comment` is empty","text_ja":"`comment` の文が空です","at":null}}}
```

却下と差し戻しに理由を求める休暇の申請（`comment required  for reject, send back`。7.2）で、承認する人が理由を書かずに却下するベクタは、次の一行である。

```text
{"request":"leave","from":"scenario a_long_paid_leave, after line 47: a reason asked, and none given","now":1794787200,"ticket":{…},"event":{"reject":{"by":"e1101","revision":{"round":1,"stage":0,"edits":0},"comment":null}},"organization":{…},"answer":{"err":{"refused":"comment","text":"a `reject` of this request needs a reason: write it in the comment","text_ja":"この申請の `reject` には理由が要ります。コメントに書いてください","at":null}}}
```

差し戻された休暇の申請を、理由（`reason`。特別休暇のときだけ必須）を空にして出し直すベクタと、同じ理由に値も渡したベクタは、次の二行である。一行目の答えのチケットは、`form` に渡さなかったほかのフィールドの値を持ったままで、履歴の `resubmitted` の行は `"cleared":["reason"]` を持つ（7.3）。二行目に挙げるのは、一つだけなら空にできるフィールドである。どの申請でも必須のフィールドを挙げると、`clear` の決まりを守らない機械も、必須が欠けたとして `form` で断り、ベクタがその違いを見分けられない。

```text
{"request":"leave","from":"scenario a_long_paid_leave, after line 47: made again, with fields made empty","now":1794873600,"ticket":{…},"event":{"resubmit":{"by":"e1203","form":{},"clear":["reason"],"attempt":"attempt-2","revision":{"round":1,"stage":null,"edits":0},"comment":null}},"organization":{…},"answer":{"ok":{…}}}
{"request":"leave","from":"scenario a_long_paid_leave, after line 47: made again, with fields made empty","now":1794873600,"ticket":{…},"event":{"resubmit":{"by":"e1203","form":{"reason":"A family visit"},"clear":["reason"],"attempt":"attempt-2","revision":{"round":1,"stage":null,"edits":0},"comment":null}},"organization":{…},"answer":{"err":{"refused":"form","text":"`reason` is given a value, and is in `clear` too","text_ja":"`reason` に値を渡していて、`clear` にも挙げています","at":"reason"}}}
```

どのベクタも、参照インタプリタが実際に行った遷移である（`run` と同じ一歩）。次に渡すイベントは、機械が返したチケットから選ぶ（今のステップが待っている人、置いてある時計、通ってきたステップ）。境の値は、機械に値を一つずつ尋ねて探す。ベクタを作る側は、経路の歩き方も、期限も、範囲も持たない。機械をもう一つ書けば、いつか元の機械と食い違い、ベクタが、どの機械もしないことを言うようになるからである。

元は、申請の種類のシナリオである（申請者、機械が受け付けるフォーム、最初の時刻を持つ）。そこから次を作る。

- シナリオの行そのもの。
- シナリオが着いた各ステップで：承認を一人ずつ（議決に届く所）、却下を一人ずつ（議決に届かなくなる所）、申請者の承認（断り）と取り下げ、差し戻しとその先（再申請、破棄、再申請の期限）、通ってきたステップへの差し戻し、保留と再開と保留の時計、前の承認の取り消し、エージェントと flow の答え、だれも答えないときの時計を順に全部。シナリオの最初のステップと、前のステップがその承認を待っているステップ（5.12）と、部分が止まったステップ（5.9）では、管理する人の `add approver` と `hand over` も作る（待っているステップでは、足した人か引き継いだ人の承認と、その先も）。人の無い操作（`by` が null）と、管理する人が名指しで行う操作（`by` がその人。申請の種類ごとに一度）の両方である。
- あとのステップの承認を待っているステップ（5.12）があれば、待つ先のステップで：期限を過ぎて回した先の人の承認と、管理する人が足した人、引き継がせた人の承認（どれも、待っていたステップがそのあとに始まる）、そのあとの承認を順に、待っていたステップへの差し戻しと、そこでの答え。
- 最初のステップで：時計を 1 分早く渡す（断り）、時計が鳴る時刻ちょうどの承認とコメント（どちらも断り。鳴った時計が先）、差し戻しと再申請を上限まで（その次は断り）。
- 人がチケットに書くこと（7.2）。申請の種類ごとに一度ずつ作る。最初のシナリオの申請を、コメントを添えて出すものと、長すぎるコメントを添えて出すもの（断り）。シナリオが着いた最初のステップで：待っている承認者のコメントと申請者のコメント、空のコメント（断り）、いちばん長いコメント（2,000 字）と、それより一字長いコメント（断り）、コメントを添えた承認と差し戻し、長すぎるコメントを添えた承認（断り）。ほかに、差し戻したチケットへの、差し戻した人のコメント（知らせは申請者に行く）と申請者のコメント（知らせは差し戻した人に行く）、申請者が自分で取り下げて手元に戻したチケットへの申請者のコメント（相手がいないので知らせは出ない）、保留したチケットへのコメント、代わりの人が出したチケットへのコメント（知らせは申請者と代わりの人の両方に行く）と、それを差し戻したあとの申請者のコメント（知らせは代わりの人と差し戻した人に行く）、シナリオが終わらせたチケットへのコメント（断り）。長いコメントは、UTF-8 で 4 バイト、UTF-16 で 2 単位になる字（「𠮷」）を並べて書く。生成したコードが、バイトや UTF-16 の単位でなく字の数で数えることを、このベクタで確かめる。
- 理由を求める操作（`comment required for`。7.2）。申請の種類ごとに一度ずつ作る。シナリオが着いた最初のステップで、待っている最初の承認者が、求める操作ごとに、コメント無し、空のコメント、空白だけのコメント（「 　\n」。半角の空白、全角の空白、改行）、U+0085 だけのコメントで行うもの（どれも断り）と、空白でない一文字だけのコメント（U+FEFF と U+001C。各言語の `trim` や `strip` が、どちらかを空白として除く）で行うもの（どれも受け付ける）。ベクタがほかの所でその操作をするときは、理由（`The reason for it`）を添える。
- 値を空にすること（7.3）。申請の種類ごとに一度ずつ作る。シナリオが最初に申請者へ差し戻したチケットで、申請者が入れるフィールドを一つずつ空にする出し直し（どの申請でも必須の物は断り）、フォームに無い名前、計算するフィールド、承認者だけのフィールド、値も渡したフィールド（一つだけなら空にできて、チケットが値を持つ物）を挙げる出し直し（どれも断り）、選択のフィールドを、ほかの値の一つずつに変える出し直し（見せなくなるフィールドの値を落とす）と、選択ごとに一度、そのうちの最初にフィールドの値を落とす変え方に、落ちるフィールドの値も渡す出し直し（見せないフィールドの値なので断り。イベントが渡す値は落とさない）。シナリオが着いた最初のステップで、承認者が入れるか直せるフィールドがあれば、待っている承認者の直しで、その一つずつを空にするもの、直せない申請者のフィールド、フォームに無い名前、値も渡したフィールドを挙げるもの（どれも断り）。
- 条件で見せるフィールドへの承認者の直し（7.3）。承認者が入れるか直せるフィールドのうち `shown when` か `required when` を持つ物に、シナリオが着いた各ステップで、待っている承認者が値を入れる。フォームがそのフィールドを見せるチケットでは受け付け、見せないチケットでは断る（`form`）。フィールドごとに、受け付ける物と断る物を一本ずつ作る。
- 古い版：シナリオがチケットを別の版へ進めた最初の遷移（終わっていないチケットへの遷移があれば、そちら）のあとのチケットに、その前の版を持たせた、時計と `comment` のほかのイベントの種類を一つずつ（15 本。どれも `stale` の断り）と、版を持たない承認を一本。加えて、同じチケットへのコメント（版を持たないので、チケットが先の版へ進んでいても受け付ける）と、前の版を持たせた、長すぎるコメントを添えた承認（`stale` の断り。文より先に版を確かめる）。申請の種類ごとに一度だけ作る。
- 休みの前の日に出した申請と、その時計（次の休みと、先の一年でいちばん長い休み）。
- 組織のどの人も申請者に（出せない人は断り）。`proxies.csv` の代理申請を、期間の最初と最後の日と、その前後の日に。代わりに出した人が、そのチケットを取り下げるもの（`withdraw` の行のある申請の種類）と、差し戻されたあとに破棄するもの（履歴の行の `proxy_of` は申請者）。
- 承認の代理の承認と、その代理が承認者の席でするほかのこと（申請者への差し戻し、却下、保留と再開、承認者が直すフィールドの `edit`、承認とその取り消し。保留、`edit`、取り消しは、申請の種類に行があるものだけ）。代理の答えるステップがある最初のシナリオの申請で作る。差し戻したチケットは代理した人本人を差し戻した人として持ち、どの履歴の行の `proxy_of` も代わられた人である。`same approver approves once` の申請の種類では、最初のステップを代わられた人に引き継がせて代理が承認し、すぐあとの、その人が一人で答えるステップがその承認で済むものも作る（済んだステップの `approved` の行は、`by` が代理、`proxy_of` が代わられた人）。
- 組織の版の切り替えをまたぐチケット（前の版の最後の営業日に出し、次の版の最初の営業日に答える。差し戻して次の版で再申請する。次の版にいない申請者も）。
- フォームの数の境（申請者が入れる数、計算した値、明細の行の数の、受け付ける最後の値と、断る最初の値）。
- 条件で要るフィールドを空にした申請。見せているあいだ必須のフィールド（`shown when` があり、`optional` の無い物）と `required when` のフィールドを一つずつ、それに値を入れた最初のシナリオのフォームから外して出す。条件が成り立てば断り（`form`）、成り立たなければ受け付ける。
- 経路の例の行（シナリオのフォームに重ねる）。

`--list` は、同じベクタを、一行ずつ言葉で書く。

```text
$ ritsu ringi vectors examples/leave.ringi --list      （192 行から抜いた）
scenario a_long_paid_leave, line 47: submit by e1203 at 2026-11-16 09:00 -> manager, in progress
scenario a_long_paid_leave, after it ended: comment by e1203 at 2026-11-19 10:00 -> refused (state)
scenario a_long_paid_leave, after line 47: approvals: approve by e1101 at 2026-11-16 09:00 -> head, in progress
scenario a_long_paid_leave, after line 47: the requester: approve by e1203 at 2026-11-16 09:00 -> refused (person)
scenario a_long_paid_leave, after line 47: made again, with fields made empty: resubmit by e1203 at 2026-11-17 09:00 -> refused (form)
scenario a_long_paid_leave, after line 47: made again, with fields made empty: resubmit by e1203 at 2026-11-17 09:00 -> manager, in progress
scenario a_long_paid_leave, after line 47: sent back: comment by e1203 at 2026-11-16 09:00 -> with requester
scenario a_long_paid_leave, after line 47: clocks: remind at 2026-11-17 08:59 -> refused (clock)
scenario a_long_paid_leave, after line 47: clocks: comment by e1101 at 2026-11-17 09:00 -> refused (clock)
scenario a_long_paid_leave, after line 47: comments: comment by e1101 at 2026-11-16 09:00 -> manager, in progress
scenario a_long_paid_leave, after line 47: comments: approve by e1101 at 2026-11-16 09:00 -> refused (comment)
scenario a_long_paid_leave, after line 47: a reason asked, and none given: reject by e1101 at 2026-11-16 09:00 -> refused (comment)
scenario a_long_paid_leave, after line 47: a reason asked, and none given: send back by e1101 at 2026-11-16 09:00 -> refused (comment)
scenario a_long_paid_leave, after line 47: a reason asked, given in a character that is no white space: send back by e1101 at 2026-11-16 09:00 -> with requester
scenario a_long_paid_leave, after line 47: sent back to the limit: send back by e1101 at 2026-11-18 09:00 -> refused (limit)
scenario a_leave_sent_back_with_a_comment, line 61: comment by e1101 at 2026-11-16 11:00 -> manager, in progress
scenario a_long_paid_leave, after line 48: made on the revision before: approve by e1101 at 2026-11-16 17:00 -> refused (stale)
scenario a_long_paid_leave, after line 48: no revision: approve by e1101 at 2026-11-16 17:00 -> refused (stale)
scenario a_long_paid_leave, after line 48: a comment, which says no revision: comment by e1101 at 2026-11-16 17:00 -> head, in progress
scenario a_long_paid_leave, made on 2026-12-28 (closed from 2026-12-29): late at 2027-01-06 00:00 -> manager, in progress
scenario a_long_paid_leave, made on 2026-12-28 (closed from 2026-12-29): remind at 2027-01-07 09:00 -> manager, in progress
scenario a_long_paid_leave, made with a comment: submit by e1203 at 2026-11-16 09:00 -> refused (comment)
requester e0001: submit by e0001 at 2026-11-16 09:00 -> refused (submit)
proxy e1201 for e1203: submit by e1201 at 2026-11-30 09:00 -> refused (person)
proxy e1201 for e1203: submit by e1201 at 2026-12-01 09:00 -> manager, in progress
proxy e1201 for e1203: a comment: comment by e1101 at 2026-12-01 09:00 -> manager, in progress
proxy e1201 for e1203: taken back by the one who made it: withdraw by e1201 at 2026-12-01 09:00 -> with requester
proxy e1201 for e1203: sent back: comment by e1203 at 2026-12-01 09:00 -> with requester
across the versions of the organization: resubmit by e9001 at 2027-04-01 09:00 -> refused (person)
edge of days, going up: 20: submit by e1203 at 2026-11-16 09:00 -> manager, in progress
edge of days, going up: 21: submit by e1203 at 2026-11-16 09:00 -> refused (form)
scenario a_long_paid_leave, with reason left empty: submit by e1203 at 2026-11-16 09:00 -> manager, in progress
```

五つの例のベクタは、経費 237 本、購買 435 本、アカウント 190 本、契約 441 本、休暇 192 本（合わせて 1,495 本）で、日本語の名前の版も同じ本数である（同じ組織のデータを読み、代理の行が両方の版の名前を挙げる。5.8）。シナリオは英語の版に 19 本ある。承認で終わる物のほかに、次の四つの進み方を、どれも一つは持つ。

| 進み方 | 例とシナリオ | 見せること |
|---|---|---|
| 却下 | 購買の `the_board_votes_it_down` | 取締役会の議決で、一人が承認し、三人が理由を書いて却下する。三人目の却下で議決に届かなくなって終わり、押さえた予算を取り消す |
| 差し戻しと出し直し | 休暇の `a_leave_sent_back_with_a_comment` | 上長がコメントで尋ね、コメントを添えて差し戻し、申請者の問い返しに答え、出し直した申請を承認する |
| 取り下げ | 経費の `a_trip_called_off` | 決裁を待つ出張を、申請者が理由を書いて取り下げ、手元に戻った申請をやめる（`discard`）。予算は取り下げたときに取り消す |
| 期限切れ | 契約の `a_draft_that_does_not_come` | 法務が相手の会社の案を求めて差し戻し、10 営業日のうちに出し直されないので、再申請の期限で切れる |

経費は、ほかに購入のシナリオと出張のシナリオを二つ持つ（エージェントが決める日帰りの出張と、エージェントが人へ渡す出張）。購買とアカウントは、上長のステップがあとのステップを待つシナリオを持つ（購買は三つ、アカウントは一つ）。購買は、前払いの割合が財務の確認を足すシナリオ（`half_paid_in_advance`）も持つ。契約は、法務がリスクを書かない秘密保持契約のシナリオ（`a_non_disclosure_agreement`。`risks` は `shown when kind is not nda`）と、部分が止まり、管理する人が承認者を足して進むシナリオ（`a_lawyer_after_the_team_shrinks`。5.9）も持つ。休暇は、長い有給休暇のシナリオと、部長が理由を書いて却下するシナリオ（`a_leave_rejected_with_a_reason`。休暇は却下と差し戻しに理由を求める。7.2）も持つ。出張のフォームがあるので、例の五つの行は、どれもベクタになる。

終了コードは、書いたら 0、ファイルが検査を通らないか、シナリオが書いたとおりに進まなければ 1（何も書かない）、シナリオが無いか、組織のデータが無ければ 2 である。断りの文（`text` は英語、`text_ja` は日本語）と履歴の `note` は人が読む文で、`--lang` によらず同じに書く（ベクタも `run --format json` も、どの言語の実行でも同じ物になる）。生成したコードとサーバーは、断りを種類（`refused`）と場所（`at`）まで同じにし、文は比べない。履歴の `note` は語まで同じにし、`comment` は渡された文のとおりにする（12.4 の「適合」）。

申請の種類の入口 `worked`（12.2、13.7）のベクタは、`ritsu gen --target requests` が `vectors/worked/<名前>.jsonl` に書く（`ritsu ringi vectors` は書かない）。一行が一つの組で、キーは `from`（どこから作ったか）、`input`（`{"form": …, "edited": … | null}`）、`answer`（参照の答え `{"fields": …}`）である。チケットの機械のベクタと形が違うので、同じディレクトリに置かない。サーバーの試験は種類のベクタを `vectors/*.jsonl` で読み、ファイルの名前を種類の名前にするので、並べて置くと、`worked` のベクタを種類のベクタと読み違えるからである。

元は、その種類のチケットの機械のベクタのうち、フォームを渡す物である。申請が渡すフォーム（`submit` の `form` から計算するフィールドを除いた物。境の値のベクタのフォームも入る）と、承認者の直し（`edit`）の前のチケットのフォームとその直しの組である。そのうえで、最初の申請のフォームから次を作る。

- 何も入れていないフォーム（`{}`）。
- フィールドごとに、空にした物、選択の値の一つずつと選択肢に無い値、数と日付の範囲の両端とその一つ外、真偽の true・false・無い物、空白だけの文。
- 明細は、行が無い物、一行だけの物、上限より一行多い物と、最初の行のセルごとに、空、範囲の両端とその一つ外、空白だけの文。
- 承認者が直せるフィールドがあれば、どのフィールドも渡さない直し（`{}`）と、フィールドごとに上と同じ値を渡す直し（null はフォームの値を残す）。

同じ入力は一度だけ書く。例の五つの種類で、アカウント 17 本、契約 42 本、経費 46 本、休暇 24 本、購買 69 本で、日本語の名前の版も同じ本数である（合わせて 396 本）。経費の最初のシナリオの一行は次のとおりである。

```text
{"from":"scenario a_joint_purchase, line 100","input":{"form":{"category":"purchase","items":[{"what":"Server","price":800000},{"what":"Storage","price":400000}],"purpose":"A server and its storage for the test lab","used_on":"2026-11-16","receipt":"receipt.pdf"},"edited":null},"answer":{"fields":{"amount":{"value":1200000},"trip_to":{"shown":false},"receipt":{"required":true}}}}
```

## 11. ページ

`ritsu ringi doc` と `ritsu ringi diff` が書く。英語と日本語（`--lang ja`）、Markdown と HTML（`--format markdown|html`。ほかの言語の `doc` と同じフラグ）。二つの形式は同じことを書き、HTML は一つのファイルで、外から何も読まない。ページの言葉は、承認する人と管理する人が読む言葉にし、規則の行や出力の名前、下ろしたファイルの名前を出さない。フィールド、選択の値、ステップ、承認者の決め方は、ページの言語で書いた題で呼ぶ。ファイルがほかの言語で書いてあれば `labels` の語、それも無ければ書いた名前で呼ぶ。

数は、画面と同じく ritsu の `formatSteps` の書き方で書く（`1.25円`、`31.25%`。五桁以上の整数の部分は三桁ごとにカンマで区切る）。ページは、申請を出す人と承認する人が画面と並べて読むからである。診断は、ファイルを直す人が読むので、ファイルの書き方のまま書く（`ritsu_units::written`、`1_000_000JPY`）。

ページは、ファイルをルートからのパスで書く。`./f`、`x/../f`、絶対パスのどれで渡しても、同じページになる。描いた日に左右される物は書かない。例の日付も、ファイルとデータの日付から決める。

### 11.1 経路を一枚で見せるページ

`ritsu ringi doc <file>` は、申請の種類一つを一枚に並べる。節は次の順である。

| 節（英語の見出し） | 日本語の見出し | 書く物 |
|---|---|---|
| 頭（申請の種類の題） | 同じ | 説明、ファイル（ルートからのパス、名前、版、SHA-256 の短い形）、受け持つチーム、効き始める日、読む組織の版 |
| When it can be used | 使える期間 | 最初の日と終わりの日と、それぞれを決めている物（9.6） |
| Who may submit it | 出せる人 | `submitted by` の行を文で。組織の版ごとの人数（使える期間に入る版だけ。`check` の `ok` の下の数と同じ）。代理申請 |
| Form | フォーム | フィールドごとの型、受け付ける値、必須、表示（条件は文で）。計算するフィールドは、書いた範囲と、計算の届く範囲（経費の `amount` は 1 円から 10,000,000 円を受け付け、計算は 1 円から 20,000,000 円まで届く）。明細は行の数。値の無いときの読み方（4.9）を一つの注で |
| Route | 経路 | 表の形は、一行が一つの場合で、通るステップを「上長（一次） → 部長と経理（二次） → 本部長（決裁）」の形で並べる。ステップの形は、ステップごとに答える人と来るとき。共有の経路は、渡す条件と一緒に、その経路のステップを並べる |
| How the approvers are chosen | 承認者の決め方 | 決め方ごとの文。使う規則の出力で決めるステップは、規則が出力しうる決め方と一緒に |
| Deadlines and reminders | 期限と催促 | `every stage` とステップごとの行の文（期限を過ぎたときと、答える人がいないときも）。例の日に始まるステップの催促、期限、上へ回したあとの期限（時刻は UTC からのずれと一緒に）。いちばん長い営業日（7.6） |
| Rules of a ticket | チケットの決まり | 自己承認、同じ承認者、差し戻し、出し直し、取り下げ、承認の取り消し、保留、承認者が直すフィールド、却下、承認者を足すことと引き継ぎ |
| Comments | コメント | コメントの決まり（7.2） |
| Proxies | 代理 | 代理のしくみと、組織のデータにある、この申請の種類の代理 |
| Books and flows | 帳簿と flow | `on` の行ごとに、いつ何をするか |
| Claims | 主張 | 主張ごとに、それが言うこと |
| Examples | 経路の例 | `examples` の表 |
| Scenarios | シナリオ | シナリオごとの申請者、フォーム、イベントの表 |

例の日は、使える最初の日から数えて最初の、次の日が休みの営業日である（11.3 の「休みの前の日に出し」と同じ選び方）。経費では 2026-04-03（金）になる。

経路の表は、下ろした `route.rule` の `doc` からではなく、申請の種類が書いた表から作る。下ろした規則は、ステップごとに列を持ち、セルは決め方の ASCII の名前で、その `doc` のページは下ろしたファイルについてのページになる。そのページのセルを承認者の文言に替えると、rulec がページを描く形に頼ることになり、描き方が変われば壊れる。表が `simulate` と同じ経路を言うことは、`tests/doc.rs` が、場合ごとに範囲の両端で確かめる。

### 11.2 変更の前後を見せるページ

`ritsu ringi diff old.ringi new.ringi` は、同じファイルの二つの版を比べ、言うことと答えが違う所を、どの申請の、どのステップに、だれが答えるかで言う。二つは同じ種類で同じ名前のファイルでなければならず、種類か名前が違えば 2 で断る。どちらも検査を通らなければならず、通らない版があれば、その診断を `check` と同じに出して 2 で終わる。違いが無ければ 0、あれば 1 で終わる。

申請の種類では、まず経路を比べる。二つの版の下ろした `route.rule` を `Rules::diff` で比べ（`rulec diff` と同じ歩き方）、ステップの違う区間ごとに、区間をフォームのフィールドの題と値で、ステップをその決め方の文で言う。ある版に無いステップ（足したか除いたステップ）は、その版では来ないステップとして読む。除いたステップは、歩き方の区間に、古い版の出力として出る。足したステップは、新しい版にだけある出力で、歩き方は宣言の違いとして言うだけで、区間には出さない。そこで、新しい版にだけあるステップがあれば、二つの経路を逆の向きにも比べ、そのステップの区間を、来なかったステップが来るようになる区間として言う。足したステップも、除いたステップと同じく、経路の節で言う。同じステップを同じに変え、一つの入力のほかは同じ値を取る二つの区間は、一つの区間にまとめる。歩き方が区間を切るのは、変わらないほかのステップが違う所でもあるからで、まとめないと同じ変更が何行にも分かれる。区間を確かめきれなかったとき（予算を超えた、比べられない形だった）は、その区間と理由を並べ、「ほかの申請は、どれも同じステップに進みます」とは言わない。予算は `--budget <n>` で、`rulec diff` と同じに数える。

組織のデータを読む申請の種類では、組織の版ごとに、承認者が変わる申請者を並べる。版の最初の日に、両方の版で、フォームを渡さずにすべての申請者の `simulate` を走らせ、区間のステップに答える人が前と後で違う申請者を、人の名前で並べる。版の最初の日は、両方の版が使える期間に寄せる。答える人の比べ方は、決め方ごとの人の ID、何人の承認が要るか、その日の代理、答える人がいないときにどこへ回るか（上の人の ID、`then` の決め方の人の ID、飛ばす、待つ）である。決め方の名前は比べない。別の決め方が同じ人に来るなら、承認者は変わらないからである。人とチームの名前も比べない。名前だけを変えた版で、承認者が変わったと言わないためである。ステップの決め方の一部だけが変わったときは、変わった決め方だけを書く。端末の文とページは、版ごとに 10 行まで書いて「ほかに N 件あります」と添え、JSON はすべてを持つ。

経路が変わらなくても、答える人は変わりうる。決め方を書き直したとき（`security = team it_security` を `team it_operations` にした）と、読むデータが変わったとき（人がほかのチームに移った）である。これは「承認者」の節に、同じ比べ方で並べる。

経路のほかは、二つの版のページ（11.1、11.6）を節ごとに比べる。表の一行は最初のセルで対応させ、違うセルを「何が、前、後」で並べる。文は、同じ見出しの下で、無くなった文と増えた文を順に組にする。ページが版の言うことのすべてなので、比べる物もページと同じだけになる。経路の例とシナリオは比べない。一つの版が答えることを言うもので、違いは経路の区間がすでに言っている。

最後に、進行中のチケットへの効き方を言う（7.7、8.7）。版を上げたなら、チケットは出したときの版で終わり、変更は新しい版で出す申請に効く。版が同じなら、進行中のチケットと変更のあとの申請を見分けられないので、版を上げるように言う。申請の種類が変わらず、読むデータの変更で承認者が変わるときは、承認者が決まったステップはそのまま進み、変更は次に始まるステップから効く、と言う。チケットが持つのは申請の種類の版で、申請の種類が読むファイルの版は持たない。そこで、組織とマスタの変更には、次に始まるステップから効くと言い（マスタでは、チケットはフォームで選んだ行のキーを持ったままである）、共有の経路の変更には、共有の経路の版ではなく、それを使う申請の種類の版をそれぞれ上げるように言う。チケットが持つのは申請の種類の版だからである。

経費の、100,001 円から 1,000,000 円の経費の行の `precheck` を空にして版を 2 にした写し（`expense.v2.ringi`、経路の例の行も同じに直したもの）を `examples/` に置いて比べると、次のようになる（承認者の行は、2026-04-01 の版の 3 行だけを載せ、ほかは略す）。

```text
$ ritsu ringi diff expense.ringi expense.v2.ringi
  The request `expense` (v1 → v2): `crates/ringi/examples/expense.ringi` → `crates/ringi/examples/expense.v2.ringi`
Route:
  Requests where “Category” is “Expenses” and “Amount” is from 100,001 JPY to 1,000,000 JPY: the stage “precheck” (check) no longer comes.
  In the organization from 2026-04-01, the approvers change for 24 of 24 requesters.
    e0001 (President (fictional)), precheck: check (manager when the agent doubts): no head is above the requester; it goes to `board`: one of e0201 (Director 1), e0202 (Director 2), e0203 (Director 3), e0204 (Director 4) → does not come
    e0101 (Head of Sales Division), precheck: check (manager when the agent doubts): e0001 (President (fictional)) → does not come
    e0102 (Head of Corporate Division), precheck: check (manager when the agent doubts): e0001 (President (fictional)) → does not come
…
  And 14 more: `--format json` lists them all.
  In the organization from 2027-04-01, the approvers change for 23 of 23 requesters.
…
  And 13 more: `--format json` lists them all.
  Every other request comes to the same stages.
Tickets in progress:
  A ticket finishes with the version of the request it was submitted with: tickets submitted with version 1 finish as version 1 says, and what changes here applies to requests submitted with version 2.
```

`--lang ja --format markdown` のページでは、区間は見出しになり、「「種別」が「経費」で、「合計」が100,001円から1,000,000円までの申請：ステップ「事前の確かめ」（エージェントの確かめ）が来なくなります」と言う。ステップと決め方は `labels` の題で呼ぶ。

申請の種類のほかのファイル（共有の経路、組織、マスタ）は、ページの節を比べる。共有の経路の経路も、ページの経路の節として比べる。下ろした規則は、使う申請の種類が渡す条件と一緒にしか答えないからである。

出す形は、端末の文（既定。走らせた言語で）、Markdown のページ（`--format markdown`）、一つのファイルの HTML（`--format html`）、JSON（`--format json`、16.2）である。ページと JSON の言語は `--lang` が決める。三つは同じことを言い、`tests/diff.rs` が確かめる。

### 11.3 期限がいちばん長くなる場合

`ritsu ringi doc --longest` は、7.6 で数えたいちばん長い並びを、出した日を選んで日付つきで見せる（休みの前の日に出し、どのステップも期限まで待って上へ回り、差し戻しを上限まで受け、再申請を期限の日にした場合）。期限を変えると何日短くなるかを、管理する人が読めるようにする。

ページは、7.6 の数の内訳、いちばん長い並びのステップ（答える人、期限、期限を過ぎたとき、長くて何営業日か）、出した日からの日付の表、何を変えるとどれだけ短くなるかの表を並べる。日付は、機械の時計で歩く。どのステップも、上へ回せるだけ回したあと、最後の期限の直前に承認する。差し戻しは最後のステップで上限の回数まで受け、申請者は出し直しの期限の日に出し直す。保留は最初のステップで使い切る。日の終わりの期限が休みの前の日に切れると、ステップは休みの日に上へ回り、その分だけ短くなる（7.6 の数は上限である）。そのときは、歩いた営業日が数より少ないわけを注で言う。短くする表は、いちばん長い並びの期限の営業日、差し戻しの回数、出し直しと保留の営業日を一つずつ減らした場合を、7.6 と同じ数え方で数え直す。ほかの並びがいちばん長くなるなら、その並びで数える。

出す日は `--on <日付>` で選べる。申請の種類が使えない日なら 2 で断る。渡さなければ 11.1 の例の日である。`--on` は `--longest` と一緒にだけ渡せる。`--longest` を受けるのは申請の種類だけで、ほかのファイルには 2 で断る。

### 11.4 組織の版の前後

`ritsu ringi diff --versions <日付> <日付> <file>` は、一つの申請の種類を、二つの日に効いている組織の版で読み比べる。予約した版を置いたら、切り替えの前に読む。並べる物は次のとおりである。

- **承認者**：両方の版にいる申請者のうち、答える人が変わる申請者を、ステップごとに前と後で（比べ方は 11.2 と同じ）。
- **申請者**：片方の版にだけいる申請者。
- **出せる人**：`submitted by` を書いた申請の種類で、両方の版にいる人のうち、出せるようになる人と出せなくなる人。
- **答える人がいないステップ**：答える人が見つからない申請者の数が二つの版で違うステップを、そのときどうなるか（上かほかへ回る、飛ばされる、承認者を足すまで待つ）に分けて、前と後の数で。
- **進行中のチケット**：切り替えをまたぐチケットがどうなるかを一文で（8.7）。承認者が決まったステップはそのまま進み、切り替えの日からあとに始まるステップは新しい版で承認者が決まる。待っている承認者が組織からいなくなれば、管理する人が引き継ぐ。

断る場合と文は次のとおりである。どれも 2 で終わる。申請の種類でないファイルには「`--versions` が比べるのは申請の種類の承認者です」。組織のデータを読んでいなければ、組織のページと同じ理由の文。どちらかの日に効く版が無ければ「2025-04-01 に効いている組織の版がありません（最初の版は 2026-04-01 から）」。`--at` は版を一つだけ読むので、一緒には渡せない。二つの日が同じ版にあれば、「どちらの日も 2026-04-01 からの版にあります。変わるものはありません」と言って 0 で終わる。変わる物があれば 1 で終わる。

経費を 2026-04-01 と 2027-04-01 で比べると、e1201 と e1203 の課がなくなって上長が営業一部長に変わり、法務部長が抜けて弁護士の上長が管理本部長に変わる。`tests/golden/diff/versions.expense.txt` が、その全文である。

### 11.5 `explain`

`ritsu ringi explain E502` は、台帳の文と、それが出る小さな例を見せる。`ritsu explain E911` は ritsu のコードを同じように見せる。

### 11.6 共有の経路、組織、マスタのページ

`ritsu ringi doc` は、申請の種類のほかの三つのファイルにもページを書く。検査を通るファイルは、どれも管理する人が読むものだからである。

- **共有の経路**：頭、使える期間、受け取る物の表、経路、承認者の決め方、期限と催促（11.1 と同じ書き方）、ステップを続けて通ったときのいちばん長い営業日、使う申請の種類。使う申請の種類は、プロジェクトの `.ringi` のうち、`use route` の行がこのファイルを読むものを、題と名前とルートからのパスで並べる。プロジェクトのルートは `check` と同じに決め（`--root`、無ければファイルの上で `.git` か `ritsu.mod` を持つディレクトリ）、ファイルは `ritsu check` と同じたどり方で拾う。どのファイルが何を読むかは、プロジェクトの索引が読むのと同じ関数（`ports::uses_of`）で読む。プロジェクトが見つからなければ、使う申請の種類はこのファイルからは分からず、申請の種類のページが渡す条件と一緒に経路を見せる、とだけ書く。経路を変えたらどの申請の種類に効くかは、管理する人がまず知りたいことである。
- **組織**：役職とチームの種類、データの版ごとの期間とチームと人の数、版ごとのチームの表（種類、上のチーム、長、人数）、版ごとの人の表（名前、チーム、役職、主に属するチームか）、代理の表。代理の表は、代理を書いたファイルを、組織のファイルがデータの場所を書いた形に続けて書く（`../org/proxies.csv`）。ファイルをどう渡しても、走らせた場所によらず同じページになる。人について書くのは、版を確かめるのに要る物（名前、役職、チーム、代理）だけで、データにあるほかの物（メールのアドレス）は書かない。
- **マスタ**：マスタごとに、行を見分けるフィールド、フィールドの表（型と受け付ける値）、行の表（選べる最初の日と最後の日と一緒に）。

`--longest` は、この三つには 2 で断る。

## 12. 生成

### 12.1 チケットの機械のコード

チケットの機械は、kaname のモジュール `ticket` として出す。機械の本体は kaname で一度だけ書き（クレートの `src/lower/ticket.kn`）、申請ごとのつなぎは ringi が `.kn` に下ろす。どちらも kaname が TypeScript、Python、Go、Rust にする。機械の本体の型は、参照インタプリタの型（`src/engine/state.rs`、`event.rs`、`effects.rs`、`part.rs`）を写した物で、ベクタの JSON と違うのは、kaname が受け付けない語と、kaname の型で書けない形だけである（`src/lower/shape.rs` の一か所が持つ）。申請ごとに違う物は型の引数（フォームの `F`、イベントの値の `G`）で受け、申請の種類が言うこと（決め方、ステップ、`when empty`、使う日の範囲、オフセット）は、つなぎがデータで渡す。

理由：規則、日付、帳簿、gate と同じパッケージで、同じプロセスから呼べる。Rust から Wasm にでき、画面の側でも同じ決まりが動く。kaname の total と X2〜X6 がつなぎにかかり、`property` と `ritsu gen --target lean` で性質を Lean に出せる。

その代わり、kaname の決まりに合わせて書く。表に書くべき判断を `if` で選ぶと W501、範囲は `if` でしか狭められず（W301、E401）、再帰は `diverges` になり、時刻の型は無い。機械の本体は、判断を関数の呼び出しに分け、`if` でなく `match` で選び、組織をたどることを `reduce(numbers(1, 30), …)` で書いて、どの関数も total にした（`ritsu kaname check src/lower/ticket.kn` は `ok src/lower/ticket.kn: 256 functions, total`）。時刻は数（秒）で渡し、時刻の計算は koyomi の `at` と `CalendarFacts` のオフセットで、下ろした `.cal` がする。

ほかに三つの形を比べて、捨てた。

| 形 | 中身 | 良い所 | 捨てた理由 |
|---|---|---|---|
| ringi が出力先ごとに書く | ringi が TypeScript、Python、Go、Rust のコードを直接書く | 書き方の自由がある | 生成器を四つ持ち、出力先のあいだで食い違いうる。total の確かめが無い |
| データだけを出す | 経路を JSON の表で出し、サーバーが解釈する | 生成が小さい | 意味をサーバーを書く人がもう一度書く。食い違いを見つける手立てが無い |
| rulec の機械 | チケットの進み方を rulec の `machine` に下ろす | 機械の検査（終わりの状態から出ない、承認は却下のあとに来ない）がそのまま使え、試しでも通った（変異は E124、E126 で、短い呼び出しの並びを示した） | 承認者の集合、人ごとの答え、ステップの数が申請ごとに違い、状態が小さな有限の集合にならない |

rulec の機械は、ringi が出す診断の言い方（短い呼び出しの並び）の手本として使う。

ringi の参照インタプリタ（10.1）と kaname のモジュールは、同じ意味を二つの形で書くことになる。ベクタ（10.4）と Lean（17 章）で突き合わせる。rulec の参照インタプリタと、rulec が書く 12 の言語のコードの関係と同じである。

`ritsu gen` は、置いたつなぎと機械を、プロジェクトのファイルと同じに kaname で検査する（12.3）。どちらも ringi が書くコードなので、kaname が何か言えば、警告でも、total でない関数でも ringi の不具合で、申請の最初の行の E201 にして止まる（9.2）。申請を書く人に kaname の言葉は見せず、注には kaname のコードと行だけを書く。2026-11-17 のモジュール `ticket` は関数 256 個で、例の 10 の申請と機械の試験の 16 の申請のつなぎと一緒に、どれも total で警告が無い（`crates/ritsu/tests/gen_requests.rs`。四つの出力先で、全部のベクタが参照インタプリタと同じ答えと同じ断りの文を返す）。

```text
$ ritsu check src/lower/ticket.kn
ok src/lower/ticket.kn: 256 functions, total
ritsu check: 1 file (kaname 1): all pass; borders between the languages: 1 checked, 0 undecided
```

total に書くために、機械の数は型に上限を持つ。申請の種類の版、回（`round`）、差し戻しと承認者の直しの回数は 1,000,000、することの通し番号は 1,000,000,000、上へ回した回数は 255、保留の営業日は 100,000 までである。どれも実際に届く数ではない。約束の JSON がこれを越える値を持てば、入口は言語のエラーで返す（12.2）。一つのイベントで過ぎるステップの数には上限を置かない。機械の一つの動きは、ステップの数と部分の数の和に 2 を足した回数まで回り、一回でステップを一つ過ぎるか、終わる（待つ部分のために足すステップは、その部分がそこで決まるので、この回数に収まる）。

つなぎが計算した値を書いた範囲と比べることと、生成するコードが `values.rule` を読むことは 4.4 に書いた。

### 12.2 申請の種類一つに書き出す物

`ritsu gen --target typescript`（python、go、rust も）は、パッケージの中に次を書く。

- 下ろした規則と日付のコード（rulec と koyomi が書く。`expense_route`、`expense_form`、`expense_values`、`expense_choices`、`expense_next_up`、`expense_terms`）と、つなぎが営業日を数える日付のコード（`expense_open`）、機械が日の番号から日付を作る日付のコード（`ticket_days`）。
- 申請ごとのつなぎのコード（kaname が書く。`<ファイル>.lowered/<名前>.kn` に下ろした、申請の種類の ASCII の名前のモジュール）。`start(submit, organization, now)`、`next(ticket, event, organization, now)`、フォームの検査と自動計算の関数。`submit` は、チケットの ID、申請者、代理申請ならその人、フォームの値で、参照インタプリタの `Machine::start` が受け取る物と同じである。
- 帳簿のクライアント、gate のコード、flow のコード（chobo、sekisho、dandori が書く。今と同じで、`ritsu gen` に渡したファイルの物が入る。Rust のパッケージには gate と flow のコードが無い）。
- 申請の種類の一覧（2026-11-15）。パッケージに入っている申請の種類ごとに、ASCII の名前、版、`start`、`next`、`worked` の JSON の入口を、名前の順に並べ、約束の版（12.4）と、入口のエラーの文の言語（`en` か `ja`。パッケージを書いた `ritsu gen --lang` の言語）を添える。入口は約束の形の JSON（12.4）を受け取り、約束の形の JSON を返すので、サーバーは申請の種類ごとのコードを持たずに、この一覧だけで全部の種類を動かせる。`worked` は、途中まで埋めたフォームと承認者の直し（`{"form": …, "edited": … | null}`）を受け取り、条件つきのフィールドを見せるか、必須か、計算するフィールドの値を、画面と同じ決め方（13.7）で答える（`{"fields": …}`。形は 12.4）。チケットも組織も時刻も読まない。何も計算せず、条件も無いフォームの種類は `{"fields":{}}` を答える（受け取った物が約束の形かは確かめる）。`next` が断ったイベント（7.2）は、答えの JSON の `err` で返す。約束の呼び出しでない物（型の値でない JSON、機械の数の上限（12.1）を越える値）は、入口のエラーで返す。四つの出力先が同じ一つの文で、どの引数のどの場所の値が、何で、何でなければならないかを言う（下の「入口のエラー」）。フォームの範囲と選択の外の値は約束の呼び出しで、申請の断り `form` で返す（ベクタのとおり）。`.ringi` の無いプロジェクトのパッケージには書かない。

| 言語 | ファイル | 一覧と入口 |
|---|---|---|
| TypeScript | `ticket_kinds.ts` | `contract`、`lang`、`kinds: readonly Kind[]`。`start(submit: string, organization: string, now: bigint \| number): string`、`next(ticket: string, event: string, organization: string, now: bigint \| number): string`、`worked(input: string): string`。約束の呼び出しでなければ、つなぎの `KanameError`（`kind` は `input`、`message` が文、`path` が場所）を投げる |
| Python | `<パッケージ>/ticket_kinds.py` | `CONTRACT`、`LANG`、`KINDS: tuple[Kind, ...]`。`start(submit: str, organization: str, now: int) -> str`、`next(ticket: str, event: str, organization: str, now: int) -> str`、`worked(input: str) -> str`。約束の呼び出しでなければ、つなぎの `KanameError`（`kind` は `"input"`、`message` が文、`path` が場所）を投げる |
| Go | `ticketkinds/ticketkinds.go`（パッケージ `ticketkinds`） | `Contract`、`Lang`、`Kinds []Kind`。`Start func(submit json.RawMessage, organization json.RawMessage, now int64) (json.RawMessage, error)`、`Next func(ticket json.RawMessage, event json.RawMessage, organization json.RawMessage, now int64) (json.RawMessage, error)`、`Worked func(in json.RawMessage) (json.RawMessage, error)`。約束の呼び出しでなければ、`error` に `*kaname.KanameError`（`Kind` は `"input"`、`Message` が文、`Path` が場所。`errors.As` で取り出す）を返す |
| Rust | `ticket_kinds.rs` | `CONTRACT: u32`、`LANG: &str`、`KINDS: &[Kind]`。`start: fn(submit: &str, organization: &str, now: i64) -> Result<String, EntryError>`、`next: fn(ticket: &str, event: &str, organization: &str, now: i64) -> Result<String, EntryError>`、`worked: fn(input: &str) -> Result<String, EntryError>`。クレートの入口（kaname の `glue::door::answer`）を通し、約束の呼び出しでなければ `Err` に `EntryError`（`path` が場所、`message` が文）を入れる |

`now` は秒の数（7.5）。言語（`lang`、`LANG`、`Lang`）は、入口のエラーの文を、サーバーがそのまま出すか、その言語の物として読むためにある。断り（7.2）は、どの言語のパッケージでも英日の両方を持つ（`text`、`text_ja`）。名前は、生成物に言語の名前を出さず、各言語で広く使われているライブラリの名前と重ならない物にした。Rust の一覧は、種類ごとの入口を `start_<名前>`、`next_<名前>`、`worked_<名前>` の関数で持つ。大文字を含む名前の種類（`capsDays`）の入口は、Rust が名前の形を警告するので（`non_snake_case`）、`#[allow(non_snake_case)]` を付けて書く。

**入口のエラー**（2026-11-16）。入口は、関数を呼ぶ前に引数を全部読み、値を一つ読むたびに JSON の型を確かめる。型の値でない JSON は、最初に見つけた一か所を一つの文で言う。読み方は kaname のハーネスの物で（kaname の DESIGN 16.4。Rust は `glue::door`、36.11）、四つの出力先で同じ文になる。文は `ritsu gen --lang` の言語で書き、一覧の `lang`（`LANG`、`Lang`）がその言語を言う。

- 場所は、引数の名前から `.` と `[i]` でつなぐ（`submit.form.days`、`organization.members[0].primary`）。直和型の値の中は、値の名前を挟む（`event.approve.by`）。場所はエラーの `path`（Go は `Path`）にも入るので、サーバーは文を読まずに、どの引数のどこかを引ける（kaname の DESIGN 4.4）。
- 渡された物は、数を書いたまま（`2.5`）、文字列を JSON の書き方で 40 字まで（越えれば `…`）、`true`・`false`・`null` をそのまま言い、配列とオブジェクトは種類だけを言う。直和型の値に渡したオブジェクトは、キーで言う（`an object with the key "aprove"`、`an object with 2 keys`）。
- 要る物は、型で言う（文字列、整数、`true` か `false`、`YYYY-MM-DD` と書いた日付、配列、オブジェクト、`[キー, 値]` の組）。列挙は語を全部挙げる。直和型（イベント）は、名前だけで書ける値（時計のイベント）と、オブジェクトのキーにできる名前を全部挙げる。渡された物がオブジェクトなら、キーにできる名前だけを挙げる。
- 整数は、`20`、`20.0`、`2e1` のどれも 20 と読む（約束の形の内）。int64 に入らない整数は、書いた桁の数によらず int64 の外と言う。機械の数の上限（12.1）を越える値は、関数の入口の範囲の文で言う。どれも、ベクタの答えは変えない。
- 文字列は、Unicode として正しいかを確かめる。対になっていないサロゲートの書き方（`"\ud800"`）は、どの出力先も、別の字に置き換えずにそのまま読み、正しくない文字列として断る。文がそういう文字列を見せるときは、その字を U+FFFD で見せる。
- 同じキーが二度あるオブジェクトは、後の値を読む（JavaScript の `JSON.parse` と同じ）。
- 読まないキー（フォームに無いフィールドなど。12.4 の表）は、今までどおり読まない。
- `worked` の文は、関数の名前を、つなぎの関数の名前 `worked_out`（何も計算しない種類は `worked_none`）で言う。つなぎには、フォームの計算をまとめて返す関数 `worked` が前からあるためである。範囲の外の値と選択肢に無い値は、`worked` では入口のエラーでなく、決まらない値（null）として答える（12.4）。

休暇の例で、四つの出力先が返す文（`crates/ritsu/tests/gen_requests.rs` の `json_out_of_the_contract_is_said_alike_in_the_four_languages`。約束の外の 38 の呼び出しと、約束の内の 8 つを、英日で）：

| 渡した物 | 英語 | 日本語（`--lang ja`） |
|---|---|---|
| キーが無い | The argument `submit.requester` of `start` is missing, and must be a string | 関数 `start` の引数 `submit.requester` がありません。文字列が要ります |
| 整数の所に文字列 | The argument `submit.form.days` of `start` is "three", and must be an integer | 関数 `start` の引数 `submit.form.days` が "three" です。整数が要ります |
| 整数の所に小数 | The argument `submit.form.days` of `start` is 2.5, and must be an integer | 関数 `start` の引数 `submit.form.days` が 2.5 です。整数が要ります |
| 日付の所に、書き方の違う日付 | The argument `submit.form.first_day` of `start` is "2026-4-1", and must be a date written YYYY-MM-DD | 関数 `start` の引数 `submit.form.first_day` が "2026-4-1" です。YYYY-MM-DD と書いた日付が要ります |
| 配列の所にオブジェクト | The argument `organization.teams` of `start` is an object, and must be an array | 関数 `start` の引数 `organization.teams` がオブジェクトです。配列が要ります |
| 真偽の所に文字列 | The argument `organization.members[0].primary` of `start` is "yes", and must be true or false | 関数 `start` の引数 `organization.members[0].primary` が "yes" です。true か false が要ります |
| 約束に無い語 | The argument `ticket.status` of `next` is "done", and must be one of "in_progress", "paused", "with_requester", "approved", "rejected", "withdrawn", "expired" | 関数 `next` の引数 `ticket.status` が "done" です。"in_progress"、"paused"、"with_requester"、"approved"、"rejected"、"withdrawn"、"expired" のどれかが要ります |
| 約束に無いイベント | The argument `event` of `next` is an object with the key "aprove", and must have one key of "resubmit", "approve", …, "flow_failed"（20 の名前） | 関数 `next` の引数 `event` がキー "aprove" のオブジェクトです。キーを "resubmit"、"approve"、…、"flow_failed" のどれか一つだけ持つオブジェクトが要ります |
| 値を持つイベントを名前だけで | The argument `event` of `next` is "approve", and must be one of "remind", "late", "resubmit_late", "pause_ends", or an object with one key of "resubmit", "approve", …, "flow_failed" | 関数 `next` の引数 `event` が "approve" です。"remind"、"late"、"resubmit_late"、"pause_ends" のどれか、またはキーを "resubmit"、"approve"、…、"flow_failed" のどれか一つだけ持つオブジェクトが要ります |
| イベントの中のキーが無い | The argument `event.approve.by` of `next` is missing, and must be a string | 関数 `next` の引数 `event.approve.by` がありません。文字列が要ります |
| JSON として読めない文、値が二つの文 | The argument `submit` of `start` is not one value of JSON | 関数 `start` の引数 `submit` は、一つの JSON の値ではありません |
| Unicode として正しくない文字列（`"e\ud8001"`） | The argument `submit.requester` of `start` is no well-formed string of Unicode | 関数 `start` の引数 `submit.requester` は、Unicode の文字列として正しくありません |
| 同じキーを二度書き、後の値が約束の外（`requester` を、ID のあとに 5 で） | The argument `submit.requester` of `start` is 5, and must be a string | 関数 `start` の引数 `submit.requester` が 5 です。文字列が要ります |
| int64 の外の数（`1e30`、`9223372036854775808`、5,000 桁の整数） | The argument `ticket.round` of `next` is outside int64 | 関数 `next` の引数 `ticket.round` が int64 の外にあります |
| 機械の数の上限を越える値 | The argument `ticket.round` of `next` is 1000001, outside its range >=1 <=1000000 | 関数 `next` の引数 `ticket.round` の 1000001 は、範囲 >=1 <=1000000 の外です |
| 暦に無い日 | The argument `submit.form.first_day` of `start` is 2026-13-40, outside its range >=0001-01-01 <=9999-12-31 | 関数 `start` の引数 `submit.form.first_day` の 2026-13-40 は、範囲 >=0001-01-01 <=9999-12-31 の外です |
| `worked` に、フォームの無い入力 | The argument `input.form` of `worked_out` is missing, and must be an object | 関数 `worked_out` の引数 `input.form` がありません。オブジェクトが要ります |
| `worked` に、整数の所に文字列 | The argument `input.form.days` of `worked_out` is "3", and must be an integer | 関数 `worked_out` の引数 `input.form.days` が "3" です。整数が要ります |

入口で確かめないと、文が言語ごとに違い（Python の `KeyError('requester')`、Go の `interface conversion: …`）、エラーにならずに読み替える物も出る（Python は整数に `"3"` と `2.5` を渡すと 3 と 2 と読み、チームの一覧に `{}` を渡すと空の一覧と読む。TypeScript は真偽に `"yes"` を渡すと真と読み、ステップの期限に文字列を渡すとそのまま持つ。チケットの `status` が無いか約束に無い語なら、TypeScript と Go は申請の断り `state` で答える）。そこで、どれも入口のエラーにした。入口の確かめは、約束の形の JSON への答えを変えない。

`ritsu gen --target requests` は、言語に依らない物を一度だけ `<out>/requests/` に書く（2026-11-15）。ほかの出力先と同じく、`ritsu gen` の一覧（`--format json` の `target` は `requests`）、`--check`、検査が通らなければ何も書かないこと、`ritsu.mod` の `target` の行に入る。どのファイルも `--lang` によらず英語の名前とキーで書き（画面は英語と日本語の二つのファイルにする。13.4）、ベクタのほかは ritsu が生成したファイルの頭を持つ（JSON は最初のキー `//`、OpenAPI は `x-ritsu-generated`、JSON Schema は `$comment`、画面のカタログは `description`、Markdown は最初の行の `<!-- … -->`、`functions.js` は最初の行のコメント）。画面のテンプレートが書くカタログの ID（13.2）を、`--catalog <url>` か ritsu.mod の `catalog` の行で渡さなければ、`requests` は何も書かずに 2 で終わる。ritsu はこの頭で前に書いたファイルを見分け、渡されなくなった申請の種類の説明を、そのベクタ（入口 `worked` のベクタも）と画面と一緒に消す（画面のディレクトリ `a2ui/<名前>/` も、空になれば消す。`--check` では残っていると言う）。

ライブラリの形（`ritsu gen --library`、根の DESIGN 17.16）でも同じである（2026-11-16 に確かめた）。言語ごとのライブラリは、モジュールの申請の種類の全部の一覧を持ち、一覧はつなぎをライブラリの名前で読む（Python は `library python` の行の import の名前の下の `glue`、Go は `library go` の行のパスの下の `glue/<名前>`）。TypeScript のライブラリの入口（`index.ts`）は一覧を `ticket_kinds` として出す。`requests` は言語のライブラリにならず、Cedar と同じく、どのライブラリにも入れずに `<out>/requests/` に一度だけ書く。`library` の行は要らない。`--format json` の一覧は書いたファイルと同じで、書いたあとの `--check` は差を言わない（`crates/ritsu/tests/gen_requests.rs`）。

| ファイル | 中身 |
|---|---|
| `types/<名前>.json` | 申請の種類の説明。キーはどの種類でも同じで、無い物は null か空の配列にする。約束の版、ファイル、名前（ASCII と書いたまま）、版、書いた言語（`language`。`en` か `ja`。題と、中のどの文言（`label`）もこの言語で書き、`labels` はほかの言語で足した文言である。`labels` を書いていない方の言語で、どちらも書いていなければ、申請の名前が ASCII でない申請は `ja`、ほかは `en`。サーバーは、ページの言語の文言が無く、書いた言語のまま出す名前に、この言語の印（HTML の `lang`）を付けられる）、題と `labels`、説明、受け持つ部門、効き始める日、使える日（9.6。`ringi api` と同じ形で、決めたデータを添える）、日を読む UTC オフセット（`utc_offset_minutes`。UTC から東へ何分かの整数で、`+09:00` なら 540。7.5。サーバーはこれでイベントの日を決める）、出せる人（`submitted by` のグループと除くグループ。誰でも出せれば null）、自己承認を許すか（`self_approval`。`self approval allowed` なら true、`not allowed` か書いていなければ false。5.5。サーバーが gate にステップの `self_approval` として渡す値。12.4）、読む組織（役職を低い順に、チームの種類を広い順に、データの場所、どのイベントに渡す部分にも要る人とチーム `needs`（`people`、`teams`、`members_of`。7.5））、フォーム（規則の列挙から選ぶフィールドの選択肢は、フィールドの下の題と `labels`。4.8）、ステップ（それぞれ、理由を求める操作を約束の名前で挙げる `comment_required`。`reject`、`send_back`。求めなければ空の配列。7.2。サーバーの画面と道具は、その操作でコメントの欄を必須として見せる）、決め方（ステップと決め方は、共有の経路の物なら、題と `labels` を経路の `labels` から足す。5.11）、終わり方、人がするイベント（約束の名前。`comment` も）、承認者が直すフィールド、読むマスタ（キー、画面が行の名前を引く列（`name_field`。キーでない最初の文の列）、フィールド、データの場所）、使う flow（`flows`。下） |
| `openapi.json` | サーバーの API の OpenAPI 3.1（12.4 の表）。申請の種類ごとの操作に `x-ritsu-request`（ファイル、名前、版）を付ける。申請の種類ごとに、入口 `worked` の入力と答えのスキーマ（`<名前>.worked.in`、`<名前>.worked.out`）と、それで答える読むだけの操作（`worked_<名前>`、`ticket_worked_<名前>`）を書く（12.4）。操作ごとに、サーバーが返す答えを全部書き、断り（409）のほかのエラーの本文は共有のスキーマ `error` である（12.4）。`servers` は書かない |
| `mcp/tools.json` | エージェントが使う道具の定義（MCP の `tools/list` の形）。道具は `openapi.json` の操作で、名前は `operationId` と同じ：申請の種類の一覧、承認待ちの一覧と自分の申請の一覧（どちらもページで。答えの `more` が次のページがあるかを言い、最後のチケットの ID を `before` にして次のページを読むと、道具の説明が言う）、チケットを見る、種類ごとの申請と、その種類が持つなら出し直しと承認者の直し、途中のフォームの答え（`worked_<名前>` と、出し直しか承認者の直しのある種類なら `ticket_worked_<名前>`。12.4）、承認、差し戻し、却下、取り下げ、差し戻されたあとの破棄、承認の取り消し、コメント、組織、マスタ、人とチームとマスタの行を探す道具（`find_people`、`find_teams`、`find_master_<マスタのファイル>_<マスタ>`。12.4、13.2）。読む道具は `readOnlyHint` を、取り下げと破棄と承認の取り消しは、戻せない操作の印（`destructiveHint`）を持つ。管理する人の操作（`pause`、`resume`、`add_approver`、`hand_over`）は道具にしない。だれが決めるかを変えるか、期限の時計を止める操作で、チケットと組織を見た人が自分で判断して行う物だからである（アシスタントに頼むのは、申請する人と承認する人の操作である）。チケットを動かす道具は、どれも人が書く文（`comment`）と操作の冪等キー（`idempotency_key`。12.4 の説明と形）を受ける。出し直しと直しの道具の説明は、値を null にしても消えないこと、空にするフィールドは `clear` に名前を挙げること、隠れたフィールドの値は名前を挙げなくても落ち、履歴の行の `cleared` に残ることを言う。申請の道具の説明は、申請の種類の説明（`description`）を含み、フィールドは文言、ほかの言語の文言、表示と必須の条件、選択の値の文言を持つ。エージェントは、道具の一覧だけを見て、資料からフォームを埋める（2 章）。入力のスキーマは参照する物を `$defs` に持ち、一つで読める |
| `organization/schema.json`、`organization/columns.md` | 組織のデータの版の `organization.json` の JSON Schema（チームの種類と役職は宣言の名前、代理と組織の部分を `$defs` に。組織の部分の人は、ID と名前（`name`、`name_ja`）を持つ。7.5。組織が二つあれば `oneOf`）と、CSV（チーム、人、所属、代理と、各マスタ）の列の説明 |
| `vectors/<名前>.jsonl` | その種類のベクタ。`ritsu ringi vectors` と同じ物 |
| `vectors/worked/<名前>.jsonl` | その種類の入口 `worked` のベクタ（10.4）。一行が、入力（`input`）、参照の答え（`answer`）、元（`from`）を持つ |
| `organization/find.jsonl` | 探す操作のベクタ（13.2）。どの申請の種類にもよらない一つのファイルで、一行が一つの場合である。行は、場合の名前（`case`）、探す相手（`among`：`people`、`teams`、`master`）、相手の値（`values`。人とチームは `{"value", "name", "name_ja"}`、マスタの行は `{"value", "name", "valid_from", "valid_to"}` で、選べる日の端の無い所は空の文字列）、尋ねた物（`q`、`limit`、`lang`、`on`。`limit` と `on` を渡さない場合は null で、`on` を渡すのはマスタの行だけ）、参照の答え（`answer`。200 で返す `{"found": […], "more": …}` か、400 で答える場合の、合わない引数の名前 `{"bad": "q"}`）を持つ。サーバーの探す操作（12.4）と画面の `/find`（13.8）を、行の `values` だけを相手にして、同じ答えを返すかで確かめる。申請の種類によらないので、種類ごとのファイルを置く `vectors/` でなく、組織のデータの説明と同じ `organization/` に置く |
| `a2ui/<名前>/{form,card,list,mine,history,offered}.{en,ja}.json` | その種類の六つの画面の、英語と日本語の A2UI 1.0 のテンプレート（13.1） |
| `a2ui/catalog.json`、`a2ui/functions.js` | 画面が使う ritsu のカタログと、その関数の ES モジュール（13.2、13.7）。どの種類の分も一つのファイルに |

フォームのフィールドは、どの型でも同じキーを持つ。名前、文言、型（ASCII と書いたまま）、範囲（数は約束の JSON の整数で、日付は日付の文字列で）、文字数か行の数の限り、選択の値、`optional` と書いたか、計算するか（計算が読むフィールド `reads` と一緒に）、画面がフォームの上で計算するか（`worked_on_screen`。フォームの段の、数を計算するフィールドで、`functions.js` の `<申請>.<フィールド>` が計算する。計算する日と、明細の行の計算するフィールドは、機械だけが計算する）、承認者だけのものか、`open in` のカレンダー、表示と必須の条件（書いたままの文、それを求める下ろした規則の関数の名前、読むフィールド `reads`）、行のフィールドである。`required when` を持つフィールドは条件が成り立つときだけ必須で、`shown when` を持つフィールドは隠れているあいだ必須でも送られもしない（4.2）。

`reads` は、計算か条件の答えが変わりうるフォームのフィールドで、ほかの計算を通して読む物も入れ、フォームの順に一つずつ並べる（明細の行のフィールドは `items.price`）。下ろした規則が読む物のうち、答えを変える物だけである。`over is none` で、`over` が、空のことがある `left` と、いつも入れる `asked` から計算されるなら、`over` に値があるかは `left` だけで決まるので、`reads` は `left` だけになる（4.9）。隠れているフィールドは空として読むので、条件の読むフィールドが条件つきで見えるなら、その条件の読むフィールドも入る。画面が、条件と計算を、読むフィールドが決まったときに決めるのに使う（13.7）。OpenAPI と MCP の道具の説明では、条件のあるフィールドに「(worked out from `kind`)」と書き添える。

使う flow（`flows`）は、一つごとに、役目（`launch`：終わったときに始める。`agent`：エージェントが答える決め方。`summary`：ステップの要約）、別名（ASCII の名前と書いたまま）、ファイル、使う所（`at`：始める終わり方、エージェントの決め方、要約のステップ）と、dandori が宣言から決める物を持つ。ワークフローの名前と版、Temporal でのワークフローの型とタスクキュー（どちらも `<名前>_v<版>`。dandori が書く物と同じ）、出力（名前、dandori の書く型、`optional`）を宣言の順に、である。サーバーは、要約の出力をこの順に並べて `note ready` に渡す（15 章）。また、flow をこの型でこのキューに始め、動かす前に、種類が使う flow のどれにもワーカーがあるかを確かめられる。キーを足しただけなので、前の約束の版のサーバーは読まずに済む（12.4）。

### 12.3 プロジェクト全体に書き出す物

チケットの機械の本体（kaname のモジュール `ticket`）は、ringi のクレートが持つ `.kn` で、プロジェクトに `.ringi` があれば `ritsu-project` が Overlay に置き、パッケージに入る。名前は `ticket` にして、言語の名前を生成物に出さない。

置き場は根の下の `.lowered/ticket.kn` で、プロジェクトに一つだけ置く。申請ごとの下ろした規則と日付とつなぎも、`<ファイル>.lowered/` に同じく重ねて置く。どれもディスクには書かない。`ritsu gen` は、申請の検査をほかのファイルの検査と一緒にし（通らなければ止まる）、置いた物をプロジェクトのファイルと同じに検査してから（下ろした先の言語のエラーで止まり、つなぎと機械には kaname の警告と total でない関数でも止まる。どれも申請の行の E201 で言う。9.2）パッケージにする。組織の版ごとの承認者と申請者の一覧の誤り（E502〜E504）では止まらず、申請ごとに一行の警告で言う。生成したコードは、イベントのたびに渡された組織で決めるからである。`--format json` の `from` は、その置き場のパス（`tests/machine/held.lowered/held.kn`、`.lowered/ticket.kn`）を言う。つなぎがまだ下ろせない物を持つ申請の種類は、その申請のコードを書かずに警告で言い、ほかの部分は書く（終了コードは変えない）。`.ringi` の無いプロジェクトの生成物は、申請を読むようにする前と 1 バイトも違わない。

### 12.4 サーバーとの境目

サーバー（ritsu の外に置くフレームワーク。12.5）は、次をする。ritsu は、これを別の人が別の言語で書けるだけの約束を書き出す。約束は、生成したパッケージの入口（申請の種類の一覧と、種類ごとの `start`、`next`、`worked` の JSON の入口。12.2）と、`--target requests` のファイル（`openapi.json`、`types/*.json`、申請の種類ごとのベクタ `vectors/*.jsonl` と、入口 `worked` のベクタ `vectors/worked/*.jsonl`、組織を探す操作のベクタ `organization/find.jsonl`）である。

**約束の形。** 入口、ベクタ（10.4）、`ritsu ringi run --format json` は、同じ一つの JSON の形を使う。参照インタプリタの型をこの形に書くのは `src/lower/shape.rs` の一か所で、参照インタプリタの形は Rust の型の中にだけある。

| 入口 | 受け取る物 | 答え |
|---|---|---|
| `start` | `submit`（チケットの ID、申請者、代理申請ならその人、フォームの値、試み。7.1）、`organization`（7.5）、`now` | `{"ok": {"ticket": …, "effects": […]}}` か、`{"err": {"refused": …, "text": …, "text_ja": …, "at": …}}` |
| `next` | `ticket`（前に返したチケット）、`event`、`organization`、`now` | 同じ |
| `worked` | `input`（途中まで埋めたフォームと承認者の直し。`{"form": …, "edited": … \| null}`。下の「途中のフォームの答え」） | `{"fields": {…}}` |

7 章の語と JSON の語が違うのは、次の所だけである。どれも、kaname と kaname が書く言語が語として使う物を避けたか、kaname の型で書ける形にした物で、置き換えは `shape.rs` と `ticket.kn` の決まった所だけが持つ。

| 7 章の語 | JSON の語 |
|---|---|
| 空白を含む値（`in progress`、`resubmit late`、`went up`、`cancel timer`） | 空白を `_` にする（`in_progress`） |
| ステップの段階と部分の状態の `open` | `active` |
| 部分の `with`、`hand over` の `from` | `of` |
| 部分の `sole` の `pass`、`by` | `passing`、`{"by": {"person": …}}` |
| することの `record`、`void` | `history`、`release` |
| すること（行うこと、冪等キー、中身） | `{"key": {"ticket", "round", "serial"}, "what": {"<行うこと>": {…}}}` |
| することの引数、要約の出力、マスタの行の値 | 名前と値の並び（`[{"name": …, "value": …}]`）。することの引数と要約の出力の値は、種類の一つのキー（`str`、`num`、`truth`、`rows`）か `nothing` |
| イベント | イベントの名前の一つのキー（`{"approve": {"by": …, "revision": …, "comment": …}}`、`{"comment": {"by": …, "comment": …}}`）。時計のイベントは名前だけ（`"late"`）。`submit` は `{"submit": …}` |

**途中のフォームの答え**（入口 `worked`）。入力は `{"form": …, "edited": … | null}` である。`form` は申請の本文の `form` と同じキーと値で、値の無いフィールドはキーが無いか null である。明細は行の配列で、計算するセルは書かない。`edited` は承認者の直し（`<名前>.edited` の形）で、承認者が直せるフィールドだけを読み、null かキーの無いフィールドはフォームの値のままにする。答えは `{"fields": {…}}` で、`shown when` を持つフィールドは `shown`、`required when` を持つフィールドは `required`、計算するフィールドは `value`、計算するセルを持つ明細は `rows`（入力の行の順に、行ごとの `{"<セル>": {"value": …}}`）を持ち、どれも持たないフィールドは書かない。`shown` と `required` は true、false、null で、`value` は約束の形の値（数は整数、日付は `YYYY-MM-DD`）か null である。null は、まだ決まらないことを言う（13.7）。範囲の外の値、選択肢に無い値、下限より少ない行、上限より多い行は、断りでも入口のエラーでもない。スキーマ（`openapi.json` の `<名前>.worked.in`、`<名前>.worked.out`）は型だけを書く。範囲、`enum`、行の数を書かず、どの値も null を許し、要るのは `form` だけである。途中のフォームを受け、範囲の外の値にも、エラーでなく決まらない値で答えるためである。

**断り。** `next` が断ったときの答えは、種類（`refused`。7.2 の 12 種類）、英語の文（`text`）、日本語の文（`text_ja`）、場所（`at`）を持つ。文は機械の言葉で、どの申請の種類でも、`--lang` によらず両方を書く。生成したコードの文は、参照インタプリタの文と同じである（`crates/ritsu/tests/gen_requests.rs` が、四つの出力先で、全部のベクタの文を英日で比べる）。サーバーは見る人の言語で選ぶ（MCP の答え、ページ、画面の外の断り）。文は人に見せる物で、サーバーは文を読んで断りを分けない。分けるのは種類と場所である。

場所は、フォームの断り（`form`）がどのフィールドの断りかである。フォームのフィールドの ASCII の名前（`purpose`。計算するフィールドも同じで、明細の合計なら `amount`）か、明細の行のセルなら `<明細>[<行>].<セル>` で、行は、そのイベントが渡した行の並びの 0 からの位置である（`items[1].price`）。明細の行で計算するセルの範囲の断りは、範囲の外になった最初の行のセルを指す（`lines[0].subtotal`）。行の数の断りは、明細のフィールドの名前である（`lines`）。場所の書き方は、生成した入口のエラーの文と、規則の断りの文が要素を言う書き方（`lines[1].subtotal`）と同じで、文も、場所と同じ番号で、申請の書いた名前で言い、計算した値の断りは直すフィールドも挙げる（4.4。購買の日本語の名前の版で「計算した `明細[0].小計` が、範囲（`>=1円 <=100_000_000円`）の外です。`明細` の `数量` か `単価` を直してください」、場所は `lines[0].subtotal`）。範囲は、ファイルに書いた単位の綴りで言う。

どのフィールドでもない断り（ほかの種類の断りと、フォームに無い名前）は、場所が null である。コメントの断り（`comment`。人の操作に添えた文が 2,000 字を超えるか、イベント `comment` の文が無いか、理由を求める却下と差し戻しに理由が無い。7.2）も null で、種類がコメントの断りであることを言う。画面は、フォームの断りなら場所の最初の名前を `/refused/field` に入れてそのフィールドの横に、コメントの断りなら `/refused/comment` を真にしてコメントを書く部品の下に、文を見せる（13.8）。

**約束の版。** 約束の形が変わったことを読み手が知るための一つの整数で、ringi のクレートの `src/lower/shape.rs` の `CONTRACT` が持つ（いまは 3）。表す範囲は、チケット、イベント、組織の部分、答えの JSON の形、`start`、`next`、`worked` が受け取る物と答える物、`--target requests` のファイル（画面のテンプレートとそのデータの置き場も）である。各言語の申請の種類の一覧（12.2）と、`--target requests` が書くファイル（`openapi.json` の `x-ritsu-contract`、`types/<名前>.json` の `contract`）に書く。前の数のために書いたサーバーが、新しい数のパッケージを読み違えるとき（キーや語が変わったか、無くなったか、意味が変わった）に上げ、読み違えないとき（申請の種類が増えた、読まなくてよいキーが増えた、読まなくてよい画面のファイルが増えた）は上げない。画面のデータに、サーバーが値を入れなければならない置き場（`/edit`、`/refused`、`/upload` のような物）を足すときも上げる。その置き場を知らないサーバーはテンプレートのまま送り、画面が読み違えるからである。サーバーは、知らない数のパッケージを断る。2 は、1 に、試み（`submit` と `resubmit` の `attempt`、チケットの `attempt`。7.1）、断りの `text_ja` と `at`、画面の置き場 `/refused`・`/upload`・`/find` と承認カードの `/ticket/requester`・`/ticket/made_by`、自分の申請の一覧の画面（`mine`）を足し、フォームの値とコメントの文を `openapi.json` の `required` から外した版である。3 は、2 の次のところを変えた版である。

- 組織の部分の人を、ID の文字列から、ID と名前の組（`{"id", "name", "name_ja"}`。7.5）にした。
- チケットのステップに、催促が鳴ったかの `reminded`（真偽値）を足した。再開したステップは、鳴っていない催促を、ずらした期限より前に来るときに置く（7.2、7.5）。
- 断りの種類に `comment` を足し、コメントの断りを `form` から分けた（7.2）。明細の行で計算するセルの範囲の断りの場所を、明細のフィールドの名前（`lines`）から、範囲の外になった最初の行のセル（`lines[0].subtotal`）にした。
- 人、チーム、マスタの行を探す操作（`find_people`、`find_teams`、`find_master_<マスタのファイル>_<マスタ>`）と、そのベクタ `organization/find.jsonl` を足した（下の表、13.2）。
- 一覧の答えに、次のページがあるかの `more` を足した。
- 画面の置き場 `/refused/comment` と、二つの一覧の画面の `/more` を足した（13.8）。

入口 `worked`（12.2、13.7）と、その入力と答えのスキーマ（`openapi.json` の `<名前>.worked.in`、`<名前>.worked.out`）、操作、ベクタ（`vectors/worked/<名前>.jsonl`）は、3 のまま足した。足しただけで、前のサーバーが読み違える物は無いからである。ただし、Go の一覧の `Kind` は形が変わるので、形で受けるサーバーはコンパイルで気づき、知らない操作を断るサーバーは `openapi.json` を読むときに気づく（16.4）。操作ごとのエラーの答えと、その本文のスキーマ（`error`）も、3 のまま書いた。サーバーがもう返していた答え（401、413、415、500、一覧とマスタの行の 400、組織とマスタの行を探す操作の 404、効いている版の無い種類の 404）は、答えを変えずに約束に書いただけである。種類違いのチケットの 404 と、効いている版の無い種類の 404 の前に gate に尋ねることは、前の約束が書いていなかった答えを決めた物で、その本文の `from` は、読まなくてよいキーである。探す操作のベクタの、マスタの行を選べる日（行の `on` と、マスタの行の `valid_from` と `valid_to`）と、それを確かめる行も、3 のまま足した。サーバーがもうしていたこと（マスタの行を、その日に選べる物だけにすること、日付でない `on` を 400 で断ること）を、ベクタに書いただけだからである。それらのキーを読まないサーバーは、前からある行には同じに答え、足した行で、読んでいないことに気づく。`types/<名前>.json` の `organization.needs`（7.5）も、3 のまま足した。読まないサーバーが部分を切らずに版の全部を渡しても、答えは同じだからである。お知らせの画面（`offered`。13.1、13.8）も、3 のまま足した。それを読まないサーバーは、新しいファイルを読まず、前からある画面には同じに答えるので、読み違える物は無いからである。

**チケットの版。** 時計と `comment` のほかのイベントは、それを作ったときのチケットの版（`revision`。7.1、7.2）を持つ。画面と道具は、見せたチケットの版を操作に付ける。flow を頼んだときは、頼んだ遷移が返したチケットの版を覚え、flow の答えに付ける。`next` は、版が今のチケットと違う操作を `stale` で断る。サーバーは、チケットを見せ直す。

| サーバーがすること | 約束 |
|---|---|
| チケットと履歴の保管 | チケットは `next` が返した JSON をそのまま持つ。履歴は、イベントとすることの一覧を追記だけで持つ。チケットごとに、保存のたびに変わる番号を持つ |
| イベントの受け口 | `openapi.json` の操作。どの操作も操作の冪等キーを受け、時計と `comment` のほかのイベントはチケットの版も受ける。人の操作は、その人が書いた文（`comment`）も受けて `next` に渡す。`submit` と `resubmit` には、操作ごとの試みの名前（7.1）を付けて渡す。同じ冪等キーの二度目には、最初の答えを返す。`next` が断ったイベントは、断りの種類（7.2）で断り、チケットを変えない。帳簿が断ったときも、そのイベントを `book` の種類で断り、チケットを変えない。管理する人の操作（`add_approver`、`hand_over`）は、操作した人を `by` に入れる |
| 一件ずつ当てる | 一つのチケットへのイベントは、一つずつ、直前に保存したチケットに当てる。同時に届いたイベントの順は、届いた順でなくてよい |
| 認可 | イベントを `next` に渡す前に、sekisho の生成したコードで gate に尋ねる。`comment` も同じで、だれが書けるかは gate だけが決める（機械は見ない。7.2）。ステップとチケットの属性は、ringi が決めた値を渡す（下の表）。`submit` は、gate の `submit`（会社全体の決まり）と `submitted by`（`next` が断る。5.13）の両方を満たすときだけ通る。サーバーの管理の操作（申請の種類の版の始まりを決めること、その記録を読むこと、組織のデータを取り込むこと、その記録を読むこと）も、受け付ける前に gate に尋ねる（14.3） |
| 期限と催促の時計 | 時計（`set timer` と `cancel timer`）は、することのうち外へ出ない物で、チケットと一緒に保存する。鳴らすのはサーバーで、時刻が来たら、時計のイベントをほかのイベントと同じ道で渡す。鳴った時計のイベントは、ほかのイベントより先に渡す（`next` は、先に来たほかのイベントを `clock` で断る。7.6）。遅れて渡してもよく、期限は延びない（7.5） |
| 帳簿 | `hold`、`post`、`void` を chobo のクライアントで行う。帳簿がチケットと同じデータベースにあれば、チケットの保存と同じトランザクションに入れる（chobo の PostgreSQL のクライアントは、呼ぶ側のトランザクションを受け取る）。仮押さえのキーと、別のデータベースにあるときの順は、下に書く |
| 知らせと処理の開始 | 保存のあとに、保存したすることの一覧から、冪等キーで行う（一度より多く行っても、一度と同じ）。`notify` を経路（メール、チャット）に渡す。flow（`launch`、`ask agent`、`ask note`）は dandori の生成したコードで始め、することの冪等キーを flow の実行の ID にする。終わったら、答えを、頼んだ遷移が返したチケットの版を付けたイベントで返す |
| 組織 | 版を持ち、`now` の日に効いている版の、チケットが要る部分を渡す（7.5）。部分の形は、ベクタの `organization` である（10.4） |
| 画面の配信 | A2UI のテンプレートにチケットの値を入れて送り、ボタンの操作を、見せたチケットの版を付けたイベントにする。操作のボタンは、その人が今できる物だけを見せる：gate が許し、`next` が今のチケットでその人から受ける操作である（`/can`。13.1）。断られた操作は、断りの文と場所（コメントの断りなら、コメントの断りであること）を `/refused` に入れて画面を送り直す。一覧の画面には、一覧の答えのチケットと `more` を入れる。添付は、サーバーが決めたアドレス（`/upload`）で受け、人、チーム、マスタの行は、サーバーが決めたアドレス（`/find`）で探させる（13.2、13.8）。素の HTML の画面も同じ説明から作る（13.6）。効いている版の無い種類の申請書を開いた人には、お知らせの画面を出す（出せるようになる日を `/from`、始めのページのアドレスを `/home` に入れて）。申請の種類の一覧には、その人が出せる物だけを出す |
| 見せること | チケットを見られる人にだけ見せる（下の「見られる人」）。見られない人の操作は、どれも 404 で答える |
| 監査ログ、認証、MCP サーバー | 履歴と管理の操作を監査ログに残す。認証はサーバーの物。MCP サーバーは `mcp/tools.json` の道具を出す。コメントの間隔と数、添付の数を制限してよく、制限で断るときは 429 で答える（下の API の操作。`next` の断りではなく、チケットを変えない） |
| 適合 | 下に書く |

**gate に渡す値。** ステップ（resource の `Stage`）とチケット（`Ticket`）の属性に、ringi が決めた次の値を渡す。人の集まり（`Requesters`、`Approvers`）は、そこに入る人それぞれの親として渡す。属性の名前は例の gate（`examples/shared/company.gate`）の物である。

| 属性 | 渡す値 | 例の gate で読む所 |
|---|---|---|
| `requester`（ステップとチケット） | 申請者 | 自分の申請を決めない forbid（ステップの物。チケットの物は、例の gate では読まない） |
| `requesters`（ステップとチケット） | 申請者の側の人：申請者、代わりに出した人（チケットの `made_by`）、その日に有効な、申請者の `submit` の代理（その申請の種類を挙げる行）。機械が `resubmit`、`withdraw`、`discard` を受け付ける人と同じである（5.8） | 取り下げと破棄（action の `withdraw`）、コメント |
| `approvers`（ステップを進めているあいだ） | そのステップに経路が決めた承認者と、その日に有効な、その人たちの承認の代理。そのステップとは、チケットの版の位置（`revision.stage`）の物である。あとのステップを待ったステップは `stages` に同じ名前で二度あり、一度目は待ったときのままなので（5.12）、名前（`stage`）では探さない | 承認者のする操作、コメント |
| `approvers`（申請者に戻っているあいだ） | 差し戻した人（チケットの `sent_back_by`。7.1）。今のステップが無いので、この人を渡す。申請者の側の人が取り下げたなら空 | コメント（差し戻した人が、申請者の問い返しに答える）。承認者のするほかの操作は、gate を通っても `next` が断る（7.2） |
| `owner`（ステップ） | 申請の種類を受け持つチーム（`owner`）。書いていない種類には、その日の組織の版のどのチームの ID とも違う ID を渡し、空の文字列は渡さない（14.3） | 管理する人の操作、コメント |
| `self_approval`（ステップ） | 経路の自己承認の設定（書かなければ `false`。5.5）。`types/<名前>.json` の `self_approval`（12.2） | 自分の申請を決めない forbid |

例の gate が読む人の部門（`principal.department`）は、`init` の `gate.go` が、受け持つチームの種類でフレームワークの `TeamOf` を引いた物である。組織の版に主の所属の無い人は、gate が読まない（14.3）。

自分の申請を決めない forbid は `requester` だけを読み、`requesters` を読まない。代わりに出した人は、自己承認の判定では申請者として扱われ（5.8）、自己承認を許さない経路では承認者から外れ（5.5）、承認の代理としても答えられない（`next` が `person` で断る）。申請者の側の人のうち、このチケットを出していない `submit` の代理は、その人がステップの承認者か承認の代理なら機械が答えさせるので、gate もそれを禁じない。

**見られる人。** チケットを見られるのは、組み込みの人と、gate の `see` が許す人である。

- 申請者の側：申請者、代わりに出した人（`made_by`）、その日に有効な、申請者の `submit` の代理。
- 承認者の側：今と前のステップの承認者、答えた人とその人が答えた席の人（代理が答えたなら、代わられた人）、差し戻した人、その日に有効な、その人たちの `approve` の代理。前のステップの承認者と答えた人は、差し戻しでチケットの JSON から消えることがある（前のステップへ戻すと、そこから先の答えは無くなる）。サーバーは、チケットと履歴の行（`by`、`proxy_of`）から決める。
- gate が `see`（resource は `Ticket`）を許す人。例の gate では、申請の種類を受け持つ部門の人で、管理する人もこれで見る。

見られない人の操作は、gate が許していても、どれも 404 で答える。操作を許すことは、見せることを含まない（操作の答えはチケットの全部を返すので、見せない人にそれを読ませない）。例の gate は、`comment` を許す受け持つ部門の人と、`add_approver` と `hand_over` を許す管理する人に、`see` も許す。一覧の操作（`list_waiting`、`list_mine`）は、その人の物だけを返す。

**管理する人の操作の `by`。** `add_approver` と `hand_over` の `by` は、操作した人である。サーバーは、認証した人を `by` に入れて `next` に渡し、機械は履歴の行の `by` に残す。`by` が null の操作は、人の無い操作（サーバーの中で決まった手当て）で、履歴の行の `by` も null になる。答えは、履歴の行の `by` のほかは同じである。ベクタは両方を持つ（10.4）。

**API の操作**（`openapi.json`、2026-11-15）。人がするイベントのそれぞれと、画面とエージェントが読む物を操作にする。だれの操作かは、サーバーが認証した人で、本文には書かない（代理の申請だけは、本文の `requester` に、代わりに出す相手を書く）。

| 操作 | `operationId` | 本文 | 答え |
|---|---|---|---|
| `GET /requests` | `list_requests` | | その人が出せる申請の種類（名前、版、書いた言語（`language`）、題と、ほかの言語の題（`labels`。`types/<名前>.json` と同じ形）、説明、ファイル）。サーバーは、見る人の言語の題を `labels` から選べる |
| `POST /requests/<名前>/tickets` | `submit_<名前>` | `requester`（代理のときだけ）、`form`（フォームの値。申請者が入れるフィールドだけ）、`comment` | チケット |
| `POST /requests/<名前>/tickets/{id}/resubmit` | `resubmit_<名前>` | `revision`、`form`（変えるフィールドだけ。null は前の値のまま）、`clear`（空にするフィールドの名前。7.3）、`comment` | チケット |
| `POST /requests/<名前>/tickets/{id}/edit` | `edit_<名前>` | `revision`、`fields`（承認者が直すフィールド。null は前の値のまま）、`clear`（空にするフィールドの名前）、`comment` | チケット |
| `POST /requests/<名前>/worked` | `worked_<名前>` | 入口 `worked` の入力（`<名前>.worked.in`。`form` と `edited`。上の「途中のフォームの答え」） | 入口 `worked` の答え（`<名前>.worked.out`。見せるか、必須か、計算の値。決まらない所は null）。種類の今の版の決まりで答える |
| `POST /requests/<名前>/tickets/{id}/worked` | `ticket_worked_<名前>` | 同じ | 同じ。そのチケットを出した版の種類の決まりで答える |
| `GET /waiting?limit=&before=` | `list_waiting` | | その人か、その日にその人が代理する人の答えを待つチケット。待っているステップが始まった時刻の新しい順（`{"tickets": […], "more": …}`） |
| `GET /mine?limit=&before=&kind=` | `list_mine` | | その人の申請：その人が申請者か、代わりに出した人のチケット。作った時刻の新しい順。`kind` で申請の種類を一つに絞れる（答えの形は `list_waiting` と同じ） |
| `GET /tickets/{id}` | `get_ticket` | | チケットと履歴（見られる人だけ。上の「見られる人」） |
| `POST /tickets/{id}/<イベント>` | イベントの名前（`approve`、`reject`、`send_back`、`retract`、`pause`、`resume`、`withdraw`、`discard`、`add_approver`、`hand_over`） | `revision` と、イベントの値（差し戻す先、足す承認者、引き継ぐ人など）、`comment` | チケット |
| `POST /tickets/{id}/comment` | `comment` | `comment`（文だけ。`revision` は無い） | チケット |
| `GET /organization` | `get_organization` | | 今日効いている組織の版（人は、知らせと画面を読む言語 `language` も持つ。8.2） |
| `GET /masters/<マスタのファイル>/<マスタ>?on=<日付>` | `get_master_<マスタのファイル>_<マスタ>` | | その日に選べる行 |
| `GET /find/people?q=&limit=&lang=` | `find_people` | | 今日効いている組織の版の人のうち、言葉で見つかる物（`{"found": [{"value", "name", "label"}], "more": …}`。近い物から。13.2） |
| `GET /find/teams?q=&limit=&lang=` | `find_teams` | | 同じ版のチームのうち、言葉で見つかる物（同じ形） |
| `GET /find/masters/<マスタのファイル>/<マスタ>?q=&limit=&lang=&on=<日付>` | `find_master_<マスタのファイル>_<マスタ>` | | その日（無ければ今日）に選べる行のうち、言葉で見つかる物（同じ形。`value` は行のキー、`name` はマスタの `name_field` の列） |

- 申請の種類ごとの操作は、その種類が持つ物だけを書く。`resubmit` と `discard` は差し戻しのある種類、`resume` は保留のある種類、`edit` は承認者が直すフィールドのある種類だけで、`/tickets/{id}/<イベント>` は、どれかの種類が持つイベントだけを書く（`comment` は、どの種類のチケットも持つ）。時計のイベントと flow の答えは書かない。サーバーの中で `next` に渡す物だからである。
- 途中のフォームを尋ねる二つの操作（`worked_<名前>`、`ticket_worked_<名前>`）は、読むだけで、チケットを変えない。本文は入口 `worked` の入力のとおりで、答えは入口の答えのとおりである（13.7）。自前の画面を作る人が、条件と計算を作り直さずに使えるようにある。`worked_<名前>` は、認証した人のだれにでも答える。答えはチケットも組織も読まず、種類の決まりと渡したフォームだけで決まり、同じ決まりは画面の `functions.js` が見る人のブラウザにもう渡しているからである（承認者の直しを尋ねる人は承認者で、その種類を出せる人とは限らない）。`ticket_worked_<名前>` は、差し戻しか承認者の直しのある種類だけに書き、そのチケットを出した版の種類の決まりで答える。差し戻したチケットの出し直しと承認者の直しは、チケットを出した版の決まりで決まるので、種類の版が新しくなっていても、そのチケットの画面と同じ答えを返すためである。チケットからは版だけを読み、フォームの中身は本文の物だけで決め、チケットの中身を答えに出さない。見られる人だけに答える（下の 404）。どちらも `Idempotency-Key`、`revision`、`comment` を取らず、409 と 429 で答えない。本文の形の誤りと、生成した入口が約束の外の JSON として止めた値は 400（`unworked`）で、文は一覧の `lang` の言語である。範囲の外の値は、400 でなく、決まらない値（null）として答える。
- 人の操作の本文の `comment` は、文か null で、無くてよい（`maxLength` は 2,000）。`POST /tickets/{id}/comment` の本文の `comment` も、`required` に書かず null を取る（`minLength` は 1、`maxLength` は 2,000）。コメントの操作の、文が無い・null・空、どの操作でも 2,000 字を超える文は、機械が 409 の `comment` で断る（「the text of `comment` is empty」、「the comment is 2001 characters long, past the limit (2000)」。場所は null）。
- 途中のフォームを尋ねる二つの操作のほかは、どの POST も、ヘッダ `Idempotency-Key` で操作の冪等キーを受ける。新しい操作には新しいキーを使い、同じ呼び出しを送り直すとき（ネットワークのやり直し、答えが届かなかったとき）は同じキーを使う。同じキーの二度目には最初の答えを返し、何も変えない。値を変えた申請と出し直し、断られたあとの出し直しは新しい操作で、新しいキーを取る。キーは 1〜255 バイトの、表示できる ASCII（`!` から `~`。空白は無し）である。エージェントは、道具の説明だけを読んでキーを作る（`mcp/tools.json` にも同じことを書く）。
- 受け取ったイベントは 200 でチケットを返す。`next` が断ったイベントは 409 で、断り（種類 `refused`、文 `text` と `text_ja`、場所 `at`。上の「断り」）を返し、チケットを変えない。gate が通さない操作は `person`、帳簿が断った操作は `book` の種類で断る。
- 本文の形の誤りは 400 である：JSON の型が違う値、要るキーが無い、取らないキー、`YYYY-MM-DD` でない日付、64 ビットに入らない整数、キーの欠けた版か範囲の外の版（`revision` の `round` は 1〜1,000,000、`stage` は 0〜1,000,000 か null、`edits` は 0〜1,000,000。生成した入口と同じ範囲）、冪等キーが無いか形が違う。生成した入口が、約束の外の JSON として止めた値（12.2 の入口のエラー）も 400 で返す。人が書いた物（フォームと明細の行の値、コメントの文）は、スキーマが何を書いていても `next` に渡し、取らない値を 409 で断る：選択肢に無い値、範囲の外、長すぎる文、多すぎる行、空の必須のフィールド（null か、キーが無い）は `form`、空か長すぎるコメントは `comment` である。ベクタの答えがそのまま API の答えになり、画面は一つの断りの形を出せばよいからである。
- `openapi.json` の `required` は、生成した入口の読み方にそろえる。書くのは、無ければ入口が止めるキー（申請の `form`、`resubmit` の `form`、`edit` の `fields`、`add_approver` の `approver`、`hand_over` の `of` と `to`）と、版（`revision` と、その三つのキー）である。フォームと明細の行のフィールド、コメントの操作の `comment` は書かず、null も取る：入口は、無いキーを null と読み、機械が空の必須のフィールドを `form` で、空のコメントを `comment` で断るからである。いつも必須のフィールドは、説明に `Required` と書く（見せているあいだ必須、条件で必須のフィールドは、その条件を書く）。
- 版は例外で、入口より API が狭い。ベクタのうち、版を持たずに承認する行（申請の種類ごとに一本、`no revision`）は、機械が `stale` で断る行で、API では `revision` の無い本文として 400 で答える（生成した入口を直に呼ぶサーバーのための行である）。入口は、`stage` の無い版も、`stage` が null の版として読む（kaname の読み方は、オプショナルの値の無いキーと null を分けない）。版はチケットの `revision` をそのまま写す物なので、API はキーの欠けた版として 400 で答え、入口に渡さない。`round` か `edits` の無い版は、入口も止める。
- チケットが無いか、その人が見られなければ 404 である。種類ごとのチケットの操作（`resubmit_<名前>`、`edit_<名前>`、`ticket_worked_<名前>`）に、パスの種類と違う種類のチケットを渡したときも、だれにでも同じ 404 で答える（パスが指すチケットは無い）。外から見れば、どちらも見られるチケットが無いということなので、応答は一つ（`unseen`）で、その文が両方を言う。その日に効いている版の無い種類（版の始まりがまだ先か、止めた。14.2）への `submit_<名前>` と `worked_<名前>` も 404 である（`list_requests` にも出ない）。サーバーは先に gate に、その人がその種類を出せるか（`submit`）を尋ねる。出せる人には、応答 `unoffered` で答え、後の版の始まりが決まっていれば、その日を本文に載せる（`{"error": "…", "from": "2026-12-01"}`。いちばん近い後の版の日。止めた種類で次の版が無ければ `from` は無い）。ページとアシスタントは、それで「You can make this request from 2026-12-01.」「この申請は 2026-12-01 から出せます。」と言える。出せない人には、種類が無いときと同じ 404 で答え、効いている版が無いことも日付も言わない。チケットの操作は、チケットを出した版で動くので、これに当たらない。組織を読む操作（`get_organization`）は、その日に効いている組織の版が無いか、その人が今の版にいなければ 404（`no_organization`）である。マスタの行を探す操作は、サーバーが持つどの申請の種類もそのマスタから選ばなければ 404（`unchosen`）である。ほかの操作は 404 で答えない。
- サーバーは、コメント（`comment` の操作）の、同じ人が同じチケットに続けて書く間隔と、チケットごとの数を制限してよい。制限で断るときは 429 で答え、`next` の断りではなく、チケットを変えない（同じ冪等キーは、あとで使える）。429 になってよいのは `comment` だけで、ほかの操作は 429 で答えない。間隔の 429 には `Retry-After`（送り直せるまでの秒数。1 以上の整数）を付け、待っても減らない上限（チケットごとのコメントの数）の 429 には付けない。添付（13.2 の `FileAttachment` の `upload`）も、一人が預けてまだどのチケットも持たない数を制限してよく、超えたら `Retry-After` の無い 429 で答える（申請に付けるか、サーバーが手放すまで減らないため）。
- どの操作も、サーバーが呼び出した人を認証できなければ 401（`unsigned`）、サーバーが失敗すれば 500（`failed`）で答える。本文を取る操作は、本文が大きすぎれば 413（`oversized`）、`Content-Type` が `application/json` でなければ 415（`not_json`）で答える。500 は、チケットを動かす操作が済んだかを言わない。同じ冪等キーで送り直せば、済んでいれば最初の答えが返り、済んでいなければその操作をする。約束の操作でない呼び出し（取らないメソッドの 405、無いパスの 404）は、約束に書かない。
- 断り（409）のほかのエラーの答え（400、401、404、413、415、429、500）は、本文に `{"error": "<文>"}` だけを持つ（共有のスキーマ `error`）。`unoffered` は、そのほかに `from` を持つことがある（上の 404）。文はサーバーが書く物で、入口のエラーの 400 なら入口の文である。自前の画面とアシスタントは、約束だけを読んで、エラーの文の場所を知れる。
- 一覧（`list_waiting`、`list_mine`）は、ページで返す。`limit` は 1〜100 で、無ければ 50。`limit` が合わないか、`kind` が申請の種類の名前でなければ 400（`unasked`）である。`before` は前のページの最後のチケットの ID で、そのあとから返し（一覧に無い ID なら空のページ）、無ければ最初のページである。同じ時刻のチケットは、ID の大きい順（後の物が先）に並べる。答えは `{"tickets": […], "more": …}` で、`more` は、このページのあとにもチケットがあるか（`limit` より多くあったか）である。画面と道具は、`more` が真なら、このページの最後のチケットの ID を `before` にして次のページを読む（13.8）。
- 探す操作（`find_people`、`find_teams`、`find_master_<マスタのファイル>_<マスタ>`）は、画面の `/find` と同じ物で、比べ方と並べ方は 13.2 に一度だけ書いた。引数は `q`（UTF-8 で 200 字まで）、`limit`（1〜100 で、無ければ 20）、`lang`（`en` か `ja`）、マスタは `on`（`YYYY-MM-DD`。無ければ今日）で、どれかが合わなければ 400（`unasked`）である。人とチームは、`lang` の名前と、もう一方の言語の名前の両方で探し、`name` には `lang` の名前を入れる。読むだけの操作で、MCP の道具は `readOnlyHint` を持つ。マスタの行を読む操作（`get_master_<マスタのファイル>_<マスタ>`）も、`on` が日付でなければ 400（`unasked`）である。
- 本文と答えのスキーマは、約束の JSON の形（12.4 の上）をそのまま書く。チケット、ステップ、部分、履歴、断り、組織の人とチームと所属、各種類のフォームの値（`<名前>.given`、`<名前>.changes`、`<名前>.edited`）、明細の行、マスタの行である。フォームの値の数は約束の JSON の整数で、範囲は宣言のまま `minimum` と `maximum` に、日付は `format: date` の文字列に、選択は `enum` に書く。`clear` は、そのイベントで空にできるフィールドの名前の `enum` の配列で、どの申請でも必須のフィールドは入れない（7.3）。

dandori の flow は、この操作を `use openapi` で読める（15.4）。dandori は、タスクの引数と答えのレコードを、操作の本文と答えのスキーマに名前で突き合わせ、範囲も比べる（例の休暇の `days` を `range >=1 <=30` にした flow は、dandori の E016「`>=1 <=20` is taken」になる）。

**一つのイベントの順。**

1. だれの操作かを確かめ、チケットと組織の部分を読む（帳簿が別のデータベースにあるときは、先にチケットの鍵を取る。下に書く）。
2. gate に尋ね、`next` を呼ぶ。
3. 一つのトランザクションで、帳簿の操作（`hold`、`post`、`void`。帳簿が同じデータベースにあるとき）と、チケット、履歴、時計、することの一覧、操作の冪等キーとその答えを、チケットの番号が読んだときのままなら書く。帳簿が断るか、番号が変わっていれば、全部取り消す。番号が変わっていたときは、読み直して gate からやり直す（`next` は渡された物だけで答えるので、やり直してよい）。帳簿が断ったときは、チケットを変えずに `book` の断りを返す。保管を差し替える人が守るのは、この一つの操作である。
4. 書けたらすぐ、操作の答えを返す。知らせと処理の開始を待たない。
5. そのあとで、書いた一覧から、知らせと処理の開始を冪等キーで行う。書く前に外へ知らせない。済むまでやり直してよい。

**帳簿の仮押さえのキー。** 仮押さえの振替は、申請と試み（`ticket.attempt`。7.1）をキーにする（例の予算と休暇の帳簿は `key request, attempt` で、`hold`、`post`、`void` に `attempt: ticket.attempt` を渡す）。キーが試みを受け取らなければ E408 である（9.5）。帳簿は、キーごとに答えを覚え（断りも覚える）、取り消したキーではもう押さえない。キーが申請と `round` だけなら、帳簿をチケットと別のデータベースに置いたとき、帳簿の断りはチケットを保存しなくても残るので、値を変えた出し直しも予算が増えたあとの出し直しも、同じ `round` のうちは同じキーで断られる。保存できずに取り消した仮押さえのキーに、あとの試みが当たって何も押さえないこともある。試みをキーに入れると、帳簿をどこに置いても、次の四つが成り立つ。

1. 同じ試みの送り直しは、同じキーに当たり、同じ答え（`done_before`）を受けて、二重に押さえない。
2. 断られたあとの新しい試みは、新しいキーで、新しく確かめられる。
3. 取り消した仮押さえは、あとの試みを止めない。あとの試みは別のキーである。
4. サーバーは、保存しなかった試みの仮押さえを、その試みのキーで取り消せる。ほかの試みの仮押さえには当たらない。

`crates/ringi/tests/books_apart.rs` は、chobo が例の予算の帳簿から書く SQL を PostgreSQL で動かし、帳簿をチケットと同じトランザクションに入れる形と、帳簿の呼び出しだけを確定してチケットの側を巻き戻す形（別のデータベース）の両方で、この四つを確かめる。キーを申請と `round` に戻した帳簿では、別のデータベースの形で 2 と 3 が成り立たない。

**帳簿が別のデータベースにあるとき。** 帳簿の操作とチケットの保存を、一つのトランザクションにできない。サーバーは、次の二つを約束にする（フレームワークが持つ物。12.5）。

- **チケットごとの鍵。** 一つのチケットへの操作は、そのチケットの鍵（PostgreSQL なら、チケットの行の `SELECT … FOR UPDATE` か勧告的ロック）を取ってから読み、保存し終わるまで持つ。保存が失敗してやり直すあいだも放さない。同じチケットへの操作は一つずつ、直前に保存したチケットを読む。
- **保存が失敗したら、取り消す前にやり直す。** 帳簿の操作のあとで保存が失敗したら、すぐには取り消さない。鍵を持ったまま、チケットと組織の部分を読み直し、gate からやり直す（`now` は、最初に受け取ったときの時刻のまま渡す）。やり直した操作は同じ試みなので、同じ帳簿の操作を持てば、帳簿は同じキーに `done_before`（前に済んだ）を返し、保存できれば二つはそろう。やり直した答えが同じ仮押さえを持たないとき（gate か `next` が断るか、別の答えになったとき）は、保存しなかったその試みのキーで `void` する。

操作の種類ごとの順は、次のとおりである。

- `hold`：帳簿のキーで仮押さえ、チケットを保存する。帳簿が断れば、保存せずに `book` で断る。保存が失敗したら、上のとおりやり直す。
- `post`：保存の前に確定する。帳簿の断り（期限切れ、取り消し済み）でイベントを断れるからである。保存が失敗したら、上のとおりやり直す。やり直した操作は同じ確定に来て、`done_before` を受ける。
- `void`：保存のあとに、知らせと同じに、保存した一覧からキーで行い、済むまでやり直す。取り消しは戻せず、取り消したキーではもう押さえられない（`done_before` を返し、何も押さえない）ので、保存の前には行わない。保存しなかった試みの仮押さえの取り消しは、上のとおり、その試みのキーで行う。あとの試みは別のキーなので、取り消しに止められない。

この約束で塞がるのは、次の三つである。

- 同じチケットへの二つの操作が、互いの仮押さえに当たること。仮押さえのキーは試みごとなので、ある試みの `void` は、ほかの試みの仮押さえを取り消さない。鍵があれば、後の操作は先の操作が保存したチケットを読むので、同じ `round` に二つ目の仮押さえを作る遷移にも来ない（仮押さえを作るのは `submit` と、`round` を進める `resubmit` である）。
- 保存できなかった操作をやり直すと、チケットは押さえたつもりで、帳簿は何も押さえていないこと。取り消したキーではもう押さえられないので、取り消してからやり直すとこうなる。取り消す前にやり直すので、やり直した操作は、まだ開いている仮押さえに `done_before` で当たる。
- 確定のあとで保存が失敗し、やり直す前にほかのイベント（同時の却下や取り下げ）が保存されて、確定した仮押さえと承認されていないチケットが残ること。鍵を持ったままやり直すので、あいだにほかのイベントは保存されない。

残るのは、次の三つである。

- 帳簿の操作のあとで、保存もやり直しもしないままサーバーが止まったとき。仮押さえなら、期限切れが最後の手当てになる。確定なら、確定した仮押さえと進んでいないチケットが残り、同じ操作が同じ冪等キーで届き直すまでそろわない（届き直せば、`done_before` を受けて保存できる）。
- やり直した答えが、最初の答えと違うとき。チケットと時刻は同じなので `next` は同じに答えるが、gate と組織の部分は読み直すので、そのあいだに変わっていれば、答えが変わる。仮押さえは取り消せばよいが、確定は取り消せないので、確定した仮押さえと進んでいないチケットが残る。
- `void` が済むまでのあいだ、帳簿がチケットより長く押さえて見えること（済まなければ、期限切れが最後の手当て）。

**適合。** サーバーの試験にする物は三つある。

- ベクタの一行ごとに、渡された物（前のチケット、イベント、組織の部分、時刻）から、同じチケットとすることの一覧、または同じ種類の断りを返す。サーバーが 7.5 のとおりに切った部分は、行の `organization` と同じになる。比べるとき、値が null のキーと無いキーは同じと見て、オブジェクトのキーの順は問わない。断りは種類（`refused`）と場所（`at`）を比べ、文（`text`、`text_ja`）は比べない。履歴の `note` は語まで、`comment` は渡された文のとおりに比べる。
- 探す操作のベクタ（`organization/find.jsonl`。12.2）の一行ごとに、行の `values` だけを人、チーム、マスタの行（`valid_from` と `valid_to` の日とともに）として持ち、行の `q`、`limit`、`lang`、`on`（null でない行だけ）で尋ねて、同じ答え（`found` の並びと、それぞれの `value`、`name`、`label`、`more`）を返すか、`answer` が `bad` の行なら 400 で答える（13.2）。
- 同じチケットに同時に操作を流し、次を確かめる。(a) 保存した履歴の順にイベントを `next` に流し直すと、保存したチケットと同じになる。(b) することの冪等キーが重ならない。(c) 断られた操作は、チケットを変えていない。(d) 答えが返った時点で、チケット、履歴、することの一覧が保存されている。(e) 保存されていない操作の知らせは出ない。

**約束の外の値。** 約束の形の JSON には、生成したコードと参照インタプリタ（10.1）が同じに答える。値が null のキーは、無いキーと同じに読む。申請では、そのフィールドを入れていない。再申請と承認者の直しでは、チケットが持つ値をそのまま残し、直しの履歴の `to` にも挙げない。値を空にするのは、イベントの `clear` に挙げた名前だけである（7.3。こう分ける理由も 7.3 に書いた）。フォームの値は、渡された順によらず、渡したフィールド、計算する数、数える日付の順に、それぞれ宣言の順で持ち、flow に渡す値と直しの `to` も同じ順で書く。明細の行のセルも宣言の順で、値の無いセルは書かない。二つが違うのは、約束の外の物を渡したときだけで、次の表のとおりである。参照インタプリタは JSON を読まないので、右の列は、同じ値を `Machine::start` と `next` に渡したときの答えである。

| 渡した物 | 生成したコード | 参照インタプリタ |
|---|---|---|
| フォームに無いフィールドのキー、計算するフィールドのキー、明細の行に無いフィールドのキー（スキーマの `additionalProperties: false` の外） | 読まない。キーが無いときと同じに答える | `form` で断る（`the form has no field …`、`… is computed, and is not given`、明細は `the value of … does not fit the field's type`） |
| 型の違う値（テキストのフィールドに数、日付でない文字列、配列でない明細）。フォームの外でも、型の違う値、無いキー、約束に無い語とイベント、JSON として読めない文 | 入口のエラー（12.2）。四つの出力先が同じ一つの文で、値の場所、渡された物、要る物を言う（「The argument `submit.form.days` of `start` is "three", and must be an integer」。文の全部は 12.2 の「入口のエラー」） | フォームの値は `form` で断る（`the value of … does not fit the field's type`）。チケット、イベント、組織は型の値で受け取るので、型の違う値は渡せない |
| 機械の数の上限（12.1）を越える値（`round` が 1,000,000 を越えるチケットなど） | 入口のエラー（12.2） | 上限を持たず、そのまま答える |
| 申請の種類が数えるより多い営業日を数える値（`pause_left` が `pause` の日数より大きいチケットなど）と、ステップが始まりうる日の外の日から営業日を数える値 | `data` で断る（`the business days of the calendar … cannot be counted from …`）。つなぎが営業日を数える日付のコード（12.2 の `expense_open`）は、その日々だけを数える | カレンダーのデータの中なら、数えて答える。外なら `data` で断る（文が違う） |

### 12.5 P5 と、ritsu の外に置くフレームワーク

rulec の P5 は、実行時のランタイムを持たず、読めるコードを生成すると決めている。ringi が生成するのも、時計も保管も持たない純粋な関数のライブラリで、この決まりに沿う。チケットを保管し、時計を回し、知らせを送る物は要るが、それは生成したコードの一部ではなく、それを使う別の製品（サーバー）として、ritsu のリポジトリの外に置く。ritsu の中に置けば、ritsu は生成したコードを動かす物まで持つことになり、P5 を守れない。サーバーは ritsu が書き出す約束だけに頼り、約束はベクタで確かめる。

外に置く物は、言語ごとのフレームワークにする（2026-11-15）。生成したパッケージを渡せば、申請の種類の一覧（12.2）から全部の申請の種類が動き、組織ごとに違う所だけを差し替える。フレームワークが申請の種類ごとの型を知らずに済むのは、一覧の入口が約束の形の JSON を受け渡すからである。

- **持つ物**：申請の種類によらない一つのコード。12.4 の表の、サーバーがすることの全部（イベントの受け口、一件ずつ当てること、一つのイベントの順、帳簿が別のデータベースにあるときのチケットごとの鍵と保存のやり直し、gate に尋ねること、期限と催促の時計、することを冪等キーで行うこと、組織の版を渡すこと、画面の配信、MCP サーバー、管理の画面と、反映と取り込みの記録（14 章））と、その適合の試験。
- **差し替える物**：組織ごとに違う所。認証、保管、通知、添付の置き場、組織のデータの取り込み元、flow の動かし方、エージェントのステップ、監査ログの出し先。

一つ目の言語は Go で、`net/http` の `http.Handler` として出す。本体は PostgreSQL だけで動き、flow を始める申請の種類があるときだけ、dandori の temporal-go の出力先を使う。flow のプラグインは別の Go のモジュールにする。二つ目からの言語と形は、Go が済んでから決める。プロジェクトの最初の一式を書き出すコマンドは ritsu の側に置く（`ritsu ringi init`、16.4）。

## 13. 画面

### 13.1 A2UI 1.0 のメッセージの作り方

`ritsu gen --target requests` は、申請の種類一つに、申請書、承認カード、承認待ちの一覧、自分の申請の一覧、履歴と、「いつから申請できるか」のお知らせの六つの画面を、英語と日本語で書く（`a2ui/<名前>/{form,card,list,mine,history,offered}.{en,ja}.json`）。画面は、フォームと経路の宣言から機械的に作る。確かめで手書きした申請書と承認カード（標準カタログだけで書け、公式のスキーマも通った）を作るのに要った情報は、フィールドの型と範囲と文言、条件、明細の行、ステップの並びと今の位置、ボタンが送る値で、どれもフォームと経路の宣言にある。

画面は値を入れる前のテンプレートで、三つのメッセージからなる。面を作る `createSurface`、値の無いデータの形を渡す `updateDataModel`、部品を渡す `updateComponents` である。サーバーは、チケットの値をデータの置き場（13.8）に入れて送る。面の既定のカタログは A2UI の標準カタログで、ritsu のカタログ（13.2）の部品と関数の呼び出しは、`@catalogId` と `catalogId` に ritsu のカタログの ID を書く。面の ID は `<名前>.<画面>`（`expense.card`）である。A2UI は、面の ID がレンダラの中で一つに決まることを求めるので、同じ画面を二つ出すとき（二つのチケットの承認カード）は、サーバーがチケットの番号を足して分ける。ファイルの最初のキー `//` は、ritsu が生成したファイルの頭である（12.2）。

| フォームの型 | 申請書 | 承認カード |
|---|---|---|
| 短文、長文 | `TextField`（`shortText`、`longText`） | `Text` |
| 整数、単位の付いた数 | `ritsu:AmountField`（人が数える単位で入れ、単位の刻みの整数を書き込む。下の表） | `Text` と、単位の一つを刻みにする金額は `formatCurrency`（円は小数 0 桁を必ず書く）、単位の無い数は `formatNumber`。ほかの数（率、セントや銭の金額、量）は ritsu の `formatSteps` |
| 日付 | `DateTimeInput`（日付だけ） | `Text` と `formatDate` |
| 選択 | `ChoicePicker`（値は配列で来るので、一つ目を読む） | `Text` と `ritsu:lookup`（コードを文言に） |
| マスタから選ぶ（選ぶ値をファイルに書かないもの） | `ritsu:MasterPicker` | `Text` と `ritsu:lookup`（`/masters/<フィールド>` の名前に） |
| 添付 | `ritsu:FileAttachment` | 添付の値（ファイルの ID）と、開くボタン（`openUrl` で、`/files` の表がその ID に持つ URL を開く。明細の行の添付も同じ表を引く） |
| 人、チーム | `ritsu:PersonPicker` | `Text` と `ritsu:lookup`（`/people` の名前に） |
| 真偽 | `CheckBox` | `Text` と `ritsu:lookup`（はい、いいえ） |
| 明細 | `ritsu:LineItems`（行を足し、消す） | `List` と行のテンプレート |
| `shown when` | `ritsu:Show(when, child)` | 同じ |
| 経路の今の位置 | — | `ritsu:ApprovalRoute(stages, current)` |

申請書の部品は、フィールドの範囲（数と日付の `min` と `max`、明細の行の数の `min` と `max`）を持ち、フィールドの検査を `@checks` に持つ（13.2 の `<申請>.check`）。

**数の見せ方**：数のフィールドは、JSON に単位の刻みの整数を持つ。人が入れて読むのは、人が数える単位（円、ドル、パーセント、キログラム）の値である。小数の桁は、刻みのどの倍数も正確に書ける一番少ない桁にし、0 でも書く（`rate[step 0.1%]` の範囲の 0 から 1000 は `0.0%` から `100.0%`）。整数の部分は、5 桁以上なら 3 桁ずつコンマで区切る（`9999`、`10,000`）。単位は、率は `%`、金額は通貨の ISO 4217 のコード（日本語の画面の円は `円`）、量は単位の綴り（日本語の画面は数字に続けて書く）を付ける。

| 型 | 刻み一つ | 入れる値と見せる値 | JSON の 125 |
|---|---|---|---|
| `number` | 1 | `125` | `125` |
| `money[JPY]`、`money[円]` | 1 円 | `125` と `JPY`（日本語は `円`） | `125 JPY`、`125円` |
| `money[銭]` | 1/100 円 | `1.25` と `JPY` | `1.25 JPY`、`1.25円` |
| `money[USD]` | 1 ドル | `125` と `USD` | `125 USD` |
| `money[USDc]` | 1/100 ドル | `1.25` と `USD` | `1.25 USD` |
| `rate`（`rate[step 1%]` と同じ） | 1% | `125` と `%` | `125%` |
| `rate[step 0.1%]` | 0.1% | `12.5` と `%` | `12.5%` |
| `rate[step 0.25%]` | 0.25% | `31.25` と `%` | `31.25%` |
| `rate[step 2.5%]` | 2.5% | `312.5` と `%` | `312.5%` |
| `mass[kg]` ほかの量 | 単位の 1 | `125` と `kg` | `125 kg`、`125kg` |

`AmountField` は、小数の桁（`decimals`）と、刻み一つを小数の最後の桁で数えた整数（`step`。1 なら書かない。`rate[step 0.25%]` は小数 2 桁の 25、`rate[step 2.5%]` は小数 1 桁の 25）と、通貨（`currency`）か単位（`unit`。率は `%`）を持つ。入れた値は、10^`decimals` 倍して `step` で割った整数として書き込む。刻みの倍数でない値は丸めずに断る。12.34 と入れた人の意図は、0.1% の刻みの画面には分からないからである。見せるほうは、単位の一つを刻みにする金額と単位の無い数が、これまでどおり標準カタログの `formatCurrency` と `formatNumber` で、ほかの数は ritsu の `formatSteps(value, decimals, step, unit)` である。`formatSteps` は、整数だけで計算するので（JavaScript の `BigInt`）、画面が持つどの値も正確に書く。フィールドの検査の文が言う範囲も、同じ書き方である（`Share は-10.00%から150.00%までにしてください。`）。フレームワークのページも同じ表で書く（数の見せ方を変えるときは、二つを一緒に変える）。

これまで、率は小数の桁を刻みの分母の 10 の桁で数え（`rate[step 0.25%]` を小数 3 桁）、セントと銭の金額は小数 0 桁で見せていた（`money[USDc]` の 125 を `125 USD`）。どちらも、送る値と見せる値の刻みが合わず、W702 も、小数の桁だけを比べていたので言わなかった。いまの W702 は、小数の桁と刻みの両方を比べる（13.3）。

承認カードと一覧の、数、金額、日付、添付の値は、値があるあいだだけ見せる（`ritsu:Show` に入れ、ラベルは残す）。申請書の計算するフィールドの値も、フォームが計算できるまで見せない。標準カタログの書式の関数（`formatCurrency`、`formatNumber`、`formatDate`）は、null を受け取ったときの形を決めていない。null を 0 として書式にするレンダラでは、まだ無い合計が「0 円」に見えるので、どの画面も、この関数に null を渡さない。

六つの画面には、次のものを置く。

| 画面 | 置くもの |
|---|---|
| 申請書（`form`） | 題、代わりに申請する相手（その人が申請の代理として出せる相手がいるときだけ。5.8）、差し戻した人（差し戻されたチケットのときだけ）、申請者が入れるフィールド、計算するフィールドの値、コメント、ボタン（申請する。差し戻しのある種類では、もう一度申請する、申請をやめる） |
| 承認カード（`card`） | 題、チケットの番号と状態、申請者の名前（代わりに出した申請なら、代わりに出した人の名前も）、経路、全部のフィールドの値、承認者が直すフィールドと保存のボタン（承認者が直すフィールドのある種類）、コメント、その種類が持つ操作のボタン（承認、却下、差し戻し、承認の取り消し、保留、保留を解く、取り下げ、申請をやめる、承認者を加える、引き継ぐ）、コメントだけを送るボタン。差し戻す先（前のステップにも戻せる種類）と、引き継ぐ前と後の承認者は、ボタンの横の部品で選ぶ。承認者を加えるときは、加える人を部品で選び、今のステップの、承認者を待っている部分ごとに置いたボタン（部分の名前を決め方の文言で見せる）を押す。押したボタンの部分を、API の `part` として送る。待っている部分が一つなら、ボタンも一つで、部分は選ばない |
| 承認待ちの一覧（`list`） | 題と、チケットごとの行。行には、番号、状態、ステップ、申請者、短いフィールド（明細、長文、添付と、条件で見せるフィールドを除く）と、チケットを開くボタンを置く。行の下に、次のページがあるあいだだけ、次のページのボタンを置く（13.8 の `/more`）。サーバーは `list_waiting` の答えを入れる |
| 自分の申請の一覧（`mine`） | 題と、チケットごとの行。行には、番号、状態、ステップ、代わりに出した人（代わりに出した申請だけ）、短いフィールドと、チケットを開くボタンを置く。行の下の次のページのボタンは `list` と同じである。サーバーは `list_mine` の答え（その種類の物）を入れる。申請する人がいちばんよく開く画面である |
| 履歴（`history`） | 題とチケットの番号と、履歴の行ごとの、時刻、したこと、ステップ、した人、だれの代わりか、コメント、空になったフィールド、メモ |
| お知らせ（`offered`） | 題、「いつから申請できるか」の文（英 "This request may be made from 2027-04-01."、日「この申請は2027-04-01から出せます。」。日付は文の `${/from}` に入る）、ホームへ戻るボタン。効いている版が無い種類の申請書を開いた人に、申請書の代わりに出す。サーバーは、出せるようになる日を `/from` に、戻る先のアドレスを `/home` に入れる（13.8）。ボタンはイベントを送らず、画面の側でアドレスを開く（`openUrl`）ので、出す人を名指す操作は要らない |

申請書と承認カードは、ボタンが送った操作が断られたとき、その断りを見せる（`/refused`。13.8）。コメントの断り（種類 `comment`）はコメントを書く部品の下に、フィールドを名指さない断りは画面の頭に、文を見せる。フィールドの断りを見せる所は、画面とフィールドの種類で次のとおりで、どのフィールドの断りも、どこにも見せないことはない。

| 画面 | フィールド | 断りを見せる所（部品の ID） |
|---|---|---|
| 申請書 | 申請者が入れるフィールド（明細も） | そのフィールドの下（`field.<名前>.refused.shown`） |
| 申請書 | 画面が求める計算（`worked_on_screen`。例の経費と購買の合計） | その値の下（`field.<名前>.refused.shown`） |
| 申請書 | 画面に置かないフィールド：承認者だけが入れるフィールドと、機械だけが求める計算（休暇の、数えて求める日付） | 画面の頭（`refused.shown`） |
| 承認カード | 全部のフィールド（カードは全部のフィールドを置く） | そのフィールドの値の下（`field.<名前>.refused.shown`） |

この表は `crates/ringi/tests/a2ui.rs` の `a_refusal_of_every_field_is_shown_where_the_table_says` が、例の全部の申請書と承認カード（英日）の、全部のフィールドの断りを描いて確かめる。

理由を求める操作（`comment required for`。7.2）のボタンは、送るコメントが無いあいだ（空か、空白だけ）、送らずに「理由をコメントに書いてください。」（Write the reason in the comment.）を見せる（カタログの `commentGiven`。13.2）。断られてから見せる形にしないのは、コメントの長さ（`commentFits`）と同じく、画面で分かることは押す前に言う決まりにそろえるためである。空白は、機械と同じ Unicode の White_Space の 25 文字で読む（13.7）。どちらの形でも、機械は同じ文を `comment` で断るので、画面の検査を回避した送り方でも、理由の無い操作は通らない。

ボタンは、どれも `ritsu:Show` に入れ、データの `/can/<操作>` が真のあいだだけ見せる。その人に今できる操作かは、サーバーが gate に尋ね（12.4）、gate が許す操作を `next` が今のチケットでその人から受けるかを確かめて入れる。gate だけでは決まらない。gate が言うのはだれがしてよいかで、チケットが受けるかではないからである。終わったチケットの申請者も申請者のままで、gate だけに尋ねると、押せば必ず断られるボタンが出る。`next` の断りのうち、チケットの状態やその人がだれかによる物（`state`、`person`、`event`、`limit`）でボタンを隠し、書いた物やデータによる物（`form`、`comment`、`data`、`book` など）では隠さない。隠すと、できない理由がどこにも出ないからである。ボタンが送るイベントの名前は、API の操作の `operationId` である（`submit_expense`、`approve`。一覧の行のボタンは `get_ticket`、次のページのボタンは `list_waiting` か `list_mine`）。`context` の `ticket` がパスのチケット（`{id}`）で、ほかのキーが操作の本文になる。サーバーは、申請の種類ごとのコードを持たずに、イベントを操作に変えられる。

### 13.2 ritsu のカタログ

標準カタログに無い部品と関数を、ritsu のカタログとして定義する（`a2ui/catalog.json`。A2UI 1.0 のカタログの形）。部品は、どの申請の種類でも同じである。関数は、どの種類も使うもののあとに、種類ごとのものを並べる。関数の中身は `a2ui/functions.js` にある（13.7）。

| 名前 | 種類 | 意味 |
|---|---|---|
| `Show` | 部品 | `when` が成り立つ（false、null、空の文字列、空の配列のどれでもない）あいだだけ、子を見せる。隠れたフィールドは送らない |
| `LineItems` | 部品 | 明細の行のテンプレートと、行を足すボタンと消すボタン。行の数を `min` から `max` までに保つ |
| `AmountField` | 部品 | 人が数える単位で入れた数を、単位の刻みの整数として書き込む（10^`decimals` 倍して `step` で割る。刻みの倍数でない値は断る）。通貨（ISO 4217 のコード）か単位（率は `%`）を付けて見せる。範囲を持つ（13.1） |
| `FileAttachment` | 部品 | ファイルを選んでサーバーに預け、サーバーが返した ID を書き込む。預け先は `upload`（データの `/upload`。13.8）で、`file` という一つの部分の `multipart/form-data` を `POST` し、`{"id": …}` を受け取る。その人が預けて、まだどのチケットにも付いていないファイルが、サーバーの上限の数だけあれば、`Retry-After` の無い 429 を受け取る（12.4） |
| `PersonPicker` | 部品 | 組織の人（`of` が `team` ならチーム）をサーバーで探して（`find`。データの `/find`。13.8）、ID を書き込む。`among` があれば、その ID の人だけから選ぶ |
| `MasterPicker` | 部品 | サーバーが配るマスタ（`master` は `/masters/` の下のパス。`company_masters/vendors`）の行をサーバーで探して（`find`）、キーを書き込む |
| `ApprovalRoute` | 部品 | ステップの並び、承認者、答え、今の位置。ステップと決め方の文言を、画面の言語で持つ |
| `lookup(value, table, separator)` | 関数 | コードを文言にする（表に無ければコードのまま）。配列なら一つずつ文言にして、`separator` でつなぐ |
| `equals(a, b)` | 関数 | 等しいか |
| `formatSteps(value, decimals, step, unit)` | 関数 | 単位の刻みの整数 `value` を、人が数える単位の文字列にする。`value` × `step` を小数 `decimals` 桁で書き（0 も書く）、整数の部分が 5 桁以上ならコンマで区切り、`unit` をそのまま後ろに付ける（`31.25%`、`1.25 USD`、`12,000 kg`）。null は空の文字列（13.1） |
| `numberFrom(text)` | 関数 | 文字列の数字を数にする |
| `one(values)` | 関数 | 選択の値（`ChoicePicker` は配列で書く）を一つにする。選んでいなければ null |
| `comment(text)`、`commentFits(text)`、`commentGiven(text)` | 関数 | 送るコメント（両端の空白を除いた文。空白は Unicode の White_Space の 25 文字で、機械と同じ。何も残らなければ null）、それが 2,000 字以内か、送るコメントがあるか（理由を求める操作のボタンが読む。13.1） |
| `lastOf(values, key)` | 関数 | オブジェクトの配列 `values` の最後の物の `key` の値。配列が空か、最後の物にそのキーが無ければ null。一覧の次のページのボタンが、ページの最後のチケットの ID を `before` にするのに使う（13.8） |
| `<申請>.check(form, code, row, filling)` | 関数 | フォームが検査 `code` を通るか。`code` は `<フィールド>.required`（いつも必須、見せているあいだ必須、`required when` が成り立つあいだ必須のどれも）、`.choice`（選ぶ値が選択肢の一つ）、`.count`、`.range`、明細の列は `<明細>.<列>.…`（`row` があればその行だけ）、計算した値の範囲は `<フィールド>.worked`。隠れているフィールドの検査は通す。`filling` が真なら、入れているあいだに明細の部品の下に出す検査で、行の数が下限に届かないことは言わない（申請するボタンの検査は `filling` 無しで止める。13.7）。計算した値の範囲は、計算が読む明細の行が下限に届くまで照らさない（13.7）。文はペイロードの `CheckRule.message` に置く |
| `<申請>.given(form)` | 関数 | 申請するときに送るフォーム。見えていて空でないフィールドを、API が取る形で（文は両端の空白を除く。13.7） |
| `<申請>.changes(form, before)`、`<申請>.cleared(form, before)` | 関数 | もう一度申請するときの、前の値から変えたフィールドと、空にした（か隠れた）フィールドの名前。差し戻しのある種類だけ |
| `<申請>.edited(edit, before)` | 関数 | 承認者が直したあとのフォーム。承認者が入れるか直すフィールドは `edit` の値、ほかは `before` の値。直す部品の表示の条件と検査は、このフォームで読む。承認者が直すフィールドのある種類だけ |
| `<申請>.edits(edit, before)`、`<申請>.edits_cleared(edit, before)` | 関数 | 承認者が入れたか変えたフィールドと、空にしたか、直したあとのフォームで隠れたフィールドの名前。承認者が直すフィールドのある種類だけ |
| `<申請>.shown_<フィールド>`、`<申請>.required_<フィールド>`、`<申請>.<計算するフィールド>` | 関数 | 表示の条件、必須の条件、計算するフィールドの値（下ろした規則から。13.7） |

**値を探すアドレス。** `PersonPicker` と `MasterPicker` は、`find`（データの `/find`。サーバーが決めて入れる。13.8）の下で値を探す。人は `<find>/people`、チームは `<find>/teams`、マスタの行は `<find>/masters/<master>`（`MasterPicker` の `master`）に、`GET` で尋ねる。API の探す操作（`find_people`、`find_teams`、`find_master_<マスタのファイル>_<マスタ>`。12.4）と同じ物で、比べ方と並べ方は下に一度だけ書く。

| 引数 | 意味 |
|---|---|
| `q` | 打った言葉（200 字まで。字はコードポイントで数える）。言葉が無ければ、どの値も見つかる |
| `limit` | 答える数。1〜100 で、無ければ 20 |
| `lang` | 名前の言語（`en` か `ja`）。無ければ、`Accept-Language` のうち最初の `en` か `ja`、それも無ければ `en` |
| `on` | マスタの行を選べる日（`YYYY-MM-DD`）。API の `find_master_…` だけが受け、無ければ今日である。画面は送らない |

答えは `{"found": [{"value": "e1203", "name": "Sales staff 3", "label": "Sales staff 3 (e1203)"}], "more": false}` の形である。`value` は見つかった値（人とチームの ID、マスタの行のキー）、`name` はその `lang` の名前（無ければ空の文字列）、`label` は一覧に出す形の名前、`more` は、答えた数のほかにも見つかった値があるか、である。人とチームの `lang` の名前は、`ja` なら `name_ja`（空なら `name`）、`en` なら `name` で、探すときはもう一方の言語の名前も読む。マスタの行の名前は、マスタの `name_field` の列の一つで、どちらの言語でも同じに読む。`label` は、名前のあとに値を括弧に入れて書く（`en` は `Sales staff 3 (e1203)`、`ja` は全角の括弧で `営業担当 3（e1203）`）。名前が空か、値と同じなら、値だけである。マスタの行は、その日（`on`。無ければ今日）に選べる物だけを相手にする。

**比べ方。** 言葉と値は、次の四つの段で直した文字列で比べる（`find::fold`）。

1. NFKC にする（Unicode 17.0.0 の表）。全角の英数字は ASCII に、半角のカタカナは全角に（濁点と半濁点は前の字と一つの字に）、`①` は `1` に、`㈱` は `(株)` になる。
2. Unicode の `White_Space` の字（U+0009〜U+000D、U+0020、U+0085、U+00A0、U+1680、U+2000〜U+200A、U+2028、U+2029、U+202F、U+205F、U+3000 の 25 字）を除く。NFKC が作る空白（全角の空白 U+3000 は U+0020 に、`¨` は U+0020 と結合の字になる）も、ここで除く。
3. ひらがな（U+3041〜U+3096）を、同じ音のカタカナ（U+30A1〜U+30F6。コードポイントに 0x60 を足す）にする。NFKC はひらがなとカタカナを別の字のままにするので、別の段に置いた。
4. 一字ずつ、UnicodeData.txt の単純な小文字の対応（一字を一字にする対応で、前後の字も言語の決まりも読まない）にし、そのあと語末のシグマ `ς` を `σ` にする。大文字のシグマ `Σ` は、語末でも `σ` になるので、`ΑΛΕΞΗΣ` と `αλεξης` は同じになる。`İ`（U+0130）は `i` に、`ẞ`（U+1E9E）は `ß` になる。case folding ではないので、`ß` は `ss` にならない。

`q` は、`White_Space` の字で言葉に切り、言葉ごとに直す（直して空になる言葉は、無い物とする）。値は、どの言葉も、値（ID かキー）か名前のどれかを直した文字列に含まれれば見つかる。

**並べ方。** 見つかった値を、次の順位の小さい順に並べ、同じ順位の中では値（ID かキー）をコードポイントの順に並べる。そのはじめの `limit` 個を答え、`more` は、`limit` より多く見つかったかである。

| 順位 | 値 |
|---|---|
| 0 | 値が、`q` から両端の `White_Space` を除いた物と、直さずに等しい |
| 1 | 値を直した物が、`q` の全体を直した物（言葉のあいだの空白も除いた物）と等しい |
| 2 | 名前のどれかを直した物が、`q` の全体を直した物と等しい |
| 3 | 値か名前のどれかを直した物が、最初の言葉を直した物で始まる |
| 4 | ほかの見つかった値 |

**引数の誤り。** `q` が 200 字を超えるか、`limit` が 1〜100 の外か、`lang` が `en` でも `ja` でもないか、マスタの `on` が `YYYY-MM-DD` の形の日付でなければ（無い日の `2026-02-29` も）、400 で答える。二つ以上が合わなければ、`q`、`limit`、`lang`、`on` の順に見て最初に合わない引数を言う。ベクタは、その名前を `{"bad": "q"}` の形で持つ。どの申請の種類も選ばないマスタなら 404 である。

**サーバーの言語で書くとき。** どの段も、言語の標準の関数で書ける。ただし、次の関数は、そのままでは上の決まりと違う答えを出す。

| 段 | Go | TypeScript | Python | Rust |
|---|---|---|---|---|
| 言葉に切る、空白を除く、両端を除く | `unicode.IsSpace`。`strings.Fields` と `strings.TrimSpace` も同じ字を読む | 正規表現の `\p{White_Space}`（`u` の印を付けて）。`\s` と `trim` は、U+FEFF を空白とし、U+0085 を空白としないので使わない | 25 字を書いた正規表現。`str.split()`、`strip()`、`re` の `\s` は、U+001C〜U+001F も空白とするので使わない | `char::is_whitespace`。`split_whitespace` と `trim` も同じ字を読む |
| NFKC | `golang.org/x/text/unicode/norm` の `norm.NFKC.String` | `normalize("NFKC")` | `unicodedata.normalize("NFKC", …)` | `unicode-normalization` の `nfkc` |
| 小文字 | 一字ずつ `unicode.ToLower`（`strings.ToLower` も同じ）。`golang.org/x/text/cases` の `Lower` は、`İ` を二字にし、語末のシグマを前後の字で決め、`Fold` は `ß` を `ss` にするので使わない | 一字（コードポイント）ずつ `toLowerCase()` を呼び、二字になる `İ` は `i` にする。文字列の全体の `toLowerCase()` は、語末のシグマを前後の字で `ς` にする | 一字ずつ `lower()` を呼び、二字になる `İ` は `i` にする。文字列の全体の `lower()` は、語末のシグマを前後の字で `ς` にし、`casefold()` は `ß` を `ss` にする | 一字ずつ `char::to_lowercase` を呼び、二字になる `İ` は `i` にする。`str::to_lowercase` は、語末のシグマを前後の字で `ς` にする |
| 字の数 | `utf8.RuneCountInString`（`len` はバイトの数） | `[...q].length`（`length` は UTF-16 の単位の数） | `len` | `chars().count()`（`len` はバイトの数） |
| 値の順 | 文字列の `<`（UTF-8 のバイトの順は、コードポイントの順と同じ） | コードポイントで比べる。文字列の `<` は UTF-16 の単位の順で、U+10000 からの字が U+E000〜U+FFFF の字より前に来る | 文字列の `<` | 文字列の `<` |

参照は Unicode 17.0.0 の表で直す（ritsu の NFKC の表、`ritsu_emit::normal`）。標準の関数が前の版の表を持つ言語では、その版のあとに足された字だけが違いうる（`golang.org/x/text` は、v0.42.0 で、Go 1.27 からは 17.0.0 の表を使う）。

ベクタ `organization/find.jsonl`（12.2）は、直す段の一つずつ（全角の英字、半角のカタカナと濁点、`①`、`㈱`、ひらがなとカタカナ、語末のシグマ、`İ`、`ẞ`、`ß` と `ss`、全角の空白）、標準の関数が違って読む空白（U+0085 は空白、U+FEFF と U+001C は空白でない）、順位と同じ順位の並び（データの順でも UTF-16 の単位の順でもなく、コードポイントの順であること。`ｚ`（U+FF5A）は `𠮷`（U+20BB7）より前に来る）、名前が値と同じときの `label`、`limit` の端と `more`、空の `q`、200 字と 201 字の `q`（U+10000 からの字なので、UTF-16 の単位では 400 と 402 になる）、`lang`、マスタの行を選べる日（`on` の日が `valid_from` か `valid_to` の日でも選べる）、二つの引数が同時に合わないときに言う引数を、場合ごとに一行で持つ。サーバーは、行の `values` だけを相手にして（マスタの行は、`on` の日に選べる物だけを）、同じ答えを返す（12.4 の適合）。

カタログの ID は、そのプロジェクトのカタログを指す名前で、自分の持つ URL の形で書く。`catalog.json` を配る場所と同じにしておくと分かりやすいが、同じである要は無い。環境が変わっても同じ値を使う。生成物は環境ごとに作り分けない。A2UI 1.0 の仕様も、カタログの ID を、カタログを一つに決める名前の文字列とし、その URI に何かが置いてある要は無いとしている。`ritsu gen --target requests` に `--catalog <url>` で渡すか、モジュールなら ritsu.mod に `catalog "<url>"` の行を書く。両方あれば `--catalog` を使う。テンプレート、カタログ、`functions.js`（`export const catalogId`）は、同じ ID を書く。

画面はカタログを ID で選ぶので、一つのサーバーや画面が一緒に読むパッケージ（別のプロジェクトから作った物も）は、それぞれ別の ID にする。同じ ID の二つは、どちらの部品と関数で描くかが決まらない。ritsu は、プロジェクトをまたいで同じになる ID を自分では決めない。`ritsu gen` には既定の ID が無く、渡し方を言って止まる（下）。`ritsu ringi init` が `--catalog` の無いときに書く ID は、`--module` の Go のモジュールのパスから作る `https://<モジュールのパス>/a2ui/catalog.json` で、モジュールが違えば ID も違う。

ID が無ければ、`ritsu gen` は渡し方を言い、何も書かずに 2 で終わる。http か https の URL で、ホストがあり、ユーザー、クエリ（`?` から後）、フラグメント（`#` から後）が無いものでなければ、断って 2 で終わる。ritsu.mod の行は、同じことを E302 で言う。`requests` の無い `ritsu gen` に `--catalog` を渡しても 2 である。カタログはキーの決まったファイルなので、ritsu が生成したファイルの頭を `description` に書く。

### 13.3 見せた値と送る値の突き合わせ

確かめで使った突き合わせ（各ボタンの `context` と `userMessage` から `@path` を集め、同じパスを画面の部品が見せているか）を、`ritsu gen --target requests` が書くどの画面にもかける。根の部品から子をたどり、たどり着いた部品だけを読む。`ritsu:Show` の子と、`List` と `ritsu:LineItems` の行のテンプレートもたどる。行のテンプレートの相対パスは、行ごとのパス（`/form/items/*/price`）として読む。

ボタンでない部品は、`@checks` を除くプロパティが読むパスを、見せる。ボタンは、`context` と `userMessage` が読むパスを、送る。送る値を作る関数（`<申請>.given`、`.changes`、`.cleared`、`.edits`、`.edits_cleared`）は、読むパスでなく、送るフィールド（`form` か `edit` の下の、申請者か承認者が入れるフィールド）で数える。比べる相手の `before` は、送るものに数えない。チケットの番号と版（`/ticket/id`、`/ticket/revision`）は、どのチケットのどの状態への操作かを言う値で、人が決めた値ではないので数えない。

見せていない値を送れば E701、見せるか読む値の刻み（小数の桁と、刻み一つを最後の桁で数えた整数）が送る値の刻みと違えば W702 である。どちらも、テンプレートを書く前に、申請の最初の行で言う。E701 なら何も書かず、W702 は警告で、書く。テンプレートは宣言から作るので、出るのは ringi の不具合のときだけである（9.5）。承認カードは、種別の文言を別のパスに置かず、`lookup` で同じパスから見せるようにテンプレートを作る（確かめで NOT SHOWN になった形を作らない）。`sendDataModel` は使わず、ボタンの `context` に送る値を並べる。

画面は数を JavaScript の数で持つ。2^53 − 1（9_007_199_254_740_991）を越える整数は丸まり、別の値として確かめて送ってしまう。数のフィールドの範囲の端が、単位の刻みで数えてそれを越えれば E704 である。申請者が入れるフィールド、承認者のフィールド、計算するフィールド、明細の列のどれでも言う。E703 と同じく、`ritsu gen --target requests` が画面を作る前にフィールドの行で言い、何も書かない。範囲の中の値は、どれも画面が正確に持てる。範囲の検査を通った値は、書いたとおりに送られる（13.7）。

E701、E703、E704 で止まる申請は、`ritsu check` を通る。止まるのは画面を作る `--target requests` だけで、言語の出力先は書ける。そこで `ritsu gen` の最後の行は、その申請を検査を通らないファイルに数えず、ほかの言語で出力先が書けないファイルと同じ形で言う（dandori の E040、E050 など）。

明細の列の範囲が広すぎる申請（`parts.ringi`。試験の小さな組織 `tests/mutants/shared/small.ringi` を読む）：

```ringi
request parts v1 "Parts"

use organization org from "small.ringi"

form
  items : rows "Items"  count >=1 <=10
    what   : text "What"  count <=100
    serial : number "Serial number"  range >=1 <=99_999_999_999_999_999

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
  submitted by   team sales
```

```text
$ ritsu check parts.ringi
ok parts.ringi: a request, 1 field, 1 stage
  used from 2026-04-01: the first version of the organization `small.ringi` (data `org-small`)
  used with no end: no data it reads ends
  may never end: the stage `check` has no deadline (`due in …`), and waits until it is answered
  submitted by 2 of 4 people in the organization from 2026-04-01 (`team sales`)
ritsu check: 1 file (ringi 1): all pass; borders between the languages: 0 checked, 0 undecided
$ ritsu gen parts.ringi --target requests --catalog https://requests.example.com/a2ui/catalog.json
error[ringi E704]: parts.ringi:8:1: The range of `items.serial` reaches `99_999_999_999_999_999`, past what a screen holds exactly (`9_007_199_254_740_991`)
     8 |     serial : number "Serial number"  range >=1 <=99_999_999_999_999_999
  = A screen holds a number as JavaScript does, exactly only from `-9_007_199_254_740_991` to `9_007_199_254_740_991`: a value past it would be rounded, and checked and sent as another. Narrow the range to within it.
error: `parts.ringi` cannot be made into the requests package
```

日本語の実行では「エラー: `parts.ringi` からは requests のパッケージを生成できません」である。同じ実行に検査を通らないファイルもあれば、`ritsu gen` は先にそちらで止まり、画面までは確かめない。

### 13.4 文言

テンプレートは言語ごとに作る（`form.en.json`、`form.ja.json`）。`CheckRule.message` が、言語ごとに一つの文字列しか持てないためである。フィールド、選ぶ値、ステップ、決め方、申請の題の文言は、4.8 のとおり `labels <言語>` の文言で、無ければファイルに書いた題である。規則の列挙から選ぶ値はフィールドの下に書いた題か、ファイルの言語で書いた規則の表示名、共有の経路のステップと決め方は、経路の言語の名前か、経路の `labels` の題で見せる（4.8）。宣言した言語で文言が欠けていれば E703 で、`ritsu gen` は何も書かない。ボタン、状態、履歴のしたこと、検査の文の型は、ringi が英語と日本語で持つ。

英語の名前で書き、`labels ja` の無いファイル（機械の試験の申請。例はどれも両方の言語の文言を持つ）の日本語の画面は、英語の題を日本語の文に入れる。ASCII の文字（英字、数字、記号）で終わる題のあとと、文の途中に来る、ASCII の文字で始まる題の前には、空白を置く（`Kind を選んでください。`、`Lines の Price は1円から1000円までにしてください。`、`Supplies with values left empty の履歴`。`tests/a2ui.rs` の `an_english_label_in_a_japanese_sentence_is_set_apart`）。

金額と日付の整形のロケールは、レンダラの側で画面の言語に合わせて渡す（ペイロードには書けない）。

### 13.5 版の固定と名前の手当て

ringi は A2UI 1.0 の仕様のとおりの名前（`@checks`、`@accessibility`、`@metadata`、`@catalogId`）で書く。確かめのときに公開されていた web のレンダラ（npm の 0.15.0）は、この名前に変わる前の版で、仕様どおりのペイロードを描かなかった。サーバーは、使うレンダラの版を固定し、その版が古い名前しか読まなければ、部品の直下の四つの名前の `@` を外してから渡す手当てを持つ。新しい名前を読むレンダラが出たら、手当てを外す。手当てはサーバーの物で、ritsu の生成物は仕様のとおりのまま変えない。

ritsu の試験（`crates/ringi/tests/a2ui.rs`）は、A2UI のリポジトリから写した 1.0 のスキーマ（`crates/ringi/tests/fixtures/a2ui-1.0/`。Apache License 2.0 の文と NOTICE を添えた）で、全部の画面とカタログを検証する。ritsu のカタログは、A2UI 自身の検証と同じく、標準カタログと合わせた一つの `catalog.json` として読ませる。検証するのは、試験の中に書いた JSON Schema 2020-12 の読み手である。写したスキーマが使うキーワードだけを読み、知らないキーワード、パターン、形式（`format`）に会えば止まるので、読み落としたまま通ることは無い。`format` は、注釈でなく検査として読む（日付と時刻は RFC 3339 の形）。

### 13.6 代わりの形

六つの画面のどれも、素の HTML の画面にできる。サーバーは、`types/<名前>.json` と `a2ui/functions.js` とチケットから作る。ブラウザだけで承認する人のための承認カードも、その一つである。A2UI の画面は、チャットやアシスタントの中に出す物で、レンダラが落ち着かないあいだは、承認カードのように、そこに出す画面から使う。どちらの形でも、フォームの検査と自動計算は同じ関数を使い（13.7）、どのフィールドを画面で計算するかは、説明のフィールドの `worked_on_screen` が言う（12.2）。素の HTML の画面は、条件つきのフィールドを見せるか、必須か、計算の値を、ブラウザで関数を動かさずに、入口 `worked` の操作（12.4）でサーバーに尋ねてもよい。答えは画面の関数と同じ決め方による（13.7）。断りをどのフィールドの横に出すかは、断りの `at` が言う（12.4）。

### 13.7 同じ決まりを三か所で動かす

フォームの検査、表示と必須の条件、自動計算は、下ろした規則が元である。検査と参照インタプリタは `form.rule` を、生成するコードはその写しの `values.rule` を読み、計算した値を書いた範囲と比べるのは申請ごとのつなぎである（4.4、2026-11-15）。サーバーの側は、`values.rule` をサーバーの言語にしたコードを、つなぎが呼ぶ。

画面の側の関数（`a2ui/functions.js`）は、ビルドの道具なしにブラウザがそのまま読む ES モジュールである。申請の種類ごとの `values.rule` を rulec がページのために書く JavaScript（サーバーの側の TypeScript から型を外したもの）を、種類ごとの関数に包んで入れる。この JavaScript は、ritsu が下ろしたファイルを検査したあと、rulec がその検査で覚えたものから作るので、規則も、規則が読む `choices.rule` も、もう一度は検査しない（ritsu の DESIGN 4.15）。コードの中の文（入力の誤りを言う文や注釈）は、実行の言語によらず英語で書くので、このファイルはどの実行でも同じになる。フィールドの検査（入れたか、選択肢の一つか、字の数（コードポイント）か行の数、範囲）は、`types/<名前>.json` と同じ宣言から作ったフォームの説明を、どの種類にも同じ一つのコードが読む。計算した値を書いた範囲と比べるのは、つなぎと同じ出力を読んでする。

画面は、条件と計算を、出力ごとに、それが読むフィールドだけで決める（4.9）。出力が読む引数にはフォームの値を、読まない引数には範囲の中の値（仮の値。数は下限を刻みで割り切れる値、日付は下限、選択は最初の値、真偽は false、明細は行の数の下限だけの行）を入れて、`values.rule` の関数を呼ぶ。`values.rule` は和の範囲のほかは範囲に照らさず、表は網羅されていて、割る数の範囲は 0 を含まないので、読まない引数の値が何であっても、求める出力は変わらず、投げもしない。出力ごとの読む物は、下ろすときに規則から集める（`types/<名前>.json` の `reads` と同じ物。12.2）。値の無いことがあるフィールドは、機械と同じに空として読み、隠れているフィールドも空として読む。

出力が決まらないのは、読むフィールドのうち申請者が必ず入れるフィールドが空のとき、読む値が範囲の外のとき、読む明細の行が上限より多いとき（どれも、そのフィールドの検査が誤りを言う）である。サーバーのコードが写しに上限までの行しか渡さないので、画面もそれより多い行からは求めない。読む明細は、そのときある行で読む（読む列のうち必ず入れる列が空の行は、まだ無い行として数える）。行の数が下限に届かないあいだも、ある行から求め、行が一つも無ければ合計は 0 である。決まらないあいだ、`shown when` のフィールドは隠し、`required when` のフィールドは必須にせず、計算の値は見せない（0 も見せない）。空の申請書では、条件つきのフィールドは見えない。種別を選べば、その種別で見えるフィールドが現れる。行を埋めていくあいだは、埋まった行の合計が見え、合計を読む条件も、その合計で決まる。

入れている途中には、誤りを見せない。明細の部品の下の検査（`check` の `filling`。13.2）は、行の数が下限に届かないことを言わない。計算の値を範囲に照らすのは、計算が決まり、その計算が読む明細の行の数が下限に届いてからである。申請するボタンは、行の数が下限に届かなければ止める。

画面の検査は、機械がフォームを読むとおりに読む。いつも必須のフィールド、見せているあいだ必須のフィールド（`shown when` があり、`optional` の無い物）、`required when` が成り立つフィールドは、空なら止める。隠れているフィールドは検査せず、送らない。選択のフィールドは、選択肢に無い値を止める。部品は選択肢に無い値を作らないが、データは外から入ることがある。数の範囲の検査は、画面が正確に持てない数（`Number.isSafeInteger` でない数。13.3）を範囲の外として止める。

計算の値が範囲の外のときの文（`<フィールド>.worked`）は、範囲を言ったあとに、申請者が直すフィールドを挙げる。機械の断り（4.4）と同じく、計算が読むフィールド（`reads`）を挙げ、ほかの検査と同じく、見る人の言語の題で言う。どれも同じ明細のセルなら、明細の名前を一度だけ添える。購買の明細の小計では次のとおりで、サーバーのページが機械の断りを言い直す文とも同じである。

```text
The Subtotal (Lines) the form works out is outside its range (from 1 JPY to 100,000,000 JPY). Change Quantity or Unit price (Lines).
計算した明細の小計が、範囲（1円から100,000,000円まで）の外です。明細の数量か単価を直してください。
```

画面のほうが厳しい所が一つある。文のフィールド（短文、長文、明細の行の文）は、コメントと同じく、両端の空白を除いて読む。空白は、機械が理由の空白を読むのと同じ Unicode の White_Space の 25 文字（kaname の `trim` の空白）で、JavaScript の `trim` は使わない（U+FEFF を除き、U+0085 を残すので、理由を求める操作で、画面と機械の判定が分かれる）。何も残らなければ、値が無いのと同じに読み（必須なら止め、任意なら送らない）、残れば、除いた文を送り、字の数もその文で数える。機械は渡された文をそのまま持つので、空白だけの必須の文を、機械は受け付け、画面は止める。両端に空白のある文は、機械は空白ごと持ち、画面は空白を除いて送る。

画面で確かめられない物もある。マスタから選ぶフィールドのキー（行はサーバーのデータにある）、人とチームのフィールドの ID が組織にいるか、`open in` の日付がカレンダーの開いている日か、である。どれも、サーバーが `next` で確かめて `form` で断る。

下ろしたファイルは、言語がほかの物として読む語を別の名前で書く（6.4）。規則は入力を並びの順に受け取るので、フィールドの名前が違っても呼び方は変わらない。行のフィールドは名前で渡すので、フォームの説明が、規則の側の名前と並べて持つ（`count` の行の `max` は `max_value`）。選択の値も名前で渡す。rulec が別の物として読む値（`after`、`by`）は、規則には `after_value`、`by_value` で渡す。フォームの説明は、名前の違う値だけを、規則の側の名前と並べて持つ（`"after": "after_value"`）。関数は、フォームの値をその名前に直して規則を呼ぶ。

どちらも、参照インタプリタと突き合わせる。`crates/ringi/tests/a2ui.rs` は、例の 10 の申請の種類のベクタから、フォームを 422 個取り出して、Node で関数に通す。参照インタプリタが受け取る 360 個は、画面のどの検査も通り、`<申請>.given` が送る値は参照インタプリタが受け取った値と同じで、計算した値（172 個）も同じになる。参照インタプリタが断る 62 個（フォームの断りが 52 個、コメントの長さの断りが 10 個）は、画面のどれかの検査（コメントの長さは `commentFits`）で止まる。選択の値を規則が別の名前で書く申請（値 `after`、`by`、`usual` のそれぞれにシナリオを一つ持つ）でも、同じことを 39 個のフォームで確かめる。

ベクタに無い形は、フォームを作って確かめる（`tests/a2ui/forms.rs`）。`types/<名前>.json` から、フィールドごと、検査の種類ごとに、その検査だけを破るフォームと、端でちょうど通るフォームを作る。検査の種類は、必須、見せているあいだ必須、`required when` の成り立つ側と成り立たない側、隠れているフィールドが値を持つ形、字の数と行の数の両端、数と金額と日付の範囲の両端、無い日付、選択肢に無い値、明細の行の中のそれぞれ、計算した値の範囲の両端である。もとにするのは、参照インタプリタが受け取る申請、差し戻して出し直す申請、承認者が直すチケットで、出し直しと直しでは、フィールドを一つずつ空にする形と変える形も作る。画面は、レンダラが読むとおりに読む（描かれる部品の検査と、押すボタンの検査）。ボタンが送る物を参照インタプリタに渡し、どのフォームでも判定が同じで、両方が通すときは、画面が送った値と画面が見せたフォームをチケットが持つことを確かめる。例の 10 の種類と、例に無い形のために書いた申請一つ（見せているあいだ必須の文、明細の中の選択と添付、画面が正確に持てる一番大きい整数、0.1% 刻みの率、承認者が直す選択とそれが見せるフィールドなど）で 934 のフォームを作り、両方が止めるのは 414 個、判定が分かれるものは無い。文の両端の空白で分かれうる 61 個は、参照インタプリタに書いたままの文も渡して数える。ボタンが作るメッセージ（934 個）は、どれも A2UI 1.0 のレンダラからサーバーへのメッセージのスキーマ（`renderer_to_agent.json`。`tests/fixtures/a2ui-1.0/` に写した）を通る。テンプレートから検査を一つ抜くと、この試験が落ちる。

申請書を人が埋めていく順も作って確かめる（`tests/a2ui/steps.rs`）。例の全部と、機械の試験の四つの申請（`absent`：空のことがある値を読む条件と計算、`ranges`：行の数と範囲の端、`steps`：条件が計算を読む形、計算が計算を読む形、行の中の値を条件が読む形、`supplies`：表示の条件が表示の条件を読む形）で、フィールドを一つずつ埋め（下限、下限の次、上限、ベクタの値。選択は全部の値）、行を空で足して一つずつ埋め、二行目を足し、一行目を消し、全部埋めたあとで各フィールドを変える、空にする、行を全部消す。その各時点で、見えるか、必須か、計算の値、範囲の検査を、読むフィールドだけを渡した参照インタプリタと比べる。行の数が下限に届かない時点で、下ろしたフォーム（`form.rule`）が入口で断るとき（合計がその範囲の下限より小さい。例の経費と購買では、行が無い時点の合計 0）は、画面が読む写し（`values.rule`）を同じ評価器で求めた答えと比べる（`engine::eval::values_outs`）。2,191 の答えが同じで、12 は参照インタプリタがほかの理由で断るので比べない。読まないフィールドを空にしても埋めても、22,235 回とも答えは動かない。読む引数かセルを一つ減らす変異と、必ず入れる値を一つ足す変異の 75 個は、どれもこの試験が見つける。読む明細のフィールドを一つ足す形は、行をあるとおりに渡すので答えが変わらず、変異に数えない。

三つ目の所は、申請の種類の一覧の入口 `worked`（12.2）である。サーバーとエージェントが、画面を持たずに、途中まで埋めたフォームで、どのフィールドが見えて必須か、計算の値がいくつかを尋ねるためにある（API の操作は 12.4）。参照は ringi のライブラリの `engine::worked::answer` で、生成したコードはつなぎの `worked_out`（kaname）から作る。決め方は画面と同じである。出力ごとに読むフィールドだけで決め、読まない引数には画面と同じ仮の値を渡す。隠れているフィールドと、値の無いことがあるフィールドは、空として読む。申請者が必ず入れるフィールドが空のとき、読む値が範囲の外か選択肢に無いとき、読む明細の行が上限より多いときは、その出力は決まらない（kaname のつなぎは、写しの和が範囲に収まるよう、行を上限までしか渡さない形で書く）。文は画面と同じ空白を両端から除いて読み、真偽のフィールドは、無いか null なら false と読む（チェックしていないチェックボックスと同じ）。明細は、ある行で読み、行の数が下限に届かなくても、ある行から求める。承認者の直しは、直せるフィールドだけをフォームの値と入れ替え、承認者だけのフィールドの `required` は false である。

画面の求めない値は、機械が求めるとおりに求める。明細の行の中の計算は、その行だけを写しに渡して求め、行の読むセルが範囲の外か、必ず入れるセルが空なら、その行の値は決まらない。計算する日付は `form.cal` で数える。

`crates/ringi/tests/a2ui/worked.rs` は、例の全部と機械の試験の四つの申請（`absent`、`ranges`、`steps`、`supplies`）の `worked` のベクタを Node で画面の関数に通し、見せるか、必須か、画面が計算する値（`worked_on_screen`）を比べる。615 のフォームで 1,818 の答えが同じである。そのうち 22 のフォームは、読む明細の行が上限より多く、画面も `worked` も、その行を読む出力を決まらないと答える。`crates/ritsu/tests/gen_requests.rs` は、四つの出力先の入口 `worked` が、どのベクタでも参照と同じ答えを返すことを確かめる。

Node が無ければ `SKIP:` の行を出す。画面の検査は利用者が回避できるので、サーバーは、受けた値を同じ規則でもう一度確かめる（`next` の `submit` が確かめる）。

### 13.8 データの置き場

サーバーは、画面のデータの次の置き場に値を入れて送る。どの置き場も、テンプレートの `updateDataModel` が値の無い形で持っている。サーバーが値を入れなければならない置き場を足すときは、約束の版を上げる（12.4）。値は API と同じ形で、フィールドは ASCII の名前、選択はコード、数は単位の刻みの整数、日付は `YYYY-MM-DD` である（12.4）。

| 置き場 | 画面 | 中身 |
|---|---|---|
| `/form` | 申請書、承認カード | フォームの値。申請書では人が入れる値で、選択は一つだけの配列（`ChoicePicker` が配列で書くため）、明細は行の配列にする。承認カードでは、チケットの値 |
| `/before` | 申請書 | もう一度申請するときの、チケットが持つ値。変えたフィールドと空にしたフィールドを、この値と比べて送る |
| `/requester`、`/submit_for` | 申請書 | 代わりに申請する相手と、その人が申請の代理として出せる人（その日に有効な `proxies.csv` の `submit` の行の人で、`submitted by` に当たる人。5.8、5.13）の ID の配列。配列が空なら、相手を選ぶ部品は出ず、`/requester` は null のまま、自分の申請になる。もう一度申請するときは空にする |
| `/may_submit` | 申請書（`submitted by` を書いた種類） | その人が自分で出せるか（5.13）。出せない人も、代わりに申請する相手を選べば申請できる。どちらでもなければ、申請するボタンは「この申請は、自分の申請としては出せません。」で止める。相手を選んだ申請の相手が出せない人なら、機械が `submit` で断り、その文が相手を名指す |
| `/ticket` | 申請書、承認カード、履歴 | チケットの番号（`id`）。申請書と承認カードは版（`revision`）も持ち、申請書は差し戻した人（`sent_back_by`）、承認カードは状態（`status`）、ステップ（`stage`）、申請者（`requester`）、代わりに出した人（`made_by`）も持つ。サーバーは、API のチケットのキーを `/ticket` の下にそのまま入れてよい |
| `/route` | 承認カード | 経路（チケットの `stages`） |
| `/edit` | 承認カード | 承認者が入れるか直すフィールドの値。サーバーが、チケットの値で埋めて送る。直したあとのフォームは、このフィールドを `/edit` から、ほかを `/form` から読むので、埋めずに送ると、値のあるフィールドを空にした直しになる |
| `/comment` | 申請書、承認カード | 書いているコメント。空の文字列から始める |
| `/refused` | 申請書、承認カード | ボタンが送った操作の断り：`text`（画面の言語の文。API の答えの `text` か `text_ja`）、`field`（フォームの断りの `at` の最初の名前。`items[1].price` なら `items`。フィールドの断りでなければ null）、`comment`（断りの種類が `comment` なら真）。断られていなければ、`text` と `field` は null、`comment` は偽である。画面は、`field` があればそのフィールドの横に、`comment` が真ならコメントを書く部品の下に、どちらでもなければ画面の頭に、文を見せる。サーバーは、断りの答えを入れて画面を送り直す |
| `/upload` | 申請書、承認カード（添付のフィールドのある種類） | 添付を預けるアドレス（`FileAttachment` の `upload`。13.2）。サーバーが決めて入れる |
| `/find` | 申請書、承認カード（人、チーム、マスタの行を選ぶ部品のある画面。申請書はいつも） | 値を探すアドレス（`PersonPicker` と `MasterPicker` の `find`。13.2）。サーバーが決めて入れる |
| `/send_back`、`/add_approver`、`/hand_over` | 承認カード | 差し戻す先（一つだけの配列。空の文字列は申請者）、加える承認者（`/add_approver/approver`）、引き継ぐ前と後の承認者 |
| `/add_approver/parts` | 承認カード | 今のステップの、承認者を待っている部分（チケットの `stages` の今のステップの `parts` のうち、`state` が `active` か `stopped` の物）ごとに一つの `{"part": "<決め方の ASCII の名前（部分の way）>"}` の配列。サーバーが入れる。行ごとに承認者を加えるボタンが一つ出て、押したボタンの行の `part` を、API の `part` として送る |
| `/can` | 申請書、承認カード | 操作ごとに、その人に今できるか（gate が許し、`next` が受けるか。13.1）。申請書の `submit` だけが真から始まる |
| `/people` | 全部 | 画面に出る人とチームの名前（ID ごと） |
| `/masters/<フィールド>` | 承認カード、二つの一覧 | マスタから選ぶフィールドの値の名前（キーごと。マスタの `name_field` の列。12.2）。一覧は、行に出すフィールドの分だけ |
| `/files` | 承認カード | 添付を開く URL を、ファイルの ID ごとに（`{"<ID>": "<URL>"}`）。フォームと明細の行のどの添付も、この表を引く |
| `/tickets` | 承認待ちの一覧、自分の申請の一覧 | チケットごとの `id`、`status`、`stage`、`requester`（承認待ちの一覧）か `made_by`（自分の申請の一覧）、`form`。一覧の答えの一つのページの物 |
| `/more` | 承認待ちの一覧、自分の申請の一覧 | 一覧の答えの `more`（このページのあとにもチケットがあるか）。偽から始まる。真なら、一覧の下に次のページのボタンが出る。ボタンは、`/tickets` の最後のチケットの ID（`lastOf`。13.2）を `before` にして、同じ一覧の操作（`list_waiting`、自分の申請の一覧なら `list_mine` と、その画面の申請の種類の `kind`）をイベントとして送る。サーバーは、その答えのページで画面を送り直す |
| `/history` | 履歴 | 履歴の行（`at`、`did`、`stage`、`by`、`proxy_of`、`comment`、`cleared`、`note`）。`at` は、サーバーが日時の文字列にして入れる |
| `/from` | お知らせ | その申請を出せるようになる日（`YYYY-MM-DD`）。文の `${/from}` の所に入る（13.1 の `offered` の表）。サーバーが入れる。null のあいだ、文は出ない |
| `/home` | お知らせ | ホームへ戻るボタンが開くアドレス（サーバーの始めのページ）。サーバーが決めて入れる。null のあいだ、ボタンは出ない |

## 14. 管理する人の作業

### 14.1 依頼から反映まで

1. 管理する人が、変えたいことを普通の言葉で頼む（「購買で 500 万円を超えたら財務部長も承認する」）。
2. エージェントが `.ringi` を直し、`ritsu check` を通す。
3. `ritsu ringi diff` が、どの申請の、どのステップの、だれが変わるかのページを作る。組織のデータがあれば、変わる人を名前で並べる。いちばん長い日数の前後も載せる。組織の新しい版なら、`ritsu ringi diff --versions <今の版の日> <新しい版の日> <申請>` が、承認者が変わる申請者とステップ、出せる人の変わりを並べる（11 章）。
4. 頼んだ人がページを読み、確かめた記録を yuen に残す（`ritsu yuen review` は人が走らせる）。規程の条と表、経路、期限の対応が変われば、yuen が確かめ直しを求める（E301）。
5. 反映する。申請の種類は、新しい版を生成したパッケージをサーバーに配り、その版がいつから申請を受けるかを決める（14.2）。組織は、新しい版のデータを取り込む（14.2）。

反映はフレームワークの管理の画面（`/pages/admin/`。同じ操作は API の `/admin/…` にもある）でする。画面が見せるのは、フレームワークが知っている物である：配った版と、それぞれを使える日の範囲と始まり、取り込むデータと今のデータの差、記録。版のあいだや組織の版のあいだで経路や承認者がどう変わるかは、3 の ritsu の道具が言う。始まりを決めたことも取り込んだことも、だれが、いつ、何をどう変えたか（理由を書けばそれも）を記録に残す。記録は変えも消しもせず、チケットの動きと並べて新しい順に読む（`/pages/admin/records`）。管理の操作は約束（`openapi.json`）に無く、エージェントの道具にもならない。チケットと組織を見た人が、自分で決めて行う物だからである。

### 14.2 版と予約

申請の種類の版は、最初の行の `v<版>` で、変えるたびに上げる。新しい版は、生成したパッケージに入れてサーバーに配り、前の版と並べて動かす。新しい申請は、その時に効いている版のうち、いちばん高い版で受ける。進行中のチケットは、出したときの版で終わる（7.7）。始まりを決め直しても、チケットも時計も変わらない。

版が効き始める時は、説明が言う時である。`effective from 2027-04-01` を書けばその日の始まりから、書かなければパッケージを配った時から効く。管理する人は、管理の画面で、これを次の四つのどれかに決める。

| 決め方 | 版が効き始める時 |
|---|---|
| 今すぐ | 決めた時 |
| 日付 | その日（今日か先の日）の始まり。日は申請の種類の日（7.5）で読む |
| 説明に戻す | 説明が言う時。前に決めたことを取り消す |
| 止める | 効かない。どの新しい申請もこの版を取らない |

決められるのは、その版を使える日の範囲（9.6）の中だけである。範囲の外の日には、生成したコードがどの申請も `data` で断るからである。だから、版を説明の日より早めることはできず、遅らせるか止めるかになる。前の版に戻すには、新しい版を止めるか、先の日付へ送る（低い版を今すぐにしても、高い版が効いているあいだは取られない）。版が一つの種類を止めると、その種類は一覧から消え、新しい申請を受けない。効いている版が無いあいだ（始まりがまだ先か、止めた）、その種類の申請（`submit_<名前>`）と途中のフォームの答え（`worked_<名前>`）は 404 で答え、その種類を出せる人には、後の版が効き始める日を添える（12.4）。

管理する人に日を決めさせたい版には、`effective from` に、いちばん早く始めてよい日を書く。書かない版は配った時から新しい申請を受けるので、管理する人が止めるまでのあいだにも申請が入りうる。

組織の版は、データの版のディレクトリを、効き始める日の名前で置く（8.3）。サーバーへの取り込みは二通りである。

- **起動のとき**：サーバーに組み込んだデータ（ソース）が、前にソースから取り込んだ物と違うときだけ取り込む。
- **動いているサーバー**：管理の画面で、ソースを読み直すか、zip を上げる。zip には、プロジェクトの根（`ritsu gen` の `--root`）からのパスのまま、組織のディレクトリ（版と `proxies.csv`）と、申請の種類が読むマスタのファイルをまとめる。上げる物は、`ritsu ringi check` を通したファイルそのものである。サーバーは起動のときと同じ確かめをかけ、今のデータとの差を日ごとに見せ、管理する人が差を読んで確かめると取り込む。

上げて取り込んだデータは、ソースが変わるまで、起動し直しても残る。同じ版は、プロジェクトの組織のディレクトリにも足す。ソースを変えて配ると、起動のときにソースのデータを取り込み、上げたデータはそれに置き換わるからである。

取り込んだデータは、そのあとのイベントから効く（見られる人、代理、gate が見つける部門、知らせの名前と宛先）。進行中のステップの承認者は始まったときのままで、新しいデータは次に始まるステップから効く（8.7）。データを先へ延ばしても、申請の種類を使える日の範囲は延びない。範囲は生成したコードが持つので、延ばすにはパッケージを生成し直す。

### 14.3 部門ごとに任せる範囲

申請の種類は `owner <チーム>` で受け持つ部門を言う。だれが何を管理できるかは会社全体の決まり（sekisho の gate）が言い、サーバーは管理の操作を受け付ける前に gate に尋ねる。尋ねる操作は四つである。

| action | resource | 尋ねる操作 |
|---|---|---|
| `change_request_type` | 申請の種類の版（`RequestType`。属性 `owner` はその版を受け持つチーム） | 版の始まりを決める（14.2） |
| `read_audit` | 申請の種類の版 | その版の記録を読む：チケットの動き（フォームの値も）と、始まりを決めた記録 |
| `import_organization` | 組織（`Organization`） | 組織のデータを確かめて取り込む |
| `read_organization_audit` | 組織 | 組織の記録（取り込み）を読む |

gate がどれかを宣言していなければ、`ritsu check` が X18 の E911 で止める（9.3）。宣言の無い action はだれにも許されず、その操作はだれもできないからである。サーバーは gate に人と resource と時刻だけを渡すので、入力（`input`）を受け取る action も E911 である。

受け持つチームを変える版（前の版と別のチームが受け持つ版）の始まりは、両方のチームで gate が許すときだけ決められる。記録は、版ごとにその版を受け持つチームで尋ねるので、部門は自分が受け持った版の記録を読む。`read_audit` を許された人は、その版のチケットの中身を読める。gate が許さない人には、その版も組織のページも、無い物として答える（404）。

例の gate（`examples/shared/company.gate` と、`ritsu ringi init` が書く `company.gate`）は、役割を四つ持つ。

- `staff`：申請を出す。受け持つ部門の人は、その種類のチケットを見る（12.4）。
- `workflow_admin`（部門の管理者）：自分の部門が受け持つ版の始まりを決め、その記録を読む。自分の部門の申請の種類で、承認者を足し、引き継ぐ。
- `system_admin`（全体の管理者）：四つの操作を条件なしで。受け持つチームの無い申請の種類も。
- `auditor`（監査する人）：二つの記録を読む。何も変えない。

三つの役割はどれも `staff` を含む。期待は次のことを確かめる。
- 全体の管理者でない部門の管理者は、ほかの部門の版を変えない。
- 全体の管理者でも監査する人でもない部門の管理者は、ほかの部門の記録を読まない。
- 全体の管理者のほかは、組織を取り込まない。
- 管理者の役割の無い監査する人は、何も変えない（版の始まり、取り込み、承認者の追加と引き継ぎ）。
- 全体の管理者は、組織を取り込める。

`examples/shared/company.gate` は、停止中の人が管理の操作をしないことも確かめる。

役割はサインインが与える（プロバイダーのクレーム、プロキシのヘッダー、手元では `-dev-login` のページで選ぶ）。生成した gate は、宣言していない役割や、その principal の型が持てない役割を一つでも持つ人を、どの操作でも断る。そこで `init` の `gate.go` は、生成したコードの `RolesOf` に principal の型で尋ねて、Staff が持てる役割だけを残して尋ね、外した役割は名前ごとに一度だけ（100 個まで）ログに書く。持てる役割が一つも残らない人は `staff` として尋ねる。

部門の比べ方（`init` の `gate.go`）。gate が読む人の部門（`principal.department`）は、問いの日の組織の版で、その人の主の所属のチームから上へたどり、申請の種類を受け持つチームと同じ種類で最初に見つかるチームである（フレームワークの `TeamOf`）。課が受け持つ申請の種類は課で、部が受け持つ物は部で、本部が受け持つ物は本部で比べる。組織のデータがチームの種類を宣言した名前で書いても、書いた名前で書いても、同じ答えになる。受け持つチームの無い問い（組織の操作）では、主の所属のチームを部門とする。

組織に居ない人と、受け持つチームを書かない申請の種類は、どの部門の物にもしない。問いの日の組織の版に主の所属の無い人は gate が読まないので、どの操作も断られる。全体の管理者も、組織の版に居なければ何もできない。その日に効いている版が無ければ、だれも管理の画面で取り込めない（起動のときの取り込みは gate に尋ねない）。受け持つチームを書かない申請の種類には、その版のどのチームの ID とも違う ID（`(no owner)`。同じ ID のチームがあれば括弧を重ねる）を受け持つチームとして渡し、空の文字列は渡さない。どの部門とも一致しないので、部門を読まない規則（全体の管理者と監査する人の物）だけが、その種類の管理を許す。

`ritsu ringi diff` は、変わった申請の種類の受け持つ部門をページの頭に出し、部門をまたぐ変更が混ざっていないかを読めるようにする。

## 15. エージェントのステップと、dandori とのつなぎ

### 15.1 エージェントが答えるステップ

`check = agent precheck_agent, else manager` は、ステップに着いたら dandori の flow を始め（`ask agent`）、その出力を答えにする。flow の出力は `verdict`（`ok` と `doubt` を持つ列挙）と `reason`（文）でなければならない（X20）。`ok` ならステップは通り、履歴に `agent ok` の行を残す（`note` に flow の名前、`comment` にエージェントが書いた理由。7.4）。`doubt` か失敗なら、`else` の人のステップになり、承認カードに理由（その行の `comment`）を出す。エージェントは却下しない。却下するのは人である。エージェントが決裁してよい範囲は、経路の表が言う（経費の 3 万円以下の出張）。その範囲は主張 `decide is travel_agent only when amount <= 30_000JPY` で確かめる。

dandori のタスクの形（`agent` のタスク、型を付けた応答、タイムアウト、リトライ、失敗のときの値）をそのまま使う。答えの型は dandori が確かめ、ringi はその型が `verdict` と `reason` であることだけを見る。

### 15.2 要約

`summary  note from summarize` は、ステップに着いたら flow を始め、出力をあとの承認カードに付けて進む。何も決めない。期限までに答えが無ければ、要約なしで進む。

要約が持てる値は、文（`str`）、整数（`num`）、真偽（`truth`）、行（`rows`。オブジェクトの並び）、無い値（`nothing`）だけである（12.4）。flow の出力は、そのどれかになる型でなければならない：文（`string`、`date`、`timestamp`、列挙）、整数（`int` と、単位の付いた数。単位で数えた整数で運ぶ）、真偽（`bool`）、行（レコードの `list` で、レコードのフィールドもこれらの型）、それらの `T?`。一つのレコード、レコードでない物の `list`、`json` は、実行してから要約が付かずに失敗するので、`ritsu check` が X20 の E210 で言う（「a note does not hold a record on its own」。一つのレコードは、一行の `list[…]` にする）。要約は、出力を `note ready` に渡された順に持ち、画面もその順に見せる。サーバーは、説明の `flows` の出力の順（flow の宣言の順。12.2）に並べて渡す。dandori の Temporal の flow のうち、Go と TypeScript の物は答えの JSON を宣言の順に書き、Python の物は名前の順に書く（SDK の JSON の変換の既定。dandori の DESIGN 4.2）。

### 15.3 承認のあとに flow を始める

`on approved launch pay(request_id: ticket.id, requester: requester.id, amount: amount, purpose: purpose)` は、承認されたら flow を始める。サーバーは、flow を始めるワークフローの型とタスクキューを、申請の種類の説明の `flows`（12.2）から読む。帳簿の確定は ringi が自分でする（`on approved post budget.reserve(…)`）。dandori は、ほかで始めた仮押さえを確定できないからである（試しでは dandori の E013「the case `hold` has not been started here」）。

### 15.4 dandori の flow の途中で、人の承認が要るとき

flow は、サーバーの「チケットを開く」操作（`submit_<名前>`）を、`use openapi` で `ritsu gen --target requests` の `openapi.json` を読んだ `http` のタスクで呼ぶ。dandori は、タスクの引数と答えのレコードを操作の本文と答えのスキーマに突き合わせ、フォームの値の範囲も比べる（12.4）。休暇を出して、開いたチケットの ID と状態を出力にする flow（`crates/ritsu/tests/gen_requests.rs` の `trip.flow`）は、dandori の検査を通る（2026-11-15）。dandori は OpenAPI の `format: date` の文字列を `date` として読まないので、その flow は日付を `string` で渡す。

最初の版の flow は、チケットを開いたところで終わる。チケットが終わったあとの続き（承認されたら発注する、など）は、申請の種類の `on` の行が、別の flow として始める（`on approved launch …`。15.3）。チケットの終わり方（`approved`、`rejected`、`withdrawn`、`expired`）とフォームの値を、開いた flow に返して同じ flow の中で待つ形は、後の版に回す（2.2）。dandori の `callback` のタスクはトークンを `lambda` か `aws sqs:sendMessage` でしか渡さず、HTTP の操作にトークンを渡してサーバーがコールバックで返す形は、dandori に手を入れなければ書けない（dandori の E007「a `callback` task hands its token over by `lambda` or by `aws sqs:sendMessage`」）からである。flow が呼ぶ操作が今の申請のものかは、X20 の反対の向きが確かめる（9.3 の「flow がチケットを開く側」）。文書が今の申請のものなら、渡すフォームの値の型と範囲は、dandori が文書と比べる。

## 16. 入口

### 16.1 コマンド

| コマンド | すること |
|---|---|
| `ritsu ringi check <path>… [--org <dir>] [--at <日付>] [--through <日付>] [--root <dir>]` | 検査。パスはファイルかディレクトリで、ディレクトリは下の `.ringi` の全部（下ろしたディレクトリ `*.lowered` と、名前が `.` で始まるものは除く）。同じファイルを二度渡しても一度だけ確かめる。組織のデータは組織のファイルの `data`、または `--org`。`--at` はその日に効いている版だけを確かめる（8.3）。`--through` はその日まで使えることを求める（9.6）。`--root` は JSON に書くパスを数えるルート（ほかの言語と同じ） |
| `ritsu ringi run <file> [--scenario <名前>] [--org <dir>] [--format json]` | シナリオを参照インタプリタに流し、行ごとにチケットの進み方を見せる（10.2）。`--scenario` が無ければ、ファイルのシナリオの全部 |
| `ritsu ringi simulate <file> (--requester <id> \| --every-requester) [--form …] [--on <日付>] [--org <dir>]` | 申請者を選んで経路を試す（10.3） |
| `ritsu ringi doc <file> [--format markdown\|html] [--longest [--on <日付>]] [--org <dir>] [--at <日付>] [--through <日付>] [--root <dir>]` | 申請の種類、共有の経路、組織、マスタのページを標準出力に書く（11 章）。`--format` は、Markdown（既定）か、外から何も読まない一つの HTML。`--longest` は、いちばん長い場合のページ（11.3）。検査を通らないファイルには、`check` と同じ診断を出し、ページを書かずに 1 で終わる。`--org`、`--at`、`--through` は `check` と同じ。`--root` は、ページのパスを数えるルートで、共有の経路のページが使う申請の種類を探すプロジェクトでもある |
| `ritsu ringi diff <old> <new> [--format text\|markdown\|html\|json] [--org <dir>] [--at <日付>] [--root <dir>] [--budget <n>]`、`ritsu ringi diff --versions <日付> <日付> <file> [--format …] [--org <dir>] [--root <dir>]` | 同じファイルの二つの版の違い（11.2）と、一つの申請の種類の、組織の二つの版での違い（11.4）。違いが無ければ 0、あれば 1、比べられなければ 2（種類か名前の違う二つ、検査を通らない版、組織のデータが無い `--versions`）。`--org` と `--at` は `check` と同じで、両方の版に効く。`--budget` は経路を比べるときの予算（`rulec diff` と同じ） |
| `ritsu ringi vectors <file> [--list] [--org <dir>]` | ベクタを JSON Lines で標準出力に書く（10.4）。`--list` は、同じベクタを一行ずつ言葉で書く。入口 `worked` のベクタは書かない（`ritsu gen --target requests` が書く。10.4） |
| `ritsu ringi api <file> [--org <dir>] [--at <日付>] [--through <日付>] [--root <dir>]` | ファイルが宣言することを JSON で標準出力に書く（16.2）。サーバーと画面を作る人と、エージェントが読む。申請の種類、共有の経路、組織、マスタのどれも取る。検査を通らないファイルには、`check` と同じ診断を出し、JSON を書かずに 1 で終わる。`--org`、`--at`、`--through` は `check` と同じ |
| `ritsu ringi extract <file> --out <dir> [--org <dir>] [--at <日付>] [--through <日付>]` | 下ろしたファイルを書き出す（共有に移すとき）。書くのは検査が通ったときだけで、通らなければ `check` と同じ診断を出して 1 で終わる。`--org`、`--at`、`--through` は `check` と同じで、`--through` の日まで使えなければ何も書かない。組織の版ごとの承認者の一覧（E502〜E504、W502、W506、W507、W510 と、`ok` の下の数。8.6）は下ろす物を変えないので、`extract` は言わない。`--out` の外を指すパス（`use calendar`、`fed by`）は `--out` から数え直し、下ろしたファイルどうしのパス（`choices.rule`）はそのまま書く。問いの規則（4.2）は書かない。組織とマスタは下ろす物が無いと言って 0 で終わる |
| `ritsu ringi explain <code>` | 台帳 |
| `ritsu ringi init [<dir>] --module <path> [--catalog <url>] [--mcp] [--temporal]` | 申請と承認のサーバーを始める Go のプロジェクトの、最初のファイルを書く（16.4）。`--mcp` はエージェントのツールを、`--temporal` は flow を持つ申請の種類とワーカーを足す。ritsu の中でだけ走る |

どれも `--lang ja|en` を取り、`check`、`simulate`、`run`、`explain`、`diff` は `--format json` も取る（`api` は、いつも JSON を書く。`--lang` は、中の文の言語を選ぶ）。`check` は、申請の種類のシナリオも流す（`--at` を渡したときを除く。10.2）。組織のデータを読むコマンドは、どれも `--org` を同じ意味で取る。`ritsu check` も `--through <日付>` を取り、プロジェクトのどの申請の種類にも渡す（9.6）。終了コードは、ほかの言語と同じく、0 は通った（警告はあってよい）、1 は検査、主張、シナリオが落ちた（`diff` は違いがあった）、2 は引数の誤りか読めないファイル（`run` と `vectors` では、シナリオが無い、その名前のシナリオが無い、組織のデータが無い、も 2）。クレートのバイナリに、ほかの言語が要るファイルを渡したときも 2 になる（E008、16.3）。

### 16.2 `--format json`

名前の決まり（3.4）は、どのコマンドの JSON でも同じである。ステップ、承認者の決め方、フィールド、`use` の別名、マスタは、括弧の中の名前（ASCII）で書く。`check`、`simulate`、`api` は、その横に、書いたとおりの名前を添える。キーが `name` なら `written`、ほかのキーなら `<キー>_written` である（`stage` と `stage_written`、`way` と `way_written`、`stages` と `stages_written`）。英語の名前で書いたファイルでは、二つは同じ文字列になる。人が読む文（`text`、`message`、`notes`、`due` など）は、書いたとおりの名前を使う。`run` とベクタは、チケットの機械と生成物が読む JSON なので、括弧の中の名前だけを書く（10.4）。値も同じに分ける。`simulate` の `computed` は、ワイヤの値（金額ならフィールドの刻みの整数）を `value` に、書いたとおりの値を `value_written` に書く。単位は、単位の表の綴りなので、書いたとおりに書く（`money[円]`）。

`check` は、ファイルごとに一つのオブジェクトを書く。キーは、どの言語で出しても英語で、`ringi`（版）、`file`、`ok`、`summary`（通ったときの一行。落ちたときは null）、`facts`（`ok` の行の下に書く事実を一行ずつ。組織のデータの版と人数、マスタの行の数、使う日の範囲、版ごとの出せる人の数、組織の形でほかへ回るか飛ぶステップと、期限を過ぎても上へ回せないステップの申請者の数。どの行も `text`（その行の文）と `cases`（数のもとになった一件ずつ。ほかの行では空）を持つ。落ちたときは空）、`usable`（使う日の範囲。9.6。決めるデータが無ければ null）、`diagnostics` の順。診断は ritsu-base の形で、`code`、`severity`、`file`、`line`、`col`、`message`、`notes`、`fix` を持つ。下ろした先の診断を ringi の言葉に直すとき（6.5）は、`witness`（例の入力。下ろした規則の名前で）、`rows`（E302 の二つの行など、ringi の行）、`from`（下ろした先の言語とコード）を足し、`fix` に足す行を ringi の表の形で入れる（答えのセルは `?`。二行以上なら改行でつなぐ）。6.5 の E301 は次のようになる。

```text
$ ritsu ringi check E301_no_row_over_100000_yen.ringi --format json --root .
{
  "ringi": "0.56.0",
  "file": "E301_no_row_over_100000_yen.ringi",
  "ok": false,
  "summary": null,
  "facts": [],
  "usable": null,
  "diagnostics": [
    {
      "code": "E301",
      "severity": "error",
      "file": "E301_no_row_over_100000_yen.ringi",
      "line": 6,
      "col": 1,
      "message": "No row of the route takes some requests",
      "notes": [
        "The requests no row takes: amount from 100_001JPY to 1_000_000JPY",
        "A request no row takes: amount = 100_001JPY",
        "Add this row to the table, writing in place of each `?` who answers that stage (a name in `who`; a blank cell where those requests skip it): | >100_000JPY | ? | ? |",
        "The cells are cut only at the bounds the table already writes and at the ends of the ranges, so the rows close the gap as they are; where the approvers change inside a row, split it there."
      ],
      "witness": {
        "inputs": {
          "amount": 100001
        }
      },
      "from": "rulec E101",
      "fix": "| >100_000JPY | ? | ? |"
    }
  ]
}
```

rulec に尋ねて ringi が出す E105 と `only when` の E306 も、`witness` と `from`（`rulec`）を持つ。ほかの言語に尋ねない検査の診断は、この三つのキーを持たない。

```text
$ ritsu ringi check E004_a_cell_not_in_who.ringi --format json --root .
{
  "ringi": "0.56.0",
  "file": "E004_a_cell_not_in_who.ringi",
  "ok": false,
  "summary": null,
  "facts": [],
  "usable": null,
  "diagnostics": [
    {
      "code": "E004",
      "severity": "error",
      "file": "E004_a_cell_not_in_who.ringi",
      "line": 7,
      "col": 10,
      "message": "`accounting` is not declared in `who`",
      "notes": [
        "The ways to choose an approver declared: `finance`"
      ],
      "fix": null
    }
  ]
}
```

組織の版ごとの一覧の診断（E502、W502、E503、E504、W506、W507、W510。8.6）は、`witness` に、版の日（`versions`）、ステップ（`stages` と `stages_written`。W506 なら二つ、W510 なら飛ばされるステップの全部）、申請者の全部（`requesters`。`id` と、代理申請なら代わりに出した人の `made_by`、上へ回すステップなら回す先の `to`、W506 なら二つのステップに答える人の `to`、W507 なら期限を過ぎる人の `to`）を持つ。テキストが 3 人までしか名前を出さないときも、ここには全部が入る。E508 と E509（5.13）の `witness` は、版の日（`versions`）と、書いたグループ（`group`。E508 は版にいない名前のあるグループ、E509 は `submitted by` のあとの全部）を持つ。

```text
$ ritsu ringi check contract.ringi --format json --root .      （E504 の診断だけを抜いた）
    {
      "code": "E504",
      "severity": "error",
      "file": "contract.ringi",
      "line": 31,
      "col": 12,
      "message": "In the organization from 2027-04-01, the stage `second` cannot get 2 approvals for some requesters",
      "notes": [
        "`legal` has only 1 member besides the requester.",
        "For 2 requesters: e9002 (Lawyer 2) and e9003 (Lawyer 3).",
        "Add people to the team, or change the quorum."
      ],
      "witness": {
        "versions": [
          "2027-04-01"
        ],
        "stages": [
          "second"
        ],
        "stages_written": [
          "second"
        ],
        "requesters": [
          {
            "id": "e9002",
            "made_by": null,
            "to": null
          },
          {
            "id": "e9003",
            "made_by": null,
            "to": null
          }
        ]
      },
      "fix": null
    }
```

日本語の名前で書いた `contract.ja.ringi` の同じ診断では、`stages` は `["second"]`、`stages_written` は `["二人目"]` になる。

`facts` の `cases` は、組織の形でステップがどう回るかを、一件ずつ持つ（5.4、5.10）。一件は、版の日（`versions`）、ステップ（`stage` と `stage_written`）、承認者の決め方（`way` と `way_written`。規則の出力で決まるステップは `authority.decider` のように、規則の別名と出力の名前）、何が起きるか（`what`：上の長へ回る `up`、`then` の決め方へ回る `then`、飛ばされる `skipped`、期限を過ぎても上へ回せない `late`）、理由（`cause`）、どう回るかの注（`notes`）、申請の行（`lines`）、申請者の全部（`requesters`。`witness` と同じ形で、`to` は回る先の人か、期限を過ぎる人）である。テキストが数だけを言うので、ページはここから一覧を作る。

```text
$ ritsu ringi check leave.ringi --format json --root .      （`facts` の一行と、その一件目だけを抜いた）
    {
      "text": "requesters whose stages (`head`) go elsewhere or are skipped by the shape of the organization: 9 of 19 in the organization from 2026-04-01, and 8 of 18 from 2027-04-01",
      "cases": [
        {
          "versions": [
            "2026-04-01",
            "2027-04-01"
          ],
          "stage": "head",
          "stage_written": "head",
          "way": "dept_head",
          "way_written": "dept_head",
          "what": "up",
          "cause": "`dept_head` finds no team of the kind `department` above the requester.",
          "notes": [
            "The stage is `when empty escalate, then skip`, and it goes up."
          ],
          "lines": [
            22
          ],
          "requesters": [
            {
              "id": "e0101",
              "made_by": null,
              "to": "e0001"
            },
            {
              "id": "e0102",
              "made_by": null,
              "to": "e0001"
            }
          ]
        },
```

`usable` は、始まり（`from`）と終わり（`through`）の日と、それを決めているデータ（`from_by`、`through_by`）を持つ。データごとに、種類（`kind`：`effective from`、`organization`、`master`、`calendar`（期限を数えるカレンダー。保留が決めるときも、この種類で、文が保留を言う）、`reminder`（催促だけを数えるカレンダーと、上へ回したあとの催促。9.6）、`form date`、`resubmit`）、`use` の別名で読んだ名前（`name` と `written`。マスタは `m.accounts`、フォームの日付はフィールドの名前）、ファイル（`file`。組織ならデータのディレクトリ）、決めている行（`line`）、カレンダーなら、そのデータの最初の日か最後の日（`data`。終わりでは、期限を数える日数のぶん、使える最後の日より後になる）、文（`text`）を書く。9.6 の例では次のようになる。

```text
$ ritsu ringi check expense.ringi --format json --root .      （`usable` だけを抜いた）
  "usable": {
    "from": "2026-01-01",
    "from_by": [
      {
        "kind": "master",
        "name": "m.accounts",
        "written": "m.accounts",
        "file": "accounts.csv",
        "line": 2,
        "data": null,
        "text": "the first row of the master `m.accounts` (`accounts.csv`, line 2)"
      },
      {
        "kind": "calendar",
        "name": "office",
        "written": "office",
        "file": "finite.cal",
        "line": null,
        "data": "2026-01-01",
        "text": "the first day of the data of the calendar `office` (`finite.cal`)"
      }
    ],
    "through": "2027-09-30",
    "through_by": [
      {
        "kind": "master",
        "name": "m.accounts",
        "written": "m.accounts",
        "file": "accounts.csv",
        "line": 2,
        "data": null,
        "text": "the rows of the master `m.accounts` end on 2027-09-30 (`accounts.csv`, line 2)"
      }
    ]
  },
```

`simulate` は一つのオブジェクトを書く。キーは `ringi`、`file`、`on`、`on_given`（日を `--on` で選んだか）、`today_in`（`--on` が無いとき、今日を読んだ UTC オフセット `offset` と、それを持つカレンダーの別名 `calendars` と `calendars_written`。UTC で読んだなら `+00:00` と空のリスト。`--on` があれば null）、`version`、`computed`（`--form` から計算した値。一つずつ `name`、`written`、`value`、`value_written`）、`may_not_submit`（版の人のうち `submitted by` に当たらない人の ID。書いていなければ空）、`requesters` である。`requesters` の一つは、申請者の ID（`requester`）、代理申請なら代わりに出した人（`made_by`）、断られる仮押さえ（`holds_refused`）、ステップ（`stages`）を持つ。`--every-requester` でも、JSON は申請者を一人ずつ書く（テキストだけが、同じ承認者になる申請者をまとめる）。

`holds_refused` の一つは、その申請者の `submit` を機械が断る仮押さえで（10.3）、仮押さえ（`hold` と `hold_written`。帳簿の別名と振替）、値を作れない引数（`argument`）、引数が読む物（`reads` と `reads_written`）、その人の上に無いチームの種類（`team_kind` と `team_kind_written`）を持つ。

ステップの一つは、`stage` と `stage_written`、経路の表のセルか行に書いた決め方（`spec` と `spec_written`）、条件のあるステップか（`if_its_condition_holds`）、期限の文（`due`）、承認者の決め方ごとの `ways` を持つ。`ways` の一つは、`way` と `way_written`、場合の文（`case`）、人の ID（`people`）、議決に要る数（`needed`）、代理（`proxies`）、だれもいないときの文（`nobody`）を持つ。`once_with`（と `once_with_written`）は、`same approver approves once` でそのステップを済ませる元のステップ（無ければ null）、`once_if_no_stage_between` は、それがあいだに条件のあるステップが来ないときだけかどうかである（5.12）。`same approver approves once, at the last stage` を書いた申請の種類では、`once_with` は、その決め方の人が一度だけ答える、あとのステップである（待たないなら null）。`once_if_it_comes` は、待つのが、そのステップが来て同じ人に当たるときだけかどうか（`--form` を渡せば、いつも false）、`once_among`（と `once_among_written`）は、そのときに待つ先になりうるステップの並びで、来て同じ人に当たる最後のものが待つ先になる（待つ先が一つに決まっていれば空）。`ways` のキーは、`ticket` に何を書いた申請の種類でも同じである。あとのステップを待つことの無い申請の種類（`same approver approves once` だけを書いたものと、どちらも書いていないもの）では、`once_if_it_comes` は false、`once_among` と `once_among_written` は空のリストになる。`late` は、期限を過ぎても上へ回せないときに、そのステップがどうなるかの文である（回せるなら null。5.10）。日本語の名前で書いた経費の例では、次のようになる。

```text
$ ritsu ringi simulate examples/expense.ja.ringi --requester e1201 --form '種別 = 購買, 明細 = [{品目: "サーバー", 金額: 800_000円}, {品目: "ストレージ", 金額: 400_000円}]' --on 2026-11-16 --format json      （`computed` と、最初のステップだけを抜いた）
  "computed": [
    {
      "name": "amount",
      "written": "合計",
      "value": 1200000,
      "value_written": "1_200_000円"
    }
  ],
…
        {
          "stage": "first",
          "stage_written": "一次",
          "spec": "manager",
          "spec_written": "上長",
          "if_its_condition_holds": false,
          "due": "due 2026-11-19 at the end of the day, reminder on 2026-11-18 09:00",
          "ways": [
            {
              "way": "manager",
              "way_written": "上長",
              "case": null,
              "people": [
                "e1101"
              ],
              "needed": null,
              "proxies": [],
              "nobody": null,
              "once_with": null,
              "once_with_written": null,
              "once_if_no_stage_between": false,
              "once_if_it_comes": false,
              "once_among": [],
              "once_among_written": [],
              "late": null
            }
          ]
        },
```

日本語の名前で書いた購買の例（`same approver approves once, at the last stage`）で、`--form` を渡さないときの、上長のステップの決め方は、次のようになる。

```text
$ ritsu ringi simulate examples/purchase.ja.ringi --requester e3101 --on 2026-11-16 --format json      （最初のステップの `ways` から抜いた）
              "once_with": "decide",
              "once_with_written": "決裁",
              "once_if_no_stage_between": false,
              "once_if_it_comes": true,
              "once_among": [
                "decide"
              ],
              "once_among_written": [
                "決裁"
              ],
              "late": null
```

`api` は一つのオブジェクトを書く。キーは、`ringi`（版）、`file`、`kind`（`request`、`route`、`organization`、`masters`）、`name` と `written`、`version`、`title`、`description`、`owner`、`effective_from`、`uses`、`form`、`items`、`usable`、`utc_offset_minutes`、`ticket` の順である。

- `uses` の一つは、`use` の種類（`kind`）、別名（`alias` と `written`。gate は null）、書いたパス（`file`）、行（`line`）。
- `form` の一つは、フィールドの `name` と `written`、文言（`label`）、型（`type` と `type_written`。マスタや規則の列挙から選ぶ型は、別名を括弧の中の名前にして `choice from m.systems`。刻みを書かない率は、整数が何を数えるかが読めるように、`type` では `rate[step 1%]` と書く。13.1）、値の範囲（`min`、`max`。数は約束の JSON の整数で、日付は日付の文字列で）、文字数か行の数の限り（`count_min`、`count_max`）、選択の値（`values`）、`optional`、`computed`、`approvers_only`、`line`、明細のフィールド（`fields`）。申請の種類のほかは空。二つの範囲は、`--target requests` の `types/<名前>.json`（12.2）のフィールドと同じキーと書き方で、`ringi api` と `types/<名前>.json` が同じフィールドに持つキーは、同じ値になる。
- `items` は、中の物の全部（9.7）。一つは、種類（`kind`）、`name` と `written`、最初と最後の行（`lines`）、定義（`text`）。口の `Items` と同じ物である。
- `usable` は、`check` の JSON と同じ（決めるデータが無ければ null）。
- `utc_offset_minutes` は、申請の種類の日を読む UTC オフセット（7.5）を、UTC から東へ何分かの整数で書く。`types/<名前>.json`（12.2）と同じ値である。申請の種類のほかは null。
- `ticket` は、口の `Requests::ticket` が言語をまたぐ検査に渡すのと同じ事実である（9.4）。自己承認を許すか（`self_approval`）と、その行、読む gate（`gates`）、サーバーが gate に尋ねる操作（`operations`。チケットの操作と、サーバーの管理の四つ（14.3）。名前、書いた行（書く物でなければ null）、承認者がするか（`by_approver`）、ステップを決めるか（`decides`））、終わり方（`endings`）、営業日の数（`time`：一つの `round`、差し戻しの上限 `send_backs`、再申請 `resubmit`、保留 `pause`、合計 `total`、仮押さえが開いているいちばん長い `held`、終わりの無い待ち `no_end`、数えるカレンダー `calendars`）、仮押さえ（`holds`：帳簿の別名、帳簿のファイル、振替、行、申請者の組織の中の場所を読む引数 `requester_arguments` と、版ごとの値の無い人）、flow（`flows`：種類 `agent`・`summary`・`launch`、別名、flow のファイル、行、尋ねる所 `at`、渡す値 `given`）を持つ。渡す値の一つは、flow の入力の名前（`name`）、渡す物（`written`）、運ぶ型と範囲（`carries`）、値が無いことがあるか（`none`：いつも有れば null、無いことがあれば、その例の文 `text` と文が指す経路の行 `lines`、ringi に言えなければ `{"undecided": 理由}`）、範囲がどこで狭まるか（`where`：文 `text` と、文が指す経路の行 `lines`）、ringi が追わない条件で範囲がさらに狭いかもしれないときの文（`may_be_less`。このとき `carries` は渡りうる範囲を下回らないが、広いことがある。無ければ null）、行を持つ。申請の種類のほかは null。

ファイルのパスは、`uses` は書いたとおり、ほか（`file`、gate、帳簿、flow、カレンダー）は `--root` から数える。主張とシナリオは、括弧の中の名前を持たないので、`name` と `written` が同じになる。

```text
$ ritsu ringi api expense.ja.ringi --root .      （`examples/` で。頭と、フィールド一つ、物一つ、日を読むオフセット、`ticket` の頭と `time` と `holds` を抜いた）
{
  "ringi": "0.56.0",
  "file": "expense.ja.ringi",
  "kind": "request",
  "name": "expense_ja",
  "written": "経費",
  "version": 1,
  "title": "経費の申請",
…
    {
      "name": "amount",
      "written": "合計",
      "label": "合計",
      "type": "money[円]",
      "type_written": "money[円]",
      "min": 1,
      "max": 10000000,
      "count_min": null,
      "count_max": null,
      "values": [],
      "optional": false,
      "computed": true,
      "approvers_only": false,
      "line": 22,
      "fields": []
    },
…
    {
      "kind": "ticket",
      "name": "withdraw",
      "written": "withdraw",
      "lines": [
        73,
        73
      ],
      "text": "withdraw until 決裁"
    },
…
  "utc_offset_minutes": 540,
  "ticket": {
    "self_approval": false,
    "self_approval_line": 68,
…
    "time": {
      "round": 21,
      "send_backs": 3,
      "resubmit": 3,
      "pause": null,
      "total": 93,
      "held": 21,
      "no_end": [],
      "calendars": [
        {
          "alias": "office",
          "written": "本社",
          "file": "shared/office.cal"
        }
      ]
    },
    "holds": [
      {
        "alias": "budget",
        "written": "予算",
        "book": "shared/budget.book",
        "transfer": "reserve",
        "line": 75,
        "requester_arguments": []
      }
    ],
…
```

`run` は一つのオブジェクトを書く。キーは `ringi`、`file`、`scenarios`。シナリオごとに、名前（`scenario`）、書いたとおりに進んだか（`as_written`）、終わりの状態（`status`）とステップ（`stage`）、置いたままの時計（`timers`。種類、ステップ、カレンダーのオフセットで読んだ時刻）、行（`rows`）、終わりの帳簿（`books`。帳簿ごとに、口座の残高と仮押さえ）、止まった所（`stopped`。書いたとおりなら null。止まったなら、E602〜E604 の診断の `code`、`line`、`column`、`message`、`notes`）を持つ。行は、行の番号（`line`）、書いた時刻（`at`）、機械の一歩（`step`）、帳簿に流した振替と帳簿の答え（`books`）を持つ。`step` は、ベクタの一行と同じ形（`now`、`ticket`、`event`、`organization`、`answer`）で、シナリオの行のベクタは、この `step` そのものである（10.4）。

チケット、イベント、することの一覧は、約束の形（12.4 の表）で書く。チケットは、`id`、`request`、`version`、`requester`、`made_by`、`sent_back_by`（申請者に戻っているあいだの、差し戻した人。7.1）、`status`、`round`、`serial`（することの一覧の通し番号）、`revision`（チケットの版。`round`、`stage`、`edits`。7.1）、`form`、`stage`（今のステップ）、`stages`、`backs`（差し戻しと取り下げの回数）、`edits`（承認者の直しが通った回数）、`pause_left`、`paused_at`、`pause_ends`、`resubmit_due`、`holds`、`notes`、`ended` を持つ。`stages` の一つは、`name`、`value`（経路の表が決めた値）、`phase`（7.1。`open` は `active`）、`started`、`held`（保留した営業日）、`remind`、`reminded`（催促が鳴ったか。始まったときと上へ回したときは false。7.2）、`due`、`up`（上へ回した回数）、`org`（承認者を決めた版）、`again`（差し戻しで戻ったステップか、待たせたあとで始めた二度目のステップか。7.1）、`parts` を持ち、`parts` の一つは、`way`（承認者の決め方）、`key`（上へ回す表の入力にする決め方）、`approvers`、`team`、`needed`、`answers`（`by`、`seat`、`yes`、`at`）、`state`（`active`、`passed`、`skipped`、`stopped`、`agent`、`checked`、`once`、`deferred`）、`of`（`agent` と `checked` なら尋ねた flow、`once` なら承認で済ませたステップ、`deferred` なら待つ先のステップ）、`sole`（`same approver` の行が読む、ただ一人の承認者。`{"by": {"person": …}}`、`passing`、null）を持つ。時刻は UTC の秒、日は `YYYY-MM-DD`、金額はフィールドの刻みの整数で書く。イベントは、名前の一つのキーに、引数と、版（時計、`submit`、`comment` のほかのイベント）と、人のイベントならその人が書いた文（`comment`）を持つ（10.4）。することの一覧の一つは、`key`（`ticket`、`round`、`serial`）と `what`（行うことの名前の一つのキーに、その中身）を持つ。行うことは `history`、`notify`、`set_timer`、`cancel_timer`、`hold`、`post`、`release`、`launch`、`ask_agent`、`ask_note` で、`history` は、`did`（7.4 の語）、`stage`、`by`、`proxy_of`、`organization`、`once_with`、`to`、`note`、`comment`（その人が書いた文か、エージェントの理由。無ければ null）、`cleared`（その行で値が無くなったフィールド。7.3。出し直しと直しのほかの行は空）を持つ。`resubmit` と `edit` のイベントは、値（`form`、`fields`）と、空にするフィールドの名前の一覧（`clear`）を持つ。

`diff` は一つのオブジェクトを書く（11.2）。キーは `ringi`、`kind`（`request`、`route`、`organization`、`masters`）、`name`、`old` と `new`（それぞれ `file`、`version`、`sha256`）、`differs`、`total`（区間の外はどの入力も同じに答える、と言い切れたか）、`route`、`unsettled`、`sections`、`approvers`、`approvers_apply_to`、`no_people`、`in_progress` の順。`route` は区間ごとに、`case`（区間が狭める入力ごとの `name`（下ろした規則の名前）、`label`（フィールドの題）、`is`（値を言葉で））、`stages`（`stage` と `stage_written`、`label`、前と後の決め方の `before`、`before_written`、`after`、`after_written`、`before_words`、`after_words`。来ないときは null）、`text`（端末の文と同じ一文）、`example`（区間の入力の一つを、下ろした規則の名前で、`rulec vectors` の書き方で）、`people`、`applies_to` を持つ。`people` と `approvers` は、組織の版ごとに、`organization_from`（版の最初の日）、`requesters`（比べた申請者の数）、`changes` を持つ。`changes` は 10 行で切らず、すべての申請者の行を持つ。一行は `requester`、`made_by`（代わりに出す人。`simulate` と同じ。いなければ null）、`named`、`stage`、`stage_written`、`spec`、`spec_written`（区間の行では null）、`label`、`before`、`after` である。`sections` は節ごとに、`section`（ページの節の id）、`title`、`changes`（`item`、`before`、`after`。無い側は null）、`applies_to` を持つ。`applies_to` は、申請の種類では、版を上げたなら `new_requests`、同じ版なら `same_version` である。共有の経路では `requests_that_use_it`（効き方は、使う申請の種類の版で決まる）、組織とマスタでは `next_stage` である（2026-11-18。`in_progress` の文と同じことを言う）。`approvers_apply_to` は、データだけが変えた承認者なら `next_stage` である。

`diff --versions` は一つのオブジェクトを書く（11.4）。キーは `ringi`、`file`、`name`、`days`（渡した二つの日）、`versions`（それぞれの日に効く版の最初の日）、`same_version`、`differs`、`requesters`（両方の版にいる申請者の数）、`changes`（`diff` の `changes` と同じ形）、`only_before`、`only_after`、`submitted_by`（`submitted by` の行。無ければ null）、`may_now`、`may_no_longer`、`nobody`（`stage`、`stage_written`、`spec`、`spec_written`、`label`、`then`（`elsewhere`、`skipped`、`waits`）、`before`、`after`）、`in_progress`（同じ版なら null）の順。

### 16.3 ritsu への組み込み

- `ritsu check` は `.ringi` を ringi の検査にかける。ringi の検査は、自分で下ろしたファイルを Overlay に置き、口で rulec と koyomi に尋ねる（6.4）。組織とマスタのデータも同じに読む。`ritsu ringi check` と同じ道なので、同じ診断と、`ok` の行の下の同じ事実（組織のデータ、使う日の範囲）になる。まとめの行は `ringi 5` のように ringi のファイルだけを数え、下ろしたファイルとデータのファイルは数えない。`ritsu check --through <日付>` は、その日をどの申請の種類にも渡す（9.6）。その日で検査したものが、その実行での各ファイルのただ一つの検査で、言語をまたぐ検査もそれを読む。その日まで使えない申請の種類は、またぐ検査からも、通らないファイルに見える（2026-11-16。前は、またぐ検査のために日を渡さずにもう一度検査していた）。
- パスの書き方と一度の検査（根の DESIGN 4.15、2026-11-16）：ringi のコマンドと `ritsu check` は、ほかの言語と同じ三つの決まりでパスを書く。ファイルが書いたパスを言う文（E005 と E202 の本文）は、書いたとおりに言う。実行が見た所と開く所（診断の場所、E202 の注に引くほかの言語の診断の場所、組織とマスタのデータのファイル、`ok` の下の事実が言うデータの場所、E201 の「下ろす所にあるファイル」）は、走らせたディレクトリからいちばん短い相対で書き、絶対パスを渡した実行だけ絶対パスで書く。JSON の `file`（`check`、`run`、`simulate`、`api`）は根から数える。コマンドに渡したファイルと、`--org`、`--out` が取るパスも、`.` と `..` を畳んで書くので、`./leave.ringi`、`x/../leave.ringi`、`leave.ringi` は、読むものも言うことも同じである。下ろしたファイルのパスは出さない（6.5）。一回の実行で、`.ringi`、下ろしたファイル、組織とマスタのデータは、一度ずつ検査する。読む物の多い `ritsu gen` も、申請の種類が読むゲートを、プロジェクトの根から sekisho に尋ねるので、プロジェクトのゲートの検査と同じ一つになる。確かめるのは `crates/ritsu/tests/once.rs`（例のディレクトリの `ritsu check`、`--through` つきの `ritsu check`、`ritsu gen`、`ritsu ringi` の各コマンド）と `crates/ritsu/tests/spellings.rs`（四つの書き方、リンクを越えるパス、フラグのパス、根の外の規則）である。
- 読む順（根の DESIGN 6.1）：ringi の検査は、規則、日付、帳簿、ワークフロー、ゲートの答えを受け取るので sekisho のあとに、ringi の中の物を指す yuen と sakai の前に走らせる（`ORDER` は rulec、koyomi、chobo、geas、`.proto`、kaname、dandori、sekisho、ringi、yuen、sakai）。
- 待っている行（根の DESIGN 8.3）：`use` で読むファイルが検査を通らないときの ringi の E202 は、そのファイルの根のエラーを待っている行になる（`Tool::Ringi` の待っている行になりうるコードは E202）。組織とマスタのデータの誤り（E501、E505）で組織かマスタのファイルが通らないときも、それを読む申請の種類は同じに待つ。
- `ritsu-project` は、口の `Requests`（`Joined::requests()`）で、下ろしたファイル、フォームの事実、使う日の範囲を受け取れる。`ritsu gen`（12 章）がこれを使う。
- `ritsu gen` は 12.2 のとおり。`--target requests` を足す（言語の名前を出力先の名前に使わない。名前を決め直しても変わらない）。
- `ritsu explain` は、ritsu の E911、E209、E210 を持つ。ringi のコードは `ritsu ringi explain`。
- `ritsu mod diff`、`ritsu impact`、`ritsu mod report`（根の DESIGN 17.9.5、17.9.6、17.10、17.10.1。2026-11-18）：振る舞いの差は、ringi の `diff`（11.2）をこのプロセスで尋ねた答えである。どれかのファイルが変われば、両方の版にある `.ringi` を全部比べる。組織のデータだけが変わっても、申請の種類の承認者は変わるからである。インターフェースの差は `ritsu ringi api` の JSON（16.2）で比べ、行（`line`、`lines`、`self_approval_line`）と、根から書いたパス（いちばん上の `file`、`ticket` と `usable` のファイル。同じファイルを `uses` がファイルから書く）を除く。中の物（`items`）は種類と名前で突き合わせ、例、シナリオ、主張は比べない。どれも使う側が呼ぶ形ではないからである。`where` と `none` の文に行を入れないのも、このためで、ファイルの上に一行足しても api の文は変わらない。人が読む出力（言語をまたぐ検査の文）は、文と行を合わせて今までと同じに書く。`ritsu mod diff` と `ritsu mod report` の Markdown（と、それから作るモジュールの一覧のサイトの変更のタブ）には、`diff` のページを、頭のコメントを除き、題を `###` の見出しにして入れる（2026-11-18）。頭のコメントは二つのファイルの名前を書くので、モジュールの書き方の側では、名前（`example.com/acme/requests@1.0.0/trip.ringi`）の `@` がコードの外に出る。サイトはコメントを知らず、文字のまま出す。題の見出しは、ほかの言語の diff の Markdown の頭にそろえた。`ritsu impact` の事実 `request_changes` は、ringi の `diff` が `.ringi` ごとに挙げる物（ステップが変わる申請の区間、節ごとの変更、承認者が変わる申請者）の数と、足した・消した `.ringi` の数を足したものである。`ritsu mod report`（根の DESIGN 17.10）も同じ関数で振る舞いを比べるので、依存を上げたとき、変わったモジュールのファイル（組織、マスタ、共有の経路）を読む手元の `.ringi` は、base の lock で読んだものと head の lock で読んだものの `diff` を「手元のファイルの答えのうち、変わるもの」に出す。二つの側は同じファイルで同じ版である。組織かマスタのデータだけが変わったなら、進行中のチケットの文は、データから来る承認者の文になる（`crates/ritsu/tests/modules_report.rs`）。共有の経路が変わって経路が変わったなら、同じ版のまま経路が変わるので、版を上げるように言う（共有の経路の文と同じことである）。
- ringi のクレートのバイナリはほかの言語を持たない。ほかの言語が要る `.ringi`（`use` で今の言語のファイルを読むか、下ろす物がある）には、`ritsu ringi` で走らせるよう言う E008 を、要る言語ごとに一つ、その言語を最初に要する行に出し、2 で終わる（根の DESIGN 2.3 の形。sekisho の E209、kaname の E202 と同じ）。下ろす物が無いのは組織とマスタのファイルだけなので、申請の種類を確かめるのは `ritsu ringi`（と、それへのリンクの `ringi`）である。表を確かめずに通ったように見せないためである（R4）。

### 16.4 サーバーのプロジェクトを始める（`ritsu ringi init`）

`ritsu ringi init [<dir>] --module <path> [--mcp] [--temporal]` は、申請と承認をフレームワーク（12.5）の上で動かす Go のプロジェクトの、最初のファイルを書く。申請の種類を一つ書けば数分で動かせるようにするためである。書く物は次のとおり。

| 書く物 | 中身 |
|---|---|
| `project/` | 申請の種類 `purchase.ringi`（金額と目的のフォーム、上長と部長の二つのステップ、期限と催促、却下、申請者への差し戻しと出し直し、シナリオ四つ）、組織の形 `shared/company.ringi` と、その 2026-04-01 からのデータ `org/2026-04-01/`（部が三つ、課が二つ、十人）、カレンダー `shared/office.cal`、gate `shared/company.gate`（役割は職員、部門の管理者、全体の管理者、監査する人。14.3）。`ritsu check` は警告なしで通る |
| `go/`、`requests/` | `project/` から `ritsu gen` が作る物を、一度だけ書く。`go mod tidy` のあと、`go generate` を待たずに組めるようにするためである |
| `generate.go` | `go generate ./...` が走らせる `//go:generate ritsu gen …` の行 |
| `go.mod` | `--module` のモジュール。フレームワークと cedar-go を、下の版で要る |
| `<パッケージ>.go`、`gate.go`、`notify.go` | プロジェクトがサーバーに渡す物（申請の種類、組織、gate、通知）、生成した gate へのつなぎ（生成した `RolesOf` に尋ねて Staff が持てる役割だけを残し、組織の版に居ない人と、受け持つチームの無い申請の種類を、どの部門の物にもしない。14.3）、通知をログに書く部品（種類を、書く言語の題と、その題の言語で書く。題はフレームワークの `Server.Title` が、種類の説明の `labels` から選び、その言語の題が無ければファイルの題を返す） |
| `cmd/server/main.go` | サーバー。フラグ、起動の確かめ、保管、ログイン（ヘッダ、開発用のログイン、OpenID Connect）、HTTP は、フレームワークの `serve` が持つ |
| `conformance_test.go`、`server_test.go`、`pages_test.go` | プロジェクトのベクタでの適合の試験、gate だけが決めること（チケットに載っていない人に見せるか、取り下げ、管理の四つの操作を役割と部門で）と、組織に居ない人、受け持つチームの無い申請の種類、受け持つチームの種類での比べ、宣言していない役割で、つなぎを確かめる試験、開発用のログインでページから申請を出して承認する試験 |
| `README.md`、`.gitignore` | 作り直し、動かし方、試験、人に使ってもらう設定（PostgreSQL、OpenID Connect、https）、申請の種類と flow を足す手順、動いているサーバーに組織の新しい版を上げて取り込む手順 |

`--mcp` と `--temporal` は、次の物を足す。両方を渡してもよい。

| オプション | 足す物 |
|---|---|
| `--mcp` | `tools.go`：サーバーの `/mcp` に、フレームワークのモジュール `mcp` のエージェントのツールを置く（`HandleTools`。`serve` の `Stores.Handle` で足すルートで、人はページと同じログインで見つける）。`tools_test.go`：だれもログインしていないリクエストに 401 が返ることと、エージェントがツールで購買を申請して読めることを確かめる |
| `--temporal` | `project/expense.ringi`：経費の精算。上長のステップ一つで、承認されると flow `pay` を始める（`launch`）。シナリオ二つ。`project/shared/pay.flow`：払い戻して知らせる flow（タスク二つ）と、そこから生成する物（`go/flows/expense_payment/` など）。`flows.go`：flow を始める `temporal.Runner`（`Flows`。種類の説明から flow を読む）と、flow のワーカー（`Workers`）。`payments.go`：flow のタスク（ログに書くだけ）。`cmd/worker/main.go`：ワーカーのコマンド。`flows_test.go`：Temporal の開発用サーバーの上で、承認から支払いと知らせまでを動かす |

どちらも、`cmd/server/main.go`、`go.mod`、`<パッケージ>.go` の注釈、README を、足した物に合わせて書く。`--temporal` の `cmd/server/main.go` は、`-temporal` のフラグ（無ければ `localhost:7233`）の Temporal に接続し、`cfg.Flows` と `cfg.Agents` を渡す。`ritsu check` は、どの組み合わせでも警告なしで通る。

`--lang ja` は、`project/` を日本語の名前で書き（例の `.ja.ringi` と同じ作り。ASCII の名前を括弧に添える）、`README.ja.md` も書く。Go のファイルは同じで、`generate.go` の行の `--lang` だけが違う。

決めたこと：

- **上書きしない。** `init` が書く物は、書いたあとは使う人の物である。書くファイルの一つでも、その場所に何かがあれば（ディレクトリを作る場所にファイルがあるときも）、ぶつかる物を全部挙げて、何も書かずに 2 で終わる。書けない所も、何も書かずに 2 で終わる。
- **`gen` との違い。** `gen` は何度でも書き直す物を書き、前に書いて今は作らない物を消す（根の DESIGN 9.6）。`init` は一度だけ書き、そのあとは使う人が直す物を書く。生成物も一度書くが、`init` はそれを、書いた `generate.go` の行の引数で `ritsu gen` を走らせて作る。それで生成物は、その行が書く物と同じになり、行に `--check` を付けた `ritsu gen` は差を言わない。そのあとの生成は `gen` の仕事である。
- **生成の行。** シェルの台本にせず、`//go:generate` の行にする。シェルの無い所でも動く。引数はフレームワークの例の作り直しの台本と同じ形で、`--module` は使う人のモジュールの下（`<module>/go`）にする。`--lang` は必ず書く。生成するコードの文の言語（12.2）が、走らせる人の環境変数で変わらないようにするためである。
- **ritsu の中でだけ走る。** 生成物を書くので `ritsu gen` が要り、ringi のクレートのバイナリは `init` を断って 2 で終わる（コマンドの表には載せ、`--help` は読める）。
- **`--module` は要る。** `go mod init` と同じく、無ければ断る。ルートのパッケージの名前は、`ritsu gen` が Go のパッケージを名付けるのと同じに、モジュールのパスの最後の部分から作る。ただし `main` と、`_test` で終わる名前は `project` にする。`main` はプログラムが読み込めず、`_test` で終わる名前のパッケージは、Go がその試験のファイルを別のパッケージの試験として読む（`found packages approvals and approvals_test`）。サーバーの `main.go` の名前とぶつかる名前なら、`main.go` は別の名前（`project`）で読み込む。`--temporal` のときは、サーバーとワーカーの `main.go` が足す名前（`client`、`server`、`temporal`、`flows`、`worker` など）とぶつかる名前も、そうする。パッケージのファイルは `<パッケージ>.go` で、`init` が書くほかのファイルと同じ名前になるとき（`…/gate`、`--mcp` のときの `…/tools`、`--temporal` のときの `…/flows` など）と、Go が一つのシステムでだけビルドする名前になるとき（`…/approvals-windows` の `approvals_windows.go`）は、`project.go` にする。
- **カタログの ID。** `--catalog`、無ければ `https://<モジュールのパス>/a2ui/catalog.json`。どちらも生成の行に書く。
- **go.sum は書かない。** ritsu はモジュールのハッシュを持たない。`go mod tidy` が書く。
- **MCP と flow はオプションで足す。** オプションの無いプロジェクトは、MCP の SDK も Temporal の SDK も要らず、Temporal のサーバーが無くても動く。`--mcp` と `--temporal` は、それぞれが要る物（フレームワークのモジュールと、その依存）を足す。オプションの無いときに書く物は、どちらのオプションにも左右されない。
- **`--temporal` は二つ目の申請の種類を足す。** `purchase.ringi` に flow を足さず、flow を持つ `expense.ringi` を別に書く。`purchase.ringi` はオプションの無いときと同じにして、flow を持つ種類と持たない種類を並べて見せる。flow は、承認のあとに払い戻す支払い（`launch`）にした。エージェントのステップ（`ask`）は、エージェントのプロバイダーの鍵が要るので書かない。
- **ワーカーは別のコマンド。** サーバーは flow を始め（`cfg.Flows`、`cfg.Agents`）、尋ねる実行のワーカーを `Runner.Run` で動かす。flow のワーカー（プロジェクトのタスクを持つ）は `cmd/worker` が動かす。フレームワークの `temporal/` の README が、ほかのプロセスで動かす flow として書く形である。サーバーは、Temporal に届かなければページも API も出さずに止まり（`-temporal` のアドレスと理由をログに書いて 1 で終わる）、30 秒のうちに全部のタスクキューのワーカーが見つからなければ、ログに警告を書いて動き続ける（`CheckWorkers`）。そのあいだ、flow とそのチケットはワーカーを待つ。
- **`TestFlows` は開発用サーバーがあれば走る。** フレームワークの `temporal/` の試験と同じく、`TEMPORAL_CLI` で指定した CLI か、SDK が OS の一時ディレクトリに置くファイル（`temporal-cli-go-sdk-<版>`）で開発用サーバーを起動し、どちらも無ければ理由を書いて `SKIP` で通る。ダウンロードはしない。版は SDK の `temporal.SDKVersion` から読む。`go test` が作るバイナリは依存のモジュールの版を持たず、`debug.ReadBuildInfo` では読めないためである。
- **go.mod の版。** フレームワークの `mcp` と `temporal` のモジュールは、フレームワークと同じ `FRAMEWORK_VERSION` で要る。フレームワークは、三つのモジュールを同じ版で公開する。Temporal の SDK と API は、生成する flow のコードが要る版（`GO_MODULES`）で要り、`go` の行もそれらの go.mod に合わせる。MCP の SDK は書かない。プロジェクトの試験が読み込むが、`go mod tidy` が、フレームワークの `mcp` のモジュールの要る版で足す。
- **版の結び方。** ritsu の各版は、組むフレームワークの版を一つ持つ。`crates/ritsu/src/ringi_init.rs` の `FRAMEWORK_VERSION`（いまは v0.1.0）と、その版の go.mod の `go` の行 `FRAMEWORK_GO` である。約束の版（12.4）を上げるリリースは、新しい約束を受けるフレームワークの版にこれを上げる。約束の版を上げずに、Go の申請の種類の一覧の `Kind` の形が変わるリリース（入口が増えるときなど）も同じである。フレームワークは生成した一覧を形で受けるので、前のフレームワークでは、書いたプロジェクトの `server.Load(ticketkinds.Contract, ticketkinds.Lang, ticketkinds.Kinds, dir)` がコンパイルで止まる。上げる所は、その二つの定数、`crates/ritsu/tests/golden/ringi-init/` の golden の八つ（言語とオプションの組ごと。`RITSU_BLESS=1` で取り直す）、この節の版である。フレームワークの側では、README の「Start your own project」に貼った `init` の出力に版が入る。書いた `go.mod` が公開済みの版を指すように、フレームワークのその版を先に公開する。cedar-go と Temporal の SDK の版は、生成するコードが要る版（`package.rs` の `GO_MODULES`）を読む。
- **試験の分担。** ritsu の側（`crates/ritsu/tests/ringi_init.rs`）は、ひな形から書く全部のファイルを、言語とオプションの組（無し、`--mcp`、`--temporal`、両方）ごとに golden で押さえ、生成の行に `--check` を付けて差が無いこと、`ritsu check` が警告なしで通ること、書く物が環境変数の言語に従わないこと、断る文を英日で確かめる。生成物の中身は `gen` の試験が押さえる。Go を組んで試験を走らせるのは、フレームワークの側の試験である（PATH の ritsu で、言語とオプションの組ごとに `init` を走らせ、`replace` でフレームワークのモジュールを読ませる）。ritsu の CI には Go のフレームワークが無いので、ritsu の側には置かない。
- **ひな形の置き場。** `crates/ritsu/src/ringi_init/`。名前はどれも `.tmpl` で終わり、木の `.ringi`・`.gate`・`.cal`・`.flow` を読む道具は読まない。`{{module}}` のような名前を、コマンドラインから決まる値で埋める。オプションが足す行と差し替える行は、`{{if mcp}}`、`{{if temporal}}`、`{{else}}`、`{{end}}` だけの行で囲む。
- **起動を止めたエラーは 1 行ずつ。** `cmd/server/main.go` は、起動を止めたエラーを `server.LogStop(log, "the server stopped", err)` でログに出す。フレームワークの `LogStop` は、エラーの一文ごとを一つのレコードにするので、起動で止まった組織のデータの問題が一つずつログに残る。

## 17. Lean

根の DESIGN の P7 と 11 章にそって、チケットの機械を Lean 4 でモデルにして証明し、Rust の参照インタプリタと突き合わせる（`proofs/RingiCore/`）。モデルは、機械の本体 `src/lower/ticket.kn`（12.1）を関数ごとに写した物で、定理はその関数について言う。`sorry`・自前の `axiom`・`native_decide` は使わず、どの宣言も Lean の三つの公理のほかに立たないことを `crates/ritsu-model/tests/proofs.rs` が確かめる。

**モデルにする物。** 7 章の機械の全部：状態とイベントと遷移（7.1〜7.3）、することの一覧（7.4）、議決（`needed` と `verdict`）、差し戻しの回数と `round`、取り下げと却下と承認の取り消し（5.6、5.7）、保留と合計の上限（5.7）、`same approver approves once` と `, at the last stage`（5.12）、時計と、鳴った時計が先であること（5.10、7.6）、仮押さえの開閉（7.4、7.6）、組織を上へたどること（8.5。30 チームまで）。機械に入った物も同じくモデルにする：チケットの版（`revision`。7.7）、人のコメント（コメントだけの操作と、操作に添える文。7.2）、代理の行（`proxy_of`。5.8、7.4）、申請者へ差し戻した人（`sent_back_by`）、`submitted by`（5.13）、申請者の側の人（代わりに出した人。5.8）。

**モデルの外に置く物。** 申請の種類のコードが答える物は、モデルは関数として受け取る（`Oracle`。`Glue.lean`）：ステップの時計（`clocks`。期限と催促、保留の残り、再申請の期限）、営業日を数えること（`open_after`。koyomi のモデルが受け持つ）、`on` の行の条件と引数（`on`）、出し直しと承認者の直しのフォーム（`reform`）、申請を出したときのフォームと経路の着くステップ（`prepare`）、フォームの値を flow に渡す形（`vals`）。フォームとイベントの値は JSON のまま持ち、モデルは中を読まない。表と帳簿と日付の答えが正しいことは、rulec・chobo・koyomi の側の検査とモデルが受け持つ。定理は、これらの関数が何を答えても成り立つ言い方にする。ファイルの分け方は、機械のコードの並びのとおり：型（`Types`）、言語の組み込みの写し（`Prelude`）、中で使うレコード（`Records`）、日付と組織（`Org`）、ステップ（`Stages`）、一つのイベント（`Moves`）、つなぎ（`Glue`）、ベクタの読み書きと一行の答え方（`Json`、`Run`）、定理（`Theorems/` の五つ）。

**示したこと。** 始めの項で挙げた六つを、今の機械に合わせた言い方で示した。2 は、書いたとおりには成り立たないので、成り立つ形に書き直した（下の「終わること」）。

1. **終わりの状態は、どのイベントも受け付けない**（`Theorems/Next.lean`）：`next_ended`（終わったチケットはどのイベント・組織・時刻・外の関数の答えでも断る）、`next_ended_state`（日が読め、鳴った時計が無く、版が合えば `state` で断る）。`end_ticket` で終わるチケットは保留と出し直しの時計を持たず、あとから鳴る時計も無い（`end_ticket_clocks`、`rung_first_ended`）。`finish` で終わるチケットはその二つを持ちうるが、進行中のチケットにだけ `finish` は来る（到達できる状態の不変条件までは示していない）。
2. **チケットが終わるのは、イベントの数ではなく時間である**（7.6。「終わること」を下に書く）：鳴った時計があるあいだ、時計でないイベントは、コメントと古い版のイベントも含めて、どれも `clock` で断る（`next_rung_first`）。
3. **どの終わり方でも、開いた仮押さえは確定か取消で閉じる**（`Theorems/Holds.lean`）：帳簿と振替ごとに、ラウンドの開いた仮押さえの数が、それを確定か取消する行のうち条件が偽でない物の数以下のとき（`HoldsCovered`）、`hold` の行を持たない終わり方は、断られるか、そのラウンドの仮押さえを一つも残さない（`on_ending_closes`、全部の条件と引数が決まれば断られない `on_ending_ok`、合わせた `on_ending_ok_closes`）。残った仮押さえは終わる前からあった物で、チケットのほかの所は変わらない。断られた条件や引数が、仮押さえを黙って開いたまま残すことは無い。三つの終わり方のそれぞれで同じことを言う（`end_ticket_*`：`rejected`・`withdrawn`・`expired`。`finish_*`：承認。`to_requester_*`：差し戻しと、申請者への取り下げ）。断りが伝わるのは、開いた仮押さえの有無にかかわらず読まれる `hold` と `launch` の行と、`on` 自体の断りに限る（`on_ending_refused`、`on_ending_on_refused`）。確定と取消の行は、開いた仮押さえが無ければ条件と引数を読まずに飛ばされるので（7.4）、「どれかの行の条件か引数が断られれば終わり方も断る」という広い形は成り立たない（条件が断られる `post` の行だけを持つ、仮押さえの無いチケットは承認で終われる）。定理は、終わり方の行に `hold` が無いことを前提にする。
4. **議決は、答えについて単調である**（`Theorems/Quorum.lean`）：通る条件は、答えを後ろに足しても保たれる（`passes_append`）。一つの答えは、部分の答えのあとに足されるだけで、通った部分を通らなくせず、済んだ部分を戻さない（`answered_seat_*`、`answered_parts_grown`）。答えのあとのチケットは、部分を育てて同じステップにとどまるか、全部済んで次へ進むか、却下で終わるかのどれかである（`answer_grows`）。取り消しで消えるのは、取り消す人が承認したステップのその人の答えだけで、それより前の答えは残り、あとのステップは来る前の姿に戻る（`retract_answers`）。`, at the last stage` では、取り消した承認で済んでいた部分は待ちに戻り（`retract_rewaits`）、ほかの書き方では前のステップはそのままである（`retract_keeps_before`）。`same approver approves once` で済んだ部分は、その承認を消す取り消しで一緒に戻る（`part_done_once`、`back_step_once`）。取り消しは答えを消す唯一のイベントではない（前のステップへの差し戻しがその先の答えを消し、出し直しは新しい `round` になり、期限を過ぎて上へ回る部分の答えは空になる。5.6、5.10）ので、単調さは答えのイベントと、取り消しが消す物について言う。`needed` は、百分率が負でなければ承認者の数で単調で（`needed_mono`）、`at least p%` は `p%` の切り上げである（`needed_percent_ceil`、`needed_percent_least`）。取り消しの定理と `stage_in_at`（イベントが読むステップと書き込む場所の一致）は、経路のステップが 1,000,000 以下であることを前提にする（12.1 の機械の数の上限と同じ）。
5. **自己承認を許さない経路では、申請者の答えで通るステップは無い**（`Theorems/Approvers.lean` と `Next.lean`）：機械が承認者に決める人（決め方の全部と、承認者のいないステップを上へ回した先）に、申請者も、代わりに出した人もいない（`solve_kept`、`solve_not_self`、`up_from_kept`、`up_from_not_self`）。外されるのはこの二人だけで（`left_out_iff`）、二人は、自分の席でも代理の席でも、承認も却下も保留も承認者の直しもできず、承認者に足すことも引き継ぐこともできない（`seats_self`、`answer_self`、`approver_self`、`next_approve_self`、`next_reject_self`、`do_add_approver_self`、`do_hand_over_self`）。自己承認を許す経路では、だれも外されない（`left_out_allowed`）。申請者の側の人のうち、代わりに出していない人（`submit` の代理）は外されず、申請者のために出せる（`may_answer_submit_proxy`。5.8）。
6. **組織をたどって見つけた人は、申請者の上のチームの長で、役職が求めた役職以上である**（`Theorems/Climb.lean`）：たどる並び `chain` は、申請者の主の所属のチームから `parent` をたどる 30 チームまでの並びで（`chain_eq_ancestors`、`chain_climbs`、`chain_length_le`）、そこで見つける人は、その並びのチームの長で、外される人ではない（上長は申請者自身でもない。`manager_of_people`、`head_of_kind_people`、`up_from_at_found`、`up_from_above_found`）。`nearest <役職> above` が見つけるのは、条件に合ういちばん近いチームの長で、そのチームでの役職の位が求めた位以上である（`nearest_way_people`。位は、組織の役職の並びでの場所。`rank_eq_some`）。

コメントだけの操作は、状態・経路・答え・版を変えず、履歴の `commented` の行と `comment` の知らせだけを足す（`next_comment_ticket`、`next_comment_same`、`next_comment_effects`）。見せた版と違う操作は `stale` で断られ、チケットを変えない（`next_stale`）。

**終わること（始めの項の定理 2 の書き直し）。** 「差し戻しの回数、保留の合計、ステップの数に上限があれば、どのチケットも有限の数のイベントで終わる」は成り立たない。承認と取り消しをくり返せば、三つとも上限があってもイベントはいくらでも受け付けられる（同じ時刻の保留と再開、引き継ぎの行き来、承認者の直しも同じ）。機械が終わりを保証するのは、イベントの数ではなく時間である（7.6）：終わりでない状態には時計の出口があり、期限は取り消しでは動かない。モデルで示したのはその芯である。鳴った時計があるあいだ、時計でないイベントはどれも `clock` で断る（`next_rung_first`）。コメントだけの操作は何も変えない。古い版の操作は断られる。時計のイベント（`late`、`pause_ends`、`resubmit_late`）が、`stay` の無い経路で、上へ回す残り（`chain_length_le` の 30）とステップの残りを減らすか、チケットを終えることは、まだ示していない。

**`same approver approves once` が数える元の承認（直した形）。** 5.12 のとおり、機械（参照インタプリタ、`ticket.kn`、モデルの `origin_on`）は、すぐ前のステップがこの決まりで済んだものならさらに前へさかのぼって元の承認を探し、その人が**自分で答えた**ステップで止まる。前のステップを読む `look_of` は、`answered` と `once` を別の形に読む（`once` で済んだステップだけを越えられる）。差し戻しで自分の答えたステップへ戻ってまた承認したときは、履歴の `approved` の行は、差し戻した人が求めたそのステップの答えを `once_with` に書く（代理の承認なら `by` と `proxy_of` もその代理になる）。効くのは履歴の行と部分の `of` だけで、だれが答えるか、ステップが済むか、取り消しで戻る所は変わらない。前の形は `answered` と `once` を同じ `parts` に読んで、自分で答えたステップでも越えていた。機械の不具合として、参照インタプリタと `ticket.kn` とモデルを一緒に直した（段階 G）。`tests/machine/once.ringi` の `traced` の形のシナリオ（自分で答えたステップと、代理が答えたステップ）がこの読みを確かめる。

**突き合わせ。** ベクタ（10.4）を Rust の参照インタプリタとモデルの両方に流し、答えを一行ずつ比べる（`crates/ritsu-model/tests/ringi.rs`）。流すのは、五つの例（英語と日本語の名前の版）と、機械のための申請（`tests/machine/`）の全部のベクタ。比べるのは、次のチケットの全部、することの一覧の全部、断りの種類で、断りの文の中身は比べない。モデルは、外に置いた物を要るときに問い（`{"need": …}`）を返し、試験が参照インタプリタの関数で答えて同じ行を渡し直す。モデルを一か所ずつわざと壊して確かめた 13 のうち、10 は突き合わせが落ちた（議決の数をずらす二つ、差し戻しの上限をずらす二つ、保留の合計を数えない、仮押さえを閉じない、版を見ない、代理の行を書かない、承認を取り消せる間、組織を上へたどる深さを一つ浅くする）。取り消せる間は、現在のステップに答えが一部入っている状態で待つベクタの取り消しが、壊した機械では通る（`seat`）。深さは、ちょうど 30 チーム上の長を見つけるベクタが落ちる（`deep`）。残る三つは、どの入力でも見える答えが変わらない壊し方である。鳴った時計の順と同時刻の二つは `rung_first`（鳴った時計から一つ選ぶ）を壊すが、その答えは、時計でないイベントを `clock` で断る文の中身にだけ出る。時計のイベントは `rung_first` を見ず、それぞれの扱いが自分の時計だけを調べ、断りの文は突き合わせないので、答えは変わらない（届く状態で鳴ってまだ渡されていない時計は多くて一つでもある）。深さを一つ深くする方は 31 番目のチームでしか効かず、正しい組織は上に 30 チームまでしか持てない（E501）ので、届かない入力への壊し方である。

kaname のモジュール（12.1）の関数の性質は、kaname の `property` と `ritsu gen --target lean` で書き出す形を、機械の写しで確かめた。関数一つの性質（議決の必要数、答えの単調さ、自己承認、たどる深さ、コメントがチケットを変えないこと）の多くは、書き出した文を手で直した上で証明できたが、今の `--lean` の書き出しは機械のファイルで組めない所がいくつかある（相互に参照する型の導出、Lean の予約語、数の字面の直後の `.toNat`、定義の順）。本物の `ticket.kn` に `property` は足していない。

## 18. 試験

| 種類 | 中身 | 置き場所 |
|---|---|---|
| 構文と名前 | 字句、構文、名前、`use`。`when empty escalate` の `then` に書けるもの（`skip`、`stay`、決め方の名前と、`who` に無い名前、`skip` と `stay` という名前の決め方）、`ticket` の `same approver approves once`（`, at the last stage` を続ける形と、コンマの無い書き方の誤りも）と `submitted by`、一度だけ書ける `ticket` の行を二度書くこと（二つの書き方を両方書くことも） | `crates/ringi/tests/` |
| 診断の golden | どのコードにも英語と日本語の golden と、出す変異 | `tests/golden/codes/`、`tests/mutants/` |
| 注に書いた直し方 | 診断の注と台帳の直し方が見せる書き方を、そのとおりに書いたファイルが検査を通る（英語と日本語の文の書き方と、書いた行を突き合わせる）。データを直すように言う注は、小さい組織のデータを注が言うセルで直して確かめる | `tests/fixes.rs` |
| 例の値 | 例のセルの値がフォームの取らない値なら、経路がその列を読むかどうかにかかわらず同じことを同じセルで言う（同じ宣言の二つの列の、読む側を入れ替える）。経路が読めない列（文、マスタから選ぶ）も。規則の列挙の値を、経路の表と条件でも確かめる | `tests/example_rows.rs` |
| 例の申請 | `examples/` の 13 本（申請の種類の英日の十本と、共有の組織、マスタ、経路）が、どの言語もつないで承認者を数えない検査を、何も言わずに通ること。組織の二つの版で数えると、言うのは組織のデータのことだけで、日本語の名前の版は英語の版と同じ場所で同じことを言うこと。`check` の出力の golden | `tests/examples.rs`、`tests/golden/examples/` |
| 下ろし方 | 例ごとの下ろしたファイルの golden と、それが今の言語の検査を通ること。予約語のコピーが本物と同じこと。下ろしたファイルの診断が、どれも ringi の行に戻ること。別の単位で書いた数を、フィールドの単位に直して下ろすこと。名前を組み合わせると、ほかの名前や言語の型の名前と同じになる名前（6.4）を、別の名前で下ろすこと。`none` になることのある規則の出力を読む条件と表の列が E309 になること | `tests/lowered.rs`、`tests/golden/lowered/`、`tests/mutants/` |
| ブラウザで試すページ | 下ろして尋ねることと、組織とマスタのデータを読むことが、wasm のモジュール（ファイルはメモリの上）でもバイナリと同じ答えになる | `crates/ritsu/tests/playground.rs` |
| 待っている行 | `use` で読むファイルの誤りを、ringi の行が待つ | `crates/ritsu/tests/check.rs` |
| 参照インタプリタ | 機械のための申請（`tests/machine/`。遷移の表の行ごとにシナリオを持つ `desk` と、その日本語の名前の版、組織のいちばん上のステップの `top`、期限を過ぎて回す先の無いステップの `late`、上へ回すたびに催促が付くステップの `reminds`、前の承認で済むステップの `once`、最後のステップで一度だけ答える `last` と、その日本語の名前の版、ステップの条件の形で待つ `waits`、出せる人の `who`、エージェントと人が答えるステップの `flows`、範囲の端の `ranges`、部の無い人には作れない仮押さえの `held` と、その日本語の名前の版、期限と催促を別のカレンダーで数えるステップの `apart`、出し直しと直しで値を空にし、見せなくなったフィールドの値を落とす `supplies` と、その日本語の名前の版、却下と差し戻しに理由を求める `reasons`）と五つの例が、どれも書いたとおりに進むこと（`run` の出力の golden を英語と日本語で）。7.3 の表の行ごとに、前の状態、次の状態、することの一覧。受け付けない状態と人、取り下げられる間、差し戻しの回数、取り消せる間と期限が動かないこと、保留の合計、承認者の直すフィールド、保留中の flow の答え。値を空にすることと見せなくなったフィールドの値を落とすこと（種別を変えた出し直しで二段に落ちる物、経路の決め直し、`clear` が断る形の全部と通す形の二つ、履歴の `cleared`、JSON の null が値を残すこと）。日付と帳簿の答えを機械が計算すること（カレンダーと帳簿を変えると、同じシナリオの期限と残高が変わる）。上へ回したあとの期限と催促（二度目も、自分の期限か催促を書いたステップも、回した先での保留と取り消しも。期限より前に来ない催促は置かないこと）。鳴った時計が先であること。範囲の端の読み方と、計算した値の範囲の断りが直すフィールドを挙げること。理由を求める却下と差し戻しを、理由が無い（コメントが無いか、空か、Unicode の White_Space だけ）とき断り、空白でない一文字なら受け付けること。`submitted by`、`same approver approves once`、`same approver approves once, at the last stage`（待たせること、先のステップの承認で済ませること、待つ先を失ったステップを決め直すか始めること、並列のステップの部分ごとの扱い、差し戻し、取り消し、取り下げ、保留、代理の席、エージェントのステップ）、期限を過ぎたステップ、いちばん上のステップ、いなくなった申請者、代理の席、管理する人の操作、使う日の範囲の外の日。英語の名前と日本語の名前で同じ JSON。`run` の終了コードとヘルプ | `tests/machine.rs`、`tests/machine/`、`tests/golden/run/` |
| 機械と検査の突き合わせ | 機械の申請と例の、シナリオのフォームごとに、二つの版のどの人も申請者にして、機械でチケットを最後まで歩かせ、`simulate --every-requester` の答え（出せない人、決め方ごとの承認者と議決の数、承認者がいないときの回り方、期限を過ぎて回す先、前の承認で済むステップ、あとのステップの承認を待つステップ）と比べる。検査が `ok` の下で数える一件ずつ（`cases`）と、W510、E502、W502 の言う申請者が、機械のすることと合う。どの歩き方も、検査が数えたいちばん長い日数を越えない（待たせた部分をあとで始めるステップのある申請でも） | `tests/machine.rs` |
| 終わることと仮押さえ | E401、W402、E405〜E408 の golden と変異。承認者が直せるフィールドを読む仮押さえ（引数が額を読む、条件が読む、引数が明細の合計を読む）と、注のとおりに直すと通ること。数えた日数の `ok` の行 | `tests/golden/codes/`、`tests/mutants/`、`tests/fixes.rs`、`tests/machine.rs` |
| シナリオの検査 | E602〜E604 の golden と変異（違うステップ、違う状態、出たステップ、ほかの人の承認、終わったあとの行、一分早い期限、時計が鳴ったあとの行） | `tests/golden/codes/`、`tests/mutants/` |
| 組織 | 版、予約と `--at`、読めないデータを一度言うこと、代理の期間と申請の種類、たどること、マスタの人、`effective from` と版、各版のいちばん上で上へ回すステップ（書かない形、`then stay`、`then skip`、`then <決め方>` と、その決め方にも人がいないとき）、組織の形で回るものを `ok` の下で数えることと JSON の `cases`、空いた席で回るものの W502、どのステップにも答える人がいない申請者の W510、期限を過ぎて回す先の無いステップ（人、チーム、表、二度目、上へ回った先の長、`then` の決め方の人。組織の形なら `ok` の下の数と `cases`、上の席が空いているか、空いた席で回ってきたなら W507）、`same approver approves once` で済むステップと済まないステップ（あいだのステップ、並列のほかの部分、エージェント、上へ回したあと）、`, at the last stage` で待つステップと待たないステップ（エージェントが先に答えるステップ）、もう一つの書き方を添える注、`submitted by` のグループごとの出せる人（`everyone in`、`team`、`position … or higher in`、`person`、`except`、代理申請）と、版にいないチームと人、だれも出せない版、パスの書き方。人の読む言語（`language`。`en`、`ja`、空のほかは E501）。E5xx、W500、W502、W506、W507、W510 の golden と変異 | `tests/org.rs`、`tests/golden/codes/`、`tests/mutants/` |
| 使う日の範囲 | 範囲の始まりと終わり、穴、`--through`、`ritsu check --through`。保留と再申請の期限を入れた最後の日が、機械の受け付ける最後の日と一日も違わないこと | `tests/cli.rs`、`tests/machine.rs`、`crates/ritsu/tests/check.rs` |
| `simulate` | 一人の申請者、フォームの値、全部の申請者のまとめ、`same approver approves once` で済むステップ、`, at the last stage` で待つステップ（必ず待つ、来れば待つ、待つ先になりうるステップが二つ以上）と JSON のキー、決め方のキーの並びが `ticket` に何を書いた申請の種類でも同じこと、期限を過ぎても上へ回せないステップに添える文、出せない申請者を断ることと、出せない人の頭の行と `may_not_submit`、`--on` の無いときの今日の読み方（時計を引数で渡す）、JSON、断る引数 | `tests/cli.rs`、`tests/org.rs` |
| ベクタ | 例（英語と日本語の名前）と機械の申請のベクタの一覧の golden（`--list`）と、休暇の JSON Lines の全部。一行が七つのキーの遷移一つであること。二度作って同じバイトになること。ベクタの持つ物だけを機械に渡すと、同じ答えになること。組織を版の全部に替えて渡しても、同じ答えになること。サーバーの持つ物だけから切った組織の部分が、ベクタの部分と同じこと。シナリオの行が `run --format json` の一歩と同じであること。どこから作ったか（シナリオ、議決の境、差し戻しの上限、休みをまたぐ期限、どの申請者も、代理、版をまたぐチケット、フォームの境、例の行、あとのステップの承認を待っているステップ、値を空にすること、条件で要るフィールドを空にした申請、理由を求める却下と差し戻し：コメント無し、空、空白だけ、U+0085 だけで断られる行と、U+FEFF だけ、U+001C だけで受け付けられる行）。名前が括弧の中の名前であること、英語と日本語の名前の申請が同じベクタになること、例の日本語の名前の版が英語の版と同じ本数のベクタ（代理の行も）を持つこと、`--lang` が文だけを変えること。ベクタを作れないファイルの終了コード | `tests/vectors.rs`、`tests/golden/vectors/` |
| ベクタと生成物 | TypeScript、Python、Go、Rust の生成物が、ベクタで参照インタプリタと同じ答えを返す（断りは種類まで。比べる物と比べない物は試験の `same` の一か所に書く）。下ろした先の言語が別の物に読む名前の申請と、名前を組み合わせると同じ名前になる申請も、付け替えた名前で組めて、同じ答えを返す。生成物は `tsc`（strict）、`mypy --strict`、`go vet` と `gofmt`、clippy の `-D warnings` を通る。道具が無ければ `SKIP:` の行を出す | `crates/ritsu/tests/gen_requests.rs`（段階 D） |
| 途中のフォームの答え | 参照（`engine::worked`）の答え：条件が成り立つ、成り立たない、決まらない（読むフィールドが、申請者が入れるのに空、範囲の外か選択肢に無い値、上限より多い行）。隠れたフィールドと空白だけの文を空として読むこと。明細の行の合計をある行から求め（下限より少なくても）、行の中の計算と数える日を機械と同じに求めること。承認者の直しを、直すフィールドだけ、フォームの値の代わりに読むこと。約束の外の呼び出しを止め、何も計算しない種類はフィールドを答えないこと。`vectors/worked/` のベクタがどの場合も持つこと。四つの出力先の入口 `worked` が、どのベクタでも参照と同じ答えを返すこと。画面の関数が、行が上限より多いフォームのほかは同じ答えを返すこと（13.7） | `tests/worked.rs`、`crates/ritsu/tests/gen_requests.rs`、`tests/a2ui/worked.rs` |
| 約束の文書 | 例の `--target requests` の文書がサーバーとエージェントに言うこと：`mcp/tools.json` の道具（エージェントが持つ操作と、戻せない操作）、途中のフォームを尋ねる二つの操作、操作の冪等キー、一覧のページとコメントの制限、操作ごとの全部の答えとエラーの本文の一つの形、それぞれの 404 が言うこと、版の範囲、断りの文と場所、申請とフィールドの説明、人とチームとマスタの行を探す操作とそのベクタ、種類の説明の flow、種類の一覧の言語と題、理由を求める操作、人の読む言語 | `tests/documents.rs` |
| 画面 | 例の 10 の申請の種類の画面とカタログを、写した A2UI 1.0 のスキーマ（標準カタログと ritsu のカタログを合わせて）で検証する。どの画面も一つの部品の木で、ritsu の部品と関数にカタログを書き、データにある置き場だけを読む。入力の部品がフィールドの範囲と検査を持ち、申請するボタンがフォームの全部の検査を持つ。ボタンは見せた値だけを送り（E701、W702）、イベントは API の操作で、本文の取るキーを送る。画面を壊す変更（見せない値を送る、桁を変える、`@` を外した名前、カタログを書かない部品と呼び出し、データに無い置き場）を見つける。数を人が数える単位で読んで見せること（`tests/machine/steps.ringi` の率とセントと銭の 10 の数を、申請書の `AmountField` の桁と刻みと単位、承認カードの `formatSteps` の引数、検査の文が言う範囲、Node の `formatSteps` と検査の文が書く 125 の刻みで。刻みを外すと W702。13.1）。`functions.js` を Node でベクタに通す（13.7。例の申請と、選択の値を規則が別の名前で書く申請。理由を求める操作のベクタのコメントで、`commentGiven` が機械と同じに答えること。Node が無ければ `SKIP:` の行）。理由を求める種類の承認カードで、却下と差し戻しのボタンだけが、カードの言語で理由の検査を持つこと（13.1）。計算した値の範囲の検査の文が、直すフィールドを見る人の言語の題で挙げること（4.4、13.7）。画面と参照インタプリタを、`types/<名前>.json` からフィールドごと、検査の種類ごとに作ったフォームで突き合わせる（その検査だけを破るフォームと、端でちょうど通るフォーム。申請、出し直し、承認者の直しで。描かれる部品とボタンの検査の判定が参照インタプリタの判定と同じで、両方が通すときは、画面が送る値と見せる値をチケットが持つ。分かれてよいのは文の両端の空白だけ。標準カタログの書式の関数に null を渡さない。テンプレートから検査を一つ抜くと見つける。13.7）。E703 と E704 の golden と変異。経費と休暇の画面、カタログ、`functions.js` の golden と、カタログの ID の渡し方と断り（`--catalog`、ritsu.mod の行）。日本語の実行でも `requests` のファイルが同じこと。`requests` だけを書く実行でも、日本語の実行でも、どのファイルも一度だけ検査すること（`crates/ritsu/tests/once.rs`）。E703 と E704 で止まる申請が `ritsu check` を通り、`ritsu gen` の最後の行が、英語でも日本語でも JSON でも、出力先が書けないファイルの形であること（13.3）。レンダラは手元で走らせない（走らせるときはサンドボックスで） | `tests/a2ui.rs`、`tests/a2ui/forms.rs`、`tests/fixtures/a2ui-1.0/`、`tests/golden/codes/`、`tests/mutants/`、`crates/ritsu/tests/gen_requests.rs`、`crates/ritsu/tests/once.rs`、`crates/ritsu-base/tests/modules_manifest.rs` |
| 言語をまたぐ検査 | X18（15 の形）、X19（11）、X20（チケットが flow に渡す側の 24 と、flow がチケットを開く側の 7）の、成り立つ形と成り立たない形と決められない形の golden（英語と日本語）。申請をディレクトリ、名前、絶対パス、申請のディレクトリの中からのどれで渡しても、文と JSON の記録が同じこと（相手が根の外にあっても）。申請を足しても、今の X2〜X6 の記録と診断が変わらないこと。E911、E209、E210 が `ritsu gen` を止めること。判定だけの試験（数え方、値の比べ方、`when` の追い方）。口の `Gates::allowed_with`（sekisho）と `Flows::declared`（dandori）の答え。台帳の再現が、そのコードだけを出すこと | `crates/ritsu/tests/cross.rs`、`gen.rs`、`crates/ritsu-cross/tests/tickets.rs`、`crates/sekisho/tests/ports.rs`、`crates/dandori/tests/api.rs`、`crates/ritsu-cross/tests/codes.rs` |
| 中の物と `api` | 例のどのファイルも、持つ物の一覧が golden と同じこと。定義が、桁のそろえ方とコメントで変わらず、意味を変える直しで、その物だけが変わること。`api` の JSON が、口の `Items` と `Requests` の言うことと同じこと。`api` を止める物。yuen の要件が ringi の物を指し、確かめない種類を `verified by` に書けば E403 になること | `tests/items.rs`、`tests/golden/items/`、`tests/golden/api/`、`crates/ritsu/tests/ringi.rs` |
| JSON の名前 | 日本語の名前で書いた五つの例の `check`、`simulate`、`api` の JSON が、名前を括弧の中の名前で書き、書いたとおりの名前を横に持つこと | `tests/cli.rs` |
| 一つの誤りは一度 | 宣言の無い名前（E004）と経路に無いステップ（E308）を、ファイルで最初に書いた所で一度だけ言い、同じ名前を書いたほかの行を注に挙げること。種類か宣言する所の違う同じ綴りは別に言うこと。英語の材料と日本語の版の、英日の golden（9.1） | `tests/chains.rs`、`tests/chains/` |
| 別のカレンダーで数える催促 | 主張の E403 の golden と変異。上へ回したあとの催促が、催促のカレンダーの営業日に鳴ること。催促のカレンダーのデータが先に終わるときの、使える最後の日と、その日に作ったチケットの時計。催促だけを数えるカレンダーの文と種類 | `tests/mutants/`、`tests/machine.rs`、`tests/machine/apart.ringi` |
| 仮押さえを作れない申請者 | 機械が `submit` を断る人と、`simulate` が頭の行で言う人と、JSON の `holds_refused` が、どの版のどの人でも同じこと | `tests/machine.rs`、`tests/machine/held.ringi` |
| ほかのファイルから来る文言 | 規則の列挙から選ぶフィールドの値の題（フィールドの下の行と、`labels` の `<フィールド>.<値>`）。列挙に無い値の文言（E007）、マスタから選ぶフィールドの下の行（E001）、ほかの型のフィールドの下の行を断ること（英日）。E703 の golden と変異 | `tests/titles.rs`、`tests/items.rs`、`tests/golden/codes/`、`tests/mutants/` |
| サーバーのプロジェクトを始める | `ritsu ringi init` が、英語と日本語で、オプション無し、`--mcp`、`--temporal`、両方のそれぞれで書くファイルの golden。`//go:generate` の行が書く物が `ritsu gen` の物で、`--check` が差を言わないこと。`ritsu check` が警告無しで通すこと。環境の言語に従わないこと。止まるとき（`--module` が無い、Go のモジュールの無いパス、値を渡したオプション、行く手のファイル、書けない場所）は英日で言い、何も書かないこと。Go を組んで試すのは、フレームワークの試験である | `crates/ritsu/tests/ringi_init.rs`、`crates/ritsu/tests/golden/ringi-init/` |
| Lean | ベクタの突き合わせ | `crates/ritsu-model/tests/ringi.rs` |
| 文書 | README、`docs/reference.md`、スキルに貼る `.ringi` と CSV の行が例と試験のファイルの行であること、出力つきのコマンドを走らせると貼った物と同じになること、出力の無いコマンドが ringi のコマンドであること、リンクの先があること。reference がどの予約語も挙げること。サイトの色分けの語の一覧が `kw.rs` と同じこと。台帳が `explain --all` と同じこと | `tests/docs.rs`、`tests/skill.rs`、`tests/codes.rs`、`crates/ritsu/tests/ringi.rs` |

## 19. 例

部門をまたいで五つ。英語が先で、日本語の名前で書いた `<名前>.ja.ringi` を横に置く。会社の規程、予算、組織、人はどれも架空で、ファイルの説明にそう書いた。

| 例 | 部門 | 主に見せること |
|---|---|---|
| `expense.ringi` | 経理 | 経路の表、エージェントの確かめと決裁、上へ回す表、予算の仮押さえ、承認のあとの支払い、取り下げて予算を取り消すこと、`labels ja` |
| `purchase.ringi` | 総務 | ステップの条件の形、使う規則（決裁権限表）の出力で承認者を決める、取締役会の 60% の議決と、議決に届かなくなった却下、回覧、承認の取り消し、承認者が直すフィールド、上長と決裁者を兼ねる人が決裁で一度だけ答えること（5.12） |
| `leave.ringi` | 人事 | 共有の経路、出せる人の絞り（取締役会の人は出さない）、休暇の帳簿（有給のときだけ）、営業日の計算、開いている日、コメントを添えた差し戻しと出し直し、却下と差し戻しに理由を求めること（`comment required for`） |
| `account.ringi` | 情報システム | マスタから承認者を引く、条件のステップ、承認のあとの払い出し、上長がその人の最後のステップで一度だけ答えること（5.12） |
| `contract.ringi` | 法務 | 要約、受け付けた人が担当を指名、2 人の議決、人の指定と承認者がいないとき、保留、前の任意のステップへの差し戻し、再申請の期限切れ |

共有のファイル（`examples/shared/`）：組織（`company.ringi`）、マスタ（`masters.ringi`）、共有の経路（`manager_approval.ringi`）、カレンダー（`office.cal`）、予算と休暇の帳簿（`budget.book`、`leave.book`）、会社全体の決まり（`company.gate`）、決裁権限表（`authority.rule`）、flow 六つ、要件（`expense.req`）と規程の条のコピー四つ。組織のデータは二つの版（`org/2026-04-01/`、`org/2027-04-01/`）と代理、マスタのデータは三つ。人の名前は役職で書き（`Head of Sales Division`）、メールは `example.com`、会社は `Example Co. (fictional)` で、どれも架空である。経費の決まり（表、期限、予算）は、今の言語で書いた経費申請と同じ中身にした。

経費の例は、空行を除いて `expense.ringi` が 147 行、それだけが使う flow が三つで 73 行（エージェントの確かめと決裁、支払い）。今の言語で書いた経費申請では、同じ所に規則 64 行、期限の日付 24 行、flow 382 行、gate の大半、手で書いた API の文書 230 行がかかっていた。147 行には、例、主張、シナリオ四つ（購入と、出張の三つ：エージェントが決める日帰りの出張、エージェントが人へ渡す出張、申請者が取り下げる出張）、予算の仮押さえ、日本語の文言（25 行）、注（9 行）も入っている。

経費の例の頭（全文は `examples/expense.ringi`）：

```
request expense v1 "Expense request"
owner accounting

use organization org from "shared/company.ringi"
use calendar office from "shared/office.cal"
use book budget from "shared/budget.book"
use gate "shared/company.gate"
use flow precheck_agent from "shared/precheck.flow"
…
form
  category : choice "Category"
    travel   "Travel"
    expense  "Expenses"
    purchase "Purchase"
  items : rows "Items"  count >=1 <=20
    what  : text "What"  count <=100
    price : money[JPY] "Price"  range >=1JPY <=1_000_000JPY
  amount  : money[JPY] "Amount" = sum(items.price)  range >=1JPY <=10_000_000JPY
  …
who
  manager       = manager of requester
  dept_head     = head of requester.department
  accounting    = team accounting
  division_head = nearest division_head above requester
  check         = agent precheck_agent, else manager
  travel_agent  = agent travel_decision, else manager

ticket
  self approval  not allowed
  # A stage right after one the same person approved is done with that approval. No stage waits: the manager answers before accounting.
  same approver  approves once
  send back      to the requester, at most 3 times
  resubmit       within 3 business days
  withdraw       until decide
  # The president, the heads of divisions and the directors are above every department: their
  # requests hold no department's budget.
  on submitted   hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: requester.department, amount: amount)  when requester.department is not none
  on approved    post budget.reserve(request: ticket.id, attempt: ticket.attempt)
  on approved    launch pay(request_id: ticket.id, requester: requester.id, amount: amount, purpose: purpose)
  on sent back, rejected, withdrawn, expired  void budget.reserve(request: ticket.id, attempt: ticket.attempt)
```

経費と購買の仮押さえは、部の予算を押さえる。社長、本部長、取締役は、どの部の上にもいるので、その人の申請は予算を押さえずに進むと決め、仮押さえの行に `when requester.department is not none` を書いた（架空の決まり）。確定と取消の行は、開いている仮押さえが無ければ何もしない（7.4）。予算の帳簿の仮押さえは 365 日で切れる（9.3 の X19）。

`ritsu check examples` は 25 ファイル（rulec 1、koyomi 1、chobo 2、dandori 6、sekisho 1、ringi 13、yuen 1）を読み、言語のあいだの境目の 76 がどれも成り立つ。落ちるのは、2027 年の版で通らない契約の二本（E503 と E504。8.6）と、人がまだ確かめていないと言う `expense.req` の E301（21 件）である。

```text
$ ritsu check examples      （最後の行だけを抜いた）
ritsu check: 25 files (rulec 1, koyomi 1, chobo 2, dandori 6, sekisho 1, ringi 13, yuen 1): 3 fail (25 errors, 2 warnings), 22 pass (16 warnings); borders between the languages: 76 checked, 0 undecided
```

## 20. 捨てた形と、まだやらないこと

### 20.1 捨てた形

- **中に書いた物を ringi の構文と検査で持つ形（6 章の形 1）**：rulec の表の検査をもう一度書くことになり、P4 に反し、食い違いが入り込む。
- **今の言語の領域として埋め込む形（形 2）**：P1 を変え、rulec と koyomi にファイルの一部だけを読む形を足すことになる。
- **下ろしたファイルを利用者のリポジトリに置く形**：申請の種類一つが二つ以上のファイルになり、古いファイルが残りうる（6.3）。
- **データだけを出し、サーバーが解釈する形**（12.1）：意味をサーバーを書く人がもう一度書く。
- **チケットの進み方を rulec の機械に下ろす形**（12.1）：状態が小さな有限の集合にならない。
- **申請の種類ごとに gate を生成する形**：経路が決める承認者は組織のデータとチケットごとに変わり、今の sekisho の関係は一段までなので、組織を上へたどる指定を gate に書けない。ringi が承認者を決めて `Approvers` として渡し、gate は `principal in resource.approvers` の一段だけを見る。
- **エージェントが却下する形**：却下は人がする。エージェントは確かめるか、表が渡した範囲で決めるか、人に渡すかだけをする。
- **語**：`step`、`go up`、`start`、`cancel approval`、`file`、`words`、`never`、`hold`（保留の意味）、`unit` を捨てた（3.6）。
- **取り下げを終わりにする形**：取り下げたチケットは申請者の手元へ戻り、直して出し直せる。申請と承認の製品で普通のふるまいに合わせた。
- **出せる人を申請者の属性の条件で書く形**（`submit only when requester.position >= manager and …`）：自由に組めるが、条件の言葉に、版で変わるチームの ID を型として入れることになる。経路の条件は申請者を読まない（E309）ので、同じ形が場所によって読めたり読めなかったりする。グループを並べる `submitted by` にした（5.13）。
- **出せる人を gate の `submit` に書く形**：gate の主体の属性は役職とチームの木を持たず、ringi の一覧が出せる人を知れない。申請の種類のファイルを読んでも、だれが出せるかが分からない。gate の `submit` は会社全体の決まりのまま残し、両方を満たすときだけ出せる（12.4）。
- **離れたステップを、前の承認で済ませる形**（`same approver approves once` を、あいだにほかの人のステップがあっても効かせる）：決裁する人が、あいだの確かめの結果を見ないまま決裁したことになる。答えるのを後ろへ送る書き方（`same approver approves once, at the last stage`）を足した（5.12）。
- **待たせたステップをあとで始めるときに、元のステップを並びの中で動かす形と、今のステップだけを前へ戻す形**：動かす形は、先に答えた部分と、あとで答える部分の時間の位置が一つのステップに混ざり、承認の取り消しでどちらを消すかが決まらない。前へ戻す形は、時間の順を並びとは別に持つことになる。同じ名前のステップをもう一度足す形にした（7.1）。
- **期限を過ぎて回したステップを止めて、それを待っていたステップを先に始める形**：時計と承認者を持ったまま止めるステップの状態が、保留のほかにもう一つ要る。回したステップの答えがそろってから、待っていたステップを始める形にした（5.12）。
- **`simulate` の JSON の `once_if_it_comes` と `once_among` を、`same approver approves once, at the last stage` を書いた申請の種類の決め方だけに持たせる形**：読む側（ページ、サーバー、生成する型）が、申請の種類によって決め方の形を変えることになる。どの申請の種類の決め方にも持たせ、あとのステップを待つことの無い申請の種類では false と空のリストにした（16.2）。
- **いちばん上の人の申請の承認者を、経路の条件で申請者を読んで決める形**（`when requester is at the top` など）：経路の表に申請者の属性を入力として足すことになり、版ごとに表の答えが変わる。上へ回す先が無いときに回す決め方を書く `when empty escalate, then <決め方>` にした（5.4）。
- **仮押さえのキーを、フォームの値の要約から作る形**：値を変えた出し直しは新しいキーになるが、断られたあとに同じ値で出し直すと（予算が増えたあとなど）同じキーに当たり、帳簿が覚えた断りを受ける。保存しなかった試みの、取り消したキーに、同じ値のあとの試みが当たって何も押さえないことも残る。操作ごとの試みの名前をキーにした（7.1、12.4）。
- **帳簿が断りを覚えない形と、取り消したキーでもう一度押さえられる形**：どちらも chobo のふるまいを変える。前の形では、送り直した呼び出しが同じ答えを受けることが崩れ、取り消したキーがあとの試みを止めることは残る。あとの形では、取り消したあとに届いた送り直しが押さえ直し、保存しなかった試みの仮押さえが残る。試みの名前なら、chobo を変えずに四つのこと（12.4）が成り立つ。

### 20.2 まだやらないこと

2.2 の後の版に回す物のほか：

- 画面のレンダラの web 以外（ネイティブ、アシスタントの中）の試し。
- 規則の表示名を、規則と違う言語の申請の種類で題に使うこと。規則の表示名は一つの言語で一つなので（rulec の DESIGN §15.211）、いまは申請の種類がフィールドの下か `labels` に、その言語の題を書く（4.8）。
- 申請の種類のあいだの関係（購買の稟議が承認されたら発注の申請を開く）。flow を通せば今でも書ける。
- 組織の人事の仕組みからの同期。CSV と JSON で受ける。
- 下ろしたファイルの診断の言葉の直しを、全部のコードについて持つこと。最初の版は 9.2 の表の分だけで、ほかはエラーを E201、警告を W201 にして元の文を見せる。
- 経路のステップの条件（`when`）を読んで、一緒には来ない二つのステップを W506 で比べないこと。今は、条件を読まずに、両方来るものとして比べる（8.6）。
- 承認者の直しで、仮押さえを押さえ直すこと。今は、仮押さえの条件か引数が読むフィールドを承認者が直せる形を、E405 にする（7.6）。
- 仮押さえの行の `when` が、`requester.<種類> is not none` のほかの条件のときに、その条件で仮押さえを作らない申請者を、X19 の人数から外すこと。今は、その条件を読まずに、値の無い人を全部数える（9.3）。
- X19 の日数を、条件の両立まで読んで、ちょうどの数にすること。今は、検査が数えた営業日の上限をカレンダーに置く（9.3）。
- X20 で、`or` と `not` のある `when`、渡す値と一緒に計算するフィールドを読む `when` を追うこと。今は、合わないように見えるときだけ W210 にして、追う形を言う（9.3）。
- チケットの終わりを、それを開いた flow に返して待つ形（15.4）。2.2 の後の版に回した。
- 期限を数える申請の種類の検査のたびに、koyomi が下ろした `terms.cal` の 146,097 日を一度数えること（debug のビルドで 0.2 秒ほど）を省くこと。同じ検査の中で二度は数えない。省くには、koyomi の側に、数えた結果を持ち回る口が要る。
- いちばん長い日数を、カレンダーの並びと条件の両立まで読んで、ちょうどの数にすること。今は上限を数える（7.6）。
- 例の日付のセルが、`open in` のカレンダーの開いている日かを確かめること。開いている日はカレンダーのデータを読んで決まり、今は範囲だけを確かめる（5.3）。

## 21. 確かめで見つかった気になる点と、設計への効き方

今の言語で経費申請を書いたときに見つかった 11 件は、どれも今の言語の側で直すことにし、どれも直った（根の PLAN 7.135〜7.140）。ringi は、直る前の形に回り道を作らず、直ったあとの形を前提にした。それぞれについて、何が直り、ringi が何に頼るかを書く。

1. **dandori のシナリオが `ritsu run` でそのまま走らない**（日付と帳簿の答えをシナリオが持ち、ずれる）：直った（根の PLAN 7.137）。`ritsu dandori scenarios` は、日付と帳簿の答えを口で計算し、シナリオに書かない。ringi のシナリオも日付と帳簿の答えを持たず、`run` が計算する（10.2）。承認のあとに始める flow のシナリオは、`ritsu run` で流す。
2. **参照インタプリタの `now` が、待ちのあとも始めの時刻のまま**：直った（根の PLAN 7.137）。`now` は、それを読む文が走るときの時計を読む（dandori の DESIGN 1.16）。ringi はイベントごとに時刻を渡し、ステップの時計を、そのステップが始まった時刻から数える。上へ回したあとの期限と催促は、元の期限が切れた時刻から数える（7.5）。走り始めの時刻のまま数える機械と、上へ回したあとに数え直さない機械は、試験が落とす（`tests/machine.rs`）。
3. **sekisho のページが、起こりえない組み合わせを「許す」に載せる**：直った（根の PLAN 7.138）。sekisho は、規則から起こりうる組だけを数える。X18 はページを読まず、属性の値を決めて gate に尋ね（`Gates::allowed_with`）、この数え方の答えを使う。
4. **sekisho のページの空の文**：直った（根の PLAN 7.138）。0 通りのときの文と、行き先の無い注である。ringi には効かない。会社の gate の下書きは、どの action にも principal を書いた。
5. **enums のファイルから import した列挙が、`use rule` の別名ごとに別の型になる**：直った（dandori は根の PLAN 7.136、kaname と sekisho は 7.140）。一つの `enums` ファイルの列挙は、どの名前で読んでも一つの型である。下ろした規則は、選択の列挙を `choices.rule` から一度だけ読むので、`.rule` の段ではもともと起きない（段階 C1 で確かめた）。下ろした `.kn`（12.2）は、この形を前提にする。
6. **列挙の値のリストが `for … in [a, b]` で E002**：直った（根の PLAN 7.136）。ringi は下ろすときにこの形を書かない。
7. **一つの主張の失敗が、ほかの検査に連鎖して 18 件のエラーになる**：直った（根の PLAN 7.135）。`ritsu check` は、一つのファイルの誤りを一度だけ言い、それを読むほかのファイルの行を、根ごとの「待っている」塊に並べる（根の DESIGN 8.3）。ringi の E202 も待っている行になる（16.3）。下ろしたファイルどうしの連鎖は、利用者に見えないファイルなので、ringi の診断一つにまとめ、ほかを数だけ言う（6.5）。
8. **gate の workflow の検査が、flow の match の絞り込みを読まない**：直った（根の PLAN 7.139）。ゲートがワークフローに許すものの検査（X16）は、フローの分岐で絞られた値を読む。チケットの操作は人が行い、gate の `workflow` の principal を使わないので、ringi には効かない。
9. **rulec の E101 のヒントが、帯の行でなく等号のセルの行を示す**：直った（根の PLAN 7.138）。rulec は、表がすでに書いている区切りと範囲の端だけで切った行を、穴の全部について返す（8 か所まで。残りは次の実行で言う）。ringi の E301 は、その行を ringi の表の列に並べ直し、入力のセルを作り直さない。どの行にも当てはまらない申請の範囲は、行のセルから申請の言葉で言う（6.5）。
10. **`ritsu gen .` が、yuen の E301 で `ritsu check .` が落ちていても 0 で書き出す**：直った（根の PLAN 7.135）。`ritsu gen` は、パッケージに入る言語の検査と言語をまたぐ検査のどれかがエラーを言えば、何も書かずに 1 で終わる。yuen の要件と来歴の記録では止めない（根の DESIGN 9.3）。ringi のファイルが検査を通らなければ、`ritsu gen` は ringi の物も書かない（12 章）。
11. **void に retry を付けても W101 が消えない**：直った（根の PLAN 7.135）。W101 の注が、警告の出た形に合う直し方を言う。ringi の取消は、サーバーが冪等キーで行うので、dandori の void を生成しない。ringi には効かない。
