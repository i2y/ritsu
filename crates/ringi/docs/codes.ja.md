# 診断のコード

`ringi explain --all --format markdown --lang ja` の出力です。手で直しません。

<a id="e001"></a>

## E001 — 読めない行です

**いつ出るか**: 行が ringi の書き方に合わないとき。知らない語や記号、閉じていない文字列や括弧、タブ、全角の空白、存在しない日付、ブロックの順の誤り、ファイルの種類に欠かせないブロック（申請の種類の `form`、`route`、`ticket` など）が無いとき、選ぶ値の無い `choice`、`key` か `fields` の無いマスタのときに出ます。`step` と書いた行には、`stage` と書くよう言います。

**直し方**: 示された位置を、メッセージと注のとおりに直してください。経路の一段は `stage`（ステップ）と書きます。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

every step
  due in 3 business days at end of day

ticket
  self approval  not allowed
```

関連: [E006](#e006)

<a id="e002"></a>

## E002 — 使えない名前です

**いつ出るか**: 名前が、条件を書く 7 語（`and` `or` `not` `is` `none` `true` `false`）のどれかのとき。フィールドの名前が、式が読む `requester` か `ticket` のとき。承認者の決め方の名前が、`when empty escalate, then …` の語の `skip` か `stay` のとき。ASCII でない名前の後ろに、括弧で ASCII の名前を書いていないとき、括弧の中が ASCII の名前でないとき。名前が、コードを生成する言語（TypeScript、Python、Go）の予約語のとき。フォームと明細の行のフィールドの ASCII の名前が、kaname の予約語（`then` `match` `some` など）のとき。名前が、kaname が生成するコードの補助のモジュールとその型の名前（`kaname` `KanameError` `Q` `Date` `Rat` `glue`）のとき。ASCII の名前が `_` だけのとき。申請の種類の ASCII の名前が、`_` で始まるか `_` で終わるとき、`_` が二つ続くとき。フォームの二つのフィールド（明細の一つの行の二つの欄も）の ASCII の名前が、Go に生成するコードで同じ名前になるとき（`a_b` と `aB` はどちらも `AB`）。名前の付いた表の入力の列が、フィールドと同じ名前で、行に承認者の決め方を書いているとき。承認者の決め方の名前が、いくつかの決め方やその割合を書いたステップの値（`a and b` の `a_and_b`、`all of legal` の `all_of_legal`）と同じとき。最後の三つは、重なる名前の一つがチケットの JSON のキーか値なので、ringi がほかの名前に置き換えられません。

**直し方**: 別の名前にしてください。日本語の名前には、`金額(amount)` のように、すぐ後ろの括弧に ASCII の名前を書いてください。生成するコードと JSON は、その ASCII の名前を使います。表の列なら、承認者の決め方を書く列に、どのフィールドにも無い名前を付けてください。申請の種類の名前は、英字で始め、英字か数字で終え、`_` を続けないでください。下ろしたファイルと生成するコードの名前の頭になります。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額 : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
```

関連: [E003](#e003)

<a id="e003"></a>

## E003 — 同じ名前が二度宣言されています

**いつ出るか**: 同じ置き場（フォームのフィールド、選ぶ値、明細の列、経路のステップ、`who`、表とその列、`use` の別名、主張、シナリオ、役職、チームの種類）に、同じ名前か同じ ASCII の名前を二度宣言したとき。フィールドの名前が `use` の別名と同じとき、同じ言語の `labels` を二度書いたとき、`labels` に同じ項目を二度書いたときにも出ます。

**直し方**: どちらかの名前を変えるか、二つを一つにまとめてください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance)    = team finance
  経理(accounting) = team accounting

ticket
  self approval  not allowed
```

関連: [E002](#e002), [E004](#e004)

<a id="e004"></a>

## E004 — 宣言の無い名前です

**いつ出るか**: 宣言の無い名前を読んだとき。フィールド、選ぶ値（`choice` の値と、`choice from` が選ぶ規則の列挙の値）、ステップ、`who` の承認者の決め方、表とその列、`use` の別名、マスタとそのフィールド、組織の役職とチームの種類のどれでもない名前です。`manager of requester` や `requester.department` のように組織を読むものを、`use organization` の無いファイルに書いたときと、カレンダーを読む営業日の数え方を、カレンダーを決められないファイルに書いたときにも出ます。同じ種類の同じ名前を何か所にも書いたときは、最初に書いた所で一度だけ出て、ほかの行の番号を注に挙げます（明細の列は明細ごと、マスタのフィールドはマスタごと）。

**直し方**: 注に並べた名前から選ぶか、その名前を宣言してください。組織を読むなら `use organization` を、営業日を数えるなら `use calendar` を書いてください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理部

who
  経理(finance) = team finance

ticket
  self approval  not allowed
```

関連: [E003](#e003), [E308](#e308)

<a id="e005"></a>

## E005 — 読めない `use` です

**いつ出るか**: `use` の行のファイルを読めないとき。ファイルが無いか読めない、拡張子が `use` の種類と合わない（`use calendar` に `.book`）、`.ringi` や `.cal` の最初の語が種類と合わない（`use organization` に申請の種類のファイル）、パスがプロジェクトの外を指す、パスが絶対パス（`/` や、ドライブの文字で始まる）のときに出ます。

**直し方**: パスと種類を直してください。パスは、そのファイルのあるディレクトリから書きます。種類と拡張子は、`organization`・`masters`・`route` が `.ringi`、`calendar`・`dates` が `.cal`、`book` が `.book`、`flow` が `.flow`、`rule` が `.rule`、`gate` が `.gate` です。

**再現**:

```ringi
request 例(example) v1 "例"

use calendar 本社(office) from "予算.book"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
```

関連: [E202](#e202)

<a id="e006"></a>

## E006 — この種類のファイルには書けないブロックです

**いつ出るか**: ファイルの種類が持たないブロックを書いたとき（組織のファイルの `route`、マスタのファイルの `form` など）。一度だけ書くブロックを二度書いたとき、`ticket` に一度だけ書く行（`on` のほかの行。`self approval`、`submitted by` など）を二度書いたとき、ファイルの種類を言う行を二度書いたとき、共有の経路のファイルが `use route` でほかの経路を読むときにも出ます。

**直し方**: そのブロックを、それを持つ種類のファイルに移すか、消してください。注に、その種類のファイルが持つブロックを順に並べます。経路を組み合わせるのは申請の種類のファイルです。二度書いたブロックや `ticket` の行は、一つにまとめてください。

**再現**:

```ringi
organization 会社(company) v1 "会社"

positions
  社員(staff)   "社員"
  課長(manager) "課長"

team kinds
  部(department) "部"

route
  確かめ(check)  経理
```

関連: [E001](#e001)

<a id="e007"></a>

## E007 — 何も指していない文言です

**いつ出るか**: `labels` の項目が、ファイルに無いものを指すとき。無いフィールド、選ぶ値、明細の列、経路に無いステップ（`stage …`）、`who` に無い承認者の決め方（`who …`）、申請の種類でないファイルの `request` です。

**直し方**: 項目の名前を、ファイルの中の名前に合わせて直すか、要らない行を消してください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed

labels en
  金額         "Amount"
  stage 確かめ  "Check"
  stage 承認    "Approval"
```

関連: [E004](#e004), [E703](#e703)

<a id="e008"></a>

## E008 — ほかの言語が要るファイルです。`ritsu ringi` で確かめてください

**いつ出るか**: ほかの言語を持たない ringi のクレートのバイナリに、ほかの言語が要るファイルを渡したとき。申請の種類は kaname のモジュールに、フォームと経路は rulec の規則に、期限は koyomi の日付に下ろすので、どれも要ります。`use` で規則、カレンダー、帳簿、flow、gate を読むファイルも同じです。要る言語ごとに一つ、その言語を最初に要する行に出て、2 で終わります。組織とマスタのファイルは、ほかの言語が要りません。

**直し方**: 同じコマンドを、すべての言語をつないだ `ritsu ringi`（`ritsu ringi check <ファイル>`）で走らせてください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
```

<a id="e009"></a>

## E009 — 型の合わない値です

**いつ出るか**: 値の型が、書いた場所の型と合わないとき。単位の違う数を比べる・足す（`amount > 30_000` は、円の金額と単位の無い数）、条件に真偽でない値を書く、`range` を文字に・`count` を数に・`open in` を日付でないものに書く、範囲の端に値でないものや型の違う値を書く、明細の列を `sum` で囲まずに読むときなどに出ます。

**直し方**: 値を、書いた場所の型に合わせてください。金額には単位を付けて書きます（`30_000JPY`）。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount)    : money[円] "金額"  range >=1円 <=100_000円
  領収書(receipt) : attachment "領収書"  required when 金額 > 30_000

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
```

関連: [E004](#e004), [E101](#e101)

<a id="e101"></a>

## E101 — 範囲の書かれていない数、金額、日付です

**いつ出るか**: 申請者か承認者が入れる数、金額、日付のフィールドに、`range` の上下の端が無いとき。明細（`rows`）の行の数に上限が無いときにも出ます。範囲は、確かめる値の範囲と、画面と生成するコードが受け付ける値になります。計算するフィールドには要りません。

**直し方**: `range >=1 <=20` のように、両方の端を書いてください。明細には `count >=1 <=20` のように行の数の上限を書いてください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  日数(days) : number "日数"

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
```

関連: [E102](#e102), [E009](#e009)

<a id="e102"></a>

## E102 — 長さの上限の書かれていない文です

**いつ出るか**: 申請者か承認者が入れる文（`text`、`long text`）に、`count` の上限が無いとき。上限が無いと、どれだけ長い文でも受け付けることになります。

**直し方**: `count <=100` のように、入れられる文字の数の上限を書いてください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  目的(purpose) : long text "目的"

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
```

関連: [E101](#e101)

<a id="e103"></a>

## E103 — 計算した値が一度も入らない範囲です

**いつ出るか**: 計算するフィールドに書いた範囲と、計算した値がとりうる範囲（読むフィールドの範囲から ringi が数えます）が、少しも重ならないとき（`total = price * quantity` が 1 万円までなのに、`range >=20_000JPY <=30_000JPY`）。計算するフィールドに書いた範囲は、受け付ける値の上下の端です。計算した値が範囲の外になる申請は出せず、画面と生成するコードが、人が入れるフィールドの範囲と同じように断ります。そのフィールドを読む経路の表、条件、ほかの計算、主張は、書いた範囲の中だけを見ます。範囲が、とりうる範囲より広くても、一部だけ重なっても、誤りではありません。どの申請も出せない範囲だけが、このエラーです。

**直し方**: 範囲を、計算した値がとりうる範囲と重なるように直すか、読むフィールドの範囲を直してください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  単価(price)    : money[円] "単価"  range >=1円 <=1_000円
  数量(quantity) : number "数量"  range >=1 <=10
  合計(total)    : money[円] "合計" = 単価 * 数量  range >=20_000円 <=30_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
```

関連: [E101](#e101), [E104](#e104)

<a id="e104"></a>

## E104 — 丸め方の書かれていない計算です

**いつ出るか**: 計算するフィールドの式が、そのフィールドの刻みで割り切れない値になることがあるのに、`round` が無いとき（円の金額に `* 3%` を掛ける、整数の数を割る）。

**直し方**: `round down(1JPY)` のように、丸め方（`up`、`down`、`half_up`、`half_down`、`half_even`）と刻みを書いてください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount)  : money[円] "金額"  range >=1円 <=100_000円
  手数料(fee)   : money[円] "手数料" = 金額 * 3%

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
```

関連: [E009](#e009), [E103](#e103)

<a id="e105"></a>

## E105 — 隠れていることのある必須のフィールドです

**いつ出るか**: 必須のフィールドが、表示の条件によっては隠れているのに必須になるとき（`required when amount > 30_000JPY` と `shown when category is travel`）。申請者は、見えないフィールドを入れられません。二つの条件を下ろした規則で rulec に尋ね、そうなる申請の例を添えます。rulec が決められないときは、W201 で決められないと言います。

**直し方**: 必須の条件を表示の条件の中に収めるか、表示の条件を広げてください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  種別(category) : choice "種別"
    出張(travel)  "出張"
    経費(expense) "経費"
  金額(amount)  : money[円] "金額"  range >=1円 <=100_000円
  領収書(receipt) : attachment "領収書"  shown when 種別 is 出張  required when 金額 > 30_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
```

関連: [E106](#e106), [W201](#w201)

<a id="e106"></a>

## E106 — 入る前のフィールドを読む条件です

**いつ出るか**: 申請のときに決めること（フィールドの表示と必須の条件、経路の表とステップの条件、共有の経路に渡す値、`on submitted` の行）が、承認者だけが入れるフィールド（`approvers only`）を、そのまま、または計算を通して読むとき。申請のとき、そのフィールドはまだ空です。

**直し方**: 申請のときに決めることは、申請者が入れるフィールドだけを読むようにしてください。承認者が決めることなら、それを読むステップの指名に移してください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount)     : money[円] "金額"  range >=1円 <=100_000円
  勘定科目(account) : text "勘定科目"  count <=10  approvers only
  領収書(receipt)   : attachment "領収書"  required when 勘定科目 is "6100"

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
```

関連: [W108](#w108), [E305](#e305)

<a id="e107"></a>

## E107 — 選べる行の無い日があるマスタです

**いつ出るか**: 申請を使う日の範囲の中に、`choice from` や `lookup` で読むマスタの行が一つも効いていない日があるとき（勘定科目の行が 2026 年 6 月で切れ、次の行が 8 月から）。使う日の範囲は、`effective from`（無ければ、読むデータがそろう最初の日）から、読むデータが続く最後の日までです。診断は、穴の日と、前後の行の有効期間をどう直せば埋まるかを言います。データの終わりは誤りにせず、`check` が「いつまで使えるか」として言います。`check --through <日付>` を渡すと、その日の前にマスタの行が終わることもこのエラーです。

**直し方**: その日にも効く行を足すか、前後の行の `valid_to`・`valid_from` を延ばして、穴を埋めてください。`--through` なら、その日まで効く行を足してください。

**再現**:

```ringi
request 例(example) v1 "例"

use masters マスタ(m) from "masters.ringi"

form
  勘定科目(account) : choice from マスタ.accounts "勘定科目"

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
```

`masters.ringi`:

```
masters company_masters v1
description "The account codes a request chooses from (fictional)"

master accounts "Account codes"
  key code
  fields
    code  : text "Code"  count <=10
    label : text "Name"  count <=60
  data "accounts.csv"
```

`accounts.csv`:

```
code,label,valid_from,valid_to
5103,Supplies,2026-01-01,2026-06-30
5110,Supplies and small equipment,2026-08-01,
```

関連: [E404](#e404), [E505](#e505)

<a id="w108"></a>

## W108 — 承認者が直すと経路が変わるフィールドです

**いつ出るか**: `approvers edit` で承認者が直せるフィールドを、経路（表の列、ステップの条件、共有の経路に渡す値、`lookup … by`、`chosen by requester in`）が、そのまま、または計算を通して読むとき。直したときは、今のステップより後の経路を直した値で決め直し、前のステップの答えは変えません。

**直し方**: 経路が変わってよいなら、そのままでかまいません。変わってはいけないなら、`approvers edit` からそのフィールドを外してください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理
  決裁(decide)   部長  when 金額 > 50_000円

who
  経理(finance) = team finance
  部長(heads)   = team finance_heads

ticket
  self approval  not allowed
  approvers edit 金額
```

関連: [E106](#e106)

<a id="w109"></a>

## W109 — 秘密の値を入れるフィールドです

**いつ出るか**: 文のフィールドの名前、ASCII の名前、題が、秘密の値を指す語（`password`、`secret`、`token`、`api key`、パスワード、暗証番号など）を含むとき。入れた値はチケットの履歴に残り、承認者と、ページとログを読める人に見えます。

**直し方**: 秘密の値は申請で受け取らず、それを扱う仕組み（鍵の置き場、本人が入れる画面）で渡してください。秘密でない値なら、名前と題を、秘密に読めない言い方にしてください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  システム(system)       : text "システム"  count <=30
  パスワード(password)   : text "パスワード"  count <=50

route
  確かめ(check)  情報システム

who
  情報システム(it_desk) = team it_desk

ticket
  self approval  not allowed
```

関連: [W901](#w901)

<a id="e201"></a>

## E201 — 中に書いた物が、下ろした先の言語の検査を通りません

**いつ出るか**: ringi のファイルを下ろした規則か日付に、ringi のコードに直せないエラーが出たとき（計算するフィールドの値が int64 に収まらない。rulec の E108）。元の言語のコードと文を注に書きます。下ろしたファイルを置くパス（ファイルの隣の `<名前>.lowered/`）に、ディスクのファイルがあるときにも出ます。`ritsu gen` が申請のために置くコード（申請ごとのつなぎと、チケットの機械）に kaname が何か言ったとき（警告も、total でない関数も）にも、申請の最初の行に出ます。これは ringi の不具合で、注には kaname のコードと行だけを書きます。

**直し方**: 注の元のコードと文を読んで、示された行を直してください。`.lowered/` にファイルがあるときは、そのファイルを消すか、別の名前にしてください。`ritsu gen` が置くコードについてのものは、申請を直さず、申請と ritsu の版を添えて ringi の不具合として知らせてください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  単価(price)    : money[円] "単価"  range >=1円 <=9_000_000_000_000_000円
  数量(quantity) : number "数量"  range >=1 <=10_000
  合計(total)    : money[円] "合計" = 単価 * 数量

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
```

関連: [W201](#w201), [E202](#e202)

<a id="w201"></a>

## W201 — 中に書いた物に、下ろした先の言語が警告を出します

**いつ出るか**: ringi のファイルを下ろした規則か日付に、ringi のコードに直せない警告が出たとき（フィールドの ASCII の名前が、コードを生成する言語の予約語で、生成するコードでは後ろに `_` が付く。rulec の W121）。元の言語のコードと文を注に書きます。主張と、必須なのに見せていないことがあるか（E105 の問い）を、下ろした規則で決められなかったときにも出ます。

**直し方**: 注を読んで、それでよいか確かめてください。よくなければ、示された行を直してください（名前なら、別の ASCII の名前にします）。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円
  実装(impl)   : choice "作り方"
    社内(inhouse) "社内で作る"
    委託(vendor)  "外に頼む"

route
| 実装 | 金額        | -> 決裁(decide) |
| 社内 | <=50_000円  | 上長            |
| 社内 | >50_000円   | 部長            |
| 委託 | -           | 部長            |

who
  上長(manager) = team managers
  部長(head)    = team heads

ticket
  self approval  not allowed
```

関連: [E201](#e201), [E105](#e105)

<a id="e202"></a>

## E202 — `use` で読んだファイルが検査を通りません

**いつ出るか**: `use` で読んだファイルが、その言語の検査を通らないとき。ringi のファイル（組織、マスタ、共有の経路）は ringi の、規則は rulec の、カレンダーと日付のファイルは koyomi の、帳簿は chobo の、flow は dandori の、gate は sekisho の検査です。そのファイルのエラーを三つまで注に書きます。`ritsu check` では、この行は読んだファイルが通るのを待っている行として、そのファイルの誤りの下にまとめて出ます。

**直し方**: 読んだファイルを、その言語の検査が通るように直してください。

**再現**:

```ringi
request 例(example) v1 "例"

use organization 組織(org) from "会社.ringi"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"

team kinds
  部(department) "部"
```

関連: [E005](#e005)

<a id="e203"></a>

## E203 — 規則の出力の値に、承認者の決め方がありません

**いつ出るか**: 経路が `use rule` の規則の出力で承認者を決めるとき、その出力の列挙の値に、`who` の承認者の決め方が無いとき（`authority.decider` の値 `cfo` が `who` に無い）。出力の型は規則から読みます。承認者を決める出力が列挙でないときは E009、規則に無い出力は E004 です。

**直し方**: `who` に、その値の名前で承認者の決め方を足してください。

**再現**:

```ringi
request 例(example) v1 "例"

use rule 権限(authority) from "authority.rule"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  決裁(decide)  権限.decider

who
  上長(manager) = team managers

ticket
  self approval  not allowed
```

`authority.rule`:

```
rule authority v1
description "Who decides, by the amount (fictional)"

enum decider = manager "Manager" | cfo "Chief financial officer"

inputs
  amount : money[JPY]  range >=1JPY <=100_000JPY

outputs
  decider : decider

table decider
| amount      | -> decider |
| <=50_000JPY | manager    |
| >50_000JPY  | cfo        |
```

関連: [E004](#e004)

<a id="e301"></a>

## E301 — 経路の表に当てはまらない申請があります

**いつ出るか**: 経路の表のどの行にも当てはまらない申請があるとき（購買の 100 万円を超える行が無い）。表を下ろした規則で rulec が見つけます。当てはまらない申請の範囲をフィールドの名前と単位で言い、その一つを例に添え、足す行を表の形で示します。行の入力のセルは、表がすでに書いている区切りと範囲の端だけで切ってあり、ステップのセル（経路を選ぶ表なら経路のセル）は `?` です。`?` は検査を通らないので、貼っただけの行は、答えを書くまでそこで止まります。範囲が 8 か所を超えるときは、最初の 8 か所の行を示し、残りは次の検査で言います。

**直し方**: 示された行を表に足し、`?` に、そのステップに答える承認者（経路を選ぶ表なら共有の経路）を書いてください。行の途中で答えが変わるなら、そこで行を分けてください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=1_000_000円

route
| 金額         | -> 確かめ(check) | 決裁(decide) |
| <=100_000円  | 経理             | 上長         |

who
  経理(finance) = team finance
  上長(manager) = team managers

ticket
  self approval  not allowed
```

関連: [E302](#e302), [E303](#e303)

<a id="e302"></a>

## E302 — 経路の表の二つの行に当てはまる申請があります

**いつ出るか**: 一つの申請が、経路の表の二つの行に当てはまるとき（`<=100_000JPY` と `>=100_000JPY`）。表を下ろした規則で rulec が見つけ、両方に当てはまる例を添えます。両方に当てはまる申請が一つでなければ、その範囲も言います（二つの行が同じ申請を取るときと、片方に当てはまる申請がどれももう片方にも当てはまるときは、そう言います）。

**直し方**: 端の片方を `<` か `>` にして、行が重ならないようにしてください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=1_000_000円

route
| 金額         | -> 決裁(decide) |
| <=100_000円  | 上長            |
| >=100_000円  | 部長            |

who
  上長(manager) = team managers
  部長(head)    = team heads

ticket
  self approval  not allowed
```

関連: [E301](#e301)

<a id="e303"></a>

## E303 — どの申請も当てはまらない経路の表の行です

**いつ出るか**: 経路の表の行に、どの申請も当てはまらないとき（`amount` の範囲が 100 万円までなのに、`>1_000_000JPY` の行）。表を下ろした規則で rulec が見つけます。

**直し方**: 行の区切りを直すか、その行を消してください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=1_000_000円

route
| 金額                     | -> 決裁(decide) |
| <=100_000円              | 上長            |
| >100_000円 <=1_000_000円 | 部長            |
| >1_000_000円             | 部長            |

who
  上長(manager) = team managers
  部長(head)    = team heads

ticket
  self approval  not allowed
```

関連: [E301](#e301), [W303](#w303)

<a id="w303"></a>

## W303 — どの申請も届かないステップの承認者です

**いつ出るか**: 経路のステップの承認者に、どの申請も届かないとき。ステップの条件が、読むフィールドの範囲の中では成り立たない（`amount` が 10 万円までなのに `when amount > 200_000JPY`）、`only when` の主張だけが指す承認者のときです。経路を下ろした規則で rulec が見つけます。`possible` や例だけが指す承認者は、その主張か例が成り立たないこと（E306、E601）で言います。

**直し方**: 行やステップの条件を直すか、どの申請も届かない物を経路から消してください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理
  部長(head)     部長たち  when 金額 > 200_000円

who
  経理(finance)    = team finance
  部長たち(heads)  = team heads

ticket
  self approval  not allowed
```

関連: [E303](#e303), [E306](#e306)

<a id="e304"></a>

## E304 — 議決に要る数が、承認者の数を超えます

**いつ出るか**: `at least 2 of …` のように二人以上を求める議決が、一人を決める承認者の決め方（`person`、`manager of requester`、`head of`、`nearest`、`chosen by`、`lookup`）を数えるとき。`at least …%` が 100% を超えるときにも出ます。

**直し方**: 人数を数える議決は、`team legal` のようにチームで決める承認者に使ってください。一人に答えてもらうなら、議決をやめて名前だけを書いてください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  決裁(decide)  at least 2 of 社長

who
  社長(president) = person "e0101"

ticket
  self approval  not allowed
```

<a id="e305"></a>

## E305 — 前のステップが指名できない指名です

**いつ出るか**: `chosen by previous approver` のステップより前に、条件の無い承認のステップが無いとき（最初のステップ）。前の承認者が指名するのに、申請者が入れるフィールドで指名するときと、申請者が指名するのに、承認者だけが入れるフィールドで指名するときにも出ます。

**直し方**: 前の承認者が指名するなら、そのステップの前に承認のステップを置き、指名するフィールドに `approvers only` を書いてください。申請者が指名するなら `chosen by requester in …` と書いてください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount)       : money[円] "金額"  range >=1円 <=100_000円
  確かめる人(reviewer) : person "確かめる人"  approvers only

route
  確かめ(review)  指名された人

who
  指名された人(chosen_reviewer) = chosen by previous approver in 確かめる人

ticket
  self approval  not allowed
```

関連: [E106](#e106)

<a id="e306"></a>

## E306 — 経路の主張が成り立ちません

**いつ出るか**: `claims` の `only when` や `possible` が、経路の表では成り立たないとき（`decide is manager only when amount <= 50_000JPY` に、5 万 1 円で上長の行）。下ろした規則に rulec が尋ね、成り立たない例を添えます。

**直し方**: 例を読んで、表か主張のどちらが誤りかを決めて直してください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=1_000_000円

route
| 金額         | -> 決裁(decide) |
| <=100_000円  | 上長            |
| >100_000円   | 部長            |

who
  上長(manager) = team managers
  部長(head)    = team heads

ticket
  self approval  not allowed

claims
  小さい額は上長が決める : 決裁 is 上長 only when 金額 <= 50_000円
```

関連: [E601](#e601), [W303](#w303)

<a id="e307"></a>

## E307 — 共有の経路に、受け取らない引数を渡しています

**いつ出るか**: `route 名前(…)` で共有の経路に渡す値が、その経路の `inputs` と合わないとき。受け取らない名前、同じ名前の二度、型の違う値、渡していない入力（`optional` でないもの）です。経路の表のセルが、入力を受け取る共有の経路を使うときにも出ます。

**直し方**: 注に並べた入力の名前と型で渡してください。入力を受け取る共有の経路は、`route 名前(入力: …)` のように一行で使ってください。

**再現**:

```ringi
request 例(example) v1 "例"

use route 標準(standard) from "標準.ringi"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route 標準(日数: 5)

ticket
  self approval  not allowed
```

`標準.ringi`:

```
route 標準(standard) v1 "標準の承認"

inputs
  長い(long) : bool

route
  確かめ(check)  経理
  部長(head)     部長たち  when 長い

who
  経理(finance)    = team finance
  部長たち(heads)  = team finance_heads
```

関連: [E009](#e009)

<a id="e308"></a>

## E308 — 経路に無いステップです

**いつ出るか**: `withdraw until`、主張、`examples` の列、シナリオの行が、経路に無いステップを指すとき（`withdraw until counsel` で、経路に `counsel` が無い）。同じ名前を何か所にも書いたときは、最初に書いた所で一度だけ出て、ほかの行の番号を注に挙げます。

**直し方**: 注に並べたステップの名前から選んでください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
  withdraw       until 法務
```

関連: [E004](#e004)

<a id="e309"></a>

## E309 — 経路かフォームが読めない条件です

**いつ出るか**: 経路の条件（ステップの `when`、経路の表の列、共有の経路に渡す値、`only when` の主張）か、フォームの条件（`shown when`、`required when`）が、表に下ろせない物を読むとき。申請者（`requester.department`、`requester.id`）、チケット（`ticket.round`）、文や人やマスタから選ぶフィールド、数か日付でない計算するフィールド、二つの値どうしの比べ（`amount > limit`）、条件の中の計算（`sum(items.price) > 100_000JPY`）で出ます。経路とフォームの条件は、読む値を列にした表に下ろして rulec が確かめるので、表の列にならない物は読めません。値の無いことがある物（任意のフィールド、`none` になることのある規則の出力、共有の経路の入力のうち申請が渡していない物）は、値の無いことを `none` という値の一つとして読みます（DESIGN 4.9）。検査を通った申請の種類には、いつも下ろした経路があります。

**直し方**: 数どうしの比べや計算は、計算するフィールドに置いて、そのフィールドを値と比べてください（`difference : money[JPY] = amount - limit` として `when difference > 0JPY`）。文のフィールドで分けるなら `choice` にしてください。申請を出せる人を絞るなら `ticket` に `submitted by` を、組織のいちばん上の人の申請をだれかに承認させるなら上に人がいないときの回し先を書いてください（`who` に `board = team board` と宣言して `when empty escalate, then board`）。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円
  上限(limit)  : money[円] "上限"  range >=1円 <=100_000円

route
  確かめ(check)  経理
  監査(audit)    取締役会  when 金額 > 上限

who
  経理(finance)   = team finance
  取締役会(board) = team board

ticket
  self approval  not allowed
```

関連: [E009](#e009), [E307](#e307), [E502](#e502)

<a id="e401"></a>

## E401 — 主張の日数のうちに終わらないことがあるチケットです

**いつ出るか**: `every ticket ends within … business days` が成り立たないとき。チケットが長くていくつ営業日かかるかを数えます。ステップは期限の日数、期限を過ぎて上へ回すなら回すたびに同じ日数（期限が日の終わりなら、回す日の一日も）です。経路のいちばん長い並びのステップを足し（条件で来るステップは、来るものとして数えます）、戻せる回数だけくり返し、戻るたびに `resubmit within` の日数を足し、`pause at most` の日数を足します。数え方を添えます。数は上限なので、主張が数以上なら必ず成り立ちます。数のとおりにかかるチケットが、あるとは限りません。終わらないことのあるチケット（期限を過ぎても待つステップ、期限の無いステップ、`resubmit within` の無い差し戻し）も、このエラーです。

**直し方**: 主張を添えた数にするか、期限、戻せる回数、保留の日数を減らしてください。終わらない待ちには、終わりを書いてください（`expire` か `escalate`、`due in …`、`resubmit within …`）。

**再現**:

```ringi
request 例(example) v1 "例"

use calendar 事務所(office) from "office.cal"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理
  決裁(decide)   取締役会

who
  経理(finance) = team finance
  取締役会(board) = team board

every stage
  due in 3 business days at end of day
  when late escalate, then expire

ticket
  self approval  not allowed
  send back      to the requester, at most 2 times
  resubmit       within 5 business days
  pause          at most 4 business days

claims
  期限のうちに終わる : every ticket ends within 30 business days
```

`office.cal`:

```
calendar office v1
description "Saturdays and Sundays are closed (fictional)"
offset +09:00

closed weekly sat, sun
```

関連: [W402](#w402)

<a id="w402"></a>

## W402 — いつまでも待つことのあるステップです

**いつ出るか**: 期限を過ぎても待ち続けるステップがあるとき（`when late stay`、`when late escalate, then stay`）。期限を書いてあるのに、チケットはそこで終わりません。`every ticket ends within` の主張があれば E401 です。期限の無いステップと、`resubmit within` の無い差し戻しも終わりの無い待ちですが、警告にはせず、`ok` の下の行（`may never end: …`）で言います。

**直し方**: 待ってよいなら、そのままでかまいません。終わらせるなら、`stay` を `expire` か `escalate` にしてください。

**再現**:

```ringi
request 例(example) v1 "例"

use calendar 事務所(office) from "office.cal"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

every stage
  due in 3 business days at end of day
  when late stay

ticket
  self approval  not allowed
```

`office.cal`:

```
calendar office v1
description "Saturdays and Sundays are closed (fictional)"
offset +09:00

closed weekly sat, sun
```

関連: [E401](#e401)

<a id="e403"></a>

## E403 — 期限より前に来ない催促があります

**いつ出るか**: `claims` に `every stage reminds before it is due` を書いたのに、催促が期限と同じか後に来ることがあるとき（`remind after 3 business days at 10:00` と `due in 3 business days at 09:00`）。下ろした日付で koyomi が、数え始める日のすべてについて確かめ、いちばん外れる日を添えます。催促と期限を別のカレンダーで数えるステップ（`due in 2 business days in office at end of day` と `remind after 2 business days in plant at 09:00`）は、ringi がステップの始まりうる日を一日ずつ数えて確かめ、最初に外れる日と、外れる日の数を添えます。

**直し方**: 催促を早めるか、期限を遅らせてください。別のカレンダーで数えているなら、催促と期限を同じカレンダーで数える直し方もあります。

**再現**:

```ringi
request 例(example) v1 "例"

use calendar 事務所(office) from "office.cal"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

every stage
  due in 3 business days at 09:00
  remind after 3 business days at 10:00

ticket
  self approval  not allowed

claims
  期限の前に催促する : every stage reminds before it is due
```

`office.cal`:

```
calendar office v1
description "Saturdays and Sundays are closed (fictional)"
offset +09:00

closed weekly sat, sun
```

関連: [E404](#e404)

<a id="e404"></a>

## E404 — 使う日の中に、カレンダーのデータが無い日があります

**いつ出るか**: 期限やフォームの日付を営業日で数え始める日が、カレンダーの休みの表が始まるより前に来ることがあるとき。`effective from` が表の最初の日より前のときと、営業日で数えるフォームの日付（`back_on = first_day + days business days in office`）の数え始める日付の範囲が、表より前から始まるときです（表は 2026 年からで、`first_day` は 2025 年 10 月から）。表の終わりは誤りにせず、`check` が「いつまで使えるか」と、それを決めるカレンダーを言います（それより後に始まるステップや、それより後の日付から数えるフォームは、足りないデータを言って断ります）。`check --through <日付>` を渡すと、その日の前に表が終わることもこのエラーです。

**直し方**: カレンダーの休みの表を、その日まで前へ広げるか、`effective from` や数え始める日付の範囲を、表が始まる日からにしてください。`--through` なら、その日のあとまで表を足してください。

**再現**:

```ringi
request 例(example) v1 "例"

use calendar 事務所(office) from "office.cal"

form
  初日(first_day) : date "初日"  range >=2025-10-01 <=2027-12-31
  日数(days)      : number "営業日数"  range >=1 <=20
  復帰日(back_on) : date "復帰日" = 初日 + 日数 business days in 事務所

route
  確かめ(check)  上長

who
  上長(manager) = team managers

ticket
  self approval  not allowed
```

`office.cal`:

```
calendar office v1
description "Saturdays, Sundays and the holidays of 2026 and 2027 are closed (fictional)"
offset +09:00

source holidays = file "holidays.csv" sha256:5927e83156e00064
  format csv
  covers 2026-01-01..2027-12-31

closed weekly sat, sun
closed holidays
```

`holidays.csv`:

```
2026-01-01,New Year's Day
2027-01-01,New Year's Day
```

関連: [E403](#e403), [E107](#e107)

<a id="e405"></a>

## E405 — 確定も取消もされずに残るか、申請の値と食い違うことのある仮押さえがあります

**いつ出るか**: `on submitted hold` で押さえた仮押さえが、開いたまま迎えることのある終わり方（承認、却下、差し戻し、取り下げ、期限切れ）のどれかで、確定も取消もされないとき（`on expired` に `void` が無い）。`when` の付いた `post` と `void` は、仮押さえに同じ `when` が付いているときだけ数えます。`submitted` 以外で押さえる `hold` も、閉じる行が無いので、このエラーです。仮押さえの条件（`when`）か引数が、承認者の直せるフィールド（`approvers edit`）を、そのまま、または計算するフィールドか規則の出力を通して読むときも、このエラーです。仮押さえは、申請を出したときの値で一度だけ行い、承認者が直しても押さえ直しません。額を直せば、確定される額が申請の額と食い違い、条件が成り立たなくなれば、同じ条件の `post` と `void` が行われず、仮押さえが開いたまま終わります。

**直し方**: その終わり方に `post` か `void` の行を足してください。承認者の直せるフィールドを読む仮押さえは、そのフィールドを `approvers edit` から外してください。

**再現**:

```ringi
request 例(example) v1 "例"

use calendar 事務所(office) from "office.cal"
use book 予算(budget) from "budget.book"

form
  金額(amount) : money[JPY] "金額"  range >=1JPY <=100_000JPY
  部(dept)     : text "部"  count >=1 <=20

route
  確かめ(check)  経理

who
  経理(finance) = team finance

every stage
  due in 3 business days at end of day
  when late expire

ticket
  self approval  not allowed
  on submitted   hold 予算.reserve(request: ticket.id, attempt: ticket.attempt, dept: 部, amount: 金額)
  on approved    post 予算.reserve(request: ticket.id, attempt: ticket.attempt)
  on rejected    void 予算.reserve(request: ticket.id, attempt: ticket.attempt)
```

`office.cal`:

```
calendar office v1
description "Saturdays and Sundays are closed (fictional)"
offset +09:00

closed weekly sat, sun
```

関連: [E406](#e406), [E407](#e407)

<a id="e406"></a>

## E406 — 先に仮押さえの無い確定か取消です

**いつ出るか**: `post` か `void` の振替に、先に `hold` した仮押さえが無いとき（`on approved post` だけで `on submitted hold` が無い）。

**直し方**: `on submitted hold` を足すか、その行を消してください。

**再現**:

```ringi
request 例(example) v1 "例"

use book 予算(budget) from "budget.book"

form
  金額(amount) : money[JPY] "金額"  range >=1JPY <=100_000JPY

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
  on approved    post 予算.reserve(request: ticket.id, attempt: ticket.attempt)
```

関連: [E405](#e405)

<a id="e407"></a>

## E407 — このチケットには起きない終わり方の処理です

**いつ出るか**: チケットの決まりでは起きない終わり方に `on` の行があるとき。`send back` の行が無いのに `sent back`。`withdraw` の行も `send back` の行も無いのに `withdrawn`（戻ったチケットは申請者がやめられるので、`send back` があれば起きます）。期限で切れるステップも `resubmit within` も無いのに `expired`。人が答えるステップが無いのに `rejected`。

**直し方**: その行を消すか、その終わり方を許してください。

**再現**:

```ringi
request 例(example) v1 "例"

use book 予算(budget) from "budget.book"

form
  金額(amount) : money[JPY] "金額"  range >=1JPY <=100_000JPY
  部(dept)     : text "部"  count >=1 <=20

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
  on submitted   hold 予算.reserve(request: ticket.id, attempt: ticket.attempt, dept: 部, amount: 金額)
  on approved    post 予算.reserve(request: ticket.id, attempt: ticket.attempt)
  on rejected, withdrawn  void 予算.reserve(request: ticket.id, attempt: ticket.attempt)
```

関連: [E405](#e405)

<a id="e408"></a>

## E408 — 試みを見分けないキーの仮押さえです

**いつ出るか**: 仮押さえの振替のキーが、試みを見分ける値（`ticket.attempt`）を受け取らないとき（キーが申請と `round` だけ）。帳簿はキーごとに答えを覚え（断りも覚えます）、取り消したキーではもう押さえません。サーバーが帳簿をチケットと別のデータベースに置くと、帳簿の断りはチケットが保存されなくても残ります。キーが申請と `round` だけなら、値を変えた出し直しや予算が増えたあとの出し直しも、同じ回のうちは同じキーで断られ、保存できずに取り消した仮押さえのキーに、あとの試みが当たって何も押さえません。`ticket.attempt` は、サーバーが `submit` と `resubmit` に渡す試みの名前で、同じ操作の送り直しには同じ名前、新しい操作には新しい名前です。chobo が帳簿を読めるときに確かめます。

**直し方**: 振替に試みの引数を足してキーに入れ（`attempt: string` と `key request, attempt`）、`hold`、`post`、`void` に `attempt: ticket.attempt` を渡してください。

**再現**:

```ringi
request 例(example) v1 "例"

use book 予算(budget) from "budget.book"

form
  金額(amount) : money[JPY] "金額"  range >=1JPY <=100_000JPY
  部(dept)     : text "部"  count >=1 <=20

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
  on submitted   hold 予算.reserve(request: ticket.id, round: ticket.round, dept: 部, amount: 金額)
  on approved    post 予算.reserve(request: ticket.id, round: ticket.round)
  on rejected    void 予算.reserve(request: ticket.id, round: ticket.round)
```

関連: [E405](#e405), [E406](#e406)

<a id="e409"></a>

## E409 — このチケットには無い操作に理由を求めています

**いつ出るか**: `comment required for` が、チケットの決まりでは起きない操作を挙げたとき。`send back` の行が無いのに `send back`。人が答えるステップが無いのに `reject`。E407 と同じ見方で決めます。

**直し方**: その操作を行から消すか、その操作を許す行を書いてください。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[JPY] "金額"  range >=1JPY <=100_000JPY

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
  comment required  for reject, send back
```

関連: [E407](#e407), [E006](#e006)

<a id="w500"></a>

## W500 — 組織のデータが無いので、承認者が空くかを決められません

**いつ出るか**: 申請の種類が読む組織のファイルに `data` の行が無いか、その置き場にデータが無く、`--org` も無いとき。承認者のいないステップ、届かない議決、二つのステップに当たる同じ人は決めずに、ほかの検査だけを行います。通ったことにはせず、この警告で言います。マスタのファイルが、人のフィールドの行を、組織と比べられないとき（組織を読まないか、読んだ組織にデータが無い）にも出ます。

**直し方**: 組織のファイルに `data` の行を書いてデータを置くか、`--org` でデータのディレクトリを渡してください。

**再現**:

```ringi
request 例(example) v1 "例"

use organization 組織(org) from "会社.ringi"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  上長

who
  上長(manager) = manager of requester

ticket
  self approval  not allowed
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"
```

関連: [E501](#e501)

<a id="e501"></a>

## E501 — 読めない組織のデータです

**いつ出るか**: 組織のデータが読めないとき。列が足りない、知らない役職やチームの種類、無いチームや人を指す行、`parent` の輪、長がそのチームの人でない、主の所属が一人に一つでない、同じ ID が二度、版のディレクトリの名前が日付でない、などです。上限を越える値（ID やマスタのキーは 255 バイト、名前は 200 字、マスタの値は 1,000 字、アドレスは 254 バイト）、サーバーが断る文字（制御文字、書式の文字、行や段落を区切る文字）、形の合わないアドレス、たたむと同じになる二人のアドレス、`organization.json` の最上位と要素の中の知らないキー、CSV の読み方の誤り（列とは違う個数の値、大きすぎる行、引用符の誤り）もです。データのファイルの行で言い、その版を読む申請の種類は、組織のファイルが通るのを待ちます（E202）。マスタのデータの誤り（列が足りない、キーの重なり、型の合わない値）も、マスタのファイルの行でこのエラーです。

**直し方**: 示されたデータの行を直してください。

**再現**:

```ringi
organization 例(example) v1 "例"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,section,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

関連: [W500](#w500), [E202](#e202)

<a id="e502"></a>

## E502 — 組織のある版で、承認者のいないステップがあります

**いつ出るか**: 組織のある版とある申請者で、ステップに答える人がいないのに、チケットがそのステップで待ち続けるとき。`when empty stay`（書かなければこれ）のステップと、`when empty escalate` で上に回せる人がいない（組織のいちばん上）ステップのうち、`then skip` も `then <決め方>` も書いていないものと、`then <決め方>` の決め方にも人がいないものです（社長の申請の `manager of requester`、長の空いたチーム、自己承認を許さないのに申請者しかいない、組織にいない人）。版ごと、ステップごとに一つにまとめ、当たる申請者の数と三人までの名前を言います。`effective from` の日に効いている版が無いときも、このエラーです。

**直し方**: 版のデータを直すか、ステップに `when empty escalate`（上へ回す）か `when empty skip`（飛ばす）を書いてください。上に回せる人がいないときは、飛ばしてよいなら `when empty escalate, then skip` と書き、だれかが見るべきなら、`then` のあとに、その人の申請に答える承認者の決め方を書いてください（`who` に `board = team board` と宣言して `when empty escalate, then board`）。`then` のあとの決め方に人がいなければ、どの版でも人のいる決め方にしてください。

**再現**:

```ringi
request 例(example) v1 "例"

use organization 組織(org) from "会社.ringi"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  上長

who
  上長(manager) = manager of requester

ticket
  self approval  not allowed
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

関連: [W502](#w502), [E503](#e503)

<a id="w502"></a>

## W502 — 承認者がいないので、ほかへ回るか飛ばすステップがあります

**いつ出るか**: 組織のある版とある申請者で、版のデータが欠けているために承認者のいないステップが、`when empty escalate` で上の長へ回るか、`when empty skip` で飛ばされるとき。上へたどった先の長の席が空いていて、`then skip` で飛ばされるときと、`then <決め方>` の決め方へ回るときもです。データの欠けは、長の空いたチーム、人のいないチーム、版にいない人かチーム、上へたどる途中の空いた長の席です。空いた席を書いたとおりに埋めてよいのか、データの誤りなのかは人が決めることなので、警告にします。組織の形でほかへ回るか飛ばされるステップ（承認者が申請者だけで自己承認を許さない、申請者が組織のいちばん上にいる、上に決め方の種類のチームが無い）は、書いたとおりに回るので警告にせず、`ok` の下の行で申請者の数を言います。だれの申請がどこへ回るかは `ringi simulate --every-requester` で見られます。回る先の人ごとに申請者をまとめて言います。

**直し方**: 版のデータを直すか、そのままでよいかを確かめてください。

**再現**:

```ringi
request 例(example) v1 "例"

use organization 組織(org) from "会社.ringi"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  部長

who
  部長(head) = head of requester.部

every stage
  when empty escalate

ticket
  self approval  allowed
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

関連: [E502](#e502), [W510](#w510)

<a id="e503"></a>

## E503 — 組織のある版にいない人かチームを、上へ回すステップが指しています

**いつ出るか**: `when empty escalate` のステップの承認者の決め方が、組織のある版にいない人（`person "e09"`）か無いチーム（`team legal`）を指すとき。いない人やチームからは、上へたどる元が無いので回せません。承認者の決め方の行で言います。`when empty stay` なら E502、`skip` なら W502 です。`then skip` や `then <決め方>` を書いても、このエラーです（どちらも、上に人がいないときにだけ効きます）。マスタの行の人が組織にいないときは E505 です。

**直し方**: ID を直すか、その版のデータに人かチームを足してください。

**再現**:

```ringi
request 例(example) v1 "例"

use organization 組織(org) from "会社.ringi"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  決裁(decide)  法務部長

who
  法務部長(counsel) = person "e09"

every stage
  when empty escalate

ticket
  self approval  not allowed
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

関連: [E502](#e502), [E505](#e505)

<a id="e504"></a>

## E504 — 議決に届かない申請者がいます

**いつ出るか**: 組織のある版とある申請者で、議決（`at least 2 of sales`）に要る数だけの承認者がいないとき（申請者自身を除くと、チームの人数が足りない）。

**直し方**: 議決の数を減らすか、チームに人を足してください。

**再現**:

```ringi
request 例(example) v1 "例"

use organization 組織(org) from "会社.ringi"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確認(review)  at least 2 of 営業部

who
  営業部(sales) = team sales

ticket
  self approval  not allowed
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

関連: [E304](#e304)

<a id="e505"></a>

## E505 — 組織にいない人の代理か、マスタの行です

**いつ出るか**: 代理の表（`proxies.csv`）の代わられる人か代わる人、またはマスタの行の人のフィールドの人が、その行の期間に効く組織の版にいないとき（辞めた人の代理）。行と版ごとに、データのファイルの行で言います。マスタの行を確かめるのは、マスタのファイルが `use organization` で組織を読むときです。

**直し方**: その行を直すか、期間を終わらせてください。

**再現**:

```ringi
organization 例(example) v1 "例"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

`org/proxies.csv`:

```
person,proxy,kind,from,to,requests
e09,e02,approve,2026-04-01,2026-06-30,
```

関連: [E503](#e503)

<a id="w506"></a>

## W506 — 一つのチケットの二つのステップに同じ人が当たります

**いつ出るか**: 組織のある版とある申請者で、別の承認者の決め方の二つのステップに、同じ一人だけが当たるとき（上長と部長が同じ人。チームの人が、決裁する人一人だけ）。一人が二度確かめても、二人が確かめたことにはなりません。当たる人ごとに申請者をまとめて言います。上へ回した先の人と、何人かのうちの一人で足りるステップは数えません。`ticket` に `same approver approves once` と書いた申請の種類では、あとのステップがすぐ前のステップの承認で済む組は言わず、済まない組には、済まない理由（あいだのステップ、前のステップのほかの部分、エージェントのステップ）を添えます。`same approver approves once, at the last stage` と書いた申請の種類では、前のステップがあとのステップの承認を待つので、エージェントが先に答えるステップとの組だけを言います。書いていないか、書いた行では一度にならない組には、一度になる書き方を添えます。

**直し方**: どちらかのステップの承認者の決め方を変えるか、一度の承認でよいなら `ticket` に `same approver approves once`（すぐ前のステップの承認で済ませる）か `same approver approves once, at the last stage`（あいだにほかのステップがあっても、最後のステップで一度だけ答える）と書くか、そのままでよいかを確かめてください。

**再現**:

```ringi
request 例(example) v1 "例"

use organization 組織(org) from "会社.ringi"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  財務
  決裁(decide)   財務部長

who
  財務(finance)    = team finance
  財務部長(head) = person "e04"

ticket
  self approval  allowed
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

関連: [E502](#e502)

<a id="w507"></a>

## W507 — 期限を過ぎても、上へ回す先のないステップがあります

**いつ出るか**: 組織のある版とある申請者で、`when late escalate` のステップを、版のデータが欠けているために上へ回せないとき。答える人の上の長の席が空いているときと、`escalate by <表>` の表が回す先の承認者の決め方が、データの欠け（長の空いたチーム、人のいないチーム、版にいない人かチーム）で人を見つけないときです。答える人には、承認者がいないので `when empty escalate` で回った先の人（上の長と、`then <決め方>` の決め方の人）も入ります。二度目に回すとき（`escalate, then escalate`）も見ます。上へ回せないステップは、`escalate` のあとに書いたことを、その期限で行います（`then expire` か何も無ければ期限切れ、`then stay` なら待ち続けます）。答える人が組織のいちばん上にいるときのように、組織の形で上へ回せないステップは、書いたとおりに期限で終わるので警告にせず、`ok` の下の行で申請者の数を言います。期限を過ぎたときにどうなるかは `ringi simulate` が言います。期限を過ぎる人ごとに申請者をまとめて言います。

**直し方**: 版のデータを直してください。ほかの人へ回すなら、`when late escalate by <表>` の表で回す先を決めてください。

**再現**:

```ringi
request 例(example) v1 "例"

use organization 組織(org) from "会社.ringi"
use calendar 事務所(office) from "office.cal"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  決裁(decide)  営業部長

who
  営業部長(sales_head) = person "e02"

every stage
  due in 3 business days at end of day
  when late escalate, then expire

ticket
  self approval  allowed
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

`office.cal`:

```
calendar office v1
description "Saturdays and Sundays are closed (fictional)"
offset +09:00

closed weekly sat, sun
```

関連: [W502](#w502), [W402](#w402)

<a id="e508"></a>

## E508 — 組織のある版にいないチームか人を、`submitted by` が指しています

**いつ出るか**: `ticket` の `submitted by` が、組織のある版に無いチーム（`everyone in legal`、`team legal`、`position … in legal`）か、いない人（`person "e09"`）を指すとき。ほかのグループに人がいても誤りにします。課がなくなって、出せるはずの人が出せなくなるのを、切り替えの前に見つけるためです。書いた場所で、版をまとめて言います。この誤りのある版では E509 を言いません。

**直し方**: ID を直すか、その版のデータにチームか人を足してください。

**再現**:

```ringi
request 例(example) v1 "例"

use organization 組織(org) from "会社.ringi"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  財務

who
  財務(finance) = team finance

ticket
  submitted by   everyone in legal
  self approval  not allowed
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

関連: [E509](#e509), [E503](#e503)

<a id="e509"></a>

## E509 — 組織のある版で、この申請をだれも出せません

**いつ出るか**: 組織のある版で、`submitted by` に当たる人が一人もいないとき（`except` で全員が外れるときも）。その版では、だれもこの申請を出せません。書いたチームか人が版にいないときは、E508 だけを言います。

**直し方**: `submitted by` を広げるか、版のデータを直してください。

**再現**:

```ringi
request 例(example) v1 "例"

use organization 組織(org) from "会社.ringi"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  財務

who
  財務(finance) = team finance

ticket
  submitted by   team finance except position 部長
  self approval  not allowed
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

関連: [E508](#e508)

<a id="w510"></a>

## W510 — どのステップにも答える人がいない申請者がいます

**いつ出るか**: 組織のある版とある申請者で、承認者のいないステップがどれも飛ばされ（`when empty skip`、`when empty escalate, then skip`）、ほかに答える人のいるステップが無いとき。その人のチケットは、出したとたんに、だれも答えないまま承認で終わります。社長の申請の上長のステップを、上に人がいないので `then skip` で飛ばすときなどです。一つ一つのステップが組織の形で書いたとおりに飛ばされていても、申請をだれも見ないので、警告にします。申請を出せる人（`submitted by`）だけを見ます。最初の承認のステップの行で言い、飛ばされるステップの組ごとに、申請者をまとめて言います。

**直し方**: だれかが見るべきなら、`then` のあとに、その人の申請に答える承認者の決め方を書くか（`who` に `board = team board` と宣言して `when empty escalate, then board`）、版のデータを直してください。その人が出さない申請なら、`submitted by … except …` で外してください。

**再現**:

```ringi
request 例(example) v1 "例"

use organization 組織(org) from "会社.ringi"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  上長

who
  上長(manager) = manager of requester

every stage
  when empty escalate, then skip

ticket
  self approval  not allowed
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

関連: [W502](#w502), [E502](#e502), [E509](#e509)

<a id="e511"></a>

## E511 — 組織のある版にないチームを、`owner` が指しています

**いつ出るか**: 申請の種類を受け持つチーム（`owner`）が、申請を使う日の組織のある版に無いとき。サーバーは、だれかが管理の画面で種類を扱うとき、このチームを会社の決まり（sekisho の gate）に渡します。版に無いチームはどの部門の物でもないので、部門を読まない規則（全体の管理者の物）しか、その種類の管理を許しません。書いた場所で、版をまとめて言います。

**直し方**: チームの ID を直すか、その版のデータにチームを足してください。

**再現**:

```ringi
request 例(example) v1 "例"
owner accounting

use organization 組織(org) from "会社.ringi"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  財務

who
  財務(finance) = team finance

ticket
  submitted by   everyone in sales
  self approval  not allowed
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

関連: [E508](#e508)

<a id="e601"></a>

## E601 — 経路の例が成り立ちません

**いつ出るか**: `examples` の行の答えが、経路の表の答えと違うとき。下ろした規則で rulec が確かめ、表の答えを添えます。例の行のセルに、フォームが取らない値を書いたときにも出ます。範囲の外の数や日付（計算するフィールドは、計算がとりうる値の外）、単位の刻みの倍数でない数、長さが `count` の外の文、マスタのどの行でもないキーです。経路がその列を読むかどうかにかかわらず、そのセルで言います。選べない値は E004、型の違う値は E009 です。

**直し方**: 例か表のどちらが誤りかを決めて直してください。フォームが取らない値は、取る値に直してください（例の行は、出せる申請です）。

**再現**:

```ringi
request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=1_000_000円

route
| 金額         | -> 決裁(decide) |
| <=100_000円  | 上長            |
| >100_000円   | 部長            |

who
  上長(manager) = team managers
  部長(head)    = team heads

ticket
  self approval  not allowed

examples
| 金額      | -> 決裁 |
| 50_000円  | 部長    |
```

関連: [E306](#e306), [E004](#e004), [E009](#e009)

<a id="e602"></a>

## E602 — シナリオが書いたとおりに進みません

**いつ出るか**: シナリオの行のあとのステップか状態が、チケットの進み方と違うとき。実際のステップと状態と、置いてある時計を添えます。`ringi check` はシナリオをどれもチケットの機械で流し、`ringi run` は進んだ所までを一行ずつ見せます。

**直し方**: シナリオか、経路とチケットの決まりのどちらが誤りかを決めて直してください。

**再現**:

```ringi
request 例(example) v1 "例"

use organization 組織(org) from "会社.ringi"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理
  決裁(decide)   営業

who
  経理(finance) = team finance
  営業(sales)   = team sales

ticket
  self approval  allowed

scenario 小さな経費
  requester e03
  form 金額 = 5_000円
| at               | event   | by  | -> stage | status      |
| 2026-06-01 09:00 | submit  | e03 | 確かめ   | in progress |
| 2026-06-01 10:00 | approve | e04 |          | approved    |
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

関連: [E603](#e603)

<a id="e603"></a>

## E603 — チケットが受け付けないイベントがシナリオにあります

**いつ出るか**: シナリオの行を、チケットが受け付けないとき。機械が断るイベント（承認者でない人の `approve`、終わったあとのイベント、2,000 字を超えるコメント）、帳簿が拒否する仮押さえ、二度目の `submit`、上の行より前の時刻、イベントが取らない引数、フォームが取らない値、時計か flow の行の `by` や `comment` のセルに書いたものです。

**直し方**: イベントか、それをする人か、時刻を直してください。

**再現**:

```ringi
request 例(example) v1 "例"

use organization 組織(org) from "会社.ringi"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理
  決裁(decide)   営業

who
  経理(finance) = team finance
  営業(sales)   = team sales

ticket
  self approval  allowed

scenario 小さな経費
  requester e03
  form 金額 = 5_000円
| at               | event   | by  | -> stage | status      |
| 2026-06-01 09:00 | submit  | e03 | 確かめ   | in progress |
| 2026-06-01 10:00 | approve | e02 | 決裁     | in progress |
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

関連: [E602](#e602)

<a id="e604"></a>

## E604 — 時計のイベントの時刻が違います

**いつ出るか**: シナリオの時計の行（`remind`、`late`、`resubmit late`、`pause ends`）の時刻が、時計が鳴る時刻と違うとき（期限が日の終わりなのに `late` を 23:59 に書いた）、その時計が置かれていないとき、時計が鳴る時刻かそのあとの行を、時計の行なしに書いたとき。置いてある時計と、書く行を添えます。時刻は下ろした日付が決め、シナリオは持ちません。

**直し方**: 時計の行を、添えた時刻に書いてください。

**再現**:

```ringi
request 例(example) v1 "例"

use organization 組織(org) from "会社.ringi"
use calendar 事務所(office) from "office.cal"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

every stage
  due in 3 business days at end of day
  when late expire

ticket
  self approval  allowed

scenario だれも答えない
  requester e03
  form 金額 = 5_000円
| at               | event  | by  | -> stage | status      |
| 2026-06-01 09:00 | submit | e03 | 確かめ   | in progress |
| 2026-06-04 23:59 | late   |     |          | expired     |
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

`office.cal`:

```
calendar office v1
description "Saturdays and Sundays are closed (fictional)"
offset +09:00

closed weekly sat, sun
```

関連: [E602](#e602)

<a id="e701"></a>

## E701 — 画面に見せていない値をボタンが送ります

**いつ出るか**: `ritsu gen --target requests` が書く画面のテンプレートで、ボタンが送る値を、その画面のどの部品も見せていないとき（承認カードが種別の文言を見せ、コードを送る）。テンプレートを書く前に、どの画面にも確かめます。テンプレートは申請のファイルの宣言から作るので、これが出るのは ringi の不具合で、ファイルの書き方では出ません。

**直し方**: 申請のファイルは直さず、その申請と ritsu の版（`ritsu --version`）を添えて、ringi の不具合として知らせてください。

**再現**: 再現はありません。これを出すテンプレートを ringi は書かず、`ritsu gen` がこれを言うのは ringi の不具合のときだけです。

関連: [W702](#w702)

<a id="w702"></a>

## W702 — 送る値と違う刻みで見せるか読む数です

**いつ出るか**: `ritsu gen --target requests` が書く画面のテンプレートが、ボタンが送る数を、送る刻みと違う刻みで見せるか読むとき（円の小数を整数に丸めて見せる、0.25% 刻みの率を 0.01% 刻みで読む）。E701 と同じく、ringi の不具合でだけ出ます。

**直し方**: 申請のファイルは直さず、その申請と ritsu の版（`ritsu --version`）を添えて、ringi の不具合として知らせてください。

**再現**: 再現はありません。これを出すテンプレートを ringi は書かず、`ritsu gen` がこれを言うのは ringi の不具合のときだけです。

関連: [E701](#e701)

<a id="e703"></a>

## E703 — 画面に出る項目に、その言語の文言がありません

**いつ出るか**: 画面に出る項目に、要る言語の文言が無いとき。一つは、`labels` で宣言した言語に文言が無いとき（`labels ja` に `items.price` が無い）。項目は、`request`、フィールド（明細の列は `<明細>.<列>`）、選ぶ値（`<フィールド>.<値>`。規則の列挙から選ぶ値も）、`stage <名前>`、`who <名前>` です。もう一つは、ほかのファイルから来る項目に、ファイルの言語の題が無いとき。規則の列挙から選ぶ値で、フィールドの下に題の行が無く、規則がファイルの言語で書いた表示名も無い物（フィールドの行で言う）と、ほかの言語で書いた共有の経路のステップと決め方で、経路の `labels <ファイルの言語>` に題が無い物（`use route` の行で言う）です。申請の検査（`ritsu check`）が言い、`ritsu gen --target requests` も画面を作る前に言って何も書きません。

**直し方**: その言語の `labels` に項目の文言を足すか、列挙の値の題をフィールドの下に書くか（`goods "物品・サービス"`）、共有の経路のファイルの `labels` に題を足してください。

**再現**:

```ringi
request 例(example) v1 "例"

use organization 組織(org) from "会社.ringi"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  上長

who
  上長(manager) = manager of requester

ticket
  submitted by   everyone in sales
  self approval  not allowed

labels en
  request  "Example"
  金額      "Amount"
  who 上長  "Manager"
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

関連: [E007](#e007)

<a id="e704"></a>

## E704 — 画面が正確に持てない数の範囲です

**いつ出るか**: 数のフィールドの範囲の端が、単位の刻みで数えて 2^53 − 1（9_007_199_254_740_991）を越えるとき。申請者が入れるフィールド、承認者のフィールド、計算するフィールド、明細の列のどれでも出ます。画面は数を JavaScript と同じに持つので、それを越える値は丸まり、別の値として確かめて送ってしまいます。`ritsu gen --target requests` が画面を作る前にフィールドの行で言い、何も書きません。

**直し方**: 範囲を、刻みで数えて ±9_007_199_254_740_991 の中に狭めてください。

**再現**:

```ringi
request 例(example) v1 "例"

use organization 組織(org) from "会社.ringi"

form
  金額(amount) : money[円] "金額"  range >=1円 <=10_000_000_000_000_000円

route
  確かめ(check)  上長

who
  上長(manager) = manager of requester

every stage
  when empty escalate, then skip

ticket
  self approval  not allowed
```

`会社.ringi`:

```
organization 会社(company) v1 "会社"
description "小さな会社（架空）"

positions
  担当(staff)     "担当"
  部長(dept_head) "部長"
  社長(president) "社長"

team kinds
  会社(company)  "会社"
  部(department) "部"

data "org"
```

`org/2026-04-01/teams.csv`:

```
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
```

`org/2026-04-01/people.csv`:

```
id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
```

`org/2026-04-01/members.csv`:

```
person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
```

関連: [E101](#e101)

<a id="w901"></a>

## W901 — 鍵の形の値がファイルに書いてあります

**いつ出るか**: `.ringi` のどこか（文字列でもコメントでも）に、プロバイダーが形を決めている鍵の形の値（AWS のアクセスキー ID、GitHub・Slack・Stripe・OpenAI・Anthropic・Google の鍵やトークン、Slack の Incoming Webhook の URL、PEM の秘密鍵）があるとき。ファイルが読めないときも出ます。診断には鍵の種類と、接頭辞と、長さだけを出し、鍵そのものも、その行も出しません。

**直し方**: 鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。本物の鍵なら、まずプロバイダーで無効にしてください。ファイルから消しても、リポジトリの履歴には残ります。テスト用の値なら、同じ行のコメントに `ritsu: test secret` と書いてください。

**再現**:

```ringi
request 例(example) v1 "例"
description "事務所の地図は、検証用のサイトの API キーで読む: AIzaSyD-ritsu-fake-key-for-tests-000000"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
```

関連: [W109](#w109)
