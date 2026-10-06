//! The ledger of every diagnostic code (DESIGN 10): `sekisho explain` reads it, `docs/codes.md`
//! and `docs/codes.ja.md` are its Markdown, and a test runs every entry's example and requires its
//! code to come out, so the example cannot go stale while the prose around it still reads well. An
//! example is in English names, and in Japanese names for `explain --lang ja`. The entries are
//! sekisho's; how they are written out, and how every example is run, is ritsu-base's
//! ([`ritsu_base::ledger`]).
//!
//! The codes of the checks of every combination and of the contracts (E202–E208, W201, E301–E307,
//! W301–W303, W401) are in the ledger with no example yet ([`Repro::Later`]) until those checks
//! print them.

use ritsu_base::ledger::{Entry, Ledger, Repro};
use ritsu_base::text::Text;

/// An entry with its example in English names, and in Japanese names for `explain --lang ja`.
fn e(code: &'static str, title: Text, when: Text, fix: Text, en: &'static str, ja: &'static str, related: &'static [&'static str]) -> Entry {
    let mut x = Entry::new(code, title, when, fix, Repro::File { body: en, beside: &[] }, related);
    x.repro_ja = Some(Repro::File { body: ja, beside: &[] });
    x
}

/// An entry whose example is written in a later stage.
fn later(code: &'static str, title: Text, when: Text, fix: Text, related: &'static [&'static str]) -> Entry {
    Entry::new(code, title, when, fix, Repro::Later, related)
}

/// The same entry, with its example in Japanese names.
fn with_ja(mut x: Entry, ja: Repro) -> Entry {
    x.repro_ja = Some(ja);
    x
}

/// The `.gate` files the examples of E210 and E211 read with `use gate`.
const PEOPLE: (&str, &[u8]) = ("people.gate", "gate people v1\nnamespace Shop\n\nrole clerk\n".as_bytes());
const STAFF: (&str, &[u8]) = ("staff.gate", "gate staff v1\nnamespace Shop\n\nrole clerk\n".as_bytes());
const PEOPLE_ELSEWHERE: (&str, &[u8]) = ("people.gate", "gate people v1\nnamespace People\n\nrole clerk\n".as_bytes());
const STAFF_JA: (&str, &[u8]) = ("職員.gate", "gate 職員(staff) v1\nnamespace Shop\n\nrole 係(clerk)\n".as_bytes());
const CLERKS_JA: (&str, &[u8]) = ("店員.gate", "gate 店員(clerks) v1\nnamespace Shop\n\nrole 係(clerk)\n".as_bytes());
const STAFF_ELSEWHERE: (&str, &[u8]) = ("職員.gate", "gate 職員(staff) v1\nnamespace Staff\n\nrole 係(clerk)\n".as_bytes());

/// An OpenAPI document with no operations, for an example that names one.
const SHOP: (&str, &[u8]) = ("shop.json", b"{\"openapi\": \"3.1.0\", \"info\": {\"title\": \"shop\", \"version\": \"1\"}, \"paths\": {}}\n");

pub fn ledger() -> Ledger {
    let entries = vec![
        // ── Words and lines ──
        e(
            "E001",
            tr!("読めない字句があります", "Something cannot be read as a word of the language"),
            tr!(
                "知らない文字、閉じていない文字列、文字列の外の全角の空白、形の崩れた日付、単位の表に無い単位の付いた数があるとき。Cedar の `==`、`!=`、`&&`、`||`、`!` も sekisho の字句にはありません。",
                "A character the language does not have, a string not closed, a full-width space outside a string, a date of the wrong shape, or a number with a unit the table of units does not have. Cedar's `==`, `!=`, `&&`, `||` and `!` are not words of sekisho either."
            ),
            tr!(
                "示された位置を直してください。等しいことは `is`、等しくないことは `is not` で書き、「かつ」「または」「でない」は `and`、`or`、`not` で書きます。",
                "Correct it where it points. The same is `is`, not the same is `is not`, and the others are `and`, `or` and `not`."
            ),
            "gate t v1\ndescription \"not closed\n",
            "gate t v1\ndescription \"閉じていない\n",
            &["E005"],
        ),
        e(
            "E002",
            tr!("キーワードを名前にしています", "A keyword is used as a name"),
            tr!(
                "宣言した名前か別名が、条件と計算と型を書く語（`and`、`or`、`not`、`in`、`is`、`principal`、`resource`、`workflow`、`true`、`false`、`any`、`today`、`bool`、`date`、`number`、`rate`）のどれかのとき。名前を書く位置に、文字列や数など名前でないものを書いたときも出ます。",
                "A declared name or alias is one of the words conditions, computed values and types are written with (`and`, `or`, `not`, `in`, `is`, `principal`, `resource`, `workflow`, `true`, `false`, `any`, `today`, `bool`, `date`, `number`, `rate`); or where a name goes, something that is no name is written (a string, a number)."
            ),
            tr!("別の名前にしてください。", "Choose another name."),
            "gate t v1\n\nrole is\n",
            "gate t v1\n\nrole 係(is)\n",
            &["E007"],
        ),
        e(
            "E003",
            tr!("節や行の順か数が違います", "A section or a line is out of order, repeated, or missing"),
            tr!(
                "ファイルが `gate` の行で始まらないとき、節が決まった順にないとき、一度だけの節（`description`、`namespace`、`today`）やブロックの中の一度だけの行を二度書いたとき、ブロックに要る行（action の `principal` と `resource`、ポリシーと期待の `action`、`separate` の `actions`）が無いとき、`attributes`、`input`、`context` の下に行が無いとき。",
                "The file does not start with a `gate` line; the sections are out of their order; a section written once (`description`, `namespace`, `today`) or a line written once in a block is written twice; a line a block needs is missing (an action's `principal` and `resource`, the `action` of a policy or an expectation, the `actions` of a `separate`); or `attributes`, `input` or `context` has nothing under it."
            ),
            tr!(
                "ファイルは、見出し（`gate`）、`description`、`namespace`、`use`、`today`、`enum`、`role`、`principal`、`workflow`、`resource`、`action`、`permit` と `forbid`、`expect`、`separate` の順に書いてください。ブロックの中の順は、診断の注に出ます。",
                "A file goes: the heading (`gate`), `description`, `namespace`, `use`, `today`, `enum`, `role`, `principal`, `workflow`, `resource`, `action`, `permit` and `forbid`, `expect`, `separate`; the order of a block's lines is in the diagnostic's note."
            ),
            "gate t v1\n\nresource Order\n\nrole clerk\n",
            "gate t v1\n\nresource 注文(Order)\n\nrole 係(clerk)\n",
            &["E005"],
        ),
        e(
            "E004",
            tr!("字下げがそろっていません", "The indentation does not line up"),
            tr!(
                "字下げにタブがあるとき、同じブロックの行の字下げがそろっていないとき、字下げした行を取らない行の下に字下げした行があるとき。",
                "The indentation has a tab, the lines of one block are not indented alike, or an indented line follows a line that takes none."
            ),
            tr!(
                "字下げはスペースで書き、同じブロックの行は同じ幅にしてください。`attributes`、`input`、`context` の下の行は、もう一段深く字下げします。",
                "Indent with spaces, every line of a block by the same amount; the lines under `attributes`, `input` and `context` one step further."
            ),
            "gate t v1\n\nrole clerk\n\tdescription \"Answers customers\"\n",
            "gate t v1\n\nrole 係(clerk)\n\tdescription \"お客さまに応対する\"\n",
            &["E003"],
        ),
        e(
            "E005",
            tr!("その場所に書けない行です", "A line that cannot be written there"),
            tr!(
                "行の初めの語を、そのブロックが取らないとき（`role` の中の `roles`、行の初めの `when` など）、行がその語の取る形になっていないとき（`use` のあとの知らない語、`offset` の無い `today`、ほかの action と並べた `action any`、一つだけの `actions` など）。",
                "A block does not take the line's first word (`roles` in a `role`, `when` at the left margin), or the line is not of the form its first word takes (an unknown word after `use`, a `today` without `offset`, `action any` listed with other actions, `actions` with one action)."
            ),
            tr!("注に挙がる形に直してください。", "Write it in the form the note gives."),
            "gate t v1\n\nrole clerk\n  roles manager\n",
            "gate t v1\n\nrole 係(clerk)\n  roles 責任者\n",
            &["E003", "E004"],
        ),
        // ── Names and types ──
        e(
            "E006",
            tr!("同じ名前を二度宣言しています", "A name is declared twice"),
            tr!(
                "同じ種類のもの（型と列挙、役割、列挙の値、型の属性、ワークフロー、action、action の入力と計算した値、ポリシー、期待、職務の分離、`use` の名前）のあいだで、名前か別名が重なるとき。`use gate` で読んだファイルの宣言とも比べます。規則の入力に二度値を渡したとき、`actions` に同じ action を二度並べたときも出ます。",
                "A name or an alias is declared twice among the things of one kind (the types and the enums, the roles, an enum's values, a type's attributes, the workflows, the actions, an action's inputs and computed values, the policies, the expectations, the separations, the names of the `use` lines), also against what a file read with `use gate` declares; or an input of a rule is given twice, or `actions` lists an action twice."
            ),
            tr!("どちらかの名前か別名を変えてください。", "Rename one of them, or give it another alias."),
            "gate t v1\n\nrole clerk\nrole clerk\n",
            "gate t v1\n\nrole 係(clerk)\nrole 窓口(clerk)\n",
            &["E007"],
        ),
        e(
            "E007",
            tr!("Cedar に出る名前に ASCII の別名がありません", "A name that goes to Cedar has no ASCII alias, or one of the wrong form"),
            tr!(
                "Cedar に出る名前（ファイル、列挙と値、役割、principal と resource の型、属性、ワークフロー、action、入力、計算した値、ポリシー）が別名の形の ASCII でなく、丸括弧の別名も無いとき、別名の形が違うとき、名前空間が ASCII の識別子でないとき。型の別名は `[A-Z][A-Za-z0-9]*`、ほかは `[a-z][a-z0-9_]*` です。期待と職務の分離は Cedar に出ないので、別名は要りません。",
                "A name that goes to Cedar (the file, an enum and its values, a role, a principal's or a resource's type, an attribute, a workflow, an action, an input, a computed value, a policy) is not ASCII of the form of an alias and has no alias in parentheses; or its alias is not of the form; or the namespace is not an ASCII identifier. A type's alias is `[A-Z][A-Za-z0-9]*`, any other `[a-z][a-z0-9_]*`. An expectation and a separation do not go to Cedar, and need none."
            ),
            tr!("`返金する(refund_order)` のように、丸括弧で別名を付けてください。", "Give it an alias in parentheses, like `返金する(refund_order)`."),
            "gate t v1\n\nrole Clerk\n",
            "gate t v1\n\nrole 係\n",
            &["E002", "E008"],
        ),
        e(
            "E008",
            tr!("別名が Cedar か生成するコードの予約語です", "An alias is a reserved word of Cedar or of the generated code"),
            tr!(
                "別名（別名の無い ASCII の名前も）が、Cedar の予約語（`if`、`then`、`else`、`has`、`like`）か、TypeScript・Python・Go の予約語のとき。型の名前が、Cedar の `Action` と組み込みの型、sekisho が宣言する `Role` と `Workflow` のときも出ます。",
                "An alias (or an ASCII name with none) is a reserved word of Cedar (`if`, `then`, `else`, `has`, `like`) or of TypeScript, Python or Go; or a type's name is Cedar's `Action` or one of its builtin types, or `Role` or `Workflow`, which sekisho declares."
            ),
            tr!("別の別名にしてください。", "Choose another alias."),
            "gate t v1\n\nrole has\n",
            "gate t v1\n\nrole 持つ(has)\n",
            &["E007"],
        ),
        e(
            "E101",
            tr!("知らない名前です", "A name names nothing"),
            tr!(
                "参照した役割、型、属性、列挙の値、action、入力、計算した値、`use` の名前、規則の出力と入力、日付の関数、単位が無いとき。ポリシーと期待が、ほかのファイルの action を書いたときも出ます。",
                "A role, a type, an attribute, a value of an enum, an action, an input, a computed value, the name of a `use`, a rule's output or input, a date function or a unit that is referred to is not there; or a policy or an expectation names an action of another file."
            ),
            tr!("書き違いを直すか、宣言を足してください。注に、書ける名前が並びます。", "Correct the spelling, or declare it; the note lists the names there are."),
            "gate t v1\n\nrole clerk\n\nprincipal User\n  roles clerk\n\nresource Order\n\naction view\n  principal User\n  resource Order\n\npermit clerks_view\n  principal in clerck\n  action view\n",
            "gate t v1\n\nrole 係(clerk)\n\nprincipal 職員(User)\n  roles 係\n\nresource 注文(Order)\n\naction 見る(view)\n  principal 職員\n  resource 注文\n\npermit 係は見られる(clerks_view)\n  principal in 系\n  action 見る\n",
            &["E006"],
        ),
        e(
            "E102",
            tr!("型が合いません", "A type that does not fit"),
            tr!(
                "真偽を列挙の値と比べる、列挙を数と比べる、単位の違う数を比べる、真偽でない値を条件にそのまま書く、範囲を書けない型に範囲を書く、規則の入力に型の違う値を渡すとき。",
                "True or false compared with a value of an enum, an enum with a number, numbers of different units; a value that is not true or false written alone as a condition; a range on a type that takes none; a value of another type given to a rule's input."
            ),
            tr!(
                "比べる値を、左の値の型に合わせてください。数は単位まで同じにします（`JPY` と `円` は同じ、税込と税抜は別）。",
                "Make the value fit the type of what it is compared with; a number has to be of the same unit (`JPY` and `円` are the same; with tax and without are not)."
            ),
            "gate t v1\n\nprincipal User\n  attributes\n    suspended : bool\n\nresource Order\n\naction view\n  principal User\n  resource Order\n\nforbid suspended_users\n  principal is User\n  action view\n  when principal.suspended is yes\n",
            "gate t v1\n\nprincipal 職員(User)\n  attributes\n    停止中(suspended) : bool\n\nresource 注文(Order)\n\naction 見る(view)\n  principal 職員\n  resource 注文\n\nforbid 停止中の職員(suspended_users)\n  principal is 職員\n  action 見る\n  when principal.停止中 is はい\n",
            &["E103"],
        ),
        e(
            "E103",
            tr!("範囲の誤りです", "A range that is wrong"),
            tr!(
                "数か日付の属性と入力に `range` が無いとき、端が片方しかないとき、範囲に入る値が無いとき、定数がその単位で整数にならないとき、±(2⁵³ − 1) を超えるとき、条件で比べる定数が範囲の外のとき。",
                "A number or a date (an attribute, an input) has no `range`, or one end of it only, or a range no value is in; a constant does not come to a whole number in its unit, or is past ±(2⁵³ − 1); a constant a condition compares with is outside the range."
            ),
            tr!(
                "`range >=1GBP <=10_000GBP` のように両端を書き、定数をその単位で整数になるように直してください。",
                "Write both ends, like `range >=1GBP <=10_000GBP`, and constants that come to whole numbers in the unit."
            ),
            "gate t v1\n\nprincipal User\n  attributes\n    refund_limit : money[GBP, incl_tax]\n",
            "gate t v1\n\nprincipal 職員(User)\n  attributes\n    返金できる額(refund_limit) : money[GBP, incl_tax]\n",
            &["E102"],
        ),
        e(
            "E104",
            tr!("v1 で書けない関係です", "A relation v1 does not write"),
            tr!(
                "属性を二段以上たどるとき（`resource.order.customer`）、型の違う二つの属性を同じかで比べるとき、principal の来ない型の属性を principal と比べるとき、数をほかの値と比べるとき。",
                "An attribute is followed by another (`resource.order.customer`); two attributes of different types are compared; an attribute is compared with the principal when no principal of its type comes; a number is compared with another value."
            ),
            tr!(
                "関係は一段だけにしてください（`resource.customer is principal`、`resource.tenant is principal.tenant`、`principal in resource.team`）。二段たどる答えは、それを計算する規則の真偽の出力にできます。",
                "Keep a relation to one step (`resource.customer is principal`, `resource.tenant is principal.tenant`, `principal in resource.team`); an answer that goes two steps can be a true-or-false output of a rule."
            ),
            "gate t v1\n\nprincipal User\n\nresource Order\n\naction view\n  principal User\n  resource Order\n\npermit owners_view\n  principal is User\n  action view\n  when resource.order.owner is principal\n",
            "gate t v1\n\nprincipal 職員(User)\n\nresource 注文(Order)\n\naction 見る(view)\n  principal 職員\n  resource 注文\n\npermit 持ち主は見られる(owners_view)\n  principal is 職員\n  action 見る\n  when resource.注文.持ち主 is principal\n",
            &["E102"],
        ),
        with_ja(
            Entry::new(
                "E105",
                tr!("計算した値の誤りです", "A computed value that is wrong"),
                tr!(
                    "計算した値が規則でも日付でもないもの（`use openapi` の名前など）を呼ぶとき、規則の出力が列挙か真偽でないとき、要素の並びをたどる規則を呼ぶとき、規則や日付の入力に計算した値かエンティティを渡すとき、入力に値を渡していないとき。",
                    "A computed value calls what is neither a rule nor a date (the name of a `use openapi`); a rule's output is neither an enum nor true or false; the rule walks a list of elements; a computed value or an entity is given to the input of a rule or a date; an input is given nothing."
                ),
                tr!(
                    "計算した値は、規則の列挙か真偽の出力、日付の関数との比べ方（`today <= …`）、営業日（`today is open in …`）にしてください。入力に渡せるのは、principal と resource の属性、action の入力、定数、`today` です。",
                    "A computed value is an enum or a true-or-false output of a rule, a test of today against a date (`today <= …`), or a business day (`today is open in …`); an input takes an attribute of the principal or the resource, an input of the action, a constant, or `today`."
                ),
                Repro::File {
                    body: "gate t v1\n\nuse openapi shop from \"shop.json\"\n\nprincipal User\n\nresource Order\n\naction view\n  principal User\n  resource Order\n  context\n    allowed = shop(id: 1).ok\n",
                    beside: &[SHOP],
                },
                &["E101", "E102"],
            ),
            Repro::File {
                body: "gate t v1\n\nuse openapi 店 from \"shop.json\"\n\nprincipal 職員(User)\n\nresource 注文(Order)\n\naction 見る(view)\n  principal 職員\n  resource 注文\n  context\n    許す(allowed) = 店(id: 1).ok\n",
                beside: &[SHOP],
            },
        ),
        e(
            "E106",
            tr!("principal の型が action に来ません", "A principal the actions never take"),
            tr!(
                "ポリシーか期待の `principal` の行（`is <型>`、`is workflow <名前>`、`in <役割>`）に当たる principal が、並べた action に来ないとき。`action any` では、どの action にも来ないとき。",
                "No principal a policy's or an expectation's `principal` line picks (`is <type>`, `is workflow <name>`, `in <role>`) comes to an action it lists; with `action any`, to no action at all."
            ),
            tr!(
                "`principal` の行を直すか、action の `principal` の行に型を足してください。役割で選ぶなら、その役割を持てる型の `roles` に役割を書きます。",
                "Correct the `principal` line, or add the type to the action's `principal` line; for a role, write it in the `roles` of a type that holds it."
            ),
            "gate t v1\n\nprincipal User\n\nprincipal Customer\n\nresource Order\n\naction refund\n  principal User\n  resource Order\n\npermit customers_refund\n  principal is Customer\n  action refund\n",
            "gate t v1\n\nprincipal 職員(User)\n\nprincipal 顧客(Customer)\n\nresource 注文(Order)\n\naction 返金する(refund)\n  principal 職員\n  resource 注文\n\npermit 顧客は返金できる(customers_refund)\n  principal is 顧客\n  action 返金する\n",
            &["E101"],
        ),
        e(
            "E107",
            tr!("`today` の誤りです", "Today without its line, or with a time zone's name"),
            tr!(
                "計算した値か規則の入力が `today` を使うのに `today` の行が無いとき、`offset` がタイムゾーンの名前か、`±HH:MM` の形でないとき。",
                "A computed value or a rule's input uses `today`, and there is no `today` line; or the `offset` is a time zone's name, or not `±HH:MM`."
            ),
            tr!(
                "`today range >=2026-10-01 <=2028-10-31 offset +00:00` のように、範囲と、日を変えるオフセットを数で書いてください。",
                "Write the range and the offset the day changes at, as a number: `today range >=2026-10-01 <=2028-10-31 offset +00:00`."
            ),
            "gate t v1\n\ntoday range >=2026-10-01 <=2026-12-31 offset Europe/London\n",
            "gate t v1\n\n# イングランドとウェールズの日で\ntoday range >=2026-10-01 <=2026-12-31 offset Europe/London\n",
            &["E103"],
        ),
        e(
            "E108",
            tr!("役割の `includes` が輪になっています", "The roles' `includes` go round in a circle"),
            tr!(
                "役割の `includes` をたどると、もとの役割に戻るとき（`clerk → manager → clerk`）。自分を `includes` する役割も輪です。輪一つにつき一度、輪の最初の役割の `includes` の行に、輪の役割を順に並べて出ます。",
                "Following the roles' `includes` comes back to the role it started from (`clerk → manager → clerk`); a role that includes itself is a circle too. Each circle is said once, at the `includes` of its first role, with its roles in order."
            ),
            tr!(
                "輪のどこかの `includes` を消してください。`includes` は「この役割を持つ人は、その役割も持つ」なので、輪になると、どの役割がどれを含むかが決まらず、Cedar の役割の親子も輪になります。",
                "Remove an `includes` of the circle. `includes` says whoever holds this role holds that one too: in a circle, which role includes which is not decided, and the parents of Cedar's roles would go round too."
            ),
            "gate t v1\n\nrole clerk\n  includes manager\n\nrole manager\n  includes clerk\n",
            "gate t v1\n\nrole 係(clerk)\n  includes 責任者\n\nrole 責任者(manager)\n  includes 係\n",
            &["E101"],
        ),
        // ── Other languages and contracts ──
        e(
            "E201",
            tr!("`use` のファイルが検査を通らないか、読めないか、ルートの外にあります", "A file a `use` reads does not pass its language's check, cannot be read, or is outside the root"),
            tr!(
                "`use rule` の規則が rulec の、`use dates` の日付のファイルが koyomi の検査を通らないか、読めないとき（その言語の言うことが注に出ます）。`use gate` のファイルが sekisho の検査を通らないとき、読めないとき、`use gate` が輪になっているときも出ます。`use openapi`・`use proto`・`use asyncapi`・`use book` のファイルがルートの外にあるときも出ます（action が守る操作は、ルートからのパスで参照するため）。",
                "A rule of a `use rule` does not pass rulec's check, or a dates file of a `use dates` koyomi's, or it cannot be read (the note says what the language says); or a file of a `use gate` does not pass sekisho's check, cannot be read, or the `use gate` lines go round in a circle; or the file of a `use openapi`, `use proto`, `use asyncapi` or `use book` is outside the root (an operation an action guards is named by a reference whose path is from the root)."
            ),
            tr!(
                "そのファイルを、その言語の検査が通るように直してください。ルートの外にあるファイルなら、そのファイルを含むディレクトリを `--root` でルートにしてください。",
                "Correct that file until its language's check passes it. For a file outside the root, give `--root` a directory that holds it."
            ),
            "gate t v1\n\nuse gate \"people.gate\"\n",
            "gate t v1\n\nuse gate \"職員.gate\"\n",
            &["E209"],
        ),
        later(
            "E202",
            tr!("守る操作が契約にありません", "The operation an action guards is not in the contract"),
            tr!("`guards` の操作が、契約の文書（OpenAPI、`.proto`、AsyncAPI）か帳簿に無いとき。", "The operation a `guards` names is not in the contract (OpenAPI, `.proto`, AsyncAPI) or the book."),
            tr!(
                "操作の `operationId`、`\"POST /orders/{{orderId}}/refunds\"`、`\"Service/Method\"` を、契約にあるとおりに書いてください。",
                "Write the operation as the contract has it: its `operationId`, `\"POST /orders/{{orderId}}/refunds\"`, `\"Service/Method\"`."
            ),
            &["E203", "E205"],
        ),
        later(
            "E203",
            tr!("入力が操作の受け取るものと合いません", "An input does not fit what the operation takes"),
            tr!(
                "`input` が操作の受け取るもの（パスとクエリの引数、本文のフィールド、リクエストのメッセージのフィールド）に無いか、型か範囲が操作と合わないとき。",
                "An `input` is not among what the operation takes (its path and query parameters, the fields of its body or of its request message), or its type or range differs from the operation's."
            ),
            tr!("入力の名前、型、範囲を、操作の受け取るものに合わせてください。", "Make the input's name, type and range those of the operation."),
            &["E202"],
        ),
        later(
            "E204",
            tr!("resource の ID を取る引数が操作にありません", "The parameter the resource's id comes from is not the operation's"),
            tr!("`resource … from <引数>` の引数が、操作のパスかクエリの引数に無いとき。", "The parameter of `resource … from <parameter>` is not a path or query parameter of the operation."),
            tr!("操作のパスかクエリの引数の名前を書いてください。", "Name a path or query parameter of the operation."),
            &["E202"],
        ),
        later(
            "E205",
            tr!("二つの action が同じ操作を守っています", "Two actions guard one operation"),
            tr!("一つの操作を二つ以上の action が守るとき。どちらの判断で守るかが決まりません。", "Two actions or more guard one operation, which leaves whose answer guards it undecided."),
            tr!("操作を守る action を一つにしてください。", "Keep one action to guard the operation."),
            &["E202"],
        ),
        later(
            "E206",
            tr!("規則か日付の入力に渡す値が、その範囲か前提から外れることがあります", "A value given to a rule or a date can fall outside its range or break its precondition"),
            tr!(
                "規則か日付の入力に渡す値の範囲が、その入力の範囲を外れうるとき、または規則の前提を破りうるとき（例つき）。",
                "The range of a value given to a rule's or a date's input can fall outside that input's range, or break a precondition of the rule (with an example)."
            ),
            tr!("渡す値の範囲を、入力の範囲に収めてください。", "Keep the range of the value within the input's."),
            &["W201"],
        ),
        later(
            "W201",
            tr!("規則の前提を守るかを決められません", "Whether a rule's precondition holds cannot be decided"),
            tr!(
                "渡す値の範囲の上で、規則の前提を守るかを rulec が決められないとき。生成したコードが、走らせたときに確かめます。",
                "rulec cannot decide whether a precondition of the rule holds over the range of the values given; the generated code checks it when it runs."
            ),
            tr!("渡す値の範囲を狭めると、決められることがあります。", "A narrower range of the values given may let it be decided."),
            &["E206"],
        ),
        later(
            "E207",
            tr!("カレンダーのデータが日を覆いません", "The calendar's data does not cover the days"),
            tr!("カレンダーのデータの範囲が、`today` の範囲か、日付の関数が返しうる日を覆わないとき。", "The days a calendar's data covers do not cover the range of `today`, or the days a date function can come to."),
            tr!("カレンダーのデータを足すか、`today` の範囲を狭めてください。", "Add data to the calendar, or narrow the range of `today`."),
            &["E107"],
        ),
        later(
            "E208",
            tr!("ワークフローのフローが dandori の検査を通りません", "The flow of a workflow does not pass dandori's check"),
            tr!("`workflow … from` の `.flow` が dandori の検査を通らないか、読めないとき。", "The `.flow` of a `workflow … from` does not pass dandori's check, or cannot be read."),
            tr!("フローを、dandori の検査が通るように直してください。", "Correct the flow until dandori's check passes it."),
            &["E201"],
        ),
        e(
            "E209",
            tr!("ほかの言語を読めない sekisho で、それを読むファイルを確かめています", "A file that reads another language, checked with a sekisho that reads none"),
            tr!(
                "sekisho 単独のバイナリ（`sekisho`）で、`use rule`、`use dates`、`use calendar`、`use book`、`workflow … from` のあるファイルを確かめるとき。このバイナリには rulec、koyomi、chobo、dandori が入っていません。最初の一行で一度だけ出し、ほかの診断は出しません。exit code は 2 です（ファイルの誤りではなく、走らせ方の問題なので）。",
                "A file with `use rule`, `use dates`, `use calendar`, `use book` or `workflow … from`, checked with the binary of sekisho's own crate (`sekisho`), which holds none of rulec, koyomi, chobo and dandori. It is said once, at the first such line, and nothing else is said. The exit code is 2: it is how the command is run, not what the file says."
            ),
            tr!(
                "同じコマンドを `ritsu sekisho` で走らせてください（`ritsu sekisho check refunds.gate`）。`ritsu sekisho` は、規則、日付のファイル、カレンダー、帳簿、フローを同じプロセスの中で読みます。",
                "Run the same command as `ritsu sekisho` (`ritsu sekisho check refunds.gate`), which reads the rules, the dates files, the calendars, the books and the flows in the same process."
            ),
            "gate t v1\n\nuse rule limit from \"limit.rule\"\n",
            "gate t v1\n\nuse rule 上限 from \"上限.rule\"\n",
            &["E201"],
        ),
        with_ja(
            Entry::new(
                "E210",
                tr!("`use gate` で読むファイルの名前空間が違います", "A file read with `use gate` is in another namespace"),
                tr!(
                    "`use gate` で読んだファイルの Cedar の名前空間が、読む側のファイルの名前空間と違うとき。`namespace` を書かないファイルの名前空間は、ファイルの別名を Pascal case にしたもの（`refunds` なら `Refunds`）なので、`use gate` で読み合うファイルには、同じ `namespace` の行が要ります。",
                    "The Cedar namespace of a file read with `use gate` is not the namespace of the file that reads it. A file with no `namespace` line is in its alias in Pascal case (`Refunds` for `refunds`), so files that read one another with `use gate` take the same `namespace` line."
                ),
                tr!("二つのファイルに、同じ `namespace` の行を書いてください（`namespace Shop`）。", "Write the same `namespace` line in both files (`namespace Shop`)."),
                Repro::File { body: "gate refunds v1\nnamespace Shop\n\nuse gate \"people.gate\"\n", beside: &[PEOPLE_ELSEWHERE] },
                &["E211", "E201"],
            ),
            Repro::File { body: "gate 返金(refunds) v1\nnamespace Shop\n\nuse gate \"職員.gate\"\n", beside: &[STAFF_ELSEWHERE] },
        ),
        with_ja(
            Entry::new(
                "E211",
                tr!("`use gate` で読んだ二つのファイルが、同じものを宣言しています", "Two files read with `use gate` declare one thing"),
                tr!(
                    "`use gate` で読んだ二つのファイル（読んだファイルがさらに読むものも含む）が、同じ名前か別名の型・列挙・役割・ワークフローを宣言しているとき。読んだものは一つの Cedar の名前空間に並ぶので、同じ名前のものが二つになります。読んだ二つのファイルが同じファイルを読むときは、そのファイルを一つと数えます。あとから読んだファイルの `use gate` の行に出ます。",
                    "Two files read with `use gate` (with what they read in turn) declare a type, an enum, a role or a workflow of the same name or alias: what is read stands in one Cedar namespace, where it would be two things of one name. A file that two of them both read counts once. It is said at the `use gate` line of the file read later."
                ),
                tr!(
                    "同じ名前のものは一つのファイルにだけ宣言し、ほかのファイルはそのファイルを `use gate` で読んでください。",
                    "Declare a thing of one name in one file, and have the other files read that one with `use gate`."
                ),
                Repro::File { body: "gate refunds v1\nnamespace Shop\n\nuse gate \"people.gate\"\nuse gate \"staff.gate\"\n", beside: &[PEOPLE, STAFF] },
                &["E210", "E006"],
            ),
            Repro::File { body: "gate 返金(refunds) v1\nnamespace Shop\n\nuse gate \"職員.gate\"\nuse gate \"店員.gate\"\n", beside: &[STAFF_JA, CLERKS_JA] },
        ),
        // ── Every combination ──
        later(
            "E301",
            tr!("どの permit も action を許しません", "No permit allows the action"),
            tr!(
                "どの組み合わせでも、どの permit も action を許さないとき。だれにも許さないつもりなら、action に `nobody \"<理由>\"` を書けば出ません。",
                "No permit allows the action in any combination; `nobody \"<reason>\"` on the action says no one is meant to be allowed it."
            ),
            tr!("action を許す permit を書くか、`nobody \"<理由>\"` を書いてください。", "Write a permit that allows it, or `nobody \"<reason>\"`."),
            &["E302"],
        ),
        later(
            "E302",
            tr!("permit が何も許しません", "A permit allows nothing"),
            tr!("permit が許そうとする組み合わせが、どれも forbid に拒まれるとき（覆う forbid と、例つき）。", "Every combination a permit would allow is denied by a forbid (with the forbids and an example)."),
            tr!("forbid を狭めるか、だれにも許さないつもりなら permit を消してください。", "Narrow the forbid, or remove the permit if no one is meant to be allowed this."),
            &["E301", "E303"],
        ),
        later(
            "E303",
            tr!("条件がどの組み合わせでも成り立ちません", "A condition holds in no combination"),
            tr!("permit か forbid の条件が、どの組み合わせでも成り立たないとき。", "The conditions of a permit or a forbid hold in no combination."),
            tr!("条件を直すか、ポリシーを消してください。", "Correct the conditions, or remove the policy."),
            &["E302"],
        ),
        later(
            "E304",
            tr!("期待が成り立ちません", "An expectation does not hold"),
            tr!("期待が選ぶ組み合わせのどれかで、答えが期待と違うとき（成り立たない数と、一つの例）。", "In some combination the expectation picks, the answer is not the one expected (how many, and an example)."),
            tr!("ポリシーか期待を直してください。", "Correct the policies, or the expectation."),
            &["E305"],
        ),
        later(
            "E305",
            tr!("職務の分離が成り立ちません", "A separation does not hold"),
            tr!(
                "同じ principal が、`separate` に並べた action の二つ以上を許されるとき（principal と、それぞれの action を許す例）。",
                "One principal is allowed two of the actions a `separate` lists (the principal, and an example for each action)."
            ),
            tr!("どちらかの action を、その principal に許さないようにしてください。", "Keep one of the actions from that principal."),
            &["E304"],
        ),
        later(
            "E306",
            tr!("役割が `can` に無い action を許されます", "A role is allowed an action its `can` does not list"),
            tr!(
                "その役割だけを持つ principal が、`can` に無い action を許される組み合わせがあるとき（例つき）。",
                "A principal that holds only the role is allowed, in some combination, an action its `can` does not list (with an example)."
            ),
            tr!("`can` に action を足すか、ポリシーを狭めてください。", "Add the action to `can`, or narrow the policies."),
            &["W302"],
        ),
        later(
            "E307",
            tr!("組み合わせの数が上限を超えます", "The combinations are more than the budget"),
            tr!(
                "一つの action の組み合わせの数が上限（既定 10⁸、`--budget`）を超えるとき。何も確かめず、何も生成しません。",
                "The combinations of one action are more than the budget (10⁸ by default, `--budget`); nothing is checked, and nothing generated."
            ),
            tr!("`roles` を型ごとに分けるか、条件の読む値を減らしてください。", "Split the `roles` by type, or have the conditions read fewer values."),
            &[],
        ),
        later(
            "W301",
            tr!("ほかの一つの permit で足りている permit です", "A permit another one covers"),
            tr!("permit が許す組み合わせを、ほかの一つの permit が全部許すとき。消しても答えが変わりません。", "Every combination a permit allows, one other permit allows too; removing it changes no answer."),
            tr!("要らなければ permit を消してください。", "Remove the permit if it is not needed."),
            &["E302"],
        ),
        later(
            "W302",
            tr!("`can` の action を、その役割だけでは一度も許されません", "An action of `can` the role alone is never allowed"),
            tr!("`can` に書いた action を、その役割だけを持つ principal が、どの組み合わせでも許されないとき。", "A principal that holds only the role is allowed an action of its `can` in no combination."),
            tr!("ポリシーを足すか、`can` から action を消してください。", "Add a policy, or remove the action from `can`."),
            &["E306"],
        ),
        later(
            "W303",
            tr!("決められません", "Cannot be decided"),
            tr!(
                "多めに数えた組み合わせから出た例を、具体的な入力で起こせなかったとき、読んだ Cedar に有限でない式があるとき。",
                "An example from combinations counted too generously could not be made with concrete inputs, or a Cedar policy read has an expression that is not finite."
            ),
            tr!("理由が注に出ます。", "The note says why."),
            &[],
        ),
        // ── Generation ──
        later(
            "W401",
            tr!("Verified Permissions の上限を超えます", "Over a quota of Verified Permissions"),
            tr!(
                "生成したポリシーかスキーマが、Verified Permissions の上限（ポリシー 10,000 バイト、スキーマ 100,000 バイト、親の深さ 100 など）を超えるとき（`--authorizer avp`）。",
                "A generated policy or schema is over a quota of Verified Permissions (10,000 bytes a policy, 100,000 bytes a schema, 100 ancestors, …) (`--authorizer avp`)."
            ),
            tr!("ポリシーを分けるか、役割の入れ子を浅くしてください。", "Split the policy, or make the roles nest less deep."),
            &[],
        ),
    ];
    // the codes of the borders, the contracts and the checks of every combination, with their
    // examples, in place of the entries written for later; a code not written here comes after the
    // last code of its band (W304 after W303)
    let theirs = crate::repros::entries();
    let mut entries: Vec<Entry> = entries.into_iter().map(|e| if matches!(e.repro, Repro::Later) { theirs.iter().find(|x| x.code == e.code).cloned().unwrap_or(e) } else { e }).collect();
    for x in theirs {
        if entries.iter().any(|e| e.code == x.code) {
            continue;
        }
        let band = &x.code[1..2];
        let at = entries.iter().rposition(|e| &e.code[1..2] == band && e.code < x.code).map(|i| i + 1).unwrap_or(entries.len());
        entries.insert(at, x);
    }
    Ledger {
        tool: "sekisho",
        example_file: "example.gate",
        fence: "gate",
        repro_heading: tr!("再現", "Example"),
        later_text: tr!("（まだ再現がありません。これを出す検査は、この先の段階で入ります）", "(no example yet: the check that prints it comes in a later stage)"),
        later_markdown: tr!("まだ再現がありません。これを出す検査は、この先の段階で入ります。", "No example yet: the check that prints it comes in a later stage."),
        entries,
    }
}
