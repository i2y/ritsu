# 診断のコード

`ritsu explain --all --format markdown --lang ja` の出力です。手で直しません。

<a id="e101"></a>

## E101 — .proto として読めないファイル

**いつ出るか**: プロジェクトの `.proto` を、ritsu の一つの読み手（ritsu-proto）が読めないとき。閉じていない `{`、`;` の無い文、知らない `syntax`、proto2 の `group`、UTF-8 でないファイル。どの言語もこの読み手で `.proto` を読むので、読めないファイルは、どの言語からも読めません。それを読む言語は、読むところで自分のコードでも言います（rulec の E013、dandori の E016、sakai の E106、yuen の E205）。

**直し方**: 示された位置を直し、proto3 の `.proto` にします。`buf build` が組めるファイルなら、ritsu の読み手も読みます。

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

**いつ出るか**: ワークフローが規則を呼ぶところで、dandori が知っている値の範囲の中に、規則の前提（入力どうしの関係 `constraint`）を破る組み合わせがあるとき（X2、DESIGN 7.4）。範囲は dandori が値を入れるすべての場所から集めたもので、dandori の E014 と同じ読み方です。前提を破る呼び出しは、規則の生成したコードが入口で断るので、ワークフローを走らせたときに初めて落ちます。注に、二つの値の範囲と、破る組み合わせを書きます。

**直し方**: 呼ぶ前に前提が保たれるよう分岐するか、値の範囲を狭めます（入力やタスクの結果の `range`）。前提のほうが業務に合っていないなら、規則の `constraint` を直します。

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

**いつ出るか**: ワークフローが規則を呼ぶところで、前提が保たれるかを決められないとき（X2）。渡す値に範囲の無いところから来るものがある（タスクの結果に `range` が無い、など）、前提が並びの合計や長さの上限である（dandori は並びの長さを知りません）、前提が koyomi の日付がとる日である（dandori は日付の範囲を運びません）、のどれかです。決められない前提は、規則の生成したコードが、ワークフローを走らせたときに入口で確かめます。

**直し方**: 値の来るところに範囲を書きます（タスクの結果やワークフローの入力の `range`）。範囲を書けないなら、このままで構いません。規則が実行時に断ります。

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

**いつ出るか**: ワークフローが koyomi の日付の結果を規則の日付の入力に渡すとき、koyomi がとりうる日として数えた日のどれかが、規則が宣言した入力の範囲の外にあるとき（X3 の (a)、DESIGN 7.5）。koyomi は範囲のすべての入力で日付を計算するので、外れる日は例として一つに決まります。

**直し方**: 規則の入力の範囲を広げるか、規則の範囲を `range from koyomi` にして、koyomi の日をそのまま範囲にします（rulec の §15.174）。

**再現**: ritsu はこのコードをまだ出さないので、再現はありません

関連: [W202](#w202), [E205](#e205)

<a id="w202"></a>

## W202 — koyomi の日付がとる日が、規則の入力の範囲に収まるかを決められません

**いつ出るか**: koyomi が日付のとる日を数えられないとき（入力の組み合わせが確かめる数を超える、途中で計算が止まる入力がある）か、規則が入力の範囲を答えないとき（X3 の (a)）。

**直し方**: koyomi のファイルの入力の範囲を狭めて、koyomi が数えられるようにします。

**再現**: ritsu はこのコードをまだ出さないので、再現はありません

関連: [E202](#e202)

<a id="e203"></a>

## E203 — 規則の出力が、chobo の受け取らない額になることがあります

**いつ出るか**: ワークフローが規則の数の出力を chobo の振替の額に渡すとき、その出力が 1 未満か 2⁶³ − 1 を超えることがあるとき（X4、DESIGN 7.6）。chobo は範囲の外の額を、断る（業務の結果）のではなく呼び出しの失敗にします。注に、その額になる規則の入力の例（規則のベクタから）を書きます。

**直し方**: 規則が返す額を 1 以上にするか（返金などの負の額は、向きの違う振替に分けます）、振替に渡す前に分岐します。

**再現**: ritsu はこのコードをまだ出さないので、再現はありません

関連: [E204](#e204), [W204](#w204)

<a id="e204"></a>

## E204 — 振替が断られうる理由を、タスクが処理していません

**いつ出るか**: chobo の探索が、規則の出力の範囲の額で振替の操作が断られる例を見つけた理由を、その振替を呼ぶタスクが宣言したエラーとして処理していないとき（X4）。探索は chobo の検査と同じ深さまでたどり、見つけた理由には、そこへ至る操作の例があります。

**直し方**: その理由をタスクのエラーとして宣言し、処理します。

**再現**: ritsu はこのコードをまだ出さないので、再現はありません

関連: [E203](#e203), [W204](#w204)

<a id="w204"></a>

## W204 — タスクが処理する断る理由を、chobo の探索が見つけません

**いつ出るか**: タスクが振替の断る理由として処理しているのに、chobo の探索が、規則の出力の範囲の額でその理由になる例を見つけないとき（X4）。探索は chobo の検査と同じ深さまでしかたどらないので、「起きない」と言えるのはその深さまでです。額や帳簿が決まらないときも、この警告で決められないと言います。

**直し方**: 起きない理由なら、タスクのエラーから外します。深い手順でしか起きないなら、そのままで構いません。

**再現**: ritsu はこのコードをまだ出さないので、再現はありません

関連: [E204](#e204)

<a id="e205"></a>

## E205 — koyomi の日付に渡す日が、入力かカレンダーのデータの範囲を外れます

**いつ出るか**: ワークフローが koyomi の日付を呼ぶとき、渡す日付の範囲が、koyomi の入力の範囲か、カレンダーがデータを持つ日の範囲を外れるとき（X6、DESIGN 7.8）。注に、外れる日と、何の範囲を外れるかを書きます。

**直し方**: koyomi の入力の範囲を広げるか（カレンダーのデータも足します）、渡す日付の範囲を狭めます。

**再現**: ritsu はこのコードをまだ出さないので、再現はありません

関連: [W205](#w205), [E202](#e202)

<a id="w205"></a>

## W205 — koyomi の日付に渡す日が、範囲に収まるかを決められません

**いつ出るか**: ワークフローが koyomi の日付に渡す日付の範囲を、dandori が知らないとき（X6）。koyomi が実行時に範囲の外の日付を断ります。

**直し方**: 渡す日付の来るところに範囲を書きます。

**再現**: ritsu はこのコードをまだ出さないので、再現はありません

関連: [E205](#e205)
