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

**いつ出るか**: ワークフローが規則を呼ぶところで、dandori が知っている値の範囲の中に、規則の前提（入力どうしの関係 `constraint`）を破る組み合わせがあるとき。範囲は dandori が値を入れるすべての場所から集めたもので、dandori の E014 と同じ読み方です。範囲を `range from koyomi` にした日付の入力では、渡す値が koyomi の日付の日なら、その日のどれかが規則の日でないときも、このエラーです。前提を破る呼び出しは、規則から生成したコードが入口で受け付けないので、ワークフローを走らせたときに初めて落ちます。注には、値の範囲と、前提を破る組み合わせや日が出ます。

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

**いつ出るか**: ワークフローが規則を呼ぶところで、前提が保たれるかを決められないとき。渡す値に範囲の無いところから来るものがある（タスクの結果に `range` が無い、など）、前提が並びの合計や長さの上限である（dandori は並びの長さを知りません）、前提が koyomi の日付の日で、渡す値が何日かを言わないところ（ワークフローの入力、タスクの結果、`now`）からも来る、のどれかです。決められない前提は、`ritsu dandori build` が書くワークフローのコードが実行時に確かめます。値ができたところですぐに確かめ、前提を破る実行を `Dandori.BrokenPrecondition` で失敗させます。

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

**いつ出るか**: ワークフローが koyomi の日付の日（`due.day`）を規則の日付の入力に渡すとき、koyomi がその日付について数えた日のどれかが、規則が宣言した入力の範囲の外にあるとき。koyomi は入力の範囲のすべてで日付を計算するので、外れる日は例として一つに決まります。注には、その日と、koyomi がその日を返す入力が出ます。範囲を `range from koyomi` にした入力では、日は規則の前提なので、E201 と W201 が確かめます。

**直し方**: 規則の入力の範囲を広げるか、規則の範囲を `range from koyomi` にして、koyomi の日をそのまま範囲にしてください。

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

**いつ出るか**: 規則の日付の入力に渡す値が、koyomi の日付の日のほかに、何日かを言わないところ（ワークフローの入力、タスクの結果、`now`）からも来ることがあるとき、または koyomi がその日付の日を数えないとき（入力の組み合わせが確かめる数を超える、途中で計算が止まる入力がある）。規則から生成したコードが、ワークフローを走らせたときに入口で日を確かめます。

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

**いつ出るか**: ワークフローが規則の数の出力を chobo の振替の額に渡すとき、その出力が負か 2⁶³ − 1 を超えることがあるとき（chobo は 0 から 2⁶³ − 1 までを受け取ります）。出力の値は rulec が求めます（表の行に書いた数か、区間の計算から）。chobo は範囲の外の額を、拒否する（業務の結果）のではなく呼び出しの失敗にします。注には、その額になる規則の入力の例（規則のベクタから取ったもの）が出ます。

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

**いつ出るか**: ワークフローが規則の数の出力を振替の額に渡すとき、出力の範囲に上限か下限が無いか、値が範囲の分からないところ（範囲の無いタスクの結果など）からも来ることがあるとき。chobo が受け取らない額なら、ワークフローを走らせたときに呼び出しが失敗します。

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

## E204 — 振替が拒否されうる理由を、タスクが処理していません

**いつ出るか**: 規則の出力を額に渡す `do` か `hold` の呼び出しで、操作が拒否されうる理由を、タスクが宣言したエラーとして処理していないとき。拒否されうる理由とは、額を呼び出しが渡す範囲に限った chobo の探索で、拒否される例が見つかった理由です。比べるのは帳簿の境界の理由（勘定の `refused as`）、つまり額で決まる拒否だけです。同じキーを別の引数で使い直すことのように、前の呼び出しで決まる拒否は比べません。探索は chobo の検査と同じ深さまでたどります。`post` と `void` は仮押さえの状態で拒否され、dandori が案件の状態ごとに確かめます（dandori の E022）。

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

## W204 — 振替がどの理由で拒否されうるかを決められません

**いつ出るか**: タスクが帳簿の境界の理由を処理しているのに、額を呼び出しが渡す範囲に限った chobo の探索で、その理由で拒否される例が見つからないとき。探索は chobo の検査と同じ深さまでしかたどらないので、起きないと言えるのはその深さまでです。額の範囲が分からないときや、chobo から帳簿の情報を得られないときも、この警告で「決められない」と知らせます。

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

**いつ出るか**: ワークフローが koyomi の日付を呼ぶとき、日付の入力に渡す日が、koyomi の入力の範囲を外れることがあるとき。渡す日がほかの koyomi の日付の日なら、koyomi が数えたその日の全部で確かめます。範囲の中の日なら、カレンダーに問い合わせる日がデータの範囲に収まることを、koyomi の検査が確かめています（koyomi の E203）。注には、外れる日と、koyomi がその日を返す入力が出ます。

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

**いつ出るか**: ワークフローが koyomi の日付の入力に渡す日を、dandori が知らないとき。ワークフローの入力、タスクの結果、`now` から来る日は、何日かを言いません。dandori には、日付の範囲を書く書き方がまだありません。範囲の外の日は、ワークフローを走らせたときに、koyomi が生成したコードが受け付けません。

**直し方**: ほかの koyomi の日付の日を渡せば、決められるようになります。そうでなければ、このままで構いません。範囲の外の日は、koyomi が実行時に受け付けません。

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

**いつ出るか**: ワークフローが chobo の仮押さえを案件として追い（`case … follows <帳簿>.<振替>`）、確定か取消をするところで、仮押さえを作ってからその呼び出しまでの長さの下限が、有効期限（振替の `pending expires after`）以上のとき。帳簿はどの実行でもその呼び出しを `expired` で拒否し、通ったあとの流れは動きません。長さは dandori がフローの文から数えます。決まった長さの `wait` はその長さです。koyomi の日付の時刻までの `wait until` は、日付の入力に仮押さえのあとで読んだ `now` を渡したものなら、koyomi が数えた入力から日付までの日数の最小と最大と、日付の時刻から出ます。注には、下限と、その下限になる文が出ます。

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

**いつ出るか**: 仮押さえを作ってから確定か取消までの長さが、有効期限の前にも後にもなりうるとき、または長さの上限が分からないとき。上限は、仮押さえを作るタスクと、あいだのタスクの `timeout` とリトライの回数から出ます。`timeout` の無いタスク、規則や日付の呼び出し（`.flow` に時間の上限がありません）、上限の分からない `wait until` があれば、上限はありません。期限が切れていれば帳簿が `expired` で拒否し、フローはそれを処理しています（dandori の E022）。期限が切れないと示せたとき（上限が有効期限より短いとき）は、何も出しません。

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

<a id="w901"></a>

## W901 — 鍵が契約の文書に書かれています

**いつ出るか**: プロジェクトの `.proto` か、言語が読む OpenAPI・AsyncAPI・JSON Schema の文書（dandori の `use openapi`・`use smithy`、rulec の `import jsonschema` と `shape … jsonschema`、sakai の地図の公表された言語の `openapi`・`asyncapi` が参照する `.json`・`.yaml`・`.yml`）のどこか（値でもコメントでも）に、鍵の形の値があるとき。調べる鍵は、AWS のアクセスキー ID、GitHub・Slack・Stripe・OpenAI・Anthropic・Google の鍵やトークン、Slack の Incoming Webhook の URL、PEM の秘密鍵で、どれもプロバイダーが接頭辞や形を決めているものです。契約の文書は、同じものを複数の言語が読むので、言語ごとではなく ritsu がファイルごとに一度だけ調べます。言語のファイルに書いた鍵は、それぞれの言語の W901 です。診断には鍵の種類と、接頭辞と、長さだけを出し、鍵そのものも、その行も出しません。

**直し方**: 鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。本物の鍵なら、まずプロバイダーで無効にしてください。ファイルから消しても、リポジトリの履歴には残ります。テスト用の値なら、同じ行のコメントに `ritsu: test secret` と書いてください（`.proto` は `//`、YAML は `#` で始まるコメント）。JSON の文書にはコメントが書けないので、例の値（`EXAMPLE` で終わる AWS のアクセスキー ID や、接頭辞のあとが一つの文字の繰り返しの値）に替えてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`maps.proto`:

```proto
syntax = "proto3";

package maps.v1;

// サービスを呼ぶときの Google Maps の API キー: AIzaSyD-ritsu-fake-key-for-tests-000000
message Place {
  string id = 1;
}
```

<a id="e905"></a>

## E905 — 秘密の値を、地図の外か、印を付けたコンテキストと関係の無いコンテキストへ送ります

**いつ出るか**: ワークフローが、契約が秘密と印を付けた値（`.proto` の `debug_redact`、OpenAPI のスキーマの `x-data-classification`・`x-sensitive-data`・`format: password`、`.flow` の `secret`）を、プロジェクトの中のファイル（OpenAPI の文書、`.proto`、Connect で呼ぶ規則、子の `.flow`、帳簿、日付のファイル）へ送り、そのファイルが地図のどのコンテキストにも属さないか、印を付けたコンテキストと地図の上で関係の無いコンテキストに属するとき。`separate ways` は関係に数えません。印を付けたコンテキストは、印を書いたファイルが属するコンテキストです。`.flow` の `secret` の印と、どのコンテキストにも属さないファイルの印は、フローのコンテキストのものとします。どの地図のコンテキストにも属さないフローは見ません。プロジェクトの外の相手（モデルのプロバイダー、判断のモデルの API、URL、AWS のサービス）へ送ることは、dandori の E906 が言います。

**直し方**: 値の代わりに参照（ID やシークレットの名前）を送り、受け取る側で値を取ってきてください。受け取る側が値を持ってよいなら、地図に関係を足すか、送り先のファイルをコンテキストに入れてください（`owns`）。そこへ送ることを意図しているなら、タスクの下に `discloses <引数> "<理由>"` と書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`店.ctx`:

```ctx
map 店(shop) v1
description "決済がカードの代金を受け取り、受注が注文を受け、通知が客に知らせる"

use context "contexts/決済.ctx"
use context "contexts/受注.ctx"
use context "contexts/通知.ctx"

covers "決済", "受注", "通知"
```

`contexts/決済.ctx`:

```ctx
context 決済(payments) v1
description "客のカードで代金を受け取る"
owner "決済の担当"

owns
  dir "../決済"

published language payments.v1
  proto "../決済/v1/card.proto"

partnership with 受注
```

`contexts/受注.ctx`:

```ctx
context 受注(ordering) v1
description "注文を受け、客に知らせてもらう"
owner "受注の担当"

owns
  dir "../受注"

partnership with 決済
partnership with 通知
```

`contexts/通知.ctx`:

```ctx
context 通知(notices) v1
description "客に知らせる"
owner "顧客対応の担当"

owns
  dir "../通知"

published language notices.v1
  openapi "../通知/api/notices.json"

partnership with 受注
```

`決済/v1/card.proto`:

```proto
syntax = "proto3";

package payments.v1;

message Card {
  string id = 1;
  string number = 2 [debug_redact = true];
}
```

`通知/api/notices.json`:

```
{
  "openapi": "3.1.0",
  "info": {
    "title": "通知",
    "version": "1.0.0"
  },
  "servers": [
    {
      "url": "https://notices.example.com"
    }
  ],
  "security": [
    {
      "bearer": []
    }
  ],
  "paths": {
    "/notices": {
      "post": {
        "operationId": "tell",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": [
                  "order",
                  "card"
                ],
                "properties": {
                  "order": {
                    "type": "string"
                  },
                  "card": {
                    "type": "object",
                    "properties": {
                      "id": {
                        "type": "string"
                      },
                      "number": {
                        "type": "string"
                      }
                    }
                  }
                }
              }
            }
          }
        },
        "responses": {
          "204": {
            "description": "知らせた"
          }
        }
      }
    }
  },
  "components": {
    "securitySchemes": {
      "bearer": {
        "type": "http",
        "scheme": "bearer"
      }
    }
  }
}
```

`受注/注文.flow`:

```flow
workflow 注文 v1
  history encrypted
description "カードで払った注文を受け、客に知らせてもらう"

use proto 決済 from "../決済/v1/card.proto"
use openapi 通知 from "../通知/api/notices.json"

inputs
  注文 : string
  カード : 決済.Card

task 知らせる(order: string, card: 決済.Card)
  http POST 通知 "/notices"
  key

flow
  知らせる(order: 注文, card: カード)
```

関連: [W905](#w905)

<a id="w905"></a>

## W905 — 秘密の値の送り先が地図のどこかを、決められません

**いつ出るか**: ワークフローが秘密の値をプロジェクトの中のファイルへ送るとき、地図が sakai の検査を通らず、フローや送り先のファイルがどのコンテキストに属するかを sakai が答えられないとき。

**直し方**: `sakai check` が通るよう地図を直してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`店.ctx`:

```ctx
map 店(shop) v1
description "決済がカードの代金を受け取り、受注が注文を受け、通知が客に知らせる"

use context "contexts/決済.ctx"
use context "contexts/受注.ctx"
use context "contexts/通知.ctx"
use context "contexts/請求.ctx"

covers "決済", "受注", "通知"
```

`contexts/決済.ctx`:

```ctx
context 決済(payments) v1
description "客のカードで代金を受け取る"
owner "決済の担当"

owns
  dir "../決済"

published language payments.v1
  proto "../決済/v1/card.proto"

partnership with 受注
```

`contexts/受注.ctx`:

```ctx
context 受注(ordering) v1
description "注文を受け、客に知らせてもらう"
owner "受注の担当"

owns
  dir "../受注"

partnership with 決済
partnership with 通知
```

`contexts/通知.ctx`:

```ctx
context 通知(notices) v1
description "客に知らせる"
owner "顧客対応の担当"

owns
  dir "../通知"

published language notices.v1
  openapi "../通知/api/notices.json"

partnership with 受注
```

`決済/v1/card.proto`:

```proto
syntax = "proto3";

package payments.v1;

message Card {
  string id = 1;
  string number = 2 [debug_redact = true];
}
```

`通知/api/notices.json`:

```
{
  "openapi": "3.1.0",
  "info": {
    "title": "通知",
    "version": "1.0.0"
  },
  "servers": [
    {
      "url": "https://notices.example.com"
    }
  ],
  "security": [
    {
      "bearer": []
    }
  ],
  "paths": {
    "/notices": {
      "post": {
        "operationId": "tell",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": [
                  "order",
                  "card"
                ],
                "properties": {
                  "order": {
                    "type": "string"
                  },
                  "card": {
                    "type": "object",
                    "properties": {
                      "id": {
                        "type": "string"
                      },
                      "number": {
                        "type": "string"
                      }
                    }
                  }
                }
              }
            }
          }
        },
        "responses": {
          "204": {
            "description": "知らせた"
          }
        }
      }
    }
  },
  "components": {
    "securitySchemes": {
      "bearer": {
        "type": "http",
        "scheme": "bearer"
      }
    }
  }
}
```

`受注/注文.flow`:

```flow
workflow 注文 v1
  history encrypted
description "カードで払った注文を受け、客に知らせてもらう"

use proto 決済 from "../決済/v1/card.proto"
use openapi 通知 from "../通知/api/notices.json"

inputs
  注文 : string
  カード : 決済.Card

task 知らせる(order: string, card: 決済.Card)
  http POST 通知 "/notices"
  key

flow
  知らせる(order: 注文, card: カード)
```

関連: [E905](#e905)

<a id="e907"></a>

## E907 — コンテキストが公開する操作を、どの action も守っていません

**いつ出るか**: 地図のコンテキストが `open host service` で公開する操作（`.proto` のサービスのメソッドと、OpenAPI の文書の操作）を、どの `.gate` の action も `guards` で守らず（`cedar "…"` と書いた Cedar のスキーマの `@guards` でも守らず）、文書がだれでも呼べると書いてもいない（`security: []`）とき。そのコンテキストのほかの操作を守る action があるときに出ます。どれも守られていなければ W907 です。AsyncAPI のチャネルと規則の Connect のサービスは、`guards` で書けるものでないので、求めません。

**直し方**: その操作を守る action を、そのコンテキストの `.gate` に書いてください。だれでも呼べるようにわざとしている OpenAPI の操作なら、文書でその操作に `security: []` と書いてください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`店.ctx`:

```ctx
map 店(shop) v1
description "注文を受ける店"

use context "contexts/受注.ctx"

covers "受注"
```

`contexts/受注.ctx`:

```ctx
context 受注(orders) v1
description "客の注文を受け、返金する"
owner "受注の担当"

owns
  dir "../受注"

published language orders.v1
  openapi "../受注/api/orders.json"
  open host service getOrder, refundOrder
```

`受注/api/orders.json`:

```
{
  "openapi": "3.1.0",
  "info": {"title": "注文", "version": "1.0.0"},
  "servers": [{"url": "https://orders.example.com"}],
  "security": [{"bearer": []}],
  "paths": {
    "/orders/{orderId}": {
      "get": {
        "operationId": "getOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"200": {"description": "注文"}}
      }
    },
    "/orders/{orderId}/refunds": {
      "post": {
        "operationId": "refundOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"201": {"description": "返金した"}, "403": {"description": "許されていない"}}
      }
    }
  },
  "components": {"securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}}}
}
```

`受注/返金.gate`:

```
gate 返金(refunds) v1
description "注文を返金してよい人"

use openapi 注文 from "api/orders.json"

role 係(clerk)
  description "客に応対し、返金する"

principal 職員(User)
  description "店の職員"
  roles 係

resource 注文(Order)
  description "注文"

action 返金する(refund_order)
  description "注文を返金する"
  guards 注文 refundOrder
  principal 職員
  resource 注文 from orderId

permit 係は返金できる(clerks_refund)
  description "係は注文を返金できる"
  principal in 係
  action 返金する
```

関連: [W907](#w907)

<a id="w907"></a>

## W907 — コンテキストが、公開する操作のどれも action で守っていません

**いつ出るか**: 地図のコンテキストが公開する操作（E907 と同じもの）を、どの action も守っていないとき。コンテキストごとに一つ出ます。そのコンテキストは、まだ sekisho で認可を書いていません。プロジェクトに `.gate` も、地図か要件が指す Cedar も無ければ、X15 は何も言いません。

**直し方**: そのコンテキストの `.gate` を書き、公開する操作をそれぞれ action で守ってください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`店.ctx`:

```ctx
map 店(shop) v1
description "注文を受け、カードで代金を受け取る店"

use context "contexts/受注.ctx"
use context "contexts/決済.ctx"

covers "受注", "決済"
```

`contexts/受注.ctx`:

```ctx
context 受注(orders) v1
description "客の注文を受け、返金する"
owner "受注の担当"

owns
  dir "../受注"

published language orders.v1
  openapi "../受注/api/orders.json"
  open host service getOrder, refundOrder
```

`contexts/決済.ctx`:

```ctx
context 決済(payments) v1
description "客のカードで代金を受け取る"
owner "決済の担当"

owns
  dir "../決済"

published language payments.v1
  openapi "../決済/api/payments.json"
  open host service createCharge
```

`受注/api/orders.json`:

```
{
  "openapi": "3.1.0",
  "info": {"title": "注文", "version": "1.0.0"},
  "servers": [{"url": "https://orders.example.com"}],
  "security": [{"bearer": []}],
  "paths": {
    "/orders/{orderId}": {
      "get": {
        "operationId": "getOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"200": {"description": "注文"}}
      }
    },
    "/orders/{orderId}/refunds": {
      "post": {
        "operationId": "refundOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"201": {"description": "返金した"}, "403": {"description": "許されていない"}}
      }
    }
  },
  "components": {"securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}}}
}
```

`決済/api/payments.json`:

```
{
  "openapi": "3.1.0",
  "info": {"title": "決済", "version": "1.0.0"},
  "servers": [{"url": "https://payments.example.com"}],
  "security": [{"bearer": []}],
  "paths": {
    "/charges": {
      "post": {
        "operationId": "createCharge",
        "responses": {"201": {"description": "支払"}}
      }
    }
  },
  "components": {"securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}}}
}
```

`決済/代金.gate`:

```
gate 代金(charges) v1
description "カードで代金を受け取ってよい人"

use openapi 決済 from "api/payments.json"

role 会計係(cashier)
  description "注文の支払いを受け付ける"

principal 職員(User)
  description "店の職員"
  roles 会計係

resource 支払(Charge)
  description "カードの支払"

action 代金を受け取る(create_charge)
  description "カードで代金を受け取る"
  guards 決済 createCharge
  principal 職員
  resource 支払

permit 会計係は受け取れる(cashiers_charge)
  description "会計係はカードで代金を受け取れる"
  principal in 会計係
  action 代金を受け取る
```

関連: [E907](#e907)

<a id="e908"></a>

## E908 — ワークフローが呼ぶ操作を、ゲートがそのワークフローにどの組み合わせでも許しません

**いつ出るか**: `.gate` が `workflow … from` で書いたワークフローが、そのゲートの action が守る操作を呼び（`use openapi` の `http`、`use proto` の `connect`）、その action がワークフローをどの組み合わせでも許さないとき。その呼び出しまで進んだ実行は、いつもそこで拒まれます。

**直し方**: ゲートに、ワークフローを許す permit を書くか（`principal is workflow <名前>`）、呼び出しを消してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`orders.json`:

```
{
  "openapi": "3.1.0",
  "info": {"title": "注文", "version": "1.0.0"},
  "servers": [{"url": "https://orders.example.com"}],
  "security": [{"bearer": []}],
  "paths": {
    "/orders/{orderId}": {
      "get": {
        "operationId": "getOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"200": {"description": "注文"}}
      }
    },
    "/orders/{orderId}/refunds": {
      "post": {
        "operationId": "refundOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"201": {"description": "返金した"}, "403": {"description": "許されていない"}}
      }
    }
  },
  "components": {"securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}}}
}
```

`返品.flow`:

```flow
workflow 返品 v1
description "品物が戻ったら、注文を返金する"

use openapi 注文 from "orders.json"

inputs
  注文番号 : string

task 返金する(orderId: string)
  http POST 注文 "/orders/{orderId}/refunds"
  key

flow
  返金する(orderId: 注文番号)
```

`返金.gate`:

```
gate 返金(refunds) v1
description "注文を見て、返金してよい人"

use openapi 注文 from "orders.json"

enum 注文の状態(order_status) = 支払済(paid) | 返品済(returned)

role 係(clerk)
  description "客に応対し、返金する"

principal 職員(User)
  description "店の職員"
  roles 係

workflow 返品(returns) from "返品.flow"
  description "品物が戻ったら、注文を返金する"

resource 注文(Order)
  description "注文"
  attributes
    状態(status) : 注文の状態

action 返金する(refund_order)
  description "注文を返金する"
  guards 注文 refundOrder
  principal 職員, Workflow
  resource 注文 from orderId

permit 係は返金できる(clerks_refund)
  description "係は注文を返金できる"
  principal in 係
  action 返金する
```

関連: [W909](#w909), [W908](#w908)

<a id="w908"></a>

## W908 — ワークフローが、呼ばない操作の action を許されています

**いつ出るか**: `.gate` の action のうち、`workflow … from` で書いたワークフローが許されるもの（組み合わせによって許されるものも）が守る操作を、そのワークフローのフローがどこでも呼ばないとき。ワークフローは要るより多く許されています。

**直し方**: ワークフローを許す permit から、その action を外してください。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`orders.json`:

```
{
  "openapi": "3.1.0",
  "info": {"title": "注文", "version": "1.0.0"},
  "servers": [{"url": "https://orders.example.com"}],
  "security": [{"bearer": []}],
  "paths": {
    "/orders/{orderId}": {
      "get": {
        "operationId": "getOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"200": {"description": "注文"}}
      }
    },
    "/orders/{orderId}/refunds": {
      "post": {
        "operationId": "refundOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"201": {"description": "返金した"}, "403": {"description": "許されていない"}}
      }
    }
  },
  "components": {"securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}}}
}
```

`返品.flow`:

```flow
workflow 返品 v1
description "品物が戻ったら、注文を返金する"

use openapi 注文 from "orders.json"

inputs
  注文番号 : string

task 返金する(orderId: string)
  http POST 注文 "/orders/{orderId}/refunds"
  errors denied = 403
  key

flow
  返金する(orderId: 注文番号)
    on denied => fail Denied "ゲートが返金を許さなかった"
```

`返金.gate`:

```
gate 返金(refunds) v1
description "注文を見て、返金してよい人"

use openapi 注文 from "orders.json"

enum 注文の状態(order_status) = 支払済(paid) | 返品済(returned)

role 係(clerk)
  description "客に応対し、返金する"

principal 職員(User)
  description "店の職員"
  roles 係

workflow 返品(returns) from "返品.flow"
  description "品物が戻ったら、注文を返金する"

resource 注文(Order)
  description "注文"
  attributes
    状態(status) : 注文の状態

action 注文を見る(view_order)
  description "注文を見る"
  guards 注文 getOrder
  principal 職員, Workflow
  resource 注文 from orderId

action 返金する(refund_order)
  description "注文を返金する"
  guards 注文 refundOrder
  principal 職員, Workflow
  resource 注文 from orderId

permit 係は返金できる(clerks_refund)
  description "係は注文を返金できる"
  principal in 係
  action 返金する

permit 係は注文を見られる(clerks_look)
  description "係は注文を見られる"
  principal in 係
  action 注文を見る

permit 返品は注文を見て返金できる(returns_look_and_refund)
  description "返品のワークフローは、どの注文も見て、返金できる"
  principal is workflow 返品
  action 注文を見る, 返金する
```

関連: [E908](#e908)

<a id="w909"></a>

## W909 — 拒まれることのある呼び出しが、拒まれたときのエラーを宣言していません

**いつ出るか**: ワークフローが呼ぶ操作を守る action が、ワークフローを組み合わせによっては拒む（拒むかを決められないときも）のに、呼ぶタスクが、拒まれたときのエラー（HTTP の 403、Connect の `permission_denied`）を宣言していないとき。拒まれると、ワークフローは宣言していない失敗で止まります。

**直し方**: タスクに `errors denied = 403`（Connect なら `errors denied = permission_denied`）と書き、拒まれたときにどうするかを呼び出しの下に書いてください（`on denied => …`）。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`orders.json`:

```
{
  "openapi": "3.1.0",
  "info": {"title": "注文", "version": "1.0.0"},
  "servers": [{"url": "https://orders.example.com"}],
  "security": [{"bearer": []}],
  "paths": {
    "/orders/{orderId}": {
      "get": {
        "operationId": "getOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"200": {"description": "注文"}}
      }
    },
    "/orders/{orderId}/refunds": {
      "post": {
        "operationId": "refundOrder",
        "parameters": [{"name": "orderId", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {"201": {"description": "返金した"}, "403": {"description": "許されていない"}}
      }
    }
  },
  "components": {"securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}}}
}
```

`返品.flow`:

```flow
workflow 返品 v1
description "品物が戻ったら、注文を返金する"

use openapi 注文 from "orders.json"

inputs
  注文番号 : string

task 返金する(orderId: string)
  http POST 注文 "/orders/{orderId}/refunds"
  key

flow
  返金する(orderId: 注文番号)
```

`返金.gate`:

```
gate 返金(refunds) v1
description "注文を見て、返金してよい人"

use openapi 注文 from "orders.json"

enum 注文の状態(order_status) = 支払済(paid) | 返品済(returned)

role 係(clerk)
  description "客に応対し、返金する"

principal 職員(User)
  description "店の職員"
  roles 係

workflow 返品(returns) from "返品.flow"
  description "品物が戻ったら、注文を返金する"

resource 注文(Order)
  description "注文"
  attributes
    状態(status) : 注文の状態

action 返金する(refund_order)
  description "注文を返金する"
  guards 注文 refundOrder
  principal 職員, Workflow
  resource 注文 from orderId

permit 係は返金できる(clerks_refund)
  description "係は注文を返金できる"
  principal in 係
  action 返金する

permit 返品は戻った注文を返金できる(returns_refunds_returned_orders)
  description "返品のワークフローは、戻ってきた注文を返金できる"
  principal is workflow 返品
  action 返金する
  when resource.状態 is 返品済
```

関連: [E908](#e908)
