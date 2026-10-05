//! The ledger of ritsu's own codes (DESIGN 4.3, 7.1): what `ritsu check` says itself, apart from
//! what each language says in its own codes. `ritsu explain` reads it, `docs/codes.md` and
//! `docs/codes.ja.md` are its Markdown, and a test lays out every reproduction — the files of a
//! small project — runs `ritsu check .` on it, and requires the code to come out, so a
//! reproduction cannot go stale while the prose around it still reads well. The entries are
//! ritsu's; how they are written out is ritsu-base's ([`ritsu_base::ledger`]).
//!
//! The numbers go in bands: E1xx for the files of a project as ritsu reads them, E2xx for the
//! checks of the borders between the languages (DESIGN 7.2). The reproductions are small English
//! projects. A code that is retired keeps its entry, and its number is given to nothing else
//! (DESIGN 7.10).

use ritsu_base::ledger::{Entry, Ledger, Repro};
use ritsu_base::tr;

/// The command a reproduction is run with: in the directory the files are laid out in.
const CHECK: [&str; 3] = ["ritsu", "check", "."];

/// The rule of the reproductions of E201 and W201: a refund never asks for more than was paid.
const REFUND_RULE: &str = "rule refund_check v1\ndescription \"Whether a refund is paid at once or reviewed. A refund never asks for more than was paid, which the rule takes for granted\"\n\nenum path = at_once | review\n\ninputs\n  paid  : number  range >=0 <=10000\n  asked : number  range >=0 <=10000\n\nconstraint asked <= paid\n\noutputs\n  route : path\n\ntable pick\npolicy unique\n| asked | -> route : path |\n| <=100 | at_once         |\n| >100  | review          |\n";

/// The flow of E201: what the customer asks for can be more than was paid.
const REFUND_FLOW: &str = "workflow refund v1\ndescription \"Pays a refund back at once or sends it to review, as the rule decides\"\n\nuse rule check from \"refund_check.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:refund-check\"\n\ninputs\n  order : string\n  paid  : int  range >=0 <=10000\n\ntask ask_amount(order: string) -> int range >=0 <=10000\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:ask-amount\"\n  idempotent\n\ntask pay_back(order: string, amount: int range >=0 <=10000)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:pay-back\"\n  key\n\nflow\n  let asked = ask_amount(order: order)\n  let decision = check(paid: paid, asked: asked)\n  match decision.route\n    at_once => pay_back(order: order, amount: asked)\n    review => pass\n";

/// The flow of W201: nothing says what range the amount asked for is in.
const REFUND_FLOW_OPEN: &str = "workflow refund v1\ndescription \"Pays a refund back at once or sends it to review, as the rule decides\"\n\nuse rule check from \"refund_check.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:refund-check\"\n\ninputs\n  order : string\n  paid  : int  range >=0 <=10000\n\ntask ask_amount(order: string) -> int\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:ask-amount\"\n  idempotent\n\ntask pay_back(order: string, amount: int range >=0 <=10000)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:pay-back\"\n  key\n\nflow\n  let asked = ask_amount(order: order)\n  let decision = check(paid: paid, asked: asked)\n  match decision.route\n    at_once => pay_back(order: order, amount: asked)\n    review => pass\n";

/// The dates file of X3 (a) and X6: the 10th of the month after a closing on the 20th, over the receipts of 2026.
const TERMS_CAL: &str = "dates payment_terms v1\ndescription \"Closes on the 20th and pays on the 10th of the next month. No calendar, so the day it pays on is always the 10th\"\n\ninputs\n  received : date  range >=2026-01-01 <=2026-12-20\n\ndate closing = received\n  close day 20          # closes on the 20th\n\ndate payment = closing\n  day 10 of month +1    # pays on the 10th of the next month\n";

/// The rule of X3 (a) for E202: its range of days starts after the first payment day.
const BATCH_RULE: &str = "rule batch v1\ndescription \"The billing batch a payment day falls in\"\n\nenum run = spring | autumn\n\ninputs\n  pay_day : date  range >=2026-03-01 <=2027-01-31\n\noutputs\n  batch : run\n\ntable pick\npolicy unique\n| pay_day      | -> batch : run |\n| <=2026-08-31 | spring         |\n| >=2026-09-01 | autumn         |\n";

/// The rule of X3 (a) for W202: its range holds every payment day.
const BATCH_RULE_WIDE: &str = "rule batch v1\ndescription \"The billing batch a payment day falls in\"\n\nenum run = spring | autumn\n\ninputs\n  pay_day : date  range >=2026-02-01 <=2027-01-31\n\noutputs\n  batch : run\n\ntable pick\npolicy unique\n| pay_day      | -> batch : run |\n| <=2026-08-31 | spring         |\n| >=2026-09-01 | autumn         |\n";

/// The flow of X3 (a) for E202: the payment day goes to the rule.
const BILLING_FLOW: &str = "workflow billing v1\ndescription \"Bills an order in the batch the rule picks for the day its payment is due\"\n\nuse dates terms from \"payment_terms.cal\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:payment-terms\"\nuse rule batch from \"batch.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:batch\"\n\ninputs\n  order    : string\n  received : date\n\ntask bill(order: string, due: date, run: batch.run)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:bill\"\n  key\n\nflow\n  let due = terms.payment(received: received)\n  let pick = batch(pay_day: due.day)\n  bill(order: order, due: due.day, run: pick.batch)\n";

/// The flow of W202: the day the rule is given can also be the day received, which says nothing of what day it is.
const BILLING_FLOW_EITHER: &str = "workflow billing v1\ndescription \"Bills an order in the batch the rule picks for the day its payment is due\"\n\nuse dates terms from \"payment_terms.cal\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:payment-terms\"\nuse rule batch from \"batch.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:batch\"\n\ninputs\n  order    : string\n  received : date\n  at_once  : bool\n\ntask bill(order: string, due: date, run: batch.run)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:bill\"\n  key\n\nflow\n  let due = terms.payment(received: received)\n  match at_once\n    true => let day = received\n    false => let day = due.day\n  let pick = batch(pay_day: day)\n  bill(order: order, due: day, run: pick.batch)\n";

/// The rule of X4: the seats an event of a kind needs.
const SEATS_RULE: &str = "rule seats v1\ndescription \"How many seats an event of a kind needs\"\n\nenum kind = workshop | talk\n\ninputs\n  event_kind : kind\n\noutputs\n  needed : number  round down(1)\n\ntable pick\npolicy unique\n| event_kind | -> needed : number |\n| workshop   | 30                 |\n| talk       | 80                 |\n";

/// The rule of E203: a handback gives seats back, and a negative amount is none chobo takes (chobo
/// takes 0 to 2⁶³ − 1; an amount of 0 moves nothing).
const SEATS_RULE_HANDBACK: &str = "rule seats v1\ndescription \"How many seats an event of a kind needs; a handback gives seats back\"\n\nenum kind = handback | workshop | talk\n\ninputs\n  event_kind : kind\n\noutputs\n  needed : number  round down(1)\n\ntable pick\npolicy unique\n| event_kind | -> needed : number |\n| handback   | -20                |\n| workshop   | 30                 |\n| talk       | 80                 |\n";

/// The rule of E204: a concert needs more seats than the hall holds.
const SEATS_RULE_CONCERT: &str = "rule seats v1\ndescription \"How many seats an event of a kind needs\"\n\nenum kind = workshop | talk | concert\n\ninputs\n  event_kind : kind\n\noutputs\n  needed : number  round down(1)\n\ntable pick\npolicy unique\n| event_kind | -> needed : number |\n| workshop   | 30                 |\n| talk       | 80                 |\n| concert    | 400                |\n";

/// The book of X4: the hall holds 300, and an event is given its seats at once.
const HALL_BOOK: &str = "book hall v1\ndescription \"The seats of the hall. An event is given the seats it needs at once, and the hall holds 300\"\n\nunit seat\n\naccount given(event: string) : seat\n  description \"the seats an event is given\"\n  at least 0 refused as not_given\n  at most 300 refused as over_capacity\naccount venue : seat outside\n\ntransfer assign(event: string, count: seat)\n  key event\n  move count from venue to given(event)\n\ntransfer release(event: string, count: seat)\n  key event\n  move count from given(event) to venue\n";

/// The flow of X4: the seats the rule says go to the transfer.
const BOOKING_FLOW: &str = "workflow booking v1\ndescription \"Gives an event the seats its kind needs, all at once\"\n\nuse rule seats from \"seats.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:seats\"\nuse book hall from \"hall.book\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:hall\"\n\ninputs\n  event : string\n  kind  : seats.kind\n\ntask give_seats(event: string, count: int)\n  book hall.assign.do\n\nflow\n  let need = seats(event_kind: kind)\n  give_seats(event: event, count: need.needed)\n";

/// The flow of W203: the seats can also come from an answer with no range.
const BOOKING_FLOW_ASKED: &str = "workflow booking v1\ndescription \"Gives an event the seats its kind needs, all at once\"\n\nuse rule seats from \"seats.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:seats\"\nuse book hall from \"hall.book\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:hall\"\n\ninputs\n  event : string\n  kind  : seats.kind\n\ntask give_seats(event: string, count: int)\n  book hall.assign.do\n\ntask ask_organiser(event: string) -> int\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:ask-organiser\"\n  idempotent\n\nflow\n  let need = seats(event_kind: kind)\n  let count = need.needed\n  match kind\n    workshop => let count = ask_organiser(event: event)\n    talk => pass\n  give_seats(event: event, count: count)\n";

/// The flow of W204: the task handles the hall being full, which no kind of event comes to.
const BOOKING_FLOW_HANDLES: &str = "workflow booking v1\ndescription \"Gives an event the seats its kind needs, all at once\"\n\nuse rule seats from \"seats.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:seats\"\nuse book hall from \"hall.book\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:hall\"\n\ninputs\n  event : string\n  kind  : seats.kind\n\ntask give_seats(event: string, count: int)\n  book hall.assign.do\n  errors over_capacity\n\nflow\n  let need = seats(event_kind: kind)\n  give_seats(event: event, count: need.needed)\n    on over_capacity => fail OverCapacity \"the hall cannot seat the event\"\n";

/// The dates file of E205: a reminder a week before a day, whose range ends before the last payment day.
const REMINDERS_CAL: &str = "dates reminders v1\ndescription \"A reminder a week before a payment is due\"\n\ninputs\n  due : date  range >=2026-02-01 <=2026-12-31\n\ndate reminder = due\n  - 7 days              # a week before\n";

/// The dates file of W205: the same, over every payment day.
const REMINDERS_CAL_WIDE: &str = "dates reminders v1\ndescription \"A reminder a week before a payment is due\"\n\ninputs\n  due : date  range >=2026-02-01 <=2027-01-31\n\ndate reminder = due\n  - 7 days              # a week before\n";

/// The flow of X6: the payment day goes to the reminder.
const REMINDING_FLOW: &str = "workflow reminding v1\ndescription \"Reminds a customer a week before the payment of an order is due\"\n\nuse dates terms from \"payment_terms.cal\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:payment-terms\"\nuse dates reminders from \"reminders.cal\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:reminders\"\n\ninputs\n  order    : string\n  received : date\n\ntask remind(order: string, on: date)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:remind\"\n  key\n\nflow\n  let due = terms.payment(received: received)\n  let note = reminders.reminder(due: due.day)\n  remind(order: order, on: note.day)\n";

/// The calendar of X5: open Monday to Friday, in UTC.
const WEEKDAYS_CAL: &str = "calendar weekdays v1\ndescription \"A business that keeps its days in UTC, open Monday to Friday\"\noffset +00:00\n\nclosed weekly sat, sun\n";

/// The dates file of X5: the payment day, at 09:00, on the business day before when the 10th is closed.
const TERMS_AT_CAL: &str = "dates payment_terms v1\ndescription \"Closes on the 20th and pays on the 10th of the next month at 09:00, or on the business day before when that day is closed\"\nuse calendar \"weekdays.cal\"\n\ninputs\n  received : date  range >=2026-01-01 <=2026-12-20\n\ndate closing = received\n  close day 20          # closes on the 20th\n\ndate payment = closing\n  day 10 of month +1    # pays on the 10th of the next month\n  roll preceding        # or on the business day before\n  at 09:00\n";

/// The book of E206: an order holds its goods for 14 days at most.
const STOCK_BOOK: &str = "book stock v1\ndescription \"Stock per SKU. An order holds what it takes for 14 days at most; shipping posts the hold\"\n\nunit pcs\n\naccount shelf(sku: string) : pcs\n  description \"what is on the shelves\"\n  at least 0 refused as out_of_stock\naccount suppliers : pcs outside\naccount customers : pcs outside\n\ntransfer receive(delivery: string, sku: string, qty: pcs)\n  key delivery, sku\n  move qty from suppliers to shelf(sku)\n\ntransfer reserve(order: string, sku: string, qty: pcs)\n  description \"posted when the order ships\"\n  key order, sku\n  pending expires after 14 days\n  move qty from shelf(sku) to customers\n";

/// The book of W206: an order holds its goods for 60 days at most.
const STOCK_BOOK_LONG: &str = "book stock v1\ndescription \"Stock per SKU. An order holds what it takes for 60 days at most; shipping posts the hold\"\n\nunit pcs\n\naccount shelf(sku: string) : pcs\n  description \"what is on the shelves\"\n  at least 0 refused as out_of_stock\naccount suppliers : pcs outside\naccount customers : pcs outside\n\ntransfer receive(delivery: string, sku: string, qty: pcs)\n  key delivery, sku\n  move qty from suppliers to shelf(sku)\n\ntransfer reserve(order: string, sku: string, qty: pcs)\n  description \"posted when the order ships\"\n  key order, sku\n  pending expires after 60 days\n  move qty from shelf(sku) to customers\n";

/// The flow of X5: the goods are held until the payment is due, then shipped.
const INVOICE_FLOW: &str = "workflow invoice v1\ndescription \"Holds an order's goods until its payment is due, then ships them\"\n\nuse dates terms from \"payment_terms.cal\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:payment-terms\"\nuse book stock from \"stock.book\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:stock\"\n\ninputs\n  order : string\n  sku   : string\n  qty   : int  range >=1 <=100\n\ntask reserve(order: string, sku: string, qty: int) -> stock.reserve\n  book stock.reserve.hold\n  starts stock.reserve\n  errors out_of_stock\n\ntask ship(order: string, sku: string) -> stock.reserve\n  book stock.reserve.post\n  sends post\n  errors expired\n\ncase goods : stock.reserve follows stock.reserve\n\nflow\n  goods <- reserve(order: order, sku: sku, qty: qty)\n    on out_of_stock => fail OutOfStock \"nothing left on the shelf\"\n  let due = terms.payment(received: now)\n  wait until due.at\n  goods <- ship(order: order, sku: sku)\n    on expired => fail Expired \"the hold expired before the payment was due\"\n";

pub fn ledger() -> Ledger {
    let entries = vec![
        // ── The files of a project ──
        Entry::new(
            "E101",
            tr!(".proto として読めないファイル", "A file that does not read as a .proto"),
            tr!(
                "プロジェクトの `.proto` を、ritsu の共通のパーサー（ritsu-proto）が読めないとき。閉じていない `{{`、`;` の無い文、知らない `syntax`、proto2 の `group`、UTF-8 でないファイルなどです。どの言語もこのパーサーで `.proto` を読むので、このファイルはどの言語からも読めません。そのファイルを読む言語も、読むところで自分の診断のコードを出します（rulec の E013、dandori の E016、sakai の E106、yuen の E205）。",
                "A `.proto` of the project that ritsu's one reader of `.proto` files (ritsu-proto) cannot read: a `{{` that is never closed, a statement without its `;`, a `syntax` it does not know, a proto2 `group`, a file that is not UTF-8. Every language reads a `.proto` with that reader, so none of them can read the file; a language that reads it says so where it does, in its own code too (rulec's E013, dandori's E016, sakai's E106, yuen's E205)."
            ),
            tr!(
                "示された位置を直し、proto3 の `.proto` にしてください。`buf build` でビルドできるファイルなら、ritsu のパーサーも読めます。",
                "Correct it where it points, as a proto3 `.proto`; a file `buf build` builds, ritsu's reader reads."
            ),
            Repro::Dir { files: vec![("shop.proto", "syntax = \"proto3\";\n\npackage shop.v1;\n\nmessage Order {\n  string id = 1;\n")], command: CHECK.to_vec() },
            &[],
        ),
        // ── The borders between the languages (DESIGN 7.2) ──
        Entry::new(
            "E201",
            tr!("規則を呼ぶところで、前提を破る値を渡すことがあります", "A call can give a rule values that break its precondition"),
            tr!(
                "ワークフローが規則を呼ぶところで、dandori が知っている値の範囲の中に、規則の前提（入力どうしの関係 `constraint`）を破る組み合わせがあるとき。範囲は dandori が値を入れるすべての場所から集めたもので、dandori の E014 と同じ読み方です。範囲を `range from koyomi` にした日付の入力では、渡す値が koyomi の日付の日なら、その日のどれかが規則の日でないときも、このエラーです。前提を破る呼び出しは、規則から生成したコードが入口で受け付けないので、ワークフローを走らせたときに初めて落ちます。注には、値の範囲と、前提を破る組み合わせや日が出ます。",
                "Where a workflow calls a rule, the ranges dandori knows for the values it gives hold a combination that breaks one of the rule's preconditions, a relation between two inputs (`constraint`). The ranges are dandori's, gathered from every place a value comes from, read as dandori's E014 reads them. A date input whose range is `range from koyomi` and is given the day of a koyomi date is this error too when one of those days is not one of the rule's. The rule's generated code refuses such a call at its door, so it would fail only when the workflow runs. The notes give the ranges and the combination or the day that breaks it."
            ),
            tr!(
                "呼ぶ前に前提が保たれるよう分岐するか、値の範囲を狭めてください（入力やタスクの結果の `range`）。前提のほうが業務に合っていないなら、規則の `constraint` を直してください。",
                "Branch so that the precondition holds before the call, or narrow the ranges (the `range` of an input or a task's result). If the precondition is the part that is wrong, correct the rule's `constraint`."
            ),
            Repro::Dir { files: vec![("refund_check.rule", REFUND_RULE), ("refund.flow", REFUND_FLOW)], command: CHECK.to_vec() },
            &["W201"],
        ),
        Entry::new(
            "W201",
            tr!("規則を呼ぶところで、前提が保たれるかを決められません", "Whether a call keeps a rule's precondition cannot be decided"),
            tr!(
                "ワークフローが規則を呼ぶところで、前提が保たれるかを決められないとき。渡す値に範囲の無いところから来るものがある（タスクの結果に `range` が無い、など）、前提が並びの合計や長さの上限である（dandori は並びの長さを知りません）、前提が koyomi の日付の日で、渡す値が何日かを言わないところ（ワークフローの入力、タスクの結果、`now`）からも来る、のどれかです。決められない前提は、`ritsu dandori build` が書くワークフローのコードが実行時に確かめます。値ができたところですぐに確かめ、前提を破る実行を `Dandori.BrokenPrecondition` で失敗させます。",
                "Where a workflow calls a rule, whether a precondition holds cannot be decided: a value comes from a place with no range (a task's result without `range`, say), the precondition bounds the total or the length of a list (dandori knows no list's length), or it is the days of a koyomi date and the value can come from somewhere that says nothing of what day it is (an input of the workflow, a task's answer, `now`). The workflow's code that `ritsu dandori build` writes checks such a precondition when the workflow runs, as soon as the values are made, and fails a run that breaks it with `Dandori.BrokenPrecondition`."
            ),
            tr!(
                "値の来るところに範囲を書いてください（タスクの結果やワークフローの入力の `range`）。範囲を書けないなら、このままで構いません。ワークフローのコードが実行時に確かめます。",
                "Give the place the value comes from a range (the `range` of a task's result or of the workflow's input). Where none can be given, leave it: the workflow's code checks it at run time."
            ),
            Repro::Dir { files: vec![("refund_check.rule", REFUND_RULE), ("refund.flow", REFUND_FLOW_OPEN)], command: CHECK.to_vec() },
            &["E201"],
        ),
        Entry::new(
            "E202",
            tr!("koyomi の日付がとる日が、規則の入力の範囲を外れます", "The days a koyomi date comes to fall outside a rule input's range"),
            tr!(
                "ワークフローが koyomi の日付の日（`due.day`）を規則の日付の入力に渡すとき、koyomi がその日付について数えた日のどれかが、規則が宣言した入力の範囲の外にあるとき。koyomi は入力の範囲のすべてで日付を計算するので、外れる日は例として一つに決まります。注には、その日と、koyomi がその日を返す入力が出ます。範囲を `range from koyomi` にした入力では、日は規則の前提なので、E201 と W201 が確かめます。",
                "Where a workflow gives the day of a koyomi date (`due.day`) to a rule's date input, a day koyomi counts that date coming to lies outside the range the rule declares for the input. koyomi computes the date on every input of its range, so the day outside is an exact example; the notes give it, with the input at which koyomi comes to it. An input whose range is `range from koyomi` takes the days as a precondition of the rule, which E201 and W201 hold the call to."
            ),
            tr!(
                "規則の入力の範囲を広げるか、規則の範囲を `range from koyomi` にして、koyomi の日をそのまま範囲にしてください。",
                "Widen the rule input's range, or make it `range from koyomi`, so that koyomi's days are the range."
            ),
            Repro::Dir { files: vec![("payment_terms.cal", TERMS_CAL), ("batch.rule", BATCH_RULE), ("billing.flow", BILLING_FLOW)], command: CHECK.to_vec() },
            &["W202", "E205"],
        ),
        Entry::new(
            "W202",
            tr!("koyomi の日付がとる日が、規則の入力の範囲に収まるかを決められません", "Whether the days of a koyomi date stay inside a rule input's range cannot be decided"),
            tr!(
                "規則の日付の入力に渡す値が、koyomi の日付の日のほかに、何日かを言わないところ（ワークフローの入力、タスクの結果、`now`）からも来ることがあるとき、または koyomi がその日付の日を数えないとき（入力の組み合わせが確かめる数を超える、途中で計算が止まる入力がある）。規則から生成したコードが、ワークフローを走らせたときに入口で日を確かめます。",
                "The value given to a rule's date input can come from somewhere that says nothing of what day it is (an input of the workflow, a task's answer, `now`) as well as from a koyomi date, or koyomi does not count the days of the date (more input combinations than it checks, or an input where the computation stops). The rule's generated code checks the day at its door when the workflow runs."
            ),
            tr!(
                "値を koyomi の日付の日だけから渡せば、決められるようになります。koyomi が日を数えないなら、koyomi のファイルの入力の範囲を狭めてください。",
                "Give the input days of koyomi dates only, and it can be decided; where koyomi does not count them, narrow the inputs of its file."
            ),
            Repro::Dir { files: vec![("payment_terms.cal", TERMS_CAL), ("batch.rule", BATCH_RULE_WIDE), ("billing.flow", BILLING_FLOW_EITHER)], command: CHECK.to_vec() },
            &["E202"],
        ),
        Entry::new(
            "E203",
            tr!("規則の出力が、chobo の受け取らない額になることがあります", "A rule's output can be an amount chobo does not take"),
            tr!(
                "ワークフローが規則の数の出力を chobo の振替の額に渡すとき、その出力が負か 2⁶³ − 1 を超えることがあるとき（chobo は 0 から 2⁶³ − 1 までを受け取ります）。出力の値は rulec が求めます（表の行に書いた数か、区間の計算から）。chobo は範囲の外の額を、拒否する（業務の結果）のではなく呼び出しの失敗にします。注には、その額になる規則の入力の例（規則のベクタから取ったもの）が出ます。",
                "Where a workflow gives a rule's numeric output to a chobo transfer as its amount, the output can be below 0 or above 2⁶³ − 1 (chobo takes 0 to 2⁶³ − 1). rulec counts what the output comes to (the numbers the rows write, or its intervals). chobo fails such a call rather than refusing it as a business outcome. The notes give an input of the rule that comes to that amount, from the rule's vectors."
            ),
            tr!(
                "規則が返す額を 0 以上にするか（返金などの負の額は、向きの違う振替に分けてください）、振替に渡す前に分岐してください。",
                "Make the rule's amounts 0 or more (a negative one, such as a refund, is a transfer the other way), or branch before the transfer."
            ),
            Repro::Dir { files: vec![("seats.rule", SEATS_RULE_HANDBACK), ("hall.book", HALL_BOOK), ("booking.flow", BOOKING_FLOW)], command: CHECK.to_vec() },
            &["W203", "E204"],
        ),
        Entry::new(
            "W203",
            tr!("規則の出力を、chobo が額として受け取るかを決められません", "Whether chobo takes a rule's output as an amount cannot be decided"),
            tr!(
                "ワークフローが規則の数の出力を振替の額に渡すとき、出力の範囲に上限か下限が無いか、値が範囲の分からないところ（範囲の無いタスクの結果など）からも来ることがあるとき。chobo が受け取らない額なら、ワークフローを走らせたときに呼び出しが失敗します。",
                "Where a workflow gives a rule's numeric output to a transfer as its amount, the output's range has an open end, or the value can also come from somewhere with no range (a task's answer without one, say). An amount chobo does not take fails the call when the workflow runs."
            ),
            tr!(
                "値の来るところに範囲を書いてください（タスクの結果やワークフローの入力の `range`）。",
                "Give the places the value comes from a range (the `range` of a task's answer or of the workflow's input)."
            ),
            Repro::Dir { files: vec![("seats.rule", SEATS_RULE), ("hall.book", HALL_BOOK), ("booking.flow", BOOKING_FLOW_ASKED)], command: CHECK.to_vec() },
            &["E203"],
        ),
        Entry::new(
            "E204",
            tr!("振替が拒否されうる理由を、タスクが処理していません", "The task does not handle a refusal the transfer can come to"),
            tr!(
                "規則の出力を額に渡す `do` か `hold` の呼び出しで、操作が拒否されうる理由を、タスクが宣言したエラーとして処理していないとき。拒否されうる理由とは、額を呼び出しが渡す範囲に限った chobo の探索で、拒否される例が見つかった理由です。比べるのは帳簿の境界の理由（勘定の `refused as`）、つまり額で決まる拒否だけです。同じキーを別の引数で使い直すことのように、前の呼び出しで決まる拒否は比べません。探索は chobo の検査と同じ深さまでたどります。`post` と `void` は仮押さえの状態で拒否され、dandori が案件の状態ごとに確かめます（dandori の E022）。",
                "Where a `do` or a `hold` is given a rule's output as its amount, chobo's search, with the amounts held to the range the call gives, finds a run in which the operation is refused for a reason the task does not handle as an error it declares. Only the reasons of the book's bounds (an account's `refused as`) are compared, the refusals that turn on the amounts; one that turns on the calls made before, such as a key used again with other arguments, is not. The search goes as deep as chobo's check goes. A `post` and a `void` are refused for the state their hold is in, which dandori follows with the case (dandori's E022)."
            ),
            tr!(
                "その理由をタスクのエラーとして宣言し、処理してください。",
                "Declare the reason as an error of the task, and handle it."
            ),
            Repro::Dir { files: vec![("seats.rule", SEATS_RULE_CONCERT), ("hall.book", HALL_BOOK), ("booking.flow", BOOKING_FLOW)], command: CHECK.to_vec() },
            &["E203", "W204"],
        ),
        Entry::new(
            "W204",
            tr!("振替がどの理由で拒否されうるかを決められません", "Which refusals a transfer can come to cannot be decided"),
            tr!(
                "タスクが帳簿の境界の理由を処理しているのに、額を呼び出しが渡す範囲に限った chobo の探索で、その理由で拒否される例が見つからないとき。探索は chobo の検査と同じ深さまでしかたどらないので、起きないと言えるのはその深さまでです。額の範囲が分からないときや、chobo から帳簿の情報を得られないときも、この警告で「決められない」と知らせます。",
                "The task handles a reason of the book's bounds, and chobo's search, with the amounts held to the range the call gives, finds no run that comes to it. The search goes only as deep as chobo's check does, so all it shows is that the reason does not come within that depth. A case that cannot be decided at all (no range for the amounts, a book that does not answer) is this warning too."
            ),
            tr!(
                "起きない理由なら、タスクのエラーから外してください。深い手順でしか起きないなら、そのままで構いません。",
                "If the reason cannot happen, drop it from the task's errors; if it happens only after a longer run, leave it."
            ),
            Repro::Dir { files: vec![("seats.rule", SEATS_RULE), ("hall.book", HALL_BOOK), ("booking.flow", BOOKING_FLOW_HANDLES)], command: CHECK.to_vec() },
            &["E204"],
        ),
        Entry::new(
            "E205",
            tr!("koyomi の日付に渡す日が、入力の範囲を外れます", "A day given to a koyomi date is outside its input's range"),
            tr!(
                "ワークフローが koyomi の日付を呼ぶとき、日付の入力に渡す日が、koyomi の入力の範囲を外れることがあるとき。渡す日がほかの koyomi の日付の日なら、koyomi が数えたその日の全部で確かめます。範囲の中の日なら、カレンダーに問い合わせる日がデータの範囲に収まることを、koyomi の検査が確かめています（koyomi の E203）。注には、外れる日と、koyomi がその日を返す入力が出ます。",
                "Where a workflow calls a koyomi date, the day it gives the date input can be outside the range of that input. A day of another koyomi date is held to it with every day koyomi counts for that date. A day inside the range is one koyomi's own check has held to the data of its calendar wherever it asks the calendar (koyomi's E203). The notes give the day outside and the input at which koyomi comes to it."
            ),
            tr!(
                "koyomi の入力の範囲を広げるか（カレンダーのデータも足してください）、範囲に収まる日を渡してください。",
                "Widen the koyomi input's range (and the calendar's data), or give it a day that stays inside."
            ),
            Repro::Dir { files: vec![("payment_terms.cal", TERMS_CAL), ("reminders.cal", REMINDERS_CAL), ("reminding.flow", REMINDING_FLOW)], command: CHECK.to_vec() },
            &["W205", "E202"],
        ),
        Entry::new(
            "W205",
            tr!("koyomi の日付に渡す日が、範囲に収まるかを決められません", "Whether a day given to a koyomi date stays inside its range cannot be decided"),
            tr!(
                "ワークフローが koyomi の日付の入力に渡す日を、dandori が知らないとき。ワークフローの入力、タスクの結果、`now` から来る日は、何日かを言いません。dandori には、日付の範囲を書く書き方がまだありません。範囲の外の日は、ワークフローを走らせたときに、koyomi が生成したコードが受け付けません。",
                "dandori does not know the day a workflow gives a koyomi date's input: a day that comes from an input of the workflow, a task's answer or `now` says nothing of what day it is, and dandori has no way yet to write the range of a date. koyomi's generated code refuses a day outside its range when the workflow runs."
            ),
            tr!(
                "ほかの koyomi の日付の日を渡せば、決められるようになります。そうでなければ、このままで構いません。範囲の外の日は、koyomi が実行時に受け付けません。",
                "Give it the day of another koyomi date, and it can be decided; otherwise leave it: koyomi refuses at run time."
            ),
            Repro::Dir { files: vec![("payment_terms.cal", TERMS_CAL), ("reminders.cal", REMINDERS_CAL_WIDE), ("reminding.flow", REMINDING_FLOW)], command: CHECK.to_vec() },
            &["E205"],
        ),
        Entry::new(
            "E206",
            tr!("仮押さえの期限が、確定や取消のときには必ず切れています", "A hold has always expired when a call on it comes"),
            tr!(
                "ワークフローが chobo の仮押さえを案件として追い（`case … follows <帳簿>.<振替>`）、確定か取消をするところで、仮押さえを作ってからその呼び出しまでの長さの下限が、有効期限（振替の `pending expires after`）以上のとき。帳簿はどの実行でもその呼び出しを `expired` で拒否し、通ったあとの流れは動きません。長さは dandori がフローの文から数えます。決まった長さの `wait` はその長さです。koyomi の日付の時刻までの `wait until` は、日付の入力に仮押さえのあとで読んだ `now` を渡したものなら、koyomi が数えた入力から日付までの日数の最小と最大と、日付の時刻から出ます。注には、下限と、その下限になる文が出ます。",
                "Where a workflow follows a chobo hold as a case (`case … follows <book>.<transfer>`) and posts or voids it, the fewest seconds from making the hold to the call are at least the hold's expiry (the transfer's `pending expires after`). The book refuses the call with `expired` on every run, and what follows it going through never runs. dandori counts the time from the statements of the flow: a `wait` of a fixed time takes that time, and a `wait until` the time of a koyomi date, whose date input was given `now` read after the hold, takes from the fewest to the most days koyomi counts from the input to the date, at the date's time. The notes give the fewest and the statements that make them up."
            ),
            tr!(
                "仮押さえの期限を延ばすか（振替の `pending expires after`）、もっと早く呼んでください。",
                "Make the hold last longer (the transfer's `pending expires after`), or make the call sooner."
            ),
            Repro::Dir { files: vec![("weekdays.cal", WEEKDAYS_CAL), ("payment_terms.cal", TERMS_AT_CAL), ("stock.book", STOCK_BOOK), ("invoice.flow", INVOICE_FLOW)], command: CHECK.to_vec() },
            &["W206"],
        ),
        Entry::new(
            "W206",
            tr!("仮押さえの期限が、確定や取消のときに切れているかを決められません", "Whether a hold has expired when a call on it comes cannot be decided"),
            tr!(
                "仮押さえを作ってから確定か取消までの長さが、有効期限の前にも後にもなりうるとき、または長さの上限が分からないとき。上限は、仮押さえを作るタスクと、あいだのタスクの `timeout` とリトライの回数から出ます。`timeout` の無いタスク、規則や日付の呼び出し（`.flow` に時間の上限がありません）、上限の分からない `wait until` があれば、上限はありません。期限が切れていれば帳簿が `expired` で拒否し、フローはそれを処理しています（dandori の E022）。期限が切れないと示せたとき（上限が有効期限より短いとき）は、何も出しません。",
                "The time from making a hold to posting or voiding it can fall either side of the hold's expiry, or nothing bounds it. The most comes from the `timeout` and the retries of the task that makes the hold and of the tasks between; a task with no `timeout`, a call of a rule or a date (the flow gives them no limit), or a `wait until` a time nothing bounds leaves no most. An expired hold is refused with `expired`, which the flow handles (dandori's E022). When the call is shown to come before the hold expires (the most is shorter than the expiry), nothing is said."
            ),
            tr!(
                "仮押さえを作るタスクとあいだのタスクに `timeout` を書くと、上限が決まります。書けないなら、このままで構いません。",
                "Give the task that makes the hold, and the tasks between, a `timeout`, and the most is known; where none can be given, leave it."
            ),
            Repro::Dir { files: vec![("weekdays.cal", WEEKDAYS_CAL), ("payment_terms.cal", TERMS_AT_CAL), ("stock.book", STOCK_BOOK_LONG), ("invoice.flow", INVOICE_FLOW)], command: CHECK.to_vec() },
            &["E206"],
        ),
    ];
    Ledger {
        tool: "ritsu",
        example_file: "",
        fence: "",
        repro_heading: tr!("再現", "Reproduction"),
        later_text: tr!("（ritsu はこのコードをまだ出さないので、再現はありません）", "(ritsu does not print this code yet; it has no reproduction)"),
        later_markdown: tr!("ritsu はこのコードをまだ出さないので、再現はありません", "ritsu does not print this code yet; it has no reproduction"),
        entries,
    }
}

/// The codes `ritsu check` can print, in the order of the ledger.
pub fn codes() -> Vec<&'static str> {
    ledger().entries.iter().filter(|e| !e.is_retired()).map(|e| e.code).collect()
}
