//! The ledger of every diagnostic code: what it says, when it appears, how to fix it, and
//! the smallest book that makes it appear. `chobo explain` renders from here, and a test
//! checks every example so that none of them can drift from what the checker says.

use crate::diag::Severity;
use ritsu_base::text::{Lang, Text};

pub struct Entry {
    pub code: &'static str,
    pub severity: Severity,
    pub title: Text,
    pub when: Text,
    pub fix: Text,
    /// the smallest book that shows it
    pub example: &'static str,
    /// for the codes of `--diff-base`: the book at the revision it is compared with
    pub before: Option<&'static str>,
    /// for the codes of `chobo build`: the target whose build gives it
    pub target: Option<&'static str>,
    pub related: &'static [&'static str],
}

const X_E001: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
  keep 3
";

const X_E002: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
";

const X_E003: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
";

const X_E004: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock
";

const X_E005: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move sku from supplier to stock(sku)
";

const X_E010: &str = "book shop v1
unit pcs
unit yen
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account customers : yen outside
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
";

const X_E011: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0.5 refused as out_of_stock
";

const X_E012: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
transfer shuffle(note: string, sku: string, qty: pcs)
  key note
  move qty from stock(sku) to stock(sku)
";

const X_E013: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
transfer count(note: string)
  key note
";

const X_E014: &str = "book shop v1
unit pcs incl_tax
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
";

const X_E020: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
";

const X_E021: &str = "book shop v1
unit pcs
account supplier : pcs outside
  at least 0 refused as supplier_short
";

const X_E022: &str = "book shop v1
unit pcs
account shelf(sku: string) : pcs
  at least 10 refused as below_safety_stock
  at most 5 refused as shelf_full
";

const X_E023: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0
";

const X_E030: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  move qty from supplier to stock(sku)
";

const X_E031: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, qty
  move qty from supplier to stock(sku)
";

const X_E040: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account customers : pcs outside
transfer reserve(order: string, sku: string, qty: pcs)
  key order, sku
  pending
  move qty from stock(sku) to customers
";

const X_E041: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account customers : pcs outside
transfer reserve(order: string, sku: string, qty: pcs)
  key order, sku
  pending expires after 0 minutes
  move qty from stock(sku) to customers
";

const SHOP: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
";

const X_E050: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 3 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
";

const X_E051: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship(order: string, sku: string, qty: pcs)
  key order
  move qty from stock(sku) to customers
";

const X_W107: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer send(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
";

const X_W101: &str = "book shop v1
unit yen
account wallet(member: string) : yen
  at least 0 refused as short
account bank : yen outside
transfer top_up(slip: string, member: string, amount: yen)
  key slip
  move amount from bank to wallet(member)
";

const X_W102: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account customers : pcs outside
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
";

const X_W103: &str = "book market v1
unit yen
account buyers : yen outside
account fees : yen outside
account sales(shop: string) : yen
  at least 0 refused as sales_short
transfer sell(order: string, shop: string, price: yen, fee: yen)
  key order
  move fee from sales(shop) to fees
  move price from buyers to sales(shop)
";

const X_W104: &str = "book market v1
unit yen
account buyers : yen outside
account payouts : yen outside
account escrow(order: string) : yen
  at least 0 refused as escrow_short
transfer pay(order: string, price: yen)
  key order
  pending expires after 1 hour
  move price from buyers to escrow(order)
  move price from escrow(order) to payouts
";

const X_W105: &str = "book shop v1
unit pcs
unit boxes
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
";

const X_W106: &str = "book shop v1
unit yen
account credit(member: string) : yen
  at least -1000 refused as over_limit
  at most 0 refused as overpaid
account shops : yen outside
transfer spend(slip: string, member: string, amount: yen)
  key slip
  move amount from credit(member) to shops
";

const X_E060: &str = "book split v1
unit yen
account pool(id: string) : yen
  at least 1 refused as pool_short
  at most 1000000 refused as pool_full
account credit(id: string) : yen
  at least -1000 refused as over_limit
  at most 1000 refused as overpaid
account bank : yen outside
transfer fund(note: string, id: string, amount: yen)
  key note
  move amount from bank to pool(id)
transfer repay(note: string, id: string, amount: yen)
  key note
  move amount from credit(id) to bank
transfer spread(note: string, a: string, b: string, amount: yen)
  key note
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
";

const X_E061: &str = "book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship_the_goods_to_the_customers_who_have_ordered_and_paid_for_them(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
";

fn e(code: &'static str, title: Text, when: Text, fix: Text, example: &'static str, related: &'static [&'static str]) -> Entry {
    let severity = if code.starts_with('W') { Severity::Warning } else { Severity::Error };
    Entry { code, severity, title, when, fix, example, before: None, target: None, related }
}

/// Every code the checker can print, errors first.
pub fn ledger() -> Vec<Entry> {
    let mut v = vec![
        e(
            "E001",
            tr!("読めない行があります", "A line that does not read"),
            tr!(
                "知らない語がある、字下げがそろっていない、文字列が閉じていない、行が決まった形になっていないときに出ます。ファイルの先頭の BOM、タブ、結合文字（濁点やアクセントを別の文字として付けたもの）も、このコードになります。読めない行があると、そこから先は推測になるので、chobo は名前の解決やほかの検査をしません。",
                "A word chobo does not know, an indentation that does not line up, a string left open, or a line that is not in one of the forms. A byte order mark, a tab and a combining mark (a voiced sound mark or an accent written as a character of its own) stop here too. With a line it cannot read, anything after would be a guess, so the names are not resolved and nothing else is checked."
            ),
            tr!(
                "メッセージが示す形に書き直してください。勘定の下に書けるのは `description`、`at least`、`at most`、振替の下に書けるのは `description`、`key`、`pending`、`move` です。結合文字は、合成済みの一文字（「か」と濁点ではなく「が」）で書き直してください。",
                "Rewrite the line in the form the message gives. Under an account go `description`, `at least` and `at most`; under a transfer go `description`, `key`, `pending` and `move`. Write a letter with a combining mark as the one precomposed character."
            ),
            X_E001,
            &["E002"],
        ),
        e(
            "E002",
            tr!("名前が見つかりません", "A name that is not declared"),
            tr!(
                "単位、勘定、型、振替の引数の名前が、宣言されたどれとも合わないときに出ます。chobo は名前を、書いたとおりに比べます。",
                "A unit, an account, a type or a parameter of the transfer is named that nothing declares. Names are compared exactly as written."
            ),
            tr!(
                "宣言を足すか、宣言にある名前に直してください。外の世界を表す勘定（仕入先、客）も、`account 仕入先 : 個 outside` のように宣言してください。",
                "Declare it, or correct the name to one that is declared. An account for the world outside the book (a supplier, the customers) is declared too: `account supplier : pcs outside`."
            ),
            X_E002,
            &["E001", "E004"],
        ),
        e(
            "E003",
            tr!("同じものが二度書かれています", "Declared twice"),
            tr!(
                "同じ名前の単位、勘定、振替、引数を二度宣言したときに出ます。一つの勘定や振替の下に `description`、`at least`、`at most`、`key`、`pending` を二度書いたとき、キーに同じ引数を二度並べたときも出ます。",
                "A unit, an account, a transfer or a parameter is declared twice under one name; or `description`, `at least`, `at most`, `key` or `pending` appears twice under one account or transfer; or the key lists a parameter twice."
            ),
            tr!("どちらかを消すか、別の名前にしてください。キーは一行にまとめて書いてください。", "Remove one, or give it another name. The key goes on one line."),
            X_E003,
            &[],
        ),
        e(
            "E004",
            tr!("勘定に渡す引数の数が合いません", "The wrong number of arguments for an account"),
            tr!(
                "移動に書いた勘定の引数の数が、勘定の宣言と違うときに出ます。引数は、宣言の順に位置で渡す決まりです。",
                "A move gives an account more or fewer arguments than the account declares. Arguments are passed by position, in the order they are declared."
            ),
            tr!(
                "宣言の数だけ渡してください。`account 在庫(sku: string)` なら `在庫(sku)` です。引数の無い勘定は、括弧を書かずに `客` と書いてください。",
                "Give as many as the account declares: `stock(sku)` for `account stock(sku: string)`. An account without parameters is written with no parentheses at all: `customers`."
            ),
            X_E004,
            &["E002", "E005"],
        ),
        e(
            "E005",
            tr!("型が合いません", "A parameter used as the wrong type"),
            tr!(
                "string の引数を移動の額に使ったとき、額の引数を勘定の引数に使ったとき、勘定の引数を string 以外の型で宣言したときに出ます。",
                "A string parameter is the amount of a move, an amount parameter is an account's argument, or an account declares a parameter of a type other than string."
            ),
            tr!(
                "額には、単位を型にした引数（`数: 個`）か数を書いてください。勘定の引数には、string の引数か文字列を書いてください。",
                "An amount is a parameter whose type is a unit (`qty: pcs`), or a number; an account's argument is a string parameter, or a string."
            ),
            X_E005,
            &["E010"],
        ),
        e(
            "E010",
            tr!("移動の単位が合いません", "A move between units that differ"),
            tr!(
                "一つの移動の、元の勘定、先の勘定、額の単位がそろっていないときに出ます。chobo は換算しません。",
                "The account a move takes from, the account it puts into and its amount are not all in one unit. chobo does not convert."
            ),
            tr!(
                "単位をそろえてください。両替や換算は、単位ごとに外の勘定を置いて、単位の違う二つの移動で書いてください。換算した額は、呼ぶ側が計算して渡します。",
                "Make them one unit. To exchange one unit for another, write two moves, each through an outside account of its own unit, with amounts the caller works out."
            ),
            X_E010,
            &["E005"],
        ),
        e(
            "E011",
            tr!("数が単位に収まりません", "A number that does not fit its unit"),
            tr!(
                "帳簿に書いた数（境界や移動の額）の小数が単位の `scale` より多いとき、移動の額が負のとき、いちばん小さい単位で数えて −(2⁶³ − 1) から 2⁶³ − 1 までの外にあるときに出ます。",
                "A number written in the book (a bound, or the amount of a move) has more decimal places than the unit's `scale`, an amount is negative, or a number is outside −(2⁶³ − 1) to 2⁶³ − 1, counted in the unit's smallest step."
            ),
            tr!(
                "単位の `scale` に合う桁数で書いてください。小数の要る単位は、`unit USD scale 2` のように宣言してください。向きを変えたい移動は、`from` と `to` を入れ替えてください。",
                "Write it with no more decimal places than the unit's `scale`, and declare a unit that needs them as `unit USD scale 2`. To move the other way, swap `from` and `to`."
            ),
            X_E011,
            &["E022"],
        ),
        e(
            "E012",
            tr!("移動の元と先がいつも同じ勘定です", "A move from an account to the same account"),
            tr!(
                "一つの移動の `from` と `to` に、同じ勘定と同じ引数を書いたときに出ます。どう呼んでも same_account で断られます。",
                "One move names the same account with the same arguments in `from` and in `to`. Every call is refused with same_account."
            ),
            tr!(
                "動かす先を別の勘定にしてください。同じ種類の勘定のあいだで動かすなら、別の引数にしてください（`在庫(元) to 在庫(先)`）。",
                "Move to another account; to move between two accounts of one kind, give them different arguments (`stock(from_sku) to stock(to_sku)`)."
            ),
            X_E012,
            &[],
        ),
        e(
            "E013",
            tr!("振替に移動がありません", "A transfer with no move"),
            tr!("振替の下に `move` の行が一つも無いときに出ます。", "A transfer has no `move` line."),
            tr!(
                "`move <額> from <勘定> to <勘定>` で、どこからどこへ動かすかを書いてください。",
                "Write where it moves from and to: `move <amount> from <account> to <account>`."
            ),
            X_E013,
            &[],
        ),
        e(
            "E014",
            tr!("お金でない単位に、税込か税抜を書いています", "A tax on a unit that is not money"),
            tr!(
                "`unit` の行の最後の `incl_tax` か `excl_tax` が、お金の単位でない単位に付いているときに出ます。お金の単位は、通貨の名前（`円`、`JPY`、ISO 4217 のコード）の単位で、円と JPY は scale 0、ほかの通貨は scale 0 か 2 のものです。ほかの単位（`個`、`kg`、scale 2 の円）には、税込も税抜もありません。",
                "A `unit` line ends with `incl_tax` or `excl_tax`, and the unit is not money. A unit of money is named for a currency (`円`, `JPY`, an ISO 4217 code): yen and JPY with scale 0, any other currency with scale 0 or 2. Any other unit (`pcs`, `kg`, yen with scale 2) is neither with tax nor without."
            ),
            tr!(
                "`incl_tax` か `excl_tax` を外すか、通貨の名前の単位にしてください（`unit 円 incl_tax`、`unit USD scale 2 excl_tax`）。税込か税抜かは、規則やワークフローから額を受け取るときに、同じ区別の額だけを受け取るために書くものです。",
                "Take `incl_tax` or `excl_tax` off, or name the unit for a currency (`unit JPY incl_tax`, `unit USD scale 2 excl_tax`). Whether it is with tax or without is written so that an amount handed over from a rule or a workflow is taken only when it says the same."
            ),
            X_E014,
            &["E010"],
        ),
        e(
            "E020",
            tr!("中の勘定に境界がありません", "An account with no bound"),
            tr!(
                "`outside` を付けていない勘定に、`at least` も `at most` も無いときに出ます。chobo が守るのは勘定の境界だけなので、境界の無い中の勘定では何も守られません。",
                "An account not marked `outside` has neither `at least` nor `at most`. The bounds of accounts are all chobo keeps, so such an account would keep nothing."
            ),
            tr!(
                "`at least 0 refused as 在庫切れ` のように境界を書いてください。外の世界を表す勘定（仕入先、客、銀行）なら、`outside` を付けてください。",
                "Give it a bound, like `at least 0 refused as out_of_stock`; or mark it `outside` if it stands for the world outside the book (a supplier, the customers, a bank)."
            ),
            X_E020,
            &["E021"],
        ),
        e(
            "E021",
            tr!("外の勘定に境界があります", "An outside account with a bound"),
            tr!(
                "`outside` を付けた勘定に `at least` か `at most` を書いたときに出ます。外の勘定は外の世界を表し、マイナスにもなります。",
                "An account marked `outside` has `at least` or `at most`. An outside account stands for the world outside the book, and may go below 0."
            ),
            tr!(
                "境界を消してください。その勘定の残高を守りたいなら、`outside` を外して中の勘定にしてください。",
                "Remove the bound; or, to keep that balance, remove `outside` so that the book keeps the account."
            ),
            X_E021,
            &["E020"],
        ),
        e(
            "E022",
            tr!("境界に収まる残高がありません", "Bounds that leave no balance"),
            tr!(
                "下限が上限より大きいとき、上限が 0 より小さいときに出ます。勘定は残高 0 から始まるので、上限が 0 より小さいと何も入れられません。",
                "The lower bound is above the upper bound, or the upper bound is below 0. An account starts at 0, so below 0 nothing could ever be put into it."
            ),
            tr!("下限を上限以下にし、上限を 0 以上にしてください。", "Make the lower bound no higher than the upper bound, and the upper bound 0 or more."),
            X_E022,
            &["E011"],
        ),
        e(
            "E023",
            tr!("境界に断る理由の名前がありません", "A bound with no reason to refuse with"),
            tr!(
                "境界に `refused as <理由>` が無いとき、または理由の名前が chobo の決めている名前（key_conflict、already_refused、same_account、no_such_hold、already_posted、already_voided、expired、over_hold）と同じときに出ます。理由の名前は、断ったときに呼ぶ側が受け取るもので、dandori のタスクのエラーの名前にもなります。",
                "A bound has no `refused as <reason>`, or its reason is one of the names chobo gives itself (key_conflict, already_refused, same_account, no_such_hold, already_posted, already_voided, expired, over_hold). The reason is what a caller gets back when the bound refuses, and it is the name of the error of a dandori task."
            ),
            tr!(
                "業務の言葉で理由を付けてください（`at least 0 refused as 在庫切れ`）。",
                "Name it in the business's words: `at least 0 refused as out_of_stock`."
            ),
            X_E023,
            &[],
        ),
        e(
            "E030",
            tr!("振替にキーがありません", "A transfer with no key"),
            tr!(
                "振替に `key` の行が無いとき、またはキーに書いた名前が振替の引数に無いときに出ます。どの振替にも冪等のキーが要ります。同じキーで同じ中身の二度目の呼び出しは何もせず、中身が違えば断られます。",
                "A transfer has no `key` line, or the key names something that is not one of its parameters. Every transfer needs an idempotency key: a second call with the same key and the same content does nothing, and one with other content is refused."
            ),
            tr!(
                "一度だけにしたい単位を、引数で並べてください。注文と SKU の組ごとに一度なら `key 注文, sku` です。",
                "List the parameters it is to happen once for: once per order and SKU is `key order, sku`."
            ),
            X_E030,
            &["E031"],
        ),
        e(
            "E031",
            tr!("キーに額の引数があります", "An amount in the key"),
            tr!(
                "キーに、単位を型にした引数（額）を入れたときに出ます。入れると、額だけが違うリトライが別の振替として両方通り、二重に動きます。",
                "The key lists a parameter whose type is a unit. A retry with another amount would then go through as a second transfer, and move twice."
            ),
            tr!(
                "キーから額を外してください。額だけが違う二度目の呼び出しは、key_conflict で断られるようになります。",
                "Take the amount out of the key; a second call that differs only in the amount is then refused with key_conflict."
            ),
            X_E031,
            &["E030"],
        ),
        e(
            "E040",
            tr!("仮押さえの終わり方がありません", "A hold that does not say how it ends"),
            tr!(
                "`pending` だけを書いて、有効期限も `never expires` も書いていないときに出ます。仮押さえは、確定、取消、有効期限切れのどれかで終わります。",
                "`pending` stands alone, with neither an expiry nor `never expires`. A hold ends when it is posted, voided or expires."
            ),
            tr!(
                "`pending expires after 30 minutes` か `pending never expires` と書いてください。`never expires` の仮押さえを終わらせるのは、呼ぶ側だけです。",
                "Write `pending expires after 30 minutes` or `pending never expires`. Only the caller ends a hold that never expires."
            ),
            X_E040,
            &["E041"],
        ),
        e(
            "E041",
            tr!("有効期限が範囲の外です", "An expiry out of range"),
            tr!(
                "有効期限が 1 秒より短いとき、4294967295 秒（2³² − 1 秒、約 136 年）より長いとき、整数でないときに出ます。TigerBeetle の timeout は 32 ビットの秒です。",
                "The expiry is under 1 second, over 4294967295 seconds (2³² − 1 seconds, about 136 years), or not a whole number. TigerBeetle's timeout is 32 bits of seconds."
            ),
            tr!(
                "1 秒から 2³² − 1 秒までの整数で書いてください。それより長く押さえるなら、`never expires` にしてください。",
                "Write a whole number from 1 second to 2³² − 1 seconds; to hold for longer than that, use `never expires`."
            ),
            X_E041,
            &["E040"],
        ),
        Entry {
            before: Some(SHOP),
            ..e(
                "E050",
                tr!("前のリビジョンからある勘定の定義が変わりました", "An account kind changed since the revision compared with"),
                tr!(
                    "`--diff-base` で比べたリビジョンにある勘定の、単位、`scale`、外の勘定かどうか、引数、境界が変わったときに出ます。勘定の同一性に帳簿のバージョンは入らないので、すでにある勘定は前の定義のまま残ります。",
                    "An account kind of the revision given to `--diff-base` has a new unit, `scale`, parameters or bound, or is outside where it was not (or the other way round). The book's version is not part of an account's identity, so the accounts that exist keep the old definition."
                ),
                tr!(
                    "新しい名前の勘定を宣言し、残高を移す振替を書いてください。変わりうる限度は、はじめから勘定にしておいてください（会員ごとの与信枠なら「使える枠」の勘定）。",
                    "Declare an account under a new name, and write a transfer that moves the balances over. Make a limit that may change an account from the start (a member's credit limit becomes an account of credit left)."
                ),
                X_E050,
                &["E051", "W107"],
            )
        },
        Entry {
            before: Some(SHOP),
            ..e(
                "E051",
                tr!("前のリビジョンからある振替の定義が変わりました", "A transfer kind changed since the revision compared with"),
                tr!(
                    "`--diff-base` で比べたリビジョンにある振替の、引数、キー、仮押さえの終わり方、移動が変わったときに出ます。呼んでいる途中の操作をリトライすると key_conflict で断られ、押さえ中の仮押さえを確定できなくなります。",
                    "A transfer kind of the revision given to `--diff-base` has new parameters, a new key, a new way for its holds to end, or new moves. A retry in flight would be refused with key_conflict, and a hold still held could not be posted."
                ),
                tr!(
                    "新しい名前の振替を宣言し、前の振替は残しておいてください。前の振替の仮押さえがどれも終わってから、前の振替を消してください。",
                    "Declare the new form under a new name and keep the old one; remove the old one once every hold it made has ended."
                ),
                X_E051,
                &["E050", "W107"],
            )
        },
        Entry {
            target: Some("tigerbeetle-typescript"),
            ..e(
                "E060",
                tr!("一つの操作が TigerBeetle の一つのリクエストに入りません", "An operation that does not fit one TigerBeetle request"),
                tr!(
                    "`chobo build` の TigerBeetle のターゲットで、ある振替の一つの操作が送るチェーンが 253 件の振替を超えるときに出ます。チェーンは一つのリクエストで送らないと全部か無しになりませんが、`--development` で立てたレプリカは一つのリクエストに 253 件までしか受け取りません（そうでないレプリカは 8189 件まで）。境界のある勘定のあいだの移動は、一つで最大 6 件の振替になります。",
                    "With a TigerBeetle target of `chobo build`: one operation of a transfer kind sends a chain of more than 253 transfers. The chain has to go in one request to go through whole or not at all, and a replica started with `--development` takes at most 253 in one (8189 without it). A move between bounded accounts takes up to 6 transfers."
                ),
                tr!(
                    "振替の種類を、移動の少ないいくつかの種類に分けてください。分けた振替はそれぞれが一度の書き込みになるので、全部か無しにしたい移動は同じ種類に残してください。",
                    "Split the transfer kind into kinds with fewer moves. Each kind is a write of its own, so keep together the moves that have to go through all or none."
                ),
                X_E060,
                &["E061"],
            )
        },
        Entry {
            target: Some("postgres"),
            ..e(
                "E061",
                tr!("名前が PostgreSQL の識別子に収まりません", "A name too long for PostgreSQL"),
                tr!(
                    "`chobo build` の PostgreSQL のターゲットで、スキーマ（帳簿の名前）、関数（`<振替>_<操作>`、`balance_<勘定>`）、関数の引数（`p_<引数>`）の名前が 63 バイトを超えるときに出ます。PostgreSQL はそれより長い名前を黙って切り詰めるので、二つの関数が同じ名前になりえます。日本語は一字 3 バイトです。",
                    "With a PostgreSQL target of `chobo build`: the name of the schema (the book's), of a function (`<transfer>_<operation>`, `balance_<account>`) or of a function's parameter (`p_<parameter>`) is longer than 63 bytes. PostgreSQL cuts a longer name short without a word, so two functions could end up with one name. A Japanese character is 3 bytes."
                ),
                tr!("名前を短くしてください。ASCII なら 63 字、日本語なら 21 字までです（関数の名前では、後ろに付く `_hold` などの分を引いた字数です）。", "Make the name shorter: 63 ASCII letters, or 21 Japanese characters, less what follows it in a function's name (`_hold`, …)."),
                X_E061,
                &["E060"],
            )
        },
        e(
            "W101",
            tr!("残高がたまる一方の勘定", "An account that only fills"),
            tr!(
                "中の勘定へ入れる移動はあるのに、そこから取る移動がどこにも無いときに出ます。入ったものは出ていけません。",
                "Moves put into an account the book keeps, and no move anywhere takes out of it: whatever goes in stays."
            ),
            tr!(
                "取り出す振替を書いてください。外の世界を表す勘定なら、`outside` を付けてください。",
                "Write a transfer that takes out of it, or mark it `outside` if it stands for the world outside the book."
            ),
            X_W101,
            &["W102", "W106"],
        ),
        e(
            "W102",
            tr!("いつも断られる振替", "A transfer that is always refused"),
            tr!(
                "振替が取る元の勘定に、入れる移動がどこにも無く、その勘定の下限が 0 以上のときに出ます。0 より多く動かせば、いつも断られます。",
                "What a transfer takes from has a lower bound of 0 or more, and nothing anywhere puts into it: whenever the transfer moves more than 0, it is refused."
            ),
            tr!("その勘定へ入れる振替（入荷、入金）を書いてください。", "Write a transfer that puts into that account (a delivery, a top-up)."),
            X_W102,
            &["W101"],
        ),
        e(
            "W103",
            tr!("移動の順序のせいで断られる振替", "A transfer refused for the order of its moves"),
            tr!(
                "すぐに確定する振替で、前の移動が勘定から取り、後の移動が同じ勘定へ入れるとき（下限）、または前の移動が入れ、後の移動が取るとき（上限）に出ます。chobo は移動を書いた順に一つずつ確かめるので、合わせれば収まる場合でも、前の移動で断られます。",
                "In a transfer that posts at once, an earlier move takes from an account that a later move puts into (a lower bound), or puts into one that a later move takes from (an upper bound). Moves are checked one at a time in the order they are written, so the earlier move is refused even when the two together would fit."
            ),
            tr!(
                "頼られる側の移動を先に書いてください。下限なら入れる移動を、上限なら取る移動を先にしてください。",
                "Write first the move the other one counts on: for a lower bound the one that puts in, for an upper bound the one that takes out."
            ),
            X_W103,
            &["W104"],
        ),
        e(
            "W104",
            tr!("同じ仮押さえのほかの移動に頼る移動", "A move of a hold that counts on another move of the same hold"),
            tr!(
                "仮押さえの振替で、ある移動が、同じ仮押さえのほかの移動が入れる勘定から取るとき（または、ほかの移動が取る勘定へ入れるとき）に出ます。押さえた額は、入ってくる側では使えず、出ていく側では空きになりません。順序を変えても通りません。",
                "In a hold, one move takes from an account another move of the same hold puts into (or puts into one another move takes from). What is held cannot be spent where it comes in, nor makes room where it goes out, so changing the order does not help."
            ),
            tr!(
                "取る分をはじめからその勘定に入れておくか、確定のあとで動かす別の振替に分けてください。",
                "Have the amount in that account beforehand, or move it in another transfer after the hold is posted."
            ),
            X_W104,
            &["W103"],
        ),
        e(
            "W105",
            tr!("使われない宣言", "Declared and never used"),
            tr!(
                "どの勘定にも引数にも使われない単位、どの移動にも出てこない勘定、キーにも移動にも使われない引数があるときに出ます。",
                "A unit that no account or parameter uses, an account no move names, or a parameter that is in neither the key nor any move."
            ),
            tr!("消すか、使うところを書いてください。", "Remove it, or write where it is used."),
            X_W105,
            &["W106"],
        ),
        e(
            "W106",
            tr!("効かない境界", "A bound that never matters"),
            tr!(
                "上限のある勘定へ入れる移動がどこにも無いとき（下限のある勘定から取る移動が無いときも）に出ます。その境界で断られる振替がありません。",
                "Nothing puts into an account with an upper bound (or takes out of one with a lower bound), so the bound never refuses anything."
            ),
            tr!("境界を消すか、その境界で確かめるはずだった振替を書いてください。", "Remove the bound, or write the transfer it was meant to check."),
            X_W106,
            &["W101", "W105"],
        ),
        Entry {
            before: Some(SHOP),
            ..e(
                "W107",
                tr!("前のリビジョンにあった勘定や振替がありません", "An account or transfer kind gone since the revision compared with"),
                tr!(
                    "`--diff-base` で比べたリビジョンにあった勘定や振替が、いまの帳簿に無いとき、または帳簿の名前が変わったときに出ます。データベースには、その残高と仮押さえが残ります。",
                    "An account or transfer kind of the revision given to `--diff-base` is gone, or the book has a new name. Its balances and holds stay in the database."
                ),
                tr!(
                    "残高を移す振替を書いてから消すか、残しておいてください。",
                    "Move the balances out with a transfer before removing it, or keep it."
                ),
                X_W107,
                &["E050", "E051"],
            )
        },
    ];
    v.sort_by_key(|e| (e.code.starts_with('W'), e.code));
    v
}

pub fn find(code: &str) -> Option<Entry> {
    let c = code.to_ascii_uppercase();
    ledger().into_iter().find(|e| e.code == c)
}

fn word(s: Severity, lang: Lang) -> &'static str {
    s.word(lang)
}

fn indent(s: &str) -> String {
    s.lines().map(|l| if l.is_empty() { "\n".to_string() } else { format!("  {l}\n") }).collect()
}

/// `chobo explain <code>`.
pub fn render_text(e: &Entry, lang: Lang) -> String {
    let mut o = format!("{}[{}]: {}\n", word(e.severity, lang), e.code, e.title.get(lang));
    o.push_str(tr!("\nいつ出るか\n", "\nWhen\n").get(lang));
    o.push_str(&indent(e.when.get(lang)));
    o.push_str(tr!("\n直し方\n", "\nFix\n").get(lang));
    o.push_str(&indent(e.fix.get(lang)));
    if let Some(t) = e.target {
        o.push_str(&format!("{}  chobo build <file.book> --target {t}\n", tr!("\nどのコマンドで出るか\n", "\nThe command that shows it\n").get(lang)));
    }
    if let Some(b) = e.before {
        o.push_str(tr!("\n比べるリビジョンの帳簿\n", "\nThe book at the revision compared with\n").get(lang));
        o.push_str(&indent(b));
        o.push_str(tr!("\nいまの帳簿（最小の再現）\n", "\nThe book now (the smallest reproduction)\n").get(lang));
    } else {
        o.push_str(tr!("\n最小の再現\n", "\nSmallest reproduction\n").get(lang));
    }
    o.push_str(&indent(e.example));
    if !e.related.is_empty() {
        o.push_str(&format!("{}{}\n", tr!("\n関係するコード: ", "\nRelated codes: ").get(lang), e.related.join(" ")));
    }
    o
}

/// One entry as Markdown, for `docs/codes.md`.
pub fn render_markdown(e: &Entry, lang: Lang) -> String {
    let mut o = format!("## {}\n\n", e.code);
    o.push_str(&format!("`{}` — **{}**\n\n", word(e.severity, lang), e.title.get(lang)));
    o.push_str(&format!("{} {}\n\n", tr!("**いつ出るか。**", "**When.**").get(lang), e.when.get(lang)));
    o.push_str(&format!("{} {}\n\n", tr!("**直し方。**", "**Fix.**").get(lang), e.fix.get(lang)));
    if let Some(t) = e.target {
        o.push_str(&format!("{} `chobo build <file.book> --target {t}`\n\n", tr!("**どのコマンドで出るか**:", "**The command that shows it**:").get(lang)));
    }
    if let Some(b) = e.before {
        o.push_str(tr!("**比べるリビジョンの帳簿**:\n\n", "**The book at the revision compared with**:\n\n").get(lang));
        o.push_str(&format!("```book\n{b}```\n\n"));
        o.push_str(tr!("**いまの帳簿（最小の再現）**:\n\n", "**The book now (the smallest reproduction)**:\n\n").get(lang));
    } else {
        o.push_str(tr!("**最小の再現**:\n\n", "**Smallest reproduction**:\n\n").get(lang));
    }
    o.push_str(&format!("```book\n{}```\n", e.example));
    if !e.related.is_empty() {
        let links: Vec<String> = e.related.iter().map(|c| format!("[{c}](#{})", c.to_ascii_lowercase())).collect();
        o.push_str(&format!("{}{}\n", tr!("\n関係するコード: ", "\nRelated codes: ").get(lang), links.join(", ")));
    }
    o
}

/// `chobo explain --all --format markdown`: every entry, with a table of them first.
pub fn markdown_all(lang: Lang) -> String {
    let all = ledger();
    let mut o = tr!(
        "<!-- `chobo explain --all --format markdown --lang ja` の出力です。手で編集しないでください。 -->\n\n# chobo の診断\n\nchobo が出すコードの全部について、いつ出るかと、どう直すかを書いています。一件だけ読むには `chobo explain E020` を使ってください。\n\n| コード | 種別 | 見出し |\n|---|---|---|\n",
        "<!-- The output of `chobo explain --all --format markdown --lang en`. Do not edit by hand. -->\n\n# chobo diagnostics\n\nEvery code chobo prints, when it appears, and how to fix it. For one of them: `chobo explain E020`.\n\n| Code | Severity | Title |\n|---|---|---|\n"
    )
    .get(lang)
    .to_string();
    for e in &all {
        o.push_str(&format!("| [{}](#{}) | {} | {} |\n", e.code, e.code.to_ascii_lowercase(), word(e.severity, lang), e.title.get(lang)));
    }
    for e in &all {
        o.push('\n');
        o.push_str(&render_markdown(e, lang));
    }
    o
}

pub fn text_all(lang: Lang) -> String {
    ledger().iter().map(|e| render_text(e, lang)).collect::<Vec<_>>().join("\n")
}
