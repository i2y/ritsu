# ringi

会社の申請と承認を書く小さな言語です。
申請の種類ごとに、フォーム、経路とステップごとに承認する人、期限と催促、終わったときに帳簿とワークフローですることを、一つのファイルに書きます。
経費、購買、休暇、会社のシステムのアカウント、契約の確認は、どれも一つのファイルで、経理、総務、人事、情報システム、法務が、同じ書き方でそれぞれの申請を書きます。
申請の種類は、会社の組織（人、チーム、役職を版ごとに持つデータ）、フォームが選ぶマスタ、カレンダー、帳簿が持つ残高、ゲート、ワークフローを読みます。ringi はそれらと突き合わせて、どの申請も経路のどれかの行に当たること、組織のどの版でも、どの申請者のどのステップにも答える人がいること、どのチケットも終わること、帳簿に仮押さえした物が、チケットがどう終わっても確定か取り消しになることを確かめます。
経路の表、条件、計算するフィールド、期限は、rulec、koyomi、kaname のファイルに下ろして、それぞれの言語に確かめさせます。その言語が見つけたことは、ringi が `.ringi` の行で言います。
出された申請一件をチケットと呼びます。チケットは、今の状態、来たイベント、その日の組織、時刻から、次の状態とすることを決める一つの関数で進みます。`ritsu gen` は、その関数を TypeScript、Python、Go、Rust で書き、`--target requests` を付ければ、サーバーが配る API、エージェントの道具、画面も書きます。
ringi はサーバーではありません。チケットの保管、時計、知らせ、画面の配信はサーバーの仕事です。
ringi は [ritsu](../../README.ja.md) の十の言語の一つです。
ritsu のサイトの **[ringi のページ](https://i2y.github.io/ritsu/ja/ringi/)** では、申請の種類の書き方、検査が保証すること、申請を管理する人がページをどう読むか、生成する物とサーバーがすること、申請が読むほかの言語のファイルを順に読めます。

## 申請の種類

[examples/expense.ja.ringi](examples/expense.ja.ringi) は、経費、出張、購買の申請です。
読むファイルを挙げ、次にフォームを書きます。

```ringi
request 経費(expense_ja) v1 "経費の申請"
…
use organization 組織(org) from "shared/company.ringi"
use masters マスタ(m) from "shared/masters.ringi"
use calendar 本社(office) from "shared/office.cal"
use book 予算(budget) from "shared/budget.book"
use gate "shared/company.gate"
use flow 事前の確かめ(precheck_agent) from "shared/precheck.flow"
…
form
  種別(category) : choice "種別"
    出張(travel)   "出張"
    経費(expense)  "経費"
    購買(purchase) "購買"
  明細(items) : rows "明細"  count >=1 <=20
    品目(what)  : text "品目"  count <=100
    金額(price) : money[円] "金額"  range >=1円 <=1_000_000円
  合計(amount)      : money[円] "合計" = sum(明細.金額)  range >=1円 <=10_000_000円
  …
  行き先(trip_to)   : text "行き先"  count <=100  shown when 種別 is 出張
  領収書(receipt)   : attachment "領収書"  required when 合計 > 30_000円
  勘定科目(account) : choice from マスタ.accounts "勘定科目"  approvers only
```

経路は表で書きます。フォームの場合ごとに一行、ステップごとに一列で、セルにはそのステップに答える人を書きます。その行の申請がステップを飛ばすなら、セルを空けます。
答える人の決め方は `who` に書きます。ステップが始まった日の組織から決まります。

```ringi
route
| 種別 | 合計                     | -> 確かめ(precheck) | 一次(first) | 二次(second)    | 決裁(decide)   |
| 出張 | <=30_000円               |                    |             |                 | エージェント決裁 |
| 出張 | >30_000円 <=100_000円    |                    |             |                 | 上長           |
| 出張 | >100_000円               |                    | 上長        |                 | 部長           |
| 経費 | <=100_000円              | エージェント確認    |             |                 | 上長           |
| 経費 | >100_000円 <=1_000_000円 | エージェント確認    | 上長        |                 | 部長           |
| 経費 | >1_000_000円             |                    | 上長        | 部長            | 本部長         |
| 購買 | <=1_000_000円            |                    | 上長        | 経理            | 部長           |
| 購買 | >1_000_000円             |                    | 上長        | 部長 and 経理   | 本部長         |

who
  上長(manager)                 = manager of requester
  部長(dept_head)               = head of requester.department
  経理(accounting)              = team accounting
  本部長(division_head)         = nearest division_head above requester
  エージェント確認(check)        = agent 事前の確かめ, else 上長
  エージェント決裁(travel_agent) = agent 出張の決裁, else 上長
  取締役(board)                 = team board
```

`エージェント確認` と `エージェント決裁` は、dandori のワークフローを通してエージェントが答えるステップです。エージェントが決めきれないときは上長が答えます。
どのステップにも、カレンダーの営業日で数える期限と催促があり、期限を過ぎたときと、答える人がいないときにどこへ回すかを書きます。
`ticket` には、だれが何を承認できるか、何回まで差し戻せるか、終わったときに予算の帳簿で何をし、どのワークフローを始めるかを書きます。

```ringi
every stage
  due in 3 business days at end of day
  remind after 2 business days at 09:00
  when late escalate by 一段上, then expire
  …
  when empty escalate, then 取締役
…
ticket
  self approval  not allowed
  …
  same approver  approves once
  send back      to the requester, at most 3 times
  resubmit       within 3 business days
  withdraw       until 決裁
  …
  on submitted   hold 予算.reserve(request: ticket.id, attempt: ticket.attempt, dept: requester.department, amount: 合計)  when requester.department is not none
  on approved    post 予算.reserve(request: ticket.id, attempt: ticket.attempt)
  on approved    launch 支払い(request_id: ticket.id, requester: requester.id, amount: 合計, purpose: 目的)
  on sent back, rejected, withdrawn, expired  void 予算.reserve(request: ticket.id, attempt: ticket.attempt)
```

同じファイルに、主張、経路の例、シナリオも書き、検査がそれを確かめます。

## 組織

組織は別のファイル [examples/shared/company.ringi](examples/shared/company.ringi) に書きます。役職、チームの種類、データを置くフォルダーを挙げます。
データは、効き始める日を名前にしたフォルダーに版ごとに置き、人、チーム、だれがどのチームにいるかを CSV で書きます。

```csv
id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e0001
sales_div,Sales Division,営業本部,division,company,e0101
sales1,Sales Department 1,営業一部,department,sales_div,e3101
sales1_a,Sales Section 1A,営業一課,section,sales1,e1101
…
```

例には 2026-04-01 からの版と 2027-04-01 からの版があります。2027-04-01 からの版では、営業一課が営業一部にまとまり、法務部長がいなくなります。
代理は、その横のもう一つのファイルに書きます。一行ごとに、本人、代理の人、代理ですること、期間、申請の種類を書きます。

```csv
person,proxy,kind,from,to,requests
e3101,e1101,approve,2026-12-28,2027-01-08,expense expense_ja purchase purchase_ja
```

## 検査が言うこと

`check` は、組織を版ごとに読み、どの申請者のどのステップにだれが答えるかを求めて、見つけたことを言います。

```console
$ ritsu ringi check examples/expense.ja.ringi --lang ja
警告[W502]: examples/expense.ja.ringi:33:84: 2027-04-01 からの組織では、ステップ `決裁` は承認者がいないので、ほかへ回るか飛ばされます
    33 | | 出張 | >100_000円               |                    | 上長        |                 | 部長           |
  = `部長` はチーム legal（法務部）に当たり、その長が空いています。
  = ステップは `when empty escalate, then 取締役` なので、上へ回ります。
  = e0102（管理本部長）へ回る申請者 2 人: e9002（弁護士 2）、e9003（弁護士 3）。
  = 版のデータを直すか、そのままでよいかを確かめてください。
  = 33, 35, 37 行目の行で、どれも同じです。
…
ok examples/expense.ja.ringi: 申請の種類、フィールド 8 個、ステップ 4 個
  2026-04-01 から使えます: 組織 `shared/company.ringi` の最初の版（データ `../org`）
  終わり無く使えます: 読むデータに終わりがありません
  長くても 93 営業日で終わります（ステップに 4 × 21、申請者のもとで 3 × 3）
  組織の形で、ステップ（`確かめ`、`一次`、`二次`、`決裁`）がほかへ回るか飛ばされる申請者: 2026-04-01 からの組織の 24 人のうち 14 人、2027-04-01 からの 23 人のうち 13 人
  組織の形で、期限を過ぎたステップ（`二次`）を上へ回せない申請者: 2026-04-01 からの組織の 24 人のうち 1 人、2027-04-01 からの 23 人のうち 1 人
  だれの申請の、どのステップがどこへ回るか: `ritsu ringi simulate examples/expense.ja.ringi --every-requester`
```

途中で省いた警告は、一部の申請者のチケットで、一人が二つのステップに答えることを言っています（W506）。あいだに別のステップがあるので、一度の承認で両方を済ませられません。
経路のどの行にも当たらない申請があればエラーで、足す行を示します。セルは、表がすでに書いている区切りで切ってあります。

```console
$ ritsu ringi check tests/mutants/E301_10万円を超える行が無い.ringi --lang ja
エラー[E301]: tests/mutants/E301_10万円を超える行が無い.ringi:6:1: 経路の表に当てはまらない申請があります
     6 | route
  = どの行にも当てはまらない申請の範囲: 金額が 100_001円 から 1_000_000円
  = 当てはまる行の無い申請: 金額 = 100_001円
  = 表に次の行を足し、`?` に、そのステップに答える承認者（`who` の名前）を書いてください（そのステップを飛ばす申請なら空のセル）: | >100_000円 | ? | ? |
  = セルは、表がすでに書いている区切りと範囲の端だけで切ってあるので、このまま足せば穴は埋まります。行の途中で承認者が変わるなら、そこで行を分けてください。
```

だれにも当たらず、そこで待ってしまうステップもエラーです。

```console
$ ritsu ringi check tests/mutants/E502_社長に上長がいない.ringi --lang ja
エラー[E502]: tests/mutants/E502_社長に上長がいない.ringi:9:15: 2026-04-01 からの組織では、ステップ `確かめ` に承認者がいません
     9 |   確かめ(check)  上長
  = 申請者の上に、`上長` に当たる長がいません。
  = 申請者 1 人: e01（社長（架空））。
  = `when empty stay` なので、チケットはこのステップで、管理する人が承認者を足すまで待ちます。
  = 版のデータを直すか、ステップの承認者の決め方か `when empty` を変えてください。
```

下ろしたファイルについて rulec、koyomi、kaname が言うことも、同じように、元になった経路の行、フィールド、期限の所で言います。
どのコードも、それを出す一番小さなファイルと一緒に [docs/codes.ja.md](docs/codes.ja.md) にあります。

## 申請者を選んで試す

`simulate` は、一人の申請者のある日の申請で、どのステップにだれが答えるかを見せます。
組織のデータでは、2026-12-28 から 2027-01-08 まで、e1101 が経費と購買の承認で e3101 の代理です。そのため 2026-12-29 には、部長に来るステップに二人のどちらも答えられます。

```console
$ ritsu ringi simulate examples/expense.ja.ringi --requester e1201 --on 2026-12-29 --lang ja
examples/expense.ja.ringi（2026-12-29、2026-04-01 からの組織）
  `--form` が無いので、経路が来うるステップを、承認者の決め方ごとに見せます。どれが来るかはフォームの値で決まります
申請者 e1201（営業担当 1）
  確かめ  エージェント確認（エージェントが迷えば上長へ）：e1101（営業一課長）
  一次    上長：e1101（営業一課長）
  二次    部長：e3101（営業一部長）［e3101 の代理で e1101（営業一課長）も］
          経理：e1001（経理部長）、e1002（経理担当）のだれか
  決裁    エージェント決裁（エージェントが迷えば上長へ）：e1101（営業一課長）
          上長：e1101（営業一課長）
          部長：e3101（営業一部長）［e3101 の代理で e1101（営業一課長）も］
          本部長：e0101（営業本部長）
```

`--form` でフォームの値を渡すと、その値が当たる行のステップだけを見せます。`--every-requester` は組織のすべての人を見せ、同じ承認者に当たる人をまとめます。

## シナリオを走らせる

シナリオは、時刻つきのイベントの表で、イベントのたびにチケットがどこにあるはずかを書きます。
`run` は、それをチケットの機械に渡し、したことを見せます。ステップは下ろした規則から、期限は下ろした日付から、残高は帳簿から決まります。

```console
$ ritsu ringi run examples/expense.ja.ringi --scenario 部長と経理で二次承認をする購買 --lang ja
2026-11-16 10:00  submit   e1201  -> 一次（e1101）、期限は 2026-11-19 の終わり; hold 予算.reserve 1_200_000 JPY
2026-11-16 15:00  approve  e1101  -> 二次（部長: e3101; 経理: e1001、e1002 のだれか）、期限は 2026-11-19 の終わり
2026-11-17 11:00  approve  e1001  -> 二次（部長: e3101）、期限は 2026-11-19 の終わり
2026-11-17 16:00  approve  e3101  -> 決裁（e0101）、期限は 2026-11-20 の終わり
2026-11-18 09:30  approve  e0101  -> approved; launch 支払い; post 予算.reserve
帳簿 `予算` の終わりの残高: dept_budget(sales1) 3_800_000 JPY、allocations -5_000_000 JPY、payees 1_200_000 JPY
  仮押さえ `reserve`（T-1、attempt-1）: posted
シナリオ `部長と経理で二次承認をする購買`: 書いたとおり（5 行）
```

コメントを添えて差し戻され、やりとりのあとで出し直す休暇です。休暇の帳簿の日数は、仮押さえされ、取り消され、また仮押さえされます。

```console
$ ritsu ringi run examples/leave.ja.ringi --scenario コメントを添えて差し戻す休暇 --lang ja
2026-11-16 09:00  submit                               e1203  -> manager（e1101）、期限は 2026-11-18 の終わり; hold 休暇.take 3 day
2026-11-16 11:00  comment                              e1101  -> manager（e1101）、期限は 2026-11-18 の終わり; コメント「休みのあいだ、だれが代わりに対応しますか」
2026-11-16 13:00  comment                              e1203  -> manager（e1101）、期限は 2026-11-18 の終わり; コメント「e1201 さんです」
2026-11-16 17:00  send back                            e1101  -> with requester、再申請は 2026-11-23 の終わりまで; コメント「代わりの人に e1201 さんを書いてください」; void 休暇.take
2026-11-16 17:30  comment                              e1203  -> with requester、再申請は 2026-11-23 の終わりまで; コメント「代わりの人は、どこに書けばよいですか」
2026-11-16 17:45  comment                              e1101  -> with requester、再申請は 2026-11-23 の終わりまで; コメント「フォームの「代わりに対応する人」の欄です」
2026-11-17 09:00  resubmit（代わり = person "e1201"）  e1203  -> manager（e1101）、期限は 2026-11-19 の終わり; コメント「代わりの人に e1201 さんを書きました」; hold 休暇.take 3 day
2026-11-17 10:00  approve                              e1101  -> approved; コメント「よい休みを」; post 休暇.take
帳簿 `休暇` の終わりの残高: leave_left(e1203) 17 day、granted -20 day、taken 3 day
  仮押さえ `take`（T-1、attempt-1）: voided
  仮押さえ `take`（T-1、attempt-2）: posted
シナリオ `コメントを添えて差し戻す休暇`: 書いたとおり（8 行）
```

理由を添えて却下される休暇です。

```console
$ ritsu ringi run examples/leave.ja.ringi --scenario 理由を添えて却下する休暇 --lang ja
2026-11-16 09:00  submit   e1203  -> manager（e1101）、期限は 2026-11-18 の終わり; hold 休暇.take 8 day
2026-11-16 17:00  approve  e1101  -> head（e3101）、期限は 2026-11-18 の終わり
2026-11-16 18:00  reject   e3101  -> rejected; コメント「年末の前の週は、一年でいちばん忙しい週です」; void 休暇.take
帳簿 `休暇` の終わりの残高: leave_left(e1203) 20 day、granted -20 day、taken 0 day
  仮押さえ `take`（T-1、attempt-1）: voided
シナリオ `理由を添えて却下する休暇`: 書いたとおり（3 行）
```

相手の案を付けるように差し戻された契約です。出し直せる日が過ぎても案が届かず、チケットは期限切れで終わります。

```console
$ ritsu ringi run examples/contract.ja.ringi --scenario 相手の案が届かない契約 --lang ja
2026-11-16 10:00  submit         e1201  -> 上長（e1101）、期限は 2026-11-19 の終わり
2026-11-16 11:00  approve        e1101  -> 要約（flow 要約）、期限は 2026-11-19 の終わり; flow 要約 に頼んだ
2026-11-16 11:05  note ready            -> 受付（e9001、e9002、e9003 のだれか）、期限は 2026-11-19 の終わり
2026-11-16 14:00  send back      e9002  -> with requester、再申請は 2026-11-30 の終わりまで; コメント「相手から届いた案を付けてください。双方が署名する本文を確かめます」
2026-12-01 00:00  resubmit late         -> expired; 期限切れ（resubmit late）
シナリオ `相手の案が届かない契約`: 書いたとおり（5 行）
```

`vectors` は、機械の遷移を一行に一つの JSON で書き出します。シナリオと、そこから作った物（議決とフォームの境目、組織のすべての人を申請者にしたもの、代理、組織の版をまたぐチケット）です。生成したコードとサーバーは、同じ物を渡されたら同じ答えを返します。

## 人が読むページ

`ritsu ringi doc` は、申請の種類を、承認する人と申請を管理する人の言葉で一つのページにします。使える日、出せる人、フォーム、場合ごとの経路、承認者の決め方、日付つきの期限、チケットの決まり、帳簿とワークフロー、主張、例、シナリオが載ります。
`--longest` は、チケットがいちばん長くかかるときの日ごとの流れと、何をすれば短くなるかを見せます。
Markdown はプルリクエストに貼るためのもので、HTML は外の物を何も読まない一つのファイルです。
例のページは [経費の申請のページ](tests/golden/doc/expense.ja.md) と [いちばん長くかかるチケット](tests/golden/doc/expense.ja.longest.md) にあります。

`ritsu ringi diff` は、申請の種類の二つの版で、言うことや答えが違う所を言います。経路が変わる場合、承認者が変わる申請者、それぞれの変更が進行中のチケットにどう効くかです（[経路の行を変えた例](tests/golden/diff/expense_row.ja.md)）。
`--versions` は、一つの申請の種類を、組織の二つの版で読み比べます。

```console
$ ritsu ringi diff --versions 2026-04-01 2027-04-01 examples/expense.ja.ringi --lang ja
  申請の種類 `経費`（`crates/ringi/examples/expense.ja.ringi`）を、2026-04-01 に効いている組織（2026-04-01 からの版）と、2027-04-01 に効いている組織（2027-04-01 からの版）で読み比べます。
承認者：
  二つの版にいる 23 人の申請者のうち、4 人の承認者が変わります。
    e1201（営業担当 1）、確かめ：エージェント確認（エージェントが迷えば上長へ）：e1101（営業一課長） → エージェント確認（エージェントが迷えば上長へ）：e3101（営業一部長）
…
  ほかに 12 件あります。すべては `--format json` で出ます。
申請者：
  - 2026-04-01 からの版にだけいる人：e9001（法務部長）
答える人がいないステップ：
  答える人が見つからない申請者の数が、二つの版で違うステップです。そのときどうなるかは、申請の種類の決まりによります。
    二次（部長）、上へ回るか、ほかへ回ります：14 → 15
    二次（部長と経理）、上へ回るか、ほかへ回ります：14 → 15
    決裁（部長）、上へ回るか、ほかへ回ります：14 → 15
進行中のチケット：
  承認者が決まったステップは、2027-04-01 のあともそのまま進みます。2027-04-01 以降に始まるステップは、2027-04-01 からの版で承認者が決まります。待っている承認者が組織からいなくなったチケットは、管理する人が引き継ぎます。期限を過ぎて上へ回るときは、そのとき効いている版で回る先が決まります。
```

プルリクエストの変更では、`ritsu impact` と `ritsu mod diff` が、ほかの言語のファイルの答えと並べて、申請について同じことを言います。

## ほかの言語に下ろす

申請の種類の中に書いた物は、それを確かめる言語のファイルに下ろします。経路の表と条件は rulec の規則に、期限と催促は koyomi の日付のファイルに、申請をチケットの機械につなぐ部分は kaname のモジュールになります。
検査は下ろしたファイルをメモリの上にだけ置きます。`extract` はそれをディスクに書き出します。中身を読むときや、中に書いた物を別のファイルに移すときに使います。

```console
$ ritsu ringi extract examples/expense.ja.ringi --out lowered --lang ja
生成しました: lowered/choices.rule
生成しました: lowered/form.rule
生成しました: lowered/next_up.rule
生成しました: lowered/route.rule
生成しました: lowered/terms.cal
生成しました: lowered/values.rule
```

英語の例を下ろしたファイルは [tests/golden/lowered/expense](tests/golden/lowered/expense) にあります。

## 言語をまたぐ確かめ

`ritsu check` は、申請の種類を、それが読むほかのファイルと突き合わせます。

- **経路とゲート**（E911、W911）：チケットのどの操作にも、申請が読むゲートの action があること。経路が選ぶ承認者に、ゲートが承認と却下を許すこと。申請者が自分の申請を決める操作を、ゲートが許すのは、経路が自己承認を許すときだけであること。
- **仮押さえと帳簿**（E209、W209）：申請を出したときの仮押さえが、確定か取り消しをする前に切れないこと。経路のいちばん長い道をカレンダーで数えて比べます。また、出せるどの申請者にも、仮押さえが読む値があること。
- **チケットとワークフロー**（E210、W210）：チケットがワークフローに渡す物が、ワークフローの入力に入ること。エージェントのワークフローが `verdict` と `reason` で答えること。申請の API を読むワークフローが、今の申請の API を読んでいること。

## 生成するコード

`ritsu gen --target typescript`（と `python`、`go`、`rust`）は、申請が読む規則、日付、帳簿、ゲート、フローのコードの隣に、申請を下ろしたファイルのコード、チケットの機械（kaname のモジュール）、申請の種類の一覧を書きます。一覧の入口 `start` と `next` は JSON を受けて JSON を返すので、一つのサーバーが、どの種類の型も持たずにすべての種類を動かせます。
`start` と `next` に約束の形でない JSON を渡すと、入口が機械を動かす前に断ります。断る文は四つの言語で同じで、どこが、何を渡され、何が要るかを言います（``関数 `start` の引数 `submit.form.days` が "three" です。整数が要ります``）。
単位の付いた ritsu の型（金額、率、量）のフィールドは、計算するフィールドも含めて生成でき、画面は単位の刻みで見せます。
`ritsu gen --target requests` は、サーバーと画面が読む物を、プロジェクトに一度だけ書きます。申請の種類、OpenAPI 3.1 のサーバーの API、エージェントに渡す道具、A2UI 1.0 のメッセージの画面と、ritsu のカタログと画面が呼ぶ関数、組織のデータのスキーマ、ベクタです。
ファイルごとの中身と、サーバーが足す物は [docs/reference.md](docs/reference.md#what-gen-writes)（英語）にあります。

## サーバー

ritsu が生成したものを動かすのは、Go のフレームワーク [ringi-server](https://github.com/i2y/ringi-server) です。
チケットを PostgreSQL に保管し、人をログインさせ、時計を鳴らし、人がブラウザで使うページ、API、エージェントのツールを出します。どの申請の種類も同じコードで動きます。
`ritsu ringi init` は、その上のプロジェクトの最初のファイルを書きます。申請の種類一つ、`ritsu gen` がそこから作るコード、サーバー、その試験です。

```sh
ritsu ringi init server --module example.com/requests --lang ja
cd server
go mod tidy
go run ./cmd/server -dev-login
```

このあと http://127.0.0.1:8080/pages/ を開き、だれとして使うかを選びます。
ページの様子、プロジェクトがサーバーに渡すもの、人に使ってもらうときの動かし方は、[ringi-server の README](https://github.com/i2y/ringi-server/blob/main/README.ja.md) にあります。

## AI エージェント向けのスキル

[skills/ringi](../../skills/ringi/SKILL.md) は、申請の種類、組織、マスタを書いたり変えたりするエージェントのためのスキルです。書く流れ、申請を管理する人に聞くこと、ringi の診断と ritsu の言語をまたぐ診断の直し方があります。
`SKILL.md` のほかのページは `docs/` からコピーしたもので、`skills/sync.sh` がコピーします。

## インストール

ringi は ritsu と一緒に入ります。

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

`ritsu ringi …` で走らせます。
`ringi` という名前のリンクから呼ぶと、ritsu は ringi として動き、申請を下ろす先の言語と、申請が読む言語がつながります。リリースのアーカイブにはリンクが入っています。手で作るなら、ritsu の隣で `ln -s ritsu ringi` とします。
ringi のクレートのバイナリはほかの言語を持たないので、rulec、koyomi、kaname に下ろす申請の種類では E008 で止まります。

## コマンド

```console
$ ritsu ringi check examples --lang ja
$ ritsu ringi check examples/expense.ja.ringi --at 2027-04-01 --lang ja
$ ritsu ringi simulate examples/leave.ja.ringi --every-requester --on 2026-11-16 --lang ja
$ ritsu ringi run examples/leave.ja.ringi --lang ja
$ ritsu ringi vectors examples/leave.ja.ringi
$ ritsu ringi api examples/leave.ja.ringi
$ ritsu ringi doc examples/expense.ja.ringi --format html --lang ja
$ ritsu ringi diff --versions 2026-04-01 2027-04-01 examples/leave.ja.ringi --lang ja
$ ritsu ringi extract examples/leave.ja.ringi --out lowered
$ ritsu ringi init server --module example.com/requests --lang ja
$ ritsu ringi explain E301 --lang ja
```

どのコマンドも `--lang ja|en`（無ければ `RINGI_LANG`、次に `RITSU_LANG`、どちらも無ければ英語）と `--help` を受け付けます。
`check` は、`--at <日付>` でその日に効いている組織の版だけを、`--through <日付>` でその日まで使えない申請の種類をエラーにし、`--org <ディレクトリ>` で `data` の行と別のデータを読みます。
終了コードは、エラーが無ければ 0、エラーがあれば 1（`diff` では違いがあれば 1）、引数の誤り、読めないファイル、つながっていない言語のファイルなら 2 です。
コマンドとフラグの全部は [docs/reference.md](docs/reference.md#commands)（英語）にあります。

## 例

どの例も英語の版があり、その隣に日本語の名前の版（`<名前>.ja.ringi`）があります。[examples/shared](examples/shared) の組織、マスタ、カレンダー、帳簿、ゲート、フローと、[examples/org](examples/org) の組織のデータ、[examples/masters](examples/masters) のマスタの行を読みます。

- [examples/expense.ringi](examples/expense.ringi)：経費、出張、購買の申請です。種別と金額で決まる役の人が確かめ、承認し、決裁します。100 万円までの経費はエージェントが確かめ、3 万円までの出張はエージェントが決裁します。出したときに予算を仮押さえし、承認されたらワークフローが支払います。
- [examples/purchase.ringi](examples/purchase.ringi)：購買の稟議です。会社の決裁権限表が種類と金額で定める人が審査と決裁をし、承認されたらワークフローが発注します。
- [examples/leave.ringi](examples/leave.ringi)：有給休暇、病気休暇、特別休暇の申請です。上長が承認し、5 営業日を超えるときは部長も承認します。有給休暇は、休暇の帳簿の残りから日数を引きます。
- [examples/account.ringi](examples/account.ringi)：会社のシステムを使うためのアカウントの申請です。上長と、システムの一覧が示す持ち主が承認し、管理者の権限なら情報セキュリティの担当も承認します。承認されたら情報システムの運用担当に回覧し、ワークフローがアカウントを作ります。
- [examples/contract.ringi](examples/contract.ringi)：法務が確かめる契約です。エージェントが確かめる人のために要約し、法務の一人が受け付けて担当を決めます。500 万円を超える契約は法務の二人が確かめ、1,000 万円を超える契約は法務部長が決めます。

## どう確かめているか

`cargo test -p ringi` が、言語を例と試験の申請に当てます。

- 例の五つの申請の種類（英語の版と日本語の名前の版）と、共有の組織、マスタ、経路が検査を通り、二つの版が同じ所で同じことを言うことを確かめます。組織の二つの版で数えると、言うのは版のデータから来ることだけです。
- どの診断のコードにも、それを出す一番小さなファイルが英語と日本語の両方であります。注が示す直し方を書き入れると、検査を通ります。
- チケットの機械が、例のシナリオと、機械のために書いた 25 のファイルのシナリオを、英語と日本語で、書いたとおりに進めます。どのシナリオのフォームでも、二つの版のすべての人を申請者にしてチケットを最後まで進め、`simulate` と検査の言うとおりに動き、検査の数えた営業日を越えないことを確かめます。
- TypeScript、Python、Go、Rust に `ritsu gen` が書いたコードが、例と機械の申請のベクタ 10,100 件に機械と同じ答えを返し（ritsu の `tests/gen_requests.rs`）、`tsc --strict`、`mypy --strict`、`go vet`、`cargo clippy -- -D warnings` を通ります。
- 画面を A2UI 1.0 と ritsu のカタログで検証し、画面が呼ぶ関数が、途中のフォームのベクタに Node の下で機械と同じ答えを返すことを確かめます。
- `doc` と `diff` のページは golden です。例ごとの下ろしたファイルも golden で、それぞれの言語の検査を通ります。
- 言語をまたぐ確かめは、成り立つ形、成り立たない形、決められない形を、英語と日本語の golden で持ちます（ritsu の `tests/cross.rs`）。
- この README と `docs/` とスキルに載せた出力、診断、`.ringi` と CSV の行は本物です。`tests/docs.rs` が突き合わせ、コマンドは ritsu の `tests/ringi.rs` が走らせます。

## 状態

言語、`check`、`simulate`、`run`、`vectors`、`api`、`doc`、`diff`、`extract`、`init`、`explain`、言語をまたぐ確かめ、`ritsu gen` が書くコードはできています（[DESIGN.md](DESIGN.md)）。
まだ作っていないもの：全員が読むまで待つ回覧、日や時刻を選んだ催促と繰り返す催促、いくつものフィールドに一つの条件、マスタから引くチーム、申請者を助けるエージェント、開いたチケットの終わりを待つワークフロー、コメントの添付（全部は [docs/reference.md](docs/reference.md#what-the-language-does-not-have) にあります）。
サーバーは ringi の外にあり、`--target requests` が書く約束に従って、チケットを保管し、時計を鳴らし、人に知らせ、画面を配り、ログインを扱います。

## ライセンス

[Apache License, Version 2.0](../../LICENSE-APACHE) と [MIT license](../../LICENSE-MIT) のどちらかを選んで使えます。
