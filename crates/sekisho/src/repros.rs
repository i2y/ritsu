//! The entries of the ledger for the codes of the checks of every combination, the borders and
//! the contracts (E202–E208, W201, E301–E307, W301–W304): what `explain` says of each, and the
//! smallest input that prints it. The ledger itself is `codes.rs`'s; it takes these in place of the
//! entries it writes for later, and puts a code it does not write (W304) after the last of its band.
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
            tr!("注にある操作の名前で書いてください。", "Write one of the operations the note lists."),
            file(shop_with_orders!("action refund_order\n  description \"Refund an order\"\n  guards orders refundOrders\n  principal User\n  resource Order\n\npermit clerks_refund\n  description \"A clerk refunds\"\n  principal in clerk\n  action refund_order\n"), &[ORDERS]),
            &["E203", "E205"],
        ),
        Entry::new(
            "E203",
            tr!("input が、操作の受け取るものと合いません", "An input is not what the operation takes"),
            tr!(
                "`input` の名前が、守る操作の引数にも本文のフィールドにも無いとき。型が違うとき（数を小数や文字列として受け取る、列挙の値が違う）。範囲が操作の受け取る範囲を超えるとき。操作が求めない値に `?` が無いとき。",
                "A name under `input` is neither a parameter nor a field of the body of an operation the action guards; or its type is not what the operation takes (a number taken as one with a fraction or as a string, other values of an enum); or its range goes past what the operation takes; or the operation does not require it and it has no `?`."
            ),
            tr!(
                "操作が受け取るものを、同じ名前、同じ型、その中の範囲で書いてください。",
                "Write what the operation takes, by the same name and type, with a range within the operation's."
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
            tr!("操作のパスかクエリの引数の名前で書いてください。", "Write the name of a parameter of the operation's path or query."),
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
                "計算した値で規則や日付の関数に渡す属性・input・定数の範囲が、その入力の範囲に収まらないとき。渡す値の範囲の中に、規則の前提（`constraint`）を破る例があるとき。",
                "The range of an attribute, an input or a constant a computed value gives to a rule or a date is not within the range of the input it is given to; or, over the ranges given, an example breaks a precondition (`constraint`) of the rule."
            ),
            tr!("渡す値の範囲を、入力の範囲（前提を守る範囲）に狭めてください。", "Narrow the ranges of what is given to the input's, or to ones that keep the precondition."),
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
                "渡す値の範囲で、規則の前提（`constraint`）を守るかを rulec が決められないとき。生成するコードは、走らせたときに前提を確かめ、破ればリクエストを拒みます。",
                "rulec cannot decide whether a precondition (`constraint`) of the rule holds over the ranges given; the generated code checks it when it runs, and denies the request when it breaks."
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
            tr!("ワークフローの `.flow` が dandori の検査を通りません", "A workflow's `.flow` does not pass dandori's check"),
            tr!("`workflow <名前> from \"<.flow>\"` のフローが、dandori の検査を通らないか、読めないとき。", "The flow of `workflow <name> from \"<.flow>\"` does not pass dandori's check, or cannot be read."),
            tr!("フローを dandori の検査が通るように直してください。注に、dandori の言うことがあります。", "Fix the flow until dandori's check passes; the notes say what dandori says."),
            joined(vec![
                (
                    "example.gate",
                    "gate shop v1\ndescription \"A shop's orders\"\n\nworkflow returns from \"returns.flow\"\n  description \"Refunds a returned order\"\n\nresource Order\n  description \"An order\"\n\naction refund_order\n  description \"Refund an order\"\n  principal Workflow\n  resource Order\n\npermit returns_refunds\n  description \"The returns workflow refunds\"\n  principal is workflow returns\n  action refund_order\n",
                ),
                ("returns.flow", "workflow returns v1\ndescription \"Calls a task it does not declare\"\n\ninputs\n  order_id : string\n\noutputs\n  refund : string\n\nflow\n  let r = refund_order(orderId: order_id)\n  succeed refund = r.id\n"),
            ]),
            &["E201"],
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
            tr!("ポリシーの `principal` の行と `when`・`unless` の行が、どの組み合わせでもそろって成り立たないとき。", "The `principal` line and the `when` and `unless` lines of a policy never hold together, in any combination."),
            tr!("満たせない行か、ポリシーそのものを消してください。", "Remove the line that cannot be met, or the policy itself."),
            file(
                "gate shop v1\ndescription \"A shop's orders\"\n\nrole clerk\n  description \"Answers customers\"\n\nprincipal User\n  description \"A member of the staff\"\n  roles clerk\n  attributes\n    suspended : bool\n\nresource Order\n  description \"An order\"\n\naction refund_order\n  description \"Refund an order\"\n  principal User\n  resource Order\n\npermit clerks_refund\n  description \"A clerk refunds\"\n  principal in clerk\n  action refund_order\n\nforbid suspended_and_not\n  description \"Asks for a suspended user who is not suspended\"\n  action refund_order\n  when principal.suspended\n  unless principal.suspended\n",
                &[],
            ),
            &["E302"],
        ),
        Entry::new(
            "E304",
            tr!("期待が成り立ちません", "An expectation does not hold"),
            tr!("`expect allow` か `expect deny` が選ぶ組み合わせのうち、期待と違う答えになるものがあるとき。成り立たない数と、一つの例を示します。", "A combination an `expect allow` or `expect deny` picks is answered otherwise: how many, and one of them, are shown. So is an action under `nobody` that some combination allows."),
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
                    "`expect` の `principal`・`when`・`unless` の行が、その action のどの組み合わせでも、そろって成り立たないとき。期待は成り立ちますが、何も確かめていません（たいていは行の書き違いです）。",
                    "The `principal`, `when` and `unless` lines of an `expect` hold together in no combination of its actions: the expectation holds, and checks nothing (most often a line is written wrong)."
                ),
                tr!("同時に満たせない行を直すか、期待を消してください。", "Correct the lines that cannot be met together, or remove the expectation."),
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
