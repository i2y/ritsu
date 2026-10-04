# 診断のコード

`ritsu explain --all --format markdown --lang ja` の出力です。手で直しません。

<a id="e101"></a>

## E101 — .proto として読めないファイル

**いつ出るか**: プロジェクトの `.proto` を、ritsu の共通のパーサー（ritsu-proto）が読めないとき。閉じていない `{`、`;` の無い文、知らない `syntax`、proto2 の `group`、UTF-8 でないファイルなどです。どの言語もこのパーサーで `.proto` を読むので、このファイルはどの言語からも読めません。そのファイルを読む言語も、読むところで自分の診断のコードを出します（rulec の E013、dandori の E016、sakai の E106、yuen の E205）。

**直し方**: 示された位置を直し、proto3 の `.proto` にしてください。`buf build` でビルドできるファイルなら、ritsu のパーサーも読めます。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`shop.proto`:

```proto
syntax = "proto3";

package shop.v1;

message Order {
  string id = 1;
```

<a id="e201"></a>

## E201 — 規則を呼ぶところで、前提を破る値を渡すことがあります

**いつ出るか**: ワークフローが規則を呼ぶところで、dandori が知っている値の範囲の中に、規則の前提（入力どうしの関係 `constraint`）を破る組み合わせがあるとき（X2、DESIGN 7.4）。範囲は dandori が値を入れるすべての場所から集めたもので、dandori の E014 と同じ読み方です。範囲を `range from koyomi` にした日付の入力では、渡す値が koyomi の日付の日なら、その日のどれかが規則の日でないときも、このエラーです（X3 の (a)）。前提を破る呼び出しは、規則から生成したコードが入口で断るので、ワークフローを走らせたときに初めて落ちます。注には、値の範囲と、前提を破る組み合わせや日が出ます。

**直し方**: 呼ぶ前に前提が保たれるよう分岐するか、値の範囲を狭めてください（入力やタスクの結果の `range`）。前提のほうが業務に合っていないなら、規則の `constraint` を直してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`refund_check.rule`:

```rule
rule refund_check v1
description "Whether a refund is paid at once or reviewed. A refund never asks for more than was paid, which the rule takes for granted"

enum path = at_once | review

inputs
  paid  : number  range >=0 <=10000
  asked : number  range >=0 <=10000

constraint asked <= paid

outputs
  route : path

table pick
policy unique
| asked | -> route : path |
| <=100 | at_once         |
| >100  | review          |
```

`refund.flow`:

```flow
workflow refund v1
description "Pays a refund back at once or sends it to review, as the rule decides"

use rule check from "refund_check.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:refund-check"

inputs
  order : string
  paid  : int  range >=0 <=10000

task ask_amount(order: string) -> int range >=0 <=10000
  lambda "arn:aws:lambda:us-east-1:123456789012:function:ask-amount"
  idempotent

task pay_back(order: string, amount: int range >=0 <=10000)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:pay-back"
  key

flow
  let asked = ask_amount(order: order)
  let decision = check(paid: paid, asked: asked)
  match decision.route
    at_once => pay_back(order: order, amount: asked)
    review => pass
```

関連: [W201](#w201)

<a id="w201"></a>

## W201 — 規則を呼ぶところで、前提が保たれるかを決められません

**いつ出るか**: ワークフローが規則を呼ぶところで、前提が保たれるかを決められないとき（X2）。渡す値に範囲の無いところから来るものがある（タスクの結果に `range` が無い、など）、前提が並びの合計や長さの上限である（dandori は並びの長さを知りません）、前提が koyomi の日付の日で、渡す値が何日かを言わないところ（ワークフローの入力、タスクの結果、`now`）からも来る、のどれかです。決められない前提は、`ritsu dandori build` が書くワークフローのコードが実行時に確かめます。値ができたところですぐに確かめ、前提を破る実行を `Dandori.BrokenPrecondition` で失敗させます（dandori の DESIGN 1.17）。

**直し方**: 値の来るところに範囲を書いてください（タスクの結果やワークフローの入力の `range`）。範囲を書けないなら、このままで構いません。ワークフローのコードが実行時に確かめます。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`refund_check.rule`:

```rule
rule refund_check v1
description "Whether a refund is paid at once or reviewed. A refund never asks for more than was paid, which the rule takes for granted"

enum path = at_once | review

inputs
  paid  : number  range >=0 <=10000
  asked : number  range >=0 <=10000

constraint asked <= paid

outputs
  route : path

table pick
policy unique
| asked | -> route : path |
| <=100 | at_once         |
| >100  | review          |
```

`refund.flow`:

```flow
workflow refund v1
description "Pays a refund back at once or sends it to review, as the rule decides"

use rule check from "refund_check.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:refund-check"

inputs
  order : string
  paid  : int  range >=0 <=10000

task ask_amount(order: string) -> int
  lambda "arn:aws:lambda:us-east-1:123456789012:function:ask-amount"
  idempotent

task pay_back(order: string, amount: int range >=0 <=10000)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:pay-back"
  key

flow
  let asked = ask_amount(order: order)
  let decision = check(paid: paid, asked: asked)
  match decision.route
    at_once => pay_back(order: order, amount: asked)
    review => pass
```

関連: [E201](#e201)

<a id="e202"></a>

## E202 — koyomi の日付がとる日が、規則の入力の範囲を外れます

**いつ出るか**: ワークフローが koyomi の日付の日（`due.day`）を規則の日付の入力に渡すとき、koyomi がその日付について数えた日のどれかが、規則が宣言した入力の範囲の外にあるとき（X3 の (a)、DESIGN 7.5）。koyomi は入力の範囲のすべてで日付を計算するので、外れる日は例として一つに決まります。注には、その日と、koyomi がその日を返す入力が出ます。範囲を `range from koyomi` にした入力では、日は規則の前提なので、E201 と W201 が確かめます。

**直し方**: 規則の入力の範囲を広げるか、規則の範囲を `range from koyomi` にして、koyomi の日をそのまま範囲にしてください（rulec の §15.174）。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`payment_terms.cal`:

```cal
dates payment_terms v1
description "Closes on the 20th and pays on the 10th of the next month. No calendar, so the day it pays on is always the 10th"

inputs
  received : date  range >=2026-01-01 <=2026-12-20

date closing = received
  close day 20          # closes on the 20th

date payment = closing
  day 10 of month +1    # pays on the 10th of the next month
```

`batch.rule`:

```rule
rule batch v1
description "The billing batch a payment day falls in"

enum run = spring | autumn

inputs
  pay_day : date  range >=2026-03-01 <=2027-01-31

outputs
  batch : run

table pick
policy unique
| pay_day      | -> batch : run |
| <=2026-08-31 | spring         |
| >=2026-09-01 | autumn         |
```

`billing.flow`:

```flow
workflow billing v1
description "Bills an order in the batch the rule picks for the day its payment is due"

use dates terms from "payment_terms.cal"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:payment-terms"
use rule batch from "batch.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:batch"

inputs
  order    : string
  received : date

task bill(order: string, due: date, run: batch.run)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:bill"
  key

flow
  let due = terms.payment(received: received)
  let pick = batch(pay_day: due.day)
  bill(order: order, due: due.day, run: pick.batch)
```

関連: [W202](#w202), [E205](#e205)

<a id="w202"></a>

## W202 — koyomi の日付がとる日が、規則の入力の範囲に収まるかを決められません

**いつ出るか**: 規則の日付の入力に渡す値が、koyomi の日付の日のほかに、何日かを言わないところ（ワークフローの入力、タスクの結果、`now`）からも来ることがあるとき、または koyomi がその日付の日を数えないとき（入力の組み合わせが確かめる数を超える、途中で計算が止まる入力がある）（X3 の (a)）。規則から生成したコードが、ワークフローを走らせたときに入口で日を確かめます。

**直し方**: 値を koyomi の日付の日だけから渡せば、決められるようになります。koyomi が日を数えないなら、koyomi のファイルの入力の範囲を狭めてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`payment_terms.cal`:

```cal
dates payment_terms v1
description "Closes on the 20th and pays on the 10th of the next month. No calendar, so the day it pays on is always the 10th"

inputs
  received : date  range >=2026-01-01 <=2026-12-20

date closing = received
  close day 20          # closes on the 20th

date payment = closing
  day 10 of month +1    # pays on the 10th of the next month
```

`batch.rule`:

```rule
rule batch v1
description "The billing batch a payment day falls in"

enum run = spring | autumn

inputs
  pay_day : date  range >=2026-02-01 <=2027-01-31

outputs
  batch : run

table pick
policy unique
| pay_day      | -> batch : run |
| <=2026-08-31 | spring         |
| >=2026-09-01 | autumn         |
```

`billing.flow`:

```flow
workflow billing v1
description "Bills an order in the batch the rule picks for the day its payment is due"

use dates terms from "payment_terms.cal"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:payment-terms"
use rule batch from "batch.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:batch"

inputs
  order    : string
  received : date
  at_once  : bool

task bill(order: string, due: date, run: batch.run)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:bill"
  key

flow
  let due = terms.payment(received: received)
  match at_once
    true => let day = received
    false => let day = due.day
  let pick = batch(pay_day: day)
  bill(order: order, due: day, run: pick.batch)
```

関連: [E202](#e202)

<a id="e203"></a>

## E203 — 規則の出力が、chobo の受け取らない額になることがあります

**いつ出るか**: ワークフローが規則の数の出力を chobo の振替の額に渡すとき、その出力が負か 2⁶³ − 1 を超えることがあるとき（X4、DESIGN 7.6。chobo は 0 から 2⁶³ − 1 までを受け取ります）。出力の値は rulec が求めます（表の行に書いた数か、区間の計算から）。chobo は範囲の外の額を、断る（業務の結果）のではなく呼び出しの失敗にします。注には、その額になる規則の入力の例（規則のベクタから取ったもの）が出ます。

**直し方**: 規則が返す額を 0 以上にするか（返金などの負の額は、向きの違う振替に分けてください）、振替に渡す前に分岐してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`seats.rule`:

```rule
rule seats v1
description "How many seats an event of a kind needs; a handback gives seats back"

enum kind = handback | workshop | talk

inputs
  event_kind : kind

outputs
  needed : number  round down(1)

table pick
policy unique
| event_kind | -> needed : number |
| handback   | -20                |
| workshop   | 30                 |
| talk       | 80                 |
```

`hall.book`:

```book
book hall v1
description "The seats of the hall. An event is given the seats it needs at once, and the hall holds 300"

unit seat

account given(event: string) : seat
  description "the seats an event is given"
  at least 0 refused as not_given
  at most 300 refused as over_capacity
account venue : seat outside

transfer assign(event: string, count: seat)
  key event
  move count from venue to given(event)

transfer release(event: string, count: seat)
  key event
  move count from given(event) to venue
```

`booking.flow`:

```flow
workflow booking v1
description "Gives an event the seats its kind needs, all at once"

use rule seats from "seats.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:seats"
use book hall from "hall.book"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:hall"

inputs
  event : string
  kind  : seats.kind

task give_seats(event: string, count: int)
  book hall.assign.do

flow
  let need = seats(event_kind: kind)
  give_seats(event: event, count: need.needed)
```

関連: [W203](#w203), [E204](#e204)

<a id="w203"></a>

## W203 — 規則の出力を、chobo が額として受け取るかを決められません

**いつ出るか**: ワークフローが規則の数の出力を振替の額に渡すとき、出力の範囲に上限か下限が無いか、値が範囲の分からないところ（範囲の無いタスクの結果など）からも来ることがあるとき（X4）。chobo が受け取らない額なら、ワークフローを走らせたときに呼び出しが失敗します。

**直し方**: 値の来るところに範囲を書いてください（タスクの結果やワークフローの入力の `range`）。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`seats.rule`:

```rule
rule seats v1
description "How many seats an event of a kind needs"

enum kind = workshop | talk

inputs
  event_kind : kind

outputs
  needed : number  round down(1)

table pick
policy unique
| event_kind | -> needed : number |
| workshop   | 30                 |
| talk       | 80                 |
```

`hall.book`:

```book
book hall v1
description "The seats of the hall. An event is given the seats it needs at once, and the hall holds 300"

unit seat

account given(event: string) : seat
  description "the seats an event is given"
  at least 0 refused as not_given
  at most 300 refused as over_capacity
account venue : seat outside

transfer assign(event: string, count: seat)
  key event
  move count from venue to given(event)

transfer release(event: string, count: seat)
  key event
  move count from given(event) to venue
```

`booking.flow`:

```flow
workflow booking v1
description "Gives an event the seats its kind needs, all at once"

use rule seats from "seats.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:seats"
use book hall from "hall.book"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:hall"

inputs
  event : string
  kind  : seats.kind

task give_seats(event: string, count: int)
  book hall.assign.do

task ask_organiser(event: string) -> int
  lambda "arn:aws:lambda:us-east-1:123456789012:function:ask-organiser"
  idempotent

flow
  let need = seats(event_kind: kind)
  let count = need.needed
  match kind
    workshop => let count = ask_organiser(event: event)
    talk => pass
  give_seats(event: event, count: count)
```

関連: [E203](#e203)

<a id="e204"></a>

## E204 — 振替が断られうる理由を、タスクが処理していません

**いつ出るか**: 規則の出力を額に渡す `do` か `hold` の呼び出しで、操作が断られうる理由を、タスクが宣言したエラーとして処理していないとき（X4）。断られうる理由とは、額を呼び出しが渡す範囲に限った chobo の探索で、断られる例が見つかった理由です。比べるのは帳簿の境界の理由（勘定の `refused as`）、つまり額で決まる断りだけです。同じキーを別の引数で使い直すことのように、前の呼び出しで決まる断りは比べません。探索は chobo の検査と同じ深さまでたどります。`post` と `void` は仮押さえの状態で断られ、dandori が案件の状態ごとに確かめます（dandori の E022）。

**直し方**: その理由をタスクのエラーとして宣言し、処理してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`seats.rule`:

```rule
rule seats v1
description "How many seats an event of a kind needs"

enum kind = workshop | talk | concert

inputs
  event_kind : kind

outputs
  needed : number  round down(1)

table pick
policy unique
| event_kind | -> needed : number |
| workshop   | 30                 |
| talk       | 80                 |
| concert    | 400                |
```

`hall.book`:

```book
book hall v1
description "The seats of the hall. An event is given the seats it needs at once, and the hall holds 300"

unit seat

account given(event: string) : seat
  description "the seats an event is given"
  at least 0 refused as not_given
  at most 300 refused as over_capacity
account venue : seat outside

transfer assign(event: string, count: seat)
  key event
  move count from venue to given(event)

transfer release(event: string, count: seat)
  key event
  move count from given(event) to venue
```

`booking.flow`:

```flow
workflow booking v1
description "Gives an event the seats its kind needs, all at once"

use rule seats from "seats.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:seats"
use book hall from "hall.book"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:hall"

inputs
  event : string
  kind  : seats.kind

task give_seats(event: string, count: int)
  book hall.assign.do

flow
  let need = seats(event_kind: kind)
  give_seats(event: event, count: need.needed)
```

関連: [E203](#e203), [W204](#w204)

<a id="w204"></a>

## W204 — 振替がどの理由で断られうるかを決められません

**いつ出るか**: タスクが帳簿の境界の理由を処理しているのに、額を呼び出しが渡す範囲に限った chobo の探索で、その理由で断られる例が見つからないとき（X4）。探索は chobo の検査と同じ深さまでしかたどらないので、起きないと言えるのはその深さまでです。額の範囲が分からないときや、chobo から帳簿の情報を得られないときも、この警告で「決められない」と知らせます。

**直し方**: 起きない理由なら、タスクのエラーから外してください。深い手順でしか起きないなら、そのままで構いません。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`seats.rule`:

```rule
rule seats v1
description "How many seats an event of a kind needs"

enum kind = workshop | talk

inputs
  event_kind : kind

outputs
  needed : number  round down(1)

table pick
policy unique
| event_kind | -> needed : number |
| workshop   | 30                 |
| talk       | 80                 |
```

`hall.book`:

```book
book hall v1
description "The seats of the hall. An event is given the seats it needs at once, and the hall holds 300"

unit seat

account given(event: string) : seat
  description "the seats an event is given"
  at least 0 refused as not_given
  at most 300 refused as over_capacity
account venue : seat outside

transfer assign(event: string, count: seat)
  key event
  move count from venue to given(event)

transfer release(event: string, count: seat)
  key event
  move count from given(event) to venue
```

`booking.flow`:

```flow
workflow booking v1
description "Gives an event the seats its kind needs, all at once"

use rule seats from "seats.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:seats"
use book hall from "hall.book"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:hall"

inputs
  event : string
  kind  : seats.kind

task give_seats(event: string, count: int)
  book hall.assign.do
  errors over_capacity

flow
  let need = seats(event_kind: kind)
  give_seats(event: event, count: need.needed)
    on over_capacity => fail OverCapacity "the hall cannot seat the event"
```

関連: [E204](#e204)

<a id="e205"></a>

## E205 — koyomi の日付に渡す日が、入力の範囲を外れます

**いつ出るか**: ワークフローが koyomi の日付を呼ぶとき、日付の入力に渡す日が、koyomi の入力の範囲を外れることがあるとき（X6、DESIGN 7.8）。渡す日がほかの koyomi の日付の日なら、koyomi が数えたその日の全部で確かめます。範囲の中の日なら、カレンダーに問い合わせる日がデータの範囲に収まることを、koyomi の検査が確かめています（koyomi の E203）。注には、外れる日と、koyomi がその日を返す入力が出ます。

**直し方**: koyomi の入力の範囲を広げるか（カレンダーのデータも足してください）、範囲に収まる日を渡してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`payment_terms.cal`:

```cal
dates payment_terms v1
description "Closes on the 20th and pays on the 10th of the next month. No calendar, so the day it pays on is always the 10th"

inputs
  received : date  range >=2026-01-01 <=2026-12-20

date closing = received
  close day 20          # closes on the 20th

date payment = closing
  day 10 of month +1    # pays on the 10th of the next month
```

`reminders.cal`:

```cal
dates reminders v1
description "A reminder a week before a payment is due"

inputs
  due : date  range >=2026-02-01 <=2026-12-31

date reminder = due
  - 7 days              # a week before
```

`reminding.flow`:

```flow
workflow reminding v1
description "Reminds a customer a week before the payment of an order is due"

use dates terms from "payment_terms.cal"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:payment-terms"
use dates reminders from "reminders.cal"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:reminders"

inputs
  order    : string
  received : date

task remind(order: string, on: date)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:remind"
  key

flow
  let due = terms.payment(received: received)
  let note = reminders.reminder(due: due.day)
  remind(order: order, on: note.day)
```

関連: [W205](#w205), [E202](#e202)

<a id="w205"></a>

## W205 — koyomi の日付に渡す日が、範囲に収まるかを決められません

**いつ出るか**: ワークフローが koyomi の日付の入力に渡す日を、dandori が知らないとき（X6）。ワークフローの入力、タスクの結果、`now` から来る日は、何日かを言いません。dandori には、日付の範囲を書く書き方がまだありません。範囲の外の日は、ワークフローを走らせたときに、koyomi が生成したコードが断ります。

**直し方**: ほかの koyomi の日付の日を渡せば、決められるようになります。そうでなければ、このままで構いません。範囲の外の日は、koyomi が実行時に断ります。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`payment_terms.cal`:

```cal
dates payment_terms v1
description "Closes on the 20th and pays on the 10th of the next month. No calendar, so the day it pays on is always the 10th"

inputs
  received : date  range >=2026-01-01 <=2026-12-20

date closing = received
  close day 20          # closes on the 20th

date payment = closing
  day 10 of month +1    # pays on the 10th of the next month
```

`reminders.cal`:

```cal
dates reminders v1
description "A reminder a week before a payment is due"

inputs
  due : date  range >=2026-02-01 <=2027-01-31

date reminder = due
  - 7 days              # a week before
```

`reminding.flow`:

```flow
workflow reminding v1
description "Reminds a customer a week before the payment of an order is due"

use dates terms from "payment_terms.cal"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:payment-terms"
use dates reminders from "reminders.cal"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:reminders"

inputs
  order    : string
  received : date

task remind(order: string, on: date)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:remind"
  key

flow
  let due = terms.payment(received: received)
  let note = reminders.reminder(due: due.day)
  remind(order: order, on: note.day)
```

関連: [E205](#e205)

<a id="e206"></a>

## E206 — 仮押さえの期限が、確定や取消のときには必ず切れています

**いつ出るか**: ワークフローが chobo の仮押さえを案件として追い（`case … follows <帳簿>.<振替>`）、確定か取消をするところで、仮押さえを作ってからその呼び出しまでの長さの下限が、有効期限（振替の `pending expires after`）以上のとき（X5、DESIGN 7.7）。帳簿はどの実行でもその呼び出しを `expired` で断り、通ったあとの流れは動きません。長さは dandori がフローの文から数えます。決まった長さの `wait` はその長さです。koyomi の日付の時刻までの `wait until` は、日付の入力に仮押さえのあとで読んだ `now` を渡したものなら、koyomi が数えた入力から日付までの日数の最小と最大と、日付の時刻から出ます。注には、下限と、その下限になる文が出ます。

**直し方**: 仮押さえの期限を延ばすか（振替の `pending expires after`）、もっと早く呼んでください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`weekdays.cal`:

```cal
calendar weekdays v1
description "A business that keeps its days in UTC, open Monday to Friday"
offset +00:00

closed weekly sat, sun
```

`payment_terms.cal`:

```cal
dates payment_terms v1
description "Closes on the 20th and pays on the 10th of the next month at 09:00, or on the business day before when that day is closed"
use calendar "weekdays.cal"

inputs
  received : date  range >=2026-01-01 <=2026-12-20

date closing = received
  close day 20          # closes on the 20th

date payment = closing
  day 10 of month +1    # pays on the 10th of the next month
  roll preceding        # or on the business day before
  at 09:00
```

`stock.book`:

```book
book stock v1
description "Stock per SKU. An order holds what it takes for 14 days at most; shipping posts the hold"

unit pcs

account shelf(sku: string) : pcs
  description "what is on the shelves"
  at least 0 refused as out_of_stock
account suppliers : pcs outside
account customers : pcs outside

transfer receive(delivery: string, sku: string, qty: pcs)
  key delivery, sku
  move qty from suppliers to shelf(sku)

transfer reserve(order: string, sku: string, qty: pcs)
  description "posted when the order ships"
  key order, sku
  pending expires after 14 days
  move qty from shelf(sku) to customers
```

`invoice.flow`:

```flow
workflow invoice v1
description "Holds an order's goods until its payment is due, then ships them"

use dates terms from "payment_terms.cal"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:payment-terms"
use book stock from "stock.book"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:stock"

inputs
  order : string
  sku   : string
  qty   : int  range >=1 <=100

task reserve(order: string, sku: string, qty: int) -> stock.reserve
  book stock.reserve.hold
  starts stock.reserve
  errors out_of_stock

task ship(order: string, sku: string) -> stock.reserve
  book stock.reserve.post
  sends post
  errors expired

case goods : stock.reserve follows stock.reserve

flow
  goods <- reserve(order: order, sku: sku, qty: qty)
    on out_of_stock => fail OutOfStock "nothing left on the shelf"
  let due = terms.payment(received: now)
  wait until due.at
  goods <- ship(order: order, sku: sku)
    on expired => fail Expired "the hold expired before the payment was due"
```

関連: [W206](#w206)

<a id="w206"></a>

## W206 — 仮押さえの期限が、確定や取消のときに切れているかを決められません

**いつ出るか**: 仮押さえを作ってから確定か取消までの長さが、有効期限の前にも後にもなりうるとき、または長さの上限が分からないとき（X5）。上限は、仮押さえを作るタスクと、あいだのタスクの `timeout` とリトライの回数から出ます。`timeout` の無いタスク、規則や日付の呼び出し（`.flow` に時間の上限がありません）、上限の分からない `wait until` があれば、上限はありません。期限が切れていれば帳簿が `expired` で断り、フローはそれを処理しています（dandori の E022）。期限が切れないと示せたとき（上限が有効期限より短いとき）は、何も出しません。

**直し方**: 仮押さえを作るタスクとあいだのタスクに `timeout` を書くと、上限が決まります。書けないなら、このままで構いません。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`weekdays.cal`:

```cal
calendar weekdays v1
description "A business that keeps its days in UTC, open Monday to Friday"
offset +00:00

closed weekly sat, sun
```

`payment_terms.cal`:

```cal
dates payment_terms v1
description "Closes on the 20th and pays on the 10th of the next month at 09:00, or on the business day before when that day is closed"
use calendar "weekdays.cal"

inputs
  received : date  range >=2026-01-01 <=2026-12-20

date closing = received
  close day 20          # closes on the 20th

date payment = closing
  day 10 of month +1    # pays on the 10th of the next month
  roll preceding        # or on the business day before
  at 09:00
```

`stock.book`:

```book
book stock v1
description "Stock per SKU. An order holds what it takes for 60 days at most; shipping posts the hold"

unit pcs

account shelf(sku: string) : pcs
  description "what is on the shelves"
  at least 0 refused as out_of_stock
account suppliers : pcs outside
account customers : pcs outside

transfer receive(delivery: string, sku: string, qty: pcs)
  key delivery, sku
  move qty from suppliers to shelf(sku)

transfer reserve(order: string, sku: string, qty: pcs)
  description "posted when the order ships"
  key order, sku
  pending expires after 60 days
  move qty from shelf(sku) to customers
```

`invoice.flow`:

```flow
workflow invoice v1
description "Holds an order's goods until its payment is due, then ships them"

use dates terms from "payment_terms.cal"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:payment-terms"
use book stock from "stock.book"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:stock"

inputs
  order : string
  sku   : string
  qty   : int  range >=1 <=100

task reserve(order: string, sku: string, qty: int) -> stock.reserve
  book stock.reserve.hold
  starts stock.reserve
  errors out_of_stock

task ship(order: string, sku: string) -> stock.reserve
  book stock.reserve.post
  sends post
  errors expired

case goods : stock.reserve follows stock.reserve

flow
  goods <- reserve(order: order, sku: sku, qty: qty)
    on out_of_stock => fail OutOfStock "nothing left on the shelf"
  let due = terms.payment(received: now)
  wait until due.at
  goods <- ship(order: order, sku: sku)
    on expired => fail Expired "the hold expired before the payment was due"
```

関連: [E206](#e206)
