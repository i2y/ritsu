# 申請の種類の書き方

*`.ringi` を書くエージェントと、それを頼み、書かれたものを読むエンジニアのためのページです。*

一つの種類の申請は、一つの `.ringi` のファイルです。フォーム、経路と各ステップで答える人、期限、チケットの決まり、そして意図どおりかを確かめる主張とシナリオを書きます。いくつかの種類で共通するものは別のファイルに書き、`use` で読み込みます。ファイルの種類は、最初の語で決まります。

| 最初の行 | ファイルに書くもの | 例 |
|---|---|---|
| `request <名前> v<版> "<題名>"` | 一つの種類の申請 | `expense.ja.ringi` |
| `route <名前> v<版> "<題名>"` | いくつかの申請が通る経路 | `shared/manager_approval.ringi` |
| `organization <名前> v<版> "<題名>"` | 組織の形（役職、チームの種類、データの場所） | `shared/company.ringi` |
| `masters <名前> v<版>` | フォームで選ぶ一覧、承認者を引く一覧 | `shared/masters.ringi` |

例は、五つの部門の五種類の申請です。経費（経理）、休暇（人事）、購入（総務）、契約の審査（法務）、システムの権限（情報システム）で、`crates/ringi/examples` にあります。どれにも、名前を日本語で書いた版（`.ja.ringi`）が隣にあります。このページのコードは、どれも例のファイルの行です。

## 申請の頭

申請は、名前、版、題名から始まります。続けて、何の申請か、どの部門が受け持つか、何を読むかを書きます。

<!-- crates/ringi/examples/expense.ja.ringi -->
```ringi
request 経費(expense_ja) v1 "経費の申請"
…
owner accounting

use organization 組織(org) from "shared/company.ringi"
use masters マスタ(m) from "shared/masters.ringi"
use calendar 本社(office) from "shared/office.cal"
use book 予算(budget) from "shared/budget.book"
use gate "shared/company.gate"
use flow 事前の確かめ(precheck_agent) from "shared/precheck.flow"
use flow 出張の決裁(travel_decision) from "shared/travel_decision.flow"
use flow 支払い(pay) from "shared/pay.flow"
```

カレンダーは営業日を数え、帳簿は予算を押さえます。ゲートは会社全体の決まり（自分の申請を自分で承認しない、など）を持ち、ワークフローはエージェントの仕事と承認のあとの仕事をします。どれも ritsu のほかの言語のファイルです（[ほかの言語と組む](together.md)）。

## フォーム

フィールドには、名前、型、題名と、人が入れられる値の範囲を書きます。数と日付には `range`、文字列と行には `count` です。範囲の無いフィールドはエラーになります（E101、E102）。同じ範囲で、画面、生成するコード、経路の表の行を確かめるからです。

<!-- crates/ringi/examples/expense.ja.ringi -->
```ringi
form
  種別(category) : choice "種別"
    出張(travel)   "出張"
    経費(expense)  "経費"
    購買(purchase) "購買"
  明細(items) : rows "明細"  count >=1 <=20
    品目(what)  : text "品目"  count <=100
    金額(price) : money[円] "金額"  range >=1円 <=1_000_000円
  合計(amount)      : money[円] "合計" = sum(明細.金額)  range >=1円 <=10_000_000円
  目的(purpose)     : long text "目的"  count >=1 <=200
  使った日(used_on) : date "使った日"  range >=2026-01-01 <=2027-12-31
  行き先(trip_to)   : text "行き先"  count <=100  shown when 種別 is 出張
  領収書(receipt)   : attachment "領収書"  required when 合計 > 30_000円
  勘定科目(account) : choice from マスタ.accounts "勘定科目"  approvers only
```

| 型 | 書き方 | 画面に出るもの |
|---|---|---|
| 文字列 | `text "題名"  count <=100` | 一行の入力 |
| 長い文字列 | `long text "題名"  count >=1 <=200` | 複数行の入力 |
| 数 | `number "題名"  range >=1 <=20` | 整数 |
| 単位のある数 | `money[円]`、`rate[step 5%]`、`mass[kg]` など ritsu の単位 | 単位の付いた整数 |
| 日付 | `date "題名"  range >=2026-01-01 <=2027-12-31` | 日付 |
| 選択 | `choice "題名"` と、その下に値 | 値のどれか一つ |
| 一覧からの選択 | `choice from m.vendors "題名"` | その日に有効な一覧の行 |
| 規則の値からの選択 | `choice from authority.kind "題名"` | 規則が宣言する値のどれか一つ |
| 添付 | `attachment "題名"` | 添付するファイル |
| 人、チーム | `person "題名"`、`team "題名"` | 組織のだれか、どこかのチーム |
| はい・いいえ | `bool "題名"` | チェックボックス |
| 行 | `rows "題名"  count >=1 <=20` と、その下にフィールド | 足したり消したりできる行 |

`optional` と書かないフィールドは必須です。`required when` は条件が成り立つときだけ必須にし、`shown when` は条件が成り立つときだけ表示します。隠れたフィールドは必須にならず、送られもしません。`=` で書いたフィールドは計算で決まります（`合計` は明細の金額の合計です）。`approvers only` のフィールドは承認者が入れます。`open in 本社` は、カレンダーで営業している日だけを受け付けます。

## 経路

経路は、申請が順に通るステップです。ステップには名前と答える人を書き、条件が成り立つときだけ通るようにもできます。購入の申請（`purchase.ja.ringi`）に、ステップの種類がそろっています。

<!-- crates/ringi/examples/purchase.ja.ringi -->
```ringi
route
…
  一次(first)        上長
    when empty escalate, then skip
  技術(technical)    確認者                when 技術の確認者 is not none
…
  財務(finance)      財務部                when 権限表.finance or 前払い > 30%
  取締役会(board)    at least 60% of 取締役 when 権限表.board
    due in 10 business days at end of day
    when late expire
…
  決裁(decide)       権限表.decider
    when empty escalate, then 取締役
  回覧(inform)       circulate to 総務部
```

| ステップ | 書き方 | 済む条件 |
|---|---|---|
| 承認 | `一次(first)  上長` | 承認者のだれか一人が承認する |
| 定足数 | `at least 60% of 取締役`、`at least 2 of legal`、`all of legal`、表のセルの `部長 and 経理` | 定足数が求めるだけの人が承認する。却下で定足数に届かなくなれば、申請は却下になる |
| 回覧 | `回覧(inform)  circulate to 総務部` | すぐに済む。人に知らせ、読めるようにする |
| エージェントの答え | 承認者の選び方に `agent <ワークフロー>, else <承認者>` と書く | ワークフローが `ok` と答える。疑いや失敗のときは、`else` の後の承認者へ回る |
| 要約 | `summary  note from summarize` | ワークフローの要約ができ、後のステップの承認者がそれを読める |

ステップの答える人を規則で決めることもできます。`権限表.decider` は会社の職務権限表（rulec の規則）の出力で、その値は承認者の選び方です。

経路が二つか三つのフィールドの組み合わせで決まるときは、表に書きます。各行には、フィールドの値の組と、その組で各ステップに答える人を書きます。空いたセルは、その申請が飛ばすステップです。ringi は表を rulec の規則に下ろし、rulec はフォームが取りうるどの値もちょうど一つの行に当たることを確かめます。

<!-- crates/ringi/examples/expense.ja.ringi -->
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
```

## 答える人

`who` には、承認者の選び方を一つずつ名前を付けて書きます。どれも組織の中の関係で、ステップが始まるときに、その日に有効な組織の版で決まります。

<!-- crates/ringi/examples/expense.ja.ringi -->
```ringi
who
  上長(manager)                 = manager of requester
  部長(dept_head)               = head of requester.department
  経理(accounting)              = team accounting
  本部長(division_head)         = nearest division_head above requester
  エージェント確認(check)        = agent 事前の確かめ, else 上長
  エージェント決裁(travel_agent) = agent 出張の決裁, else 上長
  取締役(board)                 = team board
```

| 選び方 | 書き方 | 答える人 |
|---|---|---|
| 一人の人 | `counsel = person "e9001"` | その人 |
| 申請者の上長 | `manager = manager of requester` | 申請者の主なチームの長。それが申請者自身か空席なら、一つ上のチームの長 |
| 申請者の上にある、ある種類のチームの長 | `dept_head = head of requester.department` | 申請者の上にある、その種類の最初のチームの長 |
| ある役職のいちばん近い人 | `division_head = nearest division_head above requester` | 申請者の上にいる長のうち、役職がそれと同じか上の最初の人 |
| チーム | `accounting = team accounting` | チームのだれか |
| 申請者が選ぶ | `reviewer = chosen by requester in reviewer` | 申請者がそのフィールドに入れた人 |
| 前の承認者が選ぶ | `reviewer = chosen by previous approver in reviewer` | 前のステップの承認者が、承認者の入れるフィールドに入れた人 |
| 一覧から引く | `system_owner = lookup m.systems.owner by system` | フォームの値で引いた一覧の行の人 |
| エージェント | `check = agent precheck_agent, else manager` | dandori のワークフロー。疑いがあれば `else` の後の承認者 |

## 期限と、人のいないステップ

`every stage` には、各ステップの期限と催促の時刻をカレンダーの営業日で書きます。期限を過ぎたとき、答える人がいないときにどうするかも書きます。前の節の取締役会のように、ステップごとにその下に書いて変えることもできます。

<!-- crates/ringi/examples/expense.ja.ringi -->
```ringi
every stage
  due in 3 business days at end of day
  remind after 2 business days at 09:00
  when late escalate by 一段上, then expire
…
  when empty escalate, then 取締役
```

`escalate` は組織を上へたどるか、表に沿って回します（`by 一段上`）。`expire` はチケットを終わらせ、`skip` はステップを飛ばし、`stay` は待ちます。ringi は、どの催促も期限より前に来ることを確かめ、一件のチケットが長くてどれだけかかるかを示します。

## チケット

`ticket` には、申請について人ができることと、出されたとき、終わったときに申請がすることを書きます。

<!-- crates/ringi/examples/purchase.ja.ringi -->
```ringi
ticket
  self approval     not allowed
…
  same approver     approves once, at the last stage
  send back         to any earlier stage, at most 3 times
  resubmit          within 10 business days
  withdraw          until 決裁
  retract approval until the next stage answers
  approvers edit    用途
…
  on submitted      hold 予算.reserve(request: ticket.id, attempt: ticket.attempt, dept: requester.department, amount: 合計)  when requester.department is not none
  on approved       post 予算.reserve(request: ticket.id, attempt: ticket.attempt)
  on approved       launch 発注(request_id: ticket.id, vendor: 取引先, amount: 合計, summary: 用途)
  on sent back, rejected, withdrawn, expired  void 予算.reserve(request: ticket.id, attempt: ticket.attempt)
```

購入が出されると予算を仮押さえし、承認されると確定し、差し戻し、却下、取り下げ、期限切れのときは仮押さえを外します。承認のあとの発注は、ワークフローがします。だれが出せるかは `submitted by` に書きます（休暇の申請では `everyone except team board`）。`comment required for reject, send back` と書くと、却下と差し戻しをする人は理由を書く必要があります。

## 主張、例、シナリオ

主張には、フォームのどの値についても経路が満たすべきことを書きます。検査はそれを証明するか、成り立たない場合を示します。例は、経路の表に書いたとおりの答えになるべき行です。シナリオには、一件のチケットに届くイベントと、それぞれの後でチケットがどうなるかを書きます。

<!-- crates/ringi/examples/expense.ja.ringi -->
```ringi
claims
  エージェントが決裁するのは3万円まで : 決裁 is エージェント決裁 only when 合計 <= 30_000円
  二次承認を部長と経理でする場合がある : possible 二次 is 部長 and 経理
  催促は期限より前                   : every stage reminds before it is due
  期限内に終わる                     : every ticket ends within 93 business days
```

<!-- crates/ringi/examples/leave.ja.ringi -->
```ringi
scenario 理由を添えて却下する休暇
  requester e1203
  given 休暇.grant(person: "e1203", year: "2026", days: 20)
  form 種類 = 有給, 初日 = 2026-12-21, 日数 = 8, 理由 = "家族に会いに行く"
| at               | event   | by    | comment                                  | -> stage | status      |
| 2026-11-16 09:00 | submit  | e1203 |                                          | manager  | in progress |
| 2026-11-16 17:00 | approve | e1101 |                                          | head     | in progress |
| 2026-11-16 18:00 | reject  | e3101 | "年末の前の週は、一年でいちばん忙しい週です" |          | rejected    |
```

## 組織と一覧

組織のファイルには、役職を低いほうから、チームの種類を広いほうから書き、データの場所を書きます。データは版ごとの CSV のファイルを入れたディレクトリで、版が効き始める日の名前を付けます（`org/2026-04-01`、`org/2027-04-01`）。中身は、チーム、人、だれがどこに属するか、代理です。申請は、各ステップが始まる日に有効な版で承認者を決めます。

<!-- crates/ringi/examples/shared/company.ringi -->
```ringi
organization company v1 "Company"
…
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

マスタのファイルには、フォームで選ぶ一覧を書きます。一覧ごとに、キー、フィールド、行の CSV を書きます。行ごとに、いつからいつまで選べるかを書きます。

<!-- crates/ringi/examples/shared/masters.ringi -->
```ringi
master systems "Systems"
  key id
  fields
    id    : text "System"  count <=30
    name  : text "Name"  count <=100
    owner : person "Owner"
  data "../masters/systems.csv"
```

## 日本語の名前

名前はどの文字でも書けます。その隣の括弧の中に、コードと JSON が使う ASCII の名前を書きます。題名のほかの言語の版は `labels` の下に書きます。休暇の申請では、英語の名前で書いた版（`leave.ringi`）が日本語の版と同じ申請です。

<!-- crates/ringi/examples/leave.ringi -->
```ringi
request leave v1 "Leave request"
…
form
  kind : choice "Kind of leave"
    paid    "Paid leave"
    sick    "Sick leave"
    special "Special leave"
  first_day : date "First day"  range >=2026-01-01 <=2027-12-31  open in office
  days      : number "Business days"  range >=1 <=20
```

キーワードは英語のままです。書くのはエージェントで、人は `ritsu ringi doc` が英語か日本語で描くページを読みます（[申請の種類を読む](reading.md)）。
