//! The ledger of every diagnostic code (DESIGN 9.5): `ringi explain` reads it, `docs/codes.md` and
//! `docs/codes.ja.md` are its Markdown, and a test runs every entry's example and requires its code
//! to come out, so the example cannot go stale while the prose around it still reads well. An
//! example is in English names, and in Japanese names for `explain --lang ja`. E703 and E704 are
//! said when the screens are made, and their examples are run as `ritsu gen --target requests` runs
//! the check; E701 and W702 have no example, for ringi writes no template that gives them
//! ([`later`]). The entries
//! are ringi's; how they are written out, and how every example is run, is ritsu-base's
//! ([`ritsu_base::ledger`]).

use ritsu_base::ledger::{Entry, Ledger, Repro};
use ritsu_base::text::Text;

type Beside = &'static [(&'static str, &'static [u8])];

/// An entry with its example in English names, and in Japanese names for `explain --lang ja`.
fn e(code: &'static str, title: Text, when: Text, fix: Text, en: &'static str, ja: &'static str, related: &'static [&'static str]) -> Entry {
    with_files(code, title, when, fix, (en, &[]), (ja, &[]), related)
}

/// The same, with files beside the example (an organization, a shared route).
fn with_files(code: &'static str, title: Text, when: Text, fix: Text, en: (&'static str, Beside), ja: (&'static str, Beside), related: &'static [&'static str]) -> Entry {
    let mut x = Entry::new(code, title, when, fix, Repro::File { body: en.0, beside: en.1 }, related);
    x.repro_ja = Some(Repro::File { body: ja.0, beside: ja.1 });
    x
}

/// An entry with no example: a check of the screens that no file gives, said only of a bug in
/// ringi (E701, W702; DESIGN 13.3).
fn later(code: &'static str, title: Text, when: Text, fix: Text, related: &'static [&'static str]) -> Entry {
    Entry::new(code, title, when, fix, Repro::Later, related)
}

const E001_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

every step
  due in 3 business days at end of day

ticket
  self approval  not allowed
";
const E001_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

every step
  due in 3 business days at end of day

ticket
  self approval  not allowed
";

const E002_EN: &str = "request example v1 \"Example\"

form
  ticket : text \"Ticket number\"  count <=20

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
";
const E002_JA: &str = "request 例(example) v1 \"例\"

form
  金額 : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
";

const E003_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance
  finance = team accounting

ticket
  self approval  not allowed
";
const E003_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance)    = team finance
  経理(accounting) = team accounting

ticket
  self approval  not allowed
";

const E004_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  accounting

who
  finance = team finance

ticket
  self approval  not allowed
";
const E004_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理部

who
  経理(finance) = team finance

ticket
  self approval  not allowed
";

const E005_EN: &str = "request example v1 \"Example\"

use calendar office from \"budget.book\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
";
const E005_JA: &str = "request 例(example) v1 \"例\"

use calendar 本社(office) from \"予算.book\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
";

const E006_EN: &str = "organization company v1 \"Company\"

positions
  staff    \"Staff\"
  manager  \"Manager\"

team kinds
  department \"Department\"

route
  check  finance
";
const E006_JA: &str = "organization 会社(company) v1 \"会社\"

positions
  社員(staff)   \"社員\"
  課長(manager) \"課長\"

team kinds
  部(department) \"部\"

route
  確かめ(check)  経理
";

const E007_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed

labels ja
  amount         \"金額\"
  stage check    \"確かめ\"
  stage approve  \"承認\"
";
const E007_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed

labels en
  金額         \"Amount\"
  stage 確かめ  \"Check\"
  stage 承認    \"Approval\"
";

const E703_EN: &str = "request example v1 \"Example\"

use organization org from \"company.ringi\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  manager

who
  manager = manager of requester

every stage
  when empty escalate, then skip

ticket
  self approval  not allowed

labels ja
  request      \"例\"
  amount       \"金額\"
  who manager  \"上長\"
";

const E703_JA: &str = "request 例(example) v1 \"例\"

use organization 組織(org) from \"会社.ringi\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  上長

who
  上長(manager) = manager of requester

every stage
  when empty escalate, then skip

ticket
  self approval  not allowed

labels en
  request  \"Example\"
  金額      \"Amount\"
  who 上長  \"Manager\"
";

const E704_EN: &str = "request example v1 \"Example\"

use organization org from \"company.ringi\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=10_000_000_000_000_000JPY

route
  check  manager

who
  manager = manager of requester

every stage
  when empty escalate, then skip

ticket
  self approval  not allowed
";

const E704_JA: &str = "request 例(example) v1 \"例\"

use organization 組織(org) from \"会社.ringi\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=10_000_000_000_000_000円

route
  確かめ(check)  上長

who
  上長(manager) = manager of requester

every stage
  when empty escalate, then skip

ticket
  self approval  not allowed
";

const E008_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
";
const E008_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
";

const E009_EN: &str = "request example v1 \"Example\"

form
  amount  : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY
  receipt : attachment \"Receipt\"  required when amount > 30_000

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
";
const E009_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount)    : money[円] \"金額\"  range >=1円 <=100_000円
  領収書(receipt) : attachment \"領収書\"  required when 金額 > 30_000

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
";

const E101_EN: &str = "request example v1 \"Example\"

form
  days : number \"Days\"

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
";
const E101_JA: &str = "request 例(example) v1 \"例\"

form
  日数(days) : number \"日数\"

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
";

const E102_EN: &str = "request example v1 \"Example\"

form
  purpose : long text \"Purpose\"

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
";
const E102_JA: &str = "request 例(example) v1 \"例\"

form
  目的(purpose) : long text \"目的\"

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
";

const E104_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY
  fee    : money[JPY] \"Fee\" = amount * 3%

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
";
const E104_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount)  : money[円] \"金額\"  range >=1円 <=100_000円
  手数料(fee)   : money[円] \"手数料\" = 金額 * 3%

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
";

const E106_EN: &str = "request example v1 \"Example\"

form
  amount  : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY
  account : text \"Account code\"  count <=10  approvers only
  receipt : attachment \"Receipt\"  required when account is \"6100\"

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
";
const E106_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount)     : money[円] \"金額\"  range >=1円 <=100_000円
  勘定科目(account) : text \"勘定科目\"  count <=10  approvers only
  領収書(receipt)   : attachment \"領収書\"  required when 勘定科目 is \"6100\"

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
";

const W108_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check   finance
  decide  heads  when amount > 50_000JPY

who
  finance = team finance
  heads   = team finance_heads

ticket
  self approval  not allowed
  approvers edit amount
";
const W108_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理
  決裁(decide)   部長  when 金額 > 50_000円

who
  経理(finance) = team finance
  部長(heads)   = team finance_heads

ticket
  self approval  not allowed
  approvers edit 金額
";

const W109_EN: &str = "request example v1 \"Example\"

form
  system   : text \"System\"  count <=30
  password : text \"Password\"  count <=50

route
  check  it_desk

who
  it_desk = team it_desk

ticket
  self approval  not allowed
";
const W109_JA: &str = "request 例(example) v1 \"例\"

form
  システム(system)       : text \"システム\"  count <=30
  パスワード(password)   : text \"パスワード\"  count <=50

route
  確かめ(check)  情報システム

who
  情報システム(it_desk) = team it_desk

ticket
  self approval  not allowed
";

const E202_EN: &str = "request example v1 \"Example\"

use organization org from \"company.ringi\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
";
const E202_JA: &str = "request 例(example) v1 \"例\"

use organization 組織(org) from \"会社.ringi\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
";
/// An organization with no `positions`: it does not pass its own check (E001).
const COMPANY_EN: Beside = &[("company.ringi", b"organization company v1 \"Company\"\n\nteam kinds\n  department \"Department\"\n")];
const COMPANY_JA: Beside = &[("会社.ringi", "organization 会社(company) v1 \"会社\"\n\nteam kinds\n  部(department) \"部\"\n".as_bytes())];

const E304_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  decide  at least 2 of president

who
  president = person \"e0101\"

ticket
  self approval  not allowed
";
const E304_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  決裁(decide)  at least 2 of 社長

who
  社長(president) = person \"e0101\"

ticket
  self approval  not allowed
";

const E305_EN: &str = "request example v1 \"Example\"

form
  amount   : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY
  reviewer : person \"Reviewer\"  approvers only

route
  review  chosen_reviewer

who
  chosen_reviewer = chosen by previous approver in reviewer

ticket
  self approval  not allowed
";
const E305_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount)       : money[円] \"金額\"  range >=1円 <=100_000円
  確かめる人(reviewer) : person \"確かめる人\"  approvers only

route
  確かめ(review)  指名された人

who
  指名された人(chosen_reviewer) = chosen by previous approver in 確かめる人

ticket
  self approval  not allowed
";

const E307_EN: &str = "request example v1 \"Example\"

use route standard from \"standard.ringi\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route standard(days: 5)

ticket
  self approval  not allowed
";
const E307_JA: &str = "request 例(example) v1 \"例\"

use route 標準(standard) from \"標準.ringi\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route 標準(日数: 5)

ticket
  self approval  not allowed
";
/// A shared route that takes one input, `long`.
const STANDARD_EN: Beside = &[(
    "standard.ringi",
    b"route standard v1 \"Standard approval\"\n\ninputs\n  long : bool\n\nroute\n  check  finance\n  head   heads  when long\n\nwho\n  finance = team finance\n  heads   = team finance_heads\n",
)];
const STANDARD_JA: Beside = &[(
    "標準.ringi",
    "route 標準(standard) v1 \"標準の承認\"\n\ninputs\n  長い(long) : bool\n\nroute\n  確かめ(check)  経理\n  部長(head)     部長たち  when 長い\n\nwho\n  経理(finance)    = team finance\n  部長たち(heads)  = team finance_heads\n"
        .as_bytes(),
)];

const E308_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
  withdraw       until counsel
";
const E308_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
  withdraw       until 法務
";

const E309_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY
  limit  : money[JPY] \"Limit\"  range >=1JPY <=100_000JPY

route
  check  finance
  audit  board    when amount > limit

who
  finance = team finance
  board   = team board

ticket
  self approval  not allowed
";
const E309_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円
  上限(limit)  : money[円] \"上限\"  range >=1円 <=100_000円

route
  確かめ(check)  経理
  監査(audit)    取締役会  when 金額 > 上限

who
  経理(finance)   = team finance
  取締役会(board) = team board

ticket
  self approval  not allowed
";

const E103_EN: &str = "request example v1 \"Example\"

form
  price    : money[JPY] \"Unit price\"  range >=1JPY <=1_000JPY
  quantity : number \"Quantity\"  range >=1 <=10
  total    : money[JPY] \"Total\" = price * quantity  range >=20_000JPY <=30_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
";
const E103_JA: &str = "request 例(example) v1 \"例\"

form
  単価(price)    : money[円] \"単価\"  range >=1円 <=1_000円
  数量(quantity) : number \"数量\"  range >=1 <=10
  合計(total)    : money[円] \"合計\" = 単価 * 数量  range >=20_000円 <=30_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
";

const E105_EN: &str = "request example v1 \"Example\"

form
  category : choice \"Category\"
    travel  \"Travel\"
    expense \"Expenses\"
  amount  : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY
  receipt : attachment \"Receipt\"  shown when category is travel  required when amount > 30_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
";
const E105_JA: &str = "request 例(example) v1 \"例\"

form
  種別(category) : choice \"種別\"
    出張(travel)  \"出張\"
    経費(expense) \"経費\"
  金額(amount)  : money[円] \"金額\"  range >=1円 <=100_000円
  領収書(receipt) : attachment \"領収書\"  shown when 種別 is 出張  required when 金額 > 30_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
";

const E201_EN: &str = "request example v1 \"Example\"

form
  price    : money[JPY] \"Unit price\"  range >=1JPY <=9_000_000_000_000_000JPY
  quantity : number \"Quantity\"  range >=1 <=10_000
  total    : money[JPY] \"Total\" = price * quantity

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
";
const E201_JA: &str = "request 例(example) v1 \"例\"

form
  単価(price)    : money[円] \"単価\"  range >=1円 <=9_000_000_000_000_000円
  数量(quantity) : number \"数量\"  range >=1 <=10_000
  合計(total)    : money[円] \"合計\" = 単価 * 数量

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
";

const W201_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY
  impl   : choice \"Implementation\"
    inhouse \"In house\"
    vendor  \"Vendor\"

route
| impl    | amount       | -> decide |
| inhouse | <=50_000JPY  | manager   |
| inhouse | >50_000JPY   | head      |
| vendor  | -            | head      |

who
  manager = team managers
  head    = team heads

ticket
  self approval  not allowed
";
const W201_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円
  実装(impl)   : choice \"作り方\"
    社内(inhouse) \"社内で作る\"
    委託(vendor)  \"外に頼む\"

route
| 実装 | 金額        | -> 決裁(decide) |
| 社内 | <=50_000円  | 上長            |
| 社内 | >50_000円   | 部長            |
| 委託 | -           | 部長            |

who
  上長(manager) = team managers
  部長(head)    = team heads

ticket
  self approval  not allowed
";

const E203_EN: &str = "request example v1 \"Example\"

use rule authority from \"authority.rule\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  decide  authority.decider

who
  manager = team managers

ticket
  self approval  not allowed
";
const E203_JA: &str = "request 例(example) v1 \"例\"

use rule 権限(authority) from \"authority.rule\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  決裁(decide)  権限.decider

who
  上長(manager) = team managers

ticket
  self approval  not allowed
";

const E301_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=1_000_000JPY

route
| amount       | -> check | decide  |
| <=100_000JPY | finance  | manager |

who
  finance = team finance
  manager = team managers

ticket
  self approval  not allowed
";
const E301_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=1_000_000円

route
| 金額         | -> 確かめ(check) | 決裁(decide) |
| <=100_000円  | 経理             | 上長         |

who
  経理(finance) = team finance
  上長(manager) = team managers

ticket
  self approval  not allowed
";

const E302_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=1_000_000JPY

route
| amount        | -> decide |
| <=100_000JPY  | manager   |
| >=100_000JPY  | head      |

who
  manager = team managers
  head    = team heads

ticket
  self approval  not allowed
";
const E302_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=1_000_000円

route
| 金額         | -> 決裁(decide) |
| <=100_000円  | 上長            |
| >=100_000円  | 部長            |

who
  上長(manager) = team managers
  部長(head)    = team heads

ticket
  self approval  not allowed
";

const E303_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=1_000_000JPY

route
| amount                     | -> decide |
| <=100_000JPY               | manager   |
| >100_000JPY <=1_000_000JPY | head      |
| >1_000_000JPY              | head      |

who
  manager = team managers
  head    = team heads

ticket
  self approval  not allowed
";
const E303_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=1_000_000円

route
| 金額                     | -> 決裁(decide) |
| <=100_000円              | 上長            |
| >100_000円 <=1_000_000円 | 部長            |
| >1_000_000円             | 部長            |

who
  上長(manager) = team managers
  部長(head)    = team heads

ticket
  self approval  not allowed
";

const W303_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  finance
  head   heads  when amount > 200_000JPY

who
  finance = team finance
  heads   = team heads

ticket
  self approval  not allowed
";
const W303_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理
  部長(head)     部長たち  when 金額 > 200_000円

who
  経理(finance)    = team finance
  部長たち(heads)  = team heads

ticket
  self approval  not allowed
";

const E306_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=1_000_000JPY

route
| amount       | -> decide |
| <=100_000JPY | manager   |
| >100_000JPY  | head      |

who
  manager = team managers
  head    = team heads

ticket
  self approval  not allowed

claims
  small_by_manager : decide is manager only when amount <= 50_000JPY
";
const E306_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=1_000_000円

route
| 金額         | -> 決裁(decide) |
| <=100_000円  | 上長            |
| >100_000円   | 部長            |

who
  上長(manager) = team managers
  部長(head)    = team heads

ticket
  self approval  not allowed

claims
  小さい額は上長が決める : 決裁 is 上長 only when 金額 <= 50_000円
";

const E601_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=1_000_000JPY

route
| amount       | -> decide |
| <=100_000JPY | manager   |
| >100_000JPY  | head      |

who
  manager = team managers
  head    = team heads

ticket
  self approval  not allowed

examples
| amount    | -> decide |
| 50_000JPY | head      |
";
const E601_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=1_000_000円

route
| 金額         | -> 決裁(decide) |
| <=100_000円  | 上長            |
| >100_000円   | 部長            |

who
  上長(manager) = team managers
  部長(head)    = team heads

ticket
  self approval  not allowed

examples
| 金額      | -> 決裁 |
| 50_000円  | 部長    |
";

const E403_EN: &str = "request example v1 \"Example\"

use calendar office from \"office.cal\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

every stage
  due in 3 business days at 09:00
  remind after 3 business days at 10:00

ticket
  self approval  not allowed

claims
  reminded_in_time : every stage reminds before it is due
";
const E403_JA: &str = "request 例(example) v1 \"例\"

use calendar 事務所(office) from \"office.cal\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

every stage
  due in 3 business days at 09:00
  remind after 3 business days at 10:00

ticket
  self approval  not allowed

claims
  期限の前に催促する : every stage reminds before it is due
";

const E404_EN: &str = "request example v1 \"Example\"

use calendar office from \"office.cal\"

form
  first_day : date \"First day\"  range >=2025-10-01 <=2027-12-31
  days      : number \"Business days\"  range >=1 <=20
  back_on   : date \"Back at work on\" = first_day + days business days in office

route
  check  manager

who
  manager = team managers

ticket
  self approval  not allowed
";
const E404_JA: &str = "request 例(example) v1 \"例\"

use calendar 事務所(office) from \"office.cal\"

form
  初日(first_day) : date \"初日\"  range >=2025-10-01 <=2027-12-31
  日数(days)      : number \"営業日数\"  range >=1 <=20
  復帰日(back_on) : date \"復帰日\" = 初日 + 日数 business days in 事務所

route
  確かめ(check)  上長

who
  上長(manager) = team managers

ticket
  self approval  not allowed
";

/// The data of one version of a small organization (fictional): the president at the top, a sales
/// department of two and a finance department of one.
const ORG_TEAMS: &[u8] = "id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
".as_bytes();
const ORG_PEOPLE: &[u8] = "id,name,name_ja
e01,President (fictional),社長（架空）
e02,Head of Sales,営業部長
e03,Sales staff,営業担当
e04,Head of Finance,財務部長
".as_bytes();
const ORG_MEMBERS: &[u8] = b"person,team,position,primary
e01,company,president,yes
e02,sales,dept_head,yes
e03,sales,staff,yes
e04,finance,dept_head,yes
";
const SMALL_EN: &str = "organization company v1 \"Company\"
description \"A small company (fictional)\"

positions
  staff     \"Staff\"
  dept_head \"Head of department\"
  president \"President\"

team kinds
  company    \"Company\"
  department \"Department\"

data \"org\"
";
const SMALL_JA: &str = "organization 会社(company) v1 \"会社\"
description \"小さな会社（架空）\"

positions
  担当(staff)     \"担当\"
  部長(dept_head) \"部長\"
  社長(president) \"社長\"

team kinds
  会社(company)  \"会社\"
  部(department) \"部\"

data \"org\"
";
/// The same organization, with no `data` line.
const NO_DATA_EN: Beside = &[("company.ringi", b"organization company v1 \"Company\"
description \"A small company (fictional)\"

positions
  staff     \"Staff\"
  dept_head \"Head of department\"
  president \"President\"

team kinds
  company    \"Company\"
  department \"Department\"
")];
const NO_DATA_JA: Beside = &[("会社.ringi", "organization 会社(company) v1 \"会社\"
description \"小さな会社（架空）\"

positions
  担当(staff)     \"担当\"
  部長(dept_head) \"部長\"
  社長(president) \"社長\"

team kinds
  会社(company)  \"会社\"
  部(department) \"部\"
".as_bytes())];
const SMALL_ORG_EN: Beside = &[("company.ringi", SMALL_EN.as_bytes()), ("org/2026-04-01/teams.csv", ORG_TEAMS), ("org/2026-04-01/people.csv", ORG_PEOPLE), ("org/2026-04-01/members.csv", ORG_MEMBERS)];
const SMALL_ORG_JA: Beside = &[("会社.ringi", SMALL_JA.as_bytes()), ("org/2026-04-01/teams.csv", ORG_TEAMS), ("org/2026-04-01/people.csv", ORG_PEOPLE), ("org/2026-04-01/members.csv", ORG_MEMBERS)];
/// The same, with a team of a kind the organization does not declare.
const ORG_DATA_SECTION: Beside = &[
    ("org/2026-04-01/teams.csv", "id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,section,company,e02
finance,Finance,財務部,department,company,e04
".as_bytes()),
    ("org/2026-04-01/people.csv", ORG_PEOPLE),
    ("org/2026-04-01/members.csv", ORG_MEMBERS),
];
/// The same, with a proxy for someone the organization does not have.
const ORG_DATA_PROXY: Beside = &[
    ("org/2026-04-01/teams.csv", ORG_TEAMS),
    ("org/2026-04-01/people.csv", ORG_PEOPLE),
    ("org/2026-04-01/members.csv", ORG_MEMBERS),
    ("org/proxies.csv", b"person,proxy,kind,from,to,requests
e09,e02,approve,2026-04-01,2026-06-30,
"),
];

const W500_EN: &str = "request example v1 \"Example\"

use organization org from \"company.ringi\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  manager

who
  manager = manager of requester

ticket
  self approval  not allowed
";
const W500_JA: &str = "request 例(example) v1 \"例\"

use organization 組織(org) from \"会社.ringi\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  上長

who
  上長(manager) = manager of requester

ticket
  self approval  not allowed
";

const E501_EN: &str = "organization example v1 \"Example\"
description \"A small company (fictional)\"

positions
  staff     \"Staff\"
  dept_head \"Head of department\"
  president \"President\"

team kinds
  company    \"Company\"
  department \"Department\"

data \"org\"
";
const E501_JA: &str = "organization 例(example) v1 \"例\"
description \"小さな会社（架空）\"

positions
  担当(staff)     \"担当\"
  部長(dept_head) \"部長\"
  社長(president) \"社長\"

team kinds
  会社(company)  \"会社\"
  部(department) \"部\"

data \"org\"
";

const E502_EN: &str = "request example v1 \"Example\"

use organization org from \"company.ringi\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  manager

who
  manager = manager of requester

ticket
  self approval  not allowed
";
const E502_JA: &str = "request 例(example) v1 \"例\"

use organization 組織(org) from \"会社.ringi\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  上長

who
  上長(manager) = manager of requester

ticket
  self approval  not allowed
";

const W502_EN: &str = "request example v1 \"Example\"

use organization org from \"company.ringi\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  head

who
  head = head of requester.department

every stage
  when empty escalate

ticket
  self approval  allowed
";
const W502_JA: &str = "request 例(example) v1 \"例\"

use organization 組織(org) from \"会社.ringi\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  部長

who
  部長(head) = head of requester.部

every stage
  when empty escalate

ticket
  self approval  allowed
";

const E503_EN: &str = "request example v1 \"Example\"

use organization org from \"company.ringi\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  decide  counsel

who
  counsel = person \"e09\"

every stage
  when empty escalate

ticket
  self approval  not allowed
";
const E503_JA: &str = "request 例(example) v1 \"例\"

use organization 組織(org) from \"会社.ringi\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  決裁(decide)  法務部長

who
  法務部長(counsel) = person \"e09\"

every stage
  when empty escalate

ticket
  self approval  not allowed
";

const E504_EN: &str = "request example v1 \"Example\"

use organization org from \"company.ringi\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  review  at least 2 of sales

who
  sales = team sales

ticket
  self approval  not allowed
";
const E504_JA: &str = "request 例(example) v1 \"例\"

use organization 組織(org) from \"会社.ringi\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確認(review)  at least 2 of 営業部

who
  営業部(sales) = team sales

ticket
  self approval  not allowed
";

const W506_EN: &str = "request example v1 \"Example\"

use organization org from \"company.ringi\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check   finance
  decide  head

who
  finance = team finance
  head    = person \"e04\"

ticket
  self approval  allowed
";
const W506_JA: &str = "request 例(example) v1 \"例\"

use organization 組織(org) from \"会社.ringi\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  財務
  決裁(decide)   財務部長

who
  財務(finance)    = team finance
  財務部長(head) = person \"e04\"

ticket
  self approval  allowed
";

const W507_EN: &str = "request example v1 \"Example\"

use organization org from \"company.ringi\"
use calendar office from \"office.cal\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  decide  sales_head

who
  sales_head = person \"e02\"

every stage
  due in 3 business days at end of day
  when late escalate, then expire

ticket
  self approval  allowed
";
const W507_JA: &str = "request 例(example) v1 \"例\"

use organization 組織(org) from \"会社.ringi\"
use calendar 事務所(office) from \"office.cal\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  決裁(decide)  営業部長

who
  営業部長(sales_head) = person \"e02\"

every stage
  due in 3 business days at end of day
  when late escalate, then expire

ticket
  self approval  allowed
";
const E508_EN: &str = "request example v1 \"Example\"

use organization org from \"company.ringi\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  submitted by   everyone in legal
  self approval  not allowed
";
const E508_JA: &str = "request 例(example) v1 \"例\"

use organization 組織(org) from \"会社.ringi\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  財務

who
  財務(finance) = team finance

ticket
  submitted by   everyone in legal
  self approval  not allowed
";

const E509_EN: &str = "request example v1 \"Example\"

use organization org from \"company.ringi\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  submitted by   team finance except position dept_head
  self approval  not allowed
";
const E509_JA: &str = "request 例(example) v1 \"例\"

use organization 組織(org) from \"会社.ringi\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  財務

who
  財務(finance) = team finance

ticket
  submitted by   team finance except position 部長
  self approval  not allowed
";

const W510_EN: &str = "request example v1 \"Example\"

use organization org from \"company.ringi\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  manager

who
  manager = manager of requester

every stage
  when empty escalate, then skip

ticket
  self approval  not allowed
";
const W510_JA: &str = "request 例(example) v1 \"例\"

use organization 組織(org) from \"会社.ringi\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  上長

who
  上長(manager) = manager of requester

every stage
  when empty escalate, then skip

ticket
  self approval  not allowed
";
/// The small organization, with the head of finance empty.
const ORG_TEAMS_NO_FINANCE_HEAD: &[u8] = "id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,
".as_bytes();
const SMALL_ORG_VACANT_EN: Beside = &[("company.ringi", SMALL_EN.as_bytes()), ("org/2026-04-01/teams.csv", ORG_TEAMS_NO_FINANCE_HEAD), ("org/2026-04-01/people.csv", ORG_PEOPLE), ("org/2026-04-01/members.csv", ORG_MEMBERS)];
const SMALL_ORG_VACANT_JA: Beside = &[("会社.ringi", SMALL_JA.as_bytes()), ("org/2026-04-01/teams.csv", ORG_TEAMS_NO_FINANCE_HEAD), ("org/2026-04-01/people.csv", ORG_PEOPLE), ("org/2026-04-01/members.csv", ORG_MEMBERS)];
/// The small organization, with the seat of the president (the head of the company) empty, and the
/// office's calendar beside it.
const ORG_TEAMS_NO_COMPANY_HEAD: &[u8] = "id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,
sales,Sales,営業部,department,company,e02
finance,Finance,財務部,department,company,e04
".as_bytes();
const SMALL_ORG_NO_HEAD_OFFICE_EN: Beside = &[
    ("company.ringi", SMALL_EN.as_bytes()),
    ("org/2026-04-01/teams.csv", ORG_TEAMS_NO_COMPANY_HEAD),
    ("org/2026-04-01/people.csv", ORG_PEOPLE),
    ("org/2026-04-01/members.csv", ORG_MEMBERS),
    ("office.cal", OFFICE_CAL),
];
const SMALL_ORG_NO_HEAD_OFFICE_JA: Beside = &[
    ("会社.ringi", SMALL_JA.as_bytes()),
    ("org/2026-04-01/teams.csv", ORG_TEAMS_NO_COMPANY_HEAD),
    ("org/2026-04-01/people.csv", ORG_PEOPLE),
    ("org/2026-04-01/members.csv", ORG_MEMBERS),
    ("office.cal", OFFICE_CAL),
];

const E107_EN: &str = "request example v1 \"Example\"

use masters m from \"masters.ringi\"

form
  account : choice from m.accounts \"Account code\"

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
";
const E107_JA: &str = "request 例(example) v1 \"例\"

use masters マスタ(m) from \"masters.ringi\"

form
  勘定科目(account) : choice from マスタ.accounts \"勘定科目\"

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
";
/// Account codes with no row for July 2026.
const ACCOUNTS: Beside = &[
    ("masters.ringi", b"masters company_masters v1
description \"The account codes a request chooses from (fictional)\"

master accounts \"Account codes\"
  key code
  fields
    code  : text \"Code\"  count <=10
    label : text \"Name\"  count <=60
  data \"accounts.csv\"
"),
    ("accounts.csv", b"code,label,valid_from,valid_to
5103,Supplies,2026-01-01,2026-06-30
5110,Supplies and small equipment,2026-08-01,
"),
];

/// A rule whose output names the approver: `manager` up to 50,000 yen, `cfo` above.
const AUTHORITY: Beside = &[("authority.rule", "rule authority v1
description \"Who decides, by the amount (fictional)\"

enum decider = manager \"Manager\" | cfo \"Chief financial officer\"

inputs
  amount : money[JPY]  range >=1JPY <=100_000JPY

outputs
  decider : decider

table decider
| amount      | -> decider |
| <=50_000JPY | manager    |
| >50_000JPY  | cfo        |
".as_bytes())];
/// A calendar of the business days.
const OFFICE_CAL: &[u8] = "calendar office v1
description \"Saturdays and Sundays are closed (fictional)\"
offset +09:00

closed weekly sat, sun
".as_bytes();
const OFFICE: Beside = &[("office.cal", OFFICE_CAL)];
/// A calendar whose holidays are known for 2026 and 2027 alone.
const FINITE: Beside = &[("office.cal", "calendar office v1
description \"Saturdays, Sundays and the holidays of 2026 and 2027 are closed (fictional)\"
offset +09:00

source holidays = file \"holidays.csv\" sha256:5927e83156e00064
  format csv
  covers 2026-01-01..2027-12-31

closed weekly sat, sun
closed holidays
".as_bytes()), ("holidays.csv", "2026-01-01,New Year's Day
2027-01-01,New Year's Day
".as_bytes())];

// the key is written in two parts, so that the source holds no value of a key's shape (the
// repository's secret scanning leaves the examples alone, not this file)
const E401_EN: &str = "request example v1 \"Example\"

use calendar office from \"office.cal\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check   finance
  decide  board

who
  finance = team finance
  board   = team board

every stage
  due in 3 business days at end of day
  when late escalate, then expire

ticket
  self approval  not allowed
  send back      to the requester, at most 2 times
  resubmit       within 5 business days
  pause          at most 4 business days

claims
  ends_in_time : every ticket ends within 30 business days
";
const E401_JA: &str = "request 例(example) v1 \"例\"

use calendar 事務所(office) from \"office.cal\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理
  決裁(decide)   取締役会

who
  経理(finance) = team finance
  取締役会(board) = team board

every stage
  due in 3 business days at end of day
  when late escalate, then expire

ticket
  self approval  not allowed
  send back      to the requester, at most 2 times
  resubmit       within 5 business days
  pause          at most 4 business days

claims
  期限のうちに終わる : every ticket ends within 30 business days
";
const W402_EN: &str = "request example v1 \"Example\"

use calendar office from \"office.cal\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

every stage
  due in 3 business days at end of day
  when late stay

ticket
  self approval  not allowed
";
const W402_JA: &str = "request 例(example) v1 \"例\"

use calendar 事務所(office) from \"office.cal\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

every stage
  due in 3 business days at end of day
  when late stay

ticket
  self approval  not allowed
";
const E405_EN: &str = "request example v1 \"Example\"

use calendar office from \"office.cal\"
use book budget from \"budget.book\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY
  dept   : text \"Department\"  count >=1 <=20

route
  check  finance

who
  finance = team finance

every stage
  due in 3 business days at end of day
  when late expire

ticket
  self approval  not allowed
  on submitted   hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: dept, amount: amount)
  on approved    post budget.reserve(request: ticket.id, attempt: ticket.attempt)
  on rejected    void budget.reserve(request: ticket.id, attempt: ticket.attempt)
";
const E405_JA: &str = "request 例(example) v1 \"例\"

use calendar 事務所(office) from \"office.cal\"
use book 予算(budget) from \"budget.book\"

form
  金額(amount) : money[JPY] \"金額\"  range >=1JPY <=100_000JPY
  部(dept)     : text \"部\"  count >=1 <=20

route
  確かめ(check)  経理

who
  経理(finance) = team finance

every stage
  due in 3 business days at end of day
  when late expire

ticket
  self approval  not allowed
  on submitted   hold 予算.reserve(request: ticket.id, attempt: ticket.attempt, dept: 部, amount: 金額)
  on approved    post 予算.reserve(request: ticket.id, attempt: ticket.attempt)
  on rejected    void 予算.reserve(request: ticket.id, attempt: ticket.attempt)
";
const E406_EN: &str = "request example v1 \"Example\"

use book budget from \"budget.book\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
  on approved    post budget.reserve(request: ticket.id, attempt: ticket.attempt)
";
const E406_JA: &str = "request 例(example) v1 \"例\"

use book 予算(budget) from \"budget.book\"

form
  金額(amount) : money[JPY] \"金額\"  range >=1JPY <=100_000JPY

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
  on approved    post 予算.reserve(request: ticket.id, attempt: ticket.attempt)
";
const E407_EN: &str = "request example v1 \"Example\"

use book budget from \"budget.book\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY
  dept   : text \"Department\"  count >=1 <=20

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
  on submitted   hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: dept, amount: amount)
  on approved    post budget.reserve(request: ticket.id, attempt: ticket.attempt)
  on rejected, withdrawn  void budget.reserve(request: ticket.id, attempt: ticket.attempt)
";
const E407_JA: &str = "request 例(example) v1 \"例\"

use book 予算(budget) from \"budget.book\"

form
  金額(amount) : money[JPY] \"金額\"  range >=1JPY <=100_000JPY
  部(dept)     : text \"部\"  count >=1 <=20

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
  on submitted   hold 予算.reserve(request: ticket.id, attempt: ticket.attempt, dept: 部, amount: 金額)
  on approved    post 予算.reserve(request: ticket.id, attempt: ticket.attempt)
  on rejected, withdrawn  void 予算.reserve(request: ticket.id, attempt: ticket.attempt)
";
const E408_EN: &str = "request example v1 \"Example\"

use book budget from \"budget.book\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY
  dept   : text \"Department\"  count >=1 <=20

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
  on submitted   hold budget.reserve(request: ticket.id, round: ticket.round, dept: dept, amount: amount)
  on approved    post budget.reserve(request: ticket.id, round: ticket.round)
  on rejected    void budget.reserve(request: ticket.id, round: ticket.round)
";
const E408_JA: &str = "request 例(example) v1 \"例\"

use book 予算(budget) from \"budget.book\"

form
  金額(amount) : money[JPY] \"金額\"  range >=1JPY <=100_000JPY
  部(dept)     : text \"部\"  count >=1 <=20

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
  on submitted   hold 予算.reserve(request: ticket.id, round: ticket.round, dept: 部, amount: 金額)
  on approved    post 予算.reserve(request: ticket.id, round: ticket.round)
  on rejected    void 予算.reserve(request: ticket.id, round: ticket.round)
";
const E409_EN: &str = "request example v1 \"Example\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  self approval  not allowed
  comment required  for reject, send back
";
const E409_JA: &str = "request 例(example) v1 \"例\"

form
  金額(amount) : money[JPY] \"金額\"  range >=1JPY <=100_000JPY

route
  確かめ(check)  経理

who
  経理(finance) = team finance

ticket
  self approval  not allowed
  comment required  for reject, send back
";
const E602_EN: &str = "request example v1 \"Example\"

use organization org from \"company.ringi\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check   finance
  decide  sales

who
  finance = team finance
  sales   = team sales

ticket
  self approval  allowed

scenario a_small_expense
  requester e03
  form amount = 5_000JPY
| at               | event   | by  | -> stage | status      |
| 2026-06-01 09:00 | submit  | e03 | check    | in progress |
| 2026-06-01 10:00 | approve | e04 |          | approved    |
";
const E602_JA: &str = "request 例(example) v1 \"例\"

use organization 組織(org) from \"会社.ringi\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理
  決裁(decide)   営業

who
  経理(finance) = team finance
  営業(sales)   = team sales

ticket
  self approval  allowed

scenario 小さな経費
  requester e03
  form 金額 = 5_000円
| at               | event   | by  | -> stage | status      |
| 2026-06-01 09:00 | submit  | e03 | 確かめ   | in progress |
| 2026-06-01 10:00 | approve | e04 |          | approved    |
";
const E603_EN: &str = "request example v1 \"Example\"

use organization org from \"company.ringi\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check   finance
  decide  sales

who
  finance = team finance
  sales   = team sales

ticket
  self approval  allowed

scenario a_small_expense
  requester e03
  form amount = 5_000JPY
| at               | event   | by  | -> stage | status      |
| 2026-06-01 09:00 | submit  | e03 | check    | in progress |
| 2026-06-01 10:00 | approve | e02 | decide   | in progress |
";
const E603_JA: &str = "request 例(example) v1 \"例\"

use organization 組織(org) from \"会社.ringi\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理
  決裁(decide)   営業

who
  経理(finance) = team finance
  営業(sales)   = team sales

ticket
  self approval  allowed

scenario 小さな経費
  requester e03
  form 金額 = 5_000円
| at               | event   | by  | -> stage | status      |
| 2026-06-01 09:00 | submit  | e03 | 確かめ   | in progress |
| 2026-06-01 10:00 | approve | e02 | 決裁     | in progress |
";
const E604_EN: &str = "request example v1 \"Example\"

use organization org from \"company.ringi\"
use calendar office from \"office.cal\"

form
  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

every stage
  due in 3 business days at end of day
  when late expire

ticket
  self approval  allowed

scenario nobody_answers
  requester e03
  form amount = 5_000JPY
| at               | event  | by  | -> stage | status      |
| 2026-06-01 09:00 | submit | e03 | check    | in progress |
| 2026-06-04 23:59 | late   |     |          | expired     |
";
const E604_JA: &str = "request 例(example) v1 \"例\"

use organization 組織(org) from \"会社.ringi\"
use calendar 事務所(office) from \"office.cal\"

form
  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円

route
  確かめ(check)  経理

who
  経理(finance) = team finance

every stage
  due in 3 business days at end of day
  when late expire

ticket
  self approval  allowed

scenario だれも答えない
  requester e03
  form 金額 = 5_000円
| at               | event  | by  | -> stage | status      |
| 2026-06-01 09:00 | submit | e03 | 確かめ   | in progress |
| 2026-06-04 23:59 | late   |     |          | expired     |
";
/// A book of budgets, which a request holds its amount in.
const BUDGET_BOOK: &[u8] = "book budget v1
description \"The budget of each department (fictional): a request holds its amount while it is on its way\"

unit JPY

account dept_budget(dept: string) : JPY
  description \"what the department has left to spend\"
  at least 0 refused as over_budget
account allocations : JPY outside
account payees : JPY outside

transfer allocate(dept: string, period: string, amount: JPY)
  description \"the budget a department is given for a period\"
  key dept, period
  move amount from allocations to dept_budget(dept)

transfer reserve(request: string, attempt: string, dept: string, amount: JPY)
  description \"held when a request is submitted, posted when it is approved, voided when it ends any other way\"
  key request, attempt
  pending expires after 90 days
  move amount from dept_budget(dept) to payees
".as_bytes();
const BUDGET: Beside = &[("budget.book", BUDGET_BOOK)];
/// The budget keyed by the request and its round, which tells no try from another (E408).
const BUDGET_BY_ROUND_BOOK: &[u8] = "book budget v1
description \"The budget of each department (fictional): a request holds its amount while it is on its way\"

unit JPY

account dept_budget(dept: string) : JPY
  description \"what the department has left to spend\"
  at least 0 refused as over_budget
account payees : JPY outside

transfer reserve(request: string, round: string, dept: string, amount: JPY)
  description \"held when a request is submitted, posted when it is approved, voided when it ends any other way\"
  key request, round
  pending expires after 90 days
  move amount from dept_budget(dept) to payees
".as_bytes();
const BUDGET_BY_ROUND: Beside = &[("budget.book", BUDGET_BY_ROUND_BOOK)];
const OFFICE_BUDGET: Beside = &[("office.cal", OFFICE_CAL), ("budget.book", BUDGET_BOOK)];
const SMALL_ORG_OFFICE_EN: Beside = &[("company.ringi", SMALL_EN.as_bytes()), ("org/2026-04-01/teams.csv", ORG_TEAMS), ("org/2026-04-01/people.csv", ORG_PEOPLE), ("org/2026-04-01/members.csv", ORG_MEMBERS), ("office.cal", OFFICE_CAL)];
const SMALL_ORG_OFFICE_JA: Beside = &[("会社.ringi", SMALL_JA.as_bytes()), ("org/2026-04-01/teams.csv", ORG_TEAMS), ("org/2026-04-01/people.csv", ORG_PEOPLE), ("org/2026-04-01/members.csv", ORG_MEMBERS), ("office.cal", OFFICE_CAL)];

const W901_EN: &str = concat!(
    "request example v1 \"Example\"\ndescription \"Read the maps of the offices with the API key of the staging site: AIzaSyD-ritsu-fake-",
    "key-for-tests-000000\"\n\nform\n  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY\n\nroute\n  check  finance\n\nwho\n  finance = team finance\n\nticket\n  self approval  not allowed\n"
);
const W901_JA: &str = concat!(
    "request 例(example) v1 \"例\"\ndescription \"事務所の地図は、検証用のサイトの API キーで読む: AIzaSyD-ritsu-fake-",
    "key-for-tests-000000\"\n\nform\n  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円\n\nroute\n  確かめ(check)  経理\n\nwho\n  経理(finance) = team finance\n\nticket\n  self approval  not allowed\n"
);

pub fn ledger() -> Ledger {
    let entries = vec![
        // ── Words, names and the files read ──
        e(
            "E001",
            tr!("読めない行です", "A line that does not read"),
            tr!(
                "行が ringi の書き方に合わないとき。知らない語や記号、閉じていない文字列や括弧、タブ、全角の空白、存在しない日付、ブロックの順の誤り、ファイルの種類に欠かせないブロック（申請の種類の `form`、`route`、`ticket` など）が無いとき、選ぶ値の無い `choice`、`key` か `fields` の無いマスタのときに出ます。`step` と書いた行には、`stage` と書くよう言います。",
                "A line does not follow how ringi is written: a word or a sign ringi does not have, a string or a bracket not closed, a tab, a full-width space, a date that does not exist, blocks out of their order, a block the kind of file cannot do without missing (a request's `form`, `route` and `ticket`), a `choice` with no values, or a master with no `key` or no `fields`. A line that says `step` is told to say `stage`."
            ),
            tr!(
                "示された位置を、メッセージと注のとおりに直してください。経路の一段は `stage`（ステップ）と書きます。",
                "Correct it where it points, as the message and the note say. A stage of a route is written `stage`."
            ),
            E001_EN,
            E001_JA,
            &["E006"],
        ),
        e(
            "E002",
            tr!("使えない名前です", "A name that cannot be used"),
            tr!(
                "名前が、条件を書く 7 語（`and` `or` `not` `is` `none` `true` `false`）のどれかのとき。フィールドの名前が、式が読む `requester` か `ticket` のとき。承認者の決め方の名前が、`when empty escalate, then …` の語の `skip` か `stay` のとき。ASCII でない名前の後ろに、括弧で ASCII の名前を書いていないとき、括弧の中が ASCII の名前でないとき。名前が、コードを生成する言語（TypeScript、Python、Go）の予約語のとき。フォームと明細の行のフィールドの ASCII の名前が、kaname の予約語（`then` `match` `some` など）のとき。名前が、kaname が生成するコードの補助のモジュールとその型の名前（`kaname` `KanameError` `Q` `Date` `Rat` `glue`）のとき。ASCII の名前が `_` だけのとき。申請の種類の ASCII の名前が、`_` で始まるか `_` で終わるとき、`_` が二つ続くとき。フォームの二つのフィールド（明細の一つの行の二つの欄も）の ASCII の名前が、Go に生成するコードで同じ名前になるとき（`a_b` と `aB` はどちらも `AB`）。名前の付いた表の入力の列が、フィールドと同じ名前で、行に承認者の決め方を書いているとき。承認者の決め方の名前が、いくつかの決め方やその割合を書いたステップの値（`a and b` の `a_and_b`、`all of legal` の `all_of_legal`）と同じとき。最後の三つは、重なる名前の一つがチケットの JSON のキーか値なので、ringi がほかの名前に置き換えられません。",
                "A name is one of the seven words conditions are written with (`and` `or` `not` `is` `none` `true` `false`); a field is named `requester` or `ticket`, which expressions read; a way to choose an approver is named `skip` or `stay`, the words of `when empty escalate, then …`; a name that is not ASCII has no ASCII name in brackets after it, or one in brackets that is not an ASCII name; a name is a reserved word of a language the code is generated in (TypeScript, Python, Go); a field of the form or of a row has as its ASCII name a word kaname reserves (`then` `match` `some` and others); a name is one the code kaname generates keeps for its helper module and its types (`kaname` `KanameError` `Q` `Date` `Rat` `glue`); an ASCII name is `_` alone; the ASCII name of a kind of request starts or ends with `_`, or has two `_` in a row; two fields of the form, or of one row, have ASCII names that are one name in the code generated for Go (`a_b` and `aB` are both `AB`); an input column of a named table has the name of a field, and its rows name ways to choose an approver; or a way to choose an approver has the name of the value a stage takes for several ways or a share of one (`a and b` is `a_and_b`, `all of legal` is `all_of_legal`). In the last three, one of the names is a key or a value of a ticket's JSON, which ringi cannot write under another name."
            ),
            tr!(
                "別の名前にしてください。日本語の名前には、`金額(amount)` のように、すぐ後ろの括弧に ASCII の名前を書いてください。生成するコードと JSON は、その ASCII の名前を使います。表の列なら、承認者の決め方を書く列に、どのフィールドにも無い名前を付けてください。申請の種類の名前は、英字で始め、英字か数字で終え、`_` を続けないでください。下ろしたファイルと生成するコードの名前の頭になります。",
                "Choose another name. Give a name that is not ASCII an ASCII name in brackets right after it, as in `金額(amount)`: the generated code and the JSON take that name. For a column of a table, give the column of the ways to choose a name no field has. Start the name of a kind of request with a letter, end it with a letter or a digit, and write no two `_` in a row: it begins the names of the files it is lowered to and of the generated code."
            ),
            E002_EN,
            E002_JA,
            &["E003"],
        ),
        e(
            "E003",
            tr!("同じ名前が二度宣言されています", "A name declared twice"),
            tr!(
                "同じ置き場（フォームのフィールド、選ぶ値、明細の列、経路のステップ、`who`、表とその列、`use` の別名、主張、シナリオ、役職、チームの種類）に、同じ名前か同じ ASCII の名前を二度宣言したとき。フィールドの名前が `use` の別名と同じとき、同じ言語の `labels` を二度書いたとき、`labels` に同じ項目を二度書いたときにも出ます。",
                "A name, or an ASCII name, is declared twice in one place names are kept: the fields of the form, the values of a choice, the columns of rows, the stages of the route, `who`, the tables and their columns, the aliases of the `use` lines, the claims, the scenarios, the positions and the kinds of team. A field named as a `use` line's alias, `labels` of one language written twice, and one item labelled twice in a `labels` are this too."
            ),
            tr!("どちらかの名前を変えるか、二つを一つにまとめてください。", "Rename one of the two, or make them one."),
            E003_EN,
            E003_JA,
            &["E002", "E004"],
        ),
        e(
            "E004",
            tr!("宣言の無い名前です", "A name that is not declared"),
            tr!(
                "宣言の無い名前を読んだとき。フィールド、選ぶ値（`choice` の値と、`choice from` が選ぶ規則の列挙の値）、ステップ、`who` の承認者の決め方、表とその列、`use` の別名、マスタとそのフィールド、組織の役職とチームの種類のどれでもない名前です。`manager of requester` や `requester.department` のように組織を読むものを、`use organization` の無いファイルに書いたときと、カレンダーを読む営業日の数え方を、カレンダーを決められないファイルに書いたときにも出ます。同じ種類の同じ名前を何か所にも書いたときは、最初に書いた所で一度だけ出て、ほかの行の番号を注に挙げます（明細の列は明細ごと、マスタのフィールドはマスタごと）。",
                "A name is read that nothing declares: none of the fields, the values to choose from (a `choice`'s, and those of the rule's enum a `choice from` chooses from), the stages, the ways to choose approvers in `who`, the tables and their columns, the aliases of the `use` lines, the masters and their fields, the positions and the kinds of team of the organization. Something that reads the organization (`manager of requester`, `requester.department`) in a file with no `use organization`, and business days counted where no calendar can be told, are this too. A name of one kind written in several places is said once, where it is first written, and the note gives the lines of the others (a column, once for each rows; a field of a master, for each master)."
            ),
            tr!(
                "注に並べた名前から選ぶか、その名前を宣言してください。組織を読むなら `use organization` を、営業日を数えるなら `use calendar` を書いてください。",
                "Choose one of the names the note lists, or declare it. To read the organization, write `use organization`; to count business days, `use calendar`."
            ),
            E004_EN,
            E004_JA,
            &["E003", "E308"],
        ),
        e(
            "E005",
            tr!("読めない `use` です", "A `use` that cannot be read"),
            tr!(
                "`use` の行のファイルを読めないとき。ファイルが無いか読めない、拡張子が `use` の種類と合わない（`use calendar` に `.book`）、`.ringi` や `.cal` の最初の語が種類と合わない（`use organization` に申請の種類のファイル）、パスがプロジェクトの外を指す、パスが絶対パス（`/` や、ドライブの文字で始まる）のときに出ます。",
                "The file of a `use` line cannot be read: it is not there or cannot be opened, its extension is not that of the kind of `use` (`use calendar` with a `.book`), the first word of a `.ringi` or a `.cal` is not that of the kind (`use organization` with a request), the path leads out of the project, or it is an absolute path (one that starts with `/` or a drive letter)."
            ),
            tr!(
                "パスと種類を直してください。パスは、そのファイルのあるディレクトリから書きます。種類と拡張子は、`organization`・`masters`・`route` が `.ringi`、`calendar`・`dates` が `.cal`、`book` が `.book`、`flow` が `.flow`、`rule` が `.rule`、`gate` が `.gate` です。",
                "Correct the path or the kind. A path is written from the directory of the file it is in, and the kinds read: `organization`, `masters` and `route` a `.ringi`, `calendar` and `dates` a `.cal`, `book` a `.book`, `flow` a `.flow`, `rule` a `.rule`, `gate` a `.gate`."
            ),
            E005_EN,
            E005_JA,
            &["E202"],
        ),
        e(
            "E006",
            tr!("この種類のファイルには書けないブロックです", "A block this kind of file does not take"),
            tr!(
                "ファイルの種類が持たないブロックを書いたとき（組織のファイルの `route`、マスタのファイルの `form` など）。一度だけ書くブロックを二度書いたとき、`ticket` に一度だけ書く行（`on` のほかの行。`self approval`、`submitted by` など）を二度書いたとき、ファイルの種類を言う行を二度書いたとき、共有の経路のファイルが `use route` でほかの経路を読むときにも出ます。",
                "A block is written that the kind of file does not have (`route` in an organization, `form` in masters). A block a file takes once written twice, a line a `ticket` takes once (any but `on`: `self approval`, `submitted by` and the rest) written twice, a second line saying the kind of file, and a shared route that reads another route with `use route` are this too."
            ),
            tr!(
                "そのブロックを、それを持つ種類のファイルに移すか、消してください。注に、その種類のファイルが持つブロックを順に並べます。経路を組み合わせるのは申請の種類のファイルです。二度書いたブロックや `ticket` の行は、一つにまとめてください。",
                "Move the block to a kind of file that has it, or take it out; the note lists the blocks of the kind at hand, in their order. Routes are put together in a request's file. A block or a line of `ticket` written twice is made one."
            ),
            E006_EN,
            E006_JA,
            &["E001"],
        ),
        e(
            "E007",
            tr!("何も指していない文言です", "A label for nothing"),
            tr!(
                "`labels` の項目が、ファイルに無いものを指すとき。無いフィールド、選ぶ値、明細の列、経路に無いステップ（`stage …`）、`who` に無い承認者の決め方（`who …`）、申請の種類でないファイルの `request` です。",
                "An item of `labels` is for something the file does not have: a field, a value to choose from or a column of rows there is not, a stage the route does not have (`stage …`), a way to choose approvers `who` does not declare (`who …`), or `request` in a file that is not a request."
            ),
            tr!(
                "項目の名前を、ファイルの中の名前に合わせて直すか、要らない行を消してください。",
                "Make the item name what the file names, or take the line out."
            ),
            E007_EN,
            E007_JA,
            &["E004", "E703"],
        ),
        e(
            "E008",
            tr!("ほかの言語が要るファイルです。`ritsu ringi` で確かめてください", "A file that needs another language, given to ringi's own binary"),
            tr!(
                "ほかの言語を持たない ringi のクレートのバイナリに、ほかの言語が要るファイルを渡したとき。申請の種類は kaname のモジュールに、フォームと経路は rulec の規則に、期限は koyomi の日付に下ろすので、どれも要ります。`use` で規則、カレンダー、帳簿、flow、gate を読むファイルも同じです。要る言語ごとに一つ、その言語を最初に要する行に出て、2 で終わります。組織とマスタのファイルは、ほかの言語が要りません。",
                "A file that needs another language is given to the binary of ringi's own crate, which holds none. A request is lowered to a module of kaname's, its form and route to rulec's rules and its deadlines to koyomi's dates, so it needs them; so does a file whose `use` lines read a rule, a calendar, a book, a flow or a gate. It is said once for each language needed, at the first line that needs it, and the exit is 2. An organization and masters need no other language."
            ),
            tr!(
                "同じコマンドを、すべての言語をつないだ `ritsu ringi`（`ritsu ringi check <ファイル>`）で走らせてください。",
                "Run the same command through ritsu, with every language joined: `ritsu ringi check <file>`."
            ),
            E008_EN,
            E008_JA,
            &[],
        ),
        e(
            "E009",
            tr!("型の合わない値です", "A value that does not fit where it is written"),
            tr!(
                "値の型が、書いた場所の型と合わないとき。単位の違う数を比べる・足す（`amount > 30_000` は、円の金額と単位の無い数）、条件に真偽でない値を書く、`range` を文字に・`count` を数に・`open in` を日付でないものに書く、範囲の端に値でないものや型の違う値を書く、明細の列を `sum` で囲まずに読むときなどに出ます。",
                "A value is not of the type where it is written: numbers of different units compared or added (`amount > 30_000` is an amount of yen and a number with no unit), a condition that is not true or false, `range` on a text, `count` on a number, `open in` on what is not a date, an end of a range that is no value or one of another type, or a column of rows read without `sum`."
            ),
            tr!(
                "値を、書いた場所の型に合わせてください。金額には単位を付けて書きます（`30_000JPY`）。",
                "Write a value of the type the place takes. An amount is written with its unit (`30_000JPY`)."
            ),
            E009_EN,
            E009_JA,
            &["E004", "E101"],
        ),
        // ── The form ──
        e(
            "E101",
            tr!("範囲の書かれていない数、金額、日付です", "A number, an amount or a date with no range"),
            tr!(
                "申請者か承認者が入れる数、金額、日付のフィールドに、`range` の上下の端が無いとき。明細（`rows`）の行の数に上限が無いときにも出ます。範囲は、確かめる値の範囲と、画面と生成するコードが受け付ける値になります。計算するフィールドには要りません。",
                "A number, an amount or a date someone fills in has no `range`, or one without its two ends; or `rows` have no upper limit on their number. The range is what the checks reason over and what the screens and the generated code take. A computed field needs none."
            ),
            tr!(
                "`range >=1 <=20` のように、両方の端を書いてください。明細には `count >=1 <=20` のように行の数の上限を書いてください。",
                "Write both ends, as in `range >=1 <=20`; for rows, the most there may be, as in `count >=1 <=20`."
            ),
            E101_EN,
            E101_JA,
            &["E102", "E009"],
        ),
        e(
            "E102",
            tr!("長さの上限の書かれていない文です", "A text with no limit on its length"),
            tr!(
                "申請者か承認者が入れる文（`text`、`long text`）に、`count` の上限が無いとき。上限が無いと、どれだけ長い文でも受け付けることになります。",
                "A text someone fills in (`text`, `long text`) has no upper limit in `count`: with none, a text of any length is taken."
            ),
            tr!("`count <=100` のように、入れられる文字の数の上限を書いてください。", "Write the most characters it takes, as in `count <=100`."),
            E102_EN,
            E102_JA,
            &["E101"],
        ),
        e(
            "E103",
            tr!("計算した値が一度も入らない範囲です", "A range a computed field never comes to"),
            tr!(
                "計算するフィールドに書いた範囲と、計算した値がとりうる範囲（読むフィールドの範囲から ringi が数えます）が、少しも重ならないとき（`total = price * quantity` が 1 万円までなのに、`range >=20_000JPY <=30_000JPY`）。計算するフィールドに書いた範囲は、受け付ける値の上下の端です。計算した値が範囲の外になる申請は出せず、画面と生成するコードが、人が入れるフィールドの範囲と同じように断ります。そのフィールドを読む経路の表、条件、ほかの計算、主張は、書いた範囲の中だけを見ます。範囲が、とりうる範囲より広くても、一部だけ重なっても、誤りではありません。どの申請も出せない範囲だけが、このエラーです。",
                "The range written on a computed field and the values its computation can come to (which ringi works out from the ranges of what it reads) do not meet at all (`total = price * quantity` comes to 10,000 yen at most, and the range says `>=20_000JPY <=30_000JPY`). The range written on a computed field is the ends of what it takes: a request whose computed value falls outside it cannot be made, and the screens and the generated code refuse it as they refuse a value outside the range of a field someone fills in; what reads the field (the route's table, the conditions, the other computations, the claims) looks only inside the range. A range wider than what the computation comes to, or one that meets it in part, is no mistake: only a range no request can meet is this error."
            ),
            tr!(
                "範囲を、計算した値がとりうる範囲と重なるように直すか、読むフィールドの範囲を直してください。",
                "Move the range so that it meets what the computation comes to, or correct the ranges of what it reads."
            ),
            E103_EN,
            E103_JA,
            &["E101", "E104"],
        ),
        e(
            "E104",
            tr!("丸め方の書かれていない計算です", "A computed amount that does not say how it rounds"),
            tr!(
                "計算するフィールドの式が、そのフィールドの刻みで割り切れない値になることがあるのに、`round` が無いとき（円の金額に `* 3%` を掛ける、整数の数を割る）。",
                "A computed field's expression can come to a value finer than the field's step, and the field has no `round`: an amount of yen times `3%`, a whole number divided."
            ),
            tr!(
                "`round down(1JPY)` のように、丸め方（`up`、`down`、`half_up`、`half_down`、`half_even`）と刻みを書いてください。",
                "Say how it rounds and to what, as in `round down(1JPY)` (`up`, `down`, `half_up`, `half_down`, `half_even`)."
            ),
            E104_EN,
            E104_JA,
            &["E009", "E103"],
        ),
        e(
            "E105",
            tr!("隠れていることのある必須のフィールドです", "A required field that may be hidden"),
            tr!(
                "必須のフィールドが、表示の条件によっては隠れているのに必須になるとき（`required when amount > 30_000JPY` と `shown when category is travel`）。申請者は、見えないフィールドを入れられません。二つの条件を下ろした規則で rulec に尋ね、そうなる申請の例を添えます。rulec が決められないときは、W201 で決められないと言います。",
                "A required field can be required while its `shown when` hides it (`required when amount > 30_000JPY` and `shown when category is travel`): the requester cannot fill in what is not shown. rulec is asked through a rule the two conditions are lowered to, and such a request is given. When rulec cannot settle it, W201 says so."
            ),
            tr!(
                "必須の条件を表示の条件の中に収めるか、表示の条件を広げてください。",
                "Make the field required only where it is shown, or show it more widely."
            ),
            E105_EN,
            E105_JA,
            &["E106", "W201"],
        ),
        e(
            "E106",
            tr!("入る前のフィールドを読む条件です", "A condition that reads a field before it is filled"),
            tr!(
                "申請のときに決めること（フィールドの表示と必須の条件、経路の表とステップの条件、共有の経路に渡す値、`on submitted` の行）が、承認者だけが入れるフィールド（`approvers only`）を、そのまま、または計算を通して読むとき。申請のとき、そのフィールドはまだ空です。",
                "What is decided when the request is made (when a field is shown or required, the route's table and the conditions of its stages, what a shared route is given, the `on submitted` lines) reads a field only approvers fill in (`approvers only`), itself or through a computed field. When the request is made, that field is empty."
            ),
            tr!(
                "申請のときに決めることは、申請者が入れるフィールドだけを読むようにしてください。承認者が決めることなら、それを読むステップの指名に移してください。",
                "Have what is decided when the request is made read only fields the requester fills in; what approvers decide belongs where they choose, at a later stage."
            ),
            E106_EN,
            E106_JA,
            &["W108", "E305"],
        ),
        with_files(
            "E107",
            tr!("選べる行の無い日があるマスタです", "A choice from a master with no row on some day"),
            tr!(
                "申請を使う日の範囲の中に、`choice from` や `lookup` で読むマスタの行が一つも効いていない日があるとき（勘定科目の行が 2026 年 6 月で切れ、次の行が 8 月から）。使う日の範囲は、`effective from`（無ければ、読むデータがそろう最初の日）から、読むデータが続く最後の日までです。診断は、穴の日と、前後の行の有効期間をどう直せば埋まるかを言います。データの終わりは誤りにせず、`check` が「いつまで使えるか」として言います。`check --through <日付>` を渡すと、その日の前にマスタの行が終わることもこのエラーです。",
                "On some day of the range the request is used in, no row of a master it chooses from or looks up (`choice from`, `lookup`) is in effect: the account codes end in June 2026, and the next row starts in August. The range runs from `effective from` (else the first day every piece of data it reads is there) to the last day it is all there. The diagnostic gives the days of the hole, and how the rows before and after it could be made to fill it. The end of the data is no mistake: `check` says through which day the request can be used. With `check --through <day>`, a master whose rows end before that day is this error too."
            ),
            tr!(
                "その日にも効く行を足すか、前後の行の `valid_to`・`valid_from` を延ばして、穴を埋めてください。`--through` なら、その日まで効く行を足してください。",
                "Add a row in effect on those days, or extend the `valid_to` or the `valid_from` of the rows around them; for `--through`, add rows in effect through that day."
            ),
            (E107_EN, ACCOUNTS),
            (E107_JA, ACCOUNTS),
            &["E404", "E505"],
        ),
        e(
            "W108",
            tr!("承認者が直すと経路が変わるフィールドです", "A field approvers may edit that the route reads"),
            tr!(
                "`approvers edit` で承認者が直せるフィールドを、経路（表の列、ステップの条件、共有の経路に渡す値、`lookup … by`、`chosen by requester in`）が、そのまま、または計算を通して読むとき。直したときは、今のステップより後の経路を直した値で決め直し、前のステップの答えは変えません。",
                "A field approvers may edit (`approvers edit`) is read by the route, itself or through a computed field: by a column of the table, the condition of a stage, what a shared route is given, `lookup … by` or `chosen by requester in`. When it is edited, the route after the stage at hand is decided again with the new value, and the answers of the stages before it stay."
            ),
            tr!(
                "経路が変わってよいなら、そのままでかまいません。変わってはいけないなら、`approvers edit` からそのフィールドを外してください。",
                "If the route may change, leave it; if it may not, take the field out of `approvers edit`."
            ),
            W108_EN,
            W108_JA,
            &["E106"],
        ),
        e(
            "W109",
            tr!("秘密の値を入れるフィールドです", "A field that asks for a secret"),
            tr!(
                "文のフィールドの名前、ASCII の名前、題が、秘密の値を指す語（`password`、`secret`、`token`、`api key`、パスワード、暗証番号など）を含むとき。入れた値はチケットの履歴に残り、承認者と、ページとログを読める人に見えます。",
                "The name, the ASCII name or the title of a text field has a word for a secret in it (`password`, `secret`, `token`, `api key`, and their Japanese words). What is filled in stays in the ticket's history, where approvers and whoever reads the pages and the logs see it."
            ),
            tr!(
                "秘密の値は申請で受け取らず、それを扱う仕組み（鍵の置き場、本人が入れる画面）で渡してください。秘密でない値なら、名前と題を、秘密に読めない言い方にしてください。",
                "Do not take a secret through a request: pass it through what is made for it (a secret store, a screen the person fills in themselves). If the value is no secret, name and title the field so that it does not read as one."
            ),
            W109_EN,
            W109_JA,
            &["W901"],
        ),
        // ── The languages it is lowered to and reads ──
        e(
            "E201",
            tr!("中に書いた物が、下ろした先の言語の検査を通りません", "Something written inside does not pass the language it is lowered to"),
            tr!(
                "ringi のファイルを下ろした規則か日付に、ringi のコードに直せないエラーが出たとき（計算するフィールドの値が int64 に収まらない。rulec の E108）。元の言語のコードと文を注に書きます。下ろしたファイルを置くパス（ファイルの隣の `<名前>.lowered/`）に、ディスクのファイルがあるときにも出ます。`ritsu gen` が申請のために置くコード（申請ごとのつなぎと、チケットの機械）に kaname が何か言ったとき（警告も、total でない関数も）にも、申請の最初の行に出ます。これは ringi の不具合で、注には kaname のコードと行だけを書きます。",
                "The rules or the dates a file is lowered to give an error ringi has no code of its own for (a computed field whose value does not fit in int64: rulec's E108); the language's code and words are in the notes. A file on the disk where ringi lays what it lowers (`<name>.lowered/` beside the file) is this too. So is anything kaname says of the code `ritsu gen` lays for a request (its glue, and the machine of the tickets), a warning or a function that is not total too, said at the request's first line: that is a bug in ringi, and the notes give kaname's codes and lines alone."
            ),
            tr!(
                "注の元のコードと文を読んで、示された行を直してください。`.lowered/` にファイルがあるときは、そのファイルを消すか、別の名前にしてください。`ritsu gen` が置くコードについてのものは、申請を直さず、申請と ritsu の版を添えて ringi の不具合として知らせてください。",
                "Read the language's code and words in the notes, and correct the line it points to. For a file in `.lowered/`, remove it or rename it. For the code `ritsu gen` lays, leave the request as it is and report a bug in ringi, with the request and the version of ritsu."
            ),
            E201_EN,
            E201_JA,
            &["W201", "E202"],
        ),
        e(
            "W201",
            tr!("中に書いた物に、下ろした先の言語が警告を出します", "The language something written inside is lowered to warns of it"),
            tr!(
                "ringi のファイルを下ろした規則か日付に、ringi のコードに直せない警告が出たとき（フィールドの ASCII の名前が、コードを生成する言語の予約語で、生成するコードでは後ろに `_` が付く。rulec の W121）。元の言語のコードと文を注に書きます。主張と、必須なのに見せていないことがあるか（E105 の問い）を、下ろした規則で決められなかったときにも出ます。",
                "The rules or the dates a file is lowered to warn of something ringi has no code of its own for (the ASCII name of a field is a keyword of a language the code is generated in, and the generated code writes it with `_` after it: rulec's W121); the language's code and words are in the notes. A claim, or whether a field may be asked for while it is hidden (the question of E105), that the rule it is lowered to cannot settle is this too."
            ),
            tr!(
                "注を読んで、それでよいか確かめてください。よくなければ、示された行を直してください（名前なら、別の ASCII の名前にします）。",
                "Read the notes, and see that this is what is meant; if not, correct the line it points to (for a name, choose another ASCII name)."
            ),
            W201_EN,
            W201_JA,
            &["E201", "E105"],
        ),
        with_files(
            "E202",
            tr!("`use` で読んだファイルが検査を通りません", "A file read with `use` does not pass its check"),
            tr!(
                "`use` で読んだファイルが、その言語の検査を通らないとき。ringi のファイル（組織、マスタ、共有の経路）は ringi の、規則は rulec の、カレンダーと日付のファイルは koyomi の、帳簿は chobo の、flow は dandori の、gate は sekisho の検査です。そのファイルのエラーを三つまで注に書きます。`ritsu check` では、この行は読んだファイルが通るのを待っている行として、そのファイルの誤りの下にまとめて出ます。",
                "A file a `use` line reads does not pass its language's check: ringi's for a file of ringi's (an organization, masters, a shared route), rulec's for a rule, koyomi's for a calendar or a dates file, chobo's for a book, dandori's for a flow, sekisho's for a gate. Up to three of its errors are in the notes. In `ritsu check`, this line is one that waits for that file to pass, and it is printed under the mistake in that file."
            ),
            tr!("読んだファイルを、その言語の検査が通るように直してください。", "Correct the file read until its language's check passes."),
            (E202_EN, COMPANY_EN),
            (E202_JA, COMPANY_JA),
            &["E005"],
        ),
        with_files(
            "E203",
            tr!("規則の出力の値に、承認者の決め方がありません", "A value of a rule's output that names no approver"),
            tr!(
                "経路が `use rule` の規則の出力で承認者を決めるとき、その出力の列挙の値に、`who` の承認者の決め方が無いとき（`authority.decider` の値 `cfo` が `who` に無い）。出力の型は規則から読みます。承認者を決める出力が列挙でないときは E009、規則に無い出力は E004 です。",
                "A route takes its approvers from the output of a rule it reads with `use rule`, and a value of that output names no way to choose approvers in `who` (the value `cfo` of `authority.decider`). The output's type is read from the rule: an output that names approvers and is no enum is E009, and an output the rule does not have E004."
            ),
            tr!("`who` に、その値の名前で承認者の決め方を足してください。", "Add a way to choose approvers to `who`, by that value's name."),
            (E203_EN, AUTHORITY),
            (E203_JA, AUTHORITY),
            &["E004"],
        ),
        // ── The route ──
        e(
            "E301",
            tr!("経路の表に当てはまらない申請があります", "No row of the route takes some requests"),
            tr!(
                "経路の表のどの行にも当てはまらない申請があるとき（購買の 100 万円を超える行が無い）。表を下ろした規則で rulec が見つけます。当てはまらない申請の範囲をフィールドの名前と単位で言い、その一つを例に添え、足す行を表の形で示します。行の入力のセルは、表がすでに書いている区切りと範囲の端だけで切ってあり、ステップのセル（経路を選ぶ表なら経路のセル）は `?` です。`?` は検査を通らないので、貼っただけの行は、答えを書くまでそこで止まります。範囲が 8 か所を超えるときは、最初の 8 か所の行を示し、残りは次の検査で言います。",
                "Some requests fall in no row of the route's table (no row for purchases over one million yen). rulec finds it in the rule the table is lowered to. The requests no row takes are said by the fields' names and units, with one of them as an example, and the rows to add are shown in the table's form: their input cells are cut only at the bounds the table already writes and at the ends of the ranges, and the cells of the stages (of the route, in a table that chooses one) are `?`. A `?` does not pass the check, so a row pasted as it is stops there until its answers are written. Past eight parts, the rows of the first eight are shown, and the next check names the rest."
            ),
            tr!(
                "示された行を表に足し、`?` に、そのステップに答える承認者（経路を選ぶ表なら共有の経路）を書いてください。行の途中で答えが変わるなら、そこで行を分けてください。",
                "Add the rows shown to the table, writing in place of each `?` who answers that stage (the shared route, in a table that chooses one); split a row where the answer changes inside it."
            ),
            E301_EN,
            E301_JA,
            &["E302", "E303"],
        ),
        e(
            "E302",
            tr!("経路の表の二つの行に当てはまる申請があります", "Two rows of the route take one request"),
            tr!(
                "一つの申請が、経路の表の二つの行に当てはまるとき（`<=100_000JPY` と `>=100_000JPY`）。表を下ろした規則で rulec が見つけ、両方に当てはまる例を添えます。両方に当てはまる申請が一つでなければ、その範囲も言います（二つの行が同じ申請を取るときと、片方に当てはまる申請がどれももう片方にも当てはまるときは、そう言います）。",
                "One request falls in two rows of the route's table (`<=100_000JPY` and `>=100_000JPY`). rulec finds it in the rule the table is lowered to, and an example is given. Where the two share more than one request, what they share is said too (or that the two take the same requests, or that one takes every request the other does)."
            ),
            tr!("端の片方を `<` か `>` にして、行が重ならないようにしてください。", "Make one of the ends `<` or `>`, so that the rows do not overlap."),
            E302_EN,
            E302_JA,
            &["E301"],
        ),
        e(
            "E303",
            tr!("どの申請も当てはまらない経路の表の行です", "A row of the route no request reaches"),
            tr!(
                "経路の表の行に、どの申請も当てはまらないとき（`amount` の範囲が 100 万円までなのに、`>1_000_000JPY` の行）。表を下ろした規則で rulec が見つけます。",
                "No request falls in a row of the route's table (a row `>1_000_000JPY`, where `amount` goes up to one million yen). rulec finds it in the rule the table is lowered to."
            ),
            tr!("行の区切りを直すか、その行を消してください。", "Correct the row's bounds, or take the row out."),
            E303_EN,
            E303_JA,
            &["E301", "W303"],
        ),
        e(
            "W303",
            tr!("どの申請も届かないステップの承認者です", "An approver of a stage no request reaches"),
            tr!(
                "経路のステップの承認者に、どの申請も届かないとき。ステップの条件が、読むフィールドの範囲の中では成り立たない（`amount` が 10 万円までなのに `when amount > 200_000JPY`）、`only when` の主張だけが指す承認者のときです。経路を下ろした規則で rulec が見つけます。`possible` や例だけが指す承認者は、その主張か例が成り立たないこと（E306、E601）で言います。",
                "No request reaches an approver of a stage: the stage's condition never holds within the ranges of what it reads (`when amount > 200_000JPY`, where `amount` goes up to 100,000 yen), or only an `only when` claim names the approver. rulec finds it in the rule the route is lowered to. An approver only a `possible` or an example names is told by the claim or the example that does not hold (E306, E601)."
            ),
            tr!("行やステップの条件を直すか、どの申請も届かない物を経路から消してください。", "Correct the conditions of the rows or of the stage, or take out of the route what no request reaches."),
            W303_EN,
            W303_JA,
            &["E303", "E306"],
        ),
        e(
            "E304",
            tr!("議決に要る数が、承認者の数を超えます", "A quorum larger than the approvers it counts"),
            tr!(
                "`at least 2 of …` のように二人以上を求める議決が、一人を決める承認者の決め方（`person`、`manager of requester`、`head of`、`nearest`、`chosen by`、`lookup`）を数えるとき。`at least …%` が 100% を超えるときにも出ます。",
                "A quorum of two or more (`at least 2 of …`) counts a way to choose that names one person (`person`, `manager of requester`, `head of`, `nearest`, `chosen by`, `lookup`); or `at least …%` is more than 100%."
            ),
            tr!(
                "人数を数える議決は、`team legal` のようにチームで決める承認者に使ってください。一人に答えてもらうなら、議決をやめて名前だけを書いてください。",
                "Count a quorum over a team (`team legal`); for one person to answer, write the way to choose alone."
            ),
            E304_EN,
            E304_JA,
            &[],
        ),
        e(
            "E305",
            tr!("前のステップが指名できない指名です", "A choice the stage before cannot make"),
            tr!(
                "`chosen by previous approver` のステップより前に、条件の無い承認のステップが無いとき（最初のステップ）。前の承認者が指名するのに、申請者が入れるフィールドで指名するときと、申請者が指名するのに、承認者だけが入れるフィールドで指名するときにも出ます。",
                "A stage `chosen by previous approver` has no stage before it that always approves (it is the first). A person the approver before chooses in a field the requester fills in, and one the requester chooses in a field only approvers fill in, are this too."
            ),
            tr!(
                "前の承認者が指名するなら、そのステップの前に承認のステップを置き、指名するフィールドに `approvers only` を書いてください。申請者が指名するなら `chosen by requester in …` と書いてください。",
                "For the approver before to choose, put an approving stage before it and write `approvers only` on the field chosen in; for the requester to choose, write `chosen by requester in …`."
            ),
            E305_EN,
            E305_JA,
            &["E106"],
        ),
        e(
            "E306",
            tr!("経路の主張が成り立ちません", "A claim about the route that does not hold"),
            tr!(
                "`claims` の `only when` や `possible` が、経路の表では成り立たないとき（`decide is manager only when amount <= 50_000JPY` に、5 万 1 円で上長の行）。下ろした規則に rulec が尋ね、成り立たない例を添えます。",
                "An `only when` or a `possible` of the `claims` does not hold of the route (`decide is manager only when amount <= 50_000JPY`, with a row that has the manager decide 50,001 yen). rulec is asked through the rule the route is lowered to, and a counterexample is given."
            ),
            tr!("例を読んで、表か主張のどちらが誤りかを決めて直してください。", "Read the example, and correct whichever is wrong: the table or the claim."),
            E306_EN,
            E306_JA,
            &["E601", "W303"],
        ),
        with_files(
            "E307",
            tr!("共有の経路に、受け取らない引数を渡しています", "A shared route given what it does not take"),
            tr!(
                "`route 名前(…)` で共有の経路に渡す値が、その経路の `inputs` と合わないとき。受け取らない名前、同じ名前の二度、型の違う値、渡していない入力（`optional` でないもの）です。経路の表のセルが、入力を受け取る共有の経路を使うときにも出ます。",
                "What a shared route is given in `route name(…)` does not match its `inputs`: a name it does not take, a name given twice, a value of another type, or an input not given that is not `optional`. A cell of the route's table that uses a shared route taking inputs is this too: a cell gives it nothing."
            ),
            tr!(
                "注に並べた入力の名前と型で渡してください。入力を受け取る共有の経路は、`route 名前(入力: …)` のように一行で使ってください。",
                "Give what the note lists, by its names and types; use a shared route that takes inputs on a line of its own, as in `route name(input: …)`."
            ),
            (E307_EN, STANDARD_EN),
            (E307_JA, STANDARD_JA),
            &["E009"],
        ),
        e(
            "E308",
            tr!("経路に無いステップです", "A stage the route does not have"),
            tr!(
                "`withdraw until`、主張、`examples` の列、シナリオの行が、経路に無いステップを指すとき（`withdraw until counsel` で、経路に `counsel` が無い）。同じ名前を何か所にも書いたときは、最初に書いた所で一度だけ出て、ほかの行の番号を注に挙げます。",
                "`withdraw until`, a claim, a column of the `examples` or a line of a scenario names a stage the route does not have (`withdraw until counsel`, and no `counsel` in the route). A name written in several places is said once, where it is first written, and the note gives the lines of the others."
            ),
            tr!("注に並べたステップの名前から選んでください。", "Choose one of the stages the note lists."),
            E308_EN,
            E308_JA,
            &["E004"],
        ),
        e(
            "E309",
            tr!("経路かフォームが読めない条件です", "A condition the route or the form cannot read"),
            tr!(
                "経路の条件（ステップの `when`、経路の表の列、共有の経路に渡す値、`only when` の主張）か、フォームの条件（`shown when`、`required when`）が、表に下ろせない物を読むとき。申請者（`requester.department`、`requester.id`）、チケット（`ticket.round`）、文や人やマスタから選ぶフィールド、数か日付でない計算するフィールド、二つの値どうしの比べ（`amount > limit`）、条件の中の計算（`sum(items.price) > 100_000JPY`）で出ます。経路とフォームの条件は、読む値を列にした表に下ろして rulec が確かめるので、表の列にならない物は読めません。値の無いことがある物（任意のフィールド、`none` になることのある規則の出力、共有の経路の入力のうち申請が渡していない物）は、値の無いことを `none` という値の一つとして読みます（DESIGN 4.9）。検査を通った申請の種類には、いつも下ろした経路があります。",
                "A condition of the route (a stage's `when`, a column of the route's table, what a shared route is given, an `only when` claim) or of the form (`shown when`, `required when`) reads what a table cannot hold: the requester (`requester.department`, `requester.id`), the ticket (`ticket.round`), a field of text, of a person or chosen from a master, a computed field that is not a number or a date, two values compared with each other (`amount > limit`), or a computation in the condition (`sum(items.price) > 100_000JPY`). The conditions are lowered to a table of the values they read, which rulec checks, and what is no column of a table cannot be read. What may have no value (a field that may be empty, an output of a rule that may be none, an input of a shared route the request does not give) is read with `none` as one of its values (DESIGN 4.9). A request that passes the check always has its route lowered."
            ),
            tr!(
                "数どうしの比べや計算は、計算するフィールドに置いて、そのフィールドを値と比べてください（`difference : money[JPY] = amount - limit` として `when difference > 0JPY`）。文のフィールドで分けるなら `choice` にしてください。申請を出せる人を絞るなら `ticket` に `submitted by` を、組織のいちばん上の人の申請をだれかに承認させるなら上に人がいないときの回し先を書いてください（`who` に `board = team board` と宣言して `when empty escalate, then board`）。",
                "Put a comparison of two numbers, or a computation, in a computed field, and compare that field with a value (`difference : money[JPY] = amount - limit`, then `when difference > 0JPY`). To decide by a field of text, make it a `choice`. To let only some people submit the request, write `submitted by` under `ticket`; to have someone approve the requests of the person at the top of the organization, write where a stage goes when there is no one above (`when empty escalate, then board`, with `board = team board` in `who`)."
            ),
            E309_EN,
            E309_JA,
            &["E009", "E307", "E502"],
        ),
        // ── The ticket and its deadlines ──
        with_files(
            "E401",
            tr!("主張の日数のうちに終わらないことがあるチケットです", "A ticket that may not end within what the claim says"),
            tr!(
                "`every ticket ends within … business days` が成り立たないとき。チケットが長くていくつ営業日かかるかを数えます。ステップは期限の日数、期限を過ぎて上へ回すなら回すたびに同じ日数（期限が日の終わりなら、回す日の一日も）です。経路のいちばん長い並びのステップを足し（条件で来るステップは、来るものとして数えます）、戻せる回数だけくり返し、戻るたびに `resubmit within` の日数を足し、`pause at most` の日数を足します。数え方を添えます。数は上限なので、主張が数以上なら必ず成り立ちます。数のとおりにかかるチケットが、あるとは限りません。終わらないことのあるチケット（期限を過ぎても待つステップ、期限の無いステップ、`resubmit within` の無い差し戻し）も、このエラーです。",
                "`every ticket ends within … business days` does not hold. The most a ticket can take is counted: a stage takes its deadline, and the deadline again each time it goes up when it is late (with the day it goes up on, where the deadline is at the end of a day); the stages of the longest path of the route are added, a stage that comes on a condition counted as coming; that is taken once more for each time the ticket may go back, with the requester's `resubmit within` each time; and the `pause at most` is added. The count is given with how it is made. It is the most that can be, so a claim it meets holds for every ticket; a ticket that takes every one of those days may not exist. A ticket that may never end is this too: a stage that stays when it is late, a stage with no deadline, a ticket that goes back with no `resubmit within`."
            ),
            tr!(
                "主張を添えた数にするか、期限、戻せる回数、保留の日数を減らしてください。終わらない待ちには、終わりを書いてください（`expire` か `escalate`、`due in …`、`resubmit within …`）。",
                "Write the count in the claim, or shorten the deadlines, the times it goes back or the pause; give a wait that has no end one (`expire` or `escalate`, `due in …`, `resubmit within …`)."
            ),
            (E401_EN, OFFICE),
            (E401_JA, OFFICE),
            &["W402"],
        ),
        with_files(
            "W402",
            tr!("いつまでも待つことのあるステップです", "A stage that may wait for ever"),
            tr!(
                "期限を過ぎても待ち続けるステップがあるとき（`when late stay`、`when late escalate, then stay`）。期限を書いてあるのに、チケットはそこで終わりません。`every ticket ends within` の主張があれば E401 です。期限の無いステップと、`resubmit within` の無い差し戻しも終わりの無い待ちですが、警告にはせず、`ok` の下の行（`may never end: …`）で言います。",
                "A stage stays when it is late (`when late stay`, `when late escalate, then stay`): it has a deadline, and the ticket does not end at it. With a claim that every ticket ends within some days, it is E401. A stage with no deadline and a ticket that goes back with no `resubmit within` wait with no end too; no warning is given for them, and the line under `ok` says so (`may never end: …`)."
            ),
            tr!(
                "待ってよいなら、そのままでかまいません。終わらせるなら、`stay` を `expire` か `escalate` にしてください。",
                "If it may wait, leave it; for it to end, write `expire` or `escalate` in place of `stay`."
            ),
            (W402_EN, OFFICE),
            (W402_JA, OFFICE),
            &["E401"],
        ),
        with_files(
            "E403",
            tr!("期限より前に来ない催促があります", "A reminder that does not come before its deadline"),
            tr!(
                "`claims` に `every stage reminds before it is due` を書いたのに、催促が期限と同じか後に来ることがあるとき（`remind after 3 business days at 10:00` と `due in 3 business days at 09:00`）。下ろした日付で koyomi が、数え始める日のすべてについて確かめ、いちばん外れる日を添えます。催促と期限を別のカレンダーで数えるステップ（`due in 2 business days in office at end of day` と `remind after 2 business days in plant at 09:00`）は、ringi がステップの始まりうる日を一日ずつ数えて確かめ、最初に外れる日と、外れる日の数を添えます。",
                "`claims` say `every stage reminds before it is due`, and a reminder may come at its deadline or after it (`remind after 3 business days at 10:00` with `due in 3 business days at 09:00`). koyomi checks it in the dates the deadlines are lowered to, for every day a stage can start on, and the day farthest off is given. A stage whose reminder and deadline are counted in two calendars (`due in 2 business days in office at end of day` with `remind after 2 business days in plant at 09:00`) is checked by ringi, which counts both for every day a stage can start on; the first such day and how many days are so are given."
            ),
            tr!(
                "催促を早めるか、期限を遅らせてください。別のカレンダーで数えているなら、催促と期限を同じカレンダーで数える直し方もあります。",
                "Make the reminder earlier, or the deadline later; where the two are counted in two calendars, counting them in one is a fix too."
            ),
            (E403_EN, OFFICE),
            (E403_JA, OFFICE),
            &["E404"],
        ),
        with_files(
            "E404",
            tr!("使う日の中に、カレンダーのデータが無い日があります", "Days of use the calendar's data does not cover"),
            tr!(
                "期限やフォームの日付を営業日で数え始める日が、カレンダーの休みの表が始まるより前に来ることがあるとき。`effective from` が表の最初の日より前のときと、営業日で数えるフォームの日付（`back_on = first_day + days business days in office`）の数え始める日付の範囲が、表より前から始まるときです（表は 2026 年からで、`first_day` は 2025 年 10 月から）。表の終わりは誤りにせず、`check` が「いつまで使えるか」と、それを決めるカレンダーを言います（それより後に始まるステップや、それより後の日付から数えるフォームは、足りないデータを言って断ります）。`check --through <日付>` を渡すと、その日の前に表が終わることもこのエラーです。",
                "A day the deadlines or a date of the form are counted from in business days can come before the calendar's table of closed days starts: `effective from` before the first day of the table, or a date of the form counted in business days (`back_on = first_day + days business days in office`) whose date counted from may come before it (the table starts in 2026, and `first_day` in October 2025). The end of the table is no mistake: `check` says through which day the request can be used, and which calendar decides it (a stage that starts later, or a date of the form counted from a later day, is refused with what data is missing). With `check --through <day>`, a table that ends before that day is this error too."
            ),
            tr!(
                "カレンダーの休みの表を、その日まで前へ広げるか、`effective from` や数え始める日付の範囲を、表が始まる日からにしてください。`--through` なら、その日のあとまで表を足してください。",
                "Extend the calendar's table of closed days back to that day, or make `effective from` or the range of the date counted from start when the table does; for `--through`, extend the table past that day."
            ),
            (E404_EN, FINITE),
            (E404_JA, FINITE),
            &["E403", "E107"],
        ),
        with_files(
            "E405",
            tr!("確定も取消もされずに残るか、申請の値と食い違うことのある仮押さえがあります", "A hold an ending may leave open, or an edit may leave unlike the request"),
            tr!(
                "`on submitted hold` で押さえた仮押さえが、開いたまま迎えることのある終わり方（承認、却下、差し戻し、取り下げ、期限切れ）のどれかで、確定も取消もされないとき（`on expired` に `void` が無い）。`when` の付いた `post` と `void` は、仮押さえに同じ `when` が付いているときだけ数えます。`submitted` 以外で押さえる `hold` も、閉じる行が無いので、このエラーです。仮押さえの条件（`when`）か引数が、承認者の直せるフィールド（`approvers edit`）を、そのまま、または計算するフィールドか規則の出力を通して読むときも、このエラーです。仮押さえは、申請を出したときの値で一度だけ行い、承認者が直しても押さえ直しません。額を直せば、確定される額が申請の額と食い違い、条件が成り立たなくなれば、同じ条件の `post` と `void` が行われず、仮押さえが開いたまま終わります。",
                "A hold made with `on submitted hold` is neither posted nor voided by some ending the ticket can come to while the hold is open: approved, rejected, sent back, withdrawn, expired (there is no `void` on `expired`). A `post` or a `void` with a `when` of its own counts only where the hold has the same `when`. A `hold` on any ending but `submitted` is this too: no line closes it. So is a hold whose condition (`when`) or whose argument reads a field approvers may edit (`approvers edit`), itself or through a computed field or an output of a rule. A hold is made once, with the values the request is submitted with, and is not made again when an approver edits them: an amount edited is not the amount posted, and once the condition fails, a `post` or a `void` on the same condition is not done and the hold is left open."
            ),
            tr!(
                "その終わり方に `post` か `void` の行を足してください。承認者の直せるフィールドを読む仮押さえは、そのフィールドを `approvers edit` から外してください。",
                "Add a `post` or a `void` line for that ending. Where a hold reads a field approvers may edit, take the field out of `approvers edit`."
            ),
            (E405_EN, OFFICE_BUDGET),
            (E405_JA, OFFICE_BUDGET),
            &["E406", "E407"],
        ),
        with_files(
            "E406",
            tr!("先に仮押さえの無い確定か取消です", "A post or a void with no hold before it"),
            tr!(
                "`post` か `void` の振替に、先に `hold` した仮押さえが無いとき（`on approved post` だけで `on submitted hold` が無い）。",
                "A `post` or a `void` has no `hold` before it (`on approved post`, and no `on submitted hold`)."
            ),
            tr!("`on submitted hold` を足すか、その行を消してください。", "Add `on submitted hold`, or take the line out."),
            (E406_EN, BUDGET),
            (E406_JA, BUDGET),
            &["E405"],
        ),
        with_files(
            "E407",
            tr!("このチケットには起きない終わり方の処理です", "An action on an ending this ticket never has"),
            tr!(
                "チケットの決まりでは起きない終わり方に `on` の行があるとき。`send back` の行が無いのに `sent back`。`withdraw` の行も `send back` の行も無いのに `withdrawn`（戻ったチケットは申請者がやめられるので、`send back` があれば起きます）。期限で切れるステップも `resubmit within` も無いのに `expired`。人が答えるステップが無いのに `rejected`。",
                "An `on` line is for an ending the ticket never has: `sent back` with no `send back` line; `withdrawn` with neither a `withdraw` line nor a `send back` line (a ticket that has gone back can be given up by its requester, so `send back` alone allows it); `expired` where no stage expires at a deadline and no turn of the requester at `resubmit within`; `rejected` where no stage is answered by a person."
            ),
            tr!("その行を消すか、その終わり方を許してください。", "Take the line out, or allow that ending."),
            (E407_EN, BUDGET),
            (E407_JA, BUDGET),
            &["E405"],
        ),
        with_files(
            "E408",
            tr!("試みを見分けないキーの仮押さえです", "A hold keyed by nothing that tells one try from another"),
            tr!(
                "仮押さえの振替のキーが、試みを見分ける値（`ticket.attempt`）を受け取らないとき（キーが申請と `round` だけ）。帳簿はキーごとに答えを覚え（断りも覚えます）、取り消したキーではもう押さえません。サーバーが帳簿をチケットと別のデータベースに置くと、帳簿の断りはチケットが保存されなくても残ります。キーが申請と `round` だけなら、値を変えた出し直しや予算が増えたあとの出し直しも、同じ回のうちは同じキーで断られ、保存できずに取り消した仮押さえのキーに、あとの試みが当たって何も押さえません。`ticket.attempt` は、サーバーが `submit` と `resubmit` に渡す試みの名前で、同じ操作の送り直しには同じ名前、新しい操作には新しい名前です。chobo が帳簿を読めるときに確かめます。",
                "The key of the transfer a hold makes takes nothing that tells one try from another (`ticket.attempt`): it is the request and its `round` alone. A book remembers its answer for each key, a refusal too, and holds nothing again for a key it voided. Where a server keeps the book in a database apart from the tickets, a refusal of the book stays though the ticket is not saved: keyed by the request and its `round`, a request made again with other values, or once the budget has grown, is refused by the same key for the rest of the round, and a later try comes to the key of a hold voided because its ticket was not saved, and holds nothing. `ticket.attempt` is the name of the try the server gives a `submit` and a `resubmit`: the same name for the same operation sent again, a new one for a new operation. Checked where chobo reads the book."
            ),
            tr!(
                "振替に試みの引数を足してキーに入れ（`attempt: string` と `key request, attempt`）、`hold`、`post`、`void` に `attempt: ticket.attempt` を渡してください。",
                "Add a parameter for the try to the transfer and to its key (`attempt: string`, `key request, attempt`), and pass `attempt: ticket.attempt` to the `hold`, the `post` and the `void`."
            ),
            (E408_EN, BUDGET_BY_ROUND),
            (E408_JA, BUDGET_BY_ROUND),
            &["E405", "E406"],
        ),
        e(
            "E409",
            tr!("このチケットには無い操作に理由を求めています", "A reason asked with an operation this ticket never has"),
            tr!(
                "`comment required for` が、チケットの決まりでは起きない操作を挙げたとき。`send back` の行が無いのに `send back`。人が答えるステップが無いのに `reject`。E407 と同じ見方で決めます。",
                "`comment required for` names an operation the ticket never has: `send back` with no `send back` line; `reject` where no stage is answered by a person. Decided as E407 decides an ending."
            ),
            tr!("その操作を行から消すか、その操作を許す行を書いてください。", "Take the operation out of the line, or write the line that allows it."),
            E409_EN,
            E409_JA,
            &["E407", "E006"],
        ),
        // ── The organization ──
        with_files(
            "W500",
            tr!("組織のデータが無いので、承認者が空くかを決められません", "No organization data, so missing approvers are not decided"),
            tr!(
                "申請の種類が読む組織のファイルに `data` の行が無いか、その置き場にデータが無く、`--org` も無いとき。承認者のいないステップ、届かない議決、二つのステップに当たる同じ人は決めずに、ほかの検査だけを行います。通ったことにはせず、この警告で言います。マスタのファイルが、人のフィールドの行を、組織と比べられないとき（組織を読まないか、読んだ組織にデータが無い）にも出ます。",
                "The organization a request reads has no `data` line, or no data where it points, and no `--org` is given: a stage with no approver, a quorum out of reach and one person at two stages are left undecided, the other checks are made, and this warning says so rather than letting it pass as checked. A file of masters whose fields of people cannot be checked against an organization (it reads none, or one with no data) is this too."
            ),
            tr!("組織のファイルに `data` の行を書いてデータを置くか、`--org` でデータのディレクトリを渡してください。", "Write a `data` line in the organization's file and put the data there, or give the data's directory with `--org`."),
            (W500_EN, NO_DATA_EN),
            (W500_JA, NO_DATA_JA),
            &["E501"],
        ),
        with_files(
            "E501",
            tr!("読めない組織のデータです", "Organization data that does not read"),
            tr!(
                "組織のデータが読めないとき。列が足りない、知らない役職やチームの種類、無いチームや人を指す行、`parent` の輪、長がそのチームの人でない、主の所属が一人に一つでない、同じ ID が二度、版のディレクトリの名前が日付でない、などです。上限を越える値（ID やマスタのキーは 255 バイト、名前は 200 字、マスタの値は 1,000 字、アドレスは 254 バイト）、サーバーが断る文字（制御文字、書式の文字、行や段落を区切る文字）、形の合わないアドレス、たたむと同じになる二人のアドレス、`organization.json` の最上位と要素の中の知らないキー、CSV の読み方の誤り（列とは違う個数の値、大きすぎる行、引用符の誤り）もです。データのファイルの行で言い、その版を読む申請の種類は、組織のファイルが通るのを待ちます（E202）。マスタのデータの誤り（列が足りない、キーの重なり、型の合わない値）も、マスタのファイルの行でこのエラーです。",
                "The organization's data does not read: a column missing, a position or a kind of team that is not declared, a row naming a team or a person that is not there, a ring of `parent`, a head who is not in the team, a person with no primary team or two, an ID twice, a version's directory not named by a date; also a value over its limit (IDs and master keys are 255 bytes, names 200 characters, a master's values 1,000, addresses 254 bytes), a character the server refuses (control, format, line or paragraph separators), an address of the wrong shape, two addresses of one version that fold to the same, a key `organization.json` does not know at its top or inside an element, and a CSV the reader refuses (a row of a different number of values, a row too big, a misplaced quote). It is said at the row of the data's file, and the requests that read the organization wait for it to pass (E202). A master's data that does not read (a column missing, two rows of one key on one day, a value its field does not take) is this error too, at the row of the master's file."
            ),
            tr!("示されたデータの行を直してください。", "Correct the rows of the data it points to."),
            (E501_EN, ORG_DATA_SECTION),
            (E501_JA, ORG_DATA_SECTION),
            &["W500", "E202"],
        ),
        with_files(
            "E502",
            tr!("組織のある版で、承認者のいないステップがあります", "A stage with no approver in a version of the organization"),
            tr!(
                "組織のある版とある申請者で、ステップに答える人がいないのに、チケットがそのステップで待ち続けるとき。`when empty stay`（書かなければこれ）のステップと、`when empty escalate` で上に回せる人がいない（組織のいちばん上）ステップのうち、`then skip` も `then <決め方>` も書いていないものと、`then <決め方>` の決め方にも人がいないものです（社長の申請の `manager of requester`、長の空いたチーム、自己承認を許さないのに申請者しかいない、組織にいない人）。版ごと、ステップごとに一つにまとめ、当たる申請者の数と三人までの名前を言います。`effective from` の日に効いている版が無いときも、このエラーです。",
                "In a version of the organization, for some requester, no one answers a stage, and the ticket would wait there: the stage is `when empty stay` (the default), or `when empty escalate` with nobody above to go up to (the top of the organization), where it says neither `then skip` nor `then <way>`, or the way it names finds nobody either. `manager of requester` for the president, a team whose head is empty, the requester alone where self approval is not allowed, someone not in the organization. One diagnostic is said for each version and stage, with how many requesters it comes to and up to three of them by name. No version in effect on the day of `effective from` is this too."
            ),
            tr!(
                "版のデータを直すか、ステップに `when empty escalate`（上へ回す）か `when empty skip`（飛ばす）を書いてください。上に回せる人がいないときは、飛ばしてよいなら `when empty escalate, then skip` と書き、だれかが見るべきなら、`then` のあとに、その人の申請に答える承認者の決め方を書いてください（`who` に `board = team board` と宣言して `when empty escalate, then board`）。`then` のあとの決め方に人がいなければ、どの版でも人のいる決め方にしてください。",
                "Correct the data of the version, or write `when empty escalate` (go up) or `when empty skip` for the stage. Where nobody is above, write `when empty escalate, then skip` if the stage may be skipped; if someone must see the request, write after `then` the way to choose who answers it (`when empty escalate, then board`, with `board = team board` in `who`). Where the way after `then` finds nobody, make it a way that has someone in every version."
            ),
            (E502_EN, SMALL_ORG_EN),
            (E502_JA, SMALL_ORG_JA),
            &["W502", "E503"],
        ),
        with_files(
            "W502",
            tr!("承認者がいないので、ほかへ回るか飛ばすステップがあります", "A stage that goes elsewhere or is skipped for lack of an approver"),
            tr!(
                "組織のある版とある申請者で、版のデータが欠けているために承認者のいないステップが、`when empty escalate` で上の長へ回るか、`when empty skip` で飛ばされるとき。上へたどった先の長の席が空いていて、`then skip` で飛ばされるときと、`then <決め方>` の決め方へ回るときもです。データの欠けは、長の空いたチーム、人のいないチーム、版にいない人かチーム、上へたどる途中の空いた長の席です。空いた席を書いたとおりに埋めてよいのか、データの誤りなのかは人が決めることなので、警告にします。組織の形でほかへ回るか飛ばされるステップ（承認者が申請者だけで自己承認を許さない、申請者が組織のいちばん上にいる、上に決め方の種類のチームが無い）は、書いたとおりに回るので警告にせず、`ok` の下の行で申請者の数を言います。だれの申請がどこへ回るかは `ringi simulate --every-requester` で見られます。回る先の人ごとに申請者をまとめて言います。",
                "In a version of the organization, for some requester, a stage with no approver because the data has a gap goes up to a head above with `when empty escalate`, or is skipped with `when empty skip`; or, for the seat of the head it goes up to is empty, it is skipped with `then skip`, or goes to the way of `then <way>`. A gap in the data is a team whose head is empty, a team with nobody in it, a person or a team the version does not have, or the empty seat of a head on the way up. Whether the seat is to be filled as written or the data is wrong is for a person to say, so it is a warning. A stage that goes elsewhere or is skipped by the shape of the organization (the only approver is the requester, and self approval is not allowed; the requester is at the top; there is no team of the kind above) goes as it is written, and is not warned of: the line under `ok` says for how many requesters, and `ringi simulate --every-requester` shows whose requests go where. The requesters are said by the person the stage goes to."
            ),
            tr!("版のデータを直すか、そのままでよいかを確かめてください。", "Correct the data of the version, or make sure this is meant."),
            (W502_EN, SMALL_ORG_VACANT_EN),
            (W502_JA, SMALL_ORG_VACANT_JA),
            &["E502", "W510"],
        ),
        with_files(
            "E503",
            tr!("組織のある版にいない人かチームを、上へ回すステップが指しています", "A stage that goes up names someone a version does not have"),
            tr!(
                "`when empty escalate` のステップの承認者の決め方が、組織のある版にいない人（`person \"e09\"`）か無いチーム（`team legal`）を指すとき。いない人やチームからは、上へたどる元が無いので回せません。承認者の決め方の行で言います。`when empty stay` なら E502、`skip` なら W502 です。`then skip` や `then <決め方>` を書いても、このエラーです（どちらも、上に人がいないときにだけ効きます）。マスタの行の人が組織にいないときは E505 です。",
                "The way to choose the approver of a `when empty escalate` stage names a person (`person \"e09\"`) or a team (`team legal`) a version of the organization does not have: there is nothing to go up from. It is said at the line of the way to choose. With `when empty stay` it is E502, with `skip` W502; with `then skip` or `then <way>` it is this too, for both hold only where nobody is above. A person of a master's row who is not in the organization is E505."
            ),
            tr!("ID を直すか、その版のデータに人かチームを足してください。", "Correct the ID, or add the person (the team) to the data of the version."),
            (E503_EN, SMALL_ORG_EN),
            (E503_JA, SMALL_ORG_JA),
            &["E502", "E505"],
        ),
        with_files(
            "E504",
            tr!("議決に届かない申請者がいます", "A quorum some requesters cannot reach"),
            tr!(
                "組織のある版とある申請者で、議決（`at least 2 of sales`）に要る数だけの承認者がいないとき（申請者自身を除くと、チームの人数が足りない）。",
                "In a version of the organization, for some requester, the approvers a quorum counts (`at least 2 of sales`) are fewer than it needs: the team, without the requester, is too small."
            ),
            tr!("議決の数を減らすか、チームに人を足してください。", "Lower the quorum, or add people to the team."),
            (E504_EN, SMALL_ORG_EN),
            (E504_JA, SMALL_ORG_JA),
            &["E304"],
        ),
        with_files(
            "E505",
            tr!("組織にいない人の代理か、マスタの行です", "A proxy or a master row for someone not in the organization"),
            tr!(
                "代理の表（`proxies.csv`）の代わられる人か代わる人、またはマスタの行の人のフィールドの人が、その行の期間に効く組織の版にいないとき（辞めた人の代理）。行と版ごとに、データのファイルの行で言います。マスタの行を確かめるのは、マスタのファイルが `use organization` で組織を読むときです。",
                "The person or the proxy of a row of the proxies (`proxies.csv`), or the person of a field of people of a master's row, is not in a version of the organization in effect in the row's days (a proxy for someone who has left). It is said at the row of the data's file, for each row and version. A master's rows are checked when its file reads the organization with `use organization`."
            ),
            tr!("その行を直すか、期間を終わらせてください。", "Correct the row, or end it."),
            (E501_EN, ORG_DATA_PROXY),
            (E501_JA, ORG_DATA_PROXY),
            &["E503"],
        ),
        with_files(
            "W506",
            tr!("一つのチケットの二つのステップに同じ人が当たります", "One person at two stages of one ticket"),
            tr!(
                "組織のある版とある申請者で、別の承認者の決め方の二つのステップに、同じ一人だけが当たるとき（上長と部長が同じ人。チームの人が、決裁する人一人だけ）。一人が二度確かめても、二人が確かめたことにはなりません。当たる人ごとに申請者をまとめて言います。上へ回した先の人と、何人かのうちの一人で足りるステップは数えません。`ticket` に `same approver approves once` と書いた申請の種類では、あとのステップがすぐ前のステップの承認で済む組は言わず、済まない組には、済まない理由（あいだのステップ、前のステップのほかの部分、エージェントのステップ）を添えます。`same approver approves once, at the last stage` と書いた申請の種類では、前のステップがあとのステップの承認を待つので、エージェントが先に答えるステップとの組だけを言います。書いていないか、書いた行では一度にならない組には、一度になる書き方を添えます。",
                "In a version of the organization, for some requester, one person alone answers two stages chosen in different ways (the manager is the head of the department too; the only member of the team is the one who decides). One person checking twice is not two pairs of eyes. The requesters are said by the one person. Whom a stage goes up to, and a stage any one of several may answer, are not counted. In a request whose `ticket` says `same approver approves once`, a pair whose later stage is done with the approval of the stage right before it is not said, and a pair it does not do is said with why (a stage between them, another part of the earlier stage, an agent's stage). In one that says `same approver approves once, at the last stage`, the earlier stage waits for the approval of the later, and only a pair with a stage an agent answers first is said. Where neither is written, or the line written does not make it one approval, the pair is said with the writing that does."
            ),
            tr!(
                "どちらかのステップの承認者の決め方を変えるか、一度の承認でよいなら `ticket` に `same approver approves once`（すぐ前のステップの承認で済ませる）か `same approver approves once, at the last stage`（あいだにほかのステップがあっても、最後のステップで一度だけ答える）と書くか、そのままでよいかを確かめてください。",
                "Change the way to choose one of the stages; if one approval is enough, write `same approver approves once` under `ticket` (done with the approval of the stage right before) or `same approver approves once, at the last stage` (one answer at the last of the stages, whatever comes between them); or make sure it is meant."
            ),
            (W506_EN, SMALL_ORG_EN),
            (W506_JA, SMALL_ORG_JA),
            &["E502"],
        ),
        with_files(
            "W507",
            tr!("期限を過ぎても、上へ回す先のないステップがあります", "A late stage with nowhere to go up to"),
            tr!(
                "組織のある版とある申請者で、`when late escalate` のステップを、版のデータが欠けているために上へ回せないとき。答える人の上の長の席が空いているときと、`escalate by <表>` の表が回す先の承認者の決め方が、データの欠け（長の空いたチーム、人のいないチーム、版にいない人かチーム）で人を見つけないときです。答える人には、承認者がいないので `when empty escalate` で回った先の人（上の長と、`then <決め方>` の決め方の人）も入ります。二度目に回すとき（`escalate, then escalate`）も見ます。上へ回せないステップは、`escalate` のあとに書いたことを、その期限で行います（`then expire` か何も無ければ期限切れ、`then stay` なら待ち続けます）。答える人が組織のいちばん上にいるときのように、組織の形で上へ回せないステップは、書いたとおりに期限で終わるので警告にせず、`ok` の下の行で申請者の数を言います。期限を過ぎたときにどうなるかは `ringi simulate` が言います。期限を過ぎる人ごとに申請者をまとめて言います。",
                "In a version of the organization, for some requester, a stage that goes up when it is late (`when late escalate`) cannot go up because the data has a gap: the seat of the head above the one who answers it is empty, or the way to choose a table of `escalate by <table>` sends it to finds nobody for a gap in the data (a team whose head is empty, a team with nobody in it, a person or a team the version does not have). The one who answers is also the one a stage with no approver went to with `when empty escalate` (a head above, or the people of the way of `then <way>`). The second time it goes up (`escalate, then escalate`) is looked at too. A stage that cannot go up does at that deadline what is written after `escalate`: with `then expire`, or nothing, it expires; with `then stay`, it waits on. A stage that cannot go up by the shape of the organization, as when the one who answers is at the top, ends at its deadline as it is written, and is not warned of: the line under `ok` says for how many requesters, and `ringi simulate` says what the stage does when it is late. The requesters are said by the one who is late."
            ),
            tr!(
                "版のデータを直してください。ほかの人へ回すなら、`when late escalate by <表>` の表で回す先を決めてください。",
                "Correct the data of the version; for it to go to someone else, say where it goes in the table of `when late escalate by <table>`."
            ),
            (W507_EN, SMALL_ORG_NO_HEAD_OFFICE_EN),
            (W507_JA, SMALL_ORG_NO_HEAD_OFFICE_JA),
            &["W502", "W402"],
        ),
        with_files(
            "E508",
            tr!("組織のある版にいないチームか人を、`submitted by` が指しています", "`submitted by` names a team or a person a version does not have"),
            tr!(
                "`ticket` の `submitted by` が、組織のある版に無いチーム（`everyone in legal`、`team legal`、`position … in legal`）か、いない人（`person \"e09\"`）を指すとき。ほかのグループに人がいても誤りにします。課がなくなって、出せるはずの人が出せなくなるのを、切り替えの前に見つけるためです。書いた場所で、版をまとめて言います。この誤りのある版では E509 を言いません。",
                "The `submitted by` of `ticket` names a team (`everyone in legal`, `team legal`, `position … in legal`) or a person (`person \"e09\"`) a version of the organization does not have. It is an error even when the other groups have people, so that a section gone in a version, and the people who should submit left unable to, are found before the switch. It is said where the team or the person is written, the versions together; in such a version E509 is not said."
            ),
            tr!("ID を直すか、その版のデータにチームか人を足してください。", "Correct the ID, or add the team (the person) to the data of the version."),
            (E508_EN, SMALL_ORG_EN),
            (E508_JA, SMALL_ORG_JA),
            &["E509", "E503"],
        ),
        with_files(
            "E509",
            tr!("組織のある版で、この申請をだれも出せません", "No one may submit the request in a version of the organization"),
            tr!(
                "組織のある版で、`submitted by` に当たる人が一人もいないとき（`except` で全員が外れるときも）。その版では、だれもこの申請を出せません。書いたチームか人が版にいないときは、E508 だけを言います。",
                "In a version of the organization, `submitted by` comes to nobody, or `except` leaves everyone out: no one may submit the request in that version. Where a team or a person it names is not in the version, E508 alone is said."
            ),
            tr!("`submitted by` を広げるか、版のデータを直してください。", "Widen `submitted by`, or correct the data of the version."),
            (E509_EN, SMALL_ORG_EN),
            (E509_JA, SMALL_ORG_JA),
            &["E508"],
        ),
        with_files(
            "W510",
            tr!("どのステップにも答える人がいない申請者がいます", "Requesters for whom no stage has anyone to answer it"),
            tr!(
                "組織のある版とある申請者で、承認者のいないステップがどれも飛ばされ（`when empty skip`、`when empty escalate, then skip`）、ほかに答える人のいるステップが無いとき。その人のチケットは、出したとたんに、だれも答えないまま承認で終わります。社長の申請の上長のステップを、上に人がいないので `then skip` で飛ばすときなどです。一つ一つのステップが組織の形で書いたとおりに飛ばされていても、申請をだれも見ないので、警告にします。申請を出せる人（`submitted by`）だけを見ます。最初の承認のステップの行で言い、飛ばされるステップの組ごとに、申請者をまとめて言います。",
                "In a version of the organization, for some requester, every stage with no approver is skipped (`when empty skip`, `when empty escalate, then skip`), and no other stage has anyone to answer it: a ticket of theirs is approved as soon as it is submitted, with nobody answering it. The stage of the manager, skipped with `then skip` for the president's request because nobody is above, is one. Each stage may be skipped as it is written, by the shape of the organization, and still nobody sees the request, so it is a warning. Only the people who may submit the request (`submitted by`) are looked at. It is said at the line of the first stage that approves, the requesters together for the stages that are skipped."
            ),
            tr!(
                "だれかが見るべきなら、`then` のあとに、その人の申請に答える承認者の決め方を書くか（`who` に `board = team board` と宣言して `when empty escalate, then board`）、版のデータを直してください。その人が出さない申請なら、`submitted by … except …` で外してください。",
                "If someone must see their requests, write after `then` the way to choose who answers them (`when empty escalate, then board`, with `board = team board` in `who`), or correct the data of the version; if they do not make the request, leave them out with `submitted by … except …`."
            ),
            (W510_EN, SMALL_ORG_EN),
            (W510_JA, SMALL_ORG_JA),
            &["W502", "E502", "E509"],
        ),
        // ── Examples and scenarios ──
        e(
            "E601",
            tr!("経路の例が成り立ちません", "An example of the route that does not hold"),
            tr!(
                "`examples` の行の答えが、経路の表の答えと違うとき。下ろした規則で rulec が確かめ、表の答えを添えます。例の行のセルに、フォームが取らない値を書いたときにも出ます。範囲の外の数や日付（計算するフィールドは、計算がとりうる値の外）、単位の刻みの倍数でない数、長さが `count` の外の文、マスタのどの行でもないキーです。経路がその列を読むかどうかにかかわらず、そのセルで言います。選べない値は E004、型の違う値は E009 です。",
                "A row of the `examples` comes to other stages than the route gives. rulec checks it in the rule the route is lowered to, and the route's answer is given. A cell of a row with a value the form does not take is this too: a number or a date outside its range (for a computed field, outside what its computation can come to), a number that is not a multiple of its unit's step, a text whose length is outside its `count`, a key no row of its master has. It is said at the cell, whether the route reads the column or not; a value that is not one to choose from is E004, and a value of another type E009."
            ),
            tr!(
                "例か表のどちらが誤りかを決めて直してください。フォームが取らない値は、取る値に直してください（例の行は、出せる申請です）。",
                "Correct whichever is wrong: the example or the table. A value the form does not take is written as one it takes: a row of the examples is a request that can be made."
            ),
            E601_EN,
            E601_JA,
            &["E306", "E004", "E009"],
        ),
        with_files(
            "E602",
            tr!("シナリオが書いたとおりに進みません", "A scenario that goes another way"),
            tr!(
                "シナリオの行のあとのステップか状態が、チケットの進み方と違うとき。実際のステップと状態と、置いてある時計を添えます。`ringi check` はシナリオをどれもチケットの機械で流し、`ringi run` は進んだ所までを一行ずつ見せます。",
                "After a row of a scenario, the stage or the state is not what the ticket comes to; the real ones are given, with the clocks that are set. `ringi check` runs every scenario on the ticket machine, and `ringi run` prints each row as far as it goes."
            ),
            tr!("シナリオか、経路とチケットの決まりのどちらが誤りかを決めて直してください。", "Correct whichever is wrong: the scenario, or the route and the ticket."),
            (E602_EN, SMALL_ORG_EN),
            (E602_JA, SMALL_ORG_JA),
            &["E603"],
        ),
        with_files(
            "E603",
            tr!("チケットが受け付けないイベントがシナリオにあります", "An event of a scenario the ticket does not take"),
            tr!(
                "シナリオの行を、チケットが受け付けないとき。機械が断るイベント（承認者でない人の `approve`、終わったあとのイベント、2,000 字を超えるコメント）、帳簿が拒否する仮押さえ、二度目の `submit`、上の行より前の時刻、イベントが取らない引数、フォームが取らない値、時計か flow の行の `by` や `comment` のセルに書いたものです。",
                "The ticket does not take a row of a scenario: an event the machine refuses (an `approve` by someone who is not an approver, anything after the ticket has ended, a comment past 2,000 characters), a hold the book refuses, a second `submit`, a time before the row above, an argument the event does not take, a value the form does not take, something in the `by` or the `comment` cell of a clock's or a flow's row."
            ),
            tr!("イベントか、それをする人か、時刻を直してください。", "Correct the event, who makes it, or its time."),
            (E603_EN, SMALL_ORG_EN),
            (E603_JA, SMALL_ORG_JA),
            &["E602"],
        ),
        with_files(
            "E604",
            tr!("時計のイベントの時刻が違います", "A timer at another time"),
            tr!(
                "シナリオの時計の行（`remind`、`late`、`resubmit late`、`pause ends`）の時刻が、時計が鳴る時刻と違うとき（期限が日の終わりなのに `late` を 23:59 に書いた）、その時計が置かれていないとき、時計が鳴る時刻かそのあとの行を、時計の行なしに書いたとき。置いてある時計と、書く行を添えます。時刻は下ろした日付が決め、シナリオは持ちません。",
                "A clock's row of a scenario (`remind`, `late`, `resubmit late`, `pause ends`) is not at the time the clock is set for (`late` at 23:59, and the deadline at the end of the day), or no such clock is set; or a row is written at the time a clock rings or after it with no row for the clock. The clocks that are set are given, and the row to write. The times are the lowered dates'; a scenario holds none of its own."
            ),
            tr!("時計の行を、添えた時刻に書いてください。", "Write the clock's row at the time given."),
            (E604_EN, SMALL_ORG_OFFICE_EN),
            (E604_JA, SMALL_ORG_OFFICE_JA),
            &["E602"],
        ),
        // ── Screens ──
        later(
            "E701",
            tr!("画面に見せていない値をボタンが送ります", "A button sends a value the screen does not show"),
            tr!(
                "`ritsu gen --target requests` が書く画面のテンプレートで、ボタンが送る値を、その画面のどの部品も見せていないとき（承認カードが種別の文言を見せ、コードを送る）。テンプレートを書く前に、どの画面にも確かめます。テンプレートは申請のファイルの宣言から作るので、これが出るのは ringi の不具合で、ファイルの書き方では出ません。",
                "A button of a screen `ritsu gen --target requests` writes sends a value no part of the screen shows (an approval card shows a category's label and sends its code). Every screen is checked for it before the templates are written. The templates are made from what the request's file declares, so it is said only of a bug in ringi, never of how a file is written."
            ),
            tr!(
                "申請のファイルは直さず、その申請と ritsu の版（`ritsu --version`）を添えて、ringi の不具合として知らせてください。",
                "Leave the request's file as it is, and report it as a bug in ringi, with the request and the version of ritsu (`ritsu --version`)."
            ),
            &["W702"],
        ),
        later(
            "W702",
            tr!("送る値と違う刻みで見せるか読む数です", "A number shown or read in other steps than it is sent in"),
            tr!(
                "`ritsu gen --target requests` が書く画面のテンプレートが、ボタンが送る数を、送る刻みと違う刻みで見せるか読むとき（円の小数を整数に丸めて見せる、0.25% 刻みの率を 0.01% 刻みで読む）。E701 と同じく、ringi の不具合でだけ出ます。",
                "A screen `ritsu gen --target requests` writes shows or reads a number a button sends in other steps than it is sent in (fractions of a yen shown as whole yen, a rate in steps of 0.25% read in steps of 0.01%). As with E701, it is said only of a bug in ringi."
            ),
            tr!(
                "申請のファイルは直さず、その申請と ritsu の版（`ritsu --version`）を添えて、ringi の不具合として知らせてください。",
                "Leave the request's file as it is, and report it as a bug in ringi, with the request and the version of ritsu (`ritsu --version`)."
            ),
            &["E701"],
        ),
        with_files(
            "E703",
            tr!("画面に出る項目に、その言語の文言がありません", "An item a screen shows has no words in a language"),
            tr!(
                "画面に出る項目に、要る言語の文言が無いとき。一つは、`labels` で宣言した言語に文言が無いとき（`labels ja` に `items.price` が無い）。項目は、`request`、フィールド（明細の列は `<明細>.<列>`）、選ぶ値（`<フィールド>.<値>`。規則の列挙から選ぶ値も）、`stage <名前>`、`who <名前>` です。もう一つは、ほかのファイルから来る項目に、ファイルの言語の題が無いとき。規則の列挙から選ぶ値で、フィールドの下に題の行が無く、規則がファイルの言語で書いた表示名も無い物（フィールドの行で言う）と、ほかの言語で書いた共有の経路のステップと決め方で、経路の `labels <ファイルの言語>` に題が無い物（`use route` の行で言う）です。`ritsu gen --target requests` が画面を作る前に言い、何も書きません。",
                "An item a screen shows has no words in a language it needs. Either a language `labels` declares has no label for it (`labels ja` without `items.price`); the items are `request`, the fields (a column of rows as `<rows>.<column>`), the values to choose from (`<field>.<value>`, a value of a rule's enum too), `stage <name>` and `who <name>`. Or an item that comes from another file has no title in the file's language: a value of a rule's enum with no line under its field that titles it and no display name the rule writes in the file's language (said at the field's line), or a stage or a way to choose an approver of a shared route written in another language, which the route's `labels <the file's language>` does not title (said at the `use route` line). `ritsu gen --target requests` says it before it makes the screens, and writes nothing."
            ),
            tr!(
                "その言語の `labels` に項目の文言を足すか、列挙の値の題をフィールドの下に書くか（`goods \"物品・サービス\"`）、共有の経路のファイルの `labels` に題を足してください。",
                "Add the item's label to that language's `labels`, title the enum's value under its field (`goods \"Goods and services\"`), or add the title to the `labels` of the shared route's file."
            ),
            (E703_EN, SMALL_ORG_EN),
            (E703_JA, SMALL_ORG_JA),
            &["E007"],
        ),
        with_files(
            "E704",
            tr!("画面が正確に持てない数の範囲です", "A number's range a screen cannot hold exactly"),
            tr!(
                "数のフィールドの範囲の端が、単位の刻みで数えて 2^53 − 1（9_007_199_254_740_991）を越えるとき。申請者が入れるフィールド、承認者のフィールド、計算するフィールド、明細の列のどれでも出ます。画面は数を JavaScript と同じに持つので、それを越える値は丸まり、別の値として確かめて送ってしまいます。`ritsu gen --target requests` が画面を作る前にフィールドの行で言い、何も書きません。",
                "An end of a number field's range is past 2^53 − 1 (9_007_199_254_740_991) counted in the steps of its unit, whether the requester gives the field, an approver does, the form computes it or it is a column of rows. A screen holds a number as JavaScript does, so a value past it would be rounded, and checked and sent as another. `ritsu gen --target requests` says it at the field's line before it makes the screens, and writes nothing."
            ),
            tr!("範囲を、刻みで数えて ±9_007_199_254_740_991 の中に狭めてください。", "Narrow the range to within ±9_007_199_254_740_991 steps."),
            (E704_EN, SMALL_ORG_EN),
            (E704_JA, SMALL_ORG_JA),
            &["E101"],
        ),
        // ── Security (ritsu's DESIGN 16) ──
        e(
            "W901",
            tr!("鍵の形の値がファイルに書いてあります", "A value shaped like a key is written in the file"),
            tr!(
                "`.ringi` のどこか（文字列でもコメントでも）に、プロバイダーが形を決めている鍵の形の値（AWS のアクセスキー ID、GitHub・Slack・Stripe・OpenAI・Anthropic・Google の鍵やトークン、Slack の Incoming Webhook の URL、PEM の秘密鍵）があるとき。ファイルが読めないときも出ます。診断には鍵の種類と、接頭辞と、長さだけを出し、鍵そのものも、その行も出しません。",
                "Somewhere in a `.ringi`, in a string or a comment, there is a value in a shape its provider fixes for a key: an AWS access key ID, a key or token of GitHub, Slack, Stripe, OpenAI, Anthropic or Google, a Slack incoming webhook URL, a PEM private key. It is said of a file that does not read too. The diagnostic gives the kind of key, its prefix and its length, and never the key nor its line."
            ),
            tr!(
                "鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。本物の鍵なら、まずプロバイダーで無効にしてください。ファイルから消しても、リポジトリの履歴には残ります。テスト用の値なら、同じ行のコメントに `ritsu: test secret` と書いてください。",
                "Keep the key where the code runs (an environment variable, the platform's connection or secret store) and read it from there. If it is real, revoke it with its provider first: taking it out of the file leaves it in the history of the repository. If it is a value for tests, write `ritsu: test secret` in a comment on the same line."
            ),
            W901_EN,
            W901_JA,
            &["W109"],
        ),
    ];
    Ledger {
        tool: "ringi",
        example_file: "example.ringi",
        fence: "ringi",
        repro_heading: tr!("再現", "Example"),
        later_text: tr!(
            "（再現はありません。これを出すテンプレートを ringi は書かず、`ritsu gen` がこれを言うのは ringi の不具合のときだけです）",
            "(no example: ringi writes no template that gives it, and `ritsu gen` says it only of a bug in ringi)"
        ),
        later_markdown: tr!(
            "再現はありません。これを出すテンプレートを ringi は書かず、`ritsu gen` がこれを言うのは ringi の不具合のときだけです。",
            "No example: ringi writes no template that gives it, and `ritsu gen` says it only of a bug in ringi."
        ),
        entries,
    }
}
