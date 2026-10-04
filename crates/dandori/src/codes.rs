//! The ledger of every diagnostic code (ritsu's DESIGN 4.3): `dandori explain` reads it, and
//! tests/codes.rs runs every entry's example (`check`, and `build` for every platform for the two
//! codes a build finds) and requires its code to come out, so the example cannot go stale while
//! the prose around it still reads well. The entries are dandori's; how they are written out is
//! ritsu-base's ([`ritsu_base::ledger`]). The table of the site (reference/codes.md) says the same
//! in one line a code.

use ritsu_base::ledger::{Entry, Ledger, Repro};
use ritsu_base::text::Text;

/// An entry whose example is the smallest `.flow` that gets the code.
fn e(code: &'static str, title: Text, when: Text, fix: Text, example: &'static str, related: &'static [&'static str]) -> Entry {
    Entry::new(code, title, when, fix, Repro::File { body: example, beside: &[] }, related)
}

// ── What the examples need beside them ───────────────────────────────────

/// A door that is open, then shut for good: the machine the examples of the cases follow. It
/// refuses `open_it` in every state, and `shut_it` once the door is shut.
const DOOR: (&str, &[u8]) = (
    "door.rule",
    b"rule door v1\n\nenum state = open | shut\nenum event = shut_it | open_it\n\ninputs\n  state : state\n  event : event\n\noutputs\n  next_state : state\n  accepted   : bool\n\ntable step\npolicy unique\n| state | event   | -> next_state | accepted |\n| open  | shut_it | shut          | true     |\n| open  | open_it | state         | false    |\n| shut  | -       | state         | false    |\n\nmachine door over step\n  carry   state -> next_state\n  initial open\n  final   shut\n",
);
/// The child flow of E015's example, whose input is not the task's parameter.
const CHILD: (&str, &[u8]) = ("child.flow", b"workflow child v1\n\ninputs\n  name : string\n\nflow\n  pass\n");
/// The `.proto` of E017's example, which has no service.
const SHOP: (&str, &[u8]) = ("shop.proto", b"syntax = \"proto3\";\n\npackage shop.v1;\n\nmessage Order {\n  string id = 1;\n}\n");
/// A rule aliased `rules`, which rules.ts and rules.py would hide (E006).
const RULES: (&str, &[u8]) = (
    "rules.rule",
    "rule 会員の扱い(rules) v1\n\ninputs\n  member : bool\n\noutputs\n  urgent : bool\n\ntable decide\npolicy unique\n| member | -> urgent |\n| true   | true      |\n| false  | false     |\n".as_bytes(),
);

pub fn ledger() -> Ledger {
    let entries = vec![
        // ── Words and names ──
        e(
            "E001",
            tr!("構文の誤りがあります", "A syntax error"),
            tr!("字句や行の形が言語の構文に合わないとき。閉じていない文字列、言語に無い文字、字下げの誤り、要るところに無い宣言などです。", "What is written does not fit the syntax: a string not closed, a character the language does not have, an indent that is wrong, a declaration missing where one is needed."),
            tr!("示された位置を直してください。そこに何が要るかは、メッセージに出ます。", "Correct it where it points: the message says what is expected there."),
            "workflow w v1\n\nflow\n  let x = \"open\n",
            &["E009"],
        ),
        e(
            "E002",
            tr!("無い名前を書いています", "A name that is not there"),
            tr!(
                "型（`.proto` に無いメッセージや列挙と、読めなかった import にある型のフィールドを持つメッセージを含みます）・変数・フィールド・規則・タスクの名前が見つからないとき。日付のファイルに無い日付、無い帳簿、帳簿に無い振替、単位の表に無い単位、`incl_tax`・`excl_tax` のほかの税区分でも出ます。",
                "A type (a message or an enum the `.proto` does not have, or a message that has a field of a type from a file the `.proto` imports and that could not be read, too), a variable, a field, a rule or a task that is not there; a date the dates file does not have, a book, or a transfer the book does not have; a unit the table of units does not have, or a tax that is neither incl_tax nor excl_tax, too."
            ),
            tr!("名前の綴りを直すか、宣言を足してください（`task`、`record`、`use rule`、`use dates`、`use book` など）。", "Correct the spelling, or declare it (`task`, `record`, `use rule`, `use dates`, `use book`, ...)."),
            "workflow w v1\n\nflow\n  notify()\n",
            &["E006"],
        ),
        e(
            "E003",
            tr!("型が合いません", "Types that do not match"),
            tr!(
                "値の型が、置いた場所の型と違うとき。オプショナルな値をそのまま使ったもの、`none` を置けない場所に置いたもの、リストのリスト、型の決まらない `{{…}}` や `[]`、数でないものに付けた範囲、どの数も入らない範囲、型の次元に無い単位を付けた範囲の端、型の単位で数えると整数にならない範囲の端、自分自身を含むレコード（`.flow` に書いたものも、`.proto` のメッセージから作ったものも）、UTC オフセットの書かれていないカレンダーを使う日付に渡した時刻（`now` など）が、これにあたります。単位は単位の型で比べるので、`JPY` と `円` は同じ型で、`kg` と `g`、税込と税抜は違う型です。",
                "A value's type is not the type of where it goes: a value that may be absent used as it is, `none` where it cannot go, a list of lists, a `{{…}}` or `[]` whose type cannot be told, a range on what is not a number, a range no number is in, a bound of a range whose unit is not of its type or that does not come to a whole number of it, a record that contains itself, written in the `.flow` or made of a message of a `.proto`, a time (`now`, say) passed to a date of a dates file whose calendar says no UTC offset. Units are compared as units: `JPY` and `円` are one type, `kg` and `g`, and tax in and tax out, are not."
            ),
            tr!("渡す値を、受け取る側の型にしてください。単位が違うなら、換算する規則かタスクを通してください（dandori は値を換算しません）。", "Pass a value of the type that is taken; for another unit, go through a rule or a task that converts it (dandori converts nothing)."),
            "workflow w v1\n\ninputs\n  count : int\n\ntask send(text: string)\n\nflow\n  send(text: count)\n",
            &["E014"],
        ),
        e(
            "E004",
            tr!("引数や出力の数が合いません", "Too many or too few arguments or outputs"),
            tr!("呼び出しが要る引数を渡していない、無い引数を渡している、同じ引数を二度渡している、`succeed` が出力を書いていない、`{{…}}` のレコードにフィールドが足りないとき。", "A call that leaves out an argument it needs, gives one the task does not take or gives one twice, a `succeed` that leaves out an output, a field left out of a `{{…}}` record."),
            tr!("宣言のとおりに、引数と出力を一つずつ書いてください。", "Write each argument and output as the declaration has them."),
            "workflow w v1\n\ntask send(text: string)\n\nflow\n  send()\n",
            &["E003"],
        ),
        e(
            "E005",
            tr!("規則、日付のファイル、帳簿を読めません", "A rule, a dates file or a book that could not be read"),
            tr!(
                "rulec の検査を通らない規則、要素の並び（`elements`）をたどる規則（dandori は規則に並びを渡せません）、無いことがある入力か出力（`T?`）を持つ規則を使うとき。`connect` で呼ぶ規則では、Connect のサービスの情報や、サービスが列挙の値を何と呼ぶかを、rulec から得られないときにも出ます。koyomi の検査を通らない日付のファイル、chobo の検査を通らない帳簿、無いファイルでも出ます。規則・日付のファイル・帳簿を読めない `dandori` のコマンドだけで走らせたときは、E018 になります。",
                "A rule that does not pass rulec's check, one that walks a list of elements (`elements`), which dandori does not pass a rule, or one with an input or an output that may be none (`T?`); for `connect`, one whose Connect service rulec says nothing of, or does not say what the service calls the values of an enum. A dates file that does not pass koyomi's check, and a book that does not pass chobo's, too (or one that is not there). Run with the `dandori` binary alone, which reads no rule, dates file or book, it is E018."
            ),
            tr!("注に出る rulec・koyomi・chobo の診断に従って、そのファイルを直してください。", "Correct the file as the notes, rulec's, koyomi's or chobo's diagnostics, say."),
            "workflow w v1\n\nuse rule fee from \"fee.rule\"\n\nflow\n  pass\n",
            &["E002", "E018"],
        ),
        Entry::new(
            "E006",
            tr!("一つの名前が二つのものを指します", "One name for two things"),
            tr!(
                "同じ名前を二度宣言しているとき（規則・日付のファイル・帳簿は、`use rule`・`use dates`・`use book` の名前を共有します）。生成するコードで同じ名前になる型（`warehouse.Stock` と `warehouse_Stock`）や、規則と API に付けた同じ名前でも出ます。dandori は規則の名前も見ます。rulec が規則のために生成するコードは、規則の別名（関数とモジュール、Go ではパッケージ）と、受け取る列挙の別名で呼ばれ、dandori がまわりに生成するコードは、それを自分の名前と並べて読み込みます。このため、まわりのコードがもう使っている名前（`rules`、`args`、`out`、`activity`、`handler`、`ctx` など、DESIGN 1.15）は使えません。別名が同じ二つの規則（rulec が生成するファイルが重なります）や、タスクと規則のアクティビティ（`rule_<規則>`）が同じ名前になるときにも出ます。",
                "A name declared twice (rules, dates files and books share the names of `use rule`, `use dates` and `use book`); types that come to one name in the code dandori writes (`warehouse.Stock` and `warehouse_Stock`); one name for a rule and an API. The rules' names too: the code rulec generates for a rule goes by the rule's alias (its function and module, in Go its package) and the aliases of the enums it takes, and the code dandori writes around it imports those beside its own names, so a name that code uses already (`rules`, `args`, `out`, `activity`, `handler`, `ctx` and the others of DESIGN 1.15) cannot be one; nor can two rules have one alias (the files rulec generates would be one), nor a task and a rule's activity (`rule_<rule>`) one name."
            ),
            tr!("どちらかの名前を変えてください。規則の別名なら、規則のファイルの `rule <名前>(<別名>) v1` で別名を変えてください。", "Rename one of the two; for a rule's alias, change the alias in `rule <name>(<alias>) v1` in the rule's file."),
            Repro::File { body: "workflow w v1\n\nuse rule rule_of_thumb from \"rules.rule\"\n\ninputs\n  member : bool\n\nflow\n  let r = rule_of_thumb(member: member)\n", beside: &[RULES] },
            &["E002", "E005"],
        ),
        e(
            "E007",
            tr!("タスクや規則の書き方が合いません", "Clauses of a task or a rule that do not go together"),
            tr!(
                "タスクの書き方が合わないとき。呼び出し方が二つある、`flow` のタスクにほかの呼び出し方や `image` がある、Connect に無いエラーコード、ステータスの無い HTTP のエラー、呼び出し方が持てない `key` や `callback`、dandori の知らない AWS のサービス、`agent` の無い `model`、Jev が答えられない結果の型などです（一覧はサイトの診断コードの表にあります）。帳簿の操作のタスクでは、振替に無い操作（すぐに確定する振替の hold・post・void、仮押さえにする振替の do）、`key`、`refused as`、エラーの `= …` で出ます。規則の書き方が合わないとき（`lambda` と `connect` の両方、`connect` の無い `connection`、http:// でも https:// でもない `connect`）にも出ます。",
                "A task's clauses that do not go together: two ways of calling, a `flow` task with another one or with an `image`, a Connect error code that is not one, an HTTP error without its status, a `key` or a `callback` the way of calling cannot have, an AWS service it does not know, `model` without an `agent`, an answer Jev cannot give, and the others the site's table of codes lists; for a task that runs an operation of a book, an operation the transfer does not have (hold, post or void of a transfer done at once, do of one that holds first), `key`, `refused as`, an error's `= …`; and a rule's: `lambda` and `connect` together, a `connection` without `connect`, a `connect` that is not http:// or https://."
            ),
            tr!("メッセージに挙がった句を外すか、ほかの句と合う句に替えてください。", "Take out the clause the message names, or put in one that goes with the others."),
            "workflow w v1\n\ntask send(text: string)\n  lambda \"arn:aws:lambda:ap-northeast-1:123456789012:function:send\"\n  http POST \"https://example.com/send\"\n\nflow\n  send(text: \"hi\")\n",
            &["E050"],
        ),
        Entry::new(
            "E008",
            tr!("案件の宣言か、案件への呼び出しが誤っています", "A case declared wrong, or a call that does to a case what it cannot"),
            tr!("案件のレコードに状態のフィールドが無い、無いステートマシンに従う、`.proto` から作った列挙の値がステートマシンの状態と違うなど、案件の宣言が誤っているとき。タスクが案件にできないこと（始めない案件に `starts` のタスクを呼ぶなど）をしているときにも出ます。帳簿の仮押さえに従う案件では、合わない操作（hold は `starts`、post は `sends post`、void は `sends void` で、do は案件を動かしません）、`expire` のほかの `external`、`refused when` で出ます。", "A case declared wrong: its record has no field for the state, the machine it follows is not there, the values of an enum made from a `.proto` are not the machine's states; or a task that does to a case what it cannot. For a case that follows a book's hold, an operation that does not fit it (hold `starts` it, post `sends post`, void `sends void`, and do moves no case), an `external` other than `expire`, `refused when`."),
            tr!("案件のレコードに、ステートマシンの状態の型のフィールドを持たせてください（どのフィールドかは `state <フィールド>` で指定できます）。", "Give the case's record a field of the type of the machine's states (`state <field>` names it)."),
            Repro::File { body: "workflow w v1\n\nuse rule door from \"door.rule\"\n\nrecord Door\n  id : string\n\ncase d : Door follows door.door\n\nflow\n  pass\n", beside: &[DOOR] },
            &["E013", "E020"],
        ),
        e(
            "E009",
            tr!("書けない場所に文があります", "A statement where it cannot be"),
            tr!(
                "`flow` の無いワークフロー、`let <名前> = for …` の本体の最後の行でない `yield`、最後に `yield` の無いそうした `for`、`for … in parallel` のイテレーションの中の `break`・`succeed`・案件への呼び出し・イベントの待ち、`on failure` や `on cancel` の中の `succeed`、イテレーションの中と外の両方で値を入れる変数、`let` なしで呼ぶ規則で出ます。",
                "A workflow without a `flow`; a `yield` that is not the last line of the body of `let <name> = for …`, or such a `for` without one; `break`, `succeed`, a case's call or an event's wait in a round of `for … in parallel`; `succeed` in `on failure` or `on cancel`; a variable given a value both inside a round and outside; a rule called without `let`."
            ),
            tr!("文を書ける場所に移すか、メッセージにある形に書き直してください。", "Move the statement where it can be, or write it as the message says."),
            "workflow w v1\n",
            &["E001"],
        ),
        // ── Arms and values ──
        e(
            "E010",
            tr!("どの分岐にも当たらない値があります", "A value no arm of a match takes"),
            tr!("`match` の分岐が、値がとりうるすべてを書いていないとき。抜けうるのは、列挙の値、`bool` の値、案件の状態（始まっていない案件の `none` も）、オプショナルな値の `none` です。", "A `match` whose arms leave out a value it can take: a value of an enum, of a `bool`, a state of a case (`none` for a case not started, too), `none` of a value that may be absent."),
            tr!("抜けている値の分岐を足してください。その値と、そこに至る実行は、メッセージに出ます。", "Add an arm for the value left out; the message names it, and the run that gets there."),
            "workflow w v1\n\nenum color = red | green\n\ninputs\n  c : color\n\nflow\n  match c\n    red => pass\n",
            &["E011"],
        ),
        e(
            "E011",
            tr!("通ることのない分岐があります", "An arm that can never be taken"),
            tr!("`match` の分岐が、そこで値がとりえない値だけを受けるとき。前の分岐がもう受けた値や、その場所で案件のレコードがとりえない状態などです（案件の状態で分かれる `match` が読むのは、ワークフローが最後に聞いた状態です）。", "An arm of a `match` that takes only values that cannot be there: values an arm before it takes, a state the case's record cannot say at that point (a `match` on a case's state reads what the workflow last heard of it)."),
            tr!("その分岐を消すか、来うる値を受ける分岐に直してください。", "Take the arm out, or make it take a value that can come."),
            "workflow w v1\n\nenum color = red | green\n\ninputs\n  c : color\n\nflow\n  match c\n    red => pass\n    green => pass\n    red => pass\n",
            &["E010"],
        ),
        e(
            "E012",
            tr!("値がまだ入っていないことがある変数を読んでいます", "A variable read where it may have no value yet"),
            tr!("ある分岐でだけ値を入れた変数を、分岐のあとで読むときなど、その場所に至る実行のどれかで、変数にまだ値が入っていないとき。", "A variable read where some run that gets there has not given it a value yet, such as one given a value in one arm only and read after the match."),
            tr!("どの実行でも値が入るように、ほかの分岐でも値を入れるか、読むところを分岐の中に移してください。", "Give it a value on every run that gets there (in the other arms too), or read it inside the arm."),
            "workflow w v1\n\nenum color = red | green\n\ninputs\n  c : color\n\ntask send(text: string)\n\nflow\n  match c\n    red => let note = \"stop\"\n    green => pass\n  send(text: note)\n",
            &["E010"],
        ),
        // ── Cases ──
        Entry::new(
            "E013",
            tr!("始まっていない案件にタスクを呼んでいるか、案件を二度始めています", "A call on a case that has not started, or a case started twice"),
            tr!("案件を始める（`starts`）前に、案件にタスクを呼ぶとき。始まっているかもしれない案件を、もう一度始めるとき。", "A task called on a case before a task that starts it (`starts`) has; a case started again where it may have been started already."),
            tr!("案件を始めるタスクを先に呼んでください。始めたかどうかが実行によって違うなら、`match` で案件の状態（始まっていなければ `none`）を見て分けてください。", "Call the task that starts the case first; where it depends on the run, look at the case's state with `match` (`none` before it starts)."),
            Repro::File { body: "workflow w v1\n\nuse rule door from \"door.rule\"\n\nrecord Door\n  id    : string\n  state : door.state\n\ntask shut(id: string) -> Door\n  sends shut_it\n\ncase d : Door follows door.door\n\nflow\n  d <- shut(id: \"x\")\n", beside: &[DOOR] },
            &["E020", "E008"],
        ),
        e(
            "E014",
            tr!("範囲を外れうる値を渡しています", "A value that can be outside the range where it goes"),
            tr!("範囲のある場所（規則の入力、タスクの引数、書き出すレコードのフィールド、出力）に、範囲を外れうる値を渡すとき。dandori は、値の範囲を、入力の範囲、タスクの結果の範囲、規則が出す範囲、式から求めます。", "A value that can be outside the range of where it goes: a rule's input, a task's parameter, a field of a record written out, an output. The range of a value is worked out from the ranges of the inputs, of the tasks' answers, of what the rules answer, and of the expressions."),
            tr!("渡す値の範囲を狭める（入力やタスクの結果に `range` を書く）か、受け取る側の範囲を広げてください。", "Narrow the value's range (write a `range` on the input or on the task's answer), or widen the range of where it goes."),
            "workflow w v1\n\ninputs\n  n : int  range >=0 <=100\n\ntask take(n: int range >=0 <=10)\n\nflow\n  take(n: n)\n",
            &["W104", "E003"],
        ),
        Entry::new(
            "E015",
            tr!("子の .flow と合いません", "A task that does not fit the .flow it runs"),
            tr!("ほかの `.flow` を走らせるタスク（`flow \"…\"`）が子と合わないとき。dandori は、引数と子の入力、結果と子の出力、宣言したエラーと子の `fail` を比べます。子が読めないとき、子が検査を通らないとき、フローが自分を走らせるときにも出ます。", "A task that runs another `.flow` (`flow \"…\"`) and does not fit it: its parameters and the child's inputs, its answer and the child's outputs, its errors and the child's `fail`s; a child that cannot be read or does not pass the checks, and a flow that runs itself, too."),
            tr!("タスクの引数・結果・エラーを、子の入力・出力・`fail` に合わせてください。", "Make the task's parameters, answer and errors those of the child's inputs, outputs and `fail`s."),
            Repro::File { body: "workflow w v1\n\ntask child(count: int)\n  flow \"child.flow\"\n\nflow\n  child(count: 1)\n", beside: &[CHILD] },
            &["E016"],
        ),
        e(
            "E016",
            tr!("API の記述や帳簿と合いません", "A task that does not fit the API or the book it calls"),
            tr!(
                "タスクが、呼ぶ API の記述（OpenAPI、Smithy、`.proto`）と合わないとき。無い操作、受け取らない引数、要る引数の不足、型・範囲・列挙の違い、レスポンスが省きうるのに `T?` でないフィールド、返さないステータスや例外、冪等トークンでない `key`、ストリームのメソッドなどです。記述が読めないとき、`connect` で呼ぶ `.proto` に `url` が無いとき、メソッドやメッセージが、読めなかった import にある型を使っているときにも出ます。帳簿の操作のタスクでは、操作が受け取るもの（名前と型）と違う引数、仮押さえでない結果（do は何も返しません）、帳簿がその操作を断る理由でないエラーで出ます。",
                "A task that does not fit the description of the API it calls (OpenAPI, Smithy, `.proto`): an operation that is not there, a parameter it does not take or one it needs left out, a type, range or enum that differs, a field the answer may leave out that is not `T?`, a status or an exception it does not answer with, a `key` that is not its idempotency token, a method that streams; a description that cannot be read, a `.proto` without `url` that a `connect` task calls, or a method whose message has a type from a file that could not be read, too. For a task that runs an operation of a book: parameters other than what the operation takes (by name and type), an answer that is not the hold (a do answers nothing), an error that is not a reason the book refuses the operation with."
            ),
            tr!("タスクを記述や帳簿に合わせるか、記述のパスを直してください。", "Make the task fit the description or the book, or correct the description's path."),
            "workflow w v1\n\nuse openapi shop from \"shop.json\"\n\nflow\n  pass\n",
            &["E015", "E017"],
        ),
        Entry::new(
            "E017",
            tr!("実装するサービスと合いません", "A workflow that does not fit the service it implements"),
            tr!("ワークフローが、`implements` で実装する `.proto` のサービスと合わないとき。`.proto` に無いサービス、サービスに書いた名前と違うワークフロー、dandori のオプションが無いメソッド、入力や出力に無いフィールド、型の違う入力や出力、`fails` の名前の違いなどです（一覧は、サイトの診断コードの表と、サービスを実装するページにあります）。", "A workflow that does not fit the service of a `.proto` it implements (`implements`): a service the `.proto` does not have, a workflow the service does not name, a method with none of dandori's options, a field that is no input or output, an input or an output of another type, names of `fails` that differ, and the others the site's table of codes and its page on services list."),
            tr!("`.proto` のサービスとワークフローの入力・出力・`fail` を合わせてください。", "Make the inputs, outputs and `fail`s of the workflow and the service of the `.proto` agree."),
            Repro::File { body: "workflow w v1 implements shop.OrderService\n\nuse proto shop from \"shop.proto\"\n\nflow\n  pass\n", beside: &[SHOP] },
            &["E016"],
        ),
        Entry::new(
            "E018",
            tr!("規則を読めない dandori で、規則を使うフローを確かめています", "A flow that uses rules, run with a dandori that reads none"),
            tr!(
                "dandori 単独のバイナリ（`dandori`）で、`use rule` のあるフローを確かめるとき。このバイナリにはほかの言語が入っておらず（ritsu の DESIGN 2.3）、規則を読めません。dandori は、最初の `use rule` で一度だけこのエラーを出し、規則を読めないことから起きるほかの診断は出しません。exit code は 2 です（フローの誤りではなく、走らせ方の問題なので）。日付のファイル（`use dates`）と帳簿（`use book`）も同じで、koyomi と chobo が入っていないこのバイナリでは読めません。このエラーを出すのは、読めないもののうち最初の宣言で一度だけです。",
                "A flow with a `use rule`, checked with the binary of dandori's own crate (`dandori`), which holds no other language (ritsu's DESIGN 2.3) and reads no rule. It is said once, at the first `use rule`, and nothing that follows from the rules it cannot read is said. The exit code is 2: it is how the command is run, not what the flow says. So with a dates file (`use dates`) and a book (`use book`), which this binary, holding neither koyomi nor chobo, cannot read either; it is said once, at the first of the declarations it cannot read."
            ),
            tr!(
                "同じコマンドを `ritsu dandori` で走らせてください（`ritsu dandori check flow.flow`）。`ritsu dandori` は、規則、日付のファイル、帳簿を同じプロセスの中で読みます。",
                "Run the same command as `ritsu dandori` (`ritsu dandori check flow.flow`), which reads the rules, the dates files and the books in the same process."
            ),
            Repro::File { body: "workflow w v1\n\nuse rule door from \"door.rule\"\n\nflow\n  pass\n", beside: &[DOOR] },
            &["E005"],
        ),
        Entry::new(
            "E020",
            tr!("案件を終わりでない状態に残したまま終わることがあります", "The workflow can end with a case in a state that is not final"),
            tr!("ワークフローが、案件をステートマシンの終わりの状態（`final`）でない状態に残したまま終わりうるとき。`on cancel` が終わってキャンセルで終わるときにも出ます。残してよいなら、`succeed` や `fail` に `leaving <案件>` を付けて、案件を引き渡すと宣言してください。", "The workflow can end with a case in a state that is not one of the machine's final states, also when `on cancel` ends it as cancelled. Where leaving it is meant, `leaving <case>` on `succeed` or `fail` says the case is handed over."),
            tr!("終わる前に案件を終わりの状態まで動かすか、`leaving <案件>` を書いてください。", "Move the case to a final state before the end, or write `leaving <case>`."),
            Repro::File { body: "workflow w v1\n\nuse rule door from \"door.rule\"\n\nrecord Door\n  id    : string\n  state : door.state\n\ntask open_door(id: string) -> Door\n  starts door.door\n  key\n\ncase d : Door follows door.door\n\nflow\n  d <- open_door(id: \"x\")\n", beside: &[DOOR] },
            &["W101", "E013"],
        ),
        Entry::new(
            "E021",
            tr!("どの状態でも拒否されるイベントを送っています", "An event sent that every state refuses"),
            tr!("案件がその場所でとりうるどの状態でも、ステートマシンが拒否するイベントを送るとき。帳簿の仮押さえでは、帳簿がどの状態でも断る操作で出ます。", "An event sent that the machine refuses in every state the case can be in at that point; for a book's hold, an operation the book refuses in every one."),
            tr!("送るイベントか送る場所を直してください。その場所で案件がとりうる状態は、メッセージに出ます。", "Correct the event, or where it is sent; the message says the states the case can be in there."),
            Repro::File { body: "workflow w v1\n\nuse rule door from \"door.rule\"\n\nrecord Door\n  id    : string\n  state : door.state\n\ntask open_door(id: string) -> Door\n  starts door.door\n  key\n\ntask reopen(id: string) -> Door\n  sends open_it\n\ncase d : Door follows door.door\n  refused when accepted = false\n\nflow\n  d <- open_door(id: \"x\")\n  d <- reopen(id: \"x\")\n", beside: &[DOOR] },
            &["E022"],
        ),
        Entry::new(
            "E022",
            tr!("拒否されうるイベントの拒否を処理していません", "An event that can be refused, with nothing to handle the refusal"),
            tr!("案件がその場所でとりうる状態のどれかでステートマシンが拒否するイベントを送るのに、拒否されたときの処理が無いとき。拒否はタスクの `refused as <エラー>` で宣言したエラーとして返ってきます。帳簿の仮押さえでは、帳簿が断る理由そのものがエラーです（仮押さえはワークフローの外で期限が切れるので、押さえたあとの post と void は `expired` で断られることがあります）。", "An event sent that the machine refuses in some state the case can be in there, with nothing to handle the refusal; a refusal comes back as the error the task's `refused as <error>` declares. For a book's hold, the error is the reason the book refuses with (a hold expires on its own, so a post or a void after a hold may be refused with `expired`)."),
            tr!("タスクに `refused as <エラー>` を書き、呼び出しの下に `on <エラー> =>` を書くか、案件の状態を `match` で見てから送ってください。帳簿の操作なら、理由を `errors` に書き、`on <理由> =>` を書いてください。", "Write `refused as <error>` on the task, and `on <error> =>` under the call; or look at the case's state with `match` before sending it. For a book's operation, declare the reason in `errors`, and write `on <reason> =>`."),
            Repro::File { body: "workflow w v1\n\nuse rule door from \"door.rule\"\n\nrecord Door\n  id    : string\n  state : door.state\n\ninputs\n  hurry : bool\n\ntask open_door(id: string) -> Door\n  starts door.door\n  key\n\ntask shut(id: string) -> Door\n  sends shut_it\n\ncase d : Door follows door.door\n  refused when accepted = false\n\nflow\n  d <- open_door(id: \"x\")\n  match hurry\n    true => d <- shut(id: \"x\")\n    false => pass\n  d <- shut(id: \"x\")\n", beside: &[DOOR] },
            &["E021", "W102"],
        ),
        // ── Retries and platforms ──
        Entry::new(
            "E030",
            tr!("案件を動かす呼び出しを `key` なしでリトライしています", "A call that moves a case, retried without a key"),
            tr!("案件を始めるか案件にイベントを送るタスクが、失敗やタイムアウトのあとにリトライされるのに、`key` も `idempotent` も無いとき。最初の一回が外部のサービスでもう通っていると、二度目で案件が二度動きます。", "A task that starts a case or sends it an event, retried after a failure or a timeout, with neither `key` nor `idempotent`: if the first try went through on the other side, the second moves the case again."),
            tr!("`key` を付けてください。リトライだと外部のサービスに見分けてもらえます。", "Give the task `key`, so that the other side can tell a retry from a new request."),
            Repro::File { body: "workflow w v1\n\nuse rule door from \"door.rule\"\n\nrecord Door\n  id    : string\n  state : door.state\n\ntask open_door(id: string) -> Door\n  starts door.door\n  retry 2 times every 1 second\n\ntask shut(id: string) -> Door\n  sends shut_it\n  key\n\ncase d : Door follows door.door\n\nflow\n  d <- open_door(id: \"x\")\n  d <- shut(id: \"x\")\n", beside: &[DOOR] },
            &["W030", "W103"],
        ),
        e(
            "E031",
            tr!("Express のワークフローにできないことです", "What an Express workflow cannot do"),
            tr!("`kind express` のワークフローが、五分を超えて待ちうる、コールバックを待つ、ネストした実行を待つ、`key` なしで外部のデータを変えるとき。Express の実行は五分までで、非同期なら二度走ることがあります。", "A `kind express` workflow that can wait more than five minutes, waits for a callback or for a nested execution, or changes things without `key`: an Express execution runs for five minutes at most, and an asynchronous one may run twice."),
            tr!("`kind standard` にするか、待ちを短くし、外部のデータを変える呼び出しに `key` を付けてください。", "Make it `kind standard`, or shorten the waits and give the calls that change things `key`."),
            "workflow w v1\nkind express\n\nflow\n  wait 10 minutes\n",
            &["E050"],
        ),
        e(
            "E040",
            tr!("一回の実行がプラットフォームの上限を超えることがあります", "One run can grow too large for the platform"),
            tr!("一回の実行の履歴が、プラットフォームの上限を超えうるとき（`dandori build` が出します）。上限は、Step Functions が 25,000 件、Temporal が 51,200 件、Lambda durable functions が 3,000 操作です。Argo Workflows では、ノードが 10,000 個を超えうるときに出ます。", "From `dandori build`: one run's history can outgrow the platform's limit (25,000 events on Step Functions, 51,200 on Temporal, 3,000 operations on Lambda durable functions), or on Argo Workflows more than 10,000 nodes."),
            tr!("ループの回数（`at most`）を下げるか、続きを新しい実行で始めてください。", "Lower the loop counts (`at most`), or start a new execution to go on."),
            "workflow w v1\n\nrecord Status\n  done : bool\n\ntask look() -> Status\n  lambda \"arn:aws:lambda:ap-northeast-1:123456789012:function:look\"\n  idempotent\n\nflow\n  repeat at most 9000 times\n    let s = look()\n    match s.done\n      true => break\n      false => pass\n",
            &["E050"],
        ),
        e(
            "E050",
            tr!("プラットフォームに要るものが無いか、できないことです", "What the platform needs is missing, or it cannot do it"),
            tr!(
                "プラットフォームに要るものが無いときや、プラットフォームにできないことが書かれているとき（`dandori build` が出します）。Step Functions では、呼び出し方や `connection` が無いとき、ネストした実行がエラーを宣言しているとき、`http`・`agent`・`jev` の `timeout` が 60 秒を超えるとき、送信先が HTTPS でないときです。Temporal 以外では、`on cancel`、`event` のタスク、実装するサービスの、実行がいまどこにいるかを聞くメソッドがあるときです。Step Functions と Lambda durable functions では、呼ばれる規則に `lambda` か `connect` が無いとき、呼ばれる日付に `lambda` が無いときです。Step Functions では、帳簿に `lambda` が無いときにも出ます。Lambda durable functions では、invoke する関数に `timeout` があるときです。Argo では、呼び出し方や `image` が無いとき、`workflow template` がエラーを宣言しているとき、`callback` のタスクに `retry` があるときです。",
                "From `dandori build`. On Step Functions, a way of calling or a `connection`, a nested execution's declared errors, a `timeout` over 60 seconds on `http`, `agent` and `jev`, a destination that is not HTTPS; off Temporal, `on cancel`, `event` tasks, and a method of the service the workflow implements that asks where a run is; on Step Functions and Lambda durable functions, a called rule's `lambda` or `connect`, and a called date's `lambda`; on Step Functions, a book's `lambda`; on Lambda durable functions, a `timeout` on a function it invokes; on Argo, a way of calling or an `image`, a `workflow template`'s declared errors, `retry` on a `callback` task."
            ),
            tr!("メッセージに挙がったものを書き足すか、そのプラットフォームにできる書き方にしてください。", "Add what the message says the platform needs, or write it in a way the platform can do."),
            "workflow w v1\n\nflow\n  pass\n\non cancel\n  pass\n",
            &["E040", "E007"],
        ),
        // ── Warnings ──
        e(
            "W030",
            tr!("外部のデータを変えうる呼び出しを `key` なしでリトライしています", "A call that may change the other side, retried without a key"),
            tr!("外部のデータを変えうるタスク（`idempotent` でない呼び出し）が、失敗やタイムアウトのあとにリトライされるのに、`key` が無いとき。", "A task that may change the other side (a call not marked `idempotent`), retried after a failure or a timeout, without `key`."),
            tr!("`key` を付けるか、二度しても一度と同じなら `idempotent` と書いてください。", "Give it `key`, or mark it `idempotent` if doing it twice is the same as once."),
            "workflow w v1\n\ntask send(text: string)\n  http POST \"https://example.com/send\"\n  retry 2 times every 1 second\n\nflow\n  send(text: \"hi\")\n",
            &["E030"],
        ),
        e(
            "W032",
            tr!("Jev のモデルをエイリアスで書いています", "A Jev task that names an alias of the model"),
            tr!("確信度の下限を書くか確信度を受け取る Jev のタスクが、モデルをバージョンではなくエイリアス（`jev-latest`・`jev-preview`）で書いているとき。確信度の目盛りはモデルのバージョンごとに違いうるので、下限の意味が知らないうちに変わります。", "A Jev task that sets or reads a confidence, and names an alias of the model (`jev-latest`, `jev-preview`) rather than its version: how sure a model says it is may read differently from one version to the next, so the floor would come to mean something else unseen."),
            tr!("`model \"jev-1.13.0\"` のように、バージョンで書いてください。", "Name the version, as `model \"jev-1.13.0\"`."),
            "workflow w v1\n\nenum kind = returns | other\n\ntask pick(text: string) -> kind\n  jev \"Is this a return?\"\n    returns \"The customer sends an item back\"\n    other \"Anything else\"\n  model \"jev-latest\"\n  confidence 0.8 else unsure\n\nflow\n  let k = pick(text: \"x\")\n    on unsure => pass\n",
            &["E007"],
        ),
        Entry::new(
            "W101",
            tr!("処理しないエラーで、案件を残したまま失敗することがあります", "An error nothing handles can fail the run with a case not final"),
            tr!("どこでも処理しないエラーで、ワークフローが、案件を終わりでない状態に残したまま失敗しうるとき。`on failure` や `on cancel` で案件を片付けている最中にも出ます。", "An error nothing handles can fail the run with a case in a state that is not final, also while `on failure` or `on cancel` settles cases."),
            tr!("エラーを `on <エラー> =>` で処理するか、`on failure` で案件を片付けてください。", "Handle the error with `on <error> =>`, or settle the case in `on failure`."),
            Repro::File { body: "workflow w v1\n\nuse rule door from \"door.rule\"\n\nrecord Door\n  id    : string\n  state : door.state\n\ntask open_door(id: string) -> Door\n  starts door.door\n  key\n\ntask notify(id: string)\n  errors offline\n\ntask shut(id: string) -> Door\n  sends shut_it\n  key\n\ncase d : Door follows door.door\n\nflow\n  d <- open_door(id: \"x\")\n  notify(id: \"x\")\n  d <- shut(id: \"x\")\n", beside: &[DOOR] },
            &["E020"],
        ),
        Entry::new(
            "W102",
            tr!("起きることのない拒否の処理があります", "An `on <refusal>` that can never happen"),
            tr!("案件がその場所でとりうるどの状態でもステートマシンが受け付けるイベントなのに、拒否されたときの処理（`on <拒否のエラー>`）を書いているとき。", "An `on <refusal>` under a call whose event the machine takes in every state the case can be in there."),
            tr!("その処理を消してください。", "Take the handler out."),
            Repro::File { body: "workflow w v1\n\nuse rule door from \"door.rule\"\n\nrecord Door\n  id    : string\n  state : door.state\n\ntask open_door(id: string) -> Door\n  starts door.door\n  key\n\ntask shut(id: string) -> Door\n  sends shut_it\n  errors not_now\n  refused as not_now\n  key\n\ncase d : Door follows door.door\n\nflow\n  d <- open_door(id: \"x\")\n  d <- shut(id: \"x\")\n    on not_now => pass\n", beside: &[DOOR] },
            &["E022"],
        ),
        Entry::new(
            "W103",
            tr!("案件を始めるタスクに `key` がありません", "A task that starts a case without a key"),
            tr!("案件を始める（`starts`）タスクに `key` が無いとき。呼び出しが外部のサービスで通ったのに結果が届かないと、案件をもう一つ作ってしまうことがあります。", "A task that starts a case (`starts`) without `key`: when the call went through on the other side and its answer did not come back, a second case may be made."),
            tr!("`key` を付けてください。", "Give the task `key`."),
            Repro::File { body: "workflow w v1\n\nuse rule door from \"door.rule\"\n\nrecord Door\n  id    : string\n  state : door.state\n\ntask open_door(id: string) -> Door\n  starts door.door\n\ntask shut(id: string) -> Door\n  sends shut_it\n  key\n\ncase d : Door follows door.door\n\nflow\n  d <- open_door(id: \"x\")\n  d <- shut(id: \"x\")\n", beside: &[DOOR] },
            &["E030"],
        ),
        e(
            "W104",
            tr!("範囲の分からない値を渡しています", "A value whose range nothing says, where a range is"),
            tr!("範囲のある場所（規則の入力、タスクの引数など）に、範囲の分からない値（`range` の無い入力やタスクの結果から来る値）を渡しているとき。範囲を外れるかどうかを、検査は確かめられません。", "A value whose range nothing says (one that comes from an input or a task's answer without `range`), where a range is: a rule's input, a task's parameter and the like. The check cannot tell whether it stays inside."),
            tr!("値の出どころに `range` を書いてください。", "Write a `range` where the value comes from."),
            "workflow w v1\n\ninputs\n  n : int\n\ntask take(n: int range >=0 <=10)\n\nflow\n  take(n: n)\n",
            &["E014"],
        ),
    ];
    Ledger {
        tool: "dandori",
        example_file: "example.flow",
        fence: "flow",
        repro_heading: tr!("再現", "Example"),
        later_text: tr!("（まだ再現がありません）", "(no example yet)"),
        later_markdown: tr!("まだ再現がありません。", "No example yet."),
        entries,
    }
}
