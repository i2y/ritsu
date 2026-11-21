//! The entries of the ledger for the codes of the checks of every combination, the borders and
//! the contracts (E202–E208, E212, W201, W202, E301–E307, W301–W308): what `explain` says of each,
//! and the smallest input that prints it. The ledger itself is `codes.rs`'s; it takes these in place
//! of the entries it writes for later, and puts a code it does not write (W202, W304–W308) after the
//! last of its band.
//!
//! A reproduction that reads no other language is one `.gate` (with a contract beside it, for the
//! codes of `guards`); one that reads a rule, a dates file, a calendar or a flow is a directory run
//! with every language joined (`ritsu sekisho check example.gate`).

use ritsu_base::ledger::{Entry, Repro};

/// The head every reproduction starts with: a clerk, a user who may hold the role, an order.
macro_rules! shop {
    ($rest:literal) => {
        concat!(
            "gate shop v1\ndescription \"A shop's orders\"\n\nrole clerk\n  description \"Answers customers\"\n\nprincipal User\n  description \"A member of the staff\"\n  roles clerk\n\nresource Order\n  description \"An order\"\n\n",
            $rest
        )
    };
}

/// The same, reading the contract `orders.json` (a `use` line comes before the roles).
macro_rules! shop_with_orders {
    ($rest:literal) => {
        concat!(
            "gate shop v1\ndescription \"A shop's orders\"\n\nuse openapi orders from \"orders.json\"\n\nrole clerk\n  description \"Answers customers\"\n\nprincipal User\n  description \"A member of the staff\"\n  roles clerk\n\nresource Order\n  description \"An order\"\n\n",
            $rest
        )
    };
}

const ORDERS: (&str, &[u8]) = (
    "orders.json",
    br#"{
  "openapi": "3.1.0",
  "info": { "title": "Orders", "version": "1" },
  "paths": {
    "/orders/{orderId}/refunds": {
      "post": {
        "operationId": "refundOrder",
        "parameters": [ { "name": "orderId", "in": "path", "required": true, "schema": { "type": "string" } } ],
        "requestBody": { "required": true, "content": { "application/json": { "schema": {
          "type": "object", "required": [ "amount" ],
          "properties": { "amount": { "type": "integer", "minimum": 1, "maximum": 10000 } } } } } },
        "responses": { "201": { "description": "Refunded" }, "403": { "description": "Not allowed" } }
      }
    }
  }
}
"#,
);

/// A contract with the one operation a flow calls to refund an order, whose id is in the path (the
/// example of E212: the line under the workflow speaks of the flow's call of it).
const REFUNDS_ONLY: &str = r#"{
  "openapi": "3.1.0",
  "info": { "title": "Orders", "version": "1" },
  "servers": [ { "url": "https://orders.example" } ],
  "paths": {
    "/orders/{orderId}/refunds": {
      "post": {
        "operationId": "refundOrder",
        "parameters": [ { "name": "orderId", "in": "path", "required": true, "schema": { "type": "string" } } ],
        "responses": { "201": { "description": "Refunded" }, "403": { "description": "Not allowed" } }
      }
    }
  }
}
"#;

fn file(body: &'static str, beside: &'static [(&'static str, &'static [u8])]) -> Repro {
    Repro::File { body, beside }
}

/// The same entry, with its example in Japanese names for `explain --lang ja`.
fn with_ja(mut e: Entry, ja: Repro) -> Entry {
    e.repro_ja = Some(ja);
    e
}

fn joined(files: Vec<(&'static str, &'static str)>) -> Repro {
    Repro::Dir { files, command: vec!["ritsu", "sekisho", "check", "example.gate"] }
}

/// The entries, in the order of their codes.
pub fn entries() -> Vec<Entry> {
    vec![
        Entry::new(
            "E202",
            tr!("`guards` が書く操作が、契約にありません", "`guards` names an operation the contract does not have"),
            tr!(
                "`guards` の操作が、`use openapi`・`use asyncapi` の文書の `operationId`（無ければ方法とパス）や操作のキー、`use proto` のサービスとメソッド、`use book` の振替の操作のどれでもないとき。",
                "The operation of a `guards` line is not an `operationId` (or a method and a path) or an operation's key of the `use openapi` or `use asyncapi` document, a service and method of the `use proto`, or an operation of a transfer of the `use book`."
            ),
            tr!(
                "注にある操作を書いてください。注には操作が、`guards` の行にそのまま書ける形で並びます（`\"POST /orders/{{orderId}}/refunds\"` のように、一つの語として読めないものは `\"…\"` で囲んであります）。",
                "Write one of the operations the note lists. The note lists them as a `guards` line takes them: in quotes, `\"…\"`, when one is not read as a word (`\"POST /orders/{{orderId}}/refunds\"`)."
            ),
            file(shop_with_orders!("action refund_order\n  description \"Refund an order\"\n  guards orders refundOrders\n  principal User\n  resource Order\n\npermit clerks_refund\n  description \"A clerk refunds\"\n  principal in clerk\n  action refund_order\n"), &[ORDERS]),
            &["E203", "E205"],
        ),
        Entry::new(
            "E203",
            tr!("input が、操作の受け取るものと合いません", "An input is not what the operation takes"),
            tr!(
                "`input` の名前が、守る操作の引数にも本文のフィールドにも無いとき。型が違うとき（数を小数や文字列として受け取る、列挙の値が違う、`.proto` の `repeated` のフィールドを一つの値として受け取る）。範囲が操作の受け取る範囲を超えるとき。操作が求めない値に `?` が無いとき。列挙の `input` は、操作が受け取る値（OpenAPI の `enum`、`.proto` のフィールドの型を protobuf の決まりでたどった列挙のうち、Protovalidate の決まりが通す値）を全部持たなければなりません。`.proto` の値の名前は、rulec の `import proto` と同じに読みます。頭にある列挙の名前を落として小文字にし（`CATEGORY_TRAVEL` は `travel`）、同じ番号の名前（`allow_alias`）は一つの値です。",
                "A name under `input` is neither a parameter nor a field of the body of an operation the action guards; or its type is not what the operation takes (a number taken as one with a fraction or as a string, other values of an enum, a `repeated` field of a `.proto` taken as one value); or its range goes past what the operation takes; or the operation does not require it and it has no `?`. An enum `input` has every value the operation takes: an OpenAPI `enum`, or the values of the enum a `.proto` field's type comes to by protobuf's rules for names, as far as Protovalidate's rules let them through. A `.proto` value's name is read as rulec's `import proto` reads it: the enum's own name taken off the front and the rest in lower case (`CATEGORY_TRAVEL` is `travel`), and the names of one number (`allow_alias`) one value."
            ),
            tr!(
                "操作が受け取るものを、同じ名前、同じ型、その中の範囲で書いてください。列挙に足りない値は、注が挙げる名前のどれか一つで足してください。名前の読み方の全部は、rulec の reference の `import proto` の表にあります。",
                "Write what the operation takes, by the same name and type, with a range within the operation's. Add a value the enum lacks by any one of the names the note gives; rulec's reference has the whole table of how names are read, under `import proto`."
            ),
            file(shop_with_orders!("action refund_order\n  description \"Refund an order\"\n  guards orders refundOrder\n  principal User\n  resource Order from orderId\n  input\n    amount : money[GBP]  range >=1GBP <=20_000GBP\n\npermit clerks_refund\n  description \"A clerk refunds up to 50 pounds\"\n  principal in clerk\n  action refund_order\n  when amount <= 50GBP\n"), &[ORDERS]),
            &["E202", "E204"],
        ),
        Entry::new(
            "E204",
            tr!("`from` の引数が、操作のパスかクエリにありません", "`from` names no parameter of the operation's path or query"),
            tr!(
                "`resource … from <引数>` の引数が、守る操作のパスかクエリの引数（proto ならリクエストのフィールド）に無いとき。生成するコードは、その引数で resource を読みます。",
                "The argument of `resource … from <argument>` is not a parameter of the path or the query of an operation the action guards (for a `.proto`, a field of the request); the generated code reads the resource by it."
            ),
            tr!(
                "操作のパスかクエリの引数の名前で書いてください。注には引数が、`from` のあとにそのまま書ける形で並びます（`\"order-id\"` のように、一つの語として読めないものは `\"…\"` で囲んであります）。",
                "Write the name of a parameter of the operation's path or query. The note lists them as `from` takes them: in quotes, `\"…\"`, when one is not read as a word (`\"order-id\"`)."
            ),
            file(shop_with_orders!("action refund_order\n  description \"Refund an order\"\n  guards orders refundOrder\n  principal User\n  resource Order from order_id\n\npermit clerks_refund\n  description \"A clerk refunds\"\n  principal in clerk\n  action refund_order\n"), &[ORDERS]),
            &["E203"],
        ),
        Entry::new(
            "E205",
            tr!("一つの操作を、二つの action が守ります", "Two actions guard one operation"),
            tr!("二つの action が、同じ契約の同じ操作を `guards` に書いたとき。どちらの判断で守るかが決まりません。", "Two actions write the same operation of the same contract under `guards`: which of them decides is not settled."),
            tr!("一つの action にまとめてください。", "Guard the operation with one action."),
            file(shop_with_orders!("action refund_order\n  description \"Refund an order\"\n  guards orders refundOrder\n  principal User\n  resource Order\n\naction refund_again\n  description \"Refund an order once more\"\n  guards orders refundOrder\n  principal User\n  resource Order\n\npermit clerks_refund\n  description \"A clerk refunds\"\n  principal in clerk\n  action refund_order, refund_again\n"), &[ORDERS]),
            &["E202"],
        ),
        Entry::new(
            "E206",
            tr!("規則か日付に渡す値が、入力の範囲を外れるか、規則の前提を破りえます", "A value given to a rule or a date can be outside its input's range, or break a precondition of the rule"),
            tr!(
                "計算した値で規則や日付の関数に渡す属性・input・定数の範囲が、その入力の範囲に収まらないとき。規則の、順序のある列挙の入力に範囲があるとき（`range >=basic <=gold`）は、渡す列挙の値が、範囲に入らない値になりうるとき。渡す値の範囲の中に、規則の前提（`constraint` と `never`）を破る例があるとき。",
                "The range of an attribute, an input or a constant a computed value gives to a rule or a date is not within the range of the input it is given to — for a rule's input of an ordered enum with a range (`range >=basic <=gold`), the value of an enum given can be one the range does not let in; or, over the ranges given, an example breaks a precondition (`constraint`) of the rule."
            ),
            tr!(
                "渡す値の範囲を、入力の範囲（前提を守る範囲）に狭めてください。列挙の値は、値がどれも範囲の中にある列挙の属性か input を渡すか、範囲の中の値を定数で書いてください。",
                "Narrow the ranges of what is given to the input's, or to ones that keep the precondition. For an enum's value, give an attribute or an input of an enum whose values are all inside the range, or write a value inside it as a constant."
            ),
            joined(vec![
                (
                    "example.gate",
                    "gate shop v1\ndescription \"A shop's orders\"\n\nuse rule limit from \"limit.rule\"\n\nrole clerk\n  description \"Answers customers\"\n\nprincipal User\n  description \"A member of the staff\"\n  roles clerk\n  attributes\n    refund_limit : money[GBP]  range >=0GBP <=200GBP\n\nresource Order\n  description \"An order\"\n\naction refund_order\n  description \"Refund an order\"\n  principal User\n  resource Order\n  input\n    amount : money[GBP]  range >=1GBP <=100GBP\n  context\n    band = limit(amount: amount, limit: principal.refund_limit).band\n\npermit clerks_refund_within_their_limit\n  description \"A clerk refunds up to the clerk's limit\"\n  principal in clerk\n  action refund_order\n  when band is within_limit\n",
                ),
                (
                    "limit.rule",
                    "rule limit v1\ndescription \"Whether a refund is within a limit of at most 100 pounds\"\n\nenum band = within_limit | over_limit\n\ninputs\n  amount : money[GBP]  range >=1GBP <=100GBP\n  limit  : money[GBP]  range >=0GBP <=100GBP\n\noutputs\n  band : band\n\nderive excess : money[GBP] = amount - limit  range >=-99GBP <=100GBP\n\ntable decide\npolicy unique\n| excess | -> band : band |\n| <=0GBP | within_limit   |\n| >0GBP  | over_limit     |\n",
                ),
            ]),
            &["W201", "E201"],
        ),
        Entry::new(
            "W201",
            tr!("規則に渡す値が規則の前提を守るかを、決められません", "Whether the values given to a rule keep its precondition cannot be decided"),
            tr!(
                "渡す値の範囲で、規則の前提（`constraint` と `never`）を守るかを rulec が決められないとき。生成するコードは、走らせたときに前提を確かめ、破ればリクエストを拒みます。",
                "rulec cannot decide whether a precondition (`constraint` or `never`) of the rule holds over the ranges given; the generated code checks it when it runs, and denies the request when it breaks."
            ),
            tr!("渡す値の範囲を狭めると、決められることがあります。", "Narrowing the ranges given can let it be decided."),
            Repro::Later,
            &["E206"],
        ),
        Entry::new(
            "E207",
            tr!("カレンダーが、today の日をすべては覆いません", "A calendar does not cover every day of `today`"),
            tr!(
                "`today is open in <カレンダー>` のカレンダーのデータの範囲（表が知る日）が、`today` の範囲を覆わないとき。",
                "The days the data of the calendar of `today is open in <calendar>` covers (the days its tables know) do not cover the range of `today`."
            ),
            tr!(
                "today の範囲をデータの範囲の中に狭めるか、新しい表が出てからカレンダーのコピーを取り直してください。",
                "Narrow today's range to the data's, or take the calendar's copy again when a newer table is out."
            ),
            joined(vec![
                (
                    "example.gate",
                    "gate shop v1\ndescription \"A shop's orders\"\n\nuse calendar days from \"closed_days.cal\"\n\ntoday range >=2026-10-01 <=2027-03-31 offset +00:00\n\nrole clerk\n  description \"Answers customers\"\n\nprincipal User\n  description \"A member of the staff\"\n  roles clerk\n\nresource Order\n  description \"An order\"\n\naction refund_order\n  description \"Refund an order\"\n  principal User\n  resource Order\n  context\n    business_day = today is open in days\n\npermit clerks_refund_on_business_days\n  description \"A clerk refunds on a business day\"\n  principal in clerk\n  action refund_order\n  when business_day\n",
                ),
                ("closed_days.cal", "calendar closed_days v1\n\nsource holidays = file \"holidays.csv\" sha256:899aee90fcd554a9\n  format csv\n  covers 2026-01-01..2026-12-31\n\nclosed weekly sat, sun\nclosed holidays\n"),
                ("holidays.csv", "2026-01-01,New Year's Day\n2026-05-04,Greenery Day\n"),
            ]),
            &["E107"],
        ),
        Entry::new(
            "E208",
            tr!("ワークフローの `.flow` が dandori の検査を通らないか、読めないか、ルートの外にあります", "A workflow's `.flow` does not pass dandori's check, cannot be read, or is outside the root"),
            tr!(
                "`workflow <名前> from \"<.flow>\"` のフローが、dandori の検査を通らないか、読めないとき。フローがルートの外にあるときも出ます（生成する Cedar の `@doc` も、ほかの言語も、フローをルートからのパスで参照するため）。",
                "The flow of `workflow <name> from \"<.flow>\"` does not pass dandori's check, or cannot be read; or the flow is outside the root (the `@doc` of the generated Cedar and the other languages name it by a reference whose path is from the root)."
            ),
            tr!(
                "フローを dandori の検査が通るように直してください。注に、dandori の言うことがあります。ルートの外にあるフローなら、そのフローを含むディレクトリを `--root` でルートにしてください。",
                "Fix the flow until dandori's check passes; the notes say what dandori says. For a flow outside the root, give `--root` a directory that holds it."
            ),
            joined(vec![
                (
                    "example.gate",
                    "gate shop v1\ndescription \"A shop's orders\"\n\nworkflow returns from \"returns.flow\"\n  description \"Refunds a returned order\"\n\nresource Order\n  description \"An order\"\n\naction refund_order\n  description \"Refund an order\"\n  principal Workflow\n  resource Order\n\npermit returns_refunds\n  description \"The returns workflow refunds\"\n  principal is workflow returns\n  action refund_order\n",
                ),
                ("returns.flow", "workflow returns v1\ndescription \"Calls a task it does not declare\"\n\ninputs\n  order_id : string\n\noutputs\n  refund : string\n\nflow\n  let r = refund_order(orderId: order_id)\n  succeed refund = r.id\n"),
            ]),
            &["E201", "E212"],
        ),
        with_ja(
            Entry::new(
                "E212",
                tr!("ワークフローの下の `resource` の行が、フローの持つ値と合いません", "A `resource` line under a workflow is not what the flow holds"),
                tr!(
                    "`workflow … from` の下の `resource <型> from <フローの値>` と、その下の `<属性> = <フローの値>` が、フローの持つ値と合わないとき。フローにその値（入力、変数、そのフィールド）が無いとき。値の型や単位が属性と違うとき。行が当たる呼び出し（`from` の値を resource の ID として渡す呼び出し）に届く値が、属性の範囲の外になりうるとき、属性の列挙に無い値になりうるとき、無いことがあるのに属性に `?` が無いとき（そういう値が届く呼び出しを注で言います）。この行は、フローが resource について何を持っているかをゲートが言うもので、`ritsu check` は、フローの呼び出しをゲートが許すかを、この行を前提にして確かめます。",
                    "A `resource <type> from <value of the flow>` under a `workflow … from`, or an `<attribute> = <value of the flow>` under it, is not what the flow holds: the flow has no such value (an input, a variable, or a field of one); the value is of another type or unit than the attribute; or, at a call the line speaks of (one that gives the `from` value as the resource's id), what the runs that reach the call hold can be outside the attribute's range, a value the attribute's enum does not have, or none where the attribute has no `?` (a note names the calls such a value reaches). These lines are the gate's word for what the flow holds of the resource, and `ritsu check` rests on them when it asks whether the gate allows the calls of the flow."
                ),
                tr!(
                    "フローにある値を書いてください。範囲が合わないなら、フローの側の値の範囲を属性の範囲の中に狭めるか（フローの入力やレコードのフィールドの `range`）、属性の範囲を広げてください。",
                    "Name a value the flow has. Where the ranges differ, narrow what the flow's value can be to what the attribute takes (the `range` of the flow's input or of the record's field), or widen the attribute's range."
                ),
                joined(vec![
                    (
                        "example.gate",
                        "gate shop v1\ndescription \"A shop's orders\"\n\nuse openapi orders from \"orders.json\"\n\nworkflow returns from \"returns.flow\"\n  description \"Refunds a returned order\"\n  resource Order from order_id\n    total = amount\n\nresource Order\n  description \"An order\"\n  attributes\n    total : money[GBP]  range >=0GBP <=5_000GBP\n\naction refund_order\n  description \"Refund an order\"\n  guards orders refundOrder\n  principal Workflow\n  resource Order from orderId\n\npermit returns_refunds_small_orders\n  description \"The returns workflow refunds an order of up to 100 pounds\"\n  principal is workflow returns\n  action refund_order\n  when resource.total <= 100GBP\n",
                    ),
                    ("returns.flow", "workflow returns v1\ndescription \"Refunds a returned order\"\n\nuse openapi orders from \"orders.json\"\n\ninputs\n  order_id : string\n  amount   : money[GBP]  range >=0GBP <=10_000GBP\n\noutputs\n  refunded : money[GBP]\n\ntask refund(orderId: string)\n  http POST orders \"/orders/{orderId}/refunds\"\n  key\n\nflow\n  refund(orderId: order_id)\n  succeed refunded = amount\n"),
                    ("orders.json", REFUNDS_ONLY),
                ]),
                &["E208", "E101", "W202"],
            ),
            joined(vec![
                (
                    "example.gate",
                    "gate 店(shop) v1\ndescription \"店の注文\"\n\nuse openapi 注文 from \"orders.json\"\n\nworkflow 返品(returns) from \"返品.flow\"\n  description \"返品された注文を返金する\"\n  resource 注文 from 注文番号\n    合計 = 金額\n\nresource 注文(Order)\n  description \"注文\"\n  attributes\n    合計(total) : money[JPY]  range >=0JPY <=50_000JPY\n\naction 返金する(refund_order)\n  description \"注文を返金する\"\n  guards 注文 refundOrder\n  principal Workflow\n  resource 注文 from orderId\n\npermit 返品は少額の注文を返金できる(returns_refund_small_orders)\n  description \"返品のワークフローは、1,000 円までの注文を返金できる\"\n  principal is workflow 返品\n  action 返金する\n  when resource.合計 <= 1_000JPY\n",
                ),
                ("返品.flow", "workflow 返品 v1\ndescription \"返品された注文を返金する\"\n\nuse openapi 注文 from \"orders.json\"\n\ninputs\n  注文番号 : string\n  金額     : money[JPY]  range >=0JPY <=100_000JPY\n\noutputs\n  返金額 : money[JPY]\n\ntask 返金する(orderId: string)\n  http POST 注文 \"/orders/{orderId}/refunds\"\n  key\n\nflow\n  返金する(orderId: 注文番号)\n  succeed 返金額 = 金額\n"),
                ("orders.json", REFUNDS_ONLY),
            ]),
        ),
        with_ja(
            Entry::new(
                "W202",
                tr!("ワークフローの下の `resource` の行が当たる呼び出しがありません", "A `resource` line under a workflow speaks of no call"),
                tr!(
                    "`workflow … from` の下の `resource <型> from <フローの値>` に当たる呼び出し（ゲートの action が守る操作の呼び出しで、その型の resource を受け取る引数に、行の `from` の値を渡すもの）が、フローに一つも無いとき。`ritsu check` は、この行を、当たる呼び出しでだけ前提にする（E212 と、ゲートが呼び出しを許すかの確かめ）ので、この行はどの呼び出しにも効きません。注で、なぜ当たらないかを言います。その型の resource を引数から受け取る action が無い（型の書き違い）、受け取る action が、どの引数が ID かを書いていない、フローがその action の守る操作を呼ばない（別のワークフローの行か、要らない行）、呼び出しが ID の引数に別の値を渡す（値の書き違い。渡す値を言います）、のどれかです。フローが dandori の検査を通り（通らなければ E208）、action が守る操作が契約にあるとき（無ければ E202）に言います。",
                    "No call of the flow is one a `resource <type> from <value of the flow>` under a `workflow … from` speaks of: a call of an operation an action of the gate guards, that gives the line's `from` value to the argument the action takes a resource of that type from. `ritsu check` rests on the line only at such calls (E212, and whether the gate allows the call), so the line holds at none. A note says why none is: no action takes a resource of the type from an argument (the type is written wrong), the action that takes one does not say which argument gives its id, the flow calls none of the operations the actions that do guard (the line is under the wrong workflow, or of no use), or the calls give the argument another value (the value is written wrong; the note says what they give). Said once the flow passes dandori's check (else E208) and the operations the actions guard are in their contracts (else E202)."
                ),
                tr!(
                    "呼び出しが ID として渡す値か、action が受け取る resource の型を書いてください。action が ID を渡す引数を書いていなければ、`resource <型> from <引数>` と書いてください。その操作を呼ばないワークフローの下の行なら、消してください。",
                    "Write the value the calls give as the id, or the type of resource the action takes; where the action does not say which argument gives the id, write `resource <type> from <argument>` on it. Take the line out from under a workflow that calls none of the operations."
                ),
                joined(vec![
                    (
                        "example.gate",
                        "gate shop v1\ndescription \"A shop's orders\"\n\nuse openapi orders from \"orders.json\"\n\nworkflow exchanges from \"exchanges.flow\"\n  description \"Refunds the first order of an exchange\"\n  resource Order from order_id\n\nresource Order\n  description \"An order\"\n\naction refund_order\n  description \"Refund an order\"\n  guards orders refundOrder\n  principal Workflow\n  resource Order from orderId\n\npermit exchanges_refund\n  description \"The exchanges workflow refunds an order\"\n  principal is workflow exchanges\n  action refund_order\n",
                    ),
                    ("exchanges.flow", "workflow exchanges v1\ndescription \"Refunds the first order of an exchange\"\n\nuse openapi orders from \"orders.json\"\n\ninputs\n  order_id       : string\n  first_order_id : string\n\noutputs\n  done : bool\n\ntask refund(orderId: string)\n  http POST orders \"/orders/{orderId}/refunds\"\n  key\n\nflow\n  refund(orderId: first_order_id)\n  succeed done = true\n"),
                    ("orders.json", REFUNDS_ONLY),
                ]),
                &["E212"],
            ),
            joined(vec![
                (
                    "example.gate",
                    "gate 店(shop) v1\ndescription \"店の注文\"\n\nuse openapi 注文 from \"orders.json\"\n\nworkflow 交換(exchanges) from \"交換.flow\"\n  description \"交換の元の注文を返金する\"\n  resource 注文 from 注文番号\n\nresource 注文(Order)\n  description \"注文\"\n\naction 返金する(refund_order)\n  description \"注文を返金する\"\n  guards 注文 refundOrder\n  principal Workflow\n  resource 注文 from orderId\n\npermit 交換は返金できる(exchanges_refund)\n  description \"交換のワークフローは、注文を返金できる\"\n  principal is workflow 交換\n  action 返金する\n",
                ),
                ("交換.flow", "workflow 交換 v1\ndescription \"交換の元の注文を返金する\"\n\nuse openapi 注文 from \"orders.json\"\n\ninputs\n  注文番号     : string\n  元の注文番号 : string\n\noutputs\n  済み : bool\n\ntask 返金する(orderId: string)\n  http POST 注文 \"/orders/{orderId}/refunds\"\n  key\n\nflow\n  返金する(orderId: 元の注文番号)\n  succeed 済み = true\n"),
                ("orders.json", REFUNDS_ONLY),
            ]),
        ),
        Entry::new(
            "E301",
            tr!("どの組み合わせでも、どの permit も action を許しません", "No permit allows the action in any combination"),
            tr!(
                "action のどの組み合わせ（principal の型と役割、属性、input、計算した値）でも、当てはまる permit が無いか、forbid に拒まれるとき。だれもその action をできません。",
                "In every combination of the action (the principal's type and roles, the attributes, the inputs, the computed values), no permit applies or a forbid denies it: no one can do the action."
            ),
            tr!(
                "permit を書いてください。だれにもさせないつもりなら、action の下に `nobody \"<理由>\"` を書いてください。",
                "Write a permit for it; if no one is meant to do it, write `nobody \"<why>\"` under the action."
            ),
            file(shop!("action refund_order\n  description \"Refund an order\"\n  principal User\n  resource Order\n"), &[]),
            &["E302", "E303"],
        ),
        Entry::new(
            "E302",
            tr!("permit が許すはずの組み合わせを、どれも forbid が拒みます", "A forbid denies every combination a permit would allow"),
            tr!("permit が当てはまる組み合わせが、どれも forbid にも当てはまるとき。その permit は何も許しません。", "Every combination a permit applies to, a forbid applies to as well: the permit allows nothing."),
            tr!("forbid を狭めるか、だれにも許さないつもりなら permit を消してください。", "Narrow the forbid, or remove the permit if no one is meant to be allowed this."),
            file(shop!("action refund_order\n  description \"Refund an order\"\n  principal User\n  resource Order\n\npermit clerks_refund\n  description \"A clerk refunds\"\n  principal in clerk\n  action refund_order\n\nforbid nobody_refunds\n  description \"No one refunds\"\n  action refund_order\n"), &[]),
            &["E301", "E303"],
        ),
        Entry::new(
            "E303",
            tr!("permit か forbid が、どの組み合わせにも当てはまりません", "A permit or a forbid applies to no combination"),
            tr!(
                "ポリシーの `principal` の行と `when`・`unless` の行が、どの組み合わせでもそろって成り立たないとき。行ごとに、満たされる組み合わせの数を言います。規則が一緒には出さない値を求めているとき（同じ属性から計算する二つの値など）と、ゲートが渡す値には規則が答えない値を求めているときは、その値と、何から計算するかも言います。",
                "The `principal` line and the `when` and `unless` lines of a policy never hold together, in any combination. Each line is said with the number of combinations it is met on. Where the lines ask for values the rules never give together (two values computed from one attribute, say), or for a value a rule answers for nothing the gate gives it, the values and what they are computed from are said too."
            ),
            tr!(
                "満たせない行か、ポリシーそのものを消してください。規則が出さない値を求めているなら、規則が出す値に行を直してください。",
                "Remove the line that cannot be met, or the policy itself. Where the lines ask for values the rules do not give, correct them to values the rules give."
            ),
            file(
                "gate shop v1\ndescription \"A shop's orders\"\n\nrole clerk\n  description \"Answers customers\"\n\nprincipal User\n  description \"A member of the staff\"\n  roles clerk\n  attributes\n    suspended : bool\n\nresource Order\n  description \"An order\"\n\naction refund_order\n  description \"Refund an order\"\n  principal User\n  resource Order\n\npermit clerks_refund\n  description \"A clerk refunds\"\n  principal in clerk\n  action refund_order\n\nforbid suspended_and_not\n  description \"Asks for a suspended user who is not suspended\"\n  action refund_order\n  when principal.suspended\n  unless principal.suspended\n",
                &[],
            ),
            &["E302"],
        ),
        Entry::new(
            "E304",
            tr!("期待が成り立ちません", "An expectation does not hold"),
            tr!("`expect allow` か `expect deny` が選ぶ組み合わせのうち、期待と違う答えになるものがあるとき。成り立たない数と、一つの例を示します。`nobody` と書いた action を許す組み合わせがあるときも言います。例は、検査が数えた組み合わせです。その計算した値になる入力を、検査が規則にかけて確かめていれば、値のあとのかっこに添えます。`(as when …)` は確かめた入力の一つ、`(whenever …)` はどの入力もその値になる範囲です。", "A combination an `expect allow` or `expect deny` picks is answered otherwise: how many, and one of them, are shown. So is an action under `nobody` that some combination allows. The example is a combination as the check counts it. Where the check ran an input through the rules that comes to its computed values, the brackets after them say it: `(as when …)` is one such input, `(whenever …)` a stretch every input of which comes to them."),
            tr!("ポリシーを直すか、期待が言いすぎているなら期待を直してください。", "Fix the policies, or the expectation if it says more than is meant."),
            file(shop!("action refund_order\n  description \"Refund an order\"\n  principal User\n  resource Order\n\npermit clerks_refund\n  description \"A clerk refunds\"\n  principal in clerk\n  action refund_order\n\nexpect deny clerks_never_refund\n  description \"No clerk refunds\"\n  principal in clerk\n  action refund_order\n"), &[]),
            &["E305"],
        ),
        Entry::new(
            "E305",
            tr!("職務の分離が成り立ちません", "A separation does not hold"),
            tr!("`separate` に並べた action のうち二つを、同じ principal（同じ型、同じ役割の組、同じ属性）が許されるとき。resource と context は action ごとに選べます。", "One principal (of one type, holding one set of roles, with one value of each attribute) is allowed two of the actions a `separate` lists; the resource and the context of each may differ."),
            tr!("片方を許される principal がもう片方をできないよう forbid を足すか、permit を狭めてください。", "Add a forbid that keeps whoever is allowed one from the other, or narrow a permit."),
            file(shop!("action refund_order\n  description \"Refund an order\"\n  principal User\n  resource Order\n\naction export_refunds\n  description \"Export the refunds\"\n  principal User\n  resource Order\n\npermit clerks_refund\n  description \"A clerk refunds\"\n  principal in clerk\n  action refund_order\n\npermit clerks_export\n  description \"A clerk exports the refunds\"\n  principal in clerk\n  action export_refunds\n\nseparate refunding_and_exporting\n  description \"Whoever refunds does not export the refunds\"\n  actions refund_order, export_refunds\n"), &[]),
            &["E304"],
        ),
        Entry::new(
            "E306",
            tr!("役割が、`can` に無い action を許されます", "A role is allowed an action its `can` does not list"),
            tr!("その役割（と、それが includes する役割）だけを持つ principal が、どれかの組み合わせで、`can` に無い action を許されるとき。", "A principal holding the role alone (and the roles it includes) is allowed, in some combination, an action its `can` line does not list."),
            tr!("`can` に action を足すか、許す permit を狭めてください。", "Add the action to `can`, or narrow the permit that allows it."),
            file(
                "gate shop v1\ndescription \"A shop's orders\"\n\nrole clerk\n  description \"Answers customers\"\n  can view_order\n\nprincipal User\n  description \"A member of the staff\"\n  roles clerk\n\nresource Order\n  description \"An order\"\n\naction view_order\n  description \"Look at an order\"\n  principal User\n  resource Order\n\naction refund_order\n  description \"Refund an order\"\n  principal User\n  resource Order\n\npermit clerks_view_and_refund\n  description \"A clerk looks at an order and refunds it\"\n  principal in clerk\n  action view_order, refund_order\n",
                &[],
            ),
            &["W302"],
        ),
        Entry::new(
            "E307",
            tr!("action の組み合わせが、予算を超えます", "An action comes to more combinations than the budget"),
            tr!(
                "action の組み合わせの数（と、日付を確かめるために数える today と値の組）が、予算（既定は 10⁸ 通り、`--budget` で変えられる）を超えるとき。サンプリングはせず、その action は確かめず、何も生成しません。",
                "The combinations of an action (with the pairs of today and values its dates are walked over) come to more than the budget (10⁸ by default, `--budget` to change it). The check does not sample: the action is not checked, and nothing is generated."
            ),
            tr!(
                "注に、数を増やしているものが出ます。役割なら、型の役割を分けるか役割ごとに `can` で確かめ、属性や定数なら、条件が読む値を減らしてください。`--budget` で予算を上げることもできます。",
                "The note says what makes the most. For roles, split the roles of a type or check each role with `can`; for attributes and constants, have the conditions read fewer values. `--budget` raises the budget."
            ),
            file(
                "gate shop v1\ndescription \"Fourteen attributes of four values each: more than 10^8 combinations\"\n\nenum level = a | b | c | d\n\nprincipal User\n  description \"A member of the staff\"\n  attributes\n    x1 : level\n    x2 : level\n    x3 : level\n    x4 : level\n    x5 : level\n    x6 : level\n    x7 : level\n    x8 : level\n    x9 : level\n    x10 : level\n    x11 : level\n    x12 : level\n    x13 : level\n    x14 : level\n\nresource Order\n  description \"An order\"\n\naction read_order\n  description \"Look at an order\"\n  principal User\n  resource Order\n\npermit level_a_reads\n  description \"Whoever is at level a everywhere reads\"\n  action read_order\n  when principal.x1 is a and principal.x2 is a and principal.x3 is a and principal.x4 is a and principal.x5 is a and principal.x6 is a and principal.x7 is a\n  when principal.x8 is a and principal.x9 is a and principal.x10 is a and principal.x11 is a and principal.x12 is a and principal.x13 is a and principal.x14 is a\n",
                &[],
            ),
            &[],
        ),
        with_ja(
            Entry::new(
                "W304",
                tr!("期待が、どの組み合わせも選びません", "An expectation picks no combination"),
                tr!(
                    "`expect` の `principal`・`when`・`unless` の行が、その action のどの組み合わせでも、そろって成り立たないとき。期待は成り立ちますが、何も確かめていません（たいていは行の書き違いです）。規則が一緒には出さない値や、ゲートが渡す値には規則が答えない値を選んでいるときは、その値と、何から計算するかも言います。それは規則が決めていることで、ポリシーをどう書いても変わりません。",
                    "The `principal`, `when` and `unless` lines of an `expect` hold together in no combination of its actions: the expectation holds, and checks nothing (most often a line is written wrong). Where the lines pick values the rules never give together, or a value a rule answers for nothing the gate gives it, the values and what they are computed from are said: the rules decide that whatever the policies say."
                ),
                tr!(
                    "同時に満たせない行を直すか、期待を消してください。規則が出さない値を選んでいるなら、確かめたいことは規則の側（`examples`）に書いてください。",
                    "Correct the lines that cannot be met together, or remove the expectation. Where they pick values the rules do not give, write what is to be held in the rule (its `examples`)."
                ),
                file(
                    r#"gate shop v1
description "A shop's orders"

enum order_status = paid | refunded

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"
  attributes
    status : order_status

action refund_order
  description "Refund an order"
  principal User
  resource Order

permit clerks_refund
  description "A clerk refunds an order not yet refunded"
  principal in clerk
  action refund_order
  unless resource.status is refunded

expect deny nothing_paid_and_refunded_is_refunded
  description "No one refunds an order that is paid and refunded at once"
  action refund_order
  when resource.status is paid
  when resource.status is refunded
"#,
                    &[],
                ),
                &["E304", "E303"],
            ),
            file(
                r#"gate 店(shop) v1
description "店の注文"

enum 注文の状態(order_status) = 支払済(paid) | 返金済(refunded)

role 係(clerk)
  description "お客さまに応対する"

principal 職員(User)
  description "店の職員"
  roles 係

resource 注文(Order)
  description "店の注文"
  attributes
    状態(status) : 注文の状態

action 返金する(refund_order)
  description "注文を返金する"
  principal 職員
  resource 注文

permit 係は返金できる(clerks_refund)
  description "係は、まだ返金していない注文を返金できる"
  principal in 係
  action 返金する
  unless resource.状態 is 返金済

expect deny 支払済で返金済の注文は返金しない
  description "支払済でもあり返金済でもある注文を、返金する人はいない"
  action 返金する
  when resource.状態 is 支払済
  when resource.状態 is 返金済
"#,
                &[],
            ),
        ),
        with_ja(
            Entry::new(
                "W305",
                tr!("ポリシーが、挙げた action の一つに一度も当てはまりません", "A policy never applies to an action it names"),
                tr!(
                    "action を名前で挙げたポリシー（`action any` でないもの）が、挙げたほかの action には当てはまり、一つの action にはどの組み合わせでも当てはまらないとき。その action では、`principal`・`when`・`unless` の行がそろって成り立ちません（たいていは、その action の型に無い属性や、その action では来ない値を読む行です）。数えるのは起こりうる組み合わせだけで、多めに数えた組み合わせが残るときは言いません。`action any` のポリシーが一部の action に当てはまらないのは普通のことなので、言いません。",
                    "A policy that names its actions (not `action any`) applies to some of them, and to one of them in no combination: there its `principal`, `when` and `unless` lines never hold together (most often a line reads what that action's types do not have, or a value that never comes with it). Only the combinations that can occur are counted, and where a combination counted though it may not occur is left, nothing is said. A policy of `action any` that applies to some actions only is as it should be, and is not named."
                ),
                tr!(
                    "`action` の行からその action を外すか、その action にも当てはまるよう行を直してください。",
                    "Remove the action from the policy's `action` line, or correct its lines so that it applies to that action too."
                ),
                file(
                    r#"gate library v1
description "Who may read a document of the library and list a folder; a forbid names an action it never applies to"
namespace Library

role staff
  description "Works in the library"

principal User
  description "A member of the staff"
  roles staff
  attributes
    suspended : bool

resource Doc
  description "A document"
  attributes
    open_to_all : bool

resource Folder
  description "A folder of documents"

action read_doc
  description "Read a document"
  principal User
  resource Doc

action list_folder
  description "List what a folder holds"
  principal User
  resource Folder

permit staff_read_and_list
  description "Staff read documents and list folders"
  principal in staff
  action read_doc, list_folder

forbid suspended_staff_see_nothing_open
  description "A suspended member of the staff sees nothing that is open to all"
  principal in staff
  action read_doc, list_folder
  when resource.open_to_all
  when principal.suspended
"#,
                    &[],
                ),
                &["W301", "W304"],
            ),
            file(
                r#"gate 図書室(library_ja) v1
description "図書室の文書を読むこと、棚の中身を見ることを、だれがしてよいか。forbid が、一度も当てはまらない action を挙げている"
namespace LibraryJa

role 職員(staff)
  description "図書室で働く"

principal 利用者(User)
  description "図書室の職員"
  roles 職員
  attributes
    停止中(suspended) : bool

resource 文書(Doc)
  description "文書"
  attributes
    だれでも読める(open_to_all) : bool

resource 棚(Folder)
  description "文書を入れた棚"

action 文書を読む(read_doc)
  description "文書を読む"
  principal 利用者
  resource 文書

action 棚を見る(list_folder)
  description "棚の中身を見る"
  principal 利用者
  resource 棚

permit 職員は読めて見られる(staff_read_and_list)
  description "職員は文書を読め、棚の中身を見られる"
  principal in 職員
  action 文書を読む, 棚を見る

forbid 停止中の職員はだれでも読める物を見ない(suspended_staff_see_nothing_open)
  description "停止中の職員は、だれでも読める物を見ない"
  principal in 職員
  action 文書を読む, 棚を見る
  when resource.だれでも読める
  when principal.停止中
"#,
                &[],
            ),
        ),
        with_ja(
            Entry::new(
                "W306",
                tr!("permit が、挙げた action の一つでは何も許しません", "A permit allows nothing of an action it names"),
                tr!(
                    "action を名前で挙げた permit（`action any` でないもの）が、挙げたほかの action では何かを許し、一つの action では、当てはまる組み合わせをどれも forbid が拒むとき。その action では、permit が無くても答えは変わりません（E302 を action ごとにした形です）。数えるのは起こりうる組み合わせで、多めに数えた組み合わせも許す側に数えます。その action で permit が当てはまる組み合わせのうち、起こると確かめられるものが無いときは言いません。",
                    "A permit that names its actions (not `action any`) allows something of some of them, and of one of them nothing: a forbid denies every combination the permit applies to there, so the permit changes no answer of that action (E302 for one action). Only the combinations that can occur are counted, and one counted though it may not occur is counted as one the permit may allow. Where none of the combinations the permit applies to there is shown to occur, nothing is said."
                ),
                tr!(
                    "`action` の行からその action を外すか、その action も許すつもりなら forbid を狭めてください。",
                    "Remove the action from the permit's `action` line, or narrow the forbid if the permit is meant to allow that action."
                ),
                file(
                    r#"gate library v1
description "Who may read a document of the library and remove it; a permit names an action a forbid keeps from everyone"
namespace Library

role staff
  description "Works in the library"

principal User
  description "A member of the staff"
  roles staff

resource Doc
  description "A document"

action read_doc
  description "Read a document"
  principal User
  resource Doc

action purge_doc
  description "Remove a document for good"
  principal User
  resource Doc
  nobody "documents are kept"

permit staff_read_and_purge
  description "Staff read documents, and would remove them"
  principal in staff
  action read_doc, purge_doc

forbid no_one_purges
  description "Documents are kept"
  action purge_doc
"#,
                    &[],
                ),
                &["E302", "W305"],
            ),
            file(
                r#"gate 図書室(library_ja) v1
description "図書室の文書を読むこと、消すことを、だれがしてよいか。permit が、forbid がだれにも許さない action を挙げている"
namespace LibraryJa

role 職員(staff)
  description "図書室で働く"

principal 利用者(User)
  description "図書室の職員"
  roles 職員

resource 文書(Doc)
  description "文書"

action 文書を読む(read_doc)
  description "文書を読む"
  principal 利用者
  resource 文書

action 文書を消す(purge_doc)
  description "文書をすっかり消す"
  principal 利用者
  resource 文書
  nobody "文書は残す"

permit 職員は読めて消せる(staff_read_and_purge)
  description "職員は文書を読め、消すこともできるはずだった"
  principal in 職員
  action 文書を読む, 文書を消す

forbid だれも消せない(no_one_purges)
  description "文書は残す"
  action 文書を消す
"#,
                &[],
            ),
        ),
        with_ja(
            Entry::new(
                "W307",
                tr!("permit の行が、どの決定も変えません", "A line of a permit changes no decision"),
                tr!(
                    "permit の `when`・`unless` の行が外す組み合わせ（ほかの行は満たされ、その行だけが満たされないもの）を、どれも forbid が拒むとき。行を消すと permit はその組み合わせにも当てはまりますが、forbid が拒むので、答えは変わりません（W301 を行ごとにした形です）。数えるのは起こりうる組み合わせで、多めに数えた組み合わせも行が外す側に数えます。行が外す組み合わせをほかの permit が許すときは言いません（場合ごとに書いた permit では、その行が、どの場合の permit かを言います）。forbid の行と、W301 が言う permit の行も言いません。同じポリシーの二つの行が、一つずつなら何も変えず、一緒に消すと答えを変えうるときは、それぞれの注でそう言います。",
                    "Every combination a `when` or `unless` line of a permit leaves out (its other lines met, this one not) is one a forbid denies: without the line, the permit would apply to them too, and the forbid would still deny them, so no answer changes (W301 for one line). Only the combinations that can occur are counted, and one counted though it may not occur is counted as one the line leaves out. A line whose combinations another permit allows is not named (of permits written case by case, the line says which case the permit is), nor a line of a forbid, nor one of a permit W301 names. Where two lines of one policy change nothing taken out one at a time but may change an answer taken out together, each says so."
                ),
                tr!(
                    "行を消してください。ほかの組み合わせを外すつもりだったなら、行を直してください。",
                    "Remove the line, or correct it if it is meant to leave out something else."
                ),
                file(
                    r#"gate library v1
description "Who may index a document of the library; a line of a permit leaves out only what a forbid denies"
namespace Library

principal Service
  description "A program that runs on its own"

resource Doc
  description "A document"
  attributes
    open_to_all : bool

action index_doc
  description "Index a document"
  principal Service
  resource Doc

permit services_index
  description "Services index the documents open to all"
  principal is Service
  action index_doc
  when resource.open_to_all

forbid services_skip_closed_docs
  description "A service leaves alone a document that is not open to all"
  principal is Service
  action index_doc
  unless resource.open_to_all
"#,
                    &[],
                ),
                &["W301", "W308"],
            ),
            file(
                r#"gate 図書室(library_ja) v1
description "図書室の文書の索引を、だれが作ってよいか。permit の行が外すのは、forbid が拒む組み合わせだけ"
namespace LibraryJa

principal サービス(Service)
  description "ひとりでに動くプログラム"

resource 文書(Doc)
  description "文書"
  attributes
    公開(open_to_all) : bool

action 索引を作る(index_doc)
  description "文書の索引を作る"
  principal サービス
  resource 文書

permit サービスは索引を作れる(services_index)
  description "サービスは、公開の文書の索引を作れる"
  principal is サービス
  action 索引を作る
  when resource.公開

forbid サービスは非公開の文書に触れない(services_skip_closed_docs)
  description "サービスは、公開でない文書には触れない"
  principal is サービス
  action 索引を作る
  unless resource.公開
"#,
                &[],
            ),
        ),
        with_ja(
            Entry::new(
                "W308",
                tr!("ポリシーの行が、起こりうるどの組み合わせも外しません", "A line of a policy leaves out no combination that can occur"),
                tr!(
                    "permit か forbid の `when`・`unless` の行が、ほかの行が満たされる組み合わせのどれでも満たされるとき。その行が効くのは、起こらない組み合わせだけです（ほかの行が先に外したもの、規則が出さない値、など）。数えるのは起こりうる組み合わせで、多めに数えた組み合わせも行が外す側に数えるので、言うのは、そう数えても外す組み合わせが無いときだけです。規則が決めていることなら、注がどの値がなぜ来ないかを言います。同じポリシーの二つの行が、一つずつなら何も変えず、一緒に消すと答えを変えうるときは、それぞれの注でそう言います。",
                    "A `when` or `unless` line of a permit or a forbid is met on every combination its other lines are met on: it acts only on combinations that cannot occur (ones its other lines leave out already, a value a rule never gives, …). Only the combinations that can occur are counted, and one counted though it may not occur is counted as one the line leaves out, so it is said only where even so the line leaves out none. Where the rules decide it, a note says which value never comes, and why. Where two lines of one policy change nothing taken out one at a time but may change an answer taken out together, each says so."
                ),
                tr!(
                    "行を消すか、外すつもりの組み合わせを外すよう行を直してください。規則が決めていることなら、規則の答えは規則の側（`examples`）で確かめてください。",
                    "Remove the line, or correct it so that it leaves out what it is meant to. Where a rule decides it, hold what the rule answers in the rule (its `examples`)."
                ),
                file(
                    r#"gate shop v1
description "A shop's orders; a line of a permit is met wherever its other line is"

enum status = paid | refunded | returned

role clerk
  description "Answers customers"

principal User
  description "A member of the staff"
  roles clerk

resource Order
  description "An order"
  attributes
    status : status

action refund_order
  description "Refund an order"
  principal User
  resource Order

permit clerks_refund_paid_orders
  description "A clerk refunds a paid order that is not refunded yet"
  principal in clerk
  action refund_order
  when resource.status is paid
  unless resource.status is refunded
"#,
                    &[],
                ),
                &["E303", "W304", "W307"],
            ),
            file(
                r#"gate 店(shop) v1
description "店の注文。permit の行が、ほかの行が満たされるところで、いつも満たされる"

enum 注文の状態(order_status) = 支払済(paid) | 返金済(refunded) | 返品済(returned)

role 係(clerk)
  description "お客さまに応対する"

principal 職員(User)
  description "店の職員"
  roles 係

resource 注文(Order)
  description "店の注文"
  attributes
    状態(status) : 注文の状態

action 返金する(refund_order)
  description "注文を返金する"
  principal 職員
  resource 注文

permit 係は支払済の注文を返金できる(clerks_refund_paid_orders)
  description "係は、支払済で、まだ返金していない注文を返金できる"
  principal in 係
  action 返金する
  when resource.状態 is 支払済
  unless resource.状態 is 返金済
"#,
                &[],
            ),
        ),
        Entry::new(
            "W301",
            tr!("permit が許すものを、ほかの一つの permit が全部許します", "Another single permit allows everything a permit allows"),
            tr!(
                "permit が許す組み合わせを、どれもほかの一つの permit も許すとき。消しても答えは変わりません。二つが互いを覆うときは、後に書いたほうにだけ言います。",
                "Every combination a permit allows, another single permit allows too: removing it changes no answer. Of two that cover each other, only the one written later is named."
            ),
            tr!("その permit を消すか、そちらが意図どおりなら、覆うほうの permit を狭めてください。", "Remove the permit, or narrow the one that covers it if the first says what is meant."),
            file(
                "gate shop v1\ndescription \"A shop's orders\"\n\nenum status = paid | refunded\n\nrole clerk\n  description \"Answers customers\"\n\nprincipal User\n  description \"A member of the staff\"\n  roles clerk\n\nresource Order\n  description \"An order\"\n  attributes\n    status : status\n\naction refund_order\n  description \"Refund an order\"\n  principal User\n  resource Order\n\npermit clerks_refund\n  description \"A clerk refunds\"\n  principal in clerk\n  action refund_order\n\npermit clerks_refund_paid_orders\n  description \"A clerk refunds a paid order\"\n  principal in clerk\n  action refund_order\n  when resource.status is paid\n",
                &[],
            ),
            &["E302"],
        ),
        Entry::new(
            "W302",
            tr!("役割だけでは、`can` に書いた action を一度も許されません", "A role alone is never allowed an action its `can` lists"),
            tr!("その役割（と、それが includes する役割）だけを持つ principal が、`can` に書いた action を、どの組み合わせでも許されないとき。", "A principal holding the role alone (and the roles it includes) is denied, in every combination, an action its `can` line lists."),
            tr!("`can` から action を消すか、許す permit を書いてください。", "Remove the action from `can`, or write the permit that allows it."),
            file(
                "gate shop v1\ndescription \"A shop's orders\"\n\nrole clerk\n  description \"Answers customers\"\n\nrole auditor\n  description \"Checks the refunds\"\n  can refund_order\n\nprincipal User\n  description \"A member of the staff\"\n  roles clerk, auditor\n\nresource Order\n  description \"An order\"\n\naction refund_order\n  description \"Refund an order\"\n  principal User\n  resource Order\n\npermit clerks_refund\n  description \"A clerk refunds\"\n  principal in clerk\n  action refund_order\n",
                &[],
            ),
            &["E306"],
        ),
        Entry::new(
            "W303",
            tr!("起こるかどうかを確かめられない値に答えがかかっていて、決められません", "An answer rests on a value no language can vouch for, and cannot be decided"),
            tr!(
                "規則が、問われた範囲で出力がとりうる値を正確に言えず（数を計算する derive の上の表など）、多めに数えた値から出た答えを、具体的な入力で起こせないとき。読んだ Cedar に有限でない式があるときも出ます。",
                "A rule cannot say exactly what its output comes to over the ranges asked (a table over derived values whose rows whole numbers may not reach), and an answer that comes from the values counted in case is given by no concrete input tried; or a Cedar policy read has an expression that is not finite."
            ),
            tr!(
                "規則に渡す値の範囲を狭めるか、規則の表を、整数で届く行だけにしてください。",
                "Narrow what is given to the rule, or write the rule's table with rows whole numbers reach."
            ),
            joined(vec![
                (
                    "example.gate",
                    "gate shop v1\ndescription \"A shop's orders\"\n\nuse rule halves from \"halves.rule\"\n\nrole clerk\n  description \"Answers customers\"\n\nprincipal User\n  description \"A member of the staff\"\n  roles clerk\n  attributes\n    first  : number  range >=0 <=3\n    second : number  range >=0 <=3\n\nresource Order\n  description \"An order\"\n\naction read_order\n  description \"Look at an order\"\n  principal User\n  resource Order\n  context\n    kind = halves(first: principal.first, second: principal.second).answer\n\npermit clerks_read\n  description \"A clerk reads\"\n  principal in clerk\n  action read_order\n\npermit halves_read\n  description \"Whoever is at one and a half twice reads\"\n  principal in clerk\n  action read_order\n  when kind is odd_half\n",
                ),
                (
                    "halves.rule",
                    "rule halves v1\ndescription \"A row that asks for a sum of 3 and a difference of 0: real numbers reach it, whole numbers do not\"\n\nenum kind = odd_half | other\n\ninputs\n  first  : number  range >=0 <=3\n  second : number  range >=0 <=3\n\noutputs\n  answer : kind\n\nderive total : number = first + second  range >=0 <=6\nderive gap   : number = first - second  range >=-3 <=3\n\ntable t\npolicy first\n| total | gap | -> answer : kind |\n| 3     | 0   | odd_half         |\n| -     | -   | other            |\n",
                ),
            ]),
            &["W201"],
        ),
    ]
}
