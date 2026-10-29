//! The ledger of ritsu's own codes (DESIGN 4.3, 7.1): what `ritsu check` says itself, apart from
//! what each language says in its own codes. `ritsu explain` reads it, `docs/codes.md` and
//! `docs/codes.ja.md` are its Markdown, and a test lays out every reproduction — the files of a
//! small project — runs `ritsu check .` on it, and requires the code to come out, so a
//! reproduction cannot go stale while the prose around it still reads well. The entries are
//! ritsu's; how they are written out is ritsu-base's ([`ritsu_base::ledger`]).
//!
//! The numbers go in bands: E1xx for the files of a project as ritsu reads them, E2xx for the
//! checks of the borders between the languages (DESIGN 7.2), E3xx and W3xx for the modules and
//! their dependencies (DESIGN 17.20), E9xx and W9xx for the checks of security (DESIGN 16) and of
//! authorization (sekisho's DESIGN 4.6, which go on from them). The reproductions are small
//! English projects (the ones of the modules hold their dependencies as paths and as a copy in the
//! module, so none needs a registry); the ones of security and of authorization have their
//! Japanese versions besides ([`Entry::english`]), which `explain` shows in Japanese. A code that
//! is retired keeps its entry, and its number is given to nothing else (DESIGN 7.10).

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

/// The module of E201: it gives the rule the amount asked for as it is, which can be more than was paid.
const REFUND_KN: &str = "module refund v1\ndescription \"Whether a refund is paid at once or reviewed, as the rule decides for the amount the customer asks for\"\n\nuse rule check from \"refund_check.rule\"\n\nfn route(paid: number range >=0 <=10000, asked: number range >=0 <=10000) -> check.path\n  check(paid: paid, asked: asked).route\n";

/// The module of W201: the amount asked for, capped at what was paid. kaname's ranges do not show
/// that it keeps the precondition, and no run breaks it.
const REFUND_KN_CAPPED: &str = "module refund v1\ndescription \"Whether a refund is paid at once or reviewed, as the rule decides for the amount asked for, capped at what was paid\"\n\nuse rule check from \"refund_check.rule\"\n\nfn route(paid: number range >=0 <=10000, asked: number range >=0 <=10000) -> check.path\n  check(paid: paid, asked: min(asked, paid)).route\n";

/// The module of X3 (a) for E202: the payment day goes to the rule.
const BILLING_KN: &str = "module billing v1\ndescription \"The billing batch of the day an order's payment is due\"\n\nuse dates terms from \"payment_terms.cal\"\nuse rule batch from \"batch.rule\"\n\nfn pick(received: date range >=2026-01-01 <=2026-12-20) -> batch.run\n  let due = terms.payment(received: received)\n  batch(pay_day: due).batch\n";

/// The module of W202: the day promised goes to the rule when the days from the payment day to it
/// are 0 or more. The `if` narrows that number, not the day, so kaname's ranges do not show the day
/// inside the rule's range.
const BILLING_KN_PROMISED: &str = "module billing v1\ndescription \"The billing batch of an order: that of the day the customer promised to pay, when it is no earlier than the payment day, and that of the payment day otherwise\"\n\nuse dates terms from \"payment_terms.cal\"\nuse rule batch from \"batch.rule\"\n\nfn pick(received: date range >=2026-01-01 <=2026-12-20, promised: date range >=2026-01-01 <=2027-01-31) -> batch.run\n  let due = terms.payment(received: received)\n  let grace = promised - due\n  if grace >= 0\n    batch(pay_day: promised).batch\n  else\n    batch(pay_day: due).batch\n";

/// The module of X4 for E203: the seats the rule says go to the transfer the host makes.
const BOOKING_KN: &str = "module booking v1\ndescription \"The seats to give an event, as the rule says for its kind: the transfer the host makes\"\n\nuse rule seats from \"seats.rule\"\nuse book hall from \"hall.book\"\n\nfn give(event: string, kind: seats.kind) -> hall.assign.do\n  hall.assign.do(event: event, count: seats(event_kind: kind).needed)\n";

/// The module of W203: the seats needed less those held, never below 0, which kaname's ranges do not
/// show.
const BOOKING_KN_HELD: &str = "module booking v1\ndescription \"The seats to give an event: what the rule says its kind needs, less those it holds already, which are never more than it needs\"\n\nuse rule seats from \"seats.rule\"\nuse book hall from \"hall.book\"\n\nfn give(event: string, kind: seats.kind, held: number range >=0 <=50) -> hall.assign.do\n  let needed = seats(event_kind: kind).needed\n  hall.assign.do(event: event, count: needed - min(held, needed))\n";

/// The module of X6 for E205: the payment day goes to the reminder.
const REMINDING_KN: &str = "module reminding v1\ndescription \"The day to remind a customer, a week before an order's payment is due\"\n\nuse dates terms from \"payment_terms.cal\"\nuse dates reminders from \"reminders.cal\"\n\nfn remind(received: date range >=2026-01-01 <=2026-12-20) -> date\n  let due = terms.payment(received: received)\n  reminders.reminder(due: due)\n";

/// The module of W205: the day promised goes to the reminder when the days from the payment day to it
/// are 0 or more, as in W202's.
const REMINDING_KN_PROMISED: &str = "module reminding v1\ndescription \"The day to remind a customer: a week before the day the customer promised to pay, when it is no earlier than the payment day, and a week before the payment day otherwise\"\n\nuse dates terms from \"payment_terms.cal\"\nuse dates reminders from \"reminders.cal\"\n\nfn remind(received: date range >=2026-01-01 <=2026-12-20, promised: date range >=2026-01-01 <=2027-01-31) -> date\n  let due = terms.payment(received: received)\n  let grace = promised - due\n  if grace >= 0\n    reminders.reminder(due: promised)\n  else\n    reminders.reminder(due: due)\n";

/// The one fake key the reproductions of W901 hold (DESIGN 16.10): a Google API key that reads as
/// a fake. It is put together from two pieces, so that no file of the source holds a key in one
/// run; the pages of codes that `explain` writes hold it whole.
macro_rules! fake_key {
    () => {
        concat!("AIzaSyD-ritsu-fake-", "key-for-tests-000000")
    };
}

/// The `.proto` of W901: a key left in a comment.
const MAPS_PROTO: &str = concat!("syntax = \"proto3\";\n\npackage maps.v1;\n\n// the Google Maps API key the service is called with: ", fake_key!(), "\nmessage Place {\n  string id = 1;\n}\n");
const MAPS_PROTO_JA: &str = concat!("syntax = \"proto3\";\n\npackage maps.v1;\n\n// サービスを呼ぶときの Google Maps の API キー: ", fake_key!(), "\nmessage Place {\n  string id = 1;\n}\n");

/// The project of X14's reproductions (E905, W905): a map of three contexts, Payments, Ordering and
/// Notices (Ordering is a partner of the other two); Payments publishes the card, whose number is
/// marked `debug_redact`; Notices publishes the API that tells the customer; and the flow of
/// Ordering gives the card to that API. The history is encrypted, so that dandori says nothing of
/// it (its W904).
const X14_MAP: &str = "map Shop(shop) v1\ndescription \"Payments charges the card, ordering takes orders, and notices write to the customer\"\n\nuse context \"contexts/payments.ctx\"\nuse context \"contexts/ordering.ctx\"\nuse context \"contexts/notices.ctx\"\n\ncovers \"payments\", \"ordering\", \"notices\"\n";
const X14_PAYMENTS: &str = "context Payments(payments) v1\ndescription \"Charges the customer's card\"\nowner \"Payments team\"\n\nowns\n  dir \"../payments\"\n\npublished language payments.v1\n  proto \"../payments/v1/card.proto\"\n\npartnership with Ordering\n";
const X14_ORDERING: &str = "context Ordering(ordering) v1\ndescription \"Takes orders, and has the customer told of them\"\nowner \"Ordering team\"\n\nowns\n  dir \"../ordering\"\n\npartnership with Payments\npartnership with Notices\n";
const X14_NOTICES: &str = "context Notices(notices) v1\ndescription \"Writes to the customer\"\nowner \"Customer care team\"\n\nowns\n  dir \"../notices\"\n\npublished language notices.v1\n  openapi \"../notices/api/notices.json\"\n\npartnership with Ordering\n";
const X14_CARD: &str = "syntax = \"proto3\";\n\npackage payments.v1;\n\nmessage Card {\n  string id = 1;\n  string number = 2 [debug_redact = true];\n}\n";
const X14_NOTICES_API: &str = "{\n  \"openapi\": \"3.1.0\",\n  \"info\": {\n    \"title\": \"Notices\",\n    \"version\": \"1.0.0\"\n  },\n  \"servers\": [\n    {\n      \"url\": \"https://notices.example.com\"\n    }\n  ],\n  \"security\": [\n    {\n      \"bearer\": []\n    }\n  ],\n  \"paths\": {\n    \"/notices\": {\n      \"post\": {\n        \"operationId\": \"tell\",\n        \"requestBody\": {\n          \"required\": true,\n          \"content\": {\n            \"application/json\": {\n              \"schema\": {\n                \"type\": \"object\",\n                \"required\": [\n                  \"order\",\n                  \"card\"\n                ],\n                \"properties\": {\n                  \"order\": {\n                    \"type\": \"string\"\n                  },\n                  \"card\": {\n                    \"type\": \"object\",\n                    \"properties\": {\n                      \"id\": {\n                        \"type\": \"string\"\n                      },\n                      \"number\": {\n                        \"type\": \"string\"\n                      }\n                    }\n                  }\n                }\n              }\n            }\n          }\n        },\n        \"responses\": {\n          \"204\": {\n            \"description\": \"told\"\n          }\n        }\n      }\n    }\n  },\n  \"components\": {\n    \"securitySchemes\": {\n      \"bearer\": {\n        \"type\": \"http\",\n        \"scheme\": \"bearer\"\n      }\n    }\n  }\n}\n";
const X14_CHECKOUT: &str = "workflow checkout v1\n  history encrypted\ndescription \"Takes an order paid by card, and has the customer told\"\n\nuse proto pay from \"../payments/v1/card.proto\"\nuse openapi notices from \"../notices/api/notices.json\"\n\ninputs\n  order : string\n  card  : pay.Card\n\ntask tell(order: string, card: pay.Card)\n  http POST notices \"/notices\"\n  key\n\nflow\n  tell(order: order, card: card)\n";
/// The same project in Japanese names.
const X14_MAP_JA: &str = "map 店(shop) v1\ndescription \"決済がカードの代金を受け取り、受注が注文を受け、通知が客に知らせる\"\n\nuse context \"contexts/決済.ctx\"\nuse context \"contexts/受注.ctx\"\nuse context \"contexts/通知.ctx\"\n\ncovers \"決済\", \"受注\", \"通知\"\n";
const X14_PAYMENTS_JA: &str = "context 決済(payments) v1\ndescription \"客のカードで代金を受け取る\"\nowner \"決済の担当\"\n\nowns\n  dir \"../決済\"\n\npublished language payments.v1\n  proto \"../決済/v1/card.proto\"\n\npartnership with 受注\n";
const X14_ORDERING_JA: &str = "context 受注(ordering) v1\ndescription \"注文を受け、客に知らせてもらう\"\nowner \"受注の担当\"\n\nowns\n  dir \"../受注\"\n\npartnership with 決済\npartnership with 通知\n";
const X14_NOTICES_JA: &str = "context 通知(notices) v1\ndescription \"客に知らせる\"\nowner \"顧客対応の担当\"\n\nowns\n  dir \"../通知\"\n\npublished language notices.v1\n  openapi \"../通知/api/notices.json\"\n\npartnership with 受注\n";
const X14_CARD_JA: &str = "syntax = \"proto3\";\n\npackage payments.v1;\n\nmessage Card {\n  string id = 1;\n  string number = 2 [debug_redact = true];\n}\n";
const X14_NOTICES_API_JA: &str = "{\n  \"openapi\": \"3.1.0\",\n  \"info\": {\n    \"title\": \"通知\",\n    \"version\": \"1.0.0\"\n  },\n  \"servers\": [\n    {\n      \"url\": \"https://notices.example.com\"\n    }\n  ],\n  \"security\": [\n    {\n      \"bearer\": []\n    }\n  ],\n  \"paths\": {\n    \"/notices\": {\n      \"post\": {\n        \"operationId\": \"tell\",\n        \"requestBody\": {\n          \"required\": true,\n          \"content\": {\n            \"application/json\": {\n              \"schema\": {\n                \"type\": \"object\",\n                \"required\": [\n                  \"order\",\n                  \"card\"\n                ],\n                \"properties\": {\n                  \"order\": {\n                    \"type\": \"string\"\n                  },\n                  \"card\": {\n                    \"type\": \"object\",\n                    \"properties\": {\n                      \"id\": {\n                        \"type\": \"string\"\n                      },\n                      \"number\": {\n                        \"type\": \"string\"\n                      }\n                    }\n                  }\n                }\n              }\n            }\n          }\n        },\n        \"responses\": {\n          \"204\": {\n            \"description\": \"知らせた\"\n          }\n        }\n      }\n    }\n  },\n  \"components\": {\n    \"securitySchemes\": {\n      \"bearer\": {\n        \"type\": \"http\",\n        \"scheme\": \"bearer\"\n      }\n    }\n  }\n}\n";
const X14_CHECKOUT_JA: &str = "workflow 注文 v1\n  history encrypted\ndescription \"カードで払った注文を受け、客に知らせてもらう\"\n\nuse proto 決済 from \"../決済/v1/card.proto\"\nuse openapi 通知 from \"../通知/api/notices.json\"\n\ninputs\n  注文 : string\n  カード : 決済.Card\n\ntask 知らせる(order: string, card: 決済.Card)\n  http POST 通知 \"/notices\"\n  key\n\nflow\n  知らせる(order: 注文, card: カード)\n";
/// The map of W905: it uses a context file that is not there, so it does not pass sakai's check.
const X14_MAP_BROKEN: &str = "map Shop(shop) v1\ndescription \"Payments charges the card, ordering takes orders, and notices write to the customer\"\n\nuse context \"contexts/payments.ctx\"\nuse context \"contexts/ordering.ctx\"\nuse context \"contexts/notices.ctx\"\nuse context \"contexts/billing.ctx\"\n\ncovers \"payments\", \"ordering\", \"notices\"\n";
const X14_MAP_BROKEN_JA: &str = "map 店(shop) v1\ndescription \"決済がカードの代金を受け取り、受注が注文を受け、通知が客に知らせる\"\n\nuse context \"contexts/決済.ctx\"\nuse context \"contexts/受注.ctx\"\nuse context \"contexts/通知.ctx\"\nuse context \"contexts/請求.ctx\"\n\ncovers \"決済\", \"受注\", \"通知\"\n";

/// The files of X14's project, with the map given.
fn x14(map: &'static str) -> Vec<(&'static str, &'static str)> {
    vec![
        ("shop.ctx", map),
        ("contexts/payments.ctx", X14_PAYMENTS),
        ("contexts/ordering.ctx", X14_ORDERING),
        ("contexts/notices.ctx", X14_NOTICES),
        ("payments/v1/card.proto", X14_CARD),
        ("notices/api/notices.json", X14_NOTICES_API),
        ("ordering/checkout.flow", X14_CHECKOUT),
    ]
}

/// The same, in Japanese names.
fn x14_ja(map: &'static str) -> Vec<(&'static str, &'static str)> {
    vec![
        ("店.ctx", map),
        ("contexts/決済.ctx", X14_PAYMENTS_JA),
        ("contexts/受注.ctx", X14_ORDERING_JA),
        ("contexts/通知.ctx", X14_NOTICES_JA),
        ("決済/v1/card.proto", X14_CARD_JA),
        ("通知/api/notices.json", X14_NOTICES_API_JA),
        ("受注/注文.flow", X14_CHECKOUT_JA),
    ]
}

/// The map of X15's reproductions (E907, W907): one context, Orders, which opens two operations of its OpenAPI document to the others.
const X15_MAP: &str = "map Shop(shop) v1\ndescription \"A shop that takes orders\"\n\nuse context \"contexts/orders.ctx\"\n\ncovers \"orders\"\n";
const X15_ORDERS: &str = "context Orders(orders) v1\ndescription \"Takes the customers' orders, and refunds them\"\nowner \"Orders team\"\n\nowns\n  dir \"../orders\"\n\npublished language orders.v1\n  openapi \"../orders/api/orders.json\"\n  open host service getOrder, refundOrder\n";
/// The OpenAPI document of X15's and X16's reproductions: an order looked at and refunded, each said to need a bearer token.
const ORDERS_API: &str = "{\n  \"openapi\": \"3.1.0\",\n  \"info\": {\"title\": \"Orders\", \"version\": \"1.0.0\"},\n  \"servers\": [{\"url\": \"https://orders.example.com\"}],\n  \"security\": [{\"bearer\": []}],\n  \"paths\": {\n    \"/orders/{orderId}\": {\n      \"get\": {\n        \"operationId\": \"getOrder\",\n        \"parameters\": [{\"name\": \"orderId\", \"in\": \"path\", \"required\": true, \"schema\": {\"type\": \"string\"}}],\n        \"responses\": {\"200\": {\"description\": \"The order\"}}\n      }\n    },\n    \"/orders/{orderId}/refunds\": {\n      \"post\": {\n        \"operationId\": \"refundOrder\",\n        \"parameters\": [{\"name\": \"orderId\", \"in\": \"path\", \"required\": true, \"schema\": {\"type\": \"string\"}}],\n        \"responses\": {\"201\": {\"description\": \"Refunded\"}, \"403\": {\"description\": \"Not allowed\"}}\n      }\n    }\n  },\n  \"components\": {\"securitySchemes\": {\"bearer\": {\"type\": \"http\", \"scheme\": \"bearer\"}}}\n}\n";
/// The gate of E907: it guards the refund, and not the look at an order.
const X15_GATE: &str = "gate refunds v1\ndescription \"Who may refund an order\"\n\nuse openapi orders from \"api/orders.json\"\n\nrole clerk\n  description \"Answers the customers, and refunds\"\n\nprincipal User\n  description \"A member of the staff\"\n  roles clerk\n\nresource Order\n  description \"An order\"\n\naction refund_order\n  description \"Refund an order\"\n  guards orders refundOrder\n  principal User\n  resource Order from orderId\n\npermit clerks_refund\n  description \"A clerk refunds an order\"\n  principal in clerk\n  action refund_order\n";
/// The same project, in Japanese names.
const X15_MAP_JA: &str = "map 店(shop) v1\ndescription \"注文を受ける店\"\n\nuse context \"contexts/受注.ctx\"\n\ncovers \"受注\"\n";
const X15_ORDERS_JA: &str = "context 受注(orders) v1\ndescription \"客の注文を受け、返金する\"\nowner \"受注の担当\"\n\nowns\n  dir \"../受注\"\n\npublished language orders.v1\n  openapi \"../受注/api/orders.json\"\n  open host service getOrder, refundOrder\n";
const ORDERS_API_JA: &str = "{\n  \"openapi\": \"3.1.0\",\n  \"info\": {\"title\": \"注文\", \"version\": \"1.0.0\"},\n  \"servers\": [{\"url\": \"https://orders.example.com\"}],\n  \"security\": [{\"bearer\": []}],\n  \"paths\": {\n    \"/orders/{orderId}\": {\n      \"get\": {\n        \"operationId\": \"getOrder\",\n        \"parameters\": [{\"name\": \"orderId\", \"in\": \"path\", \"required\": true, \"schema\": {\"type\": \"string\"}}],\n        \"responses\": {\"200\": {\"description\": \"注文\"}}\n      }\n    },\n    \"/orders/{orderId}/refunds\": {\n      \"post\": {\n        \"operationId\": \"refundOrder\",\n        \"parameters\": [{\"name\": \"orderId\", \"in\": \"path\", \"required\": true, \"schema\": {\"type\": \"string\"}}],\n        \"responses\": {\"201\": {\"description\": \"返金した\"}, \"403\": {\"description\": \"許されていない\"}}\n      }\n    }\n  },\n  \"components\": {\"securitySchemes\": {\"bearer\": {\"type\": \"http\", \"scheme\": \"bearer\"}}}\n}\n";
const X15_GATE_JA: &str = "gate 返金(refunds) v1\ndescription \"注文を返金してよい人\"\n\nuse openapi 注文 from \"api/orders.json\"\n\nrole 係(clerk)\n  description \"客に応対し、返金する\"\n\nprincipal 職員(User)\n  description \"店の職員\"\n  roles 係\n\nresource 注文(Order)\n  description \"注文\"\n\naction 返金する(refund_order)\n  description \"注文を返金する\"\n  guards 注文 refundOrder\n  principal 職員\n  resource 注文 from orderId\n\npermit 係は返金できる(clerks_refund)\n  description \"係は注文を返金できる\"\n  principal in 係\n  action 返金する\n";
/// The workflow of X16's reproductions (E908, W909): it refunds an order once the item is back.
const X16_FLOW: &str = "workflow returns v1\ndescription \"Refunds an order once the item is back\"\n\nuse openapi orders from \"orders.json\"\n\ninputs\n  order : string\n\ntask refund_order(orderId: string)\n  http POST orders \"/orders/{orderId}/refunds\"\n  key\n\nflow\n  refund_order(orderId: order)\n";
/// The gate of E908: it names the workflow, and no permit allows it the refund.
const X16_GATE_NEVER: &str = "gate refunds v1\ndescription \"Who may look at an order and refund it\"\n\nuse openapi orders from \"orders.json\"\n\nenum order_status = paid | returned\n\nrole clerk\n  description \"Answers the customers, and refunds\"\n\nprincipal User\n  description \"A member of the staff\"\n  roles clerk\n\nworkflow returns from \"returns.flow\"\n  description \"Refunds an order once the item is back\"\n\nresource Order\n  description \"An order\"\n  attributes\n    status : order_status\n\naction refund_order\n  description \"Refund an order\"\n  guards orders refundOrder\n  principal User, Workflow\n  resource Order from orderId\n\npermit clerks_refund\n  description \"A clerk refunds an order\"\n  principal in clerk\n  action refund_order\n";
/// The gate of W909: the workflow refunds only an order that has come back.
const X16_GATE_SOMETIMES: &str = "gate refunds v1\ndescription \"Who may look at an order and refund it\"\n\nuse openapi orders from \"orders.json\"\n\nenum order_status = paid | returned\n\nrole clerk\n  description \"Answers the customers, and refunds\"\n\nprincipal User\n  description \"A member of the staff\"\n  roles clerk\n\nworkflow returns from \"returns.flow\"\n  description \"Refunds an order once the item is back\"\n\nresource Order\n  description \"An order\"\n  attributes\n    status : order_status\n\naction refund_order\n  description \"Refund an order\"\n  guards orders refundOrder\n  principal User, Workflow\n  resource Order from orderId\n\npermit clerks_refund\n  description \"A clerk refunds an order\"\n  principal in clerk\n  action refund_order\n\npermit returns_refunds_returned_orders\n  description \"The returns workflow refunds an order that has come back\"\n  principal is workflow returns\n  action refund_order\n  when resource.status is returned\n";
/// The workflow of W908: the task declares the error of a denial, and handles it.
const X16_FLOW_DENIED: &str = "workflow returns v1\ndescription \"Refunds an order once the item is back\"\n\nuse openapi orders from \"orders.json\"\n\ninputs\n  order : string\n\ntask refund_order(orderId: string)\n  http POST orders \"/orders/{orderId}/refunds\"\n  errors denied = 403\n  key\n\nflow\n  refund_order(orderId: order)\n    on denied => fail Denied \"the gate did not let the workflow refund\"\n";
/// The gate of W908: the workflow is allowed to look at an order too, which it never does.
const X16_GATE_MORE: &str = "gate refunds v1\ndescription \"Who may look at an order and refund it\"\n\nuse openapi orders from \"orders.json\"\n\nenum order_status = paid | returned\n\nrole clerk\n  description \"Answers the customers, and refunds\"\n\nprincipal User\n  description \"A member of the staff\"\n  roles clerk\n\nworkflow returns from \"returns.flow\"\n  description \"Refunds an order once the item is back\"\n\nresource Order\n  description \"An order\"\n  attributes\n    status : order_status\n\naction view_order\n  description \"Look at an order\"\n  guards orders getOrder\n  principal User, Workflow\n  resource Order from orderId\n\naction refund_order\n  description \"Refund an order\"\n  guards orders refundOrder\n  principal User, Workflow\n  resource Order from orderId\n\npermit clerks_refund\n  description \"A clerk refunds an order\"\n  principal in clerk\n  action refund_order\n\npermit clerks_look\n  description \"A clerk looks at an order\"\n  principal in clerk\n  action view_order\n\npermit returns_look_and_refund\n  description \"The returns workflow looks at any order, and refunds it\"\n  principal is workflow returns\n  action view_order, refund_order\n";
/// The same, in Japanese names.
const X16_FLOW_JA: &str = "workflow 返品 v1\ndescription \"品物が戻ったら、注文を返金する\"\n\nuse openapi 注文 from \"orders.json\"\n\ninputs\n  注文番号 : string\n\ntask 返金する(orderId: string)\n  http POST 注文 \"/orders/{orderId}/refunds\"\n  key\n\nflow\n  返金する(orderId: 注文番号)\n";
const X16_GATE_NEVER_JA: &str = "gate 返金(refunds) v1\ndescription \"注文を見て、返金してよい人\"\n\nuse openapi 注文 from \"orders.json\"\n\nenum 注文の状態(order_status) = 支払済(paid) | 返品済(returned)\n\nrole 係(clerk)\n  description \"客に応対し、返金する\"\n\nprincipal 職員(User)\n  description \"店の職員\"\n  roles 係\n\nworkflow 返品(returns) from \"返品.flow\"\n  description \"品物が戻ったら、注文を返金する\"\n\nresource 注文(Order)\n  description \"注文\"\n  attributes\n    状態(status) : 注文の状態\n\naction 返金する(refund_order)\n  description \"注文を返金する\"\n  guards 注文 refundOrder\n  principal 職員, Workflow\n  resource 注文 from orderId\n\npermit 係は返金できる(clerks_refund)\n  description \"係は注文を返金できる\"\n  principal in 係\n  action 返金する\n";
const X16_GATE_SOMETIMES_JA: &str = "gate 返金(refunds) v1\ndescription \"注文を見て、返金してよい人\"\n\nuse openapi 注文 from \"orders.json\"\n\nenum 注文の状態(order_status) = 支払済(paid) | 返品済(returned)\n\nrole 係(clerk)\n  description \"客に応対し、返金する\"\n\nprincipal 職員(User)\n  description \"店の職員\"\n  roles 係\n\nworkflow 返品(returns) from \"返品.flow\"\n  description \"品物が戻ったら、注文を返金する\"\n\nresource 注文(Order)\n  description \"注文\"\n  attributes\n    状態(status) : 注文の状態\n\naction 返金する(refund_order)\n  description \"注文を返金する\"\n  guards 注文 refundOrder\n  principal 職員, Workflow\n  resource 注文 from orderId\n\npermit 係は返金できる(clerks_refund)\n  description \"係は注文を返金できる\"\n  principal in 係\n  action 返金する\n\npermit 返品は戻った注文を返金できる(returns_refunds_returned_orders)\n  description \"返品のワークフローは、戻ってきた注文を返金できる\"\n  principal is workflow 返品\n  action 返金する\n  when resource.状態 is 返品済\n";
const X16_FLOW_DENIED_JA: &str = "workflow 返品 v1\ndescription \"品物が戻ったら、注文を返金する\"\n\nuse openapi 注文 from \"orders.json\"\n\ninputs\n  注文番号 : string\n\ntask 返金する(orderId: string)\n  http POST 注文 \"/orders/{orderId}/refunds\"\n  errors denied = 403\n  key\n\nflow\n  返金する(orderId: 注文番号)\n    on denied => fail Denied \"ゲートが返金を許さなかった\"\n";
const X16_GATE_MORE_JA: &str = "gate 返金(refunds) v1\ndescription \"注文を見て、返金してよい人\"\n\nuse openapi 注文 from \"orders.json\"\n\nenum 注文の状態(order_status) = 支払済(paid) | 返品済(returned)\n\nrole 係(clerk)\n  description \"客に応対し、返金する\"\n\nprincipal 職員(User)\n  description \"店の職員\"\n  roles 係\n\nworkflow 返品(returns) from \"返品.flow\"\n  description \"品物が戻ったら、注文を返金する\"\n\nresource 注文(Order)\n  description \"注文\"\n  attributes\n    状態(status) : 注文の状態\n\naction 注文を見る(view_order)\n  description \"注文を見る\"\n  guards 注文 getOrder\n  principal 職員, Workflow\n  resource 注文 from orderId\n\naction 返金する(refund_order)\n  description \"注文を返金する\"\n  guards 注文 refundOrder\n  principal 職員, Workflow\n  resource 注文 from orderId\n\npermit 係は返金できる(clerks_refund)\n  description \"係は注文を返金できる\"\n  principal in 係\n  action 返金する\n\npermit 係は注文を見られる(clerks_look)\n  description \"係は注文を見られる\"\n  principal in 係\n  action 注文を見る\n\npermit 返品は注文を見て返金できる(returns_look_and_refund)\n  description \"返品のワークフローは、どの注文も見て、返金できる\"\n  principal is workflow 返品\n  action 注文を見る, 返金する\n";

/// The map of W907: Orders, and Payments, whose gate guards the operation it opens.
const X15_MAP_TWO: &str = "map Shop(shop) v1\ndescription \"A shop that takes orders and charges cards\"\n\nuse context \"contexts/orders.ctx\"\nuse context \"contexts/payments.ctx\"\n\ncovers \"orders\", \"payments\"\n";
const X15_PAYMENTS: &str = "context Payments(payments) v1\ndescription \"Charges the customers' cards\"\nowner \"Payments team\"\n\nowns\n  dir \"../payments\"\n\npublished language payments.v1\n  openapi \"../payments/api/payments.json\"\n  open host service createCharge\n";
const PAYMENTS_API: &str = "{\n  \"openapi\": \"3.1.0\",\n  \"info\": {\"title\": \"Payments\", \"version\": \"1.0.0\"},\n  \"servers\": [{\"url\": \"https://payments.example.com\"}],\n  \"security\": [{\"bearer\": []}],\n  \"paths\": {\n    \"/charges\": {\n      \"post\": {\n        \"operationId\": \"createCharge\",\n        \"responses\": {\"201\": {\"description\": \"The charge\"}}\n      }\n    }\n  },\n  \"components\": {\"securitySchemes\": {\"bearer\": {\"type\": \"http\", \"scheme\": \"bearer\"}}}\n}\n";
const X15_CHARGES: &str = "gate charges v1\ndescription \"Who may charge a card\"\n\nuse openapi payments from \"api/payments.json\"\n\nrole cashier\n  description \"Takes the payment for an order\"\n\nprincipal User\n  description \"A member of the staff\"\n  roles cashier\n\nresource Charge\n  description \"A charge of a card\"\n\naction create_charge\n  description \"Charge a card\"\n  guards payments createCharge\n  principal User\n  resource Charge\n\npermit cashiers_charge\n  description \"A cashier charges a card\"\n  principal in cashier\n  action create_charge\n";
/// The same, in Japanese names.
const X15_MAP_TWO_JA: &str = "map 店(shop) v1\ndescription \"注文を受け、カードで代金を受け取る店\"\n\nuse context \"contexts/受注.ctx\"\nuse context \"contexts/決済.ctx\"\n\ncovers \"受注\", \"決済\"\n";
const X15_PAYMENTS_JA: &str = "context 決済(payments) v1\ndescription \"客のカードで代金を受け取る\"\nowner \"決済の担当\"\n\nowns\n  dir \"../決済\"\n\npublished language payments.v1\n  openapi \"../決済/api/payments.json\"\n  open host service createCharge\n";
const PAYMENTS_API_JA: &str = "{\n  \"openapi\": \"3.1.0\",\n  \"info\": {\"title\": \"決済\", \"version\": \"1.0.0\"},\n  \"servers\": [{\"url\": \"https://payments.example.com\"}],\n  \"security\": [{\"bearer\": []}],\n  \"paths\": {\n    \"/charges\": {\n      \"post\": {\n        \"operationId\": \"createCharge\",\n        \"responses\": {\"201\": {\"description\": \"支払\"}}\n      }\n    }\n  },\n  \"components\": {\"securitySchemes\": {\"bearer\": {\"type\": \"http\", \"scheme\": \"bearer\"}}}\n}\n";
const X15_CHARGES_JA: &str = "gate 代金(charges) v1\ndescription \"カードで代金を受け取ってよい人\"\n\nuse openapi 決済 from \"api/payments.json\"\n\nrole 会計係(cashier)\n  description \"注文の支払いを受け付ける\"\n\nprincipal 職員(User)\n  description \"店の職員\"\n  roles 会計係\n\nresource 支払(Charge)\n  description \"カードの支払\"\n\naction 代金を受け取る(create_charge)\n  description \"カードで代金を受け取る\"\n  guards 決済 createCharge\n  principal 職員\n  resource 支払\n\npermit 会計係は受け取れる(cashiers_charge)\n  description \"会計係はカードで代金を受け取れる\"\n  principal in 会計係\n  action 代金を受け取る\n";

/// The files of E907's project: the map, its context, its document, and the gate.
fn x15() -> Vec<(&'static str, &'static str)> {
    vec![("shop.ctx", X15_MAP), ("contexts/orders.ctx", X15_ORDERS), ("orders/api/orders.json", ORDERS_API), ("orders/refunds.gate", X15_GATE)]
}

/// The same, in Japanese names.
fn x15_ja() -> Vec<(&'static str, &'static str)> {
    vec![("店.ctx", X15_MAP_JA), ("contexts/受注.ctx", X15_ORDERS_JA), ("受注/api/orders.json", ORDERS_API_JA), ("受注/返金.gate", X15_GATE_JA)]
}

/// The files of W907's project: Orders with no gate, and Payments with its gate.
fn x15_two() -> Vec<(&'static str, &'static str)> {
    vec![
        ("shop.ctx", X15_MAP_TWO),
        ("contexts/orders.ctx", X15_ORDERS),
        ("contexts/payments.ctx", X15_PAYMENTS),
        ("orders/api/orders.json", ORDERS_API),
        ("payments/api/payments.json", PAYMENTS_API),
        ("payments/charges.gate", X15_CHARGES),
    ]
}

/// The same, in Japanese names.
fn x15_two_ja() -> Vec<(&'static str, &'static str)> {
    vec![
        ("店.ctx", X15_MAP_TWO_JA),
        ("contexts/受注.ctx", X15_ORDERS_JA),
        ("contexts/決済.ctx", X15_PAYMENTS_JA),
        ("受注/api/orders.json", ORDERS_API_JA),
        ("決済/api/payments.json", PAYMENTS_API_JA),
        ("決済/代金.gate", X15_CHARGES_JA),
    ]
}

/// The files of X16's project: the document, the flow and the gate.
fn x16(flow: &'static str, gate: &'static str) -> Vec<(&'static str, &'static str)> {
    vec![("orders.json", ORDERS_API), ("returns.flow", flow), ("refunds.gate", gate)]
}

/// The same, in Japanese names.
fn x16_ja(flow: &'static str, gate: &'static str) -> Vec<(&'static str, &'static str)> {
    vec![("orders.json", ORDERS_API_JA), ("返品.flow", flow), ("返金.gate", gate)]
}

/// The rule of the module at hand in the reproductions of the modules: a fee by the subtotal.
const FEE_RULE: &str = "rule fee v1\ndescription \"The fee of an order: 5 GBP below 100 GBP, none from 100 GBP\"\n\ninputs\n  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc\n\noutputs\n  fee : money[GBPc]  round up(1GBPc)\n\ntable pick\npolicy unique\n| subtotal    | -> fee : money[GBPc] |\n| <10000GBPc  | 500GBPc              |\n| >=10000GBPc | 0GBPc                |\n";

/// The rule of the dependency tax in the reproductions of the modules: a rate of VAT by category.
const VAT_RULE: &str = "rule vat_rate v1\ndescription \"The rate of VAT for a category of goods\"\n\nenum category = standard | reduced\n\ninputs\n  category : category\n\noutputs\n  rate : rate[step 1%]  round down(1%)\n\ntable rate_by_category\npolicy unique\n| category | -> rate : rate[step 1%] |\n| standard | 20%                     |\n| reduced  | 5%                      |\n";

/// The same rule, its standard rate written over by hand in the module's copy (E305).
const VAT_RULE_EDITED: &str = "rule vat_rate v1\ndescription \"The rate of VAT for a category of goods\"\n\nenum category = standard | reduced\n\ninputs\n  category : category\n\noutputs\n  rate : rate[step 1%]  round down(1%)\n\ntable rate_by_category\npolicy unique\n| category | -> rate : rate[step 1%] |\n| standard | 25%                     |\n| reduced  | 5%                      |\n";

/// The `ritsu.mod` of tax, as a dependency by a path.
const TAX_MOD: &str = "module tax\n";

/// The first line of every `ritsu.lock`, which `ritsu mod` writes.
macro_rules! lock_head {
    () => {
        "# Written by ritsu mod. Run `ritsu mod lock` to write it again; do not edit it by hand.\nlock 1\nritsu 0.37.0\n"
    };
}

/// The `ritsu.mod` of a module that takes tax from a registry.
const SHOP_MOD_OCI: &str = "module example.com/acme/shop\nversion 1.0.0\n\nrequire tax oci \"example.com/acme/tax\" 1.0.0\n";

/// The hex of the digest the copy of tax 1.0.0 is kept under in the module's `.ritsu/modules/`
/// (DESIGN 17.15). A copy is read by the digest of its files, the lock's `files`; the OCI
/// manifest's digest only names its directory, and the copy here holds no manifest, which `ritsu
/// check` does not read.
macro_rules! tax_entry {
    () => {
        "ad42a050d36b11d05b184ceeabbc88f82ff364f63226e2e180ece5c4fe665524"
    };
}

/// A file of the copy of tax 1.0.0 in the module.
macro_rules! tax_copy {
    ($file:literal) => {
        concat!(".ritsu/modules/sha256/", tax_entry!(), "/files/", $file)
    };
}

/// The `ritsu.mod` of tax 1.0.0, from a registry.
const TAX_MOD_OCI: &str = "module example.com/acme/tax\nversion 1.0.0\n";

/// The same, with a dependency by a path (E308).
const TAX_MOD_OCI_PATH: &str = "module example.com/acme/tax\nversion 1.0.0\n\nrequire rates path \"../rates\"\n";

/// The lock of E305: the digest of tax's files before its rule was written over.
const LOCK_E305: &str = concat!(lock_head!(), "oci example.com/acme/tax 1.0.0 sha256:", tax_entry!(), " files sha256:371cad84ba72971bb3282231b913a7301282eb675fffbf647f18ccda43b6bcf1\n");

/// The lock of E308: the digest of tax's files, which hold the `require … path`.
const LOCK_E308: &str = concat!(lock_head!(), "oci example.com/acme/tax 1.0.0 sha256:", tax_entry!(), " files sha256:a744d0c29acba47340ec3ca4332ad67719ddb5996c3ec96672573273b8036a85\n");

/// The `ritsu.mod` of E309: tax from a git repository, and a `signature` line that covers it.
const SHOP_MOD_GIT_SIGNED: &str = "module example.com/acme/shop\nversion 1.0.0\n\nrequire tax git \"https://example.com/acme/tax\" 1.0.0\n\nsignature \"example.com/acme/\" key \"cosign.pub\"\n";

/// The lock of E309: tax 1.0.0 from its git repository, the files those of the module's copy.
const LOCK_E309: &str = concat!(
    lock_head!(),
    "git example.com/acme/tax 1.0.0 sha256:",
    tax_entry!(),
    " files sha256:371cad84ba72971bb3282231b913a7301282eb675fffbf647f18ccda43b6bcf1 commit 9fceb02d0ae598e95dc970b74767f19372d61af8 \"https://example.com/acme/tax\"\n"
);

/// The lock of E304: tax at 1.0.0, while `ritsu.mod` asks for 1.2.0 or later.
const LOCK_E304: &str = concat!(lock_head!(), "oci example.com/acme/tax 1.0.0 sha256:", tax_entry!(), " files sha256:371cad84ba72971bb3282231b913a7301282eb675fffbf647f18ccda43b6bcf1\n");

/// The rule of pricing in W303: the total of an order, with the VAT rate of tax.
const TOTAL_RULE: &str = "rule total v1\ndescription \"The total of an order: its subtotal and its VAT\"\n\nenum category = standard | reduced\n\ninputs\n  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc\n  category : category\n\noutputs\n  total : money[GBPc]  round half_up(1GBPc)\n\napply vat = \"@tax/vat_rate.rule\" meaning:f2850d644389817c\n  category = category\n  rate -> rate\n\nresult total = subtotal + subtotal * rate\n";

/// The rule of the module at hand in W303: it applies pricing's total, and the VAT rate of tax
/// through pricing, which it does not require.
const RECEIPT_RULE: &str = "rule receipt v1\ndescription \"The total of an order with gift wrapping: the order's total, and 3 GBP for the wrapping with its VAT\"\n\nenum category = standard | reduced\n\ninputs\n  subtotal : money[GBPc]  range >=0GBPc <=1_000_000GBPc\n  category : category\n\noutputs\n  total : money[GBPc]  round half_up(1GBPc)\n\napply order = \"@pricing/total.rule\" meaning:caa5be4f9a41c851\n  subtotal = subtotal\n  category = category\n  total -> order_total\n\napply vat = \"@pricing/@tax/vat_rate.rule\" meaning:f2850d644389817c\n  category = category\n  rate -> rate\n\nresult total = order_total + 300GBPc + 300GBPc * rate\n";

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
                "ワークフローか kaname の関数が規則を呼ぶところで、規則の前提（入力どうしの関係 `constraint`、並びの合計と長さの上限、`range from koyomi` の日）を破る値を渡すことがあるとき。ワークフローでは、dandori が知っている値の範囲の中に、前提を破る組み合わせがあるときです。範囲は dandori が値を入れるすべての場所から集めたもので、dandori の E014 と同じ読み方です。範囲を `range from koyomi` にした日付の入力では、渡す値が koyomi の日付の日なら、その日のどれかが規則の日でないときも、このエラーです。kaname の関数では、関数のベクタを流したときに前提を破る値を渡す入力があるときです。kaname の範囲は上からの見積もりで、範囲の角は起きないことがあるので、角だけでは例にしません。前提を破る呼び出しは、規則から生成したコードが入口で受け付けないので、走らせたときに初めて落ちます。注には、値の範囲と前提を破る組み合わせや日か、前提を破る関数の入力が出ます。",
                "Where a workflow or a function of kaname calls a rule, it can give the rule values that break one of its preconditions (a relation between inputs, `constraint`; a bound on the total or the length of the list it walks; the days of `range from koyomi`). For a workflow, the ranges dandori knows for the values it gives hold a combination that breaks it. The ranges are dandori's, gathered from every place a value comes from, read as dandori's E014 reads them. A date input whose range is `range from koyomi` and is given the day of a koyomi date is this error too when one of those days is not one of the rule's. For a function of kaname, a run of the function's vectors gives values that break it: kaname's ranges are an estimate from above, so a corner of them alone is no example. The rule's generated code refuses such a call at its door, so it would fail only when it runs. The notes give the ranges and the combination or the day that breaks it, or the function's input that does."
            ),
            tr!(
                "呼ぶ前に前提が保たれるよう分岐するか、値の範囲を狭めてください（入力やタスクの結果の `range`。kaname では、呼ぶ前の `if`）。前提のほうが業務に合っていないなら、規則の `constraint` を直してください。",
                "Branch so that the precondition holds before the call, or narrow the ranges (the `range` of an input or a task's result; in kaname, an `if` before the call). If the precondition is the part that is wrong, correct the rule's `constraint`."
            ),
            Repro::Dir { files: vec![("refund_check.rule", REFUND_RULE), ("refund.flow", REFUND_FLOW), ("refund.kn", REFUND_KN)], command: CHECK.to_vec() },
            &["W201"],
        ),
        Entry::new(
            "W201",
            tr!("規則を呼ぶところで、前提が保たれるかを決められません", "Whether a call keeps a rule's precondition cannot be decided"),
            tr!(
                "ワークフローか kaname の関数が規則を呼ぶところで、前提が保たれるかを決められないとき。ワークフローでは、渡す値に範囲の無いところから来るものがある（タスクの結果に `range` が無い、など）、前提が並びの合計や長さの上限である（dandori は並びの長さを知りません）、前提が koyomi の日付の日で、渡す値が何日かを言わないところ（ワークフローの入力、タスクの結果、`now`）からも来る、のどれかです。決められない前提は、`ritsu dandori build` が書くワークフローのコードが実行時に確かめます。値ができたところですぐに確かめ、前提を破る実行を `Dandori.BrokenPrecondition` で失敗させます。kaname の関数では、範囲の解析が見積もった範囲と、kaname が知っている入力どうしの関係で示せず、関数のベクタにも前提を破る入力が無いときです。kaname の生成したコードがその場所で確かめ、規則が受け付けない呼び出しでは、関数が `rule` で失敗します。",
                "Where a workflow or a function of kaname calls a rule, whether a precondition holds cannot be decided. For a workflow: a value comes from a place with no range (a task's result without `range`, say), the precondition bounds the total or the length of a list (dandori knows no list's length), or it is the days of a koyomi date and the value can come from somewhere that says nothing of what day it is (an input of the workflow, a task's answer, `now`). The workflow's code that `ritsu dandori build` writes checks such a precondition when the workflow runs, as soon as the values are made, and fails a run that breaks it with `Dandori.BrokenPrecondition`. For a function of kaname: neither the ranges kaname finds nor a relation it keeps between the inputs shows it, and no run of the function's vectors breaks it. kaname's generated code checks it there, and the function fails with `rule` where the rule refuses the call."
            ),
            tr!(
                "値の来るところに範囲を書いてください（タスクの結果やワークフローの入力の `range`）。範囲を書けないなら、このままで構いません。ワークフローのコードが実行時に確かめます。kaname では、呼ぶ前に前提を `if` で確かめて分けるか、kaname が関係を知っている値（`running_totals` の途中までの合計など）を渡してください。",
                "Give the place the value comes from a range (the `range` of a task's result or of the workflow's input). Where none can be given, leave it: the workflow's code checks it at run time. In kaname, branch on the precondition with an `if` before the call, or give values whose relation kaname keeps (the running totals of `running_totals`)."
            ),
            Repro::Dir { files: vec![("refund_check.rule", REFUND_RULE), ("refund.flow", REFUND_FLOW_OPEN), ("refund.kn", REFUND_KN_CAPPED)], command: CHECK.to_vec() },
            &["E201"],
        ),
        Entry::new(
            "E202",
            tr!("規則の日付の入力に渡す日が、入力の範囲を外れます", "A day given to a rule's date input falls outside its range"),
            tr!(
                "ワークフローか kaname の関数が規則の日付の入力に日を渡すとき、その日が、規則が宣言した入力の範囲の外になることがあるとき。ワークフローでは、koyomi の日付の日（`due.day`）を渡すときで、koyomi がその日付について数えた日のどれかが範囲の外にあるときです。koyomi は入力の範囲のすべてで日付を計算するので、外れる日は例として一つに決まります。注には、その日と、koyomi がその日を返す入力が出ます。kaname の関数では、koyomi の日付の日や、範囲の解析が見積もった日のうち、範囲の外の日を渡す入力が、関数のベクタにあるときです。注には、その関数の入力と日が出ます。範囲を `range from koyomi` にした入力では、日は規則の前提なので、E201 と W201 が確かめます。",
                "Where a workflow or a function of kaname gives a day to a rule's date input, the day can lie outside the range the rule declares for the input. For a workflow, the day of a koyomi date (`due.day`) is given, and a day koyomi counts that date coming to lies outside: koyomi computes the date on every input of its range, so the day outside is an exact example, and the notes give it with the input at which koyomi comes to it. For a function of kaname, a run of the function's vectors gives a day outside, of the days of the koyomi dates the value comes from or of the range kaname finds; the notes give the function's input and the day. An input whose range is `range from koyomi` takes the days as a precondition of the rule, which E201 and W201 hold the call to."
            ),
            tr!(
                "規則の入力の範囲を広げるか、規則の範囲を `range from koyomi` にして、koyomi の日をそのまま範囲にしてください。",
                "Widen the rule input's range, or make it `range from koyomi`, so that koyomi's days are the range."
            ),
            Repro::Dir { files: vec![("payment_terms.cal", TERMS_CAL), ("batch.rule", BATCH_RULE), ("billing.flow", BILLING_FLOW), ("billing.kn", BILLING_KN)], command: CHECK.to_vec() },
            &["W202", "E205"],
        ),
        Entry::new(
            "W202",
            tr!("規則の日付の入力に渡す日が、入力の範囲に収まるかを決められません", "Whether a day given to a rule's date input stays inside its range cannot be decided"),
            tr!(
                "規則の日付の入力に渡す値が、koyomi の日付の日のほかに、何日かを言わないところ（ワークフローの入力、タスクの結果、`now`）からも来ることがあるとき、または koyomi がその日付の日を数えないとき（入力の組み合わせが確かめる数を超える、途中で計算が止まる入力がある）。kaname の関数では、範囲の解析が見積もった日に範囲の外の日があり、関数のベクタにはその日を渡す入力が無いときも、この警告です。規則から生成したコードが、走らせたときに入口で日を確かめます。",
                "The value given to a rule's date input can come from somewhere that says nothing of what day it is (an input of the workflow, a task's answer, `now`) as well as from a koyomi date, or koyomi does not count the days of the date (more input combinations than it checks, or an input where the computation stops). For a function of kaname, it is also the days kaname's ranges find holding one outside, which no run of the function's vectors gives. The rule's generated code checks the day at its door when it runs."
            ),
            tr!(
                "値を koyomi の日付の日だけから渡せば、決められるようになります。koyomi が日を数えないなら、koyomi のファイルの入力の範囲を狭めてください。",
                "Give the input days of koyomi dates only, and it can be decided; where koyomi does not count them, narrow the inputs of its file."
            ),
            Repro::Dir { files: vec![("payment_terms.cal", TERMS_CAL), ("batch.rule", BATCH_RULE_WIDE), ("billing.flow", BILLING_FLOW_EITHER), ("billing.kn", BILLING_KN_PROMISED)], command: CHECK.to_vec() },
            &["E202"],
        ),
        Entry::new(
            "E203",
            tr!("振替に渡す額が、chobo の受け取らない額になることがあります", "An amount given to a transfer can be one chobo does not take"),
            tr!(
                "振替の額が、負か 2⁶³ − 1 を超えることがあるとき（chobo は 0 から 2⁶³ − 1 までを受け取ります）。chobo は範囲の外の額を、拒否する（業務の結果）のではなく呼び出しの失敗にします。ワークフローでは、規則の数の出力を chobo の振替の額に渡すときに確かめ、その額になる入力が規則のベクタにあるときにこのエラーにします。rulec の数えた範囲の端や、表の行に書いた数だけでは決めません（W203）。注には、その入力と額が出ます。kaname の関数では、規則の出力でも kaname が計算した額でも、どの額も確かめ、関数のベクタにその額になる入力があるときにこのエラーにします。注には、その関数の入力と額が出ます。",
                "An amount of a transfer can be below 0 or above 2⁶³ − 1 (chobo takes 0 to 2⁶³ − 1), and chobo fails such a call rather than refusing it as a business outcome. For a workflow, it is checked where a rule's numeric output is given to a chobo transfer as its amount, and it is this error where an input from the rule's vectors comes to such an amount; an end of the range rulec counts, or a number a row writes, is not enough (W203). The notes give that input and the amount. For a function of kaname, every amount is checked, a rule's output or one kaname computes, and it is this error where a run of the function's vectors comes to such an amount; the notes give the function's input and the amount."
            ),
            tr!(
                "額を 0 以上にしてください（返金などの負の額は、向きの違う振替に分けてください）。kaname では、振替の前に `if` で額を狭めることもできます。",
                "Make the amounts 0 or more (a negative one, such as a refund, is a transfer the other way). In kaname, the amount can also be narrowed with an `if` before the transfer."
            ),
            Repro::Dir { files: vec![("seats.rule", SEATS_RULE_HANDBACK), ("hall.book", HALL_BOOK), ("booking.flow", BOOKING_FLOW), ("booking.kn", BOOKING_KN)], command: CHECK.to_vec() },
            &["W203", "E204"],
        ),
        Entry::new(
            "W203",
            tr!("振替に渡す額を、chobo が受け取るかを決められません", "Whether chobo takes an amount given to a transfer cannot be decided"),
            tr!(
                "ワークフローが規則の数の出力を振替の額に渡すとき、出力の範囲に上限か下限が無いか、値が範囲の分からないところ（範囲の無いタスクの結果など）からも来ることがあるとき。rulec の数えた出力の範囲が chobo の受け取る額を外れるのに、その額になる入力が規則のベクタに無いときも、この警告です。rulec は出力の範囲を、入力をそれぞれの範囲で動かして数え、二つの入力を結ぶ `constraint` では狭めないので、出力が実際にはそういう額にならないことがあります。chobo が受け取らない額なら、ワークフローを走らせたときに呼び出しが失敗します。kaname の関数では、範囲の解析が見積もった額が chobo の受け取る範囲に収まらず、関数のベクタにはその額になる入力が無いときです。kaname の生成したコードが振替を組み立てるところで額を確かめ、受け取らない額なら関数が `range` で失敗します。",
                "Where a workflow gives a rule's numeric output to a transfer as its amount, the output's range has an open end, or the value can also come from somewhere with no range (a task's answer without one, say). It is also this warning where rulec counts the output's range outside what chobo takes and no input from the rule's vectors comes to such an amount: rulec bounds an output by moving each input over its own range, and does not narrow the bound by a `constraint` that ties two inputs, so the output may never come to one. An amount chobo does not take fails the call when the workflow runs. For a function of kaname, the amounts kaname's ranges find do not stay inside what chobo takes, and no run of the function's vectors comes to one outside; kaname's generated code checks the amount where it builds the transfer, and the function fails with `range` for one chobo does not take."
            ),
            tr!(
                "値の来るところに範囲を書いてください（タスクの結果やワークフローの入力の `range`）。rulec の数えた出力の範囲が外れるなら、rulec が 0 以上と数えるように規則を書くか（`max(…, 0)` など）、負の額を向きの違う振替に分けてください。kaname では、振替の前に `if` で額を狭めてください。",
                "Give the places the value comes from a range (the `range` of a task's answer or of the workflow's input). Where rulec's range of the output reaches outside, write the rule so that rulec counts the output as 0 or more (`max(…, 0)`, say), or make a negative amount a transfer the other way. In kaname, narrow the amount with an `if` before the transfer."
            ),
            Repro::Dir { files: vec![("seats.rule", SEATS_RULE), ("hall.book", HALL_BOOK), ("booking.flow", BOOKING_FLOW_ASKED), ("booking.kn", BOOKING_KN_HELD)], command: CHECK.to_vec() },
            &["E203"],
        ),
        Entry::new(
            "E204",
            tr!("振替が拒否されうる理由を、タスクが処理していません", "The task does not handle a refusal the transfer can come to"),
            tr!(
                "規則の出力を額に渡す `do` か `hold` の呼び出しで、操作が拒否されうる理由を、タスクが宣言したエラーとして処理していないとき。拒否されうる理由とは、額を呼び出しが渡す範囲に限った chobo の探索で、拒否される例が見つかった理由です。比べるのは帳簿の境界の理由（勘定の `refused as`）、つまり額で決まる拒否だけです。同じキーを別の引数で使い直すことのように、前の呼び出しで決まる拒否は比べません。探索は chobo の検査と同じ深さまでたどります。`post` と `void` は仮押さえの状態で拒否され、dandori が案件の状態ごとに確かめます（dandori の E022）。kaname の関数が組み立てる振替には、このエラーを出しません。拒否を扱うのは振替を当てるホストで、探索が見つけた理由は、kaname が生成する振替の型に載ります（W204）。",
                "Where a `do` or a `hold` is given a rule's output as its amount, chobo's search, with the amounts held to the range the call gives, finds a run in which the operation is refused for a reason the task does not handle as an error it declares. Only the reasons of the book's bounds (an account's `refused as`) are compared, the refusals that turn on the amounts; one that turns on the calls made before, such as a key used again with other arguments, is not. The search goes as deep as chobo's check goes. A `post` and a `void` are refused for the state their hold is in, which dandori follows with the case (dandori's E022). A transfer a function of kaname builds does not get this error: the host that applies it handles a refusal, and the reasons the search finds go on the type kaname generates for the transfer (W204)."
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
                "タスクが帳簿の境界の理由を処理しているのに、額を呼び出しが渡す範囲に限った chobo の探索で、その理由で拒否される例が見つからないとき。探索は chobo の検査と同じ深さまでしかたどらないので、起きないと言えるのはその深さまでです。額の範囲が分からないときや、chobo から帳簿の情報を得られないときも、この警告で「決められない」と知らせます。kaname の関数が組み立てる `do` と `hold` では、拒否を扱うのは振替を当てるホストで、探索が見つけた理由を、kaname が生成する振替の型に載せます。決められないときにだけこの警告を出し、型には帳簿の境界の理由を全部載せます。",
                "The task handles a reason of the book's bounds, and chobo's search, with the amounts held to the range the call gives, finds no run that comes to it. The search goes only as deep as chobo's check does, so all it shows is that the reason does not come within that depth. A case that cannot be decided at all (no range for the amounts, a book that does not answer) is this warning too. For a `do` or a `hold` a function of kaname builds, the host that applies the transfer handles a refusal, and the reasons the search finds go on the type kaname generates for the transfer; only a case that cannot be decided is this warning, and the type then lists every reason of the book's bounds."
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
                "ワークフローか kaname の関数が koyomi の日付を呼ぶとき、日付の入力に渡す日が、koyomi の入力の範囲を外れることがあるとき。渡す日がほかの koyomi の日付の日なら、koyomi が数えたその日の全部で確かめます。範囲の中の日なら、カレンダーに問い合わせる日がデータの範囲に収まることを、koyomi の検査が確かめています（koyomi の E203）。ワークフローでは、注に、外れる日と、koyomi がその日を返す入力が出ます。kaname の関数では、範囲の外の日を渡す入力が関数のベクタにあるときで、注には、その関数の入力と日が出ます。",
                "Where a workflow or a function of kaname calls a koyomi date, the day it gives the date input can be outside the range of that input. A day of another koyomi date is held to it with every day koyomi counts for that date. A day inside the range is one koyomi's own check has held to the data of its calendar wherever it asks the calendar (koyomi's E203). For a workflow, the notes give the day outside and the input at which koyomi comes to it; for a function of kaname, a run of the function's vectors gives a day outside, and the notes give the function's input and the day."
            ),
            tr!(
                "koyomi の入力の範囲を広げるか（カレンダーのデータも足してください）、範囲に収まる日を渡してください（kaname では、呼ぶ前の `if` で狭めます）。",
                "Widen the koyomi input's range (and the calendar's data), or give it a day that stays inside (in kaname, narrow it with an `if` before the call)."
            ),
            Repro::Dir { files: vec![("payment_terms.cal", TERMS_CAL), ("reminders.cal", REMINDERS_CAL), ("reminding.flow", REMINDING_FLOW), ("reminding.kn", REMINDING_KN)], command: CHECK.to_vec() },
            &["W205", "E202"],
        ),
        Entry::new(
            "W205",
            tr!("koyomi の日付に渡す日が、範囲に収まるかを決められません", "Whether a day given to a koyomi date stays inside its range cannot be decided"),
            tr!(
                "ワークフローが koyomi の日付の入力に渡す日を、dandori が知らないとき。ワークフローの入力、タスクの結果、`now` から来る日は、何日かを言いません。dandori には、日付の範囲を書く書き方がまだありません。範囲の外の日は、ワークフローを走らせたときに、koyomi が生成したコードが受け付けません。kaname の関数では、範囲の解析が見積もった日に範囲の外の日があり、関数のベクタにはその日を渡す入力が無いときです。kaname の生成したコードがその場所で確かめ、koyomi が受け付けない日では、関数が `date` で失敗します。",
                "dandori does not know the day a workflow gives a koyomi date's input: a day that comes from an input of the workflow, a task's answer or `now` says nothing of what day it is, and dandori has no way yet to write the range of a date; koyomi's generated code refuses a day outside its range when the workflow runs. For a function of kaname, the days kaname's ranges find hold one outside the range, which no run of the function's vectors gives; kaname's generated code checks it there, and the function fails with `date` where koyomi refuses the day."
            ),
            tr!(
                "ほかの koyomi の日付の日を渡せば、決められるようになります。そうでなければ、このままで構いません。範囲の外の日は、koyomi が実行時に受け付けません。",
                "Give it the day of another koyomi date, and it can be decided; otherwise leave it: koyomi refuses at run time."
            ),
            Repro::Dir { files: vec![("payment_terms.cal", TERMS_CAL), ("reminders.cal", REMINDERS_CAL_WIDE), ("reminding.flow", REMINDING_FLOW), ("reminding.kn", REMINDING_KN_PROMISED)], command: CHECK.to_vec() },
            &["E205"],
        ),
        Entry::new(
            "E206",
            tr!("仮押さえの期限が、確定や取消のときには必ず切れています", "A hold has always expired when a call on it comes"),
            tr!(
                "ワークフローが chobo の仮押さえを案件として追い（`case … follows <帳簿>.<振替>`）、確定か取消をするところで、仮押さえを作ってからその呼び出しまでの長さの下限が、有効期限（振替の `pending expires after`）以上のとき。帳簿はどの実行でもその呼び出しを `expired` で拒否し、通ったあとの流れは動きません。長さは dandori がフローの文から数えます。決まった長さの `wait` はその長さです。koyomi の日付の時刻までの `wait until` は、日付の入力に仮押さえのあとで読んだ `now` を渡したものなら、koyomi が数えた入力から日付までの日数の最小と最大と、日付の時刻から出ます。イベントを待つタスクの `on timeout` は、そのタスクの `timeout`（書かなければ一日）が過ぎてから動きます。`timeout until` でそのような koyomi の日付の時刻まで待つなら、その時刻が来てから動きます。注には、下限と、その下限になる文が出ます。",
                "Where a workflow follows a chobo hold as a case (`case … follows <book>.<transfer>`) and posts or voids it, the fewest seconds from making the hold to the call are at least the hold's expiry (the transfer's `pending expires after`). The book refuses the call with `expired` on every run, and what follows it going through never runs. dandori counts the time from the statements of the flow: a `wait` of a fixed time takes that time; a `wait until` the time of a koyomi date, whose date input was given `now` read after the hold, takes from the fewest to the most days koyomi counts from the input to the date, at the date's time; and the `on timeout` of a task that waits for an event runs once the task's `timeout` (a day without one) has passed, or, with `timeout until` the time of such a koyomi date, once that time has come. The notes give the fewest and the statements that make them up."
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
                "仮押さえを作ってから確定か取消までの長さが、有効期限の前にも後にもなりうるとき、または長さの上限が分からないとき。上限は、仮押さえを作るタスクと、あいだのタスクの `timeout` とリトライの回数から出ます。規則と日付の呼び出しは、どの出力先でも一回 10 秒までで、1 秒と 2 秒の間を置いて 2 回までリトライするので、33 秒までと数えます。コールバックのタスクは、どの出力先でも ID を渡すのに一回 60 秒までかかりうるので、試しごとに答えの待ちに 60 秒を足して数えます。`timeout until` のタスクは、`wait until` と同じく、期限の時刻まで待つと数えます。`timeout` の無いタスクや、上限の分からない `wait until` や期限があれば、上限はありません。koyomi の日数が上限を決めるときは、それがいちばん長くなる日を注に出します。期限が切れていれば帳簿が `expired` で拒否し、フローはそれを処理しています（dandori の E022）。期限が切れないと示せたとき（上限が有効期限より短いとき）は、何も出しません。",
                "The time from making a hold to posting or voiding it can fall either side of the hold's expiry, or nothing bounds it. The most comes from the `timeout` and the retries of the task that makes the hold and of the tasks between; a call of a rule or a date takes 33 seconds at the most (ten seconds a try on every target, retried twice, one second apart and then two); each try of a callback's task takes a minute more than its wait at the most, the time every target gives it to hand its id on; a task with `timeout until` waits until its time, counted as a `wait until` is. A task with no `timeout`, or a `wait until` or a `timeout until` a time nothing bounds, leaves no most. When koyomi's days make the most, a note gives the day that makes it the longest. An expired hold is refused with `expired`, which the flow handles (dandori's E022). When the call is shown to come before the hold expires (the most is shorter than the expiry), nothing is said."
            ),
            tr!(
                "仮押さえを作るタスクとあいだのタスクに `timeout` を書くと、上限が決まります。書けないなら、このままで構いません。",
                "Give the task that makes the hold, and the tasks between, a `timeout`, and the most is known; where none can be given, leave it."
            ),
            Repro::Dir { files: vec![("weekdays.cal", WEEKDAYS_CAL), ("payment_terms.cal", TERMS_AT_CAL), ("stock.book", STOCK_BOOK_LONG), ("invoice.flow", INVOICE_FLOW)], command: CHECK.to_vec() },
            &["E206"],
        ),
        // ── The modules and their dependencies (DESIGN 17.20) ──
        Entry::new(
            "E301",
            tr!("`ritsu.mod` として読めないファイル", "A `ritsu.mod` that does not read"),
            tr!(
                "モジュールの `ritsu.mod` を読めないとき。知らない語で始まる行、値の無い語、文字列の中の `\\\"` と `\\\\` のほかのエスケープ、一度しか書けない語（`module`、`version`、`description`、`ritsu`、`license`、`source`）の二度目、`module` が無いか最初の文でないこと、文字列の外の全角の空白、タブのほかの制御文字、UTF-8 として読めないバイト、`library` の知らない出力先などです。木のどれかのモジュールの `ritsu.mod` を読めなければ、依存の木が決まらないので、言語の検査に入らずに止まります（exit 1）。",
                "A module's `ritsu.mod` that ritsu cannot read: a line that starts with a word ritsu does not know, a word with no value, an escape in a string other than `\\\"` and `\\\\`, a second statement of a word written once at most (`module`, `version`, `description`, `ritsu`, `license`, `source`), no `module` or a `module` that is not the first statement, a full-width space outside a string, a control character other than a tab, bytes that are not UTF-8, a target of `library` ritsu does not know. When the `ritsu.mod` of a module of the tree does not read, the tree of dependencies is not known, and ritsu stops before any language checks a file (exit 1)."
            ),
            tr!(
                "示された行を直してください。文は `module`、`version`、`description`、`ritsu`、`license`、`source`、`require`、`exclude`、`signature`、`library` のどれかで始まり、最初の文は `module` です。",
                "Correct the line it points at. A statement starts with one of `module`, `version`, `description`, `ritsu`, `license`, `source`, `require`, `exclude`, `signature` and `library`, and the first is `module`."
            ),
            Repro::Dir { files: vec![("ritsu.mod", "module shop\nversion 1.0.0\n\nrequires tax path \"tax\"\n"), ("fee.rule", FEE_RULE)], command: CHECK.to_vec() },
            &["E302"],
        ),
        Entry::new(
            "E302",
            tr!("`ritsu.mod` の値が、決まった形ではありません", "A value of a `ritsu.mod` is not of its form"),
            tr!(
                "`ritsu.mod` の値の形が違うとき。モジュールの名前、版の字、依存の名前、`require … path` の絶対パス、`signature` の頭の部分、`library` の名前、`ritsu >=` の版、二度書いた依存の名前、空の `license`、https でない `source`、`exclude` と鍵のファイルのパス、`require … git` の URL と `dir` などです。git の URL は https か ssh（scp の形も）で書きます。`git://`、`file://`、ローカルのパス、`<transport>::` の形、この機械の外への平文の http、`-` で始まる値は受け付けません。URL と `dir` から決まるモジュールの名前に大文字があるときも、このエラーです。ユーザーかパスワードのある https の URL、パスワードのある ssh の URL、クエリのある URL は、資格情報を含むことがあるので、診断に URL も行も出しません。`require … path` の指す先に `ritsu.mod` が無いときも、値が指すものの誤りとして、このエラーです。言語の検査に入らずに止まります（exit 1）。",
                "A value of a `ritsu.mod` that is not of its form: a module's name, a version, a dependency's name, an absolute path in `require … path`, the start of the names a `signature` covers, a name in `library`, the version of `ritsu >=`, a dependency's name written twice, an empty `license`, a `source` that is not https, the path of an `exclude` or of a key file, the URL and the `dir` of a `require … git`. A git URL is https or ssh (the scp form too); `git://`, `file://`, a local path, the form `<transport>::`, plain http to anything but this machine and a value that starts with `-` are not taken, nor a URL and a `dir` that give a module name with an uppercase letter. An https URL with a user or a password, an ssh URL with a password and a URL with a query may carry a credential, so the diagnostic shows neither the URL nor the line. A `require … path` that leads to a directory with no `ritsu.mod` is this error too, as a value that names the wrong thing. ritsu stops before any language checks a file (exit 1)."
            ),
            tr!(
                "示された値を、その文の形に直してください。依存の名前は、小文字の英字で始まり、小文字の英字と数字と `_` だけでできた 64 字までの語です。版の字は `1.4.0` のような SemVer の形です。パスの依存は、`ritsu.mod` のあるディレクトリを、モジュールのルートからの相対パスで指してください。git の依存の資格情報は `ritsu.mod` に書かず、git の資格情報のヘルパーか ssh の鍵に任せてください。",
                "Write the value in the form of its statement. A dependency's name is a word of up to 64 lowercase letters, digits and `_` that starts with a letter; a version is of the form of SemVer, `1.4.0`; a dependency by a path names a directory with a `ritsu.mod` in it, by a path from the module's root. Keep the credentials of a dependency from git out of `ritsu.mod`, and leave them to git's credential helper or an ssh key."
            ),
            Repro::Dir { files: vec![("ritsu.mod", "module shop\n\nrequire tax path \"tax\"\n"), ("fee.rule", FEE_RULE), ("tax/vat_rate.rule", VAT_RULE)], command: CHECK.to_vec() },
            &["E301"],
        ),
        Entry::new(
            "E303",
            tr!("レジストリか git の依存があるのに、`ritsu.lock` がありません", "A dependency from a registry or a git repository, and no `ritsu.lock`"),
            tr!(
                "木のどれかのモジュールがレジストリか git のリポジトリから依存を取る（`require … oci` か `require … git`）のに、手元のモジュールの `ritsu.lock` が無いか、読めないとき。そうした依存の版とダイジェスト（git ならコミットも）は `ritsu.lock` が決めるので、それが無ければ、どのファイルを読むかが決まりません。lock を読めないのは、書式が違うとき（知らない行、ダイジェストの形など）です。言語の検査に入らずに止まります（exit 1）。",
                "A module of the tree takes a dependency from a registry or a git repository (`require … oci` or `require … git`), and the module at hand has no `ritsu.lock`, or one that does not read. `ritsu.lock` pins the version and the digests of each such dependency (and the commit of one from git), so without it which files to read is not known. A lock does not read when it is not of its form (a line ritsu does not know, a digest that is not one). ritsu stops before any language checks a file (exit 1)."
            ),
            tr!("`ritsu mod lock` を走らせて、`ritsu.lock` を書いてください。lock は手で書かないでください。", "Run `ritsu mod lock` to write `ritsu.lock`; do not write the lock by hand."),
            Repro::Dir { files: vec![("ritsu.mod", SHOP_MOD_OCI), ("fee.rule", FEE_RULE)], command: CHECK.to_vec() },
            &["E304"],
        ),
        Entry::new(
            "E304",
            tr!("`ritsu.lock` が `ritsu.mod` に合いません", "`ritsu.lock` does not match `ritsu.mod`"),
            tr!(
                "`ritsu.lock` が、木のモジュールの `ritsu.mod` に合わないとき。lock に無いモジュール、`require` が求める最小の版より低い lock の版、取り先の種類やパスの違い、`require … git` と lock の URL の違い、lock の版と取ってきた `ritsu.mod` の `version` の違い、手元のモジュール自身の行、lock があるのにパスの依存の行が無いこと、パスで読むモジュールの `oci` か `git` の行などです。`ritsu.mod` を手で書き替えて、lock を書き直していないときに起きます。言語の検査に入らずに止まります（exit 1）。",
                "`ritsu.lock` does not match the `ritsu.mod` of a module of the tree: a module the lock has no line for, a version in the lock below the least a `require` asks for, a source of another kind or another path, a `require … git` and a lock line of two URLs, a version in the lock other than the `version` of the `ritsu.mod` fetched, a line for the module at hand itself, no line for a dependency by a path where there is a lock, an `oci` or a `git` line for a module read from a path. It comes when a `ritsu.mod` was edited by hand and the lock was not written again. ritsu stops before any language checks a file (exit 1)."
            ),
            tr!("`ritsu mod lock` を走らせて、lock を書き直してください。", "Run `ritsu mod lock` to write the lock again."),
            Repro::Dir {
                files: vec![("ritsu.mod", "module example.com/acme/shop\nversion 1.0.0\n\nrequire tax oci \"example.com/acme/tax\" 1.2.0\n"), ("ritsu.lock", LOCK_E304), ("fee.rule", FEE_RULE)],
                command: CHECK.to_vec(),
            },
            &["E303", "W302"],
        ),
        Entry::new(
            "E305",
            tr!("置き場か同梱のコピーのファイルが、lock のダイジェストと違います", "The files in the store or in the module's copy differ from the lock's digest"),
            tr!(
                "レジストリの依存のファイルを、置き場か、モジュールに同梱したコピー（`.ritsu/modules/`。`ritsu mod vendor` が作ります）から読むとき、そのファイルのダイジェストが `ritsu.lock` の `files` のダイジェストと違うとき。だれかがコピーのファイルを手で書き替えたか、ファイルが壊れています。lock が固定した中身ではないので読まず、言語の検査に入らずに止まります（exit 1）。",
                "The files of a dependency from a registry, read from the store or from the module's own copy of it (`.ritsu/modules/`, which `ritsu mod vendor` makes), have a digest other than the `files` digest of `ritsu.lock`: someone edited a file of the copy, or a file is damaged. They are not the contents the lock pins, so ritsu does not read them, and stops before any language checks a file (exit 1)."
            ),
            tr!(
                "同梱のコピーなら、`ritsu mod vendor` で置き直してください。置き場なら、`ritsu mod clean` で消してから、`ritsu mod fetch` で取り直してください。依存を直すのは、依存のモジュールの中です。直したものを手元で先に確かめるなら、その `require` を、直したディレクトリを指す `require … path` に書き替えてください。",
                "For the module's copy, run `ritsu mod vendor` to copy the files again; for the store, run `ritsu mod clean`, then `ritsu mod fetch` to fetch them again. A dependency is corrected in its own module; to try a correction here first, make its `require` a `require … path` to the corrected directory."
            ),
            Repro::Dir {
                files: vec![
                    ("ritsu.mod", SHOP_MOD_OCI),
                    ("ritsu.lock", LOCK_E305),
                    ("fee.rule", FEE_RULE),
                    (tax_copy!("ritsu.mod"), TAX_MOD_OCI),
                    (tax_copy!("vat_rate.rule"), VAT_RULE_EDITED),
                ],
                command: CHECK.to_vec(),
            },
            &["E304"],
        ),
        Entry::new(
            "E306",
            tr!("同じ名前のモジュールが二か所から来ます", "A module of one name comes from two places"),
            tr!(
                "木の中で、同じ名前のモジュールが二か所から来るとき。二つの `require … path` が、`module` に同じ名前を書いた別々のディレクトリを指すとき、取ってきた `ritsu.mod` の `module` が取ったリポジトリ（git なら URL と `dir` から決まる名前）と違うとき、取ってきたファイルに `ritsu.mod` が無いときです。二つの依存が、一つの名前のモジュールを別々の取り先（レジストリと git、二つの URL）から求め、手元のモジュールがそのモジュールの `require` を書いていないときも、このエラーです。`ritsu mod lock` が版を選ぶときに読んだタグの `ritsu.mod` の `version` がタグと違うときも、二か所が食い違うので、このエラーです。モジュールは名前で見分けるので、一つの名前の中身は一つでなければなりません。言語の検査に入らずに止まります（exit 1）。",
                "Within the tree, a module of one name comes from two places: two `require … path` lead to two directories whose `ritsu.mod` give the same `module`; a `ritsu.mod` fetched gives a `module` other than the repository it was fetched from (from git, the name the URL and the `dir` give); the files fetched have no `ritsu.mod`. Two dependencies that require a module of one name from two sources (a registry and git, two URLs), with neither written in the module at hand, are this error too. A tag whose `ritsu.mod`, read by `ritsu mod lock` to choose the versions, gives another `version` is this error too: the two places disagree. A module is known by its name, so one name has one content. ritsu stops before any language checks a file (exit 1)."
            ),
            tr!(
                "モジュールを一か所から読むようにしてください。コピーしたディレクトリなら、`module` に自分の名前を付けるか、`require` を一つにしてください。依存が別々の取り先から求めるなら、手元のモジュールの `require` に取り先を書けば、それが選ばれます。取ってきたモジュールなら、その作者に知らせてください。",
                "Read the module from one place: give a directory that is a copy a name of its own in `module`, or keep one `require`; where dependencies require it from two sources, a `require` of the module at hand that writes the source chooses it; for a module fetched, tell its author."
            ),
            Repro::Dir {
                files: vec![
                    ("ritsu.mod", "module shop\n\nrequire tax path \"tax\"\nrequire vat path \"vat\"\n"),
                    ("fee.rule", FEE_RULE),
                    ("tax/ritsu.mod", TAX_MOD),
                    ("tax/vat_rate.rule", VAT_RULE),
                    ("vat/ritsu.mod", TAX_MOD),
                    ("vat/vat_rate.rule", VAT_RULE),
                ],
                command: CHECK.to_vec(),
            },
            &[],
        ),
        Entry::new(
            "E307",
            tr!("モジュールのルートに、`@` で始まる名前があります", "A name that starts with `@` at the root of a module"),
            tr!(
                "モジュールのルートのすぐ下に、`@` で始まる名前のファイルかディレクトリがあるとき。モジュールの中では `@<名前>` が依存を指すので、その名前のファイルは読めません（ritsu のファイルの層では、そこに依存が見えます）。木を読むのは止めず、ほかのファイルはいつものとおり検査しますが、このエラーで exit は 1 になります。",
                "Right under the root of a module, a file or a directory has a name that starts with `@`. Within a module, `@<name>` names a dependency, so a file of that name cannot be read: in ritsu's layer of files, a dependency is seen there. The tree is still read and the other files are checked as ever, and the error makes the exit 1."
            ),
            tr!("`@` で始まらない名前に変えてください。", "Rename it to a name that does not start with `@`."),
            Repro::Dir { files: vec![("ritsu.mod", "module shop\n"), ("fee.rule", FEE_RULE), ("@drafts/fee.rule", FEE_RULE)], command: CHECK.to_vec() },
            &[],
        ),
        Entry::new(
            "E308",
            tr!("取ってきたモジュールが、持てない依存を持っています", "A module fetched has a dependency it cannot have"),
            tr!(
                "レジストリか git のリポジトリから取ったモジュールの `ritsu.mod` が `require … path` を持つとき、またはレジストリから取ったモジュールが `require … git` を持つとき。パスの依存は一つの作業ツリーの中でだけ読めるもので、モジュールを取ってきた側には、そのパスの先がありません。レジストリのモジュールを取る人が、その git のリポジトリに届くとは限らず、git の依存には確かめる署名もありません。言語の検査に入らずに止まります（exit 1）。",
                "The `ritsu.mod` of a module from a registry or a git repository has a `require … path`, or one of a module from a registry has a `require … git`. A dependency by a path is read only within one working tree, and where the module is fetched, nothing is at the end of the path; the one who takes a module from a registry may not reach the git repository, and a dependency from git has no signature to check. ritsu stops before any language checks a file (exit 1)."
            ),
            tr!(
                "そのモジュールの作者に知らせてください。レジストリに送り出すモジュールは依存をどれも `require … oci` で、git で配るモジュールはパスでない取り先で書く必要があります。直した版が出るまでは、そのモジュールを作業ツリーに置き、`require … path` で読めます。",
                "Tell the author of the module: a module pushed to a registry writes each of its dependencies as `require … oci`, and one served from git writes them with a source that is not a path. Until a version that does is out, the module can be put in the working tree and read with `require … path`."
            ),
            Repro::Dir {
                files: vec![
                    ("ritsu.mod", SHOP_MOD_OCI),
                    ("ritsu.lock", LOCK_E308),
                    ("fee.rule", FEE_RULE),
                    (tax_copy!("ritsu.mod"), TAX_MOD_OCI_PATH),
                    (tax_copy!("vat_rate.rule"), VAT_RULE),
                ],
                command: CHECK.to_vec(),
            },
            &[],
        ),
        Entry::new(
            "E309",
            tr!("`signature` の行が、git のリポジトリから取るモジュールに当たっています", "A `signature` line covers a module from a git repository"),
            tr!(
                "手元のモジュールか、そのモジュールを求めるモジュールの `ritsu.mod` で、`signature` の行の頭の部分が、git のリポジトリから取るモジュールの名前に当たるとき。ritsu が署名を確かめるのはレジストリから取るモジュールだけで、git の依存には確かめる署名がありません。確かめると書いたモジュールを確かめずに読むことはしないので、`ritsu mod fetch` は何も取らずに止まり（exit 1）、検査は言語の検査に入らずに止まります（exit 1）。`ritsu mod site` の一覧の `signature` の行なら、一覧を読めないものとして止まります（exit 2）。",
                "In the `ritsu.mod` of the module at hand, or of a module that requires it, the start of a `signature` line covers the name of a module from a git repository. ritsu checks the signatures of modules from registries only, and a dependency from git has no signature to check; a module a line says to check is not read unchecked, so `ritsu mod fetch` stops before it fetches anything (exit 1), and a check stops before any language checks a file (exit 1). A `signature` line of the list of `ritsu mod site` makes the list one that cannot be read (exit 2)."
            ),
            tr!(
                "頭の部分を狭めて、git の依存に当たらないようにしてください（`signature \"example.com/acme/pricing\" …` のように、一つのモジュールにも書けます）。署名を確かめたいモジュールは、レジストリから取ってください。",
                "Narrow the start so that it covers no dependency from git (a line can cover one module, as `signature \"example.com/acme/pricing\" …` does); take a module whose signature is to be checked from a registry."
            ),
            Repro::Dir {
                files: vec![
                    ("ritsu.mod", SHOP_MOD_GIT_SIGNED),
                    ("ritsu.lock", LOCK_E309),
                    ("cosign.pub", "-----BEGIN PUBLIC KEY-----\n-----END PUBLIC KEY-----\n"),
                    ("fee.rule", FEE_RULE),
                    (tax_copy!("ritsu.mod"), TAX_MOD_OCI),
                    (tax_copy!("vat_rate.rule"), VAT_RULE),
                ],
                command: CHECK.to_vec(),
            },
            &["E308"],
        ),
        Entry::new(
            "W301",
            tr!("どのファイルも使っていない依存があります", "A dependency no file uses"),
            tr!(
                "モジュールが `require` した依存のファイルを、そのモジュールのどのファイルも指していないとき。指すとは、言語の参照（`apply`、`use rule … from` など）、`contribution` の `to` と `after`、`.proto` の `import` です。`.proto` の `import` は `@` を書きませんが、依存の `.proto` に行き着けば、その依存を使っていると数えます。モジュールの全体を検査するときだけ言います。一部の検査では、渡さなかったファイルが何を指すかを読まないからです。依存のモジュールの W301 は、その依存の警告として数えます。",
                "No file of a module names a file of a dependency its `ritsu.mod` requires. To name is a language's reference (`apply`, `use rule … from` and the others), the `to` and the `after` of a `contribution`, and the `import` of a `.proto`: an `import` writes no `@`, and one that comes to a `.proto` of the dependency counts as a use of it. It is said only when the whole module is checked, since a check of some of its files does not read what the others name. A W301 of a dependency's module is counted with that dependency's warnings."
            ),
            tr!(
                "使わないなら、`require` の行を消してください（`ritsu mod remove <名前>`）。使うのなら、使うところで `@<名前>/…` のファイルを指してください。",
                "If the module does not use it, delete the `require` (`ritsu mod remove <name>`); if it is to, name a file of `@<name>/…` where it does."
            ),
            Repro::Dir { files: vec![("ritsu.mod", "module shop\n\nrequire tax path \"tax\"\n"), ("fee.rule", FEE_RULE), ("tax/ritsu.mod", TAX_MOD), ("tax/vat_rate.rule", VAT_RULE)], command: CHECK.to_vec() },
            &["W303"],
        ),
        Entry::new(
            "W302",
            tr!("`ritsu.lock` に、求められていないモジュールの行があります", "`ritsu.lock` has a line for a module nothing requires"),
            tr!(
                "`ritsu.lock` に、手元のモジュールから `require` をたどっても着かないモジュールの行があるとき。`ritsu.mod` から `require` を手で消して、lock を書き直していないときに起きます。木を読むのは止めません。",
                "`ritsu.lock` has a line for a module that the `require` of the tree, followed from the module at hand, do not come to. It comes when a `require` was deleted from a `ritsu.mod` by hand and the lock was not written again. The tree is still read."
            ),
            tr!("`ritsu mod lock` を走らせて、lock を書き直してください。", "Run `ritsu mod lock` to write the lock again."),
            Repro::Dir { files: vec![("ritsu.mod", "module shop\n"), ("ritsu.lock", concat!(lock_head!(), "path tax \"tax\"\n")), ("fee.rule", FEE_RULE)], command: CHECK.to_vec() },
            &["E304"],
        ),
        Entry::new(
            "W303",
            tr!("`require` していない、依存の依存のファイルを指しています", "A path to a file of a module the module does not require, through a dependency"),
            tr!(
                "ファイルが、`@pricing/@tax/…` のように、自分のモジュールが `require` していない依存の依存のファイルを指すとき。パスは、そのファイルのモジュールが自分のルートから書いた形で見ます。`.proto` の `import` は `@` を書きませんが、行き着いたファイルが依存の依存のものなら、`import` の行にも同じ警告を出します。依存の新しい版がその依存の依存を使わなくなると、このファイルは壊れます。依存のファイルに言うものは、その依存の警告として数えます。",
                "A file names a file of a dependency of a dependency that its own module does not require, as `@pricing/@tax/…` does; the path is taken as the file's module writes it from its own root. An `import` of a `.proto` writes no `@`, and one that comes to a file of a dependency of a dependency is said on its line all the same. A version of the dependency that no longer uses that module breaks the file. One said of a dependency's file is counted with that dependency's warnings."
            ),
            tr!(
                "その依存の依存を、自分の `ritsu.mod` で `require` してください（`ritsu mod add`）。`require` すれば、そのファイルは `@<名前>/…` と直に書けます。",
                "Require that module in the module's own `ritsu.mod` (`ritsu mod add`); then its file is named as `@<name>/…`."
            ),
            Repro::Dir {
                files: vec![
                    ("ritsu.mod", "module shop\n\nrequire pricing path \"pricing\"\n"),
                    ("receipt.rule", RECEIPT_RULE),
                    ("pricing/ritsu.mod", "module pricing\n\nrequire tax path \"tax\"\n"),
                    ("pricing/total.rule", TOTAL_RULE),
                    ("pricing/tax/ritsu.mod", TAX_MOD),
                    ("pricing/tax/vat_rate.rule", VAT_RULE),
                ],
                command: CHECK.to_vec(),
            },
            &["W301"],
        ),
        // ── Security (DESIGN 16) ──
        Entry::new(
            "W901",
            tr!("鍵が契約の文書に書かれています", "A key is written in a contract"),
            tr!(
                "プロジェクトの `.proto` か、言語が読む OpenAPI・AsyncAPI・JSON Schema の文書（dandori の `use openapi`・`use smithy`、rulec の `import jsonschema` と `shape … jsonschema`、sakai の地図の公表された言語の `openapi`・`asyncapi` が参照する `.json`・`.yaml`・`.yml`）のどこか（値でもコメントでも）に、鍵の形の値があるとき。調べる鍵は、AWS のアクセスキー ID、GitHub・Slack・Stripe・OpenAI・Anthropic・Google の鍵やトークン、Slack の Incoming Webhook の URL、PEM の秘密鍵で、どれもプロバイダーが接頭辞や形を決めているものです。契約の文書は、同じものを複数の言語が読むので、言語ごとではなく ritsu がファイルごとに一度だけ調べます。言語のファイルに書いた鍵は、それぞれの言語の W901 です。診断には鍵の種類と、接頭辞と、長さだけを出し、鍵そのものも、その行も出しません。",
                "Somewhere in a `.proto` of the project, or in an OpenAPI, AsyncAPI or JSON Schema document a language reads (a `.json`, `.yaml` or `.yml` that dandori's `use openapi` and `use smithy`, rulec's `import jsonschema` and `shape … jsonschema`, or the `openapi` and `asyncapi` of a published language of sakai's map refer to), in a value or a comment alike, there is a value in the shape of a key: an AWS access key ID, a key or token of GitHub, Slack, Stripe, OpenAI, Anthropic or Google, a Slack incoming webhook URL, or a PEM private key, each a shape its provider fixes. More than one language can read the same contract, so ritsu looks at each once, rather than each language that reads it; a key in a file of a language is that language's W901. The diagnostic gives the kind of key, its prefix and its length, and never the key nor its line."
            ),
            tr!(
                "鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。本物の鍵なら、まずプロバイダーで無効にしてください。ファイルから消しても、リポジトリの履歴には残ります。テスト用の値なら、同じ行のコメントに `ritsu: test secret` と書いてください（`.proto` は `//`、YAML は `#` で始まるコメント）。JSON の文書にはコメントが書けないので、例の値（`EXAMPLE` で終わる AWS のアクセスキー ID や、接頭辞のあとが一つの文字の繰り返しの値）に替えてください。",
                "Keep the key where the code runs (an environment variable, the platform's connection or secret store) and read it from there. If it is real, revoke it with its provider first: taking it out of the file leaves it in the history of the repository. If it is a value for tests, write `ritsu: test secret` in a comment on the same line (`//` in a `.proto`, `#` in YAML). A JSON document has no comments: put an example value in its place (an AWS access key ID that ends in `EXAMPLE`, or one character over and over after the prefix)."
            ),
            Repro::Dir { files: vec![("maps.proto", MAPS_PROTO_JA)], command: CHECK.to_vec() },
            &[],
        )
        .english(Repro::Dir { files: vec![("maps.proto", MAPS_PROTO)], command: CHECK.to_vec() }),
        Entry::new(
            "E905",
            tr!("秘密の値を、地図の外か、印を付けたコンテキストと関係の無いコンテキストへ送ります", "A secret value goes outside the map, or to a context unrelated to the one that marked it"),
            tr!(
                "ワークフローが、契約が秘密と印を付けた値（`.proto` の `debug_redact`、OpenAPI のスキーマの `x-data-classification`・`x-sensitive-data`・`format: password`、`.flow` の `secret`）を、プロジェクトの中のファイル（OpenAPI の文書、`.proto`、Connect で呼ぶ規則、子の `.flow`、帳簿、日付のファイル）へ送り、そのファイルが地図のどのコンテキストにも属さないか、印を付けたコンテキストと地図の上で関係の無いコンテキストに属するとき。`separate ways` は関係に数えません。印を付けたコンテキストは、印を書いたファイルが属するコンテキストです。`.flow` の `secret` の印と、どのコンテキストにも属さないファイルの印は、フローのコンテキストのものとします。どの地図のコンテキストにも属さないフローは見ません。プロジェクトの外の相手（モデルのプロバイダー、判断のモデルの API、URL、AWS のサービス）へ送ることは、dandori の E906 が言います。",
                "A workflow gives a value a contract marks secret (`debug_redact` in a `.proto`; `x-data-classification`, `x-sensitive-data` or `format: password` in an OpenAPI schema; `secret` in the `.flow`) to a file of the project (an OpenAPI document, a `.proto`, a rule called at its Connect service, a child `.flow`, a book, a dates file), and that file belongs to no context of the map, or to a context the map does not relate to the one that marked the value; `separate ways` relates nothing. The context that marked a value is the one the file of the mark belongs to; a mark the `.flow` writes, and one in a file no context holds, are the flow's own context's. A flow that belongs to no context of any map is not looked at. Sending a secret outside the project (a model's provider, a decision model's API, a URL, an AWS service) is dandori's E906."
            ),
            tr!(
                "値の代わりに参照（ID やシークレットの名前）を送り、受け取る側で値を取ってきてください。受け取る側が値を持ってよいなら、地図に関係を足すか、送り先のファイルをコンテキストに入れてください（`owns`）。そこへ送ることを意図しているなら、タスクの下に `discloses <引数> \"<理由>\"` と書いてください。",
                "Send a reference (an ID, the name of a secret) instead, and have the other side fetch the value. If the other side may hold it, add the relationship to the map, or give the file to a context (`owns`). If sending it there is intended, write `discloses <parameter> \"<why>\"` under the task."
            ),
            Repro::Dir { files: x14_ja(X14_MAP_JA), command: CHECK.to_vec() },
            &["W905"],
        )
        .english(Repro::Dir { files: x14(X14_MAP), command: CHECK.to_vec() }),
        Entry::new(
            "W905",
            tr!("秘密の値の送り先が地図のどこかを、決められません", "Where in the map a secret value goes cannot be decided"),
            tr!(
                "ワークフローが秘密の値をプロジェクトの中のファイルへ送るとき、地図が sakai の検査を通らず、フローや送り先のファイルがどのコンテキストに属するかを sakai が答えられないとき。",
                "A workflow gives a secret value to a file of the project, and the map does not pass sakai's check, so sakai cannot say which context the flow or the file belongs to."
            ),
            tr!("`sakai check` が通るよう地図を直してください。", "Correct the map so that `sakai check` passes."),
            Repro::Dir { files: x14_ja(X14_MAP_BROKEN_JA), command: CHECK.to_vec() },
            &["E905"],
        )
        .english(Repro::Dir { files: x14(X14_MAP_BROKEN), command: CHECK.to_vec() }),
        // ── Authorization (sekisho's DESIGN 4.6) ──
        Entry::new(
            "E907",
            tr!("コンテキストが公開する操作を、どの action も守っていません", "No action guards an operation a context opens"),
            tr!(
                "地図のコンテキストが `open host service` で公開する操作（`.proto` のサービスのメソッドと、OpenAPI の文書の操作）を、どの `.gate` の action も `guards` で守らず（`cedar \"…\"` と書いた Cedar のスキーマの `@guards` でも守らず）、文書がだれでも呼べると書いてもいない（`security: []`）とき。そのコンテキストのほかの操作を守る action があるときに出ます。どれも守られていなければ W907 です。AsyncAPI のチャネルと規則の Connect のサービスは、`guards` で書けるものでないので、求めません。",
                "An operation a context of the map opens with `open host service` (a method of a service of a `.proto`, an operation of an OpenAPI document) is guarded by no action of a `.gate` (`guards`), nor by the `@guards` of a schema of Cedar written as `cedar \"…\"`, and its document does not say that anyone may call it (`security: []`); when an action guards another operation of the same context. When none of them is guarded, it is W907. A channel of an AsyncAPI document and a rule's Connect service are not asked for: `guards` cannot name them."
            ),
            tr!(
                "その操作を守る action を、そのコンテキストの `.gate` に書いてください。だれでも呼べるようにわざとしている OpenAPI の操作なら、文書でその操作に `security: []` と書いてください。",
                "Write an action that guards the operation in a `.gate` of the context. If anyone may call an OpenAPI operation on purpose, write `security: []` on it in its document."
            ),
            Repro::Dir { files: x15_ja(), command: CHECK.to_vec() },
            &["W907"],
        )
        .english(Repro::Dir { files: x15(), command: CHECK.to_vec() }),
        Entry::new(
            "W907",
            tr!("コンテキストが、公開する操作のどれも action で守っていません", "A context guards none of the operations it opens"),
            tr!(
                "地図のコンテキストが公開する操作（E907 と同じもの）を、どの action も守っていないとき。コンテキストごとに一つ出ます。そのコンテキストは、まだ sekisho で認可を書いていません。プロジェクトに `.gate` も、地図か要件が指す Cedar も無ければ、X15 は何も言いません。",
                "No action guards any operation a context of the map opens (as E907 counts them); once a context. The context has written no authorization with sekisho yet. A project with no `.gate`, and no Cedar its maps or requirements name, is not asked."
            ),
            tr!("そのコンテキストの `.gate` を書き、公開する操作をそれぞれ action で守ってください。", "Write a `.gate` of the context, and guard each operation it opens with an action."),
            Repro::Dir { files: x15_two_ja(), command: CHECK.to_vec() },
            &["E907"],
        )
        .english(Repro::Dir { files: x15_two(), command: CHECK.to_vec() }),
        Entry::new(
            "E908",
            tr!("ワークフローが呼ぶ操作を、ゲートがそのワークフローにどの組み合わせでも許しません", "A gate allows a workflow an operation it calls in no combination"),
            tr!(
                "`.gate` が `workflow … from` で書いたワークフローが、そのゲートの action が守る操作を呼び（`use openapi` の `http`、`use proto` の `connect`）、その action がワークフローをどの組み合わせでも許さないとき。その呼び出しまで進んだ実行は、いつもそこで拒まれます。",
                "A workflow a `.gate` writes with `workflow … from` calls an operation an action of the gate guards (`http` on a `use openapi`, `connect` on a `use proto`), and the action allows the workflow in no combination: every run that comes to the call is denied there."
            ),
            tr!(
                "ゲートに、ワークフローを許す permit を書くか（`principal is workflow <名前>`）、呼び出しを消してください。",
                "Write a permit in the gate that allows the workflow (`principal is workflow <name>`), or take the call out."
            ),
            Repro::Dir { files: x16_ja(X16_FLOW_JA, X16_GATE_NEVER_JA), command: CHECK.to_vec() },
            &["W909", "W908"],
        )
        .english(Repro::Dir { files: x16(X16_FLOW, X16_GATE_NEVER), command: CHECK.to_vec() }),
        Entry::new(
            "W908",
            tr!("ワークフローが、呼ばない操作の action を許されています", "A workflow is allowed an action whose operations it never calls"),
            tr!(
                "`.gate` の action のうち、`workflow … from` で書いたワークフローが許されるもの（組み合わせによって許されるものも）が守る操作を、そのワークフローのフローがどこでも呼ばないとき。ワークフローは要るより多く許されています。",
                "A workflow a `.gate` writes with `workflow … from` is allowed an action of the gate (in some combinations, or in all), and its flow calls none of the operations the action guards: the workflow is allowed more than it needs."
            ),
            tr!("ワークフローを許す permit から、その action を外してください。", "Take the action out of the permits that allow the workflow."),
            Repro::Dir { files: x16_ja(X16_FLOW_DENIED_JA, X16_GATE_MORE_JA), command: CHECK.to_vec() },
            &["E908"],
        )
        .english(Repro::Dir { files: x16(X16_FLOW_DENIED, X16_GATE_MORE), command: CHECK.to_vec() }),
        Entry::new(
            "W909",
            tr!("拒まれることのある呼び出しが、拒まれたときのエラーを宣言していません", "A call that can be denied declares no error for it"),
            tr!(
                "ワークフローが呼ぶ操作を守る action が、ワークフローを組み合わせによっては拒む（拒むかを決められないときも）のに、呼ぶタスクが、拒まれたときのエラー（HTTP の 403、Connect の `permission_denied`）を宣言していないとき。拒まれると、ワークフローは宣言していない失敗で止まります。",
                "The action that guards an operation a workflow calls denies the workflow in some combinations (or whether it does cannot be decided), and the task that calls it declares no error for a denial (403 for HTTP, `permission_denied` for Connect): a run that is denied stops with a failure the workflow does not declare."
            ),
            tr!(
                "タスクに `errors denied = 403`（Connect なら `errors denied = permission_denied`）と書き、拒まれたときにどうするかを呼び出しの下に書いてください（`on denied => …`）。",
                "Declare the error on the task, `errors denied = 403` (`errors denied = permission_denied` for Connect), and say under the call what happens when it is denied (`on denied => …`)."
            ),
            Repro::Dir { files: x16_ja(X16_FLOW_JA, X16_GATE_SOMETIMES_JA), command: CHECK.to_vec() },
            &["E908"],
        )
        .english(Repro::Dir { files: x16(X16_FLOW, X16_GATE_SOMETIMES), command: CHECK.to_vec() }),
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
