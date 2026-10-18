//! The ledger of every diagnostic code (DESIGN 12): `kaname explain` reads it, `docs/codes.md` and
//! `docs/codes.ja.md` are its Markdown, and a test runs every entry's example and requires its code
//! to come out, so the example cannot go stale while the prose around it still reads well. An
//! example is in English names, and in Japanese names for `explain --lang ja`. The entries are
//! kaname's; how they are written out, and how every example is run, is ritsu-base's
//! ([`ritsu_base::ledger`]).
//!

use ritsu_base::ledger::{Entry, Ledger, Repro};
use ritsu_base::text::Text;

/// An entry with its example in English names, and in Japanese names for `explain --lang ja`.
fn e(code: &'static str, title: Text, when: Text, fix: Text, en: &'static str, ja: &'static str, related: &'static [&'static str]) -> Entry {
    let mut x = Entry::new(code, title, when, fix, Repro::File { body: en, beside: &[] }, related);
    x.repro_ja = Some(Repro::File { body: ja, beside: &[] });
    x
}

type Beside = &'static [(&'static str, &'static [u8])];

/// The same, with files beside the example (a book, another module).
fn with_files(code: &'static str, title: Text, when: Text, fix: Text, en: (&'static str, Beside), ja: (&'static str, Beside), related: &'static [&'static str]) -> Entry {
    let mut x = Entry::new(code, title, when, fix, Repro::File { body: en.0, beside: en.1 }, related);
    x.repro_ja = Some(Repro::File { body: ja.0, beside: ja.1 });
    x
}

const LEDGER_EN: Beside = &[(
    "ledger.book",
    b"book ledger v1\ndescription \"A marketplace's money, in cents\"\n\nunit USD scale 2\n\naccount seller_balance(seller: string) : USD\n  at least 0 refused as insufficient_balance\naccount cards : USD outside\n\ntransfer capture(payment: string, seller: string, amount: USD)\n  key payment\n  move amount from cards to seller_balance(seller)\n",
)];
const LEDGER_JA: Beside = &[(
    "台帳.book",
    "book 台帳 v1\ndescription \"マーケットプレイスのお金を、セントで数える\"\n\nunit USD scale 2\n\naccount 売り手の残高(売り手: string) : USD\n  at least 0 refused as 残高不足\naccount カード会社 : USD outside\n\ntransfer 入金(支払い: string, 売り手: string, 金額: USD)\n  key 支払い\n  move 金額 from カード会社 to 売り手の残高(売り手)\n".as_bytes(),
)];
const RING_EN: Beside = &[("other.kn", b"module other v1\n\nuse module example from \"example.kn\"\n")];
const RING_JA: Beside = &[("ほか.kn", "module ほか(other) v1\n\nuse module 例 from \"example.kn\"\n".as_bytes())];

pub fn ledger() -> Ledger {
    let entries = vec![
        // ── Words and lines ──
        e(
            "E001",
            tr!("kaname として読めません", "This cannot be read as kaname"),
            tr!(
                "知らない文字、閉じていない文字列や括弧、タブ、全角の空白、存在しない日付（`2026-02-30`）、単位の表に無い単位、桁区切りのカンマ（`1,000USD`）があるとき。ほかの言語の `==`、`&&`、`||`、`!` と、続けた比べ（`a < b < c`）も kaname の字句にありません。式や型を書くところに、そこで読めない語や記号があるときも出ます。",
                "A character the language does not have, a string or a bracket not closed, a tab, a full-width space, a date that does not exist, a unit the table of units does not have, or a thousands separator (`1,000USD`). The `==`, `&&`, `||` and `!` of other languages, and comparisons chained (`a < b < c`), are not kaname either; nor is a word or a sign where an expression or a type cannot take it."
            ),
            tr!(
                "示された位置を直してください。等しいかは `=`、等しくないかは `!=` で、「かつ」「または」「でない」は `and`、`or`、`not` で書きます。比べを二つ並べるときは `a < b and b < c` と書き、桁は `_` で区切れます（`1_000USD`）。",
                "Correct it where it points. Equal is `=`, not equal `!=`, and the others are `and`, `or` and `not`; two comparisons are joined, as in `a < b and b < c`, and `_` groups digits (`1_000USD`)."
            ),
            "module example v1\n\nfn same(a: number range >=0 <=9, b: number range >=0 <=9) -> bool\n  a == b\n",
            "module 例(example) v1\n\nfn 同じか(same)(左(a): number range >=0 <=9, 右(b): number range >=0 <=9) -> bool\n  左 == 右\n",
            &["E002"],
        ),
        e(
            "E002",
            tr!("予約語は名前にできません", "A keyword cannot be a name"),
            tr!(
                "名前を書くところに、式と文の中で別の意味に読める 13 語（`let` `if` `then` `else` `match` `some` `none` `true` `false` `and` `or` `not` `in`）のどれかを書いたとき。ほかのキーワードは、その位置でしか読まないので名前にできます（`total`、`count`、`description` など）。",
                "One of the thirteen words an expression or a line reads as something else (`let` `if` `then` `else` `match` `some` `none` `true` `false` `and` `or` `not` `in`) is written where a name goes. The other keywords are read only where they stand, and may be names (`total`, `count`, `description`)."
            ),
            tr!("別の名前にしてください。", "Choose another name."),
            "module example v1\n\nfn f() -> number\n  let then = 3\n  3\n",
            "module 例(example) v1\n\nfn 三(three)() -> number\n  let then = 3\n  3\n",
            &["E001", "E006"],
        ),
        e(
            "E003",
            tr!("モジュールの組み立てが違います", "The module is not put together as kaname reads it"),
            tr!(
                "一行目が `module <名前> v1` でないとき、左端の行が決まった順（`module`、`description`、`use`、宣言（`enum`、`record`、`fn`）、`examples`）にないとき、一度だけの行（`module`、`description`、`examples`）を二度書いたとき、左端の行が知らない語で始まるとき、列挙に `default` や `open` を書いたとき、`use` の行の形が違うとき。",
                "The first line is not `module <name> v1`; the lines at the left margin are out of their order (`module`, `description`, `use`, the declarations (`enum`, `record`, `fn`), `examples`); a line written once (`module`, `description`, `examples`) is written twice; a line at the left margin starts with a word kaname does not know; an enum has `default` or `open`; or a `use` line is not of its form."
            ),
            tr!(
                "モジュールは、`module`、`description`、`use` の行、宣言、`examples` の順に書いてください。`enum`、`record`、`fn` は互いに好きな順に並べられます。",
                "Write the module in its order: `module`, `description`, the `use` lines, the declarations, `examples`. The `enum`, `record` and `fn` declarations may come in any order among themselves."
            ),
            "module example v1\n\nfn one() -> number\n  1\n\nuse module pricing from \"pricing.kn\"\n",
            "module 例(example) v1\n\nfn 一(one)() -> number\n  1\n\nuse module 価格 from \"価格.kn\"\n",
            &["E004"],
        ),
        e(
            "E004",
            tr!("字下げがそろっていません", "The indentation does not line up"),
            tr!(
                "同じブロックの行の字下げがそろっていないとき、字下げした行を取らない行の下に字下げした行があるとき。",
                "The lines of one block are not indented alike, or an indented line follows a line that takes none."
            ),
            tr!("字下げは半角のスペースで書き、同じブロックの行は同じ幅にしてください。", "Indent with spaces, every line of a block by the same amount."),
            "module example v1\n\nrecord Line\n  sku : string\n   qty : number  range >=1 <=20\n",
            "module 例(example) v1\n\nrecord 明細(Line)\n  品番(sku) : string\n   数量(qty) : number  range >=1 <=20\n",
            &["E003"],
        ),
        e(
            "E005",
            tr!("生成するコードに出る名前には、ASCII の別名が要ります", "A name the generated code takes needs an ASCII alias"),
            tr!(
                "モジュール、関数、引数、レコード、フィールド、列挙、列挙の値の名前が ASCII でなく、別名が無いとき。別名が ASCII の名前（英字か `_` で始まり、英字・数字・`_` だけ）でないときも出ます。`let` の名前と、組み込みの関数に渡す関数の引数には要りません。",
                "The name of a module, a function, a parameter, a record, a field, an enum or a value of one is not ASCII and has no alias; or an alias is no ASCII name (a letter or `_`, then letters, digits and `_`). A `let` and a parameter of a function given to a builtin need none."
            ),
            tr!("名前のすぐあとに、`請求日(invoiced)` のように丸括弧で別名を付けてください。", "Write an alias in parentheses right after the name: `請求日(invoiced)`."),
            "module example v1\n\nfn café_price() -> money[USDc]\n  350USDc\n",
            "module 例(example) v1\n\nfn 喫茶の値段() -> money[USDc]\n  350USDc\n",
            &["E007"],
        ),
        e(
            "E006",
            tr!("同じ名前が二度あります", "The same name is given twice"),
            tr!(
                "モジュールの中で、`use` の名前と宣言（`enum`、`record`、`fn`）の名前か別名が重なるとき。一つのレコードのフィールド、一つの関数の引数、一つの列挙の値が重なるときも出ます。関数か `use` に組み込みの関数の名前（`sum`、`map` など）を付けたとき、`let` や `some` の名前がその場でもう名前になっているときも出ます。",
                "In a module, two of the `use` names and the declarations (`enum`, `record`, `fn`) share a name or an alias; or two fields of a record, two parameters of a function or two values of an enum do. So does a function or a `use` given a builtin's name (`sum`, `map`), and a `let` or a `some` whose name is taken where it stands."
            ),
            tr!("どちらかの名前を替えてください。", "Rename one of them."),
            "module example v1\n\nfn quote() -> number\n  1\n\nfn quote() -> number\n  2\n",
            "module 例(example) v1\n\nfn 見積もる(quote)() -> number\n  1\n\nfn 見積もる(quote_again)() -> number\n  2\n",
            &["E005", "E110"],
        ),
        e(
            "E007",
            tr!("生成するコードが使えない名前です", "A name the generated code cannot take"),
            tr!(
                "生成するコードに出る名前（別名、別名が無ければ名前）が、TypeScript、Python、Go のどれかの予約語（`class`、`def`、`func` など）か、kaname の補助のモジュールの名前（`kaname`、`KanameError` など）のとき。",
                "A name the generated code takes (the alias, or the name with none) is a reserved word of TypeScript, Python or Go (`class`, `def`, `func`), or a name of kaname's helper module (`kaname`, `KanameError`)."
            ),
            tr!("別の名前か別名にしてください。", "Choose another name or alias."),
            "module example v1\n\nrecord Seat\n  class : string\n",
            "module 例(example) v1\n\nrecord 座席(Seat)\n  等級(class) : string\n",
            &["E005"],
        ),
        // ── Names and types ──
        e(
            "E101",
            tr!("この名前のものはありません", "Nothing has this name"),
            tr!(
                "式の名前、型の名前、関数、規則の出力、レコードのフィールド、列挙の値、日付のファイルの日付が、どれも指すものを持たないとき。列挙の値は、型の決まらないところでは `<列挙>.<値>` と書きます。",
                "A name in an expression, a type, a function, an output of a rule, a field of a record, a value of an enum or a date of a dates file names nothing. A value of an enum is written `<enum>.<value>` where no type says which enum."
            ),
            tr!("綴りを確かめてください。`use` で読んだものは `<use の名前>.<名前>` で書きます。", "Check the spelling. What a `use` reads is written `<use name>.<name>`."),
            "module example v1\n\nfn total(price: money[USDc] range >=0USDc <=10_000USDc) -> money[USDc]\n  price + shipping\n",
            "module 例(example) v1\n\nfn 合計(total)(値段(price): money[USDc] range >=0USDc <=10_000USDc) -> money[USDc]\n  値段 + 送料\n",
            &["E110"],
        ),
        e(
            "E102",
            tr!("型が合いません", "The types do not agree"),
            tr!(
                "値の型が、渡す先（関数の引数と答え、レコードのフィールド、規則と日付の入力、振替の引数）の型と違うとき。単位と税区分も型で、同じ次元の違う単位（`money[USD]` と `money[USDc]`）や、税込と税抜は合いません。足し引きと比べの両辺、`if` と `match` の答えどうし、リストの要素どうしが合わないときも出ます。",
                "A value is not of the type of where it goes (a function's parameter or answer, a field of a record, an input of a rule or a date, an argument of a transfer). A unit and a tax are part of the type: two units of one dimension (`money[USD]` and `money[USDc]`) do not agree, nor do with tax and without. So do the two sides of a sum or a comparison, the answers of an `if` or a `match`, and the elements of a list, that are of two types."
            ),
            tr!(
                "同じ型にしてください。単位の換算は `fee in USDc` のように書きます（kaname は黙って換算しません）。",
                "Give a value of the type; a conversion is written, as in `fee in USDc` (kaname converts nothing unasked)."
            ),
            "module example v1\n\nfn fee(amount: money[USD] range >=0USD <=1000USD) -> money[USDc]\n  amount\n",
            "module 例(example) v1\n\nfn 手数料(fee)(金額(amount): money[USD] range >=0USD <=1000USD) -> money[USDc]\n  金額\n",
            &["E103", "E104"],
        ),
        e(
            "E103",
            tr!("丸めた値を受け取るところに、丸めていない値を渡しています", "An unrounded value is given where a rounded one goes"),
            tr!(
                "金額と率の積、量を定数で割った値、値で割った値のような丸めていない値を、関数の引数と答え、レコードのフィールド、規則と日付の入力、振替の引数、`running_totals` の重み（重みは、答えのレコードのフィールドに入ります）に渡したとき。そこが受け取るのは、単位の整数か、率の刻みの上の値です。重みが率なら、その型の刻みのまま受け取るので、丸めていない値になるのは、値で割って刻みが分からなくなったときだけです。`let` には丸めていない値を入れられます。",
                "An unrounded value — an amount times a rate, a quantity divided by a constant, anything divided by a value — is given to a function's parameter or answer, a field of a record, an input of a rule or a date, an argument of a transfer, or a weight of `running_totals` (the weights go into the fields of the records it answers). These take a whole number of their unit, or a rate on its step; a weight that is a rate is taken on the step its type says, and is unrounded only once a division by a value has left that step unknown. A `let` takes an unrounded value."
            ),
            tr!(
                "丸め方と刻みを書いてください（`half_up(amount * 3%, 1USDc)`）。丸め方は `up`、`down`、`half_up`、`half_down`、`half_even` の五つです。",
                "Write the way of rounding and the step (`half_up(amount * 3%, 1USDc)`); the five ways are `up`, `down`, `half_up`, `half_down` and `half_even`."
            ),
            "module example v1\n\nfn fee(amount: money[USDc] range >=1USDc <=100_000USDc) -> money[USDc]\n  amount * 3%\n",
            "module 例(example) v1\n\nfn 手数料(fee)(金額(amount): money[USDc] range >=1USDc <=100_000USDc) -> money[USDc]\n  金額 * 3%\n",
            &["E102", "E104"],
        ),
        e(
            "E104",
            tr!("丸めの刻み、率の刻み、単位の換算が合いません", "A grid, a step or a conversion does not fit"),
            tr!(
                "リテラルが書いた単位の整数でないとき（`0.5USD`、単位の無い `0.4`）、リテラルを読む先の単位で整数にならないとき（`mass[g]` と比べる `1lb`）、`in` で違う通貨や次元に換算しようとしたとき、率の刻みや丸めの刻みが正の値でないとき、範囲の端や値が率の刻みの上にないとき。",
                "A literal is no whole number of the unit it is written in (`0.5USD`, a bare `0.4`), or of the unit it is read in (`1lb` compared with a `mass[g]`); `in` is asked to convert across currencies or dimensions; a step of a rate or of a rounding is not positive; or an end of a range or a value is not on a rate's step."
            ),
            tr!(
                "小さな単位で書くか（`50USDc`）、割合なら率で書いてください（`40%`）。違う通貨のあいだに為替の率は無いので、換算は表に書いてください。",
                "Write it in a smaller unit (`50USDc`), or a fraction as the rate it is (`40%`); two currencies have no exchange rate here, so a conversion between them belongs in a table."
            ),
            "module example v1\n\nfn fee(amount: money[USD] range >=0USD <=1000USD) -> money[GBP]\n  amount in GBP\n",
            "module 例(example) v1\n\nfn 手数料(fee)(金額(amount): money[USD] range >=0USD <=1000USD) -> money[GBP]\n  金額 in GBP\n",
            &["E102", "E103"],
        ),
        e(
            "E105",
            tr!("その型にない演算です", "The type has no such operation"),
            tr!(
                "型が持たない演算を書いたとき。日付の足し算（`paid_on + 30`。kaname が日付でできるのは二つの日付の差だけ）、金額どうしや重さと長さの掛け算、文字列の大小の比べや `+`、順序の無い列挙の大小、値が無いことがある値の算術と比べ、値を呼ぶこと、`match` で数を分けることなど。",
                "An operation the type does not have: adding to a date (`paid_on + 30`: the one computation kaname does with dates is the difference of two), multiplying two amounts or a weight by a length, ordering strings or adding them, ordering the values of an enum with no order, arithmetic on or comparing a value that may be none, calling a value, splitting numbers with `match`."
            ),
            tr!(
                "日付の足し算は koyomi に書いて、その日付の関数を呼んでください。値が無いことがある値は `match` で分け、文字列は `concat` でつなぎます。",
                "Write the addition to a date in koyomi and call its date; split a value that may be none with `match`; join strings with `concat`."
            ),
            "module example v1\n\nfn due(invoiced: date range >=2026-01-01 <=2026-12-31) -> date\n  invoiced + 30\n",
            "module 例(example) v1\n\nfn 期日(due)(請求日(invoiced): date range >=2026-01-01 <=2026-12-31) -> date\n  請求日 + 30\n",
            &["E102"],
        ),
        e(
            "E106",
            tr!("`match` が取りこぼす値があります", "The match leaves values out"),
            tr!(
                "`match` の分岐が、列挙の値、`true` と `false`、`none` と `some x` のどれかを受けないとき。上の分岐がもう受けた値を、下の分岐にもう一度書いたときも出ます。",
                "The arms of a `match` leave out a value of the enum, `true` or `false`, or `none` or `some x`; or an arm repeats a value an arm above takes."
            ),
            tr!("受けない値の分岐を足すか、重なった値を一つの分岐にまとめてください。", "Add an arm for each value left out, or keep each value in one arm."),
            "module example v1\n\nenum tier = basic | silver | gold\n\nfn points(t: tier) -> number\n  match t\n    basic => 1\n    silver => 2\n",
            "module 例(example) v1\n\nenum 区分(tier) = 一般(basic) | 銀(silver) | 金(gold)\n\nfn 点数(points)(区分(t): 区分) -> number\n  match 区分\n    一般 => 1\n    銀 => 2\n",
            &["E105"],
        ),
        e(
            "E107",
            tr!("呼び出しの引数が合いません", "The call does not give what it takes"),
            tr!(
                "関数、規則、日付の呼び出しで、引数が足りないとき、取らない引数を渡したとき、同じ引数を二度書いたとき、名前を付けずに渡したとき。組み込みの関数に名前を付けた引数を渡したとき、引数の数が違うとき、レコードを作るのにフィールドが足りないか二度書いたとき、関数や規則を呼ばずに値として書いたときも出ます。",
                "A call of a function, a rule or a date misses an argument, gives one it does not take, gives one twice, or gives one with no name; a builtin is given a named argument or the wrong number of them; a record is built with a field missing or given twice; a function or a rule is written as a value and not called."
            ),
            tr!(
                "引数を全部、名前を付けて一度ずつ書いてください（`charges(debt: d, days_late: n)`）。組み込みの関数には名前を付けずに順に渡します。",
                "Give every argument once, by name (`charges(debt: d, days_late: n)`); a builtin takes its arguments in order, with no names."
            ),
            "module example v1\n\nfn double(n: number range >=0 <=9) -> number\n  n * 2\n\nfn four() -> number\n  double()\n",
            "module 例(example) v1\n\nfn 倍(double)(数(n): number range >=0 <=9) -> number\n  数 * 2\n\nfn 四(four)() -> number\n  倍()\n",
            &["E204"],
        ),
        e(
            "E108",
            tr!("宣言が足りないか、成り立ちません", "A declaration is missing or cannot hold"),
            tr!(
                "数と日付の引数に範囲（`range`）が無いとき、リストの引数に要素の数の上限（`count`）が無いとき、率の引数に刻み（`rate[step 0.01%]`）が無いとき。引数の型から（リスト、オプショナル、ほかのレコードを通して）届くレコードのフィールドにも要ります。答えにだけ使うレコードのフィールドは範囲を省けます。率の刻みは、関数の答えとどのレコードのフィールドにも要ります（省けるのは `let` だけです）。範囲に値が一つも無いとき、端を二度書いたとき、数と日付でないものに `range` を、リストでないものに `count` を書いたとき、レコードが自分自身を含むときも出ます。",
                "A number or a date of a function's parameters declares no range (`range`), a list no most elements (`count`), a rate no step (`rate[step 0.01%]`); so must every field of a record a parameter's type reaches (through lists, optionals and other records), while a record only answered with may leave its ranges out. The rate of a function's answer and of any field of a record declares its step too (only a `let` may leave it out). Also a range that holds no value, an end written twice, a `range` on what is no number or date, a `count` on what is no list, and a record that holds itself."
            ),
            tr!(
                "範囲を書いてください（`qty : number range >=1 <=20`、`lines : list[Line] count >=1 <=30`）。範囲はホストとの約束で、生成したコードは入口でそれを確かめます。",
                "Declare the range (`qty : number range >=1 <=20`, `lines : list[Line] count >=1 <=30`): it is the promise to the host, which the generated code holds the arguments to."
            ),
            "module example v1\n\nfn double(n: number) -> number\n  n * 2\n",
            "module 例(example) v1\n\nfn 倍(double)(数(n): number) -> number\n  数 * 2\n",
            &["E104"],
        ),
        e(
            "E109",
            tr!("組み込みの関数の使い方が違います", "A builtin function is used in a way it has not"),
            tr!(
                "組み込みの関数の引数でないところに関数（`x => …`）を書いたとき、組み込みの関数が関数を取る引数に関数でないものを渡したとき、その関数の引数の数が違うとき、`text` に率や丸めていない値を渡したとき、`mod` の割る数が正の整数の定数でないとき、組み込みの関数を呼ばずに値として書いたとき。",
                "A function (`x => …`) is written where no builtin takes it; a builtin is given something that is no function where it takes one, or a function of the wrong number of parameters; `text` is given a rate or an unrounded value; the divisor of `mod` is no positive whole constant; or a builtin is written as a value and not called."
            ),
            tr!(
                "関数は `sum(xs, x => …)` のように組み込みの関数に渡すときだけ書き、ほかで使う計算は `fn` で名前を付けてください。",
                "Write a function only where a builtin takes it, as in `sum(xs, x => …)`; a computation used elsewhere is a `fn` with a name."
            ),
            "module example v1\n\nfn f() -> number\n  let twice = x => x * 2\n  1\n",
            "module 例(example) v1\n\nfn 一(one)() -> number\n  let 倍 = x => x * 2\n  1\n",
            &["E107"],
        ),
        e(
            "E110",
            tr!("二通りに読める語です", "A word reads two ways"),
            tr!(
                "列挙の値を書くところで、その語が値の名前としても、引数や `let` の名前としても読めるとき。",
                "Where a value of an enum goes, a word reads both as the value and as the name of a parameter or a `let`."
            ),
            tr!("どちらかの名前を替えるか、値を `<列挙>.<値>` と書いてください。", "Rename one of them, or write the value `<enum>.<value>`."),
            "module example v1\n\nenum tier = basic | gold\n\nfn is_gold(t: tier, gold: bool) -> bool\n  t = gold\n",
            "module 例(example) v1\n\nenum 区分(tier) = 一般(basic) | 金(gold)\n\nfn 金か(is_gold)(区分(t): 区分, 金(gold): bool) -> bool\n  区分 = 金\n",
            &["E006", "E101"],
        ),
        e(
            "E111",
            tr!("ブロックに答えがありません", "A block has no answer"),
            tr!(
                "ブロックの最後の行が値を表す式でないとき（`let` で終わる、本体が無い）、`if` に `else` が無いとき、ブロックの答えのあとにまだ行があるとき、`if`、`else`、`match` の分岐、`let x =` の下にブロックが無いとき。",
                "The last line of a block is no expression with a value (it ends with a `let`, or there is no body); an `if` has no `else`; there are lines after a block's answer; or an `if`, an `else`, an arm of a `match` or a `let x =` has no block under it."
            ),
            tr!(
                "ブロックの最後に答えの式を書き、`if` には `else` を書いてください。値が無いことは `none` で言えます。",
                "End a block with the expression it answers, and give an `if` its `else`; `none` says there is no value."
            ),
            "module example v1\n\nfn late(days: number range >=0 <=9) -> number?\n  if days >= 1\n    days\n",
            "module 例(example) v1\n\nfn 遅れ(late)(日数(days): number range >=0 <=9) -> number?\n  if 日数 >= 1\n    日数\n",
            &["E004"],
        ),
        // ── Files a module reads ──
        e(
            "E201",
            tr!("読むファイルを読めません", "A file this module uses cannot be read"),
            tr!(
                "`use` で読むファイルが無いとき、拡張子が種類に合わないとき（`use rule` は `.rule`、`use dates` は `.cal`、`use book` は `.book`、`use module` は `.kn`）、その言語の検査を通らないとき（rulec、koyomi、chobo、kaname が言うことを注に挙げます）。",
                "A file a `use` reads is not there, its extension is not its kind's (`use rule` reads a `.rule`, `use dates` a `.cal`, `use book` a `.book`, `use module` a `.kn`), or it does not pass its language's check (the notes give what rulec, koyomi, chobo or kaname says of it)."
            ),
            tr!(
                "パスを確かめてください（パスは、このモジュールのファイルのあるディレクトリから書きます）。検査を通らないファイルは、その言語の検査が通るように直してください。",
                "Check the path (it is written from the directory of the module's file); correct a file that does not pass until its language's check passes it."
            ),
            "module example v1\n\nuse module pricing from \"pricing.kn\"\n",
            "module 例(example) v1\n\nuse module 価格 from \"価格.kn\"\n",
            &["E202", "E203"],
        ),
        e(
            "E202",
            tr!("規則と日付と帳簿は、`ritsu kaname` で読みます", "Rules, dates and books are read with ritsu kaname"),
            tr!(
                "kaname 単独のバイナリ（kaname のクレートのもの）で、規則、日付のファイル、帳簿を読むモジュールを確かめたとき。このバイナリには、rulec も koyomi も chobo も入っていません。exit code は 2 です。",
                "A module that reads a rule, a dates file or a book is checked with the binary of kaname's own crate, which holds no rulec, koyomi or chobo. The exit code is 2."
            ),
            tr!("同じコマンドを `ritsu kaname …` で走らせてください（`ritsu kaname check payout.kn`）。", "Run the same command as `ritsu kaname …` (`ritsu kaname check payout.kn`)."),
            "module example v1\n\nuse rule card_fee from \"card_fee.rule\"\n",
            "module 例(example) v1\n\nuse rule カード手数料 from \"カード手数料.rule\"\n",
            &["E201"],
        ),
        with_files(
            "E203",
            tr!("モジュールの読み込みが輪になっています", "The modules use each other in a ring"),
            tr!(
                "`use module` でたどると、読んでいるモジュールに戻ってくるとき。輪の中のモジュールは、どれもこのコードを出します。",
                "Following the `use module` lines comes back to a module being read; every module in the ring says it."
            ),
            tr!(
                "読む向きを一つにしてください。両方が使うものは、両方が読む別のモジュールに置けます。関数の相互の再帰は、一つのモジュールの中で書けます。",
                "Make the modules read one way only: what both need can go in a third module both read. Functions that call each other are written in one module."
            ),
            ("module example v1\n\nuse module other from \"other.kn\"\n", RING_EN),
            ("module 例(example) v1\n\nuse module ほか from \"ほか.kn\"\n", RING_JA),
            &["E201"],
        ),
        with_files(
            "E204",
            tr!("振替の呼び出しが、帳簿に合いません", "The transfer is not one of the book's"),
            tr!(
                "帳簿に無い振替や操作を書いたとき（すぐに確定する振替の操作は `do`、仮押さえする振替の操作は `hold`、`post`、`void`）、振替の引数が足りないとき、取らない引数を渡したとき、一部を確定する `post` で額の引数の一部だけを書いたとき。",
                "A transfer or an operation the book does not have is written (a transfer done at once takes `do`, one that holds first `hold`, `post` and `void`); an argument of the transfer is missing, or one it does not take is given; or a `post` of a part gives only some of the amounts."
            ),
            tr!(
                "帳簿の振替と操作を、`<帳簿>.<振替>.<操作>(<引数>: …)` の形で書いてください。`do` と `hold` は引数を全部、`post` と `void` はキーの引数を書きます。",
                "Write the book's transfer and operation as `<book>.<transfer>.<operation>(<argument>: …)`: `do` and `hold` give every argument, `post` and `void` the arguments of the key."
            ),
            (
                "module example v1\n\nuse book ledger from \"ledger.book\"\n\nfn take(payment: string, seller: string, amount: money[USDc] range >=1USDc <=100_000USDc) -> ledger.capture.do\n  ledger.capture.hold(payment: payment, seller: seller, amount: amount)\n",
                LEDGER_EN,
            ),
            (
                "module 例(example) v1\n\nuse book 台帳 from \"台帳.book\"\n\nfn 受ける(take)(支払い(payment): string, 売り手(seller): string, 金額(amount): money[USDc] range >=1USDc <=100_000USDc) -> 台帳.入金.do\n  台帳.入金.hold(支払い: 支払い, 売り手: 売り手, 金額: 金額)\n",
                LEDGER_JA,
            ),
            &["E107"],
        ),
        // ── The analysis of ranges ──
        e(
            "E301",
            tr!("値が、渡す先の範囲の外になることがあります", "A value can be outside the range of where it is given"),
            tr!(
                "規則の入力（数の入力と、値で範囲を書いた順序のある列挙の入力）、日付の整数の入力、関数の引数と答え、レコードのフィールド、`isqrt` の引数に渡す値が、その範囲の外になる、関数の実際の入力を見つけたとき。注に、その入力と、そのとき渡る値を書きます。`check` は、範囲の解析が収まると示せない場所についてだけ、関数のベクタを流して例を探します。",
                "An input of the function is found for which a value given to an input of a rule (a number, or an ordered enum the rule holds to a range over its values), an integer input of a date, a function's parameter or answer, a field of a record or `isqrt` is outside its range; the notes give the input and the value. The vectors of the function are run to look for one where the analysis of ranges cannot show the value stays in."
            ),
            tr!("渡す前に `if` か `match` でその範囲に狭めるか、渡す先の範囲を広げてください。", "Narrow the value to the range with an `if` or a `match` before it goes there, or widen the range it goes to."),
            "module example v1\n\nfn charge(days: number range >=1 <=30) -> number\n  days\n\nfn late(paid: date range >=2026-01-01 <=2026-12-31, due: date range >=2026-01-01 <=2026-12-31) -> number\n  charge(days: paid - due)\n",
            "module 例(example) v1\n\nfn 請求(charge)(日数(days): number range >=1 <=30) -> number\n  日数\n\nfn 遅れ(late)(支払日(paid): date range >=2026-01-01 <=2026-12-31, 期日(due): date range >=2026-01-01 <=2026-12-31) -> number\n  請求(日数: 支払日 - 期日)\n",
            &["W301", "E401"],
        ),
        e(
            "W301",
            tr!("値が渡す先の範囲に収まるかを決められません", "Whether a value stays in the range of where it is given cannot be decided"),
            tr!(
                "範囲の解析が、渡す値が範囲に収まると示せず、関数のベクタにも範囲の外にする入力が無かったとき。生成したコードが、走らせたときにその場所で確かめます。範囲の解析は区間で数えるので、同じ値が二度出る式（`side * (10 - side)`）や、再帰の答えは、本当より広く見積もります。",
                "The analysis of ranges cannot show that the value stays in the range, and no vector of the function takes it out; the generated code checks it there when it runs. The analysis counts in intervals, so an expression that uses one value twice (`side * (10 - side)`) or the answer of a recursion is estimated wider than it is."
            ),
            tr!("`if` か `match` で範囲に狭めると、範囲の解析がそれを読んで示せるようになります。", "Narrow the value with an `if` or a `match`, which the analysis reads, and it is shown."),
            "module example v1\n\nfn area(side: number range >=1 <=8) -> number range >=0 <=24\n  side * (10 - side)\n",
            "module 例(example) v1\n\nfn 面積(area)(辺(side): number range >=1 <=8) -> number range >=0 <=24\n  辺 * (10 - 辺)\n",
            &["E301"],
        ),
        e(
            "E302",
            tr!("途中の値が int64 を超えることがあります", "A value can leave int64"),
            tr!(
                "途中の値を既約分数にした分子か分母が int64 に収まらない、関数の実際の入力を見つけたとき。注に、その入力を書きます。",
                "An input of the function is found for which a value on the way, as a fraction in lowest terms, has a numerator or a denominator outside int64; the notes give the input."
            ),
            tr!("入力の範囲を狭めるか、先に丸めてください。", "Narrow the ranges of the inputs, or round sooner."),
            "module example v1\n\nfn double(n: number range >=0 <=5_000_000_000_000_000_000) -> number\n  n * 2\n",
            "module 例(example) v1\n\nfn 倍(double)(数(n): number range >=0 <=5_000_000_000_000_000_000) -> number\n  数 * 2\n",
            &["W302"],
        ),
        e(
            "W302",
            tr!("途中の値が int64 に収まるかを決められません", "Whether a value stays in int64 cannot be decided"),
            tr!(
                "範囲の解析が、途中の値が int64 に収まると示せず、関数のベクタにも超える入力が無かったとき。範囲の解析は、範囲を書かない再帰の答えを、どの値にもなりうるものとして数えます。生成したコードが、走らせたときに確かめます。",
                "The analysis of ranges cannot show that a value on the way stays in int64, and no vector of the function takes it out; the answer of a recursion with no range written is counted as any value. The generated code checks it when it runs."
            ),
            tr!("入力の範囲を狭めるか、先に丸めてください。再帰の関数には、答えの範囲を書いてください。", "Narrow the ranges of the inputs, or round sooner; give a recursive function the range of its answer."),
            "module example v1\n\nfn steps(n: number range >=0 <=100) -> number\n  if n <= 0 then 0 else steps(n: n - 1) + 1\n",
            "module 例(example) v1\n\nfn 段数(steps)(数(n): number range >=0 <=100) -> number\n  if 数 <= 0 then 0 else 段数(数: 数 - 1) + 1\n",
            &["E302"],
        ),
        e(
            "E303",
            tr!("割る数が 0 になることがあります", "A divisor can be 0"),
            tr!(
                "割る数（`allocate` の三つ目の引数も）が 0 になる、関数の実際の入力を見つけたとき。注に、その入力を書きます。",
                "An input of the function is found for which a divisor (the third argument of `allocate` too) is 0; the notes give the input."
            ),
            tr!("割る前に `if` で 0 を除くか、割る数の範囲から 0 を外してください。", "Leave 0 out with an `if` before the division, or out of the divisor's range."),
            "module example v1\n\nfn each(amount: money[USDc] range >=0USDc <=1_000USDc, people: number range >=0 <=10) -> money[USDc]\n  down(amount / people, 1USDc)\n",
            "module 例(example) v1\n\nfn 一人分(each)(金額(amount): money[USDc] range >=0USDc <=1_000USDc, 人数(people): number range >=0 <=10) -> money[USDc]\n  down(金額 / 人数, 1USDc)\n",
            &["W303"],
        ),
        e(
            "W303",
            tr!("割る数が 0 にならないかを決められません", "Whether a divisor stays away from 0 cannot be decided"),
            tr!(
                "範囲の解析が、割る数が 0 にならないと示せず、関数のベクタにも 0 にする入力が無かったとき。名前の付いた値を `if` で比べると狭まりますが、二つの式のあいだの関係（`any` が成り立てば `filter` の要素は一つ以上）は読みません。生成したコードが、割る前に確かめます。",
                "The analysis of ranges cannot show that a divisor stays away from 0, and no vector of the function makes it 0. A value with a name is narrowed by an `if` that compares it, but no relation between two expressions is read (that `filter` keeps an element when `any` holds). The generated code checks it before it divides."
            ),
            tr!("割る数に名前を付け、割る前に `if` で 0 を除いてください。", "Give the divisor a name, and leave 0 out with an `if` before the division."),
            "module example v1\n\nrecord Line\n  amount : money[USDc]  range >=0USDc <=1_000USDc\n  media  : bool\n\nfn media_average(lines: list[Line] count >=0 <=10) -> money[USDc]?\n  if any(lines, l => l.media)\n    down(sum(lines, l => l.amount) / count(filter(lines, l => l.media)), 1USDc)\n  else\n    none\n",
            "module 例(example) v1\n\nrecord 明細(Line)\n  金額(amount) : money[USDc]  range >=0USDc <=1_000USDc\n  メディア(media) : bool\n\nfn メディアの平均(media_average)(明細(lines): list[明細] count >=0 <=10) -> money[USDc]?\n  if any(明細, 一件 => 一件.メディア)\n    down(sum(明細, 一件 => 一件.金額) / count(filter(明細, 一件 => 一件.メディア)), 1USDc)\n  else\n    none\n",
            &["E303"],
        ),
        // ── Effects ──
        e(
            "E401",
            tr!("関数のエフェクトが、書いたエフェクトに収まりません", "The function does more than the effect written on it"),
            tr!(
                "`total` と書いた関数に、実行時に失敗しうる場所か、輪になる呼び出し（再帰）があるとき。`fails` に書いていない種類の失敗が起きうるとき、`diverges` と書かずに再帰があるときも出ます。呼んだ関数のエフェクトも、呼んだ側のエフェクトに入ります。言語をまたぐ判定（ritsu の X2・X3・X4・X6）が決める場所は、その答えで数えます。示せなかった場所と例のある場所は、ほかの場所と同じく、実行時に失敗しうる場所です。答えを受け取らない実行では、その場所は答えを待つ場所で、この確かめに使いません。",
                "A function written `total` has a place that can fail when it runs, or calls that go round (a recursion); or a function can fail with a kind its `fails` does not list, or recurs with no `diverges`. The effects of the functions it calls count. A place the checks across the languages decide (ritsu's X2, X3, X4 and X6) counts by their answer: one they do not show, or find an example for, can fail when it runs, as any other place; where the run has no answer, the place waits for one and does not count."
            ),
            tr!(
                "注に挙げた場所を示せるようにする（範囲を狭める分岐を書く）か、書いたエフェクトを広げてください。再帰する関数には `diverges` と書きます。",
                "Make the places the notes give shown (write the branch that narrows the value), or widen the effect written; a recursive function is written `diverges`."
            ),
            "module example v1\n\nfn steps(n: number range >=0 <=100) -> number range >=0 <=100 total\n  if n <= 0 then 0 else min(steps(n: n - 1) + 1, 100)\n",
            "module 例(example) v1\n\nfn 段数(steps)(数(n): number range >=0 <=100) -> number range >=0 <=100 total\n  if 数 <= 0 then 0 else min(段数(数: 数 - 1) + 1, 100)\n",
            &["W301", "W302", "W303"],
        ),
        // ── A decision in code ──
        e(
            "W501",
            tr!("表に書くべき判断が、コードにあります", "A decision is written in code; a table should hold it"),
            tr!(
                "`if` か `match` が、業務の定数（金額、量、率、日付、列挙の値、`true` と `false`、それだけでできたレコード。`match` では数も）を答えにするとき。業務の値と 0 でないしきい値（金額、量、率、日付、数のリテラル）の比べを、`if` の条件と `match` で分ける値のほかで値にするときも出ます。答えが `none` の分岐、`none` と `some x` に分けるだけの `match`、別の列挙の同じ名前の値に対応づけるだけの `match`、0 との比べは、警告になりません。",
                "An `if` or a `match` answers a constant of the business (an amount, a quantity, a rate, a date, a value of an enum, `true` or `false`, a record of those; in a `match`, a number too); or a comparison of a value of the business with a threshold that is not 0 (a literal amount, quantity, rate, date or number) is made a value elsewhere than in the condition of an `if` or what a `match` splits. A branch that answers `none`, a `match` that only splits `none` from `some x`, one that maps the values of an enum to those of the same names of another, and a comparison with 0 are not warned of."
            ),
            tr!(
                "判断を rulec の表に書いて、規則を呼んでください。表に書けば、完全性と重なりを rulec が確かめ、承認する人がページで読めます。",
                "Write the decision in a rulec table and call the rule: there, rulec checks it is complete and without overlap, and the person who approves it reads it on its page."
            ),
            "module example v1\n\nfn shipping(total: money[USD] range >=0USD <=10_000USD) -> money[USD]\n  if total >= 35USD then 0USD else 5USD\n",
            "module 例(example) v1\n\nfn 送料(shipping)(合計(total): money[USD] range >=0USD <=10_000USD) -> money[USD]\n  if 合計 >= 35USD then 0USD else 5USD\n",
            &[],
        ),
        // ── Examples ──
        e(
            "E601",
            tr!("例が成り立ちません", "An example does not hold"),
            tr!(
                "`examples` の行の関数を、ホストが呼ぶときと同じように参照インタプリタで流した答えが、書いてある答えと違うとき。書いてある種類で失敗しないとき、引数が関数の範囲の外にあるとき（種類 `input` の失敗）、呼び出しが深すぎて評価が止まらないときも出ます。",
                "The function of a line of `examples`, run in the reference interpreter as a host would call it, answers other than the line writes; or does not fail with the kind written, is given an argument outside the function's ranges (the failure `input`), or does not stop, its calls going too deep."
            ),
            tr!(
                "関数か、例に書いた答えのどちらかを直してください。失敗を書くときは `fails range` のように種類を書きます。",
                "Correct the function, or the answer the example writes. A failure is written with its kind, as in `fails range`."
            ),
            "module example v1\n\nfn double(n: number range >=0 <=9) -> number\n  n * 2\n\nexamples\n  double(n: 2) = 5\n",
            "module 例(example) v1\n\nfn 倍(double)(数(n): number range >=0 <=9) -> number\n  数 * 2\n\nexamples\n  倍(数: 2) = 5\n",
            &["E602"],
        ),
        e(
            "E602",
            tr!("例の書き方が違います", "An example is not a call with an answer of its type"),
            tr!(
                "`examples` の行が、このモジュールの関数の呼び出しでないとき、呼び出しのあとに `= <答え>` も `fails <種類>` も無いとき、答えがその関数の答えの型の値でないとき。",
                "A line of `examples` is no call of a function of the module, has neither `= <answer>` nor `fails <kind>` after the call, or gives an answer that is not a value of the function's type."
            ),
            tr!(
                "`<関数>(<引数>: <値>, …) = <答え>` か `<関数>(…) fails <種類>` と書いてください。続きは、より深く字下げして次の行に書けます。",
                "Write `<function>(<parameter>: <value>, …) = <answer>`, or `<function>(…) fails <kind>`; it may go on on the lines below, indented deeper."
            ),
            "module example v1\n\nfn double(n: number range >=0 <=9) -> number\n  n * 2\n\nexamples\n  double(n: 2) = true\n",
            "module 例(example) v1\n\nfn 倍(double)(数(n): number range >=0 <=9) -> number\n  数 * 2\n\nexamples\n  倍(数: 2) = true\n",
            &["E601"],
        ),
        // ── Security (ritsu's DESIGN 16) ──
        e(
            "W901",
            tr!("鍵の形の値が書かれています", "A value in the shape of a key is written"),
            tr!(
                "`.kn` のどこか（文字列でもコメントでも）に、プロバイダーが形を決めている鍵の形の値（AWS のアクセスキー ID、GitHub・Slack・Stripe・OpenAI・Anthropic・Google の鍵やトークン、Slack の Incoming Webhook の URL、PEM の秘密鍵）があるとき。モジュールが読めないときも出ます。診断には鍵の種類と、接頭辞と、長さだけを出し、鍵そのものも、その行も出しません。",
                "Somewhere in a `.kn`, in a string or a comment, there is a value in a shape its provider fixes for a key: an AWS access key ID, a key or token of GitHub, Slack, Stripe, OpenAI, Anthropic or Google, a Slack incoming webhook URL, a PEM private key. It is said of a module that does not read too. The diagnostic gives the kind of key, its prefix and its length, and never the key nor its line."
            ),
            tr!(
                "鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。本物の鍵なら、まずプロバイダーで無効にしてください。ファイルから消しても、リポジトリの履歴には残ります。テスト用の値なら、同じ行のコメントに `ritsu: test secret` と書いてください。",
                "Keep the key where the code runs (an environment variable, the platform's connection or secret store) and read it from there. If it is real, revoke it with its provider first: taking it out of the file leaves it in the history of the repository. If it is a value for tests, write `ritsu: test secret` in a comment on the same line."
            ),
            concat!("module maps v1\n\n# the maps API key of the staging site: AIzaSyD-ritsu-fake-", "key-for-tests-000000\nfn region(code: string) -> string\n  code\n"),
            concat!("module 地図(maps) v1\n\n# 検証用のサイトの地図の API キー: AIzaSyD-ritsu-fake-", "key-for-tests-000000\nfn 地域(region)(番号(code): string) -> string\n  番号\n"),
            &[],
        ),
    ];
    Ledger {
        tool: "kaname",
        example_file: "example.kn",
        fence: "kn",
        repro_heading: tr!("再現", "Example"),
        later_text: tr!("（まだ再現がありません。これを出す検査は、この先の段階で入ります）", "(no example yet: the check that prints it comes in a later stage)"),
        later_markdown: tr!("まだ再現がありません。これを出す検査は、この先の段階で入ります。", "No example yet: the check that prints it comes in a later stage."),
        entries,
    }
}
